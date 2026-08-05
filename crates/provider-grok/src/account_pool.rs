//! Native, encrypted Grok Build/Web/Console account-pool persistence.
//!
//! This boundary deliberately stores an opaque account identifier and a provider-scoped identity
//! digest instead of source identity text. Import is bounded and transactional: credentials are
//! authenticated-encrypted immediately, duplicates are idempotent only when both metadata and
//! plaintext match, and every newly created account remains attributable to a reversible batch.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    path::Path,
    sync::{Arc, Mutex},
};

use gateway_core::{CredentialId, EndpointId};
use gateway_router::{
    QuotaConfidence, QuotaSnapshot, QuotaSource, QuotaWindow, RuntimeHealthError,
    RuntimeHealthRegistry, RuntimeQuotaRegistry, RuntimeQuotaTarget,
};
use gateway_store::{
    migrate,
    secret_store::{EncryptedSecret, KeyVersion, PlaintextSecret, SecretStore},
};
use gateway_upstream::{
    CredentialPoolBuildError, CredentialSecret, EndpointCredentialInput, EndpointCredentialPool,
    EndpointCredentialPools,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

const ACCOUNT_SECRET_AAD_DOMAIN: &[u8] = b"cpa-rust-gateway/grok/account-pool/v1";
const MAX_BATCH_ITEMS: usize = 4_096;
const MAX_IDENTITY_BYTES: usize = 1_024;
const MAX_CREDENTIAL_BYTES: usize = 512 * 1_024;
const MAX_OPAQUE_ID_BYTES: usize = 128;
const MAX_RELATION_BYTES: usize = 64;

/// One independently scheduled native Grok provider namespace.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GrokAccountProvider {
    /// Grok Build OAuth accounts.
    Build,
    /// Grok Web browser-session accounts.
    Web,
    /// Grok Console SSO accounts.
    Console,
}

impl GrokAccountProvider {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Web => "web",
            Self::Console => "console",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, GrokAccountPoolError> {
        match value {
            "build" => Ok(Self::Build),
            "web" => Ok(Self::Web),
            "console" => Ok(Self::Console),
            _ => Err(GrokAccountPoolError::InvalidPersistedState),
        }
    }

    const fn credential_kind(self) -> &'static str {
        match self {
            Self::Build => "grok_build_oauth",
            Self::Web => "grok_web_sso",
            Self::Console => "grok_console_sso",
        }
    }
}

/// Authentication eligibility kept separate from quota and cooldown state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GrokAccountAuthStatus {
    /// Credential may be leased when its other eligibility gates pass.
    Active,
    /// Credential requires an explicit authentication repair.
    ReauthRequired,
    /// Operator-disabled credential.
    Disabled,
}

impl GrokAccountAuthStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::ReauthRequired => "reauth_required",
            Self::Disabled => "disabled",
        }
    }

    fn parse(value: &str) -> Result<Self, GrokAccountPoolError> {
        match value {
            "active" => Ok(Self::Active),
            "reauth_required" => Ok(Self::ReauthRequired),
            "disabled" => Ok(Self::Disabled),
            _ => Err(GrokAccountPoolError::InvalidPersistedState),
        }
    }
}

/// Source identity retained only in memory until its provider-scoped digest is derived.
pub struct GrokAccountIdentity(Vec<u8>);

impl GrokAccountIdentity {
    /// Copies a bounded, non-empty identity for immediate import.
    ///
    /// # Errors
    ///
    /// Returns [`GrokAccountPoolError::InvalidIdentity`] when the input is empty or oversized.
    pub fn try_from_bytes(value: impl AsRef<[u8]>) -> Result<Self, GrokAccountPoolError> {
        let value = value.as_ref();
        if value.is_empty() || value.len() > MAX_IDENTITY_BYTES {
            return Err(GrokAccountPoolError::InvalidIdentity);
        }
        Ok(Self(value.to_vec()))
    }
}

impl fmt::Debug for GrokAccountIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokAccountIdentity")
            .field("value", &"<redacted>")
            .field("length", &self.0.len())
            .finish()
    }
}

impl Drop for GrokAccountIdentity {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Provider credential plaintext retained only for the duration of an import operation.
pub struct GrokAccountCredential(Vec<u8>);

impl GrokAccountCredential {
    /// Copies a bounded, non-empty provider credential for immediate authenticated encryption.
    ///
    /// # Errors
    ///
    /// Returns [`GrokAccountPoolError::InvalidCredential`] for empty or oversized material.
    pub fn try_from_bytes(value: impl AsRef<[u8]>) -> Result<Self, GrokAccountPoolError> {
        let value = value.as_ref();
        if value.is_empty() || value.len() > MAX_CREDENTIAL_BYTES {
            return Err(GrokAccountPoolError::InvalidCredential);
        }
        Ok(Self(value.to_vec()))
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for GrokAccountCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokAccountCredential")
            .field("value", &"<redacted>")
            .field("length", &self.0.len())
            .finish()
    }
}

impl Drop for GrokAccountCredential {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// One strict account entry in an encrypted batch import.
#[derive(Debug)]
pub struct GrokAccountImport {
    /// Independent provider namespace.
    pub provider: GrokAccountProvider,
    /// Source identity, discarded after digest derivation.
    pub identity: GrokAccountIdentity,
    /// Complete provider credential payload, encrypted before persistence.
    pub credential: GrokAccountCredential,
    /// Imported authentication eligibility.
    pub auth_status: GrokAccountAuthStatus,
    /// Operator eligibility switch.
    pub enabled: bool,
    /// Higher values are preferred by the later scheduler composition.
    pub priority: i64,
    /// Relative scheduler weight.
    pub weight: u32,
    /// Maximum simultaneous leases for this account.
    pub max_concurrency: u32,
    /// Optional proactive-refresh deadline.
    pub refresh_due_at_ms: Option<i64>,
    /// Optional proactive quota synchronization deadline.
    pub quota_sync_due_at_ms: Option<i64>,
    /// Optional account-local cooldown deadline.
    pub cooldown_until_ms: Option<i64>,
}

/// One relationship between two entries in the same atomic import request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokAccountImportRelation {
    /// Zero-based source entry index.
    pub source_entry: usize,
    /// Zero-based target entry index.
    pub target_entry: usize,
    /// Bounded metadata-only relationship label.
    pub relation: String,
}

impl GrokAccountImport {
    fn validate(&self) -> Result<(), GrokAccountPoolError> {
        if !(-1_000..=1_000).contains(&self.priority)
            || !(1..=10_000).contains(&self.weight)
            || !(1..=10_000).contains(&self.max_concurrency)
            || self.refresh_due_at_ms.is_some_and(|value| value < 0)
            || self.quota_sync_due_at_ms.is_some_and(|value| value < 0)
            || self.cooldown_until_ms.is_some_and(|value| value < 0)
        {
            return Err(GrokAccountPoolError::InvalidSchedulingMetadata);
        }
        Ok(())
    }
}

/// Redacted account metadata safe for control-plane listing and review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokAccountMetadata {
    /// Random, non-secret CPAR account identifier.
    pub id: String,
    /// Independent provider namespace.
    pub provider: GrokAccountProvider,
    /// Authentication eligibility, without credential contents.
    pub auth_status: GrokAccountAuthStatus,
    /// Operator eligibility switch.
    pub enabled: bool,
    /// Scheduler priority.
    pub priority: i64,
    /// Scheduler weight.
    pub weight: u32,
    /// Maximum simultaneous leases.
    pub max_concurrency: u32,
    /// Proactive refresh deadline, when known.
    pub refresh_due_at_ms: Option<i64>,
    /// Proactive quota synchronization deadline, when known.
    pub quota_sync_due_at_ms: Option<i64>,
    /// Account-local cooldown deadline, when active.
    pub cooldown_until_ms: Option<i64>,
    /// Monotonic credential revision.
    pub revision: u64,
    /// Reversible import provenance.
    pub import_batch_id: String,
}

/// One provider-to-Endpoint binding used only while compiling a native runtime pool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokAccountEndpointBinding {
    provider: GrokAccountProvider,
    endpoint_id: EndpointId,
}

impl GrokAccountEndpointBinding {
    /// Binds one isolated Grok provider namespace to one compiler-approved Endpoint.
    #[must_use]
    pub const fn new(provider: GrokAccountProvider, endpoint_id: EndpointId) -> Self {
        Self {
            provider,
            endpoint_id,
        }
    }

    /// Returns the native provider namespace.
    #[must_use]
    pub const fn provider(&self) -> GrokAccountProvider {
        self.provider
    }

    /// Returns the Endpoint receiving this provider's native account pool.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }
}

/// Immutable native account compilation consumed by the existing route scheduler.
pub struct GrokNativeAccountPoolCompilation {
    credential_pools: Arc<EndpointCredentialPools>,
    providers_by_credential: BTreeMap<CredentialId, GrokAccountProvider>,
    health_bootstrap: Vec<GrokRuntimeHealthBootstrap>,
    quota_bootstrap: Vec<QuotaSnapshot>,
}

impl GrokNativeAccountPoolCompilation {
    /// Returns the exact standard pool set accepted by [`gateway_router::RouteCredentialScheduler`].
    #[must_use]
    pub fn credential_pools(&self) -> Arc<EndpointCredentialPools> {
        Arc::clone(&self.credential_pools)
    }

    /// Returns the number of native accounts compiled into runtime pools.
    #[must_use]
    pub fn account_count(&self) -> usize {
        self.providers_by_credential.len()
    }

    /// Returns the isolated provider namespace for one compiled Credential identity.
    #[must_use]
    pub fn provider_for_credential(
        &self,
        credential_id: &CredentialId,
    ) -> Option<GrokAccountProvider> {
        self.providers_by_credential.get(credential_id).copied()
    }

    /// Seeds persisted reauthentication and future cooldown state into the shared Health registry.
    ///
    /// This is a control-path restart operation. It does not read `SQLite`, inspect Secrets, create
    /// another scheduler, or mutate Quota state. A reauthentication block dominates a cooldown.
    ///
    /// # Errors
    ///
    /// Returns the exact safe [`RuntimeHealthError`] if the shared registry cannot retain state.
    pub fn seed_runtime_health(
        &self,
        runtime_health: &RuntimeHealthRegistry,
    ) -> Result<(), RuntimeHealthError> {
        for bootstrap in &self.health_bootstrap {
            match bootstrap {
                GrokRuntimeHealthBootstrap::Unauthorized {
                    endpoint_id,
                    credential_id,
                } => runtime_health
                    .mark_credential_unauthorized(endpoint_id.clone(), credential_id.clone())?,
                GrokRuntimeHealthBootstrap::Cooldown {
                    endpoint_id,
                    credential_id,
                    until_ms,
                } => {
                    let result = runtime_health.cool_down_until(
                        gateway_router::RuntimeHealthKey::endpoint_credential(
                            endpoint_id.clone(),
                            credential_id.clone(),
                        ),
                        *until_ms,
                    );
                    if !matches!(result, Err(RuntimeHealthError::DeadlineNotInFuture)) {
                        result?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Restores durable account/model quota snapshots into the shared runtime Quota registry.
    ///
    /// # Errors
    ///
    /// Returns a safe runtime Quota error when an exact target cannot be retained.
    pub fn seed_runtime_quota(
        &self,
        runtime_quota: &RuntimeQuotaRegistry,
    ) -> Result<(), gateway_router::RuntimeQuotaError> {
        for snapshot in &self.quota_bootstrap {
            runtime_quota.record_snapshot(snapshot.clone())?;
        }
        Ok(())
    }
}

impl fmt::Debug for GrokNativeAccountPoolCompilation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokNativeAccountPoolCompilation")
            .field("credential_pools", &self.credential_pools)
            .field("account_count", &self.providers_by_credential.len())
            .field("health_bootstrap_count", &self.health_bootstrap.len())
            .field("quota_bootstrap_count", &self.quota_bootstrap.len())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum GrokRuntimeHealthBootstrap {
    Unauthorized {
        endpoint_id: EndpointId,
        credential_id: CredentialId,
    },
    Cooldown {
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        until_ms: i64,
    },
}

/// Safe native account-to-runtime compilation failures.
#[derive(Debug)]
pub enum GrokNativeAccountCompileError {
    /// Provider or Endpoint bindings are duplicated.
    DuplicateBinding,
    /// Observation time is outside the supported timestamp domain.
    InvalidObservationTime,
    /// Persisted account metadata or encrypted credential is invalid.
    Account(GrokAccountPoolError),
    /// Existing bounded Credential pool validation rejected the compiled inputs.
    Pool(CredentialPoolBuildError),
}

impl fmt::Display for GrokNativeAccountCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::DuplicateBinding => "native Grok runtime binding is duplicated",
            Self::InvalidObservationTime => "native Grok runtime observation time is invalid",
            Self::Account(_) => "native Grok runtime account state is invalid",
            Self::Pool(_) => "native Grok runtime Credential pool is invalid",
        };
        formatter.write_str(message)
    }
}

impl Error for GrokNativeAccountCompileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Account(error) => Some(error),
            Self::Pool(error) => Some(error),
            Self::DuplicateBinding | Self::InvalidObservationTime => None,
        }
    }
}

impl From<GrokAccountPoolError> for GrokNativeAccountCompileError {
    fn from(error: GrokAccountPoolError) -> Self {
        Self::Account(error)
    }
}

impl From<CredentialPoolBuildError> for GrokNativeAccountCompileError {
    fn from(error: CredentialPoolBuildError) -> Self {
        Self::Pool(error)
    }
}

/// Result of one committed native account import batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrokAccountImportOutcome {
    /// Accounts created by this batch.
    pub created: usize,
    /// Existing exact accounts left unchanged.
    pub unchanged: usize,
}

/// Result of rolling back exactly one import batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrokAccountRollbackOutcome {
    /// Accounts removed; their explicit links cascade with them.
    pub removed: usize,
    /// Whether the batch had already been rolled back.
    pub already_rolled_back: bool,
}

/// Safe failure classes for native Grok account-pool operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokAccountPoolError {
    /// Database, transaction, lock, or migration operation failed.
    StoreUnavailable,
    /// Source identity is empty or outside its fixed bound.
    InvalidIdentity,
    /// Credential is empty or outside its fixed bound.
    InvalidCredential,
    /// Batch ID, account ID, relation, item count, or timestamp is invalid.
    InvalidRequest,
    /// Priority, weight, concurrency, or refresh deadline is invalid.
    InvalidSchedulingMetadata,
    /// The batch ID already exists and cannot ambiguously be reused.
    BatchAlreadyExists,
    /// The same provider identity appeared more than once in one batch.
    DuplicateIdentity,
    /// An existing identity has different credential or scheduling metadata.
    ExistingAccountConflict,
    /// Requested batch or account does not exist.
    NotFound,
    /// Authenticated encryption or decryption failed.
    SecretStoreFailure,
    /// Persisted metadata or envelope is malformed.
    InvalidPersistedState,
}

impl fmt::Display for GrokAccountPoolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::StoreUnavailable => "native Grok account store is unavailable",
            Self::InvalidIdentity => "native Grok account identity is invalid",
            Self::InvalidCredential => "native Grok account credential is invalid",
            Self::InvalidRequest => "native Grok account request is invalid",
            Self::InvalidSchedulingMetadata => "native Grok account scheduling metadata is invalid",
            Self::BatchAlreadyExists => "native Grok account import batch already exists",
            Self::DuplicateIdentity => "native Grok account import contains a duplicate identity",
            Self::ExistingAccountConflict => "native Grok account conflicts with existing state",
            Self::NotFound => "native Grok account resource was not found",
            Self::SecretStoreFailure => "native Grok account encryption failed",
            Self::InvalidPersistedState => "native Grok account persisted state is invalid",
        };
        formatter.write_str(message)
    }
}

impl Error for GrokAccountPoolError {}

/// SQLite-backed native Grok account aggregate.
pub struct GrokAccountPoolStore {
    pub(crate) connection: Mutex<Connection>,
    pub(crate) secret_store: SecretStore,
}

impl GrokAccountPoolStore {
    /// Opens one direct `SQLite` path and applies the current versioned schema.
    ///
    /// Filesystem ownership and symbolic-link policy remain the caller's control-plane
    /// responsibility. Keeping `rusqlite` inside this Provider boundary prevents deployment
    /// binaries from acquiring a second persistence implementation dependency.
    ///
    /// # Errors
    ///
    /// Returns [`GrokAccountPoolError::StoreUnavailable`] when the database cannot be opened or
    /// migrated.
    pub fn try_open(
        database: impl AsRef<Path>,
        secret_store: SecretStore,
    ) -> Result<Self, GrokAccountPoolError> {
        let connection =
            Connection::open(database).map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        Self::try_new(connection, secret_store)
    }

    /// Creates the store and applies the current versioned schema before accepting accounts.
    ///
    /// # Errors
    ///
    /// Returns [`GrokAccountPoolError::StoreUnavailable`] when migration or database setup fails.
    pub fn try_new(
        mut connection: Connection,
        secret_store: SecretStore,
    ) -> Result<Self, GrokAccountPoolError> {
        migrate(&mut connection).map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        Ok(Self {
            connection: Mutex::new(connection),
            secret_store,
        })
    }

    /// Atomically imports one bounded batch without persisting source identity plaintext.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, conflict, encryption, or storage classification. The transaction
    /// is rolled back on every error.
    pub fn import_batch(
        &self,
        batch_id: &str,
        entries: &[GrokAccountImport],
        observed_at_ms: i64,
    ) -> Result<GrokAccountImportOutcome, GrokAccountPoolError> {
        self.import_batch_with_relations(batch_id, entries, &[], observed_at_ms)
    }

    /// Atomically imports accounts and their metadata-only relationships.
    ///
    /// Relationship indices address `entries`, allowing generated or existing CPAR account IDs to
    /// remain private. All validation, encryption, account insertion and link insertion share one
    /// transaction; any failure leaves neither accounts, links nor an applied batch row.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, conflict, encryption, or storage classification.
    #[allow(clippy::too_many_lines)] // One SQLite transaction keeps accounts, links and batch audit atomic.
    pub fn import_batch_with_relations(
        &self,
        batch_id: &str,
        entries: &[GrokAccountImport],
        relations: &[GrokAccountImportRelation],
        observed_at_ms: i64,
    ) -> Result<GrokAccountImportOutcome, GrokAccountPoolError> {
        if !valid_component(batch_id, MAX_OPAQUE_ID_BYTES)
            || entries.is_empty()
            || entries.len() > MAX_BATCH_ITEMS
            || relations.len() > MAX_BATCH_ITEMS
            || observed_at_ms < 0
        {
            return Err(GrokAccountPoolError::InvalidRequest);
        }

        let mut identities = BTreeSet::new();
        for entry in entries {
            entry.validate()?;
            let digest = identity_digest(entry.provider, &entry.identity.0);
            if !identities.insert((entry.provider, digest)) {
                return Err(GrokAccountPoolError::DuplicateIdentity);
            }
        }
        let mut unique_relations = BTreeSet::new();
        for relation in relations {
            if relation.source_entry >= entries.len()
                || relation.target_entry >= entries.len()
                || relation.source_entry == relation.target_entry
                || !valid_component(&relation.relation, MAX_RELATION_BYTES)
                || !unique_relations.insert((
                    relation.source_entry,
                    relation.target_entry,
                    relation.relation.as_str(),
                ))
            {
                return Err(GrokAccountPoolError::InvalidRequest);
            }
        }

        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let batch_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM grok_account_import_batches WHERE id = ?1)",
                [batch_id],
                |row| row.get(0),
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        if batch_exists {
            return Err(GrokAccountPoolError::BatchAlreadyExists);
        }

        transaction
            .execute(
                "INSERT INTO grok_account_import_batches (\
                    id, status, created_count, unchanged_count, created_at_ms, rolled_back_at_ms\
                 ) VALUES (?1, 'applied', 0, 0, ?2, NULL)",
                params![batch_id, observed_at_ms],
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;

        let mut created = 0_usize;
        let mut unchanged = 0_usize;
        let mut account_ids = Vec::with_capacity(entries.len());
        for entry in entries {
            let account_id = match self.import_one(&transaction, batch_id, entry, observed_at_ms)? {
                ImportOneOutcome::Created(account_id) => {
                    created += 1;
                    account_id
                }
                ImportOneOutcome::Unchanged(account_id) => {
                    unchanged += 1;
                    account_id
                }
            };
            if entry.auth_status == GrokAccountAuthStatus::ReauthRequired {
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO grok_account_reauth_state (account_id) \
                         VALUES (?1)",
                        [&account_id],
                    )
                    .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
            }
            account_ids.push(account_id);
        }

        for relation in relations {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO grok_account_links (\
                        source_account_id, target_account_id, relation, created_at_ms\
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        account_ids[relation.source_entry],
                        account_ids[relation.target_entry],
                        relation.relation,
                        observed_at_ms,
                    ],
                )
                .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        }

        transaction
            .execute(
                "UPDATE grok_account_import_batches \
                 SET created_count = ?2, unchanged_count = ?3 WHERE id = ?1",
                params![
                    batch_id,
                    i64::try_from(created).map_err(|_| GrokAccountPoolError::InvalidRequest)?,
                    i64::try_from(unchanged).map_err(|_| GrokAccountPoolError::InvalidRequest)?,
                ],
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        transaction
            .commit()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        Ok(GrokAccountImportOutcome { created, unchanged })
    }

    /// Rolls back only accounts created by the named batch and retains its audit row.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, missing-resource, persisted-state, or storage classification.
    pub fn rollback_import_batch(
        &self,
        batch_id: &str,
        observed_at_ms: i64,
    ) -> Result<GrokAccountRollbackOutcome, GrokAccountPoolError> {
        if !valid_component(batch_id, MAX_OPAQUE_ID_BYTES) || observed_at_ms < 0 {
            return Err(GrokAccountPoolError::InvalidRequest);
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let status = transaction
            .query_row(
                "SELECT status FROM grok_account_import_batches WHERE id = ?1",
                [batch_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
            .ok_or(GrokAccountPoolError::NotFound)?;
        if status == "rolled_back" {
            return Ok(GrokAccountRollbackOutcome {
                removed: 0,
                already_rolled_back: true,
            });
        }
        if status != "applied" {
            return Err(GrokAccountPoolError::InvalidPersistedState);
        }
        let removed = transaction
            .execute(
                "DELETE FROM grok_accounts WHERE import_batch_id = ?1",
                [batch_id],
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        transaction
            .execute(
                "UPDATE grok_account_import_batches \
                 SET status = 'rolled_back', rolled_back_at_ms = ?2 WHERE id = ?1",
                params![batch_id, observed_at_ms],
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        transaction
            .commit()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        Ok(GrokAccountRollbackOutcome {
            removed,
            already_rolled_back: false,
        })
    }

    /// Lists only redacted metadata; identity digests and ciphertext are intentionally omitted.
    ///
    /// # Errors
    ///
    /// Returns a safe storage or invalid-persisted-state classification.
    pub fn list_accounts(&self) -> Result<Vec<GrokAccountMetadata>, GrokAccountPoolError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let mut statement = connection
            .prepare(
                "SELECT id, provider, auth_status, enabled, priority, weight, max_concurrency, \
                        refresh_due_at_ms, quota_sync_due_at_ms, cooldown_until_ms, revision, \
                        import_batch_id \
                 FROM grok_accounts ORDER BY provider, id",
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        statement
            .query_map([], decode_metadata)
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
            .map(|row| {
                row.map_err(|_| GrokAccountPoolError::InvalidPersistedState)
                    .and_then(validate_metadata)
            })
            .collect()
    }

    /// Compiles enabled native accounts into the gateway's existing immutable Credential pools.
    ///
    /// Disabled accounts never enter a pool. Reauthentication-required accounts do enter so the
    /// shared controlled recovery flow can address them, but the returned compilation seeds an
    /// exact unauthorized Health block before scheduling begins. Future persisted cooldowns are
    /// seeded into that same registry. Higher native priorities are monotonically translated to
    /// the existing pool's lower-is-preferred priority domain.
    ///
    /// # Errors
    ///
    /// Returns a safe binding, persisted-account, decryption, or bounded-pool classification. No
    /// partial pool set is returned.
    pub fn compile_native_runtime(
        &self,
        bindings: &[GrokAccountEndpointBinding],
        observed_at_ms: i64,
    ) -> Result<GrokNativeAccountPoolCompilation, GrokNativeAccountCompileError> {
        if observed_at_ms < 0 {
            return Err(GrokNativeAccountCompileError::InvalidObservationTime);
        }
        let mut bindings_by_provider = BTreeMap::new();
        let mut endpoint_ids = BTreeSet::new();
        for binding in bindings {
            if bindings_by_provider
                .insert(binding.provider, binding.endpoint_id.clone())
                .is_some()
                || !endpoint_ids.insert(binding.endpoint_id.clone())
            {
                return Err(GrokNativeAccountCompileError::DuplicateBinding);
            }
        }

        let mut inputs_by_endpoint: BTreeMap<EndpointId, Vec<EndpointCredentialInput>> =
            BTreeMap::new();
        let mut providers_by_credential = BTreeMap::new();
        let mut runtime_bindings_by_account = BTreeMap::new();
        let mut health_bootstrap = Vec::new();
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        for row in load_native_runtime_rows(&connection)? {
            let provider = GrokAccountProvider::parse(&row.provider)?;
            let auth_status = GrokAccountAuthStatus::parse(&row.auth_status)?;
            let enabled = sqlite_bool(row.enabled)?;
            if !enabled || auth_status == GrokAccountAuthStatus::Disabled {
                continue;
            }
            let Some(endpoint_id) = bindings_by_provider.get(&provider) else {
                continue;
            };
            let identity_digest: [u8; 32] = row
                .identity_digest
                .try_into()
                .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?;
            let persisted = PersistedCredential {
                provider,
                identity_digest,
                ciphertext: row.ciphertext,
                key_version: KeyVersion::try_from_sqlite_i64(row.key_version)
                    .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
            };
            let plaintext = open_persisted_credential(&self.secret_store, &persisted)?;
            let account_id = row.id.clone();
            let credential_id = CredentialId::try_new(row.id)
                .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?;
            let priority = 1_000_i64
                .checked_sub(row.priority)
                .ok_or(GrokAccountPoolError::InvalidPersistedState)?;
            let input = EndpointCredentialInput {
                credential_id: credential_id.clone(),
                credential_kind: provider.credential_kind().to_owned(),
                credential_revision: row.revision,
                priority,
                weight: row.weight,
                concurrency: row.max_concurrency,
                secret: CredentialSecret::try_new(plaintext.as_bytes().to_vec())?,
            };
            if providers_by_credential
                .insert(credential_id.clone(), provider)
                .is_some()
            {
                return Err(GrokAccountPoolError::InvalidPersistedState.into());
            }
            runtime_bindings_by_account
                .insert(account_id, (endpoint_id.clone(), credential_id.clone()));
            if auth_status == GrokAccountAuthStatus::ReauthRequired {
                health_bootstrap.push(GrokRuntimeHealthBootstrap::Unauthorized {
                    endpoint_id: endpoint_id.clone(),
                    credential_id: credential_id.clone(),
                });
            } else if let Some(until_ms) = row
                .cooldown_until_ms
                .filter(|until_ms| *until_ms > observed_at_ms)
            {
                health_bootstrap.push(GrokRuntimeHealthBootstrap::Cooldown {
                    endpoint_id: endpoint_id.clone(),
                    credential_id: credential_id.clone(),
                    until_ms,
                });
            }
            inputs_by_endpoint
                .entry(endpoint_id.clone())
                .or_default()
                .push(input);
        }
        let quota_bootstrap = load_quota_bootstrap(&connection, &runtime_bindings_by_account)?;
        drop(connection);
        let pools = inputs_by_endpoint
            .into_iter()
            .map(|(endpoint_id, inputs)| EndpointCredentialPool::try_new(endpoint_id, inputs))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(GrokNativeAccountPoolCompilation {
            credential_pools: Arc::new(EndpointCredentialPools::try_new(pools)?),
            providers_by_credential,
            health_bootstrap,
            quota_bootstrap,
        })
    }

    /// Opens one exact credential for an authorized native runtime caller.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, missing-resource, persisted-state, or authentication failure.
    pub fn open_credential(
        &self,
        account_id: &str,
    ) -> Result<PlaintextSecret, GrokAccountPoolError> {
        if !valid_component(account_id, MAX_OPAQUE_ID_BYTES) {
            return Err(GrokAccountPoolError::InvalidRequest);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let persisted = load_persisted_credential(&connection, account_id)?
            .ok_or(GrokAccountPoolError::NotFound)?;
        open_persisted_credential(&self.secret_store, &persisted)
    }

    /// Adds a metadata-only relationship without merging account health, quota, or cooldown.
    ///
    /// # Errors
    ///
    /// Returns a safe validation or storage classification. Missing account references fail
    /// closed through `SQLite` foreign-key enforcement.
    pub fn link_accounts(
        &self,
        source_account_id: &str,
        target_account_id: &str,
        relation: &str,
        observed_at_ms: i64,
    ) -> Result<(), GrokAccountPoolError> {
        if !valid_component(source_account_id, MAX_OPAQUE_ID_BYTES)
            || !valid_component(target_account_id, MAX_OPAQUE_ID_BYTES)
            || source_account_id == target_account_id
            || !valid_component(relation, MAX_RELATION_BYTES)
            || observed_at_ms < 0
        {
            return Err(GrokAccountPoolError::InvalidRequest);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        connection
            .execute(
                "INSERT OR IGNORE INTO grok_account_links (\
                    source_account_id, target_account_id, relation, created_at_ms\
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    source_account_id,
                    target_account_id,
                    relation,
                    observed_at_ms
                ],
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        Ok(())
    }

    fn import_one(
        &self,
        transaction: &Transaction<'_>,
        batch_id: &str,
        entry: &GrokAccountImport,
        observed_at_ms: i64,
    ) -> Result<ImportOneOutcome, GrokAccountPoolError> {
        let digest = identity_digest(entry.provider, &entry.identity.0);
        let existing = load_existing_by_identity(transaction, entry.provider, &digest)?;
        if let Some(existing) = existing {
            let plaintext = open_persisted_credential(&self.secret_store, &existing.credential)?;
            let credential_matches = plaintext.as_bytes() == entry.credential.0;
            let metadata_matches = existing.enabled == entry.enabled
                && existing.auth_status == entry.auth_status
                && existing.priority == entry.priority
                && existing.weight == entry.weight
                && existing.max_concurrency == entry.max_concurrency
                && existing.refresh_due_at_ms == entry.refresh_due_at_ms
                && existing.quota_sync_due_at_ms == entry.quota_sync_due_at_ms
                && existing.cooldown_until_ms == entry.cooldown_until_ms;
            return if credential_matches && metadata_matches {
                Ok(ImportOneOutcome::Unchanged(existing.id))
            } else {
                Err(GrokAccountPoolError::ExistingAccountConflict)
            };
        }

        let associated_data = credential_aad(entry.provider, &digest);
        let encrypted = self
            .secret_store
            .seal(&entry.credential.0, &associated_data)
            .map_err(|_| GrokAccountPoolError::SecretStoreFailure)?;
        let account_id = random_account_id()?;
        transaction
            .execute(
                "INSERT INTO grok_accounts (\
                    id, provider, identity_digest, credential_ciphertext, credential_key_version, \
                    auth_status, enabled, priority, weight, max_concurrency, refresh_due_at_ms, \
                    last_refresh_at_ms, refresh_failure_count, cooldown_until_ms, revision, \
                    import_batch_id, created_at_ms, updated_at_ms, quota_sync_due_at_ms\
                 ) VALUES (\
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, \
                    NULL, 0, ?12, 0, ?13, ?14, ?14, ?15\
                 )",
                params![
                    account_id,
                    entry.provider.as_str(),
                    digest.as_slice(),
                    encrypted.ciphertext(),
                    encrypted.key_version().as_sqlite_i64(),
                    entry.auth_status.as_str(),
                    i64::from(entry.enabled),
                    entry.priority,
                    i64::from(entry.weight),
                    i64::from(entry.max_concurrency),
                    entry.refresh_due_at_ms,
                    entry.cooldown_until_ms,
                    batch_id,
                    observed_at_ms,
                    entry.quota_sync_due_at_ms,
                ],
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        Ok(ImportOneOutcome::Created(account_id))
    }
}

enum ImportOneOutcome {
    Created(String),
    Unchanged(String),
}

struct ExistingAccount {
    id: String,
    credential: PersistedCredential,
    auth_status: GrokAccountAuthStatus,
    enabled: bool,
    priority: i64,
    weight: u32,
    max_concurrency: u32,
    refresh_due_at_ms: Option<i64>,
    quota_sync_due_at_ms: Option<i64>,
    cooldown_until_ms: Option<i64>,
}

struct PersistedCredential {
    provider: GrokAccountProvider,
    identity_digest: [u8; 32],
    ciphertext: Vec<u8>,
    key_version: KeyVersion,
}

struct NativeRuntimeAccountRow {
    id: String,
    provider: String,
    identity_digest: Vec<u8>,
    ciphertext: Vec<u8>,
    key_version: i64,
    auth_status: String,
    enabled: i64,
    priority: i64,
    weight: i64,
    max_concurrency: i64,
    cooldown_until_ms: Option<i64>,
    revision: i64,
}

struct PersistedQuotaRow {
    account_id: String,
    scope_kind: String,
    model_label: String,
    window_label: String,
    limit: Option<i64>,
    remaining: Option<i64>,
    reset_at_ms: Option<i64>,
    source: String,
    confidence: String,
    observed_at_ms: i64,
}

struct QuotaBootstrapGroup {
    source: QuotaSource,
    confidence: QuotaConfidence,
    observed_at_ms: i64,
    windows: Vec<QuotaWindow>,
}

fn load_native_runtime_rows(
    connection: &Connection,
) -> Result<Vec<NativeRuntimeAccountRow>, GrokAccountPoolError> {
    let mut statement = connection
        .prepare(
            "SELECT id, provider, identity_digest, credential_ciphertext, \
                    credential_key_version, auth_status, enabled, priority, weight, \
                    max_concurrency, cooldown_until_ms, revision \
             FROM grok_accounts ORDER BY provider, id",
        )
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
    statement
        .query_map([], |row| {
            Ok(NativeRuntimeAccountRow {
                id: row.get(0)?,
                provider: row.get(1)?,
                identity_digest: row.get(2)?,
                ciphertext: row.get(3)?,
                key_version: row.get(4)?,
                auth_status: row.get(5)?,
                enabled: row.get(6)?,
                priority: row.get(7)?,
                weight: row.get(8)?,
                max_concurrency: row.get(9)?,
                cooldown_until_ms: row.get(10)?,
                revision: row.get(11)?,
            })
        })
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)
}

fn load_quota_bootstrap(
    connection: &Connection,
    runtime_bindings: &BTreeMap<String, (EndpointId, CredentialId)>,
) -> Result<Vec<QuotaSnapshot>, GrokAccountPoolError> {
    let mut statement = connection
        .prepare(
            "SELECT account_id, scope_kind, model_label, window_label, quota_limit, \
                    quota_remaining, reset_at_ms, source, confidence, observed_at_ms \
             FROM grok_account_quota_windows \
             ORDER BY account_id, scope_kind, model_label, window_label",
        )
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
    let rows = statement
        .query_map([], |row| {
            Ok(PersistedQuotaRow {
                account_id: row.get(0)?,
                scope_kind: row.get(1)?,
                model_label: row.get(2)?,
                window_label: row.get(3)?,
                limit: row.get(4)?,
                remaining: row.get(5)?,
                reset_at_ms: row.get(6)?,
                source: row.get(7)?,
                confidence: row.get(8)?,
                observed_at_ms: row.get(9)?,
            })
        })
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
    let mut groups: BTreeMap<RuntimeQuotaTarget, QuotaBootstrapGroup> = BTreeMap::new();
    for row in rows {
        let Some((endpoint_id, credential_id)) = runtime_bindings.get(&row.account_id) else {
            continue;
        };
        let target = match row.scope_kind.as_str() {
            "account" if row.model_label.is_empty() => {
                RuntimeQuotaTarget::endpoint_credential(endpoint_id.clone(), credential_id.clone())
            }
            "model" if !row.model_label.is_empty() => {
                RuntimeQuotaTarget::endpoint_credential_model(
                    endpoint_id.clone(),
                    credential_id.clone(),
                    row.model_label,
                )
                .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?
            }
            _ => return Err(GrokAccountPoolError::InvalidPersistedState),
        };
        let source = parse_quota_source(&row.source)?;
        let confidence = parse_quota_confidence(&row.confidence)?;
        let window = QuotaWindow::try_new(
            row.window_label,
            optional_i64_to_u64(row.limit)?,
            optional_i64_to_u64(row.remaining)?,
            row.reset_at_ms,
        )
        .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?;
        let group = groups.entry(target).or_insert_with(|| QuotaBootstrapGroup {
            source,
            confidence,
            observed_at_ms: row.observed_at_ms,
            windows: Vec::new(),
        });
        if group.source != source
            || group.confidence != confidence
            || group.observed_at_ms != row.observed_at_ms
        {
            return Err(GrokAccountPoolError::InvalidPersistedState);
        }
        group.windows.push(window);
    }
    groups
        .into_iter()
        .map(|(target, group)| {
            QuotaSnapshot::try_new(
                target,
                group.windows,
                group.source,
                group.confidence,
                group.observed_at_ms,
            )
            .map_err(|_| GrokAccountPoolError::InvalidPersistedState)
        })
        .collect()
}

fn parse_quota_source(value: &str) -> Result<QuotaSource, GrokAccountPoolError> {
    match value {
        "billing" => Ok(QuotaSource::Billing),
        "rest" => Ok(QuotaSource::Rest),
        "grpc" => Ok(QuotaSource::Grpc),
        "estimated" => Ok(QuotaSource::Estimated),
        _ => Err(GrokAccountPoolError::InvalidPersistedState),
    }
}

fn parse_quota_confidence(value: &str) -> Result<QuotaConfidence, GrokAccountPoolError> {
    match value {
        "authoritative" => Ok(QuotaConfidence::Authoritative),
        "observed" => Ok(QuotaConfidence::Observed),
        "estimated" => Ok(QuotaConfidence::Estimated),
        _ => Err(GrokAccountPoolError::InvalidPersistedState),
    }
}

fn optional_i64_to_u64(value: Option<i64>) -> Result<Option<u64>, GrokAccountPoolError> {
    value
        .map(|value| u64::try_from(value).map_err(|_| GrokAccountPoolError::InvalidPersistedState))
        .transpose()
}

fn load_existing_by_identity(
    transaction: &Transaction<'_>,
    provider: GrokAccountProvider,
    identity_digest: &[u8; 32],
) -> Result<Option<ExistingAccount>, GrokAccountPoolError> {
    transaction
        .query_row(
            "SELECT id, credential_ciphertext, credential_key_version, auth_status, enabled, \
                    priority, weight, max_concurrency, refresh_due_at_ms, quota_sync_due_at_ms, \
                    cooldown_until_ms \
             FROM grok_accounts WHERE provider = ?1 AND identity_digest = ?2",
            params![provider.as_str(), identity_digest.as_slice()],
            |row| {
                let key_version = row.get::<_, i64>(2)?;
                let weight = row.get::<_, i64>(6)?;
                let max_concurrency = row.get::<_, i64>(7)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    key_version,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    weight,
                    max_concurrency,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                ))
            },
        )
        .optional()
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
        .map(
            |(
                id,
                ciphertext,
                key_version,
                auth_status,
                enabled,
                priority,
                weight,
                max_concurrency,
                refresh_due,
                quota_sync_due,
                cooldown_until,
            )| {
                Ok(ExistingAccount {
                    id,
                    credential: PersistedCredential {
                        provider,
                        identity_digest: *identity_digest,
                        ciphertext,
                        key_version: KeyVersion::try_from_sqlite_i64(key_version)
                            .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
                    },
                    auth_status: GrokAccountAuthStatus::parse(&auth_status)?,
                    enabled: sqlite_bool(enabled)?,
                    priority,
                    weight: u32::try_from(weight)
                        .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
                    max_concurrency: u32::try_from(max_concurrency)
                        .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
                    refresh_due_at_ms: refresh_due,
                    quota_sync_due_at_ms: quota_sync_due,
                    cooldown_until_ms: cooldown_until,
                })
            },
        )
        .transpose()
}

fn load_persisted_credential(
    connection: &Connection,
    account_id: &str,
) -> Result<Option<PersistedCredential>, GrokAccountPoolError> {
    connection
        .query_row(
            "SELECT provider, identity_digest, credential_ciphertext, credential_key_version \
             FROM grok_accounts WHERE id = ?1",
            [account_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
        .map(|(provider, digest, ciphertext, key_version)| {
            Ok(PersistedCredential {
                provider: GrokAccountProvider::parse(&provider)?,
                identity_digest: digest
                    .try_into()
                    .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
                ciphertext,
                key_version: KeyVersion::try_from_sqlite_i64(key_version)
                    .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
            })
        })
        .transpose()
}

fn open_persisted_credential(
    secret_store: &SecretStore,
    persisted: &PersistedCredential,
) -> Result<PlaintextSecret, GrokAccountPoolError> {
    let encrypted =
        EncryptedSecret::try_from_persisted(persisted.key_version, persisted.ciphertext.clone())
            .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?;
    secret_store
        .open(
            &encrypted,
            &credential_aad(persisted.provider, &persisted.identity_digest),
        )
        .map_err(|_| GrokAccountPoolError::SecretStoreFailure)
}

fn decode_metadata(row: &rusqlite::Row<'_>) -> rusqlite::Result<GrokAccountMetadata> {
    let provider = row.get::<_, String>(1)?;
    let auth_status = row.get::<_, String>(2)?;
    let enabled = row.get::<_, i64>(3)?;
    let weight = row.get::<_, i64>(5)?;
    let max_concurrency = row.get::<_, i64>(6)?;
    let revision = row.get::<_, i64>(10)?;
    Ok(GrokAccountMetadata {
        id: row.get(0)?,
        provider: GrokAccountProvider::parse(&provider).map_err(|_| {
            rusqlite::Error::InvalidColumnType(
                1,
                "provider".to_owned(),
                rusqlite::types::Type::Text,
            )
        })?,
        auth_status: GrokAccountAuthStatus::parse(&auth_status).map_err(|_| {
            rusqlite::Error::InvalidColumnType(
                2,
                "auth_status".to_owned(),
                rusqlite::types::Type::Text,
            )
        })?,
        enabled: sqlite_bool(enabled)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, enabled))?,
        priority: row.get(4)?,
        weight: u32::try_from(weight)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(5, weight))?,
        max_concurrency: u32::try_from(max_concurrency)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, max_concurrency))?,
        refresh_due_at_ms: row.get(7)?,
        quota_sync_due_at_ms: row.get(8)?,
        cooldown_until_ms: row.get(9)?,
        revision: u64::try_from(revision)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(10, revision))?,
        import_batch_id: row.get(11)?,
    })
}

fn validate_metadata(
    metadata: GrokAccountMetadata,
) -> Result<GrokAccountMetadata, GrokAccountPoolError> {
    if !valid_component(&metadata.id, MAX_OPAQUE_ID_BYTES)
        || !valid_component(&metadata.import_batch_id, MAX_OPAQUE_ID_BYTES)
        || !(-1_000..=1_000).contains(&metadata.priority)
        || !(1..=10_000).contains(&metadata.weight)
        || !(1..=10_000).contains(&metadata.max_concurrency)
        || metadata.refresh_due_at_ms.is_some_and(|value| value < 0)
        || metadata.quota_sync_due_at_ms.is_some_and(|value| value < 0)
        || metadata.cooldown_until_ms.is_some_and(|value| value < 0)
    {
        return Err(GrokAccountPoolError::InvalidPersistedState);
    }
    Ok(metadata)
}

fn identity_digest(provider: GrokAccountProvider, identity: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(ACCOUNT_SECRET_AAD_DOMAIN);
    digest.update([0]);
    digest.update(provider.as_str().as_bytes());
    digest.update([0]);
    digest.update(identity);
    digest.finalize().into()
}

pub(crate) fn credential_aad(provider: GrokAccountProvider, identity_digest: &[u8; 32]) -> Vec<u8> {
    let mut associated_data = Vec::with_capacity(
        ACCOUNT_SECRET_AAD_DOMAIN.len() + provider.as_str().len() + identity_digest.len() + 2,
    );
    associated_data.extend_from_slice(ACCOUNT_SECRET_AAD_DOMAIN);
    associated_data.push(0);
    associated_data.extend_from_slice(provider.as_str().as_bytes());
    associated_data.push(0);
    associated_data.extend_from_slice(identity_digest);
    associated_data
}

fn random_account_id() -> Result<String, GrokAccountPoolError> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
    let mut id = String::with_capacity(5 + random.len() * 2);
    id.push_str("grok-");
    for byte in random {
        use std::fmt::Write as _;
        write!(&mut id, "{byte:02x}").map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
    }
    Ok(id)
}

fn sqlite_bool(value: i64) -> Result<bool, GrokAccountPoolError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(GrokAccountPoolError::InvalidPersistedState),
    }
}

fn valid_component(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes
}
