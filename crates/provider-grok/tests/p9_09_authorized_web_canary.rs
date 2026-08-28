//! One explicit, bounded Grok Web Canary for P9-09/G9.
//!
//! The live test is ignored unless its dedicated authorization, one-request cap, opaque target
//! label, temporary SSO token, current exact-path Statsig signature, and loopback-only SOCKS5
//! egress profile are all present. The profile is assembled explicitly by the P9-09 operator;
//! this test performs no browser/profile discovery, server mutation, retry, or credential rotation.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fmt,
    net::{IpAddr, Ipv4Addr},
    num::NonZeroUsize,
    sync::atomic::{AtomicU8, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gateway_core::EgressPolicyId;
use gateway_upstream::{
    EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme,
    RedirectPolicy, SystemEgressDnsResolver, UpstreamClientPool, UpstreamHttpResponse,
    UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};
use provider_grok::{
    GROK_WEB_CANARY_HOST, GROK_WEB_CANARY_URL, GrokWebAccountEvidence, GrokWebBrowserEgressSession,
    GrokWebBrowserUserAgent, GrokWebCanaryRequestBuilder, GrokWebCredential,
    GrokWebEgressSessionId, GrokWebFailureAction, GrokWebLiveStreamDecoder,
    GrokWebStatsigSignature, GrokWebTlsProfile, MAX_GROK_WEB_CANARY_REQUEST_BYTES,
    classify_grok_web_http_failure,
};
use serde_json::Value;
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn Error>>;

const AUTHORIZATION_ENV: &str = "P9_09_LIVE_AUTHORIZATION";
const AUTHORIZATION_VALUE: &str = "grok2api-web-canary-approved";
const REQUEST_CAP_ENV: &str = "P9_09_MAX_EXTERNAL_REQUESTS";
const TARGET_LABEL_ENV: &str = "P9_09_TARGET_LABEL";
const SSO_TOKEN_ENV: &str = "P9_09_GROK2API_SSO_TOKEN";
const STATSIG_ID_ENV: &str = "P9_09_STATSIG_ID";
const EGRESS_SOCKS5_ENV: &str = "P9_09_EGRESS_SOCKS5";

const EXTERNAL_REQUEST_CAP: u8 = 1;
const MAX_SSO_TOKEN_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TTFB_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(20);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(45);
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";

static EXTERNAL_SENDS: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeConfigError {
    NotAuthorized,
    MissingRequiredValue,
    InvalidRequestCap,
    InvalidTargetLabel,
    InvalidSsoToken,
    InvalidStatsigSignature,
    InvalidEgressProxy,
    SystemClock,
    InvalidCredential,
    InvalidEgressPolicy,
    InvalidTimeouts,
    InvalidCanaryRequest,
}

impl fmt::Display for ProbeConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotAuthorized => "P9-09 Web Canary is not explicitly authorized",
            Self::MissingRequiredValue => "P9-09 Web Canary configuration is incomplete",
            Self::InvalidRequestCap => "P9-09 Web Canary must use its fixed one-request cap",
            Self::InvalidTargetLabel => "P9-09 Web Canary target label is not opaque",
            Self::InvalidSsoToken => "P9-09 Web Canary SSO token is invalid",
            Self::InvalidStatsigSignature => "P9-09 Web Canary Statsig signature is invalid",
            Self::InvalidEgressProxy => "P9-09 Web Canary egress proxy is invalid",
            Self::SystemClock => "P9-09 Web Canary clock is unavailable",
            Self::InvalidCredential => "P9-09 Web Canary temporary credential is invalid",
            Self::InvalidEgressPolicy => "P9-09 Web Canary egress policy is invalid",
            Self::InvalidTimeouts => "P9-09 Web Canary timeout profile is invalid",
            Self::InvalidCanaryRequest => "P9-09 Web Canary fixed request is invalid",
        })
    }
}

impl Error for ProbeConfigError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SafeStatusClass {
    Informational,
    Success,
    Redirect,
    ClientError,
    ServerError,
    Invalid,
}

impl SafeStatusClass {
    const fn from_status(status: u16) -> Self {
        match status {
            100..=199 => Self::Informational,
            200..=299 => Self::Success,
            300..=399 => Self::Redirect,
            400..=499 => Self::ClientError,
            500..=599 => Self::ServerError,
            _ => Self::Invalid,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Informational => "1xx",
            Self::Success => "2xx",
            Self::Redirect => "3xx",
            Self::ClientError => "4xx",
            Self::ServerError => "5xx",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SafeContentType {
    Json,
    EventStream,
    OtherOrMissing,
}

impl SafeContentType {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::EventStream => "event_stream",
            Self::OtherOrMissing => "other_or_missing",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SafeBodyShape {
    Empty,
    ConversationFrame,
    ConversationFrameSequence,
    ErrorLikeObject,
    OtherObject,
    ObjectSequence,
    Array,
    Scalar,
    InvalidJson,
}

impl SafeBodyShape {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::ConversationFrame => "conversation_frame",
            Self::ConversationFrameSequence => "conversation_frame_sequence",
            Self::ErrorLikeObject => "error_like_object",
            Self::OtherObject => "other_object",
            Self::ObjectSequence => "object_sequence",
            Self::Array => "array",
            Self::Scalar => "scalar",
            Self::InvalidJson => "invalid_json",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SafeFailureAction {
    None,
    RequireReauthorization,
    RebuildEgressSession,
    CoolProvider,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SafeCanonicalProjection {
    CompleteTextLifecycle,
    NotAccepted,
}

impl SafeCanonicalProjection {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CompleteTextLifecycle => "complete_text_lifecycle",
            Self::NotAccepted => "not_accepted",
        }
    }
}

impl SafeFailureAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::RequireReauthorization => "require_reauthorization",
            Self::RebuildEgressSession => "rebuild_egress_session",
            Self::CoolProvider => "cool_provider",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SafeObservation {
    status: SafeStatusClass,
    content_type: SafeContentType,
    body: SafeBodyShape,
    canonical: SafeCanonicalProjection,
    failure_action: SafeFailureAction,
}

impl SafeObservation {
    fn is_accepted(self) -> bool {
        self.status == SafeStatusClass::Success
            && matches!(
                self.body,
                SafeBodyShape::ConversationFrame | SafeBodyShape::ConversationFrameSequence
            )
            && self.canonical == SafeCanonicalProjection::CompleteTextLifecycle
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeFailure {
    EgressAdmissionFailed,
    TransportFailed,
    ResponseReadFailed,
    ResponseTooLarge,
    ProtocolNotAccepted(SafeObservation),
    InternalInvariant,
}

impl ProbeFailure {
    const fn safe_outcome(self) -> &'static str {
        match self {
            Self::EgressAdmissionFailed => "egress_admission_failed",
            Self::TransportFailed => "transport_failed",
            Self::ResponseReadFailed => "response_read_failed",
            Self::ResponseTooLarge => "response_too_large",
            Self::ProtocolNotAccepted(_) => "protocol_not_accepted",
            Self::InternalInvariant => "internal_invariant",
        }
    }
}

impl fmt::Display for ProbeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "P9-09 Web Canary stopped: {}",
            self.safe_outcome()
        )
    }
}

impl Error for ProbeFailure {}

struct AuthorizedProbeConfig {
    target_label: String,
    session: GrokWebBrowserEgressSession,
    statsig_signature: GrokWebStatsigSignature,
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
        let token = Zeroizing::new(required_value(read, SSO_TOKEN_ENV)?);
        let statsig_signature =
            GrokWebStatsigSignature::try_new(&required_value(read, STATSIG_ID_ENV)?)
                .map_err(|_| ProbeConfigError::InvalidStatsigSignature)?;
        let proxy = loopback_socks_proxy(&required_value(read, EGRESS_SOCKS5_ENV)?)?;
        let session = session_from_sso_token(token, proxy.clone())?;
        Ok(Self {
            target_label,
            session,
            statsig_signature,
            proxy,
        })
    }

    fn prepare(self) -> Result<PreparedProbe, ProbeConfigError> {
        let outbound =
            GrokWebCanaryRequestBuilder::build(&self.session, self.statsig_signature, now_ms()?)
                .map_err(|_| ProbeConfigError::InvalidCanaryRequest)?;
        let egress_policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("p9-09-web-canary-egress")
                .map_err(|_| ProbeConfigError::InvalidEgressPolicy)?,
            name: "P9 authorized Grok Web Canary policy".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Https]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new(GROK_WEB_CANARY_HOST)
                .map_err(|_| ProbeConfigError::InvalidEgressPolicy)?]),
            allowed_ports: BTreeSet::from([443]),
            allowed_cidrs: BTreeSet::new(),
            redirect_policy: RedirectPolicy::Deny,
        })
        .map_err(|_| ProbeConfigError::InvalidEgressPolicy)?;
        let timeouts =
            UpstreamTimeouts::try_new(CONNECT_TIMEOUT, TTFB_TIMEOUT, IDLE_TIMEOUT, TOTAL_TIMEOUT)
                .map_err(|_| ProbeConfigError::InvalidTimeouts)?;
        let profile = UpstreamTransportProfile::new(
            timeouts,
            self.proxy,
            NonZeroUsize::new(1).ok_or(ProbeConfigError::InvalidTimeouts)?,
        );
        Ok(PreparedProbe {
            target_label: self.target_label,
            outbound,
            egress_policy,
            client_pool: UpstreamClientPool::new(
                NonZeroUsize::new(1).ok_or(ProbeConfigError::InvalidTimeouts)?,
            ),
            profile,
        })
    }
}

struct PreparedProbe {
    target_label: String,
    outbound: provider_grok::GrokWebCanaryOutboundRequest,
    egress_policy: EgressPolicy,
    client_pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
}

impl PreparedProbe {
    fn target_label(&self) -> &str {
        &self.target_label
    }
}

async fn execute_one_probe(probe: PreparedProbe) -> Result<SafeObservation, ProbeFailure> {
    let target = probe
        .egress_policy
        .admit_url(GROK_WEB_CANARY_URL, &SystemEgressDnsResolver)
        .map_err(|_| ProbeFailure::EgressAdmissionFailed)?;
    let request = probe
        .outbound
        .into_transport_request(target)
        .map_err(|_| ProbeFailure::InternalInvariant)?;
    let previous = EXTERNAL_SENDS.fetch_add(1, Ordering::SeqCst);
    if previous != 0 {
        return Err(ProbeFailure::InternalInvariant);
    }
    let mut response = probe
        .client_pool
        .send(request, &probe.profile)
        .await
        .map_err(|_| ProbeFailure::TransportFailed)?;
    let status_code = response.status();
    let body = read_bounded(&mut response).await?;
    let observation = SafeObservation {
        status: SafeStatusClass::from_status(status_code),
        content_type: safe_content_type(&response),
        body: safe_body_shape(&body),
        canonical: safe_canonical_projection(&body),
        failure_action: safe_failure_action(status_code),
    };
    if observation.is_accepted() {
        Ok(observation)
    } else {
        Err(ProbeFailure::ProtocolNotAccepted(observation))
    }
}

fn safe_canonical_projection(body: &[u8]) -> SafeCanonicalProjection {
    let mut decoder = GrokWebLiveStreamDecoder::new();
    let Ok(events) = decoder.push_bytes(body) else {
        return SafeCanonicalProjection::NotAccepted;
    };
    let Ok(terminal) = decoder.finish() else {
        return SafeCanonicalProjection::NotAccepted;
    };
    let mut response_started = false;
    let mut text_seen = false;
    let mut response_ended = false;
    for event in events.into_iter().chain(terminal) {
        match event {
            gateway_core::CanonicalEvent::ResponseStart(_) => response_started = true,
            gateway_core::CanonicalEvent::TextDelta(_) => text_seen = true,
            gateway_core::CanonicalEvent::ResponseEnd(_) => response_ended = true,
            _ => {}
        }
    }
    if response_started && text_seen && response_ended {
        SafeCanonicalProjection::CompleteTextLifecycle
    } else {
        SafeCanonicalProjection::NotAccepted
    }
}

async fn read_bounded(
    response: &mut UpstreamHttpResponse,
) -> Result<Zeroizing<Vec<u8>>, ProbeFailure> {
    let mut body = Zeroizing::new(Vec::new());
    while let Some(chunk) = response
        .next_chunk()
        .await
        .map_err(|_| ProbeFailure::ResponseReadFailed)?
    {
        let new_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or(ProbeFailure::ResponseTooLarge)?;
        if new_len > MAX_RESPONSE_BYTES {
            return Err(ProbeFailure::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn safe_content_type(response: &UpstreamHttpResponse) -> SafeContentType {
    let value = response
        .header("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let media_type = value.split(';').next().unwrap_or_default().trim();
    if media_type.eq_ignore_ascii_case("application/json") {
        SafeContentType::Json
    } else if media_type.eq_ignore_ascii_case("text/event-stream") {
        SafeContentType::EventStream
    } else {
        SafeContentType::OtherOrMissing
    }
}

fn safe_body_shape(body: &[u8]) -> SafeBodyShape {
    if body.is_empty() {
        return SafeBodyShape::Empty;
    }
    let mut values = serde_json::Deserializer::from_slice(body).into_iter::<Value>();
    let mut object_count = 0_u8;
    let mut value_count = 0_u8;
    let mut contains_error = false;
    let mut contains_conversation_frame = false;
    let mut first_non_object = None;
    for value in &mut values {
        let Ok(value) = value else {
            return SafeBodyShape::InvalidJson;
        };
        value_count = value_count.saturating_add(1);
        match value {
            Value::Object(object) => {
                object_count = object_count.saturating_add(1);
                contains_error |= object.contains_key("error");
                contains_conversation_frame |= is_conversation_frame(&object);
            }
            Value::Array(_) => {
                first_non_object.get_or_insert(SafeBodyShape::Array);
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                first_non_object.get_or_insert(SafeBodyShape::Scalar);
            }
        }
    }
    if value_count == 0 {
        return SafeBodyShape::InvalidJson;
    }
    if value_count == 1 {
        if contains_error {
            return SafeBodyShape::ErrorLikeObject;
        }
        if contains_conversation_frame {
            return SafeBodyShape::ConversationFrame;
        }
        if object_count == 1 {
            return SafeBodyShape::OtherObject;
        }
        return first_non_object.unwrap_or(SafeBodyShape::InvalidJson);
    }
    if contains_error {
        SafeBodyShape::ErrorLikeObject
    } else if contains_conversation_frame && object_count == value_count {
        SafeBodyShape::ConversationFrameSequence
    } else if object_count == value_count {
        SafeBodyShape::ObjectSequence
    } else {
        SafeBodyShape::InvalidJson
    }
}

fn is_conversation_frame(object: &serde_json::Map<String, Value>) -> bool {
    let Some(result) = object.get("result").and_then(Value::as_object) else {
        return false;
    };
    result
        .get("conversation")
        .and_then(Value::as_object)
        .and_then(|conversation| conversation.get("conversationId"))
        .and_then(Value::as_str)
        .is_some()
        && result
            .get("response")
            .and_then(Value::as_object)
            .and_then(|response| response.get("token"))
            .and_then(Value::as_str)
            .is_some()
}

fn safe_failure_action(status: u16) -> SafeFailureAction {
    match classify_grok_web_http_failure(status, GrokWebAccountEvidence::None) {
        Ok(disposition) => match disposition.action() {
            GrokWebFailureAction::None => SafeFailureAction::None,
            GrokWebFailureAction::RequireReauthorization => {
                SafeFailureAction::RequireReauthorization
            }
            GrokWebFailureAction::RebuildEgressSession => SafeFailureAction::RebuildEgressSession,
            GrokWebFailureAction::CoolProvider => SafeFailureAction::CoolProvider,
            GrokWebFailureAction::MarkExactAccountForbidden => SafeFailureAction::Unknown,
        },
        Err(_) => SafeFailureAction::Unknown,
    }
}

fn session_from_sso_token(
    mut token: Zeroizing<String>,
    proxy: UpstreamProxy,
) -> Result<GrokWebBrowserEgressSession, ProbeConfigError> {
    let token = normalize_sso_token(token.as_mut_str())?;
    let now_ms = now_ms()?;
    let expires_at_ms = now_ms
        .checked_add(15 * 60 * 1_000)
        .ok_or(ProbeConfigError::SystemClock)?;
    let export = Zeroizing::new(
        serde_json::to_vec(&serde_json::json!({
            "kind": "grok_web_sso",
            "account_ref": "p9_09_grok2api_web",
            "lineage_ref": "grok2api_export",
            "revision": 1,
            "expires_at_ms": expires_at_ms,
            "cookies": [
                {"name": "sso", "value": token, "domain": ".grok.com", "path": "/", "secure": true, "http_only": true},
                {"name": "sso-rw", "value": token, "domain": ".grok.com", "path": "/", "secure": true, "http_only": true}
            ]
        }))
        .map_err(|_| ProbeConfigError::InvalidCredential)?,
    );
    let credential = GrokWebCredential::import_sso_json(export.as_slice(), now_ms)
        .map_err(|_| ProbeConfigError::InvalidCredential)?;
    GrokWebBrowserEgressSession::try_new(
        GrokWebEgressSessionId::try_new("p9_09_direct_egress")
            .map_err(|_| ProbeConfigError::InvalidCredential)?,
        credential,
        GrokWebBrowserUserAgent::try_new(USER_AGENT)
            .map_err(|_| ProbeConfigError::InvalidCredential)?,
        GrokWebTlsProfile::try_new("reqwest_native_tls_p9_canary")
            .map_err(|_| ProbeConfigError::InvalidCredential)?,
        proxy,
        now_ms,
    )
    .map_err(|_| ProbeConfigError::InvalidCredential)
}

fn loopback_socks_proxy(value: &str) -> Result<UpstreamProxy, ProbeConfigError> {
    let parsed = url::Url::parse(value).map_err(|_| ProbeConfigError::InvalidEgressProxy)?;
    let host = parsed
        .host_str()
        .and_then(|host| host.parse::<IpAddr>().ok())
        .ok_or(ProbeConfigError::InvalidEgressProxy)?;
    if !host.is_loopback() {
        return Err(ProbeConfigError::InvalidEgressProxy);
    }
    UpstreamProxy::try_socks5(value).map_err(|_| ProbeConfigError::InvalidEgressProxy)
}

fn normalize_sso_token(value: &mut str) -> Result<&str, ProbeConfigError> {
    let value = value.trim();
    let value = value.strip_prefix("sso=").unwrap_or(value).trim();
    if value.is_empty()
        || value.len() > MAX_SSO_TOKEN_BYTES
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
        || value.contains(';')
    {
        return Err(ProbeConfigError::InvalidSsoToken);
    }
    Ok(value)
}

fn now_ms() -> Result<i64, ProbeConfigError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProbeConfigError::SystemClock)?;
    i64::try_from(duration.as_millis()).map_err(|_| ProbeConfigError::SystemClock)
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

fn config_from_map(
    values: &BTreeMap<String, String>,
) -> Result<AuthorizedProbeConfig, ProbeConfigError> {
    AuthorizedProbeConfig::from_values(&mut |name| values.get(name).cloned())
}

fn complete_values() -> BTreeMap<String, String> {
    BTreeMap::from([
        (AUTHORIZATION_ENV.to_owned(), AUTHORIZATION_VALUE.to_owned()),
        (REQUEST_CAP_ENV.to_owned(), "1".to_owned()),
        (TARGET_LABEL_ENV.to_owned(), "p9-web-grok2api".to_owned()),
        (
            SSO_TOKEN_ENV.to_owned(),
            "synthetic_sso_token_0123456789".to_owned(),
        ),
        (
            STATSIG_ID_ENV.to_owned(),
            "synthetic_statsig_signature_0123456789".to_owned(),
        ),
        (
            EGRESS_SOCKS5_ENV.to_owned(),
            "socks5://127.0.0.1:7897".to_owned(),
        ),
    ])
}

struct FixedResolver;

impl EgressDnsResolver for FixedResolver {
    fn resolve(&self, _: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))])
    }
}

#[tokio::test]
#[ignore = "requires P9_09_* authorization, one temporary grok2api Web SSO token, one target label, and one-request configuration"]
async fn authorized_web_canary_uses_one_fixed_target_and_one_send() -> TestResult {
    EXTERNAL_SENDS.store(0, Ordering::SeqCst);
    let prepared = AuthorizedProbeConfig::from_environment()?.prepare()?;
    let target_label = prepared.target_label().to_owned();
    println!("p9-09 web canary target={target_label} request_count=1 result=started");
    match execute_one_probe(prepared).await {
        Ok(observation) => {
            println!(
                "p9-09 web canary target={target_label} request_count=1 status={} content_type={} body_shape={} canonical_projection={} failure_action={} result=pass",
                observation.status.as_str(),
                observation.content_type.as_str(),
                observation.body.as_str(),
                observation.canonical.as_str(),
                observation.failure_action.as_str(),
            );
            Ok(())
        }
        Err(ProbeFailure::ProtocolNotAccepted(observation)) => {
            println!(
                "p9-09 web canary target={target_label} request_count=1 status={} content_type={} body_shape={} canonical_projection={} failure_action={} result=stopped outcome=protocol_not_accepted",
                observation.status.as_str(),
                observation.content_type.as_str(),
                observation.body.as_str(),
                observation.canonical.as_str(),
                observation.failure_action.as_str(),
            );
            Err(ProbeFailure::ProtocolNotAccepted(observation).into())
        }
        Err(error) => {
            println!(
                "p9-09 web canary target={target_label} request_count=1 result=stopped outcome={}",
                error.safe_outcome()
            );
            Err(error.into())
        }
    }
}

#[test]
fn missing_authorization_stops_before_any_sso_value_is_read() {
    assert!(matches!(
        config_from_map(&BTreeMap::new()),
        Err(ProbeConfigError::NotAuthorized)
    ));
}

#[test]
fn invalid_cap_or_nonopaque_label_stops_before_preparation() {
    let mut values = complete_values();
    values.insert(REQUEST_CAP_ENV.to_owned(), "2".to_owned());
    assert!(matches!(
        config_from_map(&values),
        Err(ProbeConfigError::InvalidRequestCap)
    ));
    let mut values = complete_values();
    values.insert(
        TARGET_LABEL_ENV.to_owned(),
        "not an opaque label".to_owned(),
    );
    assert!(matches!(
        config_from_map(&values),
        Err(ProbeConfigError::InvalidTargetLabel)
    ));
}

#[test]
fn external_egress_and_unvalidated_signature_are_rejected_before_preparation() {
    let mut values = complete_values();
    values.insert(
        EGRESS_SOCKS5_ENV.to_owned(),
        "socks5://198.51.100.1:1080".to_owned(),
    );
    assert!(matches!(
        config_from_map(&values),
        Err(ProbeConfigError::InvalidEgressProxy)
    ));

    let mut values = complete_values();
    values.insert(STATSIG_ID_ENV.to_owned(), "bad\r\nvalue".to_owned());
    assert!(matches!(
        config_from_map(&values),
        Err(ProbeConfigError::InvalidStatsigSignature)
    ));
}

#[test]
fn complete_synthetic_configuration_has_a_fixed_default_off_and_bounded_request() -> TestResult {
    let prepared = config_from_map(&complete_values())?.prepare()?;
    assert_eq!(prepared.target_label(), "p9-web-grok2api");
    assert!(prepared.outbound.body().len() <= MAX_GROK_WEB_CANARY_REQUEST_BYTES);
    let body: Value = serde_json::from_slice(prepared.outbound.body())?;
    assert_eq!(body["modeId"], "auto");
    assert_eq!(body["temporary"], true);
    assert_eq!(body["disableMemory"], true);
    assert_eq!(body["disableSearch"], true);
    assert_eq!(body["enableImageGeneration"], false);
    assert_eq!(body["enableImageStreaming"], false);
    assert_eq!(body["fileAttachments"], serde_json::json!([]));
    assert!(body.get("tools").is_none());
    assert!(body.get("tool_choice").is_none());
    assert_eq!(prepared.outbound.header("origin"), Some("https://grok.com"));
    assert!(prepared.outbound.header("x-statsig-id").is_some());
    assert!(prepared.outbound.header("x-xai-request-id").is_some());
    assert_eq!(
        prepared.outbound.header("referer"),
        Some("https://grok.com/")
    );
    let diagnostic = format!("{:?}", prepared.outbound);
    for private_value in [
        "synthetic_sso_token_0123456789",
        "synthetic_statsig_signature_0123456789",
        USER_AGENT,
        "Reply with exactly: ready",
        GROK_WEB_CANARY_URL,
    ] {
        assert!(!diagnostic.contains(private_value));
    }
    Ok(())
}

#[test]
fn fixed_target_admission_rejects_query_or_origin_substitution_without_network() -> TestResult {
    let prepared = config_from_map(&complete_values())?.prepare()?;
    let target = prepared
        .egress_policy
        .admit_url(GROK_WEB_CANARY_URL, &FixedResolver)?;
    let request = prepared.outbound.into_transport_request(target)?;
    assert_eq!(request.method(), gateway_upstream::UpstreamHttpMethod::Post);
    assert!(request.body().len() <= MAX_GROK_WEB_CANARY_REQUEST_BYTES);
    let diagnostic = format!("{request:?}");
    assert!(!diagnostic.contains("synthetic_sso_token_0123456789"));
    assert!(!diagnostic.contains(GROK_WEB_CANARY_URL));

    let prepared = config_from_map(&complete_values())?.prepare()?;
    let query_target = prepared.egress_policy.admit_url(
        "https://grok.com/rest/app-chat/conversations/new?unexpected=1",
        &FixedResolver,
    )?;
    assert!(matches!(
        prepared.outbound.into_transport_request(query_target),
        Err(provider_grok::GrokWebCanaryRequestError::TargetMismatch)
    ));
    Ok(())
}

#[test]
fn body_shape_projection_never_retains_response_values() {
    let private_body = br#"{"result":{"conversation":{"conversationId":"private-conversation"},"response":{"token":"private-token","modelResponse":{"message":"private text"}}}}"#;
    assert_eq!(
        safe_body_shape(private_body),
        SafeBodyShape::ConversationFrame
    );
    assert_eq!(
        safe_body_shape(br#"{"error":{"message":"private upstream error"}}"#),
        SafeBodyShape::ErrorLikeObject
    );
    assert_eq!(safe_body_shape(b"not-json"), SafeBodyShape::InvalidJson);
    assert_eq!(
        safe_body_shape(b"{\"result\":{}}\n{\"result\":{}}"),
        SafeBodyShape::ObjectSequence
    );
    assert_eq!(
        safe_body_shape(
            b"{\"result\":{\"conversation\":{\"conversationId\":\"private\"},\"response\":{\"token\":\"private\"}}}\n{\"result\":{}}"
        ),
        SafeBodyShape::ConversationFrameSequence
    );
}

#[test]
fn live_canonical_projection_is_value_free_and_requires_a_complete_lifecycle() {
    let complete = br#"{"result":{"conversation":{"conversationId":"private-conversation"},"response":{"token":"ready"}}}{"result":{"response":{"modelResponse":{"message":"ready"}}}}"#;
    assert_eq!(
        safe_canonical_projection(complete),
        SafeCanonicalProjection::CompleteTextLifecycle
    );
    assert_eq!(
        safe_canonical_projection(br#"{"result":{"conversation":{"conversationId":"private-conversation"},"response":{"token":"ready"}}}"#),
        SafeCanonicalProjection::NotAccepted
    );
}
