//! Read-only configuration comparison with opaque revision-pinned continuation.
use super::{
    ManagementResourceError, ManagementResourceHttpState, internal_error, invalid_input,
    management_error, read_operations,
};
use actix_web::{
    HttpRequest, HttpResponse,
    http::{StatusCode, header},
    web,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_store::control_plane::{
    ConfigVersionId, ConfigurationDiffError, ConfigurationDiffPage, ConfigurationDiffQuery,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    base_id: String,
    limit: Option<u16>,
    cursor: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    base: String,
    target: String,
    base_revision: i64,
    target_revision: i64,
    after_kind: String,
    after_key: String,
}
struct Input {
    base: ConfigVersionId,
    target: ConfigVersionId,
    limit: u16,
    cursor: Option<Cursor>,
}
fn parse(request: &HttpRequest, target: String) -> Result<Input, HttpResponse> {
    let params = web::Query::<Params>::from_query(request.query_string())
        .map_err(|_| invalid_input())?
        .into_inner();
    let base = ConfigVersionId::try_new(params.base_id).map_err(|_| invalid_input())?;
    let target = ConfigVersionId::try_new(target).map_err(|_| invalid_input())?;
    let limit = params.limit.unwrap_or(100);
    if !(1..=200).contains(&limit) {
        return Err(invalid_input());
    }
    let cursor = params
        .cursor
        .map(|raw| {
            if raw.len() > 8192 {
                return Err(invalid_input());
            }
            let bytes = URL_SAFE_NO_PAD.decode(raw).map_err(|_| invalid_input())?;
            let cursor: Cursor = serde_json::from_slice(&bytes).map_err(|_| invalid_input())?;
            if cursor.base != base.as_str() || cursor.target != target.as_str() {
                return Err(diff_error(&ConfigurationDiffError::RevisionChanged));
            }
            if cursor.base_revision < 0
                || cursor.target_revision < 0
                || cursor.after_kind.is_empty()
                || cursor.after_kind.len() > 64
                || cursor.after_key.is_empty()
                || cursor.after_key.len() > 2048
            {
                return Err(invalid_input());
            }
            Ok(cursor)
        })
        .transpose()?;
    Ok(Input {
        base,
        target,
        limit,
        cursor,
    })
}

pub(super) async fn read(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let input = match parse(&request, path.into_inner()) {
        Ok(input) => input,
        Err(response) => return response,
    };
    // Only copy the source handle while holding the mutation mutex. SQL opens its own
    // read-only connection inside the existing bounded blocking admission.
    let reader = match state.service.lock() {
        Ok(service) => service.configuration_diff_reader(),
        Err(_) => return internal_error(),
    };
    let Some(reader) = reader else {
        return internal_error();
    };
    match read_operations(&state, move || {
        Ok(reader.read(ConfigurationDiffQuery {
            base: &input.base,
            target: &input.target,
            limit: input.limit,
            expected_revisions: input
                .cursor
                .as_ref()
                .map(|cursor| (cursor.base_revision, cursor.target_revision)),
            after: input
                .cursor
                .as_ref()
                .map(|cursor| (cursor.after_kind.as_str(), cursor.after_key.as_str())),
        }))
    })
    .await
    {
        Ok(Ok(page)) => response(page),
        Ok(Err(error)) => diff_error(&error),
        Err(error) => management_error(ManagementResourceError::from(error)),
    }
}

fn response(page: ConfigurationDiffPage) -> HttpResponse {
    let cursor = if page.has_more {
        page.items.last().map(|row| Cursor {
            base: page.base.id.as_str().to_owned(),
            target: page.target.id.as_str().to_owned(),
            base_revision: page.base.revision,
            target_revision: page.target.revision,
            after_kind: row.resource_kind.clone(),
            after_key: row.resource_key.clone(),
        })
    } else {
        None
    };
    let next_cursor = match cursor.map(|cursor| serde_json::to_vec(&cursor)).transpose() {
        Ok(bytes) => bytes.map(|bytes| URL_SAFE_NO_PAD.encode(bytes)),
        Err(_) => return internal_error(),
    };
    let items = page.items.into_iter().map(|row| serde_json::json!({"resource_kind":row.resource_kind, "resource_key":row.resource_key, "change":row.change, "changed_fields":row.changed_fields})).collect::<Vec<_>>();
    HttpResponse::Ok().insert_header((header::CACHE_CONTROL, "no-store")).json(serde_json::json!({
        "base": {"id":page.base.id.as_str(), "revision":format!("rev-{}",page.base.revision)},
        "target": {"id":page.target.id.as_str(), "revision":format!("rev-{}",page.target.revision)},
        "items":items, "next_cursor":next_cursor,
    }))
}
fn diff_error(error: &ConfigurationDiffError) -> HttpResponse {
    let (status, code) = match error {
        ConfigurationDiffError::InvalidQuery => {
            (StatusCode::BAD_REQUEST, "invalid_management_request")
        }
        ConfigurationDiffError::MissingVersion => {
            (StatusCode::NOT_FOUND, "management_resource_not_found")
        }
        ConfigurationDiffError::RevisionChanged => {
            (StatusCode::CONFLICT, "configuration_diff_conflict")
        }
        ConfigurationDiffError::Store(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "configuration_diff_unavailable",
        ),
    };
    HttpResponse::build(status).json(serde_json::json!({"error":{"code":code,"message":"Configuration comparison is unavailable; check versions and restart comparison."}}))
}
