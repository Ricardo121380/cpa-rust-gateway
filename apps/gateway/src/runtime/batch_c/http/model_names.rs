//! #69 uses public HTTP and a separate receiver with production route compilation/admission.
use super::*;
use gateway_store::control_plane::{
    AccessGroupConfiguration, AdministrativeStatus, ModelAliasConfiguration,
};

fn source(uri: &str, model: &str, streaming: bool) -> Value {
    match uri {
        "/v1/messages" => {
            json!({"model":model,"max_tokens":32000,"stream":streaming,"messages":[{"role":"user","content":"question"}],"thinking":{"type":"enabled","budget_tokens":24576}})
        }
        "/v1/chat/completions" => {
            json!({"model":model,"max_completion_tokens":32000,"stream":streaming,"messages":[{"role":"user","content":"question"}],"reasoning_effort":"high"})
        }
        _ => {
            json!({"model":model,"max_output_tokens":32000,"stream":streaming,"input":"question","reasoning":{"effort":"high"},"vendor_option":{"future":true}})
        }
    }
}

#[actix_web::test]
async fn cpa_69_prefixed_suffix_names_keep_permissions_and_native_wire() -> TestResult {
    for uri in ["/v1/messages", "/v1/chat/completions", "/v1/responses"] {
        for streaming in [false, true] {
            let peer = Peer::start(false, None)?;
            let mut denied_key = None;
            let fixture = fixture_with_configuration(
                Channel {
                    adapter: "openai-compatible.responses",
                    format: "openai/responses",
                    base: "https://api.moonshot.cn/v1",
                    path: "/responses",
                    model: "gpt-5.5",
                },
                b"synthetic-key",
                false,
                |config| {
                    if uri == "/v1/responses" {
                        config.route_candidates[0].transform_mode = TransformMode::Passthrough;
                    }
                    let id = config.public_models[0].id.clone();
                    for alias in ["team/p12-test-model", "team/p12-test-model(low)"] {
                        config.model_aliases.push(ModelAliasConfiguration {
                            alias: alias.into(),
                            public_model_id: id.clone(),
                        });
                    }
                    let mut hidden = config.public_models[0].clone();
                    hidden.id = gateway_core::PublicModelId::try_new("hidden-model")?;
                    hidden.model_name = "hidden-model".into();
                    let mut hidden_route = config.model_routes[0].clone();
                    hidden_route.id = RouteId::try_new("hidden-route")?;
                    hidden_route.public_model_id = hidden.id.clone();
                    let mut hidden_candidate = config.route_candidates[0].clone();
                    hidden_candidate.id =
                        gateway_core::RouteCandidateId::try_new("hidden-candidate")?;
                    hidden_candidate.route_id = hidden_route.id.clone();
                    hidden_candidate.upstream_model = "gpt-5.5(medium)".into();
                    config.model_aliases.push(ModelAliasConfiguration {
                        alias: "team/p12-test-model(medium)".into(),
                        public_model_id: hidden.id.clone(),
                    });
                    config.public_models.push(hidden);
                    config.model_routes.push(hidden_route);
                    config.route_candidates.push(hidden_candidate);
                    let group = gateway_core::AccessGroupId::try_new("no-model-access")?;
                    config.access_groups.push(AccessGroupConfiguration {
                        id: group.clone(),
                        name: "denied".into(),
                        limits_json: "{}".into(),
                        status: AdministrativeStatus::Active,
                    });
                    let issuer =
                        ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x63; 32])?);
                    let (key, presented) = issuer
                        .issue(ClientKeyId::try_new("denied-key")?, group, None)?
                        .into_parts();
                    denied_key = Some(presented.as_str().to_owned());
                    config.client_keys.push(StoredClientKey::try_new(
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
            for (index, (name, effort)) in [
                ("team/p12-test-model(512)", "low"),
                ("team/p12-test-model(-1)", "medium"),
                ("team/p12-test-model(0)", "low"),
                ("team/p12-test-model(HIGH)", "high"),
                ("team/p12-test-model(low)", "high"),
            ]
            .into_iter()
            .enumerate()
            {
                // The final name is an exact configured alias and retains body high; it is not
                // interpreted as a suffix. Other names override the explicit high body.
                let reply = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(uri)
                        .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                        .set_json(source(uri, name, streaming))
                        .to_request(),
                )
                .await;
                let status = reply.status();
                let bytes = test::read_body(reply).await;
                assert_eq!(
                    status,
                    StatusCode::OK,
                    "{uri} {name}: {}",
                    String::from_utf8_lossy(&bytes)
                );
                super::super::public_response::assert_round(uri, streaming, 1, &bytes)?;
                let received = peer.received.lock().map_err(|_| "receive")?;
                assert_eq!(received.len(), index + 1);
                let wire = received.last().ok_or("wire")?;
                assert_eq!(wire["model"], "gpt-5.5");
                assert_eq!(wire["reasoning"]["effort"], effort);
                if uri == "/v1/responses" {
                    assert_eq!(wire["vendor_option"], json!({"future":true}));
                }
            }
            for name in [
                "team/p12-test-model(medium)",
                "gpt-5.5(medium)",
                "other/p12-test-model(high)",
                "team/p12-test-model(bogus)",
                "team/p12-test-model()",
                "team/p12-test-model(-2)",
                "team/p12-test-model( high )",
                "team/p12-test-model(999999999999999999999999)",
            ] {
                let reply = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(uri)
                        .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                        .set_json(source(uri, name, streaming))
                        .to_request(),
                )
                .await;
                assert!(reply.status().is_client_error());
                let _ = test::read_body(reply).await;
            }
            if uri == "/v1/messages" {
                let mut invalid = source(uri, "team/p12-test-model(512)", streaming);
                invalid["thinking"] = json!({"type":"bogus"});
                let reply = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(uri)
                        .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                        .set_json(invalid)
                        .to_request(),
                )
                .await;
                assert_eq!(reply.status(), StatusCode::BAD_REQUEST);
                let _ = test::read_body(reply).await;
            }
            let denied_key = denied_key.ok_or("denied key")?;
            for name in [
                "team/p12-test-model",
                "team/p12-test-model(512)",
                "team/p12-test-model(low)",
                "gpt-5.5(high)",
            ] {
                let reply = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(uri)
                        .insert_header(("authorization", format!("Bearer {denied_key}")))
                        .set_json(source(uri, name, streaming))
                        .to_request(),
                )
                .await;
                assert_eq!(reply.status(), StatusCode::NOT_FOUND);
                let _ = test::read_body(reply).await;
            }
            let list = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri("/v1/models")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .to_request(),
            )
            .await;
            let list: Value = test::read_body_json(list).await;
            assert!(
                list["data"]
                    .as_array()
                    .ok_or("list")?
                    .iter()
                    .any(|v| v["id"] == "team/p12-test-model")
            );
            assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 5);
            assert_eq!(fixture.events.attempts()?, 5);
            peer.server.stop(false).await;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn cpa_69_native_messages_and_chat_suffix_overlay_preserves_options() -> TestResult {
    for (uri, adapter, format, path, model, pointer) in [
        (
            "/v1/messages",
            "anthropic-compatible.messages",
            "anthropic/messages",
            "/messages",
            "claude-sonnet-4-6",
            "/output_config/effort",
        ),
        (
            "/v1/chat/completions",
            "openai-compatible.chat-completions",
            "openai/chat-completions",
            "/chat/completions",
            "gpt-5.5",
            "/reasoning_effort",
        ),
    ] {
        for streaming in [false, true] {
            let peer = Peer::start(false, None)?;
            let fixture = fixture_with_configuration(
                Channel {
                    adapter,
                    format,
                    path,
                    model,
                    base: "https://api.moonshot.cn/v1",
                },
                b"synthetic-key",
                false,
                |c| {
                    c.route_candidates[0].transform_mode = TransformMode::Passthrough;
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
            let mut body = source(uri, "p12-test-model(high)", streaming);
            body["vendor_option"] = json!({"retained":[true,17]});
            if uri == "/v1/messages" {
                body["output_config"] = json!({"effort":"low","format":{"type":"json_schema","schema":{"type":"object"}},"future_option":17});
            }
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(uri)
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(&body)
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
                let wire = received.last().ok_or("wire")?;
                assert_eq!(wire["model"], model);
                assert_eq!(wire.pointer(pointer).and_then(Value::as_str), Some("high"));
                assert_eq!(wire["vendor_option"], body["vendor_option"]);
                if uri == "/v1/messages" {
                    assert_eq!(
                        wire["output_config"]["format"],
                        body["output_config"]["format"]
                    );
                    assert_eq!(wire["output_config"]["future_option"], 17);
                    assert_eq!(wire["thinking"]["type"], "adaptive");
                    assert!(wire["thinking"].get("budget_tokens").is_none());
                }
            }
            let mut malformed = body;
            if uri == "/v1/messages" {
                malformed["output_config"] = json!({"effort":false});
            } else {
                malformed["reasoning_effort"] = json!(false);
            }
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(uri)
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(malformed)
                    .to_request(),
            )
            .await;
            assert_eq!(reply.status(), StatusCode::BAD_REQUEST);
            let _ = test::read_body(reply).await;
            let mut invalid_native = source(uri, "p12-test-model", streaming);
            if uri == "/v1/messages" {
                invalid_native["output_config"] = json!({"effort":false});
            } else {
                invalid_native["reasoning_effort"] = json!(false);
            }
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(uri)
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(invalid_native)
                    .to_request(),
            )
            .await;
            assert_eq!(reply.status(), StatusCode::BAD_REQUEST);
            let _ = test::read_body(reply).await;
            assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
            peer.server.stop(false).await;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn cpa_69_named_controls_keep_two_tool_cycles_and_followup() -> TestResult {
    for streaming in [false, true] {
        let peer = Peer::start(false, None)?;
        let fixture = fixture_with_configuration(
            Channel {
                adapter: "openai-compatible.responses",
                format: "openai/responses",
                base: "https://api.moonshot.cn/v1",
                path: "/responses",
                model: "gpt-5.5",
            },
            b"synthetic-key",
            false,
            |c| {
                c.model_aliases.push(ModelAliasConfiguration {
                    alias: "tools/model".into(),
                    public_model_id: c.public_models[0].id.clone(),
                });
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
        let mut history = vec![json!({"role":"user","content":"question"})];
        for round in 0..5 {
            if round == 1 || round == 3 {
                let (id, result) = if round == 1 {
                    ("call-c", "result-c")
                } else {
                    ("call-d", "result-d")
                };
                history.push(json!({"role":"assistant","content":[{"type":"tool_use","id":id,"name":"lookup","input":{}}]}));
                history.push(json!({"role":"user","content":[{"type":"tool_result","tool_use_id":id,"content":result}]}));
            } else if round == 2 {
                history.push(json!({"role":"user","content":"next-question"}));
            } else if round == 4 {
                history.push(json!({"role":"user","content":"follow-up"}));
            }
            let body = json!({"model":"tools/model(0)","max_tokens":32000,"stream":streaming,"messages":history,"thinking":{"type":"enabled","budget_tokens":8192},"tools":[{"name":"lookup","input_schema":{"type":"object"}}]});
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/messages")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(&body)
                    .to_request(),
            )
            .await;
            let status = reply.status();
            let bytes = test::read_body(reply).await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{round}: {}",
                String::from_utf8_lossy(&bytes)
            );
            super::super::public_response::assert_round(
                "/v1/messages",
                streaming,
                if round == 4 { 5 } else { round },
                &bytes,
            )?;
            let received = peer.received.lock().map_err(|_| "receiver")?;
            let wire = received.last().ok_or("wire")?;
            assert_eq!(wire["model"], "gpt-5.5");
            assert_eq!(wire["reasoning"]["effort"], "low");
            let text = wire.to_string();
            if round >= 1 {
                assert!(text.contains("call-c") && text.contains("result-c"));
            }
            if round >= 3 {
                assert!(text.contains("call-d") && text.contains("result-d"));
            }
        }
        assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 5);
        assert_eq!(fixture.events.attempts()?, 5);
        peer.server.stop(false).await;
    }
    Ok(())
}

#[actix_web::test]
async fn cpa_69_numeric_suffix_budget_reaches_messages_receiver_unchanged() -> TestResult {
    for model in ["claude-sonnet-4-5-20250929", "claude-sonnet-4-6"] {
        for uri in ["/v1/responses", "/v1/chat/completions"] {
            for streaming in [false, true] {
                let peer = Peer::start(false, None)?;
                let fixture = fixture(
                    Channel {
                        adapter: "anthropic-compatible.messages",
                        format: "anthropic/messages",
                        path: "/messages",
                        base: "https://api.anthropic.com/v1",
                        model,
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
                let body = source(uri, "p12-test-model(4096)", streaming);
                let reply = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(uri)
                        .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                        .set_json(body)
                        .to_request(),
                )
                .await;
                let status = reply.status();
                let bytes = test::read_body(reply).await;
                assert_eq!(
                    status,
                    StatusCode::OK,
                    "{model} {uri}: {}",
                    String::from_utf8_lossy(&bytes)
                );
                {
                    let received = peer.received.lock().map_err(|_| "receiver")?;
                    assert_eq!(received.len(), 1);
                    let wire = received.last().ok_or("wire")?;
                    assert_eq!(wire["thinking"]["type"], "enabled");
                    assert_eq!(wire["thinking"]["budget_tokens"], 4096);
                    assert!(wire["output_config"].get("effort").is_none());
                }
                peer.server.stop(false).await;
            }
        }
    }
    Ok(())
}

#[actix_web::test]
async fn cpa_69_ambiguous_exact_id_stays_out_of_prefix_discovery() -> TestResult {
    let peer = Peer::start(false, None)?;
    let mut group = None;
    let fixture = fixture_with_configuration(
        Channel {
            adapter: "openai-compatible.responses",
            format: "openai/responses",
            base: "https://api.moonshot.cn/v1",
            path: "/responses",
            model: "team/model",
        },
        b"synthetic-key",
        false,
        |config| {
            let mut other = config.public_models[0].clone();
            other.id = gateway_core::PublicModelId::try_new("other-model")?;
            other.model_name = "other-model".into();
            let mut route = config.model_routes[0].clone();
            route.id = RouteId::try_new("other-route")?;
            route.public_model_id = other.id.clone();
            let mut candidate = config.route_candidates[0].clone();
            candidate.id = gateway_core::RouteCandidateId::try_new("other-candidate")?;
            candidate.route_id = route.id.clone();
            let mut grant = config.access_group_routes[0].clone();
            grant.route_id = route.id.clone();
            config.model_aliases.push(ModelAliasConfiguration {
                alias: "team/model".into(),
                public_model_id: config.public_models[0].id.clone(),
            });
            group = Some(config.access_groups[0].id.clone());
            config.public_models.push(other);
            config.model_routes.push(route);
            config.route_candidates.push(candidate);
            config.access_group_routes.push(grant);
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
    let list = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/models")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .to_request(),
    )
    .await;
    let list: Value = test::read_body_json(list).await;
    assert_eq!(list["data"], json!([]));
    let snapshot = fixture.executor.scheduler.snapshot();
    assert!(
        snapshot
            .effective_models_for_access_group(&group.ok_or("group")?)
            .ok_or("models")?
            .is_empty()
    );
    for streaming in [false, true] {
        for name in ["team/model", "team/model(high)"] {
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(source("/v1/responses", name, streaming))
                    .to_request(),
            )
            .await;
            assert_eq!(reply.status(), StatusCode::NOT_FOUND);
            let _ = test::read_body(reply).await;
        }
    }
    assert!(peer.received.lock().map_err(|_| "receiver")?.is_empty());
    assert_eq!(fixture.events.attempts()?, 0);
    peer.server.stop(false).await;
    Ok(())
}
