//! Immutable P13-05 catalog projection into request-time P13-07 price evidence.
//!
//! This module is the only bridge from durable billing catalog rows to router-owned price-rate
//! vectors. It runs while a runtime composition is built, never while a request is selecting a
//! Provider. Matching is exact by Provider, Channel, and canonical public model; aliases,
//! upstream model labels, request bodies, token estimates, credentials, quota, and Health are not
//! inputs.

use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

use gateway_core::{PublicModelId, RouteCandidateId};
use gateway_router::{ProviderScopedPriceRates, RouteSnapshot, SnapshotVersion};
use gateway_store::{
    billing_ledger::{BillingPriceCatalog, BillingPriceEntry},
    control_plane::{ConfigVersionId, RoutingPriceComparison, RoutingPricePolicyConfiguration},
};

/// Maximum catalog rows admitted to one composition-time price projection.
pub const MAX_ROUTING_PRICE_CATALOG_ENTRIES: usize = 16_384;
/// Maximum Snapshot Candidates admitted for one Route selection projection.
///
/// The limit is per Route because the router selector evaluates one Route at a time. Applying it
/// to the whole Snapshot would reject an otherwise valid multi-Route graph solely because it has
/// many independent small Routes.
pub const MAX_ROUTING_PRICE_CANDIDATES: usize = 4_096;

/// Secret-free immutable rates plus the exact configuration/catalog lineage that produced them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingPriceSnapshot {
    config_version_id: ConfigVersionId,
    snapshot_version: SnapshotVersion,
    catalog_version_id: String,
    catalog_effective_at_ms: u64,
    observed_at_ms: u64,
    comparison: RoutingPriceComparison,
    candidate_price_rates: Arc<BTreeMap<RouteCandidateId, ProviderScopedPriceRates>>,
}

impl RoutingPriceSnapshot {
    /// Returns the exact selected Config Version.
    #[must_use]
    pub fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Returns the independently checked immutable Route Snapshot version.
    #[must_use]
    pub fn snapshot_version(&self) -> &SnapshotVersion {
        &self.snapshot_version
    }

    /// Returns the exact immutable billing catalog version selected by the policy.
    #[must_use]
    pub fn catalog_version_id(&self) -> &str {
        &self.catalog_version_id
    }

    /// Returns the catalog's inclusive effective timestamp.
    #[must_use]
    pub const fn catalog_effective_at_ms(&self) -> u64 {
        self.catalog_effective_at_ms
    }

    /// Returns the composition-time observation used for the effective-time check.
    #[must_use]
    pub const fn observed_at_ms(&self) -> u64 {
        self.observed_at_ms
    }

    /// Returns the closed comparison algorithm selected by the Config Version.
    #[must_use]
    pub const fn comparison(&self) -> RoutingPriceComparison {
        self.comparison
    }

    /// Returns exact known rate vectors keyed by Route Candidate identity.
    ///
    /// Absence is the explicit `Unpriced`/unknown signal; it is never converted to a zero vector.
    #[must_use]
    pub fn candidate_price_rates(&self) -> &BTreeMap<RouteCandidateId, ProviderScopedPriceRates> {
        &self.candidate_price_rates
    }

    /// Clones the immutable map handle for injection into the request-time scheduler.
    #[must_use]
    pub fn candidate_price_rates_arc(
        &self,
    ) -> Arc<BTreeMap<RouteCandidateId, ProviderScopedPriceRates>> {
        Arc::clone(&self.candidate_price_rates)
    }
}

/// Fail-closed composition errors for an immutable routing-price snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutingPriceSnapshotError {
    /// The selected Config Version and immutable Route Snapshot are not the same graph.
    ConfigVersionMismatch,
    /// The policy and supplied immutable catalog do not name the same catalog version.
    CatalogVersionMismatch,
    /// The selected catalog is not yet effective at composition time.
    CatalogNotEffective,
    /// The supplied catalog is malformed or contains duplicate exact tuple rows.
    InvalidCatalog,
    /// The catalog entry count exceeds the finite composition bound.
    CatalogEntryLimitExceeded,
    /// The Snapshot Candidate count exceeds the finite request-time selector bound.
    CandidateLimitExceeded,
    /// A compiler invariant relating a Route to its canonical public model was absent.
    SnapshotPublicModelMissing,
}

impl fmt::Display for RoutingPriceSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigVersionMismatch => {
                formatter.write_str("routing price Config/Snapshot version mismatch")
            }
            Self::CatalogVersionMismatch => {
                formatter.write_str("routing price policy/catalog version mismatch")
            }
            Self::CatalogNotEffective => {
                formatter.write_str("routing price catalog is not yet effective")
            }
            Self::InvalidCatalog => formatter.write_str("routing price catalog is invalid"),
            Self::CatalogEntryLimitExceeded => {
                formatter.write_str("routing price catalog entry limit exceeded")
            }
            Self::CandidateLimitExceeded => {
                formatter.write_str("routing price Candidate limit exceeded")
            }
            Self::SnapshotPublicModelMissing => {
                formatter.write_str("routing price Snapshot public model is missing")
            }
        }
    }
}

impl Error for RoutingPriceSnapshotError {}

/// Compiles one exact Config/Snapshot/catalog binding into immutable router price evidence.
///
/// A catalog row is selected only by exact `(candidate.upstream_id, candidate.endpoint_id,
/// canonical public-model name)`. A missing row is normal and leaves the Candidate absent from
/// the map. This function does not estimate tokens or inspect an upstream-model label.
///
/// # Errors
///
/// Returns [`RoutingPriceSnapshotError`] when lineage, effective time, source shape, or bounded
/// composition invariants are invalid.
pub fn compile_routing_price_snapshot(
    snapshot: &RouteSnapshot,
    config_version_id: &ConfigVersionId,
    policy: &RoutingPricePolicyConfiguration,
    catalog: &BillingPriceCatalog,
    observed_at_ms: u64,
) -> Result<RoutingPriceSnapshot, RoutingPriceSnapshotError> {
    if snapshot.version().as_str() != config_version_id.as_str() {
        return Err(RoutingPriceSnapshotError::ConfigVersionMismatch);
    }
    if policy.catalog_version_id != catalog.catalog_version_id {
        return Err(RoutingPriceSnapshotError::CatalogVersionMismatch);
    }
    if catalog.effective_at_ms > observed_at_ms {
        return Err(RoutingPriceSnapshotError::CatalogNotEffective);
    }
    if catalog.entries.is_empty() {
        return Err(RoutingPriceSnapshotError::InvalidCatalog);
    }
    if catalog.entries.len() > MAX_ROUTING_PRICE_CATALOG_ENTRIES {
        return Err(RoutingPriceSnapshotError::CatalogEntryLimitExceeded);
    }

    let mut entries = BTreeMap::<(&str, &str, &str), &BillingPriceEntry>::new();
    for entry in &catalog.entries {
        if entry.provider_id.trim().is_empty()
            || entry.channel_id.trim().is_empty()
            || entry.model.trim().is_empty()
            || entries
                .insert((&entry.provider_id, &entry.channel_id, &entry.model), entry)
                .is_some()
        {
            return Err(RoutingPriceSnapshotError::InvalidCatalog);
        }
    }

    let public_model_names = snapshot
        .public_models()
        .map(|model| (model.id().clone(), model.model_name()))
        .collect::<BTreeMap<PublicModelId, &str>>();
    let mut candidate_price_rates = BTreeMap::new();
    for route in snapshot.routes() {
        if route.candidates().len() > MAX_ROUTING_PRICE_CANDIDATES {
            return Err(RoutingPriceSnapshotError::CandidateLimitExceeded);
        }
        let public_model_name = public_model_names
            .get(route.public_model_id())
            .ok_or(RoutingPriceSnapshotError::SnapshotPublicModelMissing)?;
        for candidate in route.candidates() {
            let Some(entry) = entries.get(&(
                candidate.upstream_id().as_str(),
                candidate.endpoint_id().as_str(),
                *public_model_name,
            )) else {
                continue;
            };
            candidate_price_rates.insert(candidate.id().clone(), price_rates(entry));
        }
    }

    Ok(RoutingPriceSnapshot {
        config_version_id: config_version_id.clone(),
        snapshot_version: snapshot.version().clone(),
        catalog_version_id: catalog.catalog_version_id.clone(),
        catalog_effective_at_ms: catalog.effective_at_ms,
        observed_at_ms,
        comparison: policy.comparison,
        candidate_price_rates: Arc::new(candidate_price_rates),
    })
}

const fn price_rates(entry: &BillingPriceEntry) -> ProviderScopedPriceRates {
    ProviderScopedPriceRates::new(
        entry.input_microunits_per_million,
        entry.output_microunits_per_million,
        entry.reasoning_microunits_per_million,
        entry.cache_read_microunits_per_million,
        entry.cache_creation_microunits_per_million,
        entry.cached_microunits_per_million,
    )
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, error::Error};

    use gateway_catalog::{CapabilitySet, CatalogModelState};
    use gateway_core::{
        AccessGroupId, EndpointId, PublicModelId, RouteCandidateId, RouteId, UpstreamId,
    };
    use gateway_router::{
        RouteSnapshotInput, SnapshotAccessGroup, SnapshotCatalogAdmission, SnapshotPublicModel,
        SnapshotRoute, SnapshotRouteCandidate, SnapshotRouteCandidateInput, SnapshotRoutePolicy,
        SnapshotTransformMode,
    };
    use gateway_store::billing_ledger::BillingCatalogSource;

    use super::*;

    type TestResult = Result<(), Box<dyn Error>>;

    fn snapshot(version: &str) -> TestResultOf<RouteSnapshot> {
        let public_model_id = PublicModelId::try_new("public-model-id")?;
        let route_id = RouteId::try_new("route-a")?;
        Ok(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new(version)?,
            vec![SnapshotPublicModel::new(
                public_model_id.clone(),
                "public-model".to_owned(),
                "Public Model".to_owned(),
                CapabilitySet::empty(),
                route_id.clone(),
            )],
            vec![("public-alias".to_owned(), public_model_id.clone())],
            vec![SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                SnapshotRoutePolicy::PriorityFailover,
                2,
                1_000,
                vec![SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
                    id: RouteCandidateId::try_new("candidate-a")?,
                    endpoint_id: EndpointId::try_new("channel-a")?,
                    upstream_id: UpstreamId::try_new("provider-a")?,
                    endpoint_api_format: "openai/responses".to_owned(),
                    upstream_model: "upstream-model".to_owned(),
                    transform_mode: SnapshotTransformMode::Canonical,
                    priority: 0,
                    weight: 1,
                    effective_capabilities: CapabilitySet::empty(),
                    catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
                    active_binding_count: 1,
                })],
            )],
            vec![SnapshotAccessGroup::new(
                AccessGroupId::try_new("group-a")?,
                "Group A".to_owned(),
                BTreeSet::from([route_id]),
            )],
            Vec::new(),
        ))?)
    }

    type TestResultOf<T> = Result<T, Box<dyn Error>>;

    fn entry(model: &str) -> BillingPriceEntry {
        BillingPriceEntry {
            provider_id: "provider-a".to_owned(),
            channel_id: "channel-a".to_owned(),
            model: model.to_owned(),
            input_microunits_per_million: 1,
            output_microunits_per_million: 2,
            reasoning_microunits_per_million: 3,
            cache_read_microunits_per_million: 4,
            cache_creation_microunits_per_million: 5,
            cached_microunits_per_million: 6,
        }
    }

    fn catalog(version: &str, effective_at_ms: u64, model: &str) -> BillingPriceCatalog {
        BillingPriceCatalog {
            catalog_version_id: version.to_owned(),
            effective_at_ms,
            source: BillingCatalogSource::Test,
            created_at_ms: 1,
            entries: vec![entry(model)],
        }
    }

    fn policy(version: &str) -> TestResultOf<RoutingPricePolicyConfiguration> {
        Ok(RoutingPricePolicyConfiguration::try_new(
            version,
            RoutingPriceComparison::RateDominanceV1,
        )?)
    }

    #[test]
    fn exact_public_model_match_compiles_all_six_rates() -> TestResult {
        let snapshot = snapshot("config-a")?;
        let compiled = compile_routing_price_snapshot(
            &snapshot,
            &ConfigVersionId::try_new("config-a")?,
            &policy("catalog-a")?,
            &catalog("catalog-a", 100, "public-model"),
            100,
        )?;
        let rates = compiled
            .candidate_price_rates()
            .get(&RouteCandidateId::try_new("candidate-a")?)
            .ok_or("candidate price missing")?;
        assert_eq!(rates.input_microunits_per_million(), 1);
        assert_eq!(rates.output_microunits_per_million(), 2);
        assert_eq!(rates.reasoning_microunits_per_million(), 3);
        assert_eq!(rates.cache_read_microunits_per_million(), 4);
        assert_eq!(rates.cache_creation_microunits_per_million(), 5);
        assert_eq!(rates.cached_microunits_per_million(), 6);
        assert_eq!(compiled.config_version_id().as_str(), "config-a");
        assert_eq!(compiled.snapshot_version().as_str(), "config-a");
        assert_eq!(compiled.catalog_version_id(), "catalog-a");
        assert_eq!(compiled.catalog_effective_at_ms(), 100);
        assert_eq!(compiled.observed_at_ms(), 100);
        Ok(())
    }

    #[test]
    fn upstream_model_and_alias_are_not_price_keys() -> TestResult {
        let snapshot = snapshot("config-a")?;
        for wrong_model in ["upstream-model", "public-alias"] {
            let compiled = compile_routing_price_snapshot(
                &snapshot,
                &ConfigVersionId::try_new("config-a")?,
                &policy("catalog-a")?,
                &catalog("catalog-a", 1, wrong_model),
                1,
            )?;
            assert!(compiled.candidate_price_rates().is_empty());
        }
        Ok(())
    }

    #[test]
    fn missing_tuple_is_unpriced_not_zero() -> TestResult {
        let snapshot = snapshot("config-a")?;
        let mut missing = entry("public-model");
        missing.channel_id = "other-channel".to_owned();
        let catalog = BillingPriceCatalog {
            entries: vec![missing],
            ..catalog("catalog-a", 1, "public-model")
        };
        let compiled = compile_routing_price_snapshot(
            &snapshot,
            &ConfigVersionId::try_new("config-a")?,
            &policy("catalog-a")?,
            &catalog,
            1,
        )?;
        assert!(compiled.candidate_price_rates().is_empty());
        Ok(())
    }

    #[test]
    fn future_catalog_and_lineage_mismatches_fail_closed() -> TestResult {
        let snapshot = snapshot("config-a")?;
        assert_eq!(
            compile_routing_price_snapshot(
                &snapshot,
                &ConfigVersionId::try_new("config-a")?,
                &policy("catalog-a")?,
                &catalog("catalog-a", 101, "public-model"),
                100,
            ),
            Err(RoutingPriceSnapshotError::CatalogNotEffective)
        );
        assert_eq!(
            compile_routing_price_snapshot(
                &snapshot,
                &ConfigVersionId::try_new("config-b")?,
                &policy("catalog-a")?,
                &catalog("catalog-a", 100, "public-model"),
                100,
            ),
            Err(RoutingPriceSnapshotError::ConfigVersionMismatch)
        );
        assert_eq!(
            compile_routing_price_snapshot(
                &snapshot,
                &ConfigVersionId::try_new("config-a")?,
                &policy("catalog-a")?,
                &catalog("rollback-catalog", 100, "public-model"),
                100,
            ),
            Err(RoutingPriceSnapshotError::CatalogVersionMismatch)
        );
        Ok(())
    }

    #[test]
    fn duplicate_exact_catalog_tuple_fails_closed() -> TestResult {
        let snapshot = snapshot("config-a")?;
        let mut catalog = catalog("catalog-a", 1, "public-model");
        catalog.entries.push(entry("public-model"));
        assert_eq!(
            compile_routing_price_snapshot(
                &snapshot,
                &ConfigVersionId::try_new("config-a")?,
                &policy("catalog-a")?,
                &catalog,
                1,
            ),
            Err(RoutingPriceSnapshotError::InvalidCatalog)
        );
        Ok(())
    }
}
