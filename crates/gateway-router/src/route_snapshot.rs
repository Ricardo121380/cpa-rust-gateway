//! Immutable router-safe route configuration and atomic publication primitives.
//!
//! The data plane loads one [`RouteSnapshot`] `Arc` per request. Publication is serialized only
//! on the management path; it never adds a lock or persistence read to a route lookup.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

use arc_swap::ArcSwap;
use gateway_catalog::{CapabilitySet, CatalogModelState};
use gateway_core::{
    AccessGroupId, EndpointId, InvalidIdentifier, PublicModelId, RouteCandidateId, RouteId,
    UpstreamId,
};

/// Opaque Config Version identity carried by an immutable runtime Snapshot.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SnapshotVersion(String);

impl SnapshotVersion {
    /// Creates a non-empty Snapshot Version identity.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentifier::Empty`] when `value` is empty.
    pub fn try_new(value: impl Into<String>) -> Result<Self, InvalidIdentifier> {
        let value = value.into();
        if value.is_empty() {
            return Err(InvalidIdentifier::Empty);
        }
        Ok(Self(value))
    }

    /// Returns the exact persisted Config Version representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SnapshotVersion {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SnapshotVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Router-safe form of the persisted Route scheduling policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotRoutePolicy {
    /// Rotate eligible Candidates in ordinary order.
    RoundRobin,
    /// Use a later smooth weighted rotation inside a priority tier.
    SmoothWeightedRoundRobin,
    /// Prefer lower-numbered priority tiers before failover.
    PriorityFailover,
}

/// Router-safe form of a Candidate request conversion mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotTransformMode {
    /// A later route may forward a compatible request without canonical conversion.
    Passthrough,
    /// A later route uses the gateway Canonical representation.
    Canonical,
    /// A later route may use a proven lossless compatibility bridge.
    LosslessBridge,
}

/// Catalog reason that made a Candidate hard-eligible at compilation time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotCatalogAdmission {
    /// A manual, fresh, or stale Catalog record explicitly listed the upstream model.
    Listed(CatalogModelState),
    /// The Candidate used the explicit management-time unlisted-model exception.
    AllowedUnlisted,
}

/// One active public model retained in a runtime Snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotPublicModel {
    id: PublicModelId,
    model_name: String,
    display_name: String,
    required_capabilities: CapabilitySet,
    route_id: RouteId,
}

impl SnapshotPublicModel {
    /// Creates one compiler-approved public-model view.
    #[must_use]
    pub fn new(
        id: PublicModelId,
        model_name: String,
        display_name: String,
        required_capabilities: CapabilitySet,
        route_id: RouteId,
    ) -> Self {
        Self {
            id,
            model_name,
            display_name,
            required_capabilities,
            route_id,
        }
    }

    /// Returns the stable public-model identity.
    #[must_use]
    pub fn id(&self) -> &PublicModelId {
        &self.id
    }

    /// Returns the exact client-visible model name.
    #[must_use]
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Returns the non-secret display label.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the semantic capabilities promised by the public model.
    #[must_use]
    pub fn required_capabilities(&self) -> &CapabilitySet {
        &self.required_capabilities
    }

    /// Returns the active Route identity serving this model.
    #[must_use]
    pub fn route_id(&self) -> &RouteId {
        &self.route_id
    }
}

/// One hard-eligible route Candidate retained without credential material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotRouteCandidate {
    id: RouteCandidateId,
    endpoint_id: EndpointId,
    upstream_id: UpstreamId,
    upstream_model: String,
    transform_mode: SnapshotTransformMode,
    priority: i64,
    weight: i64,
    effective_capabilities: CapabilitySet,
    catalog_admission: SnapshotCatalogAdmission,
    active_binding_count: usize,
}

/// Complete compiler-approved input for one credential-free route Candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotRouteCandidateInput {
    /// Stable Candidate identity.
    pub id: RouteCandidateId,
    /// Selected Endpoint identity.
    pub endpoint_id: EndpointId,
    /// Selected Upstream identity.
    pub upstream_id: UpstreamId,
    /// Exact non-secret model label sent to the upstream.
    pub upstream_model: String,
    /// Later request conversion mode.
    pub transform_mode: SnapshotTransformMode,
    /// Lower-is-better scheduling priority tier.
    pub priority: i64,
    /// Positive scheduling weight.
    pub weight: i64,
    /// Compiler-approved effective Endpoint capability profile.
    pub effective_capabilities: CapabilitySet,
    /// Management-time Catalog admission reason.
    pub catalog_admission: SnapshotCatalogAdmission,
    /// Count of active bindings, without Credential identities or secrets.
    pub active_binding_count: usize,
}

impl SnapshotRouteCandidate {
    /// Creates one compiler-approved, credential-free Candidate view.
    #[must_use]
    pub fn new(input: SnapshotRouteCandidateInput) -> Self {
        Self {
            id: input.id,
            endpoint_id: input.endpoint_id,
            upstream_id: input.upstream_id,
            upstream_model: input.upstream_model,
            transform_mode: input.transform_mode,
            priority: input.priority,
            weight: input.weight,
            effective_capabilities: input.effective_capabilities,
            catalog_admission: input.catalog_admission,
            active_binding_count: input.active_binding_count,
        }
    }

    /// Returns the stable Candidate identity.
    #[must_use]
    pub fn id(&self) -> &RouteCandidateId {
        &self.id
    }

    /// Returns the selected Endpoint identity.
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

    /// Returns the later request conversion mode.
    #[must_use]
    pub const fn transform_mode(&self) -> SnapshotTransformMode {
        self.transform_mode
    }

    /// Returns the lower-is-better priority tier.
    #[must_use]
    pub const fn priority(&self) -> i64 {
        self.priority
    }

    /// Returns the positive scheduling weight.
    #[must_use]
    pub const fn weight(&self) -> i64 {
        self.weight
    }

    /// Returns the endpoint profile after compiler-approved Candidate narrowing.
    #[must_use]
    pub fn effective_capabilities(&self) -> &CapabilitySet {
        &self.effective_capabilities
    }

    /// Returns the Catalog admission reason.
    #[must_use]
    pub const fn catalog_admission(&self) -> SnapshotCatalogAdmission {
        self.catalog_admission
    }

    /// Returns only the number of active bindings, never Credential identities or secrets.
    #[must_use]
    pub const fn active_binding_count(&self) -> usize {
        self.active_binding_count
    }
}

/// One active Route with deterministically ordered hard-eligible Candidates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotRoute {
    id: RouteId,
    public_model_id: PublicModelId,
    policy: SnapshotRoutePolicy,
    max_attempts: i64,
    bootstrap_timeout_ms: i64,
    candidates: Vec<SnapshotRouteCandidate>,
}

impl SnapshotRoute {
    /// Creates one compiler-approved Route view.
    #[must_use]
    pub fn new(
        id: RouteId,
        public_model_id: PublicModelId,
        policy: SnapshotRoutePolicy,
        max_attempts: i64,
        bootstrap_timeout_ms: i64,
        candidates: Vec<SnapshotRouteCandidate>,
    ) -> Self {
        Self {
            id,
            public_model_id,
            policy,
            max_attempts,
            bootstrap_timeout_ms,
            candidates,
        }
    }

    /// Returns the stable Route identity.
    #[must_use]
    pub fn id(&self) -> &RouteId {
        &self.id
    }

    /// Returns the associated public-model identity.
    #[must_use]
    pub fn public_model_id(&self) -> &PublicModelId {
        &self.public_model_id
    }

    /// Returns the later runtime scheduling policy.
    #[must_use]
    pub const fn policy(&self) -> SnapshotRoutePolicy {
        self.policy
    }

    /// Returns the configured total-attempt bound.
    #[must_use]
    pub const fn max_attempts(&self) -> i64 {
        self.max_attempts
    }

    /// Returns the bootstrap timeout in milliseconds.
    #[must_use]
    pub const fn bootstrap_timeout_ms(&self) -> i64 {
        self.bootstrap_timeout_ms
    }

    /// Returns hard-eligible Candidates in deterministic order.
    #[must_use]
    pub fn candidates(&self) -> &[SnapshotRouteCandidate] {
        &self.candidates
    }
}

/// One active Access Group permission view in a runtime Snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotAccessGroup {
    id: AccessGroupId,
    name: String,
    allowed_route_ids: BTreeSet<RouteId>,
}

impl SnapshotAccessGroup {
    /// Creates one compiler-approved Access Group view.
    #[must_use]
    pub fn new(id: AccessGroupId, name: String, allowed_route_ids: BTreeSet<RouteId>) -> Self {
        Self {
            id,
            name,
            allowed_route_ids,
        }
    }

    /// Returns the stable Access Group identity.
    #[must_use]
    pub fn id(&self) -> &AccessGroupId {
        &self.id
    }

    /// Returns the non-secret group name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether this group permits one active Route.
    #[must_use]
    pub fn permits_route(&self, route_id: &RouteId) -> bool {
        self.allowed_route_ids.contains(route_id)
    }

    /// Iterates allowed Route identities in stable order.
    pub fn allowed_route_ids(&self) -> impl Iterator<Item = &RouteId> {
        self.allowed_route_ids.iter()
    }
}

/// Complete secret-free input used to construct one immutable runtime Snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteSnapshotInput {
    version: SnapshotVersion,
    public_models: Vec<SnapshotPublicModel>,
    aliases: Vec<(String, PublicModelId)>,
    routes: Vec<SnapshotRoute>,
    access_groups: Vec<SnapshotAccessGroup>,
}

impl RouteSnapshotInput {
    /// Creates one complete secret-free Snapshot input.
    #[must_use]
    pub fn new(
        version: SnapshotVersion,
        public_models: Vec<SnapshotPublicModel>,
        aliases: Vec<(String, PublicModelId)>,
        routes: Vec<SnapshotRoute>,
        access_groups: Vec<SnapshotAccessGroup>,
    ) -> Self {
        Self {
            version,
            public_models,
            aliases,
            routes,
            access_groups,
        }
    }
}

/// Immutable runtime route view pinned by an `Arc` for the lifetime of one request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteSnapshot {
    version: SnapshotVersion,
    public_models: BTreeMap<String, SnapshotPublicModel>,
    public_model_names_by_id: BTreeMap<PublicModelId, String>,
    aliases: BTreeMap<String, PublicModelId>,
    routes: BTreeMap<RouteId, SnapshotRoute>,
    access_groups: BTreeMap<AccessGroupId, SnapshotAccessGroup>,
}

impl RouteSnapshot {
    /// Builds and validates an immutable, router-safe Snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`RouteSnapshotBuildError`] when a transfer input has duplicate names/identities or
    /// inconsistent public-model, Alias, Route, Candidate, or Access Group references.
    pub fn try_new(input: RouteSnapshotInput) -> Result<Self, RouteSnapshotBuildError> {
        let mut public_models = BTreeMap::new();
        let mut public_model_names_by_id = BTreeMap::new();
        for public_model in input.public_models {
            if public_models
                .insert(public_model.model_name.clone(), public_model.clone())
                .is_some()
            {
                return Err(RouteSnapshotBuildError::DuplicatePublicModelName);
            }
            if public_model_names_by_id
                .insert(public_model.id.clone(), public_model.model_name.clone())
                .is_some()
            {
                return Err(RouteSnapshotBuildError::DuplicatePublicModelId);
            }
        }

        let mut routes = BTreeMap::new();
        let mut candidate_ids = BTreeSet::new();
        for route in input.routes {
            if routes.insert(route.id.clone(), route.clone()).is_some() {
                return Err(RouteSnapshotBuildError::DuplicateRoute);
            }
            if !public_model_names_by_id.contains_key(&route.public_model_id) {
                return Err(RouteSnapshotBuildError::UnknownRoutePublicModel);
            }
            if route.candidates.is_empty() {
                return Err(RouteSnapshotBuildError::RouteHasNoCandidates);
            }
            for candidate in &route.candidates {
                if !candidate_ids.insert(candidate.id.clone()) {
                    return Err(RouteSnapshotBuildError::DuplicateCandidate);
                }
            }
        }

        for public_model in public_models.values() {
            let route = routes
                .get(&public_model.route_id)
                .ok_or(RouteSnapshotBuildError::PublicModelMissingRoute)?;
            if route.public_model_id != public_model.id {
                return Err(RouteSnapshotBuildError::PublicModelRouteMismatch);
            }
        }

        let mut aliases = BTreeMap::new();
        for (alias, public_model_id) in input.aliases {
            if public_models.contains_key(&alias) {
                return Err(RouteSnapshotBuildError::AliasConflictsPublicModel);
            }
            if !public_model_names_by_id.contains_key(&public_model_id) {
                return Err(RouteSnapshotBuildError::UnknownAliasPublicModel);
            }
            if aliases.insert(alias, public_model_id).is_some() {
                return Err(RouteSnapshotBuildError::DuplicateAlias);
            }
        }

        let mut access_groups = BTreeMap::new();
        for access_group in input.access_groups {
            if access_groups
                .insert(access_group.id.clone(), access_group.clone())
                .is_some()
            {
                return Err(RouteSnapshotBuildError::DuplicateAccessGroup);
            }
            if access_group
                .allowed_route_ids
                .iter()
                .any(|route_id| !routes.contains_key(route_id))
            {
                return Err(RouteSnapshotBuildError::AccessGroupReferencesUnknownRoute);
            }
        }

        Ok(Self {
            version: input.version,
            public_models,
            public_model_names_by_id,
            aliases,
            routes,
            access_groups,
        })
    }

    /// Returns the Config Version pinned by this Snapshot.
    #[must_use]
    pub fn version(&self) -> &SnapshotVersion {
        &self.version
    }

    /// Resolves an exact public model name or exact Alias to its public-model view.
    #[must_use]
    pub fn resolve_public_model(&self, model_name_or_alias: &str) -> Option<&SnapshotPublicModel> {
        self.public_models.get(model_name_or_alias).or_else(|| {
            let public_model_id = self.aliases.get(model_name_or_alias)?;
            let public_model_name = self.public_model_names_by_id.get(public_model_id)?;
            self.public_models.get(public_model_name)
        })
    }

    /// Returns one exact active public model by client-visible name.
    #[must_use]
    pub fn public_model(&self, model_name: &str) -> Option<&SnapshotPublicModel> {
        self.public_models.get(model_name)
    }

    /// Iterates public models in stable name order.
    pub fn public_models(&self) -> impl Iterator<Item = &SnapshotPublicModel> {
        self.public_models.values()
    }

    /// Returns an exact Alias target identity.
    #[must_use]
    pub fn alias_target(&self, alias: &str) -> Option<&PublicModelId> {
        self.aliases.get(alias)
    }

    /// Returns one active Route by identity.
    #[must_use]
    pub fn route(&self, route_id: &RouteId) -> Option<&SnapshotRoute> {
        self.routes.get(route_id)
    }

    /// Iterates Routes in stable identifier order.
    pub fn routes(&self) -> impl Iterator<Item = &SnapshotRoute> {
        self.routes.values()
    }

    /// Returns one active Access Group view.
    #[must_use]
    pub fn access_group(&self, access_group_id: &AccessGroupId) -> Option<&SnapshotAccessGroup> {
        self.access_groups.get(access_group_id)
    }

    /// Iterates active Access Groups in stable identifier order.
    pub fn access_groups(&self) -> impl Iterator<Item = &SnapshotAccessGroup> {
        self.access_groups.values()
    }
}

/// Safe construction failures for a runtime Snapshot input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteSnapshotBuildError {
    /// More than one public model used the same client-visible name.
    DuplicatePublicModelName,
    /// More than one public model used the same stable identity.
    DuplicatePublicModelId,
    /// More than one Route used the same stable identity.
    DuplicateRoute,
    /// A Route referred to a missing public model.
    UnknownRoutePublicModel,
    /// A public model referred to a missing Route.
    PublicModelMissingRoute,
    /// A public model and its Route referred to different public-model identities.
    PublicModelRouteMismatch,
    /// A Route had no hard-eligible Candidates.
    RouteHasNoCandidates,
    /// More than one Route Candidate used the same stable identity.
    DuplicateCandidate,
    /// An Alias duplicated an active public-model name.
    AliasConflictsPublicModel,
    /// More than one Alias used the same exact text.
    DuplicateAlias,
    /// An Alias referred to a missing public model.
    UnknownAliasPublicModel,
    /// More than one Access Group used the same stable identity.
    DuplicateAccessGroup,
    /// An Access Group permitted a Route absent from the Snapshot.
    AccessGroupReferencesUnknownRoute,
}

impl fmt::Display for RouteSnapshotBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = match self {
            Self::DuplicatePublicModelName => "Snapshot has a duplicate public model name",
            Self::DuplicatePublicModelId => "Snapshot has a duplicate public model identity",
            Self::DuplicateRoute => "Snapshot has a duplicate Route identity",
            Self::UnknownRoutePublicModel => "Snapshot Route refers to an unknown public model",
            Self::PublicModelMissingRoute => "Snapshot public model refers to an unknown Route",
            Self::PublicModelRouteMismatch => {
                "Snapshot public model and Route identities do not agree"
            }
            Self::RouteHasNoCandidates => "Snapshot Route has no hard-eligible Candidates",
            Self::DuplicateCandidate => "Snapshot has a duplicate Candidate identity",
            Self::AliasConflictsPublicModel => "Snapshot Alias conflicts with a public model name",
            Self::DuplicateAlias => "Snapshot has a duplicate Alias",
            Self::UnknownAliasPublicModel => "Snapshot Alias refers to an unknown public model",
            Self::DuplicateAccessGroup => "Snapshot has a duplicate Access Group identity",
            Self::AccessGroupReferencesUnknownRoute => {
                "Snapshot Access Group refers to an unknown Route"
            }
        };
        formatter.write_str(description)
    }
}

impl Error for RouteSnapshotBuildError {}

/// Version metadata describing one completed in-memory Snapshot transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotTransition {
    previous_version: SnapshotVersion,
    current_version: SnapshotVersion,
}

impl SnapshotTransition {
    fn new(previous_version: SnapshotVersion, current_version: SnapshotVersion) -> Self {
        Self {
            previous_version,
            current_version,
        }
    }

    /// Returns the Version that was replaced.
    #[must_use]
    pub fn previous_version(&self) -> &SnapshotVersion {
        &self.previous_version
    }

    /// Returns the Version now visible to newly started requests.
    #[must_use]
    pub fn current_version(&self) -> &SnapshotVersion {
        &self.current_version
    }
}

/// Lock-free reader registry with a control-path-only one-step rollback slot.
pub struct RouteSnapshotRegistry {
    current: ArcSwap<RouteSnapshot>,
    publication_state: Mutex<SnapshotPublicationState>,
}

struct SnapshotPublicationState {
    previous: Option<Arc<RouteSnapshot>>,
}

impl RouteSnapshotRegistry {
    /// Creates a registry with an initial immutable Snapshot.
    #[must_use]
    pub fn new(initial: Arc<RouteSnapshot>) -> Self {
        Self {
            current: ArcSwap::from(initial),
            publication_state: Mutex::new(SnapshotPublicationState { previous: None }),
        }
    }

    /// Loads an owned Snapshot `Arc` for one complete request or stream lifetime.
    #[must_use]
    pub fn load(&self) -> Arc<RouteSnapshot> {
        self.current.load_full()
    }

    /// Reserves a replacement Snapshot while serializing only management publication work.
    ///
    /// The returned reservation can be held while a caller performs a matching durable state
    /// transition. Dropping it without [`PreparedSnapshotPublication::commit`] leaves the current
    /// Snapshot unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`SnapshotRegistryError::SameVersion`] when `next` has the current Version, or a
    /// safe lock error when an earlier publisher panic poisoned the control-path mutex.
    pub fn prepare_publication(
        &self,
        next: Arc<RouteSnapshot>,
    ) -> Result<PreparedSnapshotPublication<'_>, SnapshotRegistryError> {
        let publication_state = self.lock_publication_state()?;
        let previous = self.load();
        if previous.version == next.version {
            return Err(SnapshotRegistryError::SameVersion);
        }
        Ok(PreparedSnapshotPublication {
            registry: self,
            publication_state,
            next,
            previous,
        })
    }

    /// Reserves the immediately previous Snapshot for a one-step rollback.
    ///
    /// # Errors
    ///
    /// Returns [`SnapshotRegistryError::NoRollbackAvailable`] when no prior successful
    /// publication is retained.
    pub fn prepare_rollback(
        &self,
    ) -> Result<PreparedSnapshotPublication<'_>, SnapshotRegistryError> {
        let publication_state = self.lock_publication_state()?;
        let next = publication_state
            .previous
            .clone()
            .ok_or(SnapshotRegistryError::NoRollbackAvailable)?;
        let previous = self.load();
        if previous.version == next.version {
            return Err(SnapshotRegistryError::SameVersion);
        }
        Ok(PreparedSnapshotPublication {
            registry: self,
            publication_state,
            next,
            previous,
        })
    }

    /// Publishes a replacement immediately without an external durable transition.
    ///
    /// Management orchestration that also changes `SQLite` should prefer
    /// [`Self::prepare_publication`] and commit only after that transaction succeeds.
    ///
    /// # Errors
    ///
    /// Returns the same safe reservation errors as [`Self::prepare_publication`].
    pub fn publish(
        &self,
        next: Arc<RouteSnapshot>,
    ) -> Result<SnapshotTransition, SnapshotRegistryError> {
        Ok(self.prepare_publication(next)?.commit())
    }

    /// Rolls back immediately to the retained predecessor without a durable transition.
    ///
    /// Management orchestration that also changes `SQLite` should prefer
    /// [`Self::prepare_rollback`] and commit only after that transaction succeeds.
    ///
    /// # Errors
    ///
    /// Returns the same safe reservation errors as [`Self::prepare_rollback`].
    pub fn rollback(&self) -> Result<SnapshotTransition, SnapshotRegistryError> {
        Ok(self.prepare_rollback()?.commit())
    }

    fn lock_publication_state(
        &self,
    ) -> Result<MutexGuard<'_, SnapshotPublicationState>, SnapshotRegistryError> {
        self.publication_state
            .lock()
            .map_err(|_| SnapshotRegistryError::PublicationLockPoisoned)
    }
}

impl fmt::Debug for RouteSnapshotRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteSnapshotRegistry")
            .field("current_version", self.load().version())
            .finish_non_exhaustive()
    }
}

/// A control-path reservation whose commit makes a prevalidated Snapshot visible atomically.
pub struct PreparedSnapshotPublication<'registry> {
    registry: &'registry RouteSnapshotRegistry,
    publication_state: MutexGuard<'registry, SnapshotPublicationState>,
    next: Arc<RouteSnapshot>,
    previous: Arc<RouteSnapshot>,
}

impl PreparedSnapshotPublication<'_> {
    /// Returns the Version that will become current if this reservation is committed.
    #[must_use]
    pub fn target_version(&self) -> &SnapshotVersion {
        self.next.version()
    }

    /// Stores the already allocated Snapshot and retains the former current value for rollback.
    ///
    /// This method performs no fallible allocation, validation, or locking. Callers use it only
    /// after their matching durable Config Version transition has committed.
    #[must_use]
    pub fn commit(mut self) -> SnapshotTransition {
        let transition =
            SnapshotTransition::new(self.previous.version.clone(), self.next.version.clone());
        self.registry.current.store(Arc::clone(&self.next));
        self.publication_state.previous = Some(Arc::clone(&self.previous));
        transition
    }
}

/// Safe errors returned while reserving or publishing Snapshot transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotRegistryError {
    /// A replacement used the same Version already visible to readers.
    SameVersion,
    /// No immediately previous Snapshot exists for a one-step rollback.
    NoRollbackAvailable,
    /// A prior panic poisoned the control-path publication mutex.
    PublicationLockPoisoned,
}

impl fmt::Display for SnapshotRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = match self {
            Self::SameVersion => "Snapshot replacement must use a new Config Version",
            Self::NoRollbackAvailable => "no previous Snapshot is available for rollback",
            Self::PublicationLockPoisoned => "Snapshot publication lock is unavailable",
        };
        formatter.write_str(description)
    }
}

impl Error for SnapshotRegistryError {}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        error::Error,
        io,
        sync::{Arc, Barrier},
        thread,
    };

    use gateway_catalog::{CapabilitySet, CatalogModelState};

    use super::{
        RouteSnapshot, RouteSnapshotBuildError, RouteSnapshotInput, RouteSnapshotRegistry,
        SnapshotAccessGroup, SnapshotCatalogAdmission, SnapshotPublicModel, SnapshotRegistryError,
        SnapshotRoute, SnapshotRouteCandidate, SnapshotRouteCandidateInput, SnapshotRoutePolicy,
        SnapshotTransformMode, SnapshotVersion,
    };
    use gateway_core::{
        AccessGroupId, EndpointId, PublicModelId, RouteCandidateId, RouteId, UpstreamId,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn resolves_models_aliases_routes_and_access_groups_from_one_complete_snapshot() -> TestResult {
        let snapshot = sample_snapshot("version-a")?;

        let resolved = snapshot
            .resolve_public_model("model-alias")
            .ok_or_else(|| io::Error::other("expected alias resolution"))?;
        assert_eq!(resolved.model_name(), "public-model");
        assert_eq!(resolved.route_id().as_str(), "route-a");
        assert!(
            resolved
                .required_capabilities()
                .supports_all(&CapabilitySet::empty())
        );

        let route = snapshot
            .route(resolved.route_id())
            .ok_or_else(|| io::Error::other("expected resolved Route"))?;
        assert_eq!(route.candidates().len(), 1);
        assert_eq!(route.candidates()[0].endpoint_id().as_str(), "endpoint-a");
        assert_eq!(
            route.candidates()[0].catalog_admission(),
            SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh)
        );

        let access_group_id = AccessGroupId::try_new("group-a")?;
        let access_group = snapshot
            .access_group(&access_group_id)
            .ok_or_else(|| io::Error::other("expected access group"))?;
        assert!(access_group.permits_route(route.id()));
        assert_eq!(snapshot.version().as_str(), "version-a");
        Ok(())
    }

    #[test]
    fn rejects_a_public_model_without_its_referenced_route() -> TestResult {
        let public_model_id = PublicModelId::try_new("model-a")?;
        let missing_route_id = RouteId::try_new("route-a")?;
        let snapshot = RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new("version-a")?,
            vec![SnapshotPublicModel::new(
                public_model_id,
                "public-model".to_owned(),
                "Public Model".to_owned(),
                CapabilitySet::empty(),
                missing_route_id,
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ));

        assert!(matches!(
            snapshot,
            Err(RouteSnapshotBuildError::PublicModelMissingRoute)
        ));
        Ok(())
    }

    #[test]
    fn one_hundred_readers_retain_the_snapshot_loaded_before_publication() -> TestResult {
        let registry = Arc::new(RouteSnapshotRegistry::new(sample_snapshot("version-a")?));
        let ready_gate = Arc::new(Barrier::new(101));
        let release_gate = Arc::new(Barrier::new(101));
        let mut readers = Vec::new();

        for _ in 0..100 {
            let reader_registry = Arc::clone(&registry);
            let ready_gate_for_thread = Arc::clone(&ready_gate);
            let release_gate_for_thread = Arc::clone(&release_gate);
            readers.push(thread::spawn(move || {
                let held_snapshot = reader_registry.load();
                ready_gate_for_thread.wait();
                release_gate_for_thread.wait();
                held_snapshot
            }));
        }

        ready_gate.wait();
        let publication = registry.publish(sample_snapshot("version-b")?);
        release_gate.wait();
        let publication = publication?;
        assert_eq!(publication.previous_version().as_str(), "version-a");
        assert_eq!(publication.current_version().as_str(), "version-b");

        for reader in readers {
            let held_snapshot = reader
                .join()
                .map_err(|_| io::Error::other("concurrent snapshot reader panicked"))?;
            assert_eq!(held_snapshot.version().as_str(), "version-a");
        }
        assert_eq!(registry.load().version().as_str(), "version-b");
        Ok(())
    }

    #[test]
    fn publication_and_rollback_toggle_current_and_predecessor_snapshots() -> TestResult {
        let registry = RouteSnapshotRegistry::new(sample_snapshot("version-a")?);

        let publication = registry.publish(sample_snapshot("version-b")?)?;
        assert_eq!(publication.previous_version().as_str(), "version-a");
        assert_eq!(publication.current_version().as_str(), "version-b");

        let rollback = registry.rollback()?;
        assert_eq!(rollback.previous_version().as_str(), "version-b");
        assert_eq!(rollback.current_version().as_str(), "version-a");
        assert_eq!(registry.load().version().as_str(), "version-a");

        let forward = registry.rollback()?;
        assert_eq!(forward.previous_version().as_str(), "version-a");
        assert_eq!(forward.current_version().as_str(), "version-b");
        Ok(())
    }

    #[test]
    fn rollback_without_a_predecessor_is_safe_and_keeps_the_current_snapshot() -> TestResult {
        let registry = RouteSnapshotRegistry::new(sample_snapshot("version-a")?);

        assert!(matches!(
            registry.rollback(),
            Err(SnapshotRegistryError::NoRollbackAvailable)
        ));
        assert_eq!(registry.load().version().as_str(), "version-a");
        Ok(())
    }

    fn sample_snapshot(version: &str) -> Result<Arc<RouteSnapshot>, Box<dyn Error>> {
        let public_model_id = PublicModelId::try_new("public-model-a")?;
        let route_id = RouteId::try_new("route-a")?;
        let candidate = SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new("candidate-a")?,
            endpoint_id: EndpointId::try_new("endpoint-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            upstream_model: "upstream-model".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
            active_binding_count: 1,
        });
        let snapshot = RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new(version.to_owned())?,
            vec![SnapshotPublicModel::new(
                public_model_id.clone(),
                "public-model".to_owned(),
                "Public Model".to_owned(),
                CapabilitySet::empty(),
                route_id.clone(),
            )],
            vec![("model-alias".to_owned(), public_model_id.clone())],
            vec![SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                SnapshotRoutePolicy::SmoothWeightedRoundRobin,
                2,
                10_000,
                vec![candidate],
            )],
            vec![SnapshotAccessGroup::new(
                AccessGroupId::try_new("group-a")?,
                "Default".to_owned(),
                BTreeSet::from([route_id]),
            )],
        ))?;
        Ok(Arc::new(snapshot))
    }
}
