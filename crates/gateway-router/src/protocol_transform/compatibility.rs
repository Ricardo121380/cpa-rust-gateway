//! CPA-style request projection for `CanonicalBridge` (ADR-0125).
//! Reference: `CLIProxyAPI` v7.2.158, 5b2785617d1e7de84a9f4dee599d275a4ccd8999,
//! internal/thinking/{convert,summary}.go and internal/translator/{openai,codex}/claude.
//! Messages effort extraction: v8.0.40, d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683,
//! internal/thinking/apply.go: extractClaudeConfig (ADR-0126).
//! The upstream MIT notice is retained in `THIRD_PARTY_NOTICES.md`.

use super::{
    ANTHROPIC_THINKING_BUDGET, CHAT_LEGACY_OUTPUT_LIMIT, CHAT_OUTPUT_LIMIT, CanonicalRequest,
    MESSAGES_OUTPUT_LIMIT, MessageContent, ProtocolFormat, ProtocolTransformRejection,
    RESPONSES_OUTPUT_LIMIT, RawExtensions, RawJson, Thinking, ThinkingEffort, Value,
    is_output_limit_for, output_limit_name, project_messages_roles, project_root_extensions,
    reject_nested_extensions_and_opaque, tool_choice_name, tool_controls,
};

pub(super) const fn root_prefix(protocol: ProtocolFormat) -> &'static str {
    match protocol {
        ProtocolFormat::AnthropicMessages => "anthropic.messages.",
        ProtocolFormat::OpenAiResponses => "openai.responses.",
        ProtocolFormat::OpenAiChatCompletions => "openai.chat.",
    }
}

fn insert(
    fields: &mut RawExtensions,
    name: &str,
    value: &Value,
) -> Result<(), ProtocolTransformRejection> {
    fields
        .try_insert(
            name,
            RawJson::from_json_string(value.to_string())
                .map_err(|_| ProtocolTransformRejection::UnknownRequestExtensions)?,
        )
        .map_err(|_| ProtocolTransformRejection::UnknownRequestExtensions)
}

pub(super) fn project(
    request: &CanonicalRequest,
    source: ProtocolFormat,
    target: ProtocolFormat,
    target_model: Option<&str>,
    target_supports_service_tier: bool,
) -> Result<CanonicalRequest, ProtocolTransformRejection> {
    if source != ProtocolFormat::AnthropicMessages
        && (request.prompt_cache_key.is_some() || request.prompt_cache_retention.is_some())
    {
        return Err(ProtocolTransformRejection::CacheControlUnsupported);
    }
    let mut filtered = request.clone();
    filtered.extensions = RawExtensions::default();
    let mut mapped = RawExtensions::default();
    for (name, raw) in request.extensions.iter() {
        if matches!(
            name,
            CHAT_OUTPUT_LIMIT
                | CHAT_LEGACY_OUTPUT_LIMIT
                | RESPONSES_OUTPUT_LIMIT
                | MESSAGES_OUTPUT_LIMIT
        ) && !is_output_limit_for(name, source)
        {
            return Err(ProtocolTransformRejection::OutputLimitCollision);
        }
        let field = name
            .strip_prefix(root_prefix(source))
            .ok_or(ProtocolTransformRejection::UnknownRequestExtensions)?;
        if is_output_limit_for(name, source)
            || name == tool_choice_name(source)
            || name == tool_controls::parallel_name(source)
            || name == crate::MESSAGES_CONTEXT_MANAGEMENT
        {
            filtered
                .extensions
                .try_insert(name, raw.clone())
                .map_err(|_| ProtocolTransformRejection::UnknownRequestExtensions)?;
        } else {
            if matches!(
                field,
                "max_tokens" | "max_completion_tokens" | "max_output_tokens"
            ) {
                return Err(ProtocolTransformRejection::OutputLimitCollision);
            }
            // Local metadata, native tools and explicit output-shape requests have separate
            // owners. They must not accidentally become an ignored vendor hint here.
            if matches!(
                field,
                "metadata" | "client_metadata" | "tools" | "include" | "response_format" | "text"
            ) {
                return Err(ProtocolTransformRejection::UnknownRequestExtensions);
            }
            let target_field = sampling_field(source, target, field);
            if let Some(field) = target_field {
                if source == ProtocolFormat::AnthropicMessages
                    && field == "top_p"
                    && request
                        .extensions
                        .get("anthropic.messages.temperature")
                        .is_some()
                {
                    continue;
                }
                let mut value: Value = serde_json::from_str(raw.get())
                    .map_err(|_| ProtocolTransformRejection::UnknownRequestExtensions)?;
                if field == "stop_sequences" && value.is_string() {
                    value = Value::Array(vec![value]);
                }
                insert(
                    &mut mapped,
                    &format!("{}{field}", root_prefix(target)),
                    &value,
                )?;
            }
            // Like CPA's fresh target templates, other source-specific root options are
            // accepted without manufacturing a target equivalent (including cache hints).
        }
    }
    let mut projected = filtered.clone();
    projected.extensions = project_root_extensions(&filtered, source, target)?;
    for (name, raw) in mapped.iter() {
        projected
            .extensions
            .try_insert(name, raw.clone())
            .map_err(|_| ProtocolTransformRejection::UnknownRequestExtensions)?;
    }
    project_service_tier(
        request,
        &mut projected,
        source,
        target,
        target_supports_service_tier,
    )?;
    projected.thinking = thinking(request, source, target, target_model)?;
    project_summary(request, &mut projected, source, target)?;
    projected.prompt_cache_key = None;
    projected.prompt_cache_retention = None;
    for message in &mut projected.messages {
        // Message/body identities and reasoning history need real representations; ordinary
        // content cache hints can be omitted when the target has no block-cache concept.
        for part in &mut message.content {
            let fields = match part {
                MessageContent::Text(text) => &mut text.extensions,
                MessageContent::ToolCall(call) => &mut call.extensions,
                MessageContent::ToolResult(result) => &mut result.extensions,
                MessageContent::Opaque(_) | MessageContent::Reasoning(_) => continue,
            };
            *fields = without_cache(fields, source)?;
        }
    }
    for tool in &mut projected.tools {
        tool.extensions = without_cache(&tool.extensions, source)?;
    }
    reject_nested_extensions_and_opaque(&projected)?;
    if target == ProtocolFormat::AnthropicMessages {
        project_messages_roles(&mut projected)?;
    }
    Ok(projected)
}

fn without_cache(
    fields: &RawExtensions,
    source: ProtocolFormat,
) -> Result<RawExtensions, ProtocolTransformRejection> {
    let mut result = RawExtensions::default();
    for (name, raw) in fields.iter() {
        if source == ProtocolFormat::AnthropicMessages && name == "anthropic.cache_control" {
            continue;
        }
        result
            .try_insert(name, raw.clone())
            .map_err(|_| ProtocolTransformRejection::UnknownContentExtensions)?;
    }
    Ok(result)
}

pub(super) fn thinking(
    request: &CanonicalRequest,
    source: ProtocolFormat,
    target: ProtocolFormat,
    target_model: Option<&str>,
) -> Result<Option<Thinking>, ProtocolTransformRejection> {
    let invalid = || ProtocolTransformRejection::ThinkingUnsupported;
    let output_effort = request
        .extensions
        .get("anthropic.messages.output_config")
        .map(|raw| {
            let fields: RawExtensions = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
            if fields.iter().any(|(name, _)| name != "effort") {
                return Err(invalid());
            }
            fields
                .get("effort")
                .map(|raw| serde_json::from_str::<String>(raw.get()).map_err(|_| invalid()))
                .transpose()
        })
        .transpose()?
        .flatten();
    let output_effort = output_effort
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    if output_effort.as_deref().is_some_and(|value| {
        !matches!(
            value,
            "none" | "auto" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
        )
    }) {
        return Err(invalid());
    }
    let mut extensions = RawExtensions::default();
    let effort = if source == ProtocolFormat::AnthropicMessages {
        if let Some(control) = &request.model_thinking {
            match control {
                gateway_core::ModelThinking::Budget(budget) => budget_level(*budget).to_owned(),
                gateway_core::ModelThinking::Effort(effort) => effort.as_str().to_owned(),
            }
        } else {
            match request.thinking.as_ref().map(|t| t.effort.as_str()) {
                Some("disabled") => "none".to_owned(),
                Some("adaptive" | "auto") => output_effort.unwrap_or_else(|| "xhigh".into()),
                Some("enabled") => match request
                    .thinking
                    .as_ref()
                    .and_then(|t| t.extensions.get(ANTHROPIC_THINKING_BUDGET))
                {
                    Some(raw) => {
                        budget_level(serde_json::from_str(raw.get()).map_err(|_| invalid())?)
                            .to_owned()
                    }
                    None => output_effort.unwrap_or_else(|| "auto".into()),
                },
                None => match output_effort {
                    Some(effort) => effort,
                    None => return Ok(None),
                },
                _ => return Err(invalid()),
            }
        }
    } else {
        let Some(thinking) = request.thinking.as_ref() else {
            return Ok(None);
        };
        if target == ProtocolFormat::AnthropicMessages {
            match thinking.effort.as_str() {
                "none" => "disabled".into(),
                "auto" => "adaptive".into(),
                _ if matches!(request.model_thinking,Some(gateway_core::ModelThinking::Budget(value)) if value>0)
                    && !super::model_thinking::is_level_only(target_model)? =>
                {
                    let Some(gateway_core::ModelThinking::Budget(budget)) = &request.model_thinking
                    else {
                        return Err(invalid());
                    };
                    insert(
                        &mut extensions,
                        ANTHROPIC_THINKING_BUDGET,
                        &Value::from(messages_budget(
                            request,
                            source,
                            u64::try_from(*budget).map_err(|_| invalid())?,
                        )?),
                    )?;
                    "enabled".into()
                }
                _ if super::model_thinking::has_levels(target_model)? => "adaptive".into(),
                effort => {
                    let budget = level_budget(effort).ok_or_else(invalid)?;
                    insert(
                        &mut extensions,
                        ANTHROPIC_THINKING_BUDGET,
                        &Value::from(messages_budget(request, source, budget)?),
                    )?;
                    "enabled".into()
                }
            }
        } else {
            thinking.effort.as_str().to_owned()
        }
    };
    Ok(Some(Thinking {
        effort: ThinkingEffort::try_new(effort).map_err(|_| invalid())?,
        extensions,
    }))
}

pub(super) const fn budget_level(budget: i64) -> &'static str {
    match budget {
        -1 => "auto",
        0 => "none",
        1..=512 => "minimal",
        513..=1024 => "low",
        1025..=8192 => "medium",
        8193..=24576 => "high",
        _ => "xhigh",
    }
}

pub(super) fn level_budget(effort: &str) -> Option<u64> {
    match effort {
        "minimal" => Some(512),
        "low" => Some(1024),
        "medium" => Some(8192),
        "high" => Some(24576),
        "xhigh" => Some(32768),
        "max" => Some(128_000),
        _ => None,
    }
}

// CPA's Claude applier bounds manual thinking below max_tokens. With no per-model
// range in Canonical, use the Messages minimum and never increase the caller's limit.
pub(super) fn messages_budget(
    request: &CanonicalRequest,
    source: ProtocolFormat,
    budget: u64,
) -> Result<u64, ProtocolTransformRejection> {
    let raw = request
        .extensions
        .get(output_limit_name(source))
        .or_else(|| {
            if source == ProtocolFormat::OpenAiChatCompletions {
                request.extensions.get(CHAT_LEGACY_OUTPUT_LIMIT)
            } else {
                None
            }
        })
        .ok_or(ProtocolTransformRejection::OutputLimitMissing)?;
    let limit: u64 = serde_json::from_str(raw.get())
        .map_err(|_| ProtocolTransformRejection::OutputLimitInvalid)?;
    if limit <= 1024 {
        return Err(ProtocolTransformRejection::ThinkingUnsupported);
    }
    Ok(budget.max(1024).min(limit - 1))
}

fn project_summary(
    request: &CanonicalRequest,
    projected: &mut CanonicalRequest,
    source: ProtocolFormat,
    target: ProtocolFormat,
) -> Result<(), ProtocolTransformRejection> {
    let summary = match source {
        ProtocolFormat::AnthropicMessages => request
            .thinking
            .as_ref()
            .filter(|t| matches!(t.effort.as_str(), "adaptive" | "enabled"))
            .and_then(|t| t.extensions.get("anthropic.thinking.display"))
            .and_then(|raw| serde_json::from_str::<String>(raw.get()).ok())
            .and_then(|display| match display.as_str() {
                "summarized" => Some("auto".to_owned()),
                "omitted" => Some("none".to_owned()),
                _ => None,
            }),
        ProtocolFormat::OpenAiChatCompletions => request
            .thinking
            .as_ref()
            .filter(|t| t.effort.as_str() != "none")
            .map(|_| "auto".to_owned()),
        ProtocolFormat::OpenAiResponses => {
            let fields = request
                .extensions
                .get("openai.responses.reasoning")
                .map(|raw| serde_json::from_str::<RawExtensions>(raw.get()))
                .transpose()
                .map_err(|_| ProtocolTransformRejection::ThinkingUnsupported)?;
            request
                .thinking
                .as_ref()
                .and_then(|t| t.extensions.get("summary"))
                .or_else(|| fields.as_ref().and_then(|fields| fields.get("summary")))
                .and_then(|raw| serde_json::from_str::<String>(raw.get()).ok())
        }
    };
    let Some(summary) = summary else {
        return Ok(());
    };
    if projected
        .thinking
        .as_ref()
        .is_some_and(|t| matches!(t.effort.as_str(), "none" | "disabled"))
    {
        return Ok(());
    }
    match target {
        ProtocolFormat::OpenAiResponses if summary != "none" => {
            if let Some(thinking) = &mut projected.thinking {
                insert(&mut thinking.extensions, "summary", &Value::String(summary))?;
            } else {
                insert(
                    &mut projected.extensions,
                    "openai.responses.reasoning",
                    &serde_json::json!({"summary":summary}),
                )?;
            }
        }
        ProtocolFormat::AnthropicMessages => {
            if projected.thinking.is_none() && summary != "none" {
                let mut extensions = RawExtensions::default();
                insert(
                    &mut extensions,
                    ANTHROPIC_THINKING_BUDGET,
                    &Value::from(messages_budget(request, source, 1024)?),
                )?;
                projected.thinking = Some(Thinking {
                    effort: ThinkingEffort::try_new("enabled")
                        .map_err(|_| ProtocolTransformRejection::ThinkingUnsupported)?,
                    extensions,
                });
            }
            if let Some(thinking) = &mut projected.thinking {
                insert(
                    &mut thinking.extensions,
                    "anthropic.thinking.display",
                    &Value::String(
                        if summary == "none" {
                            "omitted"
                        } else {
                            "summarized"
                        }
                        .into(),
                    ),
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn sampling_field(source: ProtocolFormat, target: ProtocolFormat, field: &str) -> Option<&str> {
    match (source, target, field) {
        (_, ProtocolFormat::OpenAiResponses, "temperature" | "top_p")
            if source != ProtocolFormat::AnthropicMessages =>
        {
            Some(field)
        }
        (
            _,
            ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::AnthropicMessages,
            "temperature" | "top_p",
        ) => Some(field),
        (
            ProtocolFormat::AnthropicMessages,
            ProtocolFormat::OpenAiChatCompletions,
            "stop_sequences",
        ) => Some("stop"),
        (ProtocolFormat::OpenAiChatCompletions, ProtocolFormat::AnthropicMessages, "stop") => {
            Some("stop_sequences")
        }
        _ => None,
    }
}

// CLIProxyAPI d946c4f: codex/claude request tier normalizer and Responses→Claude speed.
fn project_service_tier(
    source_request: &CanonicalRequest,
    projected: &mut CanonicalRequest,
    source: ProtocolFormat,
    target: ProtocolFormat,
    target_supports_service_tier: bool,
) -> Result<(), ProtocolTransformRejection> {
    let read = |field: &str| -> Result<Option<Value>, ProtocolTransformRejection> {
        source_request
            .extensions
            .get(&format!("{}{field}", root_prefix(source)))
            .map(|raw| {
                serde_json::from_str(raw.get())
                    .map_err(|_| ProtocolTransformRejection::UnknownRequestExtensions)
            })
            .transpose()
    };
    let tier = read("service_tier")?;
    let speed = read("speed")?;
    if tier.as_ref().is_some_and(|v| !v.is_string())
        || speed.as_ref().is_some_and(|v| !v.is_string())
    {
        return Err(ProtocolTransformRejection::UnknownRequestExtensions);
    }
    if !target_supports_service_tier {
        return Ok(());
    }
    match (source, target) {
        (ProtocolFormat::AnthropicMessages, ProtocolFormat::OpenAiResponses) => {
            let tier = tier
                .as_ref()
                .and_then(Value::as_str)
                .map(|s| s.trim().to_ascii_lowercase());
            if speed.as_ref().and_then(Value::as_str) == Some("fast")
                || matches!(tier.as_deref(), Some("fast" | "priority"))
            {
                insert(
                    &mut projected.extensions,
                    "openai.responses.service_tier",
                    &Value::from("priority"),
                )?;
            }
        }
        (ProtocolFormat::OpenAiResponses, ProtocolFormat::AnthropicMessages)
            if tier.as_ref().and_then(Value::as_str) == Some("priority") =>
        {
            insert(
                &mut projected.extensions,
                "anthropic.messages.speed",
                &Value::from("fast"),
            )?;
        }
        (ProtocolFormat::OpenAiChatCompletions, ProtocolFormat::OpenAiResponses) => {
            let tier = tier
                .as_ref()
                .and_then(Value::as_str)
                .map(|s| s.trim().to_ascii_lowercase());
            if let Some(tier) = tier.as_deref().and_then(|s| match s {
                "fast" | "priority" => Some("priority"),
                "ultrafast" => Some("ultrafast"),
                _ => None,
            }) {
                insert(
                    &mut projected.extensions,
                    "openai.responses.service_tier",
                    &Value::from(tier),
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}
