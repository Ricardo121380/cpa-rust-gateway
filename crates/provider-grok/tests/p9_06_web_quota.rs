//! P9-06 synthetic Grok Web REST/gRPC-Web quota evidence.

#![deny(unsafe_code)]

use std::error::Error;

use gateway_upstream::UpstreamProxy;
use provider_grok::{
    GrokWebBrowserEgressSession, GrokWebBrowserUserAgent, GrokWebCredential,
    GrokWebEgressSessionId, GrokWebQuotaConfidence, GrokWebQuotaError, GrokWebQuotaFixtureDecoder,
    GrokWebQuotaSource, GrokWebQuotaState, GrokWebQuotaSyncOutcome, GrokWebQuotaWindowKind,
    GrokWebTlsProfile,
};

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 1_000_000;
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/537.36";

#[test]
fn rest_and_grpc_web_quota_windows_remain_source_and_window_isolated() -> TestResult {
    let session = session("web_account_01", "lineage_01", 7, "egress_01", 1_500_000)?;
    let rest = GrokWebQuotaFixtureDecoder::decode_rest_fixture(
        br#"{
            "tier":"premium",
            "window":{"kind":"weekly","raw_type":"provider_week","remaining":2,"total":5,"window_seconds":604800,"reset_at_ms":1600000,"observed_at_ms":1000100}
        }"#,
    )?;
    let grpc = GrokWebQuotaFixtureDecoder::decode_grpc_web_fixture(
        br#"{
            "quota":{"tier":"premium","window":{"kind":"monthly","raw_type":"provider_month","remaining":50,"total":100,"window_seconds":2592000,"reset_at_ms":3600000,"observed_at_ms":1000200}}
        }"#,
    )?;
    let mut state = GrokWebQuotaState::try_new(&session, NOW_MS)?;
    assert_eq!(
        state.sync(&session, rest, NOW_MS)?,
        GrokWebQuotaSyncOutcome::Applied
    );
    assert_eq!(
        state.sync(&session, grpc, NOW_MS)?,
        GrokWebQuotaSyncOutcome::Applied
    );
    let rest = state
        .window(GrokWebQuotaSource::Rest, GrokWebQuotaWindowKind::Weekly)
        .ok_or("REST weekly quota was not retained")?;
    assert_eq!(rest.tier().as_str(), "premium");
    assert_eq!(rest.remaining(), 2);
    assert_eq!(rest.total(), 5);
    assert_eq!(rest.source(), GrokWebQuotaSource::Rest);
    assert_eq!(rest.confidence(), GrokWebQuotaConfidence::Observed);
    let grpc = state
        .window(GrokWebQuotaSource::GrpcWeb, GrokWebQuotaWindowKind::Monthly)
        .ok_or("gRPC-Web monthly quota was not retained")?;
    assert_eq!(grpc.remaining(), 50);
    assert_eq!(grpc.total(), 100);
    assert_eq!(grpc.source(), GrokWebQuotaSource::GrpcWeb);
    assert!(
        state
            .window(GrokWebQuotaSource::Rest, GrokWebQuotaWindowKind::Monthly)
            .is_none()
    );
    Ok(())
}

#[test]
fn stale_or_conflicting_observations_and_wrong_or_expired_sessions_do_not_mutate_state()
-> TestResult {
    let active_session = session("web_account_01", "lineage_01", 7, "egress_01", 1_500_000)?;
    let mut state = GrokWebQuotaState::try_new(&active_session, NOW_MS)?;
    let newest = rest_window(10, 100, 1_000_200)?;
    assert_eq!(
        state.sync(&active_session, newest, NOW_MS)?,
        GrokWebQuotaSyncOutcome::Applied
    );
    let older_observation = rest_window(9, 100, 1_000_100)?;
    assert_eq!(
        state.sync(&active_session, older_observation, NOW_MS)?,
        GrokWebQuotaSyncOutcome::IgnoredStale
    );
    let conflict = rest_window(8, 100, 1_000_200)?;
    assert_eq!(
        state.sync(&active_session, conflict, NOW_MS),
        Err(GrokWebQuotaError::ConflictingObservation)
    );
    let wrong = session("web_account_02", "lineage_01", 7, "egress_01", 1_500_000)?;
    assert_eq!(
        state.sync(&wrong, rest_window(11, 100, 1_000_300)?, NOW_MS),
        Err(GrokWebQuotaError::SessionBindingMismatch)
    );
    let retained = state
        .window(GrokWebQuotaSource::Rest, GrokWebQuotaWindowKind::Weekly)
        .ok_or("newest REST window was lost")?;
    assert_eq!(retained.remaining(), 10);
    assert_eq!(
        state.sync(&active_session, rest_window(11, 100, 1_000_300)?, 1_500_000),
        Err(GrokWebQuotaError::ExpiredEgressSession)
    );
    Ok(())
}

#[test]
fn malformed_cross_shape_or_unsafe_quota_fixtures_fail_closed_and_diagnostics_redact_values()
-> TestResult {
    let duplicate = br#"{
        "tier":"premium","tier":"other",
        "window":{"kind":"weekly","raw_type":"provider_week","remaining":2,"total":5,"window_seconds":604800,"reset_at_ms":1600000,"observed_at_ms":1000100}
    }"#;
    assert_eq!(
        GrokWebQuotaFixtureDecoder::decode_rest_fixture(duplicate),
        Err(GrokWebQuotaError::InvalidQuotaFixture)
    );
    assert_eq!(
        GrokWebQuotaFixtureDecoder::decode_rest_fixture(
            br#"{"quota":{"tier":"premium","window":{"kind":"weekly","raw_type":"w","remaining":2,"total":5,"window_seconds":1,"reset_at_ms":2,"observed_at_ms":1}}}"#
        ),
        Err(GrokWebQuotaError::InvalidQuotaFixture)
    );
    assert_eq!(
        GrokWebQuotaFixtureDecoder::decode_grpc_web_fixture(
            br#"{"quota":{"tier":"premium","window":{"kind":"weekly","raw_type":"w","remaining":6,"total":5,"window_seconds":1,"reset_at_ms":2,"observed_at_ms":1}}}"#
        ),
        Err(GrokWebQuotaError::InvalidQuotaFixture)
    );
    let window = rest_window(2, 5, 1_000_100)?;
    let diagnostic = format!("{window:?}");
    for private_value in ["premium", "provider_week"] {
        assert!(!diagnostic.contains(private_value));
    }
    assert!(diagnostic.contains("<redacted>"));
    Ok(())
}

fn rest_window(
    remaining: u64,
    total: u64,
    observed_at_ms: i64,
) -> Result<provider_grok::GrokWebQuotaWindow, GrokWebQuotaError> {
    GrokWebQuotaFixtureDecoder::decode_rest_fixture(
        format!(
            r#"{{"tier":"premium","window":{{"kind":"weekly","raw_type":"provider_week","remaining":{remaining},"total":{total},"window_seconds":604800,"reset_at_ms":2000000,"observed_at_ms":{observed_at_ms}}}}}"#
        )
        .as_bytes(),
    )
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
