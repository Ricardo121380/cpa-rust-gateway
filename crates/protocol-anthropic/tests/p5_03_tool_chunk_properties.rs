//! P5-03 properties for the Canonical Tool-to-Anthropic Messages boundary.
//!
//! The suite operates on already decoded UTF-8 Canonical fragments. It proves semantic Tool
//! projection across arbitrary scalar-boundary schedules, rather than raw network-byte parsing.

#![deny(unsafe_code)]

use std::{collections::BTreeMap, error::Error, io};

use gateway_core::{
    CanonicalEvent, CanonicalResponse, ErrorScope, GatewayErrorCode, MessageEnd, MessageRole,
    MessageStart, RawExtensions, RawJson, ResponseEnd, ResponseId, ResponseStart, TextDelta,
    ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart, Usage, UsageDelta,
};
use proptest::{
    prelude::{any, prop},
    test_runner::{Config as ProptestConfig, RngSeed, TestCaseError, TestRunner},
};
use protocol_anthropic::{
    AnthropicMessagesSseEncoder, AnthropicResponseMetadata, SseFrame, encode_response,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const FIXED_SEED: u64 = 0x5035_3033_544F_4F4C;
const FIXED_CASES: u32 = 128;
const FIRST_CALL_ID: &str = "tool-weather";
const FIRST_TOOL_NAME: &str = "lookup_weather";
const FIRST_ARGUMENTS: &str = r#"{"city":"Berlin","unicode":"世界","quoted":"a \"quote\""}"#;
const FIRST_ASCII_ARGUMENTS: &str =
    r#"{"city":"Berlin","unicode":"\u4E16\u754C","quoted":"a \"quote\""}"#;
const SECOND_CALL_ID: &str = "tool-time";
const SECOND_TOOL_NAME: &str = "lookup_time";
const SECOND_ARGUMENTS: &str = r#"{"timezone":"Asia/Berlin","path":"C:\\tmp\\tool"}"#;
const EMPTY_ARGUMENTS: &str = "{}";

#[derive(Clone, Copy)]
struct ToolExpectation {
    call_id: &'static str,
    name: &'static str,
    arguments: &'static str,
}

const FIRST_TOOL: ToolExpectation = ToolExpectation {
    call_id: FIRST_CALL_ID,
    name: FIRST_TOOL_NAME,
    arguments: FIRST_ARGUMENTS,
};
const SECOND_TOOL: ToolExpectation = ToolExpectation {
    call_id: SECOND_CALL_ID,
    name: SECOND_TOOL_NAME,
    arguments: SECOND_ARGUMENTS,
};
const ENTER_PLAN_TOOL: ToolExpectation = ToolExpectation {
    call_id: "tool-enter-plan",
    name: "EnterPlanMode",
    arguments: EMPTY_ARGUMENTS,
};
const EXIT_PLAN_TOOL: ToolExpectation = ToolExpectation {
    call_id: "tool-exit-plan",
    name: "ExitPlanMode",
    arguments: EMPTY_ARGUMENTS,
};
const ORDINARY_NO_ARG_TOOL: ToolExpectation = ToolExpectation {
    call_id: "tool-no-arg",
    name: "ordinary_no_arg_tool",
    arguments: EMPTY_ARGUMENTS,
};
const EXPECTED_TOOLS: [ToolExpectation; 5] = [
    FIRST_TOOL,
    SECOND_TOOL,
    ENTER_PLAN_TOOL,
    EXIT_PLAN_TOOL,
    ORDINARY_NO_ARG_TOOL,
];

#[test]
fn tool_fixture_matches_non_streaming_and_sse_snapshots() -> TestResult {
    let events: Vec<CanonicalEvent> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/anthropic/tool-canonical-events.json"
    ))?;
    let response = CanonicalResponse::try_new(events.clone())?;
    let non_streaming = encode_response(&response, metadata()?)?;
    ensure(
        serde_json::to_string_pretty(&non_streaming)?.trim()
            == include_str!("../../../tests/fixtures/anthropic/tool-non-streaming-response.json")
                .trim(),
        "non-streaming Tool fixture diverged from snapshot",
    )?;

    let mut encoder = AnthropicMessagesSseEncoder::new(metadata()?);
    let mut wire = String::new();
    for event in &events {
        for frame in encoder.encode_event(event)? {
            wire.push_str(&frame.to_wire()?);
        }
    }
    ensure(
        wire.trim_end()
            == include_str!("../../../tests/fixtures/anthropic/tool-stream.sse").trim_end(),
        "SSE Tool fixture diverged from snapshot",
    )?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ToolProjection {
    name: String,
    input: Value,
}

#[derive(Default)]
struct SseProjection {
    tools: BTreeMap<String, ToolProjection>,
    deltas: BTreeMap<String, String>,
    stops: BTreeMap<String, u32>,
    message_stop_count: u32,
    stop_reason: Option<String>,
}

#[test]
fn one_byte_ascii_chunks_interleave_tools_and_preserve_explicit_empty_arguments() -> TestResult {
    let first_chunks = split_ascii_bytes(FIRST_ASCII_ARGUMENTS)?;
    let second_chunks = split_ascii_bytes(SECOND_ARGUMENTS)?;
    let turns = alternating_turns(first_chunks.len(), second_chunks.len());
    let events = build_events(
        &first_chunks,
        &second_chunks,
        &turns,
        FIRST_ASCII_ARGUMENTS,
        SECOND_ARGUMENTS,
    )?;

    verify_projection(&events, FIRST_ASCII_ARGUMENTS, SECOND_ARGUMENTS)
}

#[test]
fn fixed_seed_tool_chunk_interleavings_preserve_final_tool_outputs() -> TestResult {
    run_property_suite(FIXED_SEED, FIXED_CASES)
}

#[test]
fn mismatched_completed_tool_arguments_are_rejected_after_interleaved_deltas() -> TestResult {
    let first_chunks = split_at_scalar_boundaries(FIRST_ARGUMENTS, &[true, false, true]);
    let second_chunks = split_at_scalar_boundaries(SECOND_ARGUMENTS, &[false, true]);
    let turns = alternating_turns(first_chunks.len(), second_chunks.len());
    let mut events = build_events(
        &first_chunks,
        &second_chunks,
        &turns,
        FIRST_ARGUMENTS,
        SECOND_ARGUMENTS,
    )?;

    let replacement = RawJson::from_json_string(r#"{"city":"Tokyo"}"#.to_owned())?;
    let replacement_target = events.iter_mut().find_map(|event| match event {
        CanonicalEvent::ToolCallEnd(end) if end.call_id == FIRST_CALL_ID => Some(end),
        _ => None,
    });
    let Some(replacement_target) = replacement_target else {
        return Err(failure("missing first ToolCallEnd in valid test case"));
    };
    replacement_target.arguments = replacement;

    let response = CanonicalResponse::try_new(events)?;
    assert_stream_protocol_error(encode_response(&response, metadata()?));
    Ok(())
}

#[test]
fn incomplete_tool_cannot_end_the_message_or_complete_the_response() -> TestResult {
    let metadata = metadata()?;
    let mut encoder = AnthropicMessagesSseEncoder::new(metadata);
    let events = [
        response_start()?,
        usage_delta(false),
        message_start(),
        tool_start(FIRST_TOOL),
        CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
            call_id: FIRST_CALL_ID.to_owned(),
            delta: "{\"city\":\"Berlin\"}".to_owned(),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageEnd(MessageEnd {
            extensions: RawExtensions::default(),
        }),
    ];

    for event in events.iter().take(5) {
        let _frames = encoder.encode_event(event)?;
    }
    assert_stream_protocol_error(encoder.encode_event(&events[5]));
    assert_stream_protocol_error(encoder.into_completed_response());
    Ok(())
}

#[test]
fn non_object_tool_arguments_fail_closed() -> TestResult {
    let events = vec![
        response_start()?,
        usage_delta(false),
        message_start(),
        tool_start(FIRST_TOOL),
        tool_end(FIRST_CALL_ID, "[]")?,
        CanonicalEvent::MessageEnd(MessageEnd {
            extensions: RawExtensions::default(),
        }),
        usage_delta(true),
        CanonicalEvent::ResponseEnd(ResponseEnd {
            stop_reason: Some("tool_use".to_owned()),
            stop_sequence: None,
            extensions: RawExtensions::default(),
        }),
    ];
    let response = CanonicalResponse::try_new(events)?;
    assert_stream_protocol_error(encode_response(&response, metadata()?));
    Ok(())
}

#[test]
fn whitespace_wrapped_empty_object_normalizes_without_an_input_delta() -> TestResult {
    let events = vec![
        response_start()?,
        usage_delta(false),
        message_start(),
        tool_start(ENTER_PLAN_TOOL),
        tool_end(ENTER_PLAN_TOOL.call_id, " { } ")?,
        CanonicalEvent::MessageEnd(MessageEnd {
            extensions: RawExtensions::default(),
        }),
        usage_delta(true),
        CanonicalEvent::ResponseEnd(ResponseEnd {
            stop_reason: Some("tool_use".to_owned()),
            stop_sequence: None,
            extensions: RawExtensions::default(),
        }),
    ];
    let response = CanonicalResponse::try_new(events.clone())?;
    let non_streaming = encode_response(&response, metadata()?)?;
    let output = output_projection(&non_streaming)?;
    let Some(tool) = output.get(ENTER_PLAN_TOOL.call_id) else {
        return Err(failure("missing normalized empty Tool"));
    };
    ensure(
        tool.input == serde_json::json!({}),
        "empty input did not normalize",
    )?;

    let mut encoder = AnthropicMessagesSseEncoder::new(metadata()?);
    let mut frames = Vec::new();
    for event in &events {
        frames.extend(encoder.encode_event(event)?);
    }
    ensure(
        !frames.iter().any(|frame| {
            frame.event() == "content_block_delta"
                && frame
                    .data()
                    .get("delta")
                    .and_then(|delta| delta.get("type"))
                    .and_then(Value::as_str)
                    == Some("input_json_delta")
        }),
        "normalized empty input emitted an input_json_delta",
    )?;
    Ok(())
}

#[test]
fn empty_argument_delta_does_not_create_a_false_final_input_mismatch() -> TestResult {
    let arguments = r#"{"city":"Berlin"}"#;
    let events = vec![
        response_start()?,
        usage_delta(false),
        message_start(),
        tool_start(FIRST_TOOL),
        CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
            call_id: FIRST_CALL_ID.to_owned(),
            delta: String::new(),
            extensions: RawExtensions::default(),
        }),
        tool_end(FIRST_CALL_ID, arguments)?,
        CanonicalEvent::MessageEnd(MessageEnd {
            extensions: RawExtensions::default(),
        }),
        usage_delta(true),
        CanonicalEvent::ResponseEnd(ResponseEnd {
            stop_reason: Some("tool_use".to_owned()),
            stop_sequence: None,
            extensions: RawExtensions::default(),
        }),
    ];
    let response = CanonicalResponse::try_new(events.clone())?;
    let non_streaming = encode_response(&response, metadata()?)?;
    let output = output_projection(&non_streaming)?;
    let Some(tool) = output.get(FIRST_CALL_ID) else {
        return Err(failure("missing Tool after empty argument delta"));
    };
    let expected_input: Value = serde_json::from_str(arguments)?;
    ensure(
        tool.input == expected_input,
        "empty argument delta changed final Tool input",
    )?;

    let mut encoder = AnthropicMessagesSseEncoder::new(metadata()?);
    let mut frames = Vec::new();
    for event in &events {
        frames.extend(encoder.encode_event(event)?);
    }
    let sse = sse_projection(&frames)?;
    ensure(
        sse.deltas.get(FIRST_CALL_ID) == Some(&arguments.to_owned()),
        "empty argument delta was emitted or prevented final Tool input output",
    )?;
    Ok(())
}

fn run_property_suite(seed: u64, cases: u32) -> TestResult {
    let strategy = (
        prop::collection::vec(any::<bool>(), 1..=128),
        prop::collection::vec(any::<bool>(), 1..=128),
        prop::collection::vec(any::<bool>(), 1..=256),
    );
    let config = ProptestConfig {
        cases,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(seed),
        ..ProptestConfig::default()
    };
    let mut runner = TestRunner::new(config);
    let result = runner.run(&strategy, |(first_markers, second_markers, turns)| {
        let first_chunks = split_at_scalar_boundaries(FIRST_ARGUMENTS, &first_markers);
        let second_chunks = split_at_scalar_boundaries(SECOND_ARGUMENTS, &second_markers);
        let test_result = build_events(
            &first_chunks,
            &second_chunks,
            &turns,
            FIRST_ARGUMENTS,
            SECOND_ARGUMENTS,
        )
        .and_then(|events| verify_projection(&events, FIRST_ARGUMENTS, SECOND_ARGUMENTS));

        test_result.map_err(|error| TestCaseError::fail(error.to_string()))
    });

    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(failure(format!(
            "property suite seed {seed} failed: {error}"
        ))),
    }
}

fn build_events(
    first_chunks: &[String],
    second_chunks: &[String],
    turns: &[bool],
    first_arguments: &str,
    second_arguments: &str,
) -> Result<Vec<CanonicalEvent>, Box<dyn Error>> {
    let mut events = vec![
        response_start()?,
        usage_delta(false),
        message_start(),
        CanonicalEvent::TextDelta(TextDelta {
            text: "Checking tools.".to_owned(),
            extensions: RawExtensions::default(),
        }),
    ];
    for tool in EXPECTED_TOOLS {
        events.push(tool_start(tool));
    }

    for (call_id, delta) in interleave(first_chunks, second_chunks, turns)? {
        events.push(CanonicalEvent::ToolCallArgumentsDelta(
            ToolCallArgumentsDelta {
                call_id,
                delta,
                extensions: RawExtensions::default(),
            },
        ));
    }

    events.push(tool_end(SECOND_CALL_ID, second_arguments)?);
    events.push(tool_end(
        ENTER_PLAN_TOOL.call_id,
        ENTER_PLAN_TOOL.arguments,
    )?);
    events.push(tool_end(FIRST_CALL_ID, first_arguments)?);
    events.push(tool_end(
        ORDINARY_NO_ARG_TOOL.call_id,
        ORDINARY_NO_ARG_TOOL.arguments,
    )?);
    events.push(tool_end(EXIT_PLAN_TOOL.call_id, EXIT_PLAN_TOOL.arguments)?);
    events.extend([
        CanonicalEvent::MessageEnd(MessageEnd {
            extensions: RawExtensions::default(),
        }),
        usage_delta(true),
        CanonicalEvent::ResponseEnd(ResponseEnd {
            stop_reason: Some("tool_use".to_owned()),
            stop_sequence: None,
            extensions: RawExtensions::default(),
        }),
    ]);

    Ok(events)
}

fn response_start() -> Result<CanonicalEvent, Box<dyn Error>> {
    Ok(CanonicalEvent::ResponseStart(ResponseStart {
        response_id: ResponseId::try_new("p5-03-response")?,
        extensions: RawExtensions::default(),
    }))
}

fn usage_delta(is_final: bool) -> CanonicalEvent {
    CanonicalEvent::UsageDelta(UsageDelta {
        usage: Usage {
            input_tokens: Some(17),
            output_tokens: is_final.then_some(23),
            ..Usage::default()
        },
        is_final,
        extensions: RawExtensions::default(),
    })
}

fn message_start() -> CanonicalEvent {
    CanonicalEvent::MessageStart(MessageStart {
        role: MessageRole("assistant".to_owned()),
        extensions: RawExtensions::default(),
    })
}

fn tool_start(tool: ToolExpectation) -> CanonicalEvent {
    CanonicalEvent::ToolCallStart(ToolCallStart {
        call_id: tool.call_id.to_owned(),
        name: tool.name.to_owned(),
        extensions: RawExtensions::default(),
    })
}

fn tool_end(call_id: &str, arguments: &str) -> Result<CanonicalEvent, serde_json::Error> {
    let arguments = RawJson::from_json_string(arguments.to_owned())?;
    Ok(CanonicalEvent::ToolCallEnd(ToolCallEnd {
        call_id: call_id.to_owned(),
        arguments,
        extensions: RawExtensions::default(),
    }))
}

fn interleave(
    first_chunks: &[String],
    second_chunks: &[String],
    turns: &[bool],
) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    if turns.is_empty() {
        return Err(failure(
            "an interleaving schedule must contain at least one turn",
        ));
    }

    let mut first_index = 0_usize;
    let mut second_index = 0_usize;
    let mut turn_index = 0_usize;
    let mut fragments = Vec::with_capacity(first_chunks.len() + second_chunks.len());

    while first_index < first_chunks.len() || second_index < second_chunks.len() {
        let choose_first = turns[turn_index % turns.len()];
        turn_index = turn_index.saturating_add(1);
        let take_first = (choose_first && first_index < first_chunks.len())
            || second_index >= second_chunks.len();
        if take_first {
            fragments.push((FIRST_CALL_ID.to_owned(), first_chunks[first_index].clone()));
            first_index = first_index.saturating_add(1);
        } else {
            fragments.push((
                SECOND_CALL_ID.to_owned(),
                second_chunks[second_index].clone(),
            ));
            second_index = second_index.saturating_add(1);
        }
    }

    Ok(fragments)
}

fn split_at_scalar_boundaries(input: &str, markers: &[bool]) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0_usize;
    for (index, (offset, _)) in input.char_indices().enumerate() {
        if offset > 0 && markers[index % markers.len()] {
            chunks.push(input[start..offset].to_owned());
            start = offset;
        }
    }
    chunks.push(input[start..].to_owned());
    chunks
}

fn split_ascii_bytes(input: &str) -> Result<Vec<String>, Box<dyn Error>> {
    if !input.is_ascii() {
        return Err(failure("the one-byte regression fixture must be ASCII"));
    }

    Ok(input
        .bytes()
        .map(|byte| char::from(byte).to_string())
        .collect())
}

fn alternating_turns(first_count: usize, second_count: usize) -> Vec<bool> {
    (0..first_count.saturating_add(second_count))
        .map(|index| index % 2 == 0)
        .collect()
}

fn verify_projection(
    events: &[CanonicalEvent],
    first_arguments: &str,
    second_arguments: &str,
) -> TestResult {
    let response = CanonicalResponse::try_new(events.to_vec())?;
    let metadata = metadata()?;
    let non_streaming = encode_response(&response, metadata.clone())?;

    let mut encoder = AnthropicMessagesSseEncoder::new(metadata);
    let mut frames = Vec::new();
    for event in events {
        frames.extend(encoder.encode_event(event)?);
    }
    let streamed_response = encoder.into_completed_response()?;
    ensure(
        non_streaming == streamed_response,
        "streaming and non-streaming final response projections diverged",
    )?;

    let output = output_projection(&non_streaming)?;
    let sse = sse_projection(&frames)?;
    assert_non_overlapping_content_blocks(&frames)?;
    ensure(
        output.len() == EXPECTED_TOOLS.len(),
        "unexpected non-streaming Tool output count",
    )?;
    ensure(
        sse.tools.len() == EXPECTED_TOOLS.len(),
        "unexpected SSE Tool start count",
    )?;
    ensure(
        sse.message_stop_count == 1,
        "expected exactly one Anthropic message_stop frame",
    )?;
    ensure(
        sse.stop_reason.as_deref() == Some("tool_use"),
        "Tool response did not advertise the Anthropic tool_use stop reason",
    )?;

    let expected = [
        (FIRST_CALL_ID, FIRST_TOOL_NAME, first_arguments, true),
        (SECOND_CALL_ID, SECOND_TOOL_NAME, second_arguments, true),
        (
            ENTER_PLAN_TOOL.call_id,
            ENTER_PLAN_TOOL.name,
            ENTER_PLAN_TOOL.arguments,
            false,
        ),
        (
            EXIT_PLAN_TOOL.call_id,
            EXIT_PLAN_TOOL.name,
            EXIT_PLAN_TOOL.arguments,
            false,
        ),
        (
            ORDINARY_NO_ARG_TOOL.call_id,
            ORDINARY_NO_ARG_TOOL.name,
            ORDINARY_NO_ARG_TOOL.arguments,
            false,
        ),
    ];
    for (call_id, name, arguments, emits_deltas) in expected {
        let expected_input: Value = serde_json::from_str(arguments)?;
        let Some(output_tool) = output.get(call_id) else {
            return Err(failure(format!("missing non-streaming Tool {call_id}")));
        };
        ensure(
            output_tool.name == name && output_tool.input == expected_input,
            format!("non-streaming output changed Tool {call_id}"),
        )?;

        let Some(sse_tool) = sse.tools.get(call_id) else {
            return Err(failure(format!("missing SSE Tool start for {call_id}")));
        };
        ensure(
            sse_tool.name == name && sse_tool.input == serde_json::json!({}),
            format!("SSE Tool start changed stable ID/name or default input for {call_id}"),
        )?;
        ensure(
            sse.stops.get(call_id) == Some(&1),
            format!("Tool {call_id} did not receive exactly one content_block_stop"),
        )?;

        if emits_deltas {
            let Some(reassembled) = sse.deltas.get(call_id) else {
                return Err(failure(format!("missing SSE deltas for {call_id}")));
            };
            ensure(
                reassembled == arguments,
                format!("SSE deltas mixed Tool arguments for {call_id}"),
            )?;
        } else {
            ensure(
                !sse.deltas.contains_key(call_id),
                format!("explicit empty Tool {call_id} emitted an argument delta"),
            )?;
            ensure(
                expected_input == serde_json::json!({}),
                format!("no-argument Tool {call_id} was not explicit {{}}"),
            )?;
        }
    }

    Ok(())
}

fn assert_non_overlapping_content_blocks(frames: &[SseFrame]) -> TestResult {
    let mut active_index = None;

    for frame in frames {
        match frame.event() {
            "content_block_start" => {
                ensure(
                    active_index.is_none(),
                    "Anthropic SSE started a content block before stopping the previous block",
                )?;
                active_index = Some(index_field(frame.data())?);
            }
            "content_block_delta" => {
                let index = index_field(frame.data())?;
                ensure(
                    active_index == Some(index),
                    "Anthropic SSE delta targeted a non-active content block",
                )?;
            }
            "content_block_stop" => {
                let index = index_field(frame.data())?;
                ensure(
                    active_index == Some(index),
                    "Anthropic SSE stopped a non-active content block",
                )?;
                active_index = None;
            }
            _ => {}
        }
    }

    ensure(
        active_index.is_none(),
        "Anthropic SSE ended with an open content block",
    )
}

fn output_projection(response: &Value) -> Result<BTreeMap<String, ToolProjection>, Box<dyn Error>> {
    let content = response
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| failure("non-streaming response is missing content"))?;
    let mut projection = BTreeMap::new();
    for item in content {
        if item.get("type").and_then(Value::as_str) != Some("tool_use") {
            continue;
        }
        let call_id = string_field(item, "id")?.to_owned();
        let previous = projection.insert(
            call_id.clone(),
            ToolProjection {
                name: string_field(item, "name")?.to_owned(),
                input: item
                    .get("input")
                    .cloned()
                    .ok_or_else(|| failure("non-streaming Tool lacks input"))?,
            },
        );
        ensure(
            previous.is_none(),
            format!("duplicate non-streaming Tool {call_id}"),
        )?;
    }
    Ok(projection)
}

fn sse_projection(frames: &[SseFrame]) -> Result<SseProjection, Box<dyn Error>> {
    let mut index_to_call = BTreeMap::new();
    let mut projection = SseProjection::default();
    for frame in frames {
        ensure(frame.is_semantic(), "P5-03 emitted a non-semantic frame")?;
        match frame.event() {
            "content_block_start" => {
                let data = frame.data();
                let Some(block) = data.get("content_block") else {
                    return Err(failure("content_block_start lacks content_block"));
                };
                if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                    continue;
                }
                let index = index_field(data)?;
                let call_id = string_field(block, "id")?.to_owned();
                let previous_index = index_to_call.insert(index, call_id.clone());
                ensure(
                    previous_index.is_none(),
                    format!("duplicate SSE content block index {index}"),
                )?;
                let previous_tool = projection.tools.insert(
                    call_id.clone(),
                    ToolProjection {
                        name: string_field(block, "name")?.to_owned(),
                        input: block
                            .get("input")
                            .cloned()
                            .ok_or_else(|| failure("SSE Tool start lacks input"))?,
                    },
                );
                ensure(
                    previous_tool.is_none(),
                    format!("duplicate SSE Tool {call_id}"),
                )?;
            }
            "content_block_delta" => {
                let data = frame.data();
                let Some(delta) = data.get("delta") else {
                    return Err(failure("content_block_delta lacks delta"));
                };
                if delta.get("type").and_then(Value::as_str) != Some("input_json_delta") {
                    continue;
                }
                let call_id = call_id_for_index(index_field(data)?, &index_to_call)?;
                let partial_json = string_field(delta, "partial_json")?;
                projection
                    .deltas
                    .entry(call_id)
                    .or_default()
                    .push_str(partial_json);
            }
            "content_block_stop" => {
                let index = index_field(frame.data())?;
                if let Some(call_id) = index_to_call.get(&index) {
                    *projection.stops.entry(call_id.clone()).or_default() += 1;
                }
            }
            "message_delta" => {
                let Some(delta) = frame.data().get("delta") else {
                    return Err(failure("message_delta lacks delta"));
                };
                projection.stop_reason = Some(string_field(delta, "stop_reason")?.to_owned());
            }
            "message_stop" => {
                projection.message_stop_count = projection.message_stop_count.saturating_add(1);
            }
            _ => {}
        }
    }
    Ok(projection)
}

fn call_id_for_index(
    index: u64,
    index_to_call: &BTreeMap<u64, String>,
) -> Result<String, Box<dyn Error>> {
    index_to_call.get(&index).cloned().ok_or_else(|| {
        failure(format!(
            "SSE frame references undeclared content block {index}"
        ))
    })
}

fn index_field(value: &Value) -> Result<u64, Box<dyn Error>> {
    value
        .get("index")
        .and_then(Value::as_u64)
        .ok_or_else(|| failure("missing numeric content block index"))
}

fn string_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, Box<dyn Error>> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| failure(format!("missing string field {field}")))
}

fn metadata() -> Result<AnthropicResponseMetadata, gateway_core::GatewayError> {
    AnthropicResponseMetadata::try_new("gateway-claude")
}

fn assert_stream_protocol_error<T>(result: Result<T, gateway_core::GatewayError>) {
    assert!(matches!(
        result,
        Err(error)
            if error.code() == GatewayErrorCode::UpstreamProtocolError
                && error.scope() == ErrorScope::Stream
    ));
}

fn ensure(condition: bool, message: impl Into<String>) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(failure(message))
    }
}

fn failure(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::other(message.into()))
}
