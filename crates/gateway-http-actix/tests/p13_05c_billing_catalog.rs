//! P13-05C protected immutable billing catalog management regression.

#![deny(unsafe_code)]

use std::{error::Error, net::SocketAddr};

use actix_web::{
    App,
    http::{StatusCode, header},
    test, web,
};
use gateway_control::management_mutation_service::{
    ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration, KeyVersion,
    ManagementMutationService, MasterKey, MasterKeyRing, SecretStore, SqliteControlPlaneRepository,
};
use gateway_http_actix::{
    management_resources::{ManagementResourceHttpState, configure_management_resources},
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementCsrfToken, ManagementHttpState,
        ManagementKey, ManagementNetworkPolicy, ManagementOrigin,
    },
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";
const CSRF_TOKEN: &str = "csrf_0123456789abcdefghijklmnopqrstuvwxyz";
const ORIGIN: &str = "https://admin.example.test";
const CONFIG_VERSION: &str = "billing-catalog-draft";

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 45_405))
}

fn security_state() -> Result<ManagementHttpState, Box<dyn Error>> {
    Ok(ManagementHttpState::new(
        ManagementKey::try_new(MANAGEMENT_KEY)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::SameOrigin {
            origin: ManagementOrigin::try_new(ORIGIN)?,
            csrf_token: ManagementCsrfToken::try_new(CSRF_TOKEN)?,
        },
    )?)
}

fn resource_state() -> Result<ManagementResourceHttpState, Box<dyn Error>> {
    let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
    repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
        id: ConfigVersionId::try_new(CONFIG_VERSION)?,
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision: 0,
        created_at_ms: 1,
        description: "P13-05C billing catalog fixture".to_owned(),
    }))?;
    let key_version = KeyVersion::try_new(1)?;
    let key_ring = MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x5a_u8; 32])?)],
    )?;
    Ok(ManagementResourceHttpState::new(
        ManagementMutationService::new(repository, SecretStore::new(key_ring)),
    ))
}

fn authorized(request: test::TestRequest) -> test::TestRequest {
    request
        .peer_addr(loopback())
        .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
        .insert_header((header::ORIGIN, ORIGIN))
        .insert_header(("X-Config-Version", CONFIG_VERSION))
}

fn catalog(version: &str, input_rate: u64) -> Value {
    json!({
        "catalog_version_id": version,
        "effective_at_ms": 1_000,
        "source": "imported",
        "entries": [{
            "provider_id": "provider-a",
            "channel_id": "channel-a",
            "model": "model-a",
            "input_microunits_per_million": input_rate,
            "output_microunits_per_million": 4_000_000,
            "reasoning_microunits_per_million": 0,
            "cache_read_microunits_per_million": 0,
            "cache_creation_microunits_per_million": 0,
            "cached_microunits_per_million": 0
        }]
    })
}

#[actix_web::test]
async fn catalog_import_is_csrf_guarded_atomic_revisioned_and_rollback_only_forks() -> TestResult {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(resource_state()?))
            .configure(configure_management_resources),
    )
    .await;

    let denied = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/billing/catalogs")
                .insert_header(("If-Match", "\"rev-0\""))
                .set_json(catalog("catalog-v1", 2_000_000)),
        )
        .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);

    let unsafe_integer = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/billing/catalogs")
                .insert_header(("If-Match", "\"rev-0\""))
                .insert_header(("X-Management-CSRF-Token", CSRF_TOKEN))
                .set_json(catalog("catalog-unsafe", 9_007_199_254_740_992)),
        )
        .to_request(),
    )
    .await;
    assert_eq!(unsafe_integer.status(), StatusCode::BAD_REQUEST);

    let imported = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/billing/catalogs")
                .insert_header(("If-Match", "\"rev-0\""))
                .insert_header(("X-Management-CSRF-Token", CSRF_TOKEN))
                .set_json(catalog("catalog-v1", 2_000_000)),
        )
        .to_request(),
    )
    .await;
    assert_eq!(imported.status(), StatusCode::CREATED);
    assert_eq!(
        imported.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-1\""))
    );
    let imported_body: Value = test::read_body_json(imported).await;
    assert_eq!(imported_body["catalog_version_id"], "catalog-v1");
    assert_eq!(imported_body["entry_count"], 1);
    assert_eq!(imported_body["operation"], "imported");
    assert!(imported_body["rolled_back_from"].is_null());

    let duplicate = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/billing/catalogs")
                .insert_header(("If-Match", "\"rev-1\""))
                .insert_header(("X-Management-CSRF-Token", CSRF_TOKEN))
                .set_json(catalog("catalog-v1", 2_000_000)),
        )
        .to_request(),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let conflict = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/billing/catalogs")
                .insert_header(("If-Match", "\"rev-1\""))
                .insert_header(("X-Management-CSRF-Token", CSRF_TOKEN))
                .set_json(catalog("catalog-v1", 9_000_000)),
        )
        .to_request(),
    )
    .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let conflict_body: Value = test::read_body_json(conflict).await;
    assert_eq!(
        conflict_body["error"]["code"],
        "management_billing_catalog_conflict"
    );

    let listed = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/billing/catalogs")).to_request(),
    )
    .await;
    assert_eq!(listed.status(), StatusCode::OK);
    assert_eq!(
        listed.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-1\""))
    );
    let listed_body: Value = test::read_body_json(listed).await;
    assert_eq!(listed_body.as_array().map(Vec::len), Some(1));
    assert_eq!(
        listed_body[0]["entries"][0]["input_microunits_per_million"],
        2_000_000
    );

    let rolled_back = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/billing/catalogs/catalog-v1/rollback")
                .insert_header(("If-Match", "\"rev-1\""))
                .insert_header(("X-Management-CSRF-Token", CSRF_TOKEN))
                .set_json(json!({
                    "new_catalog_version_id": "catalog-v2",
                    "effective_at_ms": 2_000
                })),
        )
        .to_request(),
    )
    .await;
    assert_eq!(rolled_back.status(), StatusCode::CREATED);
    assert_eq!(
        rolled_back.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-2\""))
    );
    let rollback_body: Value = test::read_body_json(rolled_back).await;
    assert_eq!(rollback_body["operation"], "rolled_back");
    assert_eq!(rollback_body["rolled_back_from"], "catalog-v1");
    assert_eq!(rollback_body["source"], "operator");

    let stale = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/billing/catalogs/catalog-v1/rollback")
                .insert_header(("If-Match", "\"rev-1\""))
                .insert_header(("X-Management-CSRF-Token", CSRF_TOKEN))
                .set_json(json!({
                    "new_catalog_version_id": "catalog-v3",
                    "effective_at_ms": 3_000
                })),
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);

    let final_list = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/billing/catalogs")).to_request(),
    )
    .await;
    assert_eq!(final_list.status(), StatusCode::OK);
    let final_body: Value = test::read_body_json(final_list).await;
    assert_eq!(final_body.as_array().map(Vec::len), Some(2));
    assert_eq!(final_body[0]["catalog_version_id"], "catalog-v1");
    assert_eq!(final_body[1]["catalog_version_id"], "catalog-v2");
    let serialized = serde_json::to_string(&final_body)?;
    for forbidden in [
        "secret",
        "ciphertext",
        "credential",
        "request_body",
        "source_fingerprint",
    ] {
        assert!(!serialized.to_ascii_lowercase().contains(forbidden));
    }
    Ok(())
}
