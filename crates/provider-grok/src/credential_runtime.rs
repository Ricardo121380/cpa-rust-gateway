//! Durable, revision-guarded Grok Build OAuth refresh state.
//!
//! Refresh coordination is deliberately per Credential identity. It serializes only concurrent
//! refreshes for the same Config Version/Credential pair, persists the newly sealed credential with
//! compare-and-swap, and refuses to overwrite a newer revision with an old refresh result.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    path::Path,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use gateway_core::CredentialId;
use gateway_store::{
    control_plane::ConfigVersionId,
    secret_store::{EncryptedSecret, KeyVersion, SecretStore},
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    GrokBuildCredential, GrokBuildOAuthError, GrokBuildOAuthFlow, GrokBuildOAuthTransport,
};

const CREDENTIAL_RUNTIME_AAD_DOMAIN: &[u8] = b"cpa-rust-gateway/grok-build/credential-runtime/v1";
const MAX_RUNTIME_ID_BYTES: usize = 128;

/// Default upper bound for waiting on an in-process refresh already owned by the same Credential.
///
/// This bounds waiters only. P6-03 owns the separate network deadline for a leader's actual OAuth
/// request.
pub const DEFAULT_GROK_BUILD_REFRESH_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Exact durable identity for one Grok Build OAuth runtime credential.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GrokBuildCredentialKey {
    config_version_id: ConfigVersionId,
    credential_id: CredentialId,
}

/// Safe validation failure for a Grok Build runtime identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildCredentialKeyError {
    /// The Config Version identifier is blank or exceeds the control-plane's fixed bound.
    InvalidConfigVersionId,
    /// The Credential identifier is blank or exceeds the control-plane's fixed bound.
    InvalidCredentialId,
}

impl fmt::Display for GrokBuildCredentialKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfigVersionId => {
                formatter.write_str("Grok Build runtime Config Version identity is invalid")
            }
            Self::InvalidCredentialId => {
                formatter.write_str("Grok Build runtime Credential identity is invalid")
            }
        }
    }
}

impl Error for GrokBuildCredentialKeyError {}

impl GrokBuildCredentialKey {
    /// Creates one bounded, non-blank Config Version-scoped Credential identity.
    ///
    /// # Errors
    ///
    /// Returns a classification-only error when either opaque identifier cannot represent a
    /// control-plane-compatible runtime key.
    pub fn try_new(
        config_version_id: ConfigVersionId,
        credential_id: CredentialId,
    ) -> Result<Self, GrokBuildCredentialKeyError> {
        if !is_runtime_identity_component(config_version_id.as_str()) {
            return Err(GrokBuildCredentialKeyError::InvalidConfigVersionId);
        }
        if !is_runtime_identity_component(credential_id.as_str()) {
            return Err(GrokBuildCredentialKeyError::InvalidCredentialId);
        }
        Ok(Self {
            config_version_id,
            credential_id,
        })
    }

    /// Returns the exact Config Version owning this runtime state.
    #[must_use]
    pub const fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Returns the stable non-secret Credential identifier.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }
}

/// One revisioned in-memory view of a durable Grok Build credential.
#[derive(Clone)]
pub struct GrokBuildCredentialVersion {
    credential: GrokBuildCredential,
    revision: u64,
}

impl GrokBuildCredentialVersion {
    /// Creates a credential view with an exact non-negative durable revision.
    #[must_use]
    pub const fn new(credential: GrokBuildCredential, revision: u64) -> Self {
        Self {
            credential,
            revision,
        }
    }

    /// Returns the redacted, zeroizing credential material for immediate request/refresh use.
    #[must_use]
    pub fn credential(&self) -> &GrokBuildCredential {
        &self.credential
    }

    /// Returns the durable revision associated with this exact credential value.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

impl fmt::Debug for GrokBuildCredentialVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildCredentialVersion")
            .field("credential", &self.credential)
            .field("revision", &self.revision)
            .finish()
    }
}

/// Result of inserting an initial durable credential state.
#[derive(Clone, Debug)]
pub enum GrokBuildCredentialInsertOutcome {
    /// The supplied credential became revision zero.
    Inserted(GrokBuildCredentialVersion),
    /// A state already existed and was loaded instead of being overwritten.
    Existing(GrokBuildCredentialVersion),
}

impl GrokBuildCredentialInsertOutcome {
    /// Returns the resulting durable version without changing its state.
    #[must_use]
    pub const fn version(&self) -> &GrokBuildCredentialVersion {
        match self {
            Self::Inserted(version) | Self::Existing(version) => version,
        }
    }
}

/// Result of one expected-revision durable credential update.
#[derive(Clone, Debug)]
pub enum GrokBuildCredentialCasOutcome {
    /// The new credential committed at the next exact revision.
    Committed(GrokBuildCredentialVersion),
    /// No state existed for the requested identity.
    Missing,
    /// The durable revision no longer matched the caller's expected revision.
    Conflict,
}

/// Safe failure classification for durable credential operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildCredentialPersistenceError {
    /// The state database or its in-process lock was unavailable.
    StoreUnavailable,
    /// The persisted row, revision, envelope, or decrypted payload was structurally invalid.
    InvalidPersistedState,
    /// Sealing or opening the existing authenticated credential state failed.
    SecretStoreFailure,
    /// The next revision cannot be represented as a non-negative `SQLite` integer.
    RevisionOverflow,
    /// Runtime-state associated data could not be constructed within its fixed bounds.
    InvalidAssociatedData,
}

impl fmt::Display for GrokBuildCredentialPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::StoreUnavailable => "Grok Build credential state store is unavailable",
            Self::InvalidPersistedState => "Grok Build credential state is invalid",
            Self::SecretStoreFailure => "Grok Build credential state encryption failed",
            Self::RevisionOverflow => "Grok Build credential revision is invalid",
            Self::InvalidAssociatedData => "Grok Build credential state identity is invalid",
        };
        formatter.write_str(message)
    }
}

impl Error for GrokBuildCredentialPersistenceError {}

/// Durable state port for one revisioned Grok Build OAuth credential.
///
/// Implementations must bind sealed plaintext to the complete [`GrokBuildCredentialKey`] and must
/// make `compare_and_swap` one atomic persistent operation. The refresh coordinator does not keep
/// a second, mutable token cache outside this port.
pub trait GrokBuildCredentialPersistence: Send + Sync {
    /// Loads one exact Credential state, if it exists.
    ///
    /// # Errors
    ///
    /// Returns a safe persistence error without exposing token plaintext or ciphertext.
    fn load(
        &self,
        key: &GrokBuildCredentialKey,
    ) -> Result<Option<GrokBuildCredentialVersion>, GrokBuildCredentialPersistenceError>;

    /// Persists an initial credential only if no state exists at this exact identity.
    ///
    /// # Errors
    ///
    /// Returns a safe persistence error without replacing an already present state.
    fn insert_if_absent(
        &self,
        key: &GrokBuildCredentialKey,
        credential: &GrokBuildCredential,
        updated_at_ms: i64,
    ) -> Result<GrokBuildCredentialInsertOutcome, GrokBuildCredentialPersistenceError>;

    /// Atomically replaces one credential only when its durable revision still matches.
    ///
    /// # Errors
    ///
    /// Returns a safe persistence error. A normal missing or stale revision is returned as an
    /// explicit [`GrokBuildCredentialCasOutcome`] rather than an ambiguous error.
    fn compare_and_swap(
        &self,
        key: &GrokBuildCredentialKey,
        expected_revision: u64,
        credential: &GrokBuildCredential,
        updated_at_ms: i64,
    ) -> Result<GrokBuildCredentialCasOutcome, GrokBuildCredentialPersistenceError>;
}

/// SQLite-backed, AEAD-sealed implementation of [`GrokBuildCredentialPersistence`].
///
/// The P6-02 migration owns its narrow runtime table. It is separate from the immutable active
/// configuration graph because OAuth refreshes can advance a Credential revision without changing
/// routes, Endpoint selection, or a published Snapshot.
pub struct GrokBuildCredentialSqliteStore {
    connection: Mutex<Connection>,
    secret_store: SecretStore,
}

impl GrokBuildCredentialSqliteStore {
    /// Opens and migrates one `SQLite` database for encrypted Grok Build runtime state.
    ///
    /// # Errors
    ///
    /// Returns a safe persistence error if the database cannot open/migrate; no path, token, or
    /// ciphertext is included in the error.
    pub fn open(
        path: impl AsRef<Path>,
        secret_store: SecretStore,
    ) -> Result<Self, GrokBuildCredentialPersistenceError> {
        let mut connection = gateway_store::open(path)
            .map_err(|_| GrokBuildCredentialPersistenceError::StoreUnavailable)?;
        gateway_store::migrate(&mut connection)
            .map_err(|_| GrokBuildCredentialPersistenceError::StoreUnavailable)?;
        Ok(Self {
            connection: Mutex::new(connection),
            secret_store,
        })
    }

    /// Opens and migrates an isolated in-memory state database for deterministic tests.
    ///
    /// # Errors
    ///
    /// Returns a safe persistence error if `SQLite` cannot initialize or migrations fail.
    pub fn open_in_memory(
        secret_store: SecretStore,
    ) -> Result<Self, GrokBuildCredentialPersistenceError> {
        let mut connection = gateway_store::open_in_memory()
            .map_err(|_| GrokBuildCredentialPersistenceError::StoreUnavailable)?;
        gateway_store::migrate(&mut connection)
            .map_err(|_| GrokBuildCredentialPersistenceError::StoreUnavailable)?;
        Ok(Self {
            connection: Mutex::new(connection),
            secret_store,
        })
    }

    fn seal(
        &self,
        key: &GrokBuildCredentialKey,
        credential: &GrokBuildCredential,
    ) -> Result<EncryptedSecret, GrokBuildCredentialPersistenceError> {
        let plaintext = credential
            .persisted_bytes()
            .map_err(|_| GrokBuildCredentialPersistenceError::InvalidPersistedState)?;
        let aad = credential_associated_data(key)?;
        self.secret_store
            .seal(plaintext.as_slice(), &aad)
            .map_err(|_| GrokBuildCredentialPersistenceError::SecretStoreFailure)
    }

    fn load_row(
        &self,
        key: &GrokBuildCredentialKey,
    ) -> Result<Option<SqliteCredentialRow>, GrokBuildCredentialPersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildCredentialPersistenceError::StoreUnavailable)?;
        connection
            .query_row(
                "SELECT revision, ciphertext, key_version FROM grok_build_credential_runtime \
                 WHERE config_version_id = ?1 AND credential_id = ?2",
                params![
                    key.config_version_id().as_str(),
                    key.credential_id().as_str(),
                ],
                |row| {
                    Ok(SqliteCredentialRow {
                        revision: row.get(0)?,
                        ciphertext: row.get(1)?,
                        key_version: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(|_| GrokBuildCredentialPersistenceError::StoreUnavailable)
    }

    fn decode_row(
        &self,
        key: &GrokBuildCredentialKey,
        row: SqliteCredentialRow,
    ) -> Result<GrokBuildCredentialVersion, GrokBuildCredentialPersistenceError> {
        let revision = u64::try_from(row.revision)
            .map_err(|_| GrokBuildCredentialPersistenceError::InvalidPersistedState)?;
        let key_version = KeyVersion::try_from_sqlite_i64(row.key_version)
            .map_err(|_| GrokBuildCredentialPersistenceError::InvalidPersistedState)?;
        let encrypted = EncryptedSecret::try_from_persisted(key_version, row.ciphertext)
            .map_err(|_| GrokBuildCredentialPersistenceError::InvalidPersistedState)?;
        let aad = credential_associated_data(key)?;
        let plaintext = self
            .secret_store
            .open(&encrypted, &aad)
            .map_err(|_| GrokBuildCredentialPersistenceError::SecretStoreFailure)?;
        let credential = GrokBuildCredential::from_persisted_bytes(plaintext.as_bytes())
            .map_err(|_| GrokBuildCredentialPersistenceError::InvalidPersistedState)?;
        Ok(GrokBuildCredentialVersion::new(credential, revision))
    }
}

impl GrokBuildCredentialPersistence for GrokBuildCredentialSqliteStore {
    fn load(
        &self,
        key: &GrokBuildCredentialKey,
    ) -> Result<Option<GrokBuildCredentialVersion>, GrokBuildCredentialPersistenceError> {
        self.load_row(key)?
            .map(|row| self.decode_row(key, row))
            .transpose()
    }

    fn insert_if_absent(
        &self,
        key: &GrokBuildCredentialKey,
        credential: &GrokBuildCredential,
        updated_at_ms: i64,
    ) -> Result<GrokBuildCredentialInsertOutcome, GrokBuildCredentialPersistenceError> {
        let encrypted = self.seal(key, credential)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildCredentialPersistenceError::StoreUnavailable)?;
        let rows = connection
            .execute(
                "INSERT INTO grok_build_credential_runtime (\
                    config_version_id, credential_id, revision, ciphertext, key_version, updated_at_ms\
                 ) VALUES (?1, ?2, 0, ?3, ?4, ?5) \
                 ON CONFLICT(config_version_id, credential_id) DO NOTHING",
                params![
                    key.config_version_id().as_str(),
                    key.credential_id().as_str(),
                    encrypted.ciphertext(),
                    encrypted.key_version().as_sqlite_i64(),
                    updated_at_ms,
                ],
            )
            .map_err(|_| GrokBuildCredentialPersistenceError::StoreUnavailable)?;
        drop(connection);
        if rows == 1 {
            return Ok(GrokBuildCredentialInsertOutcome::Inserted(
                GrokBuildCredentialVersion::new(credential.clone(), 0),
            ));
        }
        let existing = self
            .load(key)?
            .ok_or(GrokBuildCredentialPersistenceError::StoreUnavailable)?;
        Ok(GrokBuildCredentialInsertOutcome::Existing(existing))
    }

    fn compare_and_swap(
        &self,
        key: &GrokBuildCredentialKey,
        expected_revision: u64,
        credential: &GrokBuildCredential,
        updated_at_ms: i64,
    ) -> Result<GrokBuildCredentialCasOutcome, GrokBuildCredentialPersistenceError> {
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or(GrokBuildCredentialPersistenceError::RevisionOverflow)?;
        let expected_revision = i64::try_from(expected_revision)
            .map_err(|_| GrokBuildCredentialPersistenceError::RevisionOverflow)?;
        let next_revision_sql = i64::try_from(next_revision)
            .map_err(|_| GrokBuildCredentialPersistenceError::RevisionOverflow)?;
        let encrypted = self.seal(key, credential)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildCredentialPersistenceError::StoreUnavailable)?;
        let rows = connection
            .execute(
                "UPDATE grok_build_credential_runtime SET \
                    revision = ?1, ciphertext = ?2, key_version = ?3, updated_at_ms = ?4 \
                 WHERE config_version_id = ?5 AND credential_id = ?6 AND revision = ?7",
                params![
                    next_revision_sql,
                    encrypted.ciphertext(),
                    encrypted.key_version().as_sqlite_i64(),
                    updated_at_ms,
                    key.config_version_id().as_str(),
                    key.credential_id().as_str(),
                    expected_revision,
                ],
            )
            .map_err(|_| GrokBuildCredentialPersistenceError::StoreUnavailable)?;
        drop(connection);
        if rows == 1 {
            return Ok(GrokBuildCredentialCasOutcome::Committed(
                GrokBuildCredentialVersion::new(credential.clone(), next_revision),
            ));
        }
        match self.load_row(key)? {
            Some(_) => Ok(GrokBuildCredentialCasOutcome::Conflict),
            None => Ok(GrokBuildCredentialCasOutcome::Missing),
        }
    }
}

impl fmt::Debug for GrokBuildCredentialSqliteStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildCredentialSqliteStore")
            .field("connection", &"<redacted>")
            .field("secret_store", &self.secret_store)
            .finish()
    }
}

/// Result of checking or refreshing one durable OAuth credential.
#[derive(Clone, Debug)]
pub enum GrokBuildCredentialRefreshOutcome {
    /// The stored credential remains valid at the requested instant.
    Current(GrokBuildCredentialVersion),
    /// This caller refreshed and atomically committed the next revision.
    Refreshed(GrokBuildCredentialVersion),
    /// Another writer won the CAS race; this caller loaded the newer state without overwriting it.
    Superseded(GrokBuildCredentialVersion),
}

impl GrokBuildCredentialRefreshOutcome {
    /// Returns the credential version that is safe to use after this operation.
    #[must_use]
    pub const fn version(&self) -> &GrokBuildCredentialVersion {
        match self {
            Self::Current(version) | Self::Refreshed(version) | Self::Superseded(version) => {
                version
            }
        }
    }
}

/// Safe error from singleflight refresh coordination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildCredentialRefreshError {
    /// No durable state exists at the exact requested identity.
    MissingCredentialState,
    /// Persistent credential handling failed without exposing token/ciphertext material.
    Persistence(GrokBuildCredentialPersistenceError),
    /// The OAuth refresh protocol or injected transport failed safely.
    OAuth(GrokBuildOAuthError),
    /// Another same-Credential refresh did not complete within the configured bounded wait.
    RefreshLockTimedOut,
    /// A concurrent durable writer won with a Credential that is already expired at this request.
    ///
    /// Callers must reload/retry through the coordinator instead of treating this as a transport
    /// failure or using the expired winner.
    ConcurrentCredentialStateChanged,
}

impl fmt::Display for GrokBuildCredentialRefreshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCredentialState => {
                formatter.write_str("Grok Build credential state is unavailable")
            }
            Self::Persistence(error) => write!(
                formatter,
                "Grok Build credential persistence failed: {error}"
            ),
            Self::OAuth(error) => {
                write!(formatter, "Grok Build credential refresh failed: {error}")
            }
            Self::RefreshLockTimedOut => {
                formatter.write_str("Grok Build credential refresh coordination timed out")
            }
            Self::ConcurrentCredentialStateChanged => {
                formatter.write_str("Grok Build credential state changed during refresh")
            }
        }
    }
}

impl Error for GrokBuildCredentialRefreshError {}

/// Configuration error for [`GrokBuildCredentialRefreshCoordinator`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildCredentialRefreshCoordinatorConfigError {
    /// A zero wait would turn ordinary same-key coordination into an avoidable refresh storm.
    ZeroRefreshWaitTimeout,
}

impl fmt::Display for GrokBuildCredentialRefreshCoordinatorConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroRefreshWaitTimeout => {
                formatter.write_str("Grok Build credential refresh wait timeout must be positive")
            }
        }
    }
}

impl Error for GrokBuildCredentialRefreshCoordinatorConfigError {}

/// Per-credential refresh coordinator with bounded same-key singleflight.
pub struct GrokBuildCredentialRefreshCoordinator<P> {
    persistence: Arc<P>,
    slots: Mutex<BTreeMap<GrokBuildCredentialKey, Arc<RefreshSlot>>>,
    refresh_wait_timeout: Duration,
}

impl<P> GrokBuildCredentialRefreshCoordinator<P>
where
    P: GrokBuildCredentialPersistence,
{
    /// Creates one coordinator around a durable state port.
    #[must_use]
    pub fn new(persistence: Arc<P>) -> Self {
        Self {
            persistence,
            slots: Mutex::new(BTreeMap::new()),
            refresh_wait_timeout: DEFAULT_GROK_BUILD_REFRESH_WAIT_TIMEOUT,
        }
    }

    /// Creates a coordinator with an explicit bounded same-key wait duration.
    ///
    /// # Errors
    ///
    /// Returns a configuration error for a zero duration, which would turn a coordinated wait
    /// into a refresh storm under concurrent expiry.
    pub fn try_new_with_wait_timeout(
        persistence: Arc<P>,
        refresh_wait_timeout: Duration,
    ) -> Result<Self, GrokBuildCredentialRefreshCoordinatorConfigError> {
        if refresh_wait_timeout.is_zero() {
            return Err(GrokBuildCredentialRefreshCoordinatorConfigError::ZeroRefreshWaitTimeout);
        }
        Ok(Self {
            persistence,
            slots: Mutex::new(BTreeMap::new()),
            refresh_wait_timeout,
        })
    }

    /// Writes a Device/imported credential only if no durable state exists at its exact key.
    ///
    /// # Errors
    ///
    /// Returns a safe persistence error and never replaces an already stored newer credential.
    pub fn initialize(
        &self,
        key: &GrokBuildCredentialKey,
        credential: &GrokBuildCredential,
        updated_at_ms: i64,
    ) -> Result<GrokBuildCredentialInsertOutcome, GrokBuildCredentialPersistenceError> {
        self.persistence
            .insert_if_absent(key, credential, updated_at_ms)
    }

    /// Returns a current credential or executes exactly one same-key OAuth refresh flight.
    ///
    /// The coordinator holds no global refresh lock. A caller seeing an already active same-key
    /// flight waits for that result; after a successful winner it loads the new durable revision,
    /// and after a failed winner it receives the same safe error rather than immediately starting a
    /// second refresh storm.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildCredentialRefreshError`] for a missing state, persistence failure, or
    /// safe OAuth failure. It never returns a partially refreshed credential.
    pub fn refresh_if_expired<T: GrokBuildOAuthTransport>(
        &self,
        key: &GrokBuildCredentialKey,
        flow: &GrokBuildOAuthFlow,
        transport: &T,
        now_ms: i64,
    ) -> Result<GrokBuildCredentialRefreshOutcome, GrokBuildCredentialRefreshError> {
        let slot = self.slot(key)?;
        loop {
            let mut state = slot.state.lock().map_err(|_| {
                GrokBuildCredentialRefreshError::Persistence(
                    GrokBuildCredentialPersistenceError::StoreUnavailable,
                )
            })?;
            let observed_generation = state.generation;
            if state.refreshing {
                let wait_started = Instant::now();
                while state.refreshing {
                    let remaining = self
                        .refresh_wait_timeout
                        .checked_sub(wait_started.elapsed())
                        .unwrap_or(Duration::ZERO);
                    if remaining.is_zero() {
                        return Err(GrokBuildCredentialRefreshError::RefreshLockTimedOut);
                    }
                    let (waited_state, timeout) =
                        slot.ready.wait_timeout(state, remaining).map_err(|_| {
                            GrokBuildCredentialRefreshError::Persistence(
                                GrokBuildCredentialPersistenceError::StoreUnavailable,
                            )
                        })?;
                    state = waited_state;
                    if timeout.timed_out() && state.refreshing {
                        return Err(GrokBuildCredentialRefreshError::RefreshLockTimedOut);
                    }
                }
                if state.generation != observed_generation {
                    if let Some(error) = state.last_error {
                        return Err(error);
                    }
                    drop(state);
                    let current = self.load_required(key)?;
                    if !current.credential().is_expired_at(now_ms) {
                        return Ok(GrokBuildCredentialRefreshOutcome::Current(current));
                    }
                    // A different process may have won the CAS with a still-expired Credential.
                    // Re-enter leader election rather than pretending a local transport failed.
                    continue;
                }
            }

            let current = self.load_required(key)?;
            if !current.credential().is_expired_at(now_ms) {
                return Ok(GrokBuildCredentialRefreshOutcome::Current(current));
            }
            state.refreshing = true;
            state.last_error = None;
            drop(state);

            let result = self.refresh_once(key, flow, transport, now_ms, &current);
            let mut state = slot.state.lock().map_err(|_| {
                GrokBuildCredentialRefreshError::Persistence(
                    GrokBuildCredentialPersistenceError::StoreUnavailable,
                )
            })?;
            state.refreshing = false;
            state.generation = state.generation.wrapping_add(1);
            state.last_error = result.as_ref().err().copied();
            slot.ready.notify_all();
            return result;
        }
    }

    fn refresh_once<T: GrokBuildOAuthTransport>(
        &self,
        key: &GrokBuildCredentialKey,
        flow: &GrokBuildOAuthFlow,
        transport: &T,
        now_ms: i64,
        current: &GrokBuildCredentialVersion,
    ) -> Result<GrokBuildCredentialRefreshOutcome, GrokBuildCredentialRefreshError> {
        let refreshed = flow
            .refresh(transport, current.credential(), now_ms)
            .map_err(GrokBuildCredentialRefreshError::OAuth)?;
        match self
            .persistence
            .compare_and_swap(key, current.revision(), &refreshed, now_ms)
            .map_err(GrokBuildCredentialRefreshError::Persistence)?
        {
            GrokBuildCredentialCasOutcome::Committed(version) => {
                Ok(GrokBuildCredentialRefreshOutcome::Refreshed(version))
            }
            GrokBuildCredentialCasOutcome::Conflict | GrokBuildCredentialCasOutcome::Missing => {
                let newer = self.load_required(key)?;
                if newer.credential().is_expired_at(now_ms) {
                    return Err(GrokBuildCredentialRefreshError::ConcurrentCredentialStateChanged);
                }
                Ok(GrokBuildCredentialRefreshOutcome::Superseded(newer))
            }
        }
    }

    fn load_required(
        &self,
        key: &GrokBuildCredentialKey,
    ) -> Result<GrokBuildCredentialVersion, GrokBuildCredentialRefreshError> {
        self.persistence
            .load(key)
            .map_err(GrokBuildCredentialRefreshError::Persistence)?
            .ok_or(GrokBuildCredentialRefreshError::MissingCredentialState)
    }

    fn slot(
        &self,
        key: &GrokBuildCredentialKey,
    ) -> Result<Arc<RefreshSlot>, GrokBuildCredentialRefreshError> {
        let mut slots = self.slots.lock().map_err(|_| {
            GrokBuildCredentialRefreshError::Persistence(
                GrokBuildCredentialPersistenceError::StoreUnavailable,
            )
        })?;
        Ok(Arc::clone(
            slots
                .entry(key.clone())
                .or_insert_with(|| Arc::new(RefreshSlot::default())),
        ))
    }
}

impl<P> fmt::Debug for GrokBuildCredentialRefreshCoordinator<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokBuildCredentialRefreshCoordinator(<state redacted>)")
    }
}

#[derive(Default)]
struct RefreshSlot {
    state: Mutex<RefreshSlotState>,
    ready: Condvar,
}

#[derive(Default)]
struct RefreshSlotState {
    refreshing: bool,
    generation: u64,
    last_error: Option<GrokBuildCredentialRefreshError>,
}

struct SqliteCredentialRow {
    revision: i64,
    ciphertext: Vec<u8>,
    key_version: i64,
}

fn credential_associated_data(
    key: &GrokBuildCredentialKey,
) -> Result<Vec<u8>, GrokBuildCredentialPersistenceError> {
    let mut aad = Vec::from(CREDENTIAL_RUNTIME_AAD_DOMAIN);
    append_aad_segment(&mut aad, key.config_version_id().as_str())?;
    append_aad_segment(&mut aad, key.credential_id().as_str())?;
    Ok(aad)
}

fn append_aad_segment(
    output: &mut Vec<u8>,
    value: &str,
) -> Result<(), GrokBuildCredentialPersistenceError> {
    let length = u32::try_from(value.len())
        .map_err(|_| GrokBuildCredentialPersistenceError::InvalidAssociatedData)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn is_runtime_identity_component(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_RUNTIME_ID_BYTES
}
