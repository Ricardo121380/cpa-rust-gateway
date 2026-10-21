//! Accurate public refusals with independent receiver and durable event/ledger observations.
use super::*;

fn body(streaming: bool) -> Value {
    json!({"model":"p12-test-model","input":inputs(),"stream":streaming})
}

fn invalid_inputs(streaming: bool) -> Vec<(String, String)> {
    let mut cases = Vec::new();
    for index in 0..3 {
        for value in [
            Value::Null,
            json!(false),
            json!(1),
            json!({}),
            json!([]),
            json!(""),
            json!("x".repeat(513)),
            json!("has space"),
            json!("newline\n"),
            json!("nul\0"),
            json!("非ASCII"),
            json!("\u{7f}"),
        ] {
            let mut v = body(streaming);
            v["input"][index]["id"] = value;
            cases.push((format!("bad-id-{index}-{}", cases.len()), v.to_string()));
        }
        for field in ["status", "phase", "unknown"] {
            let mut v = body(streaming);
            v["input"][index][field] = json!(if field == "status" {
                "completed"
            } else {
                "commentary"
            });
            cases.push((format!("extra-{index}-{field}"), v.to_string()));
        }
        let role = ["developer", "user", "system"][index];
        for keys in [
            r#""id":"a","id":"a""#,
            r#""id":"a","id":"b""#,
            r#""id":"a","\u0069d":"a""#,
        ] {
            cases.push((format!("duplicate-json-{index}-{}",cases.len()),
                format!(r#"{{"model":"p12-test-model","stream":{streaming},"input":[{{"role":"{role}","content":"text",{keys}}}]}}"#)));
        }
    }
    for label in [
        "duplicate-equal",
        "duplicate-content",
        "duplicate-role",
        "empty-content",
        "nested-extension",
        "item-reference",
        "c6-cannot-grant-namespace",
        "c6-cannot-grant-search",
        "c6-cannot-grant-parallel",
        "request-cannot-qualify",
    ] {
        let mut v = body(streaming);
        match label {
            "duplicate-equal" => v["input"][1] = v["input"][0].clone(),
            "duplicate-content" => v["input"][1]["id"] = v["input"][0]["id"].clone(),
            "duplicate-role" => {
                v["input"][1] = v["input"][0].clone();
                v["input"][1]["role"] = json!("user");
            }
            "empty-content" => v["input"][0]["content"] = json!([]),
            "nested-extension" => v["input"][0]["content"][0]["unknown"] = json!(true),
            "item-reference" => {
                v["input"] = json!([{"type":"item_reference","id":"dev-C6:Original_01"}]);
            }
            "c6-cannot-grant-namespace" => {
                v["tools"] = json!([{"type":"namespace","name":"n","description":"group","tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]}]);
            }
            "c6-cannot-grant-search" => {
                v["tools"] = json!([{"type":"web_search","external_web_access":false}]);
            }
            "c6-cannot-grant-parallel" => v["parallel_tool_calls"] = json!(true),
            "request-cannot-qualify" => v["c6_input_message_ids_qualified"] = json!(true),
            _ => unreachable!(),
        }
        cases.push((label.into(), v.to_string()));
    }
    cases
}

#[actix_web::test]
async fn public_build_c6_invalid_and_unqualified_requests_send_zero() -> TestResult {
    for streaming in [false, true] {
        for qualified in [false, true] {
            let peer = ModelPeer::start_wire(streaming, tool_rounds::wire_response(1, streaming)?)?;
            let fixture = fixture_for(namespaces::transport(&peer, 7)?, false, |p| {
                p.build_input_message_ids_local_controlled = qualified;
            })?;
            let recording = Recording::start(&fixture, false)?;
            let (url, server) = namespaces::listener(&fixture)?;
            let mut cases = invalid_inputs(streaming);
            if !qualified {
                cases.push(("valid-unqualified".into(), body(streaming).to_string()));
            }
            for (label, raw) in cases {
                let reply = reqwest::Client::builder()
                    .no_proxy()
                    .build()?
                    .post(&url)
                    .bearer_auth(&fixture.key)
                    .header("content-type", "application/json")
                    .header("user-agent", "codex-C6-qualified")
                    .header("x-cpar-input-message-ids-qualified", "true")
                    .body(raw)
                    .send()
                    .await?;
                let status = reply.status().as_u16();
                let bytes = reply.bytes().await?;
                assert_eq!(
                    status,
                    400,
                    "{label}/{qualified}: {}",
                    String::from_utf8_lossy(&bytes)
                );
                assert_eq!(
                    serde_json::from_slice::<Value>(&bytes)?["error"]["code"],
                    "ClientRequestError"
                );
                assert_eq!(peer.accepted.load(Ordering::SeqCst), 0, "{label}");
                assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
                assert_eq!(fixture.events.attempts()?, 0);
                assert_eq!(active_leases(&fixture), 0);
                println!(
                    "CPAR62_C6_NEGATIVE {}",
                    json!({"label":label,"streaming":streaming,"qualified":qualified,"status":status,"receiver":0,"attempts":0,"cost":"UNKNOWN"})
                );
            }
            for (suffix, raw) in [
                (
                    "/chat/completions",
                    json!({"model":"p12-test-model","messages":[{"role":"user","id":"C6","content":"text"}],"stream":streaming}),
                ),
                (
                    "/messages",
                    json!({"model":"p12-test-model","max_tokens":32,"messages":[{"role":"user","id":"C6","content":"text"}],"stream":streaming}),
                ),
            ] {
                let url = url.trim_end_matches("/responses").to_owned() + suffix;
                let (status, bytes) = namespaces::request(&fixture, &url, &raw).await?;
                assert_eq!(status, 400, "{suffix}: {}", String::from_utf8_lossy(&bytes));
                assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
            }
            server.stop(false).await;
            let database = recording.finish(&fixture, 0).await?;
            assert_counts(&database, 0, 0).await?;
        }
    }
    Ok(())
}

pub(super) async fn assert_counts(
    database: &std::path::Path,
    attempts: usize,
    usage: usize,
) -> TestResult {
    let rows = SqliteEventStore::open_read_only(database)?.list_events()?;
    let requests: Vec<_> = rows
        .iter()
        .filter_map(|r| {
            if let GatewayEvent::Request(v) = r.event() {
                Some(v)
            } else {
                None
            }
        })
        .collect();
    let actual_attempts: Vec<_> = rows
        .iter()
        .filter_map(|r| {
            if let GatewayEvent::Attempt(v) = r.event() {
                Some(v)
            } else {
                None
            }
        })
        .collect();
    let usages: Vec<_> = rows
        .iter()
        .filter_map(|r| {
            if let GatewayEvent::Usage(v) = r.event() {
                Some((r.event_id(), v))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(actual_attempts.len(), attempts);
    assert_eq!(usages.len(), usage);
    for (_, v) in &usages {
        let attempt = actual_attempts
            .iter()
            .find(|a| Some(a.attempt_id()) == v.attempt_id())
            .ok_or("exact Attempt")?;
        assert!(
            requests
                .iter()
                .any(|r| r.request_id() == attempt.request_id())
        );
        assert_eq!(v.request_id(), attempt.request_id());
        assert_eq!(attempt.upstream_model(), "grok-4.5");
    }
    let worker = crate::billing_worker::BillingWorker::start(
        database.to_owned(),
        Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
    );
    worker.stop().await.map_err(|()| "billing")?;
    let ledger = SqliteBillingLedger::open_read_only(database)?.list_bounded(20)?;
    assert_eq!(ledger.len(), usage);
    for row in &ledger {
        let (_, v) = usages
            .iter()
            .find(|(id, _)| *id == row.source_event_id)
            .ok_or("exact ledger source")?;
        assert_eq!(row.request_id, v.request_id().as_str());
        assert_eq!(row.usage, *v.usage());
        assert!(row.cost_microunits.is_none());
    }
    println!(
        "CPAR62_C6_DURABLE {}",
        json!({"requests":requests.len(),"attempts":actual_attempts.len(),"usage":usages.len(),"ledger":ledger.len(),"cost":"UNKNOWN"})
    );
    Ok(())
}

fn mixed_history() -> Value {
    let mut input = inputs().as_array().cloned().unwrap_or_default();
    input.extend([
        json!({"type":"message","id":"assistant-history","role":"assistant","status":"completed","phase":"commentary","content":[{"type":"output_text","text":"checking","annotations":[]}]}),
        json!({"type":"reasoning","id":"reason-history","status":"completed","summary":[{"type":"summary_text","text":"plan"}]}),
        json!({"type":"function_call","id":"function-history","call_id":"user-C6~Original_02","namespace":"group","name":"lookup","arguments":"{}","status":"completed"}),
        json!({"type":"function_call_output","call_id":"user-C6~Original_02","output":"41","name":"lookup","namespace":"group"}),
        json!({"type":"web_search_call","id":"hosted-history","status":"completed","action":{"type":"search","queries":[]}}),
    ]);
    json!(input)
}

#[actix_web::test]
async fn public_build_c6_cannot_qualify_other_adapters_or_passthrough() -> TestResult {
    for native in [false, true] {
        for mode in [
            TransformMode::Canonical,
            TransformMode::CanonicalBridge,
            TransformMode::Passthrough,
        ] {
            if native && mode != TransformMode::Passthrough {
                continue;
            }
            let peer = ModelPeer::start_wire(false, tool_rounds::wire_response(1, false)?)?;
            let (channel, secret) = if native {
                native_channels()[0]
            } else {
                (
                    Channel {
                        adapter: "openai-compatible.responses",
                        format: "openai/responses",
                        base: "https://example.invalid",
                        path: "/v1/responses",
                        model: "p12-test-model",
                    },
                    "synthetic-c6-bearer",
                )
            };
            let transport = namespaces::transport(&peer, 7)?;
            let fixture = fixture_with_configuration(
                channel,
                secret.as_bytes(),
                false,
                |configuration| {
                    configuration.route_candidates[0].transform_mode = mode;
                    Ok(())
                },
                |endpoints| {
                    let mut profiles = P12TransportProfiles::try_new()?;
                    profiles.build = Some(transport);
                    profiles.build_input_message_ids_local_controlled = true;
                    endpoints.values_mut().next().ok_or("endpoint")?.transports =
                        Arc::new(profiles);
                    Ok(())
                },
            )?;
            let recording = Recording::start(&fixture, false)?;
            let (url, server) = namespaces::listener(&fixture)?;
            let (status, bytes) = namespaces::request(&fixture, &url, &body(false)).await?;
            assert_eq!(
                status,
                400,
                "{native}/{mode:?}: {}",
                String::from_utf8_lossy(&bytes)
            );
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
            assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
            server.stop(false).await;
            let database = recording.finish(&fixture, 0).await?;
            assert_counts(&database, 0, 0).await?;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_c6_cross_kind_ids_and_field_qualifications_are_independent() -> TestResult {
    for streaming in [false, true] {
        for ids in [false, true] {
            let peer = ModelPeer::start_wire(streaming, tool_rounds::wire_response(1, streaming)?)?;
            let fixture = fixture_for(namespaces::transport(&peer, 7)?, false, |p| {
                p.build_input_message_ids_local_controlled = ids;
                p.build_namespaces_local_controlled = true;
                p.build_search_local_controlled = true;
            })?;
            let recording = Recording::start(&fixture, false)?;
            let (url, server) = namespaces::listener(&fixture)?;
            let valid = json!({"model":"p12-test-model","input":mixed_history(),"stream":streaming,"tools":[
                {"type":"namespace","name":"group","description":"exact group","tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]},
                {"type":"web_search","external_web_access":false}],"parallel_tool_calls":false});
            for index in [3, 4, 5, 7] {
                let mut invalid = valid.clone();
                invalid["input"][index]["id"] = inputs()[0]["id"].clone();
                let (status, bytes) = namespaces::request(&fixture, &url, &invalid).await?;
                assert_eq!(status, 400, "{index}: {}", String::from_utf8_lossy(&bytes));
            }
            let (status, bytes) = namespaces::request(&fixture, &url, &valid).await?;
            assert_eq!(
                status,
                if ids { 200 } else { 400 },
                "{}",
                String::from_utf8_lossy(&bytes)
            );
            if ids {
                assert_eq!(
                    namespaces::terminal(&bytes, streaming)?["output"],
                    tool_rounds::output(1)
                );
            }
            server.stop(false).await;
            let count = usize::from(ids);
            assert_eq!(peer.accepted.load(Ordering::SeqCst), count);
            {
                let received = peer.requests.lock().map_err(|_| "receiver")?;
                assert_eq!(received.len(), count);
                if ids {
                    assert_eq!(received[0].body["input"], mixed_history());
                }
            }
            let database = recording.finish(&fixture, 0).await?;
            assert_counts(&database, count, count).await?;
        }
    }
    Ok(())
}
