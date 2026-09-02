//! P13-15B synthetic Grok Build catalog source and channel-isolation evidence.

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
use provider_grok::{
    GROK_BUILD_CLIENT_IDENTIFIER, GROK_BUILD_CLIENT_IDENTIFIER_HEADER, GROK_BUILD_CLIENT_MODE,
    GROK_BUILD_CLIENT_MODE_HEADER, GROK_BUILD_CLIENT_VERSION, GROK_BUILD_CLIENT_VERSION_HEADER,
    GROK_BUILD_MODELS_URL, GROK_BUILD_PROVIDER_ID, GROK_BUILD_TOKEN_AUTH_HEADER,
    GROK_BUILD_TOKEN_AUTH_VALUE, GROK_BUILD_USER_AGENT, GrokBuildCatalogAdapter,
    GrokBuildCatalogRequest, GrokBuildCatalogTransport, GrokBuildCatalogTransportResponse,
    GrokBuildCredential, GrokBuildModelsEndpoint, MAX_GROK_BUILD_CATALOG_RESPONSE_BYTES,
};

type TestResult = Result<(), Box<dyn Error>>;
const ACCESS: &str = "synthetic-build-catalog-access";

#[test]
fn build_catalog_request_uses_only_the_fixed_cli_profile() -> TestResult {
    let endpoint = GrokBuildModelsEndpoint::try_new()?;
    let credential = credential()?;
    let request = GrokBuildCatalogRequest::build(&endpoint, &credential);
    assert_eq!(endpoint.url(), GROK_BUILD_MODELS_URL);
    assert_eq!(request.url(), GROK_BUILD_MODELS_URL);
    assert_eq!(
        request.header("authorization"),
        Some("Bearer synthetic-build-catalog-access")
    );
    assert_eq!(
        request.header(GROK_BUILD_TOKEN_AUTH_HEADER),
        Some(GROK_BUILD_TOKEN_AUTH_VALUE)
    );
    assert_eq!(
        request.header(GROK_BUILD_CLIENT_VERSION_HEADER),
        Some(GROK_BUILD_CLIENT_VERSION)
    );
    assert_eq!(
        request.header(GROK_BUILD_CLIENT_IDENTIFIER_HEADER),
        Some(GROK_BUILD_CLIENT_IDENTIFIER)
    );
    assert_eq!(
        request.header(GROK_BUILD_CLIENT_MODE_HEADER),
        Some(GROK_BUILD_CLIENT_MODE)
    );
    assert_eq!(request.header("user-agent"), Some(GROK_BUILD_USER_AGENT));

    let admitted = policy()?.admit_url(request.url(), &StaticPublicResolver)?;
    let transport = request.into_transport_request(admitted)?;
    assert_eq!(transport.method(), UpstreamHttpMethod::Get);
    assert!(transport.body().is_empty());
    assert_eq!(
        transport
            .header("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer synthetic-build-catalog-access")
    );
    let diagnostic = format!("{endpoint:?} {transport:?} {credential:?}");
    assert!(!diagnostic.contains(ACCESS));
    assert!(!diagnostic.contains("cli-chat-proxy"));
    Ok(())
}

#[tokio::test]
async fn build_source_preserves_visible_upstream_ids_and_exact_credential_scope() -> TestResult {
    let transport = Arc::new(ScriptedTransport::new([response(
        200,
        br#"{"data":[
            {"id":"grok-4.6"},
            {"id":"grok-4.5","model":"must-not-replace-id"},
            {"modelId":"future-build-model"},
            {"_meta":{"model":"metadata-build-model"}},
            {"id":"hidden-model","hidden":true},
            {"id":"meta-hidden-model","_meta":{"hidden":true}},
            {"id":"grok-4.6"}
        ]}"#,
    )]));
    let adapter = adapter(Arc::clone(&transport))?;
    assert_eq!(adapter.provider_id().as_str(), GROK_BUILD_PROVIDER_ID);
    let models = adapter.models(target()?).await?;
    assert_eq!(
        names(&models),
        [
            "grok-4.6",
            "grok-4.5",
            "future-build-model",
            "metadata-build-model"
        ]
    );
    assert_eq!(transport.send_count(), 1);

    let wrong_channel = ModelCatalogTarget::new(
        EndpointId::try_new("grok-console-endpoint")?,
        CredentialId::try_new("build-catalog-credential")?,
    );
    let error = adapter
        .models(wrong_channel)
        .await
        .err()
        .ok_or("wrong channel accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::ClientRequestError);
    assert_eq!(error.scope(), ErrorScope::Request);
    assert_eq!(transport.send_count(), 1);
    Ok(())
}

#[tokio::test]
async fn build_source_rejects_unbounded_malformed_and_non_success_results() -> TestResult {
    for body in [
        br#"{"data":"not-an-array"}"#.as_slice(),
        br#"{"data":[{"id":""}]}"#.as_slice(),
        br#"{"data":[{"id":7}]}"#.as_slice(),
        br#"{"data":[{"id":"a","id":"b"}]}"#.as_slice(),
        br#"{"data":["model"]}"#.as_slice(),
    ] {
        let error = adapter(Arc::new(ScriptedTransport::new([response(200, body)])))?
            .models(target()?)
            .await
            .err()
            .ok_or("invalid Build catalog accepted")?;
        assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    }
    let oversized = vec![b' '; MAX_GROK_BUILD_CATALOG_RESPONSE_BYTES + 1];
    let error = adapter(Arc::new(ScriptedTransport::new([response(
        200, &oversized,
    )])))?
    .models(target()?)
    .await
    .err()
    .ok_or("oversized Build catalog accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    let error = adapter(Arc::new(ScriptedTransport::new([response(
        401, b"ignored",
    )])))?
    .models(target()?)
    .await
    .err()
    .ok_or("non-success Build catalog accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    Ok(())
}

fn credential() -> Result<GrokBuildCredential, Box<dyn Error>> {
    Ok(GrokBuildCredential::import_json(
        br#"{"access_token":"synthetic-build-catalog-access","refresh_token":"synthetic-build-catalog-refresh","expires_in":3600,"token_type":"Bearer"}"#,
        1_000,
    )?)
}

fn adapter(transport: Arc<ScriptedTransport>) -> Result<GrokBuildCatalogAdapter, Box<dyn Error>> {
    Ok(GrokBuildCatalogAdapter::try_new(
        EndpointId::try_new("build-catalog-endpoint")?,
        CredentialId::try_new("build-catalog-credential")?,
        credential()?,
        transport,
    )?)
}

fn target() -> Result<ModelCatalogTarget, Box<dyn Error>> {
    Ok(ModelCatalogTarget::new(
        EndpointId::try_new("build-catalog-endpoint")?,
        CredentialId::try_new("build-catalog-credential")?,
    ))
}

fn names(models: &[gateway_catalog::DiscoveredModel]) -> Vec<&str> {
    models
        .iter()
        .map(gateway_catalog::DiscoveredModel::upstream_model)
        .collect()
}

fn response(status: u16, body: &[u8]) -> GrokBuildCatalogTransportResponse {
    GrokBuildCatalogTransportResponse::new(status, body.to_vec())
}

struct ScriptedTransport {
    responses: Mutex<VecDeque<GrokBuildCatalogTransportResponse>>,
    sends: AtomicUsize,
}

impl ScriptedTransport {
    fn new(responses: impl IntoIterator<Item = GrokBuildCatalogTransportResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
            sends: AtomicUsize::new(0),
        }
    }

    fn send_count(&self) -> usize {
        self.sends.load(Ordering::SeqCst)
    }
}

impl GrokBuildCatalogTransport for ScriptedTransport {
    fn send(
        &self,
        _request: GrokBuildCatalogRequest,
    ) -> gateway_provider::ProviderFuture<
        '_,
        Result<GrokBuildCatalogTransportResponse, gateway_core::GatewayError>,
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
        id: EgressPolicyId::try_new("build-catalog-egress")?,
        name: "Build catalog test egress".to_owned(),
        allowed_schemes: BTreeSet::from([EgressScheme::Https]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new("cli-chat-proxy.grok.com")?]),
        allowed_ports: BTreeSet::from([443]),
        allowed_cidrs: BTreeSet::from([EgressCidr::try_new(
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            32,
        )?]),
        redirect_policy: RedirectPolicy::Deny,
    })?)
}
