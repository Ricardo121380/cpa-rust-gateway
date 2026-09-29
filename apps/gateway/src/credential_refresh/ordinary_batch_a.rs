//! Real loopback OAuth HTTP, durable rotation, live eligibility and management projection.
//! Only token destinations are replaced in this test build; wire dispatch/parsing are production.
use super::*;
use crate::provider_account_pool_adapter::{
    ProviderAccountDescriptor, ProviderAccountDescriptorSource, ProviderAccountPoolAdapter,
    ProviderAccountPoolClock, ProviderAccountPoolClockError,
};
use gateway_control::provider_account_pool_service::{
    ProviderAccountAuthStatus, ProviderAccountRuntimeStatus,
};
use gateway_core::{EndpointId, ProviderId, UpstreamId};
use gateway_router::{RuntimeHealthClock, RuntimeHealthClockError, RuntimeQuotaRegistry};
use gateway_store::{
    control_plane::*,
    secret_store::{KeyVersion, MasterKey, MasterKeyRing},
};
use gateway_upstream::{CredentialSecret, EndpointCredentialInput, EndpointCredentialPool};
use std::{
    io::Write,
    net::TcpListener,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const NOW: i64 = 2_000;
const MANAGEMENT_KEY: &str = "mgmt_batch_a_synthetic_management_0123456789";
const CODEX:&[u8]=br#"{"kind":"codex_oauth","access_token":"old","refresh_token":"refresh","expires_at_ms":1000,"account_id":"subject"}"#;
const KIMI:&[u8]=br#"{"kind":"kimi_oauth","access_token":"old","refresh_token":"refresh","expires_at_ms":1000,"device_id":"synthetic-device"}"#;
const CLAUDE:&[u8]=br#"{"kind":"claude_oauth","access_token":"old","refresh_token":"refresh","expires_at_ms":1000,"email":"synthetic@example.test"}"#;
const KIRO:&[u8]=br#"{"kind":"enterprise","access_token":"old","refresh_token":"refresh","expires_at_ms":1000,"client_id":"app","client_secret":"synthetic","auth_region":"us-east-1"}"#;
struct Clock;
impl ProviderAccountPoolClock for Clock {
    fn now_ms(&self) -> Result<i64, ProviderAccountPoolClockError> {
        Ok(NOW)
    }
}
impl RuntimeHealthClock for Clock {
    fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
        Ok(NOW)
    }
}
struct Fixture {
    database: PathBuf,
    secrets: SecretStore,
    version: ConfigVersionId,
    channels: BTreeMap<CredentialId, Channel>,
    pools: Arc<EndpointCredentialPools>,
    health: Arc<RuntimeHealthRegistry>,
    descriptors: Vec<ProviderAccountDescriptor>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.database);
    }
}
impl Fixture {
    #[allow(
        clippy::too_many_lines,
        reason = "one coherent loopback HTTP and durable runtime fixture"
    )]
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let key = KeyVersion::try_new(1)?;
        let secrets = SecretStore::new(MasterKeyRing::try_new(
            key,
            [(key, MasterKey::try_from_bytes([31; 32])?)],
        )?);
        let version = ConfigVersionId::try_new("batch-a-refresh")?;
        let mut config = ControlPlaneConfiguration::new(ConfigVersion {
            id: version.clone(),
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 0,
            description: "synthetic refresh".into(),
        });
        let mut channels = BTreeMap::new();
        let mut pools = Vec::new();
        let mut descriptors = Vec::new();
        for (name, channel, bytes, enabled, expiry) in [
            ("codex", Channel::Codex, CODEX, true, Some(1_000)),
            ("kimi", Channel::Kimi, KIMI, true, Some(1_000)),
            ("claude", Channel::Claude, CLAUDE, true, Some(1_000)),
            ("kiro", Channel::Kiro, KIRO, true, Some(1_000)),
            ("disabled", Channel::Claude, CLAUDE, false, Some(1_000)),
            (
                "api-key",
                Channel::Kiro,
                b"ksk_synthetic_key".as_slice(),
                true,
                None,
            ),
        ] {
            let id = CredentialId::try_new(name)?;
            let owner = UpstreamId::try_new(name)?;
            let endpoint = EndpointId::try_new(format!("endpoint-{name}"))?;
            config.upstreams.push(UpstreamConfiguration {
                id: owner.clone(),
                name: name.into(),
                kind: "test".into(),
                enabled: true,
                tags_json: "[]".into(),
                egress_policy_id: None,
            });
            config.endpoints.push(EndpointConfiguration {
                id: endpoint.clone(),
                upstream_id: owner.clone(),
                adapter_id: "test.responses".into(),
                api_format: "openai/responses".into(),
                base_url: "https://synthetic.invalid".into(),
                inference_path: "/responses".into(),
                models_path: None,
                transport: EndpointTransport::Http,
                enabled: true,
            });
            let aad = credential_associated_data(&version, &id, &owner)?;
            config.credentials.push(CredentialConfiguration {
                id: id.clone(),
                upstream_id: owner.clone(),
                kind: "bearer".into(),
                encrypted_secret: secrets.seal(bytes, &aad)?,
                status: if enabled {
                    CredentialStatus::Active
                } else {
                    CredentialStatus::Disabled
                },
                revision: 1,
            });
            config
                .endpoint_credential_bindings
                .push(EndpointCredentialBindingConfiguration {
                    endpoint_id: endpoint.clone(),
                    credential_id: id.clone(),
                    upstream_id: owner,
                    enabled: true,
                    priority: 0,
                    weight: 1,
                    concurrency: 2,
                });
            channels.insert(id.clone(), channel);
            if enabled {
                pools.push(EndpointCredentialPool::try_new(
                    endpoint.clone(),
                    [EndpointCredentialInput {
                        credential_id: id.clone(),
                        credential_kind: "bearer".into(),
                        credential_revision: 1,
                        priority: 0,
                        weight: 1,
                        concurrency: 2,
                        expires_at_ms: expiry,
                        secret: CredentialSecret::try_new(bytes.to_vec())?,
                    }],
                )?);
            }
            descriptors.push(ProviderAccountDescriptor {
                presentation: None,
                source: ProviderAccountDescriptorSource::Ordinary,
                provider_id: ProviderId::try_new(name)?,
                channel_id: endpoint,
                account_id: id,
                account_kind: "bearer".into(),
                auth_status: if enabled {
                    ProviderAccountAuthStatus::Active
                } else {
                    ProviderAccountAuthStatus::Disabled
                },
                runtime_status_hint: ProviderAccountRuntimeStatus::Available,
                enabled,
                priority: 0,
                weight: 1,
                max_concurrency: 2,
                expires_at_ms: expiry,
                refresh_due_at_ms: None,
                quota_sync_due_at_ms: None,
                entitlement: None,
                upstream_models: Vec::new(),
            });
        }
        let database = std::env::temp_dir().join(format!(
            "cpar-a-refresh-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let mut repository = SqliteControlPlaneRepository::open(&database)?;
        repository.write_configuration(&config)?;
        repository.activate_version(&version)?;
        Ok(Self {
            database,
            secrets,
            version,
            channels,
            pools: Arc::new(EndpointCredentialPools::try_new(pools)?),
            health: Arc::new(RuntimeHealthRegistry::with_clock(Arc::new(Clock))),
            descriptors,
        })
    }
    fn pass(&self) -> Pass<'_> {
        Pass {
            database: &self.database,
            secrets: &self.secrets,
            version: &self.version,
            channels: &self.channels,
            pools: Some(&self.pools),
            health: Some(&self.health),
            guard: None,
            stopped: None,
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "checks each refresh outcome across store, pool, management and restart boundaries"
)]
fn loopback_oauth_rotation_and_failures_reach_live_eligibility_and_management_http() -> TestResult {
    for status in [200, 400, 403, 429, 503] {
        let fixture = Fixture::new()?;
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let peer = std::thread::spawn(move || -> std::io::Result<()> {
            for _ in 0..4 {
                let (mut socket, _) = listener.accept()?;
                socket.set_read_timeout(Some(Duration::from_secs(5)))?;
                let mut bytes = Vec::new();
                let mut buf = [0_u8; 2048];
                let split;
                loop {
                    let count = socket.read(&mut buf)?;
                    if count == 0 {
                        return Err(std::io::Error::other("missing token request"));
                    }
                    bytes.extend_from_slice(&buf[..count]);
                    if let Some(index) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        split = index + 4;
                        break;
                    }
                }
                let head = String::from_utf8_lossy(&bytes[..split]);
                let length = head
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while bytes.len() < split + length {
                    let count = socket.read(&mut buf)?;
                    if count == 0 {
                        return Err(std::io::Error::other("truncated token request"));
                    }
                    bytes.extend_from_slice(&buf[..count]);
                }
                let body = String::from_utf8_lossy(&bytes[split..]);
                assert!(body.contains("refresh_token"));
                let camel = body.contains("grantType");
                let response = match status {
                    200 if camel => {
                        br#"{"accessToken":"new","refreshToken":"rotated","expiresIn":3600}"#
                            .as_slice()
                    }
                    200 => br#"{"access_token":"new","refresh_token":"rotated","expires_in":3600}"#
                        .as_slice(),
                    400 => br#"{"error":"invalid_grant"}"#.as_slice(),
                    _ => b"<html>temporary denial</html>".as_slice(),
                };
                counted.fetch_add(1, Ordering::SeqCst);
                write!(
                    socket,
                    "HTTP/1.1 {status} Local\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.len()
                )?;
                socket.write_all(response)?;
            }
            Ok(())
        });
        let exchange = HttpExchange {
            codex_proxy: UpstreamProxy::Direct,
            token_url_override: Some(format!("http://{address}/token")),
        };
        let summary = fixture.pass().run_with(&exchange, &|| Ok(NOW))?;
        peer.join().map_err(|_| "token peer panicked")??;
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        if status == 200 {
            assert_eq!(summary.succeeded, 4);
            assert_eq!(summary.runtime_replaced, 4);
        } else if status == 400 {
            assert_eq!(summary.reauth_required, 4);
        } else {
            assert_eq!(summary.backed_off, 4);
        }
        assert_eq!(
            fixture
                .pass()
                .run_with(&exchange, &|| Ok(NOW + 1))?
                .succeeded,
            0
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            4,
            "no automatic replay before the durable retry deadline"
        );
        let mut repository = SqliteControlPlaneRepository::open(&fixture.database)?;
        let config = repository
            .load_configuration(&fixture.version)?
            .ok_or("configuration")?;
        for row in &config.credentials {
            if row.id.as_str() == "disabled" {
                assert_eq!(row.status, CredentialStatus::Disabled);
                assert_eq!(row.revision, 1);
                continue;
            }
            if row.id.as_str() == "api-key" {
                assert_eq!(row.revision, 1);
                continue;
            }
            assert_eq!(
                row.status,
                if status == 400 {
                    CredentialStatus::Unauthorized
                } else {
                    CredentialStatus::Active
                }
            );
            assert_eq!(
                row.revision,
                if status == 200 || status == 400 { 2 } else { 1 }
            );
            let endpoint = EndpointId::try_new(format!("endpoint-{}", row.id.as_str()))?;
            let pool = fixture.pools.pool(&endpoint).ok_or("pool")?;
            assert_eq!(
                pool.try_lease_exact_eligible_at(&row.id, NOW, |_| true)
                    .is_some(),
                status == 200
            );
        }
        let adapter = ProviderAccountPoolAdapter::try_new(
            fixture.descriptors.clone(),
            fixture.pools.clone(),
            fixture.health.clone(),
            Arc::new(RuntimeQuotaRegistry::with_clock(Arc::new(Clock))),
            Arc::new(Clock),
            Duration::from_millis(1),
            Duration::from_secs(1),
        )?
        .with_config_version(fixture.version.to_string())?;
        actix_web::rt::System::new().block_on(async {
            use actix_web::{App, http::StatusCode, test, web};
            use gateway_http_actix::{
                management_resources::{
                    ManagementResourceHttpState, configure_management_resources,
                },
                management_security::{
                    ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
                    ManagementNetworkPolicy,
                },
            };
            let security = ManagementHttpState::new(
                ManagementKey::try_new(MANAGEMENT_KEY)?,
                ManagementNetworkPolicy::LoopbackOnly,
                ManagementBrowserPolicy::DenyBrowserOrigins,
            )?;
            let resources = ManagementResourceHttpState::new(
                gateway_control::management_mutation_service::ManagementMutationService::new(
                    SqliteControlPlaneRepository::open(&fixture.database)?,
                    fixture.secrets.clone(),
                ),
            )
            .with_provider_account_pools(Box::new(adapter));
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
                    .uri("/admin/operations/provider-account-pools?limit=100")
                    .peer_addr("127.0.0.1:1234".parse()?)
                    .insert_header(("X-Management-Key", MANAGEMENT_KEY))
                    .insert_header(("X-Config-Version", fixture.version.as_str()))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let body: serde_json::Value = test::read_body_json(response).await;
            let rows = body["items"].as_array().ok_or("management rows")?;
            for name in ["codex", "kimi", "claude", "kiro"] {
                let row = rows
                    .iter()
                    .find(|row| row["account_id"] == name)
                    .ok_or("account observation")?;
                assert_eq!(
                    row["auth_status"],
                    if status == 200 {
                        "active"
                    } else if status == 400 {
                        "reauth_required"
                    } else {
                        "expired"
                    }
                );
                assert_eq!(
                    row["runtime_status"],
                    if status == 200 {
                        "available"
                    } else if status == 400 {
                        "unauthorized"
                    } else {
                        "expired"
                    }
                );
                assert_eq!(row["active_leases"], 0);
            }
            assert_eq!(
                rows.iter()
                    .find(|row| row["account_id"] == "disabled")
                    .ok_or("disabled row")?["enabled"],
                false
            );
            assert!(
                rows.iter()
                    .all(|row| row.get("access_token").is_none()
                        && row.get("refresh_token").is_none())
            );
            Ok::<(), Box<dyn std::error::Error>>(())
        })?;
    }
    Ok(())
}
