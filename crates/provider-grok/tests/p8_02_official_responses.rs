//! P8-02 synthetic Official Responses HTTP/SSE boundary evidence.

#![deny(unsafe_code)]

use std::{
    collections::VecDeque,
    error::Error,
    net::{IpAddr, Ipv4Addr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use gateway_core::{
    CanonicalEvent, CanonicalRequest, CredentialId, EgressPolicyId, EndpointId, ErrorScope,
    GatewayError, GatewayErrorCode, RequestContext, RequestId,
};
use gateway_provider::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};
use gateway_router::{QuotaConfidence, QuotaSource, RuntimeQuotaRegistry, RuntimeQuotaTarget};
use gateway_upstream::{
    EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput,
    EgressScheme, RedirectPolicy, UpstreamHttpMethod,
};
use protocol_openai_responses::decode_request;
use provider_grok::{
    GROK_OFFICIAL_RESPONSES_URL, GrokOfficialApiKey, GrokOfficialExecutionMode,
    GrokOfficialInferenceAdapter, GrokOfficialRateLimitMetadata, GrokOfficialResponseBody,
    GrokOfficialResponseContentType, GrokOfficialResponsesEndpoint,
    GrokOfficialResponsesOutboundRequest, GrokOfficialResponsesRequestBuilder,
    GrokOfficialResponsesStreamDecoder, GrokOfficialRuntimeState, GrokOfficialTransport,
    GrokOfficialTransportResponse,
};

type TestResult = Result<(), Box<dyn Error>>;

const SYNTHETIC_KEY: &str = "synthetic-official-responses-key-012345";

#[test]
fn request_is_fixed_authenticated_post_text_only_and_redacted() -> TestResult {
    let endpoint = GrokOfficialResponsesEndpoint::try_new()?;
    let key = GrokOfficialApiKey::try_new(SYNTHETIC_KEY)?;
    let request = request()?;
    let outbound = GrokOfficialResponsesRequestBuilder::build(
        &key,
        "grok-test-text",
        &request,
        protocol_openai_responses::ResponseMode::NonStreaming,
    )?;
    let expected_authorization = format!("Bearer {SYNTHETIC_KEY}");

    assert_eq!(endpoint.url(), GROK_OFFICIAL_RESPONSES_URL);
    assert_eq!(outbound.url(), GROK_OFFICIAL_RESPONSES_URL);
    assert_eq!(outbound.header("accept"), Some("application/json"));
    assert_eq!(outbound.header("accept-encoding"), Some("identity"));
    assert_eq!(
        outbound.header("authorization"),
        Some(expected_authorization.as_str())
    );
    assert_eq!(outbound.header("content-type"), Some("application/json"));
    let body: serde_json::Value = serde_json::from_slice(outbound.body())?;
    assert_eq!(body["model"], "grok-test-text");
    assert_eq!(body["stream"], false);
    assert_eq!(body["input"][0]["type"], "message");
    assert_eq!(body["input"][0]["role"], "user");
    assert_eq!(body["input"][0]["content"][0]["type"], "input_text");

    let admitted = policy()?.admit_url(outbound.url(), &StaticPublicResolver)?;
    let transport = outbound.into_transport_request(admitted)?;
    assert_eq!(transport.method(), UpstreamHttpMethod::Post);
    assert_eq!(
        transport
            .header("authorization")
            .and_then(|value| value.to_str().ok()),
        Some(expected_authorization.as_str())
    );

    let wrong = GrokOfficialResponsesRequestBuilder::build(
        &key,
        "grok-test-text",
        &request,
        protocol_openai_responses::ResponseMode::NonStreaming,
    )?;
    let wrong_target = policy()?.admit_url("https://api.x.ai/v1/models", &StaticPublicResolver)?;
    let error = wrong
        .into_transport_request(wrong_target)
        .err()
        .ok_or("a distinct admitted Official target reached Responses transport")?;
    assert_eq!(error.code(), GatewayErrorCode::EgressRejected);
    assert_eq!(error.scope(), ErrorScope::Egress);

    let diagnostic = format!("{endpoint:?} {key:?} {transport:?}");
    for private_value in [SYNTHETIC_KEY, "api.x.ai", "Bearer", "grok-test-text"] {
        assert!(!diagnostic.contains(private_value));
    }
    Ok(())
}

#[tokio::test]
async fn non_streaming_text_fixture_runs_through_official_adapter() -> TestResult {
    let transport = Arc::new(ScriptedTransport::new([FixtureResponse::json(
        br#"{
            "id":"resp-p8-text",
            "status":"completed",
            "output":[{
                "id":"msg-p8-text",
                "type":"message",
                "role":"assistant",
                "status":"completed",
                "content":[{"type":"output_text","text":"ready"}]
            }],
            "usage":{"input_tokens":4,"output_tokens":1}
        }"#,
    )]));
    let adapter = adapter(GrokOfficialExecutionMode::NonStreaming, transport.clone())?;

    let events = collect(adapter.execute(context()?, request()?).await?).await?;

    assert_success_shape(&events);
    assert_eq!(adapter.provider_id().as_str(), "grok.official");
    assert_eq!(transport.send_count(), 1);
    assert!(events.iter().any(|event| {
        matches!(event, CanonicalEvent::UsageDelta(usage) if usage.usage.input_tokens == Some(4))
    }));
    Ok(())
}

#[tokio::test]
async fn sse_text_fixture_is_chunk_invariant_and_runs_through_adapter() -> TestResult {
    let fixture = successful_stream();
    let expected = decode_chunks(&fixture, 4096)?;
    for chunk_size in [1, 2, 7, 23, 61] {
        assert_eq!(decode_chunks(&fixture, chunk_size)?, expected);
    }

    let transport = Arc::new(ScriptedTransport::new([FixtureResponse::sse(
        fixture.chunks(17).map(ToOwned::to_owned).collect(),
    )]));
    let adapter = adapter(GrokOfficialExecutionMode::Streaming, transport.clone())?;
    let events = collect(adapter.execute(context()?, request()?).await?).await?;

    assert_eq!(events, expected);
    assert_success_shape(&events);
    assert_eq!(transport.send_count(), 1);
    Ok(())
}

#[tokio::test]
async fn runtime_state_receives_only_the_explicit_official_transport_observation() -> TestResult {
    let runtime_quota = Arc::new(RuntimeQuotaRegistry::new());
    let endpoint_id = EndpointId::try_new("official-runtime-endpoint")?;
    let credential_id = CredentialId::try_new("official-runtime-credential")?;
    let runtime_state = GrokOfficialRuntimeState::try_new(
        endpoint_id.clone(),
        credential_id.clone(),
        Arc::clone(&runtime_quota),
    )?;
    let metadata = GrokOfficialRateLimitMetadata::parse([
        ("x-ratelimit-limit-requests", "10"),
        ("x-ratelimit-remaining-requests", "0"),
        ("x-ratelimit-reset-requests", "1s"),
    ])?;
    let transport = Arc::new(ScriptedTransport::new([FixtureResponse::json(
        br#"{
            "id":"resp-p8-runtime",
            "status":"completed",
            "output":[{
                "id":"msg-p8-runtime",
                "type":"message",
                "role":"assistant",
                "status":"completed",
                "content":[{"type":"output_text","text":"ready"}]
            }]
        }"#,
    )
    .with_rate_limit_metadata(metadata)]));
    let adapter = adapter_with_runtime(
        GrokOfficialExecutionMode::NonStreaming,
        transport,
        runtime_state.clone(),
    )?;

    collect(adapter.execute(context()?, request()?).await?).await?;
    let target = RuntimeQuotaTarget::endpoint_credential(endpoint_id, credential_id);
    let snapshot = runtime_quota
        .snapshot(&target)?
        .ok_or("adapter dropped its explicit Official rate-limit handoff")?;
    assert_eq!(snapshot.source(), QuotaSource::Header);
    assert_eq!(snapshot.confidence(), QuotaConfidence::Observed);

    let retry_after = GrokOfficialRateLimitMetadata::parse([("retry-after", "5")])?;
    let error = adapter_with_runtime(
        GrokOfficialExecutionMode::NonStreaming,
        Arc::new(ScriptedTransport::new([FixtureResponse::new(
            429,
            GrokOfficialResponseContentType::Json,
            Vec::new(),
        )
        .with_rate_limit_metadata(retry_after)])),
        runtime_state,
    )?
    .execute(context()?, request()?)
    .await
    .err()
    .ok_or("Official 429 unexpectedly started a response")?;
    assert_eq!(error.code(), GatewayErrorCode::ProviderRateLimited);
    assert_eq!(error.scope(), ErrorScope::QuotaWindow);
    assert!(runtime_quota.snapshot(&target)?.is_some());
    Ok(())
}

#[test]
fn cache_opaque_and_unsupported_roles_are_rejected_before_transport() -> TestResult {
    let key = GrokOfficialApiKey::try_new(SYNTHETIC_KEY)?;
    for input in [
        r#"{"requested_model":"grok","messages":[{"role":"user","content":[{"text":{"text":"x","extensions":{}}}],"extensions":{}}],"prompt_cache_key":"not-official-yet","extensions":{}}"#,
        r#"{"requested_model":"grok","messages":[{"role":"tool","content":[{"text":{"text":"x","extensions":{}}}],"extensions":{}}],"extensions":{}}"#,
        r#"{"requested_model":"grok","messages":[{"role":"user","content":[{"opaque":{"raw":{"type":"input_image","image_url":"https://example.invalid/x"},"extensions":{}}}],"extensions":{}}],"extensions":{}}"#,
    ] {
        let request: CanonicalRequest = serde_json::from_str(input)?;
        let error = GrokOfficialResponsesRequestBuilder::build(
            &key,
            "grok-test-text",
            &request,
            protocol_openai_responses::ResponseMode::NonStreaming,
        )
        .err()
        .ok_or("P8-02 request builder accepted a later-task semantic")?;
        assert_eq!(error.code(), GatewayErrorCode::ClientRequestError);
        assert_eq!(error.scope(), ErrorScope::Request);
    }
    Ok(())
}

#[tokio::test]
async fn pre_start_error_is_generic_and_post_start_failure_is_one_stream_error() -> TestResult {
    let transport = Arc::new(ScriptedTransport::new([FixtureResponse::new(
        503,
        GrokOfficialResponseContentType::Json,
        vec![br#"{"error":{"message":"not retained"}}"#.to_vec()],
    )]));
    let non_stream_adapter = adapter(GrokOfficialExecutionMode::NonStreaming, transport.clone())?;
    let error = non_stream_adapter
        .execute(context()?, request()?)
        .await
        .err()
        .ok_or("non-success Official status unexpectedly started")?;
    assert_eq!(error.code(), GatewayErrorCode::ProviderTransient);
    assert_eq!(error.scope(), ErrorScope::Provider);
    assert_eq!(transport.send_count(), 1);

    let transport = Arc::new(ScriptedTransport::new([FixtureResponse::sse(vec![
        response_created_record(),
        b"event: response.output_item.added\ndata: {malformed}\n\n".to_vec(),
    ])]));
    let adapter = adapter(GrokOfficialExecutionMode::Streaming, transport)?;
    let events = collect(adapter.execute(context()?, request()?).await?).await?;
    assert!(matches!(
        events.first(),
        Some(CanonicalEvent::ResponseStart(_))
    ));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, CanonicalEvent::StreamError(_)))
            .count(),
        1
    );
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::StreamError(_))
    ));
    Ok(())
}

#[test]
fn response_failed_is_terminal_and_opaque_search_output_fails_closed() -> TestResult {
    let failed = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-p8-failed\"}}\n\n",
        "event: response.failed\n",
        "data: {\"type\":\"response.failed\",\"response\":{\"id\":\"resp-p8-failed\"}}\n\n",
        "event: done\n",
        "data: [DONE]\n\n",
    );
    let failed_events = decode_chunks(failed.as_bytes(), 3)?;
    assert!(matches!(
        failed_events.first(),
        Some(CanonicalEvent::ResponseStart(_))
    ));
    assert!(matches!(
        failed_events.last(),
        Some(CanonicalEvent::StreamError(error))
            if error.error.code() == GatewayErrorCode::ProviderPermanent
                && error.error.scope() == ErrorScope::Provider
    ));

    let mut decoder = GrokOfficialResponsesStreamDecoder::new();
    decoder.push_bytes(&response_created_record())?;
    let error = decoder
        .push_bytes(
            b"event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"search-p8\",\"type\":\"web_search_call\"}}\n\n",
        )
        .err()
        .ok_or("Official decoder accepted an opaque native Search output item")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    assert_eq!(error.scope(), ErrorScope::Stream);
    Ok(())
}

fn adapter(
    mode: GrokOfficialExecutionMode,
    transport: Arc<dyn GrokOfficialTransport>,
) -> Result<GrokOfficialInferenceAdapter, GatewayError> {
    GrokOfficialInferenceAdapter::try_new(
        GrokOfficialApiKey::try_new(SYNTHETIC_KEY)?,
        "grok-test-text",
        mode,
        transport,
    )
}

fn adapter_with_runtime(
    mode: GrokOfficialExecutionMode,
    transport: Arc<dyn GrokOfficialTransport>,
    runtime_state: GrokOfficialRuntimeState,
) -> Result<GrokOfficialInferenceAdapter, GatewayError> {
    GrokOfficialInferenceAdapter::try_new_with_runtime_state(
        GrokOfficialApiKey::try_new(SYNTHETIC_KEY)?,
        "grok-test-text",
        mode,
        transport,
        runtime_state,
    )
}

fn request() -> Result<CanonicalRequest, Box<dyn Error>> {
    Ok(
        decode_request(r#"{"model":"gateway-official","input":"Reply with exactly: ready"}"#)?
            .request,
    )
}

fn context() -> Result<RequestContext, Box<dyn Error>> {
    Ok(RequestContext::new(RequestId::try_new("p8-02-request")?))
}

async fn collect(
    mut source: Box<dyn CanonicalEventSource>,
) -> Result<Vec<CanonicalEvent>, GatewayError> {
    let mut events = Vec::new();
    while let Some(event) = source.next_event().await? {
        events.push(event);
    }
    Ok(events)
}

fn decode_chunks(fixture: &[u8], chunk_size: usize) -> Result<Vec<CanonicalEvent>, GatewayError> {
    let mut decoder = GrokOfficialResponsesStreamDecoder::new();
    let mut events = Vec::new();
    for chunk in fixture.chunks(chunk_size) {
        events.extend(decoder.push_bytes(chunk)?);
    }
    decoder.finish()?;
    Ok(events)
}

fn assert_success_shape(events: &[CanonicalEvent]) {
    assert!(matches!(
        events.first(),
        Some(CanonicalEvent::ResponseStart(_))
    ));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, CanonicalEvent::TextDelta(_)))
    );
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::ResponseEnd(_))
    ));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, CanonicalEvent::StreamError(_)))
    );
}

fn successful_stream() -> Vec<u8> {
    concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-p8-stream\"}}\n\n",
        "event: response.in_progress\n",
        "data: {\"type\":\"response.in_progress\",\"response\":{\"id\":\"resp-p8-stream\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"msg-p8-stream\",\"type\":\"message\",\"role\":\"assistant\"}}\n\n",
        "event: response.content_part.added\n",
        "data: {\"type\":\"response.content_part.added\",\"item_id\":\"msg-p8-stream\",\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg-p8-stream\",\"delta\":\"rea\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg-p8-stream\",\"delta\":\"dy\"}\n\n",
        "event: response.output_text.done\n",
        "data: {\"type\":\"response.output_text.done\",\"item_id\":\"msg-p8-stream\",\"text\":\"ready\"}\n\n",
        "event: response.content_part.done\n",
        "data: {\"type\":\"response.content_part.done\",\"item_id\":\"msg-p8-stream\",\"part\":{\"type\":\"output_text\",\"text\":\"ready\"}}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"msg-p8-stream\",\"type\":\"message\",\"role\":\"assistant\",\"status\":\"completed\",\"content\":[{\"type\":\"output_text\",\"text\":\"ready\"}]}}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-p8-stream\",\"status\":\"completed\",\"output\":[{\"id\":\"msg-p8-stream\"}],\"usage\":{\"input_tokens\":4,\"output_tokens\":1}}}\n\n",
        "event: done\n",
        "data: [DONE]\n\n",
    )
    .as_bytes()
    .to_vec()
}

fn response_created_record() -> Vec<u8> {
    b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-p8-error\"}}\n\n".to_vec()
}

struct FixtureResponse {
    status: u16,
    content_type: GrokOfficialResponseContentType,
    chunks: Vec<Vec<u8>>,
    rate_limit: GrokOfficialRateLimitMetadata,
}

impl FixtureResponse {
    fn new(
        status: u16,
        content_type: GrokOfficialResponseContentType,
        chunks: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            status,
            content_type,
            chunks,
            rate_limit: GrokOfficialRateLimitMetadata::default(),
        }
    }

    fn json(body: &[u8]) -> Self {
        Self::new(
            200,
            GrokOfficialResponseContentType::Json,
            vec![body.to_vec()],
        )
    }

    fn sse(chunks: Vec<Vec<u8>>) -> Self {
        Self::new(200, GrokOfficialResponseContentType::EventStream, chunks)
    }

    fn with_rate_limit_metadata(mut self, rate_limit: GrokOfficialRateLimitMetadata) -> Self {
        self.rate_limit = rate_limit;
        self
    }

    fn into_transport_response(self) -> GrokOfficialTransportResponse {
        GrokOfficialTransportResponse::new(
            self.status,
            self.content_type,
            Box::new(FixtureBody {
                chunks: self.chunks.into(),
            }),
        )
        .with_rate_limit_metadata(self.rate_limit)
    }
}

struct ScriptedTransport {
    responses: Mutex<VecDeque<FixtureResponse>>,
    sends: AtomicUsize,
}

impl ScriptedTransport {
    fn new(responses: impl IntoIterator<Item = FixtureResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
            sends: AtomicUsize::new(0),
        }
    }

    fn send_count(&self) -> usize {
        self.sends.load(Ordering::SeqCst)
    }
}

impl GrokOfficialTransport for ScriptedTransport {
    fn send(
        &self,
        _request: GrokOfficialResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokOfficialTransportResponse, GatewayError>> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        let response = self
            .responses
            .lock()
            .map_err(|_| internal_error())
            .and_then(|mut responses| responses.pop_front().ok_or_else(internal_error));
        Box::pin(async move { response.map(FixtureResponse::into_transport_response) })
    }
}

struct FixtureBody {
    chunks: VecDeque<Vec<u8>>,
}

impl GrokOfficialResponseBody for FixtureBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move { Ok(self.chunks.pop_front()) })
    }
}

#[derive(Clone, Copy)]
struct StaticPublicResolver;

impl EgressDnsResolver for StaticPublicResolver {
    fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
    }
}

fn policy() -> Result<EgressPolicy, Box<dyn Error>> {
    Ok(EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("p8-official-responses-egress")?,
        name: "P8 Official Responses test policy".to_owned(),
        allowed_schemes: std::collections::BTreeSet::from([EgressScheme::Https]),
        allowed_hosts: std::collections::BTreeSet::from([EgressHost::try_new("api.x.ai")?]),
        allowed_ports: std::collections::BTreeSet::from([443]),
        allowed_cidrs: std::collections::BTreeSet::from([EgressCidr::try_new(
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            32,
        )?]),
        redirect_policy: RedirectPolicy::Deny,
    })?)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}
