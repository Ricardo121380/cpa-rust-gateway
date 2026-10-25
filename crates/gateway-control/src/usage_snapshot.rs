//! Unified reporting over immutable, bounded facts from the existing event log and ledger.
use std::collections::{BTreeMap, BTreeSet};

use gateway_core::{
    AttemptEvent, AttemptOutcome, AttemptRetryDecision, GatewayEvent, InputTokenAccounting,
    RequestEvent, RequestOutcome, UsageProvenance, UsageSummary,
};
use gateway_store::{
    billing_ledger::{BillingCostConfidence, BillingLedgerEntry},
    event_store::{
        SqliteEventStore, StoredGatewayEvent, UsageEventQuery, usage_snapshot::UsageSnapshotSource,
    },
};
use serde::{Deserialize, Serialize};

use crate::billing_materializer::resolve_usage_lineage;

/// Finite creation/read failures; never expose storage errors or raw source content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    /// Malformed or unbounded filters.
    InvalidQuery,
    /// The selected source/result exceeds the supported finite capacity.
    LimitExceeded,
    /// Storage, lineage or arithmetic could not be verified.
    SourceUnavailable,
}

/// Exact, immutable filters shared by summary, details, buckets and exports.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(missing_docs)]
pub struct UsageSnapshotQuery {
    pub from_ms: i64,
    pub to_ms: i64,
    pub bucket_ms: i64,
    pub provider_id: Option<String>,
    pub channel_id: Option<String>,
    pub account_id: Option<String>,
    pub model: Option<String>,
    pub client_key_id: Option<String>,
    pub access_group_id: Option<String>,
    pub protocol: Option<String>,
}
impl UsageSnapshotQuery {
    /// Enforces explicit UTC bounds, at most 366 days and at most 1,000 buckets.
    /// # Errors
    /// Returns `InvalidQuery` for malformed ranges or identities.
    pub fn validate(&self) -> Result<(), SnapshotError> {
        if self.from_ms < 0
            || self.to_ms < self.from_ms
            || self.bucket_ms < 1_000
            || self.to_ms - self.from_ms > 366 * 86_400_000
            || (self.to_ms - self.from_ms) / self.bucket_ms >= 1_000
            || self.to_ms > 9_007_199_254_740_991
            || [
                &self.provider_id,
                &self.channel_id,
                &self.account_id,
                &self.model,
                &self.client_key_id,
                &self.access_group_id,
                &self.protocol,
            ]
            .into_iter()
            .flatten()
            .any(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
            || self.protocol.as_deref().is_some_and(|p| {
                !matches!(
                    p,
                    "openai_chat_completions" | "openai_responses" | "anthropic_messages"
                )
            })
        {
            return Err(SnapshotError::InvalidQuery);
        }
        Ok(())
    }
    fn matches_attempt(&self, a: &AttemptEvent) -> bool {
        (self.from_ms..=self.to_ms).contains(&a.ended_at_ms())
            && self
                .provider_id
                .as_deref()
                .is_none_or(|v| v == a.upstream_id().as_str())
            && self
                .channel_id
                .as_deref()
                .is_none_or(|v| v == a.endpoint_id().as_str())
            && self
                .account_id
                .as_deref()
                .is_none_or(|v| v == a.credential_id().as_str())
    }
}

/// A safe flat detail row: one terminal Attempt, or a Request with no terminal Attempt.
#[derive(Clone, Serialize)]
#[allow(missing_docs)]
pub struct UsageDetail {
    pub row_kind: &'static str,
    pub request_id: String,
    pub attempt_id: Option<String>,
    pub attempt_number: Option<u64>,
    pub occurred_at_ms: i64,
    pub model: String,
    pub protocol: gateway_core::GatewayProtocol,
    pub client_key_id: String,
    pub access_group_id: Option<String>,
    pub provider_id: Option<String>,
    pub channel_id: Option<String>,
    pub account_id: Option<String>,
    pub request_outcome: Option<RequestOutcome>,
    pub attempt_outcome: Option<&'static str>,
    pub usage_observed: bool,
    pub usage_outcome: Option<&'static str>,
    #[serde(flatten)]
    pub usage: UsageSummary,
    pub ledger_id: Option<i64>,
    pub catalog_version_id: Option<String>,
    pub cost_microunits: Option<u64>,
    pub cost_status: &'static str,
}

/// Nullable token subtotal and evidence coverage; zero requires an actual observation.
#[derive(Clone, Default, Serialize)]
#[allow(missing_docs)]
pub struct TokenTotal {
    pub total: Option<u64>,
    pub observed: u64,
    pub missing: u64,
    pub measured: u64,
    pub estimated: u64,
    pub unknown: u64,
    pub confidence: &'static str,
}
impl TokenTotal {
    fn add(
        &mut self,
        value: Option<u64>,
        provenance: UsageProvenance,
    ) -> Result<(), SnapshotError> {
        if let Some(value) = value {
            self.total = Some(
                self.total
                    .unwrap_or(0)
                    .checked_add(value)
                    .ok_or(SnapshotError::SourceUnavailable)?,
            );
            self.observed += 1;
            match provenance {
                UsageProvenance::Measured => self.measured += 1,
                UsageProvenance::Estimated => self.estimated += 1,
                UsageProvenance::Unknown => self.unknown += 1,
            }
        } else {
            self.missing += 1;
        }
        self.confidence = if self.observed == 0 || self.unknown > 0 || self.estimated > 0 {
            "unknown"
        } else if self.missing > 0 {
            "partial"
        } else {
            "exact"
        };
        Ok(())
    }
}

/// Full-filter aggregate. Request counts are distinct and are not additive across time buckets.
#[derive(Clone, Serialize)]
#[allow(missing_docs)]
pub struct UsageTotals {
    pub requests: usize,
    pub attempts: usize,
    pub usage_observations: usize,
    pub failed_usage_observations: usize,
    pub cancelled_usage_observations: usize,
    pub requests_without_terminal: usize,
    pub input_tokens: TokenTotal,
    pub output_tokens: TokenTotal,
    pub reasoning_tokens: TokenTotal,
    pub cache_read_tokens: TokenTotal,
    pub cache_creation_tokens: TokenTotal,
    pub cached_tokens: TokenTotal,
    pub input_accounting: &'static str,
    pub input_tokens_by_accounting: BTreeMap<&'static str, TokenTotal>,
    pub cost_records: BTreeMap<&'static str, usize>,
    pub known_cost_microunits: Option<u64>,
    pub cost_status: &'static str,
}

/// Bounded time bucket, aligned relative to the selected inclusive lower bound.
#[derive(Clone, Serialize)]
#[allow(missing_docs)]
pub struct UsageBucket {
    pub from_ms: i64,
    pub to_ms: i64,
    #[serde(flatten)]
    pub totals: UsageTotals,
}

/// Frozen data product. No subsequent read consults the mutable database.
#[derive(Serialize)]
#[allow(missing_docs)]
pub struct UsageSnapshot {
    pub schema_version: &'static str,
    pub query: UsageSnapshotQuery,
    pub event_ordinal: i64,
    pub ledger_id: i64,
    pub summary: UsageTotals,
    pub buckets: Vec<UsageBucket>,
    pub details: Vec<UsageDetail>,
}

/// Reads and projects one consistent source; never prices Usage or contacts a Provider.
/// # Errors
/// Rejects invalid filters, exceeded bounds, inconsistent lineage or unverifiable ledger rows.
pub fn read_usage_snapshot(
    store: &SqliteEventStore,
    query: UsageSnapshotQuery,
) -> Result<UsageSnapshot, SnapshotError> {
    query.validate()?;
    let source = store
        .usage_snapshot_source(&UsageEventQuery {
            from_ms: Some(query.from_ms),
            to_ms: Some(query.to_ms),
            provider_id: query.provider_id.as_deref(),
            channel_id: query.channel_id.as_deref(),
            account_id: query.account_id.as_deref(),
            public_model: query.model.as_deref(),
            client_key_id: query.client_key_id.as_deref(),
            access_group_id: query.access_group_id.as_deref(),
            protocol: query.protocol.as_deref(),
            ..Default::default()
        })
        .map_err(|_| SnapshotError::SourceUnavailable)?
        .ok_or(SnapshotError::LimitExceeded)?;
    compile(&source, query)
}

fn compile(
    source: &UsageSnapshotSource,
    query: UsageSnapshotQuery,
) -> Result<UsageSnapshot, SnapshotError> {
    let mut details = Vec::new();
    for lineage in source
        .events
        .chunk_by(|a, b| a.request_id() == b.request_id())
    {
        project_request(
            lineage,
            &source.ledger,
            source.billing_checkpoint,
            &query,
            &mut details,
        )?;
    }
    details.sort_by(|a, b| {
        (a.occurred_at_ms, &a.request_id, a.attempt_number).cmp(&(
            b.occurred_at_ms,
            &b.request_id,
            b.attempt_number,
        ))
    });
    let summary = totals(&details)?;
    let mut buckets = Vec::new();
    for rows in details.chunk_by(|a, b| {
        (a.occurred_at_ms - query.from_ms) / query.bucket_ms
            == (b.occurred_at_ms - query.from_ms) / query.bucket_ms
    }) {
        let from_ms = query.from_ms
            + (rows[0].occurred_at_ms - query.from_ms) / query.bucket_ms * query.bucket_ms;
        buckets.push(UsageBucket {
            from_ms,
            to_ms: from_ms.saturating_add(query.bucket_ms - 1).min(query.to_ms),
            totals: totals(rows)?,
        });
    }
    Ok(UsageSnapshot {
        schema_version: "usage-snapshot-v1",
        query,
        event_ordinal: source.event_ordinal,
        ledger_id: source.ledger_id,
        summary,
        buckets,
        details,
    })
}

fn project_request(
    lineage: &[StoredGatewayEvent],
    ledger: &BTreeMap<String, BillingLedgerEntry>,
    billing_checkpoint: i64,
    query: &UsageSnapshotQuery,
    rows: &mut Vec<UsageDetail>,
) -> Result<(), SnapshotError> {
    let request = lineage
        .iter()
        .find_map(|e| {
            if let GatewayEvent::Request(r) = e.event() {
                Some(r)
            } else {
                None
            }
        })
        .ok_or(SnapshotError::SourceUnavailable)?;
    let outcome = lineage.iter().find_map(|e| {
        if let GatewayEvent::RequestFinished(r) = e.event() {
            Some(r.outcome)
        } else {
            None
        }
    });
    let mut usages = BTreeMap::new();
    for stored in lineage {
        if let GatewayEvent::Usage(usage) = stored.event() {
            let (_, attempt) = resolve_usage_lineage(usage, lineage)
                .map_err(|_| SnapshotError::SourceUnavailable)?;
            if usages
                .insert(attempt.attempt_id().as_str().to_owned(), (stored, usage))
                .is_some()
            {
                return Err(SnapshotError::SourceUnavailable);
            }
        }
    }
    let attempts: Vec<_> = lineage
        .iter()
        .filter_map(|e| {
            if let GatewayEvent::Attempt(a) = e.event() {
                Some(a)
            } else {
                None
            }
        })
        .collect();
    let last_attempt = attempts.iter().map(|a| a.attempt_number()).max();
    if attempts.is_empty() {
        let time = request
            .started_at_ms()
            .ok_or(SnapshotError::SourceUnavailable)?;
        rows.push(detail(request, outcome, time));
    }
    for attempt in attempts.into_iter().filter(|a| query.matches_attempt(a)) {
        let mut row = detail(request, outcome, attempt.ended_at_ms());
        row.row_kind = "attempt";
        row.attempt_id = Some(attempt.attempt_id().as_str().to_owned());
        row.attempt_number = Some(attempt.attempt_number());
        row.provider_id = Some(attempt.upstream_id().as_str().to_owned());
        row.channel_id = Some(attempt.endpoint_id().as_str().to_owned());
        row.account_id = Some(attempt.credential_id().as_str().to_owned());
        row.attempt_outcome = Some(
            if attempt.retry_decision() == AttemptRetryDecision::Cancelled {
                "cancelled"
            } else if matches!(attempt.outcome(), AttemptOutcome::Succeeded) {
                "succeeded"
            } else {
                "failed"
            },
        );
        if let Some((stored, usage)) = usages.get(attempt.attempt_id().as_str()) {
            row.usage_observed = true;
            row.usage_outcome = if row.attempt_outcome == Some("succeeded")
                && Some(attempt.attempt_number()) == last_attempt
            {
                outcome.map(|o| match o {
                    RequestOutcome::Succeeded => "succeeded",
                    RequestOutcome::Failed => "failed",
                    RequestOutcome::Cancelled => "cancelled",
                })
            } else {
                row.attempt_outcome
            };

            row.usage = usage.usage().clone();
            row.cost_status = if stored.ordinal() > billing_checkpoint {
                "pending"
            } else {
                "unavailable"
            };
            if let Some(entry) = ledger.get(stored.event_id()) {
                attach_ledger(&mut row, attempt, usage, entry)?;
            }
        }
        rows.push(row);
    }
    Ok(())
}

fn attach_ledger(
    row: &mut UsageDetail,
    attempt: &AttemptEvent,
    usage: &gateway_core::UsageEvent,
    entry: &BillingLedgerEntry,
) -> Result<(), SnapshotError> {
    if entry.request_id != row.request_id
        || entry.response_id != usage.response_id().as_str()
        || entry.account_id != attempt.credential_id().as_str()
        || entry.channel_id != attempt.endpoint_id().as_str()
        || entry.provider_id != attempt.upstream_id().as_str()
        || entry.model != row.model
        || entry.usage != row.usage
        || i64::try_from(entry.occurred_at_ms).ok() != Some(row.occurred_at_ms)
        || (entry.cost_microunits.is_some() && entry.usage.provenance != UsageProvenance::Measured)
    {
        return Err(SnapshotError::SourceUnavailable);
    }
    row.ledger_id = Some(entry.ledger_id);
    row.catalog_version_id.clone_from(&entry.catalog_version_id);
    row.cost_microunits = entry.cost_microunits;
    row.cost_status = match entry.cost_confidence {
        BillingCostConfidence::Exact => "exact",
        BillingCostConfidence::Partial => "partial",
        BillingCostConfidence::Unknown => "unknown",
        BillingCostConfidence::Unpriced => "unpriced",
    };
    Ok(())
}

fn detail(request: &RequestEvent, outcome: Option<RequestOutcome>, time: i64) -> UsageDetail {
    UsageDetail {
        row_kind: "request_without_attempt",
        request_id: request.request_id().as_str().to_owned(),
        attempt_id: None,
        attempt_number: None,
        occurred_at_ms: time,
        model: request.public_model().to_owned(),
        protocol: request.protocol(),
        client_key_id: request.client_key_id().as_str().to_owned(),
        access_group_id: request.access_group_id().map(|g| g.as_str().to_owned()),
        provider_id: None,
        channel_id: None,
        account_id: None,
        request_outcome: outcome,
        attempt_outcome: None,
        usage_observed: false,
        usage_outcome: None,
        usage: UsageSummary::from(&gateway_core::Usage::default()),
        ledger_id: None,
        catalog_version_id: None,
        cost_microunits: None,
        cost_status: if outcome.is_none() {
            "pending"
        } else {
            "unknown"
        },
    }
}

fn totals(rows: &[UsageDetail]) -> Result<UsageTotals, SnapshotError> {
    let mut metrics: [TokenTotal; 6] = std::array::from_fn(|_| TokenTotal {
        confidence: "unknown",
        ..Default::default()
    });
    let mut accounting: BTreeMap<&str, TokenTotal> = BTreeMap::new();
    let mut cost_records: BTreeMap<&str, usize> = [
        "exact",
        "partial",
        "unknown",
        "unpriced",
        "pending",
        "unavailable",
    ]
    .into_iter()
    .map(|key| (key, 0))
    .collect();
    let mut cost: Option<u64> = None;
    for row in rows {
        let u = &row.usage;
        for (metric, value) in metrics.iter_mut().zip([
            u.input_tokens,
            u.output_tokens,
            u.reasoning_tokens,
            u.cache_read_tokens,
            u.cache_creation_tokens,
            u.cached_tokens,
        ]) {
            metric.add(value, u.provenance)?;
        }
        let key = match u.input_accounting {
            InputTokenAccounting::Inclusive => "inclusive",
            InputTokenAccounting::Exclusive => "exclusive",
            InputTokenAccounting::Unknown => "unknown",
        };
        accounting
            .entry(key)
            .or_default()
            .add(u.input_tokens, u.provenance)?;
        *cost_records.entry(row.cost_status).or_default() += 1;
        if let Some(amount) = row.cost_microunits {
            cost = Some(
                cost.unwrap_or(0)
                    .checked_add(amount)
                    .ok_or(SnapshotError::SourceUnavailable)?,
            );
        }
    }
    let cost_status = aggregate_cost_status(rows.len(), &cost_records, cost);
    let input_accounting = aggregate_input_accounting(&accounting);
    if matches!(input_accounting, "mixed" | "unknown") {
        metrics[0].total = None;
        metrics[0].confidence = "unknown";
    }
    let [
        input_tokens,
        output_tokens,
        reasoning_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        cached_tokens,
    ] = metrics;
    Ok(UsageTotals {
        requests: rows
            .iter()
            .map(|r| &r.request_id)
            .collect::<BTreeSet<_>>()
            .len(),
        attempts: rows.iter().filter(|r| r.attempt_id.is_some()).count(),
        usage_observations: rows.iter().filter(|r| r.usage_observed).count(),
        failed_usage_observations: rows
            .iter()
            .filter(|r| r.usage_observed && r.usage_outcome == Some("failed"))
            .count(),
        cancelled_usage_observations: rows
            .iter()
            .filter(|r| r.usage_observed && r.usage_outcome == Some("cancelled"))
            .count(),
        requests_without_terminal: rows
            .iter()
            .filter(|r| r.request_outcome.is_none())
            .map(|r| &r.request_id)
            .collect::<BTreeSet<_>>()
            .len(),
        input_tokens,
        output_tokens,
        reasoning_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        cached_tokens,
        input_accounting,
        input_tokens_by_accounting: accounting,
        cost_records,
        known_cost_microunits: cost,
        cost_status,
    })
}

fn aggregate_input_accounting(accounting: &BTreeMap<&str, TokenTotal>) -> &'static str {
    let observed_accounting: Vec<_> = accounting
        .iter()
        .filter(|(_, metric)| metric.observed > 0)
        .map(|(key, _)| *key)
        .collect();
    match observed_accounting.as_slice() {
        ["inclusive"] => "inclusive",
        ["exclusive"] => "exclusive",
        [] | ["unknown"] => "unknown",
        _ => "mixed",
    }
}

fn aggregate_cost_status(
    row_count: usize,
    cost_records: &BTreeMap<&str, usize>,
    cost: Option<u64>,
) -> &'static str {
    if row_count == 0 {
        "unknown"
    } else if cost_records.get("exact") == Some(&row_count) {
        "exact"
    } else if cost.is_some() {
        "partial"
    } else if cost_records.get("unpriced") == Some(&row_count) {
        "unpriced"
    } else if cost_records.get("pending").copied().unwrap_or(0) > 0 {
        "pending"
    } else if cost_records.get("unavailable").copied().unwrap_or(0) > 0 {
        "unavailable"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usage_snapshot_mixed_accounting_is_not_an_exact_total()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = RequestEvent::new(
            gateway_core::RequestId::try_new("mixed")?,
            gateway_core::ClientKeyId::try_new("key")?,
            None,
            gateway_core::GatewayProtocol::OpenAiResponses,
            "m".into(),
            "m".into(),
            None,
            false,
        );
        let mut inclusive = detail(&request, Some(RequestOutcome::Succeeded), 1000);
        inclusive.usage.input_tokens = Some(100);
        inclusive.usage.provenance = UsageProvenance::Measured;
        inclusive.usage.input_accounting = InputTokenAccounting::Inclusive;
        let mut exclusive = inclusive.clone();
        exclusive.usage.input_tokens = Some(60);
        exclusive.usage.input_accounting = InputTokenAccounting::Exclusive;
        let aggregate = totals(&[inclusive.clone(), exclusive]).map_err(|_| "aggregate")?;
        assert_eq!(aggregate.input_accounting, "mixed");
        assert_eq!(aggregate.input_tokens.total, None);
        assert_eq!(aggregate.input_tokens.confidence, "unknown");
        assert_eq!(
            aggregate.input_tokens_by_accounting["inclusive"].total,
            Some(100)
        );
        inclusive.usage.input_accounting = InputTokenAccounting::Unknown;
        assert_eq!(
            totals(&[inclusive.clone()])
                .map_err(|_| "aggregate")?
                .input_tokens
                .total,
            None
        );
        inclusive.usage.output_tokens = Some(u64::MAX);
        assert!(matches!(
            totals(&[inclusive.clone(), inclusive]),
            Err(SnapshotError::SourceUnavailable)
        ));
        Ok(())
    }
}
