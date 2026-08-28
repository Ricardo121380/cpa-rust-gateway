//! Explicitly opt-in and visibly emulated Tool prompt composition for `grok.web`.
//!
//! The default is disabled and emits no prompt addendum. Even when enabled this boundary does not
//! claim native Tool Calling, parse model output, execute a tool, or send a Web request.

use std::{error::Error, fmt};

use gateway_catalog::{CapabilitySet, CatalogViewError, SemanticCapability};
use gateway_core::ToolDefinition;
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::strict_json::parse_strict_json;

const MAX_TOOL_COUNT: usize = 16;
const MAX_TOOL_NAME_BYTES: usize = 128;
const MAX_TOOL_DESCRIPTION_BYTES: usize = 1_024;
const MAX_TOOL_SCHEMA_BYTES: usize = 16 * 1024;
const MAX_EMULATION_PROMPT_BYTES: usize = 64 * 1024;
const EMULATION_PREFIX: &str = "[[gateway.tool_emulation.v1]]\n";
const EMULATION_SUFFIX: &str = "\n[[/gateway.tool_emulation.v1]]\n";

/// Explicit Tool support metadata for the Web provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebToolCapability {
    /// Tool emulation is not enabled; no Tool prompt is added and native Tools remain unavailable.
    Disabled,
    /// Tool descriptions are inserted as an opt-in prompt convention, not native Tool Calling.
    Emulated,
}

/// Explicit per-request configuration for optional Web Tool prompt composition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokWebToolEmulation {
    enabled: bool,
}

impl GrokWebToolEmulation {
    /// Creates the deliberately opt-in Tool-emulation setting.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Returns whether prompt composition is explicitly enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        self.enabled
    }

    /// Returns visible metadata that never calls the emulation path native Tool support.
    #[must_use]
    pub const fn tool_capability(self) -> GrokWebToolCapability {
        if self.enabled {
            GrokWebToolCapability::Emulated
        } else {
            GrokWebToolCapability::Disabled
        }
    }

    /// Returns only genuinely native semantic capabilities for the fixture Web adapter.
    ///
    /// The result intentionally excludes `Tools` and `ParallelTools` in both flag states.
    ///
    /// # Errors
    ///
    /// Returns `ParallelToolsRequiresTools` only if this static declaration is edited into an
    /// internally inconsistent state.
    pub fn native_semantic_capabilities() -> Result<CapabilitySet, CatalogViewError> {
        CapabilitySet::try_new([SemanticCapability::Streaming])
    }

    /// Prepares a bounded prompt addendum for declared Tools, if and only if enabled.
    ///
    /// The disabled path returns no prompt even if Tool definitions are present. It therefore
    /// cannot change a request's prompt bytes. The enabled path requires bounded, extension-free
    /// Tool declarations with JSON-object schemas and labels the prompt as emulated.
    ///
    /// # Errors
    ///
    /// Returns a value-free error without retaining invalid Tool material.
    pub fn prepare(
        self,
        tools: &[ToolDefinition],
    ) -> Result<GrokWebToolEmulationPrompt, GrokWebToolEmulationError> {
        if !self.enabled || tools.is_empty() {
            return Ok(GrokWebToolEmulationPrompt {
                capability: self.tool_capability(),
                addendum: None,
            });
        }
        if tools.len() > MAX_TOOL_COUNT {
            return Err(GrokWebToolEmulationError::TooManyTools);
        }
        let mut rendered_tools = Vec::with_capacity(tools.len());
        for tool in tools {
            rendered_tools.push(render_tool(tool)?);
        }
        let encoded = serde_json::to_string(&Value::Object(Map::from_iter([
            ("mode".to_owned(), Value::String("emulated".to_owned())),
            ("tools".to_owned(), Value::Array(rendered_tools)),
        ])))
        .map_err(|_| GrokWebToolEmulationError::PromptEncoding)?;
        let addendum = format!("{EMULATION_PREFIX}{encoded}{EMULATION_SUFFIX}");
        if addendum.len() > MAX_EMULATION_PROMPT_BYTES {
            return Err(GrokWebToolEmulationError::PromptTooLarge);
        }
        Ok(GrokWebToolEmulationPrompt {
            capability: GrokWebToolCapability::Emulated,
            addendum: Some(Zeroizing::new(addendum)),
        })
    }
}

/// One prepared, explicitly labelled Tool-emulation prompt addendum.
#[derive(Eq, PartialEq)]
pub struct GrokWebToolEmulationPrompt {
    capability: GrokWebToolCapability,
    addendum: Option<Zeroizing<String>>,
}

impl GrokWebToolEmulationPrompt {
    /// Returns the emitted Tool support metadata.
    #[must_use]
    pub const fn capability(&self) -> GrokWebToolCapability {
        self.capability
    }

    /// Returns whether this specific request obtains an emulation prompt addendum.
    #[must_use]
    pub const fn has_addendum(&self) -> bool {
        self.addendum.is_some()
    }

    pub(crate) fn compose_message(&self, user_message: &str) -> Zeroizing<String> {
        let mut message = Zeroizing::new(String::new());
        if let Some(addendum) = &self.addendum {
            message.push_str(addendum);
        }
        message.push_str(user_message);
        message
    }
}

impl fmt::Debug for GrokWebToolEmulationPrompt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebToolEmulationPrompt")
            .field("capability", &self.capability)
            .field("has_addendum", &self.addendum.is_some())
            .finish()
    }
}

/// Safe Tool-emulation declaration or prompt composition error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebToolEmulationError {
    /// More than the bounded number of Tool declarations was supplied.
    TooManyTools,
    /// A Tool name was empty, oversized, or unsafe for the emulation convention.
    InvalidToolName,
    /// A Tool description was oversized or included control characters.
    InvalidToolDescription,
    /// A Tool schema was oversized, malformed, or not a JSON object.
    InvalidToolSchema,
    /// A Tool carried unsupported Canonical extensions.
    UnsupportedToolExtensions,
    /// Local prompt serialization failed.
    PromptEncoding,
    /// The bounded aggregate prompt would be too large.
    PromptTooLarge,
}

impl fmt::Display for GrokWebToolEmulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooManyTools => "Grok Web Tool emulation has too many Tools",
            Self::InvalidToolName => "Grok Web Tool emulation Tool name is invalid",
            Self::InvalidToolDescription => "Grok Web Tool emulation Tool description is invalid",
            Self::InvalidToolSchema => "Grok Web Tool emulation Tool schema is invalid",
            Self::UnsupportedToolExtensions => {
                "Grok Web Tool emulation Tool extensions are unsupported"
            }
            Self::PromptEncoding => "Grok Web Tool emulation prompt could not be encoded",
            Self::PromptTooLarge => "Grok Web Tool emulation prompt is too large",
        })
    }
}

impl Error for GrokWebToolEmulationError {}

fn render_tool(tool: &ToolDefinition) -> Result<Value, GrokWebToolEmulationError> {
    if !tool.extensions.is_empty() {
        return Err(GrokWebToolEmulationError::UnsupportedToolExtensions);
    }
    if tool.name.is_empty()
        || tool.name.len() > MAX_TOOL_NAME_BYTES
        || !tool.name.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(GrokWebToolEmulationError::InvalidToolName);
    }
    if let Some(description) = &tool.description
        && (description.len() > MAX_TOOL_DESCRIPTION_BYTES
            || !description
                .bytes()
                .all(|byte| byte.is_ascii_graphic() || byte == b' '))
    {
        return Err(GrokWebToolEmulationError::InvalidToolDescription);
    }
    let schema_text = tool.input_schema.get();
    if schema_text.len() > MAX_TOOL_SCHEMA_BYTES {
        return Err(GrokWebToolEmulationError::InvalidToolSchema);
    }
    let schema = parse_strict_json(schema_text.as_bytes(), MAX_TOOL_SCHEMA_BYTES)
        .map_err(|()| GrokWebToolEmulationError::InvalidToolSchema)?;
    if !schema.is_object() {
        return Err(GrokWebToolEmulationError::InvalidToolSchema);
    }
    let mut object = Map::from_iter([
        ("name".to_owned(), Value::String(tool.name.clone())),
        ("input_schema".to_owned(), schema),
    ]);
    if let Some(description) = &tool.description {
        object.insert(
            "description".to_owned(),
            Value::String(description.to_owned()),
        );
    }
    Ok(Value::Object(object))
}
