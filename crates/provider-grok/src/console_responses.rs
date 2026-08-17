//! Native Grok Console SSO Responses request and response boundary.
//!
//! Console speaks the `OpenAI` Responses wire format but has a distinct fixed browser/SSO request
//! profile. Responses are decoded once into Canonical events by the already reviewed strict xAI
//! Responses codec; public Chat, Responses, and Messages remain outer Canonical projections.

use std::{collections::VecDeque, error::Error, fmt, sync::Arc, time::SystemTime};

use gateway_core::{
    CanonicalEvent, CanonicalRequest, CanonicalResponse, ErrorScope, GatewayError,
    GatewayErrorCode, ProviderId, RawExtensions, RequestContext, StreamError,
};
use gateway_provider::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};
use gateway_upstream::{
    AdmittedEgressTarget, EgressDnsResolver, EgressPolicy, EndpointUrl, UpstreamClientPool,
    UpstreamHttpMethod, UpstreamHttpRequest, UpstreamHttpRequestErrorCode, UpstreamHttpResponse,
    UpstreamTransportProfile,
};
use protocol_openai_responses::ResponseMode;
use serde::Deserialize;
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::provider_egress::GrokNativeEgressEventSource;
use crate::{
    GrokConsoleDpopSession, GrokConsoleDpopSessionCache, GrokNativeEgressAttempt,
    GrokOfficialResponsesDecoder, GrokOfficialResponsesStreamDecoder, grok_console_dpop_cache_key,
    official_responses::encode_responses_body,
};

/// Fixed Console base URL.
pub const GROK_CONSOLE_RESPONSES_BASE_URL: &str = "https://console.x.ai";
/// Fixed Console Responses path.
pub const GROK_CONSOLE_RESPONSES_PATH: &str = "/v1/responses";
/// Complete fixed Console Responses URL.
pub const GROK_CONSOLE_RESPONSES_URL: &str = "https://console.x.ai/v1/responses";
/// Stable native Console provider identity.
pub const GROK_CONSOLE_PROVIDER_ID: &str = "grok.console";
/// Fixed Console cluster declaration from the frozen reference implementation.
pub const GROK_CONSOLE_CLUSTER: &str = "https://us-east-1.api.x.ai";
/// Fixed browser profile paired with the Console request headers.
pub const GROK_CONSOLE_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";

const MAX_CONSOLE_SSO_TOKEN_BYTES: usize = 16 * 1024;
const MAX_CONSOLE_MODEL_BYTES: usize = 512;
const CONSOLE_REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh"];

/// A bounded Console SSO token that never renders through `Debug`.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokConsoleSsoToken(Zeroizing<String>);

/// The bounded canonical envelope emitted by the grok2api memory exporter.
///
/// grok2api's native Console importer extracts `sso_token` from a JSON account document. CPAR's
/// migration stream additionally carries the source-observed model so a staging probe can retain
/// the same model identity without putting it into the cookie value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Grok2ApiConsoleCredentialEnvelope {
    sso_token: String,
    probe_model: String,
}

impl GrokConsoleSsoToken {
    /// Imports one canonical token value from a native account credential.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, non-UTF-8, control-bearing, or cookie-delimited values.
    pub fn try_from_bytes(value: &[u8]) -> Result<Self, GrokConsoleRequestError> {
        let value =
            std::str::from_utf8(value).map_err(|_| GrokConsoleRequestError::InvalidToken)?;
        let value = if value.trim_start().starts_with('{') {
            let envelope = serde_json::from_str::<Grok2ApiConsoleCredentialEnvelope>(value.trim())
                .map_err(|_| GrokConsoleRequestError::InvalidToken)?;
            if observed_probe_model(&envelope.probe_model).is_none() {
                return Err(GrokConsoleRequestError::InvalidToken);
            }
            envelope.sso_token
        } else {
            value.to_owned()
        };
        if value.is_empty()
            || value.len() > MAX_CONSOLE_SSO_TOKEN_BYTES
            || value.trim() != value
            || value
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte == b';')
        {
            return Err(GrokConsoleRequestError::InvalidToken);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    fn cookie_header(&self) -> Zeroizing<String> {
        Zeroizing::new(format!("sso={0}; sso-rw={0}", self.0.as_str()))
    }
}

impl fmt::Debug for GrokConsoleSsoToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokConsoleSsoToken(<redacted>)")
    }
}

/// Safe Console request construction failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokConsoleRequestError {
    /// Native account credential is not one canonical SSO token.
    InvalidToken,
    /// Selected model is outside the frozen Console text catalog.
    UnsupportedModel,
    /// Canonical semantics cannot be represented without loss.
    UnsupportedRequest,
    /// Fixed endpoint construction or JSON normalization failed.
    InternalEncodingFailure,
    /// An independently admitted target differs from the fixed Console endpoint.
    TargetMismatch,
    /// Shared request admission rejected the header/body envelope.
    TransportRequestRejected(UpstreamHttpRequestErrorCode),
}

impl fmt::Display for GrokConsoleRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidToken => "Grok Console credential is invalid",
            Self::UnsupportedModel => "Grok Console model is unsupported",
            Self::UnsupportedRequest => "Grok Console request is unsupported",
            Self::InternalEncodingFailure => "Grok Console request encoding failed",
            Self::TargetMismatch => "Grok Console target does not match the fixed endpoint",
            Self::TransportRequestRejected(_) => "Grok Console transport request was rejected",
        })
    }
}

impl Error for GrokConsoleRequestError {}

/// Redacted fixed-target Console request.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokConsoleResponsesOutboundRequest {
    target: EndpointUrl,
    cookie: Zeroizing<String>,
    authorization: Zeroizing<String>,
    dpop_proof: Option<Zeroizing<String>>,
    accept: &'static str,
    body: Vec<u8>,
}

impl GrokConsoleResponsesOutboundRequest {
    /// Returns the immutable production URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }

    /// Returns one fixed header for contract testing and transport composition.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("accept") {
            Some(self.accept)
        } else if name.eq_ignore_ascii_case("accept-encoding") {
            Some("gzip, deflate, br, zstd")
        } else if name.eq_ignore_ascii_case("accept-language") {
            Some("zh-CN,zh;q=0.9,en;q=0.8")
        } else if name.eq_ignore_ascii_case("authorization") {
            Some(self.authorization.as_str())
        } else if name.eq_ignore_ascii_case("content-type") {
            Some("application/json")
        } else if name.eq_ignore_ascii_case("cookie") {
            Some(self.cookie.as_str())
        } else if name.eq_ignore_ascii_case("origin") {
            Some(GROK_CONSOLE_RESPONSES_BASE_URL)
        } else if name.eq_ignore_ascii_case("referer") {
            Some("https://console.x.ai/")
        } else if name.eq_ignore_ascii_case("priority") {
            Some("u=1, i")
        } else if name.eq_ignore_ascii_case("sec-fetch-dest") {
            Some("empty")
        } else if name.eq_ignore_ascii_case("sec-fetch-mode") {
            Some("cors")
        } else if name.eq_ignore_ascii_case("sec-fetch-site") {
            Some("same-origin")
        } else if name.eq_ignore_ascii_case("user-agent") {
            Some(GROK_CONSOLE_USER_AGENT)
        } else if name.eq_ignore_ascii_case("x-cluster") {
            Some(GROK_CONSOLE_CLUSTER)
        } else if name.eq_ignore_ascii_case("sec-ch-ua") {
            Some("\"Google Chrome\";v=\"146\", \"Chromium\";v=\"146\", \"Not(A:Brand\";v=\"24\"")
        } else if name.eq_ignore_ascii_case("sec-ch-ua-mobile") {
            Some("?0")
        } else if name.eq_ignore_ascii_case("sec-ch-ua-platform") {
            Some("\"macOS\"")
        } else if name.eq_ignore_ascii_case("dpop") {
            self.dpop_proof.as_deref().map(String::as_str)
        } else {
            None
        }
    }

    /// Returns the normalized Responses JSON body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    fn cookie(&self) -> &str {
        self.cookie.as_str()
    }

    /// Consumes the request into the shared DNS-pinned transport envelope.
    ///
    /// # Errors
    ///
    /// Rejects target substitution or an invalid shared header/body shape.
    pub fn into_transport_request(
        self,
        target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GrokConsoleRequestError> {
        if target.request_url() != self.target.as_url() {
            return Err(GrokConsoleRequestError::TargetMismatch);
        }
        let names = [
            "accept",
            "accept-encoding",
            "accept-language",
            "authorization",
            "content-type",
            "cookie",
            "origin",
            "referer",
            "priority",
            "sec-fetch-dest",
            "sec-fetch-mode",
            "sec-fetch-site",
            "user-agent",
            "x-cluster",
            "sec-ch-ua",
            "sec-ch-ua-mobile",
            "sec-ch-ua-platform",
            "dpop",
        ];
        let headers = names
            .into_iter()
            .filter_map(|name| {
                self.header(name)
                    .map(|value| (name.to_owned(), value.to_owned()))
            })
            .collect::<Vec<_>>();
        UpstreamHttpRequest::try_new(target, UpstreamHttpMethod::Post, headers, self.body)
            .map_err(|error| GrokConsoleRequestError::TransportRequestRejected(error.code()))
    }
}

impl fmt::Debug for GrokConsoleResponsesOutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokConsoleResponsesOutboundRequest")
            .field("target", &"<redacted>")
            .field("header_count", &17)
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

impl GrokConsoleResponsesOutboundRequest {
    /// Applies one short-lived `DPoP` session to this request.
    ///
    /// The caller owns session acquisition and caching; this method only creates the proof and
    /// swaps the authorization scheme, keeping token exchange outside request encoding.
    ///
    /// # Errors
    ///
    /// Returns a request encoding error when the session is expired or proof construction fails.
    pub fn with_dpop_session(
        mut self,
        session: &GrokConsoleDpopSession,
        now: SystemTime,
    ) -> Result<Self, GrokConsoleRequestError> {
        let proof = session
            .proof("POST", self.url(), now)
            .map_err(|_| GrokConsoleRequestError::InternalEncodingFailure)?;
        self.authorization = Zeroizing::new(format!("DPoP {}", session.access_token()));
        self.dpop_proof = Some(Zeroizing::new(proof));
        Ok(self)
    }
}

/// Builds the fixed Console request profile from one Canonical request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokConsoleResponsesRequestBuilder;

impl GrokConsoleResponsesRequestBuilder {
    /// Builds one stateless Console Responses request.
    ///
    /// # Errors
    ///
    /// Rejects unsupported model/request semantics before transport.
    pub fn build(
        credential: &GrokConsoleSsoToken,
        upstream_model: &str,
        request: &CanonicalRequest,
        mode: ResponseMode,
    ) -> Result<GrokConsoleResponsesOutboundRequest, GrokConsoleRequestError> {
        let spec =
            console_model(upstream_model).ok_or(GrokConsoleRequestError::UnsupportedModel)?;
        Self::build_with_spec(credential, upstream_model, request, mode, spec)
    }

    /// Builds the text-only P12 migration probe from a source-observed successful model.
    ///
    /// This does not widen the production Console catalog: callers must use the explicitly named
    /// root-only probe path, which applies a 32-token ceiling and adds no inferred capabilities.
    ///
    /// # Errors
    ///
    /// Rejects an invalid observed model or request semantics before transport.
    pub fn build_observed_probe(
        credential: &GrokConsoleSsoToken,
        upstream_model: &str,
        request: &CanonicalRequest,
        mode: ResponseMode,
    ) -> Result<GrokConsoleResponsesOutboundRequest, GrokConsoleRequestError> {
        let spec = observed_probe_model(upstream_model)
            .ok_or(GrokConsoleRequestError::UnsupportedModel)?;
        Self::build_with_spec(credential, upstream_model, request, mode, spec)
    }

    fn build_with_spec(
        credential: &GrokConsoleSsoToken,
        upstream_model: &str,
        request: &CanonicalRequest,
        mode: ResponseMode,
        spec: ConsoleModelSpec,
    ) -> Result<GrokConsoleResponsesOutboundRequest, GrokConsoleRequestError> {
        if upstream_model.len() > MAX_CONSOLE_MODEL_BYTES
            || !upstream_model.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(GrokConsoleRequestError::UnsupportedModel);
        }
        let (request, max_output_tokens) = split_console_output_limit(request, spec)?;
        let encoded =
            encode_responses_body(upstream_model, &request, mode, CONSOLE_REASONING_EFFORTS)
                .map_err(|_| GrokConsoleRequestError::UnsupportedRequest)?;
        let mut root: Value = serde_json::from_slice(&encoded)
            .map_err(|_| GrokConsoleRequestError::InternalEncodingFailure)?;
        let object = root
            .as_object_mut()
            .ok_or(GrokConsoleRequestError::InternalEncodingFailure)?;
        normalize_console_body(object, spec, max_output_tokens);
        let body = serde_json::to_vec(&root)
            .map_err(|_| GrokConsoleRequestError::InternalEncodingFailure)?;
        let target =
            EndpointUrl::compose(GROK_CONSOLE_RESPONSES_BASE_URL, GROK_CONSOLE_RESPONSES_PATH)
                .map_err(|_| GrokConsoleRequestError::InternalEncodingFailure)?;
        Ok(GrokConsoleResponsesOutboundRequest {
            target,
            cookie: credential.cookie_header(),
            authorization: Zeroizing::new("Bearer anonymous".to_owned()),
            dpop_proof: None,
            accept: match mode {
                ResponseMode::NonStreaming => "*/*",
                ResponseMode::Streaming => "text/event-stream",
            },
            body,
        })
    }
}

/// Consumes the one output-limit extension that all three public protocols can project onto the
/// Responses wire format. Other extensions remain in the cloned request and are rejected by the
/// strict shared encoder instead of being silently discarded.
fn split_console_output_limit(
    request: &CanonicalRequest,
    spec: ConsoleModelSpec,
) -> Result<(CanonicalRequest, Option<u64>), GrokConsoleRequestError> {
    const OUTPUT_LIMIT: &str = "openai.responses.max_output_tokens";
    let Some(raw) = request.extensions.get(OUTPUT_LIMIT) else {
        return Ok((request.clone(), None));
    };
    let value = serde_json::from_str::<Value>(raw.get())
        .ok()
        .and_then(|value| value.as_u64())
        .filter(|value| *value > 0 && *value <= spec.maximum_output_tokens)
        .ok_or(GrokConsoleRequestError::UnsupportedRequest)?;
    let mut extensions = RawExtensions::default();
    for (name, extension) in request.extensions.iter() {
        if name != OUTPUT_LIMIT {
            extensions
                .try_insert(name.to_owned(), extension.clone())
                .map_err(|_| GrokConsoleRequestError::InternalEncodingFailure)?;
        }
    }
    let mut projected = request.clone();
    projected.extensions = extensions;
    Ok((projected, Some(value)))
}

fn observed_probe_model(model: &str) -> Option<ConsoleModelSpec> {
    if let Some(spec) = console_model(model) {
        return Some(spec);
    }
    (!model.is_empty()
        && model.len() <= MAX_CONSOLE_MODEL_BYTES
        && model.bytes().all(|byte| byte.is_ascii_graphic()))
    .then_some(ConsoleModelSpec {
        maximum_output_tokens: 32,
        default_reasoning_effort: None,
        search_tools: false,
    })
}

#[derive(Clone, Copy)]
struct ConsoleModelSpec {
    maximum_output_tokens: u64,
    default_reasoning_effort: Option<&'static str>,
    search_tools: bool,
}

fn console_model(model: &str) -> Option<ConsoleModelSpec> {
    let spec = match model {
        "grok-4.3" => ConsoleModelSpec {
            maximum_output_tokens: 1_000_000,
            default_reasoning_effort: Some("medium"),
            search_tools: true,
        },
        "grok-4.20-0309" | "grok-4.20-0309-reasoning" | "grok-4.20-0309-non-reasoning" => {
            ConsoleModelSpec {
                maximum_output_tokens: 1_000_000,
                default_reasoning_effort: None,
                search_tools: true,
            }
        }
        "grok-4.20-multi-agent-0309" => ConsoleModelSpec {
            maximum_output_tokens: 2_000_000,
            default_reasoning_effort: Some("medium"),
            search_tools: true,
        },
        "grok-build-0.1" => ConsoleModelSpec {
            maximum_output_tokens: 256_000,
            default_reasoning_effort: None,
            search_tools: true,
        },
        _ => return None,
    };
    Some(spec)
}

fn normalize_console_body(
    root: &mut Map<String, Value>,
    spec: ConsoleModelSpec,
    max_output_tokens: Option<u64>,
) {
    root.insert("store".to_owned(), Value::Bool(false));
    root.insert(
        "max_output_tokens".to_owned(),
        Value::Number(
            max_output_tokens
                .unwrap_or(spec.maximum_output_tokens)
                .into(),
        ),
    );
    if !root.contains_key("reasoning")
        && let Some(effort) = spec.default_reasoning_effort
    {
        root.insert(
            "reasoning".to_owned(),
            Value::Object(Map::from_iter([(
                "effort".to_owned(),
                Value::String(effort.to_owned()),
            )])),
        );
    }
    root.insert(
        "include".to_owned(),
        Value::Array(vec![Value::String(
            "reasoning.encrypted_content".to_owned(),
        )]),
    );
    if spec.search_tools {
        let tools = root
            .entry("tools".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(tools) = tools {
            tools.insert(
                0,
                Value::Object(Map::from_iter([
                    ("type".to_owned(), Value::String("x_search".to_owned())),
                    ("enable_video_understanding".to_owned(), Value::Bool(true)),
                ])),
            );
            tools.insert(
                0,
                Value::Object(Map::from_iter([
                    ("type".to_owned(), Value::String("web_search".to_owned())),
                    ("enable_image_understanding".to_owned(), Value::Bool(true)),
                ])),
            );
            root.insert("tool_choice".to_owned(), Value::String("auto".to_owned()));
        }
    }
}

/// Strict Console JSON decoder with explicit public-protocol terminal semantics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokConsoleResponsesDecoder;

impl GrokConsoleResponsesDecoder {
    /// Decodes one completed Console Responses object and supplies its lossless terminal reason.
    ///
    /// # Errors
    ///
    /// Returns the underlying strict Responses protocol/lifecycle failure.
    pub fn decode_non_streaming(
        input: &[u8],
    ) -> Result<CanonicalResponse, gateway_core::GatewayError> {
        let response = GrokOfficialResponsesDecoder::decode_non_streaming(input)?;
        let mut events = response.into_events();
        normalize_console_terminal(&mut events);
        CanonicalResponse::try_new(events)
    }
}

/// Strict incremental Console SSE decoder with explicit public-protocol terminal semantics.
#[derive(Clone, Default)]
pub struct GrokConsoleResponsesStreamDecoder {
    inner: GrokOfficialResponsesStreamDecoder,
    saw_tool_call: bool,
}

impl GrokConsoleResponsesStreamDecoder {
    /// Creates an empty Console stream decoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Supplies arbitrary transport chunks.
    ///
    /// # Errors
    ///
    /// Returns the underlying strict Responses stream failure.
    pub fn push_bytes(
        &mut self,
        chunk: &[u8],
    ) -> Result<Vec<CanonicalEvent>, gateway_core::GatewayError> {
        let mut events = self.inner.push_bytes(chunk)?;
        for event in &events {
            if matches!(event, CanonicalEvent::ToolCallStart(_)) {
                self.saw_tool_call = true;
            }
        }
        for event in &mut events {
            if let CanonicalEvent::ResponseEnd(end) = event
                && end.stop_reason.is_none()
            {
                end.stop_reason = Some(if self.saw_tool_call {
                    "tool_use".to_owned()
                } else {
                    "end_turn".to_owned()
                });
            }
        }
        Ok(events)
    }

    /// Confirms the stream ended after a complete terminal event.
    ///
    /// # Errors
    ///
    /// Returns the underlying strict truncation or lifecycle failure.
    pub fn finish(&self) -> Result<(), gateway_core::GatewayError> {
        self.inner.finish()
    }
}

impl fmt::Debug for GrokConsoleResponsesStreamDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokConsoleResponsesStreamDecoder")
            .field("inner", &self.inner)
            .field("saw_tool_call", &self.saw_tool_call)
            .finish()
    }
}

fn normalize_console_terminal(events: &mut [CanonicalEvent]) {
    let saw_tool_call = events
        .iter()
        .any(|event| matches!(event, CanonicalEvent::ToolCallStart(_)));
    for event in events {
        if let CanonicalEvent::ResponseEnd(end) = event
            && end.stop_reason.is_none()
        {
            end.stop_reason = Some(if saw_tool_call {
                "tool_use".to_owned()
            } else {
                "end_turn".to_owned()
            });
        }
    }
}

/// Exact owner of a pre-start Console HTTP failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokConsoleFailureOwner {
    /// Exact account credential must be reauthenticated.
    Credential,
    /// Browser/clearance egress must be rebuilt; credential remains eligible.
    Egress,
    /// Exact account/model quota target is unavailable.
    Quota,
    /// Provider endpoint is transiently unavailable.
    Endpoint,
    /// Request is permanently unsupported or malformed.
    Request,
}

/// Classifies Console status without inspecting or retaining an upstream body.
#[must_use]
pub const fn classify_grok_console_http_failure(
    status: u16,
    definitive_account_block: bool,
) -> GrokConsoleFailureOwner {
    match status {
        400 | 404 | 405 | 409 | 415 | 422 => GrokConsoleFailureOwner::Request,
        401 => GrokConsoleFailureOwner::Credential,
        403 if definitive_account_block => GrokConsoleFailureOwner::Credential,
        403 => GrokConsoleFailureOwner::Egress,
        429 => GrokConsoleFailureOwner::Quota,
        _ => GrokConsoleFailureOwner::Endpoint,
    }
}

/// Projects the upstream Retry-After header into a bounded absolute due instant.
///
/// # Errors
///
/// Rejects non-decimal, zero, oversized, or overflowing values.
pub fn grok_console_retry_after_due_at(
    value: &str,
    observed_at_ms: i64,
) -> Result<i64, GrokConsoleRequestError> {
    if observed_at_ms < 0 || value.is_empty() || value.len() > 10 {
        return Err(GrokConsoleRequestError::UnsupportedRequest);
    }
    let seconds = value
        .parse::<u64>()
        .map_err(|_| GrokConsoleRequestError::UnsupportedRequest)?;
    if seconds == 0 || seconds > 7 * 24 * 60 * 60 {
        return Err(GrokConsoleRequestError::UnsupportedRequest);
    }
    let milliseconds = i64::try_from(seconds.saturating_mul(1_000))
        .map_err(|_| GrokConsoleRequestError::UnsupportedRequest)?;
    observed_at_ms
        .checked_add(milliseconds)
        .ok_or(GrokConsoleRequestError::UnsupportedRequest)
}

/// Selected Console execution representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokConsoleExecutionMode {
    /// Completed JSON response.
    NonStreaming,
    /// Incremental Responses SSE stream.
    Streaming,
}

impl GrokConsoleExecutionMode {
    const fn response_mode(self) -> ResponseMode {
        match self {
            Self::NonStreaming => ResponseMode::NonStreaming,
            Self::Streaming => ResponseMode::Streaming,
        }
    }
}

/// Safe Console response content-type projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokConsoleResponseContentType {
    /// `application/json`.
    Json,
    /// `text/event-stream`.
    EventStream,
    /// Missing or unsupported.
    OtherOrMissing,
}

/// Pull-only Console response body.
pub trait GrokConsoleResponseBody: Send {
    /// Returns the next opaque body chunk.
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>>;
}

/// Injected Console transport response.
pub struct GrokConsoleTransportResponse {
    status: u16,
    content_type: GrokConsoleResponseContentType,
    body: Box<dyn GrokConsoleResponseBody>,
}

impl GrokConsoleTransportResponse {
    /// Creates one status/content-type/body handoff.
    #[must_use]
    pub fn new(
        status: u16,
        content_type: GrokConsoleResponseContentType,
        body: Box<dyn GrokConsoleResponseBody>,
    ) -> Self {
        Self {
            status,
            content_type,
            body,
        }
    }
}

impl fmt::Debug for GrokConsoleTransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokConsoleTransportResponse")
            .field("status", &self.status)
            .field("content_type", &self.content_type)
            .field("body", &"<streaming>")
            .finish()
    }
}

/// Sends one already-built Console request without implicit retries or account fallback.
pub trait GrokConsoleTransport: Send + Sync {
    /// Executes exactly one request.
    fn send(
        &self,
        request: GrokConsoleResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokConsoleTransportResponse, GatewayError>>;

    /// Sends through one optional exact Provider-local attempt ledger.
    ///
    /// Synthetic transports have no hidden DPoP/bootstrap traffic, so the default records only
    /// the sole inference submission.  The production transport overrides this method to account
    /// for token exchange and to suppress the legacy second inference after a `401`.
    fn send_with_egress_attempt(
        &self,
        request: GrokConsoleResponsesOutboundRequest,
        attempt: Option<Arc<GrokNativeEgressAttempt>>,
    ) -> ProviderFuture<'_, Result<GrokConsoleTransportResponse, GatewayError>> {
        Box::pin(async move {
            if let Some(attempt) = attempt {
                attempt
                    .record_inference_submission()
                    .map_err(map_native_egress_error)?;
            }
            self.send(request).await
        })
    }
}

/// Production Console transport through the shared DNS-pinned client.
pub struct GrokConsoleUpstreamTransport {
    egress_policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    client_pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
    dpop_sessions: Arc<GrokConsoleDpopSessionCache>,
}

impl GrokConsoleUpstreamTransport {
    /// Creates one explicit egress/client/profile binding.
    #[must_use]
    pub fn new(
        egress_policy: EgressPolicy,
        resolver: Arc<dyn EgressDnsResolver>,
        client_pool: UpstreamClientPool,
        profile: UpstreamTransportProfile,
    ) -> Self {
        Self {
            egress_policy,
            resolver,
            client_pool,
            profile: profile.with_chrome_146_emulation(),
            dpop_sessions: Arc::new(GrokConsoleDpopSessionCache::default()),
        }
    }
}

impl fmt::Debug for GrokConsoleUpstreamTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokConsoleUpstreamTransport")
            .field("egress_policy", self.egress_policy.id())
            .field("resolver", &"<injected>")
            .field("client_pool", &self.client_pool)
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

impl GrokConsoleTransport for GrokConsoleUpstreamTransport {
    #[allow(clippy::too_many_lines)] // Token exchange, one 401 renewal, and final request stay in one bounded transport transaction.
    fn send(
        &self,
        outbound: GrokConsoleResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokConsoleTransportResponse, GatewayError>> {
        self.send_with_egress_attempt(outbound, None)
    }

    #[allow(clippy::too_many_lines)]
    fn send_with_egress_attempt(
        &self,
        outbound: GrokConsoleResponsesOutboundRequest,
        egress_attempt: Option<Arc<GrokNativeEgressAttempt>>,
    ) -> ProviderFuture<'_, Result<GrokConsoleTransportResponse, GatewayError>> {
        let admitted = self
            .egress_policy
            .admit_url(outbound.url(), self.resolver.as_ref())
            .map_err(gateway_upstream::EgressAdmissionError::gateway_error);
        let dpop_sessions = Arc::clone(&self.dpop_sessions);
        let egress_policy = self.egress_policy.clone();
        let resolver = Arc::clone(&self.resolver);
        let pool = self.client_pool.clone();
        let profile = self.profile.clone();
        Box::pin(async move {
            let target = admitted?;
            let binding = format!("{}|{}", outbound.url(), outbound.cookie());
            let cache_key = grok_console_dpop_cache_key(&binding);
            let now = SystemTime::now();
            let mut session = dpop_sessions.get(&cache_key, now);
            if session.is_none() {
                if let Some(attempt) = &egress_attempt {
                    attempt
                        .begin_console_session_bootstrap()
                        .map_err(map_native_egress_error)?;
                }
                let signing_key = GrokConsoleDpopSession::generate_key();
                let token_url =
                    EndpointUrl::compose(GROK_CONSOLE_RESPONSES_BASE_URL, "/v1/dpop/token")
                        .map_err(|_| egress_error())?;
                let token_target = egress_policy
                    .admit_url(token_url.as_str(), resolver.as_ref())
                    .map_err(gateway_upstream::EgressAdmissionError::gateway_error)?;
                let token_headers = [
                    ("accept".to_owned(), "application/json".to_owned()),
                    ("content-type".to_owned(), "application/json".to_owned()),
                    ("cookie".to_owned(), outbound.cookie().to_owned()),
                    (
                        "origin".to_owned(),
                        GROK_CONSOLE_RESPONSES_BASE_URL.to_owned(),
                    ),
                    ("referer".to_owned(), "https://console.x.ai/".to_owned()),
                    ("user-agent".to_owned(), GROK_CONSOLE_USER_AGENT.to_owned()),
                ];
                let token_body = GrokConsoleDpopSession::token_exchange_body(&signing_key)
                    .map_err(|_| egress_error())?;
                let token_request = UpstreamHttpRequest::try_new(
                    token_target,
                    UpstreamHttpMethod::Post,
                    token_headers,
                    token_body,
                )
                .map_err(|_| egress_error())?;
                let mut token_response = pool.send(token_request, &profile).await?;
                let token_bytes = read_raw_console_body(&mut token_response, 64 * 1024).await?;
                if !(200..=299).contains(&token_response.status()) {
                    return Err(console_failure_error(classify_grok_console_http_failure(
                        token_response.status(),
                        false,
                    )));
                }
                let response: ConsoleDpopTokenResponse =
                    serde_json::from_slice(&token_bytes).map_err(|_| egress_error())?;
                let built = GrokConsoleDpopSession::from_token_response(
                    response.access_token,
                    &response.token_type,
                    response.expires_in,
                    signing_key,
                    now,
                )
                .map_err(|_| egress_error())?;
                dpop_sessions
                    .insert(cache_key.clone(), built.clone())
                    .map_err(|_| egress_error())?;
                if let Some(attempt) = &egress_attempt {
                    attempt
                        .complete_console_session_bootstrap(
                            built.expires_at_ms().map_err(|_| egress_error())?,
                        )
                        .map_err(map_native_egress_error)?;
                }
                session = Some(built);
            }
            let original_cookie = outbound.cookie().to_owned();
            let mut request = outbound
                .clone()
                .with_dpop_session(session.as_ref().ok_or_else(egress_error)?, now)
                .map_err(|_| egress_error())?
                .into_transport_request(target.clone())
                .map_err(|_| egress_error())?;
            if let Some(attempt) = &egress_attempt {
                attempt
                    .record_inference_submission()
                    .map_err(map_native_egress_error)?;
            }
            let mut response = pool.send(request, &profile).await?;
            if response.status() == 401 {
                dpop_sessions
                    .invalidate(
                        &cache_key,
                        session
                            .as_ref()
                            .map_or("", GrokConsoleDpopSession::access_token),
                    )
                    .map_err(|_| egress_error())?;
                if let Some(attempt) = &egress_attempt {
                    attempt
                        .require_console_session_rebuild()
                        .map_err(map_native_egress_error)?;
                    return Ok(GrokConsoleTransportResponse::new(
                        response.status(),
                        console_content_type(&response),
                        Box::new(ConsoleUpstreamBody { response }),
                    ));
                }
                let refreshed = GrokConsoleDpopSession::generate_key();
                let token_url =
                    EndpointUrl::compose(GROK_CONSOLE_RESPONSES_BASE_URL, "/v1/dpop/token")
                        .map_err(|_| egress_error())?;
                let token_target = egress_policy
                    .admit_url(token_url.as_str(), resolver.as_ref())
                    .map_err(gateway_upstream::EgressAdmissionError::gateway_error)?;
                let token_body = GrokConsoleDpopSession::token_exchange_body(&refreshed)
                    .map_err(|_| egress_error())?;
                let token_request = UpstreamHttpRequest::try_new(
                    token_target,
                    UpstreamHttpMethod::Post,
                    vec![
                        ("accept".to_owned(), "application/json".to_owned()),
                        ("content-type".to_owned(), "application/json".to_owned()),
                        ("cookie".to_owned(), original_cookie),
                        (
                            "origin".to_owned(),
                            GROK_CONSOLE_RESPONSES_BASE_URL.to_owned(),
                        ),
                        ("referer".to_owned(), "https://console.x.ai/".to_owned()),
                        ("user-agent".to_owned(), GROK_CONSOLE_USER_AGENT.to_owned()),
                    ],
                    token_body,
                )
                .map_err(|_| egress_error())?;
                let mut token_response = pool.send(token_request, &profile).await?;
                let token_bytes = read_raw_console_body(&mut token_response, 64 * 1024).await?;
                if !(200..=299).contains(&token_response.status()) {
                    return Ok(GrokConsoleTransportResponse::new(
                        response.status(),
                        console_content_type(&response),
                        Box::new(ConsoleUpstreamBody { response }),
                    ));
                }
                let token: ConsoleDpopTokenResponse =
                    serde_json::from_slice(&token_bytes).map_err(|_| egress_error())?;
                let refreshed = GrokConsoleDpopSession::from_token_response(
                    token.access_token,
                    &token.token_type,
                    token.expires_in,
                    refreshed,
                    now,
                )
                .map_err(|_| egress_error())?;
                dpop_sessions
                    .insert(cache_key, refreshed.clone())
                    .map_err(|_| egress_error())?;
                request = outbound
                    .with_dpop_session(&refreshed, now)
                    .map_err(|_| egress_error())?
                    .into_transport_request(target)
                    .map_err(|_| egress_error())?;
                response = pool.send(request, &profile).await?;
            }
            Ok(GrokConsoleTransportResponse::new(
                response.status(),
                console_content_type(&response),
                Box::new(ConsoleUpstreamBody { response }),
            ))
        })
    }
}

#[derive(Deserialize)]
struct ConsoleDpopTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
}

async fn read_raw_console_body(
    response: &mut UpstreamHttpResponse,
    limit: usize,
) -> Result<Vec<u8>, GatewayError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.next_chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(egress_error());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Executable native Console inference adapter.
#[derive(Clone)]
pub struct GrokConsoleInferenceAdapter {
    provider_id: ProviderId,
    credential: GrokConsoleSsoToken,
    upstream_model: String,
    model_spec: ConsoleModelSpec,
    mode: GrokConsoleExecutionMode,
    transport: Arc<dyn GrokConsoleTransport>,
    egress_attempt: Option<Arc<GrokNativeEgressAttempt>>,
}

impl GrokConsoleInferenceAdapter {
    /// Creates one exact credential/model/mode/transport binding.
    ///
    /// # Errors
    ///
    /// Rejects unsupported models or an invalid compiled provider identity.
    pub fn try_new(
        credential: GrokConsoleSsoToken,
        upstream_model: impl Into<String>,
        mode: GrokConsoleExecutionMode,
        transport: Arc<dyn GrokConsoleTransport>,
    ) -> Result<Self, GatewayError> {
        let upstream_model = upstream_model.into();
        let model_spec = console_model(&upstream_model).ok_or_else(request_error)?;
        Self::try_new_with_spec(credential, upstream_model, model_spec, mode, transport)
    }

    /// Creates the root-only live-migration probe for a source-observed successful model.
    ///
    /// # Errors
    ///
    /// Rejects invalid observed model text or an invalid compiled provider identity.
    pub fn try_new_observed_probe(
        credential: GrokConsoleSsoToken,
        upstream_model: impl Into<String>,
        mode: GrokConsoleExecutionMode,
        transport: Arc<dyn GrokConsoleTransport>,
    ) -> Result<Self, GatewayError> {
        let upstream_model = upstream_model.into();
        let model_spec = observed_probe_model(&upstream_model).ok_or_else(request_error)?;
        Self::try_new_with_spec(credential, upstream_model, model_spec, mode, transport)
    }

    fn try_new_with_spec(
        credential: GrokConsoleSsoToken,
        upstream_model: String,
        model_spec: ConsoleModelSpec,
        mode: GrokConsoleExecutionMode,
        transport: Arc<dyn GrokConsoleTransport>,
    ) -> Result<Self, GatewayError> {
        let provider_id = ProviderId::try_new(GROK_CONSOLE_PROVIDER_ID.to_owned())
            .map_err(|_| internal_error())?;
        Ok(Self {
            provider_id,
            credential,
            upstream_model,
            model_spec,
            mode,
            transport,
            egress_attempt: None,
        })
    }

    /// Adds the exact CPAR lease/egress attempt compiled for this adapter invocation.
    #[must_use]
    pub fn with_provider_egress_attempt(mut self, attempt: Arc<GrokNativeEgressAttempt>) -> Self {
        self.egress_attempt = Some(attempt);
        self
    }
}

impl fmt::Debug for GrokConsoleInferenceAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokConsoleInferenceAdapter")
            .field("provider_id", &self.provider_id)
            .field("credential", &self.credential)
            .field("upstream_model", &"<redacted>")
            .field("model_spec", &"<redacted>")
            .field("mode", &self.mode)
            .field("transport", &"<injected>")
            .field("provider_egress", &self.egress_attempt.is_some())
            .finish()
    }
}

impl ProviderAdapter for GrokConsoleInferenceAdapter {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
}

impl InferenceAdapter for GrokConsoleInferenceAdapter {
    fn execute(
        &self,
        _context: RequestContext,
        request: CanonicalRequest,
    ) -> ProviderFuture<'_, Result<Box<dyn CanonicalEventSource>, GatewayError>> {
        let credential = self.credential.clone();
        let upstream_model = self.upstream_model.clone();
        let model_spec = self.model_spec;
        let mode = self.mode;
        let transport = Arc::clone(&self.transport);
        let egress_attempt = self.egress_attempt.clone();
        Box::pin(async move {
            let outbound = GrokConsoleResponsesRequestBuilder::build_with_spec(
                &credential,
                &upstream_model,
                &request,
                mode.response_mode(),
                model_spec,
            )
            .map_err(map_request_error)?;
            let response = transport
                .send_with_egress_attempt(outbound, egress_attempt.clone())
                .await?;
            let GrokConsoleTransportResponse {
                status,
                content_type,
                mut body,
            } = response;
            if !(200..=299).contains(&status) {
                let _ = read_console_body(&mut *body, 64 * 1024).await?;
                return Err(console_failure_error(classify_grok_console_http_failure(
                    status, false,
                )));
            }
            match mode {
                GrokConsoleExecutionMode::NonStreaming
                    if content_type == GrokConsoleResponseContentType::Json =>
                {
                    let bytes = read_console_body(&mut *body, 8 * 1024 * 1024).await?;
                    let response = GrokConsoleResponsesDecoder::decode_non_streaming(&bytes)?;
                    let source = Box::new(ConsoleBufferedEvents::new(response.into_events()))
                        as Box<dyn CanonicalEventSource>;
                    Ok(wrap_egress_source(source, egress_attempt))
                }
                GrokConsoleExecutionMode::Streaming
                    if content_type == GrokConsoleResponseContentType::EventStream =>
                {
                    let source = Box::new(ConsoleStreamingEvents::new(body))
                        as Box<dyn CanonicalEventSource>;
                    Ok(wrap_egress_source(source, egress_attempt))
                }
                _ => Err(protocol_error()),
            }
        })
    }
}

fn wrap_egress_source(
    source: Box<dyn CanonicalEventSource>,
    attempt: Option<Arc<GrokNativeEgressAttempt>>,
) -> Box<dyn CanonicalEventSource> {
    match attempt {
        Some(attempt) => Box::new(GrokNativeEgressEventSource::new(source, attempt))
            as Box<dyn CanonicalEventSource>,
        None => source,
    }
}

const fn map_native_egress_error(_error: crate::GrokNativeEgressAttemptError) -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
}

struct ConsoleUpstreamBody {
    response: UpstreamHttpResponse,
}

impl GrokConsoleResponseBody for ConsoleUpstreamBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            self.response
                .next_chunk()
                .await
                .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
        })
    }
}

struct ConsoleBufferedEvents {
    events: VecDeque<CanonicalEvent>,
}

impl ConsoleBufferedEvents {
    fn new(events: Vec<CanonicalEvent>) -> Self {
        Self {
            events: events.into(),
        }
    }
}

impl CanonicalEventSource for ConsoleBufferedEvents {
    fn next_event(&mut self) -> ProviderFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move { Ok(self.events.pop_front()) })
    }
}

struct ConsoleStreamingEvents {
    body: Box<dyn GrokConsoleResponseBody>,
    decoder: GrokConsoleResponsesStreamDecoder,
    pending: VecDeque<CanonicalEvent>,
    response_started: bool,
    terminal_failure_emitted: bool,
    finished: bool,
}

impl ConsoleStreamingEvents {
    fn new(body: Box<dyn GrokConsoleResponseBody>) -> Self {
        Self {
            body,
            decoder: GrokConsoleResponsesStreamDecoder::new(),
            pending: VecDeque::new(),
            response_started: false,
            terminal_failure_emitted: false,
            finished: false,
        }
    }

    fn next_pending(&mut self) -> Option<CanonicalEvent> {
        let event = self.pending.pop_front()?;
        if matches!(event, CanonicalEvent::ResponseStart(_)) {
            self.response_started = true;
        }
        Some(event)
    }

    fn terminal_failure(
        &mut self,
        error: GatewayError,
    ) -> Result<Option<CanonicalEvent>, GatewayError> {
        if !self.response_started {
            return Err(error);
        }
        if self.terminal_failure_emitted {
            return Ok(None);
        }
        self.terminal_failure_emitted = true;
        self.finished = true;
        Ok(Some(CanonicalEvent::StreamError(StreamError { error })))
    }
}

impl CanonicalEventSource for ConsoleStreamingEvents {
    fn next_event(&mut self) -> ProviderFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            if let Some(event) = self.next_pending() {
                return Ok(Some(event));
            }
            if self.finished {
                return Ok(None);
            }
            loop {
                match self.body.next_chunk().await {
                    Ok(Some(chunk)) => match self.decoder.push_bytes(&chunk) {
                        Ok(events) => {
                            self.pending.extend(events);
                            if let Some(event) = self.next_pending() {
                                return Ok(Some(event));
                            }
                        }
                        Err(error) => return self.terminal_failure(error),
                    },
                    Ok(None) => {
                        self.finished = true;
                        return match self.decoder.finish() {
                            Ok(()) => Ok(None),
                            Err(error) => self.terminal_failure(error),
                        };
                    }
                    Err(error) => return self.terminal_failure(error),
                }
            }
        })
    }
}

async fn read_console_body(
    body: &mut dyn GrokConsoleResponseBody,
    maximum_bytes: usize,
) -> Result<Vec<u8>, GatewayError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next_chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(protocol_error());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn console_content_type(response: &UpstreamHttpResponse) -> GrokConsoleResponseContentType {
    match response
        .header("content-type")
        .and_then(|value| value.to_str().ok())
    {
        Some(value) if value.starts_with("application/json") => {
            GrokConsoleResponseContentType::Json
        }
        Some(value) if value.starts_with("text/event-stream") => {
            GrokConsoleResponseContentType::EventStream
        }
        _ => GrokConsoleResponseContentType::OtherOrMissing,
    }
}

const fn console_failure_error(owner: GrokConsoleFailureOwner) -> GatewayError {
    match owner {
        GrokConsoleFailureOwner::Credential => GatewayError::new(
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
        ),
        GrokConsoleFailureOwner::Egress => {
            GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
        }
        GrokConsoleFailureOwner::Quota => GatewayError::new(
            GatewayErrorCode::CredentialQuotaExceeded,
            ErrorScope::Credential,
        ),
        GrokConsoleFailureOwner::Endpoint => {
            GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider)
        }
        GrokConsoleFailureOwner::Request => {
            GatewayError::new(GatewayErrorCode::ProviderPermanent, ErrorScope::Provider)
        }
    }
}

const fn map_request_error(_: GrokConsoleRequestError) -> GatewayError {
    request_error()
}

const fn request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

const fn egress_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
}

const fn protocol_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        ErrorScope::Provider,
    )
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}
