//! P6-01 synthetic Grok Build OAuth import and Device Code evidence.

use std::{collections::VecDeque, error::Error, sync::Mutex};

use provider_grok::{
    GROK_BUILD_PUBLIC_CLIENT_ID, GrokBuildCredential, GrokBuildCredentialSource,
    GrokBuildDevicePollOutcome, GrokBuildOAuthEndpoint, GrokBuildOAuthError, GrokBuildOAuthFlow,
    GrokBuildOAuthHttpResponse, GrokBuildOAuthRequest, GrokBuildOAuthRequestKind,
    GrokBuildOAuthTransport, GrokBuildOAuthTransportError,
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
