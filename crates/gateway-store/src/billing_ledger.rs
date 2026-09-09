//! Durable versioned price catalogs and idempotent billing records.
//!
//! P13-05A deliberately keeps billing separate from the request hot path.  The caller supplies a
//! value-free Usage identity and a precomputed pricing decision; this store only validates,
//! persists and replays that decision.  Prices are integer micro-units per million tokens, so no
//! floating point or locale-dependent decimal conversion can change a bill after a restart.

use std::{path::Path, time::Duration};

use gateway_core::UsageSummary;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};

use crate::{StoreError, StoreResult, migrate, open, open_in_memory};

const MAX_ID_BYTES: usize = 512;
const MAX_SHORT_ID_BYTES: usize = 128;
const MAX_MODEL_BYTES: usize = 512;

/// Source of one immutable price catalog version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BillingCatalogSource {
    /// An operator entered the catalog through a protected management boundary.
    Operator,
    /// A catalog was imported from a reviewed external artifact.
    Imported,
    /// A deterministic fixture used only by tests or local development.
    Test,
}

impl BillingCatalogSource {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Imported => "imported",
            Self::Test => "test",
        }
    }

    fn from_sql(value: &str) -> StoreResult<Self> {
        match value {
            "operator" => Ok(Self::Operator),
            "imported" => Ok(Self::Imported),
            "test" => Ok(Self::Test),
            _ => Err(StoreError::InvalidPersistedBillingRecord),
        }
    }
}

/// One model price in an immutable catalog version.
///
/// Every rate is an integer number of micro-units per one million tokens.  For example, a value
/// of `1_500_000` means 1.5 billing units per million tokens when the ledger unit is a micro-unit.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingPriceEntry {
    pub provider_id: String,
    pub channel_id: String,
    pub model: String,
    pub input_microunits_per_million: u64,
    pub output_microunits_per_million: u64,
    pub reasoning_microunits_per_million: u64,
    pub cache_read_microunits_per_million: u64,
    pub cache_creation_microunits_per_million: u64,
    pub cached_microunits_per_million: u64,
}

/// One immutable, versioned price catalog.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingPriceCatalog {
    pub catalog_version_id: String,
    pub effective_at_ms: u64,
    pub source: BillingCatalogSource,
    pub created_at_ms: u64,
    pub entries: Vec<BillingPriceEntry>,
}

const LEDGER_ROW_SELECT: &str = "SELECT ledger_id, source_event_id, source_fingerprint, request_id, response_id, \
         provider_id, channel_id, account_id, model, occurred_at_ms, catalog_version_id, \
         input_tokens, output_tokens, reasoning_tokens, cache_read_tokens, cache_creation_tokens, \
         cached_tokens, cost_microunits, cost_confidence, retention_expires_at_ms, recorded_at_ms \
         FROM billing_ledger_entries";

/// Durable high-water mark for one billing materializer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingMaterializerCheckpoint {
    /// Stable application-owned materializer identity.
    pub materializer_id: String,
    /// Last gateway event ordinal processed successfully.
    pub event_ordinal: i64,
    /// Wall-clock time of the checkpoint write.
    pub updated_at_ms: u64,
}

/// Consistent, value-free source/checkpoint/failure watermarks for billing processing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingMaterializationProgress {
    /// Highest durable source ordinal; zero means the source is empty.
    pub source_ordinal: i64,
    /// None means no completed or durably quarantined source batch has committed a checkpoint.
    pub checkpoint_ordinal: Option<i64>,
    /// Time the checkpoint was last advanced or replayed.
    pub checkpoint_updated_at_ms: Option<u64>,
    /// Number of failures awaiting repair, including failures behind the checkpoint.
    pub unresolved_failures: u64,
}

/// Value-free durable retry evidence for a materialization failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingMaterializerFailure {
    /// Source event ordinal, retained even when source decoding fails.
    pub event_ordinal: i64,
    /// Closed failure code, never an exception message or event contents.
    pub reason: String,
    /// Initial failure observation.
    pub first_seen_at_ms: u64,
    /// Latest retry observation.
    pub last_attempt_at_ms: u64,
    /// Number of failed attempts.
    pub attempts: u64,
}

/// Cost confidence recorded for a billing row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BillingCostConfidence {
    /// All priced token dimensions were present and the catalog matched.
    Exact,
    /// At least one token dimension was absent; the stored amount is a lower-bound partial cost.
    Partial,
    /// Token usage was not sufficient to calculate a cost.
    Unknown,
    /// No matching price catalog entry was available.
    Unpriced,
}

impl BillingCostConfidence {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
            Self::Unpriced => "unpriced",
        }
    }

    const fn billing_status(self) -> &'static str {
        match self {
            Self::Exact => "priced",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
            Self::Unpriced => "unpriced",
        }
    }

    fn from_sql(value: &str) -> StoreResult<Self> {
        match value {
            "exact" => Ok(Self::Exact),
            "partial" => Ok(Self::Partial),
            "unknown" => Ok(Self::Unknown),
            "unpriced" => Ok(Self::Unpriced),
            _ => Err(StoreError::InvalidPersistedBillingRecord),
        }
    }
}

/// One value-free billing decision ready for durable insertion.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingLedgerEntryInput {
    pub source_event_id: String,
    pub request_id: String,
    pub response_id: String,
    pub provider_id: String,
    pub channel_id: String,
    pub account_id: String,
    pub model: String,
    pub occurred_at_ms: u64,
    pub catalog_version_id: Option<String>,
    pub usage: UsageSummary,
    pub cost_microunits: Option<u64>,
    pub cost_confidence: BillingCostConfidence,
    pub retention_expires_at_ms: u64,
    pub recorded_at_ms: u64,
}

impl BillingLedgerEntryInput {
    /// Computes a deterministic, value-free fingerprint for idempotent replay checks.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(self.source_event_id.as_bytes());
        digest.update([0]);
        digest.update(self.request_id.as_bytes());
        digest.update([0]);
        digest.update(self.response_id.as_bytes());
        digest.update([0]);
        digest.update(self.provider_id.as_bytes());
        digest.update([0]);
        digest.update(self.channel_id.as_bytes());
        digest.update([0]);
        digest.update(self.account_id.as_bytes());
        digest.update([0]);
        digest.update(self.model.as_bytes());
        digest.update([0]);
        digest.update(self.occurred_at_ms.to_be_bytes());
        digest.update([0]);
        if let Some(catalog_version_id) = &self.catalog_version_id {
            digest.update([1]);
            digest.update(catalog_version_id.as_bytes());
        } else {
            digest.update([0]);
        }
        digest.update(self.retention_expires_at_ms.to_be_bytes());
        for token in [
            self.usage.input_tokens,
            self.usage.output_tokens,
            self.usage.reasoning_tokens,
            self.usage.cache_read_tokens,
            self.usage.cache_creation_tokens,
            self.usage.cached_tokens,
        ] {
            digest.update(token.map_or(u64::MAX, |value| value).to_be_bytes());
        }
        digest.update(
            self.cost_microunits
                .map_or(u64::MAX, |value| value)
                .to_be_bytes(),
        );
        digest.update(self.cost_confidence.as_sql().as_bytes());
        let digest = digest.finalize();
        format!("{digest:x}")
    }
}

/// Durable billing row returned by the store.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingLedgerEntry {
    pub ledger_id: i64,
    pub source_event_id: String,
    pub source_fingerprint: String,
    pub request_id: String,
    pub response_id: String,
    pub provider_id: String,
    pub channel_id: String,
    pub account_id: String,
    pub model: String,
    pub occurred_at_ms: u64,
    pub catalog_version_id: Option<String>,
    pub usage: UsageSummary,
    pub cost_microunits: Option<u64>,
    pub cost_confidence: BillingCostConfidence,
    pub retention_expires_at_ms: u64,
    pub recorded_at_ms: u64,
}

/// Result of recording one billing source event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BillingRecordResult {
    /// A new immutable ledger row was inserted.
    Inserted(BillingLedgerEntry),
    /// An identical source event was already present; no row was added.
    Replay(BillingLedgerEntry),
}

/// Versioned catalog plus durable ledger repository.
pub struct SqliteBillingLedger {
    connection: Connection,
}

impl SqliteBillingLedger {
    /// Opens and migrates a file-backed billing store.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot open or migrate.
    pub fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        Self::from_connection(open(path)?)
    }

    /// Opens an already-migrated billing ledger without migration or journal-mode writes.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when `SQLite` cannot open the path read-only or configure its
    /// bounded busy timeout.
    pub fn open_read_only(path: impl AsRef<Path>) -> StoreResult<Self> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        Ok(Self { connection })
    }

    /// Opens and migrates an isolated in-memory billing store.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the in-memory database cannot migrate.
    pub fn open_in_memory() -> StoreResult<Self> {
        Self::from_connection(open_in_memory()?)
    }

    /// Takes an existing connection and applies the current migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the migration history is unsupported or `SQLite` rejects setup.
    pub fn from_connection(mut connection: Connection) -> StoreResult<Self> {
        migrate(&mut connection)?;
        Ok(Self { connection })
    }

    /// Inserts one immutable price catalog. Replaying the exact version is idempotent; changing
    /// metadata or entries for an existing version fails closed.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for malformed catalog values, a conflicting version, or a `SQLite`
    /// transaction failure.
    pub fn insert_catalog(&mut self, catalog: &BillingPriceCatalog) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_catalog_in_transaction(&transaction, catalog)?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads one immutable catalog version, if present.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a persisted catalog row is malformed or `SQLite` rejects the
    /// read.
    pub fn catalog(&self, catalog_version_id: &str) -> StoreResult<Option<BillingPriceCatalog>> {
        load_catalog_from_connection(&self.connection, catalog_version_id)
    }

    /// Returns all immutable price catalogs in deterministic effective-time order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the bound is invalid or a persisted catalog is malformed.
    pub fn list_catalogs_bounded(&self, limit: usize) -> StoreResult<Vec<BillingPriceCatalog>> {
        list_catalogs_bounded_from_connection(&self.connection, limit)
    }

    /// Reads billing progress in one `SQLite` statement/snapshot without loading event payloads.
    ///
    /// # Errors
    /// Returns an error for invalid identity, inconsistent watermarks or unavailable storage.
    pub fn materialization_progress(
        &self,
        materializer_id: &str,
    ) -> StoreResult<BillingMaterializationProgress> {
        validate_short_id(materializer_id)?;
        let (source, checkpoint, updated, failures): (i64, Option<i64>, Option<i64>, i64) = self.connection.query_row(
            "SELECT COALESCE((SELECT MAX(event_ordinal) FROM gateway_event_log), 0),              (SELECT event_ordinal FROM billing_materializer_checkpoints WHERE materializer_id = ?1),              (SELECT updated_at_ms FROM billing_materializer_checkpoints WHERE materializer_id = ?1),              (SELECT COUNT(*) FROM billing_materializer_failures WHERE materializer_id = ?1 AND resolved_at_ms IS NULL)",
            [materializer_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?;
        if source < 0
            || checkpoint.is_some_and(|value| value < 0 || value > source)
            || checkpoint.is_some() != updated.is_some()
        {
            return Err(StoreError::InvalidPersistedBillingRecord);
        }
        Ok(BillingMaterializationProgress {
            source_ordinal: source,
            checkpoint_ordinal: checkpoint,
            checkpoint_updated_at_ms: updated.map(u64_from_i64).transpose()?,
            unresolved_failures: u64_from_i64(failures)?,
        })
    }

    /// Loads one materializer checkpoint, if it has run before.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the identifier is invalid or the persisted checkpoint is malformed.
    pub fn load_checkpoint(
        &self,
        materializer_id: &str,
    ) -> StoreResult<Option<BillingMaterializerCheckpoint>> {
        if materializer_id.trim().is_empty() || materializer_id.len() > MAX_SHORT_ID_BYTES {
            return Err(StoreError::InvalidPersistedBillingRecord);
        }
        let row = self
            .connection
            .query_row(
                "SELECT materializer_id, event_ordinal, updated_at_ms \
                 FROM billing_materializer_checkpoints WHERE materializer_id = ?1",
                [materializer_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(materializer_id, event_ordinal, updated_at_ms)| {
            if event_ordinal < 0 {
                return Err(StoreError::InvalidPersistedBillingRecord);
            }
            Ok(BillingMaterializerCheckpoint {
                materializer_id,
                event_ordinal,
                updated_at_ms: u64_from_i64(updated_at_ms)?,
            })
        })
        .transpose()
    }

    /// Advances a materializer checkpoint monotonically after a successful batch.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for invalid identifiers/timestamps, a checkpoint regression, or a
    /// `SQLite` failure.
    pub fn save_checkpoint(
        &mut self,
        materializer_id: &str,
        event_ordinal: i64,
        updated_at_ms: u64,
    ) -> StoreResult<()> {
        if materializer_id.trim().is_empty()
            || materializer_id.len() > MAX_SHORT_ID_BYTES
            || event_ordinal < 0
        {
            return Err(StoreError::InvalidPersistedBillingRecord);
        }
        let updated_at_ms = i64_from_u64(updated_at_ms)?;
        let previous: Option<i64> = self
            .connection
            .query_row(
                "SELECT event_ordinal FROM billing_materializer_checkpoints \
                 WHERE materializer_id = ?1",
                [materializer_id],
                |row| row.get(0),
            )
            .optional()?;
        if previous.is_some_and(|value| event_ordinal < value) {
            return Err(StoreError::InvalidPersistedBillingRecord);
        }
        self.connection.execute(
            "INSERT INTO billing_materializer_checkpoints \
             (materializer_id, event_ordinal, updated_at_ms) VALUES (?1, ?2, ?3) \
             ON CONFLICT(materializer_id) DO UPDATE SET event_ordinal = excluded.event_ordinal, \
             updated_at_ms = excluded.updated_at_ms",
            params![materializer_id, event_ordinal, updated_at_ms],
        )?;
        Ok(())
    }

    /// Durably records a failed source ordinal before its checkpoint can advance.
    ///
    /// # Errors
    /// Returns an error for invalid metadata or a failed durable write.
    pub fn record_materialization_failure(
        &mut self,
        materializer_id: &str,
        event_ordinal: i64,
        reason: &str,
        observed_at_ms: u64,
    ) -> StoreResult<()> {
        validate_short_id(materializer_id)?;
        if event_ordinal <= 0
            || !matches!(
                reason,
                "invalid_lineage" | "invalid_timestamp" | "pricing_overflow" | "invalid_event"
            )
        {
            return Err(StoreError::InvalidPersistedBillingRecord);
        }
        self.connection.execute(
            "INSERT INTO billing_materializer_failures              (materializer_id, event_ordinal, reason, first_seen_at_ms, last_attempt_at_ms, attempts)              VALUES (?1, ?2, ?3, ?4, ?4, 1)              ON CONFLICT(materializer_id, event_ordinal) DO UPDATE SET reason = excluded.reason,              last_attempt_at_ms = MAX(last_attempt_at_ms, excluded.last_attempt_at_ms),              attempts = attempts + 1, resolved_at_ms = NULL",
            params![materializer_id, event_ordinal, reason, i64_from_u64(observed_at_ms)?],
        )?;
        Ok(())
    }

    /// Reads up to 1024 unresolved failures due for retry in stable oldest-attempt order.
    ///
    /// # Errors
    /// Returns an error for invalid bounds, malformed rows or unavailable storage.
    pub fn list_materialization_failures_due(
        &self,
        materializer_id: &str,
        attempted_before_ms: u64,
        limit: usize,
    ) -> StoreResult<Vec<BillingMaterializerFailure>> {
        validate_short_id(materializer_id)?;
        if !(1..=1024).contains(&limit) {
            return Err(StoreError::InvalidPersistedBillingRecord);
        }
        let mut statement = self.connection.prepare(
            "SELECT event_ordinal, reason, first_seen_at_ms, last_attempt_at_ms, attempts              FROM billing_materializer_failures WHERE materializer_id = ?1 AND resolved_at_ms IS NULL              AND last_attempt_at_ms <= ?2 ORDER BY last_attempt_at_ms, event_ordinal LIMIT ?3")?;
        let mut rows = statement.query(params![
            materializer_id,
            i64_from_u64(attempted_before_ms)?,
            i64::try_from(limit).map_err(|_| StoreError::InvalidPersistedBillingRecord)?
        ])?;
        let mut failures = Vec::new();
        while let Some(row) = rows.next()? {
            failures.push(BillingMaterializerFailure {
                event_ordinal: row.get(0)?,
                reason: row.get(1)?,
                first_seen_at_ms: u64_from_i64(row.get(2)?)?,
                last_attempt_at_ms: u64_from_i64(row.get(3)?)?,
                attempts: u64_from_i64(row.get(4)?)?,
            });
        }
        Ok(failures)
    }

    /// Retains failure history while removing a successfully repaired ordinal from retry work.
    ///
    /// # Errors
    /// Returns an error for invalid metadata or unavailable storage.
    pub fn resolve_materialization_failure(
        &mut self,
        materializer_id: &str,
        event_ordinal: i64,
        observed_at_ms: u64,
    ) -> StoreResult<()> {
        validate_short_id(materializer_id)?;
        if event_ordinal <= 0 {
            return Err(StoreError::InvalidPersistedBillingRecord);
        }
        self.connection.execute("UPDATE billing_materializer_failures SET resolved_at_ms = MAX(last_attempt_at_ms, ?3)             WHERE materializer_id = ?1 AND event_ordinal = ?2 AND resolved_at_ms IS NULL",
            params![materializer_id, event_ordinal, i64_from_u64(observed_at_ms)?])?;
        Ok(())
    }

    /// Records one value-free billing decision with source-event idempotence.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for malformed values, a conflicting replay, a missing catalog
    /// foreign key, or a `SQLite` transaction failure.
    pub fn record(&mut self, input: &BillingLedgerEntryInput) -> StoreResult<BillingRecordResult> {
        validate_input(input)?;
        let fingerprint = input.fingerprint();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(i64, String)> = transaction
            .query_row(
                "SELECT ledger_id, source_fingerprint FROM billing_ledger_entries \
                 WHERE source_event_id = ?1",
                [input.source_event_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((ledger_id, existing_fingerprint)) = existing {
            if existing_fingerprint != fingerprint {
                return Err(StoreError::ConflictingBillingLedgerReplay);
            }
            let entry = load_entry(&transaction, ledger_id)?;
            transaction.commit()?;
            return Ok(BillingRecordResult::Replay(entry));
        }

        transaction.execute(
            "INSERT INTO billing_ledger_entries \
             (source_event_id, source_fingerprint, request_id, response_id, provider_id, channel_id, \
              account_id, model, occurred_at_ms, catalog_version_id, input_tokens, output_tokens, \
              reasoning_tokens, cache_read_tokens, cache_creation_tokens, cached_tokens, \
              cost_microunits, cost_confidence, billing_status, retention_expires_at_ms, recorded_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                input.source_event_id,
                fingerprint,
                input.request_id,
                input.response_id,
                input.provider_id,
                input.channel_id,
                input.account_id,
                input.model,
                i64_from_u64(input.occurred_at_ms)?,
                input.catalog_version_id,
                optional_i64(input.usage.input_tokens)?,
                optional_i64(input.usage.output_tokens)?,
                optional_i64(input.usage.reasoning_tokens)?,
                optional_i64(input.usage.cache_read_tokens)?,
                optional_i64(input.usage.cache_creation_tokens)?,
                optional_i64(input.usage.cached_tokens)?,
                optional_i64(input.cost_microunits)?,
                input.cost_confidence.as_sql(),
                input.cost_confidence.billing_status(),
                i64_from_u64(input.retention_expires_at_ms)?,
                i64_from_u64(input.recorded_at_ms)?,
            ],
        )?;
        let ledger_id = transaction.last_insert_rowid();
        let entry = load_entry(&transaction, ledger_id)?;
        transaction.commit()?;
        Ok(BillingRecordResult::Inserted(entry))
    }

    /// Returns a bounded, deterministic ledger page in `(occurred_at_ms, ledger_id)` order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the bound cannot be represented or a persisted row is malformed.
    pub fn list_bounded(&self, limit: usize) -> StoreResult<Vec<BillingLedgerEntry>> {
        let limit = i64::try_from(limit).map_err(|_| StoreError::InvalidPersistedBillingRecord)?;
        let mut statement = self.connection.prepare(&format!(
            "{LEDGER_ROW_SELECT} ORDER BY occurred_at_ms, ledger_id LIMIT ?1"
        ))?;
        let mut rows = statement.query([limit])?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            entries.push(decode_ledger_row(row)?);
        }
        Ok(entries)
    }

    /// Deletes at most `limit` rows whose retention window has expired.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the bound cannot be represented or `SQLite` rejects the bounded
    /// delete transaction.
    pub fn purge_expired(&mut self, now_ms: u64, limit: usize) -> StoreResult<usize> {
        let now_ms = i64_from_u64(now_ms)?;
        let limit = i64::try_from(limit).map_err(|_| StoreError::InvalidPersistedBillingRecord)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "DELETE FROM billing_ledger_entries WHERE ledger_id IN (\
                SELECT ledger_id FROM billing_ledger_entries \
                WHERE retention_expires_at_ms <= ?1 ORDER BY retention_expires_at_ms, ledger_id LIMIT ?2)",
            params![now_ms, limit],
        )?;
        transaction.commit()?;
        Ok(changed)
    }
}

pub(crate) fn insert_catalog_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    catalog: &BillingPriceCatalog,
) -> StoreResult<()> {
    validate_catalog(catalog)?;
    let existing: Option<(i64, String, i64)> = transaction
        .query_row(
            "SELECT effective_at_ms, source, created_at_ms FROM billing_price_catalog_versions \
             WHERE catalog_version_id = ?1",
            [catalog.catalog_version_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((effective_at_ms, source, created_at_ms)) = existing {
        let existing_entries = load_catalog_entries(transaction, &catalog.catalog_version_id)?;
        let same = u64_from_i64(effective_at_ms)? == catalog.effective_at_ms
            && BillingCatalogSource::from_sql(&source)? == catalog.source
            && u64_from_i64(created_at_ms)? == catalog.created_at_ms
            && existing_entries == sorted_entries(catalog.entries.clone());
        if same {
            return Ok(());
        }
        return Err(StoreError::ConflictingBillingCatalogVersion);
    }

    transaction.execute(
        "INSERT INTO billing_price_catalog_versions \
         (catalog_version_id, effective_at_ms, source, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
        params![
            catalog.catalog_version_id,
            i64_from_u64(catalog.effective_at_ms)?,
            catalog.source.as_sql(),
            i64_from_u64(catalog.created_at_ms)?,
        ],
    )?;
    for entry in sorted_entries(catalog.entries.clone()) {
        transaction.execute(
            "INSERT INTO billing_price_catalog_entries \
             (catalog_version_id, provider_id, channel_id, model, \
              input_microunits_per_million, output_microunits_per_million, \
              reasoning_microunits_per_million, cache_read_microunits_per_million, \
              cache_creation_microunits_per_million, cached_microunits_per_million) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                catalog.catalog_version_id,
                entry.provider_id,
                entry.channel_id,
                entry.model,
                i64_from_u64(entry.input_microunits_per_million)?,
                i64_from_u64(entry.output_microunits_per_million)?,
                i64_from_u64(entry.reasoning_microunits_per_million)?,
                i64_from_u64(entry.cache_read_microunits_per_million)?,
                i64_from_u64(entry.cache_creation_microunits_per_million)?,
                i64_from_u64(entry.cached_microunits_per_million)?,
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn load_catalog_from_connection(
    connection: &Connection,
    catalog_version_id: &str,
) -> StoreResult<Option<BillingPriceCatalog>> {
    let Some((effective_at_ms, source, created_at_ms)) = connection
        .query_row(
            "SELECT effective_at_ms, source, created_at_ms \
             FROM billing_price_catalog_versions WHERE catalog_version_id = ?1",
            [catalog_version_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    let entries = load_catalog_entries(connection, catalog_version_id)?;
    Ok(Some(BillingPriceCatalog {
        catalog_version_id: catalog_version_id.to_owned(),
        effective_at_ms: u64_from_i64(effective_at_ms)?,
        source: BillingCatalogSource::from_sql(&source)?,
        created_at_ms: u64_from_i64(created_at_ms)?,
        entries,
    }))
}

pub(crate) fn list_catalogs_bounded_from_connection(
    connection: &Connection,
    limit: usize,
) -> StoreResult<Vec<BillingPriceCatalog>> {
    let limit = i64::try_from(limit).map_err(|_| StoreError::InvalidPersistedBillingRecord)?;
    let mut statement = connection.prepare(
        "SELECT catalog_version_id FROM billing_price_catalog_versions \
         ORDER BY effective_at_ms, catalog_version_id LIMIT ?1",
    )?;
    let ids = statement
        .query_map([limit], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|id| {
            load_catalog_from_connection(connection, &id)?
                .ok_or(StoreError::InvalidPersistedBillingRecord)
        })
        .collect()
}

fn sorted_entries(mut entries: Vec<BillingPriceEntry>) -> Vec<BillingPriceEntry> {
    entries.sort_by(|left, right| {
        (&left.provider_id, &left.channel_id, &left.model).cmp(&(
            &right.provider_id,
            &right.channel_id,
            &right.model,
        ))
    });
    entries
}

fn validate_catalog(catalog: &BillingPriceCatalog) -> StoreResult<()> {
    validate_short_id(&catalog.catalog_version_id)?;
    if catalog.entries.is_empty() {
        return Err(StoreError::InvalidPersistedBillingRecord);
    }
    let sorted = sorted_entries(catalog.entries.clone());
    if sorted.windows(2).any(|pair| {
        (&pair[0].provider_id, &pair[0].channel_id, &pair[0].model)
            == (&pair[1].provider_id, &pair[1].channel_id, &pair[1].model)
    }) {
        return Err(StoreError::InvalidPersistedBillingRecord);
    }
    for entry in sorted {
        validate_short_id(&entry.provider_id)?;
        validate_short_id(&entry.channel_id)?;
        validate_model(&entry.model)?;
    }
    Ok(())
}

fn validate_input(input: &BillingLedgerEntryInput) -> StoreResult<()> {
    validate_id(&input.source_event_id)?;
    validate_id(&input.request_id)?;
    validate_id(&input.response_id)?;
    validate_short_id(&input.provider_id)?;
    validate_short_id(&input.channel_id)?;
    validate_short_id(&input.account_id)?;
    validate_model(&input.model)?;
    if input.retention_expires_at_ms < input.occurred_at_ms {
        return Err(StoreError::InvalidPersistedBillingRecord);
    }
    if let Some(value) = &input.catalog_version_id {
        validate_short_id(value)?;
    }
    Ok(())
}

fn validate_id(value: &str) -> StoreResult<()> {
    if value.trim().is_empty() || value.len() > MAX_ID_BYTES {
        return Err(StoreError::InvalidPersistedBillingRecord);
    }
    Ok(())
}

fn validate_short_id(value: &str) -> StoreResult<()> {
    if value.trim().is_empty() || value.len() > MAX_SHORT_ID_BYTES {
        return Err(StoreError::InvalidPersistedBillingRecord);
    }
    Ok(())
}

fn validate_model(value: &str) -> StoreResult<()> {
    if value.trim().is_empty() || value.len() > MAX_MODEL_BYTES {
        return Err(StoreError::InvalidPersistedBillingRecord);
    }
    Ok(())
}

fn i64_from_u64(value: u64) -> StoreResult<i64> {
    i64::try_from(value).map_err(|_| StoreError::InvalidPersistedBillingRecord)
}

fn optional_i64(value: Option<u64>) -> StoreResult<Option<i64>> {
    value.map(i64_from_u64).transpose()
}

fn u64_from_i64(value: i64) -> StoreResult<u64> {
    u64::try_from(value).map_err(|_| StoreError::InvalidPersistedBillingRecord)
}

fn load_catalog_entries(
    connection: &Connection,
    catalog_version_id: &str,
) -> StoreResult<Vec<BillingPriceEntry>> {
    let mut statement = connection.prepare(
        "SELECT provider_id, channel_id, model, input_microunits_per_million, \
         output_microunits_per_million, reasoning_microunits_per_million, \
         cache_read_microunits_per_million, cache_creation_microunits_per_million, \
         cached_microunits_per_million FROM billing_price_catalog_entries \
         WHERE catalog_version_id = ?1 ORDER BY provider_id, channel_id, model",
    )?;
    let rows = statement
        .query_map([catalog_version_id], |row| {
            Ok(BillingPriceEntry {
                provider_id: row.get(0)?,
                channel_id: row.get(1)?,
                model: row.get(2)?,
                input_microunits_per_million: row.get::<_, i64>(3)?.try_into().map_err(|_| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Integer,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "negative billing rate",
                        )),
                    )
                })?,
                output_microunits_per_million: row.get::<_, i64>(4)?.try_into().map_err(|_| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Integer,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "negative billing rate",
                        )),
                    )
                })?,
                reasoning_microunits_per_million: row.get::<_, i64>(5)?.try_into().map_err(
                    |_| {
                        rusqlite::Error::FromSqlConversionFailure(
                            5,
                            rusqlite::types::Type::Integer,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "negative billing rate",
                            )),
                        )
                    },
                )?,
                cache_read_microunits_per_million: row.get::<_, i64>(6)?.try_into().map_err(
                    |_| {
                        rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Integer,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "negative billing rate",
                            )),
                        )
                    },
                )?,
                cache_creation_microunits_per_million: row.get::<_, i64>(7)?.try_into().map_err(
                    |_| {
                        rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Integer,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "negative billing rate",
                            )),
                        )
                    },
                )?,
                cached_microunits_per_million: row.get::<_, i64>(8)?.try_into().map_err(|_| {
                    rusqlite::Error::FromSqlConversionFailure(
                        8,
                        rusqlite::types::Type::Integer,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "negative billing rate",
                        )),
                    )
                })?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_entry(connection: &Connection, ledger_id: i64) -> StoreResult<BillingLedgerEntry> {
    let mut statement = connection.prepare(&format!("{LEDGER_ROW_SELECT} WHERE ledger_id = ?1"))?;
    let mut rows = statement.query([ledger_id])?;
    let row = rows
        .next()?
        .ok_or(StoreError::InvalidPersistedBillingRecord)?;
    decode_ledger_row(row)
}

fn decode_ledger_row(row: &rusqlite::Row<'_>) -> StoreResult<BillingLedgerEntry> {
    let row = (
        row.get::<_, i64>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, String>(5)?,
        row.get::<_, String>(6)?,
        row.get::<_, String>(7)?,
        row.get::<_, String>(8)?,
        row.get::<_, i64>(9)?,
        row.get::<_, Option<String>>(10)?,
        row.get::<_, Option<i64>>(11)?,
        row.get::<_, Option<i64>>(12)?,
        row.get::<_, Option<i64>>(13)?,
        row.get::<_, Option<i64>>(14)?,
        row.get::<_, Option<i64>>(15)?,
        row.get::<_, Option<i64>>(16)?,
        row.get::<_, Option<i64>>(17)?,
        row.get::<_, String>(18)?,
        row.get::<_, i64>(19)?,
        row.get::<_, i64>(20)?,
    );
    let (
        ledger_id,
        source_event_id,
        source_fingerprint,
        request_id,
        response_id,
        provider_id,
        channel_id,
        account_id,
        model,
        occurred_at_ms,
        catalog_version_id,
        input_tokens,
        output_tokens,
        reasoning_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        cached_tokens,
        cost_microunits,
        cost_confidence,
        retention_expires_at_ms,
        recorded_at_ms,
    ) = row;
    if source_fingerprint.len() != 64
        || !source_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(StoreError::InvalidPersistedBillingRecord);
    }
    Ok(BillingLedgerEntry {
        ledger_id,
        source_event_id,
        source_fingerprint,
        request_id,
        response_id,
        provider_id,
        channel_id,
        account_id,
        model,
        occurred_at_ms: u64_from_i64(occurred_at_ms)?,
        catalog_version_id,
        usage: UsageSummary {
            input_tokens: input_tokens.map(u64_from_i64).transpose()?,
            output_tokens: output_tokens.map(u64_from_i64).transpose()?,
            reasoning_tokens: reasoning_tokens.map(u64_from_i64).transpose()?,
            cache_read_tokens: cache_read_tokens.map(u64_from_i64).transpose()?,
            cache_creation_tokens: cache_creation_tokens.map(u64_from_i64).transpose()?,
            cached_tokens: cached_tokens.map(u64_from_i64).transpose()?,
        },
        cost_microunits: cost_microunits.map(u64_from_i64).transpose()?,
        cost_confidence: BillingCostConfidence::from_sql(&cost_confidence)?,
        retention_expires_at_ms: u64_from_i64(retention_expires_at_ms)?,
        recorded_at_ms: u64_from_i64(recorded_at_ms)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> BillingPriceCatalog {
        BillingPriceCatalog {
            catalog_version_id: "catalog-1".to_owned(),
            effective_at_ms: 100,
            source: BillingCatalogSource::Test,
            created_at_ms: 100,
            entries: vec![BillingPriceEntry {
                provider_id: "provider-a".to_owned(),
                channel_id: "channel-a".to_owned(),
                model: "model-a".to_owned(),
                input_microunits_per_million: 2_000_000,
                output_microunits_per_million: 4_000_000,
                reasoning_microunits_per_million: 0,
                cache_read_microunits_per_million: 1_000_000,
                cache_creation_microunits_per_million: 1_000_000,
                cached_microunits_per_million: 500_000,
            }],
        }
    }

    fn entry() -> BillingLedgerEntryInput {
        BillingLedgerEntryInput {
            source_event_id: "usage-event-1".to_owned(),
            request_id: "request-1".to_owned(),
            response_id: "response-1".to_owned(),
            provider_id: "provider-a".to_owned(),
            channel_id: "channel-a".to_owned(),
            account_id: "account-a".to_owned(),
            model: "model-a".to_owned(),
            occurred_at_ms: 100,
            catalog_version_id: Some("catalog-1".to_owned()),
            usage: UsageSummary {
                input_tokens: Some(1_000_000),
                output_tokens: Some(500_000),
                reasoning_tokens: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                cached_tokens: None,
            },
            cost_microunits: Some(4_000_000),
            cost_confidence: BillingCostConfidence::Exact,
            retention_expires_at_ms: 1_000,
            recorded_at_ms: 101,
        }
    }

    #[test]
    fn catalog_is_immutable_and_replayable() -> Result<(), Box<dyn std::error::Error>> {
        let mut store = SqliteBillingLedger::open_in_memory()?;
        let mut value = catalog();
        value.entries.reverse();
        store.insert_catalog(&value)?;
        store.insert_catalog(&catalog())?;
        value.created_at_ms = 101;
        assert!(matches!(
            store.insert_catalog(&value),
            Err(StoreError::ConflictingBillingCatalogVersion)
        ));
        let loaded = store
            .catalog("catalog-1")?
            .ok_or("catalog unexpectedly missing")?;
        assert_eq!(loaded.entries, catalog().entries);
        Ok(())
    }

    #[test]
    fn bulk_ledger_reads_preserve_complete_rows_and_stable_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut store = SqliteBillingLedger::open_in_memory()?;
        store.insert_catalog(&catalog())?;
        let mut expected = Vec::new();
        for index in 0..3 {
            let mut value = entry();
            value.source_event_id = format!("bulk-event-{index}");
            value.occurred_at_ms = if index == 0 { 200 } else { 100 };
            value.usage.reasoning_tokens = Some(0);
            value.usage.cache_read_tokens = Some(12);
            value.usage.cache_creation_tokens = Some(13);
            value.usage.cached_tokens = None;
            if index == 1 {
                value.catalog_version_id = None;
                value.cost_confidence = BillingCostConfidence::Unpriced;
                value.cost_microunits = None;
            }
            let BillingRecordResult::Inserted(row) = store.record(&value)? else {
                return Err("unexpected replay".into());
            };
            expected.push(row);
        }
        expected.sort_by_key(|row| (row.occurred_at_ms, row.ledger_id));
        assert_eq!(store.list_bounded(10)?, expected);
        assert_eq!(store.list_bounded(2)?, expected[..2]);
        assert!(store.list_bounded(0)?.is_empty());
        Ok(())
    }

    #[test]
    fn ledger_replay_is_idempotent_and_conflict_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut store = SqliteBillingLedger::open_in_memory()?;
        store.insert_catalog(&catalog())?;
        let value = entry();
        let first = store.record(&value)?;
        let second = store.record(&value)?;
        assert!(matches!(first, BillingRecordResult::Inserted(_)));
        assert!(matches!(second, BillingRecordResult::Replay(_)));
        assert_eq!(store.list_bounded(10)?.len(), 1);
        let mut conflict = value;
        conflict.model = "other-model".to_owned();
        assert!(matches!(
            store.record(&conflict),
            Err(StoreError::ConflictingBillingLedgerReplay)
        ));
        Ok(())
    }

    #[test]
    fn purge_is_bounded_and_survives_reopen() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("cpar-billing-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = SqliteBillingLedger::open(&path)?;
            store.insert_catalog(&catalog())?;
            store.record(&entry())?;
            let mut second = entry();
            second.source_event_id = "usage-event-2".to_owned();
            second.retention_expires_at_ms = 2_000;
            store.record(&second)?;
            assert_eq!(store.purge_expired(1_000, 1)?, 1);
        }
        let mut reopened = SqliteBillingLedger::open(&path)?;
        assert_eq!(reopened.list_bounded(10)?.len(), 1);
        assert_eq!(reopened.purge_expired(3_000, 10)?, 1);
        assert!(reopened.list_bounded(10)?.is_empty());
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn materialization_failures_are_durable_bounded_and_repairable()
    -> Result<(), Box<dyn std::error::Error>> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cpar-billing-failures-{}-{nonce}.sqlite",
            std::process::id()
        ));
        let mut store = SqliteBillingLedger::open(&path)?;
        store.record_materialization_failure("billing-v1", 2, "invalid_lineage", 1000)?;
        store.record_materialization_failure("billing-v1", 1, "pricing_overflow", 2000)?;
        let first = store.list_materialization_failures_due("billing-v1", 2000, 1)?;
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].event_ordinal, 2);
        store.record_materialization_failure("billing-v1", 2, "invalid_lineage", 3000)?;
        assert_eq!(
            store
                .list_materialization_failures_due("billing-v1", 2500, 10)?
                .len(),
            1
        );
        assert!(
            store
                .list_materialization_failures_due("billing-v1", 4000, 0)
                .is_err()
        );
        assert!(
            store
                .record_materialization_failure("billing-v1", 3, "raw exception", 4000)
                .is_err()
        );
        drop(store);
        let mut store = SqliteBillingLedger::open(&path)?;
        let pending = store.list_materialization_failures_due("billing-v1", 4000, 10)?;
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[1].first_seen_at_ms, 1000);
        assert_eq!(pending[1].attempts, 2);
        store.resolve_materialization_failure("billing-v1", 2, 4000)?;
        assert_eq!(
            store
                .list_materialization_failures_due("billing-v1", 5000, 10)?
                .len(),
            1
        );
        let retained: i64 = store.connection.query_row(
            "SELECT COUNT(*) FROM billing_materializer_failures WHERE resolved_at_ms IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(retained, 1);
        drop(store);
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn processing_progress_distinguishes_empty_lag_and_unresolved_failures()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut ledger = SqliteBillingLedger::open_in_memory()?;
        let empty = ledger.materialization_progress("billing-v1")?;
        assert_eq!(empty.source_ordinal, 0);
        assert!(empty.checkpoint_ordinal.is_none());
        assert!(empty.checkpoint_updated_at_ms.is_none());
        ledger.connection.execute("INSERT INTO gateway_event_log(event_type, event_id, payload_json) VALUES ('usage', 'first', '{}'), ('usage', 'second', '{}')", [])?;
        ledger.save_checkpoint("billing-v1", 1, 1000)?;
        ledger.record_materialization_failure("billing-v1", 1, "invalid_event", 1000)?;
        let lag = ledger.materialization_progress("billing-v1")?;
        assert_eq!(lag.source_ordinal, 2);
        assert_eq!(lag.checkpoint_ordinal, Some(1));
        assert_eq!(lag.checkpoint_updated_at_ms, Some(1000));
        assert_eq!(lag.unresolved_failures, 1);
        ledger.save_checkpoint("billing-v1", 2, 2000)?;
        assert_eq!(
            ledger
                .materialization_progress("billing-v1")?
                .unresolved_failures,
            1
        );
        ledger.resolve_materialization_failure("billing-v1", 1, 3000)?;
        assert_eq!(
            ledger
                .materialization_progress("billing-v1")?
                .unresolved_failures,
            0
        );
        ledger.save_checkpoint("billing-v1", 3, 4000)?;
        assert!(ledger.materialization_progress("billing-v1").is_err());
        Ok(())
    }

    #[test]
    fn checkpoint_is_monotonic_and_catalogs_are_stably_ordered()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut store = SqliteBillingLedger::open_in_memory()?;
        let mut later = catalog();
        later.catalog_version_id = "catalog-2".to_owned();
        later.effective_at_ms = 200;
        store.insert_catalog(&later)?;
        store.insert_catalog(&catalog())?;
        assert_eq!(
            store
                .list_catalogs_bounded(10)?
                .into_iter()
                .map(|value| value.catalog_version_id)
                .collect::<Vec<_>>(),
            vec!["catalog-1", "catalog-2"]
        );

        assert!(store.load_checkpoint("billing-v1")?.is_none());
        store.save_checkpoint("billing-v1", 10, 1_000)?;
        store.save_checkpoint("billing-v1", 10, 1_001)?;
        assert_eq!(
            store
                .load_checkpoint("billing-v1")?
                .ok_or("checkpoint missing")?,
            BillingMaterializerCheckpoint {
                materializer_id: "billing-v1".to_owned(),
                event_ordinal: 10,
                updated_at_ms: 1_001,
            }
        );
        assert!(matches!(
            store.save_checkpoint("billing-v1", 9, 1_002),
            Err(StoreError::InvalidPersistedBillingRecord)
        ));
        Ok(())
    }
}
