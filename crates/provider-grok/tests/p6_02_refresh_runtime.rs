//! P6-02 synthetic durable Grok Build refresh and revision evidence.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use gateway_core::CredentialId;
use gateway_store::{
    control_plane::ConfigVersionId,
    secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
};
use provider_grok::{
    GROK_BUILD_OAUTH_ISSUER, GROK_BUILD_PUBLIC_CLIENT_ID, GrokBuildCredential,
    GrokBuildCredentialCasOutcome, GrokBuildCredentialInsertOutcome, GrokBuildCredentialKey,
    GrokBuildCredentialKeyError, GrokBuildCredentialPersistence,
    GrokBuildCredentialRefreshCoordinator, GrokBuildCredentialRefreshError,
    GrokBuildCredentialRefreshOutcome, GrokBuildCredentialSource, GrokBuildCredentialSqliteStore,
    GrokBuildOAuthEndpoint, GrokBuildOAuthFlow, GrokBuildOAuthHttpResponse, GrokBuildOAuthRequest,
    GrokBuildOAuthRequestKind, GrokBuildOAuthTransport, GrokBuildOAuthTransportError,
};
use rusqlite::{Connection, params};

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 10_000;
static TEST_DATABASE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn sqlite_runtime_state_is_aead_sealed_revisioned_and_recovers_after_reopen() -> TestResult {
    let database = TestDatabase::new();
    let key = credential_key()?;
    let initial = credential("initial_access_012345", "initial_refresh_012345", 1, 0)?;
    let refreshed = credential(
        "refreshed_access_012345",
        "refreshed_refresh_012345",
        7_200,
        NOW_MS,
    )?;

    {
        let store = GrokBuildCredentialSqliteStore::open(database.path(), secret_store()?)?;
        let inserted = store.insert_if_absent(&key, &initial, 0)?;
        assert!(matches!(
            inserted,
            GrokBuildCredentialInsertOutcome::Inserted(ref version)
                if version.revision() == 0 && version.credential().access_token() == "initial_access_012345"
        ));

        let committed = store.compare_and_swap(&key, 0, &refreshed, NOW_MS)?;
        assert!(matches!(
            committed,
            GrokBuildCredentialCasOutcome::Committed(ref version)
                if version.revision() == 1 && version.credential().access_token() == "refreshed_access_012345"
        ));
        assert!(matches!(
            store.compare_and_swap(&key, 0, &initial, NOW_MS)?,
            GrokBuildCredentialCasOutcome::Conflict
        ));
    }

    let ciphertext = persisted_ciphertext(database.path(), &key)?;
    for plaintext in [
        b"initial_access_012345".as_slice(),
        b"refreshed_access_012345",
    ] {
        assert!(
            !ciphertext
                .windows(plaintext.len())
                .any(|window| window == plaintext),
            "AEAD ciphertext retained synthetic plaintext"
        );
    }

    let reopened = GrokBuildCredentialSqliteStore::open(database.path(), secret_store()?)?;
    let recovered = reopened
        .load(&key)?
        .ok_or("persisted Grok Build credential was absent after reopening")?;
    assert_eq!(recovered.revision(), 1);
    assert_eq!(
        recovered.credential().access_token(),
        "refreshed_access_012345"
    );
    assert_eq!(
        recovered.credential().refresh_token(),
        "refreshed_refresh_012345"
    );
    Ok(())
}

#[test]
fn indexed_cli_source_round_trips_through_aead_without_plaintext() -> TestResult {
    const OBSERVED_AT_MS: i64 = 1_735_689_600_000;
    let database = TestDatabase::new();
    let key = credential_key()?;
    let cache = format!(
        r#"{{
            "{GROK_BUILD_OAUTH_ISSUER}::{GROK_BUILD_PUBLIC_CLIENT_ID}":{{
                "key":"persisted_cli_access_012345",
                "refresh_token":"persisted_cli_refresh_012345",
                "expires_at":"2025-01-01T00:00:10Z"
            }}
        }}"#
    );
    let credential =
        GrokBuildCredential::import_official_cli_auth_cache(cache.as_bytes(), OBSERVED_AT_MS)?;
    let store = GrokBuildCredentialSqliteStore::open(database.path(), secret_store()?)?;
    store.insert_if_absent(&key, &credential, OBSERVED_AT_MS)?;

    let ciphertext = persisted_ciphertext(database.path(), &key)?;
    for plaintext in [
        b"persisted_cli_access_012345".as_slice(),
        b"persisted_cli_refresh_012345",
    ] {
        assert!(
            !ciphertext
                .windows(plaintext.len())
                .any(|window| window == plaintext),
            "AEAD ciphertext retained synthetic indexed-cache plaintext"
        );
    }

    let recovered = store
        .load(&key)?
        .ok_or("persisted indexed-cache credential was absent")?;
    assert_eq!(
        recovered.credential().source(),
        GrokBuildCredentialSource::OfficialCliAuthCache
    );
    assert_eq!(
        recovered.credential().access_token(),
        "persisted_cli_access_012345"
    );
    Ok(())
}

#[test]
fn concurrent_expiry_starts_one_refresh_and_all_callers_observe_the_new_revision() -> TestResult {
    let key = credential_key()?;
    let persistence = Arc::new(GrokBuildCredentialSqliteStore::open_in_memory(
        secret_store()?,
    )?);
    let coordinator = Arc::new(GrokBuildCredentialRefreshCoordinator::new(Arc::clone(
        &persistence,
    )));
    coordinator.initialize(&key, &expired_credential()?, 0)?;
    let transport = Arc::new(BlockingRefreshTransport::new(
        "refreshed_access_012345",
        "refreshed_refresh_012345",
    ));
    let flow = GrokBuildOAuthFlow::default();
    let start = Arc::new(std::sync::Barrier::new(5));
    let mut workers = Vec::new();

    for _ in 0..4 {
        let coordinator = Arc::clone(&coordinator);
        let key = key.clone();
        let transport = Arc::clone(&transport);
        let flow = flow.clone();
        let start = Arc::clone(&start);
        workers.push(thread::spawn(move || {
            start.wait();
            coordinator
                .refresh_if_expired(&key, &flow, transport.as_ref(), NOW_MS)
                .map(|outcome| match outcome {
                    GrokBuildCredentialRefreshOutcome::Current(version) => (
                        "current",
                        version.revision(),
                        version.credential().access_token().to_owned(),
                    ),
                    GrokBuildCredentialRefreshOutcome::Refreshed(version) => (
                        "refreshed",
                        version.revision(),
                        version.credential().access_token().to_owned(),
                    ),
                    GrokBuildCredentialRefreshOutcome::Superseded(version) => (
                        "superseded",
                        version.revision(),
                        version.credential().access_token().to_owned(),
                    ),
                })
                .map_err(|error| error.to_string())
        }));
    }
    start.wait();
    transport.wait_until_started()?;
    transport.release()?;

    let mut outcomes = Vec::new();
    for worker in workers {
        outcomes.push(worker.join().map_err(|_| "refresh worker panicked")??);
    }
    assert_eq!(transport.calls(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|(kind, _, _)| *kind == "refreshed")
            .count(),
        1
    );
    assert!(outcomes.iter().all(|(_, revision, access_token)| {
        *revision == 1 && access_token == "refreshed_access_012345"
    }));
    Ok(())
}

#[test]
fn stale_refresh_result_cannot_overwrite_an_external_newer_revision() -> TestResult {
    let key = credential_key()?;
    let persistence = Arc::new(GrokBuildCredentialSqliteStore::open_in_memory(
        secret_store()?,
    )?);
    let coordinator = Arc::new(GrokBuildCredentialRefreshCoordinator::new(Arc::clone(
        &persistence,
    )));
    coordinator.initialize(&key, &expired_credential()?, 0)?;
    let transport = Arc::new(BlockingRefreshTransport::new(
        "late_refresh_access_012345",
        "late_refresh_token_012345",
    ));
    let worker_coordinator = Arc::clone(&coordinator);
    let worker_key = key.clone();
    let worker_transport = Arc::clone(&transport);
    let worker = thread::spawn(move || {
        worker_coordinator.refresh_if_expired(
            &worker_key,
            &GrokBuildOAuthFlow::default(),
            worker_transport.as_ref(),
            NOW_MS,
        )
    });

    transport.wait_until_started()?;
    let external = credential(
        "external_winner_access_012345",
        "external_winner_refresh_012345",
        7_200,
        NOW_MS,
    )?;
    assert!(matches!(
        persistence.compare_and_swap(&key, 0, &external, NOW_MS)?,
        GrokBuildCredentialCasOutcome::Committed(ref version)
            if version.revision() == 1 && version.credential().access_token() == "external_winner_access_012345"
    ));
    transport.release()?;

    let outcome = worker.join().map_err(|_| "refresh worker panicked")??;
    assert!(matches!(
        outcome,
        GrokBuildCredentialRefreshOutcome::Superseded(ref version)
            if version.revision() == 1 && version.credential().access_token() == "external_winner_access_012345"
    ));
    let durable = persistence
        .load(&key)?
        .ok_or("durable credential disappeared after CAS conflict")?;
    assert_eq!(durable.revision(), 1);
    assert_eq!(
        durable.credential().access_token(),
        "external_winner_access_012345"
    );
    assert_ne!(
        durable.credential().access_token(),
        "late_refresh_access_012345"
    );
    assert_eq!(transport.calls(), 1);
    Ok(())
}

#[test]
fn expired_external_cas_winner_is_a_safe_retry_state_not_a_transport_failure() -> TestResult {
    let key = credential_key()?;
    let persistence = Arc::new(GrokBuildCredentialSqliteStore::open_in_memory(
        secret_store()?,
    )?);
    let coordinator = Arc::new(GrokBuildCredentialRefreshCoordinator::new(Arc::clone(
        &persistence,
    )));
    coordinator.initialize(&key, &expired_credential()?, 0)?;
    let transport = Arc::new(BlockingRefreshTransport::new(
        "late_refresh_access_012345",
        "late_refresh_token_012345",
    ));
    let worker_coordinator = Arc::clone(&coordinator);
    let worker_key = key.clone();
    let worker_transport = Arc::clone(&transport);
    let worker = thread::spawn(move || {
        worker_coordinator.refresh_if_expired(
            &worker_key,
            &GrokBuildOAuthFlow::default(),
            worker_transport.as_ref(),
            NOW_MS,
        )
    });

    transport.wait_until_started()?;
    let expired_external = credential(
        "external_expired_access_012345",
        "external_expired_refresh_012345",
        1,
        0,
    )?;
    assert!(matches!(
        persistence.compare_and_swap(&key, 0, &expired_external, NOW_MS)?,
        GrokBuildCredentialCasOutcome::Committed(ref version)
            if version.revision() == 1 && version.credential().is_expired_at(NOW_MS)
    ));
    transport.release()?;

    assert!(matches!(
        worker.join().map_err(|_| "refresh worker panicked")?,
        Err(GrokBuildCredentialRefreshError::ConcurrentCredentialStateChanged)
    ));
    let durable = persistence
        .load(&key)?
        .ok_or("expired external winner disappeared")?;
    assert_eq!(durable.revision(), 1);
    assert_eq!(
        durable.credential().access_token(),
        "external_expired_access_012345"
    );
    assert_eq!(transport.calls(), 1);
    Ok(())
}

#[test]
fn waiter_times_out_without_starting_a_second_refresh() -> TestResult {
    let key = credential_key()?;
    let persistence = Arc::new(GrokBuildCredentialSqliteStore::open_in_memory(
        secret_store()?,
    )?);
    assert!(
        GrokBuildCredentialRefreshCoordinator::try_new_with_wait_timeout(
            Arc::clone(&persistence),
            Duration::ZERO,
        )
        .is_err()
    );
    let coordinator = Arc::new(
        GrokBuildCredentialRefreshCoordinator::try_new_with_wait_timeout(
            Arc::clone(&persistence),
            Duration::from_millis(25),
        )?,
    );
    coordinator.initialize(&key, &expired_credential()?, 0)?;
    let transport = Arc::new(BlockingRefreshTransport::new(
        "timeout_refresh_access_012345",
        "timeout_refresh_token_012345",
    ));
    let worker_coordinator = Arc::clone(&coordinator);
    let worker_key = key.clone();
    let worker_transport = Arc::clone(&transport);
    let leader = thread::spawn(move || {
        worker_coordinator.refresh_if_expired(
            &worker_key,
            &GrokBuildOAuthFlow::default(),
            worker_transport.as_ref(),
            NOW_MS,
        )
    });

    transport.wait_until_started()?;
    let started = Instant::now();
    let waiter = coordinator.refresh_if_expired(
        &key,
        &GrokBuildOAuthFlow::default(),
        transport.as_ref(),
        NOW_MS,
    );
    assert!(matches!(
        waiter,
        Err(GrokBuildCredentialRefreshError::RefreshLockTimedOut)
    ));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(transport.calls(), 1);
    transport.release()?;
    assert!(matches!(
        leader.join().map_err(|_| "refresh leader panicked")??,
        GrokBuildCredentialRefreshOutcome::Refreshed(_)
    ));
    Ok(())
}

#[test]
fn runtime_identity_rejects_blank_or_oversized_components_before_aad_or_sqlite() -> TestResult {
    let oversized_config_version = ConfigVersionId::try_new("x".repeat(129))?;
    let credential_id = CredentialId::try_new("credential-p6-02")?;
    assert_eq!(
        GrokBuildCredentialKey::try_new(oversized_config_version, credential_id).err(),
        Some(GrokBuildCredentialKeyError::InvalidConfigVersionId)
    );
    let blank_config_version = ConfigVersionId::try_new("   ")?;
    let credential_id = CredentialId::try_new("credential-p6-02")?;
    assert_eq!(
        GrokBuildCredentialKey::try_new(blank_config_version, credential_id).err(),
        Some(GrokBuildCredentialKeyError::InvalidConfigVersionId)
    );
    let config_version = ConfigVersionId::try_new("config-version-p6-02")?;
    let oversized_credential_id = CredentialId::try_new("x".repeat(129))?;
    assert_eq!(
        GrokBuildCredentialKey::try_new(config_version, oversized_credential_id).err(),
        Some(GrokBuildCredentialKeyError::InvalidCredentialId)
    );
    Ok(())
}

fn credential_key() -> Result<GrokBuildCredentialKey, Box<dyn Error>> {
    Ok(GrokBuildCredentialKey::try_new(
        ConfigVersionId::try_new("config-version-p6-02")?,
        CredentialId::try_new("credential-p6-02")?,
    )?)
}

fn expired_credential() -> Result<GrokBuildCredential, provider_grok::GrokBuildOAuthError> {
    credential("expired_access_012345", "expired_refresh_012345", 1, 0)
}

fn credential(
    access_token: &str,
    refresh_token: &str,
    expires_in: u64,
    observed_at_ms: i64,
) -> Result<GrokBuildCredential, provider_grok::GrokBuildOAuthError> {
    GrokBuildCredential::import_json(
        format!(
            r#"{{"access_token":"{access_token}","refresh_token":"{refresh_token}","expires_in":{expires_in}}}"#
        )
        .as_bytes(),
        observed_at_ms,
    )
}

fn secret_store() -> Result<SecretStore, gateway_store::secret_store::SecretStoreError> {
    let key_version = KeyVersion::try_new(1)?;
    Ok(SecretStore::new(MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x62_u8; 32])?)],
    )?))
}

fn persisted_ciphertext(path: &Path, key: &GrokBuildCredentialKey) -> rusqlite::Result<Vec<u8>> {
    let connection = Connection::open(path)?;
    connection.query_row(
        "SELECT ciphertext FROM grok_build_credential_runtime \
         WHERE config_version_id = ?1 AND credential_id = ?2",
        params![
            key.config_version_id().as_str(),
            key.credential_id().as_str()
        ],
        |row| row.get(0),
    )
}

struct BlockingRefreshTransport {
    calls: AtomicUsize,
    started: (Mutex<bool>, Condvar),
    released: (Mutex<bool>, Condvar),
    access_token: &'static str,
    refresh_token: &'static str,
}

impl BlockingRefreshTransport {
    fn new(access_token: &'static str, refresh_token: &'static str) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            started: (Mutex::new(false), Condvar::new()),
            released: (Mutex::new(false), Condvar::new()),
            access_token,
            refresh_token,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn wait_until_started(&self) -> TestResult {
        let (started, ready) = &self.started;
        let started = started
            .lock()
            .map_err(|_| "refresh transport start lock poisoned")?;
        let (started, timeout) = ready
            .wait_timeout_while(started, Duration::from_secs(1), |value| !*value)
            .map_err(|_| "refresh transport start lock poisoned")?;
        if !*started || timeout.timed_out() {
            return Err("refresh transport did not start in time".into());
        }
        Ok(())
    }

    fn release(&self) -> TestResult {
        let (released, ready) = &self.released;
        let mut released = released
            .lock()
            .map_err(|_| "refresh transport release lock poisoned")?;
        *released = true;
        ready.notify_all();
        Ok(())
    }
}

impl GrokBuildOAuthTransport for BlockingRefreshTransport {
    fn send(
        &self,
        request: GrokBuildOAuthRequest,
    ) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthTransportError> {
        if request.endpoint() != GrokBuildOAuthEndpoint::Token
            || request.kind() != GrokBuildOAuthRequestKind::Refresh
        {
            return Err(GrokBuildOAuthTransportError::Unavailable);
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        let (started, ready) = &self.started;
        let mut started = started
            .lock()
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?;
        *started = true;
        ready.notify_all();
        drop(started);

        let (released, ready) = &self.released;
        let released = released
            .lock()
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?;
        let (released, _) = ready
            .wait_timeout_while(released, Duration::from_secs(2), |value| !*value)
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?;
        if !*released {
            return Err(GrokBuildOAuthTransportError::Unavailable);
        }
        GrokBuildOAuthHttpResponse::try_new(
            200,
            format!(
                r#"{{"access_token":"{}","refresh_token":"{}","expires_in":7200}}"#,
                self.access_token, self.refresh_token
            )
            .into_bytes(),
        )
        .map_err(|_| GrokBuildOAuthTransportError::Unavailable)
    }
}

struct TestDatabase {
    path: PathBuf,
}

impl TestDatabase {
    fn new() -> Self {
        let sequence = TEST_DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self {
            path: std::env::temp_dir().join(format!(
                "cpa-rust-gateway-p6-02-{}-{sequence}.sqlite",
                std::process::id()
            )),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        for suffix in ["", "-journal", "-shm", "-wal"] {
            let mut path = self.path.as_os_str().to_os_string();
            path.push(suffix);
            let _ = fs::remove_file(path);
        }
    }
}
