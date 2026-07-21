//! Bounded, side-effect-free Route-Candidate eligibility explanations.
//!
//! This module reads one immutable Route Snapshot plus non-secret runtime observations. It never
//! acquires a Credential lease, advances a scheduler cursor, opens a Circuit/Quota probe, queries
//! persistence, or executes a Provider request.

use std::{error::Error, fmt};

use gateway_core::{CredentialId, EndpointId, RouteCandidateId, RouteId, UpstreamId};
use gateway_upstream::{CredentialPoolEntrySnapshot, EndpointCredentialPools};

use crate::{
    AttemptExclusionSet, RouteSnapshot, RuntimeHealthAvailability, RuntimeHealthKey,
    RuntimeHealthRegistry, RuntimeQuotaAvailability, RuntimeQuotaRegistry, RuntimeQuotaTarget,
    SnapshotCatalogAdmission,
};

/// One immutable input for a deterministic Route Explain projection.
///
/// The two schedule starts make a test or management caller's diagnostic choice reproducible. They
/// do not read or advance the live scheduler cursors used by real requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteExplainInput {
    route_id: RouteId,
    observed_at_ms: i64,
    candidate_schedule_start: usize,
    credential_schedule_start: usize,
}

impl RouteExplainInput {
    /// Creates an Explain input at the first precompiled slot of every priority tier.
    #[must_use]
    pub fn new(route_id: RouteId, observed_at_ms: i64) -> Self {
        Self::with_schedule_starts(route_id, observed_at_ms, 0, 0)
    }

    /// Creates an Explain input with explicit side-effect-free schedule starts.
    #[must_use]
    pub const fn with_schedule_starts(
        route_id: RouteId,
        observed_at_ms: i64,
        candidate_schedule_start: usize,
        credential_schedule_start: usize,
    ) -> Self {
        Self {
            route_id,
            observed_at_ms,
            candidate_schedule_start,
            credential_schedule_start,
        }
    }

    /// Returns the exact Route being explained.
    #[must_use]
    pub fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    /// Returns the explicit time used for every Health and Quota lookup.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Returns the deterministic Candidate-schedule start.
    #[must_use]
    pub const fn candidate_schedule_start(&self) -> usize {
        self.candidate_schedule_start
    }

    /// Returns the deterministic Credential-schedule start.
    #[must_use]
    pub const fn credential_schedule_start(&self) -> usize {
        self.credential_schedule_start
    }
}

/// Safe failure when an immutable Snapshot cannot supply an explainable Route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteExplainError {
    /// The requested Route is not present in the exact immutable Snapshot.
    UnknownRoute,
    /// A valid Route unexpectedly lacked its precompiled bounded schedule.
    MissingRouteSchedule,
}

impl fmt::Display for RouteExplainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRoute => formatter.write_str("route explain route is unknown"),
            Self::MissingRouteSchedule => {
                formatter.write_str("route explain route lacks a compiled schedule")
            }
        }
    }
}

impl Error for RouteExplainError {}

/// One complete, bounded Route Explain result for a fixed input and runtime-observation time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteExplainSnapshot {
    route_id: RouteId,
    observed_at_ms: i64,
    candidates: Vec<RouteExplainCandidate>,
    projected_selection: Option<RouteExplainProjectedSelection>,
}

impl RouteExplainSnapshot {
    /// Returns the exact Route evaluated by this snapshot.
    #[must_use]
    pub fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    /// Returns the explicit time used for all dynamic-state observations.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Returns one explanation for every immutable Route Candidate in stable Snapshot order.
    #[must_use]
    pub fn candidates(&self) -> &[RouteExplainCandidate] {
        &self.candidates
    }

    /// Returns the side-effect-free projected policy choice, when one currently exists.
    ///
    /// This is not a lease and not a promise of a later real-request result: concurrent requests
    /// may change pool capacity after this snapshot is observed.
    #[must_use]
    pub fn projected_selection(&self) -> Option<&RouteExplainProjectedSelection> {
        self.projected_selection.as_ref()
    }
}

/// A deterministic policy projection over one eligible Candidate and Credential.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteExplainProjectedSelection {
    candidate_id: RouteCandidateId,
    endpoint_id: EndpointId,
    upstream_id: UpstreamId,
    upstream_model: String,
    credential_id: CredentialId,
}

impl RouteExplainProjectedSelection {
    /// Returns the exact immutable Candidate identity.
    #[must_use]
    pub fn candidate_id(&self) -> &RouteCandidateId {
        &self.candidate_id
    }

    /// Returns the selected protocol-specific Endpoint identity.
    #[must_use]
    pub fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the selected Upstream identity.
    #[must_use]
    pub fn upstream_id(&self) -> &UpstreamId {
        &self.upstream_id
    }

    /// Returns the exact non-secret upstream model label.
    #[must_use]
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// Returns the selected non-secret Credential identity.
    #[must_use]
    pub fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }
}

/// One Candidate's immutable metadata, binding observations, and exclusion reasons.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteExplainCandidate {
    candidate_id: RouteCandidateId,
    endpoint_id: EndpointId,
    upstream_id: UpstreamId,
    upstream_model: String,
    priority: i64,
    weight: i64,
    catalog_admission: SnapshotCatalogAdmission,
    active_binding_count: usize,
    reasons: Vec<RouteExplainCandidateReason>,
    credentials: Vec<RouteExplainCredential>,
}

impl RouteExplainCandidate {
    /// Returns the stable Candidate identity.
    #[must_use]
    pub fn candidate_id(&self) -> &RouteCandidateId {
        &self.candidate_id
    }

    /// Returns the Candidate's immutable Endpoint identity.
    #[must_use]
    pub fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the Candidate's immutable Upstream identity.
    #[must_use]
    pub fn upstream_id(&self) -> &UpstreamId {
        &self.upstream_id
    }

    /// Returns the Candidate's exact non-secret upstream model label.
    #[must_use]
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// Returns the immutable lower-is-better Route priority.
    #[must_use]
    pub const fn priority(&self) -> i64 {
        self.priority
    }

    /// Returns the immutable positive Route scheduling weight.
    #[must_use]
    pub const fn weight(&self) -> i64 {
        self.weight
    }

    /// Returns the compiler-time Catalog admission state.
    #[must_use]
    pub const fn catalog_admission(&self) -> SnapshotCatalogAdmission {
        self.catalog_admission
    }

    /// Returns the compiler-time number of active bindings.
    #[must_use]
    pub const fn active_binding_count(&self) -> usize {
        self.active_binding_count
    }

    /// Returns all Candidate-level exclusion reasons.
    #[must_use]
    pub fn reasons(&self) -> &[RouteExplainCandidateReason] {
        &self.reasons
    }

    /// Returns secret-free binding observations in stable Credential-ID order.
    #[must_use]
    pub fn credentials(&self) -> &[RouteExplainCredential] {
        &self.credentials
    }

    /// Returns whether this Candidate has no Candidate-level reason and one eligible binding.
    #[must_use]
    pub fn is_eligible(&self) -> bool {
        self.reasons.is_empty()
            && self
                .credentials
                .iter()
                .any(RouteExplainCredential::is_eligible)
    }

    fn credential_is_eligible(&self, credential_id: &CredentialId) -> bool {
        self.credentials.iter().any(|credential| {
            credential.credential_id() == credential_id && credential.is_eligible()
        })
    }
}

/// Safe Candidate-level exclusion reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteExplainCandidateReason {
    /// The immutable Snapshot no longer considers this Candidate hard-eligible.
    NotHardEligible,
    /// Endpoint runtime Health is cooling or has an open Circuit.
    EndpointHealth(RuntimeHealthAvailability),
    /// Endpoint Health could not be read, so the diagnostic mirrors fail-closed scheduling.
    EndpointHealthUnavailable,
    /// No matching Endpoint Credential pool exists in the exact runtime assembly.
    MissingCredentialPool,
    /// No currently eligible binding remains after per-Credential checks.
    NoEligibleCredential,
}

/// One secret-free binding observation for a Candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteExplainCredential {
    credential_id: CredentialId,
    priority: i64,
    weight: usize,
    maximum_concurrency: usize,
    active_leases: usize,
    reasons: Vec<RouteExplainCredentialReason>,
}

impl RouteExplainCredential {
    /// Returns the stable non-secret Credential identity.
    #[must_use]
    pub fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the immutable lower-is-better pool priority.
    #[must_use]
    pub const fn priority(&self) -> i64 {
        self.priority
    }

    /// Returns the immutable positive pool weight.
    #[must_use]
    pub const fn weight(&self) -> usize {
        self.weight
    }

    /// Returns the immutable concurrency limit.
    #[must_use]
    pub const fn maximum_concurrency(&self) -> usize {
        self.maximum_concurrency
    }

    /// Returns the point-in-time active lease count.
    #[must_use]
    pub const fn active_leases(&self) -> usize {
        self.active_leases
    }

    /// Returns all exact binding-level exclusion reasons.
    #[must_use]
    pub fn reasons(&self) -> &[RouteExplainCredentialReason] {
        &self.reasons
    }

    /// Returns whether no binding-level exclusion applies at the requested time.
    #[must_use]
    pub fn is_eligible(&self) -> bool {
        self.reasons.is_empty()
    }
}

/// Safe binding-level exclusion reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteExplainCredentialReason {
    /// This exact Candidate/Credential pair was excluded by the current request's prior attempt.
    RequestExcluded,
    /// The point-in-time active lease count reached the immutable concurrency maximum.
    Saturated,
    /// Exact Endpoint/Credential Health is cooling or Circuit-open.
    CredentialHealth(RuntimeHealthAvailability),
    /// Exact Endpoint/Credential/model Health is cooling or Circuit-open.
    ModelHealth(RuntimeHealthAvailability),
    /// Exact Endpoint/Credential Health could not be read and therefore fails closed.
    CredentialHealthUnavailable,
    /// Exact Endpoint/Credential/model Health could not be read and therefore fails closed.
    ModelHealthUnavailable,
    /// Exact Endpoint/Credential quota blocks this binding.
    BindingQuota(RuntimeQuotaAvailability),
    /// Exact Endpoint/Credential/model quota blocks this Candidate's model.
    ModelQuota(RuntimeQuotaAvailability),
    /// Exact Endpoint/Credential quota could not be read and therefore fails closed.
    BindingQuotaUnavailable,
    /// Exact Endpoint/Credential/model quota could not be formed/read and therefore fails closed.
    ModelQuotaUnavailable,
}

/// Builds one explain snapshot without changing scheduler or pool state.
pub(crate) fn explain(
    snapshot: &RouteSnapshot,
    credential_pools: &EndpointCredentialPools,
    runtime_health: &RuntimeHealthRegistry,
    runtime_quota: &RuntimeQuotaRegistry,
    input: &RouteExplainInput,
    exclusions: &AttemptExclusionSet,
) -> Result<RouteExplainSnapshot, RouteExplainError> {
    let route = snapshot
        .route(input.route_id())
        .ok_or(RouteExplainError::UnknownRoute)?;
    let schedule = snapshot
        .route_schedule(input.route_id())
        .ok_or(RouteExplainError::MissingRouteSchedule)?;
    let candidates: Vec<_> = route
        .candidates()
        .iter()
        .map(|candidate| {
            explain_candidate(
                candidate,
                credential_pools,
                runtime_health,
                runtime_quota,
                input.observed_at_ms(),
                exclusions,
            )
        })
        .collect();
    let projected_selection = project_selection(
        schedule,
        &candidates,
        credential_pools,
        input.candidate_schedule_start(),
        input.credential_schedule_start(),
    );

    Ok(RouteExplainSnapshot {
        route_id: input.route_id().clone(),
        observed_at_ms: input.observed_at_ms(),
        candidates,
        projected_selection,
    })
}

#[allow(clippy::too_many_arguments)] // One exact binding evaluation makes every fail-closed reason explicit.
fn explain_candidate(
    candidate: &crate::SnapshotRouteCandidate,
    credential_pools: &EndpointCredentialPools,
    runtime_health: &RuntimeHealthRegistry,
    runtime_quota: &RuntimeQuotaRegistry,
    observed_at_ms: i64,
    exclusions: &AttemptExclusionSet,
) -> RouteExplainCandidate {
    let mut reasons = Vec::new();
    if !candidate.is_hard_eligible() {
        reasons.push(RouteExplainCandidateReason::NotHardEligible);
    }
    match runtime_health.availability_at(
        &RuntimeHealthKey::endpoint(candidate.endpoint_id().clone()),
        observed_at_ms,
    ) {
        Ok(availability) if availability.is_available() => {}
        Ok(availability) => reasons.push(RouteExplainCandidateReason::EndpointHealth(availability)),
        Err(_) => reasons.push(RouteExplainCandidateReason::EndpointHealthUnavailable),
    }

    let credentials = credential_pools.pool(candidate.endpoint_id()).map(|pool| {
        pool.diagnostic_entries()
            .iter()
            .map(|entry| {
                explain_credential(
                    candidate,
                    entry,
                    runtime_health,
                    runtime_quota,
                    observed_at_ms,
                    exclusions,
                )
            })
            .collect::<Vec<_>>()
    });
    let credentials = if let Some(credentials) = credentials {
        credentials
    } else {
        reasons.push(RouteExplainCandidateReason::MissingCredentialPool);
        Vec::new()
    };
    if !credentials.iter().any(RouteExplainCredential::is_eligible) {
        reasons.push(RouteExplainCandidateReason::NoEligibleCredential);
    }

    RouteExplainCandidate {
        candidate_id: candidate.id().clone(),
        endpoint_id: candidate.endpoint_id().clone(),
        upstream_id: candidate.upstream_id().clone(),
        upstream_model: candidate.upstream_model().to_owned(),
        priority: candidate.priority(),
        weight: candidate.weight(),
        catalog_admission: candidate.catalog_admission(),
        active_binding_count: candidate.active_binding_count(),
        reasons,
        credentials,
    }
}

fn explain_credential(
    candidate: &crate::SnapshotRouteCandidate,
    entry: &CredentialPoolEntrySnapshot,
    runtime_health: &RuntimeHealthRegistry,
    runtime_quota: &RuntimeQuotaRegistry,
    observed_at_ms: i64,
    exclusions: &AttemptExclusionSet,
) -> RouteExplainCredential {
    let mut reasons = Vec::new();
    let credential_id = entry.credential_id();
    if exclusions.contains(candidate, credential_id) {
        reasons.push(RouteExplainCredentialReason::RequestExcluded);
    }
    if entry.is_saturated() {
        reasons.push(RouteExplainCredentialReason::Saturated);
    }
    push_health_reason(
        &mut reasons,
        runtime_health.availability_at(
            &RuntimeHealthKey::endpoint_credential(
                candidate.endpoint_id().clone(),
                credential_id.clone(),
            ),
            observed_at_ms,
        ),
        RouteExplainCredentialReason::CredentialHealth,
        RouteExplainCredentialReason::CredentialHealthUnavailable,
    );
    push_health_reason(
        &mut reasons,
        runtime_health.availability_at(
            &RuntimeHealthKey::endpoint_credential_model(
                candidate.endpoint_id().clone(),
                credential_id.clone(),
                candidate.upstream_model(),
            ),
            observed_at_ms,
        ),
        RouteExplainCredentialReason::ModelHealth,
        RouteExplainCredentialReason::ModelHealthUnavailable,
    );
    push_quota_reason(
        &mut reasons,
        runtime_quota
            .availability_at(
                &RuntimeQuotaTarget::endpoint_credential(
                    candidate.endpoint_id().clone(),
                    credential_id.clone(),
                ),
                observed_at_ms,
            )
            .map_err(|_| ()),
        RouteExplainCredentialReason::BindingQuota,
        RouteExplainCredentialReason::BindingQuotaUnavailable,
    );
    let model_quota = RuntimeQuotaTarget::endpoint_credential_model(
        candidate.endpoint_id().clone(),
        credential_id.clone(),
        candidate.upstream_model(),
    )
    .map_err(|_| ())
    .and_then(|target| {
        runtime_quota
            .availability_at(&target, observed_at_ms)
            .map_err(|_| ())
    });
    push_quota_reason(
        &mut reasons,
        model_quota,
        RouteExplainCredentialReason::ModelQuota,
        RouteExplainCredentialReason::ModelQuotaUnavailable,
    );

    RouteExplainCredential {
        credential_id: credential_id.clone(),
        priority: entry.priority(),
        weight: entry.weight(),
        maximum_concurrency: entry.maximum_concurrency(),
        active_leases: entry.active_leases(),
        reasons,
    }
}

fn push_health_reason(
    reasons: &mut Vec<RouteExplainCredentialReason>,
    availability: Result<RuntimeHealthAvailability, crate::RuntimeHealthError>,
    blocked: impl FnOnce(RuntimeHealthAvailability) -> RouteExplainCredentialReason,
    unavailable: RouteExplainCredentialReason,
) {
    match availability {
        Ok(availability) if availability.is_available() => {}
        Ok(availability) => reasons.push(blocked(availability)),
        Err(_) => reasons.push(unavailable),
    }
}

fn push_quota_reason(
    reasons: &mut Vec<RouteExplainCredentialReason>,
    availability: Result<RuntimeQuotaAvailability, ()>,
    blocked: impl FnOnce(RuntimeQuotaAvailability) -> RouteExplainCredentialReason,
    unavailable: RouteExplainCredentialReason,
) {
    match availability {
        Ok(availability) if availability.is_available() => {}
        Ok(availability) => reasons.push(blocked(availability)),
        Err(()) => reasons.push(unavailable),
    }
}

fn project_selection(
    schedule: &crate::SnapshotRouteSchedule,
    candidates: &[RouteExplainCandidate],
    credential_pools: &EndpointCredentialPools,
    candidate_schedule_start: usize,
    credential_schedule_start: usize,
) -> Option<RouteExplainProjectedSelection> {
    for tier in schedule.priority_tiers() {
        let slot_indexes = tier.slot_indexes();
        for offset in 0..slot_indexes.len() {
            let slot_index = candidate_schedule_start.wrapping_add(offset) % slot_indexes.len();
            let candidate_index = *slot_indexes.get(slot_index)?;
            let candidate = candidates.get(candidate_index)?;
            if !candidate.is_eligible() {
                continue;
            }
            let Some(pool) = credential_pools.pool(candidate.endpoint_id()) else {
                continue;
            };
            let Some(credential) = pool
                .peek_eligible_from(credential_schedule_start, |credential_id| {
                    candidate.credential_is_eligible(credential_id)
                })
            else {
                // A concurrent lease can make a previously observed eligible binding unavailable.
                // Keep this projection fail-closed while allowing another Candidate to be shown.
                continue;
            };
            return Some(RouteExplainProjectedSelection {
                candidate_id: candidate.candidate_id().clone(),
                endpoint_id: candidate.endpoint_id().clone(),
                upstream_id: candidate.upstream_id().clone(),
                upstream_model: candidate.upstream_model().to_owned(),
                credential_id: credential.credential_id().clone(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        sync::{
            Arc,
            atomic::{AtomicI64, Ordering},
        },
    };

    use gateway_catalog::{CapabilitySet, CatalogModelState};
    use gateway_core::{
        CredentialId, EndpointId, PublicModelId, RouteCandidateId, RouteId, UpstreamId,
    };
    use gateway_upstream::{
        CredentialSecret, EndpointCredentialInput, EndpointCredentialPool, EndpointCredentialPools,
    };

    use super::{
        RouteExplainCandidateReason, RouteExplainCredentialReason, RouteExplainError,
        RouteExplainInput,
    };
    use crate::{
        AttemptExclusionSet, QuotaConfidence, QuotaSnapshot, QuotaSource, QuotaWindow,
        RouteCredentialScheduler, RouteSnapshot, RouteSnapshotInput, RuntimeHealthAvailability,
        RuntimeHealthClock, RuntimeHealthClockError, RuntimeHealthKey, RuntimeHealthRegistry,
        RuntimeQuotaAvailability, RuntimeQuotaRegistry, RuntimeQuotaTarget,
        SnapshotCatalogAdmission, SnapshotPublicModel, SnapshotRoute, SnapshotRouteCandidate,
        SnapshotRouteCandidateInput, SnapshotRoutePolicy, SnapshotTransformMode, SnapshotVersion,
    };

    type TestResult = Result<(), Box<dyn Error>>;
    type CandidateSpec<'a> = (&'a str, &'a str, &'a str, i64, i64);
    type CredentialSpec<'a> = (&'a str, i64, i64, i64);
    type PoolSpec<'a> = (&'a str, Vec<CredentialSpec<'a>>);

    #[test]
    fn fixed_explain_reports_exact_health_and_quota_reasons_without_a_lease() -> TestResult {
        let (scheduler, route_id, pools) = scheduler(
            vec![
                ("candidate-a", "endpoint-a", "model-a", 0, 1),
                ("candidate-b", "endpoint-b", "model-b", 0, 1),
                ("candidate-c", "endpoint-c", "model-c", 1, 1),
            ],
            vec![
                ("endpoint-a", vec![("credential-a", 0, 1, 1)]),
                ("endpoint-b", vec![("credential-b", 0, 1, 1)]),
                ("endpoint-c", vec![("credential-c", 0, 1, 1)]),
            ],
            SnapshotRoutePolicy::PriorityFailover,
        )?;
        let clock = Arc::new(FixedClock::new(100));
        let health = RuntimeHealthRegistry::with_clock(clock.clone());
        let quota = RuntimeQuotaRegistry::with_clock(clock);
        health.cool_down_until(
            RuntimeHealthKey::endpoint(EndpointId::try_new("endpoint-a")?),
            200,
        )?;
        health.mark_credential_forbidden(
            EndpointId::try_new("endpoint-b")?,
            CredentialId::try_new("credential-b")?,
        )?;
        let quota_target = RuntimeQuotaTarget::endpoint_credential_model(
            EndpointId::try_new("endpoint-b")?,
            CredentialId::try_new("credential-b")?,
            "model-b",
        )?;
        quota.record_snapshot(QuotaSnapshot::try_new(
            quota_target,
            vec![QuotaWindow::try_new(
                "requests",
                Some(10),
                Some(0),
                Some(200),
            )?],
            QuotaSource::Header,
            QuotaConfidence::Observed,
            100,
        )?)?;

        let explain = scheduler.explain(
            &RouteExplainInput::new(route_id.clone(), 100),
            &health,
            &quota,
            &AttemptExclusionSet::new(),
        )?;
        let candidate_a = explain
            .candidates()
            .iter()
            .find(|candidate| candidate.candidate_id().as_str() == "candidate-a")
            .ok_or("candidate-a was not explained")?;
        assert_eq!(
            candidate_a.reasons(),
            &[RouteExplainCandidateReason::EndpointHealth(
                RuntimeHealthAvailability::CoolingDown { until_ms: 200 }
            )]
        );
        let candidate_b = explain
            .candidates()
            .iter()
            .find(|candidate| candidate.candidate_id().as_str() == "candidate-b")
            .ok_or("candidate-b was not explained")?;
        assert_eq!(
            candidate_b.credentials()[0].reasons(),
            &[
                RouteExplainCredentialReason::CredentialHealth(
                    RuntimeHealthAvailability::AccountForbidden
                ),
                RouteExplainCredentialReason::ModelQuota(RuntimeQuotaAvailability::Exhausted {
                    reset_at_ms: 200
                })
            ]
        );
        assert_eq!(
            candidate_b.reasons(),
            &[RouteExplainCandidateReason::NoEligibleCredential]
        );
        let projected = explain
            .projected_selection()
            .ok_or("healthy fallback was not projected")?;
        assert_eq!(projected.candidate_id().as_str(), "candidate-c");
        assert_eq!(projected.credential_id().as_str(), "credential-c");
        assert_eq!(
            pools
                .pool(&EndpointId::try_new("endpoint-c")?)
                .ok_or("missing endpoint-c pool")?
                .active_lease_count(&CredentialId::try_new("credential-c")?),
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn fixed_schedule_starts_never_advance_live_candidate_or_credential_cursors() -> TestResult {
        let (scheduler, route_id, pools) = scheduler(
            vec![
                ("candidate-a", "endpoint-a", "model-a", 0, 1),
                ("candidate-b", "endpoint-b", "model-b", 0, 1),
            ],
            vec![
                (
                    "endpoint-a",
                    vec![("credential-a", 0, 1, 1), ("credential-b", 0, 1, 1)],
                ),
                ("endpoint-b", vec![("credential-c", 0, 1, 1)]),
            ],
            SnapshotRoutePolicy::RoundRobin,
        )?;
        let health = RuntimeHealthRegistry::new();
        let quota = RuntimeQuotaRegistry::new();
        let candidate_projection = scheduler.explain(
            &RouteExplainInput::with_schedule_starts(route_id.clone(), 100, 1, 0),
            &health,
            &quota,
            &AttemptExclusionSet::new(),
        )?;
        assert_eq!(
            candidate_projection
                .projected_selection()
                .ok_or("expected a projected selection")?
                .candidate_id()
                .as_str(),
            "candidate-b"
        );
        let credential_projection = scheduler.explain(
            &RouteExplainInput::with_schedule_starts(route_id.clone(), 100, 0, 1),
            &health,
            &quota,
            &AttemptExclusionSet::new(),
        )?;
        assert_eq!(
            credential_projection
                .projected_selection()
                .ok_or("expected a projected selection")?
                .credential_id()
                .as_str(),
            "credential-b"
        );

        let selected = scheduler.select_eligible_and_lease_with_runtime_health_quota_and_binding(
            &route_id,
            &health,
            &quota,
            |_| true,
            |_, _| true,
        )?;
        assert_eq!(selected.candidate().id().as_str(), "candidate-a");
        assert_eq!(selected.lease().credential_id().as_str(), "credential-a");
        drop(selected);
        assert_eq!(
            pools
                .pool(&EndpointId::try_new("endpoint-a")?)
                .ok_or("missing endpoint-a pool")?
                .active_lease_count(&CredentialId::try_new("credential-a")?),
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn saturated_binding_is_explained_and_a_sibling_is_projected_without_a_new_lease() -> TestResult
    {
        let (scheduler, route_id, pools) = scheduler(
            vec![("candidate-a", "endpoint-a", "model-a", 0, 1)],
            vec![(
                "endpoint-a",
                vec![("credential-a", 0, 1, 1), ("credential-b", 0, 1, 1)],
            )],
            SnapshotRoutePolicy::RoundRobin,
        )?;
        let pool = pools
            .pool(&EndpointId::try_new("endpoint-a")?)
            .ok_or("missing endpoint-a pool")?;
        let held = pool.try_lease().ok_or("expected the initial lease")?;
        assert_eq!(held.credential_id().as_str(), "credential-a");

        let health = RuntimeHealthRegistry::new();
        let quota = RuntimeQuotaRegistry::new();
        let explain = scheduler.explain(
            &RouteExplainInput::new(route_id, 100),
            &health,
            &quota,
            &AttemptExclusionSet::new(),
        )?;
        let credentials = explain.candidates()[0].credentials();
        assert_eq!(
            credentials
                .iter()
                .find(|credential| credential.credential_id().as_str() == "credential-a")
                .ok_or("credential-a was not explained")?
                .reasons(),
            &[RouteExplainCredentialReason::Saturated]
        );
        assert_eq!(
            explain
                .projected_selection()
                .ok_or("sibling was not projected")?
                .credential_id()
                .as_str(),
            "credential-b"
        );
        assert_eq!(
            pool.active_lease_count(&CredentialId::try_new("credential-b")?),
            Some(0)
        );
        drop(held);
        Ok(())
    }

    #[test]
    fn request_local_exclusion_is_exact_and_unknown_route_stays_safe() -> TestResult {
        let (scheduler, route_id, _pools) = scheduler(
            vec![("candidate-a", "endpoint-a", "model-a", 0, 1)],
            vec![("endpoint-a", vec![("credential-a", 0, 1, 1)])],
            SnapshotRoutePolicy::RoundRobin,
        )?;
        let health = RuntimeHealthRegistry::new();
        let quota = RuntimeQuotaRegistry::new();
        let candidate = scheduler
            .route(&route_id)
            .ok_or("missing route")?
            .candidates()[0]
            .clone();
        let mut exclusions = AttemptExclusionSet::new();
        exclusions.insert(&candidate, &CredentialId::try_new("credential-a")?);
        let explain = scheduler.explain(
            &RouteExplainInput::new(route_id, 100),
            &health,
            &quota,
            &exclusions,
        )?;
        assert_eq!(
            explain.candidates()[0].credentials()[0].reasons(),
            &[RouteExplainCredentialReason::RequestExcluded]
        );
        assert_eq!(explain.projected_selection(), None);
        assert_eq!(
            scheduler.explain(
                &RouteExplainInput::new(RouteId::try_new("missing-route")?, 100),
                &health,
                &quota,
                &AttemptExclusionSet::new(),
            ),
            Err(RouteExplainError::UnknownRoute)
        );
        Ok(())
    }

    fn scheduler<'a>(
        candidate_specs: Vec<CandidateSpec<'a>>,
        pool_specs: Vec<PoolSpec<'a>>,
        policy: SnapshotRoutePolicy,
    ) -> Result<
        (
            RouteCredentialScheduler,
            RouteId,
            Arc<EndpointCredentialPools>,
        ),
        Box<dyn Error>,
    > {
        let route_id = RouteId::try_new("route-a")?;
        let public_model_id = PublicModelId::try_new("public-model-a")?;
        let candidates = candidate_specs
            .into_iter()
            .map(
                |(candidate_id, endpoint_id, upstream_model, priority, weight)| {
                    candidate(candidate_id, endpoint_id, upstream_model, priority, weight)
                },
            )
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
                policy,
                3,
                10_000,
                candidates,
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let pools = pool_specs
            .into_iter()
            .map(|(endpoint_id, entries)| endpoint_pool(endpoint_id, entries))
            .collect::<Result<Vec<_>, _>>()?;
        let pools = Arc::new(EndpointCredentialPools::try_new(pools)?);
        Ok((
            RouteCredentialScheduler::new(snapshot, Arc::clone(&pools)),
            route_id,
            pools,
        ))
    }

    fn candidate(
        candidate_id: &str,
        endpoint_id: &str,
        upstream_model: &str,
        priority: i64,
        weight: i64,
    ) -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
        Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(candidate_id)?,
            endpoint_id: EndpointId::try_new(endpoint_id)?,
            upstream_id: UpstreamId::try_new(format!("upstream-{endpoint_id}"))?,
            upstream_model: upstream_model.to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority,
            weight,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
            active_binding_count: 1,
        }))
    }

    fn endpoint_pool(
        endpoint_id: &str,
        entries: Vec<CredentialSpec<'_>>,
    ) -> Result<EndpointCredentialPool, Box<dyn Error>> {
        let entries = entries
            .into_iter()
            .map(|(credential_id, priority, weight, concurrency)| {
                Ok(EndpointCredentialInput {
                    credential_id: CredentialId::try_new(credential_id)?,
                    credential_kind: "api_key".to_owned(),
                    credential_revision: 0,
                    priority,
                    weight,
                    concurrency,
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

    #[derive(Debug)]
    struct FixedClock {
        now_ms: AtomicI64,
    }

    impl FixedClock {
        const fn new(now_ms: i64) -> Self {
            Self {
                now_ms: AtomicI64::new(now_ms),
            }
        }
    }

    impl RuntimeHealthClock for FixedClock {
        fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
            Ok(self.now_ms.load(Ordering::Acquire))
        }
    }
}
