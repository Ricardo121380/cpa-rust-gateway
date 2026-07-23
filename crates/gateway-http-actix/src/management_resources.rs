//! Protected versioned management resource handlers for P10-04 and P10-05.
//!
//! These handlers decode only bounded explicit resource shapes and delegate every durable graph
//! mutation to `gateway-control`. They never publish a Snapshot, invoke a Provider, expose a
//! credential Secret/ciphertext, or bypass the P10-02 `/admin` security scope.

use std::{collections::BTreeMap, sync::Mutex};

use actix_web::{
    HttpMessage, HttpRequest, HttpResponse,
    http::{StatusCode, header},
    web,
};
use gateway_control::management_mutation_service::{
    AccessGroupConfiguration, AccessGroupRouteConfiguration, AdministrativeStatus, ClientKeyIssue,
    ClientKeyUpdate, ClientKeyView, ConfigRevision, ConfigVersionId, CredentialScope,
    CredentialStatus, CredentialUpsert, CredentialView, EgressPolicyConfiguration,
    EndpointConfiguration, EndpointCredentialBindingConfiguration, EndpointTransport,
    ManagementMutationService, ManagementResourceError, ManagementRouteValidation,
    ModelAliasConfiguration, ModelRouteConfiguration, PublicModelConfiguration, Revisioned,
    RouteCandidateConfiguration, RoutePolicy, StoreError, StoredClientKeyStatus,
    StoredEgressRedirectMode, TransformMode, UpstreamConfiguration,
};
use gateway_core::{
    AccessGroupId, ClientKeyId, CredentialId, EgressPolicyId, EndpointId, PublicModelId,
    RouteCandidateId, RouteId, UpstreamId,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use zeroize::Zeroizing;

use crate::management_security::{ManagementRequestPrincipal, configure_management};

const CONFIG_VERSION_HEADER: &str = "x-config-version";
const IF_MATCH_HEADER: &str = "if-match";
const MAX_MANAGEMENT_JSON_BYTES: usize = 70 * 1024;

/// Management-time application state for P10-04 resource handlers.
///
/// The owned `SQLite` service stays behind a synchronous mutex because its mutations are short,
/// serialized transactions. Provider and OAuth workflows remain separately injected in later
/// P10-04 code and never run while this lock is held.
pub struct ManagementResourceHttpState {
    service: Mutex<ManagementMutationService>,
    workflow: Mutex<Box<dyn ManagementEndpointWorkflow>>,
}

impl ManagementResourceHttpState {
    /// Creates protected resource-handler state from the management-only service.
    #[must_use]
    pub fn new(service: ManagementMutationService) -> Self {
        Self::with_workflow(
            service,
            Box::new(RejectingManagementEndpointWorkflow::new()),
        )
    }

    /// Creates the state with an explicit bounded Endpoint workflow implementation.
    #[must_use]
    pub fn with_workflow(
        service: ManagementMutationService,
        workflow: Box<dyn ManagementEndpointWorkflow>,
    ) -> Self {
        Self {
            service: Mutex::new(service),
            workflow: Mutex::new(workflow),
        }
    }
}

/// The only modes accepted by a bounded Endpoint test request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementEndpointTestMode {
    /// One finite non-streaming probe.
    NonStreaming,
    /// One finite SSE probe.
    Sse,
}

/// Safe classification returned by an injected Endpoint test workflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementEndpointTestOutcome {
    /// The bounded workflow observed a complete valid Canonical lifecycle.
    Pass,
    /// No sendable runtime was configured for the selected Endpoint.
    Rejected,
    /// A transport boundary failed before a valid Provider response.
    TransportFailed,
    /// A response failed protocol or Canonical lifecycle validation.
    ProtocolFailed,
}

/// Safe status bucket for a bounded Endpoint test workflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementEndpointStatusClass {
    /// A success-class HTTP response.
    TwoXx,
    /// A client-failure HTTP response.
    FourXx,
    /// A server-failure HTTP response.
    FiveXx,
    /// No usable HTTP status class was observed.
    Other,
}

/// Value-free result of one bounded Endpoint test.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagementEndpointTestResult {
    /// Test conclusion.
    pub outcome: ManagementEndpointTestOutcome,
    /// Observed safe status class.
    pub status_class: ManagementEndpointStatusClass,
    /// Whether a complete Canonical response lifecycle was observed.
    pub canonical_lifecycle: bool,
}

/// Non-secret summary of a Catalog discovery operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagementCatalogDiff {
    /// Number of models added to the exact Endpoint discovery target.
    pub added: u64,
    /// Number of models removed from the exact Endpoint discovery target.
    pub removed: u64,
    /// Number of unchanged models.
    pub unchanged: u64,
}

/// Current state of a bounded Credential OAuth operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementCredentialOAuthState {
    /// An explicit OAuth workflow is pending completion.
    Pending,
    /// The injected workflow completed successfully.
    Complete,
    /// The workflow was explicitly cancelled.
    Cancelled,
    /// The workflow ended with a safe failure classification.
    Failed,
}

/// Value-free OAuth operation view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagementCredentialOAuthOperation {
    /// Current OAuth workflow state.
    pub state: ManagementCredentialOAuthState,
    /// Optional finite expiry time; no token, URL, or verification material is included.
    pub expires_at_ms: Option<i64>,
}

/// Explicit seam for P10-04's bounded test, Catalog, and OAuth operations.
///
/// The seam receives only stable resource identifiers. A production implementation must own its
/// own admitted Endpoint/Credential runtime handles; it must not derive a URL, Secret, Cookie or
/// arbitrary outbound request from the management HTTP body.
pub trait ManagementEndpointWorkflow: Send {
    /// Performs at most the implementation's declared bounded Endpoint test.
    fn test_endpoint(
        &mut self,
        endpoint_id: &EndpointId,
        mode: ManagementEndpointTestMode,
    ) -> ManagementEndpointTestResult;

    /// Returns a value-free Catalog preview for one exact Endpoint.
    fn preview_catalog(&mut self, endpoint_id: &EndpointId) -> ManagementCatalogDiff;

    /// Applies a previously supported Catalog action for one exact Endpoint.
    fn apply_catalog(&mut self, endpoint_id: &EndpointId) -> ManagementCatalogDiff;

    /// Starts an explicit Credential-local OAuth workflow.
    fn start_oauth(&mut self, credential_id: &CredentialId) -> ManagementCredentialOAuthOperation;

    /// Returns a Credential-local OAuth state without exposing protocol material.
    fn oauth_status(&mut self, credential_id: &CredentialId) -> ManagementCredentialOAuthOperation;

    /// Cancels a Credential-local OAuth workflow.
    fn cancel_oauth(&mut self, credential_id: &CredentialId);
}

/// Default fail-closed P10-04 workflow. It never contacts a Provider or creates OAuth material.
pub struct RejectingManagementEndpointWorkflow {
    oauth: BTreeMap<CredentialId, ManagementCredentialOAuthOperation>,
}

impl RejectingManagementEndpointWorkflow {
    /// Creates an empty no-send workflow.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            oauth: BTreeMap::new(),
        }
    }
}

impl Default for RejectingManagementEndpointWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagementEndpointWorkflow for RejectingManagementEndpointWorkflow {
    fn test_endpoint(
        &mut self,
        _endpoint_id: &EndpointId,
        _mode: ManagementEndpointTestMode,
    ) -> ManagementEndpointTestResult {
        ManagementEndpointTestResult {
            outcome: ManagementEndpointTestOutcome::Rejected,
            status_class: ManagementEndpointStatusClass::Other,
            canonical_lifecycle: false,
        }
    }

    fn preview_catalog(&mut self, _endpoint_id: &EndpointId) -> ManagementCatalogDiff {
        ManagementCatalogDiff {
            added: 0,
            removed: 0,
            unchanged: 0,
        }
    }

    fn apply_catalog(&mut self, endpoint_id: &EndpointId) -> ManagementCatalogDiff {
        self.preview_catalog(endpoint_id)
    }

    fn start_oauth(&mut self, credential_id: &CredentialId) -> ManagementCredentialOAuthOperation {
        let operation = ManagementCredentialOAuthOperation {
            state: ManagementCredentialOAuthState::Pending,
            expires_at_ms: None,
        };
        self.oauth.insert(credential_id.clone(), operation);
        operation
    }

    fn oauth_status(&mut self, credential_id: &CredentialId) -> ManagementCredentialOAuthOperation {
        self.oauth
            .get(credential_id)
            .copied()
            .unwrap_or(ManagementCredentialOAuthOperation {
                state: ManagementCredentialOAuthState::Failed,
                expires_at_ms: None,
            })
    }

    fn cancel_oauth(&mut self, credential_id: &CredentialId) {
        self.oauth.insert(
            credential_id.clone(),
            ManagementCredentialOAuthOperation {
                state: ManagementCredentialOAuthState::Cancelled,
                expires_at_ms: None,
            },
        );
    }
}

/// Mounts P10-04 resource routes inside the P10-02 protected `/admin` scope.
pub fn configure_management_resources(config: &mut web::ServiceConfig) {
    configure_management(config, resource_routes);
}

fn resource_routes(config: &mut web::ServiceConfig) {
    configure_upstream_resource_routes(config);
    configure_routing_resource_routes(config);
}

fn configure_upstream_resource_routes(config: &mut web::ServiceConfig) {
    config
        .route("/egress-policies", web::get().to(list_egress_policies))
        .route("/egress-policies", web::post().to(create_egress_policy))
        .route(
            "/egress-policies/{egress_policy_id}",
            web::get().to(get_egress_policy),
        )
        .route(
            "/egress-policies/{egress_policy_id}",
            web::patch().to(update_egress_policy),
        )
        .route(
            "/egress-policies/{egress_policy_id}",
            web::delete().to(delete_egress_policy),
        )
        .route("/upstreams", web::get().to(list_upstreams))
        .route("/upstreams", web::post().to(create_upstream))
        .route("/upstreams/{upstream_id}", web::get().to(get_upstream))
        .route("/upstreams/{upstream_id}", web::patch().to(update_upstream))
        .route(
            "/upstreams/{upstream_id}",
            web::delete().to(delete_upstream),
        )
        .route(
            "/upstreams/{upstream_id}/endpoints",
            web::post().to(create_endpoint),
        )
        .route("/endpoints/{endpoint_id}", web::get().to(get_endpoint))
        .route("/endpoints/{endpoint_id}", web::patch().to(update_endpoint))
        .route(
            "/endpoints/{endpoint_id}",
            web::delete().to(delete_endpoint),
        )
        .route(
            "/endpoints/{endpoint_id}/test",
            web::post().to(test_endpoint),
        )
        .route(
            "/endpoints/{endpoint_id}/models/discover-preview",
            web::post().to(preview_catalog_discovery),
        )
        .route(
            "/endpoints/{endpoint_id}/models/discover-apply",
            web::post().to(apply_catalog_discovery),
        )
        .route(
            "/upstreams/{upstream_id}/credentials",
            web::post().to(create_credential),
        )
        .route(
            "/credentials/{credential_id}",
            web::get().to(get_credential),
        )
        .route(
            "/credentials/{credential_id}",
            web::patch().to(update_credential),
        )
        .route(
            "/credentials/{credential_id}",
            web::delete().to(delete_credential),
        )
        .route(
            "/credentials/{credential_id}/oauth/start",
            web::post().to(start_credential_oauth),
        )
        .route(
            "/credentials/{credential_id}/oauth/status",
            web::get().to(get_credential_oauth_status),
        )
        .route(
            "/credentials/{credential_id}/oauth/cancel",
            web::post().to(cancel_credential_oauth),
        )
        .route(
            "/endpoints/{endpoint_id}/credential-bindings",
            web::get().to(list_endpoint_credential_bindings),
        )
        .route(
            "/endpoints/{endpoint_id}/credential-bindings",
            web::post().to(create_endpoint_credential_binding),
        );
}

fn configure_routing_resource_routes(config: &mut web::ServiceConfig) {
    config
        .route("/public-models", web::get().to(list_public_models))
        .route("/public-models", web::post().to(create_public_model))
        .route(
            "/public-models/{public_model_id}",
            web::get().to(get_public_model),
        )
        .route(
            "/public-models/{public_model_id}",
            web::patch().to(update_public_model),
        )
        .route(
            "/public-models/{public_model_id}",
            web::delete().to(delete_public_model),
        )
        .route(
            "/public-models/{public_model_id}/aliases",
            web::post().to(create_model_alias),
        )
        .route(
            "/public-models/{public_model_id}/routes",
            web::post().to(create_model_route),
        )
        .route("/routes/{route_id}", web::get().to(get_model_route))
        .route("/routes/{route_id}", web::patch().to(update_model_route))
        .route("/routes/{route_id}", web::delete().to(delete_model_route))
        .route(
            "/routes/{route_id}/candidates",
            web::post().to(create_route_candidate),
        )
        .route(
            "/routes/{route_id}/validate",
            web::post().to(validate_model_route),
        )
        .route("/access-groups", web::get().to(list_access_groups))
        .route("/access-groups", web::post().to(create_access_group))
        .route(
            "/access-groups/{access_group_id}",
            web::get().to(get_access_group),
        )
        .route(
            "/access-groups/{access_group_id}",
            web::patch().to(update_access_group),
        )
        .route(
            "/access-groups/{access_group_id}",
            web::delete().to(delete_access_group),
        )
        .route(
            "/access-groups/{access_group_id}/routes",
            web::get().to(list_access_group_routes),
        )
        .route(
            "/access-groups/{access_group_id}/routes",
            web::post().to(create_access_group_route),
        )
        .route("/client-keys", web::get().to(list_client_keys))
        .route("/client-keys", web::post().to(issue_client_key))
        .route(
            "/client-keys/{client_key_id}",
            web::get().to(get_client_key),
        )
        .route(
            "/client-keys/{client_key_id}",
            web::patch().to(update_client_key),
        )
        .route(
            "/client-keys/{client_key_id}",
            web::delete().to(revoke_client_key),
        );
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EgressPolicyInput {
    id: String,
    name: String,
    allowed_schemes: Vec<String>,
    allowed_hosts: Vec<String>,
    allowed_ports: Vec<i64>,
    allowed_cidrs: Vec<String>,
    redirect_mode: String,
    max_redirects: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamInput {
    id: String,
    name: String,
    kind: String,
    enabled: bool,
    tags: Vec<String>,
    egress_policy_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndpointInput {
    id: String,
    adapter_id: String,
    api_format: String,
    base_url: String,
    inference_path: String,
    models_path: Option<String>,
    transport: String,
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialInput {
    id: String,
    kind: String,
    secret: String,
    status: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingInput {
    credential_id: String,
    enabled: bool,
    priority: i64,
    weight: i64,
    concurrency: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndpointTestInput {
    mode: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicModelInput {
    id: String,
    model_name: String,
    status: String,
    display_name: String,
    capabilities: BTreeMap<String, bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AliasInput {
    alias: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteInput {
    id: String,
    policy: String,
    max_attempts: i64,
    bootstrap_timeout_ms: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateInput {
    id: String,
    endpoint_id: String,
    upstream_model: String,
    credential_scope: String,
    transform_mode: String,
    enabled: bool,
    priority: i64,
    weight: i64,
    capability_override: BTreeMap<String, bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccessGroupInput {
    id: String,
    name: String,
    status: String,
    limits: BTreeMap<String, i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccessGroupRouteInput {
    route_id: String,
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientKeyInput {
    id: String,
    access_group_id: String,
    status: String,
    expires_at_ms: Option<i64>,
}

#[derive(Serialize)]
struct EgressPolicyResponse {
    id: String,
    name: String,
    allowed_schemes: Vec<String>,
    allowed_hosts: Vec<String>,
    allowed_ports: Vec<i64>,
    allowed_cidrs: Vec<String>,
    redirect_mode: &'static str,
    max_redirects: i64,
}

#[derive(Serialize)]
struct UpstreamResponse {
    id: String,
    name: String,
    kind: String,
    enabled: bool,
    tags: Vec<String>,
    egress_policy_id: Option<String>,
}

#[derive(Serialize)]
struct EndpointResponse {
    id: String,
    upstream_id: String,
    adapter_id: String,
    api_format: String,
    base_url: String,
    inference_path: String,
    models_path: Option<String>,
    transport: &'static str,
    enabled: bool,
}

#[derive(Serialize)]
struct CredentialResponse {
    id: String,
    upstream_id: String,
    kind: String,
    status: &'static str,
    revision: i64,
    secret_present: bool,
}

#[derive(Serialize)]
struct BindingResponse {
    endpoint_id: String,
    upstream_id: String,
    credential_id: String,
    enabled: bool,
    priority: i64,
    weight: i64,
    concurrency: i64,
}

#[derive(Serialize)]
struct EndpointTestResponse {
    outcome: &'static str,
    status_class: &'static str,
    canonical_lifecycle: bool,
}

#[derive(Serialize)]
struct CatalogDiffResponse {
    added: u64,
    removed: u64,
    unchanged: u64,
}

#[derive(Serialize)]
struct CredentialOAuthResponse {
    credential_id: String,
    state: &'static str,
    expires_at_ms: Option<i64>,
}

#[derive(Serialize)]
struct PublicModelResponse {
    id: String,
    model_name: String,
    status: &'static str,
    display_name: String,
    capabilities: BTreeMap<String, bool>,
}

#[derive(Serialize)]
struct AliasResponse {
    alias: String,
    public_model_id: String,
}

#[derive(Serialize)]
struct RouteResponse {
    id: String,
    public_model_id: String,
    policy: &'static str,
    max_attempts: i64,
    bootstrap_timeout_ms: i64,
}

#[derive(Serialize)]
struct CandidateResponse {
    id: String,
    route_id: String,
    endpoint_id: String,
    upstream_model: String,
    credential_scope: &'static str,
    transform_mode: &'static str,
    enabled: bool,
    priority: i64,
    weight: i64,
    capability_override: BTreeMap<String, bool>,
}

#[derive(Serialize)]
struct AccessGroupResponse {
    id: String,
    name: String,
    status: &'static str,
    limits: BTreeMap<String, i64>,
}

#[derive(Serialize)]
struct AccessGroupRouteResponse {
    access_group_id: String,
    route_id: String,
    enabled: bool,
}

#[derive(Serialize)]
struct ClientKeyResponse {
    id: String,
    access_group_id: String,
    prefix: String,
    status: &'static str,
    expires_at_ms: Option<i64>,
}

#[derive(Serialize)]
struct IssuedClientKeyResponse {
    id: String,
    access_group_id: String,
    prefix: String,
    status: &'static str,
    expires_at_ms: Option<i64>,
    key: String,
}

#[derive(Serialize)]
struct ValidationResponse {
    valid: bool,
    error_codes: Vec<&'static str>,
}

async fn list_egress_policies(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.list_egress_policies(&context.version) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |policies| {
            policies
                .into_iter()
                .map(EgressPolicyResponse::from)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn create_egress_policy(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input: EgressPolicyInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let policy = match egress_policy(input) {
        Ok(policy) => policy,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.create_egress_policy(&actor, &context.version, context.revision, policy) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, |policy| {
            EgressPolicyResponse::from(policy)
        }),
        Err(error) => management_error(error),
    }
}

async fn get_egress_policy(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = EgressPolicyId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_egress_policy(&context.version, &id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |policy| {
            EgressPolicyResponse::from(policy)
        }),
        Err(error) => management_error(error),
    }
}

async fn update_egress_policy(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let path_id = path.into_inner();
    let input: EgressPolicyInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    if input.id != path_id {
        return invalid_input();
    }
    let policy = match egress_policy(input) {
        Ok(policy) => policy,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.update_egress_policy(&actor, &context.version, context.revision, policy) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |policy| {
            EgressPolicyResponse::from(policy)
        }),
        Err(error) => management_error(error),
    }
}

async fn delete_egress_policy(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = EgressPolicyId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.delete_egress_policy(&actor, &context.version, context.revision, &id) {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

async fn list_upstreams(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.list_upstreams(&context.version) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |upstreams| {
            upstreams
                .into_iter()
                .map(UpstreamResponse::from)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn create_upstream(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input: UpstreamInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let upstream = match upstream(input) {
        Ok(upstream) => upstream,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.create_upstream(&actor, &context.version, context.revision, upstream) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, |upstream| {
            UpstreamResponse::from(upstream)
        }),
        Err(error) => management_error(error),
    }
}

async fn get_upstream(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = UpstreamId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_upstream(&context.version, &id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |upstream| {
            UpstreamResponse::from(upstream)
        }),
        Err(error) => management_error(error),
    }
}

async fn update_upstream(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let path_id = path.into_inner();
    let input: UpstreamInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    if input.id != path_id {
        return invalid_input();
    }
    let upstream = match upstream(input) {
        Ok(upstream) => upstream,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.update_upstream(&actor, &context.version, context.revision, upstream) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |upstream| {
            UpstreamResponse::from(upstream)
        }),
        Err(error) => management_error(error),
    }
}

async fn delete_upstream(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = UpstreamId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.delete_upstream(&actor, &context.version, context.revision, &id) {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

async fn create_endpoint(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(upstream_id) = UpstreamId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let input: EndpointInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let endpoint = match endpoint(input, upstream_id) {
        Ok(endpoint) => endpoint,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.create_endpoint(&actor, &context.version, context.revision, endpoint) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, |endpoint| {
            EndpointResponse::from(endpoint)
        }),
        Err(error) => management_error(error),
    }
}

async fn get_endpoint(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = EndpointId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_endpoint(&context.version, &id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |endpoint| {
            EndpointResponse::from(endpoint)
        }),
        Err(error) => management_error(error),
    }
}

async fn update_endpoint(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(endpoint_id) = EndpointId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let input: EndpointInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    if input.id != endpoint_id.as_str() {
        return invalid_input();
    }
    let existing = {
        let mut service = match service(&state) {
            Ok(service) => service,
            Err(response) => return response,
        };
        match service.get_endpoint(&context.version, &endpoint_id) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        }
    };
    let endpoint = match endpoint(input, existing.value().upstream_id.clone()) {
        Ok(endpoint) => endpoint,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.update_endpoint(&actor, &context.version, context.revision, endpoint) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |endpoint| {
            EndpointResponse::from(endpoint)
        }),
        Err(error) => management_error(error),
    }
}

async fn delete_endpoint(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = EndpointId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.delete_endpoint(&actor, &context.version, context.revision, &id) {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

async fn test_endpoint(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(endpoint_id) = EndpointId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let input: EndpointTestInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let mode = match endpoint_test_mode(&input.mode) {
        Ok(mode) => mode,
        Err(response) => return response,
    };
    if let Err(response) = require_endpoint(&state, &context.version, &endpoint_id) {
        return response;
    }
    let result = match workflow(&state) {
        Ok(mut workflow) => workflow.test_endpoint(&endpoint_id, mode),
        Err(response) => return response,
    };
    HttpResponse::Ok().json(EndpointTestResponse::from(result))
}

async fn preview_catalog_discovery(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(endpoint_id) = EndpointId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    if let Err(response) = require_endpoint(&state, &context.version, &endpoint_id) {
        return response;
    }
    let result = match workflow(&state) {
        Ok(mut workflow) => workflow.preview_catalog(&endpoint_id),
        Err(response) => return response,
    };
    HttpResponse::Ok().json(CatalogDiffResponse::from(result))
}

async fn apply_catalog_discovery(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(endpoint_id) = EndpointId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    if let Err(response) =
        require_endpoint_at_revision(&state, &context.version, context.revision, &endpoint_id)
    {
        return response;
    }
    let result = match workflow(&state) {
        Ok(mut workflow) => workflow.apply_catalog(&endpoint_id),
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.record_draft_resource_action(
        &actor,
        &context.version,
        context.revision,
        "catalog_discovery_applied",
        "endpoint",
        endpoint_id.as_str(),
    ) {
        Ok(revision) => {
            response_with_revision(StatusCode::OK, revision, CatalogDiffResponse::from(result))
        }
        Err(error) => management_error(error),
    }
}

async fn create_credential(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(upstream_id) = UpstreamId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let input: CredentialInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let (credential_id, kind, secret, status) = match credential_input(input) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.create_credential(
        &actor,
        &context.version,
        context.revision,
        upstream_id,
        CredentialUpsert {
            id: credential_id,
            kind,
            plaintext_secret: secret.as_bytes(),
            status,
        },
    ) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, |credential| {
            CredentialResponse::from(credential)
        }),
        Err(error) => management_error(error),
    }
}

async fn get_credential(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = CredentialId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_credential(&context.version, &id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |credential| {
            CredentialResponse::from(credential)
        }),
        Err(error) => management_error(error),
    }
}

async fn update_credential(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let path_id = path.into_inner();
    let input: CredentialInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    if input.id != path_id {
        return invalid_input();
    }
    let (credential_id, kind, secret, status) = match credential_input(input) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.update_credential(
        &actor,
        &context.version,
        context.revision,
        CredentialUpsert {
            id: credential_id,
            kind,
            plaintext_secret: secret.as_bytes(),
            status,
        },
    ) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |credential| {
            CredentialResponse::from(credential)
        }),
        Err(error) => management_error(error),
    }
}

async fn delete_credential(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = CredentialId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.delete_credential(&actor, &context.version, context.revision, &id) {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

async fn start_credential_oauth(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(credential_id) = CredentialId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    if let Err(response) = require_credential(&state, &context.version, &credential_id) {
        return response;
    }
    let operation = match workflow(&state) {
        Ok(mut workflow) => workflow.start_oauth(&credential_id),
        Err(response) => return response,
    };
    HttpResponse::Accepted().json(CredentialOAuthResponse::new(&credential_id, operation))
}

async fn get_credential_oauth_status(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(credential_id) = CredentialId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    if let Err(response) = require_credential(&state, &context.version, &credential_id) {
        return response;
    }
    let operation = match workflow(&state) {
        Ok(mut workflow) => workflow.oauth_status(&credential_id),
        Err(response) => return response,
    };
    HttpResponse::Ok().json(CredentialOAuthResponse::new(&credential_id, operation))
}

async fn cancel_credential_oauth(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(credential_id) = CredentialId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    if let Err(response) = require_credential(&state, &context.version, &credential_id) {
        return response;
    }
    match workflow(&state) {
        Ok(mut workflow) => workflow.cancel_oauth(&credential_id),
        Err(response) => return response,
    }
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.record_resource_action(
        &actor,
        &context.version,
        "credential_oauth_cancelled",
        "credential",
        credential_id.as_str(),
    ) {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(error) => management_error(error),
    }
}

async fn list_endpoint_credential_bindings(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(endpoint_id) = EndpointId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.list_endpoint_credential_bindings(&context.version, &endpoint_id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |bindings| {
            bindings
                .into_iter()
                .map(BindingResponse::from)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn create_endpoint_credential_binding(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(endpoint_id) = EndpointId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let input: BindingInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let endpoint = match service.get_endpoint(&context.version, &endpoint_id) {
        Ok(endpoint) => endpoint.into_parts().0,
        Err(error) => return management_error(error),
    };
    let Ok(credential_id) = CredentialId::try_new(input.credential_id.clone()) else {
        return invalid_input();
    };
    let credential = match service.get_credential(&context.version, &credential_id) {
        Ok(credential) => credential.into_parts().0,
        Err(error) => return management_error(error),
    };
    if endpoint.upstream_id != credential.upstream_id {
        return invalid_input();
    }
    let binding = match binding(input, endpoint_id, endpoint.upstream_id) {
        Ok(binding) => binding,
        Err(response) => return response,
    };
    match service.create_endpoint_credential_binding(
        &actor,
        &context.version,
        context.revision,
        binding,
    ) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, |binding| {
            BindingResponse::from(binding)
        }),
        Err(error) => management_error(error),
    }
}

async fn list_public_models(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.list_public_models(&context.version) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |models| {
            models
                .into_iter()
                .map(PublicModelResponse::from)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn create_public_model(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let public_model = match parse_json::<PublicModelInput>(&body).and_then(public_model) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.create_public_model(&actor, &context.version, context.revision, public_model) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, PublicModelResponse::from),
        Err(error) => management_error(error),
    }
}

async fn get_public_model(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = PublicModelId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_public_model(&context.version, &id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, PublicModelResponse::from),
        Err(error) => management_error(error),
    }
}

async fn update_public_model(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let path_id = path.into_inner();
    let input = match parse_json::<PublicModelInput>(&body) {
        Ok(input) if input.id == path_id => input,
        Ok(_) | Err(_) => return invalid_input(),
    };
    let public_model = match public_model(input) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.update_public_model(&actor, &context.version, context.revision, public_model) {
        Ok(value) => revisioned_json(StatusCode::OK, value, PublicModelResponse::from),
        Err(error) => management_error(error),
    }
}

async fn delete_public_model(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = PublicModelId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.delete_public_model(&actor, &context.version, context.revision, &id) {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

async fn create_model_alias(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(public_model_id) = PublicModelId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let input = match parse_json::<AliasInput>(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let alias = match model_alias(input, public_model_id) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.create_model_alias(&actor, &context.version, context.revision, alias) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, AliasResponse::from),
        Err(error) => management_error(error),
    }
}

async fn create_model_route(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(public_model_id) = PublicModelId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let input = match parse_json::<RouteInput>(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let route = match model_route(input, public_model_id) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.create_model_route(&actor, &context.version, context.revision, route) {
        Ok(value) => revisioned_route_json(StatusCode::CREATED, value),
        Err(error) => management_error(error),
    }
}

async fn get_model_route(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = RouteId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_model_route(&context.version, &id) {
        Ok(value) => revisioned_route_json(StatusCode::OK, value),
        Err(error) => management_error(error),
    }
}

async fn update_model_route(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(route_id) = RouteId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let input = match parse_json::<RouteInput>(&body) {
        Ok(input) if input.id == route_id.as_str() => input,
        Ok(_) | Err(_) => return invalid_input(),
    };
    let existing = {
        let mut service = match service(&state) {
            Ok(service) => service,
            Err(response) => return response,
        };
        match service.get_model_route(&context.version, &route_id) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        }
    };
    let route = match model_route(input, existing.value().public_model_id.clone()) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.update_model_route(&actor, &context.version, context.revision, route) {
        Ok(value) => revisioned_route_json(StatusCode::OK, value),
        Err(error) => management_error(error),
    }
}

async fn delete_model_route(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(route_id) = RouteId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.delete_model_route(&actor, &context.version, context.revision, &route_id) {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

async fn create_route_candidate(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(route_id) = RouteId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let input = match parse_json::<CandidateInput>(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let candidate = match route_candidate(input, route_id) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.create_route_candidate(&actor, &context.version, context.revision, candidate) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, CandidateResponse::from),
        Err(error) => management_error(error),
    }
}

async fn validate_model_route(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(route_id) = RouteId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.validate_model_route(&context.version, &route_id) {
        Ok(value) => HttpResponse::Ok().json(ValidationResponse::from(value)),
        Err(error) => management_error(error),
    }
}

async fn list_access_groups(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.list_access_groups(&context.version) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |groups| {
            groups
                .into_iter()
                .map(AccessGroupResponse::from)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn create_access_group(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let access_group = match parse_json::<AccessGroupInput>(&body).and_then(access_group) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.create_access_group(&actor, &context.version, context.revision, access_group) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, AccessGroupResponse::from),
        Err(error) => management_error(error),
    }
}

async fn get_access_group(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = AccessGroupId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_access_group(&context.version, &id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, AccessGroupResponse::from),
        Err(error) => management_error(error),
    }
}

async fn update_access_group(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let path_id = path.into_inner();
    let input = match parse_json::<AccessGroupInput>(&body) {
        Ok(input) if input.id == path_id => input,
        Ok(_) | Err(_) => return invalid_input(),
    };
    let access_group = match access_group(input) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.update_access_group(&actor, &context.version, context.revision, access_group) {
        Ok(value) => revisioned_json(StatusCode::OK, value, AccessGroupResponse::from),
        Err(error) => management_error(error),
    }
}

async fn delete_access_group(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(id) = AccessGroupId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.delete_access_group(&actor, &context.version, context.revision, &id) {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

async fn list_access_group_routes(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(access_group_id) = AccessGroupId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.list_access_group_routes(&context.version, &access_group_id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |grants| {
            grants
                .into_iter()
                .map(AccessGroupRouteResponse::from)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn create_access_group_route(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(access_group_id) = AccessGroupId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let input = match parse_json::<AccessGroupRouteInput>(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let grant = match access_group_route(input, access_group_id) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.create_access_group_route(&actor, &context.version, context.revision, grant) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, AccessGroupRouteResponse::from),
        Err(error) => management_error(error),
    }
}

async fn list_client_keys(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.list_client_keys(&context.version) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |keys| {
            keys.into_iter()
                .map(ClientKeyResponse::from)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn issue_client_key(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input = match parse_json::<ClientKeyInput>(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let input = match client_key_issue(input) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.issue_client_key(&actor, &context.version, context.revision, input) {
        Ok(value) => {
            let (issued, revision) = value.into_parts();
            let response =
                IssuedClientKeyResponse::new(issued.metadata(), issued.presented_key().to_owned());
            response_with_revision(StatusCode::CREATED, revision, response)
        }
        Err(error) => management_error(error),
    }
}

async fn get_client_key(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(client_key_id) = ClientKeyId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_client_key(&context.version, &client_key_id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, ClientKeyResponse::from),
        Err(error) => management_error(error),
    }
}

async fn update_client_key(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let path_id = path.into_inner();
    let input = match parse_json::<ClientKeyInput>(&body) {
        Ok(input) if input.id == path_id => input,
        Ok(_) | Err(_) => return invalid_input(),
    };
    let (client_key_id, input) = match client_key_update(input) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.update_client_key(
        &actor,
        &context.version,
        context.revision,
        &client_key_id,
        input,
    ) {
        Ok(value) => revisioned_json(StatusCode::OK, value, ClientKeyResponse::from),
        Err(error) => management_error(error),
    }
}

async fn revoke_client_key(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(client_key_id) = ClientKeyId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.revoke_client_key(&actor, &context.version, context.revision, &client_key_id) {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

struct ReadContext {
    version: ConfigVersionId,
}
struct WriteContext {
    version: ConfigVersionId,
    revision: ConfigRevision,
}

fn read_context(request: &HttpRequest) -> Result<ReadContext, HttpResponse> {
    let value = required_header(request, CONFIG_VERSION_HEADER)?;
    let version = ConfigVersionId::try_new(value.to_owned()).map_err(|_| invalid_input())?;
    Ok(ReadContext { version })
}

fn write_context(request: &HttpRequest) -> Result<WriteContext, HttpResponse> {
    let ReadContext { version } = read_context(request)?;
    let revision =
        ConfigRevision::from_token(required_header(request, IF_MATCH_HEADER)?.trim_matches('"'))
            .map_err(|_| invalid_input())?;
    Ok(WriteContext { version, revision })
}

fn required_header<'request>(
    request: &'request HttpRequest,
    name: &str,
) -> Result<&'request str, HttpResponse> {
    let mut values = request.headers().get_all(name);
    let value = values.next().ok_or_else(invalid_input)?;
    if values.next().is_some() {
        return Err(invalid_input());
    }
    value
        .to_str()
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(invalid_input)
}

fn principal(
    request: &HttpRequest,
) -> Result<gateway_control::management_service::ManagementActor, HttpResponse> {
    request
        .extensions()
        .get::<ManagementRequestPrincipal>()
        .map(|principal| principal.actor().clone())
        .ok_or_else(internal_error)
}

fn service(
    state: &web::Data<ManagementResourceHttpState>,
) -> Result<std::sync::MutexGuard<'_, ManagementMutationService>, HttpResponse> {
    state.service.lock().map_err(|_| internal_error())
}

fn workflow(
    state: &web::Data<ManagementResourceHttpState>,
) -> Result<std::sync::MutexGuard<'_, Box<dyn ManagementEndpointWorkflow>>, HttpResponse> {
    state.workflow.lock().map_err(|_| internal_error())
}

fn require_endpoint(
    state: &web::Data<ManagementResourceHttpState>,
    config_version_id: &ConfigVersionId,
    endpoint_id: &EndpointId,
) -> Result<(), HttpResponse> {
    let mut service = service(state)?;
    service
        .get_endpoint(config_version_id, endpoint_id)
        .map(|_| ())
        .map_err(management_error)
}

fn require_endpoint_at_revision(
    state: &web::Data<ManagementResourceHttpState>,
    config_version_id: &ConfigVersionId,
    expected_revision: ConfigRevision,
    endpoint_id: &EndpointId,
) -> Result<(), HttpResponse> {
    let mut service = service(state)?;
    let (_, current_revision) = service
        .get_endpoint(config_version_id, endpoint_id)
        .map_err(management_error)?
        .into_parts();
    if current_revision == expected_revision {
        Ok(())
    } else {
        Err(management_error(ManagementResourceError::Store(
            StoreError::ConfigVersionRevisionConflict,
        )))
    }
}

fn require_credential(
    state: &web::Data<ManagementResourceHttpState>,
    config_version_id: &ConfigVersionId,
    credential_id: &CredentialId,
) -> Result<(), HttpResponse> {
    let mut service = service(state)?;
    service
        .get_credential(config_version_id, credential_id)
        .map(|_| ())
        .map_err(management_error)
}

fn parse_json<T: DeserializeOwned>(body: &[u8]) -> Result<T, HttpResponse> {
    if body.is_empty() || body.len() > MAX_MANAGEMENT_JSON_BYTES {
        return Err(invalid_input());
    }
    serde_json::from_slice(body).map_err(|_| invalid_input())
}

fn egress_policy(input: EgressPolicyInput) -> Result<EgressPolicyConfiguration, HttpResponse> {
    if input.allowed_schemes.iter().any(|scheme| scheme != "https")
        || input.allowed_schemes.len() > 8
        || input.allowed_hosts.len() > 128
        || input.allowed_ports.len() > 128
        || input.allowed_cidrs.len() > 128
        || input.max_redirects < 0
        || input.max_redirects > 5
        || input
            .allowed_ports
            .iter()
            .any(|port| !(1..=65_535).contains(port))
    {
        return Err(invalid_input());
    }
    let redirect_mode = match input.redirect_mode.as_str() {
        "deny" if input.max_redirects == 0 => StoredEgressRedirectMode::Deny,
        "revalidate" if input.max_redirects > 0 => StoredEgressRedirectMode::Revalidate,
        _ => return Err(invalid_input()),
    };
    Ok(EgressPolicyConfiguration {
        id: EgressPolicyId::try_new(input.id).map_err(|_| invalid_input())?,
        name: bounded_text(input.name, 256)?,
        allowed_schemes_json: json_string(input.allowed_schemes)?,
        allowed_hosts_json: json_string(input.allowed_hosts)?,
        allowed_ports_json: json_string(input.allowed_ports)?,
        allowed_cidrs_json: json_string(input.allowed_cidrs)?,
        redirect_mode,
        max_redirects: input.max_redirects,
    })
}

fn upstream(input: UpstreamInput) -> Result<UpstreamConfiguration, HttpResponse> {
    if input.tags.len() > 32
        || input
            .tags
            .iter()
            .any(|tag| tag.is_empty() || tag.chars().count() > 64)
    {
        return Err(invalid_input());
    }
    Ok(UpstreamConfiguration {
        id: UpstreamId::try_new(input.id).map_err(|_| invalid_input())?,
        name: bounded_text(input.name, 256)?,
        kind: bounded_text(input.kind, 128)?,
        enabled: input.enabled,
        tags_json: json_string(input.tags)?,
        egress_policy_id: input
            .egress_policy_id
            .map(EgressPolicyId::try_new)
            .transpose()
            .map_err(|_| invalid_input())?,
    })
}

fn endpoint(
    input: EndpointInput,
    upstream_id: UpstreamId,
) -> Result<EndpointConfiguration, HttpResponse> {
    if input.transport != "https" || !input.base_url.starts_with("https://") {
        return Err(invalid_input());
    }
    Ok(EndpointConfiguration {
        id: EndpointId::try_new(input.id).map_err(|_| invalid_input())?,
        upstream_id,
        adapter_id: bounded_text(input.adapter_id, 128)?,
        api_format: bounded_text(input.api_format, 128)?,
        base_url: bounded_text(input.base_url, 2048)?,
        inference_path: bounded_text(input.inference_path, 1024)?,
        models_path: input
            .models_path
            .map(|value| bounded_text(value, 1024))
            .transpose()?,
        transport: EndpointTransport::Http,
        enabled: input.enabled,
    })
}

fn credential_input(
    input: CredentialInput,
) -> Result<(CredentialId, String, Zeroizing<String>, CredentialStatus), HttpResponse> {
    let status = match input.status.as_str() {
        "active" => CredentialStatus::Active,
        "disabled" | "revoked" => CredentialStatus::Disabled,
        _ => return Err(invalid_input()),
    };
    if input.secret.is_empty() || input.secret.len() > 65_536 {
        return Err(invalid_input());
    }
    Ok((
        CredentialId::try_new(input.id).map_err(|_| invalid_input())?,
        bounded_text(input.kind, 128)?,
        Zeroizing::new(input.secret),
        status,
    ))
}

fn binding(
    input: BindingInput,
    endpoint_id: EndpointId,
    upstream_id: UpstreamId,
) -> Result<EndpointCredentialBindingConfiguration, HttpResponse> {
    if input.priority < 0
        || !(1..=10_000).contains(&input.weight)
        || !(1..=100_000).contains(&input.concurrency)
    {
        return Err(invalid_input());
    }
    Ok(EndpointCredentialBindingConfiguration {
        endpoint_id,
        upstream_id,
        credential_id: CredentialId::try_new(input.credential_id).map_err(|_| invalid_input())?,
        enabled: input.enabled,
        priority: input.priority,
        weight: input.weight,
        concurrency: input.concurrency,
    })
}

fn public_model(input: PublicModelInput) -> Result<PublicModelConfiguration, HttpResponse> {
    bounded_boolean_map(&input.capabilities, 32)?;
    Ok(PublicModelConfiguration {
        id: PublicModelId::try_new(input.id).map_err(|_| invalid_input())?,
        model_name: bounded_text(input.model_name, 256)?,
        status: administrative_status(&input.status)?,
        display_name: bounded_text(input.display_name, 256)?,
        capabilities_json: json_string(input.capabilities)?,
    })
}

fn model_alias(
    input: AliasInput,
    public_model_id: PublicModelId,
) -> Result<ModelAliasConfiguration, HttpResponse> {
    Ok(ModelAliasConfiguration {
        alias: bounded_text(input.alias, 256)?,
        public_model_id,
    })
}

fn model_route(
    input: RouteInput,
    public_model_id: PublicModelId,
) -> Result<ModelRouteConfiguration, HttpResponse> {
    if input.max_attempts <= 0
        || input.max_attempts > 16
        || input.bootstrap_timeout_ms <= 0
        || input.bootstrap_timeout_ms > 120_000
        || input.policy != "smooth_weighted_round_robin"
    {
        return Err(invalid_input());
    }
    Ok(ModelRouteConfiguration {
        id: RouteId::try_new(input.id).map_err(|_| invalid_input())?,
        public_model_id,
        policy: RoutePolicy::SmoothWeightedRoundRobin,
        max_attempts: input.max_attempts,
        bootstrap_timeout_ms: input.bootstrap_timeout_ms,
    })
}

fn route_candidate(
    input: CandidateInput,
    route_id: RouteId,
) -> Result<RouteCandidateConfiguration, HttpResponse> {
    if input.priority < 0 || !(1..=10_000).contains(&input.weight) {
        return Err(invalid_input());
    }
    bounded_boolean_map(&input.capability_override, 32)?;
    let transform_mode = match input.transform_mode.as_str() {
        "passthrough" => TransformMode::Passthrough,
        "canonical" => TransformMode::Canonical,
        "lossless_bridge" => TransformMode::LosslessBridge,
        _ => return Err(invalid_input()),
    };
    if input.credential_scope != "all_active" {
        return Err(invalid_input());
    }
    Ok(RouteCandidateConfiguration {
        id: RouteCandidateId::try_new(input.id).map_err(|_| invalid_input())?,
        route_id,
        endpoint_id: EndpointId::try_new(input.endpoint_id).map_err(|_| invalid_input())?,
        upstream_model: bounded_text(input.upstream_model, 256)?,
        credential_scope: CredentialScope::EndpointBindings,
        transform_mode,
        enabled: input.enabled,
        priority: input.priority,
        weight: input.weight,
        capability_override_json: json_string(input.capability_override)?,
    })
}

fn access_group(input: AccessGroupInput) -> Result<AccessGroupConfiguration, HttpResponse> {
    if input.limits.len() > 16
        || input
            .limits
            .iter()
            .any(|(key, value)| key.trim().is_empty() || key.chars().count() > 128 || *value < 0)
    {
        return Err(invalid_input());
    }
    Ok(AccessGroupConfiguration {
        id: AccessGroupId::try_new(input.id).map_err(|_| invalid_input())?,
        name: bounded_text(input.name, 256)?,
        status: administrative_status(&input.status)?,
        limits_json: json_string(input.limits)?,
    })
}

fn access_group_route(
    input: AccessGroupRouteInput,
    access_group_id: AccessGroupId,
) -> Result<AccessGroupRouteConfiguration, HttpResponse> {
    Ok(AccessGroupRouteConfiguration {
        access_group_id,
        route_id: RouteId::try_new(input.route_id).map_err(|_| invalid_input())?,
        enabled: input.enabled,
    })
}

fn client_key_parts(
    input: ClientKeyInput,
) -> Result<
    (
        ClientKeyId,
        AccessGroupId,
        StoredClientKeyStatus,
        Option<i64>,
    ),
    HttpResponse,
> {
    if input.expires_at_ms.is_some_and(|value| value < 0) {
        return Err(invalid_input());
    }
    let status = match input.status.as_str() {
        "active" => StoredClientKeyStatus::Active,
        "disabled" => StoredClientKeyStatus::Disabled,
        "revoked" => StoredClientKeyStatus::Revoked,
        _ => return Err(invalid_input()),
    };
    Ok((
        ClientKeyId::try_new(input.id).map_err(|_| invalid_input())?,
        AccessGroupId::try_new(input.access_group_id).map_err(|_| invalid_input())?,
        status,
        input.expires_at_ms,
    ))
}

fn client_key_issue(input: ClientKeyInput) -> Result<ClientKeyIssue, HttpResponse> {
    let (id, access_group_id, status, expires_at_ms) = client_key_parts(input)?;
    Ok(ClientKeyIssue {
        id,
        access_group_id,
        status,
        expires_at_ms,
    })
}

fn client_key_update(
    input: ClientKeyInput,
) -> Result<(ClientKeyId, ClientKeyUpdate), HttpResponse> {
    let (id, access_group_id, status, expires_at_ms) = client_key_parts(input)?;
    Ok((
        id,
        ClientKeyUpdate {
            access_group_id,
            status,
            expires_at_ms,
        },
    ))
}

fn administrative_status(value: &str) -> Result<AdministrativeStatus, HttpResponse> {
    match value {
        "active" => Ok(AdministrativeStatus::Active),
        "disabled" => Ok(AdministrativeStatus::Disabled),
        _ => Err(invalid_input()),
    }
}

fn bounded_boolean_map(
    values: &BTreeMap<String, bool>,
    maximum_entries: usize,
) -> Result<(), HttpResponse> {
    if values.len() > maximum_entries
        || values
            .keys()
            .any(|key| key.trim().is_empty() || key.chars().count() > 128)
    {
        Err(invalid_input())
    } else {
        Ok(())
    }
}

fn endpoint_test_mode(value: &str) -> Result<ManagementEndpointTestMode, HttpResponse> {
    match value {
        "non_streaming" => Ok(ManagementEndpointTestMode::NonStreaming),
        "sse" => Ok(ManagementEndpointTestMode::Sse),
        _ => Err(invalid_input()),
    }
}

fn bounded_text(value: String, maximum: usize) -> Result<String, HttpResponse> {
    if value.trim().is_empty() || value.chars().count() > maximum {
        Err(invalid_input())
    } else {
        Ok(value)
    }
}

fn json_string<T: Serialize>(value: T) -> Result<String, HttpResponse> {
    serde_json::to_string(&value).map_err(|_| internal_error())
}

fn revisioned_json<T, U: Serialize>(
    status: StatusCode,
    value: Revisioned<T>,
    convert: impl FnOnce(T) -> U,
) -> HttpResponse {
    let (resource, revision) = value.into_parts();
    HttpResponse::build(status)
        .insert_header((header::ETAG, format!("\"{}\"", revision.as_token())))
        .json(convert(resource))
}

/// Serializes the P10-05 Route contract only when the stored policy is representable by its
/// frozen single-policy `OpenAPI` enum. Older stored policies must not be silently reclassified.
fn revisioned_route_json(
    status: StatusCode,
    value: Revisioned<ModelRouteConfiguration>,
) -> HttpResponse {
    let (route, revision) = value.into_parts();
    match RouteResponse::try_from(route) {
        Ok(response) => response_with_revision(status, revision, response),
        Err(UnsupportedRoutePolicy) => internal_error(),
    }
}

fn response_with_revision<T: Serialize>(
    status: StatusCode,
    revision: ConfigRevision,
    value: T,
) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header((header::ETAG, format!("\"{}\"", revision.as_token())))
        .json(value)
}

fn empty_with_revision(revision: ConfigRevision) -> HttpResponse {
    HttpResponse::NoContent()
        .insert_header((header::ETAG, format!("\"{}\"", revision.as_token())))
        .finish()
}

fn management_error(error: ManagementResourceError) -> HttpResponse {
    let response = match &error {
        ManagementResourceError::ConfigVersionNotFound
        | ManagementResourceError::ResourceNotFound
        | ManagementResourceError::Store(
            StoreError::ConfigVersionNotFound | StoreError::ControlPlaneResourceNotFound,
        ) => error_response(
            StatusCode::NOT_FOUND,
            "management_resource_not_found",
            "Management resource was not found",
        ),
        ManagementResourceError::Store(StoreError::ConfigVersionRevisionConflict) => {
            error_response(
                StatusCode::CONFLICT,
                "management_revision_conflict",
                "Management configuration changed",
            )
        }
        ManagementResourceError::Store(StoreError::ControlPlaneMutationRequiresDraft) => {
            error_response(
                StatusCode::CONFLICT,
                "management_version_not_writable",
                "Management configuration is not writable",
            )
        }
        ManagementResourceError::InvalidRevision
        | ManagementResourceError::InvalidCredentialInput => invalid_input(),
        ManagementResourceError::Store(_)
        | ManagementResourceError::SecretStore(_)
        | ManagementResourceError::ControlPlane(_)
        | ManagementResourceError::Clock(_)
        | ManagementResourceError::ClientKey(_)
        | ManagementResourceError::ClientKeyIssuerUnavailable => internal_error(),
    };
    drop(error);
    response
}

fn invalid_input() -> HttpResponse {
    error_response(
        StatusCode::BAD_REQUEST,
        "invalid_management_request",
        "Management request is invalid",
    )
}
fn internal_error() -> HttpResponse {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "management_internal_error",
        "Management operation failed",
    )
}
fn error_response(status: StatusCode, code: &'static str, message: &'static str) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(serde_json::json!({"error":{"code":code,"message":message}}))
}

impl From<PublicModelConfiguration> for PublicModelResponse {
    fn from(value: PublicModelConfiguration) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            model_name: value.model_name,
            status: administrative_status_response(value.status),
            display_name: value.display_name,
            capabilities: json_array(&value.capabilities_json),
        }
    }
}
impl From<ModelAliasConfiguration> for AliasResponse {
    fn from(value: ModelAliasConfiguration) -> Self {
        Self {
            alias: value.alias,
            public_model_id: value.public_model_id.as_str().to_owned(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnsupportedRoutePolicy;

impl TryFrom<ModelRouteConfiguration> for RouteResponse {
    type Error = UnsupportedRoutePolicy;

    fn try_from(value: ModelRouteConfiguration) -> Result<Self, Self::Error> {
        let policy = match value.policy {
            RoutePolicy::SmoothWeightedRoundRobin => "smooth_weighted_round_robin",
            RoutePolicy::RoundRobin | RoutePolicy::PriorityFailover => {
                return Err(UnsupportedRoutePolicy);
            }
        };
        Ok(Self {
            id: value.id.as_str().to_owned(),
            public_model_id: value.public_model_id.as_str().to_owned(),
            policy,
            max_attempts: value.max_attempts,
            bootstrap_timeout_ms: value.bootstrap_timeout_ms,
        })
    }
}
impl From<RouteCandidateConfiguration> for CandidateResponse {
    fn from(value: RouteCandidateConfiguration) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            route_id: value.route_id.as_str().to_owned(),
            endpoint_id: value.endpoint_id.as_str().to_owned(),
            upstream_model: value.upstream_model,
            credential_scope: match value.credential_scope {
                CredentialScope::EndpointBindings => "all_active",
            },
            transform_mode: match value.transform_mode {
                TransformMode::Passthrough => "passthrough",
                TransformMode::Canonical => "canonical",
                TransformMode::LosslessBridge => "lossless_bridge",
            },
            enabled: value.enabled,
            priority: value.priority,
            weight: value.weight,
            capability_override: json_array(&value.capability_override_json),
        }
    }
}
impl From<AccessGroupConfiguration> for AccessGroupResponse {
    fn from(value: AccessGroupConfiguration) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            name: value.name,
            status: administrative_status_response(value.status),
            limits: json_array(&value.limits_json),
        }
    }
}
impl From<AccessGroupRouteConfiguration> for AccessGroupRouteResponse {
    fn from(value: AccessGroupRouteConfiguration) -> Self {
        Self {
            access_group_id: value.access_group_id.as_str().to_owned(),
            route_id: value.route_id.as_str().to_owned(),
            enabled: value.enabled,
        }
    }
}
impl From<ClientKeyView> for ClientKeyResponse {
    fn from(value: ClientKeyView) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            access_group_id: value.access_group_id.as_str().to_owned(),
            prefix: value.prefix,
            status: client_key_status_response(value.status),
            expires_at_ms: value.expires_at_ms,
        }
    }
}
impl IssuedClientKeyResponse {
    fn new(metadata: &ClientKeyView, key: String) -> Self {
        Self {
            id: metadata.id.as_str().to_owned(),
            access_group_id: metadata.access_group_id.as_str().to_owned(),
            prefix: metadata.prefix.clone(),
            status: client_key_status_response(metadata.status),
            expires_at_ms: metadata.expires_at_ms,
            key,
        }
    }
}
impl From<ManagementRouteValidation> for ValidationResponse {
    fn from(value: ManagementRouteValidation) -> Self {
        Self {
            valid: value.valid,
            error_codes: value.error_codes,
        }
    }
}

const fn administrative_status_response(value: AdministrativeStatus) -> &'static str {
    match value {
        AdministrativeStatus::Active => "active",
        AdministrativeStatus::Disabled => "disabled",
    }
}

const fn client_key_status_response(value: StoredClientKeyStatus) -> &'static str {
    match value {
        StoredClientKeyStatus::Active => "active",
        StoredClientKeyStatus::Disabled => "disabled",
        StoredClientKeyStatus::Revoked => "revoked",
    }
}

impl From<EgressPolicyConfiguration> for EgressPolicyResponse {
    fn from(value: EgressPolicyConfiguration) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            name: value.name,
            allowed_schemes: json_array(&value.allowed_schemes_json),
            allowed_hosts: json_array(&value.allowed_hosts_json),
            allowed_ports: json_array(&value.allowed_ports_json),
            allowed_cidrs: json_array(&value.allowed_cidrs_json),
            redirect_mode: match value.redirect_mode {
                StoredEgressRedirectMode::Deny => "deny",
                StoredEgressRedirectMode::SameOrigin | StoredEgressRedirectMode::Revalidate => {
                    "revalidate"
                }
            },
            max_redirects: value.max_redirects,
        }
    }
}
impl From<UpstreamConfiguration> for UpstreamResponse {
    fn from(value: UpstreamConfiguration) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            name: value.name,
            kind: value.kind,
            enabled: value.enabled,
            tags: json_array(&value.tags_json),
            egress_policy_id: value.egress_policy_id.map(|id| id.as_str().to_owned()),
        }
    }
}
impl From<EndpointConfiguration> for EndpointResponse {
    fn from(value: EndpointConfiguration) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            upstream_id: value.upstream_id.as_str().to_owned(),
            adapter_id: value.adapter_id,
            api_format: value.api_format,
            base_url: value.base_url,
            inference_path: value.inference_path,
            models_path: value.models_path,
            transport: "https",
            enabled: value.enabled,
        }
    }
}
impl From<CredentialView> for CredentialResponse {
    fn from(value: CredentialView) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            upstream_id: value.upstream_id.as_str().to_owned(),
            kind: value.kind,
            status: match value.status {
                CredentialStatus::Active => "active",
                CredentialStatus::Cooling
                | CredentialStatus::Unauthorized
                | CredentialStatus::Disabled => "disabled",
            },
            revision: value.revision,
            secret_present: value.secret_present,
        }
    }
}
impl From<EndpointCredentialBindingConfiguration> for BindingResponse {
    fn from(value: EndpointCredentialBindingConfiguration) -> Self {
        Self {
            endpoint_id: value.endpoint_id.as_str().to_owned(),
            upstream_id: value.upstream_id.as_str().to_owned(),
            credential_id: value.credential_id.as_str().to_owned(),
            enabled: value.enabled,
            priority: value.priority,
            weight: value.weight,
            concurrency: value.concurrency,
        }
    }
}
impl From<ManagementEndpointTestResult> for EndpointTestResponse {
    fn from(value: ManagementEndpointTestResult) -> Self {
        Self {
            outcome: match value.outcome {
                ManagementEndpointTestOutcome::Pass => "pass",
                ManagementEndpointTestOutcome::Rejected => "rejected",
                ManagementEndpointTestOutcome::TransportFailed => "transport_failed",
                ManagementEndpointTestOutcome::ProtocolFailed => "protocol_failed",
            },
            status_class: match value.status_class {
                ManagementEndpointStatusClass::TwoXx => "2xx",
                ManagementEndpointStatusClass::FourXx => "4xx",
                ManagementEndpointStatusClass::FiveXx => "5xx",
                ManagementEndpointStatusClass::Other => "other",
            },
            canonical_lifecycle: value.canonical_lifecycle,
        }
    }
}
impl From<ManagementCatalogDiff> for CatalogDiffResponse {
    fn from(value: ManagementCatalogDiff) -> Self {
        Self {
            added: value.added,
            removed: value.removed,
            unchanged: value.unchanged,
        }
    }
}
impl CredentialOAuthResponse {
    fn new(credential_id: &CredentialId, value: ManagementCredentialOAuthOperation) -> Self {
        Self {
            credential_id: credential_id.as_str().to_owned(),
            state: match value.state {
                ManagementCredentialOAuthState::Pending => "pending",
                ManagementCredentialOAuthState::Complete => "complete",
                ManagementCredentialOAuthState::Cancelled => "cancelled",
                ManagementCredentialOAuthState::Failed => "failed",
            },
            expires_at_ms: value.expires_at_ms,
        }
    }
}

fn json_array<T: DeserializeOwned>(value: &str) -> T {
    serde_json::from_str(value)
        .unwrap_or_else(|_| unreachable!("validated storage JSON must decode"))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn route_with_policy(
        policy: RoutePolicy,
    ) -> Result<ModelRouteConfiguration, gateway_core::InvalidIdentifier> {
        Ok(ModelRouteConfiguration {
            id: RouteId::try_new("route-policy")?,
            public_model_id: PublicModelId::try_new("model-policy")?,
            policy,
            max_attempts: 1,
            bootstrap_timeout_ms: 1_000,
        })
    }

    #[test]
    fn route_response_rejects_legacy_policy_instead_of_relabelling_it() -> TestResult {
        let supported =
            RouteResponse::try_from(route_with_policy(RoutePolicy::SmoothWeightedRoundRobin)?)
                .map_err(|_| std::io::Error::other("frozen P10-05 policy is not representable"))?;
        assert_eq!(supported.policy, "smooth_weighted_round_robin");
        assert!(RouteResponse::try_from(route_with_policy(RoutePolicy::RoundRobin)?).is_err());
        assert!(
            RouteResponse::try_from(route_with_policy(RoutePolicy::PriorityFailover)?).is_err()
        );
        Ok(())
    }
}
