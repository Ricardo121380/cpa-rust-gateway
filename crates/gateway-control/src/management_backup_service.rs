//! Transport-neutral P10-08 encrypted-backup and empty-target restore facade.
//!
//! The service receives configured filesystem paths and an independently loaded Backup Key only
//! at construction. HTTP callers cannot select paths or submit a Backup/Master Key. Artifact
//! creation is intentionally an embedding/operator method; the management API exposes only
//! preflight and restore operations from the frozen P10-01 contract.

#![deny(unsafe_code)]

use std::{
    fmt,
    path::{Path, PathBuf},
};

use gateway_store::{
    backup::{
        BackupError, BackupKey, BackupRestorePreflight, EmptyTargetRestore,
        create_encrypted_backup, preflight_encrypted_backup,
        restore_encrypted_backup_to_empty_target,
    },
    open, schema_version,
};

/// Maximum encrypted artifact body accepted by the protected management HTTP boundary.
pub const MAX_MANAGEMENT_BACKUP_BODY_BYTES: usize =
    gateway_store::backup::MAX_BACKUP_ARTIFACT_BYTES;

/// Configured control-plane backup/restore service.
pub struct ManagementBackupService {
    source_database: PathBuf,
    restore_target: PathBuf,
    staging_directory: PathBuf,
    backup_key: BackupKey,
}

impl ManagementBackupService {
    /// Creates a service with embedding-controlled source/target/staging paths.
    ///
    /// `source_database` and `restore_target` must be distinct. The target need not exist: a
    /// restore creates it only after staging validation. No path is ever derived from HTTP input.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementBackupServiceError::InvalidConfiguration`] when a required path is
    /// empty, a staging directory is not valid, or source and target are the same configured
    /// path.
    pub fn try_new(
        source_database: impl Into<PathBuf>,
        restore_target: impl Into<PathBuf>,
        staging_directory: impl Into<PathBuf>,
        backup_key: BackupKey,
    ) -> Result<Self, ManagementBackupServiceError> {
        let source_database = source_database.into();
        let restore_target = restore_target.into();
        let staging_directory = staging_directory.into();
        if source_database.as_os_str().is_empty()
            || restore_target.as_os_str().is_empty()
            || staging_directory.as_os_str().is_empty()
            || source_database == restore_target
            || !is_ordinary_directory(&staging_directory)
        {
            return Err(ManagementBackupServiceError::InvalidConfiguration);
        }

        Ok(Self {
            source_database,
            restore_target,
            staging_directory,
            backup_key,
        })
    }

    /// Returns safe metadata explaining the configured source backup boundary.
    ///
    /// This does not generate or return a backup artifact.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementBackupServiceError::Unavailable`] when the configured source cannot
    /// be opened as a supported control-plane database.
    pub fn backup_preflight(
        &self,
    ) -> Result<ManagementBackupPreflight, ManagementBackupServiceError> {
        let source =
            open(&self.source_database).map_err(|_| ManagementBackupServiceError::Unavailable)?;
        let schema_version = schema_version(&source)
            .map_err(|_| ManagementBackupServiceError::Unavailable)?
            .ok_or(ManagementBackupServiceError::Unavailable)?;
        if schema_version < 1 {
            return Err(ManagementBackupServiceError::Unavailable);
        }
        Ok(ManagementBackupPreflight {
            schema_version,
            secret_key_required: true,
        })
    }

    /// Creates an encrypted artifact for a trusted operator/embedding caller.
    ///
    /// This method is deliberately not mounted as an HTTP download handler.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`ManagementBackupServiceError`] when source snapshotting, bounded
    /// staging, encryption, or source-schema validation fails.
    pub fn create_operator_artifact(&self) -> Result<Vec<u8>, ManagementBackupServiceError> {
        let source =
            open(&self.source_database).map_err(|_| ManagementBackupServiceError::Unavailable)?;
        create_encrypted_backup(&source, &self.staging_directory, &self.backup_key)
            .map_err(ManagementBackupServiceError::from_backup_error)
    }

    /// Validates encrypted artifact material without changing the configured restore target.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`ManagementBackupServiceError`] when material is invalid,
    /// incompatible, or cannot be staged and checked.
    pub fn restore_preflight(
        &self,
        artifact: &[u8],
    ) -> Result<BackupRestorePreflight, ManagementBackupServiceError> {
        preflight_encrypted_backup(artifact, &self.staging_directory, &self.backup_key)
            .map_err(ManagementBackupServiceError::from_backup_error)
    }

    /// Restores one verified artifact only into the configured missing target database.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`ManagementBackupServiceError`] when material is invalid or
    /// incompatible, or when the target cannot be created without replacement.
    pub fn restore_to_empty_target(
        &self,
        artifact: &[u8],
    ) -> Result<EmptyTargetRestore, ManagementBackupServiceError> {
        restore_encrypted_backup_to_empty_target(artifact, &self.restore_target, &self.backup_key)
            .map_err(ManagementBackupServiceError::from_backup_error)
    }

    /// Returns whether the configured restore destination exists, without returning its path.
    #[must_use]
    pub fn restore_target_exists(&self) -> bool {
        self.restore_target.exists()
    }
}

impl fmt::Debug for ManagementBackupService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagementBackupService")
            .field("source_database", &"<configured>")
            .field("restore_target", &"<configured>")
            .field("staging_directory", &"<configured>")
            .field("backup_key", &self.backup_key)
            .finish()
    }
}

/// Secret-free source-backup metadata projected by the management API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagementBackupPreflight {
    schema_version: i64,
    secret_key_required: bool,
}

impl ManagementBackupPreflight {
    /// Returns the configured source database schema version.
    #[must_use]
    pub const fn schema_version(self) -> i64 {
        self.schema_version
    }

    /// States that restored credential envelopes require the separately managed Master Key.
    #[must_use]
    pub const fn secret_key_required(self) -> bool {
        self.secret_key_required
    }
}

/// Safe classifications returned by the backup facade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementBackupServiceError {
    /// A configured source, target, or staging location did not satisfy the fixed boundary.
    InvalidConfiguration,
    /// Submitted material was malformed, oversized, or failed authentication.
    InvalidArtifact,
    /// Authenticated material could not be safely migrated by this build.
    IncompatibleArtifact,
    /// The configured restore target exists or cannot be created without replacement.
    RestoreTargetUnavailable,
    /// A configured dependency, `SQLite` operation, or filesystem operation was unavailable.
    Unavailable,
}

impl ManagementBackupServiceError {
    fn from_backup_error(error: BackupError) -> Self {
        match error {
            BackupError::InvalidArtifact
            | BackupError::AuthenticationFailed
            | BackupError::MaterialTooLarge
            | BackupError::InvalidKeyLength { .. } => Self::InvalidArtifact,
            BackupError::IncompatibleSchema => Self::IncompatibleArtifact,
            BackupError::RestoreTargetUnavailable => Self::RestoreTargetUnavailable,
            BackupError::RandomnessUnavailable
            | BackupError::SourceSchemaUnavailable
            | BackupError::IntegrityCheckFailed
            | BackupError::InvalidStagingDirectory
            | BackupError::StagingUnavailable
            | BackupError::FilesystemUnavailable
            | BackupError::SnapshotUnavailable => Self::Unavailable,
        }
    }
}

impl fmt::Display for ManagementBackupServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidConfiguration => "backup service configuration is invalid",
            Self::InvalidArtifact => "backup material is invalid",
            Self::IncompatibleArtifact => "backup material is incompatible",
            Self::RestoreTargetUnavailable => "restore target is unavailable",
            Self::Unavailable => "backup service is unavailable",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ManagementBackupServiceError {}

fn is_ordinary_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .is_ok_and(|metadata| !metadata.file_type().is_symlink() && metadata.is_dir())
}
