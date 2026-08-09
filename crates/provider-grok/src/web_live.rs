//! Bounded live Grok Web JSON-object stream projection.
//!
//! Grok Web currently returns a concatenated JSON-object stream rather than the synthetic SSE
//! grammar used by the P9-03 fixture. This decoder owns only the narrow text/reasoning lifecycle
//! needed by P9-09: a conversation identity, response token deltas, and the final model-response
//! envelope. Unknown outer fields are not trusted as Canonical data; malformed frames, identity
//! changes, text rewinds, a missing final envelope, or bytes after finalization fail closed.

use std::fmt;

use gateway_core::{
    CanonicalEvent, CanonicalEventState, GatewayError, GatewayErrorCode, MessageEnd, MessageRole,
    MessageStart, RawExtensions, ReasoningDelta, ResponseEnd, ResponseId, ResponseStart, TextDelta,
};
use serde_json::{Map, Value};

use crate::strict_json::parse_strict_json;

/// Maximum retained bytes for one unfinished live JSON object.
pub const MAX_GROK_WEB_LIVE_FRAME_BYTES: usize = 8 * 1024 * 1024;

const MAX_GROK_WEB_LIVE_VISIBLE_TEXT_BYTES: usize = 8 * 1024 * 1024;

/// Incrementally decodes the admitted live JSON-object protocol into Canonical events.
#[derive(Clone, Default)]
pub struct GrokWebLiveStreamDecoder {
    pending: Vec<u8>,
    depth: u32,
    in_string: bool,
    escaped: bool,
    state: GrokWebLiveDecodeState,
}

impl GrokWebLiveStreamDecoder {
    /// Creates an empty live Web decoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds arbitrary transport chunks and returns every fully projected Canonical event.
    ///
    /// # Errors
    ///
    /// Returns a value-free stream protocol failure for malformed, oversized, inconsistent, or
    /// post-final frames. State updates are transactional for each input chunk.
    pub fn push_bytes(&mut self, chunk: &[u8]) -> Result<Vec<CanonicalEvent>, GatewayError> {
        let mut pending = self.pending.clone();
        let mut depth = self.depth;
        let mut in_string = self.in_string;
        let mut escaped = self.escaped;
        let mut state = self.state.clone();
        let mut events = Vec::new();

        for byte in chunk {
            if depth == 0 {
                if byte.is_ascii_whitespace() {
                    continue;
                }
                if *byte != b'{' {
                    return Err(stream_protocol_error());
                }
                pending.clear();
                pending.push(*byte);
                depth = 1;
                in_string = false;
                escaped = false;
                continue;
            }

            pending.push(*byte);
            if pending.len() > MAX_GROK_WEB_LIVE_FRAME_BYTES {
                return Err(stream_protocol_error());
            }
            if in_string {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == b'"' {
                    in_string = false;
                }
                continue;
            }
            match *byte {
                b'"' => in_string = true,
                b'{' => depth = depth.checked_add(1).ok_or_else(stream_protocol_error)?,
                b'}' => {
                    depth = depth.checked_sub(1).ok_or_else(stream_protocol_error)?;
                    if depth == 0 {
                        state.handle_frame(&pending, &mut events)?;
                        pending.clear();
                    }
                }
                _ => {}
            }
        }

        self.pending = pending;
        self.depth = depth;
        self.in_string = in_string;
        self.escaped = escaped;
        self.state = state;
        Ok(events)
    }

    /// Completes a clean live response only after a final model-response envelope was observed.
    ///
    /// # Errors
    ///
    /// Returns `StreamTruncated/Stream` for incomplete data or a missing final envelope.
    pub fn finish(&mut self) -> Result<Vec<CanonicalEvent>, GatewayError> {
        if self.depth != 0 || !self.pending.is_empty() || !self.state.final_seen {
            return Err(stream_truncated_error());
        }
        let mut events = Vec::new();
        self.state.finish(&mut events)?;
        Ok(events)
    }
}

impl fmt::Debug for GrokWebLiveStreamDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebLiveStreamDecoder")
            .field("pending_byte_count", &self.pending.len())
            .field("object_depth", &self.depth)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Default)]
struct GrokWebLiveDecodeState {
    canonical: CanonicalEventState,
    response_id: Option<String>,
    message_open: bool,
    visible_text: String,
    final_seen: bool,
}

impl GrokWebLiveDecodeState {
    fn handle_frame(
        &mut self,
        frame: &[u8],
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        if self.final_seen {
            return Err(stream_protocol_error());
        }
        let value = parse_strict_json(frame, MAX_GROK_WEB_LIVE_FRAME_BYTES)
            .map_err(|()| stream_protocol_error())?;
        let root = value.as_object().ok_or_else(stream_protocol_error)?;
        if root.contains_key("error") {
            return Err(stream_protocol_error());
        }
        let result = root
            .get("result")
            .and_then(Value::as_object)
            .ok_or_else(stream_protocol_error)?;

        if let Some(conversation) = result.get("conversation") {
            let conversation = conversation.as_object().ok_or_else(stream_protocol_error)?;
            self.observe_conversation(required_string(conversation, "conversationId")?, events)?;
        }
        if let Some(response) = result.get("response") {
            self.observe_response(
                response.as_object().ok_or_else(stream_protocol_error)?,
                events,
            )?;
        }
        if !result.contains_key("conversation") && !result.contains_key("response") {
            return Err(stream_protocol_error());
        }
        Ok(())
    }

    fn observe_conversation(
        &mut self,
        conversation_id: &str,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let response_id =
            ResponseId::try_new(conversation_id.to_owned()).map_err(|_| stream_protocol_error())?;
        match self.response_id.as_deref() {
            Some(current) if current == conversation_id => Ok(()),
            Some(_) => Err(stream_protocol_error()),
            None => {
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
        }
    }

    fn observe_response(
        &mut self,
        response: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        if self.response_id.is_none() {
            return Err(stream_protocol_error());
        }
        if response.contains_key("error") {
            return Err(stream_protocol_error());
        }
        if let Some(token) = response.get("token") {
            let token = token.as_str().ok_or_else(stream_protocol_error)?;
            if !token.is_empty() {
                self.ensure_message(events)?;
                if response
                    .get("isThinking")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    self.emit(
                        events,
                        CanonicalEvent::ReasoningDelta(ReasoningDelta {
                            text: token.to_owned(),
                            extensions: RawExtensions::default(),
                        }),
                    )?;
                } else {
                    self.append_visible(token, events)?;
                }
            }
        }
        if let Some(model_response) = response.get("modelResponse") {
            let model_response = model_response
                .as_object()
                .ok_or_else(stream_protocol_error)?;
            let message = required_string(model_response, "message")?;
            self.append_final_message(message, events)?;
            self.final_seen = true;
        }
        Ok(())
    }

    fn ensure_message(&mut self, events: &mut Vec<CanonicalEvent>) -> Result<(), GatewayError> {
        if self.message_open {
            return Ok(());
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

    fn append_visible(
        &mut self,
        delta: &str,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let next_length = self
            .visible_text
            .len()
            .checked_add(delta.len())
            .ok_or_else(stream_protocol_error)?;
        if next_length > MAX_GROK_WEB_LIVE_VISIBLE_TEXT_BYTES {
            return Err(stream_protocol_error());
        }
        self.emit(
            events,
            CanonicalEvent::TextDelta(TextDelta {
                text: delta.to_owned(),
                extensions: RawExtensions::default(),
            }),
        )?;
        self.visible_text.push_str(delta);
        Ok(())
    }

    fn append_final_message(
        &mut self,
        message: &str,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        self.ensure_message(events)?;
        if message == self.visible_text {
            return Ok(());
        }
        let Some(delta) = message.strip_prefix(&self.visible_text) else {
            return Err(stream_protocol_error());
        };
        if !delta.is_empty() {
            self.append_visible(delta, events)?;
        }
        Ok(())
    }

    fn finish(&mut self, events: &mut Vec<CanonicalEvent>) -> Result<(), GatewayError> {
        if self.response_id.is_none() || !self.message_open {
            return Err(stream_truncated_error());
        }
        self.emit(
            events,
            CanonicalEvent::MessageEnd(MessageEnd {
                extensions: RawExtensions::default(),
            }),
        )?;
        self.message_open = false;
        self.emit(
            events,
            CanonicalEvent::ResponseEnd(ResponseEnd {
                stop_reason: Some("end_turn".to_owned()),
                ..ResponseEnd::default()
            }),
        )?;
        self.canonical.finish()
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

impl fmt::Debug for GrokWebLiveDecodeState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebLiveDecodeState")
            .field("canonical", &self.canonical)
            .field("response_started", &self.response_id.is_some())
            .field("message_open", &self.message_open)
            .field("visible_text_bytes", &self.visible_text.len())
            .field("final_seen", &self.final_seen)
            .finish()
    }
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, GatewayError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(stream_protocol_error)
}

const fn stream_protocol_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        gateway_core::ErrorScope::Stream,
    )
}

const fn stream_truncated_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::StreamTruncated,
        gateway_core::ErrorScope::Stream,
    )
}
