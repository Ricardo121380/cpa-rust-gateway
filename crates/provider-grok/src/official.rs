//! Native xAI Official API-key model catalog boundary.
//!
//! P8-01 deliberately implements only the fixed Official `GET /v1/models` discovery path. It
//! keeps xAI API-key material, endpoint, request, catalog transport, and provider identity apart
//! from Grok Build OAuth and Grok Web state. P8-02 owns Official Responses inference and SSE.

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
use serde_json::Value;
use zeroize::Zeroizing;

use crate::strict_json::parse_strict_json;

/// xAI's fixed public REST API base for the Official provider.
pub const GROK_OFFICIAL_API_BASE_URL: &str = "https://api.x.ai/v1";
/// Fixed Official model discovery path.
pub const GROK_OFFICIAL_MODELS_PATH: &str = "/models";
/// Full fixed Official model discovery URL.
pub const GROK_OFFICIAL_MODELS_URL: &str = "https://api.x.ai/v1/models";
/// Stable Provider ID for the Official xAI API-key boundary.
pub const GROK_OFFICIAL_PROVIDER_ID: &str = "grok.official";
/// Maximum accepted bytes for a successful Official model-list payload.
pub const MAX_GROK_OFFICIAL_CATALOG_RESPONSE_BYTES: usize = 1024 * 1024;

const MAX_GROK_OFFICIAL_MODEL_ID_BYTES: usize = 256;

/// One short-lived xAI Official API key suitable only for request-scoped Bearer construction.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokOfficialApiKey(Zeroizing<String>);

impl GrokOfficialApiKey {
    /// Creates one non-empty visible-ASCII API key.
    ///
    /// A prefix is deliberately not required: provider-issued API-key formats may evolve, while
    /// headers must never accept whitespace or control bytes.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` before an invalid Authorization value can reach
    /// a transport.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GatewayError> {
        let value = value.into();
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(credential_unavailable_error());
        }
        Ok(Self(Zeroizing::new(value)))
    }

    /// Returns the credential only for constructing one request-scoped Authorization header.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for GrokOfficialApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokOfficialApiKey(<redacted>)")
    }
}

/// The fixed Official xAI model catalog endpoint.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokOfficialModelsEndpoint {
    target: EndpointUrl,
}

impl GrokOfficialModelsEndpoint {
    /// Creates the fixed Official `GET /v1/models` target.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` if a future edit makes the immutable endpoint invalid.
    pub fn try_new() -> Result<Self, GatewayError> {
        let target = EndpointUrl::compose(GROK_OFFICIAL_API_BASE_URL, GROK_OFFICIAL_MODELS_PATH)
            .map_err(|_| egress_rejected_error())?;
        Ok(Self { target })
    }

    /// Returns the complete model-catalog URL for a later admitted transport.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }
}

impl fmt::Debug for GrokOfficialModelsEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokOfficialModelsEndpoint(<redacted>)")
    }
}

/// One request-ready Official model discovery operation.
///
/// The key remains request-scoped and zeroizing. The request has no body and uses exactly the
/// fixed Official catalog target; P8-02 owns the distinct Responses request profile.
#[derive(Eq, PartialEq)]
pub struct GrokOfficialCatalogRequest {
    target: EndpointUrl,
    authorization: Zeroizing<String>,
}

impl GrokOfficialCatalogRequest {
    /// Builds the fixed authenticated `GET /v1/models` request.
    #[must_use]
    pub fn build(endpoint: &GrokOfficialModelsEndpoint, api_key: &GrokOfficialApiKey) -> Self {
        Self {
            target: endpoint.target.clone(),
            authorization: Zeroizing::new(format!("Bearer {}", api_key.as_str())),
        }
    }

    /// Returns the complete fixed endpoint URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }

    /// Returns one standard catalog header by case-insensitive name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("accept") {
            Some("application/json")
        } else if name.eq_ignore_ascii_case("authorization") {
            Some(self.authorization.as_str())
        } else {
            None
        }
    }

    /// Returns headers in deterministic transport order.
    #[must_use]
    pub fn headers(&self) -> [(&'static str, &str); 2] {
        [
            ("accept", "application/json"),
            ("authorization", self.authorization.as_str()),
        ]
    }

    /// Consumes this catalog request into one DNS-pinned shared-transport request.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` if admission was for any target other than the exact
    /// Official models URL, or `InternalError/Internal` for a shared transport invariant failure.
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
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect::<Vec<_>>(),
            Vec::new(),
        )
        .map_err(|_| internal_error())
    }
}

impl fmt::Debug for GrokOfficialCatalogRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialCatalogRequest")
            .field("target", &"<redacted>")
            .field("header_names", &["accept", "authorization"])
            .finish_non_exhaustive()
    }
}

/// Bounded status and body handoff for one Official model-catalog response.
pub struct GrokOfficialCatalogTransportResponse {
    status: u16,
    body: Vec<u8>,
}

impl GrokOfficialCatalogTransportResponse {
    /// Constructs an opaque catalog response handoff for injected test or production transport.
    #[must_use]
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self { status, body }
    }

    fn into_parts(self) -> (u16, Vec<u8>) {
        (self.status, self.body)
    }
}

impl fmt::Debug for GrokOfficialCatalogTransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialCatalogTransportResponse")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Sends one already-built Official catalog request through a caller-controlled boundary.
pub trait GrokOfficialCatalogTransport: Send + Sync {
    /// Sends exactly one request without retries, failover, key rotation, or scheduling.
    fn send(
        &self,
        request: GrokOfficialCatalogRequest,
    ) -> ProviderFuture<'_, Result<GrokOfficialCatalogTransportResponse, GatewayError>>;
}

/// Production Official catalog transport using only the shared DNS-pinned client after admission.
pub struct GrokOfficialUpstreamCatalogTransport {
    egress_policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    client_pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
}

impl GrokOfficialUpstreamCatalogTransport {
    /// Creates a production catalog transport from explicit egress and HTTP components.
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

impl fmt::Debug for GrokOfficialUpstreamCatalogTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialUpstreamCatalogTransport")
            .field("egress_policy", self.egress_policy.id())
            .field("resolver", &"<injected>")
            .field("client_pool", &self.client_pool)
            .field("profile", &self.profile)
            .finish()
    }
}

impl GrokOfficialCatalogTransport for GrokOfficialUpstreamCatalogTransport {
    fn send(
        &self,
        outbound: GrokOfficialCatalogRequest,
    ) -> ProviderFuture<'_, Result<GrokOfficialCatalogTransportResponse, GatewayError>> {
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
            Ok(GrokOfficialCatalogTransportResponse::new(status, body))
        })
    }
}

/// Native Official catalog source bound to one exact Endpoint/Credential identity.
#[derive(Clone)]
pub struct GrokOfficialCatalogAdapter {
    provider_id: ProviderId,
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    endpoint: GrokOfficialModelsEndpoint,
    credential: GrokOfficialApiKey,
    transport: Arc<dyn GrokOfficialCatalogTransport>,
}

impl GrokOfficialCatalogAdapter {
    /// Builds one Official model-catalog source for one selected endpoint and API key.
    ///
    /// The API key is deliberately not interchangeable with Build OAuth or Web SSO credentials.
    /// This boundary neither fetches storage nor refreshes, retries, schedules, or fails over.
    ///
    /// # Errors
    ///
    /// Returns `InternalError/Internal` only if the compiled stable Provider ID becomes invalid.
    pub fn try_new(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        credential: GrokOfficialApiKey,
        transport: Arc<dyn GrokOfficialCatalogTransport>,
    ) -> Result<Self, GatewayError> {
        let provider_id = ProviderId::try_new(GROK_OFFICIAL_PROVIDER_ID.to_owned())
            .map_err(|_| internal_error())?;
        Ok(Self {
            provider_id,
            endpoint_id,
            credential_id,
            endpoint: GrokOfficialModelsEndpoint::try_new()?,
            credential,
            transport,
        })
    }
}

impl fmt::Debug for GrokOfficialCatalogAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialCatalogAdapter")
            .field("provider_id", &self.provider_id)
            .field("endpoint_id", &"<redacted>")
            .field("credential_id", &"<redacted>")
            .field("endpoint", &self.endpoint)
            .field("credential", &self.credential)
            .field("transport", &"<injected>")
            .finish()
    }
}

impl ProviderAdapter for GrokOfficialCatalogAdapter {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
}

impl ModelCatalogSource for GrokOfficialCatalogAdapter {
    fn models(
        &self,
        target: ModelCatalogTarget,
    ) -> ProviderFuture<'_, Result<Vec<DiscoveredModel>, GatewayError>> {
        if target.endpoint_id() != &self.endpoint_id
            || target.credential_id() != &self.credential_id
        {
            return Box::pin(async { Err(client_request_error()) });
        }

        let request = GrokOfficialCatalogRequest::build(&self.endpoint, &self.credential);
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
    let Value::Object(root) = parse_strict_json(input, MAX_GROK_OFFICIAL_CATALOG_RESPONSE_BYTES)
        .map_err(|()| provider_protocol_error())?
    else {
        return Err(provider_protocol_error());
    };
    let Some(Value::Array(entries)) = root.get("data") else {
        return Err(provider_protocol_error());
    };

    let mut models = Vec::with_capacity(entries.len());
    let mut seen = BTreeSet::new();
    for entry in entries {
        let model = parse_catalog_model(entry)?;
        if !seen.insert(model.clone()) {
            return Err(provider_protocol_error());
        }
        models.push(model);
    }
    Ok(models)
}

fn parse_catalog_model(entry: &Value) -> Result<DiscoveredModel, GatewayError> {
    let Value::Object(entry) = entry else {
        return Err(provider_protocol_error());
    };
    let Some(Value::String(model)) = entry.get("id") else {
        return Err(provider_protocol_error());
    };
    if model.is_empty()
        || model.len() > MAX_GROK_OFFICIAL_MODEL_ID_BYTES
        || !model.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(provider_protocol_error());
    }
    DiscoveredModel::try_new(model.clone()).map_err(|_| provider_protocol_error())
}

async fn read_bounded_body(
    response: &mut gateway_upstream::UpstreamHttpResponse,
) -> Result<Vec<u8>, GatewayError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.next_chunk().await? {
        let next_length = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(provider_protocol_error)?;
        if next_length > MAX_GROK_OFFICIAL_CATALOG_RESPONSE_BYTES {
            return Err(provider_protocol_error());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
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

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}
