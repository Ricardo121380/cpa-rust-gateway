//! Versioned, audited draft-resource mutation boundary for the P10 management surface.
//!
//! This module is deliberately HTTP-free. It accepts typed resources only after the Actix
//! boundary has authenticated a management principal and decoded a bounded request. Every write
//! uses one exact Config Version revision and records the non-secret actor identity in the same
//! `SQLite` transaction. It never publishes a Snapshot, calls a Provider, or returns credential
//! plaintext/ciphertext.

use std::{error::Error, fmt, sync::Arc};

use gateway_auth::client_key::{
    ClientKeyError, ClientKeyService, ClientKeyStatus as IssuedClientKeyStatus, PresentedClientKey,
};
use gateway_core::{
    AccessGroupId, ClientKeyId, CredentialId, EgressPolicyId, EndpointId, PublicModelId, RouteId,
    UpstreamId,
};
pub use gateway_store::billing_ledger::{
    BillingCatalogSource, BillingPriceCatalog, BillingPriceEntry,
};
use gateway_store::secret_store::SecretStoreError;
pub use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
pub use gateway_store::{
    StoreError,
    control_plane::{
        AccessGroupConfiguration, AccessGroupRouteConfiguration, AdministrativeStatus,
        ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
        CredentialConfiguration, CredentialScope, CredentialStatus, EgressPolicyConfiguration,
        EndpointConfiguration, EndpointCredentialBindingConfiguration, EndpointTransport,
        ManagementResourceAuditEvent, ManagementResourceAuditEventDraft, ModelAliasConfiguration,
        ModelRouteConfiguration, PublicModelConfiguration, RouteCandidateConfiguration,
        RoutePolicy, SqliteControlPlaneRepository, StoredClientKey, StoredClientKeyStatus,
        StoredEgressRedirectMode, TransformMode, UpstreamConfiguration,
    },
};

use crate::{
    control_plane_service::{ControlPlaneServiceError, credential_associated_data},
    management_operations_service::{
        ManagementOperationsError, OperationalAccountPoolPage, OperationalAccountPoolQuery,
        compile_operational_account_pool_page,
    },
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

/// Maximum immutable billing catalog versions returned by one management read.
pub const MAX_MANAGEMENT_BILLING_CATALOGS: usize = 256;

/// Validated operator/import boundary before the service assigns durable creation time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingCatalogImport {
    /// New immutable catalog version identity.
    pub catalog_version_id: String,
    /// Inclusive timestamp at which future Usage can select this version.
    pub effective_at_ms: u64,
    /// Operator or reviewed-import provenance; test provenance is rejected by the service.
    pub source: BillingCatalogSource,
    /// Provider/Channel/public-Model integer rates.
    pub entries: Vec<BillingPriceEntry>,
}

/// Operation recorded by a protected billing catalog write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BillingCatalogMutationOperation {
    /// A new catalog was imported or manually entered.
    Imported,
    /// A new immutable catalog was created from a retained predecessor.
    RolledBack,
}

/// Secret-free receipt returned after one atomic billing catalog mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingCatalogMutationReceipt {
    catalog_version_id: String,
    effective_at_ms: u64,
    source: BillingCatalogSource,
    entry_count: usize,
    operation: BillingCatalogMutationOperation,
    rolled_back_from: Option<String>,
}

impl BillingCatalogMutationReceipt {
    fn new(
        catalog: &BillingPriceCatalog,
        operation: BillingCatalogMutationOperation,
        rolled_back_from: Option<String>,
    ) -> Self {
        Self {
            catalog_version_id: catalog.catalog_version_id.clone(),
            effective_at_ms: catalog.effective_at_ms,
            source: catalog.source,
            entry_count: catalog.entries.len(),
            operation,
            rolled_back_from,
        }
    }

    /// Returns the newly durable catalog version identity.
    #[must_use]
    pub fn catalog_version_id(&self) -> &str {
        &self.catalog_version_id
    }

    /// Returns when this catalog becomes effective for future quotes.
    #[must_use]
    pub const fn effective_at_ms(&self) -> u64 {
        self.effective_at_ms
    }

    /// Returns the non-secret catalog source classification.
    #[must_use]
    pub const fn source(&self) -> BillingCatalogSource {
        self.source
    }

    /// Returns the number of Provider/Channel/Model entries written.
    #[must_use]
    pub const fn entry_count(&self) -> usize {
        self.entry_count
    }

    /// Returns whether this write imported a new catalog or created a rollback fork.
    #[must_use]
    pub const fn operation(&self) -> BillingCatalogMutationOperation {
        self.operation
    }

    /// Returns the predecessor catalog for a rollback fork, if this was a rollback.
    #[must_use]
    pub fn rolled_back_from(&self) -> Option<&str> {
        self.rolled_back_from.as_deref()
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

/// Secret-free Client Key metadata eligible for management reads and lifecycle writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientKeyView {
    /// Stable Client Key identifier.
    pub id: ClientKeyId,
    /// The Access Group authorized by this Key.
    pub access_group_id: AccessGroupId,
    /// Public indexed Prefix; never the complete Key.
    pub prefix: String,
    /// Persisted lifecycle state.
    pub status: StoredClientKeyStatus,
    /// Optional absolute expiry timestamp.
    pub expires_at_ms: Option<i64>,
}

/// Typed input for issuing one Client Key from the management boundary.
///
/// This value contains neither a complete Key nor a digest. The service generates those values
/// only after an explicit issuer has been supplied by its embedding application.
pub struct ClientKeyIssue {
    /// Stable Client Key identity to create.
    pub id: ClientKeyId,
    /// Access Group authorized by the issued Key.
    pub access_group_id: AccessGroupId,
    /// Requested durable lifecycle status.
    pub status: StoredClientKeyStatus,
    /// Optional absolute expiry timestamp.
    pub expires_at_ms: Option<i64>,
}

/// Typed non-secret lifecycle update for one existing Client Key.
pub struct ClientKeyUpdate {
    /// Access Group authorized by the Key after this update.
    pub access_group_id: AccessGroupId,
    /// Durable lifecycle status after this update.
    pub status: StoredClientKeyStatus,
    /// Optional absolute expiry timestamp after this update.
    pub expires_at_ms: Option<i64>,
}

/// Successful Client Key issuance with an immediate-only complete Key presentation.
pub struct IssuedClientKeyView {
    metadata: ClientKeyView,
    presented_key: PresentedClientKey,
}

/// Value-free structural validation result for one draft Route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementRouteValidation {
    /// Whether the draft Route has the minimum static topology for a future publication check.
    pub valid: bool,
    /// Stable value-free rejection labels. This is not a runtime selection or Explain result.
    pub error_codes: Vec<&'static str>,
}

impl IssuedClientKeyView {
    /// Returns the durable metadata safe for a management response.
    #[must_use]
    pub fn metadata(&self) -> &ClientKeyView {
        &self.metadata
    }

    /// Returns the complete Key only to the immediate successful HTTP response assembler.
    #[must_use]
    pub fn presented_key(&self) -> &str {
        self.presented_key.as_str()
    }
}

impl fmt::Debug for IssuedClientKeyView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedClientKeyView")
            .field("metadata", &self.metadata)
            .field("presented_key", &"<redacted>")
            .finish()
    }
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
    client_key_service: Option<ClientKeyService>,
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
        Self::with_clock_and_client_key_service(repository, secret_store, clock, None)
    }

    /// Creates the service with one explicit management-time Client Key issuer.
    ///
    /// The issuer is supplied by the embedding application; this service never loads Pepper
    /// material from an environment variable, an HTTP request, or persistent configuration.
    #[must_use]
    pub fn with_client_key_service(
        repository: SqliteControlPlaneRepository,
        secret_store: SecretStore,
        client_key_service: ClientKeyService,
    ) -> Self {
        Self::with_clock_and_client_key_service(
            repository,
            secret_store,
            Arc::new(SystemManagementClock),
            Some(client_key_service),
        )
    }

    /// Creates the service with deterministic time and an optional explicit Client Key issuer.
    #[must_use]
    pub fn with_clock_and_client_key_service(
        repository: SqliteControlPlaneRepository,
        secret_store: SecretStore,
        clock: Arc<dyn ManagementClock>,
        client_key_service: Option<ClientKeyService>,
    ) -> Self {
        Self {
            repository,
            secret_store,
            clock,
            client_key_service,
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

    /// Opens one Credential only for an explicitly authorized one-time export operation.
    ///
    /// The caller receives zeroizing bytes and must immediately transform or return them through
    /// the dedicated export boundary. Ordinary management reads continue to use `get_credential`
    /// and can never reach this method's plaintext result.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested version or credential is absent, or when the encrypted
    /// secret cannot be opened with its exact associated data.
    pub fn open_credential_for_export(
        &mut self,
        config_version_id: &ConfigVersionId,
        credential_id: &CredentialId,
    ) -> Result<gateway_store::secret_store::PlaintextSecret, ManagementResourceError> {
        let credential = self
            .configuration(config_version_id)?
            .credentials
            .into_iter()
            .find(|candidate| &candidate.id == credential_id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        let associated_data =
            credential_associated_data(config_version_id, &credential.id, &credential.upstream_id)?;
        Ok(self
            .secret_store
            .open(&credential.encrypted_secret, &associated_data)?)
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

    /// Replaces one encrypted Credential only when its record revision still matches.
    ///
    /// This is the management persistence CAS used by OAuth refresh workers. A stale worker is
    /// rejected before sealing or storage mutation, so it cannot overwrite a rotated token.
    ///
    /// # Errors
    ///
    /// Returns a revision conflict for stale configuration or credential revisions, or a value-free
    /// management error when sealing or the guarded transaction fails.
    pub fn update_credential_if_revision(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_config_revision: ConfigRevision,
        expected_credential_revision: i64,
        input: CredentialUpsert<'_>,
    ) -> Result<Revisioned<CredentialView>, ManagementResourceError> {
        let current = self
            .configuration(config_version_id)?
            .credentials
            .into_iter()
            .find(|candidate| candidate.id == input.id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        if current.revision != expected_credential_revision {
            return Err(ManagementResourceError::CredentialRevisionConflict);
        }
        self.update_credential(actor, config_version_id, expected_config_revision, input)
    }

    /// Persists a validated OAuth envelope through the normal encrypted Credential CAS path.
    ///
    /// The caller validates the token response and normalizes metadata. This method owns only the
    /// durable boundary: fixed `oauth_json` kind, AEAD sealing, audit, and both revisions.
    ///
    /// # Errors
    ///
    /// Returns a revision conflict before mutation when the credential changed, or a value-free
    /// management error when validation, sealing, audit, or the guarded transaction fails.
    pub fn persist_oauth_credential_if_revision(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_config_revision: ConfigRevision,
        credential_id: CredentialId,
        expected_credential_revision: i64,
        oauth_envelope: &[u8],
    ) -> Result<Revisioned<CredentialView>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let current = configuration
            .credentials
            .into_iter()
            .find(|candidate| candidate.id == credential_id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        if current.revision != expected_credential_revision {
            return Err(ManagementResourceError::CredentialRevisionConflict);
        }
        let next_credential_revision = current
            .revision
            .checked_add(1)
            .ok_or(ManagementResourceError::InvalidRevision)?;
        let credential = self.seal_credential(
            config_version_id,
            current.upstream_id,
            CredentialUpsert {
                id: credential_id,
                kind: "oauth_json".to_owned(),
                plaintext_secret: oauth_envelope,
                status: CredentialStatus::Active,
            },
            next_credential_revision,
        )?;

        if configuration.version.status == ConfigVersionStatus::Active {
            // OAuth login/refresh changes only the encrypted account material.  Keep the
            // published graph immutable while allowing the CPA/Sub2API-style account-pool
            // rotation to succeed on the active Version through its own exact CAS boundary.
            let audit = self.audit(
                "credential_oauth_rotated",
                actor,
                config_version_id,
                "credential",
                credential.id.as_str(),
            )?;
            let mut transaction = self.repository.begin_transaction()?;
            transaction.rotate_active_credential(
                config_version_id,
                &credential,
                expected_credential_revision,
            )?;
            transaction.record_management_resource_audit_event(&audit, config_version_id)?;
            transaction.commit()?;
            return Ok(Revisioned::new(
                self.credential_view(config_version_id, &credential.id)?,
                expected_config_revision,
            ));
        }

        let audit = self.audit(
            "credential_oauth_completed",
            actor,
            config_version_id,
            "credential",
            credential.id.as_str(),
        )?;
        let ((), next_config_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_config_revision.as_i64(),
            |transaction| {
                transaction.update_credential(config_version_id, &credential)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            self.credential_view(config_version_id, &credential.id)?,
            ConfigRevision::try_new(next_config_revision)?,
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

    /// Returns a stable secret-free page of configured Provider/Channel/Account bindings.
    ///
    /// This read projects only the caller-selected Config Version. It does not decrypt a
    /// Credential, contact a Provider, or reinterpret durable state as live health or quota.
    ///
    /// # Errors
    ///
    /// Returns an error when the Version is absent, the query or persisted revision is invalid,
    /// a cursor belongs to another graph revision, or a binding violates Provider ownership.
    pub fn list_operational_account_pools(
        &mut self,
        config_version_id: &ConfigVersionId,
        query: &OperationalAccountPoolQuery,
    ) -> Result<Revisioned<OperationalAccountPoolPage>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let revision = ConfigRevision::try_new(configuration.version.revision)?;
        let page = compile_operational_account_pool_page(&configuration, query)?;
        Ok(Revisioned::new(page, revision))
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

    /// Lists all Public Models in one Version together with its current revision.
    ///
    /// # Errors
    ///
    /// Returns an error when the selected Version is absent or has an invalid persisted revision.
    pub fn list_public_models(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<Revisioned<Vec<PublicModelConfiguration>>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        Ok(Revisioned::new(
            configuration.public_models,
            ConfigRevision::try_new(configuration.version.revision)?,
        ))
    }

    /// Returns one Public Model in one Version together with its current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Public Model is absent, or its persisted revision is
    /// invalid.
    pub fn get_public_model(
        &mut self,
        config_version_id: &ConfigVersionId,
        public_model_id: &PublicModelId,
    ) -> Result<Revisioned<PublicModelConfiguration>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let revision = ConfigRevision::try_new(configuration.version.revision)?;
        let public_model = configuration
            .public_models
            .into_iter()
            .find(|candidate| &candidate.id == public_model_id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        Ok(Revisioned::new(public_model, revision))
    }

    /// Creates one Public Model using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Public Model is invalid or already present, or the resource/audit transaction cannot commit.
    pub fn create_public_model(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        public_model: PublicModelConfiguration,
    ) -> Result<Revisioned<PublicModelConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "public_model_created",
            actor,
            config_version_id,
            "public_model",
            public_model.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_public_model(config_version_id, &public_model)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            public_model,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Updates one Public Model using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Public Model is absent or invalid, or the resource/audit transaction cannot commit.
    pub fn update_public_model(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        public_model: PublicModelConfiguration,
    ) -> Result<Revisioned<PublicModelConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "public_model_updated",
            actor,
            config_version_id,
            "public_model",
            public_model.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.update_public_model(config_version_id, &public_model)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            public_model,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Deletes one Public Model with schema-owned Alias and Route descendants.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Public Model is absent, or the resource/audit transaction cannot commit.
    pub fn delete_public_model(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        public_model_id: &PublicModelId,
    ) -> Result<ConfigRevision, ManagementResourceError> {
        self.delete_resource(
            actor,
            config_version_id,
            expected_revision,
            ResourceAction {
                action: "public_model_deleted",
                resource_kind: "public_model",
                resource_id: public_model_id.as_str(),
            },
            |transaction| transaction.delete_public_model(config_version_id, public_model_id),
        )
    }

    /// Creates one exact Alias-to-Public-Model relation using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Alias or Public Model is invalid or absent, or the resource/audit transaction cannot commit.
    pub fn create_model_alias(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        alias: ModelAliasConfiguration,
    ) -> Result<Revisioned<ModelAliasConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "model_alias_created",
            actor,
            config_version_id,
            "model_alias",
            &alias.alias,
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_model_alias(config_version_id, &alias)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            alias,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Returns one Route in one Version together with its current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Route is absent, or its persisted revision is invalid.
    pub fn get_model_route(
        &mut self,
        config_version_id: &ConfigVersionId,
        route_id: &RouteId,
    ) -> Result<Revisioned<ModelRouteConfiguration>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let revision = ConfigRevision::try_new(configuration.version.revision)?;
        let route = configuration
            .model_routes
            .into_iter()
            .find(|candidate| &candidate.id == route_id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        Ok(Revisioned::new(route, revision))
    }

    /// Creates one Route under an existing Public Model using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the Route
    /// or its Public Model is invalid or absent, or the resource/audit transaction cannot commit.
    pub fn create_model_route(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        route: ModelRouteConfiguration,
    ) -> Result<Revisioned<ModelRouteConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "route_created",
            actor,
            config_version_id,
            "route",
            route.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_model_route(config_version_id, &route)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            route,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Updates one Route without allowing it to move between Public Models.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the Route
    /// is absent or invalid, or the resource/audit transaction cannot commit.
    pub fn update_model_route(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        route: ModelRouteConfiguration,
    ) -> Result<Revisioned<ModelRouteConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "route_updated",
            actor,
            config_version_id,
            "route",
            route.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.update_model_route(config_version_id, &route)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            route,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Deletes one Route with schema-owned Candidate and Access Group grant descendants.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the Route
    /// is absent, or the resource/audit transaction cannot commit.
    pub fn delete_model_route(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        route_id: &RouteId,
    ) -> Result<ConfigRevision, ManagementResourceError> {
        self.delete_resource(
            actor,
            config_version_id,
            expected_revision,
            ResourceAction {
                action: "route_deleted",
                resource_kind: "route",
                resource_id: route_id.as_str(),
            },
            |transaction| transaction.delete_model_route(config_version_id, route_id),
        )
    }

    /// Creates one Candidate under an existing Route using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Candidate, Route, or Endpoint is invalid or absent, or the resource/audit transaction fails.
    pub fn create_route_candidate(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        candidate: RouteCandidateConfiguration,
    ) -> Result<Revisioned<RouteCandidateConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "route_candidate_created",
            actor,
            config_version_id,
            "route_candidate",
            candidate.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_route_candidate(config_version_id, &candidate)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            candidate,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Validates only draft Route topology without publishing, selecting, or contacting an
    /// upstream. Full compiler/capability admission remains the later publication boundary.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Route is absent, or persisted graph data is invalid.
    pub fn validate_model_route(
        &mut self,
        config_version_id: &ConfigVersionId,
        route_id: &RouteId,
    ) -> Result<ManagementRouteValidation, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let route_exists = configuration
            .model_routes
            .iter()
            .any(|candidate| &candidate.id == route_id);
        if !route_exists {
            return Err(ManagementResourceError::ResourceNotFound);
        }

        let mut error_codes = Vec::new();
        let active_candidates = configuration
            .route_candidates
            .iter()
            .filter(|candidate| &candidate.route_id == route_id && candidate.enabled)
            .collect::<Vec<_>>();
        if active_candidates.is_empty() {
            error_codes.push("route_missing_active_candidate");
        }
        for candidate in active_candidates {
            let Some(endpoint) = configuration
                .endpoints
                .iter()
                .find(|endpoint| endpoint.id == candidate.endpoint_id)
            else {
                error_codes.push("route_candidate_endpoint_missing");
                continue;
            };
            if !endpoint.enabled {
                error_codes.push("route_candidate_endpoint_disabled");
            }
            let has_active_binding = configuration
                .endpoint_credential_bindings
                .iter()
                .filter(|binding| binding.endpoint_id == candidate.endpoint_id && binding.enabled)
                .any(|binding| {
                    configuration.credentials.iter().any(|credential| {
                        credential.id == binding.credential_id
                            && credential.status == CredentialStatus::Active
                    })
                });
            let native_grok_account_pool = configuration
                .upstreams
                .iter()
                .find(|upstream| upstream.id == endpoint.upstream_id)
                .is_some_and(|upstream| {
                    (upstream.kind == "grok-build-native"
                        && endpoint.adapter_id == "grok.build.responses")
                        || (upstream.kind == "grok-console-native"
                            && endpoint.adapter_id == "grok.console.responses")
                        || (upstream.kind == "grok-web-native"
                            && endpoint.adapter_id == "grok.web.responses")
                });
            if !has_active_binding && !native_grok_account_pool {
                error_codes.push("route_candidate_missing_active_credential");
            }
        }
        error_codes.sort_unstable();
        error_codes.dedup();
        Ok(ManagementRouteValidation {
            valid: error_codes.is_empty(),
            error_codes,
        })
    }

    /// Lists all Access Groups in one Version together with its current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is absent or has an invalid persisted revision.
    pub fn list_access_groups(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<Revisioned<Vec<AccessGroupConfiguration>>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        Ok(Revisioned::new(
            configuration.access_groups,
            ConfigRevision::try_new(configuration.version.revision)?,
        ))
    }

    /// Returns one Access Group in one Version together with its current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Access Group is absent, or its persisted revision is
    /// invalid.
    pub fn get_access_group(
        &mut self,
        config_version_id: &ConfigVersionId,
        access_group_id: &AccessGroupId,
    ) -> Result<Revisioned<AccessGroupConfiguration>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let revision = ConfigRevision::try_new(configuration.version.revision)?;
        let access_group = configuration
            .access_groups
            .into_iter()
            .find(|candidate| &candidate.id == access_group_id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        Ok(Revisioned::new(access_group, revision))
    }

    /// Creates one Access Group using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Access Group is invalid or already present, or the resource/audit transaction cannot commit.
    pub fn create_access_group(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        access_group: AccessGroupConfiguration,
    ) -> Result<Revisioned<AccessGroupConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "access_group_created",
            actor,
            config_version_id,
            "access_group",
            access_group.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_access_group(config_version_id, &access_group)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            access_group,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Updates one Access Group using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Access Group is absent or invalid, or the resource/audit transaction cannot commit.
    pub fn update_access_group(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        access_group: AccessGroupConfiguration,
    ) -> Result<Revisioned<AccessGroupConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "access_group_updated",
            actor,
            config_version_id,
            "access_group",
            access_group.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.update_access_group(config_version_id, &access_group)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            access_group,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Deletes one Access Group with schema-owned grants and Client Keys.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Access Group is absent, or the resource/audit transaction cannot commit.
    pub fn delete_access_group(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        access_group_id: &AccessGroupId,
    ) -> Result<ConfigRevision, ManagementResourceError> {
        self.delete_resource(
            actor,
            config_version_id,
            expected_revision,
            ResourceAction {
                action: "access_group_deleted",
                resource_kind: "access_group",
                resource_id: access_group_id.as_str(),
            },
            |transaction| transaction.delete_access_group(config_version_id, access_group_id),
        )
    }

    /// Lists exact Route grants for one existing Access Group and its current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Access Group is absent, or its persisted revision is
    /// invalid.
    pub fn list_access_group_routes(
        &mut self,
        config_version_id: &ConfigVersionId,
        access_group_id: &AccessGroupId,
    ) -> Result<Revisioned<Vec<AccessGroupRouteConfiguration>>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        if !configuration
            .access_groups
            .iter()
            .any(|candidate| &candidate.id == access_group_id)
        {
            return Err(ManagementResourceError::ResourceNotFound);
        }
        let grants = configuration
            .access_group_routes
            .into_iter()
            .filter(|candidate| &candidate.access_group_id == access_group_id)
            .collect();
        Ok(Revisioned::new(
            grants,
            ConfigRevision::try_new(configuration.version.revision)?,
        ))
    }

    /// Creates one exact Access Group-to-Route grant using an exact draft revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the
    /// Access Group or Route is absent, or the resource/audit transaction cannot commit.
    pub fn create_access_group_route(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        grant: AccessGroupRouteConfiguration,
    ) -> Result<Revisioned<AccessGroupRouteConfiguration>, ManagementResourceError> {
        let audit = self.audit(
            "access_group_route_granted",
            actor,
            config_version_id,
            "access_group_route",
            grant.route_id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_access_group_route(config_version_id, &grant)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            grant,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Lists only redacted Client Key metadata in one Version with its current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is absent or has an invalid persisted revision.
    pub fn list_client_keys(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<Revisioned<Vec<ClientKeyView>>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let values = configuration
            .client_keys
            .iter()
            .map(ClientKeyView::from)
            .collect();
        Ok(Revisioned::new(
            values,
            ConfigRevision::try_new(configuration.version.revision)?,
        ))
    }

    /// Returns only redacted metadata for one Client Key in one Version.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version or Client Key is absent, or its persisted revision is
    /// invalid.
    pub fn get_client_key(
        &mut self,
        config_version_id: &ConfigVersionId,
        client_key_id: &ClientKeyId,
    ) -> Result<Revisioned<ClientKeyView>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let revision = ConfigRevision::try_new(configuration.version.revision)?;
        let client_key = configuration
            .client_keys
            .iter()
            .find(|candidate| candidate.id() == client_key_id)
            .map(ClientKeyView::from)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        Ok(Revisioned::new(client_key, revision))
    }

    /// Issues and durably records one Client Key using an exact draft revision.
    ///
    /// The complete Key is retained only in the returned immediate presentation. If generation,
    /// persistence, audit append, or the revision transaction fails, the presentation is dropped
    /// and no complete Key is exposed.
    ///
    /// # Errors
    ///
    /// Returns an error if no explicit issuer is configured, the input is invalid, the Version is
    /// not an admitted draft at `expected_revision`, or the resource/audit transaction fails.
    pub fn issue_client_key(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        input: ClientKeyIssue,
    ) -> Result<Revisioned<IssuedClientKeyView>, ManagementResourceError> {
        let key_issuer = self
            .client_key_service
            .as_ref()
            .ok_or(ManagementResourceError::ClientKeyIssuerUnavailable)?;
        let issuance = key_issuer.issue(input.id, input.access_group_id, input.expires_at_ms)?;
        let (mut record, presented_key) = issuance.into_parts();
        record.set_status(issued_client_key_status(input.status));
        let stored = StoredClientKey::try_new(
            record.client_key_id().clone(),
            record.access_group_id().clone(),
            record.prefix().as_str(),
            record.secret_digest().as_bytes(),
            input.status,
            record.expires_at_ms(),
        )?;
        let metadata = ClientKeyView::from(&stored);
        let audit = self.audit(
            "client_key_issued",
            actor,
            config_version_id,
            "client_key",
            metadata.id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_client_key(config_version_id, &stored)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            IssuedClientKeyView {
                metadata,
                presented_key,
            },
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Updates non-secret Client Key lifecycle metadata without changing its Prefix or digest.
    ///
    /// # Errors
    ///
    /// Returns an error if the Client Key is absent, the Version is not an admitted draft at
    /// `expected_revision`, the input is invalid, or the resource/audit transaction cannot commit.
    pub fn update_client_key(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        client_key_id: &ClientKeyId,
        input: ClientKeyUpdate,
    ) -> Result<Revisioned<ClientKeyView>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let current = configuration
            .client_keys
            .iter()
            .find(|candidate| candidate.id() == client_key_id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        let mut view = ClientKeyView::from(current);
        view.access_group_id = input.access_group_id;
        view.status = input.status;
        view.expires_at_ms = input.expires_at_ms;
        let audit = self.audit(
            "client_key_updated",
            actor,
            config_version_id,
            "client_key",
            client_key_id.as_str(),
        )?;
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.update_client_key_metadata(
                    config_version_id,
                    client_key_id,
                    &view.access_group_id,
                    view.status,
                    view.expires_at_ms,
                )?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            view,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Revokes one Client Key using an exact draft revision while retaining its redacted record.
    ///
    /// # Errors
    ///
    /// Returns an error if the Version is not an admitted draft at `expected_revision`, the Client
    /// Key is absent, or the resource/audit transaction cannot commit.
    pub fn revoke_client_key(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        client_key_id: &ClientKeyId,
    ) -> Result<ConfigRevision, ManagementResourceError> {
        self.delete_resource(
            actor,
            config_version_id,
            expected_revision,
            ResourceAction {
                action: "client_key_revoked",
                resource_kind: "client_key",
                resource_id: client_key_id.as_str(),
            },
            |transaction| transaction.revoke_client_key(config_version_id, client_key_id),
        )
    }

    /// Lists immutable billing catalogs under the selected management Config Version.
    ///
    /// The Config Version is an admission/revision context only; catalog rows remain global to
    /// the migrated control-plane database and are never copied into the graph.
    ///
    /// # Errors
    ///
    /// Returns an error when the Version or bounded catalog source is unavailable.
    pub fn list_billing_catalogs(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<Revisioned<Vec<BillingPriceCatalog>>, ManagementResourceError> {
        let configuration = self.configuration(config_version_id)?;
        let catalogs = self
            .repository
            .list_billing_catalogs_bounded(MAX_MANAGEMENT_BILLING_CATALOGS + 1)?;
        if catalogs.len() > MAX_MANAGEMENT_BILLING_CATALOGS {
            return Err(ManagementResourceError::Store(
                StoreError::InvalidPersistedBillingRecord,
            ));
        }
        Ok(Revisioned::new(
            catalogs,
            ConfigRevision::try_new(configuration.version.revision)?,
        ))
    }

    /// Imports one immutable billing catalog and atomically advances the selected draft revision
    /// with its non-secret audit event.
    ///
    /// # Errors
    ///
    /// Returns a revision conflict for a stale draft, a catalog conflict for a reused version
    /// with different content, or a value-free validation/storage error.
    pub fn import_billing_catalog(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        input: BillingCatalogImport,
    ) -> Result<Revisioned<BillingCatalogMutationReceipt>, ManagementResourceError> {
        if matches!(input.source, BillingCatalogSource::Test) || input.entries.is_empty() {
            return Err(ManagementResourceError::InvalidBillingCatalogInput);
        }
        let created_at_ms = u64::try_from(self.clock.now_ms()?)
            .map_err(|_| ManagementResourceError::InvalidBillingCatalogInput)?;
        let catalog = BillingPriceCatalog {
            catalog_version_id: input.catalog_version_id,
            effective_at_ms: input.effective_at_ms,
            source: input.source,
            created_at_ms,
            entries: input.entries,
        };
        let audit = self.audit(
            "billing_catalog_imported",
            actor,
            config_version_id,
            "billing_catalog",
            &catalog.catalog_version_id,
        )?;
        let receipt = BillingCatalogMutationReceipt::new(
            &catalog,
            BillingCatalogMutationOperation::Imported,
            None,
        );
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_billing_catalog(&catalog)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            receipt,
            ConfigRevision::try_new(next_revision)?,
        ))
    }

    /// Creates a new immutable catalog version from a retained predecessor. Existing rows are
    /// never edited or deleted, so rollback is an auditable forward fork rather than destructive
    /// history rewriting.
    ///
    /// # Errors
    ///
    /// Returns an error when the predecessor is absent, the new version conflicts, the draft
    /// revision is stale, or the atomic catalog/audit transaction fails.
    pub fn rollback_billing_catalog(
        &mut self,
        actor: &ManagementActor,
        config_version_id: &ConfigVersionId,
        expected_revision: ConfigRevision,
        predecessor_version_id: &str,
        new_version_id: String,
        effective_at_ms: u64,
    ) -> Result<Revisioned<BillingCatalogMutationReceipt>, ManagementResourceError> {
        if predecessor_version_id == new_version_id || new_version_id.trim().is_empty() {
            return Err(ManagementResourceError::InvalidBillingCatalogInput);
        }
        let predecessor = self
            .repository
            .load_billing_catalog(predecessor_version_id)?
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        let created_at_ms = u64::try_from(self.clock.now_ms()?)
            .map_err(|_| ManagementResourceError::InvalidBillingCatalogInput)?;
        let catalog = BillingPriceCatalog {
            catalog_version_id: new_version_id,
            effective_at_ms,
            source: BillingCatalogSource::Operator,
            created_at_ms,
            entries: predecessor.entries,
        };
        let audit = self.audit(
            "billing_catalog_rolled_back",
            actor,
            config_version_id,
            "billing_catalog",
            &catalog.catalog_version_id,
        )?;
        let receipt = BillingCatalogMutationReceipt::new(
            &catalog,
            BillingCatalogMutationOperation::RolledBack,
            Some(predecessor_version_id.to_owned()),
        );
        let ((), next_revision) = self.repository.mutate_draft_configuration(
            config_version_id,
            expected_revision.as_i64(),
            |transaction| {
                transaction.insert_billing_catalog(&catalog)?;
                transaction.record_management_resource_audit_event(&audit, config_version_id)
            },
        )?;
        Ok(Revisioned::new(
            receipt,
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

impl From<&StoredClientKey> for ClientKeyView {
    fn from(value: &StoredClientKey) -> Self {
        Self {
            id: value.id().clone(),
            access_group_id: value.access_group_id().clone(),
            prefix: value.prefix().to_owned(),
            status: value.status(),
            expires_at_ms: value.expires_at_ms(),
        }
    }
}

const fn issued_client_key_status(value: StoredClientKeyStatus) -> IssuedClientKeyStatus {
    match value {
        StoredClientKeyStatus::Active => IssuedClientKeyStatus::Active,
        StoredClientKeyStatus::Disabled => IssuedClientKeyStatus::Disabled,
        StoredClientKeyStatus::Revoked => IssuedClientKeyStatus::Revoked,
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
    /// The injected Client Key issuer could not safely create a Key.
    ClientKey(ClientKeyError),
    /// This embedding did not explicitly provide a management-time Client Key issuer.
    ClientKeyIssuerUnavailable,
    /// The selected Config Version was absent.
    ConfigVersionNotFound,
    /// The selected Version-scoped resource was absent.
    ResourceNotFound,
    /// A negative or overflowed configuration revision was rejected.
    InvalidRevision,
    /// A credential mutation had no plaintext Secret or an invalid record revision.
    InvalidCredentialInput,
    /// A concurrent credential update advanced the record revision.
    CredentialRevisionConflict,
    /// A protected billing catalog request violated the bounded immutable input contract.
    InvalidBillingCatalogInput,
    /// The P13 operational read-model query, cursor, or source graph was invalid.
    Operations(ManagementOperationsError),
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
            Self::ClientKey(error) => {
                write!(formatter, "management client key issuance failed: {error}")
            }
            Self::ClientKeyIssuerUnavailable => {
                formatter.write_str("management client key issuer is unavailable")
            }
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
            Self::CredentialRevisionConflict => {
                formatter.write_str("management credential revision conflict")
            }
            Self::InvalidBillingCatalogInput => {
                formatter.write_str("management billing catalog input is invalid")
            }
            Self::Operations(error) => write!(formatter, "management operations failed: {error}"),
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
            Self::ClientKey(error) => Some(error),
            Self::Operations(error) => Some(error),
            Self::ConfigVersionNotFound
            | Self::ResourceNotFound
            | Self::InvalidRevision
            | Self::InvalidCredentialInput
            | Self::CredentialRevisionConflict
            | Self::InvalidBillingCatalogInput
            | Self::ClientKeyIssuerUnavailable => None,
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

impl From<ClientKeyError> for ManagementResourceError {
    fn from(value: ClientKeyError) -> Self {
        Self::ClientKey(value)
    }
}

impl From<ManagementOperationsError> for ManagementResourceError {
    fn from(value: ManagementOperationsError) -> Self {
        Self::Operations(value)
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, sync::Arc};

    use gateway_auth::client_key::{ClientKeyPepper, ClientKeyService};
    use gateway_core::{
        AccessGroupId, ClientKeyId, CredentialId, EgressPolicyId, EndpointId, PublicModelId,
        RouteCandidateId, RouteId, UpstreamId,
    };
    use gateway_store::{
        StoreError,
        control_plane::{
            AccessGroupConfiguration, AccessGroupRouteConfiguration, AdministrativeStatus,
            ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialScope, CredentialStatus, EgressPolicyConfiguration, EndpointConfiguration,
            EndpointCredentialBindingConfiguration, EndpointTransport, ModelAliasConfiguration,
            ModelRouteConfiguration, PublicModelConfiguration, RouteCandidateConfiguration,
            RoutePolicy, SqliteControlPlaneRepository, StoredClientKeyStatus,
            StoredEgressRedirectMode, TransformMode, UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };

    use crate::management_service::{ManagementActor, ManagementClock, ManagementClockError};

    use super::{
        BillingCatalogImport, BillingCatalogMutationOperation, BillingCatalogSource,
        BillingPriceEntry, ClientKeyIssue, ClientKeyUpdate, ConfigRevision, CredentialUpsert,
        CredentialView, ManagementMutationService, ManagementResourceError, Revisioned,
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

    #[test]
    fn billing_catalog_mutation_is_atomic_audited_and_forward_rollback_only() -> TestResult {
        let (mut service, version_id, actor) = test_service()?;
        let entry = BillingPriceEntry {
            provider_id: "provider-a".to_owned(),
            channel_id: "channel-a".to_owned(),
            model: "model-a".to_owned(),
            input_microunits_per_million: 2_000_000,
            output_microunits_per_million: 4_000_000,
            reasoning_microunits_per_million: 0,
            cache_read_microunits_per_million: 0,
            cache_creation_microunits_per_million: 0,
            cached_microunits_per_million: 0,
        };

        let imported = service.import_billing_catalog(
            &actor,
            &version_id,
            ConfigRevision::initial(),
            BillingCatalogImport {
                catalog_version_id: "catalog-v1".to_owned(),
                effective_at_ms: 1_000,
                source: BillingCatalogSource::Imported,
                entries: vec![entry.clone()],
            },
        )?;
        assert_eq!(imported.revision().as_i64(), 1);
        assert_eq!(
            imported.value().operation(),
            BillingCatalogMutationOperation::Imported
        );
        assert_eq!(service.resource_audit_events()?.len(), 1);
        assert_eq!(
            service
                .resource_audit_events()?
                .first()
                .ok_or("billing import audit event missing")?
                .action(),
            "billing_catalog_imported"
        );

        let mut conflicting_entry = entry;
        conflicting_entry.input_microunits_per_million = 9_000_000;
        let conflict = service.import_billing_catalog(
            &actor,
            &version_id,
            imported.revision(),
            BillingCatalogImport {
                catalog_version_id: "catalog-v1".to_owned(),
                effective_at_ms: 1_000,
                source: BillingCatalogSource::Imported,
                entries: vec![conflicting_entry],
            },
        );
        assert!(matches!(
            conflict,
            Err(ManagementResourceError::Store(
                StoreError::ConflictingBillingCatalogVersion
            ))
        ));
        assert_eq!(
            service
                .list_billing_catalogs(&version_id)?
                .revision()
                .as_i64(),
            1
        );
        assert_eq!(service.resource_audit_events()?.len(), 1);

        let rolled_back = service.rollback_billing_catalog(
            &actor,
            &version_id,
            imported.revision(),
            "catalog-v1",
            "catalog-v2".to_owned(),
            2_000,
        )?;
        assert_eq!(rolled_back.revision().as_i64(), 2);
        assert_eq!(
            rolled_back.value().operation(),
            BillingCatalogMutationOperation::RolledBack
        );
        assert_eq!(rolled_back.value().rolled_back_from(), Some("catalog-v1"));
        assert_eq!(service.resource_audit_events()?.len(), 2);
        assert_eq!(
            service
                .resource_audit_events()?
                .last()
                .ok_or("billing rollback audit event missing")?
                .action(),
            "billing_catalog_rolled_back"
        );
        let catalogs = service.list_billing_catalogs(&version_id)?.value().clone();
        assert_eq!(catalogs.len(), 2);
        assert_eq!(catalogs[0].catalog_version_id, "catalog-v1");
        assert_eq!(catalogs[1].catalog_version_id, "catalog-v2");
        assert_eq!(catalogs[0].entries, catalogs[1].entries);
        Ok(())
    }

    #[test]
    fn oauth_persistence_rejects_stale_credential_revision_before_mutation() -> TestResult {
        let (mut service, version_id, actor) = test_service()?;
        let policy = create_egress_policy(&mut service, &actor, &version_id)?;
        let upstream = create_upstream(&mut service, &actor, &version_id, policy.revision())?;
        let endpoint = create_endpoint(&mut service, &actor, &version_id, upstream.revision())?;
        let credential = create_credential(&mut service, &actor, &version_id, endpoint.revision())?;

        let updated = service.persist_oauth_credential_if_revision(
            &actor,
            &version_id,
            credential.revision(),
            CredentialId::try_new("credential-a")?,
            0,
            br#"{"kind":"codex_oauth","access_token":"new","refresh_token":"refresh","expires_at_ms":1000}"#,
        )?;
        assert_eq!(updated.value().kind, "oauth_json");
        assert_eq!(updated.value().revision, 1);

        let stale = service.persist_oauth_credential_if_revision(
            &actor,
            &version_id,
            updated.revision(),
            CredentialId::try_new("credential-a")?,
            0,
            br#"{"kind":"codex_oauth","access_token":"stale","refresh_token":"refresh","expires_at_ms":1000}"#,
        );
        assert!(matches!(
            stale,
            Err(ManagementResourceError::CredentialRevisionConflict)
        ));
        assert_eq!(
            service
                .get_credential(&version_id, &CredentialId::try_new("credential-a")?)?
                .value()
                .revision,
            1
        );
        Ok(())
    }

    #[test]
    fn active_oauth_persistence_rotates_only_the_credential_revision() -> TestResult {
        let (mut service, version_id, actor) = test_service()?;
        let policy = create_egress_policy(&mut service, &actor, &version_id)?;
        let upstream = create_upstream(&mut service, &actor, &version_id, policy.revision())?;
        let endpoint = create_endpoint(&mut service, &actor, &version_id, upstream.revision())?;
        let credential = create_credential(&mut service, &actor, &version_id, endpoint.revision())?;
        service.repository_mut().activate_version(&version_id)?;

        let updated = service.persist_oauth_credential_if_revision(
            &actor,
            &version_id,
            credential.revision(),
            CredentialId::try_new("credential-a")?,
            credential.value().revision,
            br#"{"kind":"codex_oauth","access_token":"new","refresh_token":"refresh","expires_at_ms":1000}"#,
        )?;
        assert_eq!(updated.revision(), credential.revision());
        assert_eq!(updated.value().revision, 1);
        assert_eq!(updated.value().kind, "oauth_json");
        let stored = service
            .repository_mut()
            .load_configuration(&version_id)?
            .ok_or("active OAuth configuration was not found")?;
        assert_eq!(stored.version.status, ConfigVersionStatus::Active);
        assert_eq!(stored.version.revision, credential.revision().as_i64());
        assert_eq!(stored.credentials[0].revision, 1);
        assert_eq!(
            service
                .resource_audit_events()?
                .last()
                .ok_or("OAuth rotation audit event was not found")?
                .action(),
            "credential_oauth_rotated"
        );
        Ok(())
    }

    #[test]
    fn credential_export_open_is_explicit_and_zeroizing() -> TestResult {
        let (mut service, version_id, actor) = test_service()?;
        let policy = create_egress_policy(&mut service, &actor, &version_id)?;
        let upstream = create_upstream(&mut service, &actor, &version_id, policy.revision())?;
        let endpoint = create_endpoint(&mut service, &actor, &version_id, upstream.revision())?;
        let credential = create_credential(&mut service, &actor, &version_id, endpoint.revision())?;
        let opened = service
            .open_credential_for_export(&version_id, &CredentialId::try_new("credential-a")?)?;
        assert_eq!(opened.as_bytes(), b"test-secret-not-returned");
        assert_eq!(credential.value().revision, 0);
        Ok(())
    }

    #[test]
    fn routing_graph_and_client_key_lifecycle_are_atomic_and_redacted() -> TestResult {
        let (mut service, version_id, actor) = test_service_with_client_key_issuer()?;
        let revision = create_minimax_routing_graph(&mut service, &actor, &version_id)?;
        let revision =
            issue_and_assert_redacted_client_key(&mut service, &actor, &version_id, revision)?;
        update_revoke_and_assert_graph_cascade(&mut service, &actor, &version_id, revision)
    }

    fn create_minimax_routing_graph(
        service: &mut ManagementMutationService,
        actor: &ManagementActor,
        version_id: &ConfigVersionId,
    ) -> Result<ConfigRevision, Box<dyn Error>> {
        let policy = create_egress_policy(service, actor, version_id)?;
        let upstream = create_upstream(service, actor, version_id, policy.revision())?;
        let endpoint = create_endpoint(service, actor, version_id, upstream.revision())?;
        let credential = create_credential(service, actor, version_id, endpoint.revision())?;
        let binding = create_binding(service, actor, version_id, credential.revision())?;

        let public_model = service.create_public_model(
            actor,
            version_id,
            binding.revision(),
            PublicModelConfiguration {
                id: PublicModelId::try_new("model-minimax-m3")?,
                model_name: "minimax-m3".to_owned(),
                status: AdministrativeStatus::Active,
                display_name: "MiniMax M3".to_owned(),
                capabilities_json: "{}".to_owned(),
            },
        )?;
        let alias = service.create_model_alias(
            actor,
            version_id,
            public_model.revision(),
            ModelAliasConfiguration {
                alias: "minimax-m3-latest".to_owned(),
                public_model_id: PublicModelId::try_new("model-minimax-m3")?,
            },
        )?;
        let route = service.create_model_route(
            actor,
            version_id,
            alias.revision(),
            ModelRouteConfiguration {
                id: RouteId::try_new("route-minimax-m3")?,
                public_model_id: PublicModelId::try_new("model-minimax-m3")?,
                policy: RoutePolicy::SmoothWeightedRoundRobin,
                max_attempts: 2,
                bootstrap_timeout_ms: 2_000,
            },
        )?;
        let candidate = service.create_route_candidate(
            actor,
            version_id,
            route.revision(),
            RouteCandidateConfiguration {
                id: RouteCandidateId::try_new("candidate-minimax-m3")?,
                route_id: RouteId::try_new("route-minimax-m3")?,
                endpoint_id: EndpointId::try_new("endpoint-a")?,
                upstream_model: "minimax-m3-upstream".to_owned(),
                credential_scope: CredentialScope::EndpointBindings,
                transform_mode: TransformMode::Canonical,
                enabled: true,
                priority: 0,
                weight: 100,
                capability_override_json: "{}".to_owned(),
            },
        )?;
        let access_group = service.create_access_group(
            actor,
            version_id,
            candidate.revision(),
            AccessGroupConfiguration {
                id: AccessGroupId::try_new("group-minimax")?,
                name: "MiniMax users".to_owned(),
                status: AdministrativeStatus::Active,
                limits_json: "{}".to_owned(),
            },
        )?;
        let grant = service.create_access_group_route(
            actor,
            version_id,
            access_group.revision(),
            AccessGroupRouteConfiguration {
                access_group_id: AccessGroupId::try_new("group-minimax")?,
                route_id: RouteId::try_new("route-minimax-m3")?,
                enabled: true,
            },
        )?;
        Ok(grant.revision())
    }

    fn issue_and_assert_redacted_client_key(
        service: &mut ManagementMutationService,
        actor: &ManagementActor,
        version_id: &ConfigVersionId,
        revision: ConfigRevision,
    ) -> Result<ConfigRevision, Box<dyn Error>> {
        let issued = service.issue_client_key(
            actor,
            version_id,
            revision,
            ClientKeyIssue {
                id: ClientKeyId::try_new("client-minimax")?,
                access_group_id: AccessGroupId::try_new("group-minimax")?,
                status: StoredClientKeyStatus::Active,
                expires_at_ms: Some(10_000),
            },
        )?;
        let presented_key = issued.value().presented_key().to_owned();
        assert!(presented_key.starts_with("rgw_"));
        assert!(!format!("{:?}", issued.value()).contains(&presented_key));
        assert_eq!(issued.revision().as_i64(), 12);

        let listed = service.list_client_keys(version_id)?;
        assert_eq!(listed.revision(), issued.revision());
        assert_eq!(listed.value().len(), 1);
        assert!(!format!("{:?}", listed.value()).contains(&presented_key));
        let stored = service
            .repository_mut()
            .load_configuration(version_id)?
            .ok_or("missing configuration")?;
        assert_eq!(stored.client_keys.len(), 1);
        assert_eq!(stored.client_keys[0].secret_digest().len(), 32);
        assert!(!format!("{:?}", stored.client_keys[0]).contains(&presented_key));

        let duplicate = service.issue_client_key(
            actor,
            version_id,
            issued.revision(),
            ClientKeyIssue {
                id: ClientKeyId::try_new("client-minimax")?,
                access_group_id: AccessGroupId::try_new("group-minimax")?,
                status: StoredClientKeyStatus::Active,
                expires_at_ms: None,
            },
        );
        assert!(matches!(duplicate, Err(ManagementResourceError::Store(_))));
        assert_eq!(
            service.list_client_keys(version_id)?.revision(),
            issued.revision()
        );
        Ok(issued.revision())
    }

    fn update_revoke_and_assert_graph_cascade(
        service: &mut ManagementMutationService,
        actor: &ManagementActor,
        version_id: &ConfigVersionId,
        revision: ConfigRevision,
    ) -> TestResult {
        let updated = service.update_client_key(
            actor,
            version_id,
            revision,
            &ClientKeyId::try_new("client-minimax")?,
            ClientKeyUpdate {
                access_group_id: AccessGroupId::try_new("group-minimax")?,
                status: StoredClientKeyStatus::Disabled,
                expires_at_ms: Some(20_000),
            },
        )?;
        assert_eq!(updated.value().status, StoredClientKeyStatus::Disabled);
        assert_eq!(updated.value().expires_at_ms, Some(20_000));
        let revoked = service.revoke_client_key(
            actor,
            version_id,
            updated.revision(),
            &ClientKeyId::try_new("client-minimax")?,
        )?;
        assert_eq!(revoked.as_i64(), 14);
        assert_eq!(
            service
                .get_client_key(version_id, &ClientKeyId::try_new("client-minimax")?)?
                .value()
                .status,
            StoredClientKeyStatus::Revoked
        );

        let deleted_route = service.delete_model_route(
            actor,
            version_id,
            revoked,
            &RouteId::try_new("route-minimax-m3")?,
        )?;
        assert!(
            service
                .list_access_group_routes(version_id, &AccessGroupId::try_new("group-minimax")?)?
                .value()
                .is_empty()
        );
        let deleted_group = service.delete_access_group(
            actor,
            version_id,
            deleted_route,
            &AccessGroupId::try_new("group-minimax")?,
        )?;
        assert_eq!(deleted_group.as_i64(), 16);
        assert!(service.list_client_keys(version_id)?.value().is_empty());
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

    fn test_service_with_client_key_issuer()
    -> Result<(ManagementMutationService, ConfigVersionId, ManagementActor), Box<dyn Error>> {
        let version_id = ConfigVersionId::try_new("draft-routing")?;
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
            id: version_id.clone(),
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 1,
            description: "routing mutation test".to_owned(),
        }))?;
        let key_version = KeyVersion::try_new(1)?;
        let key_ring = MasterKeyRing::try_new(
            key_version,
            [(key_version, MasterKey::try_from_bytes([0x72_u8; 32])?)],
        )?;
        let issuer = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x51_u8; 32])?);
        let service = ManagementMutationService::with_clock_and_client_key_service(
            repository,
            SecretStore::new(key_ring),
            Arc::new(FixedClock),
            Some(issuer),
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
