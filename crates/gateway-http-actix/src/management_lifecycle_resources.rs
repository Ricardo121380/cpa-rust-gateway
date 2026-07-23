//! Protected P10-07 Config Version lifecycle handlers.
//!
//! This module is only a narrow HTTP projection over P2-10's `ManagementService`. It owns no
//! repository, publisher, Provider, Secret, network client, or backup material. Publication and
//! rollback continue to use P2's transaction-before-`ArcSwap` lifecycle implementation.

use std::sync::Mutex;

use actix_web::{
    HttpRequest, HttpResponse,
    http::{StatusCode, header},
    web,
};
use gateway_control::{
    management_service::{
        ConfigVersion, ConfigVersionId, ConfigVersionStatus, ManagementAuditAction,
        ManagementAuditEvent, ManagementLifecycleFailure, ManagementService,
        ManagementServiceError,
    },
    snapshot_publisher::SnapshotPublication,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::management_security::configure_management;

const IF_MATCH_HEADER: &str = "if-match";
const MAX_MANAGEMENT_JSON_BYTES: usize = 70 * 1024;
const MAX_CONFIG_VERSIONS: usize = 256;
const MAX_AUDIT_EVENTS: usize = 512;

/// Isolated P10-07 state. The lifecycle facade is serialized because publication and rollback
/// are management-only control operations, never inference-path work.
pub struct ManagementLifecycleHttpState {
    lifecycle: Mutex<Box<dyn ManagementLifecycleFacade>>,
}

impl ManagementLifecycleHttpState {
    /// Creates protected lifecycle HTTP state from the real P2 management lifecycle service.
    #[must_use]
    pub fn new(service: ManagementService) -> Self {
        Self::with_facade(Box::new(ManagementServiceLifecycleFacade { service }))
    }

    /// Creates lifecycle state with an explicit bounded facade for embedding or tests.
    #[must_use]
    pub fn with_facade(lifecycle: Box<dyn ManagementLifecycleFacade>) -> Self {
        Self {
            lifecycle: Mutex::new(lifecycle),
        }
    }
}

/// Safe Config Version metadata projected to the management HTTP boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementLifecycleVersion {
    id: ConfigVersionId,
    parent_id: Option<ConfigVersionId>,
    status: ManagementLifecycleVersionStatus,
    revision: i64,
    created_at_ms: i64,
    description: String,
}

impl ManagementLifecycleVersion {
    /// Creates one bounded secret-free Config Version projection for an embedding facade.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementLifecycleError::InvalidInput`] when the caller supplies metadata that
    /// cannot be represented safely by the frozen management API.
    pub fn try_new(
        id: ConfigVersionId,
        parent_id: Option<ConfigVersionId>,
        status: ManagementLifecycleVersionStatus,
        revision: i64,
        created_at_ms: i64,
        description: String,
    ) -> Result<Self, ManagementLifecycleError> {
        if revision < 0
            || created_at_ms < 0
            || description.chars().count() > 1024
            || description.trim().is_empty()
        {
            return Err(ManagementLifecycleError::InvalidInput);
        }
        Ok(Self {
            id,
            parent_id,
            status,
            revision,
            created_at_ms,
            description,
        })
    }

    fn try_from_config(value: ConfigVersion) -> Result<Self, ManagementLifecycleError> {
        Self::try_new(
            value.id,
            value.parent_id,
            ManagementLifecycleVersionStatus::from(value.status),
            value.revision,
            value.created_at_ms,
            value.description,
        )
        .map_err(|_| ManagementLifecycleError::Unavailable)
    }

    /// Returns the opaque Config Version identity.
    #[must_use]
    pub const fn id(&self) -> &ConfigVersionId {
        &self.id
    }

    /// Returns the optional parent identity.
    #[must_use]
    pub const fn parent_id(&self) -> Option<&ConfigVersionId> {
        self.parent_id.as_ref()
    }

    /// Returns the closed lifecycle status.
    #[must_use]
    pub const fn status(&self) -> ManagementLifecycleVersionStatus {
        self.status
    }

    /// Returns the non-negative graph revision used by the frozen `If-Match` contract.
    #[must_use]
    pub const fn revision(&self) -> i64 {
        self.revision
    }

    /// Returns the safe creation timestamp.
    #[must_use]
    pub const fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    /// Returns the bounded non-secret description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
}

/// Closed Config Version lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementLifecycleVersionStatus {
    /// Structurally writable but unpublished.
    Draft,
    /// Current immutable runtime Snapshot source.
    Active,
    /// Retained historical Version.
    Archived,
}

impl From<ConfigVersionStatus> for ManagementLifecycleVersionStatus {
    fn from(value: ConfigVersionStatus) -> Self {
        match value {
            ConfigVersionStatus::Draft => Self::Draft,
            ConfigVersionStatus::Active => Self::Active,
            ConfigVersionStatus::Archived => Self::Archived,
        }
    }
}

/// Exact, bounded creation input passed to the P2 lifecycle service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementLifecycleCreate {
    id: ConfigVersionId,
    parent_id: Option<ConfigVersionId>,
    description: String,
}

impl ManagementLifecycleCreate {
    fn try_new(
        id: ConfigVersionId,
        parent_id: Option<ConfigVersionId>,
        description: String,
    ) -> Result<Self, ManagementLifecycleError> {
        if description.trim().is_empty() || description.chars().count() > 1024 {
            return Err(ManagementLifecycleError::InvalidInput);
        }
        Ok(Self {
            id,
            parent_id,
            description,
        })
    }

    /// Returns the requested Config Version identity.
    #[must_use]
    pub const fn id(&self) -> &ConfigVersionId {
        &self.id
    }

    /// Returns the optional declared parent identity.
    #[must_use]
    pub const fn parent_id(&self) -> Option<&ConfigVersionId> {
        self.parent_id.as_ref()
    }

    /// Returns the bounded non-secret description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
}

/// Value-free successful publication or rollback transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementLifecyclePublication {
    active_config_version_id: ConfigVersionId,
    replaced_config_version_id: Option<ConfigVersionId>,
}

impl ManagementLifecyclePublication {
    /// Creates one safe lifecycle transition response.
    #[must_use]
    pub const fn new(
        active_config_version_id: ConfigVersionId,
        replaced_config_version_id: Option<ConfigVersionId>,
    ) -> Self {
        Self {
            active_config_version_id,
            replaced_config_version_id,
        }
    }

    fn from_publication(value: &SnapshotPublication) -> Self {
        Self {
            active_config_version_id: value.activation().activated_version_id().clone(),
            replaced_config_version_id: value.activation().replaced_active_version_id().cloned(),
        }
    }
}

/// One bounded lifecycle audit row from P2's append-only audit stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementLifecycleAuditEvent {
    id: i64,
    action: ManagementLifecycleAuditAction,
    actor: String,
    occurred_at_ms: i64,
    config_version_id: ConfigVersionId,
    replaced_config_version_id: Option<ConfigVersionId>,
}

impl ManagementLifecycleAuditEvent {
    /// Creates one bounded lifecycle audit projection for an embedding facade.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementLifecycleError::InvalidInput`] if a caller supplies an unsafe audit
    /// projection; the rejected values are never included in the error.
    pub fn try_new(
        id: i64,
        action: ManagementLifecycleAuditAction,
        actor: String,
        occurred_at_ms: i64,
        config_version_id: ConfigVersionId,
        replaced_config_version_id: Option<ConfigVersionId>,
    ) -> Result<Self, ManagementLifecycleError> {
        if id <= 0 || occurred_at_ms < 0 || actor.is_empty() || actor.chars().count() > 128 {
            return Err(ManagementLifecycleError::InvalidInput);
        }
        Ok(Self {
            id,
            action,
            actor,
            occurred_at_ms,
            config_version_id,
            replaced_config_version_id,
        })
    }

    fn try_from_event(value: &ManagementAuditEvent) -> Result<Self, ManagementLifecycleError> {
        Self::try_new(
            value.id(),
            ManagementLifecycleAuditAction::from(value.action()),
            value.actor().to_owned(),
            value.occurred_at_ms(),
            value.config_version_id().clone(),
            value.replaced_config_version_id().cloned(),
        )
        .map_err(|_| ManagementLifecycleError::Unavailable)
    }
}

/// Closed lifecycle action type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementLifecycleAuditAction {
    /// A complete new draft was committed.
    Created,
    /// A Version was atomically activated.
    Published,
    /// The retained predecessor was atomically restored.
    RolledBack,
}

impl From<ManagementAuditAction> for ManagementLifecycleAuditAction {
    fn from(value: ManagementAuditAction) -> Self {
        match value {
            ManagementAuditAction::Created => Self::Created,
            ManagementAuditAction::Published => Self::Published,
            ManagementAuditAction::RolledBack => Self::RolledBack,
        }
    }
}

/// Safe lifecycle-facade outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementLifecycleError {
    /// Input failed before lifecycle work begins.
    InvalidInput,
    /// A valid request conflicts with current Config Version or rollback state.
    Conflict,
    /// The isolated lifecycle service cannot safely produce a result.
    Unavailable,
}

/// P10-07's only lifecycle dependency surface.
///
/// The facade cannot receive a Provider, Endpoint request client, Credential Secret/ciphertext,
/// HTTP body, backup material, or external network handle. Implementations only expose P2's
/// typed lifecycle transition and audit projections.
pub trait ManagementLifecycleFacade: Send {
    /// Lists bounded safe Config Version metadata.
    ///
    /// # Errors
    ///
    /// Returns a safe lifecycle error when the metadata cannot be read or represented.
    fn list_versions(
        &mut self,
    ) -> Result<Vec<ManagementLifecycleVersion>, ManagementLifecycleError>;

    /// Reads one Config Version metadata record.
    ///
    /// # Errors
    ///
    /// Returns a safe lifecycle error when the metadata cannot be read or represented.
    fn get_version(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<Option<ManagementLifecycleVersion>, ManagementLifecycleError>;

    /// Creates one complete empty draft and its matching P2 audit event.
    ///
    /// # Errors
    ///
    /// Returns a safe lifecycle error without a partial Version or audit append.
    fn create_version(
        &mut self,
        input: &ManagementLifecycleCreate,
    ) -> Result<ManagementLifecycleVersion, ManagementLifecycleError>;

    /// Validates exactly one Version without changing status, Snapshot, or audit state.
    ///
    /// # Errors
    ///
    /// Returns a safe lifecycle error while retaining all lifecycle state.
    fn validate_version(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<(), ManagementLifecycleError>;

    /// Publishes an exact Version only when its declared graph revision still matches.
    ///
    /// # Errors
    ///
    /// Returns a safe lifecycle error without a partial activation or Snapshot change.
    fn publish_version(
        &mut self,
        config_version_id: &ConfigVersionId,
        expected_revision: i64,
    ) -> Result<ManagementLifecyclePublication, ManagementLifecycleError>;

    /// Restores P2's retained predecessor only when the active Version revision still matches.
    ///
    /// # Errors
    ///
    /// Returns a safe lifecycle error without a partial activation or Snapshot change.
    fn rollback(
        &mut self,
        expected_revision: i64,
    ) -> Result<ManagementLifecyclePublication, ManagementLifecycleError>;

    /// Lists bounded safe P2 lifecycle audit rows only.
    ///
    /// # Errors
    ///
    /// Returns a safe lifecycle error when the durable audit projection cannot be read.
    fn audit_events(
        &mut self,
    ) -> Result<Vec<ManagementLifecycleAuditEvent>, ManagementLifecycleError>;
}

struct ManagementServiceLifecycleFacade {
    service: ManagementService,
}

impl ManagementServiceLifecycleFacade {
    fn require_draft_revision(
        &mut self,
        config_version_id: &ConfigVersionId,
        expected_revision: i64,
    ) -> Result<(), ManagementLifecycleError> {
        let Some(version) = self
            .service
            .config_version(config_version_id)
            .map_err(|error| map_service_error(&error))?
        else {
            return Err(ManagementLifecycleError::Conflict);
        };
        if version.status != ConfigVersionStatus::Draft || version.revision != expected_revision {
            return Err(ManagementLifecycleError::Conflict);
        }
        Ok(())
    }

    fn active_version(&mut self) -> Result<ManagementLifecycleVersion, ManagementLifecycleError> {
        let versions = self
            .service
            .config_versions()
            .map_err(|error| map_service_error(&error))?;
        let mut active_versions = versions
            .into_iter()
            .filter(|version| version.status == ConfigVersionStatus::Active);
        let Some(active) = active_versions.next() else {
            return Err(ManagementLifecycleError::Conflict);
        };
        if active_versions.next().is_some() {
            return Err(ManagementLifecycleError::Conflict);
        }
        ManagementLifecycleVersion::try_from_config(active)
    }
}

impl ManagementLifecycleFacade for ManagementServiceLifecycleFacade {
    fn list_versions(
        &mut self,
    ) -> Result<Vec<ManagementLifecycleVersion>, ManagementLifecycleError> {
        let versions = self
            .service
            .config_versions()
            .map_err(|error| map_service_error(&error))?;
        if versions.len() > MAX_CONFIG_VERSIONS {
            return Err(ManagementLifecycleError::Unavailable);
        }
        versions
            .into_iter()
            .map(ManagementLifecycleVersion::try_from_config)
            .collect()
    }

    fn get_version(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<Option<ManagementLifecycleVersion>, ManagementLifecycleError> {
        self.service
            .config_version(config_version_id)
            .map_err(|error| map_service_error(&error))?
            .map(ManagementLifecycleVersion::try_from_config)
            .transpose()
    }

    fn create_version(
        &mut self,
        input: &ManagementLifecycleCreate,
    ) -> Result<ManagementLifecycleVersion, ManagementLifecycleError> {
        self.service
            .create_empty_configuration(
                input.id.clone(),
                input.parent_id.clone(),
                input.description.clone(),
            )
            .map_err(|error| map_service_error(&error))?;
        self.get_version(&input.id)?
            .ok_or(ManagementLifecycleError::Unavailable)
    }

    fn validate_version(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> Result<(), ManagementLifecycleError> {
        self.service
            .validate_configuration(config_version_id)
            .map(|_| ())
            .map_err(|error| map_service_error(&error))
    }

    fn publish_version(
        &mut self,
        config_version_id: &ConfigVersionId,
        expected_revision: i64,
    ) -> Result<ManagementLifecyclePublication, ManagementLifecycleError> {
        self.require_draft_revision(config_version_id, expected_revision)?;
        self.service
            .publish_configuration(config_version_id)
            .map(|publication| ManagementLifecyclePublication::from_publication(&publication))
            .map_err(|error| map_service_error(&error))
    }

    fn rollback(
        &mut self,
        expected_revision: i64,
    ) -> Result<ManagementLifecyclePublication, ManagementLifecycleError> {
        let active = self.active_version()?;
        if active.revision() != expected_revision {
            return Err(ManagementLifecycleError::Conflict);
        }
        self.service
            .rollback_configuration()
            .map(|publication| ManagementLifecyclePublication::from_publication(&publication))
            .map_err(|error| map_service_error(&error))
    }

    fn audit_events(
        &mut self,
    ) -> Result<Vec<ManagementLifecycleAuditEvent>, ManagementLifecycleError> {
        let events = self
            .service
            .audit_events()
            .map_err(|error| map_service_error(&error))?;
        if events.len() > MAX_AUDIT_EVENTS {
            return Err(ManagementLifecycleError::Unavailable);
        }
        events
            .into_iter()
            .map(|event| ManagementLifecycleAuditEvent::try_from_event(&event))
            .collect()
    }
}

/// Default P10-07 facade. It intentionally exposes no lifecycle state or mutation until an
/// embedding provides the real P2-10 service.
pub struct RejectingManagementLifecycleFacade;

impl ManagementLifecycleFacade for RejectingManagementLifecycleFacade {
    fn list_versions(
        &mut self,
    ) -> Result<Vec<ManagementLifecycleVersion>, ManagementLifecycleError> {
        Err(ManagementLifecycleError::Unavailable)
    }

    fn get_version(
        &mut self,
        _config_version_id: &ConfigVersionId,
    ) -> Result<Option<ManagementLifecycleVersion>, ManagementLifecycleError> {
        Err(ManagementLifecycleError::Unavailable)
    }

    fn create_version(
        &mut self,
        _input: &ManagementLifecycleCreate,
    ) -> Result<ManagementLifecycleVersion, ManagementLifecycleError> {
        Err(ManagementLifecycleError::Unavailable)
    }

    fn validate_version(
        &mut self,
        _config_version_id: &ConfigVersionId,
    ) -> Result<(), ManagementLifecycleError> {
        Err(ManagementLifecycleError::Unavailable)
    }

    fn publish_version(
        &mut self,
        _config_version_id: &ConfigVersionId,
        _expected_revision: i64,
    ) -> Result<ManagementLifecyclePublication, ManagementLifecycleError> {
        Err(ManagementLifecycleError::Unavailable)
    }

    fn rollback(
        &mut self,
        _expected_revision: i64,
    ) -> Result<ManagementLifecyclePublication, ManagementLifecycleError> {
        Err(ManagementLifecycleError::Unavailable)
    }

    fn audit_events(
        &mut self,
    ) -> Result<Vec<ManagementLifecycleAuditEvent>, ManagementLifecycleError> {
        Err(ManagementLifecycleError::Unavailable)
    }
}

/// Mounts P10-07 routes inside the existing P10-02 protected `/admin` scope.
pub fn configure_management_lifecycle_resources(config: &mut web::ServiceConfig) {
    configure_management(config, lifecycle_routes);
}

fn lifecycle_routes(config: &mut web::ServiceConfig) {
    config
        .route("/config-versions", web::get().to(list_versions))
        .route("/config-versions", web::post().to(create_version))
        .route("/config-versions/rollback", web::post().to(rollback))
        .route(
            "/config-versions/{config_version_id}/validate",
            web::post().to(validate_version),
        )
        .route(
            "/config-versions/{config_version_id}/publish",
            web::post().to(publish_version),
        )
        .route(
            "/config-versions/{config_version_id}",
            web::get().to(get_version),
        )
        .route("/audit-events", web::get().to(list_audit_events));
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigVersionInput {
    id: String,
    parent_id: Option<String>,
    description: String,
}

#[derive(Serialize)]
struct ConfigVersionResponse {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<String>,
    status: &'static str,
    revision: String,
    created_at_ms: i64,
    description: String,
}

#[derive(Serialize)]
struct ValidationResponse {
    valid: bool,
    error_codes: Vec<&'static str>,
}

#[derive(Serialize)]
struct PublicationResponse {
    active_config_version_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    replaced_config_version_id: Option<String>,
}

#[derive(Serialize)]
struct AuditEventResponse {
    id: i64,
    action: &'static str,
    actor: String,
    occurred_at_ms: i64,
    config_version_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    replaced_config_version_id: Option<String>,
}

async fn list_versions(state: web::Data<ManagementLifecycleHttpState>) -> HttpResponse {
    let versions = match lifecycle(&state)
        .and_then(|mut facade| facade.list_versions().map_err(lifecycle_error))
    {
        Ok(versions) => versions,
        Err(response) => return response,
    };
    HttpResponse::Ok().json(
        versions
            .into_iter()
            .map(ConfigVersionResponse::from)
            .collect::<Vec<_>>(),
    )
}

async fn get_version(
    path: web::Path<String>,
    state: web::Data<ManagementLifecycleHttpState>,
) -> HttpResponse {
    let version_id = match config_version_id(path.into_inner()) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let version = match lifecycle(&state)
        .and_then(|mut facade| facade.get_version(&version_id).map_err(lifecycle_error))
    {
        Ok(Some(version)) => version,
        Ok(None) => return lifecycle_error(ManagementLifecycleError::Conflict),
        Err(response) => return response,
    };
    HttpResponse::Ok().json(ConfigVersionResponse::from(version))
}

async fn create_version(
    body: web::Bytes,
    state: web::Data<ManagementLifecycleHttpState>,
) -> HttpResponse {
    let input = match parse_json::<ConfigVersionInput>(&body).and_then(config_version_input) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let version = match lifecycle(&state)
        .and_then(|mut facade| facade.create_version(&input).map_err(lifecycle_error))
    {
        Ok(version) => version,
        Err(response) => return response,
    };
    HttpResponse::Created().json(ConfigVersionResponse::from(version))
}

async fn validate_version(
    path: web::Path<String>,
    state: web::Data<ManagementLifecycleHttpState>,
) -> HttpResponse {
    let version_id = match config_version_id(path.into_inner()) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match lifecycle(&state).and_then(|mut facade| {
        facade
            .validate_version(&version_id)
            .map_err(lifecycle_error)
    }) {
        Ok(()) => HttpResponse::Ok().json(ValidationResponse {
            valid: true,
            error_codes: Vec::new(),
        }),
        Err(response) => response,
    }
}

async fn publish_version(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementLifecycleHttpState>,
) -> HttpResponse {
    let version_id = match config_version_id(path.into_inner()) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let expected_revision = match expected_revision(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match lifecycle(&state).and_then(|mut facade| {
        facade
            .publish_version(&version_id, expected_revision)
            .map_err(lifecycle_error)
    }) {
        Ok(publication) => HttpResponse::Ok().json(PublicationResponse::from(publication)),
        Err(response) => response,
    }
}

async fn rollback(
    request: HttpRequest,
    state: web::Data<ManagementLifecycleHttpState>,
) -> HttpResponse {
    let expected_revision = match expected_revision(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match lifecycle(&state)
        .and_then(|mut facade| facade.rollback(expected_revision).map_err(lifecycle_error))
    {
        Ok(publication) => HttpResponse::Ok().json(PublicationResponse::from(publication)),
        Err(response) => response,
    }
}

async fn list_audit_events(state: web::Data<ManagementLifecycleHttpState>) -> HttpResponse {
    let events = match lifecycle(&state)
        .and_then(|mut facade| facade.audit_events().map_err(lifecycle_error))
    {
        Ok(events) => events,
        Err(response) => return response,
    };
    HttpResponse::Ok().json(
        events
            .into_iter()
            .map(AuditEventResponse::from)
            .collect::<Vec<_>>(),
    )
}

fn config_version_input(
    input: ConfigVersionInput,
) -> Result<ManagementLifecycleCreate, HttpResponse> {
    let id = config_version_id(input.id)?;
    let parent_id = input.parent_id.map(config_version_id).transpose()?;
    ManagementLifecycleCreate::try_new(id, parent_id, input.description).map_err(lifecycle_error)
}

fn config_version_id(value: String) -> Result<ConfigVersionId, HttpResponse> {
    if value.chars().count() > 128 || value.trim().is_empty() {
        return Err(invalid_input());
    }
    ConfigVersionId::try_new(value).map_err(|_| invalid_input())
}

fn expected_revision(request: &HttpRequest) -> Result<i64, HttpResponse> {
    let value = required_header(request, IF_MATCH_HEADER)?;
    let value = value.trim_matches('"');
    let Some(value) = value.strip_prefix("rev-") else {
        return Err(invalid_input());
    };
    value
        .parse::<i64>()
        .ok()
        .filter(|value| *value >= 0)
        .ok_or_else(invalid_input)
}

fn required_header<'request>(
    request: &'request HttpRequest,
    name: &str,
) -> Result<&'request str, HttpResponse> {
    let mut values = request.headers().get_all(name);
    let value = values.next().ok_or_else(invalid_input)?;
    if values.next().is_some() {
        return Err(invalid_input());
    }
    value
        .to_str()
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(invalid_input)
}

fn lifecycle(
    state: &web::Data<ManagementLifecycleHttpState>,
) -> Result<std::sync::MutexGuard<'_, Box<dyn ManagementLifecycleFacade>>, HttpResponse> {
    state
        .lifecycle
        .lock()
        .map_err(|_| lifecycle_error(ManagementLifecycleError::Unavailable))
}

fn parse_json<T: DeserializeOwned>(body: &[u8]) -> Result<T, HttpResponse> {
    if body.is_empty() || body.len() > MAX_MANAGEMENT_JSON_BYTES {
        return Err(invalid_input());
    }
    serde_json::from_slice(body).map_err(|_| invalid_input())
}

fn map_service_error(error: &ManagementServiceError) -> ManagementLifecycleError {
    match error.lifecycle_failure() {
        ManagementLifecycleFailure::Conflict => ManagementLifecycleError::Conflict,
        ManagementLifecycleFailure::Unavailable => ManagementLifecycleError::Unavailable,
    }
}

fn lifecycle_error(error: ManagementLifecycleError) -> HttpResponse {
    match error {
        ManagementLifecycleError::InvalidInput => invalid_input(),
        ManagementLifecycleError::Conflict => error_response(
            StatusCode::CONFLICT,
            "management_lifecycle_conflict",
            "Management lifecycle operation is not available for the current configuration",
        ),
        ManagementLifecycleError::Unavailable => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "management_lifecycle_unavailable",
            "Management lifecycle is unavailable",
        ),
    }
}

fn invalid_input() -> HttpResponse {
    error_response(
        StatusCode::BAD_REQUEST,
        "invalid_management_request",
        "Management request is invalid",
    )
}

fn error_response(status: StatusCode, code: &'static str, message: &'static str) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(serde_json::json!({"error":{"code":code,"message":message}}))
}

impl From<ManagementLifecycleVersion> for ConfigVersionResponse {
    fn from(value: ManagementLifecycleVersion) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            parent_id: value.parent_id.map(|id| id.as_str().to_owned()),
            status: match value.status {
                ManagementLifecycleVersionStatus::Draft => "draft",
                ManagementLifecycleVersionStatus::Active => "active",
                ManagementLifecycleVersionStatus::Archived => "archived",
            },
            revision: format!("rev-{}", value.revision),
            created_at_ms: value.created_at_ms,
            description: value.description,
        }
    }
}

impl From<ManagementLifecyclePublication> for PublicationResponse {
    fn from(value: ManagementLifecyclePublication) -> Self {
        Self {
            active_config_version_id: value.active_config_version_id.as_str().to_owned(),
            replaced_config_version_id: value
                .replaced_config_version_id
                .map(|id| id.as_str().to_owned()),
        }
    }
}

impl From<ManagementLifecycleAuditEvent> for AuditEventResponse {
    fn from(value: ManagementLifecycleAuditEvent) -> Self {
        Self {
            id: value.id,
            action: match value.action {
                ManagementLifecycleAuditAction::Created => "config_created",
                ManagementLifecycleAuditAction::Published => "config_published",
                ManagementLifecycleAuditAction::RolledBack => "config_rolled_back",
            },
            actor: value.actor,
            occurred_at_ms: value.occurred_at_ms,
            config_version_id: value.config_version_id.as_str().to_owned(),
            replaced_config_version_id: value
                .replaced_config_version_id
                .map(|id| id.as_str().to_owned()),
        }
    }
}
