//! Model suffixes use the same source extraction and frozen model normalization as body controls.
use super::*;
use gateway_core::ModelThinking;

fn invalid() -> ProtocolTransformRejection {
    ProtocolTransformRejection::ThinkingUnsupported
}

fn without(
    fields: &RawExtensions,
    name: &str,
) -> Result<RawExtensions, ProtocolTransformRejection> {
    let mut result = RawExtensions::default();
    for (key, value) in fields.iter().filter(|(key, _)| *key != name) {
        result
            .try_insert(key, value.clone())
            .map_err(|_| invalid())?;
    }
    Ok(result)
}

pub(super) fn source_request(
    request: &CanonicalRequest,
    source: ProtocolFormat,
) -> Result<CanonicalRequest, ProtocolTransformRejection> {
    apply_source_request(request, source, false)
}

fn validate_body_controls(
    request: &CanonicalRequest,
    source: ProtocolFormat,
    native: bool,
) -> Result<(), ProtocolTransformRejection> {
    if source != ProtocolFormat::AnthropicMessages {
        validate_target_thinking(request.thinking.as_ref(), source)?;
        return validate_target_root_extensions(request, source);
    }
    let mut body = request.clone();
    body.model_thinking = None;
    if native {
        if let Some(raw) = request.extensions.get("anthropic.messages.output_config") {
            let value: serde_json::Map<String, Value> =
                serde_json::from_str(raw.get()).map_err(|_| invalid())?;
            let mut effort = serde_json::Map::new();
            if let Some(value) = value.get("effort") {
                effort.insert("effort".into(), value.clone());
            }
            model_thinking::put(
                &mut body.extensions,
                "anthropic.messages.output_config",
                Value::Object(effort),
            )?;
        }
        if let Some(thinking) = body.thinking.as_mut()
            && thinking.effort.as_str() == "between_tools"
        {
            validate_target_thinking(Some(thinking), source)?;
            thinking.effort = ThinkingEffort::try_new("disabled").map_err(|_| invalid())?;
        }
    }
    compatibility::thinking(&body, source, ProtocolFormat::OpenAiResponses, None)?;
    Ok(())
}

fn apply_source_request(
    request: &CanonicalRequest,
    source: ProtocolFormat,
    native: bool,
) -> Result<CanonicalRequest, ProtocolTransformRejection> {
    let Some(control) = &request.model_thinking else {
        return Ok(request.clone());
    };
    // Validate body syntax with name intent removed; winning precedence cannot validate losers.
    validate_body_controls(request, source, native)?;
    let mut result = request.clone();
    let effort = match control {
        ModelThinking::Budget(budget) if *budget >= -1 => compatibility::budget_level(*budget),
        ModelThinking::Budget(_) => return Err(invalid()),
        ModelThinking::Effort(effort) => effort.as_str(),
    };
    if !matches!(
        effort,
        "none" | "auto" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
    ) {
        return Err(invalid());
    }
    let mut extensions = RawExtensions::default();
    let label = if source == ProtocolFormat::AnthropicMessages {
        if let Some(thinking) = &request.thinking {
            extensions = without(&thinking.extensions, ANTHROPIC_THINKING_BUDGET)?;
        }
        let old = request.extensions.get("anthropic.messages.output_config");
        let mut output = old
            .map(|raw| {
                serde_json::from_str::<serde_json::Map<String, Value>>(raw.get())
                    .map_err(|_| invalid())
            })
            .transpose()?
            .unwrap_or_default();
        output.remove("effort");
        result.extensions = without(&result.extensions, "anthropic.messages.output_config")?;
        let label = match control {
            ModelThinking::Budget(budget) if *budget > 0 => {
                model_thinking::put(
                    &mut extensions,
                    ANTHROPIC_THINKING_BUDGET,
                    Value::from(*budget),
                )?;
                "enabled"
            }
            _ if effort == "none" => "disabled",
            _ if effort == "auto" => "adaptive",
            _ => {
                output.insert("effort".into(), Value::from(effort));
                "adaptive"
            }
        };
        if !output.is_empty() {
            model_thinking::put(
                &mut result.extensions,
                "anthropic.messages.output_config",
                Value::Object(output),
            )?;
        }
        label
    } else {
        if let Some(thinking) = &request.thinking {
            extensions = thinking.extensions.clone();
        }
        if source == ProtocolFormat::OpenAiResponses
            && let Some(raw) = request.extensions.get("openai.responses.reasoning")
        {
            let summary: RawExtensions = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
            for (key, value) in summary.iter() {
                extensions
                    .try_insert(key, value.clone())
                    .map_err(|_| invalid())?;
            }
            result.extensions = without(&result.extensions, "openai.responses.reasoning")?;
        }
        effort
    };
    result.thinking = Some(Thinking {
        effort: ThinkingEffort::try_new(label).map_err(|_| invalid())?,
        extensions,
    });
    Ok(result)
}

pub(super) fn native_request(
    request: &CanonicalRequest,
    source: ProtocolFormat,
    model: &str,
) -> Result<CanonicalRequest, ProtocolTransformRejection> {
    let mut result = apply_source_request(request, source, true)?;
    let original = result.clone();
    model_thinking::normalize(&original, &mut result, source, source, Some(model))?;
    validate_target_thinking(result.thinking.as_ref(), source)?;
    model_thinking::validate_native(&result, source, Some(model))?;
    Ok(result)
}

/// Overlays only validated name controls onto native JSON; other native values are preserved.
///
/// # Errors
/// Rejects malformed body controls, unsupported model controls or invalid native JSON.
pub fn model_name_native_payload(
    payload: &[u8],
    request: &CanonicalRequest,
    source: ProtocolFormat,
    model: &str,
) -> Result<Vec<u8>, ProtocolTransformRejection> {
    let effective = native_request(request, source, model)?;
    let mut body: Value = serde_json::from_slice(payload).map_err(|_| invalid())?;
    let root = body.as_object_mut().ok_or_else(invalid)?;
    let thinking = effective.thinking.as_ref().ok_or_else(invalid)?;
    match source {
        ProtocolFormat::AnthropicMessages => {
            let mut fields = serde_json::Map::new();
            fields.insert("type".into(), Value::from(thinking.effort.as_str()));
            for (key, raw) in thinking.extensions.iter() {
                let key = key
                    .strip_prefix("anthropic.thinking.")
                    .ok_or_else(invalid)?;
                fields.insert(
                    key.into(),
                    serde_json::from_str(raw.get()).map_err(|_| invalid())?,
                );
            }
            root.insert("thinking".into(), Value::Object(fields));
            if let Some(raw) = effective.extensions.get("anthropic.messages.output_config") {
                root.insert(
                    "output_config".into(),
                    serde_json::from_str(raw.get()).map_err(|_| invalid())?,
                );
            } else {
                root.remove("output_config");
            }
        }
        ProtocolFormat::OpenAiChatCompletions => {
            root.insert(
                "reasoning_effort".into(),
                Value::from(thinking.effort.as_str()),
            );
        }
        ProtocolFormat::OpenAiResponses => {
            let mut fields = serde_json::Map::new();
            fields.insert("effort".into(), Value::from(thinking.effort.as_str()));
            for (key, raw) in thinking.extensions.iter() {
                fields.insert(
                    key.into(),
                    serde_json::from_str(raw.get()).map_err(|_| invalid())?,
                );
            }
            root.insert("reasoning".into(), Value::Object(fields));
        }
    }
    serde_json::to_vec(&body).map_err(|_| invalid())
}

/// Checks name-control syntax/capability for counting without creating an inference request.
///
/// # Errors
/// Rejects explicit lossless routes, models without thinking, or missing Reasoning capability.
pub fn validate_model_name_control(
    request: &CanonicalRequest,
    model: &str,
    mode: SnapshotTransformMode,
    capabilities: &CapabilitySet,
) -> Result<(), ProtocolTransformRejection> {
    if request.model_thinking.is_none() {
        return Ok(());
    }
    source_request(request, ProtocolFormat::AnthropicMessages)?;
    if mode == SnapshotTransformMode::LosslessBridge
        || !capabilities.supports(SemanticCapability::Reasoning)
        || !model_thinking::supports_name(model)?
    {
        return Err(invalid());
    }
    Ok(())
}
