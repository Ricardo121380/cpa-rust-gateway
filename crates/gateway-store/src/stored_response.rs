//! Client-key-owned, AEAD-sealed Canonical Responses.
//!
//! Response bodies, reasoning, tool arguments, usage, stop semantics, and Provider lineage are
//! serialized only inside an authenticated encrypted envelope. The cleartext index contains the
//! exact Client Key owner, downstream Response identifier, and bounded lifecycle timestamps needed
//! for ownership lookup and garbage collection. This store performs no Provider calls, routing,
//! retries, or cross-account fallback.

use std::{
    error::Error,
    fmt,
    path::Path,
    sync::{Mutex, MutexGuard},
};

use gateway_core::{
    CanonicalEvent, CanonicalRequest, CanonicalResponse, ClientKeyId, CredentialId, EndpointId,
    ProviderId, ResponseId, RouteCandidateId, RouteId, UpstreamId,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{
    StoreError, migrate, open, open_in_memory,
    secret_store::{EncryptedSecret, KeyVersion, SecretStore, SecretStoreError},
};

/// Fixed local retention window for an opt-in stored Response.
pub const STORED_RESPONSE_TTL_MILLISECONDS: i64 = 30 * 24 * 60 * 60 * 1_000;
/// Maximum canonical events retained in one successful Response.
pub const MAX_STORED_RESPONSE_EVENTS: usize = 4_096;
/// Maximum serialized plaintext admitted before AEAD sealing.
pub const MAX_STORED_RESPONSE_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
/// Maximum rows deleted by one bounded garbage-collection transaction.
pub const MAX_STORED_RESPONSE_GC_BATCH: usize = 4_096;
/// Maximum UTF-8 bytes retained in one gateway-generated compact summary.
pub const MAX_STORED_RESPONSE_COMPACTION_SUMMARY_BYTES: usize = 1024 * 1024;
/// Public prefix that distinguishes CPAR-owned compact locators from upstream blobs.
pub const STORED_RESPONSE_COMPACTION_PREFIX: &str = "cpar_compact_v1.";

const STORED_RESPONSE_PAYLOAD_VERSION: i64 = 1;
const STORED_RESPONSE_PAYLOAD_VERSION_U16: u16 = 1;
const MAX_DURABLE_IDENTIFIER_BYTES: usize = 128;
const MAX_RESPONSE_IDENTIFIER_BYTES: usize = 512;
const MAX_PUBLIC_MODEL_BYTES: usize = 512;
const AAD_DOMAIN: &[u8] = b"cpar:stored-response:v1\0";
const COMPACTION_AAD_DOMAIN: &[u8] = b"cpar:stored-response-compaction:v1\0";
const COMPACTION_PAYLOAD_VERSION: i64 = 1;
const COMPACTION_PAYLOAD_VERSION_U16: u16 = 1;
const COMPACTION_RANDOM_BYTES: usize = 16;
const COMPACTION_TOKEN_GENERATION_ATTEMPTS: usize = 4;

/// Provider/channel/route identity pinned to one stored response.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredResponseTarget {
    provider: ProviderId,
    upstream: UpstreamId,
    channel: EndpointId,
    route: RouteId,
    route_candidate: RouteCandidateId,
}

impl StoredResponseTarget {
    /// Creates one exact target after applying durable identifier bounds.
    ///
    /// # Errors
    ///
    /// Returns [`StoredResponseStoreError::InvalidInput`] when any opaque identifier is blank,
    /// oversized, or contains a NUL byte.
    pub fn try_new(
        provider_id: ProviderId,
        upstream_id: UpstreamId,
        channel_id: EndpointId,
        route_id: RouteId,
        route_candidate_id: RouteCandidateId,
    ) -> Result<Self, StoredResponseStoreError> {
        for value in [
            provider_id.as_str(),
            upstream_id.as_str(),
            channel_id.as_str(),
            route_id.as_str(),
            route_candidate_id.as_str(),
        ] {
            validate_identifier(value, MAX_DURABLE_IDENTIFIER_BYTES)?;
        }
        Ok(Self {
            provider: provider_id,
            upstream: upstream_id,
            channel: channel_id,
            route: route_id,
            route_candidate: route_candidate_id,
        })
    }

    /// Returns the exact Provider family.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider
    }

    /// Returns the exact configured Upstream.
    #[must_use]
    pub const fn upstream_id(&self) -> &UpstreamId {
        &self.upstream
    }

    /// Returns the exact protocol-specific Channel/Endpoint.
    #[must_use]
    pub const fn channel_id(&self) -> &EndpointId {
        &self.channel
    }

    /// Returns the selected public-model Route.
    #[must_use]
    pub const fn route_id(&self) -> &RouteId {
        &self.route
    }

    /// Returns the exact selected Route Candidate.
    #[must_use]
    pub const fn route_candidate_id(&self) -> &RouteCandidateId {
        &self.route_candidate
    }
}

impl fmt::Debug for StoredResponseTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StoredResponseTarget(<redacted>)")
    }
}

/// Credential revision and optional upstream Response identity pinned to one response.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredResponseCredentialBinding {
    credential_id: CredentialId,
    credential_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    upstream_response_id: Option<String>,
}

impl StoredResponseCredentialBinding {
    /// Creates an exact Credential binding with an optional opaque upstream Response identity.
    ///
    /// # Errors
    ///
    /// Returns [`StoredResponseStoreError::InvalidInput`] for an invalid durable identifier.
    pub fn try_new(
        credential_id: CredentialId,
        credential_revision: u64,
        upstream_response_id: Option<String>,
    ) -> Result<Self, StoredResponseStoreError> {
        validate_identifier(credential_id.as_str(), MAX_DURABLE_IDENTIFIER_BYTES)?;
        if let Some(upstream_response_id) = upstream_response_id.as_deref() {
            validate_identifier(upstream_response_id, MAX_RESPONSE_IDENTIFIER_BYTES)?;
        }
        Ok(Self {
            credential_id,
            credential_revision,
            upstream_response_id,
        })
    }

    /// Returns the exact selected Credential.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the Credential revision held by the successful attempt lease.
    #[must_use]
    pub const fn credential_revision(&self) -> u64 {
        self.credential_revision
    }

    /// Returns the Provider-owned upstream Response identity when one was observed.
    #[must_use]
    pub fn upstream_response_id(&self) -> Option<&str> {
        self.upstream_response_id.as_deref()
    }
}

impl fmt::Debug for StoredResponseCredentialBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredResponseCredentialBinding")
            .field("credential_id", &"<redacted>")
            .field("credential_revision", &self.credential_revision)
            .field(
                "upstream_response_id_present",
                &self.upstream_response_id.is_some(),
            )
            .finish()
    }
}

/// Versioned ownership lineage encrypted together with canonical request/response state.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredResponseLineage {
    config_version_id: String,
    target: StoredResponseTarget,
    credential: StoredResponseCredentialBinding,
}

impl StoredResponseLineage {
    /// Creates exact Config-Version, target, and Credential ownership lineage.
    ///
    /// # Errors
    ///
    /// Returns [`StoredResponseStoreError::InvalidInput`] for an invalid Config Version ID.
    pub fn try_new(
        config_version_id: impl Into<String>,
        target: StoredResponseTarget,
        credential: StoredResponseCredentialBinding,
    ) -> Result<Self, StoredResponseStoreError> {
        let config_version_id = config_version_id.into();
        validate_identifier(&config_version_id, MAX_DURABLE_IDENTIFIER_BYTES)?;
        Ok(Self {
            config_version_id,
            target,
            credential,
        })
    }

    /// Returns the exact serving Config Version.
    #[must_use]
    pub fn config_version_id(&self) -> &str {
        &self.config_version_id
    }

    /// Returns the exact Provider/channel/route target.
    #[must_use]
    pub const fn target(&self) -> &StoredResponseTarget {
        &self.target
    }

    /// Returns the exact Credential revision and upstream Response binding.
    #[must_use]
    pub const fn credential(&self) -> &StoredResponseCredentialBinding {
        &self.credential
    }
}

impl fmt::Debug for StoredResponseLineage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredResponseLineage")
            .field("config_version_id", &"<redacted>")
            .field("target", &self.target)
            .field("credential", &self.credential)
            .finish()
    }
}

/// Complete successful canonical state kept inside the AEAD envelope.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredResponsePayload {
    payload_version: u16,
    lineage: StoredResponseLineage,
    public_model: String,
    created_at_seconds: u64,
    request: CanonicalRequest,
    events: Vec<CanonicalEvent>,
}

impl StoredResponsePayload {
    /// Creates and validates one bounded successful Response payload.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identifiers, an empty/oversized model, too many events, an
    /// invalid/failed canonical lifecycle, or a serialized payload over the fixed 8 MiB bound.
    pub fn try_new(
        lineage: StoredResponseLineage,
        public_model: impl Into<String>,
        created_at_seconds: u64,
        request: CanonicalRequest,
        response: CanonicalResponse,
    ) -> Result<Self, StoredResponseStoreError> {
        let payload = Self {
            payload_version: STORED_RESPONSE_PAYLOAD_VERSION_U16,
            lineage,
            public_model: public_model.into(),
            created_at_seconds,
            request,
            events: response.into_events(),
        };
        payload.validate()?;
        let encoded =
            serde_json::to_vec(&payload).map_err(|_| StoredResponseStoreError::InvalidInput)?;
        ensure_payload_bound(encoded.len())?;
        Ok(payload)
    }

    /// Returns exact encrypted ownership lineage.
    #[must_use]
    pub const fn lineage(&self) -> &StoredResponseLineage {
        &self.lineage
    }

    /// Returns the resolved public model used for client projection.
    #[must_use]
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// Returns the response metadata timestamp in Unix seconds.
    #[must_use]
    pub const fn created_at_seconds(&self) -> u64 {
        self.created_at_seconds
    }

    /// Returns the original canonical request for owned continuation/compaction only.
    #[must_use]
    pub const fn request(&self) -> &CanonicalRequest {
        &self.request
    }

    /// Revalidates and returns the successful canonical response.
    ///
    /// # Errors
    ///
    /// Returns [`StoredResponseStoreError::InvalidPersistedRecord`] if in-memory state no longer
    /// satisfies the successful lifecycle contract.
    pub fn canonical_response(&self) -> Result<CanonicalResponse, StoredResponseStoreError> {
        CanonicalResponse::try_new(self.events.clone())
            .map_err(|_| StoredResponseStoreError::InvalidPersistedRecord)
    }

    /// Returns the downstream Response ID from the canonical `ResponseStart` event.
    ///
    /// # Errors
    ///
    /// Returns [`StoredResponseStoreError::InvalidPersistedRecord`] for a malformed lifecycle.
    pub fn response_id(&self) -> Result<&ResponseId, StoredResponseStoreError> {
        match self.events.first() {
            Some(CanonicalEvent::ResponseStart(start)) => Ok(&start.response_id),
            _ => Err(StoredResponseStoreError::InvalidPersistedRecord),
        }
    }

    fn validate(&self) -> Result<(), StoredResponseStoreError> {
        if self.payload_version != STORED_RESPONSE_PAYLOAD_VERSION_U16 {
            return Err(StoredResponseStoreError::InvalidPersistedRecord);
        }
        validate_identifier(
            &self.lineage.config_version_id,
            MAX_DURABLE_IDENTIFIER_BYTES,
        )?;
        for value in [
            self.lineage.target.provider.as_str(),
            self.lineage.target.upstream.as_str(),
            self.lineage.target.channel.as_str(),
            self.lineage.target.route.as_str(),
            self.lineage.target.route_candidate.as_str(),
            self.lineage.credential.credential_id.as_str(),
        ] {
            validate_identifier(value, MAX_DURABLE_IDENTIFIER_BYTES)?;
        }
        if let Some(upstream_response_id) = self.lineage.credential.upstream_response_id.as_deref()
        {
            validate_identifier(upstream_response_id, MAX_RESPONSE_IDENTIFIER_BYTES)?;
        }
        validate_identifier(&self.public_model, MAX_PUBLIC_MODEL_BYTES)?;
        if self.events.is_empty() || self.events.len() > MAX_STORED_RESPONSE_EVENTS {
            return Err(StoredResponseStoreError::InvalidInput);
        }
        let _validated = CanonicalResponse::try_new(self.events.clone())
            .map_err(|_| StoredResponseStoreError::InvalidInput)?;
        Ok(())
    }
}

impl fmt::Debug for StoredResponsePayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredResponsePayload")
            .field("payload_version", &self.payload_version)
            .field("lineage", &self.lineage)
            .field("public_model", &"<redacted>")
            .field("created_at_seconds", &self.created_at_seconds)
            .field("request", &self.request)
            .field("event_count", &self.events.len())
            .finish()
    }
}

/// Decrypted response returned only after exact owner and expiry admission.
#[derive(Clone, Eq, PartialEq)]
pub struct StoredResponseRecord {
    client_key_id: ClientKeyId,
    response_id: ResponseId,
    created_at_ms: i64,
    expires_at_ms: i64,
    payload: StoredResponsePayload,
}

impl StoredResponseRecord {
    /// Returns the exact Client Key owner.
    #[must_use]
    pub const fn client_key_id(&self) -> &ClientKeyId {
        &self.client_key_id
    }

    /// Returns the client-visible Response ID.
    #[must_use]
    pub const fn response_id(&self) -> &ResponseId {
        &self.response_id
    }

    /// Returns the durable creation instant.
    #[must_use]
    pub const fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    /// Returns the exclusive expiry instant.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }

    /// Returns the decrypted payload to the already-authorized caller.
    #[must_use]
    pub const fn payload(&self) -> &StoredResponsePayload {
        &self.payload
    }
}

impl fmt::Debug for StoredResponseRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredResponseRecord")
            .field("client_key_id", &"<redacted>")
            .field("response_id", &"<redacted>")
            .field("created_at_ms", &self.created_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("payload", &self.payload)
            .finish()
    }
}

/// Gateway-owned compact history sealed under an AEAD domain separate from stored Responses.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredResponseCompactionPayload {
    payload_version: u16,
    lineage: StoredResponseLineage,
    source_response_id: ResponseId,
    public_model: String,
    summary: String,
}

impl StoredResponseCompactionPayload {
    /// Creates one bounded compact history payload.
    ///
    /// # Errors
    ///
    /// Returns a safe input/bound error for invalid lineage, identifiers, model, or summary.
    pub fn try_new(
        lineage: StoredResponseLineage,
        source_response_id: ResponseId,
        public_model: impl Into<String>,
        summary: impl Into<String>,
    ) -> Result<Self, StoredResponseStoreError> {
        let payload = Self {
            payload_version: COMPACTION_PAYLOAD_VERSION_U16,
            lineage,
            source_response_id,
            public_model: public_model.into(),
            summary: summary.into(),
        };
        payload.validate()?;
        let encoded =
            serde_json::to_vec(&payload).map_err(|_| StoredResponseStoreError::InvalidInput)?;
        ensure_payload_bound(encoded.len())?;
        Ok(payload)
    }

    /// Returns the exact execution lineage inherited from the source Response.
    #[must_use]
    pub const fn lineage(&self) -> &StoredResponseLineage {
        &self.lineage
    }

    /// Returns the exact source Response identity.
    #[must_use]
    pub const fn source_response_id(&self) -> &ResponseId {
        &self.source_response_id
    }

    /// Returns the public model that the continuation must keep.
    #[must_use]
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// Returns the gateway-generated summary after exact owner admission.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    fn validate(&self) -> Result<(), StoredResponseStoreError> {
        if self.payload_version != COMPACTION_PAYLOAD_VERSION_U16 {
            return Err(StoredResponseStoreError::InvalidPersistedRecord);
        }
        validate_identifier(
            self.lineage.config_version_id(),
            MAX_DURABLE_IDENTIFIER_BYTES,
        )?;
        for value in [
            self.lineage.target().provider_id().as_str(),
            self.lineage.target().upstream_id().as_str(),
            self.lineage.target().channel_id().as_str(),
            self.lineage.target().route_id().as_str(),
            self.lineage.target().route_candidate_id().as_str(),
            self.lineage.credential().credential_id().as_str(),
        ] {
            validate_identifier(value, MAX_DURABLE_IDENTIFIER_BYTES)?;
        }
        validate_identifier(
            self.source_response_id.as_str(),
            MAX_RESPONSE_IDENTIFIER_BYTES,
        )?;
        validate_identifier(&self.public_model, MAX_PUBLIC_MODEL_BYTES)?;
        if self.summary.is_empty()
            || self.summary.len() > MAX_STORED_RESPONSE_COMPACTION_SUMMARY_BYTES
        {
            return Err(StoredResponseStoreError::PayloadTooLarge);
        }
        Ok(())
    }
}

impl fmt::Debug for StoredResponseCompactionPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredResponseCompactionPayload")
            .field("payload_version", &self.payload_version)
            .field("lineage", &self.lineage)
            .field("source_response_id", &"<redacted>")
            .field("public_model", &"<redacted>")
            .field("summary", &"<redacted>")
            .finish()
    }
}

/// Decrypted compaction returned only after exact owner and expiry admission.
#[derive(Clone, Eq, PartialEq)]
pub struct StoredResponseCompactionRecord {
    client_key_id: ClientKeyId,
    compact_id: String,
    created_at_ms: i64,
    expires_at_ms: i64,
    payload: StoredResponseCompactionPayload,
}

impl StoredResponseCompactionRecord {
    /// Returns the public opaque locator.
    #[must_use]
    pub fn compact_id(&self) -> &str {
        &self.compact_id
    }

    /// Returns the exact owner after admission.
    #[must_use]
    pub const fn client_key_id(&self) -> &ClientKeyId {
        &self.client_key_id
    }

    /// Returns the durable creation instant.
    #[must_use]
    pub const fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    /// Returns the exclusive expiry instant.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }

    /// Returns decrypted compact state to the already-authorized caller.
    #[must_use]
    pub const fn payload(&self) -> &StoredResponseCompactionPayload {
        &self.payload
    }
}

impl fmt::Debug for StoredResponseCompactionRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredResponseCompactionRecord")
            .field("client_key_id", &"<redacted>")
            .field("compact_id", &"<redacted>")
            .field("created_at_ms", &self.created_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("payload", &self.payload)
            .finish()
    }
}

/// Idempotent result of one durable put.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredResponsePutOutcome {
    /// A new encrypted row was inserted.
    Stored,
    /// The exact same owner/identity/timestamps/plaintext was already durable.
    Replayed,
}

/// Thread-safe file- or memory-backed stored-response repository.
pub struct SqliteStoredResponseStore {
    connection: Mutex<Connection>,
    secret_store: SecretStore,
}

impl SqliteStoredResponseStore {
    /// Opens and migrates a file-backed stored-response repository.
    ///
    /// # Errors
    ///
    /// Returns a safe store error when `SQLite` cannot open or migrate the database.
    pub fn open(
        path: impl AsRef<Path>,
        secret_store: SecretStore,
    ) -> Result<Self, StoredResponseStoreError> {
        Self::from_connection(open(path)?, secret_store)
    }

    /// Opens and migrates an isolated in-memory stored-response repository.
    ///
    /// # Errors
    ///
    /// Returns a safe store error when `SQLite` cannot initialize or migrate.
    pub fn open_in_memory(secret_store: SecretStore) -> Result<Self, StoredResponseStoreError> {
        Self::from_connection(open_in_memory()?, secret_store)
    }

    /// Takes an existing connection, applies migrations, and owns it behind a short-held mutex.
    ///
    /// # Errors
    ///
    /// Returns a safe store error when migration fails.
    pub fn from_connection(
        mut connection: Connection,
        secret_store: SecretStore,
    ) -> Result<Self, StoredResponseStoreError> {
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            secret_store,
        })
    }

    /// Stores one successful response under the exact Client Key owner and fixed 30-day TTL.
    ///
    /// Identical replay is idempotent. Reusing the same owner/Response ID for different metadata
    /// or canonical plaintext fails closed. No partial row is committed.
    ///
    /// # Errors
    ///
    /// Returns an input/bound error, a safe AEAD failure, a replay conflict, or a `SQLite` error.
    pub fn put_owned(
        &self,
        client_key_id: &ClientKeyId,
        created_at_ms: i64,
        payload: &StoredResponsePayload,
    ) -> Result<StoredResponsePutOutcome, StoredResponseStoreError> {
        validate_identifier(client_key_id.as_str(), MAX_DURABLE_IDENTIFIER_BYTES)?;
        if created_at_ms < 0 {
            return Err(StoredResponseStoreError::InvalidInput);
        }
        payload.validate()?;
        let response_id = payload.response_id()?;
        validate_identifier(response_id.as_str(), MAX_RESPONSE_IDENTIFIER_BYTES)?;
        let expires_at_ms = created_at_ms
            .checked_add(STORED_RESPONSE_TTL_MILLISECONDS)
            .ok_or(StoredResponseStoreError::TimeOverflow)?;
        let plaintext = Zeroizing::new(
            serde_json::to_vec(payload).map_err(|_| StoredResponseStoreError::InvalidInput)?,
        );
        ensure_payload_bound(plaintext.len())?;
        let associated_data =
            associated_data(client_key_id, response_id, created_at_ms, expires_at_ms)?;

        let mut connection = self.lock_connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)?;
        if let Some(existing) =
            load_row(&transaction, client_key_id.as_str(), response_id.as_str())?
        {
            let existing_plaintext =
                open_row(&self.secret_store, client_key_id, response_id, &existing)?;
            if existing.created_at_ms == created_at_ms
                && existing.expires_at_ms == expires_at_ms
                && existing.payload_version == STORED_RESPONSE_PAYLOAD_VERSION
                && existing_plaintext.as_bytes() == plaintext.as_slice()
            {
                transaction.commit().map_err(StoreError::from)?;
                return Ok(StoredResponsePutOutcome::Replayed);
            }
            return Err(StoredResponseStoreError::ConflictingReplay);
        }
        // Seal only after the exact replay check. An already-durable identical response therefore
        // remains idempotent even if new-nonce generation is temporarily unavailable.
        let encrypted = self
            .secret_store
            .seal(plaintext.as_slice(), &associated_data)?;

        transaction
            .execute(
                "INSERT INTO stored_responses \
                 (client_key_id, response_id, created_at_ms, expires_at_ms, payload_version, \
                  key_version, ciphertext) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    client_key_id.as_str(),
                    response_id.as_str(),
                    created_at_ms,
                    expires_at_ms,
                    STORED_RESPONSE_PAYLOAD_VERSION,
                    encrypted.key_version().as_sqlite_i64(),
                    encrypted.ciphertext(),
                ],
            )
            .map_err(StoreError::from)?;
        transaction.commit().map_err(StoreError::from)?;
        Ok(StoredResponsePutOutcome::Stored)
    }

    /// Stores one compact history under an unguessable owner-scoped locator.
    ///
    /// The compact plaintext uses a separate AEAD domain and table. Locator collisions are
    /// retried within a fixed bound and never overwrite existing owner state.
    ///
    /// # Errors
    ///
    /// Returns a safe input/bound, randomness, AEAD, collision, or `SQLite` error.
    pub fn put_compaction_owned(
        &self,
        client_key_id: &ClientKeyId,
        created_at_ms: i64,
        payload: &StoredResponseCompactionPayload,
    ) -> Result<StoredResponseCompactionRecord, StoredResponseStoreError> {
        validate_identifier(client_key_id.as_str(), MAX_DURABLE_IDENTIFIER_BYTES)?;
        if created_at_ms < 0 {
            return Err(StoredResponseStoreError::InvalidInput);
        }
        payload.validate()?;
        let expires_at_ms = created_at_ms
            .checked_add(STORED_RESPONSE_TTL_MILLISECONDS)
            .ok_or(StoredResponseStoreError::TimeOverflow)?;
        let plaintext = Zeroizing::new(
            serde_json::to_vec(payload).map_err(|_| StoredResponseStoreError::InvalidInput)?,
        );
        ensure_payload_bound(plaintext.len())?;

        for _attempt in 0..COMPACTION_TOKEN_GENERATION_ATTEMPTS {
            let compact_id = generate_compaction_id()?;
            let associated_data = compaction_associated_data(
                client_key_id,
                &compact_id,
                created_at_ms,
                expires_at_ms,
            )?;
            let encrypted = self
                .secret_store
                .seal(plaintext.as_slice(), &associated_data)?;
            let connection = self.lock_connection()?;
            let inserted = connection
                .execute(
                    "INSERT OR IGNORE INTO stored_response_compactions \
                     (client_key_id, compact_id, created_at_ms, expires_at_ms, payload_version, \
                      key_version, ciphertext) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        client_key_id.as_str(),
                        compact_id,
                        created_at_ms,
                        expires_at_ms,
                        COMPACTION_PAYLOAD_VERSION,
                        encrypted.key_version().as_sqlite_i64(),
                        encrypted.ciphertext(),
                    ],
                )
                .map_err(StoreError::from)?;
            drop(connection);
            if inserted == 1 {
                return Ok(StoredResponseCompactionRecord {
                    client_key_id: client_key_id.clone(),
                    compact_id,
                    created_at_ms,
                    expires_at_ms,
                    payload: payload.clone(),
                });
            }
        }
        Err(StoredResponseStoreError::ConflictingReplay)
    }

    /// Opens one exact, still-unexpired owner-scoped compact locator.
    ///
    /// Missing, foreign-owner, and expired locators all return `Ok(None)`. Authentication,
    /// corruption, missing keys, or malformed plaintext fail closed.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, AEAD, lock, or `SQLite` error.
    pub fn get_compaction_owned(
        &self,
        client_key_id: &ClientKeyId,
        compact_id: &str,
        now_ms: i64,
    ) -> Result<Option<StoredResponseCompactionRecord>, StoredResponseStoreError> {
        validate_identifier(client_key_id.as_str(), MAX_DURABLE_IDENTIFIER_BYTES)?;
        validate_compaction_id(compact_id)?;
        if now_ms < 0 {
            return Err(StoredResponseStoreError::InvalidInput);
        }
        let connection = self.lock_connection()?;
        let Some(row) = load_compaction_row(&connection, client_key_id.as_str(), compact_id)?
        else {
            return Ok(None);
        };
        if row.expires_at_ms <= now_ms {
            return Ok(None);
        }
        let plaintext = open_compaction_row(&self.secret_store, client_key_id, compact_id, &row)?;
        let payload: StoredResponseCompactionPayload = serde_json::from_slice(plaintext.as_bytes())
            .map_err(|_| StoredResponseStoreError::InvalidPersistedRecord)?;
        payload
            .validate()
            .map_err(|_| StoredResponseStoreError::InvalidPersistedRecord)?;
        Ok(Some(StoredResponseCompactionRecord {
            client_key_id: client_key_id.clone(),
            compact_id: compact_id.to_owned(),
            created_at_ms: row.created_at_ms,
            expires_at_ms: row.expires_at_ms,
            payload,
        }))
    }

    /// Loads one exact, unexpired owner record and authenticates its encrypted payload.
    ///
    /// A missing, foreign-owner, or expired ID returns `Ok(None)` through the same path. Corrupt
    /// or undecryptable owned state fails closed rather than being presented as another account's
    /// response.
    ///
    /// # Errors
    ///
    /// Returns a safe persisted-record, AEAD, lock, or `SQLite` error.
    pub fn get_owned(
        &self,
        client_key_id: &ClientKeyId,
        response_id: &ResponseId,
        now_ms: i64,
    ) -> Result<Option<StoredResponseRecord>, StoredResponseStoreError> {
        validate_lookup(client_key_id, response_id, now_ms)?;
        let connection = self.lock_connection()?;
        let Some(row) = load_row(&connection, client_key_id.as_str(), response_id.as_str())? else {
            return Ok(None);
        };
        validate_row(&row)?;
        if now_ms >= row.expires_at_ms {
            return Ok(None);
        }
        let plaintext = open_row(&self.secret_store, client_key_id, response_id, &row)?;
        ensure_payload_bound(plaintext.as_bytes().len())?;
        let payload: StoredResponsePayload = serde_json::from_slice(plaintext.as_bytes())
            .map_err(|_| StoredResponseStoreError::InvalidPersistedRecord)?;
        payload
            .validate()
            .map_err(|_| StoredResponseStoreError::InvalidPersistedRecord)?;
        if payload.response_id()? != response_id {
            return Err(StoredResponseStoreError::InvalidPersistedRecord);
        }
        Ok(Some(StoredResponseRecord {
            client_key_id: client_key_id.clone(),
            response_id: response_id.clone(),
            created_at_ms: row.created_at_ms,
            expires_at_ms: row.expires_at_ms,
            payload,
        }))
    }

    /// Deletes one exact, still-unexpired owner row.
    ///
    /// Missing, foreign-owner, and expired rows all return `false`. Expired rows remain eligible
    /// for the bounded GC path, which keeps public existence behavior independent of deletion.
    ///
    /// # Errors
    ///
    /// Returns a safe lock or `SQLite` error.
    pub fn delete_owned(
        &self,
        client_key_id: &ClientKeyId,
        response_id: &ResponseId,
        now_ms: i64,
    ) -> Result<bool, StoredResponseStoreError> {
        validate_lookup(client_key_id, response_id, now_ms)?;
        let connection = self.lock_connection()?;
        let deleted = connection
            .execute(
                "DELETE FROM stored_responses WHERE client_key_id = ?1 AND response_id = ?2 \
                 AND expires_at_ms > ?3",
                params![client_key_id.as_str(), response_id.as_str(), now_ms],
            )
            .map_err(StoreError::from)?;
        Ok(deleted == 1)
    }

    /// Physically removes at most `limit` rows whose exclusive expiry is at or before `now_ms`.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero/oversized batch, negative clock, lock failure, or `SQLite`
    /// error.
    pub fn purge_expired(
        &self,
        now_ms: i64,
        limit: usize,
    ) -> Result<usize, StoredResponseStoreError> {
        if now_ms < 0 || limit == 0 || limit > MAX_STORED_RESPONSE_GC_BATCH {
            return Err(StoredResponseStoreError::InvalidGcLimit);
        }
        let limit = i64::try_from(limit).map_err(|_| StoredResponseStoreError::InvalidGcLimit)?;
        let mut connection = self.lock_connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)?;
        let deleted = transaction
            .execute(
                "DELETE FROM stored_responses WHERE rowid IN (\
                    SELECT rowid FROM stored_responses WHERE expires_at_ms <= ?1 \
                    ORDER BY expires_at_ms, client_key_id, response_id LIMIT ?2\
                 )",
                params![now_ms, limit],
            )
            .map_err(StoreError::from)?;
        transaction.commit().map_err(StoreError::from)?;
        Ok(deleted)
    }

    /// Physically removes at most `limit` expired compact rows in deterministic order.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero/oversized batch, negative clock, lock failure, or `SQLite`
    /// error.
    pub fn purge_expired_compactions(
        &self,
        now_ms: i64,
        limit: usize,
    ) -> Result<usize, StoredResponseStoreError> {
        if now_ms < 0 || limit == 0 || limit > MAX_STORED_RESPONSE_GC_BATCH {
            return Err(StoredResponseStoreError::InvalidGcLimit);
        }
        let limit = i64::try_from(limit).map_err(|_| StoredResponseStoreError::InvalidGcLimit)?;
        let mut connection = self.lock_connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)?;
        let deleted = transaction
            .execute(
                "DELETE FROM stored_response_compactions WHERE rowid IN(\
                    SELECT rowid FROM stored_response_compactions WHERE expires_at_ms <= ?1 \
                    ORDER BY expires_at_ms, client_key_id, compact_id LIMIT ?2\
                 )",
                params![now_ms, limit],
            )
            .map_err(StoreError::from)?;
        transaction.commit().map_err(StoreError::from)?;
        Ok(deleted)
    }

    fn lock_connection(&self) -> Result<MutexGuard<'_, Connection>, StoredResponseStoreError> {
        self.connection
            .lock()
            .map_err(|_| StoredResponseStoreError::LockPoisoned)
    }
}

impl fmt::Debug for SqliteStoredResponseStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SqliteStoredResponseStore(<redacted>)")
    }
}

struct StoredResponseRow {
    created_at_ms: i64,
    expires_at_ms: i64,
    payload_version: i64,
    key_version: i64,
    ciphertext: Vec<u8>,
}

struct StoredResponseCompactionRow {
    created_at_ms: i64,
    expires_at_ms: i64,
    payload_version: i64,
    key_version: i64,
    ciphertext: Vec<u8>,
}

fn load_row(
    connection: &Connection,
    client_key_id: &str,
    response_id: &str,
) -> Result<Option<StoredResponseRow>, StoredResponseStoreError> {
    connection
        .query_row(
            "SELECT created_at_ms, expires_at_ms, payload_version, key_version, ciphertext \
             FROM stored_responses WHERE client_key_id = ?1 AND response_id = ?2",
            params![client_key_id, response_id],
            |row| {
                Ok(StoredResponseRow {
                    created_at_ms: row.get(0)?,
                    expires_at_ms: row.get(1)?,
                    payload_version: row.get(2)?,
                    key_version: row.get(3)?,
                    ciphertext: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
        .map_err(StoredResponseStoreError::from)
}

fn load_compaction_row(
    connection: &Connection,
    client_key_id: &str,
    compact_id: &str,
) -> Result<Option<StoredResponseCompactionRow>, StoredResponseStoreError> {
    connection
        .query_row(
            "SELECT created_at_ms, expires_at_ms, payload_version, key_version, ciphertext \
             FROM stored_response_compactions WHERE client_key_id = ?1 AND compact_id = ?2",
            params![client_key_id, compact_id],
            |row| {
                Ok(StoredResponseCompactionRow {
                    created_at_ms: row.get(0)?,
                    expires_at_ms: row.get(1)?,
                    payload_version: row.get(2)?,
                    key_version: row.get(3)?,
                    ciphertext: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
        .map_err(StoredResponseStoreError::from)
}

fn open_row(
    secret_store: &SecretStore,
    client_key_id: &ClientKeyId,
    response_id: &ResponseId,
    row: &StoredResponseRow,
) -> Result<crate::secret_store::PlaintextSecret, StoredResponseStoreError> {
    validate_row(row)?;
    let key_version = KeyVersion::try_from_sqlite_i64(row.key_version)
        .map_err(|_| StoredResponseStoreError::InvalidPersistedRecord)?;
    let encrypted = EncryptedSecret::try_from_persisted(key_version, row.ciphertext.clone())
        .map_err(|_| StoredResponseStoreError::InvalidPersistedRecord)?;
    let associated_data = associated_data(
        client_key_id,
        response_id,
        row.created_at_ms,
        row.expires_at_ms,
    )?;
    secret_store
        .open(&encrypted, &associated_data)
        .map_err(StoredResponseStoreError::from)
}

fn open_compaction_row(
    secret_store: &SecretStore,
    client_key_id: &ClientKeyId,
    compact_id: &str,
    row: &StoredResponseCompactionRow,
) -> Result<crate::secret_store::PlaintextSecret, StoredResponseStoreError> {
    validate_compaction_row(row)?;
    let key_version = KeyVersion::try_from_sqlite_i64(row.key_version)
        .map_err(|_| StoredResponseStoreError::InvalidPersistedRecord)?;
    let encrypted = EncryptedSecret::try_from_persisted(key_version, row.ciphertext.clone())
        .map_err(|_| StoredResponseStoreError::InvalidPersistedRecord)?;
    let associated_data = compaction_associated_data(
        client_key_id,
        compact_id,
        row.created_at_ms,
        row.expires_at_ms,
    )?;
    secret_store
        .open(&encrypted, &associated_data)
        .map_err(StoredResponseStoreError::from)
}

fn validate_row(row: &StoredResponseRow) -> Result<(), StoredResponseStoreError> {
    if row.created_at_ms < 0
        || row.expires_at_ms <= row.created_at_ms
        || row.payload_version != STORED_RESPONSE_PAYLOAD_VERSION
        || row.key_version <= 0
        || row.ciphertext.is_empty()
        || row.ciphertext.len() > 16 * 1024 * 1024
    {
        return Err(StoredResponseStoreError::InvalidPersistedRecord);
    }
    Ok(())
}

fn validate_compaction_row(
    row: &StoredResponseCompactionRow,
) -> Result<(), StoredResponseStoreError> {
    if row.created_at_ms < 0
        || row.expires_at_ms <= row.created_at_ms
        || row.payload_version != COMPACTION_PAYLOAD_VERSION
        || row.key_version <= 0
        || row.ciphertext.is_empty()
        || row.ciphertext.len() > 16 * 1024 * 1024
    {
        return Err(StoredResponseStoreError::InvalidPersistedRecord);
    }
    Ok(())
}

fn validate_lookup(
    client_key_id: &ClientKeyId,
    response_id: &ResponseId,
    now_ms: i64,
) -> Result<(), StoredResponseStoreError> {
    validate_identifier(client_key_id.as_str(), MAX_DURABLE_IDENTIFIER_BYTES)?;
    validate_identifier(response_id.as_str(), MAX_RESPONSE_IDENTIFIER_BYTES)?;
    if now_ms < 0 {
        return Err(StoredResponseStoreError::InvalidInput);
    }
    Ok(())
}

fn associated_data(
    client_key_id: &ClientKeyId,
    response_id: &ResponseId,
    created_at_ms: i64,
    expires_at_ms: i64,
) -> Result<Vec<u8>, StoredResponseStoreError> {
    let mut associated_data = Vec::with_capacity(
        AAD_DOMAIN.len() + client_key_id.as_str().len() + response_id.as_str().len() + 22,
    );
    associated_data.extend_from_slice(AAD_DOMAIN);
    append_length_prefixed(&mut associated_data, client_key_id.as_str())?;
    append_length_prefixed(&mut associated_data, response_id.as_str())?;
    associated_data.extend_from_slice(&STORED_RESPONSE_PAYLOAD_VERSION_U16.to_be_bytes());
    associated_data.extend_from_slice(&created_at_ms.to_be_bytes());
    associated_data.extend_from_slice(&expires_at_ms.to_be_bytes());
    Ok(associated_data)
}

fn compaction_associated_data(
    client_key_id: &ClientKeyId,
    compact_id: &str,
    created_at_ms: i64,
    expires_at_ms: i64,
) -> Result<Vec<u8>, StoredResponseStoreError> {
    let mut associated_data = Vec::with_capacity(
        COMPACTION_AAD_DOMAIN.len() + client_key_id.as_str().len() + compact_id.len() + 22,
    );
    associated_data.extend_from_slice(COMPACTION_AAD_DOMAIN);
    append_length_prefixed(&mut associated_data, client_key_id.as_str())?;
    append_length_prefixed(&mut associated_data, compact_id)?;
    associated_data.extend_from_slice(&COMPACTION_PAYLOAD_VERSION_U16.to_be_bytes());
    associated_data.extend_from_slice(&created_at_ms.to_be_bytes());
    associated_data.extend_from_slice(&expires_at_ms.to_be_bytes());
    Ok(associated_data)
}

fn generate_compaction_id() -> Result<String, StoredResponseStoreError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut random = [0_u8; COMPACTION_RANDOM_BYTES];
    getrandom::fill(&mut random).map_err(|_| StoredResponseStoreError::RandomnessUnavailable)?;
    let mut encoded = String::with_capacity(STORED_RESPONSE_COMPACTION_PREFIX.len() + 32);
    encoded.push_str(STORED_RESPONSE_COMPACTION_PREFIX);
    for byte in random {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

fn validate_compaction_id(value: &str) -> Result<(), StoredResponseStoreError> {
    validate_identifier(value, MAX_RESPONSE_IDENTIFIER_BYTES)?;
    if !value.starts_with(STORED_RESPONSE_COMPACTION_PREFIX)
        || value.len() != STORED_RESPONSE_COMPACTION_PREFIX.len() + 32
        || !value[STORED_RESPONSE_COMPACTION_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(StoredResponseStoreError::InvalidInput);
    }
    Ok(())
}

fn append_length_prefixed(
    output: &mut Vec<u8>,
    value: &str,
) -> Result<(), StoredResponseStoreError> {
    let length = u16::try_from(value.len()).map_err(|_| StoredResponseStoreError::InvalidInput)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn validate_identifier(value: &str, maximum: usize) -> Result<(), StoredResponseStoreError> {
    if value.is_empty() || value.len() > maximum || value.as_bytes().contains(&0) {
        return Err(StoredResponseStoreError::InvalidInput);
    }
    Ok(())
}

fn ensure_payload_bound(length: usize) -> Result<(), StoredResponseStoreError> {
    if length == 0 || length > MAX_STORED_RESPONSE_PAYLOAD_BYTES {
        return Err(StoredResponseStoreError::PayloadTooLarge);
    }
    Ok(())
}

/// Safe failures from the stored-response boundary.
#[derive(Debug)]
pub enum StoredResponseStoreError {
    /// `SQLite` open/migration/query/write failure.
    Store(StoreError),
    /// AEAD key, envelope, randomness, or authentication failure.
    SecretStore(SecretStoreError),
    /// Caller input violates a bounded identifier/time/lifecycle contract.
    InvalidInput,
    /// Serialized canonical plaintext exceeds the fixed 8 MiB maximum.
    PayloadTooLarge,
    /// An owned persisted row is malformed, mismatched, or cannot be structurally decoded.
    InvalidPersistedRecord,
    /// The same owner/Response ID was replayed with different durable content.
    ConflictingReplay,
    /// Compact locator generation could not obtain operating-system randomness.
    RandomnessUnavailable,
    /// Fixed TTL arithmetic overflowed the signed millisecond domain.
    TimeOverflow,
    /// A garbage-collection batch was zero, oversized, or supplied a negative clock.
    InvalidGcLimit,
    /// A prior panic poisoned the internal `SQLite` connection mutex.
    LockPoisoned,
}

impl fmt::Display for StoredResponseStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(
                formatter,
                "stored-response SQLite operation failed: {error}"
            ),
            Self::SecretStore(_) => formatter.write_str("stored-response AEAD operation failed"),
            Self::InvalidInput => formatter.write_str("stored-response input is invalid"),
            Self::PayloadTooLarge => {
                formatter.write_str("stored-response payload exceeds the finite bound")
            }
            Self::InvalidPersistedRecord => {
                formatter.write_str("persisted stored-response record is invalid")
            }
            Self::ConflictingReplay => {
                formatter.write_str("stored-response replay conflicts with durable state")
            }
            Self::RandomnessUnavailable => {
                formatter.write_str("stored-response locator randomness is unavailable")
            }
            Self::TimeOverflow => formatter.write_str("stored-response retention time overflowed"),
            Self::InvalidGcLimit => {
                formatter.write_str("stored-response garbage-collection batch is invalid")
            }
            Self::LockPoisoned => {
                formatter.write_str("stored-response connection lock is unavailable")
            }
        }
    }
}

impl Error for StoredResponseStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::SecretStore(error) => Some(error),
            Self::InvalidInput
            | Self::PayloadTooLarge
            | Self::InvalidPersistedRecord
            | Self::ConflictingReplay
            | Self::RandomnessUnavailable
            | Self::TimeOverflow
            | Self::InvalidGcLimit
            | Self::LockPoisoned => None,
        }
    }
}

impl From<StoreError> for StoredResponseStoreError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<SecretStoreError> for StoredResponseStoreError {
    fn from(error: SecretStoreError) -> Self {
        Self::SecretStore(error)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::{
        MAX_STORED_RESPONSE_COMPACTION_SUMMARY_BYTES, MAX_STORED_RESPONSE_GC_BATCH,
        MAX_STORED_RESPONSE_PAYLOAD_BYTES, STORED_RESPONSE_COMPACTION_PREFIX,
        STORED_RESPONSE_TTL_MILLISECONDS, SqliteStoredResponseStore,
        StoredResponseCompactionPayload, StoredResponseCredentialBinding, StoredResponseLineage,
        StoredResponsePayload, StoredResponsePutOutcome, StoredResponseStoreError,
        StoredResponseTarget,
    };
    use crate::secret_store::{
        KeyVersion, MASTER_KEY_BYTES, MasterKey, MasterKeyRing, SecretStore,
    };
    use gateway_core::{
        CanonicalEvent, CanonicalRequest, CanonicalResponse, ClientKeyId, CredentialId, EndpointId,
        MessageEnd, MessageRole, ProviderId, RawExtensions, ResponseEnd, ResponseId, ResponseStart,
        RouteCandidateId, RouteId, TextDelta, UpstreamId,
    };

    type TestResult = Result<(), Box<dyn Error>>;
    static TEST_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn encrypted_round_trip_is_owner_exact_and_debug_redacted() -> TestResult {
        let store = SqliteStoredResponseStore::open_in_memory(secret_store(1, &[(1, 0x11)])?)?;
        let owner = client_key("client-a")?;
        let foreign = client_key("client-b")?;
        let payload = payload("resp-a", "private response text")?;

        assert_eq!(
            store.put_owned(&owner, 1_000, &payload)?,
            StoredResponsePutOutcome::Stored
        );
        assert_eq!(
            store.put_owned(&owner, 1_000, &payload)?,
            StoredResponsePutOutcome::Replayed
        );
        let record = store
            .get_owned(&owner, &response_id("resp-a")?, 1_001)?
            .ok_or("owned response missing")?;
        assert_eq!(record.payload(), &payload);
        assert_eq!(
            record.expires_at_ms(),
            1_000 + STORED_RESPONSE_TTL_MILLISECONDS
        );
        assert!(
            store
                .get_owned(&foreign, &response_id("resp-a")?, 1_001)?
                .is_none()
        );

        let debug = format!("{record:?} {store:?}");
        for forbidden in [
            "private response text",
            "client-a",
            "resp-a",
            "credential-a",
        ] {
            assert!(!debug.contains(forbidden));
        }
        Ok(())
    }

    #[test]
    fn conflicting_replay_and_aad_metadata_or_row_swap_fail_closed() -> TestResult {
        let store = SqliteStoredResponseStore::open_in_memory(secret_store(1, &[(1, 0x11)])?)?;
        let owner = client_key("client-a")?;
        let first = payload("resp-a", "first")?;
        let conflicting = payload("resp-a", "second")?;
        store.put_owned(&owner, 1_000, &first)?;
        assert!(matches!(
            store.put_owned(&owner, 1_000, &conflicting),
            Err(StoredResponseStoreError::ConflictingReplay)
        ));

        {
            let connection = store.lock_connection()?;
            connection.execute(
                "UPDATE stored_responses SET expires_at_ms = expires_at_ms + 1 \
                 WHERE response_id = 'resp-a'",
                [],
            )?;
        }
        assert!(matches!(
            store.get_owned(&owner, &response_id("resp-a")?, 1_001),
            Err(StoredResponseStoreError::SecretStore(_))
        ));

        let row_swap_store =
            SqliteStoredResponseStore::open_in_memory(secret_store(1, &[(1, 0x11)])?)?;
        row_swap_store.put_owned(&owner, 1_000, &first)?;
        {
            let connection = row_swap_store.lock_connection()?;
            connection.execute(
                "UPDATE stored_responses SET response_id = 'resp-b' WHERE response_id = 'resp-a'",
                [],
            )?;
        }
        assert!(matches!(
            row_swap_store.get_owned(&owner, &response_id("resp-b")?, 1_001),
            Err(StoredResponseStoreError::SecretStore(_))
        ));
        Ok(())
    }

    #[test]
    fn expiry_is_invisible_and_gc_is_bounded_and_deterministic() -> TestResult {
        let store = SqliteStoredResponseStore::open_in_memory(secret_store(1, &[(1, 0x11)])?)?;
        let owner = client_key("client-a")?;
        for (index, created_at_ms) in [("a", 1_000), ("b", 2_000), ("c", 3_000)] {
            store.put_owned(
                &owner,
                created_at_ms,
                &payload(&format!("resp-{index}"), index)?,
            )?;
        }
        let first_expiry = 1_000 + STORED_RESPONSE_TTL_MILLISECONDS;
        assert!(
            store
                .get_owned(&owner, &response_id("resp-a")?, first_expiry)?
                .is_none()
        );
        assert!(!store.delete_owned(&owner, &response_id("resp-a")?, first_expiry)?);
        assert_eq!(store.purge_expired(first_expiry + 1_000, 1)?, 1);
        assert_eq!(store.purge_expired(first_expiry + 1_000, 1)?, 1);
        assert_eq!(store.purge_expired(first_expiry + 1_000, 1)?, 0);
        assert!(matches!(
            store.purge_expired(first_expiry, 0),
            Err(StoredResponseStoreError::InvalidGcLimit)
        ));
        assert!(matches!(
            store.purge_expired(first_expiry, MAX_STORED_RESPONSE_GC_BATCH + 1),
            Err(StoredResponseStoreError::InvalidGcLimit)
        ));
        assert!(
            store
                .get_owned(&owner, &response_id("resp-c")?, first_expiry + 1_000)?
                .is_some()
        );
        Ok(())
    }

    #[test]
    fn file_reopen_and_key_rotation_read_old_and_write_active_versions() -> TestResult {
        let path = temporary_database_path();
        let owner = client_key("client-a")?;
        {
            let store = SqliteStoredResponseStore::open(&path, secret_store(1, &[(1, 0x11)])?)?;
            store.put_owned(&owner, 1_000, &payload("resp-old", "old")?)?;
        }
        {
            let store =
                SqliteStoredResponseStore::open(&path, secret_store(2, &[(1, 0x11), (2, 0x22)])?)?;
            assert!(
                store
                    .get_owned(&owner, &response_id("resp-old")?, 1_001)?
                    .is_some()
            );
            store.put_owned(&owner, 2_000, &payload("resp-new", "new")?)?;
            let connection = store.lock_connection()?;
            let versions = connection
                .prepare("SELECT key_version FROM stored_responses ORDER BY response_id")?
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(versions, vec![2, 1]);
        }
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn compact_state_is_owner_exact_domain_separated_and_restart_safe() -> TestResult {
        let path = temporary_database_path();
        let owner = client_key("client-a")?;
        let foreign = client_key("client-b")?;
        let compact_payload = StoredResponseCompactionPayload::try_new(
            lineage()?,
            response_id("resp-source")?,
            "model-a",
            "bounded private summary",
        )?;
        let compact_id;
        {
            let store = SqliteStoredResponseStore::open(&path, secret_store(1, &[(1, 0x11)])?)?;
            let record = store.put_compaction_owned(&owner, 1_000, &compact_payload)?;
            compact_id = record.compact_id().to_owned();
            assert!(compact_id.starts_with(STORED_RESPONSE_COMPACTION_PREFIX));
            assert_eq!(record.payload(), &compact_payload);
            assert!(
                store
                    .get_compaction_owned(&foreign, &compact_id, 1_001)?
                    .is_none()
            );
            let opened = store
                .get_compaction_owned(&owner, &compact_id, 1_001)?
                .ok_or("owned compact missing")?;
            assert_eq!(opened.payload().summary(), "bounded private summary");

            let corrupted = store.put_compaction_owned(&owner, 1_001, &compact_payload)?;
            {
                let connection = store.lock_connection()?;
                connection.execute(
                    "UPDATE stored_response_compactions \
                     SET ciphertext = randomblob(length(ciphertext)) WHERE compact_id = ?1",
                    [corrupted.compact_id()],
                )?;
            }
            assert!(matches!(
                store.get_compaction_owned(&owner, corrupted.compact_id(), 1_002),
                Err(StoredResponseStoreError::SecretStore(_)
                    | StoredResponseStoreError::InvalidPersistedRecord)
            ));

            let response = payload("resp-a", "ordinary response")?;
            store.put_owned(&owner, 1_000, &response)?;
            let connection = store.lock_connection()?;
            let response_ciphertext: Vec<u8> = connection.query_row(
                "SELECT ciphertext FROM stored_responses WHERE response_id = 'resp-a'",
                [],
                |row| row.get(0),
            )?;
            let compact_ciphertext: Vec<u8> = connection.query_row(
                "SELECT ciphertext FROM stored_response_compactions WHERE compact_id = ?1",
                [&compact_id],
                |row| row.get(0),
            )?;
            assert_ne!(response_ciphertext, compact_ciphertext);
        }
        {
            let store =
                SqliteStoredResponseStore::open(&path, secret_store(2, &[(1, 0x11), (2, 0x22)])?)?;
            assert!(
                store
                    .get_compaction_owned(&owner, &compact_id, 1_001)?
                    .is_some()
            );
            let expiry = 1_000 + STORED_RESPONSE_TTL_MILLISECONDS;
            assert!(
                store
                    .get_compaction_owned(&owner, &compact_id, expiry)?
                    .is_none()
            );
            assert_eq!(store.purge_expired_compactions(expiry, 1)?, 1);
        }
        fs::remove_file(path)?;

        let oversized = "x".repeat(MAX_STORED_RESPONSE_COMPACTION_SUMMARY_BYTES + 1);
        assert!(matches!(
            StoredResponseCompactionPayload::try_new(
                lineage()?,
                response_id("resp-source")?,
                "model-a",
                oversized,
            ),
            Err(StoredResponseStoreError::PayloadTooLarge)
        ));
        Ok(())
    }

    #[test]
    fn payload_bounds_invalid_lifecycle_and_ciphertext_corruption_fail_closed() -> TestResult {
        let owner = client_key("client-a")?;
        let store = SqliteStoredResponseStore::open_in_memory(secret_store(1, &[(1, 0x11)])?)?;
        let valid = payload("resp-a", "safe")?;
        store.put_owned(&owner, 1_000, &valid)?;
        {
            let connection = store.lock_connection()?;
            connection.execute(
                "UPDATE stored_responses SET ciphertext = randomblob(length(ciphertext))",
                [],
            )?;
        }
        assert!(matches!(
            store.get_owned(&owner, &response_id("resp-a")?, 1_001),
            Err(StoredResponseStoreError::SecretStore(_)
                | StoredResponseStoreError::InvalidPersistedRecord)
        ));

        let response =
            CanonicalResponse::try_new(vec![CanonicalEvent::ResponseStart(ResponseStart {
                response_id: response_id("resp-incomplete")?,
                extensions: RawExtensions::default(),
            })]);
        assert!(response.is_err());
        let oversized_text = "x".repeat(MAX_STORED_RESPONSE_PAYLOAD_BYTES);
        assert!(matches!(
            StoredResponsePayload::try_new(
                lineage()?,
                "model-a",
                1,
                request()?,
                canonical_response("resp-large", &oversized_text)?,
            ),
            Err(StoredResponseStoreError::PayloadTooLarge)
        ));
        Ok(())
    }

    fn payload(
        response_id_value: &str,
        text: &str,
    ) -> Result<StoredResponsePayload, Box<dyn Error>> {
        let response = canonical_response(response_id_value, text)?;
        Ok(StoredResponsePayload::try_new(
            lineage()?,
            "model-a",
            1,
            request()?,
            response,
        )?)
    }

    fn canonical_response(
        response_id_value: &str,
        text: &str,
    ) -> Result<CanonicalResponse, Box<dyn Error>> {
        Ok(CanonicalResponse::try_new(vec![
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id: response_id(response_id_value)?,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageStart(gateway_core::MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::TextDelta(TextDelta {
                text: text.to_owned(),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageEnd(MessageEnd::default()),
            CanonicalEvent::ResponseEnd(ResponseEnd {
                stop_reason: Some("stop".to_owned()),
                stop_sequence: None,
                extensions: RawExtensions::default(),
            }),
        ])?)
    }

    fn request() -> Result<CanonicalRequest, serde_json::Error> {
        serde_json::from_str(
            r#"{"requested_model":"model-a","messages":[],"tools":[],"extensions":{}}"#,
        )
    }

    fn lineage() -> Result<StoredResponseLineage, StoredResponseStoreError> {
        StoredResponseLineage::try_new(
            "config-a",
            StoredResponseTarget::try_new(
                ProviderId::try_new("provider-a")
                    .map_err(|_| StoredResponseStoreError::InvalidInput)?,
                UpstreamId::try_new("upstream-a")
                    .map_err(|_| StoredResponseStoreError::InvalidInput)?,
                EndpointId::try_new("channel-a")
                    .map_err(|_| StoredResponseStoreError::InvalidInput)?,
                RouteId::try_new("route-a").map_err(|_| StoredResponseStoreError::InvalidInput)?,
                RouteCandidateId::try_new("candidate-a")
                    .map_err(|_| StoredResponseStoreError::InvalidInput)?,
            )?,
            StoredResponseCredentialBinding::try_new(
                CredentialId::try_new("credential-a")
                    .map_err(|_| StoredResponseStoreError::InvalidInput)?,
                7,
                Some("upstream-response-a".to_owned()),
            )?,
        )
    }

    fn client_key(value: &str) -> Result<ClientKeyId, Box<dyn Error>> {
        Ok(ClientKeyId::try_new(value)?)
    }

    fn response_id(value: &str) -> Result<ResponseId, Box<dyn Error>> {
        Ok(ResponseId::try_new(value)?)
    }

    fn secret_store(
        active_version: u32,
        entries: &[(u32, u8)],
    ) -> Result<SecretStore, Box<dyn Error>> {
        let active_version = KeyVersion::try_new(active_version)?;
        let entries = entries
            .iter()
            .map(|(version, byte)| {
                Ok((
                    KeyVersion::try_new(*version)?,
                    MasterKey::try_from_bytes([*byte; MASTER_KEY_BYTES])?,
                ))
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(SecretStore::new(MasterKeyRing::try_new(
            active_version,
            entries,
        )?))
    }

    fn temporary_database_path() -> PathBuf {
        let sequence = TEST_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "gateway-stored-response-test-{}-{sequence}.sqlite",
            std::process::id()
        ))
    }
}
