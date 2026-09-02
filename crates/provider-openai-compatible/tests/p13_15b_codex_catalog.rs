//! P13-15B synthetic Codex catalog authority and exact credential-scope evidence.

use std::{
    collections::{BTreeSet, VecDeque},
    error::Error,
    net::{IpAddr, Ipv4Addr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use gateway_catalog::{ModelCatalogSource, ModelCatalogTarget};
use gateway_core::{CredentialId, EgressPolicyId, EndpointId, ErrorScope, GatewayErrorCode};
use gateway_provider::ProviderAdapter;
use gateway_upstream::{
    EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput,
    EgressScheme, RedirectPolicy, UpstreamHttpMethod,
};
use provider_openai_compatible::{
    CODEX_CATALOG_PROVIDER_ID, CODEX_MODELS_URL, CODEX_USER_AGENT, CodexCatalogAdapter,
    CodexCatalogCredential, CodexCatalogRequest, CodexCatalogTransport,
    CodexCatalogTransportResponse, MAX_CODEX_CATALOG_RESPONSE_BYTES,
    OpenAiCompatibleRuntimeCredential,
};

type TestResult = Result<(), Box<dyn Error>>;
const ACCESS: &str = "synthetic-codex-catalog-access";
const ACCOUNT: &str = "synthetic-codex-account";

#[test]
fn codex_catalog_request_uses_the_exact_oauth_account_profile() -> TestResult {
    let runtime = runtime_credential()?;
    let credential = CodexCatalogCredential::try_from_runtime(&runtime, 1_000)?;
    let request = CodexCatalogRequest::build(&credential);
    assert_eq!(request.url(), CODEX_MODELS_URL);
    assert_eq!(
        request.header("authorization"),
        Some("Bearer synthetic-codex-catalog-access")
    );
    assert_eq!(request.header("chatgpt-account-id"), Some(ACCOUNT));
    assert_eq!(request.header("user-agent"), Some(CODEX_USER_AGENT));

    let admitted = policy()?.admit_url(request.url(), &StaticPublicResolver)?;
    let transport = request.into_transport_request(admitted)?;
    assert_eq!(transport.method(), UpstreamHttpMethod::Get);
    assert!(transport.body().is_empty());
    assert_eq!(
        transport
            .header("chatgpt-account-id")
            .and_then(|value| value.to_str().ok()),
        Some(ACCOUNT)
    );
    let diagnostic = format!("{transport:?} {credential:?}");
    assert!(!diagnostic.contains(ACCESS));
    assert!(!diagnostic.contains(ACCOUNT));
    assert!(!diagnostic.contains("chatgpt.com"));
    Ok(())
}

#[tokio::test]
async fn codex_source_uses_visible_api_entries_without_a_local_tier_table() -> TestResult {
    let transport = Arc::new(ScriptedTransport::new([response(
        200,
        br#"{"models":[
            {"slug":"gpt-5.6-terra","visibility":"list","supported_in_api":true,"available_in_plans":["free","go","plus","pro"]},
            {"slug":"gpt-5.6-luna","visibility":"list","supported_in_api":true,"available_in_plans":["free","go","plus","pro"]},
            {"slug":"gpt-reserve","visibility":"hide","supported_in_api":true},
            {"slug":"gpt-5.5","visibility":"list","supported_in_api":true},
            {"slug":"not-an-api-model","visibility":"list","supported_in_api":false},
            {"slug":"gpt-5.6-luna","visibility":"list","supported_in_api":true}
        ]}"#,
    )]));
    let adapter = adapter(Arc::clone(&transport))?;
    assert_eq!(adapter.provider_id().as_str(), CODEX_CATALOG_PROVIDER_ID);
    let models = adapter.models(target()?).await?;
    assert_eq!(names(&models), ["gpt-5.6-terra", "gpt-5.6-luna", "gpt-5.5"]);
    assert_eq!(transport.send_count(), 1);

    let wrong_credential = ModelCatalogTarget::new(
        EndpointId::try_new("codex-catalog-endpoint")?,
        CredentialId::try_new("different-codex-credential")?,
    );
    let error = adapter
        .models(wrong_credential)
        .await
        .err()
        .ok_or("wrong credential accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::ClientRequestError);
    assert_eq!(error.scope(), ErrorScope::Request);
    assert_eq!(transport.send_count(), 1);
    Ok(())
}

#[tokio::test]
async fn codex_source_rejects_malformed_oversized_and_non_success_results() -> TestResult {
    for body in [
        br#"{"models":"not-an-array"}"#.as_slice(),
        br#"{"models":[{"slug":"","visibility":"list","supported_in_api":true}]}"#.as_slice(),
        br#"{"models":[{"slug":7,"visibility":"list","supported_in_api":true}]}"#.as_slice(),
        br#"{"models":[{"slug":"a","visibility":"list"}]}"#.as_slice(),
        br#"{"models":[{"slug":"a","slug":"b","visibility":"list","supported_in_api":true}]}"#
            .as_slice(),
        br#"{"models":["gpt-5.6-luna"]}"#.as_slice(),
    ] {
        let error = adapter(Arc::new(ScriptedTransport::new([response(200, body)])))?
            .models(target()?)
            .await
            .err()
            .ok_or("invalid Codex catalog accepted")?;
        assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    }
    let oversized = vec![b' '; MAX_CODEX_CATALOG_RESPONSE_BYTES + 1];
    let error = adapter(Arc::new(ScriptedTransport::new([response(
        200, &oversized,
    )])))?
    .models(target()?)
    .await
    .err()
    .ok_or("oversized Codex catalog accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    for (status, expected) in [
        (401, GatewayErrorCode::CredentialUnauthorized),
        (403, GatewayErrorCode::CredentialForbidden),
        (429, GatewayErrorCode::ProviderRateLimited),
        (500, GatewayErrorCode::ProviderTransient),
        (404, GatewayErrorCode::ProviderPermanent),
    ] {
        let error = adapter(Arc::new(ScriptedTransport::new([response(
            status, b"ignored",
        )])))?
        .models(target()?)
        .await
        .err()
        .ok_or("non-success Codex catalog accepted")?;
        assert_eq!(error.code(), expected);
    }
    Ok(())
}

fn runtime_credential() -> Result<OpenAiCompatibleRuntimeCredential, Box<dyn Error>> {
    Ok(OpenAiCompatibleRuntimeCredential::import(
        br#"{"kind":"codex_oauth","access_token":"synthetic-codex-catalog-access","refresh_token":"synthetic-codex-catalog-refresh","expires_at_ms":100000,"account_id":"synthetic-codex-account"}"#,
    )?)
}

fn adapter(transport: Arc<ScriptedTransport>) -> Result<CodexCatalogAdapter, Box<dyn Error>> {
    Ok(CodexCatalogAdapter::try_new(
        EndpointId::try_new("codex-catalog-endpoint")?,
        CredentialId::try_new("codex-catalog-credential")?,
        CodexCatalogCredential::try_from_runtime(&runtime_credential()?, 1_000)?,
        transport,
    )?)
}

fn target() -> Result<ModelCatalogTarget, Box<dyn Error>> {
    Ok(ModelCatalogTarget::new(
        EndpointId::try_new("codex-catalog-endpoint")?,
        CredentialId::try_new("codex-catalog-credential")?,
    ))
}

fn names(models: &[gateway_catalog::DiscoveredModel]) -> Vec<&str> {
    models
        .iter()
        .map(gateway_catalog::DiscoveredModel::upstream_model)
        .collect()
}

fn response(status: u16, body: &[u8]) -> CodexCatalogTransportResponse {
    CodexCatalogTransportResponse::new(status, body.to_vec())
}

struct ScriptedTransport {
    responses: Mutex<VecDeque<CodexCatalogTransportResponse>>,
    sends: AtomicUsize,
}

impl ScriptedTransport {
    fn new(responses: impl IntoIterator<Item = CodexCatalogTransportResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
            sends: AtomicUsize::new(0),
        }
    }

    fn send_count(&self) -> usize {
        self.sends.load(Ordering::SeqCst)
    }
}

impl CodexCatalogTransport for ScriptedTransport {
    fn send(
        &self,
        _request: CodexCatalogRequest,
    ) -> gateway_provider::ProviderFuture<
        '_,
        Result<CodexCatalogTransportResponse, gateway_core::GatewayError>,
    > {
        self.sends.fetch_add(1, Ordering::SeqCst);
        let response = self
            .responses
            .lock()
            .map_err(|_| {
                gateway_core::GatewayError::new(
                    GatewayErrorCode::InternalError,
                    ErrorScope::Internal,
                )
            })
            .and_then(|mut values| {
                values.pop_front().ok_or_else(|| {
                    gateway_core::GatewayError::new(
                        GatewayErrorCode::InternalError,
                        ErrorScope::Internal,
                    )
                })
            });
        Box::pin(async move { response })
    }
}

#[derive(Clone, Copy)]
struct StaticPublicResolver;

impl EgressDnsResolver for StaticPublicResolver {
    fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
    }
}

fn policy() -> Result<EgressPolicy, Box<dyn Error>> {
    Ok(EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("codex-catalog-egress")?,
        name: "Codex catalog test egress".to_owned(),
        allowed_schemes: BTreeSet::from([EgressScheme::Https]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new("chatgpt.com")?]),
        allowed_ports: BTreeSet::from([443]),
        allowed_cidrs: BTreeSet::from([EgressCidr::try_new(
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            32,
        )?]),
        redirect_policy: RedirectPolicy::Deny,
    })?)
}
