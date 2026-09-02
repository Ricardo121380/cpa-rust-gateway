//! P6-01 synthetic Grok Build OAuth import and Device Code evidence.

use std::{collections::VecDeque, error::Error, sync::Mutex};

use provider_grok::{
    GROK_BUILD_OAUTH_ISSUER, GROK_BUILD_OAUTH_SCOPE, GROK_BUILD_PUBLIC_CLIENT_ID,
    GrokBuildCredential, GrokBuildCredentialSource, GrokBuildDevicePollOutcome,
    GrokBuildOAuthEndpoint, GrokBuildOAuthError, GrokBuildOAuthFlow, GrokBuildOAuthHttpResponse,
    GrokBuildOAuthRequest, GrokBuildOAuthRequestKind, GrokBuildOAuthTransport,
    GrokBuildOAuthTransportError,
};

#[test]
fn strict_import_retains_only_validated_fields_and_redacts_tokens() -> Result<(), Box<dyn Error>> {
    let credential = GrokBuildCredential::import_json(
        br#"{
            "access_token":"synthetic_access_012345",
            "refresh_token":"synthetic_refresh_012345",
            "expires_in":3600,
            "token_type":"Bearer",
            "id_token":"synthetic_id_012345"
        }"#,
        10_000,
    )?;

    assert_eq!(credential.access_token(), "synthetic_access_012345");
    assert_eq!(credential.refresh_token(), "synthetic_refresh_012345");
    assert_eq!(credential.expires_at_ms(), 3_610_000);
    assert_eq!(credential.client_id(), GROK_BUILD_PUBLIC_CLIENT_ID);
    assert_eq!(credential.source(), GrokBuildCredentialSource::ImportedJson);
    assert!(!credential.is_expired_at(3_609_999));
    assert!(credential.is_expired_at(3_610_000));

    let debug = format!("{credential:?}");
    for synthetic_secret in [
        "synthetic_access_012345",
        "synthetic_refresh_012345",
        "synthetic_id_012345",
    ] {
        assert!(!debug.contains(synthetic_secret));
    }
    assert!(debug.contains("<redacted>"));
    assert!(
        GrokBuildCredential::import_runtime_json(
            br#"{"access_token":"relative-access","refresh_token":"relative-refresh","expires_in":3600,"token_type":"Bearer"}"#,
            10_000,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn known_absolute_expiry_sources_import_in_memory_and_redact_tokens() -> Result<(), Box<dyn Error>>
{
    const OBSERVED_AT_MS: i64 = 1_735_689_600_000;
    const EXPIRES_AT_MS: i64 = 1_735_689_610_250;

    let cpa = GrokBuildCredential::import_cpa_xai_auth_file(
        br#"{
            "type":"xai",
            "auth_kind":"oauth",
            "access_token":"synthetic_cpa_access_012345",
            "refresh_token":"synthetic_cpa_refresh_012345",
            "expires_in":3600,
            "expired":"2025-01-01T00:00:10.250Z",
            "issuer":"https://auth.x.ai",
            "email":"ignored@example.test",
            "base_url":"https://api.x.ai/v1"
        }"#,
        OBSERVED_AT_MS,
    )?;
    let account = format!(
        r#"{{
            "access_token":"synthetic_account_access_012345",
            "refresh_token":"synthetic_account_refresh_012345",
            "expires_at":"2025-01-01T00:00:10.250Z",
            "client_id":"{GROK_BUILD_PUBLIC_CLIENT_ID}",
            "issuer":"{GROK_BUILD_OAUTH_ISSUER}",
            "scope":"{GROK_BUILD_OAUTH_SCOPE}",
            "token_type":"Bearer",
            "metadata":{{"ignored":true}}
        }}"#
    );
    let grok_account =
        GrokBuildCredential::import_grok_account_json(account.as_bytes(), OBSERVED_AT_MS)?;
    let cli_cache = format!(
        r#"{{
            "{GROK_BUILD_OAUTH_ISSUER}::{GROK_BUILD_PUBLIC_CLIENT_ID}":{{
                "key":"synthetic_cli_access_012345",
                "refresh_token":"synthetic_cli_refresh_012345",
                "expires_at":"2025-01-01T00:00:10.250Z",
                "issuer":"{GROK_BUILD_OAUTH_ISSUER}",
                "metadata":{{"ignored":true}}
            }},
            "https://other.example.test::other-client":{{"ignored":"entry"}}
        }}"#
    );
    let official_cli =
        GrokBuildCredential::import_official_cli_auth_cache(cli_cache.as_bytes(), OBSERVED_AT_MS)?;
    for (credential, source, access_token, refresh_token) in [
        (
            &cpa,
            GrokBuildCredentialSource::CpaXaiAuthFile,
            "synthetic_cpa_access_012345",
            "synthetic_cpa_refresh_012345",
        ),
        (
            &grok_account,
            GrokBuildCredentialSource::GrokAccountJson,
            "synthetic_account_access_012345",
            "synthetic_account_refresh_012345",
        ),
        (
            &official_cli,
            GrokBuildCredentialSource::OfficialCliAuthCache,
            "synthetic_cli_access_012345",
            "synthetic_cli_refresh_012345",
        ),
    ] {
        assert_eq!(credential.source(), source);
        assert_eq!(credential.access_token(), access_token);
        assert_eq!(credential.refresh_token(), refresh_token);
        assert_eq!(credential.expires_at_ms(), EXPIRES_AT_MS);
        assert_eq!(credential.client_id(), GROK_BUILD_PUBLIC_CLIENT_ID);
        assert_eq!(credential.scope(), GROK_BUILD_OAUTH_SCOPE);
        let debug = format!("{credential:?}");
        assert!(!debug.contains(access_token));
        assert!(!debug.contains(refresh_token));
        assert!(debug.contains("<redacted>"));
    }
    Ok(())
}

#[test]
fn runtime_import_accepts_only_durable_absolute_expiry_sources() -> Result<(), Box<dyn Error>> {
    const OBSERVED_AT_MS: i64 = 1_735_689_600_000;
    let cpa = br#"{
        "type":"xai","auth_kind":"oauth",
        "access_token":"runtime_cpa_access","refresh_token":"runtime_cpa_refresh",
        "expires_in":3600,"expired":"2025-01-01T00:00:10.250Z",
        "issuer":"https://auth.x.ai","email":"ignored@example.test",
        "base_url":"https://api.x.ai/v1"
    }"#;
    let account = format!(
        r#"{{
            "access_token":"runtime_account_access","refresh_token":"runtime_account_refresh",
            "expires_at":"2025-01-01T00:00:10.250Z",
            "client_id":"{GROK_BUILD_PUBLIC_CLIENT_ID}",
            "issuer":"{GROK_BUILD_OAUTH_ISSUER}","scope":"{GROK_BUILD_OAUTH_SCOPE}",
            "token_type":"Bearer"
        }}"#
    );
    let cli_cache = format!(
        r#"{{"{GROK_BUILD_OAUTH_ISSUER}::{GROK_BUILD_PUBLIC_CLIENT_ID}":{{
            "key":"runtime_cli_access","refresh_token":"runtime_cli_refresh",
            "expires_at":"2025-01-01T00:00:10.250Z",
            "issuer":"{GROK_BUILD_OAUTH_ISSUER}"
        }}}}"#
    );

    for (input, expected_source) in [
        (cpa.as_slice(), GrokBuildCredentialSource::CpaXaiAuthFile),
        (
            account.as_bytes(),
            GrokBuildCredentialSource::GrokAccountJson,
        ),
        (
            cli_cache.as_bytes(),
            GrokBuildCredentialSource::OfficialCliAuthCache,
        ),
    ] {
        assert_eq!(
            GrokBuildCredential::import_runtime_json(input, OBSERVED_AT_MS)?.source(),
            expected_source
        );
    }
    Ok(())
}

#[test]
fn refresh_import_accepts_recently_expired_access_without_serving_it() -> Result<(), Box<dyn Error>>
{
    const OBSERVED_AT_MS: i64 = 1_735_776_000_000;
    let account = format!(
        r#"{{
            "access_token":"expired_access","refresh_token":"still_refreshable",
            "expires_at":"2025-01-01T00:00:10.250Z",
            "client_id":"{GROK_BUILD_PUBLIC_CLIENT_ID}",
            "issuer":"{GROK_BUILD_OAUTH_ISSUER}","scope":"{GROK_BUILD_OAUTH_SCOPE}",
            "token_type":"Bearer"
        }}"#
    );

    assert!(
        GrokBuildCredential::import_active_runtime(account.as_bytes(), OBSERVED_AT_MS).is_err()
    );
    let refreshable =
        GrokBuildCredential::import_refreshable_runtime(account.as_bytes(), OBSERVED_AT_MS)?;
    assert!(refreshable.is_expired_at(OBSERVED_AT_MS));
    assert_eq!(refreshable.refresh_token(), "still_refreshable");

    let excessively_stale = account.replace("2025-01-01", "2023-01-01");
    assert!(
        GrokBuildCredential::import_refreshable_runtime(
            excessively_stale.as_bytes(),
            OBSERVED_AT_MS
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn known_absolute_expiry_sources_reject_wrong_identity_and_unsafe_expiry()
-> Result<(), Box<dyn Error>> {
    const OBSERVED_AT_MS: i64 = 1_735_689_600_000;

    let wrong_client = GrokBuildCredential::import_grok_account_json(
        br#"{
            "access_token":"synthetic_account_access_012345",
            "refresh_token":"synthetic_account_refresh_012345",
            "expires_at":"2025-01-01T00:00:10Z",
            "client_id":"different-client"
        }"#,
        OBSERVED_AT_MS,
    )
    .err()
    .ok_or("wrong client unexpectedly imported")?;
    assert_eq!(wrong_client, GrokBuildOAuthError::CredentialClientMismatch);

    let wrong_issuer = GrokBuildCredential::import_grok_account_json(
        br#"{
            "access_token":"synthetic_account_access_012345",
            "refresh_token":"synthetic_account_refresh_012345",
            "expires_at":"2025-01-01T00:00:10Z",
            "issuer":"https://other.example.test"
        }"#,
        OBSERVED_AT_MS,
    )
    .err()
    .ok_or("wrong issuer unexpectedly imported")?;
    assert_eq!(wrong_issuer, GrokBuildOAuthError::CredentialIssuerMismatch);

    let expired = GrokBuildCredential::import_cpa_xai_auth_file(
        br#"{
            "type":"xai",
            "access_token":"synthetic_cpa_access_012345",
            "refresh_token":"synthetic_cpa_refresh_012345",
            "expired":"2025-01-01T00:00:00Z"
        }"#,
        OBSERVED_AT_MS,
    )
    .err()
    .ok_or("expired CPA source unexpectedly imported")?;
    assert_eq!(expired, GrokBuildOAuthError::AmbiguousExpiration);

    let duplicate_cli_entry = format!(
        r#"{{
            "{GROK_BUILD_OAUTH_ISSUER}::{GROK_BUILD_PUBLIC_CLIENT_ID}":{{
                "key":"synthetic_cli_access_012345",
                "refresh_token":"synthetic_cli_refresh_012345",
                "expires_at":"2025-01-01T00:00:10Z"
            }},
            "{GROK_BUILD_OAUTH_ISSUER}::{GROK_BUILD_PUBLIC_CLIENT_ID}":{{
                "key":"different_cli_access_012345",
                "refresh_token":"different_cli_refresh_012345",
                "expires_at":"2025-01-01T00:00:10Z"
            }}
        }}"#
    );
    let duplicate = GrokBuildCredential::import_official_cli_auth_cache(
        duplicate_cli_entry.as_bytes(),
        OBSERVED_AT_MS,
    )
    .err()
    .ok_or("duplicate official CLI cache entry unexpectedly imported")?;
    assert_eq!(duplicate, GrokBuildOAuthError::InvalidJson);

    let wrong_issuer_cache = format!(
        r#"{{
            "https://other.example.test::{GROK_BUILD_PUBLIC_CLIENT_ID}":{{
                "key":"synthetic_cli_access_012345",
                "refresh_token":"synthetic_cli_refresh_012345",
                "expires_at":"2025-01-01T00:00:10Z"
            }}
        }}"#
    );
    let wrong_cache_issuer = GrokBuildCredential::import_official_cli_auth_cache(
        wrong_issuer_cache.as_bytes(),
        OBSERVED_AT_MS,
    )
    .err()
    .ok_or("wrong official CLI cache issuer unexpectedly imported")?;
    assert_eq!(
        wrong_cache_issuer,
        GrokBuildOAuthError::CredentialIssuerMismatch
    );

    let wrong_client_cache = format!(
        r#"{{
            "{GROK_BUILD_OAUTH_ISSUER}::different-client":{{
                "key":"synthetic_cli_access_012345",
                "refresh_token":"synthetic_cli_refresh_012345",
                "expires_at":"2025-01-01T00:00:10Z"
            }}
        }}"#
    );
    let wrong_cache_client = GrokBuildCredential::import_official_cli_auth_cache(
        wrong_client_cache.as_bytes(),
        OBSERVED_AT_MS,
    )
    .err()
    .ok_or("wrong official CLI cache client unexpectedly imported")?;
    assert_eq!(
        wrong_cache_client,
        GrokBuildOAuthError::CredentialClientMismatch
    );
    Ok(())
}

#[test]
fn absolute_expiry_sources_reject_conflicts_and_out_of_range_expiries() -> Result<(), Box<dyn Error>>
{
    const OBSERVED_AT_MS: i64 = 1_735_689_600_000;

    let conflicting_expiry = GrokBuildCredential::import_cpa_xai_auth_file(
        br#"{
            "type":"xai",
            "access_token":"synthetic_cpa_access_012345",
            "refresh_token":"synthetic_cpa_refresh_012345",
            "expired":"2025-01-01T00:00:10Z",
            "expires_at":"2025-01-01T00:00:20Z"
        }"#,
        OBSERVED_AT_MS,
    )
    .err()
    .ok_or("conflicting absolute expiry unexpectedly imported")?;
    assert_eq!(conflicting_expiry, GrokBuildOAuthError::AmbiguousExpiration);

    let unknown_cpa_shape = GrokBuildCredential::import_cpa_xai_auth_file(
        br#"{
            "type":"unsupported-provider",
            "access_token":"synthetic_cpa_access_012345",
            "refresh_token":"synthetic_cpa_refresh_012345",
            "expired":"2025-01-01T00:00:10Z"
        }"#,
        OBSERVED_AT_MS,
    )
    .err()
    .ok_or("unknown CPA OAuth shape unexpectedly imported")?;
    assert_eq!(unknown_cpa_shape, GrokBuildOAuthError::InvalidField);

    let too_far_future = GrokBuildCredential::import_grok_account_json(
        br#"{
            "access_token":"synthetic_account_access_012345",
            "refresh_token":"synthetic_account_refresh_012345",
            "expires_at":"2027-01-02T00:00:00Z"
        }"#,
        OBSERVED_AT_MS,
    )
    .err()
    .ok_or("overlong absolute expiry unexpectedly imported")?;
    assert_eq!(too_far_future, GrokBuildOAuthError::AmbiguousExpiration);

    let absent_cli_entry = GrokBuildCredential::import_official_cli_auth_cache(
        br#"{"https://auth.example.test::other-client":{"ignored":true}}"#,
        OBSERVED_AT_MS,
    )
    .err()
    .ok_or("missing official CLI cache entry unexpectedly imported")?;
    assert_eq!(absent_cli_entry, GrokBuildOAuthError::MissingField);
    Ok(())
}

#[test]
fn importer_rejects_duplicate_unsafe_and_ambiguous_oauth_shapes() -> Result<(), Box<dyn Error>> {
    let duplicate = GrokBuildCredential::import_json(
        br#"{
            "access_token":"synthetic_access_012345",
            "access_token":"another_synthetic_access",
            "refresh_token":"synthetic_refresh_012345",
            "expires_in":3600
        }"#,
        0,
    )
    .err()
    .ok_or("duplicate field unexpectedly imported")?;
    assert_eq!(duplicate, GrokBuildOAuthError::InvalidJson);

    let nested_duplicate = GrokBuildCredential::import_json(
        br#"{
            "access_token":"synthetic_access_012345",
            "refresh_token":"synthetic_refresh_012345",
            "expires_in":3600,
            "id_token":{"safe":1,"safe":2}
        }"#,
        0,
    )
    .err()
    .ok_or("nested duplicate field unexpectedly imported")?;
    assert_eq!(nested_duplicate, GrokBuildOAuthError::InvalidJson);

    let whitespace = GrokBuildCredential::import_json(
        br#"{
            "access_token":" synthetic_access_012345",
            "refresh_token":"synthetic_refresh_012345",
            "expires_in":3600
        }"#,
        0,
    )
    .err()
    .ok_or("whitespace token unexpectedly imported")?;
    assert_eq!(whitespace, GrokBuildOAuthError::InvalidField);

    let ambiguous_expiry = GrokBuildCredential::import_json(
        br#"{
            "access_token":"synthetic_access_012345",
            "refresh_token":"synthetic_refresh_012345",
            "expires_in":3600,
            "expires_at":1700000000
        }"#,
        0,
    )
    .err()
    .ok_or("ambiguous expiry unexpectedly imported")?;
    assert_eq!(ambiguous_expiry, GrokBuildOAuthError::AmbiguousExpiration);

    let unsupported_type = GrokBuildCredential::import_json(
        br#"{
            "access_token":"synthetic_access_012345",
            "refresh_token":"synthetic_refresh_012345",
            "expires_in":3600,
            "token_type":"DPoP"
        }"#,
        0,
    )
    .err()
    .ok_or("unsupported token type unexpectedly imported")?;
    assert_eq!(unsupported_type, GrokBuildOAuthError::UnsupportedTokenType);

    let oversized = vec![b' '; 64 * 1024 + 1];
    let too_large = GrokBuildCredential::import_json(&oversized, 0)
        .err()
        .ok_or("oversized OAuth JSON unexpectedly imported")?;
    assert_eq!(too_large, GrokBuildOAuthError::InputTooLarge);
    Ok(())
}

#[test]
fn device_code_state_machine_honors_interval_slow_down_and_redaction() -> Result<(), Box<dyn Error>>
{
    let transport = ScriptedTransport::new(vec![
        response(
            200,
            br#"{
                "device_code":"synthetic_device_012345",
                "user_code":"SYNTH-1234",
                "verification_uri":"https://auth.example.test/verify",
                "verification_uri_complete":"https://auth.example.test/verify?code=SYNTH-1234",
                "expires_in":60,
                "interval":5
            }"#,
        )?,
        response(400, br#"{"error":"authorization_pending"}"#)?,
        response(400, br#"{"error":"slow_down"}"#)?,
        response(
            200,
            br#"{
                "access_token":"synthetic_granted_access",
                "refresh_token":"synthetic_granted_refresh",
                "expires_in":3600,
                "token_type":"Bearer"
            }"#,
        )?,
    ]);
    let flow = GrokBuildOAuthFlow::try_new("synthetic-device-client", "openid profile")?;
    let authorization = flow.start_device_authorization(&transport, 1_000)?;
    let authorization_debug = format!("{authorization:?}");
    for synthetic_secret in ["synthetic_device_012345", "SYNTH-1234"] {
        assert!(!authorization_debug.contains(synthetic_secret));
    }
    assert_eq!(authorization.interval_seconds(), 5);
    assert_eq!(authorization.expires_at_ms(), 61_000);

    let mut poller = provider_grok::GrokBuildDevicePoller::new(authorization);
    assert_eq!(poller.next_poll_at_ms(), 6_000);
    assert!(matches!(
        poller.poll(&transport, 5_999),
        Err(GrokBuildOAuthError::PollingTooSoon)
    ));
    assert!(matches!(
        poller.poll(&transport, 6_000)?,
        GrokBuildDevicePollOutcome::Pending {
            retry_at_ms: 11_000
        }
    ));
    assert!(matches!(
        poller.poll(&transport, 11_000)?,
        GrokBuildDevicePollOutcome::SlowDown {
            retry_at_ms: 21_000
        }
    ));
    match poller.poll(&transport, 21_000)? {
        GrokBuildDevicePollOutcome::Granted(credential) => {
            assert_eq!(credential.access_token(), "synthetic_granted_access");
            assert_eq!(credential.source(), GrokBuildCredentialSource::DeviceCode);
            assert_eq!(credential.client_id(), "synthetic-device-client");
            assert_eq!(credential.scope(), "openid profile");
        }
        GrokBuildDevicePollOutcome::Pending { .. }
        | GrokBuildDevicePollOutcome::SlowDown { .. }
        | GrokBuildDevicePollOutcome::Denied
        | GrokBuildDevicePollOutcome::Expired => {
            return Err("Device Code flow did not grant the scripted credential".into());
        }
    }
    assert!(matches!(
        poller.poll(&transport, 21_001),
        Err(GrokBuildOAuthError::DeviceFlowCompleted)
    ));
    assert_eq!(
        transport.observed()?,
        vec![
            (
                GrokBuildOAuthEndpoint::DeviceAuthorization,
                GrokBuildOAuthRequestKind::DeviceAuthorization,
            ),
            (
                GrokBuildOAuthEndpoint::Token,
                GrokBuildOAuthRequestKind::DevicePoll
            ),
            (
                GrokBuildOAuthEndpoint::Token,
                GrokBuildOAuthRequestKind::DevicePoll
            ),
            (
                GrokBuildOAuthEndpoint::Token,
                GrokBuildOAuthRequestKind::DevicePoll
            ),
        ]
    );
    assert!(transport.debug_output_is_redacted()?);
    Ok(())
}

#[test]
fn refresh_requires_a_matching_client_and_never_exposes_form_secrets() -> Result<(), Box<dyn Error>>
{
    let credential = GrokBuildCredential::import_json(
        br#"{
            "access_token":"synthetic_access_012345",
            "refresh_token":"synthetic_refresh_012345",
            "expires_in":3600,
            "client_id":"client-a"
        }"#,
        0,
    )?;
    let mismatch = GrokBuildOAuthFlow::try_new("client-b", "openid")?;
    let no_calls = ScriptedTransport::new(Vec::new());
    assert!(matches!(
        mismatch.refresh(&no_calls, &credential, 1_000),
        Err(GrokBuildOAuthError::CredentialClientMismatch)
    ));
    assert!(no_calls.observed()?.is_empty());

    let transport = ScriptedTransport::new(vec![response(
        200,
        br#"{
            "access_token":"synthetic_refreshed_access",
            "refresh_token":"synthetic_refreshed_refresh",
            "expires_in":7200,
            "token_type":"bearer"
        }"#,
    )?]);
    let matching = GrokBuildOAuthFlow::try_new("client-a", "openid")?;
    let refreshed = matching.refresh(&transport, &credential, 1_000)?;
    assert_eq!(refreshed.source(), GrokBuildCredentialSource::Refresh);
    assert_eq!(refreshed.expires_at_ms(), 7_201_000);
    assert_eq!(
        transport.observed()?,
        vec![(
            GrokBuildOAuthEndpoint::Token,
            GrokBuildOAuthRequestKind::Refresh
        )]
    );
    assert!(transport.debug_output_is_redacted()?);
    Ok(())
}

struct ScriptedTransport {
    responses: Mutex<VecDeque<GrokBuildOAuthHttpResponse>>,
    observed: Mutex<Vec<(GrokBuildOAuthEndpoint, GrokBuildOAuthRequestKind)>>,
    debug_output: Mutex<Vec<String>>,
}

impl ScriptedTransport {
    fn new(responses: Vec<GrokBuildOAuthHttpResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
            observed: Mutex::new(Vec::new()),
            debug_output: Mutex::new(Vec::new()),
        }
    }

    fn observed(
        &self,
    ) -> Result<
        Vec<(GrokBuildOAuthEndpoint, GrokBuildOAuthRequestKind)>,
        GrokBuildOAuthTransportError,
    > {
        self.observed
            .lock()
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)
            .map(|observed| observed.clone())
    }

    fn debug_output_is_redacted(&self) -> Result<bool, GrokBuildOAuthTransportError> {
        self.debug_output
            .lock()
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)
            .map(|debug_output| {
                debug_output.iter().all(|entry| {
                    entry.contains("<redacted>")
                        && !entry.contains("synthetic_device_012345")
                        && !entry.contains("synthetic_refresh_012345")
                })
            })
    }
}

impl GrokBuildOAuthTransport for ScriptedTransport {
    fn send(
        &self,
        request: GrokBuildOAuthRequest,
    ) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthTransportError> {
        let endpoint = request.endpoint();
        let kind = request.kind();
        let debug = format!("{request:?}");
        self.observed
            .lock()
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?
            .push((endpoint, kind));
        self.debug_output
            .lock()
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?
            .push(debug);
        self.responses
            .lock()
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?
            .pop_front()
            .ok_or(GrokBuildOAuthTransportError::Unavailable)
    }
}

fn response(status: u16, body: &[u8]) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthError> {
    GrokBuildOAuthHttpResponse::try_new(status, body.to_vec())
}
