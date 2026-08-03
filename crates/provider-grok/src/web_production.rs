//! Fixed-target Grok Web text inference request boundary.
//!
//! This promotes the previously one-shot Canary target into a reusable, typed request builder
//! while retaining the same credential-bound browser session, exact Statsig signature, DNS-pinned
//! target admission and strict live JSON-object decoder. Unsupported semantic surfaces fail before
//! transport; in particular this boundary does not claim native Function Tool support.

use std::{error::Error, fmt, fmt::Write as _};

use gateway_core::{CanonicalMessage, CanonicalRequest, MessageContent};
use gateway_upstream::{
    AdmittedEgressTarget, EgressScheme, UpstreamHttpMethod, UpstreamHttpRequest,
};
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::{
    GROK_WEB_CANARY_HOST, GROK_WEB_CANARY_PATH, GROK_WEB_CANARY_URL, GrokWebBrowserEgressSession,
    GrokWebBrowserEgressSessionError, GrokWebLiveStreamDecoder, GrokWebStatsigSignature,
};

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
            .field("header_count", &16)
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
