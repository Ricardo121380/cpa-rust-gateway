//! Independent qualification and multi-result admission use public rejection oracles.
use super::*;

fn completed_history() -> Value {
    let mut input = output(0, true).as_array().cloned().unwrap_or_default();
    input.push(json!({"type":"function_call_output","call_id":"parallel-call-1","namespace":"right","name":"lookup","output":"22"}));
    input.push(json!({"type":"function_call_output","call_id":"parallel-call-0","namespace":"left","name":"lookup","output":"19"}));
    json!(input)
}

#[actix_web::test]
async fn public_build_parallel_qualifications_controls_and_results_reject_before_send() -> TestResult
{
    for streaming in [false, true] {
        for label in [
            "stock",
            "missing-compiler",
            "missing-parallel",
            "missing-transport",
            "missing-namespace",
            "missing-input-id",
            "missing-search",
            "null",
            "string",
            "number",
            "undeclared-selector",
            "namespace-selector",
            "missing-result",
            "wrong-result",
            "duplicate-result",
            "wrong-namespace",
            "wrong-result-name",
            "duplicate-call",
            "duplicate-item",
            "contradictory-history-arguments",
        ] {
            let peer = ModelPeer::start_wire(streaming, wire(1, false, streaming, false)?)?;
            let fixture = fixture_for(
                namespaces::transport(&peer, 7)?,
                false,
                !matches!(label, "stock" | "missing-compiler"),
                |p| {
                    p.build_parallel_local_controlled =
                        !matches!(label, "stock" | "missing-parallel");
                    p.build_namespaces_local_controlled = label != "missing-namespace";
                    p.build_input_message_ids_local_controlled = label != "missing-input-id";
                    p.build_search_local_controlled = label != "missing-search";
                    if label == "missing-transport" {
                        p.build = None;
                    }
                },
            )?;
            let recording = Recording::start(&fixture, false)?;
            let (url, server) = namespaces::listener(&fixture)?;
            let mut request = body(true, streaming);
            match label {
                "missing-input-id" => {
                    request["input"] = json!([{"role":"user","id":"parallel-c6","content":"seed"}]);
                }
                "missing-search" => request["tools"]
                    .as_array_mut()
                    .ok_or("tools")?
                    .push(json!({"type":"web_search","external_web_access":false})),
                "null" => request["parallel_tool_calls"] = Value::Null,
                "string" => request["parallel_tool_calls"] = json!("true"),
                "number" => request["parallel_tool_calls"] = json!(1),
                "undeclared-selector" => {
                    request["tool_choice"] = json!({"type":"function","name":"missing"});
                }
                "namespace-selector" => {
                    request["tool_choice"] =
                        json!({"type":"function","name":"lookup","namespace":"left"});
                }
                "missing-result"
                | "wrong-result"
                | "duplicate-result"
                | "wrong-namespace"
                | "wrong-result-name"
                | "duplicate-call"
                | "duplicate-item"
                | "contradictory-history-arguments" => {
                    request["input"] = completed_history();
                    match label {
                        "missing-result" => {
                            request["input"].as_array_mut().ok_or("input")?.pop();
                        }
                        "wrong-result" => request["input"][4]["call_id"] = json!("unseen-call"),
                        "duplicate-result" => request["input"][4] = request["input"][3].clone(),
                        "wrong-namespace" => request["input"][4]["namespace"] = json!("right"),
                        "wrong-result-name" => request["input"][4]["name"] = json!("combine"),
                        "duplicate-call" => {
                            request["input"][2]["call_id"] = request["input"][1]["call_id"].clone();
                        }
                        "duplicate-item" => {
                            request["input"][2]["id"] = request["input"][1]["id"].clone();
                        }
                        _ => request["input"][1]["arguments"] = json!(r#"{"seed":19,"seed":22}"#),
                    }
                }
                _ => {}
            }
            let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
            assert_eq!(
                status,
                400,
                "{label}/{streaming}: {}",
                String::from_utf8_lossy(&bytes)
            );
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 0, "{label}");
            assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
            assert_eq!(fixture.events.attempts()?, 0);
            assert_eq!(active_leases(&fixture), 0);
            server.stop(false).await;
            let database = recording.finish(&fixture, 0).await?;
            assert_counts(&database, 0, 0).await?;
            println!(
                "CPAR62_PARALLEL_PRE_REJECT {}",
                json!({"label":label,"streaming":streaming,"status":400,"accepted":0,"receiver":0,"attempts":0,"usage":0,"ledger":0})
            );
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_parallel_false_absent_and_selection_cardinalities_are_exact() -> TestResult {
    for streaming in [false, true] {
        for (label, choice, parallel, count, success) in [
            ("auto-zero", Some(json!("auto")), Some(true), 0, true),
            ("auto-two", Some(json!("auto")), Some(true), 2, true),
            ("absent-controls", None, None, 2, true),
            ("false-one", Some(json!("auto")), Some(false), 1, true),
            ("false-second", Some(json!("auto")), Some(false), 2, false),
            ("none-zero", Some(json!("none")), Some(true), 0, true),
            ("none-call", Some(json!("none")), Some(true), 1, false),
            ("required-two", Some(json!("required")), Some(true), 2, true),
            (
                "required-zero",
                Some(json!("required")),
                Some(true),
                0,
                false,
            ),
            (
                "named-one",
                Some(json!({"type":"function","name":"lookup"})),
                Some(true),
                1,
                true,
            ),
            (
                "named-zero",
                Some(json!({"type":"function","name":"lookup"})),
                Some(true),
                0,
                false,
            ),
            (
                "named-second",
                Some(json!({"type":"function","name":"lookup"})),
                Some(true),
                2,
                false,
            ),
            (
                "named-second-absent",
                Some(json!({"type":"function","name":"lookup"})),
                None,
                2,
                false,
            ),
            (
                "named-wrong",
                Some(json!({"type":"function","name":"combine"})),
                Some(true),
                1,
                false,
            ),
        ] {
            let items = if count == 0 {
                output(1, false)
            } else if count == 1 {
                json!([output(0, false)[0], output(0, false)[1]])
            } else {
                output(0, false)
            };
            let wire = if count == 2 {
                wire(0, false, streaming, false)?
            } else {
                namespaces::wire_output(&items, "parallel-native-0", streaming)?
            };
            let wire = if label.starts_with("named-second") {
                wire.replace(r#""name":"measure""#, r#""name":"lookup""#)
            } else {
                wire
            };
            let peer = ModelPeer::start_wire(streaming, wire)?;
            let fixture = fixture_for(namespaces::transport(&peer, 7)?, false, true, |p| {
                p.build_parallel_local_controlled = true;
            })?;
            let recording = Recording::start(&fixture, false)?;
            let (url, server) = namespaces::listener(&fixture)?;
            let mut request = body(false, streaming);
            if let Some(choice) = choice {
                request["tool_choice"] = choice;
            } else {
                request
                    .as_object_mut()
                    .ok_or("request")?
                    .remove("tool_choice");
            }
            if let Some(parallel) = parallel {
                request["parallel_tool_calls"] = json!(parallel);
            } else {
                request
                    .as_object_mut()
                    .ok_or("request")?
                    .remove("parallel_tool_calls");
            }
            let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
            if success {
                assert_eq!(status, 200, "{label}: {}", String::from_utf8_lossy(&bytes));
                assert_eq!(namespaces::terminal(&bytes, streaming)?["output"], items);
            } else if streaming {
                super::failures::assert_no_completion(&bytes)?;
            } else {
                assert_eq!(status, 502, "{label}");
            }
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
            assert_eq!(fixture.events.attempts()?, 1);
            let received = peer.requests.lock().map_err(|_| "receiver")?[0]
                .body
                .clone();
            assert_eq!(
                received.get("parallel_tool_calls"),
                request.get("parallel_tool_calls")
            );
            assert_eq!(received.get("tool_choice"), request.get("tool_choice"));
            server.stop(false).await;
            let database = recording.finish(&fixture, 0).await?;
            let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
            let usages = rows
                .iter()
                .filter(|r| matches!(r.event(), GatewayEvent::Usage(_)))
                .count();
            assert!(usages <= 1);
            assert_counts(&database, 1, usages).await?;
            println!(
                "CPAR62_PARALLEL_CONTROLS {}",
                json!({"label":label,"streaming":streaming,"success":success,"status":status,"receiver":1,"attempts":1,"observed_usage":usages,"cost":"UNKNOWN"})
            );
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_parallel_flag_does_not_qualify_other_adapters_or_passthrough() -> TestResult {
    for native in [false, true] {
        for mode in [
            TransformMode::Canonical,
            TransformMode::CanonicalBridge,
            TransformMode::Passthrough,
        ] {
            if native && mode != TransformMode::Passthrough {
                continue;
            }
            let peer = ModelPeer::start_wire(false, wire(1, false, false, false)?)?;
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
                    "synthetic-parallel-bearer",
                )
            };
            let transport = namespaces::transport(&peer, 7)?;
            let fixture = fixture_with_configuration(
                channel,
                secret.as_bytes(),
                false,
                |c| {
                    c.route_candidates[0].transform_mode = mode;
                    Ok(())
                },
                |endpoints| {
                    let mut p = P12TransportProfiles::try_new()?;
                    p.build = Some(transport);
                    p.build_parallel_local_controlled = true;
                    // The request also retains C6. A parallel flag cannot grant its adapter scope.
                    p.build_input_message_ids_local_controlled = true;
                    endpoints.values_mut().next().ok_or("endpoint")?.transports = Arc::new(p);
                    Ok(())
                },
            )?;
            let recording = Recording::start(&fixture, false)?;
            let (url, server) = namespaces::listener(&fixture)?;
            let mut request = body(false, false);
            request["input"] = json!([{"role":"user","id":"parallel-native-id","content":"probe"}]);
            let (status, _) = namespaces::request(&fixture, &url, &request).await?;
            assert_eq!(status, 400);
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
            assert_eq!(fixture.events.attempts()?, 0);
            server.stop(false).await;
            let database = recording.finish(&fixture, 0).await?;
            assert_counts(&database, 0, 0).await?;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_parallel_qualification_is_http_responses_only() -> TestResult {
    for (path, request) in [
        (
            "/v1/chat/completions",
            json!({"model":"p12-test-model","messages":[{"role":"user","content":"probe"}],
            "tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}],"parallel_tool_calls":true}),
        ),
        (
            "/v1/messages",
            json!({"model":"p12-test-model","messages":[{"role":"user","content":"probe"}],"max_tokens":16,
            "tools":[{"name":"lookup","input_schema":{"type":"object"}}],"tool_choice":{"type":"auto","disable_parallel_tool_use":false}}),
        ),
    ] {
        let peer = ModelPeer::start_wire(false, wire(1, false, false, false)?)?;
        let fixture = fixture_for(namespaces::transport(&peer, 7)?, false, true, |p| {
            p.build_parallel_local_controlled = true;
        })?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let url = url.replace("/v1/responses", path);
        let (status, _) = namespaces::request(&fixture, &url, &request).await?;
        assert_eq!(status, 400, "{path}");
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
        assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
        assert_eq!(fixture.events.attempts()?, 0);
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        assert_counts(&database, 0, 0).await?;
    }
    Ok(())
}
