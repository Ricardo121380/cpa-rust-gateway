//! `Actix Web` transport shell. Core crates must not depend on this crate.
//!
//! The public data plane exposes health/model discovery plus Client Key-protected Chat
//! Completions, Responses, and Messages boundaries. Request bytes are decoded by protocol adapters
//! rather than Actix's JSON extractor so duplicate JSON member names remain observable and
//! rejectable.

#![deny(unsafe_code)]

/// Backend-only Codex OAuth session state and replay-safe lifecycle.
pub mod codex_oauth_management;
/// Protected P10 encrypted-backup preflight and empty-target restore handlers.
pub mod management_backup_resources;
/// Protected P10 Config Version lifecycle and lifecycle-audit handlers.
pub mod management_lifecycle_resources;
/// Protected P12 read-only bounded Prometheus exposition for the management listener.
pub mod management_observability_resources;
/// Protected P10 draft-resource handlers for Upstreams, Endpoints, Credentials, and Egress.
pub mod management_resources;
/// Independent management HTTP authentication, network, audit-identity, and browser boundary.
pub mod management_security;
/// Embedded static management SPA resources, configured separately from public inference routes.
pub mod management_ui_resources;
mod stored_response_continuity;

use std::{
    collections::VecDeque,
    convert::Infallible,
    fmt,
    pin::Pin,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use actix_web::{
    HttpRequest, HttpResponse,
    body::{BodySize, MessageBody},
    http::{StatusCode, header},
    web,
};
use actix_ws::{
    CloseCode, CloseReason, Item as WebSocketItem, Message as WebSocketMessage,
    ProtocolError as WebSocketProtocolError,
};
use futures_util::{Stream, StreamExt, stream};
use gateway_auth::{AuthenticatedClient, ClientKeyAuthenticator};
use gateway_core::{
    AccessGroupId, CanonicalEvent, CanonicalRequest, CanonicalResponse, ClientKeyId, ErrorScope,
    GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink, GatewayProtocol,
    NoopGatewayEventSink, RequestContext, RequestEvent, RequestId, ResponseId, RouteId,
    StreamError, TransparentRetryGate, TransparentRetryGateFuture, UsageEvent,
};
use gateway_router::{
    CountTokensExecution, CountTokensExecutor, ResponsesClientTransport, ResponsesContinuationKind,
    ResponsesContinuationPin, ResponsesEventSource, ResponsesExecution, ResponsesExecutionLineage,
    ResponsesExecutionLineageRecorder, ResponsesExecutor, ResponsesResponseMode,
    SnapshotAuthenticatedClient, SnapshotClientKeyAuthenticator, UnsupportedCountTokensExecutor,
};
use gateway_store::stored_response::{
    MAX_STORED_RESPONSE_EVENTS, MAX_STORED_RESPONSE_PAYLOAD_BYTES,
    STORED_RESPONSE_COMPACTION_PREFIX, SqliteStoredResponseStore, StoredResponseCompactionPayload,
    StoredResponseCredentialBinding, StoredResponseLineage, StoredResponsePayload,
    StoredResponseRecord, StoredResponseStoreError, StoredResponseTarget,
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
use protocol_openai_chat::{
    ChatResponseMetadata, ChatSseEncoder, ChatSseFrame, ResponseMode as ChatResponseMode,
    decode_request as decode_chat_request, encode_error as encode_chat_error,
    encode_response as encode_chat_response,
};
use protocol_openai_responses::{
    DecodedResponsesRequest, OpenAiResponseMetadata, OpenAiResponsesSseEncoder, ResponseMode,
    SseFrame as OpenAiSseFrame, decode_compact_request, decode_request, decode_websocket_request,
    encode_compaction_response, encode_error, encode_model_list, encode_response,
};

use crate::stored_response_continuity::{
    compaction_request, continuation_pin, extract_compaction_summary, replay_canonical_response,
    replay_compaction, replay_stored_response,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-http-actix";

/// The P1 default maximum number of canonical events buffered between source and HTTP body.
pub const DEFAULT_STREAM_CAPACITY: usize = 8;

/// The byte-idle interval after which a streaming SSE body writes one non-semantic keepalive.
///
/// A long thinking pause or tool gap can leave a canonical stream silent for a minute or more,
/// and a byte-silent connection is what intermediaries reap: nginx's default `proxy_read_timeout`
/// is 60 seconds and Cloudflare closes a silent proxied response at roughly 100 seconds. Fifteen
/// seconds fits at least three comments inside the tightest of those windows even when one tick
/// is delayed by a busy runtime, and costs 13 bytes per idle stream per tick.
pub const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// Maximum size of one public Responses WebSocket frame and reassembled text message.
pub const RESPONSES_WEBSOCKET_MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum number of fragments accepted for one text message, including empty fragments.
pub const RESPONSES_WEBSOCKET_MAX_FRAGMENTS: usize = 64;
/// Maximum number of complete Responses turns retained by one WebSocket connection.
pub const RESPONSES_WEBSOCKET_MAX_SESSION_TURNS: usize = 16;
/// Maximum combined Canonical request/response bytes retained by one WebSocket connection.
pub const RESPONSES_WEBSOCKET_MAX_SESSION_BYTES: usize = 32 * 1024 * 1024;
/// Maximum time a single downstream WebSocket write may remain backpressured.
pub const RESPONSES_WEBSOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(15);
/// Maximum silence between Canonical events for an active WebSocket turn.
pub const RESPONSES_WEBSOCKET_EVENT_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
/// Maximum wall-clock duration of one WebSocket turn.
pub const RESPONSES_WEBSOCKET_TURN_TIMEOUT: Duration = Duration::from_mins(10);
/// Ping cadence for a public Responses WebSocket connection.
pub const RESPONSES_WEBSOCKET_PING_INTERVAL: Duration = Duration::from_secs(15);
/// Maximum time without a client Pong before closing the connection.
pub const RESPONSES_WEBSOCKET_PONG_TIMEOUT: Duration = Duration::from_secs(45);
/// Maximum idle duration while no turn is active.
pub const RESPONSES_WEBSOCKET_IDLE_TIMEOUT: Duration = Duration::from_mins(5);
/// Maximum total lifetime of one public Responses WebSocket connection.
pub const RESPONSES_WEBSOCKET_SESSION_TIMEOUT: Duration = Duration::from_hours(2);

/// The exact bytes written when a streaming SSE body has been byte-idle for one interval.
///
/// A line starting with `:` is an SSE comment and the blank line after it ends a record whose data
/// buffer is empty, so a conformant client dispatches no event for it. The comment carries no
/// [`CanonicalEvent`], which is why it can never commit the `FirstSemanticEvent` boundary.
const SSE_KEEPALIVE_COMMENT: &[u8] = b": keepalive\n\n";

/// The maximum inbound inference request body the public data plane accepts.
///
/// Actix's default `PayloadConfig` limit is 256 KiB, which rejects realistic long-session Claude
/// Code and Codex bodies before any handler runs. The data-plane handlers therefore read their
/// body with an explicit bounded loop instead of `web::Bytes`, so this bound is enforced by the
/// route that was called and its overflow keeps that route's protocol error envelope. The value
/// is a fixed crate constant rather than composition state: no `App` can silently restore the
/// 256 KiB default by forgetting to register it.
///
/// 4 MiB comfortably exceeds a full 200k-token conversation once JSON escaping and the envelope
/// are counted, while keeping the product of this bound and a deployment's connection ceiling
/// inside its memory limit: a composition that raises one must check the other.
pub const MAX_INFERENCE_REQUEST_BODY_BYTES: usize = 4 * 1024 * 1024;

/// The maximum time the data plane spends receiving one inbound inference body.
///
/// Without this, a client that opens a request and then stalls mid-body parks a handler holding
/// its partial buffer indefinitely; Actix's own client timeout covers only the request head.
const INFERENCE_REQUEST_BODY_TIMEOUT: Duration = Duration::from_secs(30);

/// The longest a gateway-owned compaction execution may retain its exact lineage lease.
///
/// The request itself also carries a fixed output-token bound, while this wall-clock bound closes
/// the case where an admitted upstream response starts but never completes.
const STORED_RESPONSE_COMPACTION_TOTAL_TIMEOUT: Duration = Duration::from_mins(2);

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

    /// Creates public Chat Completions metadata for the selected client-visible model label.
    ///
    /// # Errors
    ///
    /// Returns a safe error when the implementation cannot supply a valid public model or clock.
    fn chat_metadata(
        &self,
        public_model: &str,
        include_usage: bool,
    ) -> Result<ChatResponseMetadata, GatewayError>;
}

/// The production-default metadata implementation for P1's local vertical slice.
pub struct SystemResponsesMetadataFactory {
    request_namespace: OnceLock<[u8; 16]>,
    next_request_sequence: AtomicU64,
}

impl SystemResponsesMetadataFactory {
    /// Creates a metadata factory whose first request identifier has sequence zero and whose
    /// random process namespace is allocated lazily on the first accepted request.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            request_namespace: OnceLock::new(),
            next_request_sequence: AtomicU64::new(0),
        }
    }

    fn request_namespace(&self) -> Result<&[u8; 16], GatewayError> {
        if let Some(namespace) = self.request_namespace.get() {
            return Ok(namespace);
        }
        let mut generated = [0_u8; 16];
        getrandom::fill(&mut generated).map_err(|_| internal_error())?;
        let _ = self.request_namespace.set(generated);
        self.request_namespace.get().ok_or_else(internal_error)
    }
}

impl Default for SystemResponsesMetadataFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SystemResponsesMetadataFactory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SystemResponsesMetadataFactory")
            .field("request_namespace", &"<redacted>")
            .field(
                "next_request_sequence",
                &self.next_request_sequence.load(Ordering::Relaxed),
            )
            .finish()
    }
}

impl ResponsesMetadataFactory for SystemResponsesMetadataFactory {
    fn request_context(&self) -> Result<RequestContext, GatewayError> {
        use std::fmt::Write as _;

        let namespace = self.request_namespace()?;
        let sequence = self.next_request_sequence.fetch_add(1, Ordering::Relaxed);
        let mut namespace_hex = String::with_capacity(namespace.len() * 2);
        for byte in namespace {
            write!(&mut namespace_hex, "{byte:02x}").map_err(|_| internal_error())?;
        }
        let request_id = RequestId::try_new(format!("p1-request-{namespace_hex}-{sequence}"))
            .map_err(|_| internal_error())?;

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

    fn chat_metadata(
        &self,
        public_model: &str,
        include_usage: bool,
    ) -> Result<ChatResponseMetadata, GatewayError> {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| internal_error())?
            .as_secs();

        ChatResponseMetadata::try_new(public_model, created_at, include_usage)
    }
}

/// Shared Actix application state for public inference endpoints.
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
    stored_responses: Option<Arc<SqliteStoredResponseStore>>,
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
            stored_responses: None,
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
            stored_responses: None,
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

    /// Attaches the encrypted, Client-Key-owned stored Response repository.
    #[must_use]
    pub fn with_stored_response_store(
        mut self,
        stored_responses: Arc<SqliteStoredResponseStore>,
    ) -> Self {
        self.stored_responses = Some(stored_responses);
        self
    }
}

#[derive(Clone)]
struct StoredResponseWriteContext {
    repository: Arc<SqliteStoredResponseStore>,
    client_key_id: ClientKeyId,
    canonical_request: gateway_core::CanonicalRequest,
    public_model: String,
    created_at_seconds: u64,
    created_at_ms: i64,
    execution_lineage: ResponsesExecutionLineage,
}

struct StoredResponseCapture {
    context: StoredResponseWriteContext,
    events: Vec<CanonicalEvent>,
    serialized_bytes: usize,
}

impl StoredResponseCapture {
    fn try_new(
        context: StoredResponseWriteContext,
        first: CanonicalEvent,
    ) -> Result<Self, GatewayError> {
        let request_bytes = serde_json::to_vec(&context.canonical_request)
            .map_err(|_| internal_error())?
            .len();
        let first_bytes = serde_json::to_vec(&first)
            .map_err(|_| internal_error())?
            .len();
        let serialized_bytes = request_bytes
            .checked_add(first_bytes)
            .ok_or_else(internal_error)?;
        if serialized_bytes > MAX_STORED_RESPONSE_PAYLOAD_BYTES {
            return Err(internal_error());
        }
        Ok(Self {
            context,
            events: vec![first],
            serialized_bytes,
        })
    }

    fn push(&mut self, event: CanonicalEvent) -> Result<(), GatewayError> {
        if self.events.len() >= MAX_STORED_RESPONSE_EVENTS {
            return Err(internal_error());
        }
        let event_bytes = serde_json::to_vec(&event)
            .map_err(|_| internal_error())?
            .len();
        self.serialized_bytes = self
            .serialized_bytes
            .checked_add(event_bytes)
            .ok_or_else(internal_error)?;
        if self.serialized_bytes > MAX_STORED_RESPONSE_PAYLOAD_BYTES {
            return Err(internal_error());
        }
        self.events.push(event);
        Ok(())
    }

    fn completed_response(&self) -> Result<CanonicalResponse, GatewayError> {
        CanonicalResponse::try_new(self.events.clone())
    }
}

struct PreparedResponsesExecution {
    execution: ResponsesExecution,
    mode: ResponseMode,
    canonical_request_for_store: Option<CanonicalRequest>,
    lineage_recorder: Option<Arc<ResponsesExecutionLineageRecorder>>,
}

struct PreparedOwnedContinuation {
    request: CanonicalRequest,
    pin: ResponsesContinuationPin,
}

#[derive(Clone)]
struct WebSocketSessionTurn {
    response_id: ResponseId,
    public_model: String,
    request: CanonicalRequest,
    response: CanonicalResponse,
    lineage: ResponsesExecutionLineage,
    retained_bytes: usize,
}

#[derive(Default)]
struct WebSocketSessionCache {
    turns: VecDeque<WebSocketSessionTurn>,
    retained_bytes: usize,
}

impl WebSocketSessionCache {
    fn get(&self, response_id: &ResponseId) -> Option<WebSocketSessionTurn> {
        self.turns
            .iter()
            .find(|turn| &turn.response_id == response_id)
            .cloned()
    }

    fn insert(
        &mut self,
        response_id: ResponseId,
        public_model: String,
        request: CanonicalRequest,
        response: CanonicalResponse,
        lineage: ResponsesExecutionLineage,
    ) -> Result<(), GatewayError> {
        if let Some(previous) = self
            .turns
            .iter()
            .find(|turn| turn.response_id == response_id)
        {
            if previous.public_model == public_model
                && previous.request == request
                && previous.response == response
                && previous.lineage == lineage
            {
                return Ok(());
            }
            return Err(internal_error());
        }
        let request_bytes = serde_json::to_vec(&request)
            .map_err(|_| internal_error())?
            .len();
        let response_bytes = serde_json::to_vec(response.events())
            .map_err(|_| internal_error())?
            .len();
        let retained_bytes = request_bytes
            .checked_add(response_bytes)
            .ok_or_else(internal_error)?;
        if retained_bytes > RESPONSES_WEBSOCKET_MAX_SESSION_BYTES {
            return Err(internal_error());
        }
        while self.turns.len() >= RESPONSES_WEBSOCKET_MAX_SESSION_TURNS
            || self
                .retained_bytes
                .checked_add(retained_bytes)
                .is_none_or(|total| total > RESPONSES_WEBSOCKET_MAX_SESSION_BYTES)
        {
            let previous = self.turns.pop_front().ok_or_else(internal_error)?;
            self.retained_bytes = self
                .retained_bytes
                .checked_sub(previous.retained_bytes)
                .ok_or_else(internal_error)?;
        }
        self.retained_bytes = self
            .retained_bytes
            .checked_add(retained_bytes)
            .ok_or_else(internal_error)?;
        self.turns.push_back(WebSocketSessionTurn {
            response_id,
            public_model,
            request,
            response,
            lineage,
            retained_bytes,
        });
        Ok(())
    }
}

#[derive(Default)]
struct WebSocketFragmentAssembler {
    text: Option<Vec<u8>>,
    fragments: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WebSocketInboundError {
    Protocol,
    Unsupported,
    InvalidText,
    TooLarge,
}

impl fmt::Display for WebSocketInboundError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Protocol => "invalid WebSocket fragment sequence",
            Self::Unsupported => "unsupported WebSocket message type",
            Self::InvalidText => "invalid WebSocket text encoding",
            Self::TooLarge => "WebSocket message exceeds the bounded limit",
        })
    }
}

impl std::error::Error for WebSocketInboundError {}

impl WebSocketInboundError {
    fn close_reason(self) -> CloseReason {
        match self {
            Self::Protocol => CloseReason {
                code: CloseCode::Protocol,
                description: Some("invalid_fragment_sequence".to_owned()),
            },
            Self::Unsupported => CloseReason {
                code: CloseCode::Unsupported,
                description: Some("text_messages_only".to_owned()),
            },
            Self::InvalidText => CloseReason {
                code: CloseCode::Invalid,
                description: Some("invalid_utf8".to_owned()),
            },
            Self::TooLarge => CloseReason {
                code: CloseCode::Size,
                description: Some("message_too_large".to_owned()),
            },
        }
    }
}

impl WebSocketFragmentAssembler {
    fn push_message(
        &mut self,
        message: WebSocketMessage,
    ) -> Result<Option<String>, WebSocketInboundError> {
        match message {
            WebSocketMessage::Text(text) => {
                if self.text.is_some() {
                    return Err(WebSocketInboundError::Protocol);
                }
                if text.len() > RESPONSES_WEBSOCKET_MAX_MESSAGE_BYTES {
                    return Err(WebSocketInboundError::TooLarge);
                }
                Ok(Some(text.to_string()))
            }
            WebSocketMessage::Binary(_) => Err(WebSocketInboundError::Unsupported),
            WebSocketMessage::Continuation(WebSocketItem::FirstText(bytes)) => {
                if self.text.is_some() {
                    return Err(WebSocketInboundError::Protocol);
                }
                self.fragments = 1;
                self.text = Some(bytes.to_vec());
                self.validate_bounds()?;
                Ok(None)
            }
            WebSocketMessage::Continuation(WebSocketItem::FirstBinary(_)) => {
                Err(WebSocketInboundError::Unsupported)
            }
            WebSocketMessage::Continuation(WebSocketItem::Continue(bytes)) => {
                self.append_fragment(&bytes)?;
                Ok(None)
            }
            WebSocketMessage::Continuation(WebSocketItem::Last(bytes)) => {
                self.append_fragment(&bytes)?;
                let complete = self.text.take().ok_or(WebSocketInboundError::Protocol)?;
                self.fragments = 0;
                String::from_utf8(complete)
                    .map(Some)
                    .map_err(|_| WebSocketInboundError::InvalidText)
            }
            WebSocketMessage::Ping(_)
            | WebSocketMessage::Pong(_)
            | WebSocketMessage::Close(_)
            | WebSocketMessage::Nop => Ok(None),
        }
    }

    fn append_fragment(&mut self, bytes: &[u8]) -> Result<(), WebSocketInboundError> {
        let text = self.text.as_mut().ok_or(WebSocketInboundError::Protocol)?;
        self.fragments = self
            .fragments
            .checked_add(1)
            .ok_or(WebSocketInboundError::TooLarge)?;
        if text
            .len()
            .checked_add(bytes.len())
            .is_none_or(|length| length > RESPONSES_WEBSOCKET_MAX_MESSAGE_BYTES)
        {
            return Err(WebSocketInboundError::TooLarge);
        }
        text.extend_from_slice(bytes);
        self.validate_bounds()
    }

    fn validate_bounds(&self) -> Result<(), WebSocketInboundError> {
        if self.fragments > RESPONSES_WEBSOCKET_MAX_FRAGMENTS
            || self
                .text
                .as_ref()
                .is_some_and(|text| text.len() > RESPONSES_WEBSOCKET_MAX_MESSAGE_BYTES)
        {
            return Err(WebSocketInboundError::TooLarge);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OwnedContinuityError {
    NotFound,
    Unavailable,
    Internal,
}

/// Continuation and compaction are exact-lineage operations and never permit a second Attempt.
struct StoredContinuationRetryGate;

impl TransparentRetryGate for StoredContinuationRetryGate {
    fn is_cancelled(&self) -> bool {
        false
    }

    fn allows_transparent_retry(&self) -> bool {
        false
    }

    fn cancelled(&self) -> TransparentRetryGateFuture<'_> {
        Box::pin(std::future::pending())
    }
}

fn prepare_responses_execution(
    context: RequestContext,
    decoded: &DecodedResponsesRequest,
    original_body: &str,
    route_id: Option<RouteId>,
    retry_gate: Arc<dyn TransparentRetryGate>,
    owned_continuation: Option<PreparedOwnedContinuation>,
    require_lineage: bool,
) -> PreparedResponsesExecution {
    let mode = decoded.mode;
    let (request, continuation_pin) = owned_continuation.map_or_else(
        || (decoded.request.clone(), None),
        |owned| (owned.request, Some(owned.pin)),
    );
    let canonical_request_for_store = decoded.store.then(|| request.clone());
    let lineage_recorder = (decoded.store || require_lineage)
        .then(|| Arc::new(ResponsesExecutionLineageRecorder::new()));
    let response_mode = match mode {
        ResponseMode::NonStreaming => ResponsesResponseMode::NonStreaming,
        ResponseMode::Streaming => ResponsesResponseMode::Streaming,
    };
    let mut execution = if let Some(pin) = continuation_pin.as_ref() {
        ResponsesExecution::new(
            context,
            request,
            Some(pin.lineage().route_id().clone()),
            response_mode,
            retry_gate,
        )
    } else {
        let native_payload: Arc<[u8]> = decoded.normalized_native_payload().map_or_else(
            || Arc::from(original_body.as_bytes()),
            |payload| Arc::from(payload.to_vec()),
        );
        ResponsesExecution::new_for_protocol(
            context,
            request,
            gateway_router::ProtocolFormat::OpenAiResponses,
            native_payload,
            route_id,
            response_mode,
            retry_gate,
        )
    };
    if let Some(pin) = continuation_pin {
        execution = execution.with_continuation_pin(pin);
    }
    if let Some(recorder) = lineage_recorder.as_ref() {
        execution = execution.with_lineage_recorder(Arc::clone(recorder));
    }
    PreparedResponsesExecution {
        execution,
        mode,
        canonical_request_for_store,
        lineage_recorder,
    }
}

fn prepare_stored_response_write_context(
    state: &ResponsesHttpState,
    canonical_request: Option<CanonicalRequest>,
    lineage_recorder: Option<Arc<ResponsesExecutionLineageRecorder>>,
    client_key_id: ClientKeyId,
    public_model: String,
    created_at_seconds: u64,
) -> Result<Option<StoredResponseWriteContext>, GatewayError> {
    let Some(canonical_request) = canonical_request else {
        return Ok(None);
    };
    let repository = state.stored_responses.clone().ok_or_else(internal_error)?;
    let lineage = lineage_recorder
        .ok_or_else(internal_error)?
        .lineage()?
        .ok_or_else(internal_error)?;
    Ok(Some(StoredResponseWriteContext {
        repository,
        client_key_id,
        canonical_request,
        public_model,
        created_at_seconds,
        created_at_ms: system_now_ms()?,
        execution_lineage: lineage,
    }))
}

async fn prepare_owned_continuation(
    state: &ResponsesHttpState,
    client_key_id: &ClientKeyId,
    public_model: &str,
    current_route_id: Option<&RouteId>,
    decoded: &DecodedResponsesRequest,
) -> Result<Option<PreparedOwnedContinuation>, OwnedContinuityError> {
    if !decoded.requires_gateway_replay() {
        return Ok(None);
    }
    if !state.executor.supports_stored_response_continuity() {
        return Err(OwnedContinuityError::Unavailable);
    }
    let repository = state
        .stored_responses
        .clone()
        .ok_or(OwnedContinuityError::Internal)?;
    let now_ms = system_now_ms().map_err(|_| OwnedContinuityError::Internal)?;

    if let Some(response_id) = decoded.previous_response_id.as_ref() {
        let record = load_owned_response(
            repository,
            client_key_id.clone(),
            response_id.clone(),
            now_ms,
        )
        .await?;
        let payload = record.payload();
        ensure_continuity_identity(
            payload.public_model(),
            payload.lineage(),
            public_model,
            current_route_id,
        )?;
        return Ok(Some(PreparedOwnedContinuation {
            request: replay_stored_response(payload, decoded.request.clone())
                .map_err(|_| OwnedContinuityError::Unavailable)?,
            pin: continuation_pin(payload.lineage(), ResponsesContinuationKind::StoredResponse)
                .map_err(|_| OwnedContinuityError::Internal)?,
        }));
    }

    let compact = decoded
        .compaction
        .as_ref()
        .ok_or(OwnedContinuityError::Internal)?;
    let compact_id = compact.encrypted_content().to_owned();
    let owner = client_key_id.clone();
    let record = tokio::task::spawn_blocking(move || {
        repository.get_compaction_owned(&owner, &compact_id, now_ms)
    })
    .await
    .map_err(|_| OwnedContinuityError::Internal)?
    .map_err(|error| owned_continuity_store_error(&error))?
    .ok_or(OwnedContinuityError::NotFound)?;
    let payload = record.payload();
    ensure_continuity_identity(
        payload.public_model(),
        payload.lineage(),
        public_model,
        current_route_id,
    )?;
    Ok(Some(PreparedOwnedContinuation {
        request: replay_compaction(payload, decoded.request.clone())
            .map_err(|_| OwnedContinuityError::Unavailable)?,
        pin: continuation_pin(payload.lineage(), ResponsesContinuationKind::Compaction)
            .map_err(|_| OwnedContinuityError::Internal)?,
    }))
}

async fn load_owned_response(
    repository: Arc<SqliteStoredResponseStore>,
    client_key_id: ClientKeyId,
    response_id: ResponseId,
    now_ms: i64,
) -> Result<StoredResponseRecord, OwnedContinuityError> {
    tokio::task::spawn_blocking(move || repository.get_owned(&client_key_id, &response_id, now_ms))
        .await
        .map_err(|_| OwnedContinuityError::Internal)?
        .map_err(|error| owned_continuity_store_error(&error))?
        .ok_or(OwnedContinuityError::NotFound)
}

fn owned_continuity_store_error(error: &StoredResponseStoreError) -> OwnedContinuityError {
    match error {
        StoredResponseStoreError::InvalidInput
        | StoredResponseStoreError::InvalidPersistedRecord
        | StoredResponseStoreError::SecretStore(_) => OwnedContinuityError::NotFound,
        StoredResponseStoreError::Store(_)
        | StoredResponseStoreError::PayloadTooLarge
        | StoredResponseStoreError::ConflictingReplay
        | StoredResponseStoreError::RandomnessUnavailable
        | StoredResponseStoreError::TimeOverflow
        | StoredResponseStoreError::InvalidGcLimit
        | StoredResponseStoreError::LockPoisoned => OwnedContinuityError::Internal,
    }
}

fn ensure_continuity_identity(
    stored_public_model: &str,
    lineage: &StoredResponseLineage,
    public_model: &str,
    current_route_id: Option<&RouteId>,
) -> Result<(), OwnedContinuityError> {
    if stored_public_model != public_model
        || current_route_id.is_some_and(|route_id| route_id != lineage.target().route_id())
    {
        return Err(OwnedContinuityError::Unavailable);
    }
    Ok(())
}

/// Creates a validated P1 default bounded-stream capacity.
///
/// # Errors
///
/// Returns a stream-capacity error only if the frozen default is changed to an invalid value.
pub fn default_stream_capacity() -> Result<StreamCapacity, StreamCapacityError> {
    StreamCapacity::try_new(DEFAULT_STREAM_CAPACITY)
}

/// Registers public data-plane routes on an Actix application.
///
/// Management routes are intentionally absent: deployments must mount them only on the separate
/// management listener. [`configure_readiness`] is available for P12 staging before a later task
/// composes an authenticated inference runtime.
pub fn configure(config: &mut web::ServiceConfig) {
    configure_readiness(config);
    config
        .route("/v1/models", web::get().to(models))
        .route("/v1/chat/completions", web::post().to(chat_completions))
        .route("/v1/responses", web::get().to(responses_websocket))
        .route("/v1/responses", web::post().to(responses))
        .route("/v1/responses/compact", web::post().to(compact_responses))
        .route(
            "/v1/responses/{response_id}",
            web::get().to(retrieve_stored_response),
        )
        .route(
            "/v1/responses/{response_id}",
            web::delete().to(delete_stored_response),
        )
        .route("/v1/messages", web::post().to(messages))
        .route("/v1/messages/count_tokens", web::post().to(count_tokens));
}

/// Registers only the public reachability and health routes.
///
/// This is the intentionally limited P12-02 staging surface. It has no application data,
/// authentication, management route, route snapshot, Provider, or credential dependency.
pub fn configure_readiness(config: &mut web::ServiceConfig) {
    config
        // Claude Code probes the configured Anthropic base URL with `HEAD /` before its first
        // Messages request. This says only that the local HTTP boundary is reachable; it reveals
        // no route, model, or authentication state.
        .route("/", web::head().to(base_url_probe))
        .route("/healthz", web::get().to(healthz));
}

/// Registers the complete P10 management listener under one protected `/admin` scope.
///
/// The embedded UI remains a separate closed static route set on this listener. The function has
/// no data-plane routes and requires the caller to supply every corresponding P10 state object;
/// missing state continues to fail closed in the existing handlers and middleware.
pub fn configure_management_listener(config: &mut web::ServiceConfig) {
    management_security::configure_management(config, |protected| {
        management_resources::configure_protected_resource_routes(protected);
        management_lifecycle_resources::configure_protected_lifecycle_routes(protected);
        management_backup_resources::configure_protected_backup_routes(protected);
        management_observability_resources::configure_protected_observability_routes(protected);
    });
    management_ui_resources::configure_embedded_management_ui(config);
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

#[allow(clippy::too_many_lines)] // Keep the complete public upgrade admission boundary auditable.
async fn responses_websocket(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    payload: web::Payload,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(authenticated_client) => Arc::new(authenticated_client),
        Err(error) => return pre_websocket_error(&error),
    };
    if request.headers().contains_key(header::ORIGIN) {
        return HttpResponse::Forbidden()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .content_type("application/json")
            .body(encode_error(&client_request_error()).to_string());
    }
    let turn_state_header = header::HeaderName::from_static("x-codex-turn-state");
    let turn_state = match single_header(&request, turn_state_header.clone()) {
        Ok(value) => value.cloned(),
        Err(_) => return pre_websocket_error(&client_request_error()),
    };
    if turn_state
        .as_ref()
        .is_some_and(|value| value.as_bytes().len() > 512)
    {
        return pre_websocket_error(&client_request_error());
    }
    let Ok((mut response, session, message_stream)) = actix_ws::handle(&request, payload) else {
        return pre_websocket_error(&client_request_error());
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Some(turn_state) = turn_state {
        response.headers_mut().insert(turn_state_header, turn_state);
    }
    let message_stream = message_stream.max_frame_size(RESPONSES_WEBSOCKET_MAX_MESSAGE_BYTES);
    let state = state.clone();
    actix_web::rt::spawn(run_responses_websocket_session(
        session,
        message_stream,
        state,
        authenticated_client,
    ));
    response
}

fn pre_websocket_error(error: &GatewayError) -> HttpResponse {
    let mut response = pre_header_error(error);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

#[allow(clippy::too_many_lines)] // The state machine keeps close, queue, ping, and fragment policy together.
async fn run_responses_websocket_session(
    mut session: actix_ws::Session,
    mut messages: actix_ws::MessageStream,
    state: web::Data<ResponsesHttpState>,
    authenticated_client: Arc<AuthenticatedResponsesClient>,
) {
    let cache = Arc::new(tokio::sync::Mutex::new(WebSocketSessionCache::default()));
    let (done_sender, mut done_receiver) = tokio::sync::mpsc::channel::<()>(1);
    let mut active_turn: Option<tokio::task::JoinHandle<()>> = None;
    let mut queued_turn: Option<String> = None;
    let mut fragments = WebSocketFragmentAssembler::default();
    let mut ping = tokio::time::interval(RESPONSES_WEBSOCKET_PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let started_at = tokio::time::Instant::now();
    let mut last_activity = started_at;
    let mut last_pong = started_at;

    loop {
        tokio::select! {
            () = tokio::time::sleep_until(started_at + RESPONSES_WEBSOCKET_SESSION_TIMEOUT) => {
                close_active_turn(&mut active_turn);
                close_websocket_session(session, Some(websocket_close_reason(
                    CloseCode::Away,
                    "session_timeout",
                ))).await;
                return;
            }
            _ = ping.tick() => {
                let now = tokio::time::Instant::now();
                if now.duration_since(last_pong) > RESPONSES_WEBSOCKET_PONG_TIMEOUT {
                    close_active_turn(&mut active_turn);
                    close_websocket_session(session, Some(websocket_close_reason(
                        CloseCode::Away,
                        "pong_timeout",
                    ))).await;
                    return;
                }
                if active_turn.is_none()
                    && now.duration_since(last_activity) > RESPONSES_WEBSOCKET_IDLE_TIMEOUT
                {
                    close_websocket_session(session, Some(websocket_close_reason(
                        CloseCode::Away,
                        "idle_timeout",
                    ))).await;
                    return;
                }
                if write_websocket_ping(&mut session, b"cpar").await.is_err() {
                    close_active_turn(&mut active_turn);
                    return;
                }
            }
            done = done_receiver.recv(), if active_turn.is_some() => {
                if done.is_none() {
                    close_active_turn(&mut active_turn);
                    return;
                }
                active_turn.take();
                last_activity = tokio::time::Instant::now();
                if let Some(request) = queued_turn.take() {
                    active_turn = Some(spawn_responses_websocket_turn(
                        request,
                        session.clone(),
                        state.clone(),
                        Arc::clone(&authenticated_client),
                        Arc::clone(&cache),
                        done_sender.clone(),
                    ));
                }
            }
            incoming = messages.recv() => {
                let Some(incoming) = incoming else {
                    close_active_turn(&mut active_turn);
                    return;
                };
                let message = match incoming {
                    Ok(message) => message,
                    Err(error) => {
                        close_active_turn(&mut active_turn);
                        close_websocket_session(
                            session,
                            Some(websocket_stream_error_reason(&error)),
                        )
                        .await;
                        return;
                    }
                };
                last_activity = tokio::time::Instant::now();
                match message {
                    WebSocketMessage::Ping(bytes) => {
                        if write_websocket_pong(&mut session, &bytes).await.is_err() {
                            close_active_turn(&mut active_turn);
                            return;
                        }
                    }
                    WebSocketMessage::Pong(_) => {
                        last_pong = tokio::time::Instant::now();
                    }
                    WebSocketMessage::Close(reason) => {
                        close_active_turn(&mut active_turn);
                        close_websocket_session(session, reason).await;
                        return;
                    }
                    WebSocketMessage::Nop => {}
                    message => {
                        let complete = match fragments.push_message(message) {
                            Ok(complete) => complete,
                            Err(error) => {
                                close_active_turn(&mut active_turn);
                                close_websocket_session(session, Some(error.close_reason())).await;
                                return;
                            }
                        };
                        let Some(request) = complete else {
                            continue;
                        };
                        if active_turn.is_none() {
                            active_turn = Some(spawn_responses_websocket_turn(
                                request,
                                session.clone(),
                                state.clone(),
                                Arc::clone(&authenticated_client),
                                Arc::clone(&cache),
                                done_sender.clone(),
                            ));
                        } else if queued_turn.is_none() {
                            queued_turn = Some(request);
                        } else {
                            close_active_turn(&mut active_turn);
                            close_websocket_session(session, Some(websocket_close_reason(
                                CloseCode::Policy,
                                "too_many_pending_requests",
                            ))).await;
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn close_active_turn(active_turn: &mut Option<tokio::task::JoinHandle<()>>) {
    if let Some(active_turn) = active_turn.take() {
        active_turn.abort();
    }
}

fn websocket_close_reason(code: CloseCode, description: &str) -> CloseReason {
    CloseReason {
        code,
        description: Some(description.to_owned()),
    }
}

fn websocket_stream_error_reason(error: &WebSocketProtocolError) -> CloseReason {
    match error {
        WebSocketProtocolError::Overflow => {
            websocket_close_reason(CloseCode::Size, "frame_too_large")
        }
        WebSocketProtocolError::Io(error) if error.kind() == std::io::ErrorKind::InvalidData => {
            websocket_close_reason(CloseCode::Invalid, "invalid_utf8")
        }
        _ => websocket_close_reason(CloseCode::Protocol, "invalid_frame"),
    }
}

async fn close_websocket_session(session: actix_ws::Session, reason: Option<CloseReason>) {
    let _closed =
        tokio::time::timeout(RESPONSES_WEBSOCKET_WRITE_TIMEOUT, session.close(reason)).await;
}

async fn write_websocket_ping(
    session: &mut actix_ws::Session,
    payload: &[u8],
) -> Result<(), GatewayError> {
    tokio::time::timeout(RESPONSES_WEBSOCKET_WRITE_TIMEOUT, session.ping(payload))
        .await
        .map_err(|_| stream_protocol_error())?
        .map_err(|_| stream_protocol_error())
}

async fn write_websocket_pong(
    session: &mut actix_ws::Session,
    payload: &[u8],
) -> Result<(), GatewayError> {
    tokio::time::timeout(RESPONSES_WEBSOCKET_WRITE_TIMEOUT, session.pong(payload))
        .await
        .map_err(|_| stream_protocol_error())?
        .map_err(|_| stream_protocol_error())
}

fn spawn_responses_websocket_turn(
    request: String,
    mut session: actix_ws::Session,
    state: web::Data<ResponsesHttpState>,
    authenticated_client: Arc<AuthenticatedResponsesClient>,
    cache: Arc<tokio::sync::Mutex<WebSocketSessionCache>>,
    done_sender: tokio::sync::mpsc::Sender<()>,
) -> tokio::task::JoinHandle<()> {
    actix_web::rt::spawn(async move {
        let result = tokio::time::timeout(
            RESPONSES_WEBSOCKET_TURN_TIMEOUT,
            execute_responses_websocket_turn(
                &request,
                &mut session,
                &state,
                authenticated_client.as_ref(),
                &cache,
            ),
        )
        .await
        .unwrap_or_else(|_| Err(stream_protocol_error()));
        if let Err(error) = result {
            let _sent = send_websocket_error(&mut session, &error).await;
        }
        let _done = done_sender.send(()).await;
    })
}

#[allow(clippy::too_many_lines)] // Mirrors the POST Responses admission path in one reviewable flow.
async fn execute_responses_websocket_turn(
    body: &str,
    session: &mut actix_ws::Session,
    state: &ResponsesHttpState,
    authenticated_client: &AuthenticatedResponsesClient,
    cache: &tokio::sync::Mutex<WebSocketSessionCache>,
) -> Result<(), GatewayError> {
    let decoded_websocket = decode_websocket_request(body)?;
    let decoded = &decoded_websocket.response;
    if decoded.store
        && (state.stored_responses.is_none() || !state.executor.supports_stored_response_lineage())
    {
        return Err(internal_error());
    }
    let requested_model = decoded.request.requested_model.clone();
    let (public_model, route_alias, route_id) =
        resolve_public_model(authenticated_client, &requested_model)?;
    let context = state.metadata_factory.request_context()?;
    let request_id = context.request_id().clone();
    let (client_key_id, access_group_id) = authenticated_client.event_identity();
    let owned_continuation = prepare_websocket_continuation(
        state,
        cache,
        &client_key_id,
        &public_model,
        route_id.as_ref(),
        decoded,
    )
    .await
    .map_err(owned_continuity_gateway_error)?;
    let _request_event = state
        .event_sink
        .try_emit(GatewayEvent::Request(RequestEvent::new(
            request_id.clone(),
            client_key_id.clone(),
            access_group_id,
            GatewayProtocol::OpenAiResponses,
            requested_model,
            public_model.clone(),
            route_alias,
            true,
        )));
    let (sender, stream) = bounded_canonical_stream(state.stream_capacity);
    let retry_gate: Arc<dyn TransparentRetryGate> = Arc::new(stream.control());
    let normalized_body =
        std::str::from_utf8(decoded_websocket.native_payload()).map_err(|_| internal_error())?;
    let PreparedResponsesExecution {
        mut execution,
        mode: _,
        canonical_request_for_store,
        lineage_recorder,
    } = prepare_responses_execution(
        context,
        decoded,
        normalized_body,
        route_id,
        retry_gate,
        owned_continuation,
        true,
    );
    let canonical_request_for_session = execution.request().clone();
    execution = execution.with_client_transport(ResponsesClientTransport::WebSocket);
    let mut source = state.executor.execute_routed(execution).await?;
    let Some(first @ CanonicalEvent::ResponseStart(_)) = source.next_event().await? else {
        return Err(stream_protocol_error());
    };
    let metadata = state.metadata_factory.response_metadata(&public_model)?;
    let stored_response = prepare_stored_response_write_context(
        state,
        canonical_request_for_store,
        lineage_recorder.clone(),
        client_key_id,
        public_model.clone(),
        metadata.created_at(),
    )?;
    let usage_observer = UsageEventObserver::new(request_id, Arc::clone(&state.event_sink));
    let stream = start_bounded_transport(
        source,
        first,
        sender,
        stream,
        usage_observer,
        stored_response,
    )
    .await?;
    deliver_responses_websocket_turn(
        session,
        stream,
        metadata,
        cache,
        public_model,
        canonical_request_for_session,
        lineage_recorder.ok_or_else(internal_error)?,
    )
    .await
}

async fn prepare_websocket_continuation(
    state: &ResponsesHttpState,
    cache: &tokio::sync::Mutex<WebSocketSessionCache>,
    client_key_id: &ClientKeyId,
    public_model: &str,
    current_route_id: Option<&RouteId>,
    decoded: &DecodedResponsesRequest,
) -> Result<Option<PreparedOwnedContinuation>, OwnedContinuityError> {
    if let Some(response_id) = decoded.previous_response_id.as_ref()
        && let Some(previous) = cache.lock().await.get(response_id)
    {
        if previous.public_model != public_model
            || current_route_id.is_some_and(|route_id| route_id != previous.lineage.route_id())
        {
            return Err(OwnedContinuityError::Unavailable);
        }
        let request = replay_canonical_response(
            &previous.request,
            &previous.response,
            decoded.request.clone(),
        )
        .map_err(|_| OwnedContinuityError::Unavailable)?;
        return Ok(Some(PreparedOwnedContinuation {
            request,
            pin: ResponsesContinuationPin::new(
                previous.lineage,
                ResponsesContinuationKind::WebSocketSession,
            ),
        }));
    }
    prepare_owned_continuation(
        state,
        client_key_id,
        public_model,
        current_route_id,
        decoded,
    )
    .await
}

fn owned_continuity_gateway_error(error: OwnedContinuityError) -> GatewayError {
    match error {
        OwnedContinuityError::NotFound => route_not_found(),
        OwnedContinuityError::Unavailable => GatewayError::new(
            GatewayErrorCode::CredentialUnavailable,
            ErrorScope::Credential,
        ),
        OwnedContinuityError::Internal => internal_error(),
    }
}

#[allow(clippy::too_many_arguments)]
async fn deliver_responses_websocket_turn(
    session: &mut actix_ws::Session,
    mut stream: CanonicalEventStream,
    metadata: OpenAiResponseMetadata,
    cache: &tokio::sync::Mutex<WebSocketSessionCache>,
    public_model: String,
    canonical_request: CanonicalRequest,
    lineage_recorder: Arc<ResponsesExecutionLineageRecorder>,
) -> Result<(), GatewayError> {
    let tracker = stream.control().first_semantic_event_tracker();
    let mut encoder = OpenAiResponsesSseEncoder::new(metadata);
    let mut events = Vec::new();
    let mut canonical_bytes = 0_usize;
    let mut output_bytes = 0_usize;

    loop {
        let event = tokio::time::timeout(RESPONSES_WEBSOCKET_EVENT_IDLE_TIMEOUT, stream.recv())
            .await
            .map_err(|_| stream_protocol_error())??
            .ok_or_else(stream_protocol_error)?;
        if events.len() >= MAX_STORED_RESPONSE_EVENTS {
            return Err(internal_error());
        }
        canonical_bytes = canonical_bytes
            .checked_add(
                serde_json::to_vec(&event)
                    .map_err(|_| internal_error())?
                    .len(),
            )
            .ok_or_else(internal_error)?;
        if canonical_bytes > MAX_STORED_RESPONSE_PAYLOAD_BYTES {
            return Err(internal_error());
        }
        events.push(event.clone());
        let completed = matches!(event, CanonicalEvent::ResponseEnd(_));
        let failed = matches!(event, CanonicalEvent::StreamError(_));
        if completed {
            let response = CanonicalResponse::try_new(events.clone())?;
            let response_id = response
                .events()
                .first()
                .and_then(|event| match event {
                    CanonicalEvent::ResponseStart(start) => Some(start.response_id.clone()),
                    _ => None,
                })
                .ok_or_else(internal_error)?;
            let lineage = lineage_recorder.lineage()?.ok_or_else(internal_error)?;
            cache.lock().await.insert(
                response_id,
                public_model.clone(),
                canonical_request.clone(),
                response,
                lineage,
            )?;
        }
        for frame in encoder.encode_event(&event)? {
            let message = serde_json::to_string(frame.data()).map_err(|_| internal_error())?;
            if message.len() > RESPONSES_WEBSOCKET_MAX_MESSAGE_BYTES {
                return Err(internal_error());
            }
            output_bytes = output_bytes
                .checked_add(message.len())
                .ok_or_else(internal_error)?;
            if output_bytes > MAX_STORED_RESPONSE_PAYLOAD_BYTES {
                return Err(internal_error());
            }
            write_websocket_text(session, message).await?;
            if frame.is_semantic() {
                let _delivery = tracker.mark_delivered(&event);
            }
        }
        if completed || failed {
            return Ok(());
        }
    }
}

async fn write_websocket_text(
    session: &mut actix_ws::Session,
    message: String,
) -> Result<(), GatewayError> {
    tokio::time::timeout(RESPONSES_WEBSOCKET_WRITE_TIMEOUT, session.text(message))
        .await
        .map_err(|_| stream_protocol_error())?
        .map_err(|_| stream_protocol_error())
}

async fn send_websocket_error(
    session: &mut actix_ws::Session,
    error: &GatewayError,
) -> Result<(), GatewayError> {
    let encoded = encode_error(error);
    let payload = serde_json::json!({
        "type": "error",
        "error": encoded.get("error").cloned().unwrap_or(serde_json::Value::Null),
    });
    write_websocket_text(
        session,
        serde_json::to_string(&payload).map_err(|_| internal_error())?,
    )
    .await
}

async fn chat_completions(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    payload: web::Payload,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(authenticated_client) => authenticated_client,
        Err(error) => return pre_header_chat_error(&error),
    };
    let body = match read_bounded_request_body(&request, payload).await {
        Ok(body) => body,
        Err(error) => return chat_request_body_error(error),
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return pre_header_chat_error(&client_request_error());
    };
    let decoded = match decode_chat_request(body) {
        Ok(decoded) => decoded,
        Err(error) => return pre_header_chat_error(&error),
    };
    let requested_model = decoded.request.requested_model.clone();
    let (public_model, route_alias, route_id) =
        match resolve_public_model(&authenticated_client, &decoded.request.requested_model) {
            Ok(resolved) => resolved,
            Err(error) => return pre_header_chat_error(&error),
        };
    let context = match state.metadata_factory.request_context() {
        Ok(context) => context,
        Err(error) => return pre_header_chat_error(&error),
    };
    let request_id = context.request_id().clone();
    let (client_key_id, access_group_id) = authenticated_client.event_identity();
    let _request_event = state
        .event_sink
        .try_emit(GatewayEvent::Request(RequestEvent::new(
            request_id.clone(),
            client_key_id,
            access_group_id,
            GatewayProtocol::OpenAiChatCompletions,
            requested_model,
            public_model.clone(),
            route_alias,
            decoded.mode == ChatResponseMode::Streaming,
        )));
    let (sender, stream) = bounded_canonical_stream(state.stream_capacity);
    let retry_gate: Arc<dyn TransparentRetryGate> = Arc::new(stream.control());
    let response_mode = match decoded.mode {
        ChatResponseMode::NonStreaming => ResponsesResponseMode::NonStreaming,
        ChatResponseMode::Streaming => ResponsesResponseMode::Streaming,
    };
    let execution = ResponsesExecution::new_for_protocol(
        context,
        decoded.request,
        gateway_router::ProtocolFormat::OpenAiChatCompletions,
        Arc::from(body.as_bytes()),
        route_id,
        response_mode,
        retry_gate,
    );
    let mut source = match state.executor.execute_routed(execution).await {
        Ok(source) => source,
        Err(error) => return pre_header_chat_error(&error),
    };
    let first = match source.next_event().await {
        Ok(Some(event @ CanonicalEvent::ResponseStart(_))) => event,
        Ok(Some(_) | None) => return pre_header_chat_error(&stream_protocol_error()),
        Err(error) => return pre_header_chat_error(&error),
    };
    let metadata = match state
        .metadata_factory
        .chat_metadata(&public_model, decoded.include_usage)
    {
        Ok(metadata) => metadata,
        Err(error) => return pre_header_chat_error(&error),
    };
    let usage_observer = UsageEventObserver::new(request_id, Arc::clone(&state.event_sink));

    match decoded.mode {
        ChatResponseMode::NonStreaming => {
            chat_non_streaming_response(source, first, metadata, usage_observer, sender, stream)
                .await
        }
        ChatResponseMode::Streaming => {
            chat_streaming_response(source, first, metadata, usage_observer, sender, stream).await
        }
    }
}

#[allow(clippy::too_many_lines)] // Keep public admission, first-event, and durable-store ordering auditable.
async fn responses(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    payload: web::Payload,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(authenticated_client) => authenticated_client,
        Err(error) => return pre_header_error(&error),
    };
    let body = match read_bounded_request_body(&request, payload).await {
        Ok(body) => body,
        Err(error) => return request_body_error(error),
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return pre_header_error(&client_request_error());
    };
    let decoded = match decode_request(body) {
        Ok(decoded) => decoded,
        Err(error) => return pre_header_error(&error),
    };
    if decoded.store
        && (state.stored_responses.is_none() || !state.executor.supports_stored_response_lineage())
    {
        return pre_header_stored_response_error(&internal_error());
    }
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
    let owned_continuation = match prepare_owned_continuation(
        &state,
        &client_key_id,
        &public_model,
        route_id.as_ref(),
        &decoded,
    )
    .await
    {
        Ok(continuation) => continuation,
        Err(error) => return owned_continuity_error(error),
    };
    let _request_event = state
        .event_sink
        .try_emit(GatewayEvent::Request(RequestEvent::new(
            request_id.clone(),
            client_key_id.clone(),
            access_group_id,
            GatewayProtocol::OpenAiResponses,
            requested_model,
            public_model.clone(),
            route_alias,
            decoded.mode == ResponseMode::Streaming,
        )));
    let (sender, stream) = bounded_canonical_stream(state.stream_capacity);
    let retry_gate: Arc<dyn TransparentRetryGate> = Arc::new(stream.control());
    let PreparedResponsesExecution {
        execution,
        mode,
        canonical_request_for_store,
        lineage_recorder,
    } = prepare_responses_execution(
        context,
        &decoded,
        body,
        route_id,
        retry_gate,
        owned_continuation,
        false,
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
    let stored_response = match prepare_stored_response_write_context(
        &state,
        canonical_request_for_store,
        lineage_recorder,
        client_key_id,
        public_model.clone(),
        metadata.created_at(),
    ) {
        Ok(context) => context,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let usage_observer = UsageEventObserver::new(request_id, Arc::clone(&state.event_sink));

    deliver_responses(
        mode,
        source,
        first,
        metadata,
        usage_observer,
        sender,
        stream,
        stored_response,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn deliver_responses(
    mode: ResponseMode,
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    metadata: OpenAiResponseMetadata,
    usage_observer: UsageEventObserver,
    sender: CanonicalEventSender,
    stream: CanonicalEventStream,
    stored_response: Option<StoredResponseWriteContext>,
) -> HttpResponse {
    match mode {
        ResponseMode::NonStreaming => {
            non_streaming_response(
                source,
                first,
                metadata,
                usage_observer,
                sender,
                stream,
                stored_response,
            )
            .await
        }
        ResponseMode::Streaming => {
            streaming_response(
                source,
                first,
                metadata,
                usage_observer,
                sender,
                stream,
                stored_response,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_lines)] // One linear flow proves owner lookup precedes the sole exact attempt and AEAD write.
async fn compact_responses(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    payload: web::Payload,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(authenticated_client) => authenticated_client,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let body = match read_bounded_request_body(&request, payload).await {
        Ok(body) => body,
        Err(error) => return stored_response_request_body_error(error),
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return pre_header_stored_response_error(&client_request_error());
    };
    let decoded = match decode_compact_request(body) {
        Ok(decoded) => decoded,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    if !state.executor.supports_stored_response_continuity() {
        return continuity_unavailable();
    }
    let Some(repository) = state.stored_responses.clone() else {
        return pre_header_stored_response_error(&internal_error());
    };
    let requested_model = decoded.requested_model;
    let (public_model, route_alias, route_id) =
        match resolve_public_model(&authenticated_client, &requested_model) {
            Ok(resolved) => resolved,
            Err(error) => return pre_header_stored_response_error(&error),
        };
    let (client_key_id, access_group_id) = authenticated_client.event_identity();
    let lookup_now_ms = match system_now_ms() {
        Ok(now_ms) => now_ms,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let stored = match load_owned_response(
        Arc::clone(&repository),
        client_key_id.clone(),
        decoded.previous_response_id,
        lookup_now_ms,
    )
    .await
    {
        Ok(record) => record,
        Err(error) => return owned_continuity_error(error),
    };
    if let Err(error) = ensure_continuity_identity(
        stored.payload().public_model(),
        stored.payload().lineage(),
        &public_model,
        route_id.as_ref(),
    ) {
        return owned_continuity_error(error);
    }
    let context = match state.metadata_factory.request_context() {
        Ok(context) => context,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let request_id = context.request_id().clone();
    let _request_event = state
        .event_sink
        .try_emit(GatewayEvent::Request(RequestEvent::new(
            request_id.clone(),
            client_key_id.clone(),
            access_group_id,
            GatewayProtocol::OpenAiResponses,
            requested_model,
            public_model.clone(),
            route_alias,
            false,
        )));
    let compact_request = match compaction_request(stored.payload()) {
        Ok(request) => request,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let pin = match continuation_pin(
        stored.payload().lineage(),
        ResponsesContinuationKind::Compaction,
    ) {
        Ok(pin) => pin,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let execution = ResponsesExecution::new(
        context,
        compact_request,
        Some(pin.lineage().route_id().clone()),
        ResponsesResponseMode::NonStreaming,
        Arc::new(StoredContinuationRetryGate),
    )
    .with_continuation_pin(pin);
    let mut source = match state.executor.execute_routed(execution).await {
        Ok(source) => source,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let canonical = match tokio::time::timeout(
        STORED_RESPONSE_COMPACTION_TOTAL_TIMEOUT,
        collect_bounded_source(&mut source),
    )
    .await
    {
        Ok(Ok(canonical)) => canonical,
        Ok(Err(error)) => return pre_header_stored_response_error(&error),
        Err(_) => return pre_header_stored_response_error(&compaction_timeout_error()),
    };
    let mut usage_observer = UsageEventObserver::new(request_id, Arc::clone(&state.event_sink));
    for event in canonical.events() {
        usage_observer.observe(event);
    }
    let summary = match extract_compaction_summary(&canonical) {
        Ok(summary) => summary,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let metadata = match state.metadata_factory.response_metadata(&public_model) {
        Ok(metadata) => metadata,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let persisted_at_ms = match system_now_ms() {
        Ok(now_ms) => now_ms,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let Ok(compact_payload) = StoredResponseCompactionPayload::try_new(
        stored.payload().lineage().clone(),
        stored.response_id().clone(),
        public_model.clone(),
        summary,
    ) else {
        return pre_header_stored_response_error(&internal_error());
    };
    let Ok(Ok(compact_record)) = tokio::task::spawn_blocking(move || {
        repository.put_compaction_owned(&client_key_id, persisted_at_ms, &compact_payload)
    })
    .await
    else {
        return pre_header_stored_response_error(&internal_error());
    };
    let Some(locator_suffix) = compact_record
        .compact_id()
        .strip_prefix(STORED_RESPONSE_COMPACTION_PREFIX)
    else {
        return pre_header_stored_response_error(&internal_error());
    };
    let item_id = format!("cmp_{locator_suffix}");
    match encode_compaction_response(&canonical, metadata, &item_id, compact_record.compact_id()) {
        Ok(body) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .content_type("application/json")
            .body(body.to_string()),
        Err(error) => pre_header_stored_response_error(&error),
    }
}

async fn retrieve_stored_response(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    response_id: web::Path<String>,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(authenticated_client) => authenticated_client,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let Some(response_id) = parse_stored_response_id(response_id.into_inner()) else {
        return stored_response_not_found();
    };
    let (client_key_id, _access_group_id) = authenticated_client.event_identity();
    let Some(repository) = state.stored_responses.clone() else {
        return pre_header_stored_response_error(&internal_error());
    };
    let now_ms = match system_now_ms() {
        Ok(now_ms) => now_ms,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let response_id_for_lookup = response_id.clone();
    let record = match tokio::task::spawn_blocking(move || {
        repository.get_owned(&client_key_id, &response_id_for_lookup, now_ms)
    })
    .await
    {
        Ok(Ok(Some(record))) => record,
        Ok(Ok(None)) => return stored_response_not_found(),
        Ok(Err(_)) | Err(_) => return pre_header_stored_response_error(&internal_error()),
    };
    let Ok(canonical) = record.payload().canonical_response() else {
        return pre_header_stored_response_error(&internal_error());
    };
    let metadata = match OpenAiResponseMetadata::try_new(
        record.payload().public_model(),
        record.payload().created_at_seconds(),
    ) {
        Ok(metadata) => metadata,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    match encode_response(&canonical, metadata) {
        Ok(body) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .content_type("application/json")
            .body(body.to_string()),
        Err(error) => pre_header_stored_response_error(&error),
    }
}

async fn delete_stored_response(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    response_id: web::Path<String>,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(authenticated_client) => authenticated_client,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let Some(response_id) = parse_stored_response_id(response_id.into_inner()) else {
        return stored_response_not_found();
    };
    let (client_key_id, _access_group_id) = authenticated_client.event_identity();
    let Some(repository) = state.stored_responses.clone() else {
        return pre_header_stored_response_error(&internal_error());
    };
    let now_ms = match system_now_ms() {
        Ok(now_ms) => now_ms,
        Err(error) => return pre_header_stored_response_error(&error),
    };
    let response_id_for_delete = response_id.clone();
    let Ok(Ok(deleted)) = tokio::task::spawn_blocking(move || {
        repository.delete_owned(&client_key_id, &response_id_for_delete, now_ms)
    })
    .await
    else {
        return pre_header_stored_response_error(&internal_error());
    };
    if !deleted {
        return stored_response_not_found();
    }
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .content_type("application/json")
        .body(
            serde_json::json!({
                "id": response_id.as_str(),
                "object": "response.deleted",
                "deleted": true,
            })
            .to_string(),
        )
}

async fn messages(
    request: HttpRequest,
    state: web::Data<ResponsesHttpState>,
    payload: web::Payload,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(authenticated_client) => authenticated_client,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let body = match read_bounded_request_body(&request, payload).await {
        Ok(body) => body,
        Err(error) => return anthropic_request_body_error(error),
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
    let execution = ResponsesExecution::new_for_protocol(
        context,
        decoded.request,
        gateway_router::ProtocolFormat::AnthropicMessages,
        Arc::from(body.as_bytes()),
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
    payload: web::Payload,
) -> HttpResponse {
    let authenticated_client = match authenticate_client_key_request(&request, &state.authenticator)
    {
        Ok(authenticated_client) => authenticated_client,
        Err(error) => return pre_header_anthropic_error(&error),
    };
    let body = match read_bounded_request_body(&request, payload).await {
        Ok(body) => body,
        Err(error) => return anthropic_request_body_error(error),
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

/// Bounded inbound-body failures observed before any handler state exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestBodyError {
    /// The declared or streamed body exceeded [`MAX_INFERENCE_REQUEST_BODY_BYTES`].
    TooLarge,
    /// The transport failed before the complete body arrived.
    Unreadable,
}

/// Reads one complete request body without buffering more than
/// [`MAX_INFERENCE_REQUEST_BODY_BYTES`].
///
/// The handlers own this read instead of the `web::Bytes` extractor for two reasons: an oversized
/// body must produce the calling route's protocol error envelope rather than Actix's plain-text
/// extractor rejection, and no body byte may be buffered before Client Key admission succeeds.
async fn read_bounded_request_body(
    request: &HttpRequest,
    payload: web::Payload,
) -> Result<Vec<u8>, RequestBodyError> {
    if declared_length_exceeds_limit(request) {
        return Err(RequestBodyError::TooLarge);
    }

    tokio::time::timeout(
        INFERENCE_REQUEST_BODY_TIMEOUT,
        receive_bounded_request_body(payload),
    )
    .await
    .map_err(|_| RequestBodyError::Unreadable)?
}

/// Accumulates the payload under the byte bound, leaving the time bound to the caller.
async fn receive_bounded_request_body(
    mut payload: web::Payload,
) -> Result<Vec<u8>, RequestBodyError> {
    let mut body = Vec::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|_| RequestBodyError::Unreadable)?;
        let new_length = body
            .len()
            .checked_add(chunk.len())
            .ok_or(RequestBodyError::TooLarge)?;
        if new_length > MAX_INFERENCE_REQUEST_BODY_BYTES {
            return Err(RequestBodyError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }

    Ok(body)
}

/// Rejects an oversized body from its declared length before any chunk is buffered.
fn declared_length_exceeds_limit(request: &HttpRequest) -> bool {
    request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_INFERENCE_REQUEST_BODY_BYTES)
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

async fn chat_non_streaming_response(
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    metadata: ChatResponseMetadata,
    usage_observer: UsageEventObserver,
    sender: CanonicalEventSender,
    stream: CanonicalEventStream,
) -> HttpResponse {
    let mut stream =
        match start_bounded_transport(source, first, sender, stream, usage_observer, None).await {
            Ok(stream) => stream,
            Err(error) => return pre_header_chat_error(&error),
        };
    let tracker = stream.control().first_semantic_event_tracker();
    let response = match collect_completed_response(&mut stream).await {
        Ok(response) => response,
        Err(error) => return pre_header_chat_error(&error),
    };
    let Some(delivery_event) = response.events().first().cloned() else {
        return pre_header_chat_error(&internal_error());
    };
    let body = match encode_chat_response(&response, metadata) {
        Ok(body) => body,
        Err(error) => return pre_header_chat_error(&error),
    };
    let body = JsonDeliveryBody::new(web::Bytes::from(body.to_string()), tracker, delivery_event);

    match HttpResponse::Ok()
        .content_type("application/json")
        .message_body(body)
    {
        Ok(response) => response.map_into_boxed_body(),
        Err(_) => pre_header_chat_error(&internal_error()),
    }
}

async fn non_streaming_response(
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    metadata: OpenAiResponseMetadata,
    usage_observer: UsageEventObserver,
    sender: CanonicalEventSender,
    stream: CanonicalEventStream,
    stored_response: Option<StoredResponseWriteContext>,
) -> HttpResponse {
    let mut stream = match start_bounded_transport(
        source,
        first,
        sender,
        stream,
        usage_observer,
        stored_response,
    )
    .await
    {
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

async fn chat_streaming_response(
    source: Box<dyn ResponsesEventSource>,
    first: CanonicalEvent,
    metadata: ChatResponseMetadata,
    usage_observer: UsageEventObserver,
    sender: CanonicalEventSender,
    stream: CanonicalEventStream,
) -> HttpResponse {
    let mut initial_encoder = ChatSseEncoder::new(metadata.clone());
    if let Err(error) = initial_encoder.encode_event(&first) {
        return pre_header_chat_error(&error);
    }

    let stream =
        match start_bounded_transport(source, first, sender, stream, usage_observer, None).await {
            Ok(stream) => stream,
            Err(error) => return pre_header_chat_error(&error),
        };
    let tracker = stream.control().first_semantic_event_tracker();
    let body = ProtocolSseBody::new(stream, ChatSseEncoder::new(metadata), tracker);

    match HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .content_type("text/event-stream")
        .message_body(body)
    {
        Ok(response) => response.map_into_boxed_body(),
        Err(_) => pre_header_chat_error(&internal_error()),
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
        match start_bounded_transport(source, first, sender, stream, usage_observer, None).await {
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
    stored_response: Option<StoredResponseWriteContext>,
) -> HttpResponse {
    // Commit no headers until the initial event is shown encodable by a fresh protocol encoder.
    // The body owns a separate encoder so the first event still travels through P1-04 transport.
    let mut initial_encoder = OpenAiResponsesSseEncoder::new(metadata.clone());
    if let Err(error) = initial_encoder.encode_event(&first) {
        return pre_header_error(&error);
    }

    let stream = match start_bounded_transport(
        source,
        first,
        sender,
        stream,
        usage_observer,
        stored_response,
    )
    .await
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

    let stream =
        match start_bounded_transport(source, first, sender, stream, usage_observer, None).await {
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
    stored_response: Option<StoredResponseWriteContext>,
) -> Result<CanonicalEventStream, GatewayError> {
    sender.send(first.clone()).await?;
    usage_observer.observe(&first);
    let cancellation = sender.cancellation();

    tokio::spawn(async move {
        pump_source(
            source,
            sender,
            cancellation,
            usage_observer,
            stored_response,
            first,
        )
        .await;
    });

    Ok(stream)
}

async fn pump_source(
    mut source: Box<dyn ResponsesEventSource>,
    mut sender: CanonicalEventSender,
    cancellation: StreamCancellation,
    mut usage_observer: UsageEventObserver,
    stored_response: Option<StoredResponseWriteContext>,
    first: CanonicalEvent,
) {
    let mut stored_capture = match stored_response {
        Some(context) => match StoredResponseCapture::try_new(context, first) {
            Ok(capture) => Some(capture),
            Err(error) => {
                send_terminal_failure(&mut sender, error, &cancellation).await;
                return;
            }
        },
        None => None,
    };
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
                if !matches!(event, CanonicalEvent::StreamError(_))
                    && let Some(capture) = stored_capture.as_mut()
                {
                    if let Err(error) = capture.push(event.clone()) {
                        send_terminal_failure(&mut sender, error, &cancellation).await;
                        return;
                    }
                    if matches!(event, CanonicalEvent::ResponseEnd(_)) {
                        let response = match capture.completed_response() {
                            Ok(response) => response,
                            Err(error) => {
                                send_terminal_failure(&mut sender, error, &cancellation).await;
                                return;
                            }
                        };
                        if let Err(error) =
                            persist_completed_response(capture.context.clone(), response).await
                        {
                            send_terminal_failure(&mut sender, error, &cancellation).await;
                            return;
                        }
                    }
                }
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

async fn collect_bounded_source(
    source: &mut Box<dyn ResponsesEventSource>,
) -> Result<CanonicalResponse, GatewayError> {
    let mut events = Vec::new();
    let mut serialized_bytes = 0_usize;
    while let Some(event) = source.next_event().await? {
        if events.len() >= MAX_STORED_RESPONSE_EVENTS {
            return Err(internal_error());
        }
        let event_bytes = serde_json::to_vec(&event)
            .map_err(|_| internal_error())?
            .len();
        serialized_bytes = serialized_bytes
            .checked_add(event_bytes)
            .ok_or_else(internal_error)?;
        if serialized_bytes > MAX_STORED_RESPONSE_PAYLOAD_BYTES {
            return Err(internal_error());
        }
        events.push(event);
    }
    CanonicalResponse::try_new(events)
}

async fn persist_completed_response(
    context: StoredResponseWriteContext,
    response: CanonicalResponse,
) -> Result<(), GatewayError> {
    tokio::task::spawn_blocking(move || {
        let response_id = response
            .events()
            .first()
            .and_then(|event| match event {
                CanonicalEvent::ResponseStart(start) => Some(start.response_id.clone()),
                _ => None,
            })
            .ok_or_else(internal_error)?;
        let target = StoredResponseTarget::try_new(
            context.execution_lineage.provider_id().clone(),
            context.execution_lineage.upstream_id().clone(),
            context.execution_lineage.channel_id().clone(),
            context.execution_lineage.route_id().clone(),
            context.execution_lineage.route_candidate_id().clone(),
        )
        .map_err(|_| internal_error())?;
        let credential = StoredResponseCredentialBinding::try_new(
            context.execution_lineage.credential_id().clone(),
            context.execution_lineage.credential_revision(),
            Some(response_id.as_str().to_owned()),
        )
        .map_err(|_| internal_error())?;
        let lineage = StoredResponseLineage::try_new(
            context
                .execution_lineage
                .snapshot_version()
                .as_str()
                .to_owned(),
            target,
            credential,
        )
        .map_err(|_| internal_error())?;
        let payload = StoredResponsePayload::try_new(
            lineage,
            context.public_model,
            context.created_at_seconds,
            context.canonical_request,
            response,
        )
        .map_err(|_| internal_error())?;
        context
            .repository
            .put_owned(&context.client_key_id, context.created_at_ms, &payload)
            .map(|_outcome| ())
            .map_err(|_| internal_error())
    })
    .await
    .map_err(|_| internal_error())?
}

fn parse_stored_response_id(value: String) -> Option<ResponseId> {
    if value.is_empty() || value.len() > 512 || value.as_bytes().contains(&0) {
        return None;
    }
    ResponseId::try_new(value).ok()
}

fn system_now_ms() -> Result<i64, GatewayError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| internal_error())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| internal_error())
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

impl EncodedSseFrame for ChatSseFrame {
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

impl CanonicalSseEncoder for ChatSseEncoder {
    type Frame = ChatSseFrame;

    fn encode_event(&mut self, event: &CanonicalEvent) -> Result<Vec<Self::Frame>, GatewayError> {
        ChatSseEncoder::encode_event(self, event)
    }
}

struct SseEncodingState<E> {
    stream: CanonicalEventStream,
    encoder: E,
    pending: VecDeque<PendingSseChunk>,
    finished: bool,
    keepalive_deadline: tokio::time::Instant,
}

/// A streaming HTTP body that commits `FirstSemanticEvent` only when it gives Actix a semantic
/// bytes chunk, not when the chunk is queued, received, or encoded.
///
/// Both the `OpenAI` Responses and Anthropic Messages streaming paths use this one body, so its
/// byte-idle keepalive comment covers both. A keepalive chunk carries no canonical event, so
/// [`FirstSemanticEventTracker::mark_delivered`] is never reached for it and a transparent retry
/// stays permitted for as long as no semantic chunk has been written.
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
            keepalive_deadline: next_keepalive_deadline(),
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
            state.keepalive_deadline = next_keepalive_deadline();
            return Some((chunk, state));
        }
        if state.finished {
            return None;
        }

        // The idle tick races the bounded stream after the terminal check, so a keepalive can
        // never follow the terminal event or jump ahead of a queued frame. `recv` is cancel-safe,
        // so a tick that wins the race cannot consume a canonical event, and the biased order
        // keeps real output ahead of the tick when both are ready.
        let keepalive_deadline = state.keepalive_deadline;
        let received = tokio::select! {
            biased;
            received = state.stream.recv() => Some(received),
            () = tokio::time::sleep_until(keepalive_deadline) => None,
        };
        let Some(received) = received else {
            state.keepalive_deadline = next_keepalive_deadline();
            return Some((keepalive_sse_chunk(), state));
        };

        match received {
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

/// Builds the idle chunk whose absent delivery event keeps it outside the semantic boundary.
fn keepalive_sse_chunk() -> PendingSseChunk {
    PendingSseChunk {
        bytes: web::Bytes::from_static(SSE_KEEPALIVE_COMMENT),
        delivery_event: None,
    }
}

/// Returns the instant at which a body that writes nothing further owes its next keepalive.
fn next_keepalive_deadline() -> tokio::time::Instant {
    tokio::time::Instant::now() + SSE_KEEPALIVE_INTERVAL
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

fn pre_header_stored_response_error(error: &GatewayError) -> HttpResponse {
    let mut response = pre_header_error(error);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

fn owned_continuity_error(error: OwnedContinuityError) -> HttpResponse {
    match error {
        OwnedContinuityError::NotFound => stored_response_not_found(),
        OwnedContinuityError::Unavailable => continuity_unavailable(),
        OwnedContinuityError::Internal => pre_header_stored_response_error(&internal_error()),
    }
}

fn continuity_unavailable() -> HttpResponse {
    HttpResponse::Conflict()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .content_type("application/json")
        .body(
            serde_json::json!({
                "error": {
                    "type": "invalid_request_error",
                    "code": "continuity_unavailable",
                    "message": "the stored response cannot be continued on its exact target",
                    "param": "previous_response_id",
                }
            })
            .to_string(),
        )
}

fn stored_response_request_body_error(error: RequestBodyError) -> HttpResponse {
    let mut response = request_body_error(error);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

fn stored_response_not_found() -> HttpResponse {
    HttpResponse::NotFound()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .content_type("application/json")
        .body(
            serde_json::json!({
                "error": {
                    "type": "invalid_request_error",
                    "code": "response_not_found",
                    "message": "the stored response was not found",
                    "param": "response_id",
                }
            })
            .to_string(),
        )
}

fn pre_header_chat_error(error: &GatewayError) -> HttpResponse {
    let mut response = HttpResponse::build(error_status(error));
    if error.code() == GatewayErrorCode::ClientUnauthorized {
        response.insert_header((header::WWW_AUTHENTICATE, "Bearer"));
    }
    response
        .content_type("application/json")
        .body(encode_chat_error(error).to_string())
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

/// Reports a bounded-body failure using the `OpenAI` Responses error envelope.
fn request_body_error(error: RequestBodyError) -> HttpResponse {
    HttpResponse::build(request_body_error_status(error))
        .content_type("application/json")
        .body(encode_error(&client_request_error()).to_string())
}

/// Reports a bounded-body failure using the OpenAI-compatible Chat error envelope.
fn chat_request_body_error(error: RequestBodyError) -> HttpResponse {
    HttpResponse::build(request_body_error_status(error))
        .content_type("application/json")
        .body(encode_chat_error(&client_request_error()).to_string())
}

/// Reports a bounded-body failure using the Anthropic Messages error envelope.
fn anthropic_request_body_error(error: RequestBodyError) -> HttpResponse {
    HttpResponse::build(request_body_error_status(error))
        .content_type("application/json")
        .body(encode_anthropic_error(&client_request_error()).to_string())
}

/// Maps a bounded-body failure to its status without widening the frozen error taxonomy.
///
/// The taxonomy has no request-too-large category, so the envelope stays `ClientRequestError`
/// while the status still tells the client that size, not syntax, was the problem.
const fn request_body_error_status(error: RequestBodyError) -> StatusCode {
    match error {
        RequestBodyError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        RequestBodyError::Unreadable => StatusCode::BAD_REQUEST,
    }
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

const fn compaction_timeout_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeSet, VecDeque},
        error::Error,
        future::poll_fn,
        net::{Ipv4Addr, TcpListener},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use actix_web::{
        App, HttpServer,
        body::MessageBody,
        http::{StatusCode, header},
        test, web,
    };
    use actix_ws::{CloseCode, Item as WebSocketItem, Message as ActixWebSocketMessage};
    use futures_util::{SinkExt, StreamExt};
    use gateway_auth::{
        ClientKeyAuthenticator, InMemoryClientKey, InMemoryClientKeyAuthenticator,
        client_key::{ClientKeyPepper, ClientKeyService},
    };
    use gateway_core::{
        AccessGroupId, CanonicalEvent, CanonicalRequest, CanonicalResponse, ClientKeyId,
        CredentialId, EndpointId, ErrorScope, ExactInputTokenCount, GatewayError, GatewayErrorCode,
        GatewayEvent, GatewayEventSink, GatewayProtocol, MessageEnd, MessageRole, MessageStart,
        ProviderId, PublicModelId, RawExtensions, RawJson, ReasoningDelta, RequestContext,
        RequestId, ResponseEnd, ResponseId, ResponseStart, RouteCandidateId, RouteId, StreamError,
        TextDelta, UpstreamId, Usage, UsageDelta,
    };
    use gateway_observability::{BoundedEventQueue, EventQueueConfig};
    use gateway_router::{
        CapabilitySet, CountTokensExecution, CountTokensExecutor, CountTokensFuture,
        DeterministicMockEmission, DeterministicMockResponsesExecutor, ResponsesClientTransport,
        ResponsesContinuationKind, ResponsesContinuationPin, ResponsesEventSource,
        ResponsesExecution, ResponsesExecutionLineage, ResponsesExecutor, ResponsesFuture,
        RouteSnapshot, RouteSnapshotInput, RouteSnapshotRegistry, SnapshotAccessGroup,
        SnapshotCatalogAdmission, SnapshotClientKeyAuthenticator, SnapshotClientKeyView,
        SnapshotPublicModel, SnapshotRoute, SnapshotRouteCandidate, SnapshotRouteCandidateInput,
        SnapshotRoutePolicy, SnapshotTransformMode, SnapshotVersion,
    };
    use gateway_store::{
        secret_store::{KeyVersion, MASTER_KEY_BYTES, MasterKey, MasterKeyRing, SecretStore},
        stored_response::{
            SqliteStoredResponseStore, StoredResponseCredentialBinding, StoredResponseLineage,
            StoredResponsePayload, StoredResponseTarget,
        },
    };
    use gateway_stream::{FirstSemanticEventTracker, StreamCapacity, bounded_canonical_stream};
    use protocol_anthropic::{AnthropicMessagesSseEncoder, AnthropicResponseMetadata};
    use protocol_openai_chat::ChatResponseMetadata;
    use protocol_openai_responses::{OpenAiResponseMetadata, OpenAiResponsesSseEncoder};
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    use super::{
        JsonDeliveryBody, MAX_INFERENCE_REQUEST_BODY_BYTES, RESPONSES_WEBSOCKET_MAX_FRAGMENTS,
        ResponsesHttpState, ResponsesMetadataFactory, SSE_KEEPALIVE_COMMENT,
        SSE_KEEPALIVE_INTERVAL, SystemResponsesMetadataFactory, WebSocketFragmentAssembler,
        WebSocketInboundError, WebSocketSessionCache, client_request_error, configure,
        websocket_stream_error_reason,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    const TEST_CLIENT_KEY: &str = "p1-test-client-key";
    const FOREIGN_TEST_CLIENT_KEY: &str = "p13-foreign-client-key";
    const SNAPSHOT_PUBLIC_MODEL: &str = "public-model";

    #[actix_web::test]
    async fn websocket_fragment_assembler_is_text_only_and_bounded_by_bytes_and_count() -> TestResult
    {
        let mut assembler = WebSocketFragmentAssembler::default();
        assert_eq!(
            assembler.push_message(ActixWebSocketMessage::Text("complete".into()))?,
            Some("complete".to_owned())
        );
        assert_eq!(
            assembler.push_message(ActixWebSocketMessage::Continuation(
                WebSocketItem::FirstText(web::Bytes::from_static(b"frag")),
            ))?,
            None
        );
        assert_eq!(
            assembler.push_message(ActixWebSocketMessage::Continuation(WebSocketItem::Last(
                web::Bytes::from_static(b"mented"),
            )))?,
            Some("fragmented".to_owned())
        );
        assert_eq!(
            assembler.push_message(ActixWebSocketMessage::Binary(web::Bytes::from_static(
                b"binary",
            ))),
            Err(WebSocketInboundError::Unsupported)
        );

        let mut fragments = WebSocketFragmentAssembler::default();
        assert_eq!(
            fragments.push_message(ActixWebSocketMessage::Continuation(
                WebSocketItem::FirstText(web::Bytes::new()),
            ))?,
            None
        );
        for _ in 1..RESPONSES_WEBSOCKET_MAX_FRAGMENTS {
            assert_eq!(
                fragments.push_message(ActixWebSocketMessage::Continuation(
                    WebSocketItem::Continue(web::Bytes::new()),
                ))?,
                None
            );
        }
        assert_eq!(
            fragments.push_message(ActixWebSocketMessage::Continuation(
                WebSocketItem::Continue(web::Bytes::new()),
            )),
            Err(WebSocketInboundError::TooLarge)
        );
        assert_eq!(
            websocket_stream_error_reason(&actix_ws::ProtocolError::Overflow).code,
            CloseCode::Size
        );
        assert_eq!(
            websocket_stream_error_reason(&actix_ws::ProtocolError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid text",
            )))
            .code,
            CloseCode::Invalid
        );
        Ok(())
    }

    #[actix_web::test]
    async fn websocket_session_cache_is_idempotent_but_rejects_response_id_collisions() -> TestResult
    {
        let request =
            protocol_openai_responses::decode_request(r#"{"model":"mock-model","input":"first"}"#)?
                .request;
        let response = CanonicalResponse::try_new(websocket_text_events(
            "ws-cache-response",
            "first answer",
        )?)?;
        let lineage = websocket_test_lineage()?;
        let mut cache = WebSocketSessionCache::default();
        cache.insert(
            ResponseId::try_new("ws-cache-response")?,
            "mock-model".to_owned(),
            request.clone(),
            response.clone(),
            lineage.clone(),
        )?;
        cache.insert(
            ResponseId::try_new("ws-cache-response")?,
            "mock-model".to_owned(),
            request.clone(),
            response,
            lineage.clone(),
        )?;
        let conflicting = CanonicalResponse::try_new(websocket_text_events(
            "ws-cache-response",
            "different answer",
        )?)?;
        let error = cache
            .insert(
                ResponseId::try_new("ws-cache-response")?,
                "mock-model".to_owned(),
                request,
                conflicting,
                lineage,
            )
            .err()
            .ok_or("conflicting response ID must fail closed")?;
        assert_eq!(error.code(), GatewayErrorCode::InternalError);
        assert_eq!(cache.turns.len(), 1);
        Ok(())
    }

    #[actix_web::test]
    async fn responses_websocket_handshake_stream_and_session_continuation_are_exact() -> TestResult
    {
        let observations = Arc::new(Mutex::new(Vec::new()));
        let executor = WebSocketTestExecutor {
            responses: Mutex::new(
                vec![
                    websocket_text_events("ws-response-1", "first answer")?,
                    websocket_text_events("ws-response-2", "second answer")?,
                ]
                .into(),
            ),
            observations: Arc::clone(&observations),
        };
        let state = ResponsesHttpState::with_metadata(
            Arc::new(executor),
            Arc::new(FixedMetadata),
            test_authenticator()?,
            StreamCapacity::try_new(2)?,
        );
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let server_state = state.clone();
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(server_state.clone()))
                .configure(configure)
        })
        .workers(1)
        .listen(listener)?
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::task::yield_now().await;

        let url = format!("ws://{address}/v1/responses");
        let unauthorized = tokio_tungstenite::connect_async(url.clone())
            .await
            .err()
            .ok_or("missing Client Key must be rejected before upgrade")?;
        match unauthorized {
            tokio_tungstenite::tungstenite::Error::Http(response) => {
                assert_eq!(response.status(), 401);
                assert_eq!(
                    response
                        .headers()
                        .get("cache-control")
                        .and_then(|value| value.to_str().ok()),
                    Some("no-store")
                );
            }
            other => return Err(format!("unexpected authentication rejection: {other}").into()),
        }
        let mut request = url.clone().into_client_request()?;
        request.headers_mut().insert(
            "authorization",
            format!("Bearer {TEST_CLIENT_KEY}").parse()?,
        );
        request
            .headers_mut()
            .insert("x-codex-turn-state", "turn-state-1".parse()?);
        let (mut socket, response) = tokio_tungstenite::connect_async(request).await?;
        assert_eq!(response.status(), 101);
        assert_eq!(
            response
                .headers()
                .get("x-codex-turn-state")
                .and_then(|value| value.to_str().ok()),
            Some("turn-state-1")
        );
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"type":"response.create","model":"mock-model","input":"first"}"#.into(),
            ))
            .await?;
        let first = read_websocket_response(&mut socket).await?;
        assert_eq!(
            first.first().and_then(|event| event["type"].as_str()),
            Some("response.created")
        );
        assert_eq!(
            first.last().and_then(|event| event["type"].as_str()),
            Some("response.completed")
        );

        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"type":"response.create","model":"mock-model","previous_response_id":"ws-response-1","input":"second"}"#
                    .into(),
            ))
            .await?;
        let second = read_websocket_response(&mut socket).await?;
        assert_eq!(
            second.last().and_then(|event| event["type"].as_str()),
            Some("response.completed")
        );
        socket.close(None).await?;

        let observed = { observations.lock().map_err(|_| "observation lock")?.clone() };
        assert_eq!(observed.len(), 2);
        assert_eq!(observed[0].transport, ResponsesClientTransport::WebSocket);
        let first_native = observed[0]
            .native_payload
            .as_ref()
            .ok_or("first turn must retain native payload")?;
        assert_eq!(first_native["stream"], true);
        assert!(first_native.get("type").is_none());
        assert_eq!(observed[0].continuation, None);
        assert_eq!(
            observed[1].continuation,
            Some(ResponsesContinuationKind::WebSocketSession)
        );
        assert!(observed[1].native_payload.is_none());
        assert!(observed[1].message_count >= 3);
        let mut origin_request = url.into_client_request()?;
        origin_request.headers_mut().insert(
            "authorization",
            format!("Bearer {TEST_CLIENT_KEY}").parse()?,
        );
        origin_request
            .headers_mut()
            .insert("origin", "https://browser.example".parse()?);
        let rejected = tokio_tungstenite::connect_async(origin_request)
            .await
            .err()
            .ok_or("browser Origin must be rejected")?;
        match rejected {
            tokio_tungstenite::tungstenite::Error::Http(response) => {
                assert_eq!(response.status(), 403);
                assert_eq!(
                    response
                        .headers()
                        .get("cache-control")
                        .and_then(|value| value.to_str().ok()),
                    Some("no-store")
                );
            }
            other => return Err(format!("unexpected WebSocket rejection: {other}").into()),
        }

        handle.stop(true).await;
        task.await??;
        Ok(())
    }

    #[actix_web::test]
    #[allow(clippy::too_many_lines)]
    async fn responses_websocket_pending_bound_closes_and_cancels_the_active_source() -> TestResult
    {
        let dropped = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let state = ResponsesHttpState::with_metadata(
            Arc::new(WebSocketBlockingExecutor {
                dropped: Arc::clone(&dropped),
                calls: Arc::clone(&calls),
            }),
            Arc::new(FixedMetadata),
            test_authenticator()?,
            StreamCapacity::try_new(1)?,
        );
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let server_state = state.clone();
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(server_state.clone()))
                .configure(configure)
        })
        .workers(1)
        .listen(listener)?
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::task::yield_now().await;

        let mut request = format!("ws://{address}/v1/responses").into_client_request()?;
        request.headers_mut().insert(
            "authorization",
            format!("Bearer {TEST_CLIENT_KEY}").parse()?,
        );
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await?;
        let request_body = r#"{"type":"response.create","model":"mock-model","input":"blocked"}"#;
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                request_body.into(),
            ))
            .await?;

        loop {
            let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
                .await?
                .ok_or("WebSocket closed before response.created")??;
            match message {
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    let event: serde_json::Value = serde_json::from_str(text.as_str())?;
                    if event["type"] == "response.created" {
                        break;
                    }
                }
                tokio_tungstenite::tungstenite::Message::Ping(bytes) => {
                    socket
                        .send(tokio_tungstenite::tungstenite::Message::Pong(bytes))
                        .await?;
                }
                _ => {}
            }
        }

        for _ in 0..2 {
            socket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    request_body.into(),
                ))
                .await?;
        }
        let close = loop {
            let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
                .await?
                .ok_or("WebSocket ended without bounded close")??;
            match message {
                tokio_tungstenite::tungstenite::Message::Close(reason) => break reason,
                tokio_tungstenite::tungstenite::Message::Ping(bytes) => {
                    socket
                        .send(tokio_tungstenite::tungstenite::Message::Pong(bytes))
                        .await?;
                }
                _ => {}
            }
        };
        assert!(close.is_some_and(|reason| u16::from(reason.code) == 1008));
        tokio::time::timeout(Duration::from_secs(2), async {
            while !dropped.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert_eq!(calls.load(Ordering::Acquire), 1);

        handle.stop(true).await;
        task.await??;
        Ok(())
    }

    #[actix_web::test]
    async fn system_request_ids_are_restart_unique_bounded_and_debug_redacted() -> TestResult {
        let first_factory = SystemResponsesMetadataFactory::new();
        let first = first_factory.request_context()?;
        let second = first_factory.request_context()?;
        let restarted = SystemResponsesMetadataFactory::new().request_context()?;

        let first_id = first.request_id().as_str();
        let second_id = second.request_id().as_str();
        let restarted_id = restarted.request_id().as_str();
        assert!(first_id.starts_with("p1-request-"));
        assert!(first_id.ends_with("-0"));
        assert!(second_id.ends_with("-1"));
        assert_ne!(first_id, second_id);
        assert_ne!(first_id, restarted_id);
        assert!(first_id.len() <= 64);
        let diagnostic = format!("{first_factory:?}");
        assert!(!diagnostic.contains(&first_id[11..43]));
        assert!(diagnostic.contains("<redacted>"));
        Ok(())
    }
    const SNAPSHOT_MODEL_ALIAS: &str = "client-model-alias";

    /// Actix's `PayloadConfig` default, which the data-plane handlers must no longer inherit.
    const ACTIX_DEFAULT_PAYLOAD_LIMIT_BYTES: usize = 262_144;

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

        fn chat_metadata(
            &self,
            public_model: &str,
            include_usage: bool,
        ) -> Result<ChatResponseMetadata, GatewayError> {
            ChatResponseMetadata::try_new(public_model, 1, include_usage)
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

    fn websocket_text_events(
        response_id: &str,
        text: &str,
    ) -> Result<Vec<CanonicalEvent>, Box<dyn Error>> {
        Ok(vec![
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id: ResponseId::try_new(response_id.to_owned())?,
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

    async fn read_websocket_response<S>(
        socket: &mut tokio_tungstenite::WebSocketStream<S>,
    ) -> Result<Vec<serde_json::Value>, Box<dyn Error>>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let mut events = Vec::new();
        loop {
            let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
                .await?
                .ok_or("WebSocket closed before a terminal response")??;
            match message {
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    let event: serde_json::Value = serde_json::from_str(text.as_str())?;
                    let terminal = matches!(
                        event.get("type").and_then(serde_json::Value::as_str),
                        Some("response.completed" | "response.failed" | "response.incomplete")
                    );
                    events.push(event);
                    if terminal {
                        return Ok(events);
                    }
                }
                tokio_tungstenite::tungstenite::Message::Ping(bytes) => {
                    socket
                        .send(tokio_tungstenite::tungstenite::Message::Pong(bytes))
                        .await?;
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => {
                    return Err("WebSocket closed before a terminal response".into());
                }
                _ => {}
            }
        }
    }

    fn authorized_as(request: test::TestRequest, key: &str) -> test::TestRequest {
        request.insert_header((header::AUTHORIZATION, format!("Bearer {key}")))
    }

    fn stored_response_authenticator() -> Result<Arc<dyn ClientKeyAuthenticator>, Box<dyn Error>> {
        let owner = InMemoryClientKey::try_new(
            TEST_CLIENT_KEY,
            ClientKeyId::try_new("http-test-client-key")?,
            true,
        )?;
        let foreign = InMemoryClientKey::try_new(
            FOREIGN_TEST_CLIENT_KEY,
            ClientKeyId::try_new("http-foreign-client-key")?,
            true,
        )?;
        Ok(Arc::new(InMemoryClientKeyAuthenticator::try_new([
            owner, foreign,
        ])?))
    }

    fn stored_response_store() -> Result<Arc<SqliteStoredResponseStore>, Box<dyn Error>> {
        let version = KeyVersion::try_new(1)?;
        let key = MasterKey::try_from_bytes([0x31; MASTER_KEY_BYTES])?;
        let secret_store = SecretStore::new(MasterKeyRing::try_new(version, [(version, key)])?);
        Ok(Arc::new(SqliteStoredResponseStore::open_in_memory(
            secret_store,
        )?))
    }

    fn stored_response_lineage() -> Result<StoredResponseLineage, Box<dyn Error>> {
        let target = StoredResponseTarget::try_new(
            ProviderId::try_new("stored-provider")?,
            UpstreamId::try_new("stored-provider")?,
            EndpointId::try_new("stored-channel")?,
            RouteId::try_new("stored-route")?,
            RouteCandidateId::try_new("stored-candidate")?,
        )?;
        let credential = StoredResponseCredentialBinding::try_new(
            CredentialId::try_new("stored-credential")?,
            11,
            Some("owned-upstream-response".to_owned()),
        )?;
        Ok(StoredResponseLineage::try_new(
            "stored-config-v1",
            target,
            credential,
        )?)
    }

    fn seed_owned_stored_response(
        store: &SqliteStoredResponseStore,
        response_id: &str,
        public_model: &str,
        prompt: &str,
        answer: &str,
    ) -> Result<(), Box<dyn Error>> {
        let request = protocol_openai_responses::decode_request(
            &serde_json::json!({"model": "mock-model", "input": prompt}).to_string(),
        )?
        .request;
        let response = CanonicalResponse::try_new(text_response_events(response_id, answer)?)?;
        let payload = StoredResponsePayload::try_new(
            stored_response_lineage()?,
            public_model,
            1,
            request,
            response,
        )?;
        let _ = store.put_owned(
            &ClientKeyId::try_new("http-test-client-key")?,
            super::system_now_ms()?,
            &payload,
        )?;
        Ok(())
    }

    fn text_response_events(
        response_id: &str,
        text: &str,
    ) -> Result<Vec<CanonicalEvent>, Box<dyn Error>> {
        Ok(vec![
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id: ResponseId::try_new(response_id)?,
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
            CanonicalEvent::MessageEnd(MessageEnd::default()),
            CanonicalEvent::ResponseEnd(ResponseEnd::default()),
        ])
    }

    fn stored_response_state(
        events: Vec<CanonicalEvent>,
        observed_native: Arc<Mutex<Vec<serde_json::Value>>>,
        store: Arc<SqliteStoredResponseStore>,
    ) -> Result<ResponsesHttpState, Box<dyn Error>> {
        Ok(ResponsesHttpState::with_metadata(
            Arc::new(StoredResponsesExecutor {
                events,
                observed_native,
            }),
            Arc::new(FixedMetadata),
            stored_response_authenticator()?,
            StreamCapacity::try_new(2)?,
        )
        .with_stored_response_store(store))
    }

    fn continuity_state(
        responses: Vec<Vec<CanonicalEvent>>,
        observed: Arc<Mutex<Vec<ContinuityObservation>>>,
        store: Arc<SqliteStoredResponseStore>,
    ) -> Result<ResponsesHttpState, Box<dyn Error>> {
        Ok(ResponsesHttpState::with_metadata(
            Arc::new(ContinuityExecutor {
                responses: Mutex::new(responses.into()),
                observed,
            }),
            Arc::new(FixedMetadata),
            stored_response_authenticator()?,
            StreamCapacity::try_new(2)?,
        )
        .with_stored_response_store(store))
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
    async fn non_streaming_chat_uses_bounded_transport_and_emits_chat_request_protocol()
    -> TestResult {
        let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(2, 1)?)?;
        let event_sink: Arc<dyn GatewayEventSink> = Arc::new(queue);
        let state = mock_state_with_event_sink(text_events_with_final_usage()?, event_sink)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/chat/completions")
                .set_payload(
                    r#"{"model":"mock-model","messages":[{"role":"user","content":"hello"}]}"#,
                ),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(&test::read_body(response).await)?;
        assert_eq!(body["object"], "chat.completion");
        assert_eq!(
            body["choices"][0]["message"]["content"],
            "deterministic hello"
        );
        assert_eq!(body["choices"][0]["finish_reason"], "stop");
        assert_eq!(body["usage"]["total_tokens"], 8);

        let Some(GatewayEvent::Request(event)) = receiver.try_recv() else {
            return Err("expected Chat Request event".into());
        };
        assert_eq!(event.protocol(), GatewayProtocol::OpenAiChatCompletions);
        assert!(!event.streaming());
        assert!(matches!(receiver.try_recv(), Some(GatewayEvent::Usage(_))));
        Ok(())
    }

    #[actix_web::test]
    async fn streaming_chat_emits_finish_usage_done_in_order() -> TestResult {
        let state = mock_state(text_events_with_final_usage()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/chat/completions")
                .set_payload(r#"{"model":"mock-model","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":{"include_usage":true}}"#),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        let data_lines = body
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .collect::<Vec<_>>();
        let content = data_lines
            .iter()
            .position(|line| line.contains(r#""content":"deterministic hello""#))
            .ok_or("missing content")?;
        let finish = data_lines
            .iter()
            .position(|line| line.contains(r#""finish_reason":"stop""#))
            .ok_or("missing finish")?;
        let usage = data_lines
            .iter()
            .position(|line| {
                serde_json::from_str::<serde_json::Value>(line).is_ok_and(|value| {
                    value["choices"].as_array().is_some_and(Vec::is_empty)
                        && value["usage"]["total_tokens"] == 8
                })
            })
            .ok_or("missing usage")?;
        let done = data_lines
            .iter()
            .position(|line| *line == "[DONE]")
            .ok_or("missing done")?;
        assert!(content < finish && finish < usage && usage < done);
        assert_eq!(body.matches("data: [DONE]").count(), 1);
        Ok(())
    }

    #[actix_web::test]
    async fn chat_auth_and_body_limits_fail_before_executor_start() -> TestResult {
        let calls = Arc::new(AtomicUsize::new(0));
        let state = ResponsesHttpState::with_metadata(
            Arc::new(CountingExecutor {
                calls: calls.clone(),
            }),
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

        let unauthorized = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/chat/completions")
                .set_payload("not-json")
                .to_request(),
        )
        .await;
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let oversized = test::call_service(
            &app,
            authorized(
                test::TestRequest::post()
                    .uri("/v1/chat/completions")
                    .set_payload(vec![b'p'; MAX_INFERENCE_REQUEST_BODY_BYTES + 1]),
            )
            .to_request(),
        )
        .await;
        assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body: serde_json::Value = serde_json::from_slice(&test::read_body(oversized).await)?;
        assert_eq!(body["error"]["code"], "ClientRequestError");
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

    #[tokio::test(start_paused = true)]
    async fn idle_sse_body_writes_a_keepalive_only_after_the_full_interval() -> TestResult {
        let (mut sender, stream) = bounded_canonical_stream(StreamCapacity::try_new(1)?);
        let tracker = stream.control().first_semantic_event_tracker();
        sender.send(response_start()?).await?;
        let mut body = Box::pin(super::ProtocolSseBody::new(
            stream,
            OpenAiResponsesSseEncoder::new(OpenAiResponseMetadata::try_new("mock-model", 1)?),
            tracker.clone(),
        ));
        let first = poll_fn(|context| body.as_mut().poll_next(context)).await;
        assert!(matches!(first, Some(Ok(_))));
        // Drain any further frames this one event encoded to, so the body is genuinely byte-idle.
        // A zero timeout polls once and gives up without advancing the paused clock, so the idle
        // window measured below is the full one the last delivered chunk started.
        while tokio::time::timeout(
            Duration::ZERO,
            poll_fn(|context| body.as_mut().poll_next(context)),
        )
        .await
        .is_ok()
        {}

        // Delivering a chunk restarts the idle window, so the next comment is one full interval
        // away rather than one interval from the body's construction.
        let idle_since = tokio::time::Instant::now();
        let idle = poll_fn(|context| body.as_mut().poll_next(context)).await;
        assert!(matches!(
            idle,
            Some(Ok(bytes)) if bytes.as_ref() == SSE_KEEPALIVE_COMMENT
        ));
        assert!(tokio::time::Instant::now().duration_since(idle_since) >= SSE_KEEPALIVE_INTERVAL);
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn a_keepalive_never_commits_the_first_semantic_event_boundary() -> TestResult {
        let (mut sender, stream) = bounded_canonical_stream(StreamCapacity::try_new(1)?);
        let tracker = stream.control().first_semantic_event_tracker();
        let mut body = Box::pin(super::ProtocolSseBody::new(
            stream,
            OpenAiResponsesSseEncoder::new(OpenAiResponseMetadata::try_new("mock-model", 1)?),
            tracker.clone(),
        ));

        let idle = tokio::time::timeout(
            SSE_KEEPALIVE_INTERVAL + Duration::from_secs(1),
            poll_fn(|context| body.as_mut().poll_next(context)),
        )
        .await?;
        assert!(matches!(
            idle,
            Some(Ok(bytes)) if bytes.as_ref() == SSE_KEEPALIVE_COMMENT
        ));
        assert!(!tracker.is_committed());

        // A real event after the idle comment still commits the boundary and is delivered intact.
        sender.send(response_start()?).await?;
        let semantic = poll_fn(|context| body.as_mut().poll_next(context)).await;
        assert!(matches!(
            semantic,
            Some(Ok(bytes)) if String::from_utf8_lossy(&bytes).contains("event: response.created")
        ));
        assert!(tracker.is_committed());
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn the_anthropic_streaming_body_shares_the_same_keepalive() -> TestResult {
        let (_sender, stream) = bounded_canonical_stream(StreamCapacity::try_new(1)?);
        let tracker = stream.control().first_semantic_event_tracker();
        let mut body = Box::pin(super::ProtocolSseBody::new(
            stream,
            AnthropicMessagesSseEncoder::new(AnthropicResponseMetadata::try_new("mock-model")?),
            tracker.clone(),
        ));

        let idle = tokio::time::timeout(
            SSE_KEEPALIVE_INTERVAL + Duration::from_secs(1),
            poll_fn(|context| body.as_mut().poll_next(context)),
        )
        .await?;
        assert!(matches!(
            idle,
            Some(Ok(bytes)) if bytes.as_ref() == SSE_KEEPALIVE_COMMENT
        ));
        assert!(!tracker.is_committed());
        Ok(())
    }

    #[actix_web::test]
    async fn responses_accepts_a_body_larger_than_the_actix_default_payload_limit() -> TestResult {
        let state = mock_state(text_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let padding = "p".repeat(400 * 1024);
        let payload = format!(r#"{{"model":"mock-model","input":"{padding}"}}"#);
        assert!(payload.len() > ACTIX_DEFAULT_PAYLOAD_LIMIT_BYTES);
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(payload),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""status":"completed""#));
        Ok(())
    }

    #[actix_web::test]
    async fn messages_accepts_a_body_larger_than_the_actix_default_payload_limit() -> TestResult {
        let state = mock_state(anthropic_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let padding = "p".repeat(400 * 1024);
        let payload = format!(
            r#"{{"model":"mock-model","max_tokens":1,"messages":[{{"role":"user","content":"{padding}"}}]}}"#
        );
        assert!(payload.len() > ACTIX_DEFAULT_PAYLOAD_LIMIT_BYTES);
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/messages")
                .set_payload(payload),
        )
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
    async fn responses_rejects_a_body_over_the_inference_limit_with_an_openai_error_envelope()
    -> TestResult {
        let calls = Arc::new(AtomicUsize::new(0));
        let state = ResponsesHttpState::with_metadata(
            Arc::new(CountingExecutor {
                calls: calls.clone(),
            }),
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
                .set_payload(vec![b'p'; MAX_INFERENCE_REQUEST_BODY_BYTES + 1]),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let body: serde_json::Value = serde_json::from_slice(&test::read_body(response).await)?;
        assert_eq!(
            body.pointer("/error/type"),
            Some(&serde_json::json!("invalid_request_error"))
        );
        assert_eq!(
            body.pointer("/error/code"),
            Some(&serde_json::json!("ClientRequestError"))
        );
        assert_eq!(body.pointer("/error/param"), Some(&serde_json::Value::Null));
        assert_eq!(calls.load(Ordering::Acquire), 0);
        Ok(())
    }

    #[actix_web::test]
    async fn messages_rejects_a_body_over_the_inference_limit_with_an_anthropic_error_envelope()
    -> TestResult {
        let state = mock_state(anthropic_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/messages")
                .set_payload(vec![b'p'; MAX_INFERENCE_REQUEST_BODY_BYTES + 1]),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let body: serde_json::Value = serde_json::from_slice(&test::read_body(response).await)?;
        assert_eq!(body.pointer("/type"), Some(&serde_json::json!("error")));
        assert_eq!(
            body.pointer("/error/type"),
            Some(&serde_json::json!("invalid_request_error"))
        );
        assert_eq!(body.pointer("/error/code"), None);
        Ok(())
    }

    #[actix_web::test]
    async fn count_tokens_rejects_a_body_over_the_inference_limit_with_an_anthropic_error_envelope()
    -> TestResult {
        let state = mock_state(anthropic_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/messages/count_tokens")
                .set_payload(vec![b'p'; MAX_INFERENCE_REQUEST_BODY_BYTES + 1]),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body: serde_json::Value = serde_json::from_slice(&test::read_body(response).await)?;
        assert_eq!(body.pointer("/type"), Some(&serde_json::json!("error")));
        assert_eq!(
            body.pointer("/error/type"),
            Some(&serde_json::json!("invalid_request_error"))
        );
        assert_eq!(body.pointer("/input_tokens"), None);
        Ok(())
    }

    #[actix_web::test]
    async fn a_declared_content_length_over_the_inference_limit_is_rejected_before_the_body_streams()
    -> TestResult {
        let state = mock_state(text_events()?)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let declared = MAX_INFERENCE_REQUEST_BODY_BYTES + 1;
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"hello"}"#)
                .insert_header((header::CONTENT_LENGTH, declared.to_string())),
        )
        .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        Ok(())
    }

    #[actix_web::test]
    async fn an_unauthenticated_oversized_body_is_rejected_before_it_is_buffered() -> TestResult {
        let calls = Arc::new(AtomicUsize::new(0));
        let state = ResponsesHttpState::with_metadata(
            Arc::new(CountingExecutor {
                calls: calls.clone(),
            }),
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
        let request = test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header((
                header::CONTENT_LENGTH,
                (MAX_INFERENCE_REQUEST_BODY_BYTES + 1).to_string(),
            ))
            .set_payload("not-json")
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""code":"ClientUnauthorized""#));
        assert_eq!(calls.load(Ordering::Acquire), 0);
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

    #[actix_web::test]
    async fn stored_json_response_is_owner_exact_retrievable_and_deletable() -> TestResult {
        let observed_native = Arc::new(Mutex::new(Vec::new()));
        let store = stored_response_store()?;
        let state = stored_response_state(
            text_events()?,
            Arc::clone(&observed_native),
            Arc::clone(&store),
        )?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;

        let create = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"private prompt","store":true}"#),
        )
        .to_request();
        let create = test::call_service(&app, create).await;
        assert_eq!(create.status(), StatusCode::OK);
        let created: serde_json::Value = serde_json::from_slice(&test::read_body(create).await)?;
        assert_eq!(created["id"], "http-test-response");
        {
            let observed = observed_native.lock().map_err(|_| "native lock")?;
            assert_eq!(observed.len(), 1);
            assert_eq!(observed[0]["store"], false);
            assert_eq!(observed[0]["input"], "private prompt");
        }

        let durable = store
            .get_owned(
                &ClientKeyId::try_new("http-test-client-key")?,
                &ResponseId::try_new("http-test-response")?,
                super::system_now_ms()?,
            )?
            .ok_or("missing durable stored response")?;
        assert_eq!(
            durable.payload().lineage().target().provider_id().as_str(),
            "stored-provider"
        );
        assert_eq!(
            durable.payload().lineage().target().channel_id().as_str(),
            "stored-channel"
        );
        assert_eq!(
            durable.payload().lineage().target().route_id().as_str(),
            "stored-route"
        );
        assert_eq!(
            durable
                .payload()
                .lineage()
                .credential()
                .credential_id()
                .as_str(),
            "stored-credential"
        );
        assert_eq!(
            durable
                .payload()
                .lineage()
                .credential()
                .credential_revision(),
            11
        );

        let retrieve = authorized(test::TestRequest::get().uri("/v1/responses/http-test-response"))
            .to_request();
        let retrieve = test::call_service(&app, retrieve).await;
        assert_eq!(retrieve.status(), StatusCode::OK);
        assert_eq!(
            retrieve
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let retrieved: serde_json::Value =
            serde_json::from_slice(&test::read_body(retrieve).await)?;
        assert_eq!(retrieved, created);

        let foreign = authorized_as(
            test::TestRequest::get().uri("/v1/responses/http-test-response"),
            FOREIGN_TEST_CLIENT_KEY,
        )
        .to_request();
        let foreign = test::call_service(&app, foreign).await;
        assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            foreign
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let foreign_body = test::read_body(foreign).await;
        let missing =
            authorized(test::TestRequest::get().uri("/v1/responses/unknown-response")).to_request();
        let missing = test::call_service(&app, missing).await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(foreign_body, test::read_body(missing).await);

        let foreign_delete = authorized_as(
            test::TestRequest::delete().uri("/v1/responses/http-test-response"),
            FOREIGN_TEST_CLIENT_KEY,
        )
        .to_request();
        assert_eq!(
            test::call_service(&app, foreign_delete).await.status(),
            StatusCode::NOT_FOUND
        );

        let unauthenticated = test::TestRequest::get()
            .uri("/v1/responses/http-test-response")
            .to_request();
        let unauthenticated = test::call_service(&app, unauthenticated).await;
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            unauthenticated
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );

        let delete =
            authorized(test::TestRequest::delete().uri("/v1/responses/http-test-response"))
                .to_request();
        let delete = test::call_service(&app, delete).await;
        assert_eq!(delete.status(), StatusCode::OK);
        let deleted: serde_json::Value = serde_json::from_slice(&test::read_body(delete).await)?;
        assert_eq!(deleted["object"], "response.deleted");
        assert_eq!(deleted["deleted"], true);

        let after_delete =
            authorized(test::TestRequest::get().uri("/v1/responses/http-test-response"))
                .to_request();
        assert_eq!(
            test::call_service(&app, after_delete).await.status(),
            StatusCode::NOT_FOUND
        );
        Ok(())
    }

    #[actix_web::test]
    async fn previous_response_replays_owned_history_on_the_exact_lineage() -> TestResult {
        let store = stored_response_store()?;
        seed_owned_stored_response(
            &store,
            "owned-history-response",
            "mock-model",
            "private first turn",
            "private prior answer",
        )?;
        seed_owned_stored_response(
            &store,
            "wrong-model-response",
            "other-model",
            "private mismatched turn",
            "private mismatched answer",
        )?;
        let observed = Arc::new(Mutex::new(Vec::new()));
        let state = continuity_state(
            vec![text_response_events(
                "continued-history-response",
                "continued answer",
            )?],
            Arc::clone(&observed),
            Arc::clone(&store),
        )?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;

        let continuation = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(
                    r#"{"model":"mock-model","input":"current turn","previous_response_id":"owned-history-response","store":true}"#,
                ),
        )
        .to_request();
        let continuation = test::call_service(&app, continuation).await;
        assert_eq!(continuation.status(), StatusCode::OK);
        let response: serde_json::Value =
            serde_json::from_slice(&test::read_body(continuation).await)?;
        assert_eq!(response["id"], "continued-history-response");

        let request_json = {
            let observations = observed.lock().map_err(|_| "continuity lock")?;
            assert_eq!(observations.len(), 1);
            let observation = &observations[0];
            assert!(!observation.had_native_payload);
            let pin = observation.pin.as_ref().ok_or("missing continuation pin")?;
            assert_eq!(pin.kind(), ResponsesContinuationKind::StoredResponse);
            assert_eq!(pin.lineage().route_id().as_str(), "stored-route");
            assert_eq!(pin.lineage().credential_id().as_str(), "stored-credential");
            assert_eq!(pin.lineage().credential_revision(), 11);
            serde_json::to_string(&observation.request)?
        };
        assert!(request_json.contains("private first turn"));
        assert!(request_json.contains("private prior answer"));
        assert!(request_json.contains("current turn"));
        assert!(!request_json.contains("owned-history-response"));

        let durable = store
            .get_owned(
                &ClientKeyId::try_new("http-test-client-key")?,
                &ResponseId::try_new("continued-history-response")?,
                super::system_now_ms()?,
            )?
            .ok_or("continued response was not durably stored")?;
        let durable_request = serde_json::to_string(durable.payload().request())?;
        assert!(durable_request.contains("private first turn"));
        assert!(durable_request.contains("private prior answer"));
        assert!(durable_request.contains("current turn"));

        let foreign = authorized_as(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(
                    r#"{"model":"mock-model","input":"probe","previous_response_id":"owned-history-response"}"#,
                ),
            FOREIGN_TEST_CLIENT_KEY,
        )
        .to_request();
        let foreign = test::call_service(&app, foreign).await;
        assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            foreign
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        assert_eq!(observed.lock().map_err(|_| "continuity lock")?.len(), 1);

        let wrong_model = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(
                    r#"{"model":"mock-model","input":"probe","previous_response_id":"wrong-model-response"}"#,
                ),
        )
        .to_request();
        let wrong_model = test::call_service(&app, wrong_model).await;
        assert_eq!(wrong_model.status(), StatusCode::CONFLICT);
        assert_eq!(
            wrong_model
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        assert_eq!(observed.lock().map_err(|_| "continuity lock")?.len(), 1);
        Ok(())
    }

    #[actix_web::test]
    async fn compaction_is_owner_exact_encrypted_and_locally_replayed() -> TestResult {
        let store = stored_response_store()?;
        seed_owned_stored_response(
            &store,
            "compact-source-response",
            "mock-model",
            "private source prompt",
            "private source answer",
        )?;
        let observed = Arc::new(Mutex::new(Vec::new()));
        let state = continuity_state(
            vec![
                text_response_events("compact-summary-response", "private compact summary")?,
                text_response_events("compact-followup-response", "followup answer")?,
            ],
            Arc::clone(&observed),
            Arc::clone(&store),
        )?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;

        let compact = authorized(
            test::TestRequest::post()
                .uri("/v1/responses/compact")
                .set_payload(
                    r#"{"model":"mock-model","previous_response_id":"compact-source-response","stream":false}"#,
                ),
        )
        .to_request();
        let compact = test::call_service(&app, compact).await;
        assert_eq!(compact.status(), StatusCode::OK);
        assert_eq!(
            compact
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let compact_body = test::read_body(compact).await;
        let compact_json: serde_json::Value = serde_json::from_slice(&compact_body)?;
        assert_eq!(compact_json["output"][0]["type"], "compaction");
        assert_eq!(compact_json["output"][0]["created_by"], "cpar");
        let token = compact_json["output"][0]["encrypted_content"]
            .as_str()
            .ok_or("missing compact locator")?
            .to_owned();
        assert!(token.starts_with(super::STORED_RESPONSE_COMPACTION_PREFIX));
        let public_body = String::from_utf8(compact_body.to_vec())?;
        for forbidden in [
            "private compact summary",
            "private source prompt",
            "private source answer",
            "stored-credential",
            "stored-provider",
        ] {
            assert!(!public_body.contains(forbidden));
        }

        let foreign_body = serde_json::json!({
            "model": "mock-model",
            "input": [
                {
                    "type": "compaction",
                    "encrypted_content": token,
                    "created_by": "cpar"
                },
                {"role": "user", "content": "foreign probe"}
            ]
        })
        .to_string();
        let foreign = authorized_as(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(foreign_body),
            FOREIGN_TEST_CLIENT_KEY,
        )
        .to_request();
        assert_eq!(
            test::call_service(&app, foreign).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(observed.lock().map_err(|_| "continuity lock")?.len(), 1);

        let continuation_body = serde_json::json!({
            "model": "mock-model",
            "input": [
                {
                    "type": "compaction",
                    "id": compact_json["output"][0]["id"],
                    "encrypted_content": token,
                    "created_by": "cpar"
                },
                {"role": "user", "content": "continue from summary"}
            ]
        })
        .to_string();
        let continuation = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(continuation_body),
        )
        .to_request();
        let continuation = test::call_service(&app, continuation).await;
        assert_eq!(continuation.status(), StatusCode::OK);

        let observations = observed.lock().map_err(|_| "continuity lock")?;
        assert_eq!(observations.len(), 2);
        for observation in observations.iter() {
            assert!(!observation.had_native_payload);
            assert_eq!(
                observation.pin.as_ref().map(ResponsesContinuationPin::kind),
                Some(ResponsesContinuationKind::Compaction)
            );
        }
        let summary_request = serde_json::to_string(&observations[0].request)?;
        assert!(summary_request.contains("private source prompt"));
        assert!(summary_request.contains("private source answer"));
        let continued_request = serde_json::to_string(&observations[1].request)?;
        assert!(continued_request.contains("private compact summary"));
        assert!(continued_request.contains("continue from summary"));
        assert!(!continued_request.contains("cpar_compact_v1"));
        Ok(())
    }

    #[actix_web::test]
    async fn stored_sse_waits_for_success_and_stream_error_is_not_retrievable() -> TestResult {
        let successful_store = stored_response_store()?;
        let successful_state = stored_response_state(
            text_events()?,
            Arc::new(Mutex::new(Vec::new())),
            successful_store,
        )?;
        let successful_app = test::init_service(
            App::new()
                .app_data(web::Data::new(successful_state))
                .configure(configure),
        )
        .await;
        let stream =
            authorized(test::TestRequest::post().uri("/v1/responses").set_payload(
                r#"{"model":"mock-model","input":"hello","store":true,"stream":true}"#,
            ))
            .to_request();
        let stream = test::call_service(&successful_app, stream).await;
        assert_eq!(stream.status(), StatusCode::OK);
        let stream_body = String::from_utf8(test::read_body(stream).await.to_vec())?;
        assert!(stream_body.contains("event: response.completed"));
        let retrieve = authorized(test::TestRequest::get().uri("/v1/responses/http-test-response"))
            .to_request();
        assert_eq!(
            test::call_service(&successful_app, retrieve).await.status(),
            StatusCode::OK
        );

        let failed_store = stored_response_store()?;
        let failed_state = stored_response_state(
            vec![
                CanonicalEvent::ResponseStart(ResponseStart {
                    response_id: ResponseId::try_new("failed-stored-response")?,
                    extensions: RawExtensions::default(),
                }),
                CanonicalEvent::StreamError(StreamError {
                    error: GatewayError::new(
                        GatewayErrorCode::ProviderTransient,
                        ErrorScope::Provider,
                    ),
                }),
            ],
            Arc::new(Mutex::new(Vec::new())),
            failed_store,
        )?;
        let failed_app = test::init_service(
            App::new()
                .app_data(web::Data::new(failed_state))
                .configure(configure),
        )
        .await;
        let failed =
            authorized(test::TestRequest::post().uri("/v1/responses").set_payload(
                r#"{"model":"mock-model","input":"hello","store":true,"stream":true}"#,
            ))
            .to_request();
        let failed = test::call_service(&failed_app, failed).await;
        assert_eq!(failed.status(), StatusCode::OK);
        let failed_body = String::from_utf8(test::read_body(failed).await.to_vec())?;
        assert!(failed_body.contains("response.failed"));
        let retrieve_failed =
            authorized(test::TestRequest::get().uri("/v1/responses/failed-stored-response"))
                .to_request();
        assert_eq!(
            test::call_service(&failed_app, retrieve_failed)
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        Ok(())
    }

    #[actix_web::test]
    async fn stored_sse_capture_fails_before_exceeding_the_durable_event_bound() -> TestResult {
        let mut events = Vec::with_capacity(super::MAX_STORED_RESPONSE_EVENTS + 2);
        events.push(CanonicalEvent::ResponseStart(ResponseStart {
            response_id: ResponseId::try_new("bounded-stored-response")?,
            extensions: RawExtensions::default(),
        }));
        events.extend((0..super::MAX_STORED_RESPONSE_EVENTS).map(|_| {
            CanonicalEvent::TextDelta(TextDelta {
                text: "x".to_owned(),
                extensions: RawExtensions::default(),
            })
        }));
        events.push(CanonicalEvent::ResponseEnd(ResponseEnd::default()));

        let state = stored_response_state(
            events,
            Arc::new(Mutex::new(Vec::new())),
            stored_response_store()?,
        )?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let response =
            authorized(test::TestRequest::post().uri("/v1/responses").set_payload(
                r#"{"model":"mock-model","input":"hello","store":true,"stream":true}"#,
            ))
            .to_request();
        let response = test::call_service(&app, response).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains("response.failed"));
        assert!(!body.contains("response.completed"));

        let retrieve =
            authorized(test::TestRequest::get().uri("/v1/responses/bounded-stored-response"))
                .to_request();
        assert_eq!(
            test::call_service(&app, retrieve).await.status(),
            StatusCode::NOT_FOUND
        );
        Ok(())
    }

    #[actix_web::test]
    async fn storage_is_opt_in_and_missing_lineage_capability_fails_before_execution() -> TestResult
    {
        let observed_native = Arc::new(Mutex::new(Vec::new()));
        let store = stored_response_store()?;
        let state = stored_response_state(
            text_events()?,
            Arc::clone(&observed_native),
            Arc::clone(&store),
        )?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let unstored = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"hello"}"#),
        )
        .to_request();
        assert_eq!(
            test::call_service(&app, unstored).await.status(),
            StatusCode::OK
        );
        assert!(
            observed_native
                .lock()
                .map_err(|_| "native lock")?
                .first()
                .is_some_and(|payload| payload.get("store").is_none())
        );
        let retrieve = authorized(test::TestRequest::get().uri("/v1/responses/http-test-response"))
            .to_request();
        assert_eq!(
            test::call_service(&app, retrieve).await.status(),
            StatusCode::NOT_FOUND
        );

        let calls = Arc::new(AtomicUsize::new(0));
        let unsupported = ResponsesHttpState::with_metadata(
            Arc::new(CountingExecutor {
                calls: Arc::clone(&calls),
            }),
            Arc::new(FixedMetadata),
            test_authenticator()?,
            StreamCapacity::try_new(2)?,
        )
        .with_stored_response_store(store);
        let unsupported_app = test::init_service(
            App::new()
                .app_data(web::Data::new(unsupported))
                .configure(configure),
        )
        .await;
        let stored = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(r#"{"model":"mock-model","input":"hello","store":true}"#),
        )
        .to_request();
        assert_eq!(
            test::call_service(&unsupported_app, stored).await.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(calls.load(Ordering::Acquire), 0);
        Ok(())
    }

    struct CountingExecutor {
        calls: Arc<AtomicUsize>,
    }

    #[derive(Clone)]
    struct WebSocketExecutionObservation {
        transport: ResponsesClientTransport,
        native_payload: Option<serde_json::Value>,
        continuation: Option<ResponsesContinuationKind>,
        message_count: usize,
    }

    struct WebSocketTestExecutor {
        responses: Mutex<VecDeque<Vec<CanonicalEvent>>>,
        observations: Arc<Mutex<Vec<WebSocketExecutionObservation>>>,
    }

    struct WebSocketBlockingExecutor {
        dropped: Arc<AtomicBool>,
        calls: Arc<AtomicUsize>,
    }

    fn websocket_test_lineage() -> Result<ResponsesExecutionLineage, GatewayError> {
        Ok(ResponsesExecutionLineage::new(
            SnapshotVersion::try_new("websocket-config-v1").map_err(|_| super::internal_error())?,
            ProviderId::try_new("websocket-provider").map_err(|_| super::internal_error())?,
            UpstreamId::try_new("websocket-provider").map_err(|_| super::internal_error())?,
            EndpointId::try_new("websocket-channel").map_err(|_| super::internal_error())?,
            RouteId::try_new("websocket-route").map_err(|_| super::internal_error())?,
            RouteCandidateId::try_new("websocket-candidate")
                .map_err(|_| super::internal_error())?,
            CredentialId::try_new("websocket-credential").map_err(|_| super::internal_error())?,
            1,
        ))
    }

    impl ResponsesExecutor for WebSocketBlockingExecutor {
        fn execute(
            &self,
            _context: RequestContext,
            _request: CanonicalRequest,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            Box::pin(async { Err(super::internal_error()) })
        }

        fn execute_routed(
            &self,
            execution: ResponsesExecution,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            let result = execution
                .lineage_recorder()
                .ok_or_else(super::internal_error)
                .and_then(|recorder| recorder.record(websocket_test_lineage()?));
            let dropped = Arc::clone(&self.dropped);
            Box::pin(async move {
                result?;
                Ok(Box::new(DroppingSource {
                    dropped,
                    first_event_pending: true,
                }) as Box<dyn ResponsesEventSource>)
            })
        }
    }

    impl ResponsesExecutor for WebSocketTestExecutor {
        fn execute(
            &self,
            _context: RequestContext,
            _request: CanonicalRequest,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            Box::pin(async { Err(super::internal_error()) })
        }

        fn execute_routed(
            &self,
            execution: ResponsesExecution,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            let result = (|| {
                let lineage = match execution.continuation_pin() {
                    Some(pin) => pin.lineage().clone(),
                    None => websocket_test_lineage()?,
                };
                execution
                    .lineage_recorder()
                    .ok_or_else(super::internal_error)?
                    .record(lineage)?;
                let native_payload = execution
                    .native_payload()
                    .map(|payload| serde_json::from_slice(payload))
                    .transpose()
                    .map_err(|_| super::internal_error())?;
                self.observations
                    .lock()
                    .map_err(|_| super::internal_error())?
                    .push(WebSocketExecutionObservation {
                        transport: execution.client_transport(),
                        native_payload,
                        continuation: execution
                            .continuation_pin()
                            .map(ResponsesContinuationPin::kind),
                        message_count: execution.request().messages.len(),
                    });
                let events = self
                    .responses
                    .lock()
                    .map_err(|_| super::internal_error())?
                    .pop_front()
                    .ok_or_else(super::internal_error)?;
                Ok(Box::new(FiniteResponsesSource {
                    events: events.into(),
                }) as Box<dyn ResponsesEventSource>)
            })();
            Box::pin(async move { result })
        }
    }

    struct StoredResponsesExecutor {
        events: Vec<CanonicalEvent>,
        observed_native: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    #[derive(Clone)]
    struct ContinuityObservation {
        request: CanonicalRequest,
        pin: Option<ResponsesContinuationPin>,
        had_native_payload: bool,
    }

    struct ContinuityExecutor {
        responses: Mutex<VecDeque<Vec<CanonicalEvent>>>,
        observed: Arc<Mutex<Vec<ContinuityObservation>>>,
    }

    impl ResponsesExecutor for ContinuityExecutor {
        fn supports_stored_response_lineage(&self) -> bool {
            true
        }

        fn supports_stored_response_continuity(&self) -> bool {
            true
        }

        fn execute(
            &self,
            _context: RequestContext,
            _request: CanonicalRequest,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            Box::pin(async { Err(super::internal_error()) })
        }

        fn execute_routed(
            &self,
            execution: ResponsesExecution,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            let result = (|| {
                if let (Some(recorder), Some(pin)) =
                    (execution.lineage_recorder(), execution.continuation_pin())
                {
                    recorder.record(pin.lineage().clone())?;
                }
                self.observed
                    .lock()
                    .map_err(|_| super::internal_error())?
                    .push(ContinuityObservation {
                        request: execution.request().clone(),
                        pin: execution.continuation_pin().cloned(),
                        had_native_payload: execution.native_payload().is_some(),
                    });
                let events = self
                    .responses
                    .lock()
                    .map_err(|_| super::internal_error())?
                    .pop_front()
                    .ok_or_else(super::internal_error)?;
                Ok(Box::new(FiniteResponsesSource {
                    events: events.into(),
                }) as Box<dyn ResponsesEventSource>)
            })();
            Box::pin(async move { result })
        }
    }

    impl ResponsesExecutor for StoredResponsesExecutor {
        fn supports_stored_response_lineage(&self) -> bool {
            true
        }

        fn execute(
            &self,
            _context: RequestContext,
            _request: gateway_core::CanonicalRequest,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            Box::pin(async { Err(super::internal_error()) })
        }

        fn execute_routed(
            &self,
            execution: ResponsesExecution,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            let result = (|| {
                if let Some(recorder) = execution.lineage_recorder() {
                    recorder.record(ResponsesExecutionLineage::new(
                        SnapshotVersion::try_new("stored-config-v1")
                            .map_err(|_| super::internal_error())?,
                        ProviderId::try_new("stored-provider")
                            .map_err(|_| super::internal_error())?,
                        UpstreamId::try_new("stored-provider")
                            .map_err(|_| super::internal_error())?,
                        EndpointId::try_new("stored-channel")
                            .map_err(|_| super::internal_error())?,
                        RouteId::try_new("stored-route").map_err(|_| super::internal_error())?,
                        RouteCandidateId::try_new("stored-candidate")
                            .map_err(|_| super::internal_error())?,
                        CredentialId::try_new("stored-credential")
                            .map_err(|_| super::internal_error())?,
                        11,
                    ))?;
                }
                let native: serde_json::Value = serde_json::from_slice(
                    execution
                        .native_payload()
                        .ok_or_else(super::internal_error)?,
                )
                .map_err(|_| super::internal_error())?;
                self.observed_native
                    .lock()
                    .map_err(|_| super::internal_error())?
                    .push(native);
                Ok(Box::new(FiniteResponsesSource {
                    events: self.events.clone().into(),
                }) as Box<dyn ResponsesEventSource>)
            })();
            Box::pin(async move { result })
        }
    }

    struct FiniteResponsesSource {
        events: VecDeque<CanonicalEvent>,
    }

    impl ResponsesEventSource for FiniteResponsesSource {
        fn next_event(
            &mut self,
        ) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
            let event = self.events.pop_front();
            Box::pin(async move { Ok(event) })
        }
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
