//! Strict Kiro credential boundary.
//!
//! The module accepts one explicitly supplied credential object, keeps secrets zeroized and
//! diagnostic-safe, and exposes refresh only through an injected transport. It never discovers a
//! cache, reads environment variables, opens a socket, or selects an IDE/CLI endpoint.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    sync::{Condvar, Mutex},
    time::Duration,
};

use gateway_core::CredentialId;
use gateway_store::secret_store::{EncryptedSecret, SecretStore};
use serde::{
    Deserialize,
    de::{self, MapAccess, Visitor},
};
use serde_json::Value;
use zeroize::Zeroizing;

const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_SECRET_BYTES: usize = 16 * 1024;
const MAX_NON_SECRET_BYTES: usize = 512;
const MAX_REGION_BYTES: usize = 128;
const MAX_LIFETIME_MS: i64 = 366 * 24 * 60 * 60 * 1_000;
const PERSISTED_FORMAT_VERSION: u8 = 1;
const AAD_DOMAIN: &[u8] = b"cpa-rust-gateway/kiro/credential/v1";

/// Default bounded wait for an already-running refresh of the same Credential.
pub const DEFAULT_KIRO_REFRESH_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

/// The three mutually exclusive Kiro authentication families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroCredentialKind {
    /// Builder/Social OAuth with a refresh token.
    Social,
    /// Enterprise/IdC OAuth with client authentication and an auth Region.
    Enterprise,
    /// A headless CLI `ksk_` key; it never participates in OAuth refresh.
    ApiKey,
}

/// Non-secret provenance of a validated Kiro credential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroCredentialSource {
    /// The explicit strict JSON import requested by the control plane.
    ImportedJson,
    /// A backward-compatible raw API key supplied by one encrypted runtime lease.
    RuntimeLease,
    /// A later injected Social or Enterprise refresh response.
    Refresh,
}

/// A Kiro credential whose secret values are redacted and zeroized on drop.
#[derive(Clone)]
pub enum KiroCredential {
    /// Builder/Social OAuth credential.
    Social(KiroOAuthCredential),
    /// Enterprise/IdC OAuth credential.
    Enterprise(KiroEnterpriseCredential),
    /// CLI API key credential.
    ApiKey(KiroApiKeyCredential),
}

/// The shared OAuth values used only by Social and Enterprise credentials.
#[derive(Clone)]
pub struct KiroOAuthCredential {
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
    expires_at_ms: i64,
    source: KiroCredentialSource,
}

/// Enterprise OAuth values in addition to the shared OAuth token pair.
#[derive(Clone)]
pub struct KiroEnterpriseCredential {
    oauth: KiroOAuthCredential,
    client_id: String,
    client_secret: Zeroizing<String>,
    auth_region: String,
}

/// A CLI-only `ksk_` key.
#[derive(Clone)]
pub struct KiroApiKeyCredential {
    api_key: Zeroizing<String>,
    source: KiroCredentialSource,
}

impl KiroCredential {
    /// Imports one request-time runtime lease without discovering any ambient credential source.
    ///
    /// Existing P12 deployments store headless `ksk_` material directly, while Social and
    /// Enterprise credentials use the strict control-plane JSON shape documented by
    /// [`Self::import_json`]. A value that is not an exact raw API key is never coerced into one;
    /// it must pass the duplicate-free, exact-field JSON importer and its absolute-expiry check.
    ///
    /// # Errors
    ///
    /// Returns a safe credential error for malformed raw keys, invalid JSON, mixed credential
    /// families, or expired OAuth material.
    pub fn import_runtime_secret(
        input: &[u8],
        observed_at_ms: i64,
    ) -> Result<Self, KiroCredentialError> {
        if input.starts_with(b"ksk_") {
            let key = std::str::from_utf8(input)
                .map_err(|_| KiroCredentialError::InvalidField)?
                .to_owned();
            return Ok(Self::ApiKey(KiroApiKeyCredential::new(
                key,
                KiroCredentialSource::RuntimeLease,
            )?));
        }
        Self::import_json(input, observed_at_ms)
    }

    /// Imports one strict, versionless control-plane credential object.
    ///
    /// Social requires `kind`, `access_token`, `refresh_token`, and `expires_at_ms`; Enterprise
    /// adds `client_id`, `client_secret`, and `auth_region`; API-key input has only `kind` and
    /// `api_key`. Endpoint/API Region configuration belongs to P7-02 and is deliberately absent.
    ///
    /// # Errors
    ///
    /// Returns a safe classification for malformed, ambiguous, expired, or unsafe input.
    pub fn import_json(input: &[u8], observed_at_ms: i64) -> Result<Self, KiroCredentialError> {
        let object = strict_object(input)?;
        let kind = required_string(&object, "kind", MAX_NON_SECRET_BYTES)?;
        match kind {
            "social" => {
                require_exact_fields(
                    &object,
                    &["kind", "access_token", "refresh_token", "expires_at_ms"],
                )?;
                Ok(Self::Social(KiroOAuthCredential::new(
                    required_secret(&object, "access_token")?,
                    required_secret(&object, "refresh_token")?,
                    required_expiry(&object, observed_at_ms)?,
                    KiroCredentialSource::ImportedJson,
                )?))
            }
            "enterprise" => {
                require_exact_fields(
                    &object,
                    &[
                        "kind",
                        "access_token",
                        "refresh_token",
                        "expires_at_ms",
                        "client_id",
                        "client_secret",
                        "auth_region",
                    ],
                )?;
                let oauth = KiroOAuthCredential::new(
                    required_secret(&object, "access_token")?,
                    required_secret(&object, "refresh_token")?,
                    required_expiry(&object, observed_at_ms)?,
                    KiroCredentialSource::ImportedJson,
                )?;
                Ok(Self::Enterprise(KiroEnterpriseCredential::new(
                    oauth,
                    required_string(&object, "client_id", MAX_NON_SECRET_BYTES)?.to_owned(),
                    required_secret(&object, "client_secret")?,
                    required_region(&object, "auth_region")?,
                )?))
            }
            "api_key" => {
                require_exact_fields(&object, &["kind", "api_key"])?;
                Ok(Self::ApiKey(KiroApiKeyCredential::new(
                    required_secret(&object, "api_key")?,
                    KiroCredentialSource::ImportedJson,
                )?))
            }
            _ => Err(KiroCredentialError::InvalidKind),
        }
    }

    /// Returns this credential's exact family without rendering secret material.
    #[must_use]
    pub const fn kind(&self) -> KiroCredentialKind {
        match self {
            Self::Social(_) => KiroCredentialKind::Social,
            Self::Enterprise(_) => KiroCredentialKind::Enterprise,
            Self::ApiKey(_) => KiroCredentialKind::ApiKey,
        }
    }

    /// Returns the non-secret provenance of this validated credential value.
    #[must_use]
    pub const fn source(&self) -> KiroCredentialSource {
        match self {
            Self::Social(value) => value.source,
            Self::Enterprise(value) => value.oauth.source,
            Self::ApiKey(value) => value.source,
        }
    }

    /// Returns whether an OAuth credential is expired at the caller-supplied instant.
    #[must_use]
    pub const fn is_expired_at(&self, now_ms: i64) -> bool {
        match self {
            Self::Social(value) => value.is_expired_at(now_ms),
            Self::Enterprise(value) => value.oauth.is_expired_at(now_ms),
            Self::ApiKey(_) => false,
        }
    }

    /// Borrows a Social/Enterprise access token only for immediate request construction.
    ///
    /// # Errors
    ///
    /// Returns an error when called on a CLI API-key credential.
    pub fn access_token(&self) -> Result<&str, KiroCredentialError> {
        match self {
            Self::Social(value) => Ok(value.access_token()),
            Self::Enterprise(value) => Ok(value.oauth.access_token()),
            Self::ApiKey(_) => Err(KiroCredentialError::WrongCredentialKind),
        }
    }

    /// Borrows a CLI API key only for immediate request construction.
    ///
    /// # Errors
    ///
    /// Returns an error when called on a Social or Enterprise credential.
    pub fn api_key(&self) -> Result<&str, KiroCredentialError> {
        match self {
            Self::ApiKey(value) => Ok(value.api_key()),
            Self::Social(_) | Self::Enterprise(_) => Err(KiroCredentialError::WrongCredentialKind),
        }
    }

    /// Returns Enterprise's auth Region; Social/API-key credentials do not invent one.
    #[must_use]
    pub fn auth_region(&self) -> Option<&str> {
        match self {
            Self::Enterprise(value) => Some(value.auth_region()),
            Self::Social(_) | Self::ApiKey(_) => None,
        }
    }

    /// Performs exactly one injected OAuth refresh for a refreshable credential.
    ///
    /// # Errors
    ///
    /// Returns a safe error for API-key credentials, transport failure, or an invalid token result.
    pub fn refresh<T: KiroRefreshTransport>(
        &self,
        transport: &T,
        observed_at_ms: i64,
    ) -> Result<Self, KiroCredentialError> {
        let request = match self {
            Self::Social(value) => KiroRefreshRequest::social(value.refresh_token()),
            Self::Enterprise(value) => KiroRefreshRequest::enterprise(
                value.oauth.refresh_token(),
                value.client_id(),
                value.client_secret(),
                value.auth_region(),
            ),
            Self::ApiKey(_) => return Err(KiroCredentialError::NotRefreshable),
        };
        let response = transport
            .refresh(request)
            .map_err(|_| KiroCredentialError::TransportUnavailable)?;
        let object = strict_object(response.body())?;
        require_exact_fields(
            &object,
            &["access_token", "refresh_token", "expires_in", "token_type"],
        )?;
        if let Some(token_type) = optional_string(&object, "token_type", MAX_NON_SECRET_BYTES)?
            && !token_type.eq_ignore_ascii_case("bearer")
        {
            return Err(KiroCredentialError::InvalidRefreshResponse);
        }
        let access = required_secret(&object, "access_token")?;
        let refresh = optional_secret(&object, "refresh_token")?;
        let expires_at_ms = observed_at_ms
            .checked_add(required_lifetime_ms(&object)?)
            .ok_or(KiroCredentialError::InvalidTimestamp)?;
        match self {
            Self::Social(value) => Ok(Self::Social(KiroOAuthCredential::new(
                access,
                refresh.unwrap_or_else(|| value.refresh_token().to_owned()),
                expires_at_ms,
                KiroCredentialSource::Refresh,
            )?)),
            Self::Enterprise(value) => Ok(Self::Enterprise(KiroEnterpriseCredential::new(
                KiroOAuthCredential::new(
                    access,
                    refresh.unwrap_or_else(|| value.oauth.refresh_token().to_owned()),
                    expires_at_ms,
                    KiroCredentialSource::Refresh,
                )?,
                value.client_id().to_owned(),
                value.client_secret().to_owned(),
                value.auth_region().to_owned(),
            )?)),
            Self::ApiKey(_) => Err(KiroCredentialError::NotRefreshable),
        }
    }

    /// Seals this bounded credential under an exact control-plane Credential identity.
    ///
    /// # Errors
    ///
    /// Returns a safe error when serialization or authenticated encryption fails.
    pub fn seal(
        &self,
        store: &SecretStore,
        credential_id: &CredentialId,
    ) -> Result<KiroSealedCredential, KiroCredentialError> {
        let bytes = self.persisted_bytes()?;
        let encrypted_secret = store
            .seal(&bytes, &associated_data(credential_id))
            .map_err(|_| KiroCredentialError::EncryptionFailed)?;
        Ok(KiroSealedCredential { encrypted_secret })
    }

    fn persisted_bytes(&self) -> Result<Zeroizing<Vec<u8>>, KiroCredentialError> {
        let mut output = Zeroizing::new(vec![PERSISTED_FORMAT_VERSION]);
        match self {
            Self::Social(value) => {
                output.push(0);
                output.push(source_byte(value.source));
                write_segment(&mut output, value.access_token())?;
                write_segment(&mut output, value.refresh_token())?;
                output.extend_from_slice(&value.expires_at_ms.to_be_bytes());
            }
            Self::Enterprise(value) => {
                output.push(1);
                output.push(source_byte(value.oauth.source));
                write_segment(&mut output, value.oauth.access_token())?;
                write_segment(&mut output, value.oauth.refresh_token())?;
                output.extend_from_slice(&value.oauth.expires_at_ms.to_be_bytes());
                write_segment(&mut output, value.client_id())?;
                write_segment(&mut output, value.client_secret())?;
                write_segment(&mut output, value.auth_region())?;
            }
            Self::ApiKey(value) => {
                output.push(2);
                output.push(source_byte(value.source));
                write_segment(&mut output, value.api_key())?;
            }
        }
        Ok(output)
    }

    fn from_persisted_bytes(input: &[u8]) -> Result<Self, KiroCredentialError> {
        let mut cursor = 0;
        if read_byte(input, &mut cursor)? != PERSISTED_FORMAT_VERSION {
            return Err(KiroCredentialError::InvalidPersistedCredential);
        }
        let kind = read_byte(input, &mut cursor)?;
        let source = source_from_byte(read_byte(input, &mut cursor)?)?;
        let credential = match kind {
            0 => Self::Social(KiroOAuthCredential::new(
                read_segment(input, &mut cursor)?,
                read_segment(input, &mut cursor)?,
                read_i64(input, &mut cursor)?,
                source,
            )?),
            1 => {
                let oauth = KiroOAuthCredential::new(
                    read_segment(input, &mut cursor)?,
                    read_segment(input, &mut cursor)?,
                    read_i64(input, &mut cursor)?,
                    source,
                )?;
                Self::Enterprise(KiroEnterpriseCredential::new(
                    oauth,
                    read_segment(input, &mut cursor)?,
                    read_segment(input, &mut cursor)?,
                    read_segment(input, &mut cursor)?,
                )?)
            }
            2 => Self::ApiKey(KiroApiKeyCredential::new(
                read_segment(input, &mut cursor)?,
                source,
            )?),
            _ => return Err(KiroCredentialError::InvalidPersistedCredential),
        };
        if cursor != input.len() {
            return Err(KiroCredentialError::InvalidPersistedCredential);
        }
        Ok(credential)
    }
}

impl fmt::Debug for KiroCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroCredential")
            .field("kind", &self.kind())
            .field("auth_region", &self.auth_region())
            .field("secret", &"<redacted>")
            .finish()
    }
}

impl KiroOAuthCredential {
    fn new(
        access_token: String,
        refresh_token: String,
        expires_at_ms: i64,
        source: KiroCredentialSource,
    ) -> Result<Self, KiroCredentialError> {
        validate_secret(&access_token)?;
        validate_secret(&refresh_token)?;
        if expires_at_ms <= 0 {
            return Err(KiroCredentialError::InvalidTimestamp);
        }
        Ok(Self {
            access_token: Zeroizing::new(access_token),
            refresh_token: Zeroizing::new(refresh_token),
            expires_at_ms,
            source,
        })
    }
    fn access_token(&self) -> &str {
        self.access_token.as_str()
    }
    fn refresh_token(&self) -> &str {
        self.refresh_token.as_str()
    }
    const fn is_expired_at(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }
}

impl fmt::Debug for KiroOAuthCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroOAuthCredential")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("source", &self.source)
            .field("access_token_length", &self.access_token.len())
            .field("refresh_token_length", &self.refresh_token.len())
            .finish()
    }
}

impl KiroEnterpriseCredential {
    fn new(
        oauth: KiroOAuthCredential,
        client_id: String,
        client_secret: String,
        auth_region: String,
    ) -> Result<Self, KiroCredentialError> {
        validate_non_secret(&client_id, MAX_NON_SECRET_BYTES)?;
        validate_secret(&client_secret)?;
        validate_region_value(&auth_region)?;
        Ok(Self {
            oauth,
            client_id,
            client_secret: Zeroizing::new(client_secret),
            auth_region,
        })
    }
    fn client_id(&self) -> &str {
        &self.client_id
    }
    fn client_secret(&self) -> &str {
        self.client_secret.as_str()
    }
    fn auth_region(&self) -> &str {
        &self.auth_region
    }
}

impl fmt::Debug for KiroEnterpriseCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroEnterpriseCredential")
            .field("oauth", &self.oauth)
            .field("client_id", &self.client_id)
            .field("auth_region", &self.auth_region)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}

impl KiroApiKeyCredential {
    fn new(api_key: String, source: KiroCredentialSource) -> Result<Self, KiroCredentialError> {
        validate_secret(&api_key)?;
        if !api_key.starts_with("ksk_") {
            return Err(KiroCredentialError::InvalidApiKey);
        }
        Ok(Self {
            api_key: Zeroizing::new(api_key),
            source,
        })
    }
    fn api_key(&self) -> &str {
        self.api_key.as_str()
    }
}

impl fmt::Debug for KiroApiKeyCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroApiKeyCredential")
            .field("source", &self.source)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

/// Opaque encrypted credential material for a caller-owned persistence layer.
pub struct KiroSealedCredential {
    encrypted_secret: EncryptedSecret,
}

impl KiroSealedCredential {
    /// Reopens this exact encrypted record only with the owning Credential's AAD.
    ///
    /// # Errors
    ///
    /// Returns a safe error when authentication or the bounded persisted format fails.
    pub fn open(
        &self,
        store: &SecretStore,
        credential_id: &CredentialId,
    ) -> Result<KiroCredential, KiroCredentialError> {
        let plaintext = store
            .open(&self.encrypted_secret, &associated_data(credential_id))
            .map_err(|_| KiroCredentialError::EncryptionFailed)?;
        KiroCredential::from_persisted_bytes(plaintext.as_bytes())
    }
    /// Returns the opaque envelope for explicit persistence without exposing plaintext.
    #[must_use]
    pub const fn encrypted_secret(&self) -> &EncryptedSecret {
        &self.encrypted_secret
    }
}

impl fmt::Debug for KiroSealedCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroSealedCredential")
            .field("encrypted_secret", &self.encrypted_secret)
            .finish()
    }
}

/// A revisioned in-memory Kiro Credential view.
#[derive(Clone)]
pub struct KiroCredentialVersion {
    credential: KiroCredential,
    revision: u64,
}

impl KiroCredentialVersion {
    /// Returns the secret-redacted credential view for immediate use.
    #[must_use]
    pub const fn credential(&self) -> &KiroCredential {
        &self.credential
    }

    /// Returns the monotonic revision associated with this exact credential value.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

impl fmt::Debug for KiroCredentialVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroCredentialVersion")
            .field("credential", &self.credential)
            .field("revision", &self.revision)
            .finish()
    }
}

/// Result of an exact expected-revision runtime replacement.
#[derive(Clone, Debug)]
pub enum KiroCredentialCasOutcome {
    /// The replacement committed at the next revision.
    Committed(KiroCredentialVersion),
    /// The requested Credential was not configured.
    Missing,
    /// Another writer already changed the Credential revision.
    Conflict,
}

/// Per-Credential refresh coordinator with bounded same-key singleflight.
pub struct KiroCredentialRefreshCoordinator {
    state: Mutex<BTreeMap<CredentialId, KiroCredentialRuntimeState>>,
    changed: Condvar,
    wait_timeout: Duration,
}

struct KiroCredentialRuntimeState {
    version: KiroCredentialVersion,
    refresh_in_flight: bool,
}

impl KiroCredentialRefreshCoordinator {
    /// Creates a coordinator with initial revision-zero Credential values.
    ///
    /// # Errors
    ///
    /// Returns an error if the same Credential ID appears more than once.
    pub fn try_new(
        credentials: impl IntoIterator<Item = (CredentialId, KiroCredential)>,
    ) -> Result<Self, KiroCredentialError> {
        Self::try_new_with_timeout(credentials, DEFAULT_KIRO_REFRESH_WAIT_TIMEOUT)
    }

    /// Creates a coordinator with a caller-selected positive same-key wait limit.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero wait timeout or duplicate Credential ID.
    pub fn try_new_with_timeout(
        credentials: impl IntoIterator<Item = (CredentialId, KiroCredential)>,
        wait_timeout: Duration,
    ) -> Result<Self, KiroCredentialError> {
        if wait_timeout.is_zero() {
            return Err(KiroCredentialError::InvalidRefreshWaitTimeout);
        }
        let mut state = BTreeMap::new();
        for (credential_id, credential) in credentials {
            let entry = KiroCredentialRuntimeState {
                version: KiroCredentialVersion {
                    credential,
                    revision: 0,
                },
                refresh_in_flight: false,
            };
            if state.insert(credential_id, entry).is_some() {
                return Err(KiroCredentialError::DuplicateCredentialId);
            }
        }
        Ok(Self {
            state: Mutex::new(state),
            changed: Condvar::new(),
            wait_timeout,
        })
    }

    /// Loads one exact revisioned Credential without changing refresh state.
    ///
    /// # Errors
    ///
    /// Returns a safe error when the Credential is absent or the runtime lock is unavailable.
    pub fn load(
        &self,
        credential_id: &CredentialId,
    ) -> Result<KiroCredentialVersion, KiroCredentialError> {
        let state = self
            .state
            .lock()
            .map_err(|_| KiroCredentialError::RuntimeUnavailable)?;
        state
            .get(credential_id)
            .map(|entry| entry.version.clone())
            .ok_or(KiroCredentialError::RuntimeCredentialMissing)
    }

    /// Replaces a Credential only if the exact prior revision is still current.
    ///
    /// # Errors
    ///
    /// Returns a safe lock or revision-overflow failure; ordinary CAS states are explicit.
    pub fn compare_and_swap(
        &self,
        credential_id: &CredentialId,
        expected_revision: u64,
        credential: KiroCredential,
    ) -> Result<KiroCredentialCasOutcome, KiroCredentialError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| KiroCredentialError::RuntimeUnavailable)?;
        let Some(entry) = state.get_mut(credential_id) else {
            return Ok(KiroCredentialCasOutcome::Missing);
        };
        if entry.version.revision != expected_revision {
            return Ok(KiroCredentialCasOutcome::Conflict);
        }
        let revision = expected_revision
            .checked_add(1)
            .ok_or(KiroCredentialError::RevisionOverflow)?;
        entry.version = KiroCredentialVersion {
            credential,
            revision,
        };
        self.changed.notify_all();
        Ok(KiroCredentialCasOutcome::Committed(entry.version.clone()))
    }

    /// Refreshes an expired OAuth Credential once per exact Credential ID.
    ///
    /// Followers wait only for the same Credential. They then return the new revision or an
    /// explicit reload state, never a second refresh and never a stale leader result.
    ///
    /// # Errors
    ///
    /// Returns a safe error for a missing runtime value, bounded wait expiry, refresh failure, or
    /// a stale leader whose source revision changed during the transport call.
    pub fn refresh_if_expired<T: KiroRefreshTransport>(
        &self,
        credential_id: &CredentialId,
        transport: &T,
        observed_at_ms: i64,
    ) -> Result<KiroCredentialVersion, KiroCredentialError> {
        let (credential, revision) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| KiroCredentialError::RuntimeUnavailable)?;
            let (refresh_in_flight, current_revision) = {
                let entry = state
                    .get(credential_id)
                    .ok_or(KiroCredentialError::RuntimeCredentialMissing)?;
                if !entry.version.credential.is_expired_at(observed_at_ms) {
                    return Ok(entry.version.clone());
                }
                (entry.refresh_in_flight, entry.version.revision)
            };
            if refresh_in_flight {
                let (state, wait) = self
                    .changed
                    .wait_timeout_while(state, self.wait_timeout, |state| {
                        state.get(credential_id).is_some_and(|entry| {
                            entry.refresh_in_flight && entry.version.revision == current_revision
                        })
                    })
                    .map_err(|_| KiroCredentialError::RuntimeUnavailable)?;
                let entry = state
                    .get(credential_id)
                    .ok_or(KiroCredentialError::RuntimeCredentialMissing)?;
                if entry.version.revision != current_revision {
                    return Ok(entry.version.clone());
                }
                if entry.refresh_in_flight {
                    debug_assert!(wait.timed_out());
                    return Err(KiroCredentialError::RefreshWaitTimedOut);
                }
                return if entry.version.credential.is_expired_at(observed_at_ms) {
                    Err(KiroCredentialError::RefreshCompletedByPeer)
                } else {
                    Ok(entry.version.clone())
                };
            }
            let entry = state
                .get_mut(credential_id)
                .ok_or(KiroCredentialError::RuntimeCredentialMissing)?;
            entry.refresh_in_flight = true;
            (entry.version.credential.clone(), entry.version.revision)
        };

        let refreshed = credential.refresh(transport, observed_at_ms);
        let mut state = self
            .state
            .lock()
            .map_err(|_| KiroCredentialError::RuntimeUnavailable)?;
        let entry = state
            .get_mut(credential_id)
            .ok_or(KiroCredentialError::RuntimeCredentialMissing)?;
        entry.refresh_in_flight = false;
        self.changed.notify_all();
        let refreshed = refreshed?;
        if entry.version.revision != revision {
            return Err(KiroCredentialError::ConcurrentCredentialStateChanged);
        }
        let revision = revision
            .checked_add(1)
            .ok_or(KiroCredentialError::RevisionOverflow)?;
        entry.version = KiroCredentialVersion {
            credential: refreshed,
            revision,
        };
        Ok(entry.version.clone())
    }
}

impl fmt::Debug for KiroCredentialRefreshCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self
            .state
            .lock()
            .map(|state| state.len())
            .unwrap_or_default();
        formatter
            .debug_struct("KiroCredentialRefreshCoordinator")
            .field("credential_count", &count)
            .field("wait_timeout", &self.wait_timeout)
            .finish_non_exhaustive()
    }
}

/// One secret-free refresh-operation category visible to an injected transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroRefreshKind {
    /// Social OAuth refresh.
    Social,
    /// Enterprise `IdC` refresh.
    Enterprise,
}

/// A private-payload refresh request; only its kind and auth Region are inspectable.
pub struct KiroRefreshRequest {
    kind: KiroRefreshKind,
    auth_region: Option<String>,
    refresh_token: Zeroizing<String>,
    client_id: Option<String>,
    client_secret: Option<Zeroizing<String>>,
}

impl KiroRefreshRequest {
    fn social(refresh_token: &str) -> Self {
        Self {
            kind: KiroRefreshKind::Social,
            auth_region: None,
            refresh_token: Zeroizing::new(refresh_token.to_owned()),
            client_id: None,
            client_secret: None,
        }
    }
    fn enterprise(
        refresh_token: &str,
        client_id: &str,
        client_secret: &str,
        auth_region: &str,
    ) -> Self {
        Self {
            kind: KiroRefreshKind::Enterprise,
            auth_region: Some(auth_region.to_owned()),
            refresh_token: Zeroizing::new(refresh_token.to_owned()),
            client_id: Some(client_id.to_owned()),
            client_secret: Some(Zeroizing::new(client_secret.to_owned())),
        }
    }
    /// Returns the refresh family. Secret form values stay private to this module.
    #[must_use]
    pub const fn kind(&self) -> KiroRefreshKind {
        self.kind
    }
    /// Returns Enterprise's validated auth Region, if this request has one.
    #[must_use]
    pub fn auth_region(&self) -> Option<&str> {
        self.auth_region.as_deref()
    }
}

impl fmt::Debug for KiroRefreshRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroRefreshRequest")
            .field("kind", &self.kind)
            .field("auth_region", &self.auth_region)
            .field("refresh_token_length", &self.refresh_token.len())
            .field("client_id", &self.client_id)
            .field(
                "client_secret_length",
                &self.client_secret.as_ref().map(|value| value.len()),
            )
            .finish()
    }
}

/// Bounded response bytes returned by an injected local refresh transport.
pub struct KiroRefreshResponse {
    body: Zeroizing<Vec<u8>>,
}

impl KiroRefreshResponse {
    /// Wraps bounded test/transport response bytes.
    #[must_use]
    pub fn new(body: impl Into<Vec<u8>>) -> Self {
        Self {
            body: Zeroizing::new(body.into()),
        }
    }

    fn body(&self) -> &[u8] {
        &self.body
    }
}
impl fmt::Debug for KiroRefreshResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroRefreshResponse")
            .field("body", &"<redacted>")
            .finish()
    }
}

/// Injected Social/Enterprise OAuth refresh transport. It owns all HTTP and endpoint details.
pub trait KiroRefreshTransport {
    /// Sends one refresh operation without implicit retry.
    ///
    /// # Errors
    ///
    /// Returns only a secret-free transport or response classification.
    fn refresh(
        &self,
        request: KiroRefreshRequest,
    ) -> Result<KiroRefreshResponse, KiroCredentialError>;
}

/// Secret-safe failures from the Kiro credential boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroCredentialError {
    /// Input exceeds the strict byte bound.
    InputTooLarge,
    /// Input is not one strict JSON object.
    InvalidJson,
    /// Input contains a field outside the selected credential shape.
    UnexpectedField,
    /// A required field is missing.
    MissingField,
    /// A field is empty, malformed, or has the wrong JSON type.
    InvalidField,
    /// The selected credential kind is unsupported.
    InvalidKind,
    /// An API key is not a `ksk_` key.
    InvalidApiKey,
    /// An Enterprise auth Region is malformed.
    InvalidRegion,
    /// An expiry or lifetime cannot be represented safely.
    InvalidTimestamp,
    /// A refresh response does not match the strict token shape.
    InvalidRefreshResponse,
    /// An API key was asked to refresh.
    NotRefreshable,
    /// A caller requested a secret not held by this credential kind.
    WrongCredentialKind,
    /// The injected transport could not complete the one refresh operation.
    TransportUnavailable,
    /// Authenticated encryption or decryption failed.
    EncryptionFailed,
    /// An authenticated persisted payload has an invalid format.
    InvalidPersistedCredential,
    /// The same Credential ID was supplied more than once to a runtime coordinator.
    DuplicateCredentialId,
    /// A requested runtime Credential does not exist.
    RuntimeCredentialMissing,
    /// The in-process runtime lock is unavailable.
    RuntimeUnavailable,
    /// The caller selected a zero same-key refresh wait bound.
    InvalidRefreshWaitTimeout,
    /// A follower waited too long for an in-flight same-Credential refresh.
    RefreshWaitTimedOut,
    /// A peer completed or failed an in-flight refresh and the caller must reload state.
    RefreshCompletedByPeer,
    /// A leader's source revision changed while its refresh transport call was in flight.
    ConcurrentCredentialStateChanged,
    /// A revision cannot advance without overflowing its durable numeric domain.
    RevisionOverflow,
}

impl fmt::Display for KiroCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InputTooLarge => "Kiro credential input exceeds its bound",
            Self::InvalidJson => "Kiro credential input is not strict JSON",
            Self::UnexpectedField => "Kiro credential input has an unsupported field",
            Self::MissingField => "Kiro credential input is missing a required field",
            Self::InvalidField => "Kiro credential input has an invalid field",
            Self::InvalidKind => "Kiro credential kind is unsupported",
            Self::InvalidApiKey => "Kiro API key is invalid",
            Self::InvalidRegion => "Kiro auth Region is invalid",
            Self::InvalidTimestamp => "Kiro credential expiry is invalid",
            Self::InvalidRefreshResponse => "Kiro refresh response is invalid",
            Self::NotRefreshable => "Kiro API key credentials are not refreshable",
            Self::WrongCredentialKind => "Kiro credential kind cannot provide that secret",
            Self::TransportUnavailable => "Kiro refresh transport is unavailable",
            Self::EncryptionFailed => "Kiro credential encryption failed",
            Self::InvalidPersistedCredential => "persisted Kiro credential is invalid",
            Self::DuplicateCredentialId => "Kiro runtime Credential ID is duplicated",
            Self::RuntimeCredentialMissing => "Kiro runtime Credential is missing",
            Self::RuntimeUnavailable => "Kiro credential runtime is unavailable",
            Self::InvalidRefreshWaitTimeout => "Kiro refresh wait timeout is invalid",
            Self::RefreshWaitTimedOut => "Kiro Credential refresh wait timed out",
            Self::RefreshCompletedByPeer => "Kiro Credential refresh completed by another request",
            Self::ConcurrentCredentialStateChanged => "Kiro Credential changed during refresh",
            Self::RevisionOverflow => "Kiro Credential revision is invalid",
        })
    }
}
impl Error for KiroCredentialError {}

fn strict_object(input: &[u8]) -> Result<BTreeMap<String, Value>, KiroCredentialError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(KiroCredentialError::InputTooLarge);
    }
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let object = StrictObject::deserialize(&mut deserializer)
        .map_err(|_| KiroCredentialError::InvalidJson)?
        .0;
    deserializer
        .end()
        .map_err(|_| KiroCredentialError::InvalidJson)?;
    Ok(object)
}
struct StrictObject(BTreeMap<String, Value>);
impl<'de> Deserialize<'de> for StrictObject {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictObject;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON object without duplicate fields")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    if values.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate field"));
                    }
                }
                Ok(StrictObject(values))
            }
        }
        deserializer.deserialize_map(StrictVisitor)
    }
}
fn require_exact_fields(
    object: &BTreeMap<String, Value>,
    allowed: &[&str],
) -> Result<(), KiroCredentialError> {
    if object
        .keys()
        .any(|field| !allowed.contains(&field.as_str()))
    {
        return Err(KiroCredentialError::UnexpectedField);
    }
    Ok(())
}
fn required_string<'a>(
    object: &'a BTreeMap<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<&'a str, KiroCredentialError> {
    let value = object
        .get(field)
        .ok_or(KiroCredentialError::MissingField)?
        .as_str()
        .ok_or(KiroCredentialError::InvalidField)?;
    validate_non_secret(value, maximum)?;
    Ok(value)
}
fn optional_string<'a>(
    object: &'a BTreeMap<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Option<&'a str>, KiroCredentialError> {
    object
        .get(field)
        .map(|_| required_string(object, field, maximum))
        .transpose()
}
fn required_secret(
    object: &BTreeMap<String, Value>,
    field: &str,
) -> Result<String, KiroCredentialError> {
    let value = required_string(object, field, MAX_SECRET_BYTES)?;
    validate_secret(value)?;
    Ok(value.to_owned())
}
fn optional_secret(
    object: &BTreeMap<String, Value>,
    field: &str,
) -> Result<Option<String>, KiroCredentialError> {
    object
        .get(field)
        .map(|_| required_secret(object, field))
        .transpose()
}
fn required_region(
    object: &BTreeMap<String, Value>,
    field: &str,
) -> Result<String, KiroCredentialError> {
    let value = required_string(object, field, MAX_REGION_BYTES)?;
    validate_region_value(value)?;
    Ok(value.to_owned())
}
fn required_expiry(
    object: &BTreeMap<String, Value>,
    observed_at_ms: i64,
) -> Result<i64, KiroCredentialError> {
    let value = object
        .get("expires_at_ms")
        .ok_or(KiroCredentialError::MissingField)?
        .as_i64()
        .ok_or(KiroCredentialError::InvalidTimestamp)?;
    if value <= observed_at_ms
        || value
            .checked_sub(observed_at_ms)
            .is_none_or(|remaining| remaining > MAX_LIFETIME_MS)
    {
        return Err(KiroCredentialError::InvalidTimestamp);
    }
    Ok(value)
}
fn required_lifetime_ms(object: &BTreeMap<String, Value>) -> Result<i64, KiroCredentialError> {
    let seconds = object
        .get("expires_in")
        .ok_or(KiroCredentialError::MissingField)?
        .as_u64()
        .ok_or(KiroCredentialError::InvalidRefreshResponse)?;
    let milliseconds = seconds
        .checked_mul(1_000)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or(KiroCredentialError::InvalidTimestamp)?;
    if milliseconds <= 0 || milliseconds > MAX_LIFETIME_MS {
        return Err(KiroCredentialError::InvalidTimestamp);
    }
    Ok(milliseconds)
}
fn validate_non_secret(value: &str, maximum: usize) -> Result<(), KiroCredentialError> {
    if value.is_empty()
        || value.len() > maximum
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        Err(KiroCredentialError::InvalidField)
    } else {
        Ok(())
    }
}
fn validate_secret(value: &str) -> Result<(), KiroCredentialError> {
    validate_non_secret(value, MAX_SECRET_BYTES)
}
fn validate_region_value(value: &str) -> Result<(), KiroCredentialError> {
    if value.len() > MAX_REGION_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || value.split('-').count() < 3
    {
        Err(KiroCredentialError::InvalidRegion)
    } else {
        Ok(())
    }
}
fn associated_data(credential_id: &CredentialId) -> Vec<u8> {
    let mut output = Vec::with_capacity(AAD_DOMAIN.len() + 1 + credential_id.as_str().len());
    output.extend_from_slice(AAD_DOMAIN);
    output.push(0);
    output.extend_from_slice(credential_id.as_str().as_bytes());
    output
}
fn source_byte(source: KiroCredentialSource) -> u8 {
    match source {
        KiroCredentialSource::ImportedJson => 0,
        KiroCredentialSource::Refresh => 1,
        KiroCredentialSource::RuntimeLease => 2,
    }
}
fn source_from_byte(value: u8) -> Result<KiroCredentialSource, KiroCredentialError> {
    match value {
        0 => Ok(KiroCredentialSource::ImportedJson),
        1 => Ok(KiroCredentialSource::Refresh),
        2 => Ok(KiroCredentialSource::RuntimeLease),
        _ => Err(KiroCredentialError::InvalidPersistedCredential),
    }
}
fn write_segment(output: &mut Vec<u8>, value: &str) -> Result<(), KiroCredentialError> {
    let length =
        u16::try_from(value.len()).map_err(|_| KiroCredentialError::InvalidPersistedCredential)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}
fn read_byte(input: &[u8], cursor: &mut usize) -> Result<u8, KiroCredentialError> {
    let value = *input
        .get(*cursor)
        .ok_or(KiroCredentialError::InvalidPersistedCredential)?;
    *cursor += 1;
    Ok(value)
}
fn read_i64(input: &[u8], cursor: &mut usize) -> Result<i64, KiroCredentialError> {
    let end = cursor
        .checked_add(8)
        .ok_or(KiroCredentialError::InvalidPersistedCredential)?;
    let bytes: [u8; 8] = input
        .get(*cursor..end)
        .ok_or(KiroCredentialError::InvalidPersistedCredential)?
        .try_into()
        .map_err(|_| KiroCredentialError::InvalidPersistedCredential)?;
    *cursor = end;
    Ok(i64::from_be_bytes(bytes))
}
fn read_segment(input: &[u8], cursor: &mut usize) -> Result<String, KiroCredentialError> {
    let end_length = cursor
        .checked_add(2)
        .ok_or(KiroCredentialError::InvalidPersistedCredential)?;
    let length = u16::from_be_bytes(
        input
            .get(*cursor..end_length)
            .ok_or(KiroCredentialError::InvalidPersistedCredential)?
            .try_into()
            .map_err(|_| KiroCredentialError::InvalidPersistedCredential)?,
    ) as usize;
    *cursor = end_length;
    let end = cursor
        .checked_add(length)
        .ok_or(KiroCredentialError::InvalidPersistedCredential)?;
    let value = std::str::from_utf8(
        input
            .get(*cursor..end)
            .ok_or(KiroCredentialError::InvalidPersistedCredential)?,
    )
    .map_err(|_| KiroCredentialError::InvalidPersistedCredential)?
    .to_owned();
    *cursor = end;
    Ok(value)
}
