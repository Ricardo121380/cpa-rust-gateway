//! The new history permission is independent of every existing field and transport qualification.
use super::*;

fn body(streaming: bool) -> Value {
    json!({"model":"p12-test-model","stream":streaming,
        "tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}],
        "input":[{"type":"reasoning","summary":[{"type":"summary_text","text":"retain summary"}],"content":null,"encrypted_content":null},
            {"type":"function_call","id":"history-call-item","call_id":"history-call","name":"lookup","arguments":"{}"},
            {"type":"function_call_output","id":"history-result","call_id":"history-call","output":"seed=41"},
            {"role":"user","content":"continue"}]})
}

#[actix_web::test]
async fn public_build_codex_history_keeps_native_values_and_external_c4() -> TestResult {
    for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire_response(1, streaming)?)?;
        let fixture =
            parallel::fixture_for(namespaces::transport(&peer, size)?, false, false, |p| {
                p.build_input_history_local_controlled = true;
            })?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let mut body = body(streaming);
        let mut metadata = client_metadata::metadata();
        let blob = metadata["x-codex-turn-metadata"]
            .as_str()
            .ok_or("blob")?
            .replace("seatbelt", "external")
            .replace("workspace-write", "external-sandbox");
        metadata["x-codex-turn-metadata"] = json!(blob);
        body["client_metadata"] = metadata.clone();
        let (status, bytes) = namespaces::request(&fixture, &url, &body).await?;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
        let terminal = namespaces::terminal(&bytes, streaming)?;
        assert_eq!(terminal["usage"]["input_tokens"], 20);
        client_metadata::assert_capture(&fixture, &metadata.to_string(), 1)?;
        {
            let received = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(received.len(), 1);
            let mut expected = body["input"].clone();
            expected[3]["type"] = json!("message");
            expected[3]["content"] = json!([{"type":"input_text","text":"continue"}]);
            assert_eq!(received[0].body["input"], expected);
            assert!(received[0].body.get("client_metadata").is_none());
        }
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        namespaces::assert_recorded(&database, 1).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_codex_history_invalid_and_unqualified_send_zero() -> TestResult {
    for qualified in [false, true] {
        let peer = ModelPeer::start_wire(false, tool_rounds::wire_response(1, false)?)?;
        let fixture = parallel::fixture_for(namespaces::transport(&peer, 7)?, false, true, |p| {
            p.build_input_history_local_controlled = qualified;
            p.build_input_message_ids_local_controlled = true;
            p.build_namespaces_local_controlled = true;
            p.build_search_local_controlled = true;
            p.build_parallel_local_controlled = true;
        })?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let base = body(false);
        let mut cases = Vec::new();
        if !qualified {
            cases.push(base.clone());
        }
        for id in [
            Value::Null,
            json!(1),
            json!(""),
            json!("a b"),
            json!("非ASCII"),
            json!("x".repeat(513)),
            json!("history-call-item"),
        ] {
            let mut invalid = base.clone();
            invalid["input"][2]["id"] = id;
            cases.push(invalid);
        }
        for (index, key, value) in [
            (0, "id", Value::Null),
            (0, "content", json!([])),
            (0, "encrypted_content", json!("ciphertext")),
            (0, "unknown", json!(true)),
            (2, "unknown", json!(true)),
            (2, "call_id", json!("wrong-call")),
        ] {
            let mut invalid = base.clone();
            invalid["input"][index][key] = value;
            cases.push(invalid);
        }
        for pair in [
            json!({"sandbox":"external"}),
            json!({"sandbox_mode":"external-sandbox"}),
            json!({"sandbox":"seatbelt","sandbox_mode":"external-sandbox"}),
            json!({"sandbox":"external","sandbox_mode":"workspace-write"}),
        ] {
            let mut invalid = base.clone();
            invalid["client_metadata"] = json!({"x-codex-turn-metadata":pair.to_string()});
            cases.push(invalid);
        }
        for invalid in cases {
            let reply = reqwest::Client::builder()
                .no_proxy()
                .build()?
                .post(&url)
                .bearer_auth(&fixture.key)
                .header("user-agent", "codex-history-qualified")
                .header("x-cpar-input-history-qualified", "true")
                .header("content-type", "application/json")
                .body(invalid.to_string())
                .send()
                .await?;
            assert_eq!(reply.status().as_u16(), 400, "{}", reply.text().await?);
            assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
            assert_eq!(fixture.events.attempts()?, 0);
            assert_eq!(active_leases(&fixture), 0);
        }
        server.stop(false).await;
        recording.finish(&fixture, 0).await?;
    }
    Ok(())
}
