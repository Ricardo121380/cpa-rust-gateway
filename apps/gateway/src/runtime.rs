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
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
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
    AttemptEvent, AttemptOutcome, CanonicalEvent, CanonicalRequest, CanonicalResponse, EndpointId,
    ErrorScope, EventEmission, GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink,
    MessageEnd, MessageRole, MessageStart, RawExtensions, RawJson, RequestContext, RequestId,
    ResponseEnd, ResponseId, ResponseStart, StreamError, TextDelta, ToolCallArgumentsDelta,
    ToolCallEnd, ToolCallStart, Usage, UsageDelta,
};
use gateway_http_actix::{
    ResponsesHttpState, SystemResponsesMetadataFactory, default_stream_capacity,
    management_resources::{
        ManagementCatalogStatus, ManagementQuotaRecoveryState, ManagementRequestAttempt,
        ManagementRequestAttemptStage, ManagementRouteExplain, ManagementRouteExplainCandidate,
        ManagementRouteExplainRequest, ManagementRuntimeAvailabilityStatus, ManagementRuntimeError,
        ManagementRuntimeFacade, ManagementRuntimeTarget,
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

/// The largest complete non-streaming Responses body this runtime will buffer.
///
/// A completed Responses envelope carries the entire output text, every tool-call argument string,
/// and any reasoning item in one JSON document. Current models emit up to 128k output tokens, which
/// is roughly 0.5-2 MiB of UTF-8 once JSON escaping and the envelope are counted, so the previous
/// 64 KiB bound (about 16k tokens of ASCII) rejected ordinary long answers. P12 leases one
/// Credential at concurrency 1, so the worst-case resident body stays exactly one buffer.
const MAX_UPSTREAM_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// The largest undelivered SSE residue this runtime will buffer between two canonical events.
///
/// `response.output_text.done`, `response.output_item.done`, and `response.completed` each repeat
/// the whole accumulated output inside a single frame, so this bound must match the complete-body
/// bound rather than the size of one delta.
const MAX_SSE_FRAME_BYTES: usize = MAX_UPSTREAM_RESPONSE_BYTES;
/// The TCP/TLS connect bound shared by both response modes; expiry is always pre-first-byte.
const P12_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
/// The streaming first-byte bound: an SSE upstream emits `response.created` immediately.
const P12_STREAMING_TTFB_TIMEOUT: Duration = Duration::from_secs(30);
/// The streaming byte-liveness bound: the quiet period allowed between two upstream body reads.
///
/// This detects a dead transport. It cannot detect a wedged upstream that keeps the socket warm
/// with periodic keepalive frames; that is [`P12_STREAMING_PROGRESS_TIMEOUT`]'s job.
const P12_STREAMING_IDLE_TIMEOUT: Duration = Duration::from_mins(2);
/// The streaming semantic-liveness bound: the longest wall-clock gap tolerated between two frames
/// that prove generation is advancing.
///
/// A reasoning model that was not asked for summaries may legitimately emit nothing but
/// keepalives for minutes while it thinks, so this deadline sits several multiples past the
/// plausible tail of one uninterrupted thinking stretch (single-digit minutes at the highest
/// reasoning efforts served through this relay). Frames the decoder drops without any canonical
/// projection still count as progress when the upstream only produces them while generating --
/// reasoning traffic, content-part lifecycle, refusals -- while `response.in_progress` and SSE
/// comments never do. Expiry is a terminal stream failure: the lease-holding source drops and
/// this runtime's one Credential frees after at most this deadline plus one idle window, instead
/// of after the full absolute ceiling.
const P12_STREAMING_PROGRESS_TIMEOUT: Duration = Duration::from_mins(15);
/// The streaming absolute ceiling, deliberately far past any plausible single completion.
///
/// A streaming attempt is unretryable once its first semantic event has reached the client, so an
/// absolute deadline can only truncate a healthy answer, never fail it over. Byte liveness is the
/// idle bound's job and semantic liveness is the progress deadline's, which leaves this ceiling
/// as the last resort against an upstream that fabricates progress evidence forever. It stays at
/// one hour because a maximal healthy completion -- a long thinking stretch followed by a
/// six-figure-token answer at ordinary streaming rates -- genuinely approaches this order of
/// magnitude, and truncating one such answer past the unretryable boundary is strictly worse
/// than one bounded stale-lease window.
const P12_STREAMING_TOTAL_TIMEOUT: Duration = Duration::from_hours(1);
/// The one bounded wait for a complete non-streaming body.
///
/// A buffered `OpenAI`-compatible upstream sends response headers only after generation finishes,
/// so first-byte, response-idle, and total collapse into a single deadline for this mode. Every
/// byte is still pre-first-byte for the client, so expiry remains a safely retryable failure.
///
/// The whole non-streaming exchange still runs inside `AttemptDriver::start`, so the driver
/// declares this ceiling to the orchestrator through its `start_timeout` port: the Route's
/// bootstrap deadline (admitted at no more than [`P12_BOOTSTRAP_TIMEOUT_MILLISECONDS`]) keeps
/// governing when an attempt may begin, while the one in-flight non-streaming attempt is bounded
/// by this transport total instead of being cut at the bootstrap deadline.
const P12_NON_STREAMING_TOTAL_TIMEOUT: Duration = Duration::from_mins(10);
/// The isolated P12 streaming decoder retains at most this many Tool argument bytes per response.
///
/// This must admit exactly what the non-streaming decoder admits: there, every Tool argument
/// string arrives inside the one complete body, so the effective bound is the complete-body bound.
/// A smaller streaming bound would reject a response the same upstream serves successfully in the
/// other mode, and would do so after `ToolCallStart` already crossed the unretryable boundary.
const MAX_SSE_TOOL_ARGUMENT_BYTES: usize = MAX_UPSTREAM_RESPONSE_BYTES;
/// The isolated P12 streaming decoder admits at most this many Tool calls in one response.
const MAX_SSE_TOOL_CALLS: usize = 32;
/// The longest output-item or call identifier the streaming decoder retains per Tool call.
///
/// Real Responses implementations emit `fc_`-prefixed item identifiers and `call_`-prefixed
/// call identifiers of roughly 30-70 bytes with no documented upper bound, so 256 bytes is
/// safely generous. Without this bound each retained identifier is limited only by the frame
/// bound, letting one response pin [`MAX_SSE_TOOL_CALLS`] identifiers of up to
/// [`MAX_SSE_FRAME_BYTES`] each -- hundreds of mebibytes of state the bounded-buffer baseline
/// forbids.
const MAX_SSE_IDENTIFIER_BYTES: usize = 256;
/// The longest run of consecutive progress-free SSE frames the streaming decoder tolerates.
///
/// This is the clock-free complement of [`P12_STREAMING_PROGRESS_TIMEOUT`], enforced inside the
/// decoder where chunk boundaries cannot influence it: the run advances only per decoded frame,
/// never per transport read. It is sized so no plausible keepalive cadence reaches it before the
/// wall-clock deadline -- even one keepalive per second sustained for the whole absolute ceiling
/// stays under it -- while a keepalive spam loop is stopped after a bounded amount of decode
/// work instead of burning CPU until a timer expires.
const MAX_SSE_PROGRESS_FREE_FRAMES: usize = 4096;
/// The four JSON insignificant whitespace characters used to frame assembled Tool arguments.
const JSON_WHITESPACE: [char; 4] = [' ', '\t', '\n', '\r'];
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
    let attempt_stages = Arc::new(P12AttemptStageStore::new());
    let event_sink: Arc<dyn GatewayEventSink> =
        Arc::new(P12AttemptEventSink::new(Arc::clone(&attempt_stages)));
    let executor: Arc<dyn ResponsesExecutor> = match repository
        .load_active_configuration()
        .map_err(|_| RuntimeCompositionError::Unavailable)?
    {
        Some(configuration) => Arc::new(P12OpenAiResponsesExecutor::try_new(
            &configuration,
            secret_store,
            Arc::clone(&registry),
            Arc::clone(&attempt_stages),
            Arc::clone(&event_sink),
        )?),
        None => Arc::new(NoActiveConfigurationExecutor),
    };
    let authenticator = Arc::new(gateway_router::SnapshotClientKeyAuthenticator::new(
        Arc::clone(&registry),
        client_key_service,
    ));
    let data = ResponsesHttpState::with_snapshot_metadata_and_event_sink(
        executor,
        Arc::new(SystemResponsesMetadataFactory::new()),
        authenticator,
        event_sink,
        default_stream_capacity().map_err(|_| RuntimeCompositionError::Unavailable)?,
    );

    Ok(DataPlaneComposition {
        data,
        management_runtime: Box::new(SnapshotManagementRuntimeFacade {
            registry,
            attempt_stages,
        }),
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

/// P12's deliberately tiny in-memory Attempt-stage ledger.
///
/// The ledger is process-local and contains only an opaque request/attempt correlation, one closed
/// stage enum, and terminal success/failure. It never receives an endpoint, credential, URL,
/// header, body, status, error detail, model, token, or timestamp. Its request-path methods use
/// `try_lock`: loss or contention is remembered and later fails the management read closed instead
/// of delaying or changing an upstream request.
struct P12AttemptStageStore {
    records: Mutex<BTreeMap<RequestId, P12AttemptStageRecord>>,
    unavailable: AtomicBool,
}

struct P12AttemptStageRecord {
    stage: ManagementRequestAttemptStage,
    attempt_id: Option<String>,
    outcome: Option<&'static str>,
}

impl P12AttemptStageStore {
    const MAX_RECORDS: usize = 8;

    fn new() -> Self {
        Self {
            records: Mutex::new(BTreeMap::new()),
            unavailable: AtomicBool::new(false),
        }
    }

    fn record_stage(&self, request_id: &RequestId, stage: ManagementRequestAttemptStage) {
        if self.unavailable.load(Ordering::Acquire) {
            return;
        }
        let Ok(mut records) = self.records.try_lock() else {
            self.mark_unavailable();
            return;
        };
        if records.contains_key(request_id) {
            if let Some(record) = records.get_mut(request_id) {
                record.stage = stage;
            }
            return;
        }
        if records.len() >= Self::MAX_RECORDS {
            self.mark_unavailable();
            return;
        }
        records.insert(
            request_id.clone(),
            P12AttemptStageRecord {
                stage,
                attempt_id: None,
                outcome: None,
            },
        );
    }

    fn record_terminal(&self, event: &AttemptEvent) -> EventEmission {
        if self.unavailable.load(Ordering::Acquire) {
            return EventEmission::RequiredQueueFull;
        }
        let Ok(mut records) = self.records.try_lock() else {
            self.mark_unavailable();
            return EventEmission::RequiredQueueFull;
        };
        let Some(record) = records.get_mut(event.request_id()) else {
            self.mark_unavailable();
            return EventEmission::RequiredQueueFull;
        };
        if record.attempt_id.is_some() || record.outcome.is_some() {
            self.mark_unavailable();
            return EventEmission::RequiredQueueFull;
        }
        record.attempt_id = Some(event.attempt_id().as_str().to_owned());
        record.outcome = Some(match event.outcome() {
            AttemptOutcome::Succeeded => "succeeded",
            AttemptOutcome::Failed(_) => "failed",
        });
        EventEmission::Enqueued
    }

    fn list_request_attempts(
        &self,
        request_id: &RequestId,
    ) -> Result<Vec<ManagementRequestAttempt>, ManagementRuntimeError> {
        if self.unavailable.load(Ordering::Acquire) {
            return Err(ManagementRuntimeError::Unavailable);
        }
        let records = self
            .records
            .try_lock()
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        if self.unavailable.load(Ordering::Acquire) {
            return Err(ManagementRuntimeError::Unavailable);
        }
        let Some(record) = records.get(request_id) else {
            return Ok(Vec::new());
        };
        let (Some(attempt_id), Some(outcome)) = (&record.attempt_id, record.outcome) else {
            return Err(ManagementRuntimeError::Unavailable);
        };
        let attempt = ManagementRequestAttempt::try_new(attempt_id.clone(), outcome, None, None)?
            .with_stage(record.stage);
        Ok(vec![attempt])
    }

    fn mark_unavailable(&self) {
        self.unavailable.store(true, Ordering::Release);
    }
}

/// Bridges the existing non-blocking Attempt event port into P12's value-free stage ledger.
///
/// Request and Usage events are deliberately ignored because they carry no terminal stage and the
/// P12 management projection must remain an Attempt-only surface.
struct P12AttemptEventSink {
    attempts: Arc<P12AttemptStageStore>,
}

impl P12AttemptEventSink {
    fn new(attempts: Arc<P12AttemptStageStore>) -> Self {
        Self { attempts }
    }
}

impl GatewayEventSink for P12AttemptEventSink {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        match event {
            GatewayEvent::Attempt(attempt) => self.attempts.record_terminal(&attempt),
            GatewayEvent::Request(_)
            | GatewayEvent::Usage(_)
            | GatewayEvent::Health(_)
            | GatewayEvent::Diagnostic(_) => EventEmission::Disabled,
        }
    }
}

struct P12OpenAiResponsesExecutor {
    registry: Arc<RouteSnapshotRegistry>,
    snapshot_version: SnapshotVersion,
    orchestrator: Arc<AttemptOrchestrator>,
    endpoints: Arc<BTreeMap<EndpointId, EndpointRuntime>>,
    client_pool: Arc<UpstreamClientPool>,
    attempt_stages: Arc<P12AttemptStageStore>,
    event_sink: Arc<dyn GatewayEventSink>,
}

impl P12OpenAiResponsesExecutor {
    fn try_new(
        configuration: &ControlPlaneConfiguration,
        secret_store: &SecretStore,
        registry: Arc<RouteSnapshotRegistry>,
        attempt_stages: Arc<P12AttemptStageStore>,
        event_sink: Arc<dyn GatewayEventSink>,
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
            attempt_stages,
            event_sink,
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
        let attempt_stages = Arc::clone(&self.attempt_stages);
        let event_sink = Arc::clone(&self.event_sink);
        let context = execution.context().clone();
        let request = execution.request().clone();
        let usage_projection = p12_response_usage_projection(&request);
        let route_id = execution.route_id().cloned();
        let mode = execution.mode();
        let retry_gate = Arc::clone(execution.retry_gate());

        Box::pin(async move {
            let route_id = route_id.ok_or_else(route_not_found_error)?;
            let driver = OpenAiAttemptDriver {
                request_id: context.request_id().clone(),
                request,
                usage_projection,
                mode,
                endpoints,
                client_pool,
                attempt_stages,
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
    Ok(BTreeMap::from([(
        expected_endpoint,
        EndpointRuntime {
            endpoint,
            policy,
            resolver: Arc::new(SystemEgressDnsResolver),
            transports: P12TransportProfiles::try_new()?,
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
    transports: P12TransportProfiles,
}

/// The response-mode-specific transport deadlines for P12's one admitted Endpoint.
///
/// Streaming and non-streaming cannot share one profile. A streaming attempt must survive a long
/// completion whose first bytes already crossed the `FirstSemanticEvent` boundary and can no longer
/// be retried, so its absolute ceiling is a last-resort bound and its liveness is enforced by the
/// idle deadline. A non-streaming attempt is still entirely pre-first-byte for the client, so it
/// keeps one short bounded total that a failed attempt could legally be retried against.
struct P12TransportProfiles {
    streaming: UpstreamTransportProfile,
    non_streaming: UpstreamTransportProfile,
}

impl P12TransportProfiles {
    /// Builds both profiles from the fixed P12 deadlines, failing closed on an unsafe shape.
    fn try_new() -> Result<Self, RuntimeCompositionError> {
        let maximum_idle_connections_per_host =
            NonZeroUsize::new(1).ok_or(RuntimeCompositionError::Unavailable)?;
        let streaming = UpstreamTransportProfile::new(
            UpstreamTimeouts::try_new(
                P12_CONNECT_TIMEOUT,
                P12_STREAMING_TTFB_TIMEOUT,
                P12_STREAMING_IDLE_TIMEOUT,
                P12_STREAMING_TOTAL_TIMEOUT,
            )
            .map_err(|_| RuntimeCompositionError::Unavailable)?,
            UpstreamProxy::Direct,
            maximum_idle_connections_per_host,
        );
        // The transport bounds the wait for response headers by first-byte and, through reqwest's
        // read timeout, by response-idle as well. A buffered upstream returns nothing until it has
        // finished, so both must equal this mode's total instead of a shorter streaming value.
        let non_streaming = UpstreamTransportProfile::new(
            UpstreamTimeouts::try_new(
                P12_CONNECT_TIMEOUT,
                P12_NON_STREAMING_TOTAL_TIMEOUT,
                P12_NON_STREAMING_TOTAL_TIMEOUT,
                P12_NON_STREAMING_TOTAL_TIMEOUT,
            )
            .map_err(|_| RuntimeCompositionError::Unavailable)?,
            UpstreamProxy::Direct,
            maximum_idle_connections_per_host,
        );
        Ok(Self {
            streaming,
            non_streaming,
        })
    }

    /// Returns the profile whose deadlines match this attempt's response mode.
    const fn for_mode(&self, mode: ResponsesResponseMode) -> &UpstreamTransportProfile {
        match mode {
            ResponsesResponseMode::Streaming => &self.streaming,
            ResponsesResponseMode::NonStreaming => &self.non_streaming,
        }
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

struct OpenAiAttemptDriver {
    request_id: RequestId,
    request: CanonicalRequest,
    usage_projection: P12ResponseUsageProjection,
    mode: ResponsesResponseMode,
    endpoints: Arc<BTreeMap<EndpointId, EndpointRuntime>>,
    client_pool: Arc<UpstreamClientPool>,
    attempt_stages: Arc<P12AttemptStageStore>,
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
            self.attempt_stages.record_stage(
                &self.request_id,
                ManagementRequestAttemptStage::RequestConversion,
            );
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
            self.attempt_stages.record_stage(
                &self.request_id,
                ManagementRequestAttemptStage::EgressAdmission,
            );
            let admitted = runtime
                .policy
                .admit_url(outbound.url(), runtime.resolver.as_ref())
                .map_err(|_| AttemptFailure::NonRetryable(egress_rejected_error()))?;
            let request =
                p12_transport_request(&outbound, admitted).map_err(AttemptFailure::NonRetryable)?;
            self.attempt_stages.record_stage(
                &self.request_id,
                ManagementRequestAttemptStage::HttpTransport,
            );
            let mut response = self
                .client_pool
                .send(request, runtime.transports.for_mode(self.mode))
                .await
                .map_err(|_| AttemptFailure::Connection)?;

            self.attempt_stages
                .record_stage(&self.request_id, ManagementRequestAttemptStage::HttpStatus);
            match response.status() {
                200..=299 => {}
                429 => return Err(AttemptFailure::RateLimited { retry_after: None }),
                500..=599 => return Err(AttemptFailure::ServerError),
                _ => return Err(AttemptFailure::NonRetryable(provider_permanent_error())),
            }
            self.attempt_stages
                .record_stage(&self.request_id, ManagementRequestAttemptStage::ContentType);
            if !has_expected_content_type(&response, self.mode) {
                return Err(AttemptFailure::NonRetryable(upstream_protocol_error()));
            }

            match self.mode {
                ResponsesResponseMode::NonStreaming => {
                    let events = decode_json_response(
                        &mut response,
                        self.attempt_stages.as_ref(),
                        &self.request_id,
                        self.usage_projection,
                    )
                    .await?;
                    Ok(Box::new(FiniteEventSource::new(events)) as Box<dyn ResponsesEventSource>)
                }
                ResponsesResponseMode::Streaming => {
                    self.attempt_stages.record_stage(
                        &self.request_id,
                        ManagementRequestAttemptStage::SseBootstrap,
                    );
                    let source =
                        OpenAiSseEventSource::begin(response, self.usage_projection).await?;
                    Ok(Box::new(source) as Box<dyn ResponsesEventSource>)
                }
            }
        })
    }

    fn start_timeout(&self, remaining_bootstrap: Duration) -> Duration {
        p12_attempt_start_timeout(self.mode, remaining_bootstrap)
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

/// Selects the protocol-scoped usage projection from the trusted ingress namespace.
///
/// The only P12 Messages marker is produced by the Anthropic decoder for its required
/// `max_tokens` field. Its presence lets this isolated runtime keep `OpenAI` Responses' detailed
/// usage for a Responses caller while omitting only counters that an Anthropic usage object has no
/// field to carry. It is not a client-selectable transport flag and does not change the outbound
/// request conversion.
fn p12_response_usage_projection(request: &CanonicalRequest) -> P12ResponseUsageProjection {
    if request
        .extensions
        .get(P12_ANTHROPIC_MAX_TOKENS_EXTENSION)
        .is_some()
    {
        P12ResponseUsageProjection::AnthropicMessages
    } else {
        P12ResponseUsageProjection::OpenAiResponses
    }
}

fn upstream_response_mode(mode: ResponsesResponseMode) -> ResponseMode {
    match mode {
        ResponsesResponseMode::NonStreaming => ResponseMode::NonStreaming,
        ResponsesResponseMode::Streaming => ResponseMode::Streaming,
    }
}

/// Returns the per-attempt ceiling the orchestrator applies to one P12 `start` invocation.
///
/// Streaming keeps the Route's remaining bootstrap budget: a healthy SSE upstream emits
/// `response.created` immediately, so a cut here is an ordinary retryable pre-first-byte failure.
/// A buffered non-streaming upstream returns its response headers only after generation finishes,
/// so that one attempt must be allowed the transport's bounded total on top of the remaining
/// bootstrap budget. Every byte of it is still pre-first-byte for the client, which keeps an
/// expiry a safe pre-header failure, and the window for beginning another attempt stays governed
/// by the Route's bootstrap deadline.
const fn p12_attempt_start_timeout(
    mode: ResponsesResponseMode,
    remaining_bootstrap: Duration,
) -> Duration {
    match mode {
        ResponsesResponseMode::Streaming => remaining_bootstrap,
        ResponsesResponseMode::NonStreaming => {
            remaining_bootstrap.saturating_add(P12_NON_STREAMING_TOTAL_TIMEOUT)
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum P12ResponseUsageProjection {
    OpenAiResponses,
    AnthropicMessages,
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
    attempt_stages: &P12AttemptStageStore,
    request_id: &RequestId,
    usage_projection: P12ResponseUsageProjection,
) -> Result<Vec<CanonicalEvent>, AttemptFailure> {
    attempt_stages.record_stage(request_id, ManagementRequestAttemptStage::BodyRead);
    let mut body = Vec::new();
    loop {
        let next = response
            .next_chunk()
            .await
            .map_err(|_| AttemptFailure::Connection)?;
        let Some(chunk) = next else {
            break;
        };
        append_response_chunk(&mut body, &chunk).map_err(AttemptFailure::NonRetryable)?;
    }
    attempt_stages.record_stage(request_id, ManagementRequestAttemptStage::Decoder);
    decode_json_events_with_usage_projection(&body, usage_projection)
        .map_err(|_| AttemptFailure::BootstrapTruncated)
}

#[cfg(test)]
fn decode_json_events(body: &[u8]) -> Result<Vec<CanonicalEvent>, GatewayError> {
    decode_json_events_with_usage_projection(body, P12ResponseUsageProjection::OpenAiResponses)
}

fn decode_json_events_with_usage_projection(
    body: &[u8],
    usage_projection: P12ResponseUsageProjection,
) -> Result<Vec<CanonicalEvent>, GatewayError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| upstream_protocol_error())?;
    let response_id = required_string(&value, "id")?;
    if value.get("status").and_then(Value::as_str) != Some("completed") {
        return Err(upstream_protocol_error());
    }
    let usage = project_usage_for_response(decode_usage(value.get("usage"))?, usage_projection);
    let output = value
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(upstream_protocol_error)?;
    let mut events = vec![CanonicalEvent::ResponseStart(ResponseStart {
        response_id: ResponseId::try_new(response_id).map_err(|_| upstream_protocol_error())?,
        extensions: RawExtensions::default(),
    })];
    // Anthropic's Messages representation needs the reported input usage before MessageStart,
    // while the OpenAI Responses JSON envelope supplies one completed usage object at the end.
    // Preserve that fact as an interim input-only snapshot, never inventing usage when the
    // upstream did not report input tokens; the original complete snapshot remains final below.
    if let Some(usage) = usage.as_ref().filter(|usage| usage.input_tokens.is_some()) {
        events.push(CanonicalEvent::UsageDelta(UsageDelta {
            usage: initial_usage_snapshot(usage),
            is_final: false,
            extensions: RawExtensions::default(),
        }));
    }
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
    if message_open {
        events.push(CanonicalEvent::MessageEnd(MessageEnd::default()));
    }
    if let Some(usage) = usage {
        events.push(CanonicalEvent::UsageDelta(UsageDelta {
            usage,
            is_final: true,
            extensions: RawExtensions::default(),
        }));
    }
    events.push(CanonicalEvent::ResponseEnd(ResponseEnd {
        stop_reason: Some(if call_ids.is_empty() {
            "end_turn".to_owned()
        } else {
            "tool_use".to_owned()
        }),
        stop_sequence: None,
        extensions: RawExtensions::default(),
    }));
    CanonicalResponse::try_new(events)
        .map(CanonicalResponse::into_events)
        .map_err(|_| upstream_protocol_error())
}

#[cfg(test)]
fn decode_sse_events(body: &str, chunk_size: usize) -> Result<Vec<CanonicalEvent>, GatewayError> {
    decode_sse_events_with_usage_projection(
        body,
        chunk_size,
        P12ResponseUsageProjection::OpenAiResponses,
    )
}

#[cfg(test)]
fn decode_sse_events_with_usage_projection(
    body: &str,
    chunk_size: usize,
    usage_projection: P12ResponseUsageProjection,
) -> Result<Vec<CanonicalEvent>, GatewayError> {
    let mut decoder = OpenAiSseDecoder::new(usage_projection);
    let mut events = Vec::new();
    for chunk in body.as_bytes().chunks(chunk_size.max(1)) {
        decoder.push_chunk(chunk)?;
        loop {
            decoder.drain_buffered_frames()?;
            let Some(event) = decoder.take_event() else {
                break;
            };
            events.push(event);
        }
    }
    if decoder.is_finished() {
        Ok(events)
    } else {
        Err(stream_truncated_error())
    }
}

fn project_usage_for_response(
    usage: Option<Usage>,
    usage_projection: P12ResponseUsageProjection,
) -> Option<Usage> {
    usage.map(|mut usage| {
        if usage_projection == P12ResponseUsageProjection::AnthropicMessages {
            // Anthropic reports the aggregate output count but has no representation for the
            // OpenAI-specific reasoning/cached sub-counters. Keep every representable total and
            // cache-input field so the Messages boundary does not fail after a successful decode.
            usage.reasoning_tokens = None;
            usage.cached_tokens = None;
        }
        usage
    })
}

fn initial_usage_snapshot(usage: &Usage) -> Usage {
    Usage {
        input_tokens: usage.input_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_creation_tokens: usage.cache_creation_tokens,
        cached_tokens: usage.cached_tokens,
        ..Usage::default()
    }
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
    decoder: OpenAiSseDecoder,
    /// Upstream-wait budget between two decoder progress marks before the stream is declared
    /// wedged.
    progress_deadline: Duration,
    /// The decoder progress-mark count already accounted for by `progress_wait_spent`.
    observed_progress_marks: u64,
    /// Time spent awaiting upstream chunks since the last progress frame.
    ///
    /// Only the `next_chunk` awaits accrue here, never wall-clock time between `next_event`
    /// polls: a downstream client that stops reading backpressures the bounded event channel and
    /// freezes this source without the upstream being at fault, so counting that stall would
    /// misclassify a healthy completion as a wedged upstream.
    progress_wait_spent: Duration,
}

/// Transport-free `OpenAI` Responses SSE decoder for one streamed upstream response.
///
/// Frame reassembly and Canonical projection stay outside the transport type so the same state
/// machine can be driven from arbitrary chunk boundaries: only frame contents, never network
/// segmentation, may change the emitted Canonical sequence.
struct OpenAiSseDecoder {
    buffer: Vec<u8>,
    /// Bytes of `buffer` before this offset belong to frames already extracted by `take_frame`.
    consumed: usize,
    /// Scan resume point: no frame delimiter starts inside `buffer[self.consumed..self.scanned]`.
    scanned: usize,
    pending: VecDeque<CanonicalEvent>,
    lifecycle: SseLifecycle,
    usage_projection: P12ResponseUsageProjection,
    /// Consecutive frames that proved only socket liveness, reset by any progress frame.
    progress_free_frames: usize,
    /// Monotone count of consumed frames that proved generation is advancing.
    progress_marks: u64,
}

/// The bounded lifecycle of one streamed Responses body.
enum SseLifecycle {
    /// No `response.created` frame has been accepted yet.
    AwaitingResponseStart,
    /// `ResponseStart` was emitted; output items may open, stream, and close.
    Streaming(SseStreamingState),
    /// A terminal `ResponseEnd` or `StreamError` is already queued.
    Finished,
}

/// Output-item state retained between the frames of one open streamed response.
///
/// Every visible output item of one Responses response is projected into the single Canonical
/// Message that the non-streaming decoder also produces, so a text item followed by one or more
/// Function Call items remains exactly one Message.
#[derive(Default)]
struct SseStreamingState {
    message_open: bool,
    emitted_content: bool,
    tool_calls: BTreeMap<String, SseToolCall>,
    call_ids: BTreeSet<String>,
    retained_argument_bytes: usize,
}

/// One streamed Tool call correlated to its upstream output item identifier.
struct SseToolCall {
    call_id: String,
    assembled: String,
    released: usize,
    ended: bool,
}

impl SseLifecycle {
    const fn is_finished(&self) -> bool {
        matches!(self, Self::Finished)
    }

    /// Returns the open streaming state, rejecting a frame that arrives outside it.
    fn streaming_state(&mut self) -> Result<&mut SseStreamingState, GatewayError> {
        match self {
            Self::Streaming(state) => Ok(state),
            Self::AwaitingResponseStart | Self::Finished => Err(upstream_protocol_error()),
        }
    }
}

impl SseStreamingState {
    /// Opens the one Canonical Message that carries every output item of this response.
    fn ensure_message(&mut self, pending: &mut VecDeque<CanonicalEvent>) {
        if self.message_open {
            return;
        }
        pending.push_back(CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
            extensions: RawExtensions::default(),
        }));
        self.message_open = true;
    }

    /// Declares one streamed Tool call from a `function_call` output item.
    ///
    /// Identifiers longer than [`MAX_SSE_IDENTIFIER_BYTES`] fail closed: both are retained for
    /// the rest of the response, so the retained total must stay bounded by a small constant
    /// rather than by the frame bound.
    fn start_tool_call(
        &mut self,
        item: &Value,
        pending: &mut VecDeque<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_string(item, "id")?;
        let call_id = required_string(item, "call_id")?;
        let name = required_string(item, "name")?;
        if item_id.len() > MAX_SSE_IDENTIFIER_BYTES
            || call_id.len() > MAX_SSE_IDENTIFIER_BYTES
            || self.tool_calls.len() >= MAX_SSE_TOOL_CALLS
            || self.tool_calls.contains_key(&item_id)
            || !self.call_ids.insert(call_id.clone())
        {
            return Err(upstream_protocol_error());
        }
        self.ensure_message(pending);
        pending.push_back(CanonicalEvent::ToolCallStart(ToolCallStart {
            call_id: call_id.clone(),
            name,
            extensions: RawExtensions::default(),
        }));
        self.tool_calls.insert(
            item_id,
            SseToolCall {
                call_id,
                assembled: String::new(),
                released: 0,
                ended: false,
            },
        );
        Ok(())
    }

    /// Appends one reported Tool argument fragment to its open Tool call.
    fn append_tool_arguments(
        &mut self,
        value: &Value,
        pending: &mut VecDeque<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let item_id = required_string(value, "item_id")?;
        let delta = value
            .get("delta")
            .and_then(Value::as_str)
            .ok_or_else(upstream_protocol_error)?;
        let retained = self
            .retained_argument_bytes
            .checked_add(delta.len())
            .ok_or_else(upstream_protocol_error)?;
        if retained > MAX_SSE_TOOL_ARGUMENT_BYTES {
            return Err(upstream_protocol_error());
        }
        let call = self
            .tool_calls
            .get_mut(&item_id)
            .filter(|call| !call.ended)
            .ok_or_else(upstream_protocol_error)?;
        call.assembled.push_str(delta);
        call.release_arguments(pending);
        self.retained_argument_bytes = retained;
        Ok(())
    }

    /// Completes one open Tool call with its fully assembled JSON arguments.
    ///
    /// A completion frame supplies the arguments only when no fragment was streamed: the
    /// fragments the client already received stay authoritative, because both the `OpenAI`
    /// Responses and the Anthropic Messages encoders reject a completed Tool call whose final
    /// arguments differ from the delivered fragments.
    fn end_tool_call(
        &mut self,
        item_id: &str,
        reported_arguments: Option<&str>,
        authoritative: bool,
        pending: &mut VecDeque<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let retained = self.retained_argument_bytes;
        let Some(call) = self.tool_calls.get_mut(item_id) else {
            return Err(upstream_protocol_error());
        };
        // A repeated completion frame carries no new semantics for an already delivered call.
        if call.ended {
            return Ok(());
        }
        // An arguments completion frame that reports nothing for a call that assembled nothing is
        // not evidence of an empty input: the item's own completion frame still carries the real
        // string. Leave the call open so that frame can supply it; a call still open at
        // `response.completed` fails closed rather than delivering a fabricated input.
        if !authoritative && call.has_no_value() && reported_arguments.is_none_or(str::is_empty) {
            return Ok(());
        }
        if call.has_no_value()
            && let Some(reported) = reported_arguments
        {
            let next = retained
                .checked_add(reported.len())
                .ok_or_else(upstream_protocol_error)?;
            if next > MAX_SSE_TOOL_ARGUMENT_BYTES {
                return Err(upstream_protocol_error());
            }
            call.assembled.push_str(reported);
            call.release_arguments(pending);
            self.retained_argument_bytes = next;
        }
        pending.push_back(CanonicalEvent::ToolCallEnd(ToolCallEnd {
            call_id: call.call_id.clone(),
            arguments: call.completed_arguments()?,
            extensions: RawExtensions::default(),
        }));
        call.ended = true;
        self.emitted_content = true;
        Ok(())
    }

    /// Reports whether any declared Tool call has not yet ended.
    fn has_open_tool_call(&self) -> bool {
        self.tool_calls.values().any(|call| !call.ended)
    }

    /// Mirrors the non-streaming completion projection for this response.
    fn stop_reason(&self) -> &'static str {
        if self.call_ids.is_empty() {
            "end_turn"
        } else {
            "tool_use"
        }
    }
}

impl SseToolCall {
    /// Delivers every assembled argument byte that the JSON value already frames.
    ///
    /// Whitespace outside the value is held back: `RawJson` retains only the value itself, so
    /// releasing padding would desynchronize the delivered fragments from the completed arguments
    /// that both protocol encoders compare them against.
    fn release_arguments(&mut self, pending: &mut VecDeque<CanonicalEvent>) {
        let (start, end) = self.value_bounds();
        let from = self.released.max(start);
        if end <= from {
            return;
        }
        let delta = self.assembled[from..end].to_owned();
        self.released = end;
        pending.push_back(CanonicalEvent::ToolCallArgumentsDelta(
            ToolCallArgumentsDelta {
                call_id: self.call_id.clone(),
                delta,
                extensions: RawExtensions::default(),
            },
        ));
    }

    /// Returns the byte range of the assembled JSON value without its surrounding whitespace.
    fn value_bounds(&self) -> (usize, usize) {
        let start = self
            .assembled
            .len()
            .saturating_sub(self.assembled.trim_start_matches(JSON_WHITESPACE).len());
        let end = self.assembled.trim_end_matches(JSON_WHITESPACE).len();
        (start, end)
    }

    /// Returns whether no JSON value has been assembled yet.
    fn has_no_value(&self) -> bool {
        let (start, end) = self.value_bounds();
        end <= start
    }

    /// Returns the complete assembled arguments, normalizing an absent value to `{}`.
    fn completed_arguments(&self) -> Result<RawJson, GatewayError> {
        let (start, end) = self.value_bounds();
        // A Tool without required fields may report no arguments at all.  Normalizing that empty
        // input to one empty JSON object keeps the Tool call representable instead of failing an
        // otherwise complete stream.
        let arguments = if end <= start {
            "{}".to_owned()
        } else {
            self.assembled[start..end].to_owned()
        };
        let retained =
            RawJson::from_json_string(arguments.clone()).map_err(|_| upstream_protocol_error())?;
        if retained.get() == arguments {
            Ok(retained)
        } else {
            Err(upstream_protocol_error())
        }
    }
}

impl OpenAiSseDecoder {
    fn new(usage_projection: P12ResponseUsageProjection) -> Self {
        Self {
            buffer: Vec::new(),
            consumed: 0,
            scanned: 0,
            pending: VecDeque::new(),
            lifecycle: SseLifecycle::AwaitingResponseStart,
            usage_projection,
            progress_free_frames: 0,
            progress_marks: 0,
        }
    }

    /// Appends one bounded transport chunk without interpreting it.
    ///
    /// The frame bound applies to the undecoded residue only. Decoded bytes are compacted away
    /// once they outweigh that residue, so the bytes ever moved stay linear in the bytes
    /// streamed and the buffer itself never holds more than twice [`MAX_SSE_FRAME_BYTES`].
    fn push_chunk(&mut self, chunk: &[u8]) -> Result<(), GatewayError> {
        if self.consumed >= self.buffer.len().saturating_sub(self.consumed) {
            self.buffer.drain(..self.consumed);
            self.scanned = self.scanned.saturating_sub(self.consumed);
            self.consumed = 0;
        }
        let live = self.buffer.len().saturating_sub(self.consumed);
        if live.saturating_add(chunk.len()) > MAX_SSE_FRAME_BYTES {
            return Err(upstream_protocol_error());
        }
        self.buffer.extend_from_slice(chunk);
        Ok(())
    }

    /// Decodes buffered frames until one event is queued or no complete frame remains.
    fn drain_buffered_frames(&mut self) -> Result<(), GatewayError> {
        while self.pending.is_empty() && !self.lifecycle.is_finished() {
            let Some(frame) = self.take_frame() else {
                return Ok(());
            };
            self.consume_frame(&frame)?;
        }
        Ok(())
    }

    fn is_finished(&self) -> bool {
        self.lifecycle.is_finished()
    }

    fn peek_event(&self) -> Option<&CanonicalEvent> {
        self.pending.front()
    }

    fn take_event(&mut self) -> Option<CanonicalEvent> {
        self.pending.pop_front()
    }

    /// Extracts the next complete SSE frame, resuming the delimiter scan where it last stopped.
    ///
    /// `scanned` marks the delimiter-free prefix of the undecoded region, so every buffered byte
    /// is examined once no matter how many chunks or frames arrive. When no delimiter is found,
    /// the resume point holds back the last three bytes: a delimiter is at most four bytes, so
    /// one completed by a later chunk can begin no earlier than three bytes before the current
    /// end of the buffer.
    fn take_frame(&mut self) -> Option<Vec<u8>> {
        let start = self.scanned.max(self.consumed);
        let found = (start..self.buffer.len()).find_map(|position| {
            let suffix = &self.buffer[position..];
            if suffix.starts_with(b"\n\n") {
                Some((position, 2))
            } else if suffix.starts_with(b"\r\n\r\n") {
                Some((position, 4))
            } else {
                None
            }
        });
        let Some((position, delimiter_length)) = found else {
            self.scanned = self.buffer.len().saturating_sub(3).max(self.consumed);
            return None;
        };
        let frame = self.buffer[self.consumed..position].to_vec();
        self.consumed = position + delimiter_length;
        self.scanned = self.consumed;
        Some(frame)
    }

    /// Returns the monotone count of consumed frames that proved generation is advancing.
    ///
    /// The transport shell compares successive values to reset its wall-clock progress deadline.
    /// Keeping the counter here keeps the decoder clock-free: the classification of every frame
    /// is decided by its content alone, never by transport segmentation or elapsed time.
    const fn progress_marks(&self) -> u64 {
        self.progress_marks
    }

    /// Records one consumed frame that proves the upstream is still generating.
    fn note_progress_frame(&mut self) {
        self.progress_free_frames = 0;
        self.progress_marks = self.progress_marks.saturating_add(1);
    }

    /// Records one keepalive-class frame that proves only that the socket is alive.
    ///
    /// A run longer than [`MAX_SSE_PROGRESS_FREE_FRAMES`] is a wedged upstream, not a thinking
    /// model. It terminates with the same terminal projection as an upstream `response.failed`,
    /// so the lease-holding source drops and this runtime's one Credential frees.
    fn note_progress_free_frame(&mut self) {
        self.progress_free_frames = self.progress_free_frames.saturating_add(1);
        if self.progress_free_frames > MAX_SSE_PROGRESS_FREE_FRAMES {
            self.pending
                .push_back(CanonicalEvent::StreamError(StreamError {
                    error: provider_transient_error(),
                }));
            self.lifecycle = SseLifecycle::Finished;
        }
    }

    fn consume_frame(&mut self, frame: &[u8]) -> Result<(), GatewayError> {
        let frame = std::str::from_utf8(frame).map_err(|_| upstream_protocol_error())?;
        let data = frame
            .lines()
            .filter_map(|line| line.strip_prefix("data:").map(str::trim))
            .collect::<Vec<_>>()
            .join("\n");
        // SSE comments and keep-alive frames carry no event payload.  They must not alter the
        // Canonical lifecycle or consume the bounded response budget, but each one spends one
        // unit of the bounded progress-free run: only evidence of generation may refill it.
        if data.is_empty() {
            self.note_progress_free_frame();
            return Ok(());
        }
        let value: Value = serde_json::from_str(&data).map_err(|_| upstream_protocol_error())?;
        let kind = required_string(&value, "type")?;
        // `response.in_progress` is the one payload-bearing frame that proves only relay
        // liveness, never generation progress: a wedged upstream can repeat it forever.  Every
        // other accepted frame kind is emitted only while the model is actually producing
        // output, reasoning, or item lifecycle transitions, so it counts as progress even when
        // its canonical projection below is a no-op.
        if kind == "response.in_progress" {
            self.note_progress_free_frame();
            return Ok(());
        }
        self.note_progress_frame();

        match kind.as_str() {
            "response.created" => self.consume_response_created(&value),
            // Informational frames carry no canonical semantics. They must be ignored rather than
            // rejected: this dispatch runs past the unretryable boundary, so treating an upstream's
            // extra progress frame as fatal would truncate an otherwise healthy answer.
            "response.content_part.added"
            | "response.content_part.done"
            | "response.output_text.done"
            | "response.output_text.annotation.added"
            | "response.reasoning_summary_part.added"
            | "response.reasoning_summary_part.done"
            | "response.reasoning_summary_text.delta"
            | "response.reasoning_summary_text.done"
            | "response.reasoning_text.delta"
            | "response.reasoning_text.done"
            | "response.refusal.delta"
            | "response.refusal.done" => Ok(()),
            "response.output_item.added" => self.consume_output_item_added(&value),
            "response.output_text.delta" => self.consume_output_text_delta(&value),
            "response.function_call_arguments.delta" => {
                self.consume_function_arguments_delta(&value)
            }
            "response.function_call_arguments.done" => self.consume_function_arguments_done(&value),
            "response.output_item.done" => self.consume_output_item_done(&value),
            "response.completed" => self.consume_response_completed(&value),
            "response.incomplete" => self.consume_response_incomplete(&value),
            "response.failed" => self.consume_response_failed(),
            _ => Err(upstream_protocol_error()),
        }
    }

    fn consume_response_created(&mut self, value: &Value) -> Result<(), GatewayError> {
        if !matches!(self.lifecycle, SseLifecycle::AwaitingResponseStart) {
            return Err(upstream_protocol_error());
        }
        let response = value.get("response").ok_or_else(upstream_protocol_error)?;
        let response_id = ResponseId::try_new(required_string(response, "id")?)
            .map_err(|_| upstream_protocol_error())?;
        let usage =
            project_usage_for_response(decode_usage(response.get("usage"))?, self.usage_projection);
        self.lifecycle = SseLifecycle::Streaming(SseStreamingState::default());
        self.pending
            .push_back(CanonicalEvent::ResponseStart(ResponseStart {
                response_id,
                extensions: RawExtensions::default(),
            }));
        // Anthropic's Messages representation needs the reported input usage before MessageStart,
        // exactly as the non-streaming decoder supplies it.  Usage the upstream did not report is
        // never invented here.
        if let Some(usage) = usage.as_ref().filter(|usage| usage.input_tokens.is_some()) {
            self.pending
                .push_back(CanonicalEvent::UsageDelta(UsageDelta {
                    usage: initial_usage_snapshot(usage),
                    is_final: false,
                    extensions: RawExtensions::default(),
                }));
        }
        Ok(())
    }

    fn consume_output_item_added(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item = value.get("item").ok_or_else(upstream_protocol_error)?;
        let state = self.lifecycle.streaming_state()?;
        match item.get("type").and_then(Value::as_str) {
            Some("message") if item.get("role").and_then(Value::as_str) == Some("assistant") => {
                state.ensure_message(&mut self.pending);
                Ok(())
            }
            // A Responses model may open an internal reasoning item before its visible output.
            // P12 does not expose it, but it must not fail an otherwise valid response.
            Some("reasoning") => Ok(()),
            Some("function_call") => state.start_tool_call(item, &mut self.pending),
            _ => Err(upstream_protocol_error()),
        }
    }

    fn consume_output_text_delta(&mut self, value: &Value) -> Result<(), GatewayError> {
        let delta = value
            .get("delta")
            .and_then(Value::as_str)
            .ok_or_else(upstream_protocol_error)?;
        let state = self.lifecycle.streaming_state()?;
        if !state.message_open {
            return Err(upstream_protocol_error());
        }
        // An empty fragment carries no client-visible semantics and cannot become a Canonical
        // TextDelta, so it is dropped instead of failing the stream.
        if delta.is_empty() {
            return Ok(());
        }
        state.emitted_content = true;
        self.pending.push_back(CanonicalEvent::TextDelta(TextDelta {
            text: delta.to_owned(),
            extensions: RawExtensions::default(),
        }));
        Ok(())
    }

    fn consume_function_arguments_delta(&mut self, value: &Value) -> Result<(), GatewayError> {
        let state = self.lifecycle.streaming_state()?;
        state.append_tool_arguments(value, &mut self.pending)
    }

    fn consume_function_arguments_done(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item_id = required_string(value, "item_id")?;
        let state = self.lifecycle.streaming_state()?;
        state.end_tool_call(
            &item_id,
            value.get("arguments").and_then(Value::as_str),
            false,
            &mut self.pending,
        )
    }

    fn consume_output_item_done(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item = value.get("item").ok_or_else(upstream_protocol_error)?;
        if item.get("type").and_then(Value::as_str) != Some("function_call") {
            return Ok(());
        }
        let item_id = required_string(item, "id")?;
        let state = self.lifecycle.streaming_state()?;
        // A completed Function Call item is the last chance to close a Tool call whose upstream
        // omitted the dedicated arguments completion frame.
        state.end_tool_call(
            &item_id,
            item.get("arguments").and_then(Value::as_str),
            true,
            &mut self.pending,
        )
    }

    fn consume_response_completed(&mut self, value: &Value) -> Result<(), GatewayError> {
        self.finish_response(value, None)
    }

    /// Terminates a response the upstream stopped before it finished generating.
    ///
    /// The Responses API reports a `max_output_tokens` cutoff with this frame instead of
    /// `response.completed`, and every `/v1/messages` request carries an output limit, so this is
    /// an ordinary terminal frame. Rejecting it would truncate the stream past the unretryable
    /// boundary and hide the real reason the answer stopped.
    fn consume_response_incomplete(&mut self, value: &Value) -> Result<(), GatewayError> {
        let stop_reason = value
            .get("response")
            .and_then(|response| response.get("incomplete_details"))
            .and_then(|details| details.get("reason"))
            .and_then(Value::as_str)
            .map_or("max_tokens", |reason| match reason {
                "content_filter" => "refusal",
                _ => "max_tokens",
            });
        self.finish_response(value, Some(stop_reason))
    }

    /// Emits the shared terminal projection, overriding the stop reason when the upstream gave one.
    fn finish_response(
        &mut self,
        value: &Value,
        reported_stop_reason: Option<&str>,
    ) -> Result<(), GatewayError> {
        let response = value.get("response").ok_or_else(upstream_protocol_error)?;
        let usage =
            project_usage_for_response(decode_usage(response.get("usage"))?, self.usage_projection);
        let state = self.lifecycle.streaming_state()?;
        if !state.emitted_content || state.has_open_tool_call() {
            return Err(upstream_protocol_error());
        }
        let message_open = state.message_open;
        let stop_reason = reported_stop_reason
            .unwrap_or_else(|| state.stop_reason())
            .to_owned();
        if message_open {
            self.pending
                .push_back(CanonicalEvent::MessageEnd(MessageEnd::default()));
        }
        if let Some(usage) = usage {
            self.pending
                .push_back(CanonicalEvent::UsageDelta(UsageDelta {
                    usage,
                    is_final: true,
                    extensions: RawExtensions::default(),
                }));
        }
        self.pending
            .push_back(CanonicalEvent::ResponseEnd(ResponseEnd {
                stop_reason: Some(stop_reason),
                stop_sequence: None,
                extensions: RawExtensions::default(),
            }));
        self.lifecycle = SseLifecycle::Finished;
        Ok(())
    }

    fn consume_response_failed(&mut self) -> Result<(), GatewayError> {
        if matches!(self.lifecycle, SseLifecycle::AwaitingResponseStart) {
            return Err(upstream_protocol_error());
        }
        self.pending
            .push_back(CanonicalEvent::StreamError(StreamError {
                error: provider_transient_error(),
            }));
        self.lifecycle = SseLifecycle::Finished;
        Ok(())
    }
}

impl OpenAiSseEventSource {
    async fn begin(
        response: UpstreamHttpResponse,
        usage_projection: P12ResponseUsageProjection,
    ) -> Result<Self, AttemptFailure> {
        Self::begin_with_progress_deadline(
            response,
            usage_projection,
            P12_STREAMING_PROGRESS_TIMEOUT,
        )
        .await
    }

    /// Starts one streamed source under an explicit semantic-progress deadline.
    ///
    /// Production always passes [`P12_STREAMING_PROGRESS_TIMEOUT`] through [`Self::begin`]; the
    /// explicit parameter exists so tests can expire the deadline in milliseconds against a live
    /// peer instead of waiting out the production value.
    async fn begin_with_progress_deadline(
        response: UpstreamHttpResponse,
        usage_projection: P12ResponseUsageProjection,
        progress_deadline: Duration,
    ) -> Result<Self, AttemptFailure> {
        let mut source = Self {
            response,
            decoder: OpenAiSseDecoder::new(usage_projection),
            progress_deadline,
            observed_progress_marks: 0,
            progress_wait_spent: Duration::ZERO,
        };
        source
            .read_until_event()
            .await
            .map_err(|_| AttemptFailure::BootstrapTruncated)?;
        if !matches!(
            source.decoder.peek_event(),
            Some(CanonicalEvent::ResponseStart(_))
        ) {
            return Err(AttemptFailure::BootstrapTruncated);
        }
        Ok(source)
    }

    /// Restarts the upstream-wait progress window whenever the decoder consumed progress evidence.
    fn observe_decoder_progress(&mut self) {
        let marks = self.decoder.progress_marks();
        if marks != self.observed_progress_marks {
            self.observed_progress_marks = marks;
            self.progress_wait_spent = Duration::ZERO;
        }
    }

    async fn read_until_event(&mut self) -> Result<(), GatewayError> {
        loop {
            self.decoder.drain_buffered_frames()?;
            self.observe_decoder_progress();
            if self.decoder.peek_event().is_some() || self.decoder.is_finished() {
                return Ok(());
            }
            // The transport's byte-idle bound wakes the wait below at least once per idle
            // window, so this check runs even when the upstream sends only keepalives that
            // reset that byte-idle timer.  A wedged upstream therefore holds this runtime's one
            // Credential lease for at most the progress deadline plus one idle window, while a
            // thinking model stays alive through any genuine progress frame.
            if self.progress_wait_spent >= self.progress_deadline {
                return Err(provider_transient_error());
            }
            let wait_started = Instant::now();
            let next = self.response.next_chunk().await?;
            self.progress_wait_spent = self
                .progress_wait_spent
                .saturating_add(wait_started.elapsed());
            let Some(chunk) = next else {
                return Err(stream_truncated_error());
            };
            self.decoder.push_chunk(&chunk)?;
        }
    }
}

impl ResponsesEventSource for OpenAiSseEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            if self.decoder.peek_event().is_none() && !self.decoder.is_finished() {
                self.read_until_event().await?;
            }
            Ok(self.decoder.take_event())
        })
    }
}

/// Appends one raw non-streaming body chunk under the bounded complete-response limit.
fn append_response_chunk(body: &mut Vec<u8>, chunk: &[u8]) -> Result<(), GatewayError> {
    if body.len().saturating_add(chunk.len()) > MAX_UPSTREAM_RESPONSE_BYTES {
        return Err(upstream_protocol_error());
    }
    body.extend_from_slice(chunk);
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
    attempt_stages: Arc<P12AttemptStageStore>,
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
        request_id: &gateway_core::RequestId,
    ) -> Result<Vec<ManagementRequestAttempt>, ManagementRuntimeError> {
        self.attempt_stages.list_request_attempts(request_id)
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
        num::NonZeroUsize,
        path::{Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use actix_web::{
        App,
        http::{StatusCode, header},
        test as actix_test, web,
    };
    use gateway_auth::{
        ClientKeyAuthenticator, InMemoryClientKey, InMemoryClientKeyAuthenticator,
        client_key::{ClientKeyPepper, ClientKeyService},
    };
    use gateway_control::{
        control_plane_service::credential_associated_data,
        management_service::{ManagementActor, ManagementService},
    };
    use gateway_core::{
        AccessGroupId, AttemptEvent, AttemptOutcome, AttemptRetryDecision, CanonicalEvent,
        CanonicalResponse, ClientKeyId, CredentialId, EgressPolicyId, EndpointId, ErrorScope,
        EventEmission, GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink, ProviderId,
        PublicModelId, RequestId, RouteCandidateId, RouteId, UpstreamId,
    };
    use gateway_http_actix::{
        ResponsesHttpState, configure, default_stream_capacity,
        management_resources::{ManagementRequestAttemptStage, ManagementRuntimeError},
    };
    use gateway_router::{
        DeterministicMockEmission, DeterministicMockResponsesExecutor, ResponsesEventSource,
        ResponsesResponseMode,
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
        AdmittedEgressTarget, EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost,
        EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy, UpstreamClientPool,
        UpstreamHttpMethod, UpstreamHttpRequest, UpstreamProxy, UpstreamTimeouts,
        UpstreamTransportProfile,
    };
    use protocol_openai_responses::{ResponseMode, decode_request};
    use provider_openai_compatible::{
        OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesRequestBuilder,
    };
    use serde_json::Value;

    use super::{
        MAX_SSE_FRAME_BYTES, MAX_SSE_IDENTIFIER_BYTES, MAX_SSE_PROGRESS_FREE_FRAMES,
        MAX_SSE_TOOL_CALLS, MAX_UPSTREAM_RESPONSE_BYTES, OpenAiSseDecoder, OpenAiSseEventSource,
        P12_BOOTSTRAP_TIMEOUT_MILLISECONDS, P12_CONNECT_TIMEOUT,
        P12_KRILL_COMPATIBILITY_USER_AGENT, P12_NON_STREAMING_TOTAL_TIMEOUT,
        P12_STAGING_ENDPOINT_ID, P12_STREAMING_IDLE_TIMEOUT, P12_STREAMING_PROGRESS_TIMEOUT,
        P12_STREAMING_TOTAL_TIMEOUT, P12_STREAMING_TTFB_TIMEOUT, P12AttemptEventSink,
        P12AttemptStageStore, P12ResponseUsageProjection, P12TransportProfiles,
        append_response_chunk, build_data_plane_composition, decode_json_events,
        decode_json_events_with_usage_projection, decode_sse_events,
        decode_sse_events_with_usage_projection, has_exact_p12_egress_shape,
        has_p12_unlisted_model_override, p12_attempt_start_timeout, p12_openai_compatible_request,
        p12_response_usage_projection, p12_transport_headers, p12_transport_request,
        staging_route_compiler,
    };

    /// Renders one upstream SSE body from ordered `data`-only frames.
    ///
    /// The decoder reads only `data:` lines, so building the body here keeps fixtures free of the
    /// leading indentation that a multi-line raw string literal would inject into every frame.
    fn sse_stream_body<T: AsRef<str>>(frames: &[T]) -> String {
        use std::fmt::Write as _;

        frames.iter().fold(String::new(), |mut body, frame| {
            let _ = writeln!(body, "data: {}\n", frame.as_ref());
            body
        })
    }

    fn canonical_event_labels(events: &[CanonicalEvent]) -> Vec<&'static str> {
        events
            .iter()
            .map(|event| match event {
                CanonicalEvent::ResponseStart(_) => "response_start",
                CanonicalEvent::MessageStart(_) => "message_start",
                CanonicalEvent::TextDelta(_) => "text_delta",
                CanonicalEvent::ReasoningDelta(_) => "reasoning_delta",
                CanonicalEvent::ToolCallStart(_) => "tool_call_start",
                CanonicalEvent::ToolCallArgumentsDelta(_) => "tool_call_arguments_delta",
                CanonicalEvent::ToolCallEnd(_) => "tool_call_end",
                CanonicalEvent::UsageDelta(_) => "usage_delta",
                CanonicalEvent::MessageEnd(_) => "message_end",
                CanonicalEvent::ResponseEnd(_) => "response_end",
                CanonicalEvent::StreamError(_) => "stream_error",
            })
            .collect()
    }

    /// One realistic streamed Responses body whose only visible output item is a Function Call.
    fn p12_streamed_tool_body() -> String {
        sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-stream-tool","usage":{"input_tokens":3}}}"#,
            r#"{"type":"response.in_progress","response":{"id":"response-p12-stream-tool"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"fc-p12-stream","type":"function_call","call_id":"call-p12-stream","name":"echo","arguments":""}}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-p12-stream","output_index":0,"delta":"{\"value\":"}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-p12-stream","output_index":0,"delta":"\"ok\"}"}"#,
            r#"{"type":"response.function_call_arguments.done","item_id":"fc-p12-stream","output_index":0,"arguments":"{\"value\":\"ok\"}"}"#,
            r#"{"type":"response.output_item.done","output_index":0,"item":{"id":"fc-p12-stream","type":"function_call","call_id":"call-p12-stream","name":"echo","arguments":"{\"value\":\"ok\"}","status":"completed"}}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-stream-tool","status":"completed","usage":{"input_tokens":3,"output_tokens":5,"output_tokens_details":{"reasoning_tokens":2}}}}"#,
        ])
    }

    #[test]
    fn streamed_function_call_emits_a_complete_canonical_tool_lifecycle()
    -> Result<(), Box<dyn Error>> {
        let body = p12_streamed_tool_body();
        let events = decode_sse_events(&body, body.len())?;

        assert_eq!(
            canonical_event_labels(&events),
            vec![
                "response_start",
                "usage_delta",
                "message_start",
                "tool_call_start",
                "tool_call_arguments_delta",
                "tool_call_arguments_delta",
                "tool_call_end",
                "message_end",
                "usage_delta",
                "response_end",
            ]
        );
        assert!(events.iter().any(|event| matches!(
            event,
            CanonicalEvent::ToolCallStart(start)
                if start.call_id == "call-p12-stream" && start.name == "echo"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            CanonicalEvent::ToolCallEnd(end)
                if end.call_id == "call-p12-stream" && end.arguments.get() == r#"{"value":"ok"}"#
        )));
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::ResponseEnd(end)) if end.stop_reason.as_deref() == Some("tool_use")
        ));
        assert!(CanonicalResponse::try_new(events).is_ok());
        Ok(())
    }

    #[test]
    fn streamed_text_then_function_call_stays_one_message_and_reports_tool_use()
    -> Result<(), Box<dyn Error>> {
        let body = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-stream-mixed"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"msg-p12","type":"message","role":"assistant","content":[]}}"#,
            r#"{"type":"response.content_part.added","item_id":"msg-p12","output_index":0}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg-p12","output_index":0,"delta":"ok"}"#,
            r#"{"type":"response.output_text.done","item_id":"msg-p12","output_index":0}"#,
            r#"{"type":"response.content_part.done","item_id":"msg-p12","output_index":0}"#,
            r#"{"type":"response.output_item.done","output_index":0,"item":{"id":"msg-p12","type":"message","role":"assistant","status":"completed"}}"#,
            r#"{"type":"response.output_item.added","output_index":1,"item":{"id":"fc-p12","type":"function_call","call_id":"call-p12-mixed","name":"echo","arguments":""}}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-p12","output_index":1,"delta":"{\"value\":\"ok\"}"}"#,
            r#"{"type":"response.function_call_arguments.done","item_id":"fc-p12","output_index":1,"arguments":"{\"value\":\"ok\"}"}"#,
            r#"{"type":"response.output_item.done","output_index":1,"item":{"id":"fc-p12","type":"function_call","call_id":"call-p12-mixed","name":"echo","arguments":"{\"value\":\"ok\"}","status":"completed"}}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-stream-mixed","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ]);
        let events = decode_sse_events(&body, body.len())?;
        let labels = canonical_event_labels(&events);

        assert_eq!(
            labels
                .iter()
                .filter(|label| **label == "message_start")
                .count(),
            1
        );
        assert_eq!(
            labels
                .iter()
                .filter(|label| **label == "message_end")
                .count(),
            1
        );
        let text_index = labels
            .iter()
            .position(|label| *label == "text_delta")
            .ok_or("missing text delta")?;
        let tool_index = labels
            .iter()
            .position(|label| *label == "tool_call_start")
            .ok_or("missing tool call start")?;
        let message_end_index = labels
            .iter()
            .position(|label| *label == "message_end")
            .ok_or("missing message end")?;
        assert!(text_index < tool_index);
        assert!(tool_index < message_end_index);
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::ResponseEnd(end)) if end.stop_reason.as_deref() == Some("tool_use")
        ));
        assert!(CanonicalResponse::try_new(events).is_ok());
        Ok(())
    }

    #[test]
    fn streamed_parallel_function_calls_emit_two_independent_tool_lifecycles()
    -> Result<(), Box<dyn Error>> {
        let body = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-stream-parallel"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"fc-alpha","type":"function_call","call_id":"call-alpha","name":"first","arguments":""}}"#,
            r#"{"type":"response.output_item.added","output_index":1,"item":{"id":"fc-beta","type":"function_call","call_id":"call-beta","name":"second","arguments":""}}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-alpha","delta":"{\"x\":"}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-beta","delta":"{\"y\":"}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-alpha","delta":"1}"}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-beta","delta":"2}"}"#,
            r#"{"type":"response.function_call_arguments.done","item_id":"fc-alpha","arguments":"{\"x\":1}"}"#,
            r#"{"type":"response.function_call_arguments.done","item_id":"fc-beta","arguments":"{\"y\":2}"}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-stream-parallel","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ]);
        let events = decode_sse_events(&body, body.len())?;

        assert_eq!(
            events
                .iter()
                .filter_map(|event| match event {
                    CanonicalEvent::ToolCallArgumentsDelta(delta) => Some(delta.call_id.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec!["call-alpha", "call-beta", "call-alpha", "call-beta"]
        );
        assert_eq!(
            events
                .iter()
                .filter_map(|event| match event {
                    CanonicalEvent::ToolCallEnd(end) =>
                        Some((end.call_id.as_str(), end.arguments.get())),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![("call-alpha", r#"{"x":1}"#), ("call-beta", r#"{"y":2}"#)]
        );
        assert_eq!(
            canonical_event_labels(&events)
                .iter()
                .filter(|label| **label == "message_start")
                .count(),
            1
        );
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::ResponseEnd(end)) if end.stop_reason.as_deref() == Some("tool_use")
        ));
        assert!(CanonicalResponse::try_new(events).is_ok());
        Ok(())
    }

    #[test]
    fn streamed_tool_arguments_are_independent_of_transport_chunk_boundaries()
    -> Result<(), Box<dyn Error>> {
        use std::fmt::Write as _;

        // Mixed frame delimiters and interleaved comment frames force every resume path of the
        // scan cursor: a CRLF delimiter split across appends, several frames inside one chunk,
        // and no-event frames between event-bearing ones.
        let frames = [
            r#"{"type":"response.created","response":{"id":"response-p12-stream-chunks"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"fc-chunks","type":"function_call","call_id":"call-chunks","name":"echo","arguments":""}}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-chunks","delta":"{\"value\":\"caf"}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-chunks","delta":"é\"}"}"#,
            r#"{"type":"response.function_call_arguments.done","item_id":"fc-chunks","arguments":"{\"value\":\"café\"}"}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-stream-chunks","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ];
        let body = frames
            .iter()
            .enumerate()
            .fold(String::new(), |mut body, (index, frame)| {
                let delimiter = if index % 2 == 0 { "\r\n\r\n" } else { "\n\n" };
                let _ = write!(body, "data: {frame}{delimiter}: keep-alive{delimiter}");
                body
            });
        let reference = decode_sse_events(&body, body.len())?;

        for chunk_size in [1, 3, 29] {
            assert_eq!(decode_sse_events(&body, chunk_size)?, reference);
        }
        assert!(reference.iter().any(|event| matches!(
            event,
            CanonicalEvent::ToolCallEnd(end) if end.arguments.get() == "{\"value\":\"café\"}"
        )));
        assert!(CanonicalResponse::try_new(reference).is_ok());
        Ok(())
    }

    #[test]
    fn streamed_tool_arguments_reported_only_by_the_item_completion_are_preserved()
    -> Result<(), Box<dyn Error>> {
        // The dedicated arguments frame reports nothing; the item's own completion carries the
        // real string. Closing on the earlier frame would deliver a fabricated empty input.
        let body = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-late-args"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"fc-late","type":"function_call","call_id":"call-late","name":"echo","arguments":""}}"#,
            r#"{"type":"response.function_call_arguments.done","item_id":"fc-late"}"#,
            r#"{"type":"response.output_item.done","output_index":0,"item":{"id":"fc-late","type":"function_call","call_id":"call-late","name":"echo","arguments":"{\"value\":\"ok\"}","status":"completed"}}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-late-args","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ]);
        let events = decode_sse_events(&body, 7)?;

        assert!(events.iter().any(|event| matches!(
            event,
            CanonicalEvent::ToolCallEnd(end)
                if end.call_id == "call-late" && end.arguments.get() == r#"{"value":"ok"}"#
        )));
        assert!(CanonicalResponse::try_new(events).is_ok());
        Ok(())
    }

    #[test]
    fn streamed_response_incomplete_terminates_with_the_reported_stop_reason()
    -> Result<(), Box<dyn Error>> {
        // Every /v1/messages request carries an output limit, so a max_output_tokens cutoff is an
        // ordinary terminal frame rather than a protocol failure.
        let body = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-incomplete"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"msg-1","type":"message","role":"assistant"}}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg-1","delta":"partial"}"#,
            r#"{"type":"response.incomplete","response":{"id":"response-p12-incomplete","status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ]);
        let events = decode_sse_events(&body, 11)?;

        assert_eq!(
            canonical_event_labels(&events),
            vec![
                "response_start",
                "message_start",
                "text_delta",
                "message_end",
                "usage_delta",
                "response_end",
            ]
        );
        assert!(events.iter().any(|event| matches!(
            event,
            CanonicalEvent::ResponseEnd(end) if end.stop_reason.as_deref() == Some("max_tokens")
        )));
        assert!(CanonicalResponse::try_new(events).is_ok());
        Ok(())
    }

    #[test]
    fn streamed_informational_reasoning_frames_never_abort_a_healthy_stream()
    -> Result<(), Box<dyn Error>> {
        let body = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-reasoning"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"rs-1","type":"reasoning"}}"#,
            r#"{"type":"response.reasoning_summary_part.added","item_id":"rs-1","summary_index":0}"#,
            r#"{"type":"response.reasoning_summary_text.delta","item_id":"rs-1","delta":"thinking"}"#,
            r#"{"type":"response.reasoning_summary_text.done","item_id":"rs-1","text":"thinking"}"#,
            r#"{"type":"response.output_item.added","output_index":1,"item":{"id":"msg-1","type":"message","role":"assistant"}}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg-1","delta":"visible"}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-reasoning","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ]);
        let events = decode_sse_events(&body, 13)?;

        assert_eq!(
            canonical_event_labels(&events),
            vec![
                "response_start",
                "message_start",
                "text_delta",
                "message_end",
                "usage_delta",
                "response_end",
            ]
        );
        assert!(CanonicalResponse::try_new(events).is_ok());
        Ok(())
    }

    #[test]
    fn streamed_function_call_without_arguments_normalizes_the_empty_input()
    -> Result<(), Box<dyn Error>> {
        let body = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-stream-empty"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"fc-empty","type":"function_call","call_id":"call-empty","name":"enter_plan_mode","arguments":""}}"#,
            r#"{"type":"response.function_call_arguments.done","item_id":"fc-empty","arguments":""}"#,
            r#"{"type":"response.output_item.done","output_index":0,"item":{"id":"fc-empty","type":"function_call","call_id":"call-empty","name":"enter_plan_mode","arguments":"","status":"completed"}}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-stream-empty","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ]);
        let events = decode_sse_events(&body, 5)?;

        assert_eq!(
            canonical_event_labels(&events),
            vec![
                "response_start",
                "message_start",
                "tool_call_start",
                "tool_call_end",
                "message_end",
                "usage_delta",
                "response_end",
            ]
        );
        assert!(events.iter().any(|event| matches!(
            event,
            CanonicalEvent::ToolCallEnd(end)
                if end.call_id == "call-empty" && end.arguments.get() == "{}"
        )));
        assert!(CanonicalResponse::try_new(events).is_ok());
        Ok(())
    }

    #[test]
    fn streamed_text_only_response_ignores_reasoning_items_and_reports_end_turn()
    -> Result<(), Box<dyn Error>> {
        let body = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-stream-text"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"rsn-p12","type":"reasoning","summary":[]}}"#,
            r#"{"type":"response.output_item.added","output_index":1,"item":{"id":"msg-p12","type":"message","role":"assistant","content":[]}}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg-p12","output_index":1,"delta":"ok"}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-stream-text","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ]);
        let events = decode_sse_events(&body, 11)?;

        assert_eq!(
            canonical_event_labels(&events),
            vec![
                "response_start",
                "message_start",
                "text_delta",
                "message_end",
                "usage_delta",
                "response_end",
            ]
        );
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::ResponseEnd(end)) if end.stop_reason.as_deref() == Some("end_turn")
        ));
        assert!(CanonicalResponse::try_new(events).is_ok());
        Ok(())
    }

    #[test]
    fn streamed_tool_frames_reject_unknown_items_duplicate_calls_and_open_completion() {
        let created =
            r#"{"type":"response.created","response":{"id":"response-p12-stream-guard"}}"#;
        let added = r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"fc-guard","type":"function_call","call_id":"call-guard","name":"echo","arguments":""}}"#;
        let unknown_item = sse_stream_body(&[
            created,
            added,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-missing","delta":"{}"}"#,
        ]);
        let duplicate_call = sse_stream_body(&[
            created,
            added,
            r#"{"type":"response.output_item.added","output_index":1,"item":{"id":"fc-other","type":"function_call","call_id":"call-guard","name":"echo","arguments":""}}"#,
        ]);
        let open_completion = sse_stream_body(&[
            created,
            added,
            r#"{"type":"response.completed","response":{"id":"response-p12-stream-guard","status":"completed"}}"#,
        ]);
        let mut overflow = vec![created.to_owned()];
        for index in 0..=MAX_SSE_TOOL_CALLS {
            overflow.push(format!(
                r#"{{"type":"response.output_item.added","item":{{"id":"fc-{index}","type":"function_call","call_id":"call-{index}","name":"echo","arguments":""}}}}"#
            ));
        }
        let overflow = sse_stream_body(&overflow);

        for body in [unknown_item, duplicate_call, open_completion, overflow] {
            assert_eq!(
                decode_sse_events(&body, body.len())
                    .err()
                    .map(|error| (error.code(), error.scope())),
                Some((GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream))
            );
        }
    }

    #[test]
    fn streamed_tool_identifiers_at_the_bound_decode_and_longer_ones_fail_closed()
    -> Result<(), Box<dyn Error>> {
        let bounded_item_id = "i".repeat(MAX_SSE_IDENTIFIER_BYTES);
        let bounded_call_id = "c".repeat(MAX_SSE_IDENTIFIER_BYTES);
        let accepted = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-id-bound"}}"#.to_owned(),
            format!(
                r#"{{"type":"response.output_item.added","output_index":0,"item":{{"id":"{bounded_item_id}","type":"function_call","call_id":"{bounded_call_id}","name":"echo","arguments":""}}}}"#
            ),
            format!(
                r#"{{"type":"response.function_call_arguments.done","item_id":"{bounded_item_id}","arguments":"{{}}"}}"#
            ),
            r#"{"type":"response.completed","response":{"id":"response-p12-id-bound","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#.to_owned(),
        ]);
        let events = decode_sse_events(&accepted, 17)?;
        assert!(events.iter().any(|event| matches!(
            event,
            CanonicalEvent::ToolCallEnd(end) if end.call_id == bounded_call_id
        )));

        let long_item_id = "i".repeat(MAX_SSE_IDENTIFIER_BYTES + 1);
        let long_call_id = "c".repeat(MAX_SSE_IDENTIFIER_BYTES + 1);
        let oversized_item = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-id-guard"}}"#.to_owned(),
            format!(
                r#"{{"type":"response.output_item.added","output_index":0,"item":{{"id":"{long_item_id}","type":"function_call","call_id":"call-short","name":"echo","arguments":""}}}}"#
            ),
        ]);
        let oversized_call = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-id-guard"}}"#.to_owned(),
            format!(
                r#"{{"type":"response.output_item.added","output_index":0,"item":{{"id":"fc-short","type":"function_call","call_id":"{long_call_id}","name":"echo","arguments":""}}}}"#
            ),
        ]);
        for body in [oversized_item, oversized_call] {
            assert_eq!(
                decode_sse_events(&body, body.len())
                    .err()
                    .map(|error| (error.code(), error.scope())),
                Some((GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream))
            );
        }
        Ok(())
    }

    #[actix_web::test]
    async fn p12_streamed_tool_lifecycle_is_encodable_by_the_openai_responses_boundary()
    -> Result<(), Box<dyn Error>> {
        let body = p12_streamed_tool_body();
        let events = decode_sse_events(&body, 9)?;
        let app = actix_test::init_service(
            App::new()
                .app_data(web::Data::new(p12_decoded_messages_http_state(events)?))
                .configure(configure),
        )
        .await;
        let request = actix_test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header((header::AUTHORIZATION, "Bearer p12-decoder-http-test-key"))
            .set_payload(r#"{"model":"p12-decoder-http-model","input":"ok"}"#)
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(&actix_test::read_body(response).await)?;
        assert_eq!(
            body.pointer("/status").and_then(Value::as_str),
            Some("completed")
        );
        assert_eq!(
            body.pointer("/output/0/type").and_then(Value::as_str),
            Some("function_call")
        );
        assert_eq!(
            body.pointer("/output/0/call_id").and_then(Value::as_str),
            Some("call-p12-stream")
        );
        assert_eq!(
            body.pointer("/output/0/arguments").and_then(Value::as_str),
            Some(r#"{"value":"ok"}"#)
        );
        assert_eq!(
            body.pointer("/usage/output_tokens_details/reasoning_tokens")
                .and_then(Value::as_u64),
            Some(2)
        );
        Ok(())
    }

    #[actix_web::test]
    async fn p12_streamed_tool_lifecycle_is_encodable_by_the_anthropic_messages_boundary()
    -> Result<(), Box<dyn Error>> {
        let body = p12_streamed_tool_body();
        let events = decode_sse_events_with_usage_projection(
            &body,
            9,
            P12ResponseUsageProjection::AnthropicMessages,
        )?;
        let app = actix_test::init_service(
            App::new()
                .app_data(web::Data::new(p12_decoded_messages_http_state(events)?))
                .configure(configure),
        )
        .await;
        let request = actix_test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header((header::AUTHORIZATION, "Bearer p12-decoder-http-test-key"))
            .set_payload(
                r#"{
                  "model":"p12-decoder-http-model",
                  "max_tokens":1,
                  "messages":[{"role":"user","content":"ok"}],
                  "tools":[{"name":"echo","input_schema":{"type":"object"}}]
                }"#,
            )
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(&actix_test::read_body(response).await)?;
        assert_eq!(
            body.pointer("/stop_reason").and_then(Value::as_str),
            Some("tool_use")
        );
        assert_eq!(
            body.pointer("/content/0/type").and_then(Value::as_str),
            Some("tool_use")
        );
        assert_eq!(
            body.pointer("/content/0/id").and_then(Value::as_str),
            Some("call-p12-stream")
        );
        assert_eq!(
            body.pointer("/content/0/input/value")
                .and_then(Value::as_str),
            Some("ok")
        );
        assert_eq!(
            body.pointer("/usage/input_tokens").and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            body.pointer("/usage/output_tokens").and_then(Value::as_u64),
            Some(5)
        );
        Ok(())
    }

    #[test]
    fn streaming_transport_outlives_a_long_completion_while_non_streaming_stays_short()
    -> Result<(), Box<dyn Error>> {
        let profiles = P12TransportProfiles::try_new()?;
        let streaming = profiles
            .for_mode(ResponsesResponseMode::Streaming)
            .timeouts();
        let non_streaming = profiles
            .for_mode(ResponsesResponseMode::NonStreaming)
            .timeouts();

        assert_eq!(streaming.connect(), P12_CONNECT_TIMEOUT);
        assert_eq!(streaming.ttfb(), P12_STREAMING_TTFB_TIMEOUT);
        assert_eq!(streaming.idle(), P12_STREAMING_IDLE_TIMEOUT);
        assert_eq!(streaming.total(), P12_STREAMING_TOTAL_TIMEOUT);
        // The regression this guards: a 45-second absolute deadline truncated every longer
        // completion after its first bytes had already passed the unretryable client boundary.
        assert!(streaming.total() >= Duration::from_mins(30));
        // Semantic liveness sits between byte liveness and the absolute ceiling: generous enough
        // for one long healthy thinking stretch, small enough that a keepalive wedge cannot hold
        // the single P12 Credential for the whole ceiling.
        assert!(P12_STREAMING_PROGRESS_TIMEOUT > streaming.idle());
        assert!(P12_STREAMING_PROGRESS_TIMEOUT >= Duration::from_mins(10));
        assert!(P12_STREAMING_PROGRESS_TIMEOUT < streaming.total());
        // Even one keepalive per second sustained for the entire ceiling stays under the frame
        // budget, so the count-based bound cannot outrun the wall-clock deadline on any healthy
        // keepalive cadence; it exists to stop high-rate spam after bounded decode work.
        assert!(
            MAX_SSE_PROGRESS_FREE_FRAMES >= usize::try_from(P12_STREAMING_TOTAL_TIMEOUT.as_secs())?
        );
        assert!(streaming.idle() < streaming.total());

        assert_eq!(non_streaming.connect(), P12_CONNECT_TIMEOUT);
        assert_eq!(non_streaming.total(), P12_NON_STREAMING_TOTAL_TIMEOUT);
        assert!(non_streaming.total() < streaming.total());
        // A buffered upstream produces nothing until it finishes, so neither the first-byte nor
        // the response-idle bound may cut a non-streaming answer before its own total deadline.
        assert_eq!(non_streaming.ttfb(), non_streaming.total());
        assert_eq!(non_streaming.idle(), non_streaming.total());

        assert_ne!(
            profiles.for_mode(ResponsesResponseMode::Streaming),
            profiles.for_mode(ResponsesResponseMode::NonStreaming)
        );
        Ok(())
    }

    #[test]
    fn megabyte_scale_completions_fit_inside_the_streaming_and_non_streaming_bounds()
    -> Result<(), Box<dyn Error>> {
        const ONE_MEBIBYTE: usize = 1024 * 1024;

        // `response.output_text.done` and `response.completed` each repeat the entire answer in
        // one frame, so a megabyte of buffered residue must not be a protocol failure.
        let mut decoder = OpenAiSseDecoder::new(P12ResponseUsageProjection::OpenAiResponses);
        decoder.push_chunk(&vec![b'x'; ONE_MEBIBYTE])?;
        decoder.push_chunk(b"tail")?;
        assert_eq!(decoder.buffer.len(), ONE_MEBIBYTE + 4);

        let mut body = vec![b'y'; ONE_MEBIBYTE];
        append_response_chunk(&mut body, b"tail")?;
        assert_eq!(body.len(), ONE_MEBIBYTE + 4);
        Ok(())
    }

    #[test]
    fn an_oversized_non_streaming_body_is_rejected_without_buffer_growth() {
        let mut body = vec![b'y'; MAX_UPSTREAM_RESPONSE_BYTES];
        assert!(append_response_chunk(&mut body, b"y").is_err());
        assert_eq!(body.len(), MAX_UPSTREAM_RESPONSE_BYTES);
    }

    #[test]
    fn non_streaming_attempts_extend_their_start_ceiling_to_the_transport_total()
    -> Result<(), Box<dyn Error>> {
        let admitted_bootstrap =
            Duration::from_millis(u64::try_from(P12_BOOTSTRAP_TIMEOUT_MILLISECONDS)?);

        assert_eq!(
            p12_attempt_start_timeout(ResponsesResponseMode::Streaming, admitted_bootstrap),
            admitted_bootstrap
        );
        let non_streaming =
            p12_attempt_start_timeout(ResponsesResponseMode::NonStreaming, admitted_bootstrap);
        // The regression this guards: the orchestrator cut every non-streaming attempt at the
        // route bootstrap deadline, so the ten-minute transport total was unreachable.
        assert_eq!(
            non_streaming,
            admitted_bootstrap + P12_NON_STREAMING_TOTAL_TIMEOUT
        );
        assert!(non_streaming > admitted_bootstrap);
        Ok(())
    }

    struct LoopbackResolver;

    impl EgressDnsResolver for LoopbackResolver {
        fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
            Ok(vec![IpAddr::V4(Ipv4Addr::LOCALHOST)])
        }
    }

    fn live_admitted_target(port: u16) -> Result<AdmittedEgressTarget, Box<dyn Error>> {
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("p12-live-progress-policy")?,
            name: "P12 live progress test policy".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Http]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("relay.test")?]),
            allowed_ports: BTreeSet::from([port]),
            allowed_cidrs: BTreeSet::from([EgressCidr::try_new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                32,
            )?]),
            redirect_policy: RedirectPolicy::Deny,
        })?;
        Ok(policy.admit_url(
            &format!("http://relay.test:{port}/responses"),
            &LoopbackResolver,
        )?)
    }

    fn live_transport_request(
        target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, Box<dyn Error>> {
        Ok(UpstreamHttpRequest::try_new(
            target,
            UpstreamHttpMethod::Post,
            [("accept".to_owned(), "text/event-stream".to_owned())],
            br"{}".to_vec(),
        )?)
    }

    /// A live transport profile whose byte-idle bound is short enough that only frames arriving
    /// on the wire, never the test's own patience, keep it fresh.
    fn live_progress_test_profile() -> Result<UpstreamTransportProfile, Box<dyn Error>> {
        Ok(UpstreamTransportProfile::new(
            UpstreamTimeouts::try_new(
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(30),
            )?,
            UpstreamProxy::Direct,
            NonZeroUsize::new(1).ok_or("live pool needs one idle connection")?,
        ))
    }

    async fn write_all_to_peer(
        socket: &actix_web::rt::net::TcpStream,
        mut bytes: &[u8],
    ) -> std::io::Result<()> {
        while !bytes.is_empty() {
            socket.writable().await?;
            match socket.try_write(bytes) {
                Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
                Ok(written) => bytes = &bytes[written..],
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    async fn read_request_head(socket: &actix_web::rt::net::TcpStream) -> std::io::Result<()> {
        let mut head = Vec::new();
        loop {
            socket.readable().await?;
            let mut chunk = [0_u8; 1024];
            match socket.try_read(&mut chunk) {
                Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
                Ok(read) => {
                    head.extend_from_slice(&chunk[..read]);
                    if head.windows(4).any(|window| window == b"\r\n\r\n") {
                        return Ok(());
                    }
                    if head.len() > 65_536 {
                        return Err(std::io::ErrorKind::InvalidData.into());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
        }
    }

    /// Serves one live SSE response over loopback HTTP, then streams follow-up frames.
    ///
    /// Written through inherent `TcpStream` methods because this binary crate deliberately has
    /// no direct tokio dependency; `actix_web::rt` re-exports the runtime it already runs on.
    fn spawn_live_sse_peer(
        listener: actix_web::rt::net::TcpListener,
        prelude: String,
        follow_up_frame: String,
        follow_up_count: usize,
        follow_up_gap: Duration,
        epilogue: String,
    ) -> actix_web::rt::task::JoinHandle<std::io::Result<()>> {
        actix_web::rt::spawn(async move {
            let (socket, _) = listener.accept().await?;
            read_request_head(&socket).await?;
            write_all_to_peer(
                &socket,
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
            )
            .await?;
            write_all_to_peer(&socket, prelude.as_bytes()).await?;
            for _ in 0..follow_up_count {
                actix_web::rt::time::sleep(follow_up_gap).await;
                write_all_to_peer(&socket, follow_up_frame.as_bytes()).await?;
            }
            write_all_to_peer(&socket, epilogue.as_bytes()).await?;
            Ok(())
        })
    }

    #[test]
    fn a_pure_keepalive_stream_exhausts_the_progress_frame_budget_into_a_stream_error()
    -> Result<(), Box<dyn Error>> {
        let mut frames = vec![
            r#"{"type":"response.created","response":{"id":"response-p12-wedged"}}"#.to_owned(),
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"msg-wedged","type":"message","role":"assistant"}}"#.to_owned(),
            r#"{"type":"response.output_text.delta","item_id":"msg-wedged","delta":"ok"}"#.to_owned(),
        ];
        for _ in 0..=MAX_SSE_PROGRESS_FREE_FRAMES {
            frames.push(r#"{"type":"response.in_progress"}"#.to_owned());
        }
        let body = sse_stream_body(&frames);
        let reference = decode_sse_events(&body, body.len())?;

        assert_eq!(
            canonical_event_labels(&reference),
            vec![
                "response_start",
                "message_start",
                "text_delta",
                "stream_error"
            ]
        );
        assert!(matches!(
            reference.last(),
            Some(CanonicalEvent::StreamError(stream_error))
                if stream_error.error.code() == GatewayErrorCode::ProviderTransient
                    && stream_error.error.scope() == ErrorScope::Provider
        ));
        // BL-04: transport segmentation must change neither when the budget expires nor what
        // the terminated stream emitted.
        for chunk_size in [7, 4096] {
            assert_eq!(decode_sse_events(&body, chunk_size)?, reference);
        }
        Ok(())
    }

    #[test]
    fn comment_only_keepalive_frames_spend_the_same_progress_budget() -> Result<(), Box<dyn Error>>
    {
        let mut body = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-comment-wedged"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"msg-cw","type":"message","role":"assistant"}}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg-cw","delta":"ok"}"#,
        ]);
        for _ in 0..=MAX_SSE_PROGRESS_FREE_FRAMES {
            body.push_str(": keepalive\n\n");
        }
        let events = decode_sse_events(&body, 23)?;

        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::StreamError(stream_error))
                if stream_error.error.code() == GatewayErrorCode::ProviderTransient
        ));
        Ok(())
    }

    #[test]
    fn reasoning_summary_progress_refills_the_budget_between_keepalive_runs()
    -> Result<(), Box<dyn Error>> {
        // Three runs of keepalives, each one frame short of tripping the budget, separated by
        // reasoning-summary deltas: frames the decoder drops that still prove generation is
        // advancing.  A cumulative counter would fail this stream; only a consecutive one may.
        let mut frames = vec![
            r#"{"type":"response.created","response":{"id":"response-p12-thinking"}}"#.to_owned(),
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"rs-think","type":"reasoning"}}"#.to_owned(),
        ];
        for _ in 0..3 {
            for _ in 0..MAX_SSE_PROGRESS_FREE_FRAMES {
                frames.push(r#"{"type":"response.in_progress"}"#.to_owned());
            }
            frames.push(
                r#"{"type":"response.reasoning_summary_text.delta","item_id":"rs-think","delta":"…"}"#
                    .to_owned(),
            );
        }
        frames.extend([
            r#"{"type":"response.output_item.added","output_index":1,"item":{"id":"msg-think","type":"message","role":"assistant"}}"#.to_owned(),
            r#"{"type":"response.output_text.delta","item_id":"msg-think","delta":"ok"}"#.to_owned(),
            r#"{"type":"response.completed","response":{"id":"response-p12-thinking","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#.to_owned(),
        ]);
        let body = sse_stream_body(&frames);
        let events = decode_sse_events(&body, 4096)?;

        assert_eq!(
            canonical_event_labels(&events),
            vec![
                "response_start",
                "message_start",
                "text_delta",
                "message_end",
                "usage_delta",
                "response_end",
            ]
        );
        assert!(CanonicalResponse::try_new(events).is_ok());
        Ok(())
    }

    #[actix_web::test]
    async fn a_live_keepalive_only_stream_is_cut_by_the_progress_deadline_not_the_byte_idle_bound()
    -> Result<(), Box<dyn Error>> {
        let listener = actix_web::rt::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let prelude = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-live-keepalive"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"msg-live","type":"message","role":"assistant"}}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg-live","delta":"ok"}"#,
        ]);
        let keepalive = sse_stream_body(&[r#"{"type":"response.in_progress"}"#]);
        let server = spawn_live_sse_peer(
            listener,
            prelude,
            keepalive,
            200,
            Duration::from_millis(25),
            String::new(),
        );

        let response = UpstreamClientPool::new(NonZeroUsize::new(1).ok_or("live pool size")?)
            .send(
                live_transport_request(live_admitted_target(port)?)?,
                &live_progress_test_profile()?,
            )
            .await?;
        let mut source = OpenAiSseEventSource::begin_with_progress_deadline(
            response,
            P12ResponseUsageProjection::OpenAiResponses,
            Duration::from_millis(200),
        )
        .await
        .map_err(|_| std::io::Error::other("live SSE bootstrap failed"))?;

        let started = Instant::now();
        let error = loop {
            match source.next_event().await {
                Ok(Some(_)) => {}
                Ok(None) => {
                    return Err("keepalive-only stream ended without the progress failure".into());
                }
                Err(error) => break error,
            }
        };
        assert_eq!(
            (error.code(), error.scope()),
            (GatewayErrorCode::ProviderTransient, ErrorScope::Provider)
        );
        // The keepalives kept every transport deadline fresh, so only the progress deadline can
        // have fired -- and it must fire well before the transport's two-second byte-idle bound
        // would have had a first chance to see silence.
        assert!(started.elapsed() < Duration::from_secs(2));
        server.abort();
        Ok(())
    }

    #[actix_web::test]
    async fn a_stalled_downstream_client_does_not_spend_the_upstream_progress_budget()
    -> Result<(), Box<dyn Error>> {
        let listener = actix_web::rt::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let prelude = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-live-stall"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"msg-live","type":"message","role":"assistant"}}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg-live","delta":"ok"}"#,
        ]);
        let epilogue = sse_stream_body(&[
            r#"{"type":"response.completed","response":{"id":"response-p12-live-stall","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ]);
        let server = spawn_live_sse_peer(
            listener,
            prelude,
            String::new(),
            0,
            Duration::ZERO,
            epilogue,
        );

        let response = UpstreamClientPool::new(NonZeroUsize::new(1).ok_or("live pool size")?)
            .send(
                live_transport_request(live_admitted_target(port)?)?,
                &live_progress_test_profile()?,
            )
            .await?;
        let mut source = OpenAiSseEventSource::begin_with_progress_deadline(
            response,
            P12ResponseUsageProjection::OpenAiResponses,
            Duration::from_millis(200),
        )
        .await
        .map_err(|_| std::io::Error::other("live SSE bootstrap failed"))?;

        // Consume the first event, then stall far past the progress deadline without polling.
        // Only upstream-wait time may spend the budget: a client that stops reading freezes this
        // source through channel backpressure while the upstream stays healthy, so resuming must
        // find the remaining events instead of a fabricated wedge failure.
        assert!(source.next_event().await?.is_some());
        actix_web::rt::time::sleep(Duration::from_millis(600)).await;
        let mut events = Vec::new();
        while let Some(event) = source.next_event().await? {
            events.push(event);
        }
        assert!(events.iter().any(|event| matches!(
            event,
            CanonicalEvent::ResponseEnd(end) if end.stop_reason.as_deref() == Some("end_turn")
        )));
        server.abort();
        Ok(())
    }

    #[actix_web::test]
    async fn a_live_thinking_stream_survives_progress_deadlines_through_reasoning_summary_frames()
    -> Result<(), Box<dyn Error>> {
        let listener = actix_web::rt::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let prelude = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-live-thinking"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"rs-live","type":"reasoning"}}"#,
        ]);
        let summary_delta = sse_stream_body(&[
            r#"{"type":"response.reasoning_summary_text.delta","item_id":"rs-live","delta":"thinking"}"#,
        ]);
        let epilogue = sse_stream_body(&[
            r#"{"type":"response.output_item.added","output_index":1,"item":{"id":"msg-live","type":"message","role":"assistant"}}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg-live","delta":"ok"}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-live-thinking","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ]);
        // Thirty dropped-but-progress frames 40 ms apart accumulate 1.2 s of upstream wait --
        // past one deadline -- so each frame must restart the window for the stream to reach its
        // epilogue. The deadline sits 25x above the cadence so a loaded CI runner cannot fake a
        // wedge inside one gap.
        let server = spawn_live_sse_peer(
            listener,
            prelude,
            summary_delta,
            30,
            Duration::from_millis(40),
            epilogue,
        );

        let response = UpstreamClientPool::new(NonZeroUsize::new(1).ok_or("live pool size")?)
            .send(
                live_transport_request(live_admitted_target(port)?)?,
                &live_progress_test_profile()?,
            )
            .await?;
        let started = Instant::now();
        let mut source = OpenAiSseEventSource::begin_with_progress_deadline(
            response,
            P12ResponseUsageProjection::OpenAiResponses,
            Duration::from_secs(1),
        )
        .await
        .map_err(|_| std::io::Error::other("live SSE bootstrap failed"))?;

        let mut events = Vec::new();
        while let Some(event) = source.next_event().await? {
            events.push(event);
        }
        assert!(started.elapsed() >= Duration::from_secs(1));
        assert_eq!(
            canonical_event_labels(&events),
            vec![
                "response_start",
                "message_start",
                "text_delta",
                "message_end",
                "usage_delta",
                "response_end",
            ]
        );
        assert!(CanonicalResponse::try_new(events).is_ok());
        let _joined = server.await;
        Ok(())
    }

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

    fn p12_decoded_messages_http_state(
        events: Vec<CanonicalEvent>,
    ) -> Result<ResponsesHttpState, Box<dyn Error>> {
        let emissions = events
            .into_iter()
            .map(|event| DeterministicMockEmission::new(Duration::ZERO, event))
            .collect();
        let executor = DeterministicMockResponsesExecutor::try_new(
            ProviderId::try_new("p12-decoder-http-test-provider")?,
            emissions,
        )?;
        let client_key = InMemoryClientKey::try_new(
            "p12-decoder-http-test-key",
            ClientKeyId::try_new("p12-decoder-http-test-client")?,
            true,
        )?;
        let authenticator: Arc<dyn ClientKeyAuthenticator> =
            Arc::new(InMemoryClientKeyAuthenticator::try_new([client_key])?);

        Ok(ResponsesHttpState::new(
            Arc::new(executor),
            authenticator,
            default_stream_capacity()?,
        ))
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
    fn p12_attempt_stage_projection_is_terminal_bounded_and_value_free()
    -> Result<(), Box<dyn Error>> {
        let attempts = std::sync::Arc::new(P12AttemptStageStore::new());
        let request_id = RequestId::try_new("p12-stage-request")?;
        attempts.record_stage(&request_id, ManagementRequestAttemptStage::Decoder);
        let sink = P12AttemptEventSink::new(std::sync::Arc::clone(&attempts));
        let event = AttemptEvent::new(
            request_id.clone(),
            1,
            RouteId::try_new("p12-stage-route")?,
            RouteCandidateId::try_new("p12-stage-candidate")?,
            CredentialId::try_new("credential-must-not-appear")?,
            EndpointId::try_new("endpoint-must-not-appear")?,
            UpstreamId::try_new("upstream-must-not-appear")?,
            "model-must-not-appear".to_owned(),
            1,
            2,
            AttemptOutcome::Failed(GatewayError::new(
                GatewayErrorCode::UpstreamProtocolError,
                gateway_core::ErrorScope::Stream,
            )),
            AttemptRetryDecision::NonRetryable,
        );

        assert_eq!(
            sink.try_emit(GatewayEvent::Attempt(event)),
            EventEmission::Enqueued
        );
        let rows = attempts
            .list_request_attempts(&request_id)
            .map_err(|_| std::io::Error::other("attempt stage projection unavailable"))?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].outcome(), "failed");
        assert_eq!(
            rows[0].stage(),
            Some(ManagementRequestAttemptStage::Decoder)
        );
        assert!(rows[0].endpoint_id().is_none());
        assert!(rows[0].credential_id().is_none());
        let rendered = format!("{rows:?}");
        for forbidden in [
            "credential-must-not-appear",
            "endpoint-must-not-appear",
            "upstream-must-not-appear",
            "model-must-not-appear",
        ] {
            assert!(!rendered.contains(forbidden));
        }
        Ok(())
    }

    #[test]
    fn p12_attempt_stage_contention_fails_the_management_projection_closed()
    -> Result<(), Box<dyn Error>> {
        let attempts = P12AttemptStageStore::new();
        let request_id = RequestId::try_new("p12-stage-contention")?;
        let guard = attempts
            .records
            .lock()
            .map_err(|_| std::io::Error::other("attempt stage lock poisoned"))?;
        attempts.record_stage(
            &request_id,
            ManagementRequestAttemptStage::RequestConversion,
        );
        drop(guard);

        assert_eq!(
            attempts.list_request_attempts(&request_id),
            Err(ManagementRuntimeError::Unavailable)
        );
        Ok(())
    }

    #[test]
    fn p12_attempt_stage_capacity_fails_the_management_projection_closed()
    -> Result<(), Box<dyn Error>> {
        let attempts = P12AttemptStageStore::new();
        for index in 0..P12AttemptStageStore::MAX_RECORDS {
            let request_id = RequestId::try_new(format!("p12-stage-capacity-{index}"))?;
            attempts.record_stage(&request_id, ManagementRequestAttemptStage::HttpTransport);
        }
        let overflow = RequestId::try_new("p12-stage-capacity-overflow")?;
        attempts.record_stage(&overflow, ManagementRequestAttemptStage::HttpTransport);

        assert_eq!(
            attempts.list_request_attempts(&overflow),
            Err(ManagementRuntimeError::Unavailable)
        );
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
        assert_eq!(
            p12_response_usage_projection(&openai.request),
            P12ResponseUsageProjection::OpenAiResponses
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
        assert_eq!(
            p12_response_usage_projection(&anthropic),
            P12ResponseUsageProjection::AnthropicMessages
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
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::ResponseEnd(end)) if end.stop_reason.as_deref() == Some("tool_use")
        ));
        Ok(())
    }

    #[test]
    fn completed_non_streaming_response_seeds_anthropic_initial_usage_and_end_turn()
    -> Result<(), Box<dyn Error>> {
        let events = decode_json_events(
            br#"{
              "id":"response-p12-anthropic-lifecycle",
              "status":"completed",
              "output":[{
                "type":"message",
                "role":"assistant",
                "content":[{"type":"output_text","text":"ok"}]
              }],
              "usage":{
                "input_tokens":3,
                "output_tokens":5,
                "output_tokens_details":{"reasoning_tokens":2}
              }
            }"#,
        )?;
        let initial_usage_index = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    CanonicalEvent::UsageDelta(delta)
                        if !delta.is_final
                            && delta.usage.input_tokens == Some(3)
                            && delta.usage.output_tokens.is_none()
                            && delta.usage.reasoning_tokens.is_none()
                )
            })
            .ok_or("missing input-only initial usage")?;
        let message_start_index = events
            .iter()
            .position(|event| matches!(event, CanonicalEvent::MessageStart(_)))
            .ok_or("missing message start")?;
        let message_end_index = events
            .iter()
            .position(|event| matches!(event, CanonicalEvent::MessageEnd(_)))
            .ok_or("missing message end")?;
        let final_usage_index = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    CanonicalEvent::UsageDelta(delta)
                        if delta.is_final
                            && delta.usage.input_tokens == Some(3)
                            && delta.usage.output_tokens == Some(5)
                            && delta.usage.reasoning_tokens == Some(2)
                )
            })
            .ok_or("missing final usage")?;
        assert!(initial_usage_index < message_start_index);
        assert!(message_end_index < final_usage_index);
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::ResponseEnd(end)) if end.stop_reason.as_deref() == Some("end_turn")
        ));
        Ok(())
    }

    #[actix_web::test]
    async fn p12_decoded_completed_response_is_encodable_by_the_anthropic_messages_boundary()
    -> Result<(), Box<dyn Error>> {
        let events = decode_json_events_with_usage_projection(
            br#"{
              "id":"response-p12-anthropic-http",
              "status":"completed",
              "output":[{
                "type":"message",
                "role":"assistant",
                "content":[{"type":"output_text","text":"ok"}]
              }],
              "usage":{
                "input_tokens":3,
                "output_tokens":5,
                "output_tokens_details":{"reasoning_tokens":2}
              }
            }"#,
            P12ResponseUsageProjection::AnthropicMessages,
        )?;
        let app = actix_test::init_service(
            App::new()
                .app_data(web::Data::new(p12_decoded_messages_http_state(events)?))
                .configure(configure),
        )
        .await;
        let request = actix_test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header((header::AUTHORIZATION, "Bearer p12-decoder-http-test-key"))
            .set_payload(
                r#"{"model":"p12-decoder-http-model","max_tokens":1,"messages":[{"role":"user","content":"ok"}]}"#,
            )
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(&actix_test::read_body(response).await)?;
        assert_eq!(
            body.pointer("/stop_reason").and_then(Value::as_str),
            Some("end_turn")
        );
        assert_eq!(
            body.pointer("/usage/input_tokens").and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            body.pointer("/usage/output_tokens").and_then(Value::as_u64),
            Some(5)
        );
        Ok(())
    }

    #[actix_web::test]
    async fn p12_decoded_completed_response_remains_encodable_by_the_openai_responses_boundary()
    -> Result<(), Box<dyn Error>> {
        let events = decode_json_events(
            br#"{
              "id":"response-p12-openai-http",
              "status":"completed",
              "output":[{
                "type":"message",
                "role":"assistant",
                "content":[{"type":"output_text","text":"ok"}]
              }],
              "usage":{
                "input_tokens":3,
                "output_tokens":5,
                "output_tokens_details":{"reasoning_tokens":2}
              }
            }"#,
        )?;
        let app = actix_test::init_service(
            App::new()
                .app_data(web::Data::new(p12_decoded_messages_http_state(events)?))
                .configure(configure),
        )
        .await;
        let request = actix_test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header((header::AUTHORIZATION, "Bearer p12-decoder-http-test-key"))
            .set_payload(r#"{"model":"p12-decoder-http-model","input":"ok"}"#)
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(&actix_test::read_body(response).await)?;
        assert_eq!(
            body.pointer("/status").and_then(Value::as_str),
            Some("completed")
        );
        assert_eq!(
            body.pointer("/usage/input_tokens").and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            body.pointer("/usage/output_tokens").and_then(Value::as_u64),
            Some(5)
        );
        assert_eq!(
            body.pointer("/usage/output_tokens_details/reasoning_tokens")
                .and_then(Value::as_u64),
            Some(2)
        );
        Ok(())
    }

    #[actix_web::test]
    async fn p12_decoded_tool_completion_is_encodable_by_the_anthropic_messages_boundary()
    -> Result<(), Box<dyn Error>> {
        let events = decode_json_events_with_usage_projection(
            br#"{
              "id":"response-p12-tool-http",
              "status":"completed",
              "output":[{
                "type":"function_call",
                "call_id":"call-p12-tool-http",
                "name":"echo",
                "arguments":"{\"value\":\"ok\"}"
              }],
              "usage":{
                "input_tokens":3,
                "output_tokens":5,
                "output_tokens_details":{"reasoning_tokens":2}
              }
            }"#,
            P12ResponseUsageProjection::AnthropicMessages,
        )?;
        let app = actix_test::init_service(
            App::new()
                .app_data(web::Data::new(p12_decoded_messages_http_state(events)?))
                .configure(configure),
        )
        .await;
        let request = actix_test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header((header::AUTHORIZATION, "Bearer p12-decoder-http-test-key"))
            .set_payload(
                r#"{
                  "model":"p12-decoder-http-model",
                  "max_tokens":1,
                  "messages":[{"role":"user","content":"ok"}],
                  "tools":[{"name":"echo","input_schema":{"type":"object"}}]
                }"#,
            )
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(&actix_test::read_body(response).await)?;
        assert_eq!(
            body.pointer("/stop_reason").and_then(Value::as_str),
            Some("tool_use")
        );
        assert_eq!(
            body.pointer("/content/0/type").and_then(Value::as_str),
            Some("tool_use")
        );
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
    fn oversized_sse_frame_is_rejected_without_buffer_growth() -> Result<(), Box<dyn Error>> {
        let mut decoder = OpenAiSseDecoder::new(P12ResponseUsageProjection::OpenAiResponses);
        decoder.push_chunk(&vec![b'x'; MAX_SSE_FRAME_BYTES])?;
        assert!(decoder.push_chunk(b"y").is_err());
        assert_eq!(decoder.buffer.len(), MAX_SSE_FRAME_BYTES);
        Ok(())
    }

    #[test]
    fn sse_frame_budget_counts_only_the_undecoded_residue() -> Result<(), Box<dyn Error>> {
        const HALF_FRAME: usize = MAX_SSE_FRAME_BYTES / 2;

        let mut decoder = OpenAiSseDecoder::new(P12ResponseUsageProjection::OpenAiResponses);
        let mut first = b": keep-alive\n\n".to_vec();
        first.extend_from_slice(&vec![b'x'; HALF_FRAME]);
        decoder.push_chunk(&first)?;
        // The comment frame decodes to nothing, leaving dead bytes ahead of a large open frame.
        decoder.drain_buffered_frames()?;
        assert!(decoder.consumed > 0);

        // A full frame bound of live bytes must still be admitted despite the decoded prefix...
        decoder.push_chunk(&vec![b'x'; MAX_SSE_FRAME_BYTES - HALF_FRAME])?;
        // ...while the first byte past the live bound is rejected and the buffer stays bounded.
        assert!(decoder.push_chunk(b"y").is_err());
        assert!(decoder.buffer.len() <= MAX_SSE_FRAME_BYTES * 2);
        Ok(())
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
