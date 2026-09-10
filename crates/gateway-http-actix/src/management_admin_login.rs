//! Narrow unauthenticated login route; every other management path stays behind admission.
use crate::management_security::{
    MANAGEMENT_KEY_HEADER, ManagementHttpState, management_denied_response, single_header,
};
use actix_web::{HttpMessage, HttpRequest, HttpResponse, http::header, web};
use futures_util::StreamExt;
use gateway_control::admin_login::{AdminLoginError, AdminLoginService};
use serde::Deserialize;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use zeroize::{Zeroize, Zeroizing};
const CSRF: &str = "x-management-csrf-token";
const MAX_BODY: usize = 2048;

#[derive(Clone)]
pub(crate) struct AdminLoginHttpState {
    pub(crate) service: Arc<AdminLoginService>,
    hashing: Arc<Semaphore>,
}
impl AdminLoginHttpState {
    pub(crate) fn new(service: AdminLoginService) -> Self {
        Self {
            service: Arc::new(service),
            hashing: Arc::new(Semaphore::new(2)),
        }
    }
}
/// Mount only the exact password login resource ahead of the protected `/admin` scope.
pub(crate) fn configure_login(config: &mut web::ServiceConfig) {
    config.route("/admin/auth/login", web::post().to(login));
}
pub(crate) fn configure_protected(config: &mut web::ServiceConfig) {
    config
        .route("/auth/password", web::post().to(change_password))
        .route("/auth/logout", web::post().to(logout));
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginBody {
    username: String,
    password: String,
}
impl Drop for LoginBody {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PasswordBody {
    current_password: String,
    new_password: String,
}
impl Drop for PasswordBody {
    fn drop(&mut self) {
        self.current_password.zeroize();
        self.new_password.zeroize();
    }
}

async fn read_body(
    request: &HttpRequest,
    mut payload: web::Payload,
) -> Result<Zeroizing<Vec<u8>>, HttpResponse> {
    if request.content_type() != "application/json" {
        return Err(invalid_request());
    }
    let read = async {
        let mut body = Zeroizing::new(Vec::new());
        while let Some(chunk) = payload.next().await {
            let chunk = chunk.map_err(|_| invalid_request())?;
            if body.len().saturating_add(chunk.len()) > MAX_BODY {
                return Err(invalid_request());
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    };
    tokio::time::timeout(Duration::from_secs(5), read)
        .await
        .map_err(|_| invalid_request())?
}
async fn login(
    request: HttpRequest,
    payload: web::Payload,
    state: Option<web::Data<ManagementHttpState>>,
) -> HttpResponse {
    let Some(state) = state else {
        return management_denied_response();
    };
    if state.admit_login(&request).is_err() {
        return management_denied_response();
    }
    let Some(login) = state.admin_login.as_ref() else {
        return error_response(AdminLoginError::Unavailable);
    };
    let Ok(permit) = Arc::clone(&login.hashing).try_acquire_owned() else {
        return error_response(AdminLoginError::RateLimited);
    };
    let bytes = match read_body(&request, payload).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Ok(body) = serde_json::from_slice::<LoginBody>(&bytes) else {
        return invalid_request();
    };
    if body.username.is_empty()
        || body.username.len() > 64
        || body.password.is_empty()
        || body.password.len() > 512
    {
        return invalid_request();
    }
    let service = Arc::clone(&login.service);
    match tokio::task::spawn_blocking(move || { let _permit = permit; service.login(&body.username, &body.password) }).await {
        Ok(Ok(grant)) => HttpResponse::Ok().insert_header((header::CACHE_CONTROL, "no-store"))
            .json(json!({"session_token": grant.session_token.as_str(), "csrf_token": grant.csrf_token.as_str(),
                "username": grant.username, "expires_at_ms": grant.expires_at_ms, "password_change_required": grant.password_change_required})),
        Ok(Err(error)) => error_response(error),
        Err(_) => error_response(AdminLoginError::Unavailable),
    }
}
fn session_headers(request: &HttpRequest) -> Option<(Zeroizing<String>, Zeroizing<String>)> {
    let token = single_header(request, MANAGEMENT_KEY_HEADER)
        .ok()??
        .to_str()
        .ok()?;
    let csrf = single_header(request, CSRF).ok()??.to_str().ok()?;
    if !token.starts_with("session_") {
        return None;
    }
    Some((
        Zeroizing::new(token.to_owned()),
        Zeroizing::new(csrf.to_owned()),
    ))
}
async fn change_password(
    request: HttpRequest,
    payload: web::Payload,
    state: web::Data<ManagementHttpState>,
) -> HttpResponse {
    let (Some(login), Some((token, csrf))) =
        (state.admin_login.as_ref(), session_headers(&request))
    else {
        return management_denied_response();
    };
    let Ok(permit) = Arc::clone(&login.hashing).try_acquire_owned() else {
        return error_response(AdminLoginError::RateLimited);
    };
    let bytes = match read_body(&request, payload).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Ok(body) = serde_json::from_slice::<PasswordBody>(&bytes) else {
        return invalid_request();
    };
    if body.current_password.len() > 512 || body.new_password.len() > 512 {
        return invalid_request();
    }
    let service = Arc::clone(&login.service);
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        service.change_password(&token, &csrf, &body.current_password, &body.new_password)
    })
    .await
    {
        Ok(Ok(())) => HttpResponse::NoContent()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .finish(),
        Ok(Err(error)) => error_response(error),
        Err(_) => error_response(AdminLoginError::Unavailable),
    }
}
async fn logout(request: HttpRequest, state: web::Data<ManagementHttpState>) -> HttpResponse {
    let (Some(login), Some((token, _))) = (state.admin_login.as_ref(), session_headers(&request))
    else {
        return management_denied_response();
    };
    match login.service.logout(&token) {
        Ok(()) => HttpResponse::NoContent()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .finish(),
        Err(error) => error_response(error),
    }
}
fn invalid_request() -> HttpResponse {
    HttpResponse::BadRequest().insert_header((header::CACHE_CONTROL, "no-store"))
        .json(json!({"error":{"code":"management_login_invalid_request","message":"Invalid login request"}}))
}
fn error_response(error: AdminLoginError) -> HttpResponse {
    let (status, code, message) = match error {
        AdminLoginError::InvalidCredentials => (
            401,
            "management_login_invalid_credentials",
            "Username or password is incorrect",
        ),
        AdminLoginError::InvalidPassword => (
            400,
            "management_login_invalid_password",
            "Use a different password with 12 to 128 characters",
        ),
        AdminLoginError::RateLimited => (
            429,
            "management_login_rate_limited",
            "Too many attempts; try again in a minute",
        ),
        AdminLoginError::Unavailable => (
            503,
            "management_login_unavailable",
            "Administrator login is unavailable",
        ),
        AdminLoginError::InvalidSession => return management_denied_response(),
    };
    let mut response = HttpResponse::build(
        actix_web::http::StatusCode::from_u16(status)
            .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR),
    );
    response.insert_header((header::CACHE_CONTROL, "no-store"));
    if status == 429 {
        response.insert_header((header::RETRY_AFTER, "60"));
    }
    response.json(json!({"error":{"code":code,"message":message}}))
}
