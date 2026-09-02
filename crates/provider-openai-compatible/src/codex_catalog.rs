//! Credential-scoped Codex model discovery over the official `ChatGPT` backend catalog.
//!
//! The server response, not a local plan-to-model table, is authoritative. Hidden or non-API
//! entries are deliberately excluded from the client-visible result.

use std::{collections::BTreeSet, fmt, sync::Arc};

use gateway_catalog::{DiscoveredModel, ModelCatalogSource, ModelCatalogTarget};
use gateway_core::{
    CredentialId, EndpointId, ErrorScope, GatewayError, GatewayErrorCode, ProviderId,
};
use gateway_provider::{ProviderAdapter, ProviderFuture};
use gateway_upstream::{
    AdmittedEgressTarget, EgressDnsResolver, EgressPolicy, UpstreamClientPool, UpstreamHttpMethod,
    UpstreamHttpRequest, UpstreamTransportProfile,
};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::{OpenAiCompatibleRuntimeCredential, runtime_credential::reject_duplicate_json_names};

/// Stable Provider identity for the official Codex OAuth channel.
pub const CODEX_CATALOG_PROVIDER_ID: &str = "openai.codex";
/// Exact Responses base URL used by the official Codex channel.
pub const CODEX_RESPONSES_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
/// Exact inference operation below the Codex base.
pub const CODEX_RESPONSES_PATH: &str = "/responses";
/// Current compatible Codex catalog client version.
pub const CODEX_CATALOG_CLIENT_VERSION: &str = "0.144.1";
/// Official client originator header.
pub const CODEX_ORIGINATOR: &str = "codex_cli_rs";
/// Client profile shared by inference and discovery.
pub const CODEX_USER_AGENT: &str = "codex_cli_rs/0.144.1 (Linux; arm64)";
/// Exact credential-scoped model-list URL.
pub const CODEX_MODELS_URL: &str =
    "https://chatgpt.com/backend-api/codex/models?client_version=0.144.1";
/// Maximum accepted bytes for one successful Codex catalog response.
pub const MAX_CODEX_CATALOG_RESPONSE_BYTES: usize = 1024 * 1024;

const MAX_CODEX_CATALOG_ENTRIES: usize = 512;
const MAX_CODEX_MODEL_ID_BYTES: usize = 512;

/// Request-only OAuth material for one Codex catalog lookup.
pub struct CodexCatalogCredential {
    bearer: Zeroizing<String>,
    account_id: Zeroizing<String>,
}

impl CodexCatalogCredential {
    /// Copies the minimum request material from one currently usable Codex OAuth credential.
    ///
    /// # Errors
    ///
    /// Returns a safe credential error for API keys, expired OAuth, missing account binding, or
    /// values that cannot be represented as HTTP header content.
    pub fn try_from_runtime(
        credential: &OpenAiCompatibleRuntimeCredential,
        observed_at_ms: i64,
    ) -> Result<Self, GatewayError> {
        let bearer = credential.bearer_at(observed_at_ms)?;
        let account_id = credential
            .account_id()
            .ok_or_else(credential_unavailable_error)?;
        if !header_value_is_admissible(bearer) || !header_value_is_admissible(account_id) {
            return Err(credential_unavailable_error());
        }
        Ok(Self {
            bearer: Zeroizing::new(bearer.to_owned()),
            account_id: Zeroizing::new(account_id.to_owned()),
        })
    }
}

impl fmt::Debug for CodexCatalogCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CodexCatalogCredential(<redacted>)")
    }
}

/// One request-ready Codex catalog lookup.
pub struct CodexCatalogRequest {
    target: &'static str,
    authorization: Zeroizing<String>,
    account_id: Zeroizing<String>,
}

impl CodexCatalogRequest {
    /// Builds the fixed catalog request from one exact credential.
    #[must_use]
    pub fn build(credential: &CodexCatalogCredential) -> Self {
        Self {
            target: CODEX_MODELS_URL,
            authorization: Zeroizing::new(format!("Bearer {}", credential.bearer.as_str())),
            account_id: Zeroizing::new(credential.account_id.to_string()),
        }
    }

    /// Returns the exact request URL.
    #[must_use]
    pub const fn url(&self) -> &'static str {
        self.target
    }

    /// Returns one fixed request header by case-insensitive name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("accept") {
            Some("application/json")
        } else if name.eq_ignore_ascii_case("authorization") {
            Some(self.authorization.as_str())
        } else if name.eq_ignore_ascii_case("chatgpt-account-id") {
            Some(self.account_id.as_str())
        } else if name.eq_ignore_ascii_case("user-agent") {
            Some(CODEX_USER_AGENT)
        } else {
            None
        }
    }

    /// Consumes this request into the shared DNS-pinned transport boundary.
    ///
    /// # Errors
    ///
    /// Returns a safe error if admission does not preserve the exact fixed URL or a fixed header
    /// cannot be represented by the common transport.
    pub fn into_transport_request(
        self,
        admitted_target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GatewayError> {
        if admitted_target.request_url().as_str() != CODEX_MODELS_URL {
            return Err(egress_rejected_error());
        }
        UpstreamHttpRequest::try_new(
            admitted_target,
            UpstreamHttpMethod::Get,
            [
                ("accept".to_owned(), "application/json".to_owned()),
                ("authorization".to_owned(), self.authorization.to_string()),
                ("chatgpt-account-id".to_owned(), self.account_id.to_string()),
                ("user-agent".to_owned(), CODEX_USER_AGENT.to_owned()),
            ],
            Vec::new(),
        )
        .map_err(|_| internal_error())
    }
}

impl fmt::Debug for CodexCatalogRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexCatalogRequest")
            .field("target", &"<redacted>")
            .field(
                "header_names",
                &[
                    "accept",
                    "authorization",
                    "chatgpt-account-id",
                    "user-agent",
                ],
            )
            .finish()
    }
}

/// Bounded status/body handoff for one Codex catalog response.
pub struct CodexCatalogTransportResponse {
    status: u16,
    body: Vec<u8>,
}

impl CodexCatalogTransportResponse {
    /// Creates one opaque transport response.
    #[must_use]
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self { status, body }
    }

    fn into_parts(self) -> (u16, Vec<u8>) {
        (self.status, self.body)
    }
}

impl fmt::Debug for CodexCatalogTransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexCatalogTransportResponse")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Sends exactly one already-built Codex catalog request.
pub trait CodexCatalogTransport: Send + Sync {
    /// Performs no retry, credential rotation, persistence, or generic-endpoint fallback.
    fn send(
        &self,
        request: CodexCatalogRequest,
    ) -> ProviderFuture<'_, Result<CodexCatalogTransportResponse, GatewayError>>;
}

/// Production transport over the common DNS-pinned client.
pub struct CodexUpstreamCatalogTransport {
    egress_policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    client_pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
}

impl CodexUpstreamCatalogTransport {
    /// Creates one transport from explicit egress and HTTP components.
    #[must_use]
    pub fn new(
        egress_policy: EgressPolicy,
        resolver: Arc<dyn EgressDnsResolver>,
        client_pool: UpstreamClientPool,
        profile: UpstreamTransportProfile,
    ) -> Self {
        Self {
            egress_policy,
            resolver,
            client_pool,
            profile,
        }
    }
}

impl fmt::Debug for CodexUpstreamCatalogTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexUpstreamCatalogTransport")
            .field("egress_policy", self.egress_policy.id())
            .field("resolver", &"<injected>")
            .field("client_pool", &self.client_pool)
            .field("profile", &self.profile)
            .finish()
    }
}

impl CodexCatalogTransport for CodexUpstreamCatalogTransport {
    fn send(
        &self,
        outbound: CodexCatalogRequest,
    ) -> ProviderFuture<'_, Result<CodexCatalogTransportResponse, GatewayError>> {
        let admitted = self
            .egress_policy
            .admit_url(outbound.url(), self.resolver.as_ref())
            .map_err(gateway_upstream::EgressAdmissionError::gateway_error);
        let request = admitted.and_then(|target| outbound.into_transport_request(target));
        let pool = self.client_pool.clone();
        let profile = self.profile.clone();
        Box::pin(async move {
            let mut response = pool.send(request?, &profile).await?;
            let status = response.status();
            let body = if (200..=299).contains(&status) {
                read_bounded_body(&mut response).await?
            } else {
                Vec::new()
            };
            Ok(CodexCatalogTransportResponse::new(status, body))
        })
    }
}

/// Codex catalog source bound to one exact Endpoint/Credential identity.
pub struct CodexCatalogAdapter {
    provider_id: ProviderId,
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    credential: CodexCatalogCredential,
    transport: Arc<dyn CodexCatalogTransport>,
}

impl CodexCatalogAdapter {
    /// Creates one isolated official Codex catalog source.
    ///
    /// # Errors
    ///
    /// Returns a safe internal error if the fixed Provider identity becomes invalid.
    pub fn try_new(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        credential: CodexCatalogCredential,
        transport: Arc<dyn CodexCatalogTransport>,
    ) -> Result<Self, GatewayError> {
        Ok(Self {
            provider_id: ProviderId::try_new(CODEX_CATALOG_PROVIDER_ID)
                .map_err(|_| internal_error())?,
            endpoint_id,
            credential_id,
            credential,
            transport,
        })
    }
}

impl fmt::Debug for CodexCatalogAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexCatalogAdapter")
            .field("provider_id", &self.provider_id)
            .field("endpoint_id", &"<redacted>")
            .field("credential_id", &"<redacted>")
            .field("credential", &self.credential)
            .field("transport", &"<injected>")
            .finish()
    }
}

impl ProviderAdapter for CodexCatalogAdapter {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
}

impl ModelCatalogSource for CodexCatalogAdapter {
    fn models(
        &self,
        target: ModelCatalogTarget,
    ) -> ProviderFuture<'_, Result<Vec<DiscoveredModel>, GatewayError>> {
        if target.endpoint_id() != &self.endpoint_id
            || target.credential_id() != &self.credential_id
        {
            return Box::pin(async { Err(client_request_error()) });
        }
        let request = CodexCatalogRequest::build(&self.credential);
        let transport = Arc::clone(&self.transport);
        Box::pin(async move {
            let response = transport.send(request).await?;
            let (status, body) = response.into_parts();
            if !(200..=299).contains(&status) {
                return Err(catalog_http_error(status));
            }
            parse_catalog(&body)
        })
    }
}

fn parse_catalog(input: &[u8]) -> Result<Vec<DiscoveredModel>, GatewayError> {
    if input.len() > MAX_CODEX_CATALOG_RESPONSE_BYTES || reject_duplicate_json_names(input).is_err()
    {
        return Err(provider_protocol_error());
    }
    let Value::Object(root) =
        serde_json::from_slice(input).map_err(|_| provider_protocol_error())?
    else {
        return Err(provider_protocol_error());
    };
    let Some(Value::Array(entries)) = root.get("models") else {
        return Err(provider_protocol_error());
    };
    if entries.len() > MAX_CODEX_CATALOG_ENTRIES {
        return Err(provider_protocol_error());
    }

    let mut models = Vec::with_capacity(entries.len());
    let mut seen = BTreeSet::new();
    for entry in entries {
        let Value::Object(entry) = entry else {
            return Err(provider_protocol_error());
        };
        let slug = entry
            .get("slug")
            .and_then(Value::as_str)
            .ok_or_else(provider_protocol_error)?;
        let visibility = entry
            .get("visibility")
            .and_then(Value::as_str)
            .ok_or_else(provider_protocol_error)?;
        let supported_in_api = entry
            .get("supported_in_api")
            .and_then(Value::as_bool)
            .ok_or_else(provider_protocol_error)?;
        if visibility != "list" || !supported_in_api {
            continue;
        }
        if slug.is_empty()
            || slug.len() > MAX_CODEX_MODEL_ID_BYTES
            || !slug.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(provider_protocol_error());
        }
        if seen.insert(slug.to_owned()) {
            models.push(
                DiscoveredModel::try_new(slug.to_owned()).map_err(|_| provider_protocol_error())?,
            );
        }
    }
    Ok(models)
}

async fn read_bounded_body(
    response: &mut gateway_upstream::UpstreamHttpResponse,
) -> Result<Vec<u8>, GatewayError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.next_chunk().await? {
        if body.len().saturating_add(chunk.len()) > MAX_CODEX_CATALOG_RESPONSE_BYTES {
            return Err(provider_protocol_error());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn header_value_is_admissible(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_graphic())
}

const fn egress_rejected_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
}

const fn credential_unavailable_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::CredentialUnavailable,
        ErrorScope::Credential,
    )
}

const fn client_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

const fn provider_protocol_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        ErrorScope::Provider,
    )
}

const fn catalog_http_error(status: u16) -> GatewayError {
    match status {
        401 => GatewayError::new(
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
        ),
        403 => GatewayError::new(
            GatewayErrorCode::CredentialForbidden,
            ErrorScope::Credential,
        ),
        429 => GatewayError::new(
            GatewayErrorCode::ProviderRateLimited,
            ErrorScope::QuotaWindow,
        ),
        408 | 500..=599 => {
            GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider)
        }
        _ => GatewayError::new(GatewayErrorCode::ProviderPermanent, ErrorScope::Provider),
    }
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}
