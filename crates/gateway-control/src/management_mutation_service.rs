//! Versioned, audited draft-resource mutation boundary for the P10 management surface.
//!
//! This module is deliberately HTTP-free. It accepts typed resources only after the Actix
//! boundary has authenticated a management principal and decoded a bounded request. Every write
//! uses one exact Config Version revision and records the non-secret actor identity in the same
//! `SQLite` transaction. It never publishes a Snapshot, calls a Provider, or returns credential
//! plaintext/ciphertext.

use std::{error::Error, fmt, sync::Arc};

use gateway_core::{CredentialId, EgressPolicyId, EndpointId, UpstreamId};
use gateway_store::secret_store::SecretStoreError;
pub use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
pub use gateway_store::{
    StoreError,
    control_plane::{
        ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
        CredentialConfiguration, CredentialStatus, EgressPolicyConfiguration,
        EndpointConfiguration, EndpointCredentialBindingConfiguration, EndpointTransport,
        ManagementResourceAuditEvent, ManagementResourceAuditEventDraft,
        SqliteControlPlaneRepository, StoredEgressRedirectMode, UpstreamConfiguration,
    },
};

use crate::{
    control_plane_service::{ControlPlaneServiceError, credential_associated_data},
    management_service::{
        ManagementActor, ManagementClock, ManagementClockError, SystemManagementClock,
    },
};

/// Opaque monotonic Config Version token used for conditional management writes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConfigRevision(i64);

impl ConfigRevision {
    /// The revision assigned to a newly created draft graph before its first resource mutation.
    #[must_use]
    pub const fn initial() -> Self {
        Self(0)
    }

    /// Reconstructs a non-negative revision from the durable Version row.
    ///
    /// # Errors
    ///
    /// A negative value is rejected as malformed persisted control-plane state.
    pub fn try_new(value: i64) -> Result<Self, ManagementResourceError> {
        if value < 0 {
            return Err(ManagementResourceError::InvalidRevision);
        }
        Ok(Self(value))
    }

    /// Returns the internal non-negative value for the persistence boundary.
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        self.0
    }

    /// Renders the stable opaque HTTP token used in `If-Match` and `ETag` headers.
    #[must_use]
    pub fn as_token(self) -> String {
        format!("rev-{}", self.0)
    }

    /// Parses the exact opaque HTTP token format without accepting whitespace or alternate forms.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementResourceError::InvalidRevision`] when the token is not a canonical
    /// non-negative `rev-<decimal>` value.
    pub fn from_token(value: &str) -> Result<Self, ManagementResourceError> {
        let Some(decimal) = value.strip_prefix("rev-") else {
            return Err(ManagementResourceError::InvalidRevision);
        };
        if decimal.is_empty()
            || (decimal.len() > 1 && decimal.starts_with('0'))
            || !decimal.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ManagementResourceError::InvalidRevision);
        }
        let value = decimal
            .parse::<i64>()
            .map_err(|_| ManagementResourceError::InvalidRevision)?;
        Self::try_new(value)
    }
}

/// Result of one successful conditional resource mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Revisioned<T> {
    value: T,
    revision: ConfigRevision,
}

impl<T> Revisioned<T> {
    fn new(value: T, revision: ConfigRevision) -> Self {
        Self { value, revision }
    }

    /// Returns the non-secret resource view.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Returns the next revision required by a later mutation.
    #[must_use]
    pub const fn revision(&self) -> ConfigRevision {
        self.revision
    }

    /// Consumes the result into its resource and next revision.
    #[must_use]
    pub fn into_parts(self) -> (T, ConfigRevision) {
        (self.value, self.revision)
    }
}

/// Secret-free Credential data eligible for a management response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialView {
    /// Stable credential identity.
    pub id: CredentialId,
    /// Owning upstream identity.
    pub upstream_id: UpstreamId,
    /// Non-secret credential family.
    pub kind: String,
    /// Stored lifecycle status. A revoked management request is retained as disabled because the
    /// existing P2 credential graph has no third persistent revoked state.
    pub status: CredentialStatus,
    /// Per-credential record revision.
    pub revision: i64,
    /// Always true for a persisted credential; plaintext is never returned.
    pub secret_present: bool,
}

/// One management Credential create or replacement request.
///
/// The plaintext Secret is borrowed for immediate AEAD sealing only. It is never retained in this
/// request type, returned by the service, or included in audit metadata.
pub struct CredentialUpsert<'secret> {
    /// Stable Credential identity.
    pub id: CredentialId,
    /// Non-secret Credential family.
    pub kind: String,
    /// Plaintext to seal immediately under the Version/Credential binding.
    pub plaintext_secret: &'secret [u8],
    /// Requested durable lifecycle status.
    pub status: CredentialStatus,
}

#[derive(Clone, Copy)]
struct ResourceAction<'action> {
    action: &'action str,
    resource_kind: &'action str,
    resource_id: &'action str,
}

/// Management-only resource service. It is not a request-path handle.
pub struct ManagementMutationService {
    repository: SqliteControlPlaneRepository,
    secret_store: SecretStore,
    clock: Arc<dyn ManagementClock>,
}

impl ManagementMutationService {
    /// Creates the service from an owned Repository and externally supplied Secret Store.
    #[must_use]
    pub fn new(repository: SqliteControlPlaneRepository, secret_store: SecretStore) -> Self {
        Self::with_clock(repository, secret_store, Arc::new(SystemManagementClock))
    }

    /// Creates the service with a deterministic clock for tests or a controlled embedding.
    #[must_use]
    pub fn with_clock(
        repository: SqliteControlPlaneRepository,
        secret_store: SecretStore,
        clock: Arc<dyn ManagementClock>,
    ) -> Self {
        Self {
            repository,
            secret_store,
            clock,
        }
    }

    /// Returns one Version-scoped Egress Policy if present, together with the current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Policy is absent, or its persisted revision is invalid.
    pub fn get_egress_policy(
        &mut self,
        config_version_id: &ConfigVersionId,
        egress_policy_id: &EgressPolicyId,
    ) -> Result<Revisioned<EgressPolicyConfiguration>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let revision = ConfigRevision::try_new(configuration.version.revision)?;
        let policy = configuration
            .egress_policies
            .into_iter()
            .find(|candidate| &candidate.id == egress_policy_id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        Ok(Revisioned::new(policy, revision))
    }

    /// Returns all Egress Policies in a Version, sorted by their persisted identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is absent or its persisted revision is invalid.
    pub fn list_egress_policies(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<Revisioned<Vec<EgressPolicyConfiguration>>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        Ok(Revisioned::new(
            configuration.egress_policies,
            ConfigRevision::try_new(configuration.version.revision)?,
        ))
    }

    /// Creates one Egress Policy using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Policy is invalid, or the resource/audit transaction cannot commit.
    pub fn create_egress_policy(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        policy: EgressPolicyConfiguration,
    ) -> Result<Revisioned<EgressPolicyConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "egress_policy_created",
            actor,
            config_version_id,
            "egress_policy",
            policy.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_egress_policy(config_version_id, &policy)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            policy,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Updates one Egress Policy using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Policy is absent, or the resource/audit transaction cannot commit.
    pub fn update_egress_policy(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        policy: EgressPolicyConfiguration,
    ) -> Result<Revisioned<EgressPolicyConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "egress_policy_updated",
            actor,
            config_version_id,
            "egress_policy",
            policy.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.update_egress_policy(config_version_id, &policy)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            policy,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Deletes one Egress Policy with an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Policy is absent, or the resource/audit transaction cannot commit.
    pub fn delete_egress_policy(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        egress_policy_id: &EgressPolicyId,
    ) -> Result<ConfigRevision, ManagementResourceError> {
        self.delete_resource(
            actor,
            config_version_id,
            expected_revision,
            ResourceAction {
                action: "egress_policy_deleted",
                resource_kind: "egress_policy",
                resource_id: egress_policy_id.as_str(),
            },
            |transaction| transaction.delete_egress_policy(config_version_id, egress_policy_id),
        )
    }

    /// Returns one Version-scoped Upstream if present, together with the current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Upstream is absent, or its persisted revision is invalid.
    pub fn get_upstream(
        &mut self,
        config_version_id: &ConfigVersionId,
        upstream_id: &UpstreamId,
    ) -> Result<Revisioned<UpstreamConfiguration>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let revision = ConfigRevision::try_new(configuration.version.revision)?;
        let upstream = configuration
            .upstreams
            .into_iter()
            .find(|candidate| &candidate.id == upstream_id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        Ok(Revisioned::new(upstream, revision))
    }

    /// Returns every Upstream in one Version together with its current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is absent or its persisted revision is invalid.
    pub fn list_upstreams(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<Revisioned<Vec<UpstreamConfiguration>>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        Ok(Revisioned::new(
            configuration.upstreams,
            ConfigRevision::try_new(configuration.version.revision)?,
        ))
    }

    /// Creates one Upstream using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, a
    /// referenced Egress Policy is absent, or the resource/audit transaction cannot commit.
    pub fn create_upstream(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        upstream: UpstreamConfiguration,
    ) -> Result<Revisioned<UpstreamConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "upstream_created",
            actor,
            config_version_id,
            "upstream",
            upstream.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_upstream(config_version_id, &upstream)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            upstream,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Updates one Upstream using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Upstream is absent, or the resource/audit transaction cannot commit.
    pub fn update_upstream(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        upstream: UpstreamConfiguration,
    ) -> Result<Revisioned<UpstreamConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "upstream_updated",
            actor,
            config_version_id,
            "upstream",
            upstream.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.update_upstream(config_version_id, &upstream)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            upstream,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Deletes one Upstream with its schema-owned descendants using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Upstream is absent, or the resource/audit transaction cannot commit.
    pub fn delete_upstream(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        upstream_id: &UpstreamId,
    ) -> Result<ConfigRevision, ManagementResourceError> {
        self.delete_resource(
            actor,
            config_version_id,
            expected_revision,
            ResourceAction {
                action: "upstream_deleted",
                resource_kind: "upstream",
                resource_id: upstream_id.as_str(),
            },
            |transaction| transaction.delete_upstream(config_version_id, upstream_id),
        )
    }

    /// Creates one Endpoint under its explicit Upstream using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// owning Upstream is absent, or the resource/audit transaction cannot commit.
    pub fn create_endpoint(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        endpoint: EndpointConfiguration,
    ) -> Result<Revisioned<EndpointConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "endpoint_created",
            actor,
            config_version_id,
            "endpoint",
            endpoint.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_endpoint(config_version_id, &endpoint)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            endpoint,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Returns one Endpoint and the current Version revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Endpoint is absent, or its persisted revision is invalid.
    pub fn get_endpoint(
        &mut self,
        config_version_id: &ConfigVersionId,
        endpoint_id: &EndpointId,
    ) -> Result<Revisioned<EndpointConfiguration>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let revision = ConfigRevision::try_new(configuration.version.revision)?;
        let endpoint = configuration
            .endpoints
            .into_iter()
            .find(|candidate| &candidate.id == endpoint_id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        Ok(Revisioned::new(endpoint, revision))
    }

    /// Updates an Endpoint in place; it cannot silently move between Upstreams.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Endpoint is absent, or the resource/audit transaction cannot commit.
    pub fn update_endpoint(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        endpoint: EndpointConfiguration,
    ) -> Result<Revisioned<EndpointConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "endpoint_updated",
            actor,
            config_version_id,
            "endpoint",
            endpoint.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.update_endpoint(config_version_id, &endpoint)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            endpoint,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Deletes one Endpoint with its schema-owned descendants using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Endpoint is absent, or the resource/audit transaction cannot commit.
    pub fn delete_endpoint(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        endpoint_id: &EndpointId,
    ) -> Result<ConfigRevision, ManagementResourceError> {
        self.delete_resource(
            actor,
            config_version_id,
            expected_revision,
            ResourceAction {
                action: "endpoint_deleted",
                resource_kind: "endpoint",
                resource_id: endpoint_id.as_str(),
            },
            |transaction| transaction.delete_endpoint(config_version_id, endpoint_id),
        )
    }

    /// Creates one opaque Credential and returns only safe metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if AEAD sealing fails, the Version is not an admitted draft at
    /// `expected_revision`, the Upstream is absent, or the resource/audit transaction cannot commit.
    pub fn create_credential(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        upstream_id: UpstreamId,
        input: CredentialUpsert<'_>,
    ) -> Result<Revisioned<CredentialView>, ManagementResourceError> {
        let credential = self.seal_credential(config_version_id, upstream_id, input, 0)?;
        let audit = self.audit(
            "credential_created",
            actor,
            config_version_id,
            "credential",
            credential.id.as_str(),
        )?;
        let credential_id = credential.id.clone();
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_credential(config_version_id, &credential)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            self.credential_view(config_version_id, &credential_id)?,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Returns safe metadata for one persisted Credential and the current Version revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Credential is absent, or its persisted revision is invalid.
    pub fn get_credential(
        &mut self,
        config_version_id: &ConfigVersionId,
        credential_id: &CredentialId,
    ) -> Result<Revisioned<CredentialView>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let revision = ConfigRevision::try_new(configuration.version.revision)?;
        let credential = configuration
            .credentials
            .into_iter()
            .find(|candidate| &candidate.id == credential_id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        Ok(Revisioned::new(CredentialView::from(credential), revision))
    }

    /// Re-seals and replaces one Credential while preserving its owning Upstream identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the Credential is absent, AEAD sealing fails, the Version is not an
    /// admitted draft at `expected_revision`, or the resource/audit transaction cannot commit.
    pub fn update_credential(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        input: CredentialUpsert<'_>,
    ) -> Result<Revisioned<CredentialView>, ManagementResourceError> {
        let current = self
            .configuration(config_version_id)?
            .credentials
            .into_iter()
            .find(|candidate| candidate.id == input.id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        let credential = self.seal_credential(
            config_version_id,
            current.upstream_id,
            input,
            current
                .revision
                .checked_add(1)
                .ok_or(ManagementResourceError::InvalidRevision)?,
        )?;
        let audit = self.audit(
            "credential_updated",
            actor,
            config_version_id,
            "credential",
            credential.id.as_str(),
        )?;
        let credential_id = credential.id.clone();
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.update_credential(config_version_id, &credential)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            self.credential_view(config_version_id, &credential_id)?,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Deletes one Credential with its schema-owned bindings using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Credential is absent, or the resource/audit transaction cannot commit.
    pub fn delete_credential(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        credential_id: &CredentialId,
    ) -> Result<ConfigRevision, ManagementResourceError> {
        self.delete_resource(
            actor,
            config_version_id,
            expected_revision,
            ResourceAction {
                action: "credential_deleted",
                resource_kind: "credential",
                resource_id: credential_id.as_str(),
            },
            |transaction| transaction.delete_credential(config_version_id, credential_id),
        )
    }

    /// Lists endpoint-local Credential bindings together with the current Version revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Endpoint is absent, or its persisted revision is invalid.
    pub fn list_endpoint_credential_bindings(
        &mut self,
        config_version_id: &ConfigVersionId,
        endpoint_id: &EndpointId,
    ) -> Result<Revisioned<Vec<EndpointCredentialBindingConfiguration>>, ManagementResourceError>
    {
        let configuration = self.configuration(config_version_id)?;
        if !configuration
            .endpoints
            .iter()
            .any(|candidate| &candidate.id == endpoint_id)
        {
            return Err(ManagementResourceError::ResourceNotFound);
        }
        let bindings = configuration
            .endpoint_credential_bindings
            .into_iter()
            .filter(|candidate| &candidate.endpoint_id == endpoint_id)
            .collect();
        Ok(Revisioned::new(
            bindings,
            ConfigRevision::try_new(configuration.version.revision)?,
        ))
    }

    /// Creates one exact Endpoint/Credential binding using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, a bound
    /// resource is absent, or the resource/audit transaction cannot commit.
    pub fn create_endpoint_credential_binding(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        binding: EndpointCredentialBindingConfiguration,
    ) -> Result<Revisioned<EndpointCredentialBindingConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "endpoint_credential_binding_created",
            actor,
            config_version_id,
            "endpoint_credential_binding",
            binding.credential_id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_endpoint_credential_binding(config_version_id, &binding)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            binding,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Returns the owned Repository only to an explicitly management-time caller.
    #[must_use]
    pub fn repository_mut(&mut self) -> &mut SqliteControlPlaneRepository {
        &mut self.repository
    }

    /// Returns append-only resource mutation audit records for a later protected audit page.
    ///
    /// # Errors
    ///
    /// Returns an error if durable audit records cannot be decoded or loaded.
    pub fn resource_audit_events(
        &mut self,
    ) -> Result<Vec<ManagementResourceAuditEvent>, ManagementResourceError> {
        Ok(self.repository.list_management_resource_audit_events()?)
    }

    /// Records a bounded non-graph resource operation such as OAuth cancellation.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is absent, audit metadata is invalid, or the append-only
    /// audit write cannot commit.
    pub fn record_resource_action(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        action: &str,
        resource_kind: &str,
        resource_id: &str,
    ) -> Result<(), ManagementResourceError> {
        let audit = self.audit(action, actor, config_version_id, resource_kind, resource_id)?;
        self.repository
            .record_management_resource_audit_event(config_version_id, &audit)?;
        Ok(())
    }

    /// Records a bounded graph-affecting operation such as Catalog application under an exact
    /// draft revision, even when its durable Catalog state is owned by another P4 runtime store.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, audit
    /// metadata is invalid, or the audit/revision transaction cannot commit.
    pub fn record_draft_resource_action(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        action: &str,
        resource_kind: &str,
        resource_id: &str,
    ) -> Result<ConfigRevision, ManagementResourceError> {
        let audit = self.audit(action, actor, config_version_id, resource_kind, resource_id)?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        ConfigRevision::try_new(next_revision)
    }

    fn delete_resource<F>(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        action: ResourceAction<'_>,
        delete: F,
    ) -> Result<ConfigRevision, ManagementResourceError>
    where
        F: FnOnce(
            &mut gateway_store::control_plane::ControlPlaneTransaction<'_>,
        ) -> Result<(), StoreError>,
    {
        let audit = self.audit(
            action.action,
            actor,
            config_version_id,
            action.resource_kind,
            action.resource_id,
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                delete(transaction)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        ConfigRevision::try_new(next_revision)
    }

    fn seal_credential(
        &self,
        config_version_id: &ConfigVersionId,
        upstream_id: UpstreamId,
        input: CredentialUpsert<'_>,
        revision: i64,
    ) -> Result<CredentialConfiguration, ManagementResourceError> {
        if input.plaintext_secret.is_empty() || revision < 0 {
            return Err(ManagementResourceError::InvalidCredentialInput);
        }
        let associated_data =
            credential_associated_data(config_version_id, &input.id, &upstream_id)?;
        let encrypted_secret = self
            .secret_store
            .seal(input.plaintext_secret, &associated_data)?;
        Ok(CredentialConfiguration {
            id: input.id,
            upstream_id,
            kind: input.kind,
            encrypted_secret,
            status: input.status,
            revision,
        })
    }

    fn credential_view(
        &mut self,
        config_version_id: &ConfigVersionId,
        credential_id: &CredentialId,
    ) -> Result<CredentialView, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        configuration
            .credentials
            .into_iter()
            .find(|candidate| &candidate.id == credential_id)
            .map(CredentialView::from)
            .ok_or(ManagementResourceError::ResourceNotFound)
    }

    fn configuration(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<gateway_store::control_plane::ControlPlaneConfiguration, ManagementResourceError>
    {
        self.repository
            .load_configuration(config_version_id)?
            .ok_or(ManagementResourceError::ConfigVersionNotFound)
    }

    fn audit(
        &self,
        action: &str,
        actor: &ManagementActor,
        _config_version_id: &ConfigVersionId,
        resource_kind: &str,
        resource_id: &str,
    ) -> Result<ManagementResourceAuditEventDraft, ManagementResourceError> {
        Ok(ManagementResourceAuditEventDraft::try_new(
            action,
            actor.as_str(),
            self.clock.now_ms()?,
            resource_kind,
            resource_id,
        )?)
    }
}

impl From<CredentialConfiguration> for CredentialView {
    fn from(value: CredentialConfiguration) -> Self {
        Self {
            id: value.id,
            upstream_id: value.upstream_id,
            kind: value.kind,
            status: value.status,
            revision: value.revision,
            secret_present: true,
        }
    }
}

impl fmt::Debug for ManagementMutationService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagementMutationService")
            .field("repository", &self.repository)
            .field("secret_store", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Safe failure classes for versioned P10 resource mutations.
#[derive(Debug)]
pub enum ManagementResourceError {
    /// A durable store operation failed without exposing values.
    Store(StoreError),
    /// A credential seal operation failed without exposing plaintext or ciphertext.
    SecretStore(SecretStoreError),
    /// Stable credential associated-data construction failed.
    ControlPlane(ControlPlaneServiceError),
    /// The management clock could not safely provide an audit timestamp.
    Clock(ManagementClockError),
    /// The selected Config Version was absent.
    ConfigVersionNotFound,
    /// The selected Version-scoped resource was absent.
    ResourceNotFound,
    /// A negative or overflowed configuration revision was rejected.
    InvalidRevision,
    /// A credential mutation had no plaintext Secret or an invalid record revision.
    InvalidCredentialInput,
}

impl fmt::Display for ManagementResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "management resource storage failed: {error}"),
            Self::SecretStore(error) => {
                write!(formatter, "management credential sealing failed: {error}")
            }
            Self::ControlPlane(error) => {
                write!(formatter, "management credential boundary failed: {error}")
            }
            Self::Clock(error) => write!(formatter, "management clock failed: {error}"),
            Self::ConfigVersionNotFound => {
                formatter.write_str("management Config Version was not found")
            }
            Self::ResourceNotFound => formatter.write_str("management resource was not found"),
            Self::InvalidRevision => {
                formatter.write_str("management Config Version revision is invalid")
            }
            Self::InvalidCredentialInput => {
                formatter.write_str("management credential input is invalid")
            }
        }
    }
}

impl Error for ManagementResourceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::SecretStore(error) => Some(error),
            Self::ControlPlane(error) => Some(error),
            Self::Clock(error) => Some(error),
            Self::ConfigVersionNotFound
            | Self::ResourceNotFound
            | Self::InvalidRevision
            | Self::InvalidCredentialInput => None,
        }
    }
}

impl From<StoreError> for ManagementResourceError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

impl From<SecretStoreError> for ManagementResourceError {
    fn from(value: SecretStoreError) -> Self {
        Self::SecretStore(value)
    }
}

impl From<ControlPlaneServiceError> for ManagementResourceError {
    fn from(value: ControlPlaneServiceError) -> Self {
        Self::ControlPlane(value)
    }
}

impl From<ManagementClockError> for ManagementResourceError {
    fn from(value: ManagementClockError) -> Self {
        Self::Clock(value)
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, sync::Arc};

    use gateway_core::{CredentialId, EgressPolicyId, EndpointId, UpstreamId};
    use gateway_store::{
        StoreError,
        control_plane::{
            ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialStatus, EgressPolicyConfiguration, EndpointConfiguration,
            EndpointCredentialBindingConfiguration, EndpointTransport,
            SqliteControlPlaneRepository, StoredEgressRedirectMode, UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };

    use crate::management_service::{ManagementActor, ManagementClock, ManagementClockError};

    use super::{
        ConfigRevision, CredentialUpsert, CredentialView, ManagementMutationService,
        ManagementResourceError, Revisioned,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[derive(Debug)]
    struct FixedClock;

    impl ManagementClock for FixedClock {
        fn now_ms(&self) -> Result<i64, ManagementClockError> {
            Ok(42)
        }
    }

    #[test]
    fn resource_mutations_are_revision_guarded_audited_and_secret_free() -> TestResult {
        let (mut service, version_id, actor) = test_service()?;

        let policy = create_egress_policy(&mut service, &actor, &version_id)?;
        assert_eq!(policy.revision().as_i64(), 1);

        assert_stale_revision_rejected(&mut service, &actor, &version_id)?;
        assert!(service.list_upstreams(&version_id)?.value().is_empty());

        let upstream = create_upstream(&mut service, &actor, &version_id, policy.revision())?;
        let endpoint = create_endpoint(&mut service, &actor, &version_id, upstream.revision())?;
        let credential = create_credential(&mut service, &actor, &version_id, endpoint.revision())?;
        assert!(credential.value().secret_present);
        assert_eq!(credential.value().revision, 0);
        assert!(!format!("{:?}", credential.value()).contains("test-secret-not-returned"));

        let binding = create_binding(&mut service, &actor, &version_id, credential.revision())?;
        assert_eq!(binding.revision().as_i64(), 5);
        assert_eq!(
            service
                .list_endpoint_credential_bindings(
                    &version_id,
                    &EndpointId::try_new("endpoint-a")?
                )?
                .value()
                .len(),
            1
        );

        let catalog_revision = service.record_draft_resource_action(
            &actor,
            &version_id,
            binding.revision(),
            "catalog_discovery_applied",
            "endpoint",
            "endpoint-a",
        )?;
        assert_eq!(catalog_revision.as_i64(), 6);
        service.record_resource_action(
            &actor,
            &version_id,
            "credential_oauth_cancelled",
            "credential",
            "credential-a",
        )?;

        assert_audit_events(&mut service)?;
        Ok(())
    }

    fn test_service()
    -> Result<(ManagementMutationService, ConfigVersionId, ManagementActor), Box<dyn Error>> {
        let version_id = ConfigVersionId::try_new("draft-a")?;
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
            id: version_id.clone(),
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 1,
            description: "management mutation test".to_owned(),
        }))?;
        let key_version = KeyVersion::try_new(1)?;
        let key_ring = MasterKeyRing::try_new(
            key_version,
            [(key_version, MasterKey::try_from_bytes([0x71_u8; 32])?)],
        )?;
        let service = ManagementMutationService::with_clock(
            repository,
            SecretStore::new(key_ring),
            Arc::new(FixedClock),
        );
        let actor = ManagementActor::try_new("management-key")?;
        Ok((service, version_id, actor))
    }

    fn create_egress_policy(
        service: &mut ManagementMutationService,
        actor: &ManagementActor,
        version_id: &ConfigVersionId,
    ) -> Result<Revisioned<EgressPolicyConfiguration>, Box<dyn Error>> {
        Ok(service.create_egress_policy(
            actor,
            version_id,
            ConfigRevision::initial(),
            EgressPolicyConfiguration {
                id: EgressPolicyId::try_new("policy-a")?,
                name: "allow-provider".to_owned(),
                allowed_schemes_json: "[\"https\"]".to_owned(),
                allowed_hosts_json: "[\"api.example.test\"]".to_owned(),
                allowed_ports_json: "[443]".to_owned(),
                allowed_cidrs_json: "[]".to_owned(),
                redirect_mode: StoredEgressRedirectMode::Deny,
                max_redirects: 0,
            },
        )?)
    }

    fn assert_stale_revision_rejected(
        service: &mut ManagementMutationService,
        actor: &ManagementActor,
        version_id: &ConfigVersionId,
    ) -> TestResult {
        let stale = service.create_upstream(
            actor,
            version_id,
            ConfigRevision::initial(),
            upstream("upstream-stale", None)?,
        );
        assert!(matches!(
            stale,
            Err(ManagementResourceError::Store(
                StoreError::ConfigVersionRevisionConflict
            ))
        ));
        Ok(())
    }

    fn create_upstream(
        service: &mut ManagementMutationService,
        actor: &ManagementActor,
        version_id: &ConfigVersionId,
        revision: ConfigRevision,
    ) -> Result<Revisioned<UpstreamConfiguration>, Box<dyn Error>> {
        Ok(service.create_upstream(
            actor,
            version_id,
            revision,
            upstream("upstream-a", Some("policy-a"))?,
        )?)
    }

    fn create_endpoint(
        service: &mut ManagementMutationService,
        actor: &ManagementActor,
        version_id: &ConfigVersionId,
        revision: ConfigRevision,
    ) -> Result<Revisioned<EndpointConfiguration>, Box<dyn Error>> {
        Ok(service.create_endpoint(
            actor,
            version_id,
            revision,
            EndpointConfiguration {
                id: EndpointId::try_new("endpoint-a")?,
                upstream_id: UpstreamId::try_new("upstream-a")?,
                adapter_id: "openai-compatible.responses".to_owned(),
                api_format: "openai/responses".to_owned(),
                base_url: "https://api.example.test/v1".to_owned(),
                inference_path: "/responses".to_owned(),
                models_path: Some("/models".to_owned()),
                transport: EndpointTransport::Http,
                enabled: true,
            },
        )?)
    }

    fn create_credential(
        service: &mut ManagementMutationService,
        actor: &ManagementActor,
        version_id: &ConfigVersionId,
        revision: ConfigRevision,
    ) -> Result<Revisioned<CredentialView>, Box<dyn Error>> {
        Ok(service.create_credential(
            actor,
            version_id,
            revision,
            UpstreamId::try_new("upstream-a")?,
            CredentialUpsert {
                id: CredentialId::try_new("credential-a")?,
                kind: "api_key".to_owned(),
                plaintext_secret: b"test-secret-not-returned",
                status: CredentialStatus::Active,
            },
        )?)
    }

    fn create_binding(
        service: &mut ManagementMutationService,
        actor: &ManagementActor,
        version_id: &ConfigVersionId,
        revision: ConfigRevision,
    ) -> Result<Revisioned<EndpointCredentialBindingConfiguration>, Box<dyn Error>> {
        Ok(service.create_endpoint_credential_binding(
            actor,
            version_id,
            revision,
            EndpointCredentialBindingConfiguration {
                endpoint_id: EndpointId::try_new("endpoint-a")?,
                credential_id: CredentialId::try_new("credential-a")?,
                upstream_id: UpstreamId::try_new("upstream-a")?,
                enabled: true,
                priority: 0,
                weight: 1,
                concurrency: 1,
            },
        )?)
    }

    fn assert_audit_events(service: &mut ManagementMutationService) -> TestResult {
        let audit_events = service.resource_audit_events()?;
        assert_eq!(audit_events.len(), 7);
        assert!(
            audit_events
                .iter()
                .all(|event| event.actor() == "management-key")
        );
        assert_eq!(audit_events[3].action(), "credential_created");
        assert_eq!(audit_events[3].resource_kind(), "credential");
        assert_eq!(audit_events[3].resource_id(), "credential-a");
        assert_eq!(audit_events[5].action(), "catalog_discovery_applied");
        assert_eq!(audit_events[6].action(), "credential_oauth_cancelled");
        Ok(())
    }

    fn upstream(
        id: &str,
        egress_policy_id: Option<&str>,
    ) -> Result<UpstreamConfiguration, Box<dyn Error>> {
        Ok(UpstreamConfiguration {
            id: UpstreamId::try_new(id)?,
            name: format!("name-{id}"),
            kind: "openai-compatible".to_owned(),
            enabled: true,
            tags_json: "[\"test\"]".to_owned(),
            egress_policy_id: egress_policy_id.map(EgressPolicyId::try_new).transpose()?,
        })
    }
}
