//! Durable origin guards for edits forked from the active configuration.
use super::{
    ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration, ManagementAuditAction,
    ManagementAuditEventDraft, SqliteControlPlaneRepository, load_config_version,
    load_configuration,
};
use crate::{StoreError, StoreResult};
use rusqlite::{OptionalExtension, Transaction, params};

/// One active graph and the exact revisions against which its edit was started.
pub struct ConfigurationEditSource {
    /// Source graph; callers must re-seal version-bound secrets before writing a fork.
    pub configuration: ControlPlaneConfiguration,
    /// Durable origin metadata without retaining a second copy of encrypted resources.
    pub origin: ConfigurationEditOrigin,
}

/// Non-secret comparison data retained when the graph is moved into an edit.
pub struct ConfigurationEditOrigin {
    /// Original active version metadata.
    pub source_version: super::ConfigVersion,
    /// Changes including credential rotations on otherwise immutable active graphs.
    pub resource_sequence: i64,
    /// Publication/rollback sequence, excluding unrelated draft creation.
    pub lifecycle_sequence: i64,
}

fn sequences(transaction: &Transaction<'_>, version: &ConfigVersionId) -> StoreResult<(i64, i64)> {
    let resource = transaction.query_row("SELECT COALESCE(MAX(id), 0) FROM management_resource_audit_events WHERE config_version_id = ?1", [version.as_str()], |row| row.get(0))?;
    let lifecycle = transaction.query_row("SELECT COALESCE(MAX(id), 0) FROM management_audit_events WHERE action IN ('config_published', 'config_rolled_back')", [], |row| row.get(0))?;
    Ok((resource, lifecycle))
}

fn ensure_source(
    transaction: &Transaction<'_>,
    version: &ConfigVersionId,
    revision: i64,
    resource: i64,
    lifecycle: i64,
) -> StoreResult<()> {
    let source =
        load_config_version(transaction, version)?.ok_or(StoreError::ConfigVersionNotFound)?;
    if source.status != ConfigVersionStatus::Active
        || source.revision != revision
        || sequences(transaction, version)? != (resource, lifecycle)
    {
        return Err(StoreError::ConfigVersionRevisionConflict);
    }
    Ok(())
}

/// Checks the original active source inside the activation transaction.
pub(super) fn validate_origin(
    transaction: &Transaction<'_>,
    target: &ConfigVersionId,
) -> StoreResult<()> {
    let origin: Option<(String, i64, i64, i64)> = transaction.query_row(
        "SELECT source_version_id, source_revision, source_resource_sequence, source_lifecycle_sequence FROM configuration_edit_origins WHERE config_version_id = ?1",
        [target.as_str()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).optional()?;
    if let Some((source, revision, resource, lifecycle)) = origin {
        let source = ConfigVersionId::try_new(source)
            .map_err(|_| StoreError::ConfigVersionRevisionConflict)?;
        ensure_source(transaction, &source, revision, resource, lifecycle)?;
    }
    Ok(())
}

impl SqliteControlPlaneRepository {
    /// Reads an active edit source and both change clocks in one transaction.
    /// # Errors
    /// Rejects missing, non-active or stale source versions.
    pub fn configuration_edit_source(
        &mut self,
        version: &ConfigVersionId,
        expected_revision: i64,
    ) -> StoreResult<ConfigurationEditSource> {
        let transaction = self.connection.transaction()?;
        let configuration =
            load_configuration(&transaction, version)?.ok_or(StoreError::ConfigVersionNotFound)?;
        if configuration.version.status != ConfigVersionStatus::Active
            || configuration.version.revision != expected_revision
        {
            return Err(StoreError::ConfigVersionRevisionConflict);
        }
        let (resource_sequence, lifecycle_sequence) = sequences(&transaction, version)?;
        transaction.commit()?;
        Ok(ConfigurationEditSource {
            origin: ConfigurationEditOrigin {
                source_version: configuration.version.clone(),
                resource_sequence,
                lifecycle_sequence,
            },
            configuration,
        })
    }

    /// Persists a fully re-sealed fork, source guard and creation audit atomically.
    /// # Errors
    /// Rejects source changes during sealing, invalid target identity and partial graph writes.
    pub fn write_configuration_edit(
        &mut self,
        source: &ConfigurationEditOrigin,
        target: &ControlPlaneConfiguration,
        audit: &ManagementAuditEventDraft,
    ) -> StoreResult<()> {
        if target.version.status != ConfigVersionStatus::Draft
            || target.version.revision != 0
            || target.version.id == source.source_version.id
            || target.version.parent_id.as_ref() != Some(&source.source_version.id)
            || audit.action() != ManagementAuditAction::Created
        {
            return Err(StoreError::ConfigVersionRevisionConflict);
        }
        let mut transaction = self.begin_transaction()?;
        ensure_source(
            &transaction.transaction,
            &source.source_version.id,
            source.source_version.revision,
            source.resource_sequence,
            source.lifecycle_sequence,
        )?;
        transaction.write_configuration(target)?;
        // Preserve observation timestamps and removal evidence, not a fresh observation.
        // Re-sealing a configuration does not renew a provider's model entitlement.
        for (table, columns) in [
            (
                "model_catalog_targets",
                "endpoint_id, credential_id, snapshot_version, observed_at_ms, stale_at_ms, refresh_due_at_ms, expires_at_ms",
            ),
            (
                "model_catalog_models",
                "endpoint_id, credential_id, model, present_in_last_success, consecutive_successful_misses, first_missing_at_ms, removal_eligible_at_ms",
            ),
            (
                "model_catalog_failures",
                "endpoint_id, credential_id, failed_at_ms, failure_class",
            ),
        ] {
            transaction.transaction.execute(
                &format!("INSERT INTO {table} (config_version_id, {columns}) SELECT ?1, {columns} FROM {table} WHERE config_version_id = ?2"),
                params![target.version.id.as_str(),source.source_version.id.as_str()],
            )?;
        }
        transaction.transaction.execute(
            "INSERT INTO configuration_edit_origins (config_version_id, source_version_id, source_revision, source_resource_sequence, source_lifecycle_sequence) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![target.version.id.as_str(), source.source_version.id.as_str(), source.source_version.revision, source.resource_sequence, source.lifecycle_sequence],
        )?;
        transaction.record_management_audit_event(audit, target.version.id.clone(), None)?;
        transaction.commit()
    }
}
