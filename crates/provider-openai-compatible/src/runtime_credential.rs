//! Strict, value-redacting credential boundary for API-key and Codex OAuth runtimes.

use std::{
    error::Error,
    fmt::{self, Write as _},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};
use serde::{Deserialize, de};
use serde_json::Value;
use serde_json::json;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use zeroize::{Zeroize, Zeroizing};

/// Fixed OAuth client identity used by the incumbent Codex implementation.
pub const CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// Fixed Codex token endpoint. A caller must still pass it through DNS-pinned egress admission.
pub const CODEX_OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const MAX_CREDENTIAL_BYTES: usize = 64 * 1024;

/// A credential accepted by the OpenAI-compatible runtime.
#[allow(clippy::large_enum_variant)]
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
    id_token: Option<Zeroizing<String>>,
    chatgpt_user_id: Option<String>,
    organization_id: Option<String>,
    client_id: Option<String>,
    last_refresh: Option<String>,
    metadata: CodexCredentialMetadata,
}

/// Non-secret management metadata extracted from an explicitly imported envelope.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct CodexCredentialMetadata {
    /// Provider plan label.
    pub plan: Option<String>,
    /// Provider quota label or bounded numeric value.
    pub quota: Option<String>,
    /// Source platform label.
    pub platform: Option<String>,
    /// Account email supplied by the explicit export.
    pub email: Option<String>,
    /// Accepted source envelope family.
    pub source_format: Option<String>,
}

impl fmt::Debug for CodexCredentialMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexCredentialMetadata")
            .field("plan", &self.plan)
            .field("quota", &self.quota)
            .field("platform", &self.platform)
            .field("email", &self.email.as_ref().map(|_| "[REDACTED]"))
            .field("source_format", &self.source_format)
            .finish()
    }
}

/// A value-owning refresh request that can only be sent by an explicitly composed refresh worker.
pub struct CodexOAuthRefreshRequest {
    refresh_token: Zeroizing<String>,
}

/// Explicit credential export shapes supported by the management API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexCredentialExportFormat {
    /// Flat CPA-compatible envelope.
    Cpa,
    /// Nested sub2api-compatible envelope.
    Sub2Api,
}

impl CodexCredentialExportFormat {
    /// Parses the stable management query value.
    ///
    /// # Errors
    ///
    /// Returns [`OpenAiRuntimeCredentialError::Invalid`] for an unknown format.
    pub fn parse(value: &str) -> Result<Self, OpenAiRuntimeCredentialError> {
        match value {
            "cpa" => Ok(Self::Cpa),
            "sub2api" => Ok(Self::Sub2Api),
            _ => Err(OpenAiRuntimeCredentialError::Invalid),
        }
    }
}

/// Revisioned owner used by a refresh worker to apply compare-and-swap semantics.
///
/// The management layer owns persistence; this small value type makes the concurrency contract
/// explicit without exposing token material or requiring a provider-specific database handle.
pub struct CodexOAuthRevisionedCredential {
    credential: OpenAiCompatibleRuntimeCredential,
    revision: u64,
}

impl CodexOAuthRevisionedCredential {
    /// Wraps one OAuth credential at its persisted revision.
    ///
    /// # Errors
    ///
    /// Returns [`OpenAiRuntimeCredentialError::NotRefreshable`] for an API-key credential.
    pub fn new(
        credential: OpenAiCompatibleRuntimeCredential,
        revision: u64,
    ) -> Result<Self, OpenAiRuntimeCredentialError> {
        if !matches!(credential, OpenAiCompatibleRuntimeCredential::CodexOAuth(_)) {
            return Err(OpenAiRuntimeCredentialError::NotRefreshable);
        }
        Ok(Self {
            credential,
            revision,
        })
    }

    /// Returns the current revision without exposing credential data.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns a refresh request for the current revision.
    ///
    /// # Errors
    ///
    /// Returns a value-free error when the wrapped credential is not refreshable.
    pub fn refresh_request(
        &self,
    ) -> Result<CodexOAuthRefreshRequest, OpenAiRuntimeCredentialError> {
        self.credential.refresh_request()
    }

    /// Applies a successful rotation only when the caller still owns the expected revision.
    ///
    /// A stale worker receives `Conflict` and must discard its response; it must never overwrite
    /// a newer refresh token. The revision is advanced only after the response is validated.
    ///
    /// # Errors
    ///
    /// Returns `Conflict` for a stale revision, or a value-free validation error for an invalid
    /// provider response.
    pub fn apply_refresh_if_revision(
        &mut self,
        expected_revision: u64,
        input: &[u8],
        now_ms: i64,
    ) -> Result<u64, OpenAiRuntimeCredentialError> {
        if self.revision != expected_revision {
            return Err(OpenAiRuntimeCredentialError::Conflict);
        }
        self.credential.apply_refresh_response(input, now_ms)?;
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(OpenAiRuntimeCredentialError::Invalid)?;
        Ok(self.revision)
    }

    /// Borrows the credential for request composition without exposing secret fields.
    #[must_use]
    pub const fn credential(&self) -> &OpenAiCompatibleRuntimeCredential {
        &self.credential
    }
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
            id_token: None,
            chatgpt_user_id: None,
            organization_id: None,
            client_id: None,
            last_refresh: None,
            metadata: CodexCredentialMetadata::default(),
        }))
    }

    /// Imports the three supported Codex credential families:
    /// CPAR's tagged document, CPA's flat export, and sub2api's `type:codex`
    /// export (including its optional nested `token` object).  The accepted
    /// field set is deliberately allow-listed so arbitrary configuration JSON
    /// cannot silently become a bearer credential.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` when the document is malformed, contains unsupported fields, has missing
    /// required token fields, or carries an expired or invalid expiry value.
    #[allow(clippy::too_many_lines)]
    pub fn import_compatible(
        input: &[u8],
        now_ms: i64,
    ) -> Result<Self, OpenAiRuntimeCredentialError> {
        if let Ok(value) = Self::import(input) {
            return Ok(value);
        }
        if input.is_empty() || input.len() > MAX_CREDENTIAL_BYTES {
            return Err(OpenAiRuntimeCredentialError::Invalid);
        }
        let trimmed = input.trim_ascii();
        reject_duplicate_json_names(trimmed)?;
        let root: Value =
            serde_json::from_slice(trimmed).map_err(|_| OpenAiRuntimeCredentialError::Invalid)?;
        let object = root
            .as_object()
            .ok_or(OpenAiRuntimeCredentialError::Invalid)?;
        let token = object
            .get("token")
            .or_else(|| object.get("tokens"))
            .or_else(|| object.get("credentials"))
            .and_then(Value::as_object)
            .unwrap_or(object);
        for key in object.keys().chain(token.keys()) {
            if !matches!(
                key.as_str(),
                "type"
                    | "kind"
                    | "provider"
                    | "auth_mode"
                    | "openai_api_key"
                    | "OPENAI_API_KEY"
                    | "name"
                    | "tokens"
                    | "credentials"
                    | "last_refresh"
                    | "agent_identity"
                    | "personal_access_token"
                    | "bedrock_api_key"
                    | "_meta"
                    | "access_token"
                    | "refresh_token"
                    | "expires_at"
                    | "expires_at_ms"
                    | "expired"
                    | "expires_in"
                    | "account_id"
                    | "chatgpt_account_id"
                    | "chatgpt_user_id"
                    | "email"
                    | "id_token"
                    | "org_id"
                    | "organization_id"
                    | "client_id"
                    | "plan_type"
                    | "plan"
                    | "package"
                    | "package_name"
                    | "subscription_tier"
                    | "quota"
                    | "platform"
                    | "notes"
                    | "extra"
                    | "group_ids"
                    | "concurrency"
                    | "priority"
                    | "auto_pause_on_expired"
                    | "token"
            ) {
                return Err(OpenAiRuntimeCredentialError::Invalid);
            }
        }
        validate_metadata_objects(object)?;
        let access_token = string_field(token, "access_token")?;
        let refresh_token = string_field(token, "refresh_token")?;
        let expires_at_ms = expiry_ms(token, now_ms)?;
        if access_token.trim().is_empty() || refresh_token.trim().is_empty() || expires_at_ms <= 0 {
            return Err(OpenAiRuntimeCredentialError::Invalid);
        }
        let source_format = if object.contains_key("token")
            || object.contains_key("tokens")
            || object.contains_key("credentials")
        {
            Some("sub2api".to_owned())
        } else {
            Some("cpa".to_owned())
        };
        let metadata = metadata_from_objects(object, token, source_format);
        let id_token = optional_string_field(token, "id_token");
        let access_token_text = token.get("access_token").and_then(Value::as_str);
        let user_id = optional_string_field(token, "chatgpt_user_id")
            .or_else(|| jwt_auth_field(access_token_text, "chatgpt_user_id"));
        let organization_id = optional_string_field(token, "organization_id")
            .or_else(|| optional_string_field(token, "org_id"))
            .or_else(|| jwt_auth_field(access_token_text, "organization_id"))
            .or_else(|| jwt_auth_field(id_token.as_deref(), "organization_id"))
            .or_else(|| jwt_auth_field(access_token_text, "poid"))
            .or_else(|| jwt_auth_field(id_token.as_deref(), "poid"));
        let client_id = optional_string_field(token, "client_id");
        let mut metadata = metadata;
        metadata.email = metadata
            .email
            .or_else(|| jwt_string_claim(id_token.as_deref(), "email"))
            .or_else(|| jwt_string_claim(access_token_text, "email"));
        metadata.plan = metadata
            .plan
            .or_else(|| jwt_auth_field(id_token.as_deref(), "chatgpt_plan_type"))
            .or_else(|| jwt_auth_field(access_token_text, "chatgpt_plan_type"));
        Ok(Self::CodexOAuth(CodexOAuthCredential {
            access_token: Zeroizing::new(access_token),
            refresh_token: Zeroizing::new(refresh_token),
            expires_at_ms,
            account_id: token
                .get("account_id")
                .or_else(|| token.get("chatgpt_account_id"))
                // Older CPA exports used `org_id` for the same account binding slot.  Keep it
                // only as a last explicit-field fallback; a JWT claim still wins below.
                .or_else(|| token.get("org_id"))
                .or_else(|| object.get("account_id"))
                .or_else(|| object.get("chatgpt_account_id"))
                .or_else(|| object.get("org_id"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| jwt_account_id(token.get("access_token").and_then(Value::as_str)))
                .or_else(|| jwt_account_id(token.get("id_token").and_then(Value::as_str)))
                .map(Zeroizing::new),
            id_token: id_token.map(Zeroizing::new),
            chatgpt_user_id: user_id,
            organization_id,
            client_id,
            last_refresh: optional_string_field(token, "last_refresh")
                .or_else(|| optional_string_field(object, "last_refresh")),
            metadata,
        }))
    }

    /// Normalizes one successful authorization-code token response into the same encrypted
    /// credential representation used by CPA and sub2api imports.
    ///
    /// OAuth protocol-only members such as `scope` and `token_type` are discarded. Duplicate JSON
    /// names, missing refresh material, invalid expiry, and arbitrary credential fields still fail
    /// closed before any value can reach persistence.
    ///
    /// # Errors
    ///
    /// Returns a value-free error when the response is malformed or does not contain a valid
    /// access/refresh pair and future expiry.
    pub fn import_oauth_token_response(
        input: &[u8],
        now_ms: i64,
    ) -> Result<Self, OpenAiRuntimeCredentialError> {
        if input.is_empty() || input.len() > MAX_CREDENTIAL_BYTES {
            return Err(OpenAiRuntimeCredentialError::Invalid);
        }
        reject_duplicate_json_names(input)?;
        let response: Value =
            serde_json::from_slice(input).map_err(|_| OpenAiRuntimeCredentialError::Invalid)?;
        let root = response
            .as_object()
            .ok_or(OpenAiRuntimeCredentialError::Invalid)?;
        let token = nested_token_object(root);
        let mut normalized = serde_json::Map::new();
        for key in [
            "access_token",
            "refresh_token",
            "expires_in",
            "expires_at",
            "expires_at_ms",
            "expired",
            "account_id",
            "chatgpt_account_id",
            "chatgpt_user_id",
            "organization_id",
            "org_id",
            "client_id",
            "id_token",
            "email",
            "plan_type",
            "plan",
            "package",
            "package_name",
            "subscription_tier",
            "quota",
            "platform",
        ] {
            if let Some(value) = token.get(key).or_else(|| root.get(key)) {
                normalized.insert(key.to_owned(), value.clone());
            }
        }
        if let Some(last_refresh) = rfc3339_from_ms(now_ms) {
            normalized.insert("last_refresh".to_owned(), Value::String(last_refresh));
        }
        for key in ["_meta", "extra"] {
            if let Some(value) = root.get(key) {
                normalized.insert(key.to_owned(), value.clone());
            }
        }
        let normalized = serde_json::to_vec(&Value::Object(normalized))
            .map_err(|_| OpenAiRuntimeCredentialError::Invalid)?;
        let mut credential = Self::import_compatible(&normalized, now_ms)?;
        if let Self::CodexOAuth(value) = &mut credential {
            value.metadata.source_format = Some("direct_oauth".to_owned());
        }
        Ok(credential)
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
        let response: Value =
            serde_json::from_slice(input).map_err(|_| OpenAiRuntimeCredentialError::Invalid)?;
        let root = response
            .as_object()
            .ok_or(OpenAiRuntimeCredentialError::Invalid)?;
        let token = nested_token_object(root);
        let access_token = string_field(token, "access_token")?;
        let rotated_refresh = optional_response_string(token, "refresh_token")?;
        let id_token = optional_response_string(token, "id_token")?;
        let expires_at_ms = refresh_expiry_ms(token, now_ms)?;
        let response_account_id = explicit_account_id(root, token)
            .or_else(|| jwt_account_id(token.get("access_token").and_then(Value::as_str)))
            .or_else(|| jwt_account_id(id_token.as_deref()));
        let response_user_id = optional_string_field(token, "chatgpt_user_id")
            .or_else(|| {
                jwt_auth_field(
                    token.get("access_token").and_then(Value::as_str),
                    "chatgpt_user_id",
                )
            })
            .or_else(|| jwt_auth_field(id_token.as_deref(), "chatgpt_user_id"));
        let response_organization_id = optional_string_field(token, "organization_id")
            .or_else(|| optional_string_field(token, "org_id"))
            .or_else(|| {
                jwt_auth_field(
                    token.get("access_token").and_then(Value::as_str),
                    "organization_id",
                )
            })
            .or_else(|| jwt_auth_field(id_token.as_deref(), "organization_id"))
            .or_else(|| jwt_auth_field(token.get("access_token").and_then(Value::as_str), "poid"))
            .or_else(|| jwt_auth_field(id_token.as_deref(), "poid"));
        let response_client_id = optional_string_field(token, "client_id");
        let mut metadata = metadata_from_objects(root, token, None);
        metadata.email = metadata
            .email
            .or_else(|| jwt_string_claim(id_token.as_deref(), "email"))
            .or_else(|| {
                jwt_string_claim(token.get("access_token").and_then(Value::as_str), "email")
            });
        metadata.plan = metadata
            .plan
            .or_else(|| jwt_auth_field(id_token.as_deref(), "chatgpt_plan_type"))
            .or_else(|| {
                jwt_auth_field(
                    token.get("access_token").and_then(Value::as_str),
                    "chatgpt_plan_type",
                )
            });
        let Self::CodexOAuth(current) = self else {
            return Err(OpenAiRuntimeCredentialError::NotRefreshable);
        };
        if let (Some(current), Some(next)) = (
            current.account_id.as_deref(),
            response_account_id.as_deref(),
        ) && current != next
        {
            // A refresh response for another account must never replace this credential.  Keep the
            // error value-free so the management boundary cannot become an account oracle.
            return Err(OpenAiRuntimeCredentialError::Invalid);
        }
        metadata.plan = metadata.plan.or_else(|| current.metadata.plan.clone());
        metadata.quota = metadata.quota.or_else(|| current.metadata.quota.clone());
        metadata.platform = metadata
            .platform
            .or_else(|| current.metadata.platform.clone());
        metadata.email = metadata.email.or_else(|| current.metadata.email.clone());
        metadata
            .source_format
            .clone_from(&current.metadata.source_format);
        current.access_token.zeroize();
        current.access_token = Zeroizing::new(access_token);
        if let Some(next_refresh) = rotated_refresh {
            current.refresh_token.zeroize();
            current.refresh_token = Zeroizing::new(next_refresh);
        }
        if let Some(id_token) = id_token {
            current.id_token = Some(Zeroizing::new(id_token));
        }
        if current.account_id.is_none() {
            current.account_id = response_account_id.map(Zeroizing::new);
        }
        current.chatgpt_user_id = response_user_id.or_else(|| current.chatgpt_user_id.clone());
        current.organization_id =
            response_organization_id.or_else(|| current.organization_id.clone());
        current.client_id = response_client_id.or_else(|| current.client_id.clone());
        current.expires_at_ms = expires_at_ms;
        current.last_refresh = rfc3339_from_ms(now_ms).or_else(|| current.last_refresh.clone());
        current.metadata = metadata;
        Ok(())
    }

    /// Returns whether this carries a non-empty account binding without exposing its value.
    #[must_use]
    pub fn has_account_binding(&self) -> bool {
        matches!(self, Self::CodexOAuth(value) if value.account_id.is_some())
    }

    /// Returns the account binding only for request-scoped official Codex headers.
    #[must_use]
    pub fn account_id(&self) -> Option<&str> {
        match self {
            Self::CodexOAuth(value) => value.account_id.as_deref().map(String::as_str),
            Self::ApiKey(_) => None,
        }
    }

    /// Returns non-secret metadata for the protected management read model.
    #[must_use]
    pub fn metadata(&self) -> Option<&CodexCredentialMetadata> {
        match self {
            Self::CodexOAuth(value) => Some(&value.metadata),
            Self::ApiKey(_) => None,
        }
    }

    /// Exports validated OAuth fields in the selected incumbent-compatible shape.
    ///
    /// # Errors
    ///
    /// Returns [`OpenAiRuntimeCredentialError::NotRefreshable`] for an API key or `Invalid` when
    /// the selected export cannot be serialized.
    pub fn export_json(
        &self,
        format: CodexCredentialExportFormat,
    ) -> Result<Zeroizing<Vec<u8>>, OpenAiRuntimeCredentialError> {
        let Self::CodexOAuth(value) = self else {
            return Err(OpenAiRuntimeCredentialError::NotRefreshable);
        };
        let account_id = value.account_id.as_deref().map_or("", String::as_str);
        let email = value.metadata.email.as_deref().unwrap_or("");
        let id_token = value.id_token.as_deref().map_or("", String::as_str);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_millis()).ok())
            .unwrap_or(0);
        let expires_at = value.expires_at_ms / 1_000;
        let expires_in = value
            .expires_at_ms
            .saturating_sub(now_ms)
            .checked_div(1_000)
            .unwrap_or(0)
            .max(0);
        let last_refresh = value.last_refresh.as_deref().unwrap_or("");
        let platform = value.metadata.platform.as_deref().unwrap_or("openai");
        let plan = value.metadata.plan.as_deref().unwrap_or("");
        let quota = value.metadata.quota.as_deref().unwrap_or("");
        let document = match format {
            CodexCredentialExportFormat::Cpa => json!({
                "type": "codex",
                "email": email,
                "expired": expires_at,
                "id_token": id_token,
                "account_id": account_id,
                "access_token": value.access_token.as_str(),
                "refresh_token": value.refresh_token.as_str(),
                "last_refresh": last_refresh,
            }),
            CodexCredentialExportFormat::Sub2Api => json!({
                "name": if email.is_empty() { account_id } else { email },
                "notes": "",
                "platform": platform,
                "type": "oauth",
                "credentials": {
                    "access_token": value.access_token.as_str(),
                    "refresh_token": value.refresh_token.as_str(),
                    "expires_in": expires_in,
                    "expires_at": expires_at,
                    "chatgpt_account_id": account_id,
                    "chatgpt_user_id": value.chatgpt_user_id.as_deref().unwrap_or(""),
                    "organization_id": value.organization_id.as_deref().unwrap_or(""),
                    "client_id": value.client_id.as_deref().unwrap_or(CODEX_OAUTH_CLIENT_ID),
                    "id_token": id_token,
                },
                "extra": {
                    "email": email,
                    "plan": plan,
                    "quota": quota,
                    "platform": platform,
                },
                "group_ids": [],
                "concurrency": 10,
                "priority": 1,
                "auto_pause_on_expired": true,
            }),
        };
        serde_json::to_vec(&document)
            .map(Zeroizing::new)
            .map_err(|_| OpenAiRuntimeCredentialError::Invalid)
    }
}

fn string_field(
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<String, OpenAiRuntimeCredentialError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or(OpenAiRuntimeCredentialError::Invalid)
}

fn optional_string_field(object: &serde_json::Map<String, Value>, name: &str) -> Option<String> {
    object
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn optional_response_string(
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<String>, OpenAiRuntimeCredentialError> {
    let Some(value) = object.get(name) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or(OpenAiRuntimeCredentialError::Invalid)?;
    Ok(Some(value.to_owned()))
}

fn nested_token_object(root: &serde_json::Map<String, Value>) -> &serde_json::Map<String, Value> {
    for name in ["token", "tokens", "credentials"] {
        if let Some(object) = root.get(name).and_then(Value::as_object) {
            return object;
        }
    }
    root
}

fn explicit_account_id(
    root: &serde_json::Map<String, Value>,
    token: &serde_json::Map<String, Value>,
) -> Option<String> {
    [
        token.get("account_id"),
        token.get("chatgpt_account_id"),
        root.get("account_id"),
        root.get("chatgpt_account_id"),
    ]
    .into_iter()
    .flatten()
    .find_map(Value::as_str)
    .filter(|value| !value.trim().is_empty())
    .map(ToOwned::to_owned)
}

fn validate_metadata_objects(
    root: &serde_json::Map<String, Value>,
) -> Result<(), OpenAiRuntimeCredentialError> {
    for name in ["_meta", "extra"] {
        let Some(value) = root.get(name) else {
            continue;
        };
        // These containers are metadata-only.  They must be JSON objects, but unknown members
        // are intentionally ignored rather than promoted to credential fields; this preserves
        // forward compatibility with CPA/Sub2API bookkeeping without weakening the token
        // allow-list above.
        let _ = value
            .as_object()
            .ok_or(OpenAiRuntimeCredentialError::Invalid)?;
    }
    Ok(())
}

fn metadata_from_objects(
    root: &serde_json::Map<String, Value>,
    token: &serde_json::Map<String, Value>,
    source_format: Option<String>,
) -> CodexCredentialMetadata {
    let meta = root.get("_meta").and_then(Value::as_object);
    let extra = root.get("extra").and_then(Value::as_object);
    let containers = [Some(token), Some(root), meta, extra];
    let lookup = |name: &str| {
        containers
            .iter()
            .flatten()
            .find_map(|object| object.get(name))
    };
    let text = |name: &str| lookup(name).and_then(bounded_metadata_scalar);
    let quota = text("quota").or_else(|| quota_summary(meta, extra));
    let platform = text("platform").or_else(|| {
        if meta
            .and_then(|value| value.get("platform"))
            .is_some_and(Value::is_object)
            || extra
                .and_then(|value| value.get("platform"))
                .is_some_and(Value::is_object)
        {
            Some("openai".to_owned())
        } else {
            None
        }
    });
    CodexCredentialMetadata {
        plan: text("plan_type")
            .or_else(|| text("plan"))
            .or_else(|| text("package"))
            .or_else(|| text("package_name"))
            .or_else(|| text("subscription_tier")),
        quota,
        platform,
        email: text("email"),
        source_format,
    }
}

fn bounded_metadata_scalar(value: &Value) -> Option<String> {
    let value = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => return None,
    };
    let value = value.trim();
    (!value.is_empty() && value.len() <= 128).then(|| value.to_owned())
}

fn quota_summary(
    meta: Option<&serde_json::Map<String, Value>>,
    extra: Option<&serde_json::Map<String, Value>>,
) -> Option<String> {
    let containers = [meta, extra];
    let lookup = |name: &str| {
        containers
            .iter()
            .flatten()
            .find_map(|object| object.get(name))
            .and_then(bounded_metadata_scalar)
    };
    let mut fields = Vec::new();
    for name in ["used_percent", "reset_after_secs", "balance"] {
        if let Some(value) = lookup(name) {
            fields.push(format!("{name}={value}"));
        }
    }
    if fields.is_empty() {
        None
    } else {
        let summary = fields.join(";");
        (summary.len() <= 128).then_some(summary)
    }
}

fn expiry_ms(
    object: &serde_json::Map<String, Value>,
    now_ms: i64,
) -> Result<i64, OpenAiRuntimeCredentialError> {
    if let Some(value) = object.get("expires_at_ms").and_then(Value::as_i64) {
        return Ok(value);
    }
    if let Some(value) = object
        .get("expires_in")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
    {
        return now_ms
            .checked_add(
                value
                    .checked_mul(1_000)
                    .ok_or(OpenAiRuntimeCredentialError::Invalid)?,
            )
            .ok_or(OpenAiRuntimeCredentialError::Invalid);
    }
    if let Some(expiry) = jwt_expiry_ms(object.get("access_token").and_then(Value::as_str))
        .or_else(|| jwt_expiry_ms(object.get("id_token").and_then(Value::as_str)))
    {
        return Ok(expiry);
    }
    let value = object
        .get("expires_at")
        .or_else(|| object.get("expired"))
        .ok_or(OpenAiRuntimeCredentialError::Invalid)?;
    let raw = value.as_i64().or_else(|| {
        value.as_str().and_then(|text| {
            text.parse::<i64>().ok().or_else(|| {
                OffsetDateTime::parse(text.trim(), &Rfc3339)
                    .ok()
                    .and_then(|value| value.unix_timestamp_nanos().checked_div(1_000_000))
                    .and_then(|value| i64::try_from(value).ok())
            })
        })
    });
    let raw = raw.ok_or(OpenAiRuntimeCredentialError::Invalid)?;
    Ok(if raw < 100_000_000_000 {
        raw.checked_mul(1_000)
            .ok_or(OpenAiRuntimeCredentialError::Invalid)?
    } else {
        raw
    })
}

fn refresh_expiry_ms(
    object: &serde_json::Map<String, Value>,
    now_ms: i64,
) -> Result<i64, OpenAiRuntimeCredentialError> {
    let candidate = object
        .get("expires_at_ms")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .or_else(|| {
            object
                .get("expires_in")
                .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
                .filter(|value| *value > 0)
                .and_then(|value| now_ms.checked_add(value.checked_mul(1_000)?))
        })
        .or_else(|| jwt_expiry_ms(object.get("access_token").and_then(Value::as_str)))
        .or_else(|| jwt_expiry_ms(object.get("id_token").and_then(Value::as_str)))
        .or_else(|| {
            let value = object.get("expires_at").or_else(|| object.get("expired"))?;
            let raw = value.as_i64().or_else(|| {
                value.as_str().and_then(|text| {
                    text.parse::<i64>().ok().or_else(|| {
                        OffsetDateTime::parse(text.trim(), &Rfc3339)
                            .ok()
                            .and_then(|value| value.unix_timestamp_nanos().checked_div(1_000_000))
                            .and_then(|value| i64::try_from(value).ok())
                    })
                })
            })?;
            if raw < 100_000_000_000 {
                raw.checked_mul(1_000)
            } else {
                Some(raw)
            }
        })
        .ok_or(OpenAiRuntimeCredentialError::Invalid)?;
    if candidate <= now_ms {
        return Err(OpenAiRuntimeCredentialError::Invalid);
    }
    Ok(candidate)
}

fn jwt_string_claim(token: Option<&str>, name: &str) -> Option<String> {
    let payload = token?.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    let value: Value = serde_json::from_slice(&decoded).ok()?;
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn rfc3339_from_ms(now_ms: i64) -> Option<String> {
    let timestamp = i128::from(now_ms).checked_mul(1_000_000)?;
    OffsetDateTime::from_unix_timestamp_nanos(timestamp)
        .ok()?
        .format(&Rfc3339)
        .ok()
}

fn jwt_expiry_ms(token: Option<&str>) -> Option<i64> {
    let payload = token?.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    let value: Value = serde_json::from_slice(&decoded).ok()?;
    value.get("exp")?.as_i64()?.checked_mul(1_000)
}

fn jwt_account_id(token: Option<&str>) -> Option<String> {
    jwt_auth_field(token, "chatgpt_account_id")
}

fn jwt_auth_field(token: Option<&str>, name: &str) -> Option<String> {
    let payload = token?.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    let value: Value = serde_json::from_slice(&decoded).ok()?;
    value
        .get("https://api.openai.com/auth")
        .and_then(Value::as_object)
        .and_then(|auth| auth.get(name))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
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

/// Secret-free credential import or refresh failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAiRuntimeCredentialError {
    /// Input did not satisfy the strict credential contract.
    Invalid,
    /// The selected credential kind has no refresh operation.
    NotRefreshable,
    /// A concurrent refresh already advanced the stored credential revision.
    Conflict,
}

impl fmt::Display for OpenAiRuntimeCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "invalid OpenAI-compatible credential",
            Self::NotRefreshable => "credential is not refreshable",
            Self::Conflict => "credential refresh revision conflict",
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
    fn compatible_import_accepts_cpa_and_sub2api_shapes() -> Result<(), Box<dyn Error>> {
        let cpa = br#"{"access_token":"a","refresh_token":"r","expires_at":"2030-01-01T00:00:00Z","account_id":"account","email":"user@example.test","expired":"2030-01-01T00:00:00Z"}"#;
        let imported = OpenAiCompatibleRuntimeCredential::import_compatible(cpa, 1_000)?;
        assert_eq!(imported.bearer_at(1_000)?, "a");
        assert!(imported.has_account_binding());

        let sub2api = br#"{"type":"codex","token":{"access_token":"a2","refresh_token":"r2","expires_in":30},"org_id":"org"}"#;
        let imported = OpenAiCompatibleRuntimeCredential::import_compatible(sub2api, 1_000)?;
        assert_eq!(imported.bearer_at(1_001)?, "a2");
        assert!(imported.has_account_binding());
        Ok(())
    }

    #[test]
    fn compatible_import_derives_account_id_from_openai_jwt_claim() -> Result<(), Box<dyn Error>> {
        let payload = URL_SAFE_NO_PAD.encode(
            br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"account"},"exp":2000}"#,
        );
        let access_token = format!("header.{payload}.signature");
        let document = format!(
            r#"{{"type":"codex","access_token":"{access_token}","refresh_token":"refresh"}}"#
        );
        let imported =
            OpenAiCompatibleRuntimeCredential::import_compatible(document.as_bytes(), 1_000)?;
        assert!(imported.has_account_binding());
        assert_eq!(imported.bearer_at(1_999_999)?, access_token);
        Ok(())
    }

    #[test]
    fn compatible_import_accepts_sub2api_chatgpt_go_envelope() -> Result<(), Box<dyn Error>> {
        let document = br#"{"auth_mode":"chatgpt","openai_api_key":"ignored","tokens":{"id_token":"id","access_token":"header.eyJleHAiOjIwMDB9.signature","refresh_token":"refresh","account_id":"account"},"_meta":{"plan_type":"go"}}"#;
        let imported = OpenAiCompatibleRuntimeCredential::import_compatible(document, 1_000)?;
        assert_eq!(
            imported.bearer_at(1_999_999)?,
            "header.eyJleHAiOjIwMDB9.signature"
        );
        assert!(imported.has_account_binding());
        Ok(())
    }

    #[test]
    fn compatible_import_accepts_native_codex_auth_envelope() -> Result<(), Box<dyn Error>> {
        let document = br#"{"OPENAI_API_KEY":null,"auth_mode":"chatgpt","last_refresh":"now","tokens":{"access_token":"header.eyJleHAiOjIwMDB9.signature","refresh_token":"refresh","account_id":"account","id_token":"id"}}"#;
        let imported = OpenAiCompatibleRuntimeCredential::import_compatible(document, 1_000)?;
        assert_eq!(
            imported.bearer_at(1_999_999)?,
            "header.eyJleHAiOjIwMDB9.signature"
        );
        assert!(imported.has_account_binding());
        Ok(())
    }

    #[test]
    fn compatible_import_rejects_unknown_fields_and_normalizes_expiry() -> Result<(), Box<dyn Error>>
    {
        assert!(
            OpenAiCompatibleRuntimeCredential::import_compatible(
                br#"{"access_token":"a","refresh_token":"r","expires_at":200,"secret":"x"}"#,
                1_000,
            )
            .is_err()
        );
        let imported = OpenAiCompatibleRuntimeCredential::import_compatible(
            br#"{"access_token":"a","refresh_token":"r","expires_at":1}"#,
            1_000,
        )?;
        assert!(imported.bearer_at(1_000).is_err());
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
        assert!(!form.contains("redirect_uri="));
        assert!(form.contains("scope=openid%20profile%20email"));
        assert!(!format!("{request:?}").contains("old refresh"));

        oauth.apply_refresh_response(
            br#"{"access_token":"new-access","expires_in":30,"token_type":"bearer","scope":"openid profile email offline_access"}"#,
            1_000,
        )?;
        assert_eq!(oauth.bearer_at(30_999)?, "new-access");
        let second = oauth.refresh_request()?.form_body();
        assert!(second.contains("old%20refresh%2F%2B"));
        Ok(())
    }

    #[test]
    fn refresh_accepts_jwt_expiry_and_rotates_id_token_metadata() -> Result<(), Box<dyn Error>> {
        let mut oauth = OpenAiCompatibleRuntimeCredential::import_compatible(
            br#"{"type":"oauth","credentials":{"access_token":"old","refresh_token":"old-refresh","expires_in":30,"chatgpt_account_id":"account"},"extra":{"email":"old@example.test","plan":"go"}}"#,
            1_000,
        )?;
        let id_token = "header.eyJleHAiOjIwMDAsImVtYWlsIjoibmV3QGV4YW1wbGUudGVzdCIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X3BsYW5fdHlwZSI6InBybyIsImNoYXRncHRfYWNjb3VudF9pZCI6ImFjY291bnQifX0.signature";
        let response = format!(r#"{{"access_token":"new","id_token":"{id_token}"}}"#);
        oauth.apply_refresh_response(response.as_bytes(), 1_000)?;
        assert_eq!(oauth.bearer_at(1_999_999)?, "new");
        assert_eq!(
            oauth.metadata().and_then(|m| m.email.as_deref()),
            Some("new@example.test")
        );
        assert_eq!(
            oauth.metadata().and_then(|m| m.plan.as_deref()),
            Some("pro")
        );
        Ok(())
    }

    #[test]
    fn refresh_rejects_account_switch_and_preserves_old_values() -> Result<(), Box<dyn Error>> {
        let mut oauth = OpenAiCompatibleRuntimeCredential::import(
            br#"{"kind":"codex_oauth","access_token":"old","refresh_token":"old-refresh","expires_at_ms":100,"account_id":"account-a"}"#,
        )?;
        assert_eq!(
            oauth.apply_refresh_response(
                br#"{"access_token":"new","refresh_token":"new-refresh","expires_in":30,"account_id":"account-b"}"#,
                1_000,
            ),
            Err(OpenAiRuntimeCredentialError::Invalid)
        );
        assert_eq!(oauth.bearer_at(99)?, "old");
        let request = oauth.refresh_request()?.form_body();
        assert!(request.contains("old-refresh"));
        Ok(())
    }

    #[test]
    fn refresh_preserves_rotating_refresh_token_when_response_omits_it()
    -> Result<(), Box<dyn Error>> {
        let mut oauth = OpenAiCompatibleRuntimeCredential::import(
            br#"{"kind":"codex_oauth","access_token":"old","refresh_token":"old-refresh","expires_at_ms":100}"#,
        )?;
        oauth.apply_refresh_response(br#"{"access_token":"new","expires_in":30}"#, 1_000)?;
        assert!(oauth.refresh_request()?.form_body().contains("old-refresh"));
        Ok(())
    }

    #[test]
    fn revisioned_refresh_rejects_stale_worker_without_mutation() -> Result<(), Box<dyn Error>> {
        let credential = OpenAiCompatibleRuntimeCredential::import(
            br#"{"kind":"codex_oauth","access_token":"old-access","refresh_token":"old-refresh","expires_at_ms":100}"#,
        )?;
        let mut revisioned = CodexOAuthRevisionedCredential::new(credential, 7)?;
        revisioned.apply_refresh_if_revision(
            7,
            br#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":30}"#,
            1_000,
        )?;
        assert_eq!(revisioned.revision(), 8);
        assert_eq!(revisioned.credential().bearer_at(30_999)?, "new-access");
        assert_eq!(
            revisioned.apply_refresh_if_revision(
                7,
                br#"{"access_token":"stale-access","expires_in":30}"#,
                2_000,
            ),
            Err(OpenAiRuntimeCredentialError::Conflict)
        );
        assert_eq!(revisioned.credential().bearer_at(30_999)?, "new-access");
        Ok(())
    }

    #[test]
    fn malformed_duplicate_and_unknown_fields_fail_closed() {
        assert!(OpenAiCompatibleRuntimeCredential::import(br#"{"kind":"codex_oauth","kind":"codex_oauth","access_token":"a","refresh_token":"r","expires_at_ms":1}"#).is_err());
        assert!(OpenAiCompatibleRuntimeCredential::import(br#"{"kind":"codex_oauth","access_token":"a","refresh_token":"r","expires_at_ms":1,"foreign":true}"#).is_err());
        assert!(OpenAiCompatibleRuntimeCredential::import(b" api-key-value").is_err());
    }

    #[test]
    fn exports_only_the_two_allowlisted_credential_shapes() -> Result<(), Box<dyn Error>> {
        let credential = OpenAiCompatibleRuntimeCredential::import(
            br#"{"kind":"codex_oauth","access_token":"access","refresh_token":"refresh","expires_at_ms":2000,"account_id":"account"}"#,
        )?;
        let cpa = credential.export_json(CodexCredentialExportFormat::Cpa)?;
        let sub2api = credential.export_json(CodexCredentialExportFormat::Sub2Api)?;
        let cpa_text = std::str::from_utf8(cpa.as_slice())?;
        let sub2api_text = std::str::from_utf8(sub2api.as_slice())?;
        assert!(cpa_text.contains("access_token"));
        assert!(sub2api_text.contains("\"credentials\""));
        assert!(sub2api_text.contains("\"type\":\"oauth\""));
        assert!(!format!("{credential:?}").contains("access\""));
        assert_eq!(
            CodexCredentialExportFormat::parse("cpa")?,
            CodexCredentialExportFormat::Cpa
        );
        assert!(CodexCredentialExportFormat::parse("unknown").is_err());
        Ok(())
    }

    #[test]
    fn cpa_and_sub2api_exports_round_trip_back_into_cpar() -> Result<(), Box<dyn Error>> {
        let original = OpenAiCompatibleRuntimeCredential::import(
            br#"{"kind":"codex_oauth","access_token":"round-access","refresh_token":"round-refresh","expires_at_ms":200000,"account_id":"round-account"}"#,
        )?;
        for format in [
            CodexCredentialExportFormat::Cpa,
            CodexCredentialExportFormat::Sub2Api,
        ] {
            let exported = original.export_json(format)?;
            let imported =
                OpenAiCompatibleRuntimeCredential::import_compatible(exported.as_slice(), 1_000)?;
            assert_eq!(imported.bearer_at(199_999)?, "round-access");
            assert!(imported.has_account_binding());
            assert_eq!(imported.account_id(), Some("round-account"));
        }
        Ok(())
    }

    #[test]
    fn metadata_is_projected_without_retaining_unknown_fields() -> Result<(), Box<dyn Error>> {
        let imported = OpenAiCompatibleRuntimeCredential::import_compatible(
            br#"{"auth_mode":"chatgpt","tokens":{"access_token":"a","refresh_token":"r","expires_in":30,"account_id":"account","email":"user@example.test"},"_meta":{"plan_type":"go","platform":"chatgpt","quota":42,"secret_like":"drop"}}"#,
            1_000,
        )?;
        let metadata = imported.metadata().ok_or("OAuth metadata missing")?;
        assert_eq!(metadata.plan.as_deref(), Some("go"));
        assert_eq!(metadata.quota.as_deref(), Some("42"));
        assert_eq!(metadata.platform.as_deref(), Some("chatgpt"));
        assert_eq!(metadata.email.as_deref(), Some("user@example.test"));
        assert_eq!(metadata.source_format.as_deref(), Some("sub2api"));
        assert!(!format!("{metadata:?}").contains("user@example.test"));
        assert!(format!("{metadata:?}").contains("[REDACTED]"));
        Ok(())
    }

    #[test]
    fn chatgpt_go_metadata_keeps_bounded_quota_summary_and_platform_label()
    -> Result<(), Box<dyn Error>> {
        let imported = OpenAiCompatibleRuntimeCredential::import_compatible(
            br#"{
                "auth_mode":"chatgpt",
                "tokens":{"access_token":"a","refresh_token":"r","expires_in":30,"account_id":"account"},
                "_meta":{
                    "plan_type":"go",
                    "used_percent":12,
                    "reset_after_secs":3600,
                    "balance":4.5,
                    "platform":{"org_id":"opaque-org","project_id":"opaque-project"},
                    "wham_raw":{"access_token":"must-not-be-promoted"}
                }
            }"#,
            1_000,
        )?;
        let metadata = imported.metadata().ok_or("OAuth metadata missing")?;
        assert_eq!(metadata.plan.as_deref(), Some("go"));
        assert_eq!(metadata.platform.as_deref(), Some("openai"));
        let quota = metadata.quota.as_deref().ok_or("quota summary missing")?;
        assert!(quota.contains("used_percent=12"));
        assert!(quota.contains("reset_after_secs=3600"));
        assert!(quota.contains("balance=4.5"));
        assert!(!format!("{metadata:?}").contains("opaque-org"));
        assert!(!format!("{metadata:?}").contains("must-not-be-promoted"));
        Ok(())
    }

    #[test]
    fn sub2api_export_matches_the_account_payload_shape_and_preserves_metadata()
    -> Result<(), Box<dyn Error>> {
        let imported = OpenAiCompatibleRuntimeCredential::import_compatible(
            br#"{"type":"oauth","name":"go@example.test","platform":"openai","credentials":{"access_token":"a","refresh_token":"r","expires_at":4102444800,"chatgpt_account_id":"account","chatgpt_user_id":"user","organization_id":"org","client_id":"client","id_token":"id"},"extra":{"email":"go@example.test","plan":"go","quota":42}}"#,
            1_000,
        )?;
        let output = imported.export_json(CodexCredentialExportFormat::Sub2Api)?;
        let value: Value = serde_json::from_slice(output.as_slice())?;
        assert_eq!(value["type"], "oauth");
        assert_eq!(value["credentials"]["chatgpt_account_id"], "account");
        assert_eq!(value["credentials"]["organization_id"], "org");
        assert_eq!(value["extra"]["plan"], "go");
        assert_eq!(value["extra"]["quota"], "42");
        Ok(())
    }
}
