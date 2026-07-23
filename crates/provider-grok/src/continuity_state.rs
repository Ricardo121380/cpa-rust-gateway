//! Grok Build cache-affinity, response-ownership, and encrypted reasoning-replay state.
//!
//! The three state kinds use independent tables and exact client-key namespaces. In particular,
//! an owned response never falls through to a different credential, and a replay payload is always
//! AEAD-sealed with all of its tenancy and model identity as associated data.

use std::{error::Error, fmt, path::Path, sync::Mutex};

use gateway_core::{ClientKeyId, CredentialId, EgressPolicyId, ResponseId};
use gateway_store::secret_store::{EncryptedSecret, KeyVersion, SecretStore};
use hmac::{Hmac, Mac};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

const GROK_BUILD_PROVIDER_ID: &str = "grok.build";
const GROK_BUILD_REPLAY_SIGNATURE: &str = "grok-build-responses-v1";
const MAX_DURABLE_IDENTIFIER_BYTES: usize = 128;
const MAX_CONTINUITY_IDENTIFIER_BYTES: usize = 512;
const MAX_REPLAY_BYTES: usize = 64 * 1024;
const MAX_ASSOCIATED_DATA_BYTES: usize = 2048;
const CACHE_IDENTITY_DOMAIN: &[u8] = b"grok-build-cache-identity:v1\0";
type PersistedReplayRow = (String, KeyVersion, Vec<u8>, i64);
type HmacSha256 = Hmac<Sha256>;

/// Why one cache affinity is preferred for a Build request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildAffinityReason {
    /// The upstream prompt-cache identity benefits from the selected credential.
    PromptCache,
    /// The upstream explicitly requested continuity on this credential.
    ServerRequested,
    /// A prior owned response requires its credential for safe continuation.
    ResponseContinuation,
}

impl GrokBuildAffinityReason {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::PromptCache => "prompt_cache",
            Self::ServerRequested => "server_requested",
            Self::ResponseContinuation => "response_continuation",
        }
    }

    fn from_sql(value: &str) -> Result<Self, GrokBuildContinuityError> {
        match value {
            "prompt_cache" => Ok(Self::PromptCache),
            "server_requested" => Ok(Self::ServerRequested),
            "response_continuation" => Ok(Self::ResponseContinuation),
            _ => Err(GrokBuildContinuityError::InvalidPersistedState),
        }
    }
}

/// The cause and loss estimate recorded when an affinity changes credentials.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildAffinityBreakReason {
    /// The previous affinity had expired.
    Expired,
    /// The previous credential is no longer schedulable.
    CredentialUnavailable,
    /// The required egress identity changed.
    EgressChanged,
    /// An explicit operator-controlled rebind was requested.
    OperatorRebind,
}

impl GrokBuildAffinityBreakReason {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Expired => "expired",
            Self::CredentialUnavailable => "credential_unavailable",
            Self::EgressChanged => "egress_changed",
            Self::OperatorRebind => "operator_rebind",
        }
    }
}

/// A versioned, tenant-isolated opaque cache identity safe to send to the Build upstream.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct GrokBuildCacheIdentity(String);

impl GrokBuildCacheIdentity {
    /// Returns the derived opaque identity only to the immediate Build request builder or store.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GrokBuildCacheIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokBuildCacheIdentity(<redacted>)")
    }
}

/// In-memory key material for deriving versioned Grok Build cache identities.
pub struct GrokBuildCacheIdentityDeriver {
    tenant_secret: Zeroizing<[u8; 32]>,
}

impl GrokBuildCacheIdentityDeriver {
    /// Creates a cache-identity deriver from caller-owned per-tenant secret material.
    #[must_use]
    pub fn new(tenant_secret: [u8; 32]) -> Self {
        Self {
            tenant_secret: Zeroizing::new(tenant_secret),
        }
    }

    /// Derives a stable HMAC identity without sending raw client or prompt-cache values upstream.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` when a bound identifier is blank, oversized, or contains a NUL byte.
    pub fn derive(
        &self,
        client_key_id: &ClientKeyId,
        upstream_model: &str,
        prompt_cache_key: &str,
    ) -> Result<GrokBuildCacheIdentity, GrokBuildContinuityError> {
        validate_durable_identifier(client_key_id.as_str())?;
        let client_key_id = client_key_id.as_str();
        let upstream_model = validate_identifier(upstream_model.to_owned())?;
        let prompt_cache_key = validate_identifier(prompt_cache_key.to_owned())?;
        let mut mac = HmacSha256::new_from_slice(self.tenant_secret.as_slice())
            .map_err(|_| GrokBuildContinuityError::InvalidState)?;
        mac.update(CACHE_IDENTITY_DOMAIN);
        for field in [
            client_key_id,
            upstream_model.as_str(),
            prompt_cache_key.as_str(),
        ] {
            let length =
                u16::try_from(field.len()).map_err(|_| GrokBuildContinuityError::InvalidState)?;
            mac.update(&length.to_be_bytes());
            mac.update(field.as_bytes());
        }
        let digest = mac.finalize().into_bytes();
        let mut identity = String::from("grok-build-cache:v1:");
        for byte in digest {
            append_hex_byte(&mut identity, byte);
        }
        Ok(GrokBuildCacheIdentity(identity))
    }
}

impl fmt::Debug for GrokBuildCacheIdentityDeriver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokBuildCacheIdentityDeriver(<redacted>)")
    }
}

/// One tenant/provider/model/cache-identity key for cache affinity.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct GrokBuildCacheAffinityKey {
    client_key_id: ClientKeyId,
    upstream_model: String,
    cache_identity: GrokBuildCacheIdentity,
}

impl GrokBuildCacheAffinityKey {
    /// Creates a Build-only affinity key from a derived, tenant-isolated cache identity.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` when the model is invalid.
    pub fn try_new(
        client_key_id: ClientKeyId,
        upstream_model: impl Into<String>,
        cache_identity: GrokBuildCacheIdentity,
    ) -> Result<Self, GrokBuildContinuityError> {
        validate_durable_identifier(client_key_id.as_str())?;
        Ok(Self {
            client_key_id,
            upstream_model: validate_identifier(upstream_model.into())?,
            cache_identity,
        })
    }

    /// Returns the owning client-key namespace.
    #[must_use]
    pub const fn client_key_id(&self) -> &ClientKeyId {
        &self.client_key_id
    }
}

fn append_hex_byte(output: &mut String, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push(char::from(HEX[usize::from(byte >> 4)]));
    output.push(char::from(HEX[usize::from(byte & 0x0f)]));
}

impl fmt::Debug for GrokBuildCacheAffinityKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildCacheAffinityKey")
            .field("client_key_id", &self.client_key_id)
            .field("upstream_model", &"<redacted>")
            .field("cache_identity", &"<redacted>")
            .finish()
    }
}

/// The preferred credential/egress pair for one stable Build cache identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokBuildCacheAffinity {
    credential_id: CredentialId,
    egress_policy_id: Option<EgressPolicyId>,
    expires_at_ms: i64,
    reason: GrokBuildAffinityReason,
}

impl GrokBuildCacheAffinity {
    /// Creates an affinity that remains valid only before the explicit expiry instant.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` when the expiry is not positive.
    pub fn try_new(
        credential_id: CredentialId,
        egress_policy_id: Option<EgressPolicyId>,
        expires_at_ms: i64,
        reason: GrokBuildAffinityReason,
    ) -> Result<Self, GrokBuildContinuityError> {
        validate_durable_identifier(credential_id.as_str())?;
        if let Some(egress_policy_id) = &egress_policy_id {
            validate_durable_identifier(egress_policy_id.as_str())?;
        }
        if expires_at_ms <= 0 {
            return Err(GrokBuildContinuityError::InvalidState);
        }
        Ok(Self {
            credential_id,
            egress_policy_id,
            expires_at_ms,
            reason,
        })
    }

    /// Returns the preferred credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the optional required egress-policy identity.
    #[must_use]
    pub const fn egress_policy_id(&self) -> Option<&EgressPolicyId> {
        self.egress_policy_id.as_ref()
    }

    /// Returns the affinity expiry instant.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }

    /// Returns why the affinity exists.
    #[must_use]
    pub const fn reason(&self) -> GrokBuildAffinityReason {
        self.reason
    }
}

/// Required metadata for a credential-changing affinity update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrokBuildAffinityBreakInput {
    reason: GrokBuildAffinityBreakReason,
    estimated_cache_loss_tokens: u64,
    occurred_at_ms: i64,
}

impl GrokBuildAffinityBreakInput {
    /// Creates a bounded and timestamped cache-loss observation.
    #[must_use]
    pub const fn new(
        reason: GrokBuildAffinityBreakReason,
        estimated_cache_loss_tokens: u64,
        occurred_at_ms: i64,
    ) -> Self {
        Self {
            reason,
            estimated_cache_loss_tokens,
            occurred_at_ms,
        }
    }
}

/// A durable record of a deliberate credential-changing affinity break.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokBuildAffinityBreak {
    prior_credential_id: CredentialId,
    next_credential_id: CredentialId,
    reason: GrokBuildAffinityBreakReason,
    estimated_cache_loss_tokens: u64,
    occurred_at_ms: i64,
}

impl GrokBuildAffinityBreak {
    /// Returns the credential that lost affinity.
    #[must_use]
    pub const fn prior_credential_id(&self) -> &CredentialId {
        &self.prior_credential_id
    }

    /// Returns the credential that replaces the previous affinity.
    #[must_use]
    pub const fn next_credential_id(&self) -> &CredentialId {
        &self.next_credential_id
    }

    /// Returns the bounded cache-loss estimate.
    #[must_use]
    pub const fn estimated_cache_loss_tokens(&self) -> u64 {
        self.estimated_cache_loss_tokens
    }
}

/// Safe outcome of binding a cache affinity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GrokBuildAffinityBindOutcome {
    /// No previous valid affinity existed.
    Bound,
    /// The existing credential/egress pair was retained and its expiry was refreshed.
    Refreshed,
    /// A caller supplied required break evidence and rebound to a new credential.
    Rebound(GrokBuildAffinityBreak),
}

/// An upstream response ID retained only for exact continuation ownership.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct GrokBuildUpstreamResponseId(String);

impl GrokBuildUpstreamResponseId {
    /// Creates a bounded opaque upstream response identity.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` when the response identity is invalid.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GrokBuildContinuityError> {
        Ok(Self(validate_identifier(value.into())?))
    }

    /// Returns the value only to the immediate Build continuation caller.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GrokBuildUpstreamResponseId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokBuildUpstreamResponseId(<redacted>)")
    }
}

/// The exact credential that owns one downstream response continuation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokBuildResponseOwnership {
    credential_id: CredentialId,
    upstream_response_id: GrokBuildUpstreamResponseId,
    expires_at_ms: i64,
}

impl GrokBuildResponseOwnership {
    /// Creates one exact owned response mapping.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` when the expiry is not positive.
    pub fn try_new(
        credential_id: CredentialId,
        upstream_response_id: GrokBuildUpstreamResponseId,
        expires_at_ms: i64,
    ) -> Result<Self, GrokBuildContinuityError> {
        validate_durable_identifier(credential_id.as_str())?;
        if expires_at_ms <= 0 {
            return Err(GrokBuildContinuityError::InvalidState);
        }
        Ok(Self {
            credential_id,
            upstream_response_id,
            expires_at_ms,
        })
    }

    /// Returns the only credential allowed to continue this response.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the upstream response ID for the owning credential.
    #[must_use]
    pub const fn upstream_response_id(&self) -> &GrokBuildUpstreamResponseId {
        &self.upstream_response_id
    }
}

/// A per-client/model/session key for encrypted Build reasoning replay.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct GrokBuildReplayKey {
    client_key_id: ClientKeyId,
    upstream_model: String,
    session_id: String,
}

impl GrokBuildReplayKey {
    /// Creates an exact replay namespace without exposing any field in diagnostics.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` when the model or session identity is invalid.
    pub fn try_new(
        client_key_id: ClientKeyId,
        upstream_model: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Result<Self, GrokBuildContinuityError> {
        validate_durable_identifier(client_key_id.as_str())?;
        Ok(Self {
            client_key_id,
            upstream_model: validate_identifier(upstream_model.into())?,
            session_id: validate_identifier(session_id.into())?,
        })
    }
}

impl fmt::Debug for GrokBuildReplayKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildReplayKey")
            .field("client_key_id", &self.client_key_id)
            .field("upstream_model", &"<redacted>")
            .field("session_id", &"<redacted>")
            .finish()
    }
}

/// A zeroizing, provider-signature-validated replay payload.
pub struct GrokBuildReasoningReplay {
    payload: Zeroizing<Vec<u8>>,
}

impl GrokBuildReasoningReplay {
    /// Validates a signed Grok Build replay payload before it can be persisted.
    ///
    /// # Errors
    ///
    /// Returns `InvalidReplayPayload` for an empty or oversized payload.
    pub fn try_new(payload: impl Into<Vec<u8>>) -> Result<Self, GrokBuildContinuityError> {
        let payload = payload.into();
        if payload.is_empty() || payload.len() > MAX_REPLAY_BYTES {
            return Err(GrokBuildContinuityError::InvalidReplayPayload);
        }
        Ok(Self {
            payload: Zeroizing::new(payload),
        })
    }

    /// Returns the payload only to the immediate Provider request builder.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.payload.as_slice()
    }
}

impl fmt::Debug for GrokBuildReasoningReplay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildReasoningReplay")
            .field("payload", &"<redacted>")
            .field("length", &self.payload.len())
            .finish()
    }
}

/// Safe outcome of writing one replay payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildReplayWriteOutcome {
    /// A new encrypted replay record was inserted.
    Inserted,
    /// An identical replay payload was already stored and was not re-encrypted.
    Deduplicated,
    /// A distinct signed replay payload replaced an older payload in the same exact namespace.
    Replaced,
}

/// Durable Build continuity state using the P6-05/P6-06 tables.
pub struct GrokBuildContinuityStore {
    connection: Mutex<Connection>,
    secret_store: SecretStore,
}

impl GrokBuildContinuityStore {
    /// Opens and migrates the Build continuity state store without exposing its path.
    ///
    /// # Errors
    ///
    /// Returns `StoreUnavailable` when the database cannot be safely opened or migrated.
    pub fn open(
        path: impl AsRef<Path>,
        secret_store: SecretStore,
    ) -> Result<Self, GrokBuildContinuityError> {
        let mut connection =
            gateway_store::open(path).map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        gateway_store::migrate(&mut connection)
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        Ok(Self {
            connection: Mutex::new(connection),
            secret_store,
        })
    }

    /// Opens a migrated in-memory Build continuity store for deterministic tests.
    ///
    /// # Errors
    ///
    /// Returns `StoreUnavailable` when the in-memory database cannot be safely migrated.
    pub fn open_in_memory(secret_store: SecretStore) -> Result<Self, GrokBuildContinuityError> {
        let mut connection = gateway_store::open_in_memory()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        gateway_store::migrate(&mut connection)
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        Ok(Self {
            connection: Mutex::new(connection),
            secret_store,
        })
    }

    /// Resolves an unexpired affinity for its exact client/model/cache key.
    ///
    /// # Errors
    ///
    /// Returns `InvalidPersistedState` for a malformed affinity or `StoreUnavailable` for a
    /// database failure.
    pub fn cache_affinity(
        &self,
        key: &GrokBuildCacheAffinityKey,
        now_ms: i64,
    ) -> Result<Option<GrokBuildCacheAffinity>, GrokBuildContinuityError> {
        if now_ms <= 0 {
            return Err(GrokBuildContinuityError::InvalidState);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        let row = connection
            .query_row(
                "SELECT credential_id, egress_policy_id, expires_at_ms, reason \
                 FROM grok_build_cache_affinities WHERE client_key_id = ?1 AND provider_id = ?2 \
                 AND upstream_model = ?3 AND cache_identity = ?4",
                params![
                    key.client_key_id.as_str(),
                    GROK_BUILD_PROVIDER_ID,
                    key.upstream_model,
                    key.cache_identity.as_str(),
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        let Some((credential_id, egress_policy_id, expires_at_ms, reason)) = row else {
            return Ok(None);
        };
        if expires_at_ms <= now_ms {
            return Ok(None);
        }
        GrokBuildCacheAffinity::try_new(
            CredentialId::try_new(credential_id)
                .map_err(|_| GrokBuildContinuityError::InvalidPersistedState)?,
            egress_policy_id
                .map(EgressPolicyId::try_new)
                .transpose()
                .map_err(|_| GrokBuildContinuityError::InvalidPersistedState)?,
            expires_at_ms,
            GrokBuildAffinityReason::from_sql(&reason)?,
        )
        .map(Some)
    }

    /// Binds an affinity, requiring explicit break evidence before replacing an active credential.
    ///
    /// # Errors
    ///
    /// Returns `AffinityBreakRequired` for a credential or egress replacement without durable
    /// evidence, `InvalidState` for invalid times, or `StoreUnavailable` if the atomic update fails.
    pub fn bind_cache_affinity(
        &self,
        key: &GrokBuildCacheAffinityKey,
        affinity: &GrokBuildCacheAffinity,
        now_ms: i64,
        break_input: Option<GrokBuildAffinityBreakInput>,
    ) -> Result<GrokBuildAffinityBindOutcome, GrokBuildContinuityError> {
        if now_ms <= 0 || affinity.expires_at_ms <= now_ms {
            return Err(GrokBuildContinuityError::InvalidState);
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        let existing = transaction
            .query_row(
                "SELECT credential_id, egress_policy_id, expires_at_ms, reason \
                 FROM grok_build_cache_affinities WHERE client_key_id = ?1 AND provider_id = ?2 \
                 AND upstream_model = ?3 AND cache_identity = ?4",
                params![
                    key.client_key_id.as_str(),
                    GROK_BUILD_PROVIDER_ID,
                    key.upstream_model,
                    key.cache_identity.as_str(),
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?
            .map(|(credential_id, egress_policy_id, expires_at_ms, reason)| {
                GrokBuildCacheAffinity::try_new(
                    CredentialId::try_new(credential_id)
                        .map_err(|_| GrokBuildContinuityError::InvalidPersistedState)?,
                    egress_policy_id
                        .map(EgressPolicyId::try_new)
                        .transpose()
                        .map_err(|_| GrokBuildContinuityError::InvalidPersistedState)?,
                    expires_at_ms,
                    GrokBuildAffinityReason::from_sql(&reason)?,
                )
            })
            .transpose()?;
        let outcome = match existing {
            None => GrokBuildAffinityBindOutcome::Bound,
            Some(existing)
                if existing.credential_id == affinity.credential_id
                    && existing.egress_policy_id == affinity.egress_policy_id =>
            {
                GrokBuildAffinityBindOutcome::Refreshed
            }
            Some(existing) => {
                let input = break_input.ok_or(GrokBuildContinuityError::AffinityBreakRequired)?;
                if input.occurred_at_ms <= 0 || input.occurred_at_ms > now_ms {
                    return Err(GrokBuildContinuityError::InvalidState);
                }
                let break_record = GrokBuildAffinityBreak {
                    prior_credential_id: existing.credential_id,
                    next_credential_id: affinity.credential_id.clone(),
                    reason: input.reason,
                    estimated_cache_loss_tokens: input.estimated_cache_loss_tokens,
                    occurred_at_ms: input.occurred_at_ms,
                };
                Self::record_affinity_break(&transaction, key, &break_record)?;
                GrokBuildAffinityBindOutcome::Rebound(break_record)
            }
        };
        transaction
            .execute(
                "INSERT INTO grok_build_cache_affinities \
                 (client_key_id, provider_id, upstream_model, cache_identity, credential_id, \
                  egress_policy_id, expires_at_ms, reason, updated_at_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                 ON CONFLICT(client_key_id, provider_id, upstream_model, cache_identity) DO UPDATE SET \
                   credential_id = excluded.credential_id, egress_policy_id = excluded.egress_policy_id, \
                   expires_at_ms = excluded.expires_at_ms, reason = excluded.reason, \
                   updated_at_ms = excluded.updated_at_ms",
                params![
                    key.client_key_id.as_str(),
                    GROK_BUILD_PROVIDER_ID,
                    key.upstream_model,
                    key.cache_identity.as_str(),
                    affinity.credential_id.as_str(),
                    affinity.egress_policy_id.as_ref().map(EgressPolicyId::as_str),
                    affinity.expires_at_ms,
                    affinity.reason.as_sql(),
                    now_ms,
                ],
            )
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        transaction
            .commit()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        Ok(outcome)
    }

    /// Records one intentional cache-affinity break before a replacement is committed.
    fn record_affinity_break(
        transaction: &Transaction<'_>,
        key: &GrokBuildCacheAffinityKey,
        break_record: &GrokBuildAffinityBreak,
    ) -> Result<(), GrokBuildContinuityError> {
        transaction
            .execute(
                "INSERT INTO grok_build_affinity_breaks \
                 (client_key_id, upstream_model, cache_identity, prior_credential_id, \
                  next_credential_id, reason, estimated_cache_loss_tokens, occurred_at_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    key.client_key_id.as_str(),
                    key.upstream_model,
                    key.cache_identity.as_str(),
                    break_record.prior_credential_id.as_str(),
                    break_record.next_credential_id.as_str(),
                    break_record.reason.as_sql(),
                    i64::try_from(break_record.estimated_cache_loss_tokens)
                        .map_err(|_| GrokBuildContinuityError::InvalidState)?,
                    break_record.occurred_at_ms,
                ],
            )
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        Ok(())
    }

    /// Records response ownership once; a conflicting owner cannot silently overwrite it.
    ///
    /// # Errors
    ///
    /// Returns `OwnershipConflict` when the exact owner differs or `StoreUnavailable` for a
    /// database failure.
    pub fn record_response_ownership(
        &self,
        client_key_id: &ClientKeyId,
        downstream_response_id: &ResponseId,
        ownership: &GrokBuildResponseOwnership,
        created_at_ms: i64,
    ) -> Result<(), GrokBuildContinuityError> {
        validate_durable_identifier(client_key_id.as_str())?;
        validate_identifier(downstream_response_id.as_str().to_owned())?;
        if created_at_ms <= 0 || ownership.expires_at_ms <= created_at_ms {
            return Err(GrokBuildContinuityError::InvalidState);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        let inserted = connection
            .execute(
                "INSERT INTO grok_build_response_ownership \
                 (client_key_id, downstream_response_id, provider_id, credential_id, \
                  upstream_response_id, expires_at_ms, created_at_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT DO NOTHING",
                params![
                    client_key_id.as_str(),
                    downstream_response_id.as_str(),
                    GROK_BUILD_PROVIDER_ID,
                    ownership.credential_id.as_str(),
                    ownership.upstream_response_id.as_str(),
                    ownership.expires_at_ms,
                    created_at_ms,
                ],
            )
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        if inserted == 1 {
            return Ok(());
        }
        let existing: (String, String, i64) = connection
            .query_row(
                "SELECT credential_id, upstream_response_id, expires_at_ms \
                 FROM grok_build_response_ownership \
                 WHERE client_key_id = ?1 AND downstream_response_id = ?2",
                params![client_key_id.as_str(), downstream_response_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        if existing.0 == ownership.credential_id.as_str()
            && existing.1 == ownership.upstream_response_id.as_str()
            && existing.2 == ownership.expires_at_ms
        {
            Ok(())
        } else {
            Err(GrokBuildContinuityError::OwnershipConflict)
        }
    }

    /// Resolves the exact response owner and rejects a selected different credential.
    ///
    /// # Errors
    ///
    /// Returns an explicit missing, expired, or credential-mismatch error; malformed persisted
    /// state and database failures are also rejected without fallback.
    pub fn resolve_response_ownership(
        &self,
        client_key_id: &ClientKeyId,
        downstream_response_id: &ResponseId,
        selected_credential_id: &CredentialId,
        now_ms: i64,
    ) -> Result<GrokBuildResponseOwnership, GrokBuildContinuityError> {
        validate_durable_identifier(client_key_id.as_str())?;
        validate_durable_identifier(selected_credential_id.as_str())?;
        validate_identifier(downstream_response_id.as_str().to_owned())?;
        if now_ms <= 0 {
            return Err(GrokBuildContinuityError::InvalidState);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        let row = connection
            .query_row(
                "SELECT credential_id, upstream_response_id, expires_at_ms \
                 FROM grok_build_response_ownership WHERE client_key_id = ?1 \
                 AND downstream_response_id = ?2",
                params![client_key_id.as_str(), downstream_response_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?
            .ok_or(GrokBuildContinuityError::OwnershipMissing)?;
        let ownership = GrokBuildResponseOwnership::try_new(
            CredentialId::try_new(row.0)
                .map_err(|_| GrokBuildContinuityError::InvalidPersistedState)?,
            GrokBuildUpstreamResponseId::try_new(row.1)?,
            row.2,
        )?;
        if ownership.expires_at_ms <= now_ms {
            return Err(GrokBuildContinuityError::OwnershipExpired);
        }
        if ownership.credential_id != *selected_credential_id {
            return Err(GrokBuildContinuityError::OwnershipCredentialMismatch);
        }
        Ok(ownership)
    }

    /// Writes a signed replay payload using tenant/model/session-associated AEAD data.
    ///
    /// # Errors
    ///
    /// Returns a safe continuity error when the state is invalid, existing ciphertext cannot be
    /// authenticated, or the durable write cannot complete.
    pub fn write_reasoning_replay(
        &self,
        key: &GrokBuildReplayKey,
        replay: &GrokBuildReasoningReplay,
        expires_at_ms: i64,
        updated_at_ms: i64,
    ) -> Result<GrokBuildReplayWriteOutcome, GrokBuildContinuityError> {
        if updated_at_ms <= 0 || expires_at_ms <= updated_at_ms {
            return Err(GrokBuildContinuityError::InvalidState);
        }
        let associated_data = replay_associated_data(key)?;
        let existing = self.load_replay_row(key)?;
        if let Some((signature, key_version, ciphertext, existing_expiry)) = existing {
            if signature != GROK_BUILD_REPLAY_SIGNATURE {
                return Err(GrokBuildContinuityError::InvalidPersistedState);
            }
            let encrypted = EncryptedSecret::try_from_persisted(key_version, ciphertext)
                .map_err(|_| GrokBuildContinuityError::SecretStoreFailure)?;
            let existing_plaintext = self
                .secret_store
                .open(&encrypted, associated_data.as_slice())
                .map_err(|_| GrokBuildContinuityError::SecretStoreFailure)?;
            if existing_expiry > updated_at_ms && existing_plaintext.as_bytes() == replay.as_bytes()
            {
                return Ok(GrokBuildReplayWriteOutcome::Deduplicated);
            }
        }
        let encrypted = self
            .secret_store
            .seal(replay.as_bytes(), associated_data.as_slice())
            .map_err(|_| GrokBuildContinuityError::SecretStoreFailure)?;
        let outcome = if self.load_replay_row(key)?.is_some() {
            GrokBuildReplayWriteOutcome::Replaced
        } else {
            GrokBuildReplayWriteOutcome::Inserted
        };
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        connection
            .execute(
                "INSERT INTO grok_build_reasoning_replay \
                 (client_key_id, provider_id, upstream_model, session_id, signature, ciphertext, \
                  key_version, expires_at_ms, updated_at_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                 ON CONFLICT(client_key_id, provider_id, upstream_model, session_id) DO UPDATE SET \
                   signature = excluded.signature, ciphertext = excluded.ciphertext, \
                   key_version = excluded.key_version, expires_at_ms = excluded.expires_at_ms, \
                   updated_at_ms = excluded.updated_at_ms",
                params![
                    key.client_key_id.as_str(),
                    GROK_BUILD_PROVIDER_ID,
                    key.upstream_model,
                    key.session_id,
                    GROK_BUILD_REPLAY_SIGNATURE,
                    encrypted.ciphertext(),
                    encrypted.key_version().as_sqlite_i64(),
                    expires_at_ms,
                    updated_at_ms,
                ],
            )
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        Ok(outcome)
    }

    /// Loads a non-expired replay payload for its exact client/model/session namespace.
    ///
    /// # Errors
    ///
    /// Returns a safe continuity error when stored ciphertext cannot be authenticated or decoded.
    pub fn reasoning_replay(
        &self,
        key: &GrokBuildReplayKey,
        now_ms: i64,
    ) -> Result<Option<GrokBuildReasoningReplay>, GrokBuildContinuityError> {
        if now_ms <= 0 {
            return Err(GrokBuildContinuityError::InvalidState);
        }
        let Some((signature, key_version, ciphertext, expires_at_ms)) =
            self.load_replay_row(key)?
        else {
            return Ok(None);
        };
        if expires_at_ms <= now_ms {
            return Ok(None);
        }
        if signature != GROK_BUILD_REPLAY_SIGNATURE {
            return Err(GrokBuildContinuityError::InvalidPersistedState);
        }
        let associated_data = replay_associated_data(key)?;
        let encrypted = EncryptedSecret::try_from_persisted(key_version, ciphertext)
            .map_err(|_| GrokBuildContinuityError::SecretStoreFailure)?;
        let plaintext = self
            .secret_store
            .open(&encrypted, associated_data.as_slice())
            .map_err(|_| GrokBuildContinuityError::SecretStoreFailure)?;
        GrokBuildReasoningReplay::try_new(plaintext.as_bytes().to_vec()).map(Some)
    }

    /// Deletes replay only after a successful response explicitly confirms no replay state remains.
    ///
    /// # Errors
    ///
    /// Returns `StoreUnavailable` when the exact replay row cannot be deleted.
    pub fn clear_reasoning_replay(
        &self,
        key: &GrokBuildReplayKey,
    ) -> Result<(), GrokBuildContinuityError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        connection
            .execute(
                "DELETE FROM grok_build_reasoning_replay WHERE client_key_id = ?1 \
                 AND provider_id = ?2 AND upstream_model = ?3 AND session_id = ?4",
                params![
                    key.client_key_id.as_str(),
                    GROK_BUILD_PROVIDER_ID,
                    key.upstream_model,
                    key.session_id,
                ],
            )
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        Ok(())
    }

    fn load_replay_row(
        &self,
        key: &GrokBuildReplayKey,
    ) -> Result<Option<PersistedReplayRow>, GrokBuildContinuityError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?;
        connection
            .query_row(
                "SELECT signature, key_version, ciphertext, expires_at_ms \
                 FROM grok_build_reasoning_replay WHERE client_key_id = ?1 AND provider_id = ?2 \
                 AND upstream_model = ?3 AND session_id = ?4",
                params![
                    key.client_key_id.as_str(),
                    GROK_BUILD_PROVIDER_ID,
                    key.upstream_model,
                    key.session_id,
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| GrokBuildContinuityError::StoreUnavailable)?
            .map(|(signature, key_version, ciphertext, expires_at_ms)| {
                Ok((
                    signature,
                    KeyVersion::try_from_sqlite_i64(key_version)
                        .map_err(|_| GrokBuildContinuityError::InvalidPersistedState)?,
                    ciphertext,
                    expires_at_ms,
                ))
            })
            .transpose()
    }
}

impl fmt::Debug for GrokBuildContinuityStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildContinuityStore")
            .field("connection", &"<redacted>")
            .field("secret_store", &self.secret_store)
            .finish()
    }
}

/// Safe errors for P6 cache, ownership, and replay state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildContinuityError {
    /// The durable state store cannot be used safely.
    StoreUnavailable,
    /// An opaque key or expiry is invalid.
    InvalidState,
    /// A replay payload was absent, oversized, or not structurally eligible.
    InvalidReplayPayload,
    /// A persisted row failed strict structural validation.
    InvalidPersistedState,
    /// AEAD sealing/opening failed without releasing plaintext or ciphertext.
    SecretStoreFailure,
    /// Rebinding an active affinity omitted mandatory break evidence.
    AffinityBreakRequired,
    /// A downstream response already belongs to a different credential.
    OwnershipConflict,
    /// No ownership row exists for a required continuation.
    OwnershipMissing,
    /// The owned continuation mapping had expired.
    OwnershipExpired,
    /// The selected credential is not the response's exact owner.
    OwnershipCredentialMismatch,
}

impl fmt::Display for GrokBuildContinuityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::StoreUnavailable => "Grok Build continuity store is unavailable",
            Self::InvalidState => "Grok Build continuity state is invalid",
            Self::InvalidReplayPayload => "Grok Build reasoning replay payload is invalid",
            Self::InvalidPersistedState => "Grok Build persisted continuity state is invalid",
            Self::SecretStoreFailure => "Grok Build continuity encryption failed",
            Self::AffinityBreakRequired => "Grok Build affinity rebind requires break evidence",
            Self::OwnershipConflict => "Grok Build response ownership conflicts",
            Self::OwnershipMissing => "Grok Build response ownership is missing",
            Self::OwnershipExpired => "Grok Build response ownership expired",
            Self::OwnershipCredentialMismatch => {
                "Grok Build response owner differs from selected credential"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for GrokBuildContinuityError {}

fn validate_identifier(value: String) -> Result<String, GrokBuildContinuityError> {
    if value.trim().is_empty()
        || value.len() > MAX_CONTINUITY_IDENTIFIER_BYTES
        || value.bytes().any(|byte| byte == b'\0')
    {
        return Err(GrokBuildContinuityError::InvalidState);
    }
    Ok(value)
}

fn validate_durable_identifier(value: &str) -> Result<(), GrokBuildContinuityError> {
    if value.trim().is_empty()
        || value.len() > MAX_DURABLE_IDENTIFIER_BYTES
        || value.bytes().any(|byte| byte == b'\0')
    {
        return Err(GrokBuildContinuityError::InvalidState);
    }
    Ok(())
}

fn replay_associated_data(
    key: &GrokBuildReplayKey,
) -> Result<Zeroizing<Vec<u8>>, GrokBuildContinuityError> {
    let mut associated_data = Zeroizing::new(
        format!(
            "grok-build-replay:v1:{}:{}:{}:{}",
            key.client_key_id.as_str().len(),
            key.client_key_id.as_str(),
            key.upstream_model,
            key.session_id,
        )
        .into_bytes(),
    );
    if associated_data.is_empty() || associated_data.len() > MAX_ASSOCIATED_DATA_BYTES {
        associated_data.zeroize();
        return Err(GrokBuildContinuityError::InvalidState);
    }
    Ok(associated_data)
}
