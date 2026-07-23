//! P7-07 Kiro Tool, Thinking, and Claude Code semantic compatibility fixtures.

use std::error::Error;

use gateway_core::{CanonicalEvent, CanonicalRequest, CanonicalResponse, ResponseId};
use provider_kiro::{
    conversation_request::{
        KiroConversationContext, KiroConversationId, KiroConversationRequestBuilder,
        KiroEnvironmentState,
    },
    endpoint_policy::{KiroApiRegion, KiroEndpointKind, KiroEndpointPolicy},
    event_semantics::{KiroEventSemanticError, KiroEventSemanticMapper},
    event_stream::KiroEventStreamDecoder,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn historical_tools_and_endpoint_specific_thinking_preserve_kiro_semantics() -> TestResult {
    let request = decode_request(json!({
        "requested_model": "public-alias-never-forwarded",
        "messages": [
            {"role":"assistant","content":[
                {"text":{"text":"I will ask a question.","extensions":{}}},
                {"tool_call":{"id":"call-ask","name":"AskUserQuestion","arguments":{"questions":[{"question":"Choose a mode","options":[{"label":"safe"}],"multiSelect":false}]},"extensions":{}}}
            ],"extensions":{}},
            {"role":"user","content":[
                {"tool_result":{"call_id":"call-ask","output":{"selected":"safe"},"is_error":false,"extensions":{}}}
            ],"extensions":{}},
            {"role":"user","content":[{"text":{"text":"Continue safely.","extensions":{}}}],"extensions":{}}
        ],
        "thinking":{"effort":"high","extensions":{}},
        "extensions":{}
    }))?;

    let ide = build(&request, KiroEndpointKind::Ide)?;
    assert_eq!(
        ide.body(),
        &json!({
            "conversationState": {
                "conversationId": "p7-07-fixture",
                "history": [
                    {"assistantResponseMessage": {
                        "content":"I will ask a question.",
                        "toolUses":[{"name":"AskUserQuestion","toolUseId":"call-ask","input":{"questions":[{"question":"Choose a mode","options":[{"label":"safe"}],"multiSelect":false}]}}]
                    }},
                    {"userInputMessage": {
                        "modelId":"selected-kiro-model",
                        "origin":"AI_EDITOR",
                        "userInputMessageContext":{"toolResults":[{"toolUseId":"call-ask","content":{"selected":"safe"},"status":"success"}]}
                    }}
                ],
                "currentMessage":{"userInputMessage":{
                    "content":"Continue safely.",
                    "modelId":"selected-kiro-model",
                    "origin":"AI_EDITOR",
                    "userInputMessageContext":{
                        "envState":{"operatingSystem":"linux","currentWorkingDirectory":"/workspace/p7-07"},
                        "additionalModelRequestFields":{"thinking":{"effort":"high"}}
                    }
                }}
            }
        })
    );

    let cli = build(&request, KiroEndpointKind::Cli)?;
    let context = &cli.body()["conversationState"]["currentMessage"]["userInputMessage"]["userInputMessageContext"];
    assert_eq!(context["outputConfig"]["effort"], "high");
    assert!(context.get("additionalModelRequestFields").is_none());
    assert_eq!(
        cli.body()["conversationState"]["history"][1]["userInputMessage"]["origin"],
        "KIRO_CLI"
    );
    Ok(())
}

#[test]
fn every_wire_split_has_the_same_text_reasoning_and_tool_semantics() -> TestResult {
    let wire = [
        event_frame(
            "assistantResponseEvent",
            &json!({"content":"visible ","code":"code"}),
        )?,
        event_frame("reasoningContentEvent", &json!({"reasoningContent":"reason"}))?,
        event_frame(
            "toolUseEvent",
            &json!({"toolUseId":"ask-1","name":"AskUserQuestion","input":"{\"questions\":[{\"header\":\"Choose\",","stop":false}),
        )?,
        event_frame(
            "toolUseEvent",
            &json!({"toolUseId":"ask-1","input":"\"options\":[{\"label\":\"A\"}],\"multiSelect\":false}]}","stop":true}),
        )?,
        event_frame(
            "toolUseEvent",
            &json!({"toolUseId":"plan-1","name":"EnterPlanMode","stop":true}),
        )?,
        event_frame("contextUsageEvent", &json!({"ignored":true}))?,
    ]
    .concat();
    let baseline = map_chunks(&wire, &[wire.len()])?;

    for split in 0..=wire.len() {
        assert_eq!(map_chunks(&wire, &[split, wire.len() - split])?, baseline);
    }
    assert_eq!(map_chunks(&wire, &vec![1; wire.len()])?, baseline);
    CanonicalResponse::try_new(baseline.clone())?;

    let ask_arguments = baseline.iter().find_map(|event| match event {
        CanonicalEvent::ToolCallEnd(end) if end.call_id == "ask-1" => Some(end.arguments.get()),
        _ => None,
    });
    let ask_arguments: Value =
        serde_json::from_str(ask_arguments.ok_or("AskUserQuestion was absent")?)?;
    assert_eq!(ask_arguments["questions"][0]["question"], "Choose");
    assert_eq!(ask_arguments["questions"][0]["options"][0]["label"], "A");
    assert_eq!(ask_arguments["questions"][0]["multiSelect"], false);
    assert!(baseline.iter().any(
        |event| matches!(event, CanonicalEvent::ReasoningDelta(value) if value.text == "reason")
    ));
    assert_eq!(
        baseline
            .iter()
            .filter(|event| matches!(event, CanonicalEvent::TextDelta(_)))
            .count(),
        2
    );
    assert!(baseline.iter().any(
        |event| matches!(event, CanonicalEvent::ToolCallEnd(end) if end.call_id == "plan-1" && end.arguments.get() == "{}")
    ));
    Ok(())
}

#[test]
fn incomplete_or_unsafe_tool_inputs_fail_closed_without_plan_mode_coercion() -> TestResult {
    let incomplete = event_frame(
        "toolUseEvent",
        &json!({"toolUseId":"bad-1","name":"lookup","input":"{\"location\":","stop":true}),
    )?;
    let empty_regular = event_frame(
        "toolUseEvent",
        &json!({"toolUseId":"bad-2","name":"lookup","stop":true}),
    )?;
    let duplicate = event_frame(
        "toolUseEvent",
        &json!({"toolUseId":"bad-3","name":"lookup","input":"{\"city\":\"A\",\"city\":\"B\"}","stop":true}),
    )?;

    for wire in [&incomplete, &duplicate] {
        let error = map_one(wire)
            .err()
            .ok_or("invalid Tool unexpectedly mapped")?;
        assert_eq!(error, KiroEventSemanticError::InvalidPayload);
    }
    assert_eq!(
        map_one(&empty_regular).err(),
        Some(KiroEventSemanticError::InvalidToolState)
    );
    Ok(())
}

#[test]
fn malformed_history_and_debug_forms_do_not_leak_tool_or_thinking_values() -> TestResult {
    let unpaired_result = decode_request(json!({
        "requested_model":"m",
        "messages":[
            {"role":"user","content":[{"tool_result":{"call_id":"unknown","output":{"secret":"result"},"is_error":true,"extensions":{}}}],"extensions":{}}
        ],
        "extensions":{}
    }))?;
    let policy =
        KiroEndpointPolicy::try_new(KiroEndpointKind::Ide, KiroApiRegion::try_new("us-east-1")?)?;
    let error = KiroConversationRequestBuilder::build(
        &policy,
        &context()?,
        "selected-secret-model",
        &unpaired_result,
    )
    .err()
    .ok_or("unpaired result unexpectedly accepted")?;
    assert_eq!(
        error,
        provider_kiro::conversation_request::KiroConversationRequestError::InvalidHistoricalTool
    );

    let duplicate_result = decode_request(json!({
        "requested_model":"m",
        "messages":[
            {"role":"assistant","content":[{"tool_call":{"id":"call-once","name":"lookup","arguments":{},"extensions":{}}}],"extensions":{}},
            {"role":"user","content":[{"tool_result":{"call_id":"call-once","output":{"first":true},"is_error":false,"extensions":{}}}],"extensions":{}},
            {"role":"user","content":[{"tool_result":{"call_id":"call-once","output":{"second":true},"is_error":false,"extensions":{}}}],"extensions":{}}
        ],
        "extensions":{}
    }))?;
    assert_eq!(
        KiroConversationRequestBuilder::build(
            &policy,
            &context()?,
            "selected-secret-model",
            &duplicate_result,
        ),
        Err(provider_kiro::conversation_request::KiroConversationRequestError::InvalidHistoricalTool)
    );

    let mapper = KiroEventSemanticMapper::new(ResponseId::try_new("response-secret")?);
    let diagnostic = format!("{mapper:?}{error:?}");
    for value in [
        "response-secret",
        "selected-secret-model",
        "secret",
        "result",
        "unknown",
    ] {
        assert!(!diagnostic.contains(value));
    }
    Ok(())
}

fn build(
    request: &CanonicalRequest,
    kind: KiroEndpointKind,
) -> Result<provider_kiro::conversation_request::KiroConversationRequest, Box<dyn Error>> {
    let policy = KiroEndpointPolicy::try_new(kind, KiroApiRegion::try_new("us-east-1")?)?;
    Ok(KiroConversationRequestBuilder::build(
        &policy,
        &context()?,
        "selected-kiro-model",
        request,
    )?)
}

fn context() -> Result<KiroConversationContext, Box<dyn Error>> {
    Ok(KiroConversationContext::new(
        KiroConversationId::try_new("p7-07-fixture")?,
        KiroEnvironmentState::try_new("linux", "/workspace/p7-07")?,
    ))
}

fn decode_request(value: Value) -> Result<CanonicalRequest, serde_json::Error> {
    serde_json::from_value(value)
}

fn map_one(wire: &[u8]) -> Result<Vec<CanonicalEvent>, KiroEventSemanticError> {
    map_chunks(wire, &[wire.len()])
}

fn map_chunks(
    wire: &[u8],
    chunks: &[usize],
) -> Result<Vec<CanonicalEvent>, KiroEventSemanticError> {
    let mut framing = KiroEventStreamDecoder::new();
    let mut mapper = KiroEventSemanticMapper::new(
        ResponseId::try_new("p7-07-response")
            .map_err(|_| KiroEventSemanticError::InvalidLifecycle)?,
    );
    let mut events = mapper.start()?;
    let mut offset = 0;
    for length in chunks {
        let end = offset + length;
        framing
            .feed(&wire[offset..end])
            .map_err(|_| KiroEventSemanticError::UnexpectedFrame)?;
        while let Some(frame) = framing
            .next_frame()
            .map_err(|_| KiroEventSemanticError::UnexpectedFrame)?
        {
            events.extend(mapper.push_frame(&frame)?);
        }
        offset = end;
    }
    assert_eq!(offset, wire.len());
    framing
        .finish()
        .map_err(|_| KiroEventSemanticError::UnexpectedFrame)?;
    events.extend(mapper.finish()?);
    Ok(events)
}

fn event_frame(event_type: &str, payload: &Value) -> Result<Vec<u8>, Box<dyn Error>> {
    let payload = serde_json::to_vec(payload)?;
    wire_frame(
        &[
            string_header(":message-type", "event")?,
            string_header(":event-type", event_type)?,
        ],
        &payload,
    )
}

fn wire_frame(headers: &[Vec<u8>], payload: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let headers = headers.concat();
    let total_length = 12 + headers.len() + payload.len() + 4;
    let mut wire = Vec::with_capacity(total_length);
    wire.extend_from_slice(&u32::try_from(total_length)?.to_be_bytes());
    wire.extend_from_slice(&u32::try_from(headers.len())?.to_be_bytes());
    wire.extend_from_slice(&crc32(&wire).to_be_bytes());
    wire.extend_from_slice(&headers);
    wire.extend_from_slice(payload);
    wire.extend_from_slice(&crc32(&wire).to_be_bytes());
    Ok(wire)
}

fn string_header(name: &str, value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut header = Vec::with_capacity(2 + name.len() + value.len());
    header.push(u8::try_from(name.len())?);
    header.extend_from_slice(name.as_bytes());
    header.push(7);
    header.extend_from_slice(&u16::try_from(value.len())?.to_be_bytes());
    header.extend_from_slice(value.as_bytes());
    Ok(header)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}
