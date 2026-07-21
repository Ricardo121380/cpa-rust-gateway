use std::fmt;

use gateway_core::{
    CanonicalMessage, CanonicalRequest, MessageContent, MessageRole, OpaqueContent, RawExtensions,
    RawJson, TextContent, Thinking, ThinkingEffort, ToolCall, ToolDefinition, ToolResult,
};
use serde_json::{Map, Value};

use crate::json::{
    array, client_request_error, extensions_except, object, raw_json, reject_duplicate_names,
    required_string, required_value,
};

const ROOT_FIELDS: &[&str] = &["model", "messages", "system", "tools", "stream", "thinking"];

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

/// Collects the one Canonical request-level retention semantic that can be proven from Anthropic
/// cache controls. The original per-block controls remain attached to their content/tool raw
/// extension so a later bridge cannot mistake this summary for a placement-preserving conversion.
#[derive(Default)]
struct CacheControlCollector {
    retention: Option<String>,
}

impl CacheControlCollector {
    fn observe(&mut self, value: &Value) -> Result<(), gateway_core::GatewayError> {
        let control = object(value)?;
        if required_string(control, "type")? != "ephemeral" {
            return Err(client_request_error());
        }
        let retention = match control.get("ttl") {
            None => "ephemeral".to_owned(),
            Some(Value::String(value)) if !value.is_empty() => value.clone(),
            Some(_) => return Err(client_request_error()),
        };
        match self.retention.as_deref() {
            None => self.retention = Some(retention),
            Some(existing) if existing == retention => {}
            Some(_) => return Err(client_request_error()),
        }
        Ok(())
    }

    fn into_retention(self) -> Option<String> {
        self.retention
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

    let mut cache_controls = CacheControlCollector::default();
    let mut messages = decode_system(root.get("system"), &mut cache_controls)?;
    let external_messages = array(required_value(root, "messages")?)?;
    if external_messages.is_empty() {
        return Err(client_request_error());
    }
    for message in external_messages {
        messages.extend(decode_message(message, &mut cache_controls)?);
    }

    let tools = root.get("tools").map_or_else(
        || Ok(Vec::new()),
        |value| decode_tools(value, &mut cache_controls),
    )?;
    let thinking = root.get("thinking").map(decode_thinking).transpose()?;
    let extensions = extensions_except(root, ROOT_FIELDS, "anthropic.messages.")?;

    Ok((
        CanonicalRequest {
            requested_model,
            messages,
            tools,
            thinking,
            prompt_cache_key: None,
            prompt_cache_retention: cache_controls.into_retention(),
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

fn decode_thinking(value: &Value) -> Result<Thinking, gateway_core::GatewayError> {
    let thinking = object(value)?;
    let effort = ThinkingEffort::try_new(required_string(thinking, "type")?.to_owned())
        .map_err(|_| client_request_error())?;
    let mut extensions =
        extensions_except(thinking, &["type", "budget_tokens"], "anthropic.thinking.")?;
    if let Some(budget_tokens) = thinking.get("budget_tokens") {
        if budget_tokens.as_u64().is_none_or(|tokens| tokens == 0) {
            return Err(client_request_error());
        }
        extensions
            .try_insert("anthropic.thinking.budget_tokens", raw_json(budget_tokens)?)
            .map_err(|_| client_request_error())?;
    }

    Ok(Thinking { effort, extensions })
}

fn decode_system(
    value: Option<&Value>,
    cache_controls: &mut CacheControlCollector,
) -> Result<Vec<CanonicalMessage>, gateway_core::GatewayError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let content = decode_content(value, ContentContext::System, cache_controls)?;
    if content.is_empty() {
        return Err(client_request_error());
    }
    Ok(vec![CanonicalMessage {
        role: MessageRole("system".to_owned()),
        content,
        extensions: RawExtensions::default(),
    }])
}

fn decode_message(
    value: &Value,
    cache_controls: &mut CacheControlCollector,
) -> Result<Vec<CanonicalMessage>, gateway_core::GatewayError> {
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
    let content = decode_content(
        required_value(message, "content")?,
        block_context,
        cache_controls,
    )?;
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
    cache_controls: &mut CacheControlCollector,
) -> Result<Vec<MessageContent>, gateway_core::GatewayError> {
    match value {
        Value::String(text) => Ok(vec![text_content(text.clone(), RawExtensions::default())]),
        Value::Array(parts) => parts
            .iter()
            .map(|part| decode_content_block(part, context, cache_controls))
            .collect(),
        _ => Err(client_request_error()),
    }
}

fn decode_content_block(
    value: &Value,
    context: ContentContext,
    cache_controls: &mut CacheControlCollector,
) -> Result<MessageContent, gateway_core::GatewayError> {
    let block = object(value)?;
    if let Some(cache_control) = block.get("cache_control") {
        cache_controls.observe(cache_control)?;
    }
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

fn decode_tools(
    value: &Value,
    cache_controls: &mut CacheControlCollector,
) -> Result<Vec<ToolDefinition>, gateway_core::GatewayError> {
    array(value)?
        .iter()
        .map(|tool| decode_tool(tool, cache_controls))
        .collect()
}

fn decode_tool(
    value: &Value,
    cache_controls: &mut CacheControlCollector,
) -> Result<ToolDefinition, gateway_core::GatewayError> {
    let tool = object(value)?;
    if let Some(cache_control) = tool.get("cache_control") {
        cache_controls.observe(cache_control)?;
    }
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
        assert_eq!(
            decoded
                .request
                .thinking
                .as_ref()
                .map(|thinking| thinking.effort.as_str()),
            Some("enabled")
        );
        assert_eq!(
            decoded
                .request
                .thinking
                .as_ref()
                .and_then(|thinking| thinking.extensions.get("anthropic.thinking.budget_tokens"))
                .map(gateway_core::RawJson::get),
            Some("1024")
        );
        assert_eq!(
            decoded.request.prompt_cache_retention.as_deref(),
            Some("ephemeral")
        );
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
    fn cache_controls_and_thinking_fail_closed_when_their_canonical_semantics_are_ambiguous() {
        for input in [
            r#"{"model":"m","max_tokens":1,"thinking":{"type":"enabled","budget_tokens":0},"messages":[{"role":"user","content":"x"}]}"#,
            r#"{"model":"m","max_tokens":1,"messages":[{"role":"user","content":[{"type":"text","text":"x","cache_control":{"type":"persistent"}}]}]}"#,
            r#"{"model":"m","max_tokens":1,"system":[{"type":"text","text":"s","cache_control":{"type":"ephemeral","ttl":"5m"}}],"messages":[{"role":"user","content":[{"type":"text","text":"x","cache_control":{"type":"ephemeral","ttl":"1h"}}]}]}"#,
        ] {
            assert!(decode_request(input).is_err(), "accepted {input}");
        }
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
