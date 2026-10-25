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
        for budget in [0, -1] {
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
