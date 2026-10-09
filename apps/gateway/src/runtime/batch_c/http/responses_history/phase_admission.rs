//! O-01: ordinary Responses phase is content; paired turn IDs are a private wire envelope.
use super::*;

fn stored_fixture(peer: &Arc<Peer>) -> Result<Fixture, Box<dyn Error>> {
    fixture_with_storage(
        Channel {
            adapter: "openai-compatible.responses",
            format: "openai/responses",
            base: "https://gateway.example.test/v1",
            path: "/responses",
            model: "upstream-c",
        },
        b"synthetic-key",
        true,
        |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.http = Some(peer.clone());
            endpoint.transports = Arc::new(profiles);
            endpoint.resolver = Arc::new(PublicResolver);
            Ok(())
        },
    )
}

async fn assert_phase_recovery(streaming: bool) -> TestResult {
    for (phase, paired) in [
        ("commentary", false),
        ("final_answer", false),
        ("final_answer", true),
    ] {
        let mut wire = multipart_output();
        wire[0]["phase"] = json!(phase);
        let expected = wire.clone();
        if paired {
            wire[0]["metadata"] = json!({"turn_id":"original-turn"});
            wire[0]["internal_chat_message_metadata_passthrough"] =
                json!({"turn_id":"original-turn"});
        }
        let peer = Peer::start(false, Some(wire))?;
        let fixture = stored_fixture(&peer)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let response = test::call_service(&app, test::TestRequest::post().uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(json!({"model":"p12-test-model","input":"question","stream":streaming,"store":true}))
            .to_request()).await;
        let status = response.status();
        let bytes = test::read_body(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "phase={phase} paired={paired}: {}",
            String::from_utf8_lossy(&bytes)
        );
        assert_eq!(
            completed_response(&bytes, streaming, &expected)?["output"],
            expected
        );
        let get = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/responses/resp-b01")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .to_request(),
        )
        .await;
        assert_eq!(get.status(), StatusCode::OK);
        let saved: Value = serde_json::from_slice(&test::read_body(get).await)?;
        assert_eq!(saved["output"], expected);
        let continuation = test::call_service(&app, test::TestRequest::post().uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(json!({"model":"p12-test-model","input":"next-question","stream":streaming,"previous_response_id":"resp-b01"}))
            .to_request()).await;
        let status = continuation.status();
        let bytes = test::read_body(continuation).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        assert_eq!(
            completed_response(&bytes, streaming, &expected)?["output"],
            expected
        );
        let received = peer.received.lock().map_err(|_| "received")?;
        assert_eq!(received.len(), 2);
        let native = received[1]["input"]
            .as_array()
            .ok_or("history")?
            .iter()
            .filter(|item| item.get("id").is_some())
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            json!(native),
            expected,
            "phase and ordered parts in stored history"
        );
        assert_eq!(peer.requests.lock().map_err(|_| "requests")?.len(), 2);
        eprintln!(
            "O-01 stream={streaming} phase={phase} paired={paired}: public output, GET, exact history, receives=2"
        );
    }
    Ok(())
}

#[actix_web::test]
async fn ordinary_responses_phase_json_survives_output_storage_and_history() -> TestResult {
    assert_phase_recovery(false).await
}

#[actix_web::test]
async fn ordinary_responses_phase_sse_survives_output_storage_and_history() -> TestResult {
    assert_phase_recovery(true).await
}

#[actix_web::test]
async fn ordinary_responses_phase_rejects_invalid_private_metadata_without_storage_or_retry()
-> TestResult {
    let mut cases = Vec::new();
    for (label, public, internal, phase) in [
        (
            "missing internal",
            json!({"turn_id":"turn"}),
            Value::Null,
            "final_answer",
        ),
        (
            "missing public",
            Value::Null,
            json!({"turn_id":"turn"}),
            "final_answer",
        ),
        (
            "conflicting turn",
            json!({"turn_id":"turn"}),
            json!({"turn_id":"other"}),
            "final_answer",
        ),
        (
            "unknown private field",
            json!({"turn_id":"turn","extra":true}),
            json!({"turn_id":"turn"}),
            "final_answer",
        ),
        (
            "empty identity",
            json!({"turn_id":""}),
            json!({"turn_id":""}),
            "final_answer",
        ),
        (
            "unproven paired phase",
            json!({"turn_id":"turn"}),
            json!({"turn_id":"turn"}),
            "commentary",
        ),
    ] {
        let mut output = multipart_output();
        output[0]["phase"] = json!(phase);
        if !public.is_null() {
            output[0]["metadata"] = public;
        }
        if !internal.is_null() {
            output[0]["internal_chat_message_metadata_passthrough"] = internal;
        }
        cases.push((label, output));
    }
    for (label, field, value) in [
        ("illegal phase", "phase", json!("analysis")),
        ("unknown output field", "unreviewed", json!(true)),
    ] {
        let mut output = multipart_output();
        output[0][field] = value;
        cases.push((label, output));
    }
    for (label, output) in cases {
        for streaming in [false, true] {
            let peer = Peer::start(false, Some(output.clone()))?;
            let fixture = stored_fixture(&peer)?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let response = test::call_service(&app, test::TestRequest::post().uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(json!({"model":"p12-test-model","input":"question","stream":streaming,"store":true}))
                .to_request()).await;
            let status = response.status();
            let bytes = test::read_body(response).await;
            let text = String::from_utf8_lossy(&bytes);
            if status.is_success() {
                assert!(
                    text.contains("response.failed"),
                    "{label} stream={streaming}: {text}"
                );
                assert!(!text.contains("response.completed"));
            }
            let get = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri("/v1/responses/resp-b01")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .to_request(),
            )
            .await;
            assert_eq!(
                get.status(),
                StatusCode::NOT_FOUND,
                "{label} stream={streaming}"
            );
            let continuation = test::call_service(&app, test::TestRequest::post().uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(json!({"model":"p12-test-model","input":"next","previous_response_id":"resp-b01"}))
                .to_request()).await;
            assert!(!continuation.status().is_success(), "{label}");
            assert_eq!(
                peer.received.lock().map_err(|_| "received")?.len(),
                1,
                "{label}"
            );
            assert_eq!(fixture.events.attempts()?, 1, "{label}");
            eprintln!(
                "O-01 rejected {label} stream={streaming}: no root, continuation receive count=1"
            );
        }
    }
    Ok(())
}
