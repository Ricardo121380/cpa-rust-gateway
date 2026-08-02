//! Strict, value-redacting credential boundary for API-key and Codex OAuth runtimes.

use std::{
    error::Error,
    fmt::{self, Write as _},
};

use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};
use serde::{Deserialize, de};
use zeroize::{Zeroize, Zeroizing};

/// Fixed OAuth client identity used by the incumbent Codex implementation.
pub const CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// Fixed Codex token endpoint. A caller must still pass it through DNS-pinned egress admission.
pub const CODEX_OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const MAX_CREDENTIAL_BYTES: usize = 64 * 1024;

/// A credential accepted by the OpenAI-compatible runtime.
pub enum OpenAiCompatibleRuntimeCredential {
    /// Opaque API-key/Bearer material.
    ApiKey(Zeroizing<String>),
    /// Codex OAuth material with an explicit expiry and refresh token.
    CodexOAuth(CodexOAuthCredential),
}

/// Opaque Codex OAuth state. Its fields are intentionally inaccessible outside this module.
pub struct CodexOAuthCredential {
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
    expires_at_ms: i64,
    account_id: Option<Zeroizing<String>>,
}

/// A value-owning refresh request that can only be sent by an explicitly composed refresh worker.
pub struct CodexOAuthRefreshRequest {
    refresh_token: Zeroizing<String>,
}

impl CodexOAuthRefreshRequest {
    /// Returns the fixed DNS-pinned-admission target.
    #[must_use]
    pub const fn token_url() -> &'static str {
        CODEX_OAUTH_TOKEN_URL
    }

    /// Returns an encoded OAuth form. The result is zeroized on drop and must never be logged.
    #[must_use]
    pub fn form_body(&self) -> Zeroizing<String> {
        Zeroizing::new(format!(
            "client_id={CODEX_OAUTH_CLIENT_ID}&grant_type=refresh_token&refresh_token={}&scope=openid%20profile%20email",
            percent_encode(self.refresh_token.as_bytes())
        ))
    }
}

impl fmt::Debug for CodexOAuthRefreshRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexOAuthRefreshRequest")
            .field("token_url", &CODEX_OAUTH_TOKEN_URL)
            .field("refresh_token", &"[REDACTED]")
            .finish()
    }
}

impl OpenAiCompatibleRuntimeCredential {
    /// Imports either an opaque API key or a strict tagged Codex OAuth JSON document.
    ///
    /// # Errors
    ///
    /// Returns a value-free error for empty, oversized, duplicate-key, malformed, expired-field,
    /// or unknown-field input.
    pub fn import(input: &[u8]) -> Result<Self, OpenAiRuntimeCredentialError> {
        if input.is_empty() || input.len() > MAX_CREDENTIAL_BYTES {
            return Err(OpenAiRuntimeCredentialError::Invalid);
        }
        let trimmed = input.trim_ascii();
        if trimmed.first() != Some(&b'{') {
            if trimmed.len() != input.len() {
                return Err(OpenAiRuntimeCredentialError::Invalid);
            }
            let value = std::str::from_utf8(input)
                .map_err(|_| OpenAiRuntimeCredentialError::Invalid)?
                .to_owned();
            if value.trim().is_empty() {
                return Err(OpenAiRuntimeCredentialError::Invalid);
            }
            return Ok(Self::ApiKey(Zeroizing::new(value)));
        }
        reject_duplicate_json_names(trimmed)?;
        let document: CodexOAuthDocument =
            serde_json::from_slice(trimmed).map_err(|_| OpenAiRuntimeCredentialError::Invalid)?;
        if document.kind != "codex_oauth"
            || document.access_token.trim().is_empty()
            || document.refresh_token.trim().is_empty()
            || document.expires_at_ms <= 0
            || document.account_id.as_deref().is_some_and(str::is_empty)
        {
            return Err(OpenAiRuntimeCredentialError::Invalid);
        }
        Ok(Self::CodexOAuth(CodexOAuthCredential {
            access_token: Zeroizing::new(document.access_token),
            refresh_token: Zeroizing::new(document.refresh_token),
            expires_at_ms: document.expires_at_ms,
            account_id: document.account_id.map(Zeroizing::new),
        }))
    }

    /// Returns the current Bearer only while this credential is usable.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnauthorized/Credential` after a Codex OAuth expiry.
    pub fn bearer_at(&self, now_ms: i64) -> Result<&str, GatewayError> {
        match self {
            Self::ApiKey(value) => Ok(value.as_str()),
            Self::CodexOAuth(value) if now_ms < value.expires_at_ms => {
                Ok(value.access_token.as_str())
            }
            Self::CodexOAuth(_) => Err(GatewayError::new(
                GatewayErrorCode::CredentialUnauthorized,
                ErrorScope::Credential,
            )),
        }
    }

    /// Builds a refresh handoff only for Codex OAuth material.
    ///
    /// # Errors
    ///
    /// Returns a value-free error for an API key, which has no refresh operation.
    pub fn refresh_request(
        &self,
    ) -> Result<CodexOAuthRefreshRequest, OpenAiRuntimeCredentialError> {
        match self {
            Self::ApiKey(_) => Err(OpenAiRuntimeCredentialError::NotRefreshable),
            Self::CodexOAuth(value) => Ok(CodexOAuthRefreshRequest {
                refresh_token: Zeroizing::new(value.refresh_token.to_string()),
            }),
        }
    }

    /// Applies one strict successful token response, retaining the prior refresh token when the
    /// provider omits rotation.
    ///
    /// # Errors
    ///
    /// Returns a value-free error for malformed, duplicate-key, empty, or non-positive expiry
    /// responses, and for API keys.
    pub fn apply_refresh_response(
        &mut self,
        input: &[u8],
        now_ms: i64,
    ) -> Result<(), OpenAiRuntimeCredentialError> {
        reject_duplicate_json_names(input)?;
        let response: CodexRefreshResponse =
            serde_json::from_slice(input).map_err(|_| OpenAiRuntimeCredentialError::Invalid)?;
        if response.access_token.trim().is_empty()
            || response.expires_in <= 0
            || response
                .refresh_token
                .as_deref()
                .is_some_and(|token| token.trim().is_empty())
        {
            return Err(OpenAiRuntimeCredentialError::Invalid);
        }
        let expires_at_ms = now_ms
            .checked_add(
                response
                    .expires_in
                    .checked_mul(1_000)
                    .ok_or(OpenAiRuntimeCredentialError::Invalid)?,
            )
            .ok_or(OpenAiRuntimeCredentialError::Invalid)?;
        let Self::CodexOAuth(current) = self else {
            return Err(OpenAiRuntimeCredentialError::NotRefreshable);
        };
        current.access_token.zeroize();
        current.access_token = Zeroizing::new(response.access_token);
        if let Some(refresh_token) = response.refresh_token {
            current.refresh_token.zeroize();
            current.refresh_token = Zeroizing::new(refresh_token);
        }
        current.expires_at_ms = expires_at_ms;
        Ok(())
    }

    /// Returns whether this carries a non-empty account binding without exposing its value.
    #[must_use]
    pub fn has_account_binding(&self) -> bool {
        matches!(self, Self::CodexOAuth(value) if value.account_id.is_some())
    }
}

impl fmt::Debug for OpenAiCompatibleRuntimeCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey(_) => {
                formatter.write_str("OpenAiCompatibleRuntimeCredential::ApiKey([REDACTED])")
            }
            Self::CodexOAuth(value) => formatter
                .debug_struct("OpenAiCompatibleRuntimeCredential::CodexOAuth")
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
struct CodexOAuthDocument {
    kind: String,
    access_token: String,
    refresh_token: String,
    expires_at_ms: i64,
    account_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodexRefreshResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    #[serde(default, rename = "token_type")]
    _token_type: Option<String>,
    #[serde(default, rename = "id_token")]
    _id_token: Option<String>,
}

/// Secret-free credential import or refresh failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAiRuntimeCredentialError {
    /// Input did not satisfy the strict credential contract.
    Invalid,
    /// The selected credential kind has no refresh operation.
    NotRefreshable,
}

impl fmt::Display for OpenAiRuntimeCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "invalid OpenAI-compatible credential",
            Self::NotRefreshable => "credential is not refreshable",
        })
    }
}

impl Error for OpenAiRuntimeCredentialError {}

fn percent_encode(input: &[u8]) -> String {
    let mut output = String::new();
    for byte in input {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                output.push(char::from(*byte));
            }
            _ => {
                let _ = write!(output, "%{byte:02X}");
            }
        }
    }
    output
}

fn reject_duplicate_json_names(input: &[u8]) -> Result<(), OpenAiRuntimeCredentialError> {
    serde_json::from_slice::<DuplicateFreeJson>(input)
        .map(|_| ())
        .map_err(|_| OpenAiRuntimeCredentialError::Invalid)
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
    fn api_key_and_oauth_are_strict_and_debug_is_redacted() -> Result<(), Box<dyn Error>> {
        let key = OpenAiCompatibleRuntimeCredential::import(b"api-key-value")?;
        assert_eq!(key.bearer_at(i64::MAX)?, "api-key-value");
        assert!(!format!("{key:?}").contains("api-key-value"));

        let oauth = OpenAiCompatibleRuntimeCredential::import(
            br#"{"kind":"codex_oauth","access_token":"access-value","refresh_token":"refresh-value","expires_at_ms":200,"account_id":"account-value"}"#,
        )?;
        assert_eq!(oauth.bearer_at(199)?, "access-value");
        let Err(expired) = oauth.bearer_at(200) else {
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
    fn refresh_rotates_access_and_optionally_refresh_token() -> Result<(), Box<dyn Error>> {
        let mut oauth = OpenAiCompatibleRuntimeCredential::import(
            br#"{"kind":"codex_oauth","access_token":"old-access","refresh_token":"old refresh/+","expires_at_ms":100}"#,
        )?;
        let request = oauth.refresh_request()?;
        let form = request.form_body();
        assert!(form.contains("grant_type=refresh_token"));
        assert!(form.contains("old%20refresh%2F%2B"));
        assert!(!format!("{request:?}").contains("old refresh"));

        oauth.apply_refresh_response(
            br#"{"access_token":"new-access","expires_in":30,"token_type":"bearer"}"#,
            1_000,
        )?;
        assert_eq!(oauth.bearer_at(30_999)?, "new-access");
        let second = oauth.refresh_request()?.form_body();
        assert!(second.contains("old%20refresh%2F%2B"));
        Ok(())
    }

    #[test]
    fn malformed_duplicate_and_unknown_fields_fail_closed() {
        assert!(OpenAiCompatibleRuntimeCredential::import(br#"{"kind":"codex_oauth","kind":"codex_oauth","access_token":"a","refresh_token":"r","expires_at_ms":1}"#).is_err());
        assert!(OpenAiCompatibleRuntimeCredential::import(br#"{"kind":"codex_oauth","access_token":"a","refresh_token":"r","expires_at_ms":1,"foreign":true}"#).is_err());
        assert!(OpenAiCompatibleRuntimeCredential::import(b" api-key-value").is_err());
    }
}
