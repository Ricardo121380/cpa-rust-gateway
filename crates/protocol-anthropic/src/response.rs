use std::fmt;

use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalResponse, ExactInputTokenCount, GatewayError,
    GatewayErrorCode, Usage,
};
use serde_json::{Value, json};

use crate::json::{internal_error, stream_protocol_error};

/// Public-model metadata owned by the Anthropic response boundary.
#[derive(Clone, Eq, PartialEq)]
pub struct AnthropicResponseMetadata {
    model: String,
}

impl AnthropicResponseMetadata {
    /// Creates metadata for one non-empty client-visible model label.
    ///
    /// # Errors
    ///
    /// Returns `InternalError/Internal` for an empty label.
    pub fn try_new(model: impl Into<String>) -> Result<Self, GatewayError> {
        let model = model.into();
        if model.is_empty() {
            return Err(internal_error());
        }
        Ok(Self { model })
    }

    /// Returns the selected public model label.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }
}

impl fmt::Debug for AnthropicResponseMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnthropicResponseMetadata")
            .field("model", &"<redacted>")
            .finish()
    }
}

/// One typed Anthropic Server-Sent Event frame.
#[derive(Clone, Eq, PartialEq)]
pub struct SseFrame {
    event: &'static str,
    data: Value,
    semantic: bool,
}

impl SseFrame {
    /// Returns the SSE event name.
    #[must_use]
    pub const fn event(&self) -> &'static str {
        self.event
    }

    /// Returns the structured JSON payload.
    #[must_use]
    pub const fn data(&self) -> &Value {
        &self.data
    }

    /// Every P5-01 frame is client-visible semantic data.
    #[must_use]
    pub const fn is_semantic(&self) -> bool {
        self.semantic
    }

    /// Encodes the frame using standard `event` plus JSON `data` lines.
    ///
    /// # Errors
    ///
    /// Returns a safe internal error if JSON serialization unexpectedly fails.
    pub fn to_wire(&self) -> Result<String, GatewayError> {
        let data = serde_json::to_string(&self.data).map_err(|_| internal_error())?;
        Ok(format!("event: {}\ndata: {data}\n\n", self.event))
    }
}

impl fmt::Debug for SseFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SseFrame")
            .field("event", &self.event)
            .field("data", &"<redacted>")
            .field("semantic", &self.semantic)
            .finish()
    }
}

/// Encodes one safe gateway error in Anthropic's public error envelope.
#[must_use]
pub fn encode_error(error: &GatewayError) -> Value {
    json!({
        "type": "error",
        "error": {
            "type": anthropic_error_type(error),
            "message": error.safe_message(),
        }
    })
}

/// Encodes one exact Anthropic `count_tokens` response.
#[must_use]
pub fn encode_count_tokens(count: ExactInputTokenCount) -> Value {
    json!({"input_tokens": count.input_tokens()})
}

/// Encodes a validated canonical response as one Anthropic Message object.
///
/// # Errors
///
/// Returns `UpstreamProtocolError/Stream` when the canonical response uses semantics outside the
/// P5-01 text/usage slice or cannot be represented without loss.
pub fn encode_response(
    response: &CanonicalResponse,
    metadata: AnthropicResponseMetadata,
) -> Result<Value, GatewayError> {
    let mut encoder = AnthropicMessagesSseEncoder::new(metadata);
    for event in response.events() {
        let _frames = encoder.encode_event(event)?;
    }
    encoder.into_completed_response()
}

/// Stateful Canonical Event to Anthropic SSE encoder.
pub struct AnthropicMessagesSseEncoder {
    metadata: AnthropicResponseMetadata,
    lifecycle: CanonicalEventState,
    assembly: Assembly,
}

impl AnthropicMessagesSseEncoder {
    /// Creates a fresh encoder for one response.
    #[must_use]
    pub fn new(metadata: AnthropicResponseMetadata) -> Self {
        Self {
            metadata,
            lifecycle: CanonicalEventState::default(),
            assembly: Assembly::default(),
        }
    }

    /// Validates and maps one canonical event to zero or more Anthropic SSE frames.
    ///
    /// `ResponseStart` and Usage snapshots may produce no frame. Anthropic's first `message_start`
    /// is emitted only once the canonical stream has supplied exact input Usage and `MessageStart`.
    ///
    /// # Errors
    ///
    /// Returns a safe stream protocol error for invalid ordering or unrepresentable semantics.
    pub fn encode_event(&mut self, event: &CanonicalEvent) -> Result<Vec<SseFrame>, GatewayError> {
        ensure_representable(event)?;
        let mut next_lifecycle = self.lifecycle.clone();
        next_lifecycle.apply(event)?;
        let frames = self.assembly.apply(event, &self.metadata)?;
        self.lifecycle = next_lifecycle;
        Ok(frames)
    }

    /// Consumes a successfully completed encoder into one non-streaming Message object.
    ///
    /// # Errors
    ///
    /// Returns a safe stream protocol error unless normal `ResponseEnd` was accepted.
    pub fn into_completed_response(self) -> Result<Value, GatewayError> {
        if !self.lifecycle.is_success() || self.assembly.terminal != TerminalPhase::Completed {
            return Err(stream_protocol_error());
        }
        self.assembly.completed_value(&self.metadata)
    }
}

impl fmt::Debug for AnthropicMessagesSseEncoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnthropicMessagesSseEncoder")
            .field("metadata", &self.metadata)
            .field("lifecycle", &self.lifecycle)
            .field("text_len", &self.assembly.text.len())
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Assembly {
    response_id: Option<String>,
    message: MessagePhase,
    text: String,
    usage: Option<Usage>,
    terminal: TerminalPhase,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum MessagePhase {
    #[default]
    NotStarted,
    Started,
    Content,
    Ended,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum TerminalPhase {
    #[default]
    Open,
    Completed,
    Failed,
}

impl Assembly {
    fn apply(
        &mut self,
        event: &CanonicalEvent,
        metadata: &AnthropicResponseMetadata,
    ) -> Result<Vec<SseFrame>, GatewayError> {
        match event {
            CanonicalEvent::ResponseStart(start) => {
                self.response_id = Some(start.response_id.as_str().to_owned());
                Ok(Vec::new())
            }
            CanonicalEvent::UsageDelta(delta) => {
                self.usage = Some(delta.usage.clone());
                Ok(Vec::new())
            }
            CanonicalEvent::MessageStart(start) => self.start_message(&start.role.0, metadata),
            CanonicalEvent::TextDelta(delta) => self.append_text(&delta.text),
            CanonicalEvent::MessageEnd(_) => self.end_message(),
            CanonicalEvent::ResponseEnd(_) => self.end_response(),
            CanonicalEvent::StreamError(error) => {
                self.terminal = TerminalPhase::Failed;
                Ok(vec![frame("error", encode_error(&error.error))])
            }
            CanonicalEvent::ReasoningDelta(_)
            | CanonicalEvent::ToolCallStart(_)
            | CanonicalEvent::ToolCallArgumentsDelta(_)
            | CanonicalEvent::ToolCallEnd(_) => Err(stream_protocol_error()),
        }
    }

    fn start_message(
        &mut self,
        role: &str,
        metadata: &AnthropicResponseMetadata,
    ) -> Result<Vec<SseFrame>, GatewayError> {
        if role != "assistant" || self.message != MessagePhase::NotStarted {
            return Err(stream_protocol_error());
        }
        let id = self
            .response_id
            .as_deref()
            .ok_or_else(stream_protocol_error)?;
        let input_tokens = self
            .usage
            .as_ref()
            .and_then(|usage| usage.input_tokens)
            .ok_or_else(stream_protocol_error)?;
        self.message = MessagePhase::Started;
        Ok(vec![frame(
            "message_start",
            json!({
                "type": "message_start",
                "message": {
                    "id": id,
                    "type": "message",
                    "role": "assistant",
                    "content": [],
                    "model": metadata.model(),
                    "stop_reason": null,
                    "stop_sequence": null,
                    "usage": {"input_tokens": input_tokens, "output_tokens": 0}
                }
            }),
        )])
    }

    fn append_text(&mut self, text: &str) -> Result<Vec<SseFrame>, GatewayError> {
        if !matches!(self.message, MessagePhase::Started | MessagePhase::Content) {
            return Err(stream_protocol_error());
        }
        let mut frames = Vec::new();
        if self.message == MessagePhase::Started {
            self.message = MessagePhase::Content;
            frames.push(frame(
                "content_block_start",
                json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": {"type": "text", "text": ""}
                }),
            ));
        }
        self.text.push_str(text);
        frames.push(frame(
            "content_block_delta",
            json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {"type": "text_delta", "text": text}
            }),
        ));
        Ok(frames)
    }

    fn end_message(&mut self) -> Result<Vec<SseFrame>, GatewayError> {
        if self.message != MessagePhase::Content {
            return Err(stream_protocol_error());
        }
        self.message = MessagePhase::Ended;
        Ok(vec![frame(
            "content_block_stop",
            json!({"type": "content_block_stop", "index": 0}),
        )])
    }

    fn end_response(&mut self) -> Result<Vec<SseFrame>, GatewayError> {
        if self.message != MessagePhase::Ended || self.terminal != TerminalPhase::Open {
            return Err(stream_protocol_error());
        }
        let output_tokens = self
            .usage
            .as_ref()
            .and_then(|usage| usage.output_tokens)
            .ok_or_else(stream_protocol_error)?;
        self.terminal = TerminalPhase::Completed;
        Ok(vec![
            frame(
                "message_delta",
                json!({
                    "type": "message_delta",
                    "delta": {"stop_reason": "end_turn", "stop_sequence": null},
                    "usage": {"output_tokens": output_tokens}
                }),
            ),
            frame("message_stop", json!({"type": "message_stop"})),
        ])
    }

    fn completed_value(&self, metadata: &AnthropicResponseMetadata) -> Result<Value, GatewayError> {
        let id = self
            .response_id
            .as_deref()
            .ok_or_else(stream_protocol_error)?;
        let usage = self.usage.as_ref().ok_or_else(stream_protocol_error)?;
        let input_tokens = usage.input_tokens.ok_or_else(stream_protocol_error)?;
        let output_tokens = usage.output_tokens.ok_or_else(stream_protocol_error)?;
        Ok(json!({
            "id": id,
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": self.text}],
            "model": metadata.model(),
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
            }
        }))
    }
}

fn ensure_representable(event: &CanonicalEvent) -> Result<(), GatewayError> {
    let extensions_empty = match event {
        CanonicalEvent::ResponseStart(value) => value.extensions.is_empty(),
        CanonicalEvent::MessageStart(value) => value.extensions.is_empty(),
        CanonicalEvent::TextDelta(value) => value.extensions.is_empty(),
        CanonicalEvent::ReasoningDelta(value) => value.extensions.is_empty(),
        CanonicalEvent::ToolCallStart(value) => value.extensions.is_empty(),
        CanonicalEvent::ToolCallArgumentsDelta(value) => value.extensions.is_empty(),
        CanonicalEvent::ToolCallEnd(value) => value.extensions.is_empty(),
        CanonicalEvent::UsageDelta(value) => {
            value.extensions.is_empty()
                && value.usage.extensions.is_empty()
                && value.usage.reasoning_tokens.is_none()
                && value.usage.cache_read_tokens.is_none()
                && value.usage.cache_creation_tokens.is_none()
                && value.usage.cached_tokens.is_none()
        }
        CanonicalEvent::MessageEnd(value) => value.extensions.is_empty(),
        CanonicalEvent::ResponseEnd(value) => value.extensions.is_empty(),
        CanonicalEvent::StreamError(_) => true,
    };
    if extensions_empty {
        Ok(())
    } else {
        Err(stream_protocol_error())
    }
}

const fn frame(event: &'static str, data: Value) -> SseFrame {
    SseFrame {
        event,
        data,
        semantic: true,
    }
}

const fn anthropic_error_type(error: &GatewayError) -> &'static str {
    match error.code() {
        GatewayErrorCode::ClientRequestError | GatewayErrorCode::TokenCountUnsupported => {
            "invalid_request_error"
        }
        GatewayErrorCode::ClientUnauthorized
        | GatewayErrorCode::CredentialUnauthorized
        | GatewayErrorCode::CredentialForbidden => "authentication_error",
        GatewayErrorCode::RouteNotFound => "not_found_error",
        GatewayErrorCode::ProviderRateLimited | GatewayErrorCode::CredentialQuotaExceeded => {
            "rate_limit_error"
        }
        GatewayErrorCode::ProviderPermanent => "api_error",
        GatewayErrorCode::CredentialUnavailable
        | GatewayErrorCode::EgressRejected
        | GatewayErrorCode::EgressUnavailable
        | GatewayErrorCode::ProviderTransient
        | GatewayErrorCode::UpstreamProtocolError
        | GatewayErrorCode::StreamTruncated
        | GatewayErrorCode::InternalError
        | GatewayErrorCode::Cancelled => "overloaded_error",
    }
}

#[cfg(test)]
mod tests {
    use gateway_core::{
        CanonicalEvent, CanonicalResponse, ErrorScope, GatewayError, GatewayErrorCode,
    };

    use super::{
        AnthropicMessagesSseEncoder, AnthropicResponseMetadata, encode_count_tokens, encode_error,
        encode_response,
    };

    fn events() -> Result<Vec<CanonicalEvent>, serde_json::Error> {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/anthropic/canonical-events.json"
        ))
    }

    #[test]
    fn non_streaming_fixture_matches_snapshot() -> Result<(), Box<dyn std::error::Error>> {
        let response = CanonicalResponse::try_new(events()?)?;
        let metadata = AnthropicResponseMetadata::try_new("gateway-claude")?;
        let encoded = encode_response(&response, metadata)?;
        assert_eq!(
            serde_json::to_string_pretty(&encoded)?,
            include_str!("../../../tests/fixtures/anthropic/non-streaming-response.json").trim()
        );
        Ok(())
    }

    #[test]
    fn streaming_fixture_matches_snapshot() -> Result<(), Box<dyn std::error::Error>> {
        let metadata = AnthropicResponseMetadata::try_new("gateway-claude")?;
        let mut encoder = AnthropicMessagesSseEncoder::new(metadata);
        let mut wire = String::new();
        for event in events()? {
            for frame in encoder.encode_event(&event)? {
                assert!(frame.is_semantic());
                wire.push_str(&frame.to_wire()?);
            }
        }
        assert!(wire.ends_with("\n\n"));
        assert_eq!(
            wire.trim_end(),
            include_str!("../../../tests/fixtures/anthropic/stream.sse").trim_end()
        );
        Ok(())
    }

    #[test]
    fn safe_error_envelope_contains_no_diagnostics() {
        let error = GatewayError::new(GatewayErrorCode::ClientUnauthorized, ErrorScope::Request);
        assert_eq!(
            encode_error(&error),
            serde_json::json!({
                "type": "error",
                "error": {
                    "type": "authentication_error",
                    "message": "the client is not authorized"
                }
            })
        );
    }

    #[test]
    fn exact_count_tokens_response_has_no_estimate_or_extra_fields() {
        assert_eq!(
            encode_count_tokens(gateway_core::ExactInputTokenCount::new(17)),
            serde_json::json!({"input_tokens": 17})
        );
    }

    #[test]
    fn rejects_missing_initial_usage_and_unrepresentable_events()
    -> Result<(), Box<dyn std::error::Error>> {
        let metadata = AnthropicResponseMetadata::try_new("gateway-claude")?;
        let mut encoder = AnthropicMessagesSseEncoder::new(metadata);
        let events: Vec<CanonicalEvent> = serde_json::from_str(
            r#"[
                {"response_start":{"response_id":"r","extensions":{}}},
                {"message_start":{"role":"assistant","extensions":{}}}
            ]"#,
        )?;
        assert!(encoder.encode_event(&events[0]).is_ok());
        assert!(encoder.encode_event(&events[1]).is_err());

        let metadata = AnthropicResponseMetadata::try_new("gateway-claude")?;
        let mut encoder = AnthropicMessagesSseEncoder::new(metadata);
        let events: Vec<CanonicalEvent> = serde_json::from_str(
            r#"[
                {"response_start":{"response_id":"r","extensions":{}}},
                {"usage_delta":{"usage":{"input_tokens":1,"extensions":{}},"is_final":false,"extensions":{}}},
                {"message_start":{"role":"assistant","extensions":{}}},
                {"reasoning_delta":{"text":"hidden","extensions":{}}}
            ]"#,
        )?;
        for event in &events[..3] {
            assert!(encoder.encode_event(event).is_ok());
        }
        assert!(encoder.encode_event(&events[3]).is_err());
        Ok(())
    }

    #[test]
    fn encoder_debug_redacts_model_and_text() -> Result<(), GatewayError> {
        let metadata = AnthropicResponseMetadata::try_new("secret-model")?;
        let encoder = AnthropicMessagesSseEncoder::new(metadata);
        let diagnostic = format!("{encoder:?}");
        assert!(!diagnostic.contains("secret-model"));
        Ok(())
    }
}
