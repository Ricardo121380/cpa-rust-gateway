//! P10-08 protected encrypted-backup and empty-target restore HTTP regression tests.

#![deny(unsafe_code)]

use std::{
    error::Error,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use actix_web::{
    App,
    http::{StatusCode, header},
    test, web,
};
use gateway_control::management_backup_service::ManagementBackupService;
use gateway_http_actix::{
    management_backup_resources::{
        ManagementBackupHttpState, configure_management_backup_resources,
    },
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use gateway_store::backup::BackupKey;
use gateway_store::{migrate, open, schema_version};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Result<Self, Box<dyn Error>> {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cpa-rust-gateway-p10-08-http-{elapsed}-{}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn join(&self, leaf: &str) -> PathBuf {
        self.0.join(leaf)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 44_408))
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

fn key() -> Result<BackupKey, Box<dyn Error>> {
    Ok(BackupKey::try_from_bytes([0xA5; 32])?)
}

fn populated_database(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut connection = open(path)?;
    migrate(&mut connection)?;
    connection.execute(
        "INSERT INTO config_versions (id, parent_id, status, created_at_ms, description) \
         VALUES (?1, NULL, 'draft', 1, 'management backup source')",
        ["source-version"],
    )?;
    Ok(())
}

fn configured_service(
    directory: &TemporaryDirectory,
    source: PathBuf,
    target: PathBuf,
) -> Result<ManagementBackupService, Box<dyn Error>> {
    Ok(ManagementBackupService::try_new(
        source,
        target,
        directory.path(),
        key()?,
    )?)
}

async fn response_json(response: actix_web::dev::ServiceResponse) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&test::read_body(response).await)?)
}

#[actix_web::test]
async fn backup_preflight_and_binary_restore_are_protected_bounded_and_complete() -> TestResult {
    let directory = TemporaryDirectory::new()?;
    let source = directory.join("source.sqlite3");
    let target = directory.join("recovered.sqlite3");
    populated_database(&source)?;

    let artifact_service = configured_service(
        &directory,
        source.clone(),
        directory.join("not-used.sqlite3"),
    )?;
    let artifact = artifact_service.create_operator_artifact()?;
    let service = configured_service(&directory, source.clone(), target.clone())?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(ManagementBackupHttpState::new(service)))
            .configure(configure_management_backup_resources),
    )
    .await;

    let denied = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/admin/backups/preflight")
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);

    let source_preflight = test::call_service(
        &app,
        authorized(test::TestRequest::post().uri("/admin/backups/preflight")).to_request(),
    )
    .await;
    assert_eq!(source_preflight.status(), StatusCode::OK);
    assert_eq!(
        response_json(source_preflight).await?,
        json!({"schema_version":9,"secret_key_required":true})
    );

    let restore_preflight = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/restores/preflight")
                .insert_header((header::CONTENT_TYPE, "application/octet-stream"))
                .set_payload(artifact.clone()),
        )
        .to_request(),
    )
    .await;
    assert_eq!(restore_preflight.status(), StatusCode::OK);
    assert_eq!(
        response_json(restore_preflight).await?,
        json!({"schema_version":9,"quick_check_required":true,"compatible":true})
    );

    let restored = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/restores")
                .insert_header((header::CONTENT_TYPE, "application/octet-stream"))
                .set_payload(artifact),
        )
        .to_request(),
    )
    .await;
    assert_eq!(restored.status(), StatusCode::ACCEPTED);
    assert_eq!(response_json(restored).await?, json!({"state":"complete"}));

    let restored_connection = open(&target)?;
    assert_eq!(schema_version(&restored_connection)?, Some(9));
    let description: String = restored_connection.query_row(
        "SELECT description FROM config_versions WHERE id = ?1",
        ["source-version"],
        |row| row.get(0),
    )?;
    assert_eq!(description, "management backup source");
    Ok(())
}

#[actix_web::test]
async fn backup_endpoints_fail_closed_for_missing_content_type_bad_material_and_existing_targets()
-> TestResult {
    let directory = TemporaryDirectory::new()?;
    let source = directory.join("source.sqlite3");
    let target = directory.join("existing.sqlite3");
    populated_database(&source)?;
    let artifact_service = configured_service(
        &directory,
        source.clone(),
        directory.join("not-used.sqlite3"),
    )?;
    let artifact = artifact_service.create_operator_artifact()?;
    fs::write(&target, b"existing target must survive")?;

    let service = configured_service(&directory, source, target.clone())?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(ManagementBackupHttpState::new(service)))
            .configure(configure_management_backup_resources),
    )
    .await;

    let missing_content_type = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/restores/preflight")
                .set_payload(artifact.clone()),
        )
        .to_request(),
    )
    .await;
    assert_eq!(missing_content_type.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(missing_content_type).await?,
        json!({"error":{"code":"invalid_management_request","message":"Management request is invalid"}})
    );

    let malformed = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/restores/preflight")
                .insert_header((header::CONTENT_TYPE, "application/octet-stream"))
                .set_payload(vec![0_u8; 17]),
        )
        .to_request(),
    )
    .await;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(malformed).await?,
        json!({"error":{"code":"invalid_backup_material","message":"Backup material is invalid"}})
    );

    let existing_target = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/restores")
                .insert_header((header::CONTENT_TYPE, "application/octet-stream"))
                .set_payload(artifact),
        )
        .to_request(),
    )
    .await;
    assert_eq!(existing_target.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(existing_target).await?,
        json!({"error":{"code":"restore_target_unavailable","message":"Restore cannot create the configured empty target"}})
    );
    assert_eq!(fs::read(&target)?, b"existing target must survive");
    Ok(())
}
