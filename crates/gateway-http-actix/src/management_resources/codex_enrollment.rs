//! First authorization has no placeholder credential. The PKCE session is bound to
//! owner, proposed id, version and revision; only a completed exchange may import it.
use super::*;
use gateway_store::control_plane::ConfigVersionStatus;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartInput {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompleteInput {
    id: String,
    callback: CredentialOAuthCallbackRequest,
}

fn conflict() -> HttpResponse {
    error_response(
        StatusCode::CONFLICT,
        "management_enrollment_conflict",
        "授权上下文已改变或已结束，请重新开始",
    )
}

fn admit(
    state: &web::Data<ManagementResourceHttpState>,
    context: &WriteContext,
    owner: &UpstreamId,
    id: &CredentialId,
) -> Result<CredentialId, HttpResponse> {
    if !workflow(state)?.codex_enrollment_available() {
        return Err(error_response(
            StatusCode::NOT_IMPLEMENTED,
            "management_enrollment_unavailable",
            "当前未配置此授权方式",
        ));
    }
    let mut service = service(state)?;
    let version = service
        .repository_mut()
        .load_config_version(&context.version)
        .map_err(|e| management_error(e.into()))?
        .ok_or_else(conflict)?;
    if version.status != ConfigVersionStatus::Draft || version.revision != context.revision.as_i64()
    {
        return Err(conflict());
    }
    let upstream = service
        .get_upstream(&context.version, owner)
        .map_err(management_error)?;
    if !["codex", "chatgpt", "openai-compatible"].contains(&upstream.value().kind.as_str()) {
        return Err(invalid_input());
    }
    match service.get_credential(&context.version, id) {
        Err(ManagementResourceError::ResourceNotFound) => {}
        Ok(_) => return Err(conflict()),
        Err(e) => return Err(management_error(e)),
    }
    let mut hash = sha2::Sha256::new();
    hash.update(b"cpar-codex-first-authorization");
    for value in [context.version.as_str(), owner.as_str(), id.as_str()] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    hash.update(context.revision.as_i64().to_be_bytes());
    CredentialId::try_new(format!(
        "enroll-{}",
        URL_SAFE_NO_PAD.encode(hash.finalize())
    ))
    .map_err(|_| invalid_input())
}

pub(super) async fn start(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input: StartInput = match parse_json(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Ok(owner), Ok(id)) = (
        UpstreamId::try_new(path.into_inner()),
        CredentialId::try_new(input.id),
    ) else {
        return invalid_input();
    };
    let session = match admit(&state, &context, &owner, &id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let operation = match workflow(&state) {
        Ok(mut w) => w.start_oauth(&session),
        Err(r) => return r,
    };
    HttpResponse::Accepted()
        .insert_header(("Cache-Control", "no-store"))
        .json(CredentialOAuthResponse::new(&id, operation))
}

pub(super) async fn cancel(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input: StartInput = match parse_json(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Ok(owner), Ok(id)) = (
        UpstreamId::try_new(path.into_inner()),
        CredentialId::try_new(input.id),
    ) else {
        return invalid_input();
    };
    let session = match admit(&state, &context, &owner, &id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match workflow(&state) {
        Ok(mut w) => w.cancel_oauth(&session),
        Err(r) => return r,
    }
    HttpResponse::NoContent().finish()
}

pub(super) async fn complete(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input: CompleteInput = match parse_json(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Ok(owner), Ok(id)) = (
        UpstreamId::try_new(path.into_inner()),
        CredentialId::try_new(input.id),
    ) else {
        return invalid_input();
    };
    let session = match admit(&state, &context, &owner, &id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let callback = match parse_oauth_callback_request(&input.callback) {
        Ok(v) => v,
        Err(OAuthCallbackInputError::Invalid) => return invalid_input(),
        Err(OAuthCallbackInputError::ProviderRejected {
            state: callback_state,
        }) => {
            if let Some(decoded) = callback_state
                .as_deref()
                .and_then(|s| decode_oauth_state(s.as_bytes()))
                && let Ok(mut workflow) = workflow(&state)
            {
                let _ = workflow.reject_oauth(&session, &decoded);
            }
            return conflict();
        }
    };
    let Some(decoded) = decode_oauth_state(callback.state.as_bytes()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let worker = state.clone();
    // Token exchange is bounded by the existing operational-read semaphore and runs off Actix.
    let result = read_operations(&state, move || {
        let envelope = worker
            .workflow
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?
            .complete_oauth(&session, &decoded, Zeroizing::new(callback.code));
        let Some(envelope) = envelope else {
            return Ok(None);
        };
        let result = worker
            .service
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?
            .import_credential(
                &actor,
                &context.version,
                context.revision,
                owner,
                CredentialUpsert {
                    id,
                    kind: "oauth_json".to_owned(),
                    plaintext_secret: &envelope,
                    status: CredentialStatus::Active,
                },
            );
        let finalized = worker
            .workflow
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?
            .finalize_oauth(&session, result.is_ok());
        if result.is_ok() && !finalized {
            return Err(ManagementOperationsError::SourceUnavailable);
        }
        Ok(Some(result))
    })
    .await;
    match result {
        Ok(Some(Ok((value, created)))) => revisioned_json(
            if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            value,
            CredentialResponse::from,
        ),
        Ok(Some(Err(e))) => management_error(e),
        Ok(None) => conflict(),
        Err(e) => management_error(e.into()),
    }
}
