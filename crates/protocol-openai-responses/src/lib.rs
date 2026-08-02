//! Pure `OpenAI` Responses request, response, and Server-Sent Events codec.
//!
//! This crate deliberately has no HTTP, Provider, routing, or bounded-delivery dependency. A
//! later transport owns writing encoded frames and commits first-semantic-event delivery only after
//! a write reaches the client.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

mod upstream_response;

use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalMessage, CanonicalRequest, CanonicalResponse,
    ErrorScope, GatewayError, GatewayErrorCode, MessageContent, MessageRole, OpaqueContent,
    RawExtensions, RawJson, TextContent, Thinking, ThinkingEffort, ToolCall, ToolDefinition,
    ToolResult,
};
use serde::{Deserialize, de};
use serde_json::{Map, Value, json};

pub use upstream_response::{OpenAiResponsesSseDecoder, decode_upstream_response};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "protocol-openai-responses";

/// The output mode requested by the `stream` field of a Responses request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseMode {
    /// Return one completed JSON response object.
    NonStreaming,
    /// Return typed Server-Sent Event frames.
    Streaming,
}

/// The result of decoding a Responses request without coupling it to HTTP.
#[derive(Clone, Eq, PartialEq)]
pub struct DecodedResponsesRequest {
    /// Protocol-neutral request data for later routing and Provider execution.
    pub request: CanonicalRequest,
    /// Requested response representation.
    pub mode: ResponseMode,
}

impl fmt::Debug for DecodedResponsesRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecodedResponsesRequest")
            .field("request", &self.request)
            .field("mode", &self.mode)
            .finish()
    }
}

/// Decodes one complete `OpenAI` Responses JSON request into canonical request data.
///
/// # Errors
///
/// Returns `ClientRequestError/Request` for malformed, ambiguous, or unsupported request data.
/// Fields that do not yet have a canonical meaning are retained under explicit raw extensions;
/// they are never silently discarded.
pub fn decode_request(input: &str) -> Result<DecodedResponsesRequest, GatewayError> {
    reject_duplicate_json_names(input)?;
    let value: Value = serde_json::from_str(input).map_err(|_| client_request_error())?;
    let root = object(&value)?;
    reject_unimplemented_execution_controls(root)?;

    let requested_model = required_string(root, "model")?.to_owned();
    if requested_model.is_empty() {
        return Err(client_request_error());
    }

    let mode = match root.get("stream") {
        Some(Value::Bool(true)) => ResponseMode::Streaming,
        None | Some(Value::Bool(false)) => ResponseMode::NonStreaming,
        Some(_) => return Err(client_request_error()),
    };

    let mut messages = decode_instructions(root)?;
    if let Some(input) = root.get("input") {
        messages.extend(decode_input(input)?);
    }

    let tools = match root.get("tools") {
        None => Vec::new(),
        Some(value) => decode_tools(value)?,
    };
    if root.get("tool_choice").and_then(Value::as_str) == Some("required") && tools.is_empty() {
        return Err(client_request_error());
    }

    let thinking = match root.get("reasoning") {
        None => None,
        Some(value) => Some(decode_reasoning(value)?),
    };

    let prompt_cache_key = optional_string(root, "prompt_cache_key")?;
    let prompt_cache_retention = optional_string(root, "prompt_cache_retention")?;
    let extensions = extensions_except_with_prefix(
        root,
        &[
            "model",
            "stream",
            "input",
            "instructions",
            "tools",
            "reasoning",
            "prompt_cache_key",
            "prompt_cache_retention",
        ],
        "openai.responses.",
    )?;

    Ok(DecodedResponsesRequest {
        request: CanonicalRequest {
            requested_model,
            messages,
            tools,
            thinking,
            prompt_cache_key,
            prompt_cache_retention,
            extensions,
        },
        mode,
    })
}

fn decode_instructions(root: &Map<String, Value>) -> Result<Vec<CanonicalMessage>, GatewayError> {
    let Some(value) = root.get("instructions") else {
        return Ok(Vec::new());
    };
    let instructions = string(value)?.to_owned();

    Ok(vec![CanonicalMessage {
        role: MessageRole("developer".to_owned()),
        content: vec![MessageContent::Text(TextContent {
            text: instructions,
            extensions: RawExtensions::default(),
        })],
        extensions: RawExtensions::default(),
    }])
}

fn decode_input(input: &Value) -> Result<Vec<CanonicalMessage>, GatewayError> {
    match input {
        Value::String(text) => Ok(vec![CanonicalMessage {
            role: MessageRole("user".to_owned()),
            content: vec![MessageContent::Text(TextContent {
                text: text.clone(),
                extensions: RawExtensions::default(),
            })],
            extensions: RawExtensions::default(),
        }]),
        Value::Array(items) => items.iter().map(decode_input_item).collect(),
        _ => Err(client_request_error()),
    }
}

fn decode_input_item(item: &Value) -> Result<CanonicalMessage, GatewayError> {
    let item = object(item)?;
    let item_type = item.get("type").and_then(Value::as_str);

    match item_type {
        Some("function_call") => decode_function_call(item),
        Some("function_call_output") => decode_function_call_output(item),
        Some("message") | None if item.contains_key("role") => decode_message_item(item),
        _ => Err(client_request_error()),
    }
}

fn decode_message_item(item: &Map<String, Value>) -> Result<CanonicalMessage, GatewayError> {
    let role = required_string(item, "role")?.to_owned();
    if !matches!(role.as_str(), "user" | "developer" | "system" | "assistant") {
        return Err(client_request_error());
    }
    let content = required_value(item, "content")?;
    let content = decode_message_content(content)?;

    Ok(CanonicalMessage {
        role: MessageRole(role),
        content,
        extensions: extensions_except(item, &["type", "role", "content"])?,
    })
}

fn decode_message_content(value: &Value) -> Result<Vec<MessageContent>, GatewayError> {
    match value {
        Value::String(text) => Ok(vec![MessageContent::Text(TextContent {
            text: text.clone(),
            extensions: RawExtensions::default(),
        })]),
        Value::Array(parts) => parts.iter().map(decode_content_part).collect(),
        _ => Err(client_request_error()),
    }
}

fn decode_content_part(part: &Value) -> Result<MessageContent, GatewayError> {
    let object = object(part)?;
    match object.get("type").and_then(Value::as_str) {
        Some("input_text" | "output_text") => {
            let text = required_string(object, "text")?.to_owned();
            Ok(MessageContent::Text(TextContent {
                text,
                extensions: extensions_except(object, &["type", "text"])?,
            }))
        }
        Some(_) => Ok(MessageContent::Opaque(OpaqueContent::new(raw_json(part)?))),
        None => Err(client_request_error()),
    }
}

fn decode_function_call(item: &Map<String, Value>) -> Result<CanonicalMessage, GatewayError> {
    let id = required_string(item, "call_id")?.to_owned();
    let name = required_string(item, "name")?.to_owned();
    let arguments = required_string(item, "arguments")?.to_owned();
    if id.is_empty() || name.is_empty() {
        return Err(client_request_error());
    }
    // `arguments` is JSON encoded inside a JSON string, so the request-wide parser cannot see
    // duplicate members in it. Validate this second JSON document before retaining it raw.
    reject_duplicate_json_names(&arguments)?;
    let arguments = RawJson::from_json_string(arguments).map_err(|_| client_request_error())?;

    Ok(CanonicalMessage {
        role: MessageRole("assistant".to_owned()),
        content: vec![MessageContent::ToolCall(ToolCall {
            id,
            name,
            arguments,
            extensions: extensions_except(item, &["type", "call_id", "name", "arguments"])?,
        })],
        extensions: RawExtensions::default(),
    })
}

fn decode_function_call_output(
    item: &Map<String, Value>,
) -> Result<CanonicalMessage, GatewayError> {
    let call_id = required_string(item, "call_id")?.to_owned();
    if call_id.is_empty() {
        return Err(client_request_error());
    }
    let output = raw_json(required_value(item, "output")?)?;
    if item.contains_key("status") {
        return Err(client_request_error());
    }

    Ok(CanonicalMessage {
        role: MessageRole("tool".to_owned()),
        content: vec![MessageContent::ToolResult(ToolResult {
            call_id,
            output,
            is_error: false,
            extensions: extensions_except(item, &["type", "call_id", "output"])?,
        })],
        extensions: RawExtensions::default(),
    })
}

fn decode_tools(value: &Value) -> Result<Vec<ToolDefinition>, GatewayError> {
    let tools = array(value)?;
    tools.iter().map(decode_tool).collect()
}

fn decode_tool(tool: &Value) -> Result<ToolDefinition, GatewayError> {
    let tool = object(tool)?;
    if tool.get("type").and_then(Value::as_str) != Some("function") {
        return Err(client_request_error());
    }
    let name = required_string(tool, "name")?.to_owned();
    if name.is_empty() {
        return Err(client_request_error());
    }
    let description = match tool.get("description") {
        None | Some(Value::Null) => None,
        Some(value) => Some(string(value)?.to_owned()),
    };
    let input_schema = match tool.get("parameters") {
        None => RawJson::from_json_string("{}".to_owned()).map_err(|_| internal_error())?,
        Some(Value::Object(_)) => raw_json(tool.get("parameters").ok_or_else(internal_error)?)?,
        Some(_) => return Err(client_request_error()),
    };

    Ok(ToolDefinition {
        name,
        description,
        input_schema,
        extensions: extensions_except(tool, &["type", "name", "description", "parameters"])?,
    })
}

fn decode_reasoning(value: &Value) -> Result<Thinking, GatewayError> {
    let reasoning = object(value)?;
    let effort = ThinkingEffort::try_new(required_string(reasoning, "effort")?.to_owned())
        .map_err(|_| client_request_error())?;

    Ok(Thinking {
        effort,
        extensions: extensions_except(reasoning, &["effort"])?,
    })
}

fn optional_string(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Option<String>, GatewayError> {
    object
        .get(name)
        .map(|value| string(value).map(str::to_owned))
        .transpose()
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a Value, GatewayError> {
    object.get(name).ok_or_else(client_request_error)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, GatewayError> {
    string(required_value(object, name)?)
}

fn string(value: &Value) -> Result<&str, GatewayError> {
    value.as_str().ok_or_else(client_request_error)
}

fn object(value: &Value) -> Result<&Map<String, Value>, GatewayError> {
    value.as_object().ok_or_else(client_request_error)
}

fn array(value: &Value) -> Result<&Vec<Value>, GatewayError> {
    value.as_array().ok_or_else(client_request_error)
}

fn raw_json(value: &Value) -> Result<RawJson, GatewayError> {
    let serialized = serde_json::to_string(value).map_err(|_| internal_error())?;
    RawJson::from_json_string(serialized).map_err(|_| internal_error())
}

fn extensions_except(
    object: &Map<String, Value>,
    known: &[&str],
) -> Result<RawExtensions, GatewayError> {
    extensions_except_with_prefix(object, known, "")
}

fn extensions_except_with_prefix(
    object: &Map<String, Value>,
    known: &[&str],
    prefix: &str,
) -> Result<RawExtensions, GatewayError> {
    let known: BTreeSet<&str> = known.iter().copied().collect();
    let mut extensions = RawExtensions::default();
    for (name, value) in object {
        if !known.contains(name.as_str()) {
            extensions
                .try_insert(format!("{prefix}{name}"), raw_json(value)?)
                .map_err(|_| client_request_error())?;
        }
    }

    Ok(extensions)
}

fn reject_unimplemented_execution_controls(root: &Map<String, Value>) -> Result<(), GatewayError> {
    for name in [
        "background",
        "store",
        "conversation",
        "previous_response_id",
        "text",
        "top_logprobs",
        "stream_options",
    ] {
        if root.contains_key(name) {
            return Err(client_request_error());
        }
    }

    if let Some(tool_choice) = root.get("tool_choice")
        && !matches!(tool_choice.as_str(), Some("auto" | "required"))
    {
        return Err(client_request_error());
    }
    if let Some(parallel_tool_calls) = root.get("parallel_tool_calls")
        && parallel_tool_calls.as_bool() != Some(true)
    {
        return Err(client_request_error());
    }

    Ok(())
}

/// Validates that every object in a JSON value has unique field names.
///
/// `serde_json::Value` intentionally keeps the final duplicate member, so this pass must happen
/// before conversion to `Value` and before any protocol field is interpreted.
fn reject_duplicate_json_names(input: &str) -> Result<(), GatewayError> {
    serde_json::from_str::<DuplicateFreeJson>(input)
        .map(|_| ())
        .map_err(|_| client_request_error())
}

struct DuplicateFreeJson;

impl<'de> Deserialize<'de> for DuplicateFreeJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateFreeVisitor)
    }
}

struct DuplicateFreeVisitor;

impl<'de> de::Visitor<'de> for DuplicateFreeVisitor {
    type Value = DuplicateFreeJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object member names")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(DuplicateFreeJson)
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        while sequence.next_element::<DuplicateFreeJson>()?.is_some() {}
        Ok(DuplicateFreeJson)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut names = BTreeSet::new();
        while let Some(name) = map.next_key::<String>()? {
            if !names.insert(name) {
                return Err(de::Error::custom("duplicate JSON object member name"));
            }
            let _: DuplicateFreeJson = map.next_value()?;
        }
        Ok(DuplicateFreeJson)
    }
}

const fn client_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

const fn stream_protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

/// Metadata owned by the response boundary rather than by `CanonicalResponse`.
///
/// Canonical events deliberately retain only the opaque response identifier. The public model and
/// creation time are added by the protocol boundary after route selection and before a response is
/// encoded.
#[derive(Clone, Eq, PartialEq)]
pub struct OpenAiResponseMetadata {
    model: String,
    created_at: u64,
}

impl OpenAiResponseMetadata {
    /// Creates response metadata with an explicit public model and Unix timestamp in seconds.
    ///
    /// # Errors
    ///
    /// Returns `InternalError/Internal` if a caller tries to emit an empty public model label.
    pub fn try_new(model: impl Into<String>, created_at: u64) -> Result<Self, GatewayError> {
        let model = model.into();
        if model.is_empty() {
            return Err(internal_error());
        }

        Ok(Self { model, created_at })
    }

    /// Returns the selected public model label.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Returns the response creation time as Unix seconds.
    #[must_use]
    pub const fn created_at(&self) -> u64 {
        self.created_at
    }
}

impl fmt::Debug for OpenAiResponseMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiResponseMetadata")
            .field("model", &"<redacted>")
            .field("created_at", &self.created_at)
            .finish()
    }
}

/// One typed `OpenAI` Responses Server-Sent Event frame.
#[derive(Clone, Eq, PartialEq)]
pub struct SseFrame {
    event: &'static str,
    data: Value,
    semantic: bool,
}

impl SseFrame {
    /// Returns the SSE event name and the matching payload `type` value.
    #[must_use]
    pub const fn event(&self) -> &'static str {
        self.event
    }

    /// Returns the JSON payload without converting it to an HTTP body.
    #[must_use]
    pub fn data(&self) -> &Value {
        &self.data
    }

    /// Returns whether this frame represents client-visible semantic canonical output.
    ///
    /// A later HTTP writer may use this classification to decide whether a successful write crosses
    /// the first-semantic-event boundary. This codec never commits that boundary itself.
    #[must_use]
    pub const fn is_semantic(&self) -> bool {
        self.semantic
    }

    /// Formats this frame using the SSE `event:` and `data:` fields.
    ///
    /// # Errors
    ///
    /// Returns `InternalError/Internal` only if a JSON value cannot be serialized. Adapter-created
    /// values are finite JSON, so this is an invariant guard rather than client input handling.
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
            .field("semantic", &self.semantic)
            .field("data", &"<redacted>")
            .finish()
    }
}

/// Encodes a safe core error using the `OpenAI` Responses error envelope shape.
///
/// HTTP status codes and headers are intentionally not chosen here; P1-07 owns that transport
/// decision before headers are committed.
#[must_use]
pub fn encode_error(error: &GatewayError) -> Value {
    json!({
        "error": {
            "type": openai_error_type(error),
            "code": error.code().as_str(),
            "message": error.safe_message(),
            "param": Value::Null,
        }
    })
}

/// Encodes the public `OpenAI`-compatible `/v1/models` list envelope.
///
/// Callers supply only already-authorized Public Model names from an immutable route view. The
/// output intentionally has no route, Candidate, Endpoint, Upstream, Credential, Catalog, or
/// upstream-model fields. `created` is the stable gateway-owned sentinel because Public Model
/// publication does not expose a per-model creation timestamp on the data path.
///
/// # Errors
///
/// Returns `InternalError/Internal` when a caller attempts to encode an empty Public Model name.
pub fn encode_model_list<I, S>(model_names: I) -> Result<Value, GatewayError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut data = Vec::new();
    for model_name in model_names {
        let model_name = model_name.as_ref();
        if model_name.is_empty() {
            return Err(internal_error());
        }
        data.push(json!({
            "id": model_name,
            "object": "model",
            "created": 0,
            "owned_by": "gateway",
        }));
    }

    Ok(json!({
        "object": "list",
        "data": data,
    }))
}

/// Encodes one validated successful canonical response as a non-streaming Responses object.
///
/// # Errors
///
/// Returns a stream protocol error when a canonical event carries raw extensions that the `OpenAI`
/// Responses output shape cannot represent losslessly, or when an encoder invariant is violated.
pub fn encode_response(
    response: &CanonicalResponse,
    metadata: OpenAiResponseMetadata,
) -> Result<Value, GatewayError> {
    let mut encoder = OpenAiResponsesSseEncoder::new(metadata);
    for event in response.events() {
        let _ = encoder.encode_event(event)?;
    }
    encoder.into_completed_response()
}

/// Stateful Responses SSE encoder for one ordered canonical response source.
pub struct OpenAiResponsesSseEncoder {
    metadata: OpenAiResponseMetadata,
    lifecycle: CanonicalEventState,
    response: Option<ResponseState>,
    next_sequence_number: u64,
    terminal: Option<ResponsesTerminal>,
}

impl OpenAiResponsesSseEncoder {
    /// Creates an encoder that has not yet received `ResponseStart`.
    #[must_use]
    pub fn new(metadata: OpenAiResponseMetadata) -> Self {
        Self {
            metadata,
            lifecycle: CanonicalEventState::default(),
            response: None,
            next_sequence_number: 1,
            terminal: None,
        }
    }

    /// Validates and encodes one canonical event into ordered typed SSE frames.
    ///
    /// # Errors
    ///
    /// Returns the P1-03 lifecycle error for invalid canonical order, or a stream protocol error
    /// when a raw extension cannot be expressed in the public Responses event vocabulary. A valid
    /// terminal `StreamError` returns a failed response frame in `Ok`, never an adapter error.
    pub fn encode_event(&mut self, event: &CanonicalEvent) -> Result<Vec<SseFrame>, GatewayError> {
        ensure_representable_event_extensions(event)?;
        let mut next_lifecycle = self.lifecycle.clone();
        next_lifecycle.apply(event)?;

        let response_before = self.response.clone();
        let sequence_before = self.next_sequence_number;
        let terminal_before = self.terminal;
        let result = self.encode_valid_event(event);
        let frames = match result {
            Ok(frames) => frames,
            Err(error) => {
                self.response = response_before;
                self.next_sequence_number = sequence_before;
                self.terminal = terminal_before;
                return Err(error);
            }
        };

        self.lifecycle = next_lifecycle;
        Ok(frames)
    }

    /// Consumes an encoder after normal completion and returns its final response object.
    ///
    /// # Errors
    ///
    /// Returns `StreamTruncated/Stream` unless the encoded sequence reached normal
    /// `ResponseEnd`. A sequence ending in `StreamError` has already yielded its terminal error
    /// frame and cannot become a successful non-streaming response.
    pub fn into_completed_response(self) -> Result<Value, GatewayError> {
        if !self.lifecycle.is_success() {
            return Err(GatewayError::new(
                GatewayErrorCode::StreamTruncated,
                ErrorScope::Stream,
            ));
        }
        let response = self.response.ok_or_else(internal_error)?;
        let terminal = self.terminal.ok_or_else(internal_error)?;
        Ok(response.response_value(&self.metadata, terminal.status, terminal.incomplete_reason))
    }

    fn encode_valid_event(
        &mut self,
        event: &CanonicalEvent,
    ) -> Result<Vec<SseFrame>, GatewayError> {
        match event {
            CanonicalEvent::ResponseStart(start) => {
                self.encode_response_start(start.response_id.as_str())
            }
            CanonicalEvent::MessageStart(start) => self.encode_message_start(&start.role.0),
            CanonicalEvent::TextDelta(delta) => self.encode_text_delta(&delta.text),
            CanonicalEvent::ReasoningDelta(delta) => self.encode_reasoning_delta(&delta.text),
            CanonicalEvent::ToolCallStart(start) => {
                self.encode_tool_start(&start.call_id, &start.name)
            }
            CanonicalEvent::ToolCallArgumentsDelta(delta) => {
                self.encode_tool_arguments_delta(&delta.call_id, &delta.delta)
            }
            CanonicalEvent::ToolCallEnd(end) => {
                self.encode_tool_end(&end.call_id, end.arguments.get())
            }
            CanonicalEvent::UsageDelta(delta) => self.encode_usage(&delta.usage),
            CanonicalEvent::MessageEnd(_) => self.encode_message_end(),
            CanonicalEvent::ResponseEnd(end) => self.encode_response_end(end),
            CanonicalEvent::StreamError(error) => self.encode_stream_error(&error.error),
        }
    }

    fn encode_response_start(&mut self, response_id: &str) -> Result<Vec<SseFrame>, GatewayError> {
        if self.response.is_some() {
            return Err(stream_protocol_error());
        }
        let response = ResponseState::new(response_id.to_owned());
        let payload = response.response_value(&self.metadata, "in_progress", None);
        self.response = Some(response);

        Ok(vec![
            response_frame(
                &mut self.next_sequence_number,
                "response.created",
                payload.clone(),
            )?,
            response_frame(
                &mut self.next_sequence_number,
                "response.in_progress",
                payload,
            )?,
        ])
    }

    fn encode_message_start(&mut self, role: &str) -> Result<Vec<SseFrame>, GatewayError> {
        // Responses output messages are always assistant messages.  The canonical event model is
        // intentionally more general, so reject an otherwise valid core sequence here rather
        // than emitting a wire object that the public Responses schema cannot represent.
        if role != "assistant" {
            return Err(stream_protocol_error());
        }
        self.response_mut()?.begin_message(role.to_owned())?;
        Ok(Vec::new())
    }

    fn encode_text_delta(&mut self, delta: &str) -> Result<Vec<SseFrame>, GatewayError> {
        let appended = self.response_mut()?.append_text(delta)?;
        let mut frames = Vec::new();
        if appended.item_added {
            frames.push(output_item_added_frame(
                &mut self.next_sequence_number,
                appended.output_index,
                appended.item.ok_or_else(internal_error)?,
            )?);
        }
        if appended.part_added {
            frames.push(content_part_added_frame(
                &mut self.next_sequence_number,
                appended.output_index,
                &appended.item_id,
                appended.content_index,
            )?);
        }
        frames.push(text_delta_frame(
            &mut self.next_sequence_number,
            appended.output_index,
            &appended.item_id,
            appended.content_index,
            delta,
        )?);
        Ok(frames)
    }

    fn encode_reasoning_delta(&mut self, delta: &str) -> Result<Vec<SseFrame>, GatewayError> {
        let appended = self.response_mut()?.append_reasoning(delta)?;
        let mut frames = Vec::new();
        if appended.item_added {
            frames.push(output_item_added_frame(
                &mut self.next_sequence_number,
                appended.output_index,
                appended.item.ok_or_else(internal_error)?,
            )?);
        }
        frames.push(reasoning_delta_frame(
            &mut self.next_sequence_number,
            appended.output_index,
            &appended.item_id,
            delta,
        )?);
        Ok(frames)
    }

    fn encode_tool_start(
        &mut self,
        call_id: &str,
        name: &str,
    ) -> Result<Vec<SseFrame>, GatewayError> {
        let (output_index, item) = self.response_mut()?.begin_tool(call_id, name)?;
        Ok(vec![output_item_added_frame(
            &mut self.next_sequence_number,
            output_index,
            item,
        )?])
    }

    fn encode_tool_arguments_delta(
        &mut self,
        call_id: &str,
        delta: &str,
    ) -> Result<Vec<SseFrame>, GatewayError> {
        let appended = self.response_mut()?.append_tool_arguments(call_id, delta)?;
        Ok(vec![function_arguments_delta_frame(
            &mut self.next_sequence_number,
            appended.output_index,
            &appended.item_id,
            delta,
        )?])
    }

    fn encode_tool_end(
        &mut self,
        call_id: &str,
        arguments: &str,
    ) -> Result<Vec<SseFrame>, GatewayError> {
        let completed = self.response_mut()?.finish_tool(call_id, arguments)?;
        Ok(vec![
            function_arguments_done_frame(
                &mut self.next_sequence_number,
                completed.output_index,
                &completed.item_id,
                &completed.name,
                arguments,
            )?,
            output_item_done_frame(
                &mut self.next_sequence_number,
                completed.output_index,
                completed.item,
            )?,
        ])
    }

    fn encode_usage(&mut self, usage: &gateway_core::Usage) -> Result<Vec<SseFrame>, GatewayError> {
        self.response_mut()?.record_usage(usage)?;
        Ok(Vec::new())
    }

    fn encode_message_end(&mut self) -> Result<Vec<SseFrame>, GatewayError> {
        let completed = self.response_mut()?.finish_message()?;
        let mut frames = Vec::new();
        if let Some(reasoning) = completed.reasoning {
            append_reasoning_done_frames(&mut frames, &mut self.next_sequence_number, reasoning)?;
        }
        if let Some(message) = completed.message {
            append_message_done_frames(&mut frames, &mut self.next_sequence_number, message)?;
        }
        Ok(frames)
    }

    fn encode_response_end(
        &mut self,
        end: &gateway_core::ResponseEnd,
    ) -> Result<Vec<SseFrame>, GatewayError> {
        let terminal = ResponsesTerminal::try_from_end(end)?;
        let metadata = self.metadata.clone();
        let payload = self.response_mut()?.response_value(
            &metadata,
            terminal.status,
            terminal.incomplete_reason,
        );
        self.terminal = Some(terminal);
        Ok(vec![response_frame(
            &mut self.next_sequence_number,
            terminal.event,
            payload,
        )?])
    }

    fn encode_stream_error(&mut self, error: &GatewayError) -> Result<Vec<SseFrame>, GatewayError> {
        let response = self.response.as_ref().ok_or_else(stream_protocol_error)?;
        let payload = response.failed_value(&self.metadata, error);
        Ok(vec![response_frame(
            &mut self.next_sequence_number,
            "response.failed",
            payload,
        )?])
    }

    fn response_mut(&mut self) -> Result<&mut ResponseState, GatewayError> {
        self.response.as_mut().ok_or_else(stream_protocol_error)
    }
}

impl fmt::Debug for OpenAiResponsesSseEncoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiResponsesSseEncoder")
            .field("metadata", &self.metadata)
            .field("lifecycle", &self.lifecycle)
            .field("response_started", &self.response.is_some())
            .field("next_sequence_number", &self.next_sequence_number)
            .field("terminal", &self.terminal.is_some())
            .finish()
    }
}

#[derive(Clone)]
struct ResponseState {
    response_id: String,
    output: Vec<OutputItem>,
    usage: Option<Value>,
    message: Option<MessageState>,
    tools: BTreeMap<String, ToolState>,
}

#[derive(Clone, Copy)]
struct ResponsesTerminal {
    event: &'static str,
    status: &'static str,
    incomplete_reason: Option<&'static str>,
}

impl ResponsesTerminal {
    fn try_from_end(end: &gateway_core::ResponseEnd) -> Result<Self, GatewayError> {
        if end.stop_sequence.is_some() {
            return Err(stream_protocol_error());
        }
        match end.stop_reason.as_deref() {
            None | Some("end_turn" | "stop" | "tool_use" | "tool_calls") => Ok(Self {
                event: "response.completed",
                status: "completed",
                incomplete_reason: None,
            }),
            Some("max_tokens" | "length") => Ok(Self {
                event: "response.incomplete",
                status: "incomplete",
                incomplete_reason: Some("max_output_tokens"),
            }),
            Some("refusal" | "content_filter") => Ok(Self {
                event: "response.incomplete",
                status: "incomplete",
                incomplete_reason: Some("content_filter"),
            }),
            _ => Err(stream_protocol_error()),
        }
    }
}

impl ResponseState {
    fn new(response_id: String) -> Self {
        Self {
            response_id,
            output: Vec::new(),
            usage: None,
            message: None,
            tools: BTreeMap::new(),
        }
    }

    fn response_value(
        &self,
        metadata: &OpenAiResponseMetadata,
        status: &'static str,
        incomplete_reason: Option<&'static str>,
    ) -> Value {
        let mut response = Map::new();
        response.insert("id".to_owned(), Value::String(self.response_id.clone()));
        response.insert("object".to_owned(), Value::String("response".to_owned()));
        response.insert(
            "created_at".to_owned(),
            Value::Number(serde_json::Number::from(metadata.created_at())),
        );
        response.insert("status".to_owned(), Value::String(status.to_owned()));
        response.insert(
            "model".to_owned(),
            Value::String(metadata.model().to_owned()),
        );
        response.insert("error".to_owned(), Value::Null);
        response.insert(
            "incomplete_details".to_owned(),
            incomplete_reason.map_or(Value::Null, |reason| json!({"reason": reason})),
        );
        response.insert(
            "output".to_owned(),
            Value::Array(self.output.iter().map(OutputItem::to_value).collect()),
        );
        if let Some(usage) = &self.usage {
            response.insert("usage".to_owned(), usage.clone());
        } else {
            response.insert("usage".to_owned(), Value::Null);
        }
        Value::Object(response)
    }

    fn failed_value(&self, metadata: &OpenAiResponseMetadata, error: &GatewayError) -> Value {
        let mut response = match self.response_value(metadata, "failed", None) {
            Value::Object(response) => response,
            _ => Map::new(),
        };
        response.insert(
            "error".to_owned(),
            json!({
                "code": error.code().as_str(),
                "message": error.safe_message(),
            }),
        );
        Value::Object(response)
    }

    fn begin_message(&mut self, role: String) -> Result<(), GatewayError> {
        if self.message.is_some() {
            return Err(stream_protocol_error());
        }
        self.message = Some(MessageState {
            role,
            output_index: None,
            item_id: None,
            text_content_index: None,
            reasoning_output_index: None,
        });

        Ok(())
    }

    fn append_text(&mut self, delta: &str) -> Result<TextAppended, GatewayError> {
        let (role, existing_output_index, existing_item_id, existing_content_index) = {
            let message = self.message.as_ref().ok_or_else(stream_protocol_error)?;
            (
                message.role.clone(),
                message.output_index,
                message.item_id.clone(),
                message.text_content_index,
            )
        };
        if role != "assistant" {
            return Err(stream_protocol_error());
        }

        let (output_index, item_id, item_added) = match (existing_output_index, existing_item_id) {
            (Some(output_index), Some(item_id)) => (output_index, item_id, false),
            (None, None) => {
                let output_index = self.output.len();
                let item_id = format!("msg_{}_{}", self.response_id, output_index);
                self.output.push(OutputItem::Message {
                    id: item_id.clone(),
                    role,
                    status: OutputStatus::InProgress,
                    content: Vec::new(),
                });
                let message = self.message.as_mut().ok_or_else(internal_error)?;
                message.output_index = Some(output_index);
                message.item_id = Some(item_id.clone());
                (output_index, item_id, true)
            }
            _ => return Err(internal_error()),
        };

        let (content_index, part_added) = if let Some(content_index) = existing_content_index {
            (content_index, false)
        } else {
            let content_index = match self.output.get_mut(output_index) {
                Some(OutputItem::Message { content, .. }) => {
                    content.push(OutputTextPart::default());
                    content.len().saturating_sub(1)
                }
                _ => return Err(internal_error()),
            };
            let message = self.message.as_mut().ok_or_else(internal_error)?;
            message.text_content_index = Some(content_index);
            (content_index, true)
        };

        let item = if item_added {
            Some(self.output_item_value(output_index)?)
        } else {
            None
        };

        match self.output.get_mut(output_index) {
            Some(OutputItem::Message { content, .. }) => {
                let text = content.get_mut(content_index).ok_or_else(internal_error)?;
                text.text.push_str(delta);
            }
            _ => return Err(internal_error()),
        }

        Ok(TextAppended {
            output_index,
            item_id,
            content_index,
            item_added,
            part_added,
            item,
        })
    }

    fn append_reasoning(&mut self, delta: &str) -> Result<ReasoningAppended, GatewayError> {
        let (role, existing_index) = {
            let message = self.message.as_ref().ok_or_else(stream_protocol_error)?;
            (message.role.clone(), message.reasoning_output_index)
        };
        if role != "assistant" {
            return Err(stream_protocol_error());
        }

        let (output_index, item_id, item_added) = if let Some(output_index) = existing_index {
            let item_id = self.output_item_id(output_index)?.to_owned();
            (output_index, item_id, false)
        } else {
            let output_index = self.output.len();
            let item_id = format!("rsn_{}_{}", self.response_id, output_index);
            self.output.push(OutputItem::Reasoning {
                id: item_id.clone(),
                status: OutputStatus::InProgress,
                content: vec![String::new()],
            });
            let message = self.message.as_mut().ok_or_else(internal_error)?;
            message.reasoning_output_index = Some(output_index);
            (output_index, item_id, true)
        };

        let item = if item_added {
            Some(self.output_item_value(output_index)?)
        } else {
            None
        };

        match self.output.get_mut(output_index) {
            Some(OutputItem::Reasoning { content, .. }) => {
                let text = content.first_mut().ok_or_else(internal_error)?;
                text.push_str(delta);
            }
            _ => return Err(internal_error()),
        }

        Ok(ReasoningAppended {
            output_index,
            item_id,
            item_added,
            item,
        })
    }

    fn begin_tool(&mut self, call_id: &str, name: &str) -> Result<(usize, Value), GatewayError> {
        let message = self.message.as_ref().ok_or_else(stream_protocol_error)?;
        if message.role != "assistant" {
            return Err(stream_protocol_error());
        }
        if self.tools.contains_key(call_id) {
            return Err(stream_protocol_error());
        }
        let output_index = self.output.len();
        let item_id = format!("fc_{}_{}", self.response_id, output_index);
        self.output.push(OutputItem::FunctionCall {
            id: item_id.clone(),
            call_id: call_id.to_owned(),
            name: name.to_owned(),
            arguments: String::new(),
            status: OutputStatus::InProgress,
        });
        self.tools.insert(
            call_id.to_owned(),
            ToolState {
                output_index,
                item_id,
                saw_arguments_delta: false,
            },
        );

        Ok((output_index, self.output_item_value(output_index)?))
    }

    fn append_tool_arguments(
        &mut self,
        call_id: &str,
        delta: &str,
    ) -> Result<ToolAppended, GatewayError> {
        let tool = {
            let tool = self
                .tools
                .get_mut(call_id)
                .ok_or_else(stream_protocol_error)?;
            tool.saw_arguments_delta = true;
            tool.clone()
        };
        match self.output.get_mut(tool.output_index) {
            Some(OutputItem::FunctionCall { arguments, .. }) => arguments.push_str(delta),
            _ => return Err(internal_error()),
        }

        Ok(ToolAppended {
            output_index: tool.output_index,
            item_id: tool.item_id,
        })
    }

    fn finish_tool(
        &mut self,
        call_id: &str,
        arguments: &str,
    ) -> Result<ToolCompleted, GatewayError> {
        let tool = self
            .tools
            .remove(call_id)
            .ok_or_else(stream_protocol_error)?;
        let name = match self.output.get(tool.output_index) {
            Some(OutputItem::FunctionCall {
                name,
                arguments: accumulated,
                ..
            }) => {
                if tool.saw_arguments_delta && accumulated != arguments {
                    return Err(stream_protocol_error());
                }
                name.clone()
            }
            _ => return Err(internal_error()),
        };
        match self.output.get_mut(tool.output_index) {
            Some(OutputItem::FunctionCall {
                arguments: accumulated,
                status,
                ..
            }) => {
                arguments.clone_into(accumulated);
                *status = OutputStatus::Completed;
            }
            _ => return Err(internal_error()),
        }

        Ok(ToolCompleted {
            output_index: tool.output_index,
            item_id: tool.item_id,
            name,
            item: self.output_item_value(tool.output_index)?,
        })
    }

    fn record_usage(&mut self, usage: &gateway_core::Usage) -> Result<(), GatewayError> {
        self.usage = Some(usage_value(usage)?);
        Ok(())
    }

    fn finish_message(&mut self) -> Result<MessageCompleted, GatewayError> {
        let message = self.message.take().ok_or_else(stream_protocol_error)?;
        let completed_message = match (message.output_index, message.item_id) {
            (Some(output_index), Some(item_id)) => {
                let text = match message.text_content_index {
                    Some(content_index) => match self.output.get(output_index) {
                        Some(OutputItem::Message { content, .. }) => {
                            let content = content.get(content_index).ok_or_else(internal_error)?;
                            Some(CompletedText {
                                content_index,
                                text: content.text.clone(),
                            })
                        }
                        _ => return Err(internal_error()),
                    },
                    None => None,
                };
                match self.output.get_mut(output_index) {
                    Some(OutputItem::Message { status, .. }) => *status = OutputStatus::Completed,
                    _ => return Err(internal_error()),
                }
                Some(CompletedMessage {
                    output_index,
                    item_id,
                    item: self.output_item_value(output_index)?,
                    text,
                })
            }
            (None, None) => None,
            _ => return Err(internal_error()),
        };

        let reasoning = match message.reasoning_output_index {
            Some(output_index) => {
                let (item_id, text) = match self.output.get_mut(output_index) {
                    Some(OutputItem::Reasoning {
                        id,
                        status,
                        content,
                    }) => {
                        *status = OutputStatus::Completed;
                        let text = content.first().cloned().ok_or_else(internal_error)?;
                        (id.clone(), text)
                    }
                    _ => return Err(internal_error()),
                };
                Some(CompletedReasoning {
                    output_index,
                    item_id,
                    text,
                    item: self.output_item_value(output_index)?,
                })
            }
            None => None,
        };

        Ok(MessageCompleted {
            message: completed_message,
            reasoning,
        })
    }

    fn output_item_value(&self, output_index: usize) -> Result<Value, GatewayError> {
        self.output
            .get(output_index)
            .map(OutputItem::to_value)
            .ok_or_else(internal_error)
    }

    fn output_item_id(&self, output_index: usize) -> Result<&str, GatewayError> {
        self.output
            .get(output_index)
            .map(OutputItem::id)
            .ok_or_else(internal_error)
    }
}

#[derive(Clone)]
struct MessageState {
    role: String,
    output_index: Option<usize>,
    item_id: Option<String>,
    text_content_index: Option<usize>,
    reasoning_output_index: Option<usize>,
}

#[derive(Clone)]
struct ToolState {
    output_index: usize,
    item_id: String,
    saw_arguments_delta: bool,
}

struct TextAppended {
    output_index: usize,
    item_id: String,
    content_index: usize,
    item_added: bool,
    part_added: bool,
    item: Option<Value>,
}

struct ReasoningAppended {
    output_index: usize,
    item_id: String,
    item_added: bool,
    item: Option<Value>,
}

struct ToolAppended {
    output_index: usize,
    item_id: String,
}

struct ToolCompleted {
    output_index: usize,
    item_id: String,
    name: String,
    item: Value,
}

struct CompletedText {
    content_index: usize,
    text: String,
}

struct CompletedReasoning {
    output_index: usize,
    item_id: String,
    text: String,
    item: Value,
}

struct MessageCompleted {
    message: Option<CompletedMessage>,
    reasoning: Option<CompletedReasoning>,
}

struct CompletedMessage {
    output_index: usize,
    item_id: String,
    item: Value,
    text: Option<CompletedText>,
}

fn append_message_done_frames(
    frames: &mut Vec<SseFrame>,
    sequence: &mut u64,
    message: CompletedMessage,
) -> Result<(), GatewayError> {
    if let Some(text) = message.text {
        frames.push(text_done_frame(
            sequence,
            message.output_index,
            &message.item_id,
            text.content_index,
            &text.text,
        )?);
        frames.push(content_part_done_frame(
            sequence,
            message.output_index,
            &message.item_id,
            text.content_index,
            &text.text,
        )?);
    }
    frames.push(output_item_done_frame(
        sequence,
        message.output_index,
        message.item,
    )?);
    Ok(())
}

fn append_reasoning_done_frames(
    frames: &mut Vec<SseFrame>,
    sequence: &mut u64,
    reasoning: CompletedReasoning,
) -> Result<(), GatewayError> {
    frames.push(reasoning_done_frame(
        sequence,
        reasoning.output_index,
        &reasoning.item_id,
        &reasoning.text,
    )?);
    frames.push(output_item_done_frame(
        sequence,
        reasoning.output_index,
        reasoning.item,
    )?);
    Ok(())
}

#[derive(Clone)]
enum OutputItem {
    Message {
        id: String,
        role: String,
        status: OutputStatus,
        content: Vec<OutputTextPart>,
    },
    FunctionCall {
        id: String,
        call_id: String,
        name: String,
        arguments: String,
        status: OutputStatus,
    },
    Reasoning {
        id: String,
        status: OutputStatus,
        content: Vec<String>,
    },
}

impl OutputItem {
    fn id(&self) -> &str {
        match self {
            Self::Message { id, .. }
            | Self::FunctionCall { id, .. }
            | Self::Reasoning { id, .. } => id,
        }
    }

    fn to_value(&self) -> Value {
        match self {
            Self::Message {
                id,
                role,
                status,
                content,
            } => json!({
                "id": id,
                "type": "message",
                "status": status.as_str(),
                "role": role,
                "content": content.iter().map(OutputTextPart::to_value).collect::<Vec<_>>(),
            }),
            Self::FunctionCall {
                id,
                call_id,
                name,
                arguments,
                status,
            } => json!({
                "id": id,
                "type": "function_call",
                "status": status.as_str(),
                "call_id": call_id,
                "name": name,
                "arguments": arguments,
            }),
            Self::Reasoning {
                id,
                status,
                content,
            } => json!({
                "id": id,
                "type": "reasoning",
                "status": status.as_str(),
                "content": content.iter().map(|text| json!({
                    "type": "reasoning_text",
                    "text": text,
                })).collect::<Vec<_>>(),
            }),
        }
    }
}

#[derive(Clone, Default)]
struct OutputTextPart {
    text: String,
}

impl OutputTextPart {
    fn to_value(&self) -> Value {
        json!({
            "type": "output_text",
            "text": self.text,
            "annotations": [],
        })
    }
}

#[derive(Clone, Copy)]
enum OutputStatus {
    InProgress,
    Completed,
}

impl OutputStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
        }
    }
}

fn usage_value(usage: &gateway_core::Usage) -> Result<Value, GatewayError> {
    let mut encoded = Map::new();
    insert_optional_number(&mut encoded, "input_tokens", usage.input_tokens);
    insert_optional_number(&mut encoded, "output_tokens", usage.output_tokens);
    if let (Some(input), Some(output)) = (usage.input_tokens, usage.output_tokens) {
        encoded.insert(
            "total_tokens".to_owned(),
            Value::Number(serde_json::Number::from(
                input
                    .checked_add(output)
                    .ok_or_else(stream_protocol_error)?,
            )),
        );
    }

    let mut input_details = Map::new();
    insert_optional_number(&mut input_details, "cached_tokens", usage.cached_tokens);
    if !input_details.is_empty() {
        encoded.insert(
            "input_tokens_details".to_owned(),
            Value::Object(input_details),
        );
    }

    if let Some(reasoning_tokens) = usage.reasoning_tokens {
        encoded.insert(
            "output_tokens_details".to_owned(),
            json!({"reasoning_tokens": reasoning_tokens}),
        );
    }

    Ok(Value::Object(encoded))
}

fn insert_optional_number(object: &mut Map<String, Value>, name: &str, value: Option<u64>) {
    if let Some(value) = value {
        object.insert(
            name.to_owned(),
            Value::Number(serde_json::Number::from(value)),
        );
    }
}

fn ensure_representable_event_extensions(event: &CanonicalEvent) -> Result<(), GatewayError> {
    if let CanonicalEvent::UsageDelta(value) = event {
        // The Responses usage schema cannot losslessly distinguish these canonical values or
        // carry generic raw fields. Reject instead of inventing an aggregate or dropping data.
        if !value.extensions.is_empty()
            || !value.usage.extensions.is_empty()
            || value.usage.cache_read_tokens.is_some()
            || value.usage.cache_creation_tokens.is_some()
        {
            return Err(stream_protocol_error());
        }
        return Ok(());
    }

    let extensions = match event {
        CanonicalEvent::ResponseStart(value) => &value.extensions,
        CanonicalEvent::MessageStart(value) => &value.extensions,
        CanonicalEvent::TextDelta(value) => &value.extensions,
        CanonicalEvent::ReasoningDelta(value) => &value.extensions,
        CanonicalEvent::ToolCallStart(value) => &value.extensions,
        CanonicalEvent::ToolCallArgumentsDelta(value) => &value.extensions,
        CanonicalEvent::ToolCallEnd(value) => &value.extensions,
        CanonicalEvent::MessageEnd(value) => &value.extensions,
        CanonicalEvent::ResponseEnd(value) => &value.extensions,
        // The branch above has already checked nested Usage representability.
        CanonicalEvent::UsageDelta(_) | CanonicalEvent::StreamError(_) => return Ok(()),
    };
    if extensions.is_empty() {
        Ok(())
    } else {
        Err(stream_protocol_error())
    }
}

fn openai_error_type(error: &GatewayError) -> &'static str {
    match error.code() {
        GatewayErrorCode::ClientRequestError | GatewayErrorCode::TokenCountUnsupported => {
            "invalid_request_error"
        }
        GatewayErrorCode::ClientUnauthorized => "authentication_error",
        GatewayErrorCode::RouteNotFound => "not_found_error",
        GatewayErrorCode::ProviderRateLimited | GatewayErrorCode::CredentialQuotaExceeded => {
            "rate_limit_error"
        }
        GatewayErrorCode::Cancelled => "cancelled_error",
        GatewayErrorCode::CredentialUnauthorized
        | GatewayErrorCode::CredentialForbidden
        | GatewayErrorCode::CredentialUnavailable
        | GatewayErrorCode::EgressRejected
        | GatewayErrorCode::EgressUnavailable
        | GatewayErrorCode::ProviderTransient
        | GatewayErrorCode::ProviderPermanent
        | GatewayErrorCode::UpstreamProtocolError
        | GatewayErrorCode::StreamTruncated
        | GatewayErrorCode::InternalError => "server_error",
    }
}

fn response_frame(
    sequence: &mut u64,
    event: &'static str,
    response: Value,
) -> Result<SseFrame, GatewayError> {
    let mut data = Map::new();
    data.insert("response".to_owned(), response);
    frame(sequence, event, data)
}

fn output_item_added_frame(
    sequence: &mut u64,
    output_index: usize,
    item: Value,
) -> Result<SseFrame, GatewayError> {
    frame(
        sequence,
        "response.output_item.added",
        indexed_item_data(output_index, item),
    )
}

fn output_item_done_frame(
    sequence: &mut u64,
    output_index: usize,
    item: Value,
) -> Result<SseFrame, GatewayError> {
    frame(
        sequence,
        "response.output_item.done",
        indexed_item_data(output_index, item),
    )
}

fn indexed_item_data(output_index: usize, item: Value) -> Map<String, Value> {
    let mut data = Map::new();
    data.insert(
        "output_index".to_owned(),
        Value::Number(serde_json::Number::from(output_index)),
    );
    data.insert("item".to_owned(), item);
    data
}

fn content_part_added_frame(
    sequence: &mut u64,
    output_index: usize,
    item_id: &str,
    content_index: usize,
) -> Result<SseFrame, GatewayError> {
    let mut data = content_data(output_index, item_id, content_index);
    data.insert(
        "part".to_owned(),
        json!({"type": "output_text", "text": "", "annotations": []}),
    );
    frame(sequence, "response.content_part.added", data)
}

fn content_part_done_frame(
    sequence: &mut u64,
    output_index: usize,
    item_id: &str,
    content_index: usize,
    text: &str,
) -> Result<SseFrame, GatewayError> {
    let mut data = content_data(output_index, item_id, content_index);
    data.insert(
        "part".to_owned(),
        json!({"type": "output_text", "text": text, "annotations": []}),
    );
    frame(sequence, "response.content_part.done", data)
}

fn text_delta_frame(
    sequence: &mut u64,
    output_index: usize,
    item_id: &str,
    content_index: usize,
    delta: &str,
) -> Result<SseFrame, GatewayError> {
    let mut data = content_data(output_index, item_id, content_index);
    data.insert("delta".to_owned(), Value::String(delta.to_owned()));
    data.insert("logprobs".to_owned(), Value::Array(Vec::new()));
    frame(sequence, "response.output_text.delta", data)
}

fn text_done_frame(
    sequence: &mut u64,
    output_index: usize,
    item_id: &str,
    content_index: usize,
    text: &str,
) -> Result<SseFrame, GatewayError> {
    let mut data = content_data(output_index, item_id, content_index);
    data.insert("text".to_owned(), Value::String(text.to_owned()));
    data.insert("logprobs".to_owned(), Value::Array(Vec::new()));
    frame(sequence, "response.output_text.done", data)
}

fn content_data(output_index: usize, item_id: &str, content_index: usize) -> Map<String, Value> {
    let mut data = Map::new();
    data.insert(
        "output_index".to_owned(),
        Value::Number(serde_json::Number::from(output_index)),
    );
    data.insert("item_id".to_owned(), Value::String(item_id.to_owned()));
    data.insert(
        "content_index".to_owned(),
        Value::Number(serde_json::Number::from(content_index)),
    );
    data
}

fn reasoning_delta_frame(
    sequence: &mut u64,
    output_index: usize,
    item_id: &str,
    delta: &str,
) -> Result<SseFrame, GatewayError> {
    let mut data = reasoning_data(output_index, item_id);
    data.insert("delta".to_owned(), Value::String(delta.to_owned()));
    frame(sequence, "response.reasoning_text.delta", data)
}

fn reasoning_done_frame(
    sequence: &mut u64,
    output_index: usize,
    item_id: &str,
    text: &str,
) -> Result<SseFrame, GatewayError> {
    let mut data = reasoning_data(output_index, item_id);
    data.insert("text".to_owned(), Value::String(text.to_owned()));
    frame(sequence, "response.reasoning_text.done", data)
}

fn reasoning_data(output_index: usize, item_id: &str) -> Map<String, Value> {
    let mut data = Map::new();
    data.insert(
        "output_index".to_owned(),
        Value::Number(serde_json::Number::from(output_index)),
    );
    data.insert("item_id".to_owned(), Value::String(item_id.to_owned()));
    data.insert(
        "content_index".to_owned(),
        Value::Number(serde_json::Number::from(0_u64)),
    );
    data
}

fn function_arguments_delta_frame(
    sequence: &mut u64,
    output_index: usize,
    item_id: &str,
    delta: &str,
) -> Result<SseFrame, GatewayError> {
    let mut data = function_data(output_index, item_id);
    data.insert("delta".to_owned(), Value::String(delta.to_owned()));
    frame(sequence, "response.function_call_arguments.delta", data)
}

fn function_arguments_done_frame(
    sequence: &mut u64,
    output_index: usize,
    item_id: &str,
    name: &str,
    arguments: &str,
) -> Result<SseFrame, GatewayError> {
    let mut data = function_data(output_index, item_id);
    data.insert("arguments".to_owned(), Value::String(arguments.to_owned()));
    data.insert("name".to_owned(), Value::String(name.to_owned()));
    frame(sequence, "response.function_call_arguments.done", data)
}

fn function_data(output_index: usize, item_id: &str) -> Map<String, Value> {
    let mut data = Map::new();
    data.insert(
        "output_index".to_owned(),
        Value::Number(serde_json::Number::from(output_index)),
    );
    data.insert("item_id".to_owned(), Value::String(item_id.to_owned()));
    data
}

fn frame(
    sequence: &mut u64,
    event: &'static str,
    mut data: Map<String, Value>,
) -> Result<SseFrame, GatewayError> {
    let sequence_number = *sequence;
    *sequence = sequence.checked_add(1).ok_or_else(internal_error)?;
    data.insert("type".to_owned(), Value::String(event.to_owned()));
    data.insert(
        "sequence_number".to_owned(),
        Value::Number(serde_json::Number::from(sequence_number)),
    );
    Ok(SseFrame {
        event,
        data: Value::Object(data),
        semantic: true,
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{
        CanonicalEvent, CanonicalResponse, ErrorScope, GatewayError, GatewayErrorCode,
    };

    use super::{
        OpenAiResponseMetadata, OpenAiResponsesSseEncoder, ResponseMode, decode_request,
        encode_error, encode_model_list, encode_response,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    fn metadata() -> Result<OpenAiResponseMetadata, Box<dyn Error>> {
        Ok(OpenAiResponseMetadata::try_new(
            "gateway-model",
            1_700_000_000,
        )?)
    }

    fn canonical_events() -> Result<Vec<CanonicalEvent>, Box<dyn Error>> {
        Ok(serde_json::from_str(include_str!(
            "../../../tests/fixtures/openai-responses/canonical-events.json"
        ))?)
    }

    #[test]
    fn decodes_the_supported_openai_responses_request_fixture() -> TestResult {
        let decoded = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;

        assert_eq!(decoded.mode, ResponseMode::Streaming);
        assert_eq!(decoded.request.requested_model, "gateway-model");
        assert_eq!(decoded.request.messages.len(), 4);
        assert_eq!(decoded.request.messages[0].role.0, "developer");
        assert_eq!(decoded.request.messages[1].role.0, "user");
        assert_eq!(decoded.request.tools.len(), 1);
        assert_eq!(decoded.request.tools[0].name, "lookup_weather");
        assert_eq!(
            decoded.request.tools[0].input_schema.get(),
            r#"{"properties":{"city":{"type":"string"}},"type":"object"}"#
        );
        assert_eq!(
            decoded
                .request
                .extensions
                .get("openai.responses.metadata")
                .map(gateway_core::RawJson::get),
            Some(r#"{"source":"fixture"}"#)
        );
        assert!(decoded.request.thinking.is_some());
        if let Some(thinking) = decoded.request.thinking {
            assert_eq!(thinking.effort.as_str(), "medium");
            assert_eq!(
                thinking
                    .extensions
                    .get("summary")
                    .map(gateway_core::RawJson::get),
                Some(r#""auto""#)
            );
        }

        Ok(())
    }

    #[test]
    fn retains_unmapped_request_options_without_claiming_p1_execution_support() -> TestResult {
        let decoded = decode_request(
            r#"{
              "model":"gateway-model",
              "max_output_tokens":128,
              "temperature":0.25,
              "include":["reasoning.encrypted_content"]
            }"#,
        )?;

        assert_eq!(decoded.mode, ResponseMode::NonStreaming);
        assert_eq!(
            decoded
                .request
                .extensions
                .get("openai.responses.max_output_tokens")
                .map(gateway_core::RawJson::get),
            Some("128")
        );
        assert_eq!(
            decoded
                .request
                .extensions
                .get("openai.responses.temperature")
                .map(gateway_core::RawJson::get),
            Some("0.25")
        );
        assert_eq!(
            decoded
                .request
                .extensions
                .get("openai.responses.include")
                .map(gateway_core::RawJson::get),
            Some(r#"["reasoning.encrypted_content"]"#)
        );

        Ok(())
    }

    #[test]
    fn rejects_duplicate_json_names_at_every_nesting_level() {
        for request in [
            r#"{"model":"first","model":"second"}"#,
            r#"{"model":"gateway-model","input":[{"type":"message","role":"user","role":"assistant","content":"text"}]}"#,
            r#"{"model":"gateway-model","tools":[{"type":"function","name":"lookup","parameters":{"type":"object","properties":{"city":{"type":"string","type":"number"}}}}]}"#,
            r#"{"model":"gateway-model","input":[{"type":"message","role":"user","content":[{"type":"input_image","payload":{"url":"first","url":"second"}}]}]}"#,
            r#"{"model":"gateway-model","input":[{"type":"function_call","call_id":"history-call","name":"lookup","arguments":"{\"city\":\"Jakarta\",\"city\":\"Tokyo\"}"}]}"#,
            r#"{"model":"gateway-model","metadata":{"nested":{"safe":true,"safe":false}}}"#,
        ] {
            let result = decode_request(request);
            assert!(matches!(
                result,
                Err(error)
                    if error.code() == GatewayErrorCode::ClientRequestError
                        && error.scope() == ErrorScope::Request
            ));
        }
    }

    #[test]
    fn rejects_controls_that_cannot_be_honored_by_the_canonical_execution_path() {
        for request in [
            r#"{"model":"gateway-model","background":true}"#,
            r#"{"model":"gateway-model","tool_choice":"required"}"#,
            r#"{"model":"gateway-model","parallel_tool_calls":false}"#,
            r#"{"model":"gateway-model","text":{"format":{"type":"json_schema"}}}"#,
            r#"{"model":"gateway-model","tools":[{"type":"web_search_preview"}]}"#,
        ] {
            assert!(decode_request(request).is_err());
        }
    }

    #[test]
    fn required_tool_choice_is_retained_only_with_function_tools() -> TestResult {
        let decoded = decode_request(
            r#"{
          "model":"gateway-model",
          "input":[{"type":"message","role":"user","content":"call"}],
          "tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}],
          "tool_choice":"required"
        }"#,
        )?;
        assert_eq!(
            decoded
                .request
                .extensions
                .get("openai.responses.tool_choice")
                .map(gateway_core::RawJson::get),
            Some("\"required\"")
        );
        Ok(())
    }

    #[test]
    fn encodes_a_gateway_owned_public_models_list() -> TestResult {
        let encoded = encode_model_list(["gateway-model", "public-model-two"])?;

        assert_eq!(encoded["object"], "list");
        assert_eq!(encoded["data"].as_array().map(Vec::len), Some(2));
        assert_eq!(encoded["data"][0]["id"], "gateway-model");
        assert_eq!(encoded["data"][0]["object"], "model");
        assert_eq!(encoded["data"][0]["created"], 0);
        assert_eq!(encoded["data"][0]["owned_by"], "gateway");
        assert!(encoded["data"][0].get("upstream_model").is_none());
        assert!(encoded["data"][0].get("endpoint_id").is_none());
        Ok(())
    }

    #[test]
    fn rejects_an_empty_public_model_name_in_a_models_list() {
        let encoded = encode_model_list([""]);
        assert!(matches!(
            encoded,
            Err(error)
                if error.code() == GatewayErrorCode::InternalError
                    && error.scope() == ErrorScope::Internal
        ));
    }

    #[test]
    fn encodes_a_stable_typed_sse_lifecycle() -> TestResult {
        let mut encoder = OpenAiResponsesSseEncoder::new(metadata()?);
        let events = canonical_events()?;
        let mut frames = Vec::new();
        for event in &events {
            frames.extend(encoder.encode_event(event)?);
        }

        let event_names = frames
            .iter()
            .map(super::SseFrame::event)
            .collect::<Vec<_>>();
        assert_eq!(
            event_names,
            vec![
                "response.created",
                "response.in_progress",
                "response.output_item.added",
                "response.reasoning_text.delta",
                "response.output_item.added",
                "response.content_part.added",
                "response.output_text.delta",
                "response.output_item.added",
                "response.output_item.added",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.done",
                "response.output_item.done",
                "response.function_call_arguments.done",
                "response.output_item.done",
                "response.reasoning_text.done",
                "response.output_item.done",
                "response.output_text.done",
                "response.content_part.done",
                "response.output_item.done",
                "response.completed",
            ]
        );
        assert!(frames.iter().all(super::SseFrame::is_semantic));
        assert!(frames.iter().all(|frame| {
            frame.data().get("type").and_then(serde_json::Value::as_str) == Some(frame.event())
        }));
        let sequence_numbers = frames
            .iter()
            .filter_map(|frame| {
                frame
                    .data()
                    .get("sequence_number")
                    .and_then(serde_json::Value::as_u64)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            sequence_numbers,
            (1_u64..=u64::try_from(frames.len())?).collect::<Vec<_>>()
        );
        let rendered = frames
            .iter()
            .map(super::SseFrame::to_wire)
            .collect::<Result<String, _>>()?;
        assert_eq!(
            rendered,
            format!(
                "{}\n",
                include_str!("../../../tests/fixtures/openai-responses/stream.sse")
            )
        );

        Ok(())
    }

    #[test]
    fn encodes_non_streaming_response_from_the_same_canonical_sequence() -> TestResult {
        let events = canonical_events()?;
        let response = CanonicalResponse::try_new(events)?;
        let encoded = encode_response(&response, metadata()?);

        assert!(encoded.is_ok());
        if let Ok(encoded) = encoded {
            assert_eq!(
                format!("{}\n", serde_json::to_string_pretty(&encoded)?),
                include_str!(
                    "../../../tests/fixtures/openai-responses/non-streaming-response.json"
                )
            );
            assert_eq!(encoded["id"], "response-01");
            assert_eq!(encoded["status"], "completed");
            assert_eq!(encoded["model"], "gateway-model");
            assert_eq!(encoded["usage"]["total_tokens"], 20);
            assert_eq!(encoded["output"][0]["type"], "reasoning");
            assert_eq!(encoded["output"][1]["type"], "message");
            assert_eq!(encoded["output"][2]["type"], "function_call");
            assert_eq!(encoded["output"][3]["type"], "function_call");
        }

        Ok(())
    }

    #[test]
    fn max_token_and_refusal_stops_emit_incomplete_responses() -> TestResult {
        for (stop_reason, wire_reason) in [
            ("max_tokens", "max_output_tokens"),
            ("refusal", "content_filter"),
        ] {
            let mut events = canonical_events()?;
            let Some(CanonicalEvent::ResponseEnd(end)) = events.last_mut() else {
                return Err(std::io::Error::other("fixture lacks response end").into());
            };
            end.stop_reason = Some(stop_reason.to_owned());
            let response = CanonicalResponse::try_new(events.clone())?;
            let non_streaming = encode_response(&response, metadata()?)?;
            assert_eq!(non_streaming["status"], "incomplete");
            assert_eq!(non_streaming["incomplete_details"]["reason"], wire_reason);

            let mut stream_encoder = OpenAiResponsesSseEncoder::new(metadata()?);
            let mut frames = Vec::new();
            for event in &events {
                frames.extend(stream_encoder.encode_event(event)?);
            }
            let terminal = frames
                .last()
                .ok_or_else(|| std::io::Error::other("missing terminal frame"))?;
            assert_eq!(terminal.event(), "response.incomplete");
            assert_eq!(terminal.data()["response"]["status"], "incomplete");
            assert_eq!(
                terminal.data()["response"]["incomplete_details"]["reason"],
                wire_reason
            );
        }
        Ok(())
    }

    #[test]
    fn stream_error_is_a_terminal_failed_response_not_a_completed_response() -> TestResult {
        let mut encoder = OpenAiResponsesSseEncoder::new(metadata()?);
        let response_start: CanonicalEvent = serde_json::from_str(
            r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#,
        )?;
        let stream_error: CanonicalEvent = serde_json::from_str(
            r#"{"stream_error":{"error":{"code":"ProviderTransient","scope":"provider"}}}"#,
        )?;

        let _ = encoder.encode_event(&response_start)?;
        let frames = encoder.encode_event(&stream_error)?;

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event(), "response.failed");
        assert_eq!(frames[0].data()["response"]["status"], "failed");
        assert_eq!(
            frames[0].data()["response"]["error"]["code"],
            "ProviderTransient"
        );
        assert!(encoder.into_completed_response().is_err());

        Ok(())
    }

    #[test]
    fn rejects_non_assistant_output_and_events_after_terminality() -> TestResult {
        let response_start: CanonicalEvent = serde_json::from_str(
            r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#,
        )?;
        let user_message_start: CanonicalEvent =
            serde_json::from_str(r#"{"message_start":{"role":"user","extensions":{}}}"#)?;
        let assistant_message_start: CanonicalEvent =
            serde_json::from_str(r#"{"message_start":{"role":"assistant","extensions":{}}}"#)?;

        let mut encoder = OpenAiResponsesSseEncoder::new(metadata()?);
        let _ = encoder.encode_event(&response_start)?;
        let invalid_role = encoder.encode_event(&user_message_start);
        assert!(matches!(
            invalid_role,
            Err(error)
                if error.code() == GatewayErrorCode::UpstreamProtocolError
                    && error.scope() == ErrorScope::Stream
        ));
        // A rejected adapter-only shape must not advance the canonical lifecycle.
        assert!(encoder.encode_event(&assistant_message_start).is_ok());

        let stream_error: CanonicalEvent = serde_json::from_str(
            r#"{"stream_error":{"error":{"code":"ProviderTransient","scope":"provider"}}}"#,
        )?;
        let response_end: CanonicalEvent =
            serde_json::from_str(r#"{"response_end":{"extensions":{}}}"#)?;
        let mut terminal_encoder = OpenAiResponsesSseEncoder::new(metadata()?);
        let _ = terminal_encoder.encode_event(&response_start)?;
        let terminal_frames = terminal_encoder.encode_event(&stream_error)?;
        assert_eq!(terminal_frames.len(), 1);
        assert!(terminal_encoder.encode_event(&response_end).is_err());

        Ok(())
    }

    #[test]
    fn rejects_unrepresentable_raw_event_extensions_without_exposing_them() -> TestResult {
        let mut encoder = OpenAiResponsesSseEncoder::new(metadata()?);
        let response_start: CanonicalEvent = serde_json::from_str(
            r#"{"response_start":{"response_id":"response-01","extensions":{"vendor":{"secret":"do-not-send"}}}}"#,
        )?;
        let result = encoder.encode_event(&response_start);

        assert!(matches!(
            result,
            Err(error)
                if error.code() == GatewayErrorCode::UpstreamProtocolError
                    && error.scope() == ErrorScope::Stream
        ));
        assert!(!format!("{encoder:?}").contains("do-not-send"));

        Ok(())
    }

    #[test]
    fn rejects_mismatched_tool_argument_deltas() -> TestResult {
        let mut encoder = OpenAiResponsesSseEncoder::new(metadata()?);
        let events: Vec<CanonicalEvent> = serde_json::from_str(
            r#"[
              {"response_start":{"response_id":"response-01","extensions":{}}},
              {"message_start":{"role":"assistant","extensions":{}}},
              {"tool_call_start":{"call_id":"call-01","name":"lookup","extensions":{}}},
              {"tool_call_arguments_delta":{"call_id":"call-01","delta":"{\"city\":\"Jakarta\"}","extensions":{}}},
              {"tool_call_end":{"call_id":"call-01","arguments":{"city":"Tokyo"},"extensions":{}}}
            ]"#,
        )?;

        for event in events.iter().take(4) {
            let _ = encoder.encode_event(event)?;
        }
        let result = encoder.encode_event(&events[4]);
        assert!(matches!(
            result,
            Err(error)
                if error.code() == GatewayErrorCode::UpstreamProtocolError
                    && error.scope() == ErrorScope::Stream
        ));

        Ok(())
    }

    #[test]
    fn rejects_unrepresentable_or_overflowing_usage_without_silent_loss() -> TestResult {
        let response_start: CanonicalEvent = serde_json::from_str(
            r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#,
        )?;
        for usage in [
            r#"{"usage_delta":{"usage":{"input_tokens":1,"extensions":{"vendor":{"trace":"opaque"}}},"extensions":{}}}"#,
            r#"{"usage_delta":{"usage":{"cache_read_tokens":1,"extensions":{}},"extensions":{}}}"#,
            r#"{"usage_delta":{"usage":{"cache_creation_tokens":1,"extensions":{}},"extensions":{}}}"#,
            r#"{"usage_delta":{"usage":{"input_tokens":18446744073709551615,"output_tokens":1,"extensions":{}},"extensions":{}}}"#,
        ] {
            let usage: CanonicalEvent = serde_json::from_str(usage)?;
            let mut encoder = OpenAiResponsesSseEncoder::new(metadata()?);
            let _ = encoder.encode_event(&response_start)?;
            assert!(matches!(
                encoder.encode_event(&usage),
                Err(error)
                    if error.code() == GatewayErrorCode::UpstreamProtocolError
                        && error.scope() == ErrorScope::Stream
            ));
        }

        let response_events: Vec<CanonicalEvent> = serde_json::from_str(
            r#"[
              {"response_start":{"response_id":"response-01","extensions":{}}},
              {"usage_delta":{"usage":{"input_tokens":1,"extensions":{"vendor":{"trace":"opaque"}}},"extensions":{}}},
              {"response_end":{"extensions":{}}}
            ]"#,
        )?;
        let response = CanonicalResponse::try_new(response_events)?;
        assert!(matches!(
            encode_response(&response, metadata()?),
            Err(error)
                if error.code() == GatewayErrorCode::UpstreamProtocolError
                    && error.scope() == ErrorScope::Stream
        ));

        Ok(())
    }

    #[test]
    fn safe_error_envelopes_and_frame_debug_output_do_not_leak_diagnostics() -> TestResult {
        let error = GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider);
        let envelope = encode_error(&error);
        assert_eq!(envelope["error"]["type"], "server_error");
        assert_eq!(
            envelope["error"]["message"],
            "the provider failed transiently"
        );

        let mut encoder = OpenAiResponsesSseEncoder::new(metadata()?);
        let start: CanonicalEvent = serde_json::from_str(
            r#"{"response_start":{"response_id":"response-secret","extensions":{}}}"#,
        )?;
        let frames = encoder.encode_event(&start)?;
        let diagnostic = format!("{:?}{:?}", encoder, frames[0]);
        assert!(!diagnostic.contains("response-secret"));

        Ok(())
    }
}
