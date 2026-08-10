//! P13-04A configured account-pool inventory HTTP regression tests.

#![deny(unsafe_code)]

use std::{error::Error, net::SocketAddr};

use actix_web::{
    App,
    http::{StatusCode, header},
    test, web,
};
use gateway_control::management_mutation_service::{
    ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
    CredentialConfiguration, EndpointConfiguration, EndpointCredentialBindingConfiguration,
    EndpointTransport, KeyVersion, ManagementMutationService, MasterKey, MasterKeyRing,
    SecretStore, SqliteControlPlaneRepository,
};
use gateway_core::{CredentialId, EndpointId, UpstreamId};
use gateway_http_actix::{
    management_resources::{ManagementResourceHttpState, configure_management_resources},
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use gateway_store::control_plane::{CredentialStatus, UpstreamConfiguration};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 45_405))
}

fn security_state() -> Result<ManagementHttpState, Box<dyn Error>> {
    Ok(ManagementHttpState::new(
        ManagementKey::try_new(MANAGEMENT_KEY)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?)
}

fn authorized(request: test::TestRequest, version: &str) -> test::TestRequest {
    request
        .peer_addr(loopback())
        .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
        .insert_header(("X-Config-Version", version))
}

fn resource_state() -> Result<ManagementResourceHttpState, Box<dyn Error>> {
    let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
    let key_version = KeyVersion::try_new(1)?;
    let key_ring = MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x5a_u8; 32])?)],
    )?;
    let secret_store = SecretStore::new(key_ring);
    for (version, revision) in [("inventory-v1", 4_i64), ("inventory-v2", 9_i64)] {
        repository.write_configuration(&fixture(
            ConfigVersionId::try_new(version)?,
            revision,
            &secret_store,
        )?)?;
    }
    Ok(ManagementResourceHttpState::new(
        ManagementMutationService::new(repository, secret_store),
    ))
}

fn fixture(
    version_id: ConfigVersionId,
    revision: i64,
    secret_store: &SecretStore,
) -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
    let provider_id = UpstreamId::try_new("provider-inventory")?;
    let channel_id = EndpointId::try_new("channel-inventory")?;
    let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
        id: version_id,
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision,
        created_at_ms: 1,
        description: "P13-04A HTTP fixture".to_owned(),
    });
    configuration.upstreams.push(UpstreamConfiguration {
        id: provider_id.clone(),
        name: "Inventory Provider".to_owned(),
        kind: "openai-compatible".to_owned(),
        enabled: true,
        tags_json: "[]".to_owned(),
        egress_policy_id: None,
    });
    configuration.endpoints.push(EndpointConfiguration {
        id: channel_id.clone(),
        upstream_id: provider_id.clone(),
        adapter_id: "inventory.responses".to_owned(),
        api_format: "openai/responses".to_owned(),
        base_url: "https://secret-upstream.example/v1".to_owned(),
        inference_path: "/responses".to_owned(),
        models_path: None,
        transport: EndpointTransport::Sse,
        enabled: true,
    });
    for account_name in ["account-a", "account-b"] {
        let account_id = CredentialId::try_new(account_name)?;
        configuration.credentials.push(CredentialConfiguration {
            id: account_id.clone(),
            upstream_id: provider_id.clone(),
            kind: "oauth_json".to_owned(),
            encrypted_secret: secret_store.seal(b"secret-must-not-leak", b"p13-04a-http")?,
            status: if account_name == "account-a" {
                CredentialStatus::Active
            } else {
                CredentialStatus::Cooling
            },
            revision: 2,
        });
        configuration
            .endpoint_credential_bindings
            .push(EndpointCredentialBindingConfiguration {
                endpoint_id: channel_id.clone(),
                credential_id: account_id,
                upstream_id: provider_id.clone(),
                enabled: true,
                priority: 0,
                weight: 1,
                concurrency: 2,
            });
    }
    Ok(configuration)
}

#[actix_web::test]
async fn inventory_is_protected_paginated_and_value_free() -> TestResult {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(resource_state()?))
            .configure(configure_management_resources),
    )
    .await;

    let first = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/account-pools?limit=1"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(
        first.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-4\""))
    );
    let first_body: Value = test::read_body_json(first).await;
    assert_eq!(first_body["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(first_body["items"][0]["account_id"], "account-a");
    let cursor = first_body["next_cursor"]
        .as_str()
        .ok_or("first page missing cursor")?;
    let serialized = serde_json::to_string(&first_body)?;
    for forbidden in [
        "secret-must-not-leak",
        "secret-upstream.example",
        "encrypted_secret",
        "ciphertext",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "response leaked {forbidden}"
        );
    }

    let second = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(&format!(
                "/admin/operations/account-pools?limit=1&cursor={cursor}"
            )),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    let second_body: Value = test::read_body_json(second).await;
    assert_eq!(second_body["items"][0]["account_id"], "account-b");
    assert!(second_body["next_cursor"].is_null());

    let stale = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(&format!(
                "/admin/operations/account-pools?limit=1&cursor={cursor}"
            )),
            "inventory-v2",
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);

    let invalid_query = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/account-pools?unknown=true"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(invalid_query.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid_query.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );

    let duplicate_query = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/account-pools?limit=1&limit=2"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(duplicate_query.status(), StatusCode::BAD_REQUEST);

    let denied = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/operations/account-pools")
            .peer_addr(loopback())
            .insert_header(("X-Config-Version", "inventory-v1"))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);
    Ok(())
}
