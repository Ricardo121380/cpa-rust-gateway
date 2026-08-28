//! P5-08 fixed-corpus and deterministic adversarial properties for Anthropic Messages.
//!
//! These tests exercise only pure protocol values. They retain no client-supplied corpus value in
//! a failure message and do not open a socket, load configuration, or contact a Provider.

#![deny(unsafe_code)]

use std::{
    error::Error,
    io,
    panic::{AssertUnwindSafe, catch_unwind},
};

use gateway_core::{
    CanonicalEvent, ErrorScope, GatewayError, GatewayErrorCode, MessageContent, MessageEnd,
    MessageRole, MessageStart, RawExtensions, RawJson, ResponseEnd, ResponseId, ResponseStart,
    StreamError, ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart, Usage, UsageDelta,
};
use proptest::{
    prelude::{any, prop},
    test_runner::{Config as ProptestConfig, RngSeed, TestCaseError, TestRunner},
};
use protocol_anthropic::{
    AnthropicMessagesSseEncoder, AnthropicResponseMetadata, SseFrame, decode_request,
};
use serde::Deserialize;
use serde_json::json;

type TestResult = Result<(), Box<dyn Error>>;

const FIXED_UNKNOWN_FIELD_SEED: u64 = 0x5035_3038_554E_4B4E;
const FIXED_MALFORMED_STREAM_SEED: u64 = 0x5035_3038_5354_524D;
const FIXED_CASES: u32 = 256;
const TOOL_CALL_ID: &str = "p5-08-tool";

#[derive(Deserialize)]
struct CorpusCase {
    id: String,
    expected: String,
    input: String,
}

#[test]
fn fixed_adversarial_request_corpus_is_total_and_classified() -> TestResult {
    let cases: Vec<CorpusCase> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/anthropic/p5-08-adversarial-request-corpus.json"
    ))?;
    for case in cases {
        match case.expected.as_str() {
            "accept" => {
                let decoded = decode_request(&case.input).map_err(|_| {
                    failure(format!(
                        "fixed corpus case {} unexpectedly rejected",
                        case.id
                    ))
                })?;
                if case.id == "unknown_content_block_is_opaque"
                    && !matches!(
                        decoded
                            .request
                            .messages
                            .first()
                            .and_then(|message| message.content.first()),
                        Some(MessageContent::Opaque(_))
                    )
                {
                    return Err(failure(
                        "unknown content corpus case was not retained as opaque canonical content",
                    ));
                }
            }
            "client_request_error" => {
                assert_client_request_error(decode_request(&case.input), &case.id)?;
            }
            _ => {
                return Err(failure(
                    "fixed corpus contains an unknown expected classification",
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn fixed_seed_unknown_fields_are_retained_without_panic() -> TestResult {
    run_unknown_field_property_suite(FIXED_UNKNOWN_FIELD_SEED, FIXED_CASES)
}

#[test]
fn truncated_tool_is_rejected_without_a_successful_anthropic_termination() -> TestResult {
    let (mut encoder, frames) = open_truncated_tool_encoder()?;
    assert_no_success_termination(&frames)?;

    assert_stream_protocol_error(
        encoder.encode_event(&CanonicalEvent::MessageEnd(MessageEnd {
            extensions: RawExtensions::default(),
        })),
    );
    assert_stream_protocol_error(encoder.into_completed_response());
    Ok(())
}

#[test]
fn fixed_seed_malformed_tool_streams_never_panic_or_complete() -> TestResult {
    run_malformed_stream_property_suite(FIXED_MALFORMED_STREAM_SEED, FIXED_CASES)
}

fn run_unknown_field_property_suite(seed: u64, cases: u32) -> TestResult {
    let strategy = (
        any::<u64>(),
        prop::collection::vec(any::<u8>(), 0..=32),
        any::<bool>(),
    );
    let config = ProptestConfig {
        cases,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(seed),
        ..ProptestConfig::default()
    };
    let mut runner = TestRunner::new(config);
    let result =
        runner.run(
            &strategy,
            |(root_number, message_bytes, block_enabled)| match catch_unwind(AssertUnwindSafe(
                || verify_unknown_extensions(root_number, &message_bytes, block_enabled),
            )) {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(TestCaseError::fail(error.to_string())),
                Err(_) => Err(TestCaseError::fail(
                    "unknown-field decoder property panicked without rendering generated input",
                )),
            },
        );
    property_result(seed, result)
}

fn verify_unknown_extensions(
    root_number: u64,
    message_bytes: &[u8],
    block_enabled: bool,
) -> TestResult {
    let root_value = json!({"number": root_number});
    let message_value = json!({"bytes": message_bytes});
    let block_value = json!({"enabled": block_enabled});
    let input = serde_json::to_string(&json!({
        "model": "p5-08-model",
        "max_tokens": 1,
        "vendor_root": root_value.clone(),
        "messages": [{
            "role": "user",
            "vendor_message": message_value.clone(),
            "content": [{
                "type": "text",
                "text": "fixed-marker",
                "vendor_block": block_value.clone()
            }]
        }]
    }))?;
    let decoded = decode_request(&input)?;
    let message = decoded
        .request
        .messages
        .first()
        .ok_or_else(|| failure("unknown-field request omitted its one canonical message"))?;
    let Some(MessageContent::Text(text)) = message.content.first() else {
        return Err(failure(
            "unknown-field request did not retain its text content as text",
        ));
    };

    assert_raw_extension(
        decoded
            .request
            .extensions
            .get("anthropic.messages.vendor_root"),
        &root_value,
        "root",
    )?;
    assert_raw_extension(
        message.extensions.get("anthropic.vendor_message"),
        &message_value,
        "message",
    )?;
    assert_raw_extension(
        text.extensions.get("anthropic.vendor_block"),
        &block_value,
        "content",
    )?;
    Ok(())
}

fn run_malformed_stream_property_suite(seed: u64, cases: u32) -> TestResult {
    let strategy = prop::collection::vec(any::<u8>(), 1..=64);
    let config = ProptestConfig {
        cases,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(seed),
        ..ProptestConfig::default()
    };
    let mut runner = TestRunner::new(config);
    let result = runner.run(&strategy, |schedule| {
        match catch_unwind(AssertUnwindSafe(|| run_malformed_tool_schedule(&schedule))) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(TestCaseError::fail(error.to_string())),
            Err(_) => Err(TestCaseError::fail(
                "malformed-stream property panicked without rendering its generated schedule",
            )),
        }
    });
    property_result(seed, result)
}

fn run_malformed_tool_schedule(schedule: &[u8]) -> TestResult {
    let (mut encoder, mut frames) = open_truncated_tool_encoder()?;
    for opcode in schedule {
        let event = malformed_event(*opcode)?;
        if let Ok(new_frames) = encoder.encode_event(&event) {
            frames.extend(new_frames);
        }
    }
    assert_no_success_termination(&frames)?;
    if encoder.into_completed_response().is_ok() {
        return Err(failure(
            "a malformed or truncated Tool sequence completed as a successful response",
        ));
    }
    Ok(())
}

fn open_truncated_tool_encoder()
-> Result<(AnthropicMessagesSseEncoder, Vec<SseFrame>), Box<dyn Error>> {
    let mut encoder =
        AnthropicMessagesSseEncoder::new(AnthropicResponseMetadata::try_new("p5-08-model")?);
    let events = [
        CanonicalEvent::ResponseStart(ResponseStart {
            response_id: ResponseId::try_new("p5-08-response")?,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::UsageDelta(UsageDelta {
            usage: Usage {
                input_tokens: Some(1),
                ..Usage::default()
            },
            is_final: false,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::ToolCallStart(ToolCallStart {
            call_id: TOOL_CALL_ID.to_owned(),
            name: "p5_08_tool".to_owned(),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
            call_id: TOOL_CALL_ID.to_owned(),
            delta: "{\"truncated\":".to_owned(),
            extensions: RawExtensions::default(),
        }),
    ];
    let mut frames = Vec::new();
    for event in events {
        frames.extend(encoder.encode_event(&event)?);
    }
    Ok((encoder, frames))
}

fn malformed_event(opcode: u8) -> Result<CanonicalEvent, serde_json::Error> {
    Ok(match opcode % 6 {
        0 => CanonicalEvent::MessageEnd(MessageEnd {
            extensions: RawExtensions::default(),
        }),
        1 => CanonicalEvent::ToolCallStart(ToolCallStart {
            call_id: TOOL_CALL_ID.to_owned(),
            name: "duplicate".to_owned(),
            extensions: RawExtensions::default(),
        }),
        2 => CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
            call_id: "unknown-tool".to_owned(),
            delta: "{}".to_owned(),
            extensions: RawExtensions::default(),
        }),
        3 => CanonicalEvent::ToolCallEnd(ToolCallEnd {
            call_id: TOOL_CALL_ID.to_owned(),
            arguments: RawJson::from_json_string("[]".to_owned())?,
            extensions: RawExtensions::default(),
        }),
        4 => CanonicalEvent::ResponseEnd(ResponseEnd {
            stop_reason: Some("tool_use".to_owned()),
            stop_sequence: None,
            extensions: RawExtensions::default(),
        }),
        _ => CanonicalEvent::StreamError(StreamError {
            error: GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider),
        }),
    })
}

fn assert_client_request_error<T>(result: Result<T, GatewayError>, label: &str) -> TestResult {
    match result {
        Err(error)
            if error.code() == GatewayErrorCode::ClientRequestError
                && error.scope() == ErrorScope::Request =>
        {
            Ok(())
        }
        _ => Err(failure(format!(
            "fixed corpus case {label} did not return ClientRequestError/Request"
        ))),
    }
}

fn assert_raw_extension(
    actual: Option<&RawJson>,
    expected: &serde_json::Value,
    location: &str,
) -> TestResult {
    let expected = serde_json::to_string(expected)?;
    if actual.map(RawJson::get) == Some(expected.as_str()) {
        Ok(())
    } else {
        Err(failure(format!(
            "unknown extension retention diverged at {location}"
        )))
    }
}

fn assert_stream_protocol_error<T>(result: Result<T, GatewayError>) {
    assert!(matches!(
        result,
        Err(error)
            if error.code() == GatewayErrorCode::UpstreamProtocolError
                && error.scope() == ErrorScope::Stream
    ));
}

fn assert_no_success_termination(frames: &[SseFrame]) -> TestResult {
    if frames
        .iter()
        .any(|frame| matches!(frame.event(), "message_delta" | "message_stop"))
    {
        Err(failure(
            "truncated Tool stream emitted a successful Anthropic termination frame",
        ))
    } else {
        Ok(())
    }
}

fn property_result<T: std::fmt::Debug>(
    seed: u64,
    result: Result<(), proptest::test_runner::TestError<T>>,
) -> TestResult {
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(failure(format!(
            "property suite seed {seed} failed: {error}"
        ))),
    }
}

fn failure(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::other(message.into()))
}
