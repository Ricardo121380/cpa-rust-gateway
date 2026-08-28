//! P8-04 synthetic Official Tool, Reasoning, capability, and Search-boundary evidence.

#![deny(unsafe_code)]

use std::error::Error;

use gateway_catalog::SemanticCapability;
use gateway_core::{CanonicalEvent, CanonicalRequest, ErrorScope, GatewayErrorCode};
use protocol_openai_responses::ResponseMode;
use provider_grok::{
    GrokOfficialApiKey, GrokOfficialCapabilities, GrokOfficialResponsesDecoder,
    GrokOfficialResponsesRequestBuilder, GrokOfficialResponsesStreamDecoder,
    GrokOfficialSearchCapability,
};

type TestResult = Result<(), Box<dyn Error>>;
type SemanticProjection = (String, String, Option<String>, Option<String>, Option<u64>);

const SYNTHETIC_KEY: &str = "synthetic-official-capability-key-012345";

#[test]
fn capability_declaration_admits_only_lossless_semantics() -> TestResult {
    let capabilities = GrokOfficialCapabilities::semantic_capabilities()?;
    for capability in [
        SemanticCapability::Tools,
        SemanticCapability::ParallelTools,
        SemanticCapability::Reasoning,
        SemanticCapability::JsonSchema,
        SemanticCapability::Streaming,
    ] {
        assert!(capabilities.supports(capability));
    }
    assert!(!capabilities.supports(SemanticCapability::Vision));
    assert_eq!(
        GrokOfficialCapabilities::web_search(),
        GrokOfficialSearchCapability::UnavailablePendingCanonicalContract
    );
    Ok(())
}

#[test]
fn tools_reasoning_and_history_encode_without_loss() -> TestResult {
    let request = request()?;
    let outbound = GrokOfficialResponsesRequestBuilder::build(
        &GrokOfficialApiKey::try_new(SYNTHETIC_KEY)?,
        "grok-official-tool-model",
        &request,
        ResponseMode::NonStreaming,
    )?;
    let body: serde_json::Value = serde_json::from_slice(outbound.body())?;

    assert_eq!(body["model"], "grok-official-tool-model");
    assert_eq!(body["reasoning"]["effort"], "medium");
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!(body["tools"][0]["name"], "lookup_weather");
    assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
    assert_eq!(body["input"][1]["type"], "function_call");
    assert_eq!(body["input"][1]["arguments"], r#"{"city":"Shanghai"}"#);
    assert_eq!(body["input"][2]["type"], "function_call_output");
    assert_eq!(body["input"][2]["output"], "warm");

    let diagnostic = format!("{outbound:?}");
    for private_value in [
        SYNTHETIC_KEY,
        "grok-official-tool-model",
        "Shanghai",
        "warm",
    ] {
        assert!(!diagnostic.contains(private_value));
    }
    Ok(())
}

#[test]
fn unsupported_search_and_unsafe_tool_or_reasoning_forms_fail_closed() -> TestResult {
    for input in [
        r#"{"requested_model":"grok","messages":[{"role":"user","content":[{"text":{"text":"search","extensions":{}}}],"extensions":{}}],"extensions":{"openai.responses.tools":[{"type":"web_search"}]}}"#,
        r#"{"requested_model":"grok","messages":[{"role":"user","content":[{"text":{"text":"x","extensions":{}}}],"extensions":{}}],"thinking":{"effort":"xhigh","extensions":{}},"extensions":{}}"#,
        r#"{"requested_model":"grok","messages":[{"role":"assistant","content":[{"tool_call":{"id":"call","name":"lookup","arguments":["not-an-object"],"extensions":{}}}],"extensions":{}}],"extensions":{}}"#,
    ] {
        let request: CanonicalRequest = serde_json::from_str(input)?;
        let error = GrokOfficialResponsesRequestBuilder::build(
            &GrokOfficialApiKey::try_new(SYNTHETIC_KEY)?,
            "grok-official-tool-model",
            &request,
            ResponseMode::NonStreaming,
        )
        .err()
        .ok_or("unrepresentable Official semantic reached outbound transport")?;
        assert_eq!(error.code(), GatewayErrorCode::ClientRequestError);
        assert_eq!(error.scope(), ErrorScope::Request);
    }
    Ok(())
}

#[test]
fn non_streaming_and_every_sse_chunk_size_preserve_tool_reasoning_semantics() -> TestResult {
    let expected =
        GrokOfficialResponsesDecoder::decode_non_streaming(non_streaming_fixture())?.into_events();
    assert_projection(&expected);
    let expected_projection = projection(&expected);

    let stream = stream_fixture();
    for chunk_size in [1, 2, 7, 31, 257, 4096] {
        let mut decoder = GrokOfficialResponsesStreamDecoder::new();
        let mut events = Vec::new();
        for chunk in stream.chunks(chunk_size) {
            events.extend(decoder.push_bytes(chunk)?);
        }
        decoder.finish()?;
        assert_eq!(projection(&events), expected_projection);
    }
    Ok(())
}

#[test]
fn mismatched_or_non_object_tool_arguments_remain_protocol_errors() -> TestResult {
    let invalid = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-invalid-tool\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc-invalid\",\"type\":\"function_call\",\"call_id\":\"call-invalid\",\"name\":\"lookup\"}}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc-invalid\",\"call_id\":\"call-invalid\",\"delta\":\"{}\"}\n\n",
        "event: response.function_call_arguments.done\n",
        "data: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc-invalid\",\"call_id\":\"call-invalid\",\"arguments\":\"[]\"}\n\n",
    );
    let error = GrokOfficialResponsesStreamDecoder::new()
        .push_bytes(invalid.as_bytes())
        .err()
        .ok_or("non-object Tool arguments unexpectedly completed")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    assert_eq!(error.scope(), ErrorScope::Stream);
    Ok(())
}

fn request() -> Result<CanonicalRequest, Box<dyn Error>> {
    Ok(serde_json::from_str(
        r#"{
            "requested_model":"grok",
            "messages":[
                {"role":"user","content":[{"text":{"text":"Weather?","extensions":{}}}],"extensions":{}},
                {"role":"assistant","content":[{"tool_call":{"id":"call-weather","name":"lookup_weather","arguments":{"city":"Shanghai"},"extensions":{}}}],"extensions":{}},
                {"role":"tool","content":[{"tool_result":{"call_id":"call-weather","output":"warm","is_error":false,"extensions":{}}}],"extensions":{}}
            ],
            "tools":[{"name":"lookup_weather","description":"lookup","input_schema":{"type":"object","properties":{"city":{"type":"string"}}},"extensions":{}}],
            "thinking":{"effort":"medium","extensions":{}},
            "extensions":{}
        }"#,
    )?)
}

fn non_streaming_fixture() -> &'static [u8] {
    br#"{
        "id":"resp-p8-capability",
        "status":"completed",
        "output":[
            {"id":"reason-p8-capability","type":"reasoning","status":"completed","content":[{"type":"reasoning_text","text":"considered"}]},
            {"id":"fc-p8-capability","type":"function_call","call_id":"call-p8-capability","name":"lookup_weather","arguments":"{\"city\":\"Shanghai\"}","status":"completed"},
            {"id":"msg-p8-capability","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"warm"}]}
        ],
        "usage":{"input_tokens":4,"output_tokens":2,"output_tokens_details":{"reasoning_tokens":1}}
    }"#
}

fn stream_fixture() -> &'static [u8] {
    concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-p8-capability\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"reason-p8-capability\",\"type\":\"reasoning\"}}\n\n",
        "event: response.reasoning_text.delta\n",
        "data: {\"type\":\"response.reasoning_text.delta\",\"item_id\":\"reason-p8-capability\",\"delta\":\"considered\"}\n\n",
        "event: response.reasoning.done\n",
        "data: {\"type\":\"response.reasoning.done\",\"item_id\":\"reason-p8-capability\",\"text\":\"considered\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"reason-p8-capability\",\"type\":\"reasoning\",\"status\":\"completed\",\"content\":[{\"type\":\"reasoning_text\",\"text\":\"considered\"}]}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc-p8-capability\",\"type\":\"function_call\",\"call_id\":\"call-p8-capability\",\"name\":\"lookup_weather\"}}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc-p8-capability\",\"call_id\":\"call-p8-capability\",\"delta\":\"{\\\"city\\\":\"}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc-p8-capability\",\"call_id\":\"call-p8-capability\",\"delta\":\"\\\"Shanghai\\\"}\"}\n\n",
        "event: response.function_call_arguments.done\n",
        "data: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc-p8-capability\",\"call_id\":\"call-p8-capability\",\"arguments\":\"{\\\"city\\\":\\\"Shanghai\\\"}\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"fc-p8-capability\",\"type\":\"function_call\",\"call_id\":\"call-p8-capability\",\"name\":\"lookup_weather\",\"arguments\":\"{\\\"city\\\":\\\"Shanghai\\\"}\",\"status\":\"completed\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"msg-p8-capability\",\"type\":\"message\",\"role\":\"assistant\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg-p8-capability\",\"delta\":\"warm\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"msg-p8-capability\",\"type\":\"message\",\"role\":\"assistant\",\"status\":\"completed\",\"content\":[{\"type\":\"output_text\",\"text\":\"warm\"}]}}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-p8-capability\",\"status\":\"completed\",\"output\":[{\"id\":\"reason-p8-capability\"},{\"id\":\"fc-p8-capability\"},{\"id\":\"msg-p8-capability\"}],\"usage\":{\"input_tokens\":4,\"output_tokens\":2,\"output_tokens_details\":{\"reasoning_tokens\":1}}}}\n\n",
        "event: done\n",
        "data: [DONE]\n\n",
    )
    .as_bytes()
}

fn assert_projection(events: &[CanonicalEvent]) {
    let (reasoning, text, tool_name, arguments, reasoning_tokens) = projection(events);
    assert_eq!(reasoning, "considered");
    assert_eq!(text, "warm");
    assert_eq!(tool_name.as_deref(), Some("lookup_weather"));
    assert_eq!(arguments.as_deref(), Some(r#"{"city":"Shanghai"}"#));
    assert_eq!(reasoning_tokens, Some(1));
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::ResponseEnd(_))
    ));
}

fn projection(events: &[CanonicalEvent]) -> SemanticProjection {
    let mut reasoning = String::new();
    let mut text = String::new();
    let mut tool_name = None;
    let mut arguments = None;
    let mut reasoning_tokens = None;
    for event in events {
        match event {
            CanonicalEvent::ReasoningDelta(delta) => reasoning.push_str(&delta.text),
            CanonicalEvent::TextDelta(delta) => text.push_str(&delta.text),
            CanonicalEvent::ToolCallStart(start) => tool_name = Some(start.name.as_str()),
            CanonicalEvent::ToolCallEnd(end) => arguments = Some(end.arguments.get()),
            CanonicalEvent::UsageDelta(delta) if delta.is_final => {
                reasoning_tokens = delta.usage.reasoning_tokens;
            }
            CanonicalEvent::ResponseStart(_)
            | CanonicalEvent::MessageStart(_)
            | CanonicalEvent::ToolCallArgumentsDelta(_)
            | CanonicalEvent::MessageEnd(_)
            | CanonicalEvent::ResponseEnd(_)
            | CanonicalEvent::StreamError(_)
            | CanonicalEvent::UsageDelta(_) => {}
        }
    }
    (
        reasoning,
        text,
        tool_name.map(ToOwned::to_owned),
        arguments.map(ToOwned::to_owned),
        reasoning_tokens,
    )
}
