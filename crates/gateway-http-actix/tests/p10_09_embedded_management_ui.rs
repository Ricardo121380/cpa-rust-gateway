//! P10-09 embedded-management UI and data-plane isolation regression tests.

#![deny(unsafe_code)]

use std::{error::Error, time::Instant};

use actix_web::{
    App,
    http::{StatusCode, header},
    test,
};
use gateway_http_actix::{configure, management_ui_resources::configure_embedded_management_ui};

type TestResult = Result<(), Box<dyn Error>>;

const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; base-uri 'none'; connect-src 'self'; form-action 'none'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'; style-src 'self'";
const INDEX: &[u8] = include_bytes!("../../../web/prism/dist/index.html");
const STYLES: &[u8] = include_bytes!("../../../web/prism/dist/assets/index.css");
const APPLICATION: &[u8] = include_bytes!("../../../web/prism/dist/assets/main.js");
const VENDOR: &[u8] = include_bytes!("../../../web/prism/dist/assets/vendor.js");

#[actix_web::test]
async fn embedded_management_ui_serves_only_exact_reviewed_assets_with_hardened_headers()
-> TestResult {
    let app = test::init_service(App::new().configure(configure_embedded_management_ui)).await;

    let redirect =
        test::call_service(&app, test::TestRequest::get().uri("/admin-ui").to_request()).await;
    assert_eq!(redirect.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        redirect
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok()),
        Some("/admin-ui/")
    );
    assert_eq!(
        redirect
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );

    for (path, expected_content_type, expected_body) in [
        ("/admin-ui/", "text/html; charset=utf-8", INDEX),
        (
            "/admin-ui/assets/index.css",
            "text/css; charset=utf-8",
            STYLES,
        ),
        (
            "/admin-ui/assets/main.js",
            "application/javascript; charset=utf-8",
            APPLICATION,
        ),
        (
            "/admin-ui/assets/vendor.js",
            "application/javascript; charset=utf-8",
            VENDOR,
        ),
    ] {
        let response =
            test::call_service(&app, test::TestRequest::get().uri(path).to_request()).await;
        assert_eq!(response.status(), StatusCode::OK, "asset {path}");
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some(expected_content_type),
            "asset {path}"
        );
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store"),
            "asset {path}"
        );
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some(CONTENT_SECURITY_POLICY),
            "asset {path}"
        );
        assert_eq!(
            response
                .headers()
                .get("X-Content-Type-Options")
                .and_then(|value| value.to_str().ok()),
            Some("nosniff"),
            "asset {path}"
        );
        assert_eq!(
            response
                .headers()
                .get("X-Frame-Options")
                .and_then(|value| value.to_str().ok()),
            Some("DENY"),
            "asset {path}"
        );
        assert_eq!(
            response
                .headers()
                .get(header::REFERRER_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some("no-referrer"),
            "asset {path}"
        );
        assert_eq!(
            &test::read_body(response).await[..],
            expected_body,
            "asset {path}"
        );
    }

    let unknown = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin-ui/assets/not-an-embedded-file.js")
            .to_request(),
    )
    .await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[actix_web::test]
async fn management_ui_and_public_data_plane_configurations_do_not_share_routes() -> TestResult {
    let data_plane = test::init_service(App::new().configure(configure)).await;
    let health = test::call_service(
        &data_plane,
        test::TestRequest::get().uri("/healthz").to_request(),
    )
    .await;
    assert_eq!(health.status(), StatusCode::OK);
    let data_plane_ui = test::call_service(
        &data_plane,
        test::TestRequest::get().uri("/admin-ui/").to_request(),
    )
    .await;
    assert_eq!(data_plane_ui.status(), StatusCode::NOT_FOUND);

    let management_ui =
        test::init_service(App::new().configure(configure_embedded_management_ui)).await;
    let management_health = test::call_service(
        &management_ui,
        test::TestRequest::get().uri("/healthz").to_request(),
    )
    .await;
    assert_eq!(management_health.status(), StatusCode::NOT_FOUND);
    let management_index = test::call_service(
        &management_ui,
        test::TestRequest::get().uri("/admin-ui/").to_request(),
    )
    .await;
    assert_eq!(management_index.status(), StatusCode::OK);
    Ok(())
}

#[actix_web::test]
async fn route_level_probe_reports_data_plane_timing_with_and_without_ui_registration() -> TestResult
{
    const REQUESTS: usize = 2_000;

    let data_plane_only = test::init_service(App::new().configure(configure)).await;
    let baseline_started = Instant::now();
    for _ in 0..REQUESTS {
        let response = test::call_service(
            &data_plane_only,
            test::TestRequest::get().uri("/healthz").to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let baseline_elapsed = baseline_started.elapsed();

    let data_plane_with_ui = test::init_service(
        App::new()
            .configure(configure)
            .configure(configure_embedded_management_ui),
    )
    .await;
    let with_ui_started = Instant::now();
    for _ in 0..REQUESTS {
        let response = test::call_service(
            &data_plane_with_ui,
            test::TestRequest::get().uri("/healthz").to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let with_ui_elapsed = with_ui_started.elapsed();

    println!(
        "p10-09 route-level probe: healthz requests={REQUESTS}, data-plane-only_us={}, with-ui-route-table_us={}",
        baseline_elapsed.as_micros(),
        with_ui_elapsed.as_micros(),
    );
    Ok(())
}
