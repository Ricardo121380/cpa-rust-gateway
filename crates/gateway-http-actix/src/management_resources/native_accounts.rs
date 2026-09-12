//! Native Grok account enrollment uses the same encrypted store as the serving runtime.
use super::{ManagementResourceHttpState, invalid_input, parse_json, principal, read_operations};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use provider_grok::{Grok2ApiMemoryStreamMigration, GrokAccountPoolError, GrokAccountPoolStore};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zeroize::Zeroizing;

/// Per-process native store and cursor ownership; construction does not contact any provider.
pub struct NativeAccountManagement {
    pub(super) store: Arc<GrokAccountPoolStore>,
    pub(super) device: super::grok_device::DeviceSessions,
    epoch: String,
}
impl NativeAccountManagement {
    /// Replaces the fixed OAuth transport for controlled local tests or explicit deployment wiring.
    #[must_use]
    pub fn with_device_oauth_transport(
        mut self,
        transport: Arc<dyn provider_grok::GrokBuildOAuthTransport + Send + Sync>,
    ) -> Self {
        self.device = super::grok_device::DeviceSessions::with_transport(transport);
        self
    }

    /// Attaches a store and generates a fresh cursor namespace.
    /// # Errors
    /// Rejects unavailable operating-system randomness.
    pub fn new(store: Arc<GrokAccountPoolStore>) -> Result<Self, std::io::Error> {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(|_| std::io::Error::other("randomness unavailable"))?;
        Ok(Self {
            store,
            device: super::grok_device::DeviceSessions::new(),
            epoch: URL_SAFE_NO_PAD.encode(bytes),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Query {
    limit: Option<usize>,
    cursor: Option<String>,
    q: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Cursor {
    epoch: String,
    after: String,
    search: String,
    limit: usize,
    stamp: i64,
}
fn failure(status: StatusCode, code: &str) -> HttpResponse {
    let message = if code == "management_native_account_conflict" {
        "账号名称已存在或账号列表已改变，请刷新后核对。"
    } else {
        "Grok 账号服务暂不可用。"
    };
    HttpResponse::build(status)
        .insert_header(("Cache-Control", "no-store"))
        .json(serde_json::json!({"error":{"code":code,"message":message}}))
}
fn unavailable() -> HttpResponse {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "management_native_accounts_unavailable",
    )
}
fn conflict() -> HttpResponse {
    failure(StatusCode::CONFLICT, "management_native_account_conflict")
}

pub(super) async fn list(
    request: HttpRequest,
    query: web::Query<Query>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if let Err(response) = principal(&request) {
        return response;
    }
    let Some(native) = state.native_accounts.clone() else {
        return unavailable();
    };
    let limit = query.limit.unwrap_or(100);
    let search = query.q.clone().unwrap_or_default();
    if !(1..=100).contains(&limit) || search.len() > 256 {
        return invalid_input();
    }
    let cursor = match query.cursor.as_deref() {
        None => None,
        Some(value) if value.len() <= 2048 => match URL_SAFE_NO_PAD
            .decode(value)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Cursor>(&bytes).ok())
        {
            Some(value)
                if value.epoch == native.epoch
                    && value.search == search
                    && value.limit == limit =>
            {
                Some(value)
            }
            _ => return conflict(),
        },
        Some(_) => return invalid_input(),
    };
    let result=read_operations(&state,move || {
        let page=native.store.managed_account_page(limit,cursor.as_ref().map_or("",|c|c.after.as_str()),&search,cursor.as_ref().map(|c|c.stamp));
        Ok(page.and_then(|page| {
            let next=if page.has_more {page.items.last().and_then(|last|serde_json::to_vec(&Cursor {epoch:native.epoch.clone(),after:last.id.clone(),search,limit,stamp:page.stamp}).ok()).map(|bytes|URL_SAFE_NO_PAD.encode(bytes))} else {None};
            let items=page.items.into_iter().map(|row| { let identity=native.store.open_credential(&row.id).ok().map(|secret| {
                let fallback=gateway_store::account_identity::AccountIdentity::from_credential(secret.as_bytes());
                if row.provider==provider_grok::GrokAccountProvider::Build {
                    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().and_then(|v|i64::try_from(v.as_millis()).ok()).unwrap_or(0);
                    provider_grok::GrokBuildCredential::import_refreshable_runtime(secret.as_bytes(),now).ok().map_or(fallback,|c|c.identity().clone())
                } else { fallback }
            }).unwrap_or_default(); serde_json::json!({"id":row.id,"provider":match row.provider {provider_grok::GrokAccountProvider::Build=>"grok_build",provider_grok::GrokAccountProvider::Console=>"grok_console",provider_grok::GrokAccountProvider::Web=>"grok_web"},"auth_status":match row.auth_status {provider_grok::GrokAccountAuthStatus::Active=>"active",provider_grok::GrokAccountAuthStatus::ReauthRequired=>"reauth_required",provider_grok::GrokAccountAuthStatus::Disabled=>"disabled"},"enabled":row.enabled,"revision":row.revision,"import_batch_id":row.import_batch_id,"identity":identity})}).collect::<Vec<_>>();
            native.store.managed_account_page(1,"","",Some(page.stamp))?;
            Ok(serde_json::json!({"items":items,"next_cursor":next}))
        }))
    }).await;
    match result {
        Ok(Ok(page)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(page),
        Ok(Err(GrokAccountPoolError::ExistingAccountConflict)) => conflict(),
        _ => unavailable(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    id: String,
    channel: String,
    #[serde(deserialize_with = "secret_input")]
    secret: Zeroizing<String>,
}
fn secret_input<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Zeroizing<String>, D::Error> {
    String::deserialize(d).map(Zeroizing::new)
}
#[derive(Serialize)]
struct Transfer<'a> {
    kind: &'static str,
    source_ref: &'a str,
    provider: &'a str,
    identity_key: &'a str,
    credential: &'a str,
    auth_status: &'static str,
    enabled: bool,
    priority: i64,
    weight: u32,
    max_concurrency: u32,
}

pub(super) async fn import(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if let Err(response) = principal(&request) {
        return response;
    }
    let Some(native) = state.native_accounts.clone() else {
        return unavailable();
    };
    let input: Input = match parse_json(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if input.id.trim().is_empty()
        || input.id.len() > 128
        || input.secret.is_empty()
        || input.secret.len() > 65536
    {
        return invalid_input();
    }
    let provider = match input.channel.as_str() {
        "grok.build" => "grok_build",
        "grok.console" => "grok_console",
        "grok.web" => "grok_web",
        _ => return invalid_input(),
    };
    if provider == "grok_build" {
        return import_build(state, native, input).await;
    }
    let result = read_operations(&state, move || {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|v| i64::try_from(v.as_millis()).ok())
            .ok_or(super::ManagementOperationsError::SourceUnavailable)?;
        let record = Transfer {
            kind: "account",
            source_ref: &input.id,
            provider,
            identity_key: &input.id,
            credential: &input.secret,
            auth_status: "active",
            enabled: true,
            priority: 0,
            weight: 1,
            max_concurrency: 1,
        };
        let bytes = Zeroizing::new(
            serde_json::to_vec(&record)
                .map_err(|_| super::ManagementOperationsError::SourceUnavailable)?,
        );
        Ok(Grok2ApiMemoryStreamMigration::import(
            &native.store,
            &input.id,
            std::io::Cursor::new(bytes.as_slice()),
            now,
        ))
    })
    .await;
    match result {
        Ok(Ok(value))=>HttpResponse::Created().insert_header(("Cache-Control","no-store")).json(serde_json::json!({"created":value.created_accounts,"unchanged":value.unchanged_accounts})),
        Ok(Err(error)) if error.kind()==provider_grok::Grok2ApiMigrationFailureKind::ImportFailed=>conflict(),
        Ok(Err(_))=>invalid_input(),
        Err(_)=>unavailable(),
    }
}

// Build imports use the same identity capture and encrypted compact format as device grants.
async fn import_build(
    state: web::Data<ManagementResourceHttpState>,
    native: Arc<NativeAccountManagement>,
    input: Input,
) -> HttpResponse {
    let result = read_operations(&state, move || {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|v| i64::try_from(v.as_millis()).ok())
            .ok_or(super::ManagementOperationsError::SourceUnavailable)?;
        let Ok(mut credential) =
            provider_grok::GrokBuildCredential::import_active_runtime(input.secret.as_bytes(), now)
        else {
            return Ok(Err(GrokAccountPoolError::InvalidCredential));
        };
        native.device.acquire_identity(&mut credential);
        let account = provider_grok::GrokAccountImport {
            provider: provider_grok::GrokAccountProvider::Build,
            identity: provider_grok::GrokAccountIdentity::try_from_bytes(input.id.as_bytes())
                .map_err(|_| super::ManagementOperationsError::SourceUnavailable)?,
            credential: provider_grok::GrokAccountCredential::try_from_build_credential(
                &credential,
            )
            .map_err(|_| super::ManagementOperationsError::SourceUnavailable)?,
            auth_status: provider_grok::GrokAccountAuthStatus::Active,
            enabled: true,
            priority: 0,
            weight: 1,
            max_concurrency: 1,
            refresh_due_at_ms: Some(credential.expires_at_ms().saturating_sub(60_000).max(now)),
            quota_sync_due_at_ms: None,
            cooldown_until_ms: None,
        };
        Ok(native.store.import_batch(&input.id, &[account], now))
    })
    .await;
    match result {
        Ok(Ok(value)) => HttpResponse::Created()
            .insert_header(("Cache-Control", "no-store"))
            .json(serde_json::json!({"created":value.created,"unchanged":value.unchanged})),
        Ok(Err(GrokAccountPoolError::InvalidCredential)) => invalid_input(),
        Ok(Err(GrokAccountPoolError::ExistingAccountConflict)) => conflict(),
        _ => unavailable(),
    }
}
