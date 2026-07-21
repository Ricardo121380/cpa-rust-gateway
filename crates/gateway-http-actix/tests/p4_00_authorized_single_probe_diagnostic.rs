//! A deliberately narrow, opt-in transport diagnostic for one authorized real test target.
//!
//! This is separate from the P3-10 four-probe aggregation acceptance harness. It is ignored by
//! default, reads no generic provider environment variables or `.env` files, and cannot build an
//! endpoint, DNS policy, transport profile, or outbound request until its dedicated authorization
//! value and exact one-request cap have both been accepted.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fmt,
    num::NonZeroUsize,
    time::Duration,
};

use gateway_core::EgressPolicyId;
use gateway_upstream::{
    EgressCidr, EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy,
    SystemEgressDnsResolver, UpstreamClientPool, UpstreamProxy, UpstreamTimeouts,
    UpstreamTransportProfile,
};
use protocol_openai_responses::ResponseMode;
use provider_openai_compatible::{
    OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesOutboundRequest,
    OpenAiResponsesRequestBuilder,
};
use url::Url;
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn Error>>;

const AUTHORIZATION_ENV: &str = "P4_00_DIAGNOSTIC_AUTHORIZATION";
const AUTHORIZATION_VALUE: &str = "single-probe-approved";
const REQUEST_CAP_ENV: &str = "P4_00_DIAGNOSTIC_MAX_EXTERNAL_REQUESTS";
const TARGET_LABEL_ENV: &str = "P4_00_DIAGNOSTIC_TARGET_LABEL";
const MODE_ENV: &str = "P4_00_DIAGNOSTIC_MODE";
const BASE_URL_ENV: &str = "P4_00_DIAGNOSTIC_BASE_URL";
const API_KEY_ENV: &str = "P4_00_DIAGNOSTIC_API_KEY";
const UPSTREAM_MODEL_ENV: &str = "P4_00_DIAGNOSTIC_UPSTREAM_MODEL";
const NETWORK_PROFILE_ENV: &str = "P4_00_DIAGNOSTIC_NETWORK_PROFILE";
const SOCKS5_PROXY_ENV: &str = "P4_00_DIAGNOSTIC_SOCKS5_PROXY_URL";
const ALLOWED_CIDR_ENV: &str = "P4_00_DIAGNOSTIC_ALLOWED_CIDR";

const EXTERNAL_REQUEST_CAP: u8 = 1;
const PROBE_MAX_OUTPUT_TOKENS: u64 = 32;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TTFB_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagnosticMode {
    NonStreaming,
    Sse,
}

impl DiagnosticMode {
    fn parse(value: &str) -> Result<Self, DiagnosticConfigError> {
        match value {
            "non_streaming" => Ok(Self::NonStreaming),
            "sse" => Ok(Self::Sse),
            _ => Err(DiagnosticConfigError::InvalidMode),
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
enum DiagnosticConfigError {
    NotAuthorized,
    MissingRequiredValue,
    InvalidRequestCap,
    InvalidTargetLabel,
    InvalidMode,
    InvalidNetworkProfile,
    UnexpectedProxyValue,
    InvalidProxy,
    InvalidEndpoint,
    InvalidCredential,
    InvalidAllowedCidr,
    InvalidEgressPolicy,
    InvalidTimeouts,
    InternalProbeInvariant,
}

impl fmt::Display for DiagnosticConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotAuthorized => "P4-00 diagnostic is not explicitly authorized",
            Self::MissingRequiredValue => "P4-00 diagnostic configuration is incomplete",
            Self::InvalidRequestCap => "P4-00 diagnostic must use its fixed one-request cap",
            Self::InvalidTargetLabel => "P4-00 diagnostic target label is not opaque",
            Self::InvalidMode => "P4-00 diagnostic mode is invalid",
            Self::InvalidNetworkProfile => "P4-00 diagnostic network profile is invalid",
            Self::UnexpectedProxyValue => {
                "P4-00 diagnostic direct profile cannot retain a proxy value"
            }
            Self::InvalidProxy => "P4-00 diagnostic SOCKS5 proxy is invalid",
            Self::InvalidEndpoint => "P4-00 diagnostic Endpoint configuration is invalid",
            Self::InvalidCredential => "P4-00 diagnostic credential is invalid",
            Self::InvalidAllowedCidr => "P4-00 diagnostic explicit CIDR configuration is invalid",
            Self::InvalidEgressPolicy => "P4-00 diagnostic egress policy construction failed",
            Self::InvalidTimeouts => "P4-00 diagnostic transport timeout construction failed",
            Self::InternalProbeInvariant => "P4-00 diagnostic fixed probe invariant failed",
        };
        formatter.write_str(message)
    }
}

impl Error for DiagnosticConfigError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagnosticProbeError {
    EgressAdmissionFailed,
    TransportFailed,
    NonSuccessStatus(&'static str),
    UnexpectedContentType,
    ResponseReadFailed,
    ResponseTooLarge,
    InternalInvariant,
}

impl DiagnosticProbeError {
    const fn safe_outcome(self) -> &'static str {
        match self {
            Self::EgressAdmissionFailed => "egress_admission_failed",
            Self::TransportFailed => "transport_failed",
            Self::NonSuccessStatus(status_class) => status_class,
            Self::UnexpectedContentType => "unexpected_content_type",
            Self::ResponseReadFailed => "response_read_failed",
            Self::ResponseTooLarge => "response_too_large",
            Self::InternalInvariant => "internal_invariant",
        }
    }
}

impl fmt::Display for DiagnosticProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "P4-00 single-probe diagnostic stopped: {}",
            self.safe_outcome()
        )
    }
}

impl Error for DiagnosticProbeError {}

struct AuthorizedDiagnosticConfig {
    target_label: String,
    mode: DiagnosticMode,
    base_url: String,
    api_key: Zeroizing<String>,
    upstream_model: String,
    allowed_cidr: Option<EgressCidr>,
    proxy: UpstreamProxy,
}

impl AuthorizedDiagnosticConfig {
    fn from_environment() -> Result<Self, DiagnosticConfigError> {
        Self::from_values(&mut |name| env::var(name).ok())
    }

    fn from_values<F>(read: &mut F) -> Result<Self, DiagnosticConfigError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        // This exact first read is the non-network, no-construction authorization boundary.
        if read(AUTHORIZATION_ENV).as_deref() != Some(AUTHORIZATION_VALUE) {
            return Err(DiagnosticConfigError::NotAuthorized);
        }

        let request_cap = required_value(read, REQUEST_CAP_ENV)?
            .parse::<u8>()
            .map_err(|_| DiagnosticConfigError::InvalidRequestCap)?;
        if request_cap != EXTERNAL_REQUEST_CAP {
            return Err(DiagnosticConfigError::InvalidRequestCap);
        }

        let target_label = required_value(read, TARGET_LABEL_ENV)?;
        if !is_opaque_target_label(&target_label) {
            return Err(DiagnosticConfigError::InvalidTargetLabel);
        }
        let mode = DiagnosticMode::parse(&required_value(read, MODE_ENV)?)?;
        let proxy = match required_value(read, NETWORK_PROFILE_ENV)?.as_str() {
            "direct" => {
                if read(SOCKS5_PROXY_ENV).is_some() {
                    return Err(DiagnosticConfigError::UnexpectedProxyValue);
                }
                UpstreamProxy::Direct
            }
            "socks5" => UpstreamProxy::try_socks5(&required_value(read, SOCKS5_PROXY_ENV)?)
                .map_err(|_| DiagnosticConfigError::InvalidProxy)?,
            _ => return Err(DiagnosticConfigError::InvalidNetworkProfile),
        };
        let base_url = required_value(read, BASE_URL_ENV)?;
        let api_key = Zeroizing::new(required_value(read, API_KEY_ENV)?);
        if !api_key.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(DiagnosticConfigError::InvalidCredential);
        }
        let upstream_model = required_value(read, UPSTREAM_MODEL_ENV)?;
        let allowed_cidr = match read(ALLOWED_CIDR_ENV) {
            Some(value) if value.trim().is_empty() => {
                return Err(DiagnosticConfigError::InvalidAllowedCidr);
            }
            Some(value) => Some(
                EgressCidr::try_parse(&value)
                    .map_err(|_| DiagnosticConfigError::InvalidAllowedCidr)?,
            ),
            None => None,
        };

        Ok(Self {
            target_label,
            mode,
            base_url,
            api_key,
            upstream_model,
            allowed_cidr,
            proxy,
        })
    }

    fn prepare(self) -> Result<PreparedDiagnostic, DiagnosticConfigError> {
        let endpoint = OpenAiResponsesEndpoint::try_new(&self.base_url, "/responses")
            .map_err(|_| DiagnosticConfigError::InvalidEndpoint)?;
        let parsed =
            Url::parse(endpoint.url()).map_err(|_| DiagnosticConfigError::InvalidEndpoint)?;
        let scheme = EgressScheme::try_from_url_scheme(parsed.scheme())
            .map_err(|_| DiagnosticConfigError::InvalidEndpoint)?;
        let host = parsed
            .host_str()
            .ok_or(DiagnosticConfigError::InvalidEndpoint)
            .and_then(|value| {
                EgressHost::try_new(value).map_err(|_| DiagnosticConfigError::InvalidEndpoint)
            })?;
        let port = parsed
            .port_or_known_default()
            .ok_or(DiagnosticConfigError::InvalidEndpoint)?;
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("p4-00-diagnostic-egress".to_owned())
                .map_err(|_| DiagnosticConfigError::InvalidEgressPolicy)?,
            name: "P4-00 authorized single-probe policy".to_owned(),
            allowed_schemes: BTreeSet::from([scheme]),
            allowed_hosts: BTreeSet::from([host]),
            allowed_ports: BTreeSet::from([port]),
            allowed_cidrs: self.allowed_cidr.into_iter().collect(),
            redirect_policy: RedirectPolicy::Deny,
        })
        .map_err(|_| DiagnosticConfigError::InvalidEgressPolicy)?;
        let timeouts = diagnostic_timeouts()?;
        let profile = UpstreamTransportProfile::new(
            timeouts,
            self.proxy,
            NonZeroUsize::new(1).ok_or(DiagnosticConfigError::InvalidTimeouts)?,
        );
        let credential = OpenAiResponsesApiKey::try_new(self.api_key.as_str())
            .map_err(|_| DiagnosticConfigError::InvalidCredential)?;
        let decoded = protocol_openai_responses::decode_request(&probe_payload(self.mode))
            .map_err(|_| DiagnosticConfigError::InternalProbeInvariant)?;
        if decoded.mode != self.mode.response_mode() {
            return Err(DiagnosticConfigError::InternalProbeInvariant);
        }
        let outbound = OpenAiResponsesRequestBuilder::build(
            &endpoint,
            &credential,
            &self.upstream_model,
            &decoded.request,
            decoded.mode,
        )
        .map_err(|_| DiagnosticConfigError::InternalProbeInvariant)?;

        Ok(PreparedDiagnostic {
            target_label: self.target_label,
            mode: self.mode,
            policy,
            profile,
            outbound,
        })
    }
}

struct PreparedDiagnostic {
    target_label: String,
    mode: DiagnosticMode,
    policy: EgressPolicy,
    profile: UpstreamTransportProfile,
    outbound: OpenAiResponsesOutboundRequest,
}

impl PreparedDiagnostic {
    fn target_label(&self) -> &str {
        &self.target_label
    }

    const fn mode(&self) -> DiagnosticMode {
        self.mode
    }

    fn safe_summary(&self) -> String {
        format!("target={} mode={}", self.target_label, self.mode.as_str())
    }
}

fn diagnostic_timeouts() -> Result<UpstreamTimeouts, DiagnosticConfigError> {
    UpstreamTimeouts::try_new(CONNECT_TIMEOUT, TTFB_TIMEOUT, IDLE_TIMEOUT, TOTAL_TIMEOUT)
        .map_err(|_| DiagnosticConfigError::InvalidTimeouts)
}

fn required_value<F>(read: &mut F, name: &str) -> Result<String, DiagnosticConfigError>
where
    F: FnMut(&str) -> Option<String>,
{
    read(name)
        .filter(|value| !value.trim().is_empty())
        .ok_or(DiagnosticConfigError::MissingRequiredValue)
}

fn is_opaque_target_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn probe_payload(mode: DiagnosticMode) -> String {
    format!(
        r#"{{"model":"p4-00-diagnostic","input":"Reply with exactly: ready","max_output_tokens":{PROBE_MAX_OUTPUT_TOKENS},"stream":{}}}"#,
        matches!(mode, DiagnosticMode::Sse)
    )
}

async fn execute_one_probe(probe: PreparedDiagnostic) -> Result<(), DiagnosticProbeError> {
    let PreparedDiagnostic {
        target_label: _,
        mode,
        policy,
        profile,
        outbound,
    } = probe;
    let resolver = SystemEgressDnsResolver;
    let admitted = policy
        .admit_url(outbound.url(), &resolver)
        .map_err(|_| DiagnosticProbeError::EgressAdmissionFailed)?;
    let request = outbound
        .into_transport_request(admitted)
        .map_err(|_| DiagnosticProbeError::EgressAdmissionFailed)?;
    let client_pool = UpstreamClientPool::new(
        NonZeroUsize::new(1).ok_or(DiagnosticProbeError::InternalInvariant)?,
    );

    // There is exactly one `send` call: no retry loop, candidate selection, or failover path.
    // `UpstreamClientPool` additionally configures the underlying client with `retry::never()`.
    let mut response = client_pool
        .send(request, &profile)
        .await
        .map_err(|_| DiagnosticProbeError::TransportFailed)?;
    if !(200..=299).contains(&response.status()) {
        return Err(DiagnosticProbeError::NonSuccessStatus(safe_status_class(
            response.status(),
        )));
    }
    let has_expected_content_type = response
        .header("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with(mode.expected_content_type()));
    if !has_expected_content_type {
        return Err(DiagnosticProbeError::UnexpectedContentType);
    }

    // Response chunks are bounded and discarded immediately; no response body or frame is logged.
    let mut response_bytes = 0_usize;
    while let Some(chunk) = response
        .next_chunk()
        .await
        .map_err(|_| DiagnosticProbeError::ResponseReadFailed)?
    {
        response_bytes = response_bytes
            .checked_add(chunk.len())
            .ok_or(DiagnosticProbeError::ResponseTooLarge)?;
        if response_bytes > MAX_RESPONSE_BYTES {
            return Err(DiagnosticProbeError::ResponseTooLarge);
        }
    }

    Ok(())
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

#[test]
fn missing_authorization_stops_before_any_other_value_is_read() {
    let mut reads = Vec::new();
    let mut read = |name: &str| {
        reads.push(name.to_owned());
        None::<String>
    };

    assert!(matches!(
        AuthorizedDiagnosticConfig::from_values(&mut read),
        Err(DiagnosticConfigError::NotAuthorized)
    ));
    assert_eq!(reads, vec![AUTHORIZATION_ENV]);
}

#[test]
fn fixed_single_request_cap_must_equal_one() {
    let mut values = synthetic_values();
    values.insert(REQUEST_CAP_ENV.to_owned(), "2".to_owned());

    assert!(matches!(
        config_from_map(&values),
        Err(DiagnosticConfigError::InvalidRequestCap)
    ));
    assert_eq!(EXTERNAL_REQUEST_CAP, 1);
}

#[test]
fn invalid_mode_and_profile_are_rejected_before_preparation() {
    let mut invalid_mode = synthetic_values();
    invalid_mode.insert(MODE_ENV.to_owned(), "streaming".to_owned());
    assert!(matches!(
        config_from_map(&invalid_mode),
        Err(DiagnosticConfigError::InvalidMode)
    ));

    let mut invalid_profile = synthetic_values();
    invalid_profile.insert(NETWORK_PROFILE_ENV.to_owned(), "system".to_owned());
    assert!(matches!(
        config_from_map(&invalid_profile),
        Err(DiagnosticConfigError::InvalidNetworkProfile)
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
        Err(DiagnosticConfigError::UnexpectedProxyValue)
    ));
}

#[test]
fn complete_synthetic_configuration_prepares_without_dns_or_http() -> TestResult {
    let prepared = config_from_map(&synthetic_values())?.prepare()?;

    assert_eq!(prepared.target_label(), "target_a");
    assert_eq!(prepared.mode(), DiagnosticMode::NonStreaming);
    assert_eq!(prepared.profile.timeouts().connect(), CONNECT_TIMEOUT);
    assert_eq!(prepared.profile.timeouts().ttfb(), TTFB_TIMEOUT);
    assert_eq!(prepared.profile.timeouts().idle(), IDLE_TIMEOUT);
    assert_eq!(prepared.profile.timeouts().total(), TOTAL_TIMEOUT);
    assert_eq!(
        prepared.profile.maximum_idle_connections_per_host().get(),
        1
    );
    assert!(matches!(prepared.profile.proxy(), UpstreamProxy::Direct));
    Ok(())
}

#[test]
fn fixed_probe_payload_caps_output_and_selects_the_requested_mode() -> TestResult {
    for (mode, streaming) in [
        (DiagnosticMode::NonStreaming, false),
        (DiagnosticMode::Sse, true),
    ] {
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
    }
    Ok(())
}

#[test]
fn safe_diagnostic_summary_never_contains_private_configuration() -> TestResult {
    let prepared = config_from_map(&synthetic_values())?.prepare()?;
    let summary = prepared.safe_summary();

    assert_eq!(summary, "target=target_a mode=non_streaming");
    for private_value in [
        "private-relay.invalid",
        "synthetic-credential",
        "synthetic-upstream-model",
    ] {
        assert!(!summary.contains(private_value));
    }
    Ok(())
}

#[actix_web::test]
#[ignore = "requires dedicated P4_00_DIAGNOSTIC_* authorization and one-request configuration"]
async fn authorized_single_probe_uses_one_target_one_mode_and_one_send() -> TestResult {
    let prepared = AuthorizedDiagnosticConfig::from_environment()?.prepare()?;
    let target_label = prepared.target_label().to_owned();
    let mode = prepared.mode().as_str();
    println!("p4-00 diagnostic target={target_label} mode={mode} result=started");

    match execute_one_probe(prepared).await {
        Ok(()) => {
            println!("p4-00 diagnostic target={target_label} mode={mode} result=pass");
            Ok(())
        }
        Err(error) => {
            println!(
                "p4-00 diagnostic target={target_label} mode={mode} result=stopped outcome={}",
                error.safe_outcome()
            );
            Err(error.into())
        }
    }
}

fn config_from_map(
    values: &BTreeMap<String, String>,
) -> Result<AuthorizedDiagnosticConfig, DiagnosticConfigError> {
    AuthorizedDiagnosticConfig::from_values(&mut |name| values.get(name).cloned())
}

fn synthetic_values() -> BTreeMap<String, String> {
    BTreeMap::from([
        (AUTHORIZATION_ENV.to_owned(), AUTHORIZATION_VALUE.to_owned()),
        (REQUEST_CAP_ENV.to_owned(), EXTERNAL_REQUEST_CAP.to_string()),
        (TARGET_LABEL_ENV.to_owned(), "target_a".to_owned()),
        (MODE_ENV.to_owned(), "non_streaming".to_owned()),
        (
            BASE_URL_ENV.to_owned(),
            "https://private-relay.invalid/v1".to_owned(),
        ),
        (API_KEY_ENV.to_owned(), "synthetic-credential".to_owned()),
        (
            UPSTREAM_MODEL_ENV.to_owned(),
            "synthetic-upstream-model".to_owned(),
        ),
        (NETWORK_PROFILE_ENV.to_owned(), "direct".to_owned()),
    ])
}
