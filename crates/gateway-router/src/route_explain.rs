//! Bounded, side-effect-free Route-Candidate eligibility explanations.
//!
//! This module reads one immutable Route Snapshot plus non-secret runtime observations. It never
//! acquires a Credential lease, advances a scheduler cursor, opens a Circuit/Quota probe, queries
//! persistence, or executes a Provider request.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use gateway_core::{CredentialId, EndpointId, ProviderId, RouteCandidateId, RouteId, UpstreamId};
use gateway_upstream::{CredentialPoolEntrySnapshot, EndpointCredentialPools};

use crate::{
    AttemptExclusionSet, ProviderScopedCandidate, ProviderScopedHealth, ProviderScopedQuota,
    ProviderScopedSelection, ProviderScopedSelector, ProviderScopedSelectorError, RouteSnapshot,
    RuntimeHealthAvailability, RuntimeHealthKey, RuntimeHealthRegistry, RuntimeQuotaAvailability,
    RuntimeQuotaRegistry, RuntimeQuotaTarget, SnapshotCatalogAdmission,
};

const MAX_PROVIDER_SCOPED_EXPLAIN_ITEMS: usize = 4_096;

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

/// Safe construction or composition failure for a Provider-scoped Route Explain snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderScopedRouteExplainError {
    /// The immutable Route Explain base could not resolve its exact Route/schedule.
    Route(RouteExplainError),
    /// The admitted-ID or cost input exceeded the finite composition bound.
    TooManyItems,
    /// A Provider identity or aggregate scheduling observation failed closed validation.
    InvalidObservation,
    /// A runtime Quota observation could not be read coherently.
    QuotaUnavailable,
    /// The deterministic Provider-scoped selector rejected malformed or duplicate observations.
    Selector(ProviderScopedSelectorError),
}

impl fmt::Display for ProviderScopedRouteExplainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Route(_) => "provider-scoped route explain base is unavailable",
            Self::TooManyItems => "provider-scoped route explain input exceeds its finite bound",
            Self::InvalidObservation => "provider-scoped route explain observation is invalid",
            Self::QuotaUnavailable => "provider-scoped route explain quota is unavailable",
            Self::Selector(_) => "provider-scoped route explain selection failed",
        })
    }
}

impl Error for ProviderScopedRouteExplainError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Route(error) => Some(error),
            Self::Selector(error) => Some(error),
            Self::TooManyItems | Self::InvalidObservation | Self::QuotaUnavailable => None,
        }
    }
}

impl From<RouteExplainError> for ProviderScopedRouteExplainError {
    fn from(error: RouteExplainError) -> Self {
        Self::Route(error)
    }
}

impl From<ProviderScopedSelectorError> for ProviderScopedRouteExplainError {
    fn from(error: ProviderScopedSelectorError) -> Self {
        Self::Selector(error)
    }
}

/// Explicit, bounded inputs for one Provider-scoped composition over an ordinary Route Explain.
///
/// `admitted_candidate_ids` is supplied by the management/protocol boundary after capability
/// admission. An empty `candidate_cost_microunits` map means cost is unknown; this API never
/// substitutes unknown cost with zero.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderScopedRouteExplainInput {
    route_explain: RouteExplainInput,
    provider_id: ProviderId,
    admitted_candidate_ids: BTreeSet<RouteCandidateId>,
    candidate_cost_microunits: BTreeMap<RouteCandidateId, u64>,
}

impl ProviderScopedRouteExplainInput {
    /// Creates one exact Provider-scoped diagnostic input.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderScopedRouteExplainError::TooManyItems`] when either caller-owned map
    /// exceeds the same bounded domain as the Provider selector, or a selector validation error
    /// when the Provider identity is not safe for this policy seam.
    pub fn try_new(
        route_explain: RouteExplainInput,
        provider_id: ProviderId,
        admitted_candidate_ids: BTreeSet<RouteCandidateId>,
        candidate_cost_microunits: BTreeMap<RouteCandidateId, u64>,
    ) -> Result<Self, ProviderScopedRouteExplainError> {
        if admitted_candidate_ids.len() > MAX_PROVIDER_SCOPED_EXPLAIN_ITEMS
            || candidate_cost_microunits.len() > MAX_PROVIDER_SCOPED_EXPLAIN_ITEMS
        {
            return Err(ProviderScopedRouteExplainError::TooManyItems);
        }
        ProviderScopedSelector::try_new(provider_id.clone())?;
        Ok(Self {
            route_explain,
            provider_id,
            admitted_candidate_ids,
            candidate_cost_microunits,
        })
    }

    /// Returns the ordinary immutable Route Explain input used as the diagnostic base.
    #[must_use]
    pub const fn route_explain(&self) -> &RouteExplainInput {
        &self.route_explain
    }

    /// Returns the exact requested Provider scope.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns the protocol/capability-admitted Candidate identities.
    #[must_use]
    pub const fn admitted_candidate_ids(&self) -> &BTreeSet<RouteCandidateId> {
        &self.admitted_candidate_ids
    }

    /// Returns caller-supplied, versioned Candidate costs; absent entries remain unknown.
    #[must_use]
    pub const fn candidate_cost_microunits(&self) -> &BTreeMap<RouteCandidateId, u64> {
        &self.candidate_cost_microunits
    }
}

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

/// One ordinary Route Explain snapshot plus its Provider-scoped policy projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderScopedRouteExplainSnapshot {
    base: RouteExplainSnapshot,
    provider_selection: ProviderScopedSelection,
}

impl ProviderScopedRouteExplainSnapshot {
    /// Returns the complete existing Route Explain evidence for every immutable Candidate.
    #[must_use]
    pub const fn base(&self) -> &RouteExplainSnapshot {
        &self.base
    }

    /// Returns the exact Provider-scoped deterministic selection over admitted eligible rows.
    #[must_use]
    pub const fn provider_selection(&self) -> &ProviderScopedSelection {
        &self.provider_selection
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
    expires_at_ms: Option<i64>,
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

    /// Returns the optional absolute Credential expiry retained by the exact runtime pool.
    #[must_use]
    pub const fn expires_at_ms(&self) -> Option<i64> {
        self.expires_at_ms
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
    /// The absolute Credential expiry is at or before the explicit observation time.
    Expired,
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
    let expires_at_ms = entry.expires_at_ms();
    if expires_at_ms.is_some_and(|expires_at_ms| expires_at_ms <= observed_at_ms) {
        reasons.push(RouteExplainCredentialReason::Expired);
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
        expires_at_ms,
        active_leases: entry.active_leases(),
        reasons,
    }
}

/// Builds a Provider-scoped Route Explain projection from the same immutable base evidence.
///
/// Only Candidates that are both base-eligible and explicitly admitted are handed to the
/// Provider selector. Provider identity is derived directly from each Candidate's immutable
/// `upstream_id`; no Endpoint adapter or implicit fallback map is consulted. The function only
/// reads pool diagnostics and runtime Health/Quota state, so it never advances a cursor or acquires
/// a lease.
pub(crate) fn explain_provider_scoped(
    snapshot: &RouteSnapshot,
    credential_pools: &EndpointCredentialPools,
    runtime_health: &RuntimeHealthRegistry,
    runtime_quota: &RuntimeQuotaRegistry,
    input: &ProviderScopedRouteExplainInput,
    exclusions: &AttemptExclusionSet,
) -> Result<ProviderScopedRouteExplainSnapshot, ProviderScopedRouteExplainError> {
    let base = explain(
        snapshot,
        credential_pools,
        runtime_health,
        runtime_quota,
        input.route_explain(),
        exclusions,
    )?;
    let selector = ProviderScopedSelector::try_new(input.provider_id().clone())?;
    let mut observations = Vec::new();
    for candidate in base.candidates().iter().filter(|candidate| {
        candidate.is_eligible()
            && input
                .admitted_candidate_ids()
                .contains(candidate.candidate_id())
    }) {
        let provider_id = ProviderId::try_new(candidate.upstream_id().as_str().to_owned())
            .map_err(|_| ProviderScopedRouteExplainError::InvalidObservation)?;
        let (active_leases, max_concurrency) = aggregate_capacity(candidate)?;
        let quota = aggregate_quota(candidate, runtime_quota, base.observed_at_ms())?;
        let weight = u32::try_from(candidate.weight())
            .map_err(|_| ProviderScopedRouteExplainError::InvalidObservation)?;
        observations.push(ProviderScopedCandidate::try_new(
            provider_id,
            candidate.endpoint_id().clone(),
            candidate.candidate_id().clone(),
            candidate.priority(),
            weight,
            active_leases,
            max_concurrency,
            input
                .candidate_cost_microunits()
                .get(candidate.candidate_id())
                .copied(),
            ProviderScopedHealth::Available,
            quota,
            true,
            false,
        )?);
    }
    let provider_selection = selector.select(observations)?;
    Ok(ProviderScopedRouteExplainSnapshot {
        base,
        provider_selection,
    })
}

fn aggregate_capacity(
    candidate: &RouteExplainCandidate,
) -> Result<(u32, u32), ProviderScopedRouteExplainError> {
    let mut active_leases = 0_u32;
    let mut max_concurrency = 0_u32;
    for credential in candidate
        .credentials()
        .iter()
        .filter(|credential| credential.is_eligible())
    {
        active_leases = active_leases
            .checked_add(
                u32::try_from(credential.active_leases())
                    .map_err(|_| ProviderScopedRouteExplainError::InvalidObservation)?,
            )
            .ok_or(ProviderScopedRouteExplainError::InvalidObservation)?;
        max_concurrency = max_concurrency
            .checked_add(
                u32::try_from(credential.maximum_concurrency())
                    .map_err(|_| ProviderScopedRouteExplainError::InvalidObservation)?,
            )
            .ok_or(ProviderScopedRouteExplainError::InvalidObservation)?;
    }
    if max_concurrency == 0 {
        return Err(ProviderScopedRouteExplainError::InvalidObservation);
    }
    Ok((active_leases, max_concurrency))
}

fn aggregate_quota(
    candidate: &RouteExplainCandidate,
    runtime_quota: &RuntimeQuotaRegistry,
    observed_at_ms: i64,
) -> Result<ProviderScopedQuota, ProviderScopedRouteExplainError> {
    let mut unknown = false;
    for credential in candidate
        .credentials()
        .iter()
        .filter(|credential| credential.is_eligible())
    {
        let binding_target = RuntimeQuotaTarget::endpoint_credential(
            candidate.endpoint_id().clone(),
            credential.credential_id().clone(),
        );
        let model_target = RuntimeQuotaTarget::endpoint_credential_model(
            candidate.endpoint_id().clone(),
            credential.credential_id().clone(),
            candidate.upstream_model(),
        )
        .map_err(|_| ProviderScopedRouteExplainError::QuotaUnavailable)?;
        for target in [binding_target, model_target] {
            let status = runtime_quota
                .status_at(&target, observed_at_ms)
                .map_err(|_| ProviderScopedRouteExplainError::QuotaUnavailable)?;
            match status.availability() {
                RuntimeQuotaAvailability::Available => {
                    if status.snapshot().is_none() {
                        unknown = true;
                    }
                }
                RuntimeQuotaAvailability::Exhausted { .. }
                | RuntimeQuotaAvailability::RecoveryRequired { .. } => {
                    return Ok(ProviderScopedQuota::Blocked);
                }
                RuntimeQuotaAvailability::RecoveryProbeInFlight { .. } => {
                    return Ok(ProviderScopedQuota::RecoveryInFlight);
                }
            }
        }
    }
    Ok(if unknown {
        ProviderScopedQuota::Unknown
    } else {
        ProviderScopedQuota::Available
    })
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
        collections::{BTreeMap, BTreeSet},
        error::Error,
        sync::{
            Arc,
            atomic::{AtomicI64, Ordering},
        },
    };

    use gateway_catalog::{CapabilitySet, CatalogModelState};
    use gateway_core::{
        CredentialId, EndpointId, ProviderId, PublicModelId, RouteCandidateId, RouteId, UpstreamId,
    };
    use gateway_upstream::{
        CredentialSecret, EndpointCredentialInput, EndpointCredentialPool, EndpointCredentialPools,
    };

    use super::{
        ProviderScopedRouteExplainError, ProviderScopedRouteExplainInput,
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

    #[test]
    fn provider_composition_prefers_known_quota_then_least_loaded_without_side_effects()
    -> TestResult {
        let (scheduler, route_id, pools) = scheduler_from_candidates(
            vec![
                candidate_for_provider("candidate-a", "endpoint-a", "provider-a", "model-a")?,
                candidate_for_provider("candidate-b", "endpoint-b", "provider-a", "model-b")?,
            ],
            vec![
                ("endpoint-a", vec![("credential-a", 0, 1, 4)]),
                ("endpoint-b", vec![("credential-b", 0, 1, 4)]),
            ],
            SnapshotRoutePolicy::RoundRobin,
        )?;
        let held = pools
            .pool(&EndpointId::try_new("endpoint-a")?)
            .and_then(|pool| pool.try_lease())
            .ok_or("expected one held lease")?;
        let health = RuntimeHealthRegistry::new();
        let quota = RuntimeQuotaRegistry::new();
        record_known_available_quota(&quota, "endpoint-a", "credential-a", "model-a", 100)?;
        record_known_available_quota(&quota, "endpoint-b", "credential-b", "model-b", 100)?;
        let input = ProviderScopedRouteExplainInput::try_new(
            RouteExplainInput::new(route_id.clone(), 100),
            ProviderId::try_new("provider-a")?,
            ["candidate-a", "candidate-b"]
                .into_iter()
                .map(RouteCandidateId::try_new)
                .collect::<Result<_, _>>()?,
            BTreeMap::new(),
        )?;

        let first = scheduler.explain_provider_scoped(
            &input,
            &health,
            &quota,
            &AttemptExclusionSet::new(),
        )?;
        let second = scheduler.explain_provider_scoped(
            &input,
            &health,
            &quota,
            &AttemptExclusionSet::new(),
        )?;
        assert_eq!(first, second);
        assert_eq!(
            first
                .provider_selection()
                .selected_candidate_id()
                .map(RouteCandidateId::as_str),
            Some("candidate-b")
        );
        assert_eq!(held.credential_id().as_str(), "credential-a");
        drop(held);

        let selected = scheduler.select_eligible_and_lease_with_runtime_health_quota_and_binding(
            &route_id,
            &health,
            &quota,
            |_| true,
            |_, _| true,
        )?;
        assert_eq!(selected.candidate().id().as_str(), "candidate-a");
        assert_eq!(selected.lease().credential_id().as_str(), "credential-a");
        Ok(())
    }

    #[test]
    fn expired_requested_provider_never_falls_back_to_a_foreign_provider() -> TestResult {
        let route_id = RouteId::try_new("route-a")?;
        let candidates = vec![
            candidate_for_provider("candidate-a", "endpoint-a", "provider-a", "model-a")?,
            candidate_for_provider("candidate-b", "endpoint-b", "provider-b", "model-b")?,
        ];
        let pools = vec![
            endpoint_pool_with_expiry("endpoint-a", vec![("credential-a", 0, 1, 1, Some(100))])?,
            endpoint_pool_with_expiry("endpoint-b", vec![("credential-b", 0, 1, 1, Some(101))])?,
        ];
        let (scheduler, pools) = scheduler_from_parts(
            route_id.clone(),
            candidates,
            pools,
            SnapshotRoutePolicy::RoundRobin,
        )?;
        let input = ProviderScopedRouteExplainInput::try_new(
            RouteExplainInput::new(route_id, 100),
            ProviderId::try_new("provider-a")?,
            ["candidate-a", "candidate-b"]
                .into_iter()
                .map(RouteCandidateId::try_new)
                .collect::<Result<_, _>>()?,
            BTreeMap::new(),
        )?;
        let explain = scheduler.explain_provider_scoped(
            &input,
            &RuntimeHealthRegistry::new(),
            &RuntimeQuotaRegistry::new(),
            &AttemptExclusionSet::new(),
        )?;
        let expired = explain
            .base()
            .candidates()
            .iter()
            .find(|candidate| candidate.candidate_id().as_str() == "candidate-a")
            .ok_or("expired candidate was not explained")?;
        assert_eq!(expired.credentials()[0].expires_at_ms(), Some(100));
        assert_eq!(
            expired.credentials()[0].reasons(),
            &[RouteExplainCredentialReason::Expired]
        );
        assert!(!expired.is_eligible());
        assert_eq!(explain.provider_selection().selected_candidate_id(), None);
        assert_eq!(explain.provider_selection().decisions().len(), 1);
        assert_eq!(
            explain.provider_selection().decisions()[0].rejections(),
            &[crate::ProviderScopedRejection::ProviderMismatch]
        );
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

    #[test]
    fn provider_composition_input_is_bounded_and_fails_closed() -> TestResult {
        let admitted = (0..=super::MAX_PROVIDER_SCOPED_EXPLAIN_ITEMS)
            .map(|index| RouteCandidateId::try_new(format!("candidate-{index}")))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let error = ProviderScopedRouteExplainInput::try_new(
            RouteExplainInput::new(RouteId::try_new("route-a")?, 100),
            ProviderId::try_new("provider-a")?,
            admitted,
            BTreeMap::new(),
        )
        .err()
        .ok_or("oversized admitted-candidate input was accepted")?;
        assert_eq!(error, ProviderScopedRouteExplainError::TooManyItems);

        let invalid_provider = ProviderScopedRouteExplainInput::try_new(
            RouteExplainInput::new(RouteId::try_new("route-a")?, 100),
            ProviderId::try_new("   ")?,
            BTreeSet::new(),
            BTreeMap::new(),
        )
        .err()
        .ok_or("whitespace-only provider scope was accepted")?;
        assert_eq!(
            invalid_provider,
            ProviderScopedRouteExplainError::Selector(
                crate::ProviderScopedSelectorError::InvalidCandidate
            )
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
        let candidates = candidate_specs
            .into_iter()
            .map(
                |(candidate_id, endpoint_id, upstream_model, priority, weight)| {
                    candidate(candidate_id, endpoint_id, upstream_model, priority, weight)
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        let pools = pool_specs
            .into_iter()
            .map(|(endpoint_id, entries)| endpoint_pool(endpoint_id, entries))
            .collect::<Result<Vec<_>, _>>()?;
        let route_id = RouteId::try_new("route-a")?;
        let (scheduler, pools) = scheduler_from_parts(route_id.clone(), candidates, pools, policy)?;
        Ok((scheduler, route_id, pools))
    }

    fn scheduler_from_candidates(
        candidates: Vec<SnapshotRouteCandidate>,
        pool_specs: Vec<PoolSpec<'_>>,
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
        let pools = pool_specs
            .into_iter()
            .map(|(endpoint_id, entries)| endpoint_pool(endpoint_id, entries))
            .collect::<Result<Vec<_>, _>>()?;
        let (scheduler, pools) = scheduler_from_parts(route_id.clone(), candidates, pools, policy)?;
        Ok((scheduler, route_id, pools))
    }

    fn scheduler_from_parts(
        route_id: RouteId,
        candidates: Vec<SnapshotRouteCandidate>,
        pools: Vec<EndpointCredentialPool>,
        policy: SnapshotRoutePolicy,
    ) -> Result<(RouteCredentialScheduler, Arc<EndpointCredentialPools>), Box<dyn Error>> {
        let public_model_id = PublicModelId::try_new("public-model-a")?;
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
                route_id,
                public_model_id,
                policy,
                3,
                10_000,
                candidates,
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let pools = Arc::new(EndpointCredentialPools::try_new(pools)?);
        Ok((
            RouteCredentialScheduler::new(snapshot, Arc::clone(&pools)),
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
        candidate_for_provider_with_schedule(
            candidate_id,
            endpoint_id,
            &format!("upstream-{endpoint_id}"),
            upstream_model,
            priority,
            weight,
        )
    }

    fn candidate_for_provider(
        candidate_id: &str,
        endpoint_id: &str,
        provider_id: &str,
        upstream_model: &str,
    ) -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
        candidate_for_provider_with_schedule(
            candidate_id,
            endpoint_id,
            provider_id,
            upstream_model,
            0,
            1,
        )
    }

    fn candidate_for_provider_with_schedule(
        candidate_id: &str,
        endpoint_id: &str,
        provider_id: &str,
        upstream_model: &str,
        priority: i64,
        weight: i64,
    ) -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
        Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(candidate_id)?,
            endpoint_id: EndpointId::try_new(endpoint_id)?,
            upstream_id: UpstreamId::try_new(provider_id)?,
            endpoint_api_format: "openai/responses".to_owned(),
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
        endpoint_pool_with_expiry(
            endpoint_id,
            entries
                .into_iter()
                .map(|(credential_id, priority, weight, concurrency)| {
                    (credential_id, priority, weight, concurrency, None)
                })
                .collect(),
        )
    }

    fn endpoint_pool_with_expiry(
        endpoint_id: &str,
        entries: Vec<(&str, i64, i64, i64, Option<i64>)>,
    ) -> Result<EndpointCredentialPool, Box<dyn Error>> {
        let entries = entries
            .into_iter()
            .map(
                |(credential_id, priority, weight, concurrency, expires_at_ms)| {
                    Ok(EndpointCredentialInput {
                        credential_id: CredentialId::try_new(credential_id)?,
                        credential_kind: "api_key".to_owned(),
                        credential_revision: 0,
                        priority,
                        weight,
                        concurrency,
                        expires_at_ms,
                        secret: CredentialSecret::try_new(
                            format!("synthetic-{credential_id}").into_bytes(),
                        )?,
                    })
                },
            )
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(EndpointCredentialPool::try_new(
            EndpointId::try_new(endpoint_id)?,
            entries,
        )?)
    }

    fn record_known_available_quota(
        quota: &RuntimeQuotaRegistry,
        endpoint_id: &str,
        credential_id: &str,
        model: &str,
        observed_at_ms: i64,
    ) -> Result<(), Box<dyn Error>> {
        let endpoint_id = EndpointId::try_new(endpoint_id)?;
        let credential_id = CredentialId::try_new(credential_id)?;
        for target in [
            RuntimeQuotaTarget::endpoint_credential(endpoint_id.clone(), credential_id.clone()),
            RuntimeQuotaTarget::endpoint_credential_model(
                endpoint_id.clone(),
                credential_id.clone(),
                model,
            )?,
        ] {
            quota.record_snapshot(QuotaSnapshot::try_new(
                target,
                vec![QuotaWindow::try_new("requests", Some(10), Some(10), None)?],
                QuotaSource::Header,
                QuotaConfidence::Observed,
                observed_at_ms,
            )?)?;
        }
        Ok(())
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
