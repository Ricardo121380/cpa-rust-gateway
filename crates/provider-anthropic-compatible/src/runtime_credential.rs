//! Strict API-key and Claude OAuth credential/refresh boundary.

use std::{error::Error, fmt};

use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};
use serde::{Deserialize, Serialize, de};
use zeroize::{Zeroize, Zeroizing};

use crate::AnthropicMessagesAuthorization;

/// Fixed token endpoint used by the pinned Claude OAuth reference.
pub const CLAUDE_OAUTH_TOKEN_URL: &str = "https://api.anthropic.com/v1/oauth/token";
/// Fixed public client identity used by the pinned Claude OAuth reference.
pub const CLAUDE_OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const MAX_CREDENTIAL_BYTES: usize = 64 * 1024;

/// One API-key or Claude OAuth credential held only inside a request/runtime boundary.
pub enum ClaudeRuntimeCredential {
    /// Generic Anthropic-compatible `x-api-key` material.
    ApiKey(Zeroizing<String>),
    /// Claude OAuth Bearer material with an explicit expiry and refresh token.
    OAuth(ClaudeOAuthCredential),
}

/// Opaque Claude OAuth state; fields are deliberately private.
pub struct ClaudeOAuthCredential {
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
    expires_at_ms: i64,
    account_id: Option<Zeroizing<String>>,
}

/// Secret-owning refresh handoff for an explicitly composed DNS-pinned worker.
pub struct ClaudeOAuthRefreshRequest {
    refresh_token: Zeroizing<String>,
}

impl ClaudeOAuthRefreshRequest {
    /// Returns the fixed target that still requires normal egress admission.
    #[must_use]
    pub const fn token_url() -> &'static str {
        CLAUDE_OAUTH_TOKEN_URL
    }

    /// Returns the strict JSON refresh body, zeroized on drop and never safe to log.
    ///
    /// # Errors
    ///
    /// Returns a value-free error only if JSON serialization fails.
    pub fn json_body(&self) -> Result<Zeroizing<Vec<u8>>, ClaudeRuntimeCredentialError> {
        let rotating_token = self.refresh_token.as_str();
        let body = serde_json::to_vec(&ClaudeRefreshBody {
            client_id: CLAUDE_OAUTH_CLIENT_ID,
            grant_type: "refresh_token",
            refresh_token: rotating_token,
        })
        .map_err(|_| ClaudeRuntimeCredentialError::Invalid)?;
        Ok(Zeroizing::new(body))
    }
}

#[derive(Serialize)]
struct ClaudeRefreshBody<'a> {
    client_id: &'static str,
    grant_type: &'static str,
    refresh_token: &'a str,
}

impl fmt::Debug for ClaudeOAuthRefreshRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClaudeOAuthRefreshRequest")
            .field("token_url", &CLAUDE_OAUTH_TOKEN_URL)
            .field("refresh_token", &"[REDACTED]")
            .finish()
    }
}

impl ClaudeRuntimeCredential {
    /// Imports a trimmed opaque API key or strict tagged `claude_oauth` JSON.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, whitespace-padded API-key, duplicate-name, unknown-field, or
    /// incomplete OAuth input without retaining a diagnostic value.
    pub fn import(input: &[u8]) -> Result<Self, ClaudeRuntimeCredentialError> {
        if input.is_empty() || input.len() > MAX_CREDENTIAL_BYTES {
            return Err(ClaudeRuntimeCredentialError::Invalid);
        }
        let trimmed = input.trim_ascii();
        if trimmed.first() != Some(&b'{') {
            if trimmed.len() != input.len() {
                return Err(ClaudeRuntimeCredentialError::Invalid);
            }
            let value = std::str::from_utf8(input)
                .map_err(|_| ClaudeRuntimeCredentialError::Invalid)?
                .to_owned();
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
                return Err(ClaudeRuntimeCredentialError::Invalid);
            }
            return Ok(Self::ApiKey(Zeroizing::new(value)));
        }
        reject_duplicate_json_names(trimmed)?;
        let document: ClaudeOAuthDocument =
            serde_json::from_slice(trimmed).map_err(|_| ClaudeRuntimeCredentialError::Invalid)?;
        if document.kind != "claude_oauth"
            || document.access_token.trim().is_empty()
            || document.refresh_token.trim().is_empty()
            || document.expires_at_ms <= 0
            || document.account_id.as_deref().is_some_and(str::is_empty)
        {
            return Err(ClaudeRuntimeCredentialError::Invalid);
        }
        Ok(Self::OAuth(ClaudeOAuthCredential {
            access_token: Zeroizing::new(document.access_token),
            refresh_token: Zeroizing::new(document.refresh_token),
            expires_at_ms: document.expires_at_ms,
            account_id: document.account_id.map(Zeroizing::new),
        }))
    }

    /// Projects the correct mutually-exclusive request authentication header.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnauthorized/Credential` when OAuth has expired.
    pub fn authorization_at(
        &self,
        now_ms: i64,
    ) -> Result<AnthropicMessagesAuthorization, GatewayError> {
        match self {
            Self::ApiKey(value) => AnthropicMessagesAuthorization::try_api_key(value.as_str()),
            Self::OAuth(value) if now_ms < value.expires_at_ms => {
                AnthropicMessagesAuthorization::try_bearer(value.access_token.as_str())
            }
            Self::OAuth(_) => Err(GatewayError::new(
                GatewayErrorCode::CredentialUnauthorized,
                ErrorScope::Credential,
            )),
        }
    }

    /// Builds a refresh handoff only for OAuth material.
    ///
    /// # Errors
    ///
    /// API keys return `NotRefreshable`.
    pub fn refresh_request(
        &self,
    ) -> Result<ClaudeOAuthRefreshRequest, ClaudeRuntimeCredentialError> {
        match self {
            Self::ApiKey(_) => Err(ClaudeRuntimeCredentialError::NotRefreshable),
            Self::OAuth(value) => Ok(ClaudeOAuthRefreshRequest {
                refresh_token: Zeroizing::new(value.refresh_token.to_string()),
            }),
        }
    }

    /// Atomically applies a strict successful OAuth refresh response.
    ///
    /// # Errors
    ///
    /// Rejects API keys, duplicate/unknown fields, empty tokens, non-positive expiry, and integer
    /// overflow before changing the current credential.
    pub fn apply_refresh_response(
        &mut self,
        input: &[u8],
        now_ms: i64,
    ) -> Result<(), ClaudeRuntimeCredentialError> {
        reject_duplicate_json_names(input)?;
        let response: ClaudeRefreshResponse =
            serde_json::from_slice(input).map_err(|_| ClaudeRuntimeCredentialError::Invalid)?;
        if response.access_token.trim().is_empty()
            || response.refresh_token.trim().is_empty()
            || response.expires_in <= 0
        {
            return Err(ClaudeRuntimeCredentialError::Invalid);
        }
        let expires_at_ms = now_ms
            .checked_add(
                response
                    .expires_in
                    .checked_mul(1_000)
                    .ok_or(ClaudeRuntimeCredentialError::Invalid)?,
            )
            .ok_or(ClaudeRuntimeCredentialError::Invalid)?;
        let Self::OAuth(current) = self else {
            return Err(ClaudeRuntimeCredentialError::NotRefreshable);
        };
        current.access_token.zeroize();
        current.refresh_token.zeroize();
        current.access_token = Zeroizing::new(response.access_token);
        current.refresh_token = Zeroizing::new(response.refresh_token);
        current.expires_at_ms = expires_at_ms;
        Ok(())
    }

    /// Returns account-binding presence without exposing its value.
    #[must_use]
    pub fn has_account_binding(&self) -> bool {
        matches!(self, Self::OAuth(value) if value.account_id.is_some())
    }
}

impl fmt::Debug for ClaudeRuntimeCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey(_) => formatter.write_str("ClaudeRuntimeCredential::ApiKey([REDACTED])"),
            Self::OAuth(value) => formatter
                .debug_struct("ClaudeRuntimeCredential::OAuth")
                .field("access_token", &"[REDACTED]")
                .field("refresh_token", &"[REDACTED]")
                .field("expires_at_ms", &value.expires_at_ms)
                .field("account_id_present", &value.account_id.is_some())
                .finish(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaudeOAuthDocument {
    kind: String,
    access_token: String,
    refresh_token: String,
    expires_at_ms: i64,
    account_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaudeRefreshResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    #[serde(default, rename = "token_type")]
    _token_type: Option<String>,
    #[serde(default, rename = "account")]
    _account: Option<de::IgnoredAny>,
    #[serde(default, rename = "organization")]
    _organization: Option<de::IgnoredAny>,
}

/// Value-free credential import/refresh failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaudeRuntimeCredentialError {
    /// Input did not satisfy the strict contract.
    Invalid,
    /// API-key credentials have no refresh operation.
    NotRefreshable,
}

impl fmt::Display for ClaudeRuntimeCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "invalid Anthropic-compatible credential",
            Self::NotRefreshable => "credential is not refreshable",
        })
    }
}

impl Error for ClaudeRuntimeCredentialError {}

fn reject_duplicate_json_names(input: &[u8]) -> Result<(), ClaudeRuntimeCredentialError> {
    serde_json::from_slice::<DuplicateFreeJson>(input)
        .map(|_| ())
        .map_err(|_| ClaudeRuntimeCredentialError::Invalid)
}

struct DuplicateFreeJson;

impl<'de> Deserialize<'de> for DuplicateFreeJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateFreeVisitor)
    }
}

struct DuplicateFreeVisitor;

impl<'de> de::Visitor<'de> for DuplicateFreeVisitor {
    type Value = DuplicateFreeJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object names")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }
    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }
    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }
    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }
    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(DuplicateFreeJson)
    }
    fn visit_string<E>(self, _: String) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }
    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        while sequence.next_element::<DuplicateFreeJson>()?.is_some() {}
        Ok(DuplicateFreeJson)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut names = std::collections::BTreeSet::new();
        while let Some(name) = map.next_key::<String>()? {
            if !names.insert(name) {
                return Err(de::Error::custom("duplicate object name"));
            }
            map.next_value::<DuplicateFreeJson>()?;
        }
        Ok(DuplicateFreeJson)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_and_oauth_choose_mutually_exclusive_headers() -> Result<(), Box<dyn Error>> {
        let key = ClaudeRuntimeCredential::import(b"anthropic-key")?;
        let key_auth = key.authorization_at(i64::MAX)?;
        assert_eq!(key_auth.header_name(), "x-api-key");

        let oauth = ClaudeRuntimeCredential::import(
            br#"{"kind":"claude_oauth","access_token":"access-value","refresh_token":"refresh-value","expires_at_ms":200,"account_id":"account-value"}"#,
        )?;
        let bearer = oauth.authorization_at(199)?;
        assert_eq!(bearer.header_name(), "authorization");
        assert!(bearer.header_value().starts_with("Bearer "));
        let Err(expired) = oauth.authorization_at(200) else {
            return Err("expired OAuth remained usable".into());
        };
        assert_eq!(expired.code(), GatewayErrorCode::CredentialUnauthorized);
        assert!(oauth.has_account_binding());
        let debug = format!("{oauth:?}");
        assert!(!debug.contains("access-value"));
        assert!(!debug.contains("refresh-value"));
        assert!(!debug.contains("account-value"));
        Ok(())
    }

    #[test]
    fn refresh_request_and_rotation_are_strict_and_redacted() -> Result<(), Box<dyn Error>> {
        let mut oauth = ClaudeRuntimeCredential::import(
            br#"{"kind":"claude_oauth","access_token":"old-access","refresh_token":"old-refresh","expires_at_ms":100}"#,
        )?;
        let request = oauth.refresh_request()?;
        let body = request.json_body()?;
        assert!(std::str::from_utf8(&body)?.contains("refresh_token"));
        assert!(!format!("{request:?}").contains("old-refresh"));
        oauth.apply_refresh_response(
            br#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":30,"token_type":"bearer"}"#,
            1_000,
        )?;
        assert_eq!(
            oauth.authorization_at(30_999)?.header_value(),
            "Bearer new-access"
        );
        Ok(())
    }

    #[test]
    fn malformed_duplicate_unknown_and_padded_values_fail_closed() {
        assert!(ClaudeRuntimeCredential::import(b" padded").is_err());
        assert!(ClaudeRuntimeCredential::import(br#"{"kind":"claude_oauth","kind":"claude_oauth","access_token":"a","refresh_token":"r","expires_at_ms":1}"#).is_err());
        assert!(ClaudeRuntimeCredential::import(br#"{"kind":"claude_oauth","access_token":"a","refresh_token":"r","expires_at_ms":1,"foreign":true}"#).is_err());
    }
}
