//! P6-03 synthetic Grok Build Responses request, stream, and error evidence.

use std::{
    collections::BTreeSet,
    error::Error,
    net::{IpAddr, Ipv4Addr},
};

use gateway_core::{
    CanonicalEvent, CanonicalResponse, EgressPolicyId, GatewayErrorCode, ResponseId,
};
use gateway_upstream::{
    EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme,
    RedirectPolicy, UpstreamHttpMethod,
};
use protocol_openai_responses::decode_request;
use provider_grok::{
    GROK_BUILD_CLIENT_VERSION, GROK_BUILD_CLIENT_VERSION_HEADER, GROK_BUILD_RESPONSES_URL,
    GROK_BUILD_TOKEN_AUTH_HEADER, GROK_BUILD_TOKEN_AUTH_VALUE, GROK_BUILD_USER_AGENT,
    GrokBuildCredential, GrokBuildResponsesDecoder, GrokBuildResponsesErrorSignal,
    GrokBuildResponsesHttpError, GrokBuildResponsesRequestBuilder, GrokBuildResponsesStreamDecoder,
};

type TestResult = Result<(), Box<dyn Error>>;
type SemanticProjection = (String, String, String, String, Option<u64>);

#[test]
fn build_request_uses_the_frozen_cli_profile_and_exact_admitted_target() -> TestResult {
    let decoded = decode_request(include_str!(
        "../../../tests/fixtures/openai-responses/request-canonical.json"
    ))?;
    let outbound = GrokBuildResponsesRequestBuilder::build(
        &credential()?,
        "grok-build-upstream",
        &decoded.request,
        decoded.mode,
    )?;

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
    assert_eq!(outbound.header("user-agent"), Some(GROK_BUILD_USER_AGENT));
    assert_eq!(outbound.header("connection"), None);

    let body = std::str::from_utf8(outbound.body())?;
    let rebuilt = decode_request(body)?;
    let mut expected = decoded.request.clone();
    expected.requested_model = "grok-build-upstream".to_owned();
    assert_eq!(rebuilt.request.requested_model, "grok-build-upstream");
    assert_eq!(rebuilt.request, expected);
    assert_eq!(rebuilt.mode, decoded.mode);
    assert!(!body.contains("gateway-model"));

    let debug = format!("{outbound:?}{:?}", credential()?);
    for secret_or_target in [
        "synthetic_grok_build_access_012345",
        "synthetic_grok_build_refresh_012345",
        "cli-chat-proxy.grok.com",
        "What is the weather?",
    ] {
        assert!(!debug.contains(secret_or_target));
    }

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
    assert!(!format!("{transport:?}").contains("synthetic_grok_build_access_012345"));
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
