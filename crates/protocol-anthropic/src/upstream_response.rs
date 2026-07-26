//! `Anthropic` Messages upstream response to Canonical Event decoding.
//!
//! The buffered JSON message and the streamed Server-Sent Event body are projected onto the same
//! Canonical sequence, mirroring in reverse the frame vocabulary [`crate::AnthropicMessagesSseEncoder`]
//! emits. Frame reassembly and Canonical projection stay free of any transport type so the same
//! state machine can be driven from arbitrary chunk boundaries: only frame contents, never network
//! segmentation, may change the emitted Canonical sequence.
//!
//! Two representation limits are deliberate and fail closed rather than degrade: a thinking
//! block's `signature_delta` has no Canonical representation, because a Canonical event extension
//! would make the sibling `Anthropic` encoder reject the stream, and an unknown content block type
//! or completion reason is refused rather than approximated. Unreported `usage` sub-fields are the
//! single documented exception: an upstream's `service_tier` and its `cache_creation` breakdown
//! carry no counter the four Canonical `Anthropic` counters do not already hold, so they are read
//! past rather than rejected.

use std::{
    collections::{BTreeSet, VecDeque},
    fmt,
};

use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalResponse, ErrorScope, GatewayError,
    GatewayErrorCode, MessageEnd, MessageRole, MessageStart, RawExtensions, RawJson,
    ReasoningDelta, ResponseEnd, ResponseId, ResponseStart, StreamError, TextDelta,
    ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart, Usage, UsageDelta,
};
use serde_json::{Map, Value};

use crate::{
    json::{raw_json, stream_protocol_error},
    response::merge_usage,
};

/// The largest undecoded SSE residue this decoder retains between two frames.
const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
/// The largest assembled Tool argument value one content block may retain.
///
/// `Anthropic` keeps exactly one content block open at a time, so this per-block bound is also the
/// per-response bound: a closed block's assembled value is dropped when its `content_block_stop`
/// is consumed.
const MAX_TOOL_ARGUMENT_BYTES: usize = MAX_FRAME_BYTES;
/// The largest number of Tool calls one streamed response may declare.
const MAX_TOOL_CALLS: usize = 32;
/// The longest Tool call identifier or Tool name this decoder retains.
const MAX_IDENTIFIER_BYTES: usize = 256;
/// The longest run of consecutive progress-free frames this decoder tolerates.
const MAX_PROGRESS_FREE_FRAMES: usize = 4096;
/// The four JSON insignificant whitespace characters used to frame assembled Tool arguments.
const JSON_WHITESPACE: [char; 4] = [' ', '\t', '\n', '\r'];
/// Every `Anthropic` completion reason this codec carries into `ResponseEnd::stop_reason`.
///
/// `ResponseEnd::stop_reason` is an open string that the sibling encoder writes to the wire
/// unchanged, so the mapping is identity over this closed set; an unlisted reason fails closed
/// rather than being approximated by a neighbouring label.
const ANTHROPIC_STOP_REASONS: &[&str] = &[
    "end_turn",
    "max_tokens",
    "model_context_window_exceeded",
    "pause_turn",
    "refusal",
    "stop_sequence",
    "tool_use",
];

/// Decodes one complete non-streaming `Anthropic` Messages JSON response.
///
/// The returned sequence is already validated by the Canonical response state machine, so it can
/// be handed to a bounded delivery boundary or re-encoded without a second check.
///
/// # Errors
///
/// Returns `UpstreamProtocolError/Stream` for malformed JSON, an unrepresentable content block, an
/// unknown completion reason, or a `usage` object without both exact token counts.
pub fn decode_upstream_response(input: &str) -> Result<Vec<CanonicalEvent>, GatewayError> {
    let value: Value = serde_json::from_str(input).map_err(|_| stream_protocol_error())?;
    let message = value.as_object().ok_or_else(stream_protocol_error)?;
    require_assistant_message(message)?;

    let usage = decode_usage(message.get("usage"))?;
    if usage.input_tokens.is_none() || usage.output_tokens.is_none() {
        return Err(stream_protocol_error());
    }
    let stop_reason = decode_stop_reason(message)?;
    let stop_sequence = decode_stop_sequence(message)?;

    let mut events = vec![CanonicalEvent::ResponseStart(ResponseStart {
        response_id: decode_response_id(message)?,
        extensions: RawExtensions::default(),
    })];
    // The Messages representation reports input usage before the message opens, exactly as the
    // streamed form does, so a later re-encode never has to invent one.
    events.push(CanonicalEvent::UsageDelta(UsageDelta {
        usage: input_usage_snapshot(&usage),
        is_final: false,
        extensions: RawExtensions::default(),
    }));
    events.push(CanonicalEvent::MessageStart(MessageStart {
        role: MessageRole("assistant".to_owned()),
        extensions: RawExtensions::default(),
    }));

    let content = message
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(stream_protocol_error)?;
    let mut call_ids = BTreeSet::new();
    let mut emitted_content = false;
    for block in content {
        decode_completed_block(block, &mut events, &mut call_ids, &mut emitted_content)?;
    }
    if !emitted_content {
        return Err(stream_protocol_error());
    }

    events.push(CanonicalEvent::MessageEnd(MessageEnd::default()));
    events.push(CanonicalEvent::UsageDelta(UsageDelta {
        usage,
        is_final: true,
        extensions: RawExtensions::default(),
    }));
    events.push(CanonicalEvent::ResponseEnd(ResponseEnd {
        stop_reason: Some(stop_reason),
        stop_sequence,
        extensions: RawExtensions::default(),
    }));

    CanonicalResponse::try_new(events)
        .map(CanonicalResponse::into_events)
        .map_err(|_| stream_protocol_error())
}

fn decode_completed_block(
    block: &Value,
    events: &mut Vec<CanonicalEvent>,
    call_ids: &mut BTreeSet<String>,
    emitted_content: &mut bool,
) -> Result<(), GatewayError> {
    let block = block.as_object().ok_or_else(stream_protocol_error)?;
    match block.get("type").and_then(Value::as_str) {
        Some("text") => {
            let text = required_str(block, "text")?;
            if text.is_empty() {
                return Ok(());
            }
            events.push(CanonicalEvent::TextDelta(TextDelta {
                text: text.to_owned(),
                extensions: RawExtensions::default(),
            }));
            *emitted_content = true;
            Ok(())
        }
        Some("thinking") => {
            let thinking = required_str(block, "thinking")?;
            if thinking.is_empty() {
                return Ok(());
            }
            events.push(CanonicalEvent::ReasoningDelta(ReasoningDelta {
                text: thinking.to_owned(),
                extensions: RawExtensions::default(),
            }));
            *emitted_content = true;
            Ok(())
        }
        Some("tool_use") => {
            let call_id = required_str(block, "id")?.to_owned();
            let name = required_str(block, "name")?.to_owned();
            let input = block.get("input").ok_or_else(stream_protocol_error)?;
            if call_id.is_empty()
                || name.is_empty()
                || call_id.len() > MAX_IDENTIFIER_BYTES
                || name.len() > MAX_IDENTIFIER_BYTES
                || call_ids.len() >= MAX_TOOL_CALLS
                || !input.is_object()
                || !call_ids.insert(call_id.clone())
            {
                return Err(stream_protocol_error());
            }
            let arguments = raw_json(input).map_err(|_| stream_protocol_error())?;
            events.push(CanonicalEvent::ToolCallStart(ToolCallStart {
                call_id: call_id.clone(),
                name,
                extensions: RawExtensions::default(),
            }));
            events.push(CanonicalEvent::ToolCallArgumentsDelta(
                ToolCallArgumentsDelta {
                    call_id: call_id.clone(),
                    delta: arguments.get().to_owned(),
                    extensions: RawExtensions::default(),
                },
            ));
            events.push(CanonicalEvent::ToolCallEnd(ToolCallEnd {
                call_id,
                arguments,
                extensions: RawExtensions::default(),
            }));
            *emitted_content = true;
            Ok(())
        }
        _ => Err(stream_protocol_error()),
    }
}

/// Transport-free `Anthropic` Messages SSE decoder for one streamed upstream response.
///
/// Every event is validated through an owned Canonical state machine before it is queued, so no
/// frame ordering this decoder fails to anticipate can produce an illegal Canonical sequence.
#[derive(Default)]
pub struct AnthropicMessagesSseDecoder {
    buffer: Vec<u8>,
    /// Bytes of `buffer` before this offset belong to frames already extracted by `take_frame`.
    consumed: usize,
    /// Scan resume point: no frame delimiter starts inside `buffer[self.consumed..self.scanned]`.
    scanned: usize,
    pending: VecDeque<CanonicalEvent>,
    lifecycle: SseLifecycle,
    state: CanonicalEventState,
    /// Consecutive frames that proved only socket liveness, reset by any progress frame.
    progress_free_frames: usize,
    /// Monotone count of consumed frames that proved generation is advancing.
    progress_marks: u64,
}

impl fmt::Debug for AnthropicMessagesSseDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnthropicMessagesSseDecoder")
            .field(
                "buffered_bytes",
                &self.buffer.len().saturating_sub(self.consumed),
            )
            .field("pending_events", &self.pending.len())
            .field("state", &self.state)
            .field("progress_marks", &self.progress_marks)
            .finish_non_exhaustive()
    }
}

/// The bounded lifecycle of one streamed `Anthropic` Messages body.
#[derive(Default)]
enum SseLifecycle {
    /// No `message_start` frame has been accepted yet.
    #[default]
    AwaitingMessageStart,
    /// `ResponseStart` was emitted; content blocks may open, stream, and close.
    ///
    /// The retained block state is boxed so one open response costs the lifecycle no more than a
    /// pointer, keeping the decoder's own footprint independent of the stream it is decoding.
    Streaming(Box<StreamingState>),
    /// A terminal `ResponseEnd` or `StreamError` is already queued.
    Finished,
}

/// Block and usage state retained between the frames of one open streamed response.
#[derive(Default)]
struct StreamingState {
    /// Input-side usage reported at `message_start`; the terminal frame supplies the output count.
    usage: Usage,
    /// `Anthropic` keeps exactly one content block open at a time.
    active: Option<ActiveBlock>,
    /// Count of started blocks, which is also the next legal `index`.
    block_count: usize,
    tool_call_ids: BTreeSet<String>,
    emitted_content: bool,
    message_ended: bool,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
}

struct ActiveBlock {
    index: usize,
    kind: ActiveBlockKind,
}

enum ActiveBlockKind {
    Text,
    Thinking,
    Tool(ToolArguments),
}

/// One streamed Tool call's assembled arguments and the fragments already delivered.
struct ToolArguments {
    call_id: String,
    assembled: String,
    released: usize,
}

impl ToolArguments {
    /// Returns every assembled argument byte that the JSON value already frames.
    ///
    /// Whitespace outside the value is held back: `RawJson` retains only the value itself, so
    /// releasing padding would desynchronize the delivered fragments from the completed arguments
    /// the sibling `Anthropic` encoder compares them against.
    fn release(&mut self) -> Option<String> {
        let (start, end) = self.value_bounds();
        let from = self.released.max(start);
        if end <= from {
            return None;
        }
        let delta = self.assembled[from..end].to_owned();
        self.released = end;
        Some(delta)
    }

    /// Returns the byte range of the assembled JSON value without its surrounding whitespace.
    fn value_bounds(&self) -> (usize, usize) {
        let start = self
            .assembled
            .len()
            .saturating_sub(self.assembled.trim_start_matches(JSON_WHITESPACE).len());
        let end = self.assembled.trim_end_matches(JSON_WHITESPACE).len();
        (start, end)
    }

    /// Returns the complete assembled arguments, normalizing an absent value to `{}`.
    ///
    /// A Tool without required fields may stream no `input_json_delta` at all. Normalizing that to
    /// one empty JSON object keeps the Tool call representable instead of failing an otherwise
    /// complete stream, and matches what the sibling encoder accepts.
    fn completed(&self) -> Result<RawJson, GatewayError> {
        let (start, end) = self.value_bounds();
        let arguments = if end <= start {
            "{}".to_owned()
        } else {
            self.assembled[start..end].to_owned()
        };
        let retained =
            RawJson::from_json_string(arguments.clone()).map_err(|_| stream_protocol_error())?;
        if retained.get() != arguments
            || !serde_json::from_str::<Value>(&arguments).is_ok_and(|value| value.is_object())
        {
            return Err(stream_protocol_error());
        }
        Ok(retained)
    }
}

impl AnthropicMessagesSseDecoder {
    /// Creates a fresh decoder for one streamed upstream response.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one bounded transport chunk without interpreting it.
    ///
    /// The frame bound applies to the undecoded residue only. Decoded bytes are compacted away
    /// once they outweigh that residue, so the bytes ever moved stay linear in the bytes streamed
    /// and the buffer itself never holds more than twice [`MAX_FRAME_BYTES`].
    ///
    /// # Errors
    ///
    /// Returns `UpstreamProtocolError/Stream` when the undecoded residue would exceed the bound.
    pub fn push_chunk(&mut self, chunk: &[u8]) -> Result<(), GatewayError> {
        if self.consumed >= self.buffer.len().saturating_sub(self.consumed) {
            self.buffer.drain(..self.consumed);
            self.scanned = self.scanned.saturating_sub(self.consumed);
            self.consumed = 0;
        }
        let live = self.buffer.len().saturating_sub(self.consumed);
        if live.saturating_add(chunk.len()) > MAX_FRAME_BYTES {
            return Err(stream_protocol_error());
        }
        self.buffer.extend_from_slice(chunk);
        Ok(())
    }

    /// Decodes buffered frames until one event is queued or no complete frame remains.
    ///
    /// # Errors
    ///
    /// Returns a safe stream protocol error for a malformed, unrepresentable, or out-of-order
    /// frame, or the mapped upstream error for an `error` frame that arrives before any Canonical
    /// event was emitted.
    pub fn drain_buffered_frames(&mut self) -> Result<(), GatewayError> {
        while self.pending.is_empty() && !self.is_finished() {
            let Some(frame) = self.take_frame() else {
                return Ok(());
            };
            self.consume_frame(&frame)?;
        }
        Ok(())
    }

    /// Removes the next decoded Canonical event, if one is queued.
    #[must_use]
    pub fn take_event(&mut self) -> Option<CanonicalEvent> {
        self.pending.pop_front()
    }

    /// Returns whether a terminal Canonical event has already been queued.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        matches!(self.lifecycle, SseLifecycle::Finished)
    }

    /// Returns the monotone count of consumed frames that proved generation is advancing.
    ///
    /// A transport shell compares successive values to reset its wall-clock progress deadline.
    /// Keeping the counter here keeps the decoder clock-free: every frame's classification is
    /// decided by its content alone, never by transport segmentation or elapsed time.
    #[must_use]
    pub const fn progress_marks(&self) -> u64 {
        self.progress_marks
    }

    /// Verifies that the upstream body ended only after a terminal Canonical event.
    ///
    /// # Errors
    ///
    /// Returns `StreamTruncated/Stream` when the body ends before `ResponseEnd` or `StreamError`.
    pub fn finish(&self) -> Result<(), GatewayError> {
        self.state.finish()
    }

    /// Extracts the next complete SSE frame, resuming the delimiter scan where it last stopped.
    ///
    /// `scanned` marks the delimiter-free prefix of the undecoded region, so every buffered byte
    /// is examined once no matter how many chunks or frames arrive. When no delimiter is found,
    /// the resume point holds back the last three bytes: a delimiter is at most four bytes, so one
    /// completed by a later chunk can begin no earlier than three bytes before the current end.
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
        let Some((position, delimiter_length)) = found else {
            self.scanned = self.buffer.len().saturating_sub(3).max(self.consumed);
            return None;
        };
        let frame = self.buffer[self.consumed..position].to_vec();
        self.consumed = position + delimiter_length;
        self.scanned = self.consumed;
        Some(frame)
    }

    /// Records one consumed frame that proves the upstream is still generating.
    fn note_progress_frame(&mut self) {
        self.progress_free_frames = 0;
        self.progress_marks = self.progress_marks.saturating_add(1);
    }

    /// Records one keepalive-class frame that proves only that the socket is alive.
    ///
    /// A run longer than [`MAX_PROGRESS_FREE_FRAMES`] is a wedged upstream, not a thinking model.
    /// The run advances per decoded frame, never per transport read, which keeps the bound
    /// independent of chunk boundaries.
    fn note_progress_free_frame(&mut self) -> Result<(), GatewayError> {
        self.progress_free_frames = self.progress_free_frames.saturating_add(1);
        if self.progress_free_frames <= MAX_PROGRESS_FREE_FRAMES {
            return Ok(());
        }
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        if matches!(lifecycle, SseLifecycle::AwaitingMessageStart) {
            return Err(provider_transient_error());
        }
        push_event(
            state,
            pending,
            CanonicalEvent::StreamError(StreamError {
                error: provider_transient_error(),
            }),
        )?;
        *lifecycle = SseLifecycle::Finished;
        Ok(())
    }

    fn consume_frame(&mut self, frame: &[u8]) -> Result<(), GatewayError> {
        let frame = std::str::from_utf8(frame).map_err(|_| stream_protocol_error())?;
        let data = frame
            .lines()
            .filter_map(|line| line.strip_prefix("data:").map(str::trim))
            .collect::<Vec<_>>()
            .join("\n");
        // SSE comments and keep-alive frames carry no event payload. They must not alter the
        // Canonical lifecycle, but each one spends one unit of the bounded progress-free run.
        if data.is_empty() {
            return self.note_progress_free_frame();
        }
        let value: Value = serde_json::from_str(&data).map_err(|_| stream_protocol_error())?;
        let payload = value.as_object().ok_or_else(stream_protocol_error)?;
        let kind = payload
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(stream_protocol_error)?;
        // `ping` is the one payload-bearing frame that proves only relay liveness: a wedged
        // upstream can repeat it forever.
        if kind == "ping" {
            return self.note_progress_free_frame();
        }

        let queued_before = self.state_queue_len();
        let outcome = match kind {
            "message_start" => self.consume_message_start(payload),
            "content_block_start" => self.consume_content_block_start(payload),
            "content_block_delta" => self.consume_content_block_delta(payload),
            "content_block_stop" => self.consume_content_block_stop(payload),
            "message_delta" => self.consume_message_delta(payload),
            "message_stop" => self.consume_message_stop(),
            "error" => self.consume_error(payload),
            // Anthropic reserves the right to add SSE event types and requires clients to
            // tolerate unknown ones. This dispatch runs past the unretryable boundary, so an
            // unrecognised frame is consumed rather than allowed to truncate a healthy answer;
            // it proves no generation, so it spends the progress-free budget.
            _ => return self.note_progress_free_frame(),
        };
        outcome?;
        // Progress is classified by outcome, never by frame type: an empty text or thinking delta
        // and an argument fragment that releases nothing all queue no Canonical event, so they
        // must not reset the liveness budgets a wedged upstream would otherwise evade forever.
        if self.state_queue_len() > queued_before {
            self.note_progress_frame();
            Ok(())
        } else {
            self.note_progress_free_frame()
        }
    }

    /// Returns how many Canonical events the decoder has queued for the transport shell.
    fn state_queue_len(&self) -> usize {
        self.pending.len()
    }

    fn consume_message_start(&mut self, frame: &Map<String, Value>) -> Result<(), GatewayError> {
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        if !matches!(lifecycle, SseLifecycle::AwaitingMessageStart) {
            return Err(stream_protocol_error());
        }
        let message = frame
            .get("message")
            .and_then(Value::as_object)
            .ok_or_else(stream_protocol_error)?;
        require_assistant_message(message)?;
        let response_id = decode_response_id(message)?;
        // A `message_start` output count is a placeholder that the terminal frame supersedes, so
        // only the exact input-side counts enter the accumulator: no partial count may survive.
        let usage = input_usage_snapshot(&decode_usage(message.get("usage"))?);

        push_event(
            state,
            pending,
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id,
                extensions: RawExtensions::default(),
            }),
        )?;
        if usage.input_tokens.is_some() {
            push_event(
                state,
                pending,
                CanonicalEvent::UsageDelta(UsageDelta {
                    usage: usage.clone(),
                    is_final: false,
                    extensions: RawExtensions::default(),
                }),
            )?;
        }
        push_event(
            state,
            pending,
            CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }),
        )?;
        *lifecycle = SseLifecycle::Streaming(Box::new(StreamingState {
            usage,
            ..StreamingState::default()
        }));
        Ok(())
    }

    fn consume_content_block_start(
        &mut self,
        frame: &Map<String, Value>,
    ) -> Result<(), GatewayError> {
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        let streaming = streaming_state(lifecycle)?;
        let index = frame_index(frame)?;
        if streaming.message_ended || streaming.active.is_some() || index != streaming.block_count {
            return Err(stream_protocol_error());
        }
        let block = frame
            .get("content_block")
            .and_then(Value::as_object)
            .ok_or_else(stream_protocol_error)?;
        let kind = match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                require_empty_initial_value(block, "text")?;
                ActiveBlockKind::Text
            }
            Some("thinking") => {
                require_empty_initial_value(block, "thinking")?;
                ActiveBlockKind::Thinking
            }
            Some("tool_use") => {
                let call_id = required_str(block, "id")?.to_owned();
                let name = required_str(block, "name")?.to_owned();
                // Canonical delivers Tool arguments through fragments only, so a start frame that
                // already carries a value cannot be represented without dropping it.
                let empty_input = match block.get("input") {
                    None => true,
                    Some(Value::Object(input)) => input.is_empty(),
                    Some(_) => false,
                };
                if call_id.is_empty()
                    || name.is_empty()
                    || call_id.len() > MAX_IDENTIFIER_BYTES
                    || name.len() > MAX_IDENTIFIER_BYTES
                    || streaming.tool_call_ids.len() >= MAX_TOOL_CALLS
                    || !empty_input
                    || !streaming.tool_call_ids.insert(call_id.clone())
                {
                    return Err(stream_protocol_error());
                }
                push_event(
                    state,
                    pending,
                    CanonicalEvent::ToolCallStart(ToolCallStart {
                        call_id: call_id.clone(),
                        name,
                        extensions: RawExtensions::default(),
                    }),
                )?;
                streaming.emitted_content = true;
                ActiveBlockKind::Tool(ToolArguments {
                    call_id,
                    assembled: String::new(),
                    released: 0,
                })
            }
            _ => return Err(stream_protocol_error()),
        };
        streaming.active = Some(ActiveBlock { index, kind });
        streaming.block_count = streaming
            .block_count
            .checked_add(1)
            .ok_or_else(stream_protocol_error)?;
        Ok(())
    }

    fn consume_content_block_delta(
        &mut self,
        frame: &Map<String, Value>,
    ) -> Result<(), GatewayError> {
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        let streaming = streaming_state(lifecycle)?;
        let index = frame_index(frame)?;
        let delta = frame
            .get("delta")
            .and_then(Value::as_object)
            .ok_or_else(stream_protocol_error)?;
        let active = streaming
            .active
            .as_mut()
            .ok_or_else(stream_protocol_error)?;
        if active.index != index {
            return Err(stream_protocol_error());
        }
        let emitted = match (&mut active.kind, delta.get("type").and_then(Value::as_str)) {
            (ActiveBlockKind::Text, Some("text_delta")) => {
                let text = required_str(delta, "text")?;
                if text.is_empty() {
                    return Ok(());
                }
                push_event(
                    state,
                    pending,
                    CanonicalEvent::TextDelta(TextDelta {
                        text: text.to_owned(),
                        extensions: RawExtensions::default(),
                    }),
                )?;
                true
            }
            (ActiveBlockKind::Thinking, Some("thinking_delta")) => {
                let thinking = required_str(delta, "thinking")?;
                if thinking.is_empty() {
                    return Ok(());
                }
                push_event(
                    state,
                    pending,
                    CanonicalEvent::ReasoningDelta(ReasoningDelta {
                        text: thinking.to_owned(),
                        extensions: RawExtensions::default(),
                    }),
                )?;
                true
            }
            (ActiveBlockKind::Tool(arguments), Some("input_json_delta")) => {
                let partial = required_str(delta, "partial_json")?;
                let retained = arguments
                    .assembled
                    .len()
                    .checked_add(partial.len())
                    .ok_or_else(stream_protocol_error)?;
                if retained > MAX_TOOL_ARGUMENT_BYTES {
                    return Err(stream_protocol_error());
                }
                arguments.assembled.push_str(partial);
                let call_id = arguments.call_id.clone();
                match arguments.release() {
                    None => false,
                    Some(delta) => {
                        push_event(
                            state,
                            pending,
                            CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
                                call_id,
                                delta,
                                extensions: RawExtensions::default(),
                            }),
                        )?;
                        true
                    }
                }
            }
            // A `signature_delta` closes every extended-thinking block and `citations_delta`
            // accompanies citation-enabled text, so both arrive on healthy streams. Neither has a
            // Canonical representation, but this dispatch runs past the unretryable boundary:
            // rejecting them would truncate an answer the client is already receiving. They are
            // consumed without emitting, exactly as the buffered path already treats a thinking
            // signature, and they spend the progress-free budget because they prove no generation.
            (_, Some("signature_delta" | "citations_delta")) => false,
            // Any other delta shape would change what the client receives if it were dropped, so
            // it still fails closed.
            _ => return Err(stream_protocol_error()),
        };
        if emitted {
            streaming.emitted_content = true;
        }
        Ok(())
    }

    fn consume_content_block_stop(
        &mut self,
        frame: &Map<String, Value>,
    ) -> Result<(), GatewayError> {
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        let streaming = streaming_state(lifecycle)?;
        let index = frame_index(frame)?;
        if streaming
            .active
            .as_ref()
            .is_none_or(|active| active.index != index)
        {
            return Err(stream_protocol_error());
        }
        let Some(active) = streaming.active.take() else {
            return Err(stream_protocol_error());
        };
        match active.kind {
            ActiveBlockKind::Text | ActiveBlockKind::Thinking => Ok(()),
            ActiveBlockKind::Tool(arguments) => push_event(
                state,
                pending,
                CanonicalEvent::ToolCallEnd(ToolCallEnd {
                    call_id: arguments.call_id.clone(),
                    arguments: arguments.completed()?,
                    extensions: RawExtensions::default(),
                }),
            ),
        }
    }

    /// Consumes the one terminal `message_delta`, which ends the message and repays output usage.
    ///
    /// The exact output count must be reported by this frame itself, never inherited from the
    /// `message_start` placeholder, so a stream that never reports one fails closed instead of
    /// publishing a fabricated zero.
    fn consume_message_delta(&mut self, frame: &Map<String, Value>) -> Result<(), GatewayError> {
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        let streaming = streaming_state(lifecycle)?;
        if streaming.active.is_some() || streaming.message_ended || !streaming.emitted_content {
            return Err(stream_protocol_error());
        }
        let delta = frame
            .get("delta")
            .and_then(Value::as_object)
            .ok_or_else(stream_protocol_error)?;
        let stop_reason = decode_stop_reason(delta)?;
        let stop_sequence = decode_stop_sequence(delta)?;
        let reported = decode_usage(frame.get("usage"))?;
        if reported.output_tokens.is_none() {
            return Err(stream_protocol_error());
        }
        let usage = merge_usage(Some(&streaming.usage), &reported);
        if usage.input_tokens.is_none() {
            return Err(stream_protocol_error());
        }

        push_event(
            state,
            pending,
            CanonicalEvent::MessageEnd(MessageEnd::default()),
        )?;
        push_event(
            state,
            pending,
            CanonicalEvent::UsageDelta(UsageDelta {
                usage: usage.clone(),
                is_final: true,
                extensions: RawExtensions::default(),
            }),
        )?;
        streaming.usage = usage;
        streaming.message_ended = true;
        streaming.stop_reason = Some(stop_reason);
        streaming.stop_sequence = stop_sequence;
        Ok(())
    }

    fn consume_message_stop(&mut self) -> Result<(), GatewayError> {
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        let streaming = streaming_state(lifecycle)?;
        if !streaming.message_ended {
            return Err(stream_protocol_error());
        }
        let stop_reason = streaming.stop_reason.clone();
        let stop_sequence = streaming.stop_sequence.clone();
        push_event(
            state,
            pending,
            CanonicalEvent::ResponseEnd(ResponseEnd {
                stop_reason,
                stop_sequence,
                extensions: RawExtensions::default(),
            }),
        )?;
        *lifecycle = SseLifecycle::Finished;
        Ok(())
    }

    /// Projects one upstream `error` frame without letting its diagnostic text cross the boundary.
    ///
    /// Before any Canonical event was emitted the failure is still pre-first-byte, so the mapped
    /// error is returned rather than published as a terminal Canonical event; a later frame ends
    /// the already started stream instead.
    fn consume_error(&mut self, frame: &Map<String, Value>) -> Result<(), GatewayError> {
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        if matches!(lifecycle, SseLifecycle::Finished) {
            return Err(stream_protocol_error());
        }
        let error = frame
            .get("error")
            .and_then(Value::as_object)
            .ok_or_else(stream_protocol_error)?;
        let kind = error
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(stream_protocol_error)?;
        if !error.get("message").is_none_or(Value::is_string) {
            return Err(stream_protocol_error());
        }
        let mapped = upstream_error(kind);
        if matches!(lifecycle, SseLifecycle::AwaitingMessageStart) {
            return Err(mapped);
        }
        push_event(
            state,
            pending,
            CanonicalEvent::StreamError(StreamError { error: mapped }),
        )?;
        *lifecycle = SseLifecycle::Finished;
        Ok(())
    }
}

/// Validates one event against the Canonical state machine before it is queued.
fn push_event(
    state: &mut CanonicalEventState,
    pending: &mut VecDeque<CanonicalEvent>,
    event: CanonicalEvent,
) -> Result<(), GatewayError> {
    state.apply(&event)?;
    pending.push_back(event);
    Ok(())
}

fn streaming_state(lifecycle: &mut SseLifecycle) -> Result<&mut StreamingState, GatewayError> {
    match lifecycle {
        SseLifecycle::Streaming(state) => Ok(state.as_mut()),
        SseLifecycle::AwaitingMessageStart | SseLifecycle::Finished => Err(stream_protocol_error()),
    }
}

fn require_assistant_message(message: &Map<String, Value>) -> Result<(), GatewayError> {
    if message.get("type").and_then(Value::as_str) != Some("message")
        || message.get("role").and_then(Value::as_str) != Some("assistant")
    {
        return Err(stream_protocol_error());
    }
    Ok(())
}

fn decode_response_id(message: &Map<String, Value>) -> Result<ResponseId, GatewayError> {
    ResponseId::try_new(required_str(message, "id")?).map_err(|_| stream_protocol_error())
}

/// Requires a started text or thinking block to carry no value the fragments would then repeat.
fn require_empty_initial_value(block: &Map<String, Value>, name: &str) -> Result<(), GatewayError> {
    match block.get(name) {
        None => Ok(()),
        Some(Value::String(value)) if value.is_empty() => Ok(()),
        Some(_) => Err(stream_protocol_error()),
    }
}

fn frame_index(frame: &Map<String, Value>) -> Result<usize, GatewayError> {
    let index = frame
        .get("index")
        .and_then(Value::as_u64)
        .ok_or_else(stream_protocol_error)?;
    usize::try_from(index).map_err(|_| stream_protocol_error())
}

fn decode_stop_reason(object: &Map<String, Value>) -> Result<String, GatewayError> {
    let stop_reason = object
        .get("stop_reason")
        .and_then(Value::as_str)
        .ok_or_else(stream_protocol_error)?;
    if !ANTHROPIC_STOP_REASONS.contains(&stop_reason) {
        return Err(stream_protocol_error());
    }
    Ok(stop_reason.to_owned())
}

fn decode_stop_sequence(object: &Map<String, Value>) -> Result<Option<String>, GatewayError> {
    match object.get("stop_sequence") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() => Ok(Some(value.clone())),
        Some(_) => Err(stream_protocol_error()),
    }
}

/// Reads the four `Anthropic` usage counters the Canonical `Usage` can carry exactly.
///
/// `reasoning_tokens` and `cached_tokens` are never set: the sibling encoder rejects them because
/// an `Anthropic` usage object has no field to carry them back.
fn decode_usage(value: Option<&Value>) -> Result<Usage, GatewayError> {
    let usage = value
        .and_then(Value::as_object)
        .ok_or_else(stream_protocol_error)?;
    Ok(Usage {
        input_tokens: optional_u64(usage, "input_tokens")?,
        output_tokens: optional_u64(usage, "output_tokens")?,
        cache_read_tokens: optional_u64(usage, "cache_read_input_tokens")?,
        cache_creation_tokens: optional_u64(usage, "cache_creation_input_tokens")?,
        ..Usage::default()
    })
}

/// Narrows one usage report to the input-side counts that may precede the first output token.
fn input_usage_snapshot(usage: &Usage) -> Usage {
    Usage {
        input_tokens: usage.input_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_creation_tokens: usage.cache_creation_tokens,
        ..Usage::default()
    }
}

fn optional_u64(object: &Map<String, Value>, name: &str) -> Result<Option<u64>, GatewayError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(stream_protocol_error),
    }
}

fn required_str<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, GatewayError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(stream_protocol_error)
}

/// Projects one `Anthropic` error type onto the frozen gateway error taxonomy.
///
/// Only the classification crosses this boundary: the upstream's own message text is validated and
/// dropped, so no upstream diagnostic string can reach a client, a log, or an error envelope.
const fn upstream_error(kind: &str) -> GatewayError {
    match kind.as_bytes() {
        b"invalid_request_error" => {
            GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
        }
        b"authentication_error" => GatewayError::new(
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
        ),
        b"permission_error" => GatewayError::new(
            GatewayErrorCode::CredentialForbidden,
            ErrorScope::Credential,
        ),
        b"billing_error" => GatewayError::new(
            GatewayErrorCode::CredentialQuotaExceeded,
            ErrorScope::Credential,
        ),
        b"rate_limit_error" => {
            GatewayError::new(GatewayErrorCode::ProviderRateLimited, ErrorScope::Provider)
        }
        b"not_found_error" | b"request_too_large" => {
            GatewayError::new(GatewayErrorCode::ProviderPermanent, ErrorScope::Provider)
        }
        // `api_error`, `overloaded_error`, and any label this codec does not know are treated as
        // the same untyped upstream failure the sibling runtime already projects transiently.
        _ => provider_transient_error(),
    }
}

const fn provider_transient_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider)
}

#[cfg(test)]
mod tests {
    use gateway_core::{CanonicalEvent, CanonicalResponse, GatewayErrorCode};

    use super::{AnthropicMessagesSseDecoder, decode_upstream_response};
    use crate::{AnthropicMessagesSseEncoder, AnthropicResponseMetadata};

    const TEXT_STREAM: &str = include_str!("../../../tests/fixtures/anthropic/upstream-stream.sse");
    const TOOL_STREAM: &str =
        include_str!("../../../tests/fixtures/anthropic/upstream-tool-stream.sse");
    const THINKING_STREAM: &str =
        include_str!("../../../tests/fixtures/anthropic/upstream-thinking-stream.sse");

    /// Terminates the fixture with the one frame delimiter a real body always sends.
    fn body(fixture: &str) -> String {
        format!("{}\n\n", fixture.trim_end())
    }

    fn decode_stream(
        body: &str,
        chunk_size: usize,
    ) -> Result<Vec<CanonicalEvent>, gateway_core::GatewayError> {
        let mut decoder = AnthropicMessagesSseDecoder::new();
        let mut events = Vec::new();
        for chunk in body.as_bytes().chunks(chunk_size.max(1)) {
            decoder.push_chunk(chunk)?;
            loop {
                decoder.drain_buffered_frames()?;
                let Some(event) = decoder.take_event() else {
                    break;
                };
                events.push(event);
            }
        }
        decoder.finish()?;
        Ok(events)
    }

    #[test]
    fn upstream_sse_decodes_identically_at_every_chunk_boundary()
    -> Result<(), Box<dyn std::error::Error>> {
        for (fixture, expected) in [
            (
                TEXT_STREAM,
                include_str!(
                    "../../../tests/fixtures/anthropic/upstream-stream-canonical-events.json"
                ),
            ),
            (
                TOOL_STREAM,
                include_str!(
                    "../../../tests/fixtures/anthropic/upstream-tool-stream-canonical-events.json"
                ),
            ),
            (
                THINKING_STREAM,
                include_str!(
                    "../../../tests/fixtures/anthropic/upstream-thinking-stream-canonical-events.json"
                ),
            ),
        ] {
            let body = body(fixture);
            let expected: serde_json::Value = serde_json::from_str(expected)?;
            for chunk_size in [1, 3, 29, body.len()] {
                let events = decode_stream(&body, chunk_size)?;
                assert_eq!(
                    serde_json::to_value(&events)?,
                    expected,
                    "chunk size {chunk_size}"
                );
                assert!(CanonicalResponse::try_new(events).is_ok());
            }
        }
        Ok(())
    }

    #[test]
    fn tool_and_thinking_streams_carry_their_arguments_reasoning_and_stop_semantics()
    -> Result<(), Box<dyn std::error::Error>> {
        let tools = decode_stream(&body(TOOL_STREAM), 7)?;
        let fragments = tools
            .iter()
            .filter_map(|event| match event {
                CanonicalEvent::ToolCallArgumentsDelta(delta) => Some(delta.delta.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let completed = tools
            .iter()
            .filter_map(|event| match event {
                CanonicalEvent::ToolCallEnd(end) => Some(end.arguments.get()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(fragments, vec![r#"{"city":"#, r#""Jakarta"}"#]);
        assert_eq!(completed, vec![r#"{"city":"Jakarta"}"#, "{}"]);

        let thinking = decode_stream(&body(THINKING_STREAM), 5)?;
        assert!(matches!(thinking[3], CanonicalEvent::ReasoningDelta(_)));
        assert!(matches!(thinking[4], CanonicalEvent::TextDelta(_)));
        assert!(matches!(
            thinking.last(),
            Some(CanonicalEvent::ResponseEnd(end))
                if end.stop_reason.as_deref() == Some("stop_sequence")
                    && end.stop_sequence.as_deref() == Some("<END>")
        ));
        Ok(())
    }

    #[test]
    fn non_streaming_upstream_response_maps_to_the_canonical_sequence()
    -> Result<(), Box<dyn std::error::Error>> {
        let events = decode_upstream_response(include_str!(
            "../../../tests/fixtures/anthropic/upstream-non-streaming-response.json"
        ))?;
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/anthropic/upstream-non-streaming-canonical-events.json"
        ))?;

        assert_eq!(serde_json::to_value(&events)?, expected);
        assert!(matches!(events[1], CanonicalEvent::UsageDelta(ref delta) if !delta.is_final));
        Ok(())
    }

    #[test]
    fn every_decoded_upstream_response_re_encodes_through_the_sibling_anthropic_encoder()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut sequences = vec![decode_upstream_response(include_str!(
            "../../../tests/fixtures/anthropic/upstream-non-streaming-response.json"
        ))?];
        for fixture in [TEXT_STREAM, TOOL_STREAM, THINKING_STREAM] {
            sequences.push(decode_stream(&body(fixture), 13)?);
        }

        for events in sequences {
            let metadata = AnthropicResponseMetadata::try_new("claude-public")?;
            let mut encoder = AnthropicMessagesSseEncoder::new(metadata);
            for event in &events {
                let _frames = encoder.encode_event(event)?;
            }
            let message = encoder.into_completed_response()?;
            assert_eq!(message["role"], serde_json::json!("assistant"));
            assert!(message["usage"]["input_tokens"].is_u64());
            assert!(message["usage"]["output_tokens"].is_u64());
        }
        Ok(())
    }

    #[test]
    fn fails_closed_on_upstream_shapes_the_canonical_core_cannot_represent() {
        const START: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n";
        const TEXT_OPEN: &str = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        const TEXT_DELTA: &str = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n";
        const TEXT_CLOSE: &str =
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n";

        for body in [
            // unknown content block type
            format!("{START}event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"redacted_thinking\",\"data\":\"x\"}}}}\n\n"),
            // thinking signature has no canonical representation
            format!("{START}event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"thinking\",\"thinking\":\"\"}}}}\n\nevent: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"signature_delta\",\"signature\":\"abc\"}}}}\n\n"),
            // missing output usage in the terminal frame
            format!("{START}{TEXT_OPEN}{TEXT_DELTA}{TEXT_CLOSE}event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\",\"stop_sequence\":null}},\"usage\":{{}}}}\n\n"),
            // unknown stop reason
            format!("{START}{TEXT_OPEN}{TEXT_DELTA}{TEXT_CLOSE}event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"unexpected_stop\",\"stop_sequence\":null}},\"usage\":{{\"output_tokens\":1}}}}\n\n"),
            // malformed frame payload
            format!("{START}event: content_block_start\ndata: {{not json\n\n"),
            // non-monotone block index
            format!("{START}event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":3,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n"),
            // duplicate tool call identifier
            format!("{START}event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"tool_use\",\"id\":\"t\",\"name\":\"n\",\"input\":{{}}}}}}\n\nevent: content_block_stop\ndata: {{\"type\":\"content_block_stop\",\"index\":0}}\n\nevent: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":1,\"content_block\":{{\"type\":\"tool_use\",\"id\":\"t\",\"name\":\"n\",\"input\":{{}}}}}}\n\n"),
            // a started tool block that already carries a value
            format!("{START}event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"tool_use\",\"id\":\"t\",\"name\":\"n\",\"input\":{{\"a\":1}}}}}}\n\n"),
            // a started text block that already carries a value
            format!("{START}event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"text\",\"text\":\"already\"}}}}\n\n"),
            // a delta with no open block
            format!("{START}{TEXT_DELTA}"),
            // a second message_start
            format!("{START}{START}"),
            // no usage object at message_start
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[]}}\n\n".to_owned(),
        ] {
            assert!(decode_stream(&body, 1).is_err(), "accepted {body}");
            assert!(decode_stream(&body, body.len()).is_err(), "accepted {body}");
        }

        for message in [
            r#"{"id":"m","type":"message","role":"assistant","content":[{"type":"image"}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}"#,
            r#"{"id":"m","type":"message","role":"assistant","content":[{"type":"text","text":"x"}],"stop_reason":"end_turn","usage":{"input_tokens":1}}"#,
            r#"{"id":"m","type":"message","role":"assistant","content":[{"type":"text","text":"x"}],"stop_reason":"guessed","usage":{"input_tokens":1,"output_tokens":1}}"#,
            r#"{"id":"m","type":"message","role":"assistant","content":[],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}"#,
        ] {
            assert!(
                decode_upstream_response(message).is_err(),
                "accepted {message}"
            );
        }
    }

    #[test]
    fn a_body_cut_before_message_stop_reports_stream_truncated()
    -> Result<(), Box<dyn std::error::Error>> {
        let full = body(TEXT_STREAM);
        let cut = full
            .rfind("event: message_stop")
            .ok_or("fixture has no message_stop frame")?;
        let truncated = decode_stream(&full[..cut], 1);

        assert_eq!(
            truncated.err().map(|error| error.code()),
            Some(GatewayErrorCode::StreamTruncated)
        );
        Ok(())
    }

    #[test]
    fn error_frames_map_to_a_terminal_stream_error_only_after_the_first_canonical_event()
    -> Result<(), Box<dyn std::error::Error>> {
        const START: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n";
        let overloaded = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"upstream secret detail\"}}\n\n";

        let before = decode_stream(overloaded, 1);
        assert_eq!(
            before.as_ref().err().map(gateway_core::GatewayError::code),
            Some(GatewayErrorCode::ProviderTransient)
        );
        let diagnostic = format!("{before:?}");
        assert!(!diagnostic.contains("upstream secret detail"));

        let events = decode_stream(&format!("{START}{overloaded}"), 1)?;
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::StreamError(error))
                if error.error.code() == GatewayErrorCode::ProviderTransient
        ));

        let rate_limited = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\",\"message\":\"slow down\"}}\n\n";
        assert_eq!(
            decode_stream(rate_limited, 1)
                .err()
                .map(|error| error.code()),
            Some(GatewayErrorCode::ProviderRateLimited)
        );
        Ok(())
    }

    #[test]
    fn keepalive_frames_never_change_the_canonical_sequence_or_the_progress_marks()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut decoder = AnthropicMessagesSseDecoder::new();
        decoder.push_chunk(b": keepalive\n\nevent: ping\ndata: {\"type\":\"ping\"}\n\n")?;
        decoder.drain_buffered_frames()?;

        assert_eq!(decoder.progress_marks(), 0);
        assert!(decoder.take_event().is_none());

        decoder.push_chunk(b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n")?;
        decoder.drain_buffered_frames()?;

        assert_eq!(decoder.progress_marks(), 1);
        assert!(matches!(
            decoder.take_event(),
            Some(CanonicalEvent::ResponseStart(_))
        ));
        assert!(!decoder.is_finished());
        Ok(())
    }

    #[test]
    fn an_oversized_undecoded_residue_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
        let mut decoder = AnthropicMessagesSseDecoder::new();
        let chunk = vec![b'x'; 1024 * 1024];
        let mut rejected = false;
        for _ in 0..16 {
            if decoder.push_chunk(&chunk).is_err() {
                rejected = true;
                break;
            }
            decoder.drain_buffered_frames()?;
        }

        assert!(rejected);
        Ok(())
    }

    #[test]
    fn decoder_debug_reports_only_shape_never_upstream_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut decoder = AnthropicMessagesSseDecoder::new();
        decoder.push_chunk(body(TEXT_STREAM).as_bytes())?;
        decoder.drain_buffered_frames()?;
        let diagnostic = format!("{decoder:?}");

        for value in ["msg_upstream_01", "Hello", " there", "claude-upstream"] {
            assert!(!diagnostic.contains(value));
        }
        assert!(diagnostic.contains("AnthropicMessagesSseDecoder"));
        Ok(())
    }
}
