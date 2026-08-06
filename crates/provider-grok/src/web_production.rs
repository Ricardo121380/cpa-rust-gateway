//! Fixed-target Grok Web text inference request boundary.
//!
//! This promotes the previously one-shot Canary target into a reusable, typed request builder
//! while retaining the same credential-bound browser session, exact Statsig signature, DNS-pinned
//! target admission and strict live JSON-object decoder. Unsupported semantic surfaces fail before
//! transport; in particular this boundary does not claim native Function Tool support.

use std::{
    collections::VecDeque,
    error::Error,
    fmt,
    fmt::Write as _,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_core::{
    CanonicalEvent, CanonicalMessage, CanonicalRequest, ErrorScope, GatewayError, GatewayErrorCode,
    MessageContent, ProviderId, RequestContext, StreamError,
};
use gateway_provider::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};
use gateway_upstream::{
    AdmittedEgressTarget, EgressDnsResolver, EgressPolicy, EgressScheme, UpstreamClientPool,
    UpstreamHttpMethod, UpstreamHttpRequest, UpstreamHttpResponse, UpstreamTransportProfile,
};
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::{
    GROK_WEB_CANARY_HOST, GROK_WEB_CANARY_PATH, GROK_WEB_CANARY_URL, GrokWebAccountEvidence,
    GrokWebBrowserEgressSession, GrokWebBrowserEgressSessionError, GrokWebLiveStreamDecoder,
    GrokWebStatsigRuntime, GrokWebStatsigSignature, classify_grok_web_http_failure,
};

/// Fixed Web base URL used by control-plane Endpoint shape validation.
pub const GROK_WEB_PRODUCTION_BASE_URL: &str = "https://grok.com";
/// Fixed browser profile paired with grok2api-compatible Web requests.
pub const GROK_WEB_PRODUCTION_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
/// Chromium client hints paired with the fixed browser profile above.
pub const GROK_WEB_PRODUCTION_SEC_CH_UA: &str =
    "\"Google Chrome\";v=\"146\", \"Chromium\";v=\"146\", \"Not(A:Brand\";v=\"24\"";
pub const GROK_WEB_PRODUCTION_SEC_CH_UA_PLATFORM: &str = "\"macOS\"";
pub const GROK_WEB_PRODUCTION_SEC_CH_UA_ARCH: &str = "x86";
pub const GROK_WEB_PRODUCTION_SEC_CH_UA_BITNESS: &str = "64";
/// Stable native Web provider identity.
pub const GROK_WEB_PRODUCTION_PROVIDER_ID: &str = "grok.web";

/// Maximum normalized text retained in one Web request.
pub const MAX_GROK_WEB_PRODUCTION_MESSAGE_BYTES: usize = 1024 * 1024;
/// Maximum complete Web production request envelope.
pub const MAX_GROK_WEB_PRODUCTION_REQUEST_BYTES: usize = 2 * 1024 * 1024;

/// Safe production Web request construction failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebProductionRequestError {
    /// Browser credential/session is unavailable for the fixed target.
    BrowserSessionUnavailable,
    /// Selected model is not a frozen Web text model.
    UnsupportedModel,
    /// Canonical request contains unsupported or lossy semantics.
    UnsupportedRequest,
    /// Normalized message or encoded body exceeds its fixed bound.
    RequestTooLarge,
    /// Request ID entropy or local JSON encoding failed.
    InternalEncodingFailure,
    /// Admitted target differs by scheme, host, port, path, or query.
    TargetMismatch,
    /// Shared request admission rejected the exact envelope.
    TransportRequestRejected,
}

impl fmt::Display for GrokWebProductionRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BrowserSessionUnavailable => "Grok Web browser session is unavailable",
            Self::UnsupportedModel => "Grok Web model is unsupported",
            Self::UnsupportedRequest => "Grok Web request is unsupported",
            Self::RequestTooLarge => "Grok Web request exceeds its fixed limit",
            Self::InternalEncodingFailure => "Grok Web request encoding failed",
            Self::TargetMismatch => "Grok Web target does not match the fixed endpoint",
            Self::TransportRequestRejected => "Grok Web transport request was rejected",
        })
    }
}

impl Error for GrokWebProductionRequestError {}

/// One redacted fixed-target Web inference request.
#[derive(Eq, PartialEq)]
pub struct GrokWebProductionOutboundRequest {
    target: &'static str,
    cookie: Zeroizing<String>,
    user_agent: Zeroizing<String>,
    statsig_signature: GrokWebStatsigSignature,
    request_id: String,
    body: Zeroizing<Vec<u8>>,
}

impl GrokWebProductionOutboundRequest {
    /// Returns the immutable production URL.
    #[must_use]
    pub fn url(&self) -> &'static str {
        self.target
    }

    /// Returns one request header for contract testing and transport composition.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("accept") {
            Some("*/*")
        } else if name.eq_ignore_ascii_case("accept-encoding") {
            Some("identity")
        } else if name.eq_ignore_ascii_case("accept-language") {
            Some("zh-CN,zh;q=0.9,en;q=0.8")
        } else if name.eq_ignore_ascii_case("cache-control") || name.eq_ignore_ascii_case("pragma")
        {
            Some("no-cache")
        } else if name.eq_ignore_ascii_case("content-type") {
            Some("application/json")
        } else if name.eq_ignore_ascii_case("origin") {
            Some("https://grok.com")
        } else if name.eq_ignore_ascii_case("referer") {
            Some("https://grok.com/")
        } else if name.eq_ignore_ascii_case("priority") {
            Some("u=1, i")
        } else if name.eq_ignore_ascii_case("sec-fetch-site") {
            Some("same-origin")
        } else if name.eq_ignore_ascii_case("sec-fetch-mode") {
            Some("cors")
        } else if name.eq_ignore_ascii_case("sec-fetch-dest") {
            Some("empty")
        } else if name.eq_ignore_ascii_case("sec-ch-ua") {
            Some(GROK_WEB_PRODUCTION_SEC_CH_UA)
        } else if name.eq_ignore_ascii_case("sec-ch-ua-mobile") {
            Some("?0")
        } else if name.eq_ignore_ascii_case("sec-ch-ua-platform") {
            Some(GROK_WEB_PRODUCTION_SEC_CH_UA_PLATFORM)
        } else if name.eq_ignore_ascii_case("sec-ch-ua-arch") {
            Some(GROK_WEB_PRODUCTION_SEC_CH_UA_ARCH)
        } else if name.eq_ignore_ascii_case("sec-ch-ua-bitness") {
            Some(GROK_WEB_PRODUCTION_SEC_CH_UA_BITNESS)
        } else if name.eq_ignore_ascii_case("cookie") {
            Some(self.cookie.as_str())
        } else if name.eq_ignore_ascii_case("user-agent") {
            Some(self.user_agent.as_str())
        } else if name.eq_ignore_ascii_case("x-statsig-id") {
            Some(self.statsig_signature.as_str())
        } else if name.eq_ignore_ascii_case("x-xai-request-id") {
            Some(self.request_id.as_str())
        } else {
            None
        }
    }

    /// Returns the normalized Web body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        self.body.as_slice()
    }

    /// Converts this request after independent exact-target DNS admission.
    ///
    /// # Errors
    ///
    /// Rejects all target substitution and shared-header admission failures.
    pub fn into_transport_request(
        self,
        target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GrokWebProductionRequestError> {
        if target.scheme() != EgressScheme::Https
            || target.host().as_str() != GROK_WEB_CANARY_HOST
            || target.port() != 443
            || target.request_url().as_str() != GROK_WEB_CANARY_URL
        {
            return Err(GrokWebProductionRequestError::TargetMismatch);
        }
        let names = [
            "accept",
            "accept-encoding",
            "accept-language",
            "cache-control",
            "pragma",
            "content-type",
            "origin",
            "referer",
            "priority",
            "sec-fetch-site",
            "sec-fetch-mode",
            "sec-fetch-dest",
            "sec-ch-ua",
            "sec-ch-ua-mobile",
            "sec-ch-ua-platform",
            "sec-ch-ua-arch",
            "sec-ch-ua-bitness",
            "cookie",
            "user-agent",
            "x-statsig-id",
            "x-xai-request-id",
        ];
        let headers = names
            .into_iter()
            .filter_map(|name| {
                self.header(name)
                    .map(|value| (name.to_owned(), value.to_owned()))
            })
            .collect::<Vec<_>>();
        UpstreamHttpRequest::try_new(
            target,
            UpstreamHttpMethod::Post,
            headers,
            self.body.to_vec(),
        )
        .map_err(|_| GrokWebProductionRequestError::TransportRequestRejected)
    }
}

impl fmt::Debug for GrokWebProductionOutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebProductionOutboundRequest")
            .field("target", &"<redacted>")
            .field("header_count", &21)
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Builds text inference requests for the fixed Grok Web endpoint.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokWebProductionRequestBuilder;

impl GrokWebProductionRequestBuilder {
    /// Builds one temporary, memory-disabled Web conversation request.
    ///
    /// # Errors
    ///
    /// Rejects unsupported models, Tool/Reasoning/cache/extensions, opaque content, empty
    /// messages, and bounded-size violations before transport.
    pub fn build(
        session: &GrokWebBrowserEgressSession,
        statsig_signature: GrokWebStatsigSignature,
        upstream_model: &str,
        request: &CanonicalRequest,
        now_ms: i64,
    ) -> Result<GrokWebProductionOutboundRequest, GrokWebProductionRequestError> {
        let mode =
            web_mode(upstream_model).ok_or(GrokWebProductionRequestError::UnsupportedModel)?;
        let message = normalized_message(request)?;
        if message.len() > MAX_GROK_WEB_PRODUCTION_MESSAGE_BYTES {
            return Err(GrokWebProductionRequestError::RequestTooLarge);
        }
        let cookie = session
            .cookie_header_for_https(GROK_WEB_CANARY_HOST, GROK_WEB_CANARY_PATH, now_ms)
            .map_err(map_session_error)?;
        let body = serde_json::to_vec(&production_payload(&message, mode))
            .map_err(|_| GrokWebProductionRequestError::InternalEncodingFailure)?;
        if body.len() > MAX_GROK_WEB_PRODUCTION_REQUEST_BYTES {
            return Err(GrokWebProductionRequestError::RequestTooLarge);
        }
        Ok(GrokWebProductionOutboundRequest {
            target: GROK_WEB_CANARY_URL,
            cookie,
            user_agent: Zeroizing::new(session.user_agent().header_value().to_owned()),
            statsig_signature,
            request_id: random_uuid_v4()?,
            body: Zeroizing::new(body),
        })
    }
}

fn web_mode(model: &str) -> Option<&'static str> {
    match model {
        "grok-chat-fast" => Some("fast"),
        "grok-chat-auto" => Some("auto"),
        "grok-chat-expert" => Some("expert"),
        "grok-chat-heavy" => Some("heavy"),
        _ => None,
    }
}

fn normalized_message(request: &CanonicalRequest) -> Result<String, GrokWebProductionRequestError> {
    if request.messages.is_empty()
        || !request.tools.is_empty()
        || request.thinking.is_some()
        || request.prompt_cache_key.is_some()
        || request.prompt_cache_retention.is_some()
        || !request.extensions.is_empty()
    {
        return Err(GrokWebProductionRequestError::UnsupportedRequest);
    }
    let mut output = String::new();
    for CanonicalMessage {
        role,
        content,
        extensions,
    } in &request.messages
    {
        if !matches!(
            role.0.as_str(),
            "assistant" | "developer" | "system" | "user"
        ) || content.is_empty()
            || !extensions.is_empty()
        {
            return Err(GrokWebProductionRequestError::UnsupportedRequest);
        }
        writeln!(&mut output, "[{}]", role.0)
            .map_err(|_| GrokWebProductionRequestError::InternalEncodingFailure)?;
        for part in content {
            let MessageContent::Text(text) = part else {
                return Err(GrokWebProductionRequestError::UnsupportedRequest);
            };
            if text.text.is_empty() || !text.extensions.is_empty() {
                return Err(GrokWebProductionRequestError::UnsupportedRequest);
            }
            output.push_str(&text.text);
            output.push('\n');
        }
        output.push('\n');
        if output.len() > MAX_GROK_WEB_PRODUCTION_MESSAGE_BYTES {
            return Err(GrokWebProductionRequestError::RequestTooLarge);
        }
    }
    let normalized = output.trim().to_owned();
    if normalized.is_empty() {
        Err(GrokWebProductionRequestError::UnsupportedRequest)
    } else {
        Ok(normalized)
    }
}

fn production_payload(message: &str, mode: &str) -> Value {
    Value::Object(Map::from_iter([
        ("collectionIds".to_owned(), Value::Array(Vec::new())),
        ("disabledConnectorIds".to_owned(), Value::Array(Vec::new())),
        (
            "deviceEnvInfo".to_owned(),
            Value::Object(Map::from_iter([
                ("darkModeEnabled".to_owned(), Value::Bool(false)),
                ("devicePixelRatio".to_owned(), Value::from(2)),
                ("screenHeight".to_owned(), Value::from(1328)),
                ("screenWidth".to_owned(), Value::from(2056)),
                ("viewportHeight".to_owned(), Value::from(1083)),
                ("viewportWidth".to_owned(), Value::from(2056)),
            ])),
        ),
        ("disableMemory".to_owned(), Value::Bool(true)),
        ("disableSearch".to_owned(), Value::Bool(false)),
        ("disableSelfHarmShortCircuit".to_owned(), Value::Bool(false)),
        ("disableTextFollowUps".to_owned(), Value::Bool(false)),
        ("enableImageGeneration".to_owned(), Value::Bool(false)),
        ("enableImageStreaming".to_owned(), Value::Bool(false)),
        ("enableSideBySide".to_owned(), Value::Bool(false)),
        ("fileAttachments".to_owned(), Value::Array(Vec::new())),
        ("forceConcise".to_owned(), Value::Bool(false)),
        ("forceSideBySide".to_owned(), Value::Bool(false)),
        ("imageAttachments".to_owned(), Value::Array(Vec::new())),
        ("imageGenerationCount".to_owned(), Value::from(0)),
        ("isAsyncChat".to_owned(), Value::Bool(false)),
        ("message".to_owned(), Value::String(message.to_owned())),
        ("modeId".to_owned(), Value::String(mode.to_owned())),
        ("responseMetadata".to_owned(), Value::Object(Map::new())),
        ("returnImageBytes".to_owned(), Value::Bool(false)),
        ("returnRawGrokInXaiRequest".to_owned(), Value::Bool(false)),
        ("sendFinalMetadata".to_owned(), Value::Bool(true)),
        ("temporary".to_owned(), Value::Bool(true)),
    ]))
}

fn random_uuid_v4() -> Result<String, GrokWebProductionRequestError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|_| GrokWebProductionRequestError::InternalEncodingFailure)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut value = String::with_capacity(36);
    for (index, byte) in bytes.into_iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            value.push('-');
        }
        write!(&mut value, "{byte:02x}")
            .map_err(|_| GrokWebProductionRequestError::InternalEncodingFailure)?;
    }
    Ok(value)
}

fn map_session_error(_: GrokWebBrowserEgressSessionError) -> GrokWebProductionRequestError {
    GrokWebProductionRequestError::BrowserSessionUnavailable
}

/// The production response decoder remains the strict P9-09 live JSON-object decoder.
pub type GrokWebProductionStreamDecoder = GrokWebLiveStreamDecoder;

/// Pull-only body returned from one admitted Web conversation request.
pub trait GrokWebProductionResponseBody: Send {
    /// Returns the next opaque body chunk or normal end of stream.
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>>;
}

/// Safe status and pull-only body projection from the Web transport.
pub struct GrokWebProductionTransportResponse {
    status: u16,
    body: Box<dyn GrokWebProductionResponseBody>,
}

impl GrokWebProductionTransportResponse {
    /// Creates a response handoff after shared HTTP transport admission.
    #[must_use]
    pub fn new(status: u16, body: Box<dyn GrokWebProductionResponseBody>) -> Self {
        Self { status, body }
    }

    fn into_parts(self) -> (u16, Box<dyn GrokWebProductionResponseBody>) {
        (self.status, self.body)
    }
}

impl fmt::Debug for GrokWebProductionTransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebProductionTransportResponse")
            .field("status", &self.status)
            .field("body", &"<streaming>")
            .finish()
    }
}

/// Sends one already-built Web conversation request without implicit account fallback.
pub trait GrokWebProductionTransport: Send + Sync {
    /// Executes exactly one admitted request.
    fn send(
        &self,
        request: GrokWebProductionOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokWebProductionTransportResponse, GatewayError>>;
}

/// Production Web conversation transport through the Chrome-emulated DNS-pinned client.
pub struct GrokWebProductionUpstreamTransport {
    egress_policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    client_pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
}

impl GrokWebProductionUpstreamTransport {
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
        }
    }
}

impl fmt::Debug for GrokWebProductionUpstreamTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebProductionUpstreamTransport")
            .field("egress_policy", self.egress_policy.id())
            .field("resolver", &"<injected>")
            .field("client_pool", &self.client_pool)
            .field("profile", &self.profile)
            .finish()
    }
}

impl GrokWebProductionTransport for GrokWebProductionUpstreamTransport {
    fn send(
        &self,
        outbound: GrokWebProductionOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokWebProductionTransportResponse, GatewayError>> {
        let admitted = self
            .egress_policy
            .admit_url(outbound.url(), self.resolver.as_ref())
            .map_err(gateway_upstream::EgressAdmissionError::gateway_error);
        let request = admitted.and_then(|target| {
            outbound
                .into_transport_request(target)
                .map_err(|_| web_egress_error())
        });
        let pool = self.client_pool.clone();
        let profile = self.profile.clone();
        Box::pin(async move {
            let response = pool.send(request?, &profile).await?;
            Ok(GrokWebProductionTransportResponse::new(
                response.status(),
                Box::new(GrokWebProductionUpstreamBody { response }),
            ))
        })
    }
}

/// Executable native Web adapter with a shared dynamic Statsig runtime.
#[derive(Clone)]
pub struct GrokWebProductionInferenceAdapter {
    provider_id: ProviderId,
    session: Arc<GrokWebBrowserEgressSession>,
    upstream_model: String,
    statsig: Arc<GrokWebStatsigRuntime>,
    transport: Arc<dyn GrokWebProductionTransport>,
    egress_refresher: Option<Arc<dyn GrokWebEgressRefresher>>,
}

/// Explicit per-request recovery hook for a rejected Web egress session.
///
/// Implementations may rebuild the credential-bound session (for example after an
/// externally managed clearance refresh or proxy rotation). The hook is deliberately
/// injected per adapter; it never mutates a global proxy or credential pool.
pub trait GrokWebEgressRefresher: Send + Sync {
    /// Rebuilds the exact account-bound session for the next attempt.
    fn refresh<'a>(
        &'a self,
        current: &'a GrokWebBrowserEgressSession,
    ) -> ProviderFuture<'a, Result<Arc<GrokWebBrowserEgressSession>, GatewayError>>;
}

impl GrokWebProductionInferenceAdapter {
    /// Creates one exact session/model/signer/transport binding.
    ///
    /// # Errors
    ///
    /// Rejects a blank model or invalid compiled provider identity before transport.
    pub fn try_new(
        session: Arc<GrokWebBrowserEgressSession>,
        upstream_model: impl Into<String>,
        statsig: Arc<GrokWebStatsigRuntime>,
        transport: Arc<dyn GrokWebProductionTransport>,
    ) -> Result<Self, GatewayError> {
        let upstream_model = upstream_model.into();
        if upstream_model.trim().is_empty() {
            return Err(web_request_error());
        }
        let provider_id = ProviderId::try_new(GROK_WEB_PRODUCTION_PROVIDER_ID.to_owned())
            .map_err(|_| web_internal_error())?;
        Ok(Self {
            provider_id,
            session,
            upstream_model,
            statsig,
            transport,
            egress_refresher: None,
        })
    }

    /// Adds an explicit, credential-bound egress recovery hook.
    #[must_use]
    pub fn with_egress_refresher(mut self, refresher: Arc<dyn GrokWebEgressRefresher>) -> Self {
        self.egress_refresher = Some(refresher);
        self
    }
}

impl fmt::Debug for GrokWebProductionInferenceAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebProductionInferenceAdapter")
            .field("provider_id", &self.provider_id)
            .field("session", &self.session)
            .field("upstream_model", &"<redacted>")
            .field("statsig", &self.statsig)
            .field("egress_refresher", &self.egress_refresher.is_some())
            .field("transport", &"<injected>")
            .finish()
    }
}

impl ProviderAdapter for GrokWebProductionInferenceAdapter {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
}

impl InferenceAdapter for GrokWebProductionInferenceAdapter {
    fn execute(
        &self,
        _context: RequestContext,
        request: CanonicalRequest,
    ) -> ProviderFuture<'_, Result<Box<dyn CanonicalEventSource>, GatewayError>> {
        let mut session = Arc::clone(&self.session);
        let upstream_model = self.upstream_model.clone();
        let statsig = Arc::clone(&self.statsig);
        let transport = Arc::clone(&self.transport);
        let egress_refresher = self.egress_refresher.clone();
        Box::pin(async move {
            let now_ms = web_now_ms()?;
            for attempt in 0_u8..=1 {
                let signature = statsig.signature(&session, now_ms).await?;
                let outbound = GrokWebProductionRequestBuilder::build(
                    &session,
                    signature.clone(),
                    &upstream_model,
                    &request,
                    now_ms,
                )
                .map_err(map_web_request_error)?;
                let response = transport.send(outbound).await?;
                let (status, mut body) = response.into_parts();
                if status == 403 && attempt == 0 {
                    read_web_body(&mut *body, 4 * 1024 * 1024).await?;
                    let _ = statsig.invalidate_signature_after_403(&signature)?;
                    if let Some(refresher) = egress_refresher.as_ref() {
                        session = refresher.refresh(&session).await?;
                    }
                    continue;
                }
                if !(200..=299).contains(&status) {
                    read_web_body(&mut *body, 64 * 1024).await?;
                    let failure =
                        classify_grok_web_http_failure(status, GrokWebAccountEvidence::None)
                            .map_err(|_| web_internal_error())?;
                    return Err(failure.error().clone());
                }
                return Ok(
                    Box::new(GrokWebProductionEvents::new(body)) as Box<dyn CanonicalEventSource>
                );
            }
            Err(web_internal_error())
        })
    }
}

struct GrokWebProductionUpstreamBody {
    response: UpstreamHttpResponse,
}

impl GrokWebProductionResponseBody for GrokWebProductionUpstreamBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            self.response
                .next_chunk()
                .await
                .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
        })
    }
}

struct GrokWebProductionEvents {
    body: Box<dyn GrokWebProductionResponseBody>,
    decoder: GrokWebProductionStreamDecoder,
    pending: VecDeque<CanonicalEvent>,
    response_started: bool,
    terminal_failure_emitted: bool,
    finished: bool,
}

impl GrokWebProductionEvents {
    fn new(body: Box<dyn GrokWebProductionResponseBody>) -> Self {
        Self {
            body,
            decoder: GrokWebProductionStreamDecoder::new(),
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

impl CanonicalEventSource for GrokWebProductionEvents {
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
                            Ok(events) => {
                                self.pending.extend(events);
                                Ok(self.next_pending())
                            }
                            Err(error) => self.terminal_failure(error),
                        };
                    }
                    Err(error) => return self.terminal_failure(error),
                }
            }
        })
    }
}

async fn read_web_body(
    body: &mut dyn GrokWebProductionResponseBody,
    maximum_bytes: usize,
) -> Result<(), GatewayError> {
    let mut total = 0_usize;
    while let Some(chunk) = body.next_chunk().await? {
        total = total
            .checked_add(chunk.len())
            .ok_or_else(web_protocol_error)?;
        if total > maximum_bytes {
            return Err(web_protocol_error());
        }
    }
    Ok(())
}

fn web_now_ms() -> Result<i64, GatewayError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| web_internal_error())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| web_internal_error())
}

const fn map_web_request_error(_: GrokWebProductionRequestError) -> GatewayError {
    web_request_error()
}

const fn web_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

const fn web_egress_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
}

const fn web_protocol_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        ErrorScope::Provider,
    )
}

const fn web_internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}
