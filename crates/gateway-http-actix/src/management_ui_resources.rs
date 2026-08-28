//! Embedded management SPA resources isolated from public inference routing.
//!
//! This module has no management credentials, persistence handle, Provider dependency, runtime
//! configuration, filesystem read, network operation, or data-plane registration. An embedding
//! explicitly mounts it on a management listener; [`crate::configure`] intentionally does not.

#![deny(unsafe_code)]

use actix_web::{HttpResponse, http::header, web};

const MANAGEMENT_UI_ROOT: &str = "/admin-ui/";
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; base-uri 'none'; connect-src 'self'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; object-src 'none'; script-src 'self'; style-src 'self'";

const INDEX: EmbeddedAsset = EmbeddedAsset::new(
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/admin-ui/dist/index.html"
    )),
    "text/html; charset=utf-8",
);
const STYLES: EmbeddedAsset = EmbeddedAsset::new(
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/admin-ui/dist/assets/styles.css"
    )),
    "text/css; charset=utf-8",
);
const APPLICATION: EmbeddedAsset = EmbeddedAsset::new(
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/admin-ui/dist/assets/main.js"
    )),
    "application/javascript; charset=utf-8",
);
const GENERATED_CLIENT: EmbeddedAsset = EmbeddedAsset::new(
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/admin-ui/dist/assets/generated/management-client.js"
    )),
    "application/javascript; charset=utf-8",
);

#[derive(Clone, Copy)]
struct EmbeddedAsset {
    body: &'static [u8],
    content_type: &'static str,
}

impl EmbeddedAsset {
    const fn new(body: &'static [u8], content_type: &'static str) -> Self {
        Self { body, content_type }
    }
}

/// Registers the embedded SPA on a dedicated management-listener configuration.
///
/// The only served paths are `/admin-ui/` and its exact build assets. This does not mount an API,
/// authenticate a request, access filesystem state, or modify the public inference configuration.
/// P12 owns selecting a listener and external network exposure.
pub fn configure_embedded_management_ui(config: &mut web::ServiceConfig) {
    config
        .route("/admin-ui", web::get().to(redirect_to_management_ui_root))
        .route(MANAGEMENT_UI_ROOT, web::get().to(index))
        .route("/admin-ui/assets/styles.css", web::get().to(styles))
        .route("/admin-ui/assets/main.js", web::get().to(application))
        .route(
            "/admin-ui/assets/generated/management-client.js",
            web::get().to(generated_client),
        );
}

async fn redirect_to_management_ui_root() -> HttpResponse {
    HttpResponse::PermanentRedirect()
        .insert_header((header::LOCATION, MANAGEMENT_UI_ROOT))
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .finish()
}

async fn index() -> HttpResponse {
    embedded_asset_response(INDEX)
}

async fn styles() -> HttpResponse {
    embedded_asset_response(STYLES)
}

async fn application() -> HttpResponse {
    embedded_asset_response(APPLICATION)
}

async fn generated_client() -> HttpResponse {
    embedded_asset_response(GENERATED_CLIENT)
}

fn embedded_asset_response(asset: EmbeddedAsset) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, asset.content_type))
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .insert_header((header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY))
        .insert_header((header::REFERRER_POLICY, "no-referrer"))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("X-Frame-Options", "DENY"))
        .body(web::Bytes::from_static(asset.body))
}
