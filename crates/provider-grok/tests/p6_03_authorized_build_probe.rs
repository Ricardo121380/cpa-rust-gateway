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
    fs::File,
    io::Read,
    num::NonZeroUsize,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
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
    GROK_BUILD_OAUTH_ISSUER, GROK_BUILD_PUBLIC_CLIENT_ID, GROK_BUILD_RESPONSES_URL,
    GrokBuildCredential, GrokBuildResponsesDecoder, GrokBuildResponsesHttpError,
    GrokBuildResponsesOutboundRequest, GrokBuildResponsesRequestBuilder,
    GrokBuildResponsesStreamDecoder, MAX_GROK_BUILD_ERROR_BODY_BYTES,
    MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES,
};
use url::Url;
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn Error>>;

const AUTHORIZATION_ENV: &str = "P6_03_LIVE_AUTHORIZATION";
const AUTHORIZATION_VALUE: &str = "single-probe-approved";
const LOCAL_CACHE_PREFLIGHT_ENV: &str = "P6_03_LOCAL_CACHE_PREFLIGHT";
const LOCAL_CACHE_PREFLIGHT_VALUE: &str = "cache-preflight-approved";
const REQUEST_CAP_ENV: &str = "P6_03_MAX_EXTERNAL_REQUESTS";
const TARGET_LABEL_ENV: &str = "P6_03_TARGET_LABEL";
const MODE_ENV: &str = "P6_03_MODE";
const OAUTH_CREDENTIAL_JSON_ENV: &str = "P6_03_OAUTH_CREDENTIAL_JSON";
const OFFICIAL_CLI_AUTH_CACHE_PATH_ENV: &str = "P6_03_OFFICIAL_CLI_AUTH_CACHE_PATH";
const UPSTREAM_MODEL_ENV: &str = "P6_03_UPSTREAM_MODEL";
const NETWORK_PROFILE_ENV: &str = "P6_03_NETWORK_PROFILE";
const SOCKS5_PROXY_ENV: &str = "P6_03_SOCKS5_PROXY_URL";
const ALLOWED_CIDR_ENV: &str = "P6_03_ALLOWED_CIDR";

const EXTERNAL_REQUEST_CAP: u8 = 1;
const MAX_OFFICIAL_CLI_AUTH_CACHE_BYTES: usize = 64 * 1024;
const MAX_OFFICIAL_CLI_AUTH_CACHE_READ_BYTES: u64 = 64 * 1024 + 1;
const PROBE_MAX_OUTPUT_TOKENS: u64 = 32;
const MAX_GROK_BUILD_PROBE_STREAM_BYTES: usize = 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TTFB_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(20);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(45);

static TEST_CACHE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    ConflictingCredentialSources,
    InvalidCredentialCachePath,
    CredentialCacheReadFailed,
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
            Self::ConflictingCredentialSources => {
                "P6-03 Build probe must select exactly one credential source"
            }
            Self::InvalidCredentialCachePath => {
                "P6-03 Build probe official CLI cache path is invalid"
            }
            Self::CredentialCacheReadFailed => {
                "P6-03 Build probe official CLI cache cannot be read safely"
            }
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

/// A whitelisted application-error output category that carries no upstream value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SafeErrorCategory {
    Model,
    Credential,
    Request,
    Quota,
    Unrecognized,
}

impl SafeErrorCategory {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Credential => "credential",
            Self::Request => "request",
            Self::Quota => "quota",
            Self::Unrecognized => "unrecognized",
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
    credential_input: ProbeCredentialInput,
    upstream_model: String,
    allowed_cidr: Option<EgressCidr>,
    proxy: UpstreamProxy,
}

enum ProbeCredentialInput {
    Json(Zeroizing<String>),
    OfficialCliAuthCachePath(PathBuf),
}

impl ProbeCredentialInput {
    fn import(&self, now_ms: i64) -> Result<GrokBuildCredential, ProbeConfigError> {
        match self {
            Self::Json(credential_json) => {
                GrokBuildCredential::import_json(credential_json.as_bytes(), now_ms)
                    .map_err(|_| ProbeConfigError::InvalidCredential)
            }
            Self::OfficialCliAuthCachePath(path) => {
                let mut reader = File::open(path)
                    .map_err(|_| ProbeConfigError::CredentialCacheReadFailed)?
                    .take(MAX_OFFICIAL_CLI_AUTH_CACHE_READ_BYTES);
                let mut cache = Zeroizing::new(Vec::new());
                reader
                    .read_to_end(&mut cache)
                    .map_err(|_| ProbeConfigError::CredentialCacheReadFailed)?;
                if cache.len() > MAX_OFFICIAL_CLI_AUTH_CACHE_BYTES {
                    return Err(ProbeConfigError::InvalidCredential);
                }
                GrokBuildCredential::import_official_cli_auth_cache(cache.as_slice(), now_ms)
                    .map_err(|_| ProbeConfigError::InvalidCredential)
            }
        }
    }
}

impl AuthorizedProbeConfig {
    fn from_environment() -> Result<Self, ProbeConfigError> {
        Self::from_values(&mut |name| env::var(name).ok())
    }

    fn from_local_cache_preflight_environment() -> Result<Self, ProbeConfigError> {
        if env::var(LOCAL_CACHE_PREFLIGHT_ENV).ok().as_deref() != Some(LOCAL_CACHE_PREFLIGHT_VALUE)
        {
            return Err(ProbeConfigError::NotAuthorized);
        }
        let config = Self::from_values(&mut |name| {
            if name == AUTHORIZATION_ENV {
                Some(AUTHORIZATION_VALUE.to_owned())
            } else {
                env::var(name).ok()
            }
        })?;
        if !matches!(
            &config.credential_input,
            ProbeCredentialInput::OfficialCliAuthCachePath(_)
        ) {
            return Err(ProbeConfigError::InvalidCredential);
        }
        Ok(config)
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
        let credential_input = match (
            read(OAUTH_CREDENTIAL_JSON_ENV),
            read(OFFICIAL_CLI_AUTH_CACHE_PATH_ENV),
        ) {
            (Some(_), Some(_)) => return Err(ProbeConfigError::ConflictingCredentialSources),
            (Some(credential_json), None) => {
                ProbeCredentialInput::Json(Zeroizing::new(credential_json))
            }
            (None, Some(path)) => {
                let path = PathBuf::from(path);
                if !path.is_absolute() {
                    return Err(ProbeConfigError::InvalidCredentialCachePath);
                }
                ProbeCredentialInput::OfficialCliAuthCachePath(path)
            }
            (None, None) => return Err(ProbeConfigError::MissingRequiredValue),
        };
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
            credential_input,
            upstream_model,
            allowed_cidr,
            proxy,
        })
    }

    fn prepare(self) -> Result<PreparedProbe, ProbeConfigError> {
        let now_ms = now_ms()?;
        let credential = self.credential_input.import(now_ms)?;
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
        if let Some(category) = safe_error_category(&body) {
            println!(
                "p6-03 build probe response target={target_label} error_category={}",
                category.as_str()
            );
        }
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
            let content_encoding = response
                .header("content-encoding")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let decoded = GrokBuildResponsesDecoder::decode_non_streaming_with_content_encoding(
                content_encoding.as_deref(),
                &body,
            )
            .map_err(|_| ProbeError::ResponseProtocolFailed)?;
            println!(
                "p6-03 build probe response target={target_label} body_shape=completed_responses_shape"
            );
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
        2 => "2xx",
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

fn safe_error_category(body: &[u8]) -> Option<SafeErrorCategory> {
    let root = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let root = root.as_object()?;
    let error = root.get("error")?;
    let Some(error) = error.as_object() else {
        return Some(SafeErrorCategory::Unrecognized);
    };
    if error.get("param").and_then(serde_json::Value::as_str) == Some("model") {
        return Some(SafeErrorCategory::Model);
    }
    for field in ["code", "type"] {
        if let Some(value) = error.get(field).and_then(serde_json::Value::as_str)
            && let Some(category) = classify_standard_error_value(value)
        {
            return Some(category);
        }
    }
    Some(SafeErrorCategory::Unrecognized)
}

fn classify_standard_error_value(value: &str) -> Option<SafeErrorCategory> {
    match value {
        "model_not_found" | "invalid_model" | "model_not_supported" | "unsupported_model" => {
            Some(SafeErrorCategory::Model)
        }
        "invalid_token"
        | "invalid_api_key"
        | "authentication_error"
        | "unauthorized"
        | "invalid_grant"
        | "invalid_client"
        | "forbidden" => Some(SafeErrorCategory::Credential),
        "invalid_request"
        | "invalid_request_error"
        | "bad_request"
        | "validation_error"
        | "unsupported_parameter"
        | "invalid_input" => Some(SafeErrorCategory::Request),
        "rate_limit_error"
        | "rate_limit_exceeded"
        | "insufficient_quota"
        | "quota_exceeded"
        | "billing_hard_limit_reached" => Some(SafeErrorCategory::Quota),
        _ => None,
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
fn official_cli_cache_source_is_file_only_bounded_and_exclusive() -> TestResult {
    const OBSERVED_AT_MS: i64 = 1_735_689_600_000;

    let cache = SyntheticCliCacheFile::new()?;
    let mut cache_values = synthetic_values();
    cache_values.remove(OAUTH_CREDENTIAL_JSON_ENV);
    cache_values.insert(
        OFFICIAL_CLI_AUTH_CACHE_PATH_ENV.to_owned(),
        cache.path().to_string_lossy().into_owned(),
    );
    let config = config_from_map(&cache_values)?;
    let credential = config.credential_input.import(OBSERVED_AT_MS)?;
    assert_eq!(
        credential.source(),
        provider_grok::GrokBuildCredentialSource::OfficialCliAuthCache
    );
    let diagnostic = format!("{credential:?}");
    for private_value in [
        "synthetic_cli_cache_access_012345",
        "synthetic_cli_cache_refresh_012345",
    ] {
        assert!(!diagnostic.contains(private_value));
    }

    let mut conflicting = cache_values.clone();
    conflicting.insert(
        OAUTH_CREDENTIAL_JSON_ENV.to_owned(),
        r#"{"access_token":"synthetic_build_access_token_012345","refresh_token":"synthetic_build_refresh_token_012345","expires_in":3600}"#.to_owned(),
    );
    assert!(matches!(
        config_from_map(&conflicting),
        Err(ProbeConfigError::ConflictingCredentialSources)
    ));

    let mut relative_path = synthetic_values();
    relative_path.remove(OAUTH_CREDENTIAL_JSON_ENV);
    relative_path.insert(
        OFFICIAL_CLI_AUTH_CACHE_PATH_ENV.to_owned(),
        "relative-cache.json".to_owned(),
    );
    assert!(matches!(
        config_from_map(&relative_path),
        Err(ProbeConfigError::InvalidCredentialCachePath)
    ));

    let oversized_cache = SyntheticCliCacheFile::oversized()?;
    let mut oversized_values = synthetic_values();
    oversized_values.remove(OAUTH_CREDENTIAL_JSON_ENV);
    oversized_values.insert(
        OFFICIAL_CLI_AUTH_CACHE_PATH_ENV.to_owned(),
        oversized_cache.path().to_string_lossy().into_owned(),
    );
    let oversized = config_from_map(&oversized_values)?;
    assert!(matches!(
        oversized.credential_input.import(OBSERVED_AT_MS),
        Err(ProbeConfigError::InvalidCredential)
    ));
    Ok(())
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

#[test]
fn safe_status_class_includes_success_without_rendering_status_values() {
    assert_eq!(safe_status_class(200), "2xx");
    assert_eq!(safe_status_class(429), "4xx");
    assert_eq!(safe_status_class(503), "5xx");
}

#[test]
fn safe_error_category_uses_only_a_fixed_whitelist() {
    let model = br#"{"error":{"code":"model_not_found","message":"private model detail"}}"#;
    let credential = br#"{"error":{"type":"invalid_token","message":"private token detail"}}"#;
    let request =
        br#"{"error":{"type":"invalid_request_error","message":"private request detail"}}"#;
    let quota = br#"{"error":{"code":"quota_exceeded","message":"private quota detail"}}"#;
    let unknown = br#"{"error":{"code":"private-unknown-code","message":"private error detail"}}"#;

    assert_eq!(safe_error_category(model), Some(SafeErrorCategory::Model));
    assert_eq!(
        safe_error_category(credential),
        Some(SafeErrorCategory::Credential)
    );
    assert_eq!(
        safe_error_category(request),
        Some(SafeErrorCategory::Request)
    );
    assert_eq!(safe_error_category(quota), Some(SafeErrorCategory::Quota));
    assert_eq!(
        safe_error_category(unknown),
        Some(SafeErrorCategory::Unrecognized)
    );
    assert_eq!(SafeErrorCategory::Unrecognized.as_str(), "unrecognized");
    for private_value in ["private-unknown-code", "private error detail"] {
        assert!(
            !SafeErrorCategory::Unrecognized
                .as_str()
                .contains(private_value)
        );
    }
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

#[test]
#[ignore = "requires explicit P6_03_LOCAL_CACHE_PREFLIGHT and one official-CLI cache path; no network"]
fn official_cli_cache_preflight_builds_without_network() -> TestResult {
    let prepared = AuthorizedProbeConfig::from_local_cache_preflight_environment()?.prepare()?;
    let target_label = prepared.target_label();
    let mode = prepared.mode().as_str();
    println!("p6-03 local cache preflight target={target_label} mode={mode} result=pass");
    Ok(())
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

struct SyntheticCliCacheFile {
    path: PathBuf,
}

impl SyntheticCliCacheFile {
    fn new() -> Result<Self, std::io::Error> {
        let cache = format!(
            r#"{{
                "{GROK_BUILD_OAUTH_ISSUER}::{GROK_BUILD_PUBLIC_CLIENT_ID}":{{
                    "key":"synthetic_cli_cache_access_012345",
                    "refresh_token":"synthetic_cli_cache_refresh_012345",
                    "expires_at":"2025-01-01T00:00:10Z"
                }}
            }}"#
        );
        Self::write(cache.as_bytes())
    }

    fn oversized() -> Result<Self, std::io::Error> {
        Self::write(&vec![b' '; MAX_OFFICIAL_CLI_AUTH_CACHE_BYTES + 1])
    }

    fn write(contents: &[u8]) -> Result<Self, std::io::Error> {
        let sequence = TEST_CACHE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "provider-grok-p6-03-synthetic-cache-{}-{sequence}.json",
            std::process::id()
        ));
        std::fs::write(&path, contents)?;
        Ok(Self { path })
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for SyntheticCliCacheFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
