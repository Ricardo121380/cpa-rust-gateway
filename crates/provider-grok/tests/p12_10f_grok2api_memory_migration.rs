//! P12-10F bounded grok2api-to-CPAR memory-stream migration evidence.

#![deny(unsafe_code)]

use std::{
    error::Error,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
use provider_grok::{
    GROK_BUILD_OAUTH_ISSUER, GROK_BUILD_OAUTH_SCOPE, GROK_BUILD_PUBLIC_CLIENT_ID,
    Grok2ApiMemoryStreamMigration, Grok2ApiMigrationFailureKind, GrokAccountCredential,
    GrokAccountIdentity, GrokAccountImport, GrokAccountPoolStore, GrokAccountProvider,
    MAX_GROK2API_MIGRATION_RECORD_BYTES,
};
use rusqlite::Connection;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 1_735_689_600_000;
static TEMPORARY_DATABASE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn imports_build_web_console_and_links_without_plaintext_persistence() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store =
        GrokAccountPoolStore::try_new(Connection::open(database.path())?, secret_store(0xA5)?)?;
    let stream = valid_stream("build-access-a", "web-cookie-a", "console-sso-a");
    let receipt =
        Grok2ApiMemoryStreamMigration::import(&store, "migration-a", Cursor::new(&stream), NOW_MS)?;
    assert_eq!(receipt.source_records, 5);
    assert_eq!(receipt.accepted_accounts, 3);
    assert_eq!(receipt.accepted_links, 2);
    assert_eq!(receipt.rejected_records, 0);
    assert_eq!(receipt.created_accounts, 3);
    assert_eq!(receipt.unchanged_accounts, 0);
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

    let rollback = store.rollback_import_batch("migration-a", NOW_MS + 1)?;
    assert_eq!(rollback.removed, 3);
    assert!(store.list_accounts()?.is_empty());
    let connection = Connection::open(database.path())?;
    let links: i64 =
        connection.query_row("SELECT count(*) FROM grok_account_links", [], |row| {
            row.get(0)
        })?;
    assert_eq!(links, 0);
    drop(connection);
    drop(store);

    let database_bytes = fs::read(database.path())?;
    for plaintext in [
        "build-identity-a",
        "build-access-a",
        "web-cookie-a",
        "console-sso-a",
    ] {
        assert!(
            !database_bytes
                .windows(plaintext.len())
                .any(|window| window == plaintext.as_bytes()),
            "migration plaintext persisted in the CPAR database"
        );
    }
    Ok(())
}

#[test]
fn exact_rerun_is_idempotent_with_value_free_counts() -> TestResult {
    let store = memory_store()?;
    let stream = valid_stream("build-access-b", "web-cookie-b", "console-sso-b");
    let first = Grok2ApiMemoryStreamMigration::import(
        &store,
        "migration-first",
        Cursor::new(&stream),
        NOW_MS,
    )?;
    let second = Grok2ApiMemoryStreamMigration::import(
        &store,
        "migration-rerun",
        Cursor::new(&stream),
        NOW_MS,
    )?;
    assert_eq!(first.created_accounts, 3);
    assert_eq!(second.created_accounts, 0);
    assert_eq!(second.unchanged_accounts, 3);
    assert_eq!(second.accepted_links, 2);
    assert_eq!(store.list_accounts()?.len(), 3);
    Ok(())
}

#[test]
fn any_rejected_record_prevents_the_whole_transaction() -> TestResult {
    let store = memory_store()?;
    let mut records = vec![account_record(
        "web-1",
        "grok_web",
        "web-identity-rejected",
        &web_credential("web-cookie-rejected"),
    )];
    records.push(json!({
        "kind":"link",
        "source_ref":"web-1",
        "target_ref":"missing-build",
        "relation":"web_build"
    }));
    records.push(json!({"kind":"unknown","credential":"must-not-render"}));
    records.push(account_record(
        "web-1",
        "grok_web",
        "duplicate-source-ref",
        &web_credential("duplicate-cookie"),
    ));
    let error = Grok2ApiMemoryStreamMigration::import(
        &store,
        "rejected",
        Cursor::new(encode_records(&records)),
        NOW_MS,
    )
    .err()
    .ok_or("a rejected record unexpectedly committed")?;
    assert_eq!(error.kind(), Grok2ApiMigrationFailureKind::RejectedRecords);
    assert_eq!(error.receipt().source_records, 4);
    assert_eq!(error.receipt().accepted_accounts, 1);
    assert_eq!(error.receipt().rejected_records, 3);
    assert!(store.list_accounts()?.is_empty());
    let diagnostic = format!("{error:?} {error}");
    for secret in [
        "web-cookie-rejected",
        "web-identity-rejected",
        "must-not-render",
        "duplicate-cookie",
    ] {
        assert!(!diagnostic.contains(secret));
    }
    Ok(())
}

#[test]
fn existing_conflict_rolls_back_new_accounts_links_and_batch_row() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store =
        GrokAccountPoolStore::try_new(Connection::open(database.path())?, secret_store(0xA5)?)?;
    let existing = GrokAccountImport {
        provider: GrokAccountProvider::Build,
        identity: GrokAccountIdentity::try_from_bytes("build-identity-a")?,
        credential: GrokAccountCredential::try_from_bytes(build_credential("original-access"))?,
        auth_status: provider_grok::GrokAccountAuthStatus::Active,
        enabled: true,
        priority: 1,
        weight: 1,
        max_concurrency: 2,
        refresh_due_at_ms: None,
        quota_sync_due_at_ms: None,
        cooldown_until_ms: None,
    };
    store.import_batch("preexisting", &[existing], NOW_MS)?;
    let stream = valid_stream("changed-access", "new-web-cookie", "new-console-sso");
    let error = Grok2ApiMemoryStreamMigration::import(
        &store,
        "atomic-conflict",
        Cursor::new(stream),
        NOW_MS,
    )
    .err()
    .ok_or("changed existing credential unexpectedly committed")?;
    assert_eq!(error.kind(), Grok2ApiMigrationFailureKind::ImportFailed);
    assert_eq!(store.list_accounts()?.len(), 1);
    let connection = Connection::open(database.path())?;
    let failed_batches: i64 = connection.query_row(
        "SELECT count(*) FROM grok_account_import_batches WHERE id = 'atomic-conflict'",
        [],
        |row| row.get(0),
    )?;
    let links: i64 =
        connection.query_row("SELECT count(*) FROM grok_account_links", [], |row| {
            row.get(0)
        })?;
    assert_eq!(failed_batches, 0);
    assert_eq!(links, 0);
    Ok(())
}

#[test]
fn oversized_record_stops_before_import() -> TestResult {
    let store = memory_store()?;
    let oversized = vec![b'x'; MAX_GROK2API_MIGRATION_RECORD_BYTES + 1];
    let error =
        Grok2ApiMemoryStreamMigration::import(&store, "oversized", Cursor::new(oversized), NOW_MS)
            .err()
            .ok_or("oversized line unexpectedly imported")?;
    assert_eq!(error.kind(), Grok2ApiMigrationFailureKind::SourceTooLarge);
    assert!(store.list_accounts()?.is_empty());
    Ok(())
}

fn valid_stream(build_access: &str, web_cookie: &str, console_sso: &str) -> Vec<u8> {
    let console_credential = json!({
        "sso_token": console_sso,
        "probe_model": "grok-4.3",
    })
    .to_string();
    encode_records(&[
        account_record(
            "build-1",
            "grok_build",
            "build-identity-a",
            &build_credential(build_access),
        ),
        account_record(
            "web-1",
            "grok_web",
            "web-identity-a",
            &web_credential(web_cookie),
        ),
        account_record(
            "console-1",
            "grok_console",
            "console-identity-a",
            &console_credential,
        ),
        json!({
            "kind":"link", "source_ref":"web-1", "target_ref":"build-1",
            "relation":"web_build"
        }),
        json!({
            "kind":"link", "source_ref":"web-1", "target_ref":"console-1",
            "relation":"web_console"
        }),
    ])
}

fn account_record(source_ref: &str, provider: &str, identity: &str, credential: &str) -> Value {
    json!({
        "kind":"account",
        "source_ref":source_ref,
        "provider":provider,
        "identity_key":identity,
        "credential":credential,
        "auth_status":"active",
        "enabled":true,
        "priority":1,
        "weight":1,
        "max_concurrency":2,
        "refresh_due_at_ms":null,
        "quota_sync_due_at_ms":null,
        "cooldown_until_ms":null
    })
}

fn build_credential(access_token: &str) -> String {
    json!({
        "access_token":access_token,
        "refresh_token":"synthetic-build-refresh",
        "expires_at":"2025-01-01T00:10:00Z",
        "client_id":GROK_BUILD_PUBLIC_CLIENT_ID,
        "issuer":GROK_BUILD_OAUTH_ISSUER,
        "scope":GROK_BUILD_OAUTH_SCOPE,
        "token_type":"Bearer"
    })
    .to_string()
}

fn web_credential(cookie: &str) -> String {
    json!({
        "kind":"grok_web_sso",
        "account_ref":"web-account-a",
        "lineage_ref":"migration-a",
        "revision":1,
        "expires_at_ms":NOW_MS + 60_000,
        "cookies":[{
            "name":"sso", "value":cookie, "domain":"grok.com", "path":"/",
            "secure":true, "http_only":true
        }]
    })
    .to_string()
}

fn encode_records(records: &[Value]) -> Vec<u8> {
    let mut output = Vec::new();
    for record in records {
        output.extend_from_slice(record.to_string().as_bytes());
        output.push(b'\n');
    }
    output
}

fn memory_store() -> Result<GrokAccountPoolStore, Box<dyn Error>> {
    Ok(GrokAccountPoolStore::try_new(
        Connection::open_in_memory()?,
        secret_store(0xA5)?,
    )?)
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
                "cpa-rust-gateway-p12-10f-{timestamp}-{}-{sequence}.sqlite3",
                std::process::id()
            ));
            if !path.exists() {
                return Ok(Self(path));
            }
        }
        Err("failed to allocate a temporary P12-10F database path".into())
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
