//! Grok Build's fixed CLI Responses request profile and bounded response decoder.
//!
//! The request layer reimplements the locked, lossless OpenAI-compatible Responses subset inside
//! this private Provider crate, then adds only the frozen Grok CLI identity headers. The decoder
//! accepts fixed JSON/SSE fixtures without opening a socket; P6-07 owns status-to-remediation
//! classification and P6-06 owns response continuity headers/state.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::Read,
    sync::OnceLock,
};

use flate2::read::MultiGzDecoder;
use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalMessage, CanonicalRequest, CanonicalResponse,
    ErrorScope, GatewayError, GatewayErrorCode, MessageContent, MessageEnd, MessageRole,
    MessageStart, RawExtensions, RawJson, ReasoningDelta, ResponseEnd, ResponseId, ResponseStart,
    StreamError, TextDelta, ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart, ToolDefinition,
    Usage, UsageDelta,
};
use gateway_upstream::{
    AdmittedEgressTarget, EndpointUrl, UpstreamHttpMethod, UpstreamHttpRequest,
};
use protocol_openai_responses::ResponseMode;
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::{GrokBuildCacheIdentity, GrokBuildCredential, strict_json::parse_strict_json};

/// Frozen Grok CLI chat-proxy base URL used by OAuth Build credentials.
pub const GROK_BUILD_RESPONSES_BASE_URL: &str = "https://cli-chat-proxy.grok.com/v1";
/// Fixed Responses path appended to [`GROK_BUILD_RESPONSES_BASE_URL`].
pub const GROK_BUILD_RESPONSES_PATH: &str = "/responses";
/// Complete fixed Grok Build Responses URL.
pub const GROK_BUILD_RESPONSES_URL: &str = "https://cli-chat-proxy.grok.com/v1/responses";
/// Header that identifies the Grok CLI OAuth token profile.
pub const GROK_BUILD_TOKEN_AUTH_HEADER: &str = "x-xai-token-auth";
/// Fixed value for [`GROK_BUILD_TOKEN_AUTH_HEADER`].
pub const GROK_BUILD_TOKEN_AUTH_VALUE: &str = "xai-grok-cli";
/// Header that carries the frozen Grok CLI client version.
pub const GROK_BUILD_CLIENT_VERSION_HEADER: &str = "x-grok-client-version";
/// Frozen Grok CLI client version used for the Build profile.
pub const GROK_BUILD_CLIENT_VERSION: &str = "0.2.106";
/// Header that identifies the current Grok CLI shell client.
pub const GROK_BUILD_CLIENT_IDENTIFIER_HEADER: &str = "x-grok-client-identifier";
/// Fixed current Grok CLI shell client identifier.
pub const GROK_BUILD_CLIENT_IDENTIFIER: &str = "grok-shell";
/// Header that selects the non-interactive Grok CLI request mode.
pub const GROK_BUILD_CLIENT_MODE_HEADER: &str = "x-grok-client-mode";
/// Fixed non-interactive Grok CLI request mode.
pub const GROK_BUILD_CLIENT_MODE: &str = "headless";
/// Header that confirms the OAuth client request profile.
pub const GROK_BUILD_AUTHENTICATE_RESPONSE_HEADER: &str = "x-authenticateresponse";
/// Fixed confirmation value for [`GROK_BUILD_AUTHENTICATE_RESPONSE_HEADER`].
pub const GROK_BUILD_AUTHENTICATE_RESPONSE_VALUE: &str = "authenticate-response";
/// Header that carries a process-scoped Grok CLI agent association.
pub const GROK_BUILD_AGENT_ID_HEADER: &str = "x-grok-agent-id";
/// Header that carries a request-scoped Grok CLI request association.
pub const GROK_BUILD_REQUEST_ID_HEADER: &str = "x-grok-req-id";
/// Header that carries a request-scoped W3C trace association.
pub const GROK_BUILD_TRACEPARENT_HEADER: &str = "traceparent";
/// Header that makes the selected upstream Build model explicit to the CLI proxy.
pub const GROK_BUILD_MODEL_OVERRIDE_HEADER: &str = "x-grok-model-override";
/// Fixed evidence-supported Grok CLI workspace user agent for the Build profile.
pub const GROK_BUILD_USER_AGENT: &str = "xai-grok-workspace/0.2.106";
/// Maximum JSON body accepted for one non-streaming Build response.
pub const MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES: usize = 1024 * 1024;
/// Maximum retained bytes for one upstream HTTP error body.
pub const MAX_GROK_BUILD_ERROR_BODY_BYTES: usize = 64 * 1024;
/// Maximum bytes for one complete SSE record, excluding the terminating blank line.
pub const MAX_GROK_BUILD_SSE_FRAME_BYTES: usize = 64 * 1024;

const MAX_GROK_BUILD_IDENTIFIER_BYTES: usize = 512;
const MAX_GROK_BUILD_TOOL_ARGUMENT_BYTES: usize = 64 * 1024;
const OPENAI_RESPONSES_EXTENSION_PREFIX: &str = "openai.responses.";
const ROOT_RESERVED_FIELDS: &[&str] = &[
    "model",
    "stream",
    "input",
    "tools",
    "reasoning",
    "prompt_cache_key",
    "prompt_cache_retention",
];
const MESSAGE_RESERVED_FIELDS: &[&str] = &["type", "role", "content"];
const TEXT_RESERVED_FIELDS: &[&str] = &["type", "text"];
const TOOL_CALL_RESERVED_FIELDS: &[&str] = &["type", "call_id", "name", "arguments"];
const TOOL_RESULT_RESERVED_FIELDS: &[&str] = &["type", "call_id", "output"];
const TOOL_RESERVED_FIELDS: &[&str] = &["type", "name", "description", "parameters"];
const REASONING_RESERVED_FIELDS: &[&str] = &["effort"];

static GROK_BUILD_PROCESS_AGENT_ID: OnceLock<Option<String>> = OnceLock::new();

/// The fixed, production Grok Build Responses endpoint.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokBuildResponsesEndpoint {
    target: EndpointUrl,
}

impl GrokBuildResponsesEndpoint {
    /// Creates the frozen Grok CLI chat-proxy Responses endpoint.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` if a future fixed endpoint constant becomes invalid.
    pub fn try_new() -> Result<Self, GatewayError> {
        let target = EndpointUrl::compose(GROK_BUILD_RESPONSES_BASE_URL, GROK_BUILD_RESPONSES_PATH)
            .map_err(|_| egress_rejected_error())?;
        Ok(Self { target })
    }

    /// Returns the complete fixed endpoint URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }
}

impl fmt::Debug for GrokBuildResponsesEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokBuildResponsesEndpoint(<redacted>)")
    }
}

/// One request-ready Grok Build Responses submission.
///
/// The private inner request retains its OAuth bearer value in zeroizing storage. This wrapper
/// exposes only the fixed Build profile and the exact P2-09-admitted transport handoff.
#[derive(Eq, PartialEq)]
pub struct GrokBuildResponsesOutboundRequest {
    target: EndpointUrl,
    authorization: Zeroizing<String>,
    accept: &'static str,
    accept_encoding: &'static str,
    agent_id: String,
    request_id: String,
    traceparent: String,
    model_override: String,
    body: Vec<u8>,
}

impl GrokBuildResponsesOutboundRequest {
    /// Returns the complete configured endpoint URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }

    /// Returns one fixed Build-profile or standard request header by case-insensitive name.
    ///
    /// The Authorization value is request-scoped and must not be logged or persisted by callers.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case(GROK_BUILD_TOKEN_AUTH_HEADER) {
            Some(GROK_BUILD_TOKEN_AUTH_VALUE)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_CLIENT_VERSION_HEADER) {
            Some(GROK_BUILD_CLIENT_VERSION)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_CLIENT_IDENTIFIER_HEADER) {
            Some(GROK_BUILD_CLIENT_IDENTIFIER)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_CLIENT_MODE_HEADER) {
            Some(GROK_BUILD_CLIENT_MODE)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_AUTHENTICATE_RESPONSE_HEADER) {
            Some(GROK_BUILD_AUTHENTICATE_RESPONSE_VALUE)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_AGENT_ID_HEADER) {
            Some(&self.agent_id)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_REQUEST_ID_HEADER) {
            Some(&self.request_id)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_TRACEPARENT_HEADER) {
            Some(&self.traceparent)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_MODEL_OVERRIDE_HEADER) {
            Some(&self.model_override)
        } else if name.eq_ignore_ascii_case("user-agent") {
            Some(GROK_BUILD_USER_AGENT)
        } else if name.eq_ignore_ascii_case("accept") {
            Some(self.accept)
        } else if name.eq_ignore_ascii_case("accept-encoding") {
            Some(self.accept_encoding)
        } else if name.eq_ignore_ascii_case("authorization") {
            Some(self.authorization.as_str())
        } else if name.eq_ignore_ascii_case("content-type") {
            Some("application/json")
        } else {
            None
        }
    }

    /// Returns the complete deterministic header set in transport order.
    #[must_use]
    pub fn headers(&self) -> [(&'static str, &str); 14] {
        [
            ("accept", self.accept),
            ("accept-encoding", self.accept_encoding),
            ("authorization", self.authorization.as_str()),
            ("content-type", "application/json"),
            (GROK_BUILD_TOKEN_AUTH_HEADER, GROK_BUILD_TOKEN_AUTH_VALUE),
            (GROK_BUILD_CLIENT_VERSION_HEADER, GROK_BUILD_CLIENT_VERSION),
            (
                GROK_BUILD_CLIENT_IDENTIFIER_HEADER,
                GROK_BUILD_CLIENT_IDENTIFIER,
            ),
            (GROK_BUILD_CLIENT_MODE_HEADER, GROK_BUILD_CLIENT_MODE),
            (
                GROK_BUILD_AUTHENTICATE_RESPONSE_HEADER,
                GROK_BUILD_AUTHENTICATE_RESPONSE_VALUE,
            ),
            (GROK_BUILD_AGENT_ID_HEADER, &self.agent_id),
            (GROK_BUILD_REQUEST_ID_HEADER, &self.request_id),
            (GROK_BUILD_TRACEPARENT_HEADER, &self.traceparent),
            ("user-agent", GROK_BUILD_USER_AGENT),
            (GROK_BUILD_MODEL_OVERRIDE_HEADER, &self.model_override),
        ]
    }

    /// Returns the complete JSON request body without rendering it to a log representation.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Consumes this request into an exact-target DNS-pinned transport request.
    ///
    /// The Build CLI reference also sets `Connection: Keep-Alive`; this gateway deliberately does
    /// not copy that hop-by-hop header because `gateway-upstream` owns connection pooling and
    /// rejects transport-controlled headers. Pooled HTTP transport supplies persistence safely.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` if `admitted_target` is not this exact fixed endpoint, or
    /// `InternalError/Internal` if a fixed header violates the shared transport invariant.
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

impl fmt::Debug for GrokBuildResponsesOutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildResponsesOutboundRequest")
            .field("target", &"<redacted>")
            .field(
                "header_names",
                &[
                    "accept",
                    "authorization",
                    "content-type",
                    GROK_BUILD_TOKEN_AUTH_HEADER,
                    GROK_BUILD_CLIENT_VERSION_HEADER,
                    GROK_BUILD_CLIENT_IDENTIFIER_HEADER,
                    GROK_BUILD_CLIENT_MODE_HEADER,
                    GROK_BUILD_AUTHENTICATE_RESPONSE_HEADER,
                    GROK_BUILD_AGENT_ID_HEADER,
                    GROK_BUILD_REQUEST_ID_HEADER,
                    GROK_BUILD_TRACEPARENT_HEADER,
                    "user-agent",
                    GROK_BUILD_MODEL_OVERRIDE_HEADER,
                ],
            )
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Stateless constructor for one Grok Build Responses HTTP request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokBuildResponsesRequestBuilder;

impl GrokBuildResponsesRequestBuilder {
    /// Builds one OAuth-authenticated Grok Build Responses request.
    ///
    /// The selected upstream model, not the public model, is serialized. Request JSON uses the
    /// locked lossless Responses subset also specified for the generic adapter, but is held here
    /// to preserve the no-cross-Provider dependency boundary.
    ///
    /// # Errors
    ///
    /// Returns the generic safe request-construction error for an invalid selected model or an
    /// unsupported Canonical representation. Credential refresh/expiry, network I/O, egress
    /// admission, response parsing, retry and continuity remain outside this constructor.
    pub fn build(
        credential: &GrokBuildCredential,
        upstream_model: &str,
        request: &CanonicalRequest,
        mode: ResponseMode,
    ) -> Result<GrokBuildResponsesOutboundRequest, GatewayError> {
        Self::build_with_cache_identity(credential, upstream_model, request, mode, None)
    }

    /// Builds one OAuth-authenticated Build request with an explicitly derived upstream cache ID.
    ///
    /// The raw Canonical `prompt_cache_key` is never eligible for upstream serialization. When
    /// cache affinity is requested, the caller must first derive a tenant/model-scoped opaque
    /// identity through the P6 continuity boundary.
    ///
    /// # Errors
    ///
    /// Returns the generic safe request-construction error for invalid request semantics, an
    /// unavailable Credential, or a cache-key/derived-identity mismatch.
    pub fn build_with_cache_identity(
        credential: &GrokBuildCredential,
        upstream_model: &str,
        request: &CanonicalRequest,
        mode: ResponseMode,
        cache_identity: Option<&GrokBuildCacheIdentity>,
    ) -> Result<GrokBuildResponsesOutboundRequest, GatewayError> {
        if credential.access_token().is_empty()
            || !credential
                .access_token()
                .bytes()
                .all(|byte| byte.is_ascii_graphic())
        {
            return Err(credential_unavailable_error());
        }
        let endpoint = GrokBuildResponsesEndpoint::try_new()?;
        if upstream_model.is_empty() {
            return Err(provider_protocol_error());
        }
        let accept = match mode {
            ResponseMode::NonStreaming => "application/json",
            ResponseMode::Streaming => "text/event-stream",
        };
        let accept_encoding = match mode {
            ResponseMode::NonStreaming => "gzip",
            ResponseMode::Streaming => "identity",
        };
        Ok(GrokBuildResponsesOutboundRequest {
            target: endpoint.target,
            authorization: Zeroizing::new(format!("Bearer {}", credential.access_token())),
            accept,
            accept_encoding,
            agent_id: process_agent_id()?.to_owned(),
            request_id: random_uuid_v4()?,
            traceparent: random_traceparent()?,
            model_override: upstream_model.to_owned(),
            body: encode_body(upstream_model, request, mode, cache_identity)?,
        })
    }
}

fn encode_body(
    upstream_model: &str,
    request: &CanonicalRequest,
    mode: ResponseMode,
    cache_identity: Option<&GrokBuildCacheIdentity>,
) -> Result<Vec<u8>, GatewayError> {
    let mut root = Map::new();
    root.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    root.insert(
        "stream".to_owned(),
        Value::Bool(matches!(mode, ResponseMode::Streaming)),
    );

    let input = encode_input(&request.messages)?;
    if !input.is_empty() {
        let input = plain_user_text_input(&request.messages).map_or_else(
            || Value::Array(input),
            |text| Value::String(text.to_owned()),
        );
        root.insert("input".to_owned(), input);
    }
    if !request.tools.is_empty() {
        root.insert(
            "tools".to_owned(),
            Value::Array(encode_tools(&request.tools)?),
        );
    }
    if let Some(thinking) = &request.thinking {
        root.insert("reasoning".to_owned(), encode_reasoning(thinking)?);
    }
    match (&request.prompt_cache_key, cache_identity) {
        (Some(_), Some(cache_identity)) => {
            root.insert(
                "prompt_cache_key".to_owned(),
                Value::String(cache_identity.as_str().to_owned()),
            );
        }
        (Some(_), None) | (None, Some(_)) => return Err(provider_protocol_error()),
        (None, None) => {}
    }
    if let Some(prompt_cache_retention) = &request.prompt_cache_retention {
        root.insert(
            "prompt_cache_retention".to_owned(),
            Value::String(prompt_cache_retention.clone()),
        );
    }
    insert_root_extensions(&mut root, &request.extensions)?;
    serde_json::to_vec(&Value::Object(root)).map_err(|_| internal_error())
}

fn plain_user_text_input(messages: &[CanonicalMessage]) -> Option<&str> {
    let [message] = messages else {
        return None;
    };
    if message.role.0 != "user" || !message.extensions.is_empty() {
        return None;
    }
    let [MessageContent::Text(text)] = message.content.as_slice() else {
        return None;
    };
    if !text.extensions.is_empty() {
        return None;
    }
    Some(&text.text)
}

fn process_agent_id() -> Result<&'static str, GatewayError> {
    GROK_BUILD_PROCESS_AGENT_ID
        .get_or_init(|| random_uuid_v4().ok())
        .as_deref()
        .ok_or_else(internal_error)
}

fn random_uuid_v4() -> Result<String, GatewayError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| internal_error())?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let mut value = String::with_capacity(36);
    for (index, byte) in bytes.into_iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            value.push('-');
        }
        append_hex_byte(&mut value, byte);
    }
    Ok(value)
}

fn random_traceparent() -> Result<String, GatewayError> {
    let mut bytes = [0_u8; 24];
    getrandom::fill(&mut bytes).map_err(|_| internal_error())?;

    let mut value = String::with_capacity(55);
    value.push_str("00-");
    for byte in &bytes[..16] {
        append_hex_byte(&mut value, *byte);
    }
    value.push('-');
    for byte in &bytes[16..] {
        append_hex_byte(&mut value, *byte);
    }
    value.push_str("-01");
    Ok(value)
}

fn append_hex_byte(output: &mut String, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push(char::from(HEX[usize::from(byte >> 4)]));
    output.push(char::from(HEX[usize::from(byte & 0x0f)]));
}

fn encode_input(messages: &[CanonicalMessage]) -> Result<Vec<Value>, GatewayError> {
    let mut input = Vec::new();
    for message in messages {
        let role = message.role.0.as_str();
        if !matches!(role, "assistant" | "developer" | "system" | "tool" | "user")
            || message.content.is_empty()
        {
            return Err(provider_protocol_error());
        }
        let contains_tool_item = message.content.iter().any(|content| {
            matches!(
                content,
                MessageContent::ToolCall(_) | MessageContent::ToolResult(_)
            )
        });
        if contains_tool_item && !message.extensions.is_empty() {
            return Err(provider_protocol_error());
        }

        let mut message_parts = Vec::new();
        for content in &message.content {
            match content {
                MessageContent::Text(text) => message_parts.push(encode_text_part(role, text)?),
                MessageContent::Opaque(opaque) => {
                    message_parts.push(encode_opaque_part(opaque.raw(), &opaque.extensions)?);
                }
                MessageContent::ToolCall(call) => {
                    flush_message_parts(&mut input, role, &message.extensions, &mut message_parts)?;
                    if role != "assistant" {
                        return Err(provider_protocol_error());
                    }
                    input.push(encode_tool_call(call)?);
                }
                MessageContent::ToolResult(result) => {
                    flush_message_parts(&mut input, role, &message.extensions, &mut message_parts)?;
                    if role != "tool" || result.is_error {
                        return Err(provider_protocol_error());
                    }
                    input.push(encode_tool_result(result)?);
                }
            }
        }
        flush_message_parts(&mut input, role, &message.extensions, &mut message_parts)?;
    }
    Ok(input)
}

fn flush_message_parts(
    input: &mut Vec<Value>,
    role: &str,
    extensions: &RawExtensions,
    message_parts: &mut Vec<Value>,
) -> Result<(), GatewayError> {
    if message_parts.is_empty() {
        return Ok(());
    }
    if role == "tool" {
        return Err(provider_protocol_error());
    }
    let mut message = Map::new();
    message.insert("type".to_owned(), Value::String("message".to_owned()));
    message.insert("role".to_owned(), Value::String(role.to_owned()));
    message.insert(
        "content".to_owned(),
        Value::Array(std::mem::take(message_parts)),
    );
    insert_extensions(&mut message, extensions, MESSAGE_RESERVED_FIELDS)?;
    input.push(Value::Object(message));
    Ok(())
}

fn encode_text_part(role: &str, text: &gateway_core::TextContent) -> Result<Value, GatewayError> {
    let part_type = match role {
        "assistant" => "output_text",
        "developer" | "system" | "user" => "input_text",
        _ => return Err(provider_protocol_error()),
    };
    let mut part = Map::new();
    part.insert("type".to_owned(), Value::String(part_type.to_owned()));
    part.insert("text".to_owned(), Value::String(text.text.clone()));
    insert_extensions(&mut part, &text.extensions, TEXT_RESERVED_FIELDS)?;
    Ok(Value::Object(part))
}

fn encode_opaque_part(raw: &RawJson, extensions: &RawExtensions) -> Result<Value, GatewayError> {
    let mut part = raw_value(raw)?;
    let Value::Object(ref mut part_object) = part else {
        return Err(provider_protocol_error());
    };
    insert_extensions(part_object, extensions, &[])?;
    Ok(part)
}

fn encode_tool_call(call: &gateway_core::ToolCall) -> Result<Value, GatewayError> {
    if call.id.is_empty() || call.name.is_empty() {
        return Err(provider_protocol_error());
    }
    let mut item = Map::new();
    item.insert("type".to_owned(), Value::String("function_call".to_owned()));
    item.insert("call_id".to_owned(), Value::String(call.id.clone()));
    item.insert("name".to_owned(), Value::String(call.name.clone()));
    item.insert(
        "arguments".to_owned(),
        Value::String(call.arguments.get().to_owned()),
    );
    insert_extensions(&mut item, &call.extensions, TOOL_CALL_RESERVED_FIELDS)?;
    Ok(Value::Object(item))
}

fn encode_tool_result(result: &gateway_core::ToolResult) -> Result<Value, GatewayError> {
    if result.call_id.is_empty() {
        return Err(provider_protocol_error());
    }
    let mut item = Map::new();
    item.insert(
        "type".to_owned(),
        Value::String("function_call_output".to_owned()),
    );
    item.insert("call_id".to_owned(), Value::String(result.call_id.clone()));
    item.insert(
        "output".to_owned(),
        encode_tool_result_output(&result.output)?,
    );
    insert_extensions(&mut item, &result.extensions, TOOL_RESULT_RESERVED_FIELDS)?;
    Ok(Value::Object(item))
}

fn encode_tool_result_output(raw: &RawJson) -> Result<Value, GatewayError> {
    let output = raw_value(raw)?;
    match &output {
        Value::String(_) => Ok(output),
        Value::Array(parts) if parts.iter().all(is_supported_tool_result_content) => Ok(output),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_) | Value::Array(_) => {
            Err(provider_protocol_error())
        }
    }
}

fn is_supported_tool_result_content(part: &Value) -> bool {
    let Some(part) = part.as_object() else {
        return false;
    };
    match part.get("type").and_then(Value::as_str) {
        Some("input_text") => part.get("text").is_some_and(Value::is_string),
        Some("input_image") => {
            has_string_field(part, "image_url") || has_string_field(part, "file_id")
        }
        Some("input_file") => {
            has_string_field(part, "file_data")
                || has_string_field(part, "file_id")
                || has_string_field(part, "file_url")
        }
        _ => false,
    }
}

fn has_string_field(object: &Map<String, Value>, name: &str) -> bool {
    object.get(name).is_some_and(Value::is_string)
}

fn encode_tools(tools: &[ToolDefinition]) -> Result<Vec<Value>, GatewayError> {
    tools.iter().map(encode_tool).collect()
}

fn encode_tool(tool: &ToolDefinition) -> Result<Value, GatewayError> {
    if tool.name.is_empty() {
        return Err(provider_protocol_error());
    }
    let parameters = raw_value(&tool.input_schema)?;
    if !parameters.is_object() {
        return Err(provider_protocol_error());
    }
    let mut encoded = Map::new();
    encoded.insert("type".to_owned(), Value::String("function".to_owned()));
    encoded.insert("name".to_owned(), Value::String(tool.name.clone()));
    if let Some(description) = &tool.description {
        encoded.insert("description".to_owned(), Value::String(description.clone()));
    }
    encoded.insert("parameters".to_owned(), parameters);
    insert_extensions(&mut encoded, &tool.extensions, TOOL_RESERVED_FIELDS)?;
    Ok(Value::Object(encoded))
}

fn encode_reasoning(thinking: &gateway_core::Thinking) -> Result<Value, GatewayError> {
    let mut reasoning = Map::new();
    reasoning.insert(
        "effort".to_owned(),
        Value::String(thinking.effort.as_str().to_owned()),
    );
    insert_extensions(
        &mut reasoning,
        &thinking.extensions,
        REASONING_RESERVED_FIELDS,
    )?;
    Ok(Value::Object(reasoning))
}

fn insert_root_extensions(
    root: &mut Map<String, Value>,
    extensions: &RawExtensions,
) -> Result<(), GatewayError> {
    for (name, raw) in extensions.iter() {
        let Some(name) = name.strip_prefix(OPENAI_RESPONSES_EXTENSION_PREFIX) else {
            return Err(provider_protocol_error());
        };
        insert_extension(root, name, raw, ROOT_RESERVED_FIELDS)?;
    }
    Ok(())
}

fn insert_extensions(
    object: &mut Map<String, Value>,
    extensions: &RawExtensions,
    reserved: &[&str],
) -> Result<(), GatewayError> {
    for (name, raw) in extensions.iter() {
        insert_extension(object, name, raw, reserved)?;
    }
    Ok(())
}

fn insert_extension(
    object: &mut Map<String, Value>,
    name: &str,
    raw: &RawJson,
    reserved: &[&str],
) -> Result<(), GatewayError> {
    if name.is_empty() || reserved.contains(&name) || object.contains_key(name) {
        return Err(provider_protocol_error());
    }
    object.insert(name.to_owned(), raw_value(raw)?);
    Ok(())
}

fn raw_value(raw: &RawJson) -> Result<Value, GatewayError> {
    serde_json::from_str(raw.get()).map_err(|_| provider_protocol_error())
}

/// A bounded signal extracted from an HTTP error body without retaining its text.
///
/// This is syntax-only evidence. P6-07 decides whether a signal changes Credential, Account,
/// Quota Window, Provider, or retry state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildResponsesErrorSignal {
    /// The body contained no recognized error label.
    None,
    /// The body signalled that the free-usage allowance is exhausted.
    FreeUsageExhausted,
    /// The OAuth token endpoint-style error signal was `invalid_grant`.
    InvalidGrant,
    /// The body signalled an invalid or expired access token.
    InvalidToken,
    /// The body had a bounded label, but it is not one of this task's stable signals.
    Unrecognized,
}

/// A redacted parsed Grok Build HTTP failure envelope.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct GrokBuildResponsesHttpError {
    status: u16,
    signal: GrokBuildResponsesErrorSignal,
}

impl GrokBuildResponsesHttpError {
    /// Parses a non-success HTTP response body without retaining raw body text.
    ///
    /// Invalid or non-JSON bodies remain usable as a status-only envelope. A body larger than the
    /// explicit bound is rejected rather than being buffered or inspected.
    ///
    /// # Errors
    ///
    /// Returns `UpstreamProtocolError/Provider` for an invalid status range or an oversized body.
    pub fn parse(status: u16, body: &[u8]) -> Result<Self, GatewayError> {
        if !(100..=599).contains(&status) || (200..=299).contains(&status) {
            return Err(provider_protocol_error());
        }
        if body.len() > MAX_GROK_BUILD_ERROR_BODY_BYTES {
            return Err(provider_protocol_error());
        }

        let signal = match parse_strict_json(body, MAX_GROK_BUILD_ERROR_BODY_BYTES) {
            Ok(Value::Object(object)) => error_signal(&object),
            Ok(
                Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::String(_)
                | Value::Array(_),
            )
            | Err(()) => GrokBuildResponsesErrorSignal::None,
        };
        Ok(Self { status, signal })
    }

    /// Returns the raw HTTP status without assigning a remediation action.
    #[must_use]
    pub const fn status(self) -> u16 {
        self.status
    }

    /// Returns the bounded, redacted body signal.
    #[must_use]
    pub const fn signal(self) -> GrokBuildResponsesErrorSignal {
        self.signal
    }
}

impl fmt::Debug for GrokBuildResponsesHttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildResponsesHttpError")
            .field("status", &self.status)
            .field("signal", &self.signal)
            .field("body", &"<redacted>")
            .finish()
    }
}

/// Decodes a completed non-streaming Build Responses JSON body into a successful Canonical response.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokBuildResponsesDecoder;

impl GrokBuildResponsesDecoder {
    /// Decodes one bounded `response.completed` representation without an HTTP client.
    ///
    /// The response must be completed, contain only representable message/reasoning/function-call
    /// output items, and pass Canonical lifecycle validation. Error HTTP bodies are parsed through
    /// [`GrokBuildResponsesHttpError`] instead.
    ///
    /// # Errors
    ///
    /// Returns a safe provider/stream protocol error for malformed or unrepresentable input.
    pub fn decode_non_streaming(input: &[u8]) -> Result<CanonicalResponse, GatewayError> {
        let value = parse_strict_json(input, MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES)
            .map_err(|()| provider_protocol_error())?;
        let response = value.as_object().ok_or_else(provider_protocol_error)?;
        let output = required_array(response, "output", provider_protocol_error())?;

        let mut state = GrokBuildResponsesDecodeState::default();
        let mut events = Vec::new();
        state.handle_response_created(response, &mut events)?;
        for item in output {
            let item = item.as_object().ok_or_else(stream_protocol_error)?;
            state.handle_output_item_added(item, &mut events)?;
            state.handle_output_item_done(item, &mut events)?;
        }
        state.handle_response_completed(response, &mut events)?;
        CanonicalResponse::try_new(events)
    }

    /// Decodes one bounded non-streaming response after accepting only identity or gzip coding.
    ///
    /// The caller supplies the upstream `Content-Encoding` value without rendering it. A missing
    /// coding is treated as identity. Gzip is decompressed into an independently bounded 1 MiB
    /// buffer before strict JSON parsing; stacked, unknown, malformed, or oversized codings fail
    /// closed as a Provider protocol error.
    ///
    /// # Errors
    ///
    /// Returns a safe provider protocol error for unsupported coding, malformed gzip, an
    /// over-limit compressed/decompressed body, or an unrepresentable Responses object.
    pub fn decode_non_streaming_with_content_encoding(
        content_encoding: Option<&str>,
        input: &[u8],
    ) -> Result<CanonicalResponse, GatewayError> {
        let decoded = decode_non_streaming_content_encoding(content_encoding, input)?;
        Self::decode_non_streaming(&decoded)
    }
}

fn decode_non_streaming_content_encoding(
    content_encoding: Option<&str>,
    input: &[u8],
) -> Result<Vec<u8>, GatewayError> {
    if input.len() > MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES {
        return Err(provider_protocol_error());
    }
    match content_encoding.map(str::trim) {
        None => Ok(input.to_vec()),
        Some(value) if value.eq_ignore_ascii_case("identity") => Ok(input.to_vec()),
        Some(value) if value.eq_ignore_ascii_case("gzip") => {
            let decoder = MultiGzDecoder::new(input);
            let limit = u64::try_from(MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES)
                .map_err(|_| internal_error())?
                .checked_add(1)
                .ok_or_else(internal_error)?;
            let mut output = Vec::new();
            decoder
                .take(limit)
                .read_to_end(&mut output)
                .map_err(|_| provider_protocol_error())?;
            if output.len() > MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES {
                return Err(provider_protocol_error());
            }
            Ok(output)
        }
        Some(_) => Err(provider_protocol_error()),
    }
}

/// Incremental parser for a Grok Build Responses SSE byte stream.
///
/// Arbitrary byte chunking is accepted. Each SSE record is bounded independently, decoded only
/// after its blank-line terminator, and checked against Canonical event lifecycle invariants.
#[derive(Clone, Default)]
pub struct GrokBuildResponsesStreamDecoder {
    pending: Vec<u8>,
    state: GrokBuildResponsesDecodeState,
}

impl GrokBuildResponsesStreamDecoder {
    /// Creates an empty SSE decoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Accepts one arbitrary raw SSE byte chunk and returns newly decoded Canonical events.
    ///
    /// State is committed only after the complete chunk is valid, so a malformed chunk cannot
    /// advance the stream after partially decoded semantic events.
    ///
    /// # Errors
    ///
    /// Returns `UpstreamProtocolError/Stream` for invalid framing, duplicate JSON fields,
    /// unsupported event/item semantics, or an oversized record.
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
                if record_length > MAX_GROK_BUILD_SSE_FRAME_BYTES {
                    return Err(stream_protocol_error());
                }
                let mut record = std::mem::take(&mut pending);
                record.truncate(record_length);
                state.handle_sse_record(&record, &mut events)?;
            } else if pending.len() > MAX_GROK_BUILD_SSE_FRAME_BYTES + 4 {
                return Err(stream_protocol_error());
            }
        }

        self.pending = pending;
        self.state = state;
        Ok(events)
    }

    /// Verifies that the byte source ended on an SSE-record boundary after a terminal event.
    ///
    /// # Errors
    ///
    /// Returns `StreamTruncated/Stream` for a partial SSE record or a response that never reached
    /// `response.completed`/`response.failed`.
    pub fn finish(&self) -> Result<(), GatewayError> {
        if !self.pending.is_empty() {
            return Err(stream_truncated_error());
        }
        self.state.canonical.finish()
    }
}

impl fmt::Debug for GrokBuildResponsesStreamDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildResponsesStreamDecoder")
            .field("pending_byte_count", &self.pending.len())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Default)]
struct GrokBuildResponsesDecodeState {
    canonical: CanonicalEventState,
    response_id: Option<String>,
    message_open: bool,
    item_kinds: BTreeMap<String, OutputItemKind>,
    done_item_ids: BTreeSet<String>,
    function_call_ids: BTreeMap<String, String>,
    function_call_names: BTreeMap<String, String>,
    function_arguments: BTreeMap<String, String>,
    completed_function_calls: BTreeSet<String>,
    text_by_item_id: BTreeMap<String, String>,
    reasoning_by_item_id: BTreeMap<String, String>,
    active_text_content_item_ids: BTreeSet<String>,
}

impl fmt::Debug for GrokBuildResponsesDecodeState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildResponsesDecodeState")
            .field("canonical", &self.canonical)
            .field("response_started", &self.response_id.is_some())
            .field("message_open", &self.message_open)
            .field("output_item_count", &self.item_kinds.len())
            .field("completed_item_count", &self.done_item_ids.len())
            .field("function_call_count", &self.function_call_ids.len())
            .field("function_call_name_count", &self.function_call_names.len())
            .field("function_arguments_count", &self.function_arguments.len())
            .field(
                "completed_function_call_count",
                &self.completed_function_calls.len(),
            )
            .field("text_item_count", &self.text_by_item_id.len())
            .field("reasoning_item_count", &self.reasoning_by_item_id.len())
            .field(
                "active_text_content_item_count",
                &self.active_text_content_item_ids.len(),
            )
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputItemKind {
    Message,
    Reasoning,
    FunctionCall,
}

impl GrokBuildResponsesDecodeState {
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
        let value = parse_strict_json(data.as_bytes(), MAX_GROK_BUILD_SSE_FRAME_BYTES)
            .map_err(|()| stream_protocol_error())?;
        let object = value.as_object().ok_or_else(stream_protocol_error)?;
        if required_string(object, "type", stream_protocol_error())? != event_name {
            return Err(stream_protocol_error());
        }

        match event_name.as_str() {
            "keepalive" => Ok(()),
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
            "response.output_item.done" => self.handle_output_item_done(
                required_object(object, "item", stream_protocol_error())?,
                events,
            ),
            "response.content_part.added" => self.handle_text_content_part_added(object),
            "response.content_part.done" => self.handle_text_content_part_done(object, events),
            "response.output_text.delta" => self.handle_text_delta(object, events),
            "response.output_text.done" => self.handle_text_done(object, events),
            "response.reasoning.delta"
            | "response.reasoning_text.delta"
            | "response.reasoning_summary_text.delta" => {
                self.handle_reasoning_delta(object, events)
            }
            "response.reasoning_summary_text.done" | "response.reasoning.done" => {
                self.handle_reasoning_terminal_text(object, events)
            }
            "response.reasoning_summary_part.added" | "response.reasoning_summary_part.done" => {
                self.handle_reasoning_summary_part(object)
            }
            "response.function_call_arguments.delta" => {
                self.handle_function_arguments_delta(object, events)
            }
            "response.function_call_arguments.done" => {
                self.handle_function_arguments_done(object, events)
            }
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
        let item_id = required_identifier(item, "id", stream_protocol_error())?;
        if self.item_kinds.contains_key(item_id) {
            return Err(stream_protocol_error());
        }

        let kind = match required_string(item, "type", stream_protocol_error())? {
            "message" => OutputItemKind::Message,
            "reasoning" => OutputItemKind::Reasoning,
            "function_call" => OutputItemKind::FunctionCall,
            _ => return Err(stream_protocol_error()),
        };
        self.ensure_message(events)?;

        if kind == OutputItemKind::FunctionCall {
            let call_id = required_identifier(item, "call_id", stream_protocol_error())?;
            let name = required_identifier(item, "name", stream_protocol_error())?;
            if self
                .function_call_ids
                .values()
                .any(|existing| existing == call_id)
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
        self.item_kinds.insert(item_id.to_owned(), kind);
        Ok(())
    }

    fn handle_output_item_done(
        &mut self,
        item: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(item, "id", stream_protocol_error())?;
        let Some(kind) = self.item_kinds.get(item_id).copied() else {
            return Err(stream_protocol_error());
        };
        if self.done_item_ids.contains(item_id) {
            return Err(stream_protocol_error());
        }
        if self.active_text_content_item_ids.contains(item_id) {
            return Err(stream_protocol_error());
        }
        let expected_kind = match kind {
            OutputItemKind::Message => "message",
            OutputItemKind::Reasoning => "reasoning",
            OutputItemKind::FunctionCall => "function_call",
        };
        if required_string(item, "type", stream_protocol_error())? != expected_kind {
            return Err(stream_protocol_error());
        }
        if required_string(item, "status", stream_protocol_error())? != "completed" {
            return Err(stream_protocol_error());
        }

        match kind {
            OutputItemKind::Message => {
                let text = output_text(item, "output_text")?;
                self.finish_text_item(item_id, text, events)?;
            }
            OutputItemKind::Reasoning => {
                // Current Build can return a completed reasoning item with no `content`. It
                // represents non-exported internal reasoning, not an empty visible text item;
                // retain lifecycle/usage while emitting no synthetic `ReasoningDelta`.
                if let Some(text) = optional_reasoning_text(item)? {
                    self.finish_reasoning_item(item_id, text, events)?;
                }
            }
            OutputItemKind::FunctionCall => {
                let call_id = required_identifier(item, "call_id", stream_protocol_error())?;
                if self.function_call_ids.get(item_id).map(String::as_str) != Some(call_id) {
                    return Err(stream_protocol_error());
                }
                let name = required_identifier(item, "name", stream_protocol_error())?;
                if self.function_call_names.get(item_id).map(String::as_str) != Some(name) {
                    return Err(stream_protocol_error());
                }
                let arguments = required_string(item, "arguments", stream_protocol_error())?;
                self.finish_function_call(call_id, arguments, events)?;
            }
        }
        self.done_item_ids.insert(item_id.to_owned());
        Ok(())
    }

    fn handle_text_delta(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(item_id) != Some(&OutputItemKind::Message) {
            return Err(stream_protocol_error());
        }
        let delta = required_string(event, "delta", stream_protocol_error())?;
        if delta.is_empty() {
            return Ok(());
        }
        self.ensure_message(events)?;
        self.emit(
            events,
            CanonicalEvent::TextDelta(TextDelta {
                text: delta.to_owned(),
                extensions: RawExtensions::default(),
            }),
        )?;
        self.text_by_item_id
            .entry(item_id.to_owned())
            .or_default()
            .push_str(delta);
        Ok(())
    }

    fn handle_text_content_part_added(
        &mut self,
        event: &Map<String, Value>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(item_id) != Some(&OutputItemKind::Message) {
            return Err(stream_protocol_error());
        }
        let part = required_object(event, "part", stream_protocol_error())?;
        if required_string(part, "type", stream_protocol_error())? != "output_text"
            || !required_string(part, "text", stream_protocol_error())?.is_empty()
            || !self.active_text_content_item_ids.insert(item_id.to_owned())
        {
            return Err(stream_protocol_error());
        }
        Ok(())
    }

    fn handle_text_content_part_done(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(item_id) != Some(&OutputItemKind::Message)
            || !self.active_text_content_item_ids.contains(item_id)
        {
            return Err(stream_protocol_error());
        }
        let part = required_object(event, "part", stream_protocol_error())?;
        if required_string(part, "type", stream_protocol_error())? != "output_text" {
            return Err(stream_protocol_error());
        }
        let text = required_string(part, "text", stream_protocol_error())?.to_owned();
        self.finish_text_item(item_id, text, events)?;
        self.active_text_content_item_ids.remove(item_id);
        Ok(())
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
        let text = required_string(event, "text", stream_protocol_error())?.to_owned();
        self.finish_text_item(item_id, text, events)
    }

    fn handle_reasoning_delta(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(item_id) != Some(&OutputItemKind::Reasoning) {
            return Err(stream_protocol_error());
        }
        let delta = required_string(event, "delta", stream_protocol_error())?;
        if delta.is_empty() {
            return Ok(());
        }
        self.ensure_message(events)?;
        self.emit(
            events,
            CanonicalEvent::ReasoningDelta(ReasoningDelta {
                text: delta.to_owned(),
                extensions: RawExtensions::default(),
            }),
        )?;
        self.reasoning_by_item_id
            .entry(item_id.to_owned())
            .or_default()
            .push_str(delta);
        Ok(())
    }

    fn handle_reasoning_summary_part(
        &self,
        event: &Map<String, Value>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(item_id) != Some(&OutputItemKind::Reasoning) {
            return Err(stream_protocol_error());
        }
        let _part = required_object(event, "part", stream_protocol_error())?;
        Ok(())
    }

    fn handle_reasoning_terminal_text(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        if self.item_kinds.get(item_id) != Some(&OutputItemKind::Reasoning) {
            return Err(stream_protocol_error());
        }
        let text = required_string(event, "text", stream_protocol_error())?;
        let accumulated = self
            .reasoning_by_item_id
            .get(item_id)
            .map(String::as_str)
            .unwrap_or_default();
        if accumulated.is_empty() && !text.is_empty() {
            self.ensure_message(events)?;
            self.emit(
                events,
                CanonicalEvent::ReasoningDelta(ReasoningDelta {
                    text: text.to_owned(),
                    extensions: RawExtensions::default(),
                }),
            )?;
            self.reasoning_by_item_id
                .insert(item_id.to_owned(), text.to_owned());
            return Ok(());
        }
        if accumulated != text {
            return Err(stream_protocol_error());
        }
        Ok(())
    }

    fn handle_function_arguments_delta(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        let call_id = required_identifier(event, "call_id", stream_protocol_error())?;
        if self.function_call_ids.get(item_id).map(String::as_str) != Some(call_id)
            || self.completed_function_calls.contains(call_id)
        {
            return Err(stream_protocol_error());
        }
        let delta = required_string(event, "delta", stream_protocol_error())?;
        if delta.is_empty() {
            return Ok(());
        }
        let next_length = self
            .function_arguments
            .get(call_id)
            .map_or(0, String::len)
            .checked_add(delta.len())
            .ok_or_else(stream_protocol_error)?;
        if next_length > MAX_GROK_BUILD_TOOL_ARGUMENT_BYTES {
            return Err(stream_protocol_error());
        }
        self.emit(
            events,
            CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
                call_id: call_id.to_owned(),
                delta: delta.to_owned(),
                extensions: RawExtensions::default(),
            }),
        )?;
        self.function_arguments
            .entry(call_id.to_owned())
            .or_default()
            .push_str(delta);
        Ok(())
    }

    fn handle_function_arguments_done(
        &mut self,
        event: &Map<String, Value>,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_identifier(event, "item_id", stream_protocol_error())?;
        let call_id = required_identifier(event, "call_id", stream_protocol_error())?;
        if self.function_call_ids.get(item_id).map(String::as_str) != Some(call_id)
            || self.done_item_ids.contains(item_id)
        {
            return Err(stream_protocol_error());
        }
        let arguments = required_string(event, "arguments", stream_protocol_error())?;
        self.finish_function_call(call_id, arguments, events)
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
        let mut completed_output_item_ids = BTreeSet::new();
        for item in output {
            let item = item.as_object().ok_or_else(stream_protocol_error)?;
            let item_id = required_identifier(item, "id", stream_protocol_error())?;
            if !completed_output_item_ids.insert(item_id.to_owned())
                || !self.done_item_ids.contains(item_id)
            {
                return Err(stream_protocol_error());
            }
        }
        if completed_output_item_ids != self.done_item_ids {
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
        self.emit(events, CanonicalEvent::ResponseEnd(ResponseEnd::default()))
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

    fn finish_text_item(
        &mut self,
        item_id: &str,
        final_text: String,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let emitted = self.text_by_item_id.get(item_id).map_or("", String::as_str);
        if !emitted.is_empty() && emitted != final_text {
            return Err(stream_protocol_error());
        }
        if emitted.is_empty() && !final_text.is_empty() {
            self.emit(
                events,
                CanonicalEvent::TextDelta(TextDelta {
                    text: final_text.clone(),
                    extensions: RawExtensions::default(),
                }),
            )?;
            self.text_by_item_id.insert(item_id.to_owned(), final_text);
        }
        Ok(())
    }

    fn finish_reasoning_item(
        &mut self,
        item_id: &str,
        final_text: String,
        events: &mut Vec<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let emitted = self
            .reasoning_by_item_id
            .get(item_id)
            .map_or("", String::as_str);
        if !emitted.is_empty() && emitted != final_text {
            return Err(stream_protocol_error());
        }
        if emitted.is_empty() && !final_text.is_empty() {
            self.emit(
                events,
                CanonicalEvent::ReasoningDelta(ReasoningDelta {
                    text: final_text.clone(),
                    extensions: RawExtensions::default(),
                }),
            )?;
            self.reasoning_by_item_id
                .insert(item_id.to_owned(), final_text);
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

        let raw_arguments =
            RawJson::from_json_string(arguments.clone()).map_err(|_| stream_protocol_error())?;
        self.emit(
            events,
            CanonicalEvent::ToolCallEnd(ToolCallEnd {
                call_id: call_id.to_owned(),
                arguments: raw_arguments,
                extensions: RawExtensions::default(),
            }),
        )?;
        self.function_arguments
            .insert(call_id.to_owned(), arguments);
        self.completed_function_calls.insert(call_id.to_owned());
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

fn normalize_tool_arguments(arguments: &str) -> Result<String, GatewayError> {
    if arguments.len() > MAX_GROK_BUILD_TOOL_ARGUMENT_BYTES {
        return Err(stream_protocol_error());
    }
    if arguments.trim().is_empty() {
        return Ok("{}".to_owned());
    }
    let value = parse_strict_json(arguments.as_bytes(), MAX_GROK_BUILD_TOOL_ARGUMENT_BYTES)
        .map_err(|()| stream_protocol_error())?;
    match value {
        Value::Object(object) if object.is_empty() => Ok("{}".to_owned()),
        Value::Object(_) => Ok(arguments.to_owned()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            Err(stream_protocol_error())
        }
    }
}

fn output_text(item: &Map<String, Value>, expected_type: &str) -> Result<String, GatewayError> {
    let content = required_array(item, "content", stream_protocol_error())?;
    let mut text = String::new();
    for part in content {
        let part = part.as_object().ok_or_else(stream_protocol_error)?;
        if required_string(part, "type", stream_protocol_error())? != expected_type {
            return Err(stream_protocol_error());
        }
        text.push_str(required_string(part, "text", stream_protocol_error())?);
    }
    Ok(text)
}

fn optional_reasoning_text(item: &Map<String, Value>) -> Result<Option<String>, GatewayError> {
    let Some(content) = item.get("content") else {
        return Ok(None);
    };
    let content = content.as_array().ok_or_else(stream_protocol_error)?;
    let mut text = String::new();
    for part in content {
        let part = part.as_object().ok_or_else(stream_protocol_error)?;
        if required_string(part, "type", stream_protocol_error())? != "reasoning_text" {
            return Err(stream_protocol_error());
        }
        text.push_str(required_string(part, "text", stream_protocol_error())?);
    }
    Ok(Some(text))
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
        Some(Value::Number(value)) => value.as_u64().map(Some).ok_or_else(|| error.clone()),
        Some(Value::Bool(_) | Value::String(_) | Value::Array(_) | Value::Object(_)) => {
            Err(error.clone())
        }
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
    if value.is_empty() || value.len() > MAX_GROK_BUILD_IDENTIFIER_BYTES {
        return Err(error);
    }
    Ok(value)
}

fn error_signal(object: &Map<String, Value>) -> GrokBuildResponsesErrorSignal {
    let mut labels = Vec::new();
    for field in ["code", "type"] {
        if let Some(value) = object.get(field).and_then(Value::as_str) {
            labels.push(value);
        }
    }
    match object.get("error") {
        Some(Value::String(value)) => labels.push(value),
        Some(Value::Object(error)) => {
            for field in ["code", "type"] {
                if let Some(value) = error.get(field).and_then(Value::as_str) {
                    labels.push(value);
                }
            }
        }
        None | Some(Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_)) => {}
    }

    if labels.is_empty() {
        return GrokBuildResponsesErrorSignal::None;
    }
    for label in labels {
        let label = label.to_ascii_lowercase();
        if label.contains("free-usage-exhausted") || label.contains("included free usage") {
            return GrokBuildResponsesErrorSignal::FreeUsageExhausted;
        }
        if label == "invalid_grant" {
            return GrokBuildResponsesErrorSignal::InvalidGrant;
        }
        if label.contains("invalid_token") || label.contains("expired_token") {
            return GrokBuildResponsesErrorSignal::InvalidToken;
        }
    }
    GrokBuildResponsesErrorSignal::Unrecognized
}

fn sse_delimiter_length(buffer: &[u8]) -> Option<usize> {
    if buffer.ends_with(b"\r\n\r\n") {
        Some(4)
    } else if buffer.ends_with(b"\n\n") {
        Some(2)
    } else {
        None
    }
}

const fn egress_rejected_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
}

const fn credential_unavailable_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::CredentialUnavailable,
        ErrorScope::Credential,
    )
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
