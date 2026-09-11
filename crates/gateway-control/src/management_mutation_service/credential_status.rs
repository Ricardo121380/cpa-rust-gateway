//! Administrative status changes retain encrypted account material.
use super::{
    ConfigRevision, ConfigVersionId, CredentialId, CredentialStatus, CredentialView,
    ManagementActor, ManagementMutationService, ManagementResourceError, Revisioned,
};

impl ManagementMutationService {
    /// Changes one draft account's administrative status without reading its plaintext.
    /// # Errors
    /// Rejects stale configuration/account revisions and non-administrative target states.
    pub fn set_credential_status(
        &mut self,
        actor: &ManagementActor,
        version: &ConfigVersionId,
        expected: ConfigRevision,
        id: &CredentialId,
        credential_revision: i64,
        status: CredentialStatus,
    ) -> Result<Revisioned<CredentialView>, ManagementResourceError> {
        if !matches!(
            status,
            CredentialStatus::Active | CredentialStatus::Disabled
        ) {
            return Err(ManagementResourceError::InvalidCredentialInput);
        }
        let mut credential = self
            .configuration(version)?
            .credentials
            .into_iter()
            .find(|entry| &entry.id == id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        if credential.revision != credential_revision {
            return Err(ManagementResourceError::CredentialRevisionConflict);
        }
        credential.status = status;
        credential.revision = credential
            .revision
            .checked_add(1)
            .ok_or(ManagementResourceError::InvalidRevision)?;
        let audit = self.audit(
            "credential_status_updated",
            actor,
            version,
            "credential",
            id.as_str(),
        )?;
        let ((), revision) = self.repository.mutate_draft_configuration(
            version,
            expected.as_i64(),
            |transaction| {
                transaction.update_credential(version, &credential)?;
                transaction.record_management_resource_audit_event(&audit, version)
            },
        )?;
        Ok(Revisioned::new(
            self.credential_view(version, id)?,
            ConfigRevision::try_new(revision)?,
        ))
    }
}
