//! Test-only HTTP handoff preserves the native request before connecting to an isolated peer.
use super::*;
use actix_web::{HttpRequest, HttpResponse, HttpServer, dev::ServerHandle};
use gateway_upstream::EgressDnsError;
use std::net::{IpAddr, Ipv4Addr, TcpListener};

struct PublicResolver;
impl EgressDnsResolver for PublicResolver {
    fn resolve(&self, _: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
    }
}
struct Recorded {
    url: String,
    body: Value,
    headers: BTreeMap<String, String>,
}
pub(in crate::runtime) struct Peer {
    origin: String,
    policy: EgressPolicy,
    requests: Mutex<Vec<Recorded>>,
    server: ServerHandle,
}
impl Peer {
    fn start(reject_limit: bool) -> Result<Arc<Self>, Box<dyn Error>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(reject_limit))
                .default_service(web::post().to(reply))
        })
        .disable_signals()
        .workers(1)
        .listen(listener)?
        .run();
        let handle = server.handle();
        actix_web::rt::spawn(server);
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("batch-c-loopback")?,
            name: "Batch C loopback".into(),
            allowed_schemes: BTreeSet::from([EgressScheme::Http]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("127.0.0.1")?]),
            allowed_ports: BTreeSet::from([port]),
            allowed_cidrs: BTreeSet::from([EgressCidr::try_new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                32,
            )?]),
            redirect_policy: RedirectPolicy::Deny,
        })?;
        Ok(Arc::new(Self {
            origin: format!("http://127.0.0.1:{port}"),
            policy,
            requests: Mutex::new(Vec::new()),
            server: handle,
        }))
    }
    pub(in crate::runtime) fn handoff(
        &self,
        request: &UpstreamHttpRequest,
    ) -> Result<UpstreamHttpRequest, GatewayError> {
        let headers = [
            "accept",
            "authorization",
            "content-type",
            "user-agent",
            "anthropic-version",
            "x-api-key",
            "chatgpt-account-id",
            "openai-beta",
            "originator",
            "version",
            "x-msh-platform",
            "x-msh-device-name",
            "x-msh-device-model",
            "x-msh-device-id",
        ]
        .into_iter()
        .filter_map(|name| request.header(name).map(|value| (name, value)))
        .map(|(name, value)| {
            Ok((
                name.to_owned(),
                value.to_str().map_err(|_| internal_error())?.to_owned(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, GatewayError>>()?;
        self.requests
            .lock()
            .map_err(|_| internal_error())?
            .push(Recorded {
                url: request.target().request_url().as_str().into(),
                body: serde_json::from_slice(request.body()).map_err(|_| internal_error())?,
                headers: headers.clone(),
            });
        let local = format!("{}{}", self.origin, request.target().request_url().path());
        let target = self
            .policy
            .admit_url(&local, &SystemEgressDnsResolver)
            .map_err(|_| internal_error())?;
        UpstreamHttpRequest::try_new(target, request.method(), headers, request.body().to_vec())
            .map_err(|_| internal_error())
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        actix_web::rt::spawn(self.server.stop(false));
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "three literal upstream JSON/SSE shapes stay together in this test-only responder"
)]
async fn reply(
    request: HttpRequest,
    body: web::Json<Value>,
    reject: web::Data<bool>,
) -> Result<HttpResponse, actix_web::Error> {
    if **reject {
        return Ok(HttpResponse::BadRequest().json(json!({"error":{"message":"Unsupported parameter: max_output_tokens","type":"invalid_request_error","param":"max_output_tokens"}})));
    }
    let streaming = body["stream"] == true;
    let history = body.to_string();
    let second_call = history.contains("next-question");
    let tools = history.contains("lookup")
        && if second_call {
            !history.contains("result-d")
        } else {
            !history.contains("result-c")
        };
    let call_id = if second_call { "call-d" } else { "call-c" };
    let model = body["model"].clone();
    let messages = request.path().ends_with("/messages");
    let chat = request.path().ends_with("/chat/completions");
    let response = if messages {
        json!({"id":"msg-c","type":"message","role":"assistant","model":model,"content":if tools {json!([{"type":"tool_use","id":call_id,"name":"lookup","input":{}}])}else{json!([{"type":"text","text":"answer"}])},"stop_reason":if tools{"tool_use"}else{"end_turn"},"stop_sequence":null,"usage":{"input_tokens":3,"output_tokens":4}})
    } else if chat {
        json!({"id":"chat-c","object":"chat.completion","created":1,"model":model,"choices":[{"index":0,"message":if tools{json!({"role":"assistant","content":null,"tool_calls":[{"id":call_id,"type":"function","function":{"name":"lookup","arguments":"{}"}}]})}else{json!({"role":"assistant","content":"answer"})},"finish_reason":if tools{"tool_calls"}else{"stop"}}],"usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7}})
    } else {
        json!({"id":"resp-c","object":"response","status":"completed","model":model,"output":if tools{json!([{"id":"fc-c","type":"function_call","status":"completed","call_id":call_id,"name":"lookup","arguments":"{}"}])}else{json!([{"id":"msg-c","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"answer","annotations":[]}]}])},"usage":{"input_tokens":3,"output_tokens":4,"total_tokens":7}})
    };
    if !streaming {
        return Ok(HttpResponse::Ok().json(response));
    }
    let mut frames = String::new();
    if messages {
        frame(
            &mut frames,
            "message_start",
            json!({"message":{ "id":"msg-c","type":"message","role":"assistant","model":model,"content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":3,"output_tokens":0}}}),
        )?;
        frame(
            &mut frames,
            "content_block_start",
            json!({"index":0,"content_block":if tools{json!({"type":"tool_use","id":call_id,"name":"lookup","input":{}})}else{json!({"type":"text","text":""})}}),
        )?;
        frame(
            &mut frames,
            "content_block_delta",
            json!({"index":0,"delta":if tools{json!({"type":"input_json_delta","partial_json":"{}"})}else{json!({"type":"text_delta","text":"answer"})}}),
        )?;
        frame(&mut frames, "content_block_stop", json!({"index":0}))?;
        frame(
            &mut frames,
            "message_delta",
            json!({"delta":{"stop_reason":response["stop_reason"],"stop_sequence":null},"usage":{"output_tokens":4}}),
        )?;
        frame(&mut frames, "message_stop", json!({}))?;
    } else if chat {
        for (delta, finish, usage) in [
            (
                if tools {
                    json!({"role":"assistant","tool_calls":[{"index":0,"id":call_id,"type":"function","function":{"name":"lookup","arguments":"{}"}}]})
                } else {
                    json!({"role":"assistant","content":"answer"})
                },
                Value::Null,
                Value::Null,
            ),
            (
                json!({}),
                response["choices"][0]["finish_reason"].clone(),
                response["usage"].clone(),
            ),
        ] {
            let value = json!({"id":"chat-c","object":"chat.completion.chunk","created":1,"model":model,"choices":[{"index":0,"delta":delta,"finish_reason":finish}],"usage":usage});
            write!(frames, "data: {value}\n\n")
                .map_err(actix_web::error::ErrorInternalServerError)?;
        }
        frames.push_str("data: [DONE]\n\n");
    } else {
        frame(
            &mut frames,
            "response.created",
            json!({"response":{"id":"resp-c","object":"response","status":"in_progress","model":model,"output":[]}}),
        )?;
        let item = &response["output"][0];
        let mut empty = item.clone();
        empty["status"] = json!("in_progress");
        if tools {
            empty["arguments"] = json!("");
        } else {
            empty["content"] = json!([]);
        }
        frame(
            &mut frames,
            "response.output_item.added",
            json!({"output_index":0,"item":empty}),
        )?;
        frame(
            &mut frames,
            if tools {
                "response.function_call_arguments.delta"
            } else {
                "response.output_text.delta"
            },
            json!({"output_index":0,"item_id":item["id"],"content_index":0,"delta":if tools{"{}"}else{"answer"}}),
        )?;
        frame(
            &mut frames,
            "response.output_item.done",
            json!({"output_index":0,"item":item}),
        )?;
        frame(
            &mut frames,
            "response.completed",
            json!({"response":response}),
        )?;
    }
    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .body(frames))
}
fn frame(frames: &mut String, kind: &str, mut payload: Value) -> Result<(), actix_web::Error> {
    payload["type"] = json!(kind);
    write!(frames, "event: {kind}\ndata: {payload}\n\n")
        .map_err(actix_web::error::ErrorInternalServerError)
}

fn oauth(kind: &str) -> Result<String, Box<dyn Error>> {
    let mut document = json!({"kind":kind,"access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_at_ms":system_now_ms_runtime()?+3_600_000});
    document[if kind == "kimi_oauth" {
        "device_id"
    } else {
        "account_id"
    }] = json!(if kind == "kimi_oauth" {
        "synthetic-device"
    } else {
        "synthetic-account"
    });
    Ok(document.to_string())
}
#[actix_web::test]
async fn ordinary_native_families_use_the_public_factory_and_preserve_identity() -> TestResult {
    let codex = oauth("codex_oauth")?;
    let kimi = oauth("kimi_oauth")?;
    let claude=json!({"kind":"claude_oauth","access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_at_ms":system_now_ms_runtime()?+3_600_000}).to_string();
    for (base, path, adapter, format, secret) in [
        (
            "https://gateway.example.test/v1",
            "/responses",
            "openai-compatible.responses",
            "openai/responses",
            "synthetic-key",
        ),
        (
            "https://api.anthropic.com/v1",
            "/messages",
            "anthropic-compatible.messages",
            "anthropic/messages",
            "synthetic-key",
        ),
        (
            "https://chatgpt.com/backend-api/codex",
            "/responses",
            "openai-compatible.responses",
            "openai/responses",
            codex.as_str(),
        ),
        (
            "https://api.anthropic.com/v1",
            "/messages",
            "anthropic-compatible.messages",
            "anthropic/messages",
            claude.as_str(),
        ),
        (
            "https://api.kimi.com/coding",
            "/v1/responses",
            "openai-compatible.responses",
            "openai/responses",
            kimi.as_str(),
        ),
        (
            "https://api.kimi.com/coding",
            "/v1/chat/completions",
            "openai-compatible.chat-completions",
            "openai/chat-completions",
            kimi.as_str(),
        ),
        (
            "https://api.moonshot.cn/v1",
            "/chat/completions",
            "openai-compatible.chat-completions",
            "openai/chat-completions",
            "synthetic-key",
        ),
        (
            "https://api.moonshot.cn/v1",
            "/responses",
            "openai-compatible.responses",
            "openai/responses",
            "synthetic-key",
        ),
    ] {
        for streaming in [false, true] {
            let peer = Peer::start(false)?;
            let fixture = fixture(
                Channel {
                    adapter,
                    format,
                    base,
                    path,
                    model: "upstream-c",
                },
                secret.as_bytes(),
                |endpoint| {
                    let mut profiles = P12TransportProfiles::try_new()?;
                    profiles.http = Some(peer.clone());
                    endpoint.transports = Arc::new(profiles);
                    endpoint.resolver = Arc::new(PublicResolver);
                    Ok(())
                },
            )?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            for (uri, rounds) in super::grok::tool_conversations(streaming)? {
                for (round, body) in rounds.into_iter().enumerate() {
                    let response = test::call_service(
                        &app,
                        test::TestRequest::post()
                            .uri(uri)
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
                        "{base} {uri} streaming={streaming}: {}; calls {}; events {:?}",
                        String::from_utf8_lossy(&bytes),
                        peer.requests.lock().map_err(|_| "requests")?.len(),
                        fixture.events.0.lock().map_err(|_| "events")?
                    );
                    super::public_response::assert_round(uri, streaming, round, &bytes)?;
                }
            }
            let expected_channel = if base.contains("chatgpt.com") {
                "codex"
            } else if base.contains("kimi.com") {
                "kimi-coding"
            } else if base.contains("moonshot.cn") {
                "kimi-api"
            } else if secret == claude {
                "claude"
            } else if format == "anthropic/messages" {
                "anthropic-compatible"
            } else {
                "openai-compatible"
            };
            let events = fixture.events.0.lock().map_err(|_| "events")?;
            let attempts = events
                .iter()
                .filter_map(|event| match event {
                    GatewayEvent::Attempt(attempt) => Some(attempt),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(attempts.len(), 12);
            for attempt in attempts {
                let actual = attempt.execution_identity().ok_or("execution identity")?;
                assert!(actual.is_valid());
                assert_eq!(actual.channel, expected_channel, "{base} {format}");
                assert_eq!(actual.config_version_id, "batch-c");
                assert_eq!(actual.credential_revision, 1);
                assert_eq!(actual.egress, gateway_core::ExecutionEgress::Direct);
            }
            drop(events);
            let requests = peer.requests.lock().map_err(|_| "requests")?;
            assert_eq!(requests.len(), 12);
            for request in requests.iter() {
                assert_eq!(request.url, format!("{base}{path}"));
                assert_eq!(request.body["model"], "upstream-c");
                let limit = if format == "openai/responses" {
                    "max_output_tokens"
                } else {
                    "max_tokens"
                };
                assert_eq!(request.body[limit], 19);
                if base.contains("chatgpt.com") {
                    assert_eq!(
                        request
                            .headers
                            .get("chatgpt-account-id")
                            .map(String::as_str),
                        Some("synthetic-account")
                    );
                    assert_eq!(request.body["stream"], true);
                }
                if base.contains("kimi.com") {
                    assert_eq!(
                        request.headers.get("x-msh-device-id").map(String::as_str),
                        Some("synthetic-device")
                    );
                    assert!(!request.headers.contains_key("chatgpt-account-id"));
                }
                if secret == claude {
                    assert!(request.headers.contains_key("authorization"));
                    assert!(!request.headers.contains_key("x-api-key"));
                }
            }
        }
    }
    Ok(())
}

#[actix_web::test]
async fn codex_rejection_never_replays_without_the_clients_output_limit() -> TestResult {
    let peer = Peer::start(true)?;
    let secret = oauth("codex_oauth")?;
    let fixture = fixture(
        Channel {
            adapter: "openai-compatible.responses",
            format: "openai/responses",
            base: "https://chatgpt.com/backend-api/codex",
            path: "/responses",
            model: "upstream-c",
        },
        secret.as_bytes(),
        |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.http = Some(peer.clone());
            endpoint.transports = Arc::new(profiles);
            endpoint.resolver = Arc::new(PublicResolver);
            Ok(())
        },
    )?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(json!({"model":"p12-test-model","input":"question","max_output_tokens":19}))
            .to_request(),
    )
    .await;
    assert!(!response.status().is_success());
    let requests = peer.requests.lock().map_err(|_| "requests")?;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].body["max_output_tokens"], 19);
    Ok(())
}
