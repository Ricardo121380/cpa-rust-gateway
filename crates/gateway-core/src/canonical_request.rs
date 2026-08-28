//! Canonical inbound request semantics shared by later gateway layers.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{CanonicalMessage, RawExtensions, Thinking, ToolDefinition};

/// A protocol-neutral request before model resolution, routing, and provider selection.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalRequest {
    /// Client-supplied model reference before alias or public-model resolution.
    pub requested_model: String,
    /// Ordered request messages.
    pub messages: Vec<CanonicalMessage>,
    /// Tool declarations that the client permits for this request.
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    /// Explicit client thinking configuration, if supplied.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_thinking",
        skip_serializing_if = "Option::is_none"
    )]
    pub thinking: Option<Thinking>,
    /// Client-supplied cache key retained without deriving provider cache identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    /// Client-supplied cache retention request retained without provider interpretation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<String>,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

fn deserialize_optional_thinking<'de, D>(deserializer: D) -> Result<Option<Thinking>, D::Error>
where
    D: Deserializer<'de>,
{
    Thinking::deserialize(deserializer).map(Some)
}

impl fmt::Debug for CanonicalRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalRequest")
            .field("requested_model", &"<redacted>")
            .field("message_count", &self.messages.len())
            .field("tool_count", &self.tools.len())
            .field("thinking_configured", &self.thinking.is_some())
            .field("prompt_cache_key_present", &self.prompt_cache_key.is_some())
            .field(
                "prompt_cache_retention_present",
                &self.prompt_cache_retention.is_some(),
            )
            .field("extensions", &self.extensions)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::CanonicalRequest;
    use crate::{MessageContent, RawJson};

    #[test]
    fn canonical_request_round_trips_through_json_and_memory() -> Result<(), serde_json::Error> {
        let request: CanonicalRequest = serde_json::from_str(include_str!(
            "../../../tests/fixtures/core/canonical-request-roundtrip.json"
        ))?;
        let serialized = serde_json::to_string(&request)?;
        let restored: CanonicalRequest = serde_json::from_str(&serialized)?;

        assert_eq!(request, restored);
        assert_eq!(request, request.clone());
        assert_eq!(request.messages.len(), 3);
        assert_eq!(request.messages[0].content.len(), 2);
        assert_eq!(request.tools.len(), 1);
        assert_eq!(request.messages[0].role.0, "developer");
        assert_eq!(request.messages[2].role.0, "tool");
        assert_eq!(
            request.tools[0].input_schema.get(),
            r#"{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}"#
        );
        assert_eq!(
            request.extensions.get("vendor_request").map(RawJson::get),
            Some(r#"{"mode":"audit","flags":[true,false]}"#)
        );

        let has_text_extension = matches!(
            &request.messages[0].content[0],
            MessageContent::Text(content)
                if content.extensions.get("vendor_text").map(RawJson::get)
                    == Some(r#"{"priority": 1}"#)
        );
        assert!(has_text_extension);

        let has_opaque_content = matches!(
            &request.messages[0].content[1],
            MessageContent::Opaque(content)
                if content.raw().get()
                    == r#"{"kind":"input_image","image_url":{"url":"https://example.invalid/fake.png"},"extensions":{"vendor_media":{"safe":true}}}"#
        );
        assert!(has_opaque_content);

        let has_tool_call = matches!(
            &request.messages[1].content[0],
            MessageContent::ToolCall(content) if content.arguments.get() == r#"{"query":"weather"}"#
        );
        assert!(has_tool_call);

        let has_tool_result = matches!(
            &request.messages[2].content[0],
            MessageContent::ToolResult(content) if content.output.get() == r#"{"forecast":"clear"}"#
        );
        assert!(has_tool_result);

        Ok(())
    }

    #[test]
    fn known_content_rejects_missing_required_fields() {
        let decoded = serde_json::from_str::<CanonicalRequest>(
            r#"{"requested_model":"gateway-model","messages":[{"role":"user","content":[{"text":{"extensions":{}}}],"extensions":{}}],"extensions":{}}"#,
        );

        assert!(decoded.is_err());
    }

    #[test]
    fn optional_thinking_and_multiple_tools_preserve_their_structure()
    -> Result<(), serde_json::Error> {
        let request: CanonicalRequest = serde_json::from_str(
            r#"{"requested_model":"gateway-model","messages":[],"tools":[{"name":"first","input_schema":{},"extensions":{}},{"name":"second","input_schema":{},"extensions":{}}],"extensions":{}}"#,
        )?;
        let serialized = serde_json::to_string(&request)?;
        let restored: CanonicalRequest = serde_json::from_str(&serialized)?;

        assert!(request.thinking.is_none());
        assert_eq!(request.tools.len(), 2);
        assert_eq!(request.tools[0].name, "first");
        assert_eq!(request.tools[1].name, "second");
        assert_eq!(request, restored);
        Ok(())
    }

    #[test]
    fn supplied_thinking_requires_a_non_empty_effort() {
        for invalid_request in [
            r#"{"requested_model":"gateway-model","messages":[],"thinking":null,"extensions":{}}"#,
            r#"{"requested_model":"gateway-model","messages":[],"thinking":{"extensions":{}},"extensions":{}}"#,
            r#"{"requested_model":"gateway-model","messages":[],"thinking":{"effort":"","extensions":{}},"extensions":{}}"#,
        ] {
            assert!(serde_json::from_str::<CanonicalRequest>(invalid_request).is_err());
        }
    }

    #[test]
    fn canonical_request_debug_redacts_client_values() -> Result<(), serde_json::Error> {
        let request: CanonicalRequest = serde_json::from_str(include_str!(
            "../../../tests/fixtures/core/canonical-request-roundtrip.json"
        ))?;
        let diagnostic = format!("{request:?}");

        for sensitive_value in [
            "gateway-model",
            "Use the available tools safely.",
            "Look up a forecast.",
            "cache-key-01",
            "24h",
        ] {
            assert!(!diagnostic.contains(sensitive_value));
        }
        assert!(diagnostic.contains("CanonicalRequest"));
        assert!(diagnostic.contains("message_count: 3"));

        Ok(())
    }

    #[test]
    fn raw_json_rejects_an_incomplete_value() {
        assert!(RawJson::from_json_string("{\"incomplete\":".to_owned()).is_err());
    }
}
