//! Production Build refresh exchange through a controlled loopback HTTP peer.
use super::*;
use gateway_control::management_mutation_service::ManagementMutationService;
use gateway_http_actix::{
    management_resources::{
        ManagementResourceHttpState, configure_management_resources,
        native_accounts::NativeAccountManagement,
    },
    management_security::{
        ManagementBrowserPolicy, ManagementHttpState, ManagementKey, ManagementNetworkPolicy,
    },
};
use gateway_store::{
    control_plane::{
        ConfigVersion, ConfigVersionStatus, EndpointConfiguration, EndpointTransport,
        UpstreamConfiguration,
    },
    secret_store::{KeyVersion, MasterKey, MasterKeyRing},
};
use provider_grok::{GrokAccountEndpointBinding, GrokAccountIdentity, GrokAccountImport};
use std::{
    error::Error,
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
};

const MANAGEMENT_KEY: &str = "mgmt_batch_a_build_synthetic_management_0123456789";

#[test]
fn build_loopback_refresh_reaches_durable_runtime_and_management_http() -> Result<(), Box<dyn Error>>
{
    for status in [200, 400, 403, 429, 503] {
        run_status(status)?;
    }
    Ok(())
}

fn run_status(status: u16) -> Result<(), Box<dyn Error>> {
    let directory = std::env::temp_dir().join(format!(
        "cpar-a-build-refresh-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let result = run_in_directory(&directory, status);
    let _ = std::fs::remove_dir_all(&directory);
    result
}

#[allow(
    clippy::too_many_lines,
    reason = "one native refresh scenario verifies HTTP, durable state, runtime and management readback"
)]
fn run_in_directory(directory: &Path, status: u16) -> Result<(), Box<dyn Error>> {
    let database = directory.join("native.sqlite3");
    let key = KeyVersion::try_new(1)?;
    let secrets = SecretStore::new(MasterKeyRing::try_new(
        key,
        [(key, MasterKey::try_from_bytes([0x6a; 32])?)],
    )?);
    let version = ConfigVersionId::try_new("batch-a-build-refresh")?;
    let owner = gateway_core::UpstreamId::try_new("batch-a-build")?;
    let endpoint = EndpointId::try_new("batch-a-build-endpoint")?;
    let mut config = ControlPlaneConfiguration::new(ConfigVersion {
        id: version.clone(),
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision: 0,
        created_at_ms: 0,
        description: "synthetic Build refresh".into(),
    });
    config.upstreams.push(UpstreamConfiguration {
        id: owner.clone(),
        name: "Build".into(),
        kind: "grok-build-native".into(),
        enabled: true,
        tags_json: "[]".into(),
        egress_policy_id: None,
    });
    config.endpoints.push(EndpointConfiguration {
        id: endpoint.clone(),
        upstream_id: owner,
        adapter_id: "grok.build.responses".into(),
        api_format: "openai/responses".into(),
        base_url: provider_grok::GROK_BUILD_RESPONSES_BASE_URL.into(),
        inference_path: provider_grok::GROK_BUILD_RESPONSES_PATH.into(),
        models_path: None,
        transport: EndpointTransport::Http,
        enabled: true,
    });
    let mut repository = SqliteControlPlaneRepository::open(&database)?;
    repository.write_configuration(&config)?;
    repository.activate_version(&version)?;
    let now = now_ms()?;
    let store = Arc::new(GrokAccountPoolStore::try_open(&database, secrets.clone())?);
    for (id, enabled) in [("active", true), ("disabled", false)] {
        let material = GrokBuildCredential::import_json(br#"{"access_token":"old-access","refresh_token":"old-refresh","expires_in":300,"token_type":"Bearer"}"#, now)?;
        store.import_batch(
            id,
            &[GrokAccountImport {
                provider: GrokAccountProvider::Build,
                identity: GrokAccountIdentity::try_from_bytes(id)?,
                credential: GrokAccountCredential::try_from_build_credential(&material)?,
                auth_status: GrokAccountAuthStatus::Active,
                enabled,
                priority: 0,
                weight: 1,
                max_concurrency: 1,
                refresh_due_at_ms: Some(now),
                quota_sync_due_at_ms: None,
                cooldown_until_ms: None,
            }],
            now,
        )?;
    }
    let id = store.single_import_account("active")?;
    let disabled = store.single_import_account("disabled")?;
    let binding = GrokAccountEndpointBinding::new(GrokAccountProvider::Build, endpoint.clone());
    let pools = store
        .compile_native_runtime(&[binding], now)?
        .credential_pools();
    let pool = pools.pool(&endpoint).ok_or("native credential pool")?;
    let prior = pool
        .diagnostic_entries()
        .into_iter()
        .find(|row| row.credential_id().as_str() == id)
        .ok_or("prior runtime material")?;
    assert_eq!(prior.credential_revision(), 1);

    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let stopped = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let peer_stopped = stopped.clone();
    let peer_calls = calls.clone();
    let peer = thread::spawn(move || -> std::io::Result<()> {
        while !peer_stopped.load(Ordering::Acquire) {
            let (mut socket, _) = match listener.accept() {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(error) => return Err(error),
            };
            // Accepted sockets may inherit the nonblocking listener mode on Darwin.
            // The synthetic HTTP peer must wait for a complete bounded request.
            socket.set_nonblocking(false)?;
            socket.set_read_timeout(Some(Duration::from_secs(3)))?;
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 2048];
            let split = loop {
                let count = socket.read(&mut buffer)?;
                if count == 0 {
                    return Err(std::io::Error::other("truncated request"));
                }
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(index) = bytes.windows(4).position(|row| row == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let head = String::from_utf8_lossy(&bytes[..split]);
            let is_post = head.starts_with("POST ");
            if is_post {
                let length = head
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while bytes.len() < split + length {
                    let count = socket.read(&mut buffer)?;
                    if count == 0 {
                        return Err(std::io::Error::other("truncated form"));
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                }
                assert!(String::from_utf8_lossy(&bytes[split..]).contains("old-refresh"));
                peer_calls.fetch_add(1, Ordering::SeqCst);
            }
            let body: &[u8] = if is_post {
                match status {
                    200 => br#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":3600,"token_type":"Bearer"}"#,
                    400 => br#"{"error":"invalid_grant"}"#,
                    _ => b"<html>temporary denial</html>",
                }
            } else {
                br#"{"email":"synthetic@example.test"}"#
            };
            let reply_status = if is_post { status } else { 200 };
            write!(
                socket,
                "HTTP/1.1 {reply_status} Local\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )?;
            socket.write_all(body)?;
        }
        Ok(())
    });
    let health = Arc::new(RuntimeHealthRegistry::new());
    let mut worker = RuntimeCredentialRefreshWorker::try_new(
        &database,
        secrets.clone(),
        Arc::clone(&pools),
        health,
        vec![endpoint.clone()],
        UpstreamProxy::Direct,
        version,
    )?
    .ok_or("native refresh worker")?;
    worker.executor.test_endpoints = Some((
        format!("http://{address}/token"),
        format!("http://{address}/userinfo"),
    ));
    let summary = worker.run_once();
    stopped.store(true, Ordering::Release);
    peer.join().map_err(|_| "Build HTTP peer panicked")??;
    let summary = summary?;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "disabled Build sibling must not exchange"
    );
    assert_eq!(summary.claimed, 1);
    assert_eq!(summary.succeeded, usize::from(status == 200));
    assert_eq!(summary.reauth_required, usize::from(status == 400));
    assert_eq!(
        summary.backed_off,
        usize::from(status == 403 || status == 429 || status == 503)
    );
    assert_eq!(summary.runtime_replaced, usize::from(status == 200));
    let metadata = store.list_accounts()?;
    let active = metadata
        .iter()
        .find(|row| row.id == id)
        .ok_or("active account")?;
    let sibling = metadata
        .iter()
        .find(|row| row.id == disabled)
        .ok_or("disabled account")?;
    assert!(!sibling.enabled);
    assert_eq!(sibling.revision, 0);
    assert_eq!(
        active.auth_status,
        if status == 400 {
            GrokAccountAuthStatus::ReauthRequired
        } else {
            GrokAccountAuthStatus::Active
        }
    );
    if status == 200 {
        assert_eq!(active.revision, 1);
        assert_eq!(
            pool.diagnostic_entries()
                .into_iter()
                .find(|row| row.credential_id().as_str() == id)
                .ok_or("rotated runtime")?
                .credential_revision(),
            2
        );
    } else if status == 403 || status == 429 || status == 503 {
        assert!(active.refresh_due_at_ms.ok_or("backoff")? > now);
    }
    let compiled = store.compile_native_runtime(
        &[GrokAccountEndpointBinding::new(
            GrokAccountProvider::Build,
            endpoint.clone(),
        )],
        now + 1,
    )?;
    let candidates = compiled
        .credential_pools()
        .pool(&EndpointId::try_new("batch-a-build-endpoint")?)
        .ok_or("compiled pool")?
        .diagnostic_entries();
    assert!(
        candidates
            .iter()
            .any(|row| row.credential_id().as_str() == id)
    );
    assert!(
        !candidates
            .iter()
            .any(|row| row.credential_id().as_str() == disabled)
    );
    let restarted_health = RuntimeHealthRegistry::new();
    compiled.seed_runtime_health(&restarted_health)?;
    assert_eq!(
        restarted_health
            .endpoint_credential_is_available(&endpoint, &CredentialId::try_new(id.clone())?),
        status != 400
    );
    actix_web::rt::System::new().block_on(async {
        use actix_web::{App, http::StatusCode, test, web};
        let security = ManagementHttpState::new(
            ManagementKey::try_new(MANAGEMENT_KEY)?,
            ManagementNetworkPolicy::LoopbackOnly,
            ManagementBrowserPolicy::DenyBrowserOrigins,
        )?;
        let resources = ManagementResourceHttpState::new(ManagementMutationService::new(
            SqliteControlPlaneRepository::open(&database)?,
            secrets,
        ))
        .with_native_accounts(NativeAccountManagement::new(store)?);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(security))
                .app_data(web::Data::new(resources))
                .configure(configure_management_resources),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/admin/native-accounts?limit=100")
                .peer_addr("127.0.0.1:1234".parse()?)
                .insert_header(("X-Management-Key", MANAGEMENT_KEY))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(response).await;
        let rows = body["items"].as_array().ok_or("native management rows")?;
        let row = rows
            .iter()
            .find(|row| row["id"] == id)
            .ok_or("native management account")?;
        assert_eq!(
            row["auth_status"],
            if status == 400 {
                "reauth_required"
            } else {
                "active"
            }
        );
        assert_eq!(row["revision"], i32::from(status == 200));
        assert_eq!(
            rows.iter()
                .find(|row| row["id"] == disabled)
                .ok_or("disabled row")?["enabled"],
            false
        );
        assert!(!body.to_string().contains("new-refresh"));
        Ok::<(), Box<dyn Error>>(())
    })?;
    Ok(())
}
