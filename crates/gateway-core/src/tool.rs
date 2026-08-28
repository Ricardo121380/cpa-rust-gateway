//! Protocol-neutral Tool declarations retained by canonical requests.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{RawExtensions, RawJson};

/// A Tool declaration that a client permits an upstream to call.
///
/// The input schema remains raw JSON because the core does not interpret JSON Schema or apply
/// provider-specific validation rules.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDefinition {
    /// Stable Tool name supplied by the client.
    pub name: String,
    /// Optional human-readable Tool description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema retained without parsing or normalization.
    pub input_schema: RawJson,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for ToolDefinition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolDefinition")
            .field("name", &"<redacted>")
            .field("description_present", &self.description.is_some())
            .field("input_schema", &self.input_schema)
            .field("extensions", &self.extensions)
            .finish()
    }
}
