//! #70 traverses an authenticated public TCP listener, the production factory/selector/leases,
//! native Build codec and a second TCP receiver. Only fixed-origin transport is handed off.
use super::*;
use actix_web::{HttpRequest, HttpResponse, HttpServer, dev::ServerHandle};
use gateway_core::{AccessGroupId, RouteCandidateId};
use gateway_store::control_plane::{ModelAliasConfiguration, RoutePolicy};
use provider_grok::{
    GrokAccountAuthStatus, GrokAccountCredential, GrokAccountImport, GrokAccountPoolStore,
    GrokAccountProvider,
};
use std::{
    net::{Ipv4Addr, TcpListener},
    sync::atomic::{AtomicBool, Ordering},
};

const SECRET: &[u8] = br#"{"access_token":"synthetic-affinity-a","refresh_token":"synthetic-affinity-refresh-a","expires_in":3600,"token_type":"Bearer"}"#;

#[derive(Clone, Debug)]
struct Received {
    endpoint: String,
    account: String,
    body: Value,
}
struct ReceiverState {
    rows: Mutex<Vec<Received>>,
    fail_once: AtomicBool,
    hold_gate: tokio::sync::Semaphore,
}
impl Default for ReceiverState {
    fn default() -> Self {
        Self {
            rows: Mutex::new(Vec::new()),
            fail_once: AtomicBool::new(false),
            hold_gate: tokio::sync::Semaphore::new(0),
        }
    }
}
struct Receiver {
    origin: String,
    state: Arc<ReceiverState>,
    server: ServerHandle,
}
impl Receiver {
    fn start() -> Result<Self, Box<dyn Error>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let origin = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
        let state = Arc::new(ReceiverState::default());
        let data = state.clone();
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(data.clone()))
                .default_service(web::post().to(receive))
        })
        .disable_signals()
        .workers(1)
        .listen(listener)?
        .run();
        let handle = server.handle();
        actix_web::rt::spawn(server);
        Ok(Self {
            origin,
            state,
            server: handle,
        })
    }
    fn rows(&self) -> Result<Vec<Received>, Box<dyn Error>> {
        Ok(self.state.rows.lock().map_err(|_| "receiver")?.clone())
    }
}
async fn receive(
    request: HttpRequest,
    bytes: web::Bytes,
    state: web::Data<Arc<ReceiverState>>,
) -> HttpResponse {
    let Ok(body) = serde_json::from_slice::<Value>(&bytes) else {
        return HttpResponse::BadRequest().finish();
    };
    let endpoint = request
        .headers()
        .get("x-synthetic-endpoint")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let account = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    if let Ok(mut rows) = state.rows.lock() {
        rows.push(Received {
            endpoint: format!("{endpoint}|{}", body["model"].as_str().unwrap_or_default()),
            account,
            body: body.clone(),
        });
    }
    if state.fail_once.swap(false, Ordering::AcqRel) {
        return HttpResponse::TooManyRequests()
            .json(json!({"error":{"message":"synthetic transient"}}));
    }
    if body.to_string().contains("hold-affinity") {
        match tokio::time::timeout(Duration::from_secs(15), state.hold_gate.acquire()).await {
            Ok(Ok(permit)) => permit.forget(),
            _ => return HttpResponse::GatewayTimeout().finish(),
        }
    }
    let peer = Peer {
        native_output: Some(
            json!([{"id":"msg-c","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"answer","annotations":[]}]}]),
        ),
        ..Peer::default()
    };
    let Ok((streaming, mut output)) = peer.reply("synthetic receiver", &bytes) else {
        return HttpResponse::InternalServerError().finish();
    };
    if streaming && body.to_string().contains("postsemantic-affinity") {
        if let Some(offset) = output
            .windows(b"event: response.completed".len())
            .position(|window| window == b"event: response.completed")
        {
            output.truncate(offset);
        }
    }
    HttpResponse::Ok()
        .content_type(if streaming {
            "text/event-stream"
        } else {
            "application/json"
        })
        .body(output)
}
struct SocketBuildTransport {
    origin: String,
    endpoint: String,
}
impl GrokBuildTransport for SocketBuildTransport {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            let streaming = serde_json::from_slice::<Value>(request.body())
                .map_err(|_| internal_error())?["stream"]
                == true;
            let reply = reqwest::Client::new()
                .post(&self.origin)
                .header(
                    "authorization",
                    request.header("authorization").ok_or_else(internal_error)?,
                )
                .header("x-synthetic-endpoint", &self.endpoint)
                .body(request.body().to_vec())
                .send()
                .await
                .map_err(|_| internal_error())?;
            let status = reply.status().as_u16();
            let bytes = reply.bytes().await.map_err(|_| internal_error())?.to_vec();
            Ok(GrokBuildTransportResponse::new(
                status,
                if streaming && status == 200 {
                    GrokBuildResponseContentType::EventStream
                } else {
                    GrokBuildResponseContentType::Json
                },
                provider_grok::GrokBuildResponseContentEncoding::Identity,
                Box::new(Body(Some(bytes))),
            ))
        })
    }
}

fn fixture_for(
    receiver: &Receiver,
    policy: RoutePolicy,
    candidates: usize,
) -> Result<(Fixture, String), Box<dyn Error>> {
    let mut second_key = String::new();
    let fixture = fixture_with_database_seed(
        Channel {
            adapter: "grok.build.responses",
            format: "openai/responses",
            base: provider_grok::GROK_BUILD_RESPONSES_BASE_URL,
            path: provider_grok::GROK_BUILD_RESPONSES_PATH,
            model: "grok-4.5",
        },
        SECRET,
        true,
        |configuration| {
            configuration.public_models[0].model_name = "grok-4.5".into();
            configuration.model_aliases.push(ModelAliasConfiguration {
                alias: "team/grok-4.5".into(),
                public_model_id: configuration.public_models[0].id.clone(),
            });
            configuration.route_candidates[0].capability_override_json =
                json!({"allow_unlisted_model":true}).to_string();
            configuration.model_routes[0].policy = policy;
            configuration.model_routes[0].max_attempts = 4;
            configuration.route_candidates[0].weight = 3;
            if candidates > 1 {
                let mut candidate = configuration.route_candidates[0].clone();
                candidate.id = RouteCandidateId::try_new("issue70-candidate-b")?;
                candidate.upstream_model = "grok-4.5-synthetic".into();
                candidate.weight = 1;
                configuration.route_candidates.push(candidate);
            }
            let issuer = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x63; 32])?);
            let (key, presented) = issuer
                .issue(
                    ClientKeyId::try_new("issue70-owner-b")?,
                    AccessGroupId::try_new(configuration.access_groups[0].id.as_str())?,
                    None,
                )?
                .into_parts();
            second_key = presented.as_str().to_owned();
            configuration.client_keys.push(StoredClientKey::try_new(
                key.client_key_id().clone(),
                key.access_group_id().clone(),
                key.prefix().as_str(),
                key.secret_digest().as_bytes(),
                StoredClientKeyStatus::Active,
                None,
            )?);
            Ok(())
        },
        |endpoints| {
            for (id, runtime) in endpoints {
                let mut profiles = P12TransportProfiles::try_new()?;
                profiles.build = Some(Arc::new(SocketBuildTransport {
                    origin: receiver.origin.clone(),
                    endpoint: id.as_str().to_owned(),
                }));
                runtime.transports = Arc::new(profiles);
            }
            Ok(())
        },
        synthetic_alias_proofs,
        deployment_route_compiler,
        |database, secrets| {
            let now = system_now_ms_runtime()?;
            let secret = br#"{"access_token":"synthetic-affinity-b","refresh_token":"synthetic-affinity-refresh-b","expires_in":3600,"token_type":"Bearer"}"#;
            let credential = GrokAccountCredential::try_from_build_credential(
                &GrokBuildCredential::import_json(secret, now)?,
            )?;
            let import = GrokAccountImport {
                provider: GrokAccountProvider::Build,
                identity: credential.enrollment_identity(GrokAccountProvider::Build, now, None)?,
                credential,
                auth_status: GrokAccountAuthStatus::Active,
                enabled: true,
                priority: 0,
                weight: 3,
                max_concurrency: 1,
                refresh_due_at_ms: None,
                quota_sync_due_at_ms: None,
                cooldown_until_ms: None,
            };
            GrokAccountPoolStore::try_open(database, secrets.clone())?.import_batch(
                "issue70-synthetic",
                &[import],
                now,
            )?;
            Ok(())
        },
    )?;
    Ok((fixture, second_key))
}
fn synthetic_alias_proofs(
    executor: &P12RoutedResponsesExecutor,
    configuration: &ControlPlaneConfiguration,
) -> Result<Arc<[provider_grok::GrokBuildModelAliasProof]>, Box<dyn Error>> {
    use gateway_router::ResponsesExecutionLineage;
    use provider_grok::{
        GrokBuildAliasAuthority, GrokBuildModelAliasAssertion, GrokBuildModelAliasProof,
        GrokBuildModelAliasScope,
    };
    use sha2::{Digest, Sha256};
    let snapshot = executor.scheduler.snapshot();
    let route = snapshot
        .route(&configuration.model_routes[0].id)
        .ok_or("route")?;
    let now = system_now_ms_runtime()?;
    let mut proofs = Vec::new();
    for candidate in route
        .candidates()
        .iter()
        .filter(|candidate| candidate.upstream_model() == "grok-4.5-synthetic")
    {
        for (endpoint, credential) in executor
            .scheduler
            .diagnostic_bindings()
            .filter(|(endpoint, _)| endpoint == candidate.endpoint_id())
        {
            for owner in &configuration.client_keys {
                for mode in [
                    provider_grok::GrokBuildExecutionMode::NonStreaming,
                    provider_grok::GrokBuildExecutionMode::Streaming,
                ] {
                    let source=b"SYNTHETIC issue70 local receiver alias, never applicable to a real provider";
                    proofs.push(GrokBuildModelAliasProof::freeze(
                        GrokBuildModelAliasScope {
                            lineage: ResponsesExecutionLineage::new(
                                snapshot.version().clone(),
                                ProviderId::try_new(candidate.upstream_id().as_str())?,
                                candidate.upstream_id().clone(),
                                endpoint.clone(),
                                route.id().clone(),
                                candidate.id().clone(),
                                credential.credential_id().clone(),
                                credential.credential_revision(),
                            ),
                            config_revision: configuration.version.revision,
                            channel: "grok.build".into(),
                            client_key_id: Some(owner.id().clone()),
                            client_protocol: ProtocolFormat::OpenAiResponses,
                            mode,
                            client_profile: provider_grok::GROK_BUILD_CLIENT_VERSION.into(),
                        },
                        GrokBuildModelAliasAssertion {
                            public_model: "grok-4.5".into(),
                            upstream_model: candidate.upstream_model().into(),
                            native_model: candidate.upstream_model().into(),
                            authority: GrokBuildAliasAuthority::ApprovedVerification,
                            source_reference: "synthetic:issue70/independent-receiver".into(),
                            source_sha256: format!("{:x}", Sha256::digest(source)),
                            valid_from_ms: now - 1000,
                            valid_until_ms: now + 60000,
                        },
                        source,
                    )?);
                }
            }
        }
    }
    Ok(proofs.into())
}

fn serve(state: ResponsesHttpState) -> Result<(String, ServerHandle), Box<dyn Error>> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let origin = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(gateway_http_actix::configure)
    })
    .disable_signals()
    .workers(2)
    .listen(listener)?
    .run();
    let handle = server.handle();
    actix_web::rt::spawn(server);
    Ok((origin, handle))
}
async fn call(
    origin: &str,
    key: &str,
    identity: &str,
    input: &str,
    streaming: bool,
) -> Result<(u16, Vec<u8>), Box<dyn Error>> {
    let reply=reqwest::Client::new().post(format!("{origin}/v1/responses")).bearer_auth(key)
        .header("content-type","application/json").body(serde_json::to_vec(&json!({"model":"team/grok-4.5", "input":input, "prompt_cache_key":identity, "stream":streaming}))?).send().await?;
    let status = reply.status().as_u16();
    Ok((status, reply.bytes().await?.to_vec()))
}
fn assert_idle(fixture: &Fixture) -> TestResult {
    assert!(
        fixture
            .executor
            .scheduler
            .diagnostic_bindings()
            .all(|(_, entry)| entry.active_leases() == 0)
    );
    Ok(())
}

#[actix_web::test]
async fn issue70_public_http_policies_rotate_candidates_and_credentials() -> TestResult {
    for (policy, count) in [
        (RoutePolicy::RoundRobin, 8),
        (RoutePolicy::WeightedRoundRobin, 16),
        (RoutePolicy::FillFirst, 8),
    ] {
        let receiver = Receiver::start()?;
        let (fixture, _) = fixture_for(&receiver, policy, 2)?;
        let (origin, server) = serve(fixture.state.clone())?;
        for index in 0..count {
            let (status, body) = call(
                &origin,
                &fixture.key,
                &format!("independent-{index}"),
                "question",
                false,
            )
            .await?;
            assert_eq!(
                status,
                200,
                "{:?}: {}",
                policy,
                String::from_utf8_lossy(&body)
            );
        }
        let rows = receiver.rows()?;
        assert_eq!(rows.len(), count);
        let mut shares = BTreeMap::<String, BTreeMap<String, usize>>::new();
        for row in &rows {
            *shares
                .entry(row.endpoint.clone())
                .or_default()
                .entry(row.account.clone())
                .or_default() += 1;
            assert_ne!(row.body["prompt_cache_key"].as_str(), Some("independent-0"));
        }
        let mut totals = shares
            .values()
            .map(|accounts| accounts.values().sum::<usize>())
            .collect::<Vec<_>>();
        totals.sort_unstable();
        match policy {
            RoutePolicy::RoundRobin => {
                assert_eq!(totals, vec![4, 4]);
                let mut accounts = BTreeMap::<String, usize>::new();
                for row in &rows {
                    *accounts.entry(row.account.clone()).or_default() += 1;
                }
                assert_eq!(accounts.values().copied().collect::<Vec<_>>(), vec![4, 4]);
                assert_ne!(rows[0].endpoint, rows[1].endpoint);
            }
            RoutePolicy::WeightedRoundRobin => {
                assert_eq!(totals, vec![4, 12]);
                let mut accounts = BTreeMap::<String, usize>::new();
                for row in &rows {
                    *accounts.entry(row.account.clone()).or_default() += 1;
                }
                assert_eq!(accounts.values().copied().collect::<Vec<_>>(), vec![4, 12]);
            }
            _ => {
                assert_eq!(totals, vec![8]);
                assert_eq!(shares.values().next().ok_or("shares")?.len(), 1);
            }
        }
        assert_idle(&fixture)?;
        server.stop(false).await;
        receiver.server.stop(false).await;
    }
    Ok(())
}

#[actix_web::test]
async fn issue70_public_http_affinity_expiry_owner_capacity_revision_and_health() -> TestResult {
    let receiver = Receiver::start()?;
    let (fixture, other_key) = fixture_for(&receiver, RoutePolicy::RoundRobin, 1)?;
    let (origin, server) = serve(fixture.state.clone())?;
    for _ in 0..3 {
        assert_eq!(
            call(&origin, &fixture.key, "same-identity", "question", false)
                .await?
                .0,
            200
        );
    }
    let rows = receiver.rows()?;
    assert!(rows.iter().all(|row| row.account == rows[0].account));
    let database = fixture.directory.path().join("batch-c.sqlite3");
    let connection = gateway_store::open(&database)?;
    connection.execute("UPDATE grok_build_cache_affinities SET expires_at_ms=1", [])?;
    assert_eq!(
        call(&origin, &fixture.key, "same-identity", "question", false)
            .await?
            .0,
        200
    );
    let rows = receiver.rows()?;
    assert_ne!(rows[3].account, rows[0].account);
    assert_eq!(
        call(&origin, &other_key, "same-identity", "question", false)
            .await?
            .0,
        200
    );
    let rows = receiver.rows()?;
    assert_ne!(
        rows[4].body["prompt_cache_key"],
        rows[3].body["prompt_cache_key"]
    );
    assert_ne!(rows[4].account, rows[3].account);
    let before = rows.len();
    let (held_origin, held_key) = (origin.clone(), fixture.key.clone());
    let held = actix_web::rt::spawn(async move {
        call(
            &held_origin,
            &held_key,
            "same-identity",
            "hold-affinity",
            false,
        )
        .await
        .map_err(|error| error.to_string())
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while receiver.rows().map_or(0, |rows| rows.len()) == before {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    assert_eq!(
        call(&origin, &fixture.key, "same-identity", "question", false)
            .await?
            .0,
        200
    );
    receiver.state.hold_gate.add_permits(1);
    assert_eq!(held.await?.map_err(std::io::Error::other)?.0, 200);
    let rows = receiver.rows()?;
    assert_ne!(rows[before].account, rows[before + 1].account);
    assert_idle(&fixture)?;
    // Exact revision replacement invalidates the soft binding even for the same credential ID.
    let endpoint = fixture.executor.endpoints.keys().next().ok_or("endpoint")?;
    let pool = fixture
        .pools
        .as_ref()
        .ok_or("native pools")?
        .pool(endpoint)
        .ok_or("pool")?;
    let expected_revision = pool
        .diagnostic_entries()
        .first()
        .ok_or("credential")?
        .credential_revision()
        + 1;
    for entry in pool.diagnostic_entries() {
        let lease = pool
            .try_lease_exact_eligible_at(entry.credential_id(), system_now_ms_runtime()?, |_| true)
            .ok_or("lease")?;
        let secret = lease.secret_bytes().to_vec();
        drop(lease);
        assert!(pool.replace_credential_if_revision(
            entry.credential_id(),
            entry.credential_revision(),
            gateway_upstream::CredentialMaterialReplacement {
                credential_revision: i64::try_from(entry.credential_revision() + 1)?,
                expires_at_ms: None,
                secret: gateway_upstream::CredentialSecret::try_new(secret)?
            }
        )?);
    }
    assert_eq!(
        call(&origin, &fixture.key, "same-identity", "question", false)
            .await?
            .0,
        200
    );
    let stored_revision:i64=connection.query_row("SELECT credential_revision FROM grok_build_cache_affinities WHERE client_key_id != 'issue70-owner-b'",[],|row|row.get(0))?;
    assert_eq!(u64::try_from(stored_revision)?, expected_revision);
    let unavailable: String=connection.query_row("SELECT credential_id FROM grok_build_cache_affinities WHERE client_key_id != 'issue70-owner-b'",[],|row|row.get(0))?;
    fixture
        .executor
        .runtime_health
        .mark_credential_unauthorized(
            endpoint.clone(),
            CredentialId::try_new(unavailable.clone())?,
        )?;
    assert_eq!(
        call(&origin, &fixture.key, "same-identity", "question", false)
            .await?
            .0,
        200
    );
    let rebound:String=connection.query_row("SELECT credential_id FROM grok_build_cache_affinities WHERE client_key_id != 'issue70-owner-b'",[],|row|row.get(0))?;
    assert_ne!(rebound, unavailable);
    let (breaks,unknown,expired):(i64,i64,i64)=connection.query_row("SELECT count(*),sum(estimated_cache_loss_tokens IS NULL),sum(reason='expired') FROM grok_build_affinity_breaks",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
    assert!(breaks >= 3);
    assert_eq!(breaks, unknown);
    assert_eq!(expired, 1);
    assert_idle(&fixture)?;
    server.stop(false).await;
    receiver.server.stop(false).await;
    Ok(())
}

#[actix_web::test]
async fn issue70_public_http_failover_and_postsemantic_no_retry() -> TestResult {
    let receiver = Receiver::start()?;
    let (fixture, _) = fixture_for(&receiver, RoutePolicy::RoundRobin, 2)?;
    let (origin, server) = serve(fixture.state.clone())?;
    receiver.state.fail_once.store(true, Ordering::Release);
    let (status, body) = call(
        &origin,
        &fixture.key,
        "failover-identity",
        "question",
        false,
    )
    .await?;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let rows = receiver.rows()?;
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].account, rows[1].account);
    assert_eq!(
        call(
            &origin,
            &fixture.key,
            "failover-identity",
            "question",
            false
        )
        .await?
        .0,
        200
    );
    assert_eq!(receiver.rows()?[2].account, rows[1].account);
    assert_eq!(receiver.rows()?[2].endpoint, rows[1].endpoint);
    let before = receiver.rows()?.len();
    let (_, body) = call(
        &origin,
        &fixture.key,
        "postsemantic-identity",
        "postsemantic-affinity",
        true,
    )
    .await?;
    assert!(String::from_utf8_lossy(&body).contains("error"));
    assert_eq!(receiver.rows()?.len(), before + 1);
    assert_idle(&fixture)?;
    server.stop(false).await;
    receiver.server.stop(false).await;
    Ok(())
}

#[actix_web::test]
async fn issue70_public_http_concurrent_capacity_releases_every_lease() -> TestResult {
    let receiver = Receiver::start()?;
    let (fixture, _) = fixture_for(&receiver, RoutePolicy::RoundRobin, 1)?;
    let (origin, server) = serve(fixture.state.clone())?;
    let mut jobs = Vec::new();
    for index in 0..12 {
        let (origin, key) = (origin.clone(), fixture.key.clone());
        jobs.push(actix_web::rt::spawn(async move {
            call(
                &origin,
                &key,
                &format!("concurrent-{index}"),
                "hold-affinity",
                false,
            )
            .await
            .map_err(|error| error.to_string())
        }));
    }
    tokio::time::timeout(Duration::from_secs(15), async {
        while jobs.iter().filter(|job| job.is_finished()).count() < 10 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    receiver.state.hold_gate.add_permits(2);
    let mut successes = 0;
    for job in jobs {
        let (status, _) = job.await?.map_err(std::io::Error::other)?;
        if status == 200 {
            successes += 1;
        } else {
            assert_eq!(status, 503);
        }
    }
    assert_eq!(successes, 2);
    assert_eq!(receiver.rows()?.len(), 2);
    assert_idle(&fixture)?;
    assert_eq!(
        call(&origin, &fixture.key, "after-capacity", "question", false)
            .await?
            .0,
        200
    );
    assert_idle(&fixture)?;
    server.stop(false).await;
    receiver.server.stop(false).await;
    Ok(())
}

#[actix_web::test]
async fn issue70_public_http_stored_lineage_never_uses_soft_fallback() -> TestResult {
    let receiver = Receiver::start()?;
    let (fixture, _) = fixture_for(&receiver, RoutePolicy::RoundRobin, 1)?;
    let (origin, server) = serve(fixture.state.clone())?;
    let client = reqwest::Client::new();
    let reply=client.post(format!("{origin}/v1/responses")).bearer_auth(&fixture.key)
        .header("content-type","application/json").body(serde_json::to_vec(&json!({"model":"team/grok-4.5", "input":"question", "prompt_cache_key":"strict-affinity", "store":true}))?).send().await?;
    assert_eq!(reply.status().as_u16(), 200);
    let body: Value = serde_json::from_slice(&reply.bytes().await?)?;
    let id = body["id"].as_str().ok_or("stored id")?;
    let connection = gateway_store::open(fixture.directory.path().join("batch-c.sqlite3"))?;
    let credential: String = connection.query_row(
        "SELECT credential_id FROM grok_build_cache_affinities",
        [],
        |row| row.get(0),
    )?;
    let endpoint = fixture
        .executor
        .endpoints
        .keys()
        .next()
        .ok_or("endpoint")?
        .clone();
    fixture
        .executor
        .runtime_health
        .mark_credential_unauthorized(endpoint, CredentialId::try_new(credential)?)?;
    let before = receiver.rows()?.len();
    let reply=client.post(format!("{origin}/v1/responses")).bearer_auth(&fixture.key)
        .header("content-type","application/json").body(serde_json::to_vec(&json!({"model":"team/grok-4.5", "input":"next-question", "prompt_cache_key":"strict-affinity", "previous_response_id":id}))?).send().await?;
    assert_ne!(reply.status().as_u16(), 200);
    let _ = reply.bytes().await?;
    assert_eq!(receiver.rows()?.len(), before);
    assert_eq!(
        call(
            &origin,
            &fixture.key,
            "strict-affinity",
            "ordinary fallback",
            false
        )
        .await?
        .0,
        200
    );
    assert_idle(&fixture)?;
    server.stop(false).await;
    receiver.server.stop(false).await;
    Ok(())
}
