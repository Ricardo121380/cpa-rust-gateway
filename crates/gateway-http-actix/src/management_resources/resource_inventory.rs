//! Complete managed inventories, distinct from running account pools.
use super::{
    ConfigRevision, CredentialResponse, CredentialStatus, EndpointResponse,
    ManagementResourceError, ManagementResourceHttpState, error_response, internal_error,
    invalid_input, management_error, query_has_duplicate_keys, read_context, read_operations,
    response_with_revision, service,
};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_store::control_plane::{ResourceInventoryPage, ResourceInventoryQuery};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    limit: Option<u16>,
    cursor: Option<String>,
    upstream_id: Option<String>,
    q: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    kind: String,
    version: String,
    revision: i64,
    audit_sequence: i64,
    upstream_id: Option<String>,
    q: String,
    after: String,
}

#[derive(Serialize)]
struct Response {
    config_version: String,
    revision: String,
    observation_version: String,
    items: Vec<serde_json::Value>,
    next_cursor: Option<String>,
}

fn mapped<T>(
    page: ResourceInventoryPage<T>,
    convert: impl Fn(T) -> serde_json::Value,
) -> ResourceInventoryPage<serde_json::Value> {
    ResourceInventoryPage {
        version: page.version,
        audit_sequence: page.audit_sequence,
        next_after: page.next_after,
        items: page.items.into_iter().map(convert).collect(),
    }
}

pub(super) async fn credentials(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    read(request, state, "credentials").await
}

pub(super) async fn endpoints(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    read(request, state, "endpoints").await
}

fn inventory_conflict() -> HttpResponse {
    error_response(
        StatusCode::CONFLICT,
        "management_inventory_cursor_conflict",
        "Inventory changed; restart enumeration",
    )
}

fn parse_cursor(
    encoded: Option<&str>,
    kind: &str,
    version: &str,
    owner: Option<&str>,
    q: &str,
) -> Result<Option<Cursor>, HttpResponse> {
    let Some(encoded) = encoded else {
        return Ok(None);
    };
    if encoded.len() > 2048 {
        return Err(invalid_input());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| invalid_input())?;
    let cursor: Cursor = serde_json::from_slice(&bytes).map_err(|_| invalid_input())?;
    if cursor.revision < 0
        || cursor.audit_sequence < 0
        || cursor.after.is_empty()
        || cursor.after.len() > 128
    {
        return Err(invalid_input());
    }
    if cursor.kind != kind
        || cursor.version != version
        || cursor.upstream_id.as_deref() != owner
        || cursor.q != q
    {
        return Err(inventory_conflict());
    }
    Ok(Some(cursor))
}

async fn read(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
    kind: &'static str,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if request.query_string().len() > 4096 || query_has_duplicate_keys(request.query_string()) {
        return invalid_input();
    }
    let params = match web::Query::<Params>::from_query(request.query_string()) {
        Ok(value) => value.into_inner(),
        Err(_) => return invalid_input(),
    };
    let limit = params.limit.unwrap_or(100);
    let q = params.q.unwrap_or_default();
    if !(1..=100).contains(&limit)
        || q.len() > 256
        || params
            .upstream_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 128)
    {
        return invalid_input();
    }
    let cursor = match parse_cursor(
        params.cursor.as_deref(),
        kind,
        context.version.as_str(),
        params.upstream_id.as_deref(),
        &q,
    ) {
        Ok(cursor) => cursor,
        Err(response) => return response,
    };
    let (reader, project) = match service(&state) {
        Ok(mut service) => (
            service.repository_mut().resource_inventory_reader(),
            service.account_identity_projector(),
        ),
        Err(response) => return response,
    };
    let Some(reader) = reader else {
        return internal_error();
    };
    let owner = params.upstream_id.clone();
    let search = q.clone();
    let result = read_operations(&state, move || {
        let query = ResourceInventoryQuery {
            version: &context.version,
            upstream_id: owner.as_deref(),
            search: &search,
            limit,
            expected_snapshot: cursor
                .as_ref()
                .map(|cursor| (cursor.revision, cursor.audit_sequence)),
            after: cursor.as_ref().map(|cursor| cursor.after.as_str()),
        };
        Ok(if kind == "credentials" {
            reader.credentials_with_identity(query,project.as_ref()).map(|page| {
                mapped(page, |row| {
                    let urls=row.connections.iter().map(|c|url::Url::parse(&c.base_url).ok()).collect::<Vec<_>>();
                    let (category,provider) = gateway_control::account_presentation::ordinary_channel(&row.kind,&row.upstream_kind,row.connections.iter().zip(&urls).map(|(c,u)|(c.adapter_id.as_str(),u.as_ref().and_then(url::Url::host_str))));
                    let connections=row.connections.iter().map(|c|serde_json::json!({"id":c.id,"api_format":c.api_format,"enabled":c.enabled,"host":url::Url::parse(&c.base_url).ok().and_then(|u|u.host_str().map(str::to_owned))})).collect::<Vec<_>>();
                    let body = CredentialResponse {
                        id: row.id.to_string(),
                        upstream_id: row.upstream_id.to_string(),
                        kind: row.kind,
                        status: match row.status {
                            CredentialStatus::Active => "active",
                            _ => "disabled",
                        },
                        revision: row.revision,
                        secret_present: row.secret_present,
                    };
                    serde_json::json!({"credential":body,"binding_count":row.binding_count,"identity":row.identity,"category":category,"provider":provider,"connections":connections})
                })
            })
        } else {
            reader
                .endpoints(query)
                .map(|page| mapped(page, |row| serde_json::json!(EndpointResponse::from(row))))
        })
    })
    .await;
    let page = match result {
        Ok(Ok(page)) => page,
        Ok(Err(gateway_store::StoreError::ConfigVersionRevisionConflict)) => {
            return inventory_conflict();
        }
        Ok(Err(error)) => return management_error(ManagementResourceError::Store(error)),
        Err(error) => return management_error(ManagementResourceError::from(error)),
    };
    response(page, kind, params.upstream_id, q)
}

fn response(
    page: ResourceInventoryPage<serde_json::Value>,
    kind: &str,
    upstream_id: Option<String>,
    q: String,
) -> HttpResponse {
    let Ok(revision) = ConfigRevision::try_new(page.version.revision) else {
        return internal_error();
    };
    let next_cursor = page
        .next_after
        .map(|after| Cursor {
            kind: kind.to_owned(),
            version: page.version.id.to_string(),
            revision: page.version.revision,
            audit_sequence: page.audit_sequence,
            upstream_id,
            q,
            after,
        })
        .map(|cursor| serde_json::to_vec(&cursor))
        .transpose();
    let next_cursor = match next_cursor {
        Ok(value) => value.map(|bytes| URL_SAFE_NO_PAD.encode(bytes)),
        Err(_) => return internal_error(),
    };
    response_with_revision(
        StatusCode::OK,
        revision,
        Response {
            config_version: page.version.id.to_string(),
            revision: revision.as_token(),
            observation_version: format!("audit-{}", page.audit_sequence),
            items: page.items,
            next_cursor,
        },
    )
}
