//! Sanitized, revision-guarded quota samples. No scheduling, Usage or monetary writes.
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, path::Path, sync::Mutex};

/// Local freshness ceiling, not a promise of polling frequency.
pub const FRESHNESS_MS: i64 = 300_000;
/// Maximum requested time span (31 days).
pub const MAX_SPAN_MS: i64 = 31 * 86_400_000;

/// Value-free history failure.
#[derive(Debug)]
pub enum Error {
    /// Database or lock unavailable.
    Unavailable,
    /// Invalid bounded input or persisted projection.
    Invalid,
    /// Account changed, disappeared, or conflicting same-time evidence arrived.
    Conflict,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "quota history {self:?}")
    }
}
impl std::error::Error for Error {}
impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::Unavailable
    }
}

/// Exact account namespace. Native accounts use an empty config version.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AccountScope {
    /// `credential` or `native`.
    pub account_kind: String,
    /// Ordinary credential's immutable configuration scope; empty for native.
    pub config_version_id: String,
    /// Internal account identifier, never a provider subject or credential.
    pub account_id: String,
}
/// In-memory pre-read fence; never serialized or exposed in diagnostics.
pub struct Fence {
    account: AccountScope,
    revision: i64,
    identity: [u8; 32],
}
/// One allowlisted window or a windowless failure/unknown sample.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Sample {
    /// Channel, distinct from transport source.
    pub provider: String,
    /// `rest`, `billing`, or `grpc`.
    pub source: String,
    /// Structural window identity; empty only for a windowless result.
    pub window: String,
    /// Completion time of the upstream observation, unchanged on cache hits.
    pub observed_at_ms: i64,
    /// `observed` for values, `unknown` for failure/absence.
    pub confidence: String,
    /// `projected`, `partial`, or `unknown`; projected covers only recognized fields.
    pub completeness: String,
    /// `observed`, `exhausted`, `unknown`, or `failed`.
    pub status: String,
    /// Used percentage, not tokens or money.
    pub used_percent: Option<f64>,
    /// Independently observed amount in the provider unit.
    pub used: Option<f64>,
    /// Independently observed limit in the provider unit.
    pub limit: Option<f64>,
    /// Allowlisted unit only.
    pub unit: Option<String>,
    /// Explicit provider reset time; never inferred from duration.
    pub reset_at_ms: Option<i64>,
    /// Explicit provider duration.
    pub duration_seconds: Option<i64>,
}
impl Sample {
    /// Creates a windowless result without manufacturing a zero quota.
    #[must_use]
    pub fn missing(provider: &str, observed_at_ms: i64, failed: bool) -> Self {
        Self {
            provider: provider.into(),
            source: if provider == "grok_build" {
                "billing"
            } else {
                "rest"
            }
            .into(),
            window: String::new(),
            observed_at_ms,
            confidence: "unknown".into(),
            completeness: "unknown".into(),
            status: if failed { "failed" } else { "unknown" }.into(),
            used_percent: None,
            used: None,
            limit: None,
            unit: None,
            reset_at_ms: None,
            duration_seconds: None,
        }
    }
    fn valid_until(&self) -> i64 {
        if self.status == "unknown" || self.status == "failed" {
            return self.observed_at_ms;
        }
        self.reset_at_ms
            .unwrap_or(i64::MAX)
            .min(self.observed_at_ms.saturating_add(FRESHNESS_MS))
            .max(self.observed_at_ms)
    }
    fn validate(&self) -> Result<(), Error> {
        if !matches!(
            self.provider.as_str(),
            "codex" | "claude" | "kiro" | "kimi" | "grok_build" | "grok_console" | "grok_web"
        ) || !matches!(self.source.as_str(), "rest" | "billing" | "grpc")
            || self.window.len() > 128
            || !self
                .window
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || self.observed_at_ms < 0
            || !matches!(self.confidence.as_str(), "observed" | "unknown")
            || !matches!(
                self.completeness.as_str(),
                "projected" | "partial" | "unknown"
            )
            || !matches!(
                self.status.as_str(),
                "observed" | "exhausted" | "unknown" | "failed"
            )
            || [self.used_percent, self.used, self.limit]
                .into_iter()
                .flatten()
                .any(|n| !n.is_finite() || n < 0.0)
            || self.reset_at_ms.is_some_and(|n| n < 0)
            || self.duration_seconds.is_some_and(|n| n <= 0)
            || self
                .unit
                .as_deref()
                .is_some_and(|s| !matches!(s, "percent" | "requests" | "credits" | "tokens"))
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
/// Bounded history filters. Pagination uses the last returned durable row ID.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// Inclusive observation time.
    pub from_ms: i64,
    /// Exclusive observation time, at most 31 days after from.
    pub to_ms: i64,
    /// Page size, 1..=200.
    pub limit: usize,
    /// Exclusive durable ID, default zero.
    #[serde(default)]
    pub after_id: i64,
    /// Optional account kind.
    pub account_kind: Option<String>,
    /// Optional ordinary configuration scope.
    pub config_version_id: Option<String>,
    /// Optional internal account ID.
    pub account_id: Option<String>,
    /// Optional channel.
    pub provider: Option<String>,
    /// Optional exact structural window; windowless failures remain included.
    pub window: Option<String>,
}
/// History row plus explicitly limited freshness and reset evidence.
#[derive(Serialize)]
pub struct Entry {
    /// Durable monotonically increasing ID.
    pub id: i64,
    /// Account namespace.
    #[serde(flatten)]
    pub account: AccountScope,
    /// Revision observed at read start and atomically checked at append.
    pub revision: i64,
    /// Sanitized upstream projection.
    #[serde(flatten)]
    pub sample: Sample,
    /// Local freshness ceiling (also capped by explicit reset).
    pub valid_until_ms: i64,
    /// Freshness at the query's server time; independent of exhaustion/completeness.
    pub stale: bool,
    /// Unobserved interval before this sample within the requested time range.
    pub gap_before: Option<[i64; 2]>,
    /// A changed provider reset timestamp, not proof of a reset or replenishment.
    pub reported_reset_changed: bool,
}
/// A bounded page, with no aggregate balance or inferred consumption.
#[derive(Serialize)]
pub struct Page {
    /// Server time used to label freshness.
    pub as_of_ms: i64,
    /// True only when another matching row exists.
    pub has_more: bool,
    /// Pass as `after_id` with the same filters.
    pub next_after_id: Option<i64>,
    /// Samples only; unobserved time remains unknown, including after the last sample.
    pub coverage: &'static str,
    /// Ordered by append ID, not an assumption about arrival time.
    pub entries: Vec<Entry>,
}
/// Independent history connection; transactions never mutate account or serving state.
pub struct QuotaHistoryStore {
    connection: Mutex<Connection>,
}
impl QuotaHistoryStore {
    /// Opens a migrated database.
    /// # Errors
    /// Returns a value-free unavailable error for open/migration failures.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let mut connection = crate::open(path).map_err(|_| Error::Unavailable)?;
        crate::migrate(&mut connection).map_err(|_| Error::Unavailable)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
    /// Captures revision, lifecycle intent and encrypted-material identity before I/O.
    /// # Errors
    /// Missing accounts or mismatched expected revisions conflict.
    pub fn capture(
        &self,
        account: AccountScope,
        expected_revision: Option<i64>,
    ) -> Result<Fence, Error> {
        let c = self.connection.lock().map_err(|_| Error::Unavailable)?;
        let (revision, identity) = identity(&c, &account)?;
        if expected_revision.is_some_and(|r| r != revision) {
            return Err(Error::Conflict);
        }
        Ok(Fence {
            account,
            revision,
            identity,
        })
    }
    /// Appends one completed read atomically after rechecking its fence.
    /// Cache duplicates merge; older observations in the same revision are ignored.
    /// # Errors
    /// Conflicting same-time evidence, changed/deleted accounts, or failed writes are rejected.
    pub fn append(&self, fence: &Fence, samples: &[Sample]) -> Result<(), Error> {
        if samples.is_empty() || samples.len() > 8 {
            return Err(Error::Invalid);
        }
        let mut c = self.connection.lock().map_err(|_| Error::Unavailable)?;
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if identity(&tx, &fence.account)? != (fence.revision, fence.identity) {
            return Err(Error::Conflict);
        }
        for sample in samples {
            sample.validate()?;
            let encoded = serde_json::to_string(sample).map_err(|_| Error::Invalid)?;
            let previous: Option<(i64, String)> = tx.query_row(
                "SELECT observed_at_ms,evidence_json FROM quota_history WHERE account_kind=?1 AND config_version_id=?2 AND account_id=?3 AND revision=?4 AND provider=?5 AND source=?6 AND window=?7 ORDER BY observed_at_ms DESC,id DESC LIMIT 1",
                params![fence.account.account_kind,fence.account.config_version_id,fence.account.account_id,fence.revision,sample.provider,sample.source,sample.window],
                |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((time, previous)) = previous {
                if time > sample.observed_at_ms {
                    continue;
                }
                if time == sample.observed_at_ms {
                    if previous != encoded {
                        return Err(Error::Conflict);
                    }
                    continue;
                }
            }
            tx.execute("INSERT INTO quota_history(account_kind,config_version_id,account_id,revision,provider,source,window,observed_at_ms,evidence_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![fence.account.account_kind,fence.account.config_version_id,fence.account.account_id,fence.revision,sample.provider,sample.source,sample.window,sample.observed_at_ms,encoded])?;
        }
        tx.commit()?;
        Ok(())
    }
    /// Reads at most 200 samples and one predecessor per sample, in a single `SQLite` snapshot.
    /// # Errors
    /// Rejects invalid bounds and malformed stored data.
    pub fn query(&self, q: &Query, now: i64) -> Result<Page, Error> {
        if q.from_ms < 0
            || q.to_ms <= q.from_ms
            || q.to_ms - q.from_ms > MAX_SPAN_MS
            || !(1..=200).contains(&q.limit)
            || q.after_id < 0
            || [
                &q.account_kind,
                &q.config_version_id,
                &q.account_id,
                &q.provider,
                &q.window,
            ]
            .into_iter()
            .flatten()
            .any(|s| s.is_empty() || s.len() > 128)
            || q.account_kind
                .as_deref()
                .is_some_and(|s| !matches!(s, "native" | "credential"))
        {
            return Err(Error::Invalid);
        }
        let mut c = self.connection.lock().map_err(|_| Error::Unavailable)?;
        let tx = c.transaction()?;
        let mut statement = tx.prepare("SELECT id,account_kind,config_version_id,account_id,revision,evidence_json FROM quota_history WHERE observed_at_ms>=?1 AND observed_at_ms<?2 AND id>?3 AND (?4 IS NULL OR account_kind=?4) AND (?5 IS NULL OR config_version_id=?5) AND (?6 IS NULL OR account_id=?6) AND (?7 IS NULL OR provider=?7) AND (?8 IS NULL OR window=?8 OR window='') ORDER BY id LIMIT ?9")?;
        let mut rows = statement.query(params![
            q.from_ms,
            q.to_ms,
            q.after_id,
            q.account_kind,
            q.config_version_id,
            q.account_id,
            q.provider,
            q.window,
            i64::try_from(q.limit + 1).map_err(|_| Error::Invalid)?
        ])?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            let sample: Sample =
                serde_json::from_str(&row.get::<_, String>(5)?).map_err(|_| Error::Invalid)?;
            sample.validate()?;
            let account = AccountScope {
                account_kind: row.get(1)?,
                config_version_id: row.get(2)?,
                account_id: row.get(3)?,
            };
            let revision = row.get(4)?;
            let previous: Option<String> = tx.query_row("SELECT evidence_json FROM quota_history WHERE account_kind=?1 AND config_version_id=?2 AND account_id=?3 AND revision=?4 AND provider=?5 AND source=?6 AND window=?7 AND observed_at_ms<?8 ORDER BY observed_at_ms DESC,id DESC LIMIT 1",params![account.account_kind,account.config_version_id,account.account_id,revision,sample.provider,sample.source,sample.window,sample.observed_at_ms],|r|r.get(0)).optional()?;
            let previous = previous
                .map(|s| serde_json::from_str::<Sample>(&s).map_err(|_| Error::Invalid))
                .transpose()?;
            let coverage_end = previous
                .as_ref()
                .map_or(q.from_ms, Sample::valid_until)
                .max(q.from_ms);
            let valid_until_ms = sample.valid_until();
            entries.push(Entry {
                id: row.get(0)?,
                account,
                revision,
                stale: now >= valid_until_ms,
                valid_until_ms,
                gap_before: (coverage_end < sample.observed_at_ms)
                    .then_some([coverage_end, sample.observed_at_ms]),
                reported_reset_changed: sample.provider != "kiro"
                    && previous.as_ref().is_some_and(|p| {
                        p.reset_at_ms.is_some()
                            && sample.reset_at_ms.is_some()
                            && p.reset_at_ms != sample.reset_at_ms
                    }),
                sample,
            });
        }
        let has_more = entries.len() > q.limit;
        entries.truncate(q.limit);
        Ok(Page {
            as_of_ms: now,
            has_more,
            next_after_id: if has_more {
                entries.last().map(|e| e.id)
            } else {
                None
            },
            coverage: "sampled_only; gaps and time after valid_until are unknown; reset changes are provider reports",
            entries,
        })
    }
}
fn identity(c: &Connection, account: &AccountScope) -> Result<(i64, [u8; 32]), Error> {
    let (revision, material, status): (i64,Vec<u8>,String) = match account.account_kind.as_str() {
        "native" if account.config_version_id.is_empty() => c.query_row("SELECT revision,credential_ciphertext,provider || ':' || enabled || ':' || auth_status || ':' || created_at_ms FROM grok_accounts WHERE id=?1",[&account.account_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?,
        "credential" => c.query_row("SELECT c.revision,c.ciphertext,c.status || ':' || v.revision || ':' || v.status FROM upstream_credentials c JOIN config_versions v ON v.id=c.config_version_id WHERE c.config_version_id=?1 AND c.id=?2",params![account.config_version_id,account.account_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?,
        _ => return Err(Error::Invalid),
    }.ok_or(Error::Conflict)?;
    let mut digest = Sha256::new();
    digest.update(material);
    digest.update(status.as_bytes());
    Ok((revision, digest.finalize().into()))
}
