//! Executable Grok Build inference boundary.
//!
//! The request builder and decoders remain deterministic and socket-free.  This module joins
//! them through an explicitly injected transport, so the real provider can be exercised without
//! putting an OAuth cache, a proxy setting, or an HTTP client in an application entry point.

use std::{collections::VecDeque, fmt, sync::Arc};

use gateway_core::{
    CanonicalEvent, CanonicalRequest, ErrorScope, GatewayError, GatewayErrorCode, ProviderId,
    RequestContext, StreamError,
};
use gateway_provider::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};
use gateway_upstream::{
    EgressDnsResolver, EgressPolicy, UpstreamClientPool, UpstreamHttpResponse,
    UpstreamTransportProfile,
};
use protocol_openai_responses::ResponseMode;

use crate::{
    GrokBuildAccountEvidence, GrokBuildCredential, GrokBuildRateLimitEvidence,
    GrokBuildResponsesDecoder, GrokBuildResponsesHttpError, GrokBuildResponsesOutboundRequest,
    GrokBuildResponsesRequestBuilder, GrokBuildResponsesStreamDecoder,
    MAX_GROK_BUILD_ERROR_BODY_BYTES, MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES,
    classify_grok_build_failure,
};

const GROK_BUILD_PROVIDER_ID: &str = "grok.build";

/// The upstream representation selected for one execution adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildExecutionMode {
    /// Request and decode one completed JSON Responses object.
    NonStreaming,
    /// Request and incrementally decode a Responses SSE stream.
    Streaming,
}

impl GrokBuildExecutionMode {
    const fn response_mode(self) -> ResponseMode {
        match self {
            Self::NonStreaming => ResponseMode::NonStreaming,
            Self::Streaming => ResponseMode::Streaming,
        }
    }
}

/// Safe classification of an upstream response content type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildResponseContentType {
    /// An `application/json` response.
    Json,
    /// A `text/event-stream` response.
    EventStream,
    /// Missing, malformed, or unsupported content type.
    OtherOrMissing,
}

/// Safe classification of a non-streaming response content encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildResponseContentEncoding {
    /// No encoding or explicit identity encoding.
    Identity,
    /// Gzip content encoding.
    Gzip,
    /// Missing from the supported set.
    Other,
}

impl GrokBuildResponseContentEncoding {
    const fn decoder_value(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Gzip => "gzip",
            Self::Other => "unsupported",
        }
    }
}

/// Pull-only raw body received from a pre-approved Grok Build transport.
pub trait GrokBuildResponseBody: Send {
    /// Returns the next opaque body chunk or the normal end of the response.
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>>;
}

/// A status and safe header projection plus a pull-only raw response body.
pub struct GrokBuildTransportResponse {
    status: u16,
    content_type: GrokBuildResponseContentType,
    content_encoding: GrokBuildResponseContentEncoding,
    body: Box<dyn GrokBuildResponseBody>,
}

impl GrokBuildTransportResponse {
    /// Creates a response handoff. Header values must be classified before crossing this boundary.
    #[must_use]
    pub fn new(
        status: u16,
        content_type: GrokBuildResponseContentType,
        content_encoding: GrokBuildResponseContentEncoding,
        body: Box<dyn GrokBuildResponseBody>,
    ) -> Self {
        Self {
            status,
            content_type,
            content_encoding,
            body,
        }
    }

    fn into_parts(
        self,
    ) -> (
        u16,
        GrokBuildResponseContentType,
        GrokBuildResponseContentEncoding,
        Box<dyn GrokBuildResponseBody>,
    ) {
        (
            self.status,
            self.content_type,
            self.content_encoding,
            self.body,
        )
    }
}

impl fmt::Debug for GrokBuildTransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildTransportResponse")
            .field("status", &self.status)
            .field("content_type", &self.content_type)
            .field("content_encoding", &self.content_encoding)
            .field("body", &"<streaming>")
            .finish()
    }
}

/// Sends an already-built request through a caller-controlled transport boundary.
pub trait GrokBuildTransport: Send + Sync {
    /// Sends exactly one request. Retries, refreshes, failover, and scheduling are intentionally
    /// not implicit in this boundary.
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokBuildTransportResponse, GatewayError>>;
}

/// Production transport that uses the shared, DNS-pinned upstream client only after policy admission.
pub struct GrokBuildUpstreamTransport {
    egress_policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    client_pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
}

impl GrokBuildUpstreamTransport {
    /// Creates a production transport from explicit egress policy, resolver, pool, and profile.
    #[must_use]
    pub fn new(
        egress_policy: EgressPolicy,
        resolver: Arc<dyn EgressDnsResolver>,
        client_pool: UpstreamClientPool,
        profile: UpstreamTransportProfile,
    ) -> Self {
        Self {
            egress_policy,
            resolver,
            client_pool,
            profile,
        }
    }
}

impl fmt::Debug for GrokBuildUpstreamTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildUpstreamTransport")
            .field("egress_policy", self.egress_policy.id())
            .field("resolver", &"<injected>")
            .field("client_pool", &self.client_pool)
            .field("profile", &self.profile)
            .finish()
    }
}

impl GrokBuildTransport for GrokBuildUpstreamTransport {
    fn send(
        &self,
        outbound: GrokBuildResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        let admitted = self
            .egress_policy
            .admit_url(outbound.url(), self.resolver.as_ref())
            .map_err(gateway_upstream::EgressAdmissionError::gateway_error);
        let request = admitted.and_then(|target| outbound.into_transport_request(target));
        let pool = self.client_pool.clone();
        let profile = self.profile.clone();

        Box::pin(async move {
            let response = pool.send(request?, &profile).await?;
            Ok(GrokBuildTransportResponse::new(
                response.status(),
                content_type(&response),
                content_encoding(&response),
                Box::new(UpstreamResponseBody { response }),
            ))
        })
    }
}

/// A real Grok Build [`InferenceAdapter`] with injected credential and transport state.
#[derive(Clone)]
pub struct GrokBuildInferenceAdapter {
    provider_id: ProviderId,
    credential: GrokBuildCredential,
    upstream_model: String,
    mode: GrokBuildExecutionMode,
    transport: Arc<dyn GrokBuildTransport>,
}

impl GrokBuildInferenceAdapter {
    /// Builds one adapter for one selected Build credential, model, and response mode.
    ///
    /// The model is checked again by the request builder, which owns the canonical wire
    /// validation. No token, endpoint, proxy, or refresh behaviour is inferred here.
    ///
    /// # Errors
    ///
    /// Returns a safe client-request error for a blank model or an internal error if the fixed
    /// provider identity could not be constructed.
    pub fn try_new(
        credential: GrokBuildCredential,
        upstream_model: impl Into<String>,
        mode: GrokBuildExecutionMode,
        transport: Arc<dyn GrokBuildTransport>,
    ) -> Result<Self, GatewayError> {
        let upstream_model = upstream_model.into();
        if upstream_model.trim().is_empty() {
            return Err(client_request_error());
        }
        let provider_id =
            ProviderId::try_new(GROK_BUILD_PROVIDER_ID.to_owned()).map_err(|_| internal_error())?;
        Ok(Self {
            provider_id,
            credential,
            upstream_model,
            mode,
            transport,
        })
    }
}

impl fmt::Debug for GrokBuildInferenceAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildInferenceAdapter")
            .field("provider_id", &self.provider_id)
            .field("credential", &self.credential)
            .field("upstream_model", &"<redacted>")
            .field("mode", &self.mode)
            .field("transport", &"<injected>")
            .finish()
    }
}

impl ProviderAdapter for GrokBuildInferenceAdapter {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
}

impl InferenceAdapter for GrokBuildInferenceAdapter {
    fn execute(
        &self,
        _context: RequestContext,
        request: CanonicalRequest,
    ) -> ProviderFuture<'_, Result<Box<dyn CanonicalEventSource>, GatewayError>> {
        let credential = self.credential.clone();
        let upstream_model = self.upstream_model.clone();
        let mode = self.mode;
        let transport = Arc::clone(&self.transport);

        Box::pin(async move {
            let outbound = GrokBuildResponsesRequestBuilder::build(
                &credential,
                &upstream_model,
                &request,
                mode.response_mode(),
            )?;
            let response = transport.send(outbound).await?;
            let (status, content_type, content_encoding, mut body) = response.into_parts();
            if !(200..=299).contains(&status) {
                let bytes = read_bounded_body(&mut *body, MAX_GROK_BUILD_ERROR_BODY_BYTES).await?;
                let envelope = GrokBuildResponsesHttpError::parse(status, &bytes)?;
                return Err(classify_grok_build_failure(
                    envelope.status(),
                    envelope.signal(),
                    GrokBuildAccountEvidence::None,
                    GrokBuildRateLimitEvidence::None,
                )
                .error()
                .clone());
            }

            match mode {
                GrokBuildExecutionMode::NonStreaming
                    if content_type == GrokBuildResponseContentType::Json =>
                {
                    let bytes =
                        read_bounded_body(&mut *body, MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES)
                            .await?;
                    let decoded =
                        GrokBuildResponsesDecoder::decode_non_streaming_with_content_encoding(
                            Some(content_encoding.decoder_value()),
                            &bytes,
                        )?;
                    Ok(Box::new(BufferedEventSource::new(decoded.into_events()))
                        as Box<dyn CanonicalEventSource>)
                }
                GrokBuildExecutionMode::Streaming
                    if content_type == GrokBuildResponseContentType::EventStream =>
                {
                    Ok(Box::new(StreamingEventSource::new(body)) as Box<dyn CanonicalEventSource>)
                }
                _ => Err(provider_protocol_error()),
            }
        })
    }
}

struct UpstreamResponseBody {
    response: UpstreamHttpResponse,
}

impl GrokBuildResponseBody for UpstreamResponseBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            self.response
                .next_chunk()
                .await
                .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
        })
    }
}

struct BufferedEventSource {
    events: VecDeque<CanonicalEvent>,
}

impl BufferedEventSource {
    fn new(events: Vec<CanonicalEvent>) -> Self {
        Self {
            events: events.into(),
        }
    }
}

impl CanonicalEventSource for BufferedEventSource {
    fn next_event(&mut self) -> ProviderFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move { Ok(self.events.pop_front()) })
    }
}

struct StreamingEventSource {
    body: Box<dyn GrokBuildResponseBody>,
    decoder: GrokBuildResponsesStreamDecoder,
    pending: VecDeque<CanonicalEvent>,
    response_started: bool,
    terminal_failure_emitted: bool,
    finished: bool,
}

impl StreamingEventSource {
    fn new(body: Box<dyn GrokBuildResponseBody>) -> Self {
        Self {
            body,
            decoder: GrokBuildResponsesStreamDecoder::new(),
            pending: VecDeque::new(),
            response_started: false,
            terminal_failure_emitted: false,
            finished: false,
        }
    }

    fn next_pending(&mut self) -> Option<CanonicalEvent> {
        let event = self.pending.pop_front()?;
        if matches!(event, CanonicalEvent::ResponseStart(_)) {
            self.response_started = true;
        }
        Some(event)
    }

    fn terminal_failure(
        &mut self,
        error: GatewayError,
    ) -> Result<Option<CanonicalEvent>, GatewayError> {
        if !self.response_started {
            return Err(error);
        }
        if self.terminal_failure_emitted {
            return Ok(None);
        }
        self.terminal_failure_emitted = true;
        self.finished = true;
        Ok(Some(CanonicalEvent::StreamError(StreamError { error })))
    }
}

impl CanonicalEventSource for StreamingEventSource {
    fn next_event(&mut self) -> ProviderFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            if let Some(event) = self.next_pending() {
                return Ok(Some(event));
            }
            if self.finished {
                return Ok(None);
            }

            loop {
                match self.body.next_chunk().await {
                    Ok(Some(chunk)) => match self.decoder.push_bytes(&chunk) {
                        Ok(events) => {
                            self.pending.extend(events);
                            if let Some(event) = self.next_pending() {
                                return Ok(Some(event));
                            }
                        }
                        Err(error) => return self.terminal_failure(error),
                    },
                    Ok(None) => {
                        self.finished = true;
                        return match self.decoder.finish() {
                            Ok(()) => Ok(None),
                            Err(error) => self.terminal_failure(error),
                        };
                    }
                    Err(error) => return self.terminal_failure(error),
                }
            }
        })
    }
}

async fn read_bounded_body(
    body: &mut dyn GrokBuildResponseBody,
    maximum_bytes: usize,
) -> Result<Vec<u8>, GatewayError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next_chunk().await? {
        let length = bytes
            .len()
            .checked_add(chunk.len())
            .ok_or_else(provider_protocol_error)?;
        if length > maximum_bytes {
            return Err(provider_protocol_error());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn content_type(response: &UpstreamHttpResponse) -> GrokBuildResponseContentType {
    match response
        .header("content-type")
        .and_then(|value| value.to_str().ok())
    {
        Some(value) if value.starts_with("application/json") => GrokBuildResponseContentType::Json,
        Some(value) if value.starts_with("text/event-stream") => {
            GrokBuildResponseContentType::EventStream
        }
        _ => GrokBuildResponseContentType::OtherOrMissing,
    }
}

fn content_encoding(response: &UpstreamHttpResponse) -> GrokBuildResponseContentEncoding {
    match response
        .header("content-encoding")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
    {
        None | Some("identity") => GrokBuildResponseContentEncoding::Identity,
        Some("gzip") => GrokBuildResponseContentEncoding::Gzip,
        Some(_) => GrokBuildResponseContentEncoding::Other,
    }
}

const fn client_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

const fn provider_protocol_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        ErrorScope::Provider,
    )
}
