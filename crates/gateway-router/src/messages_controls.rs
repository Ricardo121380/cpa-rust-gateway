//! Closed local Messages context identity rule, independent of upstream thinking support.
use crate::ProtocolTransformRejection;
use gateway_core::{CanonicalRequest, MessageContent, RawExtensions};
use serde_json::Value;

/// The sole reviewed local context policy; it is never an upstream Build field.
pub const MESSAGES_CONTEXT_MANAGEMENT: &str = "anthropic.messages.context_management";

/// Validates the entire effective history on each request and reports a requested identity rule.
/// No history is changed and no clearing statistic is inferred.
/// # Errors
/// Rejects any other policy or unrepresented native history, including tool-result blocks.
pub fn messages_keep_all(request: &CanonicalRequest) -> Result<bool, ProtocolTransformRejection> {
    let invalid = || ProtocolTransformRejection::UnknownRequestExtensions;
    let Some(raw) = request.extensions.get(MESSAGES_CONTEXT_MANAGEMENT) else {
        return Ok(false);
    };
    let policy: RawExtensions = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
    if policy.iter().len() != 1 {
        return Err(invalid());
    }
    let edits: Vec<RawExtensions> =
        serde_json::from_str(policy.get("edits").ok_or_else(invalid)?.get())
            .map_err(|_| invalid())?;
    if edits.len() != 1 || edits[0].iter().len() != 2 {
        return Err(invalid());
    }
    for (name, expected) in [("type", "clear_thinking_20251015"), ("keep", "all")] {
        let value: String = serde_json::from_str(edits[0].get(name).ok_or_else(invalid)?.get())
            .map_err(|_| invalid())?;
        if value != expected {
            return Err(invalid());
        }
    }
    request
        .validate_tool_history()
        .map_err(|_| ProtocolTransformRejection::ToolHistoryUnsupported)?;
    for part in request.messages.iter().flat_map(|message| &message.content) {
        match part {
            MessageContent::Reasoning(_) | MessageContent::Opaque(_) => {
                return Err(ProtocolTransformRejection::OpaqueContent);
            }
            MessageContent::ToolResult(result) => {
                let content: Value =
                    serde_json::from_str(result.output.get()).map_err(|_| invalid())?;
                if !text_result(&content) {
                    return Err(ProtocolTransformRejection::OpaqueContent);
                }
            }
            MessageContent::Text(_) | MessageContent::ToolCall(_) => {}
        }
    }
    Ok(true)
}

fn text_result(value: &Value) -> bool {
    match value {
        Value::String(_) => true,
        Value::Array(blocks) => blocks.iter().all(|block| {
            block.as_object().is_some_and(|block| {
                block.len() == 2
                    && block.get("type").is_some_and(|value| value == "text")
                    && block.get("text").is_some_and(Value::is_string)
            })
        }),
        _ => false,
    }
}
