//! Embedded management SPA resources isolated from public inference routing.
//!
//! This module has no management credentials, persistence handle, Provider dependency, runtime
//! configuration, filesystem read, network operation, or data-plane registration. An embedding
//! explicitly mounts it on a management listener; [`crate::configure`] intentionally does not.

#![deny(unsafe_code)]

use actix_web::{HttpResponse, http::header, web};

const MANAGEMENT_UI_ROOT: &str = "/admin-ui/";
// `data:` is admitted for images only. The management SPA generates its glass
// lens displacement maps at runtime with canvas.toDataURL (web/prism
// src/components/glass/PrismLens.tsx), and a data: image cannot execute — the
// dangerous data: sinks are script-src and object-src, which stay closed.
// Requested by the frontend side; see docs/cross-boundary-log.md 2026-08-11.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; base-uri 'none'; connect-src 'self'; form-action 'none'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'; style-src 'self'";

const INDEX: EmbeddedAsset = EmbeddedAsset::new(
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/prism/dist/index.html"
    )),
    "text/html; charset=utf-8",
);
const STYLES: EmbeddedAsset = EmbeddedAsset::new(
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/prism/dist/assets/index.css"
    )),
    "text/css; charset=utf-8",
);
const APPLICATION: EmbeddedAsset = EmbeddedAsset::new(
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/prism/dist/assets/main.js"
    )),
    "application/javascript; charset=utf-8",
);
/// Vendor chunk the entry module preloads. The SPA emits exactly four files
/// (its own C3 check pins the set), so this route count is fixed, not a policy
/// of "serve whatever the bundler produced".
const VENDOR: EmbeddedAsset = EmbeddedAsset::new(
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/prism/dist/assets/vendor.js"
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
        .route("/admin-ui/assets/index.css", web::get().to(styles))
        .route("/admin-ui/assets/main.js", web::get().to(application))
        .route("/admin-ui/assets/vendor.js", web::get().to(vendor));
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

async fn vendor() -> HttpResponse {
    embedded_asset_response(VENDOR)
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
