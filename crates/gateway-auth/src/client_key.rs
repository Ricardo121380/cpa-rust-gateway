//! Storage-neutral Client Key issuance and verification primitives.
//!
//! P2-04 intentionally does not query `SQLite` or replace the live P1 authenticator. It creates
//! the durable prefix/digest fields a later Repository and Snapshot authenticator will use.

#![deny(unsafe_code)]

use std::{error::Error, fmt, fs, path::Path};

#[cfg(unix)]
use std::{io::Read, os::unix::fs::OpenOptionsExt};

use gateway_core::{AccessGroupId, ClientKeyId};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

/// Exact byte length of one external Client Key Pepper.
pub const CLIENT_KEY_PEPPER_BYTES: usize = 32;

/// Exact byte length of a persisted HMAC-SHA256 Client Key digest.
pub const CLIENT_KEY_DIGEST_BYTES: usize = 32;

const PUBLIC_PREFIX_RANDOM_BYTES: usize = 8;
const SECRET_RANDOM_BYTES: usize = 32;
const PUBLIC_PREFIX_HEX_LENGTH: usize = PUBLIC_PREFIX_RANDOM_BYTES * 2;
const SECRET_HEX_LENGTH: usize = SECRET_RANDOM_BYTES * 2;
const CLIENT_KEY_SCHEME: &str = "rgw";
const CLIENT_KEY_SCHEME_SEPARATOR: char = '_';

type HmacSha256 = Hmac<Sha256>;

/// A dedicated external Pepper used only for Client Key HMAC calculations.
pub struct ClientKeyPepper {
    bytes: [u8; CLIENT_KEY_PEPPER_BYTES],
}

impl ClientKeyPepper {
    /// Copies and validates exact-size external Pepper material.
    ///
    /// # Errors
    ///
    /// Returns [`ClientKeyError::InvalidPepperLength`] unless `bytes` is exactly
    /// [`CLIENT_KEY_PEPPER_BYTES`] long.
    pub fn try_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, ClientKeyError> {
        let bytes = bytes.as_ref();
        if bytes.len() != CLIENT_KEY_PEPPER_BYTES {
            return Err(ClientKeyError::InvalidPepperLength {
                actual: bytes.len(),
            });
        }

        let mut pepper = [0_u8; CLIENT_KEY_PEPPER_BYTES];
        pepper.copy_from_slice(bytes);
        Ok(Self { bytes: pepper })
    }

    /// Loads an exact-size Pepper from one external direct regular file.
    ///
    /// The file must not be a symbolic link. On supported Unix deployment targets it is also
    /// opened with `O_NOFOLLOW`, so a symbolic link is not followed between metadata admission and
    /// reading.
    ///
    /// # Errors
    ///
    /// Returns a safe load error without including the Pepper, the complete Client Key, or file
    /// contents in diagnostics.
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, ClientKeyError> {
        let path = path.as_ref();
        let metadata = fs::symlink_metadata(path).map_err(|_| ClientKeyError::PepperLoadIo)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ClientKeyError::InvalidPepperFile);
        }

        let mut bytes = read_pepper_file(path)?;
        let pepper = Self::try_from_bytes(&bytes);
        bytes.zeroize();
        pepper
    }

    fn as_bytes(&self) -> &[u8; CLIENT_KEY_PEPPER_BYTES] {
        &self.bytes
    }
}

impl fmt::Debug for ClientKeyPepper {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClientKeyPepper(<redacted>)")
    }
}

impl Drop for ClientKeyPepper {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// The canonical public Prefix stored in `client_keys.prefix`.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct ClientKeyPrefix(String);

impl ClientKeyPrefix {
    /// Validates a persisted public Prefix in `rgw_<16 lowercase-hex>` form.
    ///
    /// # Errors
    ///
    /// Returns [`ClientKeyError::InvalidClientKeyPrefix`] for any other representation.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ClientKeyError> {
        let value = value.into();
        let Some(prefix_segment) = value.strip_prefix(CLIENT_KEY_SCHEME) else {
            return Err(ClientKeyError::InvalidClientKeyPrefix);
        };
        let Some(prefix_segment) = prefix_segment.strip_prefix(CLIENT_KEY_SCHEME_SEPARATOR) else {
            return Err(ClientKeyError::InvalidClientKeyPrefix);
        };
        if !is_lower_hex(prefix_segment, PUBLIC_PREFIX_HEX_LENGTH) {
            return Err(ClientKeyError::InvalidClientKeyPrefix);
        }

        Ok(Self(value))
    }

    /// Parses the public Prefix from a complete canonical presented Client Key.
    ///
    /// # Errors
    ///
    /// Returns [`ClientKeyError::InvalidClientKeyPrefix`] when the complete Key does not have the
    /// exact canonical `rgw_<prefix>_<secret>` representation. The returned Prefix is public
    /// routing metadata only; this method never exposes the secret segment.
    pub fn try_from_presented_key(presented_key: &str) -> Result<Self, ClientKeyError> {
        parse_presented_key_prefix(presented_key)
    }

    /// Returns the non-secret Prefix suitable for indexed lookup and safe audit correlation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_segment(prefix_segment: &str) -> Result<Self, ClientKeyError> {
        Self::try_new(format!(
            "{CLIENT_KEY_SCHEME}{CLIENT_KEY_SCHEME_SEPARATOR}{prefix_segment}"
        ))
    }
}

impl fmt::Debug for ClientKeyPrefix {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ClientKeyPrefix")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for ClientKeyPrefix {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A fixed-size opaque HMAC-SHA256 digest stored in `client_keys.secret_digest`.
#[derive(Eq, PartialEq)]
pub struct ClientKeyDigest {
    bytes: [u8; CLIENT_KEY_DIGEST_BYTES],
}

impl ClientKeyDigest {
    /// Reconstructs and validates a fixed-size digest from persistent storage bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ClientKeyError::InvalidDigestLength`] unless `bytes` is exactly
    /// [`CLIENT_KEY_DIGEST_BYTES`] long.
    pub fn try_from_persisted(bytes: impl AsRef<[u8]>) -> Result<Self, ClientKeyError> {
        let bytes = bytes.as_ref();
        if bytes.len() != CLIENT_KEY_DIGEST_BYTES {
            return Err(ClientKeyError::InvalidDigestLength {
                actual: bytes.len(),
            });
        }

        let mut digest = [0_u8; CLIENT_KEY_DIGEST_BYTES];
        digest.copy_from_slice(bytes);
        Ok(Self { bytes: digest })
    }

    /// Returns the opaque digest bytes for persistence or constant-time comparison.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; CLIENT_KEY_DIGEST_BYTES] {
        &self.bytes
    }

    fn from_hmac(bytes: [u8; CLIENT_KEY_DIGEST_BYTES]) -> Self {
        Self { bytes }
    }
}

impl Clone for ClientKeyDigest {
    fn clone(&self) -> Self {
        Self { bytes: self.bytes }
    }
}

impl fmt::Debug for ClientKeyDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClientKeyDigest(<redacted>)")
    }
}

impl Drop for ClientKeyDigest {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// Persisted Client Key lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientKeyStatus {
    /// The record may authenticate before its optional expiry.
    Active,
    /// The record remains present but must not authenticate.
    Disabled,
    /// The record was explicitly revoked and must not authenticate again.
    Revoked,
}

/// Storage-neutral fields for one persisted Client Key record.
#[derive(Clone, Eq, PartialEq)]
pub struct ClientKeyRecord {
    client_key_id: ClientKeyId,
    access_group_id: AccessGroupId,
    prefix: ClientKeyPrefix,
    secret_digest: ClientKeyDigest,
    status: ClientKeyStatus,
    expires_at_ms: Option<i64>,
}

impl ClientKeyRecord {
    /// Creates a persistable Client Key record from validated non-secret metadata and digest.
    ///
    /// # Errors
    ///
    /// Returns [`ClientKeyError::InvalidExpiryTimestamp`] when an expiry is negative.
    pub fn try_new(
        client_key_id: ClientKeyId,
        access_group_id: AccessGroupId,
        prefix: ClientKeyPrefix,
        secret_digest: ClientKeyDigest,
        status: ClientKeyStatus,
        expires_at_ms: Option<i64>,
    ) -> Result<Self, ClientKeyError> {
        validate_expiry(expires_at_ms)?;
        Ok(Self {
            client_key_id,
            access_group_id,
            prefix,
            secret_digest,
            status,
            expires_at_ms,
        })
    }

    /// Returns the stable non-secret Client Key identifier.
    #[must_use]
    pub fn client_key_id(&self) -> &ClientKeyId {
        &self.client_key_id
    }

    /// Returns the stable non-secret Access Group identifier.
    #[must_use]
    pub fn access_group_id(&self) -> &AccessGroupId {
        &self.access_group_id
    }

    /// Returns the indexed public Prefix.
    #[must_use]
    pub fn prefix(&self) -> &ClientKeyPrefix {
        &self.prefix
    }

    /// Returns the opaque HMAC digest for persistence and verification.
    #[must_use]
    pub fn secret_digest(&self) -> &ClientKeyDigest {
        &self.secret_digest
    }

    /// Returns the current lifecycle state.
    #[must_use]
    pub const fn status(&self) -> ClientKeyStatus {
        self.status
    }

    /// Returns the optional expiry timestamp in Unix milliseconds.
    #[must_use]
    pub const fn expires_at_ms(&self) -> Option<i64> {
        self.expires_at_ms
    }

    /// Changes the lifecycle state for a later persistence transaction.
    pub fn set_status(&mut self, status: ClientKeyStatus) {
        self.status = status;
    }

    fn permits_at(&self, now_ms: i64) -> bool {
        self.status == ClientKeyStatus::Active
            && match self.expires_at_ms {
                Some(expires_at_ms) => now_ms < expires_at_ms,
                None => true,
            }
    }
}

impl fmt::Debug for ClientKeyRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClientKeyRecord")
            .field("client_key_id", &self.client_key_id)
            .field("access_group_id", &self.access_group_id)
            .field("prefix", &self.prefix)
            .field("secret_digest", &"<redacted>")
            .field("status", &self.status)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

/// The complete Key material returned only when a Client Key is initially issued.
pub struct PresentedClientKey {
    value: String,
}

impl PresentedClientKey {
    fn new(value: String) -> Self {
        Self { value }
    }

    /// Returns the complete Key only to the immediate creation-result consumer.
    ///
    /// The caller must avoid logging, copying, or retaining this value unnecessarily.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for PresentedClientKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PresentedClientKey(<redacted>)")
    }
}

impl Drop for PresentedClientKey {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

/// An issuance result that separates one complete Key presentation from its persistable record.
pub struct IssuedClientKey {
    record: ClientKeyRecord,
    presented_key: PresentedClientKey,
}

impl IssuedClientKey {
    /// Returns the persistable record without revealing the complete Key.
    #[must_use]
    pub fn record(&self) -> &ClientKeyRecord {
        &self.record
    }

    /// Consumes the issuance result into a persistable record and its one-time presentation value.
    #[must_use]
    pub fn into_parts(self) -> (ClientKeyRecord, PresentedClientKey) {
        (self.record, self.presented_key)
    }
}

impl fmt::Debug for IssuedClientKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedClientKey")
            .field("record", &self.record)
            .field("presented_key", &"<redacted>")
            .finish()
    }
}

/// Issues canonical Client Keys and verifies Prefix-selected stored records.
pub struct ClientKeyService {
    pepper: ClientKeyPepper,
}

impl ClientKeyService {
    /// Creates a Client Key service from dedicated external Pepper material.
    #[must_use]
    pub const fn new(pepper: ClientKeyPepper) -> Self {
        Self { pepper }
    }

    /// Issues one canonical high-entropy Client Key and a matching persistable record.
    ///
    /// # Errors
    ///
    /// Returns a safe error when an expiry is invalid, OS randomness is unavailable, or HMAC
    /// initialization fails. No complete Key or record is returned on failure.
    pub fn issue(
        &self,
        client_key_id: ClientKeyId,
        access_group_id: AccessGroupId,
        expires_at_ms: Option<i64>,
    ) -> Result<IssuedClientKey, ClientKeyError> {
        validate_expiry(expires_at_ms)?;

        let mut prefix_bytes = [0_u8; PUBLIC_PREFIX_RANDOM_BYTES];
        getrandom::fill(&mut prefix_bytes).map_err(|_| ClientKeyError::RandomnessUnavailable)?;
        let mut secret_bytes = [0_u8; SECRET_RANDOM_BYTES];
        let secret_randomness =
            getrandom::fill(&mut secret_bytes).map_err(|_| ClientKeyError::RandomnessUnavailable);
        if let Err(error) = secret_randomness {
            prefix_bytes.zeroize();
            return Err(error);
        }

        let result = self.issue_from_random_bytes(
            client_key_id,
            access_group_id,
            expires_at_ms,
            prefix_bytes,
            &secret_bytes,
        );
        prefix_bytes.zeroize();
        secret_bytes.zeroize();
        result
    }

    /// Verifies one complete Key against a Prefix-selected record without revealing why it fails.
    ///
    /// # Errors
    ///
    /// Returns a safe local error only for an invalid caller clock or HMAC initialization. A
    /// malformed, unknown, disabled, revoked, expired, or wrong Key returns `Ok(false)`.
    pub fn verify(
        &self,
        presented_key: &str,
        record: &ClientKeyRecord,
        now_ms: i64,
    ) -> Result<bool, ClientKeyError> {
        validate_now(now_ms)?;

        let Ok(parsed_prefix) = ClientKeyPrefix::try_from_presented_key(presented_key) else {
            return Ok(false);
        };
        if parsed_prefix != *record.prefix() {
            return Ok(false);
        }

        let presented_digest = self.digest(presented_key)?;
        let digest_matches = bool::from(
            presented_digest
                .as_bytes()
                .ct_eq(record.secret_digest().as_bytes()),
        );
        let lifecycle_permits = record.permits_at(now_ms);
        Ok(digest_matches & lifecycle_permits)
    }

    fn issue_from_random_bytes(
        &self,
        client_key_id: ClientKeyId,
        access_group_id: AccessGroupId,
        expires_at_ms: Option<i64>,
        mut prefix_bytes: [u8; PUBLIC_PREFIX_RANDOM_BYTES],
        secret_bytes: &[u8; SECRET_RANDOM_BYTES],
    ) -> Result<IssuedClientKey, ClientKeyError> {
        let mut prefix_segment = encode_lower_hex(&prefix_bytes);
        let mut secret_segment = encode_lower_hex(secret_bytes);
        let result = (|| {
            let prefix = ClientKeyPrefix::from_segment(&prefix_segment)?;
            let presented_key = PresentedClientKey::new(format!(
                "{CLIENT_KEY_SCHEME}{CLIENT_KEY_SCHEME_SEPARATOR}{prefix_segment}{CLIENT_KEY_SCHEME_SEPARATOR}{secret_segment}"
            ));
            let digest = self.digest(presented_key.as_str())?;
            let record = ClientKeyRecord::try_new(
                client_key_id,
                access_group_id,
                prefix,
                digest,
                ClientKeyStatus::Active,
                expires_at_ms,
            )?;
            Ok(IssuedClientKey {
                record,
                presented_key,
            })
        })();
        prefix_segment.zeroize();
        secret_segment.zeroize();
        prefix_bytes.zeroize();
        result
    }

    fn digest(&self, presented_key: &str) -> Result<ClientKeyDigest, ClientKeyError> {
        let mut mac = HmacSha256::new_from_slice(self.pepper.as_bytes())
            .map_err(|_| ClientKeyError::HmacInitializationFailed)?;
        mac.update(presented_key.as_bytes());
        let output = mac.finalize().into_bytes();
        let mut digest = [0_u8; CLIENT_KEY_DIGEST_BYTES];
        digest.copy_from_slice(&output);
        let client_key_digest = ClientKeyDigest::from_hmac(digest);
        digest.zeroize();
        Ok(client_key_digest)
    }
}

impl fmt::Debug for ClientKeyService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClientKeyService")
            .field("pepper", &"<redacted>")
            .finish()
    }
}

/// Safe errors for Client Key configuration, issuance, and verification infrastructure.
#[derive(Debug)]
pub enum ClientKeyError {
    /// External Pepper material was not exactly [`CLIENT_KEY_PEPPER_BYTES`] long.
    InvalidPepperLength {
        /// Observed byte count, never Pepper contents.
        actual: usize,
    },
    /// The external Pepper path was not a direct regular non-symbolic file.
    InvalidPepperFile,
    /// External Pepper material could not be read without exposing its contents.
    PepperLoadIo,
    /// A stored Prefix did not have the canonical public shape.
    InvalidClientKeyPrefix,
    /// A stored HMAC digest was not exactly [`CLIENT_KEY_DIGEST_BYTES`] long.
    InvalidDigestLength {
        /// Observed byte count, never digest contents.
        actual: usize,
    },
    /// An optional expiry timestamp was negative.
    InvalidExpiryTimestamp,
    /// A caller supplied a negative verification timestamp.
    InvalidCurrentTimestamp,
    /// Operating-system randomness was unavailable for Client Key issuance.
    RandomnessUnavailable,
    /// HMAC could not initialize from already validated Pepper material.
    HmacInitializationFailed,
}

impl fmt::Display for ClientKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPepperLength { actual } => {
                write!(
                    formatter,
                    "Client Key Pepper has invalid length: {actual} bytes"
                )
            }
            Self::InvalidPepperFile => {
                formatter.write_str("Client Key Pepper location is not a direct regular file")
            }
            Self::PepperLoadIo => {
                formatter.write_str("Client Key Pepper material could not be loaded")
            }
            Self::InvalidClientKeyPrefix => {
                formatter.write_str("Client Key Prefix has an invalid canonical form")
            }
            Self::InvalidDigestLength { actual } => {
                write!(
                    formatter,
                    "Client Key digest has invalid length: {actual} bytes"
                )
            }
            Self::InvalidExpiryTimestamp => {
                formatter.write_str("Client Key expiry timestamp must not be negative")
            }
            Self::InvalidCurrentTimestamp => {
                formatter.write_str("Client Key verification timestamp must not be negative")
            }
            Self::RandomnessUnavailable => {
                formatter.write_str("operating-system randomness is unavailable")
            }
            Self::HmacInitializationFailed => {
                formatter.write_str("Client Key HMAC could not initialize")
            }
        }
    }
}

impl Error for ClientKeyError {}

#[cfg(unix)]
fn read_pepper_file(path: &Path) -> Result<Vec<u8>, ClientKeyError> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| ClientKeyError::PepperLoadIo)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| ClientKeyError::PepperLoadIo)?;
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_pepper_file(path: &Path) -> Result<Vec<u8>, ClientKeyError> {
    fs::read(path).map_err(|_| ClientKeyError::PepperLoadIo)
}

fn parse_presented_key_prefix(presented_key: &str) -> Result<ClientKeyPrefix, ClientKeyError> {
    let Some(remainder) = presented_key.strip_prefix(CLIENT_KEY_SCHEME) else {
        return Err(ClientKeyError::InvalidClientKeyPrefix);
    };
    let Some(remainder) = remainder.strip_prefix(CLIENT_KEY_SCHEME_SEPARATOR) else {
        return Err(ClientKeyError::InvalidClientKeyPrefix);
    };
    let Some((prefix_segment, secret_segment)) = remainder.split_once(CLIENT_KEY_SCHEME_SEPARATOR)
    else {
        return Err(ClientKeyError::InvalidClientKeyPrefix);
    };
    if secret_segment.contains(CLIENT_KEY_SCHEME_SEPARATOR)
        || !is_lower_hex(prefix_segment, PUBLIC_PREFIX_HEX_LENGTH)
        || !is_lower_hex(secret_segment, SECRET_HEX_LENGTH)
    {
        return Err(ClientKeyError::InvalidClientKeyPrefix);
    }

    ClientKeyPrefix::from_segment(prefix_segment)
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0F)]));
    }
    encoded
}

fn is_lower_hex(value: &str, expected_length: usize) -> bool {
    value.len() == expected_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_expiry(expires_at_ms: Option<i64>) -> Result<(), ClientKeyError> {
    if matches!(expires_at_ms, Some(value) if value < 0) {
        Err(ClientKeyError::InvalidExpiryTimestamp)
    } else {
        Ok(())
    }
}

fn validate_now(now_ms: i64) -> Result<(), ClientKeyError> {
    if now_ms < 0 {
        Err(ClientKeyError::InvalidCurrentTimestamp)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use zeroize::Zeroize;

    use super::{
        CLIENT_KEY_DIGEST_BYTES, CLIENT_KEY_PEPPER_BYTES, ClientKeyDigest, ClientKeyError,
        ClientKeyPepper, ClientKeyPrefix, ClientKeyRecord, ClientKeyService, ClientKeyStatus,
    };
    use gateway_core::{AccessGroupId, ClientKeyId};

    type TestResult = Result<(), Box<dyn Error>>;

    static TEST_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn issue_returns_a_redacted_one_time_key_and_persistable_hmac_record() -> TestResult {
        let service = service_with_pepper(0xA5)?;
        let issued = service.issue(ids()?.0, ids()?.1, None)?;

        let issued_debug = format!("{issued:?}");
        assert_eq!(
            issued.record().secret_digest().as_bytes().len(),
            CLIENT_KEY_DIGEST_BYTES
        );
        assert_eq!(issued.record().status(), ClientKeyStatus::Active);

        let (record, presented_key) = issued.into_parts();
        assert!(!issued_debug.contains(presented_key.as_str()));
        assert!(presented_key.as_str().starts_with("rgw_"));
        assert_eq!(presented_key.as_str().len(), 85);
        assert!(presented_key.as_str().starts_with(record.prefix().as_str()));
        assert!(service.verify(presented_key.as_str(), &record, 0)?);
        assert!(!format!("{record:?}").contains(presented_key.as_str()));
        assert!(!format!("{presented_key:?}").contains(presented_key.as_str()));
        Ok(())
    }

    #[test]
    fn valid_shape_tampering_wrong_pepper_and_wrong_prefix_fail() -> TestResult {
        let service = service_with_pepper(0xA5)?;
        let issued = service.issue(ids()?.0, ids()?.1, None)?;
        let (record, presented_key) = issued.into_parts();

        let mut tampered = presented_key.as_str().to_owned();
        let last_index = tampered.len() - 1;
        tampered.replace_range(
            last_index..,
            if tampered.ends_with('0') { "1" } else { "0" },
        );
        assert!(!service.verify(&tampered, &record, 0)?);
        tampered.zeroize();

        let wrong_pepper_service = service_with_pepper(0x5A)?;
        assert!(!wrong_pepper_service.verify(presented_key.as_str(), &record, 0)?);

        let other_issued = service.issue(ids()?.0, ids()?.1, None)?;
        let (_, other_presented_key) = other_issued.into_parts();
        assert!(!service.verify(other_presented_key.as_str(), &record, 0)?);
        assert!(!service.verify("not-a-client-key", &record, 0)?);
        Ok(())
    }

    #[test]
    fn disabled_revoked_and_expired_records_share_rejection() -> TestResult {
        let service = service_with_pepper(0xA5)?;
        let issued = service.issue(ids()?.0, ids()?.1, Some(100))?;
        let (record, presented_key) = issued.into_parts();

        assert!(service.verify(presented_key.as_str(), &record, 99)?);
        assert!(!service.verify(presented_key.as_str(), &record, 100)?);

        let mut disabled = record.clone();
        disabled.set_status(ClientKeyStatus::Disabled);
        assert!(!service.verify(presented_key.as_str(), &disabled, 99)?);

        let mut revoked = record;
        revoked.set_status(ClientKeyStatus::Revoked);
        assert!(!service.verify(presented_key.as_str(), &revoked, 99)?);
        Ok(())
    }

    #[test]
    fn persisted_fields_reject_invalid_sizes_shapes_and_times() -> TestResult {
        assert!(matches!(
            ClientKeyPrefix::try_new("rgw_NOT_LOWERCASE"),
            Err(ClientKeyError::InvalidClientKeyPrefix)
        ));
        assert!(matches!(
            ClientKeyDigest::try_from_persisted([0x11; CLIENT_KEY_DIGEST_BYTES - 1]),
            Err(ClientKeyError::InvalidDigestLength { actual: 31 })
        ));

        let prefix = ClientKeyPrefix::try_new("rgw_0123456789abcdef")?;
        let digest = ClientKeyDigest::try_from_persisted([0x11; CLIENT_KEY_DIGEST_BYTES])?;
        assert!(matches!(
            ClientKeyRecord::try_new(
                ids()?.0,
                ids()?.1,
                prefix,
                digest,
                ClientKeyStatus::Active,
                Some(-1),
            ),
            Err(ClientKeyError::InvalidExpiryTimestamp)
        ));

        let service = service_with_pepper(0xA5)?;
        let issued = service.issue(ids()?.0, ids()?.1, None)?;
        let (record, presented_key) = issued.into_parts();
        assert!(matches!(
            service.verify(presented_key.as_str(), &record, -1),
            Err(ClientKeyError::InvalidCurrentTimestamp)
        ));
        Ok(())
    }

    #[test]
    fn external_pepper_file_is_strict_and_redacted() -> TestResult {
        let file = TestPepperFile::new()?;
        file.write_bytes(&[0xA5; CLIENT_KEY_PEPPER_BYTES])?;
        let pepper = ClientKeyPepper::load_from_file(file.path())?;
        assert!(!format!("{pepper:?}").contains("a5"));

        file.write_bytes(&[0xA5; CLIENT_KEY_PEPPER_BYTES - 1])?;
        assert!(matches!(
            ClientKeyPepper::load_from_file(file.path()),
            Err(ClientKeyError::InvalidPepperLength { actual: 31 })
        ));

        fs::remove_file(file.path())?;
        fs::create_dir(file.path())?;
        assert!(matches!(
            ClientKeyPepper::load_from_file(file.path()),
            Err(ClientKeyError::InvalidPepperFile)
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn external_pepper_file_rejects_symbolic_links() -> TestResult {
        use std::os::unix::fs::symlink;

        let file = TestPepperFile::new()?;
        fs::remove_file(file.path())?;
        symlink("not-a-pepper", file.path())?;
        assert!(matches!(
            ClientKeyPepper::load_from_file(file.path()),
            Err(ClientKeyError::InvalidPepperFile)
        ));
        Ok(())
    }

    fn service_with_pepper(byte: u8) -> Result<ClientKeyService, ClientKeyError> {
        Ok(ClientKeyService::new(ClientKeyPepper::try_from_bytes(
            [byte; CLIENT_KEY_PEPPER_BYTES],
        )?))
    }

    fn ids() -> Result<(ClientKeyId, AccessGroupId), Box<dyn Error>> {
        Ok((
            ClientKeyId::try_new("client-key-id")?,
            AccessGroupId::try_new("access-group-id")?,
        ))
    }

    struct TestPepperFile {
        path: PathBuf,
    }

    impl TestPepperFile {
        fn new() -> Result<Self, std::io::Error> {
            let sequence = TEST_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "gateway-auth-client-key-pepper-test-{}-{sequence}",
                std::process::id()
            ));
            fs::write(&path, [])?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_bytes(&self, bytes: &[u8]) -> Result<(), std::io::Error> {
            fs::write(&self.path, bytes)
        }
    }

    impl Drop for TestPepperFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_dir(&self.path);
        }
    }
}
