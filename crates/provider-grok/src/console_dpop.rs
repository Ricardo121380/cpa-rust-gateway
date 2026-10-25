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
use p256::{
    ecdsa::{Signature, SigningKey, signature::Signer},
    elliptic_curve::Generate,
};
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
    pub fn generate_key() -> Result<SigningKey, GrokConsoleDpopError> {
        SigningKey::try_generate().map_err(|_| GrokConsoleDpopError::Randomness)
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

    /// Returns the non-secret absolute session expiry as Unix milliseconds.
    pub fn expires_at_ms(&self) -> Result<i64, GrokConsoleDpopError> {
        let millis = self
            .expires_at
            .duration_since(UNIX_EPOCH)
            .map_err(|_| GrokConsoleDpopError::Clock)?
            .as_millis();
        i64::try_from(millis).map_err(|_| GrokConsoleDpopError::Clock)
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
        let point = key.to_sec1_point(false);
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
    use p256::ecdsa::{VerifyingKey, signature::Verifier};
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
        let key = GrokConsoleDpopSession::generate_key()?;
        let token = token_for(&key, 1_700_000_600)?;
        let exchange: Value =
            serde_json::from_slice(&GrokConsoleDpopSession::token_exchange_body(&key)?)?;
        let jwk = &exchange["jwk"];
        assert_eq!(jwk["kty"], "EC");
        assert_eq!(jwk["crv"], "P-256");
        assert_eq!(jwk.as_object().ok_or("JWK must be an object")?.len(), 4);
        let x = jwk["x"].as_str().ok_or("missing x")?;
        let y = jwk["y"].as_str().ok_or("missing y")?;
        assert!(!x.contains('='));
        assert!(!y.contains('='));
        let x_bytes = URL_SAFE_NO_PAD.decode(x)?;
        let y_bytes = URL_SAFE_NO_PAD.decode(y)?;
        assert_eq!(x_bytes.len(), 32);
        assert_eq!(y_bytes.len(), 32);
        let mut sec1 = vec![4];
        sec1.extend_from_slice(&x_bytes);
        sec1.extend_from_slice(&y_bytes);
        let verifier = VerifyingKey::from_sec1_bytes(&sec1)?;
        assert_eq!(&verifier, key.verifying_key());
        let canonical = format!(r#"{{"crv":"P-256","kty":"EC","x":"{x}","y":"{y}"}}"#);
        let (_, thumbprint) = parse_access_token_claims(&token)?;
        assert_eq!(
            thumbprint,
            URL_SAFE_NO_PAD.encode(Sha256::digest(canonical))
        );
        let expected_ath = URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()));
        let session = GrokConsoleDpopSession::from_token_response(token, "DPoP", 600, key, now)?;
        for (method, htu) in [
            ("post", "https://console.x.ai/v1/responses"),
            ("get", "https://console.x.ai/v1/usage"),
        ] {
            let proof = session.proof(method, htu, now)?;
            let parts: Vec<_> = proof.split('.').collect();
            assert_eq!(parts.len(), 3);
            let header: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0])?)?;
            assert_eq!(header["typ"], "dpop+jwt");
            assert_eq!(header["alg"], "ES256");
            assert_eq!(&header["jwk"], jwk);
            let mut claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1])?)?;
            assert_eq!(claims["htm"], method.to_ascii_uppercase());
            assert_eq!(claims["htu"], htu);
            assert_eq!(claims["iat"], 1_700_000_000_u64);
            assert_eq!(claims["ath"], expected_ath);
            assert_eq!(
                URL_SAFE_NO_PAD
                    .decode(claims["jti"].as_str().ok_or("jti")?)?
                    .len(),
                16
            );
            let signature_bytes = URL_SAFE_NO_PAD.decode(parts[2])?;
            assert_eq!(signature_bytes.len(), 64);
            let signature = Signature::from_slice(&signature_bytes)?;
            let signing_input = format!("{}.{}", parts[0], parts[1]);
            verifier.verify(signing_input.as_bytes(), &signature)?;
            assert!(
                verifier
                    .verify(format!("{signing_input}x").as_bytes(), &signature)
                    .is_err()
            );
            claims["htm"] = Value::String("PUT".to_owned());
            let tampered = format!(
                "{}.{}",
                parts[0],
                URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims)?)
            );
            assert!(verifier.verify(tampered.as_bytes(), &signature).is_err());
        }
        Ok(())
    }

    #[test]
    fn es256_matches_rfc6979_p256_sha256_vector() -> TestResult {
        // Public test vector: RFC 6979 Appendix A.2.5, SHA-256 and message "sample".
        // https://www.rfc-editor.org/rfc/rfc6979#appendix-A.2.5
        let key = SigningKey::from_slice(&[
            0xc9, 0xaf, 0xa9, 0xd8, 0x45, 0xba, 0x75, 0x16, 0x6b, 0x5c, 0x21, 0x57, 0x67, 0xb1,
            0xd6, 0x93, 0x4e, 0x50, 0xc3, 0xdb, 0x36, 0xe8, 0x9b, 0x12, 0x7b, 0x8a, 0x62, 0x2b,
            0x12, 0x0f, 0x67, 0x21,
        ])?;
        let signature: Signature = key.sign(b"sample");
        assert_eq!(
            signature.to_bytes().as_slice(),
            &[
                0xef, 0xd4, 0x8b, 0x2a, 0xac, 0xb6, 0xa8, 0xfd, 0x11, 0x40, 0xdd, 0x9c, 0xd4, 0x5e,
                0x81, 0xd6, 0x9d, 0x2c, 0x87, 0x7b, 0x56, 0xaa, 0xf9, 0x91, 0xc3, 0x4d, 0x0e, 0xa8,
                0x4e, 0xaf, 0x37, 0x16, 0xf7, 0xcb, 0x1c, 0x94, 0x2d, 0x65, 0x7c, 0x41, 0xd4, 0x36,
                0xc7, 0xa1, 0xb6, 0xe2, 0x9f, 0x65, 0xf3, 0xe9, 0x00, 0xdb, 0xb9, 0xaf, 0xf4, 0x06,
                0x4d, 0xc4, 0xab, 0x2f, 0x84, 0x3a, 0xcd, 0xa8,
            ]
        );
        Ok(())
    }

    #[test]
    fn dpop_jwk_preserves_the_fixed_p256_generator_coordinates() -> TestResult {
        // Synthetic scalar one gives the standard P-256 generator, never an account key.
        let mut scalar = [0; 32];
        scalar[31] = 1;
        let key = SigningKey::from_slice(&scalar).map_err(|_| "synthetic scalar")?;
        let body: Value =
            serde_json::from_slice(&GrokConsoleDpopSession::token_exchange_body(&key)?)?;
        assert_eq!(
            body,
            serde_json::json!({"jwk": {
                "kty": "EC", "crv": "P-256",
                "x": "axfR8uEsQkf4vOblY6RA8ncDfYEt6zOg9KE5RdiYwpY",
                "y": "T-NC4v4af5uO5-tKfA-eFivOM1drMV7Oy7ZAaDe_UfU"
            }})
        );
        Ok(())
    }

    #[test]
    fn dpop_session_rejects_wrong_key_binding_and_expired_token() -> TestResult {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let key = GrokConsoleDpopSession::generate_key()?;
        let other = GrokConsoleDpopSession::generate_key()?;
        assert_eq!(
            GrokConsoleDpopSession::from_token_response(
                token_for(&key, 1_700_000_600)?,
                "DPoP",
                600,
                other,
                now
            )
            .err(),
            Some(GrokConsoleDpopError::KeyMismatch)
        );
        assert_eq!(
            GrokConsoleDpopSession::from_token_response(
                token_for(&key, 1_700_000_010)?,
                "DPoP",
                600,
                key,
                now
            )
            .err(),
            Some(GrokConsoleDpopError::ExpiredToken)
        );
        Ok(())
    }

    #[test]
    fn p256_sha256_matches_rfc6979_vector_and_jwk_thumbprint() -> TestResult {
        // Public test vector: RFC 6979, Thomas Pornin, Appendix A.2.5, SHA-256 / "sample".
        // https://www.rfc-editor.org/rfc/rfc6979.html#appendix-A.2.5
        // Values below are the published scalar, coordinates and r || s in base64url.
        let key = SigningKey::from_slice(
            &URL_SAFE_NO_PAD.decode("ya-p2EW6dRZrXCFXZ7HWk05Qw9s26JsSe4piKxIPZyE")?,
        )?;
        let signature: Signature = key.sign(b"sample");
        assert_eq!(
            URL_SAFE_NO_PAD.encode(signature.to_bytes()),
            "79SLKqy2qP0RQN2c1F6B1p0sh3tWqvmRw00OqE6vNxb3yxyULWV8QdQ2x6G24p9l8-kA27mv9AZNxKsvhDrNqA"
        );
        let jwk = DpopJwk::from_key(key.verifying_key());
        assert_eq!(jwk.x, "YP7UuiVanTHJYet0xjVtaMBJuJI7Yfps5mliLmDyn7Y");
        assert_eq!(jwk.y, "eQP-EAi4vJmkGunpVii8ZPLxsgwtfp9Rd6PClNRGIpk");
        // Independently SHA-256 hashed canonical RFC 7638 JWK using the published coordinates.
        assert_eq!(
            jwk.thumbprint()?,
            "DOvxvJiAdIqVWIkFt5hDtCunXLF0BV4-JGv4f-ALSm0"
        );
        key.verifying_key().verify(b"sample", &signature)?;
        assert!(key.verifying_key().verify(b"tampered", &signature).is_err());
        Ok(())
    }

    #[test]
    fn cache_invalidation_does_not_remove_a_successor() -> TestResult {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let key = GrokConsoleDpopSession::generate_key()?;
        let first = GrokConsoleDpopSession::from_token_response(
            token_for(&key, 1_700_000_600)?,
            "DPoP",
            600,
            key.clone(),
            now,
        )?;
        let successor_key = GrokConsoleDpopSession::generate_key()?;
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
