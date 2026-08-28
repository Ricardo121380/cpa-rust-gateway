//! P12-10I-01 native Grok reauthentication coordinator evidence.

#![deny(unsafe_code)]

use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
use provider_grok::{
    GrokAccountAuthStatus, GrokAccountCredential, GrokAccountIdentity, GrokAccountImport,
    GrokAccountPoolStore, GrokAccountProvider, GrokReauthAttempt, GrokReauthCoordinator,
    GrokReauthExecutor, GrokReauthResult, GrokReauthStrategy,
};
use rusqlite::Connection;

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 1_800_000_000_000;
const CLAIM_LEASE_MS: i64 = 5_000;
static TEMPORARY_DATABASE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn auto_fallback_is_serial_and_replaces_each_account_atomically() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = Arc::new(open_store(database.path())?);
    store.import_batch(
        "reauth",
        &[
            reauth_account(b"identity-a", b"old-a")?,
            reauth_account(b"identity-b", b"old-b")?,
        ],
        NOW_MS,
    )?;

    let executor = FallbackExecutor::default();
    let summary = GrokReauthCoordinator::try_new(CLAIM_LEASE_MS)?.run_once(
        &store,
        GrokReauthStrategy::Auto,
        2,
        NOW_MS,
        &executor,
    )?;
    assert_eq!(summary.claimed, 2);
    assert_eq!(summary.succeeded, 2);
    assert_eq!(summary.refreshed, 0);
    assert_eq!(summary.device_code, 0);
    assert_eq!(summary.browser_sso, 2);
    assert_eq!(executor.maximum_in_flight.load(Ordering::Acquire), 1);

    let accounts = store.list_accounts()?;
    assert!(accounts.iter().all(|account| {
        account.auth_status == GrokAccountAuthStatus::Active && account.revision == 1
    }));
    for account in accounts {
        let opened = store.open_credential(&account.id)?;
        assert!(opened.as_bytes().starts_with(b"replacement-"));
    }
    assert!(
        store
            .claim_due_reauth_job(NOW_MS, CLAIM_LEASE_MS)?
            .is_none()
    );
    assert!(!format!("{summary:?}").contains("replacement-"));
    Ok(())
}

#[test]
fn transient_failure_is_backed_off_and_interactive_failure_stays_blocked() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = Arc::new(open_store(database.path())?);
    store.import_batch(
        "transient",
        &[reauth_account(b"identity-transient", b"old")?],
        NOW_MS,
    )?;

    let transient = FixedExecutor {
        result: Mutex::new(Some(GrokReauthResult::TransientFailure)),
    };
    let coordinator = GrokReauthCoordinator::try_new(CLAIM_LEASE_MS)?;
    let summary = coordinator.run_once(
        &store,
        GrokReauthStrategy::RefreshOnly,
        1,
        NOW_MS,
        &transient,
    )?;
    assert_eq!(summary.backed_off, 1);
    assert_eq!(summary.interactive_required, 0);
    assert!(
        store
            .claim_due_reauth_job(NOW_MS, CLAIM_LEASE_MS)?
            .is_none()
    );

    let database = TemporaryDatabase::new()?;
    let store = Arc::new(open_store(database.path())?);
    store.import_batch(
        "manual",
        &[reauth_account(b"identity-manual", b"old")?],
        NOW_MS,
    )?;
    let manual = FixedExecutor {
        result: Mutex::new(Some(GrokReauthResult::NeedsInteractive)),
    };
    let summary =
        coordinator.run_once(&store, GrokReauthStrategy::RefreshOnly, 1, NOW_MS, &manual)?;
    assert_eq!(summary.interactive_required, 1);
    assert_eq!(
        store.list_accounts()?[0].auth_status,
        GrokAccountAuthStatus::ReauthRequired
    );
    assert!(
        store
            .claim_due_reauth_job(NOW_MS, CLAIM_LEASE_MS)?
            .is_none()
    );
    let account_id = store.list_accounts()?[0].id.clone();
    store.requeue_reauth(&account_id, NOW_MS)?;
    assert!(
        store
            .claim_due_reauth_job(NOW_MS, CLAIM_LEASE_MS)?
            .is_some()
    );
    Ok(())
}

#[test]
fn reauth_claim_survives_restart_until_lease_expiry() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = open_store(database.path())?;
    store.import_batch(
        "claim",
        &[reauth_account(b"identity-claim", b"old")?],
        NOW_MS,
    )?;
    let first = store
        .claim_due_reauth_job(NOW_MS, CLAIM_LEASE_MS)?
        .ok_or("missing reauth claim")?;
    drop(first);
    drop(store);

    let restarted = open_store(database.path())?;
    assert!(
        restarted
            .claim_due_reauth_job(NOW_MS + CLAIM_LEASE_MS - 1, CLAIM_LEASE_MS)?
            .is_none()
    );
    assert!(
        restarted
            .claim_due_reauth_job(NOW_MS + CLAIM_LEASE_MS, CLAIM_LEASE_MS)?
            .is_some()
    );
    Ok(())
}

#[derive(Default)]
struct FallbackExecutor {
    maximum_in_flight: AtomicUsize,
    in_flight: AtomicUsize,
    attempts: Mutex<Vec<(String, GrokReauthAttempt)>>,
}

impl GrokReauthExecutor for FallbackExecutor {
    fn execute(
        &self,
        job: &provider_grok::GrokReauthJob,
        attempt: GrokReauthAttempt,
    ) -> GrokReauthResult {
        let current = self.in_flight.fetch_add(1, Ordering::AcqRel) + 1;
        self.maximum_in_flight.fetch_max(current, Ordering::AcqRel);
        match self.attempts.lock() {
            Ok(mut attempts) => attempts.push((job.account_id().to_owned(), attempt)),
            Err(poisoned) => poisoned
                .into_inner()
                .push((job.account_id().to_owned(), attempt)),
        }
        let result = match attempt {
            GrokReauthAttempt::Refresh | GrokReauthAttempt::DeviceCode => {
                GrokReauthResult::NeedsInteractive
            }
            GrokReauthAttempt::BrowserSso => {
                match GrokAccountCredential::try_from_bytes(
                    format!("replacement-{}", job.account_id()).as_bytes(),
                ) {
                    Ok(credential) => GrokReauthResult::Reauthenticated {
                        credential,
                        acquisition: attempt,
                    },
                    Err(_) => GrokReauthResult::Denied,
                }
            }
        };
        self.in_flight.fetch_sub(1, Ordering::AcqRel);
        result
    }
}

#[derive(Default)]
struct FixedExecutor {
    result: Mutex<Option<GrokReauthResult>>,
}

impl GrokReauthExecutor for FixedExecutor {
    fn execute(
        &self,
        _job: &provider_grok::GrokReauthJob,
        _attempt: GrokReauthAttempt,
    ) -> GrokReauthResult {
        let mut result = match self.result.lock() {
            Ok(result) => result,
            Err(poisoned) => poisoned.into_inner(),
        };
        match result.take() {
            Some(result) => result,
            None => GrokReauthResult::Denied,
        }
    }
}

fn reauth_account(identity: &[u8], credential: &[u8]) -> Result<GrokAccountImport, Box<dyn Error>> {
    Ok(GrokAccountImport {
        provider: GrokAccountProvider::Console,
        identity: GrokAccountIdentity::try_from_bytes(identity)?,
        credential: GrokAccountCredential::try_from_bytes(credential)?,
        auth_status: GrokAccountAuthStatus::ReauthRequired,
        enabled: true,
        priority: 1,
        weight: 1,
        max_concurrency: 1,
        refresh_due_at_ms: None,
        quota_sync_due_at_ms: None,
        cooldown_until_ms: None,
    })
}

fn open_store(path: &Path) -> Result<GrokAccountPoolStore, Box<dyn Error>> {
    Ok(GrokAccountPoolStore::try_new(
        Connection::open(path)?,
        SecretStore::new(MasterKeyRing::try_new(
            KeyVersion::try_new(1)?,
            [(
                KeyVersion::try_new(1)?,
                MasterKey::try_from_bytes([0xA5; 32])?,
            )],
        )?),
    )?)
}

struct TemporaryDatabase(PathBuf);

impl TemporaryDatabase {
    fn new() -> Result<Self, Box<dyn Error>> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        for _ in 0..64 {
            let sequence = TEMPORARY_DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "cpa-rust-gateway-p12-10i-{timestamp}-{}-{sequence}.sqlite3",
                std::process::id()
            ));
            if !path.exists() {
                return Ok(Self(path));
            }
        }
        Err("unable to allocate temporary database".into())
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
