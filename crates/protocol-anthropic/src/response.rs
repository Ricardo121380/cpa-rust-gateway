use std::{collections::BTreeMap, fmt};

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

    /// Every P5 frame is client-visible semantic data.
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
/// supported P5 text/Tool/Usage slice or cannot be represented without loss.
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
            .field("content_block_count", &self.assembly.content.len())
            .field("tool_count", &self.assembly.tools.len())
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Assembly {
    response_id: Option<String>,
    message: MessagePhase,
    content: Vec<ContentBlock>,
    active_text_index: Option<usize>,
    next_sse_index: usize,
    active_sse_index: Option<usize>,
    tools: BTreeMap<String, ToolState>,
    usage: Option<Usage>,
    terminal: TerminalPhase,
}

enum ContentBlock {
    Text {
        text: String,
        deltas: Vec<String>,
        emitted_deltas: usize,
        closed: bool,
    },
    Tool {
        call_id: String,
        name: String,
        input: Option<Value>,
    },
}

impl ContentBlock {
    fn completed_value(&self) -> Result<Value, GatewayError> {
        match self {
            Self::Text { text, .. } => Ok(json!({"type": "text", "text": text})),
            Self::Tool {
                call_id,
                name,
                input: Some(input),
            } => Ok(json!({
                "type": "tool_use",
                "id": call_id,
                "name": name,
                "input": input,
            })),
            Self::Tool { input: None, .. } => Err(stream_protocol_error()),
        }
    }
}

#[derive(Clone)]
struct ToolState {
    content_index: usize,
    partial_json: String,
    argument_deltas: Vec<String>,
    emitted_deltas: usize,
    saw_arguments_delta: bool,
    completed: bool,
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
            CanonicalEvent::ToolCallStart(start) => self.start_tool(&start.call_id, &start.name),
            CanonicalEvent::ToolCallArgumentsDelta(delta) => {
                self.append_tool_arguments(&delta.call_id, &delta.delta)
            }
            CanonicalEvent::ToolCallEnd(end) => self.finish_tool(&end.call_id, end.arguments.get()),
            CanonicalEvent::MessageEnd(_) => self.end_message(),
            CanonicalEvent::ResponseEnd(_) => self.end_response(),
            CanonicalEvent::StreamError(error) => {
                self.terminal = TerminalPhase::Failed;
                Ok(vec![frame("error", encode_error(&error.error))])
            }
            CanonicalEvent::ReasoningDelta(_) => Err(stream_protocol_error()),
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
        let index = if let Some(index) = self.active_text_index {
            index
        } else {
            let index = self.content.len();
            self.content.push(ContentBlock::Text {
                text: String::new(),
                deltas: Vec::new(),
                emitted_deltas: 0,
                closed: false,
            });
            self.active_text_index = Some(index);
            self.message = MessagePhase::Content;
            index
        };
        match self.content.get_mut(index) {
            Some(ContentBlock::Text {
                text: accumulated,
                deltas,
                ..
            }) => {
                accumulated.push_str(text);
                deltas.push(text.to_owned());
            }
            Some(ContentBlock::Tool { .. }) | None => return Err(stream_protocol_error()),
        }
        self.flush_serialized_blocks()
    }

    fn start_tool(&mut self, call_id: &str, name: &str) -> Result<Vec<SseFrame>, GatewayError> {
        if !matches!(self.message, MessagePhase::Started | MessagePhase::Content)
            || self.tools.contains_key(call_id)
        {
            return Err(stream_protocol_error());
        }
        self.close_active_text_block()?;
        let index = self.content.len();
        self.content.push(ContentBlock::Tool {
            call_id: call_id.to_owned(),
            name: name.to_owned(),
            input: None,
        });
        self.tools.insert(
            call_id.to_owned(),
            ToolState {
                content_index: index,
                partial_json: String::new(),
                argument_deltas: Vec::new(),
                emitted_deltas: 0,
                saw_arguments_delta: false,
                completed: false,
            },
        );
        self.message = MessagePhase::Content;
        self.flush_serialized_blocks()
    }

    fn append_tool_arguments(
        &mut self,
        call_id: &str,
        delta: &str,
    ) -> Result<Vec<SseFrame>, GatewayError> {
        if delta.is_empty() {
            return Ok(Vec::new());
        }
        let tool = self
            .tools
            .get_mut(call_id)
            .ok_or_else(stream_protocol_error)?;
        if tool.completed {
            return Err(stream_protocol_error());
        }
        tool.saw_arguments_delta = true;
        tool.partial_json.push_str(delta);
        tool.argument_deltas.push(delta.to_owned());
        self.flush_serialized_blocks()
    }

    fn finish_tool(
        &mut self,
        call_id: &str,
        arguments: &str,
    ) -> Result<Vec<SseFrame>, GatewayError> {
        let tool = self
            .tools
            .get(call_id)
            .cloned()
            .ok_or_else(stream_protocol_error)?;
        if tool.completed {
            return Err(stream_protocol_error());
        }
        let arguments = normalize_tool_arguments(arguments);
        if tool.saw_arguments_delta && normalize_tool_arguments(&tool.partial_json) != arguments {
            return Err(stream_protocol_error());
        }
        let input: Value = serde_json::from_str(&arguments).map_err(|_| stream_protocol_error())?;
        if !input.is_object() {
            return Err(stream_protocol_error());
        }
        match self.content.get_mut(tool.content_index) {
            Some(ContentBlock::Tool {
                call_id: mapped_call_id,
                input: stored_input,
                ..
            }) if mapped_call_id == call_id => *stored_input = Some(input),
            Some(ContentBlock::Text { .. } | ContentBlock::Tool { .. }) | None => {
                return Err(stream_protocol_error());
            }
        }
        let completed = self
            .tools
            .get_mut(call_id)
            .ok_or_else(stream_protocol_error)?;
        completed.completed = true;
        if !tool.saw_arguments_delta && arguments != "{}" {
            completed.argument_deltas.push(arguments);
        }
        self.flush_serialized_blocks()
    }

    fn close_active_text_block(&mut self) -> Result<(), GatewayError> {
        let Some(index) = self.active_text_index.take() else {
            return Ok(());
        };
        match self.content.get_mut(index) {
            Some(ContentBlock::Text { closed, .. }) => *closed = true,
            Some(ContentBlock::Tool { .. }) | None => return Err(stream_protocol_error()),
        }
        Ok(())
    }

    fn end_message(&mut self) -> Result<Vec<SseFrame>, GatewayError> {
        if self.message != MessagePhase::Content {
            return Err(stream_protocol_error());
        }
        if self.tools.values().any(|tool| !tool.completed) {
            return Err(stream_protocol_error());
        }
        self.close_active_text_block()?;
        let frames = self.flush_serialized_blocks()?;
        if self.active_sse_index.is_some() || self.next_sse_index != self.content.len() {
            return Err(stream_protocol_error());
        }
        self.message = MessagePhase::Ended;
        Ok(frames)
    }

    /// Serializes logical Canonical content blocks in Anthropic's non-overlapping wire order.
    ///
    /// Canonical Tool argument events may be interleaved, while Anthropic requires one started
    /// content block to stop before the following block starts. The encoder therefore retains
    /// later Tool/Text fragments until every earlier logical block has closed, then flushes their
    /// original per-Tool fragment sequence under the preallocated stable index.
    fn flush_serialized_blocks(&mut self) -> Result<Vec<SseFrame>, GatewayError> {
        let mut frames = Vec::new();

        loop {
            let Some(index) = self.active_sse_index else {
                if self.next_sse_index == self.content.len() {
                    break;
                }
                self.start_next_sse_block(&mut frames)?;
                continue;
            };

            if index != self.next_sse_index {
                return Err(stream_protocol_error());
            }

            let ready_to_close = match self.content.get(index) {
                Some(ContentBlock::Text { .. }) => {
                    self.flush_active_text_block(index, &mut frames)?
                }
                Some(ContentBlock::Tool { .. }) => {
                    self.flush_active_tool_block(index, &mut frames)?
                }
                None => return Err(stream_protocol_error()),
            };
            if !ready_to_close {
                break;
            }
            self.stop_active_sse_block(index, &mut frames)?;
        }

        Ok(frames)
    }

    fn start_next_sse_block(&mut self, frames: &mut Vec<SseFrame>) -> Result<(), GatewayError> {
        let index = self.next_sse_index;
        match self.content.get(index) {
            Some(ContentBlock::Text { .. }) => frames.push(frame(
                "content_block_start",
                json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": {"type": "text", "text": ""}
                }),
            )),
            Some(ContentBlock::Tool { call_id, name, .. }) => frames.push(frame(
                "content_block_start",
                json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": {
                        "type": "tool_use",
                        "id": call_id,
                        "name": name,
                        "input": {}
                    }
                }),
            )),
            None => return Err(stream_protocol_error()),
        }
        self.active_sse_index = Some(index);
        Ok(())
    }

    fn flush_active_text_block(
        &mut self,
        index: usize,
        frames: &mut Vec<SseFrame>,
    ) -> Result<bool, GatewayError> {
        let (pending, closed) = match self.content.get(index) {
            Some(ContentBlock::Text {
                deltas,
                emitted_deltas,
                closed,
                ..
            }) => (deltas[*emitted_deltas..].to_vec(), *closed),
            Some(ContentBlock::Tool { .. }) | None => return Err(stream_protocol_error()),
        };
        for delta in &pending {
            frames.push(frame(
                "content_block_delta",
                json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": {"type": "text_delta", "text": delta}
                }),
            ));
        }
        if !pending.is_empty() {
            match self.content.get_mut(index) {
                Some(ContentBlock::Text { emitted_deltas, .. }) => {
                    *emitted_deltas = emitted_deltas
                        .checked_add(pending.len())
                        .ok_or_else(stream_protocol_error)?;
                }
                Some(ContentBlock::Tool { .. }) | None => return Err(stream_protocol_error()),
            }
        }
        Ok(closed)
    }

    fn flush_active_tool_block(
        &mut self,
        index: usize,
        frames: &mut Vec<SseFrame>,
    ) -> Result<bool, GatewayError> {
        let call_id = match self.content.get(index) {
            Some(ContentBlock::Tool { call_id, .. }) => call_id.clone(),
            Some(ContentBlock::Text { .. }) | None => return Err(stream_protocol_error()),
        };
        let (pending, completed) = {
            let tool = self.tools.get(&call_id).ok_or_else(stream_protocol_error)?;
            (
                tool.argument_deltas[tool.emitted_deltas..].to_vec(),
                tool.completed,
            )
        };
        for delta in &pending {
            frames.push(frame(
                "content_block_delta",
                json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": {"type": "input_json_delta", "partial_json": delta}
                }),
            ));
        }
        if !pending.is_empty() {
            let tool = self
                .tools
                .get_mut(&call_id)
                .ok_or_else(stream_protocol_error)?;
            tool.emitted_deltas = tool
                .emitted_deltas
                .checked_add(pending.len())
                .ok_or_else(stream_protocol_error)?;
        }
        if completed
            && !matches!(
                self.content.get(index),
                Some(ContentBlock::Tool { input: Some(_), .. })
            )
        {
            return Err(stream_protocol_error());
        }
        Ok(completed)
    }

    fn stop_active_sse_block(
        &mut self,
        index: usize,
        frames: &mut Vec<SseFrame>,
    ) -> Result<(), GatewayError> {
        frames.push(frame(
            "content_block_stop",
            json!({"type": "content_block_stop", "index": index}),
        ));
        self.active_sse_index = None;
        self.next_sse_index = self
            .next_sse_index
            .checked_add(1)
            .ok_or_else(stream_protocol_error)?;
        Ok(())
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
        let stop_reason = if self.tools.is_empty() {
            "end_turn"
        } else {
            "tool_use"
        };
        self.terminal = TerminalPhase::Completed;
        Ok(vec![
            frame(
                "message_delta",
                json!({
                    "type": "message_delta",
                    "delta": {"stop_reason": stop_reason, "stop_sequence": null},
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
        let content = self
            .content
            .iter()
            .map(ContentBlock::completed_value)
            .collect::<Result<Vec<_>, _>>()?;
        let stop_reason = if self.tools.is_empty() {
            "end_turn"
        } else {
            "tool_use"
        };
        Ok(json!({
            "id": id,
            "type": "message",
            "role": "assistant",
            "content": content,
            "model": metadata.model(),
            "stop_reason": stop_reason,
            "stop_sequence": null,
            "usage": {
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
            }
        }))
    }
}

fn normalize_tool_arguments(arguments: &str) -> String {
    if arguments.trim().is_empty() {
        return "{}".to_owned();
    }
    match serde_json::from_str::<Value>(arguments) {
        Ok(Value::Object(object)) if object.is_empty() => "{}".to_owned(),
        Ok(_) | Err(_) => arguments.to_owned(),
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
