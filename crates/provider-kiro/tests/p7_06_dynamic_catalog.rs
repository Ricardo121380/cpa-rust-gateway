//! P7-06 dynamic Kiro capability and last-success regression coverage.

use std::{collections::BTreeMap, error::Error};

use gateway_catalog::{CatalogFreshnessPolicy, CatalogSnapshotFreshness};
use gateway_core::CredentialId;
use provider_kiro::dynamic_catalog::{
    KiroCredentialCapabilityObservation, KiroCredentialCapabilityProbe,
    KiroCredentialCapabilityStore, KiroDynamicCatalogError, KiroOverageCapability,
    KiroSubscriptionPlan,
};

type TestResult = Result<(), Box<dyn Error>>;

struct FixtureProbe {
    responses: BTreeMap<
        CredentialId,
        Result<KiroCredentialCapabilityObservation, KiroDynamicCatalogError>,
    >,
}

impl FixtureProbe {
    fn new(
        responses: impl IntoIterator<
            Item = (
                CredentialId,
                Result<KiroCredentialCapabilityObservation, KiroDynamicCatalogError>,
            ),
        >,
    ) -> Self {
        Self {
            responses: responses.into_iter().collect(),
        }
    }
}

impl KiroCredentialCapabilityProbe for FixtureProbe {
    fn discover(
        &self,
        credential_id: &CredentialId,
    ) -> Result<KiroCredentialCapabilityObservation, KiroDynamicCatalogError> {
        self.responses
            .get(credential_id)
            .cloned()
            .unwrap_or(Err(KiroDynamicCatalogError::InvalidResponse))
    }
}

fn credential(value: &str) -> Result<CredentialId, gateway_core::InvalidIdentifier> {
    CredentialId::try_new(value)
}

fn observation(
    model_body: &str,
    usage_body: &str,
    observed_at_ms: i64,
) -> Result<KiroCredentialCapabilityObservation, KiroDynamicCatalogError> {
    KiroCredentialCapabilityObservation::from_json(
        model_body.as_bytes(),
        usage_body.as_bytes(),
        observed_at_ms,
    )
}

const PRO_USAGE: &str = r#"{
  "subscriptionInfo": {
    "subscriptionTitle": "KIRO PRO+",
    "overageCapability": "OVERAGE_CAPABLE"
  }
}"#;

const FREE_USAGE: &str = r#"{
  "subscriptionInfo": {
    "subscriptionTitle": "KIRO FREE",
    "overageCapability": "NOT_AVAILABLE"
  }
}"#;

#[test]
fn paired_json_keeps_only_safe_dynamic_model_and_subscription_projections() -> TestResult {
    let observation = observation(
        r#"{
          "models": [
            {"modelId": " claude-sonnet ", "tokenLimits": {"maxInputTokens": 200000}},
            {"modelId": "claude-opus"}
          ]
        }"#,
        PRO_USAGE,
        100,
    )?;

    assert_eq!(observation.models().len(), 2);
    assert_eq!(observation.models()[0].model_id(), "claude-opus");
    assert_eq!(observation.models()[1].model_id(), "claude-sonnet");
    assert_eq!(observation.models()[1].max_input_tokens(), Some(200_000));
    assert_eq!(
        observation.subscription().plan(),
        KiroSubscriptionPlan::Paid
    );
    assert_eq!(
        observation.subscription().overage(),
        KiroOverageCapability::Supported
    );

    let rendered = format!("{observation:?}");
    assert!(!rendered.contains("KIRO PRO+"));
    assert!(!rendered.contains("claude-sonnet"));
    Ok(())
}

#[test]
fn malformed_or_conflicting_source_rows_do_not_become_a_success() {
    let malformed = observation(r#"{"models": {}}"#, PRO_USAGE, 100);
    assert_eq!(malformed, Err(KiroDynamicCatalogError::InvalidResponse));

    let conflicting = observation(
        r#"{
          "models": [
            {"modelId": "sonnet", "tokenLimits": {"maxInputTokens": 100}},
            {"modelId": "sonnet", "tokenLimits": {"maxInputTokens": 200}}
          ]
        }"#,
        PRO_USAGE,
        100,
    );
    assert_eq!(conflicting, Err(KiroDynamicCatalogError::InvalidResponse));

    let unknown = observation(r#"{"models": []}"#, r"{}", 100);
    assert!(unknown.is_ok());
}

#[test]
fn snapshot_has_p4_fresh_stale_refresh_and_expiry_boundaries() -> TestResult {
    let policy = CatalogFreshnessPolicy::try_new(10, 20, 30)?;
    let store = KiroCredentialCapabilityStore::new(policy);
    let id = credential("kiro-dynamic-boundary")?;
    let snapshot = store.record_success(
        id.clone(),
        observation(r#"{"models": [{"modelId": "sonnet"}]}"#, FREE_USAGE, 100)?,
    )?;

    assert_eq!(snapshot.version(), 1);
    assert_eq!(snapshot.stale_at_ms(), 110);
    assert_eq!(snapshot.refresh_due_at_ms(), 120);
    assert_eq!(snapshot.expires_at_ms(), 130);
    assert_eq!(snapshot.freshness_at(109)?, CatalogSnapshotFreshness::Fresh);
    assert_eq!(snapshot.freshness_at(110)?, CatalogSnapshotFreshness::Stale);
    assert!(!snapshot.is_refresh_due_at(119)?);
    assert!(snapshot.is_refresh_due_at(120)?);
    assert_eq!(
        snapshot.freshness_at(130)?,
        CatalogSnapshotFreshness::Expired
    );
    assert_eq!(
        snapshot.freshness_at(99),
        Err(KiroDynamicCatalogError::ClockBeforeSnapshot)
    );
    assert_eq!(snapshot.subscription().plan(), KiroSubscriptionPlan::Free);
    assert_eq!(
        snapshot.subscription().overage(),
        KiroOverageCapability::Unsupported
    );
    Ok(())
}

#[test]
fn failed_credential_reuses_only_its_eligible_last_success_and_does_not_block_union() -> TestResult
{
    let policy = CatalogFreshnessPolicy::try_new(10, 20, 40)?;
    let store = KiroCredentialCapabilityStore::new(policy);
    let stale = credential("kiro-stale")?;
    let current = credential("kiro-current")?;
    let unavailable = credential("kiro-unavailable")?;
    store.record_success(
        stale.clone(),
        observation(
            r#"{"models": [{"modelId": "claude-sonnet"}]}"#,
            FREE_USAGE,
            100,
        )?,
    )?;

    let probe = FixtureProbe::new([
        (stale.clone(), Err(KiroDynamicCatalogError::InvalidResponse)),
        (
            current.clone(),
            Ok(observation(
                r#"{
                  "models": [
                    {"modelId": "claude-sonnet"},
                    {"modelId": "claude-opus"}
                  ]
                }"#,
                PRO_USAGE,
                110,
            )?),
        ),
        (
            unavailable.clone(),
            Err(KiroDynamicCatalogError::InvalidResponse),
        ),
    ]);
    let aggregate = store.aggregate(
        [stale.clone(), current.clone(), unavailable.clone()],
        110,
        &probe,
    )?;

    assert_eq!(aggregate.current_successes(), 1);
    assert_eq!(aggregate.retained_successes(), 1);
    assert_eq!(aggregate.unavailable_credentials(), 1);
    assert_eq!(aggregate.credential_statuses().len(), 2);
    assert_eq!(aggregate.models().len(), 2);
    assert_eq!(aggregate.models()[0].model().model_id(), "claude-opus");
    assert_eq!(aggregate.models()[1].model().model_id(), "claude-sonnet");
    assert_eq!(
        aggregate.models()[1].eligible_credential_count(),
        2,
        "the exact shared source model is deduplicated across credentials"
    );
    assert!(
        aggregate
            .models()
            .iter()
            .all(|model| model.model().model_id() != "claude-opus-thinking"),
        "this boundary must never synthesize a second thinking model"
    );
    assert!(store.last_success(&unavailable)?.is_none());
    Ok(())
}

#[test]
fn expired_failure_snapshot_is_not_admitted_and_other_credentials_continue() -> TestResult {
    let policy = CatalogFreshnessPolicy::try_new(10, 20, 30)?;
    let store = KiroCredentialCapabilityStore::new(policy);
    let expired = credential("kiro-expired")?;
    let current = credential("kiro-current-after-expiry")?;
    store.record_success(
        expired.clone(),
        observation(
            r#"{"models": [{"modelId": "expired-model"}]}"#,
            FREE_USAGE,
            100,
        )?,
    )?;
    let probe = FixtureProbe::new([
        (
            expired.clone(),
            Err(KiroDynamicCatalogError::InvalidResponse),
        ),
        (
            current.clone(),
            Ok(observation(
                r#"{"models": [{"modelId": "current-model"}]}"#,
                PRO_USAGE,
                130,
            )?),
        ),
    ]);

    let aggregate = store.aggregate([expired.clone(), current], 130, &probe)?;
    assert_eq!(aggregate.current_successes(), 1);
    assert_eq!(aggregate.retained_successes(), 0);
    assert_eq!(aggregate.unavailable_credentials(), 1);
    assert_eq!(aggregate.models().len(), 1);
    assert_eq!(aggregate.models()[0].model().model_id(), "current-model");
    assert_eq!(
        store
            .status_at(&expired, 130)?
            .map(|status| status.freshness()),
        Some(CatalogSnapshotFreshness::Expired)
    );
    Ok(())
}

#[test]
fn union_with_conflicting_token_limits_fails_closed_to_unknown_limit() -> TestResult {
    let store = KiroCredentialCapabilityStore::default();
    let first = credential("kiro-limit-a")?;
    let second = credential("kiro-limit-b")?;
    let probe = FixtureProbe::new([
        (
            first.clone(),
            Ok(observation(
                r#"{"models": [{"modelId": "sonnet", "tokenLimits": {"maxInputTokens": 100}}]}"#,
                PRO_USAGE,
                100,
            )?),
        ),
        (
            second.clone(),
            Ok(observation(
                r#"{"models": [{"modelId": "sonnet", "tokenLimits": {"maxInputTokens": 200}}]}"#,
                PRO_USAGE,
                100,
            )?),
        ),
    ]);
    let aggregate = store.aggregate([first, second], 100, &probe)?;

    assert_eq!(aggregate.models().len(), 1);
    assert_eq!(aggregate.models()[0].model().max_input_tokens(), None);
    assert_eq!(aggregate.models()[0].eligible_credential_count(), 2);
    Ok(())
}

#[test]
fn all_failed_credentials_return_an_empty_safe_projection_not_a_leaked_probe_error() -> TestResult {
    let store = KiroCredentialCapabilityStore::default();
    let first = credential("kiro-all-failed-a")?;
    let second = credential("kiro-all-failed-b")?;
    let probe = FixtureProbe::new([
        (first.clone(), Err(KiroDynamicCatalogError::InvalidResponse)),
        (
            second.clone(),
            Err(KiroDynamicCatalogError::InvalidResponse),
        ),
    ]);
    let aggregate = store.aggregate([first, second], 100, &probe)?;

    assert!(aggregate.models().is_empty());
    assert_eq!(aggregate.current_successes(), 0);
    assert_eq!(aggregate.retained_successes(), 0);
    assert_eq!(aggregate.unavailable_credentials(), 2);
    Ok(())
}

#[test]
fn duplicate_credential_input_and_non_monotonic_success_are_rejected_without_replacement()
-> TestResult {
    let store = KiroCredentialCapabilityStore::default();
    let id = credential("kiro-duplicate")?;
    let initial = store.record_success(
        id.clone(),
        observation(r#"{"models": [{"modelId": "old"}]}"#, FREE_USAGE, 200)?,
    )?;
    let stale = store.record_success(
        id.clone(),
        observation(r#"{"models": [{"modelId": "new"}]}"#, PRO_USAGE, 199)?,
    );
    assert_eq!(stale, Err(KiroDynamicCatalogError::TimestampNotMonotonic));
    assert_eq!(store.last_success(&id)?, Some(initial));

    let probe = FixtureProbe::new([(id.clone(), Err(KiroDynamicCatalogError::InvalidResponse))]);
    assert_eq!(
        store.aggregate([id.clone(), id], 200, &probe),
        Err(KiroDynamicCatalogError::DuplicateCredentialId)
    );
    Ok(())
}
