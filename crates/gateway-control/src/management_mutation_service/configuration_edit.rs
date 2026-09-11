//! Clone active configuration without exporting version-bound secrets.
use super::{
    ConfigRevision, ConfigVersion, ConfigVersionId, ConfigVersionStatus, ManagementActor,
    ManagementMutationService, ManagementResourceError,
};
use crate::control_plane_service::{
    compatible_proxy_node_associated_data, credential_associated_data,
};
use gateway_store::control_plane::{ManagementAuditAction, ManagementAuditEventDraft};

impl ManagementMutationService {
    /// Starts an edit from the complete active graph and preserves every resource identity.
    /// Credentials and proxy endpoints are opened only to re-seal them for the new version.
    /// # Errors
    /// Rejects stale source revisions, secret binding failures and concurrent source changes.
    pub fn fork_active_configuration(
        &mut self,
        actor: &ManagementActor,
        source_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        target_id: ConfigVersionId,
        description: String,
    ) -> Result<ConfigVersion, ManagementResourceError> {
        let source = self
            .repository
            .configuration_edit_source(source_id, expected_revision.as_i64())?;
        let mut target = source.configuration;
        let now = self.clock.now_ms()?;
        target.version = ConfigVersion {
            id: target_id,
            parent_id: Some(source_id.clone()),
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: now,
            description,
        };
        for credential in &mut target.credentials {
            let old_aad =
                credential_associated_data(source_id, &credential.id, &credential.upstream_id)?;
            let new_aad = credential_associated_data(
                &target.version.id,
                &credential.id,
                &credential.upstream_id,
            )?;
            let plaintext = self
                .secret_store
                .open(&credential.encrypted_secret, &old_aad)?;
            credential.encrypted_secret = self.secret_store.seal(plaintext.as_bytes(), &new_aad)?;
        }
        for node in &mut target.compatible_proxy_nodes {
            let old_aad = compatible_proxy_node_associated_data(
                source_id,
                &node.upstream_id,
                node.pool_id.as_ref(),
                &node.id,
            )?;
            let new_aad = compatible_proxy_node_associated_data(
                &target.version.id,
                &node.upstream_id,
                node.pool_id.as_ref(),
                &node.id,
            )?;
            let plaintext = self.secret_store.open(&node.encrypted_proxy, &old_aad)?;
            node.encrypted_proxy = self.secret_store.seal(plaintext.as_bytes(), &new_aad)?;
        }
        let audit = ManagementAuditEventDraft::try_new(
            ManagementAuditAction::Created,
            actor.as_str(),
            now,
        )?;
        self.repository
            .write_configuration_edit(&source.origin, &target, &audit)?;
        Ok(target.version)
    }
}
