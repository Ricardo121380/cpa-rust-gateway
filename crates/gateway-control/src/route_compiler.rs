//! Management-time semantic validation for one versioned control-plane graph.
//!
//! This compiler deliberately emits only secret-free values. It does not publish a Snapshot or
//! execute an inference request; P2-07 owns the former and the Router owns the latter.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use gateway_catalog::{
    CapabilitySet, CatalogModelState, CatalogView, EndpointCapabilityView, SemanticCapability,
};
use gateway_core::{
    AccessGroupId, EndpointId, PublicModelId, RouteCandidateId, RouteId, UpstreamId,
};
use gateway_store::control_plane::{
    AccessGroupConfiguration, AdministrativeStatus, ConfigVersionId, ControlPlaneConfiguration,
    CredentialConfiguration, CredentialStatus, EndpointConfiguration,
    EndpointCredentialBindingConfiguration, ModelRouteConfiguration, PublicModelConfiguration,
    RouteCandidateConfiguration, RoutePolicy, TransformMode, UpstreamConfiguration,
};
use serde_json::Value;

/// Validates one Config Version against injected Catalog and Endpoint capability evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteCompiler {
    catalog: CatalogView,
    endpoint_capabilities: EndpointCapabilityView,
}

impl RouteCompiler {
    /// Creates a compiler from immutable management-time evidence.
    #[must_use]
    pub const fn new(catalog: CatalogView, endpoint_capabilities: EndpointCapabilityView) -> Self {
        Self {
            catalog,
            endpoint_capabilities,
        }
    }

    /// Validates and compiles one complete Config Version.
    ///
    /// # Errors
    ///
    /// Returns [`RouteCompileError`] with a stable structural code when an Alias, reference,
    /// active Candidate, Catalog state, Endpoint capability profile, or Access Group grant cannot
    /// be published. No partial compiled configuration is returned on error.
    pub fn compile(
        &self,
        configuration: &ControlPlaneConfiguration,
    ) -> Result<CompiledRouteConfiguration, RouteCompileError> {
        let upstreams = index_upstreams(&configuration.upstreams)?;
        let endpoints = index_endpoints(&configuration.endpoints, &upstreams)?;
        let credentials = index_credentials(&configuration.credentials, &upstreams)?;
        let bindings = validate_bindings(
            &configuration.endpoint_credential_bindings,
            &endpoints,
            &credentials,
        )?;
        let public_models = index_public_models(&configuration.public_models)?;
        let required_capabilities = parse_public_model_capabilities(&public_models)?;
        let aliases = index_aliases(&configuration.model_aliases, &public_models)?;
        let routes = index_routes(&configuration.model_routes, &public_models)?;
        let candidates = index_candidates(&configuration.route_candidates, &routes, &endpoints)?;
        let access_groups = index_access_groups(&configuration.access_groups)?;
        let access_group_routes = validate_access_group_routes(
            &configuration.access_group_routes,
            &access_groups,
            &routes,
        )?;

        let candidate_context = CandidateCompilationContext {
            candidates: &candidates,
            routes: &routes,
            public_models: &public_models,
            required_capabilities: &required_capabilities,
            endpoints: &endpoints,
            upstreams: &upstreams,
            bindings: &bindings,
            credentials: &credentials,
        };
        let compiled_candidates = self.compile_active_candidates(&candidate_context)?;
        let compiled_routes =
            compile_active_routes(&routes, &public_models, &candidates, &compiled_candidates)?;
        let compiled_access_groups =
            compile_access_groups(&access_groups, &access_group_routes, &compiled_routes)?;

        let mut compiled_public_models = BTreeMap::new();
        for public_model in public_models.values() {
            if public_model.status != AdministrativeStatus::Active {
                continue;
            }
            let route_id = routes
                .route_for_public_model
                .get(&public_model.id)
                .ok_or_else(|| {
                    route_error(
                        RouteCompileErrorCode::ActivePublicModelMissingRoute,
                        public_model.id.as_str(),
                    )
                })?;
            let required_capabilities = required_capabilities
                .get(&public_model.id)
                .ok_or_else(|| {
                    route_error(
                        RouteCompileErrorCode::InvalidPublicModelCapabilities,
                        public_model.id.as_str(),
                    )
                })?
                .clone();
            compiled_public_models.insert(
                public_model.model_name.clone(),
                CompiledPublicModel {
                    id: public_model.id.clone(),
                    model_name: public_model.model_name.clone(),
                    display_name: public_model.display_name.clone(),
                    required_capabilities,
                    route_id: route_id.clone(),
                },
            );
        }

        let mut compiled_aliases = BTreeMap::new();
        for (alias, public_model_id) in aliases {
            let public_model = public_models.get(&public_model_id).ok_or_else(|| {
                route_error(
                    RouteCompileErrorCode::MissingAliasPublicModel,
                    alias.as_str(),
                )
            })?;
            if public_model.status == AdministrativeStatus::Active {
                compiled_aliases.insert(alias, public_model_id);
            }
        }

        Ok(CompiledRouteConfiguration {
            config_version_id: configuration.version.id.clone(),
            public_models: compiled_public_models,
            aliases: compiled_aliases,
            routes: compiled_routes,
            access_groups: compiled_access_groups,
        })
    }

    fn compile_active_candidates(
        &self,
        context: &CandidateCompilationContext<'_, '_>,
    ) -> Result<BTreeMap<RouteCandidateId, CompiledRouteCandidate>, RouteCompileError> {
        let mut compiled = BTreeMap::new();
        for candidate in context.candidates.by_id.values() {
            if !candidate.enabled || !candidate_has_active_public_model(candidate, context)? {
                continue;
            }
            let compiled_candidate = self.compile_active_candidate(candidate, context)?;
            compiled.insert(candidate.id.clone(), compiled_candidate);
        }
        Ok(compiled)
    }

    fn compile_active_candidate(
        &self,
        candidate: &RouteCandidateConfiguration,
        context: &CandidateCompilationContext<'_, '_>,
    ) -> Result<CompiledRouteCandidate, RouteCompileError> {
        let required = required_capabilities_for_candidate(candidate, context)?;
        let (endpoint, upstream) = active_candidate_target(candidate, context)?;
        let override_declaration = parse_candidate_override(candidate)?;
        let effective_capabilities = effective_candidate_capabilities(
            candidate,
            required,
            endpoint,
            &self.endpoint_capabilities,
            &override_declaration,
        )?;
        let catalog_admission = catalog_admission(
            &self.catalog,
            endpoint,
            candidate,
            override_declaration.allow_unlisted_model,
        )?;
        let active_binding_count = active_binding_count(endpoint, context);
        if active_binding_count == 0 {
            return Err(route_error(
                RouteCompileErrorCode::MissingActiveCredentialBinding,
                candidate.id.as_str(),
            ));
        }
        Ok(CompiledRouteCandidate {
            id: candidate.id.clone(),
            endpoint_id: endpoint.id.clone(),
            upstream_id: upstream.id.clone(),
            upstream_model: candidate.upstream_model.clone(),
            transform_mode: candidate.transform_mode,
            priority: candidate.priority,
            weight: candidate.weight,
            effective_capabilities,
            catalog_admission,
            active_binding_count,
        })
    }
}

fn candidate_has_active_public_model(
    candidate: &RouteCandidateConfiguration,
    context: &CandidateCompilationContext<'_, '_>,
) -> Result<bool, RouteCompileError> {
    let route = context
        .routes
        .by_id
        .get(&candidate.route_id)
        .ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::MissingCandidateRoute,
                candidate.id.as_str(),
            )
        })?;
    let public_model = context
        .public_models
        .get(&route.public_model_id)
        .ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::MissingRoutePublicModel,
                route.id.as_str(),
            )
        })?;
    Ok(public_model.status == AdministrativeStatus::Active)
}

/// Secret-free compilation result for one Config Version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRouteConfiguration {
    config_version_id: ConfigVersionId,
    public_models: BTreeMap<String, CompiledPublicModel>,
    aliases: BTreeMap<String, PublicModelId>,
    routes: BTreeMap<RouteId, CompiledRoute>,
    access_groups: BTreeMap<AccessGroupId, CompiledAccessGroup>,
}

impl CompiledRouteConfiguration {
    /// Returns the Config Version that was compiled.
    #[must_use]
    pub fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Returns an active Public Model by exact public name.
    #[must_use]
    pub fn public_model(&self, model_name: &str) -> Option<&CompiledPublicModel> {
        self.public_models.get(model_name)
    }

    /// Iterates active Public Models in stable public-name order.
    pub fn public_models(&self) -> impl Iterator<Item = &CompiledPublicModel> {
        self.public_models.values()
    }

    /// Returns the active Alias target by exact Alias text.
    #[must_use]
    pub fn alias_target(&self, alias: &str) -> Option<&PublicModelId> {
        self.aliases.get(alias)
    }

    /// Iterates active Alias-to-Public-Model relations in stable Alias order.
    pub fn aliases(&self) -> impl Iterator<Item = (&str, &PublicModelId)> {
        self.aliases
            .iter()
            .map(|(alias, public_model_id)| (alias.as_str(), public_model_id))
    }

    /// Returns an active compiled Route by Route identifier.
    #[must_use]
    pub fn route(&self, route_id: &RouteId) -> Option<&CompiledRoute> {
        self.routes.get(route_id)
    }

    /// Iterates active compiled Routes in stable identifier order.
    pub fn routes(&self) -> impl Iterator<Item = &CompiledRoute> {
        self.routes.values()
    }

    /// Returns an active Access Group view by identity.
    #[must_use]
    pub fn access_group(&self, access_group_id: &AccessGroupId) -> Option<&CompiledAccessGroup> {
        self.access_groups.get(access_group_id)
    }

    /// Iterates active compiled Access Groups in stable identifier order.
    pub fn access_groups(&self) -> impl Iterator<Item = &CompiledAccessGroup> {
        self.access_groups.values()
    }
}

/// An active public model that has a publishable Route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledPublicModel {
    id: PublicModelId,
    model_name: String,
    display_name: String,
    required_capabilities: CapabilitySet,
    route_id: RouteId,
}

impl CompiledPublicModel {
    /// Returns the stable public model identity.
    #[must_use]
    pub fn id(&self) -> &PublicModelId {
        &self.id
    }

    /// Returns the exact client-visible public model name.
    #[must_use]
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Returns the non-secret display label.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the capabilities promised by this public model.
    #[must_use]
    pub fn required_capabilities(&self) -> &CapabilitySet {
        &self.required_capabilities
    }

    /// Returns the active compiled Route identity.
    #[must_use]
    pub fn route_id(&self) -> &RouteId {
        &self.route_id
    }
}

/// A validated active Route with deterministically ordered Candidates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRoute {
    id: RouteId,
    public_model_id: PublicModelId,
    policy: RoutePolicy,
    max_attempts: i64,
    bootstrap_timeout_ms: i64,
    candidates: Vec<CompiledRouteCandidate>,
}

impl CompiledRoute {
    /// Returns the Route identity.
    #[must_use]
    pub fn id(&self) -> &RouteId {
        &self.id
    }

    /// Returns the associated Public Model identity.
    #[must_use]
    pub fn public_model_id(&self) -> &PublicModelId {
        &self.public_model_id
    }

    /// Returns the later runtime scheduling policy.
    #[must_use]
    pub const fn policy(&self) -> RoutePolicy {
        self.policy
    }

    /// Returns the positive total attempt bound.
    #[must_use]
    pub const fn max_attempts(&self) -> i64 {
        self.max_attempts
    }

    /// Returns the positive bootstrap timeout in milliseconds.
    #[must_use]
    pub const fn bootstrap_timeout_ms(&self) -> i64 {
        self.bootstrap_timeout_ms
    }

    /// Returns hard-eligible Candidates in deterministic priority/ID order.
    #[must_use]
    pub fn candidates(&self) -> &[CompiledRouteCandidate] {
        &self.candidates
    }
}

/// A validated Candidate without Credential or Client Key material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRouteCandidate {
    id: RouteCandidateId,
    endpoint_id: EndpointId,
    upstream_id: UpstreamId,
    upstream_model: String,
    transform_mode: TransformMode,
    priority: i64,
    weight: i64,
    effective_capabilities: CapabilitySet,
    catalog_admission: CatalogAdmission,
    active_binding_count: usize,
}

impl CompiledRouteCandidate {
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

    /// Returns the exact model string sent to the upstream.
    #[must_use]
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// Returns the declared conversion mode.
    #[must_use]
    pub const fn transform_mode(&self) -> TransformMode {
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

    /// Returns the effective Endpoint capability profile after candidate narrowing.
    #[must_use]
    pub fn effective_capabilities(&self) -> &CapabilitySet {
        &self.effective_capabilities
    }

    /// Returns the Catalog admission reason retained for management diagnostics.
    #[must_use]
    pub const fn catalog_admission(&self) -> CatalogAdmission {
        self.catalog_admission
    }

    /// Returns only the count of active bound Credentials, never their identities or ciphertext.
    #[must_use]
    pub const fn active_binding_count(&self) -> usize {
        self.active_binding_count
    }
}

/// Catalog reason allowing a Candidate to be hard-eligible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogAdmission {
    /// The Candidate was listed by a manual, fresh, or stale Catalog entry.
    Listed(CatalogModelState),
    /// The Candidate used the explicit `allow_unlisted_model` configuration exception.
    AllowedUnlisted,
}

/// A compiled active Access Group permission view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledAccessGroup {
    id: AccessGroupId,
    name: String,
    allowed_route_ids: BTreeSet<RouteId>,
}

impl CompiledAccessGroup {
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

    /// Returns whether this active group may use one active compiled Route.
    #[must_use]
    pub fn permits_route(&self, route_id: &RouteId) -> bool {
        self.allowed_route_ids.contains(route_id)
    }

    /// Iterates permitted Route identities in stable order.
    pub fn allowed_route_ids(&self) -> impl Iterator<Item = &RouteId> {
        self.allowed_route_ids.iter()
    }
}

/// Stable safe failure code for Route compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteCompileErrorCode {
    /// An Upstream ID appeared more than once.
    DuplicateUpstream,
    /// An Endpoint ID appeared more than once.
    DuplicateEndpoint,
    /// One Upstream declared more than one Endpoint for the same API format.
    DuplicateEndpointApiFormat,
    /// An Endpoint referenced an absent Upstream.
    MissingEndpointUpstream,
    /// A Credential ID appeared more than once.
    DuplicateCredential,
    /// A Credential referenced an absent Upstream.
    MissingCredentialUpstream,
    /// A binding referenced an absent Endpoint.
    MissingBindingEndpoint,
    /// A binding referenced an absent Credential.
    MissingBindingCredential,
    /// A binding did not share one Upstream with its Endpoint and Credential.
    BindingUpstreamMismatch,
    /// A Public Model ID appeared more than once.
    DuplicatePublicModel,
    /// More than one Public Model used the same client-visible name.
    DuplicatePublicModelName,
    /// Public Model capability JSON was malformed or internally inconsistent.
    InvalidPublicModelCapabilities,
    /// An Alias appeared more than once.
    DuplicateAlias,
    /// An Alias duplicated a Public Model client-visible name.
    AliasConflictsPublicModel,
    /// An Alias referenced an absent Public Model.
    MissingAliasPublicModel,
    /// A Route ID appeared more than once.
    DuplicateRoute,
    /// More than one Route referenced the same Public Model.
    DuplicateRouteForPublicModel,
    /// A Route referenced an absent Public Model.
    MissingRoutePublicModel,
    /// A Candidate ID appeared more than once.
    DuplicateCandidate,
    /// More than one Candidate used the same Route, Endpoint, and upstream model.
    DuplicateCandidateTarget,
    /// A Candidate referenced an absent Route.
    MissingCandidateRoute,
    /// A Candidate referenced an absent Endpoint.
    MissingCandidateEndpoint,
    /// Candidate capability JSON was malformed or internally inconsistent.
    InvalidCandidateCapabilities,
    /// An active Candidate referenced a disabled Endpoint.
    ActiveCandidateDisabledEndpoint,
    /// An active Candidate referenced a disabled Upstream.
    ActiveCandidateDisabledUpstream,
    /// An active Candidate had no injected Endpoint capability profile.
    MissingEndpointCapabilityProfile,
    /// Candidate configuration claimed a capability absent from the Endpoint profile.
    CandidateCapabilityEscalation,
    /// The effective Candidate profile could not meet Public Model requirements.
    CandidateCapabilityMismatch,
    /// The Candidate upstream model was absent or expired in Catalog without an exception.
    CatalogModelNotEligible,
    /// An active Candidate had no enabled binding to an active same-Upstream Credential.
    MissingActiveCredentialBinding,
    /// An Access Group ID appeared more than once.
    DuplicateAccessGroup,
    /// An Access Group route relation referenced an absent Access Group.
    MissingAccessGroup,
    /// An Access Group route relation referenced an absent Route.
    MissingAccessGroupRoute,
    /// An active Public Model did not have a Route.
    ActivePublicModelMissingRoute,
    /// An active Route had no hard-eligible Candidate.
    RouteHasNoHardEligibleCandidate,
    /// An enabled active Access Group grant targeted a nonpublishable Route.
    AccessGroupRouteNotPublishable,
}

impl RouteCompileErrorCode {
    /// Returns the stable machine-readable lower-snake-case error code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateUpstream => "duplicate_upstream",
            Self::DuplicateEndpoint => "duplicate_endpoint",
            Self::DuplicateEndpointApiFormat => "duplicate_endpoint_api_format",
            Self::MissingEndpointUpstream => "missing_endpoint_upstream",
            Self::DuplicateCredential => "duplicate_credential",
            Self::MissingCredentialUpstream => "missing_credential_upstream",
            Self::MissingBindingEndpoint => "missing_binding_endpoint",
            Self::MissingBindingCredential => "missing_binding_credential",
            Self::BindingUpstreamMismatch => "binding_upstream_mismatch",
            Self::DuplicatePublicModel => "duplicate_public_model",
            Self::DuplicatePublicModelName => "duplicate_public_model_name",
            Self::InvalidPublicModelCapabilities => "invalid_public_model_capabilities",
            Self::DuplicateAlias => "duplicate_alias",
            Self::AliasConflictsPublicModel => "alias_conflicts_public_model",
            Self::MissingAliasPublicModel => "missing_alias_public_model",
            Self::DuplicateRoute => "duplicate_route",
            Self::DuplicateRouteForPublicModel => "duplicate_route_for_public_model",
            Self::MissingRoutePublicModel => "missing_route_public_model",
            Self::DuplicateCandidate => "duplicate_candidate",
            Self::DuplicateCandidateTarget => "duplicate_candidate_target",
            Self::MissingCandidateRoute => "missing_candidate_route",
            Self::MissingCandidateEndpoint => "missing_candidate_endpoint",
            Self::InvalidCandidateCapabilities => "invalid_candidate_capabilities",
            Self::ActiveCandidateDisabledEndpoint => "active_candidate_disabled_endpoint",
            Self::ActiveCandidateDisabledUpstream => "active_candidate_disabled_upstream",
            Self::MissingEndpointCapabilityProfile => "missing_endpoint_capability_profile",
            Self::CandidateCapabilityEscalation => "candidate_capability_escalation",
            Self::CandidateCapabilityMismatch => "candidate_capability_mismatch",
            Self::CatalogModelNotEligible => "catalog_model_not_eligible",
            Self::MissingActiveCredentialBinding => "missing_active_credential_binding",
            Self::DuplicateAccessGroup => "duplicate_access_group",
            Self::MissingAccessGroup => "missing_access_group",
            Self::MissingAccessGroupRoute => "missing_access_group_route",
            Self::ActivePublicModelMissingRoute => "active_public_model_missing_route",
            Self::RouteHasNoHardEligibleCandidate => "route_has_no_hard_eligible_candidate",
            Self::AccessGroupRouteNotPublishable => "access_group_route_not_publishable",
        }
    }
}

/// Safe error returned when a Config Version cannot compile into publishable Routes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteCompileError {
    code: RouteCompileErrorCode,
    subject: String,
}

impl RouteCompileError {
    /// Returns the stable structural failure code.
    #[must_use]
    pub const fn code(&self) -> RouteCompileErrorCode {
        self.code
    }

    /// Returns the non-secret entity/configuration label associated with the failure.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }
}

impl fmt::Display for RouteCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Route compilation failed: {} ({})",
            self.code.as_str(),
            self.subject
        )
    }
}

impl Error for RouteCompileError {}

struct RouteIndex<'a> {
    by_id: BTreeMap<RouteId, &'a ModelRouteConfiguration>,
    route_for_public_model: BTreeMap<PublicModelId, RouteId>,
}

struct CandidateIndex<'a> {
    by_id: BTreeMap<RouteCandidateId, &'a RouteCandidateConfiguration>,
    by_route: BTreeMap<RouteId, Vec<&'a RouteCandidateConfiguration>>,
}

struct BindingIndex<'a> {
    by_endpoint: BTreeMap<EndpointId, Vec<&'a EndpointCredentialBindingConfiguration>>,
}

struct CandidateOverride {
    asserted: BTreeSet<SemanticCapability>,
    removed: BTreeSet<SemanticCapability>,
    allow_unlisted_model: bool,
}

struct CandidateCompilationContext<'view, 'configuration> {
    candidates: &'view CandidateIndex<'configuration>,
    routes: &'view RouteIndex<'configuration>,
    public_models: &'view BTreeMap<PublicModelId, &'configuration PublicModelConfiguration>,
    required_capabilities: &'view BTreeMap<PublicModelId, CapabilitySet>,
    endpoints: &'view BTreeMap<EndpointId, &'configuration EndpointConfiguration>,
    upstreams: &'view BTreeMap<UpstreamId, &'configuration UpstreamConfiguration>,
    bindings: &'view BindingIndex<'configuration>,
    credentials:
        &'view BTreeMap<gateway_core::CredentialId, &'configuration CredentialConfiguration>,
}

fn required_capabilities_for_candidate<'view>(
    candidate: &RouteCandidateConfiguration,
    context: &CandidateCompilationContext<'view, '_>,
) -> Result<&'view CapabilitySet, RouteCompileError> {
    let route = context
        .routes
        .by_id
        .get(&candidate.route_id)
        .ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::MissingCandidateRoute,
                candidate.id.as_str(),
            )
        })?;
    let public_model = context
        .public_models
        .get(&route.public_model_id)
        .ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::MissingRoutePublicModel,
                route.id.as_str(),
            )
        })?;
    context
        .required_capabilities
        .get(&public_model.id)
        .ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::InvalidPublicModelCapabilities,
                public_model.id.as_str(),
            )
        })
}

fn active_candidate_target<'configuration>(
    candidate: &RouteCandidateConfiguration,
    context: &CandidateCompilationContext<'_, 'configuration>,
) -> Result<
    (
        &'configuration EndpointConfiguration,
        &'configuration UpstreamConfiguration,
    ),
    RouteCompileError,
> {
    let endpoint = context
        .endpoints
        .get(&candidate.endpoint_id)
        .ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::MissingCandidateEndpoint,
                candidate.id.as_str(),
            )
        })?;
    if !endpoint.enabled {
        return Err(route_error(
            RouteCompileErrorCode::ActiveCandidateDisabledEndpoint,
            candidate.id.as_str(),
        ));
    }
    let upstream = context
        .upstreams
        .get(&endpoint.upstream_id)
        .ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::MissingEndpointUpstream,
                endpoint.id.as_str(),
            )
        })?;
    if !upstream.enabled {
        return Err(route_error(
            RouteCompileErrorCode::ActiveCandidateDisabledUpstream,
            candidate.id.as_str(),
        ));
    }
    Ok((endpoint, upstream))
}

fn effective_candidate_capabilities(
    candidate: &RouteCandidateConfiguration,
    required: &CapabilitySet,
    endpoint: &EndpointConfiguration,
    endpoint_capabilities: &EndpointCapabilityView,
    override_declaration: &CandidateOverride,
) -> Result<CapabilitySet, RouteCompileError> {
    let endpoint_capabilities = endpoint_capabilities
        .capabilities_for(&endpoint.id)
        .ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::MissingEndpointCapabilityProfile,
                endpoint.id.as_str(),
            )
        })?;
    for asserted in &override_declaration.asserted {
        if !endpoint_capabilities.supports(*asserted) {
            return Err(route_error(
                RouteCompileErrorCode::CandidateCapabilityEscalation,
                candidate.id.as_str(),
            ));
        }
    }
    let effective = endpoint_capabilities.without(override_declaration.removed.iter().copied());
    if !effective.supports_all(required) {
        return Err(route_error(
            RouteCompileErrorCode::CandidateCapabilityMismatch,
            candidate.id.as_str(),
        ));
    }
    Ok(effective)
}

fn catalog_admission(
    catalog: &CatalogView,
    endpoint: &EndpointConfiguration,
    candidate: &RouteCandidateConfiguration,
    allow_unlisted_model: bool,
) -> Result<CatalogAdmission, RouteCompileError> {
    match catalog.model_state(&endpoint.id, &candidate.upstream_model) {
        Some(state) if state.is_hard_eligible() => Ok(CatalogAdmission::Listed(state)),
        _ if allow_unlisted_model => Ok(CatalogAdmission::AllowedUnlisted),
        _ => Err(route_error(
            RouteCompileErrorCode::CatalogModelNotEligible,
            candidate.id.as_str(),
        )),
    }
}

fn active_binding_count(
    endpoint: &EndpointConfiguration,
    context: &CandidateCompilationContext<'_, '_>,
) -> usize {
    context
        .bindings
        .by_endpoint
        .get(&endpoint.id)
        .map_or(0, |endpoint_bindings| {
            endpoint_bindings
                .iter()
                .filter(|binding| {
                    binding.enabled
                        && context.credentials.get(&binding.credential_id).is_some_and(
                            |credential| {
                                credential.status == CredentialStatus::Active
                                    && credential.upstream_id == endpoint.upstream_id
                            },
                        )
                })
                .count()
        })
}

fn index_upstreams(
    upstreams: &[UpstreamConfiguration],
) -> Result<BTreeMap<UpstreamId, &UpstreamConfiguration>, RouteCompileError> {
    let mut indexed = BTreeMap::new();
    for upstream in upstreams {
        if indexed.insert(upstream.id.clone(), upstream).is_some() {
            return Err(route_error(
                RouteCompileErrorCode::DuplicateUpstream,
                upstream.id.as_str(),
            ));
        }
    }
    Ok(indexed)
}

fn index_endpoints<'a>(
    endpoints: &'a [EndpointConfiguration],
    upstreams: &BTreeMap<UpstreamId, &UpstreamConfiguration>,
) -> Result<BTreeMap<EndpointId, &'a EndpointConfiguration>, RouteCompileError> {
    let mut indexed = BTreeMap::new();
    let mut endpoint_formats = BTreeSet::new();
    for endpoint in endpoints {
        if !upstreams.contains_key(&endpoint.upstream_id) {
            return Err(route_error(
                RouteCompileErrorCode::MissingEndpointUpstream,
                endpoint.id.as_str(),
            ));
        }
        if indexed.insert(endpoint.id.clone(), endpoint).is_some() {
            return Err(route_error(
                RouteCompileErrorCode::DuplicateEndpoint,
                endpoint.id.as_str(),
            ));
        }
        if !endpoint_formats.insert((endpoint.upstream_id.clone(), endpoint.api_format.clone())) {
            return Err(route_error(
                RouteCompileErrorCode::DuplicateEndpointApiFormat,
                endpoint.id.as_str(),
            ));
        }
    }
    Ok(indexed)
}

fn index_credentials<'a>(
    credentials: &'a [CredentialConfiguration],
    upstreams: &BTreeMap<UpstreamId, &UpstreamConfiguration>,
) -> Result<BTreeMap<gateway_core::CredentialId, &'a CredentialConfiguration>, RouteCompileError> {
    let mut indexed = BTreeMap::new();
    for credential in credentials {
        if !upstreams.contains_key(&credential.upstream_id) {
            return Err(route_error(
                RouteCompileErrorCode::MissingCredentialUpstream,
                credential.id.as_str(),
            ));
        }
        if indexed.insert(credential.id.clone(), credential).is_some() {
            return Err(route_error(
                RouteCompileErrorCode::DuplicateCredential,
                credential.id.as_str(),
            ));
        }
    }
    Ok(indexed)
}

fn validate_bindings<'a>(
    bindings: &'a [EndpointCredentialBindingConfiguration],
    endpoints: &BTreeMap<EndpointId, &EndpointConfiguration>,
    credentials: &BTreeMap<gateway_core::CredentialId, &CredentialConfiguration>,
) -> Result<BindingIndex<'a>, RouteCompileError> {
    let mut by_endpoint: BTreeMap<EndpointId, Vec<&EndpointCredentialBindingConfiguration>> =
        BTreeMap::new();
    for binding in bindings {
        let endpoint = endpoints.get(&binding.endpoint_id).ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::MissingBindingEndpoint,
                binding.endpoint_id.as_str(),
            )
        })?;
        let credential = credentials.get(&binding.credential_id).ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::MissingBindingCredential,
                binding.credential_id.as_str(),
            )
        })?;
        if binding.upstream_id != endpoint.upstream_id
            || binding.upstream_id != credential.upstream_id
        {
            return Err(route_error(
                RouteCompileErrorCode::BindingUpstreamMismatch,
                binding.credential_id.as_str(),
            ));
        }
        by_endpoint
            .entry(binding.endpoint_id.clone())
            .or_default()
            .push(binding);
    }
    Ok(BindingIndex { by_endpoint })
}

fn index_public_models(
    public_models: &[PublicModelConfiguration],
) -> Result<BTreeMap<PublicModelId, &PublicModelConfiguration>, RouteCompileError> {
    let mut by_id = BTreeMap::new();
    let mut names = BTreeSet::new();
    for public_model in public_models {
        if by_id
            .insert(public_model.id.clone(), public_model)
            .is_some()
        {
            return Err(route_error(
                RouteCompileErrorCode::DuplicatePublicModel,
                public_model.id.as_str(),
            ));
        }
        if !names.insert(public_model.model_name.clone()) {
            return Err(route_error(
                RouteCompileErrorCode::DuplicatePublicModelName,
                public_model.model_name.as_str(),
            ));
        }
    }
    Ok(by_id)
}

fn parse_public_model_capabilities(
    public_models: &BTreeMap<PublicModelId, &PublicModelConfiguration>,
) -> Result<BTreeMap<PublicModelId, CapabilitySet>, RouteCompileError> {
    let mut parsed = BTreeMap::new();
    for public_model in public_models.values() {
        let capabilities = parse_capability_object(
            &public_model.capabilities_json,
            public_model.id.as_str(),
            RouteCompileErrorCode::InvalidPublicModelCapabilities,
            false,
        )?;
        let required = CapabilitySet::try_new(capabilities.asserted).map_err(|_| {
            route_error(
                RouteCompileErrorCode::InvalidPublicModelCapabilities,
                public_model.id.as_str(),
            )
        })?;
        parsed.insert(public_model.id.clone(), required);
    }
    Ok(parsed)
}

fn index_aliases(
    aliases: &[gateway_store::control_plane::ModelAliasConfiguration],
    public_models: &BTreeMap<PublicModelId, &PublicModelConfiguration>,
) -> Result<BTreeMap<String, PublicModelId>, RouteCompileError> {
    let public_names: BTreeSet<_> = public_models
        .values()
        .map(|public_model| public_model.model_name.as_str())
        .collect();
    let mut indexed = BTreeMap::new();
    for alias in aliases {
        if !public_models.contains_key(&alias.public_model_id) {
            return Err(route_error(
                RouteCompileErrorCode::MissingAliasPublicModel,
                alias.alias.as_str(),
            ));
        }
        if public_names.contains(alias.alias.as_str()) {
            return Err(route_error(
                RouteCompileErrorCode::AliasConflictsPublicModel,
                alias.alias.as_str(),
            ));
        }
        if indexed
            .insert(alias.alias.clone(), alias.public_model_id.clone())
            .is_some()
        {
            return Err(route_error(
                RouteCompileErrorCode::DuplicateAlias,
                alias.alias.as_str(),
            ));
        }
    }
    Ok(indexed)
}

fn index_routes<'a>(
    routes: &'a [ModelRouteConfiguration],
    public_models: &BTreeMap<PublicModelId, &PublicModelConfiguration>,
) -> Result<RouteIndex<'a>, RouteCompileError> {
    let mut by_id = BTreeMap::new();
    let mut route_for_public_model = BTreeMap::new();
    for route in routes {
        if !public_models.contains_key(&route.public_model_id) {
            return Err(route_error(
                RouteCompileErrorCode::MissingRoutePublicModel,
                route.id.as_str(),
            ));
        }
        if by_id.insert(route.id.clone(), route).is_some() {
            return Err(route_error(
                RouteCompileErrorCode::DuplicateRoute,
                route.id.as_str(),
            ));
        }
        if route_for_public_model
            .insert(route.public_model_id.clone(), route.id.clone())
            .is_some()
        {
            return Err(route_error(
                RouteCompileErrorCode::DuplicateRouteForPublicModel,
                route.public_model_id.as_str(),
            ));
        }
    }
    Ok(RouteIndex {
        by_id,
        route_for_public_model,
    })
}

fn index_candidates<'a>(
    candidates: &'a [RouteCandidateConfiguration],
    routes: &RouteIndex<'_>,
    endpoints: &BTreeMap<EndpointId, &EndpointConfiguration>,
) -> Result<CandidateIndex<'a>, RouteCompileError> {
    let mut by_id = BTreeMap::new();
    let mut by_route: BTreeMap<RouteId, Vec<&RouteCandidateConfiguration>> = BTreeMap::new();
    let mut targets = BTreeSet::new();
    for candidate in candidates {
        if !routes.by_id.contains_key(&candidate.route_id) {
            return Err(route_error(
                RouteCompileErrorCode::MissingCandidateRoute,
                candidate.id.as_str(),
            ));
        }
        if !endpoints.contains_key(&candidate.endpoint_id) {
            return Err(route_error(
                RouteCompileErrorCode::MissingCandidateEndpoint,
                candidate.id.as_str(),
            ));
        }
        parse_candidate_override(candidate)?;
        if by_id.insert(candidate.id.clone(), candidate).is_some() {
            return Err(route_error(
                RouteCompileErrorCode::DuplicateCandidate,
                candidate.id.as_str(),
            ));
        }
        if !targets.insert((
            candidate.route_id.clone(),
            candidate.endpoint_id.clone(),
            candidate.upstream_model.clone(),
        )) {
            return Err(route_error(
                RouteCompileErrorCode::DuplicateCandidateTarget,
                candidate.id.as_str(),
            ));
        }
        by_route
            .entry(candidate.route_id.clone())
            .or_default()
            .push(candidate);
    }
    for route_candidates in by_route.values_mut() {
        route_candidates.sort_by(|left, right| left.id.cmp(&right.id));
    }
    Ok(CandidateIndex { by_id, by_route })
}

fn index_access_groups(
    access_groups: &[AccessGroupConfiguration],
) -> Result<BTreeMap<AccessGroupId, &AccessGroupConfiguration>, RouteCompileError> {
    let mut indexed = BTreeMap::new();
    for access_group in access_groups {
        if indexed
            .insert(access_group.id.clone(), access_group)
            .is_some()
        {
            return Err(route_error(
                RouteCompileErrorCode::DuplicateAccessGroup,
                access_group.id.as_str(),
            ));
        }
    }
    Ok(indexed)
}

fn validate_access_group_routes<'a>(
    access_group_routes: &'a [gateway_store::control_plane::AccessGroupRouteConfiguration],
    access_groups: &BTreeMap<AccessGroupId, &AccessGroupConfiguration>,
    routes: &RouteIndex<'_>,
) -> Result<
    BTreeMap<AccessGroupId, Vec<&'a gateway_store::control_plane::AccessGroupRouteConfiguration>>,
    RouteCompileError,
> {
    let mut indexed = BTreeMap::new();
    for access_group_route in access_group_routes {
        if !access_groups.contains_key(&access_group_route.access_group_id) {
            return Err(route_error(
                RouteCompileErrorCode::MissingAccessGroup,
                access_group_route.access_group_id.as_str(),
            ));
        }
        if !routes.by_id.contains_key(&access_group_route.route_id) {
            return Err(route_error(
                RouteCompileErrorCode::MissingAccessGroupRoute,
                access_group_route.route_id.as_str(),
            ));
        }
        indexed
            .entry(access_group_route.access_group_id.clone())
            .or_insert_with(Vec::new)
            .push(access_group_route);
    }
    Ok(indexed)
}

fn compile_active_routes(
    routes: &RouteIndex<'_>,
    public_models: &BTreeMap<PublicModelId, &PublicModelConfiguration>,
    candidates: &CandidateIndex<'_>,
    compiled_candidates: &BTreeMap<RouteCandidateId, CompiledRouteCandidate>,
) -> Result<BTreeMap<RouteId, CompiledRoute>, RouteCompileError> {
    let mut compiled = BTreeMap::new();
    for route in routes.by_id.values() {
        let public_model = public_models.get(&route.public_model_id).ok_or_else(|| {
            route_error(
                RouteCompileErrorCode::MissingRoutePublicModel,
                route.id.as_str(),
            )
        })?;
        if public_model.status != AdministrativeStatus::Active {
            continue;
        }
        let mut route_candidates: Vec<_> = candidates
            .by_route
            .get(&route.id)
            .into_iter()
            .flatten()
            .filter(|candidate| candidate.enabled)
            .filter_map(|candidate| compiled_candidates.get(&candidate.id).cloned())
            .collect();
        route_candidates.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        if route_candidates.is_empty() {
            return Err(route_error(
                RouteCompileErrorCode::RouteHasNoHardEligibleCandidate,
                route.id.as_str(),
            ));
        }
        compiled.insert(
            route.id.clone(),
            CompiledRoute {
                id: route.id.clone(),
                public_model_id: route.public_model_id.clone(),
                policy: route.policy,
                max_attempts: route.max_attempts,
                bootstrap_timeout_ms: route.bootstrap_timeout_ms,
                candidates: route_candidates,
            },
        );
    }
    Ok(compiled)
}

fn compile_access_groups(
    access_groups: &BTreeMap<AccessGroupId, &AccessGroupConfiguration>,
    access_group_routes: &BTreeMap<
        AccessGroupId,
        Vec<&gateway_store::control_plane::AccessGroupRouteConfiguration>,
    >,
    routes: &BTreeMap<RouteId, CompiledRoute>,
) -> Result<BTreeMap<AccessGroupId, CompiledAccessGroup>, RouteCompileError> {
    let mut compiled = BTreeMap::new();
    for access_group in access_groups.values() {
        if access_group.status != AdministrativeStatus::Active {
            continue;
        }
        let mut allowed_route_ids = BTreeSet::new();
        for access_group_route in access_group_routes
            .get(&access_group.id)
            .into_iter()
            .flatten()
        {
            if !access_group_route.enabled {
                continue;
            }
            if !routes.contains_key(&access_group_route.route_id) {
                return Err(route_error(
                    RouteCompileErrorCode::AccessGroupRouteNotPublishable,
                    access_group_route.route_id.as_str(),
                ));
            }
            allowed_route_ids.insert(access_group_route.route_id.clone());
        }
        compiled.insert(
            access_group.id.clone(),
            CompiledAccessGroup {
                id: access_group.id.clone(),
                name: access_group.name.clone(),
                allowed_route_ids,
            },
        );
    }
    Ok(compiled)
}

fn parse_candidate_override(
    candidate: &RouteCandidateConfiguration,
) -> Result<CandidateOverride, RouteCompileError> {
    parse_capability_object(
        &candidate.capability_override_json,
        candidate.id.as_str(),
        RouteCompileErrorCode::InvalidCandidateCapabilities,
        true,
    )
}

fn parse_capability_object(
    value: &str,
    subject: &str,
    error_code: RouteCompileErrorCode,
    allow_unlisted_model: bool,
) -> Result<CandidateOverride, RouteCompileError> {
    let parsed: Value =
        serde_json::from_str(value).map_err(|_| route_error(error_code, subject))?;
    let object = parsed
        .as_object()
        .ok_or_else(|| route_error(error_code, subject))?;
    let mut asserted = BTreeSet::new();
    let mut removed = BTreeSet::new();
    let mut allows_unlisted = false;
    for (key, value) in object {
        let boolean = value
            .as_bool()
            .ok_or_else(|| route_error(error_code, subject))?;
        if key == "allow_unlisted_model" {
            if !allow_unlisted_model {
                return Err(route_error(error_code, subject));
            }
            allows_unlisted = boolean;
            continue;
        }
        let capability = SemanticCapability::from_json_key(key)
            .ok_or_else(|| route_error(error_code, subject))?;
        if boolean {
            asserted.insert(capability);
        } else {
            removed.insert(capability);
        }
    }
    if asserted.contains(&SemanticCapability::ParallelTools)
        && removed.contains(&SemanticCapability::Tools)
    {
        return Err(route_error(error_code, subject));
    }
    Ok(CandidateOverride {
        asserted,
        removed,
        allow_unlisted_model: allows_unlisted,
    })
}

fn route_error(code: RouteCompileErrorCode, subject: &str) -> RouteCompileError {
    RouteCompileError {
        code,
        subject: subject.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_catalog::{
        CapabilitySet, CatalogModelEntry, CatalogModelState, CatalogView, EndpointCapabilityEntry,
        EndpointCapabilityView, SemanticCapability,
    };
    use gateway_core::{
        AccessGroupId, CredentialId, EndpointId, PublicModelId, RouteCandidateId, RouteId,
        UpstreamId,
    };
    use gateway_store::{
        control_plane::{
            AccessGroupConfiguration, AccessGroupRouteConfiguration, AdministrativeStatus,
            ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialConfiguration, CredentialScope, CredentialStatus, EndpointConfiguration,
            EndpointCredentialBindingConfiguration, EndpointTransport, ModelAliasConfiguration,
            ModelRouteConfiguration, PublicModelConfiguration, RouteCandidateConfiguration,
            RoutePolicy, TransformMode, UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };

    use super::{CatalogAdmission, RouteCompileErrorCode, RouteCompiler};

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn valid_graph_compiles_to_a_secret_free_deterministic_view() -> TestResult {
        let fixture = fixture_with_two_candidates()?;
        let compiled = fixture.compiler().compile(&fixture.configuration)?;
        assert_eq!(
            compiled,
            fixture.compiler().compile(&fixture.configuration)?
        );

        let public_model = compiled
            .public_model("public-model-a")
            .ok_or("compiled public model is missing")?;
        assert_eq!(
            public_model.id(),
            &PublicModelId::try_new("public-model-a")?
        );
        assert!(
            public_model
                .required_capabilities()
                .supports(SemanticCapability::Tools)
        );
        assert_eq!(
            compiled.alias_target("model-a"),
            Some(&PublicModelId::try_new("public-model-a")?)
        );

        let route = compiled
            .route(&RouteId::try_new("route-a")?)
            .ok_or("compiled Route is missing")?;
        assert_eq!(route.candidates().len(), 2);
        assert_eq!(route.candidates()[0].id().as_str(), "candidate-b");
        assert_eq!(route.candidates()[1].id().as_str(), "candidate-a");
        assert_eq!(
            route.candidates()[0].catalog_admission(),
            CatalogAdmission::Listed(CatalogModelState::Stale)
        );
        assert_eq!(route.candidates()[0].active_binding_count(), 1);
        assert_eq!(
            route.candidates()[1].catalog_admission(),
            CatalogAdmission::Listed(CatalogModelState::Fresh)
        );
        assert_eq!(route.candidates()[1].active_binding_count(), 1);
        let access_group = compiled
            .access_group(&AccessGroupId::try_new("access-group-a")?)
            .ok_or("compiled Access Group is missing")?;
        assert!(access_group.permits_route(&RouteId::try_new("route-a")?));

        let debug = format!("{compiled:?}");
        assert!(!debug.contains("synthetic-credential"));
        assert!(!debug.contains("ciphertext"));
        Ok(())
    }

    #[test]
    fn conflict_matrix_returns_stable_codes() -> TestResult {
        let snapshot = [
            (
                "duplicate_public_model_name",
                duplicate_public_model_name_error()?.as_str(),
            ),
            ("duplicate_alias", duplicate_alias_error()?.as_str()),
            ("alias_namespace", alias_namespace_error()?.as_str()),
            (
                "missing_alias_target",
                missing_alias_target_error()?.as_str(),
            ),
            (
                "missing_route_target",
                missing_route_target_error()?.as_str(),
            ),
            (
                "missing_candidate_target",
                missing_candidate_target_error()?.as_str(),
            ),
            (
                "missing_access_group",
                missing_access_group_error()?.as_str(),
            ),
            (
                "duplicate_endpoint_format",
                duplicate_endpoint_format_error()?.as_str(),
            ),
            ("catalog_missing", catalog_missing_error()?.as_str()),
            (
                "missing_endpoint_capability",
                missing_endpoint_capability_error()?.as_str(),
            ),
            (
                "malformed_public_capabilities",
                malformed_public_capabilities_error()?.as_str(),
            ),
            (
                "capability_escalation",
                capability_escalation_error()?.as_str(),
            ),
            (
                "capability_narrowing",
                capability_narrowing_error()?.as_str(),
            ),
            ("no_active_binding", no_active_binding_error()?.as_str()),
            ("disabled_endpoint", disabled_endpoint_error()?.as_str()),
            ("disabled_upstream", disabled_upstream_error()?.as_str()),
            (
                "route_without_candidate",
                route_without_candidate_error()?.as_str(),
            ),
            ("unpublishable_grant", unpublishable_grant_error()?.as_str()),
        ];
        assert_eq!(
            snapshot,
            [
                ("duplicate_public_model_name", "duplicate_public_model_name",),
                ("duplicate_alias", "duplicate_alias"),
                ("alias_namespace", "alias_conflicts_public_model"),
                ("missing_alias_target", "missing_alias_public_model",),
                ("missing_route_target", "missing_route_public_model",),
                ("missing_candidate_target", "missing_candidate_endpoint",),
                ("missing_access_group", "missing_access_group"),
                ("duplicate_endpoint_format", "duplicate_endpoint_api_format",),
                ("catalog_missing", "catalog_model_not_eligible"),
                (
                    "missing_endpoint_capability",
                    "missing_endpoint_capability_profile",
                ),
                (
                    "malformed_public_capabilities",
                    "invalid_public_model_capabilities",
                ),
                ("capability_escalation", "candidate_capability_escalation",),
                ("capability_narrowing", "candidate_capability_mismatch",),
                ("no_active_binding", "missing_active_credential_binding",),
                ("disabled_endpoint", "active_candidate_disabled_endpoint",),
                ("disabled_upstream", "active_candidate_disabled_upstream",),
                (
                    "route_without_candidate",
                    "route_has_no_hard_eligible_candidate",
                ),
                ("unpublishable_grant", "access_group_route_not_publishable",),
            ]
        );
        Ok(())
    }

    #[test]
    fn catalog_expiry_requires_an_explicit_candidate_exception() -> TestResult {
        let mut fixture = fixture()?;
        fixture.catalog = catalog(CatalogModelState::Expired)?;
        assert_eq!(
            compile_error_code(&fixture)?,
            RouteCompileErrorCode::CatalogModelNotEligible
        );

        fixture.configuration.route_candidates[0].capability_override_json =
            r#"{"allow_unlisted_model":true}"#.to_owned();
        let compiled = fixture.compiler().compile(&fixture.configuration)?;
        let route = compiled
            .route(&RouteId::try_new("route-a")?)
            .ok_or("compiled Route is missing")?;
        assert_eq!(
            route.candidates()[0].catalog_admission(),
            CatalogAdmission::AllowedUnlisted
        );
        Ok(())
    }

    #[test]
    fn manual_fresh_and_stale_catalog_entries_are_hard_eligible() -> TestResult {
        for state in [
            CatalogModelState::Manual,
            CatalogModelState::Fresh,
            CatalogModelState::Stale,
        ] {
            let mut fixture = fixture()?;
            fixture.catalog = catalog(state)?;
            let compiled = fixture.compiler().compile(&fixture.configuration)?;
            let route = compiled
                .route(&RouteId::try_new("route-a")?)
                .ok_or("compiled Route is missing")?;
            assert_eq!(
                route.candidates()[0].catalog_admission(),
                CatalogAdmission::Listed(state)
            );
        }
        Ok(())
    }

    #[test]
    fn candidate_can_confirm_and_narrow_endpoint_capabilities() -> TestResult {
        let mut fixture = fixture()?;
        fixture.configuration.route_candidates[0].capability_override_json =
            r#"{"tools":true,"reasoning":false}"#.to_owned();

        let compiled = fixture.compiler().compile(&fixture.configuration)?;
        let route = compiled
            .route(&RouteId::try_new("route-a")?)
            .ok_or("compiled Route is missing")?;
        let capabilities = route.candidates()[0].effective_capabilities();
        assert!(capabilities.supports(SemanticCapability::Tools));
        assert!(!capabilities.supports(SemanticCapability::Reasoning));
        Ok(())
    }

    #[test]
    fn malformed_disabled_candidate_capabilities_are_not_silently_ignored() -> TestResult {
        let mut fixture = fixture()?;
        fixture.configuration.route_candidates[0].enabled = false;
        fixture.configuration.route_candidates[0].capability_override_json =
            r#"{"unknown":true}"#.to_owned();
        assert_eq!(
            compile_error_code(&fixture)?,
            RouteCompileErrorCode::InvalidCandidateCapabilities
        );
        Ok(())
    }

    #[test]
    fn disabled_public_model_is_structurally_checked_but_not_hard_eligible() -> TestResult {
        let mut fixture = fixture()?;
        fixture.configuration.public_models[0].status = AdministrativeStatus::Disabled;
        fixture.configuration.credentials[0].status = CredentialStatus::Disabled;
        fixture.configuration.access_group_routes[0].enabled = false;
        fixture.catalog = CatalogView::default();

        let compiled = fixture.compiler().compile(&fixture.configuration)?;
        assert!(compiled.public_model("public-model-a").is_none());
        assert!(compiled.alias_target("model-a").is_none());
        assert!(compiled.route(&RouteId::try_new("route-a")?).is_none());
        Ok(())
    }

    struct Fixture {
        configuration: ControlPlaneConfiguration,
        catalog: CatalogView,
        endpoint_capabilities: EndpointCapabilityView,
    }

    impl Fixture {
        fn compiler(&self) -> RouteCompiler {
            RouteCompiler::new(self.catalog.clone(), self.endpoint_capabilities.clone())
        }
    }

    fn fixture() -> Result<Fixture, Box<dyn Error>> {
        Ok(Fixture {
            configuration: configuration()?,
            catalog: catalog(CatalogModelState::Fresh)?,
            endpoint_capabilities: endpoint_capabilities(&[
                SemanticCapability::Tools,
                SemanticCapability::ParallelTools,
                SemanticCapability::Reasoning,
                SemanticCapability::Streaming,
            ])?,
        })
    }

    fn fixture_with_two_candidates() -> Result<Fixture, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.route_candidates[0].priority = 10;
        fixture.configuration.upstreams.push(UpstreamConfiguration {
            id: UpstreamId::try_new("upstream-b")?,
            name: "station-b".to_owned(),
            kind: "openai-compatible".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: None,
        });
        fixture.configuration.endpoints.push(EndpointConfiguration {
            id: EndpointId::try_new("endpoint-b")?,
            upstream_id: UpstreamId::try_new("upstream-b")?,
            adapter_id: "openai-compatible.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: "https://station-b.example/v1".to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: Some("/models".to_owned()),
            transport: EndpointTransport::Http,
            enabled: true,
        });
        fixture
            .configuration
            .credentials
            .push(CredentialConfiguration {
                id: CredentialId::try_new("credential-b")?,
                upstream_id: UpstreamId::try_new("upstream-b")?,
                kind: "api_key".to_owned(),
                encrypted_secret: encrypted_fixture_secret()?,
                status: CredentialStatus::Active,
                revision: 0,
            });
        fixture.configuration.endpoint_credential_bindings.push(
            EndpointCredentialBindingConfiguration {
                endpoint_id: EndpointId::try_new("endpoint-b")?,
                credential_id: CredentialId::try_new("credential-b")?,
                upstream_id: UpstreamId::try_new("upstream-b")?,
                enabled: true,
                priority: 0,
                weight: 1,
                concurrency: 2,
            },
        );
        fixture
            .configuration
            .route_candidates
            .push(RouteCandidateConfiguration {
                id: RouteCandidateId::try_new("candidate-b")?,
                route_id: RouteId::try_new("route-a")?,
                endpoint_id: EndpointId::try_new("endpoint-b")?,
                upstream_model: "upstream-model-b".to_owned(),
                credential_scope: CredentialScope::EndpointBindings,
                transform_mode: TransformMode::Canonical,
                enabled: true,
                priority: 0,
                weight: 50,
                capability_override_json: "{}".to_owned(),
            });
        fixture.catalog = CatalogView::try_new([
            CatalogModelEntry::try_new(
                EndpointId::try_new("endpoint-a")?,
                "upstream-model-a",
                CatalogModelState::Fresh,
            )?,
            CatalogModelEntry::try_new(
                EndpointId::try_new("endpoint-b")?,
                "upstream-model-b",
                CatalogModelState::Stale,
            )?,
        ])?;
        fixture.endpoint_capabilities = EndpointCapabilityView::try_new([
            EndpointCapabilityEntry {
                endpoint_id: EndpointId::try_new("endpoint-a")?,
                capabilities: CapabilitySet::try_new([
                    SemanticCapability::Tools,
                    SemanticCapability::ParallelTools,
                    SemanticCapability::Reasoning,
                    SemanticCapability::Streaming,
                ])?,
            },
            EndpointCapabilityEntry {
                endpoint_id: EndpointId::try_new("endpoint-b")?,
                capabilities: CapabilitySet::try_new([
                    SemanticCapability::Tools,
                    SemanticCapability::Streaming,
                ])?,
            },
        ])?;
        Ok(fixture)
    }

    fn configuration() -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
        let version_id = ConfigVersionId::try_new("version-a")?;
        let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
            id: version_id,
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            created_at_ms: 1,
            description: "compiler fixture".to_owned(),
        });
        add_upstream_tree(&mut configuration)?;
        add_public_route_tree(&mut configuration)?;
        Ok(configuration)
    }

    fn add_upstream_tree(configuration: &mut ControlPlaneConfiguration) -> TestResult {
        configuration.upstreams.push(UpstreamConfiguration {
            id: UpstreamId::try_new("upstream-a")?,
            name: "station-a".to_owned(),
            kind: "openai-compatible".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: None,
        });
        configuration.endpoints.push(EndpointConfiguration {
            id: EndpointId::try_new("endpoint-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            adapter_id: "openai-compatible.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: "https://station.example/v1".to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: Some("/models".to_owned()),
            transport: EndpointTransport::Http,
            enabled: true,
        });
        configuration.credentials.push(CredentialConfiguration {
            id: CredentialId::try_new("credential-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            kind: "api_key".to_owned(),
            encrypted_secret: encrypted_fixture_secret()?,
            status: CredentialStatus::Active,
            revision: 0,
        });
        configuration
            .endpoint_credential_bindings
            .push(EndpointCredentialBindingConfiguration {
                endpoint_id: EndpointId::try_new("endpoint-a")?,
                credential_id: CredentialId::try_new("credential-a")?,
                upstream_id: UpstreamId::try_new("upstream-a")?,
                enabled: true,
                priority: 0,
                weight: 1,
                concurrency: 2,
            });
        Ok(())
    }

    fn add_public_route_tree(configuration: &mut ControlPlaneConfiguration) -> TestResult {
        configuration.public_models.push(PublicModelConfiguration {
            id: PublicModelId::try_new("public-model-a")?,
            model_name: "public-model-a".to_owned(),
            status: AdministrativeStatus::Active,
            display_name: "Public Model A".to_owned(),
            capabilities_json: r#"{"tools":true,"streaming":true}"#.to_owned(),
        });
        configuration.model_aliases.push(ModelAliasConfiguration {
            alias: "model-a".to_owned(),
            public_model_id: PublicModelId::try_new("public-model-a")?,
        });
        configuration.model_routes.push(ModelRouteConfiguration {
            id: RouteId::try_new("route-a")?,
            public_model_id: PublicModelId::try_new("public-model-a")?,
            policy: RoutePolicy::SmoothWeightedRoundRobin,
            max_attempts: 3,
            bootstrap_timeout_ms: 20_000,
        });
        configuration
            .route_candidates
            .push(RouteCandidateConfiguration {
                id: RouteCandidateId::try_new("candidate-a")?,
                route_id: RouteId::try_new("route-a")?,
                endpoint_id: EndpointId::try_new("endpoint-a")?,
                upstream_model: "upstream-model-a".to_owned(),
                credential_scope: CredentialScope::EndpointBindings,
                transform_mode: TransformMode::Canonical,
                enabled: true,
                priority: 0,
                weight: 100,
                capability_override_json: "{}".to_owned(),
            });
        configuration.access_groups.push(AccessGroupConfiguration {
            id: AccessGroupId::try_new("access-group-a")?,
            name: "default".to_owned(),
            status: AdministrativeStatus::Active,
            limits_json: "{}".to_owned(),
        });
        configuration
            .access_group_routes
            .push(AccessGroupRouteConfiguration {
                access_group_id: AccessGroupId::try_new("access-group-a")?,
                route_id: RouteId::try_new("route-a")?,
                enabled: true,
            });
        Ok(())
    }

    fn encrypted_fixture_secret()
    -> Result<gateway_store::secret_store::EncryptedSecret, Box<dyn Error>> {
        let key_version = KeyVersion::try_new(1)?;
        let secret_store = SecretStore::new(MasterKeyRing::try_new(
            key_version,
            [(key_version, MasterKey::try_from_bytes([0x44_u8; 32])?)],
        )?);
        Ok(secret_store.seal(b"synthetic-credential", b"compiler-fixture-aad")?)
    }

    fn catalog(state: CatalogModelState) -> Result<CatalogView, Box<dyn Error>> {
        Ok(CatalogView::try_new([CatalogModelEntry::try_new(
            EndpointId::try_new("endpoint-a")?,
            "upstream-model-a",
            state,
        )?])?)
    }

    fn endpoint_capabilities(
        capabilities: &[SemanticCapability],
    ) -> Result<EndpointCapabilityView, Box<dyn Error>> {
        Ok(EndpointCapabilityView::try_new([
            EndpointCapabilityEntry {
                endpoint_id: EndpointId::try_new("endpoint-a")?,
                capabilities: CapabilitySet::try_new(capabilities.iter().copied())?,
            },
        ])?)
    }

    fn compile_error_code(fixture: &Fixture) -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        match fixture.compiler().compile(&fixture.configuration) {
            Ok(_) => Err("compiler unexpectedly succeeded".into()),
            Err(error) => Ok(error.code()),
        }
    }

    fn alias_namespace_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.model_aliases[0].alias = "public-model-a".to_owned();
        compile_error_code(&fixture)
    }

    fn duplicate_public_model_name_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        let mut duplicate = fixture.configuration.public_models[0].clone();
        duplicate.id = PublicModelId::try_new("public-model-b")?;
        fixture.configuration.public_models.push(duplicate);
        compile_error_code(&fixture)
    }

    fn duplicate_alias_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture
            .configuration
            .model_aliases
            .push(fixture.configuration.model_aliases[0].clone());
        compile_error_code(&fixture)
    }

    fn missing_alias_target_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.model_aliases[0].public_model_id =
            PublicModelId::try_new("missing-public-model")?;
        compile_error_code(&fixture)
    }

    fn missing_route_target_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.model_routes[0].public_model_id =
            PublicModelId::try_new("missing-public-model")?;
        compile_error_code(&fixture)
    }

    fn missing_candidate_target_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.route_candidates[0].endpoint_id =
            EndpointId::try_new("missing-endpoint")?;
        compile_error_code(&fixture)
    }

    fn missing_access_group_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.access_group_routes[0].access_group_id =
            AccessGroupId::try_new("missing-access-group")?;
        compile_error_code(&fixture)
    }

    fn duplicate_endpoint_format_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.endpoints.push(EndpointConfiguration {
            id: EndpointId::try_new("endpoint-b")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            adapter_id: "other-adapter".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: "https://station.example/other".to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: None,
            transport: EndpointTransport::Http,
            enabled: true,
        });
        compile_error_code(&fixture)
    }

    fn catalog_missing_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.catalog = CatalogView::default();
        compile_error_code(&fixture)
    }

    fn missing_endpoint_capability_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.endpoint_capabilities = EndpointCapabilityView::default();
        compile_error_code(&fixture)
    }

    fn malformed_public_capabilities_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.public_models[0].capabilities_json =
            r#"{"parallel_tools":true}"#.to_owned();
        compile_error_code(&fixture)
    }

    fn capability_escalation_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.route_candidates[0].capability_override_json =
            r#"{"vision":true}"#.to_owned();
        compile_error_code(&fixture)
    }

    fn capability_narrowing_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.route_candidates[0].capability_override_json =
            r#"{"tools":false}"#.to_owned();
        compile_error_code(&fixture)
    }

    fn no_active_binding_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.credentials[0].status = CredentialStatus::Disabled;
        compile_error_code(&fixture)
    }

    fn disabled_endpoint_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.endpoints[0].enabled = false;
        compile_error_code(&fixture)
    }

    fn disabled_upstream_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.upstreams[0].enabled = false;
        compile_error_code(&fixture)
    }

    fn route_without_candidate_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.route_candidates[0].enabled = false;
        compile_error_code(&fixture)
    }

    fn unpublishable_grant_error() -> Result<RouteCompileErrorCode, Box<dyn Error>> {
        let mut fixture = fixture()?;
        fixture.configuration.public_models[0].status = AdministrativeStatus::Disabled;
        compile_error_code(&fixture)
    }
}
