//! AEAD encryption for persisted upstream Secrets and externally supplied Master Keys.
//!
//! This module does not know about SQLite rows or HTTP requests. It produces and consumes the
//! opaque `(key_version, ciphertext)` pair that a later control-plane Repository will persist.

#![deny(unsafe_code)]

use std::{collections::BTreeMap, error::Error, fmt, fs, num::NonZeroU32, path::Path};

#[cfg(unix)]
use std::{io::Read, os::unix::fs::OpenOptionsExt};

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use zeroize::Zeroize;

/// Exact number of raw bytes in one XChaCha20-Poly1305 Master Key.
pub const MASTER_KEY_BYTES: usize = 32;

/// Exact number of bytes in each independently generated XChaCha20-Poly1305 nonce.
pub const NONCE_BYTES: usize = 24;

const ENVELOPE_FORMAT_VERSION: u8 = 1;
const AEAD_TAG_BYTES: usize = 16;
const MINIMUM_ENVELOPE_BYTES: usize = 1 + NONCE_BYTES + AEAD_TAG_BYTES;
const KEY_FILE_EXTENSION: &str = ".key";

/// A positive, durable Master Key version stored alongside an encrypted Secret.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KeyVersion(NonZeroU32);

impl KeyVersion {
    /// Validates and creates a Key Version.
    ///
    /// # Errors
    ///
    /// Returns [`SecretStoreError::InvalidKeyVersion`] when `value` is zero.
    pub fn try_new(value: u32) -> Result<Self, SecretStoreError> {
        NonZeroU32::new(value)
            .map(Self)
            .ok_or(SecretStoreError::InvalidKeyVersion)
    }

    /// Returns the positive numerical version for persistence and diagnostics.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Returns the version in the signed `SQLite` integer domain.
    #[must_use]
    pub fn as_sqlite_i64(self) -> i64 {
        i64::from(self.0.get())
    }

    /// Validates a positive `SQLite` Key Version before creating the in-memory form.
    ///
    /// # Errors
    ///
    /// Returns [`SecretStoreError::InvalidKeyVersion`] for zero, negative, or out-of-range values.
    pub fn try_from_sqlite_i64(value: i64) -> Result<Self, SecretStoreError> {
        let value = u32::try_from(value).map_err(|_| SecretStoreError::InvalidKeyVersion)?;
        Self::try_new(value)
    }
}

/// An in-memory Master Key whose raw bytes are redacted and zeroized on drop.
pub struct MasterKey {
    bytes: [u8; MASTER_KEY_BYTES],
}

impl MasterKey {
    /// Copies and validates exact-size raw Master Key material.
    ///
    /// # Errors
    ///
    /// Returns [`SecretStoreError::InvalidMasterKeyLength`] unless `bytes` is exactly
    /// [`MASTER_KEY_BYTES`] long.
    pub fn try_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, SecretStoreError> {
        let bytes = bytes.as_ref();
        if bytes.len() != MASTER_KEY_BYTES {
            return Err(SecretStoreError::InvalidMasterKeyLength {
                actual: bytes.len(),
            });
        }

        let mut key = [0_u8; MASTER_KEY_BYTES];
        key.copy_from_slice(bytes);
        Ok(Self { bytes: key })
    }

    fn as_bytes(&self) -> &[u8; MASTER_KEY_BYTES] {
        &self.bytes
    }
}

impl Clone for MasterKey {
    fn clone(&self) -> Self {
        Self { bytes: self.bytes }
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MasterKey(<redacted>)")
    }
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// A versioned external Master Key set with one active version for new encryptions.
pub struct MasterKeyRing {
    active_key_version: KeyVersion,
    keys: BTreeMap<KeyVersion, MasterKey>,
}

impl MasterKeyRing {
    /// Creates a Key Ring from explicitly supplied external Master Key material.
    ///
    /// # Errors
    ///
    /// Returns an error when no keys are supplied, a version is duplicated, or the active version
    /// is absent from the ring.
    pub fn try_new(
        active_key_version: KeyVersion,
        entries: impl IntoIterator<Item = (KeyVersion, MasterKey)>,
    ) -> Result<Self, SecretStoreError> {
        let mut keys = BTreeMap::new();
        for (key_version, key) in entries {
            if keys.insert(key_version, key).is_some() {
                return Err(SecretStoreError::DuplicateKeyVersion { key_version });
            }
        }

        if keys.is_empty() {
            return Err(SecretStoreError::EmptyKeyRing);
        }
        if !keys.contains_key(&active_key_version) {
            return Err(SecretStoreError::ActiveKeyMissing {
                key_version: active_key_version,
            });
        }

        Ok(Self {
            active_key_version,
            keys,
        })
    }

    /// Loads a strict external Master Key directory.
    ///
    /// The directory must contain only direct regular files named
    /// `<positive-decimal-key-version>.key`, each holding exactly 32 raw bytes. Symbolic links,
    /// non-regular files, malformed/non-canonical names, unexpected entries, and a missing active
    /// version are rejected.
    ///
    /// # Errors
    ///
    /// Returns a safe key-loading error without including key bytes or file contents.
    pub fn load_from_directory(
        directory: impl AsRef<Path>,
        active_key_version: KeyVersion,
    ) -> Result<Self, SecretStoreError> {
        let directory = directory.as_ref();
        let metadata = fs::symlink_metadata(directory).map_err(|_| SecretStoreError::KeyLoadIo)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(SecretStoreError::InvalidKeyDirectory);
        }

        let directory_entries = fs::read_dir(directory).map_err(|_| SecretStoreError::KeyLoadIo)?;
        let mut keys = Vec::new();
        for directory_entry in directory_entries {
            let directory_entry = directory_entry.map_err(|_| SecretStoreError::KeyLoadIo)?;
            let file_type = directory_entry
                .file_type()
                .map_err(|_| SecretStoreError::KeyLoadIo)?;
            if file_type.is_symlink() {
                return Err(SecretStoreError::SymbolicLinkKeyFile);
            }
            if !file_type.is_file() {
                return Err(SecretStoreError::NonRegularKeyFile);
            }

            let key_version = parse_key_file_name(&directory_entry.file_name())?;
            let mut key_bytes = read_master_key_file(&directory_entry.path())?;
            let key = MasterKey::try_from_bytes(&key_bytes);
            key_bytes.zeroize();
            keys.push((key_version, key?));
        }

        Self::try_new(active_key_version, keys)
    }

    /// Returns the Key Version used for new Secret encryption.
    #[must_use]
    pub const fn active_key_version(&self) -> KeyVersion {
        self.active_key_version
    }

    fn active_key(&self) -> Result<&MasterKey, SecretStoreError> {
        self.key_for_version(self.active_key_version)
    }

    fn key_for_version(&self, key_version: KeyVersion) -> Result<&MasterKey, SecretStoreError> {
        self.keys
            .get(&key_version)
            .ok_or(SecretStoreError::UnknownKeyVersion { key_version })
    }
}

impl fmt::Debug for MasterKeyRing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MasterKeyRing")
            .field("active_key_version", &self.active_key_version)
            .field("loaded_key_versions", &self.keys.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// A redacted, zeroizing plaintext Secret returned only after successful AEAD authentication.
pub struct PlaintextSecret {
    bytes: Vec<u8>,
}

impl PlaintextSecret {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Returns plaintext bytes for the immediate, authorized caller.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for PlaintextSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlaintextSecret")
            .field("bytes", &"<redacted>")
            .field("length", &self.bytes.len())
            .finish()
    }
}

impl Drop for PlaintextSecret {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// An opaque encrypted envelope suitable for the existing `SQLite` ciphertext BLOB.
pub struct EncryptedSecret {
    key_version: KeyVersion,
    ciphertext: Vec<u8>,
}

impl EncryptedSecret {
    fn new(key_version: KeyVersion, ciphertext: Vec<u8>) -> Result<Self, SecretStoreError> {
        validate_envelope(&ciphertext)?;
        Ok(Self {
            key_version,
            ciphertext,
        })
    }

    /// Reconstructs an opaque encrypted Secret from the two persisted `SQLite` columns.
    ///
    /// # Errors
    ///
    /// Returns a safe envelope-format error for unknown versions or malformed/truncated bytes.
    pub fn try_from_persisted(
        key_version: KeyVersion,
        ciphertext: impl Into<Vec<u8>>,
    ) -> Result<Self, SecretStoreError> {
        Self::new(key_version, ciphertext.into())
    }

    /// Returns the Key Version stored with this encrypted Secret.
    #[must_use]
    pub const fn key_version(&self) -> KeyVersion {
        self.key_version
    }

    /// Returns the opaque envelope bytes for persistence; callers must not log them.
    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    /// Transfers the opaque envelope bytes to a persistence layer.
    #[must_use]
    pub fn into_ciphertext(mut self) -> Vec<u8> {
        std::mem::take(&mut self.ciphertext)
    }
}

impl fmt::Debug for EncryptedSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptedSecret")
            .field("key_version", &self.key_version)
            .field("ciphertext", &"<redacted>")
            .field("ciphertext_length", &self.ciphertext.len())
            .finish()
    }
}

/// AEAD Secret operations backed by an external versioned Master Key Ring.
pub struct SecretStore {
    key_ring: MasterKeyRing,
}

impl SecretStore {
    /// Creates a Secret Store from an externally loaded validated Key Ring.
    #[must_use]
    pub const fn new(key_ring: MasterKeyRing) -> Self {
        Self { key_ring }
    }

    /// Returns the Key Version used by [`Self::seal`].
    #[must_use]
    pub const fn active_key_version(&self) -> KeyVersion {
        self.key_ring.active_key_version()
    }

    /// Encrypts a Secret with a fresh operating-system-random nonce and caller-provided AAD.
    ///
    /// # Errors
    ///
    /// Returns a safe error when AAD is empty, randomness is unavailable, or encryption cannot
    /// produce an authenticated envelope. No partially built envelope is returned on failure.
    pub fn seal(
        &self,
        plaintext: &[u8],
        associated_data: &[u8],
    ) -> Result<EncryptedSecret, SecretStoreError> {
        validate_associated_data(associated_data)?;

        let mut nonce = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| SecretStoreError::RandomnessUnavailable)?;
        let ciphertext = encrypt(
            self.key_ring.active_key()?,
            &nonce,
            plaintext,
            associated_data,
        )?;

        let mut envelope = Vec::with_capacity(1 + NONCE_BYTES + ciphertext.len());
        envelope.push(ENVELOPE_FORMAT_VERSION);
        envelope.extend_from_slice(&nonce);
        envelope.extend_from_slice(&ciphertext);

        EncryptedSecret::new(self.active_key_version(), envelope)
    }

    /// Authenticates and decrypts an opaque Secret envelope with exactly the same AAD.
    ///
    /// # Errors
    ///
    /// Returns a safe error for unknown Key Versions, malformed envelopes, wrong AAD, wrong key
    /// material, or modified ciphertext. It never returns partial plaintext.
    pub fn open(
        &self,
        encrypted_secret: &EncryptedSecret,
        associated_data: &[u8],
    ) -> Result<PlaintextSecret, SecretStoreError> {
        validate_associated_data(associated_data)?;
        validate_envelope(encrypted_secret.ciphertext())?;

        let key = self
            .key_ring
            .key_for_version(encrypted_secret.key_version())?;
        let nonce_end = 1 + NONCE_BYTES;
        let nonce = &encrypted_secret.ciphertext()[1..nonce_end];
        let ciphertext = &encrypted_secret.ciphertext()[nonce_end..];
        let plaintext = decrypt(key, nonce, ciphertext, associated_data)?;
        Ok(PlaintextSecret::new(plaintext))
    }

    /// Re-encrypts a Secret under the active Key Version using a fresh nonce.
    ///
    /// This operation has no persistence side effect; a later Repository persists the returned
    /// Key Version and ciphertext atomically.
    ///
    /// # Errors
    ///
    /// Returns the same safe errors as [`Self::open`] or [`Self::seal`].
    pub fn rotate(
        &self,
        encrypted_secret: &EncryptedSecret,
        associated_data: &[u8],
    ) -> Result<EncryptedSecret, SecretStoreError> {
        let plaintext = self.open(encrypted_secret, associated_data)?;
        self.seal(plaintext.as_bytes(), associated_data)
    }
}

impl fmt::Debug for SecretStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretStore")
            .field("key_ring", &self.key_ring)
            .finish()
    }
}

/// Safe failure classes for Master Key and encrypted Secret operations.
#[derive(Debug)]
pub enum SecretStoreError {
    /// A Key Version was zero, negative, or outside the supported `SQLite` integer range.
    InvalidKeyVersion,
    /// Master Key material was not exactly [`MASTER_KEY_BYTES`] long.
    InvalidMasterKeyLength {
        /// Observed byte count, never the key contents.
        actual: usize,
    },
    /// A Key Ring had no entries.
    EmptyKeyRing,
    /// More than one entry supplied the same version.
    DuplicateKeyVersion {
        /// Duplicate non-secret version number.
        key_version: KeyVersion,
    },
    /// The configured active version was absent from an otherwise valid Key Ring.
    ActiveKeyMissing {
        /// Missing non-secret version number.
        key_version: KeyVersion,
    },
    /// The supplied external Master Key path was not a direct non-symbolic directory.
    InvalidKeyDirectory,
    /// An external key directory could not be read without exposing file contents.
    KeyLoadIo,
    /// A directory entry did not have the canonical `<positive-version>.key` form.
    InvalidKeyFileName,
    /// A key directory entry was a symbolic link.
    SymbolicLinkKeyFile,
    /// A key directory entry was not a direct regular file.
    NonRegularKeyFile,
    /// A stored Key Version is not loaded in the active process.
    UnknownKeyVersion {
        /// Missing non-secret version number.
        key_version: KeyVersion,
    },
    /// An envelope used an unknown internal serialization format version.
    UnsupportedEnvelopeVersion,
    /// An envelope was too short to contain a format byte, nonce, and AEAD tag.
    TruncatedEnvelope,
    /// The caller did not supply stable associated data for a logical encrypted record.
    EmptyAssociatedData,
    /// Operating-system randomness could not provide a nonce.
    RandomnessUnavailable,
    /// The AEAD implementation could not encrypt without a complete authenticated result.
    EncryptionFailed,
    /// The AEAD authentication check failed without yielding plaintext.
    AuthenticationFailed,
    /// The AEAD implementation could not initialize from already validated key material.
    CipherInitializationFailed,
}

impl fmt::Display for SecretStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKeyVersion => formatter.write_str("Master Key Version is invalid"),
            Self::InvalidMasterKeyLength { actual } => {
                write!(formatter, "Master Key has invalid length: {actual} bytes")
            }
            Self::EmptyKeyRing => formatter.write_str("Master Key Ring has no keys"),
            Self::DuplicateKeyVersion { key_version } => {
                write!(
                    formatter,
                    "Master Key Version {} is duplicated",
                    key_version.get()
                )
            }
            Self::ActiveKeyMissing { key_version } => {
                write!(
                    formatter,
                    "active Master Key Version {} is not loaded",
                    key_version.get()
                )
            }
            Self::InvalidKeyDirectory => {
                formatter.write_str("external Master Key location is not a direct directory")
            }
            Self::KeyLoadIo => {
                formatter.write_str("external Master Key material could not be loaded")
            }
            Self::InvalidKeyFileName => {
                formatter.write_str("external Master Key file name is invalid")
            }
            Self::SymbolicLinkKeyFile => {
                formatter.write_str("external Master Key files must not be symbolic links")
            }
            Self::NonRegularKeyFile => {
                formatter.write_str("external Master Key files must be direct regular files")
            }
            Self::UnknownKeyVersion { key_version } => {
                write!(
                    formatter,
                    "Master Key Version {} is not loaded",
                    key_version.get()
                )
            }
            Self::UnsupportedEnvelopeVersion => {
                formatter.write_str("encrypted Secret envelope format is unsupported")
            }
            Self::TruncatedEnvelope => {
                formatter.write_str("encrypted Secret envelope is truncated")
            }
            Self::EmptyAssociatedData => {
                formatter.write_str("encrypted Secret associated data must not be empty")
            }
            Self::RandomnessUnavailable => {
                formatter.write_str("operating-system randomness is unavailable")
            }
            Self::EncryptionFailed => formatter.write_str("Secret encryption failed"),
            Self::AuthenticationFailed => {
                formatter.write_str("encrypted Secret authentication failed")
            }
            Self::CipherInitializationFailed => {
                formatter.write_str("Secret cipher could not initialize")
            }
        }
    }
}

impl Error for SecretStoreError {}

fn parse_key_file_name(file_name: &std::ffi::OsStr) -> Result<KeyVersion, SecretStoreError> {
    let file_name = file_name
        .to_str()
        .ok_or(SecretStoreError::InvalidKeyFileName)?;
    let version_text = file_name
        .strip_suffix(KEY_FILE_EXTENSION)
        .ok_or(SecretStoreError::InvalidKeyFileName)?;
    if version_text.is_empty()
        || !version_text.bytes().all(|byte| byte.is_ascii_digit())
        || (version_text.len() > 1 && version_text.starts_with('0'))
    {
        return Err(SecretStoreError::InvalidKeyFileName);
    }

    let value = version_text
        .parse::<u32>()
        .map_err(|_| SecretStoreError::InvalidKeyFileName)?;
    let key_version =
        KeyVersion::try_new(value).map_err(|_| SecretStoreError::InvalidKeyFileName)?;
    if key_version.get().to_string() != version_text {
        return Err(SecretStoreError::InvalidKeyFileName);
    }

    Ok(key_version)
}

#[cfg(unix)]
fn read_master_key_file(path: &Path) -> Result<Vec<u8>, SecretStoreError> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| SecretStoreError::KeyLoadIo)?;
    let mut key_bytes = Vec::new();
    file.read_to_end(&mut key_bytes)
        .map_err(|_| SecretStoreError::KeyLoadIo)?;
    Ok(key_bytes)
}

#[cfg(not(unix))]
fn read_master_key_file(path: &Path) -> Result<Vec<u8>, SecretStoreError> {
    fs::read(path).map_err(|_| SecretStoreError::KeyLoadIo)
}

fn validate_associated_data(associated_data: &[u8]) -> Result<(), SecretStoreError> {
    if associated_data.is_empty() {
        Err(SecretStoreError::EmptyAssociatedData)
    } else {
        Ok(())
    }
}

fn validate_envelope(ciphertext: &[u8]) -> Result<(), SecretStoreError> {
    let Some(format_version) = ciphertext.first() else {
        return Err(SecretStoreError::TruncatedEnvelope);
    };
    if *format_version != ENVELOPE_FORMAT_VERSION {
        return Err(SecretStoreError::UnsupportedEnvelopeVersion);
    }
    if ciphertext.len() < MINIMUM_ENVELOPE_BYTES {
        return Err(SecretStoreError::TruncatedEnvelope);
    }

    Ok(())
}

fn encrypt(
    key: &MasterKey,
    nonce: &[u8; NONCE_BYTES],
    plaintext: &[u8],
    associated_data: &[u8],
) -> Result<Vec<u8>, SecretStoreError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|_| SecretStoreError::CipherInitializationFailed)?;
    cipher
        .encrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: plaintext,
                aad: associated_data,
            },
        )
        .map_err(|_| SecretStoreError::EncryptionFailed)
}

fn decrypt(
    key: &MasterKey,
    nonce: &[u8],
    ciphertext: &[u8],
    associated_data: &[u8],
) -> Result<Vec<u8>, SecretStoreError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|_| SecretStoreError::CipherInitializationFailed)?;
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: associated_data,
            },
        )
        .map_err(|_| SecretStoreError::AuthenticationFailed)
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::{
        EncryptedSecret, KeyVersion, MASTER_KEY_BYTES, MasterKey, MasterKeyRing, SecretStore,
        SecretStoreError,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    static TEST_DIRECTORY_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn seal_open_uses_fresh_nonces_and_redacted_values() -> TestResult {
        let store = store_with_keys(7, &[(7, 0x17)])?;
        let associated_data = b"credential:v1:credential-a";

        let first = store.seal(b"synthetic-credential-value", associated_data)?;
        let second = store.seal(b"synthetic-credential-value", associated_data)?;

        assert_eq!(first.key_version().get(), 7);
        assert_eq!(second.key_version().get(), 7);
        assert_ne!(first.ciphertext(), second.ciphertext());
        assert_eq!(
            store.open(&first, associated_data)?.as_bytes(),
            b"synthetic-credential-value"
        );
        assert!(!format!("{first:?}").contains("synthetic-credential-value"));
        assert!(
            !format!("{:?}", store.open(&first, associated_data)?)
                .contains("synthetic-credential-value")
        );
        Ok(())
    }

    #[test]
    fn wrong_aad_key_or_ciphertext_fails_closed() -> TestResult {
        let store = store_with_keys(7, &[(7, 0x17)])?;
        let associated_data = b"credential:v1:credential-a";
        let encrypted = store.seal(b"synthetic-credential-value", associated_data)?;

        assert!(matches!(
            store.open(&encrypted, b"credential:v1:credential-b"),
            Err(SecretStoreError::AuthenticationFailed)
        ));

        let wrong_key_store = store_with_keys(7, &[(7, 0x71)])?;
        assert!(matches!(
            wrong_key_store.open(&encrypted, associated_data),
            Err(SecretStoreError::AuthenticationFailed)
        ));

        let key_version = encrypted.key_version();
        let mut tampered = encrypted.into_ciphertext();
        let last_index = tampered.len() - 1;
        tampered[last_index] ^= 0x01;
        let tampered = EncryptedSecret::try_from_persisted(key_version, tampered)?;
        assert!(matches!(
            store.open(&tampered, associated_data),
            Err(SecretStoreError::AuthenticationFailed)
        ));
        Ok(())
    }

    #[test]
    fn malformed_envelopes_and_empty_aad_are_rejected() -> TestResult {
        let key_version = key_version(7)?;
        assert!(matches!(
            EncryptedSecret::try_from_persisted(key_version, Vec::new()),
            Err(SecretStoreError::TruncatedEnvelope)
        ));
        assert!(matches!(
            EncryptedSecret::try_from_persisted(key_version, vec![2; 1]),
            Err(SecretStoreError::UnsupportedEnvelopeVersion)
        ));

        let store = store_with_keys(7, &[(7, 0x17)])?;
        assert!(matches!(
            store.seal(b"synthetic-credential-value", b""),
            Err(SecretStoreError::EmptyAssociatedData)
        ));
        Ok(())
    }

    #[test]
    fn external_key_directory_requires_canonical_exact_size_active_keys() -> TestResult {
        let directory = TestKeyDirectory::new()?;
        directory.write_key(1, 0x11)?;
        directory.write_key(2, 0x22)?;

        let ring = MasterKeyRing::load_from_directory(directory.path(), key_version(2)?)?;
        assert_eq!(ring.active_key_version().get(), 2);

        fs::write(directory.path().join("01.key"), [0x33; MASTER_KEY_BYTES])?;
        assert!(matches!(
            MasterKeyRing::load_from_directory(directory.path(), key_version(2)?),
            Err(SecretStoreError::InvalidKeyFileName)
        ));
        fs::remove_file(directory.path().join("01.key"))?;

        fs::write(directory.path().join("3.key"), [0x44; MASTER_KEY_BYTES - 1])?;
        assert!(matches!(
            MasterKeyRing::load_from_directory(directory.path(), key_version(2)?),
            Err(SecretStoreError::InvalidMasterKeyLength { actual: 31 })
        ));
        fs::remove_file(directory.path().join("3.key"))?;

        fs::create_dir(directory.path().join("3.key"))?;
        assert!(matches!(
            MasterKeyRing::load_from_directory(directory.path(), key_version(2)?),
            Err(SecretStoreError::NonRegularKeyFile)
        ));
        fs::remove_dir(directory.path().join("3.key"))?;

        assert!(matches!(
            MasterKeyRing::load_from_directory(directory.path(), key_version(3)?),
            Err(SecretStoreError::ActiveKeyMissing { .. })
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn external_key_directory_rejects_symbolic_link_files() -> TestResult {
        use std::os::unix::fs::symlink;

        let directory = TestKeyDirectory::new()?;
        directory.write_key(1, 0x11)?;
        symlink("not-a-key-file", directory.path().join("2.key"))?;

        assert!(matches!(
            MasterKeyRing::load_from_directory(directory.path(), key_version(1)?),
            Err(SecretStoreError::SymbolicLinkKeyFile)
        ));
        Ok(())
    }

    #[test]
    fn rotation_reencrypts_under_the_active_key_version() -> TestResult {
        let associated_data = b"credential:v1:credential-a";
        let legacy_store = store_with_keys(1, &[(1, 0x11)])?;
        let legacy_envelope = legacy_store.seal(b"synthetic-credential-value", associated_data)?;

        let rotating_store = store_with_keys(2, &[(1, 0x11), (2, 0x22)])?;
        let rotated = rotating_store.rotate(&legacy_envelope, associated_data)?;

        assert_eq!(rotated.key_version().get(), 2);
        assert_ne!(legacy_envelope.ciphertext(), rotated.ciphertext());
        assert_eq!(
            rotating_store.open(&rotated, associated_data)?.as_bytes(),
            b"synthetic-credential-value"
        );

        let legacy_only_store = store_with_keys(1, &[(1, 0x11)])?;
        assert!(matches!(
            legacy_only_store.open(&rotated, associated_data),
            Err(SecretStoreError::UnknownKeyVersion { .. })
        ));
        Ok(())
    }

    #[test]
    fn key_version_rejects_invalid_values() {
        assert!(matches!(
            KeyVersion::try_new(0),
            Err(SecretStoreError::InvalidKeyVersion)
        ));
        assert!(matches!(
            KeyVersion::try_from_sqlite_i64(-1),
            Err(SecretStoreError::InvalidKeyVersion)
        ));
        assert!(matches!(
            KeyVersion::try_from_sqlite_i64(i64::from(u32::MAX) + 1),
            Err(SecretStoreError::InvalidKeyVersion)
        ));
    }

    #[test]
    fn key_ring_rejects_duplicate_versions() -> TestResult {
        let version = key_version(7)?;
        assert!(matches!(
            MasterKeyRing::try_new(
                version,
                [
                    (version, MasterKey::try_from_bytes([0x17; MASTER_KEY_BYTES])?),
                    (version, MasterKey::try_from_bytes([0x71; MASTER_KEY_BYTES])?),
                ],
            ),
            Err(SecretStoreError::DuplicateKeyVersion { key_version: duplicate }) if duplicate == version
        ));
        Ok(())
    }

    fn store_with_keys(
        active_version: u32,
        entries: &[(u32, u8)],
    ) -> Result<SecretStore, SecretStoreError> {
        let entries = entries
            .iter()
            .map(|(version, byte)| {
                Ok((
                    key_version(*version)?,
                    MasterKey::try_from_bytes([*byte; MASTER_KEY_BYTES])?,
                ))
            })
            .collect::<Result<Vec<_>, SecretStoreError>>()?;
        Ok(SecretStore::new(MasterKeyRing::try_new(
            key_version(active_version)?,
            entries,
        )?))
    }

    fn key_version(value: u32) -> Result<KeyVersion, SecretStoreError> {
        KeyVersion::try_new(value)
    }

    struct TestKeyDirectory {
        path: PathBuf,
    }

    impl TestKeyDirectory {
        fn new() -> Result<Self, std::io::Error> {
            let sequence = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "gateway-store-secret-store-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_key(&self, version: u32, byte: u8) -> Result<(), std::io::Error> {
            fs::write(
                self.path.join(format!("{version}.key")),
                [byte; MASTER_KEY_BYTES],
            )
        }
    }

    impl Drop for TestKeyDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
