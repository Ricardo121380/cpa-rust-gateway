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

#[test]
fn chunked_backup_authenticates_order_length_and_legacy_compatibility() -> TestResult {
    use gateway_store::backup::{
        create_encrypted_backup_file, preflight_encrypted_backup_reader,
        restore_encrypted_backup_reader_to_empty_target,
    };
    let directory = TemporaryDirectory::new()?;
    let mut source = populated_source(&directory.join("source.sqlite3"))?;
    source.set_transaction_behavior(rusqlite::TransactionBehavior::Immediate);
    let key = backup_key(0xA5)?;
    let artifact_path = directory.join("chunked.backup");
    create_encrypted_backup_file(&source, directory.path(), &artifact_path, &key)?;
    let artifact = fs::read(&artifact_path)?;
    assert_eq!(&artifact[..9], b"CPABKP02\x02");
    assert!(artifact.len() > 2 * 65552 + 45);
    assert!(
        preflight_encrypted_backup_reader(&mut artifact.as_slice(), directory.path(), &key)?
            .compatible()
    );
    let target = directory.join("restored.sqlite3");
    restore_encrypted_backup_reader_to_empty_target(&mut artifact.as_slice(), &target, &key)?;
    assert_eq!(
        schema_version(&open(&target)?)?,
        Some(CURRENT_SCHEMA_VERSION)
    );
    assert_eq!(
        restore_encrypted_backup_reader_to_empty_target(&mut artifact.as_slice(), &target, &key),
        Err(BackupError::RestoreTargetUnavailable)
    );
    assert_eq!(
        preflight_encrypted_backup_reader(
            &mut artifact.as_slice(),
            directory.path(),
            &backup_key(0x5A)?
        ),
        Err(BackupError::AuthenticationFailed)
    );

    let mut cases = Vec::new();
    cases.push(artifact[..artifact.len() - 1].to_vec());
    let mut appended = artifact.clone();
    appended.push(0);
    cases.push(appended);
    let mut modified = artifact.clone();
    modified[45] ^= 1;
    cases.push(modified);
    let mut header = artifact.clone();
    header[12] ^= 1;
    cases.push(header);
    let mut reordered = artifact.clone();
    for offset in 0..65552 {
        reordered.swap(45 + offset, 45 + 65552 + offset);
    }
    cases.push(reordered);
    let mut oversized = artifact[..45].to_vec();
    oversized[37..45].copy_from_slice(
        &(gateway_store::backup::MAX_STREAM_BACKUP_PLAINTEXT_BYTES as u64 + 1).to_be_bytes(),
    );
    cases.push(oversized);
    for (index, bytes) in cases.iter().enumerate() {
        let target = directory.join(&format!("invalid-{index}.sqlite3"));
        assert!(
            restore_encrypted_backup_reader_to_empty_target(&mut bytes.as_slice(), &target, &key)
                .is_err()
        );
        assert!(!target.exists());
    }
    let legacy = create_encrypted_backup(&source, directory.path(), &key)?;
    assert!(
        preflight_encrypted_backup_reader(&mut legacy.as_slice(), directory.path(), &key)?
            .compatible()
    );
    restore_encrypted_backup_reader_to_empty_target(
        &mut legacy.as_slice(),
        &directory.join("legacy.sqlite3"),
        &key,
    )?;
    Ok(())
}

#[test]
fn chunked_backup_write_failure_and_restore_interruption_preserve_source() -> TestResult {
    use gateway_store::backup::{
        create_encrypted_backup_to_writer, restore_encrypted_backup_reader_to_empty_target,
    };
    use std::io::{self, Read, Write};
    struct Full(usize);
    impl Write for Full {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0 == 0 {
                return Err(io::Error::from_raw_os_error(libc::ENOSPC));
            }
            let written = bytes.len().min(self.0);
            self.0 -= written;
            Ok(written)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    struct Interrupted;
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::ConnectionReset))
        }
    }
    let directory = TemporaryDirectory::new()?;
    let path = directory.join("source.sqlite3");
    let source = populated_source(&path)?;
    let key = backup_key(0xA5)?;
    assert_eq!(
        create_encrypted_backup_to_writer(&source, directory.path(), &mut Full(45 + 65552), &key),
        Err(BackupError::FilesystemUnavailable)
    );
    let target = directory.join("interrupted.sqlite3");
    assert_eq!(
        restore_encrypted_backup_reader_to_empty_target(&mut Interrupted, &target, &key),
        Err(BackupError::InvalidArtifact)
    );
    assert!(!target.exists());
    assert_eq!(
        source.query_row("SELECT count(*) FROM config_versions", [], |row| row
            .get::<_, i64>(0))?,
        1
    );
    assert_eq!(
        fs::read_dir(directory.path())?
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".cpa-backup-"))
            .count(),
        0
    );
    Ok(())
}

#[test]
#[ignore = "explicit disk-capacity boundary; allocates an owned database larger than 128 MiB"]
fn chunked_backup_rejects_oversized_source_before_artifact_publication() -> TestResult {
    let directory = TemporaryDirectory::new()?;
    let source = populated_source(&directory.join("large.sqlite3"))?;
    source.execute_batch("CREATE TABLE capacity_fixture (material BLOB)")?;
    source.execute(
        "INSERT INTO capacity_fixture VALUES(zeroblob(?1))",
        [i64::try_from(
            gateway_store::backup::MAX_STREAM_BACKUP_PLAINTEXT_BYTES,
        )?],
    )?;
    let target = directory.join("must-not-exist.backup");
    assert_eq!(
        gateway_store::backup::create_encrypted_backup_file(
            &source,
            directory.path(),
            &target,
            &backup_key(0xA5)?,
        ),
        Err(BackupError::MaterialTooLarge)
    );
    assert!(!target.exists());
    assert_eq!(
        source.query_row("SELECT length(material) FROM capacity_fixture", [], |row| {
            row.get::<_, i64>(0)
        })?,
        i64::try_from(gateway_store::backup::MAX_STREAM_BACKUP_PLAINTEXT_BYTES)?
    );
    Ok(())
}

#[test]
fn chunked_backup_keeps_embedding_read_snapshot_and_transaction() -> TestResult {
    let directory = TemporaryDirectory::new()?;
    let source_path = directory.join("source.sqlite3");
    let source = populated_source(&source_path)?;
    source.execute_batch("BEGIN DEFERRED")?;
    assert_eq!(
        source.query_row("SELECT count(*) FROM config_versions", [], |row| row
            .get::<_, i64>(0))?,
        1
    );
    open(&source_path)?.execute("INSERT INTO config_versions(id,status,created_at_ms,description) VALUES('later','draft',2,'concurrent')", [])?;
    let artifact = directory.join("pinned.backup");
    let key = backup_key(0xA5)?;
    gateway_store::backup::create_encrypted_backup_file(
        &source,
        directory.path(),
        &artifact,
        &key,
    )?;
    assert!(!source.is_autocommit());
    source.execute_batch("ROLLBACK")?;
    assert_eq!(
        source.query_row("SELECT count(*) FROM config_versions", [], |row| row
            .get::<_, i64>(0))?,
        2
    );
    let target = directory.join("pinned.sqlite3");
    gateway_store::backup::restore_encrypted_backup_reader_to_empty_target(
        &mut fs::File::open(artifact)?,
        &target,
        &key,
    )?;
    assert_eq!(
        open(target)?.query_row("SELECT count(*) FROM config_versions", [], |row| row
            .get::<_, i64>(0))?,
        1
    );
    Ok(())
}
