use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    path::Path,
    sync::{Mutex, MutexGuard},
};

use gateway_core::{CredentialId, EndpointId};
use gateway_store::{StoreError, migrate, open, open_in_memory};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    CatalogFreshnessPolicy, CatalogSnapshot, CatalogSnapshotError, CatalogSnapshotFreshness,
    CatalogSnapshotStatus, DiscoveredModel, MIN_CATALOG_REMOVAL_ISOLATION_MS,
    MIN_CATALOG_SUCCESSFUL_MISSES_FOR_REMOVAL, ModelCatalogTarget,
};

/// Safe, bounded class retained for the latest failed discovery attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogDiscoveryFailureClass {
    /// Credential authentication was rejected or could not be decoded safely.
    Authentication,
    /// The credential was authenticated but forbidden for the discovery resource.
    Authorization,
    /// The target was rate- or quota-limited.
    RateLimit,
    /// DNS, proxy, egress, or HTTP transport failed.
    Transport,
    /// The Provider returned an invalid or unsuccessful application response.
    Upstream,
    /// CPAR could not safely complete its own discovery pipeline.
    Internal,
}

impl CatalogDiscoveryFailureClass {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::Authorization => "authorization",
            Self::RateLimit => "rate_limit",
            Self::Transport => "transport",
            Self::Upstream => "upstream",
            Self::Internal => "internal",
        }
    }

    fn parse(value: &str) -> Result<Self, DurableCatalogError> {
        match value {
            "authentication" => Ok(Self::Authentication),
            "authorization" => Ok(Self::Authorization),
            "rate_limit" => Ok(Self::RateLimit),
            "transport" => Ok(Self::Transport),
            "upstream" => Ok(Self::Upstream),
            "internal" => Ok(Self::Internal),
            _ => Err(DurableCatalogError::InvalidPersistedRecord),
        }
    }
}

/// One retained model and its successful-miss isolation evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableCatalogModel {
    model: DiscoveredModel,
    present_in_last_success: bool,
    consecutive_successful_misses: u64,
    first_missing_at_ms: Option<i64>,
    removal_eligible_at_ms: Option<i64>,
}

impl DurableCatalogModel {
    /// Returns the exact source-provided model identity.
    #[must_use]
    pub fn model(&self) -> &DiscoveredModel {
        &self.model
    }

    /// Returns whether the most recent successful source response contained this model.
    #[must_use]
    pub const fn is_present_in_last_success(&self) -> bool {
        self.present_in_last_success
    }

    /// Returns the number of consecutive successful source responses that omitted this model.
    #[must_use]
    pub const fn consecutive_successful_misses(&self) -> u64 {
        self.consecutive_successful_misses
    }

    /// Returns the first successful omission timestamp, when currently missing.
    #[must_use]
    pub const fn first_missing_at_ms(&self) -> Option<i64> {
        self.first_missing_at_ms
    }

    /// Returns the earliest isolation deadline for removal, when currently missing.
    #[must_use]
    pub const fn removal_eligible_at_ms(&self) -> Option<i64> {
        self.removal_eligible_at_ms
    }
}

/// Durable target-local last-success state evaluated at one explicit clock instant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableCatalogSnapshotStatus {
    status: CatalogSnapshotStatus,
    models: Vec<DurableCatalogModel>,
    last_failure: Option<(i64, CatalogDiscoveryFailureClass)>,
}

/// Latest safe discovery failure for one exact target, including targets with no success yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableCatalogFailureStatus {
    target: ModelCatalogTarget,
    failed_at_ms: i64,
    class: CatalogDiscoveryFailureClass,
}

impl DurableCatalogFailureStatus {
    /// Returns the exact Endpoint/Credential failure target.
    #[must_use]
    pub fn target(&self) -> &ModelCatalogTarget {
        &self.target
    }

    /// Returns when this latest failed discovery was observed.
    #[must_use]
    pub const fn failed_at_ms(&self) -> i64 {
        self.failed_at_ms
    }

    /// Returns the bounded, secret-free failure class.
    #[must_use]
    pub const fn class(&self) -> CatalogDiscoveryFailureClass {
        self.class
    }
}

impl DurableCatalogSnapshotStatus {
    /// Returns the immutable target-local last-success snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &CatalogSnapshot {
        self.status.snapshot()
    }

    /// Returns Fresh, Stale, or Expired at the requested observation time.
    #[must_use]
    pub const fn freshness(&self) -> CatalogSnapshotFreshness {
        self.status.freshness()
    }

    /// Returns whether the independent refresh deadline has elapsed.
    #[must_use]
    pub const fn is_refresh_due(&self) -> bool {
        self.status.is_refresh_due()
    }

    /// Returns every model still eligible under the three-success/24-hour removal rule.
    pub fn eligible_models(&self) -> impl Iterator<Item = &DiscoveredModel> {
        self.models.iter().map(DurableCatalogModel::model)
    }

    /// Returns every retained model together with its removal-isolation evidence.
    #[must_use]
    pub fn models(&self) -> &[DurableCatalogModel] {
        &self.models
    }

    /// Returns the most recent failed-at timestamp and safe failure class after last success.
    #[must_use]
    pub const fn last_failure(&self) -> Option<(i64, CatalogDiscoveryFailureClass)> {
        self.last_failure
    }
}

/// Thread-safe durable Catalog repository sharing the control-plane `SQLite` database.
pub struct SqliteCatalogSnapshotStore {
    connection: Mutex<Connection>,
    policy: CatalogFreshnessPolicy,
}

impl SqliteCatalogSnapshotStore {
    /// Opens and migrates the shared file-backed control-plane database.
    ///
    /// # Errors
    ///
    /// Returns a safe storage or migration error.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DurableCatalogError> {
        Self::from_connection(open(path)?, CatalogFreshnessPolicy::default())
    }

    /// Opens and migrates an isolated in-memory repository.
    ///
    /// # Errors
    ///
    /// Returns a safe storage or migration error.
    pub fn open_in_memory() -> Result<Self, DurableCatalogError> {
        Self::from_connection(open_in_memory()?, CatalogFreshnessPolicy::default())
    }

    /// Takes an existing connection, applies migrations, and owns it behind a short-held mutex.
    ///
    /// # Errors
    ///
    /// Returns a safe storage or migration error.
    pub fn from_connection(
        mut connection: Connection,
        policy: CatalogFreshnessPolicy,
    ) -> Result<Self, DurableCatalogError> {
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            policy,
        })
    }

    /// Atomically advances one target only for a successful source response.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, storage, migration, or persisted-record error.
    pub fn record_success(
        &self,
        config_version_id: &str,
        target: &ModelCatalogTarget,
        models: impl IntoIterator<Item = DiscoveredModel>,
        observed_at_ms: i64,
    ) -> Result<DurableCatalogSnapshotStatus, DurableCatalogError> {
        validate_scope(config_version_id)?;
        let discovered = models.into_iter().collect::<BTreeSet<_>>();
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let previous = load_target_header(&transaction, config_version_id, target)?;
        let version = previous.as_ref().map_or(Ok(1_u64), |header| {
            if observed_at_ms < header.observed_at_ms {
                return Err(DurableCatalogError::Snapshot(
                    CatalogSnapshotError::TimestampNotMonotonic,
                ));
            }
            header
                .version
                .checked_add(1)
                .ok_or(DurableCatalogError::Snapshot(
                    CatalogSnapshotError::SnapshotVersionOverflow,
                ))
        })?;
        let source_snapshot = CatalogSnapshot::try_new(
            target.clone(),
            discovered.iter().cloned(),
            version,
            observed_at_ms,
            self.policy,
        )?;
        let previous_models = load_models(&transaction, config_version_id, target)?;
        let retained = next_retained_models(previous_models, &discovered, observed_at_ms)?;

        transaction.execute(
            "INSERT INTO model_catalog_targets (
                config_version_id, endpoint_id, credential_id, snapshot_version,
                observed_at_ms, stale_at_ms, refresh_due_at_ms, expires_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(config_version_id, endpoint_id, credential_id) DO UPDATE SET
                snapshot_version = excluded.snapshot_version,
                observed_at_ms = excluded.observed_at_ms,
                stale_at_ms = excluded.stale_at_ms,
                refresh_due_at_ms = excluded.refresh_due_at_ms,
                expires_at_ms = excluded.expires_at_ms",
            params![
                config_version_id,
                target.endpoint_id().as_str(),
                target.credential_id().as_str(),
                i64::try_from(source_snapshot.version())
                    .map_err(|_| DurableCatalogError::InvalidInput)?,
                source_snapshot.observed_at_ms(),
                source_snapshot.stale_at_ms(),
                source_snapshot.refresh_due_at_ms(),
                source_snapshot.expires_at_ms(),
            ],
        )?;
        transaction.execute(
            "DELETE FROM model_catalog_models
             WHERE config_version_id = ?1 AND endpoint_id = ?2 AND credential_id = ?3",
            params![
                config_version_id,
                target.endpoint_id().as_str(),
                target.credential_id().as_str()
            ],
        )?;
        for model in retained.values() {
            transaction.execute(
                "INSERT INTO model_catalog_models (
                    config_version_id, endpoint_id, credential_id, model,
                    present_in_last_success, consecutive_successful_misses,
                    first_missing_at_ms, removal_eligible_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    config_version_id,
                    target.endpoint_id().as_str(),
                    target.credential_id().as_str(),
                    model.model.upstream_model(),
                    i64::from(model.present_in_last_success),
                    i64::try_from(model.consecutive_successful_misses)
                        .map_err(|_| DurableCatalogError::InvalidInput)?,
                    model.first_missing_at_ms,
                    model.removal_eligible_at_ms,
                ],
            )?;
        }
        transaction.execute(
            "DELETE FROM model_catalog_failures
             WHERE config_version_id = ?1 AND endpoint_id = ?2 AND credential_id = ?3
               AND failed_at_ms <= ?4",
            params![
                config_version_id,
                target.endpoint_id().as_str(),
                target.credential_id().as_str(),
                observed_at_ms
            ],
        )?;
        transaction.commit()?;
        drop(connection);
        self.status_at(config_version_id, target, observed_at_ms)?
            .ok_or(DurableCatalogError::InvalidPersistedRecord)
    }

    /// Records only a safe failure class while preserving every last-success field and model row.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, storage, or persisted-record error.
    pub fn record_failure(
        &self,
        config_version_id: &str,
        target: &ModelCatalogTarget,
        failed_at_ms: i64,
        class: CatalogDiscoveryFailureClass,
    ) -> Result<bool, DurableCatalogError> {
        validate_scope(config_version_id)?;
        if failed_at_ms < 0 {
            return Err(DurableCatalogError::InvalidInput);
        }
        let changed = self.lock_connection()?.execute(
            "INSERT INTO model_catalog_failures (
                config_version_id, endpoint_id, credential_id, failed_at_ms, failure_class
             ) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(config_version_id, endpoint_id, credential_id) DO UPDATE SET
                failed_at_ms = excluded.failed_at_ms,
                failure_class = excluded.failure_class
             WHERE excluded.failed_at_ms >= model_catalog_failures.failed_at_ms",
            params![
                config_version_id,
                target.endpoint_id().as_str(),
                target.credential_id().as_str(),
                failed_at_ms,
                class.as_str(),
            ],
        )?;
        Ok(changed == 1)
    }

    /// Loads one exact target-local last-success status at an explicit timestamp.
    ///
    /// # Errors
    ///
    /// Returns a safe input, storage, or persisted-record error.
    pub fn status_at(
        &self,
        config_version_id: &str,
        target: &ModelCatalogTarget,
        now_ms: i64,
    ) -> Result<Option<DurableCatalogSnapshotStatus>, DurableCatalogError> {
        validate_scope(config_version_id)?;
        let connection = self.lock_connection()?;
        load_status(&connection, config_version_id, target, now_ms, self.policy)
    }

    /// Lists all successful target-local statuses in stable target order.
    ///
    /// # Errors
    ///
    /// Returns a safe input, storage, or persisted-record error.
    pub fn list_statuses_at(
        &self,
        config_version_id: &str,
        now_ms: i64,
    ) -> Result<Vec<DurableCatalogSnapshotStatus>, DurableCatalogError> {
        validate_scope(config_version_id)?;
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT endpoint_id, credential_id FROM model_catalog_targets
             WHERE config_version_id = ?1 ORDER BY endpoint_id, credential_id",
        )?;
        let targets = statement
            .query_map([config_version_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        targets
            .into_iter()
            .map(|(endpoint, credential)| {
                let target = ModelCatalogTarget::new(
                    EndpointId::try_new(endpoint)
                        .map_err(|_| DurableCatalogError::InvalidPersistedRecord)?,
                    CredentialId::try_new(credential)
                        .map_err(|_| DurableCatalogError::InvalidPersistedRecord)?,
                );
                load_status(&connection, config_version_id, &target, now_ms, self.policy)?
                    .ok_or(DurableCatalogError::InvalidPersistedRecord)
            })
            .collect()
    }

    /// Lists the latest safe failure for every exact target in stable target order.
    ///
    /// # Errors
    ///
    /// Returns a safe input, storage, or persisted-record error.
    pub fn list_failures(
        &self,
        config_version_id: &str,
    ) -> Result<Vec<DurableCatalogFailureStatus>, DurableCatalogError> {
        validate_scope(config_version_id)?;
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT endpoint_id, credential_id, failed_at_ms, failure_class
             FROM model_catalog_failures WHERE config_version_id = ?1
             ORDER BY endpoint_id, credential_id",
        )?;
        let rows = statement
            .query_map([config_version_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(endpoint, credential, failed_at_ms, class)| {
                Ok(DurableCatalogFailureStatus {
                    target: ModelCatalogTarget::new(
                        EndpointId::try_new(endpoint)
                            .map_err(|_| DurableCatalogError::InvalidPersistedRecord)?,
                        CredentialId::try_new(credential)
                            .map_err(|_| DurableCatalogError::InvalidPersistedRecord)?,
                    ),
                    failed_at_ms,
                    class: CatalogDiscoveryFailureClass::parse(&class)?,
                })
            })
            .collect()
    }

    fn lock_connection(&self) -> Result<MutexGuard<'_, Connection>, DurableCatalogError> {
        self.connection
            .lock()
            .map_err(|_| DurableCatalogError::Unavailable)
    }
}

#[derive(Clone, Copy)]
struct TargetHeader {
    version: u64,
    observed_at_ms: i64,
}

fn load_target_header(
    connection: &Connection,
    config_version_id: &str,
    target: &ModelCatalogTarget,
) -> Result<Option<TargetHeader>, DurableCatalogError> {
    connection
        .query_row(
            "SELECT snapshot_version, observed_at_ms FROM model_catalog_targets
             WHERE config_version_id = ?1 AND endpoint_id = ?2 AND credential_id = ?3",
            params![
                config_version_id,
                target.endpoint_id().as_str(),
                target.credential_id().as_str()
            ],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
        .map(|(version, observed_at_ms)| {
            Ok(TargetHeader {
                version: u64::try_from(version)
                    .map_err(|_| DurableCatalogError::InvalidPersistedRecord)?,
                observed_at_ms,
            })
        })
        .transpose()
}

fn load_models(
    connection: &Connection,
    config_version_id: &str,
    target: &ModelCatalogTarget,
) -> Result<BTreeMap<DiscoveredModel, DurableCatalogModel>, DurableCatalogError> {
    let mut statement = connection.prepare(
        "SELECT model, present_in_last_success, consecutive_successful_misses,
                first_missing_at_ms, removal_eligible_at_ms
         FROM model_catalog_models
         WHERE config_version_id = ?1 AND endpoint_id = ?2 AND credential_id = ?3
         ORDER BY model",
    )?;
    let rows = statement
        .query_map(
            params![
                config_version_id,
                target.endpoint_id().as_str(),
                target.credential_id().as_str()
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let mut models = BTreeMap::new();
    for (model, present, misses, first_missing, removal_eligible) in rows {
        let model = DiscoveredModel::try_new(model)
            .map_err(|_| DurableCatalogError::InvalidPersistedRecord)?;
        let misses =
            u64::try_from(misses).map_err(|_| DurableCatalogError::InvalidPersistedRecord)?;
        let record = DurableCatalogModel {
            model: model.clone(),
            present_in_last_success: present == 1,
            consecutive_successful_misses: misses,
            first_missing_at_ms: first_missing,
            removal_eligible_at_ms: removal_eligible,
        };
        if models.insert(model, record).is_some() {
            return Err(DurableCatalogError::InvalidPersistedRecord);
        }
    }
    Ok(models)
}

fn next_retained_models(
    previous: BTreeMap<DiscoveredModel, DurableCatalogModel>,
    discovered: &BTreeSet<DiscoveredModel>,
    observed_at_ms: i64,
) -> Result<BTreeMap<DiscoveredModel, DurableCatalogModel>, DurableCatalogError> {
    let mut next = BTreeMap::new();
    for model in discovered {
        next.insert(
            model.clone(),
            DurableCatalogModel {
                model: model.clone(),
                present_in_last_success: true,
                consecutive_successful_misses: 0,
                first_missing_at_ms: None,
                removal_eligible_at_ms: None,
            },
        );
    }
    for (model, previous) in previous {
        if discovered.contains(&model) {
            continue;
        }
        let misses = previous
            .consecutive_successful_misses
            .checked_add(1)
            .ok_or(DurableCatalogError::InvalidInput)?;
        let first_missing_at_ms = previous.first_missing_at_ms.unwrap_or(observed_at_ms);
        let removal_eligible_at_ms = previous.removal_eligible_at_ms.unwrap_or(
            first_missing_at_ms
                .checked_add(MIN_CATALOG_REMOVAL_ISOLATION_MS)
                .ok_or(DurableCatalogError::InvalidInput)?,
        );
        if misses >= MIN_CATALOG_SUCCESSFUL_MISSES_FOR_REMOVAL
            && observed_at_ms >= removal_eligible_at_ms
        {
            continue;
        }
        next.insert(
            model.clone(),
            DurableCatalogModel {
                model,
                present_in_last_success: false,
                consecutive_successful_misses: misses,
                first_missing_at_ms: Some(first_missing_at_ms),
                removal_eligible_at_ms: Some(removal_eligible_at_ms),
            },
        );
    }
    Ok(next)
}

fn load_status(
    connection: &Connection,
    config_version_id: &str,
    target: &ModelCatalogTarget,
    now_ms: i64,
    policy: CatalogFreshnessPolicy,
) -> Result<Option<DurableCatalogSnapshotStatus>, DurableCatalogError> {
    let row = connection
        .query_row(
            "SELECT snapshot_version, observed_at_ms, stale_at_ms,
                    refresh_due_at_ms, expires_at_ms
             FROM model_catalog_targets
             WHERE config_version_id = ?1 AND endpoint_id = ?2 AND credential_id = ?3",
            params![
                config_version_id,
                target.endpoint_id().as_str(),
                target.credential_id().as_str()
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((version, observed_at_ms, stale_at_ms, refresh_due_at_ms, expires_at_ms)) = row else {
        return Ok(None);
    };
    let models = load_models(connection, config_version_id, target)?;
    let source_models = models
        .values()
        .filter(|model| model.present_in_last_success)
        .map(|model| model.model.clone());
    let snapshot = CatalogSnapshot::try_new(
        target.clone(),
        source_models,
        u64::try_from(version).map_err(|_| DurableCatalogError::InvalidPersistedRecord)?,
        observed_at_ms,
        policy,
    )?;
    if snapshot.stale_at_ms() != stale_at_ms
        || snapshot.refresh_due_at_ms() != refresh_due_at_ms
        || snapshot.expires_at_ms() != expires_at_ms
    {
        return Err(DurableCatalogError::InvalidPersistedRecord);
    }
    let status = CatalogSnapshotStatus::at(snapshot, now_ms)?;
    let last_failure = connection
        .query_row(
            "SELECT failed_at_ms, failure_class FROM model_catalog_failures
             WHERE config_version_id = ?1 AND endpoint_id = ?2 AND credential_id = ?3",
            params![
                config_version_id,
                target.endpoint_id().as_str(),
                target.credential_id().as_str()
            ],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .map(|(at, class)| CatalogDiscoveryFailureClass::parse(&class).map(|class| (at, class)))
        .transpose()?;
    Ok(Some(DurableCatalogSnapshotStatus {
        status,
        models: models.into_values().collect(),
        last_failure,
    }))
}

fn validate_scope(config_version_id: &str) -> Result<(), DurableCatalogError> {
    if config_version_id.is_empty() {
        return Err(DurableCatalogError::InvalidInput);
    }
    Ok(())
}

/// Safe durable Catalog errors without upstream payloads or credential material.
#[derive(Debug)]
pub enum DurableCatalogError {
    /// Shared store open or migration failed.
    Store(StoreError),
    /// A Catalog-specific `SQLite` operation failed.
    Sqlite(rusqlite::Error),
    /// Snapshot freshness or version construction failed.
    Snapshot(CatalogSnapshotError),
    /// A caller supplied an invalid scope, timestamp, or finite counter.
    InvalidInput,
    /// Persisted state violated the fail-closed decoder invariants.
    InvalidPersistedRecord,
    /// The repository mutex is unavailable after a panic.
    Unavailable,
}

impl fmt::Display for DurableCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) | Self::Sqlite(_) => "durable Catalog storage failed",
            Self::Snapshot(_) => "durable Catalog snapshot is invalid",
            Self::InvalidInput => "durable Catalog input is invalid",
            Self::InvalidPersistedRecord => "durable Catalog record is invalid",
            Self::Unavailable => "durable Catalog storage is unavailable",
        })
    }
}

impl Error for DurableCatalogError {}

impl From<StoreError> for DurableCatalogError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<rusqlite::Error> for DurableCatalogError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<CatalogSnapshotError> for DurableCatalogError {
    fn from(error: CatalogSnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{CredentialId, EndpointId};

    use super::{CatalogDiscoveryFailureClass, SqliteCatalogSnapshotStore};
    use crate::{
        CatalogSnapshotFreshness, DiscoveredModel, MIN_CATALOG_REMOVAL_ISOLATION_MS,
        ModelCatalogTarget,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    fn target() -> Result<ModelCatalogTarget, Box<dyn Error>> {
        Ok(ModelCatalogTarget::new(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new("credential-a")?,
        ))
    }

    fn model(name: &str) -> Result<DiscoveredModel, Box<dyn Error>> {
        Ok(DiscoveredModel::try_new(name)?)
    }

    #[test]
    fn success_is_durable_monotonic_and_failure_preserves_last_success() -> TestResult {
        let store = SqliteCatalogSnapshotStore::open_in_memory()?;
        let target = target()?;
        let first = store.record_success("config-a", &target, [model("grok-4.6")?], 1_000)?;
        assert_eq!(first.snapshot().version(), 1);
        assert_eq!(first.freshness(), CatalogSnapshotFreshness::Fresh);

        assert!(store.record_failure(
            "config-a",
            &target,
            2_000,
            CatalogDiscoveryFailureClass::Transport,
        )?);
        let retained = store
            .status_at("config-a", &target, 2_000)?
            .ok_or("missing status")?;
        assert_eq!(retained.snapshot().version(), 1);
        assert_eq!(
            retained.last_failure(),
            Some((2_000, CatalogDiscoveryFailureClass::Transport))
        );

        let second = store.record_success(
            "config-a",
            &target,
            [model("grok-4.6")?, model("grok-4.5")?],
            3_000,
        )?;
        assert_eq!(second.snapshot().version(), 2);
        assert_eq!(second.last_failure(), None);
        assert_eq!(second.eligible_models().count(), 2);
        Ok(())
    }

    #[test]
    fn failure_before_first_success_is_visible_and_cannot_regress() -> TestResult {
        let store = SqliteCatalogSnapshotStore::open_in_memory()?;
        let target = target()?;
        assert!(store.record_failure(
            "config-a",
            &target,
            2_000,
            CatalogDiscoveryFailureClass::Transport,
        )?);
        assert!(!store.record_failure(
            "config-a",
            &target,
            1_000,
            CatalogDiscoveryFailureClass::Internal,
        )?);
        let failures = store.list_failures("config-a")?;
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].failed_at_ms(), 2_000);
        assert_eq!(failures[0].class(), CatalogDiscoveryFailureClass::Transport);
        assert!(store.status_at("config-a", &target, 2_000)?.is_none());
        Ok(())
    }

    #[test]
    fn removal_requires_three_successful_misses_and_twenty_four_hours() -> TestResult {
        let store = SqliteCatalogSnapshotStore::open_in_memory()?;
        let target = target()?;
        store.record_success("config-a", &target, [model("grok-4.6")?], 1_000)?;
        let first_miss = store.record_success("config-a", &target, [], 2_000)?;
        assert_eq!(first_miss.eligible_models().count(), 1);
        let second_miss = store.record_success("config-a", &target, [], 3_000)?;
        assert_eq!(second_miss.eligible_models().count(), 1);
        let too_early_third = store.record_success("config-a", &target, [], 4_000)?;
        assert_eq!(too_early_third.eligible_models().count(), 1);
        let removed = store.record_success(
            "config-a",
            &target,
            [],
            2_000 + MIN_CATALOG_REMOVAL_ISOLATION_MS,
        )?;
        assert_eq!(removed.eligible_models().count(), 0);
        Ok(())
    }

    #[test]
    fn persisted_deadlines_must_match_the_configured_policy() -> TestResult {
        let store = SqliteCatalogSnapshotStore::open_in_memory()?;
        let target = target()?;
        store.record_success("config-a", &target, [model("grok-4.6")?], 1_000)?;
        store.lock_connection()?.execute(
            "UPDATE model_catalog_targets
             SET stale_at_ms = stale_at_ms + 1
             WHERE config_version_id = 'config-a'",
            [],
        )?;

        assert!(store.status_at("config-a", &target, 1_000).is_err());
        Ok(())
    }
}
