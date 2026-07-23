//! Local management lifecycle API for draft creation, validation, publication, and rollback.
//!
//! This module is intentionally transport-neutral. It is the small P2-10 management boundary
//! used by the local CLI; HTTP/OpenAPI, remote management authentication, and a Web UI remain
//! P10 work. Every externally visible state change appends a non-secret durable audit event.

use std::{
    error::Error,
    fmt,
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_catalog::{CatalogView, EndpointCapabilityView};
use gateway_router::RouteSnapshotRegistry;
use gateway_store::{
    StoreError,
    control_plane::{
        ConfigVersion, ConfigVersionStatus, ControlPlaneConfiguration, ManagementAuditAction,
        ManagementAuditEventDraft, SqliteControlPlaneRepository,
    },
};

/// Management-facing Config Version identifier re-exported without making the application depend
/// directly on the persistence crate.
pub use gateway_store::control_plane::{ConfigVersionId, ManagementAuditEvent};

use crate::{
    route_compiler::RouteCompiler,
    snapshot_publisher::{
        SnapshotPublication, SnapshotPublicationError, SnapshotPublicationService,
        SnapshotValidation,
    },
};

/// A bounded non-secret label identifying the local management actor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementActor(String);

impl ManagementActor {
    /// Creates an actor label accepted by the durable audit store.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementActorError`] when the label is empty or longer than 128 Unicode scalar
    /// values. The rejected label is never included in the error.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ManagementActorError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ManagementActorError::Empty);
        }
        if value.chars().count() > 128 {
            return Err(ManagementActorError::TooLong);
        }
        Ok(Self(value))
    }

    /// Returns the safe actor label recorded with management events.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Safe actor-label construction failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementActorError {
    /// The actor label was empty.
    Empty,
    /// The actor label exceeded the persisted bound.
    TooLong,
}

impl fmt::Display for ManagementActorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("management actor must not be empty"),
            Self::TooLong => formatter.write_str("management actor exceeds the allowed length"),
        }
    }
}

impl Error for ManagementActorError {}

/// Source of Unix-millisecond timestamps for management changes and their audit records.
pub trait ManagementClock: Send + Sync {
    /// Returns the current Unix-millisecond timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementClockError::Unavailable`] when the local clock cannot be represented
    /// safely in the persisted timestamp domain.
    fn now_ms(&self) -> Result<i64, ManagementClockError>;
}

/// System clock implementation for normal local management operations.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemManagementClock;

impl ManagementClock for SystemManagementClock {
    fn now_ms(&self) -> Result<i64, ManagementClockError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ManagementClockError::Unavailable)?;
        i64::try_from(elapsed.as_millis()).map_err(|_| ManagementClockError::Unavailable)
    }
}

/// Safe system-clock failure returned before a management mutation starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementClockError {
    /// The system clock was before the Unix epoch or outside the supported range.
    Unavailable,
}

impl fmt::Display for ManagementClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("management clock is unavailable"),
        }
    }
}

impl Error for ManagementClockError {}

/// Management-only facade that owns the Repository and immutable Snapshot publisher.
///
/// The facade is deliberately not `Sync`: `SQLite` mutations remain serialized through its owned
/// repository. The request path receives only the publisher's `RouteSnapshotRegistry` and never
/// obtains this service or a `SQLite` connection.
pub struct ManagementService {
    repository: SqliteControlPlaneRepository,
    publisher: SnapshotPublicationService,
    actor: ManagementActor,
    clock: Arc<dyn ManagementClock>,
}

impl ManagementService {
    /// Rebuilds a management service from one Repository and injected compile evidence.
    ///
    /// The active Snapshot and one persisted rollback predecessor are reconstructed before the
    /// service is returned. No active Version produces only the safe empty bootstrap Snapshot.
    ///
    /// # Errors
    ///
    /// Returns a typed bootstrap error without mutating the Repository when the persisted active
    /// graph or its predecessor cannot compile safely.
    pub fn bootstrap(
        repository: SqliteControlPlaneRepository,
        compiler: RouteCompiler,
        actor: ManagementActor,
    ) -> Result<Self, ManagementServiceError> {
        Self::bootstrap_with_clock(repository, compiler, actor, Arc::new(SystemManagementClock))
    }

    /// Same as [`Self::bootstrap`] with an explicit deterministic clock for embedding or tests.
    ///
    /// # Errors
    ///
    /// Returns the same safe bootstrap failures as [`Self::bootstrap`].
    pub fn bootstrap_with_clock(
        mut repository: SqliteControlPlaneRepository,
        compiler: RouteCompiler,
        actor: ManagementActor,
        clock: Arc<dyn ManagementClock>,
    ) -> Result<Self, ManagementServiceError> {
        let publisher = SnapshotPublicationService::bootstrap(compiler, &mut repository)?;
        Ok(Self {
            repository,
            publisher,
            actor,
            clock,
        })
    }

    /// Opens a local `SQLite` database with empty injected Catalog and Endpoint-capability views.
    ///
    /// This is the intentionally narrow CLI bootstrap: it can create, validate, publish, and
    /// roll back empty scaffolding Versions. A populated Route graph requires real immutable
    /// Catalog/capability evidence supplied through [`Self::bootstrap`], which P4 later owns.
    /// The CLI does not silently fabricate that evidence.
    ///
    /// # Errors
    ///
    /// Returns a storage or safe Snapshot bootstrap error when the database cannot open, migrate,
    /// or rebuild its active runtime view.
    pub fn open_local(
        path: impl AsRef<Path>,
        actor: ManagementActor,
    ) -> Result<Self, ManagementServiceError> {
        let repository = SqliteControlPlaneRepository::open(path)?;
        let compiler =
            RouteCompiler::new(CatalogView::default(), EndpointCapabilityView::default());
        Self::bootstrap(repository, compiler, actor)
    }

    /// Creates an empty draft Config Version for a later structured configuration transaction.
    ///
    /// The empty graph is valid but exposes no Route or Client Key. It is useful for securely
    /// establishing a Version root; P10 owns broad entity CRUD rather than a risky whole-file
    /// overwrite.
    ///
    /// # Errors
    ///
    /// Returns a clock, storage, or transaction-bound audit error without creating a partial
    /// Config Version.
    pub fn create_empty_configuration(
        &mut self,
        config_version_id: ConfigVersionId,
        parent_id: Option<ConfigVersionId>,
        description: String,
    ) -> Result<ManagementConfigurationCreated, ManagementServiceError> {
        let occurred_at_ms = self.clock.now_ms()?;
        let configuration = ControlPlaneConfiguration::new(ConfigVersion {
            id: config_version_id,
            parent_id,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: occurred_at_ms,
            description,
        });
        self.create_configuration_at(&configuration, occurred_at_ms)
    }

    /// Creates one complete draft Config Version through the typed, structured management API.
    ///
    /// The configuration must be `draft`. Repository constraints and the audit append commit as
    /// one transaction, so a rejected graph never produces a misleading `config_created` event.
    ///
    /// # Errors
    ///
    /// Returns a clock, storage, or validation-of-audit-metadata error without persisting any
    /// partial configuration graph.
    pub fn create_configuration(
        &mut self,
        configuration: &ControlPlaneConfiguration,
    ) -> Result<ManagementConfigurationCreated, ManagementServiceError> {
        let occurred_at_ms = self.clock.now_ms()?;
        self.create_configuration_at(configuration, occurred_at_ms)
    }

    /// Validates a stored Config Version without activation, publication, or audit mutation.
    ///
    /// # Errors
    ///
    /// Returns the safe `EgressPolicy`, route, Snapshot, or storage failure while retaining both
    /// the current active Version and current runtime Snapshot.
    pub fn validate_configuration(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<SnapshotValidation, ManagementServiceError> {
        Ok(self
            .publisher
            .validate_version(&mut self.repository, config_version_id)?)
    }

    /// Atomically publishes one validated Config Version and records `config_published`.
    ///
    /// # Errors
    ///
    /// Returns a clock, compiler, registry, storage, or audit failure without a partial durable
    /// activation or runtime Snapshot swap.
    pub fn publish_configuration(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<SnapshotPublication, ManagementServiceError> {
        let audit_draft = self.audit_draft(ManagementAuditAction::Published)?;
        Ok(self.publisher.publish_version_with_audit(
            &mut self.repository,
            config_version_id,
            &audit_draft,
        )?)
    }

    /// Atomically restores the retained Config Version predecessor and records
    /// `config_rolled_back`.
    ///
    /// # Errors
    ///
    /// Returns a clock, missing-predecessor, storage, or audit failure while retaining the
    /// current persisted and in-memory Version.
    pub fn rollback_configuration(
        &mut self,
    ) -> Result<SnapshotPublication, ManagementServiceError> {
        let audit_draft = self.audit_draft(ManagementAuditAction::RolledBack)?;
        Ok(self
            .publisher
            .rollback_with_audit(&mut self.repository, &audit_draft)?)
    }

    /// Lists durable, non-secret management audit events in append order.
    ///
    /// # Errors
    ///
    /// Returns a safe storage error if the append-only event sequence cannot be read or decoded.
    pub fn audit_events(&mut self) -> Result<Vec<ManagementAuditEvent>, ManagementServiceError> {
        Ok(self.repository.list_management_audit_events()?)
    }

    /// Returns the immutable runtime registry used by this management process.
    #[must_use]
    pub fn registry(&self) -> &Arc<RouteSnapshotRegistry> {
        self.publisher.registry()
    }

    /// Consumes this facade and returns the owned Repository for controlled embedding or restart
    /// simulation. It is management-only and must never be passed to an inference path.
    #[must_use]
    pub fn into_repository(self) -> SqliteControlPlaneRepository {
        self.repository
    }

    fn create_configuration_at(
        &mut self,
        configuration: &ControlPlaneConfiguration,
        occurred_at_ms: i64,
    ) -> Result<ManagementConfigurationCreated, ManagementServiceError> {
        let audit_draft = ManagementAuditEventDraft::try_new(
            ManagementAuditAction::Created,
            self.actor.as_str(),
            occurred_at_ms,
        )?;
        let audit_event = self
            .repository
            .write_configuration_with_audit(configuration, &audit_draft)?;
        Ok(ManagementConfigurationCreated {
            config_version_id: configuration.version.id.clone(),
            audit_event,
        })
    }

    fn audit_draft(
        &self,
        action: ManagementAuditAction,
    ) -> Result<ManagementAuditEventDraft, ManagementServiceError> {
        Ok(ManagementAuditEventDraft::try_new(
            action,
            self.actor.as_str(),
            self.clock.now_ms()?,
        )?)
    }
}

impl fmt::Debug for ManagementService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagementService")
            .field("repository", &self.repository)
            .field("publisher", &self.publisher)
            .field("actor", &self.actor)
            .field("clock", &"<opaque>")
            .finish()
    }
}

/// Successful durable evidence for one new draft Config Version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementConfigurationCreated {
    config_version_id: ConfigVersionId,
    audit_event: ManagementAuditEvent,
}

impl ManagementConfigurationCreated {
    /// Returns the persisted draft Config Version identity.
    #[must_use]
    pub fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Returns the matching durable `config_created` audit event.
    #[must_use]
    pub fn audit_event(&self) -> &ManagementAuditEvent {
        &self.audit_event
    }
}

/// Safe management lifecycle errors.
#[derive(Debug)]
pub enum ManagementServiceError {
    /// The Store rejected a management-only operation.
    Store(StoreError),
    /// Snapshot validation, bootstrap, publication, or rollback failed safely.
    Publication(SnapshotPublicationError),
    /// The local clock was unavailable before the mutation transaction began.
    Clock(ManagementClockError),
}

impl fmt::Display for ManagementServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "management storage operation failed: {error}"),
            Self::Publication(error) => write!(formatter, "management publication failed: {error}"),
            Self::Clock(error) => write!(formatter, "management operation failed: {error}"),
        }
    }
}

impl Error for ManagementServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::Clock(error) => Some(error),
        }
    }
}

impl From<StoreError> for ManagementServiceError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<SnapshotPublicationError> for ManagementServiceError {
    fn from(error: SnapshotPublicationError) -> Self {
        Self::Publication(error)
    }
}

impl From<ManagementClockError> for ManagementServiceError {
    fn from(error: ManagementClockError) -> Self {
        Self::Clock(error)
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, sync::Arc};

    use gateway_catalog::{CatalogView, EndpointCapabilityView};
    use gateway_store::control_plane::{
        ConfigVersionId, ManagementAuditAction, ManagementAuditEvent,
    };

    use super::{
        ManagementActor, ManagementClock, ManagementClockError, ManagementService,
        ManagementServiceError, SystemManagementClock,
    };
    use crate::{route_compiler::RouteCompiler, snapshot_publisher::SnapshotPublicationError};

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn local_management_lifecycle_is_atomic_audited_and_restart_reconstructs_rollback() -> TestResult
    {
        let actor = ManagementActor::try_new("test-operator")?;
        let clock = Arc::new(FixedManagementClock { now_ms: 100 });
        let repository =
            gateway_store::control_plane::SqliteControlPlaneRepository::open_in_memory()?;
        let compiler =
            RouteCompiler::new(CatalogView::default(), EndpointCapabilityView::default());
        let mut service = ManagementService::bootstrap_with_clock(
            repository,
            compiler.clone(),
            actor.clone(),
            clock.clone(),
        )?;
        assert_eq!(
            service.registry().load().version().as_str(),
            "bootstrap-empty"
        );

        let version_one = ConfigVersionId::try_new("version-one")?;
        let created_one = service.create_empty_configuration(
            version_one.clone(),
            None,
            "first configuration".to_owned(),
        )?;
        assert_eq!(
            created_one.audit_event().action(),
            ManagementAuditAction::Created
        );
        let validation = service.validate_configuration(&version_one)?;
        assert_eq!(validation.snapshot_version().as_str(), version_one.as_str());
        let first_publication = service.publish_configuration(&version_one)?;
        assert_eq!(
            first_publication
                .audit_event()
                .ok_or("expected publication audit event")?
                .action(),
            ManagementAuditAction::Published
        );
        assert_eq!(
            service.registry().load().version().as_str(),
            version_one.as_str()
        );
        assert!(matches!(
            service.rollback_configuration(),
            Err(ManagementServiceError::Publication(
                SnapshotPublicationError::NoPersistedRollbackTarget
            ))
        ));
        assert_eq!(
            service.registry().load().version().as_str(),
            version_one.as_str()
        );

        let version_two = ConfigVersionId::try_new("version-two")?;
        service.create_empty_configuration(
            version_two.clone(),
            Some(version_one.clone()),
            "second configuration".to_owned(),
        )?;
        service.publish_configuration(&version_two)?;
        assert_eq!(
            service.registry().load().version().as_str(),
            version_two.as_str()
        );

        let repository = service.into_repository();
        let mut restarted =
            ManagementService::bootstrap_with_clock(repository, compiler, actor, clock)?;
        assert_eq!(
            restarted.registry().load().version().as_str(),
            version_two.as_str()
        );
        let rollback = restarted.rollback_configuration()?;
        assert_eq!(
            rollback
                .audit_event()
                .ok_or("expected rollback audit event")?
                .action(),
            ManagementAuditAction::RolledBack
        );
        assert_eq!(
            restarted.registry().load().version().as_str(),
            version_one.as_str()
        );

        let audit_events = restarted.audit_events()?;
        assert_eq!(audit_events.len(), 5);
        assert!(
            audit_events
                .windows(2)
                .all(|pair| pair[0].id() < pair[1].id())
        );
        assert_eq!(
            audit_events.last().map(ManagementAuditEvent::action),
            Some(ManagementAuditAction::RolledBack)
        );
        Ok(())
    }

    #[test]
    fn invalid_actor_is_rejected_without_echoing_the_value() {
        assert!(ManagementActor::try_new("").is_err());
        assert!(ManagementActor::try_new("a".repeat(129)).is_err());
    }

    #[derive(Clone, Debug)]
    struct FixedManagementClock {
        now_ms: i64,
    }

    impl ManagementClock for FixedManagementClock {
        fn now_ms(&self) -> Result<i64, ManagementClockError> {
            Ok(self.now_ms)
        }
    }

    #[test]
    fn system_clock_is_representable_for_management_events() -> TestResult {
        assert!(SystemManagementClock.now_ms()? >= 0);
        Ok(())
    }
}
