//! Immutable route snapshot and two-stage scheduling boundary.
//!
//! P1 also keeps the Provider execution contract behind this crate so an HTTP transport can
//! execute a canonical request without importing Provider traits or concrete Provider types.

#![deny(unsafe_code)]

mod attempt_orchestrator;
mod credential_scheduler;
mod execution_lineage;
mod protocol_transform;
mod provider_scoped_selector;
mod response_transform;
mod route_explain;
mod route_scheduler;
mod route_snapshot;
mod runtime_health;
mod runtime_management_status;
mod runtime_probe;
mod runtime_quota;
mod token_count;

use std::{fmt, future::Future, pin::Pin, sync::Arc, time::Duration};

use gateway_core::{
    CanonicalEvent, CanonicalRequest, GatewayError, ProviderId, RequestContext, RouteId,
    TransparentRetryGate,
};
use gateway_provider::{
    CanonicalEventSource, DeterministicMockProvider, InferenceAdapter, MockEmission, MockFixture,
    ProviderFuture,
};

pub use attempt_orchestrator::{
    AttemptDriver, AttemptExclusionSet, AttemptFailure, AttemptFuture, AttemptOrchestrator,
    AttemptOrchestratorConfig, AttemptOrchestratorConfigError, DEFAULT_RATE_LIMIT_COOLDOWN,
    DEFAULT_TRANSIENT_COOLDOWN, StartedAttempt,
};
pub use credential_scheduler::{RouteCredentialScheduler, SelectedRouteCredential};
pub use execution_lineage::{
    ResponsesContinuationKind, ResponsesContinuationPin, ResponsesExecutionLineage,
    ResponsesExecutionLineageRecorder,
};
pub use gateway_catalog::CapabilitySet;
pub use gateway_core::TransparentRetryGate as AttemptRetryGate;
pub use protocol_transform::{
    NativePayloadAvailability, ProjectedProtocolRequest, ProtocolFormat,
    ProtocolTransformAdmission, ProtocolTransformInput, ProtocolTransformRejection,
    analyze_protocol_transform, project_protocol_request, project_registered_protocol_request,
    protocol_pair_is_publishable, protocol_pair_is_registered,
};
pub use provider_scoped_selector::{
    ProviderScopedCandidate, ProviderScopedCandidateDecision, ProviderScopedHealth,
    ProviderScopedPriceEvidence, ProviderScopedPriceRates, ProviderScopedQuota,
    ProviderScopedRejection, ProviderScopedSelection, ProviderScopedSelector,
    ProviderScopedSelectorError,
};
pub use response_transform::{
    ProtocolResponseProjector, ProtocolResponseRejection, project_protocol_response,
};
pub use route_explain::{
    ProviderScopedRouteExplainError, ProviderScopedRouteExplainInput,
    ProviderScopedRouteExplainSnapshot, RouteExplainCandidate, RouteExplainCandidateReason,
    RouteExplainCredential, RouteExplainCredentialReason, RouteExplainError, RouteExplainInput,
    RouteExplainProjectedSelection, RouteExplainSnapshot,
};
pub use route_scheduler::RouteCandidateScheduler;
pub use route_snapshot::{
    MAX_SCHEDULE_SLOTS_PER_PRIORITY_TIER, PreparedSnapshotPublication, RouteSnapshot,
    RouteSnapshotBuildError, RouteSnapshotInput, RouteSnapshotRegistry, SnapshotAccessGroup,
    SnapshotAuthenticatedClient, SnapshotCatalogAdmission, SnapshotClientKeyAuthenticator,
    SnapshotClientKeyClock, SnapshotClientKeyClockError, SnapshotClientKeyView,
    SnapshotPriorityTierSchedule, SnapshotPublicModel, SnapshotRegistryError, SnapshotRoute,
    SnapshotRouteCandidate, SnapshotRouteCandidateInput, SnapshotRoutePolicy,
    SnapshotRouteSchedule, SnapshotTransformMode, SnapshotTransition, SnapshotVersion,
    SystemSnapshotClientKeyClock,
};
pub use runtime_health::{
    DEFAULT_RUNTIME_HEALTH_SHARD_COUNT, MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD,
    MAX_RUNTIME_HEALTH_SHARD_COUNT, RuntimeCredentialAccountStatus,
    RuntimeHealthAccountRecoveryProbe, RuntimeHealthAccountRecoveryResult,
    RuntimeHealthAvailability, RuntimeHealthCircuitProbe, RuntimeHealthCircuitProbeResult,
    RuntimeHealthClock, RuntimeHealthClockError, RuntimeHealthError, RuntimeHealthKey,
    RuntimeHealthRegistry, RuntimeHealthRegistryBuildError, SystemRuntimeHealthClock,
};
pub use runtime_management_status::{
    RuntimeManagementQuotaStatus, RuntimeManagementStatusQuery, RuntimeManagementStatusQueryError,
    RuntimeManagementStatusSnapshot, RuntimeManagementStatusTarget,
    RuntimeManagementStatusTargetError,
};
pub use runtime_probe::{
    DEFAULT_RUNTIME_HEALTH_PROBE_EWMA_ALPHA_PER_MILLE, RUNTIME_HEALTH_PROBE_EWMA_SCALE_PER_MILLE,
    RuntimeHealthCircuitProbeOutcome, RuntimeHealthProbeCompletionError, RuntimeHealthProbeError,
    RuntimeHealthProbeOutcome, RuntimeHealthProbePolicy, RuntimeHealthProbePolicyError,
    RuntimeHealthProbeRegistry, RuntimeHealthProbeSnapshot, RuntimeHealthProbeTarget,
    RuntimeHealthProbeTargetError,
};
pub use runtime_quota::{
    DEFAULT_RUNTIME_QUOTA_SHARD_COUNT, MAX_QUOTA_WINDOW_LABEL_BYTES,
    MAX_QUOTA_WINDOWS_PER_SNAPSHOT, MAX_RUNTIME_QUOTA_ENTRIES_PER_SHARD,
    MAX_RUNTIME_QUOTA_SHARD_COUNT, QuotaConfidence, QuotaSnapshot, QuotaSnapshotError, QuotaSource,
    QuotaWindow, QuotaWindowError, RuntimeQuotaAvailability, RuntimeQuotaError,
    RuntimeQuotaRecoveryProbe, RuntimeQuotaRegistry, RuntimeQuotaRegistryBuildError,
    RuntimeQuotaStatusSnapshot, RuntimeQuotaTarget, RuntimeQuotaTargetError,
};
pub use token_count::{
    CountTokensExecution, CountTokensExecutor, CountTokensFuture, ProviderCountTokensExecutor,
    UnsupportedCountTokensExecutor,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-router";

/// A boxed, sendable route-execution operation without coupling this facade to an async-trait
/// macro or a Provider type.
pub type ResponsesFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Client-visible representation requested from the `OpenAI` Responses route.
///
/// This belongs to the Router-owned execution seam rather than a protocol adapter so an executor
/// can choose the already-approved upstream request shape without importing HTTP or wire types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponsesResponseMode {
    /// The caller expects one completed JSON response.
    NonStreaming,
    /// The caller expects a server-sent event response.
    Streaming,
}

/// Downstream transport selected by the public Responses ingress.
///
/// This is intentionally independent of the upstream Endpoint transport. A WebSocket client may
/// consume the same bounded Canonical lifecycle while the selected Provider remains HTTP/SSE.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponsesClientTransport {
    /// Ordinary HTTP response or server-sent events.
    Http,
    /// Persistent `OpenAI` Responses WebSocket mode.
    WebSocket,
}

/// One fully admitted request handed from the HTTP boundary to a Responses executor.
///
/// The request keeps its original client model reference for observability and provider encoding,
/// while `route_id` is the exact Snapshot-approved Route selected by the HTTP boundary when
/// Snapshot authentication is active. The retry gate is created by the bounded downstream
/// transport before execution starts, so pre-first-semantic-event retry and cancellation share one
/// request lifetime without making the Router depend on `gateway-stream`.
#[derive(Clone)]
pub struct ResponsesExecution {
    context: RequestContext,
    request: CanonicalRequest,
    client_protocol: ProtocolFormat,
    native_payload: Option<Arc<[u8]>>,
    route_id: Option<RouteId>,
    mode: ResponsesResponseMode,
    client_transport: ResponsesClientTransport,
    retry_gate: Arc<dyn TransparentRetryGate>,
    lineage_recorder: Option<Arc<ResponsesExecutionLineageRecorder>>,
    continuation_pin: Option<ResponsesContinuationPin>,
}

impl ResponsesExecution {
    /// Creates one execution handoff after HTTP authentication, decoding, and model resolution.
    #[must_use]
    pub fn new(
        context: RequestContext,
        request: CanonicalRequest,
        route_id: Option<RouteId>,
        mode: ResponsesResponseMode,
        retry_gate: Arc<dyn TransparentRetryGate>,
    ) -> Self {
        Self {
            context,
            request,
            client_protocol: ProtocolFormat::OpenAiResponses,
            native_payload: None,
            route_id,
            mode,
            client_transport: ResponsesClientTransport::Http,
            retry_gate,
            lineage_recorder: None,
            continuation_pin: None,
        }
    }

    /// Creates an execution with the ingress-decided protocol and its strictly decoded payload.
    ///
    /// The payload is available only to a same-protocol native adapter. Cross-protocol candidates
    /// must use the Canonical request and the lossless transform admission matrix.
    #[must_use]
    pub fn new_for_protocol(
        context: RequestContext,
        request: CanonicalRequest,
        client_protocol: ProtocolFormat,
        native_payload: Arc<[u8]>,
        route_id: Option<RouteId>,
        mode: ResponsesResponseMode,
        retry_gate: Arc<dyn TransparentRetryGate>,
    ) -> Self {
        Self {
            context,
            request,
            client_protocol,
            native_payload: Some(native_payload),
            route_id,
            mode,
            client_transport: ResponsesClientTransport::Http,
            retry_gate,
            lineage_recorder: None,
            continuation_pin: None,
        }
    }

    /// Returns the correlation context allocated by the ingress boundary.
    #[must_use]
    pub fn context(&self) -> &RequestContext {
        &self.context
    }

    /// Returns the canonical request without exposing any HTTP transport type.
    #[must_use]
    pub fn request(&self) -> &CanonicalRequest {
        &self.request
    }

    /// Returns the trusted protocol selected by the ingress decoder.
    #[must_use]
    pub const fn client_protocol(&self) -> ProtocolFormat {
        self.client_protocol
    }

    /// Returns the strictly decoded native body retained for same-protocol forwarding.
    #[must_use]
    pub fn native_payload(&self) -> Option<&Arc<[u8]>> {
        self.native_payload.as_ref()
    }

    /// Returns the Snapshot-approved Route when Snapshot authentication selected one.
    #[must_use]
    pub fn route_id(&self) -> Option<&RouteId> {
        self.route_id.as_ref()
    }

    /// Returns the requested public response representation.
    #[must_use]
    pub const fn mode(&self) -> ResponsesResponseMode {
        self.mode
    }

    /// Selects the downstream Responses transport without changing the upstream protocol.
    #[must_use]
    pub const fn with_client_transport(
        mut self,
        client_transport: ResponsesClientTransport,
    ) -> Self {
        self.client_transport = client_transport;
        self
    }

    /// Returns the public Responses transport selected at ingress.
    #[must_use]
    pub const fn client_transport(&self) -> ResponsesClientTransport {
        self.client_transport
    }

    /// Returns the request's downstream-owned retry and cancellation gate.
    #[must_use]
    pub fn retry_gate(&self) -> &Arc<dyn TransparentRetryGate> {
        &self.retry_gate
    }

    /// Attaches the request-local successful-attempt lineage recorder used by opt-in storage.
    #[must_use]
    pub fn with_lineage_recorder(
        mut self,
        recorder: Arc<ResponsesExecutionLineageRecorder>,
    ) -> Self {
        self.lineage_recorder = Some(recorder);
        self
    }

    /// Returns the optional request-local successful-attempt lineage recorder.
    #[must_use]
    pub fn lineage_recorder(&self) -> Option<&Arc<ResponsesExecutionLineageRecorder>> {
        self.lineage_recorder.as_ref()
    }

    /// Attaches exact Client-Key-owned stored lineage for a no-fallback execution.
    #[must_use]
    pub fn with_continuation_pin(mut self, pin: ResponsesContinuationPin) -> Self {
        self.native_payload = None;
        self.continuation_pin = Some(pin);
        self
    }

    /// Returns the optional exact stored-history execution pin.
    #[must_use]
    pub const fn continuation_pin(&self) -> Option<&ResponsesContinuationPin> {
        self.continuation_pin.as_ref()
    }

    fn into_legacy_parts(self) -> (RequestContext, CanonicalRequest) {
        (self.context, self.request)
    }
}

impl fmt::Debug for ResponsesExecution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponsesExecution")
            .field("context", &self.context)
            .field("request", &self.request)
            .field("client_protocol", &self.client_protocol)
            .field(
                "native_payload_len",
                &self.native_payload.as_ref().map(|payload| payload.len()),
            )
            .field("route_id", &self.route_id)
            .field("mode", &self.mode)
            .field("client_transport", &self.client_transport)
            .field("retry_gate", &"<downstream-owned>")
            .field("lineage_recorder", &self.lineage_recorder.is_some())
            .field("continuation_pin", &self.continuation_pin.is_some())
            .finish()
    }
}

/// Starts one selected Responses execution without exposing a Provider boundary to transport
/// crates.
///
/// P1 deliberately has no catalog lookup, retry policy, credential selection, or route snapshot.
/// A later router implementation can add those internals while preserving this core-only surface.
pub trait ResponsesExecutor: Send + Sync {
    /// Returns whether this executor can publish exact successful-attempt lineage for local store.
    #[must_use]
    fn supports_stored_response_lineage(&self) -> bool {
        false
    }

    /// Returns whether this executor enforces exact stored-history lineage without fallback.
    #[must_use]
    fn supports_stored_response_continuity(&self) -> bool {
        false
    }

    /// Starts one execution and returns its pull-only canonical event source.
    fn execute(
        &self,
        context: RequestContext,
        request: CanonicalRequest,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>>;

    /// Starts an execution after the HTTP boundary has supplied route, response-mode, and
    /// downstream retry context.
    ///
    /// The default retains the P1 execution surface for legacy embeddings. P3 aggregation
    /// executors override it to consume the Snapshot-approved Route and shared retry gate.
    fn execute_routed(
        &self,
        execution: ResponsesExecution,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        let (context, request) = execution.into_legacy_parts();
        self.execute(context, request)
    }
}

/// Pull-only canonical output available to an HTTP or other downstream transport.
///
/// It is intentionally distinct from `gateway_provider::CanonicalEventSource`: transports see
/// only router-owned canonical types, never Provider traits or concrete Provider implementations.
pub trait ResponsesEventSource: Send {
    /// Returns the next canonical event or normal end-of-source.
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>>;
}

/// One deterministic P1 mock event scheduled relative to the preceding source pull.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeterministicMockEmission {
    after: Duration,
    event: CanonicalEvent,
}

impl DeterministicMockEmission {
    /// Creates one scheduled event for [`DeterministicMockResponsesExecutor`].
    #[must_use]
    pub const fn new(after: Duration, event: CanonicalEvent) -> Self {
        Self { after, event }
    }

    /// Returns the delay before this event becomes available.
    #[must_use]
    pub const fn after(&self) -> Duration {
        self.after
    }

    /// Returns the canonical event retained by this fixture entry.
    #[must_use]
    pub const fn event(&self) -> &CanonicalEvent {
        &self.event
    }
}

/// P1's router-facing deterministic executor backed internally by the P1-06 Mock Provider.
///
/// Its public constructor accepts only canonical data, so `gateway-http-actix` can use the P1
/// vertical slice without a direct dependency on `gateway-provider` or leaked Provider types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeterministicMockResponsesExecutor {
    provider: DeterministicMockProvider,
}

/// Router-facing bridge for two real Provider execution modes.
///
/// The ingress protocol decides whether the downstream response is streaming.  This bridge
/// selects the matching already-configured Provider adapter while preserving the Router rule that
/// HTTP crates never import a concrete Provider type.  The two adapters may share credentials and
/// a transport, but their wire modes are explicit and cannot be silently substituted.
#[derive(Clone)]
pub struct RoutedProviderResponsesExecutor {
    non_streaming: Arc<dyn InferenceAdapter>,
    streaming: Arc<dyn InferenceAdapter>,
}

impl RoutedProviderResponsesExecutor {
    /// Creates an executor from explicit non-streaming and streaming Provider adapters.
    #[must_use]
    pub fn new(
        non_streaming: Arc<dyn InferenceAdapter>,
        streaming: Arc<dyn InferenceAdapter>,
    ) -> Self {
        Self {
            non_streaming,
            streaming,
        }
    }

    fn provider_for_mode(&self, mode: ResponsesResponseMode) -> Arc<dyn InferenceAdapter> {
        match mode {
            ResponsesResponseMode::NonStreaming => Arc::clone(&self.non_streaming),
            ResponsesResponseMode::Streaming => Arc::clone(&self.streaming),
        }
    }
}

impl fmt::Debug for RoutedProviderResponsesExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoutedProviderResponsesExecutor")
            .field("non_streaming", &"<injected>")
            .field("streaming", &"<injected>")
            .finish()
    }
}

impl ResponsesExecutor for RoutedProviderResponsesExecutor {
    fn execute(
        &self,
        context: RequestContext,
        request: CanonicalRequest,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        let provider = Arc::clone(&self.non_streaming);
        Box::pin(async move {
            let source = provider.execute(context, request).await?;
            Ok(Box::new(ProviderResponsesEventSource { source }) as Box<dyn ResponsesEventSource>)
        })
    }

    fn execute_routed(
        &self,
        execution: ResponsesExecution,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        let provider = self.provider_for_mode(execution.mode());
        let (context, request) = execution.into_legacy_parts();
        Box::pin(async move {
            let source = provider.execute(context, request).await?;
            Ok(Box::new(ProviderResponsesEventSource { source }) as Box<dyn ResponsesEventSource>)
        })
    }
}

impl DeterministicMockResponsesExecutor {
    /// Validates a canonical mock script and creates a reusable P1 executor.
    ///
    /// # Errors
    ///
    /// Returns the existing canonical stream lifecycle error when `emissions` is malformed or
    /// incomplete.
    pub fn try_new(
        provider_id: ProviderId,
        emissions: Vec<DeterministicMockEmission>,
    ) -> Result<Self, GatewayError> {
        let emissions = emissions
            .into_iter()
            .map(|emission| MockEmission::new(emission.after, emission.event))
            .collect();
        let fixture = MockFixture::try_events(emissions)?;

        Ok(Self {
            provider: DeterministicMockProvider::new(provider_id, fixture),
        })
    }
}

impl ResponsesExecutor for DeterministicMockResponsesExecutor {
    fn execute(
        &self,
        context: RequestContext,
        request: CanonicalRequest,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        let provider = self.provider.clone();

        Box::pin(async move {
            let source = provider.execute(context, request).await?;
            Ok(Box::new(ProviderResponsesEventSource { source }) as Box<dyn ResponsesEventSource>)
        })
    }
}

/// Private adapter that keeps the Provider source behind the router facade.
struct ProviderResponsesEventSource {
    source: Box<dyn CanonicalEventSource>,
}

impl ResponsesEventSource for ProviderResponsesEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        let future: ProviderFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> =
            self.source.next_event();
        future
    }
}
