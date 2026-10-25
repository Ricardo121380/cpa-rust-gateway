//! Count tests use `RuntimeFactory::bootstrap`'s actual production state, with only the network
//! handoff replaced by an independent loopback receiver. No real credentials or upstream dials.
use super::*;
use crate::runtime::reload::{RuntimeFactory, RuntimePublicationController};
use gateway_store::billing_ledger::SqliteBillingLedger;

pub(super) fn reply(
    body: &Value,
    script: Option<&Value>,
) -> Result<HttpResponse, actix_web::Error> {
    if body["system"] == "hold-count" {
        return Ok(HttpResponse::Ok()
            .content_type("application/json")
            .streaming(futures_util::stream::pending::<
                Result<web::Bytes, actix_web::Error>,
            >()));
    }
    let status = script.and_then(|s| s["status"].as_u64()).unwrap_or(200);
    let status = StatusCode::from_u16(
        u16::try_from(status).map_err(actix_web::error::ErrorInternalServerError)?,
    )
    .map_err(actix_web::error::ErrorInternalServerError)?;
    let body = script
        .and_then(|s| s.get("body"))
        .cloned()
        .unwrap_or(json!({"input_tokens":17}));
    Ok(HttpResponse::build(status).json(body))
}

struct CountFixture {
    state: ResponsesHttpState,
    _controller: Arc<RuntimePublicationController>,
    events: Arc<Events>,
    key: String,
    directory: TemporaryDirectory,
    writer: Option<tokio::task::JoinHandle<gateway_store::event_store::EventWriterMetrics>>,
}

#[allow(clippy::too_many_lines)] // Keep the isolated production database, identity and writer lifecycle auditable together.
fn production_fixture(
    peer: Arc<Peer>,
    native: bool,
    oauth: bool,
    prepare: impl FnOnce(&mut ControlPlaneConfiguration) -> TestResult,
) -> Result<CountFixture, Box<dyn Error>> {
    let directory = TemporaryDirectory::new()?;
    let database = directory.path().join("count.sqlite3");
    let secrets = test_secret_store()?;
    let mut config = p12_configuration_for(&secrets, "count-tokens")?;
    config.endpoints[0].adapter_id = "anthropic-compatible.messages".into();
    config.endpoints[0].api_format = "anthropic/messages".into();
    config.endpoints[0].base_url = if native {
        "https://api.anthropic.com"
    } else {
        "https://relay.test"
    }
    .into();
    config.endpoints[0].inference_path = "/v1/messages".into();
    config.egress_policies[0].allowed_hosts_json = json!([if native {
        "api.anthropic.com"
    } else {
        "relay.test"
    }])
    .to_string();
    config.route_candidates[0].transform_mode = TransformMode::CanonicalBridge;
    config.route_candidates[0].upstream_model = "claude-count-model".into();
    config
        .model_aliases
        .push(gateway_store::control_plane::ModelAliasConfiguration {
            alias: "count-alias".into(),
            public_model_id: config.public_models[0].id.clone(),
        });
    if oauth {
        let material = json!({"kind":"claude_oauth","access_token":"synthetic-count-access","refresh_token":"synthetic-count-refresh","expires_at_ms":system_now_ms_runtime()?+3_600_000}).to_string();
        config.credentials[0].kind = "oauth_json".into();
        config.credentials[0].encrypted_secret = secrets.seal(
            material.as_bytes(),
            &credential_associated_data(
                &config.version.id,
                &config.credentials[0].id,
                &config.credentials[0].upstream_id,
            )?,
        )?;
    }
    let issuer = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x68; 32])?);
    let (key, presented) = issuer
        .issue(
            config.client_keys[0].id().clone(),
            config.access_groups[0].id.clone(),
            None,
        )?
        .into_parts();
    config.client_keys = vec![StoredClientKey::try_new(
        key.client_key_id().clone(),
        key.access_group_id().clone(),
        key.prefix().as_str(),
        key.secret_digest().as_bytes(),
        StoredClientKeyStatus::Active,
        None,
    )?];
    prepare(&mut config)?;
    let mut repository = SqliteControlPlaneRepository::open(&database)?;
    repository.write_configuration(&config)?;
    repository.activate_version(&config.version.id)?;
    let lifecycle = ManagementService::bootstrap(
        SqliteControlPlaneRepository::open(&database)?,
        deployment_route_compiler(&database)?,
        ManagementActor::try_new("count-test")?,
    )?;
    let events = Arc::new(Events::default());
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::default())?;
    *events.1.lock().map_err(|_| "sink")? = Some(Arc::new(queue));
    let writer = AsyncSqliteEventWriter::new(&database, receiver, EventWriterConfig::default());
    let writer = tokio::spawn(writer.run());
    let factory = RuntimeFactory {
        count_http: Some(peer),
        database: database.clone(),
        secret_store: secrets.clone(),
        client_keys: Arc::new(issuer),
        cache_identity: Arc::new(GrokBuildCacheIdentityDeriver::new([0x69; 32])),
        codex_proxy: UpstreamProxy::Direct,
        web_proxy: None,
        flaresolverr_proxy: None,
        flaresolverr_port: 8191,
        attempt_stages: Arc::new(P12AttemptStageStore::new()),
        runtime_health: Arc::new(RuntimeHealthRegistry::new()),
        runtime_quota: Arc::new(RuntimeQuotaRegistry::new()),
        event_sink: events.clone(),
        stored_responses: Arc::new(SqliteStoredResponseStore::open(&database, secrets)?),
        concurrency: gateway_upstream::CredentialConcurrencyRegistry::default(),
        publication_gate: Arc::new(tokio::sync::Mutex::new(())),
        lifecycle_registry: lifecycle.registry().clone(),
    };
    let (controller, state, _, _, _, _, _, _) = std::thread::scope(|scope| {
        scope
            .spawn(|| RuntimePublicationController::bootstrap(factory, Some(&config)))
            .join()
            .map_err(|_| "count factory thread")
    })??;
    Ok(CountFixture {
        state,
        _controller: controller,
        events,
        key: presented.as_str().into(),
        directory,
        writer: Some(writer),
    })
}

fn serve(state: ResponsesHttpState) -> Result<(String, ServerHandle), Box<dyn Error>> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let origin = format!("http://{}", listener.local_addr()?);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(gateway_http_actix::configure)
    })
    .disable_signals()
    .workers(1)
    .listen(listener)?
    .run();
    let handle = server.handle();
    actix_web::rt::spawn(server);
    Ok((origin, handle))
}
fn payload(model: &str) -> Value {
    json!({"model":model,"system":"System text.","messages":[{"role":"user","content":"Hello."}],"max_tokens":1,"metadata":{"user_id":"synthetic"},"context_management":{},"diagnostics":{}})
}
async fn call(
    client: &reqwest::Client,
    origin: &str,
    key: &str,
    body: Value,
) -> Result<reqwest::Response, reqwest::Error> {
    client
        .post(format!("{origin}/v1/messages/count_tokens"))
        .bearer_auth(key)
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await
}
async fn assert_no_usage(fixture: &mut CountFixture) -> TestResult {
    fixture.events.1.lock().map_err(|_| "sink")?.take();
    let metrics = tokio::time::timeout(
        Duration::from_secs(5),
        fixture.writer.take().ok_or("writer")?,
    )
    .await??;
    assert_eq!(metrics.pending_required, 0);
    assert_eq!(metrics.required_events_quarantined, 0);
    assert!(fixture.events.0.lock().map_err(|_| "events")?.is_empty());
    let database = fixture.directory.path().join("count.sqlite3");
    assert!(SqliteEventStore::open(&database)?.list_events()?.is_empty());
    let worker = crate::billing_worker::BillingWorker::start(
        database.clone(),
        Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
    );
    worker.stop().await.map_err(|()| "billing worker")?;
    assert!(
        SqliteBillingLedger::open_read_only(&database)?
            .list_bounded(10)?
            .is_empty()
    );
    let connection = gateway_store::open(&database)?;
    assert_eq!(gateway_store::schema_version(&connection)?, Some(39));
    let history_rows: i64 =
        connection.query_row("SELECT count(*) FROM quota_history", [], |row| row.get(0))?;
    assert_eq!(
        history_rows, 0,
        "counting must not manufacture quota observations"
    );
    Ok(())
}

#[actix_web::test]
async fn count_production_http_native_and_estimated_never_infer_or_bill() -> TestResult {
    let client = reqwest::Client::new();
    for native in [true, false] {
        for oauth in [false, true] {
            let peer = Peer::start(false, None)?;
            let mut fixture = production_fixture(peer.clone(), native, oauth, |_| Ok(()))?;
            let (origin, server) = serve(fixture.state.clone())?;
            for model in ["p12-test-model", "count-alias", "claude-count-model"] {
                let response = call(&client, &origin, &fixture.key, payload(model)).await?;
                assert_eq!(response.status(), 200);
                assert_eq!(
                    response.headers()["x-cpar-token-count-method"],
                    if native {
                        "provider-reported"
                    } else {
                        "estimated-cpa-o200k-base"
                    }
                );
                assert_eq!(
                    serde_json::from_str::<Value>(&response.text().await?)?,
                    json!({"input_tokens":if native {17} else {7}})
                );
            }
            let fixtures: Value = serde_json::from_str(include_str!(
                "../../../../../../tests/fixtures/count-tokens/cpa-v8.json"
            ))?;
            for case in fixtures["cases"].as_array().ok_or("cases")? {
                let raw = case["request"]
                    .as_str()
                    .ok_or("request")?
                    .replace("claude-test", "count-alias");
                let response = client
                    .post(format!("{origin}/v1/messages/count_tokens"))
                    .bearer_auth(&fixture.key)
                    .header("content-type", "application/json")
                    .body(raw)
                    .send()
                    .await?;
                assert_eq!(response.status(), 200, "{}", case["name"]);
                let expected = if native {
                    17
                } else {
                    case["expected_input_tokens"].as_u64().ok_or("count")?
                };
                assert_eq!(
                    serde_json::from_str::<Value>(&response.text().await?)?,
                    json!({"input_tokens":expected})
                );
            }
            for body in [
                payload("invisible"),
                json!({"model":"count-alias","messages":[{"role":"user","content":[{"type":"tool_result","tool_use_id":"c","content":[["x"]]}]}]}),
                json!({"model":"count-alias","messages":[{"role":"user","content":[{"type":"tool_result","tool_use_id":"c","content":["x"]}]}]}),
                json!({"model":"count-alias","messages":[{"role":"user","content":[{"type":"tool_result","tool_use_id":"c","content":{"type":"text","text":"x"}}]}]}),
                json!({"model":"count-alias","messages":[{"role":"user","content":"x"}],"max_tokens":0}),
                json!({"model":"count-alias","messages":[{"role":"user","content":"x"}],"tool_choice":{"type":"tool"}}),
                json!({"model":"count-alias","messages":[]}),
                json!({"model":"count-alias","messages":[{"role":"user","content":[{"type":"image","source":{"type":"url","url":"https://private.test/img"}}]}]}),
                json!({"model":"count-alias","messages":[{"role":"user","content":"x"}],"unknown_content":"do not skip"}),
                json!({"model":"count-alias","messages":[{"role":"user","content":"x"}],"stream":true}),
                json!({"model":"count-alias","messages":[{"role":"user","content":"x".repeat(256*1024)}]}),
            ] {
                let response = call(&client, &origin, &fixture.key, body).await?;
                assert!(response.status().is_client_error());
                let error = serde_json::from_str::<Value>(&response.text().await?)?;
                assert!(error.get("input_tokens").is_none());
                assert_eq!(error["type"], "error");
            }
            assert_eq!(
                call(&client, &origin, "invalid", payload("count-alias"))
                    .await?
                    .status(),
                401
            );
            let duplicate = client
                .post(format!("{origin}/v1/messages/count_tokens"))
                .bearer_auth(&fixture.key)
                .body(r#"{"model":"count-alias","model":"bad","messages":[]}"#)
                .send()
                .await?;
            assert_eq!(duplicate.status(), 400);
            assert_eq!(
                peer.received.lock().map_err(|_| "received")?.len(),
                if native { 11 } else { 0 }
            );
            let paths = peer.received_paths.lock().map_err(|_| "paths")?.clone();
            assert!(paths.iter().all(|p| p == "/v1/messages/count_tokens"));
            assert_eq!(
                paths
                    .iter()
                    .filter(|p| p.as_str() == "/v1/messages")
                    .count(),
                0
            );
            if native {
                let requests = peer.requests.lock().map_err(|_| "requests")?;
                for request in requests.iter() {
                    assert_eq!(
                        request.url,
                        "https://api.anthropic.com/v1/messages/count_tokens?beta=true"
                    );
                    assert_eq!(request.body["model"], "claude-count-model");
                    for field in [
                        "max_tokens",
                        "metadata",
                        "context_management",
                        "diagnostics",
                    ] {
                        assert!(request.body.get(field).is_none());
                    }
                    assert_eq!(request.headers.contains_key("authorization"), oauth);
                    assert_eq!(request.headers.contains_key("x-api-key"), !oauth);
                    assert!(
                        request.headers["anthropic-beta"].contains("token-counting-2024-11-01")
                    );
                }
            }
            assert_no_usage(&mut fixture).await?;
            server.stop(false).await;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn count_upstream_failure_never_estimates_or_returns_fabricated_zero() -> TestResult {
    let client = reqwest::Client::new();
    for (status, body) in [
        (
            500,
            json!({"error":{"type":"api_error","message":"private-upstream-detail"}}),
        ),
        (429, json!({"error":{"type":"rate_limit_error"}})),
        (200, json!({})),
        (200, json!({"input_tokens":-1})),
        (200, json!({"input_tokens":1.5})),
        (200, json!({"input_tokens":"17"})),
    ] {
        let peer = Peer::start(false, Some(json!({"status":status,"body":body})))?;
        let mut fixture = production_fixture(peer.clone(), true, false, |_| Ok(()))?;
        let (origin, server) = serve(fixture.state.clone())?;
        let response = call(&client, &origin, &fixture.key, payload("count-alias")).await?;
        assert!(response.status().is_client_error() || response.status().is_server_error());
        assert!(
            response
                .headers()
                .get("x-cpar-token-count-method")
                .is_none()
        );
        let body = response.text().await?;
        assert!(!body.contains("input_tokens") && !body.contains("private-upstream-detail"));
        assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), 1);
        assert_no_usage(&mut fixture).await?;
        server.stop(false).await;
    }
    Ok(())
}

#[actix_web::test]
async fn count_denied_routes_and_exact_model_filter_fail_before_transport() -> TestResult {
    let client = reqwest::Client::new();
    for deny in ["access", "disabled-candidate", "disabled-credential"] {
        let peer = Peer::start(false, None)?;
        let built = production_fixture(peer.clone(), true, false, |config| {
            match deny {
                "access" => config.access_group_routes.clear(),
                "disabled-candidate" => config.route_candidates[0].enabled = false,
                _ => {
                    config.credentials[0].status =
                        gateway_store::control_plane::CredentialStatus::Disabled;
                }
            }
            Ok(())
        });
        if deny == "disabled-candidate" {
            let Err(error) = built else {
                return Err("disabled route must fail publication".into());
            };
            assert!(format!("{error:?}").contains("RouteHasNoHardEligibleCandidate"));
            assert!(peer.received_paths.lock().map_err(|_| "paths")?.is_empty());
            continue;
        }
        let mut fixture = built?;
        let (origin, server) = serve(fixture.state.clone())?;
        for model in ["p12-test-model", "count-alias", "claude-count-model"] {
            let response = call(&client, &origin, &fixture.key, payload(model)).await?;
            assert!(!response.status().is_success(), "{deny}");
        }
        assert!(peer.requests.lock().map_err(|_| "requests")?.is_empty());
        assert!(peer.received_paths.lock().map_err(|_| "paths")?.is_empty());
        assert_no_usage(&mut fixture).await?;
        server.stop(false).await;
    }
    let peer = Peer::start(false, None)?;
    let mut fixture = production_fixture(peer.clone(), true, false, |config| {
        let mut other = config.route_candidates[0].clone();
        other.id = gateway_core::RouteCandidateId::try_new("count-other-model")?;
        other.upstream_model = "claude-other-model".into();
        config.route_candidates[0].priority = 1;
        other.priority = 0;
        config.route_candidates.push(other);
        Ok(())
    })?;
    let (origin, server) = serve(fixture.state.clone())?;
    let response = call(
        &client,
        &origin,
        &fixture.key,
        payload("claude-count-model"),
    )
    .await?;
    assert_eq!(response.status(), 200);
    assert_eq!(
        peer.requests.lock().map_err(|_| "requests")?[0].body["model"],
        "claude-count-model"
    );
    assert_no_usage(&mut fixture).await?;
    server.stop(false).await;
    Ok(())
}

#[actix_web::test]
async fn count_stale_snapshot_and_cancelled_native_future_release_capacity() -> TestResult {
    use gateway_router::{CountTokensExecution, CountTokensExecutor};
    let peer = Peer::start(false, None)?;
    let fixture = fixture(
        Channel {
            adapter: "anthropic-compatible.messages",
            format: "anthropic/messages",
            base: "https://api.anthropic.com",
            path: "/v1/messages",
            model: "claude-count-model",
        },
        b"synthetic-key",
        |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.http = Some(peer.clone());
            endpoint.transports = Arc::new(profiles);
            endpoint.resolver = Arc::new(PublicResolver);
            Ok(())
        },
    )?;
    let snapshot = fixture.executor.scheduler.snapshot();
    let execution = |snapshot| -> Result<CountTokensExecution, Box<dyn Error>> {
        let request = CanonicalRequest {
            model_thinking: None,
            requested_model: "count-alias".into(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            extensions: RawExtensions::default(),
        };
        let mut body = payload("count-alias");
        body["system"] = json!("hold-count");
        Ok(CountTokensExecution::new(
            RequestContext::new(RequestId::try_new("count-cancel")?),
            request,
            Some(RouteId::try_new("p12-runtime-route")?),
        )
        .with_route_snapshot(Some(snapshot), Some("claude-count-model".into()))
        .with_native_payload(Arc::from(body.to_string())))
    };
    let stale = execution(Arc::new(snapshot.as_ref().clone()))?;
    assert!(fixture.executor.count_tokens(stale).await.is_err());
    assert!(peer.requests.lock().map_err(|_| "requests")?.is_empty());
    let admitted = execution(snapshot)?;
    let executor = fixture.executor.clone();
    let task = tokio::spawn(async move { executor.count_tokens(admitted).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if !peer.received_paths.lock().map_err(|_| "paths")?.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok::<_, Box<dyn Error>>(())
    })
    .await??;
    assert_eq!(
        fixture
            .executor
            .scheduler
            .diagnostic_bindings()
            .map(|(_, b)| b.active_leases())
            .sum::<usize>(),
        1
    );
    task.abort();
    assert!(task.await.is_err());
    assert_eq!(
        fixture
            .executor
            .scheduler
            .diagnostic_bindings()
            .map(|(_, b)| b.active_leases())
            .sum::<usize>(),
        0
    );
    assert!(fixture.events.0.lock().map_err(|_| "events")?.is_empty());
    Ok(())
}

#[actix_web::test]
async fn cpa_69_count_names_reuse_authorized_resolution_without_inference() -> TestResult {
    let client = reqwest::Client::new();
    for native in [false, true] {
        let peer = Peer::start(false, None)?;
        let mut denied = None;
        let mut fixture = production_fixture(peer.clone(), native, false, |config| {
            config
                .model_aliases
                .push(gateway_store::control_plane::ModelAliasConfiguration {
                    alias: gateway_router::prefixed_model_alias("count-team", "model")?,
                    public_model_id: config.public_models[0].id.clone(),
                });
            let group = gateway_core::AccessGroupId::try_new("count-denied")?;
            config
                .access_groups
                .push(gateway_store::control_plane::AccessGroupConfiguration {
                    id: group.clone(),
                    name: "denied".into(),
                    limits_json: "{}".into(),
                    status: gateway_store::control_plane::AdministrativeStatus::Active,
                });
            let issuer = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x68; 32])?);
            let (key, presented) = issuer
                .issue(ClientKeyId::try_new("count-denied-key")?, group, None)?
                .into_parts();
            denied = Some(presented.as_str().to_owned());
            config.client_keys.push(StoredClientKey::try_new(
                key.client_key_id().clone(),
                key.access_group_id().clone(),
                key.prefix().as_str(),
                key.secret_digest().as_bytes(),
                StoredClientKeyStatus::Active,
                None,
            )?);
            Ok(())
        })?;
        let (origin, server) = serve(fixture.state.clone())?;
        for name in [
            "count-team/model",
            "count-team/model(high)",
            "claude-count-model(0)",
        ] {
            let response = call(&client, &origin, &fixture.key, payload(name)).await?;
            assert_eq!(response.status(), 200, "{name}: {}", response.text().await?);
        }
        for name in [
            "other/model(high)",
            "count-team/model(bogus)",
            "count-team/model(-2)",
            "count-team/model()",
        ] {
            assert!(
                call(&client, &origin, &fixture.key, payload(name))
                    .await?
                    .status()
                    .is_client_error()
            );
        }
        let denied = denied.ok_or("denied key")?;
        for name in [
            "count-team/model",
            "count-team/model(high)",
            "claude-count-model(0)",
        ] {
            assert_eq!(
                call(&client, &origin, &denied, payload(name))
                    .await?
                    .status(),
                404
            );
        }
        assert_eq!(
            peer.received.lock().map_err(|_| "receiver")?.len(),
            if native { 3 } else { 0 }
        );
        assert_no_usage(&mut fixture).await?;
        server.stop(false).await;
        peer.server.stop(false).await;
    }
    Ok(())
}

#[actix_web::test]
async fn cpa_69_count_native_effort_and_format_match_inference_admission() -> TestResult {
    let client = reqwest::Client::new();
    for native in [false, true] {
        let peer = Peer::start(false, None)?;
        let mut fixture = production_fixture(peer.clone(), native, false, |config| {
            config.route_candidates[0].upstream_model = "claude-sonnet-4-6".into();
            config.route_candidates[0].transform_mode = TransformMode::Passthrough;
            Ok(())
        })?;
        let (origin, server) = serve(fixture.state.clone())?;
        let mut body = payload("count-alias(high)");
        body["output_config"] = json!({"effort":"low","format":{"type":"json_schema"}});
        let response = call(&client, &origin, &fixture.key, body.clone()).await?;
        let status = response.status();
        let text = response.text().await?;
        assert_eq!(status, 200, "{text}");
        body["model"] = json!("count-alias(minimal)");
        let response = call(&client, &origin, &fixture.key, body).await?;
        assert!(!response.status().is_success());
        let text = response.text().await?;
        assert!(
            serde_json::from_str::<Value>(&text)?
                .get("input_tokens")
                .is_none()
        );
        assert_eq!(
            peer.received.lock().map_err(|_| "receiver")?.len(),
            usize::from(native)
        );
        assert_no_usage(&mut fixture).await?;
        server.stop(false).await;
        peer.server.stop(false).await;
    }
    Ok(())
}
