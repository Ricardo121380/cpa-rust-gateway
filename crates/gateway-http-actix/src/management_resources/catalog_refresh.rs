//! Explicit saved-catalog refresh. Never opens models or accepts a client secret.
use super::{ManagementResourceHttpState, error_response, invalid_input, parse_json, read_context};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use gateway_core::{CredentialId, EndpointId};
use gateway_store::control_plane::ConfigVersionId;
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin, sync::Arc};

/// Metadata-only refresh result; existing model grants are never changed.
#[derive(Serialize)]
pub struct CatalogRefreshReceipt {
    /// Exact configuration inspected.
    pub config_version: String,
    /// Upstream interface inspected.
    pub endpoint_id: String,
    /// Account whose directory was read.
    pub credential_id: String,
    /// Completed observation time.
    pub observed_at_ms: i64,
    /// Complete unique model count returned by this source.
    pub model_count: usize,
}
/// Closed error categories, never upstream response bodies.
#[derive(Clone, Copy, Debug)]
pub enum CatalogRefreshError {
    /// The source has no implemented catalog API.
    Unsupported,
    /// A refresh or lease is already in progress.
    Busy,
    /// Configuration was replaced while reading.
    Conflict,
    /// Authenticated metadata read failed.
    Upstream,
}
/// Bounded metadata request without holding the management database mutex.
pub type CatalogRefreshFuture =
    Pin<Box<dyn Future<Output = Result<CatalogRefreshReceipt, CatalogRefreshError>>>>;
/// Injected live-generation source, using server-owned credentials and egress policy.
pub trait CatalogRefreshFacade: Send + Sync {
    /// Refreshes only one explicitly selected target.
    fn refresh(
        &self,
        version: ConfigVersionId,
        endpoint: EndpointId,
        credential: CredentialId,
    ) -> CatalogRefreshFuture;
}
impl ManagementResourceHttpState {
    /// Connects real metadata discovery to management without changing OAuth workflows.
    #[must_use]
    pub fn with_catalog_refresh(mut self, source: Arc<dyn CatalogRefreshFacade>) -> Self {
        self.catalog_refresh = Some(source);
        self
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    endpoint_id: String,
    credential_id: String,
}
pub(super) async fn refresh(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let input: Input = match parse_json(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Ok(endpoint), Ok(credential)) = (
        EndpointId::try_new(input.endpoint_id),
        CredentialId::try_new(input.credential_id),
    ) else {
        return invalid_input();
    };
    let Some(source) = state.catalog_refresh.as_ref() else {
        return failure(CatalogRefreshError::Unsupported);
    };
    let Ok(_permit) = state.catalog_refresh_slots.clone().try_acquire_owned() else {
        return failure(CatalogRefreshError::Busy);
    };
    match tokio::time::timeout(
        std::time::Duration::from_secs(35),
        source.refresh(
            context.version.clone(),
            endpoint.clone(),
            credential.clone(),
        ),
    )
    .await
    {
        Ok(Ok(receipt))
            if receipt.config_version != context.version.as_str()
                || receipt.endpoint_id != endpoint.as_str()
                || receipt.credential_id != credential.as_str()
                || receipt.observed_at_ms < 0 =>
        {
            failure(CatalogRefreshError::Conflict)
        }
        Ok(Ok(receipt)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(receipt),
        Ok(Err(error)) => failure(error),
        Err(_) => failure(CatalogRefreshError::Upstream),
    }
}
fn failure(error: CatalogRefreshError) -> HttpResponse {
    let (status, code, message) = match error {
        CatalogRefreshError::Unsupported => (
            StatusCode::NOT_IMPLEMENTED,
            "management_catalog_source_unsupported",
            "此接口尚未提供可读取的模型目录",
        ),
        CatalogRefreshError::Busy => (
            StatusCode::TOO_MANY_REQUESTS,
            "management_catalog_refresh_busy",
            "目录刷新繁忙，请稍后重试",
        ),
        CatalogRefreshError::Conflict => (
            StatusCode::CONFLICT,
            "management_catalog_refresh_conflict",
            "配置或账号已改变，请重新读取",
        ),
        CatalogRefreshError::Upstream => (
            StatusCode::BAD_GATEWAY,
            "management_catalog_refresh_failed",
            "上游目录读取失败，保留上次成功观测",
        ),
    };
    error_response(status, code, message)
}
