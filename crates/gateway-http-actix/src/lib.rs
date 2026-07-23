//! `Actix Web` transport shell. Core crates must not depend on this crate.
//!
//! P1 exposes a deliberately small vertical slice: public `GET /healthz` and Client Key-protected
//! `POST /v1/responses`. Request bytes are decoded by the protocol adapter rather than Actix's
//! JSON extractor so duplicate JSON member names remain observable and rejectable.

#![deny(unsafe_code)]

/// Protected P10 draft-resource handlers for Upstreams, Endpoints, Credentials, and Egress.
pub mod management_resources;
/// Independent management HTTP authentication, network, audit-identity, and browser boundary.
pub mod management_security;

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
use gateway_auth::{AuthenticatedClient, ClientKeyAuthenticator};
use gateway_core::{
    AccessGroupId, CanonicalEvent, CanonicalResponse, ClientKeyId, ErrorScope, GatewayError,
    GatewayErrorCode, GatewayEvent, GatewayEventSink, GatewayProtocol, NoopGatewayEventSink,
    RequestContext, RequestEvent, RequestId, ResponseId, StreamError, TransparentRetryGate,
    UsageEvent,
};
use gateway_router::{
    CountTokensExecution, CountTokensExecutor, ResponsesEventSource, ResponsesExecution,
    ResponsesExecutor, ResponsesResponseMode, SnapshotAuthenticatedClient,
    SnapshotClientKeyAuthenticator, UnsupportedCountTokensExecutor,
};
use gateway_stream::{
    CanonicalEventSender, CanonicalEventStream, FirstSemanticEventTracker, StreamCancellation,
    StreamCapacity, StreamCapacityError, bounded_canonical_stream,
};
use protocol_anthropic::{
    AnthropicMessagesSseEncoder, AnthropicResponseMetadata, ResponseMode as AnthropicResponseMode,
    SseFrame as AnthropicSseFrame, decode_count_tokens_request,
    decode_request as decode_anthropic_request, encode_count_tokens,
    encode_error as encode_anthropic_error, encode_response as encode_anthropic_response,
};
use protocol_openai_responses::{
    OpenAiResponseMetadata, OpenAiResponsesSseEncoder, ResponseMode, SseFrame as OpenAiSseFrame,
    decode_request, encode_error, encode_model_list, encode_response,
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
/// unauthenticated behavior. P3's Snapshot constructor additionally pins authenticated model
/// discovery and response mapping to the exact Snapshot used for Client Key admission. Health
/// remains independent of this state-owned authentication path.
#[derive(Clone)]
pub struct ResponsesHttpState {
    executor: Arc<dyn ResponsesExecutor>,
    count_tokens_executor: Arc<dyn CountTokensExecutor>,
    authenticator: ResponsesAuthenticator,
    metadata_factory: Arc<dyn ResponsesMetadataFactory>,
    stream_capacity: StreamCapacity,
    event_sink: Arc<dyn GatewayEventSink>,
}

#[derive(Clone)]
enum ResponsesAuthenticator {
    Generic(Arc<dyn ClientKeyAuthenticator>),
    Snapshot(Arc<SnapshotClientKeyAuthenticator>),
}

enum AuthenticatedResponsesClient {
    Generic(AuthenticatedClient),
    Snapshot(SnapshotAuthenticatedClient),
}

impl AuthenticatedResponsesClient {
    fn event_identity(&self) -> (ClientKeyId, Option<AccessGroupId>) {
        match self {
            Self::Generic(client) => (
                client.client_key_id().clone(),
                client.access_group_id().cloned(),
            ),
            Self::Snapshot(client) => (
                client.client_key_id().clone(),
                Some(client.access_group_id().clone()),
            ),
        }
    }
}

impl ResponsesAuthenticator {
    fn authenticate(
        &self,
        presented_key: &str,
    ) -> Result<AuthenticatedResponsesClient, GatewayError> {
        match self {
            Self::Generic(authenticator) => authenticator
                .authenticate(presented_key)
                .map(AuthenticatedResponsesClient::Generic),
            Self::Snapshot(authenticator) => authenticator
                .authenticate_pinned(presented_key)
                .map(AuthenticatedResponsesClient::Snapshot),
        }
    }
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
        Self::with_metadata_and_event_sink(
            executor,
            metadata_factory,
            authenticator,
            Arc::new(NoopGatewayEventSink),
            stream_capacity,
        )
    }

    /// Creates HTTP state with an explicit non-blocking structured event sink.
    #[must_use]
    pub fn with_metadata_and_event_sink(
        executor: Arc<dyn ResponsesExecutor>,
        metadata_factory: Arc<dyn ResponsesMetadataFactory>,
        authenticator: Arc<dyn ClientKeyAuthenticator>,
        event_sink: Arc<dyn GatewayEventSink>,
        stream_capacity: StreamCapacity,
    ) -> Self {
        Self {
            executor,
            count_tokens_executor: Arc::new(UnsupportedCountTokensExecutor),
            authenticator: ResponsesAuthenticator::Generic(authenticator),
            metadata_factory,
            stream_capacity,
            event_sink,
        }
    }

    /// Creates P3 HTTP state whose Client Key admission and public-model view share one Snapshot.
    #[must_use]
    pub fn with_snapshot_metadata(
        executor: Arc<dyn ResponsesExecutor>,
        metadata_factory: Arc<dyn ResponsesMetadataFactory>,
        authenticator: Arc<SnapshotClientKeyAuthenticator>,
        stream_capacity: StreamCapacity,
    ) -> Self {
        Self::with_snapshot_metadata_and_event_sink(
            executor,
            metadata_factory,
            authenticator,
            Arc::new(NoopGatewayEventSink),
            stream_capacity,
        )
    }

    /// Creates Snapshot-authenticated HTTP state with an explicit non-blocking event sink.
    #[must_use]
    pub fn with_snapshot_metadata_and_event_sink(
        executor: Arc<dyn ResponsesExecutor>,
        metadata_factory: Arc<dyn ResponsesMetadataFactory>,
        authenticator: Arc<SnapshotClientKeyAuthenticator>,
        event_sink: Arc<dyn GatewayEventSink>,
        stream_capacity: StreamCapacity,
    ) -> Self {
        Self {
            executor,
            count_tokens_executor: Arc::new(UnsupportedCountTokensExecutor),
            authenticator: ResponsesAuthenticator::Snapshot(authenticator),
            metadata_factory,
            stream_capacity,
            event_sink,
        }
    }

    /// Creates P3 Snapshot-authenticated HTTP state using system request/response metadata.
    #[must_use]
    pub fn new_with_snapshot_authentication(
        executor: Arc<dyn ResponsesExecutor>,
        authenticator: Arc<SnapshotClientKeyAuthenticator>,
        stream_capacity: StreamCapacity,
    ) -> Self {
        Self::with_snapshot_metadata(
            executor,
            Arc::new(SystemResponsesMetadataFactory::new()),
            authenticator,
            stream_capacity,
        )
    }

    /// Replaces the default explicit rejection with a route-aware exact token-count executor.
    ///
    /// The supplied executor must return an exact value or the stable unsupported-capability error;
    /// this state offers no local estimation fallback.
    #[must_use]
    pub fn with_count_tokens_executor(
        mut self,
        count_tokens_executor: Arc<dyn CountTokensExecutor>,
    ) -> Self {
        self.count_tokens_executor = count_tokens_executor;
        self
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

/// Registers the loopback readiness, health, public Models, `OpenAI` Responses, and Anthropic
/// Messages routes on an Actix application.
pub fn configure(config: &mut web::ServiceConfig) {
    config
        // Claude Code probes the configured Anthropic base URL with `HEAD /` before its first
        // Messages request. This says only that the local HTTP boundary is reachable; it reveals
        // no route, model, or authentication state.
        .route("/", web::head().to(base_url_probe))
        .route("/healthz", web::get().to(healthz))
        .route("/v1/models", web::get().to(models))
        .route("/v1/responses", web::post().to(responses))
        .route("/v1/messages", web::post().to(messages))
        .route("/v1/messages/count_tokens", web::post().to(count_tokens))
        .configure(management_resources::configure_management_resources);
}

async fn base_url_probe() -> HttpResponse {
    HttpResponse::Ok().finish()
}

async fn healthz() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(r#"{"status":"ok"}"#)
}

async fn models(request: HttpRequest, state: web::Data<ResponsesHttpState>) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(AuthenticatedResponsesClient::Snapshot(authenticated_client)) => authenticated_client,
        Ok(AuthenticatedResponsesClient::Generic(_)) => return pre_header_error(&route_not_found()),
        Err(error) => return pre_header_error(&error),
    };
    let body = match encode_model_list(
        authenticated_client
            .public_models()
            .map(gateway_router::SnapshotPublicModel::model_name),
    ) {
        Ok(body) => body,
        Err(error) => return pre_header_error(&error),
    };

    HttpResponse::Ok()
        .content_type("application/json")
        .body(body.to_string())
}

async fn responses(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    body: web::Bytes,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
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
    let requested_model = decoded.request.requested_model.clone();
    let (public_model, route_alias, route_id) =
        match resolve_public_model(&authenticated_client, &decoded.request.requested_model) {
            Ok(resolved) => resolved,
            Err(error) => return pre_header_error(&error),
        };
    let context = match state.metadata_factory.request_context() {
        Ok(context) => context,
        Err(error) => return pre_header_error(&error),
    };
    let request_id = context.request_id().clone();
    let (client_key_id, access_group_id) = authenticated_client.event_identity();
    let _request_event = state
        .event_sink
        .try_emit(GatewayEvent::Request(RequestEvent::new(
            request_id.clone(),
            client_key_id,
            access_group_id,
            GatewayProtocol::OpenAiResponses,
            requested_model,
            public_model.clone(),
            route_alias,
            decoded.mode == ResponseMode::Streaming,
        )));
    let (sender, stream) = bounded_canonical_stream(state.stream_capacity);
    let retry_gate: Arc<dyn TransparentRetryGate> = Arc::new(stream.control());
    let response_mode = match decoded.mode {
        ResponseMode::NonStreaming => ResponsesResponseMode::NonStreaming,
        ResponseMode::Streaming => ResponsesResponseMode::Streaming,
    };
    let execution = ResponsesExecution::new(
        context,
        decoded.request,
        route_id,
        response_mode,
        retry_gate,
    );
    let mut source = match state.executor.execute_routed(execution).await {
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
    let usage_observer = UsageEventObserver::new(request_id, Arc::clone(&state.event_sink));

    match decoded.mode {
        ResponseMode::NonStreaming => {
            non_streaming_response(source, first, metadata, usage_observer, sender, stream).await
        }
        ResponseMode::Streaming => {
            streaming_response(source, first, metadata, usage_observer, sender, stream).await
        }
    }
}

async fn messages(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    body: web::Bytes,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(authenticated_client) => authenticated_client,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return pre_header_anthropic_error(&client_request_error());
    };
    let decoded = match decode_anthropic_request(body) {
        Ok(decoded) => decoded,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let requested_model = decoded.request.requested_model.clone();
    let (public_model, route_alias, route_id) =
        match resolve_public_model(&authenticated_client, &decoded.request.requested_model) {
            Ok(resolved) => resolved,
            Err(error) => return pre_header_anthropic_error(&error),
        };
    let context = match state.metadata_factory.request_context() {
        Ok(context) => context,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let request_id = context.request_id().clone();
    let (client_key_id, access_group_id) = authenticated_client.event_identity();
    let _request_event = state
        .event_sink
        .try_emit(GatewayEvent::Request(RequestEvent::new(
            request_id.clone(),
            client_key_id,
            access_group_id,
            GatewayProtocol::AnthropicMessages,
            requested_model,
            public_model.clone(),
            route_alias,
            decoded.mode == AnthropicResponseMode::Streaming,
        )));
    let (sender, stream) = bounded_canonical_stream(state.stream_capacity);
    let retry_gate: Arc<dyn TransparentRetryGate> = Arc::new(stream.control());
    let response_mode = match decoded.mode {
        AnthropicResponseMode::NonStreaming => ResponsesResponseMode::NonStreaming,
        AnthropicResponseMode::Streaming => ResponsesResponseMode::Streaming,
    };
    let execution = ResponsesExecution::new(
        context,
        decoded.request,
        route_id,
        response_mode,
        retry_gate,
    );
    let mut source = match state.executor.execute_routed(execution).await {
        Ok(source) => source,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let first = match source.next_event().await {
        Ok(Some(event @ CanonicalEvent::ResponseStart(_))) => event,
        Ok(Some(_) | None) => return pre_header_anthropic_error(&stream_protocol_error()),
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let metadata = match AnthropicResponseMetadata::try_new(public_model) {
        Ok(metadata) => metadata,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let usage_observer = UsageEventObserver::new(request_id, Arc::clone(&state.event_sink));

    match decoded.mode {
        AnthropicResponseMode::NonStreaming => {
            anthropic_non_streaming_response(
                source,
                first,
                metadata,
                usage_observer,
                sender,
                stream,
            )
            .await
        }
        AnthropicResponseMode::Streaming => {
            anthropic_streaming_response(source, first, metadata, usage_observer, sender, stream)
                .await
        }
    }
}

async fn count_tokens(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    body: web::Bytes,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(authenticated_client) => authenticated_client,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return pre_header_anthropic_error(&client_request_error());
    };
    let decoded = match decode_count_tokens_request(body) {
        Ok(decoded) => decoded,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let (_public_model, _route_alias, route_id) =
        match resolve_public_model(&authenticated_client, &decoded.request.requested_model) {
            Ok(resolved) => resolved,
            Err(error) => return pre_header_anthropic_error(&error),
        };
    let context = match state.metadata_factory.request_context() {
        Ok(context) => context,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let execution = CountTokensExecution::new(context, decoded.request, route_id);
    let count = match state.count_tokens_executor.count_tokens(execution).await {
        Ok(count) => count,
        Err(error) => return pre_header_anthropic_error(&error),
    };

    // Model resolution happens before capability execution so a Snapshot executor receives the
    // approved route identity. The canonical request deliberately retains the client alias for
    // Provider encoding and observability; the route identity proves resolved routing.
    HttpResponse::Ok()
        .content_type("application/json")
        .body(encode_count_tokens(count).to_string())
}

fn resolve_public_model(
    authenticated_client: &AuthenticatedResponsesClient,
    requested_model: &str,
) -> Result<(String, Option<String>, Option<gateway_core::RouteId>), GatewayError> {
    match authenticated_client {
        AuthenticatedResponsesClient::Generic(_) => Ok((requested_model.to_owned(), None, None)),
        AuthenticatedResponsesClient::Snapshot(authenticated_client) => {
            let Some(public_model) = authenticated_client.resolve_public_model(requested_model)
            else {
                return Err(route_not_found());
            };
            let public_model_name = public_model.model_name().to_owned();
            let route_alias =
                (requested_model != public_model_name).then(|| requested_model.to_owned());
            Ok((
                public_model_name,
                route_alias,
                Some(public_model.route_id().clone()),
            ))
        }
    }
}

async fn non_streaming_response(
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    metadata: OpenAiResponseMetadata,
    usage_observer: UsageEventObserver,
    sender: CanonicalEventSender,
    stream: CanonicalEventStream,
) -> HttpResponse {
    let mut stream =
        match start_bounded_transport(source, first, sender, stream, usage_observer).await {
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

async fn anthropic_non_streaming_response(
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    metadata: AnthropicResponseMetadata,
    usage_observer: UsageEventObserver,
    sender: CanonicalEventSender,
    stream: CanonicalEventStream,
) -> HttpResponse {
    let mut stream =
        match start_bounded_transport(source, first, sender, stream, usage_observer).await {
            Ok(stream) => stream,
            Err(error) => return pre_header_anthropic_error(&error),
        };
    let tracker = stream.control().first_semantic_event_tracker();
    let response = match collect_completed_response(&mut stream).await {
        Ok(response) => response,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let Some(delivery_event) = response.events().first().cloned() else {
        return pre_header_anthropic_error(&internal_error());
    };
    let body = match encode_anthropic_response(&response, metadata) {
        Ok(body) => body,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let body = JsonDeliveryBody::new(web::Bytes::from(body.to_string()), tracker, delivery_event);

    match HttpResponse::Ok()
        .content_type("application/json")
        .message_body(body)
    {
        Ok(response) => response.map_into_boxed_body(),
        Err(_) => pre_header_anthropic_error(&internal_error()),
    }
}

async fn streaming_response(
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    metadata: OpenAiResponseMetadata,
    usage_observer: UsageEventObserver,
    sender: CanonicalEventSender,
    stream: CanonicalEventStream,
) -> HttpResponse {
    // Commit no headers until the initial event is shown encodable by a fresh protocol encoder.
    // The body owns a separate encoder so the first event still travels through P1-04 transport.
    let mut initial_encoder = OpenAiResponsesSseEncoder::new(metadata.clone());
    if let Err(error) = initial_encoder.encode_event(&first) {
        return pre_header_error(&error);
    }

    let stream = match start_bounded_transport(source, first, sender, stream, usage_observer).await
    {
        Ok(stream) => stream,
        Err(error) => return pre_header_error(&error),
    };
    let tracker = stream.control().first_semantic_event_tracker();
    let body = ProtocolSseBody::new(stream, OpenAiResponsesSseEncoder::new(metadata), tracker);

    match HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .content_type("text/event-stream")
        .message_body(body)
    {
        Ok(response) => response.map_into_boxed_body(),
        Err(_) => pre_header_error(&internal_error()),
    }
}

async fn anthropic_streaming_response(
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    metadata: AnthropicResponseMetadata,
    usage_observer: UsageEventObserver,
    sender: CanonicalEventSender,
    stream: CanonicalEventStream,
) -> HttpResponse {
    // Mirror the Responses boundary: no success header is committed before the first canonical
    // event is proven encodable by the protocol-specific SSE encoder.
    let mut initial_encoder = AnthropicMessagesSseEncoder::new(metadata.clone());
    if let Err(error) = initial_encoder.encode_event(&first) {
        return pre_header_anthropic_error(&error);
    }

    let stream = match start_bounded_transport(source, first, sender, stream, usage_observer).await
    {
        Ok(stream) => stream,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let tracker = stream.control().first_semantic_event_tracker();
    let body = ProtocolSseBody::new(stream, AnthropicMessagesSseEncoder::new(metadata), tracker);

    match HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .content_type("text/event-stream")
        .message_body(body)
    {
        Ok(response) => response.map_into_boxed_body(),
        Err(_) => pre_header_anthropic_error(&internal_error()),
    }
}

async fn start_bounded_transport(
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    mut sender: CanonicalEventSender,
    stream: CanonicalEventStream,
    mut usage_observer: UsageEventObserver,
) -> Result<CanonicalEventStream, GatewayError> {
    sender.send(first.clone()).await?;
    usage_observer.observe(&first);
    let cancellation = sender.cancellation();

    tokio::spawn(async move {
        pump_source(source, sender, cancellation, usage_observer).await;
    });

    Ok(stream)
}

async fn pump_source(
    mut source: Box<dyn ResponsesEventSource>,
    mut sender: CanonicalEventSender,
    cancellation: StreamCancellation,
    mut usage_observer: UsageEventObserver,
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
                if let Err(error) = sender.send(event.clone()).await {
                    if cancellation.is_cancelled() {
                        return;
                    }
                    send_terminal_failure(&mut sender, error, &cancellation).await;
                    return;
                }
                usage_observer.observe(&event);
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

/// Per-request observer that turns a canonical final Usage event into a non-blocking record.
///
/// It observes an event only after the bounded canonical stream accepted it, so invalid source
/// events cannot create a Usage record. This boundary intentionally emits final totals only and
/// discards canonical raw extensions.
struct UsageEventObserver {
    request_id: RequestId,
    event_sink: Arc<dyn GatewayEventSink>,
    response_id: Option<ResponseId>,
    final_usage_emitted: bool,
}

impl UsageEventObserver {
    fn new(request_id: RequestId, event_sink: Arc<dyn GatewayEventSink>) -> Self {
        Self {
            request_id,
            event_sink,
            response_id: None,
            final_usage_emitted: false,
        }
    }

    fn observe(&mut self, event: &CanonicalEvent) {
        match event {
            CanonicalEvent::ResponseStart(start) => {
                self.response_id = Some(start.response_id.clone());
            }
            CanonicalEvent::UsageDelta(delta) if delta.is_final && !self.final_usage_emitted => {
                self.final_usage_emitted = true;
                if let Some(response_id) = self.response_id.clone() {
                    let _usage_event =
                        self.event_sink
                            .try_emit(GatewayEvent::Usage(UsageEvent::from_usage(
                                self.request_id.clone(),
                                response_id,
                                &delta.usage,
                            )));
                }
            }
            _ => {}
        }
    }
}

struct PendingSseChunk {
    bytes: web::Bytes,
    delivery_event: Option<CanonicalEvent>,
}

trait EncodedSseFrame {
    fn is_semantic(&self) -> bool;

    fn to_wire(&self) -> Result<String, GatewayError>;
}

impl EncodedSseFrame for OpenAiSseFrame {
    fn is_semantic(&self) -> bool {
        self.is_semantic()
    }

    fn to_wire(&self) -> Result<String, GatewayError> {
        self.to_wire()
    }
}

impl EncodedSseFrame for AnthropicSseFrame {
    fn is_semantic(&self) -> bool {
        self.is_semantic()
    }

    fn to_wire(&self) -> Result<String, GatewayError> {
        self.to_wire()
    }
}

trait CanonicalSseEncoder {
    type Frame: EncodedSseFrame;

    fn encode_event(&mut self, event: &CanonicalEvent) -> Result<Vec<Self::Frame>, GatewayError>;
}

impl CanonicalSseEncoder for OpenAiResponsesSseEncoder {
    type Frame = OpenAiSseFrame;

    fn encode_event(&mut self, event: &CanonicalEvent) -> Result<Vec<Self::Frame>, GatewayError> {
        OpenAiResponsesSseEncoder::encode_event(self, event)
    }
}

impl CanonicalSseEncoder for AnthropicMessagesSseEncoder {
    type Frame = AnthropicSseFrame;

    fn encode_event(&mut self, event: &CanonicalEvent) -> Result<Vec<Self::Frame>, GatewayError> {
        AnthropicMessagesSseEncoder::encode_event(self, event)
    }
}

struct SseEncodingState<E> {
    stream: CanonicalEventStream,
    encoder: E,
    pending: VecDeque<PendingSseChunk>,
    finished: bool,
}

/// A streaming HTTP body that commits `FirstSemanticEvent` only when it gives Actix a semantic
/// bytes chunk, not when the chunk is queued, received, or encoded.
struct ProtocolSseBody<E> {
    chunks: Pin<Box<dyn Stream<Item = PendingSseChunk>>>,
    tracker: FirstSemanticEventTracker,
    _encoder: std::marker::PhantomData<E>,
}

impl<E> ProtocolSseBody<E>
where
    E: CanonicalSseEncoder + Unpin + 'static,
{
    fn new(stream: CanonicalEventStream, encoder: E, tracker: FirstSemanticEventTracker) -> Self {
        let state = SseEncodingState {
            stream,
            encoder,
            pending: VecDeque::new(),
            finished: false,
        };
        let chunks = Box::pin(stream::unfold(state, next_sse_chunk));

        Self {
            chunks,
            tracker,
            _encoder: std::marker::PhantomData,
        }
    }
}

impl<E> MessageBody for ProtocolSseBody<E>
where
    E: CanonicalSseEncoder + Unpin + 'static,
{
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

async fn next_sse_chunk<E>(
    mut state: SseEncodingState<E>,
) -> Option<(PendingSseChunk, SseEncodingState<E>)>
where
    E: CanonicalSseEncoder,
{
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

fn queue_sse_frames<E>(
    state: &mut SseEncodingState<E>,
    event: &CanonicalEvent,
    frames: Vec<E::Frame>,
) -> Result<(), GatewayError>
where
    E: CanonicalSseEncoder,
{
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

fn terminate_sse_with_failure<E>(state: &mut SseEncodingState<E>, error: GatewayError)
where
    E: CanonicalSseEncoder,
{
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

fn pre_header_anthropic_error(error: &GatewayError) -> HttpResponse {
    let mut response = HttpResponse::build(error_status(error));
    if error.code() == GatewayErrorCode::ClientUnauthorized {
        response.insert_header((header::WWW_AUTHENTICATE, "Bearer"));
    }
    response
        .content_type("application/json")
        .body(encode_anthropic_error(error).to_string())
}

fn authenticate_client_key_request(
    request: &HttpRequest,
    authenticator: &ResponsesAuthenticator,
) -> Result<AuthenticatedResponsesClient, GatewayError> {
    let presented_key = presented_client_key(request)?;
    authenticator.authenticate(presented_key)
}

fn presented_client_key(request: &HttpRequest) -> Result<&str, GatewayError> {
    let authorization = single_header(request, header::AUTHORIZATION)?;
    let api_key = single_header(request, header::HeaderName::from_static("x-api-key"))?;
    match (authorization, api_key) {
        (Some(authorization), None) => presented_bearer_value(authorization),
        (None, Some(api_key)) => presented_x_api_key_value(api_key),
        (None, None) | (Some(_), Some(_)) => Err(client_unauthorized_error()),
    }
}

fn single_header(
    request: &HttpRequest,
    name: header::HeaderName,
) -> Result<Option<&header::HeaderValue>, GatewayError> {
    let mut values = request.headers().get_all(name);
    let value = values.next();
    if values.next().is_some() {
        return Err(client_unauthorized_error());
    }
    Ok(value)
}

fn presented_bearer_value(value: &header::HeaderValue) -> Result<&str, GatewayError> {
    let value = value.to_str().map_err(|_| client_unauthorized_error())?;
    let Some(presented_key) = value.strip_prefix("Bearer ") else {
        return Err(client_unauthorized_error());
    };
    valid_presented_key(presented_key)
}

fn presented_x_api_key_value(value: &header::HeaderValue) -> Result<&str, GatewayError> {
    let presented_key = value.to_str().map_err(|_| client_unauthorized_error())?;
    valid_presented_key(presented_key)
}

fn valid_presented_key(presented_key: &str) -> Result<&str, GatewayError> {
    if presented_key.is_empty() || presented_key.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(client_unauthorized_error());
    }

    Ok(presented_key)
}

const fn error_status(error: &GatewayError) -> StatusCode {
    match error.code() {
        GatewayErrorCode::ClientRequestError => StatusCode::BAD_REQUEST,
        GatewayErrorCode::TokenCountUnsupported => StatusCode::UNPROCESSABLE_ENTITY,
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

const fn route_not_found() -> GatewayError {
    GatewayError::new(GatewayErrorCode::RouteNotFound, ErrorScope::Model)
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
            Arc, Mutex,
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
        AccessGroupId, CanonicalEvent, ClientKeyId, EndpointId, ErrorScope, ExactInputTokenCount,
        GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink, GatewayProtocol,
        MessageEnd, MessageRole, MessageStart, PublicModelId, RawExtensions, RawJson,
        ReasoningDelta, RequestContext, RequestId, ResponseEnd, ResponseId, ResponseStart,
        RouteCandidateId, RouteId, StreamError, TextDelta, UpstreamId, Usage, UsageDelta,
    };
    use gateway_observability::{BoundedEventQueue, EventQueueConfig};
    use gateway_router::{
        CapabilitySet, CountTokensExecution, CountTokensExecutor, CountTokensFuture,
        DeterministicMockEmission, DeterministicMockResponsesExecutor, ResponsesEventSource,
        ResponsesExecutor, ResponsesFuture, RouteSnapshot, RouteSnapshotInput,
        RouteSnapshotRegistry, SnapshotAccessGroup, SnapshotCatalogAdmission,
        SnapshotClientKeyAuthenticator, SnapshotClientKeyView, SnapshotPublicModel, SnapshotRoute,
        SnapshotRouteCandidate, SnapshotRouteCandidateInput, SnapshotRoutePolicy,
        SnapshotTransformMode, SnapshotVersion,
    };
    use gateway_stream::{FirstSemanticEventTracker, StreamCapacity, bounded_canonical_stream};
    use protocol_openai_responses::{OpenAiResponseMetadata, OpenAiResponsesSseEncoder};

    use super::{
        JsonDeliveryBody, ResponsesHttpState, ResponsesMetadataFactory, client_request_error,
        configure,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    const TEST_CLIENT_KEY: &str = "p1-test-client-key";
    const SNAPSHOT_PUBLIC_MODEL: &str = "public-model";
    const SNAPSHOT_MODEL_ALIAS: &str = "client-model-alias";

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

    fn text_events_with_final_usage() -> Result<Vec<CanonicalEvent>, Box<dyn Error>> {
        let mut events = text_events()?;
        events.insert(
            3,
            CanonicalEvent::UsageDelta(UsageDelta {
                usage: Usage {
                    input_tokens: Some(3),
                    output_tokens: Some(5),
                    reasoning_tokens: Some(2),
                    ..Usage::default()
                },
                is_final: true,
                extensions: RawExtensions::default(),
            }),
        );
        Ok(events)
    }

    fn anthropic_events() -> Result<Vec<CanonicalEvent>, Box<dyn Error>> {
        Ok(vec![
            response_start()?,
            CanonicalEvent::UsageDelta(UsageDelta {
                usage: Usage {
                    input_tokens: Some(3),
                    cache_read_tokens: Some(2),
                    cache_creation_tokens: Some(1),
                    ..Usage::default()
                },
                is_final: false,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::ReasoningDelta(ReasoningDelta {
                text: "deterministic thinking".to_owned(),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::TextDelta(TextDelta {
                text: "deterministic hello".to_owned(),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageEnd(MessageEnd::default()),
            CanonicalEvent::UsageDelta(UsageDelta {
                usage: Usage {
                    output_tokens: Some(5),
                    ..Usage::default()
                },
                is_final: true,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::ResponseEnd(ResponseEnd {
                stop_reason: Some("max_tokens".to_owned()),
                stop_sequence: Some("test-stop-sequence".to_owned()),
                extensions: RawExtensions::default(),
            }),
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

    fn mock_state_with_event_sink(
        events: Vec<CanonicalEvent>,
        event_sink: Arc<dyn GatewayEventSink>,
    ) -> Result<ResponsesHttpState, Box<dyn Error>> {
        let emissions = events
            .into_iter()
            .map(|event| DeterministicMockEmission::new(Duration::ZERO, event))
            .collect();
        let executor = DeterministicMockResponsesExecutor::try_new(
            gateway_core::ProviderId::try_new("http-observed-provider")?,
            emissions,
        )?;

        Ok(ResponsesHttpState::with_metadata_and_event_sink(
            Arc::new(executor),
            Arc::new(FixedMetadata),
            test_authenticator()?,
            event_sink,
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
        snapshot_auth_state_with_executor(Arc::new(executor))
    }

    fn snapshot_auth_state_with_event_sink(
        events: Vec<CanonicalEvent>,
        event_sink: Arc<dyn GatewayEventSink>,
    ) -> Result<(ResponsesHttpState, String), Box<dyn Error>> {
        let emissions = events
            .into_iter()
            .map(|event| DeterministicMockEmission::new(Duration::ZERO, event))
            .collect();
        let executor = DeterministicMockResponsesExecutor::try_new(
            gateway_core::ProviderId::try_new("http-snapshot-observed-provider")?,
            emissions,
        )?;
        snapshot_auth_state_with_executor_and_event_sink(Arc::new(executor), event_sink)
    }

    fn snapshot_auth_state_with_executor(
        executor: Arc<dyn ResponsesExecutor>,
    ) -> Result<(ResponsesHttpState, String), Box<dyn Error>> {
        snapshot_auth_state_with_executor_and_event_sink(
            executor,
            Arc::new(gateway_core::NoopGatewayEventSink),
        )
    }

    fn snapshot_auth_state_with_executor_and_event_sink(
        executor: Arc<dyn ResponsesExecutor>,
        event_sink: Arc<dyn GatewayEventSink>,
    ) -> Result<(ResponsesHttpState, String), Box<dyn Error>> {
        let service = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0xA5_u8; 32])?);
        let access_group_id = AccessGroupId::try_new("http-snapshot-access-group")?;
        let issued = service.issue(
            ClientKeyId::try_new("http-snapshot-client-key")?,
            access_group_id.clone(),
            None,
        )?;
        let (record, presented_key) = issued.into_parts();
        let public_model_id = PublicModelId::try_new("http-snapshot-public-model")?;
        let route_id = RouteId::try_new("http-snapshot-route")?;
        let candidate = SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new("http-snapshot-candidate")?,
            endpoint_id: EndpointId::try_new("http-snapshot-endpoint")?,
            upstream_id: UpstreamId::try_new("http-snapshot-upstream")?,
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: "sensitive-upstream-model".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::AllowedUnlisted,
            active_binding_count: 1,
        });
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new("http-snapshot-version")?,
            vec![SnapshotPublicModel::new(
                public_model_id.clone(),
                SNAPSHOT_PUBLIC_MODEL.to_owned(),
                "HTTP Snapshot Public Model".to_owned(),
                CapabilitySet::empty(),
                route_id.clone(),
            )],
            vec![(SNAPSHOT_MODEL_ALIAS.to_owned(), public_model_id.clone())],
            vec![SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                SnapshotRoutePolicy::RoundRobin,
                1,
                1_000,
                vec![candidate],
            )],
            vec![SnapshotAccessGroup::new(
                access_group_id,
                "HTTP Snapshot Access Group".to_owned(),
                BTreeSet::from([route_id.clone()]),
            )],
            vec![SnapshotClientKeyView::new(
                record,
                BTreeSet::from([route_id]),
            )],
        ))?);
        let authenticator = Arc::new(SnapshotClientKeyAuthenticator::new(
            Arc::new(RouteSnapshotRegistry::new(snapshot)),
            service,
        ));
        let state = ResponsesHttpState::with_snapshot_metadata_and_event_sink(
            executor,
            Arc::new(FixedMetadata),
            authenticator,
            event_sink,
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
    async fn base_url_head_probe_is_public_and_empty() -> TestResult {
        let state = mock_state(text_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::default()
                .method(actix_web::http::Method::HEAD)
                .uri("/")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(test::read_body(response).await.is_empty());
        Ok(())
    }

    #[actix_web::test]
    async fn responses_rejects_ambiguous_or_invalid_client_key_inputs_before_decode_or_provider_execution()
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
            test::TestRequest::post()
                .uri("/v1/responses")
                .append_header(("x-api-key", TEST_CLIENT_KEY))
                .append_header(("x-api-key", "another-test-key"))
                .set_payload("not-json")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header((header::AUTHORIZATION, format!("Bearer {TEST_CLIENT_KEY}")))
                .insert_header(("x-api-key", TEST_CLIENT_KEY))
                .set_payload("not-json")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("x-api-key", "unknown-test-key"))
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
    async fn messages_accepts_anthropic_x_api_key_without_bearer() -> TestResult {
        let state = mock_state(anthropic_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header(("x-api-key", TEST_CLIENT_KEY))
            .set_payload(r#"{"model":"mock-model","max_tokens":1,"messages":[{"role":"user","content":"hello"}]}"#)
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(&test::read_body(response).await)?;
        assert_eq!(
            body.pointer("/content/1/text"),
            Some(&serde_json::json!("deterministic hello"))
        );
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
    async fn non_streaming_messages_preserves_thinking_cache_usage_and_explicit_stop_semantics()
    -> TestResult {
        let state = mock_state(anthropic_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(test::TestRequest::post().uri("/v1/messages").set_payload(
            r#"{
                    "model":"mock-model",
                    "max_tokens":1,
                    "thinking":{"type":"enabled","budget_tokens":1},
                    "system":[{"type":"text","text":"system","cache_control":{"type":"ephemeral"}}],
                    "messages":[{"role":"user","content":"hello"}]
                }"#,
        ))
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
        let body: serde_json::Value = serde_json::from_slice(&test::read_body(response).await)?;
        assert_eq!(
            body.pointer("/model"),
            Some(&serde_json::json!("mock-model"))
        );
        assert_eq!(
            body.pointer("/content/0/type"),
            Some(&serde_json::json!("thinking"))
        );
        assert_eq!(
            body.pointer("/content/0/thinking"),
            Some(&serde_json::json!("deterministic thinking"))
        );
        assert_eq!(
            body.pointer("/content/1/text"),
            Some(&serde_json::json!("deterministic hello"))
        );
        assert_eq!(
            body.pointer("/stop_reason"),
            Some(&serde_json::json!("max_tokens"))
        );
        assert_eq!(
            body.pointer("/stop_sequence"),
            Some(&serde_json::json!("test-stop-sequence"))
        );
        assert_eq!(
            body.pointer("/usage/cache_read_input_tokens"),
            Some(&serde_json::json!(2))
        );
        assert_eq!(
            body.pointer("/usage/cache_creation_input_tokens"),
            Some(&serde_json::json!(1))
        );
        assert_eq!(
            body.pointer("/usage/output_tokens"),
            Some(&serde_json::json!(5))
        );
        Ok(())
    }

    #[actix_web::test]
    async fn streaming_messages_emits_anthropic_thinking_cache_and_explicit_stop_frames()
    -> TestResult {
        let state = mock_state(anthropic_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post().uri("/v1/messages").set_payload(
                r#"{"model":"mock-model","max_tokens":1,"messages":[{"role":"user","content":"hello"}],"stream":true}"#,
            ),
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
        assert!(body.contains("event: message_start"));
        assert!(body.contains(r#""cache_read_input_tokens":2"#));
        assert!(body.contains(r#""type":"thinking""#));
        assert!(body.contains(r#""type":"thinking_delta""#));
        assert!(body.contains(r#""stop_reason":"max_tokens""#));
        assert!(body.contains(r#""stop_sequence":"test-stop-sequence""#));
        assert!(body.contains("event: message_stop"));
        Ok(())
    }

    #[actix_web::test]
    async fn snapshot_messages_force_map_alias_and_emit_anthropic_request_protocol() -> TestResult {
        let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(2, 1)?)?;
        let event_sink: Arc<dyn GatewayEventSink> = Arc::new(queue);
        let (state, presented_key) =
            snapshot_auth_state_with_event_sink(anthropic_events()?, event_sink)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header((header::AUTHORIZATION, format!("Bearer {presented_key}")))
            .set_payload(format!(
                r#"{{"model":"{SNAPSHOT_MODEL_ALIAS}","max_tokens":1,"messages":[{{"role":"user","content":"hello"}}]}}"#
            ))
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""model":"public-model""#));
        assert!(!body.contains(SNAPSHOT_MODEL_ALIAS));
        assert!(!body.contains("sensitive-upstream-model"));

        let Some(GatewayEvent::Request(event)) = receiver.try_recv() else {
            return Err("expected Anthropic Request event".into());
        };
        assert_eq!(event.protocol(), GatewayProtocol::AnthropicMessages);
        assert_eq!(event.requested_model(), SNAPSHOT_MODEL_ALIAS);
        assert_eq!(event.public_model(), SNAPSHOT_PUBLIC_MODEL);
        assert!(!event.streaming());
        Ok(())
    }

    #[actix_web::test]
    async fn non_streaming_responses_emit_correlated_request_and_final_usage_events() -> TestResult
    {
        let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(2, 1)?)?;
        let queue = Arc::new(queue);
        let event_sink: Arc<dyn GatewayEventSink> = queue.clone();
        let state = mock_state_with_event_sink(text_events_with_final_usage()?, event_sink)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"must-not-enter-events"}"#),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let _body = test::read_body(response).await;

        let Some(GatewayEvent::Request(request_event)) = receiver.try_recv() else {
            return Err("expected Request event".into());
        };
        assert_eq!(request_event.request_id().as_str(), "http-test-request");
        assert_eq!(
            request_event.client_key_id().as_str(),
            "http-test-client-key"
        );
        assert_eq!(request_event.requested_model(), "mock-model");
        assert_eq!(request_event.public_model(), "mock-model");
        assert!(!request_event.streaming());
        let Some(GatewayEvent::Usage(usage_event)) = receiver.try_recv() else {
            return Err("expected final Usage event".into());
        };
        assert_eq!(usage_event.request_id(), request_event.request_id());
        assert_eq!(usage_event.response_id().as_str(), "http-test-response");
        assert_eq!(usage_event.usage().input_tokens, Some(3));
        assert_eq!(usage_event.usage().output_tokens, Some(5));
        assert_eq!(usage_event.usage().reasoning_tokens, Some(2));
        assert!(receiver.try_recv().is_none());
        assert_eq!(queue.metrics().required_queue_full, 0);
        Ok(())
    }

    #[actix_web::test]
    async fn saturated_event_queue_cannot_block_a_streaming_response() -> TestResult {
        let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        let queue = Arc::new(queue);
        let event_sink: Arc<dyn GatewayEventSink> = queue.clone();
        let state = mock_state_with_event_sink(text_events_with_final_usage()?, event_sink)?;
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
        assert!(body.contains("response.completed"));
        assert_eq!(queue.metrics().required_queue_full, 1);
        assert!(matches!(
            receiver.try_recv(),
            Some(GatewayEvent::Request(_))
        ));
        assert!(receiver.try_recv().is_none());
        Ok(())
    }

    #[actix_web::test]
    async fn snapshot_models_list_uses_only_the_pinned_public_model_view() -> TestResult {
        let calls = Arc::new(AtomicUsize::new(0));
        let (state, presented_key) =
            snapshot_auth_state_with_executor(Arc::new(CountingExecutor {
                calls: calls.clone(),
            }))?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let unauthorized = test::call_service(
            &app,
            test::TestRequest::get().uri("/v1/models").to_request(),
        )
        .await;
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            unauthorized
                .headers()
                .get(header::WWW_AUTHENTICATE)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer")
        );
        let request = test::TestRequest::get()
            .uri("/v1/models")
            .insert_header((header::AUTHORIZATION, format!("Bearer {presented_key}")))
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""object":"list""#));
        assert!(body.contains(r#""id":"public-model""#));
        assert!(body.contains(r#""owned_by":"gateway""#));
        assert!(!body.contains(SNAPSHOT_MODEL_ALIAS));
        assert!(!body.contains("sensitive-upstream-model"));
        assert!(!body.contains("http-snapshot-endpoint"));
        assert_eq!(calls.load(Ordering::Acquire), 0);
        Ok(())
    }

    #[actix_web::test]
    async fn models_fails_closed_without_snapshot_authentication() -> TestResult {
        let state = mock_state(text_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(test::TestRequest::get().uri("/v1/models")).to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""code":"RouteNotFound""#));
        Ok(())
    }

    #[actix_web::test]
    async fn snapshot_responses_force_maps_aliases_to_the_public_model_name() -> TestResult {
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
            .set_payload(format!(
                r#"{{"model":"{SNAPSHOT_MODEL_ALIAS}","input":"hello"}}"#
            ))
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""status":"completed""#));
        assert!(body.contains(r#""model":"public-model""#));
        assert!(!body.contains(SNAPSHOT_MODEL_ALIAS));
        assert!(!body.contains("sensitive-upstream-model"));
        Ok(())
    }

    #[actix_web::test]
    async fn snapshot_request_event_retains_access_group_and_force_mapped_public_model()
    -> TestResult {
        let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        let event_sink: Arc<dyn GatewayEventSink> = Arc::new(queue);
        let (state, presented_key) =
            snapshot_auth_state_with_event_sink(text_events()?, event_sink)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header((header::AUTHORIZATION, format!("Bearer {presented_key}")))
            .set_payload(format!(
                r#"{{"model":"{SNAPSHOT_MODEL_ALIAS}","input":"hello"}}"#
            ))
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let _body = test::read_body(response).await;

        let Some(GatewayEvent::Request(event)) = receiver.try_recv() else {
            return Err("expected Snapshot Request event".into());
        };
        assert_eq!(event.client_key_id().as_str(), "http-snapshot-client-key");
        assert_eq!(
            event.access_group_id().map(AccessGroupId::as_str),
            Some("http-snapshot-access-group")
        );
        assert_eq!(event.requested_model(), SNAPSHOT_MODEL_ALIAS);
        assert_eq!(event.public_model(), SNAPSHOT_PUBLIC_MODEL);
        assert_eq!(event.route_alias(), Some(SNAPSHOT_MODEL_ALIAS));
        Ok(())
    }

    #[actix_web::test]
    async fn snapshot_responses_force_maps_aliases_in_every_sse_response_object() -> TestResult {
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
            .set_payload(format!(
                r#"{{"model":"{SNAPSHOT_MODEL_ALIAS}","input":"hello","stream":true}}"#
            ))
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert_eq!(body.matches(r#""model":"public-model""#).count(), 3);
        assert!(!body.contains(SNAPSHOT_MODEL_ALIAS));
        assert!(!body.contains("sensitive-upstream-model"));
        Ok(())
    }

    #[actix_web::test]
    async fn snapshot_responses_rejects_a_non_visible_model_before_executor_start() -> TestResult {
        let calls = Arc::new(AtomicUsize::new(0));
        let (state, presented_key) =
            snapshot_auth_state_with_executor(Arc::new(CountingExecutor {
                calls: calls.clone(),
            }))?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header((header::AUTHORIZATION, format!("Bearer {presented_key}")))
            .set_payload(r#"{"model":"not-visible","input":"hello"}"#)
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""code":"RouteNotFound""#));
        assert_eq!(calls.load(Ordering::Acquire), 0);
        Ok(())
    }

    #[actix_web::test]
    async fn count_tokens_returns_only_an_exact_value_and_passes_snapshot_route_identity()
    -> TestResult {
        let response_calls = Arc::new(AtomicUsize::new(0));
        let (state, presented_key) =
            snapshot_auth_state_with_executor(Arc::new(CountingExecutor {
                calls: response_calls.clone(),
            }))?;
        let observed = Arc::new(Mutex::new(Vec::new()));
        let state = state.with_count_tokens_executor(Arc::new(RecordingExactCounter {
            observed: observed.clone(),
        }));
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = test::TestRequest::post()
            .uri("/v1/messages/count_tokens")
            .insert_header((header::AUTHORIZATION, format!("Bearer {presented_key}")))
            .set_payload(format!(
                r#"{{"model":"{SNAPSHOT_MODEL_ALIAS}","messages":[{{"role":"user","content":"count this"}}]}}"#
            ))
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
        let body = test::read_body(response).await;
        assert_eq!(body.as_ref(), br#"{"input_tokens":17}"#);
        assert_eq!(response_calls.load(Ordering::Acquire), 0);
        assert_eq!(
            observed
                .lock()
                .map_err(|_| "count-token audit lock poisoned")?
                .as_slice(),
            [CountTokensObservation {
                requested_model: SNAPSHOT_MODEL_ALIAS.to_owned(),
                route_id: Some("http-snapshot-route".to_owned()),
            }]
        );
        Ok(())
    }

    #[actix_web::test]
    async fn count_tokens_rejects_default_unsupported_capability_without_an_estimate() -> TestResult
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
                .uri("/v1/messages/count_tokens")
                .set_payload(
                    r#"{"model":"mock-model","messages":[{"role":"user","content":"count this"}]}"#,
                ),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let body = test::read_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(
            body,
            serde_json::json!({
                "type": "error",
                "error": {
                    "type": "invalid_request_error",
                    "message": "the selected route cannot accurately count tokens"
                }
            })
        );
        assert!(body.get("input_tokens").is_none());
        assert!(body.pointer("/error/code").is_none());
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
        let mut body = Box::pin(super::ProtocolSseBody::new(
            stream,
            OpenAiResponsesSseEncoder::new(OpenAiResponseMetadata::try_new("mock-model", 1)?),
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

    #[derive(Debug, Eq, PartialEq)]
    struct CountTokensObservation {
        requested_model: String,
        route_id: Option<String>,
    }

    struct RecordingExactCounter {
        observed: Arc<Mutex<Vec<CountTokensObservation>>>,
    }

    impl CountTokensExecutor for RecordingExactCounter {
        fn count_tokens(
            &self,
            execution: CountTokensExecution,
        ) -> CountTokensFuture<'_, Result<ExactInputTokenCount, GatewayError>> {
            let observation = CountTokensObservation {
                requested_model: execution.request().requested_model.clone(),
                route_id: execution
                    .route_id()
                    .map(|route_id| route_id.as_str().to_owned()),
            };
            let result = self.observed.lock().map_or_else(
                |_| {
                    Err(GatewayError::new(
                        GatewayErrorCode::InternalError,
                        ErrorScope::Internal,
                    ))
                },
                |mut observed| {
                    observed.push(observation);
                    Ok(ExactInputTokenCount::new(17))
                },
            );
            Box::pin(async move { result })
        }
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
