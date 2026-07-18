//! Ordered, protocol-neutral messages and request content.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{RawExtensions, RawJson};

/// An open-ended message role retained independently from any input protocol enum.
///
/// Roles such as `system`, `developer`, `user`, `assistant`, and `tool` are carried as their
/// supplied labels so future protocol roles are not silently collapsed.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageRole(pub String);

impl fmt::Debug for MessageRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MessageRole(<redacted>)")
    }
}

/// One ordered message in a canonical request.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalMessage {
    /// Role that supplied the message.
    pub role: MessageRole,
    /// Ordered semantic content parts for this message.
    pub content: Vec<MessageContent>,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for CanonicalMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalMessage")
            .field("role", &"<redacted>")
            .field("content_part_count", &self.content.len())
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// A protocol-neutral content part retained in a canonical message.
///
/// The externally tagged JSON representation prevents a tag parser from buffering the payload,
/// allowing schemas, Tool arguments, extensions, and opaque content to retain `RawValue` data.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageContent {
    /// Plain text supplied in a message.
    Text(TextContent),
    /// A completed historical Tool call supplied in message history.
    ToolCall(ToolCall),
    /// A historical result correlated to one Tool call.
    ToolResult(ToolResult),
    /// A future or unsupported content block retained as full raw JSON.
    Opaque(OpaqueContent),
}

impl fmt::Debug for MessageContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(_) => formatter.write_str("MessageContent::Text(<redacted>)"),
            Self::ToolCall(_) => formatter.write_str("MessageContent::ToolCall(<redacted>)"),
            Self::ToolResult(_) => formatter.write_str("MessageContent::ToolResult(<redacted>)"),
            Self::Opaque(_) => formatter.write_str("MessageContent::Opaque(<redacted>)"),
        }
    }
}

/// Text content and its explicit extensions.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextContent {
    /// Text value retained exactly as a Rust string.
    pub text: String,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for TextContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextContent")
            .field("text", &"<redacted>")
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// A completed historical Tool call in an assistant message.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    /// Client-visible Tool call correlation identifier.
    pub id: String,
    /// Tool name associated with the call.
    pub name: String,
    /// Completed Tool arguments retained as raw JSON.
    pub arguments: RawJson,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for ToolCall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolCall")
            .field("id", &"<redacted>")
            .field("name", &"<redacted>")
            .field("arguments", &self.arguments)
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// A historical result supplied for a preceding Tool call.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolResult {
    /// Correlation identifier of the completed Tool call.
    pub call_id: String,
    /// Result payload retained as raw JSON.
    pub output: RawJson,
    /// Whether the Tool result represents an application-level error.
    pub is_error: bool,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for ToolResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolResult")
            .field("call_id", &"<redacted>")
            .field("output", &self.output)
            .field("is_error", &self.is_error)
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// One unsupported content block retained without assigning it a canonical semantic meaning.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpaqueContent {
    raw: RawJson,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for OpaqueContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpaqueContent")
            .field("raw", &self.raw)
            .field("extensions", &self.extensions)
            .finish()
    }
}

impl OpaqueContent {
    /// Retains one complete unsupported content block.
    #[must_use]
    pub fn new(raw: RawJson) -> Self {
        Self {
            raw,
            extensions: RawExtensions::default(),
        }
    }

    /// Returns the complete retained content block.
    #[must_use]
    pub fn raw(&self) -> &RawJson {
        &self.raw
    }
}

#[cfg(test)]
mod tests {
    use crate::{CanonicalRequest, MessageContent, OpaqueContent, RawJson};

    #[test]
    fn message_debug_forms_redact_client_values() -> Result<(), serde_json::Error> {
        let request: CanonicalRequest = serde_json::from_str(include_str!(
            "../../../tests/fixtures/core/canonical-request-roundtrip.json"
        ))?;
        let diagnostic = format!(
            "{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
            request.messages[0],
            request.messages[0].role,
            request.messages[0].content[0],
            request.messages[0].content[1],
            request.messages[1].content[0],
            request.messages[2].content[0],
            request.tools[0],
        );

        for sensitive_value in [
            "developer",
            "Use the available tools safely.",
            "input_image",
            "https://example.invalid/fake.png",
            "call-01",
            "weather",
            "clear",
            "lookup",
            "Look up a forecast.",
        ] {
            assert!(!diagnostic.contains(sensitive_value));
        }

        Ok(())
    }

    #[test]
    fn opaque_content_uses_an_explicit_canonical_envelope() -> Result<(), serde_json::Error> {
        let content = MessageContent::Opaque(OpaqueContent::new(RawJson::from_json_string(
            r#"{"kind":"future_content","payload":[1,2]}"#.to_owned(),
        )?));

        assert_eq!(
            serde_json::to_string(&content)?,
            r#"{"opaque":{"raw":{"kind":"future_content","payload":[1,2]},"extensions":{}}}"#
        );
        assert!(
            serde_json::from_str::<MessageContent>(r#"{"future_content":{"payload":[1,2]}}"#)
                .is_err()
        );

        Ok(())
    }
}
