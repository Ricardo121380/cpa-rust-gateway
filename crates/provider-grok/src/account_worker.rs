//! Durable, bounded proactive work for native Grok credentials and quota state.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::Arc,
    thread,
};

use gateway_store::secret_store::{EncryptedSecret, KeyVersion, PlaintextSecret};
use rusqlite::{OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};

use crate::account_pool::{
    GrokAccountCredential, GrokAccountPoolError, GrokAccountPoolStore, GrokAccountProvider,
    credential_aad,
};

const MIN_REFRESH_JITTER_MS: i64 = 5 * 60 * 1_000;
const REFRESH_JITTER_RANGE_MS: u64 = 3 * 60 * 1_000 + 1;
const BASE_FAILURE_BACKOFF_MS: i64 = 60 * 1_000;
const MAX_FAILURE_BACKOFF_MS: i64 = 60 * 60 * 1_000;
const FAILURE_JITTER_RANGE_MS: u64 = 30 * 1_000 + 1;
const MAX_WORKER_BATCH: usize = 256;
const MAX_WORKER_CONCURRENCY: usize = 64;
const MIN_CLAIM_LEASE_MS: i64 = 1_000;
const MAX_CLAIM_LEASE_MS: i64 = 10 * 60 * 1_000;
const MAX_QUOTA_WINDOWS_PER_ACCOUNT_SYNC: usize = 64;
const MAX_QUOTA_WINDOWS_PER_TARGET: usize = 8;

/// One durable native-account control operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokAccountWorkerKind {
    /// Refresh expiring provider credential material.
    Refresh,
    /// Synchronize sanitized account/model quota windows.
    Quota,
}

impl GrokAccountWorkerKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Refresh => "refresh",
            Self::Quota => "quota",
        }
    }

    const fn due_column(self) -> &'static str {
        match self {
            Self::Refresh => "refresh_due_at_ms",
            Self::Quota => "quota_sync_due_at_ms",
        }
    }

    const fn failure_column(self) -> &'static str {
        match self {
            Self::Refresh => "refresh_failure_count",
            Self::Quota => "quota_sync_failure_count",
        }
    }
}

/// Sanitized quota scope retained independently for one native account.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GrokAccountQuotaScope {
    /// Window shared by all models through the account.
    Account,
    /// Window scoped to one exact upstream model label.
    Model(String),
}

/// Allow-listed origin of one native quota observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokAccountQuotaSource {
    /// Provider billing/account endpoint.
    Billing,
    /// Provider REST status endpoint.
    Rest,
    /// Provider gRPC status endpoint.
    Grpc,
    /// Bounded local estimate.
    Estimated,
}

impl GrokAccountQuotaSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Billing => "billing",
            Self::Rest => "rest",
            Self::Grpc => "grpc",
            Self::Estimated => "estimated",
        }
    }
}

/// Confidence assigned to one sanitized native quota observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokAccountQuotaConfidence {
    /// Provider endpoint is authoritative for this window.
    Authoritative,
    /// Direct but non-authoritative observation.
    Observed,
    /// Locally estimated window.
    Estimated,
}

impl GrokAccountQuotaConfidence {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Authoritative => "authoritative",
            Self::Observed => "observed",
            Self::Estimated => "estimated",
        }
    }
}

/// One bounded quota window returned by an injected provider control operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokAccountQuotaWindow {
    scope: GrokAccountQuotaScope,
    label: String,
    limit: Option<u64>,
    remaining: Option<u64>,
    reset_at_ms: Option<i64>,
    source: GrokAccountQuotaSource,
    confidence: GrokAccountQuotaConfidence,
}

impl GrokAccountQuotaWindow {
    /// Validates one sanitized account/model quota window.
    ///
    /// # Errors
    ///
    /// Returns [`GrokAccountWorkerError::InvalidRequest`] for unsafe labels, impossible counts, or
    /// timestamps outside the supported domain.
    pub fn try_new(
        scope: GrokAccountQuotaScope,
        label: impl Into<String>,
        limit: Option<u64>,
        remaining: Option<u64>,
        reset_at_ms: Option<i64>,
        source: GrokAccountQuotaSource,
        confidence: GrokAccountQuotaConfidence,
    ) -> Result<Self, GrokAccountWorkerError> {
        let label = label.into();
        let scope_valid = match &scope {
            GrokAccountQuotaScope::Account => true,
            GrokAccountQuotaScope::Model(model) => !model.trim().is_empty() && model.len() <= 256,
        };
        if !scope_valid
            || label.trim().is_empty()
            || label.len() > 64
            || limit.is_some_and(|value| value > i64::MAX.unsigned_abs())
            || remaining.is_some_and(|value| value > i64::MAX.unsigned_abs())
            || limit
                .zip(remaining)
                .is_some_and(|(limit, remaining)| remaining > limit)
            || reset_at_ms.is_some_and(|value| value < 0)
        {
            return Err(GrokAccountWorkerError::InvalidRequest);
        }
        Ok(Self {
            scope,
            label,
            limit,
            remaining,
            reset_at_ms,
            source,
            confidence,
        })
    }
}

/// One non-cloneable durable worker claim and its authenticated credential plaintext.
pub struct GrokAccountWorkerJob {
    account_id: String,
    provider: GrokAccountProvider,
    kind: GrokAccountWorkerKind,
    claim_id: String,
    expected_revision: u64,
    identity_digest: [u8; 32],
    credential: PlaintextSecret,
}

impl GrokAccountWorkerJob {
    /// Returns the opaque native account identifier.
    #[must_use]
    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    /// Returns the isolated provider namespace.
    #[must_use]
    pub const fn provider(&self) -> GrokAccountProvider {
        self.provider
    }

    /// Returns the claimed operation kind.
    #[must_use]
    pub const fn kind(&self) -> GrokAccountWorkerKind {
        self.kind
    }

    /// Returns authenticated credential bytes only to the immediate injected control operation.
    #[must_use]
    pub fn credential_bytes(&self) -> &[u8] {
        self.credential.as_bytes()
    }
}

impl fmt::Debug for GrokAccountWorkerJob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokAccountWorkerJob")
            .field("account_id", &self.account_id)
            .field("provider", &self.provider)
            .field("kind", &self.kind)
            .field("claim_id", &"<redacted>")
            .field("expected_revision", &self.expected_revision)
            .field("credential", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Provider-independent result returned by one injected control operation.
pub enum GrokAccountWorkerResult {
    /// Credential refresh succeeded with a complete replacement and absolute expiry.
    Refreshed {
        /// Complete provider credential payload.
        credential: GrokAccountCredential,
        /// Absolute expiry used to derive deterministic pre-expiry scheduling jitter.
        expires_at_ms: i64,
    },
    /// Quota synchronization succeeded with a complete replacement snapshot.
    QuotaSynchronized {
        /// Sanitized account/model windows.
        windows: Vec<GrokAccountQuotaWindow>,
        /// Next proactive synchronization instant.
        next_due_at_ms: i64,
    },
    /// Retryable provider/transport failure.
    TransientFailure,
    /// Exact credential evidence requires reauthentication.
    ReauthRequired,
}

impl fmt::Debug for GrokAccountWorkerResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refreshed {
                credential,
                expires_at_ms,
            } => formatter
                .debug_struct("Refreshed")
                .field("credential", &credential)
                .field("expires_at_ms", expires_at_ms)
                .finish(),
            Self::QuotaSynchronized {
                windows,
                next_due_at_ms,
            } => formatter
                .debug_struct("QuotaSynchronized")
                .field("window_count", &windows.len())
                .field("next_due_at_ms", next_due_at_ms)
                .finish(),
            Self::TransientFailure => formatter.write_str("TransientFailure"),
            Self::ReauthRequired => formatter.write_str("ReauthRequired"),
        }
    }
}

/// Injected provider control operation; production implementations may reuse existing OAuth/quota
/// clients while tests remain network-free.
pub trait GrokAccountWorkerExecutor: Sync {
    /// Executes one already-claimed job without changing account persistence directly.
    fn execute(&self, job: &GrokAccountWorkerJob) -> GrokAccountWorkerResult;
}

/// Value-free result counts from one bounded worker pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrokAccountWorkerRunSummary {
    /// Durable jobs claimed by this pass.
    pub claimed: usize,
    /// Successful refresh/quota outcomes committed.
    pub succeeded: usize,
    /// Retryable failures committed with durable backoff.
    pub backed_off: usize,
    /// Accounts committed as requiring reauthentication.
    pub reauth_required: usize,
    /// Panicked executions whose claims remain until lease expiry for crash recovery.
    pub panicked: usize,
}

/// Bounded parallel coordinator around durable account claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrokAccountWorkerCoordinator {
    maximum_concurrency: usize,
    claim_lease_ms: i64,
}

impl GrokAccountWorkerCoordinator {
    /// Validates finite concurrency and durable claim lease bounds.
    ///
    /// # Errors
    ///
    /// Returns [`GrokAccountWorkerError::InvalidRequest`] for zero/excess concurrency or a lease
    /// outside one second through ten minutes.
    pub fn try_new(
        maximum_concurrency: usize,
        claim_lease_ms: i64,
    ) -> Result<Self, GrokAccountWorkerError> {
        if maximum_concurrency == 0
            || maximum_concurrency > MAX_WORKER_CONCURRENCY
            || !(MIN_CLAIM_LEASE_MS..=MAX_CLAIM_LEASE_MS).contains(&claim_lease_ms)
        {
            return Err(GrokAccountWorkerError::InvalidRequest);
        }
        Ok(Self {
            maximum_concurrency,
            claim_lease_ms,
        })
    }

    /// Claims and executes at most the configured number of due jobs in parallel.
    ///
    /// A panicked execution intentionally leaves its durable claim untouched. The account remains
    /// singleflight-blocked until claim expiry, after which a new process/pass may reclaim it.
    ///
    /// # Errors
    ///
    /// Returns a safe store/claim/outcome classification. Successfully committed sibling jobs are
    /// not rolled back if another independent job fails to commit.
    pub fn run_once<E: GrokAccountWorkerExecutor>(
        &self,
        store: &Arc<GrokAccountPoolStore>,
        kind: GrokAccountWorkerKind,
        observed_at_ms: i64,
        executor: &E,
    ) -> Result<GrokAccountWorkerRunSummary, GrokAccountWorkerError> {
        let jobs = store.claim_due_worker_jobs(
            kind,
            observed_at_ms,
            self.maximum_concurrency,
            self.claim_lease_ms,
        )?;
        let claimed = jobs.len();
        let completed = thread::scope(|scope| {
            jobs.into_iter()
                .map(|job| scope.spawn(move || (executor.execute(&job), job)))
                .collect::<Vec<_>>()
                .into_iter()
                .map(thread::ScopedJoinHandle::join)
                .collect::<Vec<_>>()
        });
        let mut summary = GrokAccountWorkerRunSummary {
            claimed,
            succeeded: 0,
            backed_off: 0,
            reauth_required: 0,
            panicked: 0,
        };
        for completed_job in completed {
            let Ok((result, job)) = completed_job else {
                summary.panicked += 1;
                continue;
            };
            match result {
                GrokAccountWorkerResult::Refreshed {
                    credential,
                    expires_at_ms,
                } => {
                    store.complete_refresh_job(&job, &credential, expires_at_ms, observed_at_ms)?;
                    summary.succeeded += 1;
                }
                GrokAccountWorkerResult::QuotaSynchronized {
                    windows,
                    next_due_at_ms,
                } => {
                    store.complete_quota_job(&job, &windows, next_due_at_ms, observed_at_ms)?;
                    summary.succeeded += 1;
                }
                GrokAccountWorkerResult::TransientFailure => {
                    store.complete_worker_failure(&job, false, observed_at_ms)?;
                    summary.backed_off += 1;
                }
                GrokAccountWorkerResult::ReauthRequired => {
                    store.complete_worker_failure(&job, true, observed_at_ms)?;
                    summary.reauth_required += 1;
                }
            }
        }
        Ok(summary)
    }
}

/// Safe durable worker failures without credential, claim, or provider response values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokAccountWorkerError {
    /// Configuration, timestamp, bounds, result shape, or quota window is invalid.
    InvalidRequest,
    /// Database or account-store lock operation failed.
    StoreUnavailable,
    /// Persisted account, claim, envelope, or counter state is malformed.
    InvalidPersistedState,
    /// Credential encryption/authentication failed.
    SecretStoreFailure,
    /// The claim expired, was superseded, or no longer owns the expected account revision.
    StaleClaim,
    /// Executor returned a result that does not match its claimed operation kind.
    ResultKindMismatch,
}

impl fmt::Display for GrokAccountWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidRequest => "native Grok worker request is invalid",
            Self::StoreUnavailable => "native Grok worker store is unavailable",
            Self::InvalidPersistedState => "native Grok worker state is invalid",
            Self::SecretStoreFailure => "native Grok worker credential encryption failed",
            Self::StaleClaim => "native Grok worker claim is stale",
            Self::ResultKindMismatch => "native Grok worker result kind does not match its claim",
        };
        formatter.write_str(message)
    }
}

impl Error for GrokAccountWorkerError {}

impl GrokAccountPoolStore {
    /// Atomically claims a bounded set of due active accounts for one worker kind.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, persistence, or authenticated-decryption classification. No
    /// partial claims commit if any selected account cannot be opened.
    pub fn claim_due_worker_jobs(
        &self,
        kind: GrokAccountWorkerKind,
        observed_at_ms: i64,
        limit: usize,
        claim_lease_ms: i64,
    ) -> Result<Vec<GrokAccountWorkerJob>, GrokAccountWorkerError> {
        if observed_at_ms < 0
            || limit == 0
            || limit > MAX_WORKER_BATCH
            || !(MIN_CLAIM_LEASE_MS..=MAX_CLAIM_LEASE_MS).contains(&claim_lease_ms)
        {
            return Err(GrokAccountWorkerError::InvalidRequest);
        }
        let claim_expires_at_ms = observed_at_ms
            .checked_add(claim_lease_ms)
            .ok_or(GrokAccountWorkerError::InvalidRequest)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        let rows = due_rows(&transaction, kind, observed_at_ms, limit)?;
        let mut jobs = Vec::with_capacity(rows.len());
        for row in rows {
            let claim_id = random_claim_id()?;
            let changed = transaction
                .execute(
                    "UPDATE grok_accounts SET worker_claim_kind = ?2, worker_claim_id = ?3, \
                            worker_claim_expires_at_ms = ?4, updated_at_ms = ?5 \
                     WHERE id = ?1 AND (worker_claim_id IS NULL OR worker_claim_expires_at_ms <= ?5)",
                    params![
                        row.account_id,
                        kind.as_str(),
                        claim_id,
                        claim_expires_at_ms,
                        observed_at_ms,
                    ],
                )
                .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
            if changed != 1 {
                return Err(GrokAccountWorkerError::StaleClaim);
            }
            let provider = GrokAccountProvider::parse(&row.provider)
                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
            let identity_digest: [u8; 32] = row
                .identity_digest
                .try_into()
                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
            let key_version = KeyVersion::try_from_sqlite_i64(row.key_version)
                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
            let encrypted = EncryptedSecret::try_from_persisted(key_version, row.ciphertext)
                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
            let credential = self
                .secret_store
                .open(&encrypted, &credential_aad(provider, &identity_digest))
                .map_err(|_| GrokAccountWorkerError::SecretStoreFailure)?;
            let expected_revision = u64::try_from(row.revision)
                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
            jobs.push(GrokAccountWorkerJob {
                account_id: row.account_id,
                provider,
                kind,
                claim_id,
                expected_revision,
                identity_digest,
                credential,
            });
        }
        transaction
            .commit()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        Ok(jobs)
    }

    fn complete_refresh_job(
        &self,
        job: &GrokAccountWorkerJob,
        credential: &GrokAccountCredential,
        expires_at_ms: i64,
        observed_at_ms: i64,
    ) -> Result<(), GrokAccountWorkerError> {
        if job.kind != GrokAccountWorkerKind::Refresh || expires_at_ms <= observed_at_ms {
            return Err(GrokAccountWorkerError::ResultKindMismatch);
        }
        let next_revision = job
            .expected_revision
            .checked_add(1)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(GrokAccountWorkerError::InvalidPersistedState)?;
        let refresh_due_at_ms = deterministic_refresh_due_at(&job.account_id, expires_at_ms)?;
        let encrypted = self
            .secret_store
            .seal(
                credential.as_bytes(),
                &credential_aad(job.provider, &job.identity_digest),
            )
            .map_err(|_| GrokAccountWorkerError::SecretStoreFailure)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        let changed = connection
            .execute(
                "UPDATE grok_accounts SET credential_ciphertext = ?1, credential_key_version = ?2, \
                        revision = ?3, auth_status = 'active', refresh_due_at_ms = ?4, \
                        last_refresh_at_ms = ?5, refresh_failure_count = 0, \
                        worker_claim_kind = NULL, worker_claim_id = NULL, \
                        worker_claim_expires_at_ms = NULL, updated_at_ms = ?5 \
                 WHERE id = ?6 AND revision = ?7 AND worker_claim_kind = 'refresh' \
                       AND worker_claim_id = ?8 AND worker_claim_expires_at_ms > ?5",
                params![
                    encrypted.ciphertext(),
                    encrypted.key_version().as_sqlite_i64(),
                    next_revision,
                    refresh_due_at_ms,
                    observed_at_ms,
                    job.account_id,
                    i64::try_from(job.expected_revision)
                        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?,
                    job.claim_id,
                ],
            )
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        if changed == 1 {
            Ok(())
        } else {
            Err(GrokAccountWorkerError::StaleClaim)
        }
    }

    fn complete_quota_job(
        &self,
        job: &GrokAccountWorkerJob,
        windows: &[GrokAccountQuotaWindow],
        next_due_at_ms: i64,
        observed_at_ms: i64,
    ) -> Result<(), GrokAccountWorkerError> {
        if job.kind != GrokAccountWorkerKind::Quota
            || windows.len() > MAX_QUOTA_WINDOWS_PER_ACCOUNT_SYNC
            || next_due_at_ms <= observed_at_ms
        {
            return Err(GrokAccountWorkerError::ResultKindMismatch);
        }
        validate_quota_windows(windows)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        let changed = transaction
            .execute(
                "UPDATE grok_accounts SET quota_sync_due_at_ms = ?1, last_quota_sync_at_ms = ?2, \
                        quota_sync_failure_count = 0, worker_claim_kind = NULL, \
                        worker_claim_id = NULL, worker_claim_expires_at_ms = NULL, updated_at_ms = ?2 \
                 WHERE id = ?3 AND revision = ?4 AND worker_claim_kind = 'quota' \
                       AND worker_claim_id = ?5 AND worker_claim_expires_at_ms > ?2",
                params![
                    next_due_at_ms,
                    observed_at_ms,
                    job.account_id,
                    i64::try_from(job.expected_revision)
                        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?,
                    job.claim_id,
                ],
            )
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        if changed != 1 {
            return Err(GrokAccountWorkerError::StaleClaim);
        }
        transaction
            .execute(
                "DELETE FROM grok_account_quota_windows WHERE account_id = ?1",
                [&job.account_id],
            )
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        for window in windows {
            insert_quota_window(&transaction, &job.account_id, window, observed_at_ms)?;
        }
        transaction
            .commit()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)
    }

    fn complete_worker_failure(
        &self,
        job: &GrokAccountWorkerJob,
        reauth_required: bool,
        observed_at_ms: i64,
    ) -> Result<(), GrokAccountWorkerError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        let failure_count = current_failure_count(&transaction, job, observed_at_ms)?;
        let next_failure_count = failure_count
            .checked_add(1)
            .ok_or(GrokAccountWorkerError::InvalidPersistedState)?;
        let next_failure_count_sql = i64::try_from(next_failure_count)
            .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
        let next_due_at_ms = if reauth_required {
            None
        } else {
            Some(deterministic_backoff_due_at(
                &job.account_id,
                next_failure_count,
                observed_at_ms,
            )?)
        };
        let (due_column, failure_column) = (job.kind.due_column(), job.kind.failure_column());
        let sql = format!(
            "UPDATE grok_accounts SET {due_column} = ?1, {failure_column} = ?2, \
                    auth_status = CASE WHEN ?3 = 1 THEN 'reauth_required' ELSE auth_status END, \
                    worker_claim_kind = NULL, worker_claim_id = NULL, \
                    worker_claim_expires_at_ms = NULL, updated_at_ms = ?4 \
             WHERE id = ?5 AND revision = ?6 AND worker_claim_kind = ?7 \
                   AND worker_claim_id = ?8 AND worker_claim_expires_at_ms > ?4"
        );
        let changed = transaction
            .execute(
                &sql,
                params![
                    next_due_at_ms,
                    next_failure_count_sql,
                    i64::from(reauth_required),
                    observed_at_ms,
                    job.account_id,
                    i64::try_from(job.expected_revision)
                        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?,
                    job.kind.as_str(),
                    job.claim_id,
                ],
            )
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        if changed != 1 {
            return Err(GrokAccountWorkerError::StaleClaim);
        }
        if reauth_required {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO grok_account_reauth_state (account_id) \
                     VALUES (?1)",
                    [&job.account_id],
                )
                .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        }
        transaction
            .commit()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)
    }
}

struct DueAccountRow {
    account_id: String,
    provider: String,
    identity_digest: Vec<u8>,
    ciphertext: Vec<u8>,
    key_version: i64,
    revision: i64,
}

fn due_rows(
    transaction: &Transaction<'_>,
    kind: GrokAccountWorkerKind,
    observed_at_ms: i64,
    limit: usize,
) -> Result<Vec<DueAccountRow>, GrokAccountWorkerError> {
    let due_column = kind.due_column();
    let sql = format!(
        "SELECT id, provider, identity_digest, credential_ciphertext, \
                credential_key_version, revision \
         FROM grok_accounts \
         WHERE enabled = 1 AND auth_status = 'active' \
               AND {due_column} IS NOT NULL AND {due_column} <= ?1 \
               AND (worker_claim_id IS NULL OR worker_claim_expires_at_ms <= ?1) \
         ORDER BY {due_column}, id LIMIT ?2"
    );
    let mut statement = transaction
        .prepare(&sql)
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
    statement
        .query_map(
            params![
                observed_at_ms,
                i64::try_from(limit).map_err(|_| GrokAccountWorkerError::InvalidRequest)?
            ],
            |row| {
                Ok(DueAccountRow {
                    account_id: row.get(0)?,
                    provider: row.get(1)?,
                    identity_digest: row.get(2)?,
                    ciphertext: row.get(3)?,
                    key_version: row.get(4)?,
                    revision: row.get(5)?,
                })
            },
        )
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)
}

fn current_failure_count(
    transaction: &Transaction<'_>,
    job: &GrokAccountWorkerJob,
    observed_at_ms: i64,
) -> Result<u64, GrokAccountWorkerError> {
    let failure_column = job.kind.failure_column();
    let sql = format!(
        "SELECT {failure_column} FROM grok_accounts \
         WHERE id = ?1 AND revision = ?2 AND worker_claim_kind = ?3 \
               AND worker_claim_id = ?4 AND worker_claim_expires_at_ms > ?5"
    );
    let count = transaction
        .query_row(
            &sql,
            params![
                job.account_id,
                i64::try_from(job.expected_revision)
                    .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?,
                job.kind.as_str(),
                job.claim_id,
                observed_at_ms,
            ],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?
        .ok_or(GrokAccountWorkerError::StaleClaim)?;
    u64::try_from(count).map_err(|_| GrokAccountWorkerError::InvalidPersistedState)
}

fn insert_quota_window(
    transaction: &Transaction<'_>,
    account_id: &str,
    window: &GrokAccountQuotaWindow,
    observed_at_ms: i64,
) -> Result<(), GrokAccountWorkerError> {
    let (scope_kind, model_label) = match &window.scope {
        GrokAccountQuotaScope::Account => ("account", ""),
        GrokAccountQuotaScope::Model(model) => ("model", model.as_str()),
    };
    let limit = window
        .limit
        .map(i64::try_from)
        .transpose()
        .map_err(|_| GrokAccountWorkerError::InvalidRequest)?;
    let remaining = window
        .remaining
        .map(i64::try_from)
        .transpose()
        .map_err(|_| GrokAccountWorkerError::InvalidRequest)?;
    transaction
        .execute(
            "INSERT INTO grok_account_quota_windows (\
                account_id, scope_kind, model_label, window_label, quota_limit, quota_remaining, \
                reset_at_ms, source, confidence, observed_at_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                account_id,
                scope_kind,
                model_label,
                window.label,
                limit,
                remaining,
                window.reset_at_ms,
                window.source.as_str(),
                window.confidence.as_str(),
                observed_at_ms,
            ],
        )
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
    Ok(())
}

fn validate_quota_windows(
    windows: &[GrokAccountQuotaWindow],
) -> Result<(), GrokAccountWorkerError> {
    let mut groups: BTreeMap<
        GrokAccountQuotaScope,
        (
            GrokAccountQuotaSource,
            GrokAccountQuotaConfidence,
            BTreeSet<String>,
        ),
    > = BTreeMap::new();
    for window in windows {
        let group = groups
            .entry(window.scope.clone())
            .or_insert_with(|| (window.source, window.confidence, BTreeSet::new()));
        if group.0 != window.source
            || group.1 != window.confidence
            || !group.2.insert(window.label.clone())
            || group.2.len() > MAX_QUOTA_WINDOWS_PER_TARGET
        {
            return Err(GrokAccountWorkerError::InvalidRequest);
        }
    }
    Ok(())
}

/// Derives the deterministic five-to-eight-minute pre-expiry refresh instant for one account.
///
/// # Errors
///
/// Returns [`GrokAccountWorkerError::InvalidRequest`] for an empty account or expiry that cannot
/// represent a non-negative due instant.
pub fn deterministic_refresh_due_at(
    account_id: &str,
    expires_at_ms: i64,
) -> Result<i64, GrokAccountWorkerError> {
    if account_id.is_empty() || expires_at_ms < 0 {
        return Err(GrokAccountWorkerError::InvalidRequest);
    }
    let jitter = i64::try_from(deterministic_u64(account_id.as_bytes()) % REFRESH_JITTER_RANGE_MS)
        .map_err(|_| GrokAccountWorkerError::InvalidRequest)?;
    Ok(expires_at_ms
        .saturating_sub(MIN_REFRESH_JITTER_MS + jitter)
        .max(0))
}

fn deterministic_backoff_due_at(
    account_id: &str,
    failure_count: u64,
    observed_at_ms: i64,
) -> Result<i64, GrokAccountWorkerError> {
    let shift = u32::try_from(failure_count.saturating_sub(1).min(16))
        .map_err(|_| GrokAccountWorkerError::InvalidRequest)?;
    let backoff = BASE_FAILURE_BACKOFF_MS
        .checked_mul(1_i64 << shift)
        .unwrap_or(MAX_FAILURE_BACKOFF_MS)
        .min(MAX_FAILURE_BACKOFF_MS);
    let mut material = account_id.as_bytes().to_vec();
    material.extend_from_slice(&failure_count.to_be_bytes());
    let jitter = i64::try_from(deterministic_u64(&material) % FAILURE_JITTER_RANGE_MS)
        .map_err(|_| GrokAccountWorkerError::InvalidRequest)?;
    observed_at_ms
        .checked_add(backoff)
        .and_then(|value| value.checked_add(jitter))
        .ok_or(GrokAccountWorkerError::InvalidRequest)
}

fn deterministic_u64(material: &[u8]) -> u64 {
    let digest = Sha256::digest(material);
    u64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ])
}

fn random_claim_id() -> Result<String, GrokAccountWorkerError> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
    let mut id = String::with_capacity(38);
    id.push_str("grok-worker-");
    for byte in random {
        use std::fmt::Write as _;
        write!(&mut id, "{byte:02x}").map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
    }
    Ok(id)
}

impl From<GrokAccountPoolError> for GrokAccountWorkerError {
    fn from(error: GrokAccountPoolError) -> Self {
        match error {
            GrokAccountPoolError::SecretStoreFailure => Self::SecretStoreFailure,
            GrokAccountPoolError::InvalidPersistedState => Self::InvalidPersistedState,
            GrokAccountPoolError::StoreUnavailable => Self::StoreUnavailable,
            _ => Self::InvalidRequest,
        }
    }
}
