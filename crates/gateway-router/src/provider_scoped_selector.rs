//! Provider-scoped deterministic candidate ranking for P13-07A.
//!
//! This module is deliberately a policy seam, not a second request scheduler. It consumes a
//! caller-supplied, secret-free view of candidate state and returns a stable ranking plus closed
//! exclusion reasons. The existing [`crate::RouteCredentialScheduler`] remains the owner of
//! request-time cursor advancement, Health/Quota reads, and Credential lease acquisition.
//!
//! Keeping this first slice side-effect free makes the cost/usage policy reviewable before it is
//! connected to the serving path. In particular, an unknown price or quota observation is never
//! converted into a zero-cost or unlimited-capacity score, and a candidate from another Provider
//! can never become an implicit fallback.

use std::{cmp::Ordering, collections::HashSet, error::Error, fmt};

use gateway_core::{EndpointId, ProviderId, RouteCandidateId};

const MAX_CANDIDATES: usize = 4_096;
const MAX_SELECTOR_ID_CHARS: usize = 128;

fn valid_selector_id(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= MAX_SELECTOR_ID_CHARS
}

/// Health state supplied by the existing runtime registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderScopedHealth {
    /// The exact endpoint/candidate is currently available.
    Available,
    /// A bounded endpoint or credential cooldown is active.
    Cooling,
    /// The exact endpoint/candidate circuit is open.
    CircuitOpen,
    /// The exact account is forbidden or unauthorized.
    Unauthorized,
    /// A controlled recovery is already in flight.
    RecoveryInFlight,
}

/// Quota observation supplied by the existing runtime registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderScopedQuota {
    /// The exact target is currently schedulable.
    Available,
    /// No trustworthy quota observation exists yet. This remains eligible, but never scores as
    /// a known zero-cost/unlimited candidate.
    Unknown,
    /// The exact binding or model quota is exhausted.
    Blocked,
    /// A controlled quota recovery probe owns the target.
    RecoveryInFlight,
}

/// A secret-free candidate state assembled by a Provider-specific composition adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderScopedCandidate {
    provider_id: ProviderId,
    channel_id: EndpointId,
    candidate_id: RouteCandidateId,
    priority: i64,
    weight: u32,
    active_leases: u32,
    max_concurrency: u32,
    cost_microunits: Option<u64>,
    health: ProviderScopedHealth,
    quota: ProviderScopedQuota,
    capability_match: bool,
    expired: bool,
}

impl ProviderScopedCandidate {
    /// Creates a bounded candidate state.
    ///
    /// The constructor accepts only already-redacted identifiers and scheduler observations. It
    /// does not parse credentials, contact a Provider, or read a Store.
    /// # Errors
    ///
    /// Returns [`ProviderScopedSelectorError::InvalidCandidate`] when an identifier or scheduling
    /// bound is outside the finite policy domain.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        provider_id: ProviderId,
        channel_id: EndpointId,
        candidate_id: RouteCandidateId,
        priority: i64,
        weight: u32,
        active_leases: u32,
        max_concurrency: u32,
        cost_microunits: Option<u64>,
        health: ProviderScopedHealth,
        quota: ProviderScopedQuota,
        capability_match: bool,
        expired: bool,
    ) -> Result<Self, ProviderScopedSelectorError> {
        if !valid_selector_id(provider_id.as_str())
            || !valid_selector_id(channel_id.as_str())
            || !valid_selector_id(candidate_id.as_str())
            || priority < 0
            || !(1..=10_000).contains(&weight)
            || max_concurrency == 0
        {
            return Err(ProviderScopedSelectorError::InvalidCandidate);
        }
        Ok(Self {
            provider_id,
            channel_id,
            candidate_id,
            priority,
            weight,
            active_leases,
            max_concurrency,
            cost_microunits,
            health,
            quota,
            capability_match,
            expired,
        })
    }

    /// Returns the owning Provider identity.
    #[must_use]
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns the protocol/channel identity.
    #[must_use]
    pub fn channel_id(&self) -> &EndpointId {
        &self.channel_id
    }

    /// Returns the stable route-candidate identity.
    #[must_use]
    pub fn candidate_id(&self) -> &RouteCandidateId {
        &self.candidate_id
    }

    /// Returns the lower-is-better configured priority.
    #[must_use]
    pub const fn priority(&self) -> i64 {
        self.priority
    }

    /// Returns the configured positive scheduling weight.
    #[must_use]
    pub const fn weight(&self) -> u32 {
        self.weight
    }

    /// Returns the current point-in-time active lease count.
    #[must_use]
    pub const fn active_leases(&self) -> u32 {
        self.active_leases
    }

    /// Returns the configured concurrency ceiling.
    #[must_use]
    pub const fn max_concurrency(&self) -> u32 {
        self.max_concurrency
    }

    /// Returns the optional versioned cost supplied by the caller.
    #[must_use]
    pub const fn cost_microunits(&self) -> Option<u64> {
        self.cost_microunits
    }

    /// Returns the observed Health state.
    #[must_use]
    pub const fn health(&self) -> ProviderScopedHealth {
        self.health
    }

    /// Returns the observed Quota state.
    #[must_use]
    pub const fn quota(&self) -> ProviderScopedQuota {
        self.quota
    }

    /// Returns whether the candidate matched the caller's capability requirement.
    #[must_use]
    pub const fn capability_match(&self) -> bool {
        self.capability_match
    }

    /// Returns whether the candidate was already expired at the observation time.
    #[must_use]
    pub const fn expired(&self) -> bool {
        self.expired
    }
}

/// Safe construction/selection failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderScopedSelectorError {
    /// Candidate input or the requested scope exceeded a finite bound.
    InvalidCandidate,
    /// The selection input exceeded the bounded candidate list.
    TooManyCandidates,
    /// More than one observation used the same candidate identity.
    DuplicateCandidate,
}

impl fmt::Display for ProviderScopedSelectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCandidate => "provider-scoped candidate is invalid",
            Self::TooManyCandidates => "provider-scoped candidate list exceeds its finite bound",
            Self::DuplicateCandidate => "provider-scoped candidate identity is duplicated",
        })
    }
}

impl Error for ProviderScopedSelectorError {}

/// Closed reason why one candidate was not eligible for this Provider-scoped selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderScopedRejection {
    /// The candidate belongs to a different Provider and cannot be fallback material.
    ProviderMismatch,
    /// The requested protocol/model capability is not admitted.
    CapabilityMismatch,
    /// The candidate's credential or runtime expiry has passed.
    Expired,
    /// Health is not currently schedulable.
    Health(ProviderScopedHealth),
    /// Quota is exhausted or owned by a recovery probe.
    Quota(ProviderScopedQuota),
    /// No request slot remains at the observed point in time.
    Saturated,
}

/// One deterministic evaluation result. Candidate state remains secret-free and can be handed to
/// a Route Explain projection without exposing transport material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderScopedCandidateDecision {
    candidate: ProviderScopedCandidate,
    rejections: Vec<ProviderScopedRejection>,
}

impl ProviderScopedCandidateDecision {
    /// Returns the evaluated candidate state.
    #[must_use]
    pub fn candidate(&self) -> &ProviderScopedCandidate {
        &self.candidate
    }

    /// Returns all closed exclusion reasons, in stable enum order.
    #[must_use]
    pub fn rejections(&self) -> &[ProviderScopedRejection] {
        &self.rejections
    }

    /// Returns whether the candidate can be handed to the existing scheduler seam.
    #[must_use]
    pub fn is_eligible(&self) -> bool {
        self.rejections.is_empty()
    }
}

/// Stable result for one requested Provider scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderScopedSelection {
    provider_id: ProviderId,
    decisions: Vec<ProviderScopedCandidateDecision>,
    selected_candidate_id: Option<RouteCandidateId>,
}

impl ProviderScopedSelection {
    /// Returns the exact Provider scope used by this result.
    #[must_use]
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns decisions in deterministic ranking order for eligible candidates, followed by
    /// rejected candidates in stable identity order.
    #[must_use]
    pub fn decisions(&self) -> &[ProviderScopedCandidateDecision] {
        &self.decisions
    }

    /// Returns the candidate the caller may pass to the existing lease scheduler.
    #[must_use]
    pub fn selected_candidate_id(&self) -> Option<&RouteCandidateId> {
        self.selected_candidate_id.as_ref()
    }
}

/// Provider-scoped deterministic selector policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderScopedSelector {
    provider_id: ProviderId,
}

impl ProviderScopedSelector {
    /// Creates a selector for one exact Provider identity.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderScopedSelectorError::InvalidCandidate`] when the Provider identity is
    /// empty or exceeds the finite identifier bound.
    pub fn try_new(provider_id: ProviderId) -> Result<Self, ProviderScopedSelectorError> {
        if !valid_selector_id(provider_id.as_str()) {
            return Err(ProviderScopedSelectorError::InvalidCandidate);
        }
        Ok(Self { provider_id })
    }

    /// Evaluates and ranks a bounded set of candidates without mutating scheduler state.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderScopedSelectorError::TooManyCandidates`] when the supplied list exceeds
    /// the finite policy bound, or [`ProviderScopedSelectorError::DuplicateCandidate`] when two
    /// observations refer to the same Provider/channel/candidate identity.
    pub fn select(
        &self,
        candidates: Vec<ProviderScopedCandidate>,
    ) -> Result<ProviderScopedSelection, ProviderScopedSelectorError> {
        if candidates.len() > MAX_CANDIDATES {
            return Err(ProviderScopedSelectorError::TooManyCandidates);
        }

        let mut identities = HashSet::with_capacity(candidates.len());
        for candidate in &candidates {
            if !identities.insert(candidate.candidate_id.clone()) {
                return Err(ProviderScopedSelectorError::DuplicateCandidate);
            }
        }

        let mut decisions = candidates
            .into_iter()
            .map(|candidate| {
                let rejections = self.rejections(&candidate);
                ProviderScopedCandidateDecision {
                    candidate,
                    rejections,
                }
            })
            .collect::<Vec<_>>();

        decisions.sort_by(decision_order);
        let selected_candidate_id = decisions
            .iter()
            .find(|decision| decision.is_eligible())
            .map(|decision| decision.candidate.candidate_id.clone());

        Ok(ProviderScopedSelection {
            provider_id: self.provider_id.clone(),
            decisions,
            selected_candidate_id,
        })
    }

    fn rejections(&self, candidate: &ProviderScopedCandidate) -> Vec<ProviderScopedRejection> {
        let mut rejections = Vec::new();
        if candidate.provider_id != self.provider_id {
            rejections.push(ProviderScopedRejection::ProviderMismatch);
        }
        if !candidate.capability_match {
            rejections.push(ProviderScopedRejection::CapabilityMismatch);
        }
        if candidate.expired {
            rejections.push(ProviderScopedRejection::Expired);
        }
        if candidate.health != ProviderScopedHealth::Available {
            rejections.push(ProviderScopedRejection::Health(candidate.health));
        }
        if matches!(
            candidate.quota,
            ProviderScopedQuota::Blocked | ProviderScopedQuota::RecoveryInFlight
        ) {
            rejections.push(ProviderScopedRejection::Quota(candidate.quota));
        }
        if candidate.active_leases >= candidate.max_concurrency {
            rejections.push(ProviderScopedRejection::Saturated);
        }
        rejections
    }
}

fn decision_order(
    left: &ProviderScopedCandidateDecision,
    right: &ProviderScopedCandidateDecision,
) -> Ordering {
    match (left.is_eligible(), right.is_eligible()) {
        (true, false) => return Ordering::Less,
        (false, true) => return Ordering::Greater,
        _ => {}
    }
    if !left.is_eligible() {
        return left
            .candidate
            .provider_id
            .cmp(&right.candidate.provider_id)
            .then_with(|| left.candidate.channel_id.cmp(&right.candidate.channel_id))
            .then_with(|| {
                left.candidate
                    .candidate_id
                    .cmp(&right.candidate.candidate_id)
            });
    }

    // Known quota evidence and known cost always outrank unknown values. Unknown is never
    // substituted with zero; if every candidate is unknown, the remaining deterministic load and
    // configuration tie-breakers still provide a stable choice.
    quota_rank(left.candidate.quota)
        .cmp(&quota_rank(right.candidate.quota))
        .then_with(|| {
            cost_rank(left.candidate.cost_microunits.as_ref())
                .cmp(&cost_rank(right.candidate.cost_microunits.as_ref()))
        })
        .then_with(|| compare_load(&left.candidate, &right.candidate))
        .then_with(|| left.candidate.priority.cmp(&right.candidate.priority))
        .then_with(|| right.candidate.weight.cmp(&left.candidate.weight))
        .then_with(|| left.candidate.channel_id.cmp(&right.candidate.channel_id))
        .then_with(|| {
            left.candidate
                .candidate_id
                .cmp(&right.candidate.candidate_id)
        })
}

fn quota_rank(quota: ProviderScopedQuota) -> u8 {
    match quota {
        ProviderScopedQuota::Available => 0,
        ProviderScopedQuota::Unknown => 1,
        ProviderScopedQuota::Blocked | ProviderScopedQuota::RecoveryInFlight => 2,
    }
}

fn cost_rank(cost: Option<&u64>) -> (u8, u64) {
    match cost {
        Some(value) => (0, *value),
        None => (1, 0),
    }
}

fn compare_load(left: &ProviderScopedCandidate, right: &ProviderScopedCandidate) -> Ordering {
    // Compare active/max ratios without floating point or overflow in u32 multiplication.
    let left_product = u128::from(left.active_leases) * u128::from(right.max_concurrency);
    let right_product = u128::from(right.active_leases) * u128::from(left.max_concurrency);
    left_product
        .cmp(&right_product)
        .then_with(|| left.active_leases.cmp(&right.active_leases))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{EndpointId, ProviderId, RouteCandidateId};

    use super::{
        ProviderScopedCandidate, ProviderScopedHealth, ProviderScopedQuota,
        ProviderScopedRejection, ProviderScopedSelector, ProviderScopedSelectorError,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    fn candidate(
        provider: &str,
        id: &str,
        priority: i64,
        active: u32,
        maximum: u32,
        cost: Option<u64>,
    ) -> Result<ProviderScopedCandidate, Box<dyn Error>> {
        Ok(ProviderScopedCandidate::try_new(
            ProviderId::try_new(provider)?,
            EndpointId::try_new("channel-a")?,
            RouteCandidateId::try_new(id)?,
            priority,
            1,
            active,
            maximum,
            cost,
            ProviderScopedHealth::Available,
            ProviderScopedQuota::Available,
            true,
            false,
        )?)
    }

    #[test]
    fn provider_scope_never_falls_back_to_another_provider() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let selected = selector.select(vec![
            ProviderScopedCandidate::try_new(
                ProviderId::try_new("provider-a")?,
                EndpointId::try_new("channel-a")?,
                RouteCandidateId::try_new("blocked")?,
                0,
                1,
                0,
                1,
                Some(1),
                ProviderScopedHealth::Cooling,
                ProviderScopedQuota::Available,
                true,
                false,
            )?,
            candidate("provider-b", "foreign-healthy", 0, 0, 1, Some(0))?,
        ])?;
        assert_eq!(selected.selected_candidate_id(), None);
        assert!(selected.decisions().iter().any(|decision| {
            decision.candidate().candidate_id().as_str() == "foreign-healthy"
                && decision.rejections() == [ProviderScopedRejection::ProviderMismatch]
        }));
        Ok(())
    }

    #[test]
    fn known_cost_and_quota_beat_unknown_without_becoming_zero() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let mut unknown_cost = candidate("provider-a", "unknown-cost", 0, 0, 10, None)?;
        unknown_cost.quota = ProviderScopedQuota::Unknown;
        let selected = selector.select(vec![
            unknown_cost,
            candidate("provider-a", "known-cost", 0, 5, 10, Some(100))?,
        ])?;
        assert_eq!(
            selected
                .selected_candidate_id()
                .map(gateway_core::RouteCandidateId::as_str),
            Some("known-cost")
        );
        Ok(())
    }

    #[test]
    fn unknown_cost_uses_least_loaded_ratio_then_stable_tiebreakers() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let selected = selector.select(vec![
            candidate("provider-a", "more-loaded", 0, 3, 4, None)?,
            candidate("provider-a", "less-loaded", 0, 1, 4, None)?,
        ])?;
        assert_eq!(
            selected
                .selected_candidate_id()
                .map(gateway_core::RouteCandidateId::as_str),
            Some("less-loaded")
        );
        Ok(())
    }

    #[test]
    fn ranking_is_independent_of_input_order_and_uses_channel_tiebreaker() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let first = candidate("provider-a", "candidate-a", 1, 1, 4, Some(10))?;
        let mut second = candidate("provider-a", "candidate-b", 1, 1, 4, Some(10))?;
        second.channel_id = EndpointId::try_new("channel-b")?;
        let forward = selector.select(vec![first.clone(), second.clone()])?;
        let reverse = selector.select(vec![second, first])?;
        assert_eq!(forward, reverse);
        assert_eq!(
            forward
                .selected_candidate_id()
                .map(gateway_core::RouteCandidateId::as_str),
            Some("candidate-a")
        );
        Ok(())
    }

    #[test]
    fn load_ratio_uses_wide_arithmetic_without_overflow() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let selected = selector.select(vec![
            candidate("provider-a", "almost-full", 0, u32::MAX - 1, u32::MAX, None)?,
            candidate("provider-a", "half-full", 0, u32::MAX / 2, u32::MAX, None)?,
        ])?;
        assert_eq!(
            selected
                .selected_candidate_id()
                .map(gateway_core::RouteCandidateId::as_str),
            Some("half-full")
        );
        Ok(())
    }

    #[test]
    fn rejection_matrix_is_closed_and_deterministic() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let mut capability = candidate("provider-a", "capability", 0, 0, 1, Some(1))?;
        capability.capability_match = false;
        let mut expired = candidate("provider-a", "expired", 0, 0, 1, Some(1))?;
        expired.expired = true;
        let mut quota = candidate("provider-a", "quota", 0, 0, 1, Some(1))?;
        quota.quota = ProviderScopedQuota::Blocked;
        let saturated = candidate("provider-a", "saturated", 0, 1, 1, Some(1))?;
        let foreign = candidate("provider-b", "foreign", 0, 0, 1, Some(1))?;
        let mut cooling = candidate("provider-a", "cooling", 0, 0, 1, Some(1))?;
        cooling.health = ProviderScopedHealth::Cooling;
        let mut circuit = candidate("provider-a", "circuit", 0, 0, 1, Some(1))?;
        circuit.health = ProviderScopedHealth::CircuitOpen;
        let mut unauthorized = candidate("provider-a", "unauthorized", 0, 0, 1, Some(1))?;
        unauthorized.health = ProviderScopedHealth::Unauthorized;
        let mut recovery = candidate("provider-a", "recovery", 0, 0, 1, Some(1))?;
        recovery.health = ProviderScopedHealth::RecoveryInFlight;
        let mut quota_recovery = candidate("provider-a", "quota-recovery", 0, 0, 1, Some(1))?;
        quota_recovery.quota = ProviderScopedQuota::RecoveryInFlight;
        let selected = selector.select(vec![capability, expired, quota, saturated])?;
        assert_eq!(selected.selected_candidate_id(), None);
        assert_eq!(selected.decisions().len(), 4);
        for decision in selected.decisions() {
            assert!(!decision.is_eligible());
        }
        let selected = selector.select(vec![
            foreign,
            cooling,
            circuit,
            unauthorized,
            recovery,
            quota_recovery,
        ])?;
        assert_eq!(selected.selected_candidate_id(), None);
        let expected = [
            (
                "circuit",
                vec![ProviderScopedRejection::Health(
                    ProviderScopedHealth::CircuitOpen,
                )],
            ),
            (
                "cooling",
                vec![ProviderScopedRejection::Health(
                    ProviderScopedHealth::Cooling,
                )],
            ),
            ("foreign", vec![ProviderScopedRejection::ProviderMismatch]),
            (
                "quota-recovery",
                vec![ProviderScopedRejection::Quota(
                    ProviderScopedQuota::RecoveryInFlight,
                )],
            ),
            (
                "recovery",
                vec![ProviderScopedRejection::Health(
                    ProviderScopedHealth::RecoveryInFlight,
                )],
            ),
            (
                "unauthorized",
                vec![ProviderScopedRejection::Health(
                    ProviderScopedHealth::Unauthorized,
                )],
            ),
        ];
        assert_eq!(selected.decisions().len(), expected.len());
        for (id, rejections) in expected {
            let decision = selected
                .decisions()
                .iter()
                .find(|decision| decision.candidate().candidate_id().as_str() == id)
                .ok_or("rejection-matrix candidate was not returned")?;
            assert_eq!(decision.rejections(), rejections.as_slice(), "{id}");
        }

        let selected = selector.select(vec![{
            let mut value = candidate("provider-a", "all-reasons", 0, 1, 1, Some(1))?;
            value.capability_match = false;
            value.expired = true;
            value.health = ProviderScopedHealth::Cooling;
            value.quota = ProviderScopedQuota::Blocked;
            value
        }])?;
        assert_eq!(
            selected.decisions()[0].rejections(),
            &[
                ProviderScopedRejection::CapabilityMismatch,
                ProviderScopedRejection::Expired,
                ProviderScopedRejection::Health(ProviderScopedHealth::Cooling),
                ProviderScopedRejection::Quota(ProviderScopedQuota::Blocked),
                ProviderScopedRejection::Saturated,
            ]
        );
        Ok(())
    }

    #[test]
    fn candidate_bounds_fail_closed() -> TestResult {
        let error = ProviderScopedCandidate::try_new(
            ProviderId::try_new("provider-a")?,
            EndpointId::try_new("channel-a")?,
            RouteCandidateId::try_new("candidate-a")?,
            0,
            1,
            0,
            0,
            None,
            ProviderScopedHealth::Available,
            ProviderScopedQuota::Available,
            true,
            false,
        )
        .err()
        .ok_or("invalid zero concurrency was accepted")?;
        assert_eq!(error, ProviderScopedSelectorError::InvalidCandidate);
        Ok(())
    }

    #[test]
    fn whitespace_only_and_overlong_identities_fail_closed() -> TestResult {
        let whitespace_error = ProviderScopedCandidate::try_new(
            ProviderId::try_new("provider-a")?,
            EndpointId::try_new("   ")?,
            RouteCandidateId::try_new("candidate-a")?,
            0,
            1,
            0,
            1,
            None,
            ProviderScopedHealth::Available,
            ProviderScopedQuota::Available,
            true,
            false,
        )
        .err()
        .ok_or("whitespace-only channel identity was accepted")?;
        assert_eq!(
            whitespace_error,
            ProviderScopedSelectorError::InvalidCandidate
        );

        let overlong_error = ProviderScopedSelector::try_new(ProviderId::try_new("p".repeat(129))?)
            .err()
            .ok_or("overlong provider identity was accepted")?;
        assert_eq!(
            overlong_error,
            ProviderScopedSelectorError::InvalidCandidate
        );
        Ok(())
    }

    #[test]
    fn duplicate_candidate_identity_fails_before_ranking() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let first = candidate("provider-a", "candidate-a", 0, 0, 1, Some(1))?;
        let mut duplicate = first.clone();
        duplicate.cost_microunits = Some(2);
        let error = selector
            .select(vec![first, duplicate])
            .err()
            .ok_or("duplicate candidate identity was accepted")?;
        assert_eq!(error, ProviderScopedSelectorError::DuplicateCandidate);
        Ok(())
    }
}
