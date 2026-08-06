//! Native Grok Console `DPoP` session and proof primitives.
//!
//! The v3.1.1 Console provider exchanges the SSO cookie for a short-lived `DPoP` access token and
//! signs every request with the same ephemeral P-256 key.  This module keeps that cryptographic
//! state separate from the HTTP transport so it can be tested without network access.

#![allow(clippy::missing_errors_doc)]

use std::{
    collections::HashMap,
    fmt,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const REFRESH_SKEW: Duration = Duration::from_secs(20);
const MAX_TOKEN_LIFETIME: Duration = Duration::from_hours(1);

/// A validated `DPoP` token returned by the Console token exchange.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokConsoleDpopToken(Zeroizing<String>);

impl GrokConsoleDpopToken {
    /// Validates and stores a non-empty bounded bearer value.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GrokConsoleDpopError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 64 * 1024
            || value.bytes().any(|b| b.is_ascii_control())
        {
            return Err(GrokConsoleDpopError::InvalidToken);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    /// Borrows the token only for immediate header construction.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for GrokConsoleDpopToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GrokConsoleDpopToken(<redacted>)")
    }
}

/// A validated short-lived `DPoP` session bound to one public key thumbprint.
#[derive(Clone)]
pub struct GrokConsoleDpopSession {
    token: GrokConsoleDpopToken,
    signing_key: SigningKey,
    public_jwk: DpopJwk,
    expires_at: SystemTime,
}

impl fmt::Debug for GrokConsoleDpopSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GrokConsoleDpopSession")
            .field("token", &"<redacted>")
            .field("thumbprint", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

impl GrokConsoleDpopSession {
    /// Builds a session from a token response and the key generated for its request.
    pub fn from_token_response(
        access_token: impl Into<String>,
        token_type: &str,
        expires_in: u64,
        signing_key: SigningKey,
        now: SystemTime,
    ) -> Result<Self, GrokConsoleDpopError> {
        if !token_type.eq_ignore_ascii_case("DPoP") || expires_in == 0 {
            return Err(GrokConsoleDpopError::InvalidTokenResponse);
        }
        let lifetime = Duration::from_secs(expires_in);
        if lifetime > MAX_TOKEN_LIFETIME {
            return Err(GrokConsoleDpopError::InvalidLifetime);
        }
        let token = GrokConsoleDpopToken::try_new(access_token)?;
        let public_jwk = DpopJwk::from_key(signing_key.verifying_key());
        let thumbprint = public_jwk.thumbprint()?;
        let (jwt_expiry, jwt_thumbprint) = parse_access_token_claims(token.as_str())?;
        if jwt_thumbprint != thumbprint {
            return Err(GrokConsoleDpopError::KeyMismatch);
        }
        let expires_at = now
            .checked_add(lifetime)
            .ok_or(GrokConsoleDpopError::InvalidLifetime)?;
        let expires_at = expires_at.min(jwt_expiry);
        if expires_at
            <= now
                .checked_add(REFRESH_SKEW)
                .ok_or(GrokConsoleDpopError::InvalidLifetime)?
        {
            return Err(GrokConsoleDpopError::ExpiredToken);
        }
        Ok(Self {
            token,
            signing_key,
            public_jwk,
            expires_at,
        })
    }

    /// Generates the ephemeral P-256 signing key used in the `DPoP` token request.
    #[must_use]
    pub fn generate_key() -> SigningKey {
        SigningKey::random(&mut rand_core::OsRng)
    }

    /// Returns the `DPoP` access token for immediate Authorization header construction.
    #[must_use]
    pub fn access_token(&self) -> &str {
        self.token.as_str()
    }

    /// Returns whether the session remains usable with the refresh skew applied.
    #[must_use]
    pub fn is_valid_at(&self, now: SystemTime) -> bool {
        self.expires_at > now.checked_add(REFRESH_SKEW).unwrap_or(now)
    }

    /// Signs one `DPoP` proof JWT for an HTTP request.
    pub fn proof(
        &self,
        method: &str,
        htu: &str,
        now: SystemTime,
    ) -> Result<String, GrokConsoleDpopError> {
        if method.trim().is_empty() || htu.trim().is_empty() || !self.is_valid_at(now) {
            return Err(GrokConsoleDpopError::ExpiredToken);
        }
        let mut jti_bytes = [0u8; 16];
        getrandom::fill(&mut jti_bytes).map_err(|_| GrokConsoleDpopError::Randomness)?;
        let header = DpopHeader {
            typ: "dpop+jwt",
            alg: "ES256",
            jwk: &self.public_jwk,
        };
        let access_digest = Sha256::digest(self.token.as_str().as_bytes());
        let claims = DpopClaims {
            jti: URL_SAFE_NO_PAD.encode(jti_bytes),
            htm: method.to_ascii_uppercase(),
            htu: htu.to_owned(),
            iat: now
                .duration_since(UNIX_EPOCH)
                .map_err(|_| GrokConsoleDpopError::Clock)?
                .as_secs(),
            ath: URL_SAFE_NO_PAD.encode(access_digest),
        };
        let encoded_header = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&header).map_err(|_| GrokConsoleDpopError::Encoding)?);
        let encoded_claims = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&claims).map_err(|_| GrokConsoleDpopError::Encoding)?);
        let signing_input = format!("{encoded_header}.{encoded_claims}");
        let signature: Signature = self.signing_key.sign(signing_input.as_bytes());
        Ok(format!(
            "{signing_input}.{}",
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        ))
    }

    /// Returns the token exchange body containing the public JWK.
    pub fn token_exchange_body(signing_key: &SigningKey) -> Result<Vec<u8>, GrokConsoleDpopError> {
        serde_json::to_vec(
            &serde_json::json!({"jwk": DpopJwk::from_key(signing_key.verifying_key())}),
        )
        .map_err(|_| GrokConsoleDpopError::Encoding)
    }
}

/// A bounded, mutex-protected per-account `DPoP` session cache.
#[derive(Default)]
pub struct GrokConsoleDpopSessionCache(Mutex<HashMap<String, GrokConsoleDpopSession>>);

impl fmt::Debug for GrokConsoleDpopSessionCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GrokConsoleDpopSessionCache(<redacted>)")
    }
}

impl GrokConsoleDpopSessionCache {
    /// Returns a valid cached session for the exact opaque binding key.
    pub fn get(&self, key: &str, now: SystemTime) -> Option<GrokConsoleDpopSession> {
        let mut cache = self.0.lock().ok()?;
        let session = cache.get(key)?;
        if session.is_valid_at(now) {
            Some(session.clone())
        } else {
            cache.remove(key);
            None
        }
    }

    /// Stores or replaces one exact binding session.
    pub fn insert(
        &self,
        key: String,
        session: GrokConsoleDpopSession,
    ) -> Result<(), GrokConsoleDpopError> {
        self.0
            .lock()
            .map_err(|_| GrokConsoleDpopError::CachePoisoned)?
            .insert(key, session);
        Ok(())
    }

    /// Invalidates only the matching token, preserving a concurrently refreshed successor.
    pub fn invalidate(&self, key: &str, token: &str) -> Result<(), GrokConsoleDpopError> {
        let mut cache = self
            .0
            .lock()
            .map_err(|_| GrokConsoleDpopError::CachePoisoned)?;
        if cache
            .get(key)
            .is_some_and(|current| current.access_token() == token)
        {
            cache.remove(key);
        }
        Ok(())
    }
}

/// Derives a stable cache key without retaining the cookie or token value.
#[must_use]
pub fn grok_console_dpop_cache_key(binding: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(binding.as_bytes()))
}

#[derive(Clone, Debug, Serialize)]
struct DpopJwk {
    kty: &'static str,
    crv: &'static str,
    x: String,
    y: String,
}

impl DpopJwk {
    fn from_key(key: &p256::ecdsa::VerifyingKey) -> Self {
        let point = key.to_encoded_point(false);
        Self {
            kty: "EC",
            crv: "P-256",
            x: URL_SAFE_NO_PAD.encode(point.x().map_or_else(Vec::new, |value| value.to_vec())),
            y: URL_SAFE_NO_PAD.encode(point.y().map_or_else(Vec::new, |value| value.to_vec())),
        }
    }

    fn thumbprint(&self) -> Result<String, GrokConsoleDpopError> {
        let canonical =
            serde_json::json!({"crv": self.crv, "kty": self.kty, "x": self.x, "y": self.y});
        let bytes = serde_json::to_vec(&canonical).map_err(|_| GrokConsoleDpopError::Encoding)?;
        Ok(URL_SAFE_NO_PAD.encode(Sha256::digest(bytes)))
    }
}

#[derive(Serialize)]
struct DpopHeader<'a> {
    typ: &'static str,
    alg: &'static str,
    jwk: &'a DpopJwk,
}

#[derive(Serialize)]
struct DpopClaims {
    jti: String,
    htm: String,
    htu: String,
    iat: u64,
    ath: String,
}

fn parse_access_token_claims(token: &str) -> Result<(SystemTime, String), GrokConsoleDpopError> {
    let mut parts = token.split('.');
    let _header = parts.next().ok_or(GrokConsoleDpopError::InvalidToken)?;
    let payload = parts.next().ok_or(GrokConsoleDpopError::InvalidToken)?;
    if parts.next().is_none() || parts.next().is_some() {
        return Err(GrokConsoleDpopError::InvalidToken);
    }
    let payload = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| GrokConsoleDpopError::InvalidToken)?;
    let claims: Value =
        serde_json::from_slice(&payload).map_err(|_| GrokConsoleDpopError::InvalidToken)?;
    let exp = claims
        .get("exp")
        .and_then(Value::as_u64)
        .ok_or(GrokConsoleDpopError::InvalidToken)?;
    let thumbprint = claims
        .pointer("/cnf/jkt")
        .and_then(Value::as_str)
        .ok_or(GrokConsoleDpopError::KeyMismatch)?
        .to_owned();
    Ok((
        UNIX_EPOCH
            .checked_add(Duration::from_secs(exp))
            .ok_or(GrokConsoleDpopError::InvalidLifetime)?,
        thumbprint,
    ))
}

/// Stable, value-free `DPoP` failure categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokConsoleDpopError {
    /// The token is empty, malformed, oversized, or contains control bytes.
    InvalidToken,
    /// The token response has an unsupported type or lifetime.
    InvalidTokenResponse,
    /// The token lifetime is outside the bounded policy.
    InvalidLifetime,
    /// The access token is bound to another public key.
    KeyMismatch,
    /// The token is expired or inside the refresh skew.
    ExpiredToken,
    /// Secure randomness was unavailable.
    Randomness,
    /// The wall clock could not be represented.
    Clock,
    /// A JSON or JWT value could not be encoded.
    Encoding,
    /// The session cache lock was poisoned.
    CachePoisoned,
}

impl fmt::Display for GrokConsoleDpopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidToken => "Console DPoP token is invalid",
            Self::InvalidTokenResponse => "Console DPoP token response is invalid",
            Self::InvalidLifetime => "Console DPoP token lifetime is invalid",
            Self::KeyMismatch => "Console DPoP token key does not match",
            Self::ExpiredToken => "Console DPoP token is expired",
            Self::Randomness => "Console DPoP randomness is unavailable",
            Self::Clock => "Console DPoP clock is invalid",
            Self::Encoding => "Console DPoP encoding failed",
            Self::CachePoisoned => "Console DPoP cache is unavailable",
        })
    }
}

impl std::error::Error for GrokConsoleDpopError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    fn token_for(key: &SigningKey, exp: u64) -> TestResult<String> {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"ES256","typ":"JWT"}"#);
        let jwk = DpopJwk::from_key(key.verifying_key());
        let claims = serde_json::json!({"exp": exp, "cnf": {"jkt": jwk.thumbprint()?}});
        Ok(format!(
            "{}.{}.sig",
            header,
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims)?)
        ))
    }

    #[test]
    fn proof_contains_dpop_claims_and_fixed_width_signature() -> TestResult {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let key = GrokConsoleDpopSession::generate_key();
        let token = token_for(&key, 1_700_000_600)?;
        let session = GrokConsoleDpopSession::from_token_response(token, "DPoP", 600, key, now)?;
        let proof = session.proof("post", "https://console.x.ai/v1/responses", now)?;
        let parts: Vec<_> = proof.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(URL_SAFE_NO_PAD.decode(parts[2])?.len(), 64);
        let claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1])?)?;
        assert_eq!(claims["htm"], "POST");
        assert_eq!(claims["htu"], "https://console.x.ai/v1/responses");
        assert!(claims["ath"].as_str().is_some());
        Ok(())
    }

    #[test]
    fn cache_invalidation_does_not_remove_a_successor() -> TestResult {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let key = GrokConsoleDpopSession::generate_key();
        let first = GrokConsoleDpopSession::from_token_response(
            token_for(&key, 1_700_000_600)?,
            "DPoP",
            600,
            key.clone(),
            now,
        )?;
        let successor_key = GrokConsoleDpopSession::generate_key();
        let successor = GrokConsoleDpopSession::from_token_response(
            token_for(&successor_key, 1_700_000_600)?,
            "DPoP",
            600,
            successor_key,
            now,
        )?;
        let cache = GrokConsoleDpopSessionCache::default();
        cache.insert("account|egress".to_owned(), successor)?;
        cache.invalidate("account|egress", first.access_token())?;
        assert!(cache.get("account|egress", now).is_some());
        Ok(())
    }
}
