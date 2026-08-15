//! Provider-scoped deterministic candidate ranking for P13-07A.
//!
//! This module is deliberately a policy seam, not a second request scheduler. It consumes a
//! caller-supplied, secret-free view of candidate state and returns a stable ranking plus closed
//! exclusion reasons. The existing [`crate::RouteCredentialScheduler`] remains the owner of
//! request-time cursor advancement, Health/Quota reads, and Credential lease acquisition.
//!
//! Price evidence is intentionally a six-dimensional rate vector rather than a guessed
//! per-request cost. The selector uses one bounded, globally classified dominance pass; it never
//! feeds pairwise partial ordering into `sort_by`. In particular, an unknown price or quota
//! observation is never converted into a zero-cost or unlimited-capacity score, and a candidate
//! from another Provider can never become an implicit fallback.

use std::{cmp::Ordering, collections::HashSet, error::Error, fmt};

use gateway_core::{EndpointId, ProviderId, RouteCandidateId};

const MAX_CANDIDATES: usize = 4_096;
const MAX_SELECTOR_ID_CHARS: usize = 128;

fn valid_selector_id(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= MAX_SELECTOR_ID_CHARS
}

/// Immutable Provider/channel/model rates in integer microunits per million tokens.
///
/// These six dimensions deliberately preserve the P13-05 billing catalog shape. They are price
/// evidence, not a predicted request charge: the Router has no trustworthy request-time token
/// vector from which to compute such a charge before Provider selection.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderScopedPriceRates {
    input_microunits_per_million: u64,
    output_microunits_per_million: u64,
    reasoning_microunits_per_million: u64,
    cache_read_microunits_per_million: u64,
    cache_creation_microunits_per_million: u64,
    cached_microunits_per_million: u64,
}

impl ProviderScopedPriceRates {
    /// Creates one exact six-dimensional catalog-rate vector.
    #[must_use]
    pub const fn new(
        input_microunits_per_million: u64,
        output_microunits_per_million: u64,
        reasoning_microunits_per_million: u64,
        cache_read_microunits_per_million: u64,
        cache_creation_microunits_per_million: u64,
        cached_microunits_per_million: u64,
    ) -> Self {
        Self {
            input_microunits_per_million,
            output_microunits_per_million,
            reasoning_microunits_per_million,
            cache_read_microunits_per_million,
            cache_creation_microunits_per_million,
            cached_microunits_per_million,
        }
    }

    /// Returns the input-token rate.
    #[must_use]
    pub const fn input_microunits_per_million(&self) -> u64 {
        self.input_microunits_per_million
    }

    /// Returns the output-token rate.
    #[must_use]
    pub const fn output_microunits_per_million(&self) -> u64 {
        self.output_microunits_per_million
    }

    /// Returns the reasoning-token rate.
    #[must_use]
    pub const fn reasoning_microunits_per_million(&self) -> u64 {
        self.reasoning_microunits_per_million
    }

    /// Returns the cache-read-token rate.
    #[must_use]
    pub const fn cache_read_microunits_per_million(&self) -> u64 {
        self.cache_read_microunits_per_million
    }

    /// Returns the cache-creation-token rate.
    #[must_use]
    pub const fn cache_creation_microunits_per_million(&self) -> u64 {
        self.cache_creation_microunits_per_million
    }

    /// Returns the generic cached-token rate.
    #[must_use]
    pub const fn cached_microunits_per_million(&self) -> u64 {
        self.cached_microunits_per_million
    }

    /// Returns whether every explicitly cataloged dimension is zero.
    ///
    /// A missing rate vector is never treated as this value.
    #[must_use]
    pub const fn is_all_zero(&self) -> bool {
        self.input_microunits_per_million == 0
            && self.output_microunits_per_million == 0
            && self.reasoning_microunits_per_million == 0
            && self.cache_read_microunits_per_million == 0
            && self.cache_creation_microunits_per_million == 0
            && self.cached_microunits_per_million == 0
    }

    const fn dimensions(&self) -> [u64; 6] {
        [
            self.input_microunits_per_million,
            self.output_microunits_per_million,
            self.reasoning_microunits_per_million,
            self.cache_read_microunits_per_million,
            self.cache_creation_microunits_per_million,
            self.cached_microunits_per_million,
        ]
    }
}

/// Closed evidence emitted by the bounded `rate_dominance_v1` classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderScopedPriceEvidence {
    /// This known vector simultaneously reaches the minimum in all six dimensions.
    Dominant,
    /// Every eligible known vector is exactly equal in all six dimensions.
    Equal,
    /// A dominant vector exists and this known vector is not one of the minima.
    Dominated,
    /// Known vectors cross dimensions and no vector reaches every dimension minimum.
    Incomparable,
    /// The eligible Candidate has no exact catalog-rate vector.
    Unpriced,
    /// The Candidate was rejected before price comparison and therefore was not evaluated.
    NotEvaluated,
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
    price_rates: Option<ProviderScopedPriceRates>,
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
        price_rates: Option<ProviderScopedPriceRates>,
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
            price_rates,
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

    /// Returns the optional exact catalog-rate vector supplied by the caller.
    #[must_use]
    pub const fn price_rates(&self) -> Option<ProviderScopedPriceRates> {
        self.price_rates
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
    price_evidence: ProviderScopedPriceEvidence,
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

    /// Returns the closed price-rate classification used by deterministic ranking.
    #[must_use]
    pub const fn price_evidence(&self) -> ProviderScopedPriceEvidence {
        self.price_evidence
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
                    price_evidence: ProviderScopedPriceEvidence::NotEvaluated,
                }
            })
            .collect::<Vec<_>>();

        classify_price_evidence(&mut decisions);
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

    // Known quota evidence and classified price evidence outrank absent price evidence. Price
    // rates are not a guessed per-request cost. If vectors cross dimensions, all known vectors in
    // that comparison become Incomparable and fall through to load/configuration tie-breakers.
    quota_rank(left.candidate.quota)
        .cmp(&quota_rank(right.candidate.quota))
        .then_with(|| {
            price_evidence_rank(left.price_evidence).cmp(&price_evidence_rank(right.price_evidence))
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

fn price_evidence_rank(evidence: ProviderScopedPriceEvidence) -> u8 {
    match evidence {
        ProviderScopedPriceEvidence::Dominant | ProviderScopedPriceEvidence::Equal => 0,
        ProviderScopedPriceEvidence::Dominated | ProviderScopedPriceEvidence::Incomparable => 1,
        ProviderScopedPriceEvidence::Unpriced => 2,
        ProviderScopedPriceEvidence::NotEvaluated => 3,
    }
}

fn classify_price_evidence(decisions: &mut [ProviderScopedCandidateDecision]) {
    let known = decisions
        .iter()
        .enumerate()
        .filter(|(_, decision)| decision.is_eligible())
        .filter_map(|(index, decision)| decision.candidate.price_rates.map(|rates| (index, rates)))
        .collect::<Vec<_>>();

    for decision in decisions
        .iter_mut()
        .filter(|decision| decision.is_eligible())
    {
        decision.price_evidence = if decision.candidate.price_rates.is_some() {
            // Known vectors are classified together below.
            ProviderScopedPriceEvidence::Incomparable
        } else {
            ProviderScopedPriceEvidence::Unpriced
        };
    }
    if known.is_empty() {
        return;
    }
    if known.iter().all(|(_, rates)| *rates == known[0].1) {
        for (index, _) in known {
            decisions[index].price_evidence = ProviderScopedPriceEvidence::Equal;
        }
        return;
    }

    let mut minima = [u64::MAX; 6];
    for (_, rates) in &known {
        for (index, value) in rates.dimensions().into_iter().enumerate() {
            minima[index] = minima[index].min(value);
        }
    }
    let dominant = known
        .iter()
        .filter(|(_, rates)| rates.dimensions() == minima)
        .map(|(index, _)| *index)
        .collect::<Vec<_>>();
    if dominant.is_empty() {
        return;
    }
    for (index, _) in known {
        decisions[index].price_evidence = if dominant.contains(&index) {
            ProviderScopedPriceEvidence::Dominant
        } else {
            ProviderScopedPriceEvidence::Dominated
        };
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
        ProviderScopedCandidate, ProviderScopedHealth, ProviderScopedPriceEvidence,
        ProviderScopedPriceRates, ProviderScopedQuota, ProviderScopedRejection,
        ProviderScopedSelector, ProviderScopedSelectorError,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    const fn rates(value: u64) -> ProviderScopedPriceRates {
        ProviderScopedPriceRates::new(value, value, value, value, value, value)
    }

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
            cost.map(rates),
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
                Some(rates(1)),
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
    fn known_price_and_quota_beat_unknown_without_becoming_zero() -> TestResult {
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
    fn all_dimension_minimum_is_dominant_without_predicting_request_cost() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let mut lower = candidate("provider-a", "lower", 0, 5, 10, None)?;
        lower.price_rates = Some(ProviderScopedPriceRates::new(1, 2, 3, 4, 5, 6));
        let mut higher = candidate("provider-a", "higher", 0, 0, 10, None)?;
        higher.price_rates = Some(ProviderScopedPriceRates::new(2, 3, 4, 5, 6, 7));
        let selected = selector.select(vec![higher, lower])?;

        assert_eq!(
            selected
                .selected_candidate_id()
                .map(RouteCandidateId::as_str),
            Some("lower")
        );
        assert_eq!(
            selected.decisions()[0].price_evidence(),
            ProviderScopedPriceEvidence::Dominant
        );
        assert_eq!(
            selected.decisions()[1].price_evidence(),
            ProviderScopedPriceEvidence::Dominated
        );
        Ok(())
    }

    #[test]
    fn identical_known_vectors_are_equal_evidence() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let selected = selector.select(vec![
            candidate("provider-a", "candidate-b", 0, 0, 1, Some(9))?,
            candidate("provider-a", "candidate-a", 0, 0, 1, Some(9))?,
        ])?;

        assert!(
            selected.decisions().iter().all(|decision| {
                decision.price_evidence() == ProviderScopedPriceEvidence::Equal
            })
        );
        assert_eq!(
            selected
                .selected_candidate_id()
                .map(RouteCandidateId::as_str),
            Some("candidate-a")
        );
        Ok(())
    }

    #[test]
    fn one_known_vector_is_equal_not_unproven_dominant() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let selected = selector.select(vec![candidate(
            "provider-a",
            "only-candidate",
            0,
            0,
            1,
            Some(9),
        )?])?;

        assert_eq!(
            selected.decisions()[0].price_evidence(),
            ProviderScopedPriceEvidence::Equal
        );
        Ok(())
    }

    #[test]
    fn crossing_vectors_are_incomparable_and_input_order_independent() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let mut input_cheap = candidate("provider-a", "candidate-a", 0, 0, 1, None)?;
        input_cheap.price_rates = Some(ProviderScopedPriceRates::new(1, 9, 1, 9, 1, 9));
        let mut output_cheap = candidate("provider-a", "candidate-b", 0, 0, 1, None)?;
        output_cheap.price_rates = Some(ProviderScopedPriceRates::new(9, 1, 9, 1, 9, 1));

        let forward = selector.select(vec![input_cheap.clone(), output_cheap.clone()])?;
        let reverse = selector.select(vec![output_cheap, input_cheap])?;
        assert_eq!(forward, reverse);
        assert!(forward.decisions().iter().all(|decision| {
            decision.price_evidence() == ProviderScopedPriceEvidence::Incomparable
        }));
        assert_eq!(
            forward
                .selected_candidate_id()
                .map(RouteCandidateId::as_str),
            Some("candidate-a")
        );
        Ok(())
    }

    #[test]
    fn known_vector_ranks_before_unpriced_but_unknown_is_not_zero() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let selected = selector.select(vec![
            candidate("provider-a", "unpriced", 0, 0, 10, None)?,
            candidate("provider-a", "known", 0, 9, 10, Some(100))?,
        ])?;

        assert_eq!(
            selected
                .selected_candidate_id()
                .map(RouteCandidateId::as_str),
            Some("known")
        );
        assert_eq!(
            selected.decisions()[0].price_evidence(),
            ProviderScopedPriceEvidence::Equal
        );
        assert_eq!(
            selected.decisions()[1].price_evidence(),
            ProviderScopedPriceEvidence::Unpriced
        );
        Ok(())
    }

    #[test]
    fn explicit_all_zero_vector_is_known_zero() -> TestResult {
        let selector = ProviderScopedSelector::try_new(ProviderId::try_new("provider-a")?)?;
        let zero = ProviderScopedPriceRates::new(0, 0, 0, 0, 0, 0);
        assert!(zero.is_all_zero());
        let mut free = candidate("provider-a", "free", 0, 5, 10, None)?;
        free.price_rates = Some(zero);
        let selected = selector.select(vec![
            candidate("provider-a", "unpriced", 0, 0, 10, None)?,
            free,
        ])?;

        assert_eq!(
            selected
                .selected_candidate_id()
                .map(RouteCandidateId::as_str),
            Some("free")
        );
        assert_eq!(
            selected.decisions()[0].candidate().price_rates(),
            Some(zero)
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
        duplicate.price_rates = Some(rates(2));
        let error = selector
            .select(vec![first, duplicate])
            .err()
            .ok_or("duplicate candidate identity was accepted")?;
        assert_eq!(error, ProviderScopedSelectorError::DuplicateCandidate);
        Ok(())
    }
}
