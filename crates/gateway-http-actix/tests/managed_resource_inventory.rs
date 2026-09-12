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

#[actix_web::test]
async fn deleting_provider_clears_only_its_current_catalog_targets() -> TestResult {
    let (file, state) = fixture(false)?;
    let db = gateway_store::open(&file.0)?;
    for (owner, account) in [("owner-a", "account-000"), ("owner-b", "account-001")] {
        db.execute(
            "INSERT INTO model_catalog_targets VALUES (?1,?2,?3,1,1000,2000,3000,4000)",
            (VERSION, format!("endpoint-{owner}"), account),
        )?;
        db.execute(
            "INSERT INTO model_catalog_models VALUES (?1,?2,?3,'known-model',1,0,NULL,NULL)",
            (VERSION, format!("endpoint-{owner}"), account),
        )?;
    }
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_resources),
    )
    .await;
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::delete().uri("/admin/upstreams/owner-a"))
            .insert_header(("If-Match", "rev-0"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let remaining: String =
        db.query_row("SELECT endpoint_id FROM model_catalog_targets", [], |row| {
            row.get(0)
        })?;
    assert_eq!(remaining, "endpoint-owner-b");
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM model_catalog_models", [], |row| row
            .get::<_, i64>(
            0
        ))?,
        1
    );
    Ok(())
}

#[actix_web::test]
async fn system_information_is_authenticated_and_contains_only_explicit_build_fields() -> TestResult
{
    use gateway_http_actix::management_resources::ManagementSystemInformation;
    let (_file, state) = fixture(false)?;
    let state = state.with_system_information(ManagementSystemInformation {
        version: "0.1.0",
        build_revision: "development",
        build_target: "development",
        rust_version: "development",
        schema_version: gateway_store::CURRENT_SCHEMA_VERSION,
    });
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_resources),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::get().uri("/admin/system").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/system")).to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["configuration_application"], "live");
    assert_eq!(
        body["build"]["schema_version"],
        gateway_store::CURRENT_SCHEMA_VERSION
    );
    assert_eq!(body["build"].as_object().ok_or("build")?.len(), 5);
    assert!(body["uptime_seconds"].is_u64());
    assert!(
        body["accepting_requests"].is_null(),
        "a facade without a runtime observer must not claim readiness"
    );
    assert!(!body.to_string().contains("inventory-secret"));
    Ok(())
}
struct SessionIdentityFixture(std::sync::Arc<std::sync::atomic::AtomicUsize>);
struct RefusedSessionIdentity;
impl provider_grok::GrokSessionIdentityTransport for RefusedSessionIdentity {
    fn fetch(
        &self,
        _: provider_grok::GrokSessionIdentityRequest,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        gateway_store::account_identity::AccountIdentity,
                        provider_grok::GrokSessionIdentityError,
                    >,
                > + Send
                + '_,
        >,
    > {
        Box::pin(async { Err(provider_grok::GrokSessionIdentityError::Forbidden) })
    }
}
impl provider_grok::GrokSessionIdentityTransport for SessionIdentityFixture {
    fn fetch(
        &self,
        request: provider_grok::GrokSessionIdentityRequest,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        gateway_store::account_identity::AccountIdentity,
                        provider_grok::GrokSessionIdentityError,
                    >,
                > + Send
                + '_,
        >,
    > {
        assert!(request.cookie().contains("synthetic-"));
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let different = request.cookie().contains("another-user");
        Box::pin(async move {
            provider_grok::parse_grok_session_identity(if different {
                br#"{"status":"authenticated","session":{"email":"different@example.test"}}"#
            } else {
                br#"{"status":"authenticated","session":{"userId":"fixture-subject","email":"sso.member@example.test"}}"#
            })
        })
    }
}
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
            encrypted_secret: store.seal(if index==0 {br#"{"email":"owner@example.test","access_token":"inventory-secret-must-not-leak","sub":"opaque-subject"}"#} else {b"inventory-secret-must-not-leak"}, &aad)?,
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
    let native =
        gateway_http_actix::management_resources::native_accounts::NativeAccountManagement::new(
            std::sync::Arc::new(provider_grok::GrokAccountPoolStore::try_open(
                &file.0,
                store.clone(),
            )?),
        )?;
    Ok((
        file,
        ManagementResourceHttpState::with_workflow(
            ManagementMutationService::new(repository, store),
            Box::new(gateway_http_actix::management_resources::CodexOAuthManagementWorkflow::new()),
        )
        .with_native_accounts(native),
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

struct RuntimeApplyFixture(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl gateway_http_actix::management_resources::native_accounts::NativeAccountRuntime
    for RuntimeApplyFixture
{
    fn apply(&self, _: Option<&str>) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }
}

#[actix_web::test]
#[allow(clippy::too_many_lines)]
async fn native_account_http_actions_are_guarded_audited_and_report_saved_vs_applied() -> TestResult
{
    let (file, resources) = fixture(false)?;
    let version = KeyVersion::try_new(1)?;
    let secrets = SecretStore::new(MasterKeyRing::try_new(
        version,
        [(version, MasterKey::try_from_bytes([0x51; 32])?)],
    )?);
    let ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let native =
        gateway_http_actix::management_resources::native_accounts::NativeAccountManagement::new(
            std::sync::Arc::new(provider_grok::GrokAccountPoolStore::try_open(
                &file.0, secrets,
            )?),
        )?
        .with_identity_transport(std::sync::Arc::new(SessionIdentityFixture(
            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        )))
        .with_runtime(std::sync::Arc::new(RuntimeApplyFixture(ready.clone())));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(resources.with_native_accounts(native)))
            .configure(configure_management_resources),
    )
    .await;
    let imported=test::call_service(&app,authorized(test::TestRequest::post().uri("/admin/native-accounts/import"))
        .set_json(serde_json::json!({"id":"account-actions","channel":"grok.console","secret":"synthetic-session"})).to_request()).await;
    assert_eq!(imported.status(), StatusCode::CREATED);
    let receipt: Value = test::read_body_json(imported).await;
    assert_eq!(receipt["runtime_applied"], false);
    let rows: Value = test::read_body_json(
        test::call_service(
            &app,
            authorized(test::TestRequest::get().uri("/admin/native-accounts")).to_request(),
        )
        .await,
    )
    .await;
    let id = rows["items"][0]["id"].as_str().ok_or("account")?;
    let path = format!("/admin/native-accounts/{id}");
    let denied = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&path)
            .set_json(serde_json::json!({"revision":0,"enabled":false}))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::patch().uri(&path))
            .set_json(serde_json::json!({"revision":0,"enabled":false}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let changed: Value = test::read_body_json(response).await;
    assert_eq!(changed["revision"], 1);
    assert_eq!(changed["runtime_applied"], false);
    let stale = test::call_service(
        &app,
        authorized(test::TestRequest::delete().uri(&format!("{path}?revision=0"))).to_request(),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    ready.store(true, std::sync::atomic::Ordering::Release);
    let applied: Value = test::read_body_json(
        test::call_service(
            &app,
            authorized(test::TestRequest::post().uri("/admin/operations/runtime/apply"))
                .to_request(),
        )
        .await,
    )
    .await;
    assert_eq!(applied["runtime_applied"], true);
    let mismatch = test::call_service(
        &app,
        authorized(test::TestRequest::put().uri(&format!("{path}/credential")))
            .set_json(serde_json::json!({"revision":1,"secret":"synthetic-another-user"}))
            .to_request(),
    )
    .await;
    assert_eq!(mismatch.status(), StatusCode::CONFLICT);
    let update = test::call_service(
        &app,
        authorized(test::TestRequest::put().uri(&format!("{path}/credential")))
            .set_json(serde_json::json!({"revision":1,"secret":"synthetic-updated-session"}))
            .to_request(),
    )
    .await;
    assert_eq!(update.status(), StatusCode::OK);
    let updated: Value = test::read_body_json(update).await;
    assert_eq!(updated["revision"], 2);
    assert_eq!(updated["runtime_applied"], true);
    let remove = test::call_service(
        &app,
        authorized(test::TestRequest::delete().uri(&format!("{path}?revision=2"))).to_request(),
    )
    .await;
    assert_eq!(remove.status(), StatusCode::OK);
    let audit: Value = test::read_body_json(
        test::call_service(
            &app,
            authorized(test::TestRequest::get().uri(&format!("{path}/audit"))).to_request(),
        )
        .await,
    )
    .await;
    assert_eq!(audit.as_array().ok_or("audit")?.len(), 3);
    assert_eq!(audit[0]["action"], "removed");
    assert!(!audit.to_string().contains("synthetic-session"));
    let missing = test::call_service(
        &app,
        authorized(test::TestRequest::delete().uri(&format!("{path}?revision=3"))).to_request(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    Ok(())
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
    let (file, state) = fixture(true)?;
    let db = gateway_store::open(&file.0)?;
    db.execute("INSERT INTO model_catalog_targets VALUES (?1,'endpoint-owner-a','account-000',7,1000,2000,3000,4000)",[VERSION])?;
    db.execute("INSERT INTO model_catalog_models VALUES (?1,'endpoint-owner-a','account-000','known-model',1,0,NULL,NULL)",[VERSION])?;
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
    let copied_deadline: i64 = db.query_row(
        "SELECT expires_at_ms FROM model_catalog_targets WHERE config_version_id='editable-copy'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        copied_deadline, 4000,
        "forking must not renew expired evidence"
    );
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::patch().uri("/admin/credentials/account-000/status"))
            .insert_header(("X-Config-Version", "editable-copy"))
            .insert_header(("If-Match", "rev-0"))
            .set_json(serde_json::json!({"status":"disabled","credential_revision":0}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM model_catalog_targets WHERE config_version_id='editable-copy'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    let response=test::call_service(&app,authorized(test::TestRequest::patch().uri("/admin/credentials/account-000"))
        .insert_header(("X-Config-Version","editable-copy")).insert_header(("If-Match","rev-1"))
        .set_json(serde_json::json!({"id":"account-000","kind":"bearer","status":"disabled","secret":"replacement-synthetic-key"})).to_request()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM model_catalog_targets WHERE config_version_id='editable-copy'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        0,
        "new authorization cannot retain old identity evidence"
    );
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM model_catalog_targets WHERE config_version_id=?1",
            [VERSION],
            |row| row.get::<_, i64>(0)
        )?,
        1,
        "original observation remains intact"
    );
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

#[actix_web::test]
async fn status_update_keeps_ciphertext_and_rejects_stale_account_revision() -> TestResult {
    let (file, state) = fixture(false)?;
    let version = ConfigVersionId::try_new(VERSION)?;
    let mut repository = SqliteControlPlaneRepository::open(&file.0)?;
    let before = repository
        .load_configuration(&version)?
        .ok_or("configuration")?
        .credentials
        .remove(0);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_resources),
    )
    .await;
    let uri = format!("/admin/credentials/{}/status", before.id.as_str());
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::patch().uri(&uri))
            .insert_header(("If-Match", "rev-0"))
            .set_json(serde_json::json!({"status":"disabled", "credential_revision":0}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let result: Value = test::read_body_json(response).await;
    assert_eq!(result["status"], "disabled");
    assert_eq!(result["revision"], 1);
    assert!(result.get("secret").is_none());
    let after = repository
        .load_configuration(&version)?
        .ok_or("configuration")?
        .credentials
        .remove(0);
    assert_eq!(
        before.encrypted_secret.ciphertext(),
        after.encrypted_secret.ciphertext()
    );
    assert_eq!(before.kind, after.kind);
    assert_eq!(before.upstream_id, after.upstream_id);
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::patch().uri(&uri))
            .insert_header(("If-Match", "rev-1"))
            .set_json(serde_json::json!({"status":"active", "credential_revision":0}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let after = repository
        .load_configuration(&version)?
        .ok_or("configuration")?
        .credentials
        .remove(0);
    assert_eq!(after.status, CredentialStatus::Disabled);
    assert_eq!(after.revision, 1);
    Ok(())
}

#[actix_web::test]
async fn channel_import_is_scoped_validated_and_visible_before_binding() -> TestResult {
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
        authorized(test::TestRequest::get().uri("/admin/account-channels")).to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let channels: Value = test::read_body_json(response).await;
    assert_eq!(channels.as_array().ok_or("channels")?.len(), 10);
    let response = test::call_service(&app, authorized(test::TestRequest::post().uri("/admin/upstreams/owner-a/account-import"))
        .insert_header(("If-Match", "rev-0"))
        .set_json(serde_json::json!({"id":"imported-codex", "channel":"codex", "secret":r#"{"kind":"codex_oauth","access_token":"synthetic-import-access","refresh_token":"synthetic-import-refresh","expires_at_ms":4102444800000,"account_id":"synthetic-account"}"#})).to_request()).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let result: Value = test::read_body_json(response).await;
    assert_eq!(result["kind"], "oauth_json");
    assert!(result.get("secret").is_none());
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/credentials?q=imported-codex"))
            .to_request(),
    )
    .await;
    let result: Value = test::read_body_json(response).await;
    assert_eq!(result["items"][0]["binding_count"], 0);
    assert_eq!(result["items"][0]["credential"]["id"], "imported-codex");
    for (channel, material) in [
        ("codex", "bad-json"),
        ("grok.build", "native-account-material"),
        ("kiro", "ksk_wrong-owner"),
    ] {
        let response = test::call_service(&app, authorized(test::TestRequest::post().uri("/admin/upstreams/owner-a/account-import"))
            .insert_header(("If-Match", "rev-1"))
            .set_json(serde_json::json!({"id":"must-not-exist", "channel":channel, "secret":material})).to_request()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    Ok(())
}

#[actix_web::test]
async fn repeated_channel_import_retains_disabled_account_and_connections() -> TestResult {
    let (_file, state) = fixture(false)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_resources),
    )
    .await;
    let material = "synthetic-kimi-api-key";
    for (id, revision, expected) in [
        ("first-kimi", 0, StatusCode::CREATED),
        ("second-label", 3, StatusCode::OK),
    ] {
        let response = test::call_service(
            &app,
            authorized(test::TestRequest::post().uri("/admin/upstreams/owner-a/account-import"))
                .insert_header(("If-Match", format!("rev-{revision}")))
                .set_json(serde_json::json!({"id":id,"channel":"kimi","secret":material}))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), expected);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["id"], "first-kimi");
        assert!(body.get("secret").is_none());
        if revision == 0 {
            let response = test::call_service(
                &app,
                authorized(test::TestRequest::patch().uri("/admin/credentials/first-kimi/status"))
                    .insert_header(("If-Match", "rev-1"))
                    .set_json(serde_json::json!({"status":"disabled","credential_revision":0}))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let response = test::call_service(&app, authorized(test::TestRequest::post().uri("/admin/endpoints/endpoint-owner-a/credential-bindings"))
                .insert_header(("If-Match","rev-2"))
                .set_json(serde_json::json!({"credential_id":"first-kimi","enabled":true,"priority":2,"weight":3,"concurrency":4})).to_request()).await;
            assert_eq!(response.status(), StatusCode::CREATED);
        } else {
            assert_eq!(body["status"], "disabled");
            assert_eq!(body["revision"], 1);
        }
    }
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/credentials?q=first-kimi")).to_request(),
    )
    .await;
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["items"][0]["binding_count"], 1);
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/credentials?q=second-label")).to_request(),
    )
    .await;
    let body: Value = test::read_body_json(response).await;
    assert!(body["items"].as_array().ok_or("items")?.is_empty());
    Ok(())
}

#[actix_web::test]
async fn native_import_is_encrypted_global_and_pagination_ignores_unrelated_writes() -> TestResult {
    let (_file, state) = fixture(false)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_resources),
    )
    .await;
    let now = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let materials = [
        (
            "grok.build",
            serde_json::json!({"access_token":"synthetic-build-access","refresh_token":"synthetic-refresh","expires_at":time::OffsetDateTime::from_unix_timestamp((now+600_000)/1000)?.format(&time::format_description::well_known::Rfc3339)?}),
        ),
        (
            "grok.console",
            serde_json::json!({"sso_token":"synthetic-console-sso","probe_model":"grok-4.6"}),
        ),
        (
            "grok.web",
            serde_json::json!({"kind":"grok_web_sso","account_ref":"synthetic-web","lineage_ref":"test-lineage","revision":1,"expires_at_ms":now+600_000,"cookies":[{"name":"sso","value":"synthetic-web-cookie","domain":"grok.com","path":"/","secure":true,"http_only":true}]}),
        ),
    ];
    for (channel, material) in materials {
        let response=test::call_service(&app,authorized(test::TestRequest::post().uri("/admin/native-accounts/import")).set_json(serde_json::json!({"id":channel,"channel":channel,"secret":material.to_string()})).to_request()).await;
        assert_eq!(response.status(), StatusCode::CREATED, "{channel}");
    }
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/native-accounts?limit=1")).to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let first: Value = test::read_body_json(response).await;
    let cursor = first["next_cursor"].as_str().ok_or("cursor")?;
    let response=test::call_service(&app,authorized(test::TestRequest::post().uri("/admin/upstreams/owner-a/credentials"))
        .insert_header(("If-Match","rev-0")).set_json(serde_json::json!({"id":"unrelated","kind":"bearer","secret":"synthetic-unrelated","status":"active"})).to_request()).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let response = test::call_service(
        &app,
        authorized(
            test::TestRequest::get()
                .uri(&format!("/admin/native-accounts?limit=1&cursor={cursor}")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = first.to_string();
    assert!(!body.contains("synthetic-build-access"));
    assert!(!body.contains("synthetic-console-sso"));
    let response=test::call_service(&app,authorized(test::TestRequest::post().uri("/admin/native-accounts/import")).set_json(serde_json::json!({"id":"another","channel":"grok.console","secret":"{\"sso_token\":\"synthetic-another\",\"probe_model\":\"grok-4.6\"}"})).to_request()).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let response = test::call_service(
        &app,
        authorized(
            test::TestRequest::get()
                .uri(&format!("/admin/native-accounts?limit=1&cursor={cursor}")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    Ok(())
}

#[actix_web::test]
async fn identity_projection_exposes_email_but_not_token_subject_or_random_labels() -> TestResult {
    let (_file, resources) = fixture(false)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(resources))
            .configure(configure_management_resources),
    )
    .await;
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/credentials?limit=1")).to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = test::read_body(response).await;
    let body: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(body["items"][0]["identity"]["email"], "owner@example.test");
    assert_eq!(body["items"][0]["category"], "api");
    assert!(body["items"][0]["identity"]["username"].is_null());
    assert!(!std::str::from_utf8(&bytes)?.contains("inventory-secret-must-not-leak"));
    assert!(!std::str::from_utf8(&bytes)?.contains("opaque-subject"));
    Ok(())
}

#[actix_web::test]
async fn native_build_import_captures_id_token_identity_and_lists_compact_credentials() -> TestResult
{
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let (file, resources) = fixture(false)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(resources))
            .configure(configure_management_resources),
    )
    .await;
    let now = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let claims = URL_SAFE_NO_PAD
        .encode(br#"{"email":"imported.member@example.test","sub":"provider-user"}"#);
    let material = serde_json::json!({"access_token":"fixture-token","refresh_token":"fixture-refresh","id_token":format!("header.{claims}.signature"),"expires_at":time::OffsetDateTime::from_unix_timestamp((now+600_000)/1000)?.format(&time::format_description::well_known::Rfc3339)?});
    let response=test::call_service(&app,authorized(test::TestRequest::post().uri("/admin/native-accounts/import")).set_json(serde_json::json!({"id":"autoreg-import-marker","channel":"grok.build","secret":material.to_string()})).to_request()).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/native-accounts")).to_request(),
    )
    .await;
    let bytes = test::read_body(response).await;
    let body: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(
        body["items"][0]["identity"]["email"],
        "imported.member@example.test"
    );
    assert!(!std::str::from_utf8(&bytes)?.contains("fixture-refresh"));
    assert!(!std::str::from_utf8(&bytes)?.contains("header."));
    let db = gateway_store::open(&file.0)?;
    let ciphertext: Vec<u8> = db.query_row(
        "SELECT credential_ciphertext FROM grok_accounts",
        [],
        |row| row.get(0),
    )?;
    assert!(
        !ciphertext
            .windows(b"imported.member".len())
            .any(|w| w == b"imported.member")
    );
    Ok(())
}

#[actix_web::test]
async fn sso_import_observes_identity_and_existing_account_refresh_is_revision_guarded()
-> TestResult {
    let (file, resources) = fixture(false)?;
    let version = KeyVersion::try_new(1)?;
    let secrets = SecretStore::new(MasterKeyRing::try_new(
        version,
        [(version, MasterKey::try_from_bytes([0x51; 32])?)],
    )?);
    let pool = std::sync::Arc::new(provider_grok::GrokAccountPoolStore::try_open(
        &file.0,
        secrets.clone(),
    )?);
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let native =
        gateway_http_actix::management_resources::native_accounts::NativeAccountManagement::new(
            pool,
        )?
        .with_identity_transport(std::sync::Arc::new(SessionIdentityFixture(
            std::sync::Arc::clone(&calls),
        )));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(resources.with_native_accounts(native)))
            .configure(configure_management_resources),
    )
    .await;
    let now = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let materials = [
        (
            "grok.console",
            serde_json::json!({"sso_token":"synthetic-console-session","probe_model":"grok-4.6"}),
        ),
        (
            "grok.web",
            serde_json::json!({"kind":"grok_web_sso","account_ref":"web-fixture","lineage_ref":"lineage-fixture","revision":1,"expires_at_ms":now+600_000,"cookies":[{"name":"sso","value":"synthetic-web-session","domain":"grok.com","path":"/","secure":true,"http_only":true}]}),
        ),
    ];
    for (channel, material) in materials {
        let response=test::call_service(&app,authorized(test::TestRequest::post().uri("/admin/native-accounts/import")).set_json(serde_json::json!({"id":channel,"channel":channel,"secret":material.to_string()})).to_request()).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["identity_state"], "observed");
    }
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 2);
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/native-accounts")).to_request(),
    )
    .await;
    let body: Value = test::read_body_json(response).await;
    let rows = body["items"].as_array().ok_or("items")?;
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|r| r["identity"]["email"] == "sso.member@example.test")
    );
    assert!(!body.to_string().contains("synthetic-console-session"));
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::Relaxed),
        2,
        "GET must not contact provider"
    );
    let id = rows[0]["id"].as_str().ok_or("id")?;
    let path = format!("/admin/native-accounts/{id}/identity");
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&path)
            .set_json(serde_json::json!({"revision":0}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::post().uri(&path))
            .set_json(serde_json::json!({"revision":1}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 2);
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::post().uri(&path))
            .set_json(serde_json::json!({"revision":0}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 3);
    let reopened = provider_grok::GrokAccountPoolStore::try_open(&file.0, secrets)?;
    assert_eq!(
        reopened.observed_identity(id, now)?.email.as_deref(),
        Some("sso.member@example.test")
    );
    Ok(())
}

#[actix_web::test]
async fn refused_identity_read_preserves_import_and_reports_the_provider_refusal() -> TestResult {
    let (file, resources) = fixture(false)?;
    let version = KeyVersion::try_new(1)?;
    let secrets = SecretStore::new(MasterKeyRing::try_new(
        version,
        [(version, MasterKey::try_from_bytes([0x51; 32])?)],
    )?);
    let pool = std::sync::Arc::new(provider_grok::GrokAccountPoolStore::try_open(
        &file.0, secrets,
    )?);
    let native =
        gateway_http_actix::management_resources::native_accounts::NativeAccountManagement::new(
            pool,
        )?
        .with_identity_transport(std::sync::Arc::new(RefusedSessionIdentity));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(resources.with_native_accounts(native)))
            .configure(configure_management_resources),
    )
    .await;
    let response=test::call_service(&app,authorized(test::TestRequest::post().uri("/admin/native-accounts/import")).set_json(serde_json::json!({"id":"refused-profile","channel":"grok.console","secret":"synthetic-sso"})).to_request()).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let imported: Value = test::read_body_json(response).await;
    assert_eq!(imported["identity_state"], "unavailable");
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/native-accounts")).to_request(),
    )
    .await;
    let rows: Value = test::read_body_json(response).await;
    assert_eq!(rows["items"][0]["auth_status"], "active");
    assert!(rows["items"][0]["identity"]["email"].is_null());
    let id = rows["items"][0]["id"].as_str().ok_or("id")?;
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::post().uri(&format!("/admin/native-accounts/{id}/identity")))
            .set_json(serde_json::json!({"revision":0}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let error: Value = test::read_body_json(response).await;
    assert_eq!(error["error"]["code"], "management_identity_rejected");
    assert!(
        error["error"]["message"]
            .as_str()
            .ok_or("message")?
            .contains("403")
    );
    Ok(())
}
