//! Protected, bounded, offline history query. Reading history never fetches upstream data.
use super::{
    HttpRequest, HttpResponse, ManagementResourceHttpState, principal, read_operations, web,
};
use gateway_store::quota_history::{Error, Query};
pub(super) async fn list(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if let Err(response) = principal(&request) {
        return response;
    }
    let Ok(query) = web::Query::<Query>::from_query(request.query_string()) else {
        return super::invalid_input();
    };
    let Some(store) = state.quota_history.clone() else {
        return unavailable();
    };
    let Ok(now) = state.runtime_clock.now_ms() else {
        return unavailable();
    };
    match read_operations(&state, move || Ok(store.query(&query, now))).await {
        Ok(Ok(page)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(page),
        Ok(Err(Error::Invalid)) => super::invalid_input(),
        _ => unavailable(),
    }
}

fn unavailable() -> HttpResponse {
    HttpResponse::ServiceUnavailable().insert_header(("Cache-Control", "no-store")).json(serde_json::json!({"error":{"code":"quota_history_unavailable","message":"Quota history unavailable"}}))
}
