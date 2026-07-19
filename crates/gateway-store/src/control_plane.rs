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
    StoreError, StoreResult, migrate,
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
}

impl TransformMode {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Passthrough => "passthrough",
            Self::Canonical => "canonical",
            Self::LosslessBridge => "lossless_bridge",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "passthrough" => Some(Self::Passthrough),
            "canonical" => Some(Self::Canonical),
            "lossless_bridge" => Some(Self::LosslessBridge),
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
            "INSERT INTO config_versions (id, parent_id, status, created_at_ms, description) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                version.id.as_str(),
                version.parent_id.as_ref().map(ConfigVersionId::as_str),
                version.status.as_sql(),
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

    fn insert_upstream(
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

    fn insert_egress_policy(
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

    fn insert_endpoint(
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

    fn insert_endpoint_credential_binding(
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

    fn insert_public_model(
        &mut self,
        config_version_id: &ConfigVersionId,
        public_model: &PublicModelConfiguration,
    ) -> StoreResult<()> {
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

    fn insert_model_alias(
        &mut self,
        config_version_id: &ConfigVersionId,
        alias: &ModelAliasConfiguration,
    ) -> StoreResult<()> {
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

    fn insert_model_route(
        &mut self,
        config_version_id: &ConfigVersionId,
        route: &ModelRouteConfiguration,
    ) -> StoreResult<()> {
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

    fn insert_route_candidate(
        &mut self,
        config_version_id: &ConfigVersionId,
        candidate: &RouteCandidateConfiguration,
    ) -> StoreResult<()> {
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

    fn insert_access_group(
        &mut self,
        config_version_id: &ConfigVersionId,
        access_group: &AccessGroupConfiguration,
    ) -> StoreResult<()> {
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

    fn insert_access_group_route(
        &mut self,
        config_version_id: &ConfigVersionId,
        access_group_route: &AccessGroupRouteConfiguration,
    ) -> StoreResult<()> {
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
}

fn boolean_to_sql(value: bool) -> i64 {
    i64::from(value)
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

fn load_config_version(
    transaction: &Transaction<'_>,
    config_version_id: &ConfigVersionId,
) -> StoreResult<Option<ConfigVersion>> {
    let mut statement = transaction.prepare(
        "SELECT id, parent_id, status, created_at_ms, description \
         FROM config_versions WHERE id = ?1",
    )?;
    let mut rows = statement.query([config_version_id.as_str()])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let id = read_identifier(row, 0, ConfigVersionId::try_new, "config_versions")?;
    let parent_id = read_optional_identifier(row, 1, ConfigVersionId::try_new, "config_versions")?;
    let status_value: String = row.get(2)?;
    let status =
        ConfigVersionStatus::from_sql(&status_value).ok_or_else(|| malformed("config_versions"))?;
    let created_at_ms: i64 = row.get(3)?;
    if created_at_ms < 0 {
        return Err(malformed("config_versions"));
    }
    let description: String = row.get(4)?;
    Ok(Some(ConfigVersion {
        id,
        parent_id,
        status,
        created_at_ms,
        description,
    }))
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
        SqliteControlPlaneRepository, StoredClientKey, StoredClientKeyStatus,
        UpstreamConfiguration,
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

    fn draft_configuration(
        id: ConfigVersionId,
        parent_id: Option<ConfigVersionId>,
    ) -> ControlPlaneConfiguration {
        ControlPlaneConfiguration::new(ConfigVersion {
            id,
            parent_id,
            status: ConfigVersionStatus::Draft,
            created_at_ms: 1,
            description: "P2-07 activation fixture".to_owned(),
        })
    }
}
