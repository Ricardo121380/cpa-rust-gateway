//! Opt-in P3-10 validation against two separately authorized real test relays.
//!
//! The live test is ignored by default and cannot construct an outbound request without an
//! explicit `P3_10_LIVE_AUTHORIZATION=approved` value plus a complete, fixed-budget configuration.
//! It never reads generic provider variables or a `.env` file.

#![deny(unsafe_code)]

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fmt,
    sync::Arc,
    time::Duration,
};

use actix_web::{
    App,
    body::{self, MessageBody},
    dev::ServiceResponse,
    http::{StatusCode, header},
    test as actix_test, web,
};
use gateway_core::{
    AttemptOutcome, AttemptRetryDecision, EgressPolicyId, GatewayEvent, GatewayEventSink, RequestId,
};
use gateway_http_actix::configure;
use gateway_observability::{BoundedEventQueue, EventQueueConfig, EventQueueReceiver};
use gateway_upstream::{
    EgressCidr, EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy,
    SystemEgressDnsResolver, UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};
use provider_openai_compatible::{
    OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesRequestBuilder,
};
use support::p3_aggregation::{AggregationEndpoint, RequestIdMode, build_aggregation_harness};
use url::Url;
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn Error>>;

const PUBLIC_MODEL: &str = "minimax-m3";
const MODEL_ALIAS: &str = "minimax-m3-alias";
const LIVE_REQUEST_CAP: u32 = 4;
const PROBE_MAX_OUTPUT_TOKENS: u64 = 32;
const MAX_CLIENT_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveConfigError {
    NotAuthorized,
    MissingRequiredValue,
    InvalidRequestCap,
    InvalidNetworkProfile,
    UnexpectedProxyValue,
    InvalidProxy,
    InvalidEndpoint,
    InvalidCredential,
    InvalidAllowedCidr,
    InvalidEgressPolicy,
    InvalidTimeouts,
}

impl fmt::Display for LiveConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotAuthorized => "P3-10 live validation is not explicitly authorized",
            Self::MissingRequiredValue => "P3-10 live validation configuration is incomplete",
            Self::InvalidRequestCap => "P3-10 live validation must use its fixed request cap",
            Self::InvalidNetworkProfile => "P3-10 network profile is invalid",
            Self::UnexpectedProxyValue => "P3-10 direct profile cannot retain a proxy value",
            Self::InvalidProxy => "P3-10 SOCKS5 proxy is invalid",
            Self::InvalidEndpoint => "P3-10 Endpoint configuration is invalid",
            Self::InvalidCredential => "P3-10 test credential is invalid",
            Self::InvalidAllowedCidr => "P3-10 explicit CIDR configuration is invalid",
            Self::InvalidEgressPolicy => "P3-10 egress policy construction failed",
            Self::InvalidTimeouts => "P3-10 transport timeout construction failed",
        };
        formatter.write_str(message)
    }
}

impl Error for LiveConfigError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeError {
    GatewayResponseNotSuccessful(&'static str),
    ClientBodyTooLarge,
    ClientBodyReadFailed,
    ClientBodyNotUtf8,
    PublicModelNotRewritten,
    SensitiveValueLeaked,
    InvalidClientResponse,
    MissingPublicSseModel,
    MissingRequestEvent,
    MissingAttemptEvent,
    MissingUsageEvent,
    UnexpectedEvent,
    EventCorrelationFailed,
    CandidateSelectionFailed,
    RouteHandoffFailed,
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::GatewayResponseNotSuccessful(status_class) => {
                return write!(
                    formatter,
                    "gateway returned a non-success public response ({status_class})"
                );
            }
            Self::ClientBodyTooLarge => "gateway response exceeded the P3-10 bounded read limit",
            Self::ClientBodyReadFailed => "gateway response body could not be read safely",
            Self::ClientBodyNotUtf8 => "gateway response body was not valid UTF-8",
            Self::PublicModelNotRewritten => "gateway response did not retain the public model",
            Self::SensitiveValueLeaked => "gateway response exposed an upstream-only value",
            Self::InvalidClientResponse => "gateway response shape was not valid for P3-10",
            Self::MissingPublicSseModel => "gateway SSE response had no public response model",
            Self::MissingRequestEvent => "P3-10 probe emitted no Request event",
            Self::MissingAttemptEvent => "P3-10 probe emitted no terminal Attempt event",
            Self::MissingUsageEvent => "P3-10 probe emitted no Usage event",
            Self::UnexpectedEvent => "P3-10 probe emitted an unexpected event shape",
            Self::EventCorrelationFailed => "P3-10 event correlation did not remain request-scoped",
            Self::CandidateSelectionFailed => "P3-10 did not use the expected explicit Candidate",
            Self::RouteHandoffFailed => "P3-10 routed execution did not retain its Route identity",
        };
        formatter.write_str(message)
    }
}

impl Error for ProbeError {}

struct EndpointConfig {
    label: &'static str,
    base_url: String,
    credential: Zeroizing<String>,
    upstream_model: String,
    allowed_cidr: Option<EgressCidr>,
}

struct ProbePrivacy {
    base_url: String,
    credential: Zeroizing<String>,
    upstream_model: String,
}

struct LiveConfig {
    endpoints: [EndpointConfig; 2],
    proxy: UpstreamProxy,
}

impl LiveConfig {
    fn from_environment() -> Result<Self, LiveConfigError> {
        Self::from_values(&mut |name| env::var(name).ok())
    }

    fn from_values<F>(read: &mut F) -> Result<Self, LiveConfigError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        if read("P3_10_LIVE_AUTHORIZATION").as_deref() != Some("approved") {
            return Err(LiveConfigError::NotAuthorized);
        }
        let request_cap = required_value(read, "P3_10_MAX_EXTERNAL_REQUESTS")?
            .parse::<u32>()
            .map_err(|_| LiveConfigError::InvalidRequestCap)?;
        if request_cap != LIVE_REQUEST_CAP {
            return Err(LiveConfigError::InvalidRequestCap);
        }

        let proxy = match required_value(read, "P3_10_NETWORK_PROFILE")?.as_str() {
            "direct" => {
                if read("P3_10_SOCKS5_PROXY_URL").is_some() {
                    return Err(LiveConfigError::UnexpectedProxyValue);
                }
                UpstreamProxy::Direct
            }
            "socks5" => UpstreamProxy::try_socks5(&required_value(read, "P3_10_SOCKS5_PROXY_URL")?)
                .map_err(|_| LiveConfigError::InvalidProxy)?,
            _ => return Err(LiveConfigError::InvalidNetworkProfile),
        };

        Ok(Self {
            endpoints: [
                read_endpoint(read, "A", "a")?,
                read_endpoint(read, "B", "b")?,
            ],
            proxy,
        })
    }

    fn into_harness_inputs(
        self,
    ) -> Result<(Vec<AggregationEndpoint>, Vec<ProbePrivacy>), LiveConfigError> {
        let proxy = self.proxy;
        let mut inputs = Vec::with_capacity(self.endpoints.len());
        let mut privacy = Vec::with_capacity(self.endpoints.len());
        for endpoint in self.endpoints {
            let (input, probe_privacy) = endpoint.into_harness_input(proxy.clone())?;
            inputs.push(input);
            privacy.push(probe_privacy);
        }
        Ok((inputs, privacy))
    }
}

impl EndpointConfig {
    fn into_harness_input(
        self,
        proxy: UpstreamProxy,
    ) -> Result<(AggregationEndpoint, ProbePrivacy), LiveConfigError> {
        let endpoint = OpenAiResponsesEndpoint::try_new(&self.base_url, "/responses")
            .map_err(|_| LiveConfigError::InvalidEndpoint)?;
        let parsed = Url::parse(endpoint.url()).map_err(|_| LiveConfigError::InvalidEndpoint)?;
        let scheme = EgressScheme::try_from_url_scheme(parsed.scheme())
            .map_err(|_| LiveConfigError::InvalidEndpoint)?;
        let host = parsed
            .host_str()
            .ok_or(LiveConfigError::InvalidEndpoint)
            .and_then(|value| {
                EgressHost::try_new(value).map_err(|_| LiveConfigError::InvalidEndpoint)
            })?;
        let port = parsed
            .port_or_known_default()
            .ok_or(LiveConfigError::InvalidEndpoint)?;
        let allowed_cidrs = self.allowed_cidr.into_iter().collect::<BTreeSet<_>>();
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new(format!("p3-10-egress-{}", self.label))
                .map_err(|_| LiveConfigError::InvalidEgressPolicy)?,
            name: "P3-10 authorized real-test policy".to_owned(),
            allowed_schemes: BTreeSet::from([scheme]),
            allowed_hosts: BTreeSet::from([host]),
            allowed_ports: BTreeSet::from([port]),
            allowed_cidrs,
            redirect_policy: RedirectPolicy::Deny,
        })
        .map_err(|_| LiveConfigError::InvalidEgressPolicy)?;
        let timeouts = UpstreamTimeouts::try_new(
            Duration::from_secs(5),
            Duration::from_secs(15),
            Duration::from_secs(20),
            Duration::from_secs(45),
        )
        .map_err(|_| LiveConfigError::InvalidTimeouts)?;
        let credential = self.credential.as_bytes().to_vec();
        let probe_privacy = ProbePrivacy {
            base_url: self.base_url,
            credential: self.credential,
            upstream_model: self.upstream_model.clone(),
        };
        let input = AggregationEndpoint::new(
            self.label.to_owned(),
            endpoint,
            self.upstream_model,
            credential,
            policy,
            Arc::new(SystemEgressDnsResolver),
            UpstreamTransportProfile::new(
                timeouts,
                proxy,
                std::num::NonZeroUsize::new(1).ok_or(LiveConfigError::InvalidTimeouts)?,
            ),
        );
        Ok((input, probe_privacy))
    }
}

fn required_value<F>(read: &mut F, name: &str) -> Result<String, LiveConfigError>
where
    F: FnMut(&str) -> Option<String>,
{
    read(name)
        .filter(|value| !value.trim().is_empty())
        .ok_or(LiveConfigError::MissingRequiredValue)
}

fn read_endpoint<F>(
    read: &mut F,
    configuration_label: &str,
    endpoint_label: &'static str,
) -> Result<EndpointConfig, LiveConfigError>
where
    F: FnMut(&str) -> Option<String>,
{
    let prefix = format!("P3_10_ENDPOINT_{configuration_label}");
    let base_url = required_value(read, &format!("{prefix}_BASE_URL"))?;
    let credential = Zeroizing::new(required_value(read, &format!("{prefix}_API_KEY"))?);
    if !credential.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(LiveConfigError::InvalidCredential);
    }
    let upstream_model = required_value(read, &format!("{prefix}_UPSTREAM_MODEL"))?;
    let allowed_cidr = match read(&format!("{prefix}_ALLOWED_CIDR")) {
        Some(value) if value.trim().is_empty() => return Err(LiveConfigError::InvalidAllowedCidr),
        Some(value) => {
            Some(EgressCidr::try_parse(&value).map_err(|_| LiveConfigError::InvalidAllowedCidr)?)
        }
        None => None,
    };
    Ok(EndpointConfig {
        label: endpoint_label,
        base_url,
        credential,
        upstream_model,
        allowed_cidr,
    })
}

fn authorized(request: actix_test::TestRequest, presented_key: &str) -> actix_test::TestRequest {
    request.insert_header((header::AUTHORIZATION, format!("Bearer {presented_key}")))
}

fn probe_payload(streaming: bool) -> String {
    format!(
        r#"{{"model":"{MODEL_ALIAS}","input":"Reply with exactly: ready","max_output_tokens":{PROBE_MAX_OUTPUT_TOKENS},"stream":{streaming}}}"#
    )
}

async fn bounded_client_body<B>(response: ServiceResponse<B>) -> Result<Vec<u8>, ProbeError>
where
    B: MessageBody,
{
    let body = body::to_bytes_limited(response.into_body(), MAX_CLIENT_RESPONSE_BYTES)
        .await
        .map_err(|_| ProbeError::ClientBodyTooLarge)?
        .map_err(|_| ProbeError::ClientBodyReadFailed)?;
    Ok(body.to_vec())
}

fn verify_client_visible_boundary(
    body: &[u8],
    streaming: bool,
    privacy: &ProbePrivacy,
    presented_key: &str,
) -> Result<(), ProbeError> {
    if contains_bytes(body, privacy.base_url.as_bytes())
        || contains_bytes(body, privacy.credential.as_bytes())
        || contains_bytes(body, privacy.upstream_model.as_bytes())
        || contains_bytes(body, presented_key.as_bytes())
    {
        return Err(ProbeError::SensitiveValueLeaked);
    }
    if streaming {
        verify_sse_public_model(body)
    } else {
        let value: serde_json::Value =
            serde_json::from_slice(body).map_err(|_| ProbeError::InvalidClientResponse)?;
        if value.get("model").and_then(serde_json::Value::as_str) != Some(PUBLIC_MODEL) {
            return Err(ProbeError::PublicModelNotRewritten);
        }
        Ok(())
    }
}

fn verify_sse_public_model(body: &[u8]) -> Result<(), ProbeError> {
    let text = std::str::from_utf8(body).map_err(|_| ProbeError::ClientBodyNotUtf8)?;
    let mut saw_response_model = false;
    for frame in text.split("\n\n") {
        let data = frame
            .lines()
            .filter_map(|line| line.strip_prefix("data:").map(str::trim))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(&data).map_err(|_| ProbeError::InvalidClientResponse)?;
        let Some(model) = value
            .get("response")
            .and_then(|response| response.get("model"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if model != PUBLIC_MODEL {
            return Err(ProbeError::PublicModelNotRewritten);
        }
        saw_response_model = true;
    }
    saw_response_model
        .then_some(())
        .ok_or(ProbeError::MissingPublicSseModel)
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn safe_status_class(status: StatusCode) -> &'static str {
    if status.is_informational() {
        "1xx"
    } else if status.is_redirection() {
        "3xx"
    } else if status.is_client_error() {
        "4xx"
    } else if status.is_server_error() {
        "5xx"
    } else {
        "other"
    }
}

fn verify_events(
    receiver: &mut EventQueueReceiver,
    streaming: bool,
    expected_candidate_label: &str,
) -> Result<(), ProbeError> {
    let mut request_events = 0_usize;
    let mut attempt_events = 0_usize;
    let mut usage_events = 0_usize;
    let mut request_id: Option<RequestId> = None;
    let expected_candidate = format!("p3-10-candidate-{expected_candidate_label}");
    while let Some(event) = receiver.try_recv() {
        match event {
            GatewayEvent::Request(event) => {
                request_events += 1;
                if event.public_model() != PUBLIC_MODEL
                    || event.route_alias() != Some(MODEL_ALIAS)
                    || event.streaming() != streaming
                {
                    return Err(ProbeError::EventCorrelationFailed);
                }
                request_id = Some(event.request_id().clone());
            }
            GatewayEvent::Attempt(event) => {
                attempt_events += 1;
                if request_id.as_ref() != Some(event.request_id())
                    || event.attempt_number() != 1
                    || event.route_candidate_id().as_str() != expected_candidate
                    || !matches!(event.outcome(), AttemptOutcome::Succeeded)
                    || event.retry_decision() != AttemptRetryDecision::Completed
                {
                    return Err(ProbeError::CandidateSelectionFailed);
                }
            }
            GatewayEvent::Usage(event) => {
                usage_events += 1;
                if request_id.as_ref() != Some(event.request_id()) {
                    return Err(ProbeError::EventCorrelationFailed);
                }
            }
            GatewayEvent::Diagnostic(_) => return Err(ProbeError::UnexpectedEvent),
        }
    }
    match (request_events, attempt_events, usage_events) {
        (0, _, _) => Err(ProbeError::MissingRequestEvent),
        (_, 0, _) => Err(ProbeError::MissingAttemptEvent),
        (_, _, 0) => Err(ProbeError::MissingUsageEvent),
        (1, 1, 1) => Ok(()),
        _ => Err(ProbeError::UnexpectedEvent),
    }
}

fn synthetic_values(request_cap: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("P3_10_LIVE_AUTHORIZATION".to_owned(), "approved".to_owned()),
        (
            "P3_10_MAX_EXTERNAL_REQUESTS".to_owned(),
            request_cap.to_owned(),
        ),
        ("P3_10_NETWORK_PROFILE".to_owned(), "direct".to_owned()),
        (
            "P3_10_ENDPOINT_A_BASE_URL".to_owned(),
            "https://p3-10-a.invalid/v1".to_owned(),
        ),
        (
            "P3_10_ENDPOINT_A_API_KEY".to_owned(),
            "synthetic-credential-a".to_owned(),
        ),
        (
            "P3_10_ENDPOINT_A_UPSTREAM_MODEL".to_owned(),
            "synthetic-model-a".to_owned(),
        ),
        (
            "P3_10_ENDPOINT_B_BASE_URL".to_owned(),
            "https://p3-10-b.invalid/v1".to_owned(),
        ),
        (
            "P3_10_ENDPOINT_B_API_KEY".to_owned(),
            "synthetic-credential-b".to_owned(),
        ),
        (
            "P3_10_ENDPOINT_B_UPSTREAM_MODEL".to_owned(),
            "synthetic-model-b".to_owned(),
        ),
    ])
}

#[test]
fn missing_explicit_authorization_stops_before_endpoint_setup() {
    let mut read = |_name: &str| None;
    assert!(matches!(
        LiveConfig::from_values(&mut read),
        Err(LiveConfigError::NotAuthorized)
    ));
}

#[test]
fn live_configuration_requires_the_fixed_four_request_budget() {
    let values = synthetic_values("3");
    let mut read = |name: &str| values.get(name).cloned();
    assert!(matches!(
        LiveConfig::from_values(&mut read),
        Err(LiveConfigError::InvalidRequestCap)
    ));
}

#[test]
fn complete_synthetic_configuration_builds_without_network_activity() -> TestResult {
    let values = synthetic_values("4");
    let mut read = |name: &str| values.get(name).cloned();
    let config = LiveConfig::from_values(&mut read)?;
    let (inputs, privacy) = config.into_harness_inputs()?;
    assert_eq!(inputs.len(), 2);
    assert_eq!(privacy.len(), 2);
    Ok(())
}

#[test]
fn direct_profile_rejects_a_leftover_proxy_value() {
    let mut values = synthetic_values("4");
    values.insert(
        "P3_10_SOCKS5_PROXY_URL".to_owned(),
        "socks5://127.0.0.1:7891".to_owned(),
    );
    let mut read = |name: &str| values.get(name).cloned();
    assert!(matches!(
        LiveConfig::from_values(&mut read),
        Err(LiveConfigError::UnexpectedProxyValue)
    ));
}

#[test]
fn socks5_profile_is_explicit_and_parsed_without_network_activity() -> TestResult {
    let mut values = synthetic_values("4");
    values.insert("P3_10_NETWORK_PROFILE".to_owned(), "socks5".to_owned());
    values.insert(
        "P3_10_SOCKS5_PROXY_URL".to_owned(),
        "socks5://127.0.0.1:7891".to_owned(),
    );
    values.insert(
        "P3_10_ENDPOINT_A_ALLOWED_CIDR".to_owned(),
        "127.0.0.1/32".to_owned(),
    );
    let mut read = |name: &str| values.get(name).cloned();
    let config = LiveConfig::from_values(&mut read)?;
    let (inputs, privacy) = config.into_harness_inputs()?;
    assert_eq!(inputs.len(), 2);
    assert_eq!(privacy.len(), 2);
    Ok(())
}

#[test]
fn client_boundary_rejects_internal_values_without_rendering_them() {
    let privacy = ProbePrivacy {
        base_url: "https://test-relay.invalid/v1".to_owned(),
        credential: Zeroizing::new("synthetic-credential".to_owned()),
        upstream_model: "synthetic-upstream-model".to_owned(),
    };
    assert!(matches!(
        verify_client_visible_boundary(
            br#"{"model":"minimax-m3","marker":"synthetic-upstream-model"}"#,
            false,
            &privacy,
            "synthetic-client-key",
        ),
        Err(ProbeError::SensitiveValueLeaked)
    ));
}

#[test]
fn fixed_probe_payload_caps_the_upstream_output() -> TestResult {
    for streaming in [false, true] {
        let payload = probe_payload(streaming);
        let decoded = protocol_openai_responses::decode_request(&payload)?;
        let endpoint = OpenAiResponsesEndpoint::try_new("https://p3-10.invalid/v1", "/responses")?;
        let credential = OpenAiResponsesApiKey::try_new("synthetic-p3-10-credential")?;
        let outbound = OpenAiResponsesRequestBuilder::build(
            &endpoint,
            &credential,
            "synthetic-p3-10-model",
            &decoded.request,
            decoded.mode,
        )?;
        let payload: serde_json::Value = serde_json::from_slice(outbound.body())?;
        assert_eq!(
            payload
                .get("max_output_tokens")
                .and_then(serde_json::Value::as_u64),
            Some(PROBE_MAX_OUTPUT_TOKENS)
        );
        assert_eq!(
            payload.get("stream").and_then(serde_json::Value::as_bool),
            Some(streaming)
        );
    }
    Ok(())
}

#[test]
fn non_success_statuses_are_classified_without_rendering_codes() {
    assert_eq!(safe_status_class(StatusCode::CONTINUE), "1xx");
    assert_eq!(safe_status_class(StatusCode::MULTIPLE_CHOICES), "3xx");
    assert_eq!(safe_status_class(StatusCode::UNAUTHORIZED), "4xx");
    assert_eq!(safe_status_class(StatusCode::BAD_GATEWAY), "5xx");
}

#[actix_web::test]
#[ignore = "requires explicit P3_10_* real-test configuration and user authorization"]
async fn authorized_real_endpoints_validate_non_streaming_and_sse_paths() -> TestResult {
    let config = LiveConfig::from_environment()?;
    let (inputs, privacy) = config.into_harness_inputs()?;
    let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(16, 1)?)?;
    let event_sink: Arc<dyn GatewayEventSink> = Arc::new(queue);
    let harness = build_aggregation_harness(
        "p3-10",
        PUBLIC_MODEL,
        MODEL_ALIAS,
        RequestIdMode::Sequenced,
        1,
        inputs,
        event_sink,
    )?;
    let observed_routes = harness.observed_routes();
    let presented_key = harness.presented_key().to_owned();
    let app = actix_test::init_service(
        App::new()
            .app_data(web::Data::new(harness.state()))
            .configure(configure),
    )
    .await;

    for (streaming, expected_endpoint_index, endpoint_label) in [
        (false, 0_usize, "a"),
        (false, 1, "b"),
        (true, 0, "a"),
        (true, 1, "b"),
    ] {
        let mode = if streaming { "sse" } else { "non_streaming" };
        println!("p3-10 live probe target={endpoint_label} mode={mode} result=started");
        let request = authorized(
            actix_test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(probe_payload(streaming)),
            &presented_key,
        )
        .to_request();
        let response = actix_test::call_service(&app, request).await;
        let status = response.status();
        if !status.is_success() {
            let status_class = safe_status_class(status);
            println!(
                "p3-10 live probe target={endpoint_label} mode={mode} result=stopped status_class={status_class}"
            );
            return Err(ProbeError::GatewayResponseNotSuccessful(status_class).into());
        }
        let body = bounded_client_body(response).await?;
        verify_client_visible_boundary(
            &body,
            streaming,
            &privacy[expected_endpoint_index],
            &presented_key,
        )?;
        verify_events(&mut receiver, streaming, endpoint_label)?;
        println!("p3-10 live probe target={endpoint_label} mode={mode} result=pass");
    }

    let observed = observed_routes.lock().await;
    if observed.as_slice() != ["p3-10-route"; LIVE_REQUEST_CAP as usize] {
        return Err(ProbeError::RouteHandoffFailed.into());
    }
    Ok(())
}
