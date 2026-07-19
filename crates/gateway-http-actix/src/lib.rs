//! `Actix Web` transport shell. Core crates must not depend on this crate.
//!
//! P1 exposes a deliberately small vertical slice: public `GET /healthz` and Client Key-protected
//! `POST /v1/responses`. Request bytes are decoded by the protocol adapter rather than Actix's
//! JSON extractor so duplicate JSON member names remain observable and rejectable.

#![deny(unsafe_code)]

use std::{
    collections::VecDeque,
    convert::Infallible,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll},
    time::{SystemTime, UNIX_EPOCH},
};

use actix_web::{
    HttpRequest, HttpResponse,
    body::{BodySize, MessageBody},
    http::{StatusCode, header},
    web,
};
use futures_util::{Stream, stream};
use gateway_auth::ClientKeyAuthenticator;
use gateway_core::{
    CanonicalEvent, CanonicalResponse, ErrorScope, GatewayError, GatewayErrorCode, RequestContext,
    RequestId, StreamError,
};
use gateway_router::{ResponsesEventSource, ResponsesExecutor};
use gateway_stream::{
    CanonicalEventSender, CanonicalEventStream, FirstSemanticEventTracker, StreamCancellation,
    StreamCapacity, StreamCapacityError, bounded_canonical_stream,
};
use protocol_openai_responses::{
    OpenAiResponseMetadata, OpenAiResponsesSseEncoder, ResponseMode, SseFrame, decode_request,
    encode_error, encode_response,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-http-actix";

/// The P1 default maximum number of canonical events buffered between source and HTTP body.
pub const DEFAULT_STREAM_CAPACITY: usize = 8;

/// Creates request context and response metadata without making HTTP handlers depend on a clock
/// or identifier implementation.
///
/// Tests can inject deterministic values. The system implementation uses a process-local opaque
/// sequence for request IDs and wall-clock Unix seconds for public response metadata.
pub trait ResponsesMetadataFactory: Send + Sync {
    /// Creates correlation context for one accepted request.
    ///
    /// # Errors
    ///
    /// Returns a safe internal error if this implementation cannot allocate a valid context.
    fn request_context(&self) -> Result<RequestContext, GatewayError>;

    /// Creates public Responses metadata for the selected client-visible model label.
    ///
    /// # Errors
    ///
    /// Returns a safe error when the implementation cannot supply a valid public model or clock
    /// value.
    fn response_metadata(&self, public_model: &str)
    -> Result<OpenAiResponseMetadata, GatewayError>;
}

/// The production-default metadata implementation for P1's local vertical slice.
#[derive(Debug, Default)]
pub struct SystemResponsesMetadataFactory {
    next_request_sequence: AtomicU64,
}

impl SystemResponsesMetadataFactory {
    /// Creates a metadata factory whose first request identifier has sequence zero.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next_request_sequence: AtomicU64::new(0),
        }
    }
}

impl ResponsesMetadataFactory for SystemResponsesMetadataFactory {
    fn request_context(&self) -> Result<RequestContext, GatewayError> {
        let sequence = self.next_request_sequence.fetch_add(1, Ordering::Relaxed);
        let request_id =
            RequestId::try_new(format!("p1-request-{sequence}")).map_err(|_| internal_error())?;

        Ok(RequestContext::new(request_id))
    }

    fn response_metadata(
        &self,
        public_model: &str,
    ) -> Result<OpenAiResponseMetadata, GatewayError> {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| internal_error())?
            .as_secs();

        OpenAiResponseMetadata::try_new(public_model, created_at)
    }
}

/// Shared Actix application state for P1 Responses endpoints.
///
/// An authenticator is mandatory so the public Responses route cannot accidentally retain P1-07's
/// unauthenticated behavior. Health remains independent of this state-owned authentication path.
#[derive(Clone)]
pub struct ResponsesHttpState {
    executor: Arc<dyn ResponsesExecutor>,
    authenticator: Arc<dyn ClientKeyAuthenticator>,
    metadata_factory: Arc<dyn ResponsesMetadataFactory>,
    stream_capacity: StreamCapacity,
}

impl ResponsesHttpState {
    /// Creates P1 HTTP state using system metadata and a mandatory Client Key authenticator.
    #[must_use]
    pub fn new(
        executor: Arc<dyn ResponsesExecutor>,
        authenticator: Arc<dyn ClientKeyAuthenticator>,
        stream_capacity: StreamCapacity,
    ) -> Self {
        Self::with_metadata(
            executor,
            Arc::new(SystemResponsesMetadataFactory::new()),
            authenticator,
            stream_capacity,
        )
    }

    /// Creates P1 HTTP state with explicit metadata and Client Key authentication implementations.
    ///
    /// This is useful for deterministic HTTP tests and later configuration-owned clock/ID and
    /// P2 snapshot-authenticator implementations.
    #[must_use]
    pub fn with_metadata(
        executor: Arc<dyn ResponsesExecutor>,
        metadata_factory: Arc<dyn ResponsesMetadataFactory>,
        authenticator: Arc<dyn ClientKeyAuthenticator>,
        stream_capacity: StreamCapacity,
    ) -> Self {
        Self {
            executor,
            authenticator,
            metadata_factory,
            stream_capacity,
        }
    }
}

/// Creates a validated P1 default bounded-stream capacity.
///
/// # Errors
///
/// Returns a stream-capacity error only if the frozen default is changed to an invalid value.
pub fn default_stream_capacity() -> Result<StreamCapacity, StreamCapacityError> {
    StreamCapacity::try_new(DEFAULT_STREAM_CAPACITY)
}

/// Registers the P1 health and `OpenAI` Responses routes on an Actix application.
pub fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/healthz", web::get().to(healthz))
        .route("/v1/responses", web::post().to(responses));
}

async fn healthz() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(r#"{"status":"ok"}"#)
}

async fn responses(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    body: web::Bytes,
) -> HttpResponse {
    let _authenticated_client =
        match authenticate_bearer_request(&request, state.authenticator.as_ref()) {
            Ok(authenticated_client) => authenticated_client,
            Err(error) => return pre_header_error(&error),
        };
    let Ok(body) = std::str::from_utf8(&body) else {
        return pre_header_error(&client_request_error());
    };
    let decoded = match decode_request(body) {
        Ok(decoded) => decoded,
        Err(error) => return pre_header_error(&error),
    };
    let public_model = decoded.request.requested_model.clone();
    let context = match state.metadata_factory.request_context() {
        Ok(context) => context,
        Err(error) => return pre_header_error(&error),
    };
    let mut source = match state.executor.execute(context, decoded.request).await {
        Ok(source) => source,
        Err(error) => return pre_header_error(&error),
    };
    let first = match source.next_event().await {
        Ok(Some(event @ CanonicalEvent::ResponseStart(_))) => event,
        Ok(Some(_) | None) => return pre_header_error(&stream_protocol_error()),
        Err(error) => return pre_header_error(&error),
    };
    let metadata = match state.metadata_factory.response_metadata(&public_model) {
        Ok(metadata) => metadata,
        Err(error) => return pre_header_error(&error),
    };

    match decoded.mode {
        ResponseMode::NonStreaming => non_streaming_response(&state, source, first, metadata).await,
        ResponseMode::Streaming => streaming_response(&state, source, first, metadata).await,
    }
}

async fn non_streaming_response(
    state: &ResponsesHttpState,
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    metadata: OpenAiResponseMetadata,
) -> HttpResponse {
    let mut stream = match start_bounded_transport(source, first, state.stream_capacity).await {
        Ok(stream) => stream,
        Err(error) => return pre_header_error(&error),
    };
    let tracker = stream.control().first_semantic_event_tracker();
    let response = match collect_completed_response(&mut stream).await {
        Ok(response) => response,
        Err(error) => return pre_header_error(&error),
    };
    let Some(delivery_event) = response.events().first().cloned() else {
        return pre_header_error(&internal_error());
    };
    let body = match encode_response(&response, metadata) {
        Ok(body) => body,
        Err(error) => return pre_header_error(&error),
    };
    let body = JsonDeliveryBody::new(web::Bytes::from(body.to_string()), tracker, delivery_event);

    match HttpResponse::Ok()
        .content_type("application/json")
        .message_body(body)
    {
        Ok(response) => response.map_into_boxed_body(),
        Err(_) => pre_header_error(&internal_error()),
    }
}

async fn streaming_response(
    state: &ResponsesHttpState,
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    metadata: OpenAiResponseMetadata,
) -> HttpResponse {
    // Commit no headers until the initial event is shown encodable by a fresh protocol encoder.
    // The body owns a separate encoder so the first event still travels through P1-04 transport.
    let mut initial_encoder = OpenAiResponsesSseEncoder::new(metadata.clone());
    if let Err(error) = initial_encoder.encode_event(&first) {
        return pre_header_error(&error);
    }

    let stream = match start_bounded_transport(source, first, state.stream_capacity).await {
        Ok(stream) => stream,
        Err(error) => return pre_header_error(&error),
    };
    let tracker = stream.control().first_semantic_event_tracker();
    let body = ResponsesSseBody::new(stream, metadata, tracker);

    match HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .content_type("text/event-stream")
        .message_body(body)
    {
        Ok(response) => response.map_into_boxed_body(),
        Err(_) => pre_header_error(&internal_error()),
    }
}

async fn start_bounded_transport(
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    capacity: StreamCapacity,
) -> Result<CanonicalEventStream, GatewayError> {
    let (mut sender, stream) = bounded_canonical_stream(capacity);
    sender.send(first).await?;
    let cancellation = sender.cancellation();

    tokio::spawn(async move {
        pump_source(source, sender, cancellation).await;
    });

    Ok(stream)
}

async fn pump_source(
    mut source: Box<dyn ResponsesEventSource>,
    mut sender: CanonicalEventSender,
    cancellation: StreamCancellation,
) {
    loop {
        let next = tokio::select! {
            biased;
            () = cancellation.cancelled() => return,
            next = source.next_event() => next,
        };

        match next {
            Ok(Some(event)) => {
                let terminal = matches!(
                    event,
                    CanonicalEvent::ResponseEnd(_) | CanonicalEvent::StreamError(_)
                );
                if let Err(error) = sender.send(event).await {
                    if cancellation.is_cancelled() {
                        return;
                    }
                    send_terminal_failure(&mut sender, error, &cancellation).await;
                    return;
                }
                if terminal {
                    return;
                }
            }
            Ok(None) => {
                send_terminal_failure(
                    &mut sender,
                    GatewayError::new(GatewayErrorCode::StreamTruncated, ErrorScope::Stream),
                    &cancellation,
                )
                .await;
                return;
            }
            Err(error) => {
                send_terminal_failure(&mut sender, error, &cancellation).await;
                return;
            }
        }
    }
}

async fn send_terminal_failure(
    sender: &mut CanonicalEventSender,
    error: GatewayError,
    cancellation: &StreamCancellation,
) {
    if cancellation.is_cancelled() {
        return;
    }

    let _send_result = sender
        .send(CanonicalEvent::StreamError(StreamError { error }))
        .await;
}

async fn collect_completed_response(
    stream: &mut CanonicalEventStream,
) -> Result<CanonicalResponse, GatewayError> {
    let mut events = Vec::new();
    while let Some(event) = stream.recv().await? {
        events.push(event);
    }

    CanonicalResponse::try_new(events)
}

struct PendingSseChunk {
    bytes: web::Bytes,
    delivery_event: Option<CanonicalEvent>,
}

struct SseEncodingState {
    stream: CanonicalEventStream,
    encoder: OpenAiResponsesSseEncoder,
    pending: VecDeque<PendingSseChunk>,
    finished: bool,
}

/// A streaming HTTP body that commits `FirstSemanticEvent` only when it gives Actix a semantic
/// bytes chunk, not when the chunk is queued, received, or encoded.
struct ResponsesSseBody {
    chunks: Pin<Box<dyn Stream<Item = PendingSseChunk>>>,
    tracker: FirstSemanticEventTracker,
}

impl ResponsesSseBody {
    fn new(
        stream: CanonicalEventStream,
        metadata: OpenAiResponseMetadata,
        tracker: FirstSemanticEventTracker,
    ) -> Self {
        let state = SseEncodingState {
            stream,
            encoder: OpenAiResponsesSseEncoder::new(metadata),
            pending: VecDeque::new(),
            finished: false,
        };
        let chunks = Box::pin(stream::unfold(state, next_sse_chunk));

        Self { chunks, tracker }
    }
}

impl MessageBody for ResponsesSseBody {
    type Error = Infallible;

    fn size(&self) -> BodySize {
        BodySize::Stream
    }

    fn poll_next(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<web::Bytes, Self::Error>>> {
        let body = self.get_mut();
        match body.chunks.as_mut().poll_next(context) {
            Poll::Ready(Some(chunk)) => {
                if let Some(event) = chunk.delivery_event.as_ref() {
                    let _first_delivery = body.tracker.mark_delivered(event);
                }
                Poll::Ready(Some(Ok(chunk.bytes)))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

async fn next_sse_chunk(
    mut state: SseEncodingState,
) -> Option<(PendingSseChunk, SseEncodingState)> {
    loop {
        if let Some(chunk) = state.pending.pop_front() {
            return Some((chunk, state));
        }
        if state.finished {
            return None;
        }

        match state.stream.recv().await {
            Ok(Some(event)) => {
                let terminal = matches!(
                    event,
                    CanonicalEvent::ResponseEnd(_) | CanonicalEvent::StreamError(_)
                );
                match state.encoder.encode_event(&event) {
                    Ok(frames) => match queue_sse_frames(&mut state, &event, frames) {
                        Ok(()) => {
                            if terminal {
                                state.finished = true;
                            }
                        }
                        Err(_) => terminate_sse_with_failure(&mut state, stream_protocol_error()),
                    },
                    Err(_) => terminate_sse_with_failure(&mut state, stream_protocol_error()),
                }
            }
            Ok(None) => state.finished = true,
            Err(error) => {
                if state.stream.control().is_cancelled()
                    || error.code() == GatewayErrorCode::Cancelled
                {
                    return None;
                }
                terminate_sse_with_failure(&mut state, error);
            }
        }
    }
}

fn queue_sse_frames(
    state: &mut SseEncodingState,
    event: &CanonicalEvent,
    frames: Vec<SseFrame>,
) -> Result<(), GatewayError> {
    let mut delivery_event = Some(event.clone());
    let chunks = frames
        .into_iter()
        .map(|frame| {
            let delivery_event = if frame.is_semantic() {
                delivery_event.take()
            } else {
                None
            };
            Ok(PendingSseChunk {
                bytes: web::Bytes::from(frame.to_wire()?),
                delivery_event,
            })
        })
        .collect::<Result<Vec<_>, GatewayError>>()?;

    state.pending.extend(chunks);
    Ok(())
}

fn terminate_sse_with_failure(state: &mut SseEncodingState, error: GatewayError) {
    let failure = CanonicalEvent::StreamError(StreamError { error });
    if let Ok(frames) = state.encoder.encode_event(&failure) {
        let _queue_result = queue_sse_frames(state, &failure, frames);
    }
    state.finished = true;
}

/// A completed JSON response body that commits `FirstSemanticEvent` at the same Actix body handoff
/// boundary as streaming SSE, rather than while the JSON object is assembled.
struct JsonDeliveryBody {
    bytes: Option<web::Bytes>,
    tracker: FirstSemanticEventTracker,
    delivery_event: CanonicalEvent,
}

impl JsonDeliveryBody {
    fn new(
        bytes: web::Bytes,
        tracker: FirstSemanticEventTracker,
        delivery_event: CanonicalEvent,
    ) -> Self {
        Self {
            bytes: Some(bytes),
            tracker,
            delivery_event,
        }
    }
}

impl MessageBody for JsonDeliveryBody {
    type Error = Infallible;

    fn size(&self) -> BodySize {
        self.bytes
            .as_ref()
            .map_or(BodySize::None, |bytes| BodySize::Sized(bytes.len() as u64))
    }

    fn poll_next(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
    ) -> Poll<Option<Result<web::Bytes, Self::Error>>> {
        let body = self.get_mut();
        match body.bytes.take() {
            Some(bytes) => {
                let _first_delivery = body.tracker.mark_delivered(&body.delivery_event);
                Poll::Ready(Some(Ok(bytes)))
            }
            None => Poll::Ready(None),
        }
    }
}

fn pre_header_error(error: &GatewayError) -> HttpResponse {
    let mut response = HttpResponse::build(error_status(error));
    if error.code() == GatewayErrorCode::ClientUnauthorized {
        response.insert_header((header::WWW_AUTHENTICATE, "Bearer"));
    }
    response
        .content_type("application/json")
        .body(encode_error(error).to_string())
}

fn authenticate_bearer_request(
    request: &HttpRequest,
    authenticator: &dyn ClientKeyAuthenticator,
) -> Result<gateway_auth::AuthenticatedClient, GatewayError> {
    let mut values = request.headers().get_all(header::AUTHORIZATION);
    let Some(value) = values.next() else {
        return Err(client_unauthorized_error());
    };
    if values.next().is_some() {
        return Err(client_unauthorized_error());
    }
    let value = value.to_str().map_err(|_| client_unauthorized_error())?;
    let Some(presented_key) = value.strip_prefix("Bearer ") else {
        return Err(client_unauthorized_error());
    };
    if presented_key.is_empty() || presented_key.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(client_unauthorized_error());
    }

    authenticator.authenticate(presented_key)
}

const fn error_status(error: &GatewayError) -> StatusCode {
    match error.code() {
        GatewayErrorCode::ClientRequestError => StatusCode::BAD_REQUEST,
        GatewayErrorCode::ClientUnauthorized => StatusCode::UNAUTHORIZED,
        GatewayErrorCode::RouteNotFound => StatusCode::NOT_FOUND,
        GatewayErrorCode::ProviderRateLimited | GatewayErrorCode::CredentialQuotaExceeded => {
            StatusCode::TOO_MANY_REQUESTS
        }
        GatewayErrorCode::ProviderTransient
        | GatewayErrorCode::EgressUnavailable
        | GatewayErrorCode::CredentialUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        GatewayErrorCode::ProviderPermanent
        | GatewayErrorCode::UpstreamProtocolError
        | GatewayErrorCode::StreamTruncated
        | GatewayErrorCode::EgressRejected
        | GatewayErrorCode::CredentialUnauthorized
        | GatewayErrorCode::CredentialForbidden => StatusCode::BAD_GATEWAY,
        GatewayErrorCode::Cancelled | GatewayErrorCode::InternalError => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

const fn client_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

const fn client_unauthorized_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientUnauthorized, ErrorScope::Request)
}

const fn stream_protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        error::Error,
        future::poll_fn,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use actix_web::{
        App,
        body::MessageBody,
        http::{StatusCode, header},
        test, web,
    };
    use gateway_auth::{
        ClientKeyAuthenticator, InMemoryClientKey, InMemoryClientKeyAuthenticator,
        client_key::{ClientKeyPepper, ClientKeyService},
    };
    use gateway_core::{
        AccessGroupId, CanonicalEvent, ClientKeyId, ErrorScope, GatewayError, GatewayErrorCode,
        MessageEnd, MessageRole, MessageStart, RawExtensions, RawJson, RequestContext, RequestId,
        ResponseEnd, ResponseId, ResponseStart, StreamError, TextDelta,
    };
    use gateway_router::{
        DeterministicMockEmission, DeterministicMockResponsesExecutor, ResponsesEventSource,
        ResponsesExecutor, ResponsesFuture, RouteSnapshot, RouteSnapshotInput,
        RouteSnapshotRegistry, SnapshotAccessGroup, SnapshotClientKeyAuthenticator,
        SnapshotClientKeyView, SnapshotVersion,
    };
    use gateway_stream::{FirstSemanticEventTracker, StreamCapacity, bounded_canonical_stream};
    use protocol_openai_responses::OpenAiResponseMetadata;

    use super::{
        JsonDeliveryBody, ResponsesHttpState, ResponsesMetadataFactory, client_request_error,
        configure,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    const TEST_CLIENT_KEY: &str = "p1-test-client-key";

    #[derive(Debug)]
    struct FixedMetadata;

    impl ResponsesMetadataFactory for FixedMetadata {
        fn request_context(&self) -> Result<RequestContext, GatewayError> {
            let request_id =
                RequestId::try_new("http-test-request").map_err(|_| client_request_error())?;
            Ok(RequestContext::new(request_id))
        }

        fn response_metadata(
            &self,
            public_model: &str,
        ) -> Result<OpenAiResponseMetadata, GatewayError> {
            OpenAiResponseMetadata::try_new(public_model, 1)
        }
    }

    fn response_start() -> Result<CanonicalEvent, Box<dyn Error>> {
        Ok(CanonicalEvent::ResponseStart(ResponseStart {
            response_id: ResponseId::try_new("http-test-response")?,
            extensions: RawExtensions::default(),
        }))
    }

    fn text_events() -> Result<Vec<CanonicalEvent>, Box<dyn Error>> {
        Ok(vec![
            response_start()?,
            CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::TextDelta(TextDelta {
                text: "deterministic hello".to_owned(),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageEnd(MessageEnd::default()),
            CanonicalEvent::ResponseEnd(ResponseEnd::default()),
        ])
    }

    fn test_authenticator() -> Result<Arc<dyn ClientKeyAuthenticator>, Box<dyn Error>> {
        let key = InMemoryClientKey::try_new(
            TEST_CLIENT_KEY,
            ClientKeyId::try_new("http-test-client-key")?,
            true,
        )?;
        let authenticator = InMemoryClientKeyAuthenticator::try_new([key])?;

        Ok(Arc::new(authenticator))
    }

    fn authenticator_with_disabled_key() -> Result<Arc<dyn ClientKeyAuthenticator>, Box<dyn Error>>
    {
        let enabled = InMemoryClientKey::try_new(
            TEST_CLIENT_KEY,
            ClientKeyId::try_new("http-test-client-key")?,
            true,
        )?;
        let disabled = InMemoryClientKey::try_new(
            "p1-disabled-client-key",
            ClientKeyId::try_new("http-disabled-client-key")?,
            false,
        )?;
        let authenticator = InMemoryClientKeyAuthenticator::try_new([enabled, disabled])?;

        Ok(Arc::new(authenticator))
    }

    fn authorized(request: test::TestRequest) -> test::TestRequest {
        request.insert_header((header::AUTHORIZATION, format!("Bearer {TEST_CLIENT_KEY}")))
    }

    fn mock_state(events: Vec<CanonicalEvent>) -> Result<ResponsesHttpState, Box<dyn Error>> {
        let emissions = events
            .into_iter()
            .map(|event| DeterministicMockEmission::new(Duration::ZERO, event))
            .collect();
        let executor = DeterministicMockResponsesExecutor::try_new(
            gateway_core::ProviderId::try_new("http-test-provider")?,
            emissions,
        )?;

        Ok(ResponsesHttpState::with_metadata(
            Arc::new(executor),
            Arc::new(FixedMetadata),
            test_authenticator()?,
            StreamCapacity::try_new(2)?,
        ))
    }

    fn snapshot_auth_state(
        events: Vec<CanonicalEvent>,
    ) -> Result<(ResponsesHttpState, String), Box<dyn Error>> {
        let emissions = events
            .into_iter()
            .map(|event| DeterministicMockEmission::new(Duration::ZERO, event))
            .collect();
        let executor = DeterministicMockResponsesExecutor::try_new(
            gateway_core::ProviderId::try_new("http-snapshot-auth-provider")?,
            emissions,
        )?;
        let service = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0xA5_u8; 32])?);
        let access_group_id = AccessGroupId::try_new("http-snapshot-access-group")?;
        let issued = service.issue(
            ClientKeyId::try_new("http-snapshot-client-key")?,
            access_group_id.clone(),
            None,
        )?;
        let (record, presented_key) = issued.into_parts();
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new("http-snapshot-version")?,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![SnapshotAccessGroup::new(
                access_group_id,
                "HTTP Snapshot Access Group".to_owned(),
                BTreeSet::new(),
            )],
            vec![SnapshotClientKeyView::new(record, BTreeSet::new())],
        ))?);
        let authenticator = SnapshotClientKeyAuthenticator::new(
            Arc::new(RouteSnapshotRegistry::new(snapshot)),
            service,
        );
        let state = ResponsesHttpState::with_metadata(
            Arc::new(executor),
            Arc::new(FixedMetadata),
            Arc::new(authenticator),
            StreamCapacity::try_new(2)?,
        );

        Ok((state, presented_key.as_str().to_owned()))
    }

    #[actix_web::test]
    async fn healthz_returns_a_small_json_status() -> TestResult {
        let state = mock_state(text_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;

        let response =
            test::call_service(&app, test::TestRequest::get().uri("/healthz").to_request()).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        assert_eq!(body.as_ref(), br#"{"status":"ok"}"#);
        Ok(())
    }

    #[actix_web::test]
    async fn responses_rejects_invalid_bearer_inputs_before_decode_or_provider_execution()
    -> TestResult {
        let calls = Arc::new(AtomicUsize::new(0));
        let state = ResponsesHttpState::with_metadata(
            Arc::new(CountingExecutor {
                calls: calls.clone(),
            }),
            Arc::new(FixedMetadata),
            authenticator_with_disabled_key()?,
            StreamCapacity::try_new(2)?,
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let requests = [
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload("not-json")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header((header::AUTHORIZATION, "Basic not-a-client-key"))
                .set_payload("not-json")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/responses")
                .append_header((header::AUTHORIZATION, format!("Bearer {TEST_CLIENT_KEY}")))
                .append_header((header::AUTHORIZATION, "Bearer another-test-key"))
                .set_payload("not-json")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header((header::AUTHORIZATION, "Bearer unknown-test-key"))
                .set_payload("not-json")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header((header::AUTHORIZATION, "Bearer p1-disabled-client-key"))
                .set_payload("not-json")
                .to_request(),
        ];

        let mut expected_envelope = None;
        for request in requests {
            let response = test::call_service(&app, request).await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(
                response
                    .headers()
                    .get(header::WWW_AUTHENTICATE)
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer")
            );
            let body = String::from_utf8(test::read_body(response).await.to_vec())?;
            assert!(body.contains(r#""code":"ClientUnauthorized""#));
            assert!(!body.contains("ClientRequestError"));
            if let Some(expected) = &expected_envelope {
                assert_eq!(&body, expected);
            } else {
                expected_envelope = Some(body);
            }
        }

        assert_eq!(calls.load(Ordering::Acquire), 0);
        Ok(())
    }

    #[actix_web::test]
    async fn non_streaming_responses_uses_mock_through_router_and_bounded_transport() -> TestResult
    {
        let state = mock_state(text_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"hello"}"#),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""status":"completed""#));
        assert!(body.contains(r#""text":"deterministic hello""#));
        assert!(body.contains(r#""created_at":1"#));
        Ok(())
    }

    #[actix_web::test]
    async fn responses_accepts_a_snapshot_client_key_authenticator() -> TestResult {
        let (state, presented_key) = snapshot_auth_state(text_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header((header::AUTHORIZATION, format!("Bearer {presented_key}")))
            .set_payload(r#"{"model":"mock-model","input":"hello"}"#)
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""status":"completed""#));
        Ok(())
    }

    #[actix_web::test]
    async fn streaming_responses_emits_openai_sse_through_actix_body() -> TestResult {
        let state = mock_state(text_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"hello","stream":true}"#),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains("event: response.created"));
        assert!(body.contains("event: response.output_text.delta"));
        assert!(body.contains("event: response.completed"));
        assert!(!body.contains("event: response.failed"));
        Ok(())
    }

    #[actix_web::test]
    async fn duplicate_json_names_are_rejected_before_actix_can_normalize_them() -> TestResult {
        let state = mock_state(text_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"one","model":"two","input":"hello"}"#),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""code":"ClientRequestError""#));
        Ok(())
    }

    #[actix_web::test]
    async fn post_start_stream_error_emits_failed_not_completed() -> TestResult {
        let mut events = text_events()?;
        let _response_end = events.pop();
        events.push(CanonicalEvent::StreamError(StreamError {
            error: GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider),
        }));
        let state = mock_state(events)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"hello","stream":true}"#),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains("event: response.failed"));
        assert!(!body.contains("event: response.completed"));
        Ok(())
    }

    #[actix_web::test]
    async fn post_header_source_eof_becomes_one_safe_failed_event() -> TestResult {
        let state = ResponsesHttpState::with_metadata(
            Arc::new(EarlyEofExecutor),
            Arc::new(FixedMetadata),
            test_authenticator()?,
            StreamCapacity::try_new(2)?,
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"hello","stream":true}"#),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains("event: response.failed"));
        assert!(body.contains(r#""code":"StreamTruncated""#));
        assert!(!body.contains("event: response.completed"));
        Ok(())
    }

    #[actix_web::test]
    async fn post_header_sse_encoding_error_becomes_failed_not_completed() -> TestResult {
        let mut events = text_events()?;
        let CanonicalEvent::TextDelta(delta) = &mut events[2] else {
            return Err("text fixture lost its expected delta".into());
        };
        let mut extensions = RawExtensions::default();
        extensions.try_insert(
            "unrepresentable",
            RawJson::from_json_string("true".to_owned())?,
        )?;
        delta.extensions = extensions;
        let state = mock_state(events)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"hello","stream":true}"#),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains("event: response.failed"));
        assert!(body.contains(r#""code":"UpstreamProtocolError""#));
        assert!(!body.contains("event: response.completed"));
        Ok(())
    }

    #[actix_web::test]
    async fn pre_header_executor_errors_use_a_safe_json_envelope() -> TestResult {
        let state = ResponsesHttpState::with_metadata(
            Arc::new(FailingExecutor),
            Arc::new(FixedMetadata),
            test_authenticator()?,
            StreamCapacity::try_new(2)?,
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"hello"}"#),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""code":"ProviderTransient""#));
        Ok(())
    }

    #[actix_web::test]
    async fn completed_json_commits_fse_only_when_actix_polls_the_body() -> TestResult {
        let tracker = FirstSemanticEventTracker::default();
        let event = response_start()?;
        let mut body = Box::pin(JsonDeliveryBody::new(
            web::Bytes::from_static(b"{}"),
            tracker.clone(),
            event,
        ));
        assert!(!tracker.is_committed());

        let first = poll_fn(|context| body.as_mut().poll_next(context)).await;
        assert!(matches!(first, Some(Ok(bytes)) if bytes.as_ref() == b"{}"));
        assert!(tracker.is_committed());
        Ok(())
    }

    #[actix_web::test]
    async fn sse_commits_fse_only_when_its_first_semantic_chunk_reaches_actix() -> TestResult {
        let (mut sender, stream) = bounded_canonical_stream(StreamCapacity::try_new(1)?);
        let tracker = stream.control().first_semantic_event_tracker();
        sender.send(response_start()?).await?;
        let mut body = Box::pin(super::ResponsesSseBody::new(
            stream,
            OpenAiResponseMetadata::try_new("mock-model", 1)?,
            tracker.clone(),
        ));
        assert!(!tracker.is_committed());

        let first = poll_fn(|context| body.as_mut().poll_next(context)).await;
        assert!(matches!(
            first,
            Some(Ok(bytes)) if String::from_utf8_lossy(&bytes).contains("event: response.created")
        ));
        assert!(tracker.is_committed());
        Ok(())
    }

    #[actix_web::test]
    async fn dropping_an_unconsumed_sse_body_cancels_and_drops_the_source() -> TestResult {
        let dropped = Arc::new(AtomicBool::new(false));
        let state = ResponsesHttpState::with_metadata(
            Arc::new(DroppingExecutor {
                dropped: dropped.clone(),
            }),
            Arc::new(FixedMetadata),
            test_authenticator()?,
            StreamCapacity::try_new(1)?,
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"hello","stream":true}"#),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        drop(response);
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        assert!(dropped.load(Ordering::Acquire));
        Ok(())
    }

    struct CountingExecutor {
        calls: Arc<AtomicUsize>,
    }

    impl ResponsesExecutor for CountingExecutor {
        fn execute(
            &self,
            _context: RequestContext,
            _request: gateway_core::CanonicalRequest,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            Box::pin(async {
                Err(GatewayError::new(
                    GatewayErrorCode::InternalError,
                    ErrorScope::Internal,
                ))
            })
        }
    }

    struct FailingExecutor;

    impl ResponsesExecutor for FailingExecutor {
        fn execute(
            &self,
            _context: RequestContext,
            _request: gateway_core::CanonicalRequest,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            Box::pin(async {
                Err(GatewayError::new(
                    GatewayErrorCode::ProviderTransient,
                    ErrorScope::Provider,
                ))
            })
        }
    }

    struct DroppingExecutor {
        dropped: Arc<AtomicBool>,
    }

    struct EarlyEofExecutor;

    impl ResponsesExecutor for EarlyEofExecutor {
        fn execute(
            &self,
            _context: RequestContext,
            _request: gateway_core::CanonicalRequest,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            Box::pin(async {
                Ok(Box::new(EarlyEofSource {
                    first_event_pending: true,
                }) as Box<dyn ResponsesEventSource>)
            })
        }
    }

    struct EarlyEofSource {
        first_event_pending: bool,
    }

    impl ResponsesEventSource for EarlyEofSource {
        fn next_event(
            &mut self,
        ) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
            if self.first_event_pending {
                self.first_event_pending = false;
                let event = ResponseId::try_new("early-eof-test-response")
                    .map(|response_id| {
                        CanonicalEvent::ResponseStart(ResponseStart {
                            response_id,
                            extensions: RawExtensions::default(),
                        })
                    })
                    .map_err(|_| client_request_error());
                return Box::pin(async move { event.map(Some) });
            }

            Box::pin(async { Ok(None) })
        }
    }

    impl ResponsesExecutor for DroppingExecutor {
        fn execute(
            &self,
            _context: RequestContext,
            _request: gateway_core::CanonicalRequest,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            let dropped = self.dropped.clone();
            Box::pin(async move {
                Ok(Box::new(DroppingSource {
                    dropped,
                    first_event_pending: true,
                }) as Box<dyn ResponsesEventSource>)
            })
        }
    }

    struct DroppingSource {
        dropped: Arc<AtomicBool>,
        first_event_pending: bool,
    }

    impl Drop for DroppingSource {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::Release);
        }
    }

    impl ResponsesEventSource for DroppingSource {
        fn next_event(
            &mut self,
        ) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
            if self.first_event_pending {
                self.first_event_pending = false;
                let event = ResponseId::try_new("dropping-test-response")
                    .map(|response_id| {
                        CanonicalEvent::ResponseStart(ResponseStart {
                            response_id,
                            extensions: RawExtensions::default(),
                        })
                    })
                    .map_err(|_| client_request_error());
                return Box::pin(async move { event.map(Some) });
            }

            Box::pin(async move {
                tokio::time::sleep(Duration::from_mins(1)).await;
                Ok(None)
            })
        }
    }

    #[actix_web::test]
    async fn client_request_error_stays_request_owned() {
        assert_eq!(client_request_error().scope(), ErrorScope::Request);
    }
}
