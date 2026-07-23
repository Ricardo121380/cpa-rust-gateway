//! P8-05 Official/Build state, affinity, quota, and failure-isolation evidence.

#![deny(unsafe_code)]

use std::{error::Error, sync::Arc};

use gateway_core::{ClientKeyId, CredentialId, EndpointId, ErrorScope, GatewayErrorCode};
use gateway_router::{
    QuotaConfidence, QuotaSource, RuntimeQuotaAvailability, RuntimeQuotaRegistry,
    RuntimeQuotaTarget,
};
use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
use provider_grok::{
    GrokBuildAffinityReason, GrokBuildCacheAffinity, GrokBuildCacheAffinityKey,
    GrokBuildCacheIdentityDeriver, GrokBuildContinuityStore, GrokBuildModelCapability,
    GrokBuildModelSource, GrokBuildQuotaConfidence, GrokBuildQuotaSource, GrokBuildQuotaWindow,
    GrokBuildQuotaWindowKind, GrokBuildRuntimeStateStore, GrokOfficialContinuityPolicy,
    GrokOfficialFailureAction, GrokOfficialRateLimitMetadata, GrokOfficialRuntimeState,
    GrokOfficialRuntimeStateError,
};

type TestResult = Result<(), Box<dyn Error>>;

const OBSERVED_AT_MS: i64 = 10_000;

#[test]
#[allow(clippy::too_many_lines)]
fn official_header_quota_is_exact_and_cannot_mutate_build_state_or_affinity() -> TestResult {
    let runtime_quota = Arc::new(RuntimeQuotaRegistry::new());
    let official_endpoint = endpoint("official-endpoint")?;
    let official_credential = credential("official-credential")?;
    let official = GrokOfficialRuntimeState::try_new(
        official_endpoint.clone(),
        official_credential.clone(),
        Arc::clone(&runtime_quota),
    )?;

    let build_endpoint = endpoint("build-endpoint")?;
    let build_credential = credential("build-credential")?;
    let build_runtime = GrokBuildRuntimeStateStore::open_in_memory()?;
    let build_model = GrokBuildModelCapability::try_new(
        "shared-public-grok-model",
        GrokBuildModelSource::AccountCapability,
        OBSERVED_AT_MS,
    )?;
    build_runtime.sync_model_catalog(
        &build_credential,
        provider_grok::GrokBuildBillingPlan::PayAsYouGo,
        OBSERVED_AT_MS,
        std::slice::from_ref(&build_model),
    )?;
    let build_quota = GrokBuildQuotaWindow::try_new(
        GrokBuildQuotaWindowKind::PayAsYouGo,
        7,
        10,
        60,
        OBSERVED_AT_MS + 60_000,
        OBSERVED_AT_MS,
        GrokBuildQuotaSource::Billing,
        GrokBuildQuotaConfidence::Authoritative,
        "build-billing-window",
    )?;
    build_runtime.sync_quota_window(&build_credential, &build_quota)?;

    let build_continuity = GrokBuildContinuityStore::open_in_memory(secret_store()?)?;
    let client = ClientKeyId::try_new("build-client")?;
    let cache_identity = GrokBuildCacheIdentityDeriver::new([0x3b; 32]).derive(
        &client,
        "shared-public-grok-model",
        "synthetic-build-cache-key",
    )?;
    let cache_key =
        GrokBuildCacheAffinityKey::try_new(client, "shared-public-grok-model", cache_identity)?;
    let build_affinity = GrokBuildCacheAffinity::try_new(
        build_credential.clone(),
        None,
        OBSERVED_AT_MS + 100,
        GrokBuildAffinityReason::PromptCache,
    )?;
    build_continuity.bind_cache_affinity(&cache_key, &build_affinity, OBSERVED_AT_MS, None)?;

    let metadata = rate_metadata([
        ("x-ratelimit-limit-requests", "10"),
        ("x-ratelimit-remaining-requests", "0"),
        ("x-ratelimit-reset-requests", "1s"),
        ("x-ratelimit-limit-tokens", "100"),
        ("x-ratelimit-remaining-tokens", "25"),
        ("x-ratelimit-reset-tokens", "2s"),
    ])?;
    let snapshot = official
        .record_rate_limit_metadata(&metadata, OBSERVED_AT_MS)?
        .ok_or("complete Official Header metadata was not recorded")?;

    assert_eq!(snapshot.source(), QuotaSource::Header);
    assert_eq!(snapshot.confidence(), QuotaConfidence::Observed);
    assert_eq!(snapshot.windows().len(), 2);
    assert_eq!(snapshot.windows()[0].label(), "official.requests");
    assert_eq!(
        snapshot.windows()[0].reset_at_ms(),
        Some(OBSERVED_AT_MS + 1_000)
    );
    assert_eq!(snapshot.windows()[1].label(), "official.tokens");
    assert_eq!(
        runtime_quota.availability_at(
            &RuntimeQuotaTarget::endpoint_credential(
                official_endpoint.clone(),
                official_credential.clone(),
            ),
            OBSERVED_AT_MS,
        )?,
        RuntimeQuotaAvailability::Exhausted {
            reset_at_ms: OBSERVED_AT_MS + 1_000,
        }
    );
    assert_eq!(
        runtime_quota.availability_at(
            &RuntimeQuotaTarget::endpoint_credential(build_endpoint, build_credential.clone()),
            OBSERVED_AT_MS,
        )?,
        RuntimeQuotaAvailability::Available
    );
    assert_eq!(
        build_runtime.model_catalog(&build_credential)?,
        vec![build_model]
    );
    assert_eq!(
        build_runtime.quota_window(&build_credential, GrokBuildQuotaWindowKind::PayAsYouGo)?,
        Some(build_quota)
    );
    assert_eq!(
        build_continuity.cache_affinity(&cache_key, OBSERVED_AT_MS)?,
        Some(build_affinity)
    );
    assert_eq!(
        GrokOfficialRuntimeState::continuity_policy(),
        GrokOfficialContinuityPolicy::Stateless
    );

    let diagnostic = format!("{official:?} {metadata:?}");
    for private_value in ["official-endpoint", "official-credential", "10", "100"] {
        assert!(!diagnostic.contains(private_value));
    }
    Ok(())
}

#[test]
fn official_failures_only_classify_or_cool_their_own_exact_quota_target() -> TestResult {
    let runtime_quota = Arc::new(RuntimeQuotaRegistry::new());
    let official_endpoint = endpoint("official-failure-endpoint")?;
    let official_credential = credential("official-failure-credential")?;
    let official = GrokOfficialRuntimeState::try_new(
        official_endpoint.clone(),
        official_credential.clone(),
        Arc::clone(&runtime_quota),
    )?;
    let official_target =
        RuntimeQuotaTarget::endpoint_credential(official_endpoint, official_credential);
    let build_target = RuntimeQuotaTarget::endpoint_credential(
        endpoint("build-failure-endpoint")?,
        credential("build-failure-credential")?,
    );
    let no_headers = GrokOfficialRateLimitMetadata::default();

    for (status, code, scope, action) in [
        (
            401,
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
            GrokOfficialFailureAction::RequireCredentialReplacement,
        ),
        (
            403,
            GatewayErrorCode::EgressRejected,
            ErrorScope::Egress,
            GrokOfficialFailureAction::None,
        ),
        (
            503,
            GatewayErrorCode::ProviderTransient,
            ErrorScope::Provider,
            GrokOfficialFailureAction::CoolOfficialEndpoint,
        ),
    ] {
        let disposition = official.observe_http_failure(status, &no_headers, OBSERVED_AT_MS)?;
        assert_eq!(disposition.error().code(), code);
        assert_eq!(disposition.error().scope(), scope);
        assert_eq!(disposition.action(), action);
        assert!(runtime_quota.snapshot(&official_target)?.is_none());
        assert!(runtime_quota.snapshot(&build_target)?.is_none());
    }

    let retry_after = rate_metadata([("retry-after", "5")])?;
    let disposition = official.observe_http_failure(429, &retry_after, OBSERVED_AT_MS)?;
    assert_eq!(
        disposition.error().code(),
        GatewayErrorCode::ProviderRateLimited
    );
    assert_eq!(disposition.error().scope(), ErrorScope::QuotaWindow);
    assert_eq!(
        disposition.action(),
        GrokOfficialFailureAction::RecordExactQuota
    );
    let official_snapshot = runtime_quota
        .snapshot(&official_target)?
        .ok_or("Official 429 did not produce its own quota state")?;
    assert_eq!(official_snapshot.source(), QuotaSource::Header);
    assert_eq!(official_snapshot.confidence(), QuotaConfidence::Observed);
    assert_eq!(
        official_snapshot.blocking_reset_at_ms(),
        Some(OBSERVED_AT_MS + 5_000)
    );
    assert!(runtime_quota.snapshot(&build_target)?.is_none());

    let empty_metadata = GrokOfficialRateLimitMetadata::default();
    assert_eq!(
        official.record_rate_limit_metadata(&empty_metadata, OBSERVED_AT_MS)?,
        None
    );
    assert_eq!(
        official
            .record_rate_limit_metadata(&empty_metadata, 0)
            .err(),
        Some(GrokOfficialRuntimeStateError::InvalidObservationTime)
    );
    Ok(())
}

fn rate_metadata<const N: usize>(
    headers: [(&'static str, &'static str); N],
) -> Result<GrokOfficialRateLimitMetadata, gateway_core::GatewayError> {
    GrokOfficialRateLimitMetadata::parse(headers)
}

fn endpoint(value: &str) -> Result<EndpointId, gateway_core::InvalidIdentifier> {
    EndpointId::try_new(value)
}

fn credential(value: &str) -> Result<CredentialId, gateway_core::InvalidIdentifier> {
    CredentialId::try_new(value)
}

fn secret_store() -> Result<SecretStore, gateway_store::secret_store::SecretStoreError> {
    let key_version = KeyVersion::try_new(1)?;
    Ok(SecretStore::new(MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x74_u8; 32])?)],
    )?))
}
