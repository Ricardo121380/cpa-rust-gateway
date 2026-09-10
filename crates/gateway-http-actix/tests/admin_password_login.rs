//! Real Actix admission and durable single-administrator lifecycle, with synthetic passwords.
use actix_web::{App, HttpResponse, http::StatusCode, test, web};
use gateway_auth::admin_password::{hash_password, valid_hash};
use gateway_control::admin_login::AdminLoginService;
use gateway_http_actix::{
    configure_management_listener,
    management_security::{
        ManagementBrowserPolicy, ManagementCsrfToken, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy, ManagementOrigin, configure_management,
    },
};
use gateway_store::admin_login::{ADMIN_DATABASE_FILE, AdminAccountStore};
use serde_json::{Value, json};
use std::{
    error::Error,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

type TestResult = Result<(), Box<dyn Error>>;
const ORIGIN: &str = "https://prism.example.test";
const INITIAL: &str = "initial-synthetic-passphrase";
const NEW: &str = "changed-synthetic-passphrase";
static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Result<Self, Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!(
            "prism-admin-login-{}-{}-{}",
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn state(store: AdminAccountStore) -> Result<ManagementHttpState, Box<dyn Error>> {
    Ok(ManagementHttpState::new(
        ManagementKey::try_new(format!("mgmt_{}", "m".repeat(40)))?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::SameOrigin {
            origin: ManagementOrigin::try_new(ORIGIN)?,
            csrf_token: ManagementCsrfToken::try_new(format!("csrf_{}", "c".repeat(40)))?,
        },
    )?
    .with_admin_login(AdminLoginService::new(store)?))
}
fn request(method: &str, path: &str) -> test::TestRequest {
    test::TestRequest::default()
        .method(method.parse().unwrap_or(actix_web::http::Method::GET))
        .uri(path)
        .peer_addr(([127, 0, 0, 1], 31000).into())
        .insert_header(("Origin", ORIGIN))
}
fn session_request(method: &str, path: &str, grant: &Value) -> test::TestRequest {
    request(method, path)
        .insert_header((
            "X-Management-Key",
            grant["session_token"].as_str().unwrap_or_default(),
        ))
        .insert_header((
            "X-Management-CSRF-Token",
            grant["csrf_token"].as_str().unwrap_or_default(),
        ))
}
#[actix_web::test]
async fn first_password_change_is_durable_and_revokes_every_session() -> TestResult {
    let temp = Temp::new()?;
    let path = temp.0.join(ADMIN_DATABASE_FILE);
    let store = AdminAccountStore::initialize(&path, "admin", &hash_password(INITIAL)?)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state(store.clone())?))
            .configure(configure_management_listener),
    )
    .await;
    let login = || {
        request("POST", "/admin/auth/login")
            .set_json(json!({"username":"admin","password":INITIAL}))
            .to_request()
    };
    let response = test::call_service(&app, login()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
    let grant: Value = test::read_body_json(response).await;
    assert_eq!(grant["password_change_required"], true);
    let second: Value = test::call_and_read_body_json(&app, login()).await;
    assert_ne!(grant["session_token"], second["session_token"]);
    assert_eq!(
        test::call_service(
            &app,
            session_request("GET", "/admin/config-versions", &grant).to_request()
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let change = |current: &str, new: &str| {
        session_request("POST", "/admin/auth/password", &grant)
            .set_json(json!({"current_password":current,"new_password":new}))
            .to_request()
    };
    assert_eq!(
        test::call_service(&app, change(INITIAL, INITIAL))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        test::call_service(&app, change("wrong", NEW))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        test::call_service(&app, change(INITIAL, NEW))
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    for stale in [&grant, &second] {
        assert_eq!(
            test::call_service(
                &app,
                session_request("POST", "/admin/auth/logout", stale).to_request()
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        test::call_service(&app, login()).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let account = store.load()?;
    assert!(!account.password_change_required);
    assert_eq!(account.revision, 2);
    assert!(valid_hash(&account.password_hash));
    assert!(!account.password_hash.contains(NEW));
    // Restart reconstructs from the persisted hash; old memory sessions are absent.
    let service = AdminLoginService::new(AdminAccountStore::open(path)?)?;
    let fresh = service.login("admin", NEW)?;
    assert!(!fresh.password_change_required);
    assert!(
        service
            .authenticate(&fresh.session_token, Some(&fresh.csrf_token), true)
            .is_ok()
    );
    service.logout(&fresh.session_token)?;
    assert!(
        service
            .authenticate(&fresh.session_token, None, false)
            .is_err()
    );
    Ok(())
}
#[actix_web::test]
async fn login_origin_peer_input_and_rate_boundaries_are_enforced() -> TestResult {
    let temp = Temp::new()?;
    let store = AdminAccountStore::initialize(
        temp.0.join(ADMIN_DATABASE_FILE),
        "admin",
        &hash_password(INITIAL)?,
    )?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state(store)?))
            .configure(configure_management_listener),
    )
    .await;
    let body = json!({"username":"admin","password":INITIAL});
    for probe in [
        test::TestRequest::post()
            .uri("/admin/auth/login")
            .peer_addr(([127, 0, 0, 1], 31000).into()),
        request("POST", "/admin/auth/login").insert_header(("Origin", "https://elsewhere.test")),
        request("POST", "/admin/auth/login")
            .peer_addr(([203, 0, 113, 3], 31000).into())
            .insert_header(("X-Forwarded-For", "127.0.0.1")),
        request("POST", "/admin/auth/login").append_header(("Origin", ORIGIN)),
    ] {
        assert_eq!(
            test::call_service(&app, probe.set_json(&body).to_request())
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        test::call_service(
            &app,
            request("POST", "/admin/auth/login")
                .set_json(json!({"username":"admin","password":"x".repeat(3000)}))
                .to_request()
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        test::call_service(
            &app,
            request("POST", "/admin/auth/login")
                .set_payload("username=admin")
                .to_request()
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    for index in 0..10 {
        let response=test::call_service(&app,request("POST","/admin/auth/login").set_json(json!({"username":if index%2==0 {"admin"}else{"unknown"},"password":"wrong-password"})).to_request()).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let error: Value = test::read_body_json(response).await;
        assert_eq!(
            error["error"]["code"],
            "management_login_invalid_credentials"
        );
    }
    let response = test::call_service(
        &app,
        request("POST", "/admin/auth/login")
            .set_json(body)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok()),
        Some("60")
    );
    Ok(())
}
#[actix_web::test]
async fn ordinary_sessions_require_csrf_and_legacy_cli_stays_compatible() -> TestResult {
    let temp = Temp::new()?;
    let store = AdminAccountStore::initialize(
        temp.0.join(ADMIN_DATABASE_FILE),
        "admin",
        &hash_password(INITIAL)?,
    )?;
    store.change_password(1, &hash_password(NEW)?)?;
    let security = state(store)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security))
            .configure(configure_management_listener),
    )
    .await;
    let grant: Value = test::call_and_read_body_json(
        &app,
        request("POST", "/admin/auth/login")
            .set_json(json!({"username":"admin","password":NEW}))
            .to_request(),
    )
    .await;
    assert_eq!(grant["password_change_required"], false);
    for probe in [
        request("POST", "/admin/auth/logout").insert_header((
            "X-Management-Key",
            grant["session_token"].as_str().unwrap_or_default(),
        )),
        session_request("POST", "/admin/auth/logout", &grant)
            .insert_header(("X-Management-CSRF-Token", "wrong")),
        session_request("POST", "/admin/auth/logout", &grant)
            .insert_header(("Origin", "https://elsewhere.test")),
        test::TestRequest::post()
            .uri("/admin/auth/logout")
            .peer_addr(([127, 0, 0, 1], 31000).into())
            .insert_header((
                "X-Management-Key",
                grant["session_token"].as_str().unwrap_or_default(),
            )),
    ] {
        assert_eq!(
            test::call_service(&app, probe.to_request()).await.status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        test::call_service(
            &app,
            session_request("POST", "/admin/auth/logout", &grant).to_request()
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    let store = AdminAccountStore::open(temp.0.join(ADMIN_DATABASE_FILE))?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state(store)?))
            .configure(|cfg| {
                configure_management(cfg, |protected| {
                    protected.route(
                        "/probe",
                        web::post().to(|| async { HttpResponse::Ok().finish() }),
                    );
                });
            }),
    )
    .await;
    let cli = test::TestRequest::post()
        .uri("/admin/probe")
        .peer_addr(([127, 0, 0, 1], 31000).into())
        .insert_header(("X-Management-Key", format!("mgmt_{}", "m".repeat(40))))
        .to_request();
    assert_eq!(test::call_service(&app, cli).await.status(), StatusCode::OK);
    Ok(())
}
