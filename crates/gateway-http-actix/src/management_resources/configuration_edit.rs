//! Start a complete, guarded edit without copying secrets through the browser.
use super::{
    ConfigVersionId, ManagementOperationsError, ManagementResourceError,
    ManagementResourceHttpState, invalid_input, management_error, parse_json, principal,
    read_operations, write_context,
};
use actix_web::{HttpRequest, HttpResponse, http::header, web};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ForkInput {
    id: String,
    description: String,
}

pub(super) async fn fork(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if path.as_str() != context.version.as_str() {
        return invalid_input();
    }
    let input: ForkInput = match parse_json(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if input.id.trim().is_empty() || input.id.len() > 128 || input.description.len() > 1024 {
        return invalid_input();
    }
    let Ok(target) = ConfigVersionId::try_new(input.id) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let worker_state = state.clone();
    let result = read_operations(&state, move || {
        let mut service = worker_state
            .service
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        Ok(service.fork_active_configuration(
            &actor,
            &context.version,
            context.revision,
            target,
            input.description,
        ))
    })
    .await;
    match result {
        Ok(Ok(version)) => HttpResponse::Created().insert_header((header::CACHE_CONTROL, "no-store")).json(serde_json::json!({
            "id":version.id.as_str(), "parent_id":version.parent_id.as_ref().map(ConfigVersionId::as_str),
            "status":"draft", "revision":format!("rev-{}",version.revision),
            "created_at_ms":version.created_at_ms, "description":version.description,
        })),
        Ok(Err(error)) => management_error(error),
        Err(error) => management_error(ManagementResourceError::from(error)),
    }
}
