//! P7 native Kiro `InferenceAdapter` vertical-slice evidence without network access.

#![deny(unsafe_code)]

use std::{
    collections::VecDeque,
    error::Error,
    sync::{Arc, Mutex},
};

use gateway_core::{
    CanonicalEvent, CanonicalRequest, CanonicalResponse, ErrorScope, GatewayError,
    GatewayErrorCode, RequestContext, RequestId,
};
use gateway_provider::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};
use provider_kiro::{
    conversation_request::{KiroConversationContext, KiroConversationId, KiroEnvironmentState},
    credential::KiroCredential,
    endpoint_policy::{KiroApiRegion, KiroEndpointKind, KiroEndpointPolicy},
    failure_classification::KiroFailureSignal,
    inference::{
        KiroInferenceAdapter, KiroOutboundRequest, KiroResponseBody, KiroResponseContentType,
        KiroTransport, KiroTransportResponse,
    },
    profile_arn::{KiroEnterpriseProfileLookup, KiroProfileArnError, resolve_profile_arn},
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[tokio::test]
async fn ide_adapter_executes_native_eventstream_through_the_provider_trait() -> TestResult {
    let wire = [
        event_frame("assistantResponseEvent", &json!({"content":"ready"}))?,
        event_frame(
            "reasoningContentEvent",
            &json!({"reasoningContent":"because"}),
        )?,
        event_frame(
            "toolUseEvent",
            &json!({"toolUseId":"plan-1","name":"EnterPlanMode","stop":true}),
        )?,
    ]
    .concat();
    let transport = Arc::new(FixtureTransport::new([FixtureResponse::ok(
        wire.chunks(7).map(ToOwned::to_owned).collect(),
    )]));
    let adapter = adapter(
        KiroEndpointKind::Ide,
        social_credential()?,
        transport.clone(),
    )?;

    let events = collect(adapter.execute(context()?, request(false)?).await?).await?;

    CanonicalResponse::try_new(events.clone())?;
    assert!(matches!(
        events.first(),
        Some(CanonicalEvent::ResponseStart(_))
    ));
    assert!(events.iter().any(
        |event| matches!(event, CanonicalEvent::ReasoningDelta(delta) if delta.text == "because")
    ));
    assert!(events.iter().any(
        |event| matches!(event, CanonicalEvent::ToolCallEnd(end) if end.call_id == "plan-1" && end.arguments.get() == "{}")
    ));
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::ResponseEnd(_))
    ));
    assert_eq!(adapter.provider_id().as_str(), "kiro");
    assert_eq!(transport.call_count(), 1);

    let observed = transport.observed()?;
    assert_eq!(
        observed.url,
        "https://q.us-east-1.amazonaws.com/generateAssistantResponse"
    );
    assert_eq!(observed.origin.as_deref(), Some("AI_EDITOR"));
    assert_eq!(
        observed.accept.as_deref(),
        Some("application/vnd.amazon.eventstream")
    );
    assert!(observed.authorization_is_bearer);
    assert!(observed.body["profileArn"].as_str().is_some());
    assert_eq!(
        observed.body["conversationState"]["currentMessage"]["userInputMessage"]["content"],
        "Reply with exactly: ready"
    );
    Ok(())
}

#[tokio::test]
async fn adapter_preserves_a_preclassified_quota_signal_without_retaining_an_error_body()
-> TestResult {
    let transport = Arc::new(FixtureTransport::new([FixtureResponse::with_signal(
        429,
        KiroResponseContentType::OtherOrMissing,
        KiroFailureSignal::QuotaExhausted,
        Vec::new(),
    )]));
    let adapter = adapter(KiroEndpointKind::Ide, social_credential()?, transport)?;

    let error = adapter
        .execute(context()?, request(false)?)
        .await
        .err()
        .ok_or("quota fixture unexpectedly started")?;

    assert_eq!(error.code(), GatewayErrorCode::CredentialQuotaExceeded);
    assert_eq!(error.scope(), ErrorScope::QuotaWindow);
    Ok(())
}

#[tokio::test]
async fn cli_adapter_keeps_cli_thinking_and_api_key_marker_before_safe_403() -> TestResult {
    let transport = Arc::new(FixtureTransport::new([FixtureResponse::new(
        403,
        KiroResponseContentType::OtherOrMissing,
        Vec::new(),
    )]));
    let adapter = adapter(
        KiroEndpointKind::Cli,
        api_key_credential()?,
        transport.clone(),
    )?;

    let error = adapter
        .execute(context()?, request(true)?)
        .await
        .err()
        .ok_or("403 fixture unexpectedly started")?;

    assert_eq!(error.code(), GatewayErrorCode::EgressRejected);
    assert_eq!(error.scope(), ErrorScope::Egress);
    assert_eq!(transport.call_count(), 1);
    let observed = transport.observed()?;
    assert_eq!(observed.url, "https://runtime.us-east-1.kiro.dev/");
    assert_eq!(observed.origin.as_deref(), Some("KIRO_CLI"));
    assert_eq!(
        observed.target.as_deref(),
        Some("AmazonCodeWhispererStreamingService.GenerateAssistantResponse")
    );
    assert_eq!(observed.token_type.as_deref(), Some("API_KEY"));
    assert_eq!(observed.body.get("profileArn"), None);
    assert_eq!(
        observed.body["conversationState"]["currentMessage"]["userInputMessage"]["userInputMessageContext"]
            ["outputConfig"]["effort"],
        "high"
    );
    Ok(())
}

#[tokio::test]
async fn post_start_transport_truncation_is_one_terminal_stream_error() -> TestResult {
    let valid = event_frame("assistantResponseEvent", &json!({"content":"partial"}))?;
    let transport = Arc::new(FixtureTransport::new([FixtureResponse::ok(vec![
        valid,
        vec![0, 1, 2],
    ])]));
    let adapter = adapter(KiroEndpointKind::Ide, social_credential()?, transport)?;

    let events = collect(adapter.execute(context()?, request(false)?).await?).await?;

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
fn adapter_rejects_a_profile_resolution_from_the_wrong_credential_family() -> TestResult {
    let policy =
        KiroEndpointPolicy::try_new(KiroEndpointKind::Ide, KiroApiRegion::try_new("us-east-1")?)?;
    let social = social_credential()?;
    let mismatched_profile =
        resolve_profile_arn(social.kind(), policy.api_region(), &UnusedProfileLookup);
    let result = KiroInferenceAdapter::try_new(
        api_key_credential()?,
        policy,
        KiroConversationContext::new(
            KiroConversationId::try_new("p7-09-mismatch")?,
            KiroEnvironmentState::try_new("linux", "/workspace/p7-09")?,
        ),
        "selected-kiro-model",
        mismatched_profile,
        Arc::new(FixtureTransport::new([])),
    );
    assert!(matches!(
        result,
        Err(error)
            if error.code() == GatewayErrorCode::InternalError
                && error.scope() == ErrorScope::Internal
    ));
    Ok(())
}

fn adapter(
    kind: KiroEndpointKind,
    credential: KiroCredential,
    transport: Arc<dyn KiroTransport>,
) -> Result<KiroInferenceAdapter, Box<dyn Error>> {
    let policy = KiroEndpointPolicy::try_new(kind, KiroApiRegion::try_new("us-east-1")?)?;
    let profile = resolve_profile_arn(credential.kind(), policy.api_region(), &UnusedProfileLookup);
    Ok(KiroInferenceAdapter::try_new(
        credential,
        policy,
        KiroConversationContext::new(
            KiroConversationId::try_new("p7-09-conversation")?,
            KiroEnvironmentState::try_new("linux", "/workspace/p7-09")?,
        ),
        "selected-kiro-model",
        profile,
        transport,
    )?)
}

fn social_credential() -> Result<KiroCredential, provider_kiro::credential::KiroCredentialError> {
    KiroCredential::import_json(
        br#"{
            "kind":"social",
            "access_token":"synthetic_kiro_access_012345",
            "refresh_token":"synthetic_kiro_refresh_012345",
            "expires_at_ms":31536000000
        }"#,
        0,
    )
}

fn api_key_credential() -> Result<KiroCredential, provider_kiro::credential::KiroCredentialError> {
    KiroCredential::import_json(br#"{"kind":"api_key","api_key":"ksk_fixture_nonlive"}"#, 0)
}

fn request(thinking: bool) -> Result<CanonicalRequest, serde_json::Error> {
    let mut request = json!({
        "requested_model":"public-alias-never-forwarded",
        "messages":[{
            "role":"user",
            "content":[{"text":{"text":"Reply with exactly: ready","extensions":{}}}],
            "extensions":{}
        }],
        "extensions":{}
    });
    if thinking {
        request["thinking"] = json!({"effort":"high","extensions":{}});
    }
    serde_json::from_value(request)
}

fn context() -> Result<RequestContext, gateway_core::InvalidIdentifier> {
    Ok(RequestContext::new(RequestId::try_new("p7-09-request")?))
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

struct UnusedProfileLookup;

impl KiroEnterpriseProfileLookup for UnusedProfileLookup {
    fn lookup(&self, _region: &KiroApiRegion) -> Result<String, KiroProfileArnError> {
        Err(KiroProfileArnError::InvalidProfileArn)
    }
}

struct FixtureResponse {
    status: u16,
    content_type: KiroResponseContentType,
    failure_signal: KiroFailureSignal,
    chunks: Vec<Vec<u8>>,
}

impl FixtureResponse {
    fn new(status: u16, content_type: KiroResponseContentType, chunks: Vec<Vec<u8>>) -> Self {
        Self::with_signal(status, content_type, KiroFailureSignal::None, chunks)
    }

    fn with_signal(
        status: u16,
        content_type: KiroResponseContentType,
        failure_signal: KiroFailureSignal,
        chunks: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            status,
            content_type,
            failure_signal,
            chunks,
        }
    }

    fn ok(chunks: Vec<Vec<u8>>) -> Self {
        Self::new(200, KiroResponseContentType::EventStream, chunks)
    }

    fn into_transport_response(self) -> KiroTransportResponse {
        KiroTransportResponse::new(
            self.status,
            self.content_type,
            self.failure_signal,
            Box::new(FixtureBody {
                chunks: self.chunks.into(),
            }),
        )
    }
}

struct FixtureTransport {
    responses: Mutex<VecDeque<FixtureResponse>>,
    calls: Mutex<u8>,
    observed: Mutex<Option<ObservedRequest>>,
}

impl FixtureTransport {
    fn new(responses: impl IntoIterator<Item = FixtureResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
            calls: Mutex::new(0),
            observed: Mutex::new(None),
        }
    }

    fn call_count(&self) -> u8 {
        self.calls.lock().map_or(0, |calls| *calls)
    }

    fn observed(&self) -> Result<ObservedRequest, Box<dyn Error>> {
        self.observed
            .lock()
            .map_err(|_| "fixture observation lock poisoned")?
            .clone()
            .ok_or_else(|| "fixture did not observe an outbound request".into())
    }
}

impl KiroTransport for FixtureTransport {
    fn send(
        &self,
        request: KiroOutboundRequest,
    ) -> ProviderFuture<'_, Result<KiroTransportResponse, GatewayError>> {
        let observed = ObservedRequest::from_request(&request);
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
        if let Ok(mut slot) = self.observed.lock() {
            *slot = observed.ok();
        }
        Box::pin(async move { response.map(FixtureResponse::into_transport_response) })
    }
}

#[derive(Clone)]
struct ObservedRequest {
    url: String,
    origin: Option<String>,
    accept: Option<String>,
    target: Option<String>,
    token_type: Option<String>,
    authorization_is_bearer: bool,
    body: Value,
}

impl ObservedRequest {
    fn from_request(request: &KiroOutboundRequest) -> Result<Self, serde_json::Error> {
        Ok(Self {
            url: request.url().to_owned(),
            origin: request.header("origin").map(ToOwned::to_owned),
            accept: request.header("accept").map(ToOwned::to_owned),
            target: request.header("x-amz-target").map(ToOwned::to_owned),
            token_type: request.header("tokentype").map(ToOwned::to_owned),
            authorization_is_bearer: request
                .header("authorization")
                .is_some_and(|value| value.starts_with("Bearer ")),
            body: serde_json::from_slice(request.body())?,
        })
    }
}

struct FixtureBody {
    chunks: VecDeque<Vec<u8>>,
}

impl KiroResponseBody for FixtureBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move { Ok(self.chunks.pop_front()) })
    }
}

fn event_frame(event_type: &str, payload: &Value) -> Result<Vec<u8>, Box<dyn Error>> {
    let payload = serde_json::to_vec(payload)?;
    wire_frame(
        &[
            string_header(":message-type", "event")?,
            string_header(":event-type", event_type)?,
        ],
        &payload,
    )
}

fn wire_frame(headers: &[Vec<u8>], payload: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let headers = headers.concat();
    let total_length = 12 + headers.len() + payload.len() + 4;
    let mut wire = Vec::with_capacity(total_length);
    wire.extend_from_slice(&u32::try_from(total_length)?.to_be_bytes());
    wire.extend_from_slice(&u32::try_from(headers.len())?.to_be_bytes());
    wire.extend_from_slice(&crc32(&wire).to_be_bytes());
    wire.extend_from_slice(&headers);
    wire.extend_from_slice(payload);
    wire.extend_from_slice(&crc32(&wire).to_be_bytes());
    Ok(wire)
}

fn string_header(name: &str, value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut header = Vec::with_capacity(2 + name.len() + value.len());
    header.push(u8::try_from(name.len())?);
    header.extend_from_slice(name.as_bytes());
    header.push(7);
    header.extend_from_slice(&u16::try_from(value.len())?.to_be_bytes());
    header.extend_from_slice(value.as_bytes());
    Ok(header)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}
