use super::*;
use provider_grok::{
    GrokBuildResponseBody, GrokBuildResponseContentType, GrokBuildResponsesOutboundRequest,
    GrokBuildTransport, GrokBuildTransportResponse, GrokConsoleResponseBody,
    GrokConsoleResponseContentType, GrokConsoleResponsesOutboundRequest, GrokConsoleTransport,
    GrokConsoleTransportResponse, GrokOfficialResponseContentType,
    GrokOfficialResponsesOutboundRequest, GrokOfficialTransport, GrokOfficialTransportResponse,
};

#[derive(Default)]
struct Peer {
    bodies: Mutex<Vec<Value>>,
    urls: Mutex<Vec<String>>,
    console_session: std::sync::atomic::AtomicBool,
    native_output: Option<Value>,
}
impl Peer {
    fn reply(&self, url: &str, body: &[u8]) -> Result<(bool, Vec<u8>), GatewayError> {
        let body: Value = serde_json::from_slice(body).map_err(|_| internal_error())?;
        let streaming = body["stream"] == true;
        let history = body.to_string();
        let second_call = history.contains("next-question");
        let tools = if second_call {
            !history.contains("result-d")
        } else {
            !history.contains("result-c")
        };
        let call_id = if second_call { "call-d" } else { "call-c" };
        self.urls
            .lock()
            .map_err(|_| internal_error())?
            .push(url.into());
        self.bodies.lock().map_err(|_| internal_error())?.push(body);
        let output = if let Some(output) = &self.native_output {
            output.clone()
        } else if tools {
            json!([{"id":"fc-c","type":"function_call","status":"completed","call_id":call_id,"name":"lookup","arguments":"{}"}])
        } else {
            json!([{"id":"msg-c","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"answer","annotations":[]}]}])
        };
        let response = json!({"id":"resp-c","object":"response","status":"completed","output":output,"usage":{"input_tokens":3,"output_tokens":4,"total_tokens":7}});
        if !streaming {
            return Ok((
                false,
                serde_json::to_vec(&response).map_err(|_| internal_error())?,
            ));
        }
        let start = json!({"type":"response.created","response":{"id":"resp-c","status":"in_progress","output":[]}});
        let mut frames = format!("event: response.created\ndata: {start}\n\n");
        for (index, item) in response["output"]
            .as_array()
            .ok_or_else(internal_error)?
            .iter()
            .enumerate()
        {
            for kind in ["response.output_item.added", "response.output_item.done"] {
                let event = json!({"type":kind,"output_index":index,"item":item});
                write!(frames, "event: {kind}\ndata: {event}\n\n").map_err(|_| internal_error())?;
            }
        }
        let end = json!({"type":"response.completed","response":response});
        write!(frames, "event: response.completed\ndata: {end}\n\n")
            .map_err(|_| internal_error())?;
        Ok((true, frames.into_bytes()))
    }
}
impl GrokBuildResponseBody for Body {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async { Ok(self.0.take()) })
    }
}
impl GrokConsoleResponseBody for Body {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async { Ok(self.0.take()) })
    }
}
impl GrokBuildTransport for Peer {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            let (streaming, bytes) = self.reply(request.url(), request.body())?;
            Ok(GrokBuildTransportResponse::new(
                200,
                if streaming {
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
impl GrokConsoleTransport for Peer {
    fn send_with_egress_attempt(
        &self,
        request: GrokConsoleResponsesOutboundRequest,
        attempt: Option<Arc<provider_grok::GrokNativeEgressAttempt>>,
    ) -> BoxFuture<'_, Result<GrokConsoleTransportResponse, GatewayError>> {
        Box::pin(async move {
            if let Some(attempt) = attempt {
                if !self.console_session.load(Ordering::Acquire) {
                    attempt
                        .begin_console_session_bootstrap()
                        .map_err(|_| internal_error())?;
                    attempt
                        .complete_console_session_bootstrap(
                            system_now_ms_runtime().map_err(|_| internal_error())? + 60_000,
                        )
                        .map_err(|_| internal_error())?;
                    self.console_session.store(true, Ordering::Release);
                }
                attempt
                    .record_inference_submission()
                    .map_err(|_| internal_error())?;
            }
            GrokConsoleTransport::send(self, request).await
        })
    }

    fn send(
        &self,
        request: GrokConsoleResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokConsoleTransportResponse, GatewayError>> {
        Box::pin(async move {
            let (streaming, bytes) = self.reply(request.url(), request.body())?;
            Ok(GrokConsoleTransportResponse::new(
                200,
                if streaming {
                    GrokConsoleResponseContentType::EventStream
                } else {
                    GrokConsoleResponseContentType::Json
                },
                Box::new(Body(Some(bytes))),
            ))
        })
    }
}

impl GrokOfficialTransport for Peer {
    fn send(
        &self,
        request: GrokOfficialResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokOfficialTransportResponse, GatewayError>> {
        Box::pin(async move {
            let (streaming, bytes) = self.reply(request.url(), request.body())?;
            Ok(GrokOfficialTransportResponse::new(
                200,
                if streaming {
                    GrokOfficialResponseContentType::EventStream
                } else {
                    GrokOfficialResponseContentType::Json
                },
                Box::new(Body(Some(bytes))),
            ))
        })
    }
}

fn native_channels() -> [(Channel, &'static str); 3] {
    [
        (
            "grok.build.responses",
            provider_grok::GROK_BUILD_RESPONSES_BASE_URL,
            provider_grok::GROK_BUILD_RESPONSES_PATH,
            "grok-4.5",
            r#"{"access_token":"synthetic-build","refresh_token":"synthetic-refresh","expires_in":3600,"token_type":"Bearer"}"#,
        ),
        (
            "grok.console.responses",
            provider_grok::GROK_CONSOLE_RESPONSES_BASE_URL,
            provider_grok::GROK_CONSOLE_RESPONSES_PATH,
            "grok-4.3",
            "synthetic-sso",
        ),
        (
            "grok.official.responses",
            provider_grok::GROK_OFFICIAL_API_BASE_URL,
            provider_grok::GROK_OFFICIAL_RESPONSES_PATH,
            "grok-4",
            "synthetic-api-key",
        ),
    ].map(|(adapter, base, path, model, secret)| (Channel { adapter, format: "openai/responses", base, path, model }, secret))
}

fn install_peer(
    endpoint: &mut EndpointRuntime,
    adapter: &str,
    peer: &Arc<Peer>,
) -> Result<(), Box<dyn Error>> {
    let mut profiles = P12TransportProfiles::try_new()?;
    if adapter == "grok.build.responses" {
        profiles.build = Some(peer.clone());
    } else if adapter == "grok.console.responses" {
        profiles.console = Some(peer.clone());
    } else {
        profiles.official = Some(peer.clone());
    }
    endpoint.transports = Arc::new(profiles);
    Ok(())
}

#[actix_web::test]
async fn grok_factories_bridge_three_protocols_and_preserve_tool_rounds() -> TestResult {
    for (channel, secret) in native_channels() {
        let Channel {
            adapter,
            base,
            path,
            ..
        } = channel;
        for streaming in [false, true] {
            let peer = Arc::new(Peer::default());
            let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
                install_peer(endpoint, adapter, &peer)
            })?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            for (uri, rounds) in tool_conversations(streaming)? {
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
                        "{adapter} {uri} stream={streaming} round={round}: {}; calls {}; events {:?}",
                        String::from_utf8_lossy(&bytes),
                        peer.bodies.lock().map_err(|_| "bodies")?.len(),
                        fixture.events.0.lock().map_err(|_| "events")?
                    );
                    super::public_response::assert_round(uri, streaming, round, &bytes)?;
                }
            }
            let bodies = peer.bodies.lock().map_err(|_| "bodies")?;
            assert_eq!(bodies.len(), 12);
            for (index, body) in bodies.iter().enumerate() {
                assert_eq!(body["max_output_tokens"], 19);
                assert_eq!(body["tools"].as_array().map(Vec::len), Some(1));
                assert_eq!(body.to_string().contains("result-c"), index % 4 != 0);
                assert_eq!(body.to_string().contains("result-d"), index % 4 == 3);
            }
            assert!(
                peer.urls
                    .lock()
                    .map_err(|_| "urls")?
                    .iter()
                    .all(|url| url == &format!("{base}{path}"))
            );
            assert!(
                fixture
                    .events
                    .0
                    .lock()
                    .map_err(|_| "events")?
                    .iter()
                    .filter_map(|event| if let GatewayEvent::Attempt(attempt) = event {
                        Some(attempt)
                    } else {
                        None
                    })
                    .all(|attempt| if adapter == "grok.official.responses" {
                        attempt.credential_id().as_str() == "p12-runtime-credential"
                    } else {
                        attempt.credential_id().as_str().starts_with("grok-")
                    })
            );
        }
    }
    Ok(())
}

fn public_frames(bytes: &[u8]) -> Result<Vec<Value>, Box<dyn Error>> {
    let wire = std::str::from_utf8(bytes)?;
    if !wire.ends_with("\n\n") {
        return Err("truncated SSE".into());
    }
    wire.split("\n\n")
        .filter(|record| !record.is_empty())
        .map(|record| {
            let data = record
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .ok_or("SSE data missing")?;
            Ok(serde_json::from_str(data)?)
        })
        .collect()
}

#[actix_web::test]
async fn grok_native_metadata_survives_public_responses_and_rejects_lossy_bridges() -> TestResult {
    let output = json!([
        {"id":"reason-public","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"summary"}],"content":[{"type":"reasoning_text","text":"body"}]},
        {"id":"message-public","type":"message","role":"assistant","status":"completed","phase":"final_answer","content":[{"type":"output_text","text":"answer","annotations":[{"type":"url_citation","url":"https://example.test/source","title":"Source","start_index":0,"end_index":6}],"logprobs":null}]}
    ]);
    for (channel, secret) in native_channels() {
        for streaming in [false, true] {
            let peer = Arc::new(Peer {
                native_output: Some(output.clone()),
                ..Peer::default()
            });
            let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
                install_peer(endpoint, channel.adapter, &peer)
            })?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let mut history = output.as_array().ok_or("output")?.clone();
            history.push(json!({"role":"user","content":"next-question"}));
            for input in [json!("question"), Value::Array(history)] {
                let response = test::call_service(&app, test::TestRequest::post().uri("/v1/responses").insert_header(("authorization", format!("Bearer {}", fixture.key))).set_json(json!({"model":"p12-test-model","input":input,"stream":streaming,"max_output_tokens":19})).to_request()).await;
                let status = response.status();
                let bytes = test::read_body(response).await;
                assert_eq!(
                    status,
                    StatusCode::OK,
                    "{}: {}",
                    channel.adapter,
                    String::from_utf8_lossy(&bytes)
                );
                let encoded = if streaming {
                    let frames = public_frames(&bytes)?;
                    assert_eq!(
                        frames
                            .iter()
                            .filter(|frame| frame["type"] == "response.completed")
                            .count(),
                        1
                    );
                    frames.last().ok_or("terminal")?["response"].clone()
                } else {
                    serde_json::from_slice(&bytes)?
                };
                assert_eq!(encoded["output"], output);
            }
            {
                let bodies = peer.bodies.lock().map_err(|_| "bodies")?;
                assert_eq!(
                    &bodies[1]["input"].as_array().ok_or("input")?[..2],
                    output.as_array().ok_or("output")?
                );
            }
            for (uri, body) in [
                (
                    "/v1/chat/completions",
                    json!({"model":"p12-test-model","messages":[{"role":"user","content":"question"}],"stream":streaming,"max_tokens":19}),
                ),
                (
                    "/v1/messages",
                    json!({"model":"p12-test-model","messages":[{"role":"user","content":"question"}],"stream":streaming,"max_tokens":19}),
                ),
            ] {
                let before = peer.bodies.lock().map_err(|_| "bodies")?.len();
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
                if streaming && status.is_success() {
                    let frames = public_frames(&bytes)?;
                    assert_eq!(frames.iter().filter(|frame| frame.get("error").is_some() || frame["type"] == "error").count(), 1);
                    assert!(frames.iter().all(|frame| frame["type"] != "message_stop"
                        && frame["type"] != "response.completed"
                        && frame["choices"].as_array().is_none_or(|choices| {
                            choices
                                .iter()
                                .all(|choice| choice["finish_reason"].is_null())
                        })));
                } else {
                    assert!(!status.is_success());
                    assert!(
                        serde_json::from_slice::<Value>(&bytes)?
                            .get("error")
                            .is_some()
                    );
                }
                assert_eq!(peer.bodies.lock().map_err(|_| "bodies")?.len(), before + 1);
            }
        }
    }
    Ok(())
}

/// Exact public request histories, shared by the independently selected native factories.
pub(super) fn tool_rounds(streaming: bool) -> [(&'static str, Value, Value); 3] {
    [
        (
            "/v1/responses",
            json!({"model":"p12-test-model","input":"question","stream":streaming,"max_output_tokens":19,"tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]}),
            json!({"model":"p12-test-model","input":[{"role":"user","content":"question"},{"type":"function_call","call_id":"call-c","name":"lookup","arguments":"{}"},{"type":"function_call_output","call_id":"call-c","output":"result-c"}],"stream":streaming,"max_output_tokens":19,"tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]}),
        ),
        (
            "/v1/chat/completions",
            json!({"model":"p12-test-model","messages":[{"role":"user","content":"question"}],"stream":streaming,"max_tokens":19,"tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}]}),
            json!({"model":"p12-test-model","messages":[{"role":"user","content":"question"},{"role":"assistant","content":null,"tool_calls":[{"id":"call-c","type":"function","function":{"name":"lookup","arguments":"{}"}}]},{"role":"tool","tool_call_id":"call-c","content":"result-c"}],"stream":streaming,"max_tokens":19,"tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}]}),
        ),
        (
            "/v1/messages",
            json!({"model":"p12-test-model","max_tokens":19,"messages":[{"role":"user","content":"question"}],"stream":streaming,"tools":[{"name":"lookup","input_schema":{"type":"object"}}]}),
            json!({"model":"p12-test-model","max_tokens":19,"messages":[{"role":"user","content":"question"},{"role":"assistant","content":[{"type":"tool_use","id":"call-c","name":"lookup","input":{}}]},{"role":"user","content":[{"type":"tool_result","tool_use_id":"call-c","content":"result-c"}]}],"stream":streaming,"tools":[{"name":"lookup","input_schema":{"type":"object"}}]}),
        ),
    ]
}

type ToolConversation = (&'static str, [Value; 4]);

/// Two correlated tool cycles separated by an actual user/assistant text round.
pub(super) fn tool_conversations(streaming: bool) -> Result<Vec<ToolConversation>, Box<dyn Error>> {
    tool_rounds(streaming).into_iter().map(|(uri, first, second)| {
        let mut third = second.clone();
        let history = if uri == "/v1/responses" {"input"} else {"messages"};
        let messages = third[history].as_array_mut().ok_or("fixture history")?;
        messages.extend([
            json!({"role":"assistant","content":"answer"}),
            json!({"role":"user","content":"next-question"}),
        ]);
        let mut fourth = third.clone();
        let messages = fourth[history].as_array_mut().ok_or("fixture history")?;
        let pair = match uri {
            "/v1/responses" => [
                json!({"type":"function_call","call_id":"call-d","name":"lookup","arguments":"{}"}),
                json!({"type":"function_call_output","call_id":"call-d","output":"result-d"}),
            ],
            "/v1/chat/completions" => [
                json!({"role":"assistant","content":null,"tool_calls":[{"id":"call-d","type":"function","function":{"name":"lookup","arguments":"{}"}}]}),
                json!({"role":"tool","tool_call_id":"call-d","content":"result-d"}),
            ],
            _ => [
                json!({"role":"assistant","content":[{"type":"tool_use","id":"call-d","name":"lookup","input":{}}]}),
                json!({"role":"user","content":[{"type":"tool_result","tool_use_id":"call-d","content":"result-d"}]}),
            ],
        };
        messages.extend(pair);
        Ok((uri, [first, second, third, fourth]))
    }).collect()
}

#[derive(Default)]
struct NoNetworkDns(std::sync::atomic::AtomicUsize);
impl EgressDnsResolver for NoNetworkDns {
    fn resolve(
        &self,
        _: &gateway_upstream::EgressHost,
    ) -> Result<Vec<std::net::IpAddr>, gateway_upstream::EgressDnsError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Err(gateway_upstream::EgressDnsError)
    }
}

#[actix_web::test]
async fn web_unrepresentable_histories_tools_and_limits_reject_before_lease_or_dns() -> TestResult {
    let dns = Arc::new(NoNetworkDns::default());
    let secret = json!({"kind":"grok_web_sso","account_ref":"web-c","lineage_ref":"lineage-c","revision":1,"expires_at_ms":system_now_ms_runtime()?+3_600_000,"cookies":[{"name":"sso","value":"synthetic-web-sso","domain":"grok.com","path":"/","secure":true,"http_only":true}]}).to_string();
    let fixture = fixture(
        Channel {
            adapter: "grok.web.responses",
            format: "openai/responses",
            base: GROK_WEB_PRODUCTION_BASE_URL,
            path: GROK_WEB_CANARY_PATH,
            model: "grok-chat-fast",
        },
        secret.as_bytes(),
        |endpoint| {
            endpoint.resolver = dns.clone();
            Ok(())
        },
    )?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    for (uri, body) in [
        (
            "/v1/responses",
            json!({"model":"p12-test-model","input":"question","max_output_tokens":19}),
        ),
        (
            "/v1/messages",
            json!({"model":"p12-test-model","messages":[{"role":"user","content":"question"}],"max_tokens":19}),
        ),
        (
            "/v1/chat/completions",
            json!({"model":"p12-test-model","messages":[{"role":"user","content":"question"},{"role":"assistant","content":"answer"},{"role":"user","content":"next-question"}]}),
        ),
        (
            "/v1/responses",
            json!({"model":"p12-test-model","input":"question","tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]}),
        ),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(uri)
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(fixture.events.attempts()?, 0);
        assert_eq!(dns.0.load(Ordering::Relaxed), 0);
    }
    Ok(())
}
