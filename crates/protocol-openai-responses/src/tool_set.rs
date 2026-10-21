//! Closed native function declarations and identity, independent of Provider qualification.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use gateway_core::{CanonicalRequest, GatewayError, MessageContent, RawExtensions, RawJson};
use serde_json::Value;

use super::{client_request_error, reject_duplicate_json_names};

/// Existing extension envelope containing the entire ordered native declaration set.
pub const RESPONSES_TOOLS: &str = "openai.responses.tools";

/// Function identity. The correlation call ID deliberately is not part of this identity.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct FunctionIdentity {
    namespace: Option<String>,
    name: String,
}

impl FunctionIdentity {
    /// Constructs a top-level function identity without changing its name.
    #[must_use]
    pub fn flat(name: &str) -> Self {
        Self {
            namespace: None,
            name: name.to_owned(),
        }
    }

    /// Reads a call's reviewed namespace member; absence means a top-level function.
    /// Calls require a string when supplied; nullable identity members belong to results only.
    ///
    /// # Errors
    /// Rejects invalid names or namespace shapes without reporting their values.
    pub fn from_call(name: &str, extensions: &RawExtensions) -> Result<Self, GatewayError> {
        if !valid_name(name) {
            return Err(client_request_error());
        }
        let namespace = optional_member(extensions, "namespace")?;
        if extensions.get("namespace").is_some() && namespace.is_none() {
            return Err(client_request_error());
        }
        Ok(Self {
            namespace,
            name: name.to_owned(),
        })
    }

    /// Whether a flat forced selector selects this exact identity.
    #[must_use]
    pub fn is_flat_named(&self, name: &str) -> bool {
        self.namespace.is_none() && self.name == name
    }
}

impl fmt::Debug for FunctionIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FunctionIdentity(<redacted>)")
    }
}

/// Validated execution view over an ordered, unflattened Responses tool array.
#[derive(Clone, Eq, PartialEq)]
pub struct ResponsesToolSet {
    wire: Value,
    functions: BTreeSet<FunctionIdentity>,
    web_search: bool,
}

impl ResponsesToolSet {
    /// Validates the function-only namespace subset, including recursive duplicate JSON names.
    ///
    /// # Errors
    /// Rejects unknown members, deferred/custom/nested tools, duplicate groups/functions or bounds.
    pub fn decode(raw: &RawJson) -> Result<Self, GatewayError> {
        if raw.get().len() > 256 * 1024 {
            return Err(client_request_error());
        }
        reject_duplicate_json_names(raw.get())?;
        let wire: Value = serde_json::from_str(raw.get()).map_err(|_| client_request_error())?;
        let tools = wire
            .as_array()
            .filter(|v| v.len() <= 128)
            .ok_or_else(client_request_error)?;
        let mut functions = BTreeSet::new();
        let mut groups = BTreeSet::new();
        let mut web_search = false;
        for tool in tools {
            if tool.get("type").and_then(Value::as_str) == Some("web_search") {
                let fields = tool.as_object().ok_or_else(client_request_error)?;
                if web_search
                    || tool.to_string().len() > 256
                    || fields.len() != 2
                    || fields.get("external_web_access") != Some(&Value::Bool(false))
                {
                    return Err(client_request_error());
                }
                web_search = true;
            } else if tool.get("type").and_then(Value::as_str) == Some("namespace") {
                let fields = tool.as_object().ok_or_else(client_request_error)?;
                if fields
                    .keys()
                    .any(|key| !matches!(key.as_str(), "type" | "name" | "description" | "tools"))
                    || !fields.get("description").is_some_and(Value::is_string)
                {
                    return Err(client_request_error());
                }
                let name = tool_name(tool)?;
                if !groups.insert(name.to_owned()) {
                    return Err(client_request_error());
                }
                let children = tool
                    .get("tools")
                    .and_then(Value::as_array)
                    .filter(|v| !v.is_empty() && v.len() <= 128)
                    .ok_or_else(client_request_error)?;
                for child in children {
                    insert_function(child, Some(name), &mut functions)?;
                }
            } else {
                insert_function(tool, None, &mut functions)?;
            }
        }
        if groups.is_empty() && !web_search || functions.len() > 256 {
            return Err(client_request_error());
        }
        Ok(Self {
            wire,
            functions,
            web_search,
        })
    }

    /// Reads the existing envelope and refuses competing typed declarations.
    ///
    /// # Errors
    /// Rejects an invalid envelope or any collision with the legacy flat representation.
    pub fn from_request(request: &CanonicalRequest) -> Result<Option<Self>, GatewayError> {
        request
            .extensions
            .get(RESPONSES_TOOLS)
            .map(|raw| {
                if !request.tools.is_empty() {
                    return Err(client_request_error());
                }
                Self::decode(raw)
            })
            .transpose()
    }

    /// Original ordered declaration value, including group descriptions and child controls.
    #[must_use]
    pub const fn wire(&self) -> &Value {
        &self.wire
    }

    /// Complete execution identities; the ordered representation remains in `wire`.
    #[must_use]
    pub const fn functions(&self) -> &BTreeSet<FunctionIdentity> {
        &self.functions
    }

    /// Whether the complete set declares the independently qualified hosted search tool.
    #[must_use]
    pub const fn has_web_search(&self) -> bool {
        self.web_search
    }

    /// Confirms history against full declarations and results against the exact preceding call ID.
    ///
    /// # Errors
    /// Rejects undeclared identities, orphan/reused calls, and contradictory optional result names.
    pub fn validate_history(&self, request: &CanonicalRequest) -> Result<(), GatewayError> {
        request.validate_tool_history()?;
        crate::web_search::validate_history(request)?;
        validate_identity_history(request, Some(&self.functions))
    }
}

/// Checks supplied result identities against call IDs, with optional declaration membership.
/// The core independently owns ordered, complete call/result lifecycle validation.
///
/// # Errors
/// Rejects contradictory optional names, invalid calls, or undeclared complete identities.
pub fn validate_identity_history(
    request: &CanonicalRequest,
    declared: Option<&BTreeSet<FunctionIdentity>>,
) -> Result<(), GatewayError> {
    let mut calls = BTreeMap::new();
    for part in request.messages.iter().flat_map(|m| &m.content) {
        match part {
            MessageContent::ToolCall(call) => {
                let identity = FunctionIdentity::from_call(&call.name, &call.extensions)?;
                reject_duplicate_json_names(call.arguments.get())?;
                if declared.is_some_and(|set| !set.contains(&identity)) {
                    return Err(client_request_error());
                }
                calls.insert(call.id.as_str(), identity);
            }
            MessageContent::ToolResult(result) => {
                let identity = calls
                    .get(result.call_id.as_str())
                    .ok_or_else(client_request_error)?;
                if optional_member(&result.extensions, "name")?.is_some_and(|v| v != identity.name)
                    || optional_member(&result.extensions, "namespace")?
                        .is_some_and(|v| Some(v) != identity.namespace)
                {
                    return Err(client_request_error());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

impl fmt::Debug for ResponsesToolSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponsesToolSet")
            .field("functions", &self.functions.len())
            .finish_non_exhaustive()
    }
}

/// Whether this request uses any part of the native namespace field family.
#[must_use]
pub fn uses_namespaces(request: &CanonicalRequest) -> bool {
    request.extensions.get(RESPONSES_TOOLS).is_some_and(|raw| {
        serde_json::from_str::<Value>(raw.get()).is_ok_and(|v| {
            v.as_array().is_some_and(|tools| {
                tools
                    .iter()
                    .any(|tool| tool.get("type").and_then(Value::as_str) == Some("namespace"))
            })
        })
    }) || request
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .any(|p| match p {
            MessageContent::ToolCall(v) => v.extensions.get("namespace").is_some(),
            MessageContent::ToolResult(v) => v.extensions.get("namespace").is_some(),
            _ => false,
        })
}

/// Whether any C1–C3 native tool identity member requires adapter/target/mode qualification.
/// Result `name` is newly admitted independently of `namespace`, including explicit null.
#[must_use]
pub fn requires_tool_identity_qualification(request: &CanonicalRequest) -> bool {
    uses_namespaces(request)
        || request
            .messages
            .iter()
            .flat_map(|m| &m.content)
            .any(|part| {
                matches!(part, MessageContent::ToolResult(v) if v.extensions.get("name").is_some())
            })
}

/// Validates an optional result identity member without conflating absence, null and value.
///
/// # Errors
/// Rejects anything other than absent, null, or a valid function/namespace name.
pub fn optional_member(
    extensions: &RawExtensions,
    key: &str,
) -> Result<Option<String>, GatewayError> {
    extensions
        .get(key)
        .map(|raw| {
            let value: Value =
                serde_json::from_str(raw.get()).map_err(|_| client_request_error())?;
            match value {
                Value::Null => Ok(None),
                Value::String(v) if valid_name(&v) => Ok(Some(v)),
                _ => Err(client_request_error()),
            }
        })
        .transpose()
        .map(Option::flatten)
}

fn tool_name(tool: &Value) -> Result<&str, GatewayError> {
    tool.get("name")
        .and_then(Value::as_str)
        .filter(|v| valid_name(v))
        .ok_or_else(client_request_error)
}

fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

fn insert_function(
    tool: &Value,
    namespace: Option<&str>,
    functions: &mut BTreeSet<FunctionIdentity>,
) -> Result<(), GatewayError> {
    let fields = tool.as_object().ok_or_else(client_request_error)?;
    if functions.len() >= 256
        || tool.get("type").and_then(Value::as_str) != Some("function")
        || fields.keys().any(|key| {
            !matches!(
                key.as_str(),
                "type" | "name" | "description" | "parameters" | "strict"
            )
        })
        || tool
            .get("description")
            .is_some_and(|v| !v.is_null() && !v.is_string())
        || tool.get("parameters").is_some_and(|v| !v.is_object())
        || tool.get("strict").is_some_and(|v| !v.is_boolean())
        || !functions.insert(FunctionIdentity {
            namespace: namespace.map(str::to_owned),
            name: tool_name(tool)?.to_owned(),
        })
    {
        return Err(client_request_error());
    }
    Ok(())
}
