//! P12's explicitly bounded production data-plane composition.
//!
//! The deployment process is deliberately narrower than the test-only P3 harness: it admits the
//! reviewed production graph shape -- any number of Endpoints, weighted encrypted Credential
//! bindings, aliases, public models, and Client Keys -- while every Endpoint must declare an
//! `api_format` this build binds an adapter for and every Candidate the Canonical transform
//! (`CR-P12-ROLLOUT-001`).  Each Endpoint is bound to its adapter once, at composition, from the
//! same Config Version and `RouteSnapshot` the executor pins.  It pins the encrypted Credential
//! pools to the active Snapshot and fails closed after a management publication until the isolated
//! process restarts, so a new `RouteSnapshot` can never use an old runtime pool, and a graph
//! declaring a format this build cannot serve fails admission instead of being silently skipped.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    fmt,
    num::NonZeroUsize,
    path::Path,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::future::BoxFuture;

use gateway_auth::client_key::ClientKeyService;
use gateway_catalog::{
    CapabilitySet, CatalogView, EndpointCapabilityEntry, EndpointCapabilityView, SemanticCapability,
};
use gateway_control::{
    compatible_egress_runtime_compiler::{
        CompatibleEgressRuntimeCompiler, CompatibleEndpointBindingRuntimeSettings,
    },
    control_plane_service::open_compatible_proxy_node_endpoint,
    credential_pool_compiler::CredentialPoolCompiler,
    egress_policy_compiler::EgressPolicyCompiler,
    management_mutation_service::ConfigRevision,
    provider_account_pool_service::{
        ProviderAccountAuthStatus, ProviderAccountPoolFacade, ProviderAccountRuntimeStatus,
        RejectingProviderAccountPoolFacade,
    },
    route_compiler::RouteCompiler,
    routing_price_policy_service::{RoutingPriceSnapshot, compile_routing_price_snapshot},
};
use gateway_core::{
    AttemptEvent, AttemptOutcome, CanonicalEvent, CanonicalEventState, CanonicalMessage,
    CanonicalRequest, CredentialId, EgressPolicyId, EndpointId, ErrorScope, EventEmission,
    GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink, MessageContent, MessageRole,
    NoopGatewayEventSink, ProviderId, RawExtensions, RawJson, RequestContext, RequestId, RouteId,
    TextContent, TransparentRetryGate, TransparentRetryGateFuture, Usage, UsageDelta,
};
#[cfg(test)]
use gateway_core::{
    CanonicalResponse, MessageEnd, MessageStart, ResponseEnd, ResponseId, ResponseStart,
    StreamError, TextDelta, ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart,
};
use gateway_http_actix::{
    ResponsesHttpState, SystemResponsesMetadataFactory, default_stream_capacity,
    management_observability_resources::{
        DurabilityMetricsSource, ManagementObservabilityHttpState,
    },
    management_resources::{
        ManagementCatalogStatus, ManagementChannelPinError, ManagementChannelPinFacade,
        ManagementChannelPinFuture, ManagementChannelPinMode, ManagementChannelPinOutcome,
        ManagementChannelPinReceipt, ManagementChannelPinRequest, ManagementQuotaRecoveryState,
        ManagementRequestAttempt, ManagementRequestAttemptStage, ManagementRequestProtocol,
        ManagementRouteExplain, ManagementRouteExplainCandidate, ManagementRouteExplainPricePolicy,
        ManagementRouteExplainRequest, ManagementRuntimeAvailabilityStatus, ManagementRuntimeError,
        ManagementRuntimeFacade, ManagementRuntimeTarget, RejectingManagementChannelPinFacade,
    },
};
use gateway_observability::{
    BoundedEventQueue, EventQueueConfig, NoopOpenTelemetryExporter, PrometheusMetrics,
    TelemetryPipeline, TracingJsonExporter,
};
use gateway_protocol::{ApiFormat, ApiFormatAdapterRegistry};
use gateway_router::{
    AttemptDriver, AttemptExclusionSet, AttemptFailure, AttemptFuture, AttemptOrchestrator,
    AttemptOrchestratorConfig, CompatibleEgressNodeInput, CompatibleEgressTransportRegistry,
    CompatibleEgressTransportRegistryInput, CompatibleEndpointEgressLease,
    CompatibleEndpointRuntime, CompatibleFixedProxyInput, CompatibleProxyPoolInput,
    DEFAULT_TRANSIENT_COOLDOWN, NativePayloadAvailability, ProjectedProtocolRequest,
    ProtocolFormat, ProtocolResponseProjector, ProtocolTransformInput, ProtocolTransformRejection,
    ProviderScopedPriceEvidence, ProviderScopedRouteExplainInput,
    ProviderScopedRouteExplainSnapshot, QuotaConfidence, QuotaSnapshot, QuotaSource,
    ResponsesClientTransport, ResponsesEventSource, ResponsesExecution, ResponsesExecutionLineage,
    ResponsesExecutor, ResponsesFuture, ResponsesResponseMode, RouteCredentialScheduler,
    RouteExplainCandidate, RouteExplainCandidateReason, RouteExplainInput, RouteSnapshot,
    RouteSnapshotRegistry, RuntimeCredentialAccountStatus, RuntimeHealthAccountRecoveryResult,
    RuntimeHealthRegistry, RuntimeQuotaAvailability, RuntimeQuotaRegistry, RuntimeQuotaTarget,
    SelectedRouteCredential, SnapshotRouteCandidate, SnapshotVersion, SystemRuntimeHealthClock,
    project_registered_protocol_request, protocol_pair_is_publishable,
};
use gateway_store::{
    control_plane::{
        CompatibleEgressTargetConfiguration, ConfigVersionStatus, ControlPlaneConfiguration,
        CredentialScope, CredentialStatus, EndpointConfiguration, EndpointTransport, RoutePolicy,
        RoutingPriceComparison, SqliteControlPlaneRepository, StoredClientKeyStatus,
        StoredCompatibleFailureScope, StoredCompatibleStickiness, StoredEgressRedirectMode,
        TransformMode,
    },
    event_store::{
        AsyncSqliteEventWriter, EventWriterConfig, EventWriterMetricsHandle, SqliteEventStore,
    },
    secret_store::SecretStore,
    stored_response::SqliteStoredResponseStore,
};
use gateway_upstream::{
    AdmittedEgressTarget, CompatibleEgressTarget, CompatibleFailureScope, CompatibleRetryPolicy,
    CompatibleStickiness, CredentialLease, EgressCidr, EgressDnsResolver, EgressHost, EgressPolicy,
    EgressPolicyInput, EgressScheme, EndpointCredentialPools, RedirectPolicy,
    SystemEgressDnsResolver, UpstreamClientPool, UpstreamHttpMethod, UpstreamHttpRequest,
    UpstreamHttpResponse, UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};

use protocol_openai_chat::{
    OpenAiChatSseDecoder, ResponseMode as ChatResponseMode,
    decode_upstream_response as decode_chat_upstream_response,
};
use protocol_openai_responses::{
    OpenAiResponsesSseDecoder, ResponseMode,
    decode_upstream_response_with_reasoning_policy as decode_responses_upstream_response_with_reasoning_policy,
};
use provider_anthropic_compatible::{
    AnthropicMessagesEndpoint, AnthropicMessagesOutboundRequest, AnthropicMessagesRequestBuilder,
    AnthropicMessagesSseDecoder, AnthropicResponseMode, AnthropicRuntimeFailureAction,
    ClaudeRuntimeCredential, classify_anthropic_runtime_failure, decode_upstream_response,
};
use provider_grok::{
    GROK_BUILD_RESPONSES_BASE_URL, GROK_BUILD_RESPONSES_PATH, GROK_BUILD_RESPONSES_URL,
    GROK_CONSOLE_RESPONSES_BASE_URL, GROK_CONSOLE_RESPONSES_PATH, GROK_CONSOLE_RESPONSES_URL,
    GROK_OFFICIAL_API_BASE_URL, GROK_OFFICIAL_RESPONSES_PATH, GROK_OFFICIAL_RESPONSES_URL,
    GROK_WEB_CANARY_PATH, GROK_WEB_CANARY_URL, GROK_WEB_PRODUCTION_BASE_URL,
    GROK_WEB_PRODUCTION_USER_AGENT, GrokAccountAuthStatus, GrokAccountEndpointBinding,
    GrokAccountMetadata, GrokAccountPoolStore, GrokAccountProvider, GrokBuildCredential,
    GrokBuildExecutionMode, GrokBuildInferenceAdapter, GrokBuildUpstreamTransport,
    GrokConsoleExecutionMode, GrokConsoleInferenceAdapter, GrokConsoleSsoToken,
    GrokConsoleUpstreamTransport, GrokOfficialApiKey, GrokOfficialExecutionMode,
    GrokOfficialInferenceAdapter, GrokOfficialUpstreamTransport, GrokWebBrowserEgressSession,
    GrokWebBrowserUserAgent, GrokWebCredential, GrokWebEgressRefresher, GrokWebEgressSessionId,
    GrokWebFlareSolverrRequest, GrokWebFlareSolverrTransport, GrokWebFlareSolverrTransportResponse,
    GrokWebProductionInferenceAdapter, GrokWebProductionUpstreamTransport, GrokWebStatsigRuntime,
    GrokWebStatsigUpstreamTransport, GrokWebTlsProfile,
};
use provider_kiro::{
    CanonicalEventSource, InferenceAdapter,
    conversation_request::{KiroConversationContext, KiroConversationId, KiroEnvironmentState},
    credential::KiroCredential,
    endpoint_policy::{KiroApiRegion, KiroEndpointKind, KiroEndpointPolicy},
    inference::{KiroInferenceAdapter, KiroUpstreamTransport},
    profile_arn::{KiroEnterpriseProfileLookup, KiroProfileArnError, resolve_profile_arn},
};
use provider_openai_compatible::{
    OpenAiChatCompletionsApiKey, OpenAiChatCompletionsEndpoint,
    OpenAiChatCompletionsRequestBuilder, OpenAiCompatibleRuntimeCredential, OpenAiResponsesApiKey,
    OpenAiResponsesEndpoint, OpenAiResponsesOutboundRequest, OpenAiResponsesRequestBuilder,
    OpenAiRuntimeFailureAction, classify_openai_runtime_failure,
};
use serde_json::Value;

use crate::provider_account_pool_adapter::{
    ProviderAccountDescriptor, ProviderAccountDescriptorSource, ProviderAccountPoolAdapter,
    SystemProviderAccountPoolClock,
};

/// The largest complete non-streaming Responses body this runtime will buffer.
///
/// A completed Responses envelope carries the entire output text, every tool-call argument string,
/// and any reasoning item in one JSON document. Current models emit up to 128k output tokens, which
/// is roughly 0.5-2 MiB of UTF-8 once JSON escaping and the envelope are counted, so the previous
/// 64 KiB bound (about 16k tokens of ASCII) rejected ordinary long answers. Admission caps the
/// total Credential concurrency at [`P12_MAX_TOTAL_BINDING_CONCURRENCY`], so the worst-case
/// resident bodies stay at that many buffers.
const MAX_UPSTREAM_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Maximum error envelope inspected for structured provider ownership signals.
const MAX_PROVIDER_ERROR_BODY_BYTES: usize = 64 * 1024;

// grok2api 3.1.1 bounds the browser-solver envelope at 2 MiB.  The response is still read only
// from the fixed loopback endpoint and the provider parser retains only allowlisted cookies.
const MAX_FLARESOLVERR_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const GROK_WEB_STATSIG_SIGNER_URL_ENV: &str = "CPAR_GROK_WEB_STATSIG_SIGNER_URL";
const GROK_WEB_BROWSER_RELAY_URL_ENV: &str = "CPAR_GROK_WEB_BROWSER_RELAY_URL";

/// Loopback-only `FlareSolverr` transport used by the native Web recovery hook.
#[derive(Clone)]
struct P12GrokWebFlareSolverrTransport {
    policy: Arc<EgressPolicy>,
    client_pool: Arc<UpstreamClientPool>,
    profile: UpstreamTransportProfile,
    proxy_url: Option<String>,
    port: u16,
}

#[derive(Clone)]
struct P12GrokWebEgressRefresher {
    transport: Arc<dyn GrokWebFlareSolverrTransport>,
}

impl GrokWebEgressRefresher for P12GrokWebEgressRefresher {
    fn refresh<'a>(
        &'a self,
        current: &'a GrokWebBrowserEgressSession,
    ) -> BoxFuture<'a, Result<Arc<GrokWebBrowserEgressSession>, GatewayError>> {
        let session_id = current.egress_session_id().as_str().to_owned();
        let tls_profile = current.tls_profile().clone();
        let proxy = current.proxy().clone();
        let credential = current.credential_snapshot();
        let transport = Arc::clone(&self.transport);
        Box::pin(async move {
            let now_ms = system_now_ms().map_err(|_| internal_error())?;
            let response = transport
                .send(GrokWebFlareSolverrRequest::default())
                .await?;
            if !(200..=299).contains(&response.status()) {
                return Err(internal_error());
            }
            let clearance =
                provider_grok::GrokWebFlareSolverrClearance::parse(&response.into_body())
                    .map_err(|_| internal_error())?;
            let credential = credential
                .with_flaresolverr_clearance(&clearance, now_ms)
                .map_err(|_| credential_unavailable_error())?;
            let session = GrokWebBrowserEgressSession::try_new(
                GrokWebEgressSessionId::try_new(&session_id)
                    .map_err(|_| credential_unavailable_error())?,
                credential,
                GrokWebBrowserUserAgent::try_new(clearance.user_agent())
                    .map_err(|_| internal_error())?,
                tls_profile,
                proxy,
                now_ms,
            )
            .map_err(|_| credential_unavailable_error())?;
            Ok(Arc::new(session))
        })
    }
}

impl P12GrokWebFlareSolverrTransport {
    fn new(
        client_pool: Arc<UpstreamClientPool>,
        profile: UpstreamTransportProfile,
        proxy_url: Option<String>,
        port: u16,
    ) -> Result<Self, GatewayError> {
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("grok-web-flaresolverr-loopback")
                .map_err(|_| internal_error())?,
            name: "Grok Web FlareSolverr loopback".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Http]),
            allowed_hosts: BTreeSet::from([
                EgressHost::try_new("127.0.0.1").map_err(|_| internal_error())?
            ]),
            allowed_ports: BTreeSet::from([port]),
            allowed_cidrs: BTreeSet::from([EgressCidr::try_new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                32,
            )
            .map_err(|_| internal_error())?]),
            redirect_policy: RedirectPolicy::Deny,
        })
        .map_err(|_| internal_error())?;
        Ok(Self {
            policy: Arc::new(policy),
            client_pool,
            profile,
            proxy_url,
            port,
        })
    }
}

impl GrokWebFlareSolverrTransport for P12GrokWebFlareSolverrTransport {
    fn send(
        &self,
        request: GrokWebFlareSolverrRequest,
    ) -> BoxFuture<'_, Result<GrokWebFlareSolverrTransportResponse, GatewayError>> {
        let policy = Arc::clone(&self.policy);
        let pool = Arc::clone(&self.client_pool);
        let profile = self.profile.clone();
        let port = self.port;
        let request = match self.proxy_url.as_deref() {
            Some(proxy_url) => request.with_proxy_url(proxy_url),
            None => request,
        };
        Box::pin(async move {
            let target_url = format!("http://127.0.0.1:{port}/v1");
            let target = policy
                .admit_url(&target_url, &SystemEgressDnsResolver)
                .map_err(|_| internal_error())?;
            let body = request.to_json().map_err(|_| internal_error())?;
            let outbound = UpstreamHttpRequest::try_new(
                target,
                UpstreamHttpMethod::Post,
                [("content-type".to_owned(), "application/json".to_owned())],
                body,
            )
            .map_err(|_| internal_error())?;
            let mut response = pool.send(outbound, &profile).await?;
            let status = response.status();
            let mut body = Vec::new();
            while let Some(chunk) = response.next_chunk().await? {
                if body.len().saturating_add(chunk.len()) > MAX_FLARESOLVERR_RESPONSE_BYTES {
                    return Err(internal_error());
                }
                body.extend_from_slice(&chunk);
            }
            GrokWebFlareSolverrTransportResponse::new(status, body).map_err(|_| internal_error())
        })
    }
}
/// The largest undelivered SSE residue this runtime will buffer between two canonical events.
///
/// `response.output_text.done`, `response.output_item.done`, and `response.completed` each repeat
/// the whole accumulated output inside a single frame, so this bound must match the complete-body
/// bound rather than the size of one delta.
#[cfg(test)]
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
/// its leased Credential capacity frees after at most this deadline plus one idle window,
/// instead of after the full absolute ceiling.
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
/// The maximum idle interval allowed while draining a management Channel Pin source.
///
/// This intentionally remains shorter than the production streaming allowance: Channel Pin is a
/// bounded diagnostic for a fixed short probe, not a user-facing long-running generation.
const P13_CHANNEL_PIN_IDLE_TIMEOUT: Duration = Duration::from_secs(45);
/// The absolute wall-clock ceiling for draining one management Channel Pin source.
const P13_CHANNEL_PIN_TOTAL_TIMEOUT: Duration = Duration::from_secs(45);
/// Maximum number of concurrent management pins. A pin can hold a serving lease for the full
/// bounded drain window, so admission must be bounded independently of ordinary route capacity.
const P13_CHANNEL_PIN_MAX_IN_FLIGHT: usize = 2;
/// The isolated P12 streaming decoder retains at most this many Tool argument bytes per response.
///
/// This must admit exactly what the non-streaming decoder admits: there, every Tool argument
/// string arrives inside the one complete body, so the effective bound is the complete-body bound.
/// A smaller streaming bound would reject a response the same upstream serves successfully in the
/// other mode, and would do so after `ToolCallStart` already crossed the unretryable boundary.
#[cfg(test)]
const MAX_SSE_TOOL_ARGUMENT_BYTES: usize = MAX_UPSTREAM_RESPONSE_BYTES;
/// The isolated P12 streaming decoder admits at most this many Tool calls in one response.
#[cfg(test)]
const MAX_SSE_TOOL_CALLS: usize = 32;
/// The longest output-item or call identifier the streaming decoder retains per Tool call.
///
/// Real Responses implementations emit `fc_`-prefixed item identifiers and `call_`-prefixed
/// call identifiers of roughly 30-70 bytes with no documented upper bound, so 256 bytes is
/// safely generous. Without this bound each retained identifier is limited only by the frame
/// bound, letting one response pin [`MAX_SSE_TOOL_CALLS`] identifiers of up to
/// [`MAX_SSE_FRAME_BYTES`] each -- hundreds of mebibytes of state the bounded-buffer baseline
/// forbids.
#[cfg(test)]
const MAX_SSE_IDENTIFIER_BYTES: usize = 256;
/// The longest run of consecutive progress-free SSE frames the streaming decoder tolerates.
///
/// This is the clock-free complement of [`P12_STREAMING_PROGRESS_TIMEOUT`], enforced inside the
/// decoder where chunk boundaries cannot influence it: the run advances only per decoded frame,
/// never per transport read. It is sized so no plausible keepalive cadence reaches it before the
/// wall-clock deadline -- even one keepalive per second sustained for the whole absolute ceiling
/// stays under it -- while a keepalive spam loop is stopped after a bounded amount of decode
/// work instead of burning CPU until a timer expires.
#[cfg(test)]
const MAX_SSE_PROGRESS_FREE_FRAMES: usize = 4096;
/// The four JSON insignificant whitespace characters used to frame assembled Tool arguments.
#[cfg(test)]
const JSON_WHITESPACE: [char; 4] = [' ', '\t', '\n', '\r'];
const P12_BOOTSTRAP_TIMEOUT_MILLISECONDS: i64 = 15_000;
/// The largest per-Route transparent attempt budget this composition admits.
///
/// Every attempt under this budget is pre-first-byte: the orchestrator never retries once a
/// first semantic event exists, so this bound only caps how much sequential pre-header failover
/// one request may buy. Five attempts cover the deepest reviewed production graph (two Endpoint
/// Candidates, one of them holding three weighted Credentials) while the Route's bootstrap
/// deadline, admitted at no more than [`P12_BOOTSTRAP_TIMEOUT_MILLISECONDS`], still bounds the
/// whole pre-first-byte window regardless of the attempt count.
const P12_MAX_ROUTE_ATTEMPTS: usize = 5;
/// The largest total Credential concurrency this composition admits across all bindings.
///
/// Each concurrently leased attempt may buffer one complete non-streaming body or one SSE frame
/// of up to [`MAX_UPSTREAM_RESPONSE_BYTES`], so this cap keeps the worst-case resident upstream
/// body memory at sixteen such buffers (128 MiB), alongside the data listener's own bounded
/// inbound request memory.
const P12_MAX_TOTAL_BINDING_CONCURRENCY: i64 = 16;
const P12_ANTHROPIC_MAX_TOKENS_EXTENSION: &str = "anthropic.messages.max_tokens";
const P12_OPENAI_CHAT_MAX_TOKENS_EXTENSION: &str = "openai.chat.max_tokens";
const P12_OPENAI_MAX_OUTPUT_TOKENS_EXTENSION: &str = "openai.responses.max_output_tokens";
/// The one verified, non-secret Krill/Codex compatibility header for P12's isolated endpoint.
///
/// This stays in the P12 runtime instead of changing the generic OpenAI-compatible provider:
/// other Providers retain their existing three-header contract.
const P12_KRILL_COMPATIBILITY_USER_AGENT: &str = "codex_cli_rs/0.139.0";
const P12_CODEX_OAUTH_USER_AGENT: &str = "codex_cli_rs/0.144.1 (Linux; arm64)";
const P12_CODEX_OAUTH_ORIGINATOR: &str = "codex_cli_rs";
const P12_CODEX_OAUTH_VERSION: &str = "0.144.1";
/// Lifetime of one operator-driven recovery ticket; begin and complete happen in one call.
const P12_OPERATOR_RECOVERY_TTL_MS: i64 = 30_000;
/// Short read-model lifetime: long enough for bounded cursor pagination, short enough that
/// lease/Health/Quota observations do not masquerade as durable configuration state.
const P13_PROVIDER_ACCOUNT_POOL_SNAPSHOT_TTL: Duration = Duration::from_secs(5);
/// Cursor-bearing readers may finish a bounded multi-page traversal after the latest view refreshes.
const P13_PROVIDER_ACCOUNT_POOL_CURSOR_RETENTION: Duration = Duration::from_mins(2);
static P13_CHANNEL_PIN_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
static P13_CHANNEL_PIN_BOOT_NONCE: OnceLock<Result<String, ()>> = OnceLock::new();

fn p13_channel_pin_request_id() -> Result<RequestId, ManagementChannelPinError> {
    let boot_nonce = P13_CHANNEL_PIN_BOOT_NONCE.get_or_init(|| {
        let mut bytes = [0_u8; 8];
        getrandom::fill(&mut bytes).map_err(|_| ())?;
        let mut rendered = String::with_capacity(bytes.len().saturating_mul(2));
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut rendered, "{byte:02x}").map_err(|_| ())?;
        }
        Ok(rendered)
    });
    let boot_nonce = boot_nonce
        .as_deref()
        .map_err(|()| ManagementChannelPinError::Unavailable)?;
    RequestId::try_new(format!(
        "channel-pin-{boot_nonce}-{}",
        P13_CHANNEL_PIN_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
    .map_err(|_| ManagementChannelPinError::Unavailable)
}

/// Production pieces that must be attached to the separate P12 listeners together.
pub(crate) struct DataPlaneComposition {
    /// Authenticated data-plane state for the loopback data listener.
    pub(crate) data: ResponsesHttpState,
    /// Management projection backed by the Snapshot registry, durable event log, and stage ledger.
    pub(crate) management_runtime: Box<dyn ManagementRuntimeFacade>,
    /// Read-only projection over the exact account pools and runtime registries used by routing.
    pub(crate) provider_account_pools: Box<dyn ProviderAccountPoolFacade>,
    /// Management-only one-shot Channel Pin executor sharing the serving pools and snapshot.
    pub(crate) channel_pin: Box<dyn ManagementChannelPinFacade>,
    /// Management-listener exposition over the shared bounded telemetry registry.
    pub(crate) observability: ManagementObservabilityHttpState,
    /// Durable event consumer that the deployment envelope spawns after its listeners bind and
    /// joins with a bounded wait after they stop.
    pub(crate) event_writer: AsyncSqliteEventWriter,
}

type ProviderAccountPoolComposition = (
    Arc<dyn ResponsesExecutor>,
    Box<dyn ProviderAccountPoolFacade>,
    Option<Arc<RouteCredentialScheduler>>,
    Box<dyn ManagementChannelPinFacade>,
);
type P12ExecutorComposition = (
    P12RoutedResponsesExecutor,
    Box<dyn ProviderAccountPoolFacade>,
    Arc<RouteCredentialScheduler>,
);

/// Returns the fixed compiler evidence for every Endpoint stored in the control database.
///
/// Capabilities come only from the reviewed adapter ledger below, never from a credential,
/// upstream response, model name, or operator-supplied free-form claim. Endpoint and Candidate
/// administrative state remains a separate scheduling gate: a disabled record cannot become
/// eligible merely because its implementation has a capability profile. Reusing one Endpoint
/// identity for two adapters across stored Versions fails closed: a restart must never silently
/// reinterpret a rollback identity under a different capability set.
/// The profiles cover the union of Endpoints across every stored Config Version so the bootstrap
/// Snapshot, its rollback predecessor, and staged drafts compile from one immutable view.
pub(crate) fn deployment_route_compiler(
    database: &Path,
) -> Result<RouteCompiler, RuntimeCompositionError> {
    let mut repository = SqliteControlPlaneRepository::open(database)
        .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::ControlPlane))?;
    let versions = repository
        .list_config_versions()
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
    let mut endpoint_capabilities = BTreeMap::new();
    for version in versions {
        let Some(configuration) = repository
            .load_configuration(&version.id)
            .map_err(|_| RuntimeCompositionError::Unavailable)?
        else {
            continue;
        };
        for endpoint in configuration.endpoints {
            let capabilities = p12_adapter_capabilities(&endpoint.adapter_id)?;
            if let Some((existing_adapter, existing_capabilities)) =
                endpoint_capabilities.get(&endpoint.id)
            {
                if existing_adapter != &endpoint.adapter_id
                    || existing_capabilities != &capabilities
                {
                    return Err(RuntimeCompositionError::Unavailable);
                }
            } else {
                endpoint_capabilities.insert(endpoint.id, (endpoint.adapter_id, capabilities));
            }
        }
    }
    let capabilities = EndpointCapabilityView::try_new(endpoint_capabilities.into_iter().map(
        |(endpoint_id, (_adapter_id, capabilities))| EndpointCapabilityEntry {
            endpoint_id,
            capabilities,
        },
    ))
    .map_err(|_| RuntimeCompositionError::Unavailable)?;
    Ok(RouteCompiler::new(CatalogView::default(), capabilities))
}

/// Returns the conservative semantic capabilities proved by this build for one adapter.
///
/// This table is intentionally narrower than protocol syntax. JSON Schema accompanies only the
/// Tool-capable adapters whose typed builders validate Tool input schemas; Vision stays absent
/// without provider-level evidence. Grok Web is a recognized product channel but deliberately has
/// a text-only streaming capability; Tool, Reasoning, and Vision remain absent. Unknown
/// implementation labels fail here.
fn p12_adapter_capabilities(adapter_id: &str) -> Result<CapabilitySet, RuntimeCompositionError> {
    use SemanticCapability::{
        JsonSchema, ParallelTools, Reasoning, ResponseCompaction, ResponsesWebSocket,
        StoredResponses, Streaming, Tools,
    };

    let capabilities: &[SemanticCapability] = match adapter_id {
        "openai-compatible.chat-completions" => &[
            Tools,
            ParallelTools,
            JsonSchema,
            Streaming,
            ResponsesWebSocket,
        ],
        "openai-compatible.responses"
        | "anthropic-compatible.messages"
        | "grok.official.responses" => &[
            Tools,
            ParallelTools,
            Reasoning,
            JsonSchema,
            Streaming,
            ResponsesWebSocket,
        ],
        "grok.build.responses" => &[
            Tools,
            Reasoning,
            JsonSchema,
            Streaming,
            ResponsesWebSocket,
            StoredResponses,
            ResponseCompaction,
        ],
        "grok.console.responses" | "kiro.messages" => {
            &[Tools, Reasoning, JsonSchema, Streaming, ResponsesWebSocket]
        }
        "grok.web.responses" => &[Streaming, ResponsesWebSocket, StoredResponses],
        _ => return Err(RuntimeCompositionError::Unavailable),
    };
    CapabilitySet::try_new(capabilities.iter().copied())
        .map_err(|_| RuntimeCompositionError::Unavailable)
}

/// Builds the request-time state from exactly the active isolated control-plane configuration.
///
/// An empty Staging database deliberately starts with an authenticated but unsendable data plane.
/// Once management publishes the temporary graph, systemd must restart this isolated process so a
/// new encrypted Credential pool and exact Snapshot are built atomically at process bootstrap.
#[cfg(test)]
pub(crate) fn build_data_plane_composition(
    database: &Path,
    secret_store: &SecretStore,
    registry: Arc<RouteSnapshotRegistry>,
    client_key_service: ClientKeyService,
) -> Result<DataPlaneComposition, RuntimeCompositionError> {
    build_data_plane_composition_with_web_proxy(
        database,
        secret_store,
        registry,
        client_key_service,
        None,
        None,
        8191,
    )
}

/// Builds the data plane with an optional, explicitly supplied Web-only proxy.
///
/// The default deployment remains direct. A proxy is a process-envelope input rather than a
/// control-plane field so an operator can bind a temporary, isolated Web exit without changing
/// persisted routes or accidentally routing other providers through it.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_data_plane_composition_with_web_proxy(
    database: &Path,
    secret_store: &SecretStore,
    registry: Arc<RouteSnapshotRegistry>,
    client_key_service: ClientKeyService,
    web_proxy: Option<UpstreamProxy>,
    flaresolverr_proxy: Option<UpstreamProxy>,
    flaresolverr_port: u16,
) -> Result<DataPlaneComposition, RuntimeCompositionError> {
    let mut repository = SqliteControlPlaneRepository::open(database)
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
    let attempt_stages = Arc::new(P12AttemptStageStore::new());
    let runtime_health = Arc::new(RuntimeHealthRegistry::new());
    let runtime_quota = Arc::new(RuntimeQuotaRegistry::new());
    let (event_queue, event_receiver) = BoundedEventQueue::try_new(EventQueueConfig::default())
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
    let event_queue = Arc::new(event_queue);
    let telemetry_metrics = Arc::new(PrometheusMetrics::default());
    // `gateway_event_log` is append-only by migration 0005's triggers, so serve-time retention
    // is impossible today: the log grows by three Required rows per completed request at P12's
    // single-credential loopback concurrency. Trimming it requires a new migration plus an
    // ADR-0027 revision; until then the encrypted backup remains the only copy channel and this
    // bounded-growth risk is accepted explicitly rather than hidden.
    let event_writer =
        AsyncSqliteEventWriter::new(database, event_receiver, EventWriterConfig::default())
            .with_telemetry_pipeline(Arc::new(TelemetryPipeline::new(
                Arc::clone(&telemetry_metrics),
                Arc::new(TracingJsonExporter),
                Arc::new(NoopOpenTelemetryExporter),
            )));
    let event_sink: Arc<dyn GatewayEventSink> = Arc::new(P12FanoutEventSink::new(
        Arc::clone(&attempt_stages),
        Arc::clone(&event_queue),
    ));
    let mut routing_price_snapshot: Option<Arc<RoutingPriceSnapshot>> = None;
    let (executor, provider_account_pools, route_explain_scheduler, channel_pin):
        ProviderAccountPoolComposition = match repository
        .load_active_configuration()
        .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::ControlPlane))?
    {
        Some(configuration) => {
            let observed_at_ms = system_now_ms_runtime()?;
            if let Some(policy) = configuration.routing_price_policy.as_ref() {
                let catalog = repository
                    .load_billing_catalog(&policy.catalog_version_id)
                    .map_err(|_| {
                        RuntimeCompositionError::Stage(RuntimeCompositionStage::RoutingPricePolicy)
                    })?
                    .ok_or(RuntimeCompositionError::Stage(
                        RuntimeCompositionStage::RoutingPricePolicy,
                    ))?;
                let snapshot = registry.load();
                let compiled = compile_routing_price_snapshot(
                    &snapshot,
                    &configuration.version.id,
                    policy,
                    &catalog,
                    u64::try_from(observed_at_ms).map_err(|_| {
                        RuntimeCompositionError::Stage(RuntimeCompositionStage::RoutingPricePolicy)
                    })?,
                )
                .map_err(|_| {
                    RuntimeCompositionError::Stage(RuntimeCompositionStage::RoutingPricePolicy)
                })?;
                routing_price_snapshot = Some(Arc::new(compiled));
            }
            let (executor, provider_account_pools, route_explain_scheduler) =
                P12RoutedResponsesExecutor::try_new(
                    database,
                    &configuration,
                    secret_store,
                    Arc::clone(&registry),
                    Arc::clone(&attempt_stages),
                    Arc::clone(&event_sink),
                    Arc::clone(&runtime_health),
                    Arc::clone(&runtime_quota),
                    web_proxy,
                    flaresolverr_proxy,
                    flaresolverr_port,
                    routing_price_snapshot.as_ref(),
                )?;
            let executor = Arc::new(executor);
            let channel_pin: Box<dyn ManagementChannelPinFacade> =
                Box::new(P12ChannelPinFacade::new(Arc::clone(&executor)));
            (
                executor,
                provider_account_pools,
                Some(route_explain_scheduler),
                channel_pin,
            )
        }
        None => (
            Arc::new(NoActiveConfigurationExecutor),
            Box::new(RejectingProviderAccountPoolFacade::new()),
            None,
            Box::new(RejectingManagementChannelPinFacade::new()),
        ),
    };
    let authenticator = Arc::new(gateway_router::SnapshotClientKeyAuthenticator::new(
        Arc::clone(&registry),
        client_key_service,
    ));
    let stored_responses = Arc::new(
        SqliteStoredResponseStore::open(database, secret_store.clone())
            .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::StoredResponse))?,
    );
    let data = ResponsesHttpState::with_snapshot_metadata_and_event_sink(
        executor,
        Arc::new(SystemResponsesMetadataFactory::new()),
        authenticator,
        event_sink,
        default_stream_capacity().map_err(|_| {
            RuntimeCompositionError::Stage(RuntimeCompositionStage::EndpointRuntime)
        })?,
    )
    .with_stored_response_store(stored_responses);
    let event_store = SqliteEventStore::open(database)
        .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::EventStore))?;

    Ok(DataPlaneComposition {
        data,
        management_runtime: Box::new(SnapshotManagementRuntimeFacade {
            registry,
            attempt_stages,
            runtime_health,
            runtime_quota,
            route_explain_scheduler,
            routing_price_snapshot,
            event_store,
        }),
        provider_account_pools,
        channel_pin,
        observability: ManagementObservabilityHttpState::new(telemetry_metrics, event_queue)
            .with_durability(Arc::new(P12DurabilityMetrics::new(
                event_writer.metrics_handle(),
            ))),
        event_writer,
    })
}

/// Publishes the durable writer's counters to the protected metrics exposition.
///
/// A quarantined Required event is the one durable loss this composition permits, so it must be
/// scrapeable rather than only visible in a discarded shutdown snapshot.
struct P12DurabilityMetrics {
    handle: EventWriterMetricsHandle,
}

impl P12DurabilityMetrics {
    const fn new(handle: EventWriterMetricsHandle) -> Self {
        Self { handle }
    }
}

impl DurabilityMetricsSource for P12DurabilityMetrics {
    fn durability_counters(&self) -> (u64, u64, u64) {
        let snapshot = self.handle.snapshot();
        (
            snapshot.required_events_quarantined,
            snapshot.sqlite_write_failures,
            snapshot.pending_required,
        )
    }
}

/// Safe, target-free runtime-composition failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeCompositionError {
    /// A control-plane, Snapshot, encrypted Credential, or bounded transport invariant failed.
    Unavailable,
    /// A bounded, value-free classification used at the deployment boundary.
    Stage(RuntimeCompositionStage),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeCompositionStage {
    ControlPlane,
    RequiredResources,
    NetworkShape,
    CredentialBindings,
    RouteAccess,
    Snapshot,
    Egress,
    CredentialPool,
    NativeAccountPool,
    ProviderAccountPool,
    RoutingPricePolicy,
    AdapterRegistry,
    EndpointRuntime,
    EventStore,
    StoredResponse,
}

impl RuntimeCompositionStage {
    const fn label(self) -> &'static str {
        match self {
            Self::ControlPlane => "control_plane",
            Self::RequiredResources => "required_resources",
            Self::NetworkShape => "network_shape",
            Self::CredentialBindings => "credential_bindings",
            Self::RouteAccess => "route_access",
            Self::Snapshot => "snapshot",
            Self::Egress => "egress",
            Self::CredentialPool => "credential_pool",
            Self::NativeAccountPool => "native_account_pool",
            Self::ProviderAccountPool => "provider_account_pool",
            Self::RoutingPricePolicy => "routing_price_policy",
            Self::AdapterRegistry => "adapter_registry",
            Self::EndpointRuntime => "endpoint_runtime",
            Self::EventStore => "event_store",
            Self::StoredResponse => "stored_response",
        }
    }
}

impl fmt::Display for RuntimeCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("P12 Staging runtime is unavailable"),
            Self::Stage(stage) => write!(
                formatter,
                "P12 Staging runtime is unavailable (stage={})",
                stage.label()
            ),
        }
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
/// stage enum, and per-attempt terminal success/failure, bounded by the admitted attempt budget.
/// It never receives an endpoint, credential, URL, header, body, status, error detail, model,
/// token, or timestamp. Its request-path methods use `try_lock`: loss or contention is remembered
/// and later withholds the stage enrichment instead of delaying or changing an upstream request.
/// The durable event log is the authoritative Attempt listing; this ledger only adds the closed
/// stage that no durable event carries.
struct P12AttemptStageStore {
    records: Mutex<BTreeMap<RequestId, P12AttemptStageRecord>>,
    unavailable: AtomicBool,
    /// Source of the per-record insertion order that drives oldest-first eviction.
    sequence: AtomicU64,
}

struct P12AttemptStageRecord {
    stage: ManagementRequestAttemptStage,
    attempts: Vec<P12AttemptTerminal>,
    /// Monotone insertion order, used to evict the oldest request when the ledger is full.
    sequence: u64,
}

struct P12AttemptTerminal {
    attempt_id: String,
    outcome: &'static str,
}

impl P12AttemptStageStore {
    /// Concurrently observable requests the stage ledger retains before evicting the oldest.
    ///
    /// The widened admission bounds live requests by total Credential concurrency, so this is two
    /// generations of [`P12_MAX_TOTAL_BINDING_CONCURRENCY`] — enough for every in-flight request
    /// plus the recently completed ones an operator would inspect (the relationship is asserted by
    /// `stage_ledger_capacity_tracks_the_admitted_concurrency_bound`). Reaching the bound evicts
    /// the oldest record; it must never latch the ledger off, or one traffic burst would leave
    /// every later request unenriched.
    const MAX_RECORDS: usize = 32;

    fn new() -> Self {
        Self {
            records: Mutex::new(BTreeMap::new()),
            unavailable: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
        }
    }

    /// Drops the oldest retained request so a full ledger keeps serving the newest ones.
    fn evict_oldest(records: &mut BTreeMap<RequestId, P12AttemptStageRecord>) {
        let oldest = records
            .iter()
            .min_by_key(|(_, record)| record.sequence)
            .map(|(request_id, _)| request_id.clone());
        if let Some(oldest) = oldest {
            records.remove(&oldest);
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
            Self::evict_oldest(&mut records);
        }
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        records.insert(
            request_id.clone(),
            P12AttemptStageRecord {
                stage,
                attempts: Vec::new(),
                sequence,
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
        if record.attempts.len() >= P12_MAX_ROUTE_ATTEMPTS
            || record
                .attempts
                .iter()
                .any(|attempt| attempt.attempt_id == event.attempt_id().as_str())
        {
            self.mark_unavailable();
            return EventEmission::RequiredQueueFull;
        }
        record.attempts.push(P12AttemptTerminal {
            attempt_id: event.attempt_id().as_str().to_owned(),
            outcome: match event.outcome() {
                AttemptOutcome::Succeeded => "succeeded",
                AttemptOutcome::Failed(_) => "failed",
            },
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
        if record.attempts.is_empty() {
            return Err(ManagementRuntimeError::Unavailable);
        }
        let mut attempts = Vec::with_capacity(record.attempts.len());
        for terminal in &record.attempts {
            attempts.push(ManagementRequestAttempt::try_new(
                terminal.attempt_id.clone(),
                terminal.outcome,
                None,
                None,
            )?);
        }
        // The stage ledger tracks one request-level stage, which describes the newest attempt.
        if let Some(last) = attempts.pop() {
            attempts.push(last.with_stage(record.stage));
        }
        Ok(attempts)
    }

    /// Returns the newest stage recorded for one Request, regardless of terminal pairing.
    ///
    /// [`Self::stage_view`] withholds a stage until a terminal Attempt event pairs with it, which
    /// is the right management projection but hides which stages an attempt that failed before
    /// its first terminal did and did not reach. This seam exposes exactly that.
    fn recorded_stage(&self, request_id: &RequestId) -> Option<ManagementRequestAttemptStage> {
        let records = self.records.try_lock().ok()?;
        records.get(request_id).map(|record| record.stage)
    }

    /// Returns the closed terminal stage for one Request, or `None` on any projection loss.
    ///
    /// Contention, capacity exhaustion, a missing record, and a record without any terminal
    /// pairing all degrade to `None`. Retained as the test seam for the ledger-poisoning
    /// semantics; the management listing reads the ledger through
    /// [`Self::list_request_attempts`].
    #[cfg(test)]
    fn stage_view(&self, request_id: &RequestId) -> Option<ManagementRequestAttemptStage> {
        if self.unavailable.load(Ordering::Acquire) {
            return None;
        }
        let records = self.records.try_lock().ok()?;
        let record = records.get(request_id)?;
        if record.attempts.is_empty() {
            return None;
        }
        Some(record.stage)
    }

    fn mark_unavailable(&self) {
        self.unavailable.store(true, Ordering::Release);
    }
}

/// Fans one admitted event out to the value-free stage ledger and the bounded durable queue.
///
/// The Attempt terminal projection is recorded first so a saturated queue cannot hide the stage.
/// Every event then flows to the bounded queue, whose non-blocking admission result is the
/// authoritative outcome: Required loss stays explicit as `RequiredQueueFull` and only
/// low-priority diagnostics may be dropped, exactly as the bounded-events baseline demands.
struct P12FanoutEventSink {
    attempts: Arc<P12AttemptStageStore>,
    queue: Arc<BoundedEventQueue>,
}

impl P12FanoutEventSink {
    fn new(attempts: Arc<P12AttemptStageStore>, queue: Arc<BoundedEventQueue>) -> Self {
        Self { attempts, queue }
    }
}

impl GatewayEventSink for P12FanoutEventSink {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        if let GatewayEvent::Attempt(attempt) = &event {
            let _stage_projection = self.attempts.record_terminal(attempt);
        }
        self.queue.try_emit(event)
    }
}

struct P12RoutedResponsesExecutor {
    registry: Arc<RouteSnapshotRegistry>,
    snapshot_version: SnapshotVersion,
    config_revision: ConfigRevision,
    scheduler: Arc<RouteCredentialScheduler>,
    orchestrator: Arc<AttemptOrchestrator>,
    endpoints: Arc<BTreeMap<EndpointId, EndpointRuntime>>,
    compatible_endpoints: Arc<BTreeMap<EndpointId, Arc<CompatibleEndpointRuntime>>>,
    client_pool: Arc<UpstreamClientPool>,
    attempt_stages: Arc<P12AttemptStageStore>,
    event_sink: Arc<dyn GatewayEventSink>,
    channel_pin_in_flight: AtomicUsize,
}

impl P12RoutedResponsesExecutor {
    // The composition boundary intentionally receives the already-validated runtime stores and
    // registries explicitly. Native Grok account compilation adds the database handle alongside
    // the existing immutable snapshot dependencies; bundling them would hide ownership at the
    // control/data-plane boundary.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn try_new(
        database: &Path,
        configuration: &ControlPlaneConfiguration,
        secret_store: &SecretStore,
        registry: Arc<RouteSnapshotRegistry>,
        attempt_stages: Arc<P12AttemptStageStore>,
        event_sink: Arc<dyn GatewayEventSink>,
        runtime_health: Arc<RuntimeHealthRegistry>,
        runtime_quota: Arc<RuntimeQuotaRegistry>,
        web_proxy: Option<UpstreamProxy>,
        flaresolverr_proxy: Option<UpstreamProxy>,
        flaresolverr_port: u16,
        routing_price_snapshot: Option<&Arc<RoutingPriceSnapshot>>,
    ) -> Result<P12ExecutorComposition, RuntimeCompositionError> {
        // Keep the value-free stage detail for the native Grok diagnostic boundary, while
        // retaining the historical generic failure for ordinary graphs.  Existing callers use
        // the generic variant as a compatibility contract and it must not change merely because
        // OAuth support was added to the composition root.
        let native_grok_graph = configuration.endpoints.iter().any(is_native_grok_endpoint);
        let stage_error = |stage| {
            if native_grok_graph {
                RuntimeCompositionError::Stage(stage)
            } else {
                RuntimeCompositionError::Unavailable
            }
        };
        validate_p12_required_resources(configuration)
            .map_err(|_| stage_error(RuntimeCompositionStage::RequiredResources))?;
        validate_p12_network_shape(configuration)
            .map_err(|_| stage_error(RuntimeCompositionStage::NetworkShape))?;
        validate_p12_credential_bindings(configuration)
            .map_err(|_| stage_error(RuntimeCompositionStage::CredentialBindings))?;
        validate_p12_route_access_shape(configuration)
            .map_err(|_| stage_error(RuntimeCompositionStage::RouteAccess))?;
        let snapshot = registry.load();
        if snapshot.version().as_str() != configuration.version.id.as_str() {
            return Err(RuntimeCompositionError::Stage(
                RuntimeCompositionStage::Snapshot,
            ));
        }
        let policies = EgressPolicyCompiler::compile(configuration)
            .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::Egress))?;
        let native_endpoint_providers = configuration
            .endpoints
            .iter()
            .filter_map(|endpoint| {
                native_grok_provider_for_endpoint(endpoint)
                    .map(|provider| (endpoint.id.clone(), provider))
            })
            .collect::<BTreeMap<_, _>>();
        let native_endpoint_ids = native_endpoint_providers
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let observed_at_ms = system_now_ms_runtime()
            .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::CredentialPool))?;
        let mut provider_account_descriptors =
            ordinary_provider_account_descriptors(configuration, &native_endpoint_ids);
        let pools = CredentialPoolCompiler::new(secret_store)
            .compile_excluding_endpoints(configuration, &native_endpoint_ids)
            .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::CredentialPool))?;
        let pools = if native_endpoint_ids.is_empty() {
            pools
        } else {
            let bindings = native_endpoint_providers
                .iter()
                .map(|(endpoint_id, provider)| {
                    GrokAccountEndpointBinding::new(*provider, endpoint_id.clone())
                })
                .collect::<Vec<_>>();
            let native_store = GrokAccountPoolStore::try_open(database, secret_store.clone())
                .map_err(|_| {
                    RuntimeCompositionError::Stage(RuntimeCompositionStage::NativeAccountPool)
                })?;
            let native_compilation = native_store
                .compile_native_runtime(&bindings, observed_at_ms)
                .map_err(|_| {
                    RuntimeCompositionError::Stage(RuntimeCompositionStage::NativeAccountPool)
                })?;
            provider_account_descriptors = provider_account_descriptors.and_then(|mut ordinary| {
                let native_accounts =
                    native_compilation
                        .account_metadata()
                        .ok_or(RuntimeCompositionError::Stage(
                            RuntimeCompositionStage::ProviderAccountPool,
                        ))?;
                ordinary.extend(native_provider_account_descriptors(
                    configuration,
                    &bindings,
                    native_accounts,
                )?);
                Ok(ordinary)
            });
            for endpoint_id in &native_endpoint_ids {
                if native_compilation
                    .credential_pools()
                    .pool(endpoint_id)
                    .is_none()
                {
                    return Err(RuntimeCompositionError::Stage(
                        RuntimeCompositionStage::NativeAccountPool,
                    ));
                }
            }
            native_compilation
                .seed_runtime_health(&runtime_health)
                .map_err(|_| {
                    RuntimeCompositionError::Stage(RuntimeCompositionStage::NativeAccountPool)
                })?;
            native_compilation
                .seed_runtime_quota(&runtime_quota)
                .map_err(|_| {
                    RuntimeCompositionError::Stage(RuntimeCompositionStage::NativeAccountPool)
                })?;
            pools
                .merge((*native_compilation.credential_pools()).clone())
                .map_err(|_| {
                    RuntimeCompositionError::Stage(RuntimeCompositionStage::CredentialPool)
                })?
        };
        let pools = Arc::new(pools);
        let provider_account_pools = provider_account_pool_facade(
            configuration.version.id.as_str().to_owned(),
            provider_account_descriptors,
            Arc::clone(&pools),
            Arc::clone(&runtime_health),
            Arc::clone(&runtime_quota),
        );
        let adapters = p12_api_format_adapter_registry().map_err(|_| {
            RuntimeCompositionError::Stage(RuntimeCompositionStage::AdapterRegistry)
        })?;
        let endpoints = endpoint_runtimes(
            configuration,
            &snapshot,
            &policies,
            &adapters,
            web_proxy,
            flaresolverr_proxy,
            flaresolverr_port,
        )
        .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::EndpointRuntime))?;
        let (compatible_transport_registries, compatible_binding_settings) =
            compatible_egress_runtime_inputs(configuration, &endpoints, secret_store).map_err(
                |_| RuntimeCompositionError::Stage(RuntimeCompositionStage::EndpointRuntime),
            )?;
        let compatible_endpoints = CompatibleEgressRuntimeCompiler::new(
            configuration,
            &policies,
            Arc::clone(&pools),
            Arc::clone(&runtime_health),
            Arc::clone(&runtime_quota),
            compatible_transport_registries,
            compatible_binding_settings,
        )
        .compile()
        .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::EndpointRuntime))?
        .into_iter()
        .map(|(endpoint_id, runtime)| (endpoint_id, Arc::new(runtime)))
        .collect::<BTreeMap<_, _>>();
        let scheduler = Arc::new(RouteCredentialScheduler::new_with_provider_price_rates(
            Arc::clone(&snapshot),
            Arc::clone(&pools),
            routing_price_snapshot.map_or_else(
                || Arc::new(BTreeMap::new()),
                |snapshot| snapshot.candidate_price_rates_arc(),
            ),
        ));
        let route_explain_scheduler = Arc::clone(&scheduler);
        let orchestrator = Arc::new(AttemptOrchestrator::with_runtime_quota_and_clock_config(
            scheduler,
            runtime_health,
            runtime_quota,
            Arc::new(SystemRuntimeHealthClock),
            AttemptOrchestratorConfig::default(),
        ));
        // Two response-mode transport profiles per Endpoint host; sizing the cache to the graph
        // keeps DNS-pinned clients warm instead of evicting them through a fixed four-entry bound.
        let cached_clients = configuration.endpoints.len().saturating_mul(2).max(4);
        let client_pool = Arc::new(UpstreamClientPool::new(
            NonZeroUsize::new(cached_clients).ok_or(RuntimeCompositionError::Unavailable)?,
        ));

        Ok((
            Self {
                registry,
                snapshot_version: snapshot.version().clone(),
                config_revision: ConfigRevision::try_new(configuration.version.revision).map_err(
                    |_| RuntimeCompositionError::Stage(RuntimeCompositionStage::Snapshot),
                )?,
                scheduler: Arc::clone(&route_explain_scheduler),
                orchestrator,
                endpoints: Arc::new(endpoints),
                compatible_endpoints: Arc::new(compatible_endpoints),
                client_pool,
                attempt_stages,
                event_sink,
                channel_pin_in_flight: AtomicUsize::new(0),
            },
            provider_account_pools,
            route_explain_scheduler,
        ))
    }

    fn snapshot_is_current(&self) -> bool {
        self.registry.load().version() == &self.snapshot_version
    }

    /// Executes one exact management Channel Pin against the same pools, scheduler, endpoint
    /// runtimes, and event sink used by serving.  The request body is deliberately fixed and is
    /// never returned or written to the audit log; the management caller chooses only target IDs
    /// and JSON/SSE mode.
    #[allow(clippy::too_many_lines)]
    async fn execute_channel_pin(
        &self,
        request: ManagementChannelPinRequest,
    ) -> Result<ManagementChannelPinReceipt, ManagementChannelPinError> {
        if request.config_version_id().as_str() != self.snapshot_version.as_str()
            || request.config_revision() != self.config_revision
            || !self.snapshot_is_current()
        {
            return Err(ManagementChannelPinError::SnapshotConflict);
        }
        let observed_at_ms =
            system_now_ms_runtime().map_err(|_| ManagementChannelPinError::Unavailable)?;
        let observation = Arc::new(P13ChannelPinObservation::default());
        let route = self
            .scheduler
            .route(request.route_id())
            .ok_or(ManagementChannelPinError::InvalidTarget)?;
        let snapshot = self.registry.load();
        let Some(public_model) = snapshot
            .resolve_public_model(request.requested_model())
            .filter(|model| model.route_id() == request.route_id())
        else {
            return Err(ManagementChannelPinError::InvalidTarget);
        };
        if snapshot.route(public_model.route_id()).is_none() {
            return Err(ManagementChannelPinError::InvalidTarget);
        }
        let mut candidates = route.candidates().iter().filter(|candidate| {
            candidate.endpoint_id() == request.channel_id()
                && ProviderId::try_new(candidate.upstream_id().as_str().to_owned())
                    .is_ok_and(|provider| provider == *request.provider_id())
        });
        let Some(_candidate) = candidates.next() else {
            return Err(ManagementChannelPinError::InvalidTarget);
        };
        if candidates.next().is_some() {
            return Err(ManagementChannelPinError::InvalidTarget);
        }
        let endpoint_runtime = self
            .endpoints
            .get(request.channel_id())
            .ok_or(ManagementChannelPinError::InvalidTarget)?;
        if !channel_pin_single_transport_adapter(&endpoint_runtime.adapter) {
            // Native browser/bootstrap adapters can issue auxiliary token, Statsig, or refresh
            // requests internally. Until those Provider boundaries expose an explicit one-shot
            // transport policy, reject them before a lease or network call rather than claiming
            // that the management probe sent exactly one upstream request.
            return Err(ManagementChannelPinError::InvalidTarget);
        }
        let _in_flight = P13ChannelPinInFlightGuard::try_acquire(&self.channel_pin_in_flight)?;
        // The management request is pinned to the same immutable route snapshot as the serving
        // scheduler. Re-check immediately before constructing the driver/lease so a publication
        // between the initial graph read and exact selection cannot mix generations.
        if !self.snapshot_is_current()
            || self.scheduler.snapshot_version().as_str() != self.snapshot_version.as_str()
        {
            return Err(ManagementChannelPinError::SnapshotConflict);
        }
        let client_protocol = match request.protocol() {
            gateway_http_actix::management_resources::ManagementRequestProtocol::OpenAiChatCompletions => {
                ProtocolFormat::OpenAiChatCompletions
            }
            gateway_http_actix::management_resources::ManagementRequestProtocol::OpenAiResponses => {
                ProtocolFormat::OpenAiResponses
            }
            gateway_http_actix::management_resources::ManagementRequestProtocol::AnthropicMessages => {
                ProtocolFormat::AnthropicMessages
            }
        };
        let request_id = p13_channel_pin_request_id()?;
        let mut canonical = CanonicalRequest {
            requested_model: request.requested_model().to_owned(),
            messages: vec![CanonicalMessage {
                role: MessageRole("user".to_owned()),
                content: vec![MessageContent::Text(TextContent {
                    text: "Reply with OK.".to_owned(),
                    extensions: RawExtensions::default(),
                })],
                extensions: RawExtensions::default(),
            }],
            tools: Vec::new(),
            thinking: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            extensions: RawExtensions::default(),
        };
        // Keep the fixed probe cheap on generic adapters. Codex OAuth Responses is the one
        // reviewed exception: that upstream rejects `max_output_tokens`, and its adapter already
        // forces a bounded-compatible SSE projection, so adding the field would turn a useful
        // one-shot diagnostic into a deterministic pre-send protocol rejection.
        let probe_limit = RawJson::from_json_string("8".to_owned())
            .map_err(|_| ManagementChannelPinError::Unavailable)?;
        match &endpoint_runtime.adapter {
            EndpointAdapter::OpenAiChatCompletions(_) => canonical
                .extensions
                .try_insert(P12_OPENAI_CHAT_MAX_TOKENS_EXTENSION, probe_limit.clone())
                .map_err(|_| ManagementChannelPinError::Unavailable)?,
            EndpointAdapter::OpenAiResponses(endpoint)
                if !endpoint
                    .url()
                    .starts_with("https://chatgpt.com/backend-api/codex/") =>
            {
                canonical
                    .extensions
                    .try_insert(P12_OPENAI_MAX_OUTPUT_TOKENS_EXTENSION, probe_limit.clone())
                    .map_err(|_| ManagementChannelPinError::Unavailable)?;
            }
            EndpointAdapter::AnthropicMessages(_) => canonical
                .extensions
                .try_insert(P12_ANTHROPIC_MAX_TOKENS_EXTENSION, probe_limit)
                .map_err(|_| ManagementChannelPinError::Unavailable)?,
            _ => {}
        }
        let mode = match request.mode() {
            ManagementChannelPinMode::Json => ResponsesResponseMode::NonStreaming,
            ManagementChannelPinMode::Sse => ResponsesResponseMode::Streaming,
        };
        let endpoints = Arc::clone(&self.endpoints);
        let driver = EndpointAttemptDriver {
            request_id: request_id.clone(),
            request: canonical,
            client_protocol,
            native_payload: None,
            usage_projection: match client_protocol {
                ProtocolFormat::AnthropicMessages => P12ResponseUsageProjection::AnthropicMessages,
                ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses => {
                    P12ResponseUsageProjection::OpenAiResponses
                }
            },
            mode,
            client_transport: ResponsesClientTransport::Http,
            endpoints,
            compatible_endpoints: Arc::clone(&self.compatible_endpoints),
            client_pool: Arc::clone(&self.client_pool),
            attempt_stages: Arc::clone(&self.attempt_stages),
            allow_compatibility_retry: false,
            allow_egress_refresh: false,
            channel_pin_observation: Some(Arc::clone(&observation)),
        };
        let started = self
            .orchestrator
            .start_pinned_once_with_event_sink(
                &request_id,
                request.route_id(),
                request.provider_id(),
                request.channel_id(),
                request.credential_id(),
                |candidate| driver.project_candidate(candidate).is_ok(),
                &driver,
                &P13ChannelPinRetryGate,
                // A Channel Pin's durable terminal state is written by the management handler
                // after bounded source drain. Do not publish the serving Attempt event here: the
                // orchestrator returns before the source is consumed, so that event would claim
                // success before a later SSE/JSON decoder failure could be observed.
                &NoopGatewayEventSink,
            )
            .await;
        // Channel Pin deliberately suppresses the serving Attempt event until after the source
        // is drained, so use the request-local stage projection rather than the public attempt
        // listing view (which requires a terminal Attempt pairing).
        let mut stage = self.attempt_stages.recorded_stage(&request_id);
        let attempt_count = u8::from(observation.attempted());
        let upstream_sent = observation.upstream_sent();
        let upstream_stage = stage.is_some_and(|stage| {
            matches!(
                stage,
                ManagementRequestAttemptStage::HttpTransport
                    | ManagementRequestAttemptStage::HttpStatus
                    | ManagementRequestAttemptStage::ContentType
                    | ManagementRequestAttemptStage::BodyRead
                    | ManagementRequestAttemptStage::Decoder
                    | ManagementRequestAttemptStage::SseBootstrap
            )
        });
        let upstream_sent = upstream_sent || upstream_stage;
        match started {
            Ok(started) => {
                let (mut source, _selection) = started.into_parts();
                let mut event_count = 0_usize;
                let mut response_started = false;
                let drain_deadline = Instant::now() + P13_CHANNEL_PIN_TOTAL_TIMEOUT;
                let mut event_state = CanonicalEventState::default();
                let lifecycle_complete = loop {
                    let remaining = drain_deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        stage = Some(ManagementRequestAttemptStage::BodyRead);
                        return self.channel_pin_receipt(
                            &request,
                            request_id,
                            ManagementChannelPinOutcome::Failed,
                            attempt_count,
                            upstream_sent,
                            response_started,
                            observed_at_ms,
                            stage,
                        );
                    }
                    let wait = remaining.min(P13_CHANNEL_PIN_IDLE_TIMEOUT);
                    match actix_web::rt::time::timeout(wait, source.next_event()).await {
                        Err(_) | Ok(Err(_)) => {
                            stage = Some(ManagementRequestAttemptStage::Decoder);
                            return self.channel_pin_receipt(
                                &request,
                                request_id,
                                ManagementChannelPinOutcome::Failed,
                                attempt_count,
                                upstream_sent,
                                response_started,
                                observed_at_ms,
                                stage,
                            );
                        }
                        Ok(Ok(Some(event))) => {
                            event_count = event_count.saturating_add(1);
                            if event_count > 4096 {
                                stage = Some(ManagementRequestAttemptStage::BodyRead);
                                return self.channel_pin_receipt(
                                    &request,
                                    request_id,
                                    ManagementChannelPinOutcome::Failed,
                                    attempt_count,
                                    upstream_sent,
                                    response_started,
                                    observed_at_ms,
                                    stage,
                                );
                            }
                            if matches!(event, CanonicalEvent::StreamError(_)) {
                                stage = Some(ManagementRequestAttemptStage::Decoder);
                                return self.channel_pin_receipt(
                                    &request,
                                    request_id,
                                    ManagementChannelPinOutcome::Failed,
                                    attempt_count,
                                    upstream_sent,
                                    response_started,
                                    observed_at_ms,
                                    stage,
                                );
                            }
                            if event_state.apply(&event).is_err() {
                                stage = Some(ManagementRequestAttemptStage::Decoder);
                                return self.channel_pin_receipt(
                                    &request,
                                    request_id,
                                    ManagementChannelPinOutcome::Failed,
                                    attempt_count,
                                    upstream_sent,
                                    response_started,
                                    observed_at_ms,
                                    stage,
                                );
                            }
                            if matches!(event, CanonicalEvent::ResponseStart(_)) {
                                response_started = true;
                            }
                            if event_state.is_success() {
                                break true;
                            }
                        }
                        Ok(Ok(None)) => {
                            if !event_state.is_success() {
                                stage = Some(ManagementRequestAttemptStage::BodyRead);
                            }
                            break event_state.finish().is_ok() && event_state.is_success();
                        }
                    }
                };
                self.channel_pin_receipt(
                    &request,
                    request_id,
                    if lifecycle_complete {
                        ManagementChannelPinOutcome::Succeeded
                    } else {
                        ManagementChannelPinOutcome::Failed
                    },
                    attempt_count,
                    upstream_sent,
                    response_started,
                    observed_at_ms,
                    stage,
                )
            }
            Err(_) => self.channel_pin_receipt(
                &request,
                request_id,
                if attempt_count == 0 {
                    ManagementChannelPinOutcome::Rejected
                } else {
                    ManagementChannelPinOutcome::Failed
                },
                attempt_count,
                upstream_sent,
                false,
                observed_at_ms,
                stage,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn channel_pin_receipt(
        &self,
        request: &ManagementChannelPinRequest,
        request_id: RequestId,
        outcome: ManagementChannelPinOutcome,
        attempt_count: u8,
        upstream_sent: bool,
        response_started: bool,
        observed_at_ms: i64,
        stage: Option<ManagementRequestAttemptStage>,
    ) -> Result<ManagementChannelPinReceipt, ManagementChannelPinError> {
        ManagementChannelPinReceipt::try_new(
            request_id,
            request.config_version_id().clone(),
            self.config_revision,
            request.provider_id().clone(),
            request.channel_id().clone(),
            request.route_id().clone(),
            request.credential_id().clone(),
            request.requested_model().to_owned(),
            request.protocol(),
            request.mode(),
            outcome,
            upstream_sent,
            attempt_count,
            response_started,
            observed_at_ms,
            stage,
        )
    }
}

/// Channel Pin always disables transparent retries and has no external cancellation source.
struct P13ChannelPinRetryGate;

/// RAII admission token for the bounded management diagnostic budget.
struct P13ChannelPinInFlightGuard<'a> {
    counter: &'a AtomicUsize,
}

impl<'a> P13ChannelPinInFlightGuard<'a> {
    fn try_acquire(counter: &'a AtomicUsize) -> Result<Self, ManagementChannelPinError> {
        let acquired = counter
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < P13_CHANNEL_PIN_MAX_IN_FLIGHT).then_some(current + 1)
            })
            .is_ok();
        if acquired {
            Ok(Self { counter })
        } else {
            Err(ManagementChannelPinError::Unavailable)
        }
    }
}

impl Drop for P13ChannelPinInFlightGuard<'_> {
    fn drop(&mut self) {
        let previous = self.counter.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "Channel Pin in-flight counter underflow");
    }
}

fn channel_pin_single_transport_adapter(adapter: &EndpointAdapter) -> bool {
    matches!(
        adapter,
        EndpointAdapter::OpenAiChatCompletions(_)
            | EndpointAdapter::OpenAiResponses(_)
            | EndpointAdapter::AnthropicMessages(_)
    )
}

impl TransparentRetryGate for P13ChannelPinRetryGate {
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

struct P12ChannelPinFacade {
    executor: Arc<P12RoutedResponsesExecutor>,
}

impl P12ChannelPinFacade {
    fn new(executor: Arc<P12RoutedResponsesExecutor>) -> Self {
        Self { executor }
    }
}

impl ManagementChannelPinFacade for P12ChannelPinFacade {
    fn execute(&self, request: ManagementChannelPinRequest) -> ManagementChannelPinFuture {
        let executor = Arc::clone(&self.executor);
        Box::pin(async move { executor.execute_channel_pin(request).await })
    }
}

impl ResponsesExecutor for P12RoutedResponsesExecutor {
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
        let compatible_endpoints = Arc::clone(&self.compatible_endpoints);
        let client_pool = Arc::clone(&self.client_pool);
        let attempt_stages = Arc::clone(&self.attempt_stages);
        let event_sink = Arc::clone(&self.event_sink);
        let context = execution.context().clone();
        let request = execution.request().clone();
        let client_protocol = execution.client_protocol();
        let usage_projection = match client_protocol {
            ProtocolFormat::AnthropicMessages => P12ResponseUsageProjection::AnthropicMessages,
            ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses => {
                P12ResponseUsageProjection::OpenAiResponses
            }
        };
        let native_payload = execution.native_payload().cloned();
        let route_id = execution.route_id().cloned();
        let mode = execution.mode();
        let client_transport = execution.client_transport();
        let retry_gate = Arc::clone(execution.retry_gate());
        let lineage_recorder = execution.lineage_recorder().cloned();
        let continuation_pin = execution.continuation_pin().cloned();
        let registry = Arc::clone(&self.registry);
        let snapshot_version = self.snapshot_version.clone();

        Box::pin(async move {
            // A management publication may replace the active Config Version while this request
            // is waiting for its first executor poll. Recheck at the request-start boundary before
            // selector/lease work. A publication after this check follows the existing pinned
            // in-flight Snapshot semantics rather than trying to revoke the request atomically.
            if registry.load().version() != &snapshot_version {
                return Err(stale_runtime_error());
            }
            let route_id = route_id.ok_or_else(route_not_found_error)?;
            if continuation_pin.as_ref().is_some_and(|pin| {
                pin.lineage().snapshot_version() != &snapshot_version
                    || pin.lineage().route_id() != &route_id
            }) {
                return Err(stale_runtime_error());
            }
            let exact_continuation = continuation_pin.is_some();
            let driver = EndpointAttemptDriver {
                request_id: context.request_id().clone(),
                request,
                client_protocol,
                native_payload,
                usage_projection,
                mode,
                client_transport,
                endpoints,
                compatible_endpoints,
                client_pool,
                attempt_stages,
                allow_compatibility_retry: !exact_continuation,
                allow_egress_refresh: !exact_continuation,
                channel_pin_observation: None,
            };
            // BC-ROUTER-005: a Candidate may only serve the client protocol it speaks. Once the
            // graph can hold Endpoints of more than one format, selecting without this filter
            // hands an OpenAI-Responses request to an Anthropic Candidate, whose request build
            // then fails non-retryably after the lease was taken — a hard failure where the
            // filter would simply have chosen a different Candidate before the first byte.
            let started = match continuation_pin.as_ref() {
                Some(pin) => {
                    orchestrator
                        .start_continuation_once_with_event_sink(
                            context.request_id(),
                            pin,
                            |candidate| driver.project_candidate(candidate).is_ok(),
                            &driver,
                            retry_gate.as_ref(),
                            event_sink.as_ref(),
                        )
                        .await?
                }
                None => {
                    orchestrator
                        .start_with_event_sink_provider_scoped_matching(
                            context.request_id(),
                            &route_id,
                            |candidate| driver.project_candidate(candidate).is_ok(),
                            &driver,
                            retry_gate.as_ref(),
                            event_sink.as_ref(),
                        )
                        .await?
                }
            };
            if let Some(recorder) = lineage_recorder {
                recorder.record(stored_response_execution_lineage(
                    &snapshot_version,
                    &route_id,
                    started.candidate(),
                    started.lease(),
                )?)?;
            }
            let (source, selection) = started.into_parts();
            Ok(Box::new(LeaseHoldingEventSource {
                source: Box::new(ProjectedEventSource::new(source, client_protocol)),
                _selection: selection,
            }) as Box<dyn ResponsesEventSource>)
        })
    }
}

fn stored_response_execution_lineage(
    snapshot_version: &SnapshotVersion,
    route_id: &RouteId,
    candidate: &SnapshotRouteCandidate,
    lease: &CredentialLease,
) -> Result<ResponsesExecutionLineage, GatewayError> {
    let provider_id = ProviderId::try_new(candidate.upstream_id().as_str().to_owned())
        .map_err(|_| internal_error())?;
    Ok(ResponsesExecutionLineage::new(
        snapshot_version.clone(),
        provider_id,
        candidate.upstream_id().clone(),
        candidate.endpoint_id().clone(),
        route_id.clone(),
        candidate.id().clone(),
        lease.credential_id().clone(),
        lease.credential_revision(),
    ))
}

/// Builds one Endpoint's format-specific execution binding from its stored configuration.
///
/// The registry is keyed by format, not by Endpoint: a Base URL and inference path are
/// per-Endpoint data that the factory reads once, at composition time, from the same Config
/// Version the executor pins. Keeping the factory a plain `fn` pointer keeps the table
/// allocation-free and `Copy`.
type P12EndpointAdapterFactory =
    fn(&EndpointConfiguration) -> Result<EndpointAdapter, RuntimeCompositionError>;

/// The `api_format`-to-adapter table this deployment build serves.
type P12ApiFormatAdapterRegistry = ApiFormatAdapterRegistry<P12EndpointAdapterFactory>;

/// Builds the fixed P12 `api_format` adapter table.
///
/// A format absent from this table is a format this build cannot serve: `endpoint_runtimes`
/// turns the missing binding into a composition failure for the whole Version rather than
/// skipping the Endpoint. Removing a binding here is therefore the exact, reviewable way to
/// narrow a deployment.
fn p12_api_format_adapter_registry() -> Result<P12ApiFormatAdapterRegistry, RuntimeCompositionError>
{
    ApiFormatAdapterRegistry::try_new([
        (
            ApiFormat::OpenAiChatCompletions,
            "openai-compatible.chat-completions",
            build_openai_chat_completions_adapter as P12EndpointAdapterFactory,
        ),
        (
            ApiFormat::OpenAiResponses,
            "openai-compatible.responses",
            build_openai_responses_adapter as P12EndpointAdapterFactory,
        ),
        (
            ApiFormat::OpenAiResponses,
            "grok.build.responses",
            build_grok_build_responses_adapter as P12EndpointAdapterFactory,
        ),
        (
            ApiFormat::OpenAiResponses,
            "grok.console.responses",
            build_grok_console_responses_adapter as P12EndpointAdapterFactory,
        ),
        (
            ApiFormat::OpenAiResponses,
            "grok.official.responses",
            build_grok_official_responses_adapter as P12EndpointAdapterFactory,
        ),
        (
            ApiFormat::OpenAiResponses,
            "grok.web.responses",
            build_grok_web_responses_adapter as P12EndpointAdapterFactory,
        ),
        (
            ApiFormat::AnthropicMessages,
            "anthropic-compatible.messages",
            build_anthropic_messages_adapter as P12EndpointAdapterFactory,
        ),
        // Kiro speaks Anthropic Messages on the wire but reaches it through its own credential
        // families, endpoint hosts and profileArn injection, so it is a second implementation of
        // the same format rather than a format of its own.
        (
            ApiFormat::AnthropicMessages,
            "kiro.messages",
            build_kiro_messages_adapter as P12EndpointAdapterFactory,
        ),
    ])
    .map_err(|_| RuntimeCompositionError::Unavailable)
}

fn build_openai_chat_completions_adapter(
    endpoint: &EndpointConfiguration,
) -> Result<EndpointAdapter, RuntimeCompositionError> {
    OpenAiChatCompletionsEndpoint::try_new(&endpoint.base_url, &endpoint.inference_path)
        .map(EndpointAdapter::OpenAiChatCompletions)
        .map_err(|_| RuntimeCompositionError::Unavailable)
}

fn build_openai_responses_adapter(
    endpoint: &EndpointConfiguration,
) -> Result<EndpointAdapter, RuntimeCompositionError> {
    OpenAiResponsesEndpoint::try_new(&endpoint.base_url, &endpoint.inference_path)
        .map(EndpointAdapter::OpenAiResponses)
        .map_err(|_| RuntimeCompositionError::Unavailable)
}

fn build_grok_build_responses_adapter(
    endpoint: &EndpointConfiguration,
) -> Result<EndpointAdapter, RuntimeCompositionError> {
    if composed_endpoint_url(endpoint) != GROK_BUILD_RESPONSES_URL
        || endpoint.base_url != GROK_BUILD_RESPONSES_BASE_URL
        || endpoint.inference_path != GROK_BUILD_RESPONSES_PATH
    {
        return Err(RuntimeCompositionError::Unavailable);
    }
    Ok(EndpointAdapter::GrokBuildResponses)
}

fn build_grok_console_responses_adapter(
    endpoint: &EndpointConfiguration,
) -> Result<EndpointAdapter, RuntimeCompositionError> {
    if composed_endpoint_url(endpoint) != GROK_CONSOLE_RESPONSES_URL
        || endpoint.base_url != GROK_CONSOLE_RESPONSES_BASE_URL
        || endpoint.inference_path != GROK_CONSOLE_RESPONSES_PATH
    {
        return Err(RuntimeCompositionError::Unavailable);
    }
    Ok(EndpointAdapter::GrokConsoleResponses)
}

fn build_grok_web_responses_adapter(
    endpoint: &EndpointConfiguration,
) -> Result<EndpointAdapter, RuntimeCompositionError> {
    if composed_endpoint_url(endpoint) != GROK_WEB_CANARY_URL
        || endpoint.base_url != GROK_WEB_PRODUCTION_BASE_URL
        || endpoint.inference_path != GROK_WEB_CANARY_PATH
    {
        return Err(RuntimeCompositionError::Unavailable);
    }
    Ok(EndpointAdapter::GrokWebResponses)
}

fn ordinary_provider_account_descriptors(
    configuration: &ControlPlaneConfiguration,
    native_endpoint_ids: &BTreeSet<EndpointId>,
) -> Result<Vec<ProviderAccountDescriptor>, RuntimeCompositionError> {
    let upstreams = configuration
        .upstreams
        .iter()
        .map(|upstream| (upstream.id.clone(), upstream))
        .collect::<BTreeMap<_, _>>();
    let endpoints = configuration
        .endpoints
        .iter()
        .map(|endpoint| (endpoint.id.clone(), endpoint))
        .collect::<BTreeMap<_, _>>();
    let credentials = configuration
        .credentials
        .iter()
        .map(|credential| (credential.id.clone(), credential))
        .collect::<BTreeMap<_, _>>();
    let models = provider_account_models(configuration);
    let mut descriptors = Vec::new();

    for binding in &configuration.endpoint_credential_bindings {
        if native_endpoint_ids.contains(&binding.endpoint_id) {
            continue;
        }
        let endpoint =
            endpoints
                .get(&binding.endpoint_id)
                .ok_or(RuntimeCompositionError::Stage(
                    RuntimeCompositionStage::ProviderAccountPool,
                ))?;
        let credential =
            credentials
                .get(&binding.credential_id)
                .ok_or(RuntimeCompositionError::Stage(
                    RuntimeCompositionStage::ProviderAccountPool,
                ))?;
        let upstream =
            upstreams
                .get(&binding.upstream_id)
                .ok_or(RuntimeCompositionError::Stage(
                    RuntimeCompositionStage::ProviderAccountPool,
                ))?;
        if endpoint.upstream_id != binding.upstream_id
            || credential.upstream_id != binding.upstream_id
        {
            return Err(RuntimeCompositionError::Stage(
                RuntimeCompositionStage::ProviderAccountPool,
            ));
        }
        let (auth_status, runtime_status_hint) = match credential.status {
            CredentialStatus::Active => (
                ProviderAccountAuthStatus::Active,
                ProviderAccountRuntimeStatus::Available,
            ),
            CredentialStatus::Cooling => (
                ProviderAccountAuthStatus::Active,
                ProviderAccountRuntimeStatus::Cooling,
            ),
            CredentialStatus::Unauthorized => (
                ProviderAccountAuthStatus::ReauthRequired,
                ProviderAccountRuntimeStatus::Unauthorized,
            ),
            CredentialStatus::Disabled => (
                ProviderAccountAuthStatus::Disabled,
                ProviderAccountRuntimeStatus::Available,
            ),
        };
        descriptors.push(ProviderAccountDescriptor {
            source: ProviderAccountDescriptorSource::Ordinary,
            provider_id: ProviderId::try_new(upstream.id.as_str().to_owned()).map_err(|_| {
                RuntimeCompositionError::Stage(RuntimeCompositionStage::ProviderAccountPool)
            })?,
            channel_id: endpoint.id.clone(),
            account_id: credential.id.clone(),
            account_kind: credential.kind.clone(),
            auth_status,
            runtime_status_hint,
            enabled: upstream.enabled
                && endpoint.enabled
                && binding.enabled
                && credential.status != CredentialStatus::Disabled,
            priority: binding.priority,
            weight: u32::try_from(binding.weight).map_err(|_| {
                RuntimeCompositionError::Stage(RuntimeCompositionStage::ProviderAccountPool)
            })?,
            max_concurrency: u32::try_from(binding.concurrency).map_err(|_| {
                RuntimeCompositionError::Stage(RuntimeCompositionStage::ProviderAccountPool)
            })?,
            expires_at_ms: None,
            refresh_due_at_ms: None,
            quota_sync_due_at_ms: None,
            upstream_models: models.get(&endpoint.id).cloned().unwrap_or_default(),
        });
    }
    Ok(descriptors)
}

fn native_provider_account_descriptors(
    configuration: &ControlPlaneConfiguration,
    bindings: &[GrokAccountEndpointBinding],
    accounts: &[GrokAccountMetadata],
) -> Result<Vec<ProviderAccountDescriptor>, RuntimeCompositionError> {
    let upstreams = configuration
        .upstreams
        .iter()
        .map(|upstream| (upstream.id.clone(), upstream))
        .collect::<BTreeMap<_, _>>();
    let endpoints = configuration
        .endpoints
        .iter()
        .map(|endpoint| (endpoint.id.clone(), endpoint))
        .collect::<BTreeMap<_, _>>();
    let models = provider_account_models(configuration);
    let mut endpoints_by_provider = BTreeMap::new();
    for binding in bindings {
        if endpoints_by_provider
            .insert(binding.provider(), binding.endpoint_id().clone())
            .is_some()
        {
            return Err(RuntimeCompositionError::Stage(
                RuntimeCompositionStage::ProviderAccountPool,
            ));
        }
    }

    let mut descriptors = Vec::new();
    for account in accounts {
        let Some(endpoint_id) = endpoints_by_provider.get(&account.provider) else {
            continue;
        };
        let endpoint = endpoints
            .get(endpoint_id)
            .ok_or(RuntimeCompositionError::Stage(
                RuntimeCompositionStage::ProviderAccountPool,
            ))?;
        let upstream =
            upstreams
                .get(&endpoint.upstream_id)
                .ok_or(RuntimeCompositionError::Stage(
                    RuntimeCompositionStage::ProviderAccountPool,
                ))?;
        let auth_status = match account.auth_status {
            GrokAccountAuthStatus::Active => ProviderAccountAuthStatus::Active,
            GrokAccountAuthStatus::ReauthRequired => ProviderAccountAuthStatus::ReauthRequired,
            GrokAccountAuthStatus::Disabled => ProviderAccountAuthStatus::Disabled,
        };
        let runtime_status_hint = if auth_status == ProviderAccountAuthStatus::ReauthRequired {
            ProviderAccountRuntimeStatus::Unauthorized
        } else {
            ProviderAccountRuntimeStatus::Available
        };
        descriptors.push(ProviderAccountDescriptor {
            source: ProviderAccountDescriptorSource::Native,
            provider_id: ProviderId::try_new(upstream.id.as_str().to_owned()).map_err(|_| {
                RuntimeCompositionError::Stage(RuntimeCompositionStage::ProviderAccountPool)
            })?,
            channel_id: endpoint.id.clone(),
            account_id: CredentialId::try_new(account.id.clone()).map_err(|_| {
                RuntimeCompositionError::Stage(RuntimeCompositionStage::ProviderAccountPool)
            })?,
            account_kind: grok_account_kind(account.provider).to_owned(),
            auth_status,
            runtime_status_hint,
            enabled: upstream.enabled
                && endpoint.enabled
                && account.enabled
                && auth_status != ProviderAccountAuthStatus::Disabled,
            priority: 1_000_i64.checked_sub(account.priority).ok_or(
                RuntimeCompositionError::Stage(RuntimeCompositionStage::ProviderAccountPool),
            )?,
            weight: account.weight,
            max_concurrency: account.max_concurrency,
            // Build expiry is retained by the compiled pool diagnostic entry. Web/Console
            // lifetimes remain Provider-managed metadata and therefore stay unknown here.
            expires_at_ms: None,
            refresh_due_at_ms: account.refresh_due_at_ms,
            quota_sync_due_at_ms: account.quota_sync_due_at_ms,
            upstream_models: models.get(endpoint_id).cloned().unwrap_or_default(),
        });
    }
    Ok(descriptors)
}

fn provider_account_pool_facade(
    config_version_id: String,
    descriptors: Result<Vec<ProviderAccountDescriptor>, RuntimeCompositionError>,
    credential_pools: Arc<EndpointCredentialPools>,
    runtime_health: Arc<RuntimeHealthRegistry>,
    runtime_quota: Arc<RuntimeQuotaRegistry>,
) -> Box<dyn ProviderAccountPoolFacade> {
    descriptors
        .and_then(|descriptors| {
            ProviderAccountPoolAdapter::try_new(
                descriptors,
                credential_pools,
                runtime_health,
                runtime_quota,
                Arc::new(SystemProviderAccountPoolClock),
                P13_PROVIDER_ACCOUNT_POOL_SNAPSHOT_TTL,
                P13_PROVIDER_ACCOUNT_POOL_CURSOR_RETENTION,
            )
            .and_then(|adapter| adapter.with_config_version(config_version_id))
            .map_err(|_| {
                RuntimeCompositionError::Stage(RuntimeCompositionStage::ProviderAccountPool)
            })
        })
        .map_or_else(
            |_| Box::new(RejectingProviderAccountPoolFacade::new()) as Box<_>,
            |adapter| Box::new(adapter) as Box<_>,
        )
}

fn provider_account_models(
    configuration: &ControlPlaneConfiguration,
) -> BTreeMap<EndpointId, Vec<String>> {
    let mut models: BTreeMap<EndpointId, BTreeSet<String>> = BTreeMap::new();
    for candidate in &configuration.route_candidates {
        if candidate.enabled {
            models
                .entry(candidate.endpoint_id.clone())
                .or_default()
                .insert(candidate.upstream_model.clone());
        }
    }
    models
        .into_iter()
        .map(|(endpoint_id, models)| (endpoint_id, models.into_iter().collect()))
        .collect()
}

const fn grok_account_kind(provider: GrokAccountProvider) -> &'static str {
    match provider {
        GrokAccountProvider::Build => "grok_build_oauth",
        GrokAccountProvider::Web => "grok_web_sso",
        GrokAccountProvider::Console => "grok_console_sso",
    }
}

fn native_grok_provider_for_endpoint(
    endpoint: &EndpointConfiguration,
) -> Option<GrokAccountProvider> {
    match endpoint.adapter_id.as_str() {
        "grok.build.responses" => Some(GrokAccountProvider::Build),
        "grok.console.responses" => Some(GrokAccountProvider::Console),
        "grok.web.responses" => Some(GrokAccountProvider::Web),
        _ => None,
    }
}

fn is_native_grok_endpoint(endpoint: &EndpointConfiguration) -> bool {
    native_grok_provider_for_endpoint(endpoint).is_some()
}

fn build_grok_official_responses_adapter(
    endpoint: &EndpointConfiguration,
) -> Result<EndpointAdapter, RuntimeCompositionError> {
    if composed_endpoint_url(endpoint) != GROK_OFFICIAL_RESPONSES_URL
        || endpoint.base_url != GROK_OFFICIAL_API_BASE_URL
        || endpoint.inference_path != GROK_OFFICIAL_RESPONSES_PATH
    {
        return Err(RuntimeCompositionError::Unavailable);
    }
    Ok(EndpointAdapter::GrokOfficialResponses)
}

fn composed_endpoint_url(endpoint: &EndpointConfiguration) -> String {
    format!(
        "{}{}",
        endpoint.base_url.trim_end_matches('/'),
        endpoint.inference_path
    )
}

fn build_anthropic_messages_adapter(
    endpoint: &EndpointConfiguration,
) -> Result<EndpointAdapter, RuntimeCompositionError> {
    AnthropicMessagesEndpoint::try_new(&endpoint.base_url, &endpoint.inference_path)
        .map(EndpointAdapter::AnthropicMessages)
        .map_err(|_| RuntimeCompositionError::Unavailable)
}

/// Builds the Kiro binding for one Endpoint.
///
/// Kiro derives its own host from an endpoint kind and API Region rather than accepting a free-form
/// `base_url`, so the stored `base_url` and `inference_path` must equal what the policy derives.
/// Requiring equality keeps one Endpoint row describing exactly one reachable upstream: an operator
/// cannot point a Kiro Endpoint at an arbitrary host, and cannot silently disagree with the URL the
/// request builder will actually use.
fn build_kiro_messages_adapter(
    endpoint: &EndpointConfiguration,
) -> Result<EndpointAdapter, RuntimeCompositionError> {
    let (kind, region) = p12_kiro_endpoint_shape(endpoint)?;
    let policy = KiroEndpointPolicy::try_new(kind, region)
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
    // Kiro's policy derives one complete URL from the endpoint kind and Region — host *and* path.
    // Require the stored `base_url` + `inference_path` to denote exactly that URL, so one Endpoint
    // row describes exactly one reachable upstream and cannot disagree with the address the request
    // builder will actually use. Trailing slashes are normalised because `Url` renders an empty
    // path as "/"; doing that by hand avoids giving the composition root a `url` dependency edge.
    let stored = format!(
        "{}{}",
        endpoint.base_url.trim_end_matches('/'),
        endpoint.inference_path
    );
    if stored.trim_end_matches('/') != policy.url().as_str().trim_end_matches('/') {
        return Err(RuntimeCompositionError::Unavailable);
    }
    Ok(EndpointAdapter::KiroMessages(policy))
}

/// Reads the Kiro endpoint kind and API Region out of one Endpoint row.
///
/// The kind lives in `inference_path` because that is the only per-Endpoint free-form field this
/// build has for it, and the Region is parsed from the derived host in `base_url`. Both are
/// validated here so an unparseable pair fails the whole Version at composition rather than at the
/// first request.
fn p12_kiro_endpoint_shape(
    endpoint: &EndpointConfiguration,
) -> Result<(KiroEndpointKind, KiroApiRegion), RuntimeCompositionError> {
    let kind = match endpoint.inference_path.as_str() {
        "/generateAssistantResponse" => KiroEndpointKind::Ide,
        "/" => KiroEndpointKind::Cli,
        _ => return Err(RuntimeCompositionError::Unavailable),
    };
    let host = endpoint
        .base_url
        .strip_prefix("https://")
        .ok_or(RuntimeCompositionError::Unavailable)?
        .split('/')
        .next()
        .ok_or(RuntimeCompositionError::Unavailable)?;
    let region = match kind {
        KiroEndpointKind::Ide => host
            .strip_prefix("q.")
            .and_then(|rest| rest.strip_suffix(".amazonaws.com")),
        KiroEndpointKind::Cli => host
            .strip_prefix("runtime.")
            .and_then(|rest| rest.strip_suffix(".kiro.dev")),
    }
    .ok_or(RuntimeCompositionError::Unavailable)?;
    let region =
        KiroApiRegion::try_new(region).map_err(|_| RuntimeCompositionError::Unavailable)?;
    Ok((kind, region))
}

/// Builds one runtime per configured Endpoint, sharing the fixed transports and DNS resolver.
///
/// The transport profiles are configuration-free constants, so every Endpoint shares one
/// instance; each Endpoint still composes its own base URL and inference path, resolves its
/// upstream's compiled egress policy, and binds the adapter its declared `api_format` selects.
/// Every Snapshot Candidate must reference a configured Endpoint, and a configured Endpoint no
/// Candidate references must still conform: admission is version-level review, not best-effort
/// filtering. Binding happens here, once, against the same Config Version and Snapshot the
/// executor pins, so no attempt can later reach an adapter from a different Version.
fn endpoint_runtimes(
    configuration: &ControlPlaneConfiguration,
    snapshot: &RouteSnapshot,
    policies: &gateway_control::egress_policy_compiler::CompiledEgressPolicies,
    registry: &P12ApiFormatAdapterRegistry,
    web_proxy: Option<UpstreamProxy>,
    flaresolverr_proxy: Option<UpstreamProxy>,
    flaresolverr_port: u16,
) -> Result<BTreeMap<EndpointId, EndpointRuntime>, RuntimeCompositionError> {
    let configured_ids = configuration
        .endpoints
        .iter()
        .map(|endpoint| endpoint.id.clone())
        .collect::<BTreeSet<_>>();
    let candidate_endpoint_ids = snapshot
        .routes()
        .flat_map(gateway_router::SnapshotRoute::candidates)
        .map(|candidate| candidate.endpoint_id().clone())
        .collect::<BTreeSet<_>>();
    if !candidate_endpoint_ids.is_subset(&configured_ids) {
        return Err(RuntimeCompositionError::Unavailable);
    }
    let resolver: Arc<dyn EgressDnsResolver> = Arc::new(SystemEgressDnsResolver);
    let transports = Arc::new(P12TransportProfiles::try_new_with_web_proxy(
        web_proxy.unwrap_or(UpstreamProxy::Direct),
        flaresolverr_proxy,
        flaresolverr_port,
    )?);
    let mut runtimes = BTreeMap::new();
    for configured in &configuration.endpoints {
        let format = validate_endpoint_shape(configured)?;
        let build = registry
            .adapter(&configured.adapter_id)
            .ok_or(RuntimeCompositionError::Unavailable)?;
        let policy = policies
            .policy_for_upstream(&configured.upstream_id)
            .cloned()
            .ok_or(RuntimeCompositionError::Unavailable)?;
        let adapter = build(configured)?;
        // The registry is generic over an opaque adapter value, so a mis-ordered table would
        // compile and bind every Endpoint to the other protocol's adapter, surfacing only as a
        // per-request internal error. Prove the correspondence here instead, where a wrong
        // binding fails the whole Version at composition.
        if adapter.api_format() != format {
            return Err(RuntimeCompositionError::Unavailable);
        }
        if runtimes
            .insert(
                configured.id.clone(),
                EndpointRuntime {
                    adapter,
                    policy,
                    resolver: Arc::clone(&resolver),
                    transports: Arc::clone(&transports),
                    web_statsig: OnceLock::new(),
                },
            )
            .is_some()
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    Ok(runtimes)
}

type CompatibleTransportRegistries =
    BTreeMap<gateway_core::UpstreamId, CompatibleEgressTransportRegistry>;
type CompatibleBindingSettings =
    BTreeMap<(EndpointId, CredentialId), CompatibleEndpointBindingRuntimeSettings>;
type CompatibleEgressRuntimeInputs = (CompatibleTransportRegistries, CompatibleBindingSettings);

/// Builds the immutable transport registries and exact binding settings for generic Upstreams.
///
/// This is deliberately a composition-time operation. It opens each enabled node exactly once
/// under the Config-Version/Upstream/pool/node AAD tuple, converts the redacted proxy directly
/// into an `UpstreamTransportProfile`, and then drops the plaintext. No Store read, decryption,
/// DNS lookup, or environment-proxy lookup remains on the serving hot path.
fn compatible_egress_runtime_inputs(
    configuration: &ControlPlaneConfiguration,
    endpoints: &BTreeMap<EndpointId, EndpointRuntime>,
    secret_store: &SecretStore,
) -> Result<CompatibleEgressRuntimeInputs, RuntimeCompositionError> {
    let generic_upstreams = compatible_generic_upstreams(configuration);
    validate_compatible_resource_ownership(configuration, &generic_upstreams)?;
    let registries = compatible_transport_registries(
        configuration,
        endpoints,
        secret_store,
        &generic_upstreams,
    )?;
    let settings = compatible_binding_runtime_settings(configuration, &registries)?;
    Ok((registries, settings))
}

fn compatible_generic_upstreams(
    configuration: &ControlPlaneConfiguration,
) -> BTreeSet<gateway_core::UpstreamId> {
    configuration
        .endpoints
        .iter()
        .filter(|endpoint| endpoint.enabled && is_generic_compatible_adapter(&endpoint.adapter_id))
        .filter_map(|endpoint| {
            configuration
                .upstreams
                .iter()
                .find(|upstream| upstream.id == endpoint.upstream_id)
                .filter(|upstream| upstream.enabled)
                .map(|_| endpoint.upstream_id.clone())
        })
        .collect()
}

fn validate_compatible_resource_ownership(
    configuration: &ControlPlaneConfiguration,
    generic_upstreams: &BTreeSet<gateway_core::UpstreamId>,
) -> Result<(), RuntimeCompositionError> {
    // Compatible resources are a generic-adapter contract. Do not silently retain an enabled
    // pool/node for a native Provider and hope a later path interprets it; that would make the
    // active graph appear healthy while the resource is unreachable. Disabled draft rows remain
    // inert until an operator explicitly enables them.
    for pool in configuration
        .compatible_proxy_pools
        .iter()
        .filter(|pool| pool.enabled)
    {
        if !generic_upstreams.contains(&pool.upstream_id) {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    for node in configuration
        .compatible_proxy_nodes
        .iter()
        .filter(|node| node.enabled)
    {
        if !generic_upstreams.contains(&node.upstream_id) {
            return Err(RuntimeCompositionError::Unavailable);
        }
        if let Some(pool_id) = &node.pool_id {
            let pool = configuration
                .compatible_proxy_pools
                .iter()
                .find(|pool| pool.id == *pool_id)
                .ok_or(RuntimeCompositionError::Unavailable)?;
            if pool.upstream_id != node.upstream_id || !pool.enabled {
                return Err(RuntimeCompositionError::Unavailable);
            }
        } else if node.weight != 1 {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    Ok(())
}

fn compatible_transport_registries(
    configuration: &ControlPlaneConfiguration,
    endpoints: &BTreeMap<EndpointId, EndpointRuntime>,
    secret_store: &SecretStore,
    generic_upstreams: &BTreeSet<gateway_core::UpstreamId>,
) -> Result<CompatibleTransportRegistries, RuntimeCompositionError> {
    let mut registries = BTreeMap::new();
    for upstream_id in generic_upstreams {
        let representative = configuration
            .endpoints
            .iter()
            .filter(|endpoint| {
                endpoint.enabled
                    && endpoint.upstream_id == *upstream_id
                    && is_generic_compatible_adapter(&endpoint.adapter_id)
            })
            .min_by(|left, right| left.id.cmp(&right.id))
            .ok_or(RuntimeCompositionError::Unavailable)?;
        let runtime = endpoints
            .get(&representative.id)
            .ok_or(RuntimeCompositionError::Unavailable)?;
        let direct_profile = runtime
            .transports
            .for_mode(ResponsesResponseMode::NonStreaming)
            .clone();
        let fixed_proxies = compatible_fixed_proxy_inputs(
            configuration,
            secret_store,
            upstream_id,
            &direct_profile,
        )?;
        let proxy_pools = compatible_proxy_pool_inputs(
            configuration,
            secret_store,
            upstream_id,
            &direct_profile,
        )?;

        let registry =
            CompatibleEgressTransportRegistry::try_new(CompatibleEgressTransportRegistryInput {
                owner_upstream_id: upstream_id.clone(),
                direct_profile,
                fixed_proxies,
                proxy_pools,
            })
            .map_err(|_| RuntimeCompositionError::Unavailable)?;
        registries.insert(upstream_id.clone(), registry);
    }
    Ok(registries)
}

fn compatible_fixed_proxy_inputs(
    configuration: &ControlPlaneConfiguration,
    secret_store: &SecretStore,
    upstream_id: &gateway_core::UpstreamId,
    direct_profile: &UpstreamTransportProfile,
) -> Result<Vec<CompatibleFixedProxyInput>, RuntimeCompositionError> {
    configuration
        .compatible_proxy_nodes
        .iter()
        .filter(|node| node.upstream_id == *upstream_id && node.enabled && node.pool_id.is_none())
        .map(|node| {
            let proxy = open_compatible_proxy_node_endpoint(
                secret_store,
                &configuration.version.id,
                upstream_id,
                None,
                &node.id,
                &node.encrypted_proxy,
            )
            .map_err(|_| RuntimeCompositionError::Unavailable)?;
            Ok(CompatibleFixedProxyInput {
                profile_id: node.id.as_str().to_owned(),
                transport_profile: direct_profile.clone().with_proxy(proxy),
                maximum_concurrency: usize::try_from(node.maximum_concurrency)
                    .map_err(|_| RuntimeCompositionError::Unavailable)?,
            })
        })
        .collect()
}

fn compatible_proxy_pool_inputs(
    configuration: &ControlPlaneConfiguration,
    secret_store: &SecretStore,
    upstream_id: &gateway_core::UpstreamId,
    direct_profile: &UpstreamTransportProfile,
) -> Result<Vec<CompatibleProxyPoolInput>, RuntimeCompositionError> {
    configuration
        .compatible_proxy_pools
        .iter()
        .filter(|pool| pool.upstream_id == *upstream_id && pool.enabled)
        .map(|pool| {
            let nodes = configuration
                .compatible_proxy_nodes
                .iter()
                .filter(|node| {
                    node.upstream_id == *upstream_id
                        && node.enabled
                        && node.pool_id.as_ref() == Some(&pool.id)
                })
                .map(|node| {
                    let proxy = open_compatible_proxy_node_endpoint(
                        secret_store,
                        &configuration.version.id,
                        upstream_id,
                        Some(&pool.id),
                        &node.id,
                        &node.encrypted_proxy,
                    )
                    .map_err(|_| RuntimeCompositionError::Unavailable)?;
                    Ok(CompatibleEgressNodeInput {
                        node_id: node.id.as_str().to_owned(),
                        transport_profile: direct_profile.clone().with_proxy(proxy),
                        weight: usize::from(node.weight),
                        maximum_concurrency: usize::try_from(node.maximum_concurrency)
                            .map_err(|_| RuntimeCompositionError::Unavailable)?,
                    })
                })
                .collect::<Result<Vec<_>, RuntimeCompositionError>>()?;
            // An enabled pool is an active graph dependency. Publishing an empty pool would
            // otherwise turn a configured binding into an implicit unavailable/Direct fallback.
            if nodes.is_empty() {
                return Err(RuntimeCompositionError::Unavailable);
            }
            Ok(CompatibleProxyPoolInput {
                pool_id: pool.id.as_str().to_owned(),
                nodes,
            })
        })
        .collect()
}

fn compatible_binding_runtime_settings(
    configuration: &ControlPlaneConfiguration,
    registries: &CompatibleTransportRegistries,
) -> Result<CompatibleBindingSettings, RuntimeCompositionError> {
    let mut settings_by_binding = BTreeMap::new();
    for binding in &configuration.compatible_egress_bindings {
        let endpoint = configuration
            .endpoints
            .iter()
            .find(|endpoint| endpoint.id == binding.endpoint_id)
            .ok_or(RuntimeCompositionError::Unavailable)?;
        if !endpoint.enabled || !is_generic_compatible_adapter(&endpoint.adapter_id) {
            return Err(RuntimeCompositionError::Unavailable);
        }
        let credential = configuration
            .credentials
            .iter()
            .find(|credential| credential.id == binding.credential_id)
            .ok_or(RuntimeCompositionError::Unavailable)?;
        if credential.upstream_id != endpoint.upstream_id {
            return Err(RuntimeCompositionError::Unavailable);
        }
        let exact_binding_exists = configuration
            .endpoint_credential_bindings
            .iter()
            .any(|item| {
                item.endpoint_id == binding.endpoint_id
                    && item.credential_id == binding.credential_id
                    && item.upstream_id == endpoint.upstream_id
            });
        if !exact_binding_exists {
            return Err(RuntimeCompositionError::Unavailable);
        }
        let target = compatible_binding_target(configuration, endpoint, &binding.target)?;
        let failure_scope = match binding.failure_scope {
            StoredCompatibleFailureScope::Endpoint => CompatibleFailureScope::Endpoint,
            StoredCompatibleFailureScope::Credential => CompatibleFailureScope::Credential,
            StoredCompatibleFailureScope::EgressNode => CompatibleFailureScope::EgressNode,
        };
        let stickiness = match binding.stickiness {
            StoredCompatibleStickiness::None => CompatibleStickiness::None,
            StoredCompatibleStickiness::Credential => CompatibleStickiness::Credential,
            StoredCompatibleStickiness::CredentialAndEgress => {
                CompatibleStickiness::CredentialAndEgress
            }
        };
        let retry_policy = if binding.pre_submit_max_attempts == 1 {
            CompatibleRetryPolicy::None
        } else {
            CompatibleRetryPolicy::pre_submit(binding.pre_submit_max_attempts)
                .map_err(|_| RuntimeCompositionError::Unavailable)?
        };
        let settings = CompatibleEndpointBindingRuntimeSettings {
            target,
            failure_scope,
            stickiness,
            retry_policy,
        };
        let registry = registries
            .get(&endpoint.upstream_id)
            .ok_or(RuntimeCompositionError::Unavailable)?;
        if !registry.contains_target(&settings.target) {
            return Err(RuntimeCompositionError::Unavailable);
        }
        if settings_by_binding
            .insert(
                (binding.endpoint_id.clone(), binding.credential_id.clone()),
                settings,
            )
            .is_some()
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    Ok(settings_by_binding)
}

fn compatible_binding_target(
    configuration: &ControlPlaneConfiguration,
    endpoint: &EndpointConfiguration,
    target: &CompatibleEgressTargetConfiguration,
) -> Result<CompatibleEgressTarget, RuntimeCompositionError> {
    match target {
        CompatibleEgressTargetConfiguration::Direct => Ok(CompatibleEgressTarget::Direct),
        CompatibleEgressTargetConfiguration::FixedProxy(node_id) => {
            let node = configuration
                .compatible_proxy_nodes
                .iter()
                .find(|node| node.id == *node_id)
                .ok_or(RuntimeCompositionError::Unavailable)?;
            if node.upstream_id != endpoint.upstream_id || node.pool_id.is_some() || !node.enabled {
                return Err(RuntimeCompositionError::Unavailable);
            }
            Ok(CompatibleEgressTarget::FixedProxy {
                profile_id: node.id.as_str().to_owned(),
            })
        }
        CompatibleEgressTargetConfiguration::ProxyPool(pool_id) => {
            let pool = configuration
                .compatible_proxy_pools
                .iter()
                .find(|pool| pool.id == *pool_id)
                .ok_or(RuntimeCompositionError::Unavailable)?;
            if pool.upstream_id != endpoint.upstream_id || !pool.enabled {
                return Err(RuntimeCompositionError::Unavailable);
            }
            Ok(CompatibleEgressTarget::ProxyPool {
                pool_id: pool.id.as_str().to_owned(),
            })
        }
    }
}

fn is_generic_compatible_adapter(adapter_id: &str) -> bool {
    matches!(
        adapter_id,
        "openai-compatible.chat-completions"
            | "openai-compatible.responses"
            | "anthropic-compatible.messages"
    )
}

/// Narrows this composition to the reviewed production graph shape before a Secret can be
/// opened or an outbound request can be constructed.
///
/// The shape is no longer singleton: any number of upstreams, Endpoints, weighted Credential
/// bindings, aliases, public models, Routes, Candidates, and Client Keys are admitted.  What
/// stays fixed is fail-closed conformance: HTTPS-only egress policies, Bearer-only active
/// Credentials, Endpoints whose `adapter_id` and `api_format` form a pair this build binds an
/// adapter for, Canonical Candidates, bounded attempt budgets, and a bounded total Credential
/// concurrency.  One non-conforming row fails admission for the whole Version instead of serving
/// a subset.
fn validate_p12_required_resources(
    configuration: &ControlPlaneConfiguration,
) -> Result<(), RuntimeCompositionError> {
    let has_native_grok_endpoint = configuration.endpoints.iter().any(is_native_grok_endpoint);
    if configuration.version.status != ConfigVersionStatus::Active
        || configuration.egress_policies.is_empty()
        || configuration.upstreams.is_empty()
        || configuration.endpoints.is_empty()
        || (!has_native_grok_endpoint && configuration.credentials.is_empty())
        || (!has_native_grok_endpoint && configuration.endpoint_credential_bindings.is_empty())
        || configuration.public_models.is_empty()
        || configuration.model_routes.is_empty()
        || configuration.route_candidates.is_empty()
        || configuration.access_groups.is_empty()
        || configuration.access_group_routes.is_empty()
        || configuration.client_keys.is_empty()
    {
        return Err(RuntimeCompositionError::Unavailable);
    }
    Ok(())
}

fn validate_p12_network_shape(
    configuration: &ControlPlaneConfiguration,
) -> Result<(), RuntimeCompositionError> {
    if !configuration
        .egress_policies
        .iter()
        .all(has_p12_https_only_egress_shape)
    {
        return Err(RuntimeCompositionError::Unavailable);
    }
    let policy_ids = configuration
        .egress_policies
        .iter()
        .map(|policy| &policy.id)
        .collect::<BTreeSet<_>>();
    for upstream in &configuration.upstreams {
        if !upstream.enabled
            || upstream
                .egress_policy_id
                .as_ref()
                .is_none_or(|policy_id| !policy_ids.contains(policy_id))
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    let upstream_ids = configuration
        .upstreams
        .iter()
        .map(|upstream| &upstream.id)
        .collect::<BTreeSet<_>>();
    for endpoint in &configuration.endpoints {
        validate_endpoint_shape(endpoint)?;
        if !upstream_ids.contains(&endpoint.upstream_id) {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    Ok(())
}

fn validate_p12_credential_bindings(
    configuration: &ControlPlaneConfiguration,
) -> Result<(), RuntimeCompositionError> {
    let upstream_ids = configuration
        .upstreams
        .iter()
        .map(|upstream| &upstream.id)
        .collect::<BTreeSet<_>>();
    for credential in &configuration.credentials {
        // `oauth_json` is CPAR's encrypted, normalized Codex OAuth envelope.  It is
        // deliberately admitted at composition time alongside the incumbent opaque bearer
        // shape; the request boundary still runs the strict importer and expiry/account-binding
        // checks before a byte can become an Authorization header.  Keeping this admission here
        // is the important distinction between a persisted OAuth rotation and a startup outage.
        if !matches!(credential.kind.as_str(), "bearer" | "oauth_json")
            || credential.status != CredentialStatus::Active
            || !upstream_ids.contains(&credential.upstream_id)
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    let endpoint_upstreams = configuration
        .endpoints
        .iter()
        .map(|endpoint| (&endpoint.id, &endpoint.upstream_id))
        .collect::<BTreeMap<_, _>>();
    let credential_upstreams = configuration
        .credentials
        .iter()
        .map(|credential| (&credential.id, &credential.upstream_id))
        .collect::<BTreeMap<_, _>>();
    let mut total_concurrency: i64 = 0;
    for binding in &configuration.endpoint_credential_bindings {
        let endpoint_upstream = endpoint_upstreams
            .get(&binding.endpoint_id)
            .copied()
            .ok_or(RuntimeCompositionError::Unavailable)?;
        let credential_upstream = credential_upstreams
            .get(&binding.credential_id)
            .copied()
            .ok_or(RuntimeCompositionError::Unavailable)?;
        if !binding.enabled
            || binding.priority < 0
            || binding.weight < 1
            || binding.concurrency < 1
            || endpoint_upstream != &binding.upstream_id
            || credential_upstream != &binding.upstream_id
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
        total_concurrency = total_concurrency.saturating_add(binding.concurrency);
    }
    if total_concurrency > P12_MAX_TOTAL_BINDING_CONCURRENCY {
        return Err(RuntimeCompositionError::Unavailable);
    }
    Ok(())
}

fn validate_p12_route_access_shape(
    configuration: &ControlPlaneConfiguration,
) -> Result<(), RuntimeCompositionError> {
    for model in &configuration.public_models {
        if model.status != gateway_store::control_plane::AdministrativeStatus::Active
            || !is_empty_capability_object(&model.capabilities_json)
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    let model_ids = configuration
        .public_models
        .iter()
        .map(|model| &model.id)
        .collect::<BTreeSet<_>>();
    for route in &configuration.model_routes {
        if !model_ids.contains(&route.public_model_id)
            || route.policy != RoutePolicy::SmoothWeightedRoundRobin
            || !usize::try_from(route.max_attempts)
                .is_ok_and(|attempts| (1..=P12_MAX_ROUTE_ATTEMPTS).contains(&attempts))
            || route.bootstrap_timeout_ms <= 0
            || route.bootstrap_timeout_ms > P12_BOOTSTRAP_TIMEOUT_MILLISECONDS
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    let route_ids = configuration
        .model_routes
        .iter()
        .map(|route| &route.id)
        .collect::<BTreeSet<_>>();
    let endpoint_ids = configuration
        .endpoints
        .iter()
        .map(|endpoint| &endpoint.id)
        .collect::<BTreeSet<_>>();
    let endpoint_adapters = configuration
        .endpoints
        .iter()
        .map(|endpoint| (&endpoint.id, endpoint.adapter_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    for candidate in &configuration.route_candidates {
        let adapter_id = endpoint_adapters
            .get(&candidate.endpoint_id)
            .copied()
            .ok_or(RuntimeCompositionError::Unavailable)?;
        if !route_ids.contains(&candidate.route_id)
            || !endpoint_ids.contains(&candidate.endpoint_id)
            || candidate.credential_scope != CredentialScope::EndpointBindings
            || (adapter_id == "kiro.messages"
                && candidate.transform_mode != TransformMode::Canonical)
            || !candidate.enabled
            || candidate.priority < 0
            || candidate.weight < 1
            || !p12_candidate_override_is_admissible(
                adapter_id,
                &candidate.capability_override_json,
            )
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    for group in &configuration.access_groups {
        if group.status != gateway_store::control_plane::AdministrativeStatus::Active
            || !is_empty_capability_object(&group.limits_json)
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    let group_ids = configuration
        .access_groups
        .iter()
        .map(|group| &group.id)
        .collect::<BTreeSet<_>>();
    for binding in &configuration.access_group_routes {
        if !group_ids.contains(&binding.access_group_id)
            || !route_ids.contains(&binding.route_id)
            || !binding.enabled
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    for key in &configuration.client_keys {
        if !group_ids.contains(key.access_group_id())
            || key.status() != StoredClientKeyStatus::Active
        {
            return Err(RuntimeCompositionError::Unavailable);
        }
    }
    Ok(())
}

fn has_p12_https_only_egress_shape(
    policy: &gateway_store::control_plane::EgressPolicyConfiguration,
) -> bool {
    let allowed_schemes = serde_json::from_str::<Vec<String>>(&policy.allowed_schemes_json);
    let allowed_hosts = serde_json::from_str::<Vec<String>>(&policy.allowed_hosts_json);
    let allowed_ports = serde_json::from_str::<Vec<u16>>(&policy.allowed_ports_json);
    let allowed_cidrs = serde_json::from_str::<Vec<String>>(&policy.allowed_cidrs_json);
    matches!(allowed_schemes, Ok(schemes) if schemes.as_slice() == ["https"])
        && matches!(allowed_hosts, Ok(hosts) if !hosts.is_empty())
        && matches!(allowed_ports, Ok(ports) if !ports.is_empty())
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

/// Returns whether a P12 Candidate carries a bounded capability override admitted by the runtime
/// shape gate. A candidate may either opt into an unlisted upstream model, or explicitly narrow
/// Reasoning while keeping that model admission. The latter is needed when a Responses Endpoint is
/// exposed through a Chat bridge: it is a capability subtraction, so it cannot manufacture a
/// private-reasoning event that Chat cannot represent. Native Grok routes use the same shape.
fn p12_candidate_override_is_admissible(adapter_id: &str, value: &str) -> bool {
    has_p12_unlisted_model_override(value)
        || (matches!(
            adapter_id,
            "openai-compatible.responses" | "grok.build.responses" | "grok.console.responses"
        ) && matches!(
            serde_json::from_str::<Value>(value),
            Ok(Value::Object(object))
                if object.len() == 2
                    && object.get("allow_unlisted_model") == Some(&Value::Bool(true))
                    && object.get("reasoning") == Some(&Value::Bool(false))
        ))
}

/// Returns whether one `adapter_id` may serve an admitted API Format in this composition.
///
/// `adapter_id` names an implementation while `api_format` names a wire protocol, and the store
/// keeps both free-form, so admission checks the declared pair against the product's own table. A
/// graph that declares a serving format under a foreign implementation label fails admission
/// instead of being served by whichever adapter the format alone would select.
fn p12_adapter_id_serves(format: ApiFormat, adapter_id: &str) -> bool {
    // One source of truth with the Route Compiler's publish-time gate, so an Endpoint this
    // composition would refuse can never be published in the first place.
    format.serves(adapter_id)
}

fn validate_endpoint_shape(
    endpoint: &EndpointConfiguration,
) -> Result<ApiFormat, RuntimeCompositionError> {
    let Some(format) = ApiFormat::parse(&endpoint.api_format) else {
        return Err(RuntimeCompositionError::Unavailable);
    };
    if !endpoint.enabled
        || !p12_adapter_id_serves(format, &endpoint.adapter_id)
        || endpoint.transport != EndpointTransport::Http
    {
        return Err(RuntimeCompositionError::Unavailable);
    }
    Ok(format)
}

/// One Endpoint's declared-format adapter plus the egress and transport state it executes on.
struct EndpointRuntime {
    adapter: EndpointAdapter,
    policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    transports: Arc<P12TransportProfiles>,
    web_statsig: OnceLock<Result<Arc<GrokWebStatsigRuntime>, GatewayError>>,
}

/// The per-Endpoint execution binding selected by that Endpoint's declared `api_format`.
///
/// One variant per format this build serves. The enum is exhaustive on purpose: adding an
/// [`ApiFormat`] without adding an arm here fails to compile rather than falling back to a
/// neighbouring protocol.
enum EndpointAdapter {
    /// A native OpenAI-compatible Chat Completions path.
    OpenAiChatCompletions(OpenAiChatCompletionsEndpoint),
    /// The unchanged `OpenAI`-compatible Responses path.
    OpenAiResponses(OpenAiResponsesEndpoint),
    /// The fixed Grok Build OAuth Responses path.
    GrokBuildResponses,
    /// The fixed Grok Console SSO Responses path.
    GrokConsoleResponses,
    /// The fixed Grok Web SSO conversation path.
    GrokWebResponses,
    /// The fixed xAI Official API-key Responses path.
    GrokOfficialResponses,
    /// The Anthropic-compatible Messages path.
    AnthropicMessages(AnthropicMessagesEndpoint),
    /// The Kiro path, which serves Anthropic Messages from a derived Kiro host.
    KiroMessages(KiroEndpointPolicy),
}

impl EndpointAdapter {
    /// Returns the exact API Format this binding serves.
    const fn api_format(&self) -> ApiFormat {
        match self {
            Self::OpenAiChatCompletions(_) => ApiFormat::OpenAiChatCompletions,
            Self::OpenAiResponses(_)
            | Self::GrokBuildResponses
            | Self::GrokConsoleResponses
            | Self::GrokWebResponses
            | Self::GrokOfficialResponses => ApiFormat::OpenAiResponses,
            Self::AnthropicMessages(_) | Self::KiroMessages(_) => ApiFormat::AnthropicMessages,
        }
    }
}

/// The response-mode-specific transport deadlines shared by every admitted Endpoint.
///
/// Streaming and non-streaming cannot share one profile. A streaming attempt must survive a long
/// completion whose first bytes already crossed the `FirstSemanticEvent` boundary and can no longer
/// be retried, so its absolute ceiling is a last-resort bound and its liveness is enforced by the
/// idle deadline. A non-streaming attempt is still entirely pre-first-byte for the client, so it
/// keeps one short bounded total that a failed attempt could legally be retried against.
struct P12TransportProfiles {
    streaming: UpstreamTransportProfile,
    non_streaming: UpstreamTransportProfile,
    web_streaming: UpstreamTransportProfile,
    web_non_streaming: UpstreamTransportProfile,
    flaresolverr_proxy_url: Option<String>,
    flaresolverr_port: u16,
    statsig_signer_url: Option<String>,
    browser_relay_url: Option<String>,
}

impl P12TransportProfiles {
    /// Builds both profiles from the fixed P12 deadlines, failing closed on an unsafe shape.
    #[cfg(test)]
    fn try_new() -> Result<Self, RuntimeCompositionError> {
        Self::try_new_with_web_proxy(UpstreamProxy::Direct, None, 8191)
    }

    /// Builds the direct profiles plus an optional Web-only proxy profile.
    fn try_new_with_web_proxy(
        web_proxy: UpstreamProxy,
        flaresolverr_proxy: Option<UpstreamProxy>,
        flaresolverr_port: u16,
    ) -> Result<Self, RuntimeCompositionError> {
        let maximum_idle_connections_per_host =
            NonZeroUsize::new(1).ok_or(RuntimeCompositionError::Unavailable)?;
        let streaming_timeouts = UpstreamTimeouts::try_new(
            P12_CONNECT_TIMEOUT,
            P12_STREAMING_TTFB_TIMEOUT,
            P12_STREAMING_IDLE_TIMEOUT,
            P12_STREAMING_TOTAL_TIMEOUT,
        )
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
        let streaming = UpstreamTransportProfile::new(
            streaming_timeouts,
            UpstreamProxy::Direct,
            maximum_idle_connections_per_host,
        );
        let web_streaming = UpstreamTransportProfile::new(
            streaming_timeouts,
            web_proxy.clone(),
            maximum_idle_connections_per_host,
        );
        // The transport bounds the wait for response headers by first-byte and, through reqwest's
        // read timeout, by response-idle as well. A buffered upstream returns nothing until it has
        // finished, so both must equal this mode's total instead of a shorter streaming value.
        let non_streaming_timeouts = UpstreamTimeouts::try_new(
            P12_CONNECT_TIMEOUT,
            P12_NON_STREAMING_TOTAL_TIMEOUT,
            P12_NON_STREAMING_TOTAL_TIMEOUT,
            P12_NON_STREAMING_TOTAL_TIMEOUT,
        )
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
        let non_streaming = UpstreamTransportProfile::new(
            non_streaming_timeouts,
            UpstreamProxy::Direct,
            maximum_idle_connections_per_host,
        );
        let web_non_streaming = UpstreamTransportProfile::new(
            non_streaming_timeouts,
            web_proxy,
            maximum_idle_connections_per_host,
        );
        Ok(Self {
            streaming,
            non_streaming,
            web_streaming,
            web_non_streaming,
            flaresolverr_proxy_url: flaresolverr_proxy
                .and_then(|proxy| proxy.canonical_url().map(str::to_owned)),
            flaresolverr_port,
            statsig_signer_url: std::env::var(GROK_WEB_STATSIG_SIGNER_URL_ENV)
                .ok()
                .filter(|value| !value.trim().is_empty()),
            browser_relay_url: std::env::var(GROK_WEB_BROWSER_RELAY_URL_ENV)
                .ok()
                .filter(|value| !value.trim().is_empty()),
        })
    }

    /// Returns the profile whose deadlines match this attempt's response mode.
    const fn for_mode(&self, mode: ResponsesResponseMode) -> &UpstreamTransportProfile {
        match mode {
            ResponsesResponseMode::Streaming => &self.streaming,
            ResponsesResponseMode::NonStreaming => &self.non_streaming,
        }
    }

    /// Returns the Web-specific profile, keeping non-Web providers on direct egress.
    const fn for_web_mode(&self, mode: ResponsesResponseMode) -> &UpstreamTransportProfile {
        match mode {
            ResponsesResponseMode::Streaming => &self.web_streaming,
            ResponsesResponseMode::NonStreaming => &self.web_non_streaming,
        }
    }

    /// Returns the explicit Web proxy bound into the browser session fingerprint.
    const fn web_proxy(&self) -> &UpstreamProxy {
        self.web_streaming.proxy()
    }

    fn flaresolverr_proxy_url(&self) -> Option<&str> {
        self.flaresolverr_proxy_url.as_deref()
    }

    const fn flaresolverr_port(&self) -> u16 {
        self.flaresolverr_port
    }

    fn statsig_signer_url(&self) -> Option<&str> {
        self.statsig_signer_url.as_deref()
    }

    fn browser_relay_url(&self) -> Option<&str> {
        self.browser_relay_url.as_deref()
    }
}

struct LeaseHoldingEventSource {
    source: Box<dyn ResponsesEventSource>,
    _selection: SelectedRouteCredential,
}

/// Applies the D2 client-protocol projection to every decoded upstream event.
///
/// The upstream decoder and the target projector each validate their own lifecycle. A rejected
/// semantic never becomes visible target data; the safe stream error returned here terminates the
/// already committed downstream without retrying another upstream after FSE.
struct ProjectedEventSource {
    inner: Box<dyn ResponsesEventSource>,
    projector: ProtocolResponseProjector,
}

impl ProjectedEventSource {
    fn new(inner: Box<dyn ResponsesEventSource>, target: ProtocolFormat) -> Self {
        Self {
            inner,
            projector: ProtocolResponseProjector::new(target),
        }
    }
}

impl ResponsesEventSource for ProjectedEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            loop {
                let Some(event) = self.inner.next_event().await? else {
                    return Ok(None);
                };
                match self.projector.project_event(&event) {
                    Ok(Some(projected)) => return Ok(Some(projected)),
                    Ok(None) => {}
                    Err(_) => return Err(upstream_protocol_error()),
                }
            }
        })
    }
}

impl ResponsesEventSource for LeaseHoldingEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        self.source.next_event()
    }
}

/// Keeps a compatible egress-node reservation alive for the complete response source.
///
/// The ordinary [`SelectedRouteCredential`] is owned by the router's attempt state. This wrapper
/// owns only the second, local egress lease; dropping the source therefore releases the proxy-node
/// capacity on JSON completion, SSE cancellation, timeout, or decoder failure without creating a
/// second Credential lease.
struct CompatibleEgressLeaseHoldingEventSource {
    source: Box<dyn ResponsesEventSource>,
    _runtime: Arc<CompatibleEndpointRuntime>,
    _lease: CompatibleEndpointEgressLease,
}

impl ResponsesEventSource for CompatibleEgressLeaseHoldingEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        self.source.next_event()
    }
}

/// Borrowed request-local view of the compatible egress handoff. The owned selection is kept in
/// `EndpointAttemptDriver::start` until the provider event source has been wrapped.
#[derive(Clone, Copy)]
struct CompatibleTransportContext<'a> {
    runtime: &'a CompatibleEndpointRuntime,
    lease: &'a CompatibleEndpointEgressLease,
}

struct CompatibleEgressSelection {
    runtime: Arc<CompatibleEndpointRuntime>,
    lease: CompatibleEndpointEgressLease,
}

struct EndpointAttemptDriver {
    request_id: RequestId,
    request: CanonicalRequest,
    client_protocol: ProtocolFormat,
    native_payload: Option<Arc<[u8]>>,
    usage_projection: P12ResponseUsageProjection,
    mode: ResponsesResponseMode,
    client_transport: ResponsesClientTransport,
    endpoints: Arc<BTreeMap<EndpointId, EndpointRuntime>>,
    compatible_endpoints: Arc<BTreeMap<EndpointId, Arc<CompatibleEndpointRuntime>>>,
    client_pool: Arc<UpstreamClientPool>,
    attempt_stages: Arc<P12AttemptStageStore>,
    allow_compatibility_retry: bool,
    allow_egress_refresh: bool,
    channel_pin_observation: Option<Arc<P13ChannelPinObservation>>,
}

/// Request-local, value-free observation used by Channel Pin to distinguish a rejected lease
/// from a driver invocation when the shared stage ledger is contended or unavailable.
#[derive(Default)]
struct P13ChannelPinObservation {
    attempted: AtomicBool,
    upstream_sent: AtomicBool,
}

impl P13ChannelPinObservation {
    fn mark_attempted(&self) {
        self.attempted.store(true, Ordering::Release);
    }

    fn mark_upstream_sent(&self) {
        self.upstream_sent.store(true, Ordering::Release);
    }

    fn attempted(&self) -> bool {
        self.attempted.load(Ordering::Acquire)
    }

    fn upstream_sent(&self) -> bool {
        self.upstream_sent.load(Ordering::Acquire)
    }
}

impl EndpointAttemptDriver {
    fn mark_upstream_sent(&self) {
        if let Some(observation) = &self.channel_pin_observation {
            observation.mark_upstream_sent();
        }
    }

    /// Acquires the provider-neutral egress handoff for a generic compatible adapter. Native
    /// Provider adapters deliberately bypass this path because they own a different transport
    /// contract (and may have provider-specific bootstrap traffic).
    fn compatible_egress_for_candidate(
        &self,
        runtime: &EndpointRuntime,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
    ) -> Result<Option<CompatibleEgressSelection>, AttemptFailure> {
        if !matches!(
            &runtime.adapter,
            EndpointAdapter::OpenAiChatCompletions(_)
                | EndpointAdapter::OpenAiResponses(_)
                | EndpointAdapter::AnthropicMessages(_)
        ) {
            return Ok(None);
        }
        let Some(compatible_runtime) = self.compatible_endpoints.get(candidate.endpoint_id())
        else {
            // Test-only drivers may omit the optional P13-11C composition. The production
            // composition always supplies a direct registry for every generic Endpoint.
            return Ok(None);
        };
        let observed_at_ms = system_now_ms()?;
        let lease = compatible_runtime
            .try_lease_egress_for_credential(
                credential,
                Some(candidate.upstream_model()),
                observed_at_ms,
            )
            .map_err(|error| match error {
                gateway_router::CompatibleEndpointRuntimeError::SelectedCredentialMismatch
                | gateway_router::CompatibleEndpointRuntimeError::FailureFeedbackUnavailable => {
                    AttemptFailure::NonRetryable(internal_error())
                }
                gateway_router::CompatibleEndpointRuntimeError::HealthUnavailable
                | gateway_router::CompatibleEndpointRuntimeError::QuotaUnavailable
                | gateway_router::CompatibleEndpointRuntimeError::EndpointBlocked
                | gateway_router::CompatibleEndpointRuntimeError::NoEligibleCredential
                | gateway_router::CompatibleEndpointRuntimeError::EgressUnavailable
                | gateway_router::CompatibleEndpointRuntimeError::EgressRegistryUnavailable => {
                    AttemptFailure::CompatibleEgress
                }
            })?;
        Ok(Some(CompatibleEgressSelection {
            runtime: Arc::clone(compatible_runtime),
            lease,
        }))
    }

    fn compatible_context(
        selection: Option<&CompatibleEgressSelection>,
    ) -> Option<CompatibleTransportContext<'_>> {
        selection.map(|selection| CompatibleTransportContext {
            runtime: selection.runtime.as_ref(),
            lease: &selection.lease,
        })
    }

    fn wrap_compatible_source(
        selection: Option<CompatibleEgressSelection>,
        source: Box<dyn ResponsesEventSource>,
    ) -> Box<dyn ResponsesEventSource> {
        match selection {
            Some(selection) => Box::new(CompatibleEgressLeaseHoldingEventSource {
                source,
                _runtime: selection.runtime,
                _lease: selection.lease,
            }),
            None => source,
        }
    }

    /// Produces the exact request material for one Candidate without reading a Secret or taking a
    /// lease. The same function is used by the scheduler predicate and again by the driver, so
    /// admission and execution cannot drift.
    fn project_candidate(
        &self,
        candidate: &SnapshotRouteCandidate,
    ) -> Result<ProjectedProtocolRequest, ProtocolTransformRejection> {
        let target = candidate
            .protocol_format()
            .ok_or(ProtocolTransformRejection::PairUnregistered)?;
        let runtime = self
            .endpoints
            .get(candidate.endpoint_id())
            .ok_or(ProtocolTransformRejection::PairUnregistered)?;
        if self.client_transport == ResponsesClientTransport::WebSocket
            && !candidate
                .effective_capabilities()
                .supports(SemanticCapability::ResponsesWebSocket)
        {
            return Err(ProtocolTransformRejection::ResponsesWebSocketUnsupported);
        }
        // Provider-specific runtimes use the protocol's Canonical semantics but not a generic
        // provider's native HTTP body. Only their typed Canonical paths are registered.
        if matches!(
            &runtime.adapter,
            EndpointAdapter::KiroMessages(_)
                | EndpointAdapter::GrokBuildResponses
                | EndpointAdapter::GrokConsoleResponses
                | EndpointAdapter::GrokWebResponses
                | EndpointAdapter::GrokOfficialResponses
        ) && !matches!(
            candidate.transform_mode(),
            gateway_router::SnapshotTransformMode::Canonical
                | gateway_router::SnapshotTransformMode::CanonicalBridge
        ) {
            return Err(ProtocolTransformRejection::PairUnregistered);
        }
        project_registered_protocol_request(ProtocolTransformInput {
            source: self.client_protocol,
            target,
            mode: candidate.transform_mode(),
            native_payload: if self.native_payload.is_some() {
                NativePayloadAvailability::Exact
            } else {
                NativePayloadAvailability::Unavailable
            },
            request: &self.request,
            streaming: self.mode == ResponsesResponseMode::Streaming,
            requires_json_schema: false,
            requires_parallel_tools: false,
            target_capabilities: candidate.effective_capabilities(),
        })
    }
}

impl AttemptDriver for EndpointAttemptDriver {
    type Output = Box<dyn ResponsesEventSource>;

    fn start<'a>(
        &'a self,
        candidate: &'a SnapshotRouteCandidate,
        credential: &'a CredentialLease,
        _bootstrap_timeout: Duration,
    ) -> AttemptFuture<'a, Result<Self::Output, AttemptFailure>> {
        Box::pin(async move {
            if let Some(observation) = &self.channel_pin_observation {
                observation.mark_attempted();
            }
            self.attempt_stages.record_stage(
                &self.request_id,
                ManagementRequestAttemptStage::RequestConversion,
            );
            let Some(runtime) = self.endpoints.get(candidate.endpoint_id()) else {
                return Err(AttemptFailure::NonRetryable(internal_error()));
            };
            // The endpoint map and the Candidate come from the same pinned Config Version and
            // Snapshot, so this can only differ when a Snapshot was constructed outside the
            // compiler. Prove the agreement before a Secret is read or a URL is composed rather
            // than serving one protocol's request over another protocol's wire.
            if candidate
                .protocol_format()
                .map(ProtocolFormat::as_api_format)
                != Some(runtime.adapter.api_format())
            {
                return Err(AttemptFailure::NonRetryable(internal_error()));
            }
            let projected = self
                .project_candidate(candidate)
                .map_err(|_| AttemptFailure::NonRetryable(upstream_protocol_error()))?;
            let compatible_selection =
                self.compatible_egress_for_candidate(runtime, candidate, credential)?;
            let compatible_context = Self::compatible_context(compatible_selection.as_ref());
            let result = match &runtime.adapter {
                EndpointAdapter::OpenAiChatCompletions(endpoint) => {
                    self.start_openai_chat_completions(
                        runtime,
                        endpoint,
                        candidate,
                        credential,
                        &projected,
                        compatible_context,
                    )
                    .await
                }
                EndpointAdapter::OpenAiResponses(endpoint) => {
                    self.start_openai_responses(
                        runtime,
                        endpoint,
                        candidate,
                        credential,
                        &projected,
                        compatible_context,
                    )
                    .await
                }
                EndpointAdapter::GrokBuildResponses => {
                    self.start_grok_build(runtime, candidate, credential, &projected)
                        .await
                }
                EndpointAdapter::GrokConsoleResponses => {
                    self.start_grok_console(runtime, candidate, credential, &projected)
                        .await
                }
                EndpointAdapter::GrokWebResponses => {
                    self.start_grok_web(runtime, candidate, credential, &projected)
                        .await
                }
                EndpointAdapter::GrokOfficialResponses => {
                    self.start_grok_official(runtime, candidate, credential, &projected)
                        .await
                }
                EndpointAdapter::AnthropicMessages(endpoint) => {
                    self.start_anthropic_messages(
                        runtime,
                        endpoint,
                        candidate,
                        credential,
                        &projected,
                        compatible_context,
                    )
                    .await
                }
                EndpointAdapter::KiroMessages(policy) => {
                    self.start_kiro_messages(runtime, policy, candidate, credential, &projected)
                        .await
                }
            };
            result.map(|source| Self::wrap_compatible_source(compatible_selection, source))
        })
    }

    fn start_timeout(&self, remaining_bootstrap: Duration) -> Duration {
        p12_attempt_start_timeout(self.mode, remaining_bootstrap)
    }
}

impl EndpointAttemptDriver {
    /// Runs a native OpenAI-compatible Chat Completions attempt.
    ///
    /// The strictly decoded ingress payload is retained and only its model is replaced, matching
    /// the incumbent CPA's native `OpenAI` translator. Canonical reconstruction is reserved for the
    /// separately admitted P12-08D bridge matrix.
    async fn start_openai_chat_completions(
        &self,
        runtime: &EndpointRuntime,
        endpoint: &OpenAiChatCompletionsEndpoint,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
        projected: &ProjectedProtocolRequest,
        compatible: Option<CompatibleTransportContext<'_>>,
    ) -> Result<Box<dyn ResponsesEventSource>, AttemptFailure> {
        let credential = openai_runtime_credential(credential.secret_bytes(), system_now_ms()?)?;
        let bearer = credential
            .bearer_at(system_now_ms()?)
            .map_err(AttemptFailure::NonRetryable)?;
        let request_credential = OpenAiChatCompletionsApiKey::try_new(bearer.to_owned())
            .map_err(AttemptFailure::NonRetryable)?;
        let outbound = match projected {
            ProjectedProtocolRequest::NativeExact => {
                OpenAiChatCompletionsRequestBuilder::build_native(
                    endpoint,
                    &request_credential,
                    candidate.upstream_model(),
                    self.native_payload
                        .as_deref()
                        .ok_or_else(|| AttemptFailure::NonRetryable(upstream_protocol_error()))?,
                    chat_upstream_response_mode(self.mode),
                )
            }
            ProjectedProtocolRequest::Canonical(request) => {
                OpenAiChatCompletionsRequestBuilder::build(
                    endpoint,
                    &request_credential,
                    candidate.upstream_model(),
                    request,
                    chat_upstream_response_mode(self.mode),
                )
            }
        }
        .map_err(AttemptFailure::NonRetryable)?;
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::EgressAdmission,
        );
        let admitted = runtime
            .policy
            .admit_url(outbound.url(), runtime.resolver.as_ref())
            .map_err(|_| AttemptFailure::NonRetryable(egress_rejected_error()))?;
        let request = outbound
            .into_transport_request(admitted)
            .map_err(AttemptFailure::NonRetryable)?;
        let mut response = self
            .send_admitted_request(
                runtime,
                request,
                HttpFailureProfile::OpenAiCompatible,
                false,
                compatible,
            )
            .await?;
        match self.mode {
            ResponsesResponseMode::NonStreaming => {
                let events = decode_chat_json_response(
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
                let source = ChatSseEventSource::begin(response, self.usage_projection).await?;
                Ok(Box::new(source) as Box<dyn ResponsesEventSource>)
            }
        }
    }

    /// Runs one attempt against an `OpenAI`-compatible Responses Endpoint.
    ///
    /// This is the P12 path unchanged: the same request conversion, credential shape, request
    /// builder, egress admission, transport headers, response-mode profile, status and
    /// content-type classification, bounded JSON body, and bounded SSE decoder.
    #[allow(clippy::too_many_lines)]
    async fn start_openai_responses(
        &self,
        runtime: &EndpointRuntime,
        endpoint: &OpenAiResponsesEndpoint,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
        projected: &ProjectedProtocolRequest,
        compatible: Option<CompatibleTransportContext<'_>>,
    ) -> Result<Box<dyn ResponsesEventSource>, AttemptFailure> {
        let credential = openai_runtime_credential(credential.secret_bytes(), system_now_ms()?)?;
        let bearer = credential
            .bearer_at(system_now_ms()?)
            .map_err(AttemptFailure::NonRetryable)?;
        let request_credential = OpenAiResponsesApiKey::try_new(bearer.to_owned())
            .map_err(AttemptFailure::NonRetryable)?;
        let is_codex_oauth = endpoint
            .url()
            .starts_with("https://chatgpt.com/backend-api/codex/")
            && credential.has_account_binding();
        let mut outbound = match projected {
            ProjectedProtocolRequest::NativeExact => OpenAiResponsesRequestBuilder::build_native(
                endpoint,
                &request_credential,
                candidate.upstream_model(),
                self.native_payload
                    .as_deref()
                    .ok_or_else(|| AttemptFailure::NonRetryable(upstream_protocol_error()))?,
                upstream_response_mode(self.mode),
            ),
            ProjectedProtocolRequest::Canonical(request) => OpenAiResponsesRequestBuilder::build(
                endpoint,
                &request_credential,
                candidate.upstream_model(),
                request,
                upstream_response_mode(self.mode),
            ),
        }
        .map_err(AttemptFailure::NonRetryable)?;
        if is_codex_oauth {
            outbound
                .force_store_false()
                .map_err(AttemptFailure::NonRetryable)?;
            if self.mode == ResponsesResponseMode::NonStreaming {
                outbound
                    .force_streaming()
                    .map_err(AttemptFailure::NonRetryable)?;
            }
        }
        let mut retried_rejected_max_output_tokens = false;
        let mut response = loop {
            self.attempt_stages.record_stage(
                &self.request_id,
                ManagementRequestAttemptStage::EgressAdmission,
            );
            let admitted = runtime
                .policy
                .admit_url(outbound.url(), runtime.resolver.as_ref())
                .map_err(|_| AttemptFailure::NonRetryable(egress_rejected_error()))?;
            if is_codex_oauth {
                let request =
                    p12_transport_request(&outbound, admitted, true, credential.account_id())
                        .map_err(AttemptFailure::NonRetryable)?;
                match self
                    .send_codex_admitted_request(
                        runtime,
                        request,
                        HttpFailureProfile::OpenAiCompatible,
                        true,
                        compatible,
                    )
                    .await
                {
                    Ok(response) => break response,
                    Err(P12SendFailure::RetryWithoutMaxOutputTokens)
                        if self.allow_compatibility_retry
                            && !retried_rejected_max_output_tokens =>
                    {
                        retried_rejected_max_output_tokens = true;
                        outbound
                            .remove_root_field("max_output_tokens")
                            .map_err(AttemptFailure::NonRetryable)?;
                    }
                    Err(P12SendFailure::RetryWithoutMaxOutputTokens) => {
                        return Err(AttemptFailure::NonRetryable(upstream_protocol_error()));
                    }
                    Err(P12SendFailure::Attempt(failure)) => return Err(failure),
                }
            } else {
                let request =
                    p12_transport_request(&outbound, admitted, false, credential.account_id())
                        .map_err(AttemptFailure::NonRetryable)?;
                break self
                    .send_admitted_request(
                        runtime,
                        request,
                        HttpFailureProfile::OpenAiCompatible,
                        false,
                        compatible,
                    )
                    .await?;
            }
        };

        match self.mode {
            ResponsesResponseMode::NonStreaming if is_codex_oauth => {
                self.attempt_stages.record_stage(
                    &self.request_id,
                    ManagementRequestAttemptStage::SseBootstrap,
                );
                let source = OpenAiSseEventSource::begin_with_reasoning_policy(
                    response,
                    self.usage_projection,
                    is_codex_oauth,
                )
                .await?;
                Ok(Box::new(source) as Box<dyn ResponsesEventSource>)
            }
            ResponsesResponseMode::NonStreaming => {
                let events = decode_json_response_with_reasoning_policy(
                    &mut response,
                    self.attempt_stages.as_ref(),
                    &self.request_id,
                    self.usage_projection,
                    is_codex_oauth,
                )
                .await?;
                Ok(Box::new(FiniteEventSource::new(events)) as Box<dyn ResponsesEventSource>)
            }
            ResponsesResponseMode::Streaming => {
                self.attempt_stages.record_stage(
                    &self.request_id,
                    ManagementRequestAttemptStage::SseBootstrap,
                );
                let source = OpenAiSseEventSource::begin_with_reasoning_policy(
                    response,
                    self.usage_projection,
                    is_codex_oauth,
                )
                .await?;
                Ok(Box::new(source) as Box<dyn ResponsesEventSource>)
            }
        }
    }

    /// Runs one attempt against an Anthropic-compatible Messages Endpoint.
    ///
    /// It mirrors [`Self::start_openai_responses`] stage for stage -- the same
    /// `RequestConversion`/`EgressAdmission`/`HttpTransport`/`HttpStatus`/`ContentType` ledger
    /// order, the same `AttemptFailure` classification, the same response-mode transport profile
    /// and the same bounded body/frame ceilings -- and differs only in the wire codec it uses. It
    /// does not delegate to `InferenceAdapter::execute`, whose `GatewayError` return cannot carry
    /// the upstream status the orchestrator needs to keep a pre-first-byte failure retryable.
    async fn start_anthropic_messages(
        &self,
        runtime: &EndpointRuntime,
        endpoint: &AnthropicMessagesEndpoint,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
        projected: &ProjectedProtocolRequest,
        compatible: Option<CompatibleTransportContext<'_>>,
    ) -> Result<Box<dyn ResponsesEventSource>, AttemptFailure> {
        // No `max_tokens` translation happens here: the Anthropic Messages codec reads the
        // namespaced extension directly and fails closed on a request that carries no lossless
        // Anthropic representation, rather than inventing one.
        let credential = anthropic_runtime_credential(credential.secret_bytes())?;
        let authorization = credential
            .authorization_at(system_now_ms()?)
            .map_err(AttemptFailure::NonRetryable)?;
        let outbound = match projected {
            ProjectedProtocolRequest::NativeExact => {
                AnthropicMessagesRequestBuilder::build_native_with_authorization(
                    endpoint,
                    &authorization,
                    candidate.upstream_model(),
                    self.native_payload
                        .as_deref()
                        .ok_or_else(|| AttemptFailure::NonRetryable(upstream_protocol_error()))?,
                    anthropic_upstream_response_mode(self.mode),
                )
            }
            ProjectedProtocolRequest::Canonical(request) => {
                AnthropicMessagesRequestBuilder::build_with_authorization(
                    endpoint,
                    &authorization,
                    candidate.upstream_model(),
                    request,
                    anthropic_upstream_response_mode(self.mode),
                )
            }
        }
        .map_err(AttemptFailure::NonRetryable)?;
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::EgressAdmission,
        );
        let admitted = runtime
            .policy
            .admit_url(outbound.url(), runtime.resolver.as_ref())
            .map_err(|_| AttemptFailure::NonRetryable(egress_rejected_error()))?;
        let request = p12_anthropic_transport_request(outbound, admitted)
            .map_err(AttemptFailure::NonRetryable)?;
        let mut response = self
            .send_admitted_request(
                runtime,
                request,
                HttpFailureProfile::AnthropicCompatible,
                false,
                compatible,
            )
            .await?;

        match self.mode {
            ResponsesResponseMode::NonStreaming => {
                let events = decode_anthropic_json_response(
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
                    AnthropicSseEventSource::begin(response, self.usage_projection).await?;
                Ok(Box::new(source) as Box<dyn ResponsesEventSource>)
            }
        }
    }

    /// Runs one Canonical attempt through the fixed Grok Build OAuth Responses runtime.
    async fn start_grok_build(
        &self,
        runtime: &EndpointRuntime,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
        projected: &ProjectedProtocolRequest,
    ) -> Result<Box<dyn ResponsesEventSource>, AttemptFailure> {
        let ProjectedProtocolRequest::Canonical(request) = projected else {
            return Err(AttemptFailure::NonRetryable(upstream_protocol_error()));
        };
        let credential =
            GrokBuildCredential::import_runtime_json(credential.secret_bytes(), system_now_ms()?)
                .map_err(|_| AttemptFailure::NonRetryable(credential_unavailable_error()))?;
        let transport = GrokBuildUpstreamTransport::new(
            runtime.policy.clone(),
            Arc::clone(&runtime.resolver),
            self.client_pool.as_ref().clone(),
            runtime.transports.for_mode(self.mode).clone(),
        );
        let adapter = GrokBuildInferenceAdapter::try_new(
            credential,
            candidate.upstream_model(),
            grok_build_execution_mode(self.mode),
            Arc::new(transport),
        )
        .map_err(AttemptFailure::NonRetryable)?;
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::EgressAdmission,
        );
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::HttpTransport,
        );
        self.mark_upstream_sent();
        let source = adapter
            .execute(
                RequestContext::new(self.request_id.clone()),
                request.clone(),
            )
            .await
            .map_err(p12_classify_grok_start_failure)?;
        self.attempt_stages
            .record_stage(&self.request_id, ManagementRequestAttemptStage::HttpStatus);
        Ok(Box::new(P12ProviderEventSource::new(source)) as Box<dyn ResponsesEventSource>)
    }

    /// Runs one Canonical attempt through the fixed Grok Console SSO Responses runtime.
    async fn start_grok_console(
        &self,
        runtime: &EndpointRuntime,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
        projected: &ProjectedProtocolRequest,
    ) -> Result<Box<dyn ResponsesEventSource>, AttemptFailure> {
        let ProjectedProtocolRequest::Canonical(request) = projected else {
            return Err(AttemptFailure::NonRetryable(upstream_protocol_error()));
        };
        let credential = GrokConsoleSsoToken::try_from_bytes(credential.secret_bytes())
            .map_err(|_| AttemptFailure::NonRetryable(credential_unavailable_error()))?;
        let transport = GrokConsoleUpstreamTransport::new(
            runtime.policy.clone(),
            Arc::clone(&runtime.resolver),
            self.client_pool.as_ref().clone(),
            runtime.transports.for_mode(self.mode).clone(),
        );
        let adapter = GrokConsoleInferenceAdapter::try_new(
            credential,
            candidate.upstream_model(),
            grok_console_execution_mode(self.mode),
            Arc::new(transport),
        )
        .map_err(AttemptFailure::NonRetryable)?;
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::EgressAdmission,
        );
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::HttpTransport,
        );
        self.mark_upstream_sent();
        let source = adapter
            .execute(
                RequestContext::new(self.request_id.clone()),
                request.clone(),
            )
            .await
            .map_err(p12_classify_grok_start_failure)?;
        self.attempt_stages
            .record_stage(&self.request_id, ManagementRequestAttemptStage::HttpStatus);
        Ok(Box::new(P12ProviderEventSource::new(source)) as Box<dyn ResponsesEventSource>)
    }

    /// Runs one Canonical attempt through the fixed Grok Web SSO conversation runtime.
    async fn start_grok_web(
        &self,
        runtime: &EndpointRuntime,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
        projected: &ProjectedProtocolRequest,
    ) -> Result<Box<dyn ResponsesEventSource>, AttemptFailure> {
        let ProjectedProtocolRequest::Canonical(request) = projected else {
            return Err(AttemptFailure::NonRetryable(upstream_protocol_error()));
        };
        let now_ms = system_now_ms()?;
        let credential = GrokWebCredential::import_sso_json(credential.secret_bytes(), now_ms)
            .map_err(|_| AttemptFailure::NonRetryable(credential_unavailable_error()))?;
        let session = Arc::new(
            GrokWebBrowserEgressSession::try_new(
                GrokWebEgressSessionId::try_new(credential.account_reference())
                    .map_err(|_| AttemptFailure::NonRetryable(credential_unavailable_error()))?,
                credential,
                GrokWebBrowserUserAgent::try_new(GROK_WEB_PRODUCTION_USER_AGENT)
                    .map_err(|_| AttemptFailure::NonRetryable(internal_error()))?,
                GrokWebTlsProfile::try_new("chrome_146")
                    .map_err(|_| AttemptFailure::NonRetryable(internal_error()))?,
                runtime.transports.web_proxy().clone(),
                now_ms,
            )
            .map_err(|_| AttemptFailure::NonRetryable(credential_unavailable_error()))?,
        );
        let statsig = runtime
            .web_statsig
            .get_or_init(|| {
                let transport = GrokWebStatsigUpstreamTransport::new_with_signer_url(
                    runtime.policy.clone(),
                    Arc::clone(&runtime.resolver),
                    self.client_pool.as_ref().clone(),
                    runtime.transports.for_web_mode(self.mode),
                    runtime.transports.statsig_signer_url().map(str::to_owned),
                );
                GrokWebStatsigRuntime::try_new(Arc::new(transport))
                    .map(Arc::new)
                    .map_err(|_| internal_error())
            })
            .as_ref()
            .map_err(|error| AttemptFailure::NonRetryable(error.clone()))?
            .clone();
        let transport = GrokWebProductionUpstreamTransport::new_with_browser_relay_url(
            runtime.policy.clone(),
            Arc::clone(&runtime.resolver),
            self.client_pool.as_ref().clone(),
            runtime.transports.for_web_mode(self.mode).clone(),
            runtime.transports.browser_relay_url().map(str::to_owned),
        );
        let flaresolverr_transport = P12GrokWebFlareSolverrTransport::new(
            Arc::clone(&self.client_pool),
            runtime.transports.for_mode(self.mode).clone(),
            runtime
                .transports
                .flaresolverr_proxy_url()
                .map(str::to_owned),
            runtime.transports.flaresolverr_port(),
        )
        .map_err(AttemptFailure::NonRetryable)?;
        let mut adapter = GrokWebProductionInferenceAdapter::try_new(
            session,
            candidate.upstream_model(),
            statsig,
            Arc::new(transport),
        )
        .map_err(AttemptFailure::NonRetryable)?;
        if self.allow_egress_refresh {
            adapter = adapter.with_egress_refresher(Arc::new(P12GrokWebEgressRefresher {
                transport: Arc::new(flaresolverr_transport),
            }));
        } else {
            adapter = adapter.without_egress_retry();
        }
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::EgressAdmission,
        );
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::HttpTransport,
        );
        self.mark_upstream_sent();
        let source = adapter
            .execute(
                RequestContext::new(self.request_id.clone()),
                request.clone(),
            )
            .await
            .map_err(p12_classify_grok_start_failure)?;
        self.attempt_stages
            .record_stage(&self.request_id, ManagementRequestAttemptStage::HttpStatus);
        Ok(Box::new(P12ProviderEventSource::new(source)) as Box<dyn ResponsesEventSource>)
    }

    /// Runs one Canonical attempt through the fixed xAI Official API-key Responses runtime.
    async fn start_grok_official(
        &self,
        runtime: &EndpointRuntime,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
        projected: &ProjectedProtocolRequest,
    ) -> Result<Box<dyn ResponsesEventSource>, AttemptFailure> {
        let ProjectedProtocolRequest::Canonical(request) = projected else {
            return Err(AttemptFailure::NonRetryable(upstream_protocol_error()));
        };
        let secret = std::str::from_utf8(credential.secret_bytes())
            .map_err(|_| AttemptFailure::NonRetryable(credential_unavailable_error()))?;
        let credential =
            GrokOfficialApiKey::try_new(secret.to_owned()).map_err(AttemptFailure::NonRetryable)?;
        let transport = GrokOfficialUpstreamTransport::new(
            runtime.policy.clone(),
            Arc::clone(&runtime.resolver),
            self.client_pool.as_ref().clone(),
            runtime.transports.for_mode(self.mode).clone(),
        );
        let adapter = GrokOfficialInferenceAdapter::try_new(
            credential,
            candidate.upstream_model(),
            grok_official_execution_mode(self.mode),
            Arc::new(transport),
        )
        .map_err(AttemptFailure::NonRetryable)?;
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::EgressAdmission,
        );
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::HttpTransport,
        );
        self.mark_upstream_sent();
        let source = adapter
            .execute(
                RequestContext::new(self.request_id.clone()),
                request.clone(),
            )
            .await
            .map_err(p12_classify_grok_start_failure)?;
        self.attempt_stages
            .record_stage(&self.request_id, ManagementRequestAttemptStage::HttpStatus);
        Ok(Box::new(P12ProviderEventSource::new(source)) as Box<dyn ResponsesEventSource>)
    }

    /// Runs one attempt against a Kiro Endpoint.
    ///
    /// Kiro serves Anthropic Messages semantics but reaches them through its own request shape, so
    /// this arm delegates to `provider-kiro`'s adapter rather than the generic Anthropic codec. The
    /// adapter owns request conversion, `profileArn` placement, the AWS `EventStream` decode and Kiro
    /// failure classification; this function supplies only the credential, the derived endpoint
    /// policy, and a transport already bound to this Endpoint's egress policy.
    async fn start_kiro_messages(
        &self,
        runtime: &EndpointRuntime,
        policy: &KiroEndpointPolicy,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
        projected: &ProjectedProtocolRequest,
    ) -> Result<Box<dyn ResponsesEventSource>, AttemptFailure> {
        let ProjectedProtocolRequest::Canonical(request) = projected else {
            return Err(AttemptFailure::NonRetryable(upstream_protocol_error()));
        };
        // The lease may retain the backward-compatible raw `ksk_` form or one strict
        // Social/Enterprise JSON object. Expired OAuth fails before profile resolution or egress;
        // refresh remains an explicit F1 worker transaction rather than hidden request-path I/O.
        let kiro_credential =
            KiroCredential::import_runtime_secret(credential.secret_bytes(), system_now_ms()?)
                .map_err(|_| AttemptFailure::NonRetryable(credential_unavailable_error()))?;
        let profile = resolve_profile_arn(
            kiro_credential.kind(),
            policy.api_region(),
            &P12KiroNoEnterpriseLookup,
        );
        // The conversation identity is the request identity, so one attempt cannot inherit another
        // request's Kiro conversation. The environment projection is fixed and host-independent:
        // the converter must never read a real working directory or OS from this server.
        let conversation = KiroConversationContext::new(
            KiroConversationId::try_new(self.request_id.as_str().to_owned())
                .map_err(|_| AttemptFailure::NonRetryable(internal_error()))?,
            KiroEnvironmentState::try_new(P12_KIRO_OPERATING_SYSTEM, P12_KIRO_WORKING_DIRECTORY)
                .map_err(|_| AttemptFailure::NonRetryable(internal_error()))?,
        );
        let transport = KiroUpstreamTransport::new(
            runtime.policy.clone(),
            Arc::clone(&runtime.resolver),
            self.client_pool.as_ref().clone(),
            runtime.transports.for_mode(self.mode).clone(),
        );
        let adapter = KiroInferenceAdapter::try_new(
            kiro_credential,
            policy.clone(),
            conversation,
            candidate.upstream_model(),
            profile,
            Arc::new(transport),
        )
        .map_err(AttemptFailure::NonRetryable)?;
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::EgressAdmission,
        );
        let context = RequestContext::new(self.request_id.clone());
        // The adapter performs egress admission and the single send internally, so the shared
        // `send_admitted_request` ledger does not apply; record the transport stage here so a Kiro
        // attempt still projects the same stage sequence to the management plane.
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::HttpTransport,
        );
        self.mark_upstream_sent();
        let source = adapter
            .execute(context, p12_kiro_request_projection(request))
            .await
            .map_err(p12_classify_kiro_start_failure)?;
        self.attempt_stages
            .record_stage(&self.request_id, ManagementRequestAttemptStage::HttpStatus);
        Ok(Box::new(P12ProviderEventSource::new(source)) as Box<dyn ResponsesEventSource>)
    }

    /// Sends one already egress-admitted request and classifies its response head.
    ///
    /// Both format arms share this exact ledger order and failure classification, so the
    /// pre-first-byte retryability of a status or content-type failure cannot drift between them.
    async fn send_admitted_request(
        &self,
        runtime: &EndpointRuntime,
        request: UpstreamHttpRequest,
        failure_profile: HttpFailureProfile,
        allow_missing_content_type: bool,
        compatible: Option<CompatibleTransportContext<'_>>,
    ) -> Result<UpstreamHttpResponse, AttemptFailure> {
        self.send_admitted_request_inner(
            runtime,
            request,
            failure_profile,
            allow_missing_content_type,
            false,
            compatible,
        )
        .await
        .map_err(P12SendFailure::into_attempt)
    }

    /// The official Codex OAuth path needs one narrowly scoped compatibility retry copied from
    /// CPA/sub2api: when the upstream explicitly rejects the root `max_output_tokens` field, the
    /// caller removes that field and replays the same leased credential once. The marker never
    /// escapes this request-local loop and is not handed to the route-level failover scheduler.
    async fn send_codex_admitted_request(
        &self,
        runtime: &EndpointRuntime,
        request: UpstreamHttpRequest,
        failure_profile: HttpFailureProfile,
        allow_missing_content_type: bool,
        compatible: Option<CompatibleTransportContext<'_>>,
    ) -> Result<UpstreamHttpResponse, P12SendFailure> {
        self.send_admitted_request_inner(
            runtime,
            request,
            failure_profile,
            allow_missing_content_type,
            true,
            compatible,
        )
        .await
    }

    async fn send_admitted_request_inner(
        &self,
        runtime: &EndpointRuntime,
        request: UpstreamHttpRequest,
        failure_profile: HttpFailureProfile,
        allow_missing_content_type: bool,
        detect_codex_rejected_field: bool,
        compatible: Option<CompatibleTransportContext<'_>>,
    ) -> Result<UpstreamHttpResponse, P12SendFailure> {
        self.attempt_stages.record_stage(
            &self.request_id,
            ManagementRequestAttemptStage::HttpTransport,
        );
        self.mark_upstream_sent();
        let base_profile = runtime.transports.for_mode(self.mode);
        let compatible_profile = compatible.map(|context| {
            base_profile
                .clone()
                .with_proxy(context.lease.transport_profile().proxy().clone())
        });
        let transport_profile = compatible_profile.as_ref().unwrap_or(base_profile);
        let Ok(mut response) = self.client_pool.send(request, transport_profile).await else {
            if let Some(context) = compatible {
                let now_ms = system_now_ms().map_err(P12SendFailure::Attempt)?;
                context
                    .runtime
                    .record_transport_failure(context.lease, now_ms, DEFAULT_TRANSIENT_COOLDOWN)
                    .map_err(|_| {
                        P12SendFailure::Attempt(AttemptFailure::NonRetryable(internal_error()))
                    })?;
                return Err(P12SendFailure::Attempt(AttemptFailure::CompatibleEgress));
            }
            return Err(P12SendFailure::Attempt(AttemptFailure::Connection));
        };

        self.attempt_stages
            .record_stage(&self.request_id, ManagementRequestAttemptStage::HttpStatus);
        match response.status() {
            200..=299 => {}
            status if failure_profile == HttpFailureProfile::OpenAiCompatible => {
                let retry_after_seconds = response
                    .header("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok());
                let body = read_provider_error_body(&mut response).await?;
                if detect_codex_rejected_field
                    && status == 400
                    && p12_openai_rejects_max_output_tokens(&body)
                {
                    return Err(P12SendFailure::RetryWithoutMaxOutputTokens);
                }
                return Err(P12SendFailure::Attempt(
                    classify_openai_response_failure_body(status, &body, retry_after_seconds),
                ));
            }
            status if failure_profile == HttpFailureProfile::AnthropicCompatible => {
                return Err(P12SendFailure::Attempt(
                    classify_anthropic_response_failure(&mut response, status).await,
                ));
            }
            429 => {
                return Err(P12SendFailure::Attempt(AttemptFailure::RateLimited {
                    retry_after: None,
                }));
            }
            500..=599 => return Err(P12SendFailure::Attempt(AttemptFailure::ServerError)),
            _ => {
                return Err(P12SendFailure::Attempt(AttemptFailure::NonRetryable(
                    provider_permanent_error(),
                )));
            }
        }
        self.attempt_stages
            .record_stage(&self.request_id, ManagementRequestAttemptStage::ContentType);
        if !has_expected_content_type(&response, self.mode, allow_missing_content_type) {
            return Err(P12SendFailure::Attempt(AttemptFailure::NonRetryable(
                upstream_protocol_error(),
            )));
        }
        Ok(response)
    }
}

enum P12SendFailure {
    Attempt(AttemptFailure),
    RetryWithoutMaxOutputTokens,
}

impl From<AttemptFailure> for P12SendFailure {
    fn from(failure: AttemptFailure) -> Self {
        Self::Attempt(failure)
    }
}

impl P12SendFailure {
    fn into_attempt(self) -> AttemptFailure {
        match self {
            Self::Attempt(failure) => failure,
            Self::RetryWithoutMaxOutputTokens => {
                AttemptFailure::NonRetryable(upstream_protocol_error())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HttpFailureProfile {
    OpenAiCompatible,
    AnthropicCompatible,
}

fn openai_runtime_credential(
    input: &[u8],
    now_ms: i64,
) -> Result<OpenAiCompatibleRuntimeCredential, AttemptFailure> {
    OpenAiCompatibleRuntimeCredential::import_compatible(input, now_ms).map_err(|_| {
        AttemptFailure::NonRetryable(GatewayError::new(
            GatewayErrorCode::CredentialUnavailable,
            ErrorScope::Credential,
        ))
    })
}

fn anthropic_runtime_credential(input: &[u8]) -> Result<ClaudeRuntimeCredential, AttemptFailure> {
    ClaudeRuntimeCredential::import(input).map_err(|_| {
        AttemptFailure::NonRetryable(GatewayError::new(
            GatewayErrorCode::CredentialUnavailable,
            ErrorScope::Credential,
        ))
    })
}

fn system_now_ms() -> Result<i64, AttemptFailure> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AttemptFailure::NonRetryable(internal_error()))?;
    i64::try_from(elapsed.as_millis()).map_err(|_| AttemptFailure::NonRetryable(internal_error()))
}

fn system_now_ms_runtime() -> Result<i64, RuntimeCompositionError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
    i64::try_from(elapsed.as_millis()).map_err(|_| RuntimeCompositionError::Unavailable)
}

#[cfg(test)]
async fn classify_openai_response_failure(
    response: &mut UpstreamHttpResponse,
    status: u16,
) -> AttemptFailure {
    let retry_after_seconds = response
        .header("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let body = match read_provider_error_body(response).await {
        Ok(body) => body,
        Err(failure) => return failure,
    };
    classify_openai_response_failure_body(status, &body, retry_after_seconds)
}

fn classify_openai_response_failure_body(
    status: u16,
    body: &[u8],
    retry_after_seconds: Option<u64>,
) -> AttemptFailure {
    let now_epoch_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let disposition =
        classify_openai_runtime_failure(status, body, retry_after_seconds, now_epoch_seconds);
    match disposition.action() {
        OpenAiRuntimeFailureAction::RecordExactQuota => AttemptFailure::RateLimited {
            retry_after: disposition.retry_after(),
        },
        OpenAiRuntimeFailureAction::CoolEndpoint => AttemptFailure::ServerError,
        OpenAiRuntimeFailureAction::None
        | OpenAiRuntimeFailureAction::RequireCredentialReauthorization => {
            AttemptFailure::NonRetryable(disposition.error().clone())
        }
    }
}

/// Recognizes the bounded field-rejection signal used by CPA/sub2api's Responses retry loop.
///
/// The official `ChatGPT` endpoint has emitted both an OpenAI-shaped `error` object and a compact
/// `detail` string. Only the explicit `unknown/unsupported` + `max_output_tokens` combination is
/// eligible; authentication, quota, account, and arbitrary 400 responses remain permanent.
fn p12_openai_rejects_max_output_tokens(body: &[u8]) -> bool {
    let text = String::from_utf8_lossy(body).to_ascii_lowercase();
    (text.contains("unknown") || text.contains("unsupported")) && text.contains("max_output_tokens")
}

async fn classify_anthropic_response_failure(
    response: &mut UpstreamHttpResponse,
    status: u16,
) -> AttemptFailure {
    let retry_after_seconds = response
        .header("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let body = match read_provider_error_body(response).await {
        Ok(body) => body,
        Err(failure) => return failure,
    };
    let disposition = classify_anthropic_runtime_failure(status, &body, retry_after_seconds);
    match disposition.action() {
        AnthropicRuntimeFailureAction::RecordExactQuota => AttemptFailure::RateLimited {
            retry_after: disposition.retry_after(),
        },
        AnthropicRuntimeFailureAction::CoolEndpoint => AttemptFailure::ServerError,
        AnthropicRuntimeFailureAction::None
        | AnthropicRuntimeFailureAction::RequireCredentialReauthorization => {
            AttemptFailure::NonRetryable(disposition.error().clone())
        }
    }
}

async fn read_provider_error_body(
    response: &mut UpstreamHttpResponse,
) -> Result<Vec<u8>, AttemptFailure> {
    let mut body = Vec::new();
    loop {
        let next = response
            .next_chunk()
            .await
            .map_err(|_| AttemptFailure::Connection)?;
        let Some(chunk) = next else { break };
        if body.len().saturating_add(chunk.len()) > MAX_PROVIDER_ERROR_BODY_BYTES {
            return Ok(Vec::new());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Hands one admitted Anthropic-compatible request to the shared transport.
///
/// The header set stays exactly the four the Anthropic-compatible boundary builds --
/// `accept`, `anthropic-version`, `content-type`, and exactly one of `x-api-key` or
/// `authorization`. [`p12_transport_headers`]'s Krill compatibility `User-Agent` is specific to
/// the isolated `OpenAI`-compatible endpoint and is deliberately not added here.
fn p12_anthropic_transport_request(
    outbound: AnthropicMessagesOutboundRequest,
    admitted: AdmittedEgressTarget,
) -> Result<UpstreamHttpRequest, GatewayError> {
    outbound.into_transport_request(admitted)
}

const fn anthropic_upstream_response_mode(mode: ResponsesResponseMode) -> AnthropicResponseMode {
    match mode {
        ResponsesResponseMode::NonStreaming => AnthropicResponseMode::NonStreaming,
        ResponsesResponseMode::Streaming => AnthropicResponseMode::Streaming,
    }
}

const fn grok_build_execution_mode(mode: ResponsesResponseMode) -> GrokBuildExecutionMode {
    match mode {
        ResponsesResponseMode::NonStreaming => GrokBuildExecutionMode::NonStreaming,
        ResponsesResponseMode::Streaming => GrokBuildExecutionMode::Streaming,
    }
}

const fn grok_console_execution_mode(mode: ResponsesResponseMode) -> GrokConsoleExecutionMode {
    match mode {
        ResponsesResponseMode::NonStreaming => GrokConsoleExecutionMode::NonStreaming,
        ResponsesResponseMode::Streaming => GrokConsoleExecutionMode::Streaming,
    }
}

const fn grok_official_execution_mode(mode: ResponsesResponseMode) -> GrokOfficialExecutionMode {
    match mode {
        ResponsesResponseMode::NonStreaming => GrokOfficialExecutionMode::NonStreaming,
        ResponsesResponseMode::Streaming => GrokOfficialExecutionMode::Streaming,
    }
}

const fn chat_upstream_response_mode(mode: ResponsesResponseMode) -> ChatResponseMode {
    match mode {
        ResponsesResponseMode::NonStreaming => ChatResponseMode::NonStreaming,
        ResponsesResponseMode::Streaming => ChatResponseMode::Streaming,
    }
}
fn p12_transport_request(
    outbound: &OpenAiResponsesOutboundRequest,
    admitted: AdmittedEgressTarget,
    codex_oauth: bool,
    account_id: Option<&str>,
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

    let mut headers = vec![
        ("accept".to_owned(), accept),
        ("authorization".to_owned(), authorization),
        ("content-type".to_owned(), content_type),
        (
            "user-agent".to_owned(),
            if codex_oauth {
                P12_CODEX_OAUTH_USER_AGENT.to_owned()
            } else {
                P12_KRILL_COMPATIBILITY_USER_AGENT.to_owned()
            },
        ),
    ];
    if codex_oauth {
        let account_id = account_id.ok_or_else(credential_unavailable_error)?;
        headers.push(("chatgpt-account-id".to_owned(), account_id.to_owned()));
        headers.push((
            "openai-beta".to_owned(),
            "responses=experimental".to_owned(),
        ));
        headers.push((
            "originator".to_owned(),
            P12_CODEX_OAUTH_ORIGINATOR.to_owned(),
        ));
        headers.push(("version".to_owned(), P12_CODEX_OAUTH_VERSION.to_owned()));
    }
    UpstreamHttpRequest::try_new(
        admitted,
        UpstreamHttpMethod::Post,
        headers,
        outbound.body().to_vec(),
    )
    .map_err(|_| internal_error())
}

#[cfg(test)]
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
#[cfg(test)]
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
#[cfg(test)]
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

fn expected_content_type_matches(
    content_type: Option<&str>,
    mode: ResponsesResponseMode,
    allow_missing: bool,
) -> bool {
    let expected = match mode {
        ResponsesResponseMode::NonStreaming => "application/json",
        ResponsesResponseMode::Streaming => "text/event-stream",
    };
    content_type.map_or(allow_missing, |content_type| {
        content_type.starts_with(expected)
    })
}

/// The official `ChatGPT` Codex OAuth endpoint currently omits `content-type` on successful
/// Responses replies.  Only that explicitly authenticated route may opt into the missing-header
/// branch; a present but wrong media type remains a protocol failure, and the strict downstream
/// JSON/SSE decoder still owns the body shape.
fn has_expected_content_type(
    response: &UpstreamHttpResponse,
    mode: ResponsesResponseMode,
    allow_missing: bool,
) -> bool {
    expected_content_type_matches(
        response
            .header("content-type")
            .and_then(|value| value.to_str().ok()),
        mode,
        allow_missing,
    )
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

/// Buffers one complete Anthropic Messages JSON body under the shared response bound.
///
/// The ledger order, the bounded append, and the failure classification are exactly
/// [`decode_json_response`]'s; only the codec that projects the bytes onto Canonical events
/// differs. No usage projection applies: the Anthropic decoder already emits exactly the counters
/// an Anthropic usage object can carry.
async fn decode_anthropic_json_response(
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
    let body = std::str::from_utf8(&body).map_err(|_| AttemptFailure::BootstrapTruncated)?;
    let events = decode_upstream_response(body).map_err(|_| AttemptFailure::BootstrapTruncated)?;
    Ok(project_usage_events(events, usage_projection))
}

/// Buffers and strictly decodes one complete Chat Completions response.
async fn decode_chat_json_response(
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
        let Some(chunk) = next else { break };
        append_response_chunk(&mut body, &chunk).map_err(AttemptFailure::NonRetryable)?;
    }
    attempt_stages.record_stage(request_id, ManagementRequestAttemptStage::Decoder);
    let body = std::str::from_utf8(&body).map_err(|_| AttemptFailure::BootstrapTruncated)?;
    let events =
        decode_chat_upstream_response(body).map_err(|_| AttemptFailure::BootstrapTruncated)?;
    Ok(project_usage_events(events, usage_projection))
}

/// Streams one native Chat Completions SSE response through the protocol-owned decoder.
struct ChatSseEventSource {
    response: UpstreamHttpResponse,
    decoder: OpenAiChatSseDecoder,
    usage_projection: P12ResponseUsageProjection,
    pending: VecDeque<CanonicalEvent>,
    progress_deadline: Duration,
    progress_wait_spent: Duration,
}

impl ChatSseEventSource {
    async fn begin(
        response: UpstreamHttpResponse,
        usage_projection: P12ResponseUsageProjection,
    ) -> Result<Self, AttemptFailure> {
        let mut source = Self {
            response,
            decoder: OpenAiChatSseDecoder::new(),
            usage_projection,
            pending: VecDeque::new(),
            progress_deadline: P12_STREAMING_PROGRESS_TIMEOUT,
            progress_wait_spent: Duration::ZERO,
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
        while self.pending.is_empty() && !self.decoder.is_finished() {
            if self.progress_wait_spent >= self.progress_deadline {
                return Err(provider_transient_error());
            }
            let wait_started = Instant::now();
            let Some(chunk) = self.response.next_chunk().await? else {
                self.pending.extend(self.decoder.finish()?);
                break;
            };
            self.progress_wait_spent = self
                .progress_wait_spent
                .saturating_add(wait_started.elapsed());
            let events = self.decoder.push(&chunk)?;
            if !events.is_empty() {
                self.progress_wait_spent = Duration::ZERO;
            }
            self.pending.extend(events);
        }
        Ok(())
    }
}

impl ResponsesEventSource for ChatSseEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(project_usage_event(event, self.usage_projection)));
            }
            if self.decoder.is_finished() {
                return Ok(None);
            }
            self.read_until_event().await?;
            Ok(self
                .pending
                .pop_front()
                .map(|event| project_usage_event(event, self.usage_projection)))
        })
    }
}

/// Narrows every decoded `UsageDelta` to what the client's protocol can encode.
///
/// The upstream format decides which counters exist; the client format decides which may be
/// emitted. An Anthropic upstream always reports cache-input counters, so a Responses client
/// would fail its encode without this narrowing.
fn project_usage_events(
    events: Vec<CanonicalEvent>,
    usage_projection: P12ResponseUsageProjection,
) -> Vec<CanonicalEvent> {
    events
        .into_iter()
        .map(|event| project_usage_event(event, usage_projection))
        .collect()
}

/// Narrows one decoded event's Usage to what the client's protocol can encode.
fn project_usage_event(
    event: CanonicalEvent,
    usage_projection: P12ResponseUsageProjection,
) -> CanonicalEvent {
    match event {
        CanonicalEvent::UsageDelta(delta) => {
            let usage =
                project_usage_for_response(Some(delta.usage), usage_projection).unwrap_or_default();
            CanonicalEvent::UsageDelta(UsageDelta { usage, ..delta })
        }
        other => other,
    }
}

/// Streams one Anthropic-compatible Messages SSE body under the P12 liveness bounds.
///
/// It is the Anthropic sibling of [`OpenAiSseEventSource`] and keeps that shell's exact
/// behaviour: the same semantic-progress deadline, the same rule that only `next_chunk` awaits
/// accrue against it, the same bootstrap requirement that the first Canonical event be a
/// `ResponseStart`, and the same truncation failure when the body ends before a terminal event.
/// The bounded frame, tool-argument, and progress-free-frame ceilings live inside the shared
/// `protocol-anthropic` decoder rather than being restated here.
struct AnthropicSseEventSource {
    response: UpstreamHttpResponse,
    decoder: AnthropicMessagesSseDecoder,
    /// Narrows each decoded `UsageDelta` to what the client's protocol can encode.
    usage_projection: P12ResponseUsageProjection,
    /// One decoded event held back so bootstrap can prove the stream opened with `ResponseStart`.
    ///
    /// The shared decoder exposes no peek, so the shell owns the one-event lookahead instead.
    lookahead: Option<CanonicalEvent>,
    /// Upstream-wait budget between two decoder progress marks before the stream is declared
    /// wedged.
    progress_deadline: Duration,
    /// The decoder progress-mark count already accounted for by `progress_wait_spent`.
    observed_progress_marks: u64,
    /// Time spent awaiting upstream chunks since the last progress frame.
    progress_wait_spent: Duration,
}

impl AnthropicSseEventSource {
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
            decoder: AnthropicMessagesSseDecoder::new(),
            usage_projection,
            lookahead: None,
            progress_deadline,
            observed_progress_marks: 0,
            progress_wait_spent: Duration::ZERO,
        };
        source
            .read_until_event()
            .await
            .map_err(|_| AttemptFailure::BootstrapTruncated)?;
        if !matches!(source.lookahead, Some(CanonicalEvent::ResponseStart(_))) {
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
            if let Some(event) = self.decoder.take_event() {
                self.lookahead = Some(event);
                return Ok(());
            }
            if self.decoder.is_finished() {
                return Ok(());
            }
            // The transport's byte-idle bound wakes the wait below at least once per idle
            // window, so this check runs even when the upstream sends only `ping` frames that
            // reset that byte-idle timer.
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

impl ResponsesEventSource for AnthropicSseEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            if let Some(event) = self.lookahead.take() {
                return Ok(Some(project_usage_event(event, self.usage_projection)));
            }
            if let Some(event) = self.decoder.take_event() {
                return Ok(Some(project_usage_event(event, self.usage_projection)));
            }
            if self.decoder.is_finished() {
                return Ok(None);
            }
            self.read_until_event().await?;
            Ok(self
                .lookahead
                .take()
                .map(|event| project_usage_event(event, self.usage_projection)))
        })
    }
}

async fn decode_json_response_with_reasoning_policy(
    response: &mut UpstreamHttpResponse,
    attempt_stages: &P12AttemptStageStore,
    request_id: &RequestId,
    usage_projection: P12ResponseUsageProjection,
    suppress_reasoning: bool,
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
    let body = std::str::from_utf8(&body).map_err(|_| AttemptFailure::BootstrapTruncated)?;
    let events = decode_responses_upstream_response_with_reasoning_policy(body, suppress_reasoning)
        .map_err(|_| AttemptFailure::BootstrapTruncated)?;
    Ok(project_usage_events(events, usage_projection))
}

/// Fixed, host-independent Kiro environment projection.
///
/// Kiro places this in `userInputMessageContext`. It is a constant rather than a reading of this
/// server's real OS or working directory: the gateway is not the client, and leaking a real server
/// path or kernel version into an upstream request would be both wrong and a disclosure.
const P12_KIRO_OPERATING_SYSTEM: &str = "linux";
const P12_KIRO_WORKING_DIRECTORY: &str = "/";

/// Kiro's request shape has no output-token limit at all: `conversationState` carries `content`,
/// `modelId`, `origin`, `envState`, optional `tools` and optional `outputConfig.effort`, and nothing
/// that expresses a maximum. Anthropic Messages, meanwhile, *requires* `max_tokens`, which the
/// inbound decoder retains as [`P12_ANTHROPIC_MAX_TOKENS_EXTENSION`] rather than inventing a
/// canonical field for it. `BC-PROVIDER-007` rejects every root extension, which is correct for a
/// converter that must not silently discard a client's semantics.
///
/// Composing the two as-is makes the channel reject 100% of requests, because a compliant Anthropic
/// client always sends `max_tokens`. This projection is the deliberate resolution: drop that one
/// output *ceiling* the upstream protocol cannot express, and only for Kiro. It is the same choice
/// the reference kiro-rs implementation makes -- it accepts `max_tokens` on its Anthropic surface and
/// forwards nothing, because Kiro has nowhere to put it.
///
/// Dropping a ceiling cannot corrupt a response: the client asked for at most N tokens and receives
/// a complete answer, which may be shorter or longer. Every *other* extension is retained, so a
/// semantic a client actually depends on still fails closed inside the converter -- with the
/// converter's own classification -- rather than being silently ignored here.
fn p12_kiro_request_projection(request: &CanonicalRequest) -> CanonicalRequest {
    if request.extensions.is_empty() {
        return request.clone();
    }
    let mut retained = RawExtensions::default();
    for (name, value) in request.extensions.iter() {
        if name == P12_ANTHROPIC_MAX_TOKENS_EXTENSION {
            continue;
        }
        // `try_insert` fails only on a duplicate name, and the source is itself a map keyed by
        // name, so a collision is unreachable. Silently ignoring the result would be a real drop,
        // so on the impossible branch keep the request whole and let the converter judge it.
        if retained.try_insert(name.to_owned(), value.clone()).is_err() {
            return request.clone();
        }
    }
    let mut projected = request.clone();
    projected.extensions = retained;
    projected
}

/// Refuses hidden Enterprise profile I/O in the request path.
///
/// `resolve_profile_arn` turns this refusal into its reviewed deterministic, Region-aware fallback.
/// P12-08F1 may later supply a refreshed immutable profile snapshot, but an inference request must
/// never discover ambient state or perform an authenticated profile lookup on its own.
struct P12KiroNoEnterpriseLookup;

impl KiroEnterpriseProfileLookup for P12KiroNoEnterpriseLookup {
    fn lookup(&self, _api_region: &KiroApiRegion) -> Result<String, KiroProfileArnError> {
        Err(KiroProfileArnError::InvalidProfileArn)
    }
}

/// Adapts a Kiro `CanonicalEventSource` to the Responses executor's source trait.
///
/// Both traits are the same shape over the same boxed future type; only their names differ, because
/// one is owned by the Provider boundary and the other by the Router. This wrapper is the single
/// place that observation is made, so neither crate needs to know about the other.
struct P12ProviderEventSource {
    inner: Box<dyn CanonicalEventSource>,
}

impl P12ProviderEventSource {
    const fn new(inner: Box<dyn CanonicalEventSource>) -> Self {
        Self { inner }
    }
}

impl ResponsesEventSource for P12ProviderEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        self.inner.next_event()
    }
}

/// Maps a Kiro start failure onto the executor's retryability decision.
///
/// Every failure here happens strictly before the first semantic event, so retrying is permitted by
/// BL-05. The Kiro adapter has already classified the upstream response into a safe `GatewayError`;
/// this only decides whether the executor may try the next candidate. The mapping is an explicit
/// allow-list rather than a catch-all: a class this composition has not reasoned about must fail
/// non-retryably instead of silently burning every candidate on the same defect.
fn p12_classify_kiro_start_failure(error: GatewayError) -> AttemptFailure {
    match error.code() {
        // Transient upstream and egress classes: the next candidate may well succeed.
        GatewayErrorCode::ProviderTransient | GatewayErrorCode::EgressUnavailable => {
            AttemptFailure::Connection
        }
        // A throttled Kiro account should fail over rather than fail the request. The adapter does
        // not surface a Retry-After, so none is claimed.
        GatewayErrorCode::ProviderRateLimited | GatewayErrorCode::CredentialQuotaExceeded => {
            AttemptFailure::RateLimited { retry_after: None }
        }
        _ => AttemptFailure::NonRetryable(error),
    }
}

/// Maps an already-classified Grok pre-first-semantic failure onto Router retry ownership.
fn p12_classify_grok_start_failure(error: GatewayError) -> AttemptFailure {
    match error.code() {
        GatewayErrorCode::ProviderTransient => AttemptFailure::ServerError,
        GatewayErrorCode::EgressUnavailable => AttemptFailure::Connection,
        GatewayErrorCode::ProviderRateLimited | GatewayErrorCode::CredentialQuotaExceeded => {
            AttemptFailure::RateLimited { retry_after: None }
        }
        _ => AttemptFailure::NonRetryable(error),
    }
}

#[cfg(test)]
fn decode_json_events(body: &[u8]) -> Result<Vec<CanonicalEvent>, GatewayError> {
    decode_json_events_with_usage_projection(body, P12ResponseUsageProjection::OpenAiResponses)
}

#[cfg(test)]
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
        // The projection is CLIENT-scoped: it narrows a decoded upstream Usage to what the
        // protocol this request arrived on can encode, whatever upstream produced it.
        match usage_projection {
            P12ResponseUsageProjection::AnthropicMessages => {
                // Anthropic reports the aggregate output count but has no representation for the
                // OpenAI-specific reasoning/cached sub-counters. Keep every representable total
                // and cache-input field so the Messages boundary does not fail after a decode.
                usage.reasoning_tokens = None;
                usage.cached_tokens = None;
            }
            P12ResponseUsageProjection::OpenAiResponses => {
                // The Responses encoder has no field for Anthropic's cache-input counters, which
                // an Anthropic upstream reports on every response (as `0` when unused, so they
                // arrive present rather than absent) and would otherwise fail the encode.
                usage.cache_read_tokens = None;
                usage.cache_creation_tokens = None;
            }
        }
        usage
    })
}

#[cfg(test)]
fn initial_usage_snapshot(usage: &Usage) -> Usage {
    Usage {
        input_tokens: usage.input_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_creation_tokens: usage.cache_creation_tokens,
        cached_tokens: usage.cached_tokens,
        ..Usage::default()
    }
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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
    decoder: OpenAiResponsesSseDecoder,
    usage_projection: P12ResponseUsageProjection,
    pending: VecDeque<CanonicalEvent>,
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
#[cfg(test)]
struct OpenAiSseDecoder {
    buffer: Vec<u8>,
    /// Bytes of `buffer` before this offset belong to frames already extracted by `take_frame`.
    consumed: usize,
    /// Scan resume point: no frame delimiter starts inside `buffer[self.consumed..self.scanned]`.
    scanned: usize,
    pending: VecDeque<CanonicalEvent>,
    /// Validates every event before it is queued, so the decoder cannot emit an illegal sequence.
    ///
    /// The protocol encoders downstream run the same state machine, but they run past the point
    /// where the client is already committed. Rejecting here turns a would-be mid-stream encoder
    /// failure into a pre-first-byte decode failure that the orchestrator can still fail over.
    state: CanonicalEventState,
    lifecycle: SseLifecycle,
    usage_projection: P12ResponseUsageProjection,
    /// Consecutive frames that proved only socket liveness, reset by any progress frame.
    progress_free_frames: usize,
    /// Monotone count of consumed frames that proved generation is advancing.
    progress_marks: u64,
}

/// The bounded lifecycle of one streamed Responses body.
#[cfg(test)]
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
#[cfg(test)]
#[derive(Default)]
struct SseStreamingState {
    message_open: bool,
    emitted_content: bool,
    tool_calls: BTreeMap<String, SseToolCall>,
    call_ids: BTreeSet<String>,
    retained_argument_bytes: usize,
}

/// One streamed Tool call correlated to its upstream output item identifier.
#[cfg(test)]
struct SseToolCall {
    call_id: String,
    assembled: String,
    released: usize,
    ended: bool,
}

#[cfg(test)]
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

/// Validates one Canonical event against the response state machine, then queues it.
///
/// Every emission in this decoder goes through here: the state machine is what makes an illegal
/// Canonical sequence unreachable by construction rather than merely untested.
#[cfg(test)]
fn queue_event(
    state: &mut CanonicalEventState,
    pending: &mut VecDeque<CanonicalEvent>,
    event: CanonicalEvent,
) -> Result<(), GatewayError> {
    state.apply(&event)?;
    pending.push_back(event);
    Ok(())
}

#[cfg(test)]
impl SseStreamingState {
    /// Opens the one Canonical Message that carries every output item of this response.
    fn ensure_message(
        &mut self,
        state: &mut CanonicalEventState,
        pending: &mut VecDeque<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        if self.message_open {
            return Ok(());
        }
        queue_event(
            state,
            pending,
            CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }),
        )?;
        self.message_open = true;
        Ok(())
    }

    /// Declares one streamed Tool call from a `function_call` output item.
    ///
    /// Identifiers longer than [`MAX_SSE_IDENTIFIER_BYTES`] fail closed: both are retained for
    /// the rest of the response, so the retained total must stay bounded by a small constant
    /// rather than by the frame bound.
    fn start_tool_call(
        &mut self,
        state: &mut CanonicalEventState,
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
        self.ensure_message(state, pending)?;
        queue_event(
            state,
            pending,
            CanonicalEvent::ToolCallStart(ToolCallStart {
                call_id: call_id.clone(),
                name,
                extensions: RawExtensions::default(),
            }),
        )?;
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
        state: &mut CanonicalEventState,
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
        call.release_arguments(state, pending)?;
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
        state: &mut CanonicalEventState,
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
            call.release_arguments(state, pending)?;
            self.retained_argument_bytes = next;
        }
        queue_event(
            state,
            pending,
            CanonicalEvent::ToolCallEnd(ToolCallEnd {
                call_id: call.call_id.clone(),
                arguments: call.completed_arguments()?,
                extensions: RawExtensions::default(),
            }),
        )?;
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

#[cfg(test)]
impl SseToolCall {
    /// Delivers every assembled argument byte that the JSON value already frames.
    ///
    /// Whitespace outside the value is held back: `RawJson` retains only the value itself, so
    /// releasing padding would desynchronize the delivered fragments from the completed arguments
    /// that both protocol encoders compare them against.
    fn release_arguments(
        &mut self,
        state: &mut CanonicalEventState,
        pending: &mut VecDeque<CanonicalEvent>,
    ) -> Result<(), GatewayError> {
        let (start, end) = self.value_bounds();
        let from = self.released.max(start);
        if end <= from {
            return Ok(());
        }
        let delta = self.assembled[from..end].to_owned();
        self.released = end;
        queue_event(
            state,
            pending,
            CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
                call_id: self.call_id.clone(),
                delta,
                extensions: RawExtensions::default(),
            }),
        )
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

#[cfg(test)]
impl OpenAiSseDecoder {
    fn new(usage_projection: P12ResponseUsageProjection) -> Self {
        Self {
            buffer: Vec::new(),
            consumed: 0,
            scanned: 0,
            pending: VecDeque::new(),
            state: CanonicalEventState::default(),
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
    fn note_progress_free_frame(&mut self) -> Result<(), GatewayError> {
        self.progress_free_frames = self.progress_free_frames.saturating_add(1);
        if self.progress_free_frames > MAX_SSE_PROGRESS_FREE_FRAMES {
            queue_event(
                &mut self.state,
                &mut self.pending,
                CanonicalEvent::StreamError(StreamError {
                    error: provider_transient_error(),
                }),
            )?;
            self.lifecycle = SseLifecycle::Finished;
        }
        Ok(())
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
            self.note_progress_free_frame()?;
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
            self.note_progress_free_frame()?;
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
        queue_event(
            &mut self.state,
            &mut self.pending,
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id,
                extensions: RawExtensions::default(),
            }),
        )?;
        // Anthropic's Messages representation needs the reported input usage before MessageStart,
        // exactly as the non-streaming decoder supplies it.  Usage the upstream did not report is
        // never invented here.
        if let Some(usage) = usage.as_ref().filter(|usage| usage.input_tokens.is_some()) {
            queue_event(
                &mut self.state,
                &mut self.pending,
                CanonicalEvent::UsageDelta(UsageDelta {
                    usage: initial_usage_snapshot(usage),
                    is_final: false,
                    extensions: RawExtensions::default(),
                }),
            )?;
        }
        Ok(())
    }

    fn consume_output_item_added(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item = value.get("item").ok_or_else(upstream_protocol_error)?;
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        let streaming = lifecycle.streaming_state()?;
        match item.get("type").and_then(Value::as_str) {
            Some("message") if item.get("role").and_then(Value::as_str) == Some("assistant") => {
                streaming.ensure_message(state, pending)
            }
            // A Responses model may open an internal reasoning item before its visible output.
            // P12 does not expose it, but it must not fail an otherwise valid response.
            Some("reasoning") => Ok(()),
            Some("function_call") => streaming.start_tool_call(state, item, pending),
            _ => Err(upstream_protocol_error()),
        }
    }

    fn consume_output_text_delta(&mut self, value: &Value) -> Result<(), GatewayError> {
        let delta = value
            .get("delta")
            .and_then(Value::as_str)
            .ok_or_else(upstream_protocol_error)?;
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        let streaming = lifecycle.streaming_state()?;
        if !streaming.message_open {
            return Err(upstream_protocol_error());
        }
        // An empty fragment carries no client-visible semantics and cannot become a Canonical
        // TextDelta, so it is dropped instead of failing the stream.
        if delta.is_empty() {
            return Ok(());
        }
        streaming.emitted_content = true;
        queue_event(
            state,
            pending,
            CanonicalEvent::TextDelta(TextDelta {
                text: delta.to_owned(),
                extensions: RawExtensions::default(),
            }),
        )
    }

    fn consume_function_arguments_delta(&mut self, value: &Value) -> Result<(), GatewayError> {
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        lifecycle
            .streaming_state()?
            .append_tool_arguments(state, value, pending)
    }

    fn consume_function_arguments_done(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item_id = required_string(value, "item_id")?;
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        lifecycle.streaming_state()?.end_tool_call(
            state,
            &item_id,
            value.get("arguments").and_then(Value::as_str),
            false,
            pending,
        )
    }

    fn consume_output_item_done(&mut self, value: &Value) -> Result<(), GatewayError> {
        let item = value.get("item").ok_or_else(upstream_protocol_error)?;
        if item.get("type").and_then(Value::as_str) != Some("function_call") {
            return Ok(());
        }
        let item_id = required_string(item, "id")?;
        let Self {
            lifecycle,
            state,
            pending,
            ..
        } = self;
        // A completed Function Call item is the last chance to close a Tool call whose upstream
        // omitted the dedicated arguments completion frame.
        lifecycle.streaming_state()?.end_tool_call(
            state,
            &item_id,
            item.get("arguments").and_then(Value::as_str),
            true,
            pending,
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
            queue_event(
                &mut self.state,
                &mut self.pending,
                CanonicalEvent::MessageEnd(MessageEnd::default()),
            )?;
        }
        if let Some(usage) = usage {
            queue_event(
                &mut self.state,
                &mut self.pending,
                CanonicalEvent::UsageDelta(UsageDelta {
                    usage,
                    is_final: true,
                    extensions: RawExtensions::default(),
                }),
            )?;
        }
        queue_event(
            &mut self.state,
            &mut self.pending,
            CanonicalEvent::ResponseEnd(ResponseEnd {
                stop_reason: Some(stop_reason),
                stop_sequence: None,
                extensions: RawExtensions::default(),
            }),
        )?;
        self.lifecycle = SseLifecycle::Finished;
        Ok(())
    }

    fn consume_response_failed(&mut self) -> Result<(), GatewayError> {
        if matches!(self.lifecycle, SseLifecycle::AwaitingResponseStart) {
            return Err(upstream_protocol_error());
        }
        queue_event(
            &mut self.state,
            &mut self.pending,
            CanonicalEvent::StreamError(StreamError {
                error: provider_transient_error(),
            }),
        )?;
        self.lifecycle = SseLifecycle::Finished;
        Ok(())
    }
}

impl OpenAiSseEventSource {
    async fn begin_with_reasoning_policy(
        response: UpstreamHttpResponse,
        usage_projection: P12ResponseUsageProjection,
        suppress_reasoning: bool,
    ) -> Result<Self, AttemptFailure> {
        Self::begin_with_progress_deadline_and_reasoning_policy(
            response,
            usage_projection,
            P12_STREAMING_PROGRESS_TIMEOUT,
            suppress_reasoning,
        )
        .await
    }

    /// Starts one streamed source under an explicit semantic-progress deadline.
    ///
    /// Production always passes [`P12_STREAMING_PROGRESS_TIMEOUT`] through [`Self::begin`]; the
    /// explicit parameter exists so tests can expire the deadline in milliseconds against a live
    /// peer instead of waiting out the production value.
    #[cfg(test)]
    async fn begin_with_progress_deadline(
        response: UpstreamHttpResponse,
        usage_projection: P12ResponseUsageProjection,
        progress_deadline: Duration,
    ) -> Result<Self, AttemptFailure> {
        Self::begin_with_progress_deadline_and_reasoning_policy(
            response,
            usage_projection,
            progress_deadline,
            false,
        )
        .await
    }

    async fn begin_with_progress_deadline_and_reasoning_policy(
        response: UpstreamHttpResponse,
        usage_projection: P12ResponseUsageProjection,
        progress_deadline: Duration,
        suppress_reasoning: bool,
    ) -> Result<Self, AttemptFailure> {
        let mut source = Self {
            response,
            decoder: if suppress_reasoning {
                OpenAiResponsesSseDecoder::new_with_reasoning_suppressed()
            } else {
                OpenAiResponsesSseDecoder::new()
            },
            usage_projection,
            pending: VecDeque::new(),
            progress_deadline,
            observed_progress_marks: 0,
            progress_wait_spent: Duration::ZERO,
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
            self.observe_decoder_progress();
            if !self.pending.is_empty() || self.decoder.is_finished() {
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
                self.pending.extend(
                    self.decoder
                        .finish()?
                        .into_iter()
                        .map(|event| project_usage_event(event, self.usage_projection)),
                );
                return Ok(());
            };
            self.pending.extend(
                self.decoder
                    .push(&chunk)?
                    .into_iter()
                    .map(|event| project_usage_event(event, self.usage_projection)),
            );
        }
    }
}

impl ResponsesEventSource for OpenAiSseEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            if self.pending.is_empty() && !self.decoder.is_finished() {
                self.read_until_event().await?;
            }
            Ok(self.pending.pop_front())
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

#[cfg(test)]
fn required_string(value: &Value, field: &str) -> Result<String, GatewayError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(upstream_protocol_error)
}

#[cfg(test)]
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

#[cfg(test)]
fn required_u64(value: &Value) -> Result<u64, GatewayError> {
    value.as_u64().ok_or_else(upstream_protocol_error)
}

struct SnapshotManagementRuntimeFacade {
    registry: Arc<RouteSnapshotRegistry>,
    attempt_stages: Arc<P12AttemptStageStore>,
    runtime_health: Arc<RuntimeHealthRegistry>,
    runtime_quota: Arc<RuntimeQuotaRegistry>,
    route_explain_scheduler: Option<Arc<RouteCredentialScheduler>>,
    routing_price_snapshot: Option<Arc<RoutingPriceSnapshot>>,
    event_store: SqliteEventStore,
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

    /// Recovers one operator-confirmed forbidden account through the controlled ticket flow.
    ///
    /// The operator's authenticated request is the account-level evidence (BL-16/BL-17): a
    /// forbidden binding never reopens from data-plane traffic because selection excludes it.
    /// Begin and complete stay one local transition; no Provider request is sent.
    fn recover_forbidden_account(
        &self,
        target: &ManagementRuntimeTarget,
        observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        let expires_at_ms = observed_at_ms
            .checked_add(P12_OPERATOR_RECOVERY_TTL_MS)
            .ok_or(ManagementRuntimeError::Unavailable)?;
        let Some(ticket) = self
            .runtime_health
            .begin_account_recovery(target.endpoint_id(), target.credential_id(), expires_at_ms)
            .map_err(|_| ManagementRuntimeError::Unavailable)?
        else {
            // A concurrent recovery already owns this binding; its owner reports the outcome.
            return Ok(ManagementQuotaRecoveryState::ProbeScheduled);
        };
        self.runtime_health
            .complete_account_recovery(ticket, RuntimeHealthAccountRecoveryResult::Allowed)
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        Ok(ManagementQuotaRecoveryState::ProbeScheduled)
    }

    /// Completes one due controlled quota recovery as an explicit operator override.
    ///
    /// A pre-Reset exhausted window is refused (`RecoveryRequired`): BL-17 admits a controlled
    /// probe only after Reset, and an operator cannot move an upstream reset window. The live
    /// selection path remains the automatic probe owner; this override exists for a due target
    /// that receives no traffic.
    fn recover_quota_target(
        &self,
        target: &ManagementRuntimeTarget,
        observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        let quota_target = match target.upstream_model() {
            Some(model) => RuntimeQuotaTarget::endpoint_credential_model(
                target.endpoint_id().clone(),
                target.credential_id().clone(),
                model,
            )
            .map_err(|_| ManagementRuntimeError::InvalidInput)?,
            None => RuntimeQuotaTarget::endpoint_credential(
                target.endpoint_id().clone(),
                target.credential_id().clone(),
            ),
        };
        let availability = self
            .runtime_quota
            .availability_at(&quota_target, observed_at_ms)
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        match availability {
            RuntimeQuotaAvailability::Available => Ok(ManagementQuotaRecoveryState::Rejected),
            RuntimeQuotaAvailability::Exhausted { .. } => {
                Ok(ManagementQuotaRecoveryState::RecoveryRequired)
            }
            RuntimeQuotaAvailability::RecoveryProbeInFlight { .. } => {
                Ok(ManagementQuotaRecoveryState::ProbeScheduled)
            }
            RuntimeQuotaAvailability::RecoveryRequired { .. } => {
                self.complete_due_quota_recovery(quota_target, observed_at_ms)
            }
        }
    }

    fn complete_due_quota_recovery(
        &self,
        quota_target: RuntimeQuotaTarget,
        observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        let expires_at_ms = observed_at_ms
            .checked_add(P12_OPERATOR_RECOVERY_TTL_MS)
            .ok_or(ManagementRuntimeError::Unavailable)?;
        let Some(ticket) = self
            .runtime_quota
            .begin_recovery_probe(&quota_target, expires_at_ms)
            .map_err(|_| ManagementRuntimeError::Unavailable)?
        else {
            // The live selection path already owns a probe; it reports the outcome.
            return Ok(ManagementQuotaRecoveryState::ProbeScheduled);
        };
        let snapshot = QuotaSnapshot::try_new(
            quota_target,
            Vec::new(),
            QuotaSource::Estimated,
            QuotaConfidence::Estimated,
            observed_at_ms,
        )
        .map_err(|_| ManagementRuntimeError::Unavailable)?;
        self.runtime_quota
            .complete_recovery_probe(ticket, snapshot)
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        Ok(ManagementQuotaRecoveryState::ProbeScheduled)
    }
}

fn management_route_explain_reason(candidate: &RouteExplainCandidate) -> &'static str {
    candidate
        .reasons()
        .iter()
        .map(|reason| match reason {
            RouteExplainCandidateReason::NotHardEligible => "not_hard_eligible",
            RouteExplainCandidateReason::EndpointHealth(availability) => match availability {
                gateway_router::RuntimeHealthAvailability::CoolingDown { .. } => {
                    "endpoint_cooldown"
                }
                gateway_router::RuntimeHealthAvailability::CircuitOpen { .. } => {
                    "endpoint_circuit_open"
                }
                gateway_router::RuntimeHealthAvailability::AccountForbidden
                | gateway_router::RuntimeHealthAvailability::CredentialUnauthorized
                | gateway_router::RuntimeHealthAvailability::AccountRecoveryInFlight { .. }
                | gateway_router::RuntimeHealthAvailability::Available => "endpoint_unavailable",
            },
            RouteExplainCandidateReason::EndpointHealthUnavailable => "endpoint_unavailable",
            RouteExplainCandidateReason::MissingCredentialPool => "missing_credential_pool",
            RouteExplainCandidateReason::NoEligibleCredential => "no_eligible_credential",
        })
        .next()
        .unwrap_or("no_eligible_credential")
}

const fn management_price_evidence(value: ProviderScopedPriceEvidence) -> &'static str {
    match value {
        ProviderScopedPriceEvidence::Dominant => "dominant",
        ProviderScopedPriceEvidence::Equal => "equal",
        ProviderScopedPriceEvidence::Dominated => "dominated",
        ProviderScopedPriceEvidence::Incomparable => "incomparable",
        ProviderScopedPriceEvidence::Unpriced => "unpriced",
        ProviderScopedPriceEvidence::NotEvaluated => "not_evaluated",
    }
}

const fn management_price_comparison(value: RoutingPriceComparison) -> &'static str {
    match value {
        RoutingPriceComparison::RateDominanceV1 => "rate_dominance_v1",
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
        target: &ManagementRuntimeTarget,
        observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        self.snapshot_for(config_version_id)?;
        let account_status = self
            .runtime_health
            .credential_account_status_at(
                target.endpoint_id(),
                target.credential_id(),
                observed_at_ms,
            )
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        match account_status {
            RuntimeCredentialAccountStatus::Unauthorized => {
                if target.upstream_model().is_some() {
                    return Ok(ManagementQuotaRecoveryState::Rejected);
                }
                self.recover_forbidden_account(target, observed_at_ms)
            }
            RuntimeCredentialAccountStatus::Forbidden => {
                // An account block covers the whole binding, so only a binding-scoped request may
                // clear it. A model-scoped target expresses quota intent; honouring it here would
                // let a request addressing one model lift an account-level block it never named.
                if target.upstream_model().is_some() {
                    return Ok(ManagementQuotaRecoveryState::Rejected);
                }
                self.recover_forbidden_account(target, observed_at_ms)
            }
            RuntimeCredentialAccountStatus::RecoveryInFlight { .. } => {
                Ok(ManagementQuotaRecoveryState::ProbeScheduled)
            }
            RuntimeCredentialAccountStatus::Available => {
                self.recover_quota_target(target, observed_at_ms)
            }
        }
    }

    #[allow(clippy::too_many_lines)] // Keep the value-free scope/admission/reason projection together.
    fn explain_route(
        &mut self,
        request: &ManagementRouteExplainRequest,
    ) -> Result<ManagementRouteExplain, ManagementRuntimeError> {
        let snapshot = self.snapshot_for(request.config_version_id())?;
        if self
            .route_explain_scheduler
            .as_ref()
            .is_some_and(|scheduler| {
                scheduler.snapshot_version().as_str() != request.config_version_id().as_str()
            })
            || self.routing_price_snapshot.as_ref().is_some_and(|price| {
                price.config_version_id() != request.config_version_id()
                    || price.snapshot_version().as_str() != request.config_version_id().as_str()
            })
        {
            return Err(ManagementRuntimeError::Unavailable);
        }
        let public_model = snapshot
            .resolve_public_model(request.requested_model())
            .filter(|model| model.route_id() == request.route_id())
            .ok_or(ManagementRuntimeError::Unavailable)?;
        let route = snapshot
            .route(public_model.route_id())
            .filter(|route| route.id() == request.route_id())
            .ok_or(ManagementRuntimeError::Unavailable)?;
        let source = match request.protocol() {
            ManagementRequestProtocol::OpenAiChatCompletions => {
                ProtocolFormat::OpenAiChatCompletions
            }
            ManagementRequestProtocol::OpenAiResponses => ProtocolFormat::OpenAiResponses,
            ManagementRequestProtocol::AnthropicMessages => ProtocolFormat::AnthropicMessages,
        };
        let pair_is_publishable = |candidate: &SnapshotRouteCandidate| {
            candidate.protocol_format().is_some_and(|target| {
                protocol_pair_is_publishable(
                    source,
                    target,
                    candidate.transform_mode(),
                    candidate.effective_capabilities(),
                )
            })
        };
        let admitted_candidate_ids = route
            .candidates()
            .iter()
            .filter(|candidate| candidate.is_hard_eligible() && pair_is_publishable(candidate))
            .map(|candidate| candidate.id().clone())
            .collect::<BTreeSet<_>>();
        let provider_ids = admitted_candidate_ids
            .iter()
            .filter_map(|candidate_id| {
                route
                    .candidates()
                    .iter()
                    .find(|candidate| candidate.id() == candidate_id)
                    .map(|candidate| candidate.upstream_id().clone())
            })
            .map(|upstream_id| {
                ProviderId::try_new(upstream_id.as_str().to_owned())
                    .map_err(|_| ManagementRuntimeError::Unavailable)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let inferred_provider = match request.provider_id().cloned() {
            Some(provider_id) => Some(provider_id),
            None if provider_ids.len() == 1 => provider_ids.iter().next().cloned(),
            None => None,
        };

        let projected = if let Some(scheduler) = &self.route_explain_scheduler {
            let route_input = RouteExplainInput::new(route.id().clone(), request.observed_at_ms());
            if let Some(provider_id) = inferred_provider.clone() {
                let composition_input = ProviderScopedRouteExplainInput::try_new(
                    route_input,
                    provider_id,
                    admitted_candidate_ids.clone(),
                    BTreeMap::new(),
                )
                .map_err(|_| ManagementRuntimeError::Unavailable)?;
                Some(
                    scheduler
                        .explain_provider_scoped(
                            &composition_input,
                            &self.runtime_health,
                            &self.runtime_quota,
                            &AttemptExclusionSet::new(),
                        )
                        .map_err(|_| ManagementRuntimeError::Unavailable)?,
                )
            } else {
                None
            }
        } else {
            None
        };
        let base_explain = if projected.is_none() {
            self.route_explain_scheduler
                .as_ref()
                .map(|scheduler| {
                    scheduler
                        .explain(
                            &RouteExplainInput::new(route.id().clone(), request.observed_at_ms()),
                            &self.runtime_health,
                            &self.runtime_quota,
                            &AttemptExclusionSet::new(),
                        )
                        .map_err(|_| ManagementRuntimeError::Unavailable)
                })
                .transpose()?
        } else {
            None
        };
        let selected = projected
            .as_ref()
            .and_then(|value| value.provider_selection().selected_candidate_id());
        let price_evidence_by_candidate = self
            .routing_price_snapshot
            .as_ref()
            .and(projected.as_ref())
            .map(|value| {
                value
                    .provider_selection()
                    .decisions()
                    .iter()
                    .map(|decision| {
                        (
                            decision.candidate().candidate_id().clone(),
                            management_price_evidence(decision.price_evidence()),
                        )
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let default_price_evidence = if self.routing_price_snapshot.is_some() {
            "not_evaluated"
        } else {
            "disabled"
        };
        let ambiguous_provider_scope = self.route_explain_scheduler.is_some()
            && request.provider_id().is_none()
            && provider_ids.len() > 1;
        let legacy_selected = if projected.is_none()
            && !ambiguous_provider_scope
            && request.provider_id().is_none()
        {
            route
                .candidates()
                .iter()
                .find(|candidate| candidate.is_hard_eligible() && pair_is_publishable(candidate))
                .map(|candidate| candidate.id().clone())
        } else {
            None
        };
        let candidates = route
            .candidates()
            .iter()
            .map(|candidate| {
                let pair_is_publishable = pair_is_publishable(candidate);
                let base_candidate = projected
                    .as_ref()
                    .map(ProviderScopedRouteExplainSnapshot::base)
                    .or(base_explain.as_ref())
                    .and_then(|snapshot| {
                        snapshot
                            .candidates()
                            .iter()
                            .find(|value| value.candidate_id() == candidate.id())
                    });
                let price_evidence = price_evidence_by_candidate
                    .get(candidate.id())
                    .copied()
                    .unwrap_or(default_price_evidence);
                let explain = if selected.is_some_and(|value| value == candidate.id())
                    || legacy_selected.as_ref() == Some(candidate.id())
                {
                    ManagementRouteExplainCandidate::selected(candidate.id().clone())
                } else if !candidate.is_hard_eligible() {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        "not_hard_eligible",
                    )
                } else if !pair_is_publishable {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        "protocol_transform_unavailable",
                    )
                } else if let Some(base_candidate) =
                    base_candidate.filter(|value| !value.is_eligible())
                {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        management_route_explain_reason(base_candidate),
                    )
                } else if ambiguous_provider_scope {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        "provider_scope_required",
                    )
                } else if request.provider_id().is_some_and(|provider_id| {
                    ProviderId::try_new(candidate.upstream_id().as_str().to_owned())
                        .is_ok_and(|candidate_provider| candidate_provider != *provider_id)
                }) {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        "provider_mismatch",
                    )
                } else {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        "after_selected_candidate",
                    )
                };
                explain.with_price_evidence(price_evidence)
            })
            .collect();
        let explain = ManagementRouteExplain::try_new(route.id().clone(), candidates)?;
        Ok(match &self.routing_price_snapshot {
            Some(price_snapshot) => {
                let policy = ManagementRouteExplainPricePolicy::new(
                    price_snapshot.catalog_version_id().to_owned(),
                    management_price_comparison(price_snapshot.comparison()),
                )?;
                explain.with_price_policy(policy)
            }
            None => explain,
        })
    }

    fn list_request_attempts(
        &mut self,
        request_id: &gateway_core::RequestId,
    ) -> Result<Vec<ManagementRequestAttempt>, ManagementRuntimeError> {
        let events = self
            .event_store
            .events_for_request(request_id)
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        let mut attempts = Vec::new();
        for stored in &events {
            let GatewayEvent::Attempt(attempt) = stored.event() else {
                continue;
            };
            let outcome = match attempt.outcome() {
                AttemptOutcome::Succeeded => "succeeded",
                AttemptOutcome::Failed(_) => "failed",
            };
            attempts.push(ManagementRequestAttempt::try_new(
                attempt.attempt_id().as_str().to_owned(),
                outcome,
                Some(attempt.endpoint_id().clone()),
                Some(attempt.credential_id().clone()),
            )?);
        }
        // The in-process ledger records per-attempt terminals and one request-level stage that
        // describes the newest attempt. It is enrichment, but it is also the only evidence that
        // an attempt this process observed is missing from the durable log: a terminal the
        // bounded queue rejected never becomes durable, so serving the shorter durable list as
        // success would report a failed attempt as never having happened. Fail closed instead,
        // and pair the stage only onto an exactly matching single-attempt timeline, where the
        // request-level stage provably describes that attempt and no in-flight successor.
        if let Ok(recorded) = self.attempt_stages.list_request_attempts(request_id) {
            if recorded.len() > attempts.len() {
                return Err(ManagementRuntimeError::Unavailable);
            }
            if recorded.len() == attempts.len()
                && recorded.len() == 1
                && recorded
                    .iter()
                    .zip(&attempts)
                    .all(|(ledger, stored)| ledger.attempt_id() == stored.attempt_id())
                && let Some(stage) = recorded.last().and_then(ManagementRequestAttempt::stage)
                && let Some(attempt) = attempts.pop()
            {
                attempts.push(attempt.with_stage(stage));
            }
        }
        Ok(attempts)
    }
}

fn route_not_found_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::RouteNotFound, ErrorScope::Model)
}

fn stale_runtime_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

fn credential_unavailable_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::CredentialUnavailable,
        ErrorScope::Credential,
    )
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
        collections::{BTreeMap, BTreeSet, VecDeque},
        error::Error,
        fs,
        net::{IpAddr, Ipv4Addr},
        num::NonZeroUsize,
        path::{Path, PathBuf},
        sync::{
            Arc, OnceLock,
            atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering},
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
    use gateway_catalog::{CapabilitySet, SemanticCapability};
    use gateway_control::{
        control_plane_service::{credential_associated_data, seal_compatible_proxy_node_endpoint},
        credential_pool_compiler::CredentialPoolCompiler,
        egress_policy_compiler::EgressPolicyCompiler,
        management_service::{ManagementActor, ManagementService},
    };
    use gateway_core::{
        AccessGroupId, AttemptEvent, AttemptOutcome, AttemptRetryDecision, CanonicalEvent,
        CanonicalEventState, CanonicalRequest, CanonicalResponse, ClientKeyId, CredentialId,
        EgressPolicyId, EndpointId, ErrorScope, EventEmission, GatewayError, GatewayErrorCode,
        GatewayEvent, GatewayEventSink, MessageEnd, MessageRole, MessageStart, ProviderId,
        PublicModelId, RawExtensions, RequestContext, RequestId, ResponseEnd, ResponseId,
        ResponseStart, RouteCandidateId, RouteId, TextDelta, TransparentRetryGate,
        TransparentRetryGateFuture, UpstreamId, Usage, UsageDelta,
    };
    use gateway_http_actix::{
        ResponsesHttpState, SystemResponsesMetadataFactory, configure, default_stream_capacity,
        management_resources::{
            ManagementQuotaRecoveryState, ManagementRequestAttemptStage, ManagementRequestProtocol,
            ManagementRouteExplainRequest, ManagementRuntimeError, ManagementRuntimeFacade,
            ManagementRuntimeTarget,
        },
    };
    use gateway_observability::{
        BoundedEventQueue, EventQueueConfig, NoopOpenTelemetryExporter, NoopStructuredJsonExporter,
        PrometheusMetrics, TelemetryPipeline, diagnostic_event,
    };
    use gateway_protocol::{ApiFormat, ApiFormatAdapterRegistry};
    use gateway_router::{
        AttemptDriver, AttemptFailure, AttemptOrchestrator, DeterministicMockEmission,
        DeterministicMockResponsesExecutor, NativePayloadAvailability, ProjectedProtocolRequest,
        ProtocolFormat, ProtocolTransformInput, ResponsesClientTransport, ResponsesEventSource,
        ResponsesExecution, ResponsesExecutor, ResponsesFuture, ResponsesResponseMode,
        RouteCredentialScheduler, RouteSnapshot, RouteSnapshotInput, RouteSnapshotRegistry,
        RuntimeCredentialAccountStatus, RuntimeHealthClock, RuntimeHealthClockError,
        RuntimeHealthRegistry, RuntimeQuotaRegistry, RuntimeQuotaTarget, SnapshotCatalogAdmission,
        SnapshotPublicModel, SnapshotRoute, SnapshotRouteCandidate, SnapshotRouteCandidateInput,
        SnapshotRoutePolicy, SnapshotTransformMode, SnapshotVersion,
        project_registered_protocol_request,
    };
    use gateway_store::{
        billing_ledger::{BillingCatalogSource, BillingPriceCatalog, BillingPriceEntry},
        control_plane::{
            AccessGroupConfiguration, AccessGroupRouteConfiguration, AdministrativeStatus,
            CompatibleEgressBindingConfiguration, CompatibleEgressTargetConfiguration,
            CompatibleProxyNodeConfiguration, CompatibleProxyNodeId,
            CompatibleProxyPoolConfiguration, CompatibleProxyPoolId, ConfigVersion,
            ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialConfiguration, CredentialScope, CredentialStatus, EgressPolicyConfiguration,
            EndpointConfiguration, EndpointCredentialBindingConfiguration, EndpointTransport,
            ModelAliasConfiguration, ModelRouteConfiguration, PublicModelConfiguration,
            RouteCandidateConfiguration, RoutePolicy, RoutingPriceComparison,
            RoutingPricePolicyConfiguration, SqliteControlPlaneRepository, StoredClientKey,
            StoredClientKeyStatus, StoredCompatibleFailureScope, StoredCompatibleStickiness,
            StoredEgressRedirectMode, TransformMode, UpstreamConfiguration,
        },
        event_store::{
            AsyncSqliteEventWriter, EventWriterConfig, GatewayEventLogKind, SqliteEventStore,
            StoredGatewayEvent,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };
    use gateway_upstream::{
        AdmittedEgressTarget, CompatibleEgressTarget, CompatibleFailureScope, CompatibleStickiness,
        CredentialSecret, EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy,
        EgressPolicyInput, EgressScheme, EndpointCredentialInput, EndpointCredentialPool,
        EndpointCredentialPools, RedirectPolicy, UpstreamClientPool, UpstreamHttpMethod,
        UpstreamHttpRequest, UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
    };
    use protocol_openai_responses::{
        ResponseMode, decode_request, decode_upstream_response as decode_responses_production,
    };
    use provider_kiro::endpoint_policy::KiroEndpointKind;
    use provider_openai_compatible::{
        OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesRequestBuilder,
    };
    use serde_json::Value;

    use super::{
        AnthropicSseEventSource, EndpointAdapter, EndpointAttemptDriver, EndpointRuntime,
        FiniteEventSource, GROK_BUILD_RESPONSES_BASE_URL, GROK_BUILD_RESPONSES_PATH,
        GROK_CONSOLE_RESPONSES_BASE_URL, GROK_CONSOLE_RESPONSES_PATH, GROK_OFFICIAL_API_BASE_URL,
        GROK_OFFICIAL_RESPONSES_PATH, GROK_WEB_CANARY_PATH, GROK_WEB_PRODUCTION_BASE_URL,
        MAX_SSE_FRAME_BYTES, MAX_SSE_IDENTIFIER_BYTES, MAX_SSE_PROGRESS_FREE_FRAMES,
        MAX_SSE_TOOL_CALLS, MAX_UPSTREAM_RESPONSE_BYTES, OpenAiSseDecoder, OpenAiSseEventSource,
        P12_BOOTSTRAP_TIMEOUT_MILLISECONDS, P12_CONNECT_TIMEOUT,
        P12_KRILL_COMPATIBILITY_USER_AGENT, P12_MAX_ROUTE_ATTEMPTS,
        P12_MAX_TOTAL_BINDING_CONCURRENCY, P12_NON_STREAMING_TOTAL_TIMEOUT,
        P12_STREAMING_IDLE_TIMEOUT, P12_STREAMING_PROGRESS_TIMEOUT, P12_STREAMING_TOTAL_TIMEOUT,
        P12_STREAMING_TTFB_TIMEOUT, P12AttemptStageStore, P12EndpointAdapterFactory,
        P12FanoutEventSink, P12ResponseUsageProjection, P12RoutedResponsesExecutor,
        P12TransportProfiles, P13_CHANNEL_PIN_MAX_IN_FLIGHT, P13ChannelPinInFlightGuard,
        RuntimeCompositionError, RuntimeCompositionStage, SnapshotManagementRuntimeFacade,
        append_response_chunk, build_data_plane_composition, build_grok_build_responses_adapter,
        build_grok_console_responses_adapter, build_grok_official_responses_adapter,
        build_grok_web_responses_adapter, build_kiro_messages_adapter,
        build_openai_responses_adapter, channel_pin_single_transport_adapter,
        classify_anthropic_response_failure, classify_openai_response_failure,
        compatible_egress_runtime_inputs, decode_json_events,
        decode_json_events_with_usage_projection, decode_sse_events,
        decode_sse_events_with_usage_projection, deployment_route_compiler, endpoint_runtimes,
        expected_content_type_matches, has_p12_https_only_egress_shape,
        has_p12_unlisted_model_override, p12_adapter_capabilities, p12_adapter_id_serves,
        p12_api_format_adapter_registry, p12_attempt_start_timeout,
        p12_candidate_override_is_admissible, p12_classify_kiro_start_failure,
        p12_kiro_endpoint_shape, p12_kiro_request_projection, p12_openai_compatible_request,
        p12_response_usage_projection, p12_transport_headers, p12_transport_request,
        p13_channel_pin_request_id, project_usage_events, queue_event, validate_endpoint_shape,
        validate_p12_credential_bindings,
    };

    const P12_SINGLETON_TEST_ENDPOINT_ID: &str = "p12-krill-endpoint";

    #[test]
    fn channel_pin_admission_is_bounded_and_releases_on_drop() -> Result<(), Box<dyn Error>> {
        let counter = AtomicUsize::new(0);
        let first = P13ChannelPinInFlightGuard::try_acquire(&counter)
            .map_err(|_| std::io::Error::other("first slot should be available"))?;
        let second = P13ChannelPinInFlightGuard::try_acquire(&counter)
            .map_err(|_| std::io::Error::other("second slot should be available"))?;
        assert_eq!(
            counter.load(Ordering::Acquire),
            P13_CHANNEL_PIN_MAX_IN_FLIGHT
        );
        assert!(P13ChannelPinInFlightGuard::try_acquire(&counter).is_err());
        drop(second);
        assert_eq!(counter.load(Ordering::Acquire), 1);
        let third = P13ChannelPinInFlightGuard::try_acquire(&counter)
            .map_err(|_| std::io::Error::other("released slot should be available"))?;
        assert_eq!(
            counter.load(Ordering::Acquire),
            P13_CHANNEL_PIN_MAX_IN_FLIGHT
        );
        drop(third);
        drop(first);
        assert_eq!(counter.load(Ordering::Acquire), 0);
        Ok(())
    }

    #[test]
    fn channel_pin_request_ids_have_restart_unique_boot_nonce() -> Result<(), Box<dyn Error>> {
        let first = p13_channel_pin_request_id()
            .map_err(|_| std::io::Error::other("request id should be generated"))?;
        let second = p13_channel_pin_request_id()
            .map_err(|_| std::io::Error::other("request id should be generated"))?;
        assert_ne!(first, second);
        assert!(first.as_str().starts_with("channel-pin-"));
        assert!(second.as_str().starts_with("channel-pin-"));
        Ok(())
    }

    #[test]
    fn channel_pin_rejects_native_adapters_before_the_transport_boundary()
    -> Result<(), Box<dyn Error>> {
        let generic = EndpointAdapter::OpenAiResponses(OpenAiResponsesEndpoint::try_new(
            "https://gateway.example.test/v1",
            "/responses",
        )?);
        assert!(channel_pin_single_transport_adapter(&generic));
        assert!(!channel_pin_single_transport_adapter(
            &EndpointAdapter::GrokBuildResponses
        ));
        assert!(!channel_pin_single_transport_adapter(
            &EndpointAdapter::GrokConsoleResponses
        ));
        assert!(!channel_pin_single_transport_adapter(
            &EndpointAdapter::GrokWebResponses
        ));
        assert!(!channel_pin_single_transport_adapter(
            &EndpointAdapter::GrokOfficialResponses
        ));
        Ok(())
    }

    #[test]
    fn codex_missing_response_content_type_is_scoped_and_strict_when_present() {
        assert!(expected_content_type_matches(
            None,
            ResponsesResponseMode::Streaming,
            true
        ));
        assert!(expected_content_type_matches(
            None,
            ResponsesResponseMode::NonStreaming,
            true
        ));
        assert!(!expected_content_type_matches(
            None,
            ResponsesResponseMode::Streaming,
            false
        ));
        assert!(!expected_content_type_matches(
            Some("application/json"),
            ResponsesResponseMode::Streaming,
            true
        ));
        assert!(expected_content_type_matches(
            Some("text/event-stream; charset=utf-8"),
            ResponsesResponseMode::Streaming,
            false
        ));
        assert!(expected_content_type_matches(
            Some("application/json; charset=utf-8"),
            ResponsesResponseMode::NonStreaming,
            false
        ));
    }

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

    /// One provider-neutral completion containing Text and final Usage semantics.
    fn p12_f2_text_body() -> String {
        sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-f2"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"msg-p12-f2","type":"message","role":"assistant","content":[]}}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg-p12-f2","output_index":0,"delta":"visible"}"#,
            r#"{"type":"response.output_item.done","output_index":0,"item":{"id":"msg-p12-f2","type":"message","role":"assistant","status":"completed"}}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-f2","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        ])
    }

    /// One provider-neutral completion containing Tool and final Usage semantics.
    fn p12_f2_tool_body() -> String {
        sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-f2-tool"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"fc-p12-f2","type":"function_call","call_id":"call-p12-f2","name":"echo","arguments":""}}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc-p12-f2","output_index":0,"delta":"{\"value\":\"ok\"}"}"#,
            r#"{"type":"response.function_call_arguments.done","item_id":"fc-p12-f2","output_index":0,"arguments":"{\"value\":\"ok\"}"}"#,
            r#"{"type":"response.output_item.done","output_index":0,"item":{"id":"fc-p12-f2","type":"function_call","call_id":"call-p12-f2","name":"echo","arguments":"{\"value\":\"ok\"}","status":"completed"}}"#,
            r#"{"type":"response.completed","response":{"id":"response-p12-f2-tool","status":"completed","usage":{"input_tokens":3,"output_tokens":5}}}"#,
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
    fn the_decoder_refuses_to_emit_an_illegal_canonical_sequence() {
        // A text delta before any output item opened would put a TextDelta ahead of MessageStart.
        // The frame-level guards already reject this shape, so the value of the state machine is
        // that it holds even if a future frame handler forgets its own guard: no path can queue an
        // event the canonical grammar forbids.
        let body = sse_stream_body(&[
            r#"{"type":"response.created","response":{"id":"response-p12-illegal"}}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg-1","delta":"early"}"#,
        ]);
        assert_eq!(
            decode_sse_events(&body, body.len())
                .err()
                .map(|error| (error.code(), error.scope())),
            Some((GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream))
        );

        // Proving the guard is the state machine and not just the lifecycle check: queueing a
        // terminal ResponseEnd with no open response is refused by the same validator.
        let mut state = CanonicalEventState::default();
        let mut pending = VecDeque::new();
        assert!(
            queue_event(
                &mut state,
                &mut pending,
                CanonicalEvent::ResponseEnd(ResponseEnd {
                    stop_reason: Some("end_turn".to_owned()),
                    stop_sequence: None,
                    extensions: RawExtensions::default(),
                }),
            )
            .is_err()
        );
        assert!(pending.is_empty());
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

        let labels = canonical_event_labels(&events);
        assert_eq!(labels.first(), Some(&"response_start"));
        assert_eq!(labels.get(1), Some(&"message_start"));
        assert!(labels.ends_with(&["text_delta", "message_end", "usage_delta", "response_end",]));
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
    fn web_proxy_isolated_from_non_web_transport_profiles() -> Result<(), Box<dyn Error>> {
        let proxy = UpstreamProxy::try_socks5("socks5://127.0.0.1:19081")?;
        let profiles = P12TransportProfiles::try_new_with_web_proxy(proxy, None, 8191)?;

        assert!(matches!(
            profiles
                .for_mode(ResponsesResponseMode::NonStreaming)
                .proxy(),
            UpstreamProxy::Direct
        ));
        assert!(matches!(
            profiles
                .for_web_mode(ResponsesResponseMode::NonStreaming)
                .proxy(),
            UpstreamProxy::Socks5(_)
        ));
        assert!(matches!(profiles.web_proxy(), UpstreamProxy::Socks5(_)));
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

    #[test]
    fn stage_ledger_capacity_tracks_the_admitted_concurrency_bound() -> Result<(), Box<dyn Error>> {
        // The ledger must outlive one full generation of concurrent requests, otherwise a burst
        // at the admitted concurrency bound evicts records an operator is still inspecting.
        assert_eq!(
            P12AttemptStageStore::MAX_RECORDS,
            2 * usize::try_from(P12_MAX_TOTAL_BINDING_CONCURRENCY)?
        );
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

    struct NeverCancelledGate;

    impl TransparentRetryGate for NeverCancelledGate {
        fn is_cancelled(&self) -> bool {
            false
        }

        fn allows_transparent_retry(&self) -> bool {
            true
        }

        fn cancelled(&self) -> TransparentRetryGateFuture<'_> {
            Box::pin(std::future::pending())
        }
    }

    /// Serves one complete non-streaming JSON response over loopback HTTP, then closes.
    ///
    /// The whole request is drained to its declared `content-length` before the response is
    /// written and the socket closes, so the close is a clean FIN: closing with unread request
    /// bytes would emit an RST that can discard the client's still-buffered response body.
    fn spawn_live_json_peer(
        listener: actix_web::rt::net::TcpListener,
        body: String,
    ) -> actix_web::rt::task::JoinHandle<std::io::Result<()>> {
        actix_web::rt::spawn(async move {
            let (socket, _) = listener.accept().await?;
            read_full_request(&socket).await?;
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            write_all_to_peer(&socket, head.as_bytes()).await?;
            write_all_to_peer(&socket, body.as_bytes()).await?;
            Ok(())
        })
    }

    fn spawn_live_error_peer(
        listener: actix_web::rt::net::TcpListener,
        status: u16,
        retry_after: Option<u64>,
        body: String,
    ) -> actix_web::rt::task::JoinHandle<std::io::Result<()>> {
        actix_web::rt::spawn(async move {
            let (socket, _) = listener.accept().await?;
            read_full_request(&socket).await?;
            let retry_after = retry_after
                .map_or_else(String::new, |seconds| format!("retry-after: {seconds}\r\n"));
            let head = format!(
                "HTTP/1.1 {status} Failure\r\ncontent-type: application/json\r\ncontent-length: {}\r\n{retry_after}connection: close\r\n\r\n",
                body.len()
            );
            write_all_to_peer(&socket, head.as_bytes()).await?;
            write_all_to_peer(&socket, body.as_bytes()).await?;
            Ok(())
        })
    }

    #[actix_web::test]
    async fn codex_usage_limit_is_bounded_and_attributed_over_loopback()
    -> Result<(), Box<dyn Error>> {
        let listener = actix_web::rt::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let peer = spawn_live_error_peer(
            listener,
            400,
            None,
            r#"{"error":{"type":"usage_limit_reached","resets_in_seconds":17,"message":"never retained"}}"#
                .to_owned(),
        );
        let request = live_transport_request(live_admitted_target(port)?)?;
        let pool =
            UpstreamClientPool::new(NonZeroUsize::new(1).ok_or("client pool needs one entry")?);
        let mut response = pool.send(request, &live_progress_test_profile()?).await?;
        let status = response.status();
        let failure = classify_openai_response_failure(&mut response, status).await;
        assert_eq!(
            failure,
            AttemptFailure::RateLimited {
                retry_after: Some(Duration::from_secs(17))
            }
        );
        peer.await??;
        Ok(())
    }

    #[actix_web::test]
    async fn anthropic_rate_limit_is_bounded_and_attributed_over_loopback()
    -> Result<(), Box<dyn Error>> {
        let listener = actix_web::rt::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let peer = spawn_live_error_peer(
            listener,
            429,
            Some(13),
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"not retained"}}"#
                .to_owned(),
        );
        let request = live_transport_request(live_admitted_target(port)?)?;
        let pool =
            UpstreamClientPool::new(NonZeroUsize::new(1).ok_or("client pool needs one entry")?);
        let mut response = pool.send(request, &live_progress_test_profile()?).await?;
        let status = response.status();
        let failure = classify_anthropic_response_failure(&mut response, status).await;
        assert_eq!(
            failure,
            AttemptFailure::RateLimited {
                retry_after: Some(Duration::from_secs(13))
            }
        );
        peer.await??;
        Ok(())
    }

    async fn read_full_request(socket: &actix_web::rt::net::TcpStream) -> std::io::Result<()> {
        let mut request = Vec::new();
        let mut body_start = None;
        loop {
            socket.readable().await?;
            let mut chunk = [0_u8; 1024];
            match socket.try_read(&mut chunk) {
                Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
                Ok(read) => {
                    request.extend_from_slice(&chunk[..read]);
                    if body_start.is_none() {
                        body_start = request
                            .windows(4)
                            .position(|window| window == b"\r\n\r\n")
                            .map(|position| position + 4);
                    }
                    if let Some(body_start) = body_start
                        && request.len()
                            >= body_start + declared_content_length(&request[..body_start])?
                    {
                        return Ok(());
                    }
                    if request.len() > 1_048_576 {
                        return Err(std::io::ErrorKind::InvalidData.into());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
        }
    }

    fn declared_content_length(request: &[u8]) -> std::io::Result<usize> {
        let head = std::str::from_utf8(request)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
        for line in head.split("\r\n") {
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                return value
                    .trim()
                    .parse()
                    .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData));
            }
        }
        Ok(0)
    }

    /// The ledger-only Attempt sink: it reports the stage ledger's own admission result, so
    /// widened-ledger tests observe `record_terminal` directly instead of the durable queue's
    /// admission outcome that the production fanout sink reports.
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
            match &event {
                GatewayEvent::Attempt(attempt) => self.attempts.record_terminal(attempt),
                _ => EventEmission::Enqueued,
            }
        }
    }

    #[actix_web::test]
    async fn widened_graph_serves_after_pre_first_byte_candidate_failover_over_loopback()
    -> Result<(), Box<dyn Error>> {
        let live_listener = actix_web::rt::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let live_port = live_listener.local_addr()?.port();
        let dead_listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let dead_port = dead_listener.local_addr()?.port();
        drop(dead_listener);
        let response_body = r#"{"id":"response-p12-failover","object":"response","status":"completed","error":null,"incomplete_details":null,"output":[{"id":"message-p12-failover","type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"ok","annotations":[],"logprobs":[]}]}]}"#;
        assert!(protocol_openai_responses::decode_upstream_response(response_body).is_ok());
        let peer = spawn_live_json_peer(live_listener, response_body.to_owned());

        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let network = P12WidenedNetwork {
            allowed_scheme: "http",
            host_a: "relay-a.test",
            port_a: dead_port,
            host_b: "relay-b.test",
            port_b: live_port,
            allow_loopback: true,
            endpoint_b_adapter: "openai-compatible.responses",
            endpoint_b_api_format: "openai/responses",
            max_attempts: 4,
        };
        let configuration = p12_widened_configuration(&secret_store, &network)?;
        let config_version_id = configuration.version.id.clone();
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&configuration)?;
        repository.activate_version(&config_version_id)?;
        drop(repository);
        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p12-runtime-test")?,
        )?;
        let snapshot = lifecycle.registry().load();

        let policies = EgressPolicyCompiler::compile(&configuration)?;
        let pools = CredentialPoolCompiler::new(&secret_store).compile(&configuration)?;
        let mut endpoints = std::collections::BTreeMap::new();
        for configured in &configuration.endpoints {
            endpoints.insert(
                configured.id.clone(),
                EndpointRuntime {
                    adapter: EndpointAdapter::OpenAiResponses(OpenAiResponsesEndpoint::try_new(
                        &configured.base_url,
                        &configured.inference_path,
                    )?),
                    policy: policies
                        .policy_for_upstream(&configured.upstream_id)
                        .cloned()
                        .ok_or("missing compiled egress policy")?,
                    resolver: Arc::new(LoopbackResolver),
                    transports: Arc::new(P12TransportProfiles::try_new()?),
                    web_statsig: OnceLock::new(),
                },
            );
        }
        let scheduler = Arc::new(RouteCredentialScheduler::new(
            Arc::clone(&snapshot),
            Arc::new(pools),
        ));
        let orchestrator = AttemptOrchestrator::new(
            scheduler,
            Arc::new(gateway_router::RuntimeHealthRegistry::new()),
        );
        let attempt_stages = Arc::new(P12AttemptStageStore::new());
        let sink = P12AttemptEventSink::new(Arc::clone(&attempt_stages));
        let request_id = RequestId::try_new("p12-failover-request")?;
        // Keep this transport/failover test independent of D3 capability admission. The richer
        // fixture declares Tools, Reasoning, and streaming and therefore correctly requires an
        // Endpoint capability ledger; this graph intentionally has no such ledger yet.
        let decoded = decode_request(
            r#"{"model":"gateway-model","input":"fail over safely","stream":false}"#,
        )?;
        let driver = EndpointAttemptDriver {
            request_id: request_id.clone(),
            request: decoded.request,
            client_protocol: ProtocolFormat::OpenAiResponses,
            native_payload: None,
            usage_projection: P12ResponseUsageProjection::OpenAiResponses,
            mode: ResponsesResponseMode::NonStreaming,
            client_transport: ResponsesClientTransport::Http,
            endpoints: Arc::new(endpoints),
            compatible_endpoints: Arc::new(BTreeMap::new()),
            client_pool: Arc::new(UpstreamClientPool::new(
                NonZeroUsize::new(4).ok_or("client pool needs capacity")?,
            )),
            attempt_stages: Arc::clone(&attempt_stages),
            allow_compatibility_retry: true,
            allow_egress_refresh: true,
            channel_pin_observation: None,
        };
        let route_id = RouteId::try_new("p12-widened-route-primary")?;
        for candidate in snapshot
            .route(&route_id)
            .ok_or("missing test route")?
            .candidates()
        {
            assert!(
                driver.project_candidate(candidate).is_ok(),
                "candidate projection failed: {:?}",
                driver.project_candidate(candidate).err()
            );
        }
        let started = orchestrator
            .start_with_event_sink(&request_id, &route_id, &driver, &NeverCancelledGate, &sink)
            .await
            .map_err(|error| std::io::Error::other(format!("failover start failed: {error}")))?;
        let (mut source, _selection) = started.into_parts();
        let mut events = Vec::new();
        while let Some(event) = source
            .next_event()
            .await
            .map_err(|error| std::io::Error::other(format!("event source failed: {error}")))?
        {
            events.push(event);
        }
        let labels = canonical_event_labels(&events);
        assert!(labels.starts_with(&["response_start"]));
        assert!(labels.contains(&"text_delta"));
        assert!(labels.ends_with(&["response_end"]));

        // A Connection failure cools the whole Endpoint (`CooldownScope::Endpoint`), so the
        // orchestrator fails over to candidate B on the second attempt instead of first
        // exhausting endpoint A's remaining weighted Credentials; the widened ledger still
        // records one terminal per attempt, with only the newest carrying the stage.
        let rows = attempt_stages
            .list_request_attempts(&request_id)
            .map_err(|_| std::io::Error::other("attempt stage projection unavailable"))?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].outcome(), "failed");
        assert!(rows[0].stage().is_none());
        assert_eq!(rows[1].outcome(), "succeeded");
        assert_eq!(
            rows[1].stage(),
            Some(ManagementRequestAttemptStage::Decoder)
        );
        peer.await??;
        Ok(())
    }

    #[test]
    fn stored_response_lineage_uses_the_selected_candidate_and_live_lease()
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
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p13-09b-runtime-lineage-test")?,
        )?;
        let snapshot = lifecycle.registry().load();
        let pools = CredentialPoolCompiler::new(&secret_store).compile(&configuration)?;
        let scheduler = RouteCredentialScheduler::new(snapshot, Arc::new(pools));
        let selection = scheduler.select_and_lease(&RouteId::try_new("p12-runtime-route")?)?;
        let lineage = super::stored_response_execution_lineage(
            scheduler.snapshot_version(),
            &RouteId::try_new("p12-runtime-route")?,
            selection.candidate(),
            selection.lease(),
        )?;
        assert_eq!(lineage.snapshot_version().as_str(), "p12-runtime-config");
        assert_eq!(lineage.provider_id().as_str(), "p12-runtime-upstream");
        assert_eq!(lineage.upstream_id().as_str(), "p12-runtime-upstream");
        assert_eq!(
            lineage.channel_id().as_str(),
            P12_SINGLETON_TEST_ENDPOINT_ID
        );
        assert_eq!(lineage.route_id().as_str(), "p12-runtime-route");
        assert_eq!(
            lineage.route_candidate_id().as_str(),
            "p12-runtime-candidate"
        );
        assert_eq!(lineage.credential_id().as_str(), "p12-runtime-credential");
        assert_eq!(lineage.credential_revision(), 1);
        Ok(())
    }

    #[actix_web::test]
    async fn routed_executor_rejects_ambiguous_provider_scope_before_driver_start()
    -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let draft_configuration =
            p12_widened_configuration(&secret_store, &p12_production_network())?;
        let config_version_id = draft_configuration.version.id.clone();
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&draft_configuration)?;
        repository.activate_version(&config_version_id)?;
        let configuration = repository
            .load_active_configuration()?
            .ok_or("active configuration missing")?;
        drop(repository);
        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p13-07c-runtime-scope-test")?,
        )?;
        let attempt_stages = Arc::new(P12AttemptStageStore::new());
        let event_sink: Arc<dyn GatewayEventSink> =
            Arc::new(P12AttemptEventSink::new(Arc::clone(&attempt_stages)));
        let runtime_health = Arc::new(RuntimeHealthRegistry::new());
        let runtime_quota = Arc::new(RuntimeQuotaRegistry::new());
        let (executor, _account_pools, scheduler) = P12RoutedResponsesExecutor::try_new(
            &database,
            &configuration,
            &secret_store,
            Arc::clone(lifecycle.registry()),
            Arc::clone(&attempt_stages),
            event_sink,
            runtime_health,
            runtime_quota,
            None,
            None,
            8191,
            None,
        )?;
        let decoded = protocol_openai_responses::decode_request(
            r#"{"model":"primary","input":"must not reach a provider","stream":false}"#,
        )?;
        let request_id = RequestId::try_new("p13-07c-ambiguous-runtime")?;
        let execution = ResponsesExecution::new(
            RequestContext::new(request_id),
            decoded.request,
            Some(RouteId::try_new("p12-widened-route-primary")?),
            ResponsesResponseMode::NonStreaming,
            Arc::new(NeverCancelledGate),
        );
        let error = executor
            .execute_routed(execution)
            .await
            .err()
            .ok_or("ambiguous provider route unexpectedly started")?;
        assert_eq!(error.code(), GatewayErrorCode::CredentialUnavailable);
        assert_eq!(error.scope(), ErrorScope::Credential);
        assert!(
            scheduler
                .route(&RouteId::try_new("p12-widened-route-primary")?)
                .is_some()
        );
        assert_eq!(
            attempt_stages
                .list_request_attempts(&RequestId::try_new("p13-07c-ambiguous-runtime")?)
                .map_err(|_| "attempt ledger unexpectedly unavailable")?,
            Vec::new()
        );
        Ok(())
    }

    /// A Candidate whose declared format disagrees with its Endpoint's bound adapter must fail
    /// the attempt before a Secret is read, a URL is composed, or a socket is opened.
    ///
    /// The Snapshot here is built directly rather than compiled, which is the only way the
    /// disagreement can exist at all: the composition and the compiler both reject it earlier.
    /// The Endpoint target points at a closed loopback port, so a dial would classify as the
    /// retryable `AttemptFailure::Connection`; a `NonRetryable` result therefore proves no dial
    /// was attempted, and the ledger stage stays at `RequestConversion`.
    #[actix_web::test]
    async fn a_candidate_format_that_disagrees_with_its_bound_adapter_fails_the_attempt()
    -> Result<(), Box<dyn Error>> {
        let dead_listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let dead_port = dead_listener.local_addr()?.port();
        drop(dead_listener);

        let endpoint_id = EndpointId::try_new("p12-format-guard-endpoint")?;
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("p12-format-guard-policy")?,
            name: "P12 format guard policy".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Http]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("relay.test")?]),
            allowed_ports: BTreeSet::from([dead_port]),
            allowed_cidrs: BTreeSet::from([EgressCidr::try_new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                32,
            )?]),
            redirect_policy: RedirectPolicy::Deny,
        })?;
        let mut endpoints = std::collections::BTreeMap::new();
        endpoints.insert(
            endpoint_id.clone(),
            EndpointRuntime {
                adapter: EndpointAdapter::OpenAiResponses(OpenAiResponsesEndpoint::try_new(
                    &format!("http://relay.test:{dead_port}/v1"),
                    "/responses",
                )?),
                policy,
                resolver: Arc::new(LoopbackResolver),
                transports: Arc::new(P12TransportProfiles::try_new()?),
                web_statsig: OnceLock::new(),
            },
        );

        let candidate = SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new("p12-format-guard-candidate")?,
            endpoint_id: endpoint_id.clone(),
            upstream_id: UpstreamId::try_new("p12-format-guard-upstream")?,
            endpoint_api_format: "anthropic/messages".to_owned(),
            upstream_model: "upstream-model-guard".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: gateway_catalog::CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::AllowedUnlisted,
            active_binding_count: 1,
        });

        let pool = EndpointCredentialPool::try_new(
            endpoint_id,
            [EndpointCredentialInput {
                credential_id: CredentialId::try_new("p12-format-guard-credential")?,
                credential_kind: "bearer".to_owned(),
                credential_revision: 1,
                priority: 0,
                weight: 1,
                concurrency: 1,
                expires_at_ms: None,
                secret: CredentialSecret::try_new(b"p12-format-guard-secret".to_vec())?,
            }],
        )?;
        let lease = pool.try_lease().ok_or("credential lease unavailable")?;

        let attempt_stages = Arc::new(P12AttemptStageStore::new());
        let request_id = RequestId::try_new("p12-format-guard-request")?;
        let decoded = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;
        let driver = EndpointAttemptDriver {
            request_id: request_id.clone(),
            request: decoded.request,
            client_protocol: ProtocolFormat::OpenAiResponses,
            native_payload: None,
            usage_projection: P12ResponseUsageProjection::OpenAiResponses,
            mode: ResponsesResponseMode::NonStreaming,
            client_transport: ResponsesClientTransport::Http,
            endpoints: Arc::new(endpoints),
            compatible_endpoints: Arc::new(BTreeMap::new()),
            client_pool: Arc::new(UpstreamClientPool::new(
                NonZeroUsize::new(4).ok_or("client pool needs capacity")?,
            )),
            attempt_stages: Arc::clone(&attempt_stages),
            allow_compatibility_retry: true,
            allow_egress_refresh: true,
            channel_pin_observation: None,
        };

        let result = driver
            .start(&candidate, &lease, Duration::from_millis(500))
            .await;

        assert!(matches!(result, Err(AttemptFailure::NonRetryable(_))));
        assert_eq!(
            attempt_stages.recorded_stage(&request_id),
            Some(ManagementRequestAttemptStage::RequestConversion)
        );

        let websocket_candidate = SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new("p13-websocket-capability-guard")?,
            endpoint_id: candidate.endpoint_id().clone(),
            upstream_id: candidate.upstream_id().clone(),
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: "upstream-model-guard".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: CapabilitySet::try_new([SemanticCapability::Streaming])?,
            catalog_admission: SnapshotCatalogAdmission::AllowedUnlisted,
            active_binding_count: 1,
        });
        let websocket_driver = EndpointAttemptDriver {
            client_transport: ResponsesClientTransport::WebSocket,
            ..driver
        };
        assert_eq!(
            websocket_driver.project_candidate(&websocket_candidate),
            Err(gateway_router::ProtocolTransformRejection::ResponsesWebSocketUnsupported)
        );
        Ok(())
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
    async fn a_live_anthropic_ping_only_stream_is_cut_by_the_progress_deadline()
    -> Result<(), Box<dyn Error>> {
        // The Anthropic shell claims the OpenAI shell's liveness behaviour; prove it rather than
        // trusting the doc comment. `ping` is Anthropic's keepalive: it keeps the transport's
        // byte-idle timer fresh forever while producing no generation.
        let listener = actix_web::rt::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let prelude = sse_stream_body(&[
            r#"{"type":"message_start","message":{"id":"msg-live","type":"message","role":"assistant","content":[],"usage":{"input_tokens":3}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"ok"}}"#,
        ]);
        let ping = sse_stream_body(&[r#"{"type":"ping"}"#]);
        let server = spawn_live_sse_peer(
            listener,
            prelude,
            ping,
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
        let mut source = AnthropicSseEventSource::begin_with_progress_deadline(
            response,
            P12ResponseUsageProjection::AnthropicMessages,
            Duration::from_millis(200),
        )
        .await
        .map_err(|_| std::io::Error::other("live Anthropic SSE bootstrap failed"))?;

        let started = Instant::now();
        let error = loop {
            match source.next_event().await {
                Ok(Some(_)) => {}
                Ok(None) => {
                    return Err("ping-only stream ended without the progress failure".into());
                }
                Err(error) => break error,
            }
        };
        assert_eq!(
            (error.code(), error.scope()),
            (GatewayErrorCode::ProviderTransient, ErrorScope::Provider)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        server.abort();
        Ok(())
    }

    #[actix_web::test]
    async fn an_anthropic_stream_that_never_opens_with_message_start_fails_bootstrap()
    -> Result<(), Box<dyn Error>> {
        let listener = actix_web::rt::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        // A body that opens with a content block instead of `message_start` can never produce a
        // leading ResponseStart, so bootstrap must refuse it before any byte reaches the client.
        let prelude = sse_stream_body(&[
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        ]);
        let server = spawn_live_sse_peer(
            listener,
            prelude,
            String::new(),
            0,
            Duration::ZERO,
            String::new(),
        );

        let response = UpstreamClientPool::new(NonZeroUsize::new(1).ok_or("live pool size")?)
            .send(
                live_transport_request(live_admitted_target(port)?)?,
                &live_progress_test_profile()?,
            )
            .await?;
        assert!(
            AnthropicSseEventSource::begin(response, P12ResponseUsageProjection::AnthropicMessages)
                .await
                .is_err()
        );
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
        // Thirty reasoning-progress frames 40 ms apart accumulate 1.2 s of upstream wait -- past
        // one deadline -- so each frame must restart the window for the stream to reach its
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
        let labels = canonical_event_labels(&events);
        assert_eq!(labels.first(), Some(&"response_start"));
        assert_eq!(labels.get(1), Some(&"message_start"));
        assert_eq!(
            labels
                .iter()
                .filter(|label| **label == "reasoning_delta")
                .count(),
            30
        );
        assert!(labels.ends_with(&["text_delta", "message_end", "usage_delta", "response_end",]));
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

    #[derive(Clone, Copy, Debug)]
    enum P12F2Channel {
        OpenAi,
        Claude,
        Grok,
        Kiro,
    }

    impl P12F2Channel {
        const fn label(self) -> &'static str {
            match self {
                Self::OpenAi => "openai",
                Self::Claude => "claude",
                Self::Grok => "grok",
                Self::Kiro => "kiro",
            }
        }

        const fn target(self, source: ProtocolFormat) -> (&'static str, ProtocolFormat, bool) {
            match self {
                Self::OpenAi if matches!(source, ProtocolFormat::OpenAiChatCompletions) => (
                    "openai-compatible.chat-completions",
                    ProtocolFormat::OpenAiChatCompletions,
                    false,
                ),
                Self::OpenAi => (
                    "openai-compatible.responses",
                    ProtocolFormat::OpenAiResponses,
                    false,
                ),
                Self::Claude => (
                    "anthropic-compatible.messages",
                    ProtocolFormat::AnthropicMessages,
                    false,
                ),
                Self::Grok => (
                    "grok.build.responses",
                    ProtocolFormat::OpenAiResponses,
                    true,
                ),
                Self::Kiro => ("kiro.messages", ProtocolFormat::AnthropicMessages, true),
            }
        }
    }

    struct P12F2MatrixExecutor {
        channel: P12F2Channel,
        events: Vec<CanonicalEvent>,
        attempts: Arc<AtomicU64>,
    }

    impl ResponsesExecutor for P12F2MatrixExecutor {
        fn execute(
            &self,
            _context: RequestContext,
            _request: CanonicalRequest,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            Box::pin(async {
                Err(GatewayError::new(
                    GatewayErrorCode::InternalError,
                    ErrorScope::Internal,
                ))
            })
        }

        fn execute_routed(
            &self,
            execution: ResponsesExecution,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            Box::pin(async move {
                let source = execution.client_protocol();
                let (adapter_id, target, canonical_only) = self.channel.target(source);
                let transform_mode = if source == target {
                    SnapshotTransformMode::Canonical
                } else {
                    SnapshotTransformMode::LosslessBridge
                };
                // Grok and Kiro own typed Canonical request builders and deliberately reject the
                // generic bridge mode before any Credential lease or upstream Attempt.
                if canonical_only && transform_mode != SnapshotTransformMode::Canonical {
                    return Err(GatewayError::new(
                        GatewayErrorCode::UpstreamProtocolError,
                        ErrorScope::Provider,
                    ));
                }
                let capabilities = p12_adapter_capabilities(adapter_id).map_err(|_| {
                    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
                })?;
                let projected = project_registered_protocol_request(ProtocolTransformInput {
                    source,
                    target,
                    mode: transform_mode,
                    native_payload: execution
                        .native_payload()
                        .map_or(NativePayloadAvailability::Unavailable, |_| {
                            NativePayloadAvailability::Exact
                        }),
                    request: execution.request(),
                    streaming: execution.mode() == ResponsesResponseMode::Streaming,
                    requires_json_schema: false,
                    requires_parallel_tools: false,
                    target_capabilities: &capabilities,
                })
                .map_err(|_| {
                    GatewayError::new(
                        GatewayErrorCode::UpstreamProtocolError,
                        ErrorScope::Provider,
                    )
                })?;
                let ProjectedProtocolRequest::Canonical(projected) = projected else {
                    return Err(GatewayError::new(
                        GatewayErrorCode::InternalError,
                        ErrorScope::Internal,
                    ));
                };
                if projected.messages.is_empty() || projected.tools.is_empty() {
                    return Err(GatewayError::new(
                        GatewayErrorCode::InternalError,
                        ErrorScope::Internal,
                    ));
                }
                self.attempts.fetch_add(1, Ordering::AcqRel);
                Ok(Box::new(FiniteEventSource::new(self.events.clone()))
                    as Box<dyn ResponsesEventSource>)
            })
        }
    }

    fn p12_f2_http_state(
        channel: P12F2Channel,
        events: Vec<CanonicalEvent>,
        attempts: Arc<AtomicU64>,
    ) -> Result<ResponsesHttpState, Box<dyn Error>> {
        let client_key = InMemoryClientKey::try_new(
            "p12-f2-http-test-key",
            ClientKeyId::try_new("p12-f2-http-test-client")?,
            true,
        )?;
        let authenticator: Arc<dyn ClientKeyAuthenticator> =
            Arc::new(InMemoryClientKeyAuthenticator::try_new([client_key])?);
        Ok(ResponsesHttpState::new(
            Arc::new(P12F2MatrixExecutor {
                channel,
                events,
                attempts,
            }),
            authenticator,
            default_stream_capacity()?,
        ))
    }

    #[actix_web::test]
    async fn three_protocols_by_four_channels_obey_the_f2_loopback_matrix()
    -> Result<(), Box<dyn Error>> {
        let text_events = decode_sse_events(&p12_f2_text_body(), 7)?;
        let tool_events = decode_sse_events(&p12_f2_tool_body(), 7)?;
        CanonicalResponse::try_new(text_events.clone())?;
        CanonicalResponse::try_new(tool_events.clone())?;
        let channels = [
            P12F2Channel::OpenAi,
            P12F2Channel::Claude,
            P12F2Channel::Grok,
            P12F2Channel::Kiro,
        ];
        let protocols = [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
        ];

        for channel in channels {
            for protocol in protocols {
                let expected_supported = matches!(
                    (channel, protocol),
                    (P12F2Channel::OpenAi, _)
                        | (
                            P12F2Channel::Claude | P12F2Channel::Grok,
                            ProtocolFormat::OpenAiResponses
                        )
                        | (
                            P12F2Channel::Claude | P12F2Channel::Kiro,
                            ProtocolFormat::AnthropicMessages
                        )
                );
                let semantics = if expected_supported {
                    vec![("text", text_events.clone()), ("tool", tool_events.clone())]
                } else {
                    vec![("tool", tool_events.clone())]
                };
                for (semantic, events) in semantics {
                    for streaming in [false, true] {
                        let attempts = Arc::new(AtomicU64::new(0));
                        let app = actix_test::init_service(
                            App::new()
                                .app_data(web::Data::new(p12_f2_http_state(
                                    channel,
                                    events.clone(),
                                    Arc::clone(&attempts),
                                )?))
                                .configure(configure),
                        )
                        .await;
                        let stream = if streaming { "true" } else { "false" };
                        let chat_stream_options = if streaming {
                            r#","stream_options":{"include_usage":true}"#
                        } else {
                            ""
                        };
                        let chat_tool_choice = if semantic == "tool" {
                            r#","tool_choice":"required""#
                        } else {
                            ""
                        };
                        let responses_tool_choice = chat_tool_choice;
                        let messages_tool_choice = if semantic == "tool" {
                            r#","tool_choice":{"type":"any"}"#
                        } else {
                            ""
                        };
                        let (uri, payload) = match protocol {
                            ProtocolFormat::OpenAiChatCompletions => (
                                "/v1/chat/completions",
                                format!(
                                    r#"{{"model":"p12-f2-model","max_tokens":32,"messages":[{{"role":"user","content":"ok"}}],"tools":[{{"type":"function","function":{{"name":"echo","parameters":{{"type":"object"}}}}}}],"stream":{stream}{chat_stream_options}{chat_tool_choice}}}"#,
                                ),
                            ),
                            ProtocolFormat::OpenAiResponses => (
                                "/v1/responses",
                                format!(
                                    r#"{{"model":"p12-f2-model","input":"ok","max_output_tokens":32,"tools":[{{"type":"function","name":"echo","parameters":{{"type":"object"}}}}],"stream":{stream}{responses_tool_choice}}}"#,
                                ),
                            ),
                            ProtocolFormat::AnthropicMessages => (
                                "/v1/messages",
                                format!(
                                    r#"{{"model":"p12-f2-model","max_tokens":32,"messages":[{{"role":"user","content":"ok"}}],"tools":[{{"name":"echo","input_schema":{{"type":"object"}}}}],"stream":{stream}{messages_tool_choice}}}"#,
                                ),
                            ),
                        };
                        let request = actix_test::TestRequest::post()
                            .uri(uri)
                            .insert_header((header::AUTHORIZATION, "Bearer p12-f2-http-test-key"))
                            .set_payload(payload)
                            .to_request();
                        let response = actix_test::call_service(&app, request).await;
                        let status = response.status();
                        let body =
                            String::from_utf8(actix_test::read_body(response).await.to_vec())?;

                        if expected_supported {
                            assert_eq!(
                                status,
                                StatusCode::OK,
                                "{channel:?}/{protocol:?}/{semantic}: attempts={} {body}",
                                attempts.load(Ordering::Acquire)
                            );
                            assert_eq!(attempts.load(Ordering::Acquire), 1);
                            if semantic == "text" {
                                assert!(body.contains("visible"));
                            } else {
                                assert!(body.contains("echo"));
                            }
                            assert!(body.contains("usage"));
                            if streaming {
                                assert!(body.contains("data:"));
                            }
                        } else {
                            assert_ne!(status, StatusCode::OK, "{channel:?}/{protocol:?}");
                            assert_eq!(attempts.load(Ordering::Acquire), 0);
                            assert!(
                                body.contains("upstream protocol was invalid"),
                                "{channel:?}/{protocol:?}: {body}"
                            );
                            assert!(!body.contains(channel.label()));
                        }
                    }
                }
            }
        }
        Ok(())
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
    fn deployment_compiler_profiles_only_stored_endpoints() -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let configuration = p12_widened_configuration(&secret_store, &p12_production_network())?;

        let empty = deployment_route_compiler(&database)?;
        assert!(empty.compile(&configuration).is_err());

        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&configuration)?;
        drop(repository);
        let stored = deployment_route_compiler(&database)?;
        let compiled = stored.compile(&configuration)?;
        for route in compiled.routes() {
            for candidate in route.candidates() {
                let capabilities = candidate.effective_capabilities();
                assert!(capabilities.supports(SemanticCapability::Tools));
                assert!(capabilities.supports(SemanticCapability::ParallelTools));
                assert!(capabilities.supports(SemanticCapability::Reasoning));
                assert!(capabilities.supports(SemanticCapability::JsonSchema));
                assert!(capabilities.supports(SemanticCapability::Streaming));
                assert!(capabilities.supports(SemanticCapability::ResponsesWebSocket));
                assert!(!capabilities.supports(SemanticCapability::Vision));
            }
        }
        Ok(())
    }

    #[test]
    fn production_adapter_capability_ledger_is_conservative_and_fail_closed()
    -> Result<(), Box<dyn Error>> {
        use SemanticCapability::{
            JsonSchema, ParallelTools, Reasoning, ResponseCompaction, ResponsesWebSocket,
            StoredResponses, Streaming, Tools, Vision,
        };

        let cases = [
            (
                "openai-compatible.chat-completions",
                vec![
                    Tools,
                    ParallelTools,
                    JsonSchema,
                    Streaming,
                    ResponsesWebSocket,
                ],
            ),
            (
                "openai-compatible.responses",
                vec![
                    Tools,
                    ParallelTools,
                    Reasoning,
                    JsonSchema,
                    Streaming,
                    ResponsesWebSocket,
                ],
            ),
            (
                "anthropic-compatible.messages",
                vec![
                    Tools,
                    ParallelTools,
                    Reasoning,
                    JsonSchema,
                    Streaming,
                    ResponsesWebSocket,
                ],
            ),
            (
                "grok.build.responses",
                vec![
                    Tools,
                    Reasoning,
                    JsonSchema,
                    Streaming,
                    ResponsesWebSocket,
                    StoredResponses,
                    ResponseCompaction,
                ],
            ),
            (
                "grok.console.responses",
                vec![Tools, Reasoning, JsonSchema, Streaming, ResponsesWebSocket],
            ),
            (
                "grok.official.responses",
                vec![
                    Tools,
                    ParallelTools,
                    Reasoning,
                    JsonSchema,
                    Streaming,
                    ResponsesWebSocket,
                ],
            ),
            (
                "grok.web.responses",
                vec![Streaming, ResponsesWebSocket, StoredResponses],
            ),
            (
                "kiro.messages",
                vec![Tools, Reasoning, JsonSchema, Streaming, ResponsesWebSocket],
            ),
        ];
        for (adapter, supported) in cases {
            let capabilities = p12_adapter_capabilities(adapter)?;
            for capability in [
                Tools,
                ParallelTools,
                Reasoning,
                JsonSchema,
                Vision,
                Streaming,
                ResponsesWebSocket,
                StoredResponses,
                ResponseCompaction,
            ] {
                assert_eq!(
                    capabilities.supports(capability),
                    supported.contains(&capability),
                    "unexpected {capability:?} ledger value for {adapter}"
                );
            }
        }

        assert!(matches!(
            p12_adapter_capabilities("unknown.responses"),
            Err(RuntimeCompositionError::Unavailable)
        ));
        Ok(())
    }

    #[test]
    fn production_registry_composes_only_locally_passed_channel_pairs() -> Result<(), Box<dyn Error>>
    {
        let registry = p12_api_format_adapter_registry()?;
        for (format, adapter) in [
            (
                ApiFormat::OpenAiChatCompletions,
                "openai-compatible.chat-completions",
            ),
            (ApiFormat::OpenAiResponses, "openai-compatible.responses"),
            (
                ApiFormat::AnthropicMessages,
                "anthropic-compatible.messages",
            ),
            (ApiFormat::OpenAiResponses, "grok.build.responses"),
            (ApiFormat::OpenAiResponses, "grok.console.responses"),
            (ApiFormat::OpenAiResponses, "grok.official.responses"),
            (ApiFormat::OpenAiResponses, "grok.web.responses"),
            (ApiFormat::AnthropicMessages, "kiro.messages"),
        ] {
            assert!(p12_adapter_id_serves(format, adapter));
            assert!(registry.adapter(adapter).is_some());
            assert!(!p12_adapter_capabilities(adapter)?.eq(&CapabilitySet::empty()));
        }

        Ok(())
    }

    #[test]
    fn one_endpoint_identity_cannot_change_capability_profile_across_versions()
    -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let first = p12_widened_configuration(&secret_store, &p12_production_network())?;
        let mut second_network = p12_production_network();
        second_network.endpoint_b_adapter = "anthropic-compatible.messages";
        second_network.endpoint_b_api_format = "anthropic/messages";
        let mut second = p12_widened_configuration(&secret_store, &second_network)?;
        second.version.id = ConfigVersionId::try_new("p12-widened-config-second")?;
        second.version.description = "conflicting Endpoint capability profile".to_owned();

        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&first)?;
        repository.write_configuration(&second)?;
        drop(repository);

        assert!(matches!(
            deployment_route_compiler(&database),
            Err(RuntimeCompositionError::Unavailable)
        ));
        Ok(())
    }

    #[test]
    fn p12_attempt_stage_projection_is_terminal_and_value_free() -> Result<(), Box<dyn Error>> {
        let attempts = std::sync::Arc::new(P12AttemptStageStore::new());
        let request_id = RequestId::try_new("p12-stage-request")?;
        attempts.record_stage(&request_id, ManagementRequestAttemptStage::Decoder);
        let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(2, 1)?)?;
        let sink =
            P12FanoutEventSink::new(std::sync::Arc::clone(&attempts), std::sync::Arc::new(queue));
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
            sink.try_emit(GatewayEvent::Attempt(event.clone())),
            EventEmission::Enqueued
        );
        assert_eq!(
            attempts.stage_view(&request_id),
            Some(ManagementRequestAttemptStage::Decoder)
        );
        assert!(matches!(
            receiver.try_recv(),
            Some(GatewayEvent::Attempt(_))
        ));
        // A duplicate terminal for one Request poisons the ledger; the stage is withheld while
        // the durable queue still accepts the event for the authoritative timeline.
        assert_eq!(
            sink.try_emit(GatewayEvent::Attempt(event)),
            EventEmission::Enqueued
        );
        assert_eq!(attempts.stage_view(&request_id), None);
        Ok(())
    }

    #[test]
    fn p12_attempt_stage_projection_records_every_attempt_of_a_retried_request()
    -> Result<(), Box<dyn Error>> {
        let attempts = std::sync::Arc::new(P12AttemptStageStore::new());
        let sink = P12AttemptEventSink::new(std::sync::Arc::clone(&attempts));
        let stage_event = |request_id: &RequestId,
                           attempt_number: u64,
                           outcome: AttemptOutcome|
         -> Result<AttemptEvent, Box<dyn Error>> {
            Ok(AttemptEvent::new(
                request_id.clone(),
                attempt_number,
                RouteId::try_new("p12-stage-retry-route")?,
                RouteCandidateId::try_new("p12-stage-retry-candidate")?,
                CredentialId::try_new("p12-stage-retry-credential")?,
                EndpointId::try_new("p12-stage-retry-endpoint")?,
                UpstreamId::try_new("p12-stage-retry-upstream")?,
                "p12-stage-retry-model".to_owned(),
                1,
                2,
                outcome,
                AttemptRetryDecision::Completed,
            ))
        };

        let retried = RequestId::try_new("p12-stage-retry")?;
        attempts.record_stage(&retried, ManagementRequestAttemptStage::HttpTransport);
        let failed = AttemptOutcome::Failed(GatewayError::new(
            GatewayErrorCode::UpstreamProtocolError,
            ErrorScope::Stream,
        ));
        assert_eq!(
            sink.try_emit(GatewayEvent::Attempt(stage_event(&retried, 1, failed)?)),
            EventEmission::Enqueued
        );
        assert_eq!(
            sink.try_emit(GatewayEvent::Attempt(stage_event(
                &retried,
                2,
                AttemptOutcome::Succeeded
            )?)),
            EventEmission::Enqueued
        );
        let rows = attempts
            .list_request_attempts(&retried)
            .map_err(|_| std::io::Error::other("attempt stage projection unavailable"))?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].outcome(), "failed");
        assert!(rows[0].stage().is_none());
        assert_eq!(rows[1].outcome(), "succeeded");
        assert_eq!(
            rows[1].stage(),
            Some(ManagementRequestAttemptStage::HttpTransport)
        );

        let saturated = RequestId::try_new("p12-stage-retry-saturated")?;
        attempts.record_stage(&saturated, ManagementRequestAttemptStage::HttpTransport);
        for attempt_number in 1..=P12_MAX_ROUTE_ATTEMPTS as u64 {
            assert_eq!(
                sink.try_emit(GatewayEvent::Attempt(stage_event(
                    &saturated,
                    attempt_number,
                    AttemptOutcome::Succeeded
                )?)),
                EventEmission::Enqueued
            );
        }
        assert_eq!(
            sink.try_emit(GatewayEvent::Attempt(stage_event(
                &saturated,
                6,
                AttemptOutcome::Succeeded
            )?)),
            EventEmission::RequiredQueueFull
        );
        assert_eq!(
            attempts.list_request_attempts(&saturated),
            Err(ManagementRuntimeError::Unavailable)
        );
        Ok(())
    }

    #[test]
    fn p12_attempt_stage_contention_withholds_the_stage_projection() -> Result<(), Box<dyn Error>> {
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

        assert_eq!(attempts.stage_view(&request_id), None);
        Ok(())
    }

    #[test]
    fn p12_attempt_stage_capacity_withholds_new_stage_projections() -> Result<(), Box<dyn Error>> {
        let attempts = P12AttemptStageStore::new();
        for index in 0..P12AttemptStageStore::MAX_RECORDS {
            let request_id = RequestId::try_new(format!("p12-stage-capacity-{index}"))?;
            attempts.record_stage(&request_id, ManagementRequestAttemptStage::HttpTransport);
        }
        let overflow = RequestId::try_new("p12-stage-capacity-overflow")?;
        attempts.record_stage(&overflow, ManagementRequestAttemptStage::HttpTransport);

        assert_eq!(attempts.stage_view(&overflow), None);
        Ok(())
    }

    struct P12ObservedAttemptExecutor {
        events: Vec<CanonicalEvent>,
        event_sink: Arc<dyn GatewayEventSink>,
        attempt_stages: Arc<P12AttemptStageStore>,
        route_id: RouteId,
        candidate_id: RouteCandidateId,
        credential_id: CredentialId,
        endpoint_id: EndpointId,
        upstream_id: UpstreamId,
    }

    impl ResponsesExecutor for P12ObservedAttemptExecutor {
        fn execute(
            &self,
            _context: RequestContext,
            _request: CanonicalRequest,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            Box::pin(async {
                Err(GatewayError::new(
                    GatewayErrorCode::RouteNotFound,
                    ErrorScope::Model,
                ))
            })
        }

        fn execute_routed(
            &self,
            execution: ResponsesExecution,
        ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
            let request_id = execution.context().request_id().clone();
            Box::pin(async move {
                self.attempt_stages
                    .record_stage(&request_id, ManagementRequestAttemptStage::HttpTransport);
                let _emission = self
                    .event_sink
                    .try_emit(GatewayEvent::Attempt(AttemptEvent::new(
                        request_id,
                        1,
                        self.route_id.clone(),
                        self.candidate_id.clone(),
                        self.credential_id.clone(),
                        self.endpoint_id.clone(),
                        self.upstream_id.clone(),
                        "p12-obs-upstream-model".to_owned(),
                        10,
                        25,
                        AttemptOutcome::Succeeded,
                        AttemptRetryDecision::Completed,
                    )));
                Ok(Box::new(FiniteEventSource::new(self.events.clone()))
                    as Box<dyn ResponsesEventSource>)
            })
        }
    }

    fn p12_observed_canonical_events() -> Result<Vec<CanonicalEvent>, Box<dyn Error>> {
        Ok(vec![
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id: ResponseId::try_new("p12-obs-response")?,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::TextDelta(TextDelta {
                text: "observed".to_owned(),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageEnd(MessageEnd::default()),
            CanonicalEvent::UsageDelta(UsageDelta {
                usage: Usage {
                    input_tokens: Some(3),
                    output_tokens: Some(5),
                    ..Usage::default()
                },
                is_final: true,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::ResponseEnd(ResponseEnd {
                stop_reason: Some("end_turn".to_owned()),
                stop_sequence: None,
                extensions: RawExtensions::default(),
            }),
        ])
    }

    #[actix_web::test]
    async fn p12_serve_composition_persists_request_attempt_usage_for_management_reads()
    -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let attempt_stages = Arc::new(P12AttemptStageStore::new());
        let (event_queue, event_receiver) =
            BoundedEventQueue::try_new(EventQueueConfig::try_new(8, 1)?)?;
        let event_queue = Arc::new(event_queue);
        let telemetry_metrics = Arc::new(PrometheusMetrics::default());
        let writer = AsyncSqliteEventWriter::new(
            &database,
            event_receiver,
            EventWriterConfig::try_new(1, Duration::from_millis(5))?,
        )
        .with_telemetry_pipeline(Arc::new(TelemetryPipeline::new(
            Arc::clone(&telemetry_metrics),
            Arc::new(NoopStructuredJsonExporter),
            Arc::new(NoopOpenTelemetryExporter),
        )));
        let event_sink: Arc<dyn GatewayEventSink> = Arc::new(P12FanoutEventSink::new(
            Arc::clone(&attempt_stages),
            Arc::clone(&event_queue),
        ));
        drop(event_queue);
        let executor = P12ObservedAttemptExecutor {
            events: p12_observed_canonical_events()?,
            event_sink: Arc::clone(&event_sink),
            attempt_stages: Arc::clone(&attempt_stages),
            route_id: RouteId::try_new("p12-obs-route")?,
            candidate_id: RouteCandidateId::try_new("p12-obs-candidate")?,
            credential_id: CredentialId::try_new("p12-obs-credential")?,
            endpoint_id: EndpointId::try_new("p12-obs-endpoint")?,
            upstream_id: UpstreamId::try_new("p12-obs-upstream")?,
        };
        let client_key = InMemoryClientKey::try_new(
            "p12-obs-client-key",
            ClientKeyId::try_new("p12-obs-client")?,
            true,
        )?;
        let authenticator: Arc<dyn ClientKeyAuthenticator> =
            Arc::new(InMemoryClientKeyAuthenticator::try_new([client_key])?);
        let state = ResponsesHttpState::with_metadata_and_event_sink(
            Arc::new(executor),
            Arc::new(SystemResponsesMetadataFactory::new()),
            authenticator,
            Arc::clone(&event_sink),
            default_stream_capacity()?,
        );
        drop(event_sink);
        let app = actix_test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure),
        )
        .await;
        let request = actix_test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header((header::AUTHORIZATION, "Bearer p12-obs-client-key"))
            .set_payload(r#"{"model":"p12-obs-model","input":"ok"}"#)
            .to_request();
        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        // The service response retains the request's app-data chain (and with it one clone of
        // the fanout sink), so it must drop with the service before the queue senders close.
        drop(response);
        drop(app);

        let reported = writer.run().await;
        assert_eq!(reported.required_events_committed, 3);
        assert_eq!(reported.rows_inserted, 3);
        assert_eq!(reported.pending_required, 0);
        let snapshot = telemetry_metrics.snapshot();
        assert_eq!(snapshot.request_events, 1);
        assert_eq!(snapshot.attempt_events, 1);
        assert_eq!(snapshot.usage_events, 1);
        assert_eq!(snapshot.attempts_succeeded, 1);
        assert_eq!(snapshot.input_tokens, 3);
        assert_eq!(snapshot.output_tokens, 5);

        let store = SqliteEventStore::open(&database)?;
        let stored = store.list_events()?;
        assert_eq!(stored.len(), 3);
        let request_id = stored
            .iter()
            .find(|event| event.kind() == GatewayEventLogKind::Request)
            .and_then(StoredGatewayEvent::request_id)
            .cloned()
            .ok_or_else(|| std::io::Error::other("request event missing"))?;
        drop(store);

        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p12-obs-test")?,
        )?;
        let mut facade = SnapshotManagementRuntimeFacade {
            registry: Arc::clone(lifecycle.registry()),
            attempt_stages,
            runtime_health: Arc::new(RuntimeHealthRegistry::new()),
            runtime_quota: Arc::new(RuntimeQuotaRegistry::new()),
            route_explain_scheduler: None,
            routing_price_snapshot: None,
            event_store: SqliteEventStore::open(&database)?,
        };
        let attempts = facade
            .list_request_attempts(&request_id)
            .map_err(|_| std::io::Error::other("management listing unavailable"))?;
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].outcome(), "succeeded");
        assert_eq!(
            attempts[0].stage(),
            Some(ManagementRequestAttemptStage::HttpTransport)
        );
        assert_eq!(
            attempts[0].endpoint_id().map(EndpointId::as_str),
            Some("p12-obs-endpoint")
        );
        assert_eq!(
            attempts[0].credential_id().map(CredentialId::as_str),
            Some("p12-obs-credential")
        );
        Ok(())
    }

    #[test]
    fn p12_fanout_sink_overflow_keeps_required_loss_explicit_and_stage_projection_intact()
    -> Result<(), Box<dyn Error>> {
        let attempts = Arc::new(P12AttemptStageStore::new());
        let (event_queue, mut receiver) =
            BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        let event_queue = Arc::new(event_queue);
        let sink = P12FanoutEventSink::new(Arc::clone(&attempts), Arc::clone(&event_queue));
        let terminal_attempt = |request_id: &RequestId| -> Result<AttemptEvent, Box<dyn Error>> {
            Ok(AttemptEvent::new(
                request_id.clone(),
                1,
                RouteId::try_new("p12-fanout-route")?,
                RouteCandidateId::try_new("p12-fanout-candidate")?,
                CredentialId::try_new("p12-fanout-credential")?,
                EndpointId::try_new("p12-fanout-endpoint")?,
                UpstreamId::try_new("p12-fanout-upstream")?,
                "p12-fanout-upstream-model".to_owned(),
                1,
                2,
                AttemptOutcome::Succeeded,
                AttemptRetryDecision::Completed,
            ))
        };

        let first = RequestId::try_new("p12-fanout-request")?;
        attempts.record_stage(&first, ManagementRequestAttemptStage::HttpStatus);
        assert_eq!(
            sink.try_emit(GatewayEvent::Attempt(terminal_attempt(&first)?)),
            EventEmission::Enqueued
        );
        let second = RequestId::try_new("p12-fanout-overflow")?;
        attempts.record_stage(&second, ManagementRequestAttemptStage::HttpStatus);
        assert_eq!(
            sink.try_emit(GatewayEvent::Attempt(terminal_attempt(&second)?)),
            EventEmission::RequiredQueueFull
        );
        assert_eq!(event_queue.metrics().required_queue_full, 1);
        assert_eq!(
            attempts.stage_view(&second),
            Some(ManagementRequestAttemptStage::HttpStatus)
        );

        let diagnostic = || {
            diagnostic_event(GatewayError::new(
                GatewayErrorCode::InternalError,
                gateway_core::ErrorScope::Internal,
            ))
        };
        assert_eq!(sink.try_emit(diagnostic()), EventEmission::Enqueued);
        assert_eq!(
            sink.try_emit(diagnostic()),
            EventEmission::DiagnosticDropped
        );
        assert_eq!(event_queue.metrics().diagnostics_dropped, 1);
        assert!(receiver.try_recv().is_some());
        Ok(())
    }

    #[test]
    fn p12_management_listing_survives_stage_ledger_exhaustion() -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let request_id = RequestId::try_new("p12-ledger-request")?;
        let attempt = AttemptEvent::new(
            request_id.clone(),
            1,
            RouteId::try_new("p12-ledger-route")?,
            RouteCandidateId::try_new("p12-ledger-candidate")?,
            CredentialId::try_new("p12-ledger-credential")?,
            EndpointId::try_new("p12-ledger-endpoint")?,
            UpstreamId::try_new("p12-ledger-upstream")?,
            "p12-ledger-upstream-model".to_owned(),
            5,
            9,
            AttemptOutcome::Failed(GatewayError::new(
                GatewayErrorCode::ProviderTransient,
                gateway_core::ErrorScope::Provider,
            )),
            AttemptRetryDecision::NonRetryable,
        );
        {
            let mut store = SqliteEventStore::open(&database)?;
            assert_eq!(store.append_batch(&[GatewayEvent::Attempt(attempt)])?, 1);
        }
        let attempt_stages = Arc::new(P12AttemptStageStore::new());
        for index in 0..=P12AttemptStageStore::MAX_RECORDS {
            let filler = RequestId::try_new(format!("p12-ledger-filler-{index}"))?;
            attempt_stages.record_stage(&filler, ManagementRequestAttemptStage::HttpTransport);
        }
        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p12-ledger-test")?,
        )?;
        let mut facade = SnapshotManagementRuntimeFacade {
            registry: Arc::clone(lifecycle.registry()),
            attempt_stages,
            runtime_health: Arc::new(RuntimeHealthRegistry::new()),
            runtime_quota: Arc::new(RuntimeQuotaRegistry::new()),
            route_explain_scheduler: None,
            routing_price_snapshot: None,
            event_store: SqliteEventStore::open(&database)?,
        };
        let attempts = facade
            .list_request_attempts(&request_id)
            .map_err(|_| std::io::Error::other("durable listing must survive ledger exhaustion"))?;
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].outcome(), "failed");
        assert_eq!(attempts[0].stage(), None);
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
    fn p12_composition_admits_normalized_codex_oauth_credentials() -> Result<(), Box<dyn Error>> {
        let secret_store = test_secret_store()?;
        let mut configuration = p12_configuration(&secret_store)?;
        configuration.credentials[0].kind = "oauth_json".to_owned();
        validate_p12_credential_bindings(&configuration)
            .map_err(|_| "normalized OAuth credential should be admitted")?;

        configuration.credentials[0].kind = "foreign_json".to_owned();
        assert!(validate_p12_credential_bindings(&configuration).is_err());
        Ok(())
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

        let request = p12_transport_request(&outbound, admitted, false, None)?;
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
            p12_transport_request(&outbound, mismatched, false, None)
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
                "id":"message-p12-anthropic-http",
                "type":"message",
                "role":"assistant",
                "status":"completed",
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
        let events = project_usage_events(
            decode_responses_production(
                r#"{
              "id":"response-p12-tool-http",
              "object":"response",
              "status":"completed",
              "created_at":10,
              "completed_at":11,
              "error":null,
              "frequency_penalty":0.0,
              "presence_penalty":0.0,
              "moderation":null,
              "prompt_cache_retention":"in-memory",
              "tool_usage":{
                "image_gen":{
                  "input_tokens":0,
                  "input_tokens_details":{"image_tokens":0,"text_tokens":0},
                  "output_tokens":0,
                  "output_tokens_details":{"image_tokens":0,"text_tokens":0},
                  "total_tokens":0
                },
                "web_search":{"num_requests":0}
              },
              "output":[{
                "id":"item-p12-tool-http",
                "type":"function_call",
                "call_id":"call-p12-tool-http",
                "name":"echo",
                "status":"completed",
                "arguments":"{\"value\":\"ok\"}",
                "metadata":{"turn_id":"turn-p12-tool-http"},
                "internal_chat_message_metadata_passthrough":{"turn_id":"turn-p12-tool-http"}
              }],
              "usage":{
                "input_tokens":3,
                "input_tokens_details":{"cached_tokens":0,"cache_write_tokens":0},
                "output_tokens":5,
                "output_tokens_details":{"reasoning_tokens":0},
                "total_tokens":8
              }
            }"#,
            )
            .map_err(|_| std::io::Error::other("production Responses Tool decode failed"))?,
            P12ResponseUsageProjection::AnthropicMessages,
        );
        let events = gateway_router::project_protocol_response(
            &CanonicalResponse::try_new(events)?,
            ProtocolFormat::AnthropicMessages,
        )?
        .into_events();
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
    fn widened_egress_shape_requires_https_only_hosts_and_no_redirects()
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
        assert!(has_p12_https_only_egress_shape(&policy));
        policy.allowed_hosts_json = r#"["gateway.example.test","other.example.test"]"#.to_owned();
        assert!(has_p12_https_only_egress_shape(&policy));
        policy.allowed_hosts_json = "[]".to_owned();
        assert!(!has_p12_https_only_egress_shape(&policy));
        policy.allowed_hosts_json = r#"["gateway.example.test"]"#.to_owned();
        policy.allowed_cidrs_json = r#"["127.0.0.0/8"]"#.to_owned();
        assert!(!has_p12_https_only_egress_shape(&policy));
        policy.allowed_cidrs_json = "[]".to_owned();
        policy.allowed_schemes_json = r#"["https","http"]"#.to_owned();
        assert!(!has_p12_https_only_egress_shape(&policy));
        policy.allowed_schemes_json = r#"["https"]"#.to_owned();
        policy.redirect_mode = StoredEgressRedirectMode::SameOrigin;
        policy.max_redirects = 1;
        assert!(!has_p12_https_only_egress_shape(&policy));
        assert!(has_p12_unlisted_model_override(
            r#"{"allow_unlisted_model":true}"#
        ));
        assert!(!has_p12_unlisted_model_override(
            r#"{"allow_unlisted_model":true,"tools":true}"#
        ));
        assert!(p12_candidate_override_is_admissible(
            "grok.build.responses",
            r#"{"allow_unlisted_model":true,"reasoning":false}"#
        ));
        assert!(p12_candidate_override_is_admissible(
            "grok.console.responses",
            r#"{"allow_unlisted_model":true,"reasoning":false}"#
        ));
        assert!(p12_candidate_override_is_admissible(
            "openai-compatible.responses",
            r#"{"allow_unlisted_model":true,"reasoning":false}"#
        ));
        assert!(!p12_candidate_override_is_admissible(
            "grok.build.responses",
            r#"{"allow_unlisted_model":true,"reasoning":true}"#
        ));
        Ok(())
    }

    #[test]
    fn endpoint_shape_admission_requires_a_paired_adapter_and_serving_format()
    -> Result<(), Box<dyn Error>> {
        let base = EndpointConfiguration {
            id: EndpointId::try_new("p12-shape-endpoint")?,
            upstream_id: UpstreamId::try_new("p12-shape-upstream")?,
            adapter_id: "openai-compatible.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: "https://gateway.example.test/v1".to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: None,
            transport: EndpointTransport::Http,
            enabled: true,
        };
        assert_eq!(validate_endpoint_shape(&base)?, ApiFormat::OpenAiResponses);

        let mut chat = base.clone();
        chat.adapter_id = "openai-compatible.chat-completions".to_owned();
        chat.api_format = "openai/chat-completions".to_owned();
        chat.inference_path = "/chat/completions".to_owned();
        assert_eq!(
            validate_endpoint_shape(&chat)?,
            ApiFormat::OpenAiChatCompletions
        );

        let mut messages = base.clone();
        messages.adapter_id = "anthropic-compatible.messages".to_owned();
        messages.api_format = "anthropic/messages".to_owned();
        assert_eq!(
            validate_endpoint_shape(&messages)?,
            ApiFormat::AnthropicMessages
        );

        let mut unsupported = base.clone();
        unsupported.api_format = "openai_responses".to_owned();
        assert!(validate_endpoint_shape(&unsupported).is_err());

        let mut mismatched = base.clone();
        mismatched.api_format = "anthropic/messages".to_owned();
        assert!(validate_endpoint_shape(&mismatched).is_err());

        let mut swapped = messages.clone();
        swapped.adapter_id = "openai-compatible.responses".to_owned();
        assert!(validate_endpoint_shape(&swapped).is_err());

        let mut disabled = base.clone();
        disabled.enabled = false;
        assert!(validate_endpoint_shape(&disabled).is_err());
        Ok(())
    }

    #[test]
    fn kiro_endpoint_shape_derives_its_kind_and_region_and_rejects_foreign_hosts()
    -> Result<(), Box<dyn Error>> {
        let base = EndpointConfiguration {
            id: EndpointId::try_new("p12-kiro-endpoint")?,
            upstream_id: UpstreamId::try_new("p12-kiro-upstream")?,
            adapter_id: "kiro.messages".to_owned(),
            api_format: "anthropic/messages".to_owned(),
            base_url: "https://runtime.us-east-1.kiro.dev".to_owned(),
            inference_path: "/".to_owned(),
            models_path: None,
            transport: EndpointTransport::Http,
            enabled: true,
        };
        // Kiro is a second implementation of a served format, so shape admission accepts it.
        assert_eq!(
            validate_endpoint_shape(&base)?,
            ApiFormat::AnthropicMessages
        );

        let (kind, region) = p12_kiro_endpoint_shape(&base)?;
        assert_eq!(kind, KiroEndpointKind::Cli);
        assert_eq!(region.as_str(), "us-east-1");
        assert!(build_kiro_messages_adapter(&base).is_ok());

        let mut ide = base.clone();
        ide.base_url = "https://q.eu-west-1.amazonaws.com".to_owned();
        ide.inference_path = "/generateAssistantResponse".to_owned();
        let (kind, region) = p12_kiro_endpoint_shape(&ide)?;
        assert_eq!(kind, KiroEndpointKind::Ide);
        assert_eq!(region.as_str(), "eu-west-1");
        assert!(build_kiro_messages_adapter(&ide).is_ok());

        // A host Kiro would never derive must be refused: an operator cannot point a Kiro Endpoint
        // at an arbitrary server and have credentials sent there.
        let mut foreign_host = base.clone();
        foreign_host.base_url = "https://attacker.example.test".to_owned();
        assert!(p12_kiro_endpoint_shape(&foreign_host).is_err());
        assert!(build_kiro_messages_adapter(&foreign_host).is_err());

        // The CLI host under the IDE path, and vice versa, derive different URLs than the stored
        // base_url, so the equality check rejects the mismatched pair.
        // The CLI host under the IDE path derives a different URL than the stored pair, so the
        // equality check rejects it even though both halves are individually valid.
        let mut crossed = base.clone();
        crossed.inference_path = "/generateAssistantResponse".to_owned();
        assert!(build_kiro_messages_adapter(&crossed).is_err());

        // A Kiro host with an extra path segment appended must not be accepted either.
        let mut extra_segment = base.clone();
        extra_segment.base_url = "https://runtime.us-east-1.kiro.dev/v1".to_owned();
        assert!(build_kiro_messages_adapter(&extra_segment).is_err());

        let mut plaintext = base.clone();
        plaintext.base_url = "http://runtime.us-east-1.kiro.dev".to_owned();
        assert!(p12_kiro_endpoint_shape(&plaintext).is_err());

        let mut unknown_path = base.clone();
        unknown_path.inference_path = "/v1/messages".to_owned();
        assert!(p12_kiro_endpoint_shape(&unknown_path).is_err());
        Ok(())
    }

    #[test]
    fn the_p12_registry_binds_verified_provider_adapters() -> Result<(), Box<dyn Error>> {
        let registry = p12_api_format_adapter_registry()?;
        // Both implementations of the same wire format resolve, selected by adapter_id alone.
        assert!(
            registry
                .resolve("anthropic/messages", "anthropic-compatible.messages")
                .is_some()
        );
        assert!(
            registry
                .resolve("anthropic/messages", "kiro.messages")
                .is_some()
        );
        assert!(
            registry
                .resolve("openai/responses", "openai-compatible.responses")
                .is_some()
        );
        assert!(
            registry
                .resolve("openai/responses", "grok.build.responses")
                .is_some()
        );
        assert!(
            registry
                .resolve("openai/responses", "grok.console.responses")
                .is_some()
        );
        assert!(
            registry
                .resolve("openai/responses", "grok.official.responses")
                .is_some()
        );
        assert!(
            registry
                .resolve("openai/responses", "grok.web.responses")
                .is_some()
        );
        assert!(
            registry
                .resolve(
                    "openai/chat-completions",
                    "openai-compatible.chat-completions"
                )
                .is_some()
        );
        // A format cannot borrow another format's implementation even though it is bound.
        assert!(
            registry
                .resolve("openai/responses", "kiro.messages")
                .is_none()
        );
        assert!(registry.resolve("kiro/messages", "kiro.messages").is_none());
        assert!(
            registry
                .resolve("openai/responses", "openai-compatible.chat-completions")
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn grok_endpoint_factories_pin_credentials_to_their_verified_targets()
    -> Result<(), Box<dyn Error>> {
        let build = EndpointConfiguration {
            id: EndpointId::try_new("p12-grok-build-endpoint")?,
            upstream_id: UpstreamId::try_new("p12-grok-upstream")?,
            adapter_id: "grok.build.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: GROK_BUILD_RESPONSES_BASE_URL.to_owned(),
            inference_path: GROK_BUILD_RESPONSES_PATH.to_owned(),
            models_path: None,
            transport: EndpointTransport::Http,
            enabled: true,
        };
        assert!(matches!(
            build_grok_build_responses_adapter(&build),
            Ok(EndpointAdapter::GrokBuildResponses)
        ));

        let mut substituted_build_host = build.clone();
        substituted_build_host.base_url = "https://attacker.example.test".to_owned();
        assert!(build_grok_build_responses_adapter(&substituted_build_host).is_err());
        let mut substituted_build_path = build.clone();
        substituted_build_path.inference_path = "/v1/chat/completions".to_owned();
        assert!(build_grok_build_responses_adapter(&substituted_build_path).is_err());

        let console = EndpointConfiguration {
            id: EndpointId::try_new("p12-grok-console-endpoint")?,
            adapter_id: "grok.console.responses".to_owned(),
            base_url: GROK_CONSOLE_RESPONSES_BASE_URL.to_owned(),
            inference_path: GROK_CONSOLE_RESPONSES_PATH.to_owned(),
            ..build.clone()
        };
        assert!(matches!(
            build_grok_console_responses_adapter(&console),
            Ok(EndpointAdapter::GrokConsoleResponses)
        ));

        let mut substituted_console_host = console.clone();
        substituted_console_host.base_url = "https://attacker.example.test".to_owned();
        assert!(build_grok_console_responses_adapter(&substituted_console_host).is_err());
        let mut substituted_console_path = console;
        substituted_console_path.inference_path = "/v1/chat/completions".to_owned();
        assert!(build_grok_console_responses_adapter(&substituted_console_path).is_err());

        let web = EndpointConfiguration {
            id: EndpointId::try_new("p12-grok-web-endpoint")?,
            adapter_id: "grok.web.responses".to_owned(),
            base_url: GROK_WEB_PRODUCTION_BASE_URL.to_owned(),
            inference_path: GROK_WEB_CANARY_PATH.to_owned(),
            ..build.clone()
        };
        assert!(matches!(
            build_grok_web_responses_adapter(&web),
            Ok(EndpointAdapter::GrokWebResponses)
        ));

        let mut substituted_web_host = web.clone();
        substituted_web_host.base_url = "https://attacker.example.test".to_owned();
        assert!(build_grok_web_responses_adapter(&substituted_web_host).is_err());
        let mut substituted_web_path = web;
        substituted_web_path.inference_path = "/v1/responses".to_owned();
        assert!(build_grok_web_responses_adapter(&substituted_web_path).is_err());

        let official = EndpointConfiguration {
            id: EndpointId::try_new("p12-grok-official-endpoint")?,
            adapter_id: "grok.official.responses".to_owned(),
            base_url: GROK_OFFICIAL_API_BASE_URL.to_owned(),
            inference_path: GROK_OFFICIAL_RESPONSES_PATH.to_owned(),
            ..build
        };
        assert!(matches!(
            build_grok_official_responses_adapter(&official),
            Ok(EndpointAdapter::GrokOfficialResponses)
        ));

        let mut substituted_official_host = official.clone();
        substituted_official_host.base_url = "https://attacker.example.test".to_owned();
        assert!(build_grok_official_responses_adapter(&substituted_official_host).is_err());
        let mut substituted_official_path = official;
        substituted_official_path.inference_path = "/v1/chat/completions".to_owned();
        assert!(build_grok_official_responses_adapter(&substituted_official_path).is_err());
        Ok(())
    }

    #[test]
    fn kiro_start_failures_preserve_exact_router_state_owners() {
        let unauthorized = GatewayError::new(
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
        );
        assert!(matches!(
            p12_classify_kiro_start_failure(unauthorized),
            AttemptFailure::NonRetryable(error)
                if error.code() == GatewayErrorCode::CredentialUnauthorized
        ));

        let forbidden =
            GatewayError::new(GatewayErrorCode::CredentialForbidden, ErrorScope::Account);
        assert!(matches!(
            p12_classify_kiro_start_failure(forbidden),
            AttemptFailure::NonRetryable(error)
                if error.code() == GatewayErrorCode::CredentialForbidden
        ));

        for error in [
            GatewayError::new(
                GatewayErrorCode::CredentialQuotaExceeded,
                ErrorScope::QuotaWindow,
            ),
            GatewayError::new(GatewayErrorCode::ProviderRateLimited, ErrorScope::Provider),
        ] {
            assert!(matches!(
                p12_classify_kiro_start_failure(error),
                AttemptFailure::RateLimited { retry_after: None }
            ));
        }

        let transient =
            GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider);
        assert!(matches!(
            p12_classify_kiro_start_failure(transient),
            AttemptFailure::Connection
        ));
    }

    #[test]
    fn p12_kiro_drops_only_the_inexpressible_output_ceiling_and_keeps_every_other_extension()
    -> Result<(), Box<dyn Error>> {
        let openai = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;

        // A request with no root extensions is passed through untouched, so the projection cannot
        // become a silent rewrite of the common case.
        assert_eq!(p12_kiro_request_projection(&openai.request), openai.request);

        // Every Anthropic Messages client sends the required `max_tokens`, which the decoder retains
        // under this namespace. Kiro's wire shape cannot express an output ceiling, so this one
        // extension is dropped -- otherwise `BC-PROVIDER-007` would reject 100% of real requests.
        let mut anthropic = openai.request.clone();
        anthropic.extensions.try_insert(
            "anthropic.messages.max_tokens",
            gateway_core::RawJson::from_json_string("19".to_owned())?,
        )?;
        let projected = p12_kiro_request_projection(&anthropic);
        assert!(
            projected
                .extensions
                .get("anthropic.messages.max_tokens")
                .is_none()
        );
        // Only the ceiling was removed: every other extension the decoder retained is still here,
        // and no canonical field moved.
        let mut expected = anthropic.clone();
        expected.extensions = RawExtensions::default();
        for (name, value) in anthropic.extensions.iter() {
            if name != "anthropic.messages.max_tokens" {
                expected.extensions.try_insert(name, value.clone())?;
            }
        }
        assert_eq!(projected, expected);

        // A foreign extension is *retained*, so the converter still fails closed on a semantic the
        // client may actually depend on, with the converter's own classification rather than a
        // silent drop here.
        let mut foreign = anthropic.clone();
        foreign.extensions.try_insert(
            "vendor.private.beta_feature",
            gateway_core::RawJson::from_json_string("true".to_owned())?,
        )?;
        let projected = p12_kiro_request_projection(&foreign);
        assert!(
            projected
                .extensions
                .get("anthropic.messages.max_tokens")
                .is_none()
        );
        assert_eq!(
            projected
                .extensions
                .get("vendor.private.beta_feature")
                .map(gateway_core::RawJson::get),
            Some("true")
        );
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
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p12-runtime-test")?,
        )?;
        let composition = build_data_plane_composition(
            &database,
            &secret_store,
            std::sync::Arc::clone(lifecycle.registry()),
            ClientKeyService::new(ClientKeyPepper::try_from_bytes([0xE1_u8; 32])?),
        )?;
        let account_page = composition
            .provider_account_pools
            .list_provider_account_pools(
                &gateway_control::provider_account_pool_service::ProviderAccountPoolQuery::default(
                ),
            )?;
        assert_eq!(account_page.items.len(), 1);
        assert_eq!(
            account_page.items[0].provider_id.as_str(),
            "p12-runtime-upstream"
        );
        assert_eq!(
            account_page.items[0].channel_id.as_str(),
            P12_SINGLETON_TEST_ENDPOINT_ID
        );
        assert_eq!(
            account_page.items[0].account_id.as_str(),
            "p12-runtime-credential"
        );
        assert_eq!(
            account_page.items[0].runtime_status,
            gateway_control::provider_account_pool_service::ProviderAccountRuntimeStatus::Available
        );
        drop(composition);
        assert!(directory.path().join("control.sqlite3").is_file());
        Ok(())
    }

    #[test]
    fn config_bound_price_policy_is_shared_with_runtime_route_explain() -> Result<(), Box<dyn Error>>
    {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let mut configuration = p12_configuration(&secret_store)?;
        configuration.routing_price_policy = Some(RoutingPricePolicyConfiguration::try_new(
            "routing-catalog-v1",
            RoutingPriceComparison::RateDominanceV1,
        )?);
        let config_version_id = configuration.version.id.clone();
        let catalog = BillingPriceCatalog {
            catalog_version_id: "routing-catalog-v1".to_owned(),
            effective_at_ms: 0,
            source: BillingCatalogSource::Test,
            created_at_ms: 0,
            entries: vec![BillingPriceEntry {
                provider_id: "p12-runtime-upstream".to_owned(),
                channel_id: P12_SINGLETON_TEST_ENDPOINT_ID.to_owned(),
                model: "p12-test-model".to_owned(),
                input_microunits_per_million: 1,
                output_microunits_per_million: 2,
                reasoning_microunits_per_million: 3,
                cache_read_microunits_per_million: 4,
                cache_creation_microunits_per_million: 5,
                cached_microunits_per_million: 6,
            }],
        };
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        let mut transaction = repository.begin_transaction()?;
        transaction.insert_billing_catalog(&catalog)?;
        transaction.commit()?;
        repository.write_configuration(&configuration)?;
        repository.activate_version(&config_version_id)?;
        drop(repository);

        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p13-07d-runtime-test")?,
        )?;
        let mut composition = build_data_plane_composition(
            &database,
            &secret_store,
            Arc::clone(lifecycle.registry()),
            ClientKeyService::new(ClientKeyPepper::try_from_bytes([0xE1_u8; 32])?),
        )?;
        let request = ManagementRouteExplainRequest::try_new(
            config_version_id,
            RouteId::try_new("p12-runtime-route")?,
            "p12-test-model".to_owned(),
            ManagementRequestProtocol::OpenAiResponses,
            Some(ProviderId::try_new("p12-runtime-upstream")?),
            1,
        )
        .map_err(|_| std::io::Error::other("routing price explain request unavailable"))?;
        let explain = composition
            .management_runtime
            .explain_route(&request)
            .map_err(|_| std::io::Error::other("routing price explain unavailable"))?;
        let policy = explain
            .price_policy()
            .ok_or("routing price policy missing")?;
        assert_eq!(policy.catalog_version_id(), "routing-catalog-v1");
        assert_eq!(policy.comparison(), "rate_dominance_v1");
        assert_eq!(explain.candidates().len(), 1);
        assert_eq!(explain.candidates()[0].price_evidence(), "equal");
        assert!(explain.candidates()[0].selected_by_projection());
        Ok(())
    }

    #[test]
    fn config_bound_price_policy_fails_closed_for_a_future_catalog() -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let mut configuration = p12_configuration(&secret_store)?;
        configuration.routing_price_policy = Some(RoutingPricePolicyConfiguration::try_new(
            "routing-catalog-failure",
            RoutingPriceComparison::RateDominanceV1,
        )?);
        let catalog = BillingPriceCatalog {
            catalog_version_id: "routing-catalog-failure".to_owned(),
            effective_at_ms: 9_000_000_000_000_000,
            source: BillingCatalogSource::Test,
            created_at_ms: 0,
            entries: vec![BillingPriceEntry {
                provider_id: "p12-runtime-upstream".to_owned(),
                channel_id: P12_SINGLETON_TEST_ENDPOINT_ID.to_owned(),
                model: "p12-test-model".to_owned(),
                input_microunits_per_million: 1,
                output_microunits_per_million: 1,
                reasoning_microunits_per_million: 1,
                cache_read_microunits_per_million: 1,
                cache_creation_microunits_per_million: 1,
                cached_microunits_per_million: 1,
            }],
        };
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        let mut transaction = repository.begin_transaction()?;
        transaction.insert_billing_catalog(&catalog)?;
        transaction.commit()?;
        let config_version_id = configuration.version.id.clone();
        repository.write_configuration(&configuration)?;
        repository.activate_version(&config_version_id)?;
        drop(repository);

        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p13-07d-failure-test")?,
        )?;
        let future = build_data_plane_composition(
            &database,
            &secret_store,
            Arc::clone(lifecycle.registry()),
            ClientKeyService::new(ClientKeyPepper::try_from_bytes([0xE1_u8; 32])?),
        );
        assert!(matches!(
            future,
            Err(RuntimeCompositionError::Stage(
                RuntimeCompositionStage::RoutingPricePolicy
            ))
        ));
        Ok(())
    }

    #[test]
    fn native_grok_metadata_maps_to_the_configured_provider_channel_without_a_send()
    -> Result<(), Box<dyn Error>> {
        let secret_store = test_secret_store()?;
        let configuration = p12_configuration(&secret_store)?;
        let endpoint_id = configuration.endpoints[0].id.clone();
        let bindings = [provider_grok::GrokAccountEndpointBinding::new(
            provider_grok::GrokAccountProvider::Build,
            endpoint_id,
        )];
        let accounts = [
            provider_grok::GrokAccountMetadata {
                id: "native-build-active".to_owned(),
                provider: provider_grok::GrokAccountProvider::Build,
                auth_status: provider_grok::GrokAccountAuthStatus::Active,
                enabled: true,
                priority: -1_000,
                weight: 3,
                max_concurrency: 2,
                refresh_due_at_ms: Some(500),
                quota_sync_due_at_ms: Some(600),
                cooldown_until_ms: Some(400),
                revision: 7,
                import_batch_id: "native-build-batch".to_owned(),
            },
            provider_grok::GrokAccountMetadata {
                id: "native-build-reauth".to_owned(),
                provider: provider_grok::GrokAccountProvider::Build,
                auth_status: provider_grok::GrokAccountAuthStatus::ReauthRequired,
                enabled: true,
                priority: 800,
                weight: 1,
                max_concurrency: 1,
                refresh_due_at_ms: None,
                quota_sync_due_at_ms: None,
                cooldown_until_ms: None,
                revision: 3,
                import_batch_id: "native-build-batch".to_owned(),
            },
        ];

        let rows =
            super::native_provider_account_descriptors(&configuration, &bindings, &accounts)?;
        assert_eq!(rows.len(), 2);
        let active = rows
            .iter()
            .find(|row| row.account_id.as_str() == "native-build-active")
            .ok_or("active native row missing")?;
        assert_eq!(active.provider_id.as_str(), "p12-runtime-upstream");
        assert_eq!(active.channel_id.as_str(), P12_SINGLETON_TEST_ENDPOINT_ID);
        assert_eq!(active.account_kind, "grok_build_oauth");
        assert_eq!(active.priority, 2_000);
        assert_eq!(active.weight, 3);
        assert_eq!(active.max_concurrency, 2);
        assert_eq!(active.upstream_models, ["p12-test-upstream-model"]);
        assert_eq!(
            active.runtime_status_hint,
            gateway_control::provider_account_pool_service::ProviderAccountRuntimeStatus::Available
        );
        let reauth = rows
            .iter()
            .find(|row| row.account_id.as_str() == "native-build-reauth")
            .ok_or("reauth native row missing")?;
        assert_eq!(
            reauth.auth_status,
            gateway_control::provider_account_pool_service::ProviderAccountAuthStatus::ReauthRequired
        );
        assert_eq!(
            reauth.runtime_status_hint,
            gateway_control::provider_account_pool_service::ProviderAccountRuntimeStatus::Unauthorized
        );
        Ok(())
    }

    #[test]
    fn invalid_management_projection_does_not_block_the_serving_pool() -> Result<(), Box<dyn Error>>
    {
        let secret_store = test_secret_store()?;
        let configuration = p12_configuration(&secret_store)?;
        let mut descriptors =
            super::ordinary_provider_account_descriptors(&configuration, &BTreeSet::new())?;
        descriptors[0].upstream_models = (0..257).map(|index| format!("model-{index}")).collect();
        let pools = Arc::new(CredentialPoolCompiler::new(&secret_store).compile(&configuration)?);
        assert!(pools.pool(&configuration.endpoints[0].id).is_some());

        let facade = super::provider_account_pool_facade(
            configuration.version.id.as_str().to_owned(),
            Ok(descriptors),
            pools,
            Arc::new(RuntimeHealthRegistry::new()),
            Arc::new(RuntimeQuotaRegistry::new()),
        );
        assert_eq!(
            facade.list_provider_account_pools(
                &gateway_control::provider_account_pool_service::ProviderAccountPoolQuery::default(
                ),
            ),
            Err(gateway_control::provider_account_pool_service::ProviderAccountPoolError::SourceUnavailable)
        );
        Ok(())
    }

    #[test]
    fn active_widened_production_graph_composes_an_encrypted_runtime_without_a_send()
    -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let configuration = p12_widened_configuration(&secret_store, &p12_production_network())?;
        let config_version_id = configuration.version.id.clone();
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&configuration)?;
        repository.activate_version(&config_version_id)?;
        drop(repository);

        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
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
    fn two_endpoint_graph_binds_each_endpoint_to_its_declared_format_adapter()
    -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let mut network = p12_production_network();
        network.endpoint_b_adapter = "anthropic-compatible.messages";
        network.endpoint_b_api_format = "anthropic/messages";
        let configuration = p12_widened_configuration(&secret_store, &network)?;
        let config_version_id = configuration.version.id.clone();
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&configuration)?;
        repository.activate_version(&config_version_id)?;
        drop(repository);

        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p12-runtime-test")?,
        )?;
        let snapshot = lifecycle.registry().load();
        let policies = EgressPolicyCompiler::compile(&configuration)?;
        let registry = p12_api_format_adapter_registry()?;
        let runtimes = endpoint_runtimes(
            &configuration,
            &snapshot,
            &policies,
            &registry,
            None,
            None,
            8191,
        )?;

        assert_eq!(runtimes.len(), 2);
        let endpoint_a = EndpointId::try_new("p12-widened-endpoint-a")?;
        let endpoint_b = EndpointId::try_new("p12-widened-endpoint-b")?;
        assert_eq!(
            runtimes
                .get(&endpoint_a)
                .map(|runtime| runtime.adapter.api_format()),
            Some(ApiFormat::OpenAiResponses)
        );
        assert_eq!(
            runtimes
                .get(&endpoint_b)
                .map(|runtime| runtime.adapter.api_format()),
            Some(ApiFormat::AnthropicMessages)
        );
        for route in snapshot.routes() {
            for candidate in route.candidates() {
                let bound = runtimes
                    .get(candidate.endpoint_id())
                    .ok_or("Candidate Endpoint has no bound adapter")?;
                assert_eq!(
                    bound.adapter.api_format().as_str(),
                    candidate.endpoint_api_format()
                );
            }
        }

        let composition = build_data_plane_composition(
            &database,
            &secret_store,
            std::sync::Arc::clone(lifecycle.registry()),
            ClientKeyService::new(ClientKeyPepper::try_from_bytes([0xE1_u8; 32])?),
        )?;
        drop(composition);
        Ok(())
    }

    #[test]
    fn single_format_graph_still_composes_and_binds_only_the_openai_adapter()
    -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let configuration = p12_widened_configuration(&secret_store, &p12_production_network())?;
        let config_version_id = configuration.version.id.clone();
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&configuration)?;
        repository.activate_version(&config_version_id)?;
        drop(repository);

        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p12-runtime-test")?,
        )?;
        let snapshot = lifecycle.registry().load();
        let policies = EgressPolicyCompiler::compile(&configuration)?;
        let runtimes = endpoint_runtimes(
            &configuration,
            &snapshot,
            &policies,
            &p12_api_format_adapter_registry()?,
            None,
            None,
            8191,
        )?;

        assert_eq!(runtimes.len(), 2);
        assert!(
            runtimes
                .values()
                .all(|runtime| runtime.adapter.api_format() == ApiFormat::OpenAiResponses)
        );
        Ok(())
    }

    #[test]
    fn an_endpoint_without_a_bound_adapter_fails_composition_closed() -> Result<(), Box<dyn Error>>
    {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let mut network = p12_production_network();
        network.endpoint_b_adapter = "anthropic-compatible.messages";
        network.endpoint_b_api_format = "anthropic/messages";
        let configuration = p12_widened_configuration(&secret_store, &network)?;
        let config_version_id = configuration.version.id.clone();
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&configuration)?;
        repository.activate_version(&config_version_id)?;
        drop(repository);

        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p12-runtime-test")?,
        )?;
        let snapshot = lifecycle.registry().load();
        let policies = EgressPolicyCompiler::compile(&configuration)?;
        let openai_only = ApiFormatAdapterRegistry::try_new([(
            ApiFormat::OpenAiResponses,
            "openai-compatible.responses",
            build_openai_responses_adapter as P12EndpointAdapterFactory,
        )])?;

        assert!(matches!(
            endpoint_runtimes(
                &configuration,
                &snapshot,
                &policies,
                &openai_only,
                None,
                None,
                8191,
            ),
            Err(RuntimeCompositionError::Unavailable)
        ));
        Ok(())
    }

    #[test]
    fn max_attempts_above_the_widened_bound_fails_admission_closed() -> Result<(), Box<dyn Error>> {
        let directory = TemporaryDirectory::new()?;
        let database = directory.join("control.sqlite3");
        let secret_store = test_secret_store()?;
        let mut network = p12_production_network();
        network.max_attempts = 6;
        let configuration = p12_widened_configuration(&secret_store, &network)?;
        let config_version_id = configuration.version.id.clone();
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&configuration)?;
        repository.activate_version(&config_version_id)?;
        drop(repository);

        let lifecycle = ManagementService::bootstrap(
            SqliteControlPlaneRepository::open(&database)?,
            deployment_route_compiler(&database)?,
            ManagementActor::try_new("p12-runtime-test")?,
        )?;
        let composition = build_data_plane_composition(
            &database,
            &secret_store,
            std::sync::Arc::clone(lifecycle.registry()),
            ClientKeyService::new(ClientKeyPepper::try_from_bytes([0xE1_u8; 32])?),
        );
        assert!(matches!(
            composition,
            Err(RuntimeCompositionError::Unavailable)
        ));
        Ok(())
    }

    #[test]
    fn route_explain_projects_protocol_registry_without_an_upstream_attempt()
    -> Result<(), Box<dyn Error>> {
        let version = ConfigVersionId::try_new("p12-d3-explain-config")?;
        let route_id = RouteId::try_new("p12-d3-explain-route")?;
        let public_model_id = PublicModelId::try_new("p12-d3-explain-model")?;
        let candidate = |id: &str,
                         endpoint: &str,
                         format: &str,
                         mode: SnapshotTransformMode|
         -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
            Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
                id: RouteCandidateId::try_new(id)?,
                endpoint_id: EndpointId::try_new(endpoint)?,
                upstream_id: UpstreamId::try_new(format!("upstream-{endpoint}"))?,
                endpoint_api_format: format.to_owned(),
                upstream_model: "explain-upstream-model".to_owned(),
                transform_mode: mode,
                priority: 0,
                weight: 1,
                effective_capabilities: CapabilitySet::empty(),
                catalog_admission: SnapshotCatalogAdmission::AllowedUnlisted,
                active_binding_count: 1,
            }))
        };
        let candidates = vec![
            candidate(
                "candidate-wrong-mode",
                "endpoint-responses",
                "openai/responses",
                SnapshotTransformMode::Canonical,
            )?,
            candidate(
                "candidate-bridge",
                "endpoint-messages",
                "anthropic/messages",
                SnapshotTransformMode::LosslessBridge,
            )?,
        ];
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new(version.as_str())?,
            vec![SnapshotPublicModel::new(
                public_model_id.clone(),
                "p12-d3-public-model".to_owned(),
                "P12 D3 public model".to_owned(),
                CapabilitySet::empty(),
                route_id.clone(),
            )],
            Vec::new(),
            vec![SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                SnapshotRoutePolicy::RoundRobin,
                1,
                1_000,
                candidates,
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let mut facade = SnapshotManagementRuntimeFacade {
            registry: Arc::new(RouteSnapshotRegistry::new(snapshot)),
            attempt_stages: Arc::new(P12AttemptStageStore::new()),
            runtime_health: Arc::new(RuntimeHealthRegistry::new()),
            runtime_quota: Arc::new(RuntimeQuotaRegistry::new()),
            route_explain_scheduler: None,
            routing_price_snapshot: None,
            event_store: SqliteEventStore::open_in_memory()?,
        };
        let request = ManagementRouteExplainRequest::try_new(
            version,
            route_id,
            "p12-d3-public-model".to_owned(),
            ManagementRequestProtocol::OpenAiChatCompletions,
            None,
            1,
        )
        .map_err(|_| std::io::Error::other("route explain request unavailable"))?;
        let explained = facade
            .explain_route(&request)
            .map_err(|_| std::io::Error::other("route explain unavailable"))?;
        assert_eq!(explained.candidates().len(), 2);
        assert_eq!(
            explained.candidates()[0].reason(),
            Some("protocol_transform_unavailable"),
        );
        assert!(explained.candidates()[1].selected_by_projection());
        let attempts = facade
            .list_request_attempts(&RequestId::try_new("never-started")?)
            .map_err(|_| std::io::Error::other("attempt listing unavailable"))?;
        assert!(attempts.is_empty());
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One explicit two-Provider fixture proves the scope boundary.
    fn provider_scoped_route_explain_requires_scope_for_multiple_providers()
    -> Result<(), Box<dyn Error>> {
        let version = ConfigVersionId::try_new("p13-07b-explain-config")?;
        let route_id = RouteId::try_new("p13-07b-explain-route")?;
        let public_model_id = PublicModelId::try_new("p13-07b-explain-model")?;
        let candidate = |id: &str, endpoint: &str, provider: &str, priority: i64| {
            Ok::<_, Box<dyn Error>>(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
                id: RouteCandidateId::try_new(id)?,
                endpoint_id: EndpointId::try_new(endpoint)?,
                upstream_id: UpstreamId::try_new(provider)?,
                endpoint_api_format: "openai/responses".to_owned(),
                upstream_model: "p13-07b-upstream-model".to_owned(),
                transform_mode: SnapshotTransformMode::Canonical,
                priority,
                weight: 1,
                effective_capabilities: CapabilitySet::empty(),
                catalog_admission: SnapshotCatalogAdmission::AllowedUnlisted,
                active_binding_count: 1,
            }))
        };
        let candidates = vec![
            candidate("candidate-a", "endpoint-a", "provider-a", 0)?,
            candidate("candidate-b", "endpoint-b", "provider-b", 1)?,
        ];
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new(version.as_str())?,
            vec![SnapshotPublicModel::new(
                public_model_id.clone(),
                "p13-07b-public-model".to_owned(),
                "P13-07B public model".to_owned(),
                CapabilitySet::empty(),
                route_id.clone(),
            )],
            Vec::new(),
            vec![SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                SnapshotRoutePolicy::PriorityFailover,
                1,
                1_000,
                candidates,
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let pools = Arc::new(EndpointCredentialPools::try_new(vec![
            EndpointCredentialPool::try_new(
                EndpointId::try_new("endpoint-a")?,
                [EndpointCredentialInput {
                    credential_id: CredentialId::try_new("credential-a")?,
                    credential_kind: "bearer".to_owned(),
                    credential_revision: 1,
                    priority: 0,
                    weight: 1,
                    concurrency: 1,
                    expires_at_ms: None,
                    secret: CredentialSecret::try_new(b"p13-07b-a".to_vec())?,
                }],
            )?,
            EndpointCredentialPool::try_new(
                EndpointId::try_new("endpoint-b")?,
                [EndpointCredentialInput {
                    credential_id: CredentialId::try_new("credential-b")?,
                    credential_kind: "bearer".to_owned(),
                    credential_revision: 1,
                    priority: 0,
                    weight: 1,
                    concurrency: 1,
                    expires_at_ms: None,
                    secret: CredentialSecret::try_new(b"p13-07b-b".to_vec())?,
                }],
            )?,
        ])?);
        let scheduler = Arc::new(RouteCredentialScheduler::new(Arc::clone(&snapshot), pools));
        let runtime_health = Arc::new(RuntimeHealthRegistry::new());
        let runtime_quota = Arc::new(RuntimeQuotaRegistry::new());
        let registry = Arc::new(RouteSnapshotRegistry::new(snapshot));
        let mut facade = SnapshotManagementRuntimeFacade {
            registry,
            attempt_stages: Arc::new(P12AttemptStageStore::new()),
            runtime_health: Arc::clone(&runtime_health),
            runtime_quota: Arc::clone(&runtime_quota),
            route_explain_scheduler: Some(scheduler),
            routing_price_snapshot: None,
            event_store: SqliteEventStore::open_in_memory()?,
        };
        let unscoped = ManagementRouteExplainRequest::try_new(
            version.clone(),
            route_id.clone(),
            "p13-07b-public-model".to_owned(),
            ManagementRequestProtocol::OpenAiResponses,
            None,
            100,
        )
        .map_err(|_| std::io::Error::other("unscoped route explain request unavailable"))?;
        let unscoped = facade
            .explain_route(&unscoped)
            .map_err(|_| std::io::Error::other("unscoped route explain unavailable"))?;
        assert!(
            unscoped
                .candidates()
                .iter()
                .all(|candidate| candidate.reason() == Some("provider_scope_required"))
        );

        let scoped = ManagementRouteExplainRequest::try_new(
            version,
            route_id,
            "p13-07b-public-model".to_owned(),
            ManagementRequestProtocol::OpenAiResponses,
            Some(ProviderId::try_new("provider-a")?),
            100,
        )
        .map_err(|_| std::io::Error::other("scoped route explain request unavailable"))?;
        let scoped = facade
            .explain_route(&scoped)
            .map_err(|_| std::io::Error::other("scoped route explain unavailable"))?;
        assert!(scoped.candidates()[0].selected_by_projection());
        assert_eq!(scoped.candidates()[1].reason(), Some("provider_mismatch"));
        Ok(())
    }

    type FacadeFixture = (
        SnapshotManagementRuntimeFacade,
        Arc<FixedRuntimeClock>,
        Arc<RuntimeHealthRegistry>,
        Arc<RuntimeQuotaRegistry>,
        ConfigVersionId,
    );

    fn management_facade_fixture(now_ms: i64) -> Result<FacadeFixture, Box<dyn Error>> {
        let version = ConfigVersionId::try_new("p12-facade-config")?;
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new("p12-facade-config")?,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ))?);
        let clock = Arc::new(FixedRuntimeClock::new(now_ms));
        let runtime_clock: Arc<dyn RuntimeHealthClock> = clock.clone();
        let runtime_health = Arc::new(RuntimeHealthRegistry::with_clock(Arc::clone(
            &runtime_clock,
        )));
        let runtime_quota = Arc::new(RuntimeQuotaRegistry::with_clock(runtime_clock));
        let facade = SnapshotManagementRuntimeFacade {
            registry: Arc::new(RouteSnapshotRegistry::new(snapshot)),
            attempt_stages: Arc::new(P12AttemptStageStore::new()),
            runtime_health: Arc::clone(&runtime_health),
            runtime_quota: Arc::clone(&runtime_quota),
            route_explain_scheduler: None,
            routing_price_snapshot: None,
            event_store: SqliteEventStore::open_in_memory()?,
        };
        Ok((facade, clock, runtime_health, runtime_quota, version))
    }

    #[derive(Debug)]
    struct FixedRuntimeClock {
        now_ms: AtomicI64,
    }

    impl FixedRuntimeClock {
        const fn new(now_ms: i64) -> Self {
            Self {
                now_ms: AtomicI64::new(now_ms),
            }
        }

        fn set_now_ms(&self, now_ms: i64) {
            self.now_ms.store(now_ms, Ordering::Release);
        }
    }

    impl RuntimeHealthClock for FixedRuntimeClock {
        fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
            Ok(self.now_ms.load(Ordering::Acquire))
        }
    }

    #[test]
    fn operator_quota_reset_recovers_a_due_binding_through_the_real_handle()
    -> Result<(), Box<dyn Error>> {
        let (mut facade, clock, _health, quota, version) = management_facade_fixture(1_000)?;
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential = CredentialId::try_new("credential-a")?;
        let target = ManagementRuntimeTarget::try_new(endpoint.clone(), credential.clone(), None)
            .map_err(|_| "invalid management target")?;
        let quota_target =
            RuntimeQuotaTarget::endpoint_credential(endpoint.clone(), credential.clone());

        assert_eq!(
            facade
                .request_quota_recovery(&version, &target, 1_000)
                .map_err(|error| format!("{error:?}"))?,
            ManagementQuotaRecoveryState::Rejected
        );

        quota.record_rate_limited(
            quota_target,
            1_000,
            Some(Duration::from_millis(500)),
            Duration::from_millis(500),
        )?;
        assert_eq!(
            facade
                .request_quota_recovery(&version, &target, 1_000)
                .map_err(|error| format!("{error:?}"))?,
            ManagementQuotaRecoveryState::RecoveryRequired
        );
        assert!(!quota.endpoint_credential_is_available(&endpoint, &credential));

        clock.set_now_ms(1_500);
        assert_eq!(
            facade
                .request_quota_recovery(&version, &target, 1_500)
                .map_err(|error| format!("{error:?}"))?,
            ManagementQuotaRecoveryState::ProbeScheduled
        );
        assert!(quota.endpoint_credential_is_available(&endpoint, &credential));
        Ok(())
    }

    #[test]
    fn operator_endpoint_recovers_a_forbidden_account_with_explicit_evidence()
    -> Result<(), Box<dyn Error>> {
        let (mut facade, _clock, health, _quota, version) = management_facade_fixture(1_000)?;
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential = CredentialId::try_new("credential-a")?;
        let target = ManagementRuntimeTarget::try_new(endpoint.clone(), credential.clone(), None)
            .map_err(|_| "invalid management target")?;

        health.mark_credential_forbidden(endpoint.clone(), credential.clone())?;
        assert!(!health.endpoint_credential_is_available(&endpoint, &credential));

        assert_eq!(
            facade
                .request_quota_recovery(&version, &target, 1_000)
                .map_err(|error| format!("{error:?}"))?,
            ManagementQuotaRecoveryState::ProbeScheduled
        );
        assert_eq!(
            health.credential_account_status_at(&endpoint, &credential, 1_000)?,
            RuntimeCredentialAccountStatus::Available
        );
        assert!(health.endpoint_credential_is_available(&endpoint, &credential));
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
                endpoint: EndpointId::try_new(P12_SINGLETON_TEST_ENDPOINT_ID)?,
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

    fn p13_compatible_endpoint_runtimes(
        configuration: &ControlPlaneConfiguration,
    ) -> Result<BTreeMap<EndpointId, EndpointRuntime>, Box<dyn Error>> {
        let route = configuration
            .model_routes
            .first()
            .ok_or("missing fixture route")?;
        let candidate = configuration
            .route_candidates
            .iter()
            .find(|candidate| candidate.route_id == route.id)
            .ok_or("missing fixture candidate")?;
        let endpoint = configuration
            .endpoints
            .iter()
            .find(|endpoint| endpoint.id == candidate.endpoint_id)
            .ok_or("missing fixture endpoint")?;
        let public_model = configuration
            .public_models
            .iter()
            .find(|model| model.id == route.public_model_id)
            .ok_or("missing fixture public model")?;
        let snapshot = RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new(configuration.version.id.as_str())?,
            vec![SnapshotPublicModel::new(
                public_model.id.clone(),
                public_model.model_name.clone(),
                public_model.display_name.clone(),
                CapabilitySet::empty(),
                route.id.clone(),
            )],
            Vec::new(),
            vec![SnapshotRoute::new(
                route.id.clone(),
                route.public_model_id.clone(),
                SnapshotRoutePolicy::RoundRobin,
                route.max_attempts,
                route.bootstrap_timeout_ms,
                vec![SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
                    id: candidate.id.clone(),
                    endpoint_id: endpoint.id.clone(),
                    upstream_id: endpoint.upstream_id.clone(),
                    endpoint_api_format: endpoint.api_format.clone(),
                    upstream_model: candidate.upstream_model.clone(),
                    transform_mode: SnapshotTransformMode::Canonical,
                    priority: candidate.priority,
                    weight: candidate.weight,
                    effective_capabilities: p12_adapter_capabilities(&endpoint.adapter_id)?,
                    catalog_admission: SnapshotCatalogAdmission::AllowedUnlisted,
                    active_binding_count: configuration
                        .endpoint_credential_bindings
                        .iter()
                        .filter(|binding| binding.endpoint_id == endpoint.id && binding.enabled)
                        .count(),
                })],
            )],
            Vec::new(),
            Vec::new(),
        ))?;
        let policies = EgressPolicyCompiler::compile(configuration)?;
        Ok(endpoint_runtimes(
            configuration,
            &snapshot,
            &policies,
            &p12_api_format_adapter_registry()?,
            None,
            None,
            8191,
        )?)
    }

    #[test]
    fn p13_compatible_active_graph_compiles_fixed_pool_and_direct_defaults_without_network()
    -> Result<(), Box<dyn Error>> {
        let secret_store = test_secret_store()?;
        let mut configuration = p12_configuration(&secret_store)?;
        configuration.version.status = ConfigVersionStatus::Active;
        let ids = P12RuntimeIds::try_new()?;
        let pool_id = CompatibleProxyPoolId::try_new("p13-runtime-pool")?;
        configuration
            .compatible_proxy_pools
            .push(CompatibleProxyPoolConfiguration {
                id: pool_id.clone(),
                upstream_id: ids.upstream.clone(),
                name: "P13 runtime pool".to_owned(),
                enabled: true,
            });
        for (node, weight, port) in [("node-a", 2_u16, 1080_u16), ("node-b", 1, 1081)] {
            let node_id = CompatibleProxyNodeId::try_new(node)?;
            configuration
                .compatible_proxy_nodes
                .push(CompatibleProxyNodeConfiguration {
                    encrypted_proxy: seal_compatible_proxy_node_endpoint(
                        &secret_store,
                        &configuration.version.id,
                        &ids.upstream,
                        Some(&pool_id),
                        &node_id,
                        &format!("socks5://127.0.0.1:{port}"),
                    )?,
                    id: node_id,
                    upstream_id: ids.upstream.clone(),
                    pool_id: Some(pool_id.clone()),
                    name: format!("P13 {node}"),
                    enabled: true,
                    weight,
                    maximum_concurrency: 1,
                });
        }
        configuration
            .compatible_egress_bindings
            .push(CompatibleEgressBindingConfiguration {
                endpoint_id: ids.endpoint.clone(),
                credential_id: ids.credential.clone(),
                target: CompatibleEgressTargetConfiguration::ProxyPool(pool_id.clone()),
                failure_scope: StoredCompatibleFailureScope::EgressNode,
                stickiness: StoredCompatibleStickiness::CredentialAndEgress,
                pre_submit_max_attempts: 2,
            });
        let endpoints = p13_compatible_endpoint_runtimes(&configuration)?;
        let (registries, settings) =
            compatible_egress_runtime_inputs(&configuration, &endpoints, &secret_store)?;
        let registry = registries
            .get(&ids.upstream)
            .ok_or("missing compatible registry")?;
        let target = CompatibleEgressTarget::ProxyPool {
            pool_id: pool_id.as_str().to_owned(),
        };
        assert!(registry.contains_target(&CompatibleEgressTarget::Direct));
        assert!(registry.contains_target(&target));
        let setting = settings
            .get(&(ids.endpoint.clone(), ids.credential.clone()))
            .ok_or("missing compatible binding settings")?;
        assert_eq!(setting.target, target);
        assert_eq!(setting.failure_scope, CompatibleFailureScope::EgressNode);
        assert_eq!(
            setting.stickiness,
            CompatibleStickiness::CredentialAndEgress
        );
        assert_eq!(setting.retry_policy.max_attempts(), 2);

        let first = registry.try_acquire(&setting.target, 10)?;
        let second = registry.try_acquire(&setting.target, 10)?;
        assert_ne!(first.selected_node_id(), second.selected_node_id());
        assert!(registry.try_acquire(&setting.target, 10).is_err());
        assert!(!format!("{first:?}{second:?}").contains("127.0.0.1"));
        drop((first, second));
        assert!(registry.try_acquire(&setting.target, 10).is_ok());

        let mut direct = p12_configuration(&secret_store)?;
        direct.version.status = ConfigVersionStatus::Active;
        let direct_endpoints = p13_compatible_endpoint_runtimes(&direct)?;
        let (direct_registries, direct_settings) =
            compatible_egress_runtime_inputs(&direct, &direct_endpoints, &secret_store)?;
        assert!(direct_settings.is_empty());
        assert!(
            direct_registries
                .get(&ids.upstream)
                .is_some_and(|registry| registry.contains_target(&CompatibleEgressTarget::Direct))
        );
        Ok(())
    }

    #[test]
    fn p13_compatible_active_graph_rejects_empty_foreign_and_wrong_aad_resources()
    -> Result<(), Box<dyn Error>> {
        let secret_store = test_secret_store()?;
        let ids = P12RuntimeIds::try_new()?;

        let mut empty_pool = p12_configuration(&secret_store)?;
        empty_pool.version.status = ConfigVersionStatus::Active;
        empty_pool
            .compatible_proxy_pools
            .push(CompatibleProxyPoolConfiguration {
                id: CompatibleProxyPoolId::try_new("empty-pool")?,
                upstream_id: ids.upstream.clone(),
                name: "empty".to_owned(),
                enabled: true,
            });
        let empty_endpoints = p13_compatible_endpoint_runtimes(&empty_pool)?;
        assert!(
            compatible_egress_runtime_inputs(&empty_pool, &empty_endpoints, &secret_store).is_err()
        );

        let mut wrong_aad = p12_configuration(&secret_store)?;
        wrong_aad.version.status = ConfigVersionStatus::Active;
        let node_id = CompatibleProxyNodeId::try_new("wrong-aad-node")?;
        wrong_aad
            .compatible_proxy_nodes
            .push(CompatibleProxyNodeConfiguration {
                encrypted_proxy: seal_compatible_proxy_node_endpoint(
                    &secret_store,
                    &ConfigVersionId::try_new("another-version")?,
                    &ids.upstream,
                    None,
                    &node_id,
                    "socks5://127.0.0.1:1080",
                )?,
                id: node_id,
                upstream_id: ids.upstream.clone(),
                pool_id: None,
                name: "wrong aad".to_owned(),
                enabled: true,
                weight: 1,
                maximum_concurrency: 1,
            });
        let wrong_aad_endpoints = p13_compatible_endpoint_runtimes(&wrong_aad)?;
        assert!(
            compatible_egress_runtime_inputs(&wrong_aad, &wrong_aad_endpoints, &secret_store)
                .is_err()
        );

        let mut foreign = p12_configuration(&secret_store)?;
        foreign.version.status = ConfigVersionStatus::Active;
        let foreign_upstream = UpstreamId::try_new("foreign-native-upstream")?;
        foreign.upstreams.push(UpstreamConfiguration {
            id: foreign_upstream.clone(),
            name: "foreign".to_owned(),
            kind: "native".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: Some(ids.egress_policy.clone()),
        });
        let foreign_node = CompatibleProxyNodeId::try_new("foreign-node")?;
        foreign
            .compatible_proxy_nodes
            .push(CompatibleProxyNodeConfiguration {
                encrypted_proxy: seal_compatible_proxy_node_endpoint(
                    &secret_store,
                    &foreign.version.id,
                    &foreign_upstream,
                    None,
                    &foreign_node,
                    "socks5://127.0.0.1:1080",
                )?,
                id: foreign_node,
                upstream_id: foreign_upstream,
                pool_id: None,
                name: "foreign node".to_owned(),
                enabled: true,
                weight: 1,
                maximum_concurrency: 1,
            });
        let foreign_endpoints = p13_compatible_endpoint_runtimes(&foreign)?;
        assert!(
            compatible_egress_runtime_inputs(&foreign, &foreign_endpoints, &secret_store).is_err()
        );

        Ok(())
    }

    #[test]
    fn p13_compatible_active_graph_rejects_orphaned_binding_profile() -> Result<(), Box<dyn Error>>
    {
        let secret_store = test_secret_store()?;
        let ids = P12RuntimeIds::try_new()?;
        let mut configuration = p12_configuration(&secret_store)?;
        configuration.version.status = ConfigVersionStatus::Active;
        let endpoints = p13_compatible_endpoint_runtimes(&configuration)?;
        configuration.endpoint_credential_bindings.clear();
        configuration
            .compatible_egress_bindings
            .push(CompatibleEgressBindingConfiguration {
                endpoint_id: ids.endpoint,
                credential_id: ids.credential,
                target: CompatibleEgressTargetConfiguration::Direct,
                failure_scope: StoredCompatibleFailureScope::Credential,
                stickiness: StoredCompatibleStickiness::Credential,
                pre_submit_max_attempts: 1,
            });
        assert!(
            compatible_egress_runtime_inputs(&configuration, &endpoints, &secret_store).is_err()
        );
        Ok(())
    }

    struct P12WidenedNetwork {
        allowed_scheme: &'static str,
        host_a: &'static str,
        port_a: u16,
        host_b: &'static str,
        port_b: u16,
        allow_loopback: bool,
        endpoint_b_adapter: &'static str,
        endpoint_b_api_format: &'static str,
        max_attempts: i64,
    }

    fn p12_production_network() -> P12WidenedNetwork {
        P12WidenedNetwork {
            allowed_scheme: "https",
            host_a: "gateway-a.example.test",
            port_a: 443,
            host_b: "gateway-b.example.test",
            port_b: 443,
            allow_loopback: false,
            endpoint_b_adapter: "openai-compatible.responses",
            endpoint_b_api_format: "openai/responses",
            max_attempts: 3,
        }
    }

    #[allow(clippy::too_many_lines)] // One reviewed widened graph is clearer as a single fixture.
    fn p12_widened_configuration(
        secret_store: &SecretStore,
        network: &P12WidenedNetwork,
    ) -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
        let version = ConfigVersion {
            id: ConfigVersionId::try_new("p12-widened-config")?,
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 0,
            description: "P12 widened runtime composition test".to_owned(),
        };
        let mut configuration = ControlPlaneConfiguration::new(version);
        let cidrs = if network.allow_loopback {
            r#"["127.0.0.1/32"]"#
        } else {
            "[]"
        };
        for (suffix, host, port) in [
            ("a", network.host_a, network.port_a),
            ("b", network.host_b, network.port_b),
        ] {
            configuration
                .egress_policies
                .push(EgressPolicyConfiguration {
                    id: EgressPolicyId::try_new(format!("p12-widened-egress-{suffix}"))?,
                    name: format!("P12 widened egress {suffix}"),
                    allowed_schemes_json: format!(r#"["{}"]"#, network.allowed_scheme),
                    allowed_hosts_json: format!(r#"["{host}"]"#),
                    allowed_ports_json: format!("[{port}]"),
                    allowed_cidrs_json: cidrs.to_owned(),
                    redirect_mode: StoredEgressRedirectMode::Deny,
                    max_redirects: 0,
                });
            configuration.upstreams.push(UpstreamConfiguration {
                id: UpstreamId::try_new(format!("p12-widened-upstream-{suffix}"))?,
                name: format!("P12 widened upstream {suffix}"),
                kind: "openai-compatible".to_owned(),
                enabled: true,
                tags_json: "[]".to_owned(),
                egress_policy_id: Some(EgressPolicyId::try_new(format!(
                    "p12-widened-egress-{suffix}"
                ))?),
            });
        }
        for (suffix, host, port, adapter, api_format) in [
            (
                "a",
                network.host_a,
                network.port_a,
                "openai-compatible.responses",
                "openai/responses",
            ),
            (
                "b",
                network.host_b,
                network.port_b,
                network.endpoint_b_adapter,
                network.endpoint_b_api_format,
            ),
        ] {
            configuration.endpoints.push(EndpointConfiguration {
                id: EndpointId::try_new(format!("p12-widened-endpoint-{suffix}"))?,
                upstream_id: UpstreamId::try_new(format!("p12-widened-upstream-{suffix}"))?,
                adapter_id: adapter.to_owned(),
                api_format: api_format.to_owned(),
                base_url: format!("{}://{host}:{port}/v1", network.allowed_scheme),
                inference_path: "/responses".to_owned(),
                models_path: None,
                transport: EndpointTransport::Http,
                enabled: true,
            });
        }
        for (endpoint, name, weight) in [
            ("a", "a1", 3_i64),
            ("a", "a2", 2),
            ("a", "a3", 1),
            ("b", "b1", 1),
        ] {
            let credential_id = CredentialId::try_new(format!("p12-widened-credential-{name}"))?;
            let upstream_id = UpstreamId::try_new(format!("p12-widened-upstream-{endpoint}"))?;
            let associated_data = credential_associated_data(
                &configuration.version.id,
                &credential_id,
                &upstream_id,
            )?;
            configuration.credentials.push(CredentialConfiguration {
                id: credential_id.clone(),
                upstream_id: upstream_id.clone(),
                kind: "bearer".to_owned(),
                encrypted_secret: secret_store
                    .seal(format!("test-bearer-{name}").as_bytes(), &associated_data)?,
                status: CredentialStatus::Active,
                revision: 1,
            });
            configuration.endpoint_credential_bindings.push(
                EndpointCredentialBindingConfiguration {
                    endpoint_id: EndpointId::try_new(format!("p12-widened-endpoint-{endpoint}"))?,
                    credential_id,
                    upstream_id,
                    enabled: true,
                    priority: 0,
                    weight,
                    concurrency: 1,
                },
            );
        }
        for (model, name) in [
            ("p12-widened-model-primary", "p12-widened-primary"),
            ("p12-widened-model-secondary", "p12-widened-secondary"),
        ] {
            configuration.public_models.push(PublicModelConfiguration {
                id: PublicModelId::try_new(model)?,
                model_name: name.to_owned(),
                status: AdministrativeStatus::Active,
                display_name: name.to_owned(),
                capabilities_json: "{}".to_owned(),
            });
        }
        configuration.model_aliases.push(ModelAliasConfiguration {
            alias: "p12-widened-primary-alias".to_owned(),
            public_model_id: PublicModelId::try_new("p12-widened-model-primary")?,
        });
        configuration.model_routes.push(ModelRouteConfiguration {
            id: RouteId::try_new("p12-widened-route-primary")?,
            public_model_id: PublicModelId::try_new("p12-widened-model-primary")?,
            policy: RoutePolicy::SmoothWeightedRoundRobin,
            max_attempts: network.max_attempts,
            bootstrap_timeout_ms: 15_000,
        });
        configuration.model_routes.push(ModelRouteConfiguration {
            id: RouteId::try_new("p12-widened-route-secondary")?,
            public_model_id: PublicModelId::try_new("p12-widened-model-secondary")?,
            policy: RoutePolicy::SmoothWeightedRoundRobin,
            max_attempts: 1,
            bootstrap_timeout_ms: 15_000,
        });
        for (candidate, route, endpoint, priority) in [
            (
                "p12-widened-candidate-primary-a",
                "p12-widened-route-primary",
                "p12-widened-endpoint-a",
                0_i64,
            ),
            (
                "p12-widened-candidate-primary-b",
                "p12-widened-route-primary",
                "p12-widened-endpoint-b",
                1,
            ),
            (
                "p12-widened-candidate-secondary-b",
                "p12-widened-route-secondary",
                "p12-widened-endpoint-b",
                0,
            ),
        ] {
            configuration
                .route_candidates
                .push(RouteCandidateConfiguration {
                    id: RouteCandidateId::try_new(candidate)?,
                    route_id: RouteId::try_new(route)?,
                    endpoint_id: EndpointId::try_new(endpoint)?,
                    upstream_model: "p12-widened-upstream-model".to_owned(),
                    credential_scope: CredentialScope::EndpointBindings,
                    transform_mode: TransformMode::Canonical,
                    enabled: true,
                    priority,
                    weight: 1,
                    capability_override_json: r#"{"allow_unlisted_model":true}"#.to_owned(),
                });
        }
        configuration.access_groups.push(AccessGroupConfiguration {
            id: AccessGroupId::try_new("p12-widened-group")?,
            name: "P12 widened group".to_owned(),
            status: AdministrativeStatus::Active,
            limits_json: "{}".to_owned(),
        });
        for route in ["p12-widened-route-primary", "p12-widened-route-secondary"] {
            configuration
                .access_group_routes
                .push(AccessGroupRouteConfiguration {
                    access_group_id: AccessGroupId::try_new("p12-widened-group")?,
                    route_id: RouteId::try_new(route)?,
                    enabled: true,
                });
        }
        configuration.client_keys.push(StoredClientKey::try_new(
            ClientKeyId::try_new("p12-widened-client-key-one")?,
            AccessGroupId::try_new("p12-widened-group")?,
            "rgw_0123456789abcdef",
            [0xA2_u8; 32],
            StoredClientKeyStatus::Active,
            None,
        )?);
        configuration.client_keys.push(StoredClientKey::try_new(
            ClientKeyId::try_new("p12-widened-client-key-two")?,
            AccessGroupId::try_new("p12-widened-group")?,
            "rgw_fedcba9876543210",
            [0xB4_u8; 32],
            StoredClientKeyStatus::Active,
            None,
        )?);
        Ok(configuration)
    }
}
