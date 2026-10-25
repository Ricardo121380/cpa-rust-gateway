//! Native quota reads are bound to an exact account revision and never update serving grants.
use super::{
    Deserialize, HttpRequest, HttpResponse, ManagementResourceHttpState, conflict, invalid_input,
    principal, read_operations, unavailable, web,
};
use crate::management_resources::account_quota::AccountQuotaObservation;
use provider_grok::GrokAccountIdentitySnapshot;
use std::{future::Future, pin::Pin};

/// Bounded native-account metadata transport, injected by the deployment composition root.
pub trait NativeUsageTransport: Send + Sync {
    /// Reads quota only. Implementations must use fixed provider URLs and never run inference.
    fn read(
        &self,
        snapshot: GrokAccountIdentitySnapshot,
    ) -> Pin<Box<dyn Future<Output = Result<AccountQuotaObservation, NativeUsageError>> + Send + '_>>;
}
/// Value-free upstream failures suitable for management UI.
#[derive(Clone, Copy, Debug)]
pub enum NativeUsageError {
    /// Provider rejected the authorization.
    Unauthorized,
    /// Provider or its WAF denied the read.
    Forbidden,
    /// Response shape could not be interpreted safely.
    InvalidResponse,
    /// Transport timeout, admission or capacity failure.
    Unavailable,
}
#[derive(Deserialize)]
pub(crate) struct UsageQuery {
    revision: u64,
}

pub(crate) async fn usage(
    request: HttpRequest,
    query: web::Query<UsageQuery>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if let Err(response) = principal(&request) {
        return response;
    }
    let Some(native) = state.native_accounts.clone() else {
        return unavailable();
    };
    let Some(transport) = native.usage_transport.clone() else {
        return usage_failure("not_connected");
    };
    let Some(id) = request
        .match_info()
        .get("account_id")
        .filter(|v| !v.is_empty() && v.len() <= 128)
    else {
        return invalid_input();
    };
    let id = id.to_owned();
    let store = native.store.clone();
    let read_id = id.clone();
    let Ok(Ok(snapshot)) =
        read_operations(&state, move || Ok(store.identity_snapshot(&read_id))).await
    else {
        return unavailable();
    };
    if snapshot.revision != query.revision {
        return conflict();
    }
    let Ok(_permit) = state.catalog_refresh_slots.clone().try_acquire_owned() else {
        return usage_failure("busy");
    };
    let provider = match snapshot.provider {
        provider_grok::GrokAccountProvider::Build => "grok_build",
        provider_grok::GrokAccountProvider::Console => "grok_console",
        provider_grok::GrokAccountProvider::Web => "grok_web",
    };
    let fence = if let Some(history) = state.quota_history.clone() {
        let account = gateway_store::quota_history::AccountScope {
            account_kind: "native".into(),
            config_version_id: String::new(),
            account_id: id.clone(),
        };
        let Ok(revision) = i64::try_from(query.revision) else {
            return invalid_input();
        };
        match read_operations(&state, move || Ok(history.capture(account, Some(revision)))).await {
            Ok(Ok(fence)) => Some(fence),
            Ok(Err(gateway_store::quota_history::Error::Conflict)) => return conflict(),
            _ => return unavailable(),
        }
    } else {
        None
    };
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(20), transport.read(snapshot)).await;
    // Reject a late response after replacement, disable or deletion, including errors.
    let store = native.store.clone();
    let current = read_operations(&state, move || Ok(store.identity_snapshot(&id))).await;
    if !matches!(current,Ok(Ok(value)) if value.revision==query.revision) {
        return conflict();
    }
    let Ok(completed_at) = state.runtime_clock.now_ms() else {
        return unavailable();
    };
    if let (Some(history), Some(fence)) = (state.quota_history.clone(), fence) {
        let samples = match &result {
            Ok(Ok(observation)) => observation.history_samples(),
            _ => vec![gateway_store::quota_history::Sample::missing(
                provider,
                completed_at,
                true,
            )],
        };
        match read_operations(&state, move || Ok(history.append(&fence, &samples))).await {
            Ok(Ok(())) => {}
            Ok(Err(gateway_store::quota_history::Error::Conflict)) => return conflict(),
            _ => return unavailable(),
        }
    }
    match result {
        Ok(Ok(observation)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(serde_json::json!({"observation":observation,"error":null})),
        Ok(Err(NativeUsageError::Unauthorized)) => usage_failure("unauthorized"),
        Ok(Err(NativeUsageError::Forbidden)) => usage_failure("forbidden"),
        Ok(Err(NativeUsageError::InvalidResponse)) => usage_failure("invalid_response"),
        _ => usage_failure("unavailable"),
    }
}
fn usage_failure(code: &str) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(serde_json::json!({"observation":null,"error":code}))
}
