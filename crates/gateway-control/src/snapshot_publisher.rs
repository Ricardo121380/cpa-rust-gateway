//! Management-time publication of validated Config Versions as immutable RouteSnapshots.
//!
//! This module deliberately compiles and builds a complete replacement before it reserves the
//! router publication slot. The reservation remains held across the matching SQLite transition,
//! so a failed durable transition cannot alter the current in-memory Snapshot.

use std::{error::Error, fmt, sync::Arc};

use gateway_auth::client_key::{
    ClientKeyDigest, ClientKeyError, ClientKeyPrefix, ClientKeyRecord, ClientKeyStatus,
};
use gateway_router::{
    RouteSnapshot, RouteSnapshotBuildError, RouteSnapshotInput, RouteSnapshotRegistry,
    SnapshotAccessGroup, SnapshotCatalogAdmission, SnapshotClientKeyView, SnapshotRegistryError,
    SnapshotRoute, SnapshotRouteCandidate, SnapshotRouteCandidateInput, SnapshotRoutePolicy,
    SnapshotTransformMode, SnapshotTransition, SnapshotVersion,
};
use gateway_store::{
    StoreError,
    control_plane::{
        ConfigVersionActivation, ConfigVersionId, ControlPlaneConfiguration, ManagementAuditEvent,
        ManagementAuditEventDraft, RoutePolicy, SqliteControlPlaneRepository, StoredClientKey,
        StoredClientKeyStatus, TransformMode,
    },
};

use crate::egress_policy_compiler::{EgressPolicyCompileError, EgressPolicyCompiler};
use crate::route_compiler::{
    CatalogAdmission, CompiledRouteCandidate, CompiledRouteConfiguration, RouteCompileError,
    RouteCompiler,
};

const SYNTHETIC_BOOTSTRAP_VERSION: &str = "bootstrap-empty";

/// Publishes compiler-approved Config Versions through the runtime Snapshot registry.
#[derive(Clone, Debug)]
pub struct SnapshotPublicationService {
    compiler: RouteCompiler,
    registry: Arc<RouteSnapshotRegistry>,
    synthetic_bootstrap_version: Option<SnapshotVersion>,
}

impl SnapshotPublicationService {
    /// Creates a service with immutable compiler evidence and a shared Snapshot registry.
    #[must_use]
    pub fn new(compiler: RouteCompiler, registry: Arc<RouteSnapshotRegistry>) -> Self {
        Self {
            compiler,
            registry,
            synthetic_bootstrap_version: None,
        }
    }

    /// Returns the shared runtime registry used by this service.
    #[must_use]
    pub fn registry(&self) -> &Arc<RouteSnapshotRegistry> {
        &self.registry
    }

    /// Rebuilds a publisher and Snapshot registry from the persisted active Config Version.
    ///
    /// When no Version has ever been published, the registry starts from a deliberately empty
    /// synthetic Snapshot. It exposes no models, Routes, Access Groups, or Client Keys, so it is a
    /// safe pre-publication state rather than a database-backed configuration. The first publish
    /// cannot roll back to that synthetic state; a second persisted publication restores the
    /// normal one-step rollback behavior.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the active configuration or its audit-recorded predecessor
    /// cannot compile into a safe Snapshot. This fails closed instead of starting with an
    /// unvalidated runtime view.
    pub fn bootstrap(
        compiler: RouteCompiler,
        repository: &mut SqliteControlPlaneRepository,
    ) -> Result<Self, SnapshotPublicationError> {
        let (current, previous, synthetic_bootstrap_version) =
            if let Some(active_configuration) = repository.load_active_configuration()? {
                let current = Arc::new(compiled_snapshot(&compiler, &active_configuration)?);
                let previous = repository
                    .load_rollback_predecessor(&active_configuration.version.id)?
                    .map(|configuration| compiled_snapshot(&compiler, &configuration).map(Arc::new))
                    .transpose()?;
                (current, previous, None)
            } else {
                let current = Arc::new(empty_bootstrap_snapshot()?);
                let synthetic_bootstrap_version = Some(current.version().clone());
                (current, None, synthetic_bootstrap_version)
            };
        let registry = Arc::new(RouteSnapshotRegistry::try_new_with_previous(
            current, previous,
        )?);
        Ok(Self {
            compiler,
            registry,
            synthetic_bootstrap_version,
        })
    }

    /// Validates a persisted Version through `EgressPolicy`, `RouteCompiler`, and complete
    /// router-safe Snapshot construction without changing durable or runtime state.
    ///
    /// # Errors
    ///
    /// Returns the same safe validation errors as publication while retaining the current active
    /// Version and Snapshot unchanged.
    pub fn validate_version(
        &self,
        repository: &mut SqliteControlPlaneRepository,
        config_version_id: &ConfigVersionId,
    ) -> Result<SnapshotValidation, SnapshotPublicationError> {
        let configuration = repository
            .load_configuration(config_version_id)?
            .ok_or(SnapshotPublicationError::ConfigVersionNotFound)?;
        let snapshot = compiled_snapshot(&self.compiler, &configuration)?;
        Ok(SnapshotValidation {
            config_version_id: configuration.version.id,
            snapshot_version: snapshot.version().clone(),
        })
    }

    /// Compiles, atomically activates, and makes one persisted Config Version visible.
    ///
    /// Compilation and Snapshot construction run before the control-path reservation. The
    /// reservation then stays held through the `SQLite` transition; dropping it on any error leaves
    /// the current runtime Snapshot unchanged.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the Version is absent, compilation or Snapshot construction
    /// fails, the registry cannot reserve it, or `SQLite` rejects the activation. None of those
    /// failures changes the currently loaded runtime Snapshot.
    pub fn publish_version(
        &self,
        repository: &mut SqliteControlPlaneRepository,
        config_version_id: &ConfigVersionId,
    ) -> Result<SnapshotPublication, SnapshotPublicationError> {
        self.publish_version_with_optional_audit(repository, config_version_id, None)
    }

    /// Publishes one Config Version and appends a matching durable audit event in the same
    /// `SQLite` transaction as activation, before committing the `ArcSwap` replacement.
    ///
    /// # Errors
    ///
    /// Returns the same typed failures as [`Self::publish_version`]. A failed audit append also
    /// leaves both the persisted active Version and the current Snapshot unchanged.
    pub fn publish_version_with_audit(
        &self,
        repository: &mut SqliteControlPlaneRepository,
        config_version_id: &ConfigVersionId,
        audit_draft: &ManagementAuditEventDraft,
    ) -> Result<SnapshotPublication, SnapshotPublicationError> {
        if audit_draft.action() != gateway_store::control_plane::ManagementAuditAction::Published {
            return Err(StoreError::InvalidManagementAuditEvent.into());
        }
        self.publish_version_with_optional_audit(repository, config_version_id, Some(audit_draft))
    }

    fn publish_version_with_optional_audit(
        &self,
        repository: &mut SqliteControlPlaneRepository,
        config_version_id: &ConfigVersionId,
        audit_draft: Option<&ManagementAuditEventDraft>,
    ) -> Result<SnapshotPublication, SnapshotPublicationError> {
        let configuration = repository
            .load_configuration(config_version_id)?
            .ok_or(SnapshotPublicationError::ConfigVersionNotFound)?;
        let replacement = Arc::new(compiled_snapshot(&self.compiler, &configuration)?);
        let prepared = self.registry.prepare_publication(replacement)?;
        let (activation, audit_event) = match audit_draft {
            Some(audit_draft) => {
                let (activation, audit_event) =
                    repository.activate_version_with_audit(config_version_id, audit_draft)?;
                (activation, Some(audit_event))
            }
            None => (repository.activate_version(config_version_id)?, None),
        };
        let transition = prepared.commit();

        Ok(SnapshotPublication {
            activation,
            transition,
            audit_event,
        })
    }

    /// Atomically restores the registry's immediately preceding Config Version.
    ///
    /// # Errors
    ///
    /// Returns a typed error when no preceding Snapshot is retained, its Version cannot be
    /// represented by the Repository, or `SQLite` rejects the matching activation. In all error
    /// cases the current runtime Snapshot remains unchanged.
    pub fn rollback(
        &self,
        repository: &mut SqliteControlPlaneRepository,
    ) -> Result<SnapshotPublication, SnapshotPublicationError> {
        self.rollback_with_optional_audit(repository, None)
    }

    /// Restores the retained predecessor and records its matching durable rollback audit event.
    ///
    /// # Errors
    ///
    /// Returns the same typed failures as [`Self::rollback`]. The audit event and status
    /// transition are one `SQLite` transaction and both occur before the in-memory commit.
    pub fn rollback_with_audit(
        &self,
        repository: &mut SqliteControlPlaneRepository,
        audit_draft: &ManagementAuditEventDraft,
    ) -> Result<SnapshotPublication, SnapshotPublicationError> {
        if audit_draft.action() != gateway_store::control_plane::ManagementAuditAction::RolledBack {
            return Err(StoreError::InvalidManagementAuditEvent.into());
        }
        self.rollback_with_optional_audit(repository, Some(audit_draft))
    }

    fn rollback_with_optional_audit(
        &self,
        repository: &mut SqliteControlPlaneRepository,
        audit_draft: Option<&ManagementAuditEventDraft>,
    ) -> Result<SnapshotPublication, SnapshotPublicationError> {
        let prepared = self.registry.prepare_rollback()?;
        if self
            .synthetic_bootstrap_version
            .as_ref()
            .is_some_and(|version| prepared.target_version() == version)
        {
            return Err(SnapshotPublicationError::NoPersistedRollbackTarget);
        }
        let target_version =
            ConfigVersionId::try_new(prepared.target_version().as_str().to_owned())
                .map_err(|_| SnapshotPublicationError::InvalidSnapshotVersion)?;
        let (activation, audit_event) = match audit_draft {
            Some(audit_draft) => {
                let (activation, audit_event) =
                    repository.activate_version_with_audit(&target_version, audit_draft)?;
                (activation, Some(audit_event))
            }
            None => (repository.activate_version(&target_version)?, None),
        };
        let transition = prepared.commit();

        Ok(SnapshotPublication {
            activation,
            transition,
            audit_event,
        })
    }
}

/// Evidence that a persisted Version can become a complete immutable Snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotValidation {
    config_version_id: ConfigVersionId,
    snapshot_version: SnapshotVersion,
}

impl SnapshotValidation {
    /// Returns the persisted Config Version that completed validation.
    #[must_use]
    pub fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Returns the matching immutable Snapshot Version that was constructed transiently.
    #[must_use]
    pub fn snapshot_version(&self) -> &SnapshotVersion {
        &self.snapshot_version
    }
}

/// Durable and in-memory evidence for one successful publication or rollback transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotPublication {
    activation: ConfigVersionActivation,
    transition: SnapshotTransition,
    audit_event: Option<ManagementAuditEvent>,
}

impl SnapshotPublication {
    /// Returns the durable Config Version status transition.
    #[must_use]
    pub fn activation(&self) -> &ConfigVersionActivation {
        &self.activation
    }

    /// Returns the in-memory immutable Snapshot transition.
    #[must_use]
    pub fn transition(&self) -> &SnapshotTransition {
        &self.transition
    }

    /// Returns the durable management audit event recorded with this transition, if the caller
    /// selected an audited publication API.
    #[must_use]
    pub fn audit_event(&self) -> Option<&ManagementAuditEvent> {
        self.audit_event.as_ref()
    }
}

/// Safe failures emitted by Config Version Snapshot publication.
#[derive(Debug)]
pub enum SnapshotPublicationError {
    /// The requested Config Version was not stored.
    ConfigVersionNotFound,
    /// P2-06 rejected the complete persisted graph.
    Compile(RouteCompileError),
    /// P2-09 rejected an `EgressPolicy` or an enabled Upstream/Endpoint's static egress shape.
    EgressPolicy(EgressPolicyCompileError),
    /// The compiler output did not meet the router-safe Snapshot boundary.
    SnapshotBuild(RouteSnapshotBuildError),
    /// The runtime publication registry could not reserve the requested transition.
    Registry(SnapshotRegistryError),
    /// `SQLite` rejected the requested Config Version transition.
    Store(StoreError),
    /// A retained Snapshot Version could not be converted back to a persisted identifier.
    InvalidSnapshotVersion,
    /// The only retained predecessor is the safe synthetic Snapshot used before first publish.
    NoPersistedRollbackTarget,
    /// A stored Client Key record could not become a safe runtime HMAC view.
    ClientKeyMaterial(ClientKeyError),
    /// A stored Client Key referred to an Access Group absent from its persisted Version.
    ClientKeyAccessGroupMissing,
}

impl fmt::Display for SnapshotPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigVersionNotFound => {
                formatter.write_str("requested Config Version was not found")
            }
            Self::Compile(error) => write!(formatter, "RouteSnapshot compilation failed: {error}"),
            Self::EgressPolicy(error) => {
                write!(formatter, "EgressPolicy compilation failed: {error}")
            }
            Self::SnapshotBuild(error) => {
                write!(formatter, "RouteSnapshot construction failed: {error}")
            }
            Self::Registry(error) => write!(formatter, "RouteSnapshot publication failed: {error}"),
            Self::Store(error) => write!(formatter, "Config Version activation failed: {error}"),
            Self::InvalidSnapshotVersion => {
                formatter.write_str("retained Snapshot Version is not a valid Config Version")
            }
            Self::NoPersistedRollbackTarget => {
                formatter.write_str("no persisted Config Version is available for rollback")
            }
            Self::ClientKeyMaterial(error) => {
                write!(
                    formatter,
                    "Snapshot Client Key construction failed: {error}"
                )
            }
            Self::ClientKeyAccessGroupMissing => {
                formatter.write_str("Snapshot Client Key refers to a missing Access Group")
            }
        }
    }
}

impl Error for SnapshotPublicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ConfigVersionNotFound
            | Self::InvalidSnapshotVersion
            | Self::NoPersistedRollbackTarget
            | Self::ClientKeyAccessGroupMissing => None,
            Self::Compile(error) => Some(error),
            Self::EgressPolicy(error) => Some(error),
            Self::SnapshotBuild(error) => Some(error),
            Self::Registry(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::ClientKeyMaterial(error) => Some(error),
        }
    }
}

impl From<RouteCompileError> for SnapshotPublicationError {
    fn from(error: RouteCompileError) -> Self {
        Self::Compile(error)
    }
}

impl From<EgressPolicyCompileError> for SnapshotPublicationError {
    fn from(error: EgressPolicyCompileError) -> Self {
        Self::EgressPolicy(error)
    }
}

impl From<RouteSnapshotBuildError> for SnapshotPublicationError {
    fn from(error: RouteSnapshotBuildError) -> Self {
        Self::SnapshotBuild(error)
    }
}

impl From<SnapshotRegistryError> for SnapshotPublicationError {
    fn from(error: SnapshotRegistryError) -> Self {
        Self::Registry(error)
    }
}

impl From<StoreError> for SnapshotPublicationError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<ClientKeyError> for SnapshotPublicationError {
    fn from(error: ClientKeyError) -> Self {
        Self::ClientKeyMaterial(error)
    }
}

fn compiled_snapshot(
    compiler: &RouteCompiler,
    configuration: &ControlPlaneConfiguration,
) -> Result<RouteSnapshot, SnapshotPublicationError> {
    EgressPolicyCompiler::compile(configuration)?;
    let compiled_configuration = compiler.compile(configuration)?;
    route_snapshot_from_compiled(&compiled_configuration, configuration)
}

fn empty_bootstrap_snapshot() -> Result<RouteSnapshot, SnapshotPublicationError> {
    let version = SnapshotVersion::try_new(SYNTHETIC_BOOTSTRAP_VERSION)
        .map_err(|_| SnapshotPublicationError::InvalidSnapshotVersion)?;
    RouteSnapshot::try_new(RouteSnapshotInput::new(
        version,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ))
    .map_err(SnapshotPublicationError::from)
}

fn route_snapshot_from_compiled(
    compiled: &CompiledRouteConfiguration,
    configuration: &ControlPlaneConfiguration,
) -> Result<RouteSnapshot, SnapshotPublicationError> {
    let version = SnapshotVersion::try_new(compiled.config_version_id().as_str().to_owned())
        .map_err(|_| SnapshotPublicationError::InvalidSnapshotVersion)?;
    let public_models = compiled
        .public_models()
        .map(|public_model| {
            gateway_router::SnapshotPublicModel::new(
                public_model.id().clone(),
                public_model.model_name().to_owned(),
                public_model.display_name().to_owned(),
                public_model.required_capabilities().clone(),
                public_model.route_id().clone(),
            )
        })
        .collect();
    let aliases = compiled
        .aliases()
        .map(|(alias, public_model_id)| (alias.to_owned(), public_model_id.clone()))
        .collect();
    let routes = compiled
        .routes()
        .map(|route| {
            SnapshotRoute::new(
                route.id().clone(),
                route.public_model_id().clone(),
                snapshot_route_policy(route.policy()),
                route.max_attempts(),
                route.bootstrap_timeout_ms(),
                route
                    .candidates()
                    .iter()
                    .map(snapshot_route_candidate)
                    .collect(),
            )
        })
        .collect();
    let access_groups = compiled
        .access_groups()
        .map(|access_group| {
            SnapshotAccessGroup::new(
                access_group.id().clone(),
                access_group.name().to_owned(),
                access_group.allowed_route_ids().cloned().collect(),
            )
        })
        .collect();
    let client_keys = snapshot_client_keys_from_configuration(compiled, configuration)?;

    Ok(RouteSnapshot::try_new(RouteSnapshotInput::new(
        version,
        public_models,
        aliases,
        routes,
        access_groups,
        client_keys,
    ))?)
}

fn snapshot_client_keys_from_configuration(
    compiled: &CompiledRouteConfiguration,
    configuration: &ControlPlaneConfiguration,
) -> Result<Vec<SnapshotClientKeyView>, SnapshotPublicationError> {
    let mut client_keys = Vec::new();
    for stored_client_key in &configuration.client_keys {
        let persisted_access_group_exists = configuration
            .access_groups
            .iter()
            .any(|access_group| access_group.id == *stored_client_key.access_group_id());
        if !persisted_access_group_exists {
            return Err(SnapshotPublicationError::ClientKeyAccessGroupMissing);
        }
        let Some(access_group) = compiled.access_group(stored_client_key.access_group_id()) else {
            continue;
        };
        client_keys.push(SnapshotClientKeyView::new(
            client_key_record(stored_client_key)?,
            access_group.allowed_route_ids().cloned().collect(),
        ));
    }
    Ok(client_keys)
}

fn client_key_record(
    stored_client_key: &StoredClientKey,
) -> Result<ClientKeyRecord, SnapshotPublicationError> {
    Ok(ClientKeyRecord::try_new(
        stored_client_key.id().clone(),
        stored_client_key.access_group_id().clone(),
        ClientKeyPrefix::try_new(stored_client_key.prefix().to_owned())?,
        ClientKeyDigest::try_from_persisted(stored_client_key.secret_digest())?,
        snapshot_client_key_status(stored_client_key.status()),
        stored_client_key.expires_at_ms(),
    )?)
}

const fn snapshot_client_key_status(status: StoredClientKeyStatus) -> ClientKeyStatus {
    match status {
        StoredClientKeyStatus::Active => ClientKeyStatus::Active,
        StoredClientKeyStatus::Disabled => ClientKeyStatus::Disabled,
        StoredClientKeyStatus::Revoked => ClientKeyStatus::Revoked,
    }
}

const fn snapshot_route_policy(policy: RoutePolicy) -> SnapshotRoutePolicy {
    match policy {
        RoutePolicy::RoundRobin => SnapshotRoutePolicy::RoundRobin,
        RoutePolicy::SmoothWeightedRoundRobin => SnapshotRoutePolicy::SmoothWeightedRoundRobin,
        RoutePolicy::PriorityFailover => SnapshotRoutePolicy::PriorityFailover,
    }
}

const fn snapshot_transform_mode(mode: TransformMode) -> SnapshotTransformMode {
    match mode {
        TransformMode::Passthrough => SnapshotTransformMode::Passthrough,
        TransformMode::Canonical => SnapshotTransformMode::Canonical,
        TransformMode::LosslessBridge => SnapshotTransformMode::LosslessBridge,
    }
}

const fn snapshot_catalog_admission(admission: CatalogAdmission) -> SnapshotCatalogAdmission {
    match admission {
        CatalogAdmission::Listed(state) => SnapshotCatalogAdmission::Listed(state),
        CatalogAdmission::AllowedUnlisted => SnapshotCatalogAdmission::AllowedUnlisted,
    }
}

fn snapshot_route_candidate(candidate: &CompiledRouteCandidate) -> SnapshotRouteCandidate {
    SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
        id: candidate.id().clone(),
        endpoint_id: candidate.endpoint_id().clone(),
        upstream_id: candidate.upstream_id().clone(),
        upstream_model: candidate.upstream_model().to_owned(),
        transform_mode: snapshot_transform_mode(candidate.transform_mode()),
        priority: candidate.priority(),
        weight: candidate.weight(),
        effective_capabilities: candidate.effective_capabilities().clone(),
        catalog_admission: snapshot_catalog_admission(candidate.catalog_admission()),
        active_binding_count: candidate.active_binding_count(),
    })
}

#[cfg(test)]
mod tests {
    use std::{error::Error, io, sync::Arc};

    use gateway_auth::client_key::ClientKeyPrefix;
    use gateway_catalog::{
        CapabilitySet, CatalogModelEntry, CatalogModelState, CatalogView, EndpointCapabilityEntry,
        EndpointCapabilityView, SemanticCapability,
    };
    use gateway_core::{
        AccessGroupId, ClientKeyId, CredentialId, EgressPolicyId, EndpointId, PublicModelId,
        RouteCandidateId, RouteId, UpstreamId,
    };
    use gateway_router::RouteSnapshotRegistry;
    use gateway_store::{
        control_plane::{
            AccessGroupConfiguration, AccessGroupRouteConfiguration, AdministrativeStatus,
            ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialConfiguration, CredentialScope, CredentialStatus, EgressPolicyConfiguration,
            EndpointConfiguration, EndpointCredentialBindingConfiguration, EndpointTransport,
            ManagementAuditAction, ManagementAuditEventDraft, ModelAliasConfiguration,
            ModelRouteConfiguration, PublicModelConfiguration, RouteCandidateConfiguration,
            RoutePolicy, SqliteControlPlaneRepository, StoredClientKey, StoredClientKeyStatus,
            StoredEgressRedirectMode, TransformMode, UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };

    use super::{
        SnapshotPublicationError, SnapshotPublicationService, route_snapshot_from_compiled,
    };
    use crate::route_compiler::RouteCompiler;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn publishes_versions_and_rolls_back_the_persisted_and_in_memory_pair() -> TestResult {
        let compiler = compiler()?;
        let bootstrap_configuration = configuration("bootstrap")?;
        let bootstrap_snapshot = Arc::new(route_snapshot_from_compiled(
            &compiler.compile(&bootstrap_configuration)?,
            &bootstrap_configuration,
        )?);
        let registry = Arc::new(RouteSnapshotRegistry::new(bootstrap_snapshot));
        let service = SnapshotPublicationService::new(compiler, Arc::clone(&registry));
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        let version_one = ConfigVersionId::try_new("version-one")?;
        let version_two = ConfigVersionId::try_new("version-two")?;
        repository.write_configuration(&configuration(version_one.as_str())?)?;
        repository.write_configuration(&configuration(version_two.as_str())?)?;

        let first_publication = service.publish_version(&mut repository, &version_one)?;
        assert_eq!(
            first_publication.activation().activated_version_id(),
            &version_one
        );
        assert_eq!(
            first_publication.activation().replaced_active_version_id(),
            None
        );
        assert_eq!(
            first_publication.transition().current_version().as_str(),
            version_one.as_str()
        );
        assert_eq!(
            version_status(&mut repository, &version_one)?,
            ConfigVersionStatus::Active
        );

        let second_publication = service.publish_version(&mut repository, &version_two)?;
        assert_eq!(
            second_publication.activation().activated_version_id(),
            &version_two
        );
        assert_eq!(
            second_publication.activation().replaced_active_version_id(),
            Some(&version_one)
        );
        assert_eq!(
            version_status(&mut repository, &version_one)?,
            ConfigVersionStatus::Archived
        );
        assert_eq!(
            version_status(&mut repository, &version_two)?,
            ConfigVersionStatus::Active
        );
        assert_eq!(registry.load().version().as_str(), version_two.as_str());

        let rollback = service.rollback(&mut repository)?;
        assert_eq!(rollback.activation().activated_version_id(), &version_one);
        assert_eq!(
            rollback.activation().replaced_active_version_id(),
            Some(&version_two)
        );
        assert_eq!(
            version_status(&mut repository, &version_one)?,
            ConfigVersionStatus::Active
        );
        assert_eq!(
            version_status(&mut repository, &version_two)?,
            ConfigVersionStatus::Archived
        );
        let current_snapshot = registry.load();
        assert_eq!(current_snapshot.version().as_str(), version_one.as_str());
        let client_key = current_snapshot
            .client_key(&ClientKeyPrefix::try_new("rgw_0123456789abcdef")?)
            .ok_or("expected compiled Client Key view")?;
        assert_eq!(client_key.client_key_id().as_str(), "client-key-a");
        assert_eq!(client_key.access_group_id().as_str(), "access-group-a");
        assert!(client_key.permits_route(&RouteId::try_new("route-a")?));

        let snapshot_debug = format!("{current_snapshot:?}");
        assert!(!snapshot_debug.contains("synthetic-credential"));
        assert!(!snapshot_debug.contains("ciphertext"));
        assert!(snapshot_debug.contains("<redacted>"));
        Ok(())
    }

    #[test]
    fn compile_failure_leaves_the_active_snapshot_and_draft_version_unchanged() -> TestResult {
        let compiler = compiler()?;
        let bootstrap_configuration = configuration("bootstrap")?;
        let bootstrap_snapshot = Arc::new(route_snapshot_from_compiled(
            &compiler.compile(&bootstrap_configuration)?,
            &bootstrap_configuration,
        )?);
        let registry = Arc::new(RouteSnapshotRegistry::new(bootstrap_snapshot));
        let service = SnapshotPublicationService::new(compiler, Arc::clone(&registry));
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        let active_version = ConfigVersionId::try_new("active-version")?;
        let invalid_version = ConfigVersionId::try_new("invalid-version")?;
        repository.write_configuration(&configuration(active_version.as_str())?)?;
        let mut invalid_configuration = configuration(invalid_version.as_str())?;
        invalid_configuration.public_models[0].capabilities_json =
            r#"{"tools":"invalid"}"#.to_owned();
        repository.write_configuration(&invalid_configuration)?;

        service.publish_version(&mut repository, &active_version)?;
        let publication = service.publish_version(&mut repository, &invalid_version);

        assert!(matches!(
            publication,
            Err(SnapshotPublicationError::Compile(_))
        ));
        assert_eq!(registry.load().version().as_str(), active_version.as_str());
        assert_eq!(
            version_status(&mut repository, &active_version)?,
            ConfigVersionStatus::Active
        );
        assert_eq!(
            version_status(&mut repository, &invalid_version)?,
            ConfigVersionStatus::Draft
        );
        Ok(())
    }

    #[test]
    fn disabled_access_group_removes_its_client_key_from_the_runtime_snapshot() -> TestResult {
        let compiler = compiler()?;
        let mut configuration = configuration("disabled-access-group")?;
        configuration.access_groups[0].status = AdministrativeStatus::Disabled;

        let snapshot =
            route_snapshot_from_compiled(&compiler.compile(&configuration)?, &configuration)?;

        assert_eq!(snapshot.client_keys().count(), 0);
        Ok(())
    }

    #[test]
    fn egress_policy_rejection_keeps_the_active_snapshot_and_draft_unchanged() -> TestResult {
        let compiler = compiler()?;
        let bootstrap_configuration = configuration("egress-bootstrap")?;
        let bootstrap_snapshot = Arc::new(route_snapshot_from_compiled(
            &compiler.compile(&bootstrap_configuration)?,
            &bootstrap_configuration,
        )?);
        let registry = Arc::new(RouteSnapshotRegistry::new(bootstrap_snapshot));
        let service = SnapshotPublicationService::new(compiler, Arc::clone(&registry));
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        let active_version = ConfigVersionId::try_new("egress-active")?;
        let rejected_version = ConfigVersionId::try_new("egress-rejected")?;
        repository.write_configuration(&configuration(active_version.as_str())?)?;
        let mut rejected_configuration = configuration(rejected_version.as_str())?;
        rejected_configuration.upstreams[0].egress_policy_id = None;
        repository.write_configuration(&rejected_configuration)?;

        service.publish_version(&mut repository, &active_version)?;
        let publication = service.publish_version(&mut repository, &rejected_version);

        assert!(matches!(
            publication,
            Err(SnapshotPublicationError::EgressPolicy(_))
        ));
        assert_eq!(registry.load().version().as_str(), active_version.as_str());
        assert_eq!(
            version_status(&mut repository, &active_version)?,
            ConfigVersionStatus::Active
        );
        assert_eq!(
            version_status(&mut repository, &rejected_version)?,
            ConfigVersionStatus::Draft
        );
        Ok(())
    }

    #[test]
    fn malformed_persisted_client_key_material_leaves_the_active_snapshot_and_draft_unchanged()
    -> TestResult {
        let compiler = compiler()?;
        let bootstrap_configuration = configuration("bootstrap")?;
        let bootstrap_snapshot = Arc::new(route_snapshot_from_compiled(
            &compiler.compile(&bootstrap_configuration)?,
            &bootstrap_configuration,
        )?);
        let registry = Arc::new(RouteSnapshotRegistry::new(bootstrap_snapshot));
        let service = SnapshotPublicationService::new(compiler, Arc::clone(&registry));
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        let active_version = ConfigVersionId::try_new("active-version")?;
        let malformed_version = ConfigVersionId::try_new("malformed-client-key")?;
        repository.write_configuration(&configuration(active_version.as_str())?)?;
        repository.write_configuration(&configuration_with_client_key_prefix(
            malformed_version.as_str(),
            "not-a-canonical-prefix",
        )?)?;

        service.publish_version(&mut repository, &active_version)?;
        let publication = service.publish_version(&mut repository, &malformed_version);

        assert!(matches!(
            publication,
            Err(SnapshotPublicationError::ClientKeyMaterial(_))
        ));
        assert_eq!(registry.load().version().as_str(), active_version.as_str());
        assert_eq!(
            version_status(&mut repository, &active_version)?,
            ConfigVersionStatus::Active
        );
        assert_eq!(
            version_status(&mut repository, &malformed_version)?,
            ConfigVersionStatus::Draft
        );
        Ok(())
    }

    #[test]
    fn publication_rejects_a_rollback_audit_action_before_activation() -> TestResult {
        let compiler = compiler()?;
        let bootstrap_configuration = configuration("bootstrap")?;
        let bootstrap_snapshot = Arc::new(route_snapshot_from_compiled(
            &compiler.compile(&bootstrap_configuration)?,
            &bootstrap_configuration,
        )?);
        let registry = Arc::new(RouteSnapshotRegistry::new(bootstrap_snapshot));
        let service = SnapshotPublicationService::new(compiler, Arc::clone(&registry));
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        let version = ConfigVersionId::try_new("version-one")?;
        repository.write_configuration(&configuration(version.as_str())?)?;
        let wrong_audit = ManagementAuditEventDraft::try_new(
            ManagementAuditAction::RolledBack,
            "test-operator",
            1,
        )?;

        let publication =
            service.publish_version_with_audit(&mut repository, &version, &wrong_audit);

        assert!(matches!(
            publication,
            Err(SnapshotPublicationError::Store(_))
        ));
        assert_eq!(registry.load().version().as_str(), "bootstrap");
        assert_eq!(
            version_status(&mut repository, &version)?,
            ConfigVersionStatus::Draft
        );
        Ok(())
    }

    fn compiler() -> Result<RouteCompiler, Box<dyn Error>> {
        let endpoint_id = EndpointId::try_new("endpoint-a")?;
        let catalog = CatalogView::try_new([CatalogModelEntry::try_new(
            endpoint_id.clone(),
            "upstream-model-a",
            CatalogModelState::Fresh,
        )?])?;
        let endpoint_capabilities = EndpointCapabilityView::try_new([EndpointCapabilityEntry {
            endpoint_id,
            capabilities: CapabilitySet::try_new([
                SemanticCapability::Tools,
                SemanticCapability::Streaming,
            ])?,
        }])?;
        Ok(RouteCompiler::new(catalog, endpoint_capabilities))
    }

    fn configuration(version: &str) -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
        configuration_with_client_key_prefix(version, "rgw_0123456789abcdef")
    }

    fn configuration_with_client_key_prefix(
        version: &str,
        client_key_prefix: &str,
    ) -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
        let version_id = ConfigVersionId::try_new(version.to_owned())?;
        let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
            id: version_id,
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            created_at_ms: 1,
            description: "P2-07 publication fixture".to_owned(),
        });
        add_egress_bound_upstream(&mut configuration)?;
        configuration.endpoints.push(EndpointConfiguration {
            id: EndpointId::try_new("endpoint-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            adapter_id: "openai-compatible.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: "https://station.example/v1".to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: Some("/models".to_owned()),
            transport: EndpointTransport::Http,
            enabled: true,
        });
        configuration.credentials.push(CredentialConfiguration {
            id: CredentialId::try_new("credential-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            kind: "api_key".to_owned(),
            encrypted_secret: encrypted_fixture_secret()?,
            status: CredentialStatus::Active,
            revision: 0,
        });
        configuration
            .endpoint_credential_bindings
            .push(EndpointCredentialBindingConfiguration {
                endpoint_id: EndpointId::try_new("endpoint-a")?,
                credential_id: CredentialId::try_new("credential-a")?,
                upstream_id: UpstreamId::try_new("upstream-a")?,
                enabled: true,
                priority: 0,
                weight: 1,
                concurrency: 2,
            });
        configuration.public_models.push(PublicModelConfiguration {
            id: PublicModelId::try_new("public-model-a")?,
            model_name: "public-model".to_owned(),
            status: AdministrativeStatus::Active,
            display_name: "Public Model".to_owned(),
            capabilities_json: r#"{"tools":true,"streaming":true}"#.to_owned(),
        });
        configuration.model_aliases.push(ModelAliasConfiguration {
            alias: "model-alias".to_owned(),
            public_model_id: PublicModelId::try_new("public-model-a")?,
        });
        configuration.model_routes.push(ModelRouteConfiguration {
            id: RouteId::try_new("route-a")?,
            public_model_id: PublicModelId::try_new("public-model-a")?,
            policy: RoutePolicy::SmoothWeightedRoundRobin,
            max_attempts: 2,
            bootstrap_timeout_ms: 10_000,
        });
        configuration
            .route_candidates
            .push(RouteCandidateConfiguration {
                id: RouteCandidateId::try_new("candidate-a")?,
                route_id: RouteId::try_new("route-a")?,
                endpoint_id: EndpointId::try_new("endpoint-a")?,
                upstream_model: "upstream-model-a".to_owned(),
                credential_scope: CredentialScope::EndpointBindings,
                transform_mode: TransformMode::Canonical,
                enabled: true,
                priority: 0,
                weight: 1,
                capability_override_json: "{}".to_owned(),
            });
        configuration.access_groups.push(AccessGroupConfiguration {
            id: AccessGroupId::try_new("access-group-a")?,
            name: "default".to_owned(),
            status: AdministrativeStatus::Active,
            limits_json: "{}".to_owned(),
        });
        configuration
            .access_group_routes
            .push(AccessGroupRouteConfiguration {
                access_group_id: AccessGroupId::try_new("access-group-a")?,
                route_id: RouteId::try_new("route-a")?,
                enabled: true,
            });
        configuration.client_keys.push(StoredClientKey::try_new(
            ClientKeyId::try_new("client-key-a")?,
            AccessGroupId::try_new("access-group-a")?,
            client_key_prefix,
            [0xA5_u8; 32],
            StoredClientKeyStatus::Active,
            None,
        )?);
        Ok(configuration)
    }

    fn add_egress_bound_upstream(
        configuration: &mut ControlPlaneConfiguration,
    ) -> Result<(), Box<dyn Error>> {
        let egress_policy_id = EgressPolicyId::try_new("egress-policy-a")?;
        configuration
            .egress_policies
            .push(EgressPolicyConfiguration {
                id: egress_policy_id.clone(),
                name: "default-egress".to_owned(),
                allowed_schemes_json: r#"["https"]"#.to_owned(),
                allowed_hosts_json: r#"["station.example"]"#.to_owned(),
                allowed_ports_json: "[443]".to_owned(),
                allowed_cidrs_json: "[]".to_owned(),
                redirect_mode: StoredEgressRedirectMode::Deny,
                max_redirects: 0,
            });
        configuration.upstreams.push(UpstreamConfiguration {
            id: UpstreamId::try_new("upstream-a")?,
            name: "station-a".to_owned(),
            kind: "openai-compatible".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: Some(egress_policy_id),
        });
        Ok(())
    }

    fn encrypted_fixture_secret()
    -> Result<gateway_store::secret_store::EncryptedSecret, Box<dyn Error>> {
        let key_version = KeyVersion::try_new(1)?;
        let key_ring = MasterKeyRing::try_new(
            key_version,
            [(key_version, MasterKey::try_from_bytes([0x44_u8; 32])?)],
        )?;
        let secret_store = SecretStore::new(key_ring);
        Ok(secret_store.seal(b"synthetic-credential", b"p2-07-fixture")?)
    }

    fn version_status(
        repository: &mut SqliteControlPlaneRepository,
        version_id: &ConfigVersionId,
    ) -> Result<ConfigVersionStatus, Box<dyn Error>> {
        repository
            .load_configuration(version_id)?
            .map(|configuration| configuration.version.status)
            .ok_or_else(|| io::Error::other("expected persisted Config Version").into())
    }
}
