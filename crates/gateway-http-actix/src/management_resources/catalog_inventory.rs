//! Read-only, revision-bound enumeration of one saved upstream catalog.
use super::{
    ConfigRevision, ManagementResourceError, ManagementResourceHttpState, error_response,
    internal_error, invalid_input, management_error, query_has_duplicate_keys, read_context,
    read_operations, response_with_revision, service,
};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_store::control_plane::CatalogModelQuery;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    endpoint_id: String,
    credential_id: String,
    limit: Option<u16>,
    q: Option<String>,
    cursor: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    config: String,
    revision: i64,
    endpoint: String,
    credential: String,
    snapshot: i64,
    observed: i64,
    q: String,
    after: String,
}

fn conflict() -> HttpResponse {
    error_response(
        StatusCode::CONFLICT,
        "management_catalog_cursor_conflict",
        "Catalog changed; restart enumeration",
    )
}

fn parse_cursor(
    encoded: Option<&str>,
    config: &str,
    endpoint: &str,
    credential: &str,
    q: &str,
) -> Result<Option<Cursor>, HttpResponse> {
    let Some(encoded) = encoded else {
        return Ok(None);
    };
    if encoded.len() > 7000 {
        return Err(invalid_input());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| invalid_input())?;
    let cursor: Cursor = serde_json::from_slice(&bytes).map_err(|_| invalid_input())?;
    if cursor.revision < 0
        || cursor.snapshot <= 0
        || cursor.observed < 0
        || cursor.after.is_empty()
        || cursor.after.len() > 4096
    {
        return Err(invalid_input());
    }
    if cursor.config != config
        || cursor.endpoint != endpoint
        || cursor.credential != credential
        || cursor.q != q
    {
        return Err(conflict());
    }
    Ok(Some(cursor))
}

pub(super) async fn models(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if request.query_string().len() > 8192 || query_has_duplicate_keys(request.query_string()) {
        return invalid_input();
    }
    let params = match web::Query::<Params>::from_query(request.query_string()) {
        Ok(v) => v.into_inner(),
        Err(_) => return invalid_input(),
    };
    let limit = params.limit.unwrap_or(100);
    let q = params.q.unwrap_or_default();
    if !(1..=100).contains(&limit)
        || q.len() > 256
        || [&params.endpoint_id, &params.credential_id]
            .iter()
            .any(|id| id.is_empty() || id.len() > 128)
    {
        return invalid_input();
    }
    let cursor = match parse_cursor(
        params.cursor.as_deref(),
        context.version.as_str(),
        &params.endpoint_id,
        &params.credential_id,
        &q,
    ) {
        Ok(cursor) => cursor,
        Err(response) => return response,
    };
    let reader = match service(&state) {
        Ok(mut s) => s.repository_mut().resource_inventory_reader(),
        Err(r) => return r,
    };
    let Some(reader) = reader else {
        return internal_error();
    };
    let endpoint = params.endpoint_id.clone();
    let credential = params.credential_id.clone();
    let search = q.clone();
    let result = read_operations(&state, move || {
        Ok(reader.catalog_models(CatalogModelQuery {
            version: &context.version,
            endpoint_id: &endpoint,
            credential_id: &credential,
            search: &search,
            limit,
            expected: cursor
                .as_ref()
                .map(|c| (c.revision, c.snapshot, c.observed)),
            after: cursor.as_ref().map(|c| c.after.as_str()),
        }))
    })
    .await;
    let page = match result {
        Ok(Ok(p)) => p,
        Ok(Err(gateway_store::StoreError::ConfigVersionRevisionConflict)) => return conflict(),
        Ok(Err(e)) => return management_error(ManagementResourceError::Store(e)),
        Err(e) => return management_error(ManagementResourceError::from(e)),
    };
    let Ok(revision) = ConfigRevision::try_new(page.version.revision) else {
        return internal_error();
    };
    let next = page
        .next_after
        .map(|after| {
            serde_json::to_vec(&Cursor {
                config: page.version.id.to_string(),
                revision: page.version.revision,
                endpoint: params.endpoint_id.clone(),
                credential: params.credential_id.clone(),
                snapshot: page.target.snapshot_version,
                observed: page.target.observed_at_ms,
                q,
                after,
            })
        })
        .transpose();
    let next = match next {
        Ok(v) => v.map(|bytes| URL_SAFE_NO_PAD.encode(bytes)),
        Err(_) => return internal_error(),
    };
    response_with_revision(
        StatusCode::OK,
        revision,
        serde_json::json!({
            "config_version":page.version.id.to_string(),"revision":revision.as_token(),
            "target":{"endpoint_id":params.endpoint_id,"credential_id":params.credential_id,"snapshot_version":page.target.snapshot_version,"observed_at_ms":page.target.observed_at_ms,"stale_at_ms":page.target.stale_at_ms,"expires_at_ms":page.target.expires_at_ms},
            "items":page.items.into_iter().map(|r|serde_json::json!({"model":r.model,"present_in_last_success":r.present_in_last_success})).collect::<Vec<_>>(),
            "next_cursor":next,"current_model_count":page.current_model_count,"total_count":page.total_count,
        }),
    )
}
