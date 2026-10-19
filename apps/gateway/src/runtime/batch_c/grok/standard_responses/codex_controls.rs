//! Field-isolated, synthetic derivatives of the observed Codex request, never CLI acceptance.
use super::*;

fn control_fixture(peer: Arc<ModelPeer>) -> Result<Fixture, Box<dyn Error>> {
    let (channel, secret) = native_channels()[0];
    fixture(channel, secret.as_bytes(), |endpoint| {
        let mut profiles = P12TransportProfiles::try_new()?;
        profiles.build = Some(peer);
        endpoint.transports = Arc::new(profiles);
        Ok(())
    })
}

#[actix_web::test]
async fn public_build_summary_only_reasoning_preserves_absent_effort() -> TestResult {
    for streaming in [false, true] {
        let peer = Arc::new(ModelPeer::start_wire(
            streaming,
            tool_rounds::wire_response(1, streaming)?,
        )?);
        let fixture = control_fixture(peer.clone())?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let reply = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(
                    json!({"model":"p12-test-model","input":"field-isolated derivative",
                    "stream":streaming,"reasoning":{"summary":"auto"}}),
                )
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
        let terminal = if streaming {
            let frames = public_frames(&bytes)?;
            assert_eq!(
                frames
                    .iter()
                    .filter(|v| v["type"] == "response.completed")
                    .count(),
                1
            );
            assert_eq!(
                frames
                    .iter()
                    .filter(|v| v["type"] == "response.failed")
                    .count(),
                0
            );
            frames.last().ok_or("terminal")?["response"].clone()
        } else {
            serde_json::from_slice::<Value>(&bytes)?
        };
        assert_eq!(terminal["output"][0]["summary"][0]["text"], "计划 1");
        assert_eq!(terminal["usage"]["input_tokens"], 20);
        assert_eq!(terminal["usage"]["output_tokens"], 7);
        let requests = peer.requests.lock().map_err(|_| "receiver")?;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["reasoning"], json!({"summary":"auto"}));
        assert!(requests[0].body["reasoning"].get("effort").is_none());
        assert_eq!(fixture.events.attempts()?, 1);
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_codex_fields_remain_individually_blocked_before_send() -> TestResult {
    let function = json!({"type":"function","name":"lookup","parameters":{"type":"object"}});
    let namespace = json!({"type":"namespace","name":"multi_agent_v1","description":"synthetic",
        "tools":[function.clone()]});
    let call = json!({"type":"function_call","call_id":"call-62","name":"lookup","arguments":"{}"});
    let result =
        json!({"type":"function_call_output","call_id":"call-62","output":"synthetic result"});
    let mut namespaced_call = call.clone();
    namespaced_call["namespace"] = json!("multi_agent_v1");
    let mut namespaced_result = result.clone();
    namespaced_result["namespace"] = json!("multi_agent_v1");
    let mut other_namespace = namespace.clone();
    other_namespace["name"] = json!("another_namespace");
    for streaming in [false, true] {
        for (label, controls) in [
            ("namespace", json!({"tools":[namespace.clone()]})),
            (
                "namespace-serial",
                json!({"tools":[namespace.clone()],"parallel_tool_calls":false}),
            ),
            (
                "namespace-parallel",
                json!({"tools":[namespace.clone()],"parallel_tool_calls":true}),
            ),
            (
                "namespace-name-overlap",
                json!({"tools":[function.clone(),namespace.clone(),other_namespace.clone()]}),
            ),
            (
                "namespace-choice",
                json!({"tools":[function.clone()],"tool_choice":{"type":"function","name":"lookup","namespace":"multi_agent_v1"}}),
            ),
            (
                "namespace-call-history",
                json!({"tools":[function.clone()],"input":[{"role":"user","content":"question"},namespaced_call.clone(),result.clone()]}),
            ),
            (
                "namespace-result-history",
                json!({"tools":[function.clone()],"input":[{"role":"user","content":"question"},call.clone(),namespaced_result.clone()]}),
            ),
            (
                "web-search-cache-only",
                json!({"tools":[{"type":"web_search","external_web_access":false}]}),
            ),
            (
                "web-search-live",
                json!({"tools":[{"type":"web_search","external_web_access":true}]}),
            ),
            (
                "web-search-default",
                json!({"tools":[{"type":"web_search"}]}),
            ),
            (
                "client-metadata",
                json!({"client_metadata":{"root_turn_id":"synthetic-root","session_id":"synthetic-session","thread_id":"synthetic-thread","turn_id":"synthetic-turn","x-codex-installation-id":"synthetic-installation","x-codex-window-id":"synthetic-window","x-codex-turn-metadata":"{\"request_kind\":\"agent\",\"model\":\"synthetic-model\"}"}}),
            ),
            (
                "client-metadata-nested",
                json!({"client_metadata":{"nested":{"unqualified":true}}}),
            ),
        ] {
            let peer = Arc::new(ModelPeer::start(streaming, response())?);
            let fixture = control_fixture(peer.clone())?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let mut body = json!({"model":"p12-test-model","input":"field-isolated derivative","stream":streaming});
            body.as_object_mut()
                .ok_or("body")?
                .extend(controls.as_object().ok_or("controls")?.clone());
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert_eq!(reply.status(), StatusCode::BAD_REQUEST, "{label}");
            let bytes = test::read_body(reply).await;
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes)?["error"]["code"],
                "ClientRequestError",
                "{label}"
            );
            assert_eq!(
                peer.requests.lock().map_err(|_| "receiver")?.len(),
                0,
                "{label}"
            );
            assert_eq!(fixture.events.attempts()?, 0, "{label}");
            assert!(
                fixture
                    .events
                    .0
                    .lock()
                    .map_err(|_| "events")?
                    .iter()
                    .all(|v| !matches!(v, GatewayEvent::Usage(_))),
                "{label}"
            );
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_summary_only_invalid_controls_fail_before_send() -> TestResult {
    for reasoning in [
        json!({}),
        json!(null),
        json!({"summary":null}),
        json!({"summary":true}),
        json!({"summary":"unknown"}),
        json!({"summary":"auto","effort":null}),
        json!({"summary":"auto","effort":""}),
        json!({"summary":"auto","context":"all_turns"}),
    ] {
        let peer = Arc::new(ModelPeer::start(false, response())?);
        let fixture = control_fixture(peer.clone())?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let reply = test::call_service(&app,test::TestRequest::post().uri("/v1/responses")
            .insert_header(("authorization",format!("Bearer {}",fixture.key)))
            .set_json(json!({"model":"p12-test-model","input":"invalid control","reasoning":reasoning}))
            .to_request()).await;
        assert_eq!(reply.status(), StatusCode::BAD_REQUEST, "{reasoning}");
        assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 0);
        assert_eq!(fixture.events.attempts()?, 0);
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_namespaced_output_is_rejected_without_flattening_or_retry() -> TestResult {
    for streaming in [false, true] {
        let wire = tool_rounds::wire_response(0, streaming)?.replace(
            "\"name\":\"lookup\"",
            "\"namespace\":\"multi_agent_v1\",\"name\":\"lookup\"",
        );
        let peer = Arc::new(ModelPeer::start_wire(streaming, wire)?);
        let fixture = control_fixture(peer.clone())?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let reply = test::call_service(&app,test::TestRequest::post().uri("/v1/responses")
            .insert_header(("authorization",format!("Bearer {}",fixture.key)))
            .set_json(json!({"model":"p12-test-model","input":"output identity negative","stream":streaming,
                "tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]}))
            .to_request()).await;
        let status = reply.status();
        let bytes = test::read_body(reply).await;
        if streaming {
            assert_eq!(status, StatusCode::OK);
            let frames = public_frames(&bytes)?;
            assert_eq!(
                frames
                    .iter()
                    .filter(|v| v["type"] == "response.failed")
                    .count(),
                1
            );
            assert!(frames.iter().all(|v| v["type"] != "response.completed"
                && v["type"] != "response.function_call_arguments.done"));
        } else {
            assert_eq!(status, StatusCode::BAD_GATEWAY);
        }
        assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
        assert_eq!(fixture.events.attempts()?, 1);
    }
    Ok(())
}
