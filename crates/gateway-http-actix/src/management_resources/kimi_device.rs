//! Kimi Coding OAuth device authorization for a dedicated Kimi upstream.
//!
//! Sessions are process-local, revision-bound and secret-free at the HTTP boundary. The browser
//! receives only a user code and a verified Kimi URL; access, refresh and device material stay in
//! this module until the final encrypted credential write.

use super::*;
use gateway_store::control_plane::ConfigVersionStatus;
use std::io::Read;
use zeroize::Zeroize;

const DEVICE_AUTHORIZATION_URL: &str = "https://auth.kimi.com/api/oauth/device_authorization";
const TOKEN_URL: &str = "https://auth.kimi.com/api/oauth/token";
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;
const MAX_SESSIONS: usize = 128;
const MAX_TOKEN_LIFETIME_MS: i64 = 31 * 24 * 60 * 60 * 1_000;

type Transport = dyn Fn(&str, &str, &str) -> Result<KimiDeviceHttpResponse, ()> + Send + Sync;

/// One bounded Kimi OAuth HTTP response. Its body is processed and wiped inside the workflow.
pub struct KimiDeviceHttpResponse {
    status: u16,
    body: serde_json::Value,
}

impl KimiDeviceHttpResponse {
    /// Creates an injected fixed-origin OAuth response for protocol tests.
    #[must_use]
    pub const fn new(status: u16, body: serde_json::Value) -> Self {
        Self { status, body }
    }
}

/// Bounded Kimi sessions for device OAuth. No fields are returned through a management read.
pub struct KimiDeviceWorkflow {
    sessions: BTreeMap<String, Session>,
    transport: Box<Transport>,
}

struct Session {
    scope: String,
    device_code: Zeroizing<String>,
    device_id: Zeroizing<String>,
    expires_at_ms: i64,
    next_poll_at_ms: i64,
    interval_ms: i64,
}

impl KimiDeviceWorkflow {
    /// Creates a workflow with a fixed injectable transport for protocol-level tests.
    #[must_use]
    pub fn with_transport(transport: Box<Transport>) -> Self {
        Self {
            sessions: BTreeMap::new(),
            transport,
        }
    }

    /// Creates the real fixed-origin Kimi device OAuth workflow.
    #[must_use]
    pub fn production() -> Self {
        Self::with_transport(Box::new(post_form))
    }

    fn purge_expired(&mut self, clock: i64) {
        self.sessions
            .retain(|_, session| session.expires_at_ms > clock);
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| i64::try_from(value.as_millis()).ok())
        .unwrap_or(0)
}

fn form(fields: &[(&str, &str)]) -> Zeroizing<String> {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in fields {
        serializer.append_pair(name, value);
    }
    Zeroizing::new(serializer.finish())
}

fn post_form(url: &str, body: &str, device_id: &str) -> Result<KimiDeviceHttpResponse, ()> {
    if !matches!(url, DEVICE_AUTHORIZATION_URL | TOKEN_URL) || device_id.trim().is_empty() {
        return Err(());
    }
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| ())?;
    let response = client
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .header("User-Agent", "cpa-rust-gateway/kimi-oauth")
        .header("X-Msh-Platform", "CPAR")
        .header("X-Msh-Device-Name", "CPAR Gateway")
        .header("X-Msh-Device-Model", "gateway")
        .header("X-Msh-Device-Id", device_id)
        .body(body.as_bytes().to_vec())
        .send()
        .map_err(|_| ())?;
    let status = response.status().as_u16();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES)
    {
        return Err(());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_RESPONSE_BYTES {
        return Err(());
    }
    let body = serde_json::from_slice(bytes.as_slice()).map_err(|_| ())?;
    Ok(KimiDeviceHttpResponse { status, body })
}

fn wipe(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => text.zeroize(),
        serde_json::Value::Array(values) => values.iter_mut().for_each(wipe),
        serde_json::Value::Object(values) => values.values_mut().for_each(wipe),
        _ => {}
    }
}

fn bounded_text(value: &serde_json::Value, name: &str) -> Result<Zeroizing<String>, ()> {
    value
        .get(name)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty() && value.len() <= 16 * 1024)
        .map(|value| Zeroizing::new(value.to_owned()))
        .ok_or(())
}

fn verification_uri(value: &serde_json::Value) -> Result<Zeroizing<String>, ()> {
    let uri = value
        .get("verification_uri_complete")
        .or_else(|| value.get("verification_uri"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| value.len() <= 2048)
        .ok_or(())?;
    let parsed = url::Url::parse(uri).map_err(|_| ())?;
    if parsed.scheme() != "https"
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.port().is_some()
        || !parsed
            .host_str()
            .is_some_and(|host| host == "kimi.com" || host.ends_with(".kimi.com"))
    {
        return Err(());
    }
    Ok(Zeroizing::new(uri.to_owned()))
}

fn random_id() -> Result<String, ()> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| ())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    id: String,
    #[serde(default)]
    replace_existing: bool,
    session_id: Option<String>,
}

fn conflict() -> HttpResponse {
    error_response(
        StatusCode::CONFLICT,
        "management_kimi_authorization_conflict",
        "授权已结束或配置已改变，请重新开始",
    )
}

fn unavailable() -> HttpResponse {
    error_response(
        StatusCode::BAD_GATEWAY,
        "management_kimi_authorization_unavailable",
        "Kimi 授权服务暂不可用；未自动重试",
    )
}

fn admit(
    state: &web::Data<ManagementResourceHttpState>,
    context: &WriteContext,
    owner: &UpstreamId,
    id: &CredentialId,
    replace_existing: bool,
) -> Result<Option<CredentialView>, HttpResponse> {
    let mut service = service(state)?;
    let version = service
        .repository_mut()
        .load_config_version(&context.version)
        .map_err(|error| management_error(error.into()))?
        .ok_or_else(conflict)?;
    if version.status != ConfigVersionStatus::Draft || version.revision != context.revision.as_i64()
    {
        return Err(conflict());
    }
    if service
        .get_upstream(&context.version, owner)
        .map_err(management_error)?
        .value()
        .kind
        != "kimi-coding"
    {
        return Err(invalid_input());
    }
    match service.get_credential(&context.version, id) {
        Err(ManagementResourceError::ResourceNotFound) if !replace_existing => Ok(None),
        Ok(value)
            if replace_existing
                && value.value().upstream_id == *owner
                && value.value().kind == "oauth_json" =>
        {
            Ok(Some(value.value().clone()))
        }
        Ok(_) | Err(ManagementResourceError::ResourceNotFound) => Err(conflict()),
        Err(error) => Err(management_error(error)),
    }
}

#[derive(Clone, Copy)]
enum Action {
    Start,
    Poll,
    Cancel,
}

pub(super) async fn start(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    handle(Action::Start, request, path, body, state).await
}

pub(super) async fn poll(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    handle(Action::Poll, request, path, body, state).await
}

pub(super) async fn cancel(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    handle(Action::Cancel, request, path, body, state).await
}

#[allow(clippy::too_many_lines)]
async fn handle(
    action: Action,
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let input = match parse_json::<Input>(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if input.id.trim().is_empty()
        || input.id.len() > 128
        || input
            .session_id
            .as_ref()
            .is_some_and(|value| value.len() > 128)
    {
        return invalid_input();
    }
    let (Ok(owner), Ok(id)) = (
        UpstreamId::try_new(path.into_inner()),
        CredentialId::try_new(input.id),
    ) else {
        return invalid_input();
    };
    let previous = match admit(&state, &context, &owner, &id, input.replace_existing) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let session_revision = if matches!(action, Action::Start) {
        let mut service = match super::service(&state) {
            Ok(value) => value,
            Err(response) => return response,
        };
        match service.record_draft_resource_action(
            &actor,
            &context.version,
            context.revision,
            "kimi_authorization_started",
            "upstream",
            owner.as_str(),
        ) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        }
    } else {
        context.revision
    };
    let scope = serde_json::json!([
        context.version.as_str(),
        session_revision.as_i64(),
        owner.as_str(),
        id.as_str(),
        actor.as_str(),
        previous.as_ref().map(|value| value.revision),
    ])
    .to_string();
    let worker = state.clone();
    let revision = session_revision;
    let result = read_operations(&state, move || {
        let Some(shared) = &worker.kimi_workflow else {
            return Ok(Err(WorkflowError::Conflict));
        };
        let mut workflow = shared
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        let clock = now_ms();
        workflow.purge_expired(clock);
        match action {
            Action::Start => {
                if workflow.sessions.len() >= MAX_SESSIONS
                    || workflow.sessions.values().any(|session| session.scope == scope)
                {
                    return Ok(Err(WorkflowError::Conflict));
                }
                let session_id = random_id().map_err(|()| ManagementOperationsError::SourceUnavailable)?;
                let device_id = random_id().map_err(|()| ManagementOperationsError::SourceUnavailable)?;
                let form = form(&[("client_id", provider_openai_compatible::KIMI_OAUTH_CLIENT_ID)]);
                let mut response = (workflow.transport)(
                    DEVICE_AUTHORIZATION_URL,
                    form.as_str(),
                    device_id.as_str(),
                )
                .map_err(|()| ManagementOperationsError::SourceUnavailable)?;
                if !(200..300).contains(&response.status) {
                    wipe(&mut response.body);
                    return Err(ManagementOperationsError::SourceUnavailable);
                }
                let parsed: Result<Outcome, ()> = (|| {
                    let device_code = bounded_text(&response.body, "device_code")?;
                    let user_code = bounded_text(&response.body, "user_code")?;
                    let uri = verification_uri(&response.body)?;
                    let expires_in = response.body
                        .get("expires_in")
                        .and_then(serde_json::Value::as_i64)
                        .filter(|value| (1..=3600).contains(value))
                        .ok_or(())?;
                    let interval_ms = response.body
                        .get("interval")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(5)
                        .clamp(5, 60)
                        .saturating_mul(1_000);
                    let expires_at_ms = clock.checked_add(expires_in.saturating_mul(1_000)).ok_or(())?;
                    workflow.sessions.insert(
                        session_id.clone(),
                        Session {
                            scope,
                            device_code,
                            device_id: Zeroizing::new(device_id),
                            expires_at_ms,
                            next_poll_at_ms: clock.saturating_add(interval_ms),
                            interval_ms,
                        },
                    );
                    Ok(Outcome::State(serde_json::json!({
                        "state": "pending",
                        "session_id": session_id,
                        "user_code": user_code.as_str(),
                        "verification_uri": uri.as_str(),
                        "expires_at_ms": expires_at_ms,
                        "interval_ms": interval_ms,
                    })))
                })();
                wipe(&mut response.body);
                Ok(parsed.map_err(|()| WorkflowError::Conflict))
            }
            Action::Cancel => {
                let Some(session_id) = input.session_id else {
                    return Ok(Err(WorkflowError::Conflict));
                };
                if workflow
                    .sessions
                    .get(&session_id)
                    .is_none_or(|session| session.scope != scope)
                {
                    return Ok(Err(WorkflowError::Conflict));
                }
                workflow.sessions.remove(&session_id);
                Ok(Ok(Outcome::State(serde_json::json!({"state": "cancelled"}))))
            }
            Action::Poll => {
                let Some(session_id) = input.session_id else {
                    return Ok(Err(WorkflowError::Conflict));
                };
                let (form, device_id) = {
                    let Some(session) = workflow.sessions.get_mut(&session_id) else {
                        return Ok(Err(WorkflowError::Conflict));
                    };
                    if session.scope != scope {
                        return Ok(Err(WorkflowError::Conflict));
                    }
                    if clock < session.next_poll_at_ms {
                        return Ok(Ok(Outcome::State(serde_json::json!({
                            "state": "pending",
                            "interval_ms": session.next_poll_at_ms.saturating_sub(clock),
                        }))));
                    }
                    session.next_poll_at_ms = clock.saturating_add(session.interval_ms);
                    (
                        form(&[
                            ("client_id", provider_openai_compatible::KIMI_OAUTH_CLIENT_ID),
                            ("device_code", session.device_code.as_str()),
                            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                        ]),
                        Zeroizing::new(session.device_id.to_string()),
                    )
                };
                let Ok(mut response) = (workflow.transport)(TOKEN_URL, form.as_str(), device_id.as_str()) else {
                    // A transport result cannot prove whether the upstream consumed the device
                    // grant. Do not retry it automatically or leave a background session that
                    // could later persist unseen material; require a fresh explicit authorization.
                    workflow.sessions.remove(&session_id);
                    return Err(ManagementOperationsError::SourceUnavailable);
                };
                let provider_error = response.body
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let recognized_oauth_error = matches!(
                    provider_error.as_str(),
                    "authorization_pending" | "slow_down" | "expired_token" | "access_denied"
                );
                if recognized_oauth_error && !matches!(response.status, 200 | 400) {
                    wipe(&mut response.body);
                    return Err(ManagementOperationsError::SourceUnavailable);
                }
                if provider_error == "authorization_pending" || provider_error == "slow_down" {
                    if provider_error == "slow_down"
                        && let Some(session) = workflow.sessions.get_mut(&session_id)
                    {
                        session.interval_ms = session.interval_ms.saturating_add(5_000).min(60_000);
                        session.next_poll_at_ms = clock.saturating_add(session.interval_ms);
                    }
                    let interval_ms = workflow
                        .sessions
                        .get(&session_id)
                        .map_or(5_000, |session| session.interval_ms);
                    wipe(&mut response.body);
                    return Ok(Ok(Outcome::State(serde_json::json!({
                        "state": "pending",
                        "interval_ms": interval_ms,
                    }))));
                }
                if matches!(provider_error.as_str(), "expired_token" | "access_denied") {
                    workflow.sessions.remove(&session_id);
                    wipe(&mut response.body);
                    return Ok(Ok(Outcome::State(serde_json::json!({
                        "state": if provider_error == "expired_token" { "expired" } else { "denied" },
                    }))));
                }
                if !provider_error.is_empty() {
                    workflow.sessions.remove(&session_id);
                    wipe(&mut response.body);
                    return Ok(Ok(Outcome::State(serde_json::json!({"state": "failed"}))));
                }
                let material: Result<Zeroizing<Vec<u8>>, ()> = (|| {
                    if !(200..300).contains(&response.status) {
                        return Err(());
                    }
                    let access_token = bounded_text(&response.body, "access_token")?;
                    let refresh_token = bounded_text(&response.body, "refresh_token")?;
                    let expires_in = response.body
                        .get("expires_in")
                        .and_then(serde_json::Value::as_i64)
                        .filter(|value| (1..=MAX_TOKEN_LIFETIME_MS / 1_000).contains(value))
                        .ok_or(())?;
                    let expires_at_ms = clock.checked_add(expires_in.saturating_mul(1_000)).ok_or(())?;
                    let mut envelope = serde_json::json!({
                        "kind": "kimi_oauth",
                        "access_token": access_token.as_str(),
                        "refresh_token": refresh_token.as_str(),
                        "expires_at_ms": expires_at_ms,
                        "device_id": device_id.as_str(),
                    });
                    let bytes = Zeroizing::new(serde_json::to_vec(&envelope).map_err(|_| ())?);
                    wipe(&mut envelope);
                    provider_openai_compatible::OpenAiCompatibleRuntimeCredential::import_compatible(
                        bytes.as_slice(),
                        clock,
                    )
                    .map_err(|_| ())?;
                    Ok(bytes)
                })();
                wipe(&mut response.body);
                workflow.sessions.remove(&session_id);
                let Ok(material) = material else {
                    return Ok(Ok(Outcome::State(serde_json::json!({"state":"failed"}))));
                };
                let mut service = worker
                    .service
                    .lock()
                    .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
                let status = if previous
                    .as_ref()
                    .is_some_and(|value| value.status == CredentialStatus::Disabled)
                {
                    CredentialStatus::Disabled
                } else {
                    CredentialStatus::Active
                };
                let persisted = super::codex_enrollment::persist(
                    &mut service,
                    &actor,
                    &context,
                    owner,
                    CredentialUpsert {
                        id,
                        kind: "oauth_json".to_owned(),
                        plaintext_secret: material.as_slice(),
                        status,
                    },
                    previous,
                );
                Ok(Ok(Outcome::Saved(persisted)))
            }
        }
    })
    .await;
    match result {
        Ok(Ok(Outcome::State(value))) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .insert_header((header::ETAG, format!("\"{}\"", revision.as_token())))
            .json(value),
        Ok(Ok(Outcome::Saved(Ok((credential, created))))) => revisioned_json(
            if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            credential,
            |credential| {
                serde_json::json!({
                    "state": "completed",
                    "credential_id": credential.id.as_str(),
                })
            },
        ),
        Ok(Ok(Outcome::Saved(Err(error)))) => management_error(error),
        Ok(Err(WorkflowError::Conflict)) => conflict(),
        Err(_) => unavailable(),
    }
}

enum WorkflowError {
    Conflict,
}

enum Outcome {
    State(serde_json::Value),
    Saved(Result<(Revisioned<CredentialView>, bool), ManagementResourceError>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_url_must_stay_on_kimi_https() {
        let valid = serde_json::json!({"verification_uri": "https://auth.kimi.com/device"});
        assert!(verification_uri(&valid).is_ok());
        let invalid = serde_json::json!({"verification_uri": "https://auth.openai.com/device"});
        assert!(verification_uri(&invalid).is_err());
    }

    #[test]
    fn form_encodes_device_codes_without_leaking_to_callers() {
        let form = form(&[
            ("device_code", "code with spaces"),
            ("grant_type", "device"),
        ]);
        assert_eq!(
            form.as_str(),
            "device_code=code+with+spaces&grant_type=device"
        );
    }
}
