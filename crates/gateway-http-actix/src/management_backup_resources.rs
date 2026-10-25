//! Protected P10-08 encrypted-backup preflight and empty-target restore handlers.
//!
//! This module projects a configured, transport-neutral backup service through the frozen P10-01
//! management API. It never creates a backup download, accepts a Backup/Master Key, chooses a
//! filesystem path, logs binary material, or overwrites an active database.

#![deny(unsafe_code)]

use std::{
    fs::File,
    io::{Seek, Write},
    sync::{Arc, Mutex},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use actix_web::{
    HttpRequest, HttpResponse,
    http::{StatusCode, header},
    web,
};
use futures_util::StreamExt;
use gateway_control::management_backup_service::{
    MAX_MANAGEMENT_BACKUP_BODY_BYTES, ManagementBackupService, ManagementBackupServiceError,
};
use serde::Serialize;

use crate::management_security::configure_management;

/// Isolated backup/restore HTTP state. A mutex serializes backup operations and keeps their
/// configured filesystem authority outside the inference path.
pub struct ManagementBackupHttpState {
    facade: Mutex<Box<dyn ManagementBackupFacade>>,
    admission: Arc<Semaphore>,
}

impl ManagementBackupHttpState {
    /// Creates protected backup HTTP state from the configured transport-neutral service.
    #[must_use]
    pub fn new(service: ManagementBackupService) -> Self {
        Self::with_facade(Box::new(ManagementBackupServiceFacade { service }))
    }

    /// Creates protected state with an explicit bounded facade for embeddings or tests.
    #[must_use]
    pub fn with_facade(facade: Box<dyn ManagementBackupFacade>) -> Self {
        Self {
            facade: Mutex::new(facade),
            admission: Arc::new(Semaphore::new(1)),
        }
    }
}

/// Safe source-backup metadata exposed through `POST /admin/backups/preflight`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagementBackupMetadata {
    schema_version: i64,
    secret_key_required: bool,
}

impl ManagementBackupMetadata {
    /// Creates one safe source-backup metadata projection.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementBackupError::Unavailable`] when the supplied schema value cannot be
    /// represented by the frozen management contract.
    pub fn try_new(
        schema_version: i64,
        secret_key_required: bool,
    ) -> Result<Self, ManagementBackupError> {
        if schema_version < 1 {
            return Err(ManagementBackupError::Unavailable);
        }
        Ok(Self {
            schema_version,
            secret_key_required,
        })
    }

    /// Returns the configured source schema version.
    #[must_use]
    pub const fn schema_version(self) -> i64 {
        self.schema_version
    }

    /// Returns whether restored credentials require the independently stored Master Key.
    #[must_use]
    pub const fn secret_key_required(self) -> bool {
        self.secret_key_required
    }
}

/// Safe result of encrypted restore-material preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagementRestoreMetadata {
    schema_version: i64,
    quick_check_required: bool,
    compatible: bool,
}

impl ManagementRestoreMetadata {
    /// Creates one safe restore preflight projection.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementBackupError::Unavailable`] when the supplied projection does not
    /// retain the mandatory `SQLite` quick-check requirement.
    pub fn try_new(
        schema_version: i64,
        quick_check_required: bool,
        compatible: bool,
    ) -> Result<Self, ManagementBackupError> {
        if schema_version < 1 || !quick_check_required {
            return Err(ManagementBackupError::Unavailable);
        }
        Ok(Self {
            schema_version,
            quick_check_required,
            compatible,
        })
    }

    /// Returns the authenticated source schema version.
    #[must_use]
    pub const fn schema_version(self) -> i64 {
        self.schema_version
    }

    /// Returns the fixed `SQLite` quick-check requirement.
    #[must_use]
    pub const fn quick_check_required(self) -> bool {
        self.quick_check_required
    }

    /// Returns whether this build can migrate the artifact's history.
    #[must_use]
    pub const fn compatible(self) -> bool {
        self.compatible
    }
}

/// Closed restore result projected through the frozen `RestoreOperation` schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementRestoreState {
    /// The configured empty target was created after full staging validation.
    Complete,
}

/// Safe errors from the protected backup facade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementBackupError {
    /// The binary request body was absent, oversized, malformed, or could not authenticate.
    InvalidInput,
    /// An authenticated artifact cannot be migrated by this build.
    Incompatible,
    /// The configured target already exists or cannot be created without replacement.
    Conflict,
    /// A configured local dependency is unavailable.
    Unavailable,
}

/// Explicit P10-08 transport seam. Implementations receive anonymous encrypted disk spools and
/// retain all filesystem/key authority from embedding-time configuration.
pub trait ManagementBackupFacade: Send {
    /// Allocates an anonymous disk upload using configured filesystem authority.
    ///
    /// # Errors
    /// Returns a value-free error if staging is unavailable.
    fn create_restore_upload(&mut self) -> Result<File, ManagementBackupError>;

    /// Returns safe source metadata without generating artifact bytes.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`ManagementBackupError`] if configured source metadata is
    /// unavailable.
    fn backup_preflight(&mut self) -> Result<ManagementBackupMetadata, ManagementBackupError>;

    /// Validates supplied encrypted material without changing the configured target.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`ManagementBackupError`] when the artifact cannot safely preflight.
    fn restore_preflight(
        &mut self,
        artifact: &mut File,
    ) -> Result<ManagementRestoreMetadata, ManagementBackupError>;

    /// Restores supplied material only into the configured, absent target.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`ManagementBackupError`] when validation or target creation fails.
    fn restore(
        &mut self,
        artifact: &mut File,
    ) -> Result<ManagementRestoreState, ManagementBackupError>;
}

struct ManagementBackupServiceFacade {
    service: ManagementBackupService,
}

impl ManagementBackupFacade for ManagementBackupServiceFacade {
    fn create_restore_upload(&mut self) -> Result<File, ManagementBackupError> {
        self.service
            .create_restore_upload()
            .map_err(map_service_error)
    }

    fn backup_preflight(&mut self) -> Result<ManagementBackupMetadata, ManagementBackupError> {
        let preflight = self.service.backup_preflight().map_err(map_service_error)?;
        ManagementBackupMetadata::try_new(
            preflight.schema_version(),
            preflight.secret_key_required(),
        )
    }

    fn restore_preflight(
        &mut self,
        artifact: &mut File,
    ) -> Result<ManagementRestoreMetadata, ManagementBackupError> {
        let preflight = self
            .service
            .restore_preflight_reader(artifact)
            .map_err(map_service_error)?;
        ManagementRestoreMetadata::try_new(
            preflight.source_schema_version(),
            preflight.quick_check_required(),
            preflight.compatible(),
        )
    }

    fn restore(
        &mut self,
        artifact: &mut File,
    ) -> Result<ManagementRestoreState, ManagementBackupError> {
        self.service
            .restore_reader_to_empty_target(artifact)
            .map_err(map_service_error)?;
        Ok(ManagementRestoreState::Complete)
    }
}

/// Default fail-closed facade for a deployment that has not configured backup authority.
pub struct RejectingManagementBackupFacade;

impl RejectingManagementBackupFacade {
    /// Creates a no-filesystem, no-key backup facade.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RejectingManagementBackupFacade {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagementBackupFacade for RejectingManagementBackupFacade {
    fn create_restore_upload(&mut self) -> Result<File, ManagementBackupError> {
        Err(ManagementBackupError::Unavailable)
    }

    fn backup_preflight(&mut self) -> Result<ManagementBackupMetadata, ManagementBackupError> {
        Err(ManagementBackupError::Unavailable)
    }

    fn restore_preflight(
        &mut self,
        _artifact: &mut File,
    ) -> Result<ManagementRestoreMetadata, ManagementBackupError> {
        Err(ManagementBackupError::Unavailable)
    }

    fn restore(
        &mut self,
        _artifact: &mut File,
    ) -> Result<ManagementRestoreState, ManagementBackupError> {
        Err(ManagementBackupError::Unavailable)
    }
}

/// Mounts P10-08 routes inside the P10-02 protected `/admin` scope.
pub fn configure_management_backup_resources(config: &mut web::ServiceConfig) {
    configure_management(config, backup_routes);
}

fn backup_routes(config: &mut web::ServiceConfig) {
    config
        .route("/backups/preflight", web::post().to(preview_backup))
        .route("/restores/preflight", web::post().to(preview_restore))
        .route("/restores", web::post().to(restore_backup));
}

/// Registers only P10 backup paths inside an already protected management scope.
pub(crate) fn configure_protected_backup_routes(config: &mut web::ServiceConfig) {
    backup_routes(config);
}

async fn preview_backup(state: web::Data<ManagementBackupHttpState>) -> HttpResponse {
    let Ok(mut facade) = state.facade.try_lock() else {
        return unavailable();
    };
    match facade.backup_preflight() {
        Ok(value) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(BackupPreflightResponse::from(value)),
        Err(error) => backup_error(error),
    }
}

async fn preview_restore(
    request: HttpRequest,
    state: web::Data<ManagementBackupHttpState>,
    payload: web::Payload,
) -> HttpResponse {
    restore_material(request, state, payload, true).await
}

async fn restore_backup(
    request: HttpRequest,
    state: web::Data<ManagementBackupHttpState>,
    payload: web::Payload,
) -> HttpResponse {
    restore_material(request, state, payload, false).await
}

async fn restore_material(
    request: HttpRequest,
    state: web::Data<ManagementBackupHttpState>,
    payload: web::Payload,
    preflight: bool,
) -> HttpResponse {
    if !is_octet_stream(&request) || content_length_exceeds_limit(&request) {
        return invalid_input();
    }
    let Ok(permit) = state.admission.clone().try_acquire_owned() else {
        return unavailable();
    };
    let material = match read_binary_material(&state, payload, permit).await {
        Ok(material) => material,
        Err(response) => return response,
    };
    // The permit and anonymous file follow the blocking operation after HTTP cancellation.
    match web::block(move || complete_restore(&state, material, preflight)).await {
        Ok(Ok(value)) if preflight => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(value),
        Ok(Ok(_)) => HttpResponse::Accepted()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(RestoreOperationResponse { state: "complete" }),
        Ok(Err(error)) => backup_error(error),
        Err(_) => unavailable(),
    }
}

fn complete_restore(
    state: &ManagementBackupHttpState,
    mut material: RestoreUpload,
    preflight: bool,
) -> Result<Option<RestorePreflightResponse>, ManagementBackupError> {
    material
        .file
        .rewind()
        .map_err(|_| ManagementBackupError::Unavailable)?;
    let mut facade = state
        .facade
        .lock()
        .map_err(|_| ManagementBackupError::Unavailable)?;
    if preflight {
        facade
            .restore_preflight(&mut material.file)
            .map(|value| Some(RestorePreflightResponse::from(value)))
    } else {
        facade.restore(&mut material.file).map(|_| None)
    }
}

struct RestoreUpload {
    file: File,
    _permit: OwnedSemaphorePermit,
}

async fn read_binary_material(
    state: &ManagementBackupHttpState,
    mut payload: web::Payload,
    permit: OwnedSemaphorePermit,
) -> Result<RestoreUpload, HttpResponse> {
    let file = state
        .facade
        .lock()
        .map_err(|_| unavailable())?
        .create_restore_upload()
        .map_err(backup_error)?;
    let mut material = RestoreUpload {
        file,
        _permit: permit,
    };
    let mut length = 0usize;
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|_| invalid_input())?;
        length = length.checked_add(chunk.len()).ok_or_else(invalid_input)?;
        if length > MAX_MANAGEMENT_BACKUP_BODY_BYTES {
            return Err(invalid_input());
        }
        material = web::block(move || {
            material.file.write_all(&chunk)?;
            Ok::<_, std::io::Error>(material)
        })
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable())?;
    }
    if length == 0 {
        return Err(invalid_input());
    }
    Ok(material)
}

fn is_octet_stream(request: &HttpRequest) -> bool {
    request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| {
            value
                .trim()
                .eq_ignore_ascii_case("application/octet-stream")
        })
}

fn content_length_exceeds_limit(request: &HttpRequest) -> bool {
    request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_MANAGEMENT_BACKUP_BODY_BYTES)
}

fn map_service_error(error: ManagementBackupServiceError) -> ManagementBackupError {
    match error {
        ManagementBackupServiceError::InvalidConfiguration
        | ManagementBackupServiceError::Unavailable => ManagementBackupError::Unavailable,
        ManagementBackupServiceError::InvalidArtifact => ManagementBackupError::InvalidInput,
        ManagementBackupServiceError::IncompatibleArtifact => ManagementBackupError::Incompatible,
        ManagementBackupServiceError::RestoreTargetUnavailable => ManagementBackupError::Conflict,
    }
}

fn backup_error(error: ManagementBackupError) -> HttpResponse {
    match error {
        ManagementBackupError::InvalidInput => error_response(
            StatusCode::BAD_REQUEST,
            "invalid_backup_material",
            "Backup material is invalid",
        ),
        ManagementBackupError::Incompatible => error_response(
            StatusCode::CONFLICT,
            "incompatible_backup_material",
            "Backup material is incompatible with this gateway",
        ),
        ManagementBackupError::Conflict => error_response(
            StatusCode::CONFLICT,
            "restore_target_unavailable",
            "Restore cannot create the configured empty target",
        ),
        ManagementBackupError::Unavailable => unavailable(),
    }
}

fn invalid_input() -> HttpResponse {
    error_response(
        StatusCode::BAD_REQUEST,
        "invalid_management_request",
        "Management request is invalid",
    )
}

fn unavailable() -> HttpResponse {
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "management_backup_unavailable",
        "Management backup operation is unavailable",
    )
}

fn error_response(status: StatusCode, code: &'static str, message: &'static str) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(serde_json::json!({"error":{"code":code,"message":message}}))
}

#[derive(Serialize)]
struct BackupPreflightResponse {
    schema_version: i64,
    secret_key_required: bool,
}

impl From<ManagementBackupMetadata> for BackupPreflightResponse {
    fn from(value: ManagementBackupMetadata) -> Self {
        Self {
            schema_version: value.schema_version(),
            secret_key_required: value.secret_key_required(),
        }
    }
}

#[derive(Serialize)]
struct RestorePreflightResponse {
    schema_version: i64,
    quick_check_required: bool,
    compatible: bool,
}

impl From<ManagementRestoreMetadata> for RestorePreflightResponse {
    fn from(value: ManagementRestoreMetadata) -> Self {
        Self {
            schema_version: value.schema_version(),
            quick_check_required: value.quick_check_required(),
            compatible: value.compatible(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn cancelled_disk_work_keeps_restore_upload_admission()
    -> Result<(), Box<dyn std::error::Error>> {
        let admission = Arc::new(Semaphore::new(1));
        let upload = RestoreUpload {
            file: gateway_store::backup::create_restore_upload(&std::env::temp_dir())?,
            _permit: admission.clone().acquire_owned().await?,
        };
        let (started, wait_started) = tokio::sync::oneshot::channel();
        let (finish, wait_finish) = std::sync::mpsc::channel();
        let task = actix_web::rt::spawn(async move {
            web::block(move || {
                let _ = started.send(());
                let result = wait_finish.recv();
                drop(upload);
                result
            })
            .await
        });
        wait_started.await?;
        task.abort();
        tokio::task::yield_now().await;
        assert!(admission.clone().try_acquire_owned().is_err());
        finish.send(())?;
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while admission.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert!(admission.try_acquire_owned().is_ok());
        Ok(())
    }
}

#[derive(Serialize)]
struct RestoreOperationResponse {
    state: &'static str,
}
