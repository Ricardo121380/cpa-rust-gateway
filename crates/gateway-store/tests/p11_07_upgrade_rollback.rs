//! P11-07 deterministic in-place schema upgrade, downgrade, and backup-recovery rehearsal.
//!
//! The drill operates only on temporary `SQLite` files with a fixed synthetic backup key. It does
//! not open a production database, read configuration, contact a provider, or start a process.

#![deny(unsafe_code)]

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_store::{
    CURRENT_SCHEMA_VERSION, StoreError,
    backup::{BackupKey, create_encrypted_backup, restore_encrypted_backup_to_empty_target},
    migrate, open, rollback_to_version, schema_version,
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
                "cpa-rust-gateway-p11-07-{timestamp}-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not allocate an isolated temporary P11-07 directory".into())
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

#[test]
fn prior_schema_upgrades_in_place_and_backup_recovers_after_a_lossy_downgrade() -> TestResult {
    let previous_schema = CURRENT_SCHEMA_VERSION
        .checked_sub(1)
        .ok_or("current schema has no preceding version for a downgrade drill")?;
    let directory = TemporaryDirectory::new()?;
    let source_path = directory.join("source.sqlite3");
    let restore_path = directory.join("restored.sqlite3");
    let mut source = open(&source_path)?;
    migrate(&mut source)?;
    rollback_to_version(&mut source, previous_schema)?;
    assert_eq!(schema_version(&source)?, Some(previous_schema));
    assert!(!table_exists(&source, "grok_account_quota_windows")?);

    source.execute(
        "INSERT INTO config_versions (id, parent_id, status, created_at_ms, description) \
         VALUES (?1, NULL, 'draft', 1, ?2)",
        ("p11-07-version", "P11-07 legacy configuration"),
    )?;
    assert!(matches!(
        rollback_to_version(&mut source, CURRENT_SCHEMA_VERSION),
        Err(StoreError::UnsupportedRollbackTarget { .. })
    ));
    assert!(matches!(
        rollback_to_version(&mut source, -1),
        Err(StoreError::UnsupportedRollbackTarget { .. })
    ));

    migrate(&mut source)?;
    assert_eq!(schema_version(&source)?, Some(CURRENT_SCHEMA_VERSION));
    assert_integrity(&source)?;
    source.execute(
        "INSERT INTO management_resource_audit_events \
         (action, actor, occurred_at_ms, config_version_id, resource_kind, resource_id) \
         VALUES (?1, ?2, 2, ?3, ?4, ?5)",
        (
            "p11_07_upgrade",
            "p11-07-operator",
            "p11-07-version",
            "config_version",
            "p11-07-version",
        ),
    )?;
    let backup_key = BackupKey::try_from_bytes([0xA5; 32])?;
    let artifact = create_encrypted_backup(&source, directory.path(), &backup_key)?;

    rollback_to_version(&mut source, previous_schema)?;
    assert_eq!(schema_version(&source)?, Some(previous_schema));
    assert_eq!(
        configuration_description(&source)?,
        "P11-07 legacy configuration"
    );
    assert!(!table_exists(&source, "grok_account_quota_windows")?);
    assert_integrity(&source)?;

    migrate(&mut source)?;
    assert_eq!(schema_version(&source)?, Some(CURRENT_SCHEMA_VERSION));
    assert_eq!(audit_event_count(&source)?, 1);
    assert_integrity(&source)?;

    let restored = restore_encrypted_backup_to_empty_target(&artifact, &restore_path, &backup_key)?;
    assert_eq!(restored.source_schema_version(), CURRENT_SCHEMA_VERSION);
    assert_eq!(restored.restored_schema_version(), CURRENT_SCHEMA_VERSION);
    let restored = open(&restore_path)?;
    assert_eq!(schema_version(&restored)?, Some(CURRENT_SCHEMA_VERSION));
    assert_eq!(
        configuration_description(&restored)?,
        "P11-07 legacy configuration"
    );
    assert_eq!(audit_event_count(&restored)?, 1);
    assert_integrity(&restored)?;
    Ok(())
}

fn configuration_description(connection: &rusqlite::Connection) -> Result<String, rusqlite::Error> {
    connection.query_row(
        "SELECT description FROM config_versions WHERE id = 'p11-07-version'",
        [],
        |row| row.get(0),
    )
}

fn audit_event_count(connection: &rusqlite::Connection) -> Result<i64, rusqlite::Error> {
    connection.query_row(
        "SELECT count(*) FROM management_resource_audit_events",
        [],
        |row| row.get(0),
    )
}

fn table_exists(connection: &rusqlite::Connection, table: &str) -> Result<bool, rusqlite::Error> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )
}

fn assert_integrity(connection: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let quick_check: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    assert_eq!(quick_check, "ok");
    let foreign_key_violations: i64 =
        connection.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    assert_eq!(foreign_key_violations, 0);
    Ok(())
}
