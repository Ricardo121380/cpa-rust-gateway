//! Executable native Kiro inference boundary.
//!
//! The request, EventStream framing, semantic mapping, credential, and endpoint-policy modules
//! remain independently testable and socket-free. This module composes them only through an
//! injected one-send transport. It never discovers local Kiro-RS state, reads ambient proxy
//! configuration, refreshes credentials, retries, or fails over to another account.

use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    sync::Arc,
};

use gateway_core::{
    CanonicalEvent, CanonicalRequest, ErrorScope, GatewayError, GatewayErrorCode, ProviderId,
    RequestContext, ResponseId, StreamError,
};
use gateway_provider::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};
use gateway_upstream::{
    AdmittedEgressTarget, EgressDnsResolver, EgressPolicy, UpstreamClientPool, UpstreamHttpMethod,
    UpstreamHttpRequest, UpstreamHttpResponse, UpstreamTransportProfile,
};
use zeroize::Zeroizing;

use crate::{
    conversation_request::{KiroConversationContext, KiroConversationRequestBuilder},
    credential::{KiroCredential, KiroCredentialKind},
    endpoint_policy::KiroEndpointPolicy,
    event_semantics::KiroEventSemanticMapper,
    event_stream::KiroEventStreamDecoder,
    failure_classification::{KiroFailureSignal, classify_kiro_http_failure},
    profile_arn::{KiroProfileArnResolution, KiroProfileArnSource, inject_profile_arn},
};

const KIRO_PROVIDER_ID: &str = "kiro";
const KIRO_EVENT_STREAM_ACCEPT: &str = "application/vnd.amazon.eventstream";

/// A safe projection of the Kiro response content type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroResponseContentType {
    /// The expected AWS `EventStream` response body.
    EventStream,
    /// Any missing, malformed, or unsupported response content type.
    OtherOrMissing,
}

/// Pull-only raw bytes from one Kiro HTTP response body.
pub trait KiroResponseBody: Send {
    /// Returns the next opaque body chunk or the normal end of the response.
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>>;
}

/// A status and safe content-type projection plus one pull-only Kiro response body.
pub struct KiroTransportResponse {
    status: u16,
    content_type: KiroResponseContentType,
    failure_signal: KiroFailureSignal,
    body: Box<dyn KiroResponseBody>,
}

impl KiroTransportResponse {
    /// Creates one response handoff after headers have been classified by the transport.
    #[must_use]
    pub fn new(
        status: u16,
        content_type: KiroResponseContentType,
        failure_signal: KiroFailureSignal,
        body: Box<dyn KiroResponseBody>,
    ) -> Self {
        Self {
            status,
            content_type,
            failure_signal,
            body,
        }
    }

    fn into_parts(
        self,
    ) -> (
        u16,
        KiroResponseContentType,
        KiroFailureSignal,
        Box<dyn KiroResponseBody>,
    ) {
        (
            self.status,
            self.content_type,
            self.failure_signal,
            self.body,
        )
    }
}

impl fmt::Debug for KiroTransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroTransportResponse")
            .field("status", &self.status)
            .field("content_type", &self.content_type)
            .field("failure_signal", &self.failure_signal)
            .field("body", &"<streaming>")
            .finish()
    }
}

/// One native Kiro submission with an opaque body and redacted credential header.
pub struct KiroOutboundRequest {
    target: String,
    headers: BTreeMap<String, String>,
    authorization: Zeroizing<String>,
    body: Vec<u8>,
}

impl KiroOutboundRequest {
    /// Returns the fixed IDE or CLI endpoint selected by the immutable policy.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.target
    }

    /// Returns one request header for immediate transport construction.
    ///
    /// The authorization value must not be logged or persisted by callers.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("authorization") {
            Some(self.authorization.as_str())
        } else {
            self.headers
                .iter()
                .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        }
    }

    /// Returns the generated JSON request body without giving it a diagnostic representation.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Transfers this request only to the exact target admitted by the shared egress policy.
    ///
    /// # Errors
    ///
    /// Returns a safe egress rejection when the admitted URL differs, or an internal error when
    /// the generated fixed headers violate the shared transport boundary.
    pub fn into_transport_request(
        self,
        admitted_target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GatewayError> {
        if admitted_target.request_url().as_str() != self.target {
            return Err(egress_rejected_error());
        }
        let mut headers = self.headers.into_iter().collect::<Vec<_>>();
        headers.push(("authorization".to_owned(), self.authorization.to_string()));
        UpstreamHttpRequest::try_new(
            admitted_target,
            UpstreamHttpMethod::Post,
            headers,
            self.body,
        )
        .map_err(|_| internal_error())
    }
}

impl fmt::Debug for KiroOutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroOutboundRequest")
            .field("target", &"<redacted>")
            .field("header_names", &self.headers.keys().collect::<Vec<_>>())
            .field("authorization", &"<redacted>")
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Sends one already-built native Kiro request through an explicitly injected transport.
pub trait KiroTransport: Send + Sync {
    /// Sends exactly once. Credential refresh, retry, failover, and scheduling are deliberately
    /// outside this Provider boundary.
    fn send(
        &self,
        request: KiroOutboundRequest,
    ) -> ProviderFuture<'_, Result<KiroTransportResponse, GatewayError>>;
}

/// Production transport that uses the shared DNS-pinned client after exact egress admission.
pub struct KiroUpstreamTransport {
    egress_policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    client_pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
}

impl KiroUpstreamTransport {
    /// Creates a native Kiro transport from explicit network-boundary dependencies.
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

impl fmt::Debug for KiroUpstreamTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroUpstreamTransport")
            .field("egress_policy", self.egress_policy.id())
            .field("resolver", &"<injected>")
            .field("client_pool", &self.client_pool)
            .field("profile", &self.profile)
            .finish()
    }
}

impl KiroTransport for KiroUpstreamTransport {
    fn send(
        &self,
        outbound: KiroOutboundRequest,
    ) -> ProviderFuture<'_, Result<KiroTransportResponse, GatewayError>> {
        let admitted = self
            .egress_policy
            .admit_url(outbound.url(), self.resolver.as_ref())
            .map_err(gateway_upstream::EgressAdmissionError::gateway_error);
        let request = admitted.and_then(|target| outbound.into_transport_request(target));
        let pool = self.client_pool.clone();
        let profile = self.profile.clone();

        Box::pin(async move {
            let response = pool.send(request?, &profile).await?;
            Ok(KiroTransportResponse::new(
                response.status(),
                response_content_type(&response),
                KiroFailureSignal::None,
                Box::new(UpstreamResponseBody { response }),
            ))
        })
    }
}

/// A native Kiro [`InferenceAdapter`] for one Credential, endpoint, conversation, and model.
#[derive(Clone)]
pub struct KiroInferenceAdapter {
    provider_id: ProviderId,
    credential: KiroCredential,
    policy: KiroEndpointPolicy,
    conversation: KiroConversationContext,
    upstream_model: String,
    profile: KiroProfileArnResolution,
    transport: Arc<dyn KiroTransport>,
}

impl KiroInferenceAdapter {
    /// Builds an adapter with all credential, endpoint, model, profile, and transport selection
    /// injected by earlier bounded stages.
    ///
    /// # Errors
    ///
    /// Returns a safe client-request error for a blank model or an internal error if the fixed
    /// Provider identity cannot be constructed.
    pub fn try_new(
        credential: KiroCredential,
        policy: KiroEndpointPolicy,
        conversation: KiroConversationContext,
        upstream_model: impl Into<String>,
        profile: KiroProfileArnResolution,
        transport: Arc<dyn KiroTransport>,
    ) -> Result<Self, GatewayError> {
        let upstream_model = upstream_model.into();
        if upstream_model.trim().is_empty() {
            return Err(client_request_error());
        }
        let provider_id =
            ProviderId::try_new(KIRO_PROVIDER_ID.to_owned()).map_err(|_| internal_error())?;
        if !profile_matches_credential(credential.kind(), profile.source()) {
            return Err(internal_error());
        }
        Ok(Self {
            provider_id,
            credential,
            policy,
            conversation,
            upstream_model,
            profile,
            transport,
        })
    }
}

impl fmt::Debug for KiroInferenceAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroInferenceAdapter")
            .field("provider_id", &self.provider_id)
            .field("credential", &self.credential)
            .field("policy", &self.policy)
            .field("conversation", &"<redacted>")
            .field("upstream_model", &"<redacted>")
            .field("profile", &self.profile)
            .field("transport", &"<injected>")
            .finish()
    }
}

impl ProviderAdapter for KiroInferenceAdapter {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
}

impl InferenceAdapter for KiroInferenceAdapter {
    fn execute(
        &self,
        context: RequestContext,
        request: CanonicalRequest,
    ) -> ProviderFuture<'_, Result<Box<dyn CanonicalEventSource>, GatewayError>> {
        let credential = self.credential.clone();
        let policy = self.policy.clone();
        let conversation = self.conversation.clone();
        let upstream_model = self.upstream_model.clone();
        let profile = self.profile.clone();
        let transport = Arc::clone(&self.transport);

        Box::pin(async move {
            let outbound = build_outbound(
                &credential,
                &policy,
                &conversation,
                &upstream_model,
                &profile,
                &request,
            )?;
            let response = transport.send(outbound).await?;
            let (status, content_type, failure_signal, body) = response.into_parts();
            if !(200..=299).contains(&status) {
                return Err(classify_kiro_http_failure(status, failure_signal)
                    .error()
                    .clone());
            }
            if content_type != KiroResponseContentType::EventStream {
                return Err(provider_protocol_error());
            }
            let response_id = ResponseId::try_new(context.request_id().as_str().to_owned())
                .map_err(|_| internal_error())?;
            Ok(Box::new(KiroStreamingEventSource::new(body, response_id))
                as Box<dyn CanonicalEventSource>)
        })
    }
}

fn build_outbound(
    credential: &KiroCredential,
    policy: &KiroEndpointPolicy,
    conversation: &KiroConversationContext,
    upstream_model: &str,
    profile: &KiroProfileArnResolution,
    request: &CanonicalRequest,
) -> Result<KiroOutboundRequest, GatewayError> {
    let conversation =
        KiroConversationRequestBuilder::build(policy, conversation, upstream_model, request)
            .map_err(|_| client_request_error())?;
    let mut body = conversation.into_body();
    inject_profile_arn(&mut body, profile).map_err(|_| internal_error())?;
    let body = serde_json::to_vec(&body).map_err(|_| internal_error())?;
    let mut headers = policy.request_headers(credential.kind());
    headers.insert("accept".to_owned(), KIRO_EVENT_STREAM_ACCEPT.to_owned());
    let secret = match credential.kind() {
        KiroCredentialKind::ApiKey => credential.api_key(),
        KiroCredentialKind::Social | KiroCredentialKind::Enterprise => credential.access_token(),
    }
    .map_err(|_| credential_unavailable_error())?;
    if secret.is_empty() || !secret.bytes().all(|value| value.is_ascii_graphic()) {
        return Err(credential_unavailable_error());
    }
    Ok(KiroOutboundRequest {
        target: policy.url().as_str().to_owned(),
        headers,
        authorization: Zeroizing::new(format!("Bearer {secret}")),
        body,
    })
}

const fn profile_matches_credential(
    credential: KiroCredentialKind,
    profile: KiroProfileArnSource,
) -> bool {
    matches!(
        (credential, profile),
        (
            KiroCredentialKind::Social,
            KiroProfileArnSource::BuilderDefault
        ) | (
            KiroCredentialKind::Enterprise,
            KiroProfileArnSource::EnterpriseLookup | KiroProfileArnSource::EnterpriseFallback
        ) | (
            KiroCredentialKind::ApiKey,
            KiroProfileArnSource::ApiKeyOmitted
        )
    )
}

struct UpstreamResponseBody {
    response: UpstreamHttpResponse,
}

impl KiroResponseBody for UpstreamResponseBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            self.response
                .next_chunk()
                .await
                .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
        })
    }
}

struct KiroStreamingEventSource {
    body: Box<dyn KiroResponseBody>,
    framing: KiroEventStreamDecoder,
    semantics: KiroEventSemanticMapper,
    pending: VecDeque<CanonicalEvent>,
    lifecycle: KiroStreamLifecycle,
    response_started: bool,
    terminal_failure_emitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KiroStreamLifecycle {
    AwaitingFirstFrame,
    Streaming,
    Finished,
}

impl KiroStreamingEventSource {
    fn new(body: Box<dyn KiroResponseBody>, response_id: ResponseId) -> Self {
        Self {
            body,
            framing: KiroEventStreamDecoder::new(),
            semantics: KiroEventSemanticMapper::new(response_id),
            pending: VecDeque::new(),
            lifecycle: KiroStreamLifecycle::AwaitingFirstFrame,
            response_started: false,
            terminal_failure_emitted: false,
        }
    }

    fn next_pending(&mut self) -> Option<CanonicalEvent> {
        let event = self.pending.pop_front()?;
        if matches!(event, CanonicalEvent::ResponseStart(_)) {
            self.response_started = true;
        }
        Some(event)
    }

    fn map_available_frames(&mut self) -> Result<(), GatewayError> {
        while let Some(frame) = self
            .framing
            .next_frame()
            .map_err(|_| provider_protocol_error())?
        {
            if self.lifecycle == KiroStreamLifecycle::AwaitingFirstFrame {
                self.pending.extend(
                    self.semantics
                        .start()
                        .map_err(|_| provider_protocol_error())?,
                );
                self.lifecycle = KiroStreamLifecycle::Streaming;
            }
            self.pending.extend(
                self.semantics
                    .push_frame(&frame)
                    .map_err(|_| provider_protocol_error())?,
            );
        }
        Ok(())
    }

    fn finish_stream(&mut self) -> Result<(), GatewayError> {
        self.framing
            .finish()
            .map_err(|_| provider_protocol_error())?;
        self.map_available_frames()?;
        if self.lifecycle != KiroStreamLifecycle::Streaming {
            return Err(provider_protocol_error());
        }
        self.pending.extend(
            self.semantics
                .finish()
                .map_err(|_| provider_protocol_error())?,
        );
        self.lifecycle = KiroStreamLifecycle::Finished;
        Ok(())
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
        self.lifecycle = KiroStreamLifecycle::Finished;
        Ok(Some(CanonicalEvent::StreamError(StreamError { error })))
    }
}

impl CanonicalEventSource for KiroStreamingEventSource {
    fn next_event(&mut self) -> ProviderFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            if let Some(event) = self.next_pending() {
                return Ok(Some(event));
            }
            if self.lifecycle == KiroStreamLifecycle::Finished {
                return Ok(None);
            }

            loop {
                match self.body.next_chunk().await {
                    Ok(Some(chunk)) => {
                        if let Err(error) = self
                            .framing
                            .feed(&chunk)
                            .map_err(|_| provider_protocol_error())
                            .and_then(|()| self.map_available_frames())
                        {
                            return self.terminal_failure(error);
                        }
                        if let Some(event) = self.next_pending() {
                            return Ok(Some(event));
                        }
                    }
                    Ok(None) => match self.finish_stream() {
                        Ok(()) => return Ok(self.next_pending()),
                        Err(error) => return self.terminal_failure(error),
                    },
                    Err(error) => return self.terminal_failure(error),
                }
            }
        })
    }
}

fn response_content_type(response: &UpstreamHttpResponse) -> KiroResponseContentType {
    match response
        .header("content-type")
        .and_then(|value| value.to_str().ok())
    {
        Some(value) if value.starts_with(KIRO_EVENT_STREAM_ACCEPT) => {
            KiroResponseContentType::EventStream
        }
        _ => KiroResponseContentType::OtherOrMissing,
    }
}

const fn client_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

const fn credential_unavailable_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::CredentialUnavailable,
        ErrorScope::Credential,
    )
}

const fn egress_rejected_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
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
