//! Lock-free selection over immutable, precompiled Route schedules.
//!
//! This module owns only Candidate ordering. It deliberately has no Credential lease, health,
//! cooldown, circuit, retry, attempt, or transport behavior; later P3 tasks supply an eligibility
//! predicate around this narrow scheduling primitive.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use gateway_core::RouteId;

use crate::{RouteSnapshot, SnapshotRoute, SnapshotRouteCandidate};

/// A process-local, lock-free cursor set for one immutable [`RouteSnapshot`].
///
/// Construct this object once on a Snapshot publication boundary. Every selection uses only an
/// atomic fetch-add and bounded reads from the precompiled tier schedule; it never queries a Store
/// or takes a global scheduling lock.
#[derive(Debug)]
pub struct RouteCandidateScheduler {
    snapshot: Arc<RouteSnapshot>,
    cursors: BTreeMap<RouteId, Vec<AtomicUsize>>,
}

impl RouteCandidateScheduler {
    /// Creates a fresh cursor set for an immutable Snapshot.
    #[must_use]
    pub fn new(snapshot: Arc<RouteSnapshot>) -> Self {
        let cursors = snapshot
            .routes()
            .map(|route| {
                let tier_cursors = snapshot
                    .route_schedule(route.id())
                    .map(|schedule| {
                        schedule
                            .priority_tiers()
                            .map(|_| AtomicUsize::new(0))
                            .collect()
                    })
                    .unwrap_or_default();
                (route.id().clone(), tier_cursors)
            })
            .collect();
        Self { snapshot, cursors }
    }

    /// Returns one immutable Route from the same Snapshot used for selection.
    ///
    /// This is a configuration read only. It does not advance a cursor, query a Store, or expose
    /// any Credential material.
    #[must_use]
    pub fn route(&self, route_id: &RouteId) -> Option<&SnapshotRoute> {
        self.snapshot.route(route_id)
    }

    /// Returns the exact immutable Snapshot used by this scheduler.
    ///
    /// This crate-private diagnostic seam does not expose cursors or mutable scheduling state.
    pub(crate) fn snapshot(&self) -> &RouteSnapshot {
        &self.snapshot
    }

    /// Clones the immutable Snapshot pointer without exposing scheduler cursors.
    pub(crate) fn snapshot_arc(&self) -> Arc<RouteSnapshot> {
        Arc::clone(&self.snapshot)
    }

    /// Selects the next hard-eligible Candidate from the lowest available priority tier.
    #[must_use]
    pub fn select(&self, route_id: &RouteId) -> Option<SnapshotRouteCandidate> {
        self.select_eligible(route_id, |_| true)
    }

    /// Selects the next Candidate accepted by `is_eligible`.
    ///
    /// Each lower priority tier is considered only after every bounded schedule slot in all higher
    /// priority tiers is rejected. The predicate is intentionally supplied by the caller so this
    /// task does not own mutable Credential, health, cooldown, or circuit state.
    #[must_use]
    pub fn select_eligible<F>(
        &self,
        route_id: &RouteId,
        mut is_eligible: F,
    ) -> Option<SnapshotRouteCandidate>
    where
        F: FnMut(&SnapshotRouteCandidate) -> bool,
    {
        let route = self.snapshot.route(route_id)?;
        let schedule = self.snapshot.route_schedule(route_id)?;
        let cursors = self.cursors.get(route_id)?;
        let mut cursor_iter = cursors.iter();

        for tier in schedule.priority_tiers() {
            let cursor = cursor_iter.next()?;
            let slot_indexes = tier.slot_indexes();
            if slot_indexes.is_empty() {
                return None;
            }
            let start = cursor.fetch_add(1, Ordering::Relaxed);
            for offset in 0..slot_indexes.len() {
                let slot_index = start.wrapping_add(offset) % slot_indexes.len();
                let candidate_index = slot_indexes[slot_index];
                let candidate = route.candidates().get(candidate_index)?;
                if is_eligible(candidate) {
                    return Some(candidate.clone());
                }
            }
        }
        None
    }
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
    use gateway_core::{EndpointId, PublicModelId, RouteCandidateId, RouteId, UpstreamId};

    use super::RouteCandidateScheduler;
    use crate::{
        RouteSnapshot, RouteSnapshotInput, SnapshotCatalogAdmission, SnapshotPublicModel,
        SnapshotRoute, SnapshotRouteCandidate, SnapshotRouteCandidateInput, SnapshotRoutePolicy,
        SnapshotTransformMode, SnapshotVersion,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn smooth_weighted_schedule_has_a_stable_distribution() -> TestResult {
        let (scheduler, route_id) = scheduler(
            SnapshotRoutePolicy::SmoothWeightedRoundRobin,
            vec![
                ("candidate-a", 0, 5),
                ("candidate-b", 0, 1),
                ("candidate-c", 0, 1),
            ],
        )?;

        let selected: Vec<_> = (0..7)
            .map(|_| selected_id(&scheduler, &route_id))
            .collect::<Result<_, _>>()?;
        assert_eq!(
            selected,
            vec![
                "candidate-a",
                "candidate-a",
                "candidate-b",
                "candidate-a",
                "candidate-c",
                "candidate-a",
                "candidate-a",
            ]
        );
        Ok(())
    }

    #[test]
    fn round_robin_is_equal_weight_within_one_priority_tier() -> TestResult {
        let (scheduler, route_id) = scheduler(
            SnapshotRoutePolicy::RoundRobin,
            vec![("candidate-a", 0, 100), ("candidate-b", 0, 1)],
        )?;

        let selected: Vec<_> = (0..6)
            .map(|_| selected_id(&scheduler, &route_id))
            .collect::<Result<_, _>>()?;
        assert_eq!(
            selected,
            vec![
                "candidate-a",
                "candidate-b",
                "candidate-a",
                "candidate-b",
                "candidate-a",
                "candidate-b",
            ]
        );
        Ok(())
    }

    #[test]
    fn lower_priority_tier_is_used_only_when_higher_tier_is_ineligible() -> TestResult {
        let (scheduler, route_id) = scheduler(
            SnapshotRoutePolicy::PriorityFailover,
            vec![("candidate-high", 0, 1), ("candidate-low", 1, 1)],
        )?;

        assert_eq!(selected_id(&scheduler, &route_id)?, "candidate-high");
        let fallback = scheduler
            .select_eligible(&route_id, |candidate| {
                candidate.id().as_str() != "candidate-high"
            })
            .ok_or_else(|| io::Error::other("expected lower-tier fallback"))?;
        assert_eq!(fallback.id().as_str(), "candidate-low");
        assert_eq!(selected_id(&scheduler, &route_id)?, "candidate-high");
        Ok(())
    }

    #[test]
    fn atomic_cursors_keep_weighted_distribution_fair_under_concurrency() -> TestResult {
        let (scheduler, route_id) = scheduler(
            SnapshotRoutePolicy::SmoothWeightedRoundRobin,
            vec![
                ("candidate-a", 0, 5),
                ("candidate-b", 0, 1),
                ("candidate-c", 0, 1),
            ],
        )?;
        let scheduler = Arc::new(scheduler);
        let worker_count = 8_usize;
        let selections_per_worker = 70_usize;
        let barrier = Arc::new(Barrier::new(worker_count));
        let mut workers = Vec::new();

        for _ in 0..worker_count {
            let scheduler = Arc::clone(&scheduler);
            let route_id = route_id.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(
                move || -> Result<BTreeMap<String, usize>, String> {
                    barrier.wait();
                    let mut counts = BTreeMap::new();
                    for _ in 0..selections_per_worker {
                        let candidate = scheduler
                            .select(&route_id)
                            .ok_or_else(|| "scheduler selected no candidate".to_owned())?;
                        *counts
                            .entry(candidate.id().as_str().to_owned())
                            .or_default() += 1;
                    }
                    Ok(counts)
                },
            ));
        }

        let mut counts = BTreeMap::new();
        for worker in workers {
            let worker_counts = worker
                .join()
                .map_err(|_| io::Error::other("scheduler worker panicked"))?
                .map_err(io::Error::other)?;
            for (candidate_id, count) in worker_counts {
                *counts.entry(candidate_id).or_default() += count;
            }
        }
        assert_eq!(counts.get("candidate-a"), Some(&400));
        assert_eq!(counts.get("candidate-b"), Some(&80));
        assert_eq!(counts.get("candidate-c"), Some(&80));
        Ok(())
    }

    fn scheduler(
        policy: SnapshotRoutePolicy,
        candidate_specs: Vec<(&str, i64, i64)>,
    ) -> Result<(RouteCandidateScheduler, RouteId), Box<dyn Error>> {
        let route_id = RouteId::try_new("route-a")?;
        let public_model_id = PublicModelId::try_new("public-model-a")?;
        let candidates = candidate_specs
            .into_iter()
            .map(|(id, priority, weight)| candidate(id, priority, weight))
            .collect::<Result<Vec<_>, _>>()?;
        let snapshot = RouteSnapshot::try_new(RouteSnapshotInput::new(
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
        ))?;
        Ok((RouteCandidateScheduler::new(Arc::new(snapshot)), route_id))
    }

    fn candidate(
        id: &str,
        priority: i64,
        weight: i64,
    ) -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
        Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(id)?,
            endpoint_id: EndpointId::try_new(format!("endpoint-{id}"))?,
            upstream_id: UpstreamId::try_new(format!("upstream-{id}"))?,
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

    fn selected_id(
        scheduler: &RouteCandidateScheduler,
        route_id: &RouteId,
    ) -> Result<String, Box<dyn Error>> {
        scheduler
            .select(route_id)
            .map(|candidate| candidate.id().as_str().to_owned())
            .ok_or_else(|| io::Error::other("expected a scheduled Candidate").into())
    }
}
