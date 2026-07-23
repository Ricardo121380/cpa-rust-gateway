//! P9-03 fixture-only Grok Web Chat request and streaming grammar evidence.

#![deny(unsafe_code)]

use std::error::Error;

use gateway_core::{CanonicalEvent, CanonicalRequest, ErrorScope, GatewayErrorCode};
use gateway_upstream::UpstreamProxy;
use provider_grok::{
    GROK_WEB_CHAT_FIXTURE_HOST, GROK_WEB_CHAT_FIXTURE_PATH, GrokWebBrowserEgressSession,
    GrokWebBrowserUserAgent, GrokWebChatOutboundRequest, GrokWebChatRequestBuilder,
    GrokWebChatRequestError, GrokWebChatStreamDecoder, GrokWebCredential, GrokWebEgressSessionId,
    GrokWebTlsProfile,
};

type TestResult = Result<(), Box<dyn Error>>;

const OBSERVED_AT_MS: i64 = 1_000_000;
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/537.36";

#[test]
fn fixture_request_binds_the_immutable_browser_fingerprint_and_redacts_values() -> TestResult {
    let session = session()?;
    let request = request("reply exactly ready")?;
    let outbound =
        GrokWebChatRequestBuilder::build(&session, "grok-web-synthetic", &request, OBSERVED_AT_MS)?;

    assert_eq!(
        GrokWebChatOutboundRequest::fixture_host(),
        GROK_WEB_CHAT_FIXTURE_HOST
    );
    assert_eq!(
        GrokWebChatOutboundRequest::fixture_path(),
        GROK_WEB_CHAT_FIXTURE_PATH
    );
    assert_eq!(outbound.header("accept"), Some("text/event-stream"));
    assert_eq!(outbound.header("content-type"), Some("application/json"));
    assert_eq!(
        outbound.header("cookie"),
        Some("csrf=csrf_value; sso_session=session_value_alpha")
    );
    assert_eq!(outbound.header("user-agent"), Some(USER_AGENT));
    let body: serde_json::Value = serde_json::from_slice(outbound.body())?;
    assert_eq!(body["model"], "grok-web-synthetic");
    assert_eq!(body["message"], "reply exactly ready");
    assert_eq!(body["stream"], true);

    let diagnostic = format!("{outbound:?}");
    for private_value in [
        "grok.example.test",
        "session_value_alpha",
        "csrf_value",
        USER_AGENT,
        "grok-web-synthetic",
        "reply exactly ready",
    ] {
        assert!(!diagnostic.contains(private_value));
    }
    Ok(())
}

#[test]
fn later_web_semantics_or_an_unusable_session_are_rejected_before_any_transport() -> TestResult {
    let session = session()?;
    for unsupported in [
        r#"{"requested_model":"grok","messages":[{"role":"user","content":[{"text":{"text":"x","extensions":{}}}],"extensions":{}}],"tools":[{"name":"later","input_schema":{},"extensions":{}}],"extensions":{}}"#,
        r#"{"requested_model":"grok","messages":[{"role":"system","content":[{"text":{"text":"x","extensions":{}}}],"extensions":{}}],"extensions":{}}"#,
        r#"{"requested_model":"grok","messages":[{"role":"user","content":[{"text":{"text":"x","extensions":{}}},{"text":{"text":"y","extensions":{}}}],"extensions":{}}],"extensions":{}}"#,
    ] {
        let request: CanonicalRequest = serde_json::from_str(unsupported)?;
        assert_eq!(
            GrokWebChatRequestBuilder::build(
                &session,
                "grok-web-synthetic",
                &request,
                OBSERVED_AT_MS
            ),
            Err(GrokWebChatRequestError::UnsupportedCanonicalRequest)
        );
    }
    assert_eq!(
        GrokWebChatRequestBuilder::build(&session, "", &request("x")?, OBSERVED_AT_MS),
        Err(GrokWebChatRequestError::InvalidModel)
    );
    assert_eq!(
        GrokWebChatRequestBuilder::build(&session, "grok-web-synthetic", &request("x")?, 2_000_000),
        Err(GrokWebChatRequestError::BrowserSessionUnavailable)
    );
    Ok(())
}

#[test]
fn synthetic_sse_is_chunk_invariant_and_has_one_complete_canonical_lifecycle() -> TestResult {
    let fixture = successful_stream();
    let expected = decode_chunks(&fixture, fixture.len())?;
    for chunk_size in [1, 2, 5, 19, 67] {
        assert_eq!(decode_chunks(&fixture, chunk_size)?, expected);
    }
    assert!(matches!(
        expected.first(),
        Some(CanonicalEvent::ResponseStart(_))
    ));
    let text = expected
        .iter()
        .filter_map(|event| match event {
            CanonicalEvent::TextDelta(delta) => Some(delta.text.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(text, "ready");
    assert!(matches!(
        expected.last(),
        Some(CanonicalEvent::ResponseEnd(_))
    ));
    Ok(())
}

#[test]
fn malformed_unknown_premature_or_post_terminal_sse_fails_closed() -> TestResult {
    let mut decoder = GrokWebChatStreamDecoder::new();
    let malformed = b"event: web.response.start\ndata: {\"type\":\"web.response.start\",\"response_id\":\"one\",\"response_id\":\"two\"}\n\n";
    let error = decoder
        .push_bytes(malformed)
        .err()
        .ok_or("duplicate fixture JSON field was accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    assert_eq!(error.scope(), ErrorScope::Stream);

    let mut decoder = GrokWebChatStreamDecoder::new();
    let error = decoder
        .push_bytes(b"event: web.unknown\ndata: {\"type\":\"web.unknown\"}\n\n")
        .err()
        .ok_or("unknown fixture event was accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    assert_eq!(error.scope(), ErrorScope::Stream);

    let mut decoder = GrokWebChatStreamDecoder::new();
    decoder.push_bytes(b"event: web.response.start\ndata: {\"type\":\"web.response.start\",\"response_id\":\"resp-p9-truncated\"}\n\n")?;
    assert_eq!(
        decoder
            .finish()
            .err()
            .ok_or("premature EOF was accepted")?
            .code(),
        GatewayErrorCode::StreamTruncated
    );

    let mut decoder = GrokWebChatStreamDecoder::new();
    let terminal_without_done = concat!(
        "event: web.response.start\n",
        "data: {\"type\":\"web.response.start\",\"response_id\":\"resp-p9-terminal\"}\n\n",
        "event: web.message.start\n",
        "data: {\"type\":\"web.message.start\",\"response_id\":\"resp-p9-terminal\",\"role\":\"assistant\"}\n\n",
        "event: web.message.end\n",
        "data: {\"type\":\"web.message.end\",\"response_id\":\"resp-p9-terminal\"}\n\n",
        "event: web.response.end\n",
        "data: {\"type\":\"web.response.end\",\"response_id\":\"resp-p9-terminal\"}\n\n",
    );
    decoder.push_bytes(terminal_without_done.as_bytes())?;
    let error = decoder
        .push_bytes(b"event: web.text.delta\ndata: {\"type\":\"web.text.delta\",\"response_id\":\"resp-p9-terminal\",\"text\":\"after\"}\n\n")
        .err()
        .ok_or("data after terminal event was accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    Ok(())
}

fn decode_chunks(
    fixture: &[u8],
    chunk_size: usize,
) -> Result<Vec<CanonicalEvent>, gateway_core::GatewayError> {
    let mut decoder = GrokWebChatStreamDecoder::new();
    let mut events = Vec::new();
    for chunk in fixture.chunks(chunk_size) {
        events.extend(decoder.push_bytes(chunk)?);
    }
    decoder.finish()?;
    Ok(events)
}

fn request(text: &str) -> Result<CanonicalRequest, serde_json::Error> {
    serde_json::from_value(serde_json::json!({
        "requested_model": "gateway-web",
        "messages": [{
            "role": "user",
            "content": [{"text": {"text": text, "extensions": {}}}],
            "extensions": {}
        }],
        "extensions": {}
    }))
}

fn session() -> Result<GrokWebBrowserEgressSession, Box<dyn Error>> {
    let credential = GrokWebCredential::import_sso_json(
        br#"{
            "kind":"grok_web_sso",
            "account_ref":"web_account_01",
            "lineage_ref":"sso_import_01",
            "revision":7,
            "expires_at_ms":1500000,
            "cookies":[
                {"name":"sso_session","value":"session_value_alpha","domain":".grok.example.test","path":"/","secure":true,"http_only":true},
                {"name":"csrf","value":"csrf_value","domain":"grok.example.test","path":"/api","secure":true,"http_only":false}
            ]
        }"#,
        OBSERVED_AT_MS,
    )?;
    Ok(GrokWebBrowserEgressSession::try_new(
        GrokWebEgressSessionId::try_new("web_egress_01")?,
        credential,
        GrokWebBrowserUserAgent::try_new(USER_AGENT)?,
        GrokWebTlsProfile::try_new("chrome_136_macos")?,
        UpstreamProxy::Direct,
        OBSERVED_AT_MS,
    )?)
}

fn successful_stream() -> Vec<u8> {
    concat!(
        "event: web.response.start\n",
        "data: {\"type\":\"web.response.start\",\"response_id\":\"resp-p9-stream\"}\n\n",
        "event: web.message.start\n",
        "data: {\"type\":\"web.message.start\",\"response_id\":\"resp-p9-stream\",\"role\":\"assistant\"}\n\n",
        "event: web.text.delta\n",
        "data: {\"type\":\"web.text.delta\",\"response_id\":\"resp-p9-stream\",\"text\":\"rea\"}\n\n",
        "event: web.text.delta\n",
        "data: {\"type\":\"web.text.delta\",\"response_id\":\"resp-p9-stream\",\"text\":\"dy\"}\n\n",
        "event: web.message.end\n",
        "data: {\"type\":\"web.message.end\",\"response_id\":\"resp-p9-stream\"}\n\n",
        "event: web.response.end\n",
        "data: {\"type\":\"web.response.end\",\"response_id\":\"resp-p9-stream\"}\n\n",
        "event: done\n",
        "data: [DONE]\n\n",
    )
    .as_bytes()
    .to_vec()
}
