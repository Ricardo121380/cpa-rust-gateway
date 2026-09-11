//! Secret-free administrative account status update.
use super::{
    CredentialId, CredentialResponse, CredentialStatus, ManagementOperationsError,
    ManagementResourceError, ManagementResourceHttpState, invalid_input, management_error,
    parse_json, principal, read_operations, revisioned_json, write_context,
};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    status: String,
    credential_revision: i64,
}

pub(super) async fn update(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let input: Input = match parse_json(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let status = match input.status.as_str() {
        "active" => CredentialStatus::Active,
        "disabled" => CredentialStatus::Disabled,
        _ => return invalid_input(),
    };
    if input.credential_revision < 0 {
        return invalid_input();
    }
    let Ok(id) = CredentialId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let worker = state.clone();
    let result = read_operations(&state, move || {
        let mut service = worker
            .service
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        Ok(service.set_credential_status(
            &actor,
            &context.version,
            context.revision,
            &id,
            input.credential_revision,
            status,
        ))
    })
    .await;
    match result {
        Ok(Ok(value)) => revisioned_json(StatusCode::OK, value, CredentialResponse::from),
        Ok(Err(error)) => management_error(error),
        Err(error) => management_error(ManagementResourceError::from(error)),
    }
}
