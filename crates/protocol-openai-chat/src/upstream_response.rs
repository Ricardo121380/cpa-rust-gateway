//! OpenAI-compatible Chat Completions upstream response decoding.

use std::{collections::BTreeMap, fmt};

use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalResponse, ErrorScope, GatewayError,
    GatewayErrorCode, MessageEnd, MessageRole, MessageStart, RawExtensions, RawJson, ResponseEnd,
    ResponseId, ResponseStart, TextDelta, ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart,
    Usage, UsageDelta,
};
use serde_json::{Map, Value};

use super::reject_duplicate_json_names;

const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOOL_CALLS: usize = 32;
const MAX_IDENTIFIER_BYTES: usize = 256;

/// Decodes one complete OpenAI-compatible `chat.completion` response.
///
/// # Errors
///
/// Returns `UpstreamProtocolError/Stream` for malformed, ambiguous, multi-choice, unsupported, or
/// incomplete response semantics.
pub fn decode_upstream_response(input: &str) -> Result<Vec<CanonicalEvent>, GatewayError> {
    reject_duplicate_json_names(input).map_err(|_| protocol_error())?;
    let value: Value = serde_json::from_str(input).map_err(|_| protocol_error())?;
    let root = object(&value)?;
    require_only_keys(
        root,
        &[
            "id",
            "object",
            "created",
            "model",
            "choices",
            "usage",
            "system_fingerprint",
            "service_tier",
        ],
    )?;
    if root.get("object").and_then(Value::as_str) != Some("chat.completion") {
        return Err(protocol_error());
    }
    let response_id = response_id(root)?;
    let choice = single_choice(root)?;
    require_only_keys(choice, &["index", "message", "finish_reason", "logprobs"])?;
    if choice.get("logprobs").is_some_and(|value| !value.is_null()) {
        return Err(protocol_error());
    }
    if choice.get("index").and_then(Value::as_u64) != Some(0) {
        return Err(protocol_error());
    }
    let message = object(required(choice, "message")?)?;
    require_only_keys(message, &["role", "content", "tool_calls", "refusal"])?;
    if message.get("refusal").is_some_and(|value| !value.is_null()) {
        return Err(protocol_error());
    }
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return Err(protocol_error());
    }

    let mut events = vec![
        CanonicalEvent::ResponseStart(ResponseStart {
            response_id,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
            extensions: RawExtensions::default(),
        }),
    ];
    let mut emitted = false;
    if let Some(content) = message.get("content") {
        match content {
            Value::Null => {}
            Value::String(text) if text.is_empty() => {}
            Value::String(text) => {
                events.push(CanonicalEvent::TextDelta(TextDelta {
                    text: text.clone(),
                    extensions: RawExtensions::default(),
                }));
                emitted = true;
            }
            _ => return Err(protocol_error()),
        }
    }
    if let Some(calls) = message.get("tool_calls") {
        let calls = calls.as_array().ok_or_else(protocol_error)?;
        if calls.is_empty() || calls.len() > MAX_TOOL_CALLS {
            return Err(protocol_error());
        }
        for call in calls {
            decode_completed_tool_call(call, &mut events)?;
            emitted = true;
        }
    }
    if !emitted {
        return Err(protocol_error());
    }
    events.push(CanonicalEvent::MessageEnd(MessageEnd::default()));
    if let Some(usage) = root.get("usage") {
        events.push(CanonicalEvent::UsageDelta(UsageDelta {
            usage: decode_usage(usage)?,
            is_final: true,
            extensions: RawExtensions::default(),
        }));
    }
    events.push(CanonicalEvent::ResponseEnd(ResponseEnd {
        stop_reason: Some(decode_finish_reason(choice)?.to_owned()),
        stop_sequence: None,
        extensions: RawExtensions::default(),
    }));
    CanonicalResponse::try_new(events)
        .map(CanonicalResponse::into_events)
        .map_err(|_| protocol_error())
}

fn decode_completed_tool_call(
    value: &Value,
    events: &mut Vec<CanonicalEvent>,
) -> Result<(), GatewayError> {
    let call = object(value)?;
    require_only_keys(call, &["id", "type", "function"])?;
    if call.get("type").and_then(Value::as_str) != Some("function") {
        return Err(protocol_error());
    }
    let id = nonempty(call, "id")?.to_owned();
    let function = object(required(call, "function")?)?;
    require_only_keys(function, &["name", "arguments"])?;
    let name = nonempty(function, "name")?.to_owned();
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .ok_or_else(protocol_error)?;
    validate_identifier(&id)?;
    validate_identifier(&name)?;
    let arguments =
        RawJson::from_json_string(arguments.to_owned()).map_err(|_| protocol_error())?;
    events.push(CanonicalEvent::ToolCallStart(ToolCallStart {
        call_id: id.clone(),
        name,
        extensions: RawExtensions::default(),
    }));
    events.push(CanonicalEvent::ToolCallArgumentsDelta(
        ToolCallArgumentsDelta {
            call_id: id.clone(),
            delta: arguments.get().to_owned(),
            extensions: RawExtensions::default(),
        },
    ));
    events.push(CanonicalEvent::ToolCallEnd(ToolCallEnd {
        call_id: id,
        arguments,
        extensions: RawExtensions::default(),
    }));
    Ok(())
}

/// Bounded, transport-free decoder for an OpenAI-compatible Chat SSE body.
#[derive(Default)]
pub struct OpenAiChatSseDecoder {
    buffer: Vec<u8>,
    state: CanonicalEventState,
    lifecycle: StreamLifecycle,
    pending: Vec<CanonicalEvent>,
}

impl fmt::Debug for OpenAiChatSseDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiChatSseDecoder")
            .field("buffered_bytes", &self.buffer.len())
            .field("pending_events", &self.pending.len())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
enum StreamLifecycle {
    #[default]
    AwaitingStart,
    Streaming(StreamState),
    Finished,
}

#[derive(Default)]
struct StreamState {
    response_id: Option<String>,
    role_seen: bool,
    content_seen: bool,
    tools: BTreeMap<u64, OpenTool>,
    finish_reason: Option<String>,
    message_closed: bool,
}

struct OpenTool {
    id: String,
    arguments: String,
}

impl OpenAiChatSseDecoder {
    /// Creates an empty decoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether the unique terminal `[DONE]` marker was accepted.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(self.lifecycle, StreamLifecycle::Finished)
    }

    /// Appends arbitrary transport bytes and returns all newly decoded Canonical events.
    ///
    /// # Errors
    ///
    /// Returns a stream-scoped protocol error for an oversized frame or invalid SSE/Chat event.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<CanonicalEvent>, GatewayError> {
        if matches!(self.lifecycle, StreamLifecycle::Finished) {
            return Err(protocol_error());
        }
        let mut remaining = chunk;
        while !remaining.is_empty() {
            let available = MAX_FRAME_BYTES
                .saturating_add(4)
                .saturating_sub(self.buffer.len());
            if available == 0 {
                return Err(protocol_error());
            }
            let take = available.min(remaining.len());
            self.buffer.extend_from_slice(&remaining[..take]);
            remaining = &remaining[take..];
            let mut drained = false;
            while let Some((frame_end, delimiter_len)) = find_frame(&self.buffer) {
                if frame_end > MAX_FRAME_BYTES {
                    return Err(protocol_error());
                }
                let frame = self.buffer[..frame_end].to_vec();
                self.buffer.drain(..frame_end + delimiter_len);
                self.decode_frame(&frame)?;
                drained = true;
            }
            if !remaining.is_empty() && !drained {
                return Err(protocol_error());
            }
        }
        Ok(std::mem::take(&mut self.pending))
    }

    /// Verifies that the byte source ended after exactly one `[DONE]` marker.
    ///
    /// # Errors
    ///
    /// Returns `StreamTruncated/Stream` when the source ends before `[DONE]`, or a protocol error
    /// when undecoded non-whitespace bytes remain.
    pub fn finish(&mut self) -> Result<Vec<CanonicalEvent>, GatewayError> {
        if !self.buffer.iter().all(u8::is_ascii_whitespace) {
            return Err(truncated_error());
        }
        self.buffer.clear();
        self.state.finish().map_err(|_| truncated_error())?;
        if !matches!(self.lifecycle, StreamLifecycle::Finished) {
            return Err(truncated_error());
        }
        Ok(std::mem::take(&mut self.pending))
    }

    fn decode_frame(&mut self, frame: &[u8]) -> Result<(), GatewayError> {
        let frame = std::str::from_utf8(frame).map_err(|_| protocol_error())?;
        let mut data = Vec::new();
        for line in frame.lines() {
            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            let Some(value) = line.strip_prefix("data:") else {
                return Err(protocol_error());
            };
            data.push(value.strip_prefix(' ').unwrap_or(value));
        }
        if data.is_empty() {
            return Ok(());
        }
        let data = data.join("\n");
        if data == "[DONE]" {
            return self.complete();
        }
        reject_duplicate_json_names(&data).map_err(|_| protocol_error())?;
        let value: Value = serde_json::from_str(&data).map_err(|_| protocol_error())?;
        self.decode_chunk(&value)
    }

    fn decode_chunk(&mut self, value: &Value) -> Result<(), GatewayError> {
        let root = object(value)?;
        require_only_keys(
            root,
            &[
                "id",
                "object",
                "created",
                "model",
                "choices",
                "usage",
                "system_fingerprint",
                "service_tier",
            ],
        )?;
        if root.get("object").and_then(Value::as_str) != Some("chat.completion.chunk") {
            return Err(protocol_error());
        }
        let id = nonempty(root, "id")?;
        match &mut self.lifecycle {
            StreamLifecycle::AwaitingStart => {
                let response_id = ResponseId::try_new(id).map_err(|_| protocol_error())?;
                self.emit(CanonicalEvent::ResponseStart(ResponseStart {
                    response_id,
                    extensions: RawExtensions::default(),
                }))?;
                self.emit(CanonicalEvent::MessageStart(MessageStart {
                    role: MessageRole("assistant".to_owned()),
                    extensions: RawExtensions::default(),
                }))?;
                self.lifecycle = StreamLifecycle::Streaming(StreamState {
                    response_id: Some(id.to_owned()),
                    ..StreamState::default()
                });
            }
            StreamLifecycle::Streaming(state) if state.response_id.as_deref() != Some(id) => {
                return Err(protocol_error());
            }
            StreamLifecycle::Streaming(_) => {}
            StreamLifecycle::Finished => return Err(protocol_error()),
        }

        let choices = root
            .get("choices")
            .and_then(Value::as_array)
            .ok_or_else(protocol_error)?;
        if choices.is_empty() {
            if let Some(usage) = root.get("usage") {
                let usage = decode_usage(usage)?;
                self.emit(CanonicalEvent::UsageDelta(UsageDelta {
                    usage,
                    is_final: true,
                    extensions: RawExtensions::default(),
                }))?;
                return Ok(());
            }
            return Err(protocol_error());
        }
        if choices.len() != 1 {
            return Err(protocol_error());
        }
        let choice = object(&choices[0])?;
        require_only_keys(choice, &["index", "delta", "finish_reason", "logprobs"])?;
        if choice.get("logprobs").is_some_and(|value| !value.is_null()) {
            return Err(protocol_error());
        }
        if choice.get("index").and_then(Value::as_u64) != Some(0) {
            return Err(protocol_error());
        }
        let delta = object(required(choice, "delta")?)?;
        self.decode_delta(delta)?;
        if !choice.get("finish_reason").is_none_or(Value::is_null) {
            let reason = decode_finish_reason(choice)?.to_owned();
            self.close_message(reason)?;
        }
        Ok(())
    }

    fn decode_delta(&mut self, delta: &Map<String, Value>) -> Result<(), GatewayError> {
        require_only_keys(delta, &["role", "content", "tool_calls", "refusal"])?;
        if delta.get("refusal").is_some_and(|value| !value.is_null()) {
            return Err(protocol_error());
        }
        let StreamLifecycle::Streaming(state) = &mut self.lifecycle else {
            return Err(protocol_error());
        };
        if let Some(role) = delta.get("role") {
            if role.as_str() != Some("assistant") || state.role_seen {
                return Err(protocol_error());
            }
            state.role_seen = true;
        }
        let text = match delta.get("content") {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) if text.is_empty() => None,
            Some(Value::String(text)) => Some(text.clone()),
            Some(_) => return Err(protocol_error()),
        };
        let calls = delta
            .get("tool_calls")
            .map(|value| value.as_array().ok_or_else(protocol_error))
            .transpose()?
            .cloned();
        if let Some(text) = text {
            state.content_seen = true;
            self.emit(CanonicalEvent::TextDelta(TextDelta {
                text,
                extensions: RawExtensions::default(),
            }))?;
        }
        if let Some(calls) = calls {
            for call in &calls {
                self.decode_tool_delta(call)?;
            }
        }
        Ok(())
    }

    fn decode_tool_delta(&mut self, value: &Value) -> Result<(), GatewayError> {
        let call = object(value)?;
        require_only_keys(call, &["index", "id", "type", "function"])?;
        let index = call
            .get("index")
            .and_then(Value::as_u64)
            .ok_or_else(protocol_error)?;
        let StreamLifecycle::Streaming(state) = &mut self.lifecycle else {
            return Err(protocol_error());
        };
        if !state.tools.contains_key(&index) {
            if state.tools.len() >= MAX_TOOL_CALLS
                || call.get("type").and_then(Value::as_str) != Some("function")
            {
                return Err(protocol_error());
            }
            let id = nonempty(call, "id")?.to_owned();
            let function = object(required(call, "function")?)?;
            require_only_keys(function, &["name", "arguments"])?;
            let name = nonempty(function, "name")?.to_owned();
            validate_identifier(&id)?;
            validate_identifier(&name)?;
            let arguments = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            self.emit(CanonicalEvent::ToolCallStart(ToolCallStart {
                call_id: id.clone(),
                name: name.clone(),
                extensions: RawExtensions::default(),
            }))?;
            if !arguments.is_empty() {
                self.emit(CanonicalEvent::ToolCallArgumentsDelta(
                    ToolCallArgumentsDelta {
                        call_id: id.clone(),
                        delta: arguments.clone(),
                        extensions: RawExtensions::default(),
                    },
                ))?;
            }
            let StreamLifecycle::Streaming(state) = &mut self.lifecycle else {
                return Err(protocol_error());
            };
            state.tools.insert(index, OpenTool { id, arguments });
            state.content_seen = true;
            return Ok(());
        }
        if call.contains_key("id") || call.contains_key("type") {
            return Err(protocol_error());
        }
        let function = object(required(call, "function")?)?;
        require_only_keys(function, &["arguments"])?;
        if function.contains_key("name") {
            return Err(protocol_error());
        }
        let fragment = function
            .get("arguments")
            .and_then(Value::as_str)
            .ok_or_else(protocol_error)?;
        let tool = state.tools.get_mut(&index).ok_or_else(protocol_error)?;
        tool.arguments.push_str(fragment);
        if tool.arguments.len() > MAX_FRAME_BYTES {
            return Err(protocol_error());
        }
        let id = tool.id.clone();
        if !fragment.is_empty() {
            self.emit(CanonicalEvent::ToolCallArgumentsDelta(
                ToolCallArgumentsDelta {
                    call_id: id,
                    delta: fragment.to_owned(),
                    extensions: RawExtensions::default(),
                },
            ))?;
        }
        Ok(())
    }

    fn close_message(&mut self, finish_reason: String) -> Result<(), GatewayError> {
        let StreamLifecycle::Streaming(state) = &mut self.lifecycle else {
            return Err(protocol_error());
        };
        if state.message_closed || !state.content_seen {
            return Err(protocol_error());
        }
        let tools = std::mem::take(&mut state.tools);
        for (_, tool) in tools {
            let arguments =
                RawJson::from_json_string(tool.arguments).map_err(|_| protocol_error())?;
            self.emit(CanonicalEvent::ToolCallEnd(ToolCallEnd {
                call_id: tool.id,
                arguments,
                extensions: RawExtensions::default(),
            }))?;
        }
        self.emit(CanonicalEvent::MessageEnd(MessageEnd::default()))?;
        let StreamLifecycle::Streaming(state) = &mut self.lifecycle else {
            return Err(protocol_error());
        };
        state.finish_reason = Some(finish_reason);
        state.message_closed = true;
        Ok(())
    }

    fn complete(&mut self) -> Result<(), GatewayError> {
        let StreamLifecycle::Streaming(state) = &mut self.lifecycle else {
            return Err(protocol_error());
        };
        if !state.message_closed {
            return Err(truncated_error());
        }
        let reason = state.finish_reason.take().ok_or_else(protocol_error)?;
        self.emit(CanonicalEvent::ResponseEnd(ResponseEnd {
            stop_reason: Some(reason),
            stop_sequence: None,
            extensions: RawExtensions::default(),
        }))?;
        self.lifecycle = StreamLifecycle::Finished;
        Ok(())
    }

    fn emit(&mut self, event: CanonicalEvent) -> Result<(), GatewayError> {
        self.state.apply(&event).map_err(|_| protocol_error())?;
        self.pending.push(event);
        Ok(())
    }
}

fn find_frame(buffer: &[u8]) -> Option<(usize, usize)> {
    buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|position| (position, 2))
        .or_else(|| {
            buffer
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|position| (position, 4))
        })
}

fn decode_usage(value: &Value) -> Result<Usage, GatewayError> {
    let usage = object(value)?;
    require_only_keys(
        usage,
        &[
            "prompt_tokens",
            "completion_tokens",
            "total_tokens",
            "prompt_tokens_details",
            "completion_tokens_details",
        ],
    )?;
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(Value::as_u64)
        .ok_or_else(protocol_error)?;
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .ok_or_else(protocol_error)?;
    let expected_total = input_tokens
        .checked_add(output_tokens)
        .ok_or_else(protocol_error)?;
    if usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .is_some_and(|total| total != expected_total)
    {
        return Err(protocol_error());
    }
    let cached_tokens = match usage.get("prompt_tokens_details") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let details = object(value)?;
            require_only_keys(details, &["cached_tokens", "audio_tokens"])?;
            if details
                .get("audio_tokens")
                .and_then(Value::as_u64)
                .is_some_and(|tokens| tokens != 0)
            {
                return Err(protocol_error());
            }
            details.get("cached_tokens").and_then(Value::as_u64)
        }
    };
    let reasoning_tokens = match usage.get("completion_tokens_details") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let details = object(value)?;
            require_only_keys(
                details,
                &[
                    "reasoning_tokens",
                    "audio_tokens",
                    "accepted_prediction_tokens",
                    "rejected_prediction_tokens",
                ],
            )?;
            for name in [
                "audio_tokens",
                "accepted_prediction_tokens",
                "rejected_prediction_tokens",
            ] {
                if details
                    .get(name)
                    .and_then(Value::as_u64)
                    .is_some_and(|tokens| tokens != 0)
                {
                    return Err(protocol_error());
                }
            }
            details.get("reasoning_tokens").and_then(Value::as_u64)
        }
    };
    Ok(Usage {
        input_tokens: Some(input_tokens),
        output_tokens: Some(output_tokens),
        reasoning_tokens,
        cache_read_tokens: None,
        cache_creation_tokens: None,
        cached_tokens,
        extensions: RawExtensions::default(),
    })
}

fn single_choice(root: &Map<String, Value>) -> Result<&Map<String, Value>, GatewayError> {
    let choices = root
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(protocol_error)?;
    let [choice] = choices.as_slice() else {
        return Err(protocol_error());
    };
    object(choice)
}

fn decode_finish_reason(choice: &Map<String, Value>) -> Result<&str, GatewayError> {
    match choice.get("finish_reason").and_then(Value::as_str) {
        Some("stop") => Ok("end_turn"),
        Some("length") => Ok("max_tokens"),
        Some("tool_calls") => Ok("tool_use"),
        _ => Err(protocol_error()),
    }
}

fn response_id(root: &Map<String, Value>) -> Result<ResponseId, GatewayError> {
    ResponseId::try_new(nonempty(root, "id")?).map_err(|_| protocol_error())
}

fn validate_identifier(value: &str) -> Result<(), GatewayError> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        Err(protocol_error())
    } else {
        Ok(())
    }
}

fn required<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a Value, GatewayError> {
    object.get(name).ok_or_else(protocol_error)
}

fn nonempty<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, GatewayError> {
    let value = object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(protocol_error)?;
    if value.is_empty() {
        return Err(protocol_error());
    }
    Ok(value)
}

fn object(value: &Value) -> Result<&Map<String, Value>, GatewayError> {
    value.as_object().ok_or_else(protocol_error)
}

fn require_only_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), GatewayError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        Err(protocol_error())
    } else {
        Ok(())
    }
}

const fn protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

const fn truncated_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::StreamTruncated, ErrorScope::Stream)
}

#[cfg(test)]
mod tests {
    use gateway_core::{CanonicalEvent, GatewayErrorCode};

    use super::{OpenAiChatSseDecoder, decode_upstream_response};

    #[test]
    fn non_streaming_text_tool_usage_and_finish_are_canonical()
    -> Result<(), Box<dyn std::error::Error>> {
        let input = r#"{"id":"chat-1","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"hi","tool_calls":[{"id":"call-1","type":"function","function":{"name":"lookup","arguments":"{\"q\":1}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}"#;
        let events = decode_upstream_response(input)?;
        assert!(matches!(
            events.first(),
            Some(CanonicalEvent::ResponseStart(_))
        ));
        assert!(
            matches!(events.last(), Some(CanonicalEvent::ResponseEnd(end)) if end.stop_reason.as_deref() == Some("tool_use"))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, CanonicalEvent::ToolCallEnd(_)))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, CanonicalEvent::UsageDelta(_)))
        );
        Ok(())
    }

    #[test]
    fn streaming_tool_arguments_are_invariant_to_transport_chunking()
    -> Result<(), Box<dyn std::error::Error>> {
        let wire = concat!(
            "data: {\"id\":\"chat-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"call-1\",\"type\":\"function\",\"function\":{\"name\":\"lookup\",\"arguments\":\"{\\\"q\\\":\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chat-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1}\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chat-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: {\"id\":\"chat-1\",\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}\n\n",
            "data: [DONE]\n\n"
        );
        let mut one = OpenAiChatSseDecoder::new();
        let mut expected = one.push(wire.as_bytes())?;
        expected.extend(one.finish()?);

        let mut split = OpenAiChatSseDecoder::new();
        let mut actual = Vec::new();
        for chunk in wire.as_bytes().chunks(7) {
            actual.extend(split.push(chunk)?);
        }
        actual.extend(split.finish()?);
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn missing_done_and_unknown_finish_reason_fail_closed() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut decoder = OpenAiChatSseDecoder::new();
        decoder.push(b"data: {\"id\":\"x\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"x\"},\"finish_reason\":null}]}\n\n")?;
        assert_eq!(
            decoder.finish().err().map(|error| error.code()),
            Some(GatewayErrorCode::StreamTruncated)
        );
        assert!(decode_upstream_response(r#"{"id":"x","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"x"},"finish_reason":"content_filter"}]}"#).is_err());
        Ok(())
    }
}
