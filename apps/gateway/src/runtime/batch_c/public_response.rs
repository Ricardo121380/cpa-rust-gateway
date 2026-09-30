//! Decode the complete public wire response and compare it with the independent round oracle.
use std::{error::Error, fmt::Write as _};

use gateway_core::{
    CanonicalEvent, CanonicalResponse, MessageEnd, MessageRole, MessageStart, RawExtensions,
    RawJson, ResponseEnd, ResponseId, ResponseStart, TextDelta, ToolCallArgumentsDelta,
    ToolCallEnd, ToolCallStart,
};
use serde_json::{Map, Value, json};

pub(super) fn assert_round(
    uri: &str,
    streaming: bool,
    round: usize,
    bytes: &[u8],
) -> Result<(), Box<dyn Error>> {
    let events = decode_public(uri, streaming, bytes).map_err(|error| {
        format!(
            "{uri} streaming={streaming} round={round}: {error}; synthetic public body: {}",
            String::from_utf8_lossy(bytes)
        )
    })?;
    CanonicalResponse::try_new(events.clone())?;
    let ends = events
        .iter()
        .filter_map(|event| match event {
            CanonicalEvent::ResponseEnd(end) => Some(end),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(ends.len(), 1, "one successful public terminal required");
    let end = ends.first().ok_or("public terminal")?;
    let starts = events
        .iter()
        .filter_map(|event| match event {
            CanonicalEvent::ToolCallStart(start) => Some(start),
            _ => None,
        })
        .collect::<Vec<_>>();
    let tools = events
        .iter()
        .filter_map(|event| match event {
            CanonicalEvent::ToolCallEnd(tool) => Some(tool),
            _ => None,
        })
        .collect::<Vec<_>>();
    let text = events
        .iter()
        .filter_map(|event| match event {
            CanonicalEvent::TextDelta(delta) => Some(delta.text.as_str()),
            _ => None,
        })
        .collect::<String>();
    if round.is_multiple_of(2) {
        let expected_id = if round == 0 { "call-c" } else { "call-d" };
        assert_eq!(starts.len(), 1);
        assert_eq!(tools.len(), 1);
        let start = starts.first().ok_or("public tool start")?;
        let tool = tools.first().ok_or("public tool end")?;
        assert_eq!(start.call_id, expected_id);
        assert_eq!(start.name, "lookup");
        assert_eq!(tool.call_id, expected_id);
        assert_eq!(
            serde_json::from_str::<Value>(tool.arguments.get())?,
            json!({})
        );
        let arguments = events
            .iter()
            .filter_map(|event| match event {
                CanonicalEvent::ToolCallArgumentsDelta(delta) if delta.call_id == expected_id => {
                    Some(delta.delta.as_str())
                }
                _ => None,
            })
            .collect::<String>();
        assert_eq!(serde_json::from_str::<Value>(&arguments)?, json!({}));
        assert!(matches!(
            end.stop_reason.as_deref(),
            Some("tool_use" | "tool_calls")
        ));
        assert!(text.is_empty());
    } else {
        assert!(starts.is_empty() && tools.is_empty());
        assert_eq!(text, "answer");
        assert!(matches!(
            end.stop_reason.as_deref(),
            Some("end_turn" | "stop")
        ));
    }
    Ok(())
}

fn decode_public(
    uri: &str,
    streaming: bool,
    bytes: &[u8],
) -> Result<Vec<CanonicalEvent>, Box<dyn Error>> {
    let view = native_decode_view(streaming, bytes)?;
    if !streaming {
        let input = view.as_str();
        return Ok(match uri {
            "/v1/responses" => protocol_openai_responses::decode_upstream_response(input)?,
            "/v1/chat/completions" => protocol_openai_chat::decode_upstream_response(input)?,
            "/v1/messages" => decode_public_message(&serde_json::from_str(input)?)?,
            _ => return Err("public protocol".into()),
        });
    }
    validate_wire_terminal(uri, bytes)?;
    let bytes = view.as_bytes();
    let mut events = Vec::new();
    match uri {
        "/v1/responses" => {
            let mut decoder = protocol_openai_responses::OpenAiResponsesSseDecoder::new();
            for chunk in bytes.chunks(7) {
                events.extend(decoder.push(chunk)?);
            }
            events.extend(decoder.finish()?);
        }
        "/v1/chat/completions" => {
            let mut decoder = protocol_openai_chat::OpenAiChatSseDecoder::new();
            for chunk in bytes.chunks(7) {
                events.extend(decoder.push(chunk)?);
            }
            events.extend(decoder.finish()?);
        }
        "/v1/messages" => {
            events = decode_public_message(&reassemble_message_sse(bytes)?)?;
        }
        _ => return Err("public protocol".into()),
    }
    Ok(events)
}

// Public Messages may omit an unprovable exclusive input aggregate (ADR-0100). The upstream
// codec deliberately requires Provider counters, so validate public content/lifecycle here.
fn decode_public_message(value: &Value) -> Result<Vec<CanonicalEvent>, Box<dyn Error>> {
    assert_eq!(value["type"], "message");
    assert_eq!(value["role"], "assistant");
    let usage = value["usage"].as_object().ok_or("public Messages usage")?;
    for (name, count) in usage {
        assert!(matches!(
            name.as_str(),
            "input_tokens"
                | "output_tokens"
                | "cache_read_input_tokens"
                | "cache_creation_input_tokens"
        ));
        assert!(count.as_u64().is_some());
    }
    let mut events = vec![
        CanonicalEvent::ResponseStart(ResponseStart {
            response_id: ResponseId::try_new(value["id"].as_str().ok_or("message id")?)?,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".into()),
            extensions: RawExtensions::default(),
        }),
    ];
    for block in value["content"]
        .as_array()
        .ok_or("public message content")?
    {
        match block["type"].as_str() {
            Some("text") => events.push(CanonicalEvent::TextDelta(TextDelta {
                text: block["text"].as_str().ok_or("text")?.into(),
                extensions: RawExtensions::default(),
            })),
            Some("tool_use") => {
                let call_id = block["id"].as_str().ok_or("tool id")?;
                assert!(block["input"].is_object());
                let arguments = block["input"].to_string();
                events.extend([
                    CanonicalEvent::ToolCallStart(ToolCallStart {
                        call_id: call_id.into(),
                        name: block["name"].as_str().ok_or("tool name")?.into(),
                        extensions: RawExtensions::default(),
                    }),
                    CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
                        call_id: call_id.into(),
                        delta: arguments.clone(),
                        extensions: RawExtensions::default(),
                    }),
                    CanonicalEvent::ToolCallEnd(ToolCallEnd {
                        call_id: call_id.into(),
                        arguments: RawJson::from_json_string(arguments)?,
                        extensions: RawExtensions::default(),
                    }),
                ]);
            }
            _ => return Err("unexpected public content".into()),
        }
    }
    events.extend([
        CanonicalEvent::MessageEnd(MessageEnd::default()),
        CanonicalEvent::ResponseEnd(ResponseEnd {
            stop_reason: Some(value["stop_reason"].as_str().ok_or("stop reason")?.into()),
            ..ResponseEnd::default()
        }),
    ]);
    Ok(events)
}

fn reassemble_message_sse(bytes: &[u8]) -> Result<Value, Box<dyn Error>> {
    let mut message = None;
    let mut blocks: Vec<Value> = Vec::new();
    let mut active = None;
    let mut arguments = String::new();
    let mut ended = false;
    let mut stopped = false;
    for data in std::str::from_utf8(bytes)?
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
    {
        assert!(!stopped, "data after message_stop");
        let event: Value = serde_json::from_str(data)?;
        match event["type"].as_str() {
            Some("message_start") => {
                assert!(message.is_none());
                assert_eq!(event["message"]["content"], json!([]));
                message = Some(event["message"].clone());
            }
            Some("content_block_start") => {
                assert!(message.is_some() && active.is_none() && !ended);
                assert_eq!(event["index"].as_u64(), Some(u64::try_from(blocks.len())?));
                active = Some(blocks.len());
                blocks.push(event["content_block"].clone());
                arguments.clear();
            }
            Some("content_block_delta") => {
                let index = active.ok_or("active content block")?;
                assert_eq!(event["index"].as_u64(), Some(u64::try_from(index)?));
                let block = blocks.get_mut(index).ok_or("content block")?;
                match (block["type"].as_str(), event["delta"]["type"].as_str()) {
                    (Some("text"), Some("text_delta")) => {
                        block["text"] = json!(format!(
                            "{}{}",
                            block["text"].as_str().ok_or("text")?,
                            event["delta"]["text"].as_str().ok_or("text delta")?
                        ));
                    }
                    (Some("tool_use"), Some("input_json_delta")) => arguments.push_str(
                        event["delta"]["partial_json"]
                            .as_str()
                            .ok_or("JSON delta")?,
                    ),
                    _ => return Err("content delta kind".into()),
                }
            }
            Some("content_block_stop") => {
                let index = active.take().ok_or("active content block")?;
                assert_eq!(event["index"].as_u64(), Some(u64::try_from(index)?));
                if !arguments.is_empty() {
                    let block = blocks.get_mut(index).ok_or("content block")?;
                    assert_eq!(block["type"], "tool_use");
                    block["input"] = serde_json::from_str(&arguments)?;
                }
            }
            Some("message_delta") => {
                assert!(active.is_none() && !ended && !blocks.is_empty());
                let message = message.as_mut().ok_or("message start")?;
                message["stop_reason"] = event["delta"]["stop_reason"].clone();
                message["usage"] = event["usage"].clone();
                ended = true;
            }
            Some("message_stop") => {
                assert!(ended && active.is_none());
                stopped = true;
            }
            _ => return Err("public Messages event".into()),
        }
    }
    assert!(stopped, "truncated public Messages stream");
    let mut message = message.ok_or("message start")?;
    message["content"] = Value::Array(blocks);
    Ok(message)
}

// ADR-0100's public evidence is validated independently; upstream codecs only understand
// native Usage fields. This view does not alter the actual gateway response or invent counts.
fn native_decode_view(streaming: bool, bytes: &[u8]) -> Result<String, Box<dyn Error>> {
    if !streaming {
        let mut value: Value = serde_json::from_slice(bytes)?;
        validate_public_usage(&mut value)?;
        return Ok(value.to_string());
    }
    let mut view = String::new();
    for line in std::str::from_utf8(bytes)?.lines() {
        if let Some(data) = line.strip_prefix("data:")
            && data.trim() != "[DONE]"
        {
            let mut value: Value = serde_json::from_str(data)?;
            validate_public_usage(&mut value)?;
            writeln!(view, "data: {value}")?;
        } else {
            writeln!(view, "{line}")?;
        }
    }
    Ok(view)
}

fn validate_public_usage(value: &mut Value) -> Result<(), Box<dyn Error>> {
    let object = value.as_object_mut().ok_or("public object")?;
    validate_usage_evidence(object)?;
    for name in ["response", "message"] {
        if let Some(object) = object.get_mut(name).and_then(Value::as_object_mut) {
            validate_usage_evidence(object)?;
        }
    }
    Ok(())
}

fn validate_usage_evidence(parent: &mut Map<String, Value>) -> Result<(), Box<dyn Error>> {
    let chat = parent
        .get("object")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "chat.completion" | "chat.completion.chunk"));
    if parent.get("usage").is_some_and(Value::is_null) {
        parent.remove("usage");
        return Ok(());
    }
    let Some(usage) = parent.get_mut("usage").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    let Some(evidence) = usage.remove("cpar_usage") else {
        return Ok(());
    };
    let evidence = evidence.as_object().ok_or("public usage evidence")?;
    assert_eq!(evidence.len(), 8);
    for field in [
        "input_tokens",
        "output_tokens",
        "cached_tokens",
        "reasoning_tokens",
        "cache_read_tokens",
        "cache_creation_tokens",
    ] {
        let count = evidence.get(field).ok_or("source count")?;
        assert!(count.is_null() || count.as_u64().is_some());
    }
    assert!(matches!(
        evidence.get("provenance").and_then(Value::as_str),
        Some("measured" | "estimated" | "unknown")
    ));
    assert!(matches!(
        evidence.get("input_accounting").and_then(Value::as_str),
        Some("inclusive" | "exclusive" | "unknown")
    ));
    if chat && (!usage.contains_key("prompt_tokens") || !usage.contains_key("completion_tokens")) {
        // Public aggregates may be absent when cache accounting cannot be converted exactly.
        // Validate the retained evidence/counters without inventing an upstream decoder total.
        for (name, count) in usage.iter() {
            assert!(matches!(
                name.as_str(),
                "prompt_tokens" | "completion_tokens"
            ));
            assert!(count.as_u64().is_some());
        }
        parent.remove("usage");
    }
    Ok(())
}

fn validate_wire_terminal(uri: &str, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let expected = match uri {
        "/v1/responses" => "response.completed",
        "/v1/chat/completions" => "[DONE]",
        "/v1/messages" => "message_stop",
        _ => return Err("public protocol".into()),
    };
    let mut terminals = 0;
    for data in std::str::from_utf8(bytes)?
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
    {
        let data = data.trim();
        if data == "[DONE]" {
            assert_eq!(expected, "[DONE]");
            terminals += 1;
        } else {
            let value: Value = serde_json::from_str(data)?;
            assert!(value.get("error").is_none());
            assert!(!matches!(
                value["type"].as_str(),
                Some("error" | "response.failed" | "response.incomplete")
            ));
            if value["type"] == expected {
                terminals += 1;
            }
        }
    }
    assert_eq!(terminals, 1, "exactly one public wire success terminal");
    Ok(())
}
