//! First authorization has no placeholder credential. The PKCE session is bound to
//! owner, proposed id, version and revision; only a completed exchange may import it.
use super::*;
use gateway_control::management_service::ManagementActor;
use gateway_store::control_plane::ConfigVersionStatus;
use provider_anthropic_compatible::ClaudeRuntimeCredential;
use provider_openai_compatible::OpenAiCompatibleRuntimeCredential;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartInput {
    id: String,
    #[serde(default)]
    replace_existing: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompleteInput {
    id: String,
    #[serde(default)]
    replace_existing: bool,
    callback: CredentialOAuthCallbackRequest,
}

#[derive(Clone, Copy)]
enum Channel {
    Codex,
    Claude,
}
impl Channel {
    fn workflows(
        self,
        state: &ManagementResourceHttpState,
    ) -> &Mutex<Box<dyn ManagementEndpointWorkflow>> {
        match self {
            Self::Codex => &state.workflow,
            Self::Claude => &state.claude_workflow,
        }
    }
    fn kind(self) -> &'static str {
        match self {
            Self::Codex => "oauth_json",
            Self::Claude => "bearer",
        }
    }
}
fn channel_workflow(
    state: &web::Data<ManagementResourceHttpState>,
    channel: Channel,
) -> Result<std::sync::MutexGuard<'_, Box<dyn ManagementEndpointWorkflow>>, HttpResponse> {
    channel
        .workflows(state)
        .lock()
        .map_err(|_| internal_error())
}
struct Admission {
    session: CredentialId,
    previous: Option<CredentialView>,
}

fn validated_legacy_owner(
    service: &mut ManagementMutationService,
    version: &ConfigVersionId,
    owner: &UpstreamId,
    credential: &CredentialView,
    channel: Channel,
) -> Result<bool, ManagementResourceError> {
    let upstream = service.get_upstream(version, owner)?;
    let plaintext = service.open_credential_for_export(version, &credential.id)?;
    match (channel, upstream.value().kind.as_str()) {
        (Channel::Codex, "openai-compatible") => Ok(matches!(
            OpenAiCompatibleRuntimeCredential::import_compatible(plaintext.as_bytes(), 0),
            Ok(OpenAiCompatibleRuntimeCredential::CodexOAuth(_))
        )),
        (Channel::Claude, "anthropic-compatible") => Ok(matches!(
            ClaudeRuntimeCredential::import_at(plaintext.as_bytes(), 0),
            Ok(ClaudeRuntimeCredential::OAuth(_))
        )),
        _ => Ok(false),
    }
}

fn same_observed_account(old: &[u8], new: &[u8]) -> bool {
    #[derive(Deserialize)]
    struct Binding {
        account_id: Option<String>,
    }
    let binding = |bytes: &[u8]| {
        serde_json::from_slice::<Binding>(bytes)
            .ok()
            .and_then(|b| b.account_id)
            .filter(|id| !id.is_empty())
    };
    let old_binding = binding(old);
    if old_binding.is_some() && old_binding != binding(new) {
        return false;
    }
    let identity = gateway_store::account_identity::AccountIdentity::from_credential(old);
    identity.email.is_none()
        || identity.email
            == gateway_store::account_identity::AccountIdentity::from_credential(new).email
}

pub(super) fn persist(
    service: &mut ManagementMutationService,
    actor: &ManagementActor,
    context: &WriteContext,
    owner: UpstreamId,
    input: CredentialUpsert<'_>,
    previous: Option<CredentialView>,
) -> Result<(Revisioned<CredentialView>, bool), ManagementResourceError> {
    if let Some(previous) = previous {
        let old = service.open_credential_for_export(&context.version, &input.id)?;
        if !same_observed_account(old.as_bytes(), input.plaintext_secret) {
            return Err(ManagementResourceError::InvalidCredentialInput);
        }
        service
            .update_credential_if_revision(
                actor,
                &context.version,
                context.revision,
                previous.revision,
                input,
            )
            .map(|value| (value, false))
    } else {
        service.import_credential(actor, &context.version, context.revision, owner, input)
    }
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
    channel: Channel,
    replace_existing: bool,
) -> Result<Admission, HttpResponse> {
    if !channel_workflow(state, channel)?.codex_enrollment_available() {
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
    let previous = match service.get_credential(&context.version, id) {
        Err(ManagementResourceError::ResourceNotFound) if !replace_existing => None,
        Ok(value)
            if replace_existing
                && value.value().upstream_id == *owner
                && value.value().kind == channel.kind() =>
        {
            Some(value.value().clone())
        }
        Ok(_) | Err(ManagementResourceError::ResourceNotFound) => return Err(conflict()),
        Err(e) => return Err(management_error(e)),
    };
    let upstream = service
        .get_upstream(&context.version, owner)
        .map_err(management_error)?;
    let allowed: &[&str] = match channel {
        Channel::Codex => &["codex", "chatgpt"],
        Channel::Claude => &["claude"],
    };
    let legacy = match previous.as_ref() {
        Some(credential) => {
            validated_legacy_owner(&mut service, &context.version, owner, credential, channel)
                .map_err(management_error)?
        }
        None => false,
    };
    if !allowed.contains(&upstream.value().kind.as_str()) && !legacy {
        return Err(invalid_input());
    }
    let mut hash = sha2::Sha256::new();
    hash.update(match channel {
        Channel::Codex => b"cpar-codex-first-authorization".as_slice(),
        Channel::Claude => b"cpar-claude-authorization".as_slice(),
    });
    if let Some(credential) = &previous {
        hash.update(credential.revision.to_be_bytes());
    }
    for value in [context.version.as_str(), owner.as_str(), id.as_str()] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    hash.update(context.revision.as_i64().to_be_bytes());
    CredentialId::try_new(format!(
        "enroll-{}",
        URL_SAFE_NO_PAD.encode(hash.finalize())
    ))
    .map(|session| Admission { session, previous })
    .map_err(|_| invalid_input())
}

fn start_for(
    channel: Channel,
    request: &HttpRequest,
    path: web::Path<String>,
    body: &web::Bytes,
    state: &web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(request) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input: StartInput = match parse_json(body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Ok(owner), Ok(id)) = (
        UpstreamId::try_new(path.into_inner()),
        CredentialId::try_new(input.id),
    ) else {
        return invalid_input();
    };
    let admission = match admit(
        state,
        &context,
        &owner,
        &id,
        channel,
        input.replace_existing,
    ) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let operation = match channel_workflow(state, channel) {
        Ok(mut w) => w.start_oauth(&admission.session),
        Err(r) => return r,
    };
    HttpResponse::Accepted()
        .insert_header(("Cache-Control", "no-store"))
        .json(CredentialOAuthResponse::new(&id, operation))
}

fn cancel_for(
    channel: Channel,
    request: &HttpRequest,
    path: web::Path<String>,
    body: &web::Bytes,
    state: &web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(request) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input: StartInput = match parse_json(body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Ok(owner), Ok(id)) = (
        UpstreamId::try_new(path.into_inner()),
        CredentialId::try_new(input.id),
    ) else {
        return invalid_input();
    };
    let admission = match admit(
        state,
        &context,
        &owner,
        &id,
        channel,
        input.replace_existing,
    ) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match channel_workflow(state, channel) {
        Ok(mut w) => w.cancel_oauth(&admission.session),
        Err(r) => return r,
    }
    HttpResponse::NoContent().finish()
}

fn callback_for(
    input: &CredentialOAuthCallbackRequest,
    state: &web::Data<ManagementResourceHttpState>,
    channel: Channel,
    session: &CredentialId,
) -> Result<ParsedOAuthCallback, HttpResponse> {
    match parse_oauth_callback_request(input) {
        Ok(v) => Ok(v),
        Err(OAuthCallbackInputError::Invalid) => Err(invalid_input()),
        Err(OAuthCallbackInputError::ProviderRejected {
            state: callback_state,
        }) => {
            if let Some(decoded) = callback_state
                .as_deref()
                .and_then(|s| decode_oauth_state(s.as_bytes()))
                && let Ok(mut workflow) = channel_workflow(state, channel)
            {
                let _ = workflow.reject_oauth(session, &decoded);
            }
            Err(conflict())
        }
    }
}

async fn complete_for(
    channel: Channel,
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
    let admission = match admit(
        &state,
        &context,
        &owner,
        &id,
        channel,
        input.replace_existing,
    ) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let callback = match callback_for(&input.callback, &state, channel, &admission.session) {
        Ok(value) => value,
        Err(response) => return response,
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
        let envelope = channel
            .workflows(&worker)
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?
            .complete_oauth(&admission.session, &decoded, Zeroizing::new(callback.code));
        let Some(envelope) = envelope else {
            return Ok(None);
        };
        let input = CredentialUpsert {
            id,
            kind: channel.kind().to_owned(),
            plaintext_secret: &envelope,
            status: if admission
                .previous
                .as_ref()
                .is_some_and(|p| p.status == CredentialStatus::Disabled)
            {
                CredentialStatus::Disabled
            } else {
                CredentialStatus::Active
            },
        };
        let result = persist(
            &mut *worker
                .service
                .lock()
                .map_err(|_| ManagementOperationsError::SourceUnavailable)?,
            &actor,
            &context,
            owner,
            input,
            admission.previous,
        );
        let finalized = channel
            .workflows(&worker)
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?
            .finalize_oauth(&admission.session, result.is_ok());
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

pub(super) async fn start(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    start_for(Channel::Codex, &request, path, &body, &state)
}

pub(super) async fn cancel(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    cancel_for(Channel::Codex, &request, path, &body, &state)
}

pub(super) async fn complete(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    complete_for(Channel::Codex, request, path, body, state).await
}

pub(super) async fn start_claude(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    start_for(Channel::Claude, &request, path, &body, &state)
}

pub(super) async fn cancel_claude(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    cancel_for(Channel::Claude, &request, path, &body, &state)
}

pub(super) async fn complete_claude(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    complete_for(Channel::Claude, request, path, body, state).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn replacement_must_preserve_an_observed_account_binding() {
        assert!(super::same_observed_account(
            br#"{"account_id":"a"}"#,
            br#"{"account_id":"a"}"#
        ));
        assert!(!super::same_observed_account(
            br#"{"account_id":"a"}"#,
            br#"{"account_id":"b"}"#
        ));
        assert!(!super::same_observed_account(
            br#"{"account_id":"a"}"#,
            b"{}"
        ));
    }
}
