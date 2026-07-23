//! Encrypted, bounded SQLite control-plane backup and empty-target restore primitives.
//!
//! This module deliberately accepts only caller-configured source, staging, and target paths.
//! It never reads a credential Master Key, exposes an artifact through HTTP, or overwrites a
//! database. The encrypted artifact preserves existing credential ciphertext as ordinary SQLite
//! data, so a restored deployment still needs its separate Master Key directory.

#![deny(unsafe_code)]

use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use rusqlite::{Connection, backup::Backup};
use zeroize::{Zeroize, Zeroizing};

use crate::{CURRENT_SCHEMA_VERSION, StoreError, migrate, schema_version};

/// Exact raw byte length accepted for the independent encrypted-backup key.
pub const BACKUP_KEY_BYTES: usize = 32;
/// Largest complete encrypted artifact accepted by this release.
pub const MAX_BACKUP_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;

const ARTIFACT_MAGIC: [u8; 8] = *b"CPABKP01";
const ARTIFACT_FORMAT_VERSION: u8 = 1;
const NONCE_BYTES: usize = 24;
const AEAD_TAG_BYTES: usize = 16;
const HEADER_BYTES: usize = ARTIFACT_MAGIC.len() + 1 + 4 + NONCE_BYTES;
const MINIMUM_ARTIFACT_BYTES: usize = HEADER_BYTES + AEAD_TAG_BYTES;
const MAX_BACKUP_PLAINTEXT_BYTES: usize = MAX_BACKUP_ARTIFACT_BYTES - HEADER_BYTES - AEAD_TAG_BYTES;
const TEMPORARY_NAME_ATTEMPTS: usize = 8;

/// A configured backup key whose bytes are redacted and zeroized on drop.
pub struct BackupKey {
    bytes: [u8; BACKUP_KEY_BYTES],
}

impl BackupKey {
    /// Copies exact-size backup key material from an embedding-only configuration source.
    ///
    /// # Errors
    ///
    /// Returns [`BackupError::InvalidKeyLength`] unless `bytes` contains exactly 32 raw bytes.
    pub fn try_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, BackupError> {
        let bytes = bytes.as_ref();
        if bytes.len() != BACKUP_KEY_BYTES {
            return Err(BackupError::InvalidKeyLength {
                actual: bytes.len(),
            });
        }

        let mut key = [0_u8; BACKUP_KEY_BYTES];
        key.copy_from_slice(bytes);
        Ok(Self { bytes: key })
    }

    fn as_bytes(&self) -> &[u8; BACKUP_KEY_BYTES] {
        &self.bytes
    }
}

impl Clone for BackupKey {
    fn clone(&self) -> Self {
        Self { bytes: self.bytes }
    }
}

impl fmt::Debug for BackupKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupKey(<redacted>)")
    }
}

impl Drop for BackupKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// Secret-free result returned by encrypted-artifact preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackupRestorePreflight {
    source_schema_version: i64,
    quick_check_required: bool,
    compatible: bool,
}

impl BackupRestorePreflight {
    /// Returns the authenticated source schema version from the artifact header.
    #[must_use]
    pub const fn source_schema_version(self) -> i64 {
        self.source_schema_version
    }

    /// States that every restore must retain `SQLite` quick-check validation.
    #[must_use]
    pub const fn quick_check_required(self) -> bool {
        self.quick_check_required
    }

    /// Returns whether this build can safely migrate the authenticated database history.
    #[must_use]
    pub const fn compatible(self) -> bool {
        self.compatible
    }
}

/// Successful completion result for one empty-target restore.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmptyTargetRestore {
    source_schema_version: i64,
    restored_schema_version: i64,
}

impl EmptyTargetRestore {
    /// Returns the authenticated source schema version.
    #[must_use]
    pub const fn source_schema_version(self) -> i64 {
        self.source_schema_version
    }

    /// Returns the current schema version reached in the newly created target.
    #[must_use]
    pub const fn restored_schema_version(self) -> i64 {
        self.restored_schema_version
    }
}

/// Value-free failures from the encrypted backup boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupError {
    /// A configured backup key did not have the exact required raw-byte length.
    InvalidKeyLength {
        /// Observed length, never key contents.
        actual: usize,
    },
    /// A complete artifact or decrypted snapshot exceeded the fixed bound.
    MaterialTooLarge,
    /// The artifact did not have the fixed authenticated format.
    InvalidArtifact,
    /// The artifact could not be authenticated with the configured backup key.
    AuthenticationFailed,
    /// The operating system could not obtain a fresh encryption nonce.
    RandomnessUnavailable,
    /// The source database had no supported migrated control-plane schema.
    SourceSchemaUnavailable,
    /// The artifact contains a database history this build cannot migrate safely.
    IncompatibleSchema,
    /// `SQLite` integrity or foreign-key validation rejected staged material.
    IntegrityCheckFailed,
    /// The configured staging directory was not an existing ordinary directory.
    InvalidStagingDirectory,
    /// A bounded temporary staging file could not be allocated.
    StagingUnavailable,
    /// The configured restore target already existed or could not be created without replacement.
    RestoreTargetUnavailable,
    /// A filesystem operation failed without returning a usable partial result.
    FilesystemUnavailable,
    /// `SQLite` could not create or consume a consistent snapshot.
    SnapshotUnavailable,
}

impl fmt::Display for BackupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidKeyLength { .. } => "backup key has an invalid length",
            Self::MaterialTooLarge => "backup material exceeds the configured limit",
            Self::InvalidArtifact => "backup artifact is invalid",
            Self::AuthenticationFailed => "backup artifact could not be authenticated",
            Self::RandomnessUnavailable => "backup randomness is unavailable",
            Self::SourceSchemaUnavailable => "backup source schema is unavailable",
            Self::IncompatibleSchema => "backup schema is incompatible with this build",
            Self::IntegrityCheckFailed => "backup integrity validation failed",
            Self::InvalidStagingDirectory => "backup staging directory is invalid",
            Self::StagingUnavailable => "backup staging is unavailable",
            Self::RestoreTargetUnavailable => "restore target is unavailable",
            Self::FilesystemUnavailable => "backup filesystem operation failed",
            Self::SnapshotUnavailable => "SQLite snapshot operation failed",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for BackupError {}

/// Creates one bounded encrypted artifact from a consistent `SQLite` snapshot.
///
/// The caller supplies an already-open source connection and a configured staging directory.
/// The returned bytes are intended for an operator/embedding path, never an HTTP download.
///
/// # Errors
///
/// Returns a value-free [`BackupError`] if snapshot creation, encryption, staging, or schema
/// validation fails. It never returns a partial artifact.
pub fn create_encrypted_backup(
    source: &Connection,
    staging_directory: impl AsRef<Path>,
    backup_key: &BackupKey,
) -> Result<Vec<u8>, BackupError> {
    let source_schema_version = source_schema_version(source)?;
    let staging = TemporarySqliteFile::create_in(staging_directory.as_ref())?;

    {
        let mut snapshot =
            Connection::open(staging.path()).map_err(|_| BackupError::SnapshotUnavailable)?;
        let backup =
            Backup::new(source, &mut snapshot).map_err(|_| BackupError::SnapshotUnavailable)?;
        backup
            .step(-1)
            .map_err(|_| BackupError::SnapshotUnavailable)?;
    }

    let snapshot_bytes = Zeroizing::new(read_bounded(staging.path(), MAX_BACKUP_PLAINTEXT_BYTES)?);
    let mut header = artifact_header(source_schema_version)?;
    let mut nonce = [0_u8; NONCE_BYTES];
    getrandom::fill(&mut nonce).map_err(|_| BackupError::RandomnessUnavailable)?;
    header[HEADER_BYTES - NONCE_BYTES..].copy_from_slice(&nonce);

    let cipher = XChaCha20Poly1305::new_from_slice(backup_key.as_bytes())
        .map_err(|_| BackupError::AuthenticationFailed)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: snapshot_bytes.as_slice(),
                aad: &header,
            },
        )
        .map_err(|_| BackupError::AuthenticationFailed)?;

    let total_length = header
        .len()
        .checked_add(ciphertext.len())
        .ok_or(BackupError::MaterialTooLarge)?;
    if total_length > MAX_BACKUP_ARTIFACT_BYTES {
        return Err(BackupError::MaterialTooLarge);
    }

    let mut artifact = header;
    artifact.extend_from_slice(&ciphertext);
    Ok(artifact)
}

/// Decrypts and validates an artifact without changing any restore target.
///
/// A structurally valid, integrity-clean artifact with an unsupported migration history returns a
/// safe `compatible: false` projection. Malformed, unauthenticated, or corrupt material fails.
///
/// # Errors
///
/// Returns a value-free [`BackupError`] if the artifact cannot authenticate, cannot be decoded as
/// bounded `SQLite` material, or fails integrity validation. No target is changed.
pub fn preflight_encrypted_backup(
    artifact: &[u8],
    staging_directory: impl AsRef<Path>,
    backup_key: &BackupKey,
) -> Result<BackupRestorePreflight, BackupError> {
    let (header, plaintext) = decrypt_artifact(artifact, backup_key)?;
    let staging = TemporarySqliteFile::create_in(staging_directory.as_ref())?;
    write_staging_file(staging.path(), &plaintext)?;

    let compatible = match validate_staged_database(staging.path(), header.source_schema_version) {
        Ok(()) => true,
        Err(BackupError::IncompatibleSchema) => false,
        Err(error) => return Err(error),
    };

    Ok(BackupRestorePreflight {
        source_schema_version: header.source_schema_version,
        quick_check_required: true,
        compatible,
    })
}

/// Restores an encrypted artifact into a configured database path only when that path is absent.
///
/// The function writes and validates a same-directory staging database before atomically creating
/// the target without replacement. It never accepts a caller-selected destination or overwrites
/// an existing database.
///
/// # Errors
///
/// Returns a value-free [`BackupError`] if material is invalid or incompatible, staging fails, or
/// the configured target already exists. It never partially overwrites that target.
pub fn restore_encrypted_backup_to_empty_target(
    artifact: &[u8],
    target: impl AsRef<Path>,
    backup_key: &BackupKey,
) -> Result<EmptyTargetRestore, BackupError> {
    let target = target.as_ref();
    if fs::symlink_metadata(target).is_ok() {
        return Err(BackupError::RestoreTargetUnavailable);
    }
    let parent = target
        .parent()
        .ok_or(BackupError::RestoreTargetUnavailable)?;
    let (header, plaintext) = decrypt_artifact(artifact, backup_key)?;
    let staging = TemporarySqliteFile::create_in(parent)?;
    write_staging_file(staging.path(), &plaintext)?;

    validate_staged_database(staging.path(), header.source_schema_version)?;
    let restored_schema_version = migrate_staged_database(staging.path())?;
    staging.persist_new_target(target)?;

    Ok(EmptyTargetRestore {
        source_schema_version: header.source_schema_version,
        restored_schema_version,
    })
}

struct ArtifactHeader {
    source_schema_version: i64,
    encoded: Vec<u8>,
    nonce: [u8; NONCE_BYTES],
}

fn decrypt_artifact(
    artifact: &[u8],
    backup_key: &BackupKey,
) -> Result<(ArtifactHeader, Zeroizing<Vec<u8>>), BackupError> {
    if !(MINIMUM_ARTIFACT_BYTES..=MAX_BACKUP_ARTIFACT_BYTES).contains(&artifact.len()) {
        return Err(BackupError::InvalidArtifact);
    }

    let header = parse_header(&artifact[..HEADER_BYTES])?;
    let cipher = XChaCha20Poly1305::new_from_slice(backup_key.as_bytes())
        .map_err(|_| BackupError::AuthenticationFailed)?;
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(&header.nonce),
            Payload {
                msg: &artifact[HEADER_BYTES..],
                aad: &header.encoded,
            },
        )
        .map_err(|_| BackupError::AuthenticationFailed)?;
    if plaintext.len() > MAX_BACKUP_PLAINTEXT_BYTES {
        return Err(BackupError::MaterialTooLarge);
    }
    Ok((header, Zeroizing::new(plaintext)))
}

fn artifact_header(source_schema_version: i64) -> Result<Vec<u8>, BackupError> {
    let schema =
        u32::try_from(source_schema_version).map_err(|_| BackupError::SourceSchemaUnavailable)?;
    if schema == 0 {
        return Err(BackupError::SourceSchemaUnavailable);
    }

    let mut header = Vec::with_capacity(HEADER_BYTES);
    header.extend_from_slice(&ARTIFACT_MAGIC);
    header.push(ARTIFACT_FORMAT_VERSION);
    header.extend_from_slice(&schema.to_be_bytes());
    header.extend_from_slice(&[0_u8; NONCE_BYTES]);
    Ok(header)
}

fn parse_header(encoded: &[u8]) -> Result<ArtifactHeader, BackupError> {
    if encoded.len() != HEADER_BYTES || encoded[..ARTIFACT_MAGIC.len()] != ARTIFACT_MAGIC {
        return Err(BackupError::InvalidArtifact);
    }
    if encoded[ARTIFACT_MAGIC.len()] != ARTIFACT_FORMAT_VERSION {
        return Err(BackupError::InvalidArtifact);
    }

    let schema_start = ARTIFACT_MAGIC.len() + 1;
    let schema_end = schema_start + 4;
    let source_schema_version = i64::from(u32::from_be_bytes(
        encoded[schema_start..schema_end]
            .try_into()
            .map_err(|_| BackupError::InvalidArtifact)?,
    ));
    if source_schema_version < 1 {
        return Err(BackupError::InvalidArtifact);
    }

    let mut nonce = [0_u8; NONCE_BYTES];
    nonce.copy_from_slice(&encoded[schema_end..]);
    Ok(ArtifactHeader {
        source_schema_version,
        encoded: encoded.to_vec(),
        nonce,
    })
}

fn source_schema_version(source: &Connection) -> Result<i64, BackupError> {
    schema_version(source)
        .map_err(|_| BackupError::SourceSchemaUnavailable)?
        .filter(|version| *version >= 1)
        .ok_or(BackupError::SourceSchemaUnavailable)
}

fn validate_staged_database(path: &Path, expected_schema_version: i64) -> Result<(), BackupError> {
    let connection = crate::open(path).map_err(|_| BackupError::IntegrityCheckFailed)?;
    quick_check(&connection)?;
    foreign_key_check(&connection)?;

    match schema_version(&connection) {
        Ok(Some(schema_version)) if schema_version == expected_schema_version => Ok(()),
        Ok(_) | Err(StoreError::UnsupportedMigrationHistory { .. }) => {
            Err(BackupError::IncompatibleSchema)
        }
        Err(_) => Err(BackupError::IntegrityCheckFailed),
    }
}

fn migrate_staged_database(path: &Path) -> Result<i64, BackupError> {
    let mut connection = crate::open(path).map_err(|_| BackupError::IntegrityCheckFailed)?;
    migrate(&mut connection).map_err(|error| match error {
        StoreError::UnsupportedMigrationHistory { .. } => BackupError::IncompatibleSchema,
        _ => BackupError::IntegrityCheckFailed,
    })?;
    quick_check(&connection)?;
    foreign_key_check(&connection)?;
    let schema_version = schema_version(&connection)
        .map_err(|_| BackupError::IntegrityCheckFailed)?
        .ok_or(BackupError::IntegrityCheckFailed)?;
    if schema_version != CURRENT_SCHEMA_VERSION {
        return Err(BackupError::IncompatibleSchema);
    }
    Ok(schema_version)
}

fn quick_check(connection: &Connection) -> Result<(), BackupError> {
    let mut statement = connection
        .prepare("PRAGMA quick_check")
        .map_err(|_| BackupError::IntegrityCheckFailed)?;
    let mut rows = statement
        .query([])
        .map_err(|_| BackupError::IntegrityCheckFailed)?;
    let mut found_ok = false;
    while let Some(row) = rows.next().map_err(|_| BackupError::IntegrityCheckFailed)? {
        let result: String = row.get(0).map_err(|_| BackupError::IntegrityCheckFailed)?;
        if result != "ok" || found_ok {
            return Err(BackupError::IntegrityCheckFailed);
        }
        found_ok = true;
    }
    if found_ok {
        Ok(())
    } else {
        Err(BackupError::IntegrityCheckFailed)
    }
}

fn foreign_key_check(connection: &Connection) -> Result<(), BackupError> {
    let mut statement = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(|_| BackupError::IntegrityCheckFailed)?;
    let mut rows = statement
        .query([])
        .map_err(|_| BackupError::IntegrityCheckFailed)?;
    match rows.next().map_err(|_| BackupError::IntegrityCheckFailed)? {
        None => Ok(()),
        Some(_) => Err(BackupError::IntegrityCheckFailed),
    }
}

fn read_bounded(path: &Path, maximum_bytes: usize) -> Result<Vec<u8>, BackupError> {
    let metadata = fs::metadata(path).map_err(|_| BackupError::FilesystemUnavailable)?;
    let size = usize::try_from(metadata.len()).map_err(|_| BackupError::MaterialTooLarge)?;
    if size == 0 {
        return Err(BackupError::SnapshotUnavailable);
    }
    if size > maximum_bytes {
        return Err(BackupError::MaterialTooLarge);
    }
    fs::read(path).map_err(|_| BackupError::FilesystemUnavailable)
}

fn write_staging_file(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    if bytes.is_empty() || bytes.len() > MAX_BACKUP_PLAINTEXT_BYTES {
        return Err(BackupError::InvalidArtifact);
    }
    let mut file = open_staging_file(path)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| BackupError::FilesystemUnavailable)
}

fn open_staging_file(path: &Path) -> Result<File, BackupError> {
    let mut options = OpenOptions::new();
    options.write(true).truncate(true);
    #[cfg(unix)]
    {
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options
        .open(path)
        .map_err(|_| BackupError::FilesystemUnavailable)
}

struct TemporarySqliteFile {
    path: PathBuf,
    retained: bool,
}

impl TemporarySqliteFile {
    fn create_in(directory: &Path) -> Result<Self, BackupError> {
        let metadata =
            fs::symlink_metadata(directory).map_err(|_| BackupError::InvalidStagingDirectory)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(BackupError::InvalidStagingDirectory);
        }

        for _ in 0..TEMPORARY_NAME_ATTEMPTS {
            let mut random = [0_u8; 16];
            getrandom::fill(&mut random).map_err(|_| BackupError::RandomnessUnavailable)?;
            let path = directory.join(format!(".cpa-backup-{}.sqlite3", hexadecimal(&random)));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
            }
            match options.open(&path) {
                Ok(file) => {
                    drop(file);
                    return Ok(Self {
                        path,
                        retained: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(BackupError::StagingUnavailable),
            }
        }
        Err(BackupError::StagingUnavailable)
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn persist_new_target(mut self, target: &Path) -> Result<(), BackupError> {
        if fs::symlink_metadata(target).is_ok() {
            return Err(BackupError::RestoreTargetUnavailable);
        }
        fs::hard_link(&self.path, target).map_err(|_| BackupError::RestoreTargetUnavailable)?;
        if fs::remove_file(&self.path).is_err() {
            // `hard_link` has already created the only permitted target at this point. Do not
            // report a failed restore while knowingly leaving that target behind: best-effort
            // unlink it before the temporary-file guard performs its own cleanup. If a hostile
            // concurrent filesystem change prevents this cleanup, the value-free failure still
            // forces an operator to inspect the configured target rather than retrying into it.
            let _ = fs::remove_file(target);
            return Err(BackupError::FilesystemUnavailable);
        }
        self.retained = true;
        Ok(())
    }
}

impl Drop for TemporarySqliteFile {
    fn drop(&mut self) {
        if !self.retained {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn hexadecimal(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
