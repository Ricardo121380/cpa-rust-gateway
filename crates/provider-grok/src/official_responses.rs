//! Native xAI Official API-key Responses HTTP and SSE boundary.
//!
//! This module is deliberately independent from the Grok Build OAuth Responses implementation.
//! It owns the fixed Official request profile, its bounded text-only codec, and the injected
//! transport/adapter vertical slice. P8-03 owns HTTP quota/status semantics and P8-04 expands
//! this deliberately narrow text-only subset to Tool, Reasoning, and Search semantics.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    sync::Arc,
};

use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalMessage, CanonicalRequest, CanonicalResponse,
    ErrorScope, GatewayError, GatewayErrorCode, MessageContent, MessageEnd, MessageRole,
    MessageStart, RawExtensions, RawJson, ReasoningDelta, RequestContext, ResponseEnd, ResponseId,
    ResponseStart, StreamError, TextDelta, ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart,
    ToolDefinition, Usage, UsageDelta,
};
use gateway_provider::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};
use gateway_upstream::{
    AdmittedEgressTarget, EgressDnsResolver, EgressPolicy, EndpointUrl, UpstreamClientPool,
    UpstreamHttpMethod, UpstreamHttpRequest, UpstreamHttpResponse, UpstreamTransportProfile,
};
use protocol_openai_responses::ResponseMode;
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::native_response_semantics as native;
use crate::{
    GROK_OFFICIAL_API_BASE_URL, GROK_OFFICIAL_PROVIDER_ID, GrokOfficialApiKey,
    GrokOfficialRateLimitMetadata, GrokOfficialRuntimeState, classify_grok_official_http_failure,
    strict_json::{could_be_empty_tool_arguments, parse_strict_json},
};

/// Fixed Official Responses path.
pub const GROK_OFFICIAL_RESPONSES_PATH: &str = "/responses";
/// Full fixed Official Responses URL.
pub const GROK_OFFICIAL_RESPONSES_URL: &str = "https://api.x.ai/v1/responses";
/// Maximum bytes buffered for one completed Official JSON response.
pub const MAX_GROK_OFFICIAL_NON_STREAMING_RESPONSE_BYTES: usize = 1024 * 1024;
/// Maximum retained bytes for one pre-start Official HTTP failure body.
pub const MAX_GROK_OFFICIAL_ERROR_BODY_BYTES: usize = 64 * 1024;
/// Maximum bytes for one complete Official SSE record, excluding its blank-line delimiter.
pub const MAX_GROK_OFFICIAL_SSE_FRAME_BYTES: usize = 64 * 1024;

const MAX_GROK_OFFICIAL_MODEL_BYTES: usize = 512;
const MAX_GROK_OFFICIAL_TOOL_ARGUMENT_BYTES: usize = 64 * 1024;
const OFFICIAL_REASONING_EFFORTS: &[&str] = &["low", "medium", "high"];

/// The fixed production xAI Official Responses endpoint.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokOfficialResponsesEndpoint {
    target: EndpointUrl,
}

impl GrokOfficialResponsesEndpoint {
    /// Creates the fixed Official `POST /v1/responses` target.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` if a future edit makes the immutable endpoint invalid.
    pub fn try_new() -> Result<Self, GatewayError> {
        let target = EndpointUrl::compose(GROK_OFFICIAL_API_BASE_URL, GROK_OFFICIAL_RESPONSES_PATH)
            .map_err(|_| egress_rejected_error())?;
        Ok(Self { target })
    }

    /// Returns the complete fixed Responses URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }
}

impl fmt::Debug for GrokOfficialResponsesEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokOfficialResponsesEndpoint(<redacted>)")
    }
}

/// A request-ready Official Responses submission.
///
/// The only credential-derived value is zeroizing and request-scoped. `Debug` never exposes the
/// endpoint, API key, selected model, request body, or body-derived client text.
#[derive(Eq, PartialEq)]
pub struct GrokOfficialResponsesOutboundRequest {
    target: EndpointUrl,
    authorization: Zeroizing<String>,
    accept: &'static str,
    body: Vec<u8>,
}

impl GrokOfficialResponsesOutboundRequest {
    /// Returns the complete configured endpoint URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }

    /// Returns one standard request header by case-insensitive name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("accept") {
            Some(self.accept)
        } else if name.eq_ignore_ascii_case("accept-encoding") {
            Some("identity")
        } else if name.eq_ignore_ascii_case("authorization") {
            Some(self.authorization.as_str())
        } else if name.eq_ignore_ascii_case("content-type") {
            Some("application/json")
        } else {
            None
        }
    }

    /// Returns headers in deterministic transport order.
    #[must_use]
    pub fn headers(&self) -> [(&'static str, &str); 4] {
        [
            ("accept", self.accept),
            ("accept-encoding", "identity"),
            ("authorization", self.authorization.as_str()),
            ("content-type", "application/json"),
        ]
    }

    /// Returns the exact JSON payload without providing a log representation.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Consumes the request into one exact-target, DNS-pinned shared transport request.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` when the admitted target differs from the immutable
    /// Official endpoint, or `InternalError/Internal` for a shared-transport invariant failure.
    pub fn into_transport_request(
        self,
        admitted_target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GatewayError> {
        if admitted_target.request_url() != self.target.as_url() {
            return Err(egress_rejected_error());
        }
        let headers = self
            .headers()
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();
        UpstreamHttpRequest::try_new(
            admitted_target,
            UpstreamHttpMethod::Post,
            headers,
            self.body,
        )
        .map_err(|_| internal_error())
    }
}

impl fmt::Debug for GrokOfficialResponsesOutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialResponsesOutboundRequest")
            .field("target", &"<redacted>")
            .field(
                "header_names",
                &["accept", "accept-encoding", "authorization", "content-type"],
            )
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Stateless builder for the supported Official Responses subset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokOfficialResponsesRequestBuilder;

impl GrokOfficialResponsesRequestBuilder {
    /// Builds one API-key-authenticated Official Responses request.
    ///
    /// Accepts text, Function Tools, historical Function Calls/Results, bounded named Reasoning
    /// effort, and reviewed native assistant item/part history metadata. Cache, opaque content,
    /// native Search, and unknown provider extensions remain rejected until they have their own
    /// lossless Canonical contracts.
    ///
    /// # Errors
    ///
    /// Returns a safe client request error before an unrepresentable request reaches a transport.
    pub fn build(
        credential: &GrokOfficialApiKey,
        upstream_model: &str,
        request: &CanonicalRequest,
        mode: ResponseMode,
    ) -> Result<GrokOfficialResponsesOutboundRequest, GatewayError> {
        if upstream_model.is_empty()
            || upstream_model.len() > MAX_GROK_OFFICIAL_MODEL_BYTES
            || !upstream_model.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(client_request_error());
        }
        let endpoint = GrokOfficialResponsesEndpoint::try_new()?;
        let accept = match mode {
            ResponseMode::NonStreaming => "application/json",
            ResponseMode::Streaming => "text/event-stream",
        };
        Ok(GrokOfficialResponsesOutboundRequest {
            target: endpoint.target,
            authorization: Zeroizing::new(format!("Bearer {}", credential.as_str())),
            accept,
            body: encode_body(upstream_model, request, mode)?,
        })
    }
}

fn encode_body(
    upstream_model: &str,
    request: &CanonicalRequest,
    mode: ResponseMode,
) -> Result<Vec<u8>, GatewayError> {
    encode_responses_body(upstream_model, request, mode, OFFICIAL_REASONING_EFFORTS)
}

pub(crate) fn encode_responses_body(
    upstream_model: &str,
    request: &CanonicalRequest,
    mode: ResponseMode,
    reasoning_efforts: &[&str],
) -> Result<Vec<u8>, GatewayError> {
    if request.prompt_cache_key.is_some() || request.prompt_cache_retention.is_some() {
        return Err(client_request_error());
    }

    request.validate_tool_history()?;
    gateway_router::ToolExecutionConstraints::from_request(
        request,
        gateway_router::ProtocolFormat::OpenAiResponses,
    )
    .map_err(|_| client_request_error())?;

    let mut root = Map::new();
    root.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    root.insert(
        "stream".to_owned(),
        Value::Bool(matches!(mode, ResponseMode::Streaming)),
    );
    root.insert(
        "input".to_owned(),
        Value::Array(encode_input(&request.messages)?),
    );
    if !request.tools.is_empty() {
        root.insert(
            "tools".to_owned(),
            Value::Array(encode_tools(&request.tools)?),
        );
    }
    if let Some(thinking) = &request.thinking {
        root.insert(
            "reasoning".to_owned(),
            encode_reasoning(thinking, reasoning_efforts)?,
        );
    }
    for (name, raw) in request.extensions.iter() {
        let field = name
            .strip_prefix("openai.responses.")
            .filter(|field| {
                matches!(
                    *field,
                    "max_output_tokens" | "tool_choice" | "parallel_tool_calls"
                )
            })
            .ok_or_else(client_request_error)?;
        let value = raw_value(raw)?;
        if field == "max_output_tokens" && value.as_u64().is_none_or(|limit| limit == 0) {
            return Err(client_request_error());
        }
        root.insert(field.to_owned(), value);
    }
    serde_json::to_vec(&Value::Object(root)).map_err(|_| internal_error())
}

fn encode_input(messages: &[CanonicalMessage]) -> Result<Vec<Value>, GatewayError> {
    let mut input = Vec::with_capacity(messages.len());
    for message in messages {
        let role = message.role.0.as_str();
        if !matches!(role, "assistant" | "developer" | "system" | "tool" | "user")
            || message.content.is_empty()
        {
            return Err(client_request_error());
        }
        if !message.extensions.is_empty()
            && (role != "assistant"
                || message
                    .content
                    .iter()
                    .any(|part| !matches!(part, MessageContent::Text(_))))
        {
            return Err(client_request_error());
        }
        let mut content = Vec::with_capacity(message.content.len());
        for part in &message.content {
            match part {
                MessageContent::Text(text) => content.push(encode_text_part(role, text)?),
                MessageContent::Reasoning(history) => {
                    flush_message_content(&mut input, role, &message.extensions, &mut content)?;
                    if role != "assistant" {
                        return Err(client_request_error());
                    }
                    let item = protocol_openai_responses::encode_reasoning_history(history)?;
                    item_metadata(item.as_object().ok_or_else(client_request_error)?, true)
                        .map_err(|_| client_request_error())?;
                    input.push(item);
                }
                MessageContent::ToolCall(call) => {
                    flush_message_content(&mut input, role, &message.extensions, &mut content)?;
                    if role != "assistant" {
                        return Err(client_request_error());
                    }
                    input.push(encode_tool_call(call)?);
                }
                MessageContent::ToolResult(result) => {
                    flush_message_content(&mut input, role, &message.extensions, &mut content)?;
                    if role != "tool" || result.is_error {
                        return Err(client_request_error());
                    }
                    input.push(encode_tool_result(result)?);
                }
                MessageContent::Opaque(_) => return Err(client_request_error()),
            }
        }
        flush_message_content(&mut input, role, &message.extensions, &mut content)?;
    }
    Ok(input)
}

fn flush_message_content(
    input: &mut Vec<Value>,
    role: &str,
    extensions: &RawExtensions,
    content: &mut Vec<Value>,
) -> Result<(), GatewayError> {
    if content.is_empty() {
        return Ok(());
    }
    if role == "tool" {
        return Err(client_request_error());
    }
    let mut item = Map::from_iter([
        ("type".to_owned(), Value::String("message".to_owned())),
        ("role".to_owned(), Value::String(role.to_owned())),
        ("content".to_owned(), Value::Array(std::mem::take(content))),
    ]);
    insert_history_fields(&mut item, extensions)?;
    input.push(Value::Object(item));
    Ok(())
}

fn encode_text_part(role: &str, text: &gateway_core::TextContent) -> Result<Value, GatewayError> {
    if role != "assistant" && !text.extensions.is_empty() {
        return Err(client_request_error());
    }
    let content_type = match role {
        "assistant" => "output_text",
        "developer" | "system" | "user" => "input_text",
        _ => return Err(client_request_error()),
    };
    let mut part = Map::from_iter([
        ("type".to_owned(), Value::String(content_type.to_owned())),
        ("text".to_owned(), Value::String(text.text.clone())),
    ]);
    for (key, value) in text.extensions.iter() {
        if !matches!(key, "annotations" | "logprobs") {
            return Err(client_request_error());
        }
        part.insert(key.to_owned(), raw_value(value)?);
    }
    if role == "assistant" {
        native::validate_part(&part).map_err(|_| client_request_error())?;
    }
    Ok(Value::Object(part))
}

fn encode_tool_call(call: &gateway_core::ToolCall) -> Result<Value, GatewayError> {
    if call.id.is_empty() || call.name.is_empty() {
        return Err(client_request_error());
    }
    let arguments = normalize_outbound_tool_arguments(call.arguments.get())?;
    let mut item = Map::from_iter([
        ("type".to_owned(), Value::String("function_call".to_owned())),
        ("call_id".to_owned(), Value::String(call.id.clone())),
        ("name".to_owned(), Value::String(call.name.clone())),
        ("arguments".to_owned(), Value::String(arguments)),
    ]);
    insert_history_fields(&mut item, &call.extensions)?;
    Ok(Value::Object(item))
}

fn insert_history_fields(
    item: &mut Map<String, Value>,
    extensions: &RawExtensions,
) -> Result<(), GatewayError> {
    for (key, raw) in extensions.iter() {
        let value = raw_value(raw)?;
        match key {
            "id" if value.as_str().is_some_and(|id| {
                !id.is_empty() && id.len() <= 512 && id.bytes().all(|byte| byte.is_ascii_graphic())
            }) => {}
            "status"
                if matches!(
                    value.as_str(),
                    Some("completed" | "incomplete" | "in_progress")
                ) => {}
            "phase"
                if item.get("type").and_then(Value::as_str) == Some("message")
                    && matches!(value.as_str(), Some("commentary" | "final_answer")) => {}
            _ => return Err(client_request_error()),
        }
        item.insert(key.to_owned(), value);
    }
    Ok(())
}

fn encode_tool_result(result: &gateway_core::ToolResult) -> Result<Value, GatewayError> {
    if result.call_id.is_empty() || !result.extensions.is_empty() {
        return Err(client_request_error());
    }
    let output = raw_value(&result.output)?;
    if !output.is_string() {
        return Err(client_request_error());
    }
    Ok(Value::Object(Map::from_iter([
        (
            "type".to_owned(),
            Value::String("function_call_output".to_owned()),
        ),
        ("call_id".to_owned(), Value::String(result.call_id.clone())),
        ("output".to_owned(), output),
    ])))
}

fn encode_tools(tools: &[ToolDefinition]) -> Result<Vec<Value>, GatewayError> {
    tools.iter().map(encode_tool).collect()
}

fn encode_tool(tool: &ToolDefinition) -> Result<Value, GatewayError> {
    if tool.name.is_empty() || !tool.extensions.is_empty() {
        return Err(client_request_error());
    }
    let parameters = raw_value(&tool.input_schema)?;
    if !parameters.is_object() {
        return Err(client_request_error());
    }
    let mut encoded = Map::new();
    encoded.insert("type".to_owned(), Value::String("function".to_owned()));
    encoded.insert("name".to_owned(), Value::String(tool.name.clone()));
    if let Some(description) = &tool.description {
        encoded.insert("description".to_owned(), Value::String(description.clone()));
    }
    encoded.insert("parameters".to_owned(), parameters);
    Ok(Value::Object(encoded))
}

fn encode_reasoning(
    thinking: &gateway_core::Thinking,
    supported_efforts: &[&str],
) -> Result<Value, GatewayError> {
    if !thinking.extensions.is_empty() || !supported_efforts.contains(&thinking.effort.as_str()) {
        return Err(client_request_error());
    }
    Ok(Value::Object(Map::from_iter([(
        "effort".to_owned(),
        Value::String(thinking.effort.as_str().to_owned()),
    )])))
}

fn raw_value(raw: &RawJson) -> Result<Value, GatewayError> {
    parse_strict_json(raw.get().as_bytes(), MAX_GROK_OFFICIAL_TOOL_ARGUMENT_BYTES)
        .map_err(|()| client_request_error())
}

fn normalize_outbound_tool_arguments(arguments: &str) -> Result<String, GatewayError> {
    if arguments.len() > MAX_GROK_OFFICIAL_TOOL_ARGUMENT_BYTES {
        return Err(client_request_error());
    }
    let value = parse_strict_json(arguments.as_bytes(), MAX_GROK_OFFICIAL_TOOL_ARGUMENT_BYTES)
        .map_err(|()| client_request_error())?;
    if value.is_object() {
        Ok(arguments.to_owned())
    } else {
        Err(client_request_error())
    }
}

/// Decodes one bounded completed Official Responses object.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokOfficialResponsesDecoder;

impl GrokOfficialResponsesDecoder {
    /// Converts one completed Official Responses JSON object into a validated Canonical response.
    ///
    /// Function calls and exported Reasoning map to their explicit Canonical semantics. Search
    /// results and opaque output still fail closed because no current Canonical contract represents
    /// their provider-owned payloads.
    ///
    /// # Errors
    ///
    /// Returns `UpstreamProtocolError/Provider` for an oversized, ambiguous, malformed, incomplete,
    /// or text-unrepresentable response, including an invalid Canonical lifecycle.
    pub fn decode_non_streaming(input: &[u8]) -> Result<CanonicalResponse, GatewayError> {
        let value = parse_strict_json(input, MAX_GROK_OFFICIAL_NON_STREAMING_RESPONSE_BYTES)
            .map_err(|()| provider_protocol_error())?;
        let response = value.as_object().ok_or_else(provider_protocol_error)?;
        let mut state = GrokOfficialResponsesDecodeState::default();
        let mut events = Vec::new();
        state.handle_response_created(response, &mut events)?;
        if required_string(response, "status", provider_protocol_error())? != "completed" {
            return Err(provider_protocol_error());
        }
        let output = required_array(response, "output", provider_protocol_error())?;
        for item in output {
            let item = item.as_object().ok_or_else(provider_protocol_error)?;
            state.handle_output_item_added(item, &mut events)?;
            state.handle_output_item_done(item, &mut events)?;
        }
        state.handle_response_completed(response, &mut events)?;
        CanonicalResponse::try_new(events)
    }
}

/// Incremental parser for one Official Responses SSE byte stream.
#[derive(Clone, Default)]
pub struct GrokOfficialResponsesStreamDecoder {
    pending: Vec<u8>,
    state: GrokOfficialResponsesDecodeState,
}

impl GrokOfficialResponsesStreamDecoder {
    /// Creates an empty decoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Accepts arbitrary byte chunking and returns newly decoded Canonical events.
    ///
    /// Both the pending bytes and semantic state commit only after the whole supplied chunk
    /// parses, so a malformed record cannot leave a partially advanced stream behind.
    ///
    /// # Errors
    ///
    /// Returns `UpstreamProtocolError/Stream` for invalid or oversized framing, malformed/ambiguous
    /// JSON, unsupported output semantics, response/item correlation failure, or an invalid
    /// Canonical lifecycle.
    pub fn push_bytes(&mut self, chunk: &[u8]) -> Result<Vec<CanonicalEvent>, GatewayError> {
        let mut pending = self.pending.clone();
        let mut state = self.state.clone();
        let mut events = Vec::new();

        for byte in chunk {
            pending.push(*byte);
            if let Some(delimiter_length) = sse_delimiter_length(&pending) {
                let record_length = pending
                    .len()
                    .checked_sub(delimiter_length)
                    .ok_or_else(stream_protocol_error)?;
                if record_length > MAX_GROK_OFFICIAL_SSE_FRAME_BYTES {
                    return Err(stream_protocol_error());
                }
                let mut record = std::mem::take(&mut pending);
                record.truncate(record_length);
                state.handle_sse_record(&record, &mut events)?;
            } else if pending.len() > MAX_GROK_OFFICIAL_SSE_FRAME_BYTES + 4 {
                return Err(stream_protocol_error());
            }
        }

        self.pending = pending;
        self.state = state;
        Ok(events)
    }

    /// Verifies the source ended at an SSE boundary after a terminal semantic event.
    ///
    /// # Errors
    ///
    /// Returns `StreamTruncated/Stream` for a partial terminal record or a stream that did not
    /// emit `response.completed` or `response.failed`.
    pub fn finish(&self) -> Result<(), GatewayError> {
        if !self.pending.is_empty() {
            return Err(stream_truncated_error());
        }
        self.state.canonical.finish()
    }
}

impl fmt::Debug for GrokOfficialResponsesStreamDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialResponsesStreamDecoder")
            .field("pending_byte_count", &self.pending.len())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Default)]
struct GrokOfficialResponsesDecodeState {
    canonical: CanonicalEventState,
    response_id: Option<String>,
    message_open: bool,
    item_kinds: BTreeMap<String, OutputItemKind>,
    completed_item_ids: BTreeSet<String>,
    completed_items: BTreeMap<String, Value>,
    item_order: Vec<String>,
    item_phases: BTreeMap<String, Value>,
    active_content_parts: BTreeSet<(String, String, usize)>,
    part_snapshots: BTreeMap<(String, String, usize), Value>,
    annotations_by_part: BTreeMap<(String, usize), Vec<Value>>,
    function_call_ids: BTreeMap<String, String>,
    function_call_names: BTreeMap<String, String>,
    function_arguments: BTreeMap<String, String>,
    emitted_function_arguments: BTreeSet<String>,
    completed_function_calls: BTreeSet<String>,
    parts_by_item: BTreeMap<(String, String, usize), String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputItemKind {
    Message,
    Reasoning,
    FunctionCall,
}

impl fmt::Debug for GrokOfficialResponsesDecodeState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialResponsesDecodeState")
            .field("canonical", &self.canonical)
            .field("response_started", &self.response_id.is_some())
            .field("message_open", &self.message_open)
            .field("output_item_count", &self.item_kinds.len())
            .field("completed_item_count", &self.completed_item_ids.len())
            .field("completed_snapshot_count", &self.completed_items.len())
            .field(
                "active_content_part_count",
                &self.active_content_parts.len(),
            )
            .field("function_call_count", &self.function_call_ids.len())
            .field("function_argument_count", &self.function_arguments.len())
            .field("part_count", &self.parts_by_item.len())
            .finish_non_exhaustive()
    }
}

impl GrokOfficialResponsesDecodeState {
    fn handle_sse_record(
        &mut self,
        record: &[u8],
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        if record.is_empty() {
            return Ok(());
        }
        let record = std::str::from_utf8(record).map_err(|_| stream_protocol_error())?;
        let mut event_name = None;
        let mut data_lines = Vec::new();
        for raw_line in record.split('\n') {
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = match line.split_once(':') {
                Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
                None => (line, ""),
            };
            match field {
                "event" => {
                    if event_name.replace(value.to_owned()).is_some() || value.is_empty() {
                        return Err(stream_protocol_error());
                    }
                }
                "data" => data_lines.push(value),
                _ => return Err(stream_protocol_error()),
            }
        }
        if event_name.is_none() && data_lines.is_empty() {
            return Ok(());
        }
        let event_name = event_name.ok_or_else(stream_protocol_error)?;
        if data_lines.is_empty() {
            return Err(stream_protocol_error());
        }
        let data = data_lines.join("\n");
        if data == "[DONE]" {
            return if self.canonical.is_terminal() {
                Ok(())
            } else {
                Err(stream_protocol_error())
            };
        }

        let value = parse_strict_json(data.as_bytes(), MAX_GROK_OFFICIAL_SSE_FRAME_BYTES)
            .map_err(|()| stream_protocol_error())?;
        let object = value.as_object().ok_or_else(stream_protocol_error)?;
        if required_string(object, "type", stream_protocol_error())? != event_name {
            return Err(stream_protocol_error());
        }
        native::validate_event(object, &self.item_order)?;
        match event_name.as_str() {
            "response.created" => self.handle_response_created(
                required_object(object, "response", stream_protocol_error())?,
                events,
            ),
            "response.in_progress" => self.handle_response_in_progress(required_object(
                object,
                "response",
                stream_protocol_error(),
            )?),
            "response.output_item.added" => self.handle_output_item_added(
                required_object(object, "item", stream_protocol_error())?,
                events,
            ),
            "response.content_part.added" | "response.reasoning_summary_part.added" => {
                self.handle_content_part_added(object, events)
            }
            "response.output_text.delta" => self.handle_text_delta(object, events),
            "response.output_text.done" => self.handle_text_done(object, events),
            "response.output_text.annotation.added" => self.handle_annotation(object, events),
            "response.content_part.done" | "response.reasoning_summary_part.done" => {
                self.handle_content_part_done(object, events)
            }
            "response.reasoning.delta"
            | "response.reasoning_text.delta"
            | "response.reasoning_summary_text.delta" => {
                self.handle_reasoning_delta(object, events)
            }
            "response.reasoning.done"
            | "response.reasoning_text.done"
            | "response.reasoning_summary_text.done" => self.handle_reasoning_done(object, events),
            "response.function_call_arguments.delta" => {
                self.handle_function_arguments_delta(object, events)
            }
            "response.function_call_arguments.done" => {
                self.handle_function_arguments_done(object, events)
            }
            "response.output_item.done" => self.handle_output_item_done(
                required_object(object, "item", stream_protocol_error())?,
                events,
            ),
            "response.completed" => self.handle_response_completed(
                required_object(object, "response", stream_protocol_error())?,
                events,
            ),
            "response.failed" => self.handle_response_failed(object, events),
            _ => Err(stream_protocol_error()),
        }
    }

    fn handle_response_created(
        &mut self,
        response: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        if self.response_id.is_some() {
            return Err(stream_protocol_error());
        }
        let response_id = required_identifier(response, "id", stream_protocol_error())?;
        let canonical_id =
            ResponseId::try_new(response_id.to_owned()).map_err(|_| stream_protocol_error())?;
        self.emit(
            events,
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id: canonical_id,
                extensions: RawExtensions::default(),
            }),
        )?;
        self.response_id = Some(response_id.to_owned());
        Ok(())
    }

    fn handle_response_in_progress(
        &self,
        response: &Map<String, Value>,
    ) -> Result<(), GatewayError> {
        self.require_matching_response(response)
    }

    fn handle_output_item_added(
        &mut self,
        item: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        self.require_response_started()?;
        let metadata = item_metadata(item, false)?;
        let item_id = required_identifier(item, "id", stream_protocol_error())?;
        if self.item_kinds.contains_key(item_id) || self.item_order.len() >= 256 {
            return Err(stream_protocol_error());
        }
        let kind = match required_string(item, "type", stream_protocol_error())? {
            "message" if required_string(item, "role", stream_protocol_error())? == "assistant" => {
                OutputItemKind::Message
            }
            "reasoning" => OutputItemKind::Reasoning,
            "function_call" => OutputItemKind::FunctionCall,
            _ => return Err(stream_protocol_error()),
        };
        self.ensure_message(events)?;
        self.emit(events, CanonicalEvent::OutputItemStart(metadata))?;
        if kind == OutputItemKind::FunctionCall {
            let call_id = required_identifier(item, "call_id", stream_protocol_error())?;
            let name = required_identifier(item, "name", stream_protocol_error())?;
            if self
                .function_call_ids
                .values()
                .any(|known_call_id| known_call_id == call_id)
            {
                return Err(stream_protocol_error());
            }
            self.emit(
                events,
                CanonicalEvent::ToolCallStart(ToolCallStart {
                    call_id: call_id.to_owned(),
                    name: name.to_owned(),
                    extensions: RawExtensions::default(),
                }),
            )?;
            self.function_call_ids
                .insert(item_id.to_owned(), call_id.to_owned());
            self.function_call_names
                .insert(item_id.to_owned(), name.to_owned());
        }
        if let Some(phase) = item.get("phase") {
            self.item_phases.insert(item_id.to_owned(), phase.clone());
        }
        self.item_order.push(item_id.to_owned());
        self.item_kinds.insert(item_id.to_owned(), kind);
        Ok(())
    }

    fn handle_content_part_added(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        let field = event_part_field(event);
        let index = native::part_index(
            event,
            if field == "summary" {
                "summary_index"
            } else {
                "content_index"
            },
        )?;
        if self.completed_item_ids.contains(item_id)
            || !self
                .active_content_parts
                .insert((item_id.to_owned(), field.to_owned(), index))
        {
            return Err(stream_protocol_error());
        }
        let part = required_object(event, "part", stream_protocol_error())?;
        native::validate_part(part)?;
        if required_string(part, "type", stream_protocol_error())?
            != self.part_kind(item_id, field)?
            || !required_string(part, "text", stream_protocol_error())?.is_empty()
        {
            return Err(stream_protocol_error());
        }
        for extensions in
            native::record_part_annotations(item_id, index, part, &mut self.annotations_by_part)?
        {
            self.emit(
                events,
                CanonicalEvent::TextDelta(TextDelta {
                    text: String::new(),
                    extensions,
                }),
            )?;
        }
        Ok(())
    }

    fn handle_text_delta(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(item_id) != Some(&OutputItemKind::Message)
            || self.completed_item_ids.contains(item_id)
        {
            return Err(stream_protocol_error());
        }
        self.append_part(
            item_id,
            "content",
            native::part_index(event, "content_index")?,
            required_string(event, "delta", stream_protocol_error())?,
            events,
        )
    }

    fn handle_annotation(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(id) != Some(&OutputItemKind::Message)
            || self.completed_item_ids.contains(id)
        {
            return Err(stream_protocol_error());
        }
        let extensions = native::record_annotation(event, &mut self.annotations_by_part)?;
        self.emit(
            events,
            CanonicalEvent::TextDelta(TextDelta {
                text: String::new(),
                extensions,
            }),
        )
    }

    fn handle_text_done(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(item_id) != Some(&OutputItemKind::Message) {
            return Err(stream_protocol_error());
        }
        self.finish_part(
            item_id,
            "content",
            native::part_index(event, "content_index")?,
            required_string(event, "text", stream_protocol_error())?,
            events,
        )
    }

    fn handle_reasoning_delta(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(item_id) != Some(&OutputItemKind::Reasoning)
            || self.completed_item_ids.contains(item_id)
        {
            return Err(stream_protocol_error());
        }
        let field = event_part_field(event);
        self.append_part(
            item_id,
            field,
            native::part_index(
                event,
                if field == "summary" {
                    "summary_index"
                } else {
                    "content_index"
                },
            )?,
            required_string(event, "delta", stream_protocol_error())?,
            events,
        )
    }

    fn handle_reasoning_done(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(item_id) != Some(&OutputItemKind::Reasoning) {
            return Err(stream_protocol_error());
        }
        let field = event_part_field(event);
        self.finish_part(
            item_id,
            field,
            native::part_index(
                event,
                if field == "summary" {
                    "summary_index"
                } else {
                    "content_index"
                },
            )?,
            required_string(event, "text", stream_protocol_error())?,
            events,
        )
    }

    fn handle_function_arguments_delta(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        let call_id = self
            .function_call_ids
            .get(item_id)
            .cloned()
            .ok_or_else(stream_protocol_error)?;
        if event
            .get("call_id")
            .is_some_and(|value| value.as_str() != Some(call_id.as_str()))
            || self.completed_function_calls.contains(&call_id)
        {
            return Err(stream_protocol_error());
        }
        let delta = required_string(event, "delta", stream_protocol_error())?;
        if delta.is_empty() {
            return Ok(());
        }
        let next_length = self
            .function_arguments
            .get(&call_id)
            .map_or(0, String::len)
            .checked_add(delta.len())
            .ok_or_else(stream_protocol_error)?;
        if next_length > MAX_GROK_OFFICIAL_TOOL_ARGUMENT_BYTES {
            return Err(stream_protocol_error());
        }
        let arguments = self.function_arguments.entry(call_id.clone()).or_default();
        arguments.push_str(delta);
        let delta = if self.emitted_function_arguments.contains(&call_id) {
            delta.to_owned()
        } else if could_be_empty_tool_arguments(arguments) {
            return Ok(());
        } else {
            arguments.clone()
        };
        self.emit(
            events,
            CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
                call_id: call_id.clone(),
                delta,
                extensions: RawExtensions::default(),
            }),
        )?;
        self.emitted_function_arguments.insert(call_id);
        Ok(())
    }

    fn handle_function_arguments_done(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        let call_id = self
            .function_call_ids
            .get(item_id)
            .cloned()
            .ok_or_else(stream_protocol_error)?;
        if event
            .get("call_id")
            .is_some_and(|value| value.as_str() != Some(call_id.as_str()))
            || event.get("name").is_some_and(|value| {
                value.as_str() != self.function_call_names.get(item_id).map(String::as_str)
            })
            || self.completed_item_ids.contains(item_id)
        {
            return Err(stream_protocol_error());
        }
        self.finish_function_call(
            &call_id,
            required_string(event, "arguments", stream_protocol_error())?,
            events,
        )
    }

    fn handle_content_part_done(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        let field = event_part_field(event);
        let index = native::part_index(
            event,
            if field == "summary" {
                "summary_index"
            } else {
                "content_index"
            },
        )?;
        if !self
            .active_content_parts
            .remove(&(item_id.to_owned(), field.to_owned(), index))
        {
            return Err(stream_protocol_error());
        }
        let part = required_object(event, "part", stream_protocol_error())?;
        native::validate_part(part)?;
        if required_string(part, "type", stream_protocol_error())?
            != self.part_kind(item_id, field)?
        {
            return Err(stream_protocol_error());
        }
        self.finish_part(
            item_id,
            field,
            index,
            required_string(part, "text", stream_protocol_error())?,
            events,
        )?;
        self.part_snapshots.insert(
            (item_id.to_owned(), field.to_owned(), index),
            Value::Object(part.clone()),
        );
        Ok(())
    }

    fn handle_output_item_done(
        &mut self,
        item: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let completed = normalized_completed_item(item)?;
        let completed = completed.as_object().ok_or_else(stream_protocol_error)?;
        let metadata = item_metadata(completed, true)?;
        native::confirm_part_snapshots(completed, &self.part_snapshots)?;
        native::confirm_annotations(completed, &self.annotations_by_part)?;
        let item_id = required_identifier(item, "id", stream_protocol_error())?;
        let Some(kind) = self.item_kinds.get(item_id).copied() else {
            return Err(stream_protocol_error());
        };
        if self.completed_item_ids.contains(item_id)
            || self
                .active_content_parts
                .iter()
                .any(|(id, _, _)| id == item_id)
            || self
                .item_phases
                .get(item_id)
                .is_some_and(|phase| item.get("phase") != Some(phase))
            || required_string(item, "status", stream_protocol_error())? != "completed"
        {
            return Err(stream_protocol_error());
        }
        match kind {
            OutputItemKind::Message => {
                if required_string(item, "type", stream_protocol_error())? != "message"
                    || required_string(item, "role", stream_protocol_error())? != "assistant"
                {
                    return Err(stream_protocol_error());
                }
                self.finish_item_parts(item_id, item, events)?;
            }
            OutputItemKind::Reasoning => {
                if required_string(item, "type", stream_protocol_error())? != "reasoning" {
                    return Err(stream_protocol_error());
                }
                self.finish_item_parts(item_id, item, events)?;
            }
            OutputItemKind::FunctionCall => {
                if required_string(item, "type", stream_protocol_error())? != "function_call" {
                    return Err(stream_protocol_error());
                }
                let call_id = required_identifier(item, "call_id", stream_protocol_error())?;
                let name = required_identifier(item, "name", stream_protocol_error())?;
                if self.function_call_ids.get(item_id).map(String::as_str) != Some(call_id)
                    || self.function_call_names.get(item_id).map(String::as_str) != Some(name)
                {
                    return Err(stream_protocol_error());
                }
                self.finish_function_call(
                    call_id,
                    required_string(item, "arguments", stream_protocol_error())?,
                    events,
                )?;
            }
        }
        self.emit(events, CanonicalEvent::OutputItemEnd(metadata))?;
        self.completed_items
            .insert(item_id.to_owned(), normalized_completed_item(item)?);
        self.completed_item_ids.insert(item_id.to_owned());
        Ok(())
    }

    fn handle_response_completed(
        &mut self,
        response: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        self.require_matching_response(response)?;
        if required_string(response, "status", stream_protocol_error())? != "completed" {
            return Err(stream_protocol_error());
        }
        let output = required_array(response, "output", stream_protocol_error())?;
        let mut completed_ids = BTreeSet::new();
        for (index, item) in output.iter().enumerate() {
            let item = item.as_object().ok_or_else(stream_protocol_error)?;
            let item_id = required_identifier(item, "id", stream_protocol_error())?;
            if self.item_order.get(index).map(String::as_str) != Some(item_id)
                || !completed_ids.insert(item_id.to_owned())
                || !self.completed_item_ids.contains(item_id)
                // Reviewed sparse terminals may confirm only an already completed item ID.
                // Any supplied snapshot data must exactly confirm the observed item.
                || (item.len() > 1 && self.completed_items.get(item_id) != Some(&normalized_completed_item(item)?))
            {
                return Err(stream_protocol_error());
            }
        }
        if completed_ids != self.completed_item_ids {
            return Err(stream_protocol_error());
        }
        if let Some(usage) = parse_usage(response, &stream_protocol_error())? {
            self.emit(
                events,
                CanonicalEvent::UsageDelta(UsageDelta {
                    usage,
                    is_final: true,
                    extensions: RawExtensions::default(),
                }),
            )?;
        }
        if self.message_open {
            self.emit(
                events,
                CanonicalEvent::MessageEnd(MessageEnd {
                    extensions: RawExtensions::default(),
                }),
            )?;
            self.message_open = false;
        }
        self.emit(
            events,
            CanonicalEvent::ResponseEnd(ResponseEnd {
                stop_reason: Some(
                    if self.completed_function_calls.is_empty() {
                        "end_turn"
                    } else {
                        "tool_use"
                    }
                    .to_owned(),
                ),
                ..ResponseEnd::default()
            }),
        )
    }

    fn handle_response_failed(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let response = required_object(event, "response", stream_protocol_error())?;
        self.require_matching_response(response)?;
        self.emit(
            events,
            CanonicalEvent::StreamError(StreamError {
                error: GatewayError::new(GatewayErrorCode::ProviderPermanent, ErrorScope::Provider),
            }),
        )
    }

    fn append_part(
        &mut self,
        id: &str,
        field: &str,
        index: usize,
        delta: &str,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        self.part_kind(id, field)?;
        if self.completed_item_ids.contains(id) || index >= 64 {
            return Err(stream_protocol_error());
        }
        let retained: usize = self.parts_by_item.values().map(String::len).sum();
        if retained.saturating_add(delta.len()) > MAX_GROK_OFFICIAL_NON_STREAMING_RESPONSE_BYTES {
            return Err(stream_protocol_error());
        }
        if delta.is_empty() {
            return Ok(());
        }
        let extensions = protocol_openai_responses::native_part_extensions(id, field, index)?;
        let event = if self.item_kinds.get(id) == Some(&OutputItemKind::Message) {
            CanonicalEvent::TextDelta(TextDelta {
                text: delta.to_owned(),
                extensions,
            })
        } else {
            CanonicalEvent::ReasoningDelta(ReasoningDelta {
                text: delta.to_owned(),
                extensions,
            })
        };
        self.emit(events, event)?;
        self.parts_by_item
            .entry((id.to_owned(), field.to_owned(), index))
            .or_default()
            .push_str(delta);
        Ok(())
    }

    fn part_kind(&self, id: &str, field: &str) -> Result<&'static str, GatewayError> {
        match (self.item_kinds.get(id), field) {
            (Some(OutputItemKind::Message), "content") => Ok("output_text"),
            (Some(OutputItemKind::Reasoning), "content") => Ok("reasoning_text"),
            (Some(OutputItemKind::Reasoning), "summary") => Ok("summary_text"),
            _ => Err(stream_protocol_error()),
        }
    }

    fn finish_part(
        &mut self,
        id: &str,
        field: &str,
        index: usize,
        text: &str,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let key = (id.to_owned(), field.to_owned(), index);
        let emitted = self.parts_by_item.get(&key).map_or("", String::as_str);
        let suffix = text
            .strip_prefix(emitted)
            .ok_or_else(stream_protocol_error)?
            .to_owned();
        self.append_part(id, field, index, &suffix, events)
    }

    fn finish_item_parts(
        &mut self,
        id: &str,
        item: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        for field in ["content", "summary"] {
            if let Some(parts) = item.get(field) {
                for (index, part) in parts
                    .as_array()
                    .ok_or_else(stream_protocol_error)?
                    .iter()
                    .enumerate()
                {
                    self.finish_part(
                        id,
                        field,
                        index,
                        part.get("text")
                            .and_then(Value::as_str)
                            .ok_or_else(stream_protocol_error)?,
                        events,
                    )?;
                }
            }
            for ((item_id, part_field, index), text) in &self.parts_by_item {
                if item_id == id
                    && part_field == field
                    && item
                        .get(field)
                        .and_then(Value::as_array)
                        .and_then(|parts| parts.get(*index))
                        .and_then(|part| part.get("text"))
                        .and_then(Value::as_str)
                        != Some(text.as_str())
                {
                    return Err(stream_protocol_error());
                }
            }
        }
        Ok(())
    }

    fn finish_function_call(
        &mut self,
        call_id: &str,
        arguments: &str,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let arguments = normalize_tool_arguments(arguments)?;
        let emitted = self
            .function_arguments
            .get(call_id)
            .map(String::as_str)
            .map(normalize_tool_arguments)
            .transpose()?;
        if emitted
            .as_ref()
            .is_some_and(|emitted| emitted != &arguments)
        {
            return Err(stream_protocol_error());
        }
        if self.completed_function_calls.contains(call_id) {
            return if emitted.as_deref() == Some(arguments.as_str()) {
                Ok(())
            } else {
                Err(stream_protocol_error())
            };
        }
        if !self.emitted_function_arguments.contains(call_id) {
            self.emit(
                events,
                CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
                    call_id: call_id.to_owned(),
                    delta: arguments.clone(),
                    extensions: RawExtensions::default(),
                }),
            )?;
            self.emitted_function_arguments.insert(call_id.to_owned());
        }
        self.emit(
            events,
            CanonicalEvent::ToolCallEnd(ToolCallEnd {
                call_id: call_id.to_owned(),
                arguments: RawJson::from_json_string(arguments.clone())
                    .map_err(|_| stream_protocol_error())?,
                extensions: RawExtensions::default(),
            }),
        )?;
        self.function_arguments
            .insert(call_id.to_owned(), arguments);
        self.completed_function_calls.insert(call_id.to_owned());
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

    fn require_response_started(&self) -> Result<(), GatewayError> {
        if self.response_id.is_some() {
            Ok(())
        } else {
            Err(stream_protocol_error())
        }
    }

    fn require_matching_response(&self, response: &Map<String, Value>) -> Result<(), GatewayError> {
        let response_id = required_identifier(response, "id", stream_protocol_error())?;
        if self.response_id.as_deref() == Some(response_id) {
            Ok(())
        } else {
            Err(stream_protocol_error())
        }
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

fn normalized_completed_item(item: &Map<String, Value>) -> Result<Value, GatewayError> {
    let mut completed = item.clone();
    if item.get("type").and_then(Value::as_str) == Some("function_call") {
        completed.insert(
            "arguments".into(),
            Value::String(normalize_tool_arguments(required_string(
                item,
                "arguments",
                stream_protocol_error(),
            )?)?),
        );
    }
    Ok(Value::Object(completed))
}

fn item_metadata(
    item: &Map<String, Value>,
    completed: bool,
) -> Result<gateway_core::OutputItemMetadata, GatewayError> {
    // An owned-token prefix alone is not authentication. Official/Console have no
    // ciphertext owner, so they cannot borrow the separate Build ownership path.
    if item
        .get("encrypted_content")
        .is_some_and(|value| !value.is_null())
    {
        return Err(stream_protocol_error());
    }
    protocol_openai_responses::native_item_metadata(&Value::Object(item.clone()), completed)
}

fn event_part_field(event: &Map<String, Value>) -> &'static str {
    if event
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind.contains("summary"))
    {
        "summary"
    } else {
        "content"
    }
}

fn normalize_tool_arguments(arguments: &str) -> Result<String, GatewayError> {
    if arguments.len() > MAX_GROK_OFFICIAL_TOOL_ARGUMENT_BYTES {
        return Err(stream_protocol_error());
    }
    if arguments.trim().is_empty() {
        return Ok("{}".to_owned());
    }
    let value = parse_strict_json(arguments.as_bytes(), MAX_GROK_OFFICIAL_TOOL_ARGUMENT_BYTES)
        .map_err(|()| stream_protocol_error())?;
    match value {
        Value::Object(object) if object.is_empty() => Ok("{}".to_owned()),
        Value::Object(_) => Ok(arguments.to_owned()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            Err(stream_protocol_error())
        }
    }
}

fn parse_usage(
    response: &Map<String, Value>,
    error: &GatewayError,
) -> Result<Option<Usage>, GatewayError> {
    let Some(value) = response.get("usage") else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let object = value.as_object().ok_or_else(|| error.clone())?;
    let input_tokens = optional_u64(object, "input_tokens", error)?;
    let output_tokens = optional_u64(object, "output_tokens", error)?;
    let reasoning_tokens = object
        .get("output_tokens_details")
        .filter(|value| !value.is_null())
        .map(|value| {
            let details = value.as_object().ok_or_else(|| error.clone())?;
            optional_u64(details, "reasoning_tokens", error)
        })
        .transpose()?
        .flatten();
    let cached_tokens = object
        .get("input_tokens_details")
        .filter(|value| !value.is_null())
        .map(|value| {
            let details = value.as_object().ok_or_else(|| error.clone())?;
            optional_u64(details, "cached_tokens", error)
        })
        .transpose()?
        .flatten();
    if input_tokens.is_none()
        && output_tokens.is_none()
        && reasoning_tokens.is_none()
        && cached_tokens.is_none()
    {
        return Ok(None);
    }
    Ok(Some(Usage {
        provenance: gateway_core::UsageProvenance::Measured,
        input_accounting: gateway_core::InputTokenAccounting::Inclusive,
        input_tokens,
        output_tokens,
        reasoning_tokens,
        cached_tokens,
        ..Usage::default()
    }))
}

fn optional_u64(
    object: &Map<String, Value>,
    field: &str,
    error: &GatewayError,
) -> Result<Option<u64>, GatewayError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| error.clone()),
    }
}

fn required_object<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    error: GatewayError,
) -> Result<&'a Map<String, Value>, GatewayError> {
    object.get(field).and_then(Value::as_object).ok_or(error)
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    error: GatewayError,
) -> Result<&'a Vec<Value>, GatewayError> {
    object.get(field).and_then(Value::as_array).ok_or(error)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    error: GatewayError,
) -> Result<&'a str, GatewayError> {
    object.get(field).and_then(Value::as_str).ok_or(error)
}

fn required_identifier<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    error: GatewayError,
) -> Result<&'a str, GatewayError> {
    let value = required_string(object, field, error.clone())?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(error);
    }
    Ok(value)
}

fn sse_delimiter_length(pending: &[u8]) -> Option<usize> {
    if pending.ends_with(b"\n\n") {
        Some(2)
    } else if pending.ends_with(b"\r\n\r\n") {
        Some(4)
    } else {
        None
    }
}

/// The selected Official execution representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokOfficialExecutionMode {
    /// Request one completed JSON Responses object.
    NonStreaming,
    /// Request and incrementally decode one SSE Responses stream.
    Streaming,
}

impl GrokOfficialExecutionMode {
    const fn response_mode(self) -> ResponseMode {
        match self {
            Self::NonStreaming => ResponseMode::NonStreaming,
            Self::Streaming => ResponseMode::Streaming,
        }
    }
}

/// Safe classification of an Official upstream response content type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokOfficialResponseContentType {
    /// A JSON Responses representation.
    Json,
    /// An SSE Responses representation.
    EventStream,
    /// Missing, malformed, or unsupported content type.
    OtherOrMissing,
}

/// Pull-only opaque body received from a pre-approved Official transport.
pub trait GrokOfficialResponseBody: Send {
    /// Returns the next body chunk or normal end of the response.
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>>;
}

/// A status and content-type projection plus a pull-only raw Official response body.
pub struct GrokOfficialTransportResponse {
    status: u16,
    content_type: GrokOfficialResponseContentType,
    rate_limit: GrokOfficialRateLimitMetadata,
    body: Box<dyn GrokOfficialResponseBody>,
}

impl GrokOfficialTransportResponse {
    /// Creates an opaque injected or production response handoff.
    #[must_use]
    pub fn new(
        status: u16,
        content_type: GrokOfficialResponseContentType,
        body: Box<dyn GrokOfficialResponseBody>,
    ) -> Self {
        Self {
            status,
            content_type,
            rate_limit: GrokOfficialRateLimitMetadata::default(),
            body,
        }
    }

    /// Attaches the separately parsed fixed-header rate-limit observation.
    #[must_use]
    pub fn with_rate_limit_metadata(mut self, rate_limit: GrokOfficialRateLimitMetadata) -> Self {
        self.rate_limit = rate_limit;
        self
    }

    /// Returns the safe fixed-header observation without retaining raw Header material.
    #[must_use]
    pub fn rate_limit_metadata(&self) -> &GrokOfficialRateLimitMetadata {
        &self.rate_limit
    }

    fn into_parts(
        self,
    ) -> (
        u16,
        GrokOfficialResponseContentType,
        GrokOfficialRateLimitMetadata,
        Box<dyn GrokOfficialResponseBody>,
    ) {
        (self.status, self.content_type, self.rate_limit, self.body)
    }
}

impl fmt::Debug for GrokOfficialTransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialTransportResponse")
            .field("status", &self.status)
            .field("content_type", &self.content_type)
            .field("rate_limit", &self.rate_limit)
            .field("body", &"<streaming>")
            .finish()
    }
}

/// Sends an already-built Official request through a caller-controlled transport boundary.
pub trait GrokOfficialTransport: Send + Sync {
    /// Sends exactly one request without retries, key rotation, failover, or implicit scheduling.
    fn send(
        &self,
        request: GrokOfficialResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokOfficialTransportResponse, GatewayError>>;
}

/// Production Official transport using the shared DNS-pinned upstream client only after admission.
pub struct GrokOfficialUpstreamTransport {
    egress_policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    client_pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
}

impl GrokOfficialUpstreamTransport {
    /// Creates a production transport from explicit egress policy, resolver, pool, and profile.
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
            profile,
        }
    }
}

impl fmt::Debug for GrokOfficialUpstreamTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialUpstreamTransport")
            .field("egress_policy", self.egress_policy.id())
            .field("resolver", &"<injected>")
            .field("client_pool", &self.client_pool)
            .field("profile", &self.profile)
            .finish()
    }
}

impl GrokOfficialTransport for GrokOfficialUpstreamTransport {
    fn send(
        &self,
        outbound: GrokOfficialResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokOfficialTransportResponse, GatewayError>> {
        let admitted = self
            .egress_policy
            .admit_url(outbound.url(), self.resolver.as_ref())
            .map_err(gateway_upstream::EgressAdmissionError::gateway_error);
        let request = admitted.and_then(|target| outbound.into_transport_request(target));
        let pool = self.client_pool.clone();
        let profile = self.profile.clone();

        Box::pin(async move {
            let response = pool.send(request?, &profile).await?;
            let rate_limit = rate_limit_metadata(&response)?;
            Ok(GrokOfficialTransportResponse::new(
                response.status(),
                content_type(&response),
                Box::new(UpstreamResponseBody { response }),
            )
            .with_rate_limit_metadata(rate_limit))
        })
    }
}

/// Native Official [`InferenceAdapter`] with API-key and transport state supplied explicitly.
#[derive(Clone)]
pub struct GrokOfficialInferenceAdapter {
    provider_id: gateway_core::ProviderId,
    credential: GrokOfficialApiKey,
    upstream_model: String,
    mode: GrokOfficialExecutionMode,
    transport: Arc<dyn GrokOfficialTransport>,
    runtime_state: Option<GrokOfficialRuntimeState>,
}

impl GrokOfficialInferenceAdapter {
    /// Builds one adapter for one selected Official API key, model, and execution mode.
    ///
    /// The builder revalidates the selected model and all Canonical representability. No HTTP,
    /// environment lookup, proxy discovery, status policy, retry, or quota mutation occurs here.
    ///
    /// # Errors
    ///
    /// Returns `ClientRequestError/Request` for a blank, overlong, or header-unsafe selected model,
    /// and `InternalError/Internal` only if the compiled stable Provider ID becomes invalid.
    pub fn try_new(
        credential: GrokOfficialApiKey,
        upstream_model: impl Into<String>,
        mode: GrokOfficialExecutionMode,
        transport: Arc<dyn GrokOfficialTransport>,
    ) -> Result<Self, GatewayError> {
        Self::try_new_inner(credential, upstream_model, mode, transport, None)
    }

    /// Builds one Official adapter that applies safe runtime observations to its explicit state.
    ///
    /// The state object fixes the Official Endpoint/Credential quota target and cannot carry
    /// Build/Web state. A response is observed exactly once after transport headers are available.
    ///
    /// # Errors
    ///
    /// Returns the same selected-model/provider-identity errors as [`Self::try_new`].
    pub fn try_new_with_runtime_state(
        credential: GrokOfficialApiKey,
        upstream_model: impl Into<String>,
        mode: GrokOfficialExecutionMode,
        transport: Arc<dyn GrokOfficialTransport>,
        runtime_state: GrokOfficialRuntimeState,
    ) -> Result<Self, GatewayError> {
        Self::try_new_inner(
            credential,
            upstream_model,
            mode,
            transport,
            Some(runtime_state),
        )
    }

    fn try_new_inner(
        credential: GrokOfficialApiKey,
        upstream_model: impl Into<String>,
        mode: GrokOfficialExecutionMode,
        transport: Arc<dyn GrokOfficialTransport>,
        runtime_state: Option<GrokOfficialRuntimeState>,
    ) -> Result<Self, GatewayError> {
        let upstream_model = upstream_model.into();
        if upstream_model.is_empty()
            || upstream_model.len() > MAX_GROK_OFFICIAL_MODEL_BYTES
            || !upstream_model.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(client_request_error());
        }
        let provider_id = gateway_core::ProviderId::try_new(GROK_OFFICIAL_PROVIDER_ID.to_owned())
            .map_err(|_| internal_error())?;
        Ok(Self {
            provider_id,
            credential,
            upstream_model,
            mode,
            transport,
            runtime_state,
        })
    }
}

impl fmt::Debug for GrokOfficialInferenceAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialInferenceAdapter")
            .field("provider_id", &self.provider_id)
            .field("credential", &self.credential)
            .field("upstream_model", &"<redacted>")
            .field("mode", &self.mode)
            .field("transport", &"<injected>")
            .field("runtime_state", &self.runtime_state.is_some())
            .finish()
    }
}

impl ProviderAdapter for GrokOfficialInferenceAdapter {
    fn provider_id(&self) -> &gateway_core::ProviderId {
        &self.provider_id
    }
}

impl InferenceAdapter for GrokOfficialInferenceAdapter {
    fn execute(
        &self,
        _context: RequestContext,
        request: CanonicalRequest,
    ) -> ProviderFuture<'_, Result<Box<dyn CanonicalEventSource>, GatewayError>> {
        let credential = self.credential.clone();
        let upstream_model = self.upstream_model.clone();
        let mode = self.mode;
        let transport = Arc::clone(&self.transport);
        let runtime_state = self.runtime_state.clone();

        Box::pin(async move {
            let outbound = GrokOfficialResponsesRequestBuilder::build(
                &credential,
                &upstream_model,
                &request,
                mode.response_mode(),
            )?;
            let response = transport.send(outbound).await?;
            let (status, content_type, rate_limit, mut body) = response.into_parts();
            if let Some(runtime_state) = runtime_state
                && let Some(disposition) = runtime_state
                    .observe_transport_response(status, &rate_limit)
                    .map_err(|_| internal_error())?
            {
                let _ = read_bounded_body(&mut *body, MAX_GROK_OFFICIAL_ERROR_BODY_BYTES).await?;
                return Err(disposition.error().clone());
            }
            if !(200..=299).contains(&status) {
                // Consume only a bounded amount, then retain the status-only P8 classification.
                // Raw body content never changes ownership or reaches a diagnostic.
                let _ = read_bounded_body(&mut *body, MAX_GROK_OFFICIAL_ERROR_BODY_BYTES).await?;
                return Err(classify_grok_official_http_failure(status).error().clone());
            }
            match mode {
                GrokOfficialExecutionMode::NonStreaming
                    if content_type == GrokOfficialResponseContentType::Json =>
                {
                    let bytes = read_bounded_body(
                        &mut *body,
                        MAX_GROK_OFFICIAL_NON_STREAMING_RESPONSE_BYTES,
                    )
                    .await?;
                    let decoded = GrokOfficialResponsesDecoder::decode_non_streaming(&bytes)?;
                    Ok(Box::new(BufferedEventSource::new(decoded.into_events()))
                        as Box<dyn CanonicalEventSource>)
                }
                GrokOfficialExecutionMode::Streaming
                    if content_type == GrokOfficialResponseContentType::EventStream =>
                {
                    Ok(Box::new(StreamingEventSource::new(body)) as Box<dyn CanonicalEventSource>)
                }
                _ => Err(provider_protocol_error()),
            }
        })
    }
}

struct UpstreamResponseBody {
    response: UpstreamHttpResponse,
}

impl GrokOfficialResponseBody for UpstreamResponseBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            self.response
                .next_chunk()
                .await
                .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
        })
    }
}

struct BufferedEventSource {
    events: VecDeque<CanonicalEvent>,
}

impl BufferedEventSource {
    fn new(events: Vec<CanonicalEvent>) -> Self {
        Self {
            events: events.into(),
        }
    }
}

impl CanonicalEventSource for BufferedEventSource {
    fn next_event(&mut self) -> ProviderFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move { Ok(self.events.pop_front()) })
    }
}

struct StreamingEventSource {
    body: Box<dyn GrokOfficialResponseBody>,
    decoder: GrokOfficialResponsesStreamDecoder,
    pending: VecDeque<CanonicalEvent>,
    response_started: bool,
    terminal_failure_emitted: bool,
    finished: bool,
}

impl StreamingEventSource {
    fn new(body: Box<dyn GrokOfficialResponseBody>) -> Self {
        Self {
            body,
            decoder: GrokOfficialResponsesStreamDecoder::new(),
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

impl CanonicalEventSource for StreamingEventSource {
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
                            Ok(()) => Ok(None),
                            Err(error) => self.terminal_failure(error),
                        };
                    }
                    Err(error) => return self.terminal_failure(error),
                }
            }
        })
    }
}

async fn read_bounded_body(
    body: &mut dyn GrokOfficialResponseBody,
    maximum_bytes: usize,
) -> Result<Vec<u8>, GatewayError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next_chunk().await? {
        let length = bytes
            .len()
            .checked_add(chunk.len())
            .ok_or_else(provider_protocol_error)?;
        if length > maximum_bytes {
            return Err(provider_protocol_error());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn content_type(response: &UpstreamHttpResponse) -> GrokOfficialResponseContentType {
    match response
        .header("content-type")
        .and_then(|value| value.to_str().ok())
    {
        Some(value) if value.starts_with("application/json") => {
            GrokOfficialResponseContentType::Json
        }
        Some(value) if value.starts_with("text/event-stream") => {
            GrokOfficialResponseContentType::EventStream
        }
        _ => GrokOfficialResponseContentType::OtherOrMissing,
    }
}

fn rate_limit_metadata(
    response: &UpstreamHttpResponse,
) -> Result<GrokOfficialRateLimitMetadata, GatewayError> {
    const NAMES: [&str; 7] = [
        "x-ratelimit-limit-requests",
        "x-ratelimit-remaining-requests",
        "x-ratelimit-reset-requests",
        "x-ratelimit-limit-tokens",
        "x-ratelimit-remaining-tokens",
        "x-ratelimit-reset-tokens",
        "retry-after",
    ];
    let mut headers = Vec::new();
    for name in NAMES {
        for value in response.header_values(name) {
            let value = value.to_str().map_err(|_| provider_protocol_error())?;
            headers.push((name, value));
        }
    }
    GrokOfficialRateLimitMetadata::parse(headers)
}

const fn egress_rejected_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
}

const fn client_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

const fn provider_protocol_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        ErrorScope::Provider,
    )
}

const fn stream_protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

const fn stream_truncated_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::StreamTruncated, ErrorScope::Stream)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}
