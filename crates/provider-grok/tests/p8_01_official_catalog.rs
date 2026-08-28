//! P8-01 synthetic xAI Official API-key catalog boundary evidence.

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
use gateway_core::{
    CredentialId, EgressPolicyId, EndpointId, ErrorScope, GatewayError, GatewayErrorCode,
};
use gateway_provider::ProviderAdapter;
use gateway_upstream::{
    EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput,
    EgressScheme, RedirectPolicy, UpstreamHttpMethod,
};
use provider_grok::{
    GROK_OFFICIAL_MODELS_URL, GROK_OFFICIAL_PROVIDER_ID, GrokOfficialApiKey,
    GrokOfficialCatalogAdapter, GrokOfficialCatalogRequest, GrokOfficialCatalogTransport,
    GrokOfficialCatalogTransportResponse, GrokOfficialModelsEndpoint,
    MAX_GROK_OFFICIAL_CATALOG_RESPONSE_BYTES,
};

type TestResult = Result<(), Box<dyn Error>>;

const SYNTHETIC_KEY: &str = "synthetic-official-key-012345";

#[test]
fn official_models_request_is_fixed_authenticated_get_and_exact_target_only() -> TestResult {
    let endpoint = GrokOfficialModelsEndpoint::try_new()?;
    let key = GrokOfficialApiKey::try_new(SYNTHETIC_KEY)?;
    let request = GrokOfficialCatalogRequest::build(&endpoint, &key);
    let expected_authorization = format!("Bearer {SYNTHETIC_KEY}");

    assert_eq!(endpoint.url(), GROK_OFFICIAL_MODELS_URL);
    assert_eq!(request.url(), GROK_OFFICIAL_MODELS_URL);
    assert_eq!(request.header("accept"), Some("application/json"));
    assert_eq!(
        request.header("authorization"),
        Some(expected_authorization.as_str())
    );
    assert_eq!(request.header("content-type"), None);

    let admitted = policy()?.admit_url(request.url(), &StaticPublicResolver)?;
    let transport = request.into_transport_request(admitted)?;
    assert_eq!(transport.method(), UpstreamHttpMethod::Get);
    assert!(transport.body().is_empty());
    assert_eq!(
        transport
            .header("authorization")
            .and_then(|value| value.to_str().ok()),
        Some(expected_authorization.as_str())
    );

    let request = GrokOfficialCatalogRequest::build(&endpoint, &key);
    let wrong_target =
        policy()?.admit_url("https://api.x.ai/v1/responses", &StaticPublicResolver)?;
    let error = request
        .into_transport_request(wrong_target)
        .err()
        .ok_or("a different admitted endpoint unexpectedly reached the catalog transport")?;
    assert_eq!(error.code(), GatewayErrorCode::EgressRejected);
    assert_eq!(error.scope(), ErrorScope::Egress);

    let debug_request = GrokOfficialCatalogRequest::build(&endpoint, &key);
    let diagnostic = format!("{endpoint:?} {debug_request:?} {transport:?} {key:?}");
    for private_value in [SYNTHETIC_KEY, "api.x.ai", "Bearer", "https://"] {
        assert!(!diagnostic.contains(private_value));
    }
    Ok(())
}

#[tokio::test]
async fn catalog_source_is_exact_credential_scoped_and_parses_strict_fixture() -> TestResult {
    let transport = Arc::new(ScriptedCatalogTransport::new([response(
        200,
        br#"{
            "object":"list",
            "data":[
                {"id":"grok-test-fast","object":"model"},
                {"id":"grok-test-reasoning","object":"model"}
            ]
        }"#,
    )]));
    let adapter = adapter(Arc::clone(&transport))?;
    let target = catalog_target()?;

    assert_eq!(adapter.provider_id().as_str(), GROK_OFFICIAL_PROVIDER_ID);
    let models = adapter.models(target.clone()).await?;
    assert_eq!(
        model_names(&models),
        ["grok-test-fast", "grok-test-reasoning"]
    );
    assert_eq!(transport.send_count(), 1);

    let wrong_credential = ModelCatalogTarget::new(
        EndpointId::try_new("p8-official-endpoint")?,
        CredentialId::try_new("different-official-credential")?,
    );
    let error = adapter
        .models(wrong_credential)
        .await
        .err()
        .ok_or("catalog source accepted a different credential identity")?;
    assert_eq!(error.code(), GatewayErrorCode::ClientRequestError);
    assert_eq!(error.scope(), ErrorScope::Request);
    assert_eq!(transport.send_count(), 1);

    let diagnostic = format!("{adapter:?}");
    for private_value in [SYNTHETIC_KEY, "api.x.ai", "p8-official-endpoint"] {
        assert!(!diagnostic.contains(private_value));
    }
    Ok(())
}

#[tokio::test]
async fn catalog_source_rejects_non_success_and_ambiguous_or_invalid_payloads() -> TestResult {
    let invalid_payloads = [
        br#"{"data":[{"id":"grok-test"},{"id":"grok-test"}]}"#.as_slice(),
        br#"{"data":[{"id":"grok-test","id":"grok-other"}]}"#.as_slice(),
        br#"{"data":[{"id":""}]}"#.as_slice(),
        br#"{"data":[{"name":"grok-test"}]}"#.as_slice(),
        br#"{"data":"not-an-array"}"#.as_slice(),
    ];

    for payload in invalid_payloads {
        let transport = Arc::new(ScriptedCatalogTransport::new([response(200, payload)]));
        let error = adapter(transport)?
            .models(catalog_target()?)
            .await
            .err()
            .ok_or("malformed or ambiguous Official catalog payload unexpectedly succeeded")?;
        assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
        assert_eq!(error.scope(), ErrorScope::Provider);
    }

    let oversized = vec![b' '; MAX_GROK_OFFICIAL_CATALOG_RESPONSE_BYTES + 1];
    let transport = Arc::new(ScriptedCatalogTransport::new([response(200, &oversized)]));
    let error = adapter(transport)?
        .models(catalog_target()?)
        .await
        .err()
        .ok_or("oversized Official catalog payload unexpectedly succeeded")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);

    let transport = Arc::new(ScriptedCatalogTransport::new([response(401, br"ignored")]));
    let error = adapter(transport)?
        .models(catalog_target()?)
        .await
        .err()
        .ok_or("non-success Official catalog status unexpectedly succeeded")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    assert_eq!(error.scope(), ErrorScope::Provider);
    Ok(())
}

fn adapter(
    transport: Arc<ScriptedCatalogTransport>,
) -> Result<GrokOfficialCatalogAdapter, GatewayError> {
    GrokOfficialCatalogAdapter::try_new(
        EndpointId::try_new("p8-official-endpoint").map_err(|_| internal_error())?,
        CredentialId::try_new("p8-official-credential").map_err(|_| internal_error())?,
        GrokOfficialApiKey::try_new(SYNTHETIC_KEY)?,
        transport,
    )
}

fn catalog_target() -> Result<ModelCatalogTarget, gateway_core::InvalidIdentifier> {
    Ok(ModelCatalogTarget::new(
        EndpointId::try_new("p8-official-endpoint")?,
        CredentialId::try_new("p8-official-credential")?,
    ))
}

fn model_names(models: &[gateway_catalog::DiscoveredModel]) -> Vec<&str> {
    models
        .iter()
        .map(gateway_catalog::DiscoveredModel::upstream_model)
        .collect()
}

fn response(status: u16, body: &[u8]) -> GrokOfficialCatalogTransportResponse {
    GrokOfficialCatalogTransportResponse::new(status, body.to_vec())
}

struct ScriptedCatalogTransport {
    responses: Mutex<VecDeque<GrokOfficialCatalogTransportResponse>>,
    sends: AtomicUsize,
}

impl ScriptedCatalogTransport {
    fn new(responses: impl IntoIterator<Item = GrokOfficialCatalogTransportResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
            sends: AtomicUsize::new(0),
        }
    }

    fn send_count(&self) -> usize {
        self.sends.load(Ordering::SeqCst)
    }
}

impl GrokOfficialCatalogTransport for ScriptedCatalogTransport {
    fn send(
        &self,
        _request: GrokOfficialCatalogRequest,
    ) -> gateway_provider::ProviderFuture<
        '_,
        Result<GrokOfficialCatalogTransportResponse, GatewayError>,
    > {
        self.sends.fetch_add(1, Ordering::SeqCst);
        let response = self
            .responses
            .lock()
            .map_err(|_| internal_error())
            .and_then(|mut responses| responses.pop_front().ok_or_else(internal_error));
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
        id: EgressPolicyId::try_new("p8-official-egress")?,
        name: "P8 Official test policy".to_owned(),
        allowed_schemes: BTreeSet::from([EgressScheme::Https]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new("api.x.ai")?]),
        allowed_ports: BTreeSet::from([443]),
        allowed_cidrs: BTreeSet::from([EgressCidr::try_new(
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            32,
        )?]),
        redirect_policy: RedirectPolicy::Deny,
    })?)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}
