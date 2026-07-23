//! P9-07 synthetic Grok Web 403 attribution evidence.

#![deny(unsafe_code)]

use std::error::Error;

use gateway_core::{ErrorScope, GatewayErrorCode};
use gateway_upstream::UpstreamProxy;
use provider_grok::{
    GrokWebAccountAvailability, GrokWebAccountEvidence, GrokWebAccountFailureState,
    GrokWebBrowserEgressSession, GrokWebBrowserUserAgent, GrokWebCredential,
    GrokWebEgressAvailability, GrokWebEgressFailureState, GrokWebEgressSessionId,
    GrokWebFailureAction, GrokWebFailureError, GrokWebFailureStateError, GrokWebTlsProfile,
    classify_grok_web_http_failure,
};

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 1_000_000;
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/537.36";

#[test]
fn unknown_403_rejects_only_the_exact_egress_session_and_preserves_account_state() -> TestResult {
    let first = session("account_01", "lineage_01", 4, "egress_01", 1_500_000)?;
    let sibling_egress = session("account_01", "lineage_01", 4, "egress_02", 1_500_000)?;
    let separate_account = session("account_02", "lineage_02", 4, "egress_01", 1_500_000)?;
    let disposition = classify_grok_web_http_failure(403, GrokWebAccountEvidence::None)?;
    assert_eq!(disposition.error().code(), GatewayErrorCode::EgressRejected);
    assert_eq!(disposition.error().scope(), ErrorScope::Egress);
    assert_eq!(
        disposition.action(),
        GrokWebFailureAction::RebuildEgressSession
    );

    let mut first_egress = GrokWebEgressFailureState::try_new(&first, NOW_MS)?;
    let sibling_state = GrokWebEgressFailureState::try_new(&sibling_egress, NOW_MS)?;
    let account_state = GrokWebAccountFailureState::try_new(&first, NOW_MS)?;
    let other_account_state = GrokWebAccountFailureState::try_new(&separate_account, NOW_MS)?;
    first_egress.observe_egress_rejection(&first, &disposition, NOW_MS)?;

    assert_eq!(
        first_egress.availability(),
        GrokWebEgressAvailability::Rejected
    );
    assert_eq!(
        first_egress.require_available(&first, NOW_MS),
        Err(GrokWebFailureStateError::EgressRejected)
    );
    assert_eq!(
        sibling_state.availability(),
        GrokWebEgressAvailability::Available
    );
    sibling_state.require_available(&sibling_egress, NOW_MS)?;
    account_state.require_available(&sibling_egress, NOW_MS)?;
    other_account_state.require_available(&separate_account, NOW_MS)?;
    Ok(())
}

#[test]
fn confirmed_403_account_evidence_marks_only_the_exact_account_lifecycle_not_egress() -> TestResult
{
    let first = session("account_01", "lineage_01", 4, "egress_01", 1_500_000)?;
    let sibling_egress = session("account_01", "lineage_01", 4, "egress_02", 1_500_000)?;
    let replacement = session("account_01", "lineage_01", 5, "egress_03", 1_500_000)?;
    let disposition =
        classify_grok_web_http_failure(403, GrokWebAccountEvidence::ConfirmedForbidden)?;
    assert_eq!(
        disposition.error().code(),
        GatewayErrorCode::CredentialForbidden
    );
    assert_eq!(disposition.error().scope(), ErrorScope::Account);
    assert_eq!(
        disposition.action(),
        GrokWebFailureAction::MarkExactAccountForbidden
    );

    let egress_state = GrokWebEgressFailureState::try_new(&first, NOW_MS)?;
    let mut account_state = GrokWebAccountFailureState::try_new(&first, NOW_MS)?;
    account_state.observe_account_forbidden(&first, &disposition, NOW_MS)?;
    assert_eq!(
        account_state.availability(),
        GrokWebAccountAvailability::Forbidden
    );
    assert_eq!(
        account_state.require_available(&sibling_egress, NOW_MS),
        Err(GrokWebFailureStateError::AccountForbidden)
    );
    assert_eq!(
        account_state.require_available(&replacement, NOW_MS),
        Err(GrokWebFailureStateError::SessionBindingMismatch)
    );
    egress_state.require_available(&first, NOW_MS)?;
    Ok(())
}

#[test]
fn invalid_or_wrong_owner_evidence_fails_closed_without_state_mutation() -> TestResult {
    let session = session("account_01", "lineage_01", 4, "egress_01", 1_500_000)?;
    assert_eq!(
        classify_grok_web_http_failure(500, GrokWebAccountEvidence::ConfirmedForbidden),
        Err(GrokWebFailureError::InvalidAccountEvidence)
    );
    assert_eq!(
        classify_grok_web_http_failure(99, GrokWebAccountEvidence::None),
        Err(GrokWebFailureError::InvalidHttpStatus)
    );
    let unauthorized = classify_grok_web_http_failure(401, GrokWebAccountEvidence::None)?;
    let mut egress_state = GrokWebEgressFailureState::try_new(&session, NOW_MS)?;
    let mut account_state = GrokWebAccountFailureState::try_new(&session, NOW_MS)?;
    assert_eq!(
        egress_state.observe_egress_rejection(&session, &unauthorized, NOW_MS),
        Err(GrokWebFailureStateError::InvalidEgressAction)
    );
    assert_eq!(
        account_state.observe_account_forbidden(&session, &unauthorized, NOW_MS),
        Err(GrokWebFailureStateError::InvalidAccountAction)
    );
    assert_eq!(
        egress_state.availability(),
        GrokWebEgressAvailability::Available
    );
    assert_eq!(
        account_state.availability(),
        GrokWebAccountAvailability::Available
    );
    Ok(())
}

fn session(
    account: &str,
    lineage: &str,
    revision: u64,
    egress_session_id: &str,
    expires_at_ms: i64,
) -> Result<GrokWebBrowserEgressSession, Box<dyn Error>> {
    let credential = GrokWebCredential::import_sso_json(
        format!(
            r#"{{
                "kind":"grok_web_sso",
                "account_ref":"{account}",
                "lineage_ref":"{lineage}",
                "revision":{revision},
                "expires_at_ms":{expires_at_ms},
                "cookies":[{{"name":"sso_session","value":"session_value","domain":".grok.example.test","path":"/","secure":true,"http_only":true}}]
            }}"#
        )
        .as_bytes(),
        NOW_MS,
    )?;
    Ok(GrokWebBrowserEgressSession::try_new(
        GrokWebEgressSessionId::try_new(egress_session_id)?,
        credential,
        GrokWebBrowserUserAgent::try_new(USER_AGENT)?,
        GrokWebTlsProfile::try_new("chrome_136_macos")?,
        UpstreamProxy::Direct,
        NOW_MS,
    )?)
}
