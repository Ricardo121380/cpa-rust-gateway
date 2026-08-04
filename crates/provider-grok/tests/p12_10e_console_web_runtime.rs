//! P12-10E native Console and production Web runtime contracts.

#![deny(unsafe_code)]

use std::{
    collections::BTreeSet,
    collections::VecDeque,
    error::Error,
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
};

use gateway_core::{
    CanonicalEvent, CanonicalRequest, EgressPolicyId, ErrorScope, GatewayErrorCode, RequestContext,
    RequestId,
};
use gateway_provider::{InferenceAdapter, ProviderFuture};
use gateway_router::{ProtocolFormat, project_protocol_response};
use gateway_upstream::{
    EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme,
    RedirectPolicy, UpstreamHttpMethod, UpstreamProxy,
};
use protocol_openai_responses::{ResponseMode, decode_request};
use provider_grok::{
    GROK_CONSOLE_CLUSTER, GROK_CONSOLE_RESPONSES_URL, GrokConsoleExecutionMode,
    GrokConsoleFailureOwner, GrokConsoleInferenceAdapter, GrokConsoleResponseBody,
    GrokConsoleResponseContentType, GrokConsoleResponsesDecoder,
    GrokConsoleResponsesOutboundRequest, GrokConsoleResponsesRequestBuilder,
    GrokConsoleResponsesStreamDecoder, GrokConsoleSsoToken, GrokConsoleTransport,
    GrokConsoleTransportResponse, GrokWebBrowserEgressSession, GrokWebBrowserUserAgent,
    GrokWebCredential, GrokWebEgressSessionId, GrokWebProductionRequestBuilder,
    GrokWebProductionRequestError, GrokWebProductionStreamDecoder, GrokWebStatsigSignature,
    GrokWebTlsProfile, classify_grok_console_http_failure, grok_console_retry_after_due_at,
};

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 2_000_000;
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";

#[test]
fn console_request_pins_target_headers_and_stateless_normalization() -> TestResult {
    let request = tool_request()?;
    let token = GrokConsoleSsoToken::try_from_bytes(b"synthetic-console-sso")?;
    let outbound = GrokConsoleResponsesRequestBuilder::build(
        &token,
        "grok-4.3",
        &request,
        ResponseMode::Streaming,
    )?;
    assert_eq!(outbound.url(), GROK_CONSOLE_RESPONSES_URL);
    assert_eq!(outbound.header("accept"), Some("text/event-stream"));
    assert_eq!(outbound.header("authorization"), Some("Bearer anonymous"));
    assert_eq!(outbound.header("x-cluster"), Some(GROK_CONSOLE_CLUSTER));
    assert!(outbound.header("cookie").is_some_and(|value| {
        value.contains("sso=synthetic-console-sso")
            && value.contains("sso-rw=synthetic-console-sso")
    }));
    let body: serde_json::Value = serde_json::from_slice(outbound.body())?;
    assert_eq!(body["model"], "grok-4.3");
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert_eq!(body["max_output_tokens"], 1_000_000);
    assert_eq!(body["reasoning"]["effort"], "xhigh");
    assert_eq!(body["include"][0], "reasoning.encrypted_content");
    assert_eq!(body["tools"][0]["type"], "web_search");
    assert_eq!(body["tools"][1]["type"], "x_search");
    assert_eq!(body["tools"][2]["name"], "lookup_weather");
    assert_eq!(body["tool_choice"], "auto");

    let diagnostic = format!("{outbound:?}");
    for secret in ["synthetic-console-sso", "Weather?", "lookup_weather"] {
        assert!(!diagnostic.contains(secret));
    }
    Ok(())
}

#[test]
fn console_accepts_the_native_probe_without_an_unowned_output_extension() -> TestResult {
    let request =
        decode_request(r#"{"model":"cpar-native-grok","input":"Reply with exactly: ready"}"#)?
            .request;
    let token = GrokConsoleSsoToken::try_from_bytes(b"synthetic-console-sso")?;

    let outbound = GrokConsoleResponsesRequestBuilder::build(
        &token,
        "grok-build-0.1",
        &request,
        ResponseMode::NonStreaming,
    )?;
    assert_eq!(outbound.header("accept"), Some("*/*"));
    assert_eq!(
        outbound.header("accept-encoding"),
        Some("gzip, deflate, br, zstd")
    );
    let policy = EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("p12-10e-console-transport")?,
        name: "Console transport test".to_owned(),
        allowed_schemes: BTreeSet::from([EgressScheme::Https]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new("console.x.ai")?]),
        allowed_ports: BTreeSet::from([443]),
        allowed_cidrs: BTreeSet::new(),
        redirect_policy: RedirectPolicy::Deny,
    })?;
    let admitted = policy.admit_url(outbound.url(), &StaticPublicResolver)?;
    let transport = outbound.into_transport_request(admitted)?;
    assert_eq!(transport.method(), UpstreamHttpMethod::Post);
    Ok(())
}

#[test]
fn source_observed_probe_model_is_text_only_bounded_and_not_a_catalog_widening() -> TestResult {
    let request =
        decode_request(r#"{"model":"cpar-native-grok","input":"Reply with exactly: ready"}"#)?
            .request;
    let token = GrokConsoleSsoToken::try_from_bytes(b"synthetic-console-sso")?;
    let observed_model = "fixture-source-observed-model";

    assert!(
        GrokConsoleResponsesRequestBuilder::build(
            &token,
            observed_model,
            &request,
            ResponseMode::NonStreaming,
        )
        .is_err()
    );
    let outbound = GrokConsoleResponsesRequestBuilder::build_observed_probe(
        &token,
        observed_model,
        &request,
        ResponseMode::NonStreaming,
    )?;
    let body: serde_json::Value = serde_json::from_slice(outbound.body())?;
    assert_eq!(body["max_output_tokens"], 32);
    assert!(body.get("tools").is_none());
    assert!(body.get("reasoning").is_none());
    Ok(())
}

#[test]
fn source_observed_catalog_model_keeps_its_verified_console_profile() -> TestResult {
    let request =
        decode_request(r#"{"model":"cpar-native-grok","input":"Reply with exactly: ready"}"#)?
            .request;
    let token = GrokConsoleSsoToken::try_from_bytes(b"synthetic-console-sso")?;
    let outbound = GrokConsoleResponsesRequestBuilder::build_observed_probe(
        &token,
        "grok-4.3",
        &request,
        ResponseMode::NonStreaming,
    )?;
    let body: serde_json::Value = serde_json::from_slice(outbound.body())?;
    assert_eq!(body["max_output_tokens"], 1_000_000);
    assert_eq!(body["reasoning"]["effort"], "medium");
    assert_eq!(body["tools"][0]["type"], "web_search");
    assert_eq!(body["tools"][1]["type"], "x_search");
    assert_eq!(body["tool_choice"], "auto");
    Ok(())
}

struct StaticPublicResolver;

impl EgressDnsResolver for StaticPublicResolver {
    fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
    }
}

#[test]
fn console_json_and_every_sse_chunk_size_preserve_tool_reasoning_and_usage() -> TestResult {
    let expected = GrokConsoleResponsesDecoder::decode_non_streaming(console_json())?;
    let expected_projection = projection(expected.events());
    assert_eq!(
        expected_projection,
        (
            "considered".to_owned(),
            "warm".to_owned(),
            Some("lookup_weather".to_owned()),
            Some(r#"{"city":"Shanghai"}"#.to_owned()),
            Some((4, 2, 1)),
        )
    );

    for chunk_size in [1, 2, 7, 31, 257, 4096] {
        let mut decoder = GrokConsoleResponsesStreamDecoder::new();
        let mut events = Vec::new();
        for chunk in console_sse().chunks(chunk_size) {
            events.extend(decoder.push_bytes(chunk)?);
        }
        decoder.finish()?;
        assert_eq!(projection(&events), expected_projection);
    }
    Ok(())
}

#[test]
fn one_console_execution_projects_to_all_three_public_protocols() -> TestResult {
    let response = GrokConsoleResponsesDecoder::decode_non_streaming(console_bridge_json())?;
    for protocol in [
        ProtocolFormat::OpenAiChatCompletions,
        ProtocolFormat::OpenAiResponses,
        ProtocolFormat::AnthropicMessages,
    ] {
        let projected = project_protocol_response(&response, protocol)?;
        let (_, text, tool, arguments, usage) = projection(projected.events());
        assert_eq!(text, "warm");
        assert_eq!(tool.as_deref(), Some("lookup_weather"));
        assert_eq!(arguments.as_deref(), Some(r#"{"city":"Shanghai"}"#));
        assert_eq!(usage, Some((4, 2, 0)));
    }
    Ok(())
}

#[test]
fn console_errors_and_retry_after_are_exact_and_bounded() -> TestResult {
    assert_eq!(
        classify_grok_console_http_failure(401, false),
        GrokConsoleFailureOwner::Credential
    );
    assert_eq!(
        classify_grok_console_http_failure(403, false),
        GrokConsoleFailureOwner::Egress
    );
    assert_eq!(
        classify_grok_console_http_failure(403, true),
        GrokConsoleFailureOwner::Credential
    );
    assert_eq!(
        classify_grok_console_http_failure(429, false),
        GrokConsoleFailureOwner::Quota
    );
    assert_eq!(
        grok_console_retry_after_due_at("3723", NOW_MS)?,
        NOW_MS + 3_723_000
    );
    for invalid in ["", "0", "-1", "1.5", "604801", "99999999999"] {
        assert!(grok_console_retry_after_due_at(invalid, NOW_MS).is_err());
    }
    assert!(GrokConsoleSsoToken::try_from_bytes(b"bad;cookie").is_err());
    assert!(
        GrokConsoleResponsesDecoder::decode_non_streaming(br#"{"status":"completed"}"#).is_err()
    );
    Ok(())
}

#[test]
fn console_accepts_the_grok2api_canonical_credential_envelope() -> TestResult {
    let token = GrokConsoleSsoToken::try_from_bytes(
        br#"{"sso_token":"synthetic-console-sso","probe_model":"grok-4.3"}"#,
    )?;
    let request = tool_request()?;
    let outbound = GrokConsoleResponsesRequestBuilder::build(
        &token,
        "grok-4.3",
        &request,
        ResponseMode::NonStreaming,
    )?;
    let cookie = outbound
        .header("cookie")
        .ok_or("Console envelope did not produce an SSO cookie")?;
    assert!(cookie.contains("sso=synthetic-console-sso"));
    assert!(!cookie.contains("probe_model"));
    assert!(
        GrokConsoleSsoToken::try_from_bytes(
            br#"{"sso_token":"synthetic-console-sso","probe_model":"unknown","extra":true}"#,
        )
        .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn console_inference_adapter_executes_json_sse_and_safe_http_failures() -> TestResult {
    let expected =
        projection(GrokConsoleResponsesDecoder::decode_non_streaming(console_json())?.events());
    let json = console_adapter(
        GrokConsoleExecutionMode::NonStreaming,
        FixtureConsoleTransport::new(
            200,
            GrokConsoleResponseContentType::Json,
            vec![console_json().to_vec()],
        ),
    )?;
    assert_eq!(projection(&collect(json, tool_request()?).await?), expected);

    for chunk_size in [1, 13, 257] {
        let stream = console_adapter(
            GrokConsoleExecutionMode::Streaming,
            FixtureConsoleTransport::new(
                200,
                GrokConsoleResponseContentType::EventStream,
                console_sse()
                    .chunks(chunk_size)
                    .map(ToOwned::to_owned)
                    .collect(),
            ),
        )?;
        assert_eq!(
            projection(&collect(stream, tool_request()?).await?),
            expected
        );
    }

    let forbidden = console_adapter(
        GrokConsoleExecutionMode::NonStreaming,
        FixtureConsoleTransport::new(
            403,
            GrokConsoleResponseContentType::Json,
            vec![br#"{"message":"clearance"}"#.to_vec()],
        ),
    )?;
    let error = forbidden
        .execute(context()?, tool_request()?)
        .await
        .err()
        .ok_or("Console 403 unexpectedly reached a Canonical source")?;
    assert_eq!(error.code(), GatewayErrorCode::EgressRejected);
    assert_eq!(error.scope(), ErrorScope::Egress);
    Ok(())
}

#[test]
fn web_production_binding_uses_exact_browser_profile_and_live_decoder() -> TestResult {
    let session = web_session()?;
    let request = decode_request(r#"{"model":"public","input":"ready"}"#)?.request;
    let outbound = GrokWebProductionRequestBuilder::build(
        &session,
        GrokWebStatsigSignature::try_new("synthetic-statsig")?,
        "grok-chat-fast",
        &request,
        NOW_MS,
    )?;
    assert_eq!(
        outbound.url(),
        "https://grok.com/rest/app-chat/conversations/new"
    );
    assert_eq!(outbound.header("origin"), Some("https://grok.com"));
    assert_eq!(outbound.header("x-statsig-id"), Some("synthetic-statsig"));
    assert!(outbound.header("cookie").is_some());
    let body: serde_json::Value = serde_json::from_slice(outbound.body())?;
    assert_eq!(body["modeId"], "fast");
    assert_eq!(body["temporary"], true);
    assert_eq!(body["disableMemory"], true);
    assert_eq!(body["message"], "[user]\nready");

    let mut decoder = GrokWebProductionStreamDecoder::new();
    let stream = concat!(
        "{\"result\":{\"conversation\":{\"conversationId\":\"conv-e\"},\"response\":{\"token\":\"rea\",\"isThinking\":true}}}",
        "{\"result\":{\"conversation\":{\"conversationId\":\"conv-e\"},\"response\":{\"token\":\"ready\",\"isThinking\":false}}}",
        "{\"result\":{\"conversation\":{\"conversationId\":\"conv-e\"},\"response\":{\"modelResponse\":{\"message\":\"ready\"}}}}"
    );
    let mut events = Vec::new();
    for chunk in stream.as_bytes().chunks(3) {
        events.extend(decoder.push_bytes(chunk)?);
    }
    decoder.finish()?;
    assert_eq!(projection(&events).0, "rea");
    assert_eq!(projection(&events).1, "ready");
    Ok(())
}

#[test]
fn web_production_binding_rejects_tools_reasoning_and_unknown_models() -> TestResult {
    let session = web_session()?;
    let signature = || GrokWebStatsigSignature::try_new("synthetic-statsig");
    let tool_error = GrokWebProductionRequestBuilder::build(
        &session,
        signature()?,
        "grok-chat-fast",
        &tool_request()?,
        NOW_MS,
    );
    assert!(matches!(
        tool_error,
        Err(GrokWebProductionRequestError::UnsupportedRequest)
    ));
    let text = decode_request(r#"{"model":"public","input":"ready"}"#)?.request;
    assert!(matches!(
        GrokWebProductionRequestBuilder::build(
            &session,
            signature()?,
            "unverified-web-model",
            &text,
            NOW_MS,
        ),
        Err(GrokWebProductionRequestError::UnsupportedModel)
    ));
    Ok(())
}

fn tool_request() -> Result<CanonicalRequest, serde_json::Error> {
    serde_json::from_str(
        r#"{
            "requested_model":"grok",
            "messages":[{"role":"user","content":[{"text":{"text":"Weather?","extensions":{}}}],"extensions":{}}],
            "tools":[{"name":"lookup_weather","description":"lookup","input_schema":{"type":"object","properties":{"city":{"type":"string"}}},"extensions":{}}],
            "thinking":{"effort":"xhigh","extensions":{}},
            "extensions":{}
        }"#,
    )
}

fn console_adapter(
    mode: GrokConsoleExecutionMode,
    transport: FixtureConsoleTransport,
) -> Result<GrokConsoleInferenceAdapter, gateway_core::GatewayError> {
    GrokConsoleInferenceAdapter::try_new(
        GrokConsoleSsoToken::try_from_bytes(b"synthetic-console-sso").map_err(|_| {
            gateway_core::GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
        })?,
        "grok-4.3",
        mode,
        Arc::new(transport),
    )
}

fn context() -> Result<RequestContext, gateway_core::InvalidIdentifier> {
    Ok(RequestContext::new(RequestId::try_new("p12-10e-console")?))
}

async fn collect(
    adapter: GrokConsoleInferenceAdapter,
    request: CanonicalRequest,
) -> Result<Vec<CanonicalEvent>, gateway_core::GatewayError> {
    let mut source = adapter
        .execute(
            context().map_err(|_| {
                gateway_core::GatewayError::new(
                    GatewayErrorCode::InternalError,
                    ErrorScope::Internal,
                )
            })?,
            request,
        )
        .await?;
    let mut events = Vec::new();
    while let Some(event) = source.next_event().await? {
        events.push(event);
    }
    Ok(events)
}

struct FixtureConsoleTransport {
    status: u16,
    content_type: GrokConsoleResponseContentType,
    chunks: Mutex<Option<Vec<Vec<u8>>>>,
}

impl FixtureConsoleTransport {
    fn new(
        status: u16,
        content_type: GrokConsoleResponseContentType,
        chunks: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            status,
            content_type,
            chunks: Mutex::new(Some(chunks)),
        }
    }
}

impl GrokConsoleTransport for FixtureConsoleTransport {
    fn send(
        &self,
        _request: GrokConsoleResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokConsoleTransportResponse, gateway_core::GatewayError>> {
        Box::pin(async move {
            let chunks = self
                .chunks
                .lock()
                .map_err(|_| {
                    gateway_core::GatewayError::new(
                        GatewayErrorCode::InternalError,
                        ErrorScope::Internal,
                    )
                })?
                .take()
                .ok_or_else(|| {
                    gateway_core::GatewayError::new(
                        GatewayErrorCode::InternalError,
                        ErrorScope::Internal,
                    )
                })?;
            Ok(GrokConsoleTransportResponse::new(
                self.status,
                self.content_type,
                Box::new(FixtureConsoleBody(chunks.into())),
            ))
        })
    }
}

struct FixtureConsoleBody(VecDeque<Vec<u8>>);

impl GrokConsoleResponseBody for FixtureConsoleBody {
    fn next_chunk(
        &mut self,
    ) -> ProviderFuture<'_, Result<Option<Vec<u8>>, gateway_core::GatewayError>> {
        Box::pin(async move { Ok(self.0.pop_front()) })
    }
}

fn web_session() -> Result<GrokWebBrowserEgressSession, Box<dyn Error>> {
    let credential = GrokWebCredential::import_sso_json(
        format!(
            r#"{{"kind":"grok_web_sso","account_ref":"web-e","lineage_ref":"lineage-e","revision":1,"expires_at_ms":{},"cookies":[{{"name":"sso","value":"synthetic-web-sso","domain":"grok.com","path":"/","secure":true,"http_only":true}}]}}"#,
            NOW_MS + 60_000
        )
        .as_bytes(),
        NOW_MS,
    )?;
    Ok(GrokWebBrowserEgressSession::try_new(
        GrokWebEgressSessionId::try_new("web-production-e")?,
        credential,
        GrokWebBrowserUserAgent::try_new(USER_AGENT)?,
        GrokWebTlsProfile::try_new("chromium-146-macos")?,
        UpstreamProxy::Direct,
        NOW_MS,
    )?)
}

type SemanticProjection = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<(u64, u64, u64)>,
);

fn projection(events: &[CanonicalEvent]) -> SemanticProjection {
    let mut reasoning = String::new();
    let mut text = String::new();
    let mut tool = None;
    let mut arguments = None;
    let mut usage = None;
    for event in events {
        match event {
            CanonicalEvent::ReasoningDelta(delta) => reasoning.push_str(&delta.text),
            CanonicalEvent::TextDelta(delta) => text.push_str(&delta.text),
            CanonicalEvent::ToolCallStart(start) => tool = Some(start.name.clone()),
            CanonicalEvent::ToolCallEnd(end) => arguments = Some(end.arguments.get().to_owned()),
            CanonicalEvent::UsageDelta(delta) if delta.is_final => {
                usage = Some((
                    delta.usage.input_tokens.unwrap_or(0),
                    delta.usage.output_tokens.unwrap_or(0),
                    delta.usage.reasoning_tokens.unwrap_or(0),
                ));
            }
            CanonicalEvent::ResponseStart(_)
            | CanonicalEvent::MessageStart(_)
            | CanonicalEvent::ToolCallArgumentsDelta(_)
            | CanonicalEvent::UsageDelta(_)
            | CanonicalEvent::MessageEnd(_)
            | CanonicalEvent::ResponseEnd(_)
            | CanonicalEvent::StreamError(_) => {}
        }
    }
    (reasoning, text, tool, arguments, usage)
}

fn console_json() -> &'static [u8] {
    br#"{"id":"resp-e","status":"completed","output":[{"id":"reason-e","type":"reasoning","status":"completed","content":[{"type":"reasoning_text","text":"considered"}]},{"id":"fc-e","type":"function_call","call_id":"call-e","name":"lookup_weather","arguments":"{\"city\":\"Shanghai\"}","status":"completed"},{"id":"msg-e","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"warm"}]}],"usage":{"input_tokens":4,"output_tokens":2,"output_tokens_details":{"reasoning_tokens":1}}}"#
}

fn console_bridge_json() -> &'static [u8] {
    br#"{"id":"resp-bridge","status":"completed","output":[{"id":"fc-bridge","type":"function_call","call_id":"call-bridge","name":"lookup_weather","arguments":"{\"city\":\"Shanghai\"}","status":"completed"},{"id":"msg-bridge","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"warm"}]}],"usage":{"input_tokens":4,"output_tokens":2}}"#
}

fn console_sse() -> &'static [u8] {
    concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-e\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"reason-e\",\"type\":\"reasoning\"}}\n\n",
        "event: response.reasoning_text.delta\n",
        "data: {\"type\":\"response.reasoning_text.delta\",\"item_id\":\"reason-e\",\"delta\":\"considered\"}\n\n",
        "event: response.reasoning.done\n",
        "data: {\"type\":\"response.reasoning.done\",\"item_id\":\"reason-e\",\"text\":\"considered\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"reason-e\",\"type\":\"reasoning\",\"status\":\"completed\",\"content\":[{\"type\":\"reasoning_text\",\"text\":\"considered\"}]}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc-e\",\"type\":\"function_call\",\"call_id\":\"call-e\",\"name\":\"lookup_weather\"}}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc-e\",\"call_id\":\"call-e\",\"delta\":\"{\\\"city\\\":\\\"Shanghai\\\"}\"}\n\n",
        "event: response.function_call_arguments.done\n",
        "data: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc-e\",\"call_id\":\"call-e\",\"arguments\":\"{\\\"city\\\":\\\"Shanghai\\\"}\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"fc-e\",\"type\":\"function_call\",\"call_id\":\"call-e\",\"name\":\"lookup_weather\",\"arguments\":\"{\\\"city\\\":\\\"Shanghai\\\"}\",\"status\":\"completed\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"msg-e\",\"type\":\"message\",\"role\":\"assistant\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg-e\",\"delta\":\"warm\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"msg-e\",\"type\":\"message\",\"role\":\"assistant\",\"status\":\"completed\",\"content\":[{\"type\":\"output_text\",\"text\":\"warm\"}]}}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-e\",\"status\":\"completed\",\"output\":[{\"id\":\"reason-e\"},{\"id\":\"fc-e\"},{\"id\":\"msg-e\"}],\"usage\":{\"input_tokens\":4,\"output_tokens\":2,\"output_tokens_details\":{\"reasoning_tokens\":1}}}}\n\n",
        "event: done\n",
        "data: [DONE]\n\n"
    )
    .as_bytes()
}
