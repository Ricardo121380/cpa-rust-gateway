//! #71 actual management/public sockets with a separate, controlled upstream receiver.
use super::*;
use crate::runtime::reload::{RuntimeFactory, RuntimePublicationController};
use gateway_control::management_mutation_service::ManagementMutationService;
use gateway_http_actix::{
    management_lifecycle_resources::ManagementLifecycleHttpState,
    management_resources::ManagementResourceHttpState,
    management_security::{
        ManagementBrowserPolicy, ManagementHttpState, ManagementKey, ManagementNetworkPolicy,
    },
};
const ADMIN: &str = "mgmt_issue71_synthetic_0123456789abcdef";

struct RuleFixture {
    controller: Arc<RuntimePublicationController>,
    lifecycle: web::Data<ManagementLifecycleHttpState>,
    resources: web::Data<ManagementResourceHttpState>,
    key: String,
    foreign: String,
    denied: String,
    database: std::path::PathBuf,
    endpoint: EndpointConfiguration,
    version: String,
}
#[allow(clippy::too_many_lines)]
fn fixture(peer: Arc<Peer>, target: &str) -> Result<RuleFixture, Box<dyn Error>> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../output/issue-71/state")
        .join(format!(
            "rules-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
    std::fs::create_dir_all(&root)?;
    let database = root.join("control.sqlite3");
    let secrets = test_secret_store()?;
    let mut config = p12_configuration_for(&secrets, "rules-base")?;
    let (adapter, format, path, model) = match target {
        "messages" => (
            "anthropic-compatible.messages",
            "anthropic/messages",
            "/v1/messages",
            "claude-sonnet-4-5-20250929",
        ),
        "chat" => (
            "openai-compatible.chat-completions",
            "openai/chat-completions",
            "/v1/chat/completions",
            "gpt-5.5",
        ),
        _ => (
            "openai-compatible.responses",
            "openai/responses",
            "/v1/responses",
            "gpt-5.5",
        ),
    };
    config.endpoints[0].adapter_id = adapter.into();
    config.endpoints[0].api_format = format.into();
    config.endpoints[0].base_url = "https://relay.test".into();
    config.endpoints[0].inference_path = path.into();
    config.egress_policies[0].allowed_hosts_json = json!(["relay.test"]).to_string();
    config.route_candidates[0].transform_mode = TransformMode::CanonicalBridge;
    config.route_candidates[0].upstream_model = model.into();
    // Owned storage is explicit; rules cannot change storage or reasoning qualification.
    if target == "responses" {
        config.route_candidates[0].capability_override_json =
            json!({"allow_unlisted_model":true,"stored_responses":true}).to_string();
    }
    config
        .model_aliases
        .push(gateway_store::control_plane::ModelAliasConfiguration {
            alias: "team/p12-test-model".into(),
            public_model_id: config.public_models[0].id.clone(),
        });
    let mut unmatched = config.public_models[0].clone();
    unmatched.id = gateway_core::PublicModelId::try_new("unmatched")?;
    unmatched.model_name = "unmatched".into();
    let mut route = config.model_routes[0].clone();
    route.id = RouteId::try_new("unmatched-route")?;
    route.public_model_id = unmatched.id.clone();
    let mut candidate = config.route_candidates[0].clone();
    candidate.id = gateway_core::RouteCandidateId::try_new("unmatched-candidate")?;
    candidate.route_id = route.id.clone();
    config.access_group_routes.push(
        gateway_store::control_plane::AccessGroupRouteConfiguration {
            access_group_id: config.access_groups[0].id.clone(),
            enabled: true,
            route_id: route.id.clone(),
        },
    );
    config.public_models.push(unmatched);
    config.model_routes.push(route);
    config.route_candidates.push(candidate);
    let denied_group = gateway_core::AccessGroupId::try_new("denied")?;
    config
        .access_groups
        .push(gateway_store::control_plane::AccessGroupConfiguration {
            id: denied_group.clone(),
            name: "denied".into(),
            status: gateway_store::control_plane::AdministrativeStatus::Active,
            limits_json: "{}".into(),
        });
    let issuer = Arc::new(ClientKeyService::new(ClientKeyPepper::try_from_bytes(
        [0x71; 32],
    )?));
    config.client_keys.clear();
    let mut keys = Vec::new();
    for (id, group) in [
        ("owner", config.access_groups[0].id.clone()),
        ("foreign", config.access_groups[0].id.clone()),
        ("denied", denied_group),
    ] {
        let (key, presented) = issuer
            .issue(ClientKeyId::try_new(id)?, group, None)?
            .into_parts();
        config.client_keys.push(StoredClientKey::try_new(
            key.client_key_id().clone(),
            key.access_group_id().clone(),
            key.prefix().as_str(),
            key.secret_digest().as_bytes(),
            StoredClientKeyStatus::Active,
            None,
        )?);
        keys.push(presented.as_str().to_owned());
    }
    let mut repository = SqliteControlPlaneRepository::open(&database)?;
    repository.write_configuration(&config)?;
    repository.activate_version(&config.version.id)?;
    let mut lifecycle = ManagementService::bootstrap(
        SqliteControlPlaneRepository::open(&database)?,
        deployment_route_compiler(&database)?,
        ManagementActor::try_new("rules-test")?,
    )?;
    let factory = RuntimeFactory {
        count_http: Some(peer),
        database: database.clone(),
        secret_store: secrets.clone(),
        client_keys: issuer,
        cache_identity: Arc::new(GrokBuildCacheIdentityDeriver::new([0x71; 32])),
        codex_proxy: UpstreamProxy::Direct,
        web_proxy: None,
        flaresolverr_proxy: None,
        flaresolverr_port: 8191,
        attempt_stages: Arc::new(P12AttemptStageStore::new()),
        runtime_health: Arc::new(RuntimeHealthRegistry::new()),
        runtime_quota: Arc::new(RuntimeQuotaRegistry::new()),
        event_sink: Arc::new(Events::default()),
        stored_responses: Arc::new(SqliteStoredResponseStore::open(&database, secrets.clone())?),
        concurrency: gateway_upstream::CredentialConcurrencyRegistry::default(),
        publication_gate: Arc::new(tokio::sync::Mutex::new(())),
        lifecycle_registry: lifecycle.registry().clone(),
    };
    let (controller, _, _, _, _, _, _, _) = std::thread::scope(|scope| {
        scope
            .spawn(|| RuntimePublicationController::bootstrap(factory, Some(&config)))
            .join()
            .map_err(|_| "factory thread")
    })??;
    lifecycle.set_runtime_preparer(controller.clone());
    Ok(RuleFixture {
        controller,
        lifecycle: web::Data::new(ManagementLifecycleHttpState::new(lifecycle)),
        resources: web::Data::new(ManagementResourceHttpState::new(
            ManagementMutationService::new(SqliteControlPlaneRepository::open(&database)?, secrets),
        )),
        key: keys.remove(0),
        foreign: keys.remove(0),
        denied: keys.remove(0),
        database,
        endpoint: config.endpoints.remove(0),
        version: config.version.id.as_str().into(),
    })
}
fn serve(f: &RuleFixture) -> Result<(String, ServerHandle, String, ServerHandle), Box<dyn Error>> {
    let security = web::Data::new(ManagementHttpState::new(
        ManagementKey::try_new(ADMIN)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?);
    let resources = f.resources.clone();
    let lifecycle = f.lifecycle.clone();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let admin = format!("http://{}", listener.local_addr()?);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(resources.clone())
            .app_data(lifecycle.clone())
            .app_data(security.clone())
            .configure(gateway_http_actix::configure_management_listener)
    })
    .disable_signals()
    .workers(2)
    .listen(listener)?
    .run();
    let ah = server.handle();
    actix_web::rt::spawn(server);
    let source: Arc<dyn gateway_http_actix::ResponsesStateSource> = f.controller.clone();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let public = format!("http://{}", listener.local_addr()?);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::from(source.clone()))
            .configure(gateway_http_actix::configure)
    })
    .disable_signals()
    .workers(1)
    .listen(listener)?
    .run();
    let ph = server.handle();
    actix_web::rt::spawn(server);
    Ok((admin, ah, public, ph))
}
#[allow(clippy::too_many_arguments)]
async fn admin(
    client: &reqwest::Client,
    origin: &str,
    method: reqwest::Method,
    path: &str,
    version: &str,
    revision: u64,
    body: Value,
    status: u16,
) -> Result<Value, Box<dyn Error>> {
    let response = client
        .request(method, format!("{origin}{path}"))
        .header("x-management-key", ADMIN)
        .header("x-config-version", version)
        .header("if-match", format!("rev-{revision}"))
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await?;
    let code = response.status().as_u16();
    let text = response.text().await?;
    assert_eq!(code, status, "{path}: {text}");
    assert!(!text.contains("synthetic-request-tag"));
    Ok(serde_json::from_str(&text)?)
}
fn endpoint_body(f: &RuleFixture, policy: &Value) -> Value {
    json!({"id":f.endpoint.id.as_str(),"adapter_id":f.endpoint.adapter_id,"api_format":f.endpoint.api_format,"base_url":f.endpoint.base_url,"inference_path":f.endpoint.inference_path,"models_path":null,"transport":"https","enabled":true,"request_policy":policy})
}
fn payload(source: &str, model: &str, stream: bool) -> Value {
    match source {
        "messages" => {
            json!({"model":model,"max_tokens":1000,"stream":stream,"messages":[{"role":"user","content":"question"}],"top_p":0.9})
        }
        "chat" => {
            json!({"model":model,"max_completion_tokens":1000,"stream":stream,"messages":[{"role":"user","content":"question"}],"top_p":0.9})
        }
        _ => {
            json!({"model":model,"max_output_tokens":1000,"stream":stream,"input":"question","top_p":0.9})
        }
    }
}
fn uri(source: &str) -> &str {
    match source {
        "messages" => "/v1/messages",
        "chat" => "/v1/chat/completions",
        _ => "/v1/responses",
    }
}
async fn public(
    client: &reqwest::Client,
    origin: &str,
    key: &str,
    source: &str,
    body: Value,
    status: u16,
) -> Result<String, Box<dyn Error>> {
    let response = client
        .post(format!("{origin}{}", uri(source)))
        .bearer_auth(key)
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await?;
    let code = response.status().as_u16();
    let text = response.text().await?;
    assert_eq!(code, status, "{text}");
    assert!(!text.contains("synthetic-request-tag"));
    Ok(text)
}

#[actix_web::test]
#[allow(clippy::too_many_lines)]
async fn request_rules_real_management_publish_public_json_sse_and_rollback() -> TestResult {
    for target in ["responses", "chat", "messages"] {
        let peer = Peer::start(false, None)?;
        let f = fixture(peer.clone(), target)?;
        let (a, ah, p, ph) = serve(&f)?;
        let client = reqwest::Client::builder().no_proxy().build()?;
        let ep = format!("/admin/endpoints/{}", f.endpoint.id.as_str());
        // Baseline body and absent header survive empty policy exactly.
        public(
            &client,
            &p,
            &f.key,
            target,
            payload(target, "p12-test-model", false),
            200,
        )
        .await?;
        assert!(!peer.custom_header_receives.lock().map_err(|_| "headers")?[0]);
        admin(
            &client,
            &a,
            reqwest::Method::POST,
            &format!("/admin/config-versions/{}/fork", f.version),
            &f.version,
            0,
            json!({"id":"rules-next","description":"rule configuration"}),
            201,
        )
        .await?;
        let credential = format!(
            "/admin/upstreams/{}/credentials",
            f.endpoint.upstream_id.as_str()
        );
        for value in ["bad\r\nvalue".to_owned(), "x".repeat(4097)] {
            admin(&client,&a,reqwest::Method::POST,&credential,"rules-next",0,json!({"id":"invalid-header","kind":"upstream_header","secret":value,"status":"active"}),400).await?;
        }
        admin(&client,&a,reqwest::Method::POST,&credential,"rules-next",0,json!({"id":"header-tag","kind":"upstream_header","secret":"synthetic-request-tag","status":"active"}),201).await?;
        let limit = match target {
            "responses" => "max_output_tokens",
            "chat" => "max_completion_tokens",
            _ => "max_tokens",
        };
        let policy = json!({"rules":[{"models":["p12-test-model"],"default":{"temperature":0.2},"override":{limit:128},"filter":["top_p"]},{"models":["p12-test-model"],"protocol":"openai/chat-completions","override":{"temperature":0.3}}],"headers":{"x-request-tag":"header-tag"}});
        // Illegal paths, identities, values and ambiguous operations fail at HTTP save.
        for invalid in [
            json!({"rules":[{"models":["*"],"override":{"model":"hidden"}}]}),
            json!({"rules":[{"models":["*"],"override":{"temperature":9}}]}),
            json!({"rules":[{"models":["*"],"default":{"temperature":0.1},"filter":["temperature"]}]}),
            json!({"headers":{"authorization":"header-tag"}}),
            json!({"headers":{"bad\r\nname":"header-tag"}}),
        ] {
            admin(
                &client,
                &a,
                reqwest::Method::PATCH,
                &ep,
                "rules-next",
                1,
                endpoint_body(&f, &invalid),
                400,
            )
            .await?;
        }
        // Real concurrent HTTP edits use one revision: exactly one wins, one conflicts.
        let mutation = || {
            client
                .patch(format!("{a}{ep}"))
                .header("x-management-key", ADMIN)
                .header("x-config-version", "rules-next")
                .header("if-match", "rev-1")
                .header("content-type", "application/json")
                .body(endpoint_body(&f, &policy).to_string())
                .send()
        };
        let (left, right) = tokio::join!(mutation(), mutation());
        let mut codes = [left?.status().as_u16(), right?.status().as_u16()];
        codes.sort_unstable();
        assert_eq!(codes, [200, 409]);
        // Independently verify persisted revision instead of trusting task arrival order.
        let mut repo = SqliteControlPlaneRepository::open(&f.database)?;
        let next = repo
            .load_configuration(&ConfigVersionId::try_new("rules-next")?)?
            .ok_or("next")?;
        assert_eq!(next.version.revision, 2);
        assert_eq!(
            next.endpoints[0].request_policy,
            serde_json::from_value::<gateway_core::request_policy::RequestPolicy>(policy.clone())?
        );
        admin(
            &client,
            &a,
            reqwest::Method::POST,
            "/admin/config-versions/rules-next/validate",
            "rules-next",
            2,
            Value::Null,
            200,
        )
        .await?;
        admin(
            &client,
            &a,
            reqwest::Method::POST,
            "/admin/config-versions/rules-next/publish",
            "rules-next",
            1,
            Value::Null,
            409,
        )
        .await?;
        admin(
            &client,
            &a,
            reqwest::Method::POST,
            "/admin/config-versions/rules-next/publish",
            "rules-next",
            2,
            Value::Null,
            200,
        )
        .await?;
        for source in ["responses", "chat", "messages"] {
            for streaming in [false, true] {
                let text = public(
                    &client,
                    &p,
                    &f.key,
                    source,
                    payload(source, "team/p12-test-model", streaming),
                    200,
                )
                .await?;
                assert!(!text.contains("response.failed"));
                let received = peer.received.lock().map_err(|_| "received")?;
                let wire = received.last().ok_or("wire")?;
                assert_eq!(
                    wire["model"],
                    if target == "messages" {
                        "claude-sonnet-4-5-20250929"
                    } else {
                        "gpt-5.5"
                    }
                );
                assert_eq!(
                    wire["temperature"],
                    json!(if source == "chat" { 0.3 } else { 0.2 })
                );
                assert_eq!(wire[limit], 128);
                assert!(wire.get("top_p").is_none());
                assert!(
                    *peer
                        .custom_header_receives
                        .lock()
                        .map_err(|_| "headers")?
                        .last()
                        .ok_or("header")?
                );
            }
        }
        for streaming in [false, true] {
            let mut history = vec![json!({"role":"user","content":"tool question"})];
            for round in 0..5 {
                if round == 1 || round == 3 {
                    let id = if round == 1 { "call-c" } else { "call-d" };
                    history.push(json!({"role":"assistant","content":[{"type":"tool_use","id":id,"name":"lookup","input":{}}]}));
                    history.push(json!({"role":"user","content":[{"type":"tool_result","tool_use_id":id,"content":format!("result-{id}")}]}));
                } else if round == 2 || round == 4 {
                    history.push(json!({"role":"user","content":"follow-up"}));
                }
                let body = json!({"model":"team/p12-test-model","max_tokens":1000,"stream":streaming,"messages":history,"tools":[{"name":"lookup","input_schema":{"type":"object"}}]});
                public(&client, &p, &f.key, "messages", body, 200).await?;
                let wire = peer
                    .received
                    .lock()
                    .map_err(|_| "received")?
                    .last()
                    .cloned()
                    .ok_or("wire")?;
                let text = wire.to_string();
                if round >= 1 {
                    assert!(text.contains("call-c") && text.contains("result-call-c"));
                }
                if round >= 3 {
                    assert!(text.contains("call-d") && text.contains("result-call-d"));
                }
                assert_eq!(wire[limit], 128);
            }
        }
        if target != "messages" {
            let mut named = payload("responses", "team/p12-test-model(low)", false);
            named["reasoning"] = json!({"effort":"high"});
            public(&client, &p, &f.key, "responses", named, 200).await?;
            let wire = peer
                .received
                .lock()
                .map_err(|_| "received")?
                .last()
                .cloned()
                .ok_or("wire")?;
            assert_eq!(
                if target == "chat" {
                    wire["reasoning_effort"].clone()
                } else {
                    wire["reasoning"]["effort"].clone()
                },
                "low"
            );
        }
        let sent = peer.received.lock().map_err(|_| "received")?.len();
        public(
            &client,
            &p,
            &f.denied,
            target,
            payload(target, "team/p12-test-model", false),
            404,
        )
        .await?;
        public(
            &client,
            &p,
            &f.key,
            target,
            payload(target, "missing-model", false),
            404,
        )
        .await?;
        let mut orphan = payload("responses", "p12-test-model", false);
        orphan["input"] = json!([{"type":"function_call_output","call_id":"orphan","output":"x"}]);
        public(&client, &p, &f.key, "responses", orphan, 400).await?;
        assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), sent);
        if target == "messages" {
            let mut conflict = payload("messages", "p12-test-model", false);
            conflict["max_tokens"] = json!(4096);
            conflict["thinking"] = json!({"type":"enabled","budget_tokens":1024});
            public(&client, &p, &f.key, "messages", conflict, 400).await?;
            assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), sent);
        }
        if target == "chat" {
            let mut collision = payload("chat", "p12-test-model", false);
            collision
                .as_object_mut()
                .ok_or("object")?
                .remove("max_completion_tokens");
            collision["max_tokens"] = json!(1000);
            public(&client, &p, &f.key, "chat", collision, 400).await?;
            assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), sent);
        }
        if target == "responses" {
            // Gateway-owned storage retains the original owned history under configured rules.
            let mut stored = payload("responses", "p12-test-model", false);
            stored["store"] = json!(true);
            let response: Value = serde_json::from_str(
                &public(&client, &p, &f.key, "responses", stored, 200).await?,
            )?;
            let id = response["id"].as_str().ok_or("response id")?;
            let before = peer.received.lock().map_err(|_| "received")?.len();
            let foreign = client
                .get(format!("{p}/v1/responses/{id}"))
                .bearer_auth(&f.foreign)
                .send()
                .await?;
            assert_eq!(foreign.status(), 404);
            let mut follow = payload("responses", "p12-test-model", false);
            follow["previous_response_id"] = json!(id);
            follow["input"] = json!("followup");
            public(&client, &p, &f.foreign, "responses", follow.clone(), 404).await?;
            assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), before);
            public(&client, &p, &f.key, "responses", follow, 200).await?;
            let wire = peer
                .received
                .lock()
                .map_err(|_| "received")?
                .last()
                .cloned()
                .ok_or("wire")?;
            let text = wire["input"].to_string();
            assert!(
                text.contains("question") && text.contains("answer") && text.contains("followup")
            );
        }
        // Existing config fork must reseal a nonempty header Secret for the new version AAD.
        admin(
            &client,
            &a,
            reqwest::Method::POST,
            "/admin/config-versions/rules-next/fork",
            "rules-next",
            2,
            json!({"id":"rules-copy","description":"copy policy and reseal header"}),
            201,
        )
        .await?;
        let copied = SqliteControlPlaneRepository::open(&f.database)?
            .load_configuration(&ConfigVersionId::try_new("rules-copy")?)?
            .ok_or("copied config")?;
        assert_eq!(
            copied.endpoints[0].request_policy,
            next.endpoints[0].request_policy
        );
        let copied_headers = gateway_control::request_policy::prepare_headers(
            &copied,
            &copied.endpoints[0],
            &test_secret_store()?,
        )?;
        assert!(
            copied_headers
                .get("x-request-tag")
                .is_some_and(|v| v.as_bytes() == b"synthetic-request-tag")
        );
        // #75 v2 backup carries the policy and original header ciphertext/AAD unchanged.
        let backup_root = f.database.parent().ok_or("parent")?;
        let backup = backup_root.join("policy.backup");
        let restored = backup_root.join("restored.sqlite3");
        let source = gateway_store::open(&f.database)?;
        let backup_key = gateway_store::backup::BackupKey::try_from_bytes([0x71; 32])?;
        gateway_store::backup::create_encrypted_backup_file(
            &source,
            backup_root,
            &backup,
            &backup_key,
        )?;
        gateway_store::backup::restore_encrypted_backup_reader_to_empty_target(
            &mut std::fs::File::open(&backup)?,
            &restored,
            &backup_key,
        )?;
        let restored = SqliteControlPlaneRepository::open(&restored)?
            .load_configuration(&ConfigVersionId::try_new("rules-next")?)?
            .ok_or("restored config")?;
        assert_eq!(
            restored.endpoints[0].request_policy,
            next.endpoints[0].request_policy
        );
        let headers = gateway_control::request_policy::prepare_headers(
            &restored,
            &restored.endpoints[0],
            &test_secret_store()?,
        )?;
        assert!(
            headers
                .get("x-request-tag")
                .is_some_and(|v| v.as_bytes() == b"synthetic-request-tag")
        );
        assert!(!format!("{headers:?}").contains("synthetic-request-tag"));
        // Exact model and protocol scope: another admitted public model keeps its own input.
        public(
            &client,
            &p,
            &f.foreign,
            target,
            payload(target, "unmatched", false),
            200,
        )
        .await?;
        let wire = peer
            .received
            .lock()
            .map_err(|_| "received")?
            .last()
            .cloned()
            .ok_or("wire")?;
        assert!(wire.get("temperature").is_none());
        assert_eq!(wire[limit], 1000);
        // Header-only secret never enters ordinary config or either audit surface.
        admin(
            &client,
            &a,
            reqwest::Method::GET,
            &ep,
            "rules-next",
            2,
            Value::Null,
            200,
        )
        .await?;
        admin(
            &client,
            &a,
            reqwest::Method::GET,
            "/admin/audit-events",
            "rules-next",
            2,
            Value::Null,
            200,
        )
        .await?;
        admin(
            &client,
            &a,
            reqwest::Method::POST,
            "/admin/config-versions/rollback",
            "rules-next",
            2,
            Value::Null,
            200,
        )
        .await?;
        public(
            &client,
            &p,
            &f.key,
            target,
            payload(target, "p12-test-model", false),
            200,
        )
        .await?;
        let wire = peer
            .received
            .lock()
            .map_err(|_| "received")?
            .last()
            .cloned()
            .ok_or("wire")?;
        assert!(wire.get("temperature").is_none());
        assert_eq!(wire[limit], 1000);
        assert!(
            !*peer
                .custom_header_receives
                .lock()
                .map_err(|_| "headers")?
                .last()
                .ok_or("header")?
        );
        let receives = peer.received.lock().map_err(|_| "received")?.len();
        assert_eq!(
            receives,
            match target {
                "responses" => 22,
                "chat" => 20,
                _ => 19,
            }
        );
        eprintln!(
            "#71 target={target}: receives={receives}; actual management/public sockets; JSON/SSE; rollback; tool/history negatives zero-send"
        );
        ph.stop(false).await;
        ah.stop(false).await;
        peer.server.stop(false).await;
    }
    Ok(())
}

#[actix_web::test]
async fn request_rules_header_references_and_scope_fail_closed() -> TestResult {
    use gateway_control::request_policy::{prepare_headers, validate};
    for case in [
        "valid",
        "missing",
        "foreign-upstream",
        "account-kind",
        "inactive",
        "serving-binding",
        "foreign-aad",
        "newline",
        "oversize",
        "aggregate",
    ] {
        let secrets = test_secret_store()?;
        let mut config = p12_configuration_for(&secrets, "policy-validation")?;
        let id = CredentialId::try_new("header-value")?;
        let owner = config.endpoints[0].upstream_id.clone();
        let value = match case {
            "newline" => b"a\r\nb".to_vec(),
            "oversize" => vec![b'x'; 4097],
            "aggregate" => vec![b'x'; 4096],
            _ => b"synthetic-request-tag".to_vec(),
        };
        let aad_version = if case == "foreign-aad" {
            ConfigVersionId::try_new("foreign")?
        } else {
            config.version.id.clone()
        };
        config
            .credentials
            .push(gateway_store::control_plane::CredentialConfiguration {
                id: id.clone(),
                upstream_id: if case == "foreign-upstream" {
                    gateway_core::UpstreamId::try_new("foreign")?
                } else {
                    owner.clone()
                },
                kind: if case == "account-kind" {
                    "bearer"
                } else {
                    "upstream_header"
                }
                .into(),
                encrypted_secret: secrets.seal(
                    &value,
                    &credential_associated_data(&aad_version, &id, &owner)?,
                )?,
                status: if case == "inactive" {
                    CredentialStatus::Disabled
                } else {
                    CredentialStatus::Active
                },
                revision: 0,
            });
        let reference = if case == "missing" {
            "missing"
        } else {
            "header-value"
        };
        config.endpoints[0].request_policy =
            serde_json::from_value(json!({"headers":{"x-request-tag":reference}}))?;
        if case == "aggregate" {
            for i in 0..5 {
                config.endpoints[0]
                    .request_policy
                    .headers
                    .insert(format!("x-value-{i}"), reference.into());
            }
        }
        if case == "serving-binding" {
            config.endpoint_credential_bindings[0].credential_id = id;
        }
        let result = prepare_headers(&config, &config.endpoints[0], &secrets);
        assert_eq!(result.is_ok(), case == "valid", "{case}");
        if matches!(
            case,
            "missing" | "foreign-upstream" | "account-kind" | "inactive" | "serving-binding"
        ) {
            assert!(validate(&config).is_err(), "{case}");
        }
    }
    let secrets = test_secret_store()?;
    let mut config = p12_configuration_for(&secrets, "scope-validation")?;
    config.endpoints[0].request_policy = serde_json::from_value(
        json!({"rules":[{"models":["not-a-public-model"],"override":{"temperature":0.2}}]}),
    )?;
    assert!(validate(&config).is_err());
    config.endpoints[0].request_policy.rules[0].models = vec!["p12-test-model".into()];
    validate(&config)?;
    config.route_candidates[0].transform_mode = TransformMode::LosslessBridge;
    assert!(validate(&config).is_err());
    for adapter in [
        "grok.build.responses",
        "grok.console.responses",
        "grok.web.responses",
        "grok.official.responses",
        "kiro.messages",
    ] {
        assert!(
            config.endpoints[0]
                .request_policy
                .validate(adapter, &config.endpoints[0].api_format)
                .is_err()
        );
    }
    Ok(())
}
