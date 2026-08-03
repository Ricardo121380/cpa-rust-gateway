//! P6-03 synthetic Grok Build Responses request, stream, and error evidence.

use std::{
    collections::BTreeSet,
    error::Error,
    io::Write,
    net::{IpAddr, Ipv4Addr},
};

use flate2::{Compression, write::GzEncoder};
use gateway_core::{
    CanonicalEvent, CanonicalResponse, ClientKeyId, EgressPolicyId, GatewayErrorCode,
    MessageContent, RawJson, ResponseId,
};
use gateway_upstream::{
    EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme,
    RedirectPolicy, UpstreamHttpMethod,
};
use protocol_openai_responses::decode_request;
use provider_grok::{
    GROK_BUILD_AGENT_ID_HEADER, GROK_BUILD_AUTHENTICATE_RESPONSE_HEADER,
    GROK_BUILD_AUTHENTICATE_RESPONSE_VALUE, GROK_BUILD_CLIENT_IDENTIFIER,
    GROK_BUILD_CLIENT_IDENTIFIER_HEADER, GROK_BUILD_CLIENT_MODE, GROK_BUILD_CLIENT_MODE_HEADER,
    GROK_BUILD_CLIENT_VERSION, GROK_BUILD_CLIENT_VERSION_HEADER, GROK_BUILD_MODEL_OVERRIDE_HEADER,
    GROK_BUILD_REQUEST_ID_HEADER, GROK_BUILD_RESPONSES_URL, GROK_BUILD_TOKEN_AUTH_HEADER,
    GROK_BUILD_TOKEN_AUTH_VALUE, GROK_BUILD_TRACEPARENT_HEADER, GROK_BUILD_USER_AGENT,
    GrokBuildCacheIdentityDeriver, GrokBuildCredential, GrokBuildResponsesDecoder,
    GrokBuildResponsesErrorSignal, GrokBuildResponsesHttpError, GrokBuildResponsesRequestBuilder,
    GrokBuildResponsesStreamDecoder, MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES,
};

type TestResult = Result<(), Box<dyn Error>>;
type SemanticProjection = (String, String, String, String, Option<u64>);

struct CorrelationSnapshot {
    agent_id: String,
    request_id: String,
    traceparent: String,
}

#[test]
fn build_request_uses_the_current_cli_profile_and_exact_admitted_target() -> TestResult {
    let decoded = decode_request(include_str!(
        "../../../tests/fixtures/openai-responses/request-canonical.json"
    ))?;
    assert_eq!(
        GrokBuildResponsesRequestBuilder::build(
            &credential()?,
            "grok-build-upstream",
            &decoded.request,
            decoded.mode,
        )
        .err()
        .ok_or("raw cache key unexpectedly reached Build")?
        .code(),
        GatewayErrorCode::UpstreamProtocolError
    );
    let cache_identity = cache_identity(&decoded.request)?;
    let outbound = GrokBuildResponsesRequestBuilder::build_with_cache_identity(
        &credential()?,
        "grok-build-upstream",
        &decoded.request,
        decoded.mode,
        Some(&cache_identity),
    )?;

    let correlations = assert_current_profile(&outbound)?;

    let next = GrokBuildResponsesRequestBuilder::build_with_cache_identity(
        &credential()?,
        "grok-build-upstream",
        &decoded.request,
        decoded.mode,
        Some(&cache_identity),
    )?;
    assert!(
        next.header(GROK_BUILD_AGENT_ID_HEADER)
            .is_some_and(|value| value == correlations.agent_id)
    );
    assert!(
        next.header(GROK_BUILD_REQUEST_ID_HEADER)
            .is_some_and(|value| value != correlations.request_id)
    );

    let body = std::str::from_utf8(outbound.body())?;
    let rebuilt = decode_request(body)?;
    let mut expected = decoded.request.clone();
    expected.requested_model = "grok-build-upstream".to_owned();
    expected.prompt_cache_key = Some(cache_identity.as_str().to_owned());
    assert_eq!(rebuilt.request.requested_model, "grok-build-upstream");
    assert_eq!(rebuilt.request, expected);
    assert_eq!(rebuilt.mode, decoded.mode);
    assert!(!body.contains("gateway-model"));
    assert!(!body.contains("cache-key-01"));

    assert_debug_is_redacted(&outbound, &correlations, &credential()?);
    assert_transport_handoff(outbound)?;
    Ok(())
}

fn cache_identity(
    request: &gateway_core::CanonicalRequest,
) -> Result<provider_grok::GrokBuildCacheIdentity, Box<dyn Error>> {
    let prompt_cache_key = request
        .prompt_cache_key
        .as_deref()
        .ok_or("fixture does not contain a prompt cache key")?;
    Ok(GrokBuildCacheIdentityDeriver::new([0x19; 32]).derive(
        &ClientKeyId::try_new("p6-03-fixture-client")?,
        "grok-build-upstream",
        prompt_cache_key,
    )?)
}

fn assert_current_profile(
    outbound: &provider_grok::GrokBuildResponsesOutboundRequest,
) -> Result<CorrelationSnapshot, Box<dyn Error>> {
    assert_eq!(outbound.url(), GROK_BUILD_RESPONSES_URL);
    assert_eq!(outbound.header("accept"), Some("text/event-stream"));
    assert_eq!(outbound.header("content-type"), Some("application/json"));
    assert_eq!(
        outbound.header(GROK_BUILD_TOKEN_AUTH_HEADER),
        Some(GROK_BUILD_TOKEN_AUTH_VALUE)
    );
    assert_eq!(
        outbound.header(GROK_BUILD_CLIENT_VERSION_HEADER),
        Some(GROK_BUILD_CLIENT_VERSION)
    );
    assert_eq!(
        outbound.header(GROK_BUILD_CLIENT_IDENTIFIER_HEADER),
        Some(GROK_BUILD_CLIENT_IDENTIFIER)
    );
    assert_eq!(
        outbound.header(GROK_BUILD_CLIENT_MODE_HEADER),
        Some(GROK_BUILD_CLIENT_MODE)
    );
    assert_eq!(
        outbound.header(GROK_BUILD_AUTHENTICATE_RESPONSE_HEADER),
        Some(GROK_BUILD_AUTHENTICATE_RESPONSE_VALUE)
    );
    assert_eq!(
        outbound.header(GROK_BUILD_MODEL_OVERRIDE_HEADER),
        Some("grok-build-upstream")
    );
    assert_eq!(outbound.header("accept-encoding"), Some("identity"));
    assert_eq!(GROK_BUILD_USER_AGENT, "grok-shell/0.2.111 (linux; x86_64)");
    assert_eq!(outbound.header("user-agent"), Some(GROK_BUILD_USER_AGENT));
    assert_eq!(outbound.header("connection"), None);

    let correlations = CorrelationSnapshot {
        agent_id: outbound
            .header(GROK_BUILD_AGENT_ID_HEADER)
            .ok_or("agent id missing")?
            .to_owned(),
        request_id: outbound
            .header(GROK_BUILD_REQUEST_ID_HEADER)
            .ok_or("request id missing")?
            .to_owned(),
        traceparent: outbound
            .header(GROK_BUILD_TRACEPARENT_HEADER)
            .ok_or("traceparent missing")?
            .to_owned(),
    };
    assert!(is_uuid_v4(&correlations.agent_id));
    assert!(is_uuid_v4(&correlations.request_id));
    assert!(is_traceparent(&correlations.traceparent));
    Ok(correlations)
}

fn assert_debug_is_redacted(
    outbound: &provider_grok::GrokBuildResponsesOutboundRequest,
    correlations: &CorrelationSnapshot,
    credential: &GrokBuildCredential,
) {
    let debug = format!("{outbound:?}{credential:?}");
    for secret_or_target in [
        "synthetic_grok_build_access_012345",
        "synthetic_grok_build_refresh_012345",
        "cli-chat-proxy.grok.com",
        "What is the weather?",
        "grok-build-upstream",
        &correlations.agent_id,
        &correlations.request_id,
        &correlations.traceparent,
    ] {
        assert!(!debug.contains(secret_or_target));
    }
}

fn assert_transport_handoff(
    outbound: provider_grok::GrokBuildResponsesOutboundRequest,
) -> TestResult {
    let admitted = policy()?.admit_url(outbound.url(), &StaticPublicResolver)?;
    let transport = outbound.into_transport_request(admitted)?;
    assert_eq!(transport.method(), UpstreamHttpMethod::Post);
    assert_eq!(
        transport
            .header("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer synthetic_grok_build_access_012345")
    );
    assert_eq!(
        transport
            .header(GROK_BUILD_TOKEN_AUTH_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some(GROK_BUILD_TOKEN_AUTH_VALUE)
    );
    assert_eq!(transport.header("connection"), None);
    assert!(
        transport
            .header(GROK_BUILD_AGENT_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .is_some_and(is_uuid_v4)
    );
    assert!(
        transport
            .header(GROK_BUILD_REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .is_some_and(is_uuid_v4)
    );
    assert!(
        transport
            .header(GROK_BUILD_TRACEPARENT_HEADER)
            .and_then(|value| value.to_str().ok())
            .is_some_and(is_traceparent)
    );
    assert!(!format!("{transport:?}").contains("synthetic_grok_build_access_012345"));
    Ok(())
}

#[test]
fn one_plain_user_text_uses_scalar_easy_input_without_losing_semantics() -> TestResult {
    let decoded = decode_request(
        r#"{
            "model":"gateway-model",
            "input":"Reply with exactly: ready",
            "max_output_tokens":32,
            "stream":false
        }"#,
    )?;
    let outbound = GrokBuildResponsesRequestBuilder::build(
        &credential()?,
        "grok-build-upstream",
        &decoded.request,
        decoded.mode,
    )?;
    assert_eq!(outbound.header("accept-encoding"), Some("gzip"));
    let body = std::str::from_utf8(outbound.body())?;
    let value: serde_json::Value = serde_json::from_str(body)?;
    assert!(value.get("input").is_some_and(serde_json::Value::is_string));
    let rebuilt = decode_request(body)?;
    let mut expected = decoded.request.clone();
    expected.requested_model = "grok-build-upstream".to_owned();
    assert_eq!(rebuilt.request, expected);
    assert_eq!(rebuilt.mode, decoded.mode);

    let mut extended = decoded.request.clone();
    let [message] = extended.messages.as_mut_slice() else {
        return Err("simple request did not contain one message".into());
    };
    let [MessageContent::Text(text)] = message.content.as_mut_slice() else {
        return Err("simple request did not contain one text part".into());
    };
    text.extensions
        .try_insert("vendor_text", RawJson::from_json_string("true".to_owned())?)?;
    let extended_outbound = GrokBuildResponsesRequestBuilder::build(
        &credential()?,
        "grok-build-upstream",
        &extended,
        decoded.mode,
    )?;
    let extended_body: serde_json::Value = serde_json::from_slice(extended_outbound.body())?;
    assert!(
        extended_body
            .get("input")
            .is_some_and(serde_json::Value::is_array)
    );
    let rebuilt_extended = decode_request(std::str::from_utf8(extended_outbound.body())?)?;
    extended.requested_model = "grok-build-upstream".to_owned();
    assert_eq!(rebuilt_extended.request, extended);
    Ok(())
}

#[test]
fn non_streaming_gzip_is_bounded_and_semantically_equivalent() -> TestResult {
    let plain = include_bytes!("../../../tests/fixtures/grok-build/p6-03-non-streaming.json");
    let expected = GrokBuildResponsesDecoder::decode_non_streaming(plain)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(plain)?;
    let compressed = encoder.finish()?;

    let actual = GrokBuildResponsesDecoder::decode_non_streaming_with_content_encoding(
        Some("gzip"),
        &compressed,
    )?;
    assert_eq!(actual, expected);
    assert!(
        GrokBuildResponsesDecoder::decode_non_streaming_with_content_encoding(Some("br"), plain,)
            .is_err()
    );
    assert!(
        GrokBuildResponsesDecoder::decode_non_streaming_with_content_encoding(
            Some("gzip"),
            b"not-gzip",
        )
        .is_err()
    );
    let mut trailing = compressed.clone();
    trailing.push(0);
    assert!(
        GrokBuildResponsesDecoder::decode_non_streaming_with_content_encoding(
            Some("gzip"),
            &trailing,
        )
        .is_err()
    );

    let oversized = vec![b'x'; MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES + 1];
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&oversized)?;
    let compressed_oversized = encoder.finish()?;
    assert!(
        GrokBuildResponsesDecoder::decode_non_streaming_with_content_encoding(
            Some("gzip"),
            &compressed_oversized,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn arbitrary_sse_chunks_and_non_streaming_fixture_have_the_same_semantic_projection() -> TestResult
{
    let mut stream = include_bytes!("../../../tests/fixtures/grok-build/p6-03-stream.sse").to_vec();
    stream.push(b'\n');
    let expected = GrokBuildResponsesDecoder::decode_non_streaming(include_bytes!(
        "../../../tests/fixtures/grok-build/p6-03-non-streaming.json"
    ))?;
    let expected_projection = projection(expected.events())?;

    for chunk_size in [1, 2, 7, 31, 257, 4096] {
        let mut decoder = GrokBuildResponsesStreamDecoder::new();
        let mut events = Vec::new();
        for chunk in stream.chunks(chunk_size) {
            events.extend(decoder.push_bytes(chunk)?);
        }
        decoder.finish()?;
        let response = CanonicalResponse::try_new(events)?;
        assert_eq!(projection(response.events())?, expected_projection);
    }
    Ok(())
}

#[test]
fn error_envelope_is_bounded_and_does_not_retain_upstream_text() -> TestResult {
    let error = GrokBuildResponsesHttpError::parse(
        429,
        include_bytes!("../../../tests/fixtures/grok-build/p6-03-http-error.json"),
    )?;
    assert_eq!(error.status(), 429);
    assert_eq!(
        error.signal(),
        GrokBuildResponsesErrorSignal::FreeUsageExhausted
    );
    assert!(!format!("{error:?}").contains("included free usage"));

    let opaque = GrokBuildResponsesHttpError::parse(502, b"<html>synthetic upstream error</html>")?;
    assert_eq!(opaque.signal(), GrokBuildResponsesErrorSignal::None);
    assert!(GrokBuildResponsesHttpError::parse(200, b"{}").is_err());
    Ok(())
}

#[test]
fn stream_rejects_duplicate_json_names_and_reports_truncation_without_advancing() -> TestResult {
    let duplicate = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"discarded-before-error\"}}\n\n",
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"first\",\"id\":\"second\"}}\n\n"
    );
    let mut decoder = GrokBuildResponsesStreamDecoder::new();
    let error = decoder
        .push_bytes(duplicate.as_bytes())
        .err()
        .ok_or("duplicate JSON field unexpectedly decoded")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    let recovered = decoder.push_bytes(
        b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"accepted-after-error\"}}\n\n",
    )?;
    assert!(matches!(
        recovered.as_slice(),
        [CanonicalEvent::ResponseStart(start)] if start.response_id.as_str() == "accepted-after-error"
    ));

    let mut truncated = GrokBuildResponsesStreamDecoder::new();
    truncated.push_bytes(
        b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-truncated\"}}\n\n",
    )?;
    let error = truncated.finish().err().ok_or("truncated stream passed")?;
    assert_eq!(error.code(), GatewayErrorCode::StreamTruncated);
    Ok(())
}

#[test]
fn stream_normalizes_empty_tool_arguments_and_rejects_inconsistent_tool_metadata() -> TestResult {
    let valid = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-empty-tool\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc-empty-tool\",\"type\":\"function_call\",\"call_id\":\"call-empty-tool\",\"name\":\"no_arguments\"}}\n\n",
        "event: response.function_call_arguments.done\n",
        "data: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc-empty-tool\",\"call_id\":\"call-empty-tool\",\"arguments\":\" \\t\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"fc-empty-tool\",\"type\":\"function_call\",\"call_id\":\"call-empty-tool\",\"name\":\"no_arguments\",\"arguments\":\"\",\"status\":\"completed\"}}\n\n"
    );
    let events = GrokBuildResponsesStreamDecoder::new().push_bytes(valid.as_bytes())?;
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::ToolCallEnd(end)) if end.arguments.get() == "{}"
    ));

    let inconsistent = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-mismatched-tool\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc-mismatched-tool\",\"type\":\"function_call\",\"call_id\":\"call-mismatched-tool\",\"name\":\"original_name\"}}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"fc-mismatched-tool\",\"type\":\"function_call\",\"call_id\":\"call-mismatched-tool\",\"name\":\"changed_name\",\"arguments\":\"{}\",\"status\":\"completed\"}}\n\n"
    );
    let error = GrokBuildResponsesStreamDecoder::new()
        .push_bytes(inconsistent.as_bytes())
        .err()
        .ok_or("inconsistent completed Tool metadata unexpectedly decoded")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    Ok(())
}

#[test]
fn completed_response_requires_completed_and_exactly_accounted_output_items() -> TestResult {
    let incomplete_item = r#"{
        "id":"resp-incomplete-item",
        "status":"completed",
        "output":[{
            "id":"msg-incomplete-item",
            "type":"message",
            "status":"in_progress",
            "content":[{"type":"output_text","text":"not complete"}]
        }]
    }"#;
    let incomplete = GrokBuildResponsesDecoder::decode_non_streaming(incomplete_item.as_bytes())
        .err()
        .ok_or("in-progress output item unexpectedly completed")?;
    assert_eq!(incomplete.code(), GatewayErrorCode::UpstreamProtocolError);

    let omitted_item = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-omitted-item\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"msg-omitted-item\",\"type\":\"message\"}}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"msg-omitted-item\",\"type\":\"message\",\"status\":\"completed\",\"content\":[{\"type\":\"output_text\",\"text\":\"complete but omitted\"}]}}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-omitted-item\",\"status\":\"completed\",\"output\":[]}}\n\n"
    );
    let omitted = GrokBuildResponsesStreamDecoder::new()
        .push_bytes(omitted_item.as_bytes())
        .err()
        .ok_or("completed response unexpectedly omitted a declared item")?;
    assert_eq!(omitted.code(), GatewayErrorCode::UpstreamProtocolError);
    Ok(())
}

#[test]
fn completed_reasoning_without_content_is_a_zero_delta_canonical_item() -> TestResult {
    let response = br#"{
        "id":"resp-hidden-reasoning",
        "status":"completed",
        "output":[
            {"id":"reasoning-hidden","type":"reasoning","status":"completed"},
            {"id":"message-visible","type":"message","status":"completed","content":[{"type":"output_text","text":"ready"}]}
        ]
    }"#;

    let decoded = GrokBuildResponsesDecoder::decode_non_streaming(response)?;
    assert!(
        decoded.events().iter().any(
            |event| matches!(event, CanonicalEvent::TextDelta(delta) if delta.text == "ready")
        )
    );
    assert!(
        !decoded
            .events()
            .iter()
            .any(|event| matches!(event, CanonicalEvent::ReasoningDelta(_)))
    );
    assert!(matches!(
        decoded.events().last(),
        Some(CanonicalEvent::ResponseEnd(_))
    ));
    Ok(())
}

#[test]
fn reasoning_summary_sse_events_map_to_reasoning_delta_and_validate_part_identity() -> TestResult {
    let stream = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-summary\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"reasoning-summary\",\"type\":\"reasoning\"}}\n\n",
        "event: response.reasoning_summary_part.added\n",
        "data: {\"type\":\"response.reasoning_summary_part.added\",\"item_id\":\"reasoning-summary\",\"part\":{}}\n\n",
        "event: response.reasoning_summary_text.delta\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"item_id\":\"reasoning-summary\",\"delta\":\"summary\"}\n\n",
        "event: response.reasoning_summary_text.done\n",
        "data: {\"type\":\"response.reasoning_summary_text.done\",\"item_id\":\"reasoning-summary\",\"text\":\"summary\"}\n\n",
        "event: response.reasoning_summary_part.done\n",
        "data: {\"type\":\"response.reasoning_summary_part.done\",\"item_id\":\"reasoning-summary\",\"part\":{}}\n\n",
        "event: response.reasoning.done\n",
        "data: {\"type\":\"response.reasoning.done\",\"item_id\":\"reasoning-summary\",\"text\":\"summary\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"reasoning-summary\",\"type\":\"reasoning\",\"status\":\"completed\"}}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-summary\",\"status\":\"completed\",\"output\":[{\"id\":\"reasoning-summary\"}]}}\n\n"
    );
    let mut decoder = GrokBuildResponsesStreamDecoder::new();
    let events = decoder.push_bytes(stream.as_bytes())?;
    decoder.finish()?;
    assert!(events.iter().any(
        |event| matches!(event, CanonicalEvent::ReasoningDelta(delta) if delta.text == "summary")
    ));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, CanonicalEvent::ReasoningDelta(_)))
            .count(),
        1
    );
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::ResponseEnd(_))
    ));
    Ok(())
}

#[test]
fn reasoning_summary_text_done_must_confirm_the_accumulated_delta() -> TestResult {
    let stream = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-summary-mismatch\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"reasoning-summary-mismatch\",\"type\":\"reasoning\"}}\n\n",
        "event: response.reasoning_summary_text.delta\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"item_id\":\"reasoning-summary-mismatch\",\"delta\":\"first\"}\n\n",
        "event: response.reasoning_summary_text.done\n",
        "data: {\"type\":\"response.reasoning_summary_text.done\",\"item_id\":\"reasoning-summary-mismatch\",\"text\":\"different\"}\n\n"
    );

    let error = GrokBuildResponsesStreamDecoder::new()
        .push_bytes(stream.as_bytes())
        .err()
        .ok_or("mismatched reasoning summary text unexpectedly decoded")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    Ok(())
}

#[test]
fn output_text_content_part_events_are_strict_and_chunk_safe() -> TestResult {
    let stream = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-content-part\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"message-content-part\",\"type\":\"message\"}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"type\":\"response.content_part.added\",\"item_id\":\"message-content-part\",\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"message-content-part\",\"delta\":\"ready\"}\n\n",
        "event: response.output_text.done\n",
        "data: {\"type\":\"response.output_text.done\",\"item_id\":\"message-content-part\",\"text\":\"ready\"}\n\n",
        "event: response.content_part.done\n",
        "data: {\"type\":\"response.content_part.done\",\"item_id\":\"message-content-part\",\"part\":{\"type\":\"output_text\",\"text\":\"ready\"}}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"message-content-part\",\"type\":\"message\",\"status\":\"completed\",\"content\":[{\"type\":\"output_text\",\"text\":\"ready\"}]}}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-content-part\",\"status\":\"completed\",\"output\":[{\"id\":\"message-content-part\"}]}}\n\n"
    );
    let mut decoder = GrokBuildResponsesStreamDecoder::new();
    let mut events = Vec::new();
    for &byte in stream.as_bytes() {
        events.extend(decoder.push_bytes(&[byte])?);
    }
    decoder.finish()?;
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, CanonicalEvent::TextDelta(_)))
            .count(),
        1
    );
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::ResponseEnd(_))
    ));

    let malformed = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-content-part-invalid\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"message-content-part-invalid\",\"type\":\"message\"}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"type\":\"response.content_part.added\",\"item_id\":\"message-content-part-invalid\",\"part\":{\"type\":\"output_text\",\"text\":\"unexpected\"}}\n\n"
    );
    let error = GrokBuildResponsesStreamDecoder::new()
        .push_bytes(malformed.as_bytes())
        .err()
        .ok_or("non-empty content-part start unexpectedly decoded")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    Ok(())
}

fn credential() -> Result<GrokBuildCredential, provider_grok::GrokBuildOAuthError> {
    GrokBuildCredential::import_json(
        br#"{
            "access_token":"synthetic_grok_build_access_012345",
            "refresh_token":"synthetic_grok_build_refresh_012345",
            "expires_in":3600,
            "token_type":"Bearer"
        }"#,
        0,
    )
}

fn projection(events: &[CanonicalEvent]) -> Result<SemanticProjection, Box<dyn Error>> {
    let mut visible = String::new();
    let mut reasoning = String::new();
    let mut tool_name = None;
    let mut tool_arguments = None;
    let mut cached_tokens = None;
    let mut response_id = None;

    for event in events {
        match event {
            CanonicalEvent::ResponseStart(start) => response_id = Some(start.response_id.as_str()),
            CanonicalEvent::TextDelta(delta) => visible.push_str(&delta.text),
            CanonicalEvent::ReasoningDelta(delta) => reasoning.push_str(&delta.text),
            CanonicalEvent::ToolCallStart(start) => tool_name = Some(start.name.as_str()),
            CanonicalEvent::ToolCallEnd(end) => tool_arguments = Some(end.arguments.get()),
            CanonicalEvent::UsageDelta(delta) if delta.is_final => {
                cached_tokens = delta.usage.cached_tokens;
            }
            CanonicalEvent::MessageStart(_)
            | CanonicalEvent::ToolCallArgumentsDelta(_)
            | CanonicalEvent::MessageEnd(_)
            | CanonicalEvent::ResponseEnd(_)
            | CanonicalEvent::StreamError(_)
            | CanonicalEvent::UsageDelta(_) => {}
        }
    }

    let response_id = response_id.ok_or("response id missing")?;
    assert_eq!(
        response_id,
        ResponseId::try_new("resp-grok-build-01")?.as_str()
    );
    Ok((
        visible,
        reasoning,
        tool_name.ok_or("tool name missing")?.to_owned(),
        tool_arguments.ok_or("tool arguments missing")?.to_owned(),
        cached_tokens,
    ))
}

fn is_uuid_v4(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            14 => byte == b'4',
            19 => matches!(byte, b'8' | b'9' | b'a' | b'b'),
            _ => byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase(),
        })
}

fn is_traceparent(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 55
        && bytes[2] == b'-'
        && bytes[35] == b'-'
        && bytes[52] == b'-'
        && bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 2 | 35 | 52) || (byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
        && &value[..2] == "00"
        && &value[53..] == "01"
}

struct StaticPublicResolver;

impl EgressDnsResolver for StaticPublicResolver {
    fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
    }
}

fn policy() -> Result<EgressPolicy, Box<dyn Error>> {
    Ok(EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("p6-03-provider-test-policy")?,
        name: "Grok Build test policy".to_owned(),
        allowed_schemes: BTreeSet::from([EgressScheme::Https]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new("cli-chat-proxy.grok.com")?]),
        allowed_ports: BTreeSet::from([443]),
        allowed_cidrs: BTreeSet::new(),
        redirect_policy: RedirectPolicy::Deny,
    })?)
}
