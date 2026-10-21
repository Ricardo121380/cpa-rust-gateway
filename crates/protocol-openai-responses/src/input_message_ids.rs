//! Closed C6 native non-assistant message identity representation; activation is separate.
use gateway_core::{CanonicalMessage, CanonicalRequest, GatewayError};
use serde_json::Value;

/// Whether a request carries the independently qualified C6 field family, including invalid IDs.
#[must_use]
pub fn requires_qualification(request: &CanonicalRequest) -> bool {
    request
        .messages
        .iter()
        .any(|message| message.role.0 != "assistant" && message.extensions.get("id").is_some())
}

/// Existing Responses native item ID boundary, shared with assistant/function history.
#[must_use]
pub fn valid_item_id(value: &Value) -> bool {
    value.as_str().is_some_and(|id| {
        !id.is_empty() && id.len() <= 512 && id.bytes().all(|byte| byte.is_ascii_graphic())
    })
}

/// Validates the sole newly supported extension without granting target/service qualification.
///
/// # Errors
/// Rejects unsupported roles, extra extensions, null/type/bounds or empty message content.
pub fn validate_message(message: &CanonicalMessage) -> Result<(), GatewayError> {
    if message.role.0 == "assistant" || message.extensions.get("id").is_none() {
        return Ok(());
    }
    if !matches!(message.role.0.as_str(), "user" | "developer" | "system")
        || message.content.is_empty()
        || !message.extensions.iter().all(|(key, raw)| {
            key == "id" && serde_json::from_str::<Value>(raw.get()).is_ok_and(|v| valid_item_id(&v))
        })
    {
        return Err(super::client_request_error());
    }
    Ok(())
}
