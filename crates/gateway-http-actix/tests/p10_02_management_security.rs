//! P10-02 management security-shell regression tests.

#![deny(unsafe_code)]

use std::{error::Error, net::SocketAddr};

use actix_web::{
    App, HttpMessage, HttpRequest, HttpResponse,
    http::{StatusCode, header},
    test, web,
};
use gateway_http_actix::management_security::{
    MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementCsrfToken, ManagementHttpState,
    ManagementKey, ManagementNetworkPolicy, ManagementOrigin, ManagementRequestPrincipal,
    configure_management,
};

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";
const CSRF_TOKEN: &str = "csrf_0123456789abcdefghijklmnopqrstuvwxyz";
const DENIED_BODY: &[u8] =
    br#"{"error":{"code":"management_access_denied","message":"Management access is unavailable"}}"#;

fn state(
    network_policy: ManagementNetworkPolicy,
    browser_policy: ManagementBrowserPolicy,
) -> Result<ManagementHttpState, Box<dyn Error>> {
    Ok(ManagementHttpState::new(
        ManagementKey::try_new(MANAGEMENT_KEY)?,
        network_policy,
        browser_policy,
    )?)
}

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 40_123))
}

fn socket(value: &str) -> Result<SocketAddr, Box<dyn Error>> {
    Ok(value.parse()?)
}

fn protected_routes(config: &mut web::ServiceConfig) {
    config
        .route("/probe", web::get().to(probe))
        .route("/mutate", web::post().to(probe));
}

async fn probe(request: HttpRequest) -> HttpResponse {
    let Some(principal) = request
        .extensions()
        .get::<ManagementRequestPrincipal>()
        .cloned()
    else {
        return HttpResponse::InternalServerError().finish();
    };
    HttpResponse::Ok()
        .insert_header(("x-test-audit-actor", principal.actor().as_str()))
        .finish()
}

fn management_key(request: test::TestRequest) -> test::TestRequest {
    request.insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
}

async fn assert_denied(response: actix_web::dev::ServiceResponse) {
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    assert!(response.headers().get(header::WWW_AUTHENTICATE).is_none());
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none()
    );
    assert_eq!(test::read_body(response).await.as_ref(), DENIED_BODY);
}

#[actix_web::test]
async fn management_key_is_separate_duplicate_safe_and_yields_a_safe_audit_identity() -> TestResult
{
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state(
                ManagementNetworkPolicy::LoopbackOnly,
                ManagementBrowserPolicy::DenyBrowserOrigins,
            )?))
            .configure(|config| configure_management(config, protected_routes)),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/probe")
            .peer_addr(loopback())
            .to_request(),
    )
    .await;
    assert_denied(response).await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/probe")
            .peer_addr(loopback())
            .insert_header((header::AUTHORIZATION, format!("Bearer {MANAGEMENT_KEY}")))
            .to_request(),
    )
    .await;
    assert_denied(response).await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/probe")
            .peer_addr(loopback())
            .insert_header(("x-api-key", MANAGEMENT_KEY))
            .to_request(),
    )
    .await;
    assert_denied(response).await;

    let response = test::call_service(
        &app,
        management_key(
            test::TestRequest::get()
                .uri("/admin/probe")
                .peer_addr(loopback()),
        )
        .append_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
        .to_request(),
    )
    .await;
    assert_denied(response).await;

    let response = test::call_service(
        &app,
        management_key(
            test::TestRequest::get()
                .uri("/admin/probe")
                .peer_addr(loopback()),
        )
        .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("x-test-audit-actor"),
        Some(&header::HeaderValue::from_static("management-key"))
    );
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none()
    );
    Ok(())
}

#[actix_web::test]
async fn peer_address_is_fail_closed_and_forwarded_headers_do_not_change_admission() -> TestResult {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state(
                ManagementNetworkPolicy::LoopbackOrPrivate,
                ManagementBrowserPolicy::DenyBrowserOrigins,
            )?))
            .configure(|config| configure_management(config, protected_routes)),
    )
    .await;

    for admitted in [
        "10.7.0.9:40123",
        "172.16.0.9:40123",
        "192.168.1.9:40123",
        "[fc00::9]:40123",
    ] {
        let response = test::call_service(
            &app,
            management_key(
                test::TestRequest::get()
                    .uri("/admin/probe")
                    .peer_addr(socket(admitted)?),
            )
            .to_request(),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "expected {admitted} to be admitted"
        );
    }

    for rejected in [
        "8.8.8.8:40123",
        "169.254.1.9:40123",
        "100.64.1.9:40123",
        "[fe80::9]:40123",
    ] {
        let response = test::call_service(
            &app,
            management_key(
                test::TestRequest::get()
                    .uri("/admin/probe")
                    .peer_addr(socket(rejected)?)
                    .insert_header(("x-forwarded-for", "127.0.0.1")),
            )
            .to_request(),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "expected {rejected} to be rejected"
        );
    }

    let response = test::call_service(
        &app,
        management_key(
            test::TestRequest::get()
                .uri("/admin/probe")
                .peer_addr(loopback())
                .insert_header(("x-forwarded-for", "198.51.100.7")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = test::call_service(
        &app,
        management_key(test::TestRequest::get().uri("/admin/probe")).to_request(),
    )
    .await;
    assert_denied(response).await;
    Ok(())
}

#[actix_web::test]
async fn browser_requests_are_default_denied_and_same_origin_mutations_require_csrf() -> TestResult
{
    let browser_policy = ManagementBrowserPolicy::SameOrigin {
        origin: ManagementOrigin::try_new("https://admin.example.test")?,
        csrf_token: ManagementCsrfToken::try_new(CSRF_TOKEN)?,
    };
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state(
                ManagementNetworkPolicy::LoopbackOnly,
                browser_policy,
            )?))
            .configure(|config| configure_management(config, protected_routes)),
    )
    .await;

    let response = test::call_service(
        &app,
        management_key(
            test::TestRequest::get()
                .uri("/admin/probe")
                .peer_addr(loopback())
                .insert_header((header::ORIGIN, "https://attacker.example.test")),
        )
        .to_request(),
    )
    .await;
    assert_denied(response).await;

    let response = test::call_service(
        &app,
        management_key(
            test::TestRequest::get()
                .uri("/admin/probe")
                .peer_addr(loopback())
                .insert_header((header::ORIGIN, "https://admin.example.test")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none()
    );

    for token in [
        None,
        Some("csrf_wrong-value-that-is-long-enough-0123456789"),
    ] {
        let request = management_key(
            test::TestRequest::post()
                .uri("/admin/mutate")
                .peer_addr(loopback())
                .insert_header((header::ORIGIN, "https://admin.example.test")),
        );
        let request = match token {
            Some(token) => request.insert_header(("x-management-csrf-token", token)),
            None => request,
        };
        let response = test::call_service(&app, request.to_request()).await;
        assert_denied(response).await;
    }

    let response = test::call_service(
        &app,
        management_key(
            test::TestRequest::post()
                .uri("/admin/mutate")
                .peer_addr(loopback())
                .insert_header((header::ORIGIN, "https://admin.example.test"))
                .insert_header(("x-management-csrf-token", CSRF_TOKEN)),
        )
        .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

#[actix_web::test]
async fn missing_state_and_secret_configuration_fail_closed_without_rendering_values() -> TestResult
{
    let app = test::init_service(
        App::new().configure(|config| configure_management(config, protected_routes)),
    )
    .await;
    let response = test::call_service(
        &app,
        management_key(
            test::TestRequest::get()
                .uri("/admin/probe")
                .peer_addr(loopback()),
        )
        .to_request(),
    )
    .await;
    assert_denied(response).await;

    assert!(ManagementKey::try_new("client_key_is_not_a_management_namespace_012345").is_err());
    assert!(ManagementCsrfToken::try_new("wrong_prefix_0123456789abcdefghijklmnopqrstuv").is_err());
    assert!(ManagementOrigin::try_new("https://admin.example.test/path").is_err());
    assert!(ManagementOrigin::try_new("https://admin.example.test/").is_err());
    let key = ManagementKey::try_new(MANAGEMENT_KEY)?;
    let csrf = ManagementCsrfToken::try_new(CSRF_TOKEN)?;
    assert!(!format!("{key:?} {csrf:?}").contains(MANAGEMENT_KEY));
    assert!(!format!("{key:?} {csrf:?}").contains(CSRF_TOKEN));
    Ok(())
}
