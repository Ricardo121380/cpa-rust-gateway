//! Canonical request to `Anthropic` Messages wire request encoding.
//!
//! This module is the exact inverse of [`crate::decode_request`]. Every canonical shape that
//! decoder produces is re-encoded here, and every canonical shape an `Anthropic` Messages request
//! cannot express fails closed with a safe provider protocol error instead of being silently
//! degraded: a request this encoder accepts always decodes back to the request it was given.
//!
//! `tool_choice`, `stop_sequences`, `temperature`, `top_p`, and `metadata` need no dedicated
//! mapping. They are absent from the inbound decoder's root field list, so they already round-trip
//! through the `anthropic.messages.` root extension namespace inverted below.

use gateway_core::{
    CanonicalMessage, CanonicalRequest, GatewayError, MessageContent, OpaqueContent, RawExtensions,
    RawJson, TextContent, Thinking, ToolCall, ToolDefinition, ToolResult,
};
use serde_json::{Map, Value};

use crate::{
    ResponseMode,
    json::{internal_error, provider_protocol_error},
    request::CacheControlCollector,
};

/// Namespace carrying every `Anthropic` root field the Canonical core has no dedicated field for.
const ROOT_EXTENSION_PREFIX: &str = "anthropic.messages.";
/// Namespace carrying `Anthropic` message, content-block, and Tool fields.
const BLOCK_EXTENSION_PREFIX: &str = "anthropic.";
/// Namespace carrying `Anthropic` `thinking` fields.
const THINKING_EXTENSION_PREFIX: &str = "anthropic.thinking.";
/// Root fields this encoder owns; a retained root extension may never collide with one.
const ROOT_RESERVED_FIELDS: &[&str] =
    &["model", "messages", "system", "tools", "stream", "thinking"];
const MESSAGE_RESERVED_FIELDS: &[&str] = &["role", "content"];
const TEXT_RESERVED_FIELDS: &[&str] = &["type", "text"];
const TOOL_USE_RESERVED_FIELDS: &[&str] = &["type", "id", "name", "input"];
const TOOL_RESULT_RESERVED_FIELDS: &[&str] = &["type", "tool_use_id", "content", "is_error"];
const TOOL_RESERVED_FIELDS: &[&str] = &["name", "description", "input_schema"];
const THINKING_RESERVED_FIELDS: &[&str] = &["type"];
/// Content-block type labels this codec owns; a retained opaque block may not reuse one.
const KNOWN_BLOCK_TYPES: &[&str] = &["text", "tool_use", "tool_result"];

/// Encodes one canonical request as a complete `Anthropic` Messages JSON request body.
///
/// The caller supplies the already selected upstream model; the client-visible requested model is
/// never forwarded. `Anthropic` requires an output limit, which the inbound decoder retains as the
/// `anthropic.messages.max_tokens` root extension, so a canonical request without that exact
/// positive-integer extension is rejected rather than served with an invented limit.
///
/// # Errors
///
/// Returns `UpstreamProtocolError/Provider` for an empty upstream model, a canonical semantic
/// `Anthropic` Messages cannot express, a foreign or colliding extension namespace, or a
/// `prompt_cache_retention` summary that the emitted cache controls do not re-derive. Returns
/// `InternalError/Internal` only if JSON serialization of an already validated value fails.
pub fn encode_upstream_request(
    upstream_model: &str,
    request: &CanonicalRequest,
    mode: ResponseMode,
) -> Result<String, GatewayError> {
    if upstream_model.is_empty() || request.prompt_cache_key.is_some() {
        return Err(provider_protocol_error());
    }

    let mut cache_controls = CacheControlCollector::default();
    let mut root = Map::new();
    root.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    root.insert(
        "stream".to_owned(),
        Value::Bool(mode == ResponseMode::Streaming),
    );

    let (system, messages) = encode_messages(&request.messages, &mut cache_controls)?;
    if messages.is_empty() {
        return Err(provider_protocol_error());
    }
    if let Some(system) = system {
        root.insert("system".to_owned(), Value::Array(system));
    }
    root.insert("messages".to_owned(), Value::Array(messages));

    if !request.tools.is_empty() {
        root.insert(
            "tools".to_owned(),
            Value::Array(encode_tools(&request.tools, &mut cache_controls)?),
        );
    }
    if let Some(thinking) = &request.thinking {
        root.insert("thinking".to_owned(), encode_thinking(thinking)?);
    }

    insert_prefixed_extensions(
        &mut root,
        &request.extensions,
        ROOT_EXTENSION_PREFIX,
        ROOT_RESERVED_FIELDS,
    )?;
    require_max_tokens(&root)?;
    if cache_controls.into_retention() != request.prompt_cache_retention {
        return Err(provider_protocol_error());
    }

    serde_json::to_string(&Value::Object(root)).map_err(|_| internal_error())
}

/// Requires the one `Anthropic` output limit using the inbound decoder's exact predicate.
fn require_max_tokens(root: &Map<String, Value>) -> Result<(), GatewayError> {
    match root.get("max_tokens").and_then(Value::as_u64) {
        Some(value) if value > 0 => Ok(()),
        _ => Err(provider_protocol_error()),
    }
}

/// One `Anthropic` user message still accepting the canonical messages it was split into.
struct UserTurn<'a> {
    content: Vec<Value>,
    extensions: &'a RawExtensions,
    ended_with_user: bool,
}

/// Splits canonical messages into `Anthropic`'s top-level `system` and its `messages` array.
///
/// The inbound decoder projects one top-level `system` into a leading canonical `system` message
/// and splits every `tool_result` block of one `Anthropic` user message into its own canonical
/// `tool` message, attaching the original message extensions to the first message it emits. This
/// is that projection's inverse: it lifts `system` back to the top level and re-merges a run of
/// canonical `tool`/`user` messages into the one `Anthropic` user message they came from, starting
/// a new message exactly where the decoder proves a new one must have existed -- at a message that
/// carries its own extensions, and at a `user` message whose predecessor in the run was also
/// `user`.
fn encode_messages<'a>(
    messages: &'a [CanonicalMessage],
    cache_controls: &mut CacheControlCollector,
) -> Result<(Option<Vec<Value>>, Vec<Value>), GatewayError> {
    let mut system = None;
    let mut encoded = Vec::new();
    let mut turn: Option<UserTurn<'a>> = None;

    for (position, message) in messages.iter().enumerate() {
        match message.role.0.as_str() {
            "system" => {
                if position != 0 || !message.extensions.is_empty() {
                    return Err(provider_protocol_error());
                }
                system = Some(encode_content(
                    &message.content,
                    ContentContext::System,
                    cache_controls,
                )?);
            }
            "assistant" => {
                flush_user_turn(&mut encoded, turn.take())?;
                let content =
                    encode_content(&message.content, ContentContext::Assistant, cache_controls)?;
                encoded.push(message_value("assistant", content, &message.extensions)?);
            }
            role @ ("user" | "tool") => {
                let content = encode_user_turn_content(role, message, cache_controls)?;
                let ends_with_user = role == "user";
                let continues = turn.as_ref().is_some_and(|open| {
                    message.extensions.is_empty() && !(ends_with_user && open.ended_with_user)
                });
                if continues {
                    let Some(open) = turn.as_mut() else {
                        return Err(internal_error());
                    };
                    open.content.extend(content);
                    open.ended_with_user = ends_with_user;
                } else {
                    flush_user_turn(&mut encoded, turn.take())?;
                    turn = Some(UserTurn {
                        content,
                        extensions: &message.extensions,
                        ended_with_user: ends_with_user,
                    });
                }
            }
            _ => return Err(provider_protocol_error()),
        }
    }
    flush_user_turn(&mut encoded, turn.take())?;

    Ok((system, encoded))
}

/// Encodes one canonical `user` or split-out `tool` message into `Anthropic` content blocks.
///
/// A canonical `tool` message is exactly what the inbound decoder emits for one `tool_result`
/// block, so anything richer cannot be merged back into one `Anthropic` user message without
/// changing what a re-decode would produce.
fn encode_user_turn_content(
    role: &str,
    message: &CanonicalMessage,
    cache_controls: &mut CacheControlCollector,
) -> Result<Vec<Value>, GatewayError> {
    if role == "tool" && !matches!(message.content.as_slice(), [MessageContent::ToolResult(_)]) {
        return Err(provider_protocol_error());
    }
    encode_content(&message.content, ContentContext::User, cache_controls)
}

fn flush_user_turn(
    encoded: &mut Vec<Value>,
    turn: Option<UserTurn<'_>>,
) -> Result<(), GatewayError> {
    let Some(turn) = turn else {
        return Ok(());
    };
    encoded.push(message_value("user", turn.content, turn.extensions)?);
    Ok(())
}

fn message_value(
    role: &str,
    content: Vec<Value>,
    extensions: &RawExtensions,
) -> Result<Value, GatewayError> {
    if content.is_empty() {
        return Err(provider_protocol_error());
    }
    let mut message = Map::new();
    message.insert("role".to_owned(), Value::String(role.to_owned()));
    message.insert("content".to_owned(), Value::Array(content));
    insert_prefixed_extensions(
        &mut message,
        extensions,
        BLOCK_EXTENSION_PREFIX,
        MESSAGE_RESERVED_FIELDS,
    )?;
    Ok(Value::Object(message))
}

#[derive(Clone, Copy)]
enum ContentContext {
    System,
    User,
    Assistant,
}

fn encode_content(
    parts: &[MessageContent],
    context: ContentContext,
    cache_controls: &mut CacheControlCollector,
) -> Result<Vec<Value>, GatewayError> {
    if parts.is_empty() {
        return Err(provider_protocol_error());
    }
    parts
        .iter()
        .map(|part| encode_content_block(part, context, cache_controls))
        .collect()
}

fn encode_content_block(
    part: &MessageContent,
    context: ContentContext,
    cache_controls: &mut CacheControlCollector,
) -> Result<Value, GatewayError> {
    let block = match part {
        MessageContent::Text(text) => encode_text_block(text)?,
        MessageContent::ToolCall(call) => {
            if !matches!(context, ContentContext::Assistant) {
                return Err(provider_protocol_error());
            }
            encode_tool_use_block(call)?
        }
        MessageContent::ToolResult(result) => {
            if !matches!(context, ContentContext::User) {
                return Err(provider_protocol_error());
            }
            encode_tool_result_block(result)?
        }
        MessageContent::Opaque(opaque) => encode_opaque_block(opaque)?,
    };
    observe_cache_control(&block, cache_controls)?;
    Ok(Value::Object(block))
}

/// Replays one emitted block's `cache_control` through the inbound collector.
///
/// Running the decoder's own collector is what makes the request-level
/// `prompt_cache_retention` summary provable rather than assumed.
fn observe_cache_control(
    block: &Map<String, Value>,
    cache_controls: &mut CacheControlCollector,
) -> Result<(), GatewayError> {
    let Some(cache_control) = block.get("cache_control") else {
        return Ok(());
    };
    cache_controls
        .observe(cache_control)
        .map_err(|_| provider_protocol_error())
}

fn encode_text_block(text: &TextContent) -> Result<Map<String, Value>, GatewayError> {
    let mut block = Map::new();
    block.insert("type".to_owned(), Value::String("text".to_owned()));
    block.insert("text".to_owned(), Value::String(text.text.clone()));
    insert_prefixed_extensions(
        &mut block,
        &text.extensions,
        BLOCK_EXTENSION_PREFIX,
        TEXT_RESERVED_FIELDS,
    )?;
    Ok(block)
}

fn encode_tool_use_block(call: &ToolCall) -> Result<Map<String, Value>, GatewayError> {
    if call.id.is_empty() || call.name.is_empty() {
        return Err(provider_protocol_error());
    }
    let mut block = Map::new();
    block.insert("type".to_owned(), Value::String("tool_use".to_owned()));
    block.insert("id".to_owned(), Value::String(call.id.clone()));
    block.insert("name".to_owned(), Value::String(call.name.clone()));
    block.insert("input".to_owned(), raw_value(&call.arguments)?);
    insert_prefixed_extensions(
        &mut block,
        &call.extensions,
        BLOCK_EXTENSION_PREFIX,
        TOOL_USE_RESERVED_FIELDS,
    )?;
    Ok(block)
}

fn encode_tool_result_block(result: &ToolResult) -> Result<Map<String, Value>, GatewayError> {
    if result.call_id.is_empty() {
        return Err(provider_protocol_error());
    }
    let mut block = Map::new();
    block.insert("type".to_owned(), Value::String("tool_result".to_owned()));
    block.insert(
        "tool_use_id".to_owned(),
        Value::String(result.call_id.clone()),
    );
    block.insert("content".to_owned(), raw_value(&result.output)?);
    block.insert("is_error".to_owned(), Value::Bool(result.is_error));
    insert_prefixed_extensions(
        &mut block,
        &result.extensions,
        BLOCK_EXTENSION_PREFIX,
        TOOL_RESULT_RESERVED_FIELDS,
    )?;
    Ok(block)
}

/// Restores one retained future content block without reinterpreting it.
///
/// The inbound decoder retains an unknown block whole, so it never populates the canonical opaque
/// extension namespace and never retains a block whose `type` this codec owns. A block violating
/// either fact did not come from an `Anthropic` request and cannot be replayed into one.
fn encode_opaque_block(opaque: &OpaqueContent) -> Result<Map<String, Value>, GatewayError> {
    if !opaque.extensions.is_empty() {
        return Err(provider_protocol_error());
    }
    let Value::Object(block) = raw_value(opaque.raw())? else {
        return Err(provider_protocol_error());
    };
    match block.get("type").and_then(Value::as_str) {
        Some(kind) if !KNOWN_BLOCK_TYPES.contains(&kind) => Ok(block),
        _ => Err(provider_protocol_error()),
    }
}

fn encode_tools(
    tools: &[ToolDefinition],
    cache_controls: &mut CacheControlCollector,
) -> Result<Vec<Value>, GatewayError> {
    tools
        .iter()
        .map(|tool| encode_tool(tool, cache_controls))
        .collect()
}

fn encode_tool(
    tool: &ToolDefinition,
    cache_controls: &mut CacheControlCollector,
) -> Result<Value, GatewayError> {
    if tool.name.is_empty() {
        return Err(provider_protocol_error());
    }
    let input_schema = raw_value(&tool.input_schema)?;
    if !input_schema.is_object() {
        return Err(provider_protocol_error());
    }

    let mut encoded = Map::new();
    encoded.insert("name".to_owned(), Value::String(tool.name.clone()));
    if let Some(description) = &tool.description {
        encoded.insert("description".to_owned(), Value::String(description.clone()));
    }
    encoded.insert("input_schema".to_owned(), input_schema);
    insert_prefixed_extensions(
        &mut encoded,
        &tool.extensions,
        BLOCK_EXTENSION_PREFIX,
        TOOL_RESERVED_FIELDS,
    )?;
    observe_cache_control(&encoded, cache_controls)?;
    Ok(Value::Object(encoded))
}

/// Restores the `Anthropic` `thinking` object from the canonical effort label and its namespace.
///
/// The inbound decoder rejects a non-positive `budget_tokens`, so re-emitting one would produce a
/// request that decoder would refuse. The same predicate is applied here.
fn encode_thinking(thinking: &Thinking) -> Result<Value, GatewayError> {
    let mut encoded = Map::new();
    encoded.insert(
        "type".to_owned(),
        Value::String(thinking.effort.as_str().to_owned()),
    );
    insert_prefixed_extensions(
        &mut encoded,
        &thinking.extensions,
        THINKING_EXTENSION_PREFIX,
        THINKING_RESERVED_FIELDS,
    )?;
    match encoded.get("budget_tokens") {
        None => {}
        Some(value) if value.as_u64().is_some_and(|tokens| tokens > 0) => {}
        Some(_) => return Err(provider_protocol_error()),
    }
    Ok(Value::Object(encoded))
}

/// Restores one explicit `anthropic.*` namespace into the wire object that produced it.
///
/// A name outside `prefix` belongs to another protocol and is rejected: silently forwarding it
/// would let a foreign field reach an `Anthropic` upstream under a name it never had. A name that
/// collides with a field this codec owns is rejected for the same reason.
fn insert_prefixed_extensions(
    object: &mut Map<String, Value>,
    extensions: &RawExtensions,
    prefix: &str,
    reserved: &[&str],
) -> Result<(), GatewayError> {
    for (name, raw) in extensions.iter() {
        let Some(name) = name.strip_prefix(prefix) else {
            return Err(provider_protocol_error());
        };
        if name.is_empty() || reserved.contains(&name) || object.contains_key(name) {
            return Err(provider_protocol_error());
        }
        object.insert(name.to_owned(), raw_value(raw)?);
    }
    Ok(())
}

/// Reparses retained JSON so the emitted body carries the same canonical text a re-decode builds.
fn raw_value(raw: &RawJson) -> Result<Value, GatewayError> {
    serde_json::from_str(raw.get()).map_err(|_| provider_protocol_error())
}

#[cfg(test)]
mod tests {
    use gateway_core::{CanonicalRequest, GatewayErrorCode, RawJson};
    use serde_json::Value;

    use super::encode_upstream_request;
    use crate::{ResponseMode, decode_request};

    const UPSTREAM_MODEL: &str = "claude-upstream";

    fn fixture_request() -> Result<CanonicalRequest, gateway_core::GatewayError> {
        Ok(decode_request(include_str!(
            "../../../tests/fixtures/anthropic/messages-request.json"
        ))?
        .request)
    }

    fn canonical(request: &str) -> Result<CanonicalRequest, serde_json::Error> {
        serde_json::from_str(request)
    }

    fn rejected(request: &CanonicalRequest, mode: ResponseMode) -> bool {
        matches!(
            encode_upstream_request(UPSTREAM_MODEL, request, mode),
            Err(error) if error.code() == GatewayErrorCode::UpstreamProtocolError
        )
    }

    #[test]
    fn canonical_request_round_trips_through_the_anthropic_messages_wire()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = fixture_request()?;
        let encoded = encode_upstream_request(UPSTREAM_MODEL, &request, ResponseMode::Streaming)?;
        let rebuilt = decode_request(&encoded)?;
        let mut expected = request.clone();
        expected.requested_model = UPSTREAM_MODEL.to_owned();

        assert!(!encoded.contains("gateway-claude"));
        assert_eq!(rebuilt.mode, ResponseMode::Streaming);
        assert_eq!(rebuilt.request, expected);
        assert_eq!(
            serde_json::from_str::<Value>(&encoded)?,
            serde_json::from_str::<Value>(include_str!(
                "../../../tests/fixtures/anthropic/upstream-outbound-request.json"
            ))?
        );
        Ok(())
    }

    #[test]
    fn places_system_tools_thinking_and_the_required_output_limit_at_their_anthropic_positions()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = fixture_request()?;
        let encoded: Value = serde_json::from_str(&encode_upstream_request(
            UPSTREAM_MODEL,
            &request,
            ResponseMode::NonStreaming,
        )?)?;

        assert_eq!(encoded["stream"], Value::Bool(false));
        assert_eq!(encoded["max_tokens"], serde_json::json!(128));
        assert_eq!(encoded["temperature"], serde_json::json!(0));
        assert_eq!(
            encoded["metadata"]["user_id"],
            serde_json::json!("test-user")
        );
        assert_eq!(
            encoded["system"][0]["text"],
            serde_json::json!("Follow the tool policy.")
        );
        assert_eq!(
            encoded["system"][0]["cache_control"]["type"],
            serde_json::json!("ephemeral")
        );
        assert_eq!(
            encoded["thinking"],
            serde_json::json!({"type":"enabled","budget_tokens":1024})
        );
        assert_eq!(encoded["tools"][0]["name"], serde_json::json!("lookup"));
        assert_eq!(encoded["messages"].as_array().map(Vec::len), Some(3));
        assert!(encoded["messages"].as_array().is_some_and(|messages| {
            messages
                .iter()
                .all(|message| message["role"] != serde_json::json!("system"))
        }));
        Ok(())
    }

    #[test]
    fn merges_split_tool_result_messages_back_into_one_anthropic_user_turn()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = canonical(
            r#"{
                "requested_model":"gateway-claude",
                "messages":[
                    {"role":"user","content":[{"text":{"text":"before","extensions":{}}}],
                     "extensions":{"anthropic.vendor":{"keep":true}}},
                    {"role":"tool","content":[{"tool_result":{"call_id":"call-01","output":{"ok":true},"is_error":false,"extensions":{}}}],"extensions":{}},
                    {"role":"user","content":[{"text":{"text":"after","extensions":{}}}],"extensions":{}}
                ],
                "extensions":{"anthropic.messages.max_tokens":32}
            }"#,
        )?;
        let encoded =
            encode_upstream_request(UPSTREAM_MODEL, &request, ResponseMode::NonStreaming)?;
        let wire: Value = serde_json::from_str(&encoded)?;

        assert_eq!(wire["messages"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            wire["messages"][0]["content"].as_array().map(Vec::len),
            Some(3)
        );
        assert_eq!(
            wire["messages"][0]["vendor"],
            serde_json::json!({"keep":true})
        );
        assert_eq!(
            wire["messages"][0]["content"][1]["type"],
            serde_json::json!("tool_result")
        );

        let mut expected = request.clone();
        expected.requested_model = UPSTREAM_MODEL.to_owned();
        assert_eq!(decode_request(&encoded)?.request, expected);
        Ok(())
    }

    #[test]
    fn starts_a_new_anthropic_message_where_the_decoder_proves_one_existed()
    -> Result<(), Box<dyn std::error::Error>> {
        for (source, expected_messages) in [
            (
                r#"{"requested_model":"m","messages":[
                    {"role":"user","content":[{"text":{"text":"one","extensions":{}}}],"extensions":{}},
                    {"role":"user","content":[{"text":{"text":"two","extensions":{}}}],"extensions":{}}
                ],"extensions":{"anthropic.messages.max_tokens":8}}"#,
                2,
            ),
            (
                r#"{"requested_model":"m","messages":[
                    {"role":"user","content":[{"text":{"text":"one","extensions":{}}}],"extensions":{"anthropic.a":1}},
                    {"role":"tool","content":[{"tool_result":{"call_id":"c","output":{},"is_error":false,"extensions":{}}}],"extensions":{"anthropic.b":2}}
                ],"extensions":{"anthropic.messages.max_tokens":8}}"#,
                2,
            ),
        ] {
            let request = canonical(source)?;
            let encoded =
                encode_upstream_request(UPSTREAM_MODEL, &request, ResponseMode::NonStreaming)?;
            let wire: Value = serde_json::from_str(&encoded)?;
            assert_eq!(
                wire["messages"].as_array().map(Vec::len),
                Some(expected_messages)
            );
            let mut expected = request.clone();
            expected.requested_model = UPSTREAM_MODEL.to_owned();
            assert_eq!(decode_request(&encoded)?.request, expected);
        }
        Ok(())
    }

    #[test]
    fn fails_closed_on_shapes_anthropic_messages_cannot_express()
    -> Result<(), Box<dyn std::error::Error>> {
        let text = r#"[{"text":{"text":"x","extensions":{}}}]"#;
        for source in [
            format!(r#"{{"requested_model":"m","messages":[{{"role":"user","content":{text},"extensions":{{}}}}],"extensions":{{}}}}"#),
            format!(r#"{{"requested_model":"m","messages":[{{"role":"user","content":{text},"extensions":{{}}}}],"extensions":{{"anthropic.messages.max_tokens":0}}}}"#),
            format!(r#"{{"requested_model":"m","messages":[{{"role":"user","content":{text},"extensions":{{}}}}],"prompt_cache_key":"k","extensions":{{"anthropic.messages.max_tokens":8}}}}"#),
            format!(r#"{{"requested_model":"m","messages":[{{"role":"user","content":{text},"extensions":{{}}}}],"extensions":{{"anthropic.messages.max_tokens":8,"openai.responses.max_output_tokens":8}}}}"#),
            format!(r#"{{"requested_model":"m","messages":[{{"role":"user","content":{text},"extensions":{{}}}}],"extensions":{{"anthropic.messages.max_tokens":8,"anthropic.messages.model":"override"}}}}"#),
            format!(r#"{{"requested_model":"m","messages":[{{"role":"user","content":{text},"extensions":{{}}}},{{"role":"system","content":{text},"extensions":{{}}}}],"extensions":{{"anthropic.messages.max_tokens":8}}}}"#),
            format!(r#"{{"requested_model":"m","messages":[{{"role":"system","content":{text},"extensions":{{"anthropic.a":1}}}},{{"role":"user","content":{text},"extensions":{{}}}}],"extensions":{{"anthropic.messages.max_tokens":8}}}}"#),
            format!(r#"{{"requested_model":"m","messages":[{{"role":"developer","content":{text},"extensions":{{}}}}],"extensions":{{"anthropic.messages.max_tokens":8}}}}"#),
            r#"{"requested_model":"m","messages":[{"role":"tool","content":[{"tool_result":{"call_id":"a","output":{},"is_error":false,"extensions":{}}},{"tool_result":{"call_id":"b","output":{},"is_error":false,"extensions":{}}}],"extensions":{}}],"extensions":{"anthropic.messages.max_tokens":8}}"#.to_owned(),
            r#"{"requested_model":"m","messages":[{"role":"user","content":[{"tool_call":{"id":"c","name":"n","arguments":{},"extensions":{}}}],"extensions":{}}],"extensions":{"anthropic.messages.max_tokens":8}}"#.to_owned(),
            r#"{"requested_model":"m","messages":[{"role":"assistant","content":[{"tool_result":{"call_id":"c","output":{},"is_error":false,"extensions":{}}}],"extensions":{}}],"extensions":{"anthropic.messages.max_tokens":8}}"#.to_owned(),
            r#"{"requested_model":"m","messages":[{"role":"user","content":[{"opaque":{"raw":{"type":"text","text":"smuggled"},"extensions":{}}}],"extensions":{}}],"extensions":{"anthropic.messages.max_tokens":8}}"#.to_owned(),
            r#"{"requested_model":"m","messages":[{"role":"user","content":[{"opaque":{"raw":{"type":"image"},"extensions":{"anthropic.x":1}}}],"extensions":{}}],"extensions":{"anthropic.messages.max_tokens":8}}"#.to_owned(),
            format!(r#"{{"requested_model":"m","messages":[{{"role":"user","content":{text},"extensions":{{}}}}],"tools":[{{"name":"t","input_schema":[],"extensions":{{}}}}],"extensions":{{"anthropic.messages.max_tokens":8}}}}"#),
            format!(r#"{{"requested_model":"m","messages":[{{"role":"user","content":{text},"extensions":{{}}}}],"thinking":{{"effort":"enabled","extensions":{{"anthropic.thinking.budget_tokens":0}}}},"extensions":{{"anthropic.messages.max_tokens":8}}}}"#),
            r#"{"requested_model":"m","messages":[],"extensions":{"anthropic.messages.max_tokens":8}}"#.to_owned(),
        ] {
            let request = canonical(&source)?;
            assert!(rejected(&request, ResponseMode::NonStreaming), "accepted {source}");
        }

        let valid = fixture_request()?;
        assert!(encode_upstream_request("", &valid, ResponseMode::Streaming).is_err());
        Ok(())
    }

    #[test]
    fn prompt_cache_retention_must_be_re_derivable_from_the_emitted_cache_controls()
    -> Result<(), Box<dyn std::error::Error>> {
        // The retained raw text is written in the canonical member order a real `decode_request`
        // produces, so the equality below compares semantics rather than incidental key order.
        let with_control = r#"[{"text":{"text":"x","extensions":{"anthropic.cache_control":{"ttl":"5m","type":"ephemeral"}}}}]"#;
        let plain = r#"[{"text":{"text":"x","extensions":{}}}]"#;
        let conflicting = r#"[{"text":{"text":"x","extensions":{"anthropic.cache_control":{"type":"ephemeral","ttl":"5m"}}}},{"text":{"text":"y","extensions":{"anthropic.cache_control":{"type":"ephemeral","ttl":"1h"}}}}]"#;

        for (content, retention) in [
            (plain, r#""prompt_cache_retention":"5m","#),
            (with_control, ""),
            (conflicting, r#""prompt_cache_retention":"5m","#),
        ] {
            let source = format!(
                r#"{{"requested_model":"m","messages":[{{"role":"user","content":{content},"extensions":{{}}}}],{retention}"extensions":{{"anthropic.messages.max_tokens":8}}}}"#
            );
            assert!(
                rejected(&canonical(&source)?, ResponseMode::NonStreaming),
                "accepted {source}"
            );
        }

        let source = format!(
            r#"{{"requested_model":"m","messages":[{{"role":"user","content":{with_control},"extensions":{{}}}}],"prompt_cache_retention":"5m","extensions":{{"anthropic.messages.max_tokens":8}}}}"#
        );
        let request = canonical(&source)?;
        let encoded =
            encode_upstream_request(UPSTREAM_MODEL, &request, ResponseMode::NonStreaming)?;
        let mut expected = request.clone();
        expected.requested_model = UPSTREAM_MODEL.to_owned();
        assert_eq!(decode_request(&encoded)?.request, expected);
        Ok(())
    }

    #[test]
    fn retained_raw_json_survives_the_round_trip_regardless_of_its_original_spacing()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut request = canonical(
            r#"{"requested_model":"m","messages":[{"role":"assistant","content":[{"tool_call":{"id":"c","name":"n","arguments":{ "query" : "weather" },"extensions":{}}}],"extensions":{}}],"tools":[{"name":"n","input_schema":{ "type" : "object" },"extensions":{}}],"extensions":{"anthropic.messages.max_tokens":8}}"#,
        )?;
        request.requested_model = UPSTREAM_MODEL.to_owned();
        let encoded =
            encode_upstream_request(UPSTREAM_MODEL, &request, ResponseMode::NonStreaming)?;
        let rebuilt = decode_request(&encoded)?.request;

        assert_eq!(
            rebuilt.tools[0].input_schema,
            RawJson::from_json_string(r#"{"type":"object"}"#.to_owned())?
        );
        assert_eq!(
            serde_json::to_value(&rebuilt)?,
            serde_json::to_value(&request)?
        );
        Ok(())
    }
}
