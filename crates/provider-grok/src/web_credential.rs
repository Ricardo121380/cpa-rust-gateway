//! Grok Web SSO cookie credentials and isolated revisioned lifecycle state.
//!
//! This boundary accepts only an explicitly supplied, bounded JSON export. It does not read a
//! browser profile, cookie jar, file, environment, OAuth cache, proxy, route, or server account.
//! P9-02 owns the later browser egress session and may borrow a validated cookie only while
//! constructing one request.

use std::{collections::BTreeSet, error::Error, fmt, net::IpAddr, sync::Mutex};

use gateway_store::secret_store::{EncryptedSecret, SecretStore};
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::strict_json::parse_strict_json;

const MAX_CREDENTIAL_JSON_BYTES: usize = 64 * 1024;
const MAX_ACCOUNT_REFERENCE_BYTES: usize = 128;
const MAX_LINEAGE_REFERENCE_BYTES: usize = 128;
const MAX_COOKIE_COUNT: usize = 32;
const MAX_COOKIE_NAME_BYTES: usize = 128;
const MAX_COOKIE_VALUE_BYTES: usize = 16 * 1024;
const MAX_COOKIE_DOMAIN_BYTES: usize = 253;
const MAX_COOKIE_PATH_BYTES: usize = 512;
const MAX_SESSION_LIFETIME_MS: i64 = 90 * 24 * 60 * 60 * 1_000;
const PERSISTED_CREDENTIAL_FORMAT_VERSION: u8 = 1;
const WEB_CREDENTIAL_AAD_DOMAIN: &[u8] = b"cpa-rust-gateway/grok-web/sso-credential/v1";

/// Stable Provider identity for the browser-session Web surface.
pub const GROK_WEB_PROVIDER_ID: &str = "grok.web";

/// Non-secret source class for a Web session credential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebCredentialSource {
    /// An operator explicitly supplied a bounded SSO cookie export.
    ImportedSso,
}

/// Non-secret provenance retained for a Web SSO credential.
///
/// This records that a credential originated from a distinct SSO import without retaining an
/// email, browser profile path, OAuth value, Cookie value, or Build credential identity.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebCredentialLineage {
    source: GrokWebCredentialSource,
    reference: String,
}

impl GrokWebCredentialLineage {
    /// Returns the source class for this independent Web credential.
    #[must_use]
    pub const fn source(&self) -> GrokWebCredentialSource {
        self.source
    }

    /// Returns the bounded opaque lineage reference.
    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }
}

impl fmt::Debug for GrokWebCredentialLineage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebCredentialLineage")
            .field("source", &self.source)
            .field("reference", &self.reference)
            .finish()
    }
}

/// One validated browser cookie that is safe to hand to the later P9-02 session constructor.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebSessionCookie {
    name: String,
    value: Zeroizing<String>,
    domain: String,
    path: String,
    secure: bool,
    http_only: bool,
}

impl GrokWebSessionCookie {
    /// Returns the validated cookie name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Borrows the cookie value only for immediate P9-02 header construction.
    #[must_use]
    pub fn value(&self) -> &str {
        self.value.as_str()
    }

    /// Returns the canonical cookie domain without exposing its value through `Debug` on the
    /// enclosing credential.
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Returns the validated cookie path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns whether the cookie must be sent only over a secure transport.
    #[must_use]
    pub const fn secure(&self) -> bool {
        self.secure
    }

    /// Returns whether script access is disabled for this cookie.
    #[must_use]
    pub const fn http_only(&self) -> bool {
        self.http_only
    }
}

impl fmt::Debug for GrokWebSessionCookie {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebSessionCookie")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .field("domain", &self.domain)
            .field("path", &self.path)
            .field("secure", &self.secure)
            .field("http_only", &self.http_only)
            .finish()
    }
}

/// Strict, isolated Grok Web SSO session credential.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebCredential {
    account_reference: String,
    lineage: GrokWebCredentialLineage,
    cookies: Vec<GrokWebSessionCookie>,
    expires_at_ms: i64,
    revision: u64,
}

impl GrokWebCredential {
    /// Imports one strict Grok Web SSO cookie export at the supplied observation time.
    ///
    /// The accepted shape is deliberately narrow: `kind`, opaque `account_ref`, opaque
    /// `lineage_ref`, non-negative `revision`, absolute `expires_at_ms`, and one or more scoped
    /// secure Cookie objects. Unknown or duplicate JSON fields, expired sessions, private/IP
    /// cookie domains, unsafe Cookie characters, and duplicate Cookie scopes fail closed.
    ///
    /// # Errors
    ///
    /// Returns a classification-only error without retaining or rendering Cookie values.
    pub fn import_sso_json(
        input: &[u8],
        observed_at_ms: i64,
    ) -> Result<Self, GrokWebCredentialError> {
        let value = parse_strict_json(input, MAX_CREDENTIAL_JSON_BYTES)
            .map_err(|()| GrokWebCredentialError::InvalidJson)?;
        let object = value
            .as_object()
            .ok_or(GrokWebCredentialError::InvalidField)?;
        ensure_known_fields(
            object,
            &[
                "kind",
                "account_ref",
                "lineage_ref",
                "revision",
                "expires_at_ms",
                "cookies",
            ],
        )?;
        if required_string(object, "kind")? != "grok_web_sso" {
            return Err(GrokWebCredentialError::InvalidField);
        }
        let account_reference =
            required_opaque_reference(object, "account_ref", MAX_ACCOUNT_REFERENCE_BYTES)?;
        let lineage_reference =
            required_opaque_reference(object, "lineage_ref", MAX_LINEAGE_REFERENCE_BYTES)?;
        let revision = required_revision(object, "revision")?;
        let expires_at_ms = required_positive_timestamp(object, "expires_at_ms")?;
        let latest_expiry = observed_at_ms
            .checked_add(MAX_SESSION_LIFETIME_MS)
            .ok_or(GrokWebCredentialError::InvalidTimestamp)?;
        if observed_at_ms < 0 || expires_at_ms <= observed_at_ms || expires_at_ms > latest_expiry {
            return Err(GrokWebCredentialError::InvalidTimestamp);
        }
        let cookies = required_cookies(object)?;
        Ok(Self {
            account_reference,
            lineage: GrokWebCredentialLineage {
                source: GrokWebCredentialSource::ImportedSso,
                reference: lineage_reference,
            },
            cookies,
            expires_at_ms,
            revision,
        })
    }

    /// Normalizes an otherwise valid migration credential to the local 90-day session window.
    ///
    /// This is deliberately narrower than [`Self::import_sso_json`]: direct callers retain the
    /// strict overlong-session rejection contract. A controlled migration may accept a source
    /// Cookie whose upstream expiry is farther away, but CPAR persists only an effective expiry of
    /// `min(source_expiry, observed_at_ms + 90 days)`. The returned boolean makes that reduction
    /// visible in a value-free migration receipt.
    ///
    /// # Errors
    ///
    /// Expired, malformed, overflowed, unsafe, duplicate, or otherwise invalid credentials still
    /// fail closed. The normalized bytes and all Cookie values remain zeroized by the caller.
    pub(crate) fn normalize_sso_json_for_migration(
        input: &[u8],
        observed_at_ms: i64,
    ) -> Result<(Zeroizing<Vec<u8>>, bool), GrokWebCredentialError> {
        let mut value = parse_strict_json(input, MAX_CREDENTIAL_JSON_BYTES)
            .map_err(|()| GrokWebCredentialError::InvalidJson)?;
        let object = value
            .as_object_mut()
            .ok_or(GrokWebCredentialError::InvalidField)?;
        let source_expiry = required_positive_timestamp(object, "expires_at_ms")?;
        let latest_expiry = observed_at_ms
            .checked_add(MAX_SESSION_LIFETIME_MS)
            .ok_or(GrokWebCredentialError::InvalidTimestamp)?;
        if observed_at_ms < 0 || source_expiry <= observed_at_ms {
            return Err(GrokWebCredentialError::InvalidTimestamp);
        }
        if source_expiry <= latest_expiry {
            Self::import_sso_json(input, observed_at_ms)?;
            return Ok((Zeroizing::new(input.to_vec()), false));
        }
        object.insert(
            "expires_at_ms".to_owned(),
            Value::Number(latest_expiry.into()),
        );
        let normalized =
            serde_json::to_vec(&value).map_err(|_| GrokWebCredentialError::InvalidJson)?;
        Self::import_sso_json(&normalized, observed_at_ms)?;
        Ok((Zeroizing::new(normalized), true))
    }

    /// Returns the provider identity; it is never inferred from a model name.
    #[must_use]
    pub const fn provider_id() -> &'static str {
        GROK_WEB_PROVIDER_ID
    }

    /// Returns the opaque, non-PII account reference.
    #[must_use]
    pub fn account_reference(&self) -> &str {
        &self.account_reference
    }

    /// Returns the non-secret source lineage.
    #[must_use]
    pub const fn lineage(&self) -> &GrokWebCredentialLineage {
        &self.lineage
    }

    /// Returns the fixed, validated Cookie scopes for immediate P9-02 session construction.
    #[must_use]
    pub fn cookies(&self) -> &[GrokWebSessionCookie] {
        &self.cookies
    }

    /// Returns the absolute Unix-millisecond session expiry.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }

    /// Returns the monotonic credential revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns whether this credential may no longer start a request at the supplied time.
    #[must_use]
    pub const fn is_expired_at(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }

    /// Seals this exact Web credential into an authenticated, redacted envelope.
    ///
    /// The envelope has no filesystem, database, browser, proxy, or network side effect. Its
    /// associated data binds the provider, opaque account and lineage references, revision, and
    /// expiry, so a future persistence boundary cannot substitute an envelope across Web
    /// identities or credential versions.
    ///
    /// # Errors
    ///
    /// Returns a classification-only error if the bounded state cannot be serialized or sealed.
    pub fn seal(
        &self,
        secret_store: &SecretStore,
    ) -> Result<GrokWebCredentialEnvelope, GrokWebCredentialError> {
        let plaintext = self.persisted_bytes()?;
        let associated_data = self.associated_data()?;
        let encrypted = secret_store
            .seal(plaintext.as_slice(), &associated_data)
            .map_err(|_| GrokWebCredentialError::SecretStoreFailure)?;
        Ok(GrokWebCredentialEnvelope {
            encrypted,
            account_reference: self.account_reference.clone(),
            lineage: self.lineage.clone(),
            expires_at_ms: self.expires_at_ms,
            revision: self.revision,
        })
    }

    fn persisted_bytes(&self) -> Result<Zeroizing<Vec<u8>>, GrokWebCredentialError> {
        validate_opaque_reference(&self.account_reference, MAX_ACCOUNT_REFERENCE_BYTES)?;
        validate_opaque_reference(&self.lineage.reference, MAX_LINEAGE_REFERENCE_BYTES)?;
        let cookie_count = u8::try_from(self.cookies.len())
            .map_err(|_| GrokWebCredentialError::InvalidPersistedCredential)?;
        if cookie_count == 0 || usize::from(cookie_count) > MAX_COOKIE_COUNT {
            return Err(GrokWebCredentialError::InvalidPersistedCredential);
        }
        let mut output = Zeroizing::new(Vec::new());
        output.push(PERSISTED_CREDENTIAL_FORMAT_VERSION);
        write_persisted_segment(&mut output, &self.account_reference)?;
        write_persisted_segment(&mut output, &self.lineage.reference)?;
        output.extend_from_slice(&self.revision.to_be_bytes());
        output.extend_from_slice(&self.expires_at_ms.to_be_bytes());
        output.push(cookie_count);
        let mut scopes = BTreeSet::new();
        for cookie in &self.cookies {
            validate_cookie(cookie)?;
            let scope = (
                cookie.name.clone(),
                cookie.domain.clone(),
                cookie.path.clone(),
            );
            if !scopes.insert(scope) {
                return Err(GrokWebCredentialError::InvalidPersistedCredential);
            }
            write_persisted_segment(&mut output, &cookie.name)?;
            write_persisted_segment(&mut output, cookie.value())?;
            write_persisted_segment(&mut output, &cookie.domain)?;
            write_persisted_segment(&mut output, &cookie.path)?;
            output.push(u8::from(cookie.secure));
            output.push(u8::from(cookie.http_only));
        }
        Ok(output)
    }

    fn from_persisted_bytes(input: &[u8]) -> Result<Self, GrokWebCredentialError> {
        if input.len() > MAX_CREDENTIAL_JSON_BYTES {
            return Err(GrokWebCredentialError::InvalidPersistedCredential);
        }
        let mut cursor = 0;
        if read_persisted_byte(input, &mut cursor)? != PERSISTED_CREDENTIAL_FORMAT_VERSION {
            return Err(GrokWebCredentialError::InvalidPersistedCredential);
        }
        let account_reference = read_persisted_segment(input, &mut cursor)?;
        validate_opaque_reference(account_reference, MAX_ACCOUNT_REFERENCE_BYTES)?;
        let lineage_reference = read_persisted_segment(input, &mut cursor)?;
        validate_opaque_reference(lineage_reference, MAX_LINEAGE_REFERENCE_BYTES)?;
        let revision = u64::from_be_bytes(
            read_persisted_bytes(input, &mut cursor, std::mem::size_of::<u64>())?
                .try_into()
                .map_err(|_| GrokWebCredentialError::InvalidPersistedCredential)?,
        );
        let expires_at_ms = i64::from_be_bytes(
            read_persisted_bytes(input, &mut cursor, std::mem::size_of::<i64>())?
                .try_into()
                .map_err(|_| GrokWebCredentialError::InvalidPersistedCredential)?,
        );
        if expires_at_ms <= 0 {
            return Err(GrokWebCredentialError::InvalidPersistedCredential);
        }
        let cookie_count = usize::from(read_persisted_byte(input, &mut cursor)?);
        if cookie_count == 0 || cookie_count > MAX_COOKIE_COUNT {
            return Err(GrokWebCredentialError::InvalidPersistedCredential);
        }
        let mut scopes = BTreeSet::new();
        let mut cookies = Vec::with_capacity(cookie_count);
        for _ in 0..cookie_count {
            let name = read_persisted_segment(input, &mut cursor)?;
            let value = read_persisted_segment(input, &mut cursor)?;
            let domain = read_persisted_segment(input, &mut cursor)?;
            let path = read_persisted_segment(input, &mut cursor)?;
            let secure = read_persisted_bool(input, &mut cursor)?;
            let http_only = read_persisted_bool(input, &mut cursor)?;
            let cookie = GrokWebSessionCookie {
                name: name.to_owned(),
                value: Zeroizing::new(value.to_owned()),
                domain: domain.to_owned(),
                path: path.to_owned(),
                secure,
                http_only,
            };
            validate_cookie(&cookie)?;
            let scope = (
                cookie.name.clone(),
                cookie.domain.clone(),
                cookie.path.clone(),
            );
            if !scopes.insert(scope) {
                return Err(GrokWebCredentialError::InvalidPersistedCredential);
            }
            cookies.push(cookie);
        }
        if cursor != input.len() {
            return Err(GrokWebCredentialError::InvalidPersistedCredential);
        }
        Ok(Self {
            account_reference: account_reference.to_owned(),
            lineage: GrokWebCredentialLineage {
                source: GrokWebCredentialSource::ImportedSso,
                reference: lineage_reference.to_owned(),
            },
            cookies,
            expires_at_ms,
            revision,
        })
    }

    fn associated_data(&self) -> Result<Vec<u8>, GrokWebCredentialError> {
        credential_associated_data(
            &self.account_reference,
            &self.lineage.reference,
            self.revision,
            self.expires_at_ms,
        )
    }
}

impl fmt::Debug for GrokWebCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebCredential")
            .field("provider_id", &GROK_WEB_PROVIDER_ID)
            .field("account_reference", &self.account_reference)
            .field("lineage", &self.lineage)
            .field("cookie_count", &self.cookies.len())
            .field("expires_at_ms", &self.expires_at_ms)
            .field("revision", &self.revision)
            .finish()
    }
}

/// Authenticated encrypted envelope for one exact Grok Web SSO credential version.
///
/// It is intentionally storage-neutral. A later persistence boundary may retain its opaque
/// ciphertext and non-secret identity metadata, but this type neither discovers a storage path
/// nor performs an I/O operation.
pub struct GrokWebCredentialEnvelope {
    encrypted: EncryptedSecret,
    account_reference: String,
    lineage: GrokWebCredentialLineage,
    expires_at_ms: i64,
    revision: u64,
}

impl GrokWebCredentialEnvelope {
    /// Opens this exact authenticated envelope and rejects an expired recovered session.
    ///
    /// # Errors
    ///
    /// Returns a safe error when authentication, structural validation, metadata binding, or
    /// expiry validation fails. It never renders ciphertext or Cookie values.
    pub fn open(
        &self,
        secret_store: &SecretStore,
        observed_at_ms: i64,
    ) -> Result<GrokWebCredential, GrokWebCredentialError> {
        let associated_data = credential_associated_data(
            &self.account_reference,
            &self.lineage.reference,
            self.revision,
            self.expires_at_ms,
        )?;
        let plaintext = secret_store
            .open(&self.encrypted, &associated_data)
            .map_err(|_| GrokWebCredentialError::SecretStoreFailure)?;
        let credential = GrokWebCredential::from_persisted_bytes(plaintext.as_bytes())?;
        if credential.account_reference != self.account_reference
            || credential.lineage != self.lineage
            || credential.revision != self.revision
            || credential.expires_at_ms != self.expires_at_ms
            || credential.is_expired_at(observed_at_ms)
        {
            return Err(GrokWebCredentialError::InvalidPersistedCredential);
        }
        Ok(credential)
    }

    /// Returns the opaque AEAD envelope for a dedicated persistence boundary.
    ///
    /// The returned bytes are never plaintext and must not be logged.
    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        self.encrypted.ciphertext()
    }
}

impl fmt::Debug for GrokWebCredentialEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebCredentialEnvelope")
            .field("encrypted", &self.encrypted)
            .field("account_reference", &self.account_reference)
            .field("lineage", &self.lineage)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("revision", &self.revision)
            .finish()
    }
}

/// Safe result of an expected-revision Web credential replacement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebCredentialCasOutcome {
    /// The replacement became the current credential.
    Replaced,
    /// The expected revision was stale; no credential changed.
    Conflict,
}

/// Thread-safe, provider-private Web credential lifecycle slot.
///
/// It intentionally has no Build/Official/Kiro input and no persistence or I/O. P9-02 must bind
/// any `BrowserEgressSession` to the exact revision it leased; a stale response can therefore not
/// overwrite a newer SSO session.
pub struct GrokWebCredentialSlot {
    credential: Mutex<GrokWebCredential>,
}

impl GrokWebCredentialSlot {
    /// Creates one independent in-process lifecycle slot from a validated Web credential.
    #[must_use]
    pub fn new(credential: GrokWebCredential) -> Self {
        Self {
            credential: Mutex::new(credential),
        }
    }

    /// Returns a redacted clone of the currently selected credential version.
    ///
    /// # Errors
    ///
    /// Returns a safe state error if an in-process holder panicked while owning the slot.
    pub fn load(&self) -> Result<GrokWebCredential, GrokWebCredentialError> {
        self.credential
            .lock()
            .map(|credential| credential.clone())
            .map_err(|_| GrokWebCredentialError::StateUnavailable)
    }

    /// Replaces the exact current revision only with a same-account, same-lineage next revision.
    ///
    /// # Errors
    ///
    /// Returns a safe error before mutation when the replacement changes account/lineage or skips
    /// a revision. A normal stale expected revision returns [`GrokWebCredentialCasOutcome::Conflict`].
    pub fn compare_and_replace(
        &self,
        expected_revision: u64,
        replacement: GrokWebCredential,
    ) -> Result<GrokWebCredentialCasOutcome, GrokWebCredentialError> {
        let mut current = self
            .credential
            .lock()
            .map_err(|_| GrokWebCredentialError::StateUnavailable)?;
        if current.revision != expected_revision {
            return Ok(GrokWebCredentialCasOutcome::Conflict);
        }
        if current.account_reference != replacement.account_reference
            || current.lineage != replacement.lineage
        {
            return Err(GrokWebCredentialError::LineageMismatch);
        }
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or(GrokWebCredentialError::InvalidRevision)?;
        if replacement.revision != next_revision {
            return Err(GrokWebCredentialError::InvalidRevision);
        }
        *current = replacement;
        Ok(GrokWebCredentialCasOutcome::Replaced)
    }
}

impl fmt::Debug for GrokWebCredentialSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.credential.lock() {
            Ok(credential) => formatter
                .debug_struct("GrokWebCredentialSlot")
                .field("credential", &credential)
                .finish(),
            Err(_) => formatter.write_str("GrokWebCredentialSlot(<unavailable>)"),
        }
    }
}

/// Safe invalid-input or lifecycle classification for a Grok Web credential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebCredentialError {
    /// The input was oversized, malformed, duplicated, or had trailing JSON bytes.
    InvalidJson,
    /// A required field was absent.
    MissingField,
    /// A value, Cookie scope, or unknown field was invalid.
    InvalidField,
    /// The absolute session expiry was expired, non-positive, or outside the bounded lifetime.
    InvalidTimestamp,
    /// The imported revision or an expected next revision was invalid.
    InvalidRevision,
    /// A sealed credential payload was malformed, mismatched, or expired.
    InvalidPersistedCredential,
    /// Authenticated encryption or decryption of a Web credential failed.
    SecretStoreFailure,
    /// More than one Cookie shared the same name/domain/path scope.
    DuplicateCookieScope,
    /// A replacement attempted to change its independent account or lineage.
    LineageMismatch,
    /// The in-process lifecycle slot was unavailable.
    StateUnavailable,
}

impl fmt::Display for GrokWebCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidJson => "Grok Web credential JSON is invalid",
            Self::MissingField => "Grok Web credential is incomplete",
            Self::InvalidField => "Grok Web credential field is invalid",
            Self::InvalidTimestamp => "Grok Web credential session lifetime is invalid",
            Self::InvalidRevision => "Grok Web credential revision is invalid",
            Self::InvalidPersistedCredential => "Grok Web credential sealed state is invalid",
            Self::SecretStoreFailure => "Grok Web credential secret handling failed",
            Self::DuplicateCookieScope => "Grok Web credential Cookie scope is ambiguous",
            Self::LineageMismatch => "Grok Web credential replacement lineage is invalid",
            Self::StateUnavailable => "Grok Web credential lifecycle state is unavailable",
        })
    }
}

impl Error for GrokWebCredentialError {}

fn ensure_known_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), GrokWebCredentialError> {
    if object.keys().all(|key| allowed.contains(&key.as_str())) {
        Ok(())
    } else {
        Err(GrokWebCredentialError::InvalidField)
    }
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, GrokWebCredentialError> {
    object
        .get(name)
        .ok_or(GrokWebCredentialError::MissingField)?
        .as_str()
        .ok_or(GrokWebCredentialError::InvalidField)
}

fn required_opaque_reference(
    object: &Map<String, Value>,
    name: &str,
    maximum_bytes: usize,
) -> Result<String, GrokWebCredentialError> {
    let value = required_string(object, name)?;
    validate_opaque_reference(value, maximum_bytes)?;
    Ok(value.to_owned())
}

fn required_revision(
    object: &Map<String, Value>,
    name: &str,
) -> Result<u64, GrokWebCredentialError> {
    object
        .get(name)
        .ok_or(GrokWebCredentialError::MissingField)?
        .as_u64()
        .ok_or(GrokWebCredentialError::InvalidRevision)
}

fn required_positive_timestamp(
    object: &Map<String, Value>,
    name: &str,
) -> Result<i64, GrokWebCredentialError> {
    let value = object
        .get(name)
        .ok_or(GrokWebCredentialError::MissingField)?
        .as_i64()
        .ok_or(GrokWebCredentialError::InvalidTimestamp)?;
    (value > 0)
        .then_some(value)
        .ok_or(GrokWebCredentialError::InvalidTimestamp)
}

fn required_cookies(
    object: &Map<String, Value>,
) -> Result<Vec<GrokWebSessionCookie>, GrokWebCredentialError> {
    let values = object
        .get("cookies")
        .ok_or(GrokWebCredentialError::MissingField)?
        .as_array()
        .ok_or(GrokWebCredentialError::InvalidField)?;
    if values.is_empty() || values.len() > MAX_COOKIE_COUNT {
        return Err(GrokWebCredentialError::InvalidField);
    }
    let mut scopes = BTreeSet::new();
    let mut cookies = Vec::with_capacity(values.len());
    for value in values {
        let object = value
            .as_object()
            .ok_or(GrokWebCredentialError::InvalidField)?;
        ensure_known_fields(
            object,
            &["name", "value", "domain", "path", "secure", "http_only"],
        )?;
        let name = required_cookie_name(object)?;
        let value = required_cookie_value(object)?;
        let domain = required_cookie_domain(object)?;
        let path = required_cookie_path(object)?;
        let secure = object
            .get("secure")
            .ok_or(GrokWebCredentialError::MissingField)?
            .as_bool()
            .ok_or(GrokWebCredentialError::InvalidField)?;
        let http_only = object
            .get("http_only")
            .ok_or(GrokWebCredentialError::MissingField)?
            .as_bool()
            .ok_or(GrokWebCredentialError::InvalidField)?;
        if !secure {
            return Err(GrokWebCredentialError::InvalidField);
        }
        let scope = (name.clone(), domain.clone(), path.clone());
        if !scopes.insert(scope) {
            return Err(GrokWebCredentialError::DuplicateCookieScope);
        }
        cookies.push(GrokWebSessionCookie {
            name,
            value: Zeroizing::new(value),
            domain,
            path,
            secure,
            http_only,
        });
    }
    Ok(cookies)
}

fn required_cookie_name(object: &Map<String, Value>) -> Result<String, GrokWebCredentialError> {
    let value = required_string(object, "name")?;
    validate_cookie_name(value)?;
    Ok(value.to_owned())
}

fn required_cookie_value(object: &Map<String, Value>) -> Result<String, GrokWebCredentialError> {
    let value = required_string(object, "value")?;
    validate_cookie_value(value)?;
    Ok(value.to_owned())
}

fn required_cookie_domain(object: &Map<String, Value>) -> Result<String, GrokWebCredentialError> {
    let value = required_string(object, "domain")?;
    let normalized = value
        .strip_prefix('.')
        .unwrap_or(value)
        .to_ascii_lowercase();
    validate_cookie_domain(&normalized)?;
    Ok(normalized)
}

fn required_cookie_path(object: &Map<String, Value>) -> Result<String, GrokWebCredentialError> {
    let value = required_string(object, "path")?;
    validate_cookie_path(value)?;
    Ok(value.to_owned())
}

fn validate_opaque_reference(
    value: &str,
    maximum_bytes: usize,
) -> Result<(), GrokWebCredentialError> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(GrokWebCredentialError::InvalidField);
    }
    Ok(())
}

fn validate_cookie(cookie: &GrokWebSessionCookie) -> Result<(), GrokWebCredentialError> {
    validate_cookie_name(&cookie.name)?;
    validate_cookie_value(cookie.value())?;
    validate_cookie_domain(&cookie.domain)?;
    validate_cookie_path(&cookie.path)?;
    if !cookie.secure {
        return Err(GrokWebCredentialError::InvalidField);
    }
    Ok(())
}

fn validate_cookie_name(value: &str) -> Result<(), GrokWebCredentialError> {
    if value.is_empty()
        || value.len() > MAX_COOKIE_NAME_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(GrokWebCredentialError::InvalidField);
    }
    Ok(())
}

fn validate_cookie_value(value: &str) -> Result<(), GrokWebCredentialError> {
    if value.is_empty()
        || value.len() > MAX_COOKIE_VALUE_BYTES
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || matches!(byte, b';' | b','))
    {
        return Err(GrokWebCredentialError::InvalidField);
    }
    Ok(())
}

fn validate_cookie_domain(value: &str) -> Result<(), GrokWebCredentialError> {
    if value.is_empty()
        || value.len() > MAX_COOKIE_DOMAIN_BYTES
        || value.parse::<IpAddr>().is_ok()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
        || value.starts_with('.')
        || value.ends_with('.')
        || value.contains("..")
        || !value.contains('.')
    {
        return Err(GrokWebCredentialError::InvalidField);
    }
    Ok(())
}

fn validate_cookie_path(value: &str) -> Result<(), GrokWebCredentialError> {
    if value.is_empty()
        || value.len() > MAX_COOKIE_PATH_BYTES
        || !value.starts_with('/')
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b';')
    {
        return Err(GrokWebCredentialError::InvalidField);
    }
    Ok(())
}

fn credential_associated_data(
    account_reference: &str,
    lineage_reference: &str,
    revision: u64,
    expires_at_ms: i64,
) -> Result<Vec<u8>, GrokWebCredentialError> {
    let mut output = Vec::with_capacity(
        WEB_CREDENTIAL_AAD_DOMAIN.len()
            + account_reference.len()
            + lineage_reference.len()
            + 2 * std::mem::size_of::<u16>()
            + std::mem::size_of::<u64>()
            + std::mem::size_of::<i64>(),
    );
    output.extend_from_slice(WEB_CREDENTIAL_AAD_DOMAIN);
    write_associated_data_segment(&mut output, account_reference)?;
    write_associated_data_segment(&mut output, lineage_reference)?;
    output.extend_from_slice(&revision.to_be_bytes());
    output.extend_from_slice(&expires_at_ms.to_be_bytes());
    Ok(output)
}

fn write_associated_data_segment(
    output: &mut Vec<u8>,
    value: &str,
) -> Result<(), GrokWebCredentialError> {
    let length = u16::try_from(value.len())
        .map_err(|_| GrokWebCredentialError::InvalidPersistedCredential)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_persisted_segment(
    output: &mut Vec<u8>,
    value: &str,
) -> Result<(), GrokWebCredentialError> {
    let length = u32::try_from(value.len())
        .map_err(|_| GrokWebCredentialError::InvalidPersistedCredential)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_persisted_byte(input: &[u8], cursor: &mut usize) -> Result<u8, GrokWebCredentialError> {
    let byte = *input
        .get(*cursor)
        .ok_or(GrokWebCredentialError::InvalidPersistedCredential)?;
    *cursor = cursor
        .checked_add(1)
        .ok_or(GrokWebCredentialError::InvalidPersistedCredential)?;
    Ok(byte)
}

fn read_persisted_bool(input: &[u8], cursor: &mut usize) -> Result<bool, GrokWebCredentialError> {
    match read_persisted_byte(input, cursor)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(GrokWebCredentialError::InvalidPersistedCredential),
    }
}

fn read_persisted_segment<'a>(
    input: &'a [u8],
    cursor: &mut usize,
) -> Result<&'a str, GrokWebCredentialError> {
    let length = u32::from_be_bytes(
        read_persisted_bytes(input, cursor, std::mem::size_of::<u32>())?
            .try_into()
            .map_err(|_| GrokWebCredentialError::InvalidPersistedCredential)?,
    );
    let length =
        usize::try_from(length).map_err(|_| GrokWebCredentialError::InvalidPersistedCredential)?;
    if length > MAX_CREDENTIAL_JSON_BYTES {
        return Err(GrokWebCredentialError::InvalidPersistedCredential);
    }
    let value = read_persisted_bytes(input, cursor, length)?;
    std::str::from_utf8(value).map_err(|_| GrokWebCredentialError::InvalidPersistedCredential)
}

fn read_persisted_bytes<'a>(
    input: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], GrokWebCredentialError> {
    let end = cursor
        .checked_add(length)
        .ok_or(GrokWebCredentialError::InvalidPersistedCredential)?;
    let value = input
        .get(*cursor..end)
        .ok_or(GrokWebCredentialError::InvalidPersistedCredential)?;
    *cursor = end;
    Ok(value)
}
