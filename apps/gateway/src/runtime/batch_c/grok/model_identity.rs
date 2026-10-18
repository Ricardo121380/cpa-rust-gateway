//! #60 uses only synthetic identities, the production factory, and an independent HTTP receiver.
use super::*;
use provider_grok::{
    GrokBuildAliasAuthority, GrokBuildModelAliasAssertion, GrokBuildModelAliasProof,
    GrokBuildModelAliasScope,
};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::sync::atomic::AtomicBool;

pub(super) struct ModelPeer {
    url: String,
    pub(super) requests: Arc<Mutex<Vec<ModelRequest>>>,
    stopping: Arc<AtomicBool>,
    pub(super) delivered: Arc<AtomicUsize>,
    pub(super) reads_after_body: Arc<AtomicUsize>,
    thread: Option<std::thread::JoinHandle<()>>,
}

pub(super) struct ModelRequest {
    pub(super) body: Value,
    pub(super) model_override: String,
}

impl ModelPeer {
    pub(super) fn start(streaming: bool, response: Value) -> Result<Self, Box<dyn Error>> {
        let bytes = if streaming {
            let completed = response.clone();
            let mut created = response;
            created["status"] = json!("in_progress");
            created
                .as_object_mut()
                .ok_or("response object")?
                .remove("usage");
            format!(
                "event: response.created\ndata: {}\n\nevent: response.completed\ndata: {}\n\n",
                json!({"type":"response.created","response":created}),
                json!({"type":"response.completed","response":completed})
            )
        } else {
            response.to_string()
        };
        Self::start_wire(streaming, bytes)
    }

    pub(super) fn start_wire(streaming: bool, bytes: String) -> Result<Self, Box<dyn Error>> {
        Self::start_sequence(streaming, vec![bytes])
    }

    pub(super) fn start_sequence(
        streaming: bool,
        responses: Vec<String>,
    ) -> Result<Self, Box<dyn Error>> {
        if responses.is_empty() {
            return Err("synthetic response sequence is empty".into());
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let url = format!("http://{}/responses", listener.local_addr()?);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let received = requests.clone();
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = stopping.clone();
        let thread = std::thread::spawn(move || {
            let mut request_index = 0;
            while !stop.load(Ordering::Acquire) {
                let (mut socket, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(_) => break,
                };
                let _ = socket.set_nonblocking(false);
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut input = Vec::new();
                let mut buffer = [0; 4096];
                while let Ok(n) = socket.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    input.extend_from_slice(&buffer[..n]);
                    if let Some(end) = input.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&input[..end]).to_ascii_lowercase();
                        let length = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length: "))
                            .and_then(|value| value.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if input.len() >= end + 4 + length {
                            if let Ok(mut records) = received.lock() {
                                records.push(ModelRequest {
                                    body: serde_json::from_slice(&input[end + 4..end + 4 + length])
                                        .unwrap_or_else(|_| json!({"invalid_synthetic_request":true,"length":length})),
                                    model_override: headers.lines().find_map(|line| line.strip_prefix("x-grok-model-override: ")).unwrap_or("").trim().to_owned(),
                                });
                            }
                            break;
                        }
                    }
                }
                let bytes = &responses[request_index.min(responses.len() - 1)];
                request_index += 1;
                let kind = if streaming {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                let _ = write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{bytes}",
                    bytes.len()
                );
            }
        });
        Ok(Self {
            url,
            requests,
            stopping,
            delivered: Arc::new(AtomicUsize::new(0)),
            reads_after_body: Arc::new(AtomicUsize::new(0)),
            thread: Some(thread),
        })
    }
}

struct ModelBody {
    response: reqwest::Response,
    pending: Vec<u8>,
    streaming: bool,
    remaining: u64,
    delivered: Arc<AtomicUsize>,
    reads_after_body: Arc<AtomicUsize>,
}

impl GrokBuildResponseBody for ModelBody {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            loop {
                if self.streaming
                    && let Some(end) = self.pending.windows(2).position(|w| w == b"\n\n")
                {
                    self.delivered.fetch_add(1, Ordering::SeqCst);
                    return Ok(Some(self.pending.drain(..end + 2).collect()));
                }
                if self.remaining == 0 {
                    self.reads_after_body.fetch_add(1, Ordering::SeqCst);
                }
                match self.response.chunk().await.map_err(|_| internal_error())? {
                    Some(chunk) => {
                        self.remaining = self.remaining.saturating_sub(
                            u64::try_from(chunk.len()).map_err(|_| internal_error())?,
                        );
                        if !self.streaming {
                            return Ok(Some(chunk.to_vec()));
                        }
                        self.pending.extend_from_slice(&chunk);
                    }
                    None if self.pending.is_empty() => return Ok(None),
                    None => return Ok(Some(std::mem::take(&mut self.pending))),
                }
            }
        })
    }
}

impl Drop for ModelPeer {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl GrokBuildTransport for ModelPeer {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            // Preserve production request assembly; substitute only the owned socket boundary.
            let response = reqwest::Client::builder()
                .no_proxy()
                .build()
                .map_err(|_| internal_error())?
                .post(&self.url)
                .header(
                    provider_grok::GROK_BUILD_MODEL_OVERRIDE_HEADER,
                    request
                        .header(provider_grok::GROK_BUILD_MODEL_OVERRIDE_HEADER)
                        .ok_or_else(internal_error)?,
                )
                .body(request.body().to_vec())
                .send()
                .await
                .map_err(|_| internal_error())?;
            let streaming = response
                .headers()
                .get("content-type")
                .is_some_and(|v| v == "text/event-stream");
            let remaining = response.content_length().ok_or_else(internal_error)?;
            Ok(GrokBuildTransportResponse::new(
                200,
                if streaming {
                    GrokBuildResponseContentType::EventStream
                } else {
                    GrokBuildResponseContentType::Json
                },
                provider_grok::GrokBuildResponseContentEncoding::Identity,
                Box::new(ModelBody {
                    response,
                    pending: Vec::new(),
                    streaming,
                    remaining,
                    delivered: self.delivered.clone(),
                    reads_after_body: self.reads_after_body.clone(),
                }),
            ))
        })
    }
}

#[actix_web::test]
async fn public_build_model_identity_rejects_native_mismatch_without_resend() -> TestResult {
    let mut outcomes = Vec::new();
    for streaming in [false, true] {
        let peer = Arc::new(ModelPeer::start(
            streaming,
            json!({
                "id":"synthetic-model-response", "model":"SYNTHETIC_WRONG_NATIVE_MODEL",
                "object":"response", "status":"completed", "output":[]
            }),
        )?);
        let (mut channel, secret) = native_channels()[0];
        channel.model = "p12-test-model";
        let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.build = Some(peer.clone());
            endpoint.transports = Arc::new(profiles);
            Ok(())
        })?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(
                    json!({"model":"p12-test-model", "input":"synthetic #60", "stream":streaming}),
                )
                .to_request(),
        )
        .await;
        let status = response.status();
        let wire = String::from_utf8(test::read_body(response).await.to_vec())?;
        let requests = peer.requests.lock().map_err(|_| "receiver")?;
        assert_eq!(
            requests.len(),
            1,
            "independent receiver must see exactly one inference"
        );
        assert_eq!(requests[0].body["model"], "p12-test-model");
        assert_eq!(requests[0].model_override, "p12-test-model");
        println!("synthetic native mismatch stream={streaming} status={status} wire={wire}");
        outcomes.push(
            wire.contains("model identity")
                && !wire.contains("response.completed")
                && !wire.contains("SYNTHETIC_WRONG_NATIVE_MODEL"),
        );
    }
    assert_eq!(
        outcomes,
        vec![true, true],
        "both public modes must reject native identity mismatch"
    );
    Ok(())
}

#[actix_web::test]
async fn public_build_model_identity_rejects_unproven_request_before_send() -> TestResult {
    for streaming in [false, true] {
        let peer = Arc::new(ModelPeer::start(
            streaming,
            json!({
                "id":"synthetic-model-response", "model":"grok-4.5",
                "object":"response", "status":"completed", "output":[]
            }),
        )?);
        let (channel, secret) = native_channels()[0];
        let fixture = fixture_with_configuration(
            channel,
            secret.as_bytes(),
            false,
            |_| Ok(()),
            |endpoints| {
                let endpoint = endpoints.values_mut().next().ok_or("endpoint")?;
                let mut profiles = P12TransportProfiles::try_new()?;
                profiles.build = Some(peer.clone());
                endpoint.transports = Arc::new(profiles);
                Ok(())
            },
        )?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(
                    json!({"model":"p12-test-model", "input":"synthetic #60", "stream":streaming}),
                )
                .to_request(),
        )
        .await;
        let status = response.status();
        let wire = String::from_utf8(test::read_body(response).await.to_vec())?;
        let count = peer.requests.lock().map_err(|_| "receiver")?.len();
        println!(
            "synthetic request mismatch stream={streaming} status={status} receives={count} wire={wire}"
        );
        assert_eq!(
            count, 0,
            "known public/upstream incompatibility must stop before sending"
        );
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            wire.contains("model identity rejected before dispatch"),
            "{wire}"
        );
    }
    Ok(())
}

fn synthetic_scope(
    executor: &P12RoutedResponsesExecutor,
    configuration: &ControlPlaneConfiguration,
    streaming: bool,
) -> Result<GrokBuildModelAliasScope, Box<dyn Error>> {
    let snapshot = executor.scheduler.snapshot();
    let route = snapshot
        .route(&configuration.model_routes[0].id)
        .ok_or("route")?;
    let candidate = route.candidates().first().ok_or("candidate")?;
    let (_, credential) = executor
        .scheduler
        .diagnostic_bindings()
        .find(|(endpoint, _)| endpoint == candidate.endpoint_id())
        .ok_or("credential binding")?;
    Ok(GrokBuildModelAliasScope {
        lineage: ResponsesExecutionLineage::new(
            snapshot.version().clone(),
            ProviderId::try_new(candidate.upstream_id().as_str())?,
            candidate.upstream_id().clone(),
            candidate.endpoint_id().clone(),
            configuration.model_routes[0].id.clone(),
            candidate.id().clone(),
            credential.credential_id().clone(),
            credential.credential_revision(),
        ),
        config_revision: configuration.version.revision,
        channel: "grok.build".into(),
        client_key_id: Some(configuration.client_keys[0].id().clone()),
        client_protocol: ProtocolFormat::OpenAiResponses,
        mode: if streaming {
            provider_grok::GrokBuildExecutionMode::Streaming
        } else {
            provider_grok::GrokBuildExecutionMode::NonStreaming
        },
        client_profile: provider_grok::GROK_BUILD_CLIENT_VERSION.into(),
    })
}

fn synthetic_proof(
    scope: GrokBuildModelAliasScope,
    native: &str,
    from: i64,
    until: i64,
) -> Result<GrokBuildModelAliasProof, Box<dyn Error>> {
    // This deliberately synthetic authority has no applicability to any real account or epoch.
    let source =
        format!("SYNTHETIC #60 approved verification: p12-test-model -> grok-4.5 -> {native}");
    Ok(GrokBuildModelAliasProof::freeze(
        scope,
        GrokBuildModelAliasAssertion {
            public_model: "p12-test-model".into(),
            upstream_model: "grok-4.5".into(),
            native_model: native.into(),
            authority: GrokBuildAliasAuthority::ApprovedVerification,
            source_reference: "synthetic:#60/approved-before-request".into(),
            source_sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
            valid_from_ms: from,
            valid_until_ms: until,
        },
        source.as_bytes(),
    )?)
}

#[allow(
    clippy::too_many_lines,
    reason = "one production fixture freezes the independent scope axes of the public rejection matrix"
)]
fn identity_fixture(
    peer: &Arc<ModelPeer>,
    streaming: bool,
    alias: bool,
    scope_fault: Option<&str>,
) -> Result<Fixture, Box<dyn Error>> {
    let (mut channel, secret) = native_channels()[0];
    if !alias {
        channel.model = "p12-test-model";
    }
    fixture_with_model_alias_proofs(
        channel,
        secret.as_bytes(),
        false,
        |configuration| {
            configuration.model_routes[0].max_attempts = 3;
            let mut fallback = configuration.route_candidates[0].clone();
            fallback.id = gateway_core::RouteCandidateId::try_new("synthetic-fallback-candidate")?;
            fallback.upstream_model = "synthetic-fallback-model".into();
            fallback.priority = 1;
            configuration.route_candidates.push(fallback);
            configuration.model_aliases.push(
                gateway_store::control_plane::ModelAliasConfiguration {
                    alias: "synthetic-request-alias".into(),
                    public_model_id: configuration.public_models[0].id.clone(),
                },
            );
            Ok(())
        },
        |endpoints| {
            let endpoint = endpoints.values_mut().next().ok_or("endpoint")?;
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.build = Some(peer.clone());
            endpoint.transports = Arc::new(profiles);
            Ok(())
        },
        |executor, configuration| {
            if !alias {
                return Ok(Arc::from([]));
            }
            let mut scope = synthetic_scope(executor, configuration, streaming)?;
            let now = system_now_ms_runtime()?;
            let (mut from, mut until) = (now - 1000, now + 60_000);
            match scope_fault {
                Some("config_revision") => scope.config_revision += 1,
                Some("channel") => scope.channel = "grok.console".into(),
                Some("owner") => {
                    scope.client_key_id = Some(ClientKeyId::try_new("synthetic-foreign-owner")?);
                }
                Some("protocol") => scope.client_protocol = ProtocolFormat::AnthropicMessages,
                Some("mode") => {
                    scope.mode = if streaming {
                        provider_grok::GrokBuildExecutionMode::NonStreaming
                    } else {
                        provider_grok::GrokBuildExecutionMode::Streaming
                    }
                }
                Some("profile") => scope.client_profile = "synthetic-old-profile".into(),
                Some("expired") => until = now - 1,
                Some("future") => {
                    from = now + 30_000;
                    until = now + 60_000;
                }
                Some(
                    field @ ("version"
                    | "provider"
                    | "upstream"
                    | "endpoint"
                    | "route"
                    | "candidate"
                    | "credential"
                    | "credential_revision"),
                ) => {
                    let old = &scope.lineage;
                    scope.lineage = ResponsesExecutionLineage::new(
                        if field == "version" {
                            SnapshotVersion::try_new("synthetic-other-version")?
                        } else {
                            old.snapshot_version().clone()
                        },
                        if field == "provider" {
                            ProviderId::try_new("synthetic-other-provider")?
                        } else {
                            old.provider_id().clone()
                        },
                        if field == "upstream" {
                            gateway_core::UpstreamId::try_new("synthetic-other-upstream")?
                        } else {
                            old.upstream_id().clone()
                        },
                        if field == "endpoint" {
                            EndpointId::try_new("synthetic-other-endpoint")?
                        } else {
                            old.channel_id().clone()
                        },
                        if field == "route" {
                            RouteId::try_new("synthetic-other-route")?
                        } else {
                            old.route_id().clone()
                        },
                        if field == "candidate" {
                            gateway_core::RouteCandidateId::try_new("synthetic-other-candidate")?
                        } else {
                            old.route_candidate_id().clone()
                        },
                        if field == "credential" {
                            CredentialId::try_new("synthetic-other-credential")?
                        } else {
                            old.credential_id().clone()
                        },
                        old.credential_revision() + u64::from(field == "credential_revision"),
                    );
                }
                _ => {}
            }
            let mut proofs = vec![synthetic_proof(
                scope.clone(),
                "synthetic-native-alias",
                from,
                until,
            )?];
            if scope_fault == Some("conflicting") {
                proofs.push(synthetic_proof(
                    scope,
                    "synthetic-other-native-alias",
                    from,
                    until,
                )?);
            }
            Ok(proofs.into())
        },
    )
}

async fn public_request(
    fixture: &Fixture,
    requested: &str,
    streaming: bool,
) -> Result<(StatusCode, String), Box<dyn Error>> {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(json!({"model":requested,"input":"synthetic #60","stream":streaming}))
            .to_request(),
    )
    .await;
    assert!(response.headers().get("x-request-id").is_some());
    Ok((
        response.status(),
        String::from_utf8(test::read_body(response).await.to_vec())?,
    ))
}

fn response(model: Value) -> Value {
    let mut response = json!({"id":"synthetic-model-response","object":"response","status":"completed","output":[]});
    response["model"] = model;
    response
}

#[actix_web::test]
async fn public_build_model_identity_literal_alias_and_exact_request_lineage() -> TestResult {
    for streaming in [false, true] {
        for alias in [false, true] {
            let native = if alias {
                "synthetic-native-alias"
            } else {
                "p12-test-model"
            };
            let peer = Arc::new(ModelPeer::start(streaming, response(json!(native)))?);
            let fixture = identity_fixture(&peer, streaming, alias, None)?;
            let (status, wire) =
                public_request(&fixture, "synthetic-request-alias", streaming).await?;
            assert_eq!(status, StatusCode::OK, "{wire}");
            let public: Value = if streaming {
                wire.lines()
                    .filter_map(|line| line.strip_prefix("data: "))
                    .filter_map(|data| serde_json::from_str::<Value>(data).ok())
                    .find(|value| value["type"] == "response.completed")
                    .ok_or("missing completed")?["response"]
                    .clone()
            } else {
                serde_json::from_str(&wire)?
            };
            assert_eq!(public["model"], "p12-test-model");
            assert_eq!(public["status"], "completed");
            let records = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(records.len(), 1);
            let expected_upstream = if alias { "grok-4.5" } else { "p12-test-model" };
            assert_eq!(records[0].body["model"], expected_upstream);
            assert_eq!(records[0].model_override, expected_upstream);
            let events = fixture.events.0.lock().map_err(|_| "events")?;
            let request = events
                .iter()
                .find_map(|event| {
                    if let GatewayEvent::Request(request) = event {
                        Some(request)
                    } else {
                        None
                    }
                })
                .ok_or("request")?;
            assert_eq!(request.requested_model(), "synthetic-request-alias");
            assert_eq!(request.public_model(), "p12-test-model");
            let attempts = events
                .iter()
                .filter_map(|event| {
                    if let GatewayEvent::Attempt(attempt) = event {
                        Some(attempt)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(attempts.len(), 1);
            assert_eq!(attempts[0].request_id(), request.request_id());
            assert_eq!(attempts[0].upstream_model(), expected_upstream);
            let actual = attempts[0]
                .execution_identity()
                .ok_or("actual execution identity")?;
            assert_eq!(actual.channel, "grok.build");
            assert_eq!(
                actual.config_version_id,
                fixture.executor.snapshot_version.as_str()
            );
            assert_eq!(
                actual.config_revision,
                fixture.executor.config_revision.as_i64()
            );
            println!(
                "SYNTHETIC literal={:?} stream={streaming}: public=p12-test-model upstream={expected_upstream} native={native}; receives=1; exact lineage retained",
                !alias
            );
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_model_identity_rejects_foreign_expired_and_conflicting_alias_proofs()
-> TestResult {
    for streaming in [false, true] {
        for fault in [
            "config_revision",
            "version",
            "provider",
            "upstream",
            "endpoint",
            "route",
            "candidate",
            "credential",
            "credential_revision",
            "channel",
            "owner",
            "protocol",
            "mode",
            "profile",
            "expired",
            "future",
            "conflicting",
        ] {
            let peer = Arc::new(ModelPeer::start(
                streaming,
                response(json!("synthetic-native-alias")),
            )?);
            let fixture = identity_fixture(&peer, streaming, true, Some(fault))?;
            let (status, wire) = public_request(&fixture, "p12-test-model", streaming).await?;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{fault}: {wire}");
            assert!(
                wire.contains("alias proof is not applicable"),
                "{fault}: {wire}"
            );
            assert_eq!(
                peer.requests.lock().map_err(|_| "receiver")?.len(),
                0,
                "{fault}"
            );
            println!("SYNTHETIC inapplicable alias {fault} stream={streaming}: receives=0");
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_model_identity_missing_invalid_and_phase_conflicts_are_distinct() -> TestResult
{
    for streaming in [false, true] {
        for (value, reason) in [
            (None, "native model is missing"),
            (Some(Value::Null), "invalid shape"),
            (Some(json!(42)), "invalid shape"),
            (Some(json!({"name":"p12-test-model"})), "invalid shape"),
            (Some(json!("")), "invalid shape"),
            (Some(json!(" p12-test-model")), "invalid shape"),
            (
                Some(json!("synthetic-wrong-native")),
                "native and upstream models are unproven",
            ),
        ] {
            let mut native = response(json!("p12-test-model"));
            if let Some(value) = value {
                native["model"] = value;
            } else {
                native.as_object_mut().ok_or("response")?.remove("model");
            }
            let peer = Arc::new(ModelPeer::start(streaming, native)?);
            let fixture = identity_fixture(&peer, streaming, false, None)?;
            let (status, wire) = public_request(&fixture, "p12-test-model", streaming).await?;
            assert_eq!(status, StatusCode::BAD_GATEWAY, "{wire}");
            assert!(
                wire.contains(reason) && !wire.contains("response.completed"),
                "{wire}"
            );
            assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
            assert_eq!(fixture.events.attempts()?, 1);
        }
    }
    for phase in [
        "response.in_progress",
        "response.completed",
        "response.incomplete",
        "response.failed",
    ] {
        let created = json!({"type":"response.created","response":{"id":"synthetic-model-response","model":"p12-test-model","status":"in_progress","output":[]}});
        let mut changed = response(json!("synthetic-phase-conflict"));
        changed["status"] = json!(phase.strip_prefix("response.").ok_or("phase")?);
        changed["incomplete_details"] = json!({"reason":"max_output_tokens"});
        let event = json!({"type":phase,"response":changed});
        let peer = Arc::new(ModelPeer::start_wire(
            true,
            format!(
                "event: response.created\ndata: {created}\n\nevent: {phase}\ndata: {event}\n\nevent: unread\ndata: malformed\n\n"
            ),
        )?);
        let fixture = identity_fixture(&peer, true, false, None)?;
        let (status, wire) = public_request(&fixture, "p12-test-model", true).await?;
        assert_eq!(status, StatusCode::OK, "{wire}");
        assert!(wire.contains("native response phases contradict"), "{wire}");
        assert_eq!(wire.matches("event: response.failed").count(), 1, "{wire}");
        assert!(
            !wire.contains("response.completed") && !wire.contains("synthetic-phase-conflict"),
            "{wire}"
        );
        assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
        assert_eq!(peer.delivered.load(Ordering::SeqCst), 2);
        assert_eq!(peer.reads_after_body.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.events.attempts()?, 1);
        println!(
            "SYNTHETIC {phase} identity conflict: receives=1; unique stream failure; unread tail remains unread"
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_model_identity_failure_precedes_unvalidated_usage() -> TestResult {
    for streaming in [false, true] {
        // Preserve the old #59 synthetic omission as an identity negative, with its original
        // Usage bytes. Those bytes do not establish an identity-admitted response or a bill.
        let native = format!(
            r#"{{"id":"synthetic-model-response","object":"response","status":"completed","output":[],"usage":{}}}"#,
            super::usage_evidence::RAW_USAGE
        );
        let wire = if streaming {
            format!(
                "event: response.created\ndata: {{\"type\":\"response.created\",\"response\":{{\"id\":\"synthetic-model-response\",\"status\":\"in_progress\",\"output\":[]}}}}\n\nevent: response.completed\ndata: {{\"type\":\"response.completed\",\"response\":{native}}}\n\n"
            )
        } else {
            native
        };
        let peer = Arc::new(ModelPeer::start_wire(streaming, wire)?);
        let fixture = identity_fixture(&peer, streaming, false, None)?;
        let (status, wire) = public_request(&fixture, "p12-test-model", streaming).await?;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert!(wire.contains("native model is missing"), "{wire}");
        assert!(!wire.contains("response projection") && !wire.contains("PRIVATE_USAGE_59"));
        assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
        let events = fixture.events.0.lock().map_err(|_| "events")?;
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, GatewayEvent::Usage(_)))
        );
        if streaming {
            assert_eq!(peer.delivered.load(Ordering::SeqCst), 1);
            assert_eq!(peer.reads_after_body.load(Ordering::SeqCst), 0);
        }
    }
    Ok(())
}
