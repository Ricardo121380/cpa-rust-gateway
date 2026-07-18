//! P1-09 property coverage for the existing Canonical Tool-to-Responses boundary.
//!
//! These tests deliberately accept already decoded canonical string fragments. They do not model
//! raw network bytes or source-side empty-input normalization.

#![deny(unsafe_code)]

use std::{collections::BTreeMap, error::Error, io};

use gateway_core::{
    CanonicalEvent, CanonicalResponse, ErrorScope, GatewayErrorCode, MessageEnd, MessageRole,
    MessageStart, RawExtensions, RawJson, ResponseEnd, ResponseId, ResponseStart,
    ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart,
};
use proptest::{
    prelude::{any, prop},
    test_runner::{Config as ProptestConfig, RngSeed, TestCaseError, TestRunner},
};
use protocol_openai_responses::{
    OpenAiResponseMetadata, OpenAiResponsesSseEncoder, SseFrame, encode_response,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const FIXED_SEED: u64 = 0x5031_3039_544F_4F4C;
const FIXED_CASES: u32 = 128;
const RANDOM_CASES: u32 = 256;
const FIRST_CALL_ID: &str = "call-weather";
const FIRST_TOOL_NAME: &str = "lookup_weather";
const FIRST_ARGUMENTS: &str = r#"{"city":"Jakarta","unicode":"世界","quoted":"a \\\"quote\\\""}"#;
const FIRST_ASCII_ARGUMENTS: &str =
    r#"{"city":"Jakarta","unicode":"\\u4E16\\u754C","quoted":"a \\\"quote\\\""}"#;
const SECOND_CALL_ID: &str = "call-time";
const SECOND_TOOL_NAME: &str = "lookup_time";
const SECOND_ARGUMENTS: &str = r#"{"timezone":"Asia/Jakarta","path":"C:\\\\tmp\\\\tool"}"#;
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
    call_id: "call-enter-plan",
    name: "EnterPlanMode",
    arguments: EMPTY_ARGUMENTS,
};
const EXIT_PLAN_TOOL: ToolExpectation = ToolExpectation {
    call_id: "call-exit-plan",
    name: "ExitPlanMode",
    arguments: EMPTY_ARGUMENTS,
};
const ORDINARY_NO_ARG_TOOL: ToolExpectation = ToolExpectation {
    call_id: "call-no-arg",
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct ToolProjection {
    name: String,
    arguments: String,
}

#[derive(Default)]
struct SseProjection {
    deltas: BTreeMap<String, String>,
    done: BTreeMap<String, ToolProjection>,
    output_done_counts: BTreeMap<String, u32>,
    sequence_numbers: Vec<u64>,
    completed_count: u32,
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
    match encode_response(&response, metadata()?) {
        Err(error) => ensure(
            error.code() == GatewayErrorCode::UpstreamProtocolError
                && error.scope() == ErrorScope::Stream,
            format!("expected stream protocol error, received {error}"),
        ),
        Ok(_) => Err(failure(
            "mismatched ToolCallEnd arguments unexpectedly encoded successfully",
        )),
    }
}

#[test]
#[ignore = "run scripts/run-p1-09-property.sh to generate or replay P1_09_SEED"]
fn random_seed_tool_chunk_interleavings_are_replayable() -> TestResult {
    let raw_seed = std::env::var("P1_09_SEED")
        .map_err(|_| failure("P1_09_SEED is required for the random property suite"))?;
    let seed = raw_seed
        .parse::<u64>()
        .map_err(|_| failure("P1_09_SEED must be an unsigned decimal integer"))?;

    run_property_suite(seed, RANDOM_CASES)
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
        CanonicalEvent::ResponseStart(ResponseStart {
            response_id: ResponseId::try_new("p1-09-response")?,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
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
        CanonicalEvent::ResponseEnd(ResponseEnd {
            extensions: RawExtensions::default(),
        }),
    ]);

    Ok(events)
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

    let mut encoder = OpenAiResponsesSseEncoder::new(metadata);
    let mut frames = Vec::new();
    for event in events {
        frames.extend(encoder.encode_event(event)?);
    }
    let streamed_response = encoder.into_completed_response()?;
    ensure(
        non_streaming == streamed_response,
        "streaming and non-streaming final response projections diverged",
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
    let output = output_projection(&non_streaming)?;
    let sse = sse_projection(&frames)?;
    ensure(
        output.len() == expected.len(),
        "unexpected Function Call output count",
    )?;
    ensure(
        sse.done.len() == expected.len(),
        "unexpected SSE Tool done count",
    )?;
    ensure(
        sse.completed_count == 1,
        "expected exactly one response.completed frame",
    )?;
    ensure(
        sse.sequence_numbers == (1_u64..=u64::try_from(frames.len())?).collect::<Vec<_>>(),
        "SSE sequence numbers were not contiguous and monotonic",
    )?;

    for (call_id, name, arguments, emits_deltas) in expected {
        let Some(output_tool) = output.get(call_id) else {
            return Err(failure(format!("missing output projection for {call_id}")));
        };
        ensure(
            output_tool.name == name && output_tool.arguments == arguments,
            format!("non-streaming output changed Tool {call_id}"),
        )?;

        let Some(done_tool) = sse.done.get(call_id) else {
            return Err(failure(format!("missing SSE completion for {call_id}")));
        };
        ensure(
            done_tool.name == name && done_tool.arguments == arguments,
            format!("SSE completion changed Tool {call_id}"),
        )?;
        ensure(
            sse.output_done_counts.get(call_id) == Some(&1),
            format!("Tool {call_id} did not receive exactly one output-item.done"),
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
                arguments == EMPTY_ARGUMENTS,
                format!("no-argument Tool {call_id} was not explicit {{}}"),
            )?;
        }
    }

    Ok(())
}

fn output_projection(response: &Value) -> Result<BTreeMap<String, ToolProjection>, Box<dyn Error>> {
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| failure("non-streaming response is missing output"))?;
    let mut projection = BTreeMap::new();

    for item in output {
        if item.get("type").and_then(Value::as_str) != Some("function_call") {
            continue;
        }
        let call_id = string_field(item, "call_id")?.to_owned();
        let previous = projection.insert(
            call_id.clone(),
            ToolProjection {
                name: string_field(item, "name")?.to_owned(),
                arguments: string_field(item, "arguments")?.to_owned(),
            },
        );
        ensure(
            previous.is_none(),
            format!("duplicate non-streaming Function Call {call_id}"),
        )?;
    }

    Ok(projection)
}

fn sse_projection(frames: &[SseFrame]) -> Result<SseProjection, Box<dyn Error>> {
    let mut item_to_call = BTreeMap::new();
    let mut projection = SseProjection::default();

    for frame in frames {
        ensure(
            frame.is_semantic(),
            "P1-05 emitted a non-semantic test frame",
        )?;
        let data = frame.data();
        let sequence_number = data
            .get("sequence_number")
            .and_then(Value::as_u64)
            .ok_or_else(|| failure("SSE frame lacks a numeric sequence_number"))?;
        projection.sequence_numbers.push(sequence_number);

        match frame.event() {
            "response.output_item.added" => {
                let Some(item) = data.get("item") else {
                    return Err(failure("output_item.added lacks item"));
                };
                if item.get("type").and_then(Value::as_str) == Some("function_call") {
                    let item_id = string_field(item, "id")?.to_owned();
                    let call_id = string_field(item, "call_id")?.to_owned();
                    let previous = item_to_call.insert(item_id, call_id.clone());
                    ensure(
                        previous.is_none(),
                        format!("duplicate SSE Function Call item for {call_id}"),
                    )?;
                }
            }
            "response.function_call_arguments.delta" => {
                let call_id = call_id_for_item(data, &item_to_call)?;
                let delta = string_field(data, "delta")?;
                projection
                    .deltas
                    .entry(call_id)
                    .or_default()
                    .push_str(delta);
            }
            "response.function_call_arguments.done" => {
                let call_id = call_id_for_item(data, &item_to_call)?;
                let previous = projection.done.insert(
                    call_id.clone(),
                    ToolProjection {
                        name: string_field(data, "name")?.to_owned(),
                        arguments: string_field(data, "arguments")?.to_owned(),
                    },
                );
                ensure(
                    previous.is_none(),
                    format!("duplicate SSE Tool completion for {call_id}"),
                )?;
            }
            "response.output_item.done" => {
                let Some(item) = data.get("item") else {
                    return Err(failure("output_item.done lacks item"));
                };
                if item.get("type").and_then(Value::as_str) == Some("function_call") {
                    let item_id = string_field(item, "id")?;
                    let Some(call_id) = item_to_call.get(item_id) else {
                        return Err(failure("SSE Tool completion lacks a declared item"));
                    };
                    *projection
                        .output_done_counts
                        .entry(call_id.clone())
                        .or_default() += 1;
                }
            }
            "response.completed" => {
                projection.completed_count = projection.completed_count.saturating_add(1);
            }
            _ => {}
        }
    }

    Ok(projection)
}

fn call_id_for_item(
    data: &Value,
    item_to_call: &BTreeMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    let item_id = string_field(data, "item_id")?;
    item_to_call
        .get(item_id)
        .cloned()
        .ok_or_else(|| failure(format!("SSE frame references undeclared item {item_id}")))
}

fn string_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, Box<dyn Error>> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| failure(format!("missing string field {field}")))
}

fn metadata() -> Result<OpenAiResponseMetadata, gateway_core::GatewayError> {
    OpenAiResponseMetadata::try_new("gateway-model", 1_700_000_000)
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
