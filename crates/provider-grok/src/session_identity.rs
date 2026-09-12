//! Fixed Grok Web/Console SSO session identity; no inference, token exchange or cookie mutation.
use crate::{GrokAccountProvider, GrokConsoleSsoToken, GrokWebCredential};
use gateway_provider::ProviderFuture;
use gateway_store::account_identity::AccountIdentity;
use serde::Deserialize;
use zeroize::Zeroizing;

/// Shared Web Session endpoint used by both Web and Console SSO accounts.
pub const GROK_SESSION_IDENTITY_URL: &str = "https://grok.com/api/auth/session";
/// Maximum decoded Session response bytes.
pub const MAX_GROK_SESSION_IDENTITY_BYTES: usize = 65_536;

/// Safe lookup failure without provider payloads, cookies or identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokSessionIdentityError {
    /// No valid SSO credential or authenticated provider session exists.
    Unauthorized,
    /// The provider explicitly refused this identity read (HTTP 403).
    Forbidden,
    /// The bounded provider transport could not complete the read.
    Unavailable,
    /// The provider response did not supply valid human identity.
    InvalidResponse,
}
impl std::fmt::Display for GrokSessionIdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unauthorized => "SSO session is not authenticated",
            Self::Forbidden => "SSO identity read was refused",
            Self::Unavailable => "SSO identity is unavailable",
            Self::InvalidResponse => "SSO identity response is invalid",
        })
    }
}
impl std::error::Error for GrokSessionIdentityError {}

/// Redacted, fixed-target request carrying only the account's validated scoped Cookies.
pub struct GrokSessionIdentityRequest {
    cookie: Zeroizing<String>,
}
impl std::fmt::Debug for GrokSessionIdentityRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrokSessionIdentityRequest(<redacted>)")
    }
}
impl GrokSessionIdentityRequest {
    /// Builds a Session request without contacting the provider.
    /// # Errors
    /// Rejects unsupported providers, expired Web credentials and malformed Cookies.
    pub fn from_credential(
        provider: GrokAccountProvider,
        bytes: &[u8],
        now_ms: i64,
    ) -> Result<Self, GrokSessionIdentityError> {
        let cookie = match provider {
            GrokAccountProvider::Console => GrokConsoleSsoToken::try_from_bytes(bytes)
                .map_err(|_| GrokSessionIdentityError::Unauthorized)?
                .cookie_header(),
            GrokAccountProvider::Web => {
                let credential = GrokWebCredential::import_sso_json(bytes, now_ms)
                    .map_err(|_| GrokSessionIdentityError::Unauthorized)?;
                let mut cookie = Zeroizing::new(String::new());
                for item in credential.cookies().iter().filter(|c| {
                    c.domain().trim_start_matches('.') == "grok.com"
                        && "/api/auth/session".starts_with(c.path())
                }) {
                    if !cookie.is_empty() {
                        cookie.push_str("; ");
                    }
                    cookie.push_str(item.name());
                    cookie.push('=');
                    cookie.push_str(item.value());
                }
                if cookie.is_empty() {
                    return Err(GrokSessionIdentityError::Unauthorized);
                }
                cookie
            }
            GrokAccountProvider::Build => return Err(GrokSessionIdentityError::Unauthorized),
        };
        Ok(Self { cookie })
    }
    /// Borrows the secret Cookie header only for immediate transport.
    #[must_use]
    pub fn cookie(&self) -> &str {
        self.cookie.as_str()
    }
}

/// Explicit provider transport; implementations must bound time/body and disable redirects/replay.
pub trait GrokSessionIdentityTransport: Send + Sync {
    /// Performs one Session read, returning only allowlisted display fields.
    fn fetch(
        &self,
        request: GrokSessionIdentityRequest,
    ) -> ProviderFuture<'_, Result<AccountIdentity, GrokSessionIdentityError>>;
}

#[derive(Deserialize)]
struct SessionState {
    status: Option<String>,
}

/// Parses authenticated Session identity and rejects blocked responses with residual profile fields.
/// # Errors
/// Rejects unavailable session states, invalid JSON, oversized bodies and absent human identity.
pub fn parse_grok_session_identity(
    body: &[u8],
) -> Result<AccountIdentity, GrokSessionIdentityError> {
    if body.len() > MAX_GROK_SESSION_IDENTITY_BYTES {
        return Err(GrokSessionIdentityError::InvalidResponse);
    }
    let state: SessionState =
        serde_json::from_slice(body).map_err(|_| GrokSessionIdentityError::InvalidResponse)?;
    if state
        .status
        .as_ref()
        .is_some_and(|s| !s.eq_ignore_ascii_case("authenticated"))
    {
        return Err(GrokSessionIdentityError::Unauthorized);
    }
    let identity = AccountIdentity::from_credential(body);
    if identity.is_empty() {
        return Err(GrokSessionIdentityError::InvalidResponse);
    }
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authenticated_session_and_residual_identity() {
        let good=br#"{"status":"authenticated","session":{"userId":"opaque","email":"member@example.test"}}"#;
        assert_eq!(
            parse_grok_session_identity(good)
                .ok()
                .and_then(|v| v.email)
                .as_deref(),
            Some("member@example.test")
        );
        for status in ["blocked", "unauthenticated", "unknown"] {
            let body =
                format!(r#"{{"status":"{status}","user":{{"email":"stale@example.test"}}}}"#);
            assert_eq!(
                parse_grok_session_identity(body.as_bytes()),
                Err(GrokSessionIdentityError::Unauthorized)
            );
        }
        assert_eq!(
            parse_grok_session_identity(br#"{"user":{"id":"opaque"}}"#),
            Err(GrokSessionIdentityError::InvalidResponse)
        );
        assert!(parse_grok_session_identity(&vec![b' '; 65_537]).is_err());
    }
}
