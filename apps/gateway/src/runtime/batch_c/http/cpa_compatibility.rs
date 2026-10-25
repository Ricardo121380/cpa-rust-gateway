use super::*;

#[actix_web::test]
async fn native_messages_passthrough_keeps_client_options_at_receiver() -> TestResult {
    for (thinking, effort) in [
        (Some(json!({"type":"adaptive","display":"updates"})), "high"),
        (None, " HIGH "),
        (None, "none"),
        (Some(json!({"type":"disabled"})), "high"),
        (Some(json!({"type":"enabled","budget_tokens":1024})), "high"),
    ] {
        for streaming in [false, true] {
            let peer = Peer::start(false, None)?;
            let fixture = fixture_with_configuration(
                Channel {
                    adapter: "anthropic-compatible.messages",
                    format: "anthropic/messages",
                    base: "https://api.anthropic.com/v1",
                    path: "/messages",
                    model: "upstream-c",
                },
                b"synthetic-key",
                false,
                |configuration| {
                    configuration.route_candidates[0].transform_mode = TransformMode::Passthrough;
                    Ok(())
                },
                |endpoints| {
                    for endpoint in endpoints.values_mut() {
                        let mut profiles = P12TransportProfiles::try_new()?;
                        profiles.http = Some(peer.clone());
                        endpoint.transports = Arc::new(profiles);
                        endpoint.resolver = Arc::new(PublicResolver);
                    }
                    Ok(())
                },
            )?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let mut source = json!({"model":"p12-test-model","max_tokens":32000,"stream":streaming,
            "output_config":{"effort":effort},
            "metadata":{"user_id":"synthetic-native-owner"},
            "context_management":{"edits":[{"type":"clear_thinking_20251015","keep":"all"}]},
            "vendor_option":{"future":true},
            "messages":[{"role":"user","vendor_message":3,"content":[{"type":"text","text":"question","vendor_part":17,"cache_control":{"type":"ephemeral"}}]}]});
            if let Some(thinking) = &thinking {
                source["thinking"] = thinking.clone();
            }
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/messages")
                    .insert_header(("x-api-key", fixture.key.clone()))
                    .set_json(&source)
                    .to_request(),
            )
            .await;
            let status = reply.status();
            let bytes = test::read_body(reply).await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(&bytes)
            );
            {
                let received = peer.received.lock().map_err(|_| "receiver")?;
                assert_eq!(received.len(), 1);
                let mut expected = source;
                expected["model"] = json!("upstream-c");
                assert_eq!(received[0], expected);
            }
            assert_eq!(fixture.events.attempts()?, 1);
            peer.server.stop(false).await;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn cpa_v8_messages_effort_reaches_http_receiver() -> TestResult {
    for (adapter, format, path, effort_pointer) in [
        (
            "openai-compatible.chat-completions",
            "openai/chat-completions",
            "/chat/completions",
            "/reasoning_effort",
        ),
        (
            "openai-compatible.responses",
            "openai/responses",
            "/responses",
            "/reasoning/effort",
        ),
    ] {
        for streaming in [false, true] {
            let peer = Peer::start(false, None)?;
            let fixture = fixture(
                Channel {
                    adapter,
                    format,
                    path,
                    base: "https://api.moonshot.cn/v1",
                    model: "upstream-c",
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
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let cases = [
                (None, Some(" HIGH "), Some("high")),
                (None, Some("none"), Some("none")),
                (None, Some("auto"), Some("auto")),
                (None, None, None),
                (None, Some("  "), None),
                (
                    Some(json!({"type":"enabled"})),
                    Some(" HIGH "),
                    Some("high"),
                ),
                (Some(json!({"type":"enabled"})), Some("none"), Some("none")),
                (Some(json!({"type":"enabled"})), None, Some("auto")),
                (
                    Some(json!({"type":"enabled","budget_tokens":1024})),
                    Some("high"),
                    Some("low"),
                ),
                (Some(json!({"type":"disabled"})), Some("high"), Some("none")),
            ];
            let count = cases.len();
            for (index, (thinking, effort, expected)) in cases.into_iter().enumerate() {
                let mut source = json!({"model":"p12-test-model","max_tokens":32000,"stream":streaming,"messages":[{"role":"user","content":"question"}]});
                if let Some(thinking) = thinking {
                    source["thinking"] = thinking;
                }
                if let Some(effort) = effort {
                    source["output_config"] = json!({"effort":effort});
                }
                let reply = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri("/v1/messages")
                        .insert_header(("x-api-key", fixture.key.clone()))
                        .set_json(&source)
                        .to_request(),
                )
                .await;
                let status = reply.status();
                let bytes = test::read_body(reply).await;
                assert_eq!(
                    status,
                    StatusCode::OK,
                    "adapter={adapter}, source={source}, response={}",
                    String::from_utf8_lossy(&bytes)
                );
                super::super::public_response::assert_round("/v1/messages", streaming, 1, &bytes)?;
                {
                    let received = peer.received.lock().map_err(|_| "receiver")?;
                    assert_eq!(received.len(), index + 1);
                    let wire = received.last().ok_or("received request")?;
                    assert_eq!(
                        wire.pointer(effort_pointer).and_then(Value::as_str),
                        expected,
                        "source={source}, wire={wire}"
                    );
                    assert_eq!(wire["model"], "upstream-c");
                    assert_eq!(wire["stream"], streaming);
                    assert!(wire.get("thinking").is_none());
                    assert!(wire.get("output_config").is_none());
                }
                assert_eq!(fixture.events.attempts()?, index + 1);
            }
            // Malformed controls and orphan tool history still fail before any send.
            for invalid in [
                json!({"effort":false}),
                json!({"effort":"unknown"}),
                json!({"effort":"high"}),
            ] {
                let mut source = json!({"model":"p12-test-model","max_tokens":32000,"stream":streaming,"output_config":invalid,"messages":[{"role":"user","content":"question"}]});
                if source["output_config"]["effort"] == "high" {
                    source["messages"][0]["content"] =
                        json!([{"type":"tool_result","tool_use_id":"orphan","content":"result"}]);
                }
                let reply = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri("/v1/messages")
                        .insert_header(("x-api-key", fixture.key.clone()))
                        .set_json(&source)
                        .to_request(),
                )
                .await;
                assert_eq!(reply.status(), StatusCode::BAD_REQUEST);
                let _ = test::read_body(reply).await;
                assert_eq!(peer.requests.lock().map_err(|_| "handoff")?.len(), count);
                assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), count);
                assert_eq!(fixture.events.attempts()?, count);
            }
            peer.server.stop(false).await;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn cpa_69_model_controls_reach_independent_receiver() -> TestResult {
    for (adapter, format, path, effort_pointer) in [
        (
            "openai-compatible.chat-completions",
            "openai/chat-completions",
            "/chat/completions",
            "/reasoning_effort",
        ),
        (
            "openai-compatible.responses",
            "openai/responses",
            "/responses",
            "/reasoning/effort",
        ),
    ] {
        for streaming in [false, true] {
            let peer = Peer::start(false, None)?;
            let fixture = fixture(
                Channel {
                    adapter,
                    format,
                    path,
                    base: "https://api.moonshot.cn/v1",
                    model: "gpt-5.5",
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
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            for (index, (thinking, expected, speed, tier, expected_tier)) in [
                (
                    json!({"type":"enabled","budget_tokens":0}),
                    "low",
                    None,
                    None,
                    None,
                ),
                (
                    json!({"type":"enabled","budget_tokens":-1}),
                    "medium",
                    Some("fast"),
                    None,
                    Some("priority"),
                ),
                (
                    json!({"type":"enabled","budget_tokens":512}),
                    "low",
                    None,
                    Some(" PRIORITY "),
                    Some("priority"),
                ),
                (
                    json!({"type":"enabled","budget_tokens":128_000}),
                    "xhigh",
                    None,
                    Some("ultrafast"),
                    None,
                ),
                (json!({"type":"enabled"}), "high", Some("FAST"), None, None),
            ]
            .into_iter()
            .enumerate()
            {
                let mut source = json!({"model":"p12-test-model","max_tokens":32000,"stream":streaming,"messages":[{"role":"user","content":"question"}],"thinking":thinking,"output_config":{"effort":"high"}});
                if let Some(speed) = speed {
                    source["speed"] = json!(speed);
                }
                if let Some(tier) = tier {
                    source["service_tier"] = json!(tier);
                }
                let reply = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri("/v1/messages")
                        .insert_header(("x-api-key", fixture.key.clone()))
                        .set_json(&source)
                        .to_request(),
                )
                .await;
                let status = reply.status();
                let bytes = test::read_body(reply).await;
                assert_eq!(
                    status,
                    StatusCode::OK,
                    "{source}: {}",
                    String::from_utf8_lossy(&bytes)
                );
                super::super::public_response::assert_round("/v1/messages", streaming, 1, &bytes)?;
                let received = peer.received.lock().map_err(|_| "receiver")?;
                let wire = received.last().ok_or("receive")?;
                assert_eq!(received.len(), index + 1);
                assert_eq!(wire["model"], "gpt-5.5");
                assert_eq!(
                    wire.pointer(effort_pointer).and_then(Value::as_str),
                    Some(expected)
                );
                let expected_tier = if format == "openai/responses" {
                    expected_tier
                } else {
                    None
                };
                assert_eq!(wire["service_tier"].as_str(), expected_tier);
                assert!(wire.get("speed").is_none());
            }
            for (field, value) in [
                ("thinking", json!({"type":"enabled","budget_tokens":-2})),
                ("thinking", json!({"type":"enabled","budget_tokens":"0"})),
                ("speed", json!(false)),
                ("service_tier", json!([])),
            ] {
                let mut source = json!({"model":"p12-test-model","max_tokens":32000,"stream":streaming,"messages":[{"role":"user","content":"question"}]});
                source[field] = value;
                let reply = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri("/v1/messages")
                        .insert_header(("x-api-key", fixture.key.clone()))
                        .set_json(&source)
                        .to_request(),
                )
                .await;
                assert_eq!(reply.status(), StatusCode::BAD_REQUEST);
                let _ = test::read_body(reply).await;
                assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 5);
                assert_eq!(fixture.events.attempts()?, 5);
            }
            peer.server.stop(false).await;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn cpa_69_responses_controls_reach_claude_receiver() -> TestResult {
    for streaming in [false, true] {
        let peer = Peer::start(false, None)?;
        let fixture = fixture(
            Channel {
                adapter: "anthropic-compatible.messages",
                format: "anthropic/messages",
                path: "/messages",
                base: "https://api.anthropic.com/v1",
                model: "claude-sonnet-4-6",
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
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        for (index, (effort, tier, expected_speed)) in [
            ("max", "priority", Some("fast")),
            ("minimal", " PRIORITY ", None),
        ]
        .into_iter()
        .enumerate()
        {
            let source = json!({"model":"p12-test-model","input":"question","max_output_tokens":512,"stream":streaming,"reasoning":{"effort":effort},"service_tier":tier});
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(&source)
                    .to_request(),
            )
            .await;
            let status = reply.status();
            let bytes = test::read_body(reply).await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(&bytes)
            );
            let received = peer.received.lock().map_err(|_| "receiver")?;
            assert_eq!(received.len(), index + 1);
            let wire = received.last().ok_or("wire")?;
            assert_eq!(wire["model"], "claude-sonnet-4-6");
            assert_eq!(wire["max_tokens"], 512);
            assert_eq!(wire["thinking"]["type"], "adaptive");
            assert_eq!(
                wire["output_config"]["effort"],
                if effort == "max" { "max" } else { "low" }
            );
            assert_eq!(wire["speed"].as_str(), expected_speed);
            assert!(wire.get("service_tier").is_none());
        }
        peer.server.stop(false).await;
    }
    Ok(())
}
