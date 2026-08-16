//! Bounded request-scoped Attempt orchestration.
//!
//! This module composes the existing immutable route schedule, Endpoint-local Credential leases,
//! and sharded runtime health into one pre-first-semantic-event retry loop. It intentionally owns
//! no HTTP client, Provider decoder, persistence handle, or downstream protocol writer.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use gateway_catalog::SemanticCapability;
use gateway_core::{
    AttemptEvent, AttemptOutcome, AttemptRetryDecision, CredentialId, ErrorScope, EventEmission,
    GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink, NoopGatewayEventSink,
    ProviderId, RequestId, RouteCandidateId, RouteId, TransparentRetryGate,
};
use gateway_upstream::CredentialLease;

use crate::{
    ProtocolFormat, ProviderScopedRouteExplainInput, QuotaConfidence, QuotaSnapshot, QuotaSource,
    ResponsesContinuationKind, ResponsesContinuationPin, RouteCredentialScheduler,
    RouteExplainInput, RuntimeHealthClock, RuntimeHealthKey, RuntimeHealthRegistry,
    RuntimeQuotaRecoveryProbe, RuntimeQuotaRegistry, RuntimeQuotaTarget, SelectedRouteCredential,
    SnapshotRoute, SnapshotRouteCandidate, SystemRuntimeHealthClock,
};

/// The finite estimated reset used for a 429 that does not declare retry-after information.
pub const DEFAULT_RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(30);

/// The finite Endpoint Cooldown used for connection, 5xx, and pre-semantic truncation failures.
pub const DEFAULT_TRANSIENT_COOLDOWN: Duration = Duration::from_secs(5);

/// Extra recovery-ticket lifetime granted past one driver-declared start ceiling.
///
/// A controlled quota probe is an ordinary admitted Attempt, so its exclusive ticket must outlive
/// the longest legitimate in-flight start plus completion bookkeeping. Expiry is the bounded
/// fail-closed path for an abandoned probe: the target simply becomes due again.
const QUOTA_RECOVERY_PROBE_GRACE: Duration = Duration::from_secs(30);

/// A boxed, sendable async operation used by an [`AttemptDriver`].
pub type AttemptFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Safe construction failures for [`AttemptOrchestratorConfig`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptOrchestratorConfigError {
    /// The fallback reset estimate for a 429 must be strictly positive.
    ZeroRateLimitCooldown,
    /// The Endpoint Cooldown for a transient failure must be strictly positive.
    ZeroTransientCooldown,
}

impl fmt::Display for AttemptOrchestratorConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroRateLimitCooldown => {
                formatter.write_str("rate-limit fallback reset estimate must be positive")
            }
            Self::ZeroTransientCooldown => {
                formatter.write_str("transient cooldown must be positive")
            }
        }
    }
}

impl std::error::Error for AttemptOrchestratorConfigError {}

/// Non-secret retry configuration shared by request-scoped Attempt orchestrators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttemptOrchestratorConfig {
    rate_limit_fallback_cooldown: Duration,
    transient_cooldown: Duration,
}

impl AttemptOrchestratorConfig {
    /// Validates finite positive 429 fallback and transient Cooldown values.
    ///
    /// # Errors
    ///
    /// Returns [`AttemptOrchestratorConfigError`] before a retry loop can issue an invalid
    /// runtime-state deadline.
    pub fn try_new(
        rate_limit_fallback_cooldown: Duration,
        transient_cooldown: Duration,
    ) -> Result<Self, AttemptOrchestratorConfigError> {
        if rate_limit_fallback_cooldown.is_zero() {
            return Err(AttemptOrchestratorConfigError::ZeroRateLimitCooldown);
        }
        if transient_cooldown.is_zero() {
            return Err(AttemptOrchestratorConfigError::ZeroTransientCooldown);
        }

        Ok(Self {
            rate_limit_fallback_cooldown,
            transient_cooldown,
        })
    }

    /// Returns the fallback reset estimate for a 429 without retry-after information.
    #[must_use]
    pub const fn rate_limit_fallback_cooldown(self) -> Duration {
        self.rate_limit_fallback_cooldown
    }

    /// Returns the Endpoint Cooldown for a connection, 5xx, or pre-semantic truncation failure.
    #[must_use]
    pub const fn transient_cooldown(self) -> Duration {
        self.transient_cooldown
    }
}

impl Default for AttemptOrchestratorConfig {
    fn default() -> Self {
        Self {
            rate_limit_fallback_cooldown: DEFAULT_RATE_LIMIT_COOLDOWN,
            transient_cooldown: DEFAULT_TRANSIENT_COOLDOWN,
        }
    }
}

/// A classified, secret-free result from one failed Attempt driver invocation.
#[derive(Clone, Eq, PartialEq)]
pub enum AttemptFailure {
    /// DNS, connection, TLS, or other pre-response transport failure.
    Connection,
    /// A 429 with an optional parsed retry-after duration.
    RateLimited {
        /// A positive upstream retry-after duration when one was safely parsed.
        retry_after: Option<Duration>,
    },
    /// A retryable 5xx response before downstream semantic output.
    ServerError,
    /// The source ended or became invalid before a client-visible semantic event.
    BootstrapTruncated,
    /// The request was cancelled during the Attempt.
    Cancelled,
    /// A classified failure that must not be retried by this task.
    NonRetryable(GatewayError),
}

impl AttemptFailure {
    /// Returns whether the failure is eligible for a pre-first-semantic-event retry.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Connection
                | Self::RateLimited { .. }
                | Self::ServerError
                | Self::BootstrapTruncated
        )
    }

    /// Returns the stable secret-free error exposed when this failure cannot be retried.
    #[must_use]
    pub fn safe_error(&self) -> GatewayError {
        match self {
            Self::Connection => egress_unavailable_error(),
            Self::RateLimited { .. } => provider_rate_limited_error(),
            Self::ServerError => provider_transient_error(),
            Self::BootstrapTruncated => stream_truncated_error(),
            Self::Cancelled => request_cancelled_error(),
            Self::NonRetryable(error) => error.clone(),
        }
    }

    fn cooldown(&self, config: AttemptOrchestratorConfig) -> Option<CooldownScope> {
        match self {
            Self::Connection | Self::ServerError | Self::BootstrapTruncated => {
                Some(CooldownScope::Endpoint(config.transient_cooldown()))
            }
            Self::RateLimited { .. } | Self::Cancelled | Self::NonRetryable(_) => None,
        }
    }
}

impl fmt::Debug for AttemptFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection => formatter.write_str("AttemptFailure::Connection"),
            Self::RateLimited { retry_after } => formatter
                .debug_struct("AttemptFailure::RateLimited")
                .field("retry_after_present", &retry_after.is_some())
                .finish(),
            Self::ServerError => formatter.write_str("AttemptFailure::ServerError"),
            Self::BootstrapTruncated => formatter.write_str("AttemptFailure::BootstrapTruncated"),
            Self::Cancelled => formatter.write_str("AttemptFailure::Cancelled"),
            Self::NonRetryable(error) => formatter
                .debug_tuple("AttemptFailure::NonRetryable")
                .field(error)
                .finish(),
        }
    }
}

/// Starts one selected upstream Attempt without exposing transport or Provider types to routing.
///
/// The driver receives a Candidate and a live Credential lease only by borrow. It must not retain
/// Credential bytes after its returned future completes. A successful output is retained by
/// [`StartedAttempt`], which keeps that lease alive for the caller's output lifetime.
pub trait AttemptDriver: Send + Sync {
    /// The live output started by one successful Attempt.
    type Output: Send;

    /// Starts one Attempt under this driver's per-attempt ceiling from [`Self::start_timeout`].
    ///
    /// The driver must report only a safe [`AttemptFailure`] for failure. It must not encode raw
    /// upstream status bodies, endpoint URLs, headers, or Secret material into that error.
    fn start<'a>(
        &'a self,
        candidate: &'a SnapshotRouteCandidate,
        credential: &'a CredentialLease,
        bootstrap_timeout: Duration,
    ) -> AttemptFuture<'a, Result<Self::Output, AttemptFailure>>;

    /// Returns the wall-clock ceiling applied to one in-flight [`Self::start`] invocation.
    ///
    /// The default keeps the historical bound: one Attempt may run only as long as the remaining
    /// cumulative bootstrap budget. A driver whose healthy start legitimately outlives that
    /// budget — a non-streaming Attempt against an upstream that buffers response headers until
    /// generation finishes — may return a longer mode-specific ceiling. The value bounds only the
    /// in-flight Attempt; whether a subsequent Attempt may begin is still governed exclusively by
    /// the Route's cumulative bootstrap deadline, so an expiry here remains a retryable
    /// pre-first-semantic-event failure that cannot extend the retry window.
    fn start_timeout(&self, remaining_bootstrap: Duration) -> Duration {
        remaining_bootstrap
    }
}

/// A request-local set of bindings that cannot be retried by the same external request.
#[derive(Default)]
pub struct AttemptExclusionSet {
    bindings: BTreeSet<AttemptBinding>,
}

impl AttemptExclusionSet {
    /// Creates an empty per-request exclusion set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one attempted Candidate/Credential binding.
    ///
    /// The input contains only stable non-secret IDs. Recording the same binding more than once
    /// is idempotent.
    pub fn insert(&mut self, candidate: &SnapshotRouteCandidate, credential_id: &CredentialId) {
        self.bindings
            .insert(AttemptBinding::new(candidate, credential_id));
    }

    /// Returns whether this exact Candidate/Credential binding has already failed in the request.
    #[must_use]
    pub fn contains(
        &self,
        candidate: &SnapshotRouteCandidate,
        credential_id: &CredentialId,
    ) -> bool {
        self.bindings
            .contains(&AttemptBinding::new(candidate, credential_id))
    }

    /// Returns the number of retained attempted bindings without exposing their identities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Returns whether no failed binding has been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl fmt::Debug for AttemptExclusionSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AttemptExclusionSet")
            .field("binding_count", &self.bindings.len())
            .finish()
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
struct AttemptBinding {
    candidate_id: RouteCandidateId,
    credential_id: CredentialId,
}

impl AttemptBinding {
    fn new(candidate: &SnapshotRouteCandidate, credential_id: &CredentialId) -> Self {
        Self {
            candidate_id: candidate.id().clone(),
            credential_id: credential_id.clone(),
        }
    }
}

/// One successfully started output paired with the lease that keeps its Credential reserved.
pub struct StartedAttempt<T> {
    selection: SelectedRouteCredential,
    output: T,
    attempts_started: usize,
}

impl<T> StartedAttempt<T> {
    /// Returns the selected non-secret Route Candidate.
    #[must_use]
    pub fn candidate(&self) -> &SnapshotRouteCandidate {
        self.selection.candidate()
    }

    /// Returns the live Credential lease held for this output's lifetime.
    #[must_use]
    pub fn lease(&self) -> &CredentialLease {
        self.selection.lease()
    }

    /// Returns the started live output without releasing its Credential lease.
    #[must_use]
    pub fn output(&self) -> &T {
        &self.output
    }

    /// Returns the total number of Attempts that began before this output succeeded.
    #[must_use]
    pub const fn attempts_started(&self) -> usize {
        self.attempts_started
    }

    /// Consumes the wrapper into its output and selected lease-bearing binding.
    ///
    /// Callers that keep only the output must retain the accompanying selection until its stream
    /// or response lifetime is complete; dropping the selection releases Credential capacity.
    #[must_use]
    pub fn into_parts(self) -> (T, SelectedRouteCredential) {
        (self.output, self.selection)
    }
}

impl<T> fmt::Debug for StartedAttempt<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StartedAttempt")
            .field("attempts_started", &self.attempts_started)
            .field("output", &"<opaque>")
            .finish_non_exhaustive()
    }
}

/// A reusable, process-local coordinator for one request's pre-semantic Attempt loop.
pub struct AttemptOrchestrator {
    scheduler: Arc<RouteCredentialScheduler>,
    runtime_health: Arc<RuntimeHealthRegistry>,
    runtime_quota: Arc<RuntimeQuotaRegistry>,
    clock: Arc<dyn RuntimeHealthClock>,
    config: AttemptOrchestratorConfig,
}

impl AttemptOrchestrator {
    /// Creates an orchestrator with the system clock and finite default Cooldowns.
    #[must_use]
    pub fn new(
        scheduler: Arc<RouteCredentialScheduler>,
        runtime_health: Arc<RuntimeHealthRegistry>,
    ) -> Self {
        Self::with_clock_and_config(
            scheduler,
            runtime_health,
            Arc::new(SystemRuntimeHealthClock),
            AttemptOrchestratorConfig::default(),
        )
    }

    /// Creates an orchestrator with an injected clock and explicit non-secret policy.
    ///
    /// Tests inject the same clock into runtime health and this retry budget. Production callers
    /// use [`Self::new`] unless they require a controlled clock source.
    #[must_use]
    pub fn with_clock_and_config(
        scheduler: Arc<RouteCredentialScheduler>,
        runtime_health: Arc<RuntimeHealthRegistry>,
        clock: Arc<dyn RuntimeHealthClock>,
        config: AttemptOrchestratorConfig,
    ) -> Self {
        let runtime_quota = Arc::new(RuntimeQuotaRegistry::with_clock(Arc::clone(&clock)));
        Self::with_runtime_quota_and_clock_config(
            scheduler,
            runtime_health,
            runtime_quota,
            clock,
            config,
        )
    }

    /// Creates an orchestrator with an injected exact-target quota registry and runtime clock.
    ///
    /// Deterministic tests share one clock across Health, Quota, and the retry budget. The P12
    /// production composition also uses this constructor so the exact registries the request path
    /// consults can be handed to the management facade for controlled recovery, without giving
    /// the request path a persistence or network dependency.
    #[must_use]
    pub fn with_runtime_quota_and_clock_config(
        scheduler: Arc<RouteCredentialScheduler>,
        runtime_health: Arc<RuntimeHealthRegistry>,
        runtime_quota: Arc<RuntimeQuotaRegistry>,
        clock: Arc<dyn RuntimeHealthClock>,
        config: AttemptOrchestratorConfig,
    ) -> Self {
        Self {
            scheduler,
            runtime_health,
            runtime_quota,
            clock,
            config,
        }
    }

    /// Starts one eligible Attempt, transparently falling through only before semantic delivery.
    ///
    /// The Route's immutable maximum Attempt count and cumulative bootstrap deadline are applied
    /// to this one invocation. On a retryable failure the failed binding is locally excluded and
    /// scoped runtime health is updated before the next selection. The returned wrapper owns the
    /// successful selection and keeps its Credential capacity leased until dropped.
    ///
    /// # Errors
    ///
    /// Returns only an existing secret-free [`GatewayError`]. It returns the last classified
    /// failure once the budget, runtime availability, cancellation, or first-semantic-event gate
    /// prevents another transparent Attempt.
    pub async fn start<D>(
        &self,
        route_id: &RouteId,
        driver: &D,
        retry_gate: &dyn TransparentRetryGate,
    ) -> Result<StartedAttempt<D::Output>, GatewayError>
    where
        D: AttemptDriver,
    {
        let event_sink = NoopGatewayEventSink;
        let all_candidates = |_candidate: &SnapshotRouteCandidate| true;
        self.start_inner(
            None,
            route_id,
            &all_candidates,
            false,
            driver,
            retry_gate,
            &event_sink,
        )
        .await
    }

    /// Starts one Attempt loop for a client protocol using only matching Endpoint formats.
    ///
    /// The filter is applied before Health, Quota, or Credential-pool access. A same-Upstream
    /// Endpoint that declares another protocol therefore cannot be selected, and a circuit on
    /// one protocol's Endpoint cannot make a different Endpoint ineligible by association. This
    /// narrow entrypoint admits only same-protocol Canonical Candidates; pass-through and
    /// cross-protocol bridge callers must first apply P5-04's request-local admission analysis.
    ///
    /// # Errors
    ///
    /// Returns the existing secret-free routing errors when the matching protocol has no healthy
    /// candidate or when all eligible attempts fail.
    pub async fn start_for_protocol<D>(
        &self,
        route_id: &RouteId,
        protocol: ProtocolFormat,
        driver: &D,
        retry_gate: &dyn TransparentRetryGate,
    ) -> Result<StartedAttempt<D::Output>, GatewayError>
    where
        D: AttemptDriver,
    {
        let event_sink = NoopGatewayEventSink;
        let matching_protocol = |candidate: &SnapshotRouteCandidate| {
            candidate_matches_protocol(candidate, Some(protocol))
        };
        self.start_inner(
            None,
            route_id,
            &matching_protocol,
            false,
            driver,
            retry_gate,
            &event_sink,
        )
        .await
    }

    /// Starts one Attempt loop and emits one terminal observation per actual driver invocation.
    ///
    /// The supplied sink is called only through its synchronous non-blocking `try_emit` port. A
    /// queue saturation, disabled sink, or closed receiver cannot delay, retry, or otherwise
    /// change the request's routing outcome.
    ///
    /// # Errors
    ///
    /// Returns the same safe routing errors as [`Self::start`]. Event admission does not introduce
    /// a new public error path.
    pub async fn start_with_event_sink<D>(
        &self,
        request_id: &RequestId,
        route_id: &RouteId,
        driver: &D,
        retry_gate: &dyn TransparentRetryGate,
        event_sink: &dyn GatewayEventSink,
    ) -> Result<StartedAttempt<D::Output>, GatewayError>
    where
        D: AttemptDriver,
    {
        let all_candidates = |_candidate: &SnapshotRouteCandidate| true;
        self.start_inner(
            Some(request_id),
            route_id,
            &all_candidates,
            false,
            driver,
            retry_gate,
            event_sink,
        )
        .await
    }

    /// Starts one protocol-filtered Attempt loop and records the same safe Attempt events as
    /// [`Self::start_with_event_sink`].
    ///
    /// # Errors
    ///
    /// Returns the existing secret-free routing errors when the matching protocol has no healthy
    /// candidate or when all eligible attempts fail.
    pub async fn start_with_event_sink_for_protocol<D>(
        &self,
        request_id: &RequestId,
        route_id: &RouteId,
        protocol: ProtocolFormat,
        driver: &D,
        retry_gate: &dyn TransparentRetryGate,
        event_sink: &dyn GatewayEventSink,
    ) -> Result<StartedAttempt<D::Output>, GatewayError>
    where
        D: AttemptDriver,
    {
        let matching_protocol = |candidate: &SnapshotRouteCandidate| {
            candidate_matches_protocol(candidate, Some(protocol))
        };
        self.start_inner(
            Some(request_id),
            route_id,
            &matching_protocol,
            false,
            driver,
            retry_gate,
            event_sink,
        )
        .await
    }

    /// Starts one Attempt loop using an explicit request-local Candidate admission predicate.
    ///
    /// The predicate runs before Health, Quota, Credential-pool access, lease acquisition, and
    /// driver invocation. Protocol transformation callers use this seam to exclude every
    /// unregistered or semantically lossy source/target pair without creating an upstream Attempt.
    ///
    /// # Errors
    ///
    /// Returns the existing safe routing error when no Candidate passes the predicate or when all
    /// admitted Attempts fail.
    pub async fn start_with_event_sink_matching<D, F>(
        &self,
        request_id: &RequestId,
        route_id: &RouteId,
        is_candidate_eligible: F,
        driver: &D,
        retry_gate: &dyn TransparentRetryGate,
        event_sink: &dyn GatewayEventSink,
    ) -> Result<StartedAttempt<D::Output>, GatewayError>
    where
        D: AttemptDriver,
        F: Fn(&SnapshotRouteCandidate) -> bool + Sync,
    {
        self.start_inner(
            Some(request_id),
            route_id,
            &is_candidate_eligible,
            false,
            driver,
            retry_gate,
            event_sink,
        )
        .await
    }

    /// Starts the bounded Attempt loop with the Provider-scoped selector as an advisory ranking.
    ///
    /// The route must have exactly one Provider after the caller's admission predicate is
    /// applied. The selector is rebuilt for every pre-lease iteration and every ranked Candidate
    /// is revalidated by the same scheduler immediately before its atomic lease. This keeps the
    /// existing max-attempts, first-semantic-event, cancellation, quota-recovery, and retry state
    /// machine as the sole owner of request execution while preventing an implicit cross-Provider
    /// fallback when the public request has no Provider scope field.
    ///
    /// # Errors
    ///
    /// Returns the existing secret-free `CredentialUnavailable/Credential` error when no admitted
    /// Candidate exists or more than one Provider is represented by the route.
    pub async fn start_with_event_sink_provider_scoped_matching<D, F>(
        &self,
        request_id: &RequestId,
        route_id: &RouteId,
        is_candidate_eligible: F,
        driver: &D,
        retry_gate: &dyn TransparentRetryGate,
        event_sink: &dyn GatewayEventSink,
    ) -> Result<StartedAttempt<D::Output>, GatewayError>
    where
        D: AttemptDriver,
        F: Fn(&SnapshotRouteCandidate) -> bool + Sync,
    {
        self.start_inner(
            Some(request_id),
            route_id,
            &is_candidate_eligible,
            true,
            driver,
            retry_gate,
            event_sink,
        )
        .await
    }

    /// Starts exactly one operator-pinned Attempt and never retries or falls back.
    ///
    /// This entrypoint is reserved for management diagnostics.  The caller supplies the complete
    /// immutable Route/Provider/Channel/Credential identity and an optional Candidate admission
    /// predicate (for example, protocol/adapter compatibility).  Selection revalidates Health,
    /// Quota, expiry, and capacity immediately before leasing, then invokes the driver at most
    /// once.  A retryable driver failure is reported with `RetryClosed`; no exclusion, quota
    /// recovery probe, sibling selection, or cross-Provider fallback is attempted.
    ///
    /// The returned [`StartedAttempt`] owns the live lease until its output is consumed/dropped,
    /// exactly like ordinary serving.  A failed or cancelled invocation drops the lease before
    /// returning, and the optional event sink receives one value-free terminal Attempt event when
    /// a driver invocation actually began.
    ///
    /// # Errors
    ///
    /// Returns a secret-free gateway error when the retry gate is cancelled, the route or exact
    /// binding is unavailable, the bounded bootstrap budget expires, or the single driver call
    /// fails. No error path performs a second lease or driver invocation.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // Keep one-shot ordering auditable.
    pub async fn start_pinned_once_with_event_sink<D, F>(
        &self,
        request_id: &RequestId,
        route_id: &RouteId,
        provider_id: &ProviderId,
        channel_id: &gateway_core::EndpointId,
        credential_id: &CredentialId,
        is_candidate_eligible: F,
        driver: &D,
        retry_gate: &dyn TransparentRetryGate,
        event_sink: &dyn GatewayEventSink,
    ) -> Result<StartedAttempt<D::Output>, GatewayError>
    where
        D: AttemptDriver,
        F: Fn(&SnapshotRouteCandidate) -> bool + Sync,
    {
        if retry_gate.is_cancelled() {
            return Err(request_cancelled_error());
        }

        let route = self
            .scheduler
            .route(route_id)
            .ok_or_else(credential_unavailable_error)?;
        let now_ms = self.clock.now_ms().map_err(|_| internal_error())?;
        let mut budget = RetryBudget::from_route(&route, now_ms)?;
        if !budget.can_start_at(now_ms) {
            return Err(egress_unavailable_error());
        }
        let selection = self.scheduler.select_pinned_and_lease_at(
            route_id,
            provider_id,
            channel_id,
            credential_id,
            &self.runtime_health,
            &self.runtime_quota,
            now_ms,
            is_candidate_eligible,
        )?;
        let started_at_ms = self.clock.now_ms().map_err(|_| internal_error())?;
        if retry_gate.is_cancelled() || !budget.can_start_at(started_at_ms) {
            drop(selection);
            return Err(if retry_gate.is_cancelled() {
                request_cancelled_error()
            } else {
                egress_unavailable_error()
            });
        }
        let remaining_bootstrap = budget.remaining_at(started_at_ms)?;
        budget.record_start();
        let attempt_number =
            u64::try_from(budget.attempts_started()).map_err(|_| internal_error())?;
        let attempt_result: Result<D::Output, AttemptFailure> = tokio::select! {
            biased;
            () = retry_gate.cancelled() => Err(AttemptFailure::Cancelled),
            result = tokio::time::timeout(
                driver.start_timeout(remaining_bootstrap),
                driver.start(
                    selection.candidate(),
                    selection.lease(),
                    remaining_bootstrap,
                ),
            ) => match result {
                Ok(result) => result,
                Err(_) => Err(AttemptFailure::Connection),
            },
        };

        match attempt_result {
            Ok(output) => {
                if retry_gate.is_cancelled() {
                    self.emit_pinned_attempt(
                        request_id,
                        route_id,
                        &selection,
                        attempt_number,
                        started_at_ms,
                        AttemptOutcome::Failed(request_cancelled_error()),
                        AttemptRetryDecision::Cancelled,
                        event_sink,
                    )?;
                    return Err(request_cancelled_error());
                }
                self.emit_pinned_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Succeeded,
                    AttemptRetryDecision::Completed,
                    event_sink,
                )?;
                Ok(StartedAttempt {
                    selection,
                    output,
                    attempts_started: budget.attempts_started(),
                })
            }
            Err(AttemptFailure::Cancelled) => {
                self.emit_pinned_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Failed(request_cancelled_error()),
                    AttemptRetryDecision::Cancelled,
                    event_sink,
                )?;
                Err(request_cancelled_error())
            }
            Err(failure) => {
                let safe_failure = failure.safe_error();
                // A Channel Pin is diagnostic-only: it observes the shared Health/Quota
                // registries for admission but must not feed one operator probe back into
                // serving state. The value-free Attempt event remains the audit projection.
                let retry_decision = if failure.is_retryable() {
                    AttemptRetryDecision::RetryClosed
                } else {
                    AttemptRetryDecision::NonRetryable
                };
                self.emit_pinned_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Failed(safe_failure.clone()),
                    retry_decision,
                    event_sink,
                )?;
                Err(safe_failure)
            }
        }
    }

    /// Starts one exact stored-history execution with no retry or fallback.
    ///
    /// Unlike the operator diagnostic pin, this serving path records exact Health/Quota failure
    /// ownership. It still performs one Candidate/Credential-revision lease and at most one driver
    /// call; quota recovery, sibling selection, transparent retry, and cross-Provider fallback are
    /// never entered.
    ///
    /// # Errors
    ///
    /// Returns a secret-free gateway error for stale lineage, unavailable exact state,
    /// cancellation, bootstrap timeout, event-sink loss, or the single driver failure.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub async fn start_continuation_once_with_event_sink<D, F>(
        &self,
        request_id: &RequestId,
        pin: &ResponsesContinuationPin,
        is_candidate_eligible: F,
        driver: &D,
        retry_gate: &dyn TransparentRetryGate,
        event_sink: &dyn GatewayEventSink,
    ) -> Result<StartedAttempt<D::Output>, GatewayError>
    where
        D: AttemptDriver,
        F: Fn(&SnapshotRouteCandidate) -> bool + Sync,
    {
        if retry_gate.is_cancelled() {
            return Err(request_cancelled_error());
        }
        let lineage = pin.lineage();
        let route_id = lineage.route_id();
        let route = self
            .scheduler
            .route(route_id)
            .ok_or_else(credential_unavailable_error)?;
        let now_ms = self.clock.now_ms().map_err(|_| internal_error())?;
        let mut budget = RetryBudget::from_route(&route, now_ms)?;
        if !budget.can_start_at(now_ms) {
            return Err(egress_unavailable_error());
        }
        let required_capability = match pin.kind() {
            ResponsesContinuationKind::StoredResponse => SemanticCapability::StoredResponses,
            ResponsesContinuationKind::Compaction => SemanticCapability::ResponseCompaction,
            ResponsesContinuationKind::WebSocketSession => SemanticCapability::ResponsesWebSocket,
        };
        let selection = self.scheduler.select_continuation_and_lease_at(
            route_id,
            lineage.provider_id(),
            lineage.upstream_id(),
            lineage.channel_id(),
            lineage.route_candidate_id(),
            lineage.credential_id(),
            lineage.credential_revision(),
            required_capability,
            &self.runtime_health,
            &self.runtime_quota,
            now_ms,
            is_candidate_eligible,
        )?;
        let started_at_ms = self.clock.now_ms().map_err(|_| internal_error())?;
        if retry_gate.is_cancelled() || !budget.can_start_at(started_at_ms) {
            drop(selection);
            return Err(if retry_gate.is_cancelled() {
                request_cancelled_error()
            } else {
                egress_unavailable_error()
            });
        }
        let remaining_bootstrap = budget.remaining_at(started_at_ms)?;
        budget.record_start();
        let attempt_number =
            u64::try_from(budget.attempts_started()).map_err(|_| internal_error())?;
        let attempt_result: Result<D::Output, AttemptFailure> = tokio::select! {
            biased;
            () = retry_gate.cancelled() => Err(AttemptFailure::Cancelled),
            result = tokio::time::timeout(
                driver.start_timeout(remaining_bootstrap),
                driver.start(
                    selection.candidate(),
                    selection.lease(),
                    remaining_bootstrap,
                ),
            ) => match result {
                Ok(result) => result,
                Err(_) => Err(AttemptFailure::Connection),
            },
        };

        match attempt_result {
            Ok(output) => {
                if retry_gate.is_cancelled() {
                    self.emit_pinned_attempt(
                        request_id,
                        route_id,
                        &selection,
                        attempt_number,
                        started_at_ms,
                        AttemptOutcome::Failed(request_cancelled_error()),
                        AttemptRetryDecision::Cancelled,
                        event_sink,
                    )?;
                    return Err(request_cancelled_error());
                }
                self.emit_pinned_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Succeeded,
                    AttemptRetryDecision::Completed,
                    event_sink,
                )?;
                Ok(StartedAttempt {
                    selection,
                    output,
                    attempts_started: budget.attempts_started(),
                })
            }
            Err(AttemptFailure::Cancelled) => {
                self.emit_pinned_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Failed(request_cancelled_error()),
                    AttemptRetryDecision::Cancelled,
                    event_sink,
                )?;
                Err(request_cancelled_error())
            }
            Err(failure) => {
                let safe_failure = failure.safe_error();
                if let Err(error) = self.record_runtime_state(&selection, &failure) {
                    self.emit_pinned_attempt(
                        request_id,
                        route_id,
                        &selection,
                        attempt_number,
                        started_at_ms,
                        AttemptOutcome::Failed(error.clone()),
                        AttemptRetryDecision::InfrastructureFailure,
                        event_sink,
                    )?;
                    return Err(error);
                }
                let retry_decision = if failure.is_retryable() {
                    AttemptRetryDecision::RetryClosed
                } else {
                    AttemptRetryDecision::NonRetryable
                };
                self.emit_pinned_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Failed(safe_failure.clone()),
                    retry_decision,
                    event_sink,
                )?;
                Err(safe_failure)
            }
        }
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // One retry state machine keeps lease, gate, and event ordering auditable.
    async fn start_inner<D>(
        &self,
        request_id: Option<&RequestId>,
        route_id: &RouteId,
        is_candidate_eligible: &(dyn Fn(&SnapshotRouteCandidate) -> bool + Sync),
        provider_scoped: bool,
        driver: &D,
        retry_gate: &dyn TransparentRetryGate,
        event_sink: &dyn GatewayEventSink,
    ) -> Result<StartedAttempt<D::Output>, GatewayError>
    where
        D: AttemptDriver,
    {
        if retry_gate.is_cancelled() {
            return Err(request_cancelled_error());
        }

        let route = self
            .scheduler
            .route(route_id)
            .ok_or_else(credential_unavailable_error)?;
        let provider_scope = if provider_scoped {
            Some(unique_provider_scope(&route, is_candidate_eligible)?)
        } else {
            None
        };
        let now_ms = self.clock.now_ms().map_err(|_| internal_error())?;
        let mut budget = RetryBudget::from_route(&route, now_ms)?;
        let mut exclusions = AttemptExclusionSet::new();
        let mut last_failure = None;

        loop {
            if retry_gate.is_cancelled() {
                return Err(request_cancelled_error());
            }

            let now_ms = self.clock.now_ms().map_err(|_| internal_error())?;
            if !budget.can_start_at(now_ms) {
                return Err(last_failure.unwrap_or_else(egress_unavailable_error));
            }

            let selection_result = if let Some(provider_id) = provider_scope.as_ref() {
                self.select_provider_scoped_and_lease(
                    route_id,
                    &route,
                    provider_id,
                    is_candidate_eligible,
                    &exclusions,
                    now_ms,
                )
            } else {
                self.scheduler
                    .select_eligible_and_lease_with_runtime_health_quota_and_binding_at(
                        route_id,
                        &self.runtime_health,
                        &self.runtime_quota,
                        now_ms,
                        is_candidate_eligible,
                        |candidate, credential_id| !exclusions.contains(candidate, credential_id),
                    )
            };
            let (selection, quota_probe) = match selection_result {
                Ok(selection) => (selection, None),
                Err(error) => {
                    let recovery = budget.remaining_at(now_ms).ok().and_then(|remaining| {
                        self.begin_quota_recovery_probe_selection(
                            route_id,
                            provider_scope.as_ref(),
                            is_candidate_eligible,
                            &exclusions,
                            now_ms,
                            driver.start_timeout(remaining),
                        )
                    });
                    match recovery {
                        Some((selection, probe)) => (selection, Some(probe)),
                        None => return Err(last_failure.unwrap_or(error)),
                    }
                }
            };

            let started_at_ms = self.clock.now_ms().map_err(|_| internal_error())?;
            if !budget.can_start_at(started_at_ms) {
                drop(selection);
                return Err(last_failure.unwrap_or_else(egress_unavailable_error));
            }
            let remaining_bootstrap = budget.remaining_at(started_at_ms)?;
            budget.record_start();
            let attempt_number =
                u64::try_from(budget.attempts_started()).map_err(|_| internal_error())?;

            let attempt_result: Result<D::Output, AttemptFailure> = tokio::select! {
                biased;
                () = retry_gate.cancelled() => Err(AttemptFailure::Cancelled),
                result = tokio::time::timeout(
                    driver.start_timeout(remaining_bootstrap),
                    driver.start(
                        selection.candidate(),
                        selection.lease(),
                        remaining_bootstrap,
                    ),
                ) => match result {
                    Ok(result) => result,
                    Err(_) => Err(AttemptFailure::Connection),
                },
            };

            let failure = match attempt_result {
                Ok(output) => {
                    if let Some(probe) = quota_probe {
                        self.complete_quota_recovery_probe(probe);
                    }
                    if retry_gate.is_cancelled() {
                        self.emit_attempt(
                            request_id,
                            route_id,
                            &selection,
                            attempt_number,
                            started_at_ms,
                            AttemptOutcome::Failed(request_cancelled_error()),
                            AttemptRetryDecision::Cancelled,
                            event_sink,
                        );
                        return Err(request_cancelled_error());
                    }
                    self.emit_attempt(
                        request_id,
                        route_id,
                        &selection,
                        attempt_number,
                        started_at_ms,
                        AttemptOutcome::Succeeded,
                        AttemptRetryDecision::Completed,
                        event_sink,
                    );
                    return Ok(StartedAttempt {
                        selection,
                        output,
                        attempts_started: budget.attempts_started(),
                    });
                }
                Err(failure) => failure,
            };

            if matches!(failure, AttemptFailure::Cancelled) {
                self.emit_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Failed(request_cancelled_error()),
                    AttemptRetryDecision::Cancelled,
                    event_sink,
                );
                return Err(request_cancelled_error());
            }
            let safe_failure = failure.safe_error();
            if let Err(error) = self.record_runtime_state(&selection, &failure) {
                self.emit_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Failed(error.clone()),
                    AttemptRetryDecision::InfrastructureFailure,
                    event_sink,
                );
                return Err(error);
            }
            if !failure.is_retryable() {
                if retry_gate.is_cancelled() {
                    self.emit_attempt(
                        request_id,
                        route_id,
                        &selection,
                        attempt_number,
                        started_at_ms,
                        AttemptOutcome::Failed(safe_failure),
                        AttemptRetryDecision::Cancelled,
                        event_sink,
                    );
                    return Err(request_cancelled_error());
                }
                self.emit_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Failed(safe_failure.clone()),
                    AttemptRetryDecision::NonRetryable,
                    event_sink,
                );
                return Err(safe_failure);
            }

            exclusions.insert(selection.candidate(), selection.lease().credential_id());
            if retry_gate.is_cancelled() {
                self.emit_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Failed(safe_failure),
                    AttemptRetryDecision::Cancelled,
                    event_sink,
                );
                return Err(request_cancelled_error());
            }
            if !retry_gate.allows_transparent_retry() {
                self.emit_attempt(
                    request_id,
                    route_id,
                    &selection,
                    attempt_number,
                    started_at_ms,
                    AttemptOutcome::Failed(safe_failure.clone()),
                    AttemptRetryDecision::RetryClosed,
                    event_sink,
                );
                return Err(safe_failure);
            }
            self.emit_attempt(
                request_id,
                route_id,
                &selection,
                attempt_number,
                started_at_ms,
                AttemptOutcome::Failed(safe_failure.clone()),
                AttemptRetryDecision::RetryEligible,
                event_sink,
            );
            last_failure = Some(safe_failure);
            drop(selection);
        }
    }

    fn select_provider_scoped_and_lease(
        &self,
        route_id: &RouteId,
        route: &SnapshotRoute,
        provider_id: &ProviderId,
        is_candidate_eligible: &(dyn Fn(&SnapshotRouteCandidate) -> bool + Sync),
        exclusions: &AttemptExclusionSet,
        observed_at_ms: i64,
    ) -> Result<SelectedRouteCredential, GatewayError> {
        let admitted_candidate_ids = route
            .candidates()
            .iter()
            .filter(|candidate| candidate.is_hard_eligible() && is_candidate_eligible(candidate))
            .map(|candidate| candidate.id().clone())
            .collect::<BTreeSet<_>>();
        let candidate_price_rates = admitted_candidate_ids
            .iter()
            .filter_map(|candidate_id| {
                self.scheduler
                    .provider_price_rates_for_candidate(candidate_id)
                    .map(|rates| (candidate_id.clone(), rates))
            })
            .collect::<BTreeMap<_, _>>();
        let input = ProviderScopedRouteExplainInput::try_new(
            RouteExplainInput::new(route_id.clone(), observed_at_ms),
            provider_id.clone(),
            admitted_candidate_ids,
            candidate_price_rates,
        )
        .map_err(|_| credential_unavailable_error())?;
        let explain = self
            .scheduler
            .explain_provider_scoped(
                &input,
                &self.runtime_health,
                &self.runtime_quota,
                exclusions,
            )
            .map_err(|_| credential_unavailable_error())?;
        let lease_observed_at_ms = self.clock.now_ms().map_err(|_| internal_error())?;
        self.scheduler.select_provider_scoped_and_lease_at(
            route_id,
            explain.provider_selection(),
            &self.runtime_health,
            &self.runtime_quota,
            lease_observed_at_ms,
            is_candidate_eligible,
            |candidate, credential_id| !exclusions.contains(candidate, credential_id),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_attempt(
        &self,
        request_id: Option<&RequestId>,
        route_id: &RouteId,
        selection: &SelectedRouteCredential,
        attempt_number: u64,
        started_at_ms: i64,
        outcome: AttemptOutcome,
        retry_decision: AttemptRetryDecision,
        event_sink: &dyn GatewayEventSink,
    ) -> EventEmission {
        let Some(request_id) = request_id else {
            return EventEmission::Disabled;
        };
        let ended_at_ms = match self.clock.now_ms() {
            Ok(ended_at_ms) => ended_at_ms,
            Err(_) => started_at_ms,
        };
        let event = AttemptEvent::new(
            request_id.clone(),
            attempt_number,
            route_id.clone(),
            selection.candidate().id().clone(),
            selection.lease().credential_id().clone(),
            selection.candidate().endpoint_id().clone(),
            selection.candidate().upstream_id().clone(),
            selection.candidate().upstream_model().to_owned(),
            started_at_ms,
            ended_at_ms,
            outcome,
            retry_decision,
        );
        event_sink.try_emit(GatewayEvent::Attempt(event))
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_pinned_attempt(
        &self,
        request_id: &RequestId,
        route_id: &RouteId,
        selection: &SelectedRouteCredential,
        attempt_number: u64,
        started_at_ms: i64,
        outcome: AttemptOutcome,
        retry_decision: AttemptRetryDecision,
        event_sink: &dyn GatewayEventSink,
    ) -> Result<(), GatewayError> {
        match self.emit_attempt(
            Some(request_id),
            route_id,
            selection,
            attempt_number,
            started_at_ms,
            outcome,
            retry_decision,
            event_sink,
        ) {
            EventEmission::RequiredQueueFull | EventEmission::SinkClosed => Err(internal_error()),
            EventEmission::Enqueued
            | EventEmission::Disabled
            | EventEmission::DiagnosticDropped => Ok(()),
        }
    }

    fn record_runtime_state(
        &self,
        selection: &SelectedRouteCredential,
        failure: &AttemptFailure,
    ) -> Result<(), GatewayError> {
        if matches!(
            failure,
            AttemptFailure::NonRetryable(error)
                if error.code() == GatewayErrorCode::CredentialUnauthorized
        ) {
            self.runtime_health
                .mark_credential_unauthorized(
                    selection.candidate().endpoint_id().clone(),
                    selection.lease().credential_id().clone(),
                )
                .map_err(|_| internal_error())?;
            return Ok(());
        }
        if matches!(
            failure,
            AttemptFailure::NonRetryable(error)
                if error.code() == GatewayErrorCode::CredentialForbidden
        ) {
            self.runtime_health
                .mark_credential_forbidden(
                    selection.candidate().endpoint_id().clone(),
                    selection.lease().credential_id().clone(),
                )
                .map_err(|_| internal_error())?;
            return Ok(());
        }
        if let AttemptFailure::RateLimited { retry_after } = failure {
            let now_ms = self.clock.now_ms().map_err(|_| internal_error())?;
            self.runtime_quota
                .record_rate_limited(
                    RuntimeQuotaTarget::endpoint_credential(
                        selection.candidate().endpoint_id().clone(),
                        selection.lease().credential_id().clone(),
                    ),
                    now_ms,
                    *retry_after,
                    self.config.rate_limit_fallback_cooldown(),
                )
                .map_err(|_| internal_error())?;
            return Ok(());
        }
        let Some(cooldown) = failure.cooldown(self.config) else {
            return Ok(());
        };
        let now_ms = self.clock.now_ms().map_err(|_| internal_error())?;
        let until_ms = add_duration_to_timestamp(now_ms, cooldown.duration())?;
        let key = match cooldown {
            CooldownScope::Endpoint(_) => {
                RuntimeHealthKey::endpoint(selection.candidate().endpoint_id().clone())
            }
        };
        self.runtime_health
            .cool_down_until(key, until_ms)
            .map_err(|_| internal_error())
    }

    /// Attempts to admit one controlled quota-recovery probe after ordinary selection failed.
    ///
    /// The scheduler admits a binding only when Health passes and the binding-wide quota is past
    /// its Reset (`RecoveryRequired`). Beginning the registry ticket is the final atomic gate:
    /// exactly one caller can hold it, so concurrent failed selections cannot start a second
    /// probe. Any clock, scheduling, or registry failure fails closed by admitting nothing.
    fn begin_quota_recovery_probe_selection(
        &self,
        route_id: &RouteId,
        provider_scope: Option<&ProviderId>,
        is_candidate_eligible: &(dyn Fn(&SnapshotRouteCandidate) -> bool + Sync),
        exclusions: &AttemptExclusionSet,
        now_ms: i64,
        start_ceiling: Duration,
    ) -> Option<(SelectedRouteCredential, RuntimeQuotaRecoveryProbe)> {
        let selection = self
            .scheduler
            .select_eligible_and_lease_for_quota_recovery_at(
                route_id,
                &self.runtime_health,
                &self.runtime_quota,
                now_ms,
                |candidate| {
                    is_candidate_eligible(candidate)
                        && provider_scope.is_none_or(|provider_id| {
                            candidate.is_hard_eligible()
                                && ProviderId::try_new(candidate.upstream_id().as_str().to_owned())
                                    .is_ok_and(|candidate_provider| {
                                        &candidate_provider == provider_id
                                    })
                        })
                },
                |candidate, credential_id| !exclusions.contains(candidate, credential_id),
            )
            .ok()?;
        let target = RuntimeQuotaTarget::endpoint_credential(
            selection.candidate().endpoint_id().clone(),
            selection.lease().credential_id().clone(),
        );
        let expires_at_ms = add_duration_to_timestamp(
            now_ms,
            start_ceiling.saturating_add(QUOTA_RECOVERY_PROBE_GRACE),
        )
        .ok()?;
        let probe = self
            .runtime_quota
            .begin_recovery_probe(&target, expires_at_ms)
            .ok()??;
        Some((selection, probe))
    }

    /// Reopens ordinary scheduling for one successfully probed quota target.
    ///
    /// The registry validates the ticket: a stale or superseded probe fails closed and leaves the
    /// target blocked until its next due probe, which is safer than forcing availability here.
    fn complete_quota_recovery_probe(&self, probe: RuntimeQuotaRecoveryProbe) {
        let Ok(observed_at_ms) = self.clock.now_ms() else {
            return;
        };
        let Ok(snapshot) = QuotaSnapshot::try_new(
            probe.target().clone(),
            Vec::new(),
            QuotaSource::Estimated,
            QuotaConfidence::Estimated,
            observed_at_ms,
        ) else {
            return;
        };
        let _completion = self.runtime_quota.complete_recovery_probe(probe, snapshot);
    }
}

impl fmt::Debug for AttemptOrchestrator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AttemptOrchestrator")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
enum CooldownScope {
    Endpoint(Duration),
}

impl CooldownScope {
    const fn duration(self) -> Duration {
        match self {
            Self::Endpoint(duration) => duration,
        }
    }
}

struct RetryBudget {
    max_attempts: usize,
    attempts_started: usize,
    deadline_ms: i64,
}

impl RetryBudget {
    fn from_route(route: &crate::SnapshotRoute, now_ms: i64) -> Result<Self, GatewayError> {
        let max_attempts = usize::try_from(route.max_attempts()).map_err(|_| internal_error())?;
        let bootstrap_timeout_ms =
            u64::try_from(route.bootstrap_timeout_ms()).map_err(|_| internal_error())?;
        let timeout_ms = i64::try_from(bootstrap_timeout_ms).map_err(|_| internal_error())?;
        let deadline_ms = now_ms.checked_add(timeout_ms).ok_or_else(internal_error)?;
        Ok(Self {
            max_attempts,
            attempts_started: 0,
            deadline_ms,
        })
    }

    const fn can_start_at(&self, now_ms: i64) -> bool {
        self.attempts_started < self.max_attempts && now_ms < self.deadline_ms
    }

    fn remaining_at(&self, now_ms: i64) -> Result<Duration, GatewayError> {
        let remaining_ms = self
            .deadline_ms
            .checked_sub(now_ms)
            .filter(|milliseconds| *milliseconds > 0)
            .ok_or_else(egress_unavailable_error)?;
        let remaining_ms = u64::try_from(remaining_ms).map_err(|_| internal_error())?;
        Ok(Duration::from_millis(remaining_ms))
    }

    fn record_start(&mut self) {
        self.attempts_started = self.attempts_started.saturating_add(1);
    }

    const fn attempts_started(&self) -> usize {
        self.attempts_started
    }
}

fn add_duration_to_timestamp(now_ms: i64, duration: Duration) -> Result<i64, GatewayError> {
    let duration_ms = i64::try_from(duration.as_millis()).map_err(|_| internal_error())?;
    now_ms.checked_add(duration_ms).ok_or_else(internal_error)
}

fn candidate_matches_protocol(
    candidate: &SnapshotRouteCandidate,
    protocol: Option<ProtocolFormat>,
) -> bool {
    protocol.is_none_or(|protocol| {
        candidate.protocol_format() == Some(protocol)
            && matches!(
                candidate.transform_mode(),
                crate::SnapshotTransformMode::Canonical
                    | crate::SnapshotTransformMode::CanonicalBridge
            )
    })
}

fn unique_provider_scope(
    route: &SnapshotRoute,
    is_candidate_eligible: &(dyn Fn(&SnapshotRouteCandidate) -> bool + Sync),
) -> Result<ProviderId, GatewayError> {
    let providers = route
        .candidates()
        .iter()
        .filter(|candidate| candidate.is_hard_eligible() && is_candidate_eligible(candidate))
        .map(|candidate| {
            ProviderId::try_new(candidate.upstream_id().as_str().to_owned())
                .map_err(|_| credential_unavailable_error())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut providers = providers.into_iter();
    let Some(provider_id) = providers.next() else {
        return Err(credential_unavailable_error());
    };
    if providers.next().is_some() {
        return Err(credential_unavailable_error());
    }
    Ok(provider_id)
}

const fn credential_unavailable_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::CredentialUnavailable,
        ErrorScope::Credential,
    )
}

const fn egress_unavailable_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressUnavailable, ErrorScope::Egress)
}

const fn provider_rate_limited_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ProviderRateLimited, ErrorScope::Provider)
}

const fn provider_transient_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider)
}

const fn stream_truncated_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::StreamTruncated, ErrorScope::Stream)
}

const fn request_cancelled_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::Cancelled, ErrorScope::Request)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        error::Error,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicI64, Ordering},
        },
        time::Duration,
    };

    use gateway_catalog::{CapabilitySet, CatalogModelState, SemanticCapability};
    use gateway_core::{
        AttemptOutcome, AttemptRetryDecision, CredentialId, EndpointId, ErrorScope, EventEmission,
        GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink, ProviderId, PublicModelId,
        RequestId, RouteCandidateId, RouteId, TransparentRetryGate, TransparentRetryGateFuture,
        UpstreamId,
    };
    use gateway_upstream::{
        CredentialSecret, EndpointCredentialInput, EndpointCredentialPool, EndpointCredentialPools,
    };
    use tokio::sync::Notify;

    use super::{
        AttemptDriver, AttemptExclusionSet, AttemptFailure, AttemptFuture, AttemptOrchestrator,
        AttemptOrchestratorConfig, NoopGatewayEventSink,
    };
    use crate::{
        ProtocolFormat, ResponsesContinuationKind, ResponsesContinuationPin,
        ResponsesExecutionLineage, RouteCredentialScheduler, RouteSnapshot, RouteSnapshotInput,
        RuntimeCredentialAccountStatus, RuntimeHealthAccountRecoveryResult, RuntimeHealthClock,
        RuntimeHealthClockError, RuntimeHealthRegistry, SnapshotCatalogAdmission,
        SnapshotPublicModel, SnapshotRoute, SnapshotRouteCandidate, SnapshotRouteCandidateInput,
        SnapshotRoutePolicy, SnapshotTransformMode, SnapshotVersion,
    };

    type TestResult = Result<(), Box<dyn Error>>;
    type CandidateSpec<'a> = (&'a str, &'a str);
    type CredentialSpec<'a> = (&'a str, Vec<&'a str>);
    type OrchestratorFixture = (
        AttemptOrchestrator,
        RouteId,
        Arc<FixedRuntimeHealthClock>,
        Arc<RuntimeHealthRegistry>,
        Arc<EndpointCredentialPools>,
    );
    type ProtocolIsolationFixture = (
        AttemptOrchestrator,
        RouteId,
        Arc<RuntimeHealthRegistry>,
        Arc<EndpointCredentialPools>,
    );

    #[tokio::test]
    async fn connection_failure_cools_the_endpoint_and_falls_back_to_another_candidate()
    -> TestResult {
        let (orchestrator, route_id, _clock, health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a"), ("candidate-b", "endpoint-b")],
            vec![
                ("endpoint-a", vec!["credential-a"]),
                ("endpoint-b", vec!["credential-b"]),
            ],
            3,
            100,
        )?;
        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::Connection),
            DriverStep::Success("connected".to_owned()),
        ]);
        let gate = TestRetryGate::default();

        let started = orchestrator.start(&route_id, &driver, &gate).await?;

        assert_eq!(started.candidate().id().as_str(), "candidate-b");
        assert_eq!(started.lease().credential_id().as_str(), "credential-b");
        assert_eq!(started.output(), "connected");
        assert_eq!(started.attempts_started(), 2);
        assert_eq!(
            driver.attempts()?,
            vec![
                ("candidate-a".to_owned(), "credential-a".to_owned()),
                ("candidate-b".to_owned(), "credential-b".to_owned())
            ]
        );
        assert!(!health.endpoint_is_available(&EndpointId::try_new("endpoint-a")?));
        assert!(health.endpoint_is_available(&EndpointId::try_new("endpoint-b")?));
        Ok(())
    }

    #[tokio::test]
    async fn same_upstream_protocol_failure_is_isolated_to_its_declared_endpoint() -> TestResult {
        let (orchestrator, route_id, health, _pools) = protocol_isolation_orchestrator()?;
        let responses_driver =
            ScriptedDriver::new(vec![DriverStep::Failure(AttemptFailure::Connection)]);

        let responses_error = expected_error(
            orchestrator
                .start_for_protocol(
                    &route_id,
                    ProtocolFormat::OpenAiResponses,
                    &responses_driver,
                    &TestRetryGate::default(),
                )
                .await,
            "Responses-only endpoint failure must remain an explicit safe failure",
        )?;
        assert_eq!(responses_error.code(), GatewayErrorCode::EgressUnavailable);
        assert_eq!(
            responses_driver.attempts()?,
            vec![(
                "candidate-responses".to_owned(),
                "credential-responses".to_owned()
            )]
        );

        let responses_endpoint = EndpointId::try_new("endpoint-responses")?;
        let anthropic_endpoint = EndpointId::try_new("endpoint-anthropic")?;
        assert!(!health.endpoint_is_available(&responses_endpoint));
        assert!(health.endpoint_is_available(&anthropic_endpoint));

        let anthropic_driver = ScriptedDriver::new(vec![DriverStep::Success(
            "anthropic-still-healthy".to_owned(),
        )]);
        let started = orchestrator
            .start_for_protocol(
                &route_id,
                ProtocolFormat::AnthropicMessages,
                &anthropic_driver,
                &TestRetryGate::default(),
            )
            .await?;
        assert_eq!(started.candidate().id().as_str(), "candidate-anthropic");
        assert_eq!(
            started.candidate().upstream_id().as_str(),
            "upstream-shared"
        );
        assert_eq!(
            started.candidate().endpoint_api_format(),
            "anthropic/messages"
        );
        assert_eq!(started.output(), "anthropic-still-healthy");
        assert_eq!(
            anthropic_driver.attempts()?,
            vec![(
                "candidate-anthropic".to_owned(),
                "credential-anthropic".to_owned()
            )]
        );
        Ok(())
    }

    #[tokio::test]
    async fn provider_scoped_retry_stays_within_one_provider() -> TestResult {
        let (orchestrator, route_id, _health, _pools) = protocol_isolation_orchestrator()?;
        let request_id = RequestId::try_new("provider-scoped-retry")?;
        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::Connection),
            DriverStep::Success("same-provider-sibling".to_owned()),
        ]);

        let started = orchestrator
            .start_with_event_sink_provider_scoped_matching(
                &request_id,
                &route_id,
                |_| true,
                &driver,
                &TestRetryGate::default(),
                &NoopGatewayEventSink,
            )
            .await?;
        assert_eq!(started.output(), "same-provider-sibling");
        assert_eq!(started.attempts_started(), 2);
        assert_eq!(
            driver.attempts()?,
            vec![
                (
                    "candidate-anthropic".to_owned(),
                    "credential-anthropic".to_owned()
                ),
                (
                    "candidate-responses".to_owned(),
                    "credential-responses".to_owned()
                )
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn provider_scoped_entrypoint_fails_closed_for_ambiguous_routes() -> TestResult {
        let (orchestrator, route_id, _clock, _health, pools) = orchestrator(
            vec![("candidate-a", "endpoint-a"), ("candidate-b", "endpoint-b")],
            vec![
                ("endpoint-a", vec!["credential-a"]),
                ("endpoint-b", vec!["credential-b"]),
            ],
            3,
            100,
        )?;
        let request_id = RequestId::try_new("provider-scoped-ambiguous")?;
        let driver = ScriptedDriver::new(vec![DriverStep::Success("must-not-start".to_owned())]);

        let error = expected_error(
            orchestrator
                .start_with_event_sink_provider_scoped_matching(
                    &request_id,
                    &route_id,
                    |_| true,
                    &driver,
                    &TestRetryGate::default(),
                    &NoopGatewayEventSink,
                )
                .await,
            "provider-scoped serving must reject an ambiguous route before leasing",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::CredentialUnavailable);
        assert!(driver.attempts()?.is_empty());
        for (endpoint, credential) in [
            ("endpoint-a", "credential-a"),
            ("endpoint-b", "credential-b"),
        ] {
            let pool = pools
                .pool(&EndpointId::try_new(endpoint)?)
                .ok_or("missing ambiguous-route pool")?;
            assert_eq!(
                pool.active_lease_count(&CredentialId::try_new(credential)?),
                Some(0)
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn pinned_once_invokes_exact_binding_once_and_closes_retry() -> TestResult {
        let (orchestrator, route_id, _clock, _health, pools) = orchestrator(
            vec![("candidate-a", "endpoint-a"), ("candidate-b", "endpoint-b")],
            vec![
                ("endpoint-a", vec!["credential-a"]),
                ("endpoint-b", vec!["credential-b"]),
            ],
            3,
            100,
        )?;
        let request_id = RequestId::try_new("pinned-once")?;
        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::Connection),
            DriverStep::Success("must-not-fallback".to_owned()),
        ]);
        let events = RecordingEventSink::default();
        let error = expected_error(
            orchestrator
                .start_pinned_once_with_event_sink(
                    &request_id,
                    &route_id,
                    &ProviderId::try_new("upstream-endpoint-a")?,
                    &EndpointId::try_new("endpoint-a")?,
                    &CredentialId::try_new("credential-a")?,
                    |_| true,
                    &driver,
                    &TestRetryGate::default(),
                    &events,
                )
                .await,
            "pinned diagnostic must stop after its first failure",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::EgressUnavailable);
        assert_eq!(
            driver.attempts()?,
            vec![("candidate-a".to_owned(), "credential-a".to_owned())]
        );
        let events = events.events()?;
        assert_eq!(events.len(), 1);
        let GatewayEvent::Attempt(attempt) = &events[0] else {
            return Err("expected one Attempt event".into());
        };
        assert_eq!(attempt.retry_decision(), AttemptRetryDecision::RetryClosed);
        assert_eq!(attempt.credential_id().as_str(), "credential-a");
        assert_eq!(
            pools
                .pool(&EndpointId::try_new("endpoint-a")?)
                .and_then(|pool| {
                    pool.active_lease_count(&CredentialId::try_new("credential-a").ok()?)
                }),
            Some(0)
        );
        assert_eq!(
            driver.attempts()?.len(),
            1,
            "the queued sibling step must not be consumed"
        );
        Ok(())
    }

    #[tokio::test]
    async fn pinned_once_keeps_success_lease_until_started_output_is_dropped() -> TestResult {
        let (orchestrator, route_id, _clock, _health, pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            1,
            100,
        )?;
        let driver = ScriptedDriver::new(vec![DriverStep::Success("pinned".to_owned())]);
        let started = orchestrator
            .start_pinned_once_with_event_sink(
                &RequestId::try_new("pinned-success")?,
                &route_id,
                &ProviderId::try_new("upstream-endpoint-a")?,
                &EndpointId::try_new("endpoint-a")?,
                &CredentialId::try_new("credential-a")?,
                |_| true,
                &driver,
                &TestRetryGate::default(),
                &NoopGatewayEventSink,
            )
            .await?;
        assert_eq!(started.output(), "pinned");
        assert_eq!(started.attempts_started(), 1);
        assert_eq!(
            pools
                .pool(&EndpointId::try_new("endpoint-a")?)
                .and_then(|pool| {
                    pool.active_lease_count(&CredentialId::try_new("credential-a").ok()?)
                }),
            Some(1)
        );
        drop(started);
        assert_eq!(
            pools
                .pool(&EndpointId::try_new("endpoint-a")?)
                .and_then(|pool| {
                    pool.active_lease_count(&CredentialId::try_new("credential-a").ok()?)
                }),
            Some(0)
        );
        Ok(())
    }

    #[tokio::test]
    async fn continuation_once_keeps_exact_revision_and_never_falls_back() -> TestResult {
        let (orchestrator, route_id, _clock, _health, pools) = continuation_orchestrator()?;
        let pin = continuation_pin(ResponsesContinuationKind::StoredResponse)?;
        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::Connection),
            DriverStep::Success("must-not-fallback".to_owned()),
        ]);
        let events = RecordingEventSink::default();
        let error = expected_error(
            orchestrator
                .start_continuation_once_with_event_sink(
                    &RequestId::try_new("continuation-failure")?,
                    &pin,
                    |_| true,
                    &driver,
                    &TestRetryGate::default(),
                    &events,
                )
                .await,
            "stored continuation must stop after its exact Attempt fails",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::EgressUnavailable);
        assert_eq!(
            driver.attempts()?,
            vec![(
                "candidate-continuity".to_owned(),
                "credential-continuity".to_owned()
            )]
        );
        let events = events.events()?;
        assert_eq!(events.len(), 1);
        let GatewayEvent::Attempt(attempt) = &events[0] else {
            return Err("expected one stored-continuation Attempt event".into());
        };
        assert_eq!(attempt.retry_decision(), AttemptRetryDecision::RetryClosed);
        assert_eq!(attempt.credential_id().as_str(), "credential-continuity");
        assert_eq!(attempt.route_id(), &route_id);
        assert_eq!(
            pools
                .pool(&EndpointId::try_new("endpoint-continuity")?)
                .and_then(|pool| pool
                    .active_lease_count(&CredentialId::try_new("credential-continuity").ok()?)),
            Some(0)
        );
        Ok(())
    }

    #[tokio::test]
    async fn compaction_continuation_holds_and_releases_only_the_exact_lease() -> TestResult {
        let (orchestrator, _route_id, _clock, _health, pools) = continuation_orchestrator()?;
        let driver = ScriptedDriver::new(vec![DriverStep::Success("compact".to_owned())]);
        let started = orchestrator
            .start_continuation_once_with_event_sink(
                &RequestId::try_new("compaction-success")?,
                &continuation_pin(ResponsesContinuationKind::Compaction)?,
                |_| true,
                &driver,
                &TestRetryGate::default(),
                &NoopGatewayEventSink,
            )
            .await?;
        assert_eq!(started.output(), "compact");
        assert_eq!(started.attempts_started(), 1);
        assert_eq!(
            pools
                .pool(&EndpointId::try_new("endpoint-continuity")?)
                .and_then(|pool| pool
                    .active_lease_count(&CredentialId::try_new("credential-continuity").ok()?)),
            Some(1)
        );
        drop(started);
        assert_eq!(
            pools
                .pool(&EndpointId::try_new("endpoint-continuity")?)
                .and_then(|pool| pool
                    .active_lease_count(&CredentialId::try_new("credential-continuity").ok()?)),
            Some(0)
        );
        Ok(())
    }

    #[tokio::test]
    async fn provider_scoped_quota_recovery_never_leases_a_foreign_hard_ineligible_candidate()
    -> TestResult {
        let route_id = RouteId::try_new("route-provider-recovery")?;
        let public_model_id = PublicModelId::try_new("public-model-provider-recovery")?;
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new("version-provider-recovery")?,
            vec![SnapshotPublicModel::new(
                public_model_id.clone(),
                "provider-recovery".to_owned(),
                "Provider Recovery".to_owned(),
                CapabilitySet::empty(),
                route_id.clone(),
            )],
            Vec::new(),
            vec![SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                SnapshotRoutePolicy::RoundRobin,
                2,
                100,
                vec![
                    candidate("candidate-a", "endpoint-a")?,
                    candidate_with_catalog_state(
                        "candidate-b",
                        "endpoint-b",
                        CatalogModelState::Expired,
                    )?,
                ],
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let pools = Arc::new(EndpointCredentialPools::try_new(vec![
            endpoint_pool("endpoint-a", vec!["credential-a"])?,
            endpoint_pool("endpoint-b", vec!["credential-b"])?,
        ])?);
        let scheduler = Arc::new(RouteCredentialScheduler::new(snapshot, Arc::clone(&pools)));
        let clock = Arc::new(FixedRuntimeHealthClock::new(100));
        let health_clock: Arc<dyn RuntimeHealthClock> = clock.clone();
        let health = Arc::new(RuntimeHealthRegistry::with_clock(Arc::clone(&health_clock)));
        let orchestrator = AttemptOrchestrator::with_clock_and_config(
            scheduler,
            Arc::clone(&health),
            health_clock,
            AttemptOrchestratorConfig::try_new(
                Duration::from_millis(20),
                Duration::from_millis(10),
            )?,
        );
        health.cool_down_until(
            crate::RuntimeHealthKey::endpoint(EndpointId::try_new("endpoint-a")?),
            200,
        )?;
        let foreign_target = crate::RuntimeQuotaTarget::endpoint_credential(
            EndpointId::try_new("endpoint-b")?,
            CredentialId::try_new("credential-b")?,
        );
        orchestrator.runtime_quota.record_rate_limited(
            foreign_target,
            100,
            Some(Duration::from_millis(20)),
            Duration::from_millis(20),
        )?;
        clock.advance_ms(20);
        let driver = ScriptedDriver::new(vec![DriverStep::Success("must-not-start".to_owned())]);

        let error = expected_error(
            orchestrator
                .start_with_event_sink_provider_scoped_matching(
                    &RequestId::try_new("provider-scoped-recovery")?,
                    &route_id,
                    |_| true,
                    &driver,
                    &TestRetryGate::default(),
                    &NoopGatewayEventSink,
                )
                .await,
            "quota recovery crossed the inferred Provider scope",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::CredentialUnavailable);
        assert!(driver.attempts()?.is_empty());
        for (endpoint, credential) in [
            ("endpoint-a", "credential-a"),
            ("endpoint-b", "credential-b"),
        ] {
            assert_eq!(
                pools
                    .pool(&EndpointId::try_new(endpoint)?)
                    .and_then(|pool| {
                        pool.active_lease_count(&CredentialId::try_new(credential).ok()?)
                    }),
                Some(0)
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn provider_scoped_quota_recovery_keeps_the_due_probe_inside_its_provider() -> TestResult
    {
        let (orchestrator, route_id, clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            2,
            100,
        )?;
        let target = crate::RuntimeQuotaTarget::endpoint_credential(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new("credential-a")?,
        );
        orchestrator.runtime_quota.record_rate_limited(
            target.clone(),
            100,
            Some(Duration::from_millis(20)),
            Duration::from_millis(20),
        )?;
        clock.advance_ms(20);
        let driver = ScriptedDriver::new(vec![DriverStep::Success("recovered".to_owned())]);

        let started = orchestrator
            .start_with_event_sink_provider_scoped_matching(
                &RequestId::try_new("provider-scoped-recovery-success")?,
                &route_id,
                |_| true,
                &driver,
                &TestRetryGate::default(),
                &NoopGatewayEventSink,
            )
            .await?;
        assert_eq!(started.output(), "recovered");
        assert_eq!(started.attempts_started(), 1);
        assert_eq!(driver.attempts()?.len(), 1);
        assert_eq!(
            orchestrator.runtime_quota.availability(&target)?,
            crate::RuntimeQuotaAvailability::Available
        );
        Ok(())
    }

    #[tokio::test]
    async fn protocol_filtered_start_rejects_noncanonical_candidates_before_a_lease() -> TestResult
    {
        let (orchestrator, route_id, _health, pools) =
            single_protocol_orchestrator(SnapshotTransformMode::LosslessBridge)?;
        let driver = ScriptedDriver::new(vec![DriverStep::Success("must-not-start".to_owned())]);

        let error = expected_error(
            orchestrator
                .start_for_protocol(
                    &route_id,
                    ProtocolFormat::OpenAiResponses,
                    &driver,
                    &TestRetryGate::default(),
                )
                .await,
            "same-protocol lossless bridge must not bypass P5-04 admission",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::CredentialUnavailable);
        assert!(driver.attempts()?.is_empty());
        let endpoint = EndpointId::try_new("endpoint-noncanonical")?;
        let credential = CredentialId::try_new("credential-noncanonical")?;
        let pool = pools
            .pool(&endpoint)
            .ok_or("missing non-Canonical test pool")?;
        assert_eq!(pool.active_lease_count(&credential), Some(0));
        Ok(())
    }

    #[tokio::test]
    async fn request_local_candidate_rejection_takes_no_lease_and_starts_no_attempt() -> TestResult
    {
        let (orchestrator, route_id, _health, pools) =
            single_protocol_orchestrator(SnapshotTransformMode::Canonical)?;
        let driver = ScriptedDriver::new(vec![DriverStep::Success("must-not-start".to_owned())]);
        let request_id = RequestId::try_new("request-local-rejection")?;

        let error = expected_error(
            orchestrator
                .start_with_event_sink_matching(
                    &request_id,
                    &route_id,
                    |_candidate| false,
                    &driver,
                    &TestRetryGate::default(),
                    &NoopGatewayEventSink,
                )
                .await,
            "a rejected transform must not reach the driver",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::CredentialUnavailable);
        assert!(driver.attempts()?.is_empty());
        let pool = pools
            .pool(&EndpointId::try_new("endpoint-noncanonical")?)
            .ok_or("missing test pool")?;
        assert_eq!(
            pool.active_lease_count(&CredentialId::try_new("credential-noncanonical")?),
            Some(0),
        );
        Ok(())
    }

    #[tokio::test]
    async fn observed_attempts_emit_one_terminal_record_per_driver_invocation() -> TestResult {
        let (orchestrator, route_id, _clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a"), ("candidate-b", "endpoint-b")],
            vec![
                ("endpoint-a", vec!["credential-a"]),
                ("endpoint-b", vec!["credential-b"]),
            ],
            3,
            100,
        )?;
        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::Connection),
            DriverStep::Success("connected".to_owned()),
        ]);
        let gate = TestRetryGate::default();
        let events = RecordingEventSink::default();
        let request_id = RequestId::try_new("request-observed")?;

        let started = orchestrator
            .start_with_event_sink(&request_id, &route_id, &driver, &gate, &events)
            .await?;

        assert_eq!(started.attempts_started(), 2);
        let events = events.events()?;
        assert_eq!(events.len(), 2);
        let [GatewayEvent::Attempt(first), GatewayEvent::Attempt(second)] = events.as_slice()
        else {
            return Err("expected exactly two Attempt observations".into());
        };
        assert_eq!(first.request_id(), &request_id);
        assert_eq!(first.attempt_number(), 1);
        assert_eq!(first.route_candidate_id().as_str(), "candidate-a");
        assert_eq!(first.credential_id().as_str(), "credential-a");
        assert!(matches!(
            first.outcome(),
            AttemptOutcome::Failed(error) if error.code() == GatewayErrorCode::EgressUnavailable
        ));
        assert_eq!(first.retry_decision(), AttemptRetryDecision::RetryEligible);
        assert_eq!(second.request_id(), &request_id);
        assert_eq!(second.attempt_number(), 2);
        assert_eq!(second.route_candidate_id().as_str(), "candidate-b");
        assert!(matches!(second.outcome(), AttemptOutcome::Succeeded));
        assert_eq!(second.retry_decision(), AttemptRetryDecision::Completed);
        assert!(!format!("{events:?}").contains("upstream-model"));
        Ok(())
    }

    #[tokio::test]
    async fn rate_limit_records_exact_quota_and_preserves_a_healthy_sibling() -> TestResult {
        let (orchestrator, route_id, _clock, health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a", "credential-b"])],
            3,
            100,
        )?;
        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::RateLimited {
                retry_after: Some(Duration::from_millis(20)),
            }),
            DriverStep::Success("sibling".to_owned()),
        ]);
        let gate = TestRetryGate::default();

        let started = orchestrator.start(&route_id, &driver, &gate).await?;

        assert_eq!(started.candidate().id().as_str(), "candidate-a");
        assert_eq!(started.lease().credential_id().as_str(), "credential-b");
        assert_eq!(
            driver.attempts()?,
            vec![
                ("candidate-a".to_owned(), "credential-a".to_owned()),
                ("candidate-a".to_owned(), "credential-b".to_owned())
            ]
        );
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential_a = CredentialId::try_new("credential-a")?;
        let credential_b = CredentialId::try_new("credential-b")?;
        assert!(health.endpoint_is_available(&endpoint));
        assert!(health.endpoint_credential_is_available(&endpoint, &credential_a));
        let quota_target =
            crate::RuntimeQuotaTarget::endpoint_credential(endpoint.clone(), credential_a.clone());
        let quota = orchestrator
            .runtime_quota
            .snapshot(&quota_target)?
            .ok_or("429 did not record a quota snapshot")?;
        assert_eq!(quota.source(), crate::QuotaSource::Header);
        assert_eq!(quota.blocking_reset_at_ms(), Some(120));
        assert!(
            !orchestrator
                .runtime_quota
                .endpoint_credential_is_available(&endpoint, &credential_a)
        );
        assert!(
            orchestrator
                .runtime_quota
                .endpoint_credential_is_available(&endpoint, &credential_b)
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_due_quota_reset_self_recovers_through_one_controlled_probe_attempt() -> TestResult {
        let (orchestrator, route_id, clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            3,
            60_000,
        )?;
        let target = crate::RuntimeQuotaTarget::endpoint_credential(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new("credential-a")?,
        );
        let rate_limited_driver =
            ScriptedDriver::new(vec![DriverStep::Failure(AttemptFailure::RateLimited {
                retry_after: Some(Duration::from_millis(20)),
            })]);
        let error = expected_error(
            orchestrator
                .start(&route_id, &rate_limited_driver, &TestRetryGate::default())
                .await,
            "a 429 on the only binding must fail the request",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::ProviderRateLimited);
        assert_eq!(
            orchestrator.runtime_quota.availability(&target)?,
            crate::RuntimeQuotaAvailability::Exhausted { reset_at_ms: 120 }
        );

        clock.advance_ms(20);
        assert_eq!(
            orchestrator.runtime_quota.availability(&target)?,
            crate::RuntimeQuotaAvailability::RecoveryRequired { reset_at_ms: 120 }
        );
        let probe_driver = ScriptedDriver::new(vec![DriverStep::Success("recovered".to_owned())]);
        let started = orchestrator
            .start(&route_id, &probe_driver, &TestRetryGate::default())
            .await?;
        assert_eq!(started.lease().credential_id().as_str(), "credential-a");
        assert_eq!(started.output(), "recovered");
        assert_eq!(started.attempts_started(), 1);
        assert_eq!(probe_driver.attempts()?.len(), 1);
        assert_eq!(
            orchestrator.runtime_quota.availability(&target)?,
            crate::RuntimeQuotaAvailability::Available
        );
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_selection_admits_at_most_one_quota_recovery_probe() -> TestResult {
        let (orchestrator, route_id, clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            3,
            60_000,
        )?;
        let target = crate::RuntimeQuotaTarget::endpoint_credential(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new("credential-a")?,
        );
        orchestrator.runtime_quota.record_rate_limited(
            target.clone(),
            100,
            Some(Duration::from_millis(20)),
            Duration::from_millis(20),
        )?;
        clock.advance_ms(20);

        let orchestrator = Arc::new(orchestrator);
        let probe_driver = Arc::new(PendingDriver::default());
        let probe_gate = Arc::new(TestRetryGate::default());
        let task_orchestrator = Arc::clone(&orchestrator);
        let task_driver = Arc::clone(&probe_driver);
        let task_gate = Arc::clone(&probe_gate);
        let task_route_id = route_id.clone();
        let probe_task = tokio::spawn(async move {
            task_orchestrator
                .start(&task_route_id, task_driver.as_ref(), task_gate.as_ref())
                .await
        });
        probe_driver.wait_started().await;
        assert!(matches!(
            orchestrator.runtime_quota.availability(&target)?,
            crate::RuntimeQuotaAvailability::RecoveryProbeInFlight { .. }
        ));

        let second_driver =
            ScriptedDriver::new(vec![DriverStep::Success("must-not-start".to_owned())]);
        let error = expected_error(
            orchestrator
                .start(&route_id, &second_driver, &TestRetryGate::default())
                .await,
            "a selection during an in-flight probe must not start a second probe",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::CredentialUnavailable);
        assert!(second_driver.attempts()?.is_empty());

        probe_gate.cancel();
        let result = probe_task.await?;
        assert!(result.is_err());
        assert!(matches!(
            orchestrator.runtime_quota.availability(&target)?,
            crate::RuntimeQuotaAvailability::RecoveryProbeInFlight { .. }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn a_failed_quota_probe_returns_to_cooldown_instead_of_flapping() -> TestResult {
        let (orchestrator, route_id, clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            3,
            60_000,
        )?;
        let target = crate::RuntimeQuotaTarget::endpoint_credential(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new("credential-a")?,
        );
        orchestrator.runtime_quota.record_rate_limited(
            target.clone(),
            100,
            Some(Duration::from_millis(20)),
            Duration::from_millis(20),
        )?;
        clock.advance_ms(20);
        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::RateLimited {
                retry_after: Some(Duration::from_millis(40)),
            }),
            DriverStep::Success("must-not-start".to_owned()),
        ]);

        let error = expected_error(
            orchestrator
                .start(&route_id, &driver, &TestRetryGate::default())
                .await,
            "a probe that hits another 429 must fail the request instead of flapping",
        )?;

        assert_eq!(error.code(), GatewayErrorCode::ProviderRateLimited);
        assert_eq!(driver.attempts()?.len(), 1);
        assert_eq!(
            orchestrator.runtime_quota.availability(&target)?,
            crate::RuntimeQuotaAvailability::Exhausted { reset_at_ms: 160 }
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_forbidden_account_is_never_admitted_as_a_quota_probe() -> TestResult {
        let (orchestrator, route_id, clock, health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            3,
            60_000,
        )?;
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential = CredentialId::try_new("credential-a")?;
        orchestrator.runtime_quota.record_rate_limited(
            crate::RuntimeQuotaTarget::endpoint_credential(endpoint.clone(), credential.clone()),
            100,
            Some(Duration::from_millis(20)),
            Duration::from_millis(20),
        )?;
        health.mark_credential_forbidden(endpoint, credential)?;
        clock.advance_ms(20);
        let driver = ScriptedDriver::new(vec![DriverStep::Success("must-not-start".to_owned())]);

        let error = expected_error(
            orchestrator
                .start(&route_id, &driver, &TestRetryGate::default())
                .await,
            "a forbidden account must not be admitted as a quota probe",
        )?;

        assert_eq!(error.code(), GatewayErrorCode::CredentialUnavailable);
        assert!(driver.attempts()?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn credential_forbidden_blocks_only_its_binding_until_controlled_recovery() -> TestResult
    {
        let (orchestrator, route_id, _clock, health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a", "credential-b"])],
            3,
            100,
        )?;
        let driver = ScriptedDriver::new(vec![DriverStep::Failure(AttemptFailure::NonRetryable(
            GatewayError::new(
                GatewayErrorCode::CredentialForbidden,
                ErrorScope::Credential,
            ),
        ))]);
        let gate = TestRetryGate::default();

        let error = expected_error(
            orchestrator.start(&route_id, &driver, &gate).await,
            "a provider-classified 403 must remain non-retryable",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::CredentialForbidden);
        assert_eq!(driver.attempts()?.len(), 1);

        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential_a = CredentialId::try_new("credential-a")?;
        let credential_b = CredentialId::try_new("credential-b")?;
        assert_eq!(
            health.credential_account_status_at(&endpoint, &credential_a, 100)?,
            RuntimeCredentialAccountStatus::Forbidden
        );
        assert!(!health.endpoint_credential_is_available(&endpoint, &credential_a));
        assert!(health.endpoint_credential_is_available(&endpoint, &credential_b));

        let sibling_driver = ScriptedDriver::new(vec![DriverStep::Success("sibling".to_owned())]);
        let sibling = orchestrator
            .start(&route_id, &sibling_driver, &TestRetryGate::default())
            .await?;
        assert_eq!(sibling.lease().credential_id().as_str(), "credential-b");
        drop(sibling);

        let recovery = health
            .begin_account_recovery(&endpoint, &credential_a, 200)?
            .ok_or("forbidden Credential did not issue a controlled recovery ticket")?;
        assert_eq!(
            health.credential_account_status_at(&endpoint, &credential_a, 100)?,
            RuntimeCredentialAccountStatus::RecoveryInFlight { expires_at_ms: 200 }
        );
        health.complete_account_recovery(recovery, RuntimeHealthAccountRecoveryResult::Allowed)?;
        assert_eq!(
            health.credential_account_status_at(&endpoint, &credential_a, 100)?,
            RuntimeCredentialAccountStatus::Available
        );
        assert!(health.endpoint_credential_is_available(&endpoint, &credential_a));
        Ok(())
    }

    #[tokio::test]
    async fn credential_unauthorized_blocks_only_its_binding_until_reauthorization() -> TestResult {
        let (orchestrator, route_id, _clock, health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a", "credential-b"])],
            3,
            100,
        )?;
        let driver = ScriptedDriver::new(vec![DriverStep::Failure(AttemptFailure::NonRetryable(
            GatewayError::new(
                GatewayErrorCode::CredentialUnauthorized,
                ErrorScope::Credential,
            ),
        ))]);
        let error = expected_error(
            orchestrator
                .start(&route_id, &driver, &TestRetryGate::default())
                .await,
            "an unauthorized credential must end the current request",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::CredentialUnauthorized);

        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential_a = CredentialId::try_new("credential-a")?;
        let credential_b = CredentialId::try_new("credential-b")?;
        assert_eq!(
            health.credential_account_status_at(&endpoint, &credential_a, 100)?,
            RuntimeCredentialAccountStatus::Unauthorized
        );
        assert!(!health.endpoint_credential_is_available(&endpoint, &credential_a));
        assert!(health.endpoint_credential_is_available(&endpoint, &credential_b));

        let sibling_driver = ScriptedDriver::new(vec![DriverStep::Success("sibling".to_owned())]);
        let sibling = orchestrator
            .start(&route_id, &sibling_driver, &TestRetryGate::default())
            .await?;
        assert_eq!(sibling.lease().credential_id().as_str(), "credential-b");
        Ok(())
    }

    #[tokio::test]
    async fn server_error_cools_the_endpoint_and_falls_back_to_another_candidate() -> TestResult {
        let (orchestrator, route_id, _clock, health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a"), ("candidate-b", "endpoint-b")],
            vec![
                ("endpoint-a", vec!["credential-a"]),
                ("endpoint-b", vec!["credential-b"]),
            ],
            3,
            100,
        )?;
        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::ServerError),
            DriverStep::Success("fallback".to_owned()),
        ]);
        let gate = TestRetryGate::default();

        let started = orchestrator.start(&route_id, &driver, &gate).await?;

        assert_eq!(started.candidate().id().as_str(), "candidate-b");
        assert_eq!(started.attempts_started(), 2);
        assert!(!health.endpoint_is_available(&EndpointId::try_new("endpoint-a")?));
        Ok(())
    }

    #[tokio::test]
    async fn pre_semantic_truncation_falls_back_but_post_fse_failure_never_retries() -> TestResult {
        let (count_orchestrator, route_id, _clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a"), ("candidate-b", "endpoint-b")],
            vec![
                ("endpoint-a", vec!["credential-a"]),
                ("endpoint-b", vec!["credential-b"]),
            ],
            3,
            100,
        )?;
        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::BootstrapTruncated),
            DriverStep::Success("recovered".to_owned()),
        ]);
        let gate = TestRetryGate::default();
        let started = count_orchestrator.start(&route_id, &driver, &gate).await?;
        assert_eq!(started.candidate().id().as_str(), "candidate-b");
        assert_eq!(driver.attempts()?.len(), 2);
        drop(started);

        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::BootstrapTruncated),
            DriverStep::Success("must-not-start".to_owned()),
        ]);
        let committed_gate = TestRetryGate::with_first_semantic_event();
        let error = expected_error(
            count_orchestrator
                .start(&route_id, &driver, &committed_gate)
                .await,
            "a committed first semantic event must close transparent retry",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::StreamTruncated);
        assert_eq!(error.scope(), ErrorScope::Stream);
        assert_eq!(driver.attempts()?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn budget_limits_attempt_count_and_cumulative_bootstrap_time() -> TestResult {
        let (count_orchestrator, route_id, _clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a"), ("candidate-b", "endpoint-b")],
            vec![
                ("endpoint-a", vec!["credential-a"]),
                ("endpoint-b", vec!["credential-b"]),
            ],
            1,
            100,
        )?;
        let driver = ScriptedDriver::new(vec![
            DriverStep::Failure(AttemptFailure::Connection),
            DriverStep::Success("must-not-start".to_owned()),
        ]);
        let gate = TestRetryGate::default();
        let error = expected_error(
            count_orchestrator.start(&route_id, &driver, &gate).await,
            "max-attempt budget should stop the second start",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::EgressUnavailable);
        assert_eq!(driver.attempts()?.len(), 1);

        let (timeout_orchestrator, route_id, clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a"), ("candidate-b", "endpoint-b")],
            vec![
                ("endpoint-a", vec!["credential-a"]),
                ("endpoint-b", vec!["credential-b"]),
            ],
            3,
            10,
        )?;
        let driver = ScriptedDriver::with_clock(
            vec![
                DriverStep::AdvanceClockAndFail {
                    by_ms: 10,
                    failure: AttemptFailure::Connection,
                },
                DriverStep::Success("must-not-start".to_owned()),
            ],
            Arc::clone(&clock),
        );
        let error = expected_error(
            timeout_orchestrator.start(&route_id, &driver, &gate).await,
            "cumulative bootstrap deadline should stop the second start",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::EgressUnavailable);
        assert_eq!(driver.attempts()?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn cancellation_stops_selection_and_success_holds_its_lease_until_drop() -> TestResult {
        let (orchestrator, route_id, _clock, _health, pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            3,
            100,
        )?;
        let driver = ScriptedDriver::new(vec![DriverStep::Success("live".to_owned())]);
        let gate = TestRetryGate::cancelled();
        let error = expected_error(
            orchestrator.start(&route_id, &driver, &gate).await,
            "a cancelled request must not start an attempt",
        )?;
        assert_eq!(error.code(), GatewayErrorCode::Cancelled);
        assert!(driver.attempts()?.is_empty());

        let gate = TestRetryGate::default();
        let started = orchestrator.start(&route_id, &driver, &gate).await?;
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential = CredentialId::try_new("credential-a")?;
        let pool = pools.pool(&endpoint).ok_or("missing test Endpoint pool")?;
        assert_eq!(pool.active_lease_count(&credential), Some(1));
        drop(started);
        assert_eq!(pool.active_lease_count(&credential), Some(0));
        Ok(())
    }

    #[tokio::test]
    async fn a_driver_declared_start_timeout_lets_one_attempt_outlive_the_bootstrap_deadline()
    -> TestResult {
        let (orchestrator, route_id, _clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            1,
            50,
        )?;
        let driver = ScriptedDriver::with_start_timeout(
            vec![DriverStep::SleepAndSucceed {
                delay: Duration::from_millis(200),
                output: "slow-but-healthy".to_owned(),
            }],
            Duration::from_secs(5),
        );
        let gate = TestRetryGate::default();

        let started = orchestrator.start(&route_id, &driver, &gate).await?;

        assert_eq!(started.output(), "slow-but-healthy");
        assert_eq!(started.attempts_started(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn the_default_start_timeout_still_cuts_an_attempt_at_the_remaining_bootstrap_budget()
    -> TestResult {
        let (orchestrator, route_id, _clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            1,
            50,
        )?;
        let driver = ScriptedDriver::new(vec![DriverStep::SleepAndSucceed {
            delay: Duration::from_millis(500),
            output: "must-not-complete".to_owned(),
        }]);
        let gate = TestRetryGate::default();

        let error = expected_error(
            orchestrator.start(&route_id, &driver, &gate).await,
            "the default per-attempt ceiling must still be the remaining bootstrap budget",
        )?;

        assert_eq!(error.code(), GatewayErrorCode::EgressUnavailable);
        assert_eq!(driver.attempts()?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn an_extended_start_timeout_preserves_pre_first_byte_transparent_retry() -> TestResult {
        let (orchestrator, route_id, _clock, health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a"), ("candidate-b", "endpoint-b")],
            vec![
                ("endpoint-a", vec!["credential-a"]),
                ("endpoint-b", vec!["credential-b"]),
            ],
            3,
            100,
        )?;
        let driver = ScriptedDriver::with_start_timeout(
            vec![
                DriverStep::Failure(AttemptFailure::Connection),
                DriverStep::Success("failed-over".to_owned()),
            ],
            Duration::from_secs(5),
        );
        let gate = TestRetryGate::default();

        let started = orchestrator.start(&route_id, &driver, &gate).await?;

        assert_eq!(started.candidate().id().as_str(), "candidate-b");
        assert_eq!(started.output(), "failed-over");
        assert_eq!(started.attempts_started(), 2);
        assert!(!health.endpoint_is_available(&EndpointId::try_new("endpoint-a")?));
        Ok(())
    }

    #[tokio::test]
    async fn a_mid_body_failure_under_an_extended_start_timeout_returns_the_safe_truncation_error()
    -> TestResult {
        let (orchestrator, route_id, _clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            1,
            50,
        )?;
        let driver = ScriptedDriver::with_start_timeout(
            vec![DriverStep::Failure(AttemptFailure::BootstrapTruncated)],
            Duration::from_secs(5),
        );
        let gate = TestRetryGate::default();

        let error = expected_error(
            orchestrator.start(&route_id, &driver, &gate).await,
            "an unretryable mid-body truncation must surface its safe stream error",
        )?;

        assert_eq!(error.code(), GatewayErrorCode::StreamTruncated);
        assert_eq!(error.scope(), ErrorScope::Stream);
        assert_eq!(driver.attempts()?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn cancellation_drops_an_inflight_driver_future_without_a_retry() -> TestResult {
        let (orchestrator, route_id, _clock, _health, _pools) = orchestrator(
            vec![("candidate-a", "endpoint-a")],
            vec![("endpoint-a", vec!["credential-a"])],
            3,
            100,
        )?;
        let orchestrator = Arc::new(orchestrator);
        let driver = Arc::new(PendingDriver::default());
        let gate = Arc::new(TestRetryGate::default());
        let task_orchestrator = Arc::clone(&orchestrator);
        let task_driver = Arc::clone(&driver);
        let task_gate = Arc::clone(&gate);
        let task_route_id = route_id.clone();
        let task = tokio::spawn(async move {
            task_orchestrator
                .start(&task_route_id, task_driver.as_ref(), task_gate.as_ref())
                .await
        });

        driver.wait_started().await;
        gate.cancel();
        let result = task.await?;
        let Err(error) = result else {
            return Err("cancelled Attempt unexpectedly succeeded".into());
        };
        assert_eq!(error.code(), GatewayErrorCode::Cancelled);
        assert_eq!(error.scope(), ErrorScope::Request);
        assert!(driver.was_dropped());
        Ok(())
    }

    #[test]
    fn exclusion_set_is_binding_scoped_and_redacts_its_debug_form() -> TestResult {
        let first_candidate = candidate("candidate-a", "endpoint-a")?;
        let same_candidate = candidate("candidate-a", "endpoint-b")?;
        let credential_a = CredentialId::try_new("credential-a")?;
        let credential_b = CredentialId::try_new("credential-b")?;
        let mut exclusions = AttemptExclusionSet::new();

        exclusions.insert(&first_candidate, &credential_a);
        assert!(exclusions.contains(&first_candidate, &credential_a));
        assert!(!exclusions.contains(&first_candidate, &credential_b));
        assert!(exclusions.contains(&same_candidate, &credential_a));
        assert_eq!(exclusions.len(), 1);
        assert!(!format!("{exclusions:?}").contains("credential-a"));
        assert!(!format!("{exclusions:?}").contains("candidate-a"));
        Ok(())
    }

    fn orchestrator(
        candidate_specs: Vec<CandidateSpec<'_>>,
        credential_specs: Vec<CredentialSpec<'_>>,
        max_attempts: i64,
        bootstrap_timeout_ms: i64,
    ) -> Result<OrchestratorFixture, Box<dyn Error>> {
        let route_id = RouteId::try_new("route-a")?;
        let public_model_id = PublicModelId::try_new("public-model-a")?;
        let candidates = candidate_specs
            .into_iter()
            .map(|(candidate_id, endpoint_id)| candidate(candidate_id, endpoint_id))
            .collect::<Result<Vec<_>, _>>()?;
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new("version-a")?,
            vec![SnapshotPublicModel::new(
                public_model_id.clone(),
                "public-model".to_owned(),
                "Public Model".to_owned(),
                CapabilitySet::empty(),
                route_id.clone(),
            )],
            Vec::new(),
            vec![SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                SnapshotRoutePolicy::SmoothWeightedRoundRobin,
                max_attempts,
                bootstrap_timeout_ms,
                candidates,
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let pools = credential_specs
            .into_iter()
            .map(|(endpoint_id, credentials)| endpoint_pool(endpoint_id, credentials))
            .collect::<Result<Vec<_>, _>>()?;
        let pools = Arc::new(EndpointCredentialPools::try_new(pools)?);
        let scheduler = Arc::new(RouteCredentialScheduler::new(snapshot, Arc::clone(&pools)));
        let clock = Arc::new(FixedRuntimeHealthClock::new(100));
        let health_clock: Arc<dyn RuntimeHealthClock> = clock.clone();
        let health = Arc::new(RuntimeHealthRegistry::with_clock(Arc::clone(&health_clock)));
        let config = AttemptOrchestratorConfig::try_new(
            Duration::from_millis(20),
            Duration::from_millis(10),
        )?;
        Ok((
            AttemptOrchestrator::with_clock_and_config(
                scheduler,
                Arc::clone(&health),
                health_clock,
                config,
            ),
            route_id,
            clock,
            health,
            pools,
        ))
    }

    fn continuation_orchestrator() -> Result<OrchestratorFixture, Box<dyn Error>> {
        let route_id = RouteId::try_new("route-continuity")?;
        let public_model_id = PublicModelId::try_new("public-model-continuity")?;
        let capabilities = CapabilitySet::try_new([
            SemanticCapability::StoredResponses,
            SemanticCapability::ResponseCompaction,
        ])?;
        let candidate = |id: &str, endpoint: &str, upstream: &str, priority| {
            Ok::<_, Box<dyn Error>>(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
                id: RouteCandidateId::try_new(id)?,
                endpoint_id: EndpointId::try_new(endpoint)?,
                upstream_id: UpstreamId::try_new(upstream)?,
                endpoint_api_format: "openai/responses".to_owned(),
                upstream_model: "upstream-model".to_owned(),
                transform_mode: SnapshotTransformMode::Canonical,
                priority,
                weight: 1,
                effective_capabilities: capabilities.clone(),
                catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
                active_binding_count: 1,
            }))
        };
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new("version-continuity")?,
            vec![SnapshotPublicModel::new(
                public_model_id.clone(),
                "public-model".to_owned(),
                "Public Model".to_owned(),
                CapabilitySet::empty(),
                route_id.clone(),
            )],
            Vec::new(),
            vec![SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                SnapshotRoutePolicy::RoundRobin,
                2,
                100,
                vec![
                    candidate(
                        "candidate-continuity",
                        "endpoint-continuity",
                        "upstream-continuity",
                        0,
                    )?,
                    candidate(
                        "candidate-fallback",
                        "endpoint-fallback",
                        "upstream-fallback",
                        1,
                    )?,
                ],
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let exact_pool = EndpointCredentialPool::try_new(
            EndpointId::try_new("endpoint-continuity")?,
            [EndpointCredentialInput {
                credential_id: CredentialId::try_new("credential-continuity")?,
                credential_kind: "oauth".to_owned(),
                credential_revision: 11,
                priority: 0,
                weight: 1,
                concurrency: 1,
                expires_at_ms: None,
                secret: CredentialSecret::try_new(b"continuity-secret".to_vec())?,
            }],
        )?;
        let pools = Arc::new(EndpointCredentialPools::try_new([
            exact_pool,
            endpoint_pool("endpoint-fallback", vec!["credential-fallback"])?,
        ])?);
        let scheduler = Arc::new(RouteCredentialScheduler::new(snapshot, Arc::clone(&pools)));
        let clock = Arc::new(FixedRuntimeHealthClock::new(100));
        let health_clock: Arc<dyn RuntimeHealthClock> = clock.clone();
        let health = Arc::new(RuntimeHealthRegistry::with_clock(Arc::clone(&health_clock)));
        let config = AttemptOrchestratorConfig::try_new(
            Duration::from_millis(20),
            Duration::from_millis(10),
        )?;
        Ok((
            AttemptOrchestrator::with_clock_and_config(
                scheduler,
                Arc::clone(&health),
                health_clock,
                config,
            ),
            route_id,
            clock,
            health,
            pools,
        ))
    }

    fn continuation_pin(
        kind: ResponsesContinuationKind,
    ) -> Result<ResponsesContinuationPin, Box<dyn Error>> {
        Ok(ResponsesContinuationPin::new(
            ResponsesExecutionLineage::new(
                SnapshotVersion::try_new("version-continuity")?,
                ProviderId::try_new("upstream-continuity")?,
                UpstreamId::try_new("upstream-continuity")?,
                EndpointId::try_new("endpoint-continuity")?,
                RouteId::try_new("route-continuity")?,
                RouteCandidateId::try_new("candidate-continuity")?,
                CredentialId::try_new("credential-continuity")?,
                11,
            ),
            kind,
        ))
    }

    fn protocol_isolation_orchestrator() -> Result<ProtocolIsolationFixture, Box<dyn Error>> {
        let route_id = RouteId::try_new("route-shared-upstream")?;
        let public_model_id = PublicModelId::try_new("public-model-shared-upstream")?;
        let candidates = vec![
            candidate_with_endpoint_format(
                "candidate-responses",
                "endpoint-responses",
                "upstream-shared",
                "openai/responses",
                SnapshotTransformMode::Canonical,
            )?,
            candidate_with_endpoint_format(
                "candidate-anthropic",
                "endpoint-anthropic",
                "upstream-shared",
                "anthropic/messages",
                SnapshotTransformMode::Canonical,
            )?,
        ];
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new("version-shared-upstream")?,
            vec![SnapshotPublicModel::new(
                public_model_id.clone(),
                "public-model-shared-upstream".to_owned(),
                "Shared Upstream Public Model".to_owned(),
                CapabilitySet::empty(),
                route_id.clone(),
            )],
            Vec::new(),
            vec![SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                SnapshotRoutePolicy::RoundRobin,
                2,
                100,
                candidates,
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let pools = Arc::new(EndpointCredentialPools::try_new(vec![
            endpoint_pool("endpoint-responses", vec!["credential-responses"])?,
            endpoint_pool("endpoint-anthropic", vec!["credential-anthropic"])?,
        ])?);
        let scheduler = Arc::new(RouteCredentialScheduler::new(snapshot, Arc::clone(&pools)));
        let clock: Arc<dyn RuntimeHealthClock> = Arc::new(FixedRuntimeHealthClock::new(100));
        let health = Arc::new(RuntimeHealthRegistry::with_clock(Arc::clone(&clock)));
        let config = AttemptOrchestratorConfig::try_new(
            Duration::from_millis(20),
            Duration::from_millis(10),
        )?;
        Ok((
            AttemptOrchestrator::with_clock_and_config(
                scheduler,
                Arc::clone(&health),
                clock,
                config,
            ),
            route_id,
            health,
            pools,
        ))
    }

    fn single_protocol_orchestrator(
        transform_mode: SnapshotTransformMode,
    ) -> Result<ProtocolIsolationFixture, Box<dyn Error>> {
        let route_id = RouteId::try_new("route-noncanonical")?;
        let public_model_id = PublicModelId::try_new("public-model-noncanonical")?;
        let candidate = candidate_with_endpoint_format(
            "candidate-noncanonical",
            "endpoint-noncanonical",
            "upstream-noncanonical",
            "openai/responses",
            transform_mode,
        )?;
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new("version-noncanonical")?,
            vec![SnapshotPublicModel::new(
                public_model_id.clone(),
                "public-model-noncanonical".to_owned(),
                "Non-Canonical Public Model".to_owned(),
                CapabilitySet::empty(),
                route_id.clone(),
            )],
            Vec::new(),
            vec![SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                SnapshotRoutePolicy::RoundRobin,
                1,
                100,
                vec![candidate],
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let pools = Arc::new(EndpointCredentialPools::try_new(vec![endpoint_pool(
            "endpoint-noncanonical",
            vec!["credential-noncanonical"],
        )?])?);
        let scheduler = Arc::new(RouteCredentialScheduler::new(snapshot, Arc::clone(&pools)));
        let clock: Arc<dyn RuntimeHealthClock> = Arc::new(FixedRuntimeHealthClock::new(100));
        let health = Arc::new(RuntimeHealthRegistry::with_clock(Arc::clone(&clock)));
        let config = AttemptOrchestratorConfig::try_new(
            Duration::from_millis(20),
            Duration::from_millis(10),
        )?;
        Ok((
            AttemptOrchestrator::with_clock_and_config(
                scheduler,
                Arc::clone(&health),
                clock,
                config,
            ),
            route_id,
            health,
            pools,
        ))
    }

    fn candidate(
        candidate_id: &str,
        endpoint_id: &str,
    ) -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
        Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(candidate_id)?,
            endpoint_id: EndpointId::try_new(endpoint_id)?,
            upstream_id: UpstreamId::try_new(format!("upstream-{endpoint_id}"))?,
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: "upstream-model".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
            active_binding_count: 1,
        }))
    }

    fn candidate_with_endpoint_format(
        candidate_id: &str,
        endpoint_id: &str,
        upstream_id: &str,
        endpoint_api_format: &str,
        transform_mode: SnapshotTransformMode,
    ) -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
        Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(candidate_id)?,
            endpoint_id: EndpointId::try_new(endpoint_id)?,
            upstream_id: UpstreamId::try_new(upstream_id)?,
            endpoint_api_format: endpoint_api_format.to_owned(),
            upstream_model: "upstream-model".to_owned(),
            transform_mode,
            priority: 0,
            weight: 1,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
            active_binding_count: 1,
        }))
    }

    fn candidate_with_catalog_state(
        candidate_id: &str,
        endpoint_id: &str,
        catalog_state: CatalogModelState,
    ) -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
        Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(candidate_id)?,
            endpoint_id: EndpointId::try_new(endpoint_id)?,
            upstream_id: UpstreamId::try_new(format!("upstream-{endpoint_id}"))?,
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: "upstream-model".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::Listed(catalog_state),
            active_binding_count: 1,
        }))
    }

    fn endpoint_pool(
        endpoint_id: &str,
        credential_ids: Vec<&str>,
    ) -> Result<EndpointCredentialPool, Box<dyn Error>> {
        let entries = credential_ids
            .into_iter()
            .map(|credential_id| {
                Ok(EndpointCredentialInput {
                    credential_id: CredentialId::try_new(credential_id)?,
                    credential_kind: "api_key".to_owned(),
                    credential_revision: 0,
                    priority: 0,
                    weight: 1,
                    concurrency: 1,
                    expires_at_ms: None,
                    secret: CredentialSecret::try_new(
                        format!("synthetic-{credential_id}").into_bytes(),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(EndpointCredentialPool::try_new(
            EndpointId::try_new(endpoint_id)?,
            entries,
        )?)
    }

    fn expected_error<T>(
        result: Result<T, GatewayError>,
        message: &str,
    ) -> Result<GatewayError, Box<dyn Error>> {
        match result {
            Ok(_) => Err(message.into()),
            Err(error) => Ok(error),
        }
    }

    #[derive(Default)]
    struct RecordingEventSink {
        events: Mutex<Vec<GatewayEvent>>,
    }

    impl RecordingEventSink {
        fn events(&self) -> Result<Vec<GatewayEvent>, Box<dyn Error>> {
            self.events
                .lock()
                .map(|events| events.clone())
                .map_err(|_| "event recorder lock poisoned".into())
        }
    }

    impl GatewayEventSink for RecordingEventSink {
        fn try_emit(&self, event: GatewayEvent) -> EventEmission {
            match self.events.lock() {
                Ok(mut events) => {
                    events.push(event);
                    EventEmission::Enqueued
                }
                Err(_) => EventEmission::SinkClosed,
            }
        }
    }

    #[derive(Debug)]
    struct FixedRuntimeHealthClock {
        now_ms: AtomicI64,
    }

    impl FixedRuntimeHealthClock {
        const fn new(now_ms: i64) -> Self {
            Self {
                now_ms: AtomicI64::new(now_ms),
            }
        }

        fn advance_ms(&self, by_ms: i64) {
            self.now_ms.fetch_add(by_ms, Ordering::AcqRel);
        }
    }

    impl RuntimeHealthClock for FixedRuntimeHealthClock {
        fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
            Ok(self.now_ms.load(Ordering::Acquire))
        }
    }

    #[derive(Clone)]
    enum DriverStep {
        Failure(AttemptFailure),
        AdvanceClockAndFail { by_ms: i64, failure: AttemptFailure },
        Success(String),
        SleepAndSucceed { delay: Duration, output: String },
    }

    struct ScriptedDriver {
        steps: Mutex<VecDeque<DriverStep>>,
        attempts: Mutex<Vec<(String, String)>>,
        clock: Option<Arc<FixedRuntimeHealthClock>>,
        start_timeout_override: Option<Duration>,
    }

    #[derive(Default)]
    struct PendingDriver {
        started: AtomicBool,
        started_notification: Notify,
        dropped: Arc<AtomicBool>,
    }

    impl PendingDriver {
        async fn wait_started(&self) {
            loop {
                let notified = self.started_notification.notified();
                if self.started.load(Ordering::Acquire) {
                    return;
                }
                notified.await;
            }
        }

        fn was_dropped(&self) -> bool {
            self.dropped.load(Ordering::Acquire)
        }
    }

    impl AttemptDriver for PendingDriver {
        type Output = String;

        fn start<'a>(
            &'a self,
            _candidate: &'a SnapshotRouteCandidate,
            _credential: &'a gateway_upstream::CredentialLease,
            _bootstrap_timeout: Duration,
        ) -> AttemptFuture<'a, Result<Self::Output, AttemptFailure>> {
            self.started.store(true, Ordering::Release);
            self.started_notification.notify_waiters();
            let dropped = Arc::clone(&self.dropped);
            Box::pin(async move {
                let _drop_signal = AttemptDropSignal(dropped);
                std::future::pending::<Result<String, AttemptFailure>>().await
            })
        }
    }

    struct AttemptDropSignal(Arc<AtomicBool>);

    impl Drop for AttemptDropSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    impl ScriptedDriver {
        fn new(steps: Vec<DriverStep>) -> Self {
            Self {
                steps: Mutex::new(VecDeque::from(steps)),
                attempts: Mutex::new(Vec::new()),
                clock: None,
                start_timeout_override: None,
            }
        }

        fn with_clock(steps: Vec<DriverStep>, clock: Arc<FixedRuntimeHealthClock>) -> Self {
            Self {
                steps: Mutex::new(VecDeque::from(steps)),
                attempts: Mutex::new(Vec::new()),
                clock: Some(clock),
                start_timeout_override: None,
            }
        }

        fn with_start_timeout(steps: Vec<DriverStep>, start_timeout: Duration) -> Self {
            Self {
                steps: Mutex::new(VecDeque::from(steps)),
                attempts: Mutex::new(Vec::new()),
                clock: None,
                start_timeout_override: Some(start_timeout),
            }
        }

        fn attempts(&self) -> Result<Vec<(String, String)>, Box<dyn Error>> {
            Ok(self
                .attempts
                .lock()
                .map_err(|_| "test Attempt record lock poisoned")?
                .clone())
        }
    }

    impl AttemptDriver for ScriptedDriver {
        type Output = String;

        fn start<'a>(
            &'a self,
            candidate: &'a SnapshotRouteCandidate,
            credential: &'a gateway_upstream::CredentialLease,
            _bootstrap_timeout: Duration,
        ) -> AttemptFuture<'a, Result<Self::Output, AttemptFailure>> {
            let attempt = (
                candidate.id().as_str().to_owned(),
                credential.credential_id().as_str().to_owned(),
            );
            let recorded = match self.attempts.lock() {
                Ok(mut attempts) => {
                    attempts.push(attempt);
                    true
                }
                Err(_) => false,
            };
            let step = match self.steps.lock() {
                Ok(mut steps) => steps.pop_front(),
                Err(_) => None,
            };
            let clock = self.clock.clone();

            Box::pin(async move {
                if !recorded {
                    return Err(AttemptFailure::NonRetryable(super::internal_error()));
                }
                let Some(step) = step else {
                    return Err(AttemptFailure::NonRetryable(super::internal_error()));
                };
                match step {
                    DriverStep::Failure(failure) => Err(failure),
                    DriverStep::AdvanceClockAndFail { by_ms, failure } => {
                        let Some(clock) = clock else {
                            return Err(AttemptFailure::NonRetryable(super::internal_error()));
                        };
                        clock.advance_ms(by_ms);
                        Err(failure)
                    }
                    DriverStep::Success(output) => Ok(output),
                    DriverStep::SleepAndSucceed { delay, output } => {
                        tokio::time::sleep(delay).await;
                        Ok(output)
                    }
                }
            })
        }

        fn start_timeout(&self, remaining_bootstrap: Duration) -> Duration {
            self.start_timeout_override.unwrap_or(remaining_bootstrap)
        }
    }

    #[derive(Default)]
    struct TestRetryGate {
        cancelled: AtomicBool,
        first_semantic_event: AtomicBool,
        cancellation_notification: Notify,
    }

    impl TestRetryGate {
        fn cancelled() -> Self {
            Self {
                cancelled: AtomicBool::new(true),
                first_semantic_event: AtomicBool::new(false),
                cancellation_notification: Notify::new(),
            }
        }

        fn with_first_semantic_event() -> Self {
            Self {
                cancelled: AtomicBool::new(false),
                first_semantic_event: AtomicBool::new(true),
                cancellation_notification: Notify::new(),
            }
        }

        fn cancel(&self) {
            if !self.cancelled.swap(true, Ordering::AcqRel) {
                self.cancellation_notification.notify_waiters();
            }
        }
    }

    impl TransparentRetryGate for TestRetryGate {
        fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::Acquire)
        }

        fn allows_transparent_retry(&self) -> bool {
            !self.is_cancelled() && !self.first_semantic_event.load(Ordering::Acquire)
        }

        fn cancelled(&self) -> TransparentRetryGateFuture<'_> {
            Box::pin(async move {
                loop {
                    let notified = self.cancellation_notification.notified();
                    if self.is_cancelled() {
                        return;
                    }
                    notified.await;
                }
            })
        }
    }
}
