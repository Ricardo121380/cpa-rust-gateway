//! P12's explicitly bounded production data-plane composition.
//!
//! The deployment process is deliberately narrower than the test-only P3 harness: it admits one
//! configured OpenAI-compatible Responses endpoint at a time, pins its encrypted Credential pool
//! to the active Snapshot, and fails closed after a management publication until the isolated
//! Staging process restarts.  That prevents a new `RouteSnapshot` from using an old runtime
//! pool.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    fmt,
    num::NonZeroUsize,
    path::Path,
    sync::Arc,
    time::Duration,
};

use gateway_auth::client_key::ClientKeyService;
use gateway_catalog::{
    CapabilitySet, CatalogView, EndpointCapabilityEntry, EndpointCapabilityView,
};
use gateway_control::{
    credential_pool_compiler::CredentialPoolCompiler, egress_policy_compiler::EgressPolicyCompiler,
    route_compiler::RouteCompiler,
};
use gateway_core::{
    CanonicalEvent, CanonicalRequest, CanonicalResponse, EndpointId, ErrorScope, GatewayError,
    GatewayErrorCode, GatewayEventSink, MessageEnd, MessageRole, MessageStart,
    NoopGatewayEventSink, RawExtensions, RawJson, RequestContext, ResponseEnd, ResponseId,
    ResponseStart, StreamError, TextDelta, ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart,
    Usage, UsageDelta,
};
use gateway_http_actix::{
    ResponsesHttpState, default_stream_capacity,
    management_resources::{
        ManagementCatalogStatus, ManagementQuotaRecoveryState, ManagementRequestAttempt,
        ManagementRouteExplain, ManagementRouteExplainCandidate, ManagementRouteExplainRequest,
        ManagementRuntimeAvailabilityStatus, ManagementRuntimeError, ManagementRuntimeFacade,
        ManagementRuntimeTarget,
    },
};
use gateway_router::{
    AttemptDriver, AttemptFailure, AttemptFuture, AttemptOrchestrator, ResponsesEventSource,
    ResponsesExecution, ResponsesExecutor, ResponsesFuture, ResponsesResponseMode,
    RouteCredentialScheduler, RouteSnapshot, RouteSnapshotRegistry, SelectedRouteCredential,
    SnapshotRouteCandidate, SnapshotVersion,
};
use gateway_store::{
    control_plane::{
        ConfigVersionStatus, ControlPlaneConfiguration, CredentialScope, CredentialStatus,
        EndpointConfiguration, EndpointTransport, RoutePolicy, SqliteControlPlaneRepository,
        StoredClientKeyStatus, StoredEgressRedirectMode, TransformMode,
    },
    secret_store::SecretStore,
};
use gateway_upstream::{
    AdmittedEgressTarget, CredentialLease, EgressDnsResolver, EgressPolicy,
    SystemEgressDnsResolver, UpstreamClientPool, UpstreamHttpMethod, UpstreamHttpRequest,
    UpstreamHttpResponse, UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};
use protocol_openai_responses::ResponseMode;
use provider_openai_compatible::{
    OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesOutboundRequest,
    OpenAiResponsesRequestBuilder,
};
use serde_json::Value;

/// The only Endpoint identity admitted by P12's temporary isolated runtime.
pub(crate) const P12_STAGING_ENDPOINT_ID: &str = "p12-krill-endpoint";

const MAX_UPSTREAM_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_SSE_FRAME_BYTES: usize = 64 * 1024;
const P12_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const P12_TTFB_TIMEOUT: Duration = Duration::from_secs(15);
const P12_IDLE_TIMEOUT: Duration = Duration::from_secs(45);
const P12_TOTAL_TIMEOUT: Duration = Duration::from_secs(45);
const P12_BOOTSTRAP_TIMEOUT_MILLISECONDS: i64 = 15_000;
const P12_ANTHROPIC_MAX_TOKENS_EXTENSION: &str = "anthropic.messages.max_tokens";
const P12_OPENAI_MAX_OUTPUT_TOKENS_EXTENSION: &str = "openai.responses.max_output_tokens";
/// The one verified, non-secret Krill/Codex compatibility header for P12's isolated endpoint.
///
/// This stays in the P12 runtime instead of changing the generic OpenAI-compatible provider:
/// other Providers retain their existing three-header contract.
const P12_KRILL_COMPATIBILITY_USER_AGENT: &str = "codex_cli_rs/0.139.0";

/// Production pieces that must be attached to the separate P12 listeners together.
pub(crate) struct DataPlaneComposition {
    /// Authenticated data-plane state for the loopback data listener.
    pub(crate) data: ResponsesHttpState,
    /// Value-free management projection backed only by the immutable Snapshot registry.
    pub(crate) management_runtime: Box<dyn ManagementRuntimeFacade>,
}

/// Returns the fixed compiler evidence for P12's one temporary endpoint identity.
///
/// The capability set is intentionally empty: before the controlled Tool request proves a
/// capability, the Staging graph cannot advertise that capability.  Its Candidate must use the
/// existing explicit `allow_unlisted_model` admission because P12 intentionally performs no broad
/// catalog import.
pub(crate) fn staging_route_compiler() -> Result<RouteCompiler, RuntimeCompositionError> {
    let endpoint_id = EndpointId::try_new(P12_STAGING_ENDPOINT_ID.to_owned())
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
    let capabilities = EndpointCapabilityView::try_new([EndpointCapabilityEntry {
        endpoint_id,
        capabilities: CapabilitySet::empty(),
    }])
    .map_err(|_| RuntimeCompositionError::Unavailable)?;
    Ok(RouteCompiler::new(CatalogView::default(), capabilities))
}

/// Builds the request-time state from exactly the active isolated control-plane configuration.
///
/// An empty Staging database deliberately starts with an authenticated but unsendable data plane.
/// Once management publishes the temporary graph, systemd must restart this isolated process so a
/// new encrypted Credential pool and exact Snapshot are built atomically at process bootstrap.
pub(crate) fn build_data_plane_composition(
    database: &Path,
    secret_store: &SecretStore,
    registry: Arc<RouteSnapshotRegistry>,
    client_key_service: ClientKeyService,
) -> Result<DataPlaneComposition, RuntimeCompositionError> {
    let mut repository = SqliteControlPlaneRepository::open(database)
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
    let executor: Arc<dyn ResponsesExecutor> = match repository
        .load_active_configuration()
        .map_err(|_| RuntimeCompositionError::Unavailable)?
    {
        Some(configuration) => Arc::new(P12OpenAiResponsesExecutor::try_new(
            &configuration,
            secret_store,
            Arc::clone(&registry),
        )?),
        None => Arc::new(NoActiveConfigurationExecutor),
    };
    let authenticator = Arc::new(gateway_router::SnapshotClientKeyAuthenticator::new(
        Arc::clone(&registry),
        client_key_service,
    ));
    let data = ResponsesHttpState::new_with_snapshot_authentication(
        executor,
        authenticator,
        default_stream_capacity().map_err(|_| RuntimeCompositionError::Unavailable)?,
    );

    Ok(DataPlaneComposition {
        data,
        management_runtime: Box::new(SnapshotManagementRuntimeFacade { registry }),
    })
}

/// Safe, target-free runtime-composition failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeCompositionError {
    /// A control-plane, Snapshot, encrypted Credential, or bounded transport invariant failed.
    Unavailable,
}

impl fmt::Display for RuntimeCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("P12 Staging runtime is unavailable")
    }
}

impl Error for RuntimeCompositionError {}

struct NoActiveConfigurationExecutor;

impl ResponsesExecutor for NoActiveConfigurationExecutor {
    fn execute(
        &self,
        _context: RequestContext,
        _request: CanonicalRequest,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        Box::pin(async { Err(route_not_found_error()) })
    }
}

struct P12OpenAiResponsesExecutor {
    registry: Arc<RouteSnapshotRegistry>,
    snapshot_version: SnapshotVersion,
    orchestrator: Arc<AttemptOrchestrator>,
    endpoints: Arc<BTreeMap<EndpointId, EndpointRuntime>>,
    client_pool: Arc<UpstreamClientPool>,
    event_sink: Arc<dyn GatewayEventSink>,
}

impl P12OpenAiResponsesExecutor {
    fn try_new(
        configuration: &ControlPlaneConfiguration,
        secret_store: &SecretStore,
        registry: Arc<RouteSnapshotRegistry>,
    ) -> Result<Self, RuntimeCompositionError> {
        validate_p12_configuration_shape(configuration)?;
        let snapshot = registry.load();
        if snapshot.version().as_str() != configuration.version.id.as_str() {
            return Err(RuntimeCompositionError::Unavailable);
        }
        let policies = EgressPolicyCompiler::compile(configuration)
            .map_err(|_| RuntimeCompositionError::Unavailable)?;
        let pools = CredentialPoolCompiler::new(secret_store)
            .compile(configuration)
            .map_err(|_| RuntimeCompositionError::Unavailable)?;
        let endpoints = endpoint_runtimes(configuration, &snapshot, &policies)?;
        let scheduler = Arc::new(RouteCredentialScheduler::new(
            Arc::clone(&snapshot),
            Arc::new(pools),
        ));
        let orchestrator = Arc::new(AttemptOrchestrator::new(
            scheduler,
            Arc::new(gateway_router::RuntimeHealthRegistry::new()),
        ));
        let client_pool = Arc::new(UpstreamClientPool::new(
            NonZeroUsize::new(4).ok_or(RuntimeCompositionError::Unavailable)?,
        ));

        Ok(Self {
            registry,
            snapshot_version: snapshot.version().clone(),
            orchestrator,
            endpoints: Arc::new(endpoints),
            client_pool,
            event_sink: Arc::new(NoopGatewayEventSink),
        })
    }

    fn snapshot_is_current(&self) -> bool {
        self.registry.load().version() == &self.snapshot_version
    }
}

impl ResponsesExecutor for P12OpenAiResponsesExecutor {
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
        if !self.snapshot_is_current() {
            return Box::pin(async { Err(stale_runtime_error()) });
        }
        let orchestrator = Arc::clone(&self.orchestrator);
        let endpoints = Arc::clone(&self.endpoints);
        let client_pool = Arc::clone(&self.client_pool);
        let event_sink = Arc::clone(&self.event_sink);
        let context = execution.context().clone();
        let request = execution.request().clone();
        let route_id = execution.route_id().cloned();
        let mode = execution.mode();
        let retry_gate = Arc::clone(execution.retry_gate());

        Box::pin(async move {
            let route_id = route_id.ok_or_else(route_not_found_error)?;
            let driver = OpenAiAttemptDriver {
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

fn endpoint_runtimes(
    configuration: &ControlPlaneConfiguration,
    snapshot: &RouteSnapshot,
    policies: &gateway_control::egress_policy_compiler::CompiledEgressPolicies,
) -> Result<BTreeMap<EndpointId, EndpointRuntime>, RuntimeCompositionError> {
    if configuration.endpoints.len() != 1 {
        return Err(RuntimeCompositionError::Unavailable);
    }
    let expected_endpoint = EndpointId::try_new(P12_STAGING_ENDPOINT_ID.to_owned())
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
    let configured = configuration
        .endpoints
        .first()
        .filter(|endpoint| endpoint.id == expected_endpoint)
        .ok_or(RuntimeCompositionError::Unavailable)?;
    validate_endpoint_shape(configured)?;
    let candidate_endpoint_ids = snapshot
        .routes()
        .flat_map(gateway_router::SnapshotRoute::candidates)
        .map(|candidate| candidate.endpoint_id().clone())
        .collect::<BTreeSet<_>>();
    if candidate_endpoint_ids != BTreeSet::from([expected_endpoint.clone()]) {
        return Err(RuntimeCompositionError::Unavailable);
    }
    let policy = policies
        .policy_for_upstream(&configured.upstream_id)
        .cloned()
        .ok_or(RuntimeCompositionError::Unavailable)?;
    let endpoint =
        OpenAiResponsesEndpoint::try_new(&configured.base_url, &configured.inference_path)
            .map_err(|_| RuntimeCompositionError::Unavailable)?;
    let transport = UpstreamTransportProfile::new(
        UpstreamTimeouts::try_new(
            P12_CONNECT_TIMEOUT,
            P12_TTFB_TIMEOUT,
            P12_IDLE_TIMEOUT,
            P12_TOTAL_TIMEOUT,
        )
        .map_err(|_| RuntimeCompositionError::Unavailable)?,
        UpstreamProxy::Direct,
        NonZeroUsize::new(1).ok_or(RuntimeCompositionError::Unavailable)?,
    );
    Ok(BTreeMap::from([(
        expected_endpoint,
        EndpointRuntime {
            endpoint,
            policy,
            resolver: Arc::new(SystemEgressDnsResolver),
            transport,
        },
    )]))
}

/// Narrows P12-05 to the one reviewed temporary graph before a Secret can be opened or an
/// outbound request can be constructed.  General multi-route/provider composition remains a
/// later deployment concern.
fn validate_p12_configuration_shape(
    configuration: &ControlPlaneConfiguration,
) -> Result<(), RuntimeCompositionError> {
    if configuration.version.status != ConfigVersionStatus::Active
        || configuration.egress_policies.len() != 1
        || configuration.upstreams.len() != 1
        || configuration.endpoints.len() != 1
        || configuration.credentials.len() != 1
        || configuration.endpoint_credential_bindings.len() != 1
        || configuration.public_models.len() != 1
        || !configuration.model_aliases.is_empty()
        || configuration.model_routes.len() != 1
        || configuration.route_candidates.len() != 1
        || configuration.access_groups.len() != 1
        || configuration.access_group_routes.len() != 1
        || configuration.client_keys.len() != 1
    {
        return Err(RuntimeCompositionError::Unavailable);
    }

    let endpoint = configuration
        .endpoints
        .first()
        .ok_or(RuntimeCompositionError::Unavailable)?;
    validate_endpoint_shape(endpoint)?;
    let upstream = configuration
        .upstreams
        .first()
        .filter(|upstream| upstream.enabled && upstream.id == endpoint.upstream_id)
        .ok_or(RuntimeCompositionError::Unavailable)?;
    let policy = configuration
        .egress_policies
        .first()
        .filter(|policy| upstream.egress_policy_id.as_ref() == Some(&policy.id))
        .ok_or(RuntimeCompositionError::Unavailable)?;
    if !has_exact_p12_egress_shape(policy) {
        return Err(RuntimeCompositionError::Unavailable);
    }

    validate_p12_credential_binding(configuration, endpoint, upstream)?;
    validate_p12_route_access_shape(configuration, endpoint)
}

fn validate_p12_credential_binding(
    configuration: &ControlPlaneConfiguration,
    endpoint: &EndpointConfiguration,
    upstream: &gateway_store::control_plane::UpstreamConfiguration,
) -> Result<(), RuntimeCompositionError> {
    let credential = configuration
        .credentials
        .first()
        .filter(|credential| {
            credential.upstream_id == upstream.id
                && credential.kind == "bearer"
                && credential.status == CredentialStatus::Active
        })
        .ok_or(RuntimeCompositionError::Unavailable)?;
    configuration
        .endpoint_credential_bindings
        .first()
        .filter(|binding| {
            binding.endpoint_id == endpoint.id
                && binding.credential_id == credential.id
                && binding.upstream_id == upstream.id
                && binding.enabled
                && binding.priority == 0
                && binding.weight == 1
                && binding.concurrency == 1
        })
        .ok_or(RuntimeCompositionError::Unavailable)?;
    Ok(())
}

fn validate_p12_route_access_shape(
    configuration: &ControlPlaneConfiguration,
    endpoint: &EndpointConfiguration,
) -> Result<(), RuntimeCompositionError> {
    let public_model = configuration
        .public_models
        .first()
        .filter(|model| {
            model.status == gateway_store::control_plane::AdministrativeStatus::Active
                && is_empty_capability_object(&model.capabilities_json)
        })
        .ok_or(RuntimeCompositionError::Unavailable)?;
    let route = configuration
        .model_routes
        .first()
        .filter(|route| {
            route.public_model_id == public_model.id
                && route.policy == RoutePolicy::SmoothWeightedRoundRobin
                && route.max_attempts == 1
                && route.bootstrap_timeout_ms > 0
                && route.bootstrap_timeout_ms <= P12_BOOTSTRAP_TIMEOUT_MILLISECONDS
        })
        .ok_or(RuntimeCompositionError::Unavailable)?;
    configuration
        .route_candidates
        .first()
        .filter(|candidate| {
            candidate.route_id == route.id
                && candidate.endpoint_id == endpoint.id
                && candidate.credential_scope == CredentialScope::EndpointBindings
                && candidate.transform_mode == TransformMode::Canonical
                && candidate.enabled
                && candidate.priority == 0
                && candidate.weight == 1
                && has_p12_unlisted_model_override(&candidate.capability_override_json)
        })
        .ok_or(RuntimeCompositionError::Unavailable)?;
    let access_group = configuration
        .access_groups
        .first()
        .filter(|group| {
            group.status == gateway_store::control_plane::AdministrativeStatus::Active
                && is_empty_capability_object(&group.limits_json)
        })
        .ok_or(RuntimeCompositionError::Unavailable)?;
    if configuration
        .access_group_routes
        .first()
        .is_none_or(|binding| {
            binding.access_group_id != access_group.id
                || binding.route_id != route.id
                || !binding.enabled
        })
        || configuration.client_keys.first().is_none_or(|key| {
            key.access_group_id() != &access_group.id
                || key.status() != StoredClientKeyStatus::Active
        })
    {
        return Err(RuntimeCompositionError::Unavailable);
    }
    Ok(())
}

fn has_exact_p12_egress_shape(
    policy: &gateway_store::control_plane::EgressPolicyConfiguration,
) -> bool {
    let allowed_schemes = serde_json::from_str::<Vec<String>>(&policy.allowed_schemes_json);
    let allowed_hosts = serde_json::from_str::<Vec<String>>(&policy.allowed_hosts_json);
    let allowed_ports = serde_json::from_str::<Vec<u16>>(&policy.allowed_ports_json);
    let allowed_cidrs = serde_json::from_str::<Vec<String>>(&policy.allowed_cidrs_json);
    matches!(allowed_schemes, Ok(schemes) if schemes.as_slice() == ["https"])
        && matches!(allowed_hosts, Ok(hosts) if hosts.len() == 1)
        && matches!(allowed_ports, Ok(ports) if ports.len() == 1)
        && matches!(allowed_cidrs, Ok(cidrs) if cidrs.is_empty())
        && policy.redirect_mode == StoredEgressRedirectMode::Deny
        && policy.max_redirects == 0
}

fn is_empty_capability_object(value: &str) -> bool {
    matches!(serde_json::from_str::<Value>(value), Ok(Value::Object(object)) if object.is_empty())
}

fn has_p12_unlisted_model_override(value: &str) -> bool {
    matches!(
        serde_json::from_str::<Value>(value),
        Ok(Value::Object(object))
            if object.len() == 1
                && object.get("allow_unlisted_model") == Some(&Value::Bool(true))
    )
}

fn validate_endpoint_shape(
    endpoint: &EndpointConfiguration,
) -> Result<(), RuntimeCompositionError> {
    if !endpoint.enabled
        || endpoint.adapter_id != "openai-compatible.responses"
        || endpoint.api_format != "openai/responses"
        || endpoint.transport != EndpointTransport::Http
    {
        return Err(RuntimeCompositionError::Unavailable);
    }
    Ok(())
}

struct EndpointRuntime {
    endpoint: OpenAiResponsesEndpoint,
    policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    transport: UpstreamTransportProfile,
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

struct OpenAiAttemptDriver {
    request: CanonicalRequest,
    mode: ResponsesResponseMode,
    endpoints: Arc<BTreeMap<EndpointId, EndpointRuntime>>,
    client_pool: Arc<UpstreamClientPool>,
}

impl AttemptDriver for OpenAiAttemptDriver {
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
            let request = p12_openai_compatible_request(&self.request)
                .map_err(AttemptFailure::NonRetryable)?;
            let credential = std::str::from_utf8(credential.secret_bytes())
                .map_err(|_| AttemptFailure::NonRetryable(internal_error()))?;
            let request_credential = OpenAiResponsesApiKey::try_new(credential.to_owned())
                .map_err(AttemptFailure::NonRetryable)?;
            let outbound = OpenAiResponsesRequestBuilder::build(
                &runtime.endpoint,
                &request_credential,
                candidate.upstream_model(),
                &request,
                upstream_response_mode(self.mode),
            )
            .map_err(AttemptFailure::NonRetryable)?;
            let admitted = runtime
                .policy
                .admit_url(outbound.url(), runtime.resolver.as_ref())
                .map_err(|_| AttemptFailure::NonRetryable(egress_rejected_error()))?;
            let request =
                p12_transport_request(&outbound, admitted).map_err(AttemptFailure::NonRetryable)?;
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

fn p12_transport_request(
    outbound: &OpenAiResponsesOutboundRequest,
    admitted: AdmittedEgressTarget,
) -> Result<UpstreamHttpRequest, GatewayError> {
    if admitted.request_url() != outbound.target().as_url() {
        return Err(egress_rejected_error());
    }

    let accept = outbound
        .header("accept")
        .ok_or_else(internal_error)?
        .to_owned();
    let authorization = outbound
        .header("authorization")
        .ok_or_else(internal_error)?
        .to_owned();
    let content_type = outbound
        .header("content-type")
        .ok_or_else(internal_error)?
        .to_owned();

    UpstreamHttpRequest::try_new(
        admitted,
        UpstreamHttpMethod::Post,
        p12_transport_headers(&accept, &authorization, &content_type),
        outbound.body().to_vec(),
    )
    .map_err(|_| internal_error())
}

fn p12_transport_headers(
    accept: &str,
    authorization: &str,
    content_type: &str,
) -> [(String, String); 4] {
    [
        ("accept".to_owned(), accept.to_owned()),
        ("authorization".to_owned(), authorization.to_owned()),
        ("content-type".to_owned(), content_type.to_owned()),
        (
            "user-agent".to_owned(),
            P12_KRILL_COMPATIBILITY_USER_AGENT.to_owned(),
        ),
    ]
}

/// Translates the one P12-admitted Anthropic output limit before generic Responses encoding.
///
/// Anthropic Messages requires `max_tokens`, while the isolated P12 upstream accepts the
/// `OpenAI` Responses `max_output_tokens` spelling. The pure Anthropic decoder preserves the
/// source field as a namespaced extension because the Canonical core has no shared output-limit
/// field. This boundary consumes only that positive-integer extension and deliberately leaves
/// every other foreign extension for the generic provider to reject.
fn p12_openai_compatible_request(
    request: &CanonicalRequest,
) -> Result<CanonicalRequest, GatewayError> {
    let Some(max_tokens) = request.extensions.get(P12_ANTHROPIC_MAX_TOKENS_EXTENSION) else {
        return Ok(request.clone());
    };
    if request
        .extensions
        .get(P12_OPENAI_MAX_OUTPUT_TOKENS_EXTENSION)
        .is_some()
        || !matches!(
            serde_json::from_str::<Value>(max_tokens.get()),
            Ok(Value::Number(value)) if value.as_u64().is_some_and(|value| value > 0)
        )
    {
        return Err(upstream_protocol_error());
    }

    let mut extensions = RawExtensions::default();
    for (name, value) in request.extensions.iter() {
        if name != P12_ANTHROPIC_MAX_TOKENS_EXTENSION {
            extensions
                .try_insert(name, value.clone())
                .map_err(|_| internal_error())?;
        }
    }
    extensions
        .try_insert(P12_OPENAI_MAX_OUTPUT_TOKENS_EXTENSION, max_tokens.clone())
        .map_err(|_| internal_error())?;

    let mut translated = request.clone();
    translated.extensions = extensions;
    Ok(translated)
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
    let mut events = vec![CanonicalEvent::ResponseStart(ResponseStart {
        response_id: ResponseId::try_new(response_id).map_err(|_| upstream_protocol_error())?,
        extensions: RawExtensions::default(),
    })];
    let mut message_open = false;
    let mut emitted_content = false;
    let mut call_ids = BTreeSet::new();
    for item in output {
        let kind = item
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(upstream_protocol_error)?;
        match kind {
            "message" => decode_completed_message(
                item,
                &mut events,
                &mut message_open,
                &mut emitted_content,
            )?,
            "function_call" => decode_completed_tool_call(
                item,
                &mut events,
                &mut message_open,
                &mut emitted_content,
                &mut call_ids,
            )?,
            // A Responses model may return an internal reasoning item before its visible
            // assistant message.  P12 does not expose it, but it must not turn an otherwise
            // valid visible response into a protocol failure.
            "reasoning" => {}
            _ => return Err(upstream_protocol_error()),
        }
    }
    if !emitted_content {
        return Err(upstream_protocol_error());
    }
    if let Some(usage) = decode_usage(value.get("usage"))? {
        events.push(CanonicalEvent::UsageDelta(UsageDelta {
            usage,
            is_final: true,
            extensions: RawExtensions::default(),
        }));
    }
    if message_open {
        events.push(CanonicalEvent::MessageEnd(MessageEnd::default()));
    }
    events.push(CanonicalEvent::ResponseEnd(ResponseEnd::default()));
    CanonicalResponse::try_new(events)
        .map(CanonicalResponse::into_events)
        .map_err(|_| upstream_protocol_error())
}

fn decode_completed_message(
    item: &Value,
    events: &mut Vec<CanonicalEvent>,
    message_open: &mut bool,
    emitted_content: &mut bool,
) -> Result<(), GatewayError> {
    if item.get("role").and_then(Value::as_str) != Some("assistant") {
        return Err(upstream_protocol_error());
    }
    let content = item
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(upstream_protocol_error)?;
    for part in content {
        if part.get("type").and_then(Value::as_str) != Some("output_text") {
            return Err(upstream_protocol_error());
        }
        let text = required_string(part, "text")?;
        ensure_message(events, message_open);
        events.push(CanonicalEvent::TextDelta(TextDelta {
            text,
            extensions: RawExtensions::default(),
        }));
        *emitted_content = true;
    }
    Ok(())
}

fn decode_completed_tool_call(
    item: &Value,
    events: &mut Vec<CanonicalEvent>,
    message_open: &mut bool,
    emitted_content: &mut bool,
    call_ids: &mut BTreeSet<String>,
) -> Result<(), GatewayError> {
    let call_id = required_string(item, "call_id")?;
    let name = required_string(item, "name")?;
    let arguments = required_string(item, "arguments")?;
    if !call_ids.insert(call_id.clone()) {
        return Err(upstream_protocol_error());
    }
    let arguments =
        RawJson::from_json_string(arguments.clone()).map_err(|_| upstream_protocol_error())?;
    ensure_message(events, message_open);
    events.push(CanonicalEvent::ToolCallStart(ToolCallStart {
        call_id: call_id.clone(),
        name,
        extensions: RawExtensions::default(),
    }));
    events.push(CanonicalEvent::ToolCallArgumentsDelta(
        ToolCallArgumentsDelta {
            call_id: call_id.clone(),
            delta: arguments.get().to_owned(),
            extensions: RawExtensions::default(),
        },
    ));
    events.push(CanonicalEvent::ToolCallEnd(ToolCallEnd {
        call_id,
        arguments,
        extensions: RawExtensions::default(),
    }));
    *emitted_content = true;
    Ok(())
}

fn ensure_message(events: &mut Vec<CanonicalEvent>, message_open: &mut bool) {
    if !*message_open {
        events.push(CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
            extensions: RawExtensions::default(),
        }));
        *message_open = true;
    }
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
        // SSE comments and keep-alive frames carry no event payload.  They must not alter the
        // Canonical lifecycle or consume the bounded response budget.
        if data.is_empty() {
            return Ok(());
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
                if self.lifecycle != SseLifecycle::AwaitingMessageStart {
                    return Err(upstream_protocol_error());
                }
                if is_assistant_message {
                    self.lifecycle = SseLifecycle::Streaming { saw_text: false };
                    self.pending
                        .push_back(CanonicalEvent::MessageStart(MessageStart {
                            role: MessageRole("assistant".to_owned()),
                            extensions: RawExtensions::default(),
                        }));
                } else if item.get("type").and_then(Value::as_str) != Some("reasoning") {
                    return Err(upstream_protocol_error());
                }
            }
            "response.output_text.delta" => {
                let delta = required_string(&value, "delta")?;
                let SseLifecycle::Streaming { .. } = self.lifecycle else {
                    return Err(upstream_protocol_error());
                };
                if delta.is_empty() {
                    return Ok(());
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

struct SnapshotManagementRuntimeFacade {
    registry: Arc<RouteSnapshotRegistry>,
}

impl SnapshotManagementRuntimeFacade {
    fn snapshot_for(
        &self,
        version_id: &gateway_store::control_plane::ConfigVersionId,
    ) -> Result<Arc<RouteSnapshot>, ManagementRuntimeError> {
        let snapshot = self.registry.load();
        (snapshot.version().as_str() == version_id.as_str())
            .then_some(snapshot)
            .ok_or(ManagementRuntimeError::Unavailable)
    }
}

impl ManagementRuntimeFacade for SnapshotManagementRuntimeFacade {
    fn catalog_status(
        &mut self,
        config_version_id: &gateway_store::control_plane::ConfigVersionId,
        _observed_at_ms: i64,
    ) -> Result<Vec<ManagementCatalogStatus>, ManagementRuntimeError> {
        self.snapshot_for(config_version_id).map(|_| Vec::new())
    }

    fn runtime_availability(
        &mut self,
        config_version_id: &gateway_store::control_plane::ConfigVersionId,
        _observed_at_ms: i64,
    ) -> Result<Vec<ManagementRuntimeAvailabilityStatus>, ManagementRuntimeError> {
        self.snapshot_for(config_version_id).map(|_| Vec::new())
    }

    fn request_quota_recovery(
        &mut self,
        config_version_id: &gateway_store::control_plane::ConfigVersionId,
        _target: &ManagementRuntimeTarget,
        _observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        self.snapshot_for(config_version_id)
            .map(|_| ManagementQuotaRecoveryState::Rejected)
    }

    fn explain_route(
        &mut self,
        request: &ManagementRouteExplainRequest,
    ) -> Result<ManagementRouteExplain, ManagementRuntimeError> {
        let snapshot = self.snapshot_for(request.config_version_id())?;
        let public_model = snapshot
            .resolve_public_model(request.requested_model())
            .filter(|model| model.route_id() == request.route_id())
            .ok_or(ManagementRuntimeError::Unavailable)?;
        let route = snapshot
            .route(public_model.route_id())
            .filter(|route| route.id() == request.route_id())
            .ok_or(ManagementRuntimeError::Unavailable)?;
        if route.candidates().len() != 1 || !route.candidates()[0].is_hard_eligible() {
            return Err(ManagementRuntimeError::Unavailable);
        }
        ManagementRouteExplain::try_new(
            route.id().clone(),
            vec![ManagementRouteExplainCandidate::selected(
                route.candidates()[0].id().clone(),
            )],
        )
    }

    fn list_request_attempts(
        &mut self,
        _request_id: &gateway_core::RequestId,
    ) -> Result<Vec<ManagementRequestAttempt>, ManagementRuntimeError> {
        Ok(Vec::new())
    }
}

fn route_not_found_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::RouteNotFound, ErrorScope::Model)
}

fn stale_runtime_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
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
    use std::{
        collections::BTreeSet,
        error::Error,
        fs,
        net::{IpAddr, Ipv4Addr},
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use gateway_auth::client_key::{ClientKeyPepper, ClientKeyService};
    use gateway_control::{
        control_plane_service::credential_associated_data,
        management_service::{ManagementActor, ManagementService},
    };
    use gateway_core::{
        AccessGroupId, CanonicalEvent, ClientKeyId, CredentialId, EgressPolicyId, EndpointId,
        GatewayErrorCode, PublicModelId, RouteCandidateId, RouteId, UpstreamId,
    };
    use gateway_store::{
        control_plane::{
            AccessGroupConfiguration, AccessGroupRouteConfiguration, AdministrativeStatus,
            ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialConfiguration, CredentialScope, CredentialStatus, EgressPolicyConfiguration,
            EndpointConfiguration, EndpointCredentialBindingConfiguration, EndpointTransport,
            ModelRouteConfiguration, PublicModelConfiguration, RouteCandidateConfiguration,
            RoutePolicy, SqliteControlPlaneRepository, StoredClientKey, StoredClientKeyStatus,
            StoredEgressRedirectMode, TransformMode, UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };
    use gateway_upstream::{
        EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput,
        EgressScheme, RedirectPolicy, UpstreamHttpMethod,
    };
    use protocol_openai_responses::{ResponseMode, decode_request};
    use provider_openai_compatible::{
        OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesRequestBuilder,
    };
    use serde_json::Value;

    use super::{
        MAX_SSE_FRAME_BYTES, P12_KRILL_COMPATIBILITY_USER_AGENT, P12_STAGING_ENDPOINT_ID,
        append_sse_chunk, build_data_plane_composition, decode_json_events,
        has_exact_p12_egress_shape, has_p12_unlisted_model_override, p12_openai_compatible_request,
        p12_transport_headers, p12_transport_request, staging_route_compiler,
    };

    struct TemporaryDirectory(PathBuf);

    struct StaticPublicResolver;

    impl EgressDnsResolver for StaticPublicResolver {
        fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
            Ok(vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
        }
    }

    fn p12_transport_test_policy() -> Result<EgressPolicy, Box<dyn Error>> {
        Ok(EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("p12-transport-test-policy")?,
            name: "P12 transport test policy".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Https]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("gateway.example.test")?]),
            allowed_ports: BTreeSet::from([443]),
            allowed_cidrs: BTreeSet::new(),
            redirect_policy: RedirectPolicy::Deny,
        })?)
    }

    static TEMPORARY_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    impl TemporaryDirectory {
        fn new() -> Result<Self, Box<dyn Error>> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let sequence = TEMPORARY_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "cpa-rust-gateway-p12-runtime-{suffix}-{}-{sequence}",
                std::process::id(),
            ));
            fs::create_dir(&path)?;
            Ok(Self(path))
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn staging_compiler_has_only_the_fixed_endpoint_capability_profile()
    -> Result<(), Box<dyn Error>> {
        let compiler = staging_route_compiler()?;
        assert!(format!("{compiler:?}").contains("RouteCompiler"));
        assert_eq!(P12_STAGING_ENDPOINT_ID, "p12-krill-endpoint");
        Ok(())
    }

    #[test]
    fn p12_transport_headers_preserve_standard_headers_and_add_only_the_verified_compatibility_header()
     {
        let headers = p12_transport_headers(
            "application/json",
            "Bearer fixture-credential",
            "application/json",
        );

        assert_eq!(
            headers,
            [
                ("accept".to_owned(), "application/json".to_owned()),
                (
                    "authorization".to_owned(),
                    "Bearer fixture-credential".to_owned(),
                ),
                ("content-type".to_owned(), "application/json".to_owned()),
                (
                    "user-agent".to_owned(),
                    P12_KRILL_COMPATIBILITY_USER_AGENT.to_owned(),
                ),
            ]
        );
    }

    #[test]
    fn p12_transport_request_preserves_the_admitted_target_body_and_method()
    -> Result<(), Box<dyn Error>> {
        let decoded = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;
        let endpoint =
            OpenAiResponsesEndpoint::try_new("https://gateway.example.test/v1", "/responses")?;
        let credential = OpenAiResponsesApiKey::try_new("p12-test-bearer")?;
        let outbound = OpenAiResponsesRequestBuilder::build(
            &endpoint,
            &credential,
            "p12-test-upstream-model",
            &decoded.request,
            decoded.mode,
        )?;
        let policy = p12_transport_test_policy()?;
        let admitted = policy.admit_url(outbound.url(), &StaticPublicResolver)?;

        let request = p12_transport_request(&outbound, admitted)?;
        assert_eq!(request.method(), UpstreamHttpMethod::Post);
        assert_eq!(request.body(), outbound.body());
        assert_eq!(
            request
                .header("user-agent")
                .and_then(|value| value.to_str().ok()),
            Some(P12_KRILL_COMPATIBILITY_USER_AGENT)
        );

        let mismatched = policy.admit_url(
            "https://gateway.example.test/v1/not-responses",
            &StaticPublicResolver,
        )?;
        assert_eq!(
            p12_transport_request(&outbound, mismatched)
                .err()
                .map(|error| error.code()),
            Some(GatewayErrorCode::EgressRejected)
        );
        Ok(())
    }

    #[test]
    fn p12_maps_anthropic_canonical_max_tokens_to_bounded_openai_output_without_relaxing_foreign_extensions()
    -> Result<(), Box<dyn Error>> {
        let openai = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;
        assert_eq!(
            p12_openai_compatible_request(&openai.request)?,
            openai.request
        );

        // `protocol-anthropic`'s valid Messages fixture preserves its required `max_tokens`
        // under this exact source namespace. The binary cannot directly depend on that codec, so
        // this P12 composition test starts from the already-approved Canonical representation.
        let mut anthropic = openai.request.clone();
        anthropic.extensions.try_insert(
            "anthropic.messages.max_tokens",
            gateway_core::RawJson::from_json_string("19".to_owned())?,
        )?;
        assert_eq!(
            anthropic
                .extensions
                .get("anthropic.messages.max_tokens")
                .map(gateway_core::RawJson::get),
            Some("19")
        );

        let translated = p12_openai_compatible_request(&anthropic)?;
        assert!(
            translated
                .extensions
                .get("anthropic.messages.max_tokens")
                .is_none()
        );
        assert_eq!(
            translated
                .extensions
                .get("openai.responses.max_output_tokens")
                .map(gateway_core::RawJson::get),
            Some("19")
        );

        let endpoint =
            OpenAiResponsesEndpoint::try_new("https://gateway.example.test/v1", "/responses")?;
        let credential = OpenAiResponsesApiKey::try_new("p12-test-bearer")?;
        let outbound = OpenAiResponsesRequestBuilder::build(
            &endpoint,
            &credential,
            "p12-test-upstream-model",
            &translated,
            ResponseMode::NonStreaming,
        )?;
        let body: Value = serde_json::from_slice(outbound.body())?;
        assert_eq!(body.get("max_output_tokens"), Some(&Value::from(19)));
        assert!(body.get("max_tokens").is_none());

        let mut foreign = anthropic;
        foreign.extensions.try_insert(
            "anthropic.messages.metadata",
            gateway_core::RawJson::from_json_string(r#"{"unmapped":true}"#.to_owned())?,
        )?;
        let foreign = p12_openai_compatible_request(&foreign)?;
        assert_eq!(
            OpenAiResponsesRequestBuilder::build(
                &endpoint,
                &credential,
                "p12-test-upstream-model",
                &foreign,
                ResponseMode::NonStreaming,
            )
            .err()
            .map(|error| error.code()),
            Some(GatewayErrorCode::UpstreamProtocolError)
        );
        Ok(())
    }

    #[test]
    fn completed_non_streaming_function_call_is_a_valid_canonical_tool_lifecycle()
    -> Result<(), Box<dyn Error>> {
        let events = decode_json_events(
            br#"{
              "id":"response-p12-tool",
              "status":"completed",
              "output":[{
                "type":"function_call",
                "call_id":"call-p12-tool",
                "name":"echo",
                "arguments":"{\"value\":\"ok\"}"
              }]
            }"#,
        )?;
        assert!(events.iter().any(|event| matches!(
            event,
            CanonicalEvent::ToolCallEnd(end)
                if end.call_id == "call-p12-tool" && end.arguments.get() == r#"{"value":"ok"}"#
        )));
        Ok(())
    }

    #[test]
    fn completed_non_streaming_response_ignores_internal_reasoning() -> Result<(), Box<dyn Error>> {
        let events = decode_json_events(
            br#"{
              "id":"response-p12-reasoning",
              "status":"completed",
              "output":[
                {"type":"reasoning","summary":[]},
                {"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}
              ]
            }"#,
        )?;
        assert!(events.iter().any(|event| matches!(
            event,
            CanonicalEvent::TextDelta(delta) if delta.text == "ok"
        )));
        Ok(())
    }

    #[test]
    fn staging_egress_shape_requires_one_https_host_port_and_no_redirects()
    -> Result<(), Box<dyn Error>> {
        let mut policy = EgressPolicyConfiguration {
            id: EgressPolicyId::try_new("p12-egress")?,
            name: "p12-egress".to_owned(),
            allowed_schemes_json: r#"["https"]"#.to_owned(),
            allowed_hosts_json: r#"["gateway.example.test"]"#.to_owned(),
            allowed_ports_json: "[443]".to_owned(),
            allowed_cidrs_json: "[]".to_owned(),
            redirect_mode: StoredEgressRedirectMode::Deny,
            max_redirects: 0,
        };
        assert!(has_exact_p12_egress_shape(&policy));
        policy.allowed_cidrs_json = r#"["127.0.0.0/8"]"#.to_owned();
        assert!(!has_exact_p12_egress_shape(&policy));
        policy.allowed_cidrs_json = "[]".to_owned();
        policy.allowed_hosts_json = r#"["gateway.example.test","other.example.test"]"#.to_owned();
        assert!(!has_exact_p12_egress_shape(&policy));
        policy.allowed_hosts_json = r#"["gateway.example.test"]"#.to_owned();
        policy.redirect_mode = StoredEgressRedirectMode::SameOrigin;
        policy.max_redirects = 1;
        assert!(!has_exact_p12_egress_shape(&policy));
        assert!(has_p12_unlisted_model_override(
            r#"{"allow_unlisted_model":true}"#
        ));
        assert!(!has_p12_unlisted_model_override(
            r#"{"allow_unlisted_model":true,"tools":true}"#
        ));
        Ok(())
    }

    #[test]
    fn active_singleton_graph_builds_an_encrypted_runtime_without_a_send()
    -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let configuration = p12_configuration(&secret_store)?;
        let config_version_id = configuration.version.id.clone();
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&configuration)?;
        repository.activate_version(&config_version_id)?;
        drop(repository);

        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            staging_route_compiler()?,
            ManagementActor::try_new("p12-runtime-test")?,
        )?;
        let composition = build_data_plane_composition(
            &database,
            &secret_store,
            std::sync::Arc::clone(lifecycle.registry()),
            ClientKeyService::new(ClientKeyPepper::try_from_bytes([0xE1_u8; 32])?),
        )?;
        drop(composition);
        assert!(directory.path().join("control.sqlite3").is_file());
        Ok(())
    }

    #[test]
    fn oversized_sse_frame_is_rejected_without_buffer_growth() {
        let mut buffer = vec![b'x'; MAX_SSE_FRAME_BYTES];
        assert!(append_sse_chunk(&mut buffer, b"y").is_err());
        assert_eq!(buffer.len(), MAX_SSE_FRAME_BYTES);
    }

    struct P12RuntimeIds {
        egress_policy: EgressPolicyId,
        upstream: UpstreamId,
        endpoint: EndpointId,
        credential: CredentialId,
        public_model: PublicModelId,
        route: RouteId,
        access_group: AccessGroupId,
    }

    impl P12RuntimeIds {
        fn try_new() -> Result<Self, Box<dyn Error>> {
            Ok(Self {
                egress_policy: EgressPolicyId::try_new("p12-runtime-egress")?,
                upstream: UpstreamId::try_new("p12-runtime-upstream")?,
                endpoint: EndpointId::try_new(P12_STAGING_ENDPOINT_ID)?,
                credential: CredentialId::try_new("p12-runtime-credential")?,
                public_model: PublicModelId::try_new("p12-runtime-model")?,
                route: RouteId::try_new("p12-runtime-route")?,
                access_group: AccessGroupId::try_new("p12-runtime-group")?,
            })
        }
    }

    fn p12_configuration(
        secret_store: &SecretStore,
    ) -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
        let version = ConfigVersion {
            id: ConfigVersionId::try_new("p12-runtime-config")?,
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 0,
            description: "P12 runtime composition test".to_owned(),
        };
        let mut configuration = ControlPlaneConfiguration::new(version);
        let ids = P12RuntimeIds::try_new()?;
        add_p12_network(&mut configuration, &ids);
        add_p12_credential_and_routing(&mut configuration, &ids, secret_store)?;
        Ok(configuration)
    }

    fn add_p12_network(configuration: &mut ControlPlaneConfiguration, ids: &P12RuntimeIds) {
        configuration
            .egress_policies
            .push(EgressPolicyConfiguration {
                id: ids.egress_policy.clone(),
                name: "P12 test egress".to_owned(),
                allowed_schemes_json: r#"["https"]"#.to_owned(),
                allowed_hosts_json: r#"["gateway.example.test"]"#.to_owned(),
                allowed_ports_json: "[443]".to_owned(),
                allowed_cidrs_json: "[]".to_owned(),
                redirect_mode: StoredEgressRedirectMode::Deny,
                max_redirects: 0,
            });
        configuration.upstreams.push(UpstreamConfiguration {
            id: ids.upstream.clone(),
            name: "P12 test upstream".to_owned(),
            kind: "openai-compatible".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: Some(ids.egress_policy.clone()),
        });
        configuration.endpoints.push(EndpointConfiguration {
            id: ids.endpoint.clone(),
            upstream_id: ids.upstream.clone(),
            adapter_id: "openai-compatible.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: "https://gateway.example.test/v1".to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: None,
            transport: EndpointTransport::Http,
            enabled: true,
        });
    }

    fn add_p12_credential_and_routing(
        configuration: &mut ControlPlaneConfiguration,
        ids: &P12RuntimeIds,
        secret_store: &SecretStore,
    ) -> Result<(), Box<dyn Error>> {
        let associated_data =
            credential_associated_data(&configuration.version.id, &ids.credential, &ids.upstream)?;
        configuration.credentials.push(CredentialConfiguration {
            id: ids.credential.clone(),
            upstream_id: ids.upstream.clone(),
            kind: "bearer".to_owned(),
            encrypted_secret: secret_store.seal(b"test-bearer", &associated_data)?,
            status: CredentialStatus::Active,
            revision: 1,
        });
        configuration
            .endpoint_credential_bindings
            .push(EndpointCredentialBindingConfiguration {
                endpoint_id: ids.endpoint.clone(),
                credential_id: ids.credential.clone(),
                upstream_id: ids.upstream.clone(),
                enabled: true,
                priority: 0,
                weight: 1,
                concurrency: 1,
            });
        configuration.public_models.push(PublicModelConfiguration {
            id: ids.public_model.clone(),
            model_name: "p12-test-model".to_owned(),
            status: AdministrativeStatus::Active,
            display_name: "P12 test model".to_owned(),
            capabilities_json: "{}".to_owned(),
        });
        configuration.model_routes.push(ModelRouteConfiguration {
            id: ids.route.clone(),
            public_model_id: ids.public_model.clone(),
            policy: RoutePolicy::SmoothWeightedRoundRobin,
            max_attempts: 1,
            bootstrap_timeout_ms: 15_000,
        });
        configuration
            .route_candidates
            .push(RouteCandidateConfiguration {
                id: RouteCandidateId::try_new("p12-runtime-candidate")?,
                route_id: ids.route.clone(),
                endpoint_id: ids.endpoint.clone(),
                upstream_model: "p12-test-upstream-model".to_owned(),
                credential_scope: CredentialScope::EndpointBindings,
                transform_mode: TransformMode::Canonical,
                enabled: true,
                priority: 0,
                weight: 1,
                capability_override_json: r#"{"allow_unlisted_model":true}"#.to_owned(),
            });
        configuration.access_groups.push(AccessGroupConfiguration {
            id: ids.access_group.clone(),
            name: "P12 test group".to_owned(),
            status: AdministrativeStatus::Active,
            limits_json: "{}".to_owned(),
        });
        configuration
            .access_group_routes
            .push(AccessGroupRouteConfiguration {
                access_group_id: ids.access_group.clone(),
                route_id: ids.route.clone(),
                enabled: true,
            });
        configuration.client_keys.push(StoredClientKey::try_new(
            ClientKeyId::try_new("p12-runtime-client-key")?,
            ids.access_group.clone(),
            "rgw_0123456789abcdef",
            [0xA2_u8; 32],
            StoredClientKeyStatus::Active,
            None,
        )?);
        Ok(())
    }

    fn test_secret_store() -> Result<SecretStore, Box<dyn Error>> {
        let version = KeyVersion::try_new(1)?;
        Ok(SecretStore::new(MasterKeyRing::try_new(
            version,
            [(version, MasterKey::try_from_bytes([0xA1_u8; 32])?)],
        )?))
    }
}
