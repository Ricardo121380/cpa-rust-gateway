//! Native, encrypted Grok Build/Web/Console account-pool persistence.
//!
//! This boundary deliberately stores an opaque account identifier and a provider-scoped identity
//! digest instead of source identity text. Import is bounded and transactional: credentials are
//! authenticated-encrypted immediately, duplicates are idempotent only when both metadata and
//! plaintext match, and every newly created account remains attributable to a reversible batch.

use std::{collections::BTreeSet, error::Error, fmt, sync::Mutex};

use gateway_store::{
    migrate,
    secret_store::{EncryptedSecret, KeyVersion, PlaintextSecret, SecretStore},
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
    const fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Web => "web",
            Self::Console => "console",
        }
    }

    fn parse(value: &str) -> Result<Self, GrokAccountPoolError> {
        match value {
            "build" => Ok(Self::Build),
            "web" => Ok(Self::Web),
            "console" => Ok(Self::Console),
            _ => Err(GrokAccountPoolError::InvalidPersistedState),
        }
    }
}

/// Authentication eligibility kept separate from quota and cooldown state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
}

impl GrokAccountImport {
    fn validate(&self) -> Result<(), GrokAccountPoolError> {
        if !(-1_000..=1_000).contains(&self.priority)
            || !(1..=10_000).contains(&self.weight)
            || !(1..=10_000).contains(&self.max_concurrency)
            || self.refresh_due_at_ms.is_some_and(|value| value < 0)
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
    /// Account-local cooldown deadline, when active.
    pub cooldown_until_ms: Option<i64>,
    /// Monotonic credential revision.
    pub revision: u64,
    /// Reversible import provenance.
    pub import_batch_id: String,
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
    connection: Mutex<Connection>,
    secret_store: SecretStore,
}

impl GrokAccountPoolStore {
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
        if !valid_component(batch_id, MAX_OPAQUE_ID_BYTES)
            || entries.is_empty()
            || entries.len() > MAX_BATCH_ITEMS
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
        for entry in entries {
            match self.import_one(&transaction, batch_id, entry, observed_at_ms)? {
                ImportOneOutcome::Created => created += 1,
                ImportOneOutcome::Unchanged => unchanged += 1,
            }
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
                        refresh_due_at_ms, cooldown_until_ms, revision, import_batch_id \
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
                && existing.priority == entry.priority
                && existing.weight == entry.weight
                && existing.max_concurrency == entry.max_concurrency
                && existing.refresh_due_at_ms == entry.refresh_due_at_ms;
            return if credential_matches && metadata_matches {
                Ok(ImportOneOutcome::Unchanged)
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
                    import_batch_id, created_at_ms, updated_at_ms\
                 ) VALUES (\
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, \
                    NULL, 0, NULL, 0, ?12, ?13, ?13\
                 )",
                params![
                    account_id,
                    entry.provider.as_str(),
                    digest.as_slice(),
                    encrypted.ciphertext(),
                    encrypted.key_version().as_sqlite_i64(),
                    GrokAccountAuthStatus::Active.as_str(),
                    i64::from(entry.enabled),
                    entry.priority,
                    i64::from(entry.weight),
                    i64::from(entry.max_concurrency),
                    entry.refresh_due_at_ms,
                    batch_id,
                    observed_at_ms,
                ],
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        Ok(ImportOneOutcome::Created)
    }
}

enum ImportOneOutcome {
    Created,
    Unchanged,
}

struct ExistingAccount {
    credential: PersistedCredential,
    enabled: bool,
    priority: i64,
    weight: u32,
    max_concurrency: u32,
    refresh_due_at_ms: Option<i64>,
}

struct PersistedCredential {
    provider: GrokAccountProvider,
    identity_digest: [u8; 32],
    ciphertext: Vec<u8>,
    key_version: KeyVersion,
}

fn load_existing_by_identity(
    transaction: &Transaction<'_>,
    provider: GrokAccountProvider,
    identity_digest: &[u8; 32],
) -> Result<Option<ExistingAccount>, GrokAccountPoolError> {
    transaction
        .query_row(
            "SELECT credential_ciphertext, credential_key_version, enabled, priority, weight, \
                    max_concurrency, refresh_due_at_ms \
             FROM grok_accounts WHERE provider = ?1 AND identity_digest = ?2",
            params![provider.as_str(), identity_digest.as_slice()],
            |row| {
                let key_version = row.get::<_, i64>(1)?;
                let weight = row.get::<_, i64>(4)?;
                let max_concurrency = row.get::<_, i64>(5)?;
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    key_version,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    weight,
                    max_concurrency,
                    row.get::<_, Option<i64>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
        .map(
            |(ciphertext, key_version, enabled, priority, weight, max_concurrency, refresh_due)| {
                Ok(ExistingAccount {
                    credential: PersistedCredential {
                        provider,
                        identity_digest: *identity_digest,
                        ciphertext,
                        key_version: KeyVersion::try_from_sqlite_i64(key_version)
                            .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
                    },
                    enabled: sqlite_bool(enabled)?,
                    priority,
                    weight: u32::try_from(weight)
                        .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
                    max_concurrency: u32::try_from(max_concurrency)
                        .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
                    refresh_due_at_ms: refresh_due,
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
    let revision = row.get::<_, i64>(9)?;
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
        cooldown_until_ms: row.get(8)?,
        revision: u64::try_from(revision)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(9, revision))?,
        import_batch_id: row.get(10)?,
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

fn credential_aad(provider: GrokAccountProvider, identity_digest: &[u8; 32]) -> Vec<u8> {
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
