//! P6-04 through P6-07 durable Build state and failure-boundary evidence.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use gateway_core::{
    ClientKeyId, CredentialId, EgressPolicyId, ErrorScope, GatewayErrorCode, ResponseId,
};
use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
use provider_grok::{
    GrokBuildAccountEvidence, GrokBuildAffinityBindOutcome, GrokBuildAffinityBreakInput,
    GrokBuildAffinityBreakReason, GrokBuildAffinityReason, GrokBuildCacheAffinity,
    GrokBuildCacheAffinityKey, GrokBuildCacheIdentityDeriver, GrokBuildCatalogSyncOutcome,
    GrokBuildContinuityError, GrokBuildContinuityStore, GrokBuildFailureAction,
    GrokBuildModelCapability, GrokBuildModelSource, GrokBuildQuotaConfidence, GrokBuildQuotaSource,
    GrokBuildQuotaSyncOutcome, GrokBuildQuotaWindow, GrokBuildQuotaWindowKind,
    GrokBuildRateLimitEvidence, GrokBuildReasoningReplay, GrokBuildReplayKey,
    GrokBuildReplayWriteOutcome, GrokBuildResponseOwnership, GrokBuildResponsesErrorSignal,
    GrokBuildRuntimeStateError, GrokBuildRuntimeStateStore, GrokBuildUpstreamResponseId,
    classify_grok_build_failure,
};
use rusqlite::{Connection, params};

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 50_000;
static TEST_DATABASE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn account_catalog_state_is_credential_scoped_and_monotonic() -> TestResult {
    let store = GrokBuildRuntimeStateStore::open_in_memory()?;
    let credential_a = credential("credential-runtime-a")?;
    let credential_b = credential("credential-runtime-b")?;
    let a_catalog = [model(
        "grok-build-a",
        GrokBuildModelSource::AccountCapability,
        100,
    )?];
    let b_catalog = [model(
        "grok-build-b",
        GrokBuildModelSource::BuildResponse,
        200,
    )?];

    assert_eq!(
        store.sync_model_catalog(
            &credential_a,
            provider_grok::GrokBuildBillingPlan::PayAsYouGo,
            100,
            &a_catalog,
        )?,
        GrokBuildCatalogSyncOutcome::Applied
    );
    assert_eq!(
        store.sync_model_catalog(
            &credential_b,
            provider_grok::GrokBuildBillingPlan::Free,
            200,
            &b_catalog,
        )?,
        GrokBuildCatalogSyncOutcome::Applied
    );
    assert_eq!(store.model_catalog(&credential_a)?, a_catalog);
    assert_eq!(store.model_catalog(&credential_b)?, b_catalog);

    let stale_catalog = [model(
        "stale-should-not-replace",
        GrokBuildModelSource::BuildResponse,
        90,
    )?];
    assert_eq!(
        store.sync_model_catalog(
            &credential_a,
            provider_grok::GrokBuildBillingPlan::Subscription,
            90,
            &stale_catalog,
        )?,
        GrokBuildCatalogSyncOutcome::IgnoredStale
    );
    assert_eq!(store.model_catalog(&credential_a)?, a_catalog);
    Ok(())
}

#[test]
fn quota_state_is_credential_scoped_source_labelled_and_monotonic() -> TestResult {
    let store = GrokBuildRuntimeStateStore::open_in_memory()?;
    let credential_a = credential("credential-quota-a")?;
    let credential_b = credential("credential-quota-b")?;
    let catalog = [model(
        "grok-build-quota-model-a",
        GrokBuildModelSource::AccountCapability,
        100,
    )?];
    store.sync_model_catalog(
        &credential_a,
        provider_grok::GrokBuildBillingPlan::PayAsYouGo,
        100,
        &catalog,
    )?;
    store.sync_model_catalog(
        &credential_b,
        provider_grok::GrokBuildBillingPlan::Free,
        100,
        &catalog,
    )?;

    let a_billing = quota(
        GrokBuildQuotaWindowKind::PayAsYouGo,
        80,
        100,
        400,
        GrokBuildQuotaSource::Billing,
        GrokBuildQuotaConfidence::Authoritative,
        "account-billing-window",
    )?;
    let b_estimate = quota(
        GrokBuildQuotaWindowKind::PayAsYouGo,
        7,
        20,
        500,
        GrokBuildQuotaSource::LocalEstimate,
        GrokBuildQuotaConfidence::Estimated,
        "local-estimate-window",
    )?;
    assert_eq!(
        store.sync_quota_window(&credential_a, &a_billing)?,
        GrokBuildQuotaSyncOutcome::Applied
    );
    assert_eq!(
        store.sync_quota_window(&credential_b, &b_estimate)?,
        GrokBuildQuotaSyncOutcome::Applied
    );
    assert_eq!(
        store
            .quota_window(&credential_a, GrokBuildQuotaWindowKind::PayAsYouGo)?
            .ok_or("missing credential-a quota")?,
        a_billing
    );
    assert_eq!(
        store
            .quota_window(&credential_b, GrokBuildQuotaWindowKind::PayAsYouGo)?
            .ok_or("missing credential-b quota")?,
        b_estimate
    );

    let stale_quota = quota(
        GrokBuildQuotaWindowKind::PayAsYouGo,
        1,
        100,
        399,
        GrokBuildQuotaSource::ResponseHeaders,
        GrokBuildQuotaConfidence::Observed,
        "response-header-window",
    )?;
    assert_eq!(
        store.sync_quota_window(&credential_a, &stale_quota)?,
        GrokBuildQuotaSyncOutcome::IgnoredStale
    );
    assert_eq!(
        store
            .quota_window(&credential_a, GrokBuildQuotaWindowKind::PayAsYouGo)?
            .ok_or("newer quota was lost")?,
        a_billing
    );
    assert_eq!(
        GrokBuildQuotaWindow::try_new(
            GrokBuildQuotaWindowKind::Free,
            1,
            2,
            60,
            200,
            100,
            GrokBuildQuotaSource::Billing,
            GrokBuildQuotaConfidence::Estimated,
            "mismatched-source-confidence",
        )
        .err(),
        Some(GrokBuildRuntimeStateError::InvalidQuotaSnapshot)
    );
    Ok(())
}

#[test]
fn cache_affinity_is_tenant_scoped_and_rebinding_has_durable_break_evidence() -> TestResult {
    let database = TestDatabase::new();
    let client_a = client("client-affinity-a")?;
    let client_b = client("client-affinity-b")?;
    let credential_a = credential("credential-affinity-a")?;
    let credential_b = credential("credential-affinity-b")?;
    let egress_a = EgressPolicyId::try_new("egress-affinity-a")?;
    let egress_b = EgressPolicyId::try_new("egress-affinity-b")?;
    let key_a = cache_key(client_a.clone())?;
    let key_b = cache_key(client_b.clone())?;
    let affinity_a = affinity(credential_a.clone(), Some(egress_a))?;
    let affinity_b = affinity(credential_b.clone(), Some(egress_b))?;

    {
        let store = GrokBuildContinuityStore::open(database.path(), secret_store()?)?;
        assert_eq!(
            store.bind_cache_affinity(&key_a, &affinity_a, NOW_MS, None)?,
            GrokBuildAffinityBindOutcome::Bound
        );
        assert_eq!(
            store.bind_cache_affinity(&key_b, &affinity_b, NOW_MS, None)?,
            GrokBuildAffinityBindOutcome::Bound
        );
        assert_eq!(
            store
                .cache_affinity(&key_a, NOW_MS)?
                .ok_or("missing tenant-a affinity")?
                .credential_id(),
            &credential_a
        );
        assert_eq!(
            store
                .cache_affinity(&key_b, NOW_MS)?
                .ok_or("missing tenant-b affinity")?
                .credential_id(),
            &credential_b
        );
        assert_eq!(
            store
                .bind_cache_affinity(&key_a, &affinity_b, NOW_MS, None)
                .err(),
            Some(GrokBuildContinuityError::AffinityBreakRequired)
        );
        let rebound = store.bind_cache_affinity(
            &key_a,
            &affinity_b,
            NOW_MS,
            Some(GrokBuildAffinityBreakInput::new(
                GrokBuildAffinityBreakReason::OperatorRebind,
                321,
                NOW_MS,
            )),
        )?;
        assert!(matches!(
            rebound,
            GrokBuildAffinityBindOutcome::Rebound(ref record)
                if record.prior_credential_id() == &credential_a
                    && record.next_credential_id() == &credential_b
                    && record.estimated_cache_loss_tokens() == 321
        ));
        assert_eq!(
            store
                .cache_affinity(&key_a, NOW_MS)?
                .ok_or("rebound affinity was not persisted")?
                .credential_id(),
            &credential_b
        );
    }

    let connection = Connection::open(database.path())?;
    let breaks: i64 = connection.query_row(
        "SELECT COUNT(*) FROM grok_build_affinity_breaks \
         WHERE client_key_id = ?1 AND prior_credential_id = ?2 AND next_credential_id = ?3",
        params![
            client_a.as_str(),
            credential_a.as_str(),
            credential_b.as_str()
        ],
        |row| row.get(0),
    )?;
    assert_eq!(breaks, 1);
    Ok(())
}

#[test]
fn response_ownership_and_reasoning_replay_are_exact_encrypted_and_clearable() -> TestResult {
    const REPLAY: &[u8] = b"synthetic-grok-build-reasoning-replay";

    let database = TestDatabase::new();
    let client_a = client("client-continuity-a")?;
    let client_b = client("client-continuity-b")?;
    let credential_a = credential("credential-continuity-a")?;
    let credential_b = credential("credential-continuity-b")?;
    let response = ResponseId::try_new("response-continuity-a")?;
    let ownership = GrokBuildResponseOwnership::try_new(
        credential_a.clone(),
        GrokBuildUpstreamResponseId::try_new("upstream-response-a")?,
        NOW_MS + 300,
    )?;
    let replay_key_a =
        GrokBuildReplayKey::try_new(client_a.clone(), "grok-build-model", "session-a")?;
    let replay_key_b =
        GrokBuildReplayKey::try_new(client_b.clone(), "grok-build-model", "session-a")?;
    let ciphertext_key =
        GrokBuildReplayKey::try_new(client_a.clone(), "grok-build-model", "session-ciphertext")?;
    let replay = GrokBuildReasoningReplay::try_new(REPLAY.to_vec())?;

    {
        let store = GrokBuildContinuityStore::open(database.path(), secret_store()?)?;
        store.record_response_ownership(&client_a, &response, &ownership, NOW_MS)?;
        store.record_response_ownership(&client_a, &response, &ownership, NOW_MS)?;
        assert_eq!(
            store
                .resolve_response_ownership(&client_a, &response, &credential_b, NOW_MS)
                .err(),
            Some(GrokBuildContinuityError::OwnershipCredentialMismatch)
        );
        assert_eq!(
            store
                .resolve_response_ownership(&client_b, &response, &credential_a, NOW_MS)
                .err(),
            Some(GrokBuildContinuityError::OwnershipMissing)
        );
        assert_eq!(
            store.write_reasoning_replay(&replay_key_a, &replay, NOW_MS + 300, NOW_MS)?,
            GrokBuildReplayWriteOutcome::Inserted
        );
        assert_eq!(
            store.write_reasoning_replay(&replay_key_a, &replay, NOW_MS + 300, NOW_MS)?,
            GrokBuildReplayWriteOutcome::Deduplicated
        );
        assert!(store.reasoning_replay(&replay_key_b, NOW_MS)?.is_none());
        assert_eq!(
            store
                .reasoning_replay(&replay_key_a, NOW_MS)?
                .ok_or("missing encrypted replay")?
                .as_bytes(),
            REPLAY
        );
        assert_eq!(
            store.write_reasoning_replay(&ciphertext_key, &replay, NOW_MS + 300, NOW_MS)?,
            GrokBuildReplayWriteOutcome::Inserted
        );
        store.clear_reasoning_replay(&replay_key_a)?;
        assert!(store.reasoning_replay(&replay_key_a, NOW_MS)?.is_none());
    }

    let connection = Connection::open(database.path())?;
    let ciphertext: Vec<u8> = connection.query_row(
        "SELECT ciphertext FROM grok_build_reasoning_replay \
         WHERE client_key_id = ?1 AND session_id = ?2",
        params![client_a.as_str(), "session-ciphertext"],
        |row| row.get(0),
    )?;
    assert!(
        !ciphertext
            .windows(REPLAY.len())
            .any(|window| window == REPLAY),
        "reasoning replay ciphertext retained synthetic plaintext"
    );
    Ok(())
}

#[test]
fn failure_matrix_never_turns_egress_or_transient_faults_into_permanent_credential_state() {
    let cases = [
        (
            401,
            GrokBuildResponsesErrorSignal::None,
            GrokBuildAccountEvidence::None,
            GrokBuildRateLimitEvidence::None,
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
            GrokBuildFailureAction::RequireReauthorization,
        ),
        (
            403,
            GrokBuildResponsesErrorSignal::None,
            GrokBuildAccountEvidence::None,
            GrokBuildRateLimitEvidence::None,
            GatewayErrorCode::EgressRejected,
            ErrorScope::Egress,
            GrokBuildFailureAction::None,
        ),
        (
            403,
            GrokBuildResponsesErrorSignal::None,
            GrokBuildAccountEvidence::ConfirmedForbidden,
            GrokBuildRateLimitEvidence::None,
            GatewayErrorCode::CredentialForbidden,
            ErrorScope::Account,
            GrokBuildFailureAction::MarkAccountForbidden,
        ),
        (
            429,
            GrokBuildResponsesErrorSignal::FreeUsageExhausted,
            GrokBuildAccountEvidence::None,
            GrokBuildRateLimitEvidence::None,
            GatewayErrorCode::CredentialQuotaExceeded,
            ErrorScope::QuotaWindow,
            GrokBuildFailureAction::CoolQuotaWindow,
        ),
        (
            429,
            GrokBuildResponsesErrorSignal::None,
            GrokBuildAccountEvidence::None,
            GrokBuildRateLimitEvidence::Account,
            GatewayErrorCode::ProviderRateLimited,
            ErrorScope::Account,
            GrokBuildFailureAction::CoolAccount,
        ),
        (
            503,
            GrokBuildResponsesErrorSignal::None,
            GrokBuildAccountEvidence::None,
            GrokBuildRateLimitEvidence::None,
            GatewayErrorCode::ProviderTransient,
            ErrorScope::Provider,
            GrokBuildFailureAction::CoolProvider,
        ),
    ];

    for (status, signal, account, rate_limit, code, scope, action) in cases {
        let disposition = classify_grok_build_failure(status, signal, account, rate_limit);
        assert_eq!(disposition.error().code(), code);
        assert_eq!(disposition.error().scope(), scope);
        assert_eq!(disposition.action(), action);
    }
}

fn model(
    upstream_model: &str,
    source: GrokBuildModelSource,
    observed_at_ms: i64,
) -> Result<GrokBuildModelCapability, GrokBuildRuntimeStateError> {
    GrokBuildModelCapability::try_new(upstream_model, source, observed_at_ms)
}

#[allow(clippy::too_many_arguments)]
fn quota(
    kind: GrokBuildQuotaWindowKind,
    remaining: u64,
    total: u64,
    observed_at_ms: i64,
    source: GrokBuildQuotaSource,
    confidence: GrokBuildQuotaConfidence,
    raw_window_type: &str,
) -> Result<GrokBuildQuotaWindow, GrokBuildRuntimeStateError> {
    GrokBuildQuotaWindow::try_new(
        kind,
        remaining,
        total,
        60,
        observed_at_ms + 60_000,
        observed_at_ms,
        source,
        confidence,
        raw_window_type,
    )
}

fn affinity(
    credential_id: CredentialId,
    egress_policy_id: Option<EgressPolicyId>,
) -> Result<GrokBuildCacheAffinity, GrokBuildContinuityError> {
    GrokBuildCacheAffinity::try_new(
        credential_id,
        egress_policy_id,
        NOW_MS + 300,
        GrokBuildAffinityReason::PromptCache,
    )
}

fn cache_key(
    client_key_id: ClientKeyId,
) -> Result<GrokBuildCacheAffinityKey, GrokBuildContinuityError> {
    let cache_identity = GrokBuildCacheIdentityDeriver::new([0x8c; 32]).derive(
        &client_key_id,
        "grok-build-model",
        "synthetic-client-cache-key",
    )?;
    GrokBuildCacheAffinityKey::try_new(client_key_id, "grok-build-model", cache_identity)
}

fn client(value: &str) -> Result<ClientKeyId, gateway_core::InvalidIdentifier> {
    ClientKeyId::try_new(value)
}

fn credential(value: &str) -> Result<CredentialId, gateway_core::InvalidIdentifier> {
    CredentialId::try_new(value)
}

fn secret_store() -> Result<SecretStore, gateway_store::secret_store::SecretStoreError> {
    let key_version = KeyVersion::try_new(1)?;
    Ok(SecretStore::new(MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x95_u8; 32])?)],
    )?))
}

struct TestDatabase {
    path: PathBuf,
}

impl TestDatabase {
    fn new() -> Self {
        let sequence = TEST_DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self {
            path: std::env::temp_dir().join(format!(
                "cpa-rust-gateway-p6-runtime-continuity-{}-{sequence}.sqlite",
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
