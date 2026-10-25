//! Frozen CPA v8.0.40 thinking metadata and normalization; never access-grant evidence.
//! Source d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683:
//! internal/registry/models/models.json, internal/thinking/{validate,convert}.go,
//! internal/thinking/provider/claude/apply.go. MIT attribution: `THIRD_PARTY_NOTICES.md`.

use std::sync::OnceLock;

use super::{
    ANTHROPIC_THINKING_BUDGET, CanonicalRequest, ProtocolFormat, ProtocolTransformRejection,
    RawExtensions, RawJson, Thinking, ThinkingEffort, Value,
};

const LEVELS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];
static MODELS: OnceLock<Result<Value, serde_json::Error>> = OnceLock::new();

fn invalid() -> ProtocolTransformRejection {
    ProtocolTransformRejection::ThinkingUnsupported
}

fn profile(model: &str) -> Result<Option<&'static Value>, ProtocolTransformRejection> {
    let models = MODELS
        .get_or_init(|| serde_json::from_str(include_str!("cpa_model_thinking.json")))
        .as_ref()
        .map_err(|_| invalid())?
        .as_array()
        .ok_or_else(invalid)?;
    let exact = models.iter().find(|v| v["id"].as_str() == Some(model));
    if exact.is_some() {
        return Ok(exact);
    }
    // CPA LookupStaticModelInfo: Claude context and dated variants may reuse a known family.
    let name = model.split_once('[').map_or(model, |(base, _)| base);
    let name = name
        .rsplit_once('-')
        .filter(|(_, date)| date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()))
        .map_or(name, |(base, _)| base);
    Ok(models
        .iter()
        .find(|v| v["type"] == "claude" && v["id"].as_str() == Some(name)))
}

pub(super) fn put(
    fields: &mut RawExtensions,
    name: &str,
    value: &Value,
) -> Result<(), ProtocolTransformRejection> {
    // Replacement is local normalized output, not duplicate input admission.
    let mut replacement = RawExtensions::default();
    for (key, raw) in fields.iter().filter(|(key, _)| *key != name) {
        replacement
            .try_insert(key, raw.clone())
            .map_err(|_| invalid())?;
    }
    replacement
        .try_insert(
            name,
            RawJson::from_json_string(value.to_string()).map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())?;
    *fields = replacement;
    Ok(())
}

fn supported_level<'a>(
    level: &str,
    levels: &'a [Value],
) -> Result<&'a str, ProtocolTransformRejection> {
    let position = LEVELS
        .iter()
        .position(|v| *v == level)
        .ok_or_else(invalid)?;
    levels
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|v| {
            LEVELS
                .iter()
                .position(|l| *l == v)
                .map(|i| (i.abs_diff(position), i, v))
        })
        .min_by_key(|(distance, i, _)| (*distance, *i))
        .map(|(_, _, value)| value)
        .ok_or_else(invalid)
}

/// Normalizes only compatible projections. Native values and explicit lossless paths keep their
/// separate contracts. Unknown names use the existing ADR-0126 projection without guessed ranges.
pub(super) fn normalize(
    source_request: &CanonicalRequest,
    projected: &mut CanonicalRequest,
    source: ProtocolFormat,
    target: ProtocolFormat,
    model: Option<&str>,
) -> Result<(), ProtocolTransformRejection> {
    let Some(model) = model else {
        return Ok(());
    };
    if let Some(control) = source_request.model_thinking.as_deref() {
        validate_name_support(control, source, target, model)?;
    }
    let Some(profile) = profile(model)? else {
        return Ok(());
    };
    let Some(thinking) = projected.thinking.as_mut() else {
        return Ok(());
    };
    let Some(support) = profile.get("thinking") else {
        if source_request.model_thinking.is_some() {
            return Err(invalid());
        }
        projected.thinking = None;
        return Ok(());
    };
    if target == ProtocolFormat::AnthropicMessages {
        normalize_messages(source_request, projected, source, profile)
    } else {
        normalize_openai(source_request, thinking, source, profile, support)
    }
}

fn normalize_openai(
    source_request: &CanonicalRequest,
    thinking: &mut Thinking,
    source: ProtocolFormat,
    profile: &Value,
    support: &Value,
) -> Result<(), ProtocolTransformRejection> {
    let levels = support.get("levels").and_then(Value::as_array);
    let zero = support["zero_allowed"].as_bool().unwrap_or(false);
    let dynamic = support["dynamic_allowed"].as_bool().unwrap_or(false);
    let minimum = support["min"].as_u64().unwrap_or(0);
    let maximum = support["max"].as_u64().unwrap_or(0);
    let mut effort = thinking.effort.as_str().to_owned();
    if effort == "none" && !zero {
        if let Some(levels) = levels {
            supported_level("minimal", levels)?.clone_into(&mut effort);
        }
    } else if effort == "auto" && !dynamic {
        effort = if maximum > 0 && levels.is_none() {
            super::compatibility::budget_level(
                i64::try_from(minimum.midpoint(maximum)).map_err(|_| invalid())?,
            )
            .to_owned()
        } else {
            "medium".into()
        };
    }
    if levels.is_none() && maximum > 0 && !matches!(effort.as_str(), "none" | "auto") {
        let source_budget = match source_request.model_thinking.as_deref() {
            Some(gateway_core::ModelThinking::Budget(budget)) => Some(*budget),
            _ => source_request
                .thinking
                .as_ref()
                .and_then(|t| t.extensions.get(ANTHROPIC_THINKING_BUDGET))
                .and_then(|raw| serde_json::from_str::<i64>(raw.get()).ok()),
        }
        .filter(|value| *value > 0);
        if let Some(budget) = source_budget {
            let budget = u64::try_from(budget)
                .map_err(|_| invalid())?
                .clamp(minimum, maximum);
            effort =
                super::compatibility::budget_level(i64::try_from(budget).map_err(|_| invalid())?)
                    .into();
        }
    }
    if let Some(levels) = levels
        && !matches!(effort.as_str(), "none" | "auto")
    {
        if source != ProtocolFormat::AnthropicMessages
            && profile["type"] == "openai"
            && !matches!(
                source_request.model_thinking.as_deref(),
                Some(gateway_core::ModelThinking::Budget(_))
            )
            && !levels.iter().any(|v| v.as_str() == Some(effort.as_str()))
        {
            return Err(invalid());
        }
        supported_level(&effort, levels)?.clone_into(&mut effort);
    }
    thinking.effort = ThinkingEffort::try_new(effort).map_err(|_| invalid())?;
    Ok(())
}

fn normalize_messages(
    source_request: &CanonicalRequest,
    projected: &mut CanonicalRequest,
    source: ProtocolFormat,
    profile: &Value,
) -> Result<(), ProtocolTransformRejection> {
    let support = profile.get("thinking").ok_or_else(invalid)?;
    let thinking = projected.thinking.as_ref().ok_or_else(invalid)?;
    let levels = support.get("levels").and_then(Value::as_array);
    let zero = support["zero_allowed"].as_bool().unwrap_or(false);
    let dynamic = support["dynamic_allowed"].as_bool().unwrap_or(false);
    let minimum = support["min"].as_u64().unwrap_or(0);
    let maximum = support["max"].as_u64().unwrap_or(0);
    let source_effort = match source_request.model_thinking.as_deref() {
        Some(gateway_core::ModelThinking::Budget(budget)) => {
            Some(super::compatibility::budget_level(*budget))
        }
        Some(gateway_core::ModelThinking::Effort(effort)) => Some(effort.as_str()),
        None => source_request.thinking.as_ref().map(|t| t.effort.as_str()),
    };
    let is_level = !matches!(
        source_request.model_thinking.as_deref(),
        Some(gateway_core::ModelThinking::Budget(_))
    ) && (source != ProtocolFormat::AnthropicMessages
        || matches!(
            source_request.model_thinking.as_deref(),
            Some(gateway_core::ModelThinking::Effort(_))
        ))
        && source_effort.is_some_and(|e| !matches!(e, "none" | "auto"));
    let effort = thinking.effort.as_str();
    let mut extensions = thinking.extensions.clone();
    let mode;
    update_output_effort(&mut projected.extensions, None)?;
    if let Some(levels) = levels.filter(|_| is_level || maximum == 0) {
        let effort = match source_effort {
            Some("none") if zero => None,
            Some("auto") if dynamic => None,
            Some("none") => Some(supported_level("minimal", levels)?),
            Some("auto") => Some(supported_level("medium", levels)?),
            Some(e) => Some(supported_level(e, levels)?),
            None => None,
        };
        extensions = RawExtensions::default();
        if let Some(display) = thinking.extensions.get("anthropic.thinking.display") {
            extensions
                .try_insert("anthropic.thinking.display", display.clone())
                .map_err(|_| invalid())?;
        }
        if source_effort == Some("none") && zero {
            mode = support["disable_type"].as_str().unwrap_or("disabled");
        } else {
            mode = "adaptive";
            if let Some(effort) = effort {
                update_output_effort(&mut projected.extensions, Some(effort))?;
            }
        }
    } else if is_level && maximum > 0 {
        mode = "enabled";
        let budget = super::compatibility::level_budget(source_effort.ok_or_else(invalid)?)
            .ok_or_else(invalid)?
            .clamp(minimum, maximum);
        let budget = super::compatibility::messages_budget(source_request, source, budget)?;
        put(
            &mut extensions,
            ANTHROPIC_THINKING_BUDGET,
            &Value::from(budget),
        )?;
    } else if effort == "adaptive" && !dynamic && maximum > 0 {
        mode = "enabled";
        let budget = super::compatibility::messages_budget(
            source_request,
            source,
            minimum.midpoint(maximum),
        )?;
        put(
            &mut extensions,
            ANTHROPIC_THINKING_BUDGET,
            &Value::from(budget),
        )?;
    } else if effort == "enabled" {
        mode = "enabled";
        let budget = extensions
            .get(ANTHROPIC_THINKING_BUDGET)
            .ok_or_else(invalid)?;
        let budget: u64 = serde_json::from_str(budget.get()).map_err(|_| invalid())?;
        let budget = if maximum > 0 {
            budget.clamp(minimum, maximum)
        } else {
            budget
        };
        let budget = super::compatibility::messages_budget(source_request, source, budget)?;
        put(
            &mut extensions,
            ANTHROPIC_THINKING_BUDGET,
            &Value::from(budget),
        )?;
    } else {
        mode = effort;
    }
    projected.thinking = Some(Thinking {
        effort: ThinkingEffort::try_new(mode).map_err(|_| invalid())?,
        extensions,
    });
    Ok(())
}

fn update_output_effort(
    fields: &mut RawExtensions,
    effort: Option<&str>,
) -> Result<(), ProtocolTransformRejection> {
    const NAME: &str = "anthropic.messages.output_config";
    let Some(raw) = fields.get(NAME).cloned() else {
        return if let Some(effort) = effort {
            put(fields, NAME, &serde_json::json!({"effort":effort}))
        } else {
            Ok(())
        };
    };
    let mut value: serde_json::Map<String, Value> =
        serde_json::from_str(raw.get()).map_err(|_| invalid())?;
    value.remove("effort");
    if let Some(effort) = effort {
        value.insert("effort".into(), Value::from(effort));
    }
    put(fields, NAME, &Value::Object(value))
}

pub(super) fn is_level_only(model: Option<&str>) -> Result<bool, ProtocolTransformRejection> {
    Ok(match model {
        Some(model) => profile(model)?.is_some_and(|p| {
            p["thinking"]["levels"].is_array() && p["thinking"]["max"].as_u64().unwrap_or(0) == 0
        }),
        None => false,
    })
}

pub(super) fn has_levels(model: Option<&str>) -> Result<bool, ProtocolTransformRejection> {
    Ok(match model {
        Some(model) => profile(model)?.is_some_and(|p| p["thinking"]["levels"].is_array()),
        None => false,
    })
}

pub(super) fn validate_native(
    request: &CanonicalRequest,
    protocol: ProtocolFormat,
    model: Option<&str>,
) -> Result<(), ProtocolTransformRejection> {
    let Some(model) = model else {
        return Ok(());
    };
    let Some(profile) = profile(model)? else {
        return Ok(());
    };
    let Some(thinking) = &request.thinking else {
        return Ok(());
    };
    let Some(support) = profile.get("thinking") else {
        return Err(invalid());
    };
    if protocol == ProtocolFormat::AnthropicMessages {
        if let Some(raw) = thinking.extensions.get(ANTHROPIC_THINKING_BUDGET) {
            let budget: u64 = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
            let minimum = support["min"].as_u64().unwrap_or(0);
            let maximum = support["max"].as_u64().unwrap_or(0);
            if maximum == 0 || budget < minimum || budget > maximum {
                return Err(invalid());
            }
        }
    } else if let Some(levels) = support["levels"].as_array() {
        let effort = thinking.effort.as_str();
        if !matches!(effort, "none" | "auto") && !levels.iter().any(|v| v.as_str() == Some(effort))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(super) fn validate_name_support(
    control: &gateway_core::ModelThinking,
    source: ProtocolFormat,
    target: ProtocolFormat,
    model: &str,
) -> Result<(), ProtocolTransformRejection> {
    let Some(profile) = profile(model)? else {
        return Ok(());
    };
    let support = profile.get("thinking").ok_or_else(invalid)?;
    let same_family = match (source, target) {
        (ProtocolFormat::AnthropicMessages, ProtocolFormat::AnthropicMessages) => {
            profile["type"] == "claude"
        }
        (
            ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses,
            ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses,
        ) => profile["type"] == "openai",
        _ => false,
    };
    if same_family
        && let (gateway_core::ModelThinking::Effort(effort), Some(levels)) =
            (control, support["levels"].as_array())
        && !matches!(effort.as_str(), "none" | "auto")
        && !levels.iter().any(|v| v.as_str() == Some(effort.as_str()))
    {
        return Err(invalid());
    }
    Ok(())
}
