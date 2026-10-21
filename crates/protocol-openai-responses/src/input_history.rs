//! Bounded Codex history representation. Parsing never grants Build service qualification.
use gateway_core::{CanonicalRequest, MessageContent};

/// Whether history needs the independent local-controlled Build permission.
/// Invalid result IDs also require permission and are rejected by normal validation.
#[must_use]
pub fn requires_qualification(request: &CanonicalRequest) -> bool {
    request.messages.iter().any(|message| {
        message.content.iter().any(|part| match part {
            MessageContent::Reasoning(history) => {
                serde_json::from_str::<serde_json::Value>(history.raw().get())
                    .is_ok_and(|item| item.get("id").is_none())
            }
            MessageContent::ToolResult(result) => result.extensions.get("id").is_some(),
            _ => false,
        })
    })
}
