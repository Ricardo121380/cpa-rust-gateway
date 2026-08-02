//! `OpenAI` Responses upstream JSON and SSE decoding.
//!
//! The decoder owns no transport, clock, credential, route, or client-protocol policy. Arbitrary
//! byte chunks are reassembled under fixed bounds and projected into the same Canonical semantic
//! lifecycle as one complete JSON response.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalResponse, ErrorScope, GatewayError,
    GatewayErrorCode, MessageEnd, MessageRole, MessageStart, RawExtensions, RawJson,
    ReasoningDelta, ResponseEnd, ResponseId, ResponseStart, StreamError, TextDelta,
    ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart, Usage, UsageDelta,
};
use serde_json::{Map, Value};

use super::reject_duplicate_json_names;

const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_FRAME_BYTES: usize = MAX_RESPONSE_BYTES;
const MAX_TOOL_ARGUMENT_BYTES: usize = MAX_RESPONSE_BYTES;
const MAX_OUTPUT_ITEMS: usize = 64;
const MAX_TOOL_CALLS: usize = 32;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_PROGRESS_FREE_FRAMES: usize = 4_096;
const JSON_WHITESPACE: [char; 4] = [' ', '\t', '\n', '\r'];

/// Decodes one complete successful Responses JSON envelope.
///
/// # Errors
///
/// Returns a stream-scoped upstream protocol error for an oversized, malformed, ambiguous,
/// failed, unknown, or semantically unrepresentable response.
pub fn decode_upstream_response(input: &str) -> Result<Vec<CanonicalEvent>, GatewayError> {
    if input.len() > MAX_RESPONSE_BYTES {
        return Err(protocol_error());
    }
    reject_duplicate_json_names(input).map_err(|_| protocol_error())?;
    let value: Value = serde_json::from_str(input).map_err(|_| protocol_error())?;
    let root = object(&value)?;
    require_only_keys(root, RESPONSE_FIELDS)?;
    validate_proven_response_metadata(root)?;
    if root.get("object").and_then(Value::as_str) != Some("response")
        || root.get("error").is_some_and(|value| !value.is_null())
    {
        return Err(protocol_error());
    }
    let status = required_string(root, "status")?;
    if !matches!(status, "completed" | "incomplete") {
        return Err(protocol_error());
    }
    let response_id = ResponseId::try_new(identifier(root, "id")?).map_err(|_| protocol_error())?;
    let usage = decode_usage(root.get("usage"))?;
    let output = root
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(protocol_error)?;
    if output.is_empty() || output.len() > MAX_OUTPUT_ITEMS {
        return Err(protocol_error());
    }

    let mut events = vec![CanonicalEvent::ResponseStart(ResponseStart {
        response_id,
        extensions: RawExtensions::default(),
    })];
    if let Some(usage) = usage.as_ref().filter(|usage| usage.input_tokens.is_some()) {
        events.push(CanonicalEvent::UsageDelta(UsageDelta {
            usage: initial_usage(usage),
            is_final: false,
            extensions: RawExtensions::default(),
        }));
    }
    let mut state = CompletedState::default();
    for item in output {
        state.decode_item(item, &mut events)?;
    }
    if !state.emitted_content {
        return Err(protocol_error());
    }
    events.push(CanonicalEvent::MessageEnd(MessageEnd::default()));
    if let Some(usage) = usage {
        events.push(CanonicalEvent::UsageDelta(UsageDelta {
            usage,
            is_final: true,
            extensions: RawExtensions::default(),
        }));
    }
    events.push(CanonicalEvent::ResponseEnd(ResponseEnd {
        stop_reason: Some(stop_reason(root, status, state.has_tools())?.to_owned()),
        stop_sequence: None,
        extensions: RawExtensions::default(),
    }));
    CanonicalResponse::try_new(events)
        .map(CanonicalResponse::into_events)
        .map_err(|_| protocol_error())
}

const RESPONSE_FIELDS: &[&str] = &[
    "background",
    "completed_at",
    "created_at",
    "error",
    "frequency_penalty",
    "id",
    "incomplete_details",
    "instructions",
    "max_output_tokens",
    "max_tool_calls",
    "metadata",
    "model",
    "moderation",
    "object",
    "output",
    "parallel_tool_calls",
    "presence_penalty",
    "previous_response_id",
    "prompt_cache_key",
    "prompt_cache_retention",
    "reasoning",
    "safety_identifier",
    "service_tier",
    "status",
    "store",
    "temperature",
    "text",
    "tool_choice",
    "tool_usage",
    "tools",
    "top_logprobs",
    "top_p",
    "truncation",
    "usage",
    "user",
];

#[derive(Default)]
struct CompletedState {
    message_open: bool,
    emitted_content: bool,
    call_ids: BTreeSet<String>,
}

impl CompletedState {
    fn ensure_message(&mut self, events: &mut Vec<CanonicalEvent>) {
        if !self.message_open {
            events.push(CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }));
            self.message_open = true;
        }
    }

    fn decode_item(
        &mut self,
        item: &Value,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item = object(item)?;
        match required_string(item, "type")? {
            "message" => self.decode_message(item, events),
            "reasoning" => self.decode_reasoning(item, events),
            "function_call" => self.decode_tool(item, events),
            _ => Err(protocol_error()),
        }
    }

    fn decode_message(
        &mut self,
        item: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        require_only_keys(
            item,
            &[
                "content",
                "id",
                "internal_chat_message_metadata_passthrough",
                "metadata",
                "phase",
                "role",
                "status",
                "type",
            ],
        )?;
        let _ = identifier(item, "id")?;
        if item.get("role").and_then(Value::as_str) != Some("assistant")
            || !completed_or_absent(item.get("status"))
        {
            return Err(protocol_error());
        }
        validate_proven_message_metadata(item)?;
        let content = item
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(protocol_error)?;
        for part in content {
            let part = object(part)?;
            require_only_keys(part, &["annotations", "logprobs", "text", "type"])?;
            if part.get("type").and_then(Value::as_str) != Some("output_text")
                || part
                    .get("annotations")
                    .is_some_and(|value| !value.as_array().is_some_and(Vec::is_empty))
                || part.get("logprobs").is_some_and(|value| {
                    !value.is_null() && !value.as_array().is_some_and(Vec::is_empty)
                })
            {
                return Err(protocol_error());
            }
            let text = required_string(part, "text")?;
            self.ensure_message(events);
            events.push(CanonicalEvent::TextDelta(TextDelta {
                text: text.to_owned(),
                extensions: RawExtensions::default(),
            }));
            self.emitted_content = true;
        }
        Ok(())
    }

    fn decode_reasoning(
        &mut self,
        item: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        require_only_keys(
            item,
            &[
                "content",
                "encrypted_content",
                "id",
                "status",
                "summary",
                "type",
            ],
        )?;
        let _ = identifier(item, "id")?;
        if !completed_or_absent(item.get("status"))
            || item
                .get("encrypted_content")
                .is_some_and(|value| !value.is_null())
        {
            return Err(protocol_error());
        }
        let mut emitted = false;
        for (field, part_type) in [("summary", "summary_text"), ("content", "reasoning_text")] {
            let Some(parts) = item.get(field) else {
                continue;
            };
            let parts = parts.as_array().ok_or_else(protocol_error)?;
            for part in parts {
                let part = object(part)?;
                require_only_keys(part, &["text", "type"])?;
                if part.get("type").and_then(Value::as_str) != Some(part_type) {
                    return Err(protocol_error());
                }
                let text = required_string(part, "text")?;
                self.ensure_message(events);
                events.push(CanonicalEvent::ReasoningDelta(ReasoningDelta {
                    text: text.to_owned(),
                    extensions: RawExtensions::default(),
                }));
                emitted = true;
                self.emitted_content = true;
            }
        }
        if emitted {
            Ok(())
        } else {
            Err(protocol_error())
        }
    }

    fn decode_tool(
        &mut self,
        item: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        require_only_keys(
            item,
            &["arguments", "call_id", "id", "name", "status", "type"],
        )?;
        let _ = identifier(item, "id")?;
        let call_id = identifier(item, "call_id")?.to_owned();
        let name = identifier(item, "name")?.to_owned();
        if self.call_ids.len() >= MAX_TOOL_CALLS
            || !self.call_ids.insert(call_id.clone())
            || !completed_or_absent(item.get("status"))
        {
            return Err(protocol_error());
        }
        let arguments = required_string(item, "arguments")?;
        if arguments.len() > MAX_TOOL_ARGUMENT_BYTES {
            return Err(protocol_error());
        }
        let arguments =
            RawJson::from_json_string(arguments.to_owned()).map_err(|_| protocol_error())?;
        self.ensure_message(events);
        events.extend([
            CanonicalEvent::ToolCallStart(ToolCallStart {
                call_id: call_id.clone(),
                name,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
                call_id: call_id.clone(),
                delta: arguments.get().to_owned(),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::ToolCallEnd(ToolCallEnd {
                call_id,
                arguments,
                extensions: RawExtensions::default(),
            }),
        ]);
        self.emitted_content = true;
        Ok(())
    }

    fn has_tools(&self) -> bool {
        !self.call_ids.is_empty()
    }
}

/// Bounded transport-free decoder for one Responses SSE body.
pub struct OpenAiResponsesSseDecoder {
    buffer: Vec<u8>,
    consumed: usize,
    scanned: usize,
    state: CanonicalEventState,
    lifecycle: SseLifecycle,
    pending: Vec<CanonicalEvent>,
    progress_free_frames: usize,
    progress_marks: u64,
}

impl Default for OpenAiResponsesSseDecoder {
    fn default() -> Self {
        Self {
            buffer: Vec::new(),
            consumed: 0,
            scanned: 0,
            state: CanonicalEventState::default(),
            lifecycle: SseLifecycle::AwaitingStart,
            pending: Vec::new(),
            progress_free_frames: 0,
            progress_marks: 0,
        }
    }
}

impl fmt::Debug for OpenAiResponsesSseDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiResponsesSseDecoder")
            .field(
                "buffered_bytes",
                &self.buffer.len().saturating_sub(self.consumed),
            )
            .field("pending_events", &self.pending.len())
            .field("progress_marks", &self.progress_marks)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

enum SseLifecycle {
    AwaitingStart,
    Streaming(SseState),
    Finished,
}

struct SseState {
    response_id: String,
    message_open: bool,
    emitted_content: bool,
    output_items: BTreeSet<String>,
    text_items: BTreeSet<String>,
    reasoning_items: BTreeSet<String>,
    tools: BTreeMap<String, OpenTool>,
    call_ids: BTreeSet<String>,
    retained_argument_bytes: usize,
}

struct OpenTool {
    call_id: String,
    assembled: String,
    released: usize,
    ended: bool,
}

impl OpenAiResponsesSseDecoder {
    /// Creates an empty Responses decoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether a unique terminal semantic frame was accepted.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(self.lifecycle, SseLifecycle::Finished)
    }

    /// Returns the monotone number of accepted generation-progress frames.
    #[must_use]
    pub const fn progress_marks(&self) -> u64 {
        self.progress_marks
    }

    /// Appends arbitrary transport bytes and returns every newly decoded Canonical event.
    ///
    /// # Errors
    ///
    /// Returns a stream-scoped protocol error without growing past the fixed residue bound.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<CanonicalEvent>, GatewayError> {
        if self.is_finished() && chunk.iter().any(|byte| !byte.is_ascii_whitespace()) {
            return Err(protocol_error());
        }
        self.append(chunk)?;
        while !self.is_finished() {
            let Some(frame) = self.take_frame() else {
                break;
            };
            self.consume_frame(&frame)?;
        }
        Ok(std::mem::take(&mut self.pending))
    }

    /// Completes the transport body without synthesizing success at EOF.
    ///
    /// # Errors
    ///
    /// Returns `StreamTruncated` unless a terminal frame was already accepted and no non-whitespace
    /// residue remains.
    pub fn finish(&mut self) -> Result<Vec<CanonicalEvent>, GatewayError> {
        let events = self.push(&[])?;
        if self.is_finished()
            && self.buffer[self.consumed..]
                .iter()
                .all(u8::is_ascii_whitespace)
        {
            Ok(events)
        } else {
            Err(truncated_error())
        }
    }

    fn append(&mut self, chunk: &[u8]) -> Result<(), GatewayError> {
        if self.consumed >= self.buffer.len().saturating_sub(self.consumed) {
            self.buffer.drain(..self.consumed);
            self.scanned = self.scanned.saturating_sub(self.consumed);
            self.consumed = 0;
        }
        let live = self.buffer.len().saturating_sub(self.consumed);
        if live.saturating_add(chunk.len()) > MAX_FRAME_BYTES {
            return Err(protocol_error());
        }
        self.buffer.extend_from_slice(chunk);
        Ok(())
    }

    fn take_frame(&mut self) -> Option<Vec<u8>> {
        let start = self.scanned.max(self.consumed);
        let found = (start..self.buffer.len()).find_map(|position| {
            let suffix = &self.buffer[position..];
            if suffix.starts_with(b"\n\n") {
                Some((position, 2))
            } else if suffix.starts_with(b"\r\n\r\n") {
                Some((position, 4))
            } else {
                None
            }
        });
        let Some((position, delimiter)) = found else {
            self.scanned = self.buffer.len().saturating_sub(3).max(self.consumed);
            return None;
        };
        let frame = self.buffer[self.consumed..position].to_vec();
        self.consumed = position + delimiter;
        self.scanned = self.consumed;
        Some(frame)
    }

    fn consume_frame(&mut self, frame: &[u8]) -> Result<(), GatewayError> {
        let frame = std::str::from_utf8(frame).map_err(|_| protocol_error())?;
        let mut event_name = None;
        let mut data = Vec::new();
        for line in frame.lines() {
            if let Some(value) = line.strip_prefix("event:") {
                if event_name.replace(value.trim()).is_some() {
                    return Err(protocol_error());
                }
            } else if let Some(value) = line.strip_prefix("data:") {
                data.push(value.trim());
            } else if !line.is_empty() && !line.starts_with(':') {
                return Err(protocol_error());
            }
        }
        if data.is_empty() {
            return self.note_progress_free();
        }
        let data = data.join("\n");
        reject_duplicate_json_names(&data).map_err(|_| protocol_error())?;
        let value: Value = serde_json::from_str(&data).map_err(|_| protocol_error())?;
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(protocol_error)?;
        if event_name.is_some_and(|name| name != kind) {
            return Err(protocol_error());
        }
        if kind == "response.in_progress" {
            return self.note_progress_free();
        }
        self.progress_free_frames = 0;
        self.progress_marks = self.progress_marks.saturating_add(1);
        match kind {
            "response.created" => self.start(&value),
            "response.output_item.added" => self.output_item_added(&value),
            "response.output_text.delta" => self.text_delta(&value),
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                self.reasoning_delta(&value)
            }
            "response.function_call_arguments.delta" => self.tool_delta(&value),
            "response.function_call_arguments.done" => self.tool_done(&value, false),
            "response.output_item.done" => self.output_item_done(&value),
            "response.completed" => self.end(&value, None),
            "response.incomplete" => {
                let reason = incomplete_reason(value.get("response"))?;
                self.end(&value, Some(reason))
            }
            "response.failed" => self.failed(),
            "response.content_part.added"
            | "response.content_part.done"
            | "response.output_text.done"
            | "response.reasoning_summary_part.added"
            | "response.reasoning_summary_part.done"
            | "response.reasoning_summary_text.done"
            | "response.reasoning_text.done" => Ok(()),
            _ => Err(protocol_error()),
        }
    }

    fn note_progress_free(&mut self) -> Result<(), GatewayError> {
        self.progress_free_frames = self.progress_free_frames.saturating_add(1);
        if self.progress_free_frames > MAX_PROGRESS_FREE_FRAMES {
            self.emit(CanonicalEvent::StreamError(StreamError {
                error: transient_error(),
            }))?;
            self.lifecycle = SseLifecycle::Finished;
        }
        Ok(())
    }

    fn start(&mut self, value: &Value) -> Result<(), GatewayError> {
        if !matches!(self.lifecycle, SseLifecycle::AwaitingStart) {
            return Err(protocol_error());
        }
        let response = object(value.get("response").ok_or_else(protocol_error)?)?;
        let id = ResponseId::try_new(identifier(response, "id")?).map_err(|_| protocol_error())?;
        self.lifecycle = SseLifecycle::Streaming(SseState {
            response_id: id.as_str().to_owned(),
            message_open: false,
            emitted_content: false,
            output_items: BTreeSet::new(),
            text_items: BTreeSet::new(),
            reasoning_items: BTreeSet::new(),
            tools: BTreeMap::new(),
            call_ids: BTreeSet::new(),
            retained_argument_bytes: 0,
        });
        self.emit(CanonicalEvent::ResponseStart(ResponseStart {
            response_id: id,
            extensions: RawExtensions::default(),
        }))?;
        if let Some(usage) =
            decode_usage(response.get("usage"))?.filter(|usage| usage.input_tokens.is_some())
        {
            self.emit(CanonicalEvent::UsageDelta(UsageDelta {
                usage: initial_usage(&usage),
                is_final: false,
                extensions: RawExtensions::default(),
            }))?;
        }
        Ok(())
    }

    fn output_item_added(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item = object(value.get("item").ok_or_else(protocol_error)?)?;
        let kind = required_string(item, "type")?;
        let item_id = identifier(item, "id")?.to_owned();
        let state = self.streaming_mut()?;
        if state.output_items.len() >= MAX_OUTPUT_ITEMS
            || !state.output_items.insert(item_id.clone())
        {
            return Err(protocol_error());
        }
        match kind {
            "message" if item.get("role").and_then(Value::as_str) == Some("assistant") => {
                state.text_items.insert(item_id);
                self.ensure_message()
            }
            "reasoning" => {
                if item
                    .get("encrypted_content")
                    .is_some_and(|value| !value.is_null())
                {
                    return Err(protocol_error());
                }
                state.reasoning_items.insert(item_id);
                self.ensure_message()
            }
            "function_call" => self.start_tool(item_id, item),
            _ => Err(protocol_error()),
        }
    }

    fn text_delta(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item_id = required_string_value(value, "item_id")?;
        let delta = string_value(value, "delta")?;
        if !self.streaming_mut()?.text_items.contains(&item_id) {
            return Err(protocol_error());
        }
        if !delta.is_empty() {
            self.streaming_mut()?.emitted_content = true;
            self.emit(CanonicalEvent::TextDelta(TextDelta {
                text: delta,
                extensions: RawExtensions::default(),
            }))?;
        }
        Ok(())
    }

    fn reasoning_delta(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item_id = required_string_value(value, "item_id")?;
        let delta = string_value(value, "delta")?;
        if !self.streaming_mut()?.reasoning_items.contains(&item_id) {
            return Err(protocol_error());
        }
        if !delta.is_empty() {
            self.streaming_mut()?.emitted_content = true;
            self.emit(CanonicalEvent::ReasoningDelta(ReasoningDelta {
                text: delta,
                extensions: RawExtensions::default(),
            }))?;
        }
        Ok(())
    }

    fn start_tool(
        &mut self,
        item_id: String,
        item: &Map<String, Value>,
    ) -> Result<(), GatewayError> {
        let call_id = identifier(item, "call_id")?.to_owned();
        let name = identifier(item, "name")?.to_owned();
        let state = self.streaming_mut()?;
        if state.tools.len() >= MAX_TOOL_CALLS || !state.call_ids.insert(call_id.clone()) {
            return Err(protocol_error());
        }
        state.tools.insert(
            item_id,
            OpenTool {
                call_id: call_id.clone(),
                assembled: String::new(),
                released: 0,
                ended: false,
            },
        );
        self.ensure_message()?;
        self.emit(CanonicalEvent::ToolCallStart(ToolCallStart {
            call_id,
            name,
            extensions: RawExtensions::default(),
        }))
    }

    fn tool_delta(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item_id = required_string_value(value, "item_id")?;
        let delta = value
            .get("delta")
            .and_then(Value::as_str)
            .ok_or_else(protocol_error)?;
        let state = self.streaming_mut()?;
        let retained = state
            .retained_argument_bytes
            .checked_add(delta.len())
            .ok_or_else(protocol_error)?;
        if retained > MAX_TOOL_ARGUMENT_BYTES {
            return Err(protocol_error());
        }
        let call = state
            .tools
            .get_mut(&item_id)
            .filter(|call| !call.ended)
            .ok_or_else(protocol_error)?;
        call.assembled.push_str(delta);
        let released = call.release_delta();
        let call_id = call.call_id.clone();
        state.retained_argument_bytes = retained;
        if let Some(delta) = released {
            self.emit(CanonicalEvent::ToolCallArgumentsDelta(
                ToolCallArgumentsDelta {
                    call_id,
                    delta,
                    extensions: RawExtensions::default(),
                },
            ))?;
        }
        Ok(())
    }

    fn tool_done(&mut self, value: &Value, authoritative: bool) -> Result<(), GatewayError> {
        let item_id = required_string_value(value, "item_id")?;
        let reported = value.get("arguments").and_then(Value::as_str);
        self.finish_tool(&item_id, reported, authoritative)
    }

    fn output_item_done(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item = object(value.get("item").ok_or_else(protocol_error)?)?;
        let item_id = identifier(item, "id")?;
        if !self.streaming_mut()?.output_items.contains(item_id) {
            return Err(protocol_error());
        }
        match required_string(item, "type")? {
            "function_call" => {
                self.finish_tool(item_id, item.get("arguments").and_then(Value::as_str), true)
            }
            "message"
                if self.streaming_mut()?.text_items.contains(item_id)
                    && completed_or_absent(item.get("status")) =>
            {
                Ok(())
            }
            "reasoning"
                if self.streaming_mut()?.reasoning_items.contains(item_id)
                    && completed_or_absent(item.get("status")) =>
            {
                Ok(())
            }
            _ => Err(protocol_error()),
        }
    }

    fn finish_tool(
        &mut self,
        item_id: &str,
        reported: Option<&str>,
        authoritative: bool,
    ) -> Result<(), GatewayError> {
        let state = self.streaming_mut()?;
        let call = state.tools.get_mut(item_id).ok_or_else(protocol_error)?;
        if call.ended {
            return Ok(());
        }
        if call.value_bounds().0 == call.value_bounds().1 {
            if let Some(reported) = reported.filter(|value| !value.trim().is_empty()) {
                let retained = state
                    .retained_argument_bytes
                    .checked_add(reported.len())
                    .ok_or_else(protocol_error)?;
                if retained > MAX_TOOL_ARGUMENT_BYTES {
                    return Err(protocol_error());
                }
                call.assembled.push_str(reported);
                state.retained_argument_bytes = retained;
            } else if !authoritative {
                return Ok(());
            }
        } else if let Some(reported) = reported {
            let (start, end) = call.value_bounds();
            if &call.assembled[start..end] != reported.trim() {
                return Err(protocol_error());
            }
        }
        let delta = call.release_delta();
        let arguments = call.arguments()?;
        let call_id = call.call_id.clone();
        call.ended = true;
        state.emitted_content = true;
        if let Some(delta) = delta {
            self.emit(CanonicalEvent::ToolCallArgumentsDelta(
                ToolCallArgumentsDelta {
                    call_id: call_id.clone(),
                    delta,
                    extensions: RawExtensions::default(),
                },
            ))?;
        }
        self.emit(CanonicalEvent::ToolCallEnd(ToolCallEnd {
            call_id,
            arguments,
            extensions: RawExtensions::default(),
        }))
    }

    fn end(&mut self, value: &Value, reported_reason: Option<&str>) -> Result<(), GatewayError> {
        let response = object(value.get("response").ok_or_else(protocol_error)?)?;
        let state = self.streaming_mut()?;
        let expected_status = if reported_reason.is_some() {
            "incomplete"
        } else {
            "completed"
        };
        if identifier(response, "id")? != state.response_id
            || response.get("status").and_then(Value::as_str) != Some(expected_status)
            || !state.emitted_content
            || state.tools.values().any(|tool| !tool.ended)
        {
            return Err(protocol_error());
        }
        let message_open = state.message_open;
        let has_tools = !state.call_ids.is_empty();
        if message_open {
            self.emit(CanonicalEvent::MessageEnd(MessageEnd::default()))?;
        }
        if let Some(usage) = decode_usage(response.get("usage"))? {
            self.emit(CanonicalEvent::UsageDelta(UsageDelta {
                usage,
                is_final: true,
                extensions: RawExtensions::default(),
            }))?;
        }
        self.emit(CanonicalEvent::ResponseEnd(ResponseEnd {
            stop_reason: Some(
                reported_reason
                    .unwrap_or(if has_tools { "tool_use" } else { "end_turn" })
                    .to_owned(),
            ),
            stop_sequence: None,
            extensions: RawExtensions::default(),
        }))?;
        self.lifecycle = SseLifecycle::Finished;
        Ok(())
    }

    fn failed(&mut self) -> Result<(), GatewayError> {
        if matches!(self.lifecycle, SseLifecycle::AwaitingStart) {
            return Err(protocol_error());
        }
        self.emit(CanonicalEvent::StreamError(StreamError {
            error: transient_error(),
        }))?;
        self.lifecycle = SseLifecycle::Finished;
        Ok(())
    }

    fn ensure_message(&mut self) -> Result<(), GatewayError> {
        if self.streaming_mut()?.message_open {
            return Ok(());
        }
        self.emit(CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
            extensions: RawExtensions::default(),
        }))?;
        self.streaming_mut()?.message_open = true;
        Ok(())
    }

    fn streaming_mut(&mut self) -> Result<&mut SseState, GatewayError> {
        match &mut self.lifecycle {
            SseLifecycle::Streaming(state) => Ok(state),
            SseLifecycle::AwaitingStart | SseLifecycle::Finished => Err(protocol_error()),
        }
    }

    fn emit(&mut self, event: CanonicalEvent) -> Result<(), GatewayError> {
        self.state.apply(&event).map_err(|_| protocol_error())?;
        self.pending.push(event);
        Ok(())
    }
}

impl OpenTool {
    fn value_bounds(&self) -> (usize, usize) {
        let start = self
            .assembled
            .len()
            .saturating_sub(self.assembled.trim_start_matches(JSON_WHITESPACE).len());
        let end = self.assembled.trim_end_matches(JSON_WHITESPACE).len();
        (start, end)
    }

    fn release_delta(&mut self) -> Option<String> {
        let (start, end) = self.value_bounds();
        let from = self.released.max(start);
        if end <= from {
            return None;
        }
        let delta = self.assembled[from..end].to_owned();
        self.released = end;
        Some(delta)
    }

    fn arguments(&self) -> Result<RawJson, GatewayError> {
        let (start, end) = self.value_bounds();
        let value = if start == end {
            "{}"
        } else {
            &self.assembled[start..end]
        };
        RawJson::from_json_string(value.to_owned()).map_err(|_| protocol_error())
    }
}

fn decode_usage(value: Option<&Value>) -> Result<Option<Usage>, GatewayError> {
    let Some(value) = value else { return Ok(None) };
    if value.is_null() {
        return Ok(None);
    }
    let value = object(value)?;
    require_only_keys(
        value,
        &[
            "input_tokens",
            "input_tokens_details",
            "output_tokens",
            "output_tokens_details",
            "total_tokens",
        ],
    )?;
    let input = optional_u64(value, "input_tokens")?;
    let output = optional_u64(value, "output_tokens")?;
    if let Some(total) = optional_u64(value, "total_tokens")?
        && input
            .zip(output)
            .and_then(|(input, output)| input.checked_add(output))
            != Some(total)
    {
        return Err(protocol_error());
    }
    let cached_tokens = decode_input_token_details(value)?;
    let reasoning_tokens = nested_optional_u64(value, "output_tokens_details", "reasoning_tokens")?;
    Ok(Some(Usage {
        input_tokens: input,
        output_tokens: output,
        cached_tokens,
        reasoning_tokens,
        ..Usage::default()
    }))
}

fn decode_input_token_details(value: &Map<String, Value>) -> Result<Option<u64>, GatewayError> {
    let Some(details) = value.get("input_tokens_details") else {
        return Ok(None);
    };
    if details.is_null() {
        return Ok(None);
    }
    let details = object(details)?;
    require_only_keys(details, &["cached_tokens", "cache_write_tokens"])?;
    if details
        .get("cache_write_tokens")
        .is_some_and(|value| value.as_u64() != Some(0))
    {
        return Err(protocol_error());
    }
    optional_u64(details, "cached_tokens")
}

fn validate_proven_response_metadata(root: &Map<String, Value>) -> Result<(), GatewayError> {
    if let Some(completed_at) = root.get("completed_at") {
        let completed_at = completed_at.as_u64().ok_or_else(protocol_error)?;
        let created_at = root
            .get("created_at")
            .and_then(Value::as_u64)
            .ok_or_else(protocol_error)?;
        if completed_at < created_at {
            return Err(protocol_error());
        }
    }
    for field in ["frequency_penalty", "presence_penalty"] {
        if root
            .get(field)
            .is_some_and(|value| value.as_f64() != Some(0.0))
        {
            return Err(protocol_error());
        }
    }
    if root.get("moderation").is_some_and(|value| !value.is_null()) {
        return Err(protocol_error());
    }
    if root
        .get("prompt_cache_retention")
        .is_some_and(|value| !matches!(value.as_str(), Some("in-memory" | "24h")))
    {
        return Err(protocol_error());
    }
    if let Some(tool_usage) = root.get("tool_usage") {
        validate_zero_tool_usage(object(tool_usage)?)?;
    }
    Ok(())
}

fn validate_zero_tool_usage(tool_usage: &Map<String, Value>) -> Result<(), GatewayError> {
    require_only_keys(tool_usage, &["image_gen", "web_search"])?;
    if tool_usage.len() != 2 {
        return Err(protocol_error());
    }
    let web_search = object(required(tool_usage, "web_search")?)?;
    require_only_keys(web_search, &["num_requests"])?;
    if web_search.len() != 1 || optional_u64(web_search, "num_requests")? != Some(0) {
        return Err(protocol_error());
    }
    let image_gen = object(required(tool_usage, "image_gen")?)?;
    require_only_keys(
        image_gen,
        &[
            "input_tokens",
            "input_tokens_details",
            "output_tokens",
            "output_tokens_details",
            "total_tokens",
        ],
    )?;
    if image_gen.len() != 5
        || ["input_tokens", "output_tokens", "total_tokens"]
            .iter()
            .any(|field| optional_u64(image_gen, field).ok() != Some(Some(0)))
    {
        return Err(protocol_error());
    }
    for field in ["input_tokens_details", "output_tokens_details"] {
        let details = object(required(image_gen, field)?)?;
        require_only_keys(details, &["image_tokens", "text_tokens"])?;
        if details.len() != 2
            || ["image_tokens", "text_tokens"]
                .iter()
                .any(|name| optional_u64(details, name).ok() != Some(Some(0)))
        {
            return Err(protocol_error());
        }
    }
    Ok(())
}

fn validate_proven_message_metadata(item: &Map<String, Value>) -> Result<(), GatewayError> {
    let metadata_present = item.contains_key("metadata")
        || item.contains_key("internal_chat_message_metadata_passthrough")
        || item.contains_key("phase");
    if !metadata_present {
        return Ok(());
    }
    if item.get("phase").and_then(Value::as_str) != Some("final_answer") {
        return Err(protocol_error());
    }
    let public = object(required(item, "metadata")?)?;
    let internal = object(required(
        item,
        "internal_chat_message_metadata_passthrough",
    )?)?;
    require_only_keys(public, &["turn_id"])?;
    require_only_keys(internal, &["turn_id"])?;
    let public_turn = identifier(public, "turn_id")?;
    let internal_turn = identifier(internal, "turn_id")?;
    if public.len() != 1 || internal.len() != 1 || public_turn != internal_turn {
        return Err(protocol_error());
    }
    Ok(())
}

fn initial_usage(usage: &Usage) -> Usage {
    Usage {
        input_tokens: usage.input_tokens,
        cached_tokens: usage.cached_tokens,
        ..Usage::default()
    }
}

fn nested_optional_u64(
    value: &Map<String, Value>,
    parent: &str,
    child: &str,
) -> Result<Option<u64>, GatewayError> {
    let Some(details) = value.get(parent) else {
        return Ok(None);
    };
    if details.is_null() {
        return Ok(None);
    }
    let details = object(details)?;
    require_only_keys(details, &[child])?;
    optional_u64(details, child)
}

fn optional_u64(value: &Map<String, Value>, field: &str) -> Result<Option<u64>, GatewayError> {
    value
        .get(field)
        .map(|value| value.as_u64().ok_or_else(protocol_error))
        .transpose()
}

fn stop_reason(
    root: &Map<String, Value>,
    status: &str,
    has_tools: bool,
) -> Result<&'static str, GatewayError> {
    match status {
        "completed" => Ok(if has_tools { "tool_use" } else { "end_turn" }),
        "incomplete" => incomplete_reason(Some(&Value::Object(root.clone()))),
        _ => Err(protocol_error()),
    }
}

fn incomplete_reason(response: Option<&Value>) -> Result<&'static str, GatewayError> {
    match response
        .and_then(|value| value.get("incomplete_details"))
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str)
    {
        Some("max_output_tokens") => Ok("max_tokens"),
        Some("content_filter") => Ok("refusal"),
        _ => Err(protocol_error()),
    }
}

fn completed_or_absent(status: Option<&Value>) -> bool {
    status.is_none_or(|status| status.as_str() == Some("completed"))
}

fn identifier<'a>(value: &'a Map<String, Value>, field: &str) -> Result<&'a str, GatewayError> {
    let value = required_string(value, field)?;
    if value.len() > MAX_IDENTIFIER_BYTES {
        Err(protocol_error())
    } else {
        Ok(value)
    }
}

fn required_string<'a>(
    value: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, GatewayError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(protocol_error)
}

fn required_string_value(value: &Value, field: &str) -> Result<String, GatewayError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(protocol_error)
}

fn string_value(value: &Value, field: &str) -> Result<String, GatewayError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(protocol_error)
}

fn object(value: &Value) -> Result<&Map<String, Value>, GatewayError> {
    value.as_object().ok_or_else(protocol_error)
}

fn required<'a>(value: &'a Map<String, Value>, field: &str) -> Result<&'a Value, GatewayError> {
    value.get(field).ok_or_else(protocol_error)
}

fn require_only_keys(value: &Map<String, Value>, allowed: &[&str]) -> Result<(), GatewayError> {
    if value.keys().all(|name| allowed.contains(&name.as_str())) {
        Ok(())
    } else {
        Err(protocol_error())
    }
}

const fn protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

const fn truncated_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::StreamTruncated, ErrorScope::Stream)
}

const fn transient_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider)
}

#[cfg(test)]
mod tests {
    use gateway_core::{CanonicalEvent, Usage};
    use proptest::prelude::*;

    use super::{MAX_FRAME_BYTES, OpenAiResponsesSseDecoder, decode_upstream_response};

    const JSON: &str =
        include_str!("../../../tests/fixtures/openai-responses/upstream-completed-response.json");
    const SSE: &str =
        include_str!("../../../tests/fixtures/openai-responses/upstream-completed-stream.sse");

    #[derive(Debug, Eq, PartialEq)]
    struct Digest {
        text: String,
        reasoning: String,
        tools: Vec<(String, String, String)>,
        usage: Option<Usage>,
        stop_reason: Option<String>,
    }

    fn digest(events: &[CanonicalEvent]) -> Digest {
        let mut text = String::new();
        let mut reasoning = String::new();
        let mut names = std::collections::BTreeMap::new();
        let mut tools = Vec::new();
        let mut usage = None;
        let mut stop_reason = None;
        for event in events {
            match event {
                CanonicalEvent::TextDelta(delta) => text.push_str(&delta.text),
                CanonicalEvent::ReasoningDelta(delta) => reasoning.push_str(&delta.text),
                CanonicalEvent::ToolCallStart(start) => {
                    names.insert(start.call_id.clone(), start.name.clone());
                }
                CanonicalEvent::ToolCallEnd(end) => tools.push((
                    end.call_id.clone(),
                    names.get(&end.call_id).cloned().unwrap_or_default(),
                    end.arguments.get().to_owned(),
                )),
                CanonicalEvent::UsageDelta(delta) if delta.is_final => {
                    usage = Some(delta.usage.clone());
                }
                CanonicalEvent::ResponseEnd(end) => {
                    stop_reason.clone_from(&end.stop_reason);
                }
                _ => {}
            }
        }
        Digest {
            text,
            reasoning,
            tools,
            usage,
            stop_reason,
        }
    }

    fn decode_sse(chunks: &[usize]) -> Result<Vec<CanonicalEvent>, gateway_core::GatewayError> {
        let bytes = SSE.as_bytes();
        let mut offset = 0;
        let mut events = Vec::new();
        let mut decoder = OpenAiResponsesSseDecoder::new();
        for size in chunks {
            if offset >= bytes.len() {
                break;
            }
            let end = offset.saturating_add(*size).min(bytes.len());
            events.extend(decoder.push(&bytes[offset..end])?);
            offset = end;
        }
        if offset < bytes.len() {
            events.extend(decoder.push(&bytes[offset..])?);
        }
        events.extend(decoder.push(b"\n")?);
        events.extend(decoder.finish()?);
        Ok(events)
    }

    #[test]
    fn buffered_and_streamed_responses_have_the_same_final_semantic_projection()
    -> Result<(), Box<dyn std::error::Error>> {
        let buffered = decode_upstream_response(JSON)?;
        let streamed = decode_sse(&[1])?;
        assert_eq!(digest(&buffered), digest(&streamed));
        Ok(())
    }

    #[test]
    fn buffered_response_accepts_only_proven_zero_and_redundant_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let base = serde_json::json!({
            "id": "response-id",
            "object": "response",
            "status": "completed",
            "created_at": 10,
            "completed_at": 11,
            "error": null,
            "frequency_penalty": 0.0,
            "presence_penalty": 0.0,
            "moderation": null,
            "prompt_cache_retention": "in-memory",
            "tool_usage": {
                "image_gen": {
                    "input_tokens": 0,
                    "input_tokens_details": {"image_tokens": 0, "text_tokens": 0},
                    "output_tokens": 0,
                    "output_tokens_details": {"image_tokens": 0, "text_tokens": 0},
                    "total_tokens": 0
                },
                "web_search": {"num_requests": 0}
            },
            "output": [{
                "id": "message-id",
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "phase": "final_answer",
                "metadata": {"turn_id": "turn-id"},
                "internal_chat_message_metadata_passthrough": {"turn_id": "turn-id"},
                "content": [{
                    "type": "output_text",
                    "text": "ok",
                    "annotations": [],
                    "logprobs": []
                }]
            }],
            "usage": {
                "input_tokens": 1,
                "input_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 0},
                "output_tokens": 1,
                "output_tokens_details": {"reasoning_tokens": 0},
                "total_tokens": 2
            }
        });
        assert!(decode_upstream_response(&base.to_string()).is_ok());

        for (pointer, replacement) in [
            ("/frequency_penalty", serde_json::json!(0.5)),
            ("/presence_penalty", serde_json::json!(-0.5)),
            ("/moderation", serde_json::json!({})),
            ("/prompt_cache_retention", serde_json::json!("unknown")),
            ("/tool_usage/web_search/num_requests", serde_json::json!(1)),
            ("/tool_usage/image_gen/input_tokens", serde_json::json!(1)),
            ("/output/0/phase", serde_json::json!("analysis")),
            (
                "/output/0/internal_chat_message_metadata_passthrough/turn_id",
                serde_json::json!("different"),
            ),
            (
                "/usage/input_tokens_details/cache_write_tokens",
                serde_json::json!(1),
            ),
        ] {
            let mut changed = base.clone();
            *changed
                .pointer_mut(pointer)
                .ok_or_else(|| std::io::Error::other("test pointer missing"))? = replacement;
            assert!(decode_upstream_response(&changed.to_string()).is_err());
        }

        let mut reversed_time = base;
        *reversed_time
            .pointer_mut("/completed_at")
            .ok_or_else(|| std::io::Error::other("test pointer missing"))? = serde_json::json!(9);
        assert!(decode_upstream_response(&reversed_time.to_string()).is_err());
        Ok(())
    }

    #[test]
    fn terminal_decoder_rejects_late_semantic_bytes_and_redacts_debug()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut decoder = OpenAiResponsesSseDecoder::new();
        let _ = decoder.push(SSE.as_bytes())?;
        let _ = decoder.push(b"\n")?;
        assert!(decoder.is_finished());
        assert!(decoder.push(b"\n\t").is_ok());
        assert!(decoder.push(b"data: {}\n\n").is_err());

        let diagnostic = format!("{decoder:?}");
        for private_value in ["resp_fixture", "fixture answer", "inspect first"] {
            assert!(!diagnostic.contains(private_value));
        }
        Ok(())
    }

    #[test]
    fn missing_terminal_unknown_events_and_oversized_residue_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut missing = OpenAiResponsesSseDecoder::new();
        let prefix = SSE
            .split("event: response.completed")
            .next()
            .ok_or_else(|| std::io::Error::other("fixture lacks terminal marker"))?;
        let _ = missing.push(prefix.as_bytes())?;
        assert!(missing.finish().is_err());

        let mut unknown = OpenAiResponsesSseDecoder::new();
        assert!(
            unknown
                .push(b"data: {\"type\":\"response.future\"}\n\n")
                .is_err()
        );

        let mismatched_terminal = SSE.replace(
            "\"id\":\"resp_fixture\",\"status\":\"completed\"",
            "\"id\":\"different_response\",\"status\":\"completed\"",
        );
        let mut mismatched = OpenAiResponsesSseDecoder::new();
        let _ = mismatched.push(mismatched_terminal.as_bytes())?;
        assert!(mismatched.push(b"\n").is_err());

        let mut oversized = OpenAiResponsesSseDecoder::new();
        let _ = oversized.push(&vec![b'x'; MAX_FRAME_BYTES])?;
        assert!(oversized.push(b"y").is_err());
        Ok(())
    }

    proptest! {
        #[test]
        fn arbitrary_transport_chunk_sizes_keep_the_final_projection(
            chunks in prop::collection::vec(1_usize..128, 0..128)
        ) {
            let buffered = decode_upstream_response(JSON)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let streamed = decode_sse(&chunks)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            prop_assert_eq!(digest(&buffered), digest(&streamed));
        }
    }
}
