//! Passwords are never management keys; only salted Argon2id hashes are persisted.
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::SaltString,
};
use std::{error::Error, fmt};
use zeroize::Zeroizing;

/// Safe password/entropy error, with no rejected input or hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdminPasswordError;
impl fmt::Display for AdminPasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("administrator password is invalid or unavailable")
    }
}
impl Error for AdminPasswordError {}

/// Password policy: preserve Unicode and whitespace; bound both characters and UTF-8 bytes.
#[must_use]
pub fn valid_password(value: &str) -> bool {
    value.len() <= 512
        && (12..=128).contains(&value.chars().count())
        && !value.chars().all(char::is_whitespace)
}

/// Fresh salted Argon2id v19 hash, using the OWASP 19 MiB / t=2 / p=1 profile.
/// # Errors
/// Rejects a password outside the policy or unavailable entropy/hash allocation.
pub fn hash_password(value: &str) -> Result<String, AdminPasswordError> {
    if !valid_password(value) {
        return Err(AdminPasswordError);
    }
    let mut salt = [0_u8; 16];
    getrandom::fill(&mut salt).map_err(|_| AdminPasswordError)?;
    let salt = SaltString::encode_b64(&salt).map_err(|_| AdminPasswordError)?;
    let params = Params::new(19_456, 2, 1, Some(32)).map_err(|_| AdminPasswordError)?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password(value.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AdminPasswordError)
}

/// Reject unrecognized or unbounded persisted hash parameters before invoking the KDF.
#[must_use]
pub fn valid_hash(encoded: &str) -> bool {
    let Ok(hash) = PasswordHash::new(encoded) else {
        return false;
    };
    hash.algorithm.as_str() == "argon2id"
        && hash.version == Some(19)
        && hash.params.get_decimal("m") == Some(19_456)
        && hash.params.get_decimal("t") == Some(2)
        && hash.params.get_decimal("p") == Some(1)
        && hash.salt.is_some_and(|salt| salt.as_str().len() == 22)
        && hash.hash.is_some_and(|hash| hash.len() == 32)
}

/// Constant-time verification through `RustCrypto`; never trims or truncates the password.
#[must_use]
pub fn verify_password(value: &str, encoded: &str) -> bool {
    if value.len() > 512 || !valid_hash(encoded) {
        return false;
    }
    PasswordHash::new(encoded).is_ok_and(|hash| {
        Argon2::default()
            .verify_password(value.as_bytes(), &hash)
            .is_ok()
    })
}

/// Generates 256 bits of OS entropy encoded without quoting or URL delimiters.
/// # Errors
/// Returns a redacted error if OS entropy is unavailable.
pub fn random_secret(prefix: &str) -> Result<Zeroizing<String>, AdminPasswordError> {
    use std::fmt::Write;
    let mut bytes = Zeroizing::new([0_u8; 32]);
    getrandom::fill(&mut *bytes).map_err(|_| AdminPasswordError)?;
    let mut result = Zeroizing::new(String::from(prefix));
    for byte in bytes.iter() {
        write!(result, "{byte:02x}").map_err(|_| AdminPasswordError)?;
    }
    Ok(result)
}

/// Constant-time comparison of fixed-size session/CSRF digests.
#[must_use]
pub fn constant_time_match(left: &[u8; 32], right: &[u8; 32]) -> bool {
    use subtle::ConstantTimeEq;
    bool::from(left.ct_eq(right))
}
