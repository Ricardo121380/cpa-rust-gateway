use super::{
    NativeAccountManagement, conflict, failure, invalid_input, now_ms, parse_json, principal,
    read_operations, secret_input, unavailable,
};
use crate::management_resources::ManagementResourceHttpState;
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use gateway_store::account_identity::AccountIdentity;
use provider_grok::{
    GrokAccountCredential, GrokAccountPoolError, GrokAccountProvider, GrokManagedAccountChange,
};
use serde::Deserialize;
use std::sync::Arc;
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnabledInput {
    revision: u64,
    enabled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialInput {
    revision: u64,
    #[serde(deserialize_with = "secret_input")]
    secret: Zeroizing<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::management_resources) struct RevisionQuery {
    revision: u64,
}

fn changed(
    id: &str,
    result: Result<(u64, bool), GrokAccountPoolError>,
    removed: bool,
    identity: Option<bool>,
) -> HttpResponse {
    match result {
        Ok((revision, runtime_applied)) => {
            let mut value = serde_json::json!({"account_id":id,"revision":revision,"removed":removed,"runtime_applied":runtime_applied});
            if let Some(observed) = identity {
                value["identity_state"] =
                    serde_json::json!(if observed { "observed" } else { "unavailable" });
            }
            HttpResponse::Ok()
                .insert_header(("Cache-Control", "no-store"))
                .json(value)
        }
        Err(GrokAccountPoolError::NotFound) => {
            failure(StatusCode::NOT_FOUND, "management_native_account_not_found")
        }
        Err(GrokAccountPoolError::ExistingAccountConflict) => conflict(),
        Err(GrokAccountPoolError::InvalidRequest | GrokAccountPoolError::InvalidCredential) => {
            invalid_input()
        }
        Err(_) => unavailable(),
    }
}

pub(in crate::management_resources) async fn set_enabled(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let Some(native) = state.native_accounts.clone() else {
        return unavailable();
    };
    let input: EnabledInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let Some(now) = now_ms() else {
        return unavailable();
    };
    let id = path.into_inner();
    let target = id.clone();
    let result = read_operations(&state, move || {
        Ok(native
            .store
            .manage_account(
                &target,
                input.revision,
                GrokManagedAccountChange::SetEnabled(input.enabled),
                actor.as_str(),
                now,
            )
            .map(|revision| (revision, native.apply_runtime(None))))
    })
    .await;
    match result {
        Ok(result) => changed(&id, result, false, None),
        Err(_) => unavailable(),
    }
}

pub(in crate::management_resources) async fn remove(
    request: HttpRequest,
    path: web::Path<String>,
    query: web::Query<RevisionQuery>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let Some(native) = state.native_accounts.clone() else {
        return unavailable();
    };
    let Some(now) = now_ms() else {
        return unavailable();
    };
    let id = path.into_inner();
    let target = id.clone();
    let revision = query.revision;
    let result = read_operations(&state, move || {
        Ok(native
            .store
            .manage_account(
                &target,
                revision,
                GrokManagedAccountChange::Remove,
                actor.as_str(),
                now,
            )
            .map(|revision| (revision, native.apply_runtime(None))))
    })
    .await;
    match result {
        Ok(result) => changed(&id, result, true, None),
        Err(_) => unavailable(),
    }
}

pub(super) async fn identity_for(
    native: &NativeAccountManagement,
    provider: GrokAccountProvider,
    credential: &GrokAccountCredential,
    now: i64,
) -> Option<AccountIdentity> {
    let request = credential.session_identity_request(provider, now).ok()?;
    native
        .identity_transport
        .as_ref()?
        .fetch(request)
        .await
        .ok()
}
fn different_identity(before: &AccountIdentity, after: &AccountIdentity) -> bool {
    matches!((&before.email,&after.email),(Some(a),Some(b)) if !a.eq_ignore_ascii_case(b))
        || matches!((&before.phone,&after.phone),(Some(a),Some(b)) if a!=b)
}

pub(in crate::management_resources) async fn replace_credential(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let actor = match principal(&request) {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let Some(native) = state.native_accounts.clone() else {
        return unavailable();
    };
    let input: CredentialInput = match parse_json(&body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let Some(now) = now_ms() else {
        return unavailable();
    };
    let id = path.into_inner();
    let target = id.clone();
    let store = Arc::clone(&native.store);
    let prepared = read_operations(&state, move || {
        Ok((|| {
            let current = store.identity_snapshot(&target)?;
            if current.revision != input.revision {
                return Err(GrokAccountPoolError::ExistingAccountConflict);
            }
            let previous = store.observed_identity(&target, now)?;
            let credential = GrokAccountCredential::try_from_sso(
                current.provider,
                input.secret.as_bytes(),
                now,
            )?;
            Ok((credential, current.provider, current.revision, previous))
        })())
    })
    .await;
    let (credential, provider, revision, previous) = match prepared {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => return changed(&id, Err(error), false, None),
        Err(_) => return unavailable(),
    };
    let identity = identity_for(&native, provider, &credential, now).await;
    if identity
        .as_ref()
        .is_some_and(|identity| different_identity(&previous, identity))
    {
        return HttpResponse::Conflict().insert_header(("Cache-Control","no-store")).json(serde_json::json!({"error":{"code":"management_native_account_identity_conflict","message":"凭据属于另一个账号，请使用添加账号。"}}));
    }
    let observed = identity
        .as_ref()
        .is_some_and(|identity| !identity.is_empty());
    let target = id.clone();
    let result = read_operations(&state, move || {
        Ok(native
            .store
            .manage_account(
                &target,
                revision,
                GrokManagedAccountChange::ReplaceSso {
                    credential: &credential,
                    identity: identity.as_ref(),
                },
                actor.as_str(),
                now,
            )
            .map(|revision| {
                (
                    revision,
                    native.apply_runtime(observed.then_some(target.as_str())),
                )
            }))
    })
    .await;
    match result {
        Ok(result) => changed(&id, result, false, Some(observed)),
        Err(_) => unavailable(),
    }
}

pub(in crate::management_resources) async fn audit(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if let Err(response) = principal(&request) {
        return response;
    }
    let Some(native) = state.native_accounts.clone() else {
        return unavailable();
    };
    let id = path.into_inner();
    match read_operations(&state, move || {
        Ok(native.store.account_management_events(&id))
    })
    .await
    {
        Ok(Ok(events)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(events),
        _ => unavailable(),
    }
}
pub(in crate::management_resources) async fn apply_runtime(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if let Err(response) = principal(&request) {
        return response;
    }
    let Some(native) = state.native_accounts.clone() else {
        return unavailable();
    };
    match read_operations(&state, move || Ok(native.apply_runtime(None))).await {
        Ok(applied) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(serde_json::json!({"runtime_applied":applied})),
        _ => unavailable(),
    }
}
