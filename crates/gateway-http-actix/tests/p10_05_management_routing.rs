//! P10-05 protected routing and Client Key workflow regression tests.

#![deny(unsafe_code)]

use std::{error::Error, net::SocketAddr};

use actix_web::{
    App,
    dev::ServiceResponse,
    http::{StatusCode, header},
    test, web,
};
use gateway_auth::client_key::{ClientKeyPepper, ClientKeyService};
use gateway_control::management_mutation_service::{
    ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration, KeyVersion,
    ManagementMutationService, MasterKey, MasterKeyRing, SecretStore, SqliteControlPlaneRepository,
};
use gateway_http_actix::{
    management_resources::{ManagementResourceHttpState, configure_management_resources},
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";
const VERSION: &str = "draft-p10-routing";

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 44_405))
}

fn resource_state() -> Result<ManagementResourceHttpState, Box<dyn Error>> {
    let version_id = ConfigVersionId::try_new(VERSION)?;
    let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
    repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
        id: version_id,
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision: 0,
        created_at_ms: 1,
        description: "P10-05 HTTP fixture".to_owned(),
    }))?;
    let key_version = KeyVersion::try_new(1)?;
    let key_ring = MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x63_u8; 32])?)],
    )?;
    let issuer = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x52_u8; 32])?);
    Ok(ManagementResourceHttpState::new(
        ManagementMutationService::with_client_key_service(
            repository,
            SecretStore::new(key_ring),
            issuer,
        ),
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

fn assert_revision(response: &ServiceResponse, revision: i64) {
    let expected = format!("\"rev-{revision}\"");
    assert_eq!(
        response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok()),
        Some(expected.as_str())
    );
}

#[actix_web::test]
async fn protected_minimax_m3_graph_and_client_key_lifecycle_are_exact_and_redacted() -> TestResult
{
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(resource_state()?))
            .configure(configure_management_resources),
    )
    .await;

    let denied = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/admin/public-models")
            .set_json(json!({}))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);

    let policy = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/egress-policies")
                .set_json(json!({
                    "id":"policy-routing", "name":"routing egress", "allowed_schemes":["https"],
                    "allowed_hosts":["api.example.test", "api-two.example.test"], "allowed_ports":[443], "allowed_cidrs":[],
                    "redirect_mode":"deny", "max_redirects":0
                })),
            Some("rev-0"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(policy.status(), StatusCode::CREATED);
    assert_revision(&policy, 1);

    let upstream = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams")
                .set_json(json!({
                    "id":"upstream-routing", "name":"routing fixture", "kind":"openai-compatible",
                    "enabled":true, "tags":[], "egress_policy_id":"policy-routing"
                })),
            Some("rev-1"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(upstream.status(), StatusCode::CREATED);
    assert_revision(&upstream, 2);

    let endpoint = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams/upstream-routing/endpoints")
                .set_json(json!({
                    "id":"endpoint-routing", "adapter_id":"openai-compatible.responses",
                    "api_format":"openai/responses", "base_url":"https://api.example.test/v1",
                    "inference_path":"/responses", "models_path":null, "transport":"https", "enabled":true
                })),
            Some("rev-2"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(endpoint.status(), StatusCode::CREATED);
    assert_revision(&endpoint, 3);

    let credential = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams/upstream-routing/credentials")
                .set_json(json!({
                    "id":"credential-routing", "kind":"api_key", "secret":"synthetic-routing-value", "status":"active"
                })),
            Some("rev-3"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(credential.status(), StatusCode::CREATED);
    assert_revision(&credential, 4);
    let credential_body = test::read_body(credential).await;
    assert!(
        !credential_body
            .windows(b"synthetic-routing-value".len())
            .any(|window| window == b"synthetic-routing-value")
    );

    let binding = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/endpoints/endpoint-routing/credential-bindings")
                .set_json(json!({
                    "credential_id":"credential-routing", "enabled":true, "priority":0, "weight":100, "concurrency":1
                })),
            Some("rev-4"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(binding.status(), StatusCode::CREATED);
    assert_revision(&binding, 5);

    let public_model = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/public-models")
                .set_json(json!({
                    "id":"model-minimax-m3", "model_name":"minimax-m3", "status":"active",
                    "display_name":"MiniMax M3", "capabilities":{}
                })),
            Some("rev-5"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(public_model.status(), StatusCode::CREATED);
    assert_revision(&public_model, 6);

    let updated_public_model = test::call_service(
        &app,
        authorized(
            test::TestRequest::patch()
                .uri("/admin/public-models/model-minimax-m3")
                .set_json(json!({
                    "id":"model-minimax-m3", "model_name":"minimax-m3", "status":"active",
                    "display_name":"MiniMax M3 current", "capabilities":{"streaming":true}
                })),
            Some("rev-6"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(updated_public_model.status(), StatusCode::OK);
    assert_revision(&updated_public_model, 7);

    let alias = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/public-models/model-minimax-m3/aliases")
                .set_json(json!({"alias":"minimax-m3-latest"})),
            Some("rev-7"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(alias.status(), StatusCode::CREATED);
    assert_revision(&alias, 8);

    let route = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/public-models/model-minimax-m3/routes")
                .set_json(json!({
                    "id":"route-minimax-m3", "policy":"smooth_weighted_round_robin",
                    "max_attempts":2, "bootstrap_timeout_ms":2000
                })),
            Some("rev-8"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(route.status(), StatusCode::CREATED);
    assert_revision(&route, 9);

    let updated_route = test::call_service(
        &app,
        authorized(
            test::TestRequest::patch()
                .uri("/admin/routes/route-minimax-m3")
                .set_json(json!({
                    "id":"route-minimax-m3", "policy":"smooth_weighted_round_robin",
                    "max_attempts":3, "bootstrap_timeout_ms":3000
                })),
            Some("rev-9"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(updated_route.status(), StatusCode::OK);
    assert_revision(&updated_route, 10);

    let candidate = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/routes/route-minimax-m3/candidates")
                .set_json(json!({
                    "id":"candidate-minimax-m3", "endpoint_id":"endpoint-routing", "upstream_model":"minimax-m3-upstream",
                    "credential_scope":"all_active", "transform_mode":"canonical", "enabled":true,
                    "priority":0, "weight":100, "capability_override":{}
                })),
            Some("rev-10"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(candidate.status(), StatusCode::CREATED);
    assert_revision(&candidate, 11);

    // G10 requires a genuine aggregate configuration rather than a single endpoint that merely
    // happens to use the public model name. Configure a second independently owned station
    // through the same protected API and attach both stations to the one `minimax-m3` route.
    let second_upstream = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams")
                .set_json(json!({
                    "id":"upstream-routing-two", "name":"routing fixture two", "kind":"openai-compatible",
                    "enabled":true, "tags":[], "egress_policy_id":"policy-routing"
                })),
            Some("rev-11"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(second_upstream.status(), StatusCode::CREATED);
    assert_revision(&second_upstream, 12);

    let second_endpoint = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams/upstream-routing-two/endpoints")
                .set_json(json!({
                    "id":"endpoint-routing-two", "adapter_id":"openai-compatible.responses",
                    "api_format":"openai/responses", "base_url":"https://api-two.example.test/v1",
                    "inference_path":"/responses", "models_path":null, "transport":"https", "enabled":true
                })),
            Some("rev-12"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(second_endpoint.status(), StatusCode::CREATED);
    assert_revision(&second_endpoint, 13);

    let second_credential = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams/upstream-routing-two/credentials")
                .set_json(json!({
                    "id":"credential-routing-two", "kind":"api_key", "secret":"synthetic-routing-value-two", "status":"active"
                })),
            Some("rev-13"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(second_credential.status(), StatusCode::CREATED);
    assert_revision(&second_credential, 14);
    let second_credential_body = test::read_body(second_credential).await;
    assert!(
        !second_credential_body
            .windows(b"synthetic-routing-value-two".len())
            .any(|window| window == b"synthetic-routing-value-two")
    );

    let second_binding = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/endpoints/endpoint-routing-two/credential-bindings")
                .set_json(json!({
                    "credential_id":"credential-routing-two", "enabled":true, "priority":0, "weight":100, "concurrency":1
                })),
            Some("rev-14"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(second_binding.status(), StatusCode::CREATED);
    assert_revision(&second_binding, 15);

    let second_candidate = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/routes/route-minimax-m3/candidates")
                .set_json(json!({
                    "id":"candidate-minimax-m3-two", "endpoint_id":"endpoint-routing-two", "upstream_model":"minimax-m3-upstream",
                    "credential_scope":"all_active", "transform_mode":"canonical", "enabled":true,
                    "priority":0, "weight":100, "capability_override":{}
                })),
            Some("rev-15"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(second_candidate.status(), StatusCode::CREATED);
    assert_revision(&second_candidate, 16);

    let validation = test::call_service(
        &app,
        authorized(
            test::TestRequest::post().uri("/admin/routes/route-minimax-m3/validate"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(validation.status(), StatusCode::OK);
    assert_eq!(
        test::read_body_json::<Value, _>(validation).await,
        json!({"valid":true,"error_codes":[]})
    );

    let access_group = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/access-groups")
                .set_json(json!({
                    "id":"group-minimax", "name":"MiniMax group", "status":"active", "limits":{}
                })),
            Some("rev-16"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(access_group.status(), StatusCode::CREATED);
    assert_revision(&access_group, 17);

    let updated_group = test::call_service(
        &app,
        authorized(
            test::TestRequest::patch()
                .uri("/admin/access-groups/group-minimax")
                .set_json(json!({
                    "id":"group-minimax", "name":"MiniMax group current", "status":"active", "limits":{"rpm":100}
                })),
            Some("rev-17"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(updated_group.status(), StatusCode::OK);
    assert_revision(&updated_group, 18);

    let grant = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/access-groups/group-minimax/routes")
                .set_json(json!({"route_id":"route-minimax-m3", "enabled":true})),
            Some("rev-18"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(grant.status(), StatusCode::CREATED);
    assert_revision(&grant, 19);

    let issued_key = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/client-keys")
                .set_json(json!({
                    "id":"client-minimax", "access_group_id":"group-minimax", "status":"active", "expires_at_ms":10000
                })),
            Some("rev-19"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(issued_key.status(), StatusCode::CREATED);
    assert_revision(&issued_key, 20);
    let issued_json = test::read_body_json::<Value, _>(issued_key).await;
    let presented_key = issued_json["key"]
        .as_str()
        .ok_or("missing one-time Client Key")?
        .to_owned();
    assert!(presented_key.starts_with("rgw_"));

    let stale_update = test::call_service(
        &app,
        authorized(
            test::TestRequest::patch()
                .uri("/admin/client-keys/client-minimax")
                .set_json(json!({
                    "id":"client-minimax", "access_group_id":"group-minimax", "status":"disabled", "expires_at_ms":20000
                })),
            Some("rev-19"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale_update.status(), StatusCode::CONFLICT);

    let key_read = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/client-keys/client-minimax"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(key_read.status(), StatusCode::OK);
    assert_revision(&key_read, 20);
    let key_read_body = test::read_body(key_read).await;
    assert!(
        !key_read_body
            .windows(presented_key.len())
            .any(|window| window == presented_key.as_bytes())
    );

    let updated_key = test::call_service(
        &app,
        authorized(
            test::TestRequest::patch()
                .uri("/admin/client-keys/client-minimax")
                .set_json(json!({
                    "id":"client-minimax", "access_group_id":"group-minimax", "status":"disabled", "expires_at_ms":20000
                })),
            Some("rev-20"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(updated_key.status(), StatusCode::OK);
    assert_revision(&updated_key, 21);
    let updated_key_body = test::read_body(updated_key).await;
    assert!(
        !updated_key_body
            .windows(presented_key.len())
            .any(|window| window == presented_key.as_bytes())
    );

    let revoked_key = test::call_service(
        &app,
        authorized(
            test::TestRequest::delete().uri("/admin/client-keys/client-minimax"),
            Some("rev-21"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(revoked_key.status(), StatusCode::NO_CONTENT);
    assert_revision(&revoked_key, 22);

    let deleted_route = test::call_service(
        &app,
        authorized(
            test::TestRequest::delete().uri("/admin/routes/route-minimax-m3"),
            Some("rev-22"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(deleted_route.status(), StatusCode::NO_CONTENT);
    assert_revision(&deleted_route, 23);

    let grants = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/access-groups/group-minimax/routes"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(grants.status(), StatusCode::OK);
    assert_revision(&grants, 23);
    assert_eq!(test::read_body_json::<Value, _>(grants).await, json!([]));

    let deleted_group = test::call_service(
        &app,
        authorized(
            test::TestRequest::delete().uri("/admin/access-groups/group-minimax"),
            Some("rev-23"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(deleted_group.status(), StatusCode::NO_CONTENT);
    assert_revision(&deleted_group, 24);

    let keys = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/client-keys"), None).to_request(),
    )
    .await;
    assert_eq!(keys.status(), StatusCode::OK);
    assert_revision(&keys, 24);
    let keys_body = test::read_body(keys).await;
    assert!(
        !keys_body
            .windows(presented_key.len())
            .any(|window| window == presented_key.as_bytes())
    );

    let deleted_public_model = test::call_service(
        &app,
        authorized(
            test::TestRequest::delete().uri("/admin/public-models/model-minimax-m3"),
            Some("rev-24"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(deleted_public_model.status(), StatusCode::NO_CONTENT);
    assert_revision(&deleted_public_model, 25);

    let missing_public_model = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/public-models/model-minimax-m3"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(missing_public_model.status(), StatusCode::NOT_FOUND);
    Ok(())
}
