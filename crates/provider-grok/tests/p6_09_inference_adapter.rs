//! P6 executable Provider and Router vertical-slice evidence without external credentials.

#![deny(unsafe_code)]

use std::{
    collections::VecDeque,
    error::Error,
    future,
    sync::{Arc, Mutex},
};

use gateway_core::{
    CanonicalEvent, CanonicalRequest, ClientKeyId, ErrorScope, GatewayError, GatewayErrorCode,
    RequestContext, RequestId, TransparentRetryGate, TransparentRetryGateFuture,
};
use gateway_provider::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};
use gateway_router::{
    ProtocolFormat, ResponsesExecution, ResponsesExecutor, ResponsesResponseMode,
    RoutedProviderResponsesExecutor, project_protocol_response,
};
use protocol_openai_responses::decode_request;
use provider_grok::{
    GrokBuildCacheIdentityDeriver, GrokBuildCredential, GrokBuildExecutionMode,
    GrokBuildInferenceAdapter, GrokBuildResponseBody, GrokBuildResponseContentEncoding,
    GrokBuildResponseContentType, GrokBuildResponsesOutboundRequest, GrokBuildTransport,
    GrokBuildTransportResponse,
};

type TestResult = Result<(), Box<dyn Error>>;

#[tokio::test]
async fn non_streaming_fixture_executes_through_the_real_provider_adapter() -> TestResult {
    let transport = Arc::new(FixtureTransport::new([FixtureResponse::ok_json(
        include_bytes!("../../../tests/fixtures/grok-build/p6-03-non-streaming.json"),
    )]));
    let adapter = adapter(GrokBuildExecutionMode::NonStreaming, transport.clone())?;

    let events = collect(adapter.execute(context()?, request()?).await?).await?;

    assert_success_shape(&events);
    let response = gateway_core::CanonicalResponse::try_new(events)?;
    for protocol in [
        ProtocolFormat::OpenAiChatCompletions,
        ProtocolFormat::OpenAiResponses,
        ProtocolFormat::AnthropicMessages,
    ] {
        project_protocol_response(&response, protocol)?;
    }
    assert_eq!(adapter.provider_id().as_str(), "grok.build");
    assert_eq!(transport.call_count(), 1);
    Ok(())
}

#[tokio::test]
async fn streaming_fixture_survives_arbitrary_chunk_boundaries() -> TestResult {
    let mut fixture =
        include_bytes!("../../../tests/fixtures/grok-build/p6-03-stream.sse").to_vec();
    fixture.push(b'\n');
    let chunks = fixture
        .chunks(19)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let transport = Arc::new(FixtureTransport::new([FixtureResponse::ok_sse(chunks)]));
    let adapter = adapter(GrokBuildExecutionMode::Streaming, transport.clone())?;

    let events = collect(adapter.execute(context()?, request()?).await?).await?;

    assert_success_shape(&events);
    assert_all_protocols_project(&events)?;
    assert_eq!(transport.call_count(), 1);
    Ok(())
}

#[tokio::test]
async fn explicitly_requested_reasoning_remains_in_the_canonical_response() -> TestResult {
    let transport = Arc::new(FixtureTransport::new([FixtureResponse::ok_json(
        include_bytes!("../../../tests/fixtures/grok-build/p6-03-non-streaming.json"),
    )]));
    let adapter = adapter(GrokBuildExecutionMode::NonStreaming, transport.clone())?;

    let events = collect(
        adapter
            .execute(context()?, request_with_thinking()?)
            .await?,
    )
    .await?;

    assert_success_shape(&events);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, CanonicalEvent::ReasoningDelta(_)))
    );
    assert_eq!(transport.call_count(), 1);
    Ok(())
}

#[tokio::test]
async fn prompt_cache_key_is_tenant_derived_before_build_transport() -> TestResult {
    let transport = Arc::new(FixtureTransport::new([FixtureResponse::ok_json(
        include_bytes!("../../../tests/fixtures/grok-build/p6-03-non-streaming.json"),
    )]));
    let deriver = Arc::new(GrokBuildCacheIdentityDeriver::new([0x39; 32]));
    let adapter = adapter(GrokBuildExecutionMode::NonStreaming, transport.clone())?
        .with_cache_identity_deriver(Arc::clone(&deriver));
    let client_key_id = ClientKeyId::try_new("p13-responses-cache-client")?;
    let raw_cache_key = "pi-session-cache-key";
    let mut request = request()?;
    request.prompt_cache_key = Some(raw_cache_key.to_owned());

    collect(
        adapter
            .execute(
                context()?.with_client_key_id(client_key_id.clone()),
                request,
            )
            .await?,
    )
    .await?;

    let body = transport
        .request_body()
        .ok_or("request body was not captured")?;
    let body: serde_json::Value = serde_json::from_slice(&body)?;
    let expected = deriver.derive(&client_key_id, "grok-4.5-build", raw_cache_key)?;
    assert_eq!(
        body.get("prompt_cache_key")
            .and_then(serde_json::Value::as_str),
        Some(expected.as_str())
    );
    assert!(!serde_json::to_string(&body)?.contains(raw_cache_key));
    assert_eq!(transport.call_count(), 1);
    Ok(())
}

#[tokio::test]
async fn error_envelope_uses_the_existing_p6_failure_classifier() -> TestResult {
    let transport = Arc::new(FixtureTransport::new([FixtureResponse::new(
        429,
        GrokBuildResponseContentType::Json,
        GrokBuildResponseContentEncoding::Identity,
        vec![include_bytes!("../../../tests/fixtures/grok-build/p6-03-http-error.json").to_vec()],
    )]));
    let adapter = adapter(GrokBuildExecutionMode::NonStreaming, transport.clone())?;

    let error = adapter
        .execute(context()?, request()?)
        .await
        .err()
        .ok_or("error fixture unexpectedly started")?;

    assert_eq!(error.code(), GatewayErrorCode::CredentialQuotaExceeded);
    assert_eq!(error.scope(), ErrorScope::QuotaWindow);
    assert_eq!(transport.call_count(), 1);
    Ok(())
}

#[tokio::test]
async fn router_selects_the_explicit_provider_mode_without_concrete_provider_coupling() -> TestResult
{
    let non_streaming = Arc::new(FixtureTransport::new([FixtureResponse::ok_json(
        include_bytes!("../../../tests/fixtures/grok-build/p6-03-non-streaming.json"),
    )]));
    let streaming = Arc::new(FixtureTransport::new([FixtureResponse::ok_sse(
        terminated_stream_fixture()
            .chunks(41)
            .map(ToOwned::to_owned)
            .collect(),
    )]));
    let executor = RoutedProviderResponsesExecutor::new(
        Arc::new(adapter(
            GrokBuildExecutionMode::NonStreaming,
            non_streaming.clone(),
        )?),
        Arc::new(adapter(
            GrokBuildExecutionMode::Streaming,
            streaming.clone(),
        )?),
    );

    let execution = ResponsesExecution::new(
        context()?,
        request()?,
        None,
        ResponsesResponseMode::Streaming,
        Arc::new(NeverCancelled),
    );
    let events = collect_router(executor.execute_routed(execution).await?).await?;

    assert_success_shape(&events);
    assert_eq!(non_streaming.call_count(), 0);
    assert_eq!(streaming.call_count(), 1);
    Ok(())
}

fn adapter(
    mode: GrokBuildExecutionMode,
    transport: Arc<dyn GrokBuildTransport>,
) -> Result<GrokBuildInferenceAdapter, Box<dyn Error>> {
    Ok(GrokBuildInferenceAdapter::try_new(
        credential()?,
        "grok-4.5-build",
        mode,
        transport,
    )?)
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

fn request() -> Result<CanonicalRequest, Box<dyn Error>> {
    Ok(decode_request(
        r#"{"model":"gateway-build","input":"Reply with exactly: ready","max_output_tokens":32}"#,
    )?
    .request)
}

fn request_with_thinking() -> Result<CanonicalRequest, Box<dyn Error>> {
    Ok(decode_request(
        r#"{"model":"gateway-build","input":"Reply with exactly: ready","max_output_tokens":32,"reasoning":{"effort":"medium"}}"#,
    )?
    .request)
}

fn context() -> Result<RequestContext, Box<dyn Error>> {
    Ok(RequestContext::new(RequestId::try_new("p6-09-request")?))
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

async fn collect_router(
    mut source: Box<dyn gateway_router::ResponsesEventSource>,
) -> Result<Vec<CanonicalEvent>, GatewayError> {
    let mut events = Vec::new();
    while let Some(event) = source.next_event().await? {
        events.push(event);
    }
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
    assert!(
        matches!(events.last(), Some(CanonicalEvent::ResponseEnd(_))),
        "unexpected terminal events: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, CanonicalEvent::StreamError(_)))
    );
}

fn assert_all_protocols_project(events: &[CanonicalEvent]) -> TestResult {
    let response = gateway_core::CanonicalResponse::try_new(events.to_vec())?;
    for protocol in [
        ProtocolFormat::OpenAiChatCompletions,
        ProtocolFormat::OpenAiResponses,
        ProtocolFormat::AnthropicMessages,
    ] {
        project_protocol_response(&response, protocol)?;
    }
    Ok(())
}

fn terminated_stream_fixture() -> Vec<u8> {
    let mut fixture =
        include_bytes!("../../../tests/fixtures/grok-build/p6-03-stream.sse").to_vec();
    fixture.push(b'\n');
    fixture
}

struct FixtureResponse {
    status: u16,
    content_type: GrokBuildResponseContentType,
    content_encoding: GrokBuildResponseContentEncoding,
    chunks: Vec<Vec<u8>>,
}

impl FixtureResponse {
    fn new(
        status: u16,
        content_type: GrokBuildResponseContentType,
        content_encoding: GrokBuildResponseContentEncoding,
        chunks: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            status,
            content_type,
            content_encoding,
            chunks,
        }
    }

    fn ok_json(body: &[u8]) -> Self {
        Self::new(
            200,
            GrokBuildResponseContentType::Json,
            GrokBuildResponseContentEncoding::Identity,
            vec![body.to_vec()],
        )
    }

    fn ok_sse(chunks: Vec<Vec<u8>>) -> Self {
        Self::new(
            200,
            GrokBuildResponseContentType::EventStream,
            GrokBuildResponseContentEncoding::Identity,
            chunks,
        )
    }

    fn into_transport_response(self) -> GrokBuildTransportResponse {
        GrokBuildTransportResponse::new(
            self.status,
            self.content_type,
            self.content_encoding,
            Box::new(FixtureBody {
                chunks: self.chunks.into(),
            }),
        )
    }
}

struct FixtureTransport {
    responses: Mutex<VecDeque<FixtureResponse>>,
    calls: Mutex<u8>,
    request_body: Mutex<Option<Vec<u8>>>,
}

impl FixtureTransport {
    fn new(responses: impl IntoIterator<Item = FixtureResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
            calls: Mutex::new(0),
            request_body: Mutex::new(None),
        }
    }

    fn call_count(&self) -> u8 {
        self.calls.lock().map_or(0, |calls| *calls)
    }

    fn request_body(&self) -> Option<Vec<u8>> {
        self.request_body.lock().ok().and_then(|body| body.clone())
    }
}

impl GrokBuildTransport for FixtureTransport {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        if let Ok(mut body) = self.request_body.lock() {
            *body = Some(request.body().to_vec());
        }
        let response = self
            .responses
            .lock()
            .map_err(|_| GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal))
            .and_then(|mut responses| {
                responses.pop_front().ok_or_else(|| {
                    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
                })
            });
        if let Ok(mut calls) = self.calls.lock() {
            *calls = calls.saturating_add(1);
        }
        Box::pin(async move { response.map(FixtureResponse::into_transport_response) })
    }
}

struct FixtureBody {
    chunks: VecDeque<Vec<u8>>,
}

impl GrokBuildResponseBody for FixtureBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move { Ok(self.chunks.pop_front()) })
    }
}

struct NeverCancelled;

impl TransparentRetryGate for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }

    fn allows_transparent_retry(&self) -> bool {
        true
    }

    fn cancelled(&self) -> TransparentRetryGateFuture<'_> {
        Box::pin(future::pending())
    }
}
