//! Batch C public HTTP tests use the production compiler, factory, auth and leased executor.
//! Only native transport interfaces are replaced; fixed production URLs remain unchanged.
use super::tests::{TemporaryDirectory, p12_configuration_for, test_secret_store};
use super::*;
use actix_web::{App, http::StatusCode, test, web};
use gateway_auth::client_key::ClientKeyPepper;
use gateway_control::management_service::{ManagementActor, ManagementService};
use gateway_router::SnapshotClientKeyAuthenticator;
use gateway_store::control_plane::{StoredClientKey, StoredClientKeyStatus};
use provider_grok::{
    GrokOfficialRateLimitMetadata, GrokOfficialResponseBody, GrokOfficialResponseContentType,
    GrokOfficialResponsesOutboundRequest, GrokOfficialTransport, GrokOfficialTransportResponse,
};
use provider_kiro::{
    failure_classification::KiroFailureSignal,
    inference::{
        KiroOutboundRequest, KiroResponseBody, KiroResponseContentType, KiroTransport,
        KiroTransportResponse,
    },
};
use serde_json::json;
use std::fmt::Write as _;
mod grok;
pub(super) mod http;
mod public_response;

type TestResult = Result<(), Box<dyn Error>>;

#[derive(Clone, Copy)]
struct Channel {
    adapter: &'static str,
    format: &'static str,
    base: &'static str,
    path: &'static str,
    model: &'static str,
}
struct Fixture {
    state: ResponsesHttpState,
    executor: Arc<P12RoutedResponsesExecutor>,
    events: Arc<Events>,
    key: String,
    _directory: TemporaryDirectory,
}
#[derive(Default)]
struct Events(Mutex<Vec<GatewayEvent>>);
impl GatewayEventSink for Events {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        if let Ok(mut events) = self.0.lock() {
            events.push(event);
        }
        EventEmission::Enqueued
    }
}
impl Events {
    fn attempts(&self) -> Result<usize, Box<dyn Error>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| "events")?
            .iter()
            .filter(|event| matches!(event, GatewayEvent::Attempt(_)))
            .count())
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "production bootstrap and native enrollment share one isolated fixture transaction"
)]
fn fixture(
    channel: Channel,
    secret: &[u8],
    configure: impl FnOnce(&mut EndpointRuntime) -> Result<(), Box<dyn Error>>,
) -> Result<Fixture, Box<dyn Error>> {
    let directory = TemporaryDirectory::new()?;
    let database = directory.path().join("batch-c.sqlite3");
    let secrets = test_secret_store()?;
    let mut configuration = p12_configuration_for(&secrets, "batch-c")?;
    let endpoint = &mut configuration.endpoints[0];
    endpoint.adapter_id = channel.adapter.into();
    endpoint.api_format = channel.format.into();
    endpoint.base_url = channel.base.into();
    endpoint.inference_path = channel.path.into();
    let host = reqwest::Url::parse(channel.base)?
        .host_str()
        .ok_or("host")?
        .to_owned();
    configuration.egress_policies[0].allowed_hosts_json = json!([host]).to_string();
    configuration.route_candidates[0].upstream_model = channel.model.into();
    configuration.route_candidates[0].transform_mode = TransformMode::CanonicalBridge;
    let credential = &mut configuration.credentials[0];
    credential.kind = if secret.first() == Some(&b'{') {
        "oauth_json"
    } else {
        "bearer"
    }
    .into();
    credential.encrypted_secret = secrets.seal(
        secret,
        &credential_associated_data(
            &configuration.version.id,
            &credential.id,
            &credential.upstream_id,
        )?,
    )?;
    if matches!(
        channel.adapter,
        "grok.build.responses" | "grok.console.responses" | "grok.web.responses"
    ) {
        use provider_grok::{
            GrokAccountAuthStatus, GrokAccountCredential, GrokAccountImport, GrokAccountPoolStore,
            GrokAccountProvider,
        };
        let now = system_now_ms_runtime()?;
        let (provider, credential) = if channel.adapter == "grok.build.responses" {
            let material = GrokBuildCredential::import_json(secret, now)?;
            (
                GrokAccountProvider::Build,
                GrokAccountCredential::try_from_build_credential(&material)?,
            )
        } else {
            let provider = if channel.adapter == "grok.web.responses" {
                GrokAccountProvider::Web
            } else {
                GrokAccountProvider::Console
            };
            (
                provider,
                GrokAccountCredential::try_from_sso(provider, secret, now)?,
            )
        };
        let account = GrokAccountImport {
            provider,
            identity: credential.enrollment_identity(provider, now, None)?,
            credential,
            auth_status: GrokAccountAuthStatus::Active,
            enabled: true,
            priority: 0,
            weight: 1,
            max_concurrency: 1,
            refresh_due_at_ms: None,
            quota_sync_due_at_ms: None,
            cooldown_until_ms: None,
        };
        GrokAccountPoolStore::try_open(&database, secrets.clone())?.import_batch(
            "batch-c-native",
            &[account],
            now,
        )?;
        configuration.upstreams[0].kind = match provider {
            GrokAccountProvider::Build => "grok-build-native",
            GrokAccountProvider::Console => "grok-console-native",
            GrokAccountProvider::Web => "grok-web-native",
        }
        .into();
        configuration.credentials.clear();
        configuration.endpoint_credential_bindings.clear();
    }
    let issuer = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x63; 32])?);
    let (key, presented) = issuer
        .issue(
            configuration.client_keys[0].id().clone(),
            configuration.access_groups[0].id.clone(),
            None,
        )?
        .into_parts();
    configuration.client_keys = vec![StoredClientKey::try_new(
        key.client_key_id().clone(),
        key.access_group_id().clone(),
        key.prefix().as_str(),
        key.secret_digest().as_bytes(),
        StoredClientKeyStatus::Active,
        None,
    )?];
    let mut repository = SqliteControlPlaneRepository::open(&database)?;
    repository.write_configuration(&configuration)?;
    repository.activate_version(&configuration.version.id)?;
    let lifecycle = ManagementService::bootstrap(
        SqliteControlPlaneRepository::open(&database)?,
        deployment_route_compiler(&database)?,
        ManagementActor::try_new("batch-c-test")?,
    )?;
    let stages = Arc::new(P12AttemptStageStore::new());
    let events = Arc::new(Events::default());
    let registry = Arc::clone(lifecycle.registry());
    // Deployment constructs synchronous credential refresh clients before entering Actix.
    // Keep the same factory on a scoped ordinary thread in these asynchronous HTTP tests.
    let (mut executor, _, scheduler, _, _, _) = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                P12RoutedResponsesExecutor::try_new(
                    &database,
                    &configuration,
                    &secrets,
                    registry,
                    stages,
                    events.clone(),
                    Arc::new(RuntimeHealthRegistry::new()),
                    Arc::new(RuntimeQuotaRegistry::new()),
                    Arc::new(GrokBuildCacheIdentityDeriver::new([0x64; 32])),
                    UpstreamProxy::Direct,
                    None,
                    None,
                    8191,
                    None,
                    &gateway_upstream::CredentialConcurrencyRegistry::default(),
                )
            })
            .join()
            .map_err(|_| "runtime factory thread")
    })??;
    let endpoint = Arc::get_mut(&mut executor.endpoints)
        .ok_or("shared endpoints")?
        .values_mut()
        .next()
        .ok_or("runtime endpoint")?;
    configure(endpoint)?;
    let executor = Arc::new(executor);
    let state = ResponsesHttpState::with_snapshot_metadata_and_event_sink(
        executor.clone(),
        Arc::new(SystemResponsesMetadataFactory::new()),
        Arc::new(SnapshotClientKeyAuthenticator::with_shared_verifier(
            Arc::clone(lifecycle.registry()),
            Some(scheduler),
            Arc::new(issuer),
        )),
        events.clone(),
        default_stream_capacity()?,
    );
    Ok(Fixture {
        state,
        executor,
        events,
        key: presented.as_str().to_owned(),
        _directory: directory,
    })
}

#[derive(Default)]
struct KiroPeer {
    bodies: Mutex<Vec<Value>>,
    heads: Mutex<Vec<(String, String)>>,
}
impl KiroTransport for KiroPeer {
    fn send(
        &self,
        request: KiroOutboundRequest,
    ) -> BoxFuture<'_, Result<KiroTransportResponse, GatewayError>> {
        Box::pin(async move {
            self.heads.lock().map_err(|_| internal_error())?.push((
                request.url().to_owned(),
                request
                    .header("x-amzn-codewhisperer-optout")
                    .unwrap_or_default()
                    .to_owned(),
            ));
            let body: Value =
                serde_json::from_slice(request.body()).map_err(|_| internal_error())?;
            let history = body.to_string();
            let second_call = history.contains("next-question");
            let result_round = if second_call {
                history.contains("result-d")
            } else {
                history.contains("result-c")
            };
            let call_id = if second_call { "call-d" } else { "call-c" };
            self.bodies.lock().map_err(|_| internal_error())?.push(body);
            let bytes = if result_round {
                kiro_frame("assistantResponseEvent", &json!({"content":"answer"}))?
            } else {
                kiro_frame(
                    "toolUseEvent",
                    &json!({"toolUseId":call_id,"name":"lookup","input":"{}","stop":true}),
                )?
            };
            Ok(KiroTransportResponse::new(
                200,
                KiroResponseContentType::EventStream,
                KiroFailureSignal::None,
                Box::new(Body(Some(bytes))),
            ))
        })
    }
}
struct Body(Option<Vec<u8>>);
impl KiroResponseBody for Body {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async { Ok(self.0.take()) })
    }
}
impl GrokOfficialResponseBody for Body {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async { Ok(self.0.take()) })
    }
}
fn kiro_frame(event: &str, payload: &Value) -> Result<Vec<u8>, GatewayError> {
    let mut headers = Vec::new();
    for (name, value) in [(":message-type", "event"), (":event-type", event)] {
        headers.push(u8::try_from(name.len()).map_err(|_| internal_error())?);
        headers.extend(name.as_bytes());
        headers.push(7);
        headers.extend(
            u16::try_from(value.len())
                .map_err(|_| internal_error())?
                .to_be_bytes(),
        );
        headers.extend(value.as_bytes());
    }
    let payload = serde_json::to_vec(&payload).map_err(|_| internal_error())?;
    let mut bytes = Vec::new();
    bytes.extend(
        u32::try_from(16 + headers.len() + payload.len())
            .map_err(|_| internal_error())?
            .to_be_bytes(),
    );
    bytes.extend(
        u32::try_from(headers.len())
            .map_err(|_| internal_error())?
            .to_be_bytes(),
    );
    bytes.extend(crc32(&bytes).to_be_bytes());
    bytes.extend(headers);
    bytes.extend(payload);
    bytes.extend(crc32(&bytes).to_be_bytes());
    Ok(bytes)
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}

#[actix_web::test]
async fn kiro_real_factory_bridges_text_tools_and_effort_without_dropping_limits() -> TestResult {
    for (base, path) in [
        (
            "https://q.eu-west-1.amazonaws.com",
            "/generateAssistantResponse",
        ),
        ("https://runtime.us-east-1.kiro.dev", "/"),
    ] {
        for streaming in [false, true] {
            let peer = Arc::new(KiroPeer::default());
            let secret = json!({"kind":"social","access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_at_ms":system_now_ms_runtime()?+3_600_000}).to_string();
            let fixture = fixture(
                Channel {
                    adapter: "kiro.messages",
                    format: "anthropic/messages",
                    base,
                    path,
                    model: "claude-sonnet-4.5",
                },
                secret.as_bytes(),
                |endpoint| {
                    let mut profiles = P12TransportProfiles::try_new()?;
                    profiles.kiro = Some(peer.clone());
                    endpoint.transports = Arc::new(profiles);
                    Ok(())
                },
            )?;
            let observed_profile = "arn:aws:codewhisperer:us-east-1:123456789012:profile/observed";
            if base.contains("kiro.dev") {
                fixture.executor.kiro_profiles.lock().map_err(|_| "profiles")?.insert(
                    (EndpointId::try_new("p12-krill-endpoint")?,CredentialId::try_new("p12-runtime-credential")?),
                    KiroProfileSnapshot {credential_revision:1,valid_until_ms:system_now_ms_runtime()?+60_000,
                        profile:provider_kiro::profile_arn::KiroProfileArnResolution::from_cli_catalog(provider_kiro::credential::KiroCredentialKind::Social,&KiroApiRegion::try_new("us-east-1")?,observed_profile.into())?},
                );
            }
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            for (path, rounds) in grok::tool_conversations(streaming)?.into_iter().take(2) {
                for (round, mut body) in rounds.into_iter().enumerate() {
                    let object = body.as_object_mut().ok_or("request object")?;
                    object.remove("max_output_tokens");
                    object.remove("max_tokens");
                    object.insert(
                        if path == "/v1/responses" {
                            "reasoning"
                        } else {
                            "reasoning_effort"
                        }
                        .into(),
                        if path == "/v1/responses" {
                            json!({"effort":"high"})
                        } else {
                            json!("high")
                        },
                    );
                    let response = test::call_service(
                        &app,
                        test::TestRequest::post()
                            .uri(path)
                            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                            .set_json(body)
                            .to_request(),
                    )
                    .await;
                    let status = response.status();
                    let bytes = test::read_body(response).await;
                    assert_eq!(
                        status,
                        StatusCode::OK,
                        "{path}: {} events {:?}; calls {}",
                        String::from_utf8_lossy(&bytes),
                        fixture.events.0.lock().map_err(|_| "events")?,
                        peer.bodies.lock().map_err(|_| "bodies")?.len()
                    );
                    public_response::assert_round(path, streaming, round, &bytes)?;
                }
            }
            for (path, body) in [
                (
                    "/v1/messages",
                    json!({"model":"p12-test-model","max_tokens":19,"messages":[{"role":"user","content":"question"}],"stream":streaming}),
                ),
                (
                    "/v1/responses",
                    json!({"model":"p12-test-model","max_output_tokens":19,"input":"question","stream":streaming}),
                ),
                (
                    "/v1/chat/completions",
                    json!({"model":"p12-test-model","max_tokens":19,"messages":[{"role":"user","content":"question"}],"stream":streaming}),
                ),
                (
                    "/v1/chat/completions",
                    json!({"model":"p12-test-model","max_completion_tokens":19,"messages":[{"role":"user","content":"question"}],"stream":streaming}),
                ),
            ] {
                let before = peer.bodies.lock().map_err(|_| "bodies")?.len();
                let attempts = fixture.events.attempts()?;
                let response = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(path)
                        .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                        .set_json(body)
                        .to_request(),
                )
                .await;
                assert_eq!(response.status(), StatusCode::BAD_REQUEST);
                assert_eq!(peer.bodies.lock().map_err(|_| "bodies")?.len(), before);
                assert_eq!(fixture.events.attempts()?, attempts);
            }
            let bodies = peer.bodies.lock().map_err(|_| "bodies")?;
            assert_eq!(bodies.len(), 8);
            for (index, body) in bodies.iter().enumerate() {
                if base.contains("kiro.dev") {
                    assert_eq!(body["profileArn"], observed_profile);
                }
                let input = &body["conversationState"]["currentMessage"]["userInputMessage"];
                if index % 2 == 0 {
                    assert_eq!(
                        input["content"],
                        if index % 4 == 0 {
                            "question"
                        } else {
                            "next-question"
                        }
                    );
                } else {
                    assert_eq!(
                        input["userInputMessageContext"]["toolResults"][0]["toolUseId"],
                        if index % 4 == 1 { "call-c" } else { "call-d" }
                    );
                    assert!(input.to_string().contains(if index % 4 == 1 {
                        "result-c"
                    } else {
                        "result-d"
                    }));
                }
                assert_eq!(input["modelId"], "claude-sonnet-4.5");
                let context = &input["userInputMessageContext"];
                assert_eq!(context["envState"]["currentWorkingDirectory"], "/");
                let effort = if base.contains("amazonaws") {
                    &context["additionalModelRequestFields"]["thinking"]["effort"]
                } else {
                    &context["outputConfig"]["effort"]
                };
                assert_eq!(effort, "high");
            }
            assert!(
                peer.heads
                    .lock()
                    .map_err(|_| "heads")?
                    .iter()
                    .all(|(url, _)| url == &format!("{base}{path}"))
            );
        }
    }
    Ok(())
}

struct OfficialPeer {
    bodies: Mutex<Vec<Value>>,
    exhaust_quota: bool,
}
impl GrokOfficialTransport for OfficialPeer {
    fn send(
        &self,
        request: GrokOfficialResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokOfficialTransportResponse, GatewayError>> {
        Box::pin(async move {
            if request.url() != GROK_OFFICIAL_RESPONSES_URL {
                return Err(internal_error());
            }
            let body: Value =
                serde_json::from_slice(request.body()).map_err(|_| internal_error())?;
            let streaming = body["stream"] == true;
            self.bodies.lock().map_err(|_| internal_error())?.push(body);
            let response = json!({"id":"response-c","object":"response","status":"completed","output":[{"id":"call-item","type":"function_call","call_id":"call-c","name":"lookup","arguments":"{}","status":"completed"}],"usage":{"input_tokens":3,"output_tokens":4,"total_tokens":7}});
            let (kind, bytes) = if streaming {
                let start = json!({"type":"response.created","response":{"id":"response-c","status":"in_progress","output":[]}});
                let mut frames = format!("event: response.created\ndata: {start}\n\n");
                for event in ["response.output_item.added", "response.output_item.done"] {
                    let item = json!({"type":event,"output_index":0,"item":response["output"][0]});
                    write!(frames, "event: {event}\ndata: {item}\n\n")
                        .map_err(|_| internal_error())?;
                }
                let end = json!({"type":"response.completed","response":response});
                write!(frames, "event: response.completed\ndata: {end}\n\n")
                    .map_err(|_| internal_error())?;
                (
                    GrokOfficialResponseContentType::EventStream,
                    frames.into_bytes(),
                )
            } else {
                (
                    GrokOfficialResponseContentType::Json,
                    serde_json::to_vec(&response).map_err(|_| internal_error())?,
                )
            };
            let mut response =
                GrokOfficialTransportResponse::new(200, kind, Box::new(Body(Some(bytes))));
            if self.exhaust_quota {
                response =
                    response.with_rate_limit_metadata(GrokOfficialRateLimitMetadata::parse([
                        ("x-ratelimit-limit-requests", "10"),
                        ("x-ratelimit-remaining-requests", "0"),
                        ("x-ratelimit-reset-requests", "60s"),
                    ])?);
            }
            Ok(response)
        })
    }
}
#[actix_web::test]
async fn official_real_factory_retains_tools_controls_usage_and_quota_ownership() -> TestResult {
    for streaming in [false, true] {
        let peer = Arc::new(OfficialPeer {
            bodies: Mutex::new(Vec::new()),
            exhaust_quota: true,
        });
        let fixture = fixture(
            Channel {
                adapter: "grok.official.responses",
                format: "openai/responses",
                base: GROK_OFFICIAL_API_BASE_URL,
                path: GROK_OFFICIAL_RESPONSES_PATH,
                model: "grok-4",
            },
            b"synthetic-api-key",
            |endpoint| {
                let mut profiles = P12TransportProfiles::try_new()?;
                profiles.official = Some(peer.clone());
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
        let input = json!({"model":"p12-test-model","input":"question","stream":streaming,"max_output_tokens":19,"tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}],"tool_choice":{"type":"function","name":"lookup"},"parallel_tool_calls":true});
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(&input)
                .to_request(),
        )
        .await;
        let status = response.status();
        let bytes = test::read_body(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{} events {:?}; calls {}",
            String::from_utf8_lossy(&bytes),
            fixture.events.0.lock().map_err(|_| "events")?,
            peer.bodies.lock().map_err(|_| "bodies")?.len()
        );
        public_response::assert_round("/v1/responses", streaming, 0, &bytes)?;
        assert!(String::from_utf8_lossy(&bytes).contains("input_tokens"));
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(&input)
                .to_request(),
        )
        .await;
        assert!(
            !response.status().is_success(),
            "exhausted exact binding was leased again"
        );
        let bodies = peer.bodies.lock().map_err(|_| "bodies")?;
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0]["max_output_tokens"], 19);
        assert_eq!(bodies[0]["parallel_tool_calls"], true);
        assert_eq!(bodies[0]["tool_choice"], input["tool_choice"]);
    }
    Ok(())
}
