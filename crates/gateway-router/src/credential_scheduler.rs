//! Two-stage Candidate then Credential lease selection.
//!
//! P3-03 owns immutable Candidate schedule construction and candidate cursors. P3-04 composes
//! that selector with independently scheduled Endpoint Credential pools, without allowing an
//! Endpoint's number of keys to alter Route-level weights. P3-05 may consult a separate sharded
//! runtime-health registry; attempt, retry, transport, and Provider behavior remain outside this
//! module.

use std::{collections::BTreeMap, sync::Arc};

use gateway_catalog::SemanticCapability;
use gateway_core::{
    CredentialId, EndpointId, ErrorScope, GatewayError, GatewayErrorCode, ProviderId,
    RouteCandidateId, RouteId,
};
use gateway_upstream::{CredentialLease, EndpointCredentialPools};

use crate::{
    AttemptExclusionSet, ProviderScopedPriceRates, ProviderScopedRouteExplainError,
    ProviderScopedRouteExplainInput, ProviderScopedRouteExplainSnapshot, ProviderScopedSelection,
    RouteCandidateScheduler, RouteExplainError, RouteExplainInput, RouteExplainSnapshot,
    RouteSnapshot, RuntimeHealthRegistry, RuntimeQuotaAvailability, RuntimeQuotaRegistry,
    RuntimeQuotaTarget, SnapshotRouteCandidate,
};

/// A process-local two-stage scheduler for one immutable Route Snapshot and matching pools.
///
/// It owns no `SQLite` handle, mutable health state, or global lock. Candidate and Credential
/// cursor sets are independent: a Route Candidate is selected first, then its Endpoint pool uses
/// its own atomic cursor to acquire one bounded concurrent lease. P3-05 methods only consult an
/// externally owned runtime-health registry; they never classify or mutate runtime state.
#[derive(Debug)]
pub struct RouteCredentialScheduler {
    candidates: RouteCandidateScheduler,
    credential_pools: Arc<EndpointCredentialPools>,
    provider_price_rates: Arc<BTreeMap<RouteCandidateId, ProviderScopedPriceRates>>,
}

// The pre-expiry API variants intentionally preserve their historical behavior: expiry-aware
// filtering is opt-in through the `_at` methods used by request orchestration.
const LEGACY_NO_EXPIRY_OBSERVATION_MS: i64 = i64::MIN;

impl RouteCredentialScheduler {
    /// Creates a two-stage selector from one immutable Snapshot and Endpoint-local pools.
    ///
    /// The caller builds both runtime artifacts from the same validated Config Version on the
    /// control path. A missing Endpoint pool is treated as unavailable at selection time rather
    /// than causing a Store lookup or an implicit credential fallback.
    #[must_use]
    pub fn new(
        snapshot: Arc<RouteSnapshot>,
        credential_pools: Arc<EndpointCredentialPools>,
    ) -> Self {
        Self::new_with_provider_price_rates(snapshot, credential_pools, Arc::new(BTreeMap::new()))
    }

    /// Creates a two-stage selector with one immutable, composition-time Provider price map.
    ///
    /// The map is keyed only by stable Candidate identity and contains no usage estimates or
    /// credentials. An absent entry remains unpriced. Callers must rebuild the scheduler to
    /// publish a different catalog snapshot; request-time paths never read persistence.
    #[must_use]
    pub fn new_with_provider_price_rates(
        snapshot: Arc<RouteSnapshot>,
        credential_pools: Arc<EndpointCredentialPools>,
        provider_price_rates: Arc<BTreeMap<RouteCandidateId, ProviderScopedPriceRates>>,
    ) -> Self {
        Self {
            candidates: RouteCandidateScheduler::new(snapshot),
            credential_pools,
            provider_price_rates,
        }
    }

    /// Returns the immutable Candidate-price map owned by this scheduler composition.
    #[must_use]
    pub fn provider_price_rates(&self) -> &BTreeMap<RouteCandidateId, ProviderScopedPriceRates> {
        &self.provider_price_rates
    }

    /// Returns one exact Candidate's immutable price vector, when cataloged.
    #[must_use]
    pub fn provider_price_rates_for_candidate(
        &self,
        candidate_id: &RouteCandidateId,
    ) -> Option<ProviderScopedPriceRates> {
        self.provider_price_rates.get(candidate_id).copied()
    }

    /// Returns the exact immutable Config Snapshot version owned by this scheduler.
    ///
    /// Management diagnostics use this value to reject a newly published registry Snapshot until
    /// the process has rebuilt its credential pools, price map, and scheduler together.
    #[must_use]
    pub fn snapshot_version(&self) -> &crate::SnapshotVersion {
        self.candidates.snapshot().version()
    }

    /// Selects a Candidate and immediately acquires one lease from its Endpoint Credential pool.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` when the Route is unknown, every Candidate is
    /// rejected, an Endpoint has no matching pool, or all matching pools are saturated. No
    /// Candidate, Endpoint, Credential, or Secret is exposed in the error.
    pub fn select_and_lease(
        &self,
        route_id: &RouteId,
    ) -> Result<SelectedRouteCredential, GatewayError> {
        self.select_eligible_and_lease(route_id, |_| true)
    }

    /// Selects an eligible Candidate and immediately acquires one Endpoint Credential lease.
    ///
    /// The supplied predicate remains a narrow composition point for later P3-06 attempt
    /// exclusions. This P3-04-compatible method does not consult or mutate health, circuit, quota,
    /// retry, or attempt state.
    ///
    /// # Errors
    ///
    /// Returns the same safe `CredentialUnavailable/Credential` result as
    /// [`Self::select_and_lease`] when no candidate can produce a live lease.
    pub fn select_eligible_and_lease<F>(
        &self,
        route_id: &RouteId,
        mut is_candidate_eligible: F,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        F: FnMut(&SnapshotRouteCandidate) -> bool,
    {
        let mut lease = None;
        let candidate = self.candidates.select_eligible(route_id, |candidate| {
            if !is_candidate_eligible(candidate) {
                return false;
            }
            let Some(acquired) = self.credential_pools.try_lease(candidate.endpoint_id()) else {
                return false;
            };
            lease = Some(acquired);
            true
        });

        match (candidate, lease) {
            (Some(candidate), Some(lease)) => Ok(SelectedRouteCredential { candidate, lease }),
            _ => Err(credential_unavailable_error()),
        }
    }

    /// Selects a Candidate and Credential while applying P3-05 runtime availability state.
    ///
    /// Endpoint-level Cooldown/Circuit state rejects a Candidate before its pool is read.
    /// Endpoint/Credential state is applied inside that pool's bounded weighted scan, so a cooled
    /// Credential does not incorrectly make its healthy sibling Credentials unavailable.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` without runtime identifiers when no Candidate
    /// can pass runtime availability and acquire a live Credential lease.
    pub fn select_runtime_eligible_and_lease(
        &self,
        route_id: &RouteId,
        runtime_health: &RuntimeHealthRegistry,
    ) -> Result<SelectedRouteCredential, GatewayError> {
        self.select_eligible_and_lease_with_runtime_health(route_id, runtime_health, |_| true)
    }

    /// Selects an externally eligible Candidate and Credential while applying runtime health.
    ///
    /// This retains the P3-04 caller predicate for later P3-06 attempt exclusions. A clock or
    /// shard failure in `runtime_health` fails closed by rejecting the affected Candidate or
    /// Credential, yielding the existing safe `CredentialUnavailable/Credential` result if none
    /// can acquire a live lease.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` without runtime identifiers when no Candidate
    /// can pass the caller predicate, runtime availability, and lease acquisition.
    pub fn select_eligible_and_lease_with_runtime_health<F>(
        &self,
        route_id: &RouteId,
        runtime_health: &RuntimeHealthRegistry,
        is_candidate_eligible: F,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        F: FnMut(&SnapshotRouteCandidate) -> bool,
    {
        self.select_eligible_and_lease_with_runtime_health_and_binding(
            route_id,
            runtime_health,
            is_candidate_eligible,
            |_, _| true,
        )
    }

    /// Selects an externally eligible Candidate/Credential binding while applying runtime health.
    ///
    /// The two predicates execute before a Credential capacity reservation: the Candidate
    /// predicate runs before an Endpoint pool is read, and the binding predicate runs on only the
    /// stable Candidate and Credential identities inside that pool. P3-06 uses this boundary to
    /// exclude an already attempted binding without globally excluding a healthy sibling
    /// Credential or another Endpoint.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` without runtime identities when no Candidate
    /// can pass both predicates, runtime availability, and lease acquisition.
    pub fn select_eligible_and_lease_with_runtime_health_and_binding<FCandidate, FBinding>(
        &self,
        route_id: &RouteId,
        runtime_health: &RuntimeHealthRegistry,
        is_candidate_eligible: FCandidate,
        is_binding_eligible: FBinding,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        FCandidate: FnMut(&SnapshotRouteCandidate) -> bool,
        FBinding: FnMut(&SnapshotRouteCandidate, &CredentialId) -> bool,
    {
        self.select_eligible_and_lease_with_runtime_health_and_binding_at(
            route_id,
            runtime_health,
            LEGACY_NO_EXPIRY_OBSERVATION_MS,
            is_candidate_eligible,
            is_binding_eligible,
        )
    }

    /// Expiry-aware variant of [`Self::select_eligible_and_lease_with_runtime_health_and_binding`].
    ///
    /// `observed_at_ms` is evaluated immediately before the Endpoint pool attempts its lease, so
    /// an expired slot cannot win a higher-priority tier over a current sibling.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` when no Candidate and current Credential can
    /// satisfy the supplied predicates and runtime-health state.
    pub fn select_eligible_and_lease_with_runtime_health_and_binding_at<FCandidate, FBinding>(
        &self,
        route_id: &RouteId,
        runtime_health: &RuntimeHealthRegistry,
        observed_at_ms: i64,
        mut is_candidate_eligible: FCandidate,
        mut is_binding_eligible: FBinding,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        FCandidate: FnMut(&SnapshotRouteCandidate) -> bool,
        FBinding: FnMut(&SnapshotRouteCandidate, &CredentialId) -> bool,
    {
        let mut lease = None;
        let candidate = self.candidates.select_eligible(route_id, |candidate| {
            if !is_candidate_eligible(candidate)
                || !runtime_health.endpoint_is_available(candidate.endpoint_id())
            {
                return false;
            }
            let Some(acquired) = self.credential_pools.try_lease_eligible_at(
                candidate.endpoint_id(),
                observed_at_ms,
                |credential_id| {
                    is_binding_eligible(candidate, credential_id)
                        && runtime_health.endpoint_credential_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                        )
                        && runtime_health.endpoint_credential_model_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                            candidate.upstream_model(),
                        )
                },
            ) else {
                return false;
            };
            lease = Some(acquired);
            true
        });

        match (candidate, lease) {
            (Some(candidate), Some(lease)) => Ok(SelectedRouteCredential { candidate, lease }),
            _ => Err(credential_unavailable_error()),
        }
    }

    /// Selects an externally eligible Candidate/Credential binding while applying Health and Quota.
    ///
    /// Quota checks remain target-local and execute before the pool reserves a Credential lease.
    /// A binding-wide quota blocks every model on that binding; a model-scoped quota blocks only
    /// the Candidate's exact upstream-model label. Reset expiry alone remains unavailable until
    /// `RuntimeQuotaRegistry` receives a controlled recovery result.
    ///
    /// # Errors
    ///
    /// Returns the same secret-free `CredentialUnavailable/Credential` result when no Candidate
    /// can pass caller eligibility, Health, Quota, and bounded lease acquisition.
    pub fn select_eligible_and_lease_with_runtime_health_quota_and_binding<FCandidate, FBinding>(
        &self,
        route_id: &RouteId,
        runtime_health: &RuntimeHealthRegistry,
        runtime_quota: &RuntimeQuotaRegistry,
        is_candidate_eligible: FCandidate,
        is_binding_eligible: FBinding,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        FCandidate: FnMut(&SnapshotRouteCandidate) -> bool,
        FBinding: FnMut(&SnapshotRouteCandidate, &CredentialId) -> bool,
    {
        self.select_eligible_and_lease_with_runtime_health_quota_and_binding_at(
            route_id,
            runtime_health,
            runtime_quota,
            LEGACY_NO_EXPIRY_OBSERVATION_MS,
            is_candidate_eligible,
            is_binding_eligible,
        )
    }

    /// Expiry-aware variant of
    /// [`Self::select_eligible_and_lease_with_runtime_health_quota_and_binding`].
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` when no current Credential passes the supplied
    /// predicates, runtime-health state, and quota state.
    pub fn select_eligible_and_lease_with_runtime_health_quota_and_binding_at<
        FCandidate,
        FBinding,
    >(
        &self,
        route_id: &RouteId,
        runtime_health: &RuntimeHealthRegistry,
        runtime_quota: &RuntimeQuotaRegistry,
        observed_at_ms: i64,
        mut is_candidate_eligible: FCandidate,
        mut is_binding_eligible: FBinding,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        FCandidate: FnMut(&SnapshotRouteCandidate) -> bool,
        FBinding: FnMut(&SnapshotRouteCandidate, &CredentialId) -> bool,
    {
        let mut lease = None;
        let candidate = self.candidates.select_eligible(route_id, |candidate| {
            if !is_candidate_eligible(candidate)
                || !runtime_health.endpoint_is_available(candidate.endpoint_id())
            {
                return false;
            }
            let Some(acquired) = self.credential_pools.try_lease_eligible_at(
                candidate.endpoint_id(),
                observed_at_ms,
                |credential_id| {
                    is_binding_eligible(candidate, credential_id)
                        && runtime_health.endpoint_credential_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                        )
                        && runtime_health.endpoint_credential_model_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                            candidate.upstream_model(),
                        )
                        && runtime_quota.endpoint_credential_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                        )
                        && runtime_quota.endpoint_credential_model_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                            candidate.upstream_model(),
                        )
                },
            ) else {
                return false;
            };
            lease = Some(acquired);
            true
        });

        match (candidate, lease) {
            (Some(candidate), Some(lease)) => Ok(SelectedRouteCredential { candidate, lease }),
            _ => Err(credential_unavailable_error()),
        }
    }

    /// Revalidates a read-only Provider-scoped ranking and acquires one exact request lease.
    ///
    /// The [`ProviderScopedSelection`] is advisory: its aggregate capacity, Health, Quota, and
    /// expiry observations are deliberately not trusted for serving. Every ranked Candidate is
    /// resolved again from this scheduler's immutable Route Snapshot, checked against the exact
    /// Provider scope and caller admission predicate, and then rechecked against the shared live
    /// Health/Quota registries and Credential pool immediately before the atomic lease attempt.
    /// A saturation/expiry/state race simply advances to the next already-ranked Candidate in the
    /// same Provider; it never consumes an Attempt budget slot and never falls through to another
    /// Provider.
    ///
    /// # Errors
    ///
    /// Returns the existing secret-free `CredentialUnavailable/Credential` result when the Route
    /// or ranked Candidate is no longer present, the scope no longer matches, or no ranked
    /// Candidate can pass fresh admission and acquire a live Credential lease.
    #[allow(clippy::too_many_arguments)]
    pub fn select_provider_scoped_and_lease_at<FCandidate, FBinding>(
        &self,
        route_id: &RouteId,
        selection: &ProviderScopedSelection,
        runtime_health: &RuntimeHealthRegistry,
        runtime_quota: &RuntimeQuotaRegistry,
        observed_at_ms: i64,
        mut is_candidate_eligible: FCandidate,
        mut is_binding_eligible: FBinding,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        FCandidate: FnMut(&SnapshotRouteCandidate) -> bool,
        FBinding: FnMut(&SnapshotRouteCandidate, &CredentialId) -> bool,
    {
        let route = self
            .candidates
            .route(route_id)
            .ok_or_else(credential_unavailable_error)?;
        for decision in selection
            .decisions()
            .iter()
            .filter(|decision| decision.is_eligible())
        {
            let ranked = decision.candidate();
            let Some(candidate) = route
                .candidates()
                .iter()
                .find(|candidate| candidate.id() == ranked.candidate_id())
            else {
                continue;
            };
            let Ok(candidate_provider) =
                ProviderId::try_new(candidate.upstream_id().as_str().to_owned())
            else {
                continue;
            };
            if ranked.provider_id() != selection.provider_id()
                || candidate_provider != *selection.provider_id()
                || ranked.channel_id() != candidate.endpoint_id()
                || !candidate.is_hard_eligible()
                || !is_candidate_eligible(candidate)
                || !runtime_health.endpoint_is_available(candidate.endpoint_id())
            {
                continue;
            }
            let Some(lease) = self.credential_pools.try_lease_eligible_at(
                candidate.endpoint_id(),
                observed_at_ms,
                |credential_id| {
                    is_binding_eligible(candidate, credential_id)
                        && runtime_health.endpoint_credential_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                        )
                        && runtime_health.endpoint_credential_model_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                            candidate.upstream_model(),
                        )
                        && runtime_quota.endpoint_credential_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                        )
                        && runtime_quota.endpoint_credential_model_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                            candidate.upstream_model(),
                        )
                },
            ) else {
                continue;
            };
            return Ok(SelectedRouteCredential {
                candidate: candidate.clone(),
                lease,
            });
        }
        Err(credential_unavailable_error())
    }

    /// Acquires one lease for an operator-pinned Route/Provider/Channel/Credential tuple.
    ///
    /// Unlike the normal request selector this method never ranks siblings and never advances
    /// to a second Candidate or Credential.  The immutable Route snapshot is first checked for
    /// exactly one hard-eligible Candidate whose upstream identifies `provider_id` and whose
    /// Endpoint is `channel_id`; only that Candidate's Endpoint pool is then consulted for the
    /// exact `credential_id`.  Health, Quota, expiry, and capacity are all revalidated immediately
    /// before the atomic lease attempt.  Any mismatch, duplicate pin, stale state, or saturation
    /// returns the same secret-free `CredentialUnavailable` error without acquiring a lease.
    ///
    /// The caller-supplied predicate is intentionally evaluated before runtime state and pool
    /// access.  It is useful for management diagnostics to enforce protocol/adapter admission
    /// without creating a second selector or a Provider fallback path.
    ///
    /// # Errors
    ///
    /// Returns the secret-free `CredentialUnavailable` error when the route is absent, the pin
    /// is ambiguous or mismatched, runtime Health/Quota rejects the target, the credential is
    /// expired, or the exact pool cannot acquire capacity.
    #[allow(clippy::too_many_arguments)]
    pub fn select_pinned_and_lease_at<FCandidate>(
        &self,
        route_id: &RouteId,
        provider_id: &ProviderId,
        channel_id: &EndpointId,
        credential_id: &CredentialId,
        runtime_health: &RuntimeHealthRegistry,
        runtime_quota: &RuntimeQuotaRegistry,
        observed_at_ms: i64,
        mut is_candidate_eligible: FCandidate,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        FCandidate: FnMut(&SnapshotRouteCandidate) -> bool,
    {
        let route = self
            .candidates
            .route(route_id)
            .ok_or_else(credential_unavailable_error)?;
        let mut matching = route.candidates().iter().filter(|candidate| {
            candidate.is_hard_eligible()
                && candidate.endpoint_id() == channel_id
                && ProviderId::try_new(candidate.upstream_id().as_str().to_owned())
                    .is_ok_and(|candidate_provider| candidate_provider == *provider_id)
                && is_candidate_eligible(candidate)
        });
        let Some(candidate) = matching.next() else {
            return Err(credential_unavailable_error());
        };
        // A pin must identify one immutable route candidate.  Treat duplicate provider/channel
        // entries as an ambiguous target instead of silently choosing by the route cursor.
        if matching.next().is_some() {
            return Err(credential_unavailable_error());
        }
        if !runtime_health.endpoint_is_available(candidate.endpoint_id()) {
            return Err(credential_unavailable_error());
        }
        let Some(lease) = self.credential_pools.try_lease_exact_eligible_at(
            candidate.endpoint_id(),
            credential_id,
            observed_at_ms,
            |candidate_credential_id| {
                runtime_health.endpoint_credential_is_available(
                    candidate.endpoint_id(),
                    candidate_credential_id,
                ) && runtime_health.endpoint_credential_model_is_available(
                    candidate.endpoint_id(),
                    candidate_credential_id,
                    candidate.upstream_model(),
                ) && runtime_quota.endpoint_credential_is_available(
                    candidate.endpoint_id(),
                    candidate_credential_id,
                ) && runtime_quota.endpoint_credential_model_is_available(
                    candidate.endpoint_id(),
                    candidate_credential_id,
                    candidate.upstream_model(),
                )
            },
        ) else {
            return Err(credential_unavailable_error());
        };
        Ok(SelectedRouteCredential {
            candidate: candidate.clone(),
            lease,
        })
    }

    /// Acquires the one exact Candidate and Credential revision retained by stored history.
    ///
    /// Every immutable identity is matched before pool access. The required Provider capability,
    /// runtime Health/Quota, expiry, and capacity are then revalidated at `observed_at_ms`. The
    /// direct exact-revision pool API never reads or advances ordinary routing cursors.
    ///
    /// # Errors
    ///
    /// Returns the fixed secret-free `CredentialUnavailable` error for any stale lineage,
    /// capability mismatch, runtime blocker, revision rotation, or capacity failure.
    #[allow(clippy::too_many_arguments)]
    pub fn select_continuation_and_lease_at<FCandidate>(
        &self,
        route_id: &RouteId,
        provider_id: &ProviderId,
        upstream_id: &gateway_core::UpstreamId,
        channel_id: &EndpointId,
        route_candidate_id: &RouteCandidateId,
        credential_id: &CredentialId,
        credential_revision: u64,
        required_capability: SemanticCapability,
        runtime_health: &RuntimeHealthRegistry,
        runtime_quota: &RuntimeQuotaRegistry,
        observed_at_ms: i64,
        mut is_candidate_eligible: FCandidate,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        FCandidate: FnMut(&SnapshotRouteCandidate) -> bool,
    {
        let route = self
            .candidates
            .route(route_id)
            .ok_or_else(credential_unavailable_error)?;
        let candidate = route
            .candidates()
            .iter()
            .find(|candidate| candidate.id() == route_candidate_id)
            .filter(|candidate| {
                candidate.is_hard_eligible()
                    && candidate.upstream_id() == upstream_id
                    && candidate.endpoint_id() == channel_id
                    && candidate
                        .effective_capabilities()
                        .supports(required_capability)
                    && ProviderId::try_new(candidate.upstream_id().as_str().to_owned())
                        .is_ok_and(|candidate_provider| candidate_provider == *provider_id)
                    && is_candidate_eligible(candidate)
            })
            .ok_or_else(credential_unavailable_error)?;
        if !runtime_health.endpoint_is_available(candidate.endpoint_id()) {
            return Err(credential_unavailable_error());
        }
        let Some(lease) = self.credential_pools.try_lease_exact_revision_eligible_at(
            candidate.endpoint_id(),
            credential_id,
            credential_revision,
            observed_at_ms,
            |candidate_credential_id| {
                runtime_health.endpoint_credential_is_available(
                    candidate.endpoint_id(),
                    candidate_credential_id,
                ) && runtime_health.endpoint_credential_model_is_available(
                    candidate.endpoint_id(),
                    candidate_credential_id,
                    candidate.upstream_model(),
                ) && runtime_quota.endpoint_credential_is_available(
                    candidate.endpoint_id(),
                    candidate_credential_id,
                ) && runtime_quota.endpoint_credential_model_is_available(
                    candidate.endpoint_id(),
                    candidate_credential_id,
                    candidate.upstream_model(),
                )
            },
        ) else {
            return Err(credential_unavailable_error());
        };
        Ok(SelectedRouteCredential {
            candidate: candidate.clone(),
            lease,
        })
    }

    /// Selects one binding whose only Quota blocker is a due controlled recovery.
    ///
    /// Health predicates are unchanged and run first, so a Cooldown, Circuit, or forbidden
    /// account can never be admitted as a quota probe. The binding-wide quota target must be
    /// `RecoveryRequired` -- its latest exhausted-window Reset has passed -- and the Candidate's
    /// exact model target must be quota-available. The caller owns beginning the single
    /// non-cloneable recovery ticket after this lease, so losing that race releases Credential
    /// capacity instead of leaking an unused ticket.
    ///
    /// # Errors
    ///
    /// Returns the same secret-free `CredentialUnavailable/Credential` result when no Candidate
    /// has a due binding that passes eligibility, Health, and bounded lease acquisition.
    pub fn select_eligible_and_lease_for_quota_recovery<FCandidate, FBinding>(
        &self,
        route_id: &RouteId,
        runtime_health: &RuntimeHealthRegistry,
        runtime_quota: &RuntimeQuotaRegistry,
        is_candidate_eligible: FCandidate,
        is_binding_eligible: FBinding,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        FCandidate: FnMut(&SnapshotRouteCandidate) -> bool,
        FBinding: FnMut(&SnapshotRouteCandidate, &CredentialId) -> bool,
    {
        self.select_eligible_and_lease_for_quota_recovery_at(
            route_id,
            runtime_health,
            runtime_quota,
            LEGACY_NO_EXPIRY_OBSERVATION_MS,
            is_candidate_eligible,
            is_binding_eligible,
        )
    }

    /// Expiry-aware variant of [`Self::select_eligible_and_lease_for_quota_recovery`].
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` when no current Credential is eligible for a
    /// due quota-recovery probe.
    pub fn select_eligible_and_lease_for_quota_recovery_at<FCandidate, FBinding>(
        &self,
        route_id: &RouteId,
        runtime_health: &RuntimeHealthRegistry,
        runtime_quota: &RuntimeQuotaRegistry,
        observed_at_ms: i64,
        mut is_candidate_eligible: FCandidate,
        mut is_binding_eligible: FBinding,
    ) -> Result<SelectedRouteCredential, GatewayError>
    where
        FCandidate: FnMut(&SnapshotRouteCandidate) -> bool,
        FBinding: FnMut(&SnapshotRouteCandidate, &CredentialId) -> bool,
    {
        let mut lease = None;
        let candidate = self.candidates.select_eligible(route_id, |candidate| {
            if !is_candidate_eligible(candidate)
                || !runtime_health.endpoint_is_available(candidate.endpoint_id())
            {
                return false;
            }
            let Some(acquired) = self.credential_pools.try_lease_eligible_at(
                candidate.endpoint_id(),
                observed_at_ms,
                |credential_id| {
                    is_binding_eligible(candidate, credential_id)
                        && runtime_health.endpoint_credential_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                        )
                        && runtime_health.endpoint_credential_model_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                            candidate.upstream_model(),
                        )
                        && binding_quota_is_recovery_due(
                            runtime_quota,
                            candidate.endpoint_id(),
                            credential_id,
                        )
                        && runtime_quota.endpoint_credential_model_is_available(
                            candidate.endpoint_id(),
                            credential_id,
                            candidate.upstream_model(),
                        )
                },
            ) else {
                return false;
            };
            lease = Some(acquired);
            true
        });

        match (candidate, lease) {
            (Some(candidate), Some(lease)) => Ok(SelectedRouteCredential { candidate, lease }),
            _ => Err(credential_unavailable_error()),
        }
    }

    /// Explains Candidate and Credential eligibility without acquiring a lease or advancing a cursor.
    ///
    /// The caller supplies one explicit observation time and deterministic schedule starts through
    /// [`RouteExplainInput`]. The returned projection is therefore suitable for a fixed diagnostic
    /// snapshot, not a promise of a later concurrent request outcome.
    ///
    /// # Errors
    ///
    /// Returns [`RouteExplainError`] only when the exact immutable Snapshot lacks the requested
    /// Route or its precompiled schedule. Runtime Health/Quota read failures are represented as
    /// secret-free fail-closed exclusion reasons instead.
    pub fn explain(
        &self,
        input: &RouteExplainInput,
        runtime_health: &RuntimeHealthRegistry,
        runtime_quota: &RuntimeQuotaRegistry,
        exclusions: &AttemptExclusionSet,
    ) -> Result<RouteExplainSnapshot, RouteExplainError> {
        crate::route_explain::explain(
            self.candidates.snapshot(),
            &self.credential_pools,
            runtime_health,
            runtime_quota,
            input,
            exclusions,
        )
    }

    /// Composes the existing Route Explain evidence with one exact Provider-scoped selector.
    ///
    /// This is a read-only management projection. It only hands base-eligible, explicitly
    /// admitted Candidates to the selector, reuses the supplied Health/Quota registries, and
    /// never advances Candidate/Credential cursors or acquires a lease.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderScopedRouteExplainError`] when the immutable Route, bounded input, or
    /// runtime observation cannot be represented safely.
    pub fn explain_provider_scoped(
        &self,
        input: &ProviderScopedRouteExplainInput,
        runtime_health: &RuntimeHealthRegistry,
        runtime_quota: &RuntimeQuotaRegistry,
        exclusions: &AttemptExclusionSet,
    ) -> Result<ProviderScopedRouteExplainSnapshot, ProviderScopedRouteExplainError> {
        let candidate_price_rates = input
            .admitted_candidate_ids()
            .iter()
            .filter_map(|candidate_id| {
                self.provider_price_rates_for_candidate(candidate_id)
                    .map(|rates| (candidate_id.clone(), rates))
            })
            .collect();
        let scheduler_input = input.with_candidate_price_rates(candidate_price_rates)?;
        crate::route_explain::explain_provider_scoped(
            self.candidates.snapshot(),
            &self.credential_pools,
            runtime_health,
            runtime_quota,
            &scheduler_input,
            exclusions,
        )
    }

    /// Returns a copy of one Route from the exact immutable Snapshot used for scheduling.
    ///
    /// Attempt orchestration uses the copied Route only for its validated retry budget. The result
    /// contains no Credential or Secret material and does not advance a scheduler cursor.
    #[must_use]
    pub fn route(&self, route_id: &RouteId) -> Option<crate::SnapshotRoute> {
        self.candidates.route(route_id).cloned()
    }
}

/// One selected non-secret Candidate paired with its live request-scoped Credential lease.
///
/// Dropping this value drops the underlying [`CredentialLease`] and therefore releases capacity
/// even if a caller is cancelled before starting transport.
pub struct SelectedRouteCredential {
    candidate: SnapshotRouteCandidate,
    lease: CredentialLease,
}

impl SelectedRouteCredential {
    /// Returns the selected compiler-approved Route Candidate.
    #[must_use]
    pub fn candidate(&self) -> &SnapshotRouteCandidate {
        &self.candidate
    }

    /// Returns the live request-scoped Endpoint Credential lease.
    #[must_use]
    pub fn lease(&self) -> &CredentialLease {
        &self.lease
    }

    /// Consumes the selection into its Candidate and live Credential lease.
    #[must_use]
    pub fn into_parts(self) -> (SnapshotRouteCandidate, CredentialLease) {
        (self.candidate, self.lease)
    }
}

impl std::fmt::Debug for SelectedRouteCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SelectedRouteCredential")
            .field("candidate", &self.candidate)
            .field("lease", &self.lease)
            .finish()
    }
}

const fn credential_unavailable_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::CredentialUnavailable,
        ErrorScope::Credential,
    )
}

fn binding_quota_is_recovery_due(
    runtime_quota: &RuntimeQuotaRegistry,
    endpoint_id: &EndpointId,
    credential_id: &CredentialId,
) -> bool {
    runtime_quota
        .availability(&RuntimeQuotaTarget::endpoint_credential(
            endpoint_id.clone(),
            credential_id.clone(),
        ))
        .is_ok_and(|availability| {
            matches!(
                availability,
                RuntimeQuotaAvailability::RecoveryRequired { .. }
            )
        })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        error::Error,
        io,
        sync::{
            Arc, Barrier,
            atomic::{AtomicI64, Ordering},
        },
        thread,
        time::Duration,
    };

    use gateway_catalog::{CapabilitySet, CatalogModelState, SemanticCapability};
    use gateway_core::{
        CredentialId, EndpointId, ErrorScope, GatewayErrorCode, ProviderId, PublicModelId,
        RouteCandidateId, RouteId, UpstreamId,
    };
    use gateway_upstream::{
        CredentialSecret, EndpointCredentialInput, EndpointCredentialPool, EndpointCredentialPools,
    };

    use super::RouteCredentialScheduler;
    use crate::{
        QuotaConfidence, QuotaSnapshot, QuotaSource, QuotaWindow, RouteSnapshot,
        RouteSnapshotInput, RuntimeHealthClock, RuntimeHealthClockError, RuntimeHealthKey,
        RuntimeHealthRegistry, RuntimeQuotaRegistry, RuntimeQuotaTarget, SnapshotCatalogAdmission,
        SnapshotPublicModel, SnapshotRoute, SnapshotRouteCandidate, SnapshotRouteCandidateInput,
        SnapshotRoutePolicy, SnapshotTransformMode, SnapshotVersion,
    };

    type TestResult = Result<(), Box<dyn Error>>;
    type CountMap = BTreeMap<String, usize>;
    type WorkerCounts = (CountMap, CountMap);
    type CandidateSpec<'a> = (&'a str, &'a str, i64, i64);
    type CredentialSpec<'a> = (&'a str, i64, i64, i64);
    type PoolSpec<'a> = (&'a str, Vec<CredentialSpec<'a>>);

    #[test]
    fn two_layer_atomic_cursors_preserve_route_and_endpoint_weights_under_concurrency() -> TestResult
    {
        let worker_count = 8_usize;
        let selections_per_worker = 50_usize;
        let per_credential_concurrency = i64::try_from(worker_count)?;
        let (scheduler, route_id) = scheduler(
            vec![
                ("candidate-a", "endpoint-a", 0, 3),
                ("candidate-b", "endpoint-b", 0, 1),
            ],
            vec![
                (
                    "endpoint-a",
                    // This test isolates cursor distribution. Every pool can serve every active
                    // worker, so a transient saturation cannot change the selected slot.
                    vec![
                        ("credential-a-one", 0, 1, per_credential_concurrency),
                        ("credential-a-two", 0, 1, per_credential_concurrency),
                    ],
                ),
                (
                    "endpoint-b",
                    vec![("credential-b", 0, 1, per_credential_concurrency)],
                ),
            ],
        )?;
        let scheduler = Arc::new(scheduler);
        let barrier = Arc::new(Barrier::new(worker_count));
        let mut workers = Vec::new();

        for _ in 0..worker_count {
            let scheduler = Arc::clone(&scheduler);
            let route_id = route_id.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || -> Result<WorkerCounts, String> {
                barrier.wait();
                let mut candidate_counts = BTreeMap::new();
                let mut credential_counts = BTreeMap::new();
                for _ in 0..selections_per_worker {
                    let selected = scheduler
                        .select_and_lease(&route_id)
                        .map_err(|error| error.safe_message().to_owned())?;
                    *candidate_counts
                        .entry(selected.candidate().id().as_str().to_owned())
                        .or_default() += 1;
                    *credential_counts
                        .entry(selected.lease().credential_id().as_str().to_owned())
                        .or_default() += 1;
                }
                Ok((candidate_counts, credential_counts))
            }));
        }

        let mut candidate_counts = BTreeMap::new();
        let mut credential_counts = BTreeMap::new();
        for worker in workers {
            let (worker_candidates, worker_credentials) = worker
                .join()
                .map_err(|_| io::Error::other("two-layer scheduler worker panicked"))?
                .map_err(io::Error::other)?;
            merge_counts(&mut candidate_counts, worker_candidates);
            merge_counts(&mut credential_counts, worker_credentials);
        }

        assert_eq!(candidate_counts.get("candidate-a"), Some(&300));
        assert_eq!(candidate_counts.get("candidate-b"), Some(&100));
        assert_eq!(credential_counts.get("credential-a-one"), Some(&150));
        assert_eq!(credential_counts.get("credential-a-two"), Some(&150));
        assert_eq!(credential_counts.get("credential-b"), Some(&100));
        Ok(())
    }

    #[test]
    fn saturated_preferred_candidate_falls_back_without_leaking_a_lease() -> TestResult {
        let (scheduler, route_id) = scheduler(
            vec![
                ("candidate-preferred", "endpoint-preferred", 0, 1),
                ("candidate-fallback", "endpoint-fallback", 1, 1),
            ],
            vec![
                (
                    "endpoint-preferred",
                    vec![("credential-preferred", 0, 1, 1)],
                ),
                ("endpoint-fallback", vec![("credential-fallback", 0, 1, 1)]),
            ],
        )?;

        let first = scheduler.select_and_lease(&route_id)?;
        assert_eq!(first.candidate().id().as_str(), "candidate-preferred");
        let second = scheduler.select_and_lease(&route_id)?;
        assert_eq!(second.candidate().id().as_str(), "candidate-fallback");
        let Err(error) = scheduler.select_and_lease(&route_id) else {
            return Err("all Credentials should be saturated".into());
        };
        assert_eq!(error.code(), GatewayErrorCode::CredentialUnavailable);
        assert_eq!(error.scope(), ErrorScope::Credential);
        drop(first);
        let resumed = scheduler.select_and_lease(&route_id)?;
        assert_eq!(resumed.candidate().id().as_str(), "candidate-preferred");
        Ok(())
    }

    #[test]
    fn caller_predicate_can_exclude_a_candidate_before_its_pool_is_touched() -> TestResult {
        let (scheduler, route_id) = scheduler(
            vec![
                ("candidate-a", "endpoint-a", 0, 1),
                ("candidate-b", "endpoint-b", 0, 1),
            ],
            vec![
                ("endpoint-a", vec![("credential-a", 0, 1, 1)]),
                ("endpoint-b", vec![("credential-b", 0, 1, 1)]),
            ],
        )?;

        let selected = scheduler.select_eligible_and_lease(&route_id, |candidate| {
            candidate.id().as_str() != "candidate-a"
        })?;
        assert_eq!(selected.candidate().id().as_str(), "candidate-b");
        assert_eq!(selected.lease().credential_id().as_str(), "credential-b");
        Ok(())
    }

    #[test]
    fn binding_predicate_skips_an_attempted_credential_before_a_new_lease() -> TestResult {
        let (scheduler, route_id) = scheduler(
            vec![("candidate-a", "endpoint-a", 0, 1)],
            vec![(
                "endpoint-a",
                vec![("credential-a", 0, 1, 1), ("credential-b", 0, 1, 1)],
            )],
        )?;
        let runtime_health = RuntimeHealthRegistry::new();

        let selected = scheduler.select_eligible_and_lease_with_runtime_health_and_binding(
            &route_id,
            &runtime_health,
            |_| true,
            |candidate, credential_id| {
                candidate.id().as_str() != "candidate-a" || credential_id.as_str() != "credential-a"
            },
        )?;

        assert_eq!(selected.candidate().id().as_str(), "candidate-a");
        assert_eq!(selected.lease().credential_id().as_str(), "credential-b");
        Ok(())
    }

    #[test]
    fn pinned_selection_requires_exact_provider_channel_and_credential() -> TestResult {
        let (scheduler, route_id) = scheduler(
            vec![
                ("candidate-a", "endpoint-a", 0, 1),
                ("candidate-b", "endpoint-b", 1, 1),
            ],
            vec![
                (
                    "endpoint-a",
                    vec![("credential-a", 0, 1, 1), ("credential-b", 0, 1, 1)],
                ),
                ("endpoint-b", vec![("credential-foreign", 0, 1, 1)]),
            ],
        )?;
        let health = RuntimeHealthRegistry::new();
        let quota = RuntimeQuotaRegistry::new();
        let selected = scheduler.select_pinned_and_lease_at(
            &route_id,
            &ProviderId::try_new("upstream-endpoint-a")?,
            &EndpointId::try_new("endpoint-a")?,
            &CredentialId::try_new("credential-b")?,
            &health,
            &quota,
            100,
            |_| true,
        )?;
        assert_eq!(selected.candidate().id().as_str(), "candidate-a");
        assert_eq!(selected.lease().credential_id().as_str(), "credential-b");
        drop(selected);

        let mismatch = scheduler.select_pinned_and_lease_at(
            &route_id,
            &ProviderId::try_new("upstream-endpoint-b")?,
            &EndpointId::try_new("endpoint-a")?,
            &CredentialId::try_new("credential-b")?,
            &health,
            &quota,
            100,
            |_| true,
        );
        assert!(mismatch.is_err());
        assert_eq!(
            scheduler
                .credential_pools
                .pool(&EndpointId::try_new("endpoint-a")?)
                .and_then(|pool| {
                    pool.active_lease_count(&CredentialId::try_new("credential-b").ok()?)
                }),
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn pinned_selection_does_not_advance_route_or_credential_cursors() -> TestResult {
        let build = || {
            scheduler(
                vec![
                    ("candidate-a", "endpoint-a", 0, 3),
                    ("candidate-b", "endpoint-b", 0, 1),
                ],
                vec![
                    (
                        "endpoint-a",
                        vec![("credential-a-one", 0, 3, 1), ("credential-a-two", 0, 1, 1)],
                    ),
                    ("endpoint-b", vec![("credential-b", 0, 1, 1)]),
                ],
            )
        };
        let (control, route_id) = build()?;
        let (exercised, exercised_route_id) = build()?;
        let control_prefix = control.select_and_lease(&route_id)?;
        let exercised_prefix = exercised.select_and_lease(&exercised_route_id)?;
        assert_eq!(
            (
                control_prefix.candidate().id(),
                control_prefix.lease().credential_id()
            ),
            (
                exercised_prefix.candidate().id(),
                exercised_prefix.lease().credential_id()
            )
        );
        drop(control_prefix);
        drop(exercised_prefix);

        let health = RuntimeHealthRegistry::new();
        let quota = RuntimeQuotaRegistry::new();
        let pin = exercised.select_pinned_and_lease_at(
            &exercised_route_id,
            &ProviderId::try_new("upstream-endpoint-a")?,
            &EndpointId::try_new("endpoint-a")?,
            &CredentialId::try_new("credential-a-two")?,
            &health,
            &quota,
            100,
            |_| true,
        )?;
        drop(pin);

        for _ in 0..12 {
            let control_selection = control.select_and_lease(&route_id)?;
            let exercised_selection = exercised.select_and_lease(&exercised_route_id)?;
            assert_eq!(
                (
                    control_selection.candidate().id(),
                    control_selection.lease().credential_id()
                ),
                (
                    exercised_selection.candidate().id(),
                    exercised_selection.lease().credential_id()
                ),
                "pinned selection changed the ordinary route/pool cursor sequence"
            );
            drop(control_selection);
            drop(exercised_selection);
        }
        Ok(())
    }

    #[test]
    fn pinned_selection_fails_closed_for_health_quota_and_capacity() -> TestResult {
        let (scheduler, route_id) = scheduler(
            vec![("candidate-a", "endpoint-a", 0, 1)],
            vec![("endpoint-a", vec![("credential-a", 0, 1, 1)])],
        )?;
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential = CredentialId::try_new("credential-a")?;
        let provider = ProviderId::try_new("upstream-endpoint-a")?;
        let clock = Arc::new(FixedRuntimeHealthClock::new(100));
        let quota = RuntimeQuotaRegistry::with_clock(clock.clone());

        let health = RuntimeHealthRegistry::with_clock(clock.clone());
        health.cool_down_until(RuntimeHealthKey::endpoint(endpoint.clone()), 200)?;
        assert!(
            scheduler
                .select_pinned_and_lease_at(
                    &route_id,
                    &provider,
                    &endpoint,
                    &credential,
                    &health,
                    &quota,
                    100,
                    |_| true,
                )
                .is_err()
        );

        let health = RuntimeHealthRegistry::with_clock(clock.clone());
        let quota = RuntimeQuotaRegistry::with_clock(clock);
        quota.record_rate_limited(
            RuntimeQuotaTarget::endpoint_credential(endpoint.clone(), credential.clone()),
            100,
            Some(Duration::from_millis(100)),
            Duration::from_millis(100),
        )?;
        assert!(
            scheduler
                .select_pinned_and_lease_at(
                    &route_id,
                    &provider,
                    &endpoint,
                    &credential,
                    &health,
                    &quota,
                    100,
                    |_| true,
                )
                .is_err()
        );

        let quota = RuntimeQuotaRegistry::new();
        let held = scheduler.select_and_lease(&route_id)?;
        assert!(
            scheduler
                .select_pinned_and_lease_at(
                    &route_id,
                    &provider,
                    &endpoint,
                    &credential,
                    &health,
                    &quota,
                    100,
                    |_| true,
                )
                .is_err()
        );
        drop(held);
        assert_eq!(
            scheduler
                .select_pinned_and_lease_at(
                    &route_id,
                    &provider,
                    &endpoint,
                    &credential,
                    &health,
                    &quota,
                    100,
                    |_| true,
                )?
                .lease()
                .credential_id()
                .as_str(),
            "credential-a"
        );
        Ok(())
    }

    #[test]
    fn pinned_selection_rejects_expired_credential_without_lease() -> TestResult {
        let route_id = RouteId::try_new("route-pinned-expired")?;
        let public_model_id = PublicModelId::try_new("public-model-pinned-expired")?;
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new("version-pinned-expired")?,
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
                1,
                1_000,
                vec![candidate("candidate-expired", "endpoint-expired", 0, 1)?],
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let pool = EndpointCredentialPool::try_new(
            EndpointId::try_new("endpoint-expired")?,
            [EndpointCredentialInput {
                credential_id: CredentialId::try_new("credential-expired")?,
                credential_kind: "api_key".to_owned(),
                credential_revision: 1,
                priority: 0,
                weight: 1,
                concurrency: 1,
                expires_at_ms: Some(100),
                secret: CredentialSecret::try_new(b"expired".to_vec())?,
            }],
        )?;
        let pools = Arc::new(EndpointCredentialPools::try_new([pool])?);
        let scheduler = RouteCredentialScheduler::new(snapshot, pools.clone());
        let result = scheduler.select_pinned_and_lease_at(
            &route_id,
            &ProviderId::try_new("upstream-endpoint-expired")?,
            &EndpointId::try_new("endpoint-expired")?,
            &CredentialId::try_new("credential-expired")?,
            &RuntimeHealthRegistry::new(),
            &RuntimeQuotaRegistry::new(),
            100,
            |_| true,
        );
        assert!(result.is_err());
        assert_eq!(
            pools
                .pool(&EndpointId::try_new("endpoint-expired")?)
                .and_then(|pool| {
                    pool.active_lease_count(&CredentialId::try_new("credential-expired").ok()?)
                }),
            Some(0)
        );
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One matrix keeps exact lineage, revision, and both capabilities auditable together.
    fn continuation_selection_requires_exact_lineage_revision_and_capability() -> TestResult {
        let route_id = RouteId::try_new("route-continuity")?;
        let public_model_id = PublicModelId::try_new("public-model-continuity")?;
        let continuity_capabilities = CapabilitySet::try_new([
            SemanticCapability::StoredResponses,
            SemanticCapability::ResponseCompaction,
        ])?;
        let exact_candidate = SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new("candidate-continuity")?,
            endpoint_id: EndpointId::try_new("endpoint-continuity")?,
            upstream_id: UpstreamId::try_new("upstream-continuity")?,
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: "upstream-model".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: continuity_capabilities,
            catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
            active_binding_count: 2,
        });
        let foreign_candidate = SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new("candidate-foreign")?,
            endpoint_id: EndpointId::try_new("endpoint-foreign")?,
            upstream_id: UpstreamId::try_new("upstream-foreign")?,
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: "upstream-model".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 1,
            weight: 1,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
            active_binding_count: 1,
        });
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
                1_000,
                vec![exact_candidate, foreign_candidate],
            )],
            Vec::new(),
            Vec::new(),
        ))?);
        let exact_pool = EndpointCredentialPool::try_new(
            EndpointId::try_new("endpoint-continuity")?,
            [
                EndpointCredentialInput {
                    credential_id: CredentialId::try_new("credential-continuity")?,
                    credential_kind: "oauth".to_owned(),
                    credential_revision: 11,
                    priority: 0,
                    weight: 1,
                    concurrency: 1,
                    expires_at_ms: Some(1_000),
                    secret: CredentialSecret::try_new(b"exact-secret".to_vec())?,
                },
                EndpointCredentialInput {
                    credential_id: CredentialId::try_new("credential-sibling")?,
                    credential_kind: "oauth".to_owned(),
                    credential_revision: 12,
                    priority: 0,
                    weight: 1,
                    concurrency: 1,
                    expires_at_ms: Some(1_000),
                    secret: CredentialSecret::try_new(b"sibling-secret".to_vec())?,
                },
            ],
        )?;
        let foreign_pool =
            endpoint_pool("endpoint-foreign", vec![("credential-foreign", 0, 1, 1)])?;
        let pools = Arc::new(EndpointCredentialPools::try_new([
            exact_pool,
            foreign_pool,
        ])?);
        let scheduler = RouteCredentialScheduler::new(snapshot, Arc::clone(&pools));
        let health = RuntimeHealthRegistry::new();
        let quota = RuntimeQuotaRegistry::new();

        for capability in [
            SemanticCapability::StoredResponses,
            SemanticCapability::ResponseCompaction,
        ] {
            let selected = scheduler.select_continuation_and_lease_at(
                &route_id,
                &ProviderId::try_new("upstream-continuity")?,
                &UpstreamId::try_new("upstream-continuity")?,
                &EndpointId::try_new("endpoint-continuity")?,
                &RouteCandidateId::try_new("candidate-continuity")?,
                &CredentialId::try_new("credential-continuity")?,
                11,
                capability,
                &health,
                &quota,
                100,
                |_| true,
            )?;
            assert_eq!(selected.candidate().id().as_str(), "candidate-continuity");
            assert_eq!(
                selected.lease().credential_id().as_str(),
                "credential-continuity"
            );
            drop(selected);
        }

        for (candidate, revision, capability) in [
            (
                "candidate-continuity",
                12,
                SemanticCapability::StoredResponses,
            ),
            ("candidate-foreign", 0, SemanticCapability::StoredResponses),
            (
                "candidate-foreign",
                0,
                SemanticCapability::ResponseCompaction,
            ),
        ] {
            let is_exact = candidate == "candidate-continuity";
            let provider = ProviderId::try_new(if is_exact {
                "upstream-continuity"
            } else {
                "upstream-foreign"
            })?;
            let upstream = UpstreamId::try_new(if is_exact {
                "upstream-continuity"
            } else {
                "upstream-foreign"
            })?;
            let endpoint = EndpointId::try_new(if is_exact {
                "endpoint-continuity"
            } else {
                "endpoint-foreign"
            })?;
            let credential = CredentialId::try_new(if is_exact {
                "credential-continuity"
            } else {
                "credential-foreign"
            })?;
            assert!(
                scheduler
                    .select_continuation_and_lease_at(
                        &route_id,
                        &provider,
                        &upstream,
                        &endpoint,
                        &RouteCandidateId::try_new(candidate)?,
                        &credential,
                        revision,
                        capability,
                        &health,
                        &quota,
                        100,
                        |_| true,
                    )
                    .is_err()
            );
        }
        assert_eq!(
            pools
                .pool(&EndpointId::try_new("endpoint-continuity")?)
                .and_then(|pool| pool
                    .active_lease_count(&CredentialId::try_new("credential-continuity").ok()?)),
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn runtime_health_filters_endpoints_and_credentials_without_distorting_fallback() -> TestResult
    {
        let (scheduler, route_id) = scheduler(
            vec![
                ("candidate-a", "endpoint-a", 0, 1),
                ("candidate-b", "endpoint-b", 1, 1),
            ],
            vec![
                (
                    "endpoint-a",
                    vec![("credential-a-one", 0, 1, 1), ("credential-a-two", 0, 1, 1)],
                ),
                ("endpoint-b", vec![("credential-b", 0, 1, 1)]),
            ],
        )?;
        let clock = Arc::new(FixedRuntimeHealthClock::new(100));
        let runtime_health = RuntimeHealthRegistry::with_clock(clock.clone());
        let endpoint_a = EndpointId::try_new("endpoint-a")?;
        let credential_a_one = CredentialId::try_new("credential-a-one")?;

        runtime_health.cool_down_until(
            RuntimeHealthKey::endpoint_credential(endpoint_a.clone(), credential_a_one),
            200,
        )?;
        let selected = scheduler.select_runtime_eligible_and_lease(&route_id, &runtime_health)?;
        assert_eq!(selected.candidate().id().as_str(), "candidate-a");
        assert_eq!(
            selected.lease().credential_id().as_str(),
            "credential-a-two"
        );
        drop(selected);

        runtime_health.cool_down_until(RuntimeHealthKey::endpoint(endpoint_a), 200)?;
        let fallback = scheduler.select_runtime_eligible_and_lease(&route_id, &runtime_health)?;
        assert_eq!(fallback.candidate().id().as_str(), "candidate-b");
        assert_eq!(fallback.lease().credential_id().as_str(), "credential-b");
        Ok(())
    }

    #[test]
    fn model_scoped_circuit_skips_only_the_failed_endpoint_credential_binding() -> TestResult {
        let (scheduler, route_id) = scheduler(
            vec![("candidate-a", "endpoint-a", 0, 1)],
            vec![(
                "endpoint-a",
                vec![("credential-a", 0, 1, 1), ("credential-b", 0, 1, 1)],
            )],
        )?;
        let clock = Arc::new(FixedRuntimeHealthClock::new(100));
        let runtime_health = RuntimeHealthRegistry::with_clock(clock);
        runtime_health.open_circuit_until(
            RuntimeHealthKey::endpoint_credential_model(
                EndpointId::try_new("endpoint-a")?,
                CredentialId::try_new("credential-a")?,
                "upstream-model",
            ),
            200,
        )?;

        let selected = scheduler.select_runtime_eligible_and_lease(&route_id, &runtime_health)?;
        assert_eq!(selected.candidate().id().as_str(), "candidate-a");
        assert_eq!(selected.lease().credential_id().as_str(), "credential-b");
        Ok(())
    }

    #[test]
    fn model_quota_filters_before_lease_and_reset_needs_controlled_recovery() -> TestResult {
        let (scheduler, route_id) = scheduler(
            vec![
                ("candidate-a", "endpoint-a", 0, 1),
                ("candidate-b", "endpoint-b", 1, 1),
            ],
            vec![
                ("endpoint-a", vec![("credential-a", 0, 1, 1)]),
                ("endpoint-b", vec![("credential-b", 0, 1, 1)]),
            ],
        )?;
        let clock = Arc::new(FixedRuntimeHealthClock::new(100));
        let runtime_health = RuntimeHealthRegistry::with_clock(clock.clone());
        let runtime_quota = RuntimeQuotaRegistry::with_clock(clock.clone());
        let quota_target = RuntimeQuotaTarget::endpoint_credential_model(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new("credential-a")?,
            "upstream-model",
        )?;
        runtime_quota.record_snapshot(QuotaSnapshot::try_new(
            quota_target.clone(),
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

        let while_exhausted = scheduler
            .select_eligible_and_lease_with_runtime_health_quota_and_binding(
                &route_id,
                &runtime_health,
                &runtime_quota,
                |_| true,
                |_, _| true,
            )?;
        assert_eq!(while_exhausted.candidate().id().as_str(), "candidate-b");
        drop(while_exhausted);

        clock.set_now_ms(200);
        let after_reset_without_ticket = scheduler
            .select_eligible_and_lease_with_runtime_health_quota_and_binding(
                &route_id,
                &runtime_health,
                &runtime_quota,
                |_| true,
                |_, _| true,
            )?;
        assert_eq!(
            after_reset_without_ticket.candidate().id().as_str(),
            "candidate-b"
        );
        drop(after_reset_without_ticket);

        let ticket = runtime_quota
            .begin_recovery_probe(&quota_target, 250)?
            .ok_or("due quota did not issue a controlled recovery ticket")?;
        let while_probe_in_flight = scheduler
            .select_eligible_and_lease_with_runtime_health_quota_and_binding(
                &route_id,
                &runtime_health,
                &runtime_quota,
                |_| true,
                |_, _| true,
            )?;
        assert_eq!(
            while_probe_in_flight.candidate().id().as_str(),
            "candidate-b"
        );
        drop(while_probe_in_flight);

        runtime_quota.complete_recovery_probe(
            ticket,
            QuotaSnapshot::try_new(
                quota_target,
                vec![QuotaWindow::try_new("requests", Some(10), Some(10), None)?],
                QuotaSource::Rest,
                QuotaConfidence::Observed,
                200,
            )?,
        )?;
        let recovered = scheduler.select_eligible_and_lease_with_runtime_health_quota_and_binding(
            &route_id,
            &runtime_health,
            &runtime_quota,
            |_| true,
            |_, _| true,
        )?;
        assert_eq!(recovered.candidate().id().as_str(), "candidate-a");
        assert_eq!(recovered.lease().credential_id().as_str(), "credential-a");
        Ok(())
    }

    #[test]
    fn quota_recovery_selection_admits_only_a_due_binding() -> TestResult {
        let (scheduler, route_id) = scheduler(
            vec![("candidate-a", "endpoint-a", 0, 1)],
            vec![(
                "endpoint-a",
                vec![("credential-a", 0, 1, 1), ("credential-b", 0, 1, 1)],
            )],
        )?;
        let clock = Arc::new(FixedRuntimeHealthClock::new(100));
        let runtime_health = RuntimeHealthRegistry::with_clock(clock.clone());
        let runtime_quota = RuntimeQuotaRegistry::with_clock(clock.clone());
        let exhausted = RuntimeQuotaTarget::endpoint_credential(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new("credential-a")?,
        );
        runtime_quota.record_rate_limited(
            exhausted.clone(),
            100,
            Some(Duration::from_millis(100)),
            Duration::from_millis(100),
        )?;

        let not_due = scheduler.select_eligible_and_lease_for_quota_recovery(
            &route_id,
            &runtime_health,
            &runtime_quota,
            |_| true,
            |_, _| true,
        );
        assert!(not_due.is_err());

        clock.set_now_ms(200);
        let due = scheduler.select_eligible_and_lease_for_quota_recovery(
            &route_id,
            &runtime_health,
            &runtime_quota,
            |_| true,
            |_, _| true,
        )?;
        assert_eq!(due.lease().credential_id().as_str(), "credential-a");
        drop(due);

        let ticket = runtime_quota
            .begin_recovery_probe(&exhausted, 260)?
            .ok_or("due quota did not issue a controlled recovery ticket")?;
        let in_flight = scheduler.select_eligible_and_lease_for_quota_recovery(
            &route_id,
            &runtime_health,
            &runtime_quota,
            |_| true,
            |_, _| true,
        );
        assert!(in_flight.is_err());
        drop(ticket);

        clock.set_now_ms(260);
        runtime_quota.record_snapshot(QuotaSnapshot::try_new(
            RuntimeQuotaTarget::endpoint_credential_model(
                EndpointId::try_new("endpoint-a")?,
                CredentialId::try_new("credential-a")?,
                "upstream-model",
            )?,
            vec![QuotaWindow::try_new(
                "requests",
                Some(10),
                Some(0),
                Some(400),
            )?],
            QuotaSource::Header,
            QuotaConfidence::Observed,
            260,
        )?)?;
        let model_blocked = scheduler.select_eligible_and_lease_for_quota_recovery(
            &route_id,
            &runtime_health,
            &runtime_quota,
            |_| true,
            |_, _| true,
        );
        assert!(model_blocked.is_err());
        Ok(())
    }

    #[test]
    fn unavailable_runtime_health_clock_fails_closed_before_pool_lease() -> TestResult {
        let (scheduler, route_id) = scheduler(
            vec![("candidate-a", "endpoint-a", 0, 1)],
            vec![("endpoint-a", vec![("credential-a", 0, 1, 1)])],
        )?;
        let runtime_health =
            RuntimeHealthRegistry::with_clock(Arc::new(UnavailableRuntimeHealthClock));

        let Err(error) = scheduler.select_runtime_eligible_and_lease(&route_id, &runtime_health)
        else {
            return Err("unavailable runtime-health clock unexpectedly selected a lease".into());
        };
        assert_eq!(error.code(), GatewayErrorCode::CredentialUnavailable);
        assert_eq!(error.scope(), ErrorScope::Credential);
        Ok(())
    }

    fn scheduler<'a>(
        candidate_specs: Vec<CandidateSpec<'a>>,
        pool_specs: Vec<PoolSpec<'a>>,
    ) -> Result<(RouteCredentialScheduler, RouteId), Box<dyn Error>> {
        let route_id = RouteId::try_new("route-a")?;
        let public_model_id = PublicModelId::try_new("public-model-a")?;
        let candidates = candidate_specs
            .into_iter()
            .map(|(candidate_id, endpoint_id, priority, weight)| {
                candidate(candidate_id, endpoint_id, priority, weight)
            })
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
        Ok((RouteCredentialScheduler::new(snapshot, pools), route_id))
    }

    fn candidate(
        candidate_id: &str,
        endpoint_id: &str,
        priority: i64,
        weight: i64,
    ) -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
        Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(candidate_id)?,
            endpoint_id: EndpointId::try_new(endpoint_id)?,
            upstream_id: UpstreamId::try_new(format!("upstream-{endpoint_id}"))?,
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: "upstream-model".to_owned(),
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
        entries: Vec<(&str, i64, i64, i64)>,
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

    fn merge_counts(target: &mut BTreeMap<String, usize>, source: BTreeMap<String, usize>) {
        for (key, value) in source {
            *target.entry(key).or_default() += value;
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

        fn set_now_ms(&self, now_ms: i64) {
            self.now_ms.store(now_ms, Ordering::Release);
        }
    }

    impl RuntimeHealthClock for FixedRuntimeHealthClock {
        fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
            Ok(self.now_ms.load(Ordering::Acquire))
        }
    }

    #[derive(Debug)]
    struct UnavailableRuntimeHealthClock;

    impl RuntimeHealthClock for UnavailableRuntimeHealthClock {
        fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
            Err(RuntimeHealthClockError::Unavailable)
        }
    }
}
