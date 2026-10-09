//! Exact, value-free native deletion review; shared graph resources are never removed here.
use super::{NativeAccountManagement, conflict, unavailable};
use crate::management_resources::{
    ManagementResourceHttpState, principal, read_context, read_operations,
};
use actix_web::{HttpRequest, HttpResponse, web};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_store::control_plane::{ConfigVersionId, ControlPlaneConfiguration};
use provider_grok::{
    GrokAccountAuthStatus, GrokAccountPoolError, GrokAccountPoolStore, GrokAccountProvider,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn preview(
    config: &ControlPlaneConfiguration,
    store: &GrokAccountPoolStore,
    id: &str,
    revision: u64,
) -> Result<Value, GrokAccountPoolError> {
    let mut accounts = Vec::new();
    let mut after = String::new();
    let mut stamp = None;
    loop {
        let page = store.managed_account_page(100, &after, "", stamp)?;
        stamp = Some(page.stamp);
        accounts.extend(page.items);
        if !page.has_more {
            break;
        }
        if accounts.len() >= 10_000 {
            return Err(GrokAccountPoolError::InvalidRequest);
        }
        after.clone_from(
            &accounts
                .last()
                .ok_or(GrokAccountPoolError::InvalidPersistedState)?
                .id,
        );
    }
    let target = accounts
        .iter()
        .find(|account| account.id == id)
        .ok_or(GrokAccountPoolError::NotFound)?;
    if target.revision != revision {
        return Err(GrokAccountPoolError::ExistingAccountConflict);
    }
    let provider = match target.provider {
        GrokAccountProvider::Build => "build",
        GrokAccountProvider::Console => "console",
        GrokAccountProvider::Web => "web",
    };
    let kind = format!("grok-{provider}-native");
    let adapter = format!("grok.{provider}.responses");
    let connections: Vec<Value> = config.endpoints.iter().filter_map(|endpoint| {
        if endpoint.adapter_id != adapter { return None; }
        let upstream = config.upstreams.iter().find(|upstream| upstream.id == endpoint.upstream_id && upstream.kind == kind)?;
        let models: Vec<Value> = config.route_candidates.iter().filter(|candidate| candidate.endpoint_id == endpoint.id).filter_map(|candidate| {
            let route = config.model_routes.iter().find(|route| route.id == candidate.route_id)?;
            let model = config.public_models.iter().find(|model| model.id == route.public_model_id)?;
            Some(json!({"route_id":route.id.as_str(),"model":model.model_name,"upstream_model":candidate.upstream_model,"candidate_enabled":candidate.enabled}))
        }).collect();
        Some(json!({"upstream_id":upstream.id.as_str(),"service_name":upstream.name,"upstream_enabled":upstream.enabled,"endpoint_id":endpoint.id.as_str(),"endpoint_enabled":endpoint.enabled,"models":models}))
    }).collect();
    let remaining: Vec<Value> = accounts.iter().filter(|account| account.id != id && account.provider == target.provider).map(|account| json!({"id":account.id,"revision":account.revision,"enabled":account.enabled,"auth_status":match account.auth_status {GrokAccountAuthStatus::Active => "active", GrokAccountAuthStatus::ReauthRequired => "reauth_required", GrokAccountAuthStatus::Disabled => "disabled"}})).collect();
    let mut value = json!({"config_version":config.version.id.as_str(),"config_revision":config.version.revision,"inventory_generation":stamp,"account_id":id,"account_revision":revision,"provider":provider,"connections":connections,"remaining_accounts":remaining,"replacement_guaranteed":false,"history_retained":true,"shared_resources_retained":true,"upstream_account_revoked":false});
    let bytes =
        serde_json::to_vec(&value).map_err(|_| GrokAccountPoolError::InvalidPersistedState)?;
    value["review_token"] = json!(URL_SAFE_NO_PAD.encode(Sha256::digest(bytes)));
    Ok(value)
}

#[allow(
    clippy::too_many_arguments,
    reason = "review and removal share exact graph, target, actor and time ownership"
)]
pub(super) fn reviewed_remove(
    state: &web::Data<ManagementResourceHttpState>,
    native: &NativeAccountManagement,
    version: &ConfigVersionId,
    id: &str,
    revision: u64,
    token: &str,
    actor: &str,
    now: i64,
) -> Result<u64, GrokAccountPoolError> {
    // Hold the graph mutation lock through native target CAS, as ordinary deletion does.
    let mut service = state
        .service
        .lock()
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
    let config = service
        .repository_mut()
        .load_configuration(version)
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
        .ok_or(GrokAccountPoolError::NotFound)?;
    let impact = preview(&config, &native.store, id, revision)?;
    if impact["review_token"].as_str() != Some(token) {
        return Err(GrokAccountPoolError::ExistingAccountConflict);
    }
    native.store.remove_reviewed_account(
        id,
        revision,
        impact["inventory_generation"]
            .as_i64()
            .ok_or(GrokAccountPoolError::InvalidPersistedState)?,
        actor,
        now,
    )
}

pub(in crate::management_resources) async fn read(
    request: HttpRequest,
    path: web::Path<String>,
    query: web::Query<super::maintenance::RevisionQuery>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if let Err(response) = principal(&request) {
        return response;
    }
    let context = match read_context(&request) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Some(native) = state.native_accounts.clone() else {
        return unavailable();
    };
    let owned = state.clone();
    let result = read_operations(&state, move || {
        let result = (|| {
            let mut service = owned
                .service
                .lock()
                .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
            let config = service
                .repository_mut()
                .load_configuration(&context.version)
                .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
                .ok_or(GrokAccountPoolError::NotFound)?;
            preview(&config, &native.store, &path, query.revision)
        })();
        Ok(result)
    })
    .await;
    match result {
        Ok(Ok(value)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(value),
        Ok(Err(GrokAccountPoolError::ExistingAccountConflict)) => conflict(),
        Ok(Err(GrokAccountPoolError::NotFound)) => super::failure(
            actix_web::http::StatusCode::NOT_FOUND,
            "management_native_account_not_found",
        ),
        _ => unavailable(),
    }
}
