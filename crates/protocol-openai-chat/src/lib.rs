//! Strict OpenAI-compatible Chat Completions request and response codec.
//!
//! This crate owns protocol bytes only. HTTP, routing, provider selection and delivery commit stay
//! outside it. Unsupported Chat fields are retained in an explicit `openai.chat.*` namespace when
//! a later native Chat endpoint may preserve them; legacy or ambiguous shapes fail closed.

#![deny(unsafe_code)]

use std::{collections::BTreeSet, fmt};

use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalMessage, CanonicalRequest, CanonicalResponse,
    ErrorScope, GatewayError, GatewayErrorCode, MessageContent, MessageRole, RawExtensions,
    RawJson, TextContent, ToolCall, ToolDefinition, ToolResult, Usage,
};
use serde::{Deserialize, de};
use serde_json::{Map, Value, json};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "protocol-openai-chat";

/// Chat response representation requested by the client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseMode {
    /// Return one `chat.completion` object.
    NonStreaming,
    /// Return ordered `chat.completion.chunk` SSE data frames.
    Streaming,
}

/// A decoded Chat request and its requested response representation.
#[derive(Clone, Eq, PartialEq)]
pub struct DecodedChatRequest {
    /// Protocol-neutral request data.
    pub request: CanonicalRequest,
    /// Requested response representation.
    pub mode: ResponseMode,
    /// Whether a streaming response requests the final usage-only chunk.
    pub include_usage: bool,
}

impl fmt::Debug for DecodedChatRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecodedChatRequest")
            .field("request", &self.request)
            .field("mode", &self.mode)
            .field("include_usage", &self.include_usage)
            .finish()
    }
}

/// Decodes one complete OpenAI-compatible Chat Completions JSON request.
///
/// # Errors
///
/// Returns `ClientRequestError/Request` for malformed JSON, duplicate names, multiple choices,
/// legacy `function_call`, ambiguous content, or Tool shapes that cannot enter Canonical losslessly.
pub fn decode_request(input: &str) -> Result<DecodedChatRequest, GatewayError> {
    reject_duplicate_json_names(input)?;
    let value: Value = serde_json::from_str(input).map_err(|_| client_error())?;
    let root = object(&value)?;

    let requested_model = required_nonempty_string(root, "model")?.to_owned();
    let mode = match root.get("stream") {
        None | Some(Value::Bool(false)) => ResponseMode::NonStreaming,
        Some(Value::Bool(true)) => ResponseMode::Streaming,
        Some(_) => return Err(client_error()),
    };
    if root.get("n").is_some_and(|value| value.as_u64() != Some(1)) {
        return Err(client_error());
    }
    if root.contains_key("function_call") || root.contains_key("functions") {
        return Err(client_error());
    }
    if root
        .get("parallel_tool_calls")
        .is_some_and(|value| value.as_bool() != Some(true))
    {
        return Err(client_error());
    }
    if root
        .get("tool_choice")
        .is_some_and(|value| value.as_str() != Some("auto"))
    {
        return Err(client_error());
    }

    let include_usage = decode_stream_options(root.get("stream_options"), mode)?;
    let messages = array(required(root, "messages")?)?
        .iter()
        .map(decode_message)
        .collect::<Result<Vec<_>, _>>()?;
    if messages.is_empty() {
        return Err(client_error());
    }
    let tools = root
        .get("tools")
        .map_or_else(|| Ok(Vec::new()), decode_tools)?;
    let extensions = extensions_except(
        root,
        &[
            "model",
            "messages",
            "stream",
            "stream_options",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "n",
        ],
        "openai.chat.",
    )?;

    Ok(DecodedChatRequest {
        request: CanonicalRequest {
            requested_model,
            messages,
            tools,
            thinking: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            extensions,
        },
        mode,
        include_usage,
    })
}

fn decode_stream_options(value: Option<&Value>, mode: ResponseMode) -> Result<bool, GatewayError> {
    let Some(value) = value else { return Ok(false) };
    if mode != ResponseMode::Streaming {
        return Err(client_error());
    }
    let options = object(value)?;
    if options.keys().any(|name| name != "include_usage") {
        return Err(client_error());
    }
    match options.get("include_usage") {
        None | Some(Value::Bool(false)) => Ok(false),
        Some(Value::Bool(true)) => Ok(true),
        Some(_) => Err(client_error()),
    }
}

fn decode_message(value: &Value) -> Result<CanonicalMessage, GatewayError> {
    let message = object(value)?;
    let role = required_nonempty_string(message, "role")?;
    if message.contains_key("function_call") {
        return Err(client_error());
    }

    match role {
        "system" | "developer" | "user" => decode_text_message(message, role),
        "assistant" => decode_assistant_message(message),
        "tool" => decode_tool_result_message(message),
        _ => Err(client_error()),
    }
}

fn decode_text_message(
    message: &Map<String, Value>,
    role: &str,
) -> Result<CanonicalMessage, GatewayError> {
    let text = required_string(message, "content")?.to_owned();
    Ok(CanonicalMessage {
        role: MessageRole(role.to_owned()),
        content: vec![MessageContent::Text(TextContent {
            text,
            extensions: RawExtensions::default(),
        })],
        extensions: extensions_except(message, &["role", "content"], "openai.chat.message.")?,
    })
}

fn decode_assistant_message(
    message: &Map<String, Value>,
) -> Result<CanonicalMessage, GatewayError> {
    let mut content = Vec::new();
    match message.get("content") {
        None | Some(Value::Null) => {}
        Some(Value::String(text)) => content.push(MessageContent::Text(TextContent {
            text: text.clone(),
            extensions: RawExtensions::default(),
        })),
        Some(_) => return Err(client_error()),
    }
    if let Some(tool_calls) = message.get("tool_calls") {
        for value in array(tool_calls)? {
            content.push(MessageContent::ToolCall(decode_tool_call(value)?));
        }
    }
    if content.is_empty() {
        return Err(client_error());
    }
    Ok(CanonicalMessage {
        role: MessageRole("assistant".to_owned()),
        content,
        extensions: extensions_except(
            message,
            &["role", "content", "tool_calls"],
            "openai.chat.message.",
        )?,
    })
}

fn decode_tool_call(value: &Value) -> Result<ToolCall, GatewayError> {
    let call = object(value)?;
    if required_nonempty_string(call, "type")? != "function" {
        return Err(client_error());
    }
    let function = object(required(call, "function")?)?;
    if function
        .keys()
        .any(|name| !matches!(name.as_str(), "name" | "arguments"))
    {
        return Err(client_error());
    }
    let arguments = required_string(function, "arguments")?;
    reject_duplicate_json_names(arguments)?;
    let arguments = RawJson::from_json_string(arguments.to_owned()).map_err(|_| client_error())?;
    Ok(ToolCall {
        id: required_nonempty_string(call, "id")?.to_owned(),
        name: required_nonempty_string(function, "name")?.to_owned(),
        arguments,
        extensions: extensions_except(call, &["id", "type", "function"], "openai.chat.tool_call.")?,
    })
}

fn decode_tool_result_message(
    message: &Map<String, Value>,
) -> Result<CanonicalMessage, GatewayError> {
    let output = required_string(message, "content")?;
    let output = serde_json::to_string(output).map_err(|_| client_error())?;
    Ok(CanonicalMessage {
        role: MessageRole("tool".to_owned()),
        content: vec![MessageContent::ToolResult(ToolResult {
            call_id: required_nonempty_string(message, "tool_call_id")?.to_owned(),
            output: RawJson::from_json_string(output).map_err(|_| client_error())?,
            is_error: false,
            extensions: RawExtensions::default(),
        })],
        extensions: extensions_except(
            message,
            &["role", "content", "tool_call_id"],
            "openai.chat.message.",
        )?,
    })
}

fn decode_tools(value: &Value) -> Result<Vec<ToolDefinition>, GatewayError> {
    array(value)?.iter().map(decode_tool).collect()
}

fn decode_tool(value: &Value) -> Result<ToolDefinition, GatewayError> {
    let tool = object(value)?;
    if required_nonempty_string(tool, "type")? != "function" {
        return Err(client_error());
    }
    if tool
        .keys()
        .any(|name| !matches!(name.as_str(), "type" | "function"))
    {
        return Err(client_error());
    }
    let function = object(required(tool, "function")?)?;
    let parameters = required(function, "parameters")?;
    Ok(ToolDefinition {
        name: required_nonempty_string(function, "name")?.to_owned(),
        description: optional_string(function, "description")?,
        input_schema: raw_json(parameters)?,
        extensions: extensions_except(
            function,
            &["name", "description", "parameters"],
            "openai.chat.tool.",
        )?,
    })
}

/// Gateway-owned metadata attached to Chat response objects and chunks.
#[derive(Clone, Eq, PartialEq)]
pub struct ChatResponseMetadata {
    model: String,
    created_at: u64,
    include_usage: bool,
}

impl ChatResponseMetadata {
    /// Creates metadata for one resolved public model.
    ///
    /// # Errors
    ///
    /// Returns `InternalError/Internal` for an empty public model label.
    pub fn try_new(
        model: impl Into<String>,
        created_at: u64,
        include_usage: bool,
    ) -> Result<Self, GatewayError> {
        let model = model.into();
        if model.is_empty() {
            return Err(internal_error());
        }
        Ok(Self {
            model,
            created_at,
            include_usage,
        })
    }
}

impl fmt::Debug for ChatResponseMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChatResponseMetadata")
            .field("model", &"<redacted>")
            .field("created_at", &self.created_at)
            .field("include_usage", &self.include_usage)
            .finish()
    }
}

/// One Chat Completions SSE `data:` frame.
#[derive(Clone, Eq, PartialEq)]
pub struct ChatSseFrame {
    data: Option<Value>,
    semantic: bool,
}

impl ChatSseFrame {
    /// Returns the JSON payload, or `None` for the terminal `[DONE]` marker.
    #[must_use]
    pub fn data(&self) -> Option<&Value> {
        self.data.as_ref()
    }

    /// Returns whether the frame exposes new canonical semantics to the client.
    #[must_use]
    pub const fn is_semantic(&self) -> bool {
        self.semantic
    }

    /// Formats the frame as one SSE data record.
    ///
    /// # Errors
    ///
    /// Returns `InternalError/Internal` only if an adapter-created JSON value cannot serialize.
    pub fn to_wire(&self) -> Result<String, GatewayError> {
        match &self.data {
            Some(data) => Ok(format!(
                "data: {}\n\n",
                serde_json::to_string(data).map_err(|_| internal_error())?
            )),
            None => Ok("data: [DONE]\n\n".to_owned()),
        }
    }
}

impl fmt::Debug for ChatSseFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChatSseFrame")
            .field("done", &self.data.is_none())
            .field("semantic", &self.semantic)
            .field("data", &"<redacted>")
            .finish()
    }
}

/// Encodes one finite successful canonical response as a `chat.completion` object.
///
/// # Errors
///
/// Returns a stream-scoped protocol error for reasoning, raw extensions, non-assistant output,
/// unsupported stop reasons, or invalid canonical lifecycle.
pub fn encode_response(
    response: &CanonicalResponse,
    metadata: ChatResponseMetadata,
) -> Result<Value, GatewayError> {
    let mut encoder = ChatSseEncoder::new(metadata);
    for event in response.events() {
        let _ = encoder.encode_event(event)?;
    }
    encoder.into_completed_response()
}

/// Stateful Chat Completions SSE encoder for one canonical response lifecycle.
pub struct ChatSseEncoder {
    metadata: ChatResponseMetadata,
    lifecycle: CanonicalEventState,
    state: Option<ChatState>,
}

impl ChatSseEncoder {
    /// Creates an encoder before `ResponseStart`.
    #[must_use]
    pub fn new(metadata: ChatResponseMetadata) -> Self {
        Self {
            metadata,
            lifecycle: CanonicalEventState::default(),
            state: None,
        }
    }

    /// Validates and encodes one canonical event.
    ///
    /// # Errors
    ///
    /// Returns the canonical lifecycle error or a stream protocol error when Chat cannot express
    /// the event without loss. State is unchanged when encoding fails.
    pub fn encode_event(
        &mut self,
        event: &CanonicalEvent,
    ) -> Result<Vec<ChatSseFrame>, GatewayError> {
        ensure_event_extensions_empty(event)?;
        let mut lifecycle = self.lifecycle.clone();
        lifecycle.apply(event)?;
        let state_before = self.state.clone();
        let result = self.encode_valid_event(event);
        let frames = match result {
            Ok(frames) => frames,
            Err(error) => {
                self.state = state_before;
                return Err(error);
            }
        };
        self.lifecycle = lifecycle;
        Ok(frames)
    }

    /// Consumes a normally completed encoder and returns the non-streaming object.
    ///
    /// # Errors
    ///
    /// Returns `StreamTruncated/Stream` unless normal `ResponseEnd` was observed.
    pub fn into_completed_response(self) -> Result<Value, GatewayError> {
        if !self.lifecycle.is_success() {
            return Err(GatewayError::new(
                GatewayErrorCode::StreamTruncated,
                ErrorScope::Stream,
            ));
        }
        let state = self.state.ok_or_else(internal_error)?;
        let finish_reason = state.finish_reason.as_deref().ok_or_else(internal_error)?;
        let usage = state.usage.as_ref().map(usage_value).transpose()?;
        Ok(json!({
            "id": state.id,
            "object": "chat.completion",
            "created": self.metadata.created_at,
            "model": self.metadata.model,
            "choices": [{
                "index": 0,
                "message": state.message_value(),
                "finish_reason": finish_reason,
            }],
            "usage": usage,
        }))
    }

    fn encode_valid_event(
        &mut self,
        event: &CanonicalEvent,
    ) -> Result<Vec<ChatSseFrame>, GatewayError> {
        match event {
            CanonicalEvent::ResponseStart(start) => {
                if self.state.is_some() {
                    return Err(stream_error());
                }
                self.state = Some(ChatState::new(start.response_id.as_str().to_owned()));
                Ok(Vec::new())
            }
            CanonicalEvent::MessageStart(start) => {
                if start.role.0 != "assistant" {
                    return Err(stream_error());
                }
                Ok(vec![self.chunk(
                    &json!({"role":"assistant","content":Value::Null}),
                    true,
                )?])
            }
            CanonicalEvent::TextDelta(delta) => {
                self.state_mut()?.text.push_str(&delta.text);
                Ok(vec![self.chunk(&json!({"content":delta.text}), true)?])
            }
            CanonicalEvent::ReasoningDelta(_) => Err(stream_error()),
            CanonicalEvent::ToolCallStart(start) => {
                let state = self.state_mut()?;
                let index = state.tools.len();
                state.tools.push(ChatToolState {
                    id: start.call_id.clone(),
                    name: start.name.clone(),
                    arguments: String::new(),
                });
                Ok(vec![self.chunk(
                    &json!({"tool_calls":[{
                        "index":index,"id":start.call_id,"type":"function",
                        "function":{"name":start.name,"arguments":""}
                    }]}),
                    true,
                )?])
            }
            CanonicalEvent::ToolCallArgumentsDelta(delta) => {
                let (index, tool) = self.tool_mut(&delta.call_id)?;
                tool.arguments.push_str(&delta.delta);
                Ok(vec![self.chunk(
                    &json!({"tool_calls":[{
                        "index":index,"function":{"arguments":delta.delta}
                    }]}),
                    true,
                )?])
            }
            CanonicalEvent::ToolCallEnd(end) => {
                let (_, tool) = self.tool_mut(&end.call_id)?;
                if tool.arguments != end.arguments.get() {
                    return Err(stream_error());
                }
                Ok(Vec::new())
            }
            CanonicalEvent::UsageDelta(delta) => {
                let _ = usage_value(&delta.usage)?;
                self.state_mut()?.usage = Some(delta.usage.clone());
                Ok(Vec::new())
            }
            CanonicalEvent::MessageEnd(_) => Ok(Vec::new()),
            CanonicalEvent::ResponseEnd(end) => {
                let reason = chat_finish_reason(
                    end.stop_reason.as_deref(),
                    !self.state_ref()?.tools.is_empty(),
                )?;
                self.state_mut()?.finish_reason = Some(reason.to_owned());
                let mut frames = vec![self.chunk_with_finish(reason)?];
                if self.metadata.include_usage
                    && let Some(usage) = self.state_ref()?.usage.as_ref()
                {
                    frames.push(self.usage_chunk(usage)?);
                }
                frames.push(ChatSseFrame {
                    data: None,
                    semantic: false,
                });
                Ok(frames)
            }
            CanonicalEvent::StreamError(error) => Ok(vec![
                ChatSseFrame {
                    data: Some(encode_error(&error.error)),
                    semantic: true,
                },
                ChatSseFrame {
                    data: None,
                    semantic: false,
                },
            ]),
        }
    }

    fn state_ref(&self) -> Result<&ChatState, GatewayError> {
        self.state.as_ref().ok_or_else(stream_error)
    }

    fn state_mut(&mut self) -> Result<&mut ChatState, GatewayError> {
        self.state.as_mut().ok_or_else(stream_error)
    }

    fn tool_mut(&mut self, call_id: &str) -> Result<(usize, &mut ChatToolState), GatewayError> {
        self.state_mut()?
            .tools
            .iter_mut()
            .enumerate()
            .find(|(_, tool)| tool.id == call_id)
            .ok_or_else(stream_error)
    }

    fn chunk(&self, delta: &Value, semantic: bool) -> Result<ChatSseFrame, GatewayError> {
        let state = self.state_ref()?;
        Ok(ChatSseFrame {
            data: Some(json!({
                "id":state.id,"object":"chat.completion.chunk",
                "created":self.metadata.created_at,"model":self.metadata.model,
                "choices":[{"index":0,"delta":delta,"finish_reason":Value::Null}]
            })),
            semantic,
        })
    }

    fn chunk_with_finish(&self, reason: &str) -> Result<ChatSseFrame, GatewayError> {
        let state = self.state_ref()?;
        Ok(ChatSseFrame {
            data: Some(json!({
                "id":state.id,"object":"chat.completion.chunk",
                "created":self.metadata.created_at,"model":self.metadata.model,
                "choices":[{"index":0,"delta":{},"finish_reason":reason}]
            })),
            semantic: true,
        })
    }

    fn usage_chunk(&self, usage: &Usage) -> Result<ChatSseFrame, GatewayError> {
        let state = self.state_ref()?;
        Ok(ChatSseFrame {
            data: Some(json!({
                "id":state.id,"object":"chat.completion.chunk",
                "created":self.metadata.created_at,"model":self.metadata.model,
                "choices":[],"usage":usage_value(usage)?
            })),
            semantic: true,
        })
    }
}

impl fmt::Debug for ChatSseEncoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChatSseEncoder")
            .field("metadata", &self.metadata)
            .field("lifecycle", &self.lifecycle)
            .field("state_present", &self.state.is_some())
            .finish()
    }
}

#[derive(Clone)]
struct ChatState {
    id: String,
    text: String,
    tools: Vec<ChatToolState>,
    usage: Option<Usage>,
    finish_reason: Option<String>,
}

impl ChatState {
    fn new(id: String) -> Self {
        Self {
            id,
            text: String::new(),
            tools: Vec::new(),
            usage: None,
            finish_reason: None,
        }
    }

    fn message_value(&self) -> Value {
        let mut message = Map::new();
        message.insert("role".to_owned(), Value::String("assistant".to_owned()));
        message.insert(
            "content".to_owned(),
            if self.text.is_empty() {
                Value::Null
            } else {
                Value::String(self.text.clone())
            },
        );
        let tool_calls = self
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "id":tool.id,"type":"function",
                    "function":{"name":tool.name,"arguments":tool.arguments}
                })
            })
            .collect::<Vec<_>>();
        if !tool_calls.is_empty() {
            message.insert("tool_calls".to_owned(), Value::Array(tool_calls));
        }
        Value::Object(message)
    }
}

#[derive(Clone)]
struct ChatToolState {
    id: String,
    name: String,
    arguments: String,
}

fn usage_value(usage: &Usage) -> Result<Value, GatewayError> {
    if usage.cache_read_tokens.is_some() || usage.cache_creation_tokens.is_some() {
        return Err(stream_error());
    }
    let prompt = usage.input_tokens.ok_or_else(stream_error)?;
    let completion = usage.output_tokens.ok_or_else(stream_error)?;
    let total = prompt.checked_add(completion).ok_or_else(stream_error)?;
    let mut encoded = Map::new();
    encoded.insert("prompt_tokens".to_owned(), json!(prompt));
    encoded.insert("completion_tokens".to_owned(), json!(completion));
    encoded.insert("total_tokens".to_owned(), json!(total));
    if let Some(cached_tokens) = usage.cached_tokens {
        encoded.insert(
            "prompt_tokens_details".to_owned(),
            json!({"cached_tokens": cached_tokens}),
        );
    }
    if let Some(reasoning_tokens) = usage.reasoning_tokens {
        encoded.insert(
            "completion_tokens_details".to_owned(),
            json!({"reasoning_tokens": reasoning_tokens}),
        );
    }
    Ok(Value::Object(encoded))
}

fn chat_finish_reason(reason: Option<&str>, has_tools: bool) -> Result<&'static str, GatewayError> {
    match reason {
        Some("tool_use" | "tool_calls") => Ok("tool_calls"),
        Some("max_tokens" | "length") => Ok("length"),
        Some("end_turn" | "stop") | None if !has_tools => Ok("stop"),
        None if has_tools => Ok("tool_calls"),
        _ => Err(stream_error()),
    }
}

/// Encodes a safe core error in the common `OpenAI` error envelope.
#[must_use]
pub fn encode_error(error: &GatewayError) -> Value {
    json!({"error":{
        "message":error.safe_message(),
        "type":"gateway_error",
        "code":error.code().as_str(),
        "param":Value::Null
    }})
}

fn ensure_event_extensions_empty(event: &CanonicalEvent) -> Result<(), GatewayError> {
    let empty = match event {
        CanonicalEvent::ResponseStart(value) => value.extensions.is_empty(),
        CanonicalEvent::MessageStart(value) => value.extensions.is_empty(),
        CanonicalEvent::TextDelta(value) => value.extensions.is_empty(),
        CanonicalEvent::ReasoningDelta(value) => value.extensions.is_empty(),
        CanonicalEvent::ToolCallStart(value) => value.extensions.is_empty(),
        CanonicalEvent::ToolCallArgumentsDelta(value) => value.extensions.is_empty(),
        CanonicalEvent::ToolCallEnd(value) => value.extensions.is_empty(),
        CanonicalEvent::UsageDelta(value) => {
            value.extensions.is_empty() && value.usage.extensions.is_empty()
        }
        CanonicalEvent::MessageEnd(value) => value.extensions.is_empty(),
        CanonicalEvent::ResponseEnd(value) => value.extensions.is_empty(),
        CanonicalEvent::StreamError(_) => true,
    };
    if empty { Ok(()) } else { Err(stream_error()) }
}

fn required<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a Value, GatewayError> {
    object.get(name).ok_or_else(client_error)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, GatewayError> {
    required(object, name)?.as_str().ok_or_else(client_error)
}

fn required_nonempty_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, GatewayError> {
    let value = required_string(object, name)?;
    if value.is_empty() {
        Err(client_error())
    } else {
        Ok(value)
    }
}

fn optional_string(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Option<String>, GatewayError> {
    match object.get(name) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(client_error()),
    }
}

fn object(value: &Value) -> Result<&Map<String, Value>, GatewayError> {
    value.as_object().ok_or_else(client_error)
}

fn array(value: &Value) -> Result<&Vec<Value>, GatewayError> {
    value.as_array().ok_or_else(client_error)
}

fn raw_json(value: &Value) -> Result<RawJson, GatewayError> {
    RawJson::from_json_string(serde_json::to_string(value).map_err(|_| client_error())?)
        .map_err(|_| client_error())
}

fn extensions_except(
    object: &Map<String, Value>,
    known: &[&str],
    prefix: &str,
) -> Result<RawExtensions, GatewayError> {
    let known = known.iter().copied().collect::<BTreeSet<_>>();
    let mut extensions = RawExtensions::default();
    for (name, value) in object {
        if !known.contains(name.as_str()) {
            extensions
                .try_insert(format!("{prefix}{name}"), raw_json(value)?)
                .map_err(|_| client_error())?;
        }
    }
    Ok(extensions)
}

fn reject_duplicate_json_names(input: &str) -> Result<(), GatewayError> {
    serde_json::from_str::<DuplicateFreeJson>(input)
        .map(|_| ())
        .map_err(|_| client_error())
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
        formatter.write_str("JSON without duplicate object names")
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

const fn client_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

const fn stream_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use gateway_core::{CanonicalEvent, CanonicalResponse, GatewayErrorCode, MessageContent};
    use serde_json::{Value, json};

    use super::{
        ChatResponseMetadata, ChatSseEncoder, ResponseMode, decode_request, encode_response,
    };

    fn response(events: Value) -> Result<CanonicalResponse, Box<dyn std::error::Error>> {
        let events = serde_json::from_value::<Vec<CanonicalEvent>>(events)?;
        Ok(CanonicalResponse::try_new(events)?)
    }

    fn text_events() -> Value {
        json!([
            {"response_start":{"response_id":"opaque","extensions":{}}},
            {"message_start":{"role":"assistant","extensions":{}}},
            {"text_delta":{"text":"hello","extensions":{}}},
            {"usage_delta":{"usage":{"input_tokens":2,"output_tokens":1,"extensions":{}},"is_final":true,"extensions":{}}},
            {"message_end":{"extensions":{}}},
            {"response_end":{"stop_reason":"end_turn","extensions":{}}}
        ])
    }

    #[test]
    fn request_preserves_text_tools_history_and_native_extensions()
    -> Result<(), Box<dyn std::error::Error>> {
        let decoded = decode_request(
            r#"{
          "model":"public-model","stream":true,"stream_options":{"include_usage":true},
          "temperature":0.2,
          "messages":[
            {"role":"system","content":"be exact"},
            {"role":"assistant","content":null,"tool_calls":[{"id":"call-1","type":"function","function":{"name":"lookup","arguments":"{\"q\":\"x\"}"}}]},
            {"role":"tool","tool_call_id":"call-1","content":"done"},
            {"role":"user","content":"continue"}
          ],
          "tools":[{"type":"function","function":{"name":"lookup","description":"lookup","parameters":{"type":"object"}}}],
          "tool_choice":"auto","parallel_tool_calls":true
        }"#,
        )?;

        assert_eq!(decoded.mode, ResponseMode::Streaming);
        assert!(decoded.include_usage);
        assert_eq!(decoded.request.messages.len(), 4);
        assert_eq!(decoded.request.tools.len(), 1);
        assert!(
            decoded
                .request
                .extensions
                .get("openai.chat.temperature")
                .is_some()
        );
        assert!(matches!(
            decoded.request.messages[1].content[0],
            MessageContent::ToolCall(_)
        ));
        assert!(matches!(
            decoded.request.messages[2].content[0],
            MessageContent::ToolResult(_)
        ));
        Ok(())
    }

    #[test]
    fn ambiguous_legacy_and_multi_choice_requests_fail_closed() {
        for input in [
            r#"{"model":"m","messages":[{"role":"user","content":"x"}],"n":2}"#,
            r#"{"model":"m","messages":[{"role":"user","content":"x"}],"functions":[]}"#,
            r#"{"model":"m","model":"other","messages":[{"role":"user","content":"x"}]}"#,
            r#"{"model":"m","messages":[{"role":"assistant","content":null}]}"#,
            r#"{"model":"m","messages":[{"role":"assistant","tool_calls":[{"id":"c","type":"function","function":{"name":"f","arguments":"{}","unknown":true}}]}]}"#,
        ] {
            let error = decode_request(input).err();
            assert_eq!(
                error.as_ref().map(gateway_core::GatewayError::code),
                Some(GatewayErrorCode::ClientRequestError)
            );
        }
    }

    #[test]
    fn completed_text_response_and_stream_use_chat_shapes() -> Result<(), Box<dyn std::error::Error>>
    {
        let canonical = response(text_events())?;
        let value = encode_response(
            &canonical,
            ChatResponseMetadata::try_new("public-model", 7, true)?,
        )?;
        assert_eq!(value["object"], "chat.completion");
        assert_eq!(value["choices"][0]["message"]["content"], "hello");
        assert_eq!(value["choices"][0]["finish_reason"], "stop");
        assert_eq!(value["usage"]["total_tokens"], 3);

        let events = serde_json::from_value::<Vec<CanonicalEvent>>(text_events())?;
        let mut encoder =
            ChatSseEncoder::new(ChatResponseMetadata::try_new("public-model", 7, true)?);
        let mut frames = Vec::new();
        for event in &events {
            frames.extend(encoder.encode_event(event)?);
        }
        assert!(frames.iter().any(|frame| {
            frame
                .data()
                .is_some_and(|data| data["choices"][0]["delta"]["content"] == "hello")
        }));
        assert!(frames.iter().any(|frame| {
            frame.data().is_some_and(|data| {
                data["choices"].as_array().is_some_and(Vec::is_empty) && data.get("usage").is_some()
            })
        }));
        assert!(frames.last().is_some_and(|frame| frame.data().is_none()));
        let finish_index = frames
            .iter()
            .position(|frame| {
                frame
                    .data()
                    .is_some_and(|data| data["choices"][0]["finish_reason"] == "stop")
            })
            .ok_or("missing finish frame")?;
        let usage_index = frames
            .iter()
            .position(|frame| {
                frame.data().is_some_and(|data| {
                    data["choices"].as_array().is_some_and(Vec::is_empty)
                        && data.get("usage").is_some()
                })
            })
            .ok_or("missing usage frame")?;
        assert!(finish_index < usage_index);
        assert_eq!(usage_index + 1, frames.len() - 1);
        assert_eq!(
            frames
                .last()
                .map(super::ChatSseFrame::to_wire)
                .transpose()?,
            Some("data: [DONE]\n\n".to_owned())
        );
        Ok(())
    }

    #[test]
    fn tool_argument_fragmentation_does_not_change_completed_message()
    -> Result<(), Box<dyn std::error::Error>> {
        let arguments = r#"{"q":"fragmented"}"#;
        let expected = encode_tool_response(&[arguments])?;
        for split in 0..=arguments.len() {
            if !arguments.is_char_boundary(split) {
                continue;
            }
            let actual = encode_tool_response(&[&arguments[..split], &arguments[split..]])?;
            assert_eq!(actual, expected);
        }
        Ok(())
    }

    fn encode_tool_response(chunks: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
        let mut events = vec![
            json!({"response_start":{"response_id":"opaque","extensions":{}}}),
            json!({"message_start":{"role":"assistant","extensions":{}}}),
            json!({"tool_call_start":{"call_id":"call-1","name":"lookup","extensions":{}}}),
        ];
        events.extend(chunks.iter().map(|chunk| {
            json!({"tool_call_arguments_delta":{
                "call_id":"call-1","delta":chunk,"extensions":{}
            }})
        }));
        events.extend([
            json!({"tool_call_end":{
                "call_id":"call-1","arguments":{"q":"fragmented"},"extensions":{}
            }}),
            json!({"message_end":{"extensions":{}}}),
            json!({"response_end":{"stop_reason":"tool_use","extensions":{}}}),
        ]);
        let canonical = response(Value::Array(events))?;
        Ok(encode_response(
            &canonical,
            ChatResponseMetadata::try_new("public-model", 7, false)?,
        )?)
    }

    #[test]
    fn unrepresentable_usage_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
        for usage in [
            json!({"input_tokens":u64::MAX,"output_tokens":1,"extensions":{}}),
            json!({"input_tokens":1,"extensions":{}}),
            json!({"input_tokens":1,"output_tokens":1,"cache_read_tokens":1,"extensions":{}}),
            json!({"input_tokens":1,"output_tokens":1,"cache_creation_tokens":1,"extensions":{}}),
        ] {
            let canonical = response(json!([
                {"response_start":{"response_id":"opaque","extensions":{}}},
                {"message_start":{"role":"assistant","extensions":{}}},
                {"text_delta":{"text":"x","extensions":{}}},
                {"usage_delta":{"usage":usage,"is_final":true,"extensions":{}}},
                {"message_end":{"extensions":{}}},
                {"response_end":{"stop_reason":"end_turn","extensions":{}}}
            ]))?;
            let error = encode_response(
                &canonical,
                ChatResponseMetadata::try_new("public-model", 7, true)?,
            );
            assert_eq!(
                error.err().as_ref().map(gateway_core::GatewayError::code),
                Some(GatewayErrorCode::UpstreamProtocolError)
            );
        }
        Ok(())
    }

    #[test]
    fn tool_lifecycle_maps_to_chat_tool_calls() -> Result<(), Box<dyn std::error::Error>> {
        let canonical = response(json!([
            {"response_start":{"response_id":"opaque","extensions":{}}},
            {"message_start":{"role":"assistant","extensions":{}}},
            {"tool_call_start":{"call_id":"call-1","name":"lookup","extensions":{}}},
            {"tool_call_arguments_delta":{"call_id":"call-1","delta":"{\"q\":\"x\"}","extensions":{}}},
            {"tool_call_end":{"call_id":"call-1","arguments":{"q":"x"},"extensions":{}}},
            {"message_end":{"extensions":{}}},
            {"response_end":{"stop_reason":"tool_use","extensions":{}}}
        ]))?;
        let value = encode_response(
            &canonical,
            ChatResponseMetadata::try_new("public-model", 7, false)?,
        )?;
        assert_eq!(value["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(
            value["choices"][0]["message"]["tool_calls"][0]["type"],
            "function"
        );
        assert_eq!(
            value["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
            "{\"q\":\"x\"}"
        );
        Ok(())
    }

    #[test]
    fn reasoning_is_rejected_instead_of_becoming_visible_chat_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let events = serde_json::from_value::<Vec<CanonicalEvent>>(json!([
            {"response_start":{"response_id":"opaque","extensions":{}}},
            {"message_start":{"role":"assistant","extensions":{}}},
            {"reasoning_delta":{"text":"private","extensions":{}}}
        ]))?;
        let mut encoder =
            ChatSseEncoder::new(ChatResponseMetadata::try_new("public-model", 7, false)?);
        assert!(encoder.encode_event(&events[0]).is_ok());
        assert!(encoder.encode_event(&events[1]).is_ok());
        let error = encoder.encode_event(&events[2]);
        assert_eq!(
            error.err().as_ref().map(gateway_core::GatewayError::code),
            Some(GatewayErrorCode::UpstreamProtocolError)
        );
        Ok(())
    }
}
