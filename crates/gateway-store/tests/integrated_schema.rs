//! The two historical schema41 branches must never be confused or partially downgraded.
#![deny(unsafe_code)]

use std::{error::Error, fs, io::Cursor, path::PathBuf};

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use gateway_store::{
    StoreError,
    backup::{
        BackupError, BackupKey, preflight_encrypted_backup, preflight_encrypted_backup_reader,
        restore_encrypted_backup_reader_to_empty_target, restore_encrypted_backup_to_empty_target,
    },
    migrate, open_in_memory, rollback_to_version, schema_version,
};
use rusqlite::Connection;

type TestResult = Result<(), Box<dyn Error>>;

fn historical40() -> Result<Connection, Box<dyn Error>> {
    let db = open_in_memory()?;
    db.execute_batch("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP) STRICT;")?;
    let mut paths: Vec<_> =
        fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"))?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<_, _>>()?;
    paths.sort();
    for path in paths {
        let name = path.file_name().ok_or("migration name")?.to_string_lossy();
        if !name.ends_with(".up.sql") {
            continue;
        }
        let version: i64 = name[..4].parse()?;
        if version > 40 {
            continue;
        }
        db.execute_batch(&fs::read_to_string(path)?)?;
        db.execute(
            "INSERT INTO schema_migrations(version) VALUES (?)",
            [version],
        )?;
    }
    db.execute_batch(
        "CREATE TABLE caller_retained(value BLOB); INSERT INTO caller_retained VALUES (X'01FF71');",
    )?;
    Ok(db)
}

fn routing41(db: &Connection) -> TestResult {
    db.execute_batch(include_str!(
        "../migrations/0041_routing_build_affinity.up.sql"
    ))?;
    db.execute("INSERT INTO schema_migrations(version) VALUES (41)", [])?;
    Ok(())
}

fn assert_retained(db: &Connection) -> TestResult {
    assert_eq!(
        db.query_row("SELECT value FROM caller_retained", [], |r| r
            .get::<_, Vec<u8>>(0))?,
        [1, 255, 113]
    );
    assert_eq!(
        db.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))?,
        "ok"
    );
    assert!(!db.prepare("PRAGMA foreign_key_check")?.exists([])?);
    Ok(())
}

#[test]
fn integrated_schema_upgrades_historical40_and_routing41_without_data_loss() -> TestResult {
    for old in [40, 41] {
        let mut db = historical40()?;
        if old == 41 {
            routing41(&db)?;
        }
        assert_eq!(schema_version(&db)?, Some(old));
        migrate(&mut db)?;
        assert_eq!(schema_version(&db)?, Some(42));
        assert_retained(&db)?;
        rollback_to_version(&mut db, 40)?;
        assert_eq!(schema_version(&db)?, Some(40));
        assert_retained(&db)?;
    }
    Ok(())
}

#[test]
fn integrated_schema_rejects_standalone_policy41_and_mixed_history_without_writes() -> TestResult {
    for mixed in [false, true] {
        let mut db = historical40()?;
        if mixed {
            routing41(&db)?;
        }
        db.execute_batch(include_str!(
            "../migrations/0042_endpoint_request_policy.up.sql"
        ))?;
        if !mixed {
            db.execute("INSERT INTO schema_migrations(version) VALUES (41)", [])?;
        }
        let before: String = db.query_row(
            "SELECT group_concat(sql, ';') FROM sqlite_schema",
            [],
            |r| r.get(0),
        )?;
        assert!(matches!(
            schema_version(&db),
            Err(StoreError::UnsupportedMigrationHistory { .. })
        ));
        assert!(matches!(
            migrate(&mut db),
            Err(StoreError::UnsupportedMigrationHistory { .. })
        ));
        assert!(matches!(
            rollback_to_version(&mut db, 40),
            Err(StoreError::UnsupportedMigrationHistory { .. })
        ));
        assert_eq!(
            db.query_row(
                "SELECT group_concat(sql, ';') FROM sqlite_schema",
                [],
                |r| r.get::<_, String>(0)
            )?,
            before
        );
        assert_retained(&db)?;
    }
    Ok(())
}

#[test]
fn integrated_schema_failed_multistep_downgrade_preserves_all_columns_and_history() -> TestResult {
    let mut db = historical40()?;
    migrate(&mut db)?;
    db.execute_batch("INSERT INTO grok_build_affinity_breaks(client_key_id,upstream_model,cache_identity,prior_credential_id,next_credential_id,reason,estimated_cache_loss_tokens,occurred_at_ms) VALUES ('owner','model','opaque','old','new','policy_reselection',NULL,1);")?;
    assert!(rollback_to_version(&mut db, 40).is_err());
    assert_eq!(schema_version(&db)?, Some(42));
    assert!(
        db.prepare("SELECT request_policy_json FROM upstream_endpoints")?
            .column_count()
            == 1
    );
    assert_eq!(
        db.query_row("SELECT reason FROM grok_build_affinity_breaks", [], |r| {
            r.get::<_, String>(0)
        })?,
        "policy_reselection"
    );
    assert_retained(&db)
}

#[test]
fn integrated_schema_policy_downgrade_preserves_nonempty_and_accepts_semantic_empty() -> TestResult
{
    for (policy, allowed) in [
        ("{}", true),
        (r#"{"rules":[],"headers":{}}"#, true),
        (r#"{"headers":{"x-tag":"secret-reference"}}"#, false),
        (
            r#"{"rules":[{"models":["*"],"default":{"temperature":0.2}}]}"#,
            false,
        ),
    ] {
        let mut db = historical40()?;
        migrate(&mut db)?;
        db.execute_batch("INSERT INTO config_versions(id,status,created_at_ms,description) VALUES ('v','draft',1,'fixture'); INSERT INTO upstreams(config_version_id,id,name,kind,enabled,tags_json) VALUES ('v','u','u','relay',1,'[]');")?;
        db.execute("INSERT INTO upstream_endpoints(config_version_id,id,upstream_id,adapter_id,api_format,base_url,inference_path,transport,enabled,request_policy_json) VALUES ('v','e','u','openai-compatible.responses','openai/responses','https://synthetic.invalid','/responses','http',1,?)", [policy])?;
        assert_eq!(rollback_to_version(&mut db, 41).is_ok(), allowed);
        assert_eq!(schema_version(&db)?, Some(if allowed { 41 } else { 42 }));
        if !allowed {
            assert_eq!(
                db.query_row(
                    "SELECT request_policy_json FROM upstream_endpoints",
                    [],
                    |r| r.get::<_, String>(0)
                )?,
                policy
            );
        }
    }
    Ok(())
}

#[test]
fn integrated_schema_wrong41_backup_rejected_without_publishing_target() -> TestResult {
    let db = historical40()?;
    db.execute_batch(include_str!(
        "../migrations/0042_endpoint_request_policy.up.sql"
    ))?;
    db.execute("INSERT INTO schema_migrations(version) VALUES (41)", [])?;
    let dir = std::env::temp_dir().join(format!(
        "wave3-wrong41-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&dir)?;
    let snapshot = dir.join("snapshot.sqlite3");
    let mut target = Connection::open(&snapshot)?;
    let copy = rusqlite::backup::Backup::new(&db, &mut target)?;
    assert_eq!(copy.step(-1)?, rusqlite::backup::StepResult::Done);
    drop(copy);
    drop(target);
    // Historical v1 format, sealed independently: the new producer must not bless wrong41.
    let nonce = [0x42; 24];
    let key = BackupKey::try_from_bytes([0x71; 32])?;
    let mut artifact = b"CPABKP01\x01".to_vec();
    artifact.extend_from_slice(&41_u32.to_be_bytes());
    artifact.extend_from_slice(&nonce);
    let ciphertext = XChaCha20Poly1305::new_from_slice(&[0x71; 32])
        .map_err(|_| "key")?
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &fs::read(&snapshot)?,
                aad: &artifact,
            },
        )
        .map_err(|_| "seal")?;
    artifact.extend(ciphertext);
    assert!(!preflight_encrypted_backup(&artifact, &dir, &key)?.compatible());
    assert!(
        !preflight_encrypted_backup_reader(&mut Cursor::new(&artifact), &dir, &key)?.compatible()
    );
    let restored = dir.join("restored.sqlite3");
    assert!(matches!(
        restore_encrypted_backup_to_empty_target(&artifact, &restored, &key),
        Err(BackupError::IncompatibleSchema)
    ));
    assert!(matches!(
        restore_encrypted_backup_reader_to_empty_target(
            &mut Cursor::new(&artifact),
            &restored,
            &key
        ),
        Err(BackupError::IncompatibleSchema)
    ));
    assert!(!restored.exists());
    assert_retained(&db)?;
    fs::remove_dir_all(dir)?;
    Ok(())
}
