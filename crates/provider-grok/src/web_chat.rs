//! Fixture-only Grok Web Chat request and bounded SSE response grammar.
//!
//! This is deliberately not a claim about a live `grok.com` wire protocol. It composes one
//! explicit [`GrokWebBrowserEgressSession`] into a synthetic, non-routable request blueprint and
//! decodes the corresponding strictly bounded fixture grammar. P9-09 is the only task allowed to
//! compare or bind this grammar to a real Web account, endpoint, browser, or transport.

use std::{collections::BTreeSet, error::Error, fmt};

use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalMessage, CanonicalRequest, ErrorScope,
    GatewayError, GatewayErrorCode, MessageContent, MessageEnd, MessageRole, MessageStart,
    RawExtensions, ResponseEnd, ResponseId, ResponseStart, StreamError, TextDelta,
};
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::{
    GrokWebBrowserEgressSession, GrokWebBrowserEgressSessionError, strict_json::parse_strict_json,
};

/// Non-routable fixture host used only by the P9-03 local grammar contract.
pub const GROK_WEB_CHAT_FIXTURE_HOST: &str = "grok.example.test";
/// Fixed fixture path used only by the P9-03 local grammar contract.
pub const GROK_WEB_CHAT_FIXTURE_PATH: &str = "/api/web-chat";
/// Maximum bytes retained for one complete fixture SSE record, excluding its delimiter.
pub const MAX_GROK_WEB_SSE_FRAME_BYTES: usize = 64 * 1024;

const MAX_GROK_WEB_MODEL_BYTES: usize = 512;

/// The one explicit, non-routable request target accepted during the local-only P9-03 phase.
///
/// P9-09 must replace this type with a separately authorized, admitted live target rather than
/// silently repurposing this fixture target.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokWebChatFixtureTarget;

impl GrokWebChatFixtureTarget {
    /// Returns the fixture-only host.
    #[must_use]
    pub const fn host() -> &'static str {
        GROK_WEB_CHAT_FIXTURE_HOST
    }

    /// Returns the fixture-only absolute path.
    #[must_use]
    pub const fn path() -> &'static str {
        GROK_WEB_CHAT_FIXTURE_PATH
    }
}

/// A request-ready but deliberately non-sendable Web Chat fixture blueprint.
///
/// Credential-derived headers are request-scoped and zeroized. `Debug` never renders target,
/// Cookie, User-Agent, selected model, or client message text.
#[derive(Eq, PartialEq)]
pub struct GrokWebChatOutboundRequest {
    cookie: Zeroizing<String>,
    user_agent: Zeroizing<String>,
    body: Vec<u8>,
}

impl GrokWebChatOutboundRequest {
    /// Returns the fixture-only target host. It is not a live endpoint selector.
    #[must_use]
    pub const fn fixture_host() -> &'static str {
        GrokWebChatFixtureTarget::host()
    }

    /// Returns the fixture-only target path. It is not a live endpoint selector.
    #[must_use]
    pub const fn fixture_path() -> &'static str {
        GrokWebChatFixtureTarget::path()
    }

    /// Returns one request header by case-insensitive name for local contract testing only.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("accept") {
            Some("text/event-stream")
        } else if name.eq_ignore_ascii_case("content-type") {
            Some("application/json")
        } else if name.eq_ignore_ascii_case("cookie") {
            Some(self.cookie.as_str())
        } else if name.eq_ignore_ascii_case("user-agent") {
            Some(self.user_agent.as_str())
        } else {
            None
        }
    }

    /// Returns the local fixture body for a caller-owned test transport.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl fmt::Debug for GrokWebChatOutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebChatOutboundRequest")
            .field("target", &"<fixture redacted>")
            .field(
                "header_names",
                &["accept", "content-type", "cookie", "user-agent"],
            )
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Safe failure while building the deliberately narrow local Web Chat request blueprint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebChatRequestError {
    /// The selected upstream model was empty, oversized, or unsafe as an HTTP value.
    InvalidModel,
    /// The Canonical request needs a later Web task to represent it losslessly.
    UnsupportedCanonicalRequest,
    /// The immutable browser egress session cannot safely start this fixture request.
    BrowserSessionUnavailable,
    /// The bounded fixture body could not be encoded.
    InternalEncodingFailure,
}

impl fmt::Display for GrokWebChatRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidModel => "Grok Web Chat model is invalid",
            Self::UnsupportedCanonicalRequest => "Grok Web Chat request is not supported",
            Self::BrowserSessionUnavailable => "Grok Web browser session is unavailable",
            Self::InternalEncodingFailure => "Grok Web Chat request could not be encoded",
        })
    }
}

impl Error for GrokWebChatRequestError {}

/// Stateless builder for the intentionally narrow Web Chat fixture request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokWebChatRequestBuilder;

impl GrokWebChatRequestBuilder {
    /// Builds one text-only, streaming fixture request from an immutable egress fingerprint.
    ///
    /// Exactly one extension-free user message containing exactly one non-empty text part is
    /// admitted. Conversations/parent IDs belong to P9-04; tools belong to P9-08; all thinking,
    /// cache, opaque content, and provider extensions fail closed here.
    ///
    /// This method does not create a URL, DNS lookup, client, socket, TLS handshake, proxy action,
    /// HTTP request, browser action, or account mutation.
    ///
    /// # Errors
    ///
    /// Returns a value-free category for an invalid selected model, unsupported Canonical
    /// semantics, unavailable immutable browser session, or a local serialization failure.
    pub fn build(
        session: &GrokWebBrowserEgressSession,
        upstream_model: &str,
        request: &CanonicalRequest,
        now_ms: i64,
    ) -> Result<GrokWebChatOutboundRequest, GrokWebChatRequestError> {
        if upstream_model.is_empty()
            || upstream_model.len() > MAX_GROK_WEB_MODEL_BYTES
            || !upstream_model.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(GrokWebChatRequestError::InvalidModel);
        }
        let message = extract_user_text(request)?;
        let cookie = session
            .cookie_header_for_https(
                GrokWebChatFixtureTarget::host(),
                GrokWebChatFixtureTarget::path(),
                now_ms,
            )
            .map_err(map_browser_session_error)?;
        let body = serde_json::to_vec(&Value::Object(Map::from_iter([
            ("model".to_owned(), Value::String(upstream_model.to_owned())),
            ("message".to_owned(), Value::String(message.to_owned())),
            ("stream".to_owned(), Value::Bool(true)),
        ])))
        .map_err(|_| GrokWebChatRequestError::InternalEncodingFailure)?;
        Ok(GrokWebChatOutboundRequest {
            cookie,
            user_agent: Zeroizing::new(session.user_agent().header_value().to_owned()),
            body,
        })
    }
}

fn map_browser_session_error(_: GrokWebBrowserEgressSessionError) -> GrokWebChatRequestError {
    GrokWebChatRequestError::BrowserSessionUnavailable
}

fn extract_user_text(request: &CanonicalRequest) -> Result<&str, GrokWebChatRequestError> {
    if request.messages.len() != 1
        || !request.tools.is_empty()
        || request.thinking.is_some()
        || request.prompt_cache_key.is_some()
        || request.prompt_cache_retention.is_some()
        || !request.extensions.is_empty()
    {
        return Err(GrokWebChatRequestError::UnsupportedCanonicalRequest);
    }
    let CanonicalMessage {
        role,
        content,
        extensions,
    } = &request.messages[0];
    if role.0 != "user" || content.len() != 1 || !extensions.is_empty() {
        return Err(GrokWebChatRequestError::UnsupportedCanonicalRequest);
    }
    let MessageContent::Text(text) = &content[0] else {
        return Err(GrokWebChatRequestError::UnsupportedCanonicalRequest);
    };
    if text.text.is_empty() || !text.extensions.is_empty() {
        return Err(GrokWebChatRequestError::UnsupportedCanonicalRequest);
    }
    Ok(&text.text)
}

/// Incremental decoder for the synthetic P9-03 SSE grammar.
///
/// The only accepted events are `web.response.start`, `web.message.start`, `web.text.delta`,
/// `web.message.end`, `web.response.end`, `web.response.error`, and terminal `done/[DONE]`.
/// Each JSON `type` must equal the SSE event name. Unknown/duplicate fields, malformed JSON,
/// oversized records, premature EOF, repeated terminal signals, and data after a terminal signal
/// fail closed.
#[derive(Clone, Default)]
pub struct GrokWebChatStreamDecoder {
    pending: Vec<u8>,
    state: GrokWebChatDecodeState,
    done_seen: bool,
}

impl GrokWebChatStreamDecoder {
    /// Creates an empty bounded Web Chat fixture decoder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            state: GrokWebChatDecodeState::new(),
            done_seen: false,
        }
    }

    /// Supplies arbitrary transport chunks and returns all fully decoded Canonical events.
    ///
    /// State updates are transactional: an invalid chunk leaves the decoder unchanged.
    ///
    /// # Errors
    ///
    /// Returns `UpstreamProtocolError/Stream` for malformed, unknown, duplicate, oversized, or
    /// after-terminal fixture input. It never retains raw stream text in the returned error.
    pub fn push_bytes(&mut self, chunk: &[u8]) -> Result<Vec<CanonicalEvent>, GatewayError> {
        let mut pending = self.pending.clone();
        let mut state = self.state.clone();
        let mut done_seen = self.done_seen;
        let mut events = Vec::new();

        for byte in chunk {
            pending.push(*byte);
            if let Some(delimiter_length) = sse_delimiter_length(&pending) {
                let record_length = pending
                    .len()
                    .checked_sub(delimiter_length)
                    .ok_or_else(stream_protocol_error)?;
                if record_length > MAX_GROK_WEB_SSE_FRAME_BYTES {
                    return Err(stream_protocol_error());
                }
                let mut record = std::mem::take(&mut pending);
                record.truncate(record_length);
                if done_seen && !record.is_empty() {
                    return Err(stream_protocol_error());
                }
                state.handle_sse_record(&record, &mut done_seen, &mut events)?;
            } else if pending.len() > MAX_GROK_WEB_SSE_FRAME_BYTES + 4 {
                return Err(stream_protocol_error());
            }
        }

        self.pending = pending;
        self.state = state;
        self.done_seen = done_seen;
        Ok(events)
    }

    /// Verifies that the byte source ended after exactly one terminal fixture marker.
    ///
    /// # Errors
    ///
    /// Returns `StreamTruncated/Stream` when the source ends mid-record, before a `done/[DONE]`
    /// marker, or before the Canonical lifecycle becomes terminal.
    pub fn finish(&self) -> Result<(), GatewayError> {
        if !self.pending.is_empty() || !self.done_seen {
            return Err(stream_truncated_error());
        }
        self.state.canonical.finish()
    }
}

impl fmt::Debug for GrokWebChatStreamDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebChatStreamDecoder")
            .field("pending_byte_count", &self.pending.len())
            .field("done_seen", &self.done_seen)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Default)]
struct GrokWebChatDecodeState {
    canonical: CanonicalEventState,
    response_id: Option<String>,
    message_open: bool,
}

impl GrokWebChatDecodeState {
    fn new() -> Self {
        Self {
            canonical: CanonicalEventState::default(),
            response_id: None,
            message_open: false,
        }
    }

    fn handle_sse_record(
        &mut self,
        record: &[u8],
        done_seen: &mut bool,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        if record.is_empty() {
            return Ok(());
        }
        let record = std::str::from_utf8(record).map_err(|_| stream_protocol_error())?;
        let mut event_name = None;
        let mut data = None;
        for raw_line in record.split('\n') {
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = line
                .split_once(':')
                .map(|(field, value)| (field, value.strip_prefix(' ').unwrap_or(value)))
                .ok_or_else(stream_protocol_error)?;
            match field {
                "event" if event_name.replace(value).is_none() && !value.is_empty() => {}
                "data" if data.replace(value).is_none() => {}
                _ => return Err(stream_protocol_error()),
            }
        }
        let event_name = event_name.ok_or_else(stream_protocol_error)?;
        let data = data.ok_or_else(stream_protocol_error)?;
        if event_name == "done" {
            if data != "[DONE]" || *done_seen || !self.canonical.is_terminal() {
                return Err(stream_protocol_error());
            }
            *done_seen = true;
            return Ok(());
        }
        if *done_seen || self.canonical.is_terminal() {
            return Err(stream_protocol_error());
        }
        let value = parse_strict_json(data.as_bytes(), MAX_GROK_WEB_SSE_FRAME_BYTES)
            .map_err(|()| stream_protocol_error())?;
        let object = value.as_object().ok_or_else(stream_protocol_error)?;
        if required_string(object, "type")? != event_name {
            return Err(stream_protocol_error());
        }
        match event_name {
            "web.response.start" => self.response_start(object, events),
            "web.message.start" => self.message_start(object, events),
            "web.text.delta" => self.text_delta(object, events),
            "web.message.end" => self.message_end(object, events),
            "web.response.end" => self.response_end(object, events),
            "web.response.error" => self.response_error(object, events),
            _ => Err(stream_protocol_error()),
        }
    }

    fn response_start(
        &mut self,
        object: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        ensure_known_fields(object, &["type", "response_id"])?;
        if self.response_id.is_some() {
            return Err(stream_protocol_error());
        }
        let response_id = required_string(object, "response_id")?;
        let response_id =
            ResponseId::try_new(response_id.to_owned()).map_err(|_| stream_protocol_error())?;
        let response_id_text = response_id.as_str().to_owned();
        self.emit(
            events,
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id,
                extensions: RawExtensions::default(),
            }),
        )?;
        self.response_id = Some(response_id_text);
        Ok(())
    }

    fn message_start(
        &mut self,
        object: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        ensure_known_fields(object, &["type", "response_id", "role"])?;
        self.require_response(object)?;
        if self.message_open || required_string(object, "role")? != "assistant" {
            return Err(stream_protocol_error());
        }
        self.emit(
            events,
            CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }),
        )?;
        self.message_open = true;
        Ok(())
    }

    fn text_delta(
        &mut self,
        object: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        ensure_known_fields(object, &["type", "response_id", "text"])?;
        self.require_response(object)?;
        let text = required_string(object, "text")?;
        if !self.message_open || text.is_empty() {
            return Err(stream_protocol_error());
        }
        self.emit(
            events,
            CanonicalEvent::TextDelta(TextDelta {
                text: text.to_owned(),
                extensions: RawExtensions::default(),
            }),
        )
    }

    fn message_end(
        &mut self,
        object: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        ensure_known_fields(object, &["type", "response_id"])?;
        self.require_response(object)?;
        if !self.message_open {
            return Err(stream_protocol_error());
        }
        self.emit(
            events,
            CanonicalEvent::MessageEnd(MessageEnd {
                extensions: RawExtensions::default(),
            }),
        )?;
        self.message_open = false;
        Ok(())
    }

    fn response_end(
        &mut self,
        object: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        ensure_known_fields(object, &["type", "response_id"])?;
        self.require_response(object)?;
        if self.message_open {
            return Err(stream_protocol_error());
        }
        self.emit(events, CanonicalEvent::ResponseEnd(ResponseEnd::default()))
    }

    fn response_error(
        &mut self,
        object: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        ensure_known_fields(object, &["type", "response_id"])?;
        self.require_response(object)?;
        self.emit(
            events,
            CanonicalEvent::StreamError(StreamError {
                error: GatewayError::new(GatewayErrorCode::ProviderPermanent, ErrorScope::Provider),
            }),
        )
    }

    fn require_response(&self, object: &Map<String, Value>) -> Result<(), GatewayError> {
        let expected = self
            .response_id
            .as_deref()
            .ok_or_else(stream_protocol_error)?;
        if required_string(object, "response_id")? != expected {
            return Err(stream_protocol_error());
        }
        Ok(())
    }

    fn emit(
        &mut self,
        events: &mut Vec<CanonicalEvent>,
        event: CanonicalEvent,
    ) -> Result<(), GatewayError> {
        self.canonical.apply(&event)?;
        events.push(event);
        Ok(())
    }
}

impl fmt::Debug for GrokWebChatDecodeState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebChatDecodeState")
            .field("canonical", &self.canonical)
            .field("response_started", &self.response_id.is_some())
            .field("message_open", &self.message_open)
            .finish()
    }
}

fn ensure_known_fields(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), GatewayError> {
    let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
    if object.keys().any(|field| !allowed.contains(field.as_str())) {
        return Err(stream_protocol_error());
    }
    Ok(())
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, GatewayError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(stream_protocol_error)
}

fn sse_delimiter_length(pending: &[u8]) -> Option<usize> {
    if pending.ends_with(b"\r\n\r\n") {
        Some(4)
    } else if pending.ends_with(b"\n\n") {
        Some(2)
    } else {
        None
    }
}

const fn stream_protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

const fn stream_truncated_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::StreamTruncated, ErrorScope::Stream)
}
