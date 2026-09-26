//! Safe management projection of persisted attempt evidence.
use gateway_core::{
    AttemptEvent, AttemptOutcome, AttemptRetryDecision, CredentialId, EndpointId, ErrorScope,
    GatewayError, GatewayErrorCode, RequestId, RouteCandidateId, RouteId, UpstreamId,
};
use gateway_http_actix::management_resources::ManagementRequestAttempt;
use serde_json::json;
use std::error::Error;

fn event(start: i64, end: i64) -> Result<AttemptEvent, Box<dyn Error>> {
    Ok(AttemptEvent::new(
        RequestId::try_new("request")?,
        2,
        RouteId::try_new("route")?,
        RouteCandidateId::try_new("candidate")?,
        CredentialId::try_new("account")?,
        EndpointId::try_new("endpoint")?,
        UpstreamId::try_new("provider")?,
        "private-upstream-model".into(),
        start,
        end,
        AttemptOutcome::Failed(GatewayError::new(
            GatewayErrorCode::ProviderRateLimited,
            ErrorScope::Provider,
        )),
        AttemptRetryDecision::RetryEligible,
    ))
}

fn view(
    event: &AttemptEvent,
    account: &str,
    outcome: &'static str,
) -> Result<ManagementRequestAttempt, Box<dyn Error>> {
    Ok(ManagementRequestAttempt::try_new(
        event.attempt_id().as_str().into(),
        outcome,
        Some(event.endpoint_id().clone()),
        Some(CredentialId::try_new(account)?),
    )
    .map_err(|_| "invalid management attempt fixture")?
    .with_observation(event))
}

#[test]
fn projects_only_matching_closed_durable_evidence() -> Result<(), Box<dyn Error>> {
    let event = event(100, 135)?;
    let view = view(&event, "account", "failed")?;
    assert_eq!(
        serde_json::to_value(view.observation())?,
        json!({"attempt_number":2,"upstream_id":"provider","started_at_ms":100,"ended_at_ms":135,"duration_ms":35,"error_code":"ProviderRateLimited","error_scope":"provider","retry_decision":"retry_eligible"})
    );
    assert!(!serde_json::to_string(&view.observation())?.contains("private-upstream-model"));
    Ok(())
}

#[test]
fn rejects_wrong_binding_outcome_and_invalid_times_without_inventing_observations()
-> Result<(), Box<dyn Error>> {
    for event in [event(-1, 20)?, event(20, 10)?] {
        assert!(view(&event, "account", "failed")?.observation().is_none());
    }
    let event = event(10, 20)?;
    assert!(
        view(&event, "other-account", "failed")?
            .observation()
            .is_none()
    );
    assert!(
        view(&event, "account", "succeeded")?
            .observation()
            .is_none()
    );
    Ok(())
}
