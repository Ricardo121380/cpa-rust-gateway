//! P10-04 protected versioned resource workflow regression tests.

#![deny(unsafe_code)]

use std::{
    collections::BTreeMap,
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
    ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration, KeyVersion,
    ManagementMutationService, MasterKey, MasterKeyRing, SecretStore, SqliteControlPlaneRepository,
};
use gateway_core::{CredentialId, EndpointId};
use gateway_http_actix::{
    management_resources::{
        ManagementCatalogDiff, ManagementCredentialOAuthOperation, ManagementCredentialOAuthState,
        ManagementEndpointStatusClass, ManagementEndpointTestMode, ManagementEndpointTestOutcome,
        ManagementEndpointTestResult, ManagementEndpointWorkflow, ManagementResourceHttpState,
        configure_management_resources,
    },
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";
const VERSION: &str = "draft-p10";

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 44_404))
}

fn resource_state() -> Result<ManagementResourceHttpState, Box<dyn Error>> {
    resource_state_with_workflow(Box::new(
        gateway_http_actix::management_resources::RejectingManagementEndpointWorkflow::new(),
    ))
}

fn resource_state_with_workflow(
    workflow: Box<dyn ManagementEndpointWorkflow>,
) -> Result<ManagementResourceHttpState, Box<dyn Error>> {
    let version_id = ConfigVersionId::try_new(VERSION)?;
    let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
    repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
        id: version_id,
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision: 0,
        created_at_ms: 1,
        description: "P10-04 HTTP fixture".to_owned(),
    }))?;
    let key_version = KeyVersion::try_new(1)?;
    let key_ring = MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x61_u8; 32])?)],
    )?;
    Ok(ManagementResourceHttpState::with_workflow(
        ManagementMutationService::new(repository, SecretStore::new(key_ring)),
        workflow,
    ))
}

fn security_state() -> Result<ManagementHttpState, Box<dyn Error>> {
    Ok(ManagementHttpState::new(
        ManagementKey::try_new(MANAGEMENT_KEY)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?)
}

fn authorized(request: test::TestRequest, revision: Option<&str>) -> test::TestRequest {
    let request = request
        .peer_addr(loopback())
        .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
        .insert_header(("X-Config-Version", VERSION));
    match revision {
        Some(revision) => request.insert_header(("If-Match", revision)),
        None => request,
    }
}

#[actix_web::test]
async fn exact_loopback_http_endpoint_is_admitted_without_broadening_plaintext_egress() -> TestResult
{
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(resource_state()?))
            .configure(configure_management_resources),
    )
    .await;

    let policy = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/egress-policies")
                .set_json(json!({
                    "id":"loopback-policy", "name":"exact-loopback",
                    "allowed_schemes":["http"], "allowed_hosts":["127.0.0.1"],
                    "allowed_ports":[18000], "allowed_cidrs":["127.0.0.1/32"],
                    "redirect_mode":"deny", "max_redirects":0
                })),
            Some("rev-0"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(policy.status(), StatusCode::CREATED);

    let upstream = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams")
                .set_json(json!({
                    "id":"loopback-upstream", "name":"loopback", "kind":"openai-compatible",
                    "enabled":true, "tags":[], "egress_policy_id":"loopback-policy"
                })),
            Some("rev-1"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(upstream.status(), StatusCode::CREATED);

    let endpoint = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams/loopback-upstream/endpoints")
                .set_json(json!({
                    "id":"loopback-endpoint", "adapter_id":"openai-compatible.responses",
                    "api_format":"openai/responses", "base_url":"http://127.0.0.1:18000/v1",
                    "inference_path":"/responses", "transport":"http", "enabled":true
                })),
            Some("rev-2"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(endpoint.status(), StatusCode::CREATED);

    for (schemes, hosts, ports, cidrs) in [
        (
            json!(["http"]),
            json!(["localhost"]),
            json!([18000]),
            json!(["127.0.0.1/32"]),
        ),
        (
            json!(["http"]),
            json!(["127.0.0.1"]),
            json!([18000, 18001]),
            json!(["127.0.0.1/32"]),
        ),
        (
            json!(["http"]),
            json!(["127.0.0.1"]),
            json!([18000]),
            json!(["127.0.0.0/8"]),
        ),
        (
            json!(["http", "https"]),
            json!(["127.0.0.1"]),
            json!([18000]),
            json!(["127.0.0.1/32"]),
        ),
    ] {
        let rejected = test::call_service(
            &app,
            authorized(
                test::TestRequest::post()
                    .uri("/admin/egress-policies")
                    .set_json(json!({
                        "id":"rejected-policy", "name":"rejected", "allowed_schemes":schemes,
                        "allowed_hosts":hosts, "allowed_ports":ports, "allowed_cidrs":cidrs,
                        "redirect_mode":"deny", "max_redirects":0
                    })),
                Some("rev-3"),
            )
            .to_request(),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    }

    for base_url in [
        "http://localhost:18000/v1",
        "http://127.0.0.1/v1",
        "http://user@127.0.0.1:18000/v1",
        "http://127.0.0.1:18000/v1?query=1",
    ] {
        let rejected = test::call_service(
            &app,
            authorized(
                test::TestRequest::post()
                    .uri("/admin/upstreams/loopback-upstream/endpoints")
                    .set_json(json!({
                        "id":"rejected-endpoint", "adapter_id":"openai-compatible.responses",
                        "api_format":"openai/responses", "base_url":base_url,
                        "inference_path":"/responses", "transport":"http", "enabled":true
                    })),
                Some("rev-3"),
            )
            .to_request(),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    }
    Ok(())
}

struct DeterministicWorkflow {
    endpoint_test_calls: Arc<AtomicUsize>,
    preview_calls: Arc<AtomicUsize>,
    apply_calls: Arc<AtomicUsize>,
    oauth: BTreeMap<CredentialId, ManagementCredentialOAuthOperation>,
}

impl DeterministicWorkflow {
    fn new(
        endpoint_test_calls: Arc<AtomicUsize>,
        preview_calls: Arc<AtomicUsize>,
        apply_calls: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            endpoint_test_calls,
            preview_calls,
            apply_calls,
            oauth: BTreeMap::new(),
        }
    }
}

impl ManagementEndpointWorkflow for DeterministicWorkflow {
    fn test_endpoint(
        &mut self,
        _endpoint_id: &EndpointId,
        mode: ManagementEndpointTestMode,
    ) -> ManagementEndpointTestResult {
        self.endpoint_test_calls.fetch_add(1, Ordering::SeqCst);
        match mode {
            ManagementEndpointTestMode::NonStreaming => ManagementEndpointTestResult {
                outcome: ManagementEndpointTestOutcome::Pass,
                status_class: ManagementEndpointStatusClass::TwoXx,
                canonical_lifecycle: true,
            },
            ManagementEndpointTestMode::Sse => ManagementEndpointTestResult {
                outcome: ManagementEndpointTestOutcome::ProtocolFailed,
                status_class: ManagementEndpointStatusClass::FiveXx,
                canonical_lifecycle: false,
            },
        }
    }

    fn preview_catalog(&mut self, _endpoint_id: &EndpointId) -> ManagementCatalogDiff {
        self.preview_calls.fetch_add(1, Ordering::SeqCst);
        ManagementCatalogDiff {
            added: 3,
            removed: 1,
            unchanged: 8,
        }
    }

    fn apply_catalog(&mut self, _endpoint_id: &EndpointId) -> ManagementCatalogDiff {
        self.apply_calls.fetch_add(1, Ordering::SeqCst);
        ManagementCatalogDiff {
            added: 3,
            removed: 1,
            unchanged: 8,
        }
    }

    fn start_oauth(&mut self, credential_id: &CredentialId) -> ManagementCredentialOAuthOperation {
        let operation = ManagementCredentialOAuthOperation {
            state: ManagementCredentialOAuthState::Pending,
            expires_at_ms: Some(99),
        };
        self.oauth.insert(credential_id.clone(), operation);
        operation
    }

    fn oauth_status(&mut self, credential_id: &CredentialId) -> ManagementCredentialOAuthOperation {
        self.oauth
            .get(credential_id)
            .copied()
            .unwrap_or(ManagementCredentialOAuthOperation {
                state: ManagementCredentialOAuthState::Failed,
                expires_at_ms: None,
            })
    }

    fn cancel_oauth(&mut self, credential_id: &CredentialId) {
        self.oauth.insert(
            credential_id.clone(),
            ManagementCredentialOAuthOperation {
                state: ManagementCredentialOAuthState::Cancelled,
                expires_at_ms: None,
            },
        );
    }
}

#[actix_web::test]
async fn resource_crud_is_protected_revision_guarded_and_never_returns_credential_secret()
-> TestResult {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(resource_state()?))
            .configure(configure_management_resources),
    )
    .await;

    let response = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/egress-policies")
                .set_json(json!({
                    "id":"policy-a",
                    "name":"provider-egress",
                    "allowed_schemes":["https"],
                    "allowed_hosts":["api.example.test"],
                    "allowed_ports":[443],
                    "allowed_cidrs":[],
                    "redirect_mode":"deny",
                    "max_redirects":0
                })),
            Some("rev-0"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-1\""))
    );

    let updated_policy = test::call_service(
        &app,
        authorized(
            test::TestRequest::patch()
                .uri("/admin/egress-policies/policy-a")
                .set_json(json!({
                    "id":"policy-a", "name":"provider-egress-updated",
                    "allowed_schemes":["https"], "allowed_hosts":["updated.example.test"],
                    "allowed_ports":[443], "allowed_cidrs":[], "redirect_mode":"deny",
                    "max_redirects":0
                })),
            Some("rev-1"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(updated_policy.status(), StatusCode::OK);
    assert_eq!(
        updated_policy.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-2\""))
    );

    let stale = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams")
                .set_json(json!({
                    "id":"upstream-stale", "name":"stale", "kind":"openai-compatible",
                    "enabled":true, "tags":[], "egress_policy_id":"policy-a"
                })),
            Some("rev-0"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        test::read_body_json::<Value, _>(stale).await["error"]["code"],
        "management_revision_conflict"
    );

    let upstream = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams")
                .set_json(json!({
                    "id":"upstream-a", "name":"primary", "kind":"openai-compatible",
                    "enabled":true, "tags":["test"], "egress_policy_id":"policy-a"
                })),
            Some("rev-2"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(upstream.status(), StatusCode::CREATED);
    assert_eq!(
        upstream.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-3\""))
    );

    let credential = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams/upstream-a/credentials")
                .set_json(json!({
                    "id":"credential-a", "kind":"api_key", "secret":"p10-secret-value",
                    "status":"active"
                })),
            Some("rev-3"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(credential.status(), StatusCode::CREATED);
    assert_eq!(
        credential.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-4\""))
    );
    let credential_body = test::read_body(credential).await;
    assert!(
        !credential_body
            .windows(b"p10-secret-value".len())
            .any(|window| window == b"p10-secret-value")
    );
    let credential_json: Value = serde_json::from_slice(&credential_body)?;
    assert_eq!(credential_json["secret_present"], true);
    assert!(credential_json.get("secret").is_none());

    let read = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/credentials/credential-a"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(
        read.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-4\""))
    );
    let read_body = test::read_body(read).await;
    assert!(
        !read_body
            .windows(b"p10-secret-value".len())
            .any(|window| window == b"p10-secret-value")
    );

    let updated_upstream = test::call_service(
        &app,
        authorized(
            test::TestRequest::patch()
                .uri("/admin/upstreams/upstream-a")
                .set_json(json!({
                    "id":"upstream-a", "name":"primary-updated", "kind":"openai-compatible",
                    "enabled":false, "tags":["updated"], "egress_policy_id":"policy-a"
                })),
            Some("rev-4"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(updated_upstream.status(), StatusCode::OK);
    assert_eq!(
        updated_upstream.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-5\""))
    );

    let updated_credential = test::call_service(
        &app,
        authorized(
            test::TestRequest::patch()
                .uri("/admin/credentials/credential-a")
                .set_json(json!({
                    "id":"credential-a", "kind":"api_key", "secret":"p10-updated-secret",
                    "status":"disabled"
                })),
            Some("rev-5"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(updated_credential.status(), StatusCode::OK);
    assert_eq!(
        updated_credential.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-6\""))
    );
    let updated_credential_body = test::read_body(updated_credential).await;
    assert!(
        !updated_credential_body
            .windows(b"p10-updated-secret".len())
            .any(|window| window == b"p10-updated-secret")
    );

    let deleted_upstream = test::call_service(
        &app,
        authorized(
            test::TestRequest::delete().uri("/admin/upstreams/upstream-a"),
            Some("rev-6"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(deleted_upstream.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        deleted_upstream.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-7\""))
    );

    let deleted_credential = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/credentials/credential-a"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(deleted_credential.status(), StatusCode::NOT_FOUND);

    let deleted_policy = test::call_service(
        &app,
        authorized(
            test::TestRequest::delete().uri("/admin/egress-policies/policy-a"),
            Some("rev-7"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(deleted_policy.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        deleted_policy.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-8\""))
    );

    let absent_policy = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/egress-policies/policy-a"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(absent_policy.status(), StatusCode::NOT_FOUND);

    let denied = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/upstreams")
            .peer_addr(loopback())
            .insert_header(("X-Config-Version", VERSION))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[actix_web::test]
async fn endpoint_catalog_and_oauth_workflows_are_versioned_injected_and_value_free() -> TestResult
{
    let endpoint_test_calls = Arc::new(AtomicUsize::new(0));
    let preview_calls = Arc::new(AtomicUsize::new(0));
    let apply_calls = Arc::new(AtomicUsize::new(0));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(resource_state_with_workflow(Box::new(
                DeterministicWorkflow::new(
                    Arc::clone(&endpoint_test_calls),
                    Arc::clone(&preview_calls),
                    Arc::clone(&apply_calls),
                ),
            ))?))
            .configure(configure_management_resources),
    )
    .await;

    let policy = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/egress-policies")
                .set_json(json!({
                    "id":"policy-b", "name":"workflow-egress", "allowed_schemes":["https"],
                    "allowed_hosts":["api.example.test"], "allowed_ports":[443], "allowed_cidrs":[],
                    "redirect_mode":"deny", "max_redirects":0
                })),
            Some("rev-0"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(policy.status(), StatusCode::CREATED);

    let upstream = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams")
                .set_json(json!({
                    "id":"upstream-b", "name":"workflow", "kind":"openai-compatible",
                    "enabled":true, "tags":[], "egress_policy_id":"policy-b"
                })),
            Some("rev-1"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(upstream.status(), StatusCode::CREATED);

    let endpoint = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams/upstream-b/endpoints")
                .set_json(json!({
                    "id":"endpoint-b", "adapter_id":"openai-compatible.responses",
                    "api_format":"openai/responses", "base_url":"https://api.example.test/v1",
                    "inference_path":"/responses", "models_path":"/models", "transport":"https",
                    "enabled":true
                })),
            Some("rev-2"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(endpoint.status(), StatusCode::CREATED);

    let credential = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams/upstream-b/credentials")
                .set_json(json!({
                    "id":"credential-b", "kind":"oauth", "secret":"workflow-secret", "status":"active"
                })),
            Some("rev-3"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(credential.status(), StatusCode::CREATED);

    let contract_rejected_binding = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/endpoints/endpoint-b/credential-bindings")
                .set_json(json!({
                    "endpoint_id":"endpoint-b", "credential_id":"credential-b", "enabled":true,
                    "priority":0, "weight":100, "concurrency":1
                })),
            Some("rev-4"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(contract_rejected_binding.status(), StatusCode::BAD_REQUEST);

    let binding = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/endpoints/endpoint-b/credential-bindings")
                .set_json(json!({
                    "credential_id":"credential-b", "enabled":true,
                    "priority":0, "weight":100, "concurrency":1
                })),
            Some("rev-4"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(binding.status(), StatusCode::CREATED);
    assert_eq!(
        binding.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-5\""))
    );
    assert_eq!(
        test::read_body_json::<Value, _>(binding).await,
        json!({
            "endpoint_id":"endpoint-b", "upstream_id":"upstream-b", "credential_id":"credential-b",
            "enabled":true, "priority":0, "weight":100, "concurrency":1
        })
    );

    let non_streaming = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/endpoints/endpoint-b/test")
                .set_json(json!({"mode":"non_streaming"})),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(non_streaming.status(), StatusCode::OK);
    assert_eq!(
        test::read_body_json::<Value, _>(non_streaming).await,
        json!({"outcome":"pass", "status_class":"2xx", "canonical_lifecycle":true})
    );

    let sse = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/endpoints/endpoint-b/test")
                .set_json(json!({"mode":"sse"})),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(sse.status(), StatusCode::OK);
    assert_eq!(
        test::read_body_json::<Value, _>(sse).await,
        json!({"outcome":"protocol_failed", "status_class":"5xx", "canonical_lifecycle":false})
    );
    assert_eq!(endpoint_test_calls.load(Ordering::SeqCst), 2);

    let preview = test::call_service(
        &app,
        authorized(
            test::TestRequest::post().uri("/admin/endpoints/endpoint-b/models/discover-preview"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(preview.status(), StatusCode::OK);
    assert_eq!(
        test::read_body_json::<Value, _>(preview).await,
        json!({"added":3, "removed":1, "unchanged":8})
    );
    assert_eq!(preview_calls.load(Ordering::SeqCst), 1);

    let stale_apply = test::call_service(
        &app,
        authorized(
            test::TestRequest::post().uri("/admin/endpoints/endpoint-b/models/discover-apply"),
            Some("rev-4"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale_apply.status(), StatusCode::CONFLICT);
    assert_eq!(apply_calls.load(Ordering::SeqCst), 0);

    let apply = test::call_service(
        &app,
        authorized(
            test::TestRequest::post().uri("/admin/endpoints/endpoint-b/models/discover-apply"),
            Some("rev-5"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(apply.status(), StatusCode::OK);
    assert_eq!(
        apply.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-6\""))
    );
    assert_eq!(apply_calls.load(Ordering::SeqCst), 1);

    let start = test::call_service(
        &app,
        authorized(
            test::TestRequest::post().uri("/admin/credentials/credential-b/oauth/start"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(start.status(), StatusCode::ACCEPTED);
    assert_eq!(
        test::read_body_json::<Value, _>(start).await,
        json!({"credential_id":"credential-b", "state":"pending", "expires_at_ms":99})
    );

    let pending = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/credentials/credential-b/oauth/status"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(pending.status(), StatusCode::OK);
    assert_eq!(
        test::read_body_json::<Value, _>(pending).await["state"],
        "pending"
    );

    let cancelled = test::call_service(
        &app,
        authorized(
            test::TestRequest::post().uri("/admin/credentials/credential-b/oauth/cancel"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(cancelled.status(), StatusCode::NO_CONTENT);

    let cancelled_status = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/credentials/credential-b/oauth/status"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(cancelled_status.status(), StatusCode::OK);
    assert_eq!(
        test::read_body_json::<Value, _>(cancelled_status).await,
        json!({"credential_id":"credential-b", "state":"cancelled", "expires_at_ms":null})
    );

    let updated_endpoint = test::call_service(
        &app,
        authorized(
            test::TestRequest::patch()
                .uri("/admin/endpoints/endpoint-b")
                .set_json(json!({
                    "id":"endpoint-b", "adapter_id":"openai-compatible.responses",
                    "api_format":"openai/responses", "base_url":"https://api.example.test/v2",
                    "inference_path":"/responses", "models_path":"/models", "transport":"https",
                    "enabled":false
                })),
            Some("rev-6"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(updated_endpoint.status(), StatusCode::OK);
    assert_eq!(
        updated_endpoint.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-7\""))
    );

    let deleted_endpoint = test::call_service(
        &app,
        authorized(
            test::TestRequest::delete().uri("/admin/endpoints/endpoint-b"),
            Some("rev-7"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(deleted_endpoint.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        deleted_endpoint.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-8\""))
    );

    let bindings_after_delete = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/endpoints/endpoint-b/credential-bindings"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(bindings_after_delete.status(), StatusCode::NOT_FOUND);
    Ok(())
}
