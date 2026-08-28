//! P8-06 synthetic Grok Official differential, concurrent-load, and failure-matrix evidence.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, VecDeque},
    error::Error,
    sync::{Arc, Barrier, Mutex},
    thread,
};

use gateway_core::{
    CanonicalEvent, CanonicalRequest, CredentialId, EndpointId, ErrorScope, GatewayError,
    GatewayErrorCode, RequestContext, RequestId,
};
use gateway_provider::{CanonicalEventSource, InferenceAdapter, ProviderFuture};
use gateway_router::{QuotaConfidence, QuotaSource, RuntimeQuotaRegistry, RuntimeQuotaTarget};
use protocol_openai_responses::decode_request;
use provider_grok::{
    GrokOfficialApiKey, GrokOfficialExecutionMode, GrokOfficialFailureAction,
    GrokOfficialInferenceAdapter, GrokOfficialRateLimitMetadata, GrokOfficialResponseBody,
    GrokOfficialResponseContentType, GrokOfficialResponsesDecoder,
    GrokOfficialResponsesOutboundRequest, GrokOfficialResponsesStreamDecoder,
    GrokOfficialRuntimeState, GrokOfficialTransport, GrokOfficialTransportResponse,
};

type TestResult = Result<(), Box<dyn Error>>;

const SYNTHETIC_KEY: &str = "synthetic-official-differential-key-012345";
const OBSERVED_AT_MS: i64 = 90_000;

#[tokio::test]
async fn completed_and_sse_adapters_have_one_tool_reasoning_semantic_projection() -> TestResult {
    let expected = projection(
        &GrokOfficialResponsesDecoder::decode_non_streaming(non_streaming_fixture())?.into_events(),
    );
    assert_projection(&expected);

    let non_streaming = adapter(
        GrokOfficialExecutionMode::NonStreaming,
        FixtureTransport::json(non_streaming_fixture().to_vec()),
    )?;
    assert_eq!(
        projection(&collect(non_streaming.execute(context()?, request()?).await?).await?),
        expected
    );

    let stream = stream_fixture();
    for chunk_size in [1, 2, 9, 31, 257] {
        let adapter = adapter(
            GrokOfficialExecutionMode::Streaming,
            FixtureTransport::sse(stream.chunks(chunk_size).map(ToOwned::to_owned).collect()),
        )?;
        let events = collect(adapter.execute(context()?, request()?).await?).await?;
        assert_eq!(projection(&events), expected, "chunk size {chunk_size}");
    }
    Ok(())
}

#[test]
fn ninety_six_concurrent_sse_decoders_remain_chunk_and_state_isolated() -> TestResult {
    const WORKERS: usize = 12;
    const DECODERS_PER_WORKER: usize = 8;
    let expected = projection(
        &GrokOfficialResponsesDecoder::decode_non_streaming(non_streaming_fixture())?.into_events(),
    );
    let stream = stream_fixture().to_vec();
    let chunk_sizes = [1, 2, 3, 7, 19, 64, 257];
    let start = Arc::new(Barrier::new(WORKERS));

    thread::scope(|scope| -> TestResult {
        let mut workers = Vec::with_capacity(WORKERS);
        for worker_index in 0..WORKERS {
            let expected = expected.clone();
            let stream = stream.clone();
            let start = Arc::clone(&start);
            workers.push(scope.spawn(move || -> Result<(), GatewayError> {
                // Each worker is an OS thread.  The barrier ensures all twelve are ready before
                // the 96 decoder executions begin, so this is a concurrency/isolation check
                // rather than task interleaving on Tokio's single-thread test runtime.
                start.wait();
                for decoder_index in 0..DECODERS_PER_WORKER {
                    let index = worker_index * DECODERS_PER_WORKER + decoder_index;
                    let chunk_size = chunk_sizes[index % chunk_sizes.len()];
                    let mut decoder = GrokOfficialResponsesStreamDecoder::new();
                    let mut events = Vec::new();
                    for chunk in stream.chunks(chunk_size) {
                        events.extend(decoder.push_bytes(chunk)?);
                    }
                    decoder.finish()?;
                    if projection(&events) != expected {
                        return Err(internal_error());
                    }
                }
                Ok(())
            }));
        }
        for worker in workers {
            worker
                .join()
                .map_err(|_| "concurrent decoder worker panicked")??;
        }
        Ok(())
    })?;
    Ok(())
}

#[test]
fn official_failure_matrix_is_exact_target_only_and_preserves_other_bindings() -> TestResult {
    let registry = Arc::new(RuntimeQuotaRegistry::new());
    let official_endpoint = EndpointId::try_new("official-differential-endpoint")?;
    let official_credential = CredentialId::try_new("official-differential-credential")?;
    let official = GrokOfficialRuntimeState::try_new(
        official_endpoint.clone(),
        official_credential.clone(),
        Arc::clone(&registry),
    )?;
    let official_target =
        RuntimeQuotaTarget::endpoint_credential(official_endpoint, official_credential);
    let build_target = RuntimeQuotaTarget::endpoint_credential(
        EndpointId::try_new("build-differential-endpoint")?,
        CredentialId::try_new("build-differential-credential")?,
    );
    let empty = GrokOfficialRateLimitMetadata::default();

    for (status, code, scope, action) in [
        (
            401,
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
            GrokOfficialFailureAction::RequireCredentialReplacement,
        ),
        (
            403,
            GatewayErrorCode::EgressRejected,
            ErrorScope::Egress,
            GrokOfficialFailureAction::None,
        ),
        (
            408,
            GatewayErrorCode::ProviderTransient,
            ErrorScope::Provider,
            GrokOfficialFailureAction::CoolOfficialEndpoint,
        ),
        (
            500,
            GatewayErrorCode::ProviderTransient,
            ErrorScope::Provider,
            GrokOfficialFailureAction::CoolOfficialEndpoint,
        ),
        (
            418,
            GatewayErrorCode::ProviderPermanent,
            ErrorScope::Provider,
            GrokOfficialFailureAction::None,
        ),
    ] {
        let disposition = official.observe_http_failure(status, &empty, OBSERVED_AT_MS)?;
        assert_eq!(disposition.error().code(), code);
        assert_eq!(disposition.error().scope(), scope);
        assert_eq!(disposition.action(), action);
        assert!(registry.snapshot(&official_target)?.is_none());
        assert!(registry.snapshot(&build_target)?.is_none());
    }

    let fallback = official.observe_http_failure(429, &empty, OBSERVED_AT_MS)?;
    assert_eq!(
        fallback.action(),
        GrokOfficialFailureAction::RecordExactQuota
    );
    let fallback_snapshot = registry
        .snapshot(&official_target)?
        .ok_or("Official 429 fallback did not cool the Official binding")?;
    assert_eq!(fallback_snapshot.source(), QuotaSource::Estimated);
    assert_eq!(fallback_snapshot.confidence(), QuotaConfidence::Estimated);
    assert_eq!(
        fallback_snapshot.blocking_reset_at_ms(),
        Some(OBSERVED_AT_MS + 30_000)
    );
    assert!(registry.snapshot(&build_target)?.is_none());

    let retry_after = GrokOfficialRateLimitMetadata::parse([("retry-after", "2")])?;
    official.observe_http_failure(429, &retry_after, OBSERVED_AT_MS + 1)?;
    let header_snapshot = registry
        .snapshot(&official_target)?
        .ok_or("Official retry-after did not replace the older exact observation")?;
    assert_eq!(header_snapshot.source(), QuotaSource::Header);
    assert_eq!(header_snapshot.confidence(), QuotaConfidence::Observed);
    assert_eq!(
        header_snapshot.blocking_reset_at_ms(),
        Some(OBSERVED_AT_MS + 2_001)
    );
    assert!(registry.snapshot(&build_target)?.is_none());
    Ok(())
}

fn adapter(
    mode: GrokOfficialExecutionMode,
    transport: Arc<dyn GrokOfficialTransport>,
) -> Result<GrokOfficialInferenceAdapter, GatewayError> {
    GrokOfficialInferenceAdapter::try_new(
        GrokOfficialApiKey::try_new(SYNTHETIC_KEY)?,
        "grok-official-differential-model",
        mode,
        transport,
    )
}

fn request() -> Result<CanonicalRequest, Box<dyn Error>> {
    Ok(decode_request(r#"{"model":"gateway-official","input":"ready"}"#)?.request)
}

fn context() -> Result<RequestContext, gateway_core::InvalidIdentifier> {
    Ok(RequestContext::new(RequestId::try_new("p8-06-request")?))
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct SemanticProjection {
    reasoning: String,
    text: String,
    calls: Vec<(String, String)>,
    reasoning_tokens: Option<u64>,
}

fn projection(events: &[CanonicalEvent]) -> SemanticProjection {
    let mut reasoning = String::new();
    let mut text = String::new();
    let mut names = BTreeMap::new();
    let mut calls = Vec::new();
    let mut reasoning_tokens = None;
    for event in events {
        match event {
            CanonicalEvent::ReasoningDelta(delta) => reasoning.push_str(&delta.text),
            CanonicalEvent::TextDelta(delta) => text.push_str(&delta.text),
            CanonicalEvent::ToolCallStart(start) => {
                names.insert(start.call_id.clone(), start.name.clone());
            }
            CanonicalEvent::ToolCallEnd(end) => {
                if let Some(name) = names.get(&end.call_id) {
                    calls.push((name.clone(), end.arguments.get().to_owned()));
                }
            }
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
    SemanticProjection {
        reasoning,
        text,
        calls,
        reasoning_tokens,
    }
}

fn assert_projection(projection: &SemanticProjection) {
    assert_eq!(projection.reasoning, "considered");
    assert_eq!(projection.text, "warm");
    assert_eq!(
        projection.calls,
        vec![(
            "lookup_weather".to_owned(),
            r#"{"city":"Shanghai"}"#.to_owned()
        )]
    );
    assert_eq!(projection.reasoning_tokens, Some(1));
}

fn non_streaming_fixture() -> &'static [u8] {
    br#"{
        "id":"resp-p8-06",
        "status":"completed",
        "output":[
            {"id":"reason-p8-06","type":"reasoning","status":"completed","content":[{"type":"reasoning_text","text":"considered"}]},
            {"id":"fc-p8-06","type":"function_call","call_id":"call-p8-06","name":"lookup_weather","arguments":"{\"city\":\"Shanghai\"}","status":"completed"},
            {"id":"msg-p8-06","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"warm"}]}
        ],
        "usage":{"input_tokens":4,"output_tokens":2,"output_tokens_details":{"reasoning_tokens":1}}
    }"#
}

fn stream_fixture() -> &'static [u8] {
    concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-p8-06\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"reason-p8-06\",\"type\":\"reasoning\"}}\n\n",
        "event: response.reasoning_text.delta\n",
        "data: {\"type\":\"response.reasoning_text.delta\",\"item_id\":\"reason-p8-06\",\"delta\":\"considered\"}\n\n",
        "event: response.reasoning.done\n",
        "data: {\"type\":\"response.reasoning.done\",\"item_id\":\"reason-p8-06\",\"text\":\"considered\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"reason-p8-06\",\"type\":\"reasoning\",\"status\":\"completed\",\"content\":[{\"type\":\"reasoning_text\",\"text\":\"considered\"}]}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc-p8-06\",\"type\":\"function_call\",\"call_id\":\"call-p8-06\",\"name\":\"lookup_weather\"}}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc-p8-06\",\"call_id\":\"call-p8-06\",\"delta\":\"{\\\"city\\\":\"}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc-p8-06\",\"call_id\":\"call-p8-06\",\"delta\":\"\\\"Shanghai\\\"}\"}\n\n",
        "event: response.function_call_arguments.done\n",
        "data: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc-p8-06\",\"call_id\":\"call-p8-06\",\"arguments\":\"{\\\"city\\\":\\\"Shanghai\\\"}\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"fc-p8-06\",\"type\":\"function_call\",\"call_id\":\"call-p8-06\",\"name\":\"lookup_weather\",\"arguments\":\"{\\\"city\\\":\\\"Shanghai\\\"}\",\"status\":\"completed\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"msg-p8-06\",\"type\":\"message\",\"role\":\"assistant\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg-p8-06\",\"delta\":\"warm\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"id\":\"msg-p8-06\",\"type\":\"message\",\"role\":\"assistant\",\"status\":\"completed\",\"content\":[{\"type\":\"output_text\",\"text\":\"warm\"}]}}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-p8-06\",\"status\":\"completed\",\"output\":[{\"id\":\"reason-p8-06\"},{\"id\":\"fc-p8-06\"},{\"id\":\"msg-p8-06\"}],\"usage\":{\"input_tokens\":4,\"output_tokens\":2,\"output_tokens_details\":{\"reasoning_tokens\":1}}}}\n\n",
        "event: done\n",
        "data: [DONE]\n\n",
    )
    .as_bytes()
}

struct FixtureTransport {
    response: Mutex<Option<GrokOfficialTransportResponse>>,
}

impl FixtureTransport {
    fn json(body: Vec<u8>) -> Arc<Self> {
        Arc::new(Self::new(GrokOfficialResponseContentType::Json, vec![body]))
    }

    fn sse(chunks: Vec<Vec<u8>>) -> Arc<Self> {
        Arc::new(Self::new(
            GrokOfficialResponseContentType::EventStream,
            chunks,
        ))
    }

    fn new(content_type: GrokOfficialResponseContentType, chunks: Vec<Vec<u8>>) -> Self {
        Self {
            response: Mutex::new(Some(GrokOfficialTransportResponse::new(
                200,
                content_type,
                Box::new(FixtureBody {
                    chunks: chunks.into(),
                }),
            ))),
        }
    }
}

impl GrokOfficialTransport for FixtureTransport {
    fn send(
        &self,
        _request: GrokOfficialResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokOfficialTransportResponse, GatewayError>> {
        let response = self
            .response
            .lock()
            .map_err(|_| internal_error())
            .and_then(|mut response| response.take().ok_or_else(internal_error));
        Box::pin(async move { response })
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

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}
