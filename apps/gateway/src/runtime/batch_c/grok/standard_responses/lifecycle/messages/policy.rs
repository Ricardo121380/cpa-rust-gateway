//! The limited policy does not confer exact/lossless semantics or leak to other paths.
use super::*;

#[actix_web::test]
async fn public_build_messages_nonqualified_modes_reject_before_send() -> TestResult {
    for streaming in [false, true] {
        for mode in [
            TransformMode::Passthrough,
            TransformMode::LosslessBridge,
            TransformMode::Canonical,
        ] {
            let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
            let transport = MessagesTransport::new(&peer, 7)?;
            let (channel, secret) = native_channels()[0];
            let fixture = fixture_with_configuration(
                channel,
                secret.as_bytes(),
                false,
                |configuration| {
                    configuration.public_models[0].model_name = channel.model.into();
                    configuration.model_aliases.push(
                        gateway_store::control_plane::ModelAliasConfiguration {
                            alias: "p12-test-model".into(),
                            public_model_id: configuration.public_models[0].id.clone(),
                        },
                    );
                    configuration.route_candidates[0].transform_mode = mode;
                    Ok(())
                },
                |endpoints| {
                    let mut profiles = P12TransportProfiles::try_new()?;
                    profiles.build = Some(transport);
                    endpoints.values_mut().next().ok_or("endpoint")?.transports =
                        Arc::new(profiles);
                    Ok(())
                },
            )?;
            let recording = Recording::start(&fixture, false)?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let reply=test::call_service(&app,test::TestRequest::post().uri("/v1/messages")
                .insert_header(("authorization",format!("Bearer {}",fixture.key)))
                .insert_header(("user-agent","Claude Code"))
                .set_json(json!({"model":"p12-test-model","max_tokens":19,"stream":streaming,"messages":[{"role":"user","content":"question"}]})).to_request()).await;
            let status = reply.status();
            let bytes = test::read_body(reply).await;
            assert_eq!(
                status,
                if mode == TransformMode::Canonical {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                "mode={mode:?} {}",
                String::from_utf8_lossy(&bytes)
            );
            assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 0);
            drop(app);
            let database = recording.finish(&fixture, 0).await?;
            assert!(
                !SqliteEventStore::open_read_only(database)?
                    .list_events()?
                    .iter()
                    .any(|r| matches!(
                        r.event(),
                        GatewayEvent::Attempt(_) | GatewayEvent::Usage(_)
                    ))
            );
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_policy_preserves_responses_and_other_adapters() -> TestResult {
    for streaming in [false, true] {
        for responses in [false, true] {
            let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
            let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
            let recording = Recording::start(&fixture, false)?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let (uri, body) = if responses {
                (
                    "/v1/responses",
                    json!({"model":"p12-test-model","input":"question","stream":streaming}),
                )
            } else {
                (
                    "/v1/chat/completions",
                    json!({"model":"p12-test-model","messages":[{"role":"user","content":"question"}],"stream":streaming}),
                )
            };
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
            let text = String::from_utf8_lossy(&bytes);
            assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
            if responses {
                assert_eq!(status, StatusCode::OK, "{text}");
                assert!(text.contains("计划 1"));
                assert!(text.contains("reason-b-1"));
                if streaming {
                    assert_eq!(
                        public_frames(&bytes)?
                            .iter()
                            .filter(|v| v["type"] == "response.completed")
                            .count(),
                        1
                    );
                } else {
                    assert_eq!(
                        serde_json::from_slice::<Value>(&bytes)?["output"][0]["summary"][0]["text"],
                        "计划 1"
                    );
                }
            } else {
                assert!(!text.contains("计划"));
                if streaming {
                    assert!(!text.contains("[DONE]"));
                    assert!(text.contains("error"));
                } else {
                    assert_eq!(status, StatusCode::BAD_GATEWAY);
                }
            }
            let connection = gateway_store::open(&recording.database)?;
            assert_eq!(
                connection.query_row(
                    "SELECT count(*) FROM messages_summary_evidence",
                    [],
                    |r| r.get::<_, i64>(0)
                )?,
                0
            );
            drop(app);
            recording.finish(&fixture, 0).await?;
        }
        for (channel, secret) in native_channels().into_iter().skip(1) {
            let peer = Arc::new(Peer {
                native_output: Some(
                    json!([{"id":"reason-other","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"OTHER_PRIVATE_SUMMARY"}]}]),
                ),
                ..Peer::default()
            });
            let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
                install_peer(endpoint, channel.adapter, &peer)
            })?;
            let recording = Recording::start(&fixture, false)?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let reply=test::call_service(&app,test::TestRequest::post().uri("/v1/messages").insert_header(("authorization",format!("Bearer {}",fixture.key)))
                .set_json(json!({"model":"p12-test-model","max_tokens":19,"stream":streaming,"messages":[{"role":"user","content":"question"}]})).to_request()).await;
            let status = reply.status();
            let bytes = test::read_body(reply).await;
            assert_eq!(peer.bodies.lock().map_err(|_| "peer")?.len(), 1);
            assert!(!String::from_utf8_lossy(&bytes).contains("OTHER_PRIVATE_SUMMARY"));
            if streaming {
                assert_eq!(status, StatusCode::OK);
                failures::assert_failed(&bytes)?;
            } else {
                assert_eq!(status, StatusCode::BAD_GATEWAY);
            }
            drop(app);
            recording.finish(&fixture, 0).await?;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_body_only_keeps_existing_strict_projection() -> TestResult {
    for streaming in [false, true] {
        let mut value = response();
        value["output"] = json!([
            {"id":"body-only","type":"reasoning","status":"completed","summary":[],"content":[{"type":"reasoning_text","text":"actual body"}]},
            {"id":"message-only","type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"answer","annotations":[]}]}]);
        value["usage"]["input_tokens_details"] = json!({"cached_tokens":0});
        let bytes = if streaming {
            let mut wire = frame("response.created", &created());
            for (index, item) in value["output"]
                .as_array()
                .ok_or("output")?
                .iter()
                .enumerate()
            {
                let mut added = item.clone();
                added["status"] = json!("in_progress");
                added["content"] = json!([]);
                wire += &native_event(
                    "response.output_item.added",
                    json!({"output_index":index,"item":added}),
                );
                wire += &native_event(
                    "response.output_item.done",
                    json!({"output_index":index,"item":item}),
                );
            }
            wire + &frame("response.completed", &value)
        } else {
            value.to_string()
        };
        let peer = ModelPeer::start_wire(streaming, bytes)?;
        let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
        let recording = Recording::start(&fixture, false)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let reply=test::call_service(&app,test::TestRequest::post().uri("/v1/messages").insert_header(("authorization",format!("Bearer {}",fixture.key)))
            .set_json(json!({"model":"p12-test-model","max_tokens":19,"stream":streaming,"messages":[{"role":"user","content":"question"}]})).to_request()).await;
        let status = reply.status();
        let bytes = test::read_body(reply).await;
        let text = String::from_utf8_lossy(&bytes);
        assert_eq!(status, StatusCode::OK, "{text}");
        assert!(
            text.contains("actual body") && text.contains("thinking"),
            "streaming={streaming}: {text}"
        );
        if streaming {
            assert_eq!(
                public_frames(&bytes)?
                    .iter()
                    .filter(|v| v["type"] == "message_stop")
                    .count(),
                1
            );
        }
        assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
        assert_eq!(
            gateway_store::open(&recording.database)?.query_row(
                "SELECT count(*) FROM messages_summary_evidence",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            0
        );
        drop(app);
        recording.finish(&fixture, 0).await?;
    }
    Ok(())
}
