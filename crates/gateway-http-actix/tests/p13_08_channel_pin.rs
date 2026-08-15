//! P13-08 protected Channel Pin management contract tests.

#![deny(unsafe_code)]

use std::{
    error::Error,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use actix_web::{
    App,
    http::{StatusCode, header},
    test, web,
};
use gateway_control::management_mutation_service::{
    ConfigRevision, ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
    CredentialConfiguration, EndpointConfiguration, EndpointCredentialBindingConfiguration,
    EndpointTransport, KeyVersion, ManagementMutationService, MasterKey, MasterKeyRing,
    SecretStore, SqliteControlPlaneRepository,
};
use gateway_core::{CredentialId, EndpointId, RouteId, UpstreamId};
use gateway_http_actix::{
    management_resources::{
        ManagementChannelPinError, ManagementChannelPinFacade, ManagementChannelPinFuture,
        ManagementChannelPinOutcome, ManagementChannelPinReceipt, ManagementChannelPinRequest,
        ManagementRequestAttemptStage, ManagementResourceHttpState, configure_management_resources,
    },
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use gateway_store::control_plane::{
    AdministrativeStatus, CredentialScope, CredentialStatus, ModelRouteConfiguration,
    PublicModelConfiguration, RouteCandidateConfiguration, RoutePolicy, TransformMode,
    UpstreamConfiguration,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";
const VERSION: &str = "pin-v1";
const SECRET_MARKER: &str = "channel-pin-secret-must-not-leak";

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 46_408))
}

fn security_state() -> Result<ManagementHttpState, Box<dyn Error>> {
    Ok(ManagementHttpState::new(
        ManagementKey::try_new(MANAGEMENT_KEY)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?)
}

fn authorized(request: test::TestRequest) -> test::TestRequest {
    request
        .peer_addr(loopback())
        .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
        .insert_header(("X-Config-Version", VERSION))
        .insert_header(("If-Match", "\"rev-0\""))
}

struct FixtureChannelPinFacade {
    calls: Arc<AtomicUsize>,
}

impl ManagementChannelPinFacade for FixtureChannelPinFacade {
    fn execute(&self, request: ManagementChannelPinRequest) -> ManagementChannelPinFuture {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            ManagementChannelPinReceipt::try_new(
                gateway_core::RequestId::try_new("channel-pin-test-request")
                    .map_err(|_| ManagementChannelPinError::Unavailable)?,
                request.config_version_id().clone(),
                ConfigRevision::try_new(0).map_err(|_| ManagementChannelPinError::Unavailable)?,
                request.provider_id().clone(),
                request.channel_id().clone(),
                request.route_id().clone(),
                request.credential_id().clone(),
                request.requested_model().to_owned(),
                request.protocol(),
                request.mode(),
                ManagementChannelPinOutcome::Succeeded,
                true,
                1,
                true,
                123,
                Some(ManagementRequestAttemptStage::HttpStatus),
            )
        })
    }
}

fn resource_state(calls: Arc<AtomicUsize>) -> Result<ManagementResourceHttpState, Box<dyn Error>> {
    let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
    let key_version = KeyVersion::try_new(1)?;
    let key_ring = MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x4d_u8; 32])?)],
    )?;
    let secret_store = SecretStore::new(key_ring);
    let provider_id = UpstreamId::try_new("provider-pin")?;
    let channel_id = EndpointId::try_new("channel-pin")?;
    let credential_id = CredentialId::try_new("credential-pin")?;
    let public_model_id = gateway_core::PublicModelId::try_new("model-pin")?;
    let route_id = RouteId::try_new("route-pin")?;
    let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
        id: ConfigVersionId::try_new(VERSION)?,
        parent_id: None,
        // Persist as a draft first; activation is the store's only supported status transition
        // and mirrors the runtime-current active-version boundary exercised by this test.
        status: ConfigVersionStatus::Draft,
        revision: 0,
        created_at_ms: 1,
        description: "P13-08 HTTP fixture".to_owned(),
    });
    configuration.upstreams.push(UpstreamConfiguration {
        id: provider_id.clone(),
        name: "Pin Provider".to_owned(),
        kind: "openai-compatible".to_owned(),
        enabled: true,
        tags_json: "[]".to_owned(),
        egress_policy_id: None,
    });
    configuration.endpoints.push(EndpointConfiguration {
        id: channel_id.clone(),
        upstream_id: provider_id.clone(),
        adapter_id: "openai-compatible.responses".to_owned(),
        api_format: "openai/responses".to_owned(),
        base_url: "https://pin.example.test/v1".to_owned(),
        inference_path: "/responses".to_owned(),
        models_path: None,
        transport: EndpointTransport::Sse,
        enabled: true,
    });
    configuration.credentials.push(CredentialConfiguration {
        id: credential_id.clone(),
        upstream_id: provider_id.clone(),
        kind: "api_key".to_owned(),
        encrypted_secret: secret_store.seal(SECRET_MARKER.as_bytes(), b"p13-08-http")?,
        status: CredentialStatus::Active,
        revision: 0,
    });
    configuration
        .endpoint_credential_bindings
        .push(EndpointCredentialBindingConfiguration {
            endpoint_id: channel_id.clone(),
            credential_id: credential_id.clone(),
            upstream_id: provider_id.clone(),
            enabled: true,
            priority: 0,
            weight: 1,
            concurrency: 1,
        });
    configuration.public_models.push(PublicModelConfiguration {
        id: public_model_id.clone(),
        model_name: "pin-model".to_owned(),
        status: AdministrativeStatus::Active,
        display_name: "Pin model".to_owned(),
        capabilities_json: "{}".to_owned(),
    });
    configuration.model_routes.push(ModelRouteConfiguration {
        id: route_id.clone(),
        public_model_id,
        policy: RoutePolicy::RoundRobin,
        max_attempts: 1,
        bootstrap_timeout_ms: 1_000,
    });
    configuration
        .route_candidates
        .push(RouteCandidateConfiguration {
            id: gateway_core::RouteCandidateId::try_new("candidate-pin")?,
            route_id,
            endpoint_id: channel_id,
            upstream_model: "pin-upstream-model".to_owned(),
            credential_scope: CredentialScope::EndpointBindings,
            transform_mode: TransformMode::Canonical,
            enabled: true,
            priority: 0,
            weight: 1,
            capability_override_json: "{}".to_owned(),
        });
    let version_id = configuration.version.id.clone();
    repository.write_configuration(&configuration)?;
    repository.activate_version(&version_id)?;
    Ok(
        ManagementResourceHttpState::new(ManagementMutationService::new(repository, secret_store))
            .with_channel_pin(Box::new(FixtureChannelPinFacade { calls })),
    )
}

#[actix_web::test]
async fn channel_pin_is_authenticated_bounded_and_value_free() -> TestResult {
    let calls = Arc::new(AtomicUsize::new(0));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(resource_state(Arc::clone(&calls))?))
            .configure(configure_management_resources),
    )
    .await;

    let denied = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/admin/operations/channel-pin")
            .set_json(json!({}))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let missing_revision = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/admin/operations/channel-pin")
            .peer_addr(loopback())
            .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
            .insert_header(("X-Config-Version", VERSION))
            .set_json(json!({
                "provider_id":"provider-pin",
                "channel_id":"channel-pin",
                "route_id":"route-pin",
                "credential_id":"credential-pin",
                "requested_model":"pin-model",
                "protocol":"openai_responses",
                "mode":"sse"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(missing_revision.status(), StatusCode::BAD_REQUEST);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let response = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/operations/channel-pin")
                .set_json(json!({
                    "provider_id":"provider-pin",
                    "channel_id":"channel-pin",
                    "route_id":"route-pin",
                    "credential_id":"credential-pin",
                    "requested_model":"pin-model",
                    "protocol":"openai_responses",
                    "mode":"sse"
                })),
        )
        .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    let body = test::read_body(response).await;
    let serialized = String::from_utf8(body.to_vec())?;
    assert!(!serialized.contains(SECRET_MARKER));
    assert!(!serialized.contains("pin.example.test"));
    let body: Value = serde_json::from_slice(&body)?;
    assert_eq!(body["outcome"], "succeeded");
    assert_eq!(body["attempt_count"], 1);
    assert_eq!(body["upstream_sent"], true);
    assert_eq!(body["response_started"], true);
    assert_eq!(body["observed_at_ms"], 123);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let browser_without_csrf = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/operations/channel-pin")
                .insert_header(("Origin", "http://localhost"))
                .set_json(json!({
                    "provider_id":"provider-pin",
                    "channel_id":"channel-pin",
                    "route_id":"route-pin",
                    "credential_id":"credential-pin",
                    "requested_model":"pin-model",
                    "protocol":"openai_chat_completions",
                    "mode":"json"
                })),
        )
        .to_request(),
    )
    .await;
    // The management security cloak deliberately returns 404 for a browser-origin request
    // without the independent CSRF proof, so it cannot reveal the protected route's existence.
    assert_eq!(browser_without_csrf.status(), StatusCode::NOT_FOUND);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let json_response = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/operations/channel-pin")
                .set_json(json!({
                    "provider_id":"provider-pin",
                    "channel_id":"channel-pin",
                    "route_id":"route-pin",
                    "credential_id":"credential-pin",
                    "requested_model":"pin-model",
                    "protocol":"openai_chat_completions",
                    "mode":"json"
                })),
        )
        .to_request(),
    )
    .await;
    assert_eq!(json_response.status(), StatusCode::OK);
    assert_eq!(
        json_response.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    let json_body = test::read_body(json_response).await;
    let json_body: Value = serde_json::from_slice(&json_body)?;
    assert_eq!(json_body["protocol"], "openai_chat_completions");
    assert_eq!(json_body["mode"], "json");
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let unknown_field = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/operations/channel-pin")
                .set_json(json!({
                    "provider_id":"provider-pin",
                    "channel_id":"channel-pin",
                    "route_id":"route-pin",
                    "credential_id":"credential-pin",
                    "requested_model":"pin-model",
                    "protocol":"openai_responses",
                    "mode":"json",
                    "body":"must-be-rejected"
                })),
        )
        .to_request(),
    )
    .await;
    assert_eq!(unknown_field.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        unknown_field.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    Ok(())
}
