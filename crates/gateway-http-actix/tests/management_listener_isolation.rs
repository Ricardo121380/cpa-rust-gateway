//! API-only management and public-listener route isolation.

#![deny(unsafe_code)]

use actix_web::{App, http::StatusCode, test};
use gateway_http_actix::{configure, configure_management_listener};

#[actix_web::test]
async fn public_listener_does_not_mount_management_routes_or_ui() {
    let app = test::init_service(App::new().configure(configure)).await;
    let response =
        test::call_service(&app, test::TestRequest::get().uri("/healthz").to_request()).await;
    assert_eq!(response.status(), StatusCode::OK);
    for path in [
        "/admin/config-versions",
        "/admin-ui",
        "/admin-ui/",
        "/admin-ui/assets/main.js",
    ] {
        let response =
            test::call_service(&app, test::TestRequest::get().uri(path).to_request()).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
}

#[actix_web::test]
async fn management_listener_does_not_mount_public_or_retired_ui_routes() {
    let app = test::init_service(App::new().configure(configure_management_listener)).await;
    for path in [
        "/healthz",
        "/v1/models",
        "/admin-ui",
        "/admin-ui/",
        "/admin-ui/assets/main.js",
    ] {
        let response =
            test::call_service(&app, test::TestRequest::get().uri(path).to_request()).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
}
