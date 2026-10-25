//! Standalone protocol probes; these do not edit or qualify an actual Claude request.
use super::*;

fn request(streaming: bool) -> Value {
    json!({"model":"grok-4.5","max_tokens":32000,"stream":streaming,
        "messages":[{"role":"user","content":"synthetic control probe"}]})
}

#[actix_web::test]
async fn public_build_messages_cpa_effort_without_thinking_preserves_explicit_effort() -> TestResult
{
    for streaming in [false, true] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
        let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let mut body = request(streaming);
        body["output_config"] = json!({"effort":"high"});
        body["context_management"] = local_controls::keep_all();
        let (status, id, bytes) = local_controls::send(
            &fixture,
            &url,
            local_controls::with_metadata(&body, local_controls::METADATA),
        )
        .await?;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
        tool_rounds::client_result(&bytes, streaming)?;
        local_controls::assert_metadata(&fixture, &id, local_controls::METADATA, 1)?;
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
        {
            let received = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(received.len(), 1);
            assert!(received[0].body.get("output_config").is_none());
            assert_eq!(received[0].body["reasoning"], json!({"effort":"high"}));
        }
        assert_eq!(active_leases(&fixture), 0);
        server.stop(false).await;
        accounting(&recording.finish(&fixture, 0).await?, 1).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_cpa_compatibility_preserves_independent_rejections() -> TestResult {
    for streaming in [false, true] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
        let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        for (key, value) in [
            ("thinking", json!({"type":"invalid"})),
            (
                "output_config",
                json!({"effort":"high","unknown":"private"}),
            ),
            ("output_config", json!({"effort":null})),
            ("context_management", json!({"edits":[]})),
            (
                "messages",
                json!([{"role":"user","content":[{"type":"tool_result","tool_use_id":"missing","content":"result"}]}]),
            ),
        ] {
            let mut body = request(streaming);
            body["output_config"] = json!({"effort":"high"});
            body[key] = value;
            let (status, _, bytes) = local_controls::send(
                &fixture,
                &url,
                local_controls::with_metadata(&body, local_controls::METADATA),
            )
            .await?;
            assert_eq!(status, 400);
            let error: Value = serde_json::from_slice(&bytes)?;
            assert_eq!(error["error"]["message"], "the client request is invalid");
            assert!(!String::from_utf8_lossy(&bytes).contains("private"));
        }
        let reply=reqwest::Client::builder().no_proxy().build()?.post(url.replace("/v1/responses","/v1/messages"))
            .bearer_auth("synthetic-wrong-key").header("content-type","application/json").body(json!({"model":"grok-4.5","max_tokens":19,"messages":[{"role":"user","content":"question"}],"output_config":{"effort":"high"}}).to_string()).send().await?;
        assert_eq!(reply.status(), reqwest::StatusCode::UNAUTHORIZED);
        assert!(!reply.text().await?.contains("output_config.effort"));
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
        assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
        assert_eq!(active_leases(&fixture), 0);
        server.stop(false).await;
        rejected_accounting(&recording.finish(&fixture, 0).await?).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_missing_runtime_is_not_an_effort_rejection() -> TestResult {
    let peer = ModelPeer::start_wire(false, tool_rounds::wire(1, false)?)?;
    let (channel, secret) = native_channels()[0];
    let fixture = fixture_with_configuration(
        channel,
        secret.as_bytes(),
        false,
        |config| {
            config.public_models[0].model_name = "grok-4.5".into();
            Ok(())
        },
        |endpoints| {
            endpoints.clear();
            Ok(())
        },
    )?;
    let (url, server) = namespaces::listener(&fixture)?;
    let mut body = request(false);
    body["output_config"] = json!({"effort":"high"});
    let (status, _, bytes) = local_controls::send(&fixture, &url, body.to_string()).await?;
    assert_eq!(status, 503);
    assert!(!String::from_utf8_lossy(&bytes).contains("output_config.effort"));
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.events.attempts()?, 0);
    server.stop(false).await;
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_cpa_controls_reach_receiver() -> TestResult {
    for streaming in [false, true] {
        for (control, expected) in [
            (json!({"type":"adaptive"}), json!({"effort":"xhigh"})),
            (
                json!({"type":"adaptive","display":"updates"}),
                json!({"effort":"xhigh"}),
            ),
            (
                json!({"type":"adaptive","display":"summarized"}),
                json!({"effort":"high","summary":"auto"}),
            ),
            (
                json!({"type":"enabled","budget_tokens":1024}),
                json!({"effort":"low"}),
            ),
            (json!({"type":"disabled"}), json!({"effort":"none"})),
        ] {
            let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
            let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
            let recording = Recording::start(&fixture, false)?;
            let (url, server) = namespaces::listener(&fixture)?;
            let mut body = request(streaming);
            body["thinking"] = control;
            if body["thinking"]["display"] == "summarized" {
                body["output_config"] = json!({"effort":"high"});
            }
            body["top_k"] = json!(3);
            body["vendor_option"] = json!({"future":true});
            body["system"] = json!([{"type":"text","text":"system","cache_control":{"type":"ephemeral","ttl":"1h"}}]);
            body["messages"][0]["content"] =
                json!([{"type":"text","text":"question","cache_control":{"type":"ephemeral"}}]);
            let (status, _, bytes) = local_controls::send(&fixture, &url, body.to_string()).await?;
            assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
            tool_rounds::client_result(&bytes, streaming)?;
            {
                let received = peer.requests.lock().map_err(|_| "receiver")?;
                assert_eq!(received.len(), 1);
                assert_eq!(received[0].body["reasoning"], expected);
                assert!(received[0].body.get("vendor_option").is_none());
                assert!(received[0].body.get("top_k").is_none());
                assert!(!received[0].body.to_string().contains("cache_control"));
            }
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
            assert_eq!(active_leases(&fixture), 0);
            server.stop(false).await;
            accounting(&recording.finish(&fixture, 0).await?, 1).await?;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_claude_limit_and_exact_tool_controls_reach_build() -> TestResult {
    for streaming in [false, true] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(0, streaming)?)?;
        let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
        let recording = Recording::start(&fixture, false)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let mut body = request(streaming);
        body["tools"] = json!([{"name":"lookup","input_schema":{"type":"object"}}]);
        body["tool_choice"] =
            json!({"type":"tool","name":"lookup","disable_parallel_tool_use":true});
        let reply = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/messages")
                .insert_header(("x-api-key", fixture.key.clone()))
                .set_json(body)
                .to_request(),
        )
        .await;
        let request_id = reply
            .headers()
            .get("x-request-id")
            .ok_or("request id")?
            .to_str()?
            .to_owned();
        assert_eq!(reply.status(), StatusCode::OK);
        let bytes = test::read_body(reply).await;
        let public = tool_rounds::client_result(&bytes, streaming)?;
        assert_eq!(public["content"][0]["name"], "lookup");
        let record = retained(&recording.database, &request_id)?;
        assert_eq!(record.evidence.items().len(), 2);
        let native: Value = serde_json::from_str(&tool_rounds::wire(0, false)?)?;
        for entry in record.evidence.items() {
            assert_eq!(
                serde_json::from_str::<Value>(entry.item.get())?,
                native["output"][entry.output_index]
            );
        }
        {
            let received = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
            assert_eq!(received.len(), 1);
            assert_eq!(received[0].body["max_output_tokens"], 32000);
            assert_eq!(
                received[0].body["tool_choice"],
                json!({"type":"function","name":"lookup"})
            );
            assert_eq!(received[0].body["parallel_tool_calls"], false);
            eprintln!(
                "CPAR64_PROTOCOL_CONTROL {}",
                json!({"actual_client":false,"streaming":streaming,
            "request_id":request_id,"attempt_id":record.attempt.attempt_id(),"owner":record.client_key_id,
            "max_output_tokens":received[0].body["max_output_tokens"],"parallel_tool_calls":false,
            "tool_choice":received[0].body["tool_choice"],"independent_accepts":1,"independent_receives":1,
            "public_model":public["model"],"native_model":received[0].body["model"],
            "model_override":received[0].model_override,"summary_items":record.evidence.items().len(),
            "summary_readback_matches_native":true,"known_input":public["usage"]["input_tokens"],
            "known_output":public["usage"]["output_tokens"],"cost":"UNKNOWN"})
            );
        }
        drop(app);
        let database = recording.finish(&fixture, 0).await?;
        accounting(&database, 1).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn messages_passive_capture_preserves_bytes_query_headers_and_rejection() -> TestResult {
    let peer = ModelPeer::start_wire(false, tool_rounds::wire(1, false)?)?;
    let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
    let ingress = web::Data::new(Mutex::new(Vec::<Captured>::new()));
    let app = test::init_service(
        App::new()
            .wrap(from_fn(capture))
            .app_data(ingress.clone())
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let bytes = b" { \"max_tokens\":32000, \"model\":\"grok-4.5\", \"model\":\"grok-4.5\", \"messages\":[{\"role\":\"user\",\"content\":\"probe\"}] } \n";
    let reply = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages?beta=true&probe=a%2Fb")
            .insert_header(("x-api-key", fixture.key.clone()))
            .append_header(("anthropic-beta", "one"))
            .append_header(("anthropic-beta", "two"))
            .insert_header(("anthropic-version", "2023-06-01"))
            .set_payload(bytes.as_slice())
            .to_request(),
    )
    .await;
    assert_eq!(reply.status(), StatusCode::BAD_REQUEST);
    let response = test::read_body(reply).await;
    let rows = ingress.lock().map_err(|_| "capture")?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].request_bytes, bytes);
    assert_eq!(rows[0].method, "POST");
    assert_eq!(rows[0].uri, "/v1/messages?beta=true&probe=a%2Fb");
    assert_eq!(
        rows[0].headers["anthropic-beta"],
        json!([b"one".to_vec(), b"two".to_vec()])
    );
    assert_eq!(rows[0].headers["x-api-key_count"], 1);
    assert!(!rows[0].headers.to_string().contains(&fixture.key));
    assert_eq!(rows[0].wire, response);
    assert!(!rows[0].wire_truncated && rows[0].body_complete);
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
    assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
    Ok(())
}
