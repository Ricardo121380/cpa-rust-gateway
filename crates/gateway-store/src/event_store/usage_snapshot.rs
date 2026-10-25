//! One bounded read transaction over the existing event log and billing ledger.
use std::collections::BTreeMap;

use super::{SqliteEventStore, StoredGatewayEvent, UsageEventQuery, decode_stored_event};
use crate::{StoreError, StoreResult, billing_ledger::BillingLedgerEntry};
use gateway_core::GatewayEvent;

/// Maximum source events retained while building a management snapshot.
pub const MAX_SNAPSHOT_EVENTS: usize = 20_000;
/// Maximum serialized source payload bytes admitted per snapshot.
pub const MAX_SNAPSHOT_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// Private source facts; the control projection must explicitly select exportable fields.
pub struct UsageSnapshotSource {
    /// Event append watermark in this transaction.
    pub event_ordinal: i64,
    /// Ledger append watermark in the same transaction.
    pub ledger_id: i64,
    /// Default billing worker checkpoint at capture time.
    pub billing_checkpoint: i64,
    /// Complete lineage for the selected requests, bounded before decoding.
    pub events: Vec<StoredGatewayEvent>,
    /// Existing ledger entries keyed by their exact source Usage identity.
    pub ledger: BTreeMap<String, BillingLedgerEntry>,
}

impl SqliteEventStore {
    /// Freezes filtered request lineages and their existing ledger rows in one read transaction.
    /// `None` means the finite event/byte limit was exceeded; no partial result is returned.
    ///
    /// # Errors
    /// Returns a source error for malformed records or unavailable storage.
    pub fn usage_snapshot_source(
        &self,
        query: &UsageEventQuery<'_>,
    ) -> StoreResult<Option<UsageSnapshotSource>> {
        let transaction = self.connection.unchecked_transaction()?;
        let (event_ordinal, ledger_id) = transaction.query_row(
            "SELECT COALESCE((SELECT MAX(event_ordinal) FROM gateway_event_log),0),
                    COALESCE((SELECT MAX(ledger_id) FROM billing_ledger_entries),0)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let billing_checkpoint = transaction.query_row(
            "SELECT COALESCE((SELECT event_ordinal FROM billing_materializer_checkpoints WHERE materializer_id='gateway-usage-to-billing-v1'),0)",
            [], |row| row.get(0),
        )?;
        let mut events = Vec::new();
        let mut ledger = BTreeMap::new();
        let mut bytes = 0usize;
        {
            // Select whole request lineages, then apply the exact Attempt predicate in control.
            // This preserves retry ownership even if other Attempts lie outside the filter.
            let mut statement = transaction.prepare(
                "WITH selected AS (
                    SELECT r.request_id FROM gateway_event_log r
                    WHERE r.event_type='request'
                    AND (?3 IS NULL OR json_extract(r.payload_json,'$.request.public_model')=?3)
                    AND (?4 IS NULL OR json_extract(r.payload_json,'$.request.client_key_id')=?4)
                    AND (?5 IS NULL OR json_extract(r.payload_json,'$.request.access_group_id')=?5)
                    AND (?6 IS NULL OR json_extract(r.payload_json,'$.request.protocol')=?6)
                    AND (EXISTS (SELECT 1 FROM gateway_event_log a WHERE a.request_id=r.request_id
                        AND a.event_type='attempt' AND a.occurred_at_ms>=?1 AND a.occurred_at_ms<=?2
                        AND (?7 IS NULL OR json_extract(a.payload_json,'$.attempt.upstream_id')=?7)
                        AND (?8 IS NULL OR json_extract(a.payload_json,'$.attempt.endpoint_id')=?8)
                        AND (?9 IS NULL OR json_extract(a.payload_json,'$.attempt.credential_id')=?9))
                    OR (?7 IS NULL AND ?8 IS NULL AND ?9 IS NULL
                        AND r.occurred_at_ms>=?1 AND r.occurred_at_ms<=?2
                        AND NOT EXISTS (SELECT 1 FROM gateway_event_log a
                            WHERE a.request_id=r.request_id AND a.event_type='attempt')))
                ) SELECT e.event_ordinal,e.event_type,e.event_id,e.request_id,e.occurred_at_ms,e.payload_json
                FROM gateway_event_log e JOIN selected s ON s.request_id=e.request_id
                ORDER BY e.request_id,e.event_ordinal LIMIT ?10",
            )?;
            let mut rows = statement.query(rusqlite::params![
                query.from_ms,
                query.to_ms,
                query.public_model,
                query.client_key_id,
                query.access_group_id,
                query.protocol,
                query.provider_id,
                query.channel_id,
                query.account_id,
                i64::try_from(MAX_SNAPSHOT_EVENTS + 1)
                    .map_err(|_| StoreError::InvalidPersistedGatewayEvent)?,
            ])?;
            while let Some(row) = rows.next()? {
                let payload: String = row.get(5)?;
                bytes = bytes.saturating_add(payload.len());
                if events.len() == MAX_SNAPSHOT_EVENTS || bytes > MAX_SNAPSHOT_SOURCE_BYTES {
                    return Ok(None);
                }
                let event = decode_stored_event(
                    row.get(0)?,
                    &row.get::<_, String>(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    &payload,
                )?;
                if matches!(event.event(), GatewayEvent::Usage(_))
                    && let Some(entry) =
                        crate::billing_ledger::load_source_entry(&transaction, event.event_id())?
                {
                    ledger.insert(event.event_id().to_owned(), entry);
                }
                events.push(event);
            }
        }
        transaction.commit()?;
        Ok(Some(UsageSnapshotSource {
            event_ordinal,
            ledger_id,
            billing_checkpoint,
            events,
            ledger,
        }))
    }
}
