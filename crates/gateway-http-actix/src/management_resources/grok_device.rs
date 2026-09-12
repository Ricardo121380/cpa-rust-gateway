//! Bounded device enrollment; tokens never leave this server-side workflow.
use super::native_accounts::NativeAccountManagement;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use provider_grok::{
    GrokAccountAuthStatus, GrokAccountCredential, GrokAccountImport, GrokAccountProvider,
    GrokBuildDevicePollOutcome, GrokBuildDevicePoller, GrokBuildOAuthFlow,
    GrokBuildOAuthHttpResponse, GrokBuildOAuthRequest, GrokBuildOAuthTransport,
    GrokBuildOAuthTransportError,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Read,
    sync::{Arc, Mutex},
};

pub(super) struct DeviceSessions {
    sessions: Mutex<BTreeMap<String, Session>>,
    transport: Transport,
}
struct Session {
    name: String,
    target: Option<Target>,
    poller: Option<GrokBuildDevicePoller>,
    view: View,
}
#[derive(Clone, Serialize)]
pub(super) struct View {
    session_id: String,
    state: String,
    user_code: String,
    verification_uri: String,
    expires_at_ms: i64,
    retry_at_ms: i64,
    identity: Option<gateway_store::account_identity::AccountIdentity>,
    identity_state: String,
    runtime_applied: Option<bool>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Target {
    account_id: String,
    revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Start {
    #[serde(default)]
    name: String,
    target: Option<Target>,
}
#[derive(Clone)]
struct Transport(Arc<dyn GrokBuildOAuthTransport + Send + Sync>);
impl GrokBuildOAuthTransport for Transport {
    fn send(
        &self,
        request: GrokBuildOAuthRequest,
    ) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthTransportError> {
        self.0.send(request)
    }
    fn user_info(
        &self,
        access_token: &str,
    ) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthTransportError> {
        self.0.user_info(access_token)
    }
}
struct HttpTransport;
impl GrokBuildOAuthTransport for HttpTransport {
    fn user_info(
        &self,
        access_token: &str,
    ) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthTransportError> {
        let run = || -> Result<GrokBuildOAuthHttpResponse, Box<dyn std::error::Error>> {
            let response = reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()?
                .get(provider_grok::GROK_BUILD_USERINFO_URL)
                .bearer_auth(access_token)
                .send()?;
            let status = response.status().as_u16();
            let mut bytes = Vec::new();
            response.take(65_537).read_to_end(&mut bytes)?;
            Ok(GrokBuildOAuthHttpResponse::try_new(status, bytes)?)
        };
        run().map_err(|_| GrokBuildOAuthTransportError::Unavailable)
    }

    fn send(
        &self,
        request: GrokBuildOAuthRequest,
    ) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthTransportError> {
        let run = || -> Result<GrokBuildOAuthHttpResponse, Box<dyn std::error::Error>> {
            let endpoint = request.endpoint().url();
            let form = request.into_form_body();
            let client = reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()?;
            let response = client
                .post(endpoint)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(form.to_vec())
                .send()?;
            let status = response.status().as_u16();
            let mut bytes = Vec::new();
            response.take(65537).read_to_end(&mut bytes)?;
            if bytes.len() > 65536 {
                return Err("oversized OAuth response".into());
            }
            Ok(GrokBuildOAuthHttpResponse::try_new(status, bytes)?)
        };
        run().map_err(|_| GrokBuildOAuthTransportError::Unavailable)
    }
}
impl DeviceSessions {
    pub(super) fn new() -> Self {
        Self::with_transport(Arc::new(HttpTransport))
    }
    pub(super) fn with_transport(
        transport: Arc<dyn GrokBuildOAuthTransport + Send + Sync>,
    ) -> Self {
        Self {
            sessions: Mutex::new(BTreeMap::new()),
            transport: Transport(transport),
        }
    }
    pub(super) fn acquire_identity(&self, credential: &mut provider_grok::GrokBuildCredential) {
        // Enrollment remains valid if the issuer temporarily omits profile information.
        let _ = self.read_identity(credential);
    }
    pub(super) fn read_identity(
        &self,
        credential: &mut provider_grok::GrokBuildCredential,
    ) -> Result<(), provider_grok::GrokBuildOAuthError> {
        credential.acquire_identity(&self.transport)
    }
    fn start(&self, input: Start, now: i64) -> Result<View, ()> {
        if input.name.len() > 128
            || input
                .target
                .as_ref()
                .is_some_and(|v| v.account_id.is_empty() || v.account_id.len() > 128)
        {
            return Err(());
        }
        let mut sessions = self.sessions.lock().map_err(|_| ())?;
        sessions.retain(|_, entry| entry.view.expires_at_ms > now);
        if sessions.len() >= 32 {
            return Err(());
        }
        let auth = GrokBuildOAuthFlow::default()
            .start_device_authorization(&self.transport, now)
            .map_err(|_| ())?;
        let mut id = [0; 32];
        getrandom::fill(&mut id).map_err(|_| ())?;
        let id = URL_SAFE_NO_PAD.encode(id);
        let name = if input.name.trim().is_empty() {
            format!("grok-authorization-{id}")
        } else {
            input.name
        };
        let view = View {
            runtime_applied: None,
            identity: None,
            identity_state: "pending".to_owned(),
            session_id: id.clone(),
            state: "pending".into(),
            user_code: auth.user_code().to_owned(),
            verification_uri: auth.verification_uri().to_string(),
            expires_at_ms: auth.expires_at_ms().min(now.saturating_add(900_000)),
            retry_at_ms: now.saturating_add(
                i64::try_from(auth.interval_seconds())
                    .map_err(|_| ())?
                    .saturating_mul(1000),
            ),
        };
        sessions.insert(
            id,
            Session {
                name,
                target: input.target,
                poller: Some(GrokBuildDevicePoller::new(auth)),
                view: view.clone(),
            },
        );
        Ok(view)
    }
    fn poll(&self, id: &str, native: &NativeAccountManagement, now: i64) -> Result<View, ()> {
        let mut sessions = self.sessions.lock().map_err(|_| ())?;
        let entry = sessions.get_mut(id).ok_or(())?;
        if entry.view.state != "pending" {
            return Ok(entry.view.clone());
        }
        if now >= entry.view.expires_at_ms {
            entry.view.state = "expired".into();
            entry.poller = None;
            return Ok(entry.view.clone());
        }
        if now < entry.poller.as_ref().ok_or(())?.next_poll_at_ms() {
            return Ok(entry.view.clone());
        }
        match entry.poller.as_mut().ok_or(())?.poll(&self.transport, now) {
            Ok(
                GrokBuildDevicePollOutcome::Pending { retry_at_ms }
                | GrokBuildDevicePollOutcome::SlowDown { retry_at_ms },
            ) => entry.view.retry_at_ms = retry_at_ms,
            Ok(GrokBuildDevicePollOutcome::Denied) => entry.view.state = "denied".into(),
            Ok(GrokBuildDevicePollOutcome::Expired) => entry.view.state = "expired".into(),
            Ok(GrokBuildDevicePollOutcome::Granted(mut credential)) => {
                let acquired = credential.acquire_identity(&self.transport);
                if !credential.identity().is_empty() {
                    "observed"
                } else if acquired.is_ok() {
                    "not_provided"
                } else {
                    "unavailable"
                }
                .clone_into(&mut entry.view.identity_state);
                entry.view.identity =
                    (!credential.identity().is_empty()).then(|| credential.identity().clone());
                let result = if let Some(target) = &entry.target {
                    native.store.replace_build_authorization(
                        &target.account_id,
                        target.revision,
                        &credential,
                        now,
                    )
                } else {
                    let material = GrokAccountCredential::try_from_build_credential(&credential)
                        .map_err(|_| ())?;
                    let account = GrokAccountImport {
                        provider: GrokAccountProvider::Build,
                        identity: material
                            .enrollment_identity(GrokAccountProvider::Build, now, None)
                            .map_err(|_| ())?,
                        credential: material,
                        auth_status: GrokAccountAuthStatus::Active,
                        enabled: true,
                        priority: 0,
                        weight: 1,
                        max_concurrency: 1,
                        refresh_due_at_ms: Some(
                            credential.expires_at_ms().saturating_sub(60000).max(now),
                        ),
                        quota_sync_due_at_ms: None,
                        cooldown_until_ms: None,
                    };
                    native
                        .store
                        .import_managed_account(&entry.name, &account, now)
                        .map(|_| ())
                };
                if result.is_ok()
                    && let Some(target) = &entry.target
                    && let Ok(stored) = native.store.open_credential(&target.account_id)
                    && let Ok(retained) =
                        provider_grok::GrokBuildCredential::import_refreshable_runtime(
                            stored.as_bytes(),
                            now,
                        )
                    && !retained.identity().is_empty()
                {
                    entry.view.identity = Some(retained.identity().clone());
                    "observed".clone_into(&mut entry.view.identity_state);
                }
                entry.view.state = if result.is_ok() {
                    entry.view.runtime_applied = Some(
                        native.apply_runtime(
                            entry
                                .target
                                .as_ref()
                                .map(|target| target.account_id.as_str()),
                        ),
                    );
                    "complete"
                } else {
                    "persistence_conflict"
                }
                .into();
            }
            Err(_) => entry.view.state = "failed".into(),
        }
        if entry.view.state != "pending" {
            entry.poller = None;
        }
        Ok(entry.view.clone())
    }
    fn cancel(&self, id: &str) -> Result<View, ()> {
        let mut sessions = self.sessions.lock().map_err(|_| ())?;
        let entry = sessions.get_mut(id).ok_or(())?;
        if entry.view.state == "pending" {
            entry.view.state = "cancelled".into();
            entry.poller = None;
        }
        Ok(entry.view.clone())
    }
}

pub(super) async fn start(
    request: actix_web::HttpRequest,
    body: actix_web::web::Bytes,
    state: actix_web::web::Data<super::ManagementResourceHttpState>,
) -> actix_web::HttpResponse {
    if let Err(response) = super::principal(&request) {
        return response;
    }
    let input: Start = match super::parse_json(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    execute(state, move |native, now| native.device.start(input, now)).await
}
pub(super) async fn poll(
    request: actix_web::HttpRequest,
    path: actix_web::web::Path<String>,
    state: actix_web::web::Data<super::ManagementResourceHttpState>,
) -> actix_web::HttpResponse {
    if let Err(response) = super::principal(&request) {
        return response;
    }
    execute(state, move |native, now| {
        native.device.poll(&path, native, now)
    })
    .await
}
pub(super) async fn cancel(
    request: actix_web::HttpRequest,
    path: actix_web::web::Path<String>,
    state: actix_web::web::Data<super::ManagementResourceHttpState>,
) -> actix_web::HttpResponse {
    if let Err(response) = super::principal(&request) {
        return response;
    }
    execute(state, move |native, _| native.device.cancel(&path)).await
}
async fn execute<F>(
    state: actix_web::web::Data<super::ManagementResourceHttpState>,
    run: F,
) -> actix_web::HttpResponse
where
    F: FnOnce(&NativeAccountManagement, i64) -> Result<View, ()> + Send + 'static,
{
    let Some(native) = state.native_accounts.clone() else {
        return actix_web::HttpResponse::ServiceUnavailable().finish();
    };
    let value = super::read_operations(&state, move || {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|v| i64::try_from(v.as_millis()).ok())
            .ok_or(super::ManagementOperationsError::SourceUnavailable)?;
        Ok(run(&native, now))
    })
    .await;
    match value {
        Ok(Ok(view)) => actix_web::HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(view),
        _ => actix_web::HttpResponse::ServiceUnavailable().insert_header(("Cache-Control", "no-store"))
            .json(serde_json::json!({"error":{"code":"management_native_oauth_unavailable","message":"授权会话暂不可用，请重新发起授权。"}})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
    use provider_grok::{GrokAccountPoolStore, GrokBuildOAuthRequestKind};
    use std::{collections::VecDeque, error::Error, path::PathBuf};
    struct Scripted {
        subjects: Mutex<VecDeque<&'static str>>,
    }
    impl GrokBuildOAuthTransport for Scripted {
        fn send(
            &self,
            request: GrokBuildOAuthRequest,
        ) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthTransportError> {
            let value = match request.kind() {
                GrokBuildOAuthRequestKind::DeviceAuthorization => {
                    serde_json::json!({"device_code":"synthetic-private-device","user_code":"LOCAL-TEST","verification_uri":"https://auth.example.test/verify","expires_in":60,"interval":1})
                }
                GrokBuildOAuthRequestKind::DevicePoll => {
                    let subject = self
                        .subjects
                        .lock()
                        .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?
                        .pop_front()
                        .ok_or(GrokBuildOAuthTransportError::Unavailable)?;
                    let claims = URL_SAFE_NO_PAD.encode(
                        serde_json::to_vec(&serde_json::json!({"sub":subject}))
                            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?,
                    );
                    let identity = URL_SAFE_NO_PAD.encode(
                        serde_json::json!({"sub":subject,"email":"authorized.member@example.test"})
                            .to_string(),
                    );
                    serde_json::json!({"access_token":format!("header.{claims}.synthetic-signature"),"refresh_token":"synthetic-private-refresh","expires_in":3600,"token_type":"Bearer","id_token":format!("header.{identity}.signature")})
                }
                GrokBuildOAuthRequestKind::Refresh => {
                    return Err(GrokBuildOAuthTransportError::Unavailable);
                }
            };
            GrokBuildOAuthHttpResponse::try_new(200, value.to_string().into_bytes())
                .map_err(|_| GrokBuildOAuthTransportError::Unavailable)
        }
    }
    struct File(PathBuf);
    impl Drop for File {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            for suffix in ["-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{suffix}", self.0.display()));
            }
        }
    }
    fn fixture() -> Result<(File, NativeAccountManagement), Box<dyn Error>> {
        let key = KeyVersion::try_new(1)?;
        let secrets = SecretStore::new(MasterKeyRing::try_new(
            key,
            [(key, MasterKey::try_from_bytes([0x69; 32])?)],
        )?);
        let mut id = [0; 16];
        getrandom::fill(&mut id).map_err(|_| "randomness")?;
        let file = File(std::env::temp_dir().join(format!(
            "native-device-{}.sqlite3",
            URL_SAFE_NO_PAD.encode(id)
        )));
        let store = Arc::new(GrokAccountPoolStore::try_open(&file.0, secrets)?);
        let native =
            NativeAccountManagement::new(store)?.with_device_oauth_transport(Arc::new(Scripted {
                subjects: Mutex::new(VecDeque::from([
                    "same-account",
                    "same-account",
                    "another-account",
                    "same-account",
                ])),
            }));
        Ok((file, native))
    }
    fn assert_stored_identity(
        native: &NativeAccountManagement,
        id: &str,
    ) -> Result<(), Box<dyn Error>> {
        let stored = native.store.open_credential(id)?;
        let restored = provider_grok::GrokBuildCredential::import_refreshable_runtime(
            stored.as_bytes(),
            2000,
        )?;
        assert_eq!(
            restored.identity().email.as_deref(),
            Some("authorized.member@example.test")
        );
        Ok(())
    }
    #[test]
    fn first_device_grant_and_reauthorization_preserve_identity_and_reject_wrong_or_stale_grants()
    -> Result<(), Box<dyn Error>> {
        let (_file, native) = fixture()?;
        let first = native
            .device
            .start(
                Start {
                    name: String::new(),
                    target: None,
                },
                1000,
            )
            .map_err(|()| "start")?;
        assert_eq!(
            native
                .device
                .poll(&first.session_id, &native, 1500)
                .map_err(|()| "poll")?
                .state,
            "pending"
        );
        assert_eq!(
            native
                .device
                .poll(&first.session_id, &native, 2000)
                .map_err(|()| "grant")?
                .state,
            "complete"
        );
        let before = native.store.managed_account_page(100, "", "", None)?;
        assert_eq!(before.items.len(), 1);
        let id = before.items[0].id.clone();
        assert_stored_identity(&native, &id)?;
        let terminal = native
            .device
            .poll(&first.session_id, &native, 2500)
            .map_err(|()| "terminal")?;
        assert_eq!(terminal.identity_state, "observed");
        assert_eq!(
            terminal.identity.as_ref().and_then(|i| i.email.as_deref()),
            Some("authorized.member@example.test")
        );
        let second = native
            .device
            .start(
                Start {
                    name: "local-grok".into(),
                    target: Some(Target {
                        account_id: id.clone(),
                        revision: 0,
                    }),
                },
                3000,
            )
            .map_err(|()| "start")?;
        assert_eq!(
            native
                .device
                .poll(&second.session_id, &native, 4000)
                .map_err(|()| "poll")?
                .state,
            "complete"
        );
        let after = native.store.managed_account_page(100, "", "", None)?;
        assert_eq!(after.items.len(), 1);
        assert_eq!(after.items[0].id, id);
        assert_eq!(after.items[0].revision, 1);
        for (revision, now) in [(1, 5000), (0, 7000)] {
            let session = native
                .device
                .start(
                    Start {
                        name: "local-grok".into(),
                        target: Some(Target {
                            account_id: id.clone(),
                            revision,
                        }),
                    },
                    now,
                )
                .map_err(|()| "start")?;
            assert_eq!(
                native
                    .device
                    .poll(&session.session_id, &native, now + 1000)
                    .map_err(|()| "poll")?
                    .state,
                "persistence_conflict"
            );
            assert_eq!(
                native.store.managed_account_page(100, "", "", None)?.items[0].revision,
                1
            );
        }
        Ok(())
    }
    #[test]
    fn cancellation_and_expiry_never_create_accounts() -> Result<(), Box<dyn Error>> {
        let (_file, native) = fixture()?;
        let view = native
            .device
            .start(
                Start {
                    name: "cancelled".into(),
                    target: None,
                },
                1000,
            )
            .map_err(|()| "start")?;
        assert_eq!(
            native
                .device
                .cancel(&view.session_id)
                .map_err(|()| "cancel")?
                .state,
            "cancelled"
        );
        assert_eq!(
            native
                .device
                .poll(&view.session_id, &native, 2000)
                .map_err(|()| "poll")?
                .state,
            "cancelled"
        );
        let view = native
            .device
            .start(
                Start {
                    name: "expired".into(),
                    target: None,
                },
                3000,
            )
            .map_err(|()| "start")?;
        assert_eq!(
            native
                .device
                .poll(&view.session_id, &native, 63000)
                .map_err(|()| "poll")?
                .state,
            "expired"
        );
        assert!(
            native
                .store
                .managed_account_page(100, "", "", None)?
                .items
                .is_empty()
        );
        Ok(())
    }
}
