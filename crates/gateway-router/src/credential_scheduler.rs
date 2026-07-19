//! Two-stage Candidate then Credential lease selection.
//!
//! P3-03 owns immutable Candidate schedule construction and candidate cursors. P3-04 composes
//! that selector with independently scheduled Endpoint Credential pools, without allowing an
//! Endpoint's number of keys to alter Route-level weights. Health, cooldown, circuit, attempt,
//! retry, transport, and Provider behavior remain outside this module.

use std::sync::Arc;

use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode, RouteId};
use gateway_upstream::{CredentialLease, EndpointCredentialPools};

use crate::{RouteCandidateScheduler, RouteSnapshot, SnapshotRouteCandidate};

/// A process-local two-stage scheduler for one immutable Route Snapshot and matching pools.
///
/// It owns no `SQLite` handle, mutable health state, or global lock. Candidate and Credential
/// cursor sets are independent: a Route Candidate is selected first, then its Endpoint pool uses
/// its own atomic cursor to acquire one bounded concurrent lease.
#[derive(Debug)]
pub struct RouteCredentialScheduler {
    candidates: RouteCandidateScheduler,
    credential_pools: Arc<EndpointCredentialPools>,
}

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
        Self {
            candidates: RouteCandidateScheduler::new(snapshot),
            credential_pools,
        }
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
    /// The supplied predicate is a narrow composition point for a later P3-05 health/cooldown
    /// filter. P3-04 only considers its boolean result and never mutates health, circuit, quota,
    /// retry, or attempt state itself.
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

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        error::Error,
        io,
        sync::{Arc, Barrier},
        thread,
    };

    use gateway_catalog::{CapabilitySet, CatalogModelState};
    use gateway_core::{
        CredentialId, EndpointId, ErrorScope, GatewayErrorCode, PublicModelId, RouteCandidateId,
        RouteId, UpstreamId,
    };
    use gateway_upstream::{
        CredentialSecret, EndpointCredentialInput, EndpointCredentialPool, EndpointCredentialPools,
    };

    use super::RouteCredentialScheduler;
    use crate::{
        RouteSnapshot, RouteSnapshotInput, SnapshotCatalogAdmission, SnapshotPublicModel,
        SnapshotRoute, SnapshotRouteCandidate, SnapshotRouteCandidateInput, SnapshotRoutePolicy,
        SnapshotTransformMode, SnapshotVersion,
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
        let (scheduler, route_id) = scheduler(
            vec![
                ("candidate-a", "endpoint-a", 0, 3),
                ("candidate-b", "endpoint-b", 0, 1),
            ],
            vec![
                (
                    "endpoint-a",
                    vec![("credential-a-one", 0, 1, 4), ("credential-a-two", 0, 1, 4)],
                ),
                ("endpoint-b", vec![("credential-b", 0, 1, 4)]),
            ],
        )?;
        let scheduler = Arc::new(scheduler);
        let worker_count = 8_usize;
        let selections_per_worker = 50_usize;
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
}
