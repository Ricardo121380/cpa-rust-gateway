//! Native Grok Build model discovery over the fixed CLI catalog endpoint.
//!
//! Build, Web, Console, and xAI Official are separate channel authorities. This source accepts
//! only one exact Build Endpoint/Credential target and never falls back to another Grok surface.

use std::{collections::BTreeSet, fmt, sync::Arc};

use gateway_catalog::{DiscoveredModel, ModelCatalogSource, ModelCatalogTarget};
use gateway_core::{
    CredentialId, EndpointId, ErrorScope, GatewayError, GatewayErrorCode, ProviderId,
};
use gateway_provider::{ProviderAdapter, ProviderFuture};
use gateway_upstream::{
    AdmittedEgressTarget, EgressDnsResolver, EgressPolicy, EndpointUrl, UpstreamClientPool,
    UpstreamHttpMethod, UpstreamHttpRequest, UpstreamTransportProfile,
};
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::{
    GROK_BUILD_CLIENT_IDENTIFIER, GROK_BUILD_CLIENT_IDENTIFIER_HEADER, GROK_BUILD_CLIENT_MODE,
    GROK_BUILD_CLIENT_MODE_HEADER, GROK_BUILD_CLIENT_VERSION, GROK_BUILD_CLIENT_VERSION_HEADER,
    GROK_BUILD_PROVIDER_ID, GROK_BUILD_RESPONSES_BASE_URL, GROK_BUILD_TOKEN_AUTH_HEADER,
    GROK_BUILD_TOKEN_AUTH_VALUE, GROK_BUILD_USER_AGENT, GrokBuildCredential,
    strict_json::parse_strict_json,
};

/// Fixed Build model-list path below the CLI chat-proxy base.
pub const GROK_BUILD_MODELS_PATH: &str = "/models";
/// Complete fixed Build model-list URL.
pub const GROK_BUILD_MODELS_URL: &str = "https://cli-chat-proxy.grok.com/v1/models";
/// Maximum accepted bytes for one successful Build catalog response.
pub const MAX_GROK_BUILD_CATALOG_RESPONSE_BYTES: usize = 1024 * 1024;

const MAX_GROK_BUILD_CATALOG_ENTRIES: usize = 512;
const MAX_GROK_BUILD_MODEL_ID_BYTES: usize = 512;

/// Fixed Build model-list endpoint.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokBuildModelsEndpoint {
    target: EndpointUrl,
}

impl GrokBuildModelsEndpoint {
    /// Creates the immutable CLI model-list target.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` if the fixed endpoint becomes invalid.
    pub fn try_new() -> Result<Self, GatewayError> {
        let target = EndpointUrl::compose(GROK_BUILD_RESPONSES_BASE_URL, GROK_BUILD_MODELS_PATH)
            .map_err(|_| egress_rejected_error())?;
        if target.as_str() != GROK_BUILD_MODELS_URL {
            return Err(egress_rejected_error());
        }
        Ok(Self { target })
    }

    /// Returns the fixed request URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }
}

impl fmt::Debug for GrokBuildModelsEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokBuildModelsEndpoint(<redacted>)")
    }
}

/// One request-ready Build catalog lookup.
pub struct GrokBuildCatalogRequest {
    target: EndpointUrl,
    authorization: Zeroizing<String>,
}

impl GrokBuildCatalogRequest {
    /// Builds one authenticated fixed-target request from the exact Build OAuth credential.
    #[must_use]
    pub fn build(endpoint: &GrokBuildModelsEndpoint, credential: &GrokBuildCredential) -> Self {
        Self {
            target: endpoint.target.clone(),
            authorization: Zeroizing::new(format!("Bearer {}", credential.access_token())),
        }
    }

    /// Returns the complete fixed URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }

    /// Returns one request header by case-insensitive name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("accept") {
            Some("application/json")
        } else if name.eq_ignore_ascii_case("authorization") {
            Some(self.authorization.as_str())
        } else if name.eq_ignore_ascii_case(GROK_BUILD_TOKEN_AUTH_HEADER) {
            Some(GROK_BUILD_TOKEN_AUTH_VALUE)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_CLIENT_VERSION_HEADER) {
            Some(GROK_BUILD_CLIENT_VERSION)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_CLIENT_IDENTIFIER_HEADER) {
            Some(GROK_BUILD_CLIENT_IDENTIFIER)
        } else if name.eq_ignore_ascii_case(GROK_BUILD_CLIENT_MODE_HEADER) {
            Some(GROK_BUILD_CLIENT_MODE)
        } else if name.eq_ignore_ascii_case("user-agent") {
            Some(GROK_BUILD_USER_AGENT)
        } else {
            None
        }
    }

    fn headers(&self) -> [(&'static str, &str); 7] {
        [
            ("accept", "application/json"),
            ("authorization", self.authorization.as_str()),
            (GROK_BUILD_TOKEN_AUTH_HEADER, GROK_BUILD_TOKEN_AUTH_VALUE),
            (GROK_BUILD_CLIENT_VERSION_HEADER, GROK_BUILD_CLIENT_VERSION),
            (
                GROK_BUILD_CLIENT_IDENTIFIER_HEADER,
                GROK_BUILD_CLIENT_IDENTIFIER,
            ),
            (GROK_BUILD_CLIENT_MODE_HEADER, GROK_BUILD_CLIENT_MODE),
            ("user-agent", GROK_BUILD_USER_AGENT),
        ]
    }

    /// Consumes this request into the shared DNS-pinned transport boundary.
    ///
    /// # Errors
    ///
    /// Returns a safe error if admission does not match this exact fixed target or a fixed header
    /// violates the shared transport contract.
    pub fn into_transport_request(
        self,
        admitted_target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GatewayError> {
        if admitted_target.request_url() != self.target.as_url() {
            return Err(egress_rejected_error());
        }
        UpstreamHttpRequest::try_new(
            admitted_target,
            UpstreamHttpMethod::Get,
            self.headers()
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value.to_owned())),
            Vec::new(),
        )
        .map_err(|_| internal_error())
    }
}

impl fmt::Debug for GrokBuildCatalogRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildCatalogRequest")
            .field("target", &"<redacted>")
            .field(
                "header_names",
                &[
                    "accept",
                    "authorization",
                    GROK_BUILD_TOKEN_AUTH_HEADER,
                    GROK_BUILD_CLIENT_VERSION_HEADER,
                    GROK_BUILD_CLIENT_IDENTIFIER_HEADER,
                    GROK_BUILD_CLIENT_MODE_HEADER,
                    "user-agent",
                ],
            )
            .finish_non_exhaustive()
    }
}

/// Bounded status/body handoff for one Build catalog response.
pub struct GrokBuildCatalogTransportResponse {
    status: u16,
    body: Vec<u8>,
}

impl GrokBuildCatalogTransportResponse {
    /// Creates one opaque transport response.
    #[must_use]
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self { status, body }
    }

    fn into_parts(self) -> (u16, Vec<u8>) {
        (self.status, self.body)
    }
}

impl fmt::Debug for GrokBuildCatalogTransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildCatalogTransportResponse")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Sends exactly one already-built Build catalog request.
pub trait GrokBuildCatalogTransport: Send + Sync {
    /// Performs no retry, account rotation, Web/Console fallback, or persistence.
    fn send(
        &self,
        request: GrokBuildCatalogRequest,
    ) -> ProviderFuture<'_, Result<GrokBuildCatalogTransportResponse, GatewayError>>;
}

/// Production Build catalog transport over the common DNS-pinned client.
pub struct GrokBuildUpstreamCatalogTransport {
    egress_policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    client_pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
}

impl GrokBuildUpstreamCatalogTransport {
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

impl fmt::Debug for GrokBuildUpstreamCatalogTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildUpstreamCatalogTransport")
            .field("egress_policy", self.egress_policy.id())
            .field("resolver", &"<injected>")
            .field("client_pool", &self.client_pool)
            .field("profile", &self.profile)
            .finish()
    }
}

impl GrokBuildCatalogTransport for GrokBuildUpstreamCatalogTransport {
    fn send(
        &self,
        outbound: GrokBuildCatalogRequest,
    ) -> ProviderFuture<'_, Result<GrokBuildCatalogTransportResponse, GatewayError>> {
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
            Ok(GrokBuildCatalogTransportResponse::new(status, body))
        })
    }
}

/// Build catalog source bound to one exact Endpoint/Credential identity.
#[derive(Clone)]
pub struct GrokBuildCatalogAdapter {
    provider_id: ProviderId,
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    endpoint: GrokBuildModelsEndpoint,
    credential: GrokBuildCredential,
    transport: Arc<dyn GrokBuildCatalogTransport>,
}

impl GrokBuildCatalogAdapter {
    /// Creates one isolated Build catalog source.
    ///
    /// # Errors
    ///
    /// Returns a safe internal or endpoint construction error.
    pub fn try_new(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        credential: GrokBuildCredential,
        transport: Arc<dyn GrokBuildCatalogTransport>,
    ) -> Result<Self, GatewayError> {
        let provider_id =
            ProviderId::try_new(GROK_BUILD_PROVIDER_ID).map_err(|_| internal_error())?;
        Ok(Self {
            provider_id,
            endpoint_id,
            credential_id,
            endpoint: GrokBuildModelsEndpoint::try_new()?,
            credential,
            transport,
        })
    }
}

impl fmt::Debug for GrokBuildCatalogAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildCatalogAdapter")
            .field("provider_id", &self.provider_id)
            .field("endpoint_id", &"<redacted>")
            .field("credential_id", &"<redacted>")
            .field("endpoint", &self.endpoint)
            .field("credential", &"<redacted>")
            .field("transport", &"<injected>")
            .finish()
    }
}

impl ProviderAdapter for GrokBuildCatalogAdapter {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
}

impl ModelCatalogSource for GrokBuildCatalogAdapter {
    fn models(
        &self,
        target: ModelCatalogTarget,
    ) -> ProviderFuture<'_, Result<Vec<DiscoveredModel>, GatewayError>> {
        if target.endpoint_id() != &self.endpoint_id
            || target.credential_id() != &self.credential_id
        {
            return Box::pin(async { Err(client_request_error()) });
        }
        let request = GrokBuildCatalogRequest::build(&self.endpoint, &self.credential);
        let transport = Arc::clone(&self.transport);
        Box::pin(async move {
            let response = transport.send(request).await?;
            let (status, body) = response.into_parts();
            if !(200..=299).contains(&status) {
                return Err(provider_protocol_error());
            }
            parse_catalog(&body)
        })
    }
}

fn parse_catalog(input: &[u8]) -> Result<Vec<DiscoveredModel>, GatewayError> {
    let Value::Object(root) = parse_strict_json(input, MAX_GROK_BUILD_CATALOG_RESPONSE_BYTES)
        .map_err(|()| provider_protocol_error())?
    else {
        return Err(provider_protocol_error());
    };
    let Some(Value::Array(entries)) = root.get("data") else {
        return Err(provider_protocol_error());
    };
    if entries.len() > MAX_GROK_BUILD_CATALOG_ENTRIES {
        return Err(provider_protocol_error());
    }

    let mut models = Vec::with_capacity(entries.len());
    let mut seen = BTreeSet::new();
    for entry in entries {
        let Some(model) = parse_catalog_model(entry)? else {
            continue;
        };
        if seen.insert(model.clone()) {
            models.push(model);
        }
    }
    Ok(models)
}

fn parse_catalog_model(entry: &Value) -> Result<Option<DiscoveredModel>, GatewayError> {
    let Value::Object(entry) = entry else {
        return Err(provider_protocol_error());
    };
    let metadata = match entry.get("_meta") {
        Some(Value::Object(metadata)) => Some(metadata),
        Some(_) => return Err(provider_protocol_error()),
        None => None,
    };
    if optional_bool(entry, "hidden")?.unwrap_or(false)
        || metadata
            .map(|metadata| optional_bool(metadata, "hidden"))
            .transpose()?
            .flatten()
            .unwrap_or(false)
    {
        return Ok(None);
    }
    let identifier = [
        optional_string(entry, "id")?,
        optional_string(entry, "model")?,
        optional_string(entry, "modelId")?,
        metadata
            .map(|value| optional_string(value, "model"))
            .transpose()?
            .flatten(),
        metadata
            .map(|value| optional_string(value, "modelId"))
            .transpose()?
            .flatten(),
    ]
    .into_iter()
    .flatten()
    .find(|value| !value.is_empty())
    .ok_or_else(provider_protocol_error)?;
    if identifier.len() > MAX_GROK_BUILD_MODEL_ID_BYTES
        || !identifier.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(provider_protocol_error());
    }
    DiscoveredModel::try_new(identifier.to_owned())
        .map(Some)
        .map_err(|_| provider_protocol_error())
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, GatewayError> {
    match object.get(name) {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(provider_protocol_error()),
        None => Ok(None),
    }
}

fn optional_bool(object: &Map<String, Value>, name: &str) -> Result<Option<bool>, GatewayError> {
    match object.get(name) {
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(provider_protocol_error()),
        None => Ok(None),
    }
}

async fn read_bounded_body(
    response: &mut gateway_upstream::UpstreamHttpResponse,
) -> Result<Vec<u8>, GatewayError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.next_chunk().await? {
        if body.len().saturating_add(chunk.len()) > MAX_GROK_BUILD_CATALOG_RESPONSE_BYTES {
            return Err(provider_protocol_error());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

const fn egress_rejected_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
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

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}
