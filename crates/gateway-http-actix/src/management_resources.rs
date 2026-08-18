//! Protected versioned management resource handlers for P10-04 and P10-05.
//!
//! These handlers decode only bounded explicit resource shapes and delegate every durable graph
//! mutation to `gateway-control`. They never publish a Snapshot, invoke a Provider, expose a
//! credential Secret/ciphertext, or bypass the P10-02 `/admin` security scope.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use actix_web::{
    HttpMessage, HttpRequest, HttpResponse,
    http::{StatusCode, header},
    web,
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use gateway_control::control_plane_service::ControlPlaneServiceError;
use gateway_control::management_mutation_service::{
    AccessGroupConfiguration, AccessGroupRouteConfiguration, AdministrativeStatus,
    BillingCatalogImport, BillingCatalogMutationOperation, BillingCatalogMutationReceipt,
    BillingCatalogSource, BillingPriceCatalog, BillingPriceEntry, ClientKeyIssue, ClientKeyUpdate,
    ClientKeyView, CompatibleEgressBindingConfiguration, CompatibleEgressBindingView,
    CompatibleEgressTargetConfiguration, CompatibleEgressTargetView, CompatibleProxyNodeId,
    CompatibleProxyNodeUpsert, CompatibleProxyNodeView, CompatibleProxyPoolConfiguration,
    CompatibleProxyPoolId, CompatibleProxyPoolView, ConfigRevision, ConfigVersionId,
    CredentialScope, CredentialStatus, CredentialUpsert, CredentialView, EgressPolicyConfiguration,
    EndpointConfiguration, EndpointCredentialBindingConfiguration, EndpointTransport,
    ManagementMutationService, ManagementResourceError, ManagementRouteValidation,
    ModelAliasConfiguration, ModelRouteConfiguration, PublicModelConfiguration, Revisioned,
    RouteCandidateConfiguration, RoutePolicy, RoutingPriceComparison,
    RoutingPricePolicyConfiguration, StoreError, StoredClientKeyStatus,
    StoredCompatibleFailureScope, StoredCompatibleStickiness, StoredEgressRedirectMode,
    TransformMode, UpstreamConfiguration,
};
use gateway_control::management_operations_service::{
    DEFAULT_ACCOUNT_POOL_LIMIT, DEFAULT_FAILURE_FEEDBACK_LIMIT, DEFAULT_USAGE_LIMIT,
    FailureFeedbackCursor, FailureFeedbackPage, FailureFeedbackQuery, MAX_ACCOUNT_POOL_LIMIT,
    MAX_FAILURE_FEEDBACK_LIMIT, MAX_USAGE_EVENTS, MAX_USAGE_LIMIT, MAX_USAGE_MODEL_CHARS,
    ManagementOperationsError, OperationalAccountPoolCursor, OperationalAccountPoolItem,
    OperationalAccountPoolPage, OperationalAccountPoolQuery, OperationalBillingCursor,
    OperationalBillingPage, OperationalBillingQuery, OperationalBillingStatus,
    OperationalCostConfidence, OperationalTokenConfidence, OperationalTokenMetric,
    OperationalUsageCursor, OperationalUsagePage, OperationalUsageQuery,
    compile_operational_billing_page,
};
use gateway_control::provider_account_pool_service::{
    DEFAULT_PROVIDER_ACCOUNT_POOL_LIMIT, ProviderAccountAuthStatus, ProviderAccountOperatorAction,
    ProviderAccountOperatorActionKind, ProviderAccountOperatorReceipt, ProviderAccountPoolCursor,
    ProviderAccountPoolError, ProviderAccountPoolFacade, ProviderAccountPoolItem,
    ProviderAccountPoolPage, ProviderAccountPoolQuery, ProviderAccountRuntimeStatus,
    RejectingProviderAccountPoolFacade,
};
use gateway_control::provider_egress_status_service::{
    DEFAULT_PROVIDER_EGRESS_STATUS_LIMIT, MAX_PROVIDER_EGRESS_STATUS_CURSOR_LENGTH,
    ProviderEgressStatusClearanceItem, ProviderEgressStatusCursor, ProviderEgressStatusDomain,
    ProviderEgressStatusEgressItem, ProviderEgressStatusError, ProviderEgressStatusFacade,
    ProviderEgressStatusItem, ProviderEgressStatusItemKey, ProviderEgressStatusPage,
    ProviderEgressStatusQuery, ProviderEgressStatusSessionItem, ProviderEgressStatusState,
    ProviderEgressStatusTargetKind, RejectingProviderEgressStatusFacade,
};
use gateway_core::{
    AccessGroupId, ClientKeyId, CredentialId, EgressPolicyId, EndpointId, GatewayProtocol,
    ProviderId, PublicModelId, RequestId, RouteCandidateId, RouteId, UpstreamId,
};
use gateway_store::billing_ledger::SqliteBillingLedger;
use gateway_upstream::UpstreamProxy;
use provider_openai_compatible::{
    CodexCredentialExportFormat, CodexOAuthRefreshCoordinator, CodexOAuthRevisionedCredential,
    CodexOAuthTokenTransport, CodexOAuthTransportError, OpenAiCompatibleRuntimeCredential,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use sha2::Digest;
use zeroize::Zeroizing;

use crate::codex_oauth_management::{CodexOAuthSession, CodexOAuthSessionState};
use crate::management_security::{ManagementRequestPrincipal, configure_management};

const CONFIG_VERSION_HEADER: &str = "x-config-version";
const IF_MATCH_HEADER: &str = "if-match";
const MAX_MANAGEMENT_JSON_BYTES: usize = 70 * 1024;
const MAX_RUNTIME_ROWS: usize = 256;
const MAX_REQUEST_ATTEMPTS: usize = 128;
const MAX_BILLING_CATALOG_ENTRIES: usize = 512;
// JSON numbers must remain exactly representable for generated TypeScript consumers.
const MAX_BILLING_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const CODEX_OAUTH_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CODEX_OAUTH_RESPONSE_BYTES: u64 = 256 * 1024;
const CODEX_OAUTH_USER_AGENT: &str = "codex_cli_rs/0.144.1";

/// Management-time application state for P10-04 resource handlers.
///
/// The owned `SQLite` service stays behind a synchronous mutex because its mutations are short,
/// serialized transactions. Provider and OAuth workflows remain separately injected in later
/// P10-04 code and never run while this lock is held.
pub struct ManagementResourceHttpState {
    service: Mutex<ManagementMutationService>,
    workflow: Mutex<Box<dyn ManagementEndpointWorkflow>>,
    runtime: Mutex<Box<dyn ManagementRuntimeFacade>>,
    channel_pin: Mutex<Box<dyn ManagementChannelPinFacade>>,
    usage: Mutex<Box<dyn ManagementUsageFacade>>,
    failure_feedback: Mutex<Box<dyn ManagementFailureFeedbackFacade>>,
    provider_account_pools: Mutex<Box<dyn ProviderAccountPoolFacade>>,
    provider_egress_status: Mutex<Box<dyn ProviderEgressStatusFacade>>,
    /// Credential ids with an in-flight refresh.  The claim spans decrypt, upstream refresh, and
    /// the revision-guarded persistence write so two HTTP callers can never spend the same
    /// rotating refresh token concurrently.
    oauth_refresh_claims: Mutex<BTreeSet<CredentialId>>,
    runtime_clock: Box<dyn ManagementRuntimeClock>,
}

/// Read-only source for the durable usage/cost operations projection.
pub trait ManagementUsageFacade: Send + Sync {
    /// Compiles one bounded usage page without receiving a Provider, Secret, or request body.
    ///
    /// # Errors
    ///
    /// Returns a safe operations error when the durable source or aggregation is unavailable.
    fn list_usage(
        &self,
        query: &OperationalUsageQuery,
    ) -> Result<OperationalUsagePage, ManagementOperationsError>;

    /// Lists immutable, secret-free billing rows and a filtered status/cost summary.
    ///
    /// # Errors
    ///
    /// Returns a safe operations error when the durable billing source is unavailable.
    fn list_billing(
        &self,
        _query: &OperationalBillingQuery,
    ) -> Result<OperationalBillingPage, ManagementOperationsError> {
        Err(ManagementOperationsError::SourceUnavailable)
    }
}

/// Read-only source for durable, secret-free Provider account failure feedback.
pub trait ManagementFailureFeedbackFacade: Send + Sync {
    /// Compiles a bounded page from gateway-owned Attempt events only.
    ///
    /// # Errors
    ///
    /// Returns a management operations error when the durable source is unavailable, oversized,
    /// malformed, or the cursor does not match the requested filters.
    fn list_failure_feedback(
        &self,
        query: &FailureFeedbackQuery,
    ) -> Result<FailureFeedbackPage, ManagementOperationsError>;
}

/// Fail-closed failure source used until the deployment injects its event-log reader.
pub struct RejectingManagementFailureFeedbackFacade;

impl RejectingManagementFailureFeedbackFacade {
    /// Creates a no-send, no-provider default.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RejectingManagementFailureFeedbackFacade {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagementFailureFeedbackFacade for RejectingManagementFailureFeedbackFacade {
    fn list_failure_feedback(
        &self,
        _query: &FailureFeedbackQuery,
    ) -> Result<FailureFeedbackPage, ManagementOperationsError> {
        Err(ManagementOperationsError::SourceUnavailable)
    }
}

/// Read-only billing facade backed by the durable `SQLite` ledger.
pub struct SqliteBillingManagementFacade {
    ledger: Mutex<SqliteBillingLedger>,
}

impl SqliteBillingManagementFacade {
    /// Wraps an already-migrated billing ledger for bounded management reads.
    #[must_use]
    pub fn new(ledger: SqliteBillingLedger) -> Self {
        Self {
            ledger: Mutex::new(ledger),
        }
    }
}

impl ManagementUsageFacade for SqliteBillingManagementFacade {
    fn list_usage(
        &self,
        _query: &OperationalUsageQuery,
    ) -> Result<OperationalUsagePage, ManagementOperationsError> {
        Err(ManagementOperationsError::SourceUnavailable)
    }

    fn list_billing(
        &self,
        query: &OperationalBillingQuery,
    ) -> Result<OperationalBillingPage, ManagementOperationsError> {
        let ledger = self
            .ledger
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        let entries = ledger
            .list_bounded(MAX_USAGE_EVENTS + 1)
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        if entries.len() > MAX_USAGE_EVENTS {
            return Err(ManagementOperationsError::SourceUnavailable);
        }
        compile_operational_billing_page(&entries, query)
    }
}

/// Fail-closed usage source used until the serving composition injects its event-log reader.
pub struct RejectingManagementUsageFacade;

impl RejectingManagementUsageFacade {
    /// Creates a no-op, no-send usage source.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RejectingManagementUsageFacade {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagementUsageFacade for RejectingManagementUsageFacade {
    fn list_usage(
        &self,
        _query: &OperationalUsageQuery,
    ) -> Result<OperationalUsagePage, ManagementOperationsError> {
        Err(ManagementOperationsError::SourceUnavailable)
    }
}

struct OAuthRefreshClaim<'a> {
    claims: &'a Mutex<BTreeSet<CredentialId>>,
    credential_id: CredentialId,
}

impl<'a> OAuthRefreshClaim<'a> {
    fn try_acquire(
        claims: &'a Mutex<BTreeSet<CredentialId>>,
        credential_id: CredentialId,
    ) -> Option<Self> {
        let acquired = claims
            .lock()
            .ok()
            .is_some_and(|mut claims| claims.insert(credential_id.clone()));
        acquired.then_some(Self {
            claims,
            credential_id,
        })
    }
}

impl Drop for OAuthRefreshClaim<'_> {
    fn drop(&mut self) {
        if let Ok(mut claims) = self.claims.lock() {
            claims.remove(&self.credential_id);
        }
    }
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
        Self::with_workflow_and_runtime(
            service,
            workflow,
            Box::new(RejectingManagementRuntimeFacade::new()),
            Box::new(SystemManagementRuntimeClock),
        )
    }

    /// Creates the state with explicit bounded Endpoint and runtime-management seams.
    ///
    /// The runtime facade is intentionally distinct from the P10-04 Endpoint workflow: it has no
    /// Provider request surface and can expose only the value-free P10-06 projections below.
    #[must_use]
    pub fn with_workflow_and_runtime(
        service: ManagementMutationService,
        workflow: Box<dyn ManagementEndpointWorkflow>,
        runtime: Box<dyn ManagementRuntimeFacade>,
        runtime_clock: Box<dyn ManagementRuntimeClock>,
    ) -> Self {
        Self::with_workflow_and_runtime_and_usage(
            service,
            workflow,
            runtime,
            runtime_clock,
            Box::new(RejectingManagementUsageFacade::new()),
        )
    }

    /// Creates the state with an explicit read-only usage/cost observation source.
    #[must_use]
    pub fn with_workflow_and_runtime_and_usage(
        service: ManagementMutationService,
        workflow: Box<dyn ManagementEndpointWorkflow>,
        runtime: Box<dyn ManagementRuntimeFacade>,
        runtime_clock: Box<dyn ManagementRuntimeClock>,
        usage: Box<dyn ManagementUsageFacade>,
    ) -> Self {
        Self {
            service: Mutex::new(service),
            workflow: Mutex::new(workflow),
            runtime: Mutex::new(runtime),
            channel_pin: Mutex::new(Box::new(RejectingManagementChannelPinFacade::new())),
            usage: Mutex::new(usage),
            failure_feedback: Mutex::new(Box::new(RejectingManagementFailureFeedbackFacade::new())),
            provider_account_pools: Mutex::new(Box::new(RejectingProviderAccountPoolFacade::new())),
            provider_egress_status: Mutex::new(
                Box::new(RejectingProviderEgressStatusFacade::new()),
            ),
            oauth_refresh_claims: Mutex::new(BTreeSet::new()),
            runtime_clock,
        }
    }

    /// Replaces the default rejecting usage source without changing the other management seams.
    #[must_use]
    pub fn with_usage(mut self, usage: Box<dyn ManagementUsageFacade>) -> Self {
        self.usage = Mutex::new(usage);
        self
    }

    /// Replaces the default failure-feedback source with a bounded event-log reader.
    #[must_use]
    pub fn with_failure_feedback(
        mut self,
        failure_feedback: Box<dyn ManagementFailureFeedbackFacade>,
    ) -> Self {
        self.failure_feedback = Mutex::new(failure_feedback);
        self
    }

    /// Replaces the fail-closed Provider-owned account-pool source with an injected read-only
    /// facade. The facade builds the observation snapshot; this HTTP layer never contacts a
    /// Provider or decrypts an account.
    #[must_use]
    pub fn with_provider_account_pools(
        mut self,
        provider_account_pools: Box<dyn ProviderAccountPoolFacade>,
    ) -> Self {
        self.provider_account_pools = Mutex::new(provider_account_pools);
        self
    }

    /// Replaces the fail-closed Provider-specific egress/session/clearance source with an
    /// explicitly composed, read-only facade. This setter never creates a Provider client or
    /// enables recovery actions.
    #[must_use]
    pub fn with_provider_egress_status(
        mut self,
        provider_egress_status: Box<dyn ProviderEgressStatusFacade>,
    ) -> Self {
        self.provider_egress_status = Mutex::new(provider_egress_status);
        self
    }

    /// Replaces the fail-closed Channel Pin executor with one explicitly owned by the serving
    /// composition. The executor is independent from the ordinary management runtime read model.
    #[must_use]
    pub fn with_channel_pin(mut self, channel_pin: Box<dyn ManagementChannelPinFacade>) -> Self {
        self.channel_pin = Mutex::new(channel_pin);
        self
    }

    fn claim_oauth_refresh(&self, credential_id: &CredentialId) -> Option<OAuthRefreshClaim<'_>> {
        OAuthRefreshClaim::try_acquire(&self.oauth_refresh_claims, credential_id.clone())
    }
}

/// One explicit clock for P10-06's fixed-time runtime projections.
///
/// Runtime facades receive the sampled instant instead of sampling independently, which keeps a
/// status/explain result reproducible and prevents a handler from borrowing a dataplane clock.
pub trait ManagementRuntimeClock: Send + Sync {
    /// Returns the current Unix-millisecond observation time.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::Unavailable`] when a safe observation time cannot be
    /// produced.
    fn now_ms(&self) -> Result<i64, ManagementRuntimeError>;
}

/// The normal process-local observation clock for an injected production facade.
pub struct SystemManagementRuntimeClock;

impl ManagementRuntimeClock for SystemManagementRuntimeClock {
    fn now_ms(&self) -> Result<i64, ManagementRuntimeError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        i64::try_from(elapsed.as_millis()).map_err(|_| ManagementRuntimeError::Unavailable)
    }
}

/// Exact non-secret binding target accepted by the runtime-management facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementRuntimeTarget {
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    upstream_model: Option<String>,
}

impl ManagementRuntimeTarget {
    /// Builds an exact binding target with an optional bounded model scope.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::InvalidInput`] for an empty or oversized model scope.
    pub fn try_new(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        upstream_model: Option<String>,
    ) -> Result<Self, ManagementRuntimeError> {
        if upstream_model
            .as_deref()
            .is_some_and(|model| model.is_empty() || model.chars().count() > 256)
        {
            return Err(ManagementRuntimeError::InvalidInput);
        }
        Ok(Self {
            endpoint_id,
            credential_id,
            upstream_model,
        })
    }

    /// Returns the exact Endpoint identity.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the exact Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns whether the caller supplied an upstream-model scope without exposing it to logs.
    #[must_use]
    pub const fn has_upstream_model_scope(&self) -> bool {
        self.upstream_model.is_some()
    }

    /// Returns the exact model scope to the injected runtime facade only.
    #[must_use]
    pub fn upstream_model(&self) -> Option<&str> {
        self.upstream_model.as_deref()
    }
}

/// Source-labelled freshness category for one safe Catalog observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementCatalogFreshness {
    /// The observed Catalog remains current.
    Fresh,
    /// The observed Catalog is older than the current freshness target.
    Stale,
    /// The observed Catalog has expired.
    Expired,
    /// No Catalog observation exists for the exact binding.
    Missing,
}

/// Value-free Catalog status for one exact Endpoint/Credential binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementCatalogStatus {
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    freshness: ManagementCatalogFreshness,
    observed_at_ms: i64,
}

impl ManagementCatalogStatus {
    /// Creates one bounded Catalog observation.
    #[must_use]
    pub const fn new(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        freshness: ManagementCatalogFreshness,
        observed_at_ms: i64,
    ) -> Self {
        Self {
            endpoint_id,
            credential_id,
            freshness,
            observed_at_ms,
        }
    }

    /// Returns the Endpoint identity.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns only the safe freshness category.
    #[must_use]
    pub const fn freshness(&self) -> ManagementCatalogFreshness {
        self.freshness
    }

    /// Returns the source observation time.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }
}

/// Safe scheduling availability for one exact Endpoint/Credential binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementRuntimeAvailability {
    /// The binding is currently eligible for ordinary scheduling.
    Available,
    /// A transient Health cooldown blocks the binding.
    Cooldown,
    /// A Health circuit blocks the binding.
    CircuitOpen,
    /// Quota blocks ordinary scheduling.
    QuotaBlocked,
    /// A provider-classified 403 blocks the exact account binding.
    CredentialForbidden,
    /// A controlled recovery remains required or in flight.
    RecoveryRequired,
}

/// Value-free runtime availability for one exact Endpoint/Credential binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementRuntimeAvailabilityStatus {
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    availability: ManagementRuntimeAvailability,
}

impl ManagementRuntimeAvailabilityStatus {
    /// Creates one exact value-free availability projection.
    #[must_use]
    pub const fn new(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        availability: ManagementRuntimeAvailability,
    ) -> Self {
        Self {
            endpoint_id,
            credential_id,
            availability,
        }
    }

    /// Returns the Endpoint identity.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the safe availability category.
    #[must_use]
    pub const fn availability(&self) -> ManagementRuntimeAvailability {
        self.availability
    }
}

/// The only responses to an operator's controlled quota-recovery request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementQuotaRecoveryState {
    /// A recovery is still required before a probe can be scheduled.
    RecoveryRequired,
    /// The runtime controller accepted a bounded recovery request for later handling.
    ProbeScheduled,
    /// The controller rejected the request without sending or completing a probe.
    Rejected,
}

/// One bounded, safe Route Explain request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementRouteExplainRequest {
    config_version_id: ConfigVersionId,
    route_id: RouteId,
    requested_model: String,
    protocol: ManagementRequestProtocol,
    provider_id: Option<ProviderId>,
    observed_at_ms: i64,
}

impl ManagementRouteExplainRequest {
    /// Creates a fixed-input Explain request.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::InvalidInput`] for an empty or oversized requested
    /// model before a runtime facade is called.
    pub fn try_new(
        config_version_id: ConfigVersionId,
        route_id: RouteId,
        requested_model: String,
        protocol: ManagementRequestProtocol,
        provider_id: Option<ProviderId>,
        observed_at_ms: i64,
    ) -> Result<Self, ManagementRuntimeError> {
        if requested_model.is_empty()
            || requested_model.chars().count() > 256
            || provider_id.as_ref().is_some_and(|provider_id| {
                provider_id.as_str().trim().is_empty() || provider_id.as_str().chars().count() > 128
            })
        {
            return Err(ManagementRuntimeError::InvalidInput);
        }
        Ok(Self {
            config_version_id,
            route_id,
            requested_model,
            protocol,
            provider_id,
            observed_at_ms,
        })
    }

    /// Returns the exact configuration identity whose immutable Route view is requested.
    #[must_use]
    pub const fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Returns the exact Route identity.
    #[must_use]
    pub const fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    /// Returns the requested public model to the injected immutable Route Explain facade only.
    #[must_use]
    pub fn requested_model(&self) -> &str {
        &self.requested_model
    }

    /// Returns the selected protocol without exposing a wire request.
    #[must_use]
    pub const fn protocol(&self) -> ManagementRequestProtocol {
        self.protocol
    }

    /// Returns the exact Provider scope requested by the operator.
    ///
    /// A missing scope is only admissible when the immutable Route contains one unique Provider;
    /// a runtime facade must not infer a fallback Provider from Candidate order.
    #[must_use]
    pub const fn provider_id(&self) -> Option<&ProviderId> {
        self.provider_id.as_ref()
    }

    /// Returns the fixed runtime observation time.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }
}

/// Closed management protocol enum used only for a Route Explain projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementRequestProtocol {
    /// `OpenAI` Chat Completions request semantics.
    OpenAiChatCompletions,
    /// `OpenAI` Responses request semantics.
    OpenAiResponses,
    /// Anthropic Messages request semantics.
    AnthropicMessages,
}

/// One safe candidate decision returned by a Route Explain facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementRouteExplainCandidate {
    candidate_id: RouteCandidateId,
    selected: bool,
    reason: Option<&'static str>,
    price_evidence: &'static str,
}

impl ManagementRouteExplainCandidate {
    /// Creates an exact selected candidate decision.
    #[must_use]
    pub const fn selected(candidate_id: RouteCandidateId) -> Self {
        Self {
            candidate_id,
            selected: true,
            reason: None,
            price_evidence: "disabled",
        }
    }

    /// Creates an excluded candidate with one closed, value-free reason code.
    #[must_use]
    pub const fn excluded(candidate_id: RouteCandidateId, reason: &'static str) -> Self {
        Self {
            candidate_id,
            selected: false,
            reason: Some(reason),
            price_evidence: "disabled",
        }
    }

    /// Adds one closed price-evidence category without changing selection semantics.
    #[must_use]
    pub const fn with_price_evidence(mut self, evidence: &'static str) -> Self {
        self.price_evidence = evidence;
        self
    }

    /// Returns the stable Candidate identity.
    #[must_use]
    pub const fn candidate_id(&self) -> &RouteCandidateId {
        &self.candidate_id
    }

    /// Returns whether the fixed projection selected this candidate.
    #[must_use]
    pub const fn selected_by_projection(&self) -> bool {
        self.selected
    }

    /// Returns a fixed diagnostic category, never a Provider diagnostic.
    #[must_use]
    pub const fn reason(&self) -> Option<&'static str> {
        self.reason
    }

    /// Returns the optional closed price-evidence category.
    #[must_use]
    pub const fn price_evidence(&self) -> &'static str {
        self.price_evidence
    }
}

/// Exact Config-Version-bound routing-price policy lineage shown by Route Explain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementRouteExplainPricePolicy {
    catalog_version_id: String,
    comparison: &'static str,
}

impl ManagementRouteExplainPricePolicy {
    /// Creates a value-free policy lineage projection.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::Unavailable`] when the catalog identity is blank or
    /// exceeds the closed 128-byte bound, or when the comparison is not a supported algorithm.
    pub fn new(
        catalog_version_id: String,
        comparison: &'static str,
    ) -> Result<Self, ManagementRuntimeError> {
        if catalog_version_id.trim().is_empty()
            || catalog_version_id.len() > 128
            || comparison != "rate_dominance_v1"
        {
            return Err(ManagementRuntimeError::Unavailable);
        }
        Ok(Self {
            catalog_version_id,
            comparison,
        })
    }

    /// Returns the immutable catalog identity.
    #[must_use]
    pub fn catalog_version_id(&self) -> &str {
        &self.catalog_version_id
    }

    /// Returns the closed comparison algorithm name.
    #[must_use]
    pub const fn comparison(&self) -> &'static str {
        self.comparison
    }
}

/// One complete bounded Route Explain projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementRouteExplain {
    route_id: RouteId,
    candidates: Vec<ManagementRouteExplainCandidate>,
    price_policy: Option<ManagementRouteExplainPricePolicy>,
}

impl ManagementRouteExplain {
    /// Creates a bounded Explain result.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::Unavailable`] when the injected facade exceeds the
    /// response bound, declares more than one selected Candidate, or supplies an invalid reason.
    pub fn try_new(
        route_id: RouteId,
        candidates: Vec<ManagementRouteExplainCandidate>,
    ) -> Result<Self, ManagementRuntimeError> {
        if candidates.len() > MAX_RUNTIME_ROWS
            || candidates
                .iter()
                .filter(|candidate| candidate.selected)
                .count()
                > 1
            || candidates.iter().any(|candidate| {
                candidate
                    .reason
                    .is_some_and(|reason| reason.is_empty() || reason.len() > 128)
            })
            || candidates.iter().any(|candidate| {
                !matches!(
                    candidate.price_evidence,
                    "dominant"
                        | "equal"
                        | "dominated"
                        | "incomparable"
                        | "unpriced"
                        | "not_evaluated"
                        | "disabled"
                )
            })
        {
            return Err(ManagementRuntimeError::Unavailable);
        }
        Ok(Self {
            route_id,
            candidates,
            price_policy: None,
        })
    }

    /// Adds the exact Config-Version-bound price policy lineage.
    #[must_use]
    pub fn with_price_policy(mut self, policy: ManagementRouteExplainPricePolicy) -> Self {
        self.price_policy = Some(policy);
        self
    }

    /// Returns the explained Route identity.
    #[must_use]
    pub const fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    /// Returns the bounded candidate decision list.
    #[must_use]
    pub fn candidates(&self) -> &[ManagementRouteExplainCandidate] {
        &self.candidates
    }

    /// Returns the optional immutable price-policy lineage.
    #[must_use]
    pub const fn price_policy(&self) -> Option<&ManagementRouteExplainPricePolicy> {
        self.price_policy.as_ref()
    }
}

/// Closed, value-free execution stage for a protected Attempt projection.
///
/// The stage says only where the gateway stopped. It intentionally carries no target, HTTP
/// status, error detail, request value, or response value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementRequestAttemptStage {
    /// The Canonical request was being converted for the selected upstream format.
    RequestConversion,
    /// The already-built outbound target was being admitted by egress policy.
    EgressAdmission,
    /// The admitted request was being sent through the upstream transport.
    HttpTransport,
    /// A transport response was being classified by its HTTP status class.
    HttpStatus,
    /// A success-class response was being checked for its expected content type.
    ContentType,
    /// A finite JSON response body was being read under the existing transport deadlines.
    BodyRead,
    /// A finite JSON response body was being decoded into Canonical events.
    Decoder,
    /// An SSE response was being bootstrapped into its first Canonical event source.
    SseBootstrap,
}

impl ManagementRequestAttemptStage {
    /// Returns the frozen wire category for this safe stage.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequestConversion => "request_conversion",
            Self::EgressAdmission => "egress_admission",
            Self::HttpTransport => "http_transport",
            Self::HttpStatus => "http_status",
            Self::ContentType => "content_type",
            Self::BodyRead => "body_read",
            Self::Decoder => "decoder",
            Self::SseBootstrap => "sse_bootstrap",
        }
    }
}

/// Value-free durable Attempt view for the protected management API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementRequestAttempt {
    attempt_id: String,
    outcome: &'static str,
    stage: Option<ManagementRequestAttemptStage>,
    endpoint_id: Option<EndpointId>,
    credential_id: Option<CredentialId>,
}

impl ManagementRequestAttempt {
    /// Creates a bounded Attempt view without model, route, timing, or Provider diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::Unavailable`] when a supplied safe projection cannot be
    /// represented within the frozen management bounds.
    pub fn try_new(
        attempt_id: String,
        outcome: &'static str,
        endpoint_id: Option<EndpointId>,
        credential_id: Option<CredentialId>,
    ) -> Result<Self, ManagementRuntimeError> {
        if attempt_id.is_empty()
            || attempt_id.chars().count() > 128
            || outcome.is_empty()
            || outcome.len() > 64
        {
            return Err(ManagementRuntimeError::Unavailable);
        }
        Ok(Self {
            attempt_id,
            outcome,
            stage: None,
            endpoint_id,
            credential_id,
        })
    }

    /// Adds the optional closed execution-stage projection.
    ///
    /// Existing embeddings that have only terminal Attempt outcomes can omit this field. A stage
    /// is an enum rather than a caller-provided string so the management response cannot gain an
    /// arbitrary diagnostic channel.
    #[must_use]
    pub const fn with_stage(mut self, stage: ManagementRequestAttemptStage) -> Self {
        self.stage = Some(stage);
        self
    }

    /// Returns the deterministic Attempt identity.
    #[must_use]
    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }

    /// Returns the closed terminal outcome category.
    #[must_use]
    pub const fn outcome(&self) -> &'static str {
        self.outcome
    }

    /// Returns the optional closed stage at which this Attempt reached its terminal outcome.
    #[must_use]
    pub const fn stage(&self) -> Option<ManagementRequestAttemptStage> {
        self.stage
    }

    /// Returns the exact Endpoint identity when persisted for this Attempt.
    #[must_use]
    pub const fn endpoint_id(&self) -> Option<&EndpointId> {
        self.endpoint_id.as_ref()
    }

    /// Returns the exact Credential identity when persisted for this Attempt.
    #[must_use]
    pub const fn credential_id(&self) -> Option<&CredentialId> {
        self.credential_id.as_ref()
    }
}

/// Safe, target-free failures from the P10-06 runtime facade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementRuntimeError {
    /// A request shape is invalid before the facade is called.
    InvalidInput,
    /// Runtime dependencies or an isolated state shard are unavailable.
    Unavailable,
}

/// The two upstream probe shapes accepted by the management-only Channel Pin operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementChannelPinMode {
    /// Ask the selected adapter for one bounded finite JSON response.
    Json,
    /// Ask the selected adapter for one bounded SSE response and drain it to completion.
    Sse,
}

impl ManagementChannelPinMode {
    /// Returns the closed wire category used by the management contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Sse => "sse",
        }
    }
}

/// Exact, value-free target supplied to the isolated Channel Pin executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementChannelPinRequest {
    config_version_id: ConfigVersionId,
    config_revision: ConfigRevision,
    provider_id: ProviderId,
    channel_id: EndpointId,
    route_id: RouteId,
    credential_id: CredentialId,
    requested_model: String,
    protocol: ManagementRequestProtocol,
    mode: ManagementChannelPinMode,
}

impl ManagementChannelPinRequest {
    /// Creates a request after the HTTP boundary has validated all opaque identifiers.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        config_version_id: ConfigVersionId,
        config_revision: ConfigRevision,
        provider_id: ProviderId,
        channel_id: EndpointId,
        route_id: RouteId,
        credential_id: CredentialId,
        requested_model: String,
        protocol: ManagementRequestProtocol,
        mode: ManagementChannelPinMode,
    ) -> Self {
        Self {
            config_version_id,
            config_revision,
            provider_id,
            channel_id,
            route_id,
            credential_id,
            requested_model,
            protocol,
            mode,
        }
    }

    /// Returns the selected Config Version.
    #[must_use]
    pub const fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Returns the exact revision admitted by the caller's `If-Match` precondition.
    #[must_use]
    pub const fn config_revision(&self) -> ConfigRevision {
        self.config_revision
    }

    /// Returns the explicit owning Provider identity.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns the exact Channel/Endpoint identity.
    #[must_use]
    pub const fn channel_id(&self) -> &EndpointId {
        &self.channel_id
    }

    /// Returns the exact Route identity.
    #[must_use]
    pub const fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    /// Returns the exact Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the bounded public model used by the fixed probe.
    #[must_use]
    pub fn requested_model(&self) -> &str {
        &self.requested_model
    }

    /// Returns the explicit client protocol used for adapter admission.
    #[must_use]
    pub const fn protocol(&self) -> ManagementRequestProtocol {
        self.protocol
    }

    /// Returns the upstream probe shape.
    #[must_use]
    pub const fn mode(&self) -> ManagementChannelPinMode {
        self.mode
    }
}

/// Closed terminal result for one Channel Pin attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementChannelPinOutcome {
    /// The bounded response completed its canonical lifecycle.
    Succeeded,
    /// The target was rejected before an upstream request was sent.
    Rejected,
    /// The one allowed upstream request was sent but failed its bounded lifecycle.
    Failed,
}

impl ManagementChannelPinOutcome {
    /// Returns the closed wire category.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }
}

/// Safe failure categories for a Channel Pin executor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementChannelPinError {
    /// The target was not admitted by the selected Config Version.
    InvalidTarget,
    /// The pinned target changed or the runtime snapshot is stale.
    SnapshotConflict,
    /// The isolated runtime executor is not available.
    Unavailable,
    /// The operation failed after the single upstream attempt.
    ExecutionFailed,
}

/// Value-free receipt returned by the Channel Pin executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementChannelPinReceipt {
    request_id: RequestId,
    config_version_id: ConfigVersionId,
    config_revision: ConfigRevision,
    provider_id: ProviderId,
    channel_id: EndpointId,
    route_id: RouteId,
    credential_id: CredentialId,
    requested_model: String,
    protocol: ManagementRequestProtocol,
    mode: ManagementChannelPinMode,
    outcome: ManagementChannelPinOutcome,
    upstream_sent: bool,
    attempt_count: u8,
    response_started: bool,
    observed_at_ms: i64,
    stage: Option<ManagementRequestAttemptStage>,
}

impl ManagementChannelPinReceipt {
    /// Creates a bounded receipt. `attempt_count` is deliberately limited to zero or one.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementChannelPinError::Unavailable`] when the receipt violates the
    /// zero-or-one attempt, sent/response, model, or timestamp bounds.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        request_id: RequestId,
        config_version_id: ConfigVersionId,
        config_revision: ConfigRevision,
        provider_id: ProviderId,
        channel_id: EndpointId,
        route_id: RouteId,
        credential_id: CredentialId,
        requested_model: String,
        protocol: ManagementRequestProtocol,
        mode: ManagementChannelPinMode,
        outcome: ManagementChannelPinOutcome,
        upstream_sent: bool,
        attempt_count: u8,
        response_started: bool,
        observed_at_ms: i64,
        stage: Option<ManagementRequestAttemptStage>,
    ) -> Result<Self, ManagementChannelPinError> {
        let outcome_valid = match outcome {
            ManagementChannelPinOutcome::Rejected => {
                attempt_count == 0 && !upstream_sent && !response_started && stage.is_none()
            }
            ManagementChannelPinOutcome::Succeeded => {
                attempt_count == 1 && upstream_sent && response_started
            }
            ManagementChannelPinOutcome::Failed => match attempt_count {
                0 => !upstream_sent && !response_started && stage.is_none(),
                1 => true,
                _ => false,
            },
        };
        if !outcome_valid
            || attempt_count > 1
            || (attempt_count == 0 && upstream_sent)
            || (!upstream_sent && response_started)
            || observed_at_ms < 0
            || requested_model.trim().is_empty()
            || requested_model.chars().count() > 256
        {
            return Err(ManagementChannelPinError::Unavailable);
        }
        Ok(Self {
            request_id,
            config_version_id,
            config_revision,
            provider_id,
            channel_id,
            route_id,
            credential_id,
            requested_model,
            protocol,
            mode,
            outcome,
            upstream_sent,
            attempt_count,
            response_started,
            observed_at_ms,
            stage,
        })
    }

    /// Returns the opaque request correlation identity.
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Returns the selected Config Version.
    #[must_use]
    pub const fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Returns the revision observed before execution.
    #[must_use]
    pub const fn config_revision(&self) -> ConfigRevision {
        self.config_revision
    }

    /// Returns the explicit Provider identity.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns the exact Channel identity.
    #[must_use]
    pub const fn channel_id(&self) -> &EndpointId {
        &self.channel_id
    }

    /// Returns the exact Route identity.
    #[must_use]
    pub const fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    /// Returns the exact Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the bounded public model used by the probe.
    #[must_use]
    pub fn requested_model(&self) -> &str {
        &self.requested_model
    }

    /// Returns the explicit probe protocol.
    #[must_use]
    pub const fn protocol(&self) -> ManagementRequestProtocol {
        self.protocol
    }

    /// Returns the requested upstream probe shape.
    #[must_use]
    pub const fn mode(&self) -> ManagementChannelPinMode {
        self.mode
    }

    /// Returns the terminal outcome.
    #[must_use]
    pub const fn outcome(&self) -> ManagementChannelPinOutcome {
        self.outcome
    }

    /// Returns whether the one allowed upstream request crossed the send boundary.
    #[must_use]
    pub const fn upstream_sent(&self) -> bool {
        self.upstream_sent
    }

    /// Returns the number of upstream attempts (zero or one).
    #[must_use]
    pub const fn attempt_count(&self) -> u8 {
        self.attempt_count
    }

    /// Returns whether a semantic response event was observed before the bounded drain ended.
    #[must_use]
    pub const fn response_started(&self) -> bool {
        self.response_started
    }

    /// Returns the non-secret observation timestamp captured before execution.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Returns the safe terminal stage, if known.
    #[must_use]
    pub const fn stage(&self) -> Option<ManagementRequestAttemptStage> {
        self.stage
    }
}

/// Boxed future used by the management-only Channel Pin seam.
pub type ManagementChannelPinFuture = Pin<
    Box<dyn Future<Output = Result<ManagementChannelPinReceipt, ManagementChannelPinError>> + Send>,
>;

/// Isolated executor for one exact management Channel Pin.
pub trait ManagementChannelPinFacade: Send + Sync {
    /// Executes at most one request for the exact target; the implementation owns all Provider
    /// handles and must never derive an endpoint, credential, or retry policy from free-form HTTP.
    fn execute(&self, request: ManagementChannelPinRequest) -> ManagementChannelPinFuture;
}

/// Fail-closed Channel Pin executor used until the serving composition injects the real one.
pub struct RejectingManagementChannelPinFacade;

impl RejectingManagementChannelPinFacade {
    /// Creates a no-send executor.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RejectingManagementChannelPinFacade {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagementChannelPinFacade for RejectingManagementChannelPinFacade {
    fn execute(&self, _request: ManagementChannelPinRequest) -> ManagementChannelPinFuture {
        Box::pin(async { Err(ManagementChannelPinError::Unavailable) })
    }
}

/// Explicit P10-06 runtime seam.
///
/// Implementations may read only pre-admitted runtime-management projections, immutable Route
/// Explain state, and value-free stored Attempts. They must not receive a Provider, URL, Header,
/// Body, Secret, lease, scheduling cursor, network client, or configuration-publishing handle.
pub trait ManagementRuntimeFacade: Send {
    /// Returns bounded source-labelled Catalog observations for an exact configuration graph.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::Unavailable`] when the required isolated runtime state
    /// cannot be read safely.
    fn catalog_status(
        &mut self,
        config_version_id: &ConfigVersionId,
        observed_at_ms: i64,
    ) -> Result<Vec<ManagementCatalogStatus>, ManagementRuntimeError>;

    /// Returns bounded Health/Quota/403 availability projections for an exact configuration graph.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::Unavailable`] when the required isolated runtime state
    /// cannot be read safely.
    fn runtime_availability(
        &mut self,
        config_version_id: &ConfigVersionId,
        observed_at_ms: i64,
    ) -> Result<Vec<ManagementRuntimeAvailabilityStatus>, ManagementRuntimeError>;

    /// Requests one controller-owned recovery decision for an exact binding target.
    ///
    /// An implementation may begin and complete a controlled local recovery transition in its
    /// injected runtime registries; it must never send a Provider request or read a Secret.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::Unavailable`] when no controller is safely available.
    fn request_quota_recovery(
        &mut self,
        config_version_id: &ConfigVersionId,
        target: &ManagementRuntimeTarget,
        observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError>;

    /// Returns a fixed-input, side-effect-free Route Explain projection.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::Unavailable`] when the immutable Route/runtime state
    /// cannot be read safely.
    fn explain_route(
        &mut self,
        request: &ManagementRouteExplainRequest,
    ) -> Result<ManagementRouteExplain, ManagementRuntimeError>;

    /// Returns bounded value-free attempts for one Request correlation.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementRuntimeError::Unavailable`] when the attempt store cannot safely
    /// provide a bounded projection.
    fn list_request_attempts(
        &mut self,
        request_id: &RequestId,
    ) -> Result<Vec<ManagementRequestAttempt>, ManagementRuntimeError>;
}

/// Fail-closed P10-06 facade used until an embedding injects the runtime dependencies.
pub struct RejectingManagementRuntimeFacade;

impl RejectingManagementRuntimeFacade {
    /// Creates a no-op, no-send runtime facade.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RejectingManagementRuntimeFacade {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagementRuntimeFacade for RejectingManagementRuntimeFacade {
    fn catalog_status(
        &mut self,
        _config_version_id: &ConfigVersionId,
        _observed_at_ms: i64,
    ) -> Result<Vec<ManagementCatalogStatus>, ManagementRuntimeError> {
        Err(ManagementRuntimeError::Unavailable)
    }

    fn runtime_availability(
        &mut self,
        _config_version_id: &ConfigVersionId,
        _observed_at_ms: i64,
    ) -> Result<Vec<ManagementRuntimeAvailabilityStatus>, ManagementRuntimeError> {
        Err(ManagementRuntimeError::Unavailable)
    }

    fn request_quota_recovery(
        &mut self,
        _config_version_id: &ConfigVersionId,
        _target: &ManagementRuntimeTarget,
        _observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        Err(ManagementRuntimeError::Unavailable)
    }

    fn explain_route(
        &mut self,
        _request: &ManagementRouteExplainRequest,
    ) -> Result<ManagementRouteExplain, ManagementRuntimeError> {
        Err(ManagementRuntimeError::Unavailable)
    }

    fn list_request_attempts(
        &mut self,
        _request_id: &RequestId,
    ) -> Result<Vec<ManagementRequestAttempt>, ManagementRuntimeError> {
        Err(ManagementRuntimeError::Unavailable)
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
    /// The short-lived authorization session expired before completion.
    Expired,
}

/// Value-free OAuth operation view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementCredentialOAuthOperation {
    /// Current OAuth workflow state.
    pub state: ManagementCredentialOAuthState,
    /// Optional finite expiry time; no token, URL, or verification material is included.
    pub expires_at_ms: Option<i64>,
    /// One-time browser authorization URL, returned only by start.
    pub authorization_url: Option<String>,
    /// Value-free failure category for an ended operation.
    pub failure_class: Option<&'static str>,
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

    /// Records a provider-side denial after the callback state has been verified.  A malformed or
    /// attacker-supplied state must not be allowed to terminate a legitimate pending session.
    fn reject_oauth(&mut self, _credential_id: &CredentialId, _state: &[u8]) -> bool {
        false
    }

    /// Completes one pending OAuth callback after state verification; no token is returned.
    fn complete_oauth(
        &mut self,
        _credential_id: &CredentialId,
        _state: &[u8],
        _authorization_code: Zeroizing<String>,
    ) -> Option<Zeroizing<Vec<u8>>> {
        None
    }

    /// Commits the session terminal state only after encrypted persistence has finished.
    fn finalize_oauth(&mut self, _credential_id: &CredentialId, _persisted: bool) -> bool {
        false
    }

    /// Refreshes an existing OAuth envelope outside the inference request path.
    fn refresh_oauth(
        &mut self,
        _credential_id: &CredentialId,
        _current_envelope: Zeroizing<Vec<u8>>,
        _now_ms: i64,
    ) -> Option<Zeroizing<Vec<u8>>> {
        None
    }
}

/// Injected authorization-code exchange and encrypted persistence boundary.
pub trait ManagementCodexOAuthExchange: Send {
    /// Exchanges one transient code and returns a validated, normalized CPAR envelope.
    fn exchange(
        &mut self,
        credential_id: &CredentialId,
        authorization_code: Zeroizing<String>,
        code_verifier: Zeroizing<Vec<u8>>,
    ) -> Option<Zeroizing<Vec<u8>>>;

    /// Refreshes an already imported OAuth envelope and returns a normalized replacement.
    fn refresh(
        &mut self,
        _credential_id: &CredentialId,
        _current_envelope: Zeroizing<Vec<u8>>,
        _now_ms: i64,
    ) -> Option<Zeroizing<Vec<u8>>> {
        None
    }
}

struct RejectingManagementCodexOAuthExchange;

impl ManagementCodexOAuthExchange for RejectingManagementCodexOAuthExchange {
    fn exchange(
        &mut self,
        _credential_id: &CredentialId,
        _authorization_code: Zeroizing<String>,
        _code_verifier: Zeroizing<Vec<u8>>,
    ) -> Option<Zeroizing<Vec<u8>>> {
        None
    }
}

struct ReqwestCodexOAuthTokenTransport {
    proxy: UpstreamProxy,
}

impl ReqwestCodexOAuthTokenTransport {
    fn new(proxy: UpstreamProxy) -> Self {
        Self { proxy }
    }
}

fn codex_oauth_http_client(
    proxy: &UpstreamProxy,
) -> Result<reqwest::blocking::Client, CodexOAuthTransportError> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(CODEX_OAUTH_HTTP_TIMEOUT)
        .user_agent(CODEX_OAUTH_USER_AGENT)
        // OAuth must not inherit an operator's ambient HTTP(S)_PROXY.  If a proxy is desired it
        // is supplied explicitly through the validated local-DNS SOCKS5 process option below.
        .no_proxy();
    if let Some(proxy_url) = proxy.canonical_url() {
        let admitted =
            reqwest::Proxy::all(proxy_url).map_err(|_| CodexOAuthTransportError::Unavailable)?;
        builder = builder.proxy(admitted);
    }
    builder
        .build()
        .map_err(|_| CodexOAuthTransportError::Unavailable)
}

impl CodexOAuthTokenTransport for ReqwestCodexOAuthTokenTransport {
    fn post_form(
        &mut self,
        url: &str,
        body: Zeroizing<String>,
    ) -> Result<Zeroizing<Vec<u8>>, CodexOAuthTransportError> {
        let client = codex_oauth_http_client(&self.proxy)?;
        let response = client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            // `reqwest` owns the request body; keep the caller's Zeroizing buffer scoped to
            // construction and pass only the encoded bytes to the transport.
            .body(body.as_bytes().to_vec())
            .send()
            .map_err(|_| CodexOAuthTransportError::Unavailable)?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|length| length > MAX_CODEX_OAUTH_RESPONSE_BYTES)
        {
            return Err(CodexOAuthTransportError::Rejected);
        }
        let body = response
            .bytes()
            .map_err(|_| CodexOAuthTransportError::Unavailable)?;
        if body.len() as u64 > MAX_CODEX_OAUTH_RESPONSE_BYTES {
            return Err(CodexOAuthTransportError::InvalidResponse);
        }
        Ok(Zeroizing::new(body.to_vec()))
    }
}

/// Real Codex authorization-code exchange used by the production composition root.
pub struct OpenAiCodexOAuthExchange {
    proxy: UpstreamProxy,
}

impl OpenAiCodexOAuthExchange {
    /// Creates an exchange with an explicitly admitted direct or local-DNS SOCKS5 egress.
    #[must_use]
    pub const fn new(proxy: UpstreamProxy) -> Self {
        Self { proxy }
    }
}

#[derive(Clone, Copy)]
enum CodexOAuthResponseError {
    InvalidCredentialShape,
}

fn normalize_codex_oauth_response(
    body: &[u8],
    now_ms: i64,
) -> Result<Zeroizing<Vec<u8>>, CodexOAuthResponseError> {
    let credential = OpenAiCompatibleRuntimeCredential::import_oauth_token_response(body, now_ms)
        .map_err(|_| CodexOAuthResponseError::InvalidCredentialShape)?;
    credential
        .export_json(CodexCredentialExportFormat::Cpa)
        .map_err(|_| CodexOAuthResponseError::InvalidCredentialShape)
}

impl ManagementCodexOAuthExchange for OpenAiCodexOAuthExchange {
    fn exchange(
        &mut self,
        _credential_id: &CredentialId,
        authorization_code: Zeroizing<String>,
        code_verifier: Zeroizing<Vec<u8>>,
    ) -> Option<Zeroizing<Vec<u8>>> {
        let verifier = String::from_utf8(code_verifier.to_vec()).ok()?;
        let form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "authorization_code")
            .append_pair("code", authorization_code.as_str())
            .append_pair("redirect_uri", "http://localhost:1455/auth/callback")
            .append_pair(
                "client_id",
                provider_openai_compatible::CODEX_OAUTH_CLIENT_ID,
            )
            .append_pair("code_verifier", &verifier)
            .finish();
        let client = codex_oauth_http_client(&self.proxy).ok()?;
        let Ok(response) = client
            .post(provider_openai_compatible::CODEX_OAUTH_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .body(form)
            .send()
        else {
            return None;
        };
        if !response.status().is_success() {
            return None;
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_CODEX_OAUTH_RESPONSE_BYTES)
        {
            return None;
        }
        let Ok(body) = response.bytes() else {
            return None;
        };
        if body.len() as u64 > MAX_CODEX_OAUTH_RESPONSE_BYTES {
            return None;
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|value| i64::try_from(value.as_millis()).ok())?;
        normalize_codex_oauth_response(&body, now_ms).ok()
    }

    fn refresh(
        &mut self,
        _credential_id: &CredentialId,
        current_envelope: Zeroizing<Vec<u8>>,
        now_ms: i64,
    ) -> Option<Zeroizing<Vec<u8>>> {
        let credential = OpenAiCompatibleRuntimeCredential::import_compatible(
            current_envelope.as_slice(),
            now_ms,
        )
        .ok()?;
        let revisioned = CodexOAuthRevisionedCredential::new(credential, 0).ok()?;
        let mut coordinator = CodexOAuthRefreshCoordinator::new(
            revisioned,
            ReqwestCodexOAuthTokenTransport::new(self.proxy.clone()),
        );
        coordinator.refresh(now_ms).ok()?;
        coordinator
            .credential()
            .export_json(CodexCredentialExportFormat::Cpa)
            .ok()
    }
}

/// Default fail-closed P10-04 workflow. It never contacts a Provider or creates OAuth material.
pub struct RejectingManagementEndpointWorkflow {
    oauth: BTreeMap<CredentialId, ManagementCredentialOAuthOperation>,
}

/// Backend OAuth workflow using the replay-safe Codex session state machine.
pub struct CodexOAuthManagementWorkflow {
    oauth: BTreeMap<CredentialId, CodexOAuthSession>,
    exchange: Box<dyn ManagementCodexOAuthExchange>,
}

impl CodexOAuthManagementWorkflow {
    /// Creates an empty workflow; token exchange is intentionally injected later.
    #[must_use]
    pub fn new() -> Self {
        Self {
            oauth: BTreeMap::new(),
            exchange: Box::new(RejectingManagementCodexOAuthExchange),
        }
    }

    /// Creates a workflow with one explicitly admitted exchange implementation.
    #[must_use]
    pub fn with_exchange(exchange: Box<dyn ManagementCodexOAuthExchange>) -> Self {
        Self {
            oauth: BTreeMap::new(),
            exchange,
        }
    }

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_millis()).ok())
            .unwrap_or(0)
    }
}

fn authorization_url(session: &CodexOAuthSession) -> String {
    let Ok((state, verifier)) = session.transient_challenge() else {
        return "https://auth.openai.com/oauth/authorize".to_owned();
    };
    let state = URL_SAFE_NO_PAD.encode(state);
    // The session keeps raw random bytes internally; OAuth requires the printable base64url
    // verifier string to be hashed and later sent to the token endpoint.
    let verifier = URL_SAFE_NO_PAD.encode(verifier);
    let challenge = URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
    // Keep this shape aligned with the incumbent Codex login flow.  The browser callback is
    // deliberately loopback: the machine that opened the browser must receive the code, then
    // forward only the bounded `state` + `code` to the protected management callback.  A public
    // CPAR hostname is not a registered OAuth redirect and causes an immediate provider error.
    format!(
        "https://auth.openai.com/oauth/authorize?response_type=code&client_id={}&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&scope=openid%20profile%20email%20offline_access%20api.connectors.read%20api.connectors.invoke&state={state}&code_challenge={challenge}&code_challenge_method=S256&prompt=login&id_token_add_organizations=true&codex_cli_simplified_flow=true&originator=codex_cli_rs",
        provider_openai_compatible::CODEX_OAUTH_CLIENT_ID
    )
}

impl Default for CodexOAuthManagementWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagementEndpointWorkflow for CodexOAuthManagementWorkflow {
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
        let now = Self::now_ms();
        if let Some(session) = self.oauth.get_mut(credential_id) {
            let view = session.view(now);
            if view.state == CodexOAuthSessionState::Pending {
                // Starting twice must be idempotent.  CPA/Sub2API keep one session per flow;
                // returning the same challenge avoids invalidating a browser that is already
                // on the authorization page (and prevents two simultaneous OAuth windows).
                return ManagementCredentialOAuthOperation {
                    state: ManagementCredentialOAuthState::Pending,
                    expires_at_ms: Some(view.expires_at_ms),
                    authorization_url: Some(authorization_url(session)),
                    failure_class: None,
                };
            }
        }
        let result = CodexOAuthSession::start(credential_id.clone(), now);
        match result {
            Ok(mut session) => {
                let view = session.view(now);
                let authorization = authorization_url(&session);
                self.oauth.insert(credential_id.clone(), session);
                ManagementCredentialOAuthOperation {
                    state: ManagementCredentialOAuthState::Pending,
                    expires_at_ms: Some(view.expires_at_ms),
                    authorization_url: Some(authorization),
                    failure_class: None,
                }
            }
            Err(_) => ManagementCredentialOAuthOperation {
                state: ManagementCredentialOAuthState::Failed,
                expires_at_ms: None,
                authorization_url: None,
                failure_class: Some("session_start_failed"),
            },
        }
    }

    fn oauth_status(&mut self, credential_id: &CredentialId) -> ManagementCredentialOAuthOperation {
        let Some(session) = self.oauth.get_mut(credential_id) else {
            return ManagementCredentialOAuthOperation {
                state: ManagementCredentialOAuthState::Failed,
                expires_at_ms: None,
                authorization_url: None,
                failure_class: Some("session_missing"),
            };
        };
        let view = session.view(Self::now_ms());
        ManagementCredentialOAuthOperation {
            state: match view.state {
                CodexOAuthSessionState::Pending => ManagementCredentialOAuthState::Pending,
                CodexOAuthSessionState::Complete => ManagementCredentialOAuthState::Complete,
                CodexOAuthSessionState::Cancelled => ManagementCredentialOAuthState::Cancelled,
                CodexOAuthSessionState::Expired => ManagementCredentialOAuthState::Expired,
                CodexOAuthSessionState::Failed => ManagementCredentialOAuthState::Failed,
            },
            expires_at_ms: Some(view.expires_at_ms),
            authorization_url: None,
            failure_class: view.failure_class,
        }
    }

    fn cancel_oauth(&mut self, credential_id: &CredentialId) {
        if let Some(session) = self.oauth.get_mut(credential_id) {
            let _ = session.cancel(Self::now_ms());
        }
    }

    fn reject_oauth(&mut self, credential_id: &CredentialId, state: &[u8]) -> bool {
        let now = Self::now_ms();
        let Some(session) = self.oauth.get_mut(credential_id) else {
            return false;
        };
        if !session.verify_state(state) {
            return false;
        }
        session.fail(now, "provider_rejected").is_ok()
    }

    fn complete_oauth(
        &mut self,
        credential_id: &CredentialId,
        state: &[u8],
        authorization_code: Zeroizing<String>,
    ) -> Option<Zeroizing<Vec<u8>>> {
        let now = Self::now_ms();
        let verifier = {
            let session = self.oauth.get_mut(credential_id)?;
            if !session.verify_state(state) {
                // Do not consume/terminate the session on a mismatched state.  Otherwise an
                // unsolicited callback can deny service to the browser holding the legitimate
                // state.  The caller still receives a value-free rejection and may submit the
                // correct callback once it arrives.
                return None;
            }
            if session.claim_completion(now).is_err() {
                return None;
            }
            Zeroizing::new(
                URL_SAFE_NO_PAD
                    .encode(session.transient_verifier())
                    .into_bytes(),
            )
        };
        let envelope = self
            .exchange
            .exchange(credential_id, authorization_code, verifier);
        if envelope.is_none() {
            if let Some(session) = self.oauth.get_mut(credential_id) {
                let _ = session.fail(now, "token_exchange_failed");
            }
            return None;
        }
        envelope
    }

    fn finalize_oauth(&mut self, credential_id: &CredentialId, persisted: bool) -> bool {
        let now = Self::now_ms();
        let Some(session) = self.oauth.get_mut(credential_id) else {
            return false;
        };
        if persisted {
            session.complete(now).is_ok()
        } else {
            session.fail(now, "persistence_failed").is_ok()
        }
    }

    fn refresh_oauth(
        &mut self,
        credential_id: &CredentialId,
        current_envelope: Zeroizing<Vec<u8>>,
        now_ms: i64,
    ) -> Option<Zeroizing<Vec<u8>>> {
        self.exchange
            .refresh(credential_id, current_envelope, now_ms)
    }
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
            authorization_url: None,
            failure_class: None,
        };
        self.oauth.insert(credential_id.clone(), operation.clone());
        operation
    }

    fn oauth_status(&mut self, credential_id: &CredentialId) -> ManagementCredentialOAuthOperation {
        self.oauth
            .get(credential_id)
            .cloned()
            .unwrap_or(ManagementCredentialOAuthOperation {
                state: ManagementCredentialOAuthState::Failed,
                expires_at_ms: None,
                authorization_url: None,
                failure_class: Some("session_missing"),
            })
    }

    fn cancel_oauth(&mut self, credential_id: &CredentialId) {
        self.oauth.insert(
            credential_id.clone(),
            ManagementCredentialOAuthOperation {
                state: ManagementCredentialOAuthState::Cancelled,
                expires_at_ms: None,
                authorization_url: None,
                failure_class: None,
            },
        );
    }

    fn reject_oauth(&mut self, credential_id: &CredentialId, _state: &[u8]) -> bool {
        if let Some(operation) = self.oauth.get_mut(credential_id) {
            operation.state = ManagementCredentialOAuthState::Failed;
            operation.failure_class = Some("provider_rejected");
            return true;
        }
        false
    }
}

/// Mounts P10-04 resource routes inside the P10-02 protected `/admin` scope.
pub fn configure_management_resources(config: &mut web::ServiceConfig) {
    configure_management(config, resource_routes);
}

fn resource_routes(config: &mut web::ServiceConfig) {
    configure_upstream_resource_routes(config);
    configure_routing_resource_routes(config);
    configure_runtime_resource_routes(config);
    configure_operations_resource_routes(config);
    configure_billing_resource_routes(config);
}

/// Registers only the P10 resource paths inside an already protected management scope.
///
/// The P12 listener composition uses this with lifecycle and backup paths under one `/admin`
/// scope, so Actix cannot stop route resolution at the first sibling scope.
pub(crate) fn configure_protected_resource_routes(config: &mut web::ServiceConfig) {
    resource_routes(config);
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
            "/credentials/{credential_id}/oauth/callback",
            web::post().to(complete_credential_oauth),
        )
        .route(
            "/credentials/{credential_id}/oauth/refresh",
            web::post().to(refresh_credential_oauth),
        )
        .route(
            "/credentials/{credential_id}/export",
            web::post().to(export_credential),
        )
        .route(
            "/credentials/{credential_id}/metadata",
            web::get().to(get_credential_metadata),
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

fn configure_runtime_resource_routes(config: &mut web::ServiceConfig) {
    config
        .route("/catalog/status", web::get().to(get_catalog_status))
        .route(
            "/runtime/availability",
            web::get().to(get_runtime_availability),
        )
        .route(
            "/runtime/quota/reset",
            web::post().to(request_quota_recovery),
        )
        .route("/routes/{route_id}/explain", web::get().to(explain_route))
        .route(
            "/requests/{request_id}/attempts",
            web::get().to(list_request_attempts),
        );
}

fn configure_operations_resource_routes(config: &mut web::ServiceConfig) {
    config
        .route(
            "/compatible-proxy-pools",
            web::get().to(list_compatible_proxy_pools),
        )
        .route(
            "/compatible-proxy-pools",
            web::post().to(create_compatible_proxy_pool),
        )
        .route(
            "/compatible-proxy-pools/{pool_id}",
            web::get().to(get_compatible_proxy_pool),
        )
        .route(
            "/compatible-proxy-pools/{pool_id}",
            web::patch().to(update_compatible_proxy_pool),
        )
        .route(
            "/compatible-proxy-pools/{pool_id}",
            web::delete().to(delete_compatible_proxy_pool),
        )
        .route(
            "/compatible-proxy-nodes",
            web::get().to(list_compatible_proxy_nodes),
        )
        .route(
            "/compatible-proxy-nodes",
            web::post().to(create_compatible_proxy_node),
        )
        .route(
            "/compatible-proxy-nodes/{node_id}",
            web::get().to(get_compatible_proxy_node),
        )
        .route(
            "/compatible-proxy-nodes/{node_id}",
            web::patch().to(update_compatible_proxy_node),
        )
        .route(
            "/compatible-proxy-nodes/{node_id}",
            web::delete().to(delete_compatible_proxy_node),
        )
        .route(
            "/compatible-egress-bindings",
            web::get().to(list_compatible_egress_bindings),
        )
        .route(
            "/compatible-egress-bindings",
            web::post().to(create_compatible_egress_binding),
        )
        .route(
            "/compatible-egress-bindings/{endpoint_id}/{credential_id}",
            web::get().to(get_compatible_egress_binding),
        )
        .route(
            "/compatible-egress-bindings/{endpoint_id}/{credential_id}",
            web::patch().to(update_compatible_egress_binding),
        )
        .route(
            "/compatible-egress-bindings/{endpoint_id}/{credential_id}",
            web::delete().to(delete_compatible_egress_binding),
        )
        .route(
            "/operations/channel-pin",
            web::post().to(execute_channel_pin),
        )
        .route(
            "/operations/account-pools",
            web::get().to(list_operational_account_pools),
        )
        .route(
            "/operations/provider-account-pools",
            web::get().to(list_provider_account_pools),
        )
        .route(
            "/operations/provider-account-pools/actions",
            web::post().to(apply_provider_account_pool_action),
        )
        .route(
            "/operations/provider-account-pools/failures",
            web::get().to(list_provider_account_failures),
        )
        .route(
            "/operations/provider-egress-status",
            web::get().to(list_provider_egress_status),
        )
        .route("/operations/usage", web::get().to(list_operational_usage));
    config.route(
        "/operations/billing",
        web::get().to(list_operational_billing),
    );
}

fn configure_billing_resource_routes(config: &mut web::ServiceConfig) {
    config
        .route("/billing/catalogs", web::get().to(list_billing_catalogs))
        .route("/billing/catalogs", web::post().to(import_billing_catalog))
        .route(
            "/billing/catalogs/{catalog_version_id}/rollback",
            web::post().to(rollback_billing_catalog),
        )
        .route(
            "/billing/routing-price-policy",
            web::get().to(get_routing_price_policy),
        )
        .route(
            "/billing/routing-price-policy",
            web::put().to(set_routing_price_policy),
        )
        .route(
            "/billing/routing-price-policy",
            web::delete().to(clear_routing_price_policy),
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
struct CompatibleProxyPoolInput {
    id: String,
    upstream_id: String,
    name: String,
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompatibleProxyNodeInput {
    id: String,
    upstream_id: String,
    pool_id: Option<String>,
    name: String,
    proxy_endpoint: Option<String>,
    enabled: bool,
    weight: i64,
    maximum_concurrency: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompatibleEgressBindingInput {
    endpoint_id: String,
    credential_id: String,
    target_kind: String,
    target_id: Option<String>,
    failure_scope: String,
    stickiness: String,
    pre_submit_max_attempts: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndpointTestInput {
    mode: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelPinInput {
    provider_id: String,
    channel_id: String,
    route_id: String,
    credential_id: String,
    requested_model: String,
    protocol: String,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeTargetInput {
    endpoint_id: String,
    credential_id: String,
    upstream_model: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteExplainQuery {
    requested_model: String,
    protocol: String,
    provider_id: Option<String>,
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
struct CompatibleProxyPoolResponse {
    id: String,
    upstream_id: String,
    name: String,
    enabled: bool,
}

#[derive(Serialize)]
struct CompatibleProxyNodeResponse {
    id: String,
    upstream_id: String,
    pool_id: Option<String>,
    name: String,
    enabled: bool,
    weight: u16,
    maximum_concurrency: u32,
    proxy_configured: bool,
}

#[derive(Serialize)]
struct CompatibleEgressBindingResponse {
    endpoint_id: String,
    credential_id: String,
    target_kind: &'static str,
    target_id: Option<String>,
    failure_scope: &'static str,
    stickiness: &'static str,
    pre_submit_max_attempts: u8,
}

impl From<CompatibleProxyPoolView> for CompatibleProxyPoolResponse {
    fn from(value: CompatibleProxyPoolView) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            upstream_id: value.upstream_id.as_str().to_owned(),
            name: value.name,
            enabled: value.enabled,
        }
    }
}

impl From<CompatibleProxyNodeView> for CompatibleProxyNodeResponse {
    fn from(value: CompatibleProxyNodeView) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            upstream_id: value.upstream_id.as_str().to_owned(),
            pool_id: value.pool_id.map(|id| id.as_str().to_owned()),
            name: value.name,
            enabled: value.enabled,
            weight: value.weight,
            maximum_concurrency: value.maximum_concurrency,
            proxy_configured: value.proxy_configured,
        }
    }
}

impl From<CompatibleEgressBindingView> for CompatibleEgressBindingResponse {
    fn from(value: CompatibleEgressBindingView) -> Self {
        let (target_kind, target_id) = match value.target {
            CompatibleEgressTargetView::Direct => ("direct", None),
            CompatibleEgressTargetView::FixedProxy(id) => {
                ("fixed_proxy", Some(id.as_str().to_owned()))
            }
            CompatibleEgressTargetView::ProxyPool(id) => {
                ("proxy_pool", Some(id.as_str().to_owned()))
            }
        };
        Self {
            endpoint_id: value.endpoint_id.as_str().to_owned(),
            credential_id: value.credential_id.as_str().to_owned(),
            target_kind,
            target_id,
            failure_scope: compatible_failure_scope_str(value.failure_scope),
            stickiness: compatible_stickiness_str(value.stickiness),
            pre_submit_max_attempts: value.pre_submit_max_attempts,
        }
    }
}

#[derive(Serialize)]
struct EndpointTestResponse {
    outcome: &'static str,
    status_class: &'static str,
    canonical_lifecycle: bool,
}

#[derive(Serialize)]
struct ChannelPinResponse {
    request_id: String,
    config_version_id: String,
    config_revision: i64,
    provider_id: String,
    channel_id: String,
    route_id: String,
    credential_id: String,
    requested_model: String,
    protocol: &'static str,
    mode: &'static str,
    outcome: &'static str,
    upstream_sent: bool,
    attempt_count: u8,
    response_started: bool,
    observed_at_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<&'static str>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    authorization_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_class: Option<&'static str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialOAuthCallbackRequest {
    #[serde(default)]
    state: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    callback_url: Option<String>,
}

struct ParsedOAuthCallback {
    state: String,
    code: String,
}

#[derive(Clone)]
enum OAuthCallbackInputError {
    Invalid,
    ProviderRejected { state: Option<String> },
}

/// Accepts either the relay's `{state, code}` pair or a copied loopback callback URL.
///
/// CPA/Sub2API both tolerate users pasting the complete browser callback.  Parsing it here keeps
/// the management API independent of a particular local relay implementation while never making
/// a network request or retaining the URL after this bounded handler call.
fn parse_oauth_callback_request(
    payload: &CredentialOAuthCallbackRequest,
) -> Result<ParsedOAuthCallback, OAuthCallbackInputError> {
    let explicit_state = payload.state.trim();
    if payload
        .callback_url
        .as_deref()
        .is_some_and(|value| value.len() > 20 * 1024)
        || payload
            .error
            .as_deref()
            .is_some_and(|value| value.len() > 256)
    {
        return Err(OAuthCallbackInputError::Invalid);
    }
    if payload
        .error
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        let state = (!explicit_state.is_empty() && explicit_state.len() <= 512)
            .then(|| explicit_state.to_owned());
        return Err(OAuthCallbackInputError::ProviderRejected { state });
    }
    let source = payload
        .callback_url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(payload.code.as_str())
        .trim();
    let looks_like_url = source.contains("://")
        || source.starts_with('?')
        || (source.contains('=') && (source.contains('&') || source.starts_with("code=")));
    let (mut code, mut state) = if looks_like_url {
        let normalized = if source.starts_with('?') {
            format!("http://localhost/{source}")
        } else if source.contains("://") {
            source.to_owned()
        } else {
            format!("http://{source}")
        };
        let url = url::Url::parse(&normalized).map_err(|_| OAuthCallbackInputError::Invalid)?;
        let mut code = url
            .query_pairs()
            .find(|(key, _)| key == "code")
            .map(|(_, value)| value.into_owned());
        let mut state = url
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned());
        let provider_rejected = url
            .query_pairs()
            .any(|(key, _)| key == "error" || key == "error_description");
        let fragment = url.fragment().unwrap_or_default();
        for (key, value) in url::form_urlencoded::parse(fragment.as_bytes()) {
            match key.as_ref() {
                "code" if code.is_none() => code = Some(value.into_owned()),
                "state" if state.is_none() => state = Some(value.into_owned()),
                "error" | "error_description" => {
                    return Err(OAuthCallbackInputError::ProviderRejected { state });
                }
                _ => {}
            }
        }
        if provider_rejected {
            return Err(OAuthCallbackInputError::ProviderRejected { state });
        }
        let code = code.unwrap_or_default();
        let state = state.unwrap_or_default();
        (code, state)
    } else {
        (source.to_owned(), explicit_state.to_owned())
    };
    if code.contains('#')
        && state.is_empty()
        && let Some((candidate, candidate_state)) = code
            .split_once('#')
            .map(|(candidate, candidate_state)| (candidate.to_owned(), candidate_state.to_owned()))
    {
        code = candidate;
        state = candidate_state;
    }
    if !explicit_state.is_empty() && !state.is_empty() && explicit_state != state {
        return Err(OAuthCallbackInputError::Invalid);
    }
    if state.is_empty() {
        explicit_state.clone_into(&mut state);
    }
    if code.is_empty() || state.is_empty() || state.len() > 512 || code.len() > 16 * 1024 {
        return Err(OAuthCallbackInputError::Invalid);
    }
    Ok(ParsedOAuthCallback { state, code })
}

fn decode_oauth_state(input: &[u8]) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(input)
        .or_else(|_| URL_SAFE.decode(input))
        .or_else(|_| STANDARD_NO_PAD.decode(input))
        .or_else(|_| STANDARD.decode(input))
        .ok()
}

#[derive(Deserialize)]
struct CredentialExportRequest {
    format: String,
}

#[derive(Serialize)]
struct CredentialMetadataResponse {
    credential_id: String,
    kind: String,
    revision: i64,
    plan: Option<String>,
    quota: Option<String>,
    platform: Option<String>,
    email: Option<String>,
    source_format: Option<String>,
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

#[derive(Serialize)]
struct CatalogStatusResponse {
    endpoint_id: String,
    credential_id: String,
    freshness: &'static str,
    observed_at_ms: i64,
}

#[derive(Serialize)]
struct RuntimeAvailabilityResponse {
    endpoint_id: String,
    credential_id: String,
    availability: &'static str,
}

#[derive(Serialize)]
struct RuntimeActionResponse {
    state: &'static str,
}

#[derive(Serialize)]
struct RouteExplainResponse {
    route_id: String,
    candidates: Vec<RouteExplainCandidateResponse>,
    price_policy: Option<RouteExplainPricePolicyResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct RouteExplainCandidateResponse {
    candidate_id: String,
    decision: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    price_evidence: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct RouteExplainPricePolicyResponse {
    catalog_version_id: String,
    comparison: &'static str,
}

#[derive(Serialize)]
struct RequestAttemptResponse {
    attempt_id: String,
    outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationalAccountPoolQueryParams {
    provider_id: Option<String>,
    channel_id: Option<String>,
    account_status: Option<String>,
    enabled: Option<bool>,
    limit: Option<usize>,
    cursor: Option<String>,
}

#[derive(Serialize)]
struct OperationalAccountPoolPageResponse {
    config_version_id: String,
    revision: i64,
    items: Vec<OperationalAccountPoolItemResponse>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct OperationalAccountPoolItemResponse {
    provider_id: String,
    provider_name: String,
    provider_kind: String,
    provider_enabled: bool,
    egress_policy_id: Option<String>,
    channel_id: String,
    adapter_id: String,
    api_format: String,
    transport: &'static str,
    channel_enabled: bool,
    account_id: String,
    account_kind: String,
    account_status: &'static str,
    account_revision: i64,
    binding_enabled: bool,
    configured_enabled: bool,
    priority: i64,
    weight: i64,
    concurrency: i64,
    route_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderAccountPoolQueryParams {
    provider_id: Option<String>,
    channel_id: Option<String>,
    auth_status: Option<String>,
    runtime_status: Option<String>,
    enabled: Option<bool>,
    limit: Option<usize>,
    cursor: Option<String>,
}

#[derive(Serialize)]
struct ProviderAccountPoolPageResponse {
    snapshot_id: String,
    observed_at_ms: i64,
    items: Vec<ProviderAccountPoolItemResponse>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderEgressStatusQueryParams {
    provider_id: Option<String>,
    upstream_id: Option<String>,
    channel_id: Option<String>,
    domain: Option<String>,
    state: Option<String>,
    credential_id: Option<String>,
    limit: Option<usize>,
    cursor: Option<String>,
}

#[derive(Serialize)]
struct ProviderEgressStatusPageResponse {
    config_version_id: String,
    config_revision: i64,
    runtime_revision: u64,
    snapshot_id: String,
    sampled_at_ms: i64,
    items: Vec<ProviderEgressStatusItemResponse>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "domain")]
enum ProviderEgressStatusItemResponse {
    #[serde(rename = "egress")]
    Egress {
        provider_id: String,
        upstream_id: String,
        channel_id: String,
        channel_kind: &'static str,
        target_kind: &'static str,
        target_id: Option<String>,
        state: &'static str,
        deadline_ms: Option<i64>,
    },
    #[serde(rename = "session")]
    Session {
        provider_id: String,
        upstream_id: String,
        channel_id: String,
        channel_kind: &'static str,
        credential_id: String,
        credential_revision: u64,
        session_revision: u64,
        state: &'static str,
        expires_at_ms: Option<i64>,
    },
    #[serde(rename = "clearance")]
    Clearance {
        provider_id: String,
        upstream_id: String,
        channel_id: String,
        channel_kind: &'static str,
        credential_id: String,
        credential_revision: u64,
        session_revision: u64,
        target_kind: &'static str,
        target_id: Option<String>,
        clearance_revision: u64,
        state: &'static str,
        expires_at_ms: Option<i64>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderEgressStatusCursorWire {
    config_version_id: String,
    config_revision: i64,
    runtime_revision: u64,
    snapshot_id: String,
    sampled_at_ms: i64,
    filter_fingerprint: String,
    last_key: ProviderEgressStatusCursorKeyWire,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderEgressStatusCursorKeyWire {
    provider_id: String,
    upstream_id: String,
    channel_id: String,
    domain: String,
    credential_id: Option<String>,
    credential_revision: Option<u64>,
    session_revision: Option<u64>,
    target_kind: Option<String>,
    target_id: Option<String>,
    clearance_revision: Option<u64>,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct ProviderAccountPoolItemResponse {
    provider_id: String,
    channel_id: String,
    account_id: String,
    account_kind: String,
    auth_status: &'static str,
    runtime_status: &'static str,
    enabled: bool,
    priority: i64,
    weight: u32,
    max_concurrency: u32,
    active_leases: u32,
    expires_at_ms: Option<i64>,
    refresh_due_at_ms: Option<i64>,
    quota_sync_due_at_ms: Option<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderAccountPoolCursorWire {
    snapshot_id: String,
    filter_fingerprint: String,
    provider_id: String,
    channel_id: String,
    account_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderAccountOperatorActionInput {
    provider_id: String,
    channel_id: String,
    account_id: String,
    action: String,
    upstream_model: Option<String>,
    cooldown_ms: Option<i64>,
}

#[derive(Serialize)]
struct ProviderAccountOperatorActionResponse {
    state: &'static str,
    observed_at_ms: i64,
    cooldown_until_ms: Option<i64>,
}

impl From<ProviderAccountOperatorReceipt> for ProviderAccountOperatorActionResponse {
    fn from(value: ProviderAccountOperatorReceipt) -> Self {
        Self {
            state: value.state.as_str(),
            observed_at_ms: value.observed_at_ms,
            cooldown_until_ms: value.cooldown_until_ms,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureFeedbackQueryParams {
    provider_id: Option<String>,
    channel_id: Option<String>,
    account_id: Option<String>,
    limit: Option<usize>,
    cursor: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureFeedbackCursorWire {
    ordinal: i64,
    filter_fingerprint: String,
}

#[derive(Serialize)]
struct FailureFeedbackPageResponse {
    observed_through_ordinal: Option<i64>,
    items: Vec<FailureFeedbackItemResponse>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct FailureFeedbackItemResponse {
    provider_id: String,
    channel_id: String,
    account_id: String,
    request_id: String,
    attempt_id: String,
    ended_at_ms: i64,
    error_code: &'static str,
    error_scope: &'static str,
    retry_decision: &'static str,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationalAccountPoolCursorWire {
    config_version_id: String,
    revision: i64,
    provider_id: String,
    channel_id: String,
    account_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationalUsageQueryParams {
    from_ms: Option<i64>,
    to_ms: Option<i64>,
    provider_id: Option<String>,
    channel_id: Option<String>,
    account_id: Option<String>,
    model: Option<String>,
    client_key_id: Option<String>,
    access_group_id: Option<String>,
    protocol: Option<String>,
    limit: Option<usize>,
    cursor: Option<String>,
}

#[derive(Serialize)]
struct OperationalUsagePageResponse {
    observed_through_ms: Option<i64>,
    items: Vec<OperationalUsageItemResponse>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct OperationalUsageItemResponse {
    provider_id: String,
    channel_id: String,
    account_id: String,
    public_model: String,
    protocol: &'static str,
    client_key_id: String,
    access_group_id: Option<String>,
    request_count: u64,
    usage_observations: u64,
    input_tokens: OperationalTokenMetricResponse,
    output_tokens: OperationalTokenMetricResponse,
    reasoning_tokens: OperationalTokenMetricResponse,
    cache_read_tokens: OperationalTokenMetricResponse,
    cache_creation_tokens: OperationalTokenMetricResponse,
    cached_tokens: OperationalTokenMetricResponse,
    observed_at_ms: i64,
    cost_microunits: Option<u64>,
    cost_confidence: &'static str,
}

#[derive(Serialize)]
struct OperationalTokenMetricResponse {
    total: Option<u64>,
    confidence: &'static str,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationalUsageCursorWire {
    provider_id: String,
    channel_id: String,
    account_id: String,
    public_model: String,
    protocol: String,
    client_key_id: String,
    access_group_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationalBillingQueryParams {
    from_ms: Option<u64>,
    to_ms: Option<u64>,
    provider_id: Option<String>,
    channel_id: Option<String>,
    account_id: Option<String>,
    model: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
    cursor: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationalBillingCursorWire {
    snapshot_ledger_id: i64,
    occurred_at_ms: u64,
    ledger_id: i64,
}

#[derive(Serialize)]
struct OperationalBillingPageResponse {
    snapshot_ledger_id: Option<i64>,
    items: Vec<OperationalBillingItemResponse>,
    summary: OperationalBillingSummaryResponse,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct OperationalBillingItemResponse {
    ledger_id: i64,
    request_id: String,
    response_id: String,
    provider_id: String,
    channel_id: String,
    account_id: String,
    model: String,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_creation_tokens: Option<u64>,
    cached_tokens: Option<u64>,
    occurred_at_ms: u64,
    catalog_version_id: Option<String>,
    cost_microunits: Option<u64>,
    cost_confidence: &'static str,
}

#[derive(Serialize)]
struct OperationalBillingSummaryResponse {
    records: u64,
    exact_records: u64,
    partial_records: u64,
    unknown_records: u64,
    unpriced_records: u64,
    known_cost_microunits: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BillingCatalogImportInput {
    catalog_version_id: String,
    effective_at_ms: u64,
    source: String,
    entries: Vec<BillingCatalogEntryInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BillingCatalogEntryInput {
    provider_id: String,
    channel_id: String,
    model: String,
    input_microunits_per_million: u64,
    output_microunits_per_million: u64,
    reasoning_microunits_per_million: u64,
    cache_read_microunits_per_million: u64,
    cache_creation_microunits_per_million: u64,
    cached_microunits_per_million: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BillingCatalogRollbackInput {
    new_catalog_version_id: String,
    effective_at_ms: u64,
}

#[derive(Serialize)]
struct BillingCatalogResponse {
    catalog_version_id: String,
    effective_at_ms: u64,
    source: &'static str,
    created_at_ms: u64,
    entries: Vec<BillingCatalogEntryResponse>,
}

#[derive(Serialize)]
struct BillingCatalogEntryResponse {
    provider_id: String,
    channel_id: String,
    model: String,
    input_microunits_per_million: u64,
    output_microunits_per_million: u64,
    reasoning_microunits_per_million: u64,
    cache_read_microunits_per_million: u64,
    cache_creation_microunits_per_million: u64,
    cached_microunits_per_million: u64,
}

#[derive(Serialize)]
struct BillingCatalogMutationResponse {
    catalog_version_id: String,
    effective_at_ms: u64,
    source: &'static str,
    entry_count: usize,
    operation: &'static str,
    rolled_back_from: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutingPricePolicyInput {
    catalog_version_id: String,
    comparison: String,
}

#[derive(Serialize)]
struct RoutingPricePolicyResponse {
    catalog_version_id: String,
    comparison: &'static str,
}

#[allow(clippy::too_many_lines)]
async fn list_operational_usage(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let params = match web::Query::<OperationalUsageQueryParams>::from_query(request.query_string())
    {
        Ok(params) => params.into_inner(),
        Err(_) => return invalid_input(),
    };
    let cursor = match params.cursor.as_deref() {
        Some(value) if value.len() <= 2048 => decode_operational_usage_cursor(value),
        Some(_) => Err(ManagementOperationsError::InvalidQuery),
        None => Ok(None),
    };
    let cursor = match cursor {
        Ok(cursor) => cursor,
        Err(error) => return management_error(ManagementResourceError::from(error)),
    };
    let provider_id = match usage_query_id(params.provider_id) {
        Ok(value) => value
            .map(|value| UpstreamId::try_new(value).map_err(|_| invalid_input()))
            .transpose(),
        Err(response) => return response,
    };
    let provider_id = match provider_id {
        Ok(value) => value,
        Err(response) => return response,
    };
    let channel_id = match usage_query_id(params.channel_id) {
        Ok(value) => value
            .map(|value| EndpointId::try_new(value).map_err(|_| invalid_input()))
            .transpose(),
        Err(response) => return response,
    };
    let channel_id = match channel_id {
        Ok(value) => value,
        Err(response) => return response,
    };
    let account_id = match usage_query_id(params.account_id) {
        Ok(value) => value
            .map(|value| CredentialId::try_new(value).map_err(|_| invalid_input()))
            .transpose(),
        Err(response) => return response,
    };
    let account_id = match account_id {
        Ok(value) => value,
        Err(response) => return response,
    };
    let client_key_id = match usage_query_id(params.client_key_id) {
        Ok(value) => value
            .map(|value| ClientKeyId::try_new(value).map_err(|_| invalid_input()))
            .transpose(),
        Err(response) => return response,
    };
    let client_key_id = match client_key_id {
        Ok(value) => value,
        Err(response) => return response,
    };
    let access_group_id = match usage_query_id(params.access_group_id) {
        Ok(value) => value
            .map(|value| AccessGroupId::try_new(value).map_err(|_| invalid_input()))
            .transpose(),
        Err(response) => return response,
    };
    let access_group_id = match access_group_id {
        Ok(value) => value,
        Err(response) => return response,
    };
    let model = match params.model {
        Some(value) => match bounded_text(value, MAX_USAGE_MODEL_CHARS) {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let protocol = match params.protocol {
        Some(value) => match operational_usage_protocol(&value) {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let limit = params.limit.unwrap_or(DEFAULT_USAGE_LIMIT);
    if limit == 0 || limit > MAX_USAGE_LIMIT {
        return invalid_input();
    }
    let query = match OperationalUsageQuery::try_new(
        params.from_ms,
        params.to_ms,
        provider_id,
        channel_id,
        account_id,
        model,
        client_key_id,
        access_group_id,
        protocol,
        limit,
        cursor,
    ) {
        Ok(query) => query,
        Err(error) => return management_error(ManagementResourceError::from(error)),
    };
    let usage_source = match usage(&state) {
        Ok(source) => source,
        Err(response) => return response,
    };
    match usage_source.list_usage(&query) {
        Ok(page) => match operational_usage_page_response(page) {
            Ok(response) => HttpResponse::Ok()
                .insert_header((header::CACHE_CONTROL, "no-store"))
                .json(response),
            Err(response) => response,
        },
        Err(error) => management_error(ManagementResourceError::from(error)),
    }
}

#[allow(clippy::too_many_lines)]
async fn list_operational_billing(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let params =
        match web::Query::<OperationalBillingQueryParams>::from_query(request.query_string()) {
            Ok(params) => params.into_inner(),
            Err(_) => return invalid_input(),
        };
    let cursor = match params.cursor.as_deref() {
        Some(value) if value.len() <= 2048 => decode_operational_billing_cursor(value),
        Some(_) => Err(ManagementOperationsError::InvalidQuery),
        None => Ok(None),
    };
    let cursor = match cursor {
        Ok(cursor) => cursor,
        Err(error) => return management_error(ManagementResourceError::from(error)),
    };
    let limit = params.limit.unwrap_or(DEFAULT_USAGE_LIMIT);
    if limit == 0 || limit > MAX_USAGE_LIMIT {
        return invalid_input();
    }
    let status = match params.status.as_deref() {
        Some(value) => match operational_billing_status(value) {
            Ok(status) => Some(status),
            Err(response) => return response,
        },
        None => None,
    };
    let provider_id = match params.provider_id {
        Some(value) => match operational_query_id(value) {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let channel_id = match params.channel_id {
        Some(value) => match operational_query_id(value) {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let account_id = match params.account_id {
        Some(value) => match operational_query_id(value) {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let model = match params.model {
        Some(value) => match bounded_text(value, MAX_USAGE_MODEL_CHARS) {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let query = match OperationalBillingQuery::try_new(
        params.from_ms,
        params.to_ms,
        provider_id,
        channel_id,
        account_id,
        model,
        status,
        limit,
        cursor,
    ) {
        Ok(query) => query,
        Err(error) => return management_error(ManagementResourceError::from(error)),
    };
    let source = match usage(&state) {
        Ok(source) => source,
        Err(response) => return response,
    };
    match source.list_billing(&query) {
        Ok(page) => match operational_billing_page_response(page) {
            Ok(response) => HttpResponse::Ok()
                .insert_header((header::CACHE_CONTROL, "no-store"))
                .json(response),
            Err(response) => response,
        },
        Err(error) => management_error(ManagementResourceError::from(error)),
    }
}

async fn list_billing_catalogs(
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
    match service.list_billing_catalogs(&context.version) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |catalogs| {
            catalogs
                .into_iter()
                .map(billing_catalog_response)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn get_routing_price_policy(
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
    match service.get_routing_price_policy(&context.version) {
        Ok(value) => revisioned_json(StatusCode::OK, value, routing_price_policy_response),
        Err(error) => management_error(error),
    }
}

async fn set_routing_price_policy(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input =
        match parse_json::<RoutingPricePolicyInput>(&body).and_then(routing_price_policy_input) {
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
    match service.set_routing_price_policy(&actor, &context.version, context.revision, input) {
        Ok(value) => revisioned_json(StatusCode::OK, value, routing_price_policy_response),
        Err(error) => management_error(error),
    }
}

async fn clear_routing_price_policy(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
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
    match service.clear_routing_price_policy(&actor, &context.version, context.revision) {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

async fn import_billing_catalog(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input =
        match parse_json::<BillingCatalogImportInput>(&body).and_then(billing_catalog_import) {
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
    match service.import_billing_catalog(&actor, &context.version, context.revision, input) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, |value| {
            billing_catalog_mutation_response(&value)
        }),
        Err(error) => management_error(error),
    }
}

async fn rollback_billing_catalog(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let predecessor_version_id = match bounded_text(path.into_inner(), 128) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let input = match parse_json::<BillingCatalogRollbackInput>(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    if input.effective_at_ms > MAX_BILLING_JSON_INTEGER {
        return invalid_input();
    }
    let new_catalog_version_id = match bounded_text(input.new_catalog_version_id, 128) {
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
    match service.rollback_billing_catalog(
        &actor,
        &context.version,
        context.revision,
        &predecessor_version_id,
        new_catalog_version_id,
        input.effective_at_ms,
    ) {
        Ok(value) => revisioned_json(StatusCode::CREATED, value, |value| {
            billing_catalog_mutation_response(&value)
        }),
        Err(error) => management_error(error),
    }
}

async fn list_operational_account_pools(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let params =
        match web::Query::<OperationalAccountPoolQueryParams>::from_query(request.query_string()) {
            Ok(params) => params.into_inner(),
            Err(_) => return invalid_input(),
        };
    let cursor = match params.cursor.as_deref() {
        Some(value) if value.len() <= 2048 => decode_operational_account_pool_cursor(value),
        Some(_) => Err(ManagementOperationsError::InvalidQuery),
        None => Ok(None),
    };
    let cursor = match cursor {
        Ok(cursor) => cursor,
        Err(error) => return management_error(ManagementResourceError::from(error)),
    };
    let provider_id = match params.provider_id {
        Some(value) => match operational_query_id(value)
            .and_then(|value| UpstreamId::try_new(value).map_err(|_| invalid_input()))
        {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let channel_id = match params.channel_id {
        Some(value) => match operational_query_id(value)
            .and_then(|value| EndpointId::try_new(value).map_err(|_| invalid_input()))
        {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let account_status = match params.account_status {
        Some(value) => match operational_account_status(&value) {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let limit = params.limit.unwrap_or(DEFAULT_ACCOUNT_POOL_LIMIT);
    if limit == 0 || limit > MAX_ACCOUNT_POOL_LIMIT {
        return invalid_input();
    }
    let query = match OperationalAccountPoolQuery::try_new(
        provider_id,
        channel_id,
        account_status,
        params.enabled,
        limit,
        cursor,
    ) {
        Ok(query) => query,
        Err(error) => return management_error(ManagementResourceError::from(error)),
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.list_operational_account_pools(&context.version, &query) {
        Ok(value) => {
            let (page, revision) = value.into_parts();
            match operational_account_pool_page_response(page) {
                Ok(response) => response_with_revision(StatusCode::OK, revision, response),
                Err(response) => response,
            }
        }
        Err(error) => management_error(error),
    }
}

#[allow(clippy::too_many_lines)]
async fn list_provider_account_pools(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let params =
        match web::Query::<ProviderAccountPoolQueryParams>::from_query(request.query_string()) {
            Ok(params) => params.into_inner(),
            Err(_) => return invalid_input(),
        };
    let provider_id = match params.provider_id {
        Some(value) => match operational_query_id(value)
            .and_then(|value| ProviderId::try_new(value).map_err(|_| invalid_input()))
        {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let channel_id = match params.channel_id {
        Some(value) => match operational_query_id(value)
            .and_then(|value| EndpointId::try_new(value).map_err(|_| invalid_input()))
        {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let auth_status = match params.auth_status.as_deref() {
        Some(value) => match provider_account_auth_status(value) {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let runtime_status = match params.runtime_status.as_deref() {
        Some(value) => match provider_account_runtime_status(value) {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let cursor = match params.cursor.as_deref() {
        Some(value) if value.len() <= 2048 => decode_provider_account_pool_cursor(value),
        Some(_) => Err(ProviderAccountPoolError::InvalidQuery),
        None => Ok(None),
    };
    let cursor = match cursor {
        Ok(cursor) => cursor,
        Err(error) => return provider_account_pool_error(error),
    };
    let limit = params.limit.unwrap_or(DEFAULT_PROVIDER_ACCOUNT_POOL_LIMIT);
    let query = match ProviderAccountPoolQuery::try_new(
        provider_id,
        channel_id,
        auth_status,
        runtime_status,
        params.enabled,
        limit,
        cursor,
    ) {
        Ok(query) => query,
        Err(error) => return provider_account_pool_error(error),
    };
    let source = match provider_account_pools(&state) {
        Ok(source) => source,
        Err(response) => return response,
    };
    match source.list_provider_account_pools(&query) {
        Ok(page) => match provider_account_pool_page_response(page) {
            Ok(response) => HttpResponse::Ok()
                .insert_header((header::CACHE_CONTROL, "no-store"))
                .json(response),
            Err(response) => response,
        },
        Err(error) => provider_account_pool_error(error),
    }
}

#[allow(clippy::too_many_lines)]
async fn list_provider_egress_status(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if query_has_duplicate_keys(request.query_string()) {
        return invalid_input();
    }
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let params =
        match web::Query::<ProviderEgressStatusQueryParams>::from_query(request.query_string()) {
            Ok(params) => params.into_inner(),
            Err(_) => return invalid_input(),
        };
    let provider_id = match params.provider_id {
        Some(value) => match operational_query_id(value)
            .and_then(|value| ProviderId::try_new(value).map_err(|_| invalid_input()))
        {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let upstream_id = match params.upstream_id {
        Some(value) => match operational_query_id(value)
            .and_then(|value| UpstreamId::try_new(value).map_err(|_| invalid_input()))
        {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let channel_id = match params.channel_id {
        Some(value) => match operational_query_id(value)
            .and_then(|value| EndpointId::try_new(value).map_err(|_| invalid_input()))
        {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let credential_id = match params.credential_id {
        Some(value) => match operational_query_id(value)
            .and_then(|value| CredentialId::try_new(value).map_err(|_| invalid_input()))
        {
            Ok(value) => Some(value),
            Err(response) => return response,
        },
        None => None,
    };
    let domain = match params.domain.as_deref() {
        Some(value) => match value.parse::<ProviderEgressStatusDomain>() {
            Ok(value) => Some(value),
            Err(_) => return invalid_input(),
        },
        None => None,
    };
    let status = match params.state.as_deref() {
        Some(value) => match value.parse::<ProviderEgressStatusState>() {
            Ok(value) => Some(value),
            Err(_) => return invalid_input(),
        },
        None => None,
    };
    let cursor = match params.cursor.as_deref() {
        Some(value)
            if !value.is_empty() && value.len() <= MAX_PROVIDER_EGRESS_STATUS_CURSOR_LENGTH =>
        {
            decode_provider_egress_status_cursor(value)
        }
        Some(_) => Err(ProviderEgressStatusError::InvalidQuery),
        None => Ok(None),
    };
    let cursor = match cursor {
        Ok(cursor) => cursor,
        Err(error) => return provider_egress_status_error(error),
    };
    let query = match ProviderEgressStatusQuery::try_new(
        provider_id,
        upstream_id,
        channel_id,
        domain,
        status,
        credential_id,
        params.limit.unwrap_or(DEFAULT_PROVIDER_EGRESS_STATUS_LIMIT),
        cursor,
    ) {
        Ok(query) => query,
        Err(error) => return provider_egress_status_error(error),
    };
    let revision = {
        let mut management_service = match service(&state) {
            Ok(service) => service,
            Err(response) => return response,
        };
        match management_service.require_config_version(&context.version) {
            Ok(revision) => revision,
            Err(error) => return management_error(error),
        }
    };
    let source = match provider_egress_status(&state) {
        Ok(source) => source,
        Err(response) => return response,
    };
    match source.list_provider_egress_status(&context.version, revision, &query) {
        Ok(page) => match provider_egress_status_page_response(page) {
            Ok(response) => response_with_revision(StatusCode::OK, revision, response),
            Err(response) => response,
        },
        Err(error) => provider_egress_status_error(error),
    }
}

async fn apply_provider_account_pool_action(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input = match parse_json::<ProviderAccountOperatorActionInput>(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Ok(provider_id) = ProviderId::try_new(input.provider_id) else {
        return invalid_input();
    };
    let Ok(channel_id) = EndpointId::try_new(input.channel_id) else {
        return invalid_input();
    };
    let Ok(account_id) = CredentialId::try_new(input.account_id) else {
        return invalid_input();
    };
    let kind = match input.action.as_str() {
        "cool_down" => ProviderAccountOperatorActionKind::CoolDown,
        "request_recovery" => ProviderAccountOperatorActionKind::RequestRecovery,
        _ => return invalid_input(),
    };
    let action = match ProviderAccountOperatorAction::try_new(
        context.version.as_str().to_owned(),
        provider_id,
        channel_id,
        account_id,
        input.upstream_model,
        kind,
        input.cooldown_ms,
    ) {
        Ok(value) => value,
        Err(error) => return provider_account_pool_error(error),
    };
    let observed_at_ms = match runtime_observed_at(&state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let audit_action = match kind {
        ProviderAccountOperatorActionKind::CoolDown => "provider_account_cool_down_requested",
        ProviderAccountOperatorActionKind::RequestRecovery => "provider_account_recovery_requested",
    };
    let resource_id = provider_account_action_resource_id(&action);
    let mut management_service = match service(&state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = management_service.record_resource_action(
        &actor,
        &context.version,
        audit_action,
        "provider_account",
        &resource_id,
    ) {
        return management_error(error);
    }
    let action_result = match provider_account_pools(&state) {
        Ok(source) => source.apply_operator_action(&action, observed_at_ms),
        Err(response) => return response,
    };
    match action_result {
        Ok(receipt) => HttpResponse::Accepted()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(ProviderAccountOperatorActionResponse::from(receipt)),
        Err(error) => provider_account_pool_error(error),
    }
}

async fn list_provider_account_failures(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let version_admission = match service(&state) {
        Ok(mut service) => service.require_config_version(&context.version),
        Err(response) => return response,
    };
    if let Err(error) = version_admission {
        return management_error(error);
    }
    let params = match web::Query::<FailureFeedbackQueryParams>::from_query(request.query_string())
    {
        Ok(params) => params.into_inner(),
        Err(_) => return invalid_input(),
    };
    let provider_id = match params.provider_id {
        Some(value) => match UpstreamId::try_new(value) {
            Ok(value) => Some(value),
            Err(_) => return invalid_input(),
        },
        None => None,
    };
    let channel_id = match params.channel_id {
        Some(value) => match EndpointId::try_new(value) {
            Ok(value) => Some(value),
            Err(_) => return invalid_input(),
        },
        None => None,
    };
    let account_id = match params.account_id {
        Some(value) => match CredentialId::try_new(value) {
            Ok(value) => Some(value),
            Err(_) => return invalid_input(),
        },
        None => None,
    };
    let cursor = match params.cursor.as_deref() {
        Some(value) if value.len() <= 2048 => decode_failure_feedback_cursor(value),
        Some(_) => Err(ManagementOperationsError::InvalidQuery),
        None => Ok(None),
    };
    let cursor = match cursor {
        Ok(cursor) => cursor,
        Err(error) => return management_error(ManagementResourceError::from(error)),
    };
    let limit = params.limit.unwrap_or(DEFAULT_FAILURE_FEEDBACK_LIMIT);
    if !(1..=MAX_FAILURE_FEEDBACK_LIMIT).contains(&limit) {
        return invalid_input();
    }
    let query =
        match FailureFeedbackQuery::try_new(provider_id, channel_id, account_id, limit, cursor) {
            Ok(query) => query,
            Err(error) => return management_error(ManagementResourceError::from(error)),
        };
    let source = match failure_feedback(&state) {
        Ok(source) => source,
        Err(response) => return response,
    };
    match source.list_failure_feedback(&query) {
        Ok(page) => match failure_feedback_page_response(page) {
            Ok(response) => HttpResponse::Ok()
                .insert_header((header::CACHE_CONTROL, "no-store"))
                .json(response),
            Err(response) => response,
        },
        Err(error) => management_error(ManagementResourceError::from(error)),
    }
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

async fn list_compatible_proxy_pools(
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
    match service.list_compatible_proxy_pools(&context.version) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |pools| {
            pools
                .into_iter()
                .map(CompatibleProxyPoolResponse::from)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn get_compatible_proxy_pool(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(pool_id) = CompatibleProxyPoolId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_compatible_proxy_pool(&context.version, &pool_id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, CompatibleProxyPoolResponse::from),
        Err(error) => management_error(error),
    }
}

async fn create_compatible_proxy_pool(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input: CompatibleProxyPoolInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let pool = match compatible_proxy_pool(input) {
        Ok(pool) => pool,
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
    match service.create_compatible_proxy_pool(&actor, &context.version, context.revision, &pool) {
        Ok(value) => revisioned_json(
            StatusCode::CREATED,
            value,
            CompatibleProxyPoolResponse::from,
        ),
        Err(error) => management_error(error),
    }
}

async fn update_compatible_proxy_pool(
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
    let input: CompatibleProxyPoolInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    if input.id != path_id {
        return invalid_input();
    }
    let pool = match compatible_proxy_pool(input) {
        Ok(pool) => pool,
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
    match service.update_compatible_proxy_pool(&actor, &context.version, context.revision, &pool) {
        Ok(value) => revisioned_json(StatusCode::OK, value, CompatibleProxyPoolResponse::from),
        Err(error) => management_error(error),
    }
}

async fn delete_compatible_proxy_pool(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(pool_id) = CompatibleProxyPoolId::try_new(path.into_inner()) else {
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
    match service.delete_compatible_proxy_pool(&actor, &context.version, context.revision, &pool_id)
    {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

async fn list_compatible_proxy_nodes(
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
    match service.list_compatible_proxy_nodes(&context.version) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |nodes| {
            nodes
                .into_iter()
                .map(CompatibleProxyNodeResponse::from)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn get_compatible_proxy_node(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(node_id) = CompatibleProxyNodeId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_compatible_proxy_node(&context.version, &node_id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, CompatibleProxyNodeResponse::from),
        Err(error) => management_error(error),
    }
}

async fn create_compatible_proxy_node(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input: CompatibleProxyNodeInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let node = match compatible_proxy_node(input) {
        Ok(node) => node,
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
    match service.create_compatible_proxy_node(&actor, &context.version, context.revision, &node) {
        Ok(value) => revisioned_json(
            StatusCode::CREATED,
            value,
            CompatibleProxyNodeResponse::from,
        ),
        Err(error) => management_error(error),
    }
}

async fn update_compatible_proxy_node(
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
    let input: CompatibleProxyNodeInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    if input.id != path_id {
        return invalid_input();
    }
    let node = match compatible_proxy_node(input) {
        Ok(node) => node,
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
    match service.update_compatible_proxy_node(&actor, &context.version, context.revision, &node) {
        Ok(value) => revisioned_json(StatusCode::OK, value, CompatibleProxyNodeResponse::from),
        Err(error) => management_error(error),
    }
}

async fn delete_compatible_proxy_node(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(node_id) = CompatibleProxyNodeId::try_new(path.into_inner()) else {
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
    match service.delete_compatible_proxy_node(&actor, &context.version, context.revision, &node_id)
    {
        Ok(revision) => empty_with_revision(revision),
        Err(error) => management_error(error),
    }
}

async fn list_compatible_egress_bindings(
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
    match service.list_compatible_egress_bindings(&context.version) {
        Ok(value) => revisioned_json(StatusCode::OK, value, |bindings| {
            bindings
                .into_iter()
                .map(CompatibleEgressBindingResponse::from)
                .collect::<Vec<_>>()
        }),
        Err(error) => management_error(error),
    }
}

async fn get_compatible_egress_binding(
    request: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let (endpoint_raw, credential_raw) = path.into_inner();
    let Ok(endpoint_id) = EndpointId::try_new(endpoint_raw) else {
        return invalid_input();
    };
    let Ok(credential_id) = CredentialId::try_new(credential_raw) else {
        return invalid_input();
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.get_compatible_egress_binding(&context.version, &endpoint_id, &credential_id) {
        Ok(value) => revisioned_json(StatusCode::OK, value, CompatibleEgressBindingResponse::from),
        Err(error) => management_error(error),
    }
}

async fn create_compatible_egress_binding(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input: CompatibleEgressBindingInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let binding = match compatible_egress_binding(input) {
        Ok(binding) => binding,
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
    match service.create_compatible_egress_binding(
        &actor,
        &context.version,
        context.revision,
        &binding,
    ) {
        Ok(value) => revisioned_json(
            StatusCode::CREATED,
            value,
            CompatibleEgressBindingResponse::from,
        ),
        Err(error) => management_error(error),
    }
}

async fn update_compatible_egress_binding(
    request: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let (endpoint_raw, credential_raw) = path.into_inner();
    let input: CompatibleEgressBindingInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    if input.endpoint_id != endpoint_raw || input.credential_id != credential_raw {
        return invalid_input();
    }
    let binding = match compatible_egress_binding(input) {
        Ok(binding) => binding,
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
    match service.update_compatible_egress_binding(
        &actor,
        &context.version,
        context.revision,
        &binding,
    ) {
        Ok(value) => revisioned_json(StatusCode::OK, value, CompatibleEgressBindingResponse::from),
        Err(error) => management_error(error),
    }
}

async fn delete_compatible_egress_binding(
    request: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let (endpoint_raw, credential_raw) = path.into_inner();
    let Ok(endpoint_id) = EndpointId::try_new(endpoint_raw) else {
        return invalid_input();
    };
    let Ok(credential_id) = CredentialId::try_new(credential_raw) else {
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
    match service.delete_compatible_egress_binding(
        &actor,
        &context.version,
        context.revision,
        &endpoint_id,
        &credential_id,
    ) {
        Ok(revision) => empty_with_revision(revision),
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

/// Executes one protected, exact-target Channel Pin.  The handler validates only graph identity;
/// the injected facade owns lease acquisition, transport, and the no-retry execution boundary.
#[allow(clippy::too_many_lines)]
async fn execute_channel_pin(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input: ChannelPinInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let Ok(provider_id) = ProviderId::try_new(input.provider_id) else {
        return invalid_input();
    };
    let Ok(channel_id) = EndpointId::try_new(input.channel_id) else {
        return invalid_input();
    };
    let Ok(route_id) = RouteId::try_new(input.route_id) else {
        return invalid_input();
    };
    let Ok(credential_id) = CredentialId::try_new(input.credential_id) else {
        return invalid_input();
    };
    if input.requested_model.trim().is_empty() || input.requested_model.chars().count() > 256 {
        return invalid_input();
    }
    let protocol = match management_request_protocol(&input.protocol) {
        Ok(protocol) => protocol,
        Err(response) => return response,
    };
    let mode = match input.mode.as_str() {
        "json" => ManagementChannelPinMode::Json,
        "sse" => ManagementChannelPinMode::Sse,
        _ => return invalid_input(),
    };

    // Record the operator intent before graph admission so a well-formed but rejected target
    // remains auditable. This is a value-free resource action and does not advance Config
    // Version revision or authorize the executor.
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    {
        let mut management_service = match service(&state) {
            Ok(value) => value,
            Err(response) => return response,
        };
        if let Err(error) = management_service.record_resource_action(
            &actor,
            &context.version,
            "channel_pin_requested",
            "channel_pin",
            route_id.as_str(),
        ) {
            return management_error(error);
        }
    }

    // Validate the complete identity relation from one selected Config Version before handing
    // anything to the executor.  This prevents a facade from becoming an alternate config
    // lookup or cross-Provider fallback mechanism.
    let revision = {
        let mut management_service = match service(&state) {
            Ok(value) => value,
            Err(response) => return response,
        };
        let endpoint = match management_service.get_endpoint(&context.version, &channel_id) {
            Ok(value) => value.into_parts().0,
            Err(error) => return management_error(error),
        };
        let credential = match management_service.get_credential(&context.version, &credential_id) {
            Ok(value) => value.into_parts().0,
            Err(error) => return management_error(error),
        };
        let Ok(upstream_id) = UpstreamId::try_new(provider_id.as_str().to_owned()) else {
            return invalid_input();
        };
        if endpoint.upstream_id != upstream_id || credential.upstream_id != upstream_id {
            return invalid_input();
        }
        if let Err(error) = management_service.get_upstream(&context.version, &upstream_id) {
            return management_error(error);
        }
        if let Err(error) = management_service.get_model_route(&context.version, &route_id) {
            return management_error(error);
        }
        let bindings = match management_service
            .list_endpoint_credential_bindings(&context.version, &channel_id)
        {
            Ok(value) => value.into_parts().0,
            Err(error) => return management_error(error),
        };
        if !bindings.iter().any(|binding| {
            binding.credential_id == credential_id && binding.upstream_id == upstream_id
        }) {
            return invalid_input();
        }
        match management_service.require_config_version(&context.version) {
            Ok(revision) if revision == context.revision => revision,
            Ok(_) => return channel_pin_error(ManagementChannelPinError::SnapshotConflict),
            Err(error) => return management_error(error),
        }
    };

    let pin_request = ManagementChannelPinRequest::new(
        context.version.clone(),
        revision,
        provider_id,
        channel_id,
        route_id,
        credential_id,
        input.requested_model,
        protocol,
        mode,
    );
    let expected_request = pin_request.clone();
    // Persist the final pre-execution boundary before any Provider/transport call. This makes an
    // audit-storage failure a guaranteed no-send result; after the one-shot call begins, the
    // handler never turns a receipt into a retryable 5xx merely because a second audit append
    // failed. The returned receipt remains the terminal source for the operator's exact outcome.
    {
        let mut management_service = match service(&state) {
            Ok(value) => value,
            Err(response) => return response,
        };
        if let Err(error) = management_service.record_resource_action(
            &actor,
            &context.version,
            "channel_pin_started",
            "channel_pin",
            expected_request.route_id().as_str(),
        ) {
            return management_error(error);
        }
    }
    let future = match channel_pin(&state) {
        Ok(source) => source.execute(pin_request),
        Err(response) => return response,
    };
    match future.await {
        Ok(receipt) => {
            if receipt.config_version_id() != &context.version
                || receipt.config_revision() != revision
                || receipt.provider_id() != expected_request.provider_id()
                || receipt.channel_id() != expected_request.channel_id()
                || receipt.route_id() != expected_request.route_id()
                || receipt.credential_id() != expected_request.credential_id()
                || receipt.requested_model() != expected_request.requested_model()
                || receipt.protocol() != expected_request.protocol()
                || receipt.mode() != expected_request.mode()
                || receipt.attempt_count() > 1
                || receipt.observed_at_ms() < 0
            {
                return channel_pin_error(ManagementChannelPinError::SnapshotConflict);
            }
            HttpResponse::Ok()
                .insert_header((header::CACHE_CONTROL, "no-store"))
                .json(ChannelPinResponse::from(receipt))
        }
        Err(error) => channel_pin_error(error),
    }
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
    let mut operation = match workflow(&state) {
        Ok(mut workflow) => workflow.oauth_status(&credential_id),
        Err(response) => return response,
    };
    // OAuth state/challenge material is intentionally process-local.  After a clean restart the
    // transient session map is empty, but an already persisted active `oauth_json` credential is
    // still a valid completed account.  Project that durable fact as complete without decrypting
    // or returning any credential material; a live pending/failed session always wins above.
    if operation.state == ManagementCredentialOAuthState::Failed
        && operation.expires_at_ms.is_none()
        && let Ok(mut service) = service(&state)
        && let Ok(view) = service.get_credential(&context.version, &credential_id)
        && view.value().kind == "oauth_json"
        && view.value().status == CredentialStatus::Active
    {
        operation.state = ManagementCredentialOAuthState::Complete;
        operation.failure_class = None;
    }
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

async fn complete_credential_oauth(
    request: HttpRequest,
    path: web::Path<String>,
    payload: web::Json<CredentialOAuthCallbackRequest>,
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
    let callback = match parse_oauth_callback_request(&payload) {
        Ok(callback) => callback,
        Err(OAuthCallbackInputError::Invalid) => return invalid_input(),
        Err(OAuthCallbackInputError::ProviderRejected {
            state: callback_state,
        }) => {
            if let Some(callback_state) = callback_state
                .as_deref()
                .and_then(|value| decode_oauth_state(value.as_bytes()))
                && let Ok(mut workflow) = workflow(&state)
            {
                let _ = workflow.reject_oauth(&credential_id, &callback_state);
            }
            return HttpResponse::Conflict().json(serde_json::json!({
                "error": "oauth_provider_rejected",
                "credential_id": credential_id.as_str(),
                "failure_class": "provider_rejected",
            }));
        }
    };
    let Some(decoded_state) = decode_oauth_state(callback.state.as_bytes()) else {
        return invalid_input();
    };
    let envelope = match workflow(&state) {
        Ok(mut workflow) => workflow.complete_oauth(
            &credential_id,
            &decoded_state,
            Zeroizing::new(callback.code),
        ),
        Err(response) => return response,
    };
    let Some(envelope) = envelope else {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "oauth_callback_rejected",
            "credential_id": credential_id.as_str(),
        }));
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let current = match service.get_credential(&context.version, &credential_id) {
        Ok(current) => current,
        Err(error) => return management_error(error),
    };
    let persisted = service.persist_oauth_credential_if_revision(
        &actor,
        &context.version,
        current.revision(),
        credential_id.clone(),
        current.value().revision,
        envelope.as_slice(),
    );
    drop(service);
    if let Err(error) = persisted {
        if let Ok(mut workflow) = workflow(&state) {
            let _ = workflow.finalize_oauth(&credential_id, false);
        }
        return management_error(error);
    }
    let finalized = match workflow(&state) {
        Ok(mut workflow) => workflow.finalize_oauth(&credential_id, true),
        Err(_) => false,
    };
    if !finalized {
        return internal_error();
    }
    HttpResponse::Accepted().json(serde_json::json!({
        "credential_id": credential_id.as_str(),
        "state": "complete",
    }))
}

async fn refresh_credential_oauth(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let Ok(credential_id) = CredentialId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let Some(_refresh_claim) = state.claim_oauth_refresh(&credential_id) else {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "oauth_refresh_in_progress",
            "credential_id": credential_id.as_str(),
        }));
    };
    refresh_credential_oauth_claimed(&request, &credential_id, &state)
}

fn refresh_credential_oauth_claimed(
    request: &HttpRequest,
    credential_id: &CredentialId,
    state: &web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    if let Err(response) = require_credential(state, &context.version, credential_id) {
        return response;
    }
    let (current, current_envelope) = {
        let mut service = match service(state) {
            Ok(service) => service,
            Err(response) => return response,
        };
        let current = match service.get_credential(&context.version, credential_id) {
            Ok(current) => current,
            Err(error) => return management_error(error),
        };
        if current.value().kind != "oauth_json" {
            return invalid_input();
        }
        if current.value().status != CredentialStatus::Active {
            return HttpResponse::Conflict().json(serde_json::json!({
                "error": "credential_not_active",
                "credential_id": credential_id.as_str(),
            }));
        }
        let plaintext = match service.open_credential_for_export(&context.version, credential_id) {
            Ok(plaintext) => plaintext,
            Err(error) => return management_error(error),
        };
        (current, Zeroizing::new(plaintext.as_bytes().to_vec()))
    };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0);
    let refreshed = match workflow(state) {
        Ok(mut workflow) => workflow.refresh_oauth(credential_id, current_envelope, now_ms),
        Err(response) => return response,
    };
    let Some(refreshed) = refreshed else {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "oauth_refresh_rejected",
            "credential_id": credential_id.as_str(),
        }));
    };
    let actor = match principal(request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut service = match service(state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let persisted = match service.persist_oauth_credential_if_revision(
        &actor,
        &context.version,
        current.revision(),
        credential_id.clone(),
        current.value().revision,
        refreshed.as_slice(),
    ) {
        Ok(persisted) => persisted,
        Err(error) => return management_error(error),
    };
    HttpResponse::Accepted().json(serde_json::json!({
        "credential_id": credential_id.as_str(),
        "state": "complete",
        "revision": persisted.value().revision,
    }))
}

async fn export_credential(
    request: HttpRequest,
    path: web::Path<String>,
    payload: web::Json<CredentialExportRequest>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(credential_id) = CredentialId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let Ok(format) = CodexCredentialExportFormat::parse(payload.format.as_str()) else {
        return invalid_input();
    };
    if let Err(response) = require_credential(&state, &context.version, &credential_id) {
        return response;
    }
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let plaintext = match service.open_credential_for_export(&context.version, &credential_id) {
        Ok(value) => value,
        Err(error) => return management_error(error),
    };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| i64::try_from(value.as_millis()).ok())
        .unwrap_or(0);
    let Ok(credential) =
        OpenAiCompatibleRuntimeCredential::import_compatible(plaintext.as_bytes(), now_ms)
    else {
        return invalid_input();
    };
    let Ok(output) = credential.export_json(format) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    if service
        .record_resource_action(
            &actor,
            &context.version,
            "credential_exported",
            "credential",
            credential_id.as_str(),
        )
        .is_err()
    {
        return internal_error();
    }
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .content_type("application/json")
        .body(output.as_slice().to_vec())
}

async fn get_credential_metadata(
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
    let mut service = match service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let view = match service.get_credential(&context.version, &credential_id) {
        Ok(value) => value.value().clone(),
        Err(error) => return management_error(error),
    };
    let plaintext = match service.open_credential_for_export(&context.version, &credential_id) {
        Ok(value) => value,
        Err(error) => return management_error(error),
    };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| i64::try_from(value.as_millis()).ok())
        .unwrap_or(0);
    let Ok(credential) =
        OpenAiCompatibleRuntimeCredential::import_compatible(plaintext.as_bytes(), now_ms)
    else {
        return invalid_input();
    };
    let metadata = credential.metadata();
    HttpResponse::Ok().json(CredentialMetadataResponse {
        credential_id: view.id.to_string(),
        kind: view.kind,
        revision: view.revision,
        plan: metadata.and_then(|value| value.plan.clone()),
        quota: metadata.and_then(|value| value.quota.clone()),
        platform: metadata.and_then(|value| value.platform.clone()),
        email: metadata.and_then(|value| value.email.clone()),
        source_format: metadata.and_then(|value| value.source_format.clone()),
    })
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

async fn get_catalog_status(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let observed_at_ms = match runtime_observed_at(&state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let statuses = match runtime(&state).and_then(|mut facade| {
        facade
            .catalog_status(&context.version, observed_at_ms)
            .map_err(runtime_error)
    }) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match catalog_status_response(statuses) {
        Ok(value) => HttpResponse::Ok().json(value),
        Err(error) => runtime_error(error),
    }
}

async fn get_runtime_availability(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let observed_at_ms = match runtime_observed_at(&state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let statuses = match runtime(&state).and_then(|mut facade| {
        facade
            .runtime_availability(&context.version, observed_at_ms)
            .map_err(runtime_error)
    }) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match runtime_availability_response(statuses) {
        Ok(value) => HttpResponse::Ok().json(value),
        Err(error) => runtime_error(error),
    }
}

async fn request_quota_recovery(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let input = match parse_json::<RuntimeTargetInput>(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let target = match runtime_target(input) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_runtime_target(&state, &context.version, &target) {
        return response;
    }
    let observed_at_ms = match runtime_observed_at(&state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let recovery = match runtime(&state).and_then(|mut facade| {
        facade
            .request_quota_recovery(&context.version, &target, observed_at_ms)
            .map_err(runtime_error)
    }) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let mut management_service = match service(&state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = management_service.record_resource_action(
        &actor,
        &context.version,
        "quota_recovery_requested",
        "runtime_credential",
        target.credential_id().as_str(),
    ) {
        return management_error(error);
    }
    HttpResponse::Accepted().json(RuntimeActionResponse::from(recovery))
}

async fn explain_route(
    request: HttpRequest,
    path: web::Path<String>,
    query: web::Query<RouteExplainQuery>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Ok(route_id) = RouteId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let protocol = match management_request_protocol(&query.protocol) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let provider_id = match query.provider_id.as_deref() {
        Some(value) => match ProviderId::try_new(value) {
            Ok(value) => Some(value),
            Err(_) => return invalid_input(),
        },
        None => None,
    };
    let observed_at_ms = match runtime_observed_at(&state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let explain_request = match ManagementRouteExplainRequest::try_new(
        context.version,
        route_id,
        query.requested_model.clone(),
        protocol,
        provider_id,
        observed_at_ms,
    ) {
        Ok(value) => value,
        Err(error) => return runtime_error(error),
    };
    let explain = match runtime(&state).and_then(|mut facade| {
        facade
            .explain_route(&explain_request)
            .map_err(runtime_error)
    }) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match RouteExplainResponse::try_from(explain) {
        Ok(value) => HttpResponse::Ok().json(value),
        Err(error) => runtime_error(error),
    }
}

async fn list_request_attempts(
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let Ok(request_id) = RequestId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let attempts = match runtime(&state).and_then(|mut facade| {
        facade
            .list_request_attempts(&request_id)
            .map_err(runtime_error)
    }) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match request_attempt_response(attempts) {
        Ok(value) => HttpResponse::Ok().json(value),
        Err(error) => runtime_error(error),
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

fn runtime_target(input: RuntimeTargetInput) -> Result<ManagementRuntimeTarget, HttpResponse> {
    let endpoint_id = EndpointId::try_new(input.endpoint_id).map_err(|_| invalid_input())?;
    let credential_id = CredentialId::try_new(input.credential_id).map_err(|_| invalid_input())?;
    ManagementRuntimeTarget::try_new(endpoint_id, credential_id, input.upstream_model)
        .map_err(runtime_error)
}

fn management_request_protocol(value: &str) -> Result<ManagementRequestProtocol, HttpResponse> {
    match value {
        "openai_chat_completions" => Ok(ManagementRequestProtocol::OpenAiChatCompletions),
        "openai_responses" => Ok(ManagementRequestProtocol::OpenAiResponses),
        "anthropic_messages" => Ok(ManagementRequestProtocol::AnthropicMessages),
        _ => Err(invalid_input()),
    }
}

fn management_request_protocol_str(value: ManagementRequestProtocol) -> &'static str {
    match value {
        ManagementRequestProtocol::OpenAiChatCompletions => "openai_chat_completions",
        ManagementRequestProtocol::OpenAiResponses => "openai_responses",
        ManagementRequestProtocol::AnthropicMessages => "anthropic_messages",
    }
}

fn runtime_observed_at(
    state: &web::Data<ManagementResourceHttpState>,
) -> Result<i64, HttpResponse> {
    state
        .runtime_clock
        .now_ms()
        .and_then(|value| {
            if value < 0 {
                Err(ManagementRuntimeError::Unavailable)
            } else {
                Ok(value)
            }
        })
        .map_err(runtime_error)
}

fn runtime(
    state: &web::Data<ManagementResourceHttpState>,
) -> Result<std::sync::MutexGuard<'_, Box<dyn ManagementRuntimeFacade>>, HttpResponse> {
    state.runtime.lock().map_err(|_| internal_error())
}

fn usage(
    state: &web::Data<ManagementResourceHttpState>,
) -> Result<std::sync::MutexGuard<'_, Box<dyn ManagementUsageFacade>>, HttpResponse> {
    state.usage.lock().map_err(|_| internal_error())
}

fn failure_feedback(
    state: &web::Data<ManagementResourceHttpState>,
) -> Result<std::sync::MutexGuard<'_, Box<dyn ManagementFailureFeedbackFacade>>, HttpResponse> {
    state.failure_feedback.lock().map_err(|_| internal_error())
}

fn provider_account_pools(
    state: &web::Data<ManagementResourceHttpState>,
) -> Result<std::sync::MutexGuard<'_, Box<dyn ProviderAccountPoolFacade>>, HttpResponse> {
    state
        .provider_account_pools
        .lock()
        .map_err(|_| internal_error())
}

fn provider_egress_status(
    state: &web::Data<ManagementResourceHttpState>,
) -> Result<std::sync::MutexGuard<'_, Box<dyn ProviderEgressStatusFacade>>, HttpResponse> {
    state
        .provider_egress_status
        .lock()
        .map_err(|_| provider_egress_status_error(ProviderEgressStatusError::SourceUnavailable))
}

fn channel_pin(
    state: &web::Data<ManagementResourceHttpState>,
) -> Result<std::sync::MutexGuard<'_, Box<dyn ManagementChannelPinFacade>>, HttpResponse> {
    state.channel_pin.lock().map_err(|_| internal_error())
}

fn require_runtime_target(
    state: &web::Data<ManagementResourceHttpState>,
    config_version_id: &ConfigVersionId,
    target: &ManagementRuntimeTarget,
) -> Result<(), HttpResponse> {
    let mut management_service = service(state)?;
    let endpoint = management_service
        .get_endpoint(config_version_id, target.endpoint_id())
        .map_err(management_error)?
        .into_parts()
        .0;
    let credential = management_service
        .get_credential(config_version_id, target.credential_id())
        .map_err(management_error)?
        .into_parts()
        .0;
    if endpoint.upstream_id != credential.upstream_id {
        return Err(invalid_input());
    }
    let bindings = management_service
        .list_endpoint_credential_bindings(config_version_id, target.endpoint_id())
        .map_err(management_error)?
        .into_parts()
        .0;
    if !bindings.iter().any(|binding| {
        binding.credential_id == *target.credential_id()
            && binding.upstream_id == endpoint.upstream_id
    }) {
        return Err(invalid_input());
    }
    Ok(())
}

fn catalog_status_response(
    statuses: Vec<ManagementCatalogStatus>,
) -> Result<Vec<CatalogStatusResponse>, ManagementRuntimeError> {
    if statuses.len() > MAX_RUNTIME_ROWS {
        return Err(ManagementRuntimeError::Unavailable);
    }
    let mut targets = BTreeSet::new();
    statuses
        .into_iter()
        .map(|status| {
            if status.observed_at_ms() < 0
                || !targets.insert((
                    status.endpoint_id().as_str().to_owned(),
                    status.credential_id().as_str().to_owned(),
                ))
            {
                return Err(ManagementRuntimeError::Unavailable);
            }
            Ok(CatalogStatusResponse {
                endpoint_id: status.endpoint_id().as_str().to_owned(),
                credential_id: status.credential_id().as_str().to_owned(),
                freshness: catalog_freshness_response(status.freshness()),
                observed_at_ms: status.observed_at_ms(),
            })
        })
        .collect()
}

fn runtime_availability_response(
    statuses: Vec<ManagementRuntimeAvailabilityStatus>,
) -> Result<Vec<RuntimeAvailabilityResponse>, ManagementRuntimeError> {
    if statuses.len() > MAX_RUNTIME_ROWS {
        return Err(ManagementRuntimeError::Unavailable);
    }
    let mut targets = BTreeSet::new();
    statuses
        .into_iter()
        .map(|status| {
            if !targets.insert((
                status.endpoint_id().as_str().to_owned(),
                status.credential_id().as_str().to_owned(),
            )) {
                return Err(ManagementRuntimeError::Unavailable);
            }
            Ok(RuntimeAvailabilityResponse {
                endpoint_id: status.endpoint_id().as_str().to_owned(),
                credential_id: status.credential_id().as_str().to_owned(),
                availability: runtime_availability_category(status.availability()),
            })
        })
        .collect()
}

fn request_attempt_response(
    attempts: Vec<ManagementRequestAttempt>,
) -> Result<Vec<RequestAttemptResponse>, ManagementRuntimeError> {
    if attempts.len() > MAX_REQUEST_ATTEMPTS {
        return Err(ManagementRuntimeError::Unavailable);
    }
    let mut ids = BTreeSet::new();
    attempts
        .into_iter()
        .map(|attempt| {
            if !ids.insert(attempt.attempt_id().to_owned())
                || !safe_attempt_outcome(attempt.outcome())
            {
                return Err(ManagementRuntimeError::Unavailable);
            }
            Ok(RequestAttemptResponse {
                attempt_id: attempt.attempt_id().to_owned(),
                outcome: attempt.outcome(),
                stage: attempt.stage().map(ManagementRequestAttemptStage::as_str),
                endpoint_id: attempt.endpoint_id().map(|id| id.as_str().to_owned()),
                credential_id: attempt.credential_id().map(|id| id.as_str().to_owned()),
            })
        })
        .collect()
}

fn catalog_freshness_response(value: ManagementCatalogFreshness) -> &'static str {
    match value {
        ManagementCatalogFreshness::Fresh => "fresh",
        ManagementCatalogFreshness::Stale => "stale",
        ManagementCatalogFreshness::Expired => "expired",
        ManagementCatalogFreshness::Missing => "missing",
    }
}

fn runtime_availability_category(value: ManagementRuntimeAvailability) -> &'static str {
    match value {
        ManagementRuntimeAvailability::Available => "available",
        ManagementRuntimeAvailability::Cooldown => "cooldown",
        ManagementRuntimeAvailability::CircuitOpen => "circuit_open",
        ManagementRuntimeAvailability::QuotaBlocked => "quota_blocked",
        ManagementRuntimeAvailability::CredentialForbidden => "credential_forbidden",
        ManagementRuntimeAvailability::RecoveryRequired => "recovery_required",
    }
}

fn safe_attempt_outcome(value: &str) -> bool {
    matches!(value, "succeeded" | "failed")
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
    let mut duplicate_checker = serde_json::Deserializer::from_slice(body);
    duplicate_checker
        .deserialize_any(DuplicateJsonKeyVisitor)
        .map_err(|_| invalid_input())?;
    duplicate_checker.end().map_err(|_| invalid_input())?;
    serde_json::from_slice(body).map_err(|_| invalid_input())
}

struct DuplicateJsonValueSeed;

impl<'de> DeserializeSeed<'de> for DuplicateJsonValueSeed {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateJsonKeyVisitor)
    }
}

struct DuplicateJsonKeyVisitor;

impl<'de> Visitor<'de> for DuplicateJsonKeyVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i128<E>(self, _value: i128) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u128<E>(self, _value: u128) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_char<E>(self, _value: char) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_borrowed_str<E>(self, _value: &'de str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_bytes<E>(self, _value: &[u8]) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_borrowed_bytes<E>(self, _value: &'de [u8]) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_byte_buf<E>(self, _value: Vec<u8>) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence
            .next_element_seed(DuplicateJsonValueSeed)?
            .is_some()
        {}
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            map.next_value_seed(DuplicateJsonValueSeed)?;
        }
        Ok(())
    }
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
        "canonical_bridge" => TransformMode::CanonicalBridge,
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
    if value.trim().is_empty()
        || value.trim() != value
        || value.chars().count() > maximum
        || value.chars().any(char::is_control)
    {
        Err(invalid_input())
    } else {
        Ok(value)
    }
}

fn compatible_proxy_pool(
    input: CompatibleProxyPoolInput,
) -> Result<CompatibleProxyPoolConfiguration, HttpResponse> {
    Ok(CompatibleProxyPoolConfiguration {
        id: CompatibleProxyPoolId::try_new(input.id).map_err(|_| invalid_input())?,
        upstream_id: UpstreamId::try_new(input.upstream_id).map_err(|_| invalid_input())?,
        name: bounded_text(input.name, 256)?,
        enabled: input.enabled,
    })
}

fn compatible_proxy_node(
    input: CompatibleProxyNodeInput,
) -> Result<CompatibleProxyNodeUpsert, HttpResponse> {
    if let Some(proxy_endpoint) = &input.proxy_endpoint
        && (proxy_endpoint.trim().is_empty()
            || proxy_endpoint.len() > 2048
            || proxy_endpoint.chars().any(char::is_control))
    {
        return Err(invalid_input());
    }
    let weight = u16::try_from(input.weight)
        .ok()
        .filter(|value| (1..=1024).contains(value))
        .ok_or_else(invalid_input)?;
    let maximum_concurrency = u32::try_from(input.maximum_concurrency)
        .ok()
        .filter(|value| (1..=100_000).contains(value))
        .ok_or_else(invalid_input)?;
    Ok(CompatibleProxyNodeUpsert {
        id: CompatibleProxyNodeId::try_new(input.id).map_err(|_| invalid_input())?,
        upstream_id: UpstreamId::try_new(input.upstream_id).map_err(|_| invalid_input())?,
        pool_id: input
            .pool_id
            .map(CompatibleProxyPoolId::try_new)
            .transpose()
            .map_err(|_| invalid_input())?,
        name: bounded_text(input.name, 256)?,
        proxy_endpoint: input.proxy_endpoint,
        enabled: input.enabled,
        weight,
        maximum_concurrency,
    })
}

fn compatible_egress_binding(
    input: CompatibleEgressBindingInput,
) -> Result<CompatibleEgressBindingConfiguration, HttpResponse> {
    let endpoint_id = EndpointId::try_new(input.endpoint_id).map_err(|_| invalid_input())?;
    let credential_id = CredentialId::try_new(input.credential_id).map_err(|_| invalid_input())?;
    let target = match (input.target_kind.as_str(), input.target_id) {
        ("direct", None) => CompatibleEgressTargetConfiguration::Direct,
        ("fixed_proxy", Some(value)) => CompatibleEgressTargetConfiguration::FixedProxy(
            CompatibleProxyNodeId::try_new(value).map_err(|_| invalid_input())?,
        ),
        ("proxy_pool", Some(value)) => CompatibleEgressTargetConfiguration::ProxyPool(
            CompatibleProxyPoolId::try_new(value).map_err(|_| invalid_input())?,
        ),
        _ => return Err(invalid_input()),
    };
    let failure_scope = match input.failure_scope.as_str() {
        "endpoint" => StoredCompatibleFailureScope::Endpoint,
        "credential" => StoredCompatibleFailureScope::Credential,
        "egress_node" => StoredCompatibleFailureScope::EgressNode,
        _ => return Err(invalid_input()),
    };
    let stickiness = match input.stickiness.as_str() {
        "none" => StoredCompatibleStickiness::None,
        "credential" => StoredCompatibleStickiness::Credential,
        "credential_and_egress" => StoredCompatibleStickiness::CredentialAndEgress,
        _ => return Err(invalid_input()),
    };
    let pre_submit_max_attempts = u8::try_from(input.pre_submit_max_attempts)
        .ok()
        .filter(|value| (1..=3).contains(value))
        .ok_or_else(invalid_input)?;
    if matches!(target, CompatibleEgressTargetConfiguration::Direct)
        && (matches!(failure_scope, StoredCompatibleFailureScope::EgressNode)
            || matches!(stickiness, StoredCompatibleStickiness::CredentialAndEgress))
    {
        return Err(invalid_input());
    }
    Ok(CompatibleEgressBindingConfiguration {
        endpoint_id,
        credential_id,
        target,
        failure_scope,
        stickiness,
        pre_submit_max_attempts,
    })
}

const fn compatible_failure_scope_str(value: StoredCompatibleFailureScope) -> &'static str {
    match value {
        StoredCompatibleFailureScope::Endpoint => "endpoint",
        StoredCompatibleFailureScope::Credential => "credential",
        StoredCompatibleFailureScope::EgressNode => "egress_node",
    }
}

const fn compatible_stickiness_str(value: StoredCompatibleStickiness) -> &'static str {
    match value {
        StoredCompatibleStickiness::None => "none",
        StoredCompatibleStickiness::Credential => "credential",
        StoredCompatibleStickiness::CredentialAndEgress => "credential_and_egress",
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
        .insert_header((header::CACHE_CONTROL, "no-store"))
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
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(value)
}

fn empty_with_revision(revision: ConfigRevision) -> HttpResponse {
    HttpResponse::NoContent()
        .insert_header((header::ETAG, format!("\"{}\"", revision.as_token())))
        .insert_header((header::CACHE_CONTROL, "no-store"))
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
        ManagementResourceError::CredentialRevisionConflict => error_response(
            StatusCode::CONFLICT,
            "management_credential_revision_conflict",
            "Credential changed",
        ),
        ManagementResourceError::Store(StoreError::ConflictingBillingCatalogVersion) => {
            error_response(
                StatusCode::CONFLICT,
                "management_billing_catalog_conflict",
                "Billing catalog version already exists",
            )
        }
        ManagementResourceError::Operations(ManagementOperationsError::CursorVersionConflict) => {
            error_response(
                StatusCode::CONFLICT,
                "management_operations_cursor_conflict",
                "Management inventory changed",
            )
        }
        ManagementResourceError::Operations(ManagementOperationsError::InvalidQuery) => {
            invalid_input()
        }
        ManagementResourceError::InvalidRevision
        | ManagementResourceError::InvalidCredentialInput
        | ManagementResourceError::InvalidBillingCatalogInput
        | ManagementResourceError::RoutingPriceCatalogNotEffective
        | ManagementResourceError::Store(StoreError::InvalidCompatibleEgressConfiguration)
        | ManagementResourceError::ControlPlane(
            ControlPlaneServiceError::InvalidCompatibleProxyEndpoint,
        ) => invalid_input(),
        ManagementResourceError::Store(_)
        | ManagementResourceError::SecretStore(_)
        | ManagementResourceError::ControlPlane(_)
        | ManagementResourceError::Clock(_)
        | ManagementResourceError::ClientKey(_)
        | ManagementResourceError::ClientKeyIssuerUnavailable
        | ManagementResourceError::Operations(
            ManagementOperationsError::InconsistentConfiguration
            | ManagementOperationsError::SourceUnavailable,
        ) => internal_error(),
    };
    drop(error);
    response
}

fn provider_account_pool_error(error: ProviderAccountPoolError) -> HttpResponse {
    match error {
        ProviderAccountPoolError::InvalidQuery | ProviderAccountPoolError::InvalidAction => {
            invalid_input()
        }
        ProviderAccountPoolError::CursorConflict => error_response(
            StatusCode::CONFLICT,
            "management_provider_account_pool_cursor_conflict",
            "Provider account-pool snapshot changed",
        ),
        ProviderAccountPoolError::ActionTargetUnavailable => error_response(
            StatusCode::CONFLICT,
            "management_provider_account_action_target_changed",
            "Provider account action target changed",
        ),
        ProviderAccountPoolError::InvalidSnapshot | ProviderAccountPoolError::SourceUnavailable => {
            internal_error()
        }
    }
}

fn provider_egress_status_error(error: ProviderEgressStatusError) -> HttpResponse {
    match error {
        ProviderEgressStatusError::InvalidQuery => invalid_input(),
        ProviderEgressStatusError::CursorConflict => error_response(
            StatusCode::CONFLICT,
            "management_provider_egress_status_cursor_conflict",
            "Provider egress status snapshot changed",
        ),
        ProviderEgressStatusError::ConfigConflict => error_response(
            StatusCode::CONFLICT,
            "management_provider_egress_status_config_conflict",
            "Provider egress status configuration changed",
        ),
        ProviderEgressStatusError::SourceUnavailable
        | ProviderEgressStatusError::InvalidSnapshot => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "management_runtime_unavailable",
            "Management runtime is unavailable",
        ),
    }
}

fn channel_pin_error(error: ManagementChannelPinError) -> HttpResponse {
    match error {
        ManagementChannelPinError::InvalidTarget => error_response(
            StatusCode::BAD_REQUEST,
            "invalid_management_request",
            "Management request is invalid",
        ),
        ManagementChannelPinError::SnapshotConflict => error_response(
            StatusCode::CONFLICT,
            "management_channel_pin_target_changed",
            "Channel Pin target changed",
        ),
        ManagementChannelPinError::Unavailable => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "management_runtime_unavailable",
            "Management runtime is unavailable",
        ),
        ManagementChannelPinError::ExecutionFailed => error_response(
            StatusCode::BAD_GATEWAY,
            "management_channel_pin_failed",
            "Channel Pin failed",
        ),
    }
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

fn runtime_error(error: ManagementRuntimeError) -> HttpResponse {
    match error {
        ManagementRuntimeError::InvalidInput => invalid_input(),
        ManagementRuntimeError::Unavailable => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "management_runtime_unavailable",
            "Management runtime observation is unavailable",
        ),
    }
}

fn error_response(status: StatusCode, code: &'static str, message: &'static str) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(serde_json::json!({"error":{"code":code,"message":message}}))
}

impl From<ManagementQuotaRecoveryState> for RuntimeActionResponse {
    fn from(value: ManagementQuotaRecoveryState) -> Self {
        let state = match value {
            ManagementQuotaRecoveryState::RecoveryRequired => "recovery_required",
            ManagementQuotaRecoveryState::ProbeScheduled => "probe_scheduled",
            ManagementQuotaRecoveryState::Rejected => "rejected",
        };
        Self { state }
    }
}

impl TryFrom<ManagementRouteExplain> for RouteExplainResponse {
    type Error = ManagementRuntimeError;

    fn try_from(value: ManagementRouteExplain) -> Result<Self, Self::Error> {
        let mut ids = BTreeSet::new();
        let candidates = value
            .candidates()
            .iter()
            .map(|candidate| {
                let reason = candidate.reason();
                if !ids.insert(candidate.candidate_id().as_str().to_owned())
                    || candidate.selected_by_projection() != reason.is_none()
                    || reason.is_some_and(|value| !safe_route_explain_reason(value))
                    || !safe_route_explain_price_evidence(candidate.price_evidence())
                {
                    return Err(ManagementRuntimeError::Unavailable);
                }
                Ok(RouteExplainCandidateResponse {
                    candidate_id: candidate.candidate_id().as_str().to_owned(),
                    decision: if candidate.selected_by_projection() {
                        "selected"
                    } else {
                        "excluded"
                    },
                    reason,
                    price_evidence: candidate.price_evidence(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            route_id: value.route_id().as_str().to_owned(),
            candidates,
            price_policy: value
                .price_policy()
                .map(|policy| {
                    if policy.catalog_version_id().trim().is_empty()
                        || policy.catalog_version_id().len() > 128
                        || policy.comparison() != "rate_dominance_v1"
                    {
                        return Err(ManagementRuntimeError::Unavailable);
                    }
                    Ok(RouteExplainPricePolicyResponse {
                        catalog_version_id: policy.catalog_version_id().to_owned(),
                        comparison: policy.comparison(),
                    })
                })
                .transpose()?,
        })
    }
}

fn safe_route_explain_reason(value: &str) -> bool {
    matches!(
        value,
        "not_hard_eligible"
            | "endpoint_cooldown"
            | "endpoint_circuit_open"
            | "endpoint_unavailable"
            | "missing_credential_pool"
            | "no_eligible_credential"
            | "provider_scope_required"
            | "provider_mismatch"
            | "protocol_transform_unavailable"
            | "after_selected_candidate"
    )
}

fn safe_route_explain_price_evidence(value: &str) -> bool {
    matches!(
        value,
        "dominant"
            | "equal"
            | "dominated"
            | "incomparable"
            | "unpriced"
            | "not_evaluated"
            | "disabled"
    )
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
                TransformMode::CanonicalBridge => "canonical_bridge",
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

fn operational_account_status(value: &str) -> Result<CredentialStatus, HttpResponse> {
    match value {
        "active" => Ok(CredentialStatus::Active),
        "cooling" => Ok(CredentialStatus::Cooling),
        "unauthorized" => Ok(CredentialStatus::Unauthorized),
        "disabled" => Ok(CredentialStatus::Disabled),
        _ => Err(invalid_input()),
    }
}

fn operational_query_id(value: String) -> Result<String, HttpResponse> {
    bounded_text(value, 128)
}

fn query_has_duplicate_keys(query: &str) -> bool {
    let mut keys = BTreeSet::new();
    url::form_urlencoded::parse(query.as_bytes()).any(|(key, _)| !keys.insert(key.into_owned()))
}

fn usage_query_id(value: Option<String>) -> Result<Option<String>, HttpResponse> {
    value.map(|value| bounded_text(value, 128)).transpose()
}

fn operational_usage_protocol(value: &str) -> Result<GatewayProtocol, HttpResponse> {
    match value {
        "openai_chat_completions" => Ok(GatewayProtocol::OpenAiChatCompletions),
        "openai_responses" => Ok(GatewayProtocol::OpenAiResponses),
        "anthropic_messages" => Ok(GatewayProtocol::AnthropicMessages),
        _ => Err(invalid_input()),
    }
}

fn operational_usage_protocol_response(value: GatewayProtocol) -> &'static str {
    match value {
        GatewayProtocol::OpenAiChatCompletions => "openai_chat_completions",
        GatewayProtocol::OpenAiResponses => "openai_responses",
        GatewayProtocol::AnthropicMessages => "anthropic_messages",
    }
}

fn operational_account_status_response(value: CredentialStatus) -> &'static str {
    match value {
        CredentialStatus::Active => "active",
        CredentialStatus::Cooling => "cooling",
        CredentialStatus::Unauthorized => "unauthorized",
        CredentialStatus::Disabled => "disabled",
    }
}

fn provider_account_auth_status(value: &str) -> Result<ProviderAccountAuthStatus, HttpResponse> {
    match value {
        "active" => Ok(ProviderAccountAuthStatus::Active),
        "reauth_required" => Ok(ProviderAccountAuthStatus::ReauthRequired),
        "disabled" => Ok(ProviderAccountAuthStatus::Disabled),
        "expired" => Ok(ProviderAccountAuthStatus::Expired),
        _ => Err(invalid_input()),
    }
}

fn provider_account_runtime_status(
    value: &str,
) -> Result<ProviderAccountRuntimeStatus, HttpResponse> {
    match value {
        "available" => Ok(ProviderAccountRuntimeStatus::Available),
        "cooling" => Ok(ProviderAccountRuntimeStatus::Cooling),
        "circuit_open" => Ok(ProviderAccountRuntimeStatus::CircuitOpen),
        "quota_blocked" => Ok(ProviderAccountRuntimeStatus::QuotaBlocked),
        "unauthorized" => Ok(ProviderAccountRuntimeStatus::Unauthorized),
        "recovery_in_flight" => Ok(ProviderAccountRuntimeStatus::RecoveryInFlight),
        "expired" => Ok(ProviderAccountRuntimeStatus::Expired),
        _ => Err(invalid_input()),
    }
}

fn encode_provider_account_pool_cursor(
    cursor: &ProviderAccountPoolCursor,
) -> Result<String, HttpResponse> {
    let wire = ProviderAccountPoolCursorWire {
        snapshot_id: cursor.snapshot_id().to_owned(),
        filter_fingerprint: cursor.filter_fingerprint().to_owned(),
        provider_id: cursor.provider_id().as_str().to_owned(),
        channel_id: cursor.channel_id().as_str().to_owned(),
        account_id: cursor.account_id().as_str().to_owned(),
    };
    serde_json::to_vec(&wire)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| internal_error())
}

fn encode_provider_egress_status_cursor(
    cursor: &ProviderEgressStatusCursor,
) -> Result<String, HttpResponse> {
    let key = cursor.last_key();
    let wire = ProviderEgressStatusCursorWire {
        config_version_id: cursor.config_version_id().as_str().to_owned(),
        config_revision: cursor.config_revision().as_i64(),
        runtime_revision: cursor.runtime_revision(),
        snapshot_id: cursor.snapshot_id().to_owned(),
        sampled_at_ms: cursor.sampled_at_ms(),
        filter_fingerprint: cursor.filter_fingerprint().to_owned(),
        last_key: ProviderEgressStatusCursorKeyWire {
            provider_id: key.provider_id().as_str().to_owned(),
            upstream_id: key.upstream_id().as_str().to_owned(),
            channel_id: key.channel_id().as_str().to_owned(),
            domain: key.domain().as_str().to_owned(),
            credential_id: key.credential_id().map(|value| value.as_str().to_owned()),
            credential_revision: key.credential_revision(),
            session_revision: key.session_revision(),
            target_kind: key
                .target_kind()
                .map(ProviderEgressStatusTargetKind::as_str)
                .map(str::to_owned),
            target_id: key.target_id().map(str::to_owned),
            clearance_revision: key.clearance_revision(),
        },
    };
    let encoded = serde_json::to_vec(&wire)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| provider_egress_status_error(ProviderEgressStatusError::SourceUnavailable))?;
    if encoded.len() > MAX_PROVIDER_EGRESS_STATUS_CURSOR_LENGTH {
        return Err(provider_egress_status_error(
            ProviderEgressStatusError::InvalidSnapshot,
        ));
    }
    Ok(encoded)
}

fn decode_provider_egress_status_cursor(
    value: &str,
) -> Result<Option<ProviderEgressStatusCursor>, ProviderEgressStatusError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ProviderEgressStatusError::InvalidQuery)?;
    if bytes.is_empty() || bytes.len() > 16 * 1024 {
        return Err(ProviderEgressStatusError::InvalidQuery);
    }
    let wire: ProviderEgressStatusCursorWire =
        serde_json::from_slice(&bytes).map_err(|_| ProviderEgressStatusError::InvalidQuery)?;
    let config_version_id = ConfigVersionId::try_new(wire.config_version_id)
        .map_err(|_| ProviderEgressStatusError::InvalidQuery)?;
    let config_revision = ConfigRevision::try_new(wire.config_revision)
        .map_err(|_| ProviderEgressStatusError::InvalidQuery)?;
    let provider_id = ProviderId::try_new(wire.last_key.provider_id)
        .map_err(|_| ProviderEgressStatusError::InvalidQuery)?;
    let upstream_id = UpstreamId::try_new(wire.last_key.upstream_id)
        .map_err(|_| ProviderEgressStatusError::InvalidQuery)?;
    let channel_id = EndpointId::try_new(wire.last_key.channel_id)
        .map_err(|_| ProviderEgressStatusError::InvalidQuery)?;
    let domain = wire.last_key.domain.parse::<ProviderEgressStatusDomain>()?;
    let credential_id = wire
        .last_key
        .credential_id
        .map(CredentialId::try_new)
        .transpose()
        .map_err(|_| ProviderEgressStatusError::InvalidQuery)?;
    let target_kind = wire
        .last_key
        .target_kind
        .as_deref()
        .map(str::parse::<ProviderEgressStatusTargetKind>)
        .transpose()?;
    let last_key = ProviderEgressStatusItemKey::try_new(
        provider_id,
        upstream_id,
        channel_id,
        domain,
        credential_id,
        wire.last_key.credential_revision,
        wire.last_key.session_revision,
        target_kind,
        wire.last_key.target_id,
        wire.last_key.clearance_revision,
    )?;
    ProviderEgressStatusCursor::try_new(
        config_version_id,
        config_revision,
        wire.runtime_revision,
        wire.snapshot_id,
        wire.sampled_at_ms,
        wire.filter_fingerprint,
        last_key,
    )
    .map(Some)
}

fn provider_egress_status_page_response(
    value: ProviderEgressStatusPage,
) -> Result<ProviderEgressStatusPageResponse, HttpResponse> {
    let next_cursor = value
        .next_cursor
        .as_ref()
        .map(encode_provider_egress_status_cursor)
        .transpose()?;
    Ok(ProviderEgressStatusPageResponse {
        config_version_id: value.config_version_id.as_str().to_owned(),
        config_revision: value.config_revision.as_i64(),
        runtime_revision: value.runtime_revision,
        snapshot_id: value.snapshot_id,
        sampled_at_ms: value.sampled_at_ms,
        items: value
            .items
            .into_iter()
            .map(provider_egress_status_item_response)
            .collect(),
        next_cursor,
    })
}

fn provider_egress_status_item_response(
    value: ProviderEgressStatusItem,
) -> ProviderEgressStatusItemResponse {
    match value {
        ProviderEgressStatusItem::Egress(ProviderEgressStatusEgressItem {
            channel,
            target,
            state,
            deadline_ms,
        }) => ProviderEgressStatusItemResponse::Egress {
            provider_id: channel.provider_id.as_str().to_owned(),
            upstream_id: channel.upstream_id.as_str().to_owned(),
            channel_id: channel.channel_id.as_str().to_owned(),
            channel_kind: channel.channel_kind.as_str(),
            target_kind: target.kind.as_str(),
            target_id: target.id,
            state: state.as_str(),
            deadline_ms,
        },
        ProviderEgressStatusItem::Session(ProviderEgressStatusSessionItem {
            channel,
            credential_id,
            credential_revision,
            session_revision,
            state,
            expires_at_ms,
        }) => ProviderEgressStatusItemResponse::Session {
            provider_id: channel.provider_id.as_str().to_owned(),
            upstream_id: channel.upstream_id.as_str().to_owned(),
            channel_id: channel.channel_id.as_str().to_owned(),
            channel_kind: channel.channel_kind.as_str(),
            credential_id: credential_id.as_str().to_owned(),
            credential_revision,
            session_revision,
            state: state.as_str(),
            expires_at_ms,
        },
        ProviderEgressStatusItem::Clearance(ProviderEgressStatusClearanceItem {
            channel,
            credential_id,
            credential_revision,
            session_revision,
            target,
            clearance_revision,
            state,
            expires_at_ms,
        }) => ProviderEgressStatusItemResponse::Clearance {
            provider_id: channel.provider_id.as_str().to_owned(),
            upstream_id: channel.upstream_id.as_str().to_owned(),
            channel_id: channel.channel_id.as_str().to_owned(),
            channel_kind: channel.channel_kind.as_str(),
            credential_id: credential_id.as_str().to_owned(),
            credential_revision,
            session_revision,
            target_kind: target.kind.as_str(),
            target_id: target.id,
            clearance_revision,
            state: state.as_str(),
            expires_at_ms,
        },
    }
}

fn decode_provider_account_pool_cursor(
    value: &str,
) -> Result<Option<ProviderAccountPoolCursor>, ProviderAccountPoolError> {
    if value.is_empty() {
        return Err(ProviderAccountPoolError::InvalidQuery);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ProviderAccountPoolError::InvalidQuery)?;
    let wire: ProviderAccountPoolCursorWire =
        serde_json::from_slice(&bytes).map_err(|_| ProviderAccountPoolError::InvalidQuery)?;
    if wire.snapshot_id.is_empty()
        || wire.snapshot_id.chars().count() > 128
        || wire.filter_fingerprint.chars().count() > 512
        || wire.provider_id.chars().count() > 128
        || wire.channel_id.chars().count() > 128
        || wire.account_id.chars().count() > 128
    {
        return Err(ProviderAccountPoolError::InvalidQuery);
    }
    let provider_id = ProviderId::try_new(wire.provider_id)
        .map_err(|_| ProviderAccountPoolError::InvalidQuery)?;
    let channel_id =
        EndpointId::try_new(wire.channel_id).map_err(|_| ProviderAccountPoolError::InvalidQuery)?;
    let account_id = CredentialId::try_new(wire.account_id)
        .map_err(|_| ProviderAccountPoolError::InvalidQuery)?;
    ProviderAccountPoolCursor::try_new(
        wire.snapshot_id,
        wire.filter_fingerprint,
        provider_id,
        channel_id,
        account_id,
    )
    .map(Some)
}

fn provider_account_pool_page_response(
    value: ProviderAccountPoolPage,
) -> Result<ProviderAccountPoolPageResponse, HttpResponse> {
    let next_cursor = value
        .next_cursor
        .as_ref()
        .map(encode_provider_account_pool_cursor)
        .transpose()?;
    Ok(ProviderAccountPoolPageResponse {
        snapshot_id: value.snapshot_id,
        observed_at_ms: value.observed_at_ms,
        items: value
            .items
            .into_iter()
            .map(provider_account_pool_item_response)
            .collect(),
        next_cursor,
    })
}

fn provider_account_pool_item_response(
    value: ProviderAccountPoolItem,
) -> ProviderAccountPoolItemResponse {
    ProviderAccountPoolItemResponse {
        provider_id: value.provider_id.as_str().to_owned(),
        channel_id: value.channel_id.as_str().to_owned(),
        account_id: value.account_id.as_str().to_owned(),
        account_kind: value.account_kind,
        auth_status: value.auth_status.as_str(),
        runtime_status: value.runtime_status.as_str(),
        enabled: value.enabled,
        priority: value.priority,
        weight: value.weight,
        max_concurrency: value.max_concurrency,
        active_leases: value.active_leases,
        expires_at_ms: value.expires_at_ms,
        refresh_due_at_ms: value.refresh_due_at_ms,
        quota_sync_due_at_ms: value.quota_sync_due_at_ms,
    }
}

fn encode_failure_feedback_cursor(cursor: &FailureFeedbackCursor) -> Result<String, HttpResponse> {
    let wire = FailureFeedbackCursorWire {
        ordinal: cursor.ordinal(),
        filter_fingerprint: cursor.filter_fingerprint().to_owned(),
    };
    serde_json::to_vec(&wire)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| internal_error())
}

fn decode_failure_feedback_cursor(
    value: &str,
) -> Result<Option<FailureFeedbackCursor>, ManagementOperationsError> {
    if value.is_empty() {
        return Err(ManagementOperationsError::InvalidQuery);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let wire: FailureFeedbackCursorWire =
        serde_json::from_slice(&bytes).map_err(|_| ManagementOperationsError::InvalidQuery)?;
    if wire.filter_fingerprint.chars().count() > 512 {
        return Err(ManagementOperationsError::InvalidQuery);
    }
    FailureFeedbackCursor::try_new(wire.ordinal, wire.filter_fingerprint).map(Some)
}

fn failure_feedback_page_response(
    value: FailureFeedbackPage,
) -> Result<FailureFeedbackPageResponse, HttpResponse> {
    let next_cursor = value
        .next_cursor
        .as_ref()
        .map(encode_failure_feedback_cursor)
        .transpose()?;
    Ok(FailureFeedbackPageResponse {
        observed_through_ordinal: value.observed_through_ordinal,
        items: value
            .items
            .into_iter()
            .map(|item| FailureFeedbackItemResponse {
                provider_id: item.provider_id.as_str().to_owned(),
                channel_id: item.channel_id.as_str().to_owned(),
                account_id: item.account_id.as_str().to_owned(),
                request_id: item.request_id,
                attempt_id: item.attempt_id,
                ended_at_ms: item.ended_at_ms,
                error_code: item.error_code,
                error_scope: item.error_scope,
                retry_decision: retry_decision_response(item.retry_decision),
            })
            .collect(),
        next_cursor,
    })
}

fn retry_decision_response(value: gateway_core::AttemptRetryDecision) -> &'static str {
    match value {
        gateway_core::AttemptRetryDecision::Completed => "completed",
        gateway_core::AttemptRetryDecision::RetryEligible => "retry_eligible",
        gateway_core::AttemptRetryDecision::NonRetryable => "non_retryable",
        gateway_core::AttemptRetryDecision::RetryClosed => "retry_closed",
        gateway_core::AttemptRetryDecision::Cancelled => "cancelled",
        gateway_core::AttemptRetryDecision::InfrastructureFailure => "infrastructure_failure",
    }
}

fn provider_account_action_resource_id(action: &ProviderAccountOperatorAction) -> String {
    let mut digest = sha2::Sha256::new();
    digest.update(action.provider_id.as_str().as_bytes());
    digest.update([0]);
    digest.update(action.channel_id.as_str().as_bytes());
    digest.update([0]);
    digest.update(action.account_id.as_str().as_bytes());
    format!("sha256:{:x}", digest.finalize())
}

fn operational_transport_response(value: EndpointTransport) -> &'static str {
    match value {
        EndpointTransport::Http => "http",
        EndpointTransport::Sse => "sse",
        EndpointTransport::Websocket => "websocket",
    }
}

fn encode_operational_account_pool_cursor(
    cursor: &OperationalAccountPoolCursor,
) -> Result<String, HttpResponse> {
    let wire = OperationalAccountPoolCursorWire {
        config_version_id: cursor.config_version_id().to_owned(),
        revision: cursor.revision(),
        provider_id: cursor.provider_id().as_str().to_owned(),
        channel_id: cursor.channel_id().as_str().to_owned(),
        account_id: cursor.account_id().as_str().to_owned(),
    };
    serde_json::to_vec(&wire)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| internal_error())
}

fn decode_operational_account_pool_cursor(
    value: &str,
) -> Result<Option<OperationalAccountPoolCursor>, ManagementOperationsError> {
    if value.is_empty() {
        return Err(ManagementOperationsError::InvalidQuery);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let wire: OperationalAccountPoolCursorWire =
        serde_json::from_slice(&bytes).map_err(|_| ManagementOperationsError::InvalidQuery)?;
    if wire.config_version_id.is_empty()
        || wire.config_version_id.chars().count() > 128
        || wire.provider_id.chars().count() > 128
        || wire.channel_id.chars().count() > 128
        || wire.account_id.chars().count() > 128
    {
        return Err(ManagementOperationsError::InvalidQuery);
    }
    let provider_id = UpstreamId::try_new(wire.provider_id)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let channel_id = EndpointId::try_new(wire.channel_id)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let account_id = CredentialId::try_new(wire.account_id)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    OperationalAccountPoolCursor::try_new(
        wire.config_version_id,
        wire.revision,
        provider_id,
        channel_id,
        account_id,
    )
    .map(Some)
}

fn operational_account_pool_page_response(
    value: OperationalAccountPoolPage,
) -> Result<OperationalAccountPoolPageResponse, HttpResponse> {
    let next_cursor = value
        .next_cursor
        .as_ref()
        .map(encode_operational_account_pool_cursor)
        .transpose()?;
    Ok(OperationalAccountPoolPageResponse {
        config_version_id: value.config_version_id,
        revision: value.revision,
        items: value
            .items
            .into_iter()
            .map(operational_account_pool_item_response)
            .collect(),
        next_cursor,
    })
}

fn operational_account_pool_item_response(
    value: OperationalAccountPoolItem,
) -> OperationalAccountPoolItemResponse {
    OperationalAccountPoolItemResponse {
        provider_id: value.provider_id.as_str().to_owned(),
        provider_name: value.provider_name,
        provider_kind: value.provider_kind,
        provider_enabled: value.provider_enabled,
        egress_policy_id: value.egress_policy_id.map(|id| id.as_str().to_owned()),
        channel_id: value.channel_id.as_str().to_owned(),
        adapter_id: value.adapter_id,
        api_format: value.api_format,
        transport: operational_transport_response(value.transport),
        channel_enabled: value.channel_enabled,
        account_id: value.account_id.as_str().to_owned(),
        account_kind: value.account_kind,
        account_status: operational_account_status_response(value.account_status),
        account_revision: value.account_revision,
        binding_enabled: value.binding_enabled,
        configured_enabled: value.configured_enabled,
        priority: value.priority,
        weight: value.weight,
        concurrency: value.concurrency,
        route_ids: value
            .route_ids
            .into_iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
    }
}

fn encode_operational_usage_cursor(
    cursor: &OperationalUsageCursor,
) -> Result<String, HttpResponse> {
    let wire = OperationalUsageCursorWire {
        provider_id: cursor.provider_id().as_str().to_owned(),
        channel_id: cursor.channel_id().as_str().to_owned(),
        account_id: cursor.account_id().as_str().to_owned(),
        public_model: cursor.public_model().to_owned(),
        protocol: operational_usage_protocol_response(cursor.protocol()).to_owned(),
        client_key_id: cursor.client_key_id().as_str().to_owned(),
        access_group_id: cursor.access_group_id().map(|id| id.as_str().to_owned()),
    };
    serde_json::to_vec(&wire)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| internal_error())
}

fn decode_operational_usage_cursor(
    value: &str,
) -> Result<Option<OperationalUsageCursor>, ManagementOperationsError> {
    if value.is_empty() {
        return Err(ManagementOperationsError::InvalidQuery);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let wire: OperationalUsageCursorWire =
        serde_json::from_slice(&bytes).map_err(|_| ManagementOperationsError::InvalidQuery)?;
    if wire.provider_id.chars().count() > 128
        || wire.channel_id.chars().count() > 128
        || wire.account_id.chars().count() > 128
        || wire.client_key_id.chars().count() > 128
        || wire.public_model.is_empty()
        || wire.public_model.chars().count() > MAX_USAGE_MODEL_CHARS
        || wire
            .access_group_id
            .as_deref()
            .is_some_and(|id| id.chars().count() > 128)
    {
        return Err(ManagementOperationsError::InvalidQuery);
    }
    let provider_id = UpstreamId::try_new(wire.provider_id)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let channel_id = EndpointId::try_new(wire.channel_id)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let account_id = CredentialId::try_new(wire.account_id)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let client_key_id = ClientKeyId::try_new(wire.client_key_id)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let access_group_id = wire
        .access_group_id
        .map(AccessGroupId::try_new)
        .transpose()
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let protocol = match wire.protocol.as_str() {
        "openai_chat_completions" => GatewayProtocol::OpenAiChatCompletions,
        "openai_responses" => GatewayProtocol::OpenAiResponses,
        "anthropic_messages" => GatewayProtocol::AnthropicMessages,
        _ => return Err(ManagementOperationsError::InvalidQuery),
    };
    OperationalUsageCursor::try_new(
        provider_id,
        channel_id,
        account_id,
        wire.public_model,
        protocol,
        client_key_id,
        access_group_id,
    )
    .map(Some)
}

fn operational_token_confidence_response(value: OperationalTokenConfidence) -> &'static str {
    match value {
        OperationalTokenConfidence::Exact => "exact",
        OperationalTokenConfidence::Partial => "partial",
        OperationalTokenConfidence::Unknown => "unknown",
    }
}

fn operational_token_metric_response(
    value: OperationalTokenMetric,
) -> OperationalTokenMetricResponse {
    OperationalTokenMetricResponse {
        total: value.total,
        confidence: operational_token_confidence_response(value.confidence),
    }
}

fn operational_cost_confidence_response(value: OperationalCostConfidence) -> &'static str {
    match value {
        OperationalCostConfidence::Unpriced => "unpriced",
    }
}

fn operational_usage_page_response(
    value: OperationalUsagePage,
) -> Result<OperationalUsagePageResponse, HttpResponse> {
    let next_cursor = value
        .next_cursor
        .as_ref()
        .map(encode_operational_usage_cursor)
        .transpose()?;
    Ok(OperationalUsagePageResponse {
        observed_through_ms: value.observed_through_ms,
        items: value
            .items
            .into_iter()
            .map(|item| OperationalUsageItemResponse {
                provider_id: item.provider_id.as_str().to_owned(),
                channel_id: item.channel_id.as_str().to_owned(),
                account_id: item.account_id.as_str().to_owned(),
                public_model: item.public_model,
                protocol: operational_usage_protocol_response(item.protocol),
                client_key_id: item.client_key_id.as_str().to_owned(),
                access_group_id: item.access_group_id.map(|id| id.as_str().to_owned()),
                request_count: item.request_count,
                usage_observations: item.usage_observations,
                input_tokens: operational_token_metric_response(item.input_tokens),
                output_tokens: operational_token_metric_response(item.output_tokens),
                reasoning_tokens: operational_token_metric_response(item.reasoning_tokens),
                cache_read_tokens: operational_token_metric_response(item.cache_read_tokens),
                cache_creation_tokens: operational_token_metric_response(
                    item.cache_creation_tokens,
                ),
                cached_tokens: operational_token_metric_response(item.cached_tokens),
                observed_at_ms: item.observed_at_ms,
                cost_microunits: item.cost_microunits,
                cost_confidence: operational_cost_confidence_response(item.cost_confidence),
            })
            .collect(),
        next_cursor,
    })
}

fn operational_billing_status(value: &str) -> Result<OperationalBillingStatus, HttpResponse> {
    match value {
        "exact" => Ok(OperationalBillingStatus::Exact),
        "partial" => Ok(OperationalBillingStatus::Partial),
        "unknown" => Ok(OperationalBillingStatus::Unknown),
        "unpriced" => Ok(OperationalBillingStatus::Unpriced),
        _ => Err(invalid_input()),
    }
}

fn operational_billing_status_response(value: OperationalBillingStatus) -> &'static str {
    match value {
        OperationalBillingStatus::Exact => "exact",
        OperationalBillingStatus::Partial => "partial",
        OperationalBillingStatus::Unknown => "unknown",
        OperationalBillingStatus::Unpriced => "unpriced",
    }
}

fn encode_operational_billing_cursor(
    cursor: &OperationalBillingCursor,
) -> Result<String, HttpResponse> {
    serde_json::to_vec(&OperationalBillingCursorWire {
        snapshot_ledger_id: cursor.snapshot_ledger_id(),
        occurred_at_ms: cursor.occurred_at_ms(),
        ledger_id: cursor.ledger_id(),
    })
    .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
    .map_err(|_| internal_error())
}

fn decode_operational_billing_cursor(
    value: &str,
) -> Result<Option<OperationalBillingCursor>, ManagementOperationsError> {
    if value.is_empty() {
        return Err(ManagementOperationsError::InvalidQuery);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ManagementOperationsError::InvalidQuery)?;
    let wire: OperationalBillingCursorWire =
        serde_json::from_slice(&bytes).map_err(|_| ManagementOperationsError::InvalidQuery)?;
    OperationalBillingCursor::try_new(wire.snapshot_ledger_id, wire.occurred_at_ms, wire.ledger_id)
        .map(Some)
}

fn operational_billing_page_response(
    value: OperationalBillingPage,
) -> Result<OperationalBillingPageResponse, HttpResponse> {
    let next_cursor = value
        .next_cursor
        .as_ref()
        .map(encode_operational_billing_cursor)
        .transpose()?;
    Ok(OperationalBillingPageResponse {
        snapshot_ledger_id: value.snapshot_ledger_id,
        items: value
            .items
            .into_iter()
            .map(|item| OperationalBillingItemResponse {
                ledger_id: item.ledger_id,
                request_id: item.request_id,
                response_id: item.response_id,
                provider_id: item.provider_id,
                channel_id: item.channel_id,
                account_id: item.account_id,
                model: item.model,
                input_tokens: item.usage.input_tokens,
                output_tokens: item.usage.output_tokens,
                reasoning_tokens: item.usage.reasoning_tokens,
                cache_read_tokens: item.usage.cache_read_tokens,
                cache_creation_tokens: item.usage.cache_creation_tokens,
                cached_tokens: item.usage.cached_tokens,
                occurred_at_ms: item.occurred_at_ms,
                catalog_version_id: item.catalog_version_id,
                cost_microunits: item.cost_microunits,
                cost_confidence: operational_billing_status_response(item.cost_confidence),
            })
            .collect(),
        summary: OperationalBillingSummaryResponse {
            records: value.summary.records,
            exact_records: value.summary.exact_records,
            partial_records: value.summary.partial_records,
            unknown_records: value.summary.unknown_records,
            unpriced_records: value.summary.unpriced_records,
            known_cost_microunits: value.summary.known_cost_microunits,
        },
        next_cursor,
    })
}

fn billing_catalog_import(
    input: BillingCatalogImportInput,
) -> Result<BillingCatalogImport, HttpResponse> {
    if input.entries.is_empty()
        || input.entries.len() > MAX_BILLING_CATALOG_ENTRIES
        || input.effective_at_ms > MAX_BILLING_JSON_INTEGER
    {
        return Err(invalid_input());
    }
    let catalog_version_id = bounded_text(input.catalog_version_id, 128)?;
    let source = match input.source.as_str() {
        "operator" => BillingCatalogSource::Operator,
        "imported" => BillingCatalogSource::Imported,
        _ => return Err(invalid_input()),
    };
    let mut identities = BTreeSet::new();
    let entries = input
        .entries
        .into_iter()
        .map(|entry| {
            if [
                entry.input_microunits_per_million,
                entry.output_microunits_per_million,
                entry.reasoning_microunits_per_million,
                entry.cache_read_microunits_per_million,
                entry.cache_creation_microunits_per_million,
                entry.cached_microunits_per_million,
            ]
            .into_iter()
            .any(|rate| rate > MAX_BILLING_JSON_INTEGER)
            {
                return Err(invalid_input());
            }
            let provider_id = bounded_text(entry.provider_id, 128)?;
            let channel_id = bounded_text(entry.channel_id, 128)?;
            let model = bounded_text(entry.model, 512)?;
            if !identities.insert((provider_id.clone(), channel_id.clone(), model.clone())) {
                return Err(invalid_input());
            }
            Ok(BillingPriceEntry {
                provider_id,
                channel_id,
                model,
                input_microunits_per_million: entry.input_microunits_per_million,
                output_microunits_per_million: entry.output_microunits_per_million,
                reasoning_microunits_per_million: entry.reasoning_microunits_per_million,
                cache_read_microunits_per_million: entry.cache_read_microunits_per_million,
                cache_creation_microunits_per_million: entry.cache_creation_microunits_per_million,
                cached_microunits_per_million: entry.cached_microunits_per_million,
            })
        })
        .collect::<Result<Vec<_>, HttpResponse>>()?;
    Ok(BillingCatalogImport {
        catalog_version_id,
        effective_at_ms: input.effective_at_ms,
        source,
        entries,
    })
}

const fn billing_catalog_source_response(value: BillingCatalogSource) -> &'static str {
    match value {
        BillingCatalogSource::Operator => "operator",
        BillingCatalogSource::Imported => "imported",
        BillingCatalogSource::Test => "test",
    }
}

fn billing_catalog_response(value: BillingPriceCatalog) -> BillingCatalogResponse {
    BillingCatalogResponse {
        catalog_version_id: value.catalog_version_id,
        effective_at_ms: value.effective_at_ms,
        source: billing_catalog_source_response(value.source),
        created_at_ms: value.created_at_ms,
        entries: value
            .entries
            .into_iter()
            .map(|entry| BillingCatalogEntryResponse {
                provider_id: entry.provider_id,
                channel_id: entry.channel_id,
                model: entry.model,
                input_microunits_per_million: entry.input_microunits_per_million,
                output_microunits_per_million: entry.output_microunits_per_million,
                reasoning_microunits_per_million: entry.reasoning_microunits_per_million,
                cache_read_microunits_per_million: entry.cache_read_microunits_per_million,
                cache_creation_microunits_per_million: entry.cache_creation_microunits_per_million,
                cached_microunits_per_million: entry.cached_microunits_per_million,
            })
            .collect(),
    }
}

fn billing_catalog_mutation_response(
    value: &BillingCatalogMutationReceipt,
) -> BillingCatalogMutationResponse {
    BillingCatalogMutationResponse {
        catalog_version_id: value.catalog_version_id().to_owned(),
        effective_at_ms: value.effective_at_ms(),
        source: billing_catalog_source_response(value.source()),
        entry_count: value.entry_count(),
        operation: match value.operation() {
            BillingCatalogMutationOperation::Imported => "imported",
            BillingCatalogMutationOperation::RolledBack => "rolled_back",
        },
        rolled_back_from: value.rolled_back_from().map(str::to_owned),
    }
}

fn routing_price_policy_input(
    input: RoutingPricePolicyInput,
) -> Result<RoutingPricePolicyConfiguration, HttpResponse> {
    let catalog_version_id = bounded_text(input.catalog_version_id, 128)?;
    let comparison = match input.comparison.as_str() {
        "rate_dominance_v1" => RoutingPriceComparison::RateDominanceV1,
        _ => return Err(invalid_input()),
    };
    RoutingPricePolicyConfiguration::try_new(catalog_version_id, comparison)
        .map_err(|_| invalid_input())
}

fn routing_price_policy_response(
    value: RoutingPricePolicyConfiguration,
) -> RoutingPricePolicyResponse {
    RoutingPricePolicyResponse {
        catalog_version_id: value.catalog_version_id,
        comparison: match value.comparison {
            RoutingPriceComparison::RateDominanceV1 => "rate_dominance_v1",
        },
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

impl From<ManagementChannelPinReceipt> for ChannelPinResponse {
    fn from(value: ManagementChannelPinReceipt) -> Self {
        Self {
            request_id: value.request_id().as_str().to_owned(),
            config_version_id: value.config_version_id().as_str().to_owned(),
            config_revision: value.config_revision().as_i64(),
            provider_id: value.provider_id().as_str().to_owned(),
            channel_id: value.channel_id().as_str().to_owned(),
            route_id: value.route_id().as_str().to_owned(),
            credential_id: value.credential_id().as_str().to_owned(),
            requested_model: value.requested_model().to_owned(),
            protocol: management_request_protocol_str(value.protocol()),
            mode: value.mode().as_str(),
            outcome: value.outcome().as_str(),
            upstream_sent: value.upstream_sent(),
            attempt_count: value.attempt_count(),
            response_started: value.response_started(),
            observed_at_ms: value.observed_at_ms(),
            stage: value.stage().map(ManagementRequestAttemptStage::as_str),
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
                ManagementCredentialOAuthState::Expired => "expired",
            },
            expires_at_ms: value.expires_at_ms,
            authorization_url: value.authorization_url,
            failure_class: value.failure_class,
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

    #[test]
    fn management_json_parser_rejects_duplicate_keys_at_any_depth() {
        assert!(parse_json::<serde_json::Value>(br#"{"outer":{"value":1,"value":2}}"#).is_err());
        assert!(
            parse_json::<serde_json::Value>(br#"{"outer":{"value":1},"items":[{"value":2}]}"#)
                .is_ok()
        );
    }

    #[test]
    fn codex_authorization_url_hashes_the_printable_pkce_verifier() -> TestResult {
        let session = CodexOAuthSession::start(CredentialId::try_new("cred-codex")?, 1_000)?;
        let (raw_state, raw_verifier) = session.transient_challenge()?;
        let url = url::Url::parse(&authorization_url(&session))?;
        let query: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(
            query.get("state").map(String::as_str),
            Some(
                base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(raw_state)
                    .as_str()
            )
        );
        let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw_verifier);
        let expected_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(verifier.as_bytes()));
        assert_eq!(
            query.get("code_challenge").map(String::as_str),
            Some(expected_challenge.as_str())
        );
        assert_eq!(
            query.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert!(
            query
                .get("scope")
                .is_some_and(|scope| scope.contains("offline_access"))
        );
        Ok(())
    }

    #[test]
    fn codex_oauth_start_is_idempotent_for_a_pending_session() -> TestResult {
        let credential_id = CredentialId::try_new("cred-codex")?;
        let mut workflow = CodexOAuthManagementWorkflow::new();
        let first = workflow.start_oauth(&credential_id);
        let second = workflow.start_oauth(&credential_id);
        assert_eq!(first.state, ManagementCredentialOAuthState::Pending);
        assert_eq!(second.state, ManagementCredentialOAuthState::Pending);
        assert_eq!(first.expires_at_ms, second.expires_at_ms);
        assert_eq!(first.authorization_url, second.authorization_url);
        Ok(())
    }

    #[test]
    fn oauth_refresh_claim_is_exclusive_and_released_on_drop() -> TestResult {
        let claims = Mutex::new(BTreeSet::new());
        let credential_id = CredentialId::try_new("cred-codex")?;
        let first = OAuthRefreshClaim::try_acquire(&claims, credential_id.clone())
            .ok_or("first refresh claim was rejected")?;
        assert!(OAuthRefreshClaim::try_acquire(&claims, credential_id.clone()).is_none());
        drop(first);
        assert!(OAuthRefreshClaim::try_acquire(&claims, credential_id).is_some());
        Ok(())
    }

    #[test]
    fn mismatched_callback_state_does_not_consume_the_pending_session() -> TestResult {
        let credential_id = CredentialId::try_new("cred-codex")?;
        let mut workflow = CodexOAuthManagementWorkflow::new();
        let started = workflow.start_oauth(&credential_id);
        let url = url::Url::parse(started.authorization_url.as_deref().ok_or("missing URL")?)?;
        let state = url
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned())
            .ok_or("missing state")?;
        assert!(
            workflow
                .complete_oauth(
                    &credential_id,
                    b"attacker-state",
                    Zeroizing::new("code".to_owned()),
                )
                .is_none()
        );
        assert_eq!(
            workflow.oauth_status(&credential_id).state,
            ManagementCredentialOAuthState::Pending
        );
        // The legitimate callback remains the only path allowed to claim the session.  The
        // default exchange rejects it, but it must now be classified as exchange failure rather
        // than state mismatch/terminalized by the attacker attempt.
        let decoded = decode_oauth_state(state.as_bytes()).ok_or("invalid state")?;
        assert!(
            workflow
                .complete_oauth(&credential_id, &decoded, Zeroizing::new("code".to_owned()),)
                .is_none()
        );
        assert_eq!(
            workflow.oauth_status(&credential_id).failure_class,
            Some("token_exchange_failed")
        );
        Ok(())
    }

    #[test]
    fn codex_token_response_normalization_ignores_protocol_metadata() -> TestResult {
        let body = br#"{
            "access_token":"access-value",
            "refresh_token":"refresh-value",
            "expires_in":3600,
            "account_id":"account-value",
            "token_type":"Bearer",
            "scope":"openid email offline_access"
        }"#;
        let envelope = normalize_codex_oauth_response(body, 1_000_000)
            .map_err(|_| std::io::Error::other("token response was rejected"))?;
        let imported =
            OpenAiCompatibleRuntimeCredential::import_compatible(envelope.as_slice(), 1_000_000)?;
        assert!(imported.has_account_binding());
        let text = std::str::from_utf8(envelope.as_slice())?;
        assert!(!text.contains("token_type"));
        assert!(!text.contains("scope"));
        Ok(())
    }

    #[test]
    fn codex_token_response_normalization_accepts_jwt_expiry_and_rejects_duplicates() -> TestResult
    {
        let id_token = "header.eyJleHAiOjIwMDAsImVtYWlsIjoidXNlckBleGFtcGxlLnRlc3QiLCJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjb3VudCJ9fQ.signature";
        let body = format!(
            r#"{{"access_token":"opaque-access","refresh_token":"refresh","id_token":"{id_token}","token_type":"Bearer"}}"#
        );
        let envelope = normalize_codex_oauth_response(body.as_bytes(), 1_000)
            .map_err(|_| std::io::Error::other("JWT-expiry token response was rejected"))?;
        let credential =
            OpenAiCompatibleRuntimeCredential::import_compatible(envelope.as_slice(), 1_000)?;
        assert_eq!(credential.bearer_at(1_999_999)?, "opaque-access");
        let duplicate =
            br#"{"access_token":"a","access_token":"b","refresh_token":"r","expires_in":30}"#;
        assert!(normalize_codex_oauth_response(duplicate, 1_000).is_err());
        Ok(())
    }

    #[test]
    fn oauth_callback_parser_accepts_relay_pairs_and_full_browser_urls() -> TestResult {
        let relay = CredentialOAuthCallbackRequest {
            state: "state-value".to_owned(),
            code: "code-value".to_owned(),
            error: None,
            callback_url: None,
        };
        let parsed = parse_oauth_callback_request(&relay).map_err(|_| "relay pair rejected")?;
        assert_eq!(parsed.state, "state-value");
        assert_eq!(parsed.code, "code-value");

        let full = CredentialOAuthCallbackRequest {
            state: String::new(),
            code: "http://localhost:1455/auth/callback?code=code-value&state=state-value"
                .to_owned(),
            error: None,
            callback_url: None,
        };
        let parsed = parse_oauth_callback_request(&full).map_err(|_| "callback URL rejected")?;
        assert_eq!(parsed.state, "state-value");
        assert_eq!(parsed.code, "code-value");

        let fragment = CredentialOAuthCallbackRequest {
            state: String::new(),
            code: String::new(),
            error: None,
            callback_url: Some(
                "http://localhost:1455/auth/callback#code=code-value&state=state-value".to_owned(),
            ),
        };
        let parsed = parse_oauth_callback_request(&fragment)
            .map_err(|_| "fragment callback URL rejected")?;
        assert_eq!(parsed.state, "state-value");
        assert_eq!(parsed.code, "code-value");
        Ok(())
    }

    #[test]
    fn oauth_callback_parser_rejects_state_conflicts_and_provider_errors() {
        let conflict = CredentialOAuthCallbackRequest {
            state: "state-a".to_owned(),
            code: "http://localhost:1455/auth/callback?code=code&state=state-b".to_owned(),
            error: None,
            callback_url: None,
        };
        assert!(matches!(
            parse_oauth_callback_request(&conflict),
            Err(OAuthCallbackInputError::Invalid)
        ));
        let error = CredentialOAuthCallbackRequest {
            state: String::new(),
            code: "http://localhost:1455/auth/callback?error=access_denied".to_owned(),
            error: None,
            callback_url: None,
        };
        assert!(matches!(
            parse_oauth_callback_request(&error),
            Err(OAuthCallbackInputError::ProviderRejected { state: None })
        ));
        let error_with_state = CredentialOAuthCallbackRequest {
            state: String::new(),
            code: "http://localhost:1455/auth/callback?error=access_denied&state=state-value"
                .to_owned(),
            error: None,
            callback_url: None,
        };
        assert!(matches!(
            parse_oauth_callback_request(&error_with_state),
            Err(OAuthCallbackInputError::ProviderRejected { state: Some(value) })
                if value == "state-value"
        ));
    }
}
