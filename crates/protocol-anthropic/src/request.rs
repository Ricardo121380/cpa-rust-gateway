use std::fmt;

use gateway_core::{
    CanonicalMessage, CanonicalRequest, MessageContent, MessageRole, OpaqueContent, RawExtensions,
    RawJson, TextContent, ToolCall, ToolDefinition, ToolResult,
};
use serde_json::{Map, Value};

use crate::json::{
    array, client_request_error, extensions_except, object, raw_json, reject_duplicate_names,
    required_string, required_value,
};

const ROOT_FIELDS: &[&str] = &["model", "messages", "system", "tools", "stream"];

/// The output representation requested by an Anthropic Messages client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseMode {
    /// Return one completed JSON message.
    NonStreaming,
    /// Return typed Server-Sent Event frames.
    Streaming,
}

/// One decoded Anthropic Messages request without HTTP or routing concerns.
#[derive(Clone, Eq, PartialEq)]
pub struct DecodedMessagesRequest {
    /// Protocol-neutral request data.
    pub request: CanonicalRequest,
    /// Requested response representation.
    pub mode: ResponseMode,
}

impl fmt::Debug for DecodedMessagesRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecodedMessagesRequest")
            .field("request", &self.request)
            .field("mode", &self.mode)
            .finish()
    }
}

/// One decoded Anthropic `count_tokens` request without HTTP or Provider concerns.
#[derive(Clone, Eq, PartialEq)]
pub struct DecodedCountTokensRequest {
    /// Protocol-neutral request data whose input-token count is requested.
    pub request: CanonicalRequest,
}

impl fmt::Debug for DecodedCountTokensRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecodedCountTokensRequest")
            .field("request", &self.request)
            .finish()
    }
}

/// Decodes one complete Anthropic Messages JSON request.
///
/// Unknown fields are retained in explicit `anthropic.*` extension namespaces. This codec does
/// not claim that a later selected Provider can execute every retained field.
///
/// # Errors
///
/// Returns `ClientRequestError/Request` for malformed, ambiguous, or structurally invalid input.
pub fn decode_request(input: &str) -> Result<DecodedMessagesRequest, gateway_core::GatewayError> {
    let (request, mode) = decode_canonical_request(input, true)?;

    Ok(DecodedMessagesRequest { request, mode })
}

/// Decodes an Anthropic `count_tokens` request into the same canonical input representation.
///
/// Unlike the Messages inference endpoint, this endpoint does not require `max_tokens`: no output
/// is generated. A supplied `stream: true` is rejected because a token-count result has no SSE
/// representation. Unknown fields continue to be retained as raw extensions for later capability
/// analysis rather than being silently discarded.
///
/// # Errors
///
/// Returns `ClientRequestError/Request` for malformed, ambiguous, streaming, or structurally
/// invalid input.
pub fn decode_count_tokens_request(
    input: &str,
) -> Result<DecodedCountTokensRequest, gateway_core::GatewayError> {
    let (request, mode) = decode_canonical_request(input, false)?;
    if mode == ResponseMode::Streaming {
        return Err(client_request_error());
    }

    Ok(DecodedCountTokensRequest { request })
}

fn decode_canonical_request(
    input: &str,
    requires_max_tokens: bool,
) -> Result<(CanonicalRequest, ResponseMode), gateway_core::GatewayError> {
    reject_duplicate_names(input)?;
    let value: Value = serde_json::from_str(input).map_err(|_| client_request_error())?;
    let root = object(&value)?;
    let requested_model = required_string(root, "model")?.to_owned();
    if requested_model.is_empty() {
        return Err(client_request_error());
    }
    if requires_max_tokens {
        validate_max_tokens(root)?;
    }

    let mode = match root.get("stream") {
        None | Some(Value::Bool(false)) => ResponseMode::NonStreaming,
        Some(Value::Bool(true)) => ResponseMode::Streaming,
        Some(_) => return Err(client_request_error()),
    };

    let mut messages = decode_system(root.get("system"))?;
    let external_messages = array(required_value(root, "messages")?)?;
    if external_messages.is_empty() {
        return Err(client_request_error());
    }
    for message in external_messages {
        messages.extend(decode_message(message)?);
    }

    let tools = root
        .get("tools")
        .map_or_else(|| Ok(Vec::new()), decode_tools)?;
    let extensions = extensions_except(root, ROOT_FIELDS, "anthropic.messages.")?;

    Ok((
        CanonicalRequest {
            requested_model,
            messages,
            tools,
            thinking: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            extensions,
        },
        mode,
    ))
}

fn validate_max_tokens(root: &Map<String, Value>) -> Result<(), gateway_core::GatewayError> {
    match root.get("max_tokens").and_then(Value::as_u64) {
        Some(value) if value > 0 => Ok(()),
        _ => Err(client_request_error()),
    }
}

fn decode_system(
    value: Option<&Value>,
) -> Result<Vec<CanonicalMessage>, gateway_core::GatewayError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let content = decode_content(value, ContentContext::System)?;
    if content.is_empty() {
        return Err(client_request_error());
    }
    Ok(vec![CanonicalMessage {
        role: MessageRole("system".to_owned()),
        content,
        extensions: RawExtensions::default(),
    }])
}

fn decode_message(value: &Value) -> Result<Vec<CanonicalMessage>, gateway_core::GatewayError> {
    let message = object(value)?;
    let role = required_string(message, "role")?;
    if !matches!(role, "user" | "assistant") {
        return Err(client_request_error());
    }
    let block_context = if role == "user" {
        ContentContext::User
    } else {
        ContentContext::Assistant
    };
    let content = decode_content(required_value(message, "content")?, block_context)?;
    if content.is_empty() {
        return Err(client_request_error());
    }
    let message_extensions = extensions_except(message, &["role", "content"], "anthropic.")?;
    split_tool_results(role, content, message_extensions)
}

fn split_tool_results(
    role: &str,
    content: Vec<MessageContent>,
    message_extensions: RawExtensions,
) -> Result<Vec<CanonicalMessage>, gateway_core::GatewayError> {
    if role == "assistant"
        && content
            .iter()
            .any(|part| matches!(part, MessageContent::ToolResult(_)))
    {
        return Err(client_request_error());
    }
    if role == "user"
        && content
            .iter()
            .any(|part| matches!(part, MessageContent::ToolCall(_)))
    {
        return Err(client_request_error());
    }

    let mut messages = Vec::new();
    let mut ordinary = Vec::new();
    let mut retained_extensions = Some(message_extensions);
    for part in content {
        match part {
            MessageContent::ToolResult(result) => {
                if !ordinary.is_empty() {
                    flush_message(
                        &mut messages,
                        role,
                        &mut ordinary,
                        retained_extensions.take().unwrap_or_default(),
                    );
                }
                messages.push(CanonicalMessage {
                    role: MessageRole("tool".to_owned()),
                    content: vec![MessageContent::ToolResult(result)],
                    extensions: retained_extensions.take().unwrap_or_default(),
                });
            }
            other => ordinary.push(other),
        }
    }
    if !ordinary.is_empty() {
        flush_message(
            &mut messages,
            role,
            &mut ordinary,
            retained_extensions.take().unwrap_or_default(),
        );
    }
    Ok(messages)
}

fn flush_message(
    messages: &mut Vec<CanonicalMessage>,
    role: &str,
    content: &mut Vec<MessageContent>,
    extensions: RawExtensions,
) {
    if !content.is_empty() {
        messages.push(CanonicalMessage {
            role: MessageRole(role.to_owned()),
            content: std::mem::take(content),
            extensions,
        });
    }
}

#[derive(Clone, Copy)]
enum ContentContext {
    System,
    User,
    Assistant,
}

fn decode_content(
    value: &Value,
    context: ContentContext,
) -> Result<Vec<MessageContent>, gateway_core::GatewayError> {
    match value {
        Value::String(text) => Ok(vec![text_content(text.clone(), RawExtensions::default())]),
        Value::Array(parts) => parts
            .iter()
            .map(|part| decode_content_block(part, context))
            .collect(),
        _ => Err(client_request_error()),
    }
}

fn decode_content_block(
    value: &Value,
    context: ContentContext,
) -> Result<MessageContent, gateway_core::GatewayError> {
    let block = object(value)?;
    match block.get("type").and_then(Value::as_str) {
        Some("text") => Ok(text_content(
            required_string(block, "text")?.to_owned(),
            extensions_except(block, &["type", "text"], "anthropic.")?,
        )),
        Some("tool_use") => {
            if !matches!(context, ContentContext::Assistant) {
                return Err(client_request_error());
            }
            let id = required_string(block, "id")?.to_owned();
            let name = required_string(block, "name")?.to_owned();
            if id.is_empty() || name.is_empty() {
                return Err(client_request_error());
            }
            Ok(MessageContent::ToolCall(ToolCall {
                id,
                name,
                arguments: raw_json(required_value(block, "input")?)?,
                extensions: extensions_except(
                    block,
                    &["type", "id", "name", "input"],
                    "anthropic.",
                )?,
            }))
        }
        Some("tool_result") => {
            if !matches!(context, ContentContext::User) {
                return Err(client_request_error());
            }
            let call_id = required_string(block, "tool_use_id")?.to_owned();
            if call_id.is_empty() {
                return Err(client_request_error());
            }
            let output = raw_json(required_value(block, "content")?)?;
            let is_error = match block.get("is_error") {
                None => false,
                Some(Value::Bool(value)) => *value,
                Some(_) => return Err(client_request_error()),
            };
            Ok(MessageContent::ToolResult(ToolResult {
                call_id,
                output,
                is_error,
                extensions: extensions_except(
                    block,
                    &["type", "tool_use_id", "content", "is_error"],
                    "anthropic.",
                )?,
            }))
        }
        Some(_) => Ok(MessageContent::Opaque(OpaqueContent::new(raw_json(value)?))),
        None => Err(client_request_error()),
    }
}

fn text_content(text: String, extensions: RawExtensions) -> MessageContent {
    MessageContent::Text(TextContent { text, extensions })
}

fn decode_tools(value: &Value) -> Result<Vec<ToolDefinition>, gateway_core::GatewayError> {
    array(value)?.iter().map(decode_tool).collect()
}

fn decode_tool(value: &Value) -> Result<ToolDefinition, gateway_core::GatewayError> {
    let tool = object(value)?;
    let name = required_string(tool, "name")?.to_owned();
    if name.is_empty() {
        return Err(client_request_error());
    }
    let description = match tool.get("description") {
        None => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => return Err(client_request_error()),
    };
    let input_schema = match tool.get("input_schema") {
        None => RawJson::from_json_string("{}".to_owned()).map_err(|_| client_request_error())?,
        Some(Value::Object(_)) => raw_json(required_value(tool, "input_schema")?)?,
        Some(_) => return Err(client_request_error()),
    };
    Ok(ToolDefinition {
        name,
        description,
        input_schema,
        extensions: extensions_except(
            tool,
            &["name", "description", "input_schema"],
            "anthropic.",
        )?,
    })
}

#[cfg(test)]
mod tests {
    use gateway_core::MessageContent;

    use super::{ResponseMode, decode_count_tokens_request, decode_request};

    #[test]
    fn request_fixture_maps_to_canonical_snapshot() -> Result<(), Box<dyn std::error::Error>> {
        let decoded = decode_request(include_str!(
            "../../../tests/fixtures/anthropic/messages-request.json"
        ))?;
        assert_eq!(decoded.mode, ResponseMode::Streaming);
        let actual = serde_json::to_value(&decoded.request)?;
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/anthropic/messages-request-canonical.json"
        ))?;
        assert_eq!(actual, expected);
        assert!(matches!(
            decoded.request.messages[2].content[1],
            MessageContent::ToolCall(_)
        ));
        assert!(matches!(
            decoded.request.messages[3].content[0],
            MessageContent::ToolResult(_)
        ));
        Ok(())
    }

    #[test]
    fn rejects_ambiguous_or_invalid_messages_requests() {
        for input in [
            r#"{"model":"m","model":"other","max_tokens":1,"messages":[{"role":"user","content":"x"}]}"#,
            r#"{"model":"m","max_tokens":0,"messages":[{"role":"user","content":"x"}]}"#,
            r#"{"model":"m","max_tokens":1,"messages":[]}"#,
            r#"{"model":"m","max_tokens":1,"messages":[{"role":"system","content":"x"}]}"#,
            r#"{"model":"m","max_tokens":1,"messages":[{"role":"user","content":[{"type":"tool_use","id":"x","name":"n","input":{}}]}]}"#,
            r#"{"model":"m","max_tokens":1,"messages":[{"role":"user","content":"x"}],"metadata":{"key":1,"key":2}}"#,
        ] {
            assert!(decode_request(input).is_err(), "accepted {input}");
        }
    }

    #[test]
    fn split_tool_result_retains_message_extensions() -> Result<(), gateway_core::GatewayError> {
        let decoded = decode_request(
            r#"{
                "model":"m",
                "max_tokens":1,
                "messages":[{
                    "role":"user",
                    "content":[{"type":"tool_result","tool_use_id":"call","content":"ok"}],
                    "vendor_message":{"keep":true}
                }]
            }"#,
        )?;

        assert_eq!(decoded.request.messages.len(), 1);
        assert_eq!(decoded.request.messages[0].role.0, "tool");
        assert_eq!(
            decoded.request.messages[0]
                .extensions
                .get("anthropic.vendor_message")
                .map(gateway_core::RawJson::get),
            Some(r#"{"keep":true}"#)
        );
        Ok(())
    }

    #[test]
    fn debug_redacts_request_values() -> Result<(), gateway_core::GatewayError> {
        let decoded = decode_request(include_str!(
            "../../../tests/fixtures/anthropic/messages-request.json"
        ))?;
        let diagnostic = format!("{decoded:?}");
        for value in ["gateway-claude", "lookup", "call-01", "secret prompt"] {
            assert!(!diagnostic.contains(value));
        }
        Ok(())
    }

    #[test]
    fn count_tokens_uses_the_same_canonical_input_without_an_output_limit()
    -> Result<(), gateway_core::GatewayError> {
        let decoded = decode_count_tokens_request(
            r#"{
                "model":"gateway-claude",
                "system":"Follow the policy.",
                "messages":[{"role":"user","content":"count this"}],
                "metadata":{"request":"kept"}
            }"#,
        )?;

        assert_eq!(decoded.request.requested_model, "gateway-claude");
        assert_eq!(decoded.request.messages.len(), 2);
        assert!(
            decoded
                .request
                .extensions
                .get("anthropic.messages.metadata")
                .is_some()
        );
        assert!(
            decode_count_tokens_request(
                r#"{"model":"m","messages":[{"role":"user","content":"x"}],"stream":true}"#
            )
            .is_err()
        );
        Ok(())
    }
}
