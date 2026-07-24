//! P10-07 protected Config Version lifecycle regression tests.

#![deny(unsafe_code)]

use std::{
    error::Error,
    fs,
    net::SocketAddr,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use actix_web::{App, http::StatusCode, test, web};
use gateway_control::management_service::{ManagementActor, ManagementService};
use gateway_http_actix::{
    management_lifecycle_resources::{
        ManagementLifecycleHttpState, RejectingManagementLifecycleFacade,
        configure_management_lifecycle_resources,
    },
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 44_407))
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
}

struct TemporaryDatabase(PathBuf);

impl TemporaryDatabase {
    fn new() -> Result<Self, Box<dyn Error>> {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        Ok(Self(std::env::temp_dir().join(format!(
            "cpa-rust-gateway-p10-07-{elapsed}-{}.sqlite3",
            std::process::id()
        ))))
    }

    fn service(&self) -> Result<ManagementService, Box<dyn Error>> {
        Ok(ManagementService::open_local(
            &self.0,
            ManagementActor::try_new("management-key")?,
        )?)
    }
}

impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        let _ = fs::remove_file(self.0.with_extension("sqlite3-wal"));
        let _ = fs::remove_file(self.0.with_extension("sqlite3-shm"));
    }
}

async fn response_json(response: actix_web::dev::ServiceResponse) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&test::read_body(response).await)?)
}

#[actix_web::test]
async fn lifecycle_routes_preserve_p2_publication_and_rollback_invariants() -> TestResult {
    let database = TemporaryDatabase::new()?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(ManagementLifecycleHttpState::new(
                database.service()?,
            )))
            .configure(configure_management_lifecycle_resources),
    )
    .await;

    let denied = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/config-versions")
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);

    let version_one = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/config-versions")
                .set_json(json!({"id":"version-one","description":"first safe version"})),
        )
        .to_request(),
    )
    .await;
    assert_eq!(version_one.status(), StatusCode::CREATED);
    let version_one = response_json(version_one).await?;
    assert_eq!(version_one["id"], "version-one");
    assert_eq!(version_one["status"], "draft");
    assert_eq!(version_one["revision"], "rev-0");
    assert_eq!(version_one["description"], "first safe version");
    assert!(
        version_one["created_at_ms"]
            .as_i64()
            .is_some_and(|value| value >= 0)
    );

    let version_one = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/config-versions/version-one")).to_request(),
    )
    .await;
    let version_one = response_json(version_one).await?;
    assert_eq!(version_one["id"], "version-one");
    assert_eq!(version_one["revision"], "rev-0");
    assert!(
        version_one["created_at_ms"]
            .as_i64()
            .is_some_and(|value| value >= 0)
    );

    let validation = test::call_service(
        &app,
        authorized(test::TestRequest::post().uri("/admin/config-versions/version-one/validate"))
            .to_request(),
    )
    .await;
    assert_eq!(validation.status(), StatusCode::OK);
    assert_eq!(
        response_json(validation).await?,
        json!({"valid":true,"error_codes":[]})
    );

    let failed_publish = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/config-versions/absent/publish")
                .insert_header(("If-Match", "rev-0")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(failed_publish.status(), StatusCode::CONFLICT);

    let before_publish_audit = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/audit-events")).to_request(),
    )
    .await;
    let before_publish_audit = response_json(before_publish_audit).await?;
    let events = before_publish_audit
        .as_array()
        .ok_or("audit response must be an array")?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["id"], 1);
    assert_eq!(events[0]["action"], "config_created");
    assert_eq!(events[0]["actor"], "management-key");
    assert_eq!(events[0]["config_version_id"], "version-one");
    assert!(
        events[0]["occurred_at_ms"]
            .as_i64()
            .is_some_and(|value| value >= 0)
    );

    let first_publish = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/config-versions/version-one/publish")
                .insert_header(("If-Match", "rev-0")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(first_publish.status(), StatusCode::OK);
    assert_eq!(
        response_json(first_publish).await?,
        json!({"active_config_version_id":"version-one"})
    );

    let version_two = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/config-versions")
                .set_json(json!({
                    "id":"version-two", "parent_id":"version-one", "description":"second safe version"
                })),
        )
        .to_request(),
    )
    .await;
    assert_eq!(version_two.status(), StatusCode::CREATED);

    let stale_publish = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/config-versions/version-two/publish")
                .insert_header(("If-Match", "rev-1")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale_publish.status(), StatusCode::CONFLICT);

    let versions_after_stale = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/config-versions")).to_request(),
    )
    .await;
    let versions = response_json(versions_after_stale).await?;
    let versions = versions
        .as_array()
        .ok_or("version response must be an array")?;
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0]["id"], "version-one");
    assert_eq!(versions[0]["status"], "active");
    assert_eq!(versions[0]["revision"], "rev-0");
    assert_eq!(versions[1]["id"], "version-two");
    assert_eq!(versions[1]["parent_id"], "version-one");
    assert_eq!(versions[1]["status"], "draft");
    assert_eq!(versions[1]["revision"], "rev-0");

    let second_publish = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/config-versions/version-two/publish")
                .insert_header(("If-Match", "rev-0")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(second_publish.status(), StatusCode::OK);
    assert_eq!(
        response_json(second_publish).await?,
        json!({"active_config_version_id":"version-two","replaced_config_version_id":"version-one"})
    );

    let stale_rollback = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/config-versions/rollback")
                .insert_header(("If-Match", "rev-1")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale_rollback.status(), StatusCode::CONFLICT);

    let rollback = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/config-versions/rollback")
                .insert_header(("If-Match", "rev-0")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(rollback.status(), StatusCode::OK);
    assert_eq!(
        response_json(rollback).await?,
        json!({"active_config_version_id":"version-one","replaced_config_version_id":"version-two"})
    );

    let audit = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/audit-events")).to_request(),
    )
    .await;
    let audit = response_json(audit).await?;
    let events = audit.as_array().ok_or("audit response must be an array")?;
    assert_eq!(events.len(), 5);
    assert_eq!(events[0]["action"], "config_created");
    assert_eq!(events[1]["action"], "config_published");
    assert_eq!(events[2]["action"], "config_created");
    assert_eq!(events[3]["action"], "config_published");
    assert_eq!(events[4]["action"], "config_rolled_back");
    assert!(
        events
            .windows(2)
            .all(|pair| { pair[0]["id"].as_i64() < pair[1]["id"].as_i64() })
    );
    assert_eq!(events[4]["config_version_id"], "version-one");
    assert_eq!(events[4]["replaced_config_version_id"], "version-two");
    assert!(!audit.to_string().contains("compiler"));
    assert!(!audit.to_string().contains("secret"));

    let reactivation = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/config-versions/version-two/publish")
                .insert_header(("If-Match", "rev-0")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(reactivation.status(), StatusCode::CONFLICT);
    Ok(())
}

#[actix_web::test]
async fn default_lifecycle_facade_is_fail_closed() -> TestResult {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(ManagementLifecycleHttpState::with_facade(
                Box::new(RejectingManagementLifecycleFacade),
            )))
            .configure(configure_management_lifecycle_resources),
    )
    .await;

    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/config-versions")).to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response_json(response).await?,
        json!({"error":{
            "code":"management_lifecycle_unavailable",
            "message":"Management lifecycle is unavailable"
        }})
    );
    Ok(())
}
