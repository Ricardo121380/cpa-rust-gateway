//! P3-09 composition tests for the bounded `OpenAI` Responses aggregation slice.
//!
//! The test owns two loopback-only, deterministic HTTP peers. It exercises the same typed
//! request builder, P2 egress admission/client pool, P3 scheduler/orchestrator, Snapshot HTTP
//! boundary, bounded stream, and P3-08 event port used by the individual component tests.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    io,
    net::{IpAddr, Ipv4Addr},
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use actix_web::{
    App,
    http::{StatusCode, header},
    test, web,
};
use gateway_auth::client_key::{ClientKeyPepper, ClientKeyService};
use gateway_core::{
    AccessGroupId, AttemptOutcome, AttemptRetryDecision, CanonicalEvent, CanonicalRequest,
    ClientKeyId, CredentialId, EgressPolicyId, EndpointId, ErrorScope, GatewayError,
    GatewayErrorCode, GatewayEvent, GatewayEventSink, MessageEnd, MessageRole, MessageStart,
    PublicModelId, RawExtensions, RequestContext, ResponseEnd, ResponseId, ResponseStart,
    RouteCandidateId, RouteId, StreamError, TextDelta, UpstreamId, Usage, UsageDelta,
};
use gateway_http_actix::{
    ResponsesHttpState, ResponsesMetadataFactory, configure, default_stream_capacity,
};
use gateway_observability::{BoundedEventQueue, EventQueueConfig};
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
    CredentialLease, CredentialSecret, EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost,
    EgressPolicy, EgressPolicyInput, EgressScheme, EndpointCredentialInput, EndpointCredentialPool,
    EndpointCredentialPools, RedirectPolicy, UpstreamClientPool, UpstreamHttpResponse,
    UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};
use protocol_openai_responses::{OpenAiResponseMetadata, ResponseMode};
use provider_openai_compatible::{
    OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesRequestBuilder,
};
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, Notify},
    task::JoinHandle,
    time,
};

type TestResult = Result<(), Box<dyn Error>>;
type BuiltState = (ResponsesHttpState, String, Arc<Mutex<Vec<String>>>);

const LOOPBACK_ADDRESS: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const PUBLIC_MODEL: &str = "minimax-m3";
const MODEL_ALIAS: &str = "minimax-m3-alias";
const ROUTE_ID: &str = "p3-09-route";
const REQUEST_ID: &str = "p3-09-request";
const MAX_HTTP_REQUEST_BYTES: usize = 64 * 1024;
const MAX_UPSTREAM_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_SSE_FRAME_BYTES: usize = 16 * 1024;

#[derive(Debug)]
struct FixedMetadata;

impl ResponsesMetadataFactory for FixedMetadata {
    fn request_context(&self) -> Result<RequestContext, GatewayError> {
        let request_id =
            gateway_core::RequestId::try_new(REQUEST_ID).map_err(|_| internal_error())?;
        Ok(RequestContext::new(request_id))
    }

    fn response_metadata(
        &self,
        public_model: &str,
    ) -> Result<OpenAiResponseMetadata, GatewayError> {
        OpenAiResponseMetadata::try_new(public_model, 1)
    }
}

#[derive(Clone, Copy)]
struct StaticLoopbackResolver;

impl EgressDnsResolver for StaticLoopbackResolver {
    fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![LOOPBACK_ADDRESS])
    }
}

#[derive(Clone)]
struct MockEndpoint {
    label: String,
    host: String,
    port: u16,
}

#[derive(Clone)]
enum MockBehavior {
    JsonSuccess { text: String },
    Status { code: u16 },
    StreamingStall(StreamingCancellation),
}

#[derive(Clone)]
struct StreamingCancellation {
    closed: Arc<AtomicBool>,
    closed_notify: Arc<Notify>,
}

impl StreamingCancellation {
    async fn wait_for_close(&self) -> bool {
        if self.closed.load(Ordering::Acquire) {
            return true;
        }
        time::timeout(Duration::from_secs(1), self.closed_notify.notified())
            .await
            .is_ok()
    }
}

struct MockUpstream {
    endpoint: MockEndpoint,
    models: Arc<Mutex<Vec<String>>>,
    requests: Arc<AtomicUsize>,
    task: JoinHandle<()>,
    streaming_cancellation: Option<StreamingCancellation>,
}

impl MockUpstream {
    async fn json_success(label: &str, text: &str) -> Result<Self, io::Error> {
        Self::spawn(
            label,
            MockBehavior::JsonSuccess {
                text: text.to_owned(),
            },
        )
        .await
    }

    async fn status(label: &str, code: u16) -> Result<Self, io::Error> {
        Self::spawn(label, MockBehavior::Status { code }).await
    }

    async fn streaming_stall(label: &str) -> Result<Self, io::Error> {
        let cancellation = StreamingCancellation {
            closed: Arc::new(AtomicBool::new(false)),
            closed_notify: Arc::new(Notify::new()),
        };
        let mut upstream =
            Self::spawn(label, MockBehavior::StreamingStall(cancellation.clone())).await?;
        upstream.streaming_cancellation = Some(cancellation);
        Ok(upstream)
    }

    async fn spawn(label: &str, behavior: MockBehavior) -> Result<Self, io::Error> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let endpoint = MockEndpoint {
            label: label.to_owned(),
            host: format!("p3-09-{label}.test"),
            port,
        };
        let models = Arc::new(Mutex::new(Vec::new()));
        let requests = Arc::new(AtomicUsize::new(0));
        let task_models = Arc::clone(&models);
        let task_requests = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            loop {
                let accepted = listener.accept().await;
                let Ok((socket, _address)) = accepted else {
                    return;
                };
                let models = Arc::clone(&task_models);
                let requests = Arc::clone(&task_requests);
                let behavior = behavior.clone();
                let _connection = tokio::spawn(async move {
                    let _result = serve_mock_connection(socket, behavior, models, requests).await;
                });
            }
        });

        Ok(Self {
            endpoint,
            models,
            requests,
            task,
            streaming_cancellation: None,
        })
    }

    fn endpoint(&self) -> MockEndpoint {
        self.endpoint.clone()
    }

    fn request_count(&self) -> usize {
        self.requests.load(Ordering::Acquire)
    }

    async fn received_models(&self) -> Vec<String> {
        self.models.lock().await.clone()
    }

    async fn wait_for_stream_close(&self) -> bool {
        match &self.streaming_cancellation {
            Some(cancellation) => cancellation.wait_for_close().await,
            None => false,
        }
    }
}

impl Drop for MockUpstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve_mock_connection(
    mut socket: TcpStream,
    behavior: MockBehavior,
    models: Arc<Mutex<Vec<String>>>,
    requests: Arc<AtomicUsize>,
) -> Result<(), io::Error> {
    let model = read_request_model(&mut socket).await?;
    requests.fetch_add(1, Ordering::AcqRel);
    models.lock().await.push(model);

    match behavior {
        MockBehavior::JsonSuccess { text } => write_json_success(&mut socket, &text).await,
        MockBehavior::Status { code } => write_status(&mut socket, code).await,
        MockBehavior::StreamingStall(cancellation) => {
            write_stream_start_and_wait_for_close(&mut socket, cancellation).await
        }
    }
}

async fn read_request_model(socket: &mut TcpStream) -> Result<String, io::Error> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(end) = find_header_end(&bytes) {
            break end;
        }
        let mut buffer = [0_u8; 1024];
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "mock upstream peer closed before request headers",
            ));
        }
        if bytes.len().saturating_add(read) > MAX_HTTP_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "mock upstream request exceeds the bounded test limit",
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    };
    let header = std::str::from_utf8(&bytes[..header_end]).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "mock upstream request headers are not UTF-8",
        )
    })?;
    if !header.starts_with("POST /v1/responses ") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mock upstream received an unexpected request target",
        ));
    }
    let content_length = request_content_length(header)?;
    let total_length = header_end.checked_add(content_length).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "mock upstream request length overflowed",
        )
    })?;
    if total_length > MAX_HTTP_REQUEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mock upstream request body exceeds the bounded test limit",
        ));
    }
    while bytes.len() < total_length {
        let mut buffer = [0_u8; 1024];
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "mock upstream peer closed before request body",
            ));
        }
        if bytes.len().saturating_add(read) > MAX_HTTP_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "mock upstream request exceeds the bounded test limit",
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    let value: Value = serde_json::from_slice(&bytes[header_end..total_length]).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "mock upstream request body is not JSON",
        )
    })?;
    value
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "mock upstream request has no model",
            )
        })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

fn request_content_length(header: &str) -> Result<usize, io::Error> {
    header
        .lines()
        .skip(1)
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "mock upstream needs content-length",
            )
        })
}

async fn write_json_success(socket: &mut TcpStream, text: &str) -> Result<(), io::Error> {
    let body = serde_json::json!({
        "id": "upstream-response",
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "output_text", "text": text }]
        }],
        "usage": {
            "input_tokens": 2,
            "output_tokens": 3,
            "output_tokens_details": { "reasoning_tokens": 1 }
        }
    })
    .to_string();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(head.as_bytes()).await?;
    socket.write_all(body.as_bytes()).await?;
    socket.flush().await
}

async fn write_status(socket: &mut TcpStream, code: u16) -> Result<(), io::Error> {
    let head =
        format!("HTTP/1.1 {code} Mock Failure\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    socket.write_all(head.as_bytes()).await?;
    socket.flush().await
}

async fn write_stream_start_and_wait_for_close(
    socket: &mut TcpStream,
    cancellation: StreamingCancellation,
) -> Result<(), io::Error> {
    let body = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"upstream-stream-response\"}}\n\n"
    );
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
    socket.write_all(head.as_bytes()).await?;
    socket.write_all(body.as_bytes()).await?;
    socket.flush().await?;

    let mut buffer = [0_u8; 256];
    loop {
        match socket.read(&mut buffer).await {
            Ok(0) | Err(_) => {
                cancellation.closed.store(true, Ordering::Release);
                cancellation.closed_notify.notify_waiters();
                return Ok(());
            }
            Ok(_) => {}
        }
    }
}

struct EndpointRuntime {
    endpoint: OpenAiResponsesEndpoint,
    policy: EgressPolicy,
    transport: UpstreamTransportProfile,
}

struct GatewayE2eExecutor {
    orchestrator: Arc<AttemptOrchestrator>,
    endpoints: Arc<BTreeMap<EndpointId, EndpointRuntime>>,
    client_pool: Arc<UpstreamClientPool>,
    event_sink: Arc<dyn GatewayEventSink>,
    observed_routes: Arc<Mutex<Vec<String>>>,
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
            let driver = MockHttpAttemptDriver {
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

struct MockHttpAttemptDriver {
    request: CanonicalRequest,
    mode: ResponsesResponseMode,
    endpoints: Arc<BTreeMap<EndpointId, EndpointRuntime>>,
    client_pool: Arc<UpstreamClientPool>,
}

impl AttemptDriver for MockHttpAttemptDriver {
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
                .admit_url(outbound.url(), &StaticLoopbackResolver)
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
    response_started: bool,
    message_open: bool,
    finished: bool,
}

impl OpenAiSseEventSource {
    async fn begin(response: UpstreamHttpResponse) -> Result<Self, AttemptFailure> {
        let mut source = Self {
            response,
            buffer: Vec::new(),
            pending: VecDeque::new(),
            response_started: false,
            message_open: false,
            finished: false,
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
        while self.pending.is_empty() && !self.finished {
            if let Some(frame) = self.take_frame() {
                self.consume_frame(&frame)?;
                continue;
            }
            let next = self.response.next_chunk().await?;
            let Some(chunk) = next else {
                return Err(stream_truncated_error());
            };
            if self.buffer.len().saturating_add(chunk.len()) > MAX_SSE_FRAME_BYTES {
                return Err(upstream_protocol_error());
            }
            self.buffer.extend_from_slice(&chunk);
        }
        Ok(())
    }

    fn take_frame(&mut self) -> Option<Vec<u8>> {
        let position = self
            .buffer
            .windows(2)
            .position(|window| window == b"\n\n")?;
        let mut frame: Vec<_> = self.buffer.drain(..position + 2).collect();
        frame.truncate(position);
        Some(frame)
    }

    fn consume_frame(&mut self, frame: &[u8]) -> Result<(), GatewayError> {
        let frame = std::str::from_utf8(frame).map_err(|_| upstream_protocol_error())?;
        let data = frame
            .lines()
            .find_map(|line| line.strip_prefix("data:").map(str::trim))
            .ok_or_else(upstream_protocol_error)?;
        let value: Value = serde_json::from_str(data).map_err(|_| upstream_protocol_error())?;
        let kind = required_string(&value, "type")?;

        match kind.as_str() {
            "response.created" => {
                if self.response_started {
                    return Err(upstream_protocol_error());
                }
                let response = value.get("response").ok_or_else(upstream_protocol_error)?;
                let response_id = required_string(response, "id")?;
                self.response_started = true;
                self.pending
                    .push_back(CanonicalEvent::ResponseStart(ResponseStart {
                        response_id: ResponseId::try_new(response_id)
                            .map_err(|_| upstream_protocol_error())?,
                        extensions: RawExtensions::default(),
                    }));
            }
            "response.in_progress" => {}
            "response.output_item.added" => {
                let item = value.get("item").ok_or_else(upstream_protocol_error)?;
                let is_assistant_message = item.get("type").and_then(Value::as_str)
                    == Some("message")
                    && item.get("role").and_then(Value::as_str) == Some("assistant");
                if !self.response_started || self.message_open || !is_assistant_message {
                    return Err(upstream_protocol_error());
                }
                self.message_open = true;
                self.pending
                    .push_back(CanonicalEvent::MessageStart(MessageStart {
                        role: MessageRole("assistant".to_owned()),
                        extensions: RawExtensions::default(),
                    }));
            }
            "response.output_text.delta" => {
                let delta = required_string(&value, "delta")?;
                if !self.message_open || delta.is_empty() {
                    return Err(upstream_protocol_error());
                }
                self.pending.push_back(CanonicalEvent::TextDelta(TextDelta {
                    text: delta,
                    extensions: RawExtensions::default(),
                }));
            }
            "response.completed" => {
                if !self.response_started || !self.message_open {
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
                self.message_open = false;
                self.finished = true;
            }
            "response.failed" => {
                if !self.response_started {
                    return Err(upstream_protocol_error());
                }
                self.pending
                    .push_back(CanonicalEvent::StreamError(StreamError {
                        error: provider_transient_error(),
                    }));
                self.finished = true;
            }
            _ => return Err(upstream_protocol_error()),
        }
        Ok(())
    }
}

impl ResponsesEventSource for OpenAiSseEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            if self.pending.is_empty() && !self.finished {
                self.read_until_event().await?;
            }
            Ok(self.pending.pop_front())
        })
    }
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

fn build_state(
    endpoints: &[MockEndpoint],
    event_sink: Arc<dyn GatewayEventSink>,
) -> Result<BuiltState, Box<dyn Error>> {
    let route_id = RouteId::try_new(ROUTE_ID)?;
    let public_model_id = PublicModelId::try_new("p3-09-public-model")?;
    let mut candidates = Vec::new();
    let mut endpoint_pools = Vec::new();
    let mut runtimes = BTreeMap::new();

    for (index, endpoint) in endpoints.iter().enumerate() {
        let endpoint_id = EndpointId::try_new(format!("p3-09-endpoint-{}", endpoint.label))?;
        let upstream_model = format!("minimax-m3-upstream-{}", endpoint.label);
        candidates.push(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(format!("p3-09-candidate-{}", endpoint.label))?,
            endpoint_id: endpoint_id.clone(),
            upstream_id: UpstreamId::try_new(format!("p3-09-upstream-{}", endpoint.label))?,
            upstream_model,
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
                credential_id: CredentialId::try_new(format!("p3-09-credential-{index}"))?,
                credential_kind: "api_key".to_owned(),
                credential_revision: 0,
                priority: 0,
                weight: 1,
                concurrency: 1,
                secret: CredentialSecret::try_new(
                    format!("p3-09-synthetic-credential-{index}").into_bytes(),
                )?,
            }],
        )?);
        runtimes.insert(endpoint_id, endpoint_runtime(endpoint, index)?);
    }

    let access_group_id = AccessGroupId::try_new("p3-09-access-group")?;
    let client_key_service = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x5A_u8; 32])?);
    let issued = client_key_service.issue(
        ClientKeyId::try_new("p3-09-client-key")?,
        access_group_id.clone(),
        None,
    )?;
    let (client_key_record, presented_key) = issued.into_parts();
    let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
        SnapshotVersion::try_new("p3-09-snapshot")?,
        vec![SnapshotPublicModel::new(
            public_model_id.clone(),
            PUBLIC_MODEL.to_owned(),
            "P3-09 public model".to_owned(),
            CapabilitySet::empty(),
            route_id.clone(),
        )],
        vec![(MODEL_ALIAS.to_owned(), public_model_id.clone())],
        vec![SnapshotRoute::new(
            route_id.clone(),
            public_model_id,
            SnapshotRoutePolicy::RoundRobin,
            2,
            1_000,
            candidates,
        )],
        vec![SnapshotAccessGroup::new(
            access_group_id,
            "P3-09 Access Group".to_owned(),
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
    let observed_routes = Arc::clone(&executor.observed_routes);
    let authenticator = Arc::new(SnapshotClientKeyAuthenticator::new(
        Arc::new(RouteSnapshotRegistry::new(snapshot)),
        client_key_service,
    ));
    let state = ResponsesHttpState::with_snapshot_metadata_and_event_sink(
        Arc::new(executor),
        Arc::new(FixedMetadata),
        authenticator,
        event_sink,
        default_stream_capacity()?,
    );

    Ok((state, presented_key.as_str().to_owned(), observed_routes))
}

fn endpoint_runtime(
    endpoint: &MockEndpoint,
    index: usize,
) -> Result<EndpointRuntime, Box<dyn Error>> {
    let base_url = format!("http://{}:{}/v1", endpoint.host, endpoint.port);
    let policy = EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new(format!("p3-09-egress-{index}"))?,
        name: "P3-09 loopback test policy".to_owned(),
        allowed_schemes: BTreeSet::from([EgressScheme::Http]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new(&endpoint.host)?]),
        allowed_ports: BTreeSet::from([endpoint.port]),
        allowed_cidrs: BTreeSet::from([EgressCidr::try_new(LOOPBACK_ADDRESS, 32)?]),
        redirect_policy: RedirectPolicy::Deny,
    })?;
    let timeouts = UpstreamTimeouts::try_new(
        Duration::from_millis(250),
        Duration::from_millis(250),
        Duration::from_millis(250),
        Duration::from_secs(1),
    )?;
    Ok(EndpointRuntime {
        endpoint: OpenAiResponsesEndpoint::try_new(&base_url, "/responses")?,
        policy,
        transport: UpstreamTransportProfile::new(timeouts, UpstreamProxy::Direct, non_zero(1)?),
    })
}

fn non_zero(value: usize) -> Result<NonZeroUsize, io::Error> {
    NonZeroUsize::new(value)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "test value must be non-zero"))
}

fn authorized(request: test::TestRequest, presented_key: &str) -> test::TestRequest {
    request.insert_header((header::AUTHORIZATION, format!("Bearer {presented_key}")))
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

#[actix_web::test]
async fn round_robin_reaches_each_controlled_http_upstream() -> TestResult {
    let upstream_a = MockUpstream::json_success("a", "reply from A").await?;
    let upstream_b = MockUpstream::json_success("b", "reply from B").await?;
    let event_sink: Arc<dyn GatewayEventSink> = Arc::new(gateway_core::NoopGatewayEventSink);
    let (state, presented_key, observed_routes) =
        build_state(&[upstream_a.endpoint(), upstream_b.endpoint()], event_sink)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure),
    )
    .await;

    for _ in 0..12 {
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(format!(r#"{{"model":"{MODEL_ALIAS}","input":"hello"}}"#)),
            &presented_key,
        )
        .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""model":"minimax-m3""#));
        assert!(!body.contains(MODEL_ALIAS));
        assert!(!body.contains("minimax-m3-upstream-"));
    }

    assert_eq!(upstream_a.request_count(), 6);
    assert_eq!(upstream_b.request_count(), 6);
    assert_eq!(
        upstream_a.received_models().await,
        vec!["minimax-m3-upstream-a".to_owned(); 6]
    );
    assert_eq!(
        upstream_b.received_models().await,
        vec!["minimax-m3-upstream-b".to_owned(); 6]
    );
    assert_eq!(
        observed_routes.lock().await.as_slice(),
        vec![ROUTE_ID.to_owned(); 12].as_slice()
    );
    Ok(())
}

#[actix_web::test]
async fn pre_semantic_http_5xx_fails_over_to_the_second_upstream() -> TestResult {
    let failing = MockUpstream::status("a", 503).await?;
    let healthy = MockUpstream::json_success("b", "fallback reply").await?;
    let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(8, 1)?)?;
    let queue = Arc::new(queue);
    let event_sink: Arc<dyn GatewayEventSink> = queue.clone();
    let (state, presented_key, _observed_routes) =
        build_state(&[failing.endpoint(), healthy.endpoint()], event_sink)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure),
    )
    .await;

    let request = authorized(
        test::TestRequest::post()
            .uri("/v1/responses")
            .set_payload(format!(
                r#"{{"model":"{MODEL_ALIAS}","input":"retry safely"}}"#
            )),
        &presented_key,
    )
    .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(response).await.to_vec())?;
    assert!(body.contains("fallback reply"));
    assert!(!body.contains("p3-09-synthetic-credential"));
    assert_eq!(failing.request_count(), 1);
    assert_eq!(healthy.request_count(), 1);

    let mut attempts = Vec::new();
    let mut saw_request = false;
    let mut saw_usage = false;
    while let Some(event) = receiver.try_recv() {
        match event {
            GatewayEvent::Request(event) => {
                saw_request = true;
                assert_eq!(event.request_id().as_str(), REQUEST_ID);
                assert_eq!(event.public_model(), PUBLIC_MODEL);
                assert_eq!(event.route_alias(), Some(MODEL_ALIAS));
            }
            GatewayEvent::Attempt(event) => {
                assert_eq!(event.request_id().as_str(), REQUEST_ID);
                attempts.push(event);
            }
            GatewayEvent::Usage(event) => {
                saw_usage = true;
                assert_eq!(event.request_id().as_str(), REQUEST_ID);
                assert_eq!(event.usage().input_tokens, Some(2));
                assert_eq!(event.usage().output_tokens, Some(3));
            }
            GatewayEvent::Diagnostic(_) => {}
        }
    }
    assert!(saw_request);
    assert!(saw_usage);
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].attempt_number(), 1);
    assert_eq!(
        attempts[0].route_candidate_id().as_str(),
        "p3-09-candidate-a"
    );
    assert!(matches!(
        attempts[0].outcome(),
        AttemptOutcome::Failed(error) if error.code() == GatewayErrorCode::ProviderTransient
    ));
    assert_eq!(
        attempts[0].retry_decision(),
        AttemptRetryDecision::RetryEligible
    );
    assert_eq!(attempts[1].attempt_number(), 2);
    assert_eq!(
        attempts[1].route_candidate_id().as_str(),
        "p3-09-candidate-b"
    );
    assert!(matches!(attempts[1].outcome(), AttemptOutcome::Succeeded));
    assert_eq!(
        attempts[1].retry_decision(),
        AttemptRetryDecision::Completed
    );
    Ok(())
}

#[actix_web::test]
async fn dropping_the_gateway_sse_body_closes_the_live_mock_upstream_attempt() -> TestResult {
    let streaming = MockUpstream::streaming_stall("a").await?;
    let unused_fallback = MockUpstream::json_success("b", "must not run").await?;
    let event_sink: Arc<dyn GatewayEventSink> = Arc::new(gateway_core::NoopGatewayEventSink);
    let (state, presented_key, _observed_routes) = build_state(
        &[streaming.endpoint(), unused_fallback.endpoint()],
        event_sink,
    )?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure),
    )
    .await;

    let request = authorized(
        test::TestRequest::post()
            .uri("/v1/responses")
            .set_payload(format!(
                r#"{{"model":"{MODEL_ALIAS}","input":"cancel me","stream":true}}"#
            )),
        &presented_key,
    )
    .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    drop(response);

    assert!(streaming.wait_for_stream_close().await);
    assert_eq!(unused_fallback.request_count(), 0);
    Ok(())
}
