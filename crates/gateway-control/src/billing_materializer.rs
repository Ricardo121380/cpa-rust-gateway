//! Durable Usage-to-billing materialization for P13-05B.
//!
//! The materializer consumes only gateway-owned, secret-free lifecycle events.  It resolves the
//! complete Request/Attempt/Usage lineage before writing a ledger row, and advances its
//! checkpoint only after the whole bounded batch has been accepted.  Provider calls, request
//! bodies, credentials and endpoint URLs are outside this module by construction.

use std::{collections::BTreeMap, error::Error, fmt};

use gateway_core::{AttemptEvent, AttemptOutcome, GatewayEvent, RequestEvent, UsageEvent};
use gateway_store::{
    StoreError,
    billing_ledger::{
        BillingCostConfidence, BillingLedgerEntryInput, BillingPriceCatalog, BillingRecordResult,
        SqliteBillingLedger,
    },
    event_store::{SqliteEventStore, StoredGatewayEvent},
};

use crate::billing_service::{BillingPricingError, find_effective_price_catalog, quote_usage};

/// Stable materializer identity used by the default billing projector.
pub const BILLING_MATERIALIZER_ID: &str = "gateway-usage-to-billing-v1";
/// Maximum event rows admitted to one materializer invocation.
pub const MAX_BILLING_MATERIALIZER_EVENTS: usize = 1_024;
/// Maximum immutable catalogs loaded for one materializer invocation.
pub const MAX_BILLING_CATALOGS: usize = 256;

/// Safe outcome of one bounded billing materializer invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingMaterializationReceipt {
    /// Number of event rows scanned after the previous checkpoint.
    pub scanned_events: usize,
    /// Number of new ledger rows inserted.
    pub inserted_rows: usize,
    /// Number of source events replayed idempotently.
    pub replayed_rows: usize,
    /// New high-water event ordinal, if the source returned any rows.
    pub checkpoint_ordinal: Option<i64>,
}

/// Failure at the bounded materialization boundary.
#[derive(Debug)]
pub enum BillingMaterializationError {
    /// The durable source or ledger rejected the operation.
    Store(StoreError),
    /// A complete Request/Attempt/Usage lineage was not available.
    InvalidLineage,
    /// A safe timestamp or retention window could not be represented.
    InvalidTimestamp,
    /// Fixed-point pricing overflowed.
    PricingOverflow,
    /// The source batch exceeded the frozen finite bound.
    BatchTooLarge,
}

impl fmt::Display for BillingMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(_) => formatter.write_str("billing materializer store unavailable"),
            Self::InvalidLineage => formatter.write_str("billing event lineage is incomplete"),
            Self::InvalidTimestamp => formatter.write_str("billing event timestamp is invalid"),
            Self::PricingOverflow => formatter.write_str("billing quote overflowed"),
            Self::BatchTooLarge => formatter.write_str("billing materializer batch is too large"),
        }
    }
}

impl Error for BillingMaterializationError {}

impl From<StoreError> for BillingMaterializationError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<BillingPricingError> for BillingMaterializationError {
    fn from(_: BillingPricingError) -> Self {
        Self::PricingOverflow
    }
}

/// Materializes all new Usage events after the persisted checkpoint.
///
/// Request and Attempt rows are reloaded by request id for every new Usage row.  This makes the
/// checkpoint safe even when the Request/Attempt happened in a prior event batch.  The ledger's
/// source-event uniqueness makes retries safe if a process exits after a ledger insert but before
/// the checkpoint write.
///
/// # Errors
///
/// Returns [`BillingMaterializationError`] when the bounded source, lineage, pricing arithmetic,
/// ledger write, or checkpoint write fails.
pub fn materialize_billing_events(
    event_store: &SqliteEventStore,
    billing_store: &mut SqliteBillingLedger,
    materializer_id: &str,
    max_events: usize,
    retention_ms: u64,
    now_ms: u64,
) -> Result<BillingMaterializationReceipt, BillingMaterializationError> {
    if max_events == 0 || max_events > MAX_BILLING_MATERIALIZER_EVENTS {
        return Err(BillingMaterializationError::BatchTooLarge);
    }
    let checkpoint = billing_store
        .load_checkpoint(materializer_id)?
        .map_or(0, |value| value.event_ordinal);
    let events = event_store.list_events_after_ordinal_bounded(checkpoint, max_events)?;
    let catalogs = billing_store.list_catalogs_bounded(MAX_BILLING_CATALOGS + 1)?;
    if catalogs.len() > MAX_BILLING_CATALOGS {
        return Err(BillingMaterializationError::BatchTooLarge);
    }

    let mut inserted_rows = 0;
    let mut replayed_rows = 0;
    for stored in &events {
        let GatewayEvent::Usage(usage) = stored.event() else {
            continue;
        };
        let lineage = event_store.events_for_request(usage.request_id())?;
        let input = compile_usage_entry(stored, usage, &lineage, &catalogs, retention_ms, now_ms)?;
        match billing_store.record(&input)? {
            BillingRecordResult::Inserted(_) => inserted_rows += 1,
            BillingRecordResult::Replay(_) => replayed_rows += 1,
        }
    }

    let checkpoint_ordinal = events.last().map(StoredGatewayEvent::ordinal);
    if let Some(ordinal) = checkpoint_ordinal {
        billing_store.save_checkpoint(materializer_id, ordinal, now_ms)?;
    }
    Ok(BillingMaterializationReceipt {
        scanned_events: events.len(),
        inserted_rows,
        replayed_rows,
        checkpoint_ordinal,
    })
}

fn compile_usage_entry(
    stored: &StoredGatewayEvent,
    usage: &UsageEvent,
    lineage: &[StoredGatewayEvent],
    catalogs: &[BillingPriceCatalog],
    retention_ms: u64,
    now_ms: u64,
) -> Result<BillingLedgerEntryInput, BillingMaterializationError> {
    let mut request: Option<RequestEvent> = None;
    let mut attempts = BTreeMap::<u64, AttemptEvent>::new();
    let mut matching_usage = false;
    for event in lineage {
        match event.event() {
            GatewayEvent::Request(value) => {
                if let Some(existing) = &request {
                    if existing != value {
                        return Err(BillingMaterializationError::InvalidLineage);
                    }
                } else {
                    request = Some(value.clone());
                }
            }
            GatewayEvent::Attempt(value) => {
                if value.attempt_number() == 0 || value.ended_at_ms() < 0 {
                    return Err(BillingMaterializationError::InvalidLineage);
                }
                if let Some(existing) = attempts.get(&value.attempt_number()) {
                    if existing != value {
                        return Err(BillingMaterializationError::InvalidLineage);
                    }
                } else {
                    attempts.insert(value.attempt_number(), value.clone());
                }
            }
            GatewayEvent::Usage(value) if value == usage => matching_usage = true,
            GatewayEvent::Usage(_) | GatewayEvent::Health(_) | GatewayEvent::Diagnostic(_) => {}
        }
    }
    if !matching_usage {
        return Err(BillingMaterializationError::InvalidLineage);
    }
    let request = request.ok_or(BillingMaterializationError::InvalidLineage)?;
    let attempt = attempts
        .into_iter()
        .next_back()
        .map(|(_, value)| value)
        .ok_or(BillingMaterializationError::InvalidLineage)?;
    if !matches!(attempt.outcome(), AttemptOutcome::Succeeded)
        || attempt.request_id() != request.request_id()
        || usage.request_id() != request.request_id()
    {
        return Err(BillingMaterializationError::InvalidLineage);
    }
    let occurred_at_ms = u64::try_from(attempt.ended_at_ms())
        .map_err(|_| BillingMaterializationError::InvalidTimestamp)?;
    let retention_expires_at_ms = occurred_at_ms
        .checked_add(retention_ms)
        .ok_or(BillingMaterializationError::InvalidTimestamp)?;
    let catalog = find_effective_price_catalog(
        catalogs,
        attempt.upstream_id().as_str(),
        attempt.endpoint_id().as_str(),
        request.public_model(),
        occurred_at_ms,
    );
    let (catalog_version_id, cost_microunits, cost_confidence) = match catalog {
        Some(catalog) => {
            let quote = quote_usage(
                catalog,
                attempt.upstream_id().as_str(),
                attempt.endpoint_id().as_str(),
                request.public_model(),
                usage.usage(),
            )?;
            (
                Some(quote.catalog_version_id),
                quote.cost_microunits,
                quote.confidence,
            )
        }
        None => (None, None, BillingCostConfidence::Unpriced),
    };
    Ok(BillingLedgerEntryInput {
        source_event_id: stored.event_id().to_owned(),
        request_id: request.request_id().as_str().to_owned(),
        response_id: usage.response_id().as_str().to_owned(),
        provider_id: attempt.upstream_id().as_str().to_owned(),
        channel_id: attempt.endpoint_id().as_str().to_owned(),
        account_id: attempt.credential_id().as_str().to_owned(),
        model: request.public_model().to_owned(),
        occurred_at_ms,
        catalog_version_id,
        usage: usage.usage().clone(),
        cost_microunits,
        cost_confidence,
        retention_expires_at_ms,
        recorded_at_ms: now_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_core::{
        AccessGroupId, AttemptEvent, ClientKeyId, CredentialId, EndpointId, GatewayProtocol,
        RequestId, ResponseId, RouteCandidateId, RouteId, UpstreamId, Usage, UsageEvent,
    };

    fn events() -> Result<(SqliteEventStore, RequestId), Box<dyn Error>> {
        let mut store = SqliteEventStore::open_in_memory()?;
        let request_id = RequestId::try_new("request-1")?;
        let request = gateway_core::RequestEvent::new(
            request_id.clone(),
            ClientKeyId::try_new("client-1")?,
            Some(AccessGroupId::try_new("group-1")?),
            GatewayProtocol::OpenAiResponses,
            "model-a".into(),
            "model-a".into(),
            None,
            false,
        );
        let attempt = AttemptEvent::new(
            request_id.clone(),
            1,
            RouteId::try_new("route-1")?,
            RouteCandidateId::try_new("candidate-1")?,
            CredentialId::try_new("account-1")?,
            EndpointId::try_new("channel-1")?,
            UpstreamId::try_new("provider-1")?,
            "upstream-model".into(),
            900,
            1_000,
            AttemptOutcome::Succeeded,
            gateway_core::AttemptRetryDecision::Completed,
        );
        let usage = UsageEvent::from_usage(
            request_id.clone(),
            ResponseId::try_new("response-1")?,
            &Usage {
                input_tokens: Some(1_000_000),
                output_tokens: Some(500_000),
                ..Usage::default()
            },
        );
        store.append_batch(&[
            GatewayEvent::Request(request),
            GatewayEvent::Attempt(attempt),
            GatewayEvent::Usage(usage),
        ])?;
        Ok((store, request_id))
    }

    fn catalog() -> BillingPriceCatalog {
        BillingPriceCatalog {
            catalog_version_id: "catalog-1".into(),
            effective_at_ms: 100,
            source: gateway_store::billing_ledger::BillingCatalogSource::Test,
            created_at_ms: 100,
            entries: vec![gateway_store::billing_ledger::BillingPriceEntry {
                provider_id: "provider-1".into(),
                channel_id: "channel-1".into(),
                model: "model-a".into(),
                input_microunits_per_million: 2_000_000,
                output_microunits_per_million: 4_000_000,
                reasoning_microunits_per_million: 0,
                cache_read_microunits_per_million: 0,
                cache_creation_microunits_per_million: 0,
                cached_microunits_per_million: 0,
            }],
        }
    }

    #[test]
    fn materialization_is_checkpointed_and_idempotent() -> Result<(), Box<dyn Error>> {
        let (events, _request_id) = events()?;
        let mut ledger = SqliteBillingLedger::open_in_memory()?;
        ledger.insert_catalog(&catalog())?;
        let first = materialize_billing_events(
            &events,
            &mut ledger,
            BILLING_MATERIALIZER_ID,
            16,
            10_000,
            2_000,
        )?;
        assert_eq!(first.inserted_rows, 1);
        assert_eq!(ledger.list_bounded(10)?.len(), 1);
        let second = materialize_billing_events(
            &events,
            &mut ledger,
            BILLING_MATERIALIZER_ID,
            16,
            10_000,
            3_000,
        )?;
        assert_eq!(second.scanned_events, 0);
        assert_eq!(second.inserted_rows, 0);
        assert_eq!(ledger.list_bounded(10)?.len(), 1);
        Ok(())
    }

    #[test]
    fn missing_catalog_is_explicitly_unpriced() -> Result<(), Box<dyn Error>> {
        let (events, _request_id) = events()?;
        let mut ledger = SqliteBillingLedger::open_in_memory()?;
        materialize_billing_events(
            &events,
            &mut ledger,
            BILLING_MATERIALIZER_ID,
            16,
            10_000,
            2_000,
        )?;
        let row = ledger.list_bounded(1)?.pop().ok_or("ledger row missing")?;
        assert_eq!(row.cost_microunits, None);
        assert_eq!(row.cost_confidence, BillingCostConfidence::Unpriced);
        Ok(())
    }
}
