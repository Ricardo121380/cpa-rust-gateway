//! Authenticated request history, with literal filters and cursor-bound event snapshots.
use super::{
    ManagementResourceHttpState, error_response, internal_error, invalid_input,
    query_has_duplicate_keys, read_operations, service,
};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_store::control_plane::RequestHistoryQuery;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    from_ms: Option<i64>,
    to_ms: Option<i64>,
    model: Option<String>,
    upstream_id: Option<String>,
    credential_id: Option<String>,
    client_key_id: Option<String>,
    outcome: Option<String>,
    request_id: Option<String>,
    include_unknown: Option<bool>,
    bucket_ms: Option<i64>,
    limit: Option<u16>,
    cursor: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    ledger_snapshot: i64,
    snapshot: i64,
    after: i64,
    filter: Params,
}
fn conflict() -> HttpResponse {
    error_response(
        StatusCode::CONFLICT,
        "management_request_cursor_conflict",
        "筛选或快照已改变，请重新读取",
    )
}
pub(super) async fn list(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    read(request, state, false).await
}
pub(super) async fn summary(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    read(request, state, true).await
}
async fn read(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
    summary: bool,
) -> HttpResponse {
    if request.query_string().len() > 8192 || query_has_duplicate_keys(request.query_string()) {
        return invalid_input();
    }
    let mut params = match web::Query::<Params>::from_query(request.query_string()) {
        Ok(v) => v.into_inner(),
        Err(_) => return invalid_input(),
    };
    let cursor = match params.cursor.take() {
        None => None,
        Some(encoded) if encoded.len() <= 7000 => match URL_SAFE_NO_PAD
            .decode(encoded)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Cursor>(&bytes).ok())
        {
            Some(c) => Some(c),
            None => return invalid_input(),
        },
        Some(_) => return invalid_input(),
    };
    if cursor.as_ref().is_some_and(|c| c.filter != params) {
        return conflict();
    }
    let reader = match service(&state) {
        Ok(mut s) => s.repository_mut().resource_inventory_reader(),
        Err(r) => return r,
    };
    let Some(reader) = reader else {
        return internal_error();
    };
    let query = RequestHistoryQuery {
        from_ms: params.from_ms,
        to_ms: params.to_ms,
        model: params.model.clone(),
        upstream_id: params.upstream_id.clone(),
        credential_id: params.credential_id.clone(),
        client_key_id: params.client_key_id.clone(),
        outcome: params.outcome.clone(),
        request_id: params.request_id.clone(),
        include_unknown: params.include_unknown.unwrap_or(false),
        bucket_ms: params.bucket_ms.unwrap_or(3_600_000),
        limit: params.limit.unwrap_or(50),
        ledger_snapshot: cursor.as_ref().map(|v| v.ledger_snapshot),
        snapshot: cursor.as_ref().map(|v| v.snapshot),
        after: cursor.as_ref().map(|v| v.after),
        summary: summary && cursor.is_none(),
    };
    let result = read_operations(&state, move || Ok(reader.requests(&query))).await;
    match result {
        Ok(Ok(page)) => {
            let next = page
                .next_after
                .and_then(|after| {
                    serde_json::to_vec(&Cursor {
                        ledger_snapshot: page.ledger_snapshot,
                        snapshot: page.snapshot,
                        after,
                        filter: params,
                    })
                    .ok()
                })
                .map(|bytes| URL_SAFE_NO_PAD.encode(bytes));
            HttpResponse::Ok().insert_header(("Cache-Control","no-store")).json(serde_json::json!({"ledger_snapshot":page.ledger_snapshot,"snapshot":page.snapshot,"items":page.items,"next_cursor":next,"summary":page.summary,"series":page.series}))
        }
        Ok(Err(gateway_store::StoreError::InvalidPersistedGatewayEvent)) => invalid_input(),
        Ok(Err(gateway_store::StoreError::ConfigVersionRevisionConflict)) => conflict(),
        _ => internal_error(),
    }
}
