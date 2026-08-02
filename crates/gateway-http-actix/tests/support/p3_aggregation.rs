//! Test-only P3 aggregation composition harness shared by local and authorized-live tests.
//!
//! This module remains below `tests/`, so its concrete upstream/protocol dependencies are never
//! part of the `gateway-http-actix` library target. It owns no listener, configuration file, or
//! ambient credential lookup.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use gateway_auth::client_key::{ClientKeyPepper, ClientKeyService};
use gateway_core::{
    AccessGroupId, CanonicalEvent, CanonicalRequest, ClientKeyId, CredentialId, EndpointId,
    ErrorScope, GatewayError, GatewayErrorCode, GatewayEventSink, MessageEnd, MessageRole,
    MessageStart, PublicModelId, RawExtensions, RequestContext, ResponseEnd, ResponseId,
    ResponseStart, RouteCandidateId, RouteId, StreamError, TextDelta, UpstreamId, Usage,
    UsageDelta,
};
use gateway_http_actix::{ResponsesHttpState, ResponsesMetadataFactory, default_stream_capacity};
use gateway_router::{
    AttemptDriver, AttemptFailure, AttemptFuture, AttemptOrchestrator, CapabilitySet,
    ResponsesEventSource, ResponsesExecution, ResponsesExecutor, ResponsesFuture,
    ResponsesResponseMode, RouteCredentialScheduler, RouteSnapshot, RouteSnapshotInput,
    RouteSnapshotRegistry, SelectedRouteCredential, SnapshotAccessGroup, SnapshotCatalogAdmission,
    SnapshotClientKeyAuthenticator, SnapshotClientKeyView, SnapshotPublicModel, SnapshotRoute,
    SnapshotRouteCandidate, SnapshotRouteCandidateInput, SnapshotRoutePolicy,
    SnapshotTransformMode, SnapshotVersion,
};
use gateway_upstream::{
    CredentialLease, CredentialSecret, EgressDnsResolver, EgressPolicy, EndpointCredentialInput,
    EndpointCredentialPool, EndpointCredentialPools, UpstreamClientPool, UpstreamHttpResponse,
    UpstreamTransportProfile,
};
use protocol_openai_chat::ChatResponseMetadata;
use protocol_openai_responses::{OpenAiResponseMetadata, ResponseMode};
use provider_openai_compatible::{
    OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesRequestBuilder,
};
use serde_json::Value;
use tokio::sync::Mutex;

const MAX_UPSTREAM_RESPONSE_BYTES: usize = 64 * 1024;
// P3-10 relays can emit valid lifecycle events larger than the former 16 KiB fixture limit.
// Keep the stream bounded, aligned with the other P3 test-only response limits.
const MAX_SSE_FRAME_BYTES: usize = MAX_UPSTREAM_RESPONSE_BYTES;

/// One already-admitted test-only Endpoint and its request-scoped Credential input.
pub(crate) struct AggregationEndpoint {
    label: String,
    endpoint: OpenAiResponsesEndpoint,
    upstream_model: String,
    credential: Vec<u8>,
    policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    transport: UpstreamTransportProfile,
}

impl AggregationEndpoint {
    /// Creates one explicit test Endpoint. Callers retain ownership of how its URL and policy are
    /// constructed; this constructor never resolves DNS or starts a request.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        label: String,
        endpoint: OpenAiResponsesEndpoint,
        upstream_model: String,
        credential: Vec<u8>,
        policy: EgressPolicy,
        resolver: Arc<dyn EgressDnsResolver>,
        transport: UpstreamTransportProfile,
    ) -> Self {
        Self {
            label,
            endpoint,
            upstream_model,
            credential,
            policy,
            resolver,
            transport,
        }
    }
}

/// Result of building the immutable P3 aggregation composition for one test target.
pub(crate) struct AggregationHarness {
    state: ResponsesHttpState,
    presented_key: String,
    observed_routes: Arc<Mutex<Vec<String>>>,
}

/// Request-correlation allocation used only by one test harness instance.
#[derive(Clone, Copy)]
#[allow(dead_code)] // Each integration target compiles this shared module separately and uses one mode.
pub(crate) enum RequestIdMode {
    /// Reuse one stable identifier for deterministic fixture assertions.
    Fixed,
    /// Allocate a distinct opaque identifier per test request without rendering it.
    Sequenced,
}

impl AggregationHarness {
    /// Returns a clonable HTTP state for one Actix test application.
    pub(crate) fn state(&self) -> ResponsesHttpState {
        self.state.clone()
    }

    /// Returns the one test-scoped client credential for the in-process Actix boundary.
    pub(crate) fn presented_key(&self) -> &str {
        &self.presented_key
    }

    /// Returns only stable route identities observed at the routed execution seam.
    pub(crate) fn observed_routes(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.observed_routes)
    }
}

/// Builds the fixed P3 aggregation path for exactly two explicit candidates.
///
/// The caller controls each Endpoint's policy and resolver. The harness neither reads an
/// environment variable nor opens a connection until an Actix request reaches its executor.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)] // The one composition seam keeps Snapshot, pool, executor, and HTTP state auditable together.
pub(crate) fn build_aggregation_harness(
    namespace: &str,
    public_model: &str,
    model_alias: &str,
    request_id_mode: RequestIdMode,
    max_attempts: i64,
    bootstrap_timeout: Duration,
    endpoints: Vec<AggregationEndpoint>,
    event_sink: Arc<dyn GatewayEventSink>,
) -> Result<AggregationHarness, Box<dyn Error>> {
    if endpoints.len() != 2 || max_attempts <= 0 || bootstrap_timeout.is_zero() {
        return Err(
            "P3 aggregation harness requires two endpoints, a positive attempt cap, and a positive bootstrap timeout".into(),
        );
    }
    let bootstrap_timeout_ms = i64::try_from(bootstrap_timeout.as_millis())?;
    if bootstrap_timeout_ms <= 0 {
        return Err(
            "P3 aggregation harness bootstrap timeout must be at least one millisecond".into(),
        );
    }

    let route_id = RouteId::try_new(format!("{namespace}-route"))?;
    let public_model_id = PublicModelId::try_new(format!("{namespace}-public-model"))?;
    let mut candidates = Vec::with_capacity(endpoints.len());
    let mut endpoint_pools = Vec::with_capacity(endpoints.len());
    let mut runtimes = BTreeMap::new();

    for (index, endpoint) in endpoints.into_iter().enumerate() {
        let endpoint_id = EndpointId::try_new(format!("{namespace}-endpoint-{}", endpoint.label))?;
        candidates.push(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(format!("{namespace}-candidate-{}", endpoint.label))?,
            endpoint_id: endpoint_id.clone(),
            upstream_id: UpstreamId::try_new(format!("{namespace}-upstream-{}", endpoint.label))?,
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: endpoint.upstream_model,
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::AllowedUnlisted,
            active_binding_count: 1,
        }));
        endpoint_pools.push(EndpointCredentialPool::try_new(
            endpoint_id.clone(),
            [EndpointCredentialInput {
                credential_id: CredentialId::try_new(format!("{namespace}-credential-{index}"))?,
                credential_kind: "api_key".to_owned(),
                credential_revision: 0,
                priority: 0,
                weight: 1,
                concurrency: 1,
                secret: CredentialSecret::try_new(endpoint.credential)?,
            }],
        )?);
        runtimes.insert(
            endpoint_id,
            EndpointRuntime {
                endpoint: endpoint.endpoint,
                policy: endpoint.policy,
                resolver: endpoint.resolver,
                transport: endpoint.transport,
            },
        );
    }

    let access_group_id = AccessGroupId::try_new(format!("{namespace}-access-group"))?;
    let client_key_service = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x5A_u8; 32])?);
    let issued = client_key_service.issue(
        ClientKeyId::try_new(format!("{namespace}-client-key"))?,
        access_group_id.clone(),
        None,
    )?;
    let (client_key_record, presented_key) = issued.into_parts();
    let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
        SnapshotVersion::try_new(format!("{namespace}-snapshot"))?,
        vec![SnapshotPublicModel::new(
            public_model_id.clone(),
            public_model.to_owned(),
            "P3 aggregation test public model".to_owned(),
            CapabilitySet::empty(),
            route_id.clone(),
        )],
        vec![(model_alias.to_owned(), public_model_id.clone())],
        vec![SnapshotRoute::new(
            route_id.clone(),
            public_model_id,
            SnapshotRoutePolicy::RoundRobin,
            max_attempts,
            bootstrap_timeout_ms,
            candidates,
        )],
        vec![SnapshotAccessGroup::new(
            access_group_id,
            "P3 aggregation test access group".to_owned(),
            BTreeSet::from([route_id.clone()]),
        )],
        vec![SnapshotClientKeyView::new(
            client_key_record,
            BTreeSet::from([route_id.clone()]),
        )],
    ))?);
    let scheduler = Arc::new(RouteCredentialScheduler::new(
        Arc::clone(&snapshot),
        Arc::new(EndpointCredentialPools::try_new(endpoint_pools)?),
    ));
    let executor = GatewayE2eExecutor {
        orchestrator: Arc::new(AttemptOrchestrator::new(
            scheduler,
            Arc::new(gateway_router::RuntimeHealthRegistry::new()),
        )),
        endpoints: Arc::new(runtimes),
        client_pool: Arc::new(UpstreamClientPool::new(non_zero(4)?)),
        event_sink: Arc::clone(&event_sink),
        observed_routes: Arc::new(Mutex::new(Vec::new())),
    };
    let observed_routes = executor.observed_routes();
    let authenticator = Arc::new(SnapshotClientKeyAuthenticator::new(
        Arc::new(RouteSnapshotRegistry::new(snapshot)),
        client_key_service,
    ));
    let metadata = Arc::new(HarnessMetadata::try_new(
        format!("{namespace}-request"),
        request_id_mode,
    )?);
    let state = ResponsesHttpState::with_snapshot_metadata_and_event_sink(
        Arc::new(executor),
        metadata,
        authenticator,
        event_sink,
        default_stream_capacity()?,
    );

    Ok(AggregationHarness {
        state,
        presented_key: presented_key.as_str().to_owned(),
        observed_routes,
    })
}

struct HarnessMetadata {
    request_id_prefix: String,
    request_id_mode: RequestIdMode,
    next_request_sequence: AtomicU64,
}

impl HarnessMetadata {
    fn try_new(
        request_id_prefix: String,
        request_id_mode: RequestIdMode,
    ) -> Result<Self, GatewayError> {
        gateway_core::RequestId::try_new(request_id_prefix.clone())
            .map_err(|_| internal_error())?;
        Ok(Self {
            request_id_prefix,
            request_id_mode,
            next_request_sequence: AtomicU64::new(0),
        })
    }
}

impl ResponsesMetadataFactory for HarnessMetadata {
    fn request_context(&self) -> Result<RequestContext, GatewayError> {
        let request_id = match self.request_id_mode {
            RequestIdMode::Fixed => self.request_id_prefix.clone(),
            RequestIdMode::Sequenced => format!(
                "{}-{}",
                self.request_id_prefix,
                self.next_request_sequence.fetch_add(1, Ordering::Relaxed)
            ),
        };
        let request_id =
            gateway_core::RequestId::try_new(request_id).map_err(|_| internal_error())?;
        Ok(RequestContext::new(request_id))
    }

    fn response_metadata(
        &self,
        public_model: &str,
    ) -> Result<OpenAiResponseMetadata, GatewayError> {
        OpenAiResponseMetadata::try_new(public_model, 1)
    }

    fn chat_metadata(
        &self,
        public_model: &str,
        include_usage: bool,
    ) -> Result<ChatResponseMetadata, GatewayError> {
        ChatResponseMetadata::try_new(public_model, 1, include_usage)
    }
}

struct EndpointRuntime {
    endpoint: OpenAiResponsesEndpoint,
    policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    transport: UpstreamTransportProfile,
}

struct GatewayE2eExecutor {
    orchestrator: Arc<AttemptOrchestrator>,
    endpoints: Arc<BTreeMap<EndpointId, EndpointRuntime>>,
    client_pool: Arc<UpstreamClientPool>,
    event_sink: Arc<dyn GatewayEventSink>,
    observed_routes: Arc<Mutex<Vec<String>>>,
}

impl GatewayE2eExecutor {
    fn observed_routes(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.observed_routes)
    }
}

impl ResponsesExecutor for GatewayE2eExecutor {
    fn execute(
        &self,
        _context: RequestContext,
        _request: CanonicalRequest,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        Box::pin(async { Err(route_not_found_error()) })
    }

    fn execute_routed(
        &self,
        execution: ResponsesExecution,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        let orchestrator = Arc::clone(&self.orchestrator);
        let endpoints = Arc::clone(&self.endpoints);
        let client_pool = Arc::clone(&self.client_pool);
        let event_sink = Arc::clone(&self.event_sink);
        let observed_routes = Arc::clone(&self.observed_routes);
        let context = execution.context().clone();
        let request = execution.request().clone();
        let route_id = execution.route_id().cloned();
        let mode = execution.mode();
        let retry_gate = Arc::clone(execution.retry_gate());

        Box::pin(async move {
            let route_id = route_id.ok_or_else(route_not_found_error)?;
            observed_routes
                .lock()
                .await
                .push(route_id.as_str().to_owned());
            let driver = AggregationAttemptDriver {
                request,
                mode,
                endpoints,
                client_pool,
            };
            let started = orchestrator
                .start_with_event_sink(
                    context.request_id(),
                    &route_id,
                    &driver,
                    retry_gate.as_ref(),
                    event_sink.as_ref(),
                )
                .await?;
            let (source, selection) = started.into_parts();
            Ok(Box::new(LeaseHoldingEventSource {
                source,
                _selection: selection,
            }) as Box<dyn ResponsesEventSource>)
        })
    }
}

struct LeaseHoldingEventSource {
    source: Box<dyn ResponsesEventSource>,
    _selection: SelectedRouteCredential,
}

impl ResponsesEventSource for LeaseHoldingEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        self.source.next_event()
    }
}

struct AggregationAttemptDriver {
    request: CanonicalRequest,
    mode: ResponsesResponseMode,
    endpoints: Arc<BTreeMap<EndpointId, EndpointRuntime>>,
    client_pool: Arc<UpstreamClientPool>,
}

impl AttemptDriver for AggregationAttemptDriver {
    type Output = Box<dyn ResponsesEventSource>;

    fn start<'a>(
        &'a self,
        candidate: &'a SnapshotRouteCandidate,
        credential: &'a CredentialLease,
        _bootstrap_timeout: Duration,
    ) -> AttemptFuture<'a, Result<Self::Output, AttemptFailure>> {
        Box::pin(async move {
            let Some(runtime) = self.endpoints.get(candidate.endpoint_id()) else {
                return Err(AttemptFailure::NonRetryable(internal_error()));
            };
            let credential = std::str::from_utf8(credential.secret_bytes())
                .map_err(|_| AttemptFailure::NonRetryable(internal_error()))?;
            let request_credential = OpenAiResponsesApiKey::try_new(credential.to_owned())
                .map_err(AttemptFailure::NonRetryable)?;
            let response_mode = upstream_response_mode(self.mode);
            let outbound = OpenAiResponsesRequestBuilder::build(
                &runtime.endpoint,
                &request_credential,
                candidate.upstream_model(),
                &self.request,
                response_mode,
            )
            .map_err(AttemptFailure::NonRetryable)?;
            let admitted = runtime
                .policy
                .admit_url(outbound.url(), runtime.resolver.as_ref())
                .map_err(|_| AttemptFailure::NonRetryable(egress_rejected_error()))?;
            let request = outbound
                .into_transport_request(admitted)
                .map_err(AttemptFailure::NonRetryable)?;
            let mut response = self
                .client_pool
                .send(request, &runtime.transport)
                .await
                .map_err(|_| AttemptFailure::Connection)?;

            match response.status() {
                200..=299 => {}
                429 => return Err(AttemptFailure::RateLimited { retry_after: None }),
                500..=599 => return Err(AttemptFailure::ServerError),
                _ => return Err(AttemptFailure::NonRetryable(provider_permanent_error())),
            }
            if !has_expected_content_type(&response, self.mode) {
                return Err(AttemptFailure::NonRetryable(upstream_protocol_error()));
            }

            match self.mode {
                ResponsesResponseMode::NonStreaming => {
                    let events = decode_json_response(&mut response).await?;
                    Ok(Box::new(FiniteEventSource::new(events)) as Box<dyn ResponsesEventSource>)
                }
                ResponsesResponseMode::Streaming => {
                    let source = OpenAiSseEventSource::begin(response).await?;
                    Ok(Box::new(source) as Box<dyn ResponsesEventSource>)
                }
            }
        })
    }
}

fn upstream_response_mode(mode: ResponsesResponseMode) -> ResponseMode {
    match mode {
        ResponsesResponseMode::NonStreaming => ResponseMode::NonStreaming,
        ResponsesResponseMode::Streaming => ResponseMode::Streaming,
    }
}

fn has_expected_content_type(response: &UpstreamHttpResponse, mode: ResponsesResponseMode) -> bool {
    let expected = match mode {
        ResponsesResponseMode::NonStreaming => "application/json",
        ResponsesResponseMode::Streaming => "text/event-stream",
    };
    response
        .header("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|content_type| content_type.starts_with(expected))
}

struct FiniteEventSource {
    events: VecDeque<CanonicalEvent>,
}

impl FiniteEventSource {
    fn new(events: Vec<CanonicalEvent>) -> Self {
        Self {
            events: events.into(),
        }
    }
}

impl ResponsesEventSource for FiniteEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move { Ok(self.events.pop_front()) })
    }
}

async fn decode_json_response(
    response: &mut UpstreamHttpResponse,
) -> Result<Vec<CanonicalEvent>, AttemptFailure> {
    let mut body = Vec::new();
    loop {
        let next = response
            .next_chunk()
            .await
            .map_err(|_| AttemptFailure::Connection)?;
        let Some(chunk) = next else {
            break;
        };
        if body.len().saturating_add(chunk.len()) > MAX_UPSTREAM_RESPONSE_BYTES {
            return Err(AttemptFailure::NonRetryable(upstream_protocol_error()));
        }
        body.extend_from_slice(&chunk);
    }
    decode_json_events(&body).map_err(|_| AttemptFailure::BootstrapTruncated)
}

fn decode_json_events(body: &[u8]) -> Result<Vec<CanonicalEvent>, GatewayError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| upstream_protocol_error())?;
    let response_id = required_string(&value, "id")?;
    if value.get("status").and_then(Value::as_str) != Some("completed") {
        return Err(upstream_protocol_error());
    }
    let output = value
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(upstream_protocol_error)?;
    let message = output.iter().find(|item| {
        item.get("type").and_then(Value::as_str) == Some("message")
            && item.get("role").and_then(Value::as_str) == Some("assistant")
    });
    let Some(message) = message else {
        return Err(upstream_protocol_error());
    };
    let content = message
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(upstream_protocol_error)?;
    let text = content.iter().find_map(|part| {
        (part.get("type").and_then(Value::as_str) == Some("output_text"))
            .then(|| part.get("text").and_then(Value::as_str))
            .flatten()
    });
    let Some(text) = text.filter(|text| !text.is_empty()) else {
        return Err(upstream_protocol_error());
    };

    let mut events = vec![
        CanonicalEvent::ResponseStart(ResponseStart {
            response_id: ResponseId::try_new(response_id).map_err(|_| upstream_protocol_error())?,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::TextDelta(TextDelta {
            text: text.to_owned(),
            extensions: RawExtensions::default(),
        }),
    ];
    if let Some(usage) = decode_usage(value.get("usage"))? {
        events.push(CanonicalEvent::UsageDelta(UsageDelta {
            usage,
            is_final: true,
            extensions: RawExtensions::default(),
        }));
    }
    events.push(CanonicalEvent::MessageEnd(MessageEnd::default()));
    events.push(CanonicalEvent::ResponseEnd(ResponseEnd::default()));
    Ok(events)
}

struct OpenAiSseEventSource {
    response: UpstreamHttpResponse,
    buffer: Vec<u8>,
    pending: VecDeque<CanonicalEvent>,
    lifecycle: SseLifecycle,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SseLifecycle {
    AwaitingResponseStart,
    AwaitingMessageStart,
    Streaming { saw_text: bool },
    Finished,
}

impl SseLifecycle {
    const fn is_finished(self) -> bool {
        matches!(self, Self::Finished)
    }
}

impl OpenAiSseEventSource {
    async fn begin(response: UpstreamHttpResponse) -> Result<Self, AttemptFailure> {
        let mut source = Self {
            response,
            buffer: Vec::new(),
            pending: VecDeque::new(),
            lifecycle: SseLifecycle::AwaitingResponseStart,
        };
        source
            .read_until_event()
            .await
            .map_err(|_| AttemptFailure::BootstrapTruncated)?;
        if !matches!(
            source.pending.front(),
            Some(CanonicalEvent::ResponseStart(_))
        ) {
            return Err(AttemptFailure::BootstrapTruncated);
        }
        Ok(source)
    }

    async fn read_until_event(&mut self) -> Result<(), GatewayError> {
        while self.pending.is_empty() && !self.lifecycle.is_finished() {
            if let Some(frame) = self.take_frame() {
                self.consume_frame(&frame)?;
                continue;
            }
            let next = self.response.next_chunk().await?;
            let Some(chunk) = next else {
                return Err(stream_truncated_error());
            };
            append_sse_chunk(&mut self.buffer, &chunk)?;
        }
        Ok(())
    }

    fn take_frame(&mut self) -> Option<Vec<u8>> {
        let lf_position = self
            .buffer
            .windows(2)
            .position(|window| window == b"\n\n")
            .map(|position| (position, 2));
        let crlf_position = self
            .buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|position| (position, 4));
        let (position, delimiter_length) = match (lf_position, crlf_position) {
            (Some(left), Some(right)) => {
                if left.0 <= right.0 {
                    left
                } else {
                    right
                }
            }
            (Some(position), None) | (None, Some(position)) => position,
            (None, None) => return None,
        };
        let mut frame: Vec<_> = self.buffer.drain(..position + delimiter_length).collect();
        frame.truncate(position);
        Some(frame)
    }

    fn consume_frame(&mut self, frame: &[u8]) -> Result<(), GatewayError> {
        let frame = std::str::from_utf8(frame).map_err(|_| upstream_protocol_error())?;
        let data = frame
            .lines()
            .filter_map(|line| line.strip_prefix("data:").map(str::trim))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            return Err(upstream_protocol_error());
        }
        let value: Value = serde_json::from_str(&data).map_err(|_| upstream_protocol_error())?;
        let kind = required_string(&value, "type")?;

        match kind.as_str() {
            "response.created" => {
                if self.lifecycle != SseLifecycle::AwaitingResponseStart {
                    return Err(upstream_protocol_error());
                }
                let response = value.get("response").ok_or_else(upstream_protocol_error)?;
                let response_id = required_string(response, "id")?;
                self.lifecycle = SseLifecycle::AwaitingMessageStart;
                self.pending
                    .push_back(CanonicalEvent::ResponseStart(ResponseStart {
                        response_id: ResponseId::try_new(response_id)
                            .map_err(|_| upstream_protocol_error())?,
                        extensions: RawExtensions::default(),
                    }));
            }
            "response.in_progress"
            | "response.content_part.added"
            | "response.content_part.done"
            | "response.output_text.done"
            | "response.output_item.done" => {}
            "response.output_item.added" => {
                let item = value.get("item").ok_or_else(upstream_protocol_error)?;
                let is_assistant_message = item.get("type").and_then(Value::as_str)
                    == Some("message")
                    && item.get("role").and_then(Value::as_str) == Some("assistant");
                if self.lifecycle != SseLifecycle::AwaitingMessageStart || !is_assistant_message {
                    return Err(upstream_protocol_error());
                }
                self.lifecycle = SseLifecycle::Streaming { saw_text: false };
                self.pending
                    .push_back(CanonicalEvent::MessageStart(MessageStart {
                        role: MessageRole("assistant".to_owned()),
                        extensions: RawExtensions::default(),
                    }));
            }
            "response.output_text.delta" => {
                let delta = required_string(&value, "delta")?;
                let SseLifecycle::Streaming { .. } = self.lifecycle else {
                    return Err(upstream_protocol_error());
                };
                if delta.is_empty() {
                    return Err(upstream_protocol_error());
                }
                self.lifecycle = SseLifecycle::Streaming { saw_text: true };
                self.pending.push_back(CanonicalEvent::TextDelta(TextDelta {
                    text: delta,
                    extensions: RawExtensions::default(),
                }));
            }
            "response.completed" => {
                if self.lifecycle != (SseLifecycle::Streaming { saw_text: true }) {
                    return Err(upstream_protocol_error());
                }
                let response = value.get("response").ok_or_else(upstream_protocol_error)?;
                if let Some(usage) = decode_usage(response.get("usage"))? {
                    self.pending
                        .push_back(CanonicalEvent::UsageDelta(UsageDelta {
                            usage,
                            is_final: true,
                            extensions: RawExtensions::default(),
                        }));
                }
                self.pending
                    .push_back(CanonicalEvent::MessageEnd(MessageEnd::default()));
                self.pending
                    .push_back(CanonicalEvent::ResponseEnd(ResponseEnd::default()));
                self.lifecycle = SseLifecycle::Finished;
            }
            "response.failed" => {
                if self.lifecycle == SseLifecycle::AwaitingResponseStart {
                    return Err(upstream_protocol_error());
                }
                self.pending
                    .push_back(CanonicalEvent::StreamError(StreamError {
                        error: provider_transient_error(),
                    }));
                self.lifecycle = SseLifecycle::Finished;
            }
            _ => return Err(upstream_protocol_error()),
        }
        Ok(())
    }
}

impl ResponsesEventSource for OpenAiSseEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            if self.pending.is_empty() && !self.lifecycle.is_finished() {
                self.read_until_event().await?;
            }
            Ok(self.pending.pop_front())
        })
    }
}

fn append_sse_chunk(buffer: &mut Vec<u8>, chunk: &[u8]) -> Result<(), GatewayError> {
    if buffer.len().saturating_add(chunk.len()) > MAX_SSE_FRAME_BYTES {
        return Err(upstream_protocol_error());
    }
    buffer.extend_from_slice(chunk);
    Ok(())
}

fn required_string(value: &Value, field: &str) -> Result<String, GatewayError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(upstream_protocol_error)
}

fn decode_usage(value: Option<&Value>) -> Result<Option<Usage>, GatewayError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let object = value.as_object().ok_or_else(upstream_protocol_error)?;
    let reasoning_tokens = object
        .get("output_tokens_details")
        .and_then(Value::as_object)
        .and_then(|details| details.get("reasoning_tokens"))
        .map(required_u64)
        .transpose()?;
    Ok(Some(Usage {
        input_tokens: object.get("input_tokens").map(required_u64).transpose()?,
        output_tokens: object.get("output_tokens").map(required_u64).transpose()?,
        reasoning_tokens,
        ..Usage::default()
    }))
}

fn required_u64(value: &Value) -> Result<u64, GatewayError> {
    value.as_u64().ok_or_else(upstream_protocol_error)
}

fn non_zero(value: usize) -> Result<NonZeroUsize, std::io::Error> {
    NonZeroUsize::new(value).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "test value must be non-zero",
        )
    })
}

fn route_not_found_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::RouteNotFound, ErrorScope::Model)
}

fn egress_rejected_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
}

fn provider_permanent_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ProviderPermanent, ErrorScope::Provider)
}

fn provider_transient_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider)
}

fn upstream_protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

fn stream_truncated_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::StreamTruncated, ErrorScope::Stream)
}

fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use super::{MAX_SSE_FRAME_BYTES, append_sse_chunk};
    use gateway_core::GatewayErrorCode;

    #[test]
    fn sse_frame_buffer_accepts_the_new_finite_limit() {
        let mut buffer = vec![b'x'; MAX_SSE_FRAME_BYTES - 1];

        assert!(append_sse_chunk(&mut buffer, b"y").is_ok());

        assert_eq!(buffer.len(), MAX_SSE_FRAME_BYTES);
    }

    #[test]
    fn sse_frame_buffer_rejects_data_above_the_new_finite_limit_without_appending_it() {
        let mut buffer = vec![b'x'; MAX_SSE_FRAME_BYTES];

        assert!(matches!(
            append_sse_chunk(&mut buffer, b"y"),
            Err(error) if error.code() == GatewayErrorCode::UpstreamProtocolError
        ));
        assert_eq!(buffer.len(), MAX_SSE_FRAME_BYTES);
    }
}
