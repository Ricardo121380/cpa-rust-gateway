//! P12-10D durable native Grok refresh/quota worker evidence.

#![deny(unsafe_code)]

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gateway_core::{CredentialId, EndpointId};
use gateway_router::{
    RuntimeHealthClock, RuntimeHealthClockError, RuntimeQuotaRegistry, RuntimeQuotaTarget,
};
use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
use provider_grok::{
    GrokAccountAuthStatus, GrokAccountCredential, GrokAccountEndpointBinding, GrokAccountIdentity,
    GrokAccountImport, GrokAccountPoolStore, GrokAccountProvider, GrokAccountQuotaConfidence,
    GrokAccountQuotaScope, GrokAccountQuotaSource, GrokAccountQuotaWindow,
    GrokAccountWorkerCoordinator, GrokAccountWorkerError, GrokAccountWorkerExecutor,
    GrokAccountWorkerJob, GrokAccountWorkerKind, GrokAccountWorkerResult,
    deterministic_refresh_due_at,
};
use rusqlite::Connection;

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 1_800_000_000_000;
const CLAIM_LEASE_MS: i64 = 5_000;
const ENDPOINT: &str = "grok-worker-endpoint";
static TEMPORARY_DATABASE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn refresh_jitter_is_stable_and_between_five_and_eight_minutes() -> TestResult {
    let expiry = NOW_MS + 3_600_000;
    let first = deterministic_refresh_due_at("account-a", expiry)?;
    let repeated = deterministic_refresh_due_at("account-a", expiry)?;
    let second = deterministic_refresh_due_at("account-b", expiry)?;
    assert_eq!(first, repeated);
    assert!((expiry - 8 * 60_000..=expiry - 5 * 60_000).contains(&first));
    assert!((expiry - 8 * 60_000..=expiry - 5 * 60_000).contains(&second));
    assert_ne!(first, second);
    Ok(())
}

#[test]
fn durable_claim_is_cross_kind_singleflight_and_reclaimed_only_after_expiry() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = Arc::new(open_store(database.path())?);
    store.import_batch("claim", &[account("claim", NOW_MS, NOW_MS)?], NOW_MS)?;

    let first =
        store.claim_due_worker_jobs(GrokAccountWorkerKind::Refresh, NOW_MS, 1, CLAIM_LEASE_MS)?;
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].credential_bytes(), b"credential-claim");
    let independent = Connection::open(database.path())?;
    assert!(
        independent
            .execute(
                "UPDATE grok_accounts SET worker_claim_id = NULL WHERE id = ?1",
                [first[0].account_id()],
            )
            .is_err(),
        "durable claim fields must remain all-null or all-present"
    );
    assert!(
        store
            .claim_due_worker_jobs(GrokAccountWorkerKind::Refresh, NOW_MS, 1, CLAIM_LEASE_MS,)?
            .is_empty()
    );
    assert!(
        store
            .claim_due_worker_jobs(GrokAccountWorkerKind::Quota, NOW_MS, 1, CLAIM_LEASE_MS,)?
            .is_empty()
    );
    drop(first);
    drop(store);

    let restarted = open_store(database.path())?;
    assert!(
        restarted
            .claim_due_worker_jobs(
                GrokAccountWorkerKind::Refresh,
                NOW_MS + CLAIM_LEASE_MS - 1,
                1,
                CLAIM_LEASE_MS,
            )?
            .is_empty()
    );
    let reclaimed = restarted.claim_due_worker_jobs(
        GrokAccountWorkerKind::Refresh,
        NOW_MS + CLAIM_LEASE_MS,
        1,
        CLAIM_LEASE_MS,
    )?;
    assert_eq!(reclaimed.len(), 1);
    Ok(())
}

#[test]
fn provider_scoped_refresh_never_claims_a_sibling_channel() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = Arc::new(open_store(database.path())?);
    store.import_batch(
        "provider-scope",
        &[
            account_for_provider("build-scope", GrokAccountProvider::Build, NOW_MS, NOW_MS)?,
            account_for_provider(
                "console-scope",
                GrokAccountProvider::Console,
                NOW_MS,
                NOW_MS,
            )?,
        ],
        NOW_MS,
    )?;
    let executor = RecordingExecutor::default();
    let coordinator = GrokAccountWorkerCoordinator::try_new(2, CLAIM_LEASE_MS)?;
    let build = coordinator.run_once_for_provider(
        &store,
        GrokAccountWorkerKind::Refresh,
        GrokAccountProvider::Build,
        NOW_MS,
        &executor,
    )?;
    assert_eq!(build.claimed, 1);
    assert_eq!(executor.providers()?, [GrokAccountProvider::Build]);

    let console = coordinator.run_once_for_provider(
        &store,
        GrokAccountWorkerKind::Refresh,
        GrokAccountProvider::Console,
        NOW_MS,
        &executor,
    )?;
    assert_eq!(console.claimed, 1);
    assert_eq!(
        executor.providers()?,
        [GrokAccountProvider::Build, GrokAccountProvider::Console]
    );
    Ok(())
}

#[test]
fn refresh_success_commits_next_revision_and_redacted_replacement() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = Arc::new(open_store(database.path())?);
    store.import_batch(
        "refresh",
        &[account("refresh", NOW_MS, NOW_MS + 10_000)?],
        NOW_MS,
    )?;
    let expires_at_ms = NOW_MS + 3_600_000;
    let coordinator = GrokAccountWorkerCoordinator::try_new(1, CLAIM_LEASE_MS)?;
    let summary = coordinator.run_once(
        &store,
        GrokAccountWorkerKind::Refresh,
        NOW_MS,
        &OneResult::new(GrokAccountWorkerResult::Refreshed {
            credential: GrokAccountCredential::try_from_bytes(b"replacement-credential")?,
            expires_at_ms,
        }),
    )?;
    assert_eq!(summary.claimed, 1);
    assert_eq!(summary.succeeded, 1);
    let metadata = store.list_accounts()?;
    assert_eq!(metadata[0].revision, 1);
    assert_eq!(
        metadata[0].refresh_due_at_ms,
        Some(deterministic_refresh_due_at(
            &metadata[0].id,
            expires_at_ms
        )?)
    );
    let opened = store.open_credential(&metadata[0].id)?;
    assert_eq!(opened.as_bytes(), b"replacement-credential");
    assert!(!format!("{summary:?}").contains("replacement-credential"));
    Ok(())
}

#[test]
fn stale_refresh_worker_cannot_overwrite_a_newer_revision() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = Arc::new(open_store(database.path())?);
    store.import_batch(
        "stale",
        &[account("stale", NOW_MS, NOW_MS + 10_000)?],
        NOW_MS,
    )?;
    let mutating_executor = RevisionMutatingExecutor {
        database_path: database.path().to_path_buf(),
    };
    let result = GrokAccountWorkerCoordinator::try_new(1, CLAIM_LEASE_MS)?.run_once(
        &store,
        GrokAccountWorkerKind::Refresh,
        NOW_MS,
        &mutating_executor,
    );
    assert_eq!(result, Err(GrokAccountWorkerError::StaleClaim));
    let metadata = store.list_accounts()?;
    assert_eq!(metadata[0].revision, 1);
    assert_eq!(
        store.open_credential(&metadata[0].id)?.as_bytes(),
        b"credential-stale"
    );
    Ok(())
}

#[test]
fn transient_backoff_and_reauth_are_durable_and_restart_safe() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = Arc::new(open_store(database.path())?);
    store.import_batch("failure", &[account("failure", NOW_MS, NOW_MS)?], NOW_MS)?;
    let coordinator = GrokAccountWorkerCoordinator::try_new(1, CLAIM_LEASE_MS)?;
    let transient = coordinator.run_once(
        &store,
        GrokAccountWorkerKind::Refresh,
        NOW_MS,
        &StaticExecutor(GrokAccountWorkerResultKind::Transient),
    )?;
    assert_eq!(transient.backed_off, 1);
    let next_due = store.list_accounts()?[0]
        .refresh_due_at_ms
        .ok_or("transient failure did not persist a retry deadline")?;
    assert!(next_due > NOW_MS);
    assert!(
        store
            .claim_due_worker_jobs(
                GrokAccountWorkerKind::Refresh,
                next_due - 1,
                1,
                CLAIM_LEASE_MS,
            )?
            .is_empty()
    );

    let reauth = coordinator.run_once(
        &store,
        GrokAccountWorkerKind::Refresh,
        next_due,
        &StaticExecutor(GrokAccountWorkerResultKind::Reauth),
    )?;
    assert_eq!(reauth.reauth_required, 1);
    assert_eq!(
        store.list_accounts()?[0].auth_status,
        GrokAccountAuthStatus::ReauthRequired
    );
    drop(store);
    let restarted = open_store(database.path())?;
    assert!(
        restarted
            .claim_due_worker_jobs(
                GrokAccountWorkerKind::Refresh,
                next_due + 10_000_000,
                1,
                CLAIM_LEASE_MS,
            )?
            .is_empty()
    );
    Ok(())
}

#[test]
fn quota_sync_is_atomic_and_restores_exact_runtime_targets_after_restart() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = Arc::new(open_store(database.path())?);
    store.import_batch(
        "quota",
        &[account("quota", NOW_MS + 10_000, NOW_MS)?],
        NOW_MS,
    )?;
    let windows = vec![
        GrokAccountQuotaWindow::try_new(
            GrokAccountQuotaScope::Account,
            "requests",
            Some(10),
            Some(0),
            Some(NOW_MS + 100),
            GrokAccountQuotaSource::Billing,
            GrokAccountQuotaConfidence::Authoritative,
        )?,
        GrokAccountQuotaWindow::try_new(
            GrokAccountQuotaScope::Model("grok-model".to_owned()),
            "requests",
            Some(20),
            Some(0),
            Some(NOW_MS + 200),
            GrokAccountQuotaSource::Rest,
            GrokAccountQuotaConfidence::Observed,
        )?,
    ];
    let summary = GrokAccountWorkerCoordinator::try_new(1, CLAIM_LEASE_MS)?.run_once(
        &store,
        GrokAccountWorkerKind::Quota,
        NOW_MS,
        &OneResult::new(GrokAccountWorkerResult::QuotaSynchronized {
            windows,
            next_due_at_ms: NOW_MS + 60_000,
        }),
    )?;
    assert_eq!(summary.succeeded, 1);
    let account_id = store.list_accounts()?[0].id.clone();
    drop(store);

    let restarted = open_store(database.path())?;
    let compilation = restarted.compile_native_runtime(&bindings()?, NOW_MS)?;
    let clock = Arc::new(FixedClock::new(NOW_MS));
    let quota = RuntimeQuotaRegistry::with_clock(clock);
    compilation.seed_runtime_quota(&quota)?;
    let endpoint = EndpointId::try_new(ENDPOINT)?;
    let credential = CredentialId::try_new(account_id)?;
    assert!(!quota.endpoint_credential_is_available(&endpoint, &credential));
    assert!(!quota.endpoint_credential_model_is_available(&endpoint, &credential, "grok-model"));
    assert!(quota.endpoint_credential_model_is_available(&endpoint, &credential, "other-model"));
    assert!(
        quota
            .snapshot(&RuntimeQuotaTarget::endpoint_credential(
                endpoint, credential,
            ))?
            .is_some()
    );
    Ok(())
}

#[test]
fn coordinator_never_exceeds_configured_parallelism() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = Arc::new(open_store(database.path())?);
    let entries = (0..10)
        .map(|index| account(&format!("parallel-{index}"), NOW_MS, NOW_MS + 10_000))
        .collect::<Result<Vec<_>, _>>()?;
    store.import_batch("parallel", &entries, NOW_MS)?;
    let executor = ParallelismExecutor::default();
    let summary = GrokAccountWorkerCoordinator::try_new(3, CLAIM_LEASE_MS)?.run_once(
        &store,
        GrokAccountWorkerKind::Refresh,
        NOW_MS,
        &executor,
    )?;
    assert_eq!(summary.claimed, 3);
    assert_eq!(summary.backed_off, 3);
    assert!(executor.maximum.load(Ordering::Acquire) <= 3);
    assert!(executor.maximum.load(Ordering::Acquire) >= 2);
    Ok(())
}

fn account(
    identity: &str,
    refresh_due_at_ms: i64,
    quota_sync_due_at_ms: i64,
) -> Result<GrokAccountImport, Box<dyn Error>> {
    account_for_provider(
        identity,
        GrokAccountProvider::Build,
        refresh_due_at_ms,
        quota_sync_due_at_ms,
    )
}

fn account_for_provider(
    identity: &str,
    provider: GrokAccountProvider,
    refresh_due_at_ms: i64,
    quota_sync_due_at_ms: i64,
) -> Result<GrokAccountImport, Box<dyn Error>> {
    Ok(GrokAccountImport {
        provider,
        identity: GrokAccountIdentity::try_from_bytes(identity)?,
        credential: GrokAccountCredential::try_from_bytes(format!("credential-{identity}"))?,
        auth_status: GrokAccountAuthStatus::Active,
        enabled: true,
        priority: 0,
        weight: 1,
        max_concurrency: 2,
        refresh_due_at_ms: Some(refresh_due_at_ms),
        quota_sync_due_at_ms: Some(quota_sync_due_at_ms),
        cooldown_until_ms: None,
    })
}

#[derive(Default)]
struct RecordingExecutor(Mutex<Vec<GrokAccountProvider>>);

impl RecordingExecutor {
    fn providers(&self) -> Result<Vec<GrokAccountProvider>, Box<dyn Error>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| "recording executor poisoned")?
            .clone())
    }
}

impl GrokAccountWorkerExecutor for RecordingExecutor {
    fn execute(&self, job: &GrokAccountWorkerJob) -> GrokAccountWorkerResult {
        if let Ok(mut providers) = self.0.lock() {
            providers.push(job.provider());
        }
        GrokAccountWorkerResult::TransientFailure
    }
}

fn bindings() -> Result<Vec<GrokAccountEndpointBinding>, Box<dyn Error>> {
    Ok(vec![GrokAccountEndpointBinding::new(
        GrokAccountProvider::Build,
        EndpointId::try_new(ENDPOINT)?,
    )])
}

struct OneResult(Mutex<Option<GrokAccountWorkerResult>>);

impl OneResult {
    const fn new(result: GrokAccountWorkerResult) -> Self {
        Self(Mutex::new(Some(result)))
    }
}

impl GrokAccountWorkerExecutor for OneResult {
    fn execute(&self, _job: &GrokAccountWorkerJob) -> GrokAccountWorkerResult {
        let Ok(mut result) = self.0.lock() else {
            return GrokAccountWorkerResult::TransientFailure;
        };
        result
            .take()
            .unwrap_or(GrokAccountWorkerResult::TransientFailure)
    }
}

enum GrokAccountWorkerResultKind {
    Transient,
    Reauth,
}

struct StaticExecutor(GrokAccountWorkerResultKind);

impl GrokAccountWorkerExecutor for StaticExecutor {
    fn execute(&self, _job: &GrokAccountWorkerJob) -> GrokAccountWorkerResult {
        match self.0 {
            GrokAccountWorkerResultKind::Transient => GrokAccountWorkerResult::TransientFailure,
            GrokAccountWorkerResultKind::Reauth => GrokAccountWorkerResult::ReauthRequired,
        }
    }
}

struct RevisionMutatingExecutor {
    database_path: PathBuf,
}

impl GrokAccountWorkerExecutor for RevisionMutatingExecutor {
    fn execute(&self, job: &GrokAccountWorkerJob) -> GrokAccountWorkerResult {
        let Ok(connection) = Connection::open(&self.database_path) else {
            return GrokAccountWorkerResult::TransientFailure;
        };
        if connection
            .execute(
                "UPDATE grok_accounts SET revision = revision + 1 WHERE id = ?1",
                [job.account_id()],
            )
            .is_err()
        {
            return GrokAccountWorkerResult::TransientFailure;
        }
        let Ok(credential) = GrokAccountCredential::try_from_bytes(b"stale-replacement") else {
            return GrokAccountWorkerResult::TransientFailure;
        };
        GrokAccountWorkerResult::Refreshed {
            credential,
            expires_at_ms: NOW_MS + 3_600_000,
        }
    }
}

#[derive(Default)]
struct ParallelismExecutor {
    current: AtomicUsize,
    maximum: AtomicUsize,
}

impl GrokAccountWorkerExecutor for ParallelismExecutor {
    fn execute(&self, _job: &GrokAccountWorkerJob) -> GrokAccountWorkerResult {
        let current = self.current.fetch_add(1, Ordering::AcqRel) + 1;
        self.maximum.fetch_max(current, Ordering::AcqRel);
        thread::sleep(Duration::from_millis(20));
        self.current.fetch_sub(1, Ordering::AcqRel);
        GrokAccountWorkerResult::TransientFailure
    }
}

fn open_store(path: &Path) -> Result<GrokAccountPoolStore, Box<dyn Error>> {
    Ok(GrokAccountPoolStore::try_new(
        Connection::open(path)?,
        secret_store()?,
    )?)
}

fn secret_store() -> Result<SecretStore, Box<dyn Error>> {
    let key_version = KeyVersion::try_new(1)?;
    Ok(SecretStore::new(MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0xA5; 32])?)],
    )?))
}

#[derive(Debug)]
struct FixedClock(AtomicI64);

impl FixedClock {
    const fn new(now_ms: i64) -> Self {
        Self(AtomicI64::new(now_ms))
    }
}

impl RuntimeHealthClock for FixedClock {
    fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
        Ok(self.0.load(Ordering::Acquire))
    }
}

struct TemporaryDatabase(PathBuf);

impl TemporaryDatabase {
    fn new() -> Result<Self, Box<dyn Error>> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        for _ in 0..64 {
            let sequence = TEMPORARY_DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "cpa-rust-gateway-p12-10d-{timestamp}-{}-{sequence}.sqlite3",
                std::process::id()
            ));
            if !path.exists() {
                return Ok(Self(path));
            }
        }
        Err("could not allocate isolated P12-10D database".into())
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
