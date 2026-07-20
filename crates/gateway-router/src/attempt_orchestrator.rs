//! Bounded request-scoped Attempt orchestration.
//!
//! This module composes the existing immutable route schedule, Endpoint-local Credential leases,
//! and sharded runtime health into one pre-first-semantic-event retry loop. It intentionally owns
//! no HTTP client, Provider decoder, persistence handle, or downstream protocol writer.

use std::{collections::BTreeSet, fmt, future::Future, pin::Pin, sync::Arc, time::Duration};

use gateway_core::{
    AttemptEvent, AttemptOutcome, AttemptRetryDecision, CredentialId, ErrorScope, GatewayError,
    GatewayErrorCode, GatewayEvent, GatewayEventSink, NoopGatewayEventSink, RequestId,
    RouteCandidateId, RouteId, TransparentRetryGate,
};
use gateway_upstream::CredentialLease;

use crate::{
    RouteCredentialScheduler, RuntimeHealthClock, RuntimeHealthKey, RuntimeHealthRegistry,
    SelectedRouteCredential, SnapshotRouteCandidate, SystemRuntimeHealthClock,
};

/// The finite fallback Cooldown used for a 429 that does not declare retry-after information.
pub const DEFAULT_RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(30);

/// The finite Endpoint Cooldown used for connection, 5xx, and pre-semantic truncation failures.
pub const DEFAULT_TRANSIENT_COOLDOWN: Duration = Duration::from_secs(5);

/// A boxed, sendable async operation used by an [`AttemptDriver`].
pub type AttemptFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Safe construction failures for [`AttemptOrchestratorConfig`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptOrchestratorConfigError {
    /// The fallback Cooldown for a 429 must be strictly positive.
    ZeroRateLimitCooldown,
    /// The Endpoint Cooldown for a transient failure must be strictly positive.
    ZeroTransientCooldown,
}

impl fmt::Display for AttemptOrchestratorConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroRateLimitCooldown => {
                formatter.write_str("rate-limit fallback cooldown must be positive")
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
    /// Validates finite positive fallback Cooldowns.
    ///
    /// # Errors
    ///
    /// Returns [`AttemptOrchestratorConfigError`] before a retry loop can issue an invalid
    /// runtime-health deadline.
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

    /// Returns the fallback Cooldown for a 429 without retry-after information.
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
            Self::RateLimited { retry_after } => Some(CooldownScope::EndpointCredential(
                retry_after
                    .filter(|duration| !duration.is_zero())
                    .unwrap_or(config.rate_limit_fallback_cooldown()),
            )),
            Self::Connection | Self::ServerError | Self::BootstrapTruncated => {
                Some(CooldownScope::Endpoint(config.transient_cooldown()))
            }
            Self::Cancelled | Self::NonRetryable(_) => None,
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

    /// Starts one Attempt under the remaining cumulative bootstrap deadline.
    ///
    /// The driver must report only a safe [`AttemptFailure`] for failure. It must not encode raw
    /// upstream status bodies, endpoint URLs, headers, or Secret material into that error.
    fn start<'a>(
        &'a self,
        candidate: &'a SnapshotRouteCandidate,
        credential: &'a CredentialLease,
        bootstrap_timeout: Duration,
    ) -> AttemptFuture<'a, Result<Self::Output, AttemptFailure>>;
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
        Self {
            scheduler,
            runtime_health,
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
        self.start_inner(None, route_id, driver, retry_gate, &event_sink)
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
        self.start_inner(Some(request_id), route_id, driver, retry_gate, event_sink)
            .await
    }

    #[allow(clippy::too_many_lines)] // One retry state machine keeps lease, gate, and event ordering auditable.
    async fn start_inner<D>(
        &self,
        request_id: Option<&RequestId>,
        route_id: &RouteId,
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

            let selection = match self
                .scheduler
                .select_eligible_and_lease_with_runtime_health_and_binding(
                    route_id,
                    &self.runtime_health,
                    |_| true,
                    |candidate, credential_id| !exclusions.contains(candidate, credential_id),
                ) {
                Ok(selection) => selection,
                Err(error) => return Err(last_failure.unwrap_or(error)),
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
                    remaining_bootstrap,
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
            if !failure.is_retryable() {
                let safe_failure = failure.safe_error();
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
            let safe_failure = failure.safe_error();
            if let Err(error) = self.record_runtime_health(&selection, &failure) {
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
    ) {
        let Some(request_id) = request_id else {
            return;
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
        let _emission = event_sink.try_emit(GatewayEvent::Attempt(event));
    }

    fn record_runtime_health(
        &self,
        selection: &SelectedRouteCredential,
        failure: &AttemptFailure,
    ) -> Result<(), GatewayError> {
        let Some(cooldown) = failure.cooldown(self.config) else {
            return Ok(());
        };
        let now_ms = self.clock.now_ms().map_err(|_| internal_error())?;
        let until_ms = add_duration_to_timestamp(now_ms, cooldown.duration())?;
        let key = match cooldown {
            CooldownScope::Endpoint(_) => {
                RuntimeHealthKey::endpoint(selection.candidate().endpoint_id().clone())
            }
            CooldownScope::EndpointCredential(_) => RuntimeHealthKey::endpoint_credential(
                selection.candidate().endpoint_id().clone(),
                selection.lease().credential_id().clone(),
            ),
        };
        self.runtime_health
            .cool_down_until(key, until_ms)
            .map_err(|_| internal_error())
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
    EndpointCredential(Duration),
}

impl CooldownScope {
    const fn duration(self) -> Duration {
        match self {
            Self::Endpoint(duration) | Self::EndpointCredential(duration) => duration,
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

    use gateway_catalog::{CapabilitySet, CatalogModelState};
    use gateway_core::{
        AttemptOutcome, AttemptRetryDecision, CredentialId, EndpointId, ErrorScope, EventEmission,
        GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink, PublicModelId, RequestId,
        RouteCandidateId, RouteId, TransparentRetryGate, TransparentRetryGateFuture, UpstreamId,
    };
    use gateway_upstream::{
        CredentialSecret, EndpointCredentialInput, EndpointCredentialPool, EndpointCredentialPools,
    };
    use tokio::sync::Notify;

    use super::{
        AttemptDriver, AttemptExclusionSet, AttemptFailure, AttemptFuture, AttemptOrchestrator,
        AttemptOrchestratorConfig,
    };
    use crate::{
        RouteCredentialScheduler, RouteSnapshot, RouteSnapshotInput, RuntimeHealthClock,
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
    async fn rate_limit_cools_only_the_failed_binding_and_preserves_a_healthy_sibling() -> TestResult
    {
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
        assert!(!health.endpoint_credential_is_available(&endpoint, &credential_a));
        assert!(health.endpoint_credential_is_available(&endpoint, &credential_b));
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

    fn candidate(
        candidate_id: &str,
        endpoint_id: &str,
    ) -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
        Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(candidate_id)?,
            endpoint_id: EndpointId::try_new(endpoint_id)?,
            upstream_id: UpstreamId::try_new(format!("upstream-{endpoint_id}"))?,
            upstream_model: "upstream-model".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
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
    }

    struct ScriptedDriver {
        steps: Mutex<VecDeque<DriverStep>>,
        attempts: Mutex<Vec<(String, String)>>,
        clock: Option<Arc<FixedRuntimeHealthClock>>,
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
            }
        }

        fn with_clock(steps: Vec<DriverStep>, clock: Arc<FixedRuntimeHealthClock>) -> Self {
            Self {
                steps: Mutex::new(VecDeque::from(steps)),
                attempts: Mutex::new(Vec::new()),
                clock: Some(clock),
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
                }
            })
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
