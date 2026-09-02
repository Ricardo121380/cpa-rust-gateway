//! Immutable router-safe route configuration and atomic publication primitives.
//!
//! The data plane loads one [`RouteSnapshot`] `Arc` per request. Publication is serialized only
//! on the management path; it never adds a lock or persistence read to a route lookup.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::{Arc, Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

use arc_swap::ArcSwap;
use gateway_auth::{
    AuthenticatedClient, ClientKeyAuthenticator,
    client_key::{ClientKeyPrefix, ClientKeyRecord, ClientKeyService},
};
use gateway_catalog::{CapabilitySet, CatalogModelState};
use gateway_core::{
    AccessGroupId, ClientKeyId, CredentialId, EndpointId, ErrorScope, GatewayError,
    GatewayErrorCode, InvalidIdentifier, PublicModelId, RouteCandidateId, RouteId, UpstreamId,
};
use sha2::{Digest, Sha256};

use crate::ProtocolFormat;

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
    /// A native protocol uses Canonical semantics while other registered protocols use the
    /// reviewed lossless bridge matrix.
    CanonicalBridge,
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

/// Result of resolving one exact upstream model inside one Access Group's immutable view.
///
/// `Ambiguous` is distinct from `Absent` so a legacy CPAR alias cannot shadow an upstream model
/// that is visible through more than one Route. The Config Version must first define one
/// deterministic route for that exact model identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotExactModelResolution<'snapshot> {
    /// No visible hard-eligible Candidate carries the exact upstream model.
    Absent,
    /// Exactly one visible Public Model Route carries the exact upstream model.
    Unique(&'snapshot SnapshotPublicModel),
    /// More than one visible Public Model Route carries the exact upstream model.
    Ambiguous,
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
    endpoint_api_format: String,
    upstream_model: String,
    transform_mode: SnapshotTransformMode,
    priority: i64,
    weight: i64,
    effective_capabilities: CapabilitySet,
    catalog_admission: SnapshotCatalogAdmission,
    active_binding_count: usize,
    eligible_credential_ids: Option<BTreeSet<gateway_core::CredentialId>>,
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
    /// Exact API format declared by the selected Endpoint.
    pub endpoint_api_format: String,
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
            endpoint_api_format: input.endpoint_api_format,
            upstream_model: input.upstream_model,
            transform_mode: input.transform_mode,
            priority: input.priority,
            weight: input.weight,
            effective_capabilities: input.effective_capabilities,
            catalog_admission: input.catalog_admission,
            active_binding_count: input.active_binding_count,
            eligible_credential_ids: None,
        }
    }

    /// Restricts this immutable Candidate to the exact Credentials whose Catalog listed it.
    ///
    /// Ordinary Config-compiled Candidates remain unrestricted. Discovery-materialized
    /// Candidates use this builder so a model observed on one Credential can never be leased
    /// through a sibling Credential on the same Endpoint.
    #[must_use]
    pub fn with_eligible_credentials(
        mut self,
        credential_ids: BTreeSet<gateway_core::CredentialId>,
    ) -> Self {
        self.active_binding_count = credential_ids.len();
        self.eligible_credential_ids = Some(credential_ids);
        self
    }

    /// Returns whether this Candidate permits the exact Credential binding.
    #[must_use]
    pub fn allows_credential(&self, credential_id: &gateway_core::CredentialId) -> bool {
        self.eligible_credential_ids
            .as_ref()
            .is_none_or(|credential_ids| credential_ids.contains(credential_id))
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

    /// Returns the exact configured API format for the selected Endpoint.
    ///
    /// This remains available in the router-safe Candidate view so an execution path cannot
    /// select a same-Upstream endpoint that speaks a different protocol.
    #[must_use]
    pub fn endpoint_api_format(&self) -> &str {
        &self.endpoint_api_format
    }

    /// Returns this Candidate's P5 protocol when its declared Endpoint format is known.
    ///
    /// Unknown or future API formats are deliberately not coerced to a P5 protocol; callers must
    /// reject them until their owning protocol boundary supplies an explicit mapping.
    #[must_use]
    pub fn protocol_format(&self) -> Option<ProtocolFormat> {
        ProtocolFormat::from_api_format(&self.endpoint_api_format)
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

    /// Returns whether this retained Candidate is hard-eligible for a public model view.
    ///
    /// Runtime lease saturation, 429/Cooldown/Circuit state, and request-local retry exclusions
    /// are intentionally absent: they control a particular attempt, not whether a compiled
    /// Public Model remains discoverable. A direct Snapshot constructor can still supply an
    /// expired Catalog state or zero bindings, so this guard preserves the same public predicate
    /// outside the normal control-plane compiler.
    #[must_use]
    pub const fn is_hard_eligible(&self) -> bool {
        self.active_binding_count > 0
            && match self.catalog_admission {
                SnapshotCatalogAdmission::Listed(state) => state.is_hard_eligible(),
                SnapshotCatalogAdmission::AllowedUnlisted => true,
            }
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

    /// Returns whether this Route retains at least one compiler-hard-eligible Candidate.
    ///
    /// A compiler-approved Candidate has already passed enabled Upstream/Endpoint, Catalog, and
    /// capability checks. A positive binding count is retained in the Snapshot so a public-model
    /// view can also prove that at least one Credential binding existed at publication time
    /// without exposing its identity or consulting mutable runtime availability.
    #[must_use]
    pub fn has_hard_eligible_candidate(&self) -> bool {
        self.candidates
            .iter()
            .any(SnapshotRouteCandidate::is_hard_eligible)
    }
}

/// Upper bound for one immutable priority-tier schedule.
///
/// This keeps Config Version publication and runtime scans bounded even when a persisted weight is
/// malformed or unexpectedly large. The bound is deliberately applied to every policy: ordinary
/// round-robin consumes one slot per Candidate, while smooth weighted round-robin consumes one
/// slot per unit of weight.
pub const MAX_SCHEDULE_SLOTS_PER_PRIORITY_TIER: usize = 1024;

/// Precompiled immutable Candidate schedule for one Route.
///
/// The plan is created while a [`RouteSnapshot`] is built, never while a request is selected. Its
/// priority tiers are ordered from lower (preferred) to higher values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotRouteSchedule {
    priority_tiers: Vec<SnapshotPriorityTierSchedule>,
}

impl SnapshotRouteSchedule {
    fn try_compile(route: &SnapshotRoute) -> Result<Self, RouteSnapshotBuildError> {
        let mut candidates_by_priority: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
        for (candidate_index, candidate) in route.candidates().iter().enumerate() {
            if candidate.priority() < 0 {
                return Err(RouteSnapshotBuildError::InvalidCandidatePriority);
            }
            if candidate.weight() <= 0 {
                return Err(RouteSnapshotBuildError::InvalidCandidateWeight);
            }
            candidates_by_priority
                .entry(candidate.priority())
                .or_default()
                .push(candidate_index);
        }

        let mut priority_tiers = Vec::with_capacity(candidates_by_priority.len());
        for (priority, mut candidate_indexes) in candidates_by_priority {
            candidate_indexes.sort_by(|left, right| {
                route.candidates()[*left]
                    .id()
                    .cmp(route.candidates()[*right].id())
            });
            let slot_indexes = match route.policy() {
                SnapshotRoutePolicy::RoundRobin | SnapshotRoutePolicy::PriorityFailover => {
                    if candidate_indexes.len() > MAX_SCHEDULE_SLOTS_PER_PRIORITY_TIER {
                        return Err(RouteSnapshotBuildError::RouteScheduleTooLarge);
                    }
                    candidate_indexes
                }
                SnapshotRoutePolicy::SmoothWeightedRoundRobin => {
                    smooth_weighted_slots(route, &candidate_indexes)?
                }
            };
            priority_tiers.push(SnapshotPriorityTierSchedule {
                priority,
                slot_indexes,
            });
        }

        Ok(Self { priority_tiers })
    }

    /// Iterates precompiled priority tiers in lower-is-better order.
    pub fn priority_tiers(&self) -> impl Iterator<Item = &SnapshotPriorityTierSchedule> {
        self.priority_tiers.iter()
    }
}

/// One ordered priority tier within a [`SnapshotRouteSchedule`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotPriorityTierSchedule {
    priority: i64,
    slot_indexes: Vec<usize>,
}

impl SnapshotPriorityTierSchedule {
    /// Returns the lower-is-better priority value of this tier.
    #[must_use]
    pub const fn priority(&self) -> i64 {
        self.priority
    }

    pub(crate) fn slot_indexes(&self) -> &[usize] {
        &self.slot_indexes
    }
}

fn smooth_weighted_slots(
    route: &SnapshotRoute,
    candidate_indexes: &[usize],
) -> Result<Vec<usize>, RouteSnapshotBuildError> {
    let mut total_slots = 0_usize;
    let mut weights = Vec::with_capacity(candidate_indexes.len());
    for candidate_index in candidate_indexes {
        let weight = usize::try_from(route.candidates()[*candidate_index].weight())
            .map_err(|_| RouteSnapshotBuildError::RouteScheduleTooLarge)?;
        total_slots = total_slots
            .checked_add(weight)
            .ok_or(RouteSnapshotBuildError::RouteScheduleTooLarge)?;
        if total_slots > MAX_SCHEDULE_SLOTS_PER_PRIORITY_TIER {
            return Err(RouteSnapshotBuildError::RouteScheduleTooLarge);
        }
        weights.push(
            i64::try_from(weight).map_err(|_| RouteSnapshotBuildError::RouteScheduleTooLarge)?,
        );
    }

    let total_weight =
        i64::try_from(total_slots).map_err(|_| RouteSnapshotBuildError::RouteScheduleTooLarge)?;
    let mut current_weights = vec![0_i64; weights.len()];
    let mut slot_indexes = Vec::with_capacity(total_slots);
    for _ in 0..total_slots {
        for (current_weight, weight) in current_weights.iter_mut().zip(&weights) {
            *current_weight += weight;
        }
        let mut selected = 0_usize;
        for candidate_position in 1..current_weights.len() {
            if current_weights[candidate_position] > current_weights[selected] {
                selected = candidate_position;
            }
        }
        current_weights[selected] -= total_weight;
        slot_indexes.push(candidate_indexes[selected]);
    }
    Ok(slot_indexes)
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

/// A redacted Client Key HMAC record and the copied permissions of its active Access Group.
#[derive(Clone, Eq, PartialEq)]
pub struct SnapshotClientKeyView {
    record: ClientKeyRecord,
    allowed_route_ids: BTreeSet<RouteId>,
}

impl SnapshotClientKeyView {
    /// Creates an already validated Client Key view for one active Access Group.
    #[must_use]
    pub fn new(record: ClientKeyRecord, allowed_route_ids: BTreeSet<RouteId>) -> Self {
        Self {
            record,
            allowed_route_ids,
        }
    }

    /// Returns the stable Client Key identity without exposing its complete Key or digest.
    #[must_use]
    pub fn client_key_id(&self) -> &ClientKeyId {
        self.record.client_key_id()
    }

    /// Returns the active Access Group identity resolved for this Key.
    #[must_use]
    pub fn access_group_id(&self) -> &AccessGroupId {
        self.record.access_group_id()
    }

    /// Returns the public Prefix used for one bounded Snapshot lookup.
    #[must_use]
    pub fn prefix(&self) -> &ClientKeyPrefix {
        self.record.prefix()
    }

    /// Returns whether this Key's active Access Group permits one Route.
    #[must_use]
    pub fn permits_route(&self, route_id: &RouteId) -> bool {
        self.allowed_route_ids.contains(route_id)
    }

    /// Iterates the copied allowed Routes in stable identifier order.
    pub fn allowed_route_ids(&self) -> impl Iterator<Item = &RouteId> {
        self.allowed_route_ids.iter()
    }
}

impl fmt::Debug for SnapshotClientKeyView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SnapshotClientKeyView")
            .field("record", &self.record)
            .field("allowed_route_ids", &self.allowed_route_ids)
            .finish()
    }
}

/// Complete immutable input used to construct one runtime Snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteSnapshotInput {
    version: SnapshotVersion,
    public_models: Vec<SnapshotPublicModel>,
    aliases: Vec<(String, PublicModelId)>,
    routes: Vec<SnapshotRoute>,
    access_groups: Vec<SnapshotAccessGroup>,
    client_keys: Vec<SnapshotClientKeyView>,
}

/// One exact Credential's durable discovery evidence used for data-plane materialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotCredentialCatalog {
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    state: CatalogModelState,
    models: BTreeSet<String>,
}

impl SnapshotCredentialCatalog {
    /// Creates one already-normalized exact-Credential Catalog input.
    #[must_use]
    pub fn new(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        state: CatalogModelState,
        models: BTreeSet<String>,
    ) -> Self {
        Self {
            endpoint_id,
            credential_id,
            state,
            models,
        }
    }
}

impl RouteSnapshotInput {
    /// Creates one complete Snapshot input with redacted Client Key HMAC views.
    #[must_use]
    pub fn new(
        version: SnapshotVersion,
        public_models: Vec<SnapshotPublicModel>,
        aliases: Vec<(String, PublicModelId)>,
        routes: Vec<SnapshotRoute>,
        access_groups: Vec<SnapshotAccessGroup>,
        client_keys: Vec<SnapshotClientKeyView>,
    ) -> Self {
        Self {
            version,
            public_models,
            aliases,
            routes,
            access_groups,
            client_keys,
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
    route_schedules: BTreeMap<RouteId, SnapshotRouteSchedule>,
    access_groups: BTreeMap<AccessGroupId, SnapshotAccessGroup>,
    client_keys: BTreeMap<ClientKeyPrefix, SnapshotClientKeyView>,
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

        let (routes, route_schedules) = build_routes(input.routes, &public_model_names_by_id)?;

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

        let mut client_keys = BTreeMap::new();
        let mut client_key_ids = BTreeSet::new();
        for client_key in input.client_keys {
            let access_group = access_groups
                .get(client_key.access_group_id())
                .ok_or(RouteSnapshotBuildError::ClientKeyReferencesUnknownAccessGroup)?;
            if client_key.allowed_route_ids != access_group.allowed_route_ids {
                return Err(RouteSnapshotBuildError::ClientKeyRoutePermissionsMismatch);
            }
            if !client_key_ids.insert(client_key.client_key_id().clone()) {
                return Err(RouteSnapshotBuildError::DuplicateClientKeyId);
            }
            if client_keys
                .insert(client_key.prefix().clone(), client_key)
                .is_some()
            {
                return Err(RouteSnapshotBuildError::DuplicateClientKeyPrefix);
            }
        }

        Ok(Self {
            version: input.version,
            public_models,
            public_model_names_by_id,
            aliases,
            routes,
            route_schedules,
            access_groups,
            client_keys,
        })
    }

    /// Materializes exact discovered models into a new immutable Snapshot.
    ///
    /// Existing Routes remain the capability and permission templates. A model is bound only to
    /// Credentials whose exact target-local Catalog still admits it. Newly discovered model IDs
    /// inherit their template Route's access grants; if more than one base Route could own the
    /// same new exact ID, that ID is omitted rather than resolved heuristically.
    ///
    /// # Errors
    ///
    /// Returns the ordinary Snapshot validation error if deterministic derived identities or the
    /// reconstructed graph violate an immutable routing invariant.
    #[allow(clippy::too_many_lines)] // Keep one atomic derived-Snapshot construction path auditable.
    pub fn materialize_credential_catalogs(
        &self,
        catalogs: impl IntoIterator<Item = SnapshotCredentialCatalog>,
    ) -> Result<Self, RouteSnapshotBuildError> {
        let catalogs = catalogs.into_iter().collect::<Vec<_>>();
        if catalogs.is_empty() {
            return Ok(self.clone());
        }
        let managed_endpoints = catalogs
            .iter()
            .map(|catalog| catalog.endpoint_id.clone())
            .collect::<BTreeSet<_>>();
        let mut eligibility =
            BTreeMap::<(EndpointId, String), (CatalogModelState, BTreeSet<CredentialId>)>::new();
        for catalog in &catalogs {
            for model in &catalog.models {
                if model.is_empty() || !catalog.state.is_hard_eligible() {
                    continue;
                }
                let entry = eligibility
                    .entry((catalog.endpoint_id.clone(), model.clone()))
                    .or_insert((catalog.state, BTreeSet::new()));
                entry.0 = fresher_catalog_state(entry.0, catalog.state);
                entry.1.insert(catalog.credential_id.clone());
            }
        }

        let mut public_models = self.public_models.values().cloned().collect::<Vec<_>>();
        let mut routes = Vec::new();
        let mut existing_exact_models = self.public_models.keys().cloned().collect::<BTreeSet<_>>();
        for route in self.routes.values() {
            let candidates = route
                .candidates
                .iter()
                .cloned()
                .map(|candidate| {
                    existing_exact_models.insert(candidate.upstream_model.clone());
                    restrict_catalog_candidate(candidate, &managed_endpoints, &eligibility)
                })
                .collect();
            routes.push(SnapshotRoute::new(
                route.id.clone(),
                route.public_model_id.clone(),
                route.policy,
                route.max_attempts,
                route.bootstrap_timeout_ms,
                candidates,
            ));
        }

        let discovered_models = eligibility
            .keys()
            .map(|(_, model)| model.clone())
            .collect::<BTreeSet<_>>();
        let mut derived_base_routes = BTreeMap::<RouteId, Vec<RouteId>>::new();
        for model in discovered_models.difference(&existing_exact_models) {
            let matching_routes = self
                .routes
                .values()
                .filter(|route| {
                    route.candidates.iter().any(|candidate| {
                        eligibility.contains_key(&(candidate.endpoint_id.clone(), model.clone()))
                            && eligibility.contains_key(&(
                                candidate.endpoint_id.clone(),
                                candidate.upstream_model.clone(),
                            ))
                    })
                })
                .collect::<Vec<_>>();
            if matching_routes.len() != 1 {
                continue;
            }
            let template_route = matching_routes[0];
            let Some(template_public_model_name) = self
                .public_model_names_by_id
                .get(&template_route.public_model_id)
            else {
                return Err(RouteSnapshotBuildError::UnknownRoutePublicModel);
            };
            let Some(template_public_model) = self.public_models.get(template_public_model_name)
            else {
                return Err(RouteSnapshotBuildError::UnknownRoutePublicModel);
            };
            let public_model_id = PublicModelId::try_new(derived_id(
                "catalog-model",
                template_route.id.as_str(),
                model,
            ))
            .map_err(|_| RouteSnapshotBuildError::InvalidDerivedIdentity)?;
            let route_id = RouteId::try_new(derived_id(
                "catalog-route",
                template_route.id.as_str(),
                model,
            ))
            .map_err(|_| RouteSnapshotBuildError::InvalidDerivedIdentity)?;
            let candidates = template_route
                .candidates
                .iter()
                .filter_map(|candidate| {
                    let (state, credentials) =
                        eligibility.get(&(candidate.endpoint_id.clone(), model.clone()))?;
                    let mut candidate = candidate.clone();
                    candidate.id = RouteCandidateId::try_new(derived_id(
                        "catalog-candidate",
                        candidate.id.as_str(),
                        model,
                    ))
                    .ok()?;
                    candidate.upstream_model.clone_from(model);
                    candidate.catalog_admission = SnapshotCatalogAdmission::Listed(*state);
                    Some(candidate.with_eligible_credentials(credentials.clone()))
                })
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                continue;
            }
            public_models.push(SnapshotPublicModel::new(
                public_model_id.clone(),
                model.clone(),
                model.clone(),
                template_public_model.required_capabilities.clone(),
                route_id.clone(),
            ));
            routes.push(SnapshotRoute::new(
                route_id.clone(),
                public_model_id,
                template_route.policy,
                template_route.max_attempts,
                template_route.bootstrap_timeout_ms,
                candidates,
            ));
            derived_base_routes
                .entry(template_route.id.clone())
                .or_default()
                .push(route_id);
        }

        let access_groups = self
            .access_groups
            .values()
            .cloned()
            .map(|mut group| {
                for (base_route, derived_routes) in &derived_base_routes {
                    if group.allowed_route_ids.contains(base_route) {
                        group
                            .allowed_route_ids
                            .extend(derived_routes.iter().cloned());
                    }
                }
                group
            })
            .collect::<Vec<_>>();
        let grants = access_groups
            .iter()
            .map(|group| (group.id.clone(), group.allowed_route_ids.clone()))
            .collect::<BTreeMap<_, _>>();
        let client_keys = self
            .client_keys
            .values()
            .map(|client_key| {
                let allowed = grants
                    .get(client_key.access_group_id())
                    .cloned()
                    .unwrap_or_default();
                SnapshotClientKeyView::new(client_key.record.clone(), allowed)
            })
            .collect();

        Self::try_new(RouteSnapshotInput::new(
            self.version.clone(),
            public_models,
            self.aliases
                .iter()
                .map(|(alias, target)| (alias.clone(), target.clone()))
                .collect(),
            routes,
            access_groups,
            client_keys,
        ))
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

    /// Resolves an exact Public Model or Alias only when an Access Group can visibly use it.
    ///
    /// This deliberately uses the same immutable permission and hard-eligibility predicate as
    /// [`Self::public_models_for_access_group`]. Runtime 429/Cooldown/Circuit state is not an
    /// input, so temporary availability cannot change the public mapping or model list.
    #[must_use]
    pub fn resolve_public_model_for_access_group(
        &self,
        access_group_id: &AccessGroupId,
        model_name_or_alias: &str,
    ) -> Option<&SnapshotPublicModel> {
        let access_group = self.access_group(access_group_id)?;
        let public_model = self.resolve_public_model(model_name_or_alias)?;
        self.public_model_is_visible_to(access_group, public_model)
            .then_some(public_model)
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

    /// Iterates Public Models visible to one Access Group in stable public-name order.
    ///
    /// A visible model has a Route granted to the group and at least one compiler-retained
    /// hard-eligible Candidate. This reads only this immutable Snapshot; it does not inspect
    /// Runtime Health, lease saturation, cooldowns, circuits, persistence, or a live Catalog.
    pub fn public_models_for_access_group(
        &self,
        access_group_id: &AccessGroupId,
    ) -> impl Iterator<Item = &SnapshotPublicModel> {
        let access_group = self.access_group(access_group_id);
        self.public_models.values().filter(move |public_model| {
            access_group.is_some_and(|access_group| {
                self.public_model_is_visible_to(access_group, public_model)
            })
        })
    }

    /// Iterates exact upstream model IDs visible to one Access Group in stable model order.
    ///
    /// The projection contains only hard-eligible Candidates of permitted Routes. An ID carried by
    /// more than one visible Route is omitted until an explicit deterministic route policy removes
    /// the ambiguity; request resolution independently reports the collision as
    /// [`SnapshotExactModelResolution::Ambiguous`].
    pub fn exact_upstream_models_for_access_group<'snapshot>(
        &'snapshot self,
        access_group_id: &AccessGroupId,
    ) -> impl Iterator<Item = &'snapshot str> {
        let mut models = BTreeSet::new();
        if let Some(access_group) = self.access_group(access_group_id) {
            for public_model in self.public_models.values() {
                if !self.public_model_is_visible_to(access_group, public_model) {
                    continue;
                }
                if let Some(route) = self.route(public_model.route_id()) {
                    models.extend(
                        route
                            .candidates()
                            .iter()
                            .filter(|candidate| candidate.is_hard_eligible())
                            .map(SnapshotRouteCandidate::upstream_model),
                    );
                }
            }
        }
        models.retain(|model| {
            matches!(
                self.resolve_exact_upstream_model_for_access_group(access_group_id, model),
                SnapshotExactModelResolution::Unique(_)
            )
        });
        models.into_iter()
    }

    /// Resolves one exact upstream model without allowing a CPAR alias to hide ambiguity.
    #[must_use]
    pub fn resolve_exact_upstream_model_for_access_group<'snapshot>(
        &'snapshot self,
        access_group_id: &AccessGroupId,
        upstream_model: &str,
    ) -> SnapshotExactModelResolution<'snapshot> {
        let Some(access_group) = self.access_group(access_group_id) else {
            return SnapshotExactModelResolution::Absent;
        };
        let mut resolved = None;
        for public_model in self.public_models.values() {
            if !self.public_model_is_visible_to(access_group, public_model) {
                continue;
            }
            let carries_model = self.route(public_model.route_id()).is_some_and(|route| {
                route.candidates().iter().any(|candidate| {
                    candidate.is_hard_eligible() && candidate.upstream_model() == upstream_model
                })
            });
            if !carries_model {
                continue;
            }
            if resolved.is_some() {
                return SnapshotExactModelResolution::Ambiguous;
            }
            resolved = Some(public_model);
        }
        resolved.map_or(
            SnapshotExactModelResolution::Absent,
            SnapshotExactModelResolution::Unique,
        )
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

    /// Returns the immutable precompiled Candidate schedule for one active Route.
    #[must_use]
    pub fn route_schedule(&self, route_id: &RouteId) -> Option<&SnapshotRouteSchedule> {
        self.route_schedules.get(route_id)
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

    /// Returns the Client Key view selected by one canonical public Prefix.
    #[must_use]
    pub fn client_key(&self, prefix: &ClientKeyPrefix) -> Option<&SnapshotClientKeyView> {
        self.client_keys.get(prefix)
    }

    /// Iterates Client Key views in stable Prefix order.
    pub fn client_keys(&self) -> impl Iterator<Item = &SnapshotClientKeyView> {
        self.client_keys.values()
    }

    fn public_model_is_visible_to(
        &self,
        access_group: &SnapshotAccessGroup,
        public_model: &SnapshotPublicModel,
    ) -> bool {
        access_group.permits_route(public_model.route_id())
            && self
                .route(public_model.route_id())
                .is_some_and(SnapshotRoute::has_hard_eligible_candidate)
    }
}

type BuiltRoutes = (
    BTreeMap<RouteId, SnapshotRoute>,
    BTreeMap<RouteId, SnapshotRouteSchedule>,
);

fn build_routes(
    input_routes: Vec<SnapshotRoute>,
    public_model_names_by_id: &BTreeMap<PublicModelId, String>,
) -> Result<BuiltRoutes, RouteSnapshotBuildError> {
    let mut routes = BTreeMap::new();
    let mut route_schedules = BTreeMap::new();
    let mut candidate_ids = BTreeSet::new();
    for route in input_routes {
        if routes.contains_key(&route.id) {
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
        let schedule = SnapshotRouteSchedule::try_compile(&route)?;
        route_schedules.insert(route.id.clone(), schedule);
        routes.insert(route.id.clone(), route);
    }
    Ok((routes, route_schedules))
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
    /// A Candidate used a negative priority tier.
    InvalidCandidatePriority,
    /// A Candidate used a non-positive scheduling weight.
    InvalidCandidateWeight,
    /// A precompiled priority-tier schedule would exceed its finite limit.
    RouteScheduleTooLarge,
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
    /// More than one Client Key used the same public Prefix.
    DuplicateClientKeyPrefix,
    /// More than one Client Key used the same stable identity.
    DuplicateClientKeyId,
    /// A Client Key referred to an Access Group absent from the active Snapshot.
    ClientKeyReferencesUnknownAccessGroup,
    /// A Client Key's copied Route permissions did not match its active Access Group.
    ClientKeyRoutePermissionsMismatch,
    /// A deterministic discovery-derived identifier could not be represented safely.
    InvalidDerivedIdentity,
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
            Self::InvalidCandidatePriority => "Snapshot Candidate has an invalid priority tier",
            Self::InvalidCandidateWeight => "Snapshot Candidate has an invalid scheduling weight",
            Self::RouteScheduleTooLarge => "Snapshot Route schedule exceeds its finite limit",
            Self::DuplicateCandidate => "Snapshot has a duplicate Candidate identity",
            Self::AliasConflictsPublicModel => "Snapshot Alias conflicts with a public model name",
            Self::DuplicateAlias => "Snapshot has a duplicate Alias",
            Self::UnknownAliasPublicModel => "Snapshot Alias refers to an unknown public model",
            Self::DuplicateAccessGroup => "Snapshot has a duplicate Access Group identity",
            Self::AccessGroupReferencesUnknownRoute => {
                "Snapshot Access Group refers to an unknown Route"
            }
            Self::DuplicateClientKeyPrefix => "Snapshot has a duplicate Client Key Prefix",
            Self::DuplicateClientKeyId => "Snapshot has a duplicate Client Key identity",
            Self::ClientKeyReferencesUnknownAccessGroup => {
                "Snapshot Client Key refers to an unknown active Access Group"
            }
            Self::ClientKeyRoutePermissionsMismatch => {
                "Snapshot Client Key permissions do not match its active Access Group"
            }
            Self::InvalidDerivedIdentity => "Snapshot derived Catalog identity is invalid",
        };
        formatter.write_str(description)
    }
}

fn restrict_catalog_candidate(
    mut candidate: SnapshotRouteCandidate,
    managed_endpoints: &BTreeSet<EndpointId>,
    eligibility: &BTreeMap<(EndpointId, String), (CatalogModelState, BTreeSet<CredentialId>)>,
) -> SnapshotRouteCandidate {
    if !managed_endpoints.contains(&candidate.endpoint_id) {
        return candidate;
    }
    if let Some((state, credentials)) = eligibility.get(&(
        candidate.endpoint_id.clone(),
        candidate.upstream_model.clone(),
    )) {
        candidate.catalog_admission = SnapshotCatalogAdmission::Listed(*state);
        candidate.with_eligible_credentials(credentials.clone())
    } else {
        candidate.catalog_admission = SnapshotCatalogAdmission::Listed(CatalogModelState::Expired);
        candidate.with_eligible_credentials(BTreeSet::new())
    }
}

const fn fresher_catalog_state(
    left: CatalogModelState,
    right: CatalogModelState,
) -> CatalogModelState {
    match (left, right) {
        (CatalogModelState::Fresh | CatalogModelState::Manual, _)
        | (_, CatalogModelState::Expired) => left,
        (_, CatalogModelState::Fresh | CatalogModelState::Manual) => right,
        _ => CatalogModelState::Stale,
    }
}

fn derived_id(domain: &str, base: &str, model: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(domain.as_bytes());
    digest.update([0]);
    digest.update(base.as_bytes());
    digest.update([0]);
    digest.update(model.as_bytes());
    let digest = digest.finalize();
    format!("dyn-{}", hex_prefix(&digest, 16))
}

fn hex_prefix(bytes: &[u8], length: usize) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(length.saturating_mul(2));
    for byte in bytes.iter().take(length) {
        let _ = write!(output, "{byte:02x}");
    }
    output
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

    /// Creates a registry with one current Snapshot and a durable-management reconstructed
    /// one-step rollback predecessor.
    ///
    /// This is intended for process bootstrap after the control plane has independently loaded
    /// the active Version and its most recent persisted predecessor. Request readers still only
    /// observe `current`; the optional predecessor remains exclusively behind the management
    /// publication mutex.
    ///
    /// # Errors
    ///
    /// Returns [`SnapshotRegistryError::SameVersion`] if the predecessor would not represent a
    /// real rollback transition.
    pub fn try_new_with_previous(
        initial: Arc<RouteSnapshot>,
        previous: Option<Arc<RouteSnapshot>>,
    ) -> Result<Self, SnapshotRegistryError> {
        if previous
            .as_ref()
            .is_some_and(|candidate| candidate.version == initial.version)
        {
            return Err(SnapshotRegistryError::SameVersion);
        }
        Ok(Self {
            current: ArcSwap::from(initial),
            publication_state: Mutex::new(SnapshotPublicationState { previous }),
        })
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

/// Supplies a Unix-millisecond timestamp for Snapshot Client Key admission.
pub trait SnapshotClientKeyClock: Send + Sync {
    /// Returns the current Unix-millisecond timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`SnapshotClientKeyClockError`] when the local system clock cannot be represented
    /// safely for P2-04 Client Key expiry verification.
    fn now_ms(&self) -> Result<i64, SnapshotClientKeyClockError>;
}

/// System clock implementation used by normal Snapshot Client Key authentication.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemSnapshotClientKeyClock;

impl SnapshotClientKeyClock for SystemSnapshotClientKeyClock {
    fn now_ms(&self) -> Result<i64, SnapshotClientKeyClockError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SnapshotClientKeyClockError::Unavailable)?;
        i64::try_from(elapsed.as_millis()).map_err(|_| SnapshotClientKeyClockError::Unavailable)
    }
}

/// Safe clock failures for Snapshot Client Key authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotClientKeyClockError {
    /// The system time was before the Unix epoch or outside the supported millisecond range.
    Unavailable,
}

impl fmt::Display for SnapshotClientKeyClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("Snapshot Client Key clock is unavailable"),
        }
    }
}

impl Error for SnapshotClientKeyClockError {}

/// Prefix-indexed HMAC Client Key authenticator backed only by the current immutable Snapshot.
pub struct SnapshotClientKeyAuthenticator {
    source: SnapshotSource,
    verifier: ClientKeyService,
    clock: Arc<dyn SnapshotClientKeyClock>,
}

enum SnapshotSource {
    Registry(Arc<RouteSnapshotRegistry>),
    Scheduler(Arc<crate::RouteCredentialScheduler>),
}

impl SnapshotSource {
    fn load(&self) -> Arc<RouteSnapshot> {
        match self {
            Self::Registry(registry) => registry.load(),
            Self::Scheduler(scheduler) => scheduler.snapshot(),
        }
    }
}

/// One successfully authenticated Client Key paired with its exact immutable Snapshot.
///
/// This value is the P3 public-model boundary: its Access Group filtering, Alias resolution, and
/// response-name mapping cannot accidentally reload a newer Snapshot after Client Key admission.
/// It contains no presented Key, HMAC digest, Credential, Endpoint, or Provider state.
pub struct SnapshotAuthenticatedClient {
    snapshot: Arc<RouteSnapshot>,
    authenticated_client: AuthenticatedClient,
    access_group_id: AccessGroupId,
}

impl SnapshotAuthenticatedClient {
    /// Returns the authenticated non-secret Client Key identity.
    #[must_use]
    pub fn client_key_id(&self) -> &ClientKeyId {
        self.authenticated_client.client_key_id()
    }

    /// Returns the active Access Group that was resolved from the same Snapshot.
    #[must_use]
    pub fn access_group_id(&self) -> &AccessGroupId {
        &self.access_group_id
    }

    /// Returns the Config Version retained for this authenticated request.
    #[must_use]
    pub fn snapshot_version(&self) -> &SnapshotVersion {
        self.snapshot.version()
    }

    /// Clones the exact immutable Snapshot pointer retained at authentication time.
    #[must_use]
    pub fn snapshot(&self) -> Arc<RouteSnapshot> {
        Arc::clone(&self.snapshot)
    }

    /// Iterates this Client Key's visible Public Models in stable name order.
    pub fn public_models(&self) -> impl Iterator<Item = &SnapshotPublicModel> {
        self.snapshot
            .public_models_for_access_group(self.access_group_id())
    }

    /// Iterates exact upstream model IDs visible to this authenticated Client Key.
    pub fn exact_upstream_models(&self) -> impl Iterator<Item = &str> {
        self.snapshot
            .exact_upstream_models_for_access_group(self.access_group_id())
    }

    /// Resolves an exact upstream model inside this Client Key's pinned authorized Snapshot.
    #[must_use]
    pub fn resolve_exact_upstream_model(
        &self,
        upstream_model: &str,
    ) -> SnapshotExactModelResolution<'_> {
        self.snapshot
            .resolve_exact_upstream_model_for_access_group(self.access_group_id(), upstream_model)
    }

    /// Resolves an exact Public Model or Alias to its visible stable Public Model.
    #[must_use]
    pub fn resolve_public_model(&self, model_name_or_alias: &str) -> Option<&SnapshotPublicModel> {
        self.snapshot
            .resolve_public_model_for_access_group(self.access_group_id(), model_name_or_alias)
    }

    fn into_authenticated_client(self) -> AuthenticatedClient {
        self.authenticated_client
    }
}

impl fmt::Debug for SnapshotAuthenticatedClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SnapshotAuthenticatedClient")
            .field("snapshot_version", self.snapshot.version())
            .field("client_key_id", self.client_key_id())
            .field("access_group_id", self.access_group_id())
            .finish_non_exhaustive()
    }
}

impl SnapshotClientKeyAuthenticator {
    /// Creates an authenticator using the local system clock.
    #[must_use]
    pub fn new(registry: Arc<RouteSnapshotRegistry>, verifier: ClientKeyService) -> Self {
        Self::with_clock(registry, verifier, Arc::new(SystemSnapshotClientKeyClock))
    }

    /// Creates an authenticator with an explicit clock for deterministic embedding or tests.
    #[must_use]
    pub fn with_clock(
        registry: Arc<RouteSnapshotRegistry>,
        verifier: ClientKeyService,
        clock: Arc<dyn SnapshotClientKeyClock>,
    ) -> Self {
        Self {
            source: SnapshotSource::Registry(registry),
            verifier,
            clock,
        }
    }

    /// Creates an authenticator backed by the exact mutable data-plane Catalog publication seam.
    #[must_use]
    pub fn new_with_scheduler(
        scheduler: Arc<crate::RouteCredentialScheduler>,
        verifier: ClientKeyService,
    ) -> Self {
        Self {
            source: SnapshotSource::Scheduler(scheduler),
            verifier,
            clock: Arc::new(SystemSnapshotClientKeyClock),
        }
    }

    /// Authenticates one presented Key and retains the exact Snapshot used for admission.
    ///
    /// Later HTTP model listing and response mapping use the returned value instead of loading
    /// the registry again, so an atomic publication cannot combine Client Key admission from one
    /// Config Version with permissions or Public Models from another.
    ///
    /// # Errors
    ///
    /// Returns the same safe authentication error as [`ClientKeyAuthenticator::authenticate`].
    pub fn authenticate_pinned(
        &self,
        presented_key: &str,
    ) -> Result<SnapshotAuthenticatedClient, GatewayError> {
        let snapshot = self.source.load();
        let authenticated_client = self.authenticate_snapshot(&snapshot, presented_key)?;
        let access_group_id = authenticated_client
            .access_group_id()
            .cloned()
            .ok_or_else(internal_request_error)?;
        Ok(SnapshotAuthenticatedClient {
            snapshot,
            authenticated_client,
            access_group_id,
        })
    }

    fn authenticate_snapshot(
        &self,
        snapshot: &RouteSnapshot,
        presented_key: &str,
    ) -> Result<AuthenticatedClient, GatewayError> {
        let prefix = ClientKeyPrefix::try_from_presented_key(presented_key)
            .map_err(|_| client_unauthorized_error())?;
        let client_key = snapshot
            .client_key(&prefix)
            .ok_or_else(client_unauthorized_error)?;
        let now_ms = self.clock.now_ms().map_err(|_| internal_request_error())?;
        let is_authenticated = self
            .verifier
            .verify(presented_key, &client_key.record, now_ms)
            .map_err(|_| internal_request_error())?;
        if !is_authenticated {
            return Err(client_unauthorized_error());
        }

        Ok(AuthenticatedClient::with_access_group(
            client_key.client_key_id().clone(),
            client_key.access_group_id().clone(),
        ))
    }
}

impl fmt::Debug for SnapshotClientKeyAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SnapshotClientKeyAuthenticator")
            .field("source", &"<immutable-snapshot-source>")
            .field("verifier", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl ClientKeyAuthenticator for SnapshotClientKeyAuthenticator {
    fn authenticate(&self, presented_key: &str) -> Result<AuthenticatedClient, GatewayError> {
        self.authenticate_pinned(presented_key)
            .map(SnapshotAuthenticatedClient::into_authenticated_client)
    }
}

const fn client_unauthorized_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientUnauthorized, ErrorScope::Request)
}

const fn internal_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Request)
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

    use gateway_auth::{
        ClientKeyAuthenticator,
        client_key::{
            ClientKeyDigest, ClientKeyPepper, ClientKeyPrefix, ClientKeyRecord, ClientKeyService,
            ClientKeyStatus, PresentedClientKey,
        },
    };
    use gateway_catalog::{CapabilitySet, CatalogModelState};

    use super::{
        RouteSnapshot, RouteSnapshotBuildError, RouteSnapshotInput, RouteSnapshotRegistry,
        SnapshotAccessGroup, SnapshotCatalogAdmission, SnapshotClientKeyAuthenticator,
        SnapshotClientKeyClock, SnapshotClientKeyClockError, SnapshotClientKeyView,
        SnapshotCredentialCatalog, SnapshotExactModelResolution, SnapshotPublicModel,
        SnapshotRegistryError, SnapshotRoute, SnapshotRouteCandidate, SnapshotRouteCandidateInput,
        SnapshotRoutePolicy, SnapshotTransformMode, SnapshotVersion,
    };
    use gateway_core::{
        AccessGroupId, ClientKeyId, CredentialId, EndpointId, ErrorScope, GatewayError,
        GatewayErrorCode, PublicModelId, RouteCandidateId, RouteId, UpstreamId,
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
    fn materialized_catalog_inherits_access_and_keeps_exact_credential_eligibility() -> TestResult {
        let snapshot = sample_snapshot("version-a")?;
        let materialized = snapshot.materialize_credential_catalogs([
            SnapshotCredentialCatalog::new(
                EndpointId::try_new("endpoint-a")?,
                CredentialId::try_new("credential-a")?,
                CatalogModelState::Fresh,
                BTreeSet::from(["grok-4.6".to_owned(), "upstream-model".to_owned()]),
            ),
            SnapshotCredentialCatalog::new(
                EndpointId::try_new("endpoint-a")?,
                CredentialId::try_new("credential-b")?,
                CatalogModelState::Stale,
                BTreeSet::from(["grok-4.6".to_owned(), "upstream-model".to_owned()]),
            ),
        ])?;
        let group_id = AccessGroupId::try_new("group-a")?;
        let public_model = materialized
            .resolve_public_model_for_access_group(&group_id, "grok-4.6")
            .ok_or("discovered model did not inherit access")?;
        let route = materialized
            .route(public_model.route_id())
            .ok_or("discovered route missing")?;
        assert_eq!(route.candidates().len(), 1);
        let candidate = &route.candidates()[0];
        assert!(candidate.allows_credential(&CredentialId::try_new("credential-a")?));
        assert!(candidate.allows_credential(&CredentialId::try_new("credential-b")?));
        assert!(!candidate.allows_credential(&CredentialId::try_new("credential-c")?));
        assert_eq!(candidate.active_binding_count(), 2);
        assert_eq!(
            candidate.catalog_admission(),
            SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh)
        );
        assert!(
            materialized
                .resolve_public_model_for_access_group(&group_id, "public-model")
                .is_some(),
            "the discovered anchor model must remain visible"
        );
        Ok(())
    }

    #[test]
    fn public_model_view_filters_access_groups_and_requires_hard_eligible_candidates() -> TestResult
    {
        let snapshot = visibility_snapshot("version-a")?;
        let group_a = AccessGroupId::try_new("group-a")?;
        let group_b = AccessGroupId::try_new("group-b")?;

        let visible_to_a = snapshot
            .public_models_for_access_group(&group_a)
            .map(SnapshotPublicModel::model_name)
            .collect::<Vec<_>>();
        assert_eq!(visible_to_a, vec!["alpha-model", "beta-model"]);
        assert!(!visible_to_a.contains(&"hidden-model"));
        assert!(!visible_to_a.contains(&"expired-model"));

        let visible_to_b = snapshot
            .public_models_for_access_group(&group_b)
            .map(SnapshotPublicModel::model_name)
            .collect::<Vec<_>>();
        assert_eq!(visible_to_b, vec!["beta-model"]);

        assert_eq!(
            snapshot
                .exact_upstream_models_for_access_group(&group_a)
                .collect::<Vec<_>>(),
            vec!["upstream-candidate-alpha", "upstream-candidate-beta"]
        );
        assert_eq!(
            snapshot
                .exact_upstream_models_for_access_group(&group_b)
                .collect::<Vec<_>>(),
            vec!["upstream-candidate-beta"]
        );
        assert!(matches!(
            snapshot.resolve_exact_upstream_model_for_access_group(
                &group_a,
                "upstream-candidate-alpha"
            ),
            SnapshotExactModelResolution::Unique(model) if model.model_name() == "alpha-model"
        ));
        assert_eq!(
            snapshot.resolve_exact_upstream_model_for_access_group(
                &group_b,
                "upstream-candidate-alpha"
            ),
            SnapshotExactModelResolution::Absent
        );

        let alias = snapshot
            .resolve_public_model_for_access_group(&group_a, "alias-alpha")
            .ok_or("expected visible Alias to resolve")?;
        assert_eq!(alias.model_name(), "alpha-model");
        assert!(
            snapshot
                .resolve_public_model_for_access_group(&group_b, "alias-alpha")
                .is_none()
        );
        assert!(
            snapshot
                .resolve_public_model_for_access_group(&group_a, "alias-hidden")
                .is_none()
        );
        assert!(
            snapshot
                .resolve_public_model_for_access_group(&group_a, "alias-expired")
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn exact_upstream_resolution_fails_closed_for_duplicate_visible_routes() -> TestResult {
        let snapshot = duplicate_exact_model_snapshot("version-collision")?;
        let access_group_id = AccessGroupId::try_new("group-a")?;

        assert_eq!(
            snapshot
                .exact_upstream_models_for_access_group(&access_group_id)
                .collect::<Vec<_>>(),
            Vec::<&str>::new()
        );
        assert_eq!(
            snapshot.resolve_exact_upstream_model_for_access_group(
                &access_group_id,
                "shared-upstream-model"
            ),
            SnapshotExactModelResolution::Ambiguous
        );
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
            Vec::new(),
        ));

        assert!(matches!(
            snapshot,
            Err(RouteSnapshotBuildError::PublicModelMissingRoute)
        ));
        Ok(())
    }

    #[test]
    fn rejects_invalid_or_unbounded_candidate_schedules() -> TestResult {
        assert!(matches!(
            snapshot_with_candidate_schedule(-1, 1)?,
            Err(RouteSnapshotBuildError::InvalidCandidatePriority)
        ));
        assert!(matches!(
            snapshot_with_candidate_schedule(0, 0)?,
            Err(RouteSnapshotBuildError::InvalidCandidateWeight)
        ));
        assert!(matches!(
            snapshot_with_candidate_schedule(0, 1_025)?,
            Err(RouteSnapshotBuildError::RouteScheduleTooLarge)
        ));
        Ok(())
    }

    #[test]
    fn rejects_duplicate_or_inconsistent_client_key_views() -> TestResult {
        let route_id = RouteId::try_new("route-a")?;
        let allowed_route_ids = BTreeSet::from([route_id]);
        let first =
            fixture_client_key_record("client-key-a", "group-a", "rgw_0123456789abcdef", 0xA5)?;
        let duplicate_prefix =
            fixture_client_key_record("client-key-b", "group-a", "rgw_0123456789abcdef", 0xB6)?;
        assert_snapshot_build_error(
            sample_snapshot_with_client_keys(
                "duplicate-prefix",
                vec![
                    SnapshotClientKeyView::new(first.clone(), allowed_route_ids.clone()),
                    SnapshotClientKeyView::new(duplicate_prefix, allowed_route_ids.clone()),
                ],
            ),
            RouteSnapshotBuildError::DuplicateClientKeyPrefix,
        )?;

        let duplicate_id =
            fixture_client_key_record("client-key-a", "group-a", "rgw_fedcba9876543210", 0xC7)?;
        assert_snapshot_build_error(
            sample_snapshot_with_client_keys(
                "duplicate-id",
                vec![
                    SnapshotClientKeyView::new(first.clone(), allowed_route_ids.clone()),
                    SnapshotClientKeyView::new(duplicate_id, allowed_route_ids.clone()),
                ],
            ),
            RouteSnapshotBuildError::DuplicateClientKeyId,
        )?;

        let unknown_group =
            fixture_client_key_record("client-key-b", "group-b", "rgw_fedcba9876543210", 0xD8)?;
        assert_snapshot_build_error(
            sample_snapshot_with_client_keys(
                "unknown-group",
                vec![SnapshotClientKeyView::new(
                    unknown_group,
                    allowed_route_ids.clone(),
                )],
            ),
            RouteSnapshotBuildError::ClientKeyReferencesUnknownAccessGroup,
        )?;

        assert_snapshot_build_error(
            sample_snapshot_with_client_keys(
                "permission-mismatch",
                vec![SnapshotClientKeyView::new(first, BTreeSet::new())],
            ),
            RouteSnapshotBuildError::ClientKeyRoutePermissionsMismatch,
        )?;
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
    fn reconstructed_predecessor_supports_the_same_one_step_rollback() -> TestResult {
        let registry = RouteSnapshotRegistry::try_new_with_previous(
            sample_snapshot("version-b")?,
            Some(sample_snapshot("version-a")?),
        )?;

        let rollback = registry.rollback()?;
        assert_eq!(rollback.previous_version().as_str(), "version-b");
        assert_eq!(rollback.current_version().as_str(), "version-a");
        assert_eq!(registry.load().version().as_str(), "version-a");
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

    #[test]
    fn snapshot_authenticator_pins_public_model_view_and_observes_key_disablement_after_publish()
    -> TestResult {
        let (service, active_record, presented_key) = issued_test_key(None)?;
        let mut disabled_record = active_record.clone();
        disabled_record.set_status(ClientKeyStatus::Disabled);
        let registry = Arc::new(RouteSnapshotRegistry::new(snapshot_with_client_key(
            "version-a",
            active_record.clone(),
        )?));
        let authenticator = SnapshotClientKeyAuthenticator::with_clock(
            Arc::clone(&registry),
            service,
            Arc::new(FixedClientKeyClock { now_ms: 1 }),
        );

        let authenticated = authenticator.authenticate_pinned(presented_key.as_str())?;
        assert_eq!(authenticated.client_key_id().as_str(), "client-key-a");
        assert_eq!(authenticated.access_group_id().as_str(), "group-a");
        assert_eq!(authenticated.snapshot_version().as_str(), "version-a");
        assert_eq!(
            authenticated
                .public_models()
                .map(SnapshotPublicModel::model_name)
                .collect::<Vec<_>>(),
            vec!["public-model"]
        );
        assert_eq!(
            authenticated.exact_upstream_models().collect::<Vec<_>>(),
            vec!["upstream-model"]
        );
        assert!(matches!(
            authenticated.resolve_exact_upstream_model("upstream-model"),
            SnapshotExactModelResolution::Unique(model) if model.model_name() == "public-model"
        ));
        assert_eq!(
            authenticated
                .resolve_public_model("model-alias")
                .map(SnapshotPublicModel::model_name),
            Some("public-model")
        );

        let held_snapshot = registry.load();
        let route_id = RouteId::try_new("route-a")?;
        let held_key = held_snapshot
            .client_key(active_record.prefix())
            .ok_or("expected active Client Key view")?;
        assert!(held_key.permits_route(&route_id));
        assert!(format!("{held_key:?}").contains("<redacted>"));

        registry.publish(snapshot_with_client_key("version-b", disabled_record)?)?;
        assert_eq!(held_snapshot.version().as_str(), "version-a");
        assert_eq!(authenticated.snapshot_version().as_str(), "version-a");
        assert_eq!(
            authenticated
                .public_models()
                .map(SnapshotPublicModel::model_name)
                .collect::<Vec<_>>(),
            vec!["public-model"]
        );
        assert_unauthorized(authenticator.authenticate(presented_key.as_str()))?;
        Ok(())
    }

    #[test]
    fn snapshot_authenticator_fails_closed_at_expiry_and_for_revoked_unknown_and_malformed_keys()
    -> TestResult {
        let (service, active_record, presented_key) = issued_test_key(Some(100))?;
        let registry = Arc::new(RouteSnapshotRegistry::new(snapshot_with_client_key(
            "version-a",
            active_record.clone(),
        )?));
        let before_expiry = SnapshotClientKeyAuthenticator::with_clock(
            Arc::clone(&registry),
            service,
            Arc::new(FixedClientKeyClock { now_ms: 99 }),
        );
        before_expiry.authenticate(presented_key.as_str())?;
        let wrong_secret = different_canonical_presented_key(presented_key.as_str())?;
        assert_unauthorized(before_expiry.authenticate(&wrong_secret))?;
        let unknown_key = canonical_unknown_key(&active_record);
        assert_unauthorized(before_expiry.authenticate(&unknown_key))?;

        let wrong_pepper = SnapshotClientKeyAuthenticator::with_clock(
            Arc::clone(&registry),
            client_key_service_with_pepper(0xB6)?,
            Arc::new(FixedClientKeyClock { now_ms: 99 }),
        );
        assert_unauthorized(wrong_pepper.authenticate(presented_key.as_str()))?;

        let at_expiry = SnapshotClientKeyAuthenticator::with_clock(
            Arc::clone(&registry),
            client_key_service()?,
            Arc::new(FixedClientKeyClock { now_ms: 100 }),
        );
        assert_unauthorized(at_expiry.authenticate(presented_key.as_str()))?;
        assert_unauthorized(at_expiry.authenticate("not-a-client-key"))?;

        let mut revoked_record = active_record;
        revoked_record.set_status(ClientKeyStatus::Revoked);
        registry.publish(snapshot_with_client_key("version-b", revoked_record)?)?;
        let revoked = SnapshotClientKeyAuthenticator::with_clock(
            registry,
            client_key_service()?,
            Arc::new(FixedClientKeyClock { now_ms: 99 }),
        );
        assert_unauthorized(revoked.authenticate(presented_key.as_str()))?;
        Ok(())
    }

    fn sample_snapshot(version: &str) -> Result<Arc<RouteSnapshot>, Box<dyn Error>> {
        sample_snapshot_with_client_keys(version, Vec::new())
    }

    fn snapshot_with_client_key(
        version: &str,
        client_key_record: ClientKeyRecord,
    ) -> Result<Arc<RouteSnapshot>, Box<dyn Error>> {
        let route_id = RouteId::try_new("route-a")?;
        sample_snapshot_with_client_keys(
            version,
            vec![SnapshotClientKeyView::new(
                client_key_record,
                BTreeSet::from([route_id]),
            )],
        )
    }

    fn sample_snapshot_with_client_keys(
        version: &str,
        client_keys: Vec<SnapshotClientKeyView>,
    ) -> Result<Arc<RouteSnapshot>, Box<dyn Error>> {
        let public_model_id = PublicModelId::try_new("public-model-a")?;
        let route_id = RouteId::try_new("route-a")?;
        let candidate = SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new("candidate-a")?,
            endpoint_id: EndpointId::try_new("endpoint-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            endpoint_api_format: "openai/responses".to_owned(),
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
            client_keys,
        ))?;
        Ok(Arc::new(snapshot))
    }

    fn visibility_snapshot(version: &str) -> Result<Arc<RouteSnapshot>, Box<dyn Error>> {
        let alpha_model_id = PublicModelId::try_new("public-model-alpha")?;
        let beta_model_id = PublicModelId::try_new("public-model-beta")?;
        let hidden_model_id = PublicModelId::try_new("public-model-hidden")?;
        let expired_model_id = PublicModelId::try_new("public-model-expired")?;
        let alpha_route_id = RouteId::try_new("route-alpha")?;
        let beta_route_id = RouteId::try_new("route-beta")?;
        let hidden_route_id = RouteId::try_new("route-hidden")?;
        let expired_route_id = RouteId::try_new("route-expired")?;

        let snapshot = RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new(version.to_owned())?,
            vec![
                visibility_public_model(
                    alpha_model_id.clone(),
                    "alpha-model",
                    alpha_route_id.clone(),
                ),
                visibility_public_model(beta_model_id.clone(), "beta-model", beta_route_id.clone()),
                visibility_public_model(
                    hidden_model_id.clone(),
                    "hidden-model",
                    hidden_route_id.clone(),
                ),
                visibility_public_model(
                    expired_model_id.clone(),
                    "expired-model",
                    expired_route_id.clone(),
                ),
            ],
            vec![
                ("alias-alpha".to_owned(), alpha_model_id.clone()),
                ("alias-beta".to_owned(), beta_model_id.clone()),
                ("alias-hidden".to_owned(), hidden_model_id.clone()),
                ("alias-expired".to_owned(), expired_model_id.clone()),
            ],
            vec![
                visibility_route(
                    alpha_route_id.clone(),
                    alpha_model_id,
                    "candidate-alpha",
                    "endpoint-alpha",
                    1,
                    CatalogModelState::Fresh,
                )?,
                visibility_route(
                    beta_route_id.clone(),
                    beta_model_id,
                    "candidate-beta",
                    "endpoint-beta",
                    1,
                    CatalogModelState::Fresh,
                )?,
                visibility_route(
                    hidden_route_id.clone(),
                    hidden_model_id,
                    "candidate-hidden",
                    "endpoint-hidden",
                    0,
                    CatalogModelState::Fresh,
                )?,
                visibility_route(
                    expired_route_id.clone(),
                    expired_model_id,
                    "candidate-expired",
                    "endpoint-expired",
                    1,
                    CatalogModelState::Expired,
                )?,
            ],
            vec![
                SnapshotAccessGroup::new(
                    AccessGroupId::try_new("group-a")?,
                    "Group A".to_owned(),
                    BTreeSet::from([
                        alpha_route_id.clone(),
                        beta_route_id.clone(),
                        hidden_route_id,
                        expired_route_id,
                    ]),
                ),
                SnapshotAccessGroup::new(
                    AccessGroupId::try_new("group-b")?,
                    "Group B".to_owned(),
                    BTreeSet::from([beta_route_id]),
                ),
            ],
            Vec::new(),
        ))?;
        Ok(Arc::new(snapshot))
    }

    fn duplicate_exact_model_snapshot(version: &str) -> Result<Arc<RouteSnapshot>, Box<dyn Error>> {
        let alpha_model_id = PublicModelId::try_new("public-model-alpha")?;
        let beta_model_id = PublicModelId::try_new("public-model-beta")?;
        let alpha_route_id = RouteId::try_new("route-alpha")?;
        let beta_route_id = RouteId::try_new("route-beta")?;
        let snapshot = RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new(version.to_owned())?,
            vec![
                visibility_public_model(
                    alpha_model_id.clone(),
                    "alpha-model",
                    alpha_route_id.clone(),
                ),
                visibility_public_model(beta_model_id.clone(), "beta-model", beta_route_id.clone()),
            ],
            Vec::new(),
            vec![
                visibility_route_with_upstream_model(
                    alpha_route_id.clone(),
                    alpha_model_id,
                    "candidate-alpha",
                    "endpoint-alpha",
                    "shared-upstream-model",
                    1,
                    CatalogModelState::Fresh,
                )?,
                visibility_route_with_upstream_model(
                    beta_route_id.clone(),
                    beta_model_id,
                    "candidate-beta",
                    "endpoint-beta",
                    "shared-upstream-model",
                    1,
                    CatalogModelState::Fresh,
                )?,
            ],
            vec![SnapshotAccessGroup::new(
                AccessGroupId::try_new("group-a")?,
                "Group A".to_owned(),
                BTreeSet::from([alpha_route_id, beta_route_id]),
            )],
            Vec::new(),
        ))?;
        Ok(Arc::new(snapshot))
    }

    fn visibility_public_model(
        public_model_id: PublicModelId,
        model_name: &str,
        route_id: RouteId,
    ) -> SnapshotPublicModel {
        SnapshotPublicModel::new(
            public_model_id,
            model_name.to_owned(),
            format!("{model_name} display"),
            CapabilitySet::empty(),
            route_id,
        )
    }

    fn visibility_route(
        route_id: RouteId,
        public_model_id: PublicModelId,
        candidate_id: &str,
        endpoint_id: &str,
        active_binding_count: usize,
        catalog_state: CatalogModelState,
    ) -> Result<SnapshotRoute, Box<dyn Error>> {
        let upstream_model = format!("upstream-{candidate_id}");
        visibility_route_with_upstream_model(
            route_id,
            public_model_id,
            candidate_id,
            endpoint_id,
            &upstream_model,
            active_binding_count,
            catalog_state,
        )
    }

    fn visibility_route_with_upstream_model(
        route_id: RouteId,
        public_model_id: PublicModelId,
        candidate_id: &str,
        endpoint_id: &str,
        upstream_model: &str,
        active_binding_count: usize,
        catalog_state: CatalogModelState,
    ) -> Result<SnapshotRoute, Box<dyn Error>> {
        Ok(SnapshotRoute::new(
            route_id,
            public_model_id,
            SnapshotRoutePolicy::RoundRobin,
            1,
            1_000,
            vec![visibility_candidate(
                candidate_id,
                endpoint_id,
                upstream_model,
                active_binding_count,
                catalog_state,
            )?],
        ))
    }

    fn visibility_candidate(
        candidate_id: &str,
        endpoint_id: &str,
        upstream_model: &str,
        active_binding_count: usize,
        catalog_state: CatalogModelState,
    ) -> Result<SnapshotRouteCandidate, Box<dyn Error>> {
        Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new(candidate_id)?,
            endpoint_id: EndpointId::try_new(endpoint_id)?,
            upstream_id: UpstreamId::try_new(format!("upstream-{endpoint_id}"))?,
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: upstream_model.to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::Listed(catalog_state),
            active_binding_count,
        }))
    }

    fn snapshot_with_candidate_schedule(
        priority: i64,
        weight: i64,
    ) -> Result<Result<RouteSnapshot, RouteSnapshotBuildError>, Box<dyn Error>> {
        let public_model_id = PublicModelId::try_new("public-model-a")?;
        let route_id = RouteId::try_new("route-a")?;
        let candidate = SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: RouteCandidateId::try_new("candidate-a")?,
            endpoint_id: EndpointId::try_new("endpoint-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: "upstream-model".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority,
            weight,
            effective_capabilities: CapabilitySet::empty(),
            catalog_admission: SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh),
            active_binding_count: 1,
        });
        Ok(RouteSnapshot::try_new(RouteSnapshotInput::new(
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
                SnapshotRoutePolicy::SmoothWeightedRoundRobin,
                2,
                10_000,
                vec![candidate],
            )],
            Vec::new(),
            Vec::new(),
        )))
    }

    fn issued_test_key(
        expires_at_ms: Option<i64>,
    ) -> Result<(ClientKeyService, ClientKeyRecord, PresentedClientKey), Box<dyn Error>> {
        let service = client_key_service()?;
        let issued = service.issue(
            ClientKeyId::try_new("client-key-a")?,
            AccessGroupId::try_new("group-a")?,
            expires_at_ms,
        )?;
        let (record, presented_key) = issued.into_parts();
        Ok((service, record, presented_key))
    }

    fn client_key_service() -> Result<ClientKeyService, Box<dyn Error>> {
        client_key_service_with_pepper(0xA5)
    }

    fn client_key_service_with_pepper(pepper_byte: u8) -> Result<ClientKeyService, Box<dyn Error>> {
        Ok(ClientKeyService::new(ClientKeyPepper::try_from_bytes(
            [pepper_byte; 32],
        )?))
    }

    fn fixture_client_key_record(
        client_key_id: &str,
        access_group_id: &str,
        prefix: &str,
        digest_byte: u8,
    ) -> Result<ClientKeyRecord, Box<dyn Error>> {
        Ok(ClientKeyRecord::try_new(
            ClientKeyId::try_new(client_key_id)?,
            AccessGroupId::try_new(access_group_id)?,
            ClientKeyPrefix::try_new(prefix)?,
            ClientKeyDigest::try_from_persisted([digest_byte; 32])?,
            ClientKeyStatus::Active,
            None,
        )?)
    }

    fn assert_snapshot_build_error(
        result: Result<Arc<RouteSnapshot>, Box<dyn Error>>,
        expected: RouteSnapshotBuildError,
    ) -> TestResult {
        let Err(error) = result else {
            return Err("Snapshot construction unexpectedly succeeded".into());
        };
        let actual = error
            .downcast_ref::<RouteSnapshotBuildError>()
            .ok_or("expected RouteSnapshotBuildError")?;
        assert_eq!(*actual, expected);
        Ok(())
    }

    fn different_canonical_presented_key(presented_key: &str) -> Result<String, Box<dyn Error>> {
        let mut wrong_key = presented_key.to_owned();
        let last = wrong_key
            .pop()
            .ok_or("expected a canonical presented Client Key")?;
        if !last.is_ascii_hexdigit() {
            return Err("expected a canonical presented Client Key secret".into());
        }
        wrong_key.push(if last == '0' { '1' } else { '0' });
        Ok(wrong_key)
    }

    fn canonical_unknown_key(record: &ClientKeyRecord) -> String {
        let prefix = if record.prefix().as_str() == "rgw_0000000000000000" {
            "rgw_ffffffffffffffff"
        } else {
            "rgw_0000000000000000"
        };
        format!("{prefix}_{}", "0".repeat(64))
    }

    fn assert_unauthorized(
        result: Result<gateway_auth::AuthenticatedClient, GatewayError>,
    ) -> TestResult {
        let Err(error) = result else {
            return Err("Client Key unexpectedly authenticated".into());
        };
        assert_eq!(error.code(), GatewayErrorCode::ClientUnauthorized);
        assert_eq!(error.scope(), ErrorScope::Request);
        assert_eq!(error.safe_message(), "the client is not authorized");
        Ok(())
    }

    struct FixedClientKeyClock {
        now_ms: i64,
    }

    impl SnapshotClientKeyClock for FixedClientKeyClock {
        fn now_ms(&self) -> Result<i64, SnapshotClientKeyClockError> {
            Ok(self.now_ms)
        }
    }
}
