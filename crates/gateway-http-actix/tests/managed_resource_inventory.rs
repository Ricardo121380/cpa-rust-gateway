//! Managed resources remain visible before any runtime binding exists.
#![deny(unsafe_code)]
use actix_web::{App, http::StatusCode, test, web};
use gateway_control::{
    control_plane_service::credential_associated_data,
    management_mutation_service::{
        ManagementMutationService, MasterKey, MasterKeyRing, SecretStore,
    },
};
use gateway_core::{CredentialId, EndpointId, UpstreamId};
use gateway_http_actix::{
    management_resources::{ManagementResourceHttpState, configure_management_resources},
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use gateway_store::{
    control_plane::{
        ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
        CredentialConfiguration, CredentialStatus, EndpointConfiguration, EndpointTransport,
        SqliteControlPlaneRepository, UpstreamConfiguration,
    },
    secret_store::KeyVersion,
};
use serde_json::Value;
use std::{
    error::Error,
    net::SocketAddr,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

type TestResult = Result<(), Box<dyn Error>>;
const KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";
const VERSION: &str = "inventory-draft";
static NEXT_FILE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Database(PathBuf);
impl Drop for Database {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.0.display()));
        }
    }
}

fn fixture(active: bool) -> Result<(Database, ManagementResourceHttpState), Box<dyn Error>> {
    let name = format!(
        "prism-inventory-{}-{}-{}.sqlite3",
        std::process::id(),
        NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    let file = Database(std::env::temp_dir().join(name));
    let mut repository = SqliteControlPlaneRepository::open(&file.0)?;
    let key_version = KeyVersion::try_new(1)?;
    let store = SecretStore::new(MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x51; 32])?)],
    )?);
    let version = ConfigVersionId::try_new(VERSION)?;
    let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
        id: version.clone(),
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision: 0,
        created_at_ms: 1,
        description: "inventory".to_owned(),
    });
    for owner in ["owner-a", "owner-b"] {
        configuration.upstreams.push(UpstreamConfiguration {
            id: UpstreamId::try_new(owner)?,
            name: owner.to_owned(),
            kind: "openai-compatible".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: None,
        });
        configuration.endpoints.push(EndpointConfiguration {
            id: EndpointId::try_new(format!("endpoint-{owner}"))?,
            upstream_id: UpstreamId::try_new(owner)?,
            adapter_id: "openai-compatible.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: "https://example.test".to_owned(),
            inference_path: "/v1/responses".to_owned(),
            models_path: None,
            transport: EndpointTransport::Http,
            enabled: true,
        });
    }
    for index in 0..250 {
        let id = CredentialId::try_new(format!("account-{index:03}"))?;
        let upstream_id = UpstreamId::try_new(if index % 2 == 0 { "owner-a" } else { "owner-b" })?;
        let aad = credential_associated_data(&version, &id, &upstream_id)?;
        configuration.credentials.push(CredentialConfiguration {
            id,
            upstream_id,
            kind: "bearer".to_owned(),
            encrypted_secret: store.seal(b"inventory-secret-must-not-leak", &aad)?,
            status: if index % 2 == 0 {
                CredentialStatus::Active
            } else {
                CredentialStatus::Disabled
            },
            revision: 0,
        });
    }
    repository.write_configuration(&configuration)?;
    if active {
        repository.activate_version(&version)?;
    }
    Ok((
        file,
        ManagementResourceHttpState::with_workflow(
            ManagementMutationService::new(repository, store),
            Box::new(gateway_http_actix::management_resources::CodexOAuthManagementWorkflow::new()),
        ),
    ))
}

fn security() -> Result<ManagementHttpState, Box<dyn Error>> {
    Ok(ManagementHttpState::new(
        ManagementKey::try_new(KEY)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?)
}
fn authorized(request: test::TestRequest) -> test::TestRequest {
    request
        .peer_addr(SocketAddr::from(([127, 0, 0, 1], 41001)))
        .insert_header((MANAGEMENT_KEY_HEADER, KEY))
        .insert_header(("X-Config-Version", VERSION))
}

#[actix_web::test]
async fn unbound_inventory_is_complete_filtered_bounded_and_secret_free() -> TestResult {
    let (_file, state) = fixture(false)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_resources),
    )
    .await;
    let denied = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/credentials")
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);
    let mut seen = Vec::new();
    let mut uri = "/admin/credentials?limit=100".to_owned();
    loop {
        let response = test::call_service(
            &app,
            authorized(test::TestRequest::get().uri(&uri)).to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        let text = std::str::from_utf8(&body)?;
        assert!(!text.contains("inventory-secret-must-not-leak") && !text.contains("ciphertext"));
        let value: Value = serde_json::from_slice(&body)?;
        let rows = value["items"].as_array().ok_or("missing items")?;
        assert!(rows.len() <= 100);
        for row in rows {
            assert_eq!(row["binding_count"], 0);
            assert_eq!(row["credential"]["secret_present"], true);
            seen.push(row["credential"]["id"].as_str().ok_or("id")?.to_owned());
        }
        match value["next_cursor"].as_str() {
            Some(cursor) => uri = format!("/admin/credentials?limit=100&cursor={cursor}"),
            None => break,
        }
    }
    assert_eq!(seen.len(), 250);
    assert!(seen.windows(2).all(|items| items[0] < items[1]));
    let filtered = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/credentials?upstream_id=owner-b&q=account-00"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(filtered.status(), StatusCode::OK);
    let value: Value = test::read_body_json(filtered).await;
    assert_eq!(value["items"].as_array().ok_or("items")?.len(), 5);
    assert!(
        value["items"]
            .as_array()
            .ok_or("items")?
            .iter()
            .all(|row| row["credential"]["status"] == "disabled")
    );
    let endpoints = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/endpoints")).to_request(),
    )
    .await;
    assert_eq!(endpoints.status(), StatusCode::OK);
    let value: Value = test::read_body_json(endpoints).await;
    assert_eq!(value["items"].as_array().ok_or("endpoints")?.len(), 2);
    Ok(())
}

#[actix_web::test]
async fn inventory_cursor_rejects_filter_changes_and_same_revision_audit_changes() -> TestResult {
    let (_file, state) = fixture(false)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_resources),
    )
    .await;
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/credentials?limit=1")).to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let first: Value = test::read_body_json(response).await;
    let cursor = first["next_cursor"].as_str().ok_or("cursor")?;
    for uri in [
        format!("/admin/endpoints?cursor={cursor}"),
        format!("/admin/credentials?q=changed&cursor={cursor}"),
    ] {
        let response = test::call_service(
            &app,
            authorized(test::TestRequest::get().uri(&uri)).to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
    for uri in [
        "/admin/credentials?limit=101",
        "/admin/credentials?limit=0",
        "/admin/credentials?limit=1&limit=2",
        "/admin/credentials?cursor=!",
    ] {
        let response = test::call_service(
            &app,
            authorized(test::TestRequest::get().uri(uri)).to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    // Cancellation records resource audit without advancing the graph revision, like active OAuth rotation.
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::post().uri("/admin/credentials/account-000/oauth/cancel"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri(&format!("/admin/credentials?cursor={cursor}")))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/credentials?limit=1")).to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let second: Value = test::read_body_json(response).await;
    assert_eq!(first["revision"], second["revision"]);
    assert_ne!(first["observation_version"], second["observation_version"]);
    Ok(())
}

#[actix_web::test]
async fn active_fork_returns_a_complete_draft_without_secret_material() -> TestResult {
    let (_file, state) = fixture(true)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_resources),
    )
    .await;
    let path = format!("/admin/config-versions/{VERSION}/fork");
    let response = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri(&path)
                .insert_header(("If-Match", "rev-0"))
                .set_json(serde_json::json!({"id":"editable-copy", "description":"edit accounts"})),
        )
        .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let version: Value = test::read_body_json(response).await;
    assert_eq!(version["id"], "editable-copy");
    assert_eq!(version["parent_id"], VERSION);
    assert_eq!(version["status"], "draft");
    assert_eq!(version["revision"], "rev-0");
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/credentials?limit=100"))
            .insert_header(("X-Config-Version", "editable-copy"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    assert!(!std::str::from_utf8(&body)?.contains("inventory-secret-must-not-leak"));
    let copied: Value = serde_json::from_slice(&body)?;
    assert_eq!(copied["items"].as_array().ok_or("items")?.len(), 100);
    assert!(copied["next_cursor"].is_string());
    let stale = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri(&path)
                .insert_header(("If-Match", "rev-99"))
                .set_json(serde_json::json!({"id":"stale-copy", "description":""})),
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    Ok(())
}

#[actix_web::test]
async fn forked_credentials_have_independent_oauth_sessions() -> TestResult {
    let (_file, state) = fixture(true)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_resources),
    )
    .await;
    let fork = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri(&format!("/admin/config-versions/{VERSION}/fork"))
                .insert_header(("If-Match", "rev-0"))
                .set_json(serde_json::json!({"id":"oauth-edit", "description":""})),
        )
        .to_request(),
    )
    .await;
    assert_eq!(fork.status(), StatusCode::CREATED);
    let path = "/admin/credentials/account-000/oauth/start";
    let first = test::call_service(
        &app,
        authorized(test::TestRequest::post().uri(path)).to_request(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    let first: Value = test::read_body_json(first).await;
    let second = test::call_service(
        &app,
        authorized(test::TestRequest::post().uri(path))
            .insert_header(("X-Config-Version", "oauth-edit"))
            .to_request(),
    )
    .await;
    assert_eq!(second.status(), StatusCode::ACCEPTED);
    let second: Value = test::read_body_json(second).await;
    assert_eq!(first["credential_id"], second["credential_id"]);
    assert_ne!(first["authorization_url"], second["authorization_url"]);
    let cancel = test::call_service(
        &app,
        authorized(test::TestRequest::post().uri("/admin/credentials/account-000/oauth/cancel"))
            .insert_header(("X-Config-Version", "oauth-edit"))
            .to_request(),
    )
    .await;
    assert_eq!(cancel.status(), StatusCode::NO_CONTENT);
    let status = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/credentials/account-000/oauth/status"))
            .to_request(),
    )
    .await;
    let status: Value = test::read_body_json(status).await;
    assert_eq!(status["state"], "pending");
    Ok(())
}
