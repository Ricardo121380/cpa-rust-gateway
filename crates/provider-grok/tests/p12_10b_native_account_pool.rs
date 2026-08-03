//! P12-10B native Grok account aggregate, encrypted import, and rollback evidence.

#![deny(unsafe_code)]

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
use provider_grok::{
    GrokAccountCredential, GrokAccountIdentity, GrokAccountImport, GrokAccountPoolError,
    GrokAccountPoolStore, GrokAccountProvider,
};
use rusqlite::Connection;

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 1_800_000_000_000;
static TEMPORARY_DATABASE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn imports_three_isolated_providers_without_plaintext_or_identity_persistence() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store =
        GrokAccountPoolStore::try_new(Connection::open(database.path())?, secret_store(0xA5)?)?;
    let entries = vec![
        account(
            GrokAccountProvider::Build,
            b"build@example.invalid",
            b"build-secret",
            20,
        )?,
        account(
            GrokAccountProvider::Web,
            b"web@example.invalid",
            b"web-secret",
            10,
        )?,
        account(
            GrokAccountProvider::Console,
            b"console@example.invalid",
            b"console-secret",
            30,
        )?,
    ];

    let outcome = store.import_batch("batch-a", &entries, NOW_MS)?;
    assert_eq!(outcome.created, 3);
    assert_eq!(outcome.unchanged, 0);

    let accounts = store.list_accounts()?;
    assert_eq!(accounts.len(), 3);
    assert_eq!(
        accounts
            .iter()
            .map(|account| account.provider)
            .collect::<Vec<_>>(),
        vec![
            GrokAccountProvider::Build,
            GrokAccountProvider::Console,
            GrokAccountProvider::Web,
        ]
    );
    for account in &accounts {
        let opened = store.open_credential(&account.id)?;
        assert!(opened.as_bytes().ends_with(b"-secret"));
    }
    assert!(!format!("{entries:?}").contains("example.invalid"));
    assert!(!format!("{entries:?}").contains("build-secret"));

    drop(store);
    let database_bytes = fs::read(database.path())?;
    for plaintext in [
        b"build@example.invalid".as_slice(),
        b"web@example.invalid",
        b"console@example.invalid",
        b"build-secret",
        b"web-secret",
        b"console-secret",
    ] {
        assert!(
            !database_bytes
                .windows(plaintext.len())
                .any(|window| window == plaintext),
            "database retained native Grok import plaintext"
        );
    }
    Ok(())
}

#[test]
fn rerun_is_idempotent_but_changed_credential_rolls_back_the_whole_batch() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store =
        GrokAccountPoolStore::try_new(Connection::open(database.path())?, secret_store(0xA5)?)?;
    let original = vec![account(
        GrokAccountProvider::Build,
        b"stable-identity",
        b"stable-secret",
        5,
    )?];
    assert_eq!(store.import_batch("initial", &original, NOW_MS)?.created, 1);
    let rerun = store.import_batch("rerun", &original, NOW_MS + 1)?;
    assert_eq!(rerun.created, 0);
    assert_eq!(rerun.unchanged, 1);

    let conflicting = vec![
        account(
            GrokAccountProvider::Web,
            b"would-be-created",
            b"new-secret",
            1,
        )?,
        account(
            GrokAccountProvider::Build,
            b"stable-identity",
            b"changed-secret",
            5,
        )?,
    ];
    assert_eq!(
        store.import_batch("atomic-conflict", &conflicting, NOW_MS + 2),
        Err(GrokAccountPoolError::ExistingAccountConflict)
    );
    assert_eq!(store.list_accounts()?.len(), 1);

    let connection = Connection::open(database.path())?;
    let failed_batch_count: i64 = connection.query_row(
        "SELECT count(*) FROM grok_account_import_batches WHERE id = 'atomic-conflict'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(failed_batch_count, 0);
    Ok(())
}

#[test]
fn batch_rollback_cascades_links_and_is_repeatable() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store =
        GrokAccountPoolStore::try_new(Connection::open(database.path())?, secret_store(0xA5)?)?;
    let entries = vec![
        account(
            GrokAccountProvider::Build,
            b"linked-build",
            b"build-secret",
            1,
        )?,
        account(GrokAccountProvider::Web, b"linked-web", b"web-secret", 1)?,
    ];
    store.import_batch("linked", &entries, NOW_MS)?;
    let accounts = store.list_accounts()?;
    store.link_accounts(&accounts[0].id, &accounts[1].id, "same_operator", NOW_MS)?;

    let rollback = store.rollback_import_batch("linked", NOW_MS + 1)?;
    assert_eq!(rollback.removed, 2);
    assert!(!rollback.already_rolled_back);
    assert!(store.list_accounts()?.is_empty());
    let repeated = store.rollback_import_batch("linked", NOW_MS + 2)?;
    assert_eq!(repeated.removed, 0);
    assert!(repeated.already_rolled_back);

    let connection = Connection::open(database.path())?;
    let link_count: i64 =
        connection.query_row("SELECT count(*) FROM grok_account_links", [], |row| {
            row.get(0)
        })?;
    assert_eq!(link_count, 0);
    Ok(())
}

#[test]
fn duplicate_identity_invalid_bounds_and_wrong_key_fail_closed() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store =
        GrokAccountPoolStore::try_new(Connection::open(database.path())?, secret_store(0xA5)?)?;
    let duplicate = vec![
        account(GrokAccountProvider::Console, b"duplicate", b"one", 1)?,
        account(GrokAccountProvider::Console, b"duplicate", b"two", 1)?,
    ];
    assert_eq!(
        store.import_batch("duplicate", &duplicate, NOW_MS),
        Err(GrokAccountPoolError::DuplicateIdentity)
    );
    let invalid_schedule = GrokAccountImport {
        max_concurrency: 0,
        ..account(GrokAccountProvider::Web, b"invalid", b"secret", 1)?
    };
    assert_eq!(
        store.import_batch("invalid", &[invalid_schedule], NOW_MS),
        Err(GrokAccountPoolError::InvalidSchedulingMetadata)
    );

    let valid = vec![account(
        GrokAccountProvider::Console,
        b"wrong-key",
        b"protected-secret",
        1,
    )?];
    store.import_batch("valid", &valid, NOW_MS)?;
    let account_id = store.list_accounts()?[0].id.clone();
    drop(store);

    let wrong_key_store =
        GrokAccountPoolStore::try_new(Connection::open(database.path())?, secret_store(0x5A)?)?;
    assert!(matches!(
        wrong_key_store.open_credential(&account_id),
        Err(GrokAccountPoolError::SecretStoreFailure)
    ));
    Ok(())
}

fn account(
    provider: GrokAccountProvider,
    identity: &[u8],
    credential: &[u8],
    priority: i64,
) -> Result<GrokAccountImport, GrokAccountPoolError> {
    Ok(GrokAccountImport {
        provider,
        identity: GrokAccountIdentity::try_from_bytes(identity)?,
        credential: GrokAccountCredential::try_from_bytes(credential)?,
        enabled: true,
        priority,
        weight: 1,
        max_concurrency: 2,
        refresh_due_at_ms: Some(NOW_MS + 60_000),
    })
}

fn secret_store(fill: u8) -> Result<SecretStore, Box<dyn Error>> {
    let key_version = KeyVersion::try_new(1)?;
    Ok(SecretStore::new(MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([fill; 32])?)],
    )?))
}

struct TemporaryDatabase(PathBuf);

impl TemporaryDatabase {
    fn new() -> Result<Self, Box<dyn Error>> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        for _ in 0..64 {
            let sequence = TEMPORARY_DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "cpa-rust-gateway-p12-10b-{timestamp}-{}-{sequence}.sqlite3",
                std::process::id()
            ));
            if !path.exists() {
                return Ok(Self(path));
            }
        }
        Err("could not allocate isolated P12-10B database".into())
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        let _ = fs::remove_file(self.0.with_extension("sqlite3-shm"));
        let _ = fs::remove_file(self.0.with_extension("sqlite3-wal"));
    }
}
