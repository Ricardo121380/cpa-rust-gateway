//! Version 2 disk-backed artifacts; version 1 remains readable with its original bound.
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{AeadInPlace, KeyInit},
};
use rusqlite::{
    Connection,
    backup::{Backup, StepResult},
};
use zeroize::Zeroizing;

use super::{
    AEAD_TAG_BYTES, BackupError, BackupKey, BackupRestorePreflight, EmptyTargetRestore,
    HEADER_BYTES, MAX_BACKUP_ARTIFACT_BYTES, TemporarySqliteFile, artifact_header,
    decrypt_artifact, migrate_staged_database, open_staging_file, parse_header,
    source_schema_version, validate_staged_database,
};

/// Finite `SQLite` snapshot limit for disk-backed version 2 artifacts (128 MiB).
pub const MAX_STREAM_BACKUP_PLAINTEXT_BYTES: usize = 128 * 1024 * 1024;
const CHUNK_BYTES: usize = 64 * 1024;
const STREAM_HEADER_BYTES: usize = HEADER_BYTES + 8;
/// Finite upload/operator artifact limit, including every authentication tag.
pub const MAX_STREAM_BACKUP_ARTIFACT_BYTES: usize = STREAM_HEADER_BYTES
    + MAX_STREAM_BACKUP_PLAINTEXT_BYTES
    + MAX_STREAM_BACKUP_PLAINTEXT_BYTES.div_ceil(CHUNK_BYTES) * AEAD_TAG_BYTES;
const MAGIC: &[u8; 8] = b"CPABKP02";

/// Creates a private, anonymous disk spool in an embedding-configured directory.
/// Its file descriptor is the only authority; cancellation or process exit releases it.
///
/// # Errors
/// Returns a value-free error if secure staging or unlinking is unavailable.
pub fn create_restore_upload(directory: &Path) -> Result<File, BackupError> {
    let (mut staging, file) = TemporarySqliteFile::create_file_in(directory)?;
    fs::remove_file(staging.path()).map_err(|_| BackupError::FilesystemUnavailable)?;
    staging.retained = true;
    Ok(file)
}

/// Writes a chunk-authenticated artifact without retaining a whole snapshot in memory.
/// A failed writer can contain an incomplete artifact; readers reject it. Prefer the atomic
/// file wrapper for operator artifacts. The source and configured staging directory are retained.
///
/// # Errors
/// Returns a value-free error on source size, incomplete `SQLite` backup, encryption or I/O failure.
pub fn create_encrypted_backup_to_writer(
    source: &Connection,
    directory: &Path,
    output: &mut impl Write,
    key: &BackupKey,
) -> Result<(), BackupError> {
    let staging = TemporarySqliteFile::create_in(directory)?;
    {
        // Pin the checked image through completion. An embedding's active transaction remains
        // owned by that embedding; otherwise this read transaction rolls back on scope exit.
        let _source_snapshot = if source.is_autocommit() {
            Some(
                rusqlite::Transaction::new_unchecked(
                    source,
                    rusqlite::TransactionBehavior::Deferred,
                )
                .map_err(|_| BackupError::SnapshotUnavailable)?,
            )
        } else {
            None
        };
        source_schema_version(source)?;
        let (pages, page_size): (i64, i64) = source
            .query_row(
                "SELECT page_count,page_size FROM pragma_page_count(),pragma_page_size()",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| BackupError::SnapshotUnavailable)?;
        if pages <= 0
            || page_size <= 0
            || pages.saturating_mul(page_size)
                > i64::try_from(MAX_STREAM_BACKUP_PLAINTEXT_BYTES)
                    .map_err(|_| BackupError::MaterialTooLarge)?
        {
            return Err(BackupError::MaterialTooLarge);
        }
        let mut snapshot =
            Connection::open(staging.path()).map_err(|_| BackupError::SnapshotUnavailable)?;
        let backup =
            Backup::new(source, &mut snapshot).map_err(|_| BackupError::SnapshotUnavailable)?;
        if backup
            .step(-1)
            .map_err(|_| BackupError::SnapshotUnavailable)?
            != StepResult::Done
        {
            return Err(BackupError::SnapshotUnavailable);
        }
    }
    let mut snapshot =
        File::open(staging.path()).map_err(|_| BackupError::FilesystemUnavailable)?;
    let size = snapshot
        .metadata()
        .map_err(|_| BackupError::FilesystemUnavailable)?
        .len();
    if size == 0 || size > MAX_STREAM_BACKUP_PLAINTEXT_BYTES as u64 {
        return Err(BackupError::MaterialTooLarge);
    }
    // Read schema from the completed snapshot, not from a potentially later source state.
    let schema = source_schema_version(
        &Connection::open(staging.path()).map_err(|_| BackupError::SnapshotUnavailable)?,
    )?;
    let mut header = artifact_header(schema)?;
    header[..8].copy_from_slice(MAGIC);
    header[8] = 2;
    getrandom::fill(&mut header[HEADER_BYTES - 24..])
        .map_err(|_| BackupError::RandomnessUnavailable)?;
    header.extend_from_slice(&size.to_be_bytes());
    output
        .write_all(&header)
        .map_err(|_| BackupError::FilesystemUnavailable)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|_| BackupError::AuthenticationFailed)?;
    let mut remaining = size;
    let mut index = 0_u64;
    let mut chunk = Zeroizing::new(Vec::with_capacity(CHUNK_BYTES + AEAD_TAG_BYTES));
    while remaining > 0 {
        let length = usize::try_from(remaining.min(CHUNK_BYTES as u64))
            .map_err(|_| BackupError::MaterialTooLarge)?;
        chunk.resize(length, 0);
        snapshot
            .read_exact(&mut chunk)
            .map_err(|_| BackupError::FilesystemUnavailable)?;
        let nonce = nonce(&header, index);
        cipher
            .encrypt_in_place(
                XNonce::from_slice(&nonce),
                &associated_data(&header, index),
                &mut *chunk,
            )
            .map_err(|_| BackupError::AuthenticationFailed)?;
        output
            .write_all(&chunk)
            .map_err(|_| BackupError::FilesystemUnavailable)?;
        remaining -= length as u64;
        index += 1;
    }
    Ok(())
}

/// Atomically publishes a complete encrypted operator artifact at a previously absent path.
///
/// # Errors
/// Returns a value-free error without replacing an existing file or publishing partial material.
pub fn create_encrypted_backup_file(
    source: &Connection,
    directory: &Path,
    target: &Path,
    key: &BackupKey,
) -> Result<(), BackupError> {
    if fs::symlink_metadata(target).is_ok() {
        return Err(BackupError::RestoreTargetUnavailable);
    }
    let staging = TemporarySqliteFile::create_in(
        target
            .parent()
            .ok_or(BackupError::InvalidStagingDirectory)?,
    )?;
    let mut output = open_staging_file(staging.path())?;
    create_encrypted_backup_to_writer(source, directory, &mut output, key)?;
    output
        .sync_all()
        .map_err(|_| BackupError::FilesystemUnavailable)?;
    drop(output);
    staging.persist_new_target(target)
}

/// Validates a disk-backed v1/v2 artifact without creating the restore target.
///
/// # Errors
/// Returns a value-free error for malformed/authentication/integrity/I/O failures.
pub fn preflight_encrypted_backup_reader(
    input: &mut impl Read,
    directory: &Path,
    key: &BackupKey,
) -> Result<BackupRestorePreflight, BackupError> {
    let staging = TemporarySqliteFile::create_in(directory)?;
    let schema = decrypt_to_staging(input, staging.path(), key)?;
    let compatible = match validate_staged_database(staging.path(), schema) {
        Ok(()) => true,
        Err(BackupError::IncompatibleSchema) => false,
        Err(error) => return Err(error),
    };
    Ok(BackupRestorePreflight {
        source_schema_version: schema,
        quick_check_required: true,
        compatible,
    })
}

/// Restores a disk-backed v1/v2 artifact using the same validated empty-target publication.
///
/// # Errors
/// Returns a value-free error; no target is published before all chunks and `SQLite` checks pass.
pub fn restore_encrypted_backup_reader_to_empty_target(
    input: &mut impl Read,
    target: &Path,
    key: &BackupKey,
) -> Result<EmptyTargetRestore, BackupError> {
    if fs::symlink_metadata(target).is_ok() {
        return Err(BackupError::RestoreTargetUnavailable);
    }
    let staging = TemporarySqliteFile::create_in(
        target
            .parent()
            .ok_or(BackupError::RestoreTargetUnavailable)?,
    )?;
    let source_schema_version = decrypt_to_staging(input, staging.path(), key)?;
    validate_staged_database(staging.path(), source_schema_version)?;
    let restored_schema_version = migrate_staged_database(staging.path())?;
    File::open(staging.path())
        .and_then(|file| file.sync_all())
        .map_err(|_| BackupError::FilesystemUnavailable)?;
    staging.persist_new_target(target)?;
    Ok(EmptyTargetRestore {
        source_schema_version,
        restored_schema_version,
    })
}

fn decrypt_to_staging(
    input: &mut impl Read,
    path: &Path,
    key: &BackupKey,
) -> Result<i64, BackupError> {
    let mut header = vec![0_u8; HEADER_BYTES];
    input
        .read_exact(&mut header)
        .map_err(|_| BackupError::InvalidArtifact)?;
    let mut output = open_staging_file(path)?;
    let schema = if &header[..8] == MAGIC && header[8] == 2 {
        let mut length = [0_u8; 8];
        input
            .read_exact(&mut length)
            .map_err(|_| BackupError::InvalidArtifact)?;
        let mut remaining = u64::from_be_bytes(length);
        if remaining == 0 || remaining > MAX_STREAM_BACKUP_PLAINTEXT_BYTES as u64 {
            return Err(BackupError::MaterialTooLarge);
        }
        header.extend_from_slice(&length);
        let schema = i64::from(u32::from_be_bytes([
            header[9], header[10], header[11], header[12],
        ]));
        if schema == 0 {
            return Err(BackupError::InvalidArtifact);
        }
        let cipher = XChaCha20Poly1305::new_from_slice(key.as_bytes())
            .map_err(|_| BackupError::AuthenticationFailed)?;
        let mut chunk = Zeroizing::new(Vec::with_capacity(CHUNK_BYTES + AEAD_TAG_BYTES));
        let mut index = 0_u64;
        while remaining > 0 {
            let size = usize::try_from(remaining.min(CHUNK_BYTES as u64))
                .map_err(|_| BackupError::MaterialTooLarge)?;
            chunk.resize(size + AEAD_TAG_BYTES, 0);
            input
                .read_exact(&mut chunk)
                .map_err(|_| BackupError::InvalidArtifact)?;
            let nonce = nonce(&header, index);
            cipher
                .decrypt_in_place(
                    XNonce::from_slice(&nonce),
                    &associated_data(&header, index),
                    &mut *chunk,
                )
                .map_err(|_| BackupError::AuthenticationFailed)?;
            output
                .write_all(&chunk)
                .map_err(|_| BackupError::FilesystemUnavailable)?;
            remaining -= size as u64;
            index += 1;
        }
        let mut extra = [0_u8; 1];
        if input
            .read(&mut extra)
            .map_err(|_| BackupError::InvalidArtifact)?
            != 0
        {
            return Err(BackupError::InvalidArtifact);
        }
        schema
    } else {
        parse_header(&header)?;
        let mut artifact = header;
        input
            .take((MAX_BACKUP_ARTIFACT_BYTES + 1 - HEADER_BYTES) as u64)
            .read_to_end(&mut artifact)
            .map_err(|_| BackupError::InvalidArtifact)?;
        let (header, plaintext) = decrypt_artifact(&artifact, key)?;
        output
            .write_all(&plaintext)
            .map_err(|_| BackupError::FilesystemUnavailable)?;
        header.source_schema_version
    };
    output
        .sync_all()
        .map_err(|_| BackupError::FilesystemUnavailable)?;
    Ok(schema)
}

fn nonce(header: &[u8], index: u64) -> [u8; 24] {
    let mut nonce = [0_u8; 24];
    nonce.copy_from_slice(&header[HEADER_BYTES - 24..HEADER_BYTES]);
    for (byte, counter) in nonce[16..].iter_mut().zip(index.to_be_bytes()) {
        *byte ^= counter;
    }
    nonce
}

fn associated_data(header: &[u8], index: u64) -> Vec<u8> {
    let mut aad = header.to_vec();
    aad.extend_from_slice(&index.to_be_bytes());
    aad
}
