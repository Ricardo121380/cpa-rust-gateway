//! P12-10C native Grok account composition with the existing scheduler, Health, and Quota.

#![deny(unsafe_code)]

use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_catalog::{CapabilitySet, CatalogModelState};
use gateway_core::{
    CredentialId, EndpointId, PublicModelId, RouteCandidateId, RouteId, UpstreamId,
};
use gateway_router::{
    QuotaConfidence, QuotaSnapshot, QuotaSource, QuotaWindow, RouteCredentialScheduler,
    RouteSnapshot, RouteSnapshotInput, RuntimeHealthAccountRecoveryResult, RuntimeHealthClock,
    RuntimeHealthClockError, RuntimeHealthKey, RuntimeHealthRegistry, RuntimeQuotaRegistry,
    RuntimeQuotaTarget, SnapshotCatalogAdmission, SnapshotPublicModel, SnapshotRoute,
    SnapshotRouteCandidate, SnapshotRouteCandidateInput, SnapshotRoutePolicy,
    SnapshotTransformMode, SnapshotVersion,
};
use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
use provider_grok::{
    GrokAccountAuthStatus, GrokAccountCredential, GrokAccountEndpointBinding, GrokAccountIdentity,
    GrokAccountImport, GrokAccountPoolStore, GrokAccountProvider, GrokNativeAccountCompileError,
};
use rusqlite::Connection;

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 1_800_000_000_000;
const ENDPOINT: &str = "grok-build-endpoint";
const MODEL: &str = "grok-upstream-model";
const FRESH_BUILD_EXPIRY: &str = "2027-01-15T09:00:00Z";
const EXPIRED_BUILD_EXPIRY: &str = "2027-01-15T07:59:59Z";
static TEMPORARY_DATABASE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn native_weight_uses_the_existing_endpoint_pool() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = open_store(database.path())?;
    store.import_batch(
        "weighted",
        &[
            account(
                "weighted-three",
                20,
                3,
                8,
                GrokAccountAuthStatus::Active,
                true,
                None,
            )?,
            account(
                "weighted-one",
                20,
                1,
                8,
                GrokAccountAuthStatus::Active,
                true,
                None,
            )?,
        ],
        NOW_MS,
    )?;
    let metadata = store.list_accounts()?;
    let compilation = store.compile_native_runtime(&bindings()?, NOW_MS)?;
    assert_eq!(compilation.account_count(), 2);
    let pool = compilation
        .credential_pools()
        .pool(&EndpointId::try_new(ENDPOINT)?)
        .cloned()
        .ok_or("compiled native Endpoint pool is missing")?;

    let mut counts = BTreeMap::new();
    for _ in 0..4 {
        let lease = pool.try_lease().ok_or("weighted lease was unavailable")?;
        assert_eq!(lease.credential_kind(), "grok_build_oauth");
        assert_eq!(
            compilation.provider_for_credential(lease.credential_id()),
            Some(GrokAccountProvider::Build)
        );
        *counts
            .entry(lease.credential_id().as_str().to_owned())
            .or_insert(0_usize) += 1;
    }
    let weighted_three = metadata
        .iter()
        .find(|account| account.weight == 3)
        .ok_or("weight-three account missing")?;
    let weighted_one = metadata
        .iter()
        .find(|account| account.weight == 1)
        .ok_or("weight-one account missing")?;
    assert_eq!(counts.get(&weighted_three.id), Some(&3));
    assert_eq!(counts.get(&weighted_one.id), Some(&1));
    Ok(())
}

#[test]
fn duplicate_bindings_and_oversized_existing_schedules_fail_before_publication() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = open_store(database.path())?;
    store.import_batch(
        "bounded",
        &[account(
            "oversized-weight",
            0,
            1_025,
            1,
            GrokAccountAuthStatus::Active,
            true,
            None,
        )?],
        NOW_MS,
    )?;
    let endpoint = EndpointId::try_new(ENDPOINT)?;
    let duplicate = vec![
        GrokAccountEndpointBinding::new(GrokAccountProvider::Build, endpoint.clone()),
        GrokAccountEndpointBinding::new(
            GrokAccountProvider::Build,
            EndpointId::try_new("duplicate-provider-endpoint")?,
        ),
    ];
    assert!(matches!(
        store.compile_native_runtime(&duplicate, NOW_MS),
        Err(GrokNativeAccountCompileError::DuplicateBinding)
    ));
    assert!(matches!(
        store.compile_native_runtime(
            &[GrokAccountEndpointBinding::new(
                GrokAccountProvider::Build,
                endpoint,
            )],
            NOW_MS,
        ),
        Err(GrokNativeAccountCompileError::Pool(_))
    ));
    Ok(())
}

#[test]
fn native_priority_and_concurrency_use_the_existing_endpoint_pool() -> TestResult {
    let priority_database = TemporaryDatabase::new()?;
    let priority_store = open_store(priority_database.path())?;
    priority_store.import_batch(
        "priority",
        &[
            account(
                "preferred",
                100,
                1,
                1,
                GrokAccountAuthStatus::Active,
                true,
                None,
            )?,
            account(
                "fallback",
                0,
                1,
                1,
                GrokAccountAuthStatus::Active,
                true,
                None,
            )?,
        ],
        NOW_MS + 1,
    )?;
    let metadata = priority_store.list_accounts()?;
    let compilation = priority_store.compile_native_runtime(&bindings()?, NOW_MS + 1)?;
    let pool = compilation
        .credential_pools()
        .pool(&EndpointId::try_new(ENDPOINT)?)
        .cloned()
        .ok_or("recompiled native Endpoint pool is missing")?;
    let preferred_id = metadata
        .iter()
        .find(|account| account.priority == 100)
        .map(|account| account.id.as_str())
        .ok_or("preferred account missing")?;
    let fallback_id = metadata
        .iter()
        .find(|account| account.priority == 0)
        .map(|account| account.id.as_str())
        .ok_or("fallback account missing")?;
    let preferred = pool.try_lease().ok_or("preferred lease missing")?;
    assert_eq!(preferred.credential_id().as_str(), preferred_id);
    let fallback = pool.try_lease().ok_or("fallback lease missing")?;
    assert_eq!(fallback.credential_id().as_str(), fallback_id);
    assert!(pool.active_lease_count(preferred.credential_id()).is_some());
    drop(preferred);
    drop(fallback);
    Ok(())
}

#[test]
fn expired_preferred_build_does_not_obscure_fresh_sibling_after_restart() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let store = open_store(database.path())?;
    store.import_batch(
        "build-expiry",
        &[
            build_account_with_expiry("expired-preferred", 100, EXPIRED_BUILD_EXPIRY)?,
            build_account_with_expiry("fresh-sibling", 0, FRESH_BUILD_EXPIRY)?,
        ],
        NOW_MS,
    )?;
    let metadata = store.list_accounts()?;
    let expired_id = CredentialId::try_new(
        metadata
            .iter()
            .find(|account| account.priority == 100)
            .ok_or("expired preferred account missing")?
            .id
            .clone(),
    )?;
    let fresh_id = CredentialId::try_new(
        metadata
            .iter()
            .find(|account| account.priority == 0)
            .ok_or("fresh sibling account missing")?
            .id
            .clone(),
    )?;

    let compilation = store.compile_native_runtime(&bindings()?, NOW_MS)?;
    assert_eq!(compilation.account_count(), 2);
    assert_eq!(
        compilation.provider_for_credential(&expired_id),
        Some(GrokAccountProvider::Build)
    );
    assert_fresh_build_selected(&compilation, &expired_id, &fresh_id)?;
    drop(store);

    let restarted = open_store(database.path())?;
    let compilation = restarted.compile_native_runtime(&bindings()?, NOW_MS)?;
    assert_eq!(compilation.account_count(), 2);
    assert_fresh_build_selected(&compilation, &expired_id, &fresh_id)?;
    Ok(())
}

#[test]
fn persisted_auth_and_cooldown_bootstrap_shared_health_after_restart() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let account_ids = {
        let store = open_store(database.path())?;
        store.import_batch(
            "health",
            &[
                account(
                    "cooling",
                    10,
                    1,
                    2,
                    GrokAccountAuthStatus::Active,
                    true,
                    Some(NOW_MS + 100),
                )?,
                account(
                    "reauth",
                    20,
                    1,
                    2,
                    GrokAccountAuthStatus::ReauthRequired,
                    true,
                    None,
                )?,
                account(
                    "disabled",
                    30,
                    1,
                    2,
                    GrokAccountAuthStatus::Disabled,
                    true,
                    None,
                )?,
            ],
            NOW_MS,
        )?;
        store
            .list_accounts()?
            .into_iter()
            .map(|account| (account.auth_status, account.id))
            .collect::<BTreeMap<_, _>>()
    };

    let store = open_store(database.path())?;
    let compilation = store.compile_native_runtime(&bindings()?, NOW_MS)?;
    assert_eq!(compilation.account_count(), 2);
    let clock = Arc::new(FixedClock::new(NOW_MS));
    let health = RuntimeHealthRegistry::with_clock(clock.clone());
    compilation.seed_runtime_health(&health)?;
    let scheduler = build_scheduler(compilation.credential_pools())?;
    assert!(
        scheduler
            .select_runtime_eligible_and_lease(&route_id()?, &health)
            .is_err()
    );

    let reauth_id = CredentialId::try_new(
        account_ids
            .get(&GrokAccountAuthStatus::ReauthRequired)
            .ok_or("reauth account missing")?
            .clone(),
    )?;
    let endpoint = EndpointId::try_new(ENDPOINT)?;
    let probe = health
        .begin_account_recovery(&endpoint, &reauth_id, NOW_MS + 50)?
        .ok_or("reauth account did not issue a recovery ticket")?;
    health.complete_account_recovery(probe, RuntimeHealthAccountRecoveryResult::Allowed)?;
    let recovered = scheduler.select_runtime_eligible_and_lease(&route_id()?, &health)?;
    assert_eq!(recovered.lease().credential_id(), &reauth_id);
    drop(recovered);

    clock.set(NOW_MS + 100);
    assert!(
        health.endpoint_credential_is_available(
            &endpoint,
            &CredentialId::try_new(
                account_ids
                    .get(&GrokAccountAuthStatus::Active)
                    .ok_or("cooling account missing")?
                    .clone(),
            )?
        )
    );

    let restarted_store = open_store(database.path())?;
    let restarted = restarted_store.compile_native_runtime(&bindings()?, NOW_MS)?;
    let restarted_health = RuntimeHealthRegistry::with_clock(Arc::new(FixedClock::new(NOW_MS)));
    restarted.seed_runtime_health(&restarted_health)?;
    let restarted_scheduler = build_scheduler(restarted.credential_pools())?;
    assert!(
        restarted_scheduler
            .select_runtime_eligible_and_lease(&route_id()?, &restarted_health)
            .is_err()
    );
    Ok(())
}

#[test]
fn exact_model_health_block_preserves_a_sibling_account() -> TestResult {
    let (_database, scheduler, preferred_id, sibling_id) = runtime_scheduler_fixture()?;
    let clock = Arc::new(FixedClock::new(NOW_MS));
    let health = RuntimeHealthRegistry::with_clock(clock.clone());
    let quota = RuntimeQuotaRegistry::with_clock(clock);
    health.open_circuit_until(
        RuntimeHealthKey::endpoint_credential_model(
            EndpointId::try_new(ENDPOINT)?,
            preferred_id,
            MODEL,
        ),
        NOW_MS + 10,
    )?;
    let fallback = scheduler.select_eligible_and_lease_with_runtime_health_quota_and_binding(
        &route_id()?,
        &health,
        &quota,
        |_| true,
        |_, _| true,
    )?;
    assert_eq!(fallback.lease().credential_id(), &sibling_id);
    Ok(())
}

#[test]
fn exact_model_quota_preserves_a_sibling_and_requires_controlled_recovery() -> TestResult {
    let (_database, scheduler, preferred_id, sibling_id) = runtime_scheduler_fixture()?;
    let clock = Arc::new(FixedClock::new(NOW_MS));
    let health = RuntimeHealthRegistry::with_clock(clock.clone());
    let quota = RuntimeQuotaRegistry::with_clock(clock.clone());
    let target = RuntimeQuotaTarget::endpoint_credential_model(
        EndpointId::try_new(ENDPOINT)?,
        preferred_id.clone(),
        MODEL,
    )?;
    quota.record_snapshot(QuotaSnapshot::try_new(
        target.clone(),
        vec![QuotaWindow::try_new(
            "requests",
            Some(10),
            Some(0),
            Some(NOW_MS + 10),
        )?],
        QuotaSource::Billing,
        QuotaConfidence::Authoritative,
        NOW_MS,
    )?)?;
    let quota_fallback = scheduler
        .select_eligible_and_lease_with_runtime_health_quota_and_binding(
            &route_id()?,
            &health,
            &quota,
            |_| true,
            |_, _| true,
        )?;
    assert_eq!(quota_fallback.lease().credential_id(), &sibling_id);
    drop(quota_fallback);

    clock.set(NOW_MS + 10);
    let ticket = quota
        .begin_recovery_probe(&target, NOW_MS + 20)?
        .ok_or("quota target did not issue recovery ticket")?;
    quota.complete_recovery_probe(
        ticket,
        QuotaSnapshot::try_new(
            target,
            vec![QuotaWindow::try_new("requests", Some(10), Some(10), None)?],
            QuotaSource::Billing,
            QuotaConfidence::Authoritative,
            NOW_MS + 10,
        )?,
    )?;
    let recovered = scheduler.select_eligible_and_lease_with_runtime_health_quota_and_binding(
        &route_id()?,
        &health,
        &quota,
        |_| true,
        |_, _| true,
    )?;
    assert_eq!(recovered.lease().credential_id(), &preferred_id);
    Ok(())
}

fn runtime_scheduler_fixture() -> Result<
    (
        TemporaryDatabase,
        RouteCredentialScheduler,
        CredentialId,
        CredentialId,
    ),
    Box<dyn Error>,
> {
    let database = TemporaryDatabase::new()?;
    let store = open_store(database.path())?;
    store.import_batch(
        "runtime-state",
        &[
            account(
                "preferred",
                100,
                1,
                2,
                GrokAccountAuthStatus::Active,
                true,
                None,
            )?,
            account(
                "sibling",
                0,
                1,
                2,
                GrokAccountAuthStatus::Active,
                true,
                None,
            )?,
        ],
        NOW_MS,
    )?;
    let metadata = store.list_accounts()?;
    let preferred_id = CredentialId::try_new(
        metadata
            .iter()
            .find(|account| account.priority == 100)
            .ok_or("preferred account missing")?
            .id
            .clone(),
    )?;
    let sibling_id = CredentialId::try_new(
        metadata
            .iter()
            .find(|account| account.priority == 0)
            .ok_or("sibling account missing")?
            .id
            .clone(),
    )?;
    let compilation = store.compile_native_runtime(&bindings()?, NOW_MS)?;
    let scheduler = build_scheduler(compilation.credential_pools())?;
    Ok((database, scheduler, preferred_id, sibling_id))
}

fn account(
    identity: &str,
    priority: i64,
    weight: u32,
    concurrency: u32,
    auth_status: GrokAccountAuthStatus,
    enabled: bool,
    cooldown_until_ms: Option<i64>,
) -> Result<GrokAccountImport, Box<dyn Error>> {
    Ok(GrokAccountImport {
        provider: GrokAccountProvider::Build,
        identity: GrokAccountIdentity::try_from_bytes(identity)?,
        credential: GrokAccountCredential::try_from_bytes(build_credential_json(
            identity,
            FRESH_BUILD_EXPIRY,
        ))?,
        auth_status,
        enabled,
        priority,
        weight,
        max_concurrency: concurrency,
        refresh_due_at_ms: Some(NOW_MS + 1_000),
        quota_sync_due_at_ms: Some(NOW_MS + 2_000),
        cooldown_until_ms,
    })
}

fn build_account_with_expiry(
    identity: &str,
    priority: i64,
    expires_at: &str,
) -> Result<GrokAccountImport, Box<dyn Error>> {
    Ok(GrokAccountImport {
        provider: GrokAccountProvider::Build,
        identity: GrokAccountIdentity::try_from_bytes(identity)?,
        credential: GrokAccountCredential::try_from_bytes(build_credential_json(
            identity, expires_at,
        ))?,
        auth_status: GrokAccountAuthStatus::Active,
        enabled: true,
        priority,
        weight: 1,
        max_concurrency: 1,
        refresh_due_at_ms: Some(NOW_MS),
        quota_sync_due_at_ms: None,
        cooldown_until_ms: None,
    })
}

fn build_credential_json(identity: &str, expires_at: &str) -> Vec<u8> {
    format!(
        r#"{{"access_token":"access-{identity}","refresh_token":"refresh-{identity}","expires_at":"{expires_at}"}}"#
    )
    .into_bytes()
}

fn assert_fresh_build_selected(
    compilation: &provider_grok::GrokNativeAccountPoolCompilation,
    expired_id: &CredentialId,
    fresh_id: &CredentialId,
) -> TestResult {
    let health = RuntimeHealthRegistry::with_clock(Arc::new(FixedClock::new(NOW_MS)));
    compilation.seed_runtime_health(&health)?;
    let endpoint = EndpointId::try_new(ENDPOINT)?;
    assert!(!health.endpoint_credential_is_available(&endpoint, expired_id));
    assert!(health.endpoint_credential_is_available(&endpoint, fresh_id));
    let scheduler = build_scheduler(compilation.credential_pools())?;
    let selected = scheduler.select_runtime_eligible_and_lease(&route_id()?, &health)?;
    assert_eq!(selected.lease().credential_id(), fresh_id);
    Ok(())
}

fn bindings() -> Result<Vec<GrokAccountEndpointBinding>, Box<dyn Error>> {
    Ok(vec![GrokAccountEndpointBinding::new(
        GrokAccountProvider::Build,
        EndpointId::try_new(ENDPOINT)?,
    )])
}

fn build_scheduler(
    pools: Arc<gateway_upstream::EndpointCredentialPools>,
) -> Result<RouteCredentialScheduler, Box<dyn Error>> {
    let route_id = route_id()?;
    let public_model_id = PublicModelId::try_new("grok-public")?;
    let candidate = SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
        id: RouteCandidateId::try_new("grok-candidate")?,
        endpoint_id: EndpointId::try_new(ENDPOINT)?,
        upstream_id: UpstreamId::try_new("grok-upstream")?,
        endpoint_api_format: "openai/responses".to_owned(),
        upstream_model: MODEL.to_owned(),
        transform_mode: SnapshotTransformMode::Canonical,
        priority: 0,
        weight: 1,
        effective_capabilities: CapabilitySet::empty(),
        catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
        active_binding_count: 1,
    });
    let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
        SnapshotVersion::try_new("grok-native-v1")?,
        vec![SnapshotPublicModel::new(
            public_model_id.clone(),
            "grok-public".to_owned(),
            "Grok Public".to_owned(),
            CapabilitySet::empty(),
            route_id.clone(),
        )],
        Vec::new(),
        vec![SnapshotRoute::new(
            route_id,
            public_model_id,
            SnapshotRoutePolicy::SmoothWeightedRoundRobin,
            3,
            10_000,
            vec![candidate],
        )],
        Vec::new(),
        Vec::new(),
    ))?);
    Ok(RouteCredentialScheduler::new(snapshot, pools))
}

fn route_id() -> Result<RouteId, gateway_core::InvalidIdentifier> {
    RouteId::try_new("grok-route")
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

    fn set(&self, now_ms: i64) {
        self.0.store(now_ms, Ordering::Release);
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
                "cpa-rust-gateway-p12-10c-{timestamp}-{}-{sequence}.sqlite3",
                std::process::id()
            ));
            if !path.exists() {
                return Ok(Self(path));
            }
        }
        Err("could not allocate isolated P12-10C database".into())
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
