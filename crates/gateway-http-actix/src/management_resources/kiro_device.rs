//! AWS Builder ID / IAM Identity Center device authorization for existing Kiro upstreams.
//! OIDC registration, device and token shapes follow the AWS OIDC API and official Q CLI.
use super::*;
use gateway_store::control_plane::ConfigVersionStatus;
use std::io::Read;
use zeroize::Zeroize;

type Transport = dyn Fn(&str, &serde_json::Value) -> Result<serde_json::Value, ()> + Send + Sync;
/// Bounded ephemeral device sessions. Secrets are never returned to the browser.
pub struct KiroDeviceWorkflow {
    sessions: BTreeMap<String, Session>,
    transport: Box<Transport>,
}
struct Session {
    scope: String,
    client_id: Zeroizing<String>,
    client_secret: Zeroizing<String>,
    device_code: Zeroizing<String>,
    region: String,
    expires: i64,
    next_poll: i64,
    interval: i64,
}
impl KiroDeviceWorkflow {
    /// Uses an explicitly injected bounded OIDC transport, for isolated acceptance.
    #[must_use]
    pub fn with_transport(transport: Box<Transport>) -> Self {
        Self {
            sessions: BTreeMap::new(),
            transport,
        }
    }

    /// Fixed AWS OIDC endpoints, no ambient proxies or arbitrary URL fetches.
    #[must_use]
    pub fn production() -> Self {
        Self {
            sessions: BTreeMap::new(),
            transport: Box::new(aws),
        }
    }
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|v| i64::try_from(v.as_millis()).ok())
        .unwrap_or(0)
}
fn aws(url: &str, body: &serde_json::Value) -> Result<serde_json::Value, ()> {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| ())?;
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(body).map_err(|_| ())?)
        .send()
        .map_err(|_| ())?;
    if response.content_length().is_some_and(|n| n > 65536) {
        return Err(());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    response
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.len() > 65536 {
        return Err(());
    }
    serde_json::from_slice(&bytes).map_err(|_| ())
}
fn wipe(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => s.zeroize(),
        serde_json::Value::Array(a) => a.iter_mut().for_each(wipe),
        serde_json::Value::Object(o) => o.values_mut().for_each(wipe),
        _ => {}
    }
}
fn token(value: &serde_json::Value, key: &str) -> Result<Zeroizing<String>, ()> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 16384)
        .map(|s| Zeroizing::new(s.to_owned()))
        .ok_or(())
}
fn region_valid(region: &str) -> bool {
    matches!(
        region,
        "us-east-1"
            | "us-east-2"
            | "us-west-1"
            | "us-west-2"
            | "ap-southeast-1"
            | "ap-southeast-2"
            | "ap-northeast-1"
            | "ap-northeast-2"
            | "ap-south-1"
            | "eu-west-1"
            | "eu-west-2"
            | "eu-west-3"
            | "eu-central-1"
            | "eu-north-1"
            | "ca-central-1"
    )
}
fn start_url_valid(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
            && u.host_str().is_some_and(|h| h.ends_with(".awsapps.com"))
            && u.path().trim_end_matches('/') == "/start"
            && u.query().is_none()
            && u.fragment().is_none()
    })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    id: String,
    #[serde(default)]
    replace_existing: bool,
    session_id: Option<String>,
    region: Option<String>,
    start_url: Option<String>,
}
fn conflict() -> HttpResponse {
    error_response(
        StatusCode::CONFLICT,
        "management_kiro_authorization_conflict",
        "授权已结束或配置已改变，请重新开始",
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
        .map_err(|e| management_error(e.into()))?
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
        != "kiro"
    {
        return Err(invalid_input());
    }
    match service.get_credential(&context.version, id) {
        Err(ManagementResourceError::ResourceNotFound) if !replace_existing => Ok(None),
        Ok(value)
            if replace_existing
                && value.value().upstream_id == *owner
                && value.value().kind == "bearer" =>
        {
            Ok(Some(value.value().clone()))
        }
        Ok(_) | Err(ManagementResourceError::ResourceNotFound) => Err(conflict()),
        Err(e) => Err(management_error(e)),
    }
}
#[derive(Clone, Copy)]
enum Action {
    Start,
    Poll,
    Cancel,
}
pub(super) async fn start(
    r: HttpRequest,
    p: web::Path<String>,
    b: web::Bytes,
    s: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    handle(Action::Start, r, p, b, s).await
}
pub(super) async fn poll(
    r: HttpRequest,
    p: web::Path<String>,
    b: web::Bytes,
    s: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    handle(Action::Poll, r, p, b, s).await
}
pub(super) async fn cancel(
    r: HttpRequest,
    p: web::Path<String>,
    b: web::Bytes,
    s: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    handle(Action::Cancel, r, p, b, s).await
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
        Ok(v) => v,
        Err(r) => return r,
    };
    let actor = match principal(&request) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input = match parse_json::<Input>(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Ok(owner), Ok(id)) = (
        UpstreamId::try_new(path.into_inner()),
        CredentialId::try_new(input.id),
    ) else {
        return invalid_input();
    };
    let previous = match admit(&state, &context, &owner, &id, input.replace_existing) {
        Ok(value) => value,
        Err(r) => return r,
    };
    let region = input.region.as_deref().unwrap_or("us-east-1");
    let start_url = input
        .start_url
        .as_deref()
        .unwrap_or("https://view.awsapps.com/start");
    if !region_valid(region)
        || !start_url_valid(start_url)
        || input.session_id.as_ref().is_some_and(|s| s.len() > 128)
    {
        return invalid_input();
    }
    let scope = serde_json::json!([
        context.version.as_str(),
        context.revision.as_i64(),
        owner.as_str(),
        id.as_str(),
        actor.as_str(),
        previous.as_ref().map(|value| value.revision)
    ])
    .to_string();
    let region = region.to_owned();
    let start_url = start_url.to_owned();
    let worker = state.clone();
    let revision = context.revision;
    let result=read_operations(&state,move||{
        let Some(shared)=&worker.kiro_workflow else{return Ok(Err(()));};
        let mut guard=shared.lock().map_err(|_|ManagementOperationsError::SourceUnavailable)?;
        let workflow=&mut *guard;
        let clock=now();workflow.sessions.retain(|_,s|s.expires>clock);
        match action {
            Action::Start=>{
                if workflow.sessions.len()>=128||workflow.sessions.values().any(|s|s.scope==scope){return Ok(Err(()));}
                let mut registration=(workflow.transport)(&format!("https://oidc.{region}.amazonaws.com/client/register"),&serde_json::json!({"clientName":"CPAR Kiro","clientType":"public","scopes":["codewhisperer:completions","codewhisperer:analysis","codewhisperer:conversations"]})).map_err(|()|ManagementOperationsError::SourceUnavailable)?;
                let client_id=token(&registration,"clientId");let client_secret=token(&registration,"clientSecret");wipe(&mut registration);
                let (Ok(client_id),Ok(client_secret))=(client_id,client_secret)else{return Ok(Err(()));};
                let mut payload=serde_json::json!({"clientId":client_id.as_str(),"clientSecret":client_secret.as_str(),"startUrl":start_url});
                let response=(workflow.transport)(&format!("https://oidc.{region}.amazonaws.com/device_authorization"),&payload);wipe(&mut payload);
                let mut response=response.map_err(|()|ManagementOperationsError::SourceUnavailable)?;
                let parsed=(||{
                    let device_code=token(&response,"deviceCode")?;
                    let user_code=token(&response,"userCode")?;
                    let uri=token(&response,"verificationUri")?;
                    if !url::Url::parse(&uri).is_ok_and(|u|u.scheme()=="https"&&u.host_str().is_some_and(|h|h.ends_with(".awsapps.com")||h.ends_with(".amazonaws.com"))&&u.username().is_empty()&&u.password().is_none()){return Err(());}
                    let expires=response["expiresIn"].as_i64().filter(|v|(1..=3600).contains(v)).ok_or(())?;
                    let interval=response["interval"].as_i64().unwrap_or(5).clamp(5,60)*1000;
                    let mut random=[0_u8;32];getrandom::fill(&mut random).map_err(|_|())?;let session_id=URL_SAFE_NO_PAD.encode(random);
                    workflow.sessions.insert(session_id.clone(),Session{scope,client_id,client_secret,device_code,region,expires:clock+expires*1000,next_poll:clock+interval,interval});
                    Ok(serde_json::json!({"state":"pending","session_id":session_id,"user_code":user_code.as_str(),"verification_uri":uri.as_str(),"expires_at_ms":clock+expires*1000,"interval_ms":interval}))
                })();wipe(&mut response);Ok(parsed.map(Outcome::State))
            },
            Action::Cancel=>{
                let Some(session)=input.session_id else{return Ok(Err(()));};
                if workflow.sessions.get(&session).is_none_or(|s|s.scope!=scope){return Ok(Err(()));}
                workflow.sessions.remove(&session);Ok(Ok(Outcome::State(serde_json::json!({"state":"cancelled"}))))
            },
            Action::Poll=>{
                let Some(session_id)=input.session_id else{return Ok(Err(()));};
                if workflow.sessions.get(&session_id).is_none_or(|s|s.scope!=scope){return Ok(Err(()));}
                let Some(session)=workflow.sessions.get_mut(&session_id)else{return Ok(Err(()));};
                if clock<session.next_poll{return Ok(Ok(Outcome::State(serde_json::json!({"state":"pending","interval_ms":session.next_poll-clock}))));}
                session.next_poll=clock+session.interval;
                let mut payload=serde_json::json!({"clientId":session.client_id.as_str(),"clientSecret":session.client_secret.as_str(),"deviceCode":session.device_code.as_str(),"grantType":"urn:ietf:params:oauth:grant-type:device_code"});
                let response=(workflow.transport)(&format!("https://oidc.{}.amazonaws.com/token",session.region),&payload);wipe(&mut payload);
                let Ok(mut response)=response else {workflow.sessions.remove(&session_id);return Err(ManagementOperationsError::SourceUnavailable);};
                let error=response["error"].as_str().or_else(||response["__type"].as_str()).unwrap_or("");
                if matches!(error,"authorization_pending"|"AuthorizationPendingException"|"slow_down"|"SlowDownException"){
                    if matches!(error,"slow_down"|"SlowDownException"){session.interval=(session.interval+5000).min(60000);session.next_poll=clock+session.interval;}
                    let interval=session.interval;wipe(&mut response);return Ok(Ok(Outcome::State(serde_json::json!({"state":"pending","interval_ms":interval}))));
                }
                let material:Result<Zeroizing<Vec<u8>>,()>=(||{
                    let access=token(&response,"accessToken")?;let refresh=token(&response,"refreshToken")?;
                    let expires=response["expiresIn"].as_i64().filter(|v|(1..=86400).contains(v)).ok_or(())?;
                    let mut envelope=serde_json::json!({"kind":"enterprise","access_token":access.as_str(),"refresh_token":refresh.as_str(),"expires_at_ms":clock+expires*1000,"client_id":session.client_id.as_str(),"client_secret":session.client_secret.as_str(),"auth_region":session.region});
                    let bytes=Zeroizing::new(serde_json::to_vec(&envelope).map_err(|_|())?);wipe(&mut envelope);
                    provider_kiro::credential::KiroCredential::import_runtime_secret(&bytes,clock).map_err(|_|())?;Ok(bytes)
                })();wipe(&mut response);workflow.sessions.remove(&session_id);
                let Ok(material)=material else{return Ok(Err(()));};
                let mut service=worker.service.lock().map_err(|_|ManagementOperationsError::SourceUnavailable)?;
                let status=if previous.as_ref().is_some_and(|v|v.status==CredentialStatus::Disabled){CredentialStatus::Disabled}else{CredentialStatus::Active};
                let result=super::codex_enrollment::persist(&mut service,&actor,&context,owner,CredentialUpsert{id,kind:"bearer".into(),plaintext_secret:&material,status},previous);
                Ok(Ok(Outcome::Saved(result)))
            }
        }
    }).await;
    match result {
        Ok(Ok(Outcome::State(value))) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .insert_header((header::ETAG, format!("\"{}\"", revision.as_token())))
            .json(value),
        Ok(Ok(Outcome::Saved(Ok((value, created))))) => revisioned_json(
            if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            value,
            |credential| serde_json::json!({"state":"completed","credential_id":credential.id.as_str()}),
        ),
        Ok(Ok(Outcome::Saved(Err(e)))) => management_error(e),
        Ok(Err(())) => conflict(),
        Err(_) => error_response(
            StatusCode::BAD_GATEWAY,
            "management_kiro_authorization_unavailable",
            "授权服务暂不可用，请重新开始；未自动重试",
        ),
    }
}
enum Outcome {
    State(serde_json::Value),
    Saved(Result<(Revisioned<CredentialView>, bool), ManagementResourceError>),
}
