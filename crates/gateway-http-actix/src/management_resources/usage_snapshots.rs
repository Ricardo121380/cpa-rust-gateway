//! Protected, actor-owned finite snapshots. Export pages never revisit mutable source tables.
use super::{
    ManagementResourceHttpState, error_response, internal_error, invalid_input, principal,
    query_has_duplicate_keys, read_operations,
};
use actix_web::{
    HttpRequest, HttpResponse,
    http::{StatusCode, header},
    web,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_control::usage_snapshot::{SnapshotError, UsageSnapshot, UsageSnapshotQuery};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/operations/usage-snapshots", web::post().to(create))
        .route("/operations/usage-snapshots/{id}", web::get().to(get))
        .route("/operations/usage-snapshots/{id}", web::delete().to(cancel))
        .route(
            "/operations/usage-snapshots/{id}/rows",
            web::get().to(details),
        )
        .route(
            "/operations/usage-snapshots/{id}/export",
            web::get().to(export),
        );
}

const TTL: Duration = Duration::from_mins(10);
const MAX_SNAPSHOTS: usize = 8;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;

struct Retained {
    owner: String,
    deadline: Instant,
    expires_at_ms: u128,
    bytes: usize,
    snapshot: Arc<UsageSnapshot>,
}
#[derive(Default)]
pub(super) struct Snapshots(BTreeMap<String, Retained>);
impl Snapshots {
    fn prune(&mut self) {
        self.0.retain(|_, s| s.deadline > Instant::now());
    }
    fn get(&mut self, id: &str, owner: &str) -> Option<(Arc<UsageSnapshot>, u128)> {
        self.prune();
        self.0
            .get(id)
            .filter(|s| s.owner == owner)
            .map(|s| (Arc::clone(&s.snapshot), s.expires_at_ms))
    }
    fn insert(
        &mut self,
        snapshot: UsageSnapshot,
        owner: String,
        bytes: usize,
    ) -> Result<(String, Arc<UsageSnapshot>, u128), HttpResponse> {
        self.prune();
        if bytes > MAX_SNAPSHOT_BYTES {
            return Err(snapshot_error(SnapshotError::LimitExceeded));
        }
        if self.0.len() >= MAX_SNAPSHOTS
            || self.0.values().map(|s| s.bytes).sum::<usize>() + bytes > MAX_BYTES
        {
            return Err(error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "usage_snapshot_capacity",
                "快照容量已满，请取消旧快照或稍后重试",
            ));
        }
        let mut random = [0u8; 24];
        getrandom::fill(&mut random).map_err(|_| internal_error())?;
        let id = URL_SAFE_NO_PAD.encode(random);
        let expires_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| internal_error())?
            .as_millis()
            + TTL.as_millis();
        let snapshot = Arc::new(snapshot);
        self.0.insert(
            id.clone(),
            Retained {
                owner,
                deadline: Instant::now() + TTL,
                expires_at_ms,
                bytes,
                snapshot: Arc::clone(&snapshot),
            },
        );
        Ok((id, snapshot, expires_at_ms))
    }
}

pub(super) async fn create(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if !request.query_string().is_empty() || body.len() > 4096 {
        return invalid_input();
    }
    let query: UsageSnapshotQuery = match serde_json::from_slice(&body) {
        Ok(q) => q,
        Err(_) => return invalid_input(),
    };
    if let Err(e) = query.validate() {
        return snapshot_error(e);
    }
    let actor = match principal(&request) {
        Ok(p) => p.as_str().to_owned(),
        Err(e) => return e,
    };
    let source = Arc::clone(&state.usage);
    let result = read_operations(&state, move || {
        Ok(source.create_usage_snapshot(query).and_then(|snapshot| {
            let bytes = serde_json::to_vec(&snapshot)
                .map_err(|_| SnapshotError::SourceUnavailable)?
                .len();
            if bytes > MAX_SNAPSHOT_BYTES {
                return Err(SnapshotError::LimitExceeded);
            }
            Ok((snapshot, bytes))
        }))
    })
    .await;
    let (snapshot, bytes) = match result {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return snapshot_error(e),
        Err(e) => return super::management_error(super::ManagementResourceError::from(e)),
    };
    let Ok(mut snapshots) = state.usage_snapshots.lock() else {
        return internal_error();
    };
    match snapshots.insert(snapshot, actor, bytes) {
        Ok((id, snapshot, expires)) => metadata(StatusCode::CREATED, &id, &snapshot, expires),
        Err(e) => e,
    }
}

fn metadata(status: StatusCode, id: &str, snapshot: &UsageSnapshot, expires: u128) -> HttpResponse {
    HttpResponse::build(status).insert_header((header::CACHE_CONTROL,"no-store"))
        .json(serde_json::json!({"snapshot_id":id,"expires_at_ms":expires,"schema_version":snapshot.schema_version,
            "query":snapshot.query,"event_ordinal":snapshot.event_ordinal,"ledger_id":snapshot.ledger_id,
            "detail_rows":snapshot.details.len(),"bucket_rows":snapshot.buckets.len(),"summary":snapshot.summary}))
}
fn expired() -> HttpResponse {
    error_response(
        StatusCode::GONE,
        "usage_snapshot_expired",
        "快照已失效或不可访问，请重新创建",
    )
}
fn snapshot_error(error: SnapshotError) -> HttpResponse {
    match error {
        SnapshotError::InvalidQuery => invalid_input(),
        SnapshotError::LimitExceeded => error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "usage_snapshot_limit",
            "筛选范围超过快照上限，请缩小范围",
        ),
        SnapshotError::SourceUnavailable => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "usage_snapshot_unavailable",
            "用量或账本来源暂不可用，未生成快照",
        ),
    }
}
fn retained(
    request: &HttpRequest,
    id: &str,
    state: &ManagementResourceHttpState,
) -> Result<(Arc<UsageSnapshot>, u128), HttpResponse> {
    let actor = principal(request)?.as_str().to_owned();
    state
        .usage_snapshots
        .lock()
        .map_err(|_| internal_error())?
        .get(id, &actor)
        .ok_or_else(expired)
}
pub(super) async fn get(
    request: HttpRequest,
    id: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if !request.query_string().is_empty() {
        return invalid_input();
    }
    match retained(&request, &id, &state) {
        Ok((s, expires)) => metadata(StatusCode::OK, &id, &s, expires),
        Err(e) => e,
    }
}
pub(super) async fn cancel(
    request: HttpRequest,
    id: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if !request.query_string().is_empty() {
        return invalid_input();
    }
    if let Err(e) = retained(&request, &id, &state) {
        return e;
    }
    match state.usage_snapshots.lock() {
        Ok(mut snapshots) => {
            snapshots.0.remove(id.as_str());
            HttpResponse::NoContent()
                .insert_header((header::CACHE_CONTROL, "no-store"))
                .finish()
        }
        Err(_) => internal_error(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    limit: Option<usize>,
    cursor: Option<String>,
    grain: Option<String>,
    format: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    snapshot: String,
    grain: String,
    offset: usize,
}

pub(super) async fn details(
    request: HttpRequest,
    id: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    page(&request, &id, &state, false)
}
pub(super) async fn export(
    request: HttpRequest,
    id: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    page(&request, &id, &state, true)
}
fn page(
    request: &HttpRequest,
    id: &str,
    state: &ManagementResourceHttpState,
    export: bool,
) -> HttpResponse {
    if request.query_string().len() > 2048 || query_has_duplicate_keys(request.query_string()) {
        return invalid_input();
    }
    let params = match web::Query::<PageQuery>::from_query(request.query_string()) {
        Ok(p) => p.into_inner(),
        Err(_) => return invalid_input(),
    };
    let limit = params.limit.unwrap_or(100);
    let grain = params.grain.as_deref().unwrap_or("details");
    let format = params.format.as_deref().unwrap_or("jsonl");
    if !(1..=100).contains(&limit)
        || !matches!(grain, "details" | "buckets")
        || !matches!(format, "jsonl" | "csv")
        || (!export && params.format.is_some())
    {
        return invalid_input();
    }
    let (snapshot, _) = match retained(request, id, state) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let offset = match params.cursor.as_deref() {
        None => 0,
        Some(s) => match URL_SAFE_NO_PAD
            .decode(s)
            .ok()
            .and_then(|b| serde_json::from_slice::<Cursor>(&b).ok())
        {
            Some(c) if c.snapshot == id && c.grain == grain => c.offset,
            _ => {
                return error_response(
                    StatusCode::CONFLICT,
                    "usage_snapshot_cursor_conflict",
                    "游标不属于此快照或数据粒度",
                );
            }
        },
    };
    let count = if grain == "details" {
        snapshot.details.len()
    } else {
        snapshot.buckets.len()
    };
    if offset > count {
        return invalid_input();
    }
    let end = (offset + limit).min(count);
    let values = if grain == "details" {
        serde_json::to_value(&snapshot.details[offset..end])
    } else {
        serde_json::to_value(&snapshot.buckets[offset..end])
    };
    let Ok(Value::Array(values)) = values else {
        return internal_error();
    };
    let next = if end < count {
        match serde_json::to_vec(&Cursor {
            snapshot: id.to_string(),
            grain: grain.to_owned(),
            offset: end,
        }) {
            Ok(b) => Some(URL_SAFE_NO_PAD.encode(b)),
            Err(_) => return internal_error(),
        }
    } else {
        None
    };
    if !export {
        return HttpResponse::Ok().insert_header((header::CACHE_CONTROL,"no-store")).json(serde_json::json!({
        "schema_version":snapshot.schema_version,"snapshot_id":id,"grain":grain,"items":values,"next_cursor":next,"total_rows":count}));
    }
    let Ok(body) = encode(&values, format, grain) else {
        return internal_error();
    };
    let mut response = HttpResponse::Ok();
    response
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .insert_header((
            header::CONTENT_TYPE,
            if format == "csv" {
                "text/csv; charset=utf-8"
            } else {
                "application/x-ndjson"
            },
        ))
        .insert_header((
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"usage-{grain}-v1.{format}\""),
        ))
        .insert_header(("X-Usage-Snapshot", id))
        .insert_header(("X-Usage-Schema", snapshot.schema_version))
        .insert_header(("X-Usage-Total-Rows", count.to_string()));
    if let Some(next) = next {
        response.insert_header(("X-Next-Cursor", next));
    }
    response.body(body)
}

const DETAIL_FIELDS: &str = "row_kind,request_id,attempt_id,attempt_number,occurred_at_ms,model,protocol,client_key_id,access_group_id,provider_id,channel_id,account_id,request_outcome,attempt_outcome,usage_observed,usage_outcome,provenance,input_accounting,input_tokens,output_tokens,reasoning_tokens,cache_read_tokens,cache_creation_tokens,cached_tokens,ledger_id,catalog_version_id,cost_microunits,cost_status";
const BUCKET_FIELDS: &str = "from_ms,to_ms,requests,attempts,usage_observations,failed_usage_observations,cancelled_usage_observations,requests_without_terminal,input_tokens,output_tokens,reasoning_tokens,cache_read_tokens,cache_creation_tokens,cached_tokens,input_accounting,input_tokens_by_accounting,cost_records,known_cost_microunits,cost_status";

fn encode(values: &[Value], format: &str, grain: &str) -> Result<String, serde_json::Error> {
    let mut result = String::new();
    if format == "jsonl" {
        for value in values {
            result.push_str(&serde_json::to_string(value)?);
            result.push('\n');
        }
    } else {
        let fields: Vec<_> = if grain == "details" {
            DETAIL_FIELDS
        } else {
            BUCKET_FIELDS
        }
        .split(',')
        .collect();
        result.push_str(
            &fields
                .iter()
                .map(|k| csv_cell(k))
                .collect::<Vec<_>>()
                .join(","),
        );
        result.push_str("\r\n");
        for value in values {
            result.push_str(
                &fields
                    .iter()
                    .map(|k| match &value[*k] {
                        Value::Null => String::new(),
                        Value::String(s) => csv_cell(s),
                        v => csv_cell(&v.to_string()),
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            );
            result.push_str("\r\n");
        }
    }
    Ok(result)
}
fn csv_cell(value: &str) -> String {
    let formula = value.trim_start().starts_with(['=', '+', '-', '@'])
        || value.starts_with(['\t', '\r', '\n']);
    format!(
        "\"{}{}\"",
        if formula { "'" } else { "" },
        value.replace('"', "\"\"")
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn usage_snapshot_expiry_and_owner_are_enforced() -> Result<(), Box<dyn std::error::Error>> {
        let store = gateway_store::event_store::SqliteEventStore::open_in_memory()?;
        let query =
            serde_json::from_value(serde_json::json!({"from_ms":0,"to_ms":1000,"bucket_ms":1000}))?;
        let snapshot = gateway_control::usage_snapshot::read_usage_snapshot(&store, query)
            .map_err(|_| "snapshot")?;
        let mut cache = super::Snapshots::default();
        let (id, _, _) = cache
            .insert(snapshot, "owner-a".into(), 100)
            .map_err(|_| "insert")?;
        assert!(cache.get(&id, "owner-b").is_none());
        assert!(cache.get(&id, "owner-a").is_some());
        cache.0.get_mut(&id).ok_or("retained")?.deadline = std::time::Instant::now();
        assert!(cache.get(&id, "owner-a").is_none());
        assert!(cache.0.is_empty());
        Ok(())
    }

    #[test]
    fn usage_csv_neutralizes_formulas_and_preserves_quotes() {
        assert_eq!(super::csv_cell("  =2+2"), "\"'  =2+2\"");
        assert_eq!(super::csv_cell("a,\"b\"\n"), "\"a,\"\"b\"\"\n\"");
        assert_eq!(super::csv_cell("\tplain"), "\"'\tplain\"");
    }
}
