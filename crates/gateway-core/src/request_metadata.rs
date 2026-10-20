//! Reviewed private request diagnostics, independent of Usage, reasoning and policy authority.
use crate::{ErrorScope, GatewayError, GatewayErrorCode, RawExtensions};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

/// The sole reviewed request extension consumed by the local Codex diagnostic policy.
pub const CODEX_CLIENT_METADATA: &str = "openai.responses.client_metadata";
/// Maximum complete raw field, including JSON escaping and whitespace. Never truncate.
pub const MAX_REQUEST_METADATA_BYTES: usize = 64 * 1024;

/// A complete original client field. It is not an ordinary event, authority or upstream input.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestMetadataEvidence {
    raw_json: String,
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
        Ok(Self { raw_json })
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
            "sandbox" => value == "seatbelt",
            "sandbox_mode" => value == "workspace-write",
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
    Ok(())
}
