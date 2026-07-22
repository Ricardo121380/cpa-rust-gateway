//! One deliberately narrow, opt-in Grok Build test-account probe.
//!
//! This target is ignored by default. It neither reads generic provider variables nor constructs
//! egress/transport state until the dedicated authorization and fixed one-request cap are present.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fmt,
    num::NonZeroUsize,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gateway_core::{CanonicalEvent, EgressPolicyId};
use gateway_upstream::{
    EgressCidr, EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy,
    SystemEgressDnsResolver, UpstreamClientPool, UpstreamHttpResponse, UpstreamProxy,
    UpstreamTimeouts, UpstreamTransportProfile,
};
use protocol_openai_responses::{ResponseMode, decode_request};
use provider_grok::{
    GROK_BUILD_RESPONSES_URL, GrokBuildCredential, GrokBuildResponsesDecoder,
    GrokBuildResponsesHttpError, GrokBuildResponsesOutboundRequest,
    GrokBuildResponsesRequestBuilder, GrokBuildResponsesStreamDecoder,
    MAX_GROK_BUILD_ERROR_BODY_BYTES, MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES,
};
use url::Url;
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn Error>>;

const AUTHORIZATION_ENV: &str = "P6_03_LIVE_AUTHORIZATION";
const AUTHORIZATION_VALUE: &str = "single-probe-approved";
const REQUEST_CAP_ENV: &str = "P6_03_MAX_EXTERNAL_REQUESTS";
const TARGET_LABEL_ENV: &str = "P6_03_TARGET_LABEL";
const MODE_ENV: &str = "P6_03_MODE";
const OAUTH_CREDENTIAL_JSON_ENV: &str = "P6_03_OAUTH_CREDENTIAL_JSON";
const UPSTREAM_MODEL_ENV: &str = "P6_03_UPSTREAM_MODEL";
const NETWORK_PROFILE_ENV: &str = "P6_03_NETWORK_PROFILE";
const SOCKS5_PROXY_ENV: &str = "P6_03_SOCKS5_PROXY_URL";
const ALLOWED_CIDR_ENV: &str = "P6_03_ALLOWED_CIDR";

const EXTERNAL_REQUEST_CAP: u8 = 1;
const PROBE_MAX_OUTPUT_TOKENS: u64 = 32;
const MAX_GROK_BUILD_PROBE_STREAM_BYTES: usize = 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TTFB_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(20);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeMode {
    NonStreaming,
    Sse,
}

impl ProbeMode {
    fn parse(value: &str) -> Result<Self, ProbeConfigError> {
        match value {
            "non_streaming" => Ok(Self::NonStreaming),
            "sse" => Ok(Self::Sse),
            _ => Err(ProbeConfigError::InvalidMode),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::NonStreaming => "non_streaming",
            Self::Sse => "sse",
        }
    }

    const fn response_mode(self) -> ResponseMode {
        match self {
            Self::NonStreaming => ResponseMode::NonStreaming,
            Self::Sse => ResponseMode::Streaming,
        }
    }

    const fn expected_content_type(self) -> &'static str {
        match self {
            Self::NonStreaming => "application/json",
            Self::Sse => "text/event-stream",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeConfigError {
    NotAuthorized,
    MissingRequiredValue,
    InvalidRequestCap,
    InvalidTargetLabel,
    InvalidMode,
    InvalidNetworkProfile,
    UnexpectedProxyValue,
    InvalidProxy,
    InvalidCredential,
    InvalidUpstreamModel,
    InvalidAllowedCidr,
    InvalidEndpoint,
    InvalidEgressPolicy,
    InvalidTimeouts,
    InvalidProbePayload,
    SystemClock,
}

impl fmt::Display for ProbeConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotAuthorized => "P6-03 Build probe is not explicitly authorized",
            Self::MissingRequiredValue => "P6-03 Build probe configuration is incomplete",
            Self::InvalidRequestCap => "P6-03 Build probe must use its fixed one-request cap",
            Self::InvalidTargetLabel => "P6-03 Build probe target label is not opaque",
            Self::InvalidMode => "P6-03 Build probe mode is invalid",
            Self::InvalidNetworkProfile => "P6-03 Build probe network profile is invalid",
            Self::UnexpectedProxyValue => {
                "P6-03 Build probe direct profile cannot retain a proxy value"
            }
            Self::InvalidProxy => "P6-03 Build probe SOCKS5 proxy is invalid",
            Self::InvalidCredential => "P6-03 Build probe OAuth credential is invalid or expired",
            Self::InvalidUpstreamModel => "P6-03 Build probe upstream model is invalid",
            Self::InvalidAllowedCidr => "P6-03 Build probe CIDR is invalid",
            Self::InvalidEndpoint => "P6-03 Build probe fixed endpoint is invalid",
            Self::InvalidEgressPolicy => "P6-03 Build probe egress policy is invalid",
            Self::InvalidTimeouts => "P6-03 Build probe timeout profile is invalid",
            Self::InvalidProbePayload => "P6-03 Build probe payload is invalid",
            Self::SystemClock => "P6-03 Build probe clock is unavailable",
        };
        formatter.write_str(message)
    }
}

impl Error for ProbeConfigError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeError {
    EgressAdmissionFailed,
    TransportFailed,
    NonSuccessStatus(&'static str),
    UnexpectedContentType,
    ResponseReadFailed,
    ResponseTooLarge,
    ResponseProtocolFailed,
    MissingExpectedSemanticResponse,
    InternalInvariant,
}

/// A response-body output category that carries no upstream field values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SafeBodyShape {
    ErrorLikeObject,
    ResponseLikeObject,
    OtherObject,
    Array,
    Scalar,
    InvalidJson,
}

impl SafeBodyShape {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ErrorLikeObject => "error_like_object",
            Self::ResponseLikeObject => "response_like_object",
            Self::OtherObject => "other_object",
            Self::Array => "array",
            Self::Scalar => "scalar",
            Self::InvalidJson => "invalid_json",
        }
    }
}

impl ProbeError {
    const fn safe_outcome(self) -> &'static str {
        match self {
            Self::EgressAdmissionFailed => "egress_admission_failed",
            Self::TransportFailed => "transport_failed",
            Self::NonSuccessStatus(status_class) => status_class,
            Self::UnexpectedContentType => "unexpected_content_type",
            Self::ResponseReadFailed => "response_read_failed",
            Self::ResponseTooLarge => "response_too_large",
            Self::ResponseProtocolFailed => "response_protocol_failed",
            Self::MissingExpectedSemanticResponse => "missing_expected_semantic_response",
            Self::InternalInvariant => "internal_invariant",
        }
    }
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "P6-03 Build probe stopped: {}",
            self.safe_outcome()
        )
    }
}

impl Error for ProbeError {}

struct AuthorizedProbeConfig {
    target_label: String,
    mode: ProbeMode,
    credential_json: Zeroizing<String>,
    upstream_model: String,
    allowed_cidr: Option<EgressCidr>,
    proxy: UpstreamProxy,
}

impl AuthorizedProbeConfig {
    fn from_environment() -> Result<Self, ProbeConfigError> {
        Self::from_values(&mut |name| env::var(name).ok())
    }

    fn from_values<F>(read: &mut F) -> Result<Self, ProbeConfigError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        if read(AUTHORIZATION_ENV).as_deref() != Some(AUTHORIZATION_VALUE) {
            return Err(ProbeConfigError::NotAuthorized);
        }
        let request_cap = required_value(read, REQUEST_CAP_ENV)?
            .parse::<u8>()
            .map_err(|_| ProbeConfigError::InvalidRequestCap)?;
        if request_cap != EXTERNAL_REQUEST_CAP {
            return Err(ProbeConfigError::InvalidRequestCap);
        }
        let target_label = required_value(read, TARGET_LABEL_ENV)?;
        if !is_opaque_target_label(&target_label) {
            return Err(ProbeConfigError::InvalidTargetLabel);
        }
        let mode = ProbeMode::parse(&required_value(read, MODE_ENV)?)?;
        let proxy = match required_value(read, NETWORK_PROFILE_ENV)?.as_str() {
            "direct" => {
                if read(SOCKS5_PROXY_ENV).is_some() {
                    return Err(ProbeConfigError::UnexpectedProxyValue);
                }
                UpstreamProxy::Direct
            }
            "socks5" => UpstreamProxy::try_socks5(&required_value(read, SOCKS5_PROXY_ENV)?)
                .map_err(|_| ProbeConfigError::InvalidProxy)?,
            _ => return Err(ProbeConfigError::InvalidNetworkProfile),
        };
        let credential_json = Zeroizing::new(required_value(read, OAUTH_CREDENTIAL_JSON_ENV)?);
        let upstream_model = required_value(read, UPSTREAM_MODEL_ENV)?;
        if upstream_model.is_empty() || !upstream_model.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(ProbeConfigError::InvalidUpstreamModel);
        }
        let allowed_cidr = match read(ALLOWED_CIDR_ENV) {
            Some(value) if value.trim().is_empty() => {
                return Err(ProbeConfigError::InvalidAllowedCidr);
            }
            Some(value) => Some(
                EgressCidr::try_parse(&value).map_err(|_| ProbeConfigError::InvalidAllowedCidr)?,
            ),
            None => None,
        };

        Ok(Self {
            target_label,
            mode,
            credential_json,
            upstream_model,
            allowed_cidr,
            proxy,
        })
    }

    fn prepare(self) -> Result<PreparedProbe, ProbeConfigError> {
        let now_ms = now_ms()?;
        let credential = GrokBuildCredential::import_json(self.credential_json.as_bytes(), now_ms)
            .map_err(|_| ProbeConfigError::InvalidCredential)?;
        if credential.is_expired_at(now_ms) {
            return Err(ProbeConfigError::InvalidCredential);
        }
        let parsed =
            Url::parse(GROK_BUILD_RESPONSES_URL).map_err(|_| ProbeConfigError::InvalidEndpoint)?;
        let scheme = EgressScheme::try_from_url_scheme(parsed.scheme())
            .map_err(|_| ProbeConfigError::InvalidEndpoint)?;
        let host = parsed
            .host_str()
            .ok_or(ProbeConfigError::InvalidEndpoint)
            .and_then(|value| {
                EgressHost::try_new(value).map_err(|_| ProbeConfigError::InvalidEndpoint)
            })?;
        let port = parsed
            .port_or_known_default()
            .ok_or(ProbeConfigError::InvalidEndpoint)?;
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("p6-03-build-probe-egress".to_owned())
                .map_err(|_| ProbeConfigError::InvalidEgressPolicy)?,
            name: "P6-03 authorized Grok Build probe policy".to_owned(),
            allowed_schemes: BTreeSet::from([scheme]),
            allowed_hosts: BTreeSet::from([host]),
            allowed_ports: BTreeSet::from([port]),
            allowed_cidrs: self.allowed_cidr.into_iter().collect(),
            redirect_policy: RedirectPolicy::Deny,
        })
        .map_err(|_| ProbeConfigError::InvalidEgressPolicy)?;
        let profile = UpstreamTransportProfile::new(
            UpstreamTimeouts::try_new(CONNECT_TIMEOUT, TTFB_TIMEOUT, IDLE_TIMEOUT, TOTAL_TIMEOUT)
                .map_err(|_| ProbeConfigError::InvalidTimeouts)?,
            self.proxy,
            NonZeroUsize::new(1).ok_or(ProbeConfigError::InvalidTimeouts)?,
        );
        let decoded = decode_request(&probe_payload(self.mode))
            .map_err(|_| ProbeConfigError::InvalidProbePayload)?;
        if decoded.mode != self.mode.response_mode() {
            return Err(ProbeConfigError::InvalidProbePayload);
        }
        let outbound = GrokBuildResponsesRequestBuilder::build(
            &credential,
            &self.upstream_model,
            &decoded.request,
            decoded.mode,
        )
        .map_err(|_| ProbeConfigError::InvalidCredential)?;

        Ok(PreparedProbe {
            target_label: self.target_label,
            mode: self.mode,
            policy,
            profile,
            outbound,
        })
    }
}

struct PreparedProbe {
    target_label: String,
    mode: ProbeMode,
    policy: EgressPolicy,
    profile: UpstreamTransportProfile,
    outbound: GrokBuildResponsesOutboundRequest,
}

impl PreparedProbe {
    fn target_label(&self) -> &str {
        &self.target_label
    }

    const fn mode(&self) -> ProbeMode {
        self.mode
    }

    fn safe_summary(&self) -> String {
        format!("target={} mode={}", self.target_label, self.mode.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ProbeSemanticMarker {
    ResponseStart,
    Text,
    ResponseEnd,
    StreamError,
}

#[derive(Default)]
struct ProbeResponseShape {
    markers: BTreeSet<ProbeSemanticMarker>,
}

impl ProbeResponseShape {
    fn observe(&mut self, events: &[CanonicalEvent]) {
        for event in events {
            match event {
                CanonicalEvent::ResponseStart(_) => {
                    self.markers.insert(ProbeSemanticMarker::ResponseStart);
                }
                CanonicalEvent::TextDelta(_) => {
                    self.markers.insert(ProbeSemanticMarker::Text);
                }
                CanonicalEvent::ResponseEnd(_) => {
                    self.markers.insert(ProbeSemanticMarker::ResponseEnd);
                }
                CanonicalEvent::StreamError(_) => {
                    self.markers.insert(ProbeSemanticMarker::StreamError);
                }
                CanonicalEvent::MessageStart(_)
                | CanonicalEvent::ReasoningDelta(_)
                | CanonicalEvent::ToolCallStart(_)
                | CanonicalEvent::ToolCallArgumentsDelta(_)
                | CanonicalEvent::ToolCallEnd(_)
                | CanonicalEvent::UsageDelta(_)
                | CanonicalEvent::MessageEnd(_) => {}
            }
        }
    }

    fn verify(&self) -> Result<(), ProbeError> {
        if self.markers.contains(&ProbeSemanticMarker::ResponseStart)
            && self.markers.contains(&ProbeSemanticMarker::Text)
            && self.markers.contains(&ProbeSemanticMarker::ResponseEnd)
            && !self.markers.contains(&ProbeSemanticMarker::StreamError)
        {
            Ok(())
        } else {
            Err(ProbeError::MissingExpectedSemanticResponse)
        }
    }
}

async fn execute_one_probe(probe: PreparedProbe) -> Result<(), ProbeError> {
    let PreparedProbe {
        target_label,
        mode,
        policy,
        profile,
        outbound,
    } = probe;
    let resolver = SystemEgressDnsResolver;
    let admitted = policy
        .admit_url(outbound.url(), &resolver)
        .map_err(|_| ProbeError::EgressAdmissionFailed)?;
    let request = outbound
        .into_transport_request(admitted)
        .map_err(|_| ProbeError::EgressAdmissionFailed)?;
    let client_pool =
        UpstreamClientPool::new(NonZeroUsize::new(1).ok_or(ProbeError::InternalInvariant)?);

    // This is the only send call: no retry loop, candidate selection, refresh, or failover exists.
    let mut response = client_pool
        .send(request, &profile)
        .await
        .map_err(|_| ProbeError::TransportFailed)?;
    let status_class = safe_status_class(response.status());
    let content_type = safe_content_type_class(&response, mode);
    println!(
        "p6-03 build probe response target={target_label} status_class={status_class} content_type={content_type}"
    );
    if !(200..=299).contains(&response.status()) {
        let status = response.status();
        let body = read_bounded(&mut response, MAX_GROK_BUILD_ERROR_BODY_BYTES).await?;
        println!(
            "p6-03 build probe response target={target_label} body_shape={}",
            safe_body_shape(&body).as_str()
        );
        let envelope = GrokBuildResponsesHttpError::parse(status, &body)
            .map_err(|_| ProbeError::ResponseProtocolFailed)?;
        if envelope.status() != status {
            return Err(ProbeError::InternalInvariant);
        }
        return Err(ProbeError::NonSuccessStatus(safe_status_class(status)));
    }
    let expected_content_type = response
        .header("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with(mode.expected_content_type()));
    if !expected_content_type {
        return Err(ProbeError::UnexpectedContentType);
    }

    match mode {
        ProbeMode::NonStreaming => {
            let body =
                read_bounded(&mut response, MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES).await?;
            println!(
                "p6-03 build probe response target={target_label} body_shape={}",
                safe_body_shape(&body).as_str()
            );
            let decoded = GrokBuildResponsesDecoder::decode_non_streaming(&body)
                .map_err(|_| ProbeError::ResponseProtocolFailed)?;
            let mut shape = ProbeResponseShape::default();
            shape.observe(decoded.events());
            shape.verify()
        }
        ProbeMode::Sse => decode_sse_response(&mut response).await,
    }
}

async fn decode_sse_response(response: &mut UpstreamHttpResponse) -> Result<(), ProbeError> {
    let mut decoder = GrokBuildResponsesStreamDecoder::new();
    let mut shape = ProbeResponseShape::default();
    let mut response_bytes = 0_usize;
    while let Some(chunk) = response
        .next_chunk()
        .await
        .map_err(|_| ProbeError::ResponseReadFailed)?
    {
        response_bytes = response_bytes
            .checked_add(chunk.len())
            .ok_or(ProbeError::ResponseTooLarge)?;
        if response_bytes > MAX_GROK_BUILD_PROBE_STREAM_BYTES {
            return Err(ProbeError::ResponseTooLarge);
        }
        let events = decoder
            .push_bytes(&chunk)
            .map_err(|_| ProbeError::ResponseProtocolFailed)?;
        shape.observe(&events);
    }
    decoder
        .finish()
        .map_err(|_| ProbeError::ResponseProtocolFailed)?;
    shape.verify()
}

async fn read_bounded(
    response: &mut UpstreamHttpResponse,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ProbeError> {
    let mut body = Vec::new();
    while let Some(chunk) = response
        .next_chunk()
        .await
        .map_err(|_| ProbeError::ResponseReadFailed)?
    {
        let next_length = body
            .len()
            .checked_add(chunk.len())
            .ok_or(ProbeError::ResponseTooLarge)?;
        if next_length > maximum_bytes {
            return Err(ProbeError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn required_value<F>(read: &mut F, name: &str) -> Result<String, ProbeConfigError>
where
    F: FnMut(&str) -> Option<String>,
{
    read(name)
        .filter(|value| !value.trim().is_empty())
        .ok_or(ProbeConfigError::MissingRequiredValue)
}

fn is_opaque_target_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn now_ms() -> Result<i64, ProbeConfigError> {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProbeConfigError::SystemClock)?
        .as_millis();
    i64::try_from(milliseconds).map_err(|_| ProbeConfigError::SystemClock)
}

fn probe_payload(mode: ProbeMode) -> String {
    format!(
        r#"{{"model":"p6-03-build-probe","input":"Reply with exactly: ready","max_output_tokens":{PROBE_MAX_OUTPUT_TOKENS},"stream":{}}}"#,
        matches!(mode, ProbeMode::Sse)
    )
}

const fn safe_status_class(status: u16) -> &'static str {
    match status / 100 {
        1 => "1xx",
        3 => "3xx",
        4 => "4xx",
        5 => "5xx",
        _ => "other",
    }
}

fn safe_content_type_class(response: &UpstreamHttpResponse, mode: ProbeMode) -> &'static str {
    match response
        .header("content-type")
        .and_then(|value| value.to_str().ok())
    {
        Some(value) if value.starts_with(mode.expected_content_type()) => "expected",
        Some(_) => "other",
        None => "missing_or_invalid",
    }
}

fn safe_body_shape(body: &[u8]) -> SafeBodyShape {
    match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(serde_json::Value::Object(object)) if object.contains_key("error") => {
            SafeBodyShape::ErrorLikeObject
        }
        Ok(serde_json::Value::Object(object)) if object.contains_key("output") => {
            SafeBodyShape::ResponseLikeObject
        }
        Ok(serde_json::Value::Object(_)) => SafeBodyShape::OtherObject,
        Ok(serde_json::Value::Array(_)) => SafeBodyShape::Array,
        Ok(
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_),
        ) => SafeBodyShape::Scalar,
        Err(_) => SafeBodyShape::InvalidJson,
    }
}

#[test]
fn missing_authorization_stops_before_any_other_value_is_read() {
    let mut reads = Vec::new();
    let mut read = |name: &str| {
        reads.push(name.to_owned());
        None::<String>
    };

    assert!(matches!(
        AuthorizedProbeConfig::from_values(&mut read),
        Err(ProbeConfigError::NotAuthorized)
    ));
    assert_eq!(reads, vec![AUTHORIZATION_ENV]);
}

#[test]
fn fixed_request_cap_and_mode_are_fail_closed() {
    let mut values = synthetic_values();
    values.insert(REQUEST_CAP_ENV.to_owned(), "2".to_owned());
    assert!(matches!(
        config_from_map(&values),
        Err(ProbeConfigError::InvalidRequestCap)
    ));

    let mut invalid_mode = synthetic_values();
    invalid_mode.insert(MODE_ENV.to_owned(), "streaming".to_owned());
    assert!(matches!(
        config_from_map(&invalid_mode),
        Err(ProbeConfigError::InvalidMode)
    ));
}

#[test]
fn direct_profile_rejects_a_leftover_proxy_value() {
    let mut values = synthetic_values();
    values.insert(
        SOCKS5_PROXY_ENV.to_owned(),
        "socks5://127.0.0.1:7891".to_owned(),
    );
    assert!(matches!(
        config_from_map(&values),
        Err(ProbeConfigError::UnexpectedProxyValue)
    ));
}

#[test]
fn complete_synthetic_configuration_prepares_without_dns_or_transport() -> TestResult {
    let prepared = config_from_map(&synthetic_values())?.prepare()?;
    assert_eq!(prepared.target_label(), "build_test");
    assert_eq!(prepared.mode(), ProbeMode::NonStreaming);
    assert_eq!(prepared.outbound.url(), GROK_BUILD_RESPONSES_URL);
    assert_eq!(prepared.profile.timeouts().connect(), CONNECT_TIMEOUT);
    assert_eq!(prepared.profile.timeouts().ttfb(), TTFB_TIMEOUT);
    assert_eq!(prepared.profile.timeouts().idle(), IDLE_TIMEOUT);
    assert_eq!(prepared.profile.timeouts().total(), TOTAL_TIMEOUT);
    assert!(matches!(prepared.profile.proxy(), UpstreamProxy::Direct));
    Ok(())
}

#[test]
fn fixed_probe_payload_is_mode_specific_and_secret_safe() -> TestResult {
    for (mode, streaming) in [(ProbeMode::NonStreaming, false), (ProbeMode::Sse, true)] {
        let mut values = synthetic_values();
        values.insert(MODE_ENV.to_owned(), mode.as_str().to_owned());
        let prepared = config_from_map(&values)?.prepare()?;
        let payload: serde_json::Value = serde_json::from_slice(prepared.outbound.body())?;
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
        let diagnostic = format!("{:?}{}", prepared.outbound, prepared.safe_summary());
        for private_value in [
            "synthetic_build_access_token_012345",
            "synthetic_build_refresh_token_012345",
            "synthetic-build-upstream-model",
            "cli-chat-proxy.grok.com",
        ] {
            assert!(!diagnostic.contains(private_value));
        }
    }
    Ok(())
}

#[test]
fn safe_body_shape_retains_no_upstream_values() {
    let private_body = br#"{
        "error":{"message":"private upstream text","code":"private-code"},
        "id":"private-response-id"
    }"#;
    let shape = safe_body_shape(private_body);
    assert_eq!(shape, SafeBodyShape::ErrorLikeObject);
    assert_eq!(shape.as_str(), "error_like_object");
    for private_value in [
        "private upstream text",
        "private-code",
        "private-response-id",
    ] {
        assert!(!shape.as_str().contains(private_value));
    }
    assert_eq!(
        safe_body_shape(br#"{"output":[]}"#),
        SafeBodyShape::ResponseLikeObject
    );
    assert_eq!(safe_body_shape(b"not-json"), SafeBodyShape::InvalidJson);
}

#[tokio::test]
#[ignore = "requires dedicated P6_03_* authorization, one target, one mode, and one-request configuration"]
async fn authorized_build_probe_uses_one_target_one_mode_and_one_send() -> TestResult {
    let prepared = AuthorizedProbeConfig::from_environment()?.prepare()?;
    let target_label = prepared.target_label().to_owned();
    let mode = prepared.mode().as_str();
    println!("p6-03 build probe target={target_label} mode={mode} result=started");

    match execute_one_probe(prepared).await {
        Ok(()) => {
            println!("p6-03 build probe target={target_label} mode={mode} result=pass");
            Ok(())
        }
        Err(error) => {
            println!(
                "p6-03 build probe target={target_label} mode={mode} result=stopped outcome={}",
                error.safe_outcome()
            );
            Err(error.into())
        }
    }
}

fn config_from_map(
    values: &BTreeMap<String, String>,
) -> Result<AuthorizedProbeConfig, ProbeConfigError> {
    AuthorizedProbeConfig::from_values(&mut |name| values.get(name).cloned())
}

fn synthetic_values() -> BTreeMap<String, String> {
    BTreeMap::from([
        (AUTHORIZATION_ENV.to_owned(), AUTHORIZATION_VALUE.to_owned()),
        (REQUEST_CAP_ENV.to_owned(), EXTERNAL_REQUEST_CAP.to_string()),
        (TARGET_LABEL_ENV.to_owned(), "build_test".to_owned()),
        (MODE_ENV.to_owned(), "non_streaming".to_owned()),
        (
            OAUTH_CREDENTIAL_JSON_ENV.to_owned(),
            r#"{"access_token":"synthetic_build_access_token_012345","refresh_token":"synthetic_build_refresh_token_012345","expires_in":3600,"token_type":"Bearer"}"#.to_owned(),
        ),
        (
            UPSTREAM_MODEL_ENV.to_owned(),
            "synthetic-build-upstream-model".to_owned(),
        ),
        (NETWORK_PROFILE_ENV.to_owned(), "direct".to_owned()),
    ])
}
