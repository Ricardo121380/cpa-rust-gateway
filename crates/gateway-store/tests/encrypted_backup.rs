//! P10-08 encrypted backup and empty-target restore regression tests.

#![deny(unsafe_code)]

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_store::{
    CURRENT_SCHEMA_VERSION,
    backup::{
        BackupError, BackupKey, create_encrypted_backup, preflight_encrypted_backup,
        restore_encrypted_backup_to_empty_target,
    },
    migrate, open, schema_version,
};

type TestResult = Result<(), Box<dyn Error>>;

static TEMPORARY_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Result<Self, Box<dyn Error>> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        for _ in 0..64 {
            let sequence = TEMPORARY_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "cpa-rust-gateway-p10-08-{timestamp}-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not allocate an isolated temporary backup directory".into())
    }

    fn join(&self, leaf: &str) -> PathBuf {
        self.0.join(leaf)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn backup_key(byte: u8) -> Result<BackupKey, BackupError> {
    BackupKey::try_from_bytes([byte; 32])
}

fn populated_source(path: &Path) -> Result<rusqlite::Connection, Box<dyn Error>> {
    let mut source = open(path)?;
    migrate(&mut source)?;
    source.execute(
        "INSERT INTO config_versions (id, parent_id, status, created_at_ms, description) \
         VALUES (?1, NULL, 'draft', 1, 'encrypted backup source')",
        ["backup-source"],
    )?;
    Ok(source)
}

#[test]
fn encrypted_snapshot_round_trips_to_a_new_target_with_schema_and_configuration_intact()
-> TestResult {
    let directory = TemporaryDirectory::new()?;
    let source_path = directory.join("source.sqlite3");
    let source = populated_source(&source_path)?;
    let key = backup_key(0xA5)?;

    let artifact = create_encrypted_backup(&source, directory.path(), &key)?;
    assert!(
        !artifact
            .windows(b"backup-source".len())
            .any(|window| window == b"backup-source")
    );

    let preflight = preflight_encrypted_backup(&artifact, directory.path(), &key)?;
    assert_eq!(preflight.source_schema_version(), CURRENT_SCHEMA_VERSION);
    assert!(preflight.quick_check_required());
    assert!(preflight.compatible());

    let restored_path = directory.join("restored.sqlite3");
    let restored = restore_encrypted_backup_to_empty_target(&artifact, &restored_path, &key)?;
    assert_eq!(restored.source_schema_version(), CURRENT_SCHEMA_VERSION);
    assert_eq!(restored.restored_schema_version(), CURRENT_SCHEMA_VERSION);

    let restored_connection = open(&restored_path)?;
    assert_eq!(
        schema_version(&restored_connection)?,
        Some(CURRENT_SCHEMA_VERSION)
    );
    let description: String = restored_connection.query_row(
        "SELECT description FROM config_versions WHERE id = ?1",
        ["backup-source"],
        |row| row.get(0),
    )?;
    assert_eq!(description, "encrypted backup source");
    Ok(())
}

#[test]
fn wrong_key_tampering_and_existing_target_fail_without_creating_or_replacing_a_database()
-> TestResult {
    let directory = TemporaryDirectory::new()?;
    let source = populated_source(&directory.join("source.sqlite3"))?;
    let key = backup_key(0xA5)?;
    let artifact = create_encrypted_backup(&source, directory.path(), &key)?;

    let wrong_key = backup_key(0x5A)?;
    let wrong_key_target = directory.join("wrong-key.sqlite3");
    assert_eq!(
        restore_encrypted_backup_to_empty_target(&artifact, &wrong_key_target, &wrong_key),
        Err(BackupError::AuthenticationFailed)
    );
    assert!(!wrong_key_target.exists());

    let mut tampered = artifact.clone();
    let final_index = tampered
        .len()
        .checked_sub(1)
        .ok_or("artifact cannot be empty")?;
    tampered[final_index] ^= 0x01;
    assert_eq!(
        preflight_encrypted_backup(&tampered, directory.path(), &key),
        Err(BackupError::AuthenticationFailed)
    );

    let existing_target = directory.join("existing.sqlite3");
    fs::write(&existing_target, b"must-not-be-replaced")?;
    assert_eq!(
        restore_encrypted_backup_to_empty_target(&artifact, &existing_target, &key),
        Err(BackupError::RestoreTargetUnavailable)
    );
    assert_eq!(fs::read(&existing_target)?, b"must-not-be-replaced");
    Ok(())
}

#[test]
fn malformed_and_oversized_material_are_rejected_before_staging_or_restore() -> TestResult {
    let directory = TemporaryDirectory::new()?;
    let key = backup_key(0xA5)?;
    let malformed = [0_u8; 16];
    assert_eq!(
        preflight_encrypted_backup(&malformed, directory.path(), &key),
        Err(BackupError::InvalidArtifact)
    );

    let too_large = vec![0_u8; gateway_store::backup::MAX_BACKUP_ARTIFACT_BYTES + 1];
    assert_eq!(
        preflight_encrypted_backup(&too_large, directory.path(), &key),
        Err(BackupError::InvalidArtifact)
    );
    Ok(())
}
