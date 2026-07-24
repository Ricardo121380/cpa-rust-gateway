//! One deliberately narrow, opt-in xAI Official API-key probe for the P8 Provider Gate.
//!
//! The ignored live test reads only its dedicated environment variables after an exact explicit
//! authorization and fixed one-request cap. It uses the real Official adapter and its DNS-pinned
//! transport without retry, credential rotation, failover, proxy discovery, or generic provider
//! configuration.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fmt,
    num::NonZeroUsize,
    sync::Arc,
    time::Duration,
};

use gateway_core::{CanonicalEvent, EgressPolicyId, RequestContext, RequestId};
use gateway_provider::InferenceAdapter;
use gateway_upstream::{
    EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy,
    SystemEgressDnsResolver, UpstreamClientPool, UpstreamProxy, UpstreamTimeouts,
    UpstreamTransportProfile,
};
use protocol_openai_responses::{ResponseMode, decode_request};
use provider_grok::{
    GROK_OFFICIAL_RESPONSES_URL, GrokOfficialApiKey, GrokOfficialExecutionMode,
    GrokOfficialInferenceAdapter, GrokOfficialUpstreamTransport,
};
use url::Url;

type TestResult = Result<(), Box<dyn Error>>;

const AUTHORIZATION_ENV: &str = "P8_07_LIVE_AUTHORIZATION";
const AUTHORIZATION_VALUE: &str = "single-probe-approved";
const REQUEST_CAP_ENV: &str = "P8_07_MAX_EXTERNAL_REQUESTS";
const TARGET_LABEL_ENV: &str = "P8_07_TARGET_LABEL";
const MODE_ENV: &str = "P8_07_MODE";
const API_KEY_ENV: &str = "P8_07_API_KEY";
const UPSTREAM_MODEL_ENV: &str = "P8_07_UPSTREAM_MODEL";

const EXTERNAL_REQUEST_CAP: u8 = 1;
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

    const fn execution_mode(self) -> GrokOfficialExecutionMode {
        match self {
            Self::NonStreaming => GrokOfficialExecutionMode::NonStreaming,
            Self::Sse => GrokOfficialExecutionMode::Streaming,
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
    InvalidApiKey,
    InvalidUpstreamModel,
    InvalidEndpoint,
    InvalidEgressPolicy,
    InvalidTimeouts,
    InvalidProbePayload,
}

impl fmt::Display for ProbeConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotAuthorized => "P8 Official probe is not explicitly authorized",
            Self::MissingRequiredValue => "P8 Official probe configuration is incomplete",
            Self::InvalidRequestCap => "P8 Official probe must use its fixed one-request cap",
            Self::InvalidTargetLabel => "P8 Official probe target label is not opaque",
            Self::InvalidMode => "P8 Official probe mode is invalid",
            Self::InvalidApiKey => "P8 Official probe API key is invalid",
            Self::InvalidUpstreamModel => "P8 Official probe model is invalid",
            Self::InvalidEndpoint => "P8 Official probe fixed endpoint is invalid",
            Self::InvalidEgressPolicy => "P8 Official probe egress policy is invalid",
            Self::InvalidTimeouts => "P8 Official probe timeout profile is invalid",
            Self::InvalidProbePayload => "P8 Official probe payload is invalid",
        })
    }
}

impl Error for ProbeConfigError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeFailure {
    RequestDidNotStart,
    ResponseReadFailed,
    MissingCanonicalSuccess,
}

impl ProbeFailure {
    const fn safe_outcome(self) -> &'static str {
        match self {
            Self::RequestDidNotStart => "request_did_not_start",
            Self::ResponseReadFailed => "response_read_failed",
            Self::MissingCanonicalSuccess => "missing_canonical_success",
        }
    }
}

impl fmt::Display for ProbeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "P8 Official probe stopped: {}",
            self.safe_outcome()
        )
    }
}

impl Error for ProbeFailure {}

struct AuthorizedProbeConfig {
    target_label: String,
    mode: ProbeMode,
    credential: GrokOfficialApiKey,
    upstream_model: String,
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
        let credential = GrokOfficialApiKey::try_new(required_value(read, API_KEY_ENV)?)
            .map_err(|_| ProbeConfigError::InvalidApiKey)?;
        let upstream_model = required_value(read, UPSTREAM_MODEL_ENV)?;
        if upstream_model.is_empty() || !upstream_model.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(ProbeConfigError::InvalidUpstreamModel);
        }
        Ok(Self {
            target_label,
            mode,
            credential,
            upstream_model,
        })
    }

    fn prepare(self) -> Result<PreparedProbe, ProbeConfigError> {
        let Self {
            target_label,
            mode,
            credential,
            upstream_model,
        } = self;
        let endpoint = Url::parse(GROK_OFFICIAL_RESPONSES_URL)
            .map_err(|_| ProbeConfigError::InvalidEndpoint)?;
        let scheme = EgressScheme::try_from_url_scheme(endpoint.scheme())
            .map_err(|_| ProbeConfigError::InvalidEndpoint)?;
        let host = endpoint
            .host_str()
            .ok_or(ProbeConfigError::InvalidEndpoint)
            .and_then(|value| {
                EgressHost::try_new(value).map_err(|_| ProbeConfigError::InvalidEndpoint)
            })?;
        let port = endpoint
            .port_or_known_default()
            .ok_or(ProbeConfigError::InvalidEndpoint)?;
        let egress_policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("p8-07-official-probe-egress")
                .map_err(|_| ProbeConfigError::InvalidEgressPolicy)?,
            name: "P8 authorized Grok Official probe policy".to_owned(),
            allowed_schemes: BTreeSet::from([scheme]),
            allowed_hosts: BTreeSet::from([host]),
            allowed_ports: BTreeSet::from([port]),
            allowed_cidrs: BTreeSet::new(),
            redirect_policy: RedirectPolicy::Deny,
        })
        .map_err(|_| ProbeConfigError::InvalidEgressPolicy)?;
        let timeouts =
            UpstreamTimeouts::try_new(CONNECT_TIMEOUT, TTFB_TIMEOUT, IDLE_TIMEOUT, TOTAL_TIMEOUT)
                .map_err(|_| ProbeConfigError::InvalidTimeouts)?;
        let profile = UpstreamTransportProfile::new(
            timeouts,
            UpstreamProxy::Direct,
            NonZeroUsize::new(1).ok_or(ProbeConfigError::InvalidTimeouts)?,
        );
        let decoded = decode_request(&probe_payload(mode))
            .map_err(|_| ProbeConfigError::InvalidProbePayload)?;
        if decoded.mode != mode.response_mode() {
            return Err(ProbeConfigError::InvalidProbePayload);
        }
        let transport = Arc::new(GrokOfficialUpstreamTransport::new(
            egress_policy,
            Arc::new(SystemEgressDnsResolver),
            UpstreamClientPool::new(NonZeroUsize::new(1).ok_or(ProbeConfigError::InvalidTimeouts)?),
            profile,
        ));
        let adapter = GrokOfficialInferenceAdapter::try_new(
            credential,
            upstream_model,
            mode.execution_mode(),
            transport,
        )
        .map_err(|_| ProbeConfigError::InvalidProbePayload)?;
        Ok(PreparedProbe {
            target_label,
            mode,
            adapter,
            request: decoded.request,
        })
    }
}

struct PreparedProbe {
    target_label: String,
    mode: ProbeMode,
    adapter: GrokOfficialInferenceAdapter,
    request: gateway_core::CanonicalRequest,
}

impl PreparedProbe {
    fn target_label(&self) -> &str {
        &self.target_label
    }

    const fn mode(&self) -> ProbeMode {
        self.mode
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CanonicalSuccessMarker {
    ResponseStart,
    Text,
    ResponseEnd,
    StreamError,
}

#[derive(Default)]
struct CanonicalSuccessShape {
    markers: BTreeSet<CanonicalSuccessMarker>,
}

impl CanonicalSuccessShape {
    fn observe(&mut self, event: &CanonicalEvent) {
        match event {
            CanonicalEvent::ResponseStart(_) => {
                self.markers.insert(CanonicalSuccessMarker::ResponseStart);
            }
            CanonicalEvent::TextDelta(_) => {
                self.markers.insert(CanonicalSuccessMarker::Text);
            }
            CanonicalEvent::ResponseEnd(_) => {
                self.markers.insert(CanonicalSuccessMarker::ResponseEnd);
            }
            CanonicalEvent::StreamError(_) => {
                self.markers.insert(CanonicalSuccessMarker::StreamError);
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

    fn is_success(&self) -> bool {
        self.markers
            .contains(&CanonicalSuccessMarker::ResponseStart)
            && self.markers.contains(&CanonicalSuccessMarker::Text)
            && self.markers.contains(&CanonicalSuccessMarker::ResponseEnd)
            && !self.markers.contains(&CanonicalSuccessMarker::StreamError)
    }
}

async fn execute_one_probe(probe: PreparedProbe) -> Result<(), ProbeFailure> {
    let context = RequestContext::new(
        RequestId::try_new("p8-07-official-probe").map_err(|_| ProbeFailure::RequestDidNotStart)?,
    );
    // `execute` below is the only path that can issue one request. Neither configuration parsing
    // nor `prepare` performs DNS, HTTP, retry, failover, proxy discovery, or credential rotation.
    let mut source = probe
        .adapter
        .execute(context, probe.request)
        .await
        .map_err(|_| ProbeFailure::RequestDidNotStart)?;
    let mut shape = CanonicalSuccessShape::default();
    while let Some(event) = source
        .next_event()
        .await
        .map_err(|_| ProbeFailure::ResponseReadFailed)?
    {
        shape.observe(&event);
    }
    shape
        .is_success()
        .then_some(())
        .ok_or(ProbeFailure::MissingCanonicalSuccess)
}

#[tokio::test]
#[ignore = "requires dedicated P8_07_* authorization, one Official API key, one target, one mode, and one-request configuration"]
async fn authorized_official_probe_uses_one_target_one_mode_and_one_send() -> TestResult {
    let prepared = AuthorizedProbeConfig::from_environment()?.prepare()?;
    let target_label = prepared.target_label().to_owned();
    let mode = prepared.mode().as_str();
    println!("p8-07 official probe target={target_label} mode={mode} result=started");
    match execute_one_probe(prepared).await {
        Ok(()) => {
            println!("p8-07 official probe target={target_label} mode={mode} result=pass");
            Ok(())
        }
        Err(error) => {
            println!(
                "p8-07 official probe target={target_label} mode={mode} result=stopped outcome={}",
                error.safe_outcome()
            );
            Err(error.into())
        }
    }
}

#[test]
fn missing_authorization_stops_before_external_configuration_is_read() {
    assert!(matches!(
        config_from_map(&BTreeMap::new()),
        Err(ProbeConfigError::NotAuthorized)
    ));
}

#[test]
fn invalid_request_cap_stops_before_preparation() {
    let mut values = complete_values();
    values.insert(REQUEST_CAP_ENV.to_owned(), "2".to_owned());
    assert!(matches!(
        config_from_map(&values),
        Err(ProbeConfigError::InvalidRequestCap)
    ));
}

#[test]
fn complete_synthetic_configuration_prepares_without_network() -> TestResult {
    let prepared = config_from_map(&complete_values())?.prepare()?;
    assert_eq!(prepared.target_label(), "p8-official-test");
    assert_eq!(prepared.mode(), ProbeMode::NonStreaming);
    Ok(())
}

fn config_from_map(
    values: &BTreeMap<String, String>,
) -> Result<AuthorizedProbeConfig, ProbeConfigError> {
    AuthorizedProbeConfig::from_values(&mut |name| values.get(name).cloned())
}

fn complete_values() -> BTreeMap<String, String> {
    BTreeMap::from([
        (AUTHORIZATION_ENV.to_owned(), AUTHORIZATION_VALUE.to_owned()),
        (REQUEST_CAP_ENV.to_owned(), "1".to_owned()),
        (TARGET_LABEL_ENV.to_owned(), "p8-official-test".to_owned()),
        (MODE_ENV.to_owned(), "non_streaming".to_owned()),
        (
            API_KEY_ENV.to_owned(),
            "synthetic-official-api-key-012345".to_owned(),
        ),
        (
            UPSTREAM_MODEL_ENV.to_owned(),
            "grok-official-test".to_owned(),
        ),
    ])
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

fn probe_payload(mode: ProbeMode) -> String {
    format!(
        r#"{{"model":"p8-07-official-probe","input":"Reply with exactly: ready","max_output_tokens":32,"stream":{}}}"#,
        matches!(mode, ProbeMode::Sse)
    )
}
