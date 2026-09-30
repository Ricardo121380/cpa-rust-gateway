//! Exact tool selection and parallel constraints shared by admission and response validation.

use std::{collections::BTreeSet, fmt};

use gateway_core::{CanonicalRequest, RawJson};
use serde_json::{Value, json};

use crate::{ProtocolFormat, ProtocolTransformRejection};

/// Selection that has an equivalent representation in the three supported protocols.
#[derive(Clone, Eq, PartialEq)]
pub enum ToolSelection {
    /// The model may choose whether to call a declared tool.
    Auto,
    /// The model must not call a tool.
    None,
    /// At least one declared tool must be called.
    Required,
    /// Only this declared function may be called, and at least one call is required.
    Named(String),
}

impl fmt::Debug for ToolSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Auto => "Auto",
            Self::None => "None",
            Self::Required => "Required",
            Self::Named(_) => "Named(<redacted>)",
        })
    }
}

/// Validated request controls retained for the lifetime of one execution.
#[derive(Clone, Eq, PartialEq)]
pub struct ToolExecutionConstraints {
    pub(crate) parallel: Option<bool>,
    pub(crate) selection: Option<ToolSelection>,
    pub(crate) declared: BTreeSet<String>,
}

impl ToolExecutionConstraints {
    /// Reads only reviewed protocol control fields and validates named selections and declarations.
    ///
    /// # Errors
    /// Returns a value-free admission rejection for malformed or ambiguous controls.
    pub fn from_request(
        request: &CanonicalRequest,
        source: ProtocolFormat,
    ) -> Result<Self, ProtocolTransformRejection> {
        let invalid = || ProtocolTransformRejection::ToolHistoryUnsupported;
        let mut declared = BTreeSet::new();
        for tool in &request.tools {
            if tool.name.is_empty()
                || tool.name.len() > 64
                || !tool
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                || !declared.insert(tool.name.clone())
                || !serde_json::from_str::<Value>(tool.input_schema.get())
                    .is_ok_and(|schema| schema.is_object())
            {
                return Err(invalid());
            }
        }
        let mut parallel = request
            .extensions
            .get(parallel_name(source))
            .map(|raw| {
                serde_json::from_str::<Value>(raw.get())
                    .ok()
                    .and_then(|value| value.as_bool())
                    .ok_or_else(invalid)
            })
            .transpose()?;
        let selection = request
            .extensions
            .get(choice_name(source))
            .map(|raw| decode_selection(raw, source, &mut parallel))
            .transpose()?;
        match &selection {
            Some(ToolSelection::Required) if declared.is_empty() => return Err(invalid()),
            Some(ToolSelection::Named(name)) if !declared.contains(name) => return Err(invalid()),
            _ => {}
        }
        Ok(Self {
            parallel,
            selection,
            declared,
        })
    }

    /// Whether this explicit request requires the endpoint's declared parallel capability.
    #[must_use]
    pub const fn requires_parallel_tools(&self) -> bool {
        matches!(self.parallel, Some(true))
    }

    pub(crate) fn mapped_choice(&self, target: ProtocolFormat) -> Option<Value> {
        let selection = self.selection.as_ref().or_else(|| {
            (target == ProtocolFormat::AnthropicMessages && self.parallel.is_some())
                .then_some(&ToolSelection::Auto)
        })?;
        let mut choice = match (target, selection) {
            (ProtocolFormat::AnthropicMessages, ToolSelection::Auto) => json!({"type":"auto"}),
            (ProtocolFormat::AnthropicMessages, ToolSelection::None) => json!({"type":"none"}),
            (ProtocolFormat::AnthropicMessages, ToolSelection::Required) => json!({"type":"any"}),
            (ProtocolFormat::AnthropicMessages, ToolSelection::Named(name)) => {
                json!({"type":"tool","name":name})
            }
            (_, ToolSelection::Auto) => json!("auto"),
            (_, ToolSelection::None) => json!("none"),
            (_, ToolSelection::Required) => json!("required"),
            (ProtocolFormat::OpenAiChatCompletions, ToolSelection::Named(name)) => {
                json!({"type":"function","function":{"name":name}})
            }
            (ProtocolFormat::OpenAiResponses, ToolSelection::Named(name)) => {
                json!({"type":"function","name":name})
            }
        };
        if target == ProtocolFormat::AnthropicMessages
            && let Some(parallel) = self.parallel
        {
            choice["disable_parallel_tool_use"] = json!(!parallel);
        }
        Some(choice)
    }
}

impl fmt::Debug for ToolExecutionConstraints {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolExecutionConstraints")
            .field("parallel", &self.parallel)
            .field("selection", &self.selection)
            .field("declared_count", &self.declared.len())
            .finish()
    }
}

pub(crate) const fn parallel_name(source: ProtocolFormat) -> &'static str {
    match source {
        ProtocolFormat::OpenAiChatCompletions => "openai.chat.parallel_tool_calls",
        ProtocolFormat::OpenAiResponses => "openai.responses.parallel_tool_calls",
        ProtocolFormat::AnthropicMessages => "anthropic.messages.parallel_tool_calls",
    }
}

pub(crate) const fn choice_name(source: ProtocolFormat) -> &'static str {
    match source {
        ProtocolFormat::OpenAiChatCompletions => "openai.chat.tool_choice",
        ProtocolFormat::OpenAiResponses => "openai.responses.tool_choice",
        ProtocolFormat::AnthropicMessages => "anthropic.messages.tool_choice",
    }
}

fn decode_selection(
    raw: &RawJson,
    source: ProtocolFormat,
    parallel: &mut Option<bool>,
) -> Result<ToolSelection, ProtocolTransformRejection> {
    let invalid = || ProtocolTransformRejection::UnknownRequestExtensions;
    let value: Value = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
    if source != ProtocolFormat::AnthropicMessages
        && let Some(choice) = value.as_str()
    {
        return match choice {
            "auto" => Ok(ToolSelection::Auto),
            "none" => Ok(ToolSelection::None),
            "required" => Ok(ToolSelection::Required),
            _ => Err(invalid()),
        };
    }
    let choice = value.as_object().ok_or_else(invalid)?;
    if source == ProtocolFormat::AnthropicMessages {
        if choice
            .keys()
            .any(|key| !matches!(key.as_str(), "type" | "name" | "disable_parallel_tool_use"))
        {
            return Err(invalid());
        }
        if let Some(disabled) = choice.get("disable_parallel_tool_use") {
            *parallel = Some(!disabled.as_bool().ok_or_else(invalid)?);
        }
        return match choice.get("type").and_then(Value::as_str) {
            Some("auto") if !choice.contains_key("name") => Ok(ToolSelection::Auto),
            Some("none") if !choice.contains_key("name") => Ok(ToolSelection::None),
            Some("any") if !choice.contains_key("name") => Ok(ToolSelection::Required),
            Some("tool") => Ok(ToolSelection::Named(
                choice
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .ok_or_else(invalid)?
                    .to_owned(),
            )),
            _ => Err(invalid()),
        };
    }
    if choice.len() != 2 || choice.get("type").and_then(Value::as_str) != Some("function") {
        return Err(invalid());
    }
    let name = if source == ProtocolFormat::OpenAiChatCompletions {
        let function = choice
            .get("function")
            .and_then(Value::as_object)
            .ok_or_else(invalid)?;
        if function.len() != 1 {
            return Err(invalid());
        }
        function.get("name").and_then(Value::as_str)
    } else {
        choice.get("name").and_then(Value::as_str)
    };
    Ok(ToolSelection::Named(
        name.filter(|name| !name.is_empty())
            .ok_or_else(invalid)?
            .to_owned(),
    ))
}
