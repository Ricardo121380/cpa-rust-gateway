//! Typed version-scoped control-plane Repository and transactional write boundary.
//!
//! This module deliberately models persisted administrative configuration only. It never enters
//! a Provider trait or the inference request path. P2-06 compiles these rows into a validated
//! runtime view; P2-07 owns publication of that view.

use std::{fmt, path::Path};

use gateway_core::{
    AccessGroupId, ClientKeyId, CredentialId, EgressPolicyId, EndpointId, InvalidIdentifier,
    PublicModelId, RouteCandidateId, RouteId, UpstreamId,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{
    StoreError, StoreResult,
    billing_ledger::{
        BillingPriceCatalog, insert_catalog_in_transaction, list_catalogs_bounded_from_connection,
        load_catalog_from_connection,
    },
    migrate,
    secret_store::{EncryptedSecret, KeyVersion},
};

const CLIENT_KEY_DIGEST_BYTES: usize = 32;

/// Stable identifier for one version-scoped administrative configuration graph.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConfigVersionId(String);

impl ConfigVersionId {
    /// Creates a non-empty opaque Config Version identifier.
    ///
    /// `SQLite` retains the final bounded and non-whitespace admission checks. Keeping this type
    /// opaque preserves version identity without prematurely assigning publication semantics.
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

    /// Returns the opaque Config Version representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ConfigVersionId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ConfigVersionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<String> for ConfigVersionId {
    type Error = InvalidIdentifier;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for ConfigVersionId {
    type Error = InvalidIdentifier;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

/// Persisted lifecycle state of a configuration graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigVersionStatus {
    /// A structurally writable configuration that has not been published.
    Draft,
    /// The only active configuration allowed by the schema.
    Active,
    /// A retained historical configuration that is not active.
    Archived,
}

impl ConfigVersionStatus {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

/// Persisted lifecycle state of an upstream credential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialStatus {
    /// The encrypted credential is eligible for later scheduling.
    Active,
    /// The credential is retained while a future runtime cooldown is in effect.
    Cooling,
    /// The credential requires reauthorization before it can be eligible again.
    Unauthorized,
    /// The credential is administratively disabled.
    Disabled,
}

impl CredentialStatus {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Cooling => "cooling",
            Self::Unauthorized => "unauthorized",
            Self::Disabled => "disabled",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "cooling" => Some(Self::Cooling),
            "unauthorized" => Some(Self::Unauthorized),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }
}

/// Persisted administrative state shared by Public Models and Access Groups.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdministrativeStatus {
    /// The record is administratively enabled.
    Active,
    /// The record remains stored but is administratively disabled.
    Disabled,
}

impl AdministrativeStatus {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }
}

/// Endpoint transport admitted by the P2-01 schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointTransport {
    /// Plain request/response HTTP transport.
    Http,
    /// Server-sent events transport.
    Sse,
    /// WebSocket transport.
    Websocket,
}

impl EndpointTransport {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Sse => "sse",
            Self::Websocket => "websocket",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "http" => Some(Self::Http),
            "sse" => Some(Self::Sse),
            "websocket" => Some(Self::Websocket),
            _ => None,
        }
    }
}

/// Persisted redirect behavior for one `EgressPolicy`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredEgressRedirectMode {
    /// Redirect responses must not be followed.
    Deny,
    /// Only same-origin, fully revalidated redirects may be followed.
    SameOrigin,
    /// Fully revalidated redirects to another configured Host may be followed.
    Revalidate,
}

impl StoredEgressRedirectMode {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::SameOrigin => "same_origin",
            Self::Revalidate => "revalidate",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "deny" => Some(Self::Deny),
            "same_origin" => Some(Self::SameOrigin),
            "revalidate" => Some(Self::Revalidate),
            _ => None,
        }
    }
}

/// Persisted Client Key lifecycle state, independent from the cryptographic implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredClientKeyStatus {
    /// The Key may be used subject to its optional expiry.
    Active,
    /// The Key remains stored but cannot authenticate.
    Disabled,
    /// The Key has been permanently revoked.
    Revoked,
}

impl StoredClientKeyStatus {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Revoked => "revoked",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "disabled" => Some(Self::Disabled),
            "revoked" => Some(Self::Revoked),
            _ => None,
        }
    }
}

/// Route scheduling policy stored by P2-02.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutePolicy {
    /// Select candidates in ordinary rotation.
    RoundRobin,
    /// Select candidates with a future smooth weighted round-robin scheduler.
    SmoothWeightedRoundRobin,
    /// Prefer lower-priority tiers and fail over only when needed.
    PriorityFailover,
}

impl RoutePolicy {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::RoundRobin => "round_robin",
            Self::SmoothWeightedRoundRobin => "smooth_weighted_round_robin",
            Self::PriorityFailover => "priority_failover",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "round_robin" => Some(Self::RoundRobin),
            "smooth_weighted_round_robin" => Some(Self::SmoothWeightedRoundRobin),
            "priority_failover" => Some(Self::PriorityFailover),
            _ => None,
        }
    }
}

/// The only Candidate credential scope admitted by P2-02.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialScope {
    /// Later scheduling chooses among the selected Endpoint's bindings.
    EndpointBindings,
}

impl CredentialScope {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::EndpointBindings => "endpoint_bindings",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "endpoint_bindings" => Some(Self::EndpointBindings),
            _ => None,
        }
    }
}

/// The structural conversion mode for a Route Candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformMode {
    /// The request may be forwarded without a canonical conversion.
    Passthrough,
    /// The request uses the canonical gateway representation.
    Canonical,
    /// The request uses a later lossless compatibility bridge.
    LosslessBridge,
    /// The request uses Canonical semantics for the native protocol and a proven lossless
    /// bridge when the client speaks another registered protocol.
    CanonicalBridge,
}

impl TransformMode {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Passthrough => "passthrough",
            Self::Canonical => "canonical",
            Self::LosslessBridge => "lossless_bridge",
            Self::CanonicalBridge => "canonical_bridge",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "passthrough" => Some(Self::Passthrough),
            "canonical" => Some(Self::Canonical),
            "lossless_bridge" => Some(Self::LosslessBridge),
            "canonical_bridge" => Some(Self::CanonicalBridge),
            _ => None,
        }
    }
}

/// Root row of a version-scoped control-plane graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigVersion {
    /// Stable graph identity.
    pub id: ConfigVersionId,
    /// Optional parent graph identity retained for later clone/rollback policy.
    pub parent_id: Option<ConfigVersionId>,
    /// Persisted configuration state; P2-05 does not publish it.
    pub status: ConfigVersionStatus,
    /// Monotonic draft-graph revision used for fail-closed management writes.
    pub revision: i64,
    /// Creation timestamp in Unix milliseconds.
    pub created_at_ms: i64,
    /// Non-secret operator description.
    pub description: String,
}

/// One configured upstream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamConfiguration {
    /// Stable upstream identity within the version.
    pub id: UpstreamId,
    /// Non-secret unique upstream name within the version.
    pub name: String,
    /// Provider family or upstream kind.
    pub kind: String,
    /// Administrative eligibility bit.
    pub enabled: bool,
    /// Structural JSON array; semantic validation is deferred to P2-06.
    pub tags_json: String,
    /// Optional same-version `EgressPolicy` reference; enabled use is validated by P2-09.
    pub egress_policy_id: Option<EgressPolicyId>,
}

/// Version-scoped structural storage for one outbound `EgressPolicy`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EgressPolicyConfiguration {
    /// Stable policy identity within the Config Version.
    pub id: EgressPolicyId,
    /// Non-secret unique policy name within the Config Version.
    pub name: String,
    /// Structural JSON array of scheme labels, parsed semantically by P2-09.
    pub allowed_schemes_json: String,
    /// Structural JSON array of exact Host labels, parsed semantically by P2-09.
    pub allowed_hosts_json: String,
    /// Structural JSON array of effective port integers, parsed semantically by P2-09.
    pub allowed_ports_json: String,
    /// Structural JSON array of CIDR strings, parsed semantically by P2-09.
    pub allowed_cidrs_json: String,
    /// Redirect mode stored independently from the transport implementation.
    pub redirect_mode: StoredEgressRedirectMode,
    /// Bound for an enabled redirect mode; zero only when redirects are denied.
    pub max_redirects: i64,
}

/// One protocol-specific upstream endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointConfiguration {
    /// Stable endpoint identity within the version.
    pub id: EndpointId,
    /// The owning upstream.
    pub upstream_id: UpstreamId,
    /// Later Provider adapter identifier.
    pub adapter_id: String,
    /// Single upstream protocol format.
    pub api_format: String,
    /// Configured base URL; P2-09 validates egress admission.
    pub base_url: String,
    /// Configured inference path.
    pub inference_path: String,
    /// Optional catalog path.
    pub models_path: Option<String>,
    /// Validated endpoint transport.
    pub transport: EndpointTransport,
    /// Administrative eligibility bit.
    pub enabled: bool,
}

/// One opaque AEAD-protected upstream credential.
pub struct CredentialConfiguration {
    /// Stable credential identity within the version.
    pub id: CredentialId,
    /// The owning upstream.
    pub upstream_id: UpstreamId,
    /// Non-secret credential kind.
    pub kind: String,
    /// Opaque encrypted envelope; never plaintext credential material.
    pub encrypted_secret: EncryptedSecret,
    /// Persisted lifecycle state.
    pub status: CredentialStatus,
    /// Non-negative record revision.
    pub revision: i64,
}

impl fmt::Debug for CredentialConfiguration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialConfiguration")
            .field("id", &self.id)
            .field("upstream_id", &self.upstream_id)
            .field("kind", &self.kind)
            .field("encrypted_secret", &self.encrypted_secret)
            .field("status", &self.status)
            .field("revision", &self.revision)
            .finish()
    }
}

/// One Endpoint-to-Credential scheduling binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointCredentialBindingConfiguration {
    /// The bound endpoint.
    pub endpoint_id: EndpointId,
    /// The bound encrypted credential.
    pub credential_id: CredentialId,
    /// The common owning upstream required by the composite foreign keys.
    pub upstream_id: UpstreamId,
    /// Administrative eligibility bit.
    pub enabled: bool,
    /// Lower value has higher future priority.
    pub priority: i64,
    /// Positive scheduling weight.
    pub weight: i64,
    /// Positive future concurrency limit.
    pub concurrency: i64,
}

/// One client-visible public model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicModelConfiguration {
    /// Stable public model identity within the version.
    pub id: PublicModelId,
    /// Exact public model name.
    pub model_name: String,
    /// Persisted administrative state.
    pub status: AdministrativeStatus,
    /// Non-secret display label.
    pub display_name: String,
    /// Structural JSON object; semantic validation is deferred to P2-06.
    pub capabilities_json: String,
}

/// One exact Alias-to-Public-Model relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelAliasConfiguration {
    /// Exact alias text.
    pub alias: String,
    /// Referenced public model.
    pub public_model_id: PublicModelId,
}

/// One public-model routing configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRouteConfiguration {
    /// Stable Route identity within the version.
    pub id: RouteId,
    /// Referenced public model.
    pub public_model_id: PublicModelId,
    /// Later runtime selection policy.
    pub policy: RoutePolicy,
    /// Positive attempt bound.
    pub max_attempts: i64,
    /// Positive first-event bootstrap timeout.
    pub bootstrap_timeout_ms: i64,
}

/// One structural candidate for a Route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteCandidateConfiguration {
    /// Stable Candidate identity within the version.
    pub id: RouteCandidateId,
    /// Referenced route.
    pub route_id: RouteId,
    /// Referenced endpoint.
    pub endpoint_id: EndpointId,
    /// Model text sent to the upstream.
    pub upstream_model: String,
    /// Credential selection scope.
    pub credential_scope: CredentialScope,
    /// Requested conversion mode.
    pub transform_mode: TransformMode,
    /// Administrative eligibility bit.
    pub enabled: bool,
    /// Lower value has higher future priority.
    pub priority: i64,
    /// Positive scheduling weight.
    pub weight: i64,
    /// Structural JSON object; semantic validation is deferred to P2-06.
    pub capability_override_json: String,
}

/// One Access Group definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessGroupConfiguration {
    /// Stable Access Group identity within the version.
    pub id: AccessGroupId,
    /// Non-secret unique group name within the version.
    pub name: String,
    /// Persisted administrative state.
    pub status: AdministrativeStatus,
    /// Structural JSON object; semantic validation is deferred to P2-06.
    pub limits_json: String,
}

/// One Access Group-to-Route permission relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessGroupRouteConfiguration {
    /// Referenced Access Group.
    pub access_group_id: AccessGroupId,
    /// Referenced Route.
    pub route_id: RouteId,
    /// Administrative eligibility bit.
    pub enabled: bool,
}

/// Persisted non-secret Client Key fields. Its digest is opaque and always redacted from `Debug`.
#[derive(Clone, Eq, PartialEq)]
pub struct StoredClientKey {
    id: ClientKeyId,
    access_group_id: AccessGroupId,
    prefix: String,
    secret_digest: [u8; CLIENT_KEY_DIGEST_BYTES],
    status: StoredClientKeyStatus,
    expires_at_ms: Option<i64>,
}

impl StoredClientKey {
    /// Creates a storage-safe Client Key row from opaque HMAC material.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidClientKeyDigestLength`] unless `secret_digest` is exactly
    /// 32 bytes, or [`StoreError::InvalidPersistedControlPlaneRecord`] for a negative expiry.
    pub fn try_new(
        id: ClientKeyId,
        access_group_id: AccessGroupId,
        prefix: impl Into<String>,
        secret_digest: impl AsRef<[u8]>,
        status: StoredClientKeyStatus,
        expires_at_ms: Option<i64>,
    ) -> StoreResult<Self> {
        let secret_digest = secret_digest.as_ref();
        if secret_digest.len() != CLIENT_KEY_DIGEST_BYTES {
            return Err(StoreError::InvalidClientKeyDigestLength {
                actual: secret_digest.len(),
            });
        }
        if expires_at_ms.is_some_and(|value| value < 0) {
            return Err(StoreError::InvalidPersistedControlPlaneRecord {
                table: "client_keys",
            });
        }

        let mut digest = [0_u8; CLIENT_KEY_DIGEST_BYTES];
        digest.copy_from_slice(secret_digest);
        Ok(Self {
            id,
            access_group_id,
            prefix: prefix.into(),
            secret_digest: digest,
            status,
            expires_at_ms,
        })
    }

    /// Returns the stable non-secret Client Key identifier.
    #[must_use]
    pub fn id(&self) -> &ClientKeyId {
        &self.id
    }

    /// Returns the referenced Access Group identity.
    #[must_use]
    pub fn access_group_id(&self) -> &AccessGroupId {
        &self.access_group_id
    }

    /// Returns the non-secret indexed Prefix.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Returns the opaque HMAC digest only for persistence or later snapshot compilation.
    #[must_use]
    pub fn secret_digest(&self) -> &[u8; CLIENT_KEY_DIGEST_BYTES] {
        &self.secret_digest
    }

    /// Returns the persisted lifecycle state.
    #[must_use]
    pub const fn status(&self) -> StoredClientKeyStatus {
        self.status
    }

    /// Returns the optional expiry in Unix milliseconds.
    #[must_use]
    pub const fn expires_at_ms(&self) -> Option<i64> {
        self.expires_at_ms
    }
}

impl fmt::Debug for StoredClientKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredClientKey")
            .field("id", &self.id)
            .field("access_group_id", &self.access_group_id)
            .field("prefix", &self.prefix)
            .field("secret_digest", &"<redacted>")
            .field("status", &self.status)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

/// Every P2-01/P2-02 table belonging to one configuration version.
#[derive(Debug)]
pub struct ControlPlaneConfiguration {
    /// The graph root.
    pub version: ConfigVersion,
    /// Version-scoped outbound `EgressPolicies`.
    pub egress_policies: Vec<EgressPolicyConfiguration>,
    /// Version-scoped Upstreams.
    pub upstreams: Vec<UpstreamConfiguration>,
    /// Version-scoped Endpoints.
    pub endpoints: Vec<EndpointConfiguration>,
    /// Version-scoped opaque Credentials.
    pub credentials: Vec<CredentialConfiguration>,
    /// Version-scoped Endpoint/Credential bindings.
    pub endpoint_credential_bindings: Vec<EndpointCredentialBindingConfiguration>,
    /// Version-scoped public models.
    pub public_models: Vec<PublicModelConfiguration>,
    /// Version-scoped aliases.
    pub model_aliases: Vec<ModelAliasConfiguration>,
    /// Version-scoped routes.
    pub model_routes: Vec<ModelRouteConfiguration>,
    /// Version-scoped route candidates.
    pub route_candidates: Vec<RouteCandidateConfiguration>,
    /// Version-scoped Access Groups.
    pub access_groups: Vec<AccessGroupConfiguration>,
    /// Version-scoped Access Group-to-Route relations.
    pub access_group_routes: Vec<AccessGroupRouteConfiguration>,
    /// Version-scoped Client Key records with opaque digests.
    pub client_keys: Vec<StoredClientKey>,
}

/// One atomic persisted Config Version activation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigVersionActivation {
    activated_version_id: ConfigVersionId,
    replaced_active_version_id: Option<ConfigVersionId>,
}

impl ConfigVersionActivation {
    fn new(
        activated_version_id: ConfigVersionId,
        replaced_active_version_id: Option<ConfigVersionId>,
    ) -> Self {
        Self {
            activated_version_id,
            replaced_active_version_id,
        }
    }

    /// Returns the Version that became active.
    #[must_use]
    pub fn activated_version_id(&self) -> &ConfigVersionId {
        &self.activated_version_id
    }

    /// Returns the active Version archived by this transition, if one existed.
    #[must_use]
    pub fn replaced_active_version_id(&self) -> Option<&ConfigVersionId> {
        self.replaced_active_version_id.as_ref()
    }
}

/// Audited management operation that changed durable configuration visibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementAuditAction {
    /// A complete draft Config Version was created in one transaction.
    Created,
    /// A draft or archived Config Version became active and replaced the prior active Version.
    Published,
    /// A retained predecessor became active through an explicit rollback.
    RolledBack,
}

impl ManagementAuditAction {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Created => "config_created",
            Self::Published => "config_published",
            Self::RolledBack => "config_rolled_back",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "config_created" => Some(Self::Created),
            "config_published" => Some(Self::Published),
            "config_rolled_back" => Some(Self::RolledBack),
            _ => None,
        }
    }
}

/// Bounded, non-secret metadata supplied while recording one management audit event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementAuditEventDraft {
    action: ManagementAuditAction,
    actor: String,
    occurred_at_ms: i64,
}

impl ManagementAuditEventDraft {
    /// Creates bounded metadata for one management event.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidManagementAuditEvent`] when the actor is empty, exceeds the
    /// persisted 128-character bound, or the supplied Unix timestamp is negative.
    pub fn try_new(
        action: ManagementAuditAction,
        actor: impl Into<String>,
        occurred_at_ms: i64,
    ) -> StoreResult<Self> {
        let actor = actor.into();
        if actor.is_empty() || actor.chars().count() > 128 || occurred_at_ms < 0 {
            return Err(StoreError::InvalidManagementAuditEvent);
        }
        Ok(Self {
            action,
            actor,
            occurred_at_ms,
        })
    }

    /// Returns the operation that this draft records.
    #[must_use]
    pub const fn action(&self) -> ManagementAuditAction {
        self.action
    }

    /// Returns the non-secret actor label.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Returns the supplied Unix-millisecond timestamp.
    #[must_use]
    pub const fn occurred_at_ms(&self) -> i64 {
        self.occurred_at_ms
    }
}

/// One durable, append-only management audit event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementAuditEvent {
    id: i64,
    action: ManagementAuditAction,
    actor: String,
    occurred_at_ms: i64,
    config_version_id: ConfigVersionId,
    replaced_config_version_id: Option<ConfigVersionId>,
}

/// Bounded, non-secret identity metadata for one versioned resource mutation.
///
/// This audit stream is separate from the P2 Config Version lifecycle stream so a later audit
/// page can distinguish a graph publication from a protected draft-resource edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementResourceAuditEventDraft {
    action: String,
    actor: String,
    occurred_at_ms: i64,
    resource_kind: String,
    resource_id: String,
}

/// One durable append-only record of a protected resource mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementResourceAuditEvent {
    id: i64,
    action: String,
    actor: String,
    occurred_at_ms: i64,
    config_version_id: ConfigVersionId,
    resource_kind: String,
    resource_id: String,
}

impl ManagementResourceAuditEvent {
    /// Returns the monotonic durable event identifier.
    #[must_use]
    pub const fn id(&self) -> i64 {
        self.id
    }

    /// Returns the bounded mutation operation label.
    #[must_use]
    pub fn action(&self) -> &str {
        &self.action
    }

    /// Returns the protected request actor identity.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Returns the event time in Unix milliseconds.
    #[must_use]
    pub const fn occurred_at_ms(&self) -> i64 {
        self.occurred_at_ms
    }

    /// Returns the Version whose draft graph changed.
    #[must_use]
    pub fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Returns the audited resource category.
    #[must_use]
    pub fn resource_kind(&self) -> &str {
        &self.resource_kind
    }

    /// Returns the opaque audited resource ID.
    #[must_use]
    pub fn resource_id(&self) -> &str {
        &self.resource_id
    }
}

impl ManagementResourceAuditEventDraft {
    /// Creates bounded resource mutation audit metadata without accepting any Secret or payload.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidManagementAuditEvent`] when a supplied field is empty,
    /// oversized, or the timestamp is negative.
    pub fn try_new(
        action: impl Into<String>,
        actor: impl Into<String>,
        occurred_at_ms: i64,
        resource_kind: impl Into<String>,
        resource_id: impl Into<String>,
    ) -> StoreResult<Self> {
        let action = action.into();
        let actor = actor.into();
        let resource_kind = resource_kind.into();
        let resource_id = resource_id.into();
        if !bounded_non_empty(&action, 64)
            || !bounded_non_empty(&actor, 128)
            || occurred_at_ms < 0
            || !bounded_non_empty(&resource_kind, 64)
            || !bounded_non_empty(&resource_id, 128)
        {
            return Err(StoreError::InvalidManagementAuditEvent);
        }
        Ok(Self {
            action,
            actor,
            occurred_at_ms,
            resource_kind,
            resource_id,
        })
    }

    /// Returns the bounded non-secret operation label.
    #[must_use]
    pub fn action(&self) -> &str {
        &self.action
    }

    /// Returns the request-principal actor label.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Returns the associated timestamp.
    #[must_use]
    pub const fn occurred_at_ms(&self) -> i64 {
        self.occurred_at_ms
    }

    /// Returns the audited resource category.
    #[must_use]
    pub fn resource_kind(&self) -> &str {
        &self.resource_kind
    }

    /// Returns the audited opaque resource identifier.
    #[must_use]
    pub fn resource_id(&self) -> &str {
        &self.resource_id
    }
}

impl ManagementAuditEvent {
    fn from_draft(
        id: i64,
        draft: &ManagementAuditEventDraft,
        config_version_id: ConfigVersionId,
        replaced_config_version_id: Option<ConfigVersionId>,
    ) -> StoreResult<Self> {
        if id <= 0 {
            return Err(StoreError::InvalidPersistedControlPlaneRecord {
                table: "management_audit_events",
            });
        }
        Ok(Self {
            id,
            action: draft.action,
            actor: draft.actor.clone(),
            occurred_at_ms: draft.occurred_at_ms,
            config_version_id,
            replaced_config_version_id,
        })
    }

    /// Returns the monotonically increasing durable audit identifier.
    #[must_use]
    pub const fn id(&self) -> i64 {
        self.id
    }

    /// Returns the audited management operation.
    #[must_use]
    pub const fn action(&self) -> ManagementAuditAction {
        self.action
    }

    /// Returns the non-secret actor label.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Returns the operation timestamp in Unix milliseconds.
    #[must_use]
    pub const fn occurred_at_ms(&self) -> i64 {
        self.occurred_at_ms
    }

    /// Returns the Config Version made or kept by the operation.
    #[must_use]
    pub fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Returns the active Config Version replaced by a publish or rollback, if one existed.
    #[must_use]
    pub fn replaced_config_version_id(&self) -> Option<&ConfigVersionId> {
        self.replaced_config_version_id.as_ref()
    }
}

impl ControlPlaneConfiguration {
    /// Starts an empty graph rooted at `version`.
    #[must_use]
    pub fn new(version: ConfigVersion) -> Self {
        Self {
            version,
            egress_policies: Vec::new(),
            upstreams: Vec::new(),
            endpoints: Vec::new(),
            credentials: Vec::new(),
            endpoint_credential_bindings: Vec::new(),
            public_models: Vec::new(),
            model_aliases: Vec::new(),
            model_routes: Vec::new(),
            route_candidates: Vec::new(),
            access_groups: Vec::new(),
            access_group_routes: Vec::new(),
            client_keys: Vec::new(),
        }
    }
}

/// A `SQLite`-backed Repository for versioned control-plane graphs.
pub struct SqliteControlPlaneRepository {
    connection: Connection,
}

impl SqliteControlPlaneRepository {
    /// Opens, migrates, and owns a file-backed control-plane database.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the database cannot be opened, foreign keys cannot be enabled,
    /// or the migration history is not supported.
    pub fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        let connection = crate::open(path)?;
        Self::from_connection(connection)
    }

    /// Opens, migrates, and owns an in-memory control-plane database.
    ///
    /// This is primarily useful for isolated tests and command-line validation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for a `SQLite` or migration failure.
    pub fn open_in_memory() -> StoreResult<Self> {
        let connection = crate::open_in_memory()?;
        Self::from_connection(connection)
    }

    /// Takes an existing `SQLite` connection, applies all known migrations, and owns it.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when migration or foreign-key enforcement fails.
    pub fn from_connection(mut connection: Connection) -> StoreResult<Self> {
        migrate(&mut connection)?;
        Ok(Self { connection })
    }

    /// Starts one write transaction. Dropping the returned value before [`ControlPlaneTransaction::commit`]
    /// rolls back all of its writes.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when `SQLite` cannot begin the transaction.
    pub fn begin_transaction(&mut self) -> StoreResult<ControlPlaneTransaction<'_>> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        Ok(ControlPlaneTransaction { transaction })
    }

    /// Writes one complete configuration graph in one transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] and leaves no partial graph when any later row violates a `SQLite`
    /// constraint or cannot be written.
    pub fn write_configuration(
        &mut self,
        configuration: &ControlPlaneConfiguration,
    ) -> StoreResult<()> {
        let mut transaction = self.begin_transaction()?;
        transaction.write_configuration(configuration)?;
        transaction.commit()
    }

    /// Writes one complete draft configuration and its `config_created` audit event atomically.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] and leaves neither a partial graph nor an audit event when the
    /// configuration or its audit metadata is invalid.
    pub fn write_configuration_with_audit(
        &mut self,
        configuration: &ControlPlaneConfiguration,
        audit_draft: &ManagementAuditEventDraft,
    ) -> StoreResult<ManagementAuditEvent> {
        if audit_draft.action() != ManagementAuditAction::Created {
            return Err(StoreError::InvalidManagementAuditEvent);
        }
        let mut transaction = self.begin_transaction()?;
        transaction.write_configuration(configuration)?;
        let audit_event = transaction.record_management_audit_event(
            audit_draft,
            configuration.version.id.clone(),
            None,
        )?;
        transaction.commit()?;
        Ok(audit_event)
    }

    /// Loads every persisted P2-01/P2-02 row belonging to one Config Version.
    ///
    /// An absent Config Version returns `Ok(None)`. Malformed stored encrypted envelopes, Key
    /// Versions, or Client Key digests fail closed and are never returned as a partial graph.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for `SQLite` failures or malformed persisted control-plane records.
    pub fn load_configuration(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> StoreResult<Option<ControlPlaneConfiguration>> {
        let transaction = self.connection.transaction()?;
        let configuration = load_configuration(&transaction, config_version_id)?;
        transaction.commit()?;
        Ok(configuration)
    }

    /// Lists safe Config Version root metadata in deterministic identifier order.
    ///
    /// This projection deliberately excludes every graph resource, Credential envelope, Client
    /// Key digest, and audit payload. It is the safe metadata view needed by a management UI;
    /// callers that require a complete graph must continue to use [`Self::load_configuration`].
    ///
    /// # Errors
    ///
    /// Returns a fail-closed error if any persisted Config Version row is malformed or cannot be
    /// read consistently.
    pub fn list_config_versions(&mut self) -> StoreResult<Vec<ConfigVersion>> {
        let transaction = self.connection.transaction()?;
        let versions = load_config_versions(&transaction)?;
        transaction.commit()?;
        Ok(versions)
    }

    /// Loads one immutable billing price catalog from the same migrated control-plane database.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed [`StoreError`] when the catalog is malformed or cannot be read.
    pub fn load_billing_catalog(
        &mut self,
        catalog_version_id: &str,
    ) -> StoreResult<Option<BillingPriceCatalog>> {
        load_catalog_from_connection(&self.connection, catalog_version_id)
    }

    /// Returns a bounded, deterministic list of immutable billing price catalogs.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed [`StoreError`] when the bound or persisted catalog is invalid.
    pub fn list_billing_catalogs_bounded(
        &self,
        limit: usize,
    ) -> StoreResult<Vec<BillingPriceCatalog>> {
        list_catalogs_bounded_from_connection(&self.connection, limit)
    }

    /// Loads exactly one safe Config Version root metadata record.
    ///
    /// Unlike [`Self::load_configuration`], this projection reads no graph rows, Credential
    /// envelope, Client Key digest, or resource definition. It is suitable for control-plane
    /// read views that need only a Version identity, status, revision, timestamp, and
    /// description.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed error if the persisted Config Version root cannot be decoded.
    pub fn load_config_version(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> StoreResult<Option<ConfigVersion>> {
        let transaction = self.connection.transaction()?;
        let version = load_config_version(&transaction, config_version_id)?;
        transaction.commit()?;
        Ok(version)
    }

    /// Loads the one currently active Config Version, if a publication has occurred.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed error if the active row cannot be decoded into the complete typed
    /// configuration graph.
    pub fn load_active_configuration(&mut self) -> StoreResult<Option<ControlPlaneConfiguration>> {
        let active_id: Option<String> = self
            .connection
            .query_row(
                "SELECT id FROM config_versions WHERE status = ?1",
                [ConfigVersionStatus::Active.as_sql()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(active_id) = active_id else {
            return Ok(None);
        };
        let active_id =
            ConfigVersionId::try_new(active_id).map_err(|_| malformed("config_versions"))?;
        let configuration = self
            .load_configuration(&active_id)?
            .ok_or_else(|| malformed("config_versions"))?;
        if configuration.version.status != ConfigVersionStatus::Active {
            return Err(malformed("config_versions"));
        }
        Ok(Some(configuration))
    }

    /// Returns durable management audit events in increasing append order.
    ///
    /// The event records contain only operation metadata and Config Version identifiers; they
    /// never contain credentials, ciphertext, Client Key material, or request content.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed [`StoreError`] if an audit row is malformed or `SQLite` cannot read
    /// the append-only event sequence.
    pub fn list_management_audit_events(&mut self) -> StoreResult<Vec<ManagementAuditEvent>> {
        let transaction = self.connection.transaction()?;
        let audit_events = load_management_audit_events(&transaction)?;
        transaction.commit()?;
        Ok(audit_events)
    }

    /// Returns resource mutation audit events in durable append order.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed [`StoreError`] if an audit row is malformed or `SQLite` cannot read
    /// the append-only event sequence.
    pub fn list_management_resource_audit_events(
        &mut self,
    ) -> StoreResult<Vec<ManagementResourceAuditEvent>> {
        let transaction = self.connection.transaction()?;
        let audit_events = load_management_resource_audit_events(&transaction)?;
        transaction.commit()?;
        Ok(audit_events)
    }

    /// Loads the durable predecessor recorded for the active Config Version, if one exists.
    ///
    /// This reconstructs the one-step rollback slot after a process restart from the latest
    /// successful publication or rollback audit event. An absent predecessor is a normal result
    /// for the first active Version.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed [`StoreError`] if the audit reference is malformed, absent from the
    /// configuration store, or no longer archived as a valid rollback predecessor.
    pub fn load_rollback_predecessor(
        &mut self,
        active_config_version_id: &ConfigVersionId,
    ) -> StoreResult<Option<ControlPlaneConfiguration>> {
        let transaction = self.connection.transaction()?;
        let predecessor_id =
            latest_rollback_predecessor_id(&transaction, active_config_version_id)?;
        transaction.commit()?;
        let Some(predecessor_id) = predecessor_id else {
            return Ok(None);
        };
        let predecessor = self
            .load_configuration(&predecessor_id)?
            .ok_or_else(|| malformed("management_audit_events"))?;
        if predecessor.version.status != ConfigVersionStatus::Archived {
            return Err(malformed("management_audit_events"));
        }
        Ok(Some(predecessor))
    }

    /// Atomically activates a draft or archived Config Version.
    ///
    /// The current active Version, if any, becomes archived in the same `SQLite` transaction. This
    /// is a state transition only: callers must compile and reserve a matching immutable runtime
    /// Snapshot before invoking it.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the Version is absent, already active, malformed in storage, or
    /// `SQLite` cannot commit the transition.
    pub fn activate_version(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> StoreResult<ConfigVersionActivation> {
        let mut transaction = self.begin_transaction()?;
        let activation = transaction.activate_version(config_version_id)?;
        transaction.commit()?;
        Ok(activation)
    }

    /// Atomically activates one Version and appends its matching publish or rollback audit event.
    ///
    /// The durable activation and audit append commit before a caller can make a matching
    /// `ArcSwap` publication visible. A failed event insert rolls back the status transition.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidManagementAuditEvent`] when a creation audit is supplied for
    /// an activation, and otherwise preserves the same failure behavior as [`Self::activate_version`].
    pub fn activate_version_with_audit(
        &mut self,
        config_version_id: &ConfigVersionId,
        audit_draft: &ManagementAuditEventDraft,
    ) -> StoreResult<(ConfigVersionActivation, ManagementAuditEvent)> {
        if !matches!(
            audit_draft.action(),
            ManagementAuditAction::Published | ManagementAuditAction::RolledBack
        ) {
            return Err(StoreError::InvalidManagementAuditEvent);
        }
        let mut transaction = self.begin_transaction()?;
        let activation = transaction.activate_version(config_version_id)?;
        let audit_event = transaction.record_management_audit_event(
            audit_draft,
            activation.activated_version_id().clone(),
            activation.replaced_active_version_id().cloned(),
        )?;
        transaction.commit()?;
        Ok((activation, audit_event))
    }

    /// Runs one draft-only graph mutation with an exact expected revision.
    ///
    /// The revision compare-and-increment and every callback write share one immediate `SQLite`
    /// transaction. A stale caller cannot leave a partial graph mutation behind.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::ConfigVersionRevisionConflict`] when the Version is still a draft
    /// but its stored revision differs from `expected_revision`; the rejected token is never
    /// included in the error.
    pub fn mutate_draft_configuration<T, F>(
        &mut self,
        config_version_id: &ConfigVersionId,
        expected_revision: i64,
        mutate: F,
    ) -> StoreResult<(T, i64)>
    where
        F: FnOnce(&mut ControlPlaneTransaction<'_>) -> StoreResult<T>,
    {
        let mut transaction = self.begin_transaction()?;
        let next_revision =
            transaction.require_and_bump_draft_revision(config_version_id, expected_revision)?;
        let result = mutate(&mut transaction)?;
        transaction.commit()?;
        Ok((result, next_revision))
    }

    /// Appends a resource audit event without changing a graph row.
    ///
    /// OAuth operation state is deliberately outside the immutable configuration graph, so its
    /// cancellation can be auditable without pretending it is a draft resource edit.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the Config Version does not exist, the event is invalid, or
    /// the append-only write cannot commit.
    pub fn record_management_resource_audit_event(
        &mut self,
        config_version_id: &ConfigVersionId,
        audit_draft: &ManagementResourceAuditEventDraft,
    ) -> StoreResult<()> {
        let mut transaction = self.begin_transaction()?;
        let exists: Option<i64> = transaction
            .transaction
            .query_row(
                "SELECT 1 FROM config_versions WHERE id = ?1",
                [config_version_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StoreError::ConfigVersionNotFound);
        }
        transaction.record_management_resource_audit_event(audit_draft, config_version_id)?;
        transaction.commit()
    }
}

impl fmt::Debug for SqliteControlPlaneRepository {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SqliteControlPlaneRepository(<connection redacted>)")
    }
}

/// A typed, all-or-nothing control-plane write transaction.
pub struct ControlPlaneTransaction<'connection> {
    transaction: Transaction<'connection>,
}

impl ControlPlaneTransaction<'_> {
    /// Inserts one immutable billing catalog inside this control-plane transaction.
    ///
    /// The management mutation boundary is create-only: any existing catalog identity fails
    /// closed, even though the lower billing Store retains exact-replay idempotence for crash
    /// recovery. This method exists so a catalog write, Config Version revision bump, and
    /// management audit event can commit atomically.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for malformed or conflicting catalog data.
    pub fn insert_billing_catalog(&mut self, catalog: &BillingPriceCatalog) -> StoreResult<()> {
        if load_catalog_from_connection(&self.transaction, &catalog.catalog_version_id)?.is_some() {
            return Err(StoreError::ConflictingBillingCatalogVersion);
        }
        insert_catalog_in_transaction(&self.transaction, catalog)
    }

    /// Verifies a draft graph's exact revision and advances it once for the current transaction.
    ///
    /// The method is public only so management-time services can compose one bounded resource
    /// mutation with this transaction. It does not expose the raw `SQLite` transaction.
    ///
    /// # Errors
    ///
    /// Returns a typed not-found, non-draft, or revision-conflict error without mutating rows.
    pub fn require_and_bump_draft_revision(
        &mut self,
        config_version_id: &ConfigVersionId,
        expected_revision: i64,
    ) -> StoreResult<i64> {
        if expected_revision < 0 {
            return Err(StoreError::ConfigVersionRevisionConflict);
        }

        let updated = self.transaction.execute(
            "UPDATE config_versions SET revision = revision + 1 \
             WHERE id = ?1 AND status = ?2 AND revision = ?3",
            params![
                config_version_id.as_str(),
                ConfigVersionStatus::Draft.as_sql(),
                expected_revision,
            ],
        )?;
        if updated == 1 {
            return Ok(expected_revision + 1);
        }

        let status: Option<String> = self
            .transaction
            .query_row(
                "SELECT status FROM config_versions WHERE id = ?1",
                [config_version_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        match status.as_deref() {
            None => Err(StoreError::ConfigVersionNotFound),
            Some("draft") => Err(StoreError::ConfigVersionRevisionConflict),
            Some("active" | "archived") => Err(StoreError::ControlPlaneMutationRequiresDraft),
            Some(_) => Err(malformed("config_versions")),
        }
    }
    /// Inserts every row from `configuration` in foreign-key order.
    ///
    /// The call does not publish or semantically validate the graph. A later error leaves the
    /// transaction uncommitted, and its `Drop` implementation rolls back all prior inserts.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the configuration violates an existing `SQLite` constraint.
    pub fn write_configuration(
        &mut self,
        configuration: &ControlPlaneConfiguration,
    ) -> StoreResult<()> {
        if configuration.version.status != ConfigVersionStatus::Draft {
            return Err(StoreError::ControlPlaneMutationRequiresDraft);
        }
        self.insert_config_version(&configuration.version)?;
        let config_version_id = &configuration.version.id;

        for egress_policy in &configuration.egress_policies {
            self.insert_egress_policy(config_version_id, egress_policy)?;
        }
        for upstream in &configuration.upstreams {
            self.insert_upstream(config_version_id, upstream)?;
        }
        for endpoint in &configuration.endpoints {
            self.insert_endpoint(config_version_id, endpoint)?;
        }
        for credential in &configuration.credentials {
            self.insert_credential(config_version_id, credential)?;
        }
        for binding in &configuration.endpoint_credential_bindings {
            self.insert_endpoint_credential_binding(config_version_id, binding)?;
        }
        for public_model in &configuration.public_models {
            self.insert_public_model(config_version_id, public_model)?;
        }
        for alias in &configuration.model_aliases {
            self.insert_model_alias(config_version_id, alias)?;
        }
        for route in &configuration.model_routes {
            self.insert_model_route(config_version_id, route)?;
        }
        for candidate in &configuration.route_candidates {
            self.insert_route_candidate(config_version_id, candidate)?;
        }
        for access_group in &configuration.access_groups {
            self.insert_access_group(config_version_id, access_group)?;
        }
        for access_group_route in &configuration.access_group_routes {
            self.insert_access_group_route(config_version_id, access_group_route)?;
        }
        for client_key in &configuration.client_keys {
            self.insert_client_key(config_version_id, client_key)?;
        }
        Ok(())
    }

    /// Inserts one opaque encrypted credential into an existing Config Version.
    ///
    /// The Store receives no plaintext credential bytes.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Config Version/Upstream relationship or another `SQLite`
    /// constraint rejects the row.
    pub fn insert_credential(
        &mut self,
        config_version_id: &ConfigVersionId,
        credential: &CredentialConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        self.transaction.execute(
            "INSERT INTO upstream_credentials (\
                config_version_id, id, upstream_id, kind, ciphertext, key_version, status, revision\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                config_version_id.as_str(),
                credential.id.as_str(),
                credential.upstream_id.as_str(),
                &credential.kind,
                credential.encrypted_secret.ciphertext(),
                credential.encrypted_secret.key_version().as_sqlite_i64(),
                credential.status.as_sql(),
                credential.revision,
            ],
        )?;
        Ok(())
    }

    /// Replaces one existing Egress Policy's non-identity fields.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not draft, the policy is absent, or the update
    /// cannot commit.
    pub fn update_egress_policy(
        &mut self,
        config_version_id: &ConfigVersionId,
        egress_policy: &EgressPolicyConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let updated = self.transaction.execute(
            "UPDATE egress_policies SET name = ?3, allowed_schemes_json = ?4, \
             allowed_hosts_json = ?5, allowed_ports_json = ?6, allowed_cidrs_json = ?7, \
             redirect_mode = ?8, max_redirects = ?9 \
             WHERE config_version_id = ?1 AND id = ?2",
            params![
                config_version_id.as_str(),
                egress_policy.id.as_str(),
                &egress_policy.name,
                &egress_policy.allowed_schemes_json,
                &egress_policy.allowed_hosts_json,
                &egress_policy.allowed_ports_json,
                &egress_policy.allowed_cidrs_json,
                egress_policy.redirect_mode.as_sql(),
                egress_policy.max_redirects,
            ],
        )?;
        resource_updated(updated)
    }

    /// Deletes one Egress Policy. The existing schema explicitly clears dependent Upstream
    /// policy references before the delete, preserving a structurally valid draft graph.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not draft, the policy is absent, or the delete
    /// cannot commit.
    pub fn delete_egress_policy(
        &mut self,
        config_version_id: &ConfigVersionId,
        egress_policy_id: &EgressPolicyId,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let deleted = self.transaction.execute(
            "DELETE FROM egress_policies WHERE config_version_id = ?1 AND id = ?2",
            params![config_version_id.as_str(), egress_policy_id.as_str()],
        )?;
        resource_updated(deleted)
    }

    /// Replaces one existing Upstream's non-identity fields.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not draft, the Upstream is absent, or the update
    /// cannot commit.
    pub fn update_upstream(
        &mut self,
        config_version_id: &ConfigVersionId,
        upstream: &UpstreamConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let updated = self.transaction.execute(
            "UPDATE upstreams SET name = ?3, kind = ?4, enabled = ?5, tags_json = ?6, \
             egress_policy_id = ?7 WHERE config_version_id = ?1 AND id = ?2",
            params![
                config_version_id.as_str(),
                upstream.id.as_str(),
                &upstream.name,
                &upstream.kind,
                boolean_to_sql(upstream.enabled),
                &upstream.tags_json,
                upstream
                    .egress_policy_id
                    .as_ref()
                    .map(EgressPolicyId::as_str),
            ],
        )?;
        resource_updated(updated)
    }

    /// Deletes one Upstream and the schema-owned Endpoint/Credential/Binding descendants.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not draft, the Upstream is absent, or the delete
    /// cannot commit.
    pub fn delete_upstream(
        &mut self,
        config_version_id: &ConfigVersionId,
        upstream_id: &UpstreamId,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let deleted = self.transaction.execute(
            "DELETE FROM upstreams WHERE config_version_id = ?1 AND id = ?2",
            params![config_version_id.as_str(), upstream_id.as_str()],
        )?;
        resource_updated(deleted)
    }

    /// Replaces one existing Endpoint's non-identity fields without moving it to another
    /// Upstream. The caller must preserve the stored owning Upstream identity.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not draft, the Endpoint is absent, or the update
    /// cannot commit.
    pub fn update_endpoint(
        &mut self,
        config_version_id: &ConfigVersionId,
        endpoint: &EndpointConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let updated = self.transaction.execute(
            "UPDATE upstream_endpoints SET adapter_id = ?3, api_format = ?4, base_url = ?5, \
             inference_path = ?6, models_path = ?7, transport = ?8, enabled = ?9 \
             WHERE config_version_id = ?1 AND id = ?2 AND upstream_id = ?10",
            params![
                config_version_id.as_str(),
                endpoint.id.as_str(),
                &endpoint.adapter_id,
                &endpoint.api_format,
                &endpoint.base_url,
                &endpoint.inference_path,
                &endpoint.models_path,
                endpoint.transport.as_sql(),
                boolean_to_sql(endpoint.enabled),
                endpoint.upstream_id.as_str(),
            ],
        )?;
        resource_updated(updated)
    }

    /// Deletes one Endpoint and schema-owned Bindings and Route Candidates.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not draft, the Endpoint is absent, or the delete
    /// cannot commit.
    pub fn delete_endpoint(
        &mut self,
        config_version_id: &ConfigVersionId,
        endpoint_id: &EndpointId,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let deleted = self.transaction.execute(
            "DELETE FROM upstream_endpoints WHERE config_version_id = ?1 AND id = ?2",
            params![config_version_id.as_str(), endpoint_id.as_str()],
        )?;
        resource_updated(deleted)
    }

    /// Replaces one existing opaque Credential without exposing plaintext to the Repository.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not draft, the Credential is absent, or the
    /// update cannot commit.
    pub fn update_credential(
        &mut self,
        config_version_id: &ConfigVersionId,
        credential: &CredentialConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let updated = self.transaction.execute(
            "UPDATE upstream_credentials SET kind = ?3, ciphertext = ?4, key_version = ?5, \
             status = ?6, revision = ?7 WHERE config_version_id = ?1 AND id = ?2 \
             AND upstream_id = ?8",
            params![
                config_version_id.as_str(),
                credential.id.as_str(),
                &credential.kind,
                credential.encrypted_secret.ciphertext(),
                credential.encrypted_secret.key_version().as_sqlite_i64(),
                credential.status.as_sql(),
                credential.revision,
                credential.upstream_id.as_str(),
            ],
        )?;
        resource_updated(updated)
    }

    /// Replaces one active Credential's encrypted secret as a dedicated runtime OAuth rotation.
    ///
    /// Ordinary graph mutations remain draft-only. OAuth login/refresh is different: it changes
    /// only the opaque credential material, not the Endpoint, Route, binding, or Client Key
    /// topology. This narrow operation therefore admits an `active` Version while still using an
    /// exact per-Credential revision compare-and-swap. The caller must publish any topology change
    /// through the normal draft lifecycle; this method cannot alter the Config Version revision or
    /// any graph row other than the selected Credential.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::ControlPlaneMutationRequiresDraft`] for a non-active Version,
    /// [`StoreError::ConfigVersionRevisionConflict`] when the Credential revision is stale, or
    /// [`StoreError::ControlPlaneResourceNotFound`] when the Credential is absent.
    pub fn rotate_active_credential(
        &mut self,
        config_version_id: &ConfigVersionId,
        credential: &CredentialConfiguration,
        expected_credential_revision: i64,
    ) -> StoreResult<()> {
        let status: Option<String> = self
            .transaction
            .query_row(
                "SELECT status FROM config_versions WHERE id = ?1",
                [config_version_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(status) = status else {
            return Err(StoreError::ConfigVersionNotFound);
        };
        if status != ConfigVersionStatus::Active.as_sql() {
            return Err(StoreError::ControlPlaneMutationRequiresDraft);
        }
        let Some(next_revision) = expected_credential_revision.checked_add(1) else {
            return Err(StoreError::ConfigVersionRevisionConflict);
        };
        if expected_credential_revision < 0 || credential.revision != next_revision {
            return Err(StoreError::ConfigVersionRevisionConflict);
        }
        let updated = self.transaction.execute(
            "UPDATE upstream_credentials SET kind = ?3, ciphertext = ?4, key_version = ?5, \
             status = ?6, revision = ?7 WHERE config_version_id = ?1 AND id = ?2 \
             AND upstream_id = ?8 AND revision = ?9",
            params![
                config_version_id.as_str(),
                credential.id.as_str(),
                &credential.kind,
                credential.encrypted_secret.ciphertext(),
                credential.encrypted_secret.key_version().as_sqlite_i64(),
                credential.status.as_sql(),
                credential.revision,
                credential.upstream_id.as_str(),
                expected_credential_revision,
            ],
        )?;
        if updated == 1 {
            Ok(())
        } else {
            Err(StoreError::ConfigVersionRevisionConflict)
        }
    }

    /// Deletes one Credential and its schema-owned Endpoint Bindings.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not draft, the Credential is absent, or the
    /// delete cannot commit.
    pub fn delete_credential(
        &mut self,
        config_version_id: &ConfigVersionId,
        credential_id: &CredentialId,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let deleted = self.transaction.execute(
            "DELETE FROM upstream_credentials WHERE config_version_id = ?1 AND id = ?2",
            params![config_version_id.as_str(), credential_id.as_str()],
        )?;
        resource_updated(deleted)
    }

    /// Inserts one storage-safe Client Key record into an existing Config Version.
    ///
    /// The Store receives no complete presented Client Key.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Access Group reference or a unique constraint rejects the
    /// row. Callers can rely on transaction rollback to remove prior writes.
    pub fn insert_client_key(
        &mut self,
        config_version_id: &ConfigVersionId,
        client_key: &StoredClientKey,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        self.transaction.execute(
            "INSERT INTO client_keys (\
                config_version_id, id, prefix, secret_digest, access_group_id, status, expires_at_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                config_version_id.as_str(),
                client_key.id.as_str(),
                client_key.prefix(),
                client_key.secret_digest().as_slice(),
                client_key.access_group_id.as_str(),
                client_key.status.as_sql(),
                client_key.expires_at_ms,
            ],
        )?;
        Ok(())
    }

    /// Atomically makes a draft or archived Config Version active and archives the prior active
    /// Version, if any.
    ///
    /// This is the only P2-07 status-transition operation. It intentionally does not write graph
    /// rows, decrypt Credentials, or construct a runtime Snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::ConfigVersionNotFound`] for an absent Version,
    /// [`StoreError::ConfigVersionAlreadyActive`] for a no-op request, or a fail-closed store error
    /// for malformed persistent state.
    pub fn activate_version(
        &mut self,
        config_version_id: &ConfigVersionId,
    ) -> StoreResult<ConfigVersionActivation> {
        let target_status: Option<String> = self
            .transaction
            .query_row(
                "SELECT status FROM config_versions WHERE id = ?1",
                [config_version_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        let target_status = target_status.ok_or(StoreError::ConfigVersionNotFound)?;
        match ConfigVersionStatus::from_sql(&target_status) {
            Some(ConfigVersionStatus::Draft | ConfigVersionStatus::Archived) => {}
            Some(ConfigVersionStatus::Active) => {
                return Err(StoreError::ConfigVersionAlreadyActive);
            }
            None => return Err(malformed("config_versions")),
        }

        let replaced_active_version_id: Option<String> = self
            .transaction
            .query_row(
                "SELECT id FROM config_versions WHERE status = ?1",
                [ConfigVersionStatus::Active.as_sql()],
                |row| row.get(0),
            )
            .optional()?;
        let replaced_active_version_id = replaced_active_version_id
            .map(ConfigVersionId::try_new)
            .transpose()
            .map_err(|_| malformed("config_versions"))?;

        self.transaction.execute(
            "UPDATE config_versions SET status = ?1 WHERE status = ?2",
            [
                ConfigVersionStatus::Archived.as_sql(),
                ConfigVersionStatus::Active.as_sql(),
            ],
        )?;
        let updated = self.transaction.execute(
            "UPDATE config_versions SET status = ?1 WHERE id = ?2",
            [
                ConfigVersionStatus::Active.as_sql(),
                config_version_id.as_str(),
            ],
        )?;
        if updated == 1 {
            Ok(ConfigVersionActivation::new(
                config_version_id.clone(),
                replaced_active_version_id,
            ))
        } else {
            Err(StoreError::ConfigVersionNotFound)
        }
    }

    fn record_management_audit_event(
        &mut self,
        audit_draft: &ManagementAuditEventDraft,
        config_version_id: ConfigVersionId,
        replaced_config_version_id: Option<ConfigVersionId>,
    ) -> StoreResult<ManagementAuditEvent> {
        self.transaction.execute(
            "INSERT INTO management_audit_events (\
                action, actor, occurred_at_ms, config_version_id, replaced_config_version_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                audit_draft.action().as_sql(),
                audit_draft.actor(),
                audit_draft.occurred_at_ms(),
                config_version_id.as_str(),
                replaced_config_version_id
                    .as_ref()
                    .map(ConfigVersionId::as_str),
            ],
        )?;
        ManagementAuditEvent::from_draft(
            self.transaction.last_insert_rowid(),
            audit_draft,
            config_version_id,
            replaced_config_version_id,
        )
    }

    /// Appends one non-secret resource-mutation audit record inside the caller's transaction.
    ///
    /// The row contains only the bounded action, protected request actor, Version, resource kind
    /// and opaque resource ID. It cannot carry a request body, credential plaintext, ciphertext,
    /// Header, or response value.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the bounded audit event cannot be appended to this transaction.
    pub fn record_management_resource_audit_event(
        &mut self,
        audit_draft: &ManagementResourceAuditEventDraft,
        config_version_id: &ConfigVersionId,
    ) -> StoreResult<()> {
        self.transaction.execute(
            "INSERT INTO management_resource_audit_events (\
                action, actor, occurred_at_ms, config_version_id, resource_kind, resource_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                audit_draft.action(),
                audit_draft.actor(),
                audit_draft.occurred_at_ms(),
                config_version_id.as_str(),
                audit_draft.resource_kind(),
                audit_draft.resource_id(),
            ],
        )?;
        Ok(())
    }

    /// Commits every prior write in this transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when `SQLite` cannot commit the transaction.
    pub fn commit(self) -> StoreResult<()> {
        self.transaction.commit()?;
        Ok(())
    }

    fn insert_config_version(&mut self, version: &ConfigVersion) -> StoreResult<()> {
        self.transaction.execute(
            "INSERT INTO config_versions (id, parent_id, status, revision, created_at_ms, description) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                version.id.as_str(),
                version.parent_id.as_ref().map(ConfigVersionId::as_str),
                version.status.as_sql(),
                version.revision,
                version.created_at_ms,
                &version.description,
            ],
        )?;
        Ok(())
    }

    fn ensure_draft_config_version(&self, config_version_id: &ConfigVersionId) -> StoreResult<()> {
        let status = self.transaction.query_row(
            "SELECT status FROM config_versions WHERE id = ?1",
            [config_version_id.as_str()],
            |row| row.get::<_, String>(0),
        );
        match status {
            Ok(status) if status == ConfigVersionStatus::Draft.as_sql() => Ok(()),
            Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(StoreError::ControlPlaneMutationRequiresDraft)
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Inserts one Upstream into a Version that was admitted for the current transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if an owning reference or a database constraint rejects the row.
    pub fn insert_upstream(
        &mut self,
        config_version_id: &ConfigVersionId,
        upstream: &UpstreamConfiguration,
    ) -> StoreResult<()> {
        self.transaction.execute(
            "INSERT INTO upstreams (\
                config_version_id, id, name, kind, enabled, tags_json, egress_policy_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                config_version_id.as_str(),
                upstream.id.as_str(),
                &upstream.name,
                &upstream.kind,
                boolean_to_sql(upstream.enabled),
                &upstream.tags_json,
                upstream
                    .egress_policy_id
                    .as_ref()
                    .map(EgressPolicyId::as_str),
            ],
        )?;
        Ok(())
    }

    /// Inserts one Egress Policy into a Version that was admitted for the current transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if a database constraint rejects the row.
    pub fn insert_egress_policy(
        &mut self,
        config_version_id: &ConfigVersionId,
        egress_policy: &EgressPolicyConfiguration,
    ) -> StoreResult<()> {
        self.transaction.execute(
            "INSERT INTO egress_policies (\
                config_version_id, id, name, allowed_schemes_json, allowed_hosts_json, \
                allowed_ports_json, allowed_cidrs_json, redirect_mode, max_redirects\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                config_version_id.as_str(),
                egress_policy.id.as_str(),
                &egress_policy.name,
                &egress_policy.allowed_schemes_json,
                &egress_policy.allowed_hosts_json,
                &egress_policy.allowed_ports_json,
                &egress_policy.allowed_cidrs_json,
                egress_policy.redirect_mode.as_sql(),
                egress_policy.max_redirects,
            ],
        )?;
        Ok(())
    }

    /// Inserts one Endpoint into a Version that was admitted for the current transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the owning Upstream reference or a database constraint rejects
    /// the row.
    pub fn insert_endpoint(
        &mut self,
        config_version_id: &ConfigVersionId,
        endpoint: &EndpointConfiguration,
    ) -> StoreResult<()> {
        self.transaction.execute(
            "INSERT INTO upstream_endpoints (\
                config_version_id, id, upstream_id, adapter_id, api_format, base_url, \
                inference_path, models_path, transport, enabled\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                config_version_id.as_str(),
                endpoint.id.as_str(),
                endpoint.upstream_id.as_str(),
                &endpoint.adapter_id,
                &endpoint.api_format,
                &endpoint.base_url,
                &endpoint.inference_path,
                &endpoint.models_path,
                endpoint.transport.as_sql(),
                boolean_to_sql(endpoint.enabled),
            ],
        )?;
        Ok(())
    }

    /// Inserts one Endpoint Credential binding into an admitted Version transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if an Endpoint/Credential/Upstream reference or a database
    /// constraint rejects the row.
    pub fn insert_endpoint_credential_binding(
        &mut self,
        config_version_id: &ConfigVersionId,
        binding: &EndpointCredentialBindingConfiguration,
    ) -> StoreResult<()> {
        self.transaction.execute(
            "INSERT INTO endpoint_credential_bindings (\
                config_version_id, endpoint_id, credential_id, upstream_id, enabled, priority, weight, concurrency\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                config_version_id.as_str(),
                binding.endpoint_id.as_str(),
                binding.credential_id.as_str(),
                binding.upstream_id.as_str(),
                boolean_to_sql(binding.enabled),
                binding.priority,
                binding.weight,
                binding.concurrency,
            ],
        )?;
        Ok(())
    }

    /// Inserts one Public Model into an existing draft configuration graph.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft or the database rejects the graph
    /// mutation.
    pub fn insert_public_model(
        &mut self,
        config_version_id: &ConfigVersionId,
        public_model: &PublicModelConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        self.transaction.execute(
            "INSERT INTO public_models (\
                config_version_id, id, model_name, status, display_name, capabilities_json\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                config_version_id.as_str(),
                public_model.id.as_str(),
                &public_model.model_name,
                public_model.status.as_sql(),
                &public_model.display_name,
                &public_model.capabilities_json,
            ],
        )?;
        Ok(())
    }

    /// Replaces one existing Public Model without changing its stable identity.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the Public Model is absent,
    /// or the database rejects the mutation.
    pub fn update_public_model(
        &mut self,
        config_version_id: &ConfigVersionId,
        public_model: &PublicModelConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let updated = self.transaction.execute(
            "UPDATE public_models SET model_name = ?3, status = ?4, display_name = ?5, \
             capabilities_json = ?6 WHERE config_version_id = ?1 AND id = ?2",
            params![
                config_version_id.as_str(),
                public_model.id.as_str(),
                &public_model.model_name,
                public_model.status.as_sql(),
                &public_model.display_name,
                &public_model.capabilities_json,
            ],
        )?;
        resource_updated(updated)
    }

    /// Deletes one Public Model and its schema-owned Alias/Route descendants.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the Public Model is absent,
    /// or the database rejects the deletion.
    pub fn delete_public_model(
        &mut self,
        config_version_id: &ConfigVersionId,
        public_model_id: &PublicModelId,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let deleted = self.transaction.execute(
            "DELETE FROM public_models WHERE config_version_id = ?1 AND id = ?2",
            params![config_version_id.as_str(), public_model_id.as_str()],
        )?;
        resource_updated(deleted)
    }

    /// Inserts one exact Alias-to-Public-Model relation into an existing draft graph.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the Public Model relation
    /// is invalid or duplicated, or the database rejects the mutation.
    pub fn insert_model_alias(
        &mut self,
        config_version_id: &ConfigVersionId,
        alias: &ModelAliasConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        self.transaction.execute(
            "INSERT INTO model_aliases (config_version_id, alias, public_model_id) \
             VALUES (?1, ?2, ?3)",
            params![
                config_version_id.as_str(),
                &alias.alias,
                alias.public_model_id.as_str(),
            ],
        )?;
        Ok(())
    }

    /// Inserts one Route under an existing Public Model into an existing draft graph.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the owning Public Model is
    /// absent, the Route conflicts, or the database rejects the mutation.
    pub fn insert_model_route(
        &mut self,
        config_version_id: &ConfigVersionId,
        route: &ModelRouteConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        self.transaction.execute(
            "INSERT INTO model_routes (\
                config_version_id, id, public_model_id, policy, max_attempts, bootstrap_timeout_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                config_version_id.as_str(),
                route.id.as_str(),
                route.public_model_id.as_str(),
                route.policy.as_sql(),
                route.max_attempts,
                route.bootstrap_timeout_ms,
            ],
        )?;
        Ok(())
    }

    /// Replaces one Route while preserving its owning Public Model identity.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the Route/owner does not
    /// exist, or the database rejects the mutation.
    pub fn update_model_route(
        &mut self,
        config_version_id: &ConfigVersionId,
        route: &ModelRouteConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let updated = self.transaction.execute(
            "UPDATE model_routes SET policy = ?3, max_attempts = ?4, bootstrap_timeout_ms = ?5 \
             WHERE config_version_id = ?1 AND id = ?2 AND public_model_id = ?6",
            params![
                config_version_id.as_str(),
                route.id.as_str(),
                route.policy.as_sql(),
                route.max_attempts,
                route.bootstrap_timeout_ms,
                route.public_model_id.as_str(),
            ],
        )?;
        resource_updated(updated)
    }

    /// Deletes one Route and its schema-owned Candidate and Access Group grant descendants.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the Route is absent, or
    /// the database rejects the deletion.
    pub fn delete_model_route(
        &mut self,
        config_version_id: &ConfigVersionId,
        route_id: &RouteId,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let deleted = self.transaction.execute(
            "DELETE FROM model_routes WHERE config_version_id = ?1 AND id = ?2",
            params![config_version_id.as_str(), route_id.as_str()],
        )?;
        resource_updated(deleted)
    }

    /// Inserts one Candidate under an existing Route into an existing draft graph.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, Route/Endpoint references
    /// are invalid, the Candidate conflicts, or the database rejects the mutation.
    pub fn insert_route_candidate(
        &mut self,
        config_version_id: &ConfigVersionId,
        candidate: &RouteCandidateConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        self.transaction.execute(
            "INSERT INTO route_candidates (\
                config_version_id, id, route_id, endpoint_id, upstream_model, credential_scope, \
                transform_mode, enabled, priority, weight, capability_override_json\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                config_version_id.as_str(),
                candidate.id.as_str(),
                candidate.route_id.as_str(),
                candidate.endpoint_id.as_str(),
                &candidate.upstream_model,
                candidate.credential_scope.as_sql(),
                candidate.transform_mode.as_sql(),
                boolean_to_sql(candidate.enabled),
                candidate.priority,
                candidate.weight,
                &candidate.capability_override_json,
            ],
        )?;
        Ok(())
    }

    /// Inserts one Access Group into an existing draft configuration graph.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft or the database rejects the graph
    /// mutation.
    pub fn insert_access_group(
        &mut self,
        config_version_id: &ConfigVersionId,
        access_group: &AccessGroupConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        self.transaction.execute(
            "INSERT INTO access_groups (config_version_id, id, name, status, limits_json) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                config_version_id.as_str(),
                access_group.id.as_str(),
                &access_group.name,
                access_group.status.as_sql(),
                &access_group.limits_json,
            ],
        )?;
        Ok(())
    }

    /// Replaces one Access Group without changing its stable identity.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the Access Group is absent,
    /// or the database rejects the mutation.
    pub fn update_access_group(
        &mut self,
        config_version_id: &ConfigVersionId,
        access_group: &AccessGroupConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let updated = self.transaction.execute(
            "UPDATE access_groups SET name = ?3, status = ?4, limits_json = ?5 \
             WHERE config_version_id = ?1 AND id = ?2",
            params![
                config_version_id.as_str(),
                access_group.id.as_str(),
                &access_group.name,
                access_group.status.as_sql(),
                &access_group.limits_json,
            ],
        )?;
        resource_updated(updated)
    }

    /// Deletes one Access Group and its schema-owned grants and Client Keys.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the Access Group is absent,
    /// or the database rejects the deletion.
    pub fn delete_access_group(
        &mut self,
        config_version_id: &ConfigVersionId,
        access_group_id: &AccessGroupId,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let deleted = self.transaction.execute(
            "DELETE FROM access_groups WHERE config_version_id = ?1 AND id = ?2",
            params![config_version_id.as_str(), access_group_id.as_str()],
        )?;
        resource_updated(deleted)
    }

    /// Inserts one Access Group-to-Route permission relation into an existing draft graph.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the Access Group/Route
    /// references are invalid or duplicated, or the database rejects the mutation.
    pub fn insert_access_group_route(
        &mut self,
        config_version_id: &ConfigVersionId,
        access_group_route: &AccessGroupRouteConfiguration,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        self.transaction.execute(
            "INSERT INTO access_group_routes (config_version_id, access_group_id, route_id, enabled) \
             VALUES (?1, ?2, ?3, ?4)",
            params![
                config_version_id.as_str(),
                access_group_route.access_group_id.as_str(),
                access_group_route.route_id.as_str(),
                boolean_to_sql(access_group_route.enabled),
            ],
        )?;
        Ok(())
    }

    /// Replaces non-secret Client Key lifecycle metadata without changing its Prefix or digest.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the Client Key or Access
    /// Group is absent, or the database rejects the mutation.
    pub fn update_client_key_metadata(
        &mut self,
        config_version_id: &ConfigVersionId,
        client_key_id: &ClientKeyId,
        access_group_id: &AccessGroupId,
        status: StoredClientKeyStatus,
        expires_at_ms: Option<i64>,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let updated = self.transaction.execute(
            "UPDATE client_keys SET access_group_id = ?3, status = ?4, expires_at_ms = ?5 \
             WHERE config_version_id = ?1 AND id = ?2",
            params![
                config_version_id.as_str(),
                client_key_id.as_str(),
                access_group_id.as_str(),
                status.as_sql(),
                expires_at_ms,
            ],
        )?;
        resource_updated(updated)
    }

    /// Permanently revokes one Client Key while retaining its Prefix and digest record.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the Version is not a writable draft, the Client Key is absent,
    /// or the database rejects the mutation.
    pub fn revoke_client_key(
        &mut self,
        config_version_id: &ConfigVersionId,
        client_key_id: &ClientKeyId,
    ) -> StoreResult<()> {
        self.ensure_draft_config_version(config_version_id)?;
        let updated = self.transaction.execute(
            "UPDATE client_keys SET status = 'revoked' WHERE config_version_id = ?1 AND id = ?2",
            params![config_version_id.as_str(), client_key_id.as_str()],
        )?;
        resource_updated(updated)
    }
}

fn boolean_to_sql(value: bool) -> i64 {
    i64::from(value)
}

fn resource_updated(affected_rows: usize) -> StoreResult<()> {
    if affected_rows == 1 {
        Ok(())
    } else {
        Err(StoreError::ControlPlaneResourceNotFound)
    }
}

fn bounded_non_empty(value: &str, maximum_characters: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= maximum_characters
}

fn load_configuration(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Option<ControlPlaneConfiguration>> {
    let Some(version) = load_config_version(transaction, config_version_id)? else {
        return Ok(None);
    };

    Ok(Some(ControlPlaneConfiguration {
        version,
        egress_policies: load_egress_policies(transaction, config_version_id)?,
        upstreams: load_upstreams(transaction, config_version_id)?,
        endpoints: load_endpoints(transaction, config_version_id)?,
        credentials: load_credentials(transaction, config_version_id)?,
        endpoint_credential_bindings: load_endpoint_credential_bindings(
            transaction,
            config_version_id,
        )?,
        public_models: load_public_models(transaction, config_version_id)?,
        model_aliases: load_model_aliases(transaction, config_version_id)?,
        model_routes: load_model_routes(transaction, config_version_id)?,
        route_candidates: load_route_candidates(transaction, config_version_id)?,
        access_groups: load_access_groups(transaction, config_version_id)?,
        access_group_routes: load_access_group_routes(transaction, config_version_id)?,
        client_keys: load_client_keys(transaction, config_version_id)?,
    }))
}

fn load_management_audit_events(
    transaction: &Transaction<'_>,
) -> StoreResult<Vec<ManagementAuditEvent>> {
    let mut statement = transaction.prepare(
        "SELECT id, action, actor, occurred_at_ms, config_version_id, replaced_config_version_id \
         FROM management_audit_events ORDER BY id",
    )?;
    let mut rows = statement.query([])?;
    let mut audit_events = Vec::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        if id <= 0 {
            return Err(malformed("management_audit_events"));
        }
        let action_value: String = row.get(1)?;
        let action = ManagementAuditAction::from_sql(&action_value)
            .ok_or_else(|| malformed("management_audit_events"))?;
        let actor: String = row.get(2)?;
        let occurred_at_ms: i64 = row.get(3)?;
        let draft = ManagementAuditEventDraft::try_new(action, actor, occurred_at_ms)
            .map_err(|_| malformed("management_audit_events"))?;
        let config_version_id =
            read_identifier(row, 4, ConfigVersionId::try_new, "management_audit_events")?;
        let replaced_config_version_id =
            read_optional_identifier(row, 5, ConfigVersionId::try_new, "management_audit_events")?;
        audit_events.push(ManagementAuditEvent::from_draft(
            id,
            &draft,
            config_version_id,
            replaced_config_version_id,
        )?);
    }
    Ok(audit_events)
}

fn load_management_resource_audit_events(
    transaction: &Transaction<'_>,
) -> StoreResult<Vec<ManagementResourceAuditEvent>> {
    let mut statement = transaction.prepare(
        "SELECT id, action, actor, occurred_at_ms, config_version_id, resource_kind, resource_id \
         FROM management_resource_audit_events ORDER BY id",
    )?;
    let mut rows = statement.query([])?;
    let mut audit_events = Vec::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let action: String = row.get(1)?;
        let actor: String = row.get(2)?;
        let occurred_at_ms: i64 = row.get(3)?;
        let config_version_id = read_identifier(
            row,
            4,
            ConfigVersionId::try_new,
            "management_resource_audit_events",
        )?;
        let resource_kind: String = row.get(5)?;
        let resource_id: String = row.get(6)?;
        if id <= 0
            || occurred_at_ms < 0
            || !bounded_non_empty(&action, 64)
            || !bounded_non_empty(&actor, 128)
            || !bounded_non_empty(&resource_kind, 64)
            || !bounded_non_empty(&resource_id, 128)
        {
            return Err(malformed("management_resource_audit_events"));
        }
        audit_events.push(ManagementResourceAuditEvent {
            id,
            action,
            actor,
            occurred_at_ms,
            config_version_id,
            resource_kind,
            resource_id,
        });
    }
    Ok(audit_events)
}

fn latest_rollback_predecessor_id(
    transaction: &Transaction<'_>,
    active_config_version_id: &ConfigVersionId,
) -> StoreResult<Option<ConfigVersionId>> {
    let predecessor_id: Option<Option<String>> = transaction
        .query_row(
            "SELECT replaced_config_version_id FROM management_audit_events \
             WHERE config_version_id = ?1 \
               AND action IN ('config_published', 'config_rolled_back') \
             ORDER BY id DESC LIMIT 1",
            [active_config_version_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    predecessor_id
        .flatten()
        .map(ConfigVersionId::try_new)
        .transpose()
        .map_err(|_| malformed("management_audit_events"))
}

fn load_config_version(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Option<ConfigVersion>> {
    let mut statement = transaction.prepare(
        "SELECT id, parent_id, status, revision, created_at_ms, description \
         FROM config_versions WHERE id = ?1",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    Ok(Some(config_version_from_row(row)?))
}

fn load_config_versions(transaction: &Transaction<'_>) -> StoreResult<Vec<ConfigVersion>> {
    let mut statement = transaction.prepare(
        "SELECT id, parent_id, status, revision, created_at_ms, description \
         FROM config_versions ORDER BY id",
    )?;
    let mut rows = statement.query([])?;
    let mut versions = Vec::new();
    while let Some(row) = rows.next()? {
        versions.push(config_version_from_row(row)?);
    }
    Ok(versions)
}

fn config_version_from_row(row: &rusqlite::Row<'_>) -> StoreResult<ConfigVersion> {
    let id = read_identifier(row, 0, ConfigVersionId::try_new, "config_versions")?;
    let parent_id = read_optional_identifier(row, 1, ConfigVersionId::try_new, "config_versions")?;
    let status_value: String = row.get(2)?;
    let status =
        ConfigVersionStatus::from_sql(&status_value).ok_or_else(|| malformed("config_versions"))?;
    let revision: i64 = row.get(3)?;
    if revision < 0 {
        return Err(malformed("config_versions"));
    }
    let created_at_ms: i64 = row.get(4)?;
    if created_at_ms < 0 {
        return Err(malformed("config_versions"));
    }
    let description: String = row.get(5)?;
    Ok(ConfigVersion {
        id,
        parent_id,
        status,
        revision,
        created_at_ms,
        description,
    })
}

fn load_egress_policies(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<EgressPolicyConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT id, name, allowed_schemes_json, allowed_hosts_json, allowed_ports_json, \
         allowed_cidrs_json, redirect_mode, max_redirects \
         FROM egress_policies WHERE config_version_id = ?1 ORDER BY id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut egress_policies = Vec::new();
    while let Some(row) = rows.next()? {
        let redirect_mode_value: String = row.get(6)?;
        let redirect_mode = StoredEgressRedirectMode::from_sql(&redirect_mode_value)
            .ok_or_else(|| malformed("egress_policies"))?;
        let max_redirects: i64 = row.get(7)?;
        if max_redirects < 0 {
            return Err(malformed("egress_policies"));
        }
        egress_policies.push(EgressPolicyConfiguration {
            id: read_identifier(row, 0, EgressPolicyId::try_new, "egress_policies")?,
            name: row.get(1)?,
            allowed_schemes_json: row.get(2)?,
            allowed_hosts_json: row.get(3)?,
            allowed_ports_json: row.get(4)?,
            allowed_cidrs_json: row.get(5)?,
            redirect_mode,
            max_redirects,
        });
    }
    Ok(egress_policies)
}

fn load_upstreams(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<UpstreamConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT id, name, kind, enabled, tags_json, egress_policy_id \
         FROM upstreams WHERE config_version_id = ?1 ORDER BY id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut upstreams = Vec::new();
    while let Some(row) = rows.next()? {
        upstreams.push(UpstreamConfiguration {
            id: read_identifier(row, 0, UpstreamId::try_new, "upstreams")?,
            name: row.get(1)?,
            kind: row.get(2)?,
            enabled: read_boolean(row, 3, "upstreams")?,
            tags_json: row.get(4)?,
            egress_policy_id: read_optional_identifier(
                row,
                5,
                EgressPolicyId::try_new,
                "upstreams",
            )?,
        });
    }
    Ok(upstreams)
}

fn load_endpoints(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<EndpointConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT id, upstream_id, adapter_id, api_format, base_url, inference_path, models_path, transport, enabled \
         FROM upstream_endpoints WHERE config_version_id = ?1 ORDER BY id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut endpoints = Vec::new();
    while let Some(row) = rows.next()? {
        endpoints.push(EndpointConfiguration {
            id: read_identifier(row, 0, EndpointId::try_new, "upstream_endpoints")?,
            upstream_id: read_identifier(row, 1, UpstreamId::try_new, "upstream_endpoints")?,
            adapter_id: row.get(2)?,
            api_format: row.get(3)?,
            base_url: row.get(4)?,
            inference_path: row.get(5)?,
            models_path: row.get(6)?,
            transport: EndpointTransport::from_sql(&row.get::<_, String>(7)?)
                .ok_or_else(|| malformed("upstream_endpoints"))?,
            enabled: read_boolean(row, 8, "upstream_endpoints")?,
        });
    }
    Ok(endpoints)
}

fn load_credentials(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<CredentialConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT id, upstream_id, kind, ciphertext, key_version, status, revision \
         FROM upstream_credentials WHERE config_version_id = ?1 ORDER BY id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut credentials = Vec::new();
    while let Some(row) = rows.next()? {
        let key_version = KeyVersion::try_from_sqlite_i64(row.get(4)?)
            .map_err(|_| malformed("upstream_credentials"))?;
        let ciphertext: Vec<u8> = row.get(3)?;
        let encrypted_secret = EncryptedSecret::try_from_persisted(key_version, ciphertext)
            .map_err(|_| malformed("upstream_credentials"))?;
        let status_value: String = row.get(5)?;
        let status = CredentialStatus::from_sql(&status_value)
            .ok_or_else(|| malformed("upstream_credentials"))?;
        let revision: i64 = row.get(6)?;
        if revision < 0 {
            return Err(malformed("upstream_credentials"));
        }
        credentials.push(CredentialConfiguration {
            id: read_identifier(row, 0, CredentialId::try_new, "upstream_credentials")?,
            upstream_id: read_identifier(row, 1, UpstreamId::try_new, "upstream_credentials")?,
            kind: row.get(2)?,
            encrypted_secret,
            status,
            revision,
        });
    }
    Ok(credentials)
}

fn load_endpoint_credential_bindings(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<EndpointCredentialBindingConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT endpoint_id, credential_id, upstream_id, enabled, priority, weight, concurrency \
         FROM endpoint_credential_bindings WHERE config_version_id = ?1 ORDER BY endpoint_id, credential_id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut bindings = Vec::new();
    while let Some(row) = rows.next()? {
        let priority: i64 = row.get(4)?;
        let weight: i64 = row.get(5)?;
        let concurrency: i64 = row.get(6)?;
        if priority < 0 || weight <= 0 || concurrency <= 0 {
            return Err(malformed("endpoint_credential_bindings"));
        }
        bindings.push(EndpointCredentialBindingConfiguration {
            endpoint_id: read_identifier(
                row,
                0,
                EndpointId::try_new,
                "endpoint_credential_bindings",
            )?,
            credential_id: read_identifier(
                row,
                1,
                CredentialId::try_new,
                "endpoint_credential_bindings",
            )?,
            upstream_id: read_identifier(
                row,
                2,
                UpstreamId::try_new,
                "endpoint_credential_bindings",
            )?,
            enabled: read_boolean(row, 3, "endpoint_credential_bindings")?,
            priority,
            weight,
            concurrency,
        });
    }
    Ok(bindings)
}

fn load_public_models(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<PublicModelConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT id, model_name, status, display_name, capabilities_json \
         FROM public_models WHERE config_version_id = ?1 ORDER BY id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut public_models = Vec::new();
    while let Some(row) = rows.next()? {
        let status = AdministrativeStatus::from_sql(&row.get::<_, String>(2)?)
            .ok_or_else(|| malformed("public_models"))?;
        public_models.push(PublicModelConfiguration {
            id: read_identifier(row, 0, PublicModelId::try_new, "public_models")?,
            model_name: row.get(1)?,
            status,
            display_name: row.get(3)?,
            capabilities_json: row.get(4)?,
        });
    }
    Ok(public_models)
}

fn load_model_aliases(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<ModelAliasConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT alias, public_model_id FROM model_aliases \
         WHERE config_version_id = ?1 ORDER BY alias",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut aliases = Vec::new();
    while let Some(row) = rows.next()? {
        aliases.push(ModelAliasConfiguration {
            alias: row.get(0)?,
            public_model_id: read_identifier(row, 1, PublicModelId::try_new, "model_aliases")?,
        });
    }
    Ok(aliases)
}

fn load_model_routes(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<ModelRouteConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT id, public_model_id, policy, max_attempts, bootstrap_timeout_ms \
         FROM model_routes WHERE config_version_id = ?1 ORDER BY id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut routes = Vec::new();
    while let Some(row) = rows.next()? {
        let policy_value: String = row.get(2)?;
        let policy =
            RoutePolicy::from_sql(&policy_value).ok_or_else(|| malformed("model_routes"))?;
        let max_attempts: i64 = row.get(3)?;
        let bootstrap_timeout_ms: i64 = row.get(4)?;
        if max_attempts <= 0 || bootstrap_timeout_ms <= 0 {
            return Err(malformed("model_routes"));
        }
        routes.push(ModelRouteConfiguration {
            id: read_identifier(row, 0, RouteId::try_new, "model_routes")?,
            public_model_id: read_identifier(row, 1, PublicModelId::try_new, "model_routes")?,
            policy,
            max_attempts,
            bootstrap_timeout_ms,
        });
    }
    Ok(routes)
}

fn load_route_candidates(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<RouteCandidateConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT id, route_id, endpoint_id, upstream_model, credential_scope, transform_mode, \
                enabled, priority, weight, capability_override_json \
         FROM route_candidates WHERE config_version_id = ?1 ORDER BY id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut candidates = Vec::new();
    while let Some(row) = rows.next()? {
        let credential_scope_value: String = row.get(4)?;
        let credential_scope = CredentialScope::from_sql(&credential_scope_value)
            .ok_or_else(|| malformed("route_candidates"))?;
        let transform_mode_value: String = row.get(5)?;
        let transform_mode = TransformMode::from_sql(&transform_mode_value)
            .ok_or_else(|| malformed("route_candidates"))?;
        let priority: i64 = row.get(7)?;
        let weight: i64 = row.get(8)?;
        if priority < 0 || weight <= 0 {
            return Err(malformed("route_candidates"));
        }
        candidates.push(RouteCandidateConfiguration {
            id: read_identifier(row, 0, RouteCandidateId::try_new, "route_candidates")?,
            route_id: read_identifier(row, 1, RouteId::try_new, "route_candidates")?,
            endpoint_id: read_identifier(row, 2, EndpointId::try_new, "route_candidates")?,
            upstream_model: row.get(3)?,
            credential_scope,
            transform_mode,
            enabled: read_boolean(row, 6, "route_candidates")?,
            priority,
            weight,
            capability_override_json: row.get(9)?,
        });
    }
    Ok(candidates)
}

fn load_access_groups(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<AccessGroupConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT id, name, status, limits_json FROM access_groups \
         WHERE config_version_id = ?1 ORDER BY id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut access_groups = Vec::new();
    while let Some(row) = rows.next()? {
        let status = AdministrativeStatus::from_sql(&row.get::<_, String>(2)?)
            .ok_or_else(|| malformed("access_groups"))?;
        access_groups.push(AccessGroupConfiguration {
            id: read_identifier(row, 0, AccessGroupId::try_new, "access_groups")?,
            name: row.get(1)?,
            status,
            limits_json: row.get(3)?,
        });
    }
    Ok(access_groups)
}

fn load_access_group_routes(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<AccessGroupRouteConfiguration>> {
    let mut statement = transaction.prepare(
        "SELECT access_group_id, route_id, enabled FROM access_group_routes \
         WHERE config_version_id = ?1 ORDER BY access_group_id, route_id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut access_group_routes = Vec::new();
    while let Some(row) = rows.next()? {
        access_group_routes.push(AccessGroupRouteConfiguration {
            access_group_id: read_identifier(
                row,
                0,
                AccessGroupId::try_new,
                "access_group_routes",
            )?,
            route_id: read_identifier(row, 1, RouteId::try_new, "access_group_routes")?,
            enabled: read_boolean(row, 2, "access_group_routes")?,
        });
    }
    Ok(access_group_routes)
}

fn load_client_keys(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Vec<StoredClientKey>> {
    let mut statement = transaction.prepare(
        "SELECT id, access_group_id, prefix, secret_digest, status, expires_at_ms \
         FROM client_keys WHERE config_version_id = ?1 ORDER BY id",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let mut client_keys = Vec::new();
    while let Some(row) = rows.next()? {
        let status_value: String = row.get(4)?;
        let status = StoredClientKeyStatus::from_sql(&status_value)
            .ok_or_else(|| malformed("client_keys"))?;
        let digest: Vec<u8> = row.get(3)?;
        client_keys.push(StoredClientKey::try_new(
            read_identifier(row, 0, ClientKeyId::try_new, "client_keys")?,
            read_identifier(row, 1, AccessGroupId::try_new, "client_keys")?,
            row.get::<_, String>(2)?,
            digest,
            status,
            row.get(5)?,
        )?);
    }
    Ok(client_keys)
}

fn read_identifier<T>(
    row: &rusqlite::Row<'_>,
    index: usize,
    create: impl FnOnce(String) -> Result<T, InvalidIdentifier>,
    table: &'static str,
) -> StoreResult<T> {
    let value: String = row.get(index)?;
    create(value).map_err(|_| malformed(table))
}

fn read_optional_identifier<T>(
    row: &rusqlite::Row<'_>,
    index: usize,
    create: impl FnOnce(String) -> Result<T, InvalidIdentifier> + Copy,
    table: &'static str,
) -> StoreResult<Option<T>> {
    let value: Option<String> = row.get(index)?;
    value.map(create).transpose().map_err(|_| malformed(table))
}

fn read_boolean(row: &rusqlite::Row<'_>, index: usize, table: &'static str) -> StoreResult<bool> {
    match row.get::<_, i64>(index)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(malformed(table)),
    }
}

fn malformed(table: &'static str) -> StoreError {
    StoreError::InvalidPersistedControlPlaneRecord { table }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{AccessGroupId, ClientKeyId, CredentialId, UpstreamId};
    use rusqlite::params;

    use crate::{
        StoreError,
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };

    use super::{
        AccessGroupConfiguration, AdministrativeStatus, ConfigVersion, ConfigVersionId,
        ConfigVersionStatus, ControlPlaneConfiguration, CredentialConfiguration, CredentialStatus,
        ManagementAuditAction, ManagementAuditEventDraft, SqliteControlPlaneRepository,
        StoredClientKey, StoredClientKeyStatus, UpstreamConfiguration,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn malformed_persisted_crypto_records_fail_closed() -> TestResult {
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        let version_id = ConfigVersionId::try_new("v1")?;
        let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
            id: version_id.clone(),
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 1,
            description: "test graph".to_owned(),
        });
        configuration.upstreams.push(UpstreamConfiguration {
            id: UpstreamId::try_new("upstream-a")?,
            name: "station-a".to_owned(),
            kind: "relay".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: None,
        });
        configuration.access_groups.push(AccessGroupConfiguration {
            id: AccessGroupId::try_new("access-group-a")?,
            name: "default".to_owned(),
            status: AdministrativeStatus::Active,
            limits_json: "{}".to_owned(),
        });
        repository.write_configuration(&configuration)?;

        repository.connection.execute(
            "INSERT INTO upstream_credentials (\
                config_version_id, id, upstream_id, kind, ciphertext, key_version, status, revision\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                version_id.as_str(),
                "credential-malformed",
                "upstream-a",
                "api_key",
                &[1_u8],
                1_i64,
                "active",
                0_i64,
            ],
        )?;

        let result = repository.load_configuration(&version_id);
        assert!(matches!(
            result,
            Err(StoreError::InvalidPersistedControlPlaneRecord {
                table: "upstream_credentials"
            })
        ));
        let metadata = repository
            .load_config_version(&version_id)?
            .ok_or("safe Config Version metadata was not found")?;
        assert_eq!(metadata.id, version_id);
        assert_eq!(metadata.status, ConfigVersionStatus::Draft);
        assert_eq!(metadata.description, "test graph");
        assert_eq!(repository.list_config_versions()?.len(), 1);
        Ok(())
    }

    #[test]
    fn client_key_debug_redacts_the_digest() -> TestResult {
        let client_key = StoredClientKey::try_new(
            ClientKeyId::try_new("client-key-a")?,
            AccessGroupId::try_new("access-group-a")?,
            "rgw_0123456789abcdef",
            [0xA5_u8; 32],
            StoredClientKeyStatus::Active,
            None,
        )?;
        let debug = format!("{client_key:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("165"));
        Ok(())
    }

    #[test]
    fn encrypted_credentials_remain_opaque_when_written() -> TestResult {
        let key_version = KeyVersion::try_new(1)?;
        let secret_store = SecretStore::new(MasterKeyRing::try_new(
            key_version,
            [(key_version, MasterKey::try_from_bytes([7_u8; 32])?)],
        )?);
        let encrypted_secret = secret_store.seal(b"test-credential", b"test-aad")?;
        let version_id = ConfigVersionId::try_new("v1")?;
        let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
            id: version_id.clone(),
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 1,
            description: "test graph".to_owned(),
        });
        configuration.upstreams.push(UpstreamConfiguration {
            id: UpstreamId::try_new("upstream-a")?,
            name: "station-a".to_owned(),
            kind: "relay".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: None,
        });
        configuration.credentials.push(CredentialConfiguration {
            id: CredentialId::try_new("credential-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            kind: "api_key".to_owned(),
            encrypted_secret,
            status: CredentialStatus::Active,
            revision: 0,
        });

        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        repository.write_configuration(&configuration)?;
        let loaded = repository
            .load_configuration(&version_id)?
            .ok_or("configuration was not found")?;
        let debug = format!("{loaded:?}");
        assert!(!debug.contains("test-credential"));
        assert!(debug.contains("<redacted>"));
        Ok(())
    }

    #[test]
    fn non_draft_graphs_cannot_be_written_before_snapshot_publication_exists() -> TestResult {
        let version_id = ConfigVersionId::try_new("active-version")?;
        let configuration = ControlPlaneConfiguration::new(ConfigVersion {
            id: version_id.clone(),
            parent_id: None,
            status: ConfigVersionStatus::Active,
            revision: 0,
            created_at_ms: 1,
            description: "must not publish from P2-05".to_owned(),
        });
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;

        assert!(matches!(
            repository.write_configuration(&configuration),
            Err(StoreError::ControlPlaneMutationRequiresDraft)
        ));
        assert!(repository.load_configuration(&version_id)?.is_none());
        Ok(())
    }

    #[test]
    fn activation_archives_the_prior_active_version_in_one_transition() -> TestResult {
        let version_one = ConfigVersionId::try_new("version-one")?;
        let version_two = ConfigVersionId::try_new("version-two")?;
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        repository.write_configuration(&draft_configuration(version_one.clone(), None))?;
        repository.write_configuration(&draft_configuration(
            version_two.clone(),
            Some(version_one.clone()),
        ))?;

        let first = repository.activate_version(&version_one)?;
        assert_eq!(first.activated_version_id(), &version_one);
        assert!(first.replaced_active_version_id().is_none());
        assert_eq!(
            repository
                .load_configuration(&version_one)?
                .ok_or("version one is missing")?
                .version
                .status,
            ConfigVersionStatus::Active
        );

        let second = repository.activate_version(&version_two)?;
        assert_eq!(second.activated_version_id(), &version_two);
        assert_eq!(second.replaced_active_version_id(), Some(&version_one));
        assert_eq!(
            repository
                .load_configuration(&version_one)?
                .ok_or("version one is missing")?
                .version
                .status,
            ConfigVersionStatus::Archived
        );
        assert_eq!(
            repository
                .load_configuration(&version_two)?
                .ok_or("version two is missing")?
                .version
                .status,
            ConfigVersionStatus::Active
        );
        assert!(matches!(
            repository.activate_version(&version_two),
            Err(StoreError::ConfigVersionAlreadyActive)
        ));
        assert!(matches!(
            repository.activate_version(&ConfigVersionId::try_new("missing-version")?),
            Err(StoreError::ConfigVersionNotFound)
        ));
        Ok(())
    }

    #[test]
    fn credential_and_client_key_mutations_require_an_existing_draft() -> TestResult {
        let version_id = ConfigVersionId::try_new("active-version")?;
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        repository.connection.execute(
            "INSERT INTO config_versions (id, parent_id, status, created_at_ms, description) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                version_id.as_str(),
                Option::<&str>::None,
                "active",
                1_i64,
                "future publisher fixture",
            ],
        )?;
        repository.connection.execute(
            "INSERT INTO upstreams (\
                config_version_id, id, name, kind, enabled, tags_json, egress_policy_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                version_id.as_str(),
                "upstream-a",
                "station-a",
                "relay",
                1_i64,
                "[]",
                Option::<&str>::None,
            ],
        )?;
        repository.connection.execute(
            "INSERT INTO access_groups (config_version_id, id, name, status, limits_json) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                version_id.as_str(),
                "access-group-a",
                "default",
                "active",
                "{}",
            ],
        )?;

        let key_version = KeyVersion::try_new(1)?;
        let secret_store = SecretStore::new(MasterKeyRing::try_new(
            key_version,
            [(key_version, MasterKey::try_from_bytes([9_u8; 32])?)],
        )?);
        let credential = CredentialConfiguration {
            id: CredentialId::try_new("credential-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            kind: "api_key".to_owned(),
            encrypted_secret: secret_store.seal(b"non-persisted", b"fixture-aad")?,
            status: CredentialStatus::Active,
            revision: 0,
        };
        let client_key = StoredClientKey::try_new(
            ClientKeyId::try_new("client-key-a")?,
            AccessGroupId::try_new("access-group-a")?,
            "rgw_0123456789abcdef",
            [0xA5_u8; 32],
            StoredClientKeyStatus::Active,
            None,
        )?;

        let mut transaction = repository.begin_transaction()?;
        assert!(matches!(
            transaction.insert_credential(&version_id, &credential),
            Err(StoreError::ControlPlaneMutationRequiresDraft)
        ));
        assert!(matches!(
            transaction.insert_client_key(&version_id, &client_key),
            Err(StoreError::ControlPlaneMutationRequiresDraft)
        ));
        drop(transaction);

        let loaded = repository
            .load_configuration(&version_id)?
            .ok_or("active configuration was not found")?;
        assert!(loaded.credentials.is_empty());
        assert!(loaded.client_keys.is_empty());
        Ok(())
    }

    #[test]
    fn active_oauth_rotation_is_cas_guarded_and_keeps_graph_revision() -> TestResult {
        let version_id = ConfigVersionId::try_new("active-oauth")?;
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        repository.connection.execute(
            "INSERT INTO config_versions (id, parent_id, status, revision, created_at_ms, description) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                version_id.as_str(),
                Option::<&str>::None,
                "active",
                7_i64,
                1_i64,
                "active OAuth fixture",
            ],
        )?;
        repository.connection.execute(
            "INSERT INTO upstreams (config_version_id, id, name, kind, enabled, tags_json, egress_policy_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                version_id.as_str(),
                "upstream-a",
                "station-a",
                "relay",
                1_i64,
                "[]",
                Option::<&str>::None,
            ],
        )?;
        let key_version = KeyVersion::try_new(1)?;
        let secret_store = SecretStore::new(MasterKeyRing::try_new(
            key_version,
            [(key_version, MasterKey::try_from_bytes([8_u8; 32])?)],
        )?);
        let original = secret_store.seal(b"old-oauth", b"old-aad")?;
        repository.connection.execute(
            "INSERT INTO upstream_credentials (config_version_id, id, upstream_id, kind, ciphertext, key_version, status, revision) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                version_id.as_str(),
                "credential-a",
                "upstream-a",
                "oauth_json",
                original.ciphertext(),
                original.key_version().as_sqlite_i64(),
                "active",
                3_i64,
            ],
        )?;
        let replacement = CredentialConfiguration {
            id: CredentialId::try_new("credential-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            kind: "oauth_json".to_owned(),
            encrypted_secret: secret_store.seal(b"new-oauth", b"new-aad")?,
            status: CredentialStatus::Active,
            revision: 4,
        };
        let audit = super::ManagementResourceAuditEventDraft::try_new(
            "credential_oauth_rotated",
            "test-operator",
            10,
            "credential",
            "credential-a",
        )?;
        let mut transaction = repository.begin_transaction()?;
        transaction.rotate_active_credential(&version_id, &replacement, 3)?;
        transaction.record_management_resource_audit_event(&audit, &version_id)?;
        transaction.commit()?;

        let loaded = repository
            .load_configuration(&version_id)?
            .ok_or("active OAuth configuration was not found")?;
        assert_eq!(loaded.version.status, ConfigVersionStatus::Active);
        assert_eq!(loaded.version.revision, 7);
        assert_eq!(loaded.credentials[0].revision, 4);
        assert!(matches!(
            repository
                .begin_transaction()?
                .rotate_active_credential(&version_id, &replacement, 3),
            Err(StoreError::ConfigVersionRevisionConflict)
        ));
        Ok(())
    }

    #[test]
    fn audit_events_are_durable_and_reconstruct_the_rollback_predecessor() -> TestResult {
        let version_one = ConfigVersionId::try_new("version-one")?;
        let version_two = ConfigVersionId::try_new("version-two")?;
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;

        let created_one = ManagementAuditEventDraft::try_new(
            ManagementAuditAction::Created,
            "test-operator",
            10,
        )?;
        let first_event = repository.write_configuration_with_audit(
            &draft_configuration(version_one.clone(), None),
            &created_one,
        )?;
        assert_eq!(first_event.action(), ManagementAuditAction::Created);
        assert_eq!(first_event.config_version_id(), &version_one);
        assert!(first_event.replaced_config_version_id().is_none());

        let created_two = ManagementAuditEventDraft::try_new(
            ManagementAuditAction::Created,
            "test-operator",
            20,
        )?;
        repository.write_configuration_with_audit(
            &draft_configuration(version_two.clone(), Some(version_one.clone())),
            &created_two,
        )?;

        let published_one = ManagementAuditEventDraft::try_new(
            ManagementAuditAction::Published,
            "test-operator",
            30,
        )?;
        let (_, first_publication_event) =
            repository.activate_version_with_audit(&version_one, &published_one)?;
        assert!(
            first_publication_event
                .replaced_config_version_id()
                .is_none()
        );

        let published_two = ManagementAuditEventDraft::try_new(
            ManagementAuditAction::Published,
            "test-operator",
            40,
        )?;
        let (_, second_publication_event) =
            repository.activate_version_with_audit(&version_two, &published_two)?;
        assert_eq!(
            second_publication_event.replaced_config_version_id(),
            Some(&version_one)
        );

        let active = repository
            .load_active_configuration()?
            .ok_or("expected active configuration")?;
        assert_eq!(&active.version.id, &version_two);
        let predecessor = repository
            .load_rollback_predecessor(&active.version.id)?
            .ok_or("expected rollback predecessor")?;
        assert_eq!(predecessor.version.id, version_one);
        assert_eq!(predecessor.version.status, ConfigVersionStatus::Archived);

        let events = repository.list_management_audit_events()?;
        assert_eq!(events.len(), 4);
        assert!(events.windows(2).all(|pair| pair[0].id() < pair[1].id()));

        let update = repository.connection.execute(
            "UPDATE management_audit_events SET actor = ?1 WHERE id = ?2",
            params!["different-actor", first_event.id()],
        );
        assert!(update.is_err());
        let delete = repository.connection.execute(
            "DELETE FROM management_audit_events WHERE id = ?1",
            [first_event.id()],
        );
        assert!(delete.is_err());
        assert_eq!(repository.list_management_audit_events()?.len(), 4);
        Ok(())
    }

    fn draft_configuration(
        id: ConfigVersionId,
        parent_id: Option<ConfigVersionId>,
    ) -> ControlPlaneConfiguration {
        ControlPlaneConfiguration::new(ConfigVersion {
            id,
            parent_id,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 1,
            description: "P2-07 activation fixture".to_owned(),
        })
    }
}
