//! Reviewed private request diagnostics, independent of Usage, reasoning and policy authority.
use crate::{ErrorScope, GatewayError, GatewayErrorCode, GatewayProtocol, RawExtensions};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

/// The sole reviewed request extension consumed by the local Codex diagnostic policy.
pub const CODEX_CLIENT_METADATA: &str = "openai.responses.client_metadata";
/// The separately reviewed Claude M4 local diagnostic field.
pub const ANTHROPIC_MESSAGES_METADATA: &str = "anthropic.messages.metadata";
/// Maximum complete raw field, including JSON escaping and whitespace. Never truncate.
pub const MAX_REQUEST_METADATA_BYTES: usize = 64 * 1024;

/// Reviewed field provenance, never a claim about the client product's authenticity.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestMetadataSource {
    /// The original C4 payload v1; its serialization and AAD remain unchanged.
    #[default]
    CodexClientMetadata,
    /// Claude M4 opaque user identifier, independently admitted as payload v2.
    AnthropicMessagesMetadata,
}
impl RequestMetadataSource {
    /// Exact downstream protocol that must own this capture.
    #[must_use]
    pub const fn protocol(self) -> GatewayProtocol {
        match self {
            Self::CodexClientMetadata => GatewayProtocol::OpenAiResponses,
            Self::AnthropicMessagesMetadata => GatewayProtocol::AnthropicMessages,
        }
    }

    /// Persisted payload version; source/protocol cannot be independently relabeled.
    #[must_use]
    pub const fn payload_version(self) -> u16 {
        match self {
            Self::CodexClientMetadata => 1,
            Self::AnthropicMessagesMetadata => 2,
        }
    }
}

/// A complete original client field. It is not an ordinary event, authority or upstream input.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestMetadataEvidence {
    raw_json: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<RequestMetadataSource>,
}
impl RequestMetadataEvidence {
    /// Validates the closed C4 subset without changing its original representation.
    ///
    /// # Errors
    /// Returns a value-free client error for unknown members, malformed values or bounds.
    pub fn try_new(raw_json: String) -> Result<Self, GatewayError> {
        let invalid =
            || GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request);
        if raw_json.len() > MAX_REQUEST_METADATA_BYTES {
            return Err(invalid());
        }
        let fields: RawExtensions = serde_json::from_str(&raw_json).map_err(|_| invalid())?;
        let mut strings = BTreeMap::new();
        for (name, raw) in fields.iter() {
            if !matches!(
                name,
                "root_turn_id"
                    | "session_id"
                    | "thread_id"
                    | "turn_id"
                    | "x-codex-installation-id"
                    | "x-codex-turn-metadata"
                    | "x-codex-window-id"
            ) {
                return Err(invalid());
            }
            let value: String = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
            if name != "x-codex-turn-metadata" && value.len() > 1024 {
                return Err(invalid());
            }
            strings.insert(name, value);
        }
        if let Some(blob) = strings.get("x-codex-turn-metadata") {
            validate_blob(blob, &strings)?;
        }
        Ok(Self {
            raw_json,
            source: None,
        })
    }

    /// Validates only M4's optional opaque `user_id`, preserving its complete raw JSON token.
    /// Inner JSON-looking strings are never parsed, normalized or interpreted.
    /// # Errors
    /// Rejects null, unknown/duplicate members, non-strings and values exceeding either bound.
    pub fn try_new_anthropic(raw_json: String) -> Result<Self, GatewayError> {
        let invalid =
            || GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request);
        if raw_json.len() > MAX_REQUEST_METADATA_BYTES {
            return Err(invalid());
        }
        let fields: RawExtensions = serde_json::from_str(&raw_json).map_err(|_| invalid())?;
        for (name, raw) in fields.iter() {
            if name != "user_id" {
                return Err(invalid());
            }
            let value: String = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
            if value.chars().count() > 512 {
                return Err(invalid());
            }
        }
        Ok(Self {
            raw_json,
            source: Some(RequestMetadataSource::AnthropicMessagesMetadata),
        })
    }

    /// Revalidates a deserialized private payload at its storage trust boundary.
    /// # Errors
    /// Rejects fields outside the source-specific contract.
    pub fn validate(&self) -> Result<(), GatewayError> {
        let validated = match self.source() {
            RequestMetadataSource::CodexClientMetadata => Self::try_new(self.raw_json.clone()),
            RequestMetadataSource::AnthropicMessagesMetadata => {
                Self::try_new_anthropic(self.raw_json.clone())
            }
        }?;
        if validated != *self {
            return Err(GatewayError::new(
                GatewayErrorCode::ClientRequestError,
                ErrorScope::Request,
            ));
        }
        Ok(())
    }

    /// Exact reviewed source of this field, authenticated with its owning Request.
    #[must_use]
    pub const fn source(&self) -> RequestMetadataSource {
        match self.source {
            Some(source) => source,
            None => RequestMetadataSource::CodexClientMetadata,
        }
    }
    /// Original raw JSON field; read only through an authorized diagnostic boundary.
    #[must_use]
    pub fn raw_json(&self) -> &str {
        &self.raw_json
    }
}
impl fmt::Debug for RequestMetadataEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RequestMetadataEvidence(<private>)")
    }
}

fn validate_blob(blob: &str, outer: &BTreeMap<&str, String>) -> Result<(), GatewayError> {
    let invalid = || GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request);
    let fields: RawExtensions = serde_json::from_str(blob).map_err(|_| invalid())?;
    for (name, raw) in fields.iter() {
        let value: Value = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
        let valid = match name {
            "installation_id" | "session_id" | "thread_id" | "turn_id" | "root_turn_id"
            | "window_id" | "context_window_id" | "agent_name" | "model" => {
                value.as_str().is_some_and(|s| s.len() <= 1024)
            }
            "request_kind" => value == "turn",
            "thread_source" => value == "user",
            "turn_trigger" => value == "exec",
            "sandbox" => value == "seatbelt" || value == "external",
            "sandbox_mode" => value == "workspace-write" || value == "external-sandbox",
            "auto_review_enabled"
            | "node_repl_auto_review_required"
            | "node_repl_disabled"
            | "analytics_enabled" => value.is_boolean(),
            "window_number" => value.as_u64().is_some(),
            "turn_started_at_unix_ms" => value.as_i64().is_some_and(|v| v >= 0),
            _ => false,
        };
        if !valid {
            return Err(invalid());
        }
        let flat = match name {
            "installation_id" => "x-codex-installation-id",
            "window_id" => "x-codex-window-id",
            _ => name,
        };
        if let Some(expected) = outer.get(flat)
            && value.as_str() != Some(expected.as_str())
        {
            return Err(invalid());
        }
    }
    // The added external diagnostic selectors are admitted only as the observed pair.
    // They remain untrusted statements and never grant execution or confinement authority.
    let external = fields.get("sandbox").is_some_and(|raw| {
        serde_json::from_str::<String>(raw.get()).is_ok_and(|value| value == "external")
    });
    let external_mode = fields.get("sandbox_mode").is_some_and(|raw| {
        serde_json::from_str::<String>(raw.get()).is_ok_and(|value| value == "external-sandbox")
    });
    if external != external_mode {
        return Err(invalid());
    }
    Ok(())
}
