//! Closed C5 wire shapes. Parsing does not confer adapter or service qualification.
use std::collections::BTreeSet;

use gateway_core::{CanonicalRequest, GatewayError, MessageContent, RawExtensions, RawJson};
use serde_json::Value;

use crate::{
    ResponsesToolSet, client_request_error, reject_duplicate_json_names, stream_protocol_error,
};

/// Protocol-owned carrier for an observed hosted phase, never visible answer text.
pub const PHASE: &str = "openai.responses.web_search_phase";
/// Marks a validated top-level hosted history item, distinct from nested content blocks.
pub const HISTORY: &str = "openai.responses.hosted_history";

/// Checks the bounded URL subset without fetching or normalizing its value.
#[must_use]
pub fn valid_url(value: &str) -> bool {
    value.len() <= 8192
        && !value.chars().any(|c| c.is_whitespace() || c.is_control())
        && url::Url::parse(value)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

/// Validates a hosted item's exact fields and completion status, preserving presence.
///
/// # Errors
/// Rejects unsupported fields, actions, statuses, identities or bounds.
pub fn validate_item(value: &Value, completed: bool) -> Result<(), GatewayError> {
    let invalid = stream_protocol_error;
    let item = value.as_object().ok_or_else(invalid)?;
    if value.to_string().len() > 64 * 1024
        || item
            .keys()
            .any(|key| !matches!(key.as_str(), "id" | "type" | "status" | "action"))
        || item.get("type").and_then(Value::as_str) != Some("web_search_call")
        || item.get("status").and_then(Value::as_str)
            != Some(if completed {
                "completed"
            } else {
                "in_progress"
            })
        || item.get("id").and_then(Value::as_str).is_none_or(|id| {
            id.is_empty() || id.len() > 512 || !id.bytes().all(|b| b.is_ascii_graphic())
        })
    {
        return Err(invalid());
    }
    if let Some(action) = item.get("action") {
        validate_action(action)?;
    }
    Ok(())
}

fn validate_action(value: &Value) -> Result<(), GatewayError> {
    let action = value.as_object().ok_or_else(stream_protocol_error)?;
    let fields: &[&str] = match action.get("type").and_then(Value::as_str) {
        Some("search") => &["type", "query", "queries", "sources"],
        Some("open_page") => &["type", "url"],
        Some("find_in_page") if action.get("pattern").is_some_and(bounded_text) => {
            &["type", "pattern", "url"]
        }
        _ => return Err(stream_protocol_error()),
    };
    if action.keys().any(|key| !fields.contains(&key.as_str()))
        || action.get("query").is_some_and(|v| !bounded_text(v))
        || action.get("queries").is_some_and(|v| {
            v.as_array()
                .is_none_or(|items| items.len() > 128 || !items.iter().all(bounded_text))
        })
        || action
            .get("url")
            .is_some_and(|v| !v.is_null() && v.as_str().is_none_or(|v| !valid_url(v)))
        || action.get("sources").is_some_and(|v| {
            v.as_array().is_none_or(|sources| {
                sources.len() > 128
                    || sources.iter().any(|source| {
                        source.as_object().is_none_or(|source| {
                            source.len() != 2
                                || source.get("type").and_then(Value::as_str) != Some("url")
                                || source
                                    .get("url")
                                    .and_then(Value::as_str)
                                    .is_none_or(|v| !valid_url(v))
                        })
                    })
            })
        })
    {
        return Err(stream_protocol_error());
    }
    Ok(())
}

fn bounded_text(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|v| v.len() <= 4096 && !v.contains('\0'))
}

/// Returns the validated completed top-level item carried by an existing opaque envelope.
///
/// # Errors
/// Rejects malformed or duplicate-key search history; unrelated opaque blocks return None.
pub fn history_item(part: &MessageContent) -> Result<Option<Value>, GatewayError> {
    let MessageContent::Opaque(opaque) = part else {
        return Ok(None);
    };
    let value: Value =
        serde_json::from_str(opaque.raw().get()).map_err(|_| client_request_error())?;
    if value.get("type").and_then(Value::as_str) != Some("web_search_call") {
        return Ok(None);
    }
    reject_duplicate_json_names(opaque.raw().get())?;
    if opaque.extensions.iter().len() != 1
        || opaque
            .extensions
            .get(HISTORY)
            .is_none_or(|v| v.get() != "true")
    {
        return Err(client_request_error());
    }
    validate_item(&value, true).map_err(|_| client_request_error())?;
    Ok(Some(value))
}

/// Whether a declaration or explicit native history independently needs C5 qualification.
#[must_use]
pub fn requires_qualification(request: &CanonicalRequest) -> bool {
    ResponsesToolSet::from_request(request)
        .ok()
        .flatten()
        .is_some_and(|v| v.has_web_search())
        || request
            .messages
            .iter()
            .flat_map(|m| &m.content)
            .any(|part| history_item(part).ok().flatten().is_some())
}

/// Validates the closed sole-item history envelope and unique native identities.
///
/// # Errors
/// Rejects mixed/wrong-role/unfinished history and conflicting native item IDs.
pub fn validate_history(request: &CanonicalRequest) -> Result<(), GatewayError> {
    let mut ids = BTreeSet::new();
    for message in &request.messages {
        let mut native_ids = Vec::new();
        if let Some(raw) = message.extensions.get("id") {
            native_ids.push(
                serde_json::from_str::<String>(raw.get()).map_err(|_| client_request_error())?,
            );
        }
        for part in &message.content {
            if let Some(item) = history_item(part)? {
                if message.role.0 != "assistant"
                    || message.content.len() != 1
                    || !message.extensions.is_empty()
                {
                    return Err(client_request_error());
                }
                native_ids.push(
                    item["id"]
                        .as_str()
                        .ok_or_else(client_request_error)?
                        .to_owned(),
                );
            } else {
                let raw = match part {
                    MessageContent::Reasoning(v) => Some(v.raw()),
                    MessageContent::ToolCall(v) => v.extensions.get("id"),
                    MessageContent::ToolResult(v) => v.extensions.get("id"),
                    _ => None,
                };
                if let Some(raw) = raw {
                    let value: Value =
                        serde_json::from_str(raw.get()).map_err(|_| client_request_error())?;
                    if matches!(part, MessageContent::ToolResult(_))
                        && !super::input_message_ids::valid_item_id(&value)
                    {
                        return Err(client_request_error());
                    }
                    if let Some(id) = value
                        .as_str()
                        .or_else(|| value.get("id").and_then(Value::as_str))
                    {
                        native_ids.push(id.to_owned());
                    }
                }
            }
        }
        for id in native_ids {
            if !ids.insert(id) {
                return Err(client_request_error());
            }
        }
    }
    Ok(())
}

/// Validates an observed phase's closed shape and returns its ordered stage (1..=3).
///
/// # Errors
/// Rejects unknown fields, phases, or missing/invalid correlation members.
pub fn validate_phase(value: &Value) -> Result<u8, GatewayError> {
    let event = value.as_object().ok_or_else(stream_protocol_error)?;
    if event.len() != 4
        || event.keys().any(|key| {
            !matches!(
                key.as_str(),
                "type" | "item_id" | "output_index" | "sequence_number"
            )
        })
        || event
            .get("item_id")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || event.get("output_index").and_then(Value::as_u64).is_none()
        || event
            .get("sequence_number")
            .and_then(Value::as_u64)
            .is_none()
    {
        return Err(stream_protocol_error());
    }
    match event.get("type").and_then(Value::as_str) {
        Some("response.web_search_call.in_progress") => Ok(1),
        Some("response.web_search_call.searching") => Ok(2),
        Some("response.web_search_call.completed") => Ok(3),
        _ => Err(stream_protocol_error()),
    }
}

/// Whether an observed search stage can follow the previous observed stage.
#[must_use]
pub fn valid_phase_transition(previous: Option<u8>, next: u8) -> bool {
    matches!(
        (previous, next),
        (None, 1) | (Some(1), 2 | 3) | (Some(2), 3)
    )
}

/// Builds a bounded protocol extension from an already observed phase.
///
/// # Errors
/// Rejects an invalid event or extension capacity failure.
pub fn phase_extensions(value: &Value) -> Result<RawExtensions, GatewayError> {
    validate_phase(value)?;
    let mut extensions = RawExtensions::default();
    extensions
        .try_insert(
            PHASE,
            RawJson::from_json_string(value.to_string()).map_err(|_| stream_protocol_error())?,
        )
        .map_err(|_| stream_protocol_error())?;
    Ok(extensions)
}
