//! Explicitly authorized, fixed-target Grok Web Canary request boundary.
//!
//! This module exists solely for P9-09's opt-in, bounded account Canary. It is not a general Web
//! client and accepts neither an endpoint, prompt, model, tool setting, Cookie source, nor proxy
//! setting from a caller. The ignored harness owns the one-request authorization and all network
//! work; this module only assembles the fixed request and verifies the admitted target.

use std::{error::Error, fmt, fmt::Write as _};

use gateway_upstream::{
    AdmittedEgressTarget, EgressScheme, UpstreamHttpMethod, UpstreamHttpRequest,
};
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::{
    GrokWebBrowserEgressSession, GrokWebBrowserEgressSessionError, GrokWebStatsigSignature,
};

/// Fixed host permitted by the P9-09 Canary change request.
pub const GROK_WEB_CANARY_HOST: &str = "grok.com";
/// Fixed new-conversation path permitted by the P9-09 Canary change request.
pub const GROK_WEB_CANARY_PATH: &str = "/rest/app-chat/conversations/new";
/// Fixed HTTPS target permitted by the P9-09 Canary change request.
pub const GROK_WEB_CANARY_URL: &str = "https://grok.com/rest/app-chat/conversations/new";
/// Maximum encoded request bytes for the one fixed text-only Canary payload.
pub const MAX_GROK_WEB_CANARY_REQUEST_BYTES: usize = 4 * 1024;

const CANARY_MESSAGE: &str = "[user]\nReply with exactly: ready";
const CANARY_MODE: &str = "auto";

/// Safe failure while building or admitting the fixed P9-09 request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebCanaryRequestError {
    /// The immutable browser session cannot safely scope its Cookie to the fixed target.
    BrowserSessionUnavailable,
    /// The static, bounded request envelope could not be encoded.
    InternalEncodingFailure,
    /// The runtime could not create the one opaque request correlation identifier.
    EntropyUnavailable,
    /// The encoded request would exceed its fixed Canary budget.
    RequestTooLarge,
    /// A caller tried to hand the request to any target other than the fixed Canary target.
    TargetMismatch,
    /// Shared transport header admission rejected the fixed request envelope.
    TransportRequestRejected,
}

impl fmt::Display for GrokWebCanaryRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BrowserSessionUnavailable => "Grok Web Canary browser session is unavailable",
            Self::InternalEncodingFailure => "Grok Web Canary request could not be encoded",
            Self::EntropyUnavailable => "Grok Web Canary request entropy is unavailable",
            Self::RequestTooLarge => "Grok Web Canary request exceeds its fixed byte limit",
            Self::TargetMismatch => "Grok Web Canary target is not the approved fixed target",
            Self::TransportRequestRejected => {
                "Grok Web Canary request was rejected by transport admission"
            }
        })
    }
}

impl Error for GrokWebCanaryRequestError {}

/// One redacted, fixed-target outbound Canary request.
///
/// The Cookie and browser User-Agent remain request-scoped, zeroizing memory. `Debug` never
/// renders a Cookie, User-Agent, prompt, request body, or concrete target.
#[derive(Eq, PartialEq)]
pub struct GrokWebCanaryOutboundRequest {
    cookie: Zeroizing<String>,
    user_agent: Zeroizing<String>,
    statsig_signature: GrokWebStatsigSignature,
    request_id: String,
    body: Zeroizing<Vec<u8>>,
}

impl GrokWebCanaryOutboundRequest {
    /// Returns one request header by case-insensitive name for local safety-contract testing.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("accept") {
            Some("*/*")
        } else if name.eq_ignore_ascii_case("content-type") {
            Some("application/json")
        } else if name.eq_ignore_ascii_case("origin") {
            Some("https://grok.com")
        } else if name.eq_ignore_ascii_case("referer") {
            Some("https://grok.com/")
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

    /// Returns the fixed, value-free-to-config request payload for local contract testing.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        self.body.as_slice()
    }

    /// Converts this exact fixed request into one shared-client request after independent DNS
    /// admission. A path, query, origin, scheme, host, or port substitution fails closed.
    ///
    /// # Errors
    ///
    /// Returns a value-free category without retaining target or header diagnostics.
    pub fn into_transport_request(
        self,
        target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GrokWebCanaryRequestError> {
        if target.scheme() != EgressScheme::Https
            || target.host().as_str() != GROK_WEB_CANARY_HOST
            || target.port() != 443
            || target.request_url().as_str() != GROK_WEB_CANARY_URL
        {
            return Err(GrokWebCanaryRequestError::TargetMismatch);
        }
        UpstreamHttpRequest::try_new(
            target,
            UpstreamHttpMethod::Post,
            [
                ("accept".to_owned(), "*/*".to_owned()),
                ("content-type".to_owned(), "application/json".to_owned()),
                ("origin".to_owned(), "https://grok.com".to_owned()),
                ("referer".to_owned(), "https://grok.com/".to_owned()),
                ("sec-fetch-site".to_owned(), "same-origin".to_owned()),
                ("sec-fetch-mode".to_owned(), "cors".to_owned()),
                ("sec-fetch-dest".to_owned(), "empty".to_owned()),
                ("cookie".to_owned(), self.cookie.to_string()),
                ("user-agent".to_owned(), self.user_agent.to_string()),
                (
                    "x-statsig-id".to_owned(),
                    self.statsig_signature.as_str().to_owned(),
                ),
                ("x-xai-request-id".to_owned(), self.request_id),
            ],
            self.body.to_vec(),
        )
        .map_err(|_| GrokWebCanaryRequestError::TransportRequestRejected)
    }
}

impl fmt::Debug for GrokWebCanaryOutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebCanaryOutboundRequest")
            .field("target", &"<live target redacted>")
            .field(
                "header_names",
                &[
                    "accept",
                    "content-type",
                    "origin",
                    "referer",
                    "sec-fetch-site",
                    "sec-fetch-mode",
                    "sec-fetch-dest",
                    "cookie",
                    "user-agent",
                    "x-statsig-id",
                    "x-xai-request-id",
                ],
            )
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Stateless constructor for the one P9-09 fixed Canary request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokWebCanaryRequestBuilder;

impl GrokWebCanaryRequestBuilder {
    /// Builds the only P9-09 request body: a temporary, attachment-free, Tool-free new
    /// conversation in the reference `auto` mode. Tool emulation is intentionally absent, so
    /// its default-off behavior is exercised by the real boundary without adding an emulated
    /// prompt convention.
    ///
    /// This performs no environment lookup, URL parsing, DNS lookup, proxy action, TLS handshake,
    /// HTTP request, browser action, credential persistence, or account mutation.
    ///
    /// # Errors
    ///
    /// Returns a safe Browser-session, encoding, or byte-limit category without retaining values.
    pub fn build(
        session: &GrokWebBrowserEgressSession,
        statsig_signature: GrokWebStatsigSignature,
        now_ms: i64,
    ) -> Result<GrokWebCanaryOutboundRequest, GrokWebCanaryRequestError> {
        let cookie = session
            .cookie_header_for_https(GROK_WEB_CANARY_HOST, GROK_WEB_CANARY_PATH, now_ms)
            .map_err(map_browser_session_error)?;
        let body = serde_json::to_vec(&fixed_payload())
            .map_err(|_| GrokWebCanaryRequestError::InternalEncodingFailure)?;
        if body.len() > MAX_GROK_WEB_CANARY_REQUEST_BYTES {
            return Err(GrokWebCanaryRequestError::RequestTooLarge);
        }
        Ok(GrokWebCanaryOutboundRequest {
            cookie,
            user_agent: Zeroizing::new(session.user_agent().header_value().to_owned()),
            statsig_signature,
            request_id: random_uuid_v4()?,
            body: Zeroizing::new(body),
        })
    }
}

fn random_uuid_v4() -> Result<String, GrokWebCanaryRequestError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| GrokWebCanaryRequestError::EntropyUnavailable)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut value = String::with_capacity(36);
    for (index, byte) in bytes.into_iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            value.push('-');
        }
        write!(&mut value, "{byte:02x}")
            .map_err(|_| GrokWebCanaryRequestError::EntropyUnavailable)?;
    }
    Ok(value)
}

fn map_browser_session_error(_: GrokWebBrowserEgressSessionError) -> GrokWebCanaryRequestError {
    GrokWebCanaryRequestError::BrowserSessionUnavailable
}

fn fixed_payload() -> Value {
    Value::Object(Map::from_iter([
        ("collectionIds".to_owned(), Value::Array(Vec::new())),
        ("disabledConnectorIds".to_owned(), Value::Array(Vec::new())),
        (
            "deviceEnvInfo".to_owned(),
            Value::Object(Map::from_iter([
                ("darkModeEnabled".to_owned(), Value::Bool(false)),
                (
                    "devicePixelRatio".to_owned(),
                    Value::Number(serde_json::Number::from(2)),
                ),
                (
                    "screenHeight".to_owned(),
                    Value::Number(serde_json::Number::from(1328)),
                ),
                (
                    "screenWidth".to_owned(),
                    Value::Number(serde_json::Number::from(2056)),
                ),
                (
                    "viewportHeight".to_owned(),
                    Value::Number(serde_json::Number::from(1083)),
                ),
                (
                    "viewportWidth".to_owned(),
                    Value::Number(serde_json::Number::from(2056)),
                ),
            ])),
        ),
        ("disableMemory".to_owned(), Value::Bool(true)),
        ("disableSearch".to_owned(), Value::Bool(true)),
        ("disableSelfHarmShortCircuit".to_owned(), Value::Bool(false)),
        ("disableTextFollowUps".to_owned(), Value::Bool(false)),
        ("enableImageGeneration".to_owned(), Value::Bool(false)),
        ("enableImageStreaming".to_owned(), Value::Bool(false)),
        ("enableSideBySide".to_owned(), Value::Bool(false)),
        ("fileAttachments".to_owned(), Value::Array(Vec::new())),
        ("forceConcise".to_owned(), Value::Bool(false)),
        ("forceSideBySide".to_owned(), Value::Bool(false)),
        ("imageAttachments".to_owned(), Value::Array(Vec::new())),
        (
            "imageGenerationCount".to_owned(),
            Value::Number(serde_json::Number::from(0)),
        ),
        ("isAsyncChat".to_owned(), Value::Bool(false)),
        (
            "message".to_owned(),
            Value::String(CANARY_MESSAGE.to_owned()),
        ),
        ("modeId".to_owned(), Value::String(CANARY_MODE.to_owned())),
        ("responseMetadata".to_owned(), Value::Object(Map::new())),
        ("returnImageBytes".to_owned(), Value::Bool(false)),
        ("returnRawGrokInXaiRequest".to_owned(), Value::Bool(false)),
        ("sendFinalMetadata".to_owned(), Value::Bool(true)),
        ("temporary".to_owned(), Value::Bool(true)),
    ]))
}
