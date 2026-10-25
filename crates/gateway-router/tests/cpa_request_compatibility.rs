//! CPA v7.2.158 compatibility plus v8.0.40 Messages effort extraction (ADR-0126).
#![deny(unsafe_code)]

use gateway_catalog::{CapabilitySet, SemanticCapability};
use gateway_core::CanonicalRequest;
use gateway_router::{
    NativePayloadAvailability, ProjectedProtocolRequest, ProtocolFormat, ProtocolTransformInput,
    SnapshotTransformMode, project_protocol_request,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn project(
    body: &Value,
    target: ProtocolFormat,
    mode: SnapshotTransformMode,
) -> Result<CanonicalRequest, Box<dyn std::error::Error>> {
    let decoded = protocol_anthropic::decode_request(&body.to_string())?;
    let capabilities =
        CapabilitySet::try_new([SemanticCapability::Reasoning, SemanticCapability::Tools])?;
    match project_protocol_request(ProtocolTransformInput {
        target_supports_service_tier: true,
        target_model: None,
        source: ProtocolFormat::AnthropicMessages,
        target,
        mode,
        native_payload: NativePayloadAvailability::Exact,
        request: &decoded.request,
        streaming: false,
        requires_json_schema: false,
        requires_parallel_tools: false,
        target_capabilities: &capabilities,
    })? {
        ProjectedProtocolRequest::Canonical(request) => Ok(request),
        ProjectedProtocolRequest::NativeExact => Ok(decoded.request),
    }
}

fn body() -> Value {
    json!({"model":"public","max_tokens":32000,"messages":[{"role":"user","content":"hello"}]})
}

#[test]
fn cpa_v8_messages_effort_extraction_and_precedence() -> TestResult {
    // CLIProxyAPI d946c4f, internal/thinking/apply.go: extractClaudeConfig.
    for target in [
        ProtocolFormat::OpenAiChatCompletions,
        ProtocolFormat::OpenAiResponses,
    ] {
        for (thinking, default) in [
            (None, None),
            (Some(json!({"type":"enabled"})), Some("auto")),
            (Some(json!({"type":"adaptive"})), Some("xhigh")),
            (Some(json!({"type":"auto"})), Some("xhigh")),
            (
                Some(json!({"type":"enabled","budget_tokens":1024})),
                Some("low"),
            ),
            (Some(json!({"type":"disabled"})), Some("none")),
        ] {
            for effort in [
                None,
                Some(""),
                Some("  "),
                Some(" HIGH "),
                Some("none"),
                Some("auto"),
                Some("minimal"),
                Some("max"),
            ] {
                let mut source = body();
                if let Some(thinking) = &thinking {
                    source["thinking"] = thinking.clone();
                }
                if let Some(effort) = effort {
                    source["output_config"] = json!({"effort":effort});
                }
                let explicit = effort
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_ascii_lowercase);
                let expected = if matches!(default, Some("low" | "none")) {
                    default
                } else {
                    explicit.as_deref().or(default)
                };
                let projected = project(&source, target, SnapshotTransformMode::CanonicalBridge)?;
                assert_eq!(
                    projected.thinking.as_ref().map(|v| v.effort.as_str()),
                    expected,
                    "target={target:?}, source={source}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn cpa_v8_effort_keeps_known_controls_and_capability_guards() -> TestResult {
    for target in [
        ProtocolFormat::OpenAiResponses,
        ProtocolFormat::OpenAiChatCompletions,
    ] {
        for output in [
            json!({"effort":false}),
            json!({"effort":"unknown"}),
            json!({"effort":"high","extra":true}),
            json!([]),
        ] {
            let mut source = body();
            source["output_config"] = output;
            assert!(project(&source, target, SnapshotTransformMode::CanonicalBridge).is_err());
        }
        for budget in [-2, -100] {
            let mut source = body();
            source["thinking"] = json!({"type":"enabled","budget_tokens":budget});
            source["output_config"] = json!({"effort":"high"});
            assert!(project(&source, target, SnapshotTransformMode::CanonicalBridge).is_err());
        }
        let mut source = body();
        source["output_config"] = json!({"effort":"high"});
        assert!(project(&source, target, SnapshotTransformMode::LosslessBridge).is_err());
        let request = protocol_anthropic::decode_request(&source.to_string())?.request;
        let capabilities = CapabilitySet::empty();
        assert!(
            project_protocol_request(ProtocolTransformInput {
                target_supports_service_tier: true,
                target_model: None,
                source: ProtocolFormat::AnthropicMessages,
                target,
                mode: SnapshotTransformMode::CanonicalBridge,
                native_payload: NativePayloadAvailability::Exact,
                request: &request,
                streaming: false,
                requires_json_schema: false,
                requires_parallel_tools: false,
                target_capabilities: &capabilities,
            })
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn cpa_openai_summary_and_budget_remain_valid_for_messages() -> TestResult {
    let capabilities = CapabilitySet::try_new([SemanticCapability::Reasoning])?;
    for (reasoning, expected_budget, display) in [
        (
            json!({"effort":"high","summary":"detailed"}),
            Some(2047),
            Some("summarized"),
        ),
        (json!({"effort":"minimal"}), Some(1024), None),
        (json!({"summary":"auto"}), Some(1024), Some("summarized")),
        (json!({"effort":"none","summary":"auto"}), None, None),
    ] {
        let decoded = protocol_openai_responses::decode_request(&json!({"model":"public","input":"hello", "max_output_tokens":2048,"reasoning":reasoning}).to_string())?;
        let ProjectedProtocolRequest::Canonical(request) =
            project_protocol_request(ProtocolTransformInput {
                target_supports_service_tier: true,
                target_model: None,
                source: ProtocolFormat::OpenAiResponses,
                target: ProtocolFormat::AnthropicMessages,
                mode: SnapshotTransformMode::CanonicalBridge,
                native_payload: NativePayloadAvailability::Exact,
                request: &decoded.request,
                streaming: false,
                requires_json_schema: false,
                requires_parallel_tools: false,
                target_capabilities: &capabilities,
            })?
        else {
            return Err("expected canonical".into());
        };
        let wire: Value = serde_json::from_str(&protocol_anthropic::encode_upstream_request(
            "selected",
            &request,
            protocol_anthropic::ResponseMode::NonStreaming,
        )?)?;
        assert_eq!(wire["max_tokens"], 2048);
        assert_eq!(wire["thinking"]["budget_tokens"].as_u64(), expected_budget);
        assert_eq!(wire["thinking"]["display"].as_str(), display);
        if let Some(budget) = expected_budget {
            assert!(budget < 2048);
        }
    }
    Ok(())
}

#[test]
fn cpa_native_forwarding_still_rejects_invalid_known_history() {
    for role in ["user", "assistant"] {
        let mut source = body();
        source["messages"] = json!([{"role":role,"content":[{"type":"thinking","thinking":"private","signature":""}]}]);
        assert!(
            project(
                &source,
                ProtocolFormat::AnthropicMessages,
                SnapshotTransformMode::Passthrough
            )
            .is_err()
        );
    }
}

#[test]
fn cpa_sampling_and_stop_options_use_valid_target_shapes() -> TestResult {
    let mut source = body();
    source["temperature"] = json!(0.5);
    source["top_p"] = json!(0.9);
    source["stop_sequences"] = json!(["END"]);
    let chat = project(
        &source,
        ProtocolFormat::OpenAiChatCompletions,
        SnapshotTransformMode::CanonicalBridge,
    )?;
    assert_eq!(
        chat.extensions
            .get("openai.chat.temperature")
            .map(gateway_core::RawJson::get),
        Some("0.5")
    );
    assert!(chat.extensions.get("openai.chat.top_p").is_none());
    assert_eq!(
        chat.extensions
            .get("openai.chat.stop")
            .map(gateway_core::RawJson::get),
        Some("[\"END\"]")
    );
    let capabilities = CapabilitySet::empty();
    for stop in [json!("END"), json!(["END"])] {
        let decoded = protocol_openai_chat::decode_request(&json!({"model":"public","messages":[{"role":"user","content":"hello"}],"max_completion_tokens":2048,"stop":stop}).to_string())?;
        let ProjectedProtocolRequest::Canonical(request) =
            project_protocol_request(ProtocolTransformInput {
                target_supports_service_tier: true,
                target_model: None,
                source: ProtocolFormat::OpenAiChatCompletions,
                target: ProtocolFormat::AnthropicMessages,
                mode: SnapshotTransformMode::CanonicalBridge,
                native_payload: NativePayloadAvailability::Exact,
                request: &decoded.request,
                streaming: false,
                requires_json_schema: false,
                requires_parallel_tools: false,
                target_capabilities: &capabilities,
            })?
        else {
            return Err("expected canonical".into());
        };
        let wire: Value = serde_json::from_str(&protocol_anthropic::encode_upstream_request(
            "selected",
            &request,
            protocol_anthropic::ResponseMode::NonStreaming,
        )?)?;
        assert_eq!(wire["stop_sequences"], json!(["END"]));
    }
    Ok(())
}

#[test]
fn cpa_adaptive_effort_and_cache_hints_are_accepted_together() -> TestResult {
    let mut source = body();
    source["thinking"] = json!({"type":"adaptive","display":"summarized"});
    source["output_config"] = json!({"effort":" HIGH "});
    source["vendor_option"] = json!({"future":true});
    source["messages"][0]["content"] =
        json!([{"type":"text","text":"hello","cache_control":{"type":"ephemeral"}}]);
    let projected = project(
        &source,
        ProtocolFormat::OpenAiResponses,
        SnapshotTransformMode::CanonicalBridge,
    )?;
    let thinking = projected.thinking.ok_or("missing reasoning")?;
    assert_eq!(thinking.effort.as_str(), "high");
    assert_eq!(
        thinking
            .extensions
            .get("summary")
            .map(gateway_core::RawJson::get),
        Some("\"auto\"")
    );
    assert!(projected.prompt_cache_retention.is_none());
    assert!(
        projected
            .extensions
            .get("openai.responses.vendor_option")
            .is_none()
    );
    assert!(
        project(
            &source,
            ProtocolFormat::OpenAiResponses,
            SnapshotTransformMode::LosslessBridge
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn cpa_budget_thresholds_match_the_go_reference() -> TestResult {
    for target in [
        ProtocolFormat::OpenAiResponses,
        ProtocolFormat::OpenAiChatCompletions,
    ] {
        for (budget, effort) in [
            (1, "minimal"),
            (512, "minimal"),
            (513, "low"),
            (1024, "low"),
            (1025, "medium"),
            (8192, "medium"),
            (8193, "high"),
            (24576, "high"),
            (24577, "xhigh"),
        ] {
            let mut source = body();
            source["thinking"] = json!({"type":"enabled","budget_tokens":budget});
            let projected = project(&source, target, SnapshotTransformMode::CanonicalBridge)?;
            assert_eq!(
                projected
                    .thinking
                    .ok_or("missing thinking")?
                    .effort
                    .as_str(),
                effort
            );
        }
    }
    Ok(())
}

#[test]
fn cpa_native_passthrough_preserves_unfamiliar_fields() -> TestResult {
    let mut source = body();
    source["vendor_option"] = json!({"future":true});
    source["thinking"] = json!({"type":"adaptive","display":"updates"});
    source["output_config"] = json!({"effort":"high"});
    source["messages"][0]["content"] = json!([{"type":"text","text":"hello","vendor_part":17}]);
    let projected = project(
        &source,
        ProtocolFormat::AnthropicMessages,
        SnapshotTransformMode::Passthrough,
    )?;
    assert!(
        projected
            .extensions
            .get("anthropic.messages.vendor_option")
            .is_some()
    );
    Ok(())
}

#[test]
fn cpa_mixed_cache_lifetimes_round_trip_natively() -> TestResult {
    let mut source = body();
    source["system"] =
        json!([{"type":"text","text":"system","cache_control":{"type":"ephemeral","ttl":"1h"}}]);
    source["messages"][0]["content"] =
        json!([{"type":"text","text":"hello","cache_control":{"type":"ephemeral"}}]);
    let request = protocol_anthropic::decode_request(&source.to_string())?.request;
    let encoded: Value = serde_json::from_str(&protocol_anthropic::encode_upstream_request(
        "selected",
        &request,
        protocol_anthropic::ResponseMode::NonStreaming,
    )?)?;
    assert_eq!(encoded["system"], source["system"]);
    assert_eq!(encoded["messages"], source["messages"]);
    project(
        &source,
        ProtocolFormat::OpenAiResponses,
        SnapshotTransformMode::CanonicalBridge,
    )?;
    Ok(())
}

#[test]
fn cpa_69_special_budgets_and_frozen_model_levels() -> TestResult {
    let capabilities = CapabilitySet::try_new([SemanticCapability::Reasoning])?;
    for (model, budget, expected) in [
        ("custom-model", 0, "none"),
        ("custom-model", -1, "auto"),
        ("gpt-5.5", 0, "low"),
        ("gpt-5.5", -1, "medium"),
        ("gpt-5.5", 128000, "xhigh"),
        ("gpt-6.1-sol", 512, "low"),
    ] {
        for target in [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiResponses,
        ] {
            let mut body = body();
            body["thinking"] = json!({"type":"enabled","budget_tokens":budget});
            body["output_config"] = json!({"effort":"high"});
            let decoded = protocol_anthropic::decode_request(&body.to_string())?;
            let ProjectedProtocolRequest::Canonical(request) =
                project_protocol_request(ProtocolTransformInput {
                    target_supports_service_tier: true,
                    target_model: Some(model),
                    source: ProtocolFormat::AnthropicMessages,
                    target,
                    mode: SnapshotTransformMode::CanonicalBridge,
                    native_payload: NativePayloadAvailability::Exact,
                    request: &decoded.request,
                    streaming: false,
                    requires_json_schema: false,
                    requires_parallel_tools: false,
                    target_capabilities: &capabilities,
                })?
            else {
                return Err("canonical".into());
            };
            assert_eq!(
                request.thinking.as_ref().map(|t| t.effort.as_str()),
                Some(expected)
            );
        }
    }
    Ok(())
}

#[test]
fn cpa_69_claude_model_ranges_and_adaptive_targets() -> TestResult {
    let capabilities = CapabilitySet::try_new([SemanticCapability::Reasoning])?;
    for (model, effort, limit, expected_type, output_effort, budget) in [
        (
            "claude-sonnet-4-6",
            "max",
            512,
            "adaptive",
            Some("max"),
            None,
        ),
        (
            "claude-opus-5",
            "minimal",
            512,
            "adaptive",
            Some("low"),
            None,
        ),
        ("claude-fable-5", "none", 512, "adaptive", Some("low"), None),
        (
            "claude-sonnet-5-5",
            "none",
            512,
            "between_tools",
            None,
            None,
        ),
        ("claude-opus-5", "auto", 512, "adaptive", None, None),
        (
            "claude-sonnet-4-5-20250929",
            "high",
            4096,
            "enabled",
            None,
            Some(4095),
        ),
        ("custom-model", "high", 4096, "enabled", None, Some(4095)),
    ] {
        let decoded = protocol_openai_responses::decode_request(&json!({"model":"public","input":"hello", "max_output_tokens":limit,"reasoning":{"effort":effort}}).to_string())?;
        let ProjectedProtocolRequest::Canonical(request) =
            project_protocol_request(ProtocolTransformInput {
                target_supports_service_tier: true,
                target_model: Some(model),
                source: ProtocolFormat::OpenAiResponses,
                target: ProtocolFormat::AnthropicMessages,
                mode: SnapshotTransformMode::CanonicalBridge,
                native_payload: NativePayloadAvailability::Exact,
                request: &decoded.request,
                streaming: false,
                requires_json_schema: false,
                requires_parallel_tools: false,
                target_capabilities: &capabilities,
            })
            .map_err(|e| format!("{model}/{effort}/{limit}: {e:?}"))?
        else {
            return Err("canonical".into());
        };
        let wire: Value = serde_json::from_str(&protocol_anthropic::encode_upstream_request(
            model,
            &request,
            protocol_anthropic::ResponseMode::NonStreaming,
        )?)?;
        assert_eq!(wire["thinking"]["type"], expected_type, "{model} {effort}");
        assert_eq!(
            wire["output_config"]["effort"].as_str(),
            output_effort,
            "{model} {effort}"
        );
        assert_eq!(wire["thinking"]["budget_tokens"].as_u64(), budget);
    }
    Ok(())
}

#[test]
fn cpa_69_service_tier_mapping_and_malformed_controls() -> TestResult {
    for (speed, tier, expected) in [
        (Some("fast"), None, Some("priority")),
        (Some("FAST"), None, None),
        (None, Some(" PRIORITY "), Some("priority")),
        (Some("fast"), Some("ultrafast"), Some("priority")),
        (None, Some("ultrafast"), None),
    ] {
        let mut body = body();
        if let Some(speed) = speed {
            body["speed"] = json!(speed);
        }
        if let Some(tier) = tier {
            body["service_tier"] = json!(tier);
        }
        for target in [
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::OpenAiChatCompletions,
        ] {
            let request = project(&body, target, SnapshotTransformMode::CanonicalBridge)?;
            let field = if target == ProtocolFormat::OpenAiResponses {
                "openai.responses.service_tier"
            } else {
                "openai.chat.service_tier"
            };
            let value = request
                .extensions
                .get(field)
                .map(|raw| serde_json::from_str::<Value>(raw.get()))
                .transpose()?;
            assert_eq!(
                value.as_ref().and_then(Value::as_str),
                if target == ProtocolFormat::OpenAiResponses {
                    expected
                } else {
                    None
                }
            );
        }
    }
    Ok(())
}

#[test]
fn cpa_69_suffix_controls_keep_lossless_and_capability_guards() -> TestResult {
    let mut decoded = protocol_anthropic::decode_request(&body().to_string())?.request;
    decoded.model_thinking = Some(gateway_core::ModelThinking::Budget(-1));
    for (model, mode, capabilities) in [
        (
            "gpt-5.5",
            SnapshotTransformMode::LosslessBridge,
            CapabilitySet::try_new([SemanticCapability::Reasoning])?,
        ),
        (
            "gpt-5.5",
            SnapshotTransformMode::CanonicalBridge,
            CapabilitySet::empty(),
        ),
        (
            "claude-3-5-haiku-20241022",
            SnapshotTransformMode::CanonicalBridge,
            CapabilitySet::try_new([SemanticCapability::Reasoning])?,
        ),
    ] {
        assert!(
            project_protocol_request(ProtocolTransformInput {
                target_model: Some(model),
                target_supports_service_tier: true,
                source: ProtocolFormat::AnthropicMessages,
                target: ProtocolFormat::OpenAiResponses,
                mode,
                native_payload: NativePayloadAvailability::Exact,
                request: &decoded,
                streaming: false,
                requires_json_schema: false,
                requires_parallel_tools: false,
                target_capabilities: &capabilities
            })
            .is_err()
        );
    }
    Ok(())
}

fn named_project(
    request: &CanonicalRequest,
    source: ProtocolFormat,
    target: ProtocolFormat,
    model: &str,
    mode: SnapshotTransformMode,
) -> Result<gateway_router::ProjectedProtocolRequest, Box<dyn std::error::Error>> {
    let capabilities = CapabilitySet::try_new([SemanticCapability::Reasoning])?;
    Ok(project_protocol_request(ProtocolTransformInput {
        target_model: Some(model),
        target_supports_service_tier: true,
        source,
        target,
        mode,
        native_payload: NativePayloadAvailability::Exact,
        request,
        streaming: false,
        requires_json_schema: false,
        requires_parallel_tools: false,
        target_capabilities: &capabilities,
    })?)
}

#[test]
fn cpa_69_numeric_suffix_keeps_exact_manual_budget() -> TestResult {
    for model in ["claude-sonnet-4-5-20250929", "claude-sonnet-4-6"] {
        for source in [
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::OpenAiChatCompletions,
        ] {
            let mut request = if source == ProtocolFormat::OpenAiResponses {
                protocol_openai_responses::decode_request(
                    &json!({"model":"public","input":"question","max_output_tokens":32000})
                        .to_string(),
                )?
                .request
            } else {
                protocol_openai_chat::decode_request(&json!({"model":"public","messages":[{"role":"user","content":"question"}],"max_completion_tokens":32000}).to_string())?.request
            };
            request.model_thinking = Some(gateway_core::ModelThinking::Budget(4096));
            let ProjectedProtocolRequest::Canonical(projected) = named_project(
                &request,
                source,
                ProtocolFormat::AnthropicMessages,
                model,
                SnapshotTransformMode::CanonicalBridge,
            )?
            else {
                return Err("canonical".into());
            };
            let wire: Value = serde_json::from_str(&protocol_anthropic::encode_upstream_request(
                model,
                &projected,
                protocol_anthropic::ResponseMode::NonStreaming,
            )?)?;
            assert_eq!(wire["thinking"]["type"], "enabled", "{model} {source:?}");
            assert_eq!(
                wire["thinking"]["budget_tokens"], 4096,
                "{model} {source:?}"
            );
            assert!(wire["output_config"].get("effort").is_none());
        }
    }
    Ok(())
}

#[test]
fn cpa_69_suffix_never_hides_malformed_body_thinking_type() -> TestResult {
    let mut body = body();
    body["thinking"] = json!({"type":"bogus"});
    let mut request = protocol_anthropic::decode_request(&body.to_string())?.request;
    request.model_thinking = Some(gateway_core::ModelThinking::Budget(4096));
    for (target, model, mode) in [
        (
            ProtocolFormat::OpenAiResponses,
            "gpt-5.5",
            SnapshotTransformMode::CanonicalBridge,
        ),
        (
            ProtocolFormat::AnthropicMessages,
            "claude-sonnet-4-6",
            SnapshotTransformMode::Passthrough,
        ),
    ] {
        assert!(
            named_project(
                &request,
                ProtocolFormat::AnthropicMessages,
                target,
                model,
                mode
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn cpa_69_native_suffix_retains_other_output_config_members() -> TestResult {
    let mut body = body();
    body["thinking"] = json!({"type":"enabled","budget_tokens":2048});
    body["output_config"] = json!({"effort":"low","format":{"type":"json_schema","schema":{"type":"object"}},"future_option":17});
    let mut request = protocol_anthropic::decode_request(&body.to_string())?.request;
    request.model_thinking = Some(gateway_core::ModelThinking::Effort(
        gateway_core::ThinkingEffort::try_new("high")?,
    ));
    let wire: Value = serde_json::from_slice(&gateway_router::model_name_native_payload(
        body.to_string().as_bytes(),
        &request,
        ProtocolFormat::AnthropicMessages,
        "claude-sonnet-4-6",
    )?)?;
    assert_eq!(wire["output_config"]["effort"], "high");
    assert_eq!(
        wire["output_config"]["format"],
        body["output_config"]["format"]
    );
    assert_eq!(wire["output_config"]["future_option"], 17);
    Ok(())
}

#[test]
fn cpa_69_same_family_explicit_unsupported_effort_rejects() -> TestResult {
    for named in [false, true] {
        let mut request=protocol_openai_chat::decode_request(&json!({"model":"public","messages":[{"role":"user","content":"question"}],"reasoning_effort":"max"}).to_string())?.request;
        if named {
            request.model_thinking = Some(gateway_core::ModelThinking::Effort(
                gateway_core::ThinkingEffort::try_new("max")?,
            ));
        }
        assert!(
            named_project(
                &request,
                ProtocolFormat::OpenAiChatCompletions,
                ProtocolFormat::OpenAiResponses,
                "gpt-5.5",
                SnapshotTransformMode::CanonicalBridge
            )
            .is_err()
        );
    }
    let mut request = protocol_anthropic::decode_request(&body().to_string())?.request;
    request.model_thinking = Some(gateway_core::ModelThinking::Effort(
        gateway_core::ThinkingEffort::try_new("minimal")?,
    ));
    assert!(
        named_project(
            &request,
            ProtocolFormat::AnthropicMessages,
            ProtocolFormat::AnthropicMessages,
            "claude-sonnet-4-6",
            SnapshotTransformMode::Passthrough
        )
        .is_err()
    );
    Ok(())
}
