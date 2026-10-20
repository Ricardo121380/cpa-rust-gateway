//! Qualification is not granted by parsing, headers, capabilities or a transport replacement.
use super::*;

fn base(streaming: bool) -> Value {
    let tools = declarations();
    json!({"model":"p12-test-model","input":"namespace negative","stream":streaming,
        "tools":[tools[1],tools[2]],"tool_choice":"auto","parallel_tool_calls":false})
}

fn history() -> Value {
    json!([{"role":"user","content":"seed"},
        {"type":"function_call","call_id":"call-history","namespace":"read_side","name":"lookup","arguments":"{}"},
        {"type":"function_call_output","call_id":"call-history","output":"41"}])
}

fn invalid_requests(streaming: bool) -> Vec<(&'static str, Value)> {
    let mut cases = Vec::new();
    for label in [
        "duplicate-group",
        "duplicate-child",
        "duplicate-flat",
        "nested",
        "custom",
        "deferred",
        "child-unknown",
        "strict-null",
        "force-namespace",
        "force-qualified-name",
        "force-child-only",
        "result-name",
        "result-namespace",
        "result-call-id",
        "call-namespace",
        "call-null",
        "call-args-duplicate",
        "parallel-unqualified",
    ] {
        let mut body = base(streaming);
        match label {
            "duplicate-group" => body["tools"][1] = body["tools"][0].clone(),
            "duplicate-child" => {
                body["tools"][0]["tools"] =
                    json!([body["tools"][0]["tools"][0], body["tools"][0]["tools"][0]]);
            }
            "duplicate-flat" => {
                let f = declarations()[0].clone();
                if let Some(items) = body["tools"].as_array_mut() {
                    items.extend([f.clone(), f]);
                }
            }
            "nested" => body["tools"][0]["tools"][0] = body["tools"][1].clone(),
            "custom" => body["tools"][0]["tools"][0]["type"] = json!("custom"),
            "deferred" => body["tools"][0]["tools"][0]["defer_loading"] = json!(false),
            "child-unknown" => body["tools"][0]["tools"][0]["unreviewed"] = json!(false),
            "strict-null" => body["tools"][0]["tools"][0]["strict"] = Value::Null,
            "force-namespace" => {
                body["tool_choice"] =
                    json!({"type":"function","name":"lookup","namespace":"read_side"});
            }
            "force-qualified-name" => {
                body["tool_choice"] = json!({"type":"function","name":"read_side.lookup"});
            }
            "force-child-only" => body["tool_choice"] = json!({"type":"function","name":"lookup"}),
            "parallel-unqualified" => body["parallel_tool_calls"] = json!(true),
            _ => {
                body["input"] = history();
                match label {
                    "result-name" => body["input"][2]["name"] = json!("wrong"),
                    "result-namespace" => body["input"][2]["namespace"] = json!("write_side"),
                    "result-call-id" => body["input"][2]["call_id"] = json!("orphan"),
                    "call-namespace" => body["input"][1]["namespace"] = json!("unknown"),
                    "call-null" => body["input"][1]["namespace"] = Value::Null,
                    "call-args-duplicate" => {
                        body["input"][1]["arguments"] = json!(r#"{"v":1,"v":2}"#);
                    }
                    _ => unreachable!(),
                }
            }
        }
        cases.push((label, body));
    }
    cases
}

#[actix_web::test]
async fn public_build_namespace_invalid_requests_and_unqualified_assembly_send_zero() -> TestResult
{
    for streaming in [false, true] {
        for qualified in [false, true] {
            let peer = ModelPeer::start_wire(streaming, wire(0, streaming)?)?;
            let fixture = lifecycle_fixture_qualified(transport(&peer, 7)?, qualified)?;
            let (url, server) = listener(&fixture)?;
            let mut cases = invalid_requests(streaming);
            if !qualified {
                cases.push(("valid-but-unqualified", base(streaming)));
            }
            for (label, body) in cases {
                let (status, bytes) = request(&fixture, &url, &body).await?;
                assert_eq!(
                    status,
                    400,
                    "{qualified}/{label}: {}",
                    String::from_utf8_lossy(&bytes)
                );
                assert_eq!(
                    serde_json::from_slice::<Value>(&bytes)?["error"]["code"],
                    "ClientRequestError"
                );
                assert_eq!(peer.accepted.load(Ordering::SeqCst), 0, "{label}");
                assert_eq!(
                    peer.requests.lock().map_err(|_| "receiver")?.len(),
                    0,
                    "{label}"
                );
                assert_eq!(fixture.events.attempts()?, 0, "{label}");
                assert_eq!(active_leases(&fixture), 0);
                println!(
                    "CPAR62_NAMESPACE_NEGATIVE {}",
                    json!({"label":label,"streaming":streaming,"qualified":qualified,"status":status,"receiver_count":0,"attempt_count":0})
                );
            }
            server.stop(false).await;
        }
    }
    Ok(())
}

fn corrupt_value(value: &mut Value, label: &str) {
    let kind = value["type"].as_str().unwrap_or_default().to_owned();
    if kind == "response.function_call_arguments.delta" {
        match label {
            "delta-call-id" => value["call_id"] = json!("wrong-call"),
            "delta-index" => value["output_index"] = json!(0),
            _ => {}
        }
    }
    if kind == "response.function_call_arguments.done" && label == "arguments-done" {
        value["arguments"] = json!(r#"{"rewritten":true}"#);
    }
    let item = if kind == "response.output_item.added" || kind == "response.output_item.done" {
        value.get_mut("item")
    } else if kind == "response.completed" {
        value
            .get_mut("response")
            .and_then(|v| v["output"].get_mut(1))
    } else {
        None
    };
    if let Some(item) = item.filter(|v| v["type"] == "function_call") {
        let matches_phase = label.starts_with("terminal-") && kind == "response.completed"
            || label.starts_with("done-") && kind == "response.output_item.done"
            || label == "added-missing" && kind == "response.output_item.added";
        if matches_phase {
            if label.ends_with("missing") {
                item.as_object_mut().map(|v| v.remove("namespace"));
            } else if label.ends_with("namespace") {
                item["namespace"] = json!("wrong_namespace");
            } else if label.ends_with("name") {
                item["name"] = json!("other");
            } else if label.ends_with("call-id") {
                item["call_id"] = json!("other-call");
            }
        }
    }
    if kind == "response.completed" && label == "terminal-cost" {
        value["response"]["usage"]["cost_in_usd_ticks"] = json!(1);
    }
}

fn corrupt_wire(label: &str) -> Result<String, Box<dyn Error>> {
    let mut result = String::new();
    let mut argument_started = false;
    for record in wire(2, true)?.split("\n\n").filter(|v| !v.is_empty()) {
        let line = record
            .lines()
            .find_map(|v| v.strip_prefix("data: "))
            .ok_or("data")?;
        let mut value: Value = serde_json::from_str(line)?;
        if label == "eof-after-done" && value["type"] == "response.completed" {
            continue;
        }
        corrupt_value(&mut value, label);
        if let Some(arguments) = match label {
            "arguments-incomplete" => Some("{"),
            "arguments-array" => Some("[]"),
            "arguments-duplicate" => Some(r#"{"v":1,"v":2}"#),
            _ => None,
        } {
            match value["type"].as_str() {
                Some("response.function_call_arguments.delta") => {
                    value["delta"] = json!(if argument_started { "" } else { arguments });
                    argument_started = true;
                }
                Some("response.function_call_arguments.done") => {
                    value["arguments"] = json!(arguments);
                }
                Some("response.output_item.done") if value["item"]["type"] == "function_call" => {
                    value["item"]["arguments"] = json!(arguments);
                }
                Some("response.completed") => {
                    value["response"]["output"][1]["arguments"] = json!(arguments);
                }
                _ => {}
            }
        }
        if label == "failed-after-done" && value["type"] == "response.completed" {
            value["type"] = json!("response.failed");
            value["response"]["status"] = json!("failed");
            value["response"]["error"] = json!({"code":"server_error","message":"synthetic"});
        }
        result += &native_event(value["type"].as_str().ok_or("type")?, value.clone());
        if label == "duplicate-done"
            && value["type"] == "response.output_item.done"
            && value["item"]["type"] == "function_call"
        {
            result += &native_event("response.output_item.done", value.clone());
        }
        if label == "serial-second"
            && value["type"] == "response.output_item.added"
            && value["item"]["type"] == "function_call"
        {
            let mut item = value["item"].clone();
            item["id"] = json!("second-item");
            item["call_id"] = json!("second-call");
            item["namespace"] = json!("read_side");
            result += &native_event(
                "response.output_item.added",
                json!({"output_index":2,"item":item}),
            );
        }
    }
    Ok(result)
}

#[actix_web::test]
async fn public_build_namespace_sse_identity_conflicts_never_complete_tools_or_retry() -> TestResult
{
    for size in [1, 7, 31] {
        for label in [
            "added-missing",
            "delta-call-id",
            "delta-index",
            "arguments-done",
            "done-missing",
            "done-namespace",
            "done-name",
            "done-call-id",
            "terminal-missing",
            "terminal-namespace",
            "terminal-name",
            "terminal-call-id",
            "terminal-cost",
            "eof-after-done",
            "arguments-incomplete",
            "arguments-array",
            "arguments-duplicate",
            "duplicate-done",
            "failed-after-done",
            "serial-second",
        ] {
            let peer = ModelPeer::start_wire(true, corrupt_wire(label)?)?;
            let fixture = lifecycle_fixture_qualified(transport(&peer, size)?, true)?;
            let (url, server) = listener(&fixture)?;
            let (status, bytes) = request(&fixture, &url, &base(true)).await?;
            assert_eq!(status, 200, "{size}/{label}");
            let frames = public_frames(&bytes)?;
            assert_eq!(
                frames
                    .iter()
                    .filter(|v| v["type"] == "response.failed")
                    .count(),
                1,
                "{size}/{label}: {}",
                String::from_utf8_lossy(&bytes)
            );
            assert!(
                frames.iter().all(|v| v["type"] != "response.completed"
                    && v["type"] != "response.function_call_arguments.done"
                    && !(v["type"] == "response.output_item.done"
                        && v["item"]["type"] == "function_call")),
                "{size}/{label}"
            );
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 1, "{label}");
            assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
            assert_eq!(fixture.events.attempts()?, 1);
            assert_eq!(active_leases(&fixture), 0);
            println!(
                "CPAR62_NAMESPACE_NEGATIVE {}",
                json!({"label":label,"fragment_bytes":size,"status":status,"receiver_count":1,"attempt_count":1,"tool_completion_count":0,"failure_count":1})
            );
            server.stop(false).await;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_namespace_json_invalid_identity_and_arguments_fail_once() -> TestResult {
    for (field, value) in [
        ("namespace", Value::Null),
        ("namespace", json!("unknown")),
        ("name", json!("undeclared")),
        ("arguments", json!("{")),
        ("arguments", json!("[]")),
        ("arguments", json!(r#"{"v":1,"v":2}"#)),
    ] {
        let mut source: Value = serde_json::from_str(&wire(0, false)?)?;
        source["output"][1][field] = value;
        let peer = ModelPeer::start_wire(false, source.to_string())?;
        let fixture = lifecycle_fixture_qualified(transport(&peer, 31)?, true)?;
        let (url, server) = listener(&fixture)?;
        let (status, bytes) = request(&fixture, &url, &base(false)).await?;
        assert_eq!(status, 502, "{}", String::from_utf8_lossy(&bytes));
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
        assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
        assert_eq!(fixture.events.attempts()?, 1);
        server.stop(false).await;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_namespace_selection_controls_and_result_presence_are_exact() -> TestResult {
    for streaming in [false, true] {
        for (choice, round, success) in [
            (json!("auto"), 1, true),
            (json!("none"), 1, true),
            (json!("required"), 0, true),
            (json!("none"), 0, false),
            (json!("required"), 1, false),
            (json!({"type":"function","name":"lookup"}), 0, false),
        ] {
            let peer = ModelPeer::start_wire(streaming, wire(round, streaming)?)?;
            let fixture = lifecycle_fixture_qualified(transport(&peer, 7)?, true)?;
            let (url, server) = listener(&fixture)?;
            let mut body = base(streaming);
            body["tools"] = declarations();
            body["tool_choice"] = choice;
            let (status, bytes) = request(&fixture, &url, &body).await?;
            if success {
                assert_eq!(status, 200);
                terminal(&bytes, streaming)?;
            } else if streaming {
                assert_eq!(status, 200);
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
                assert_eq!(status, 502);
            }
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
            assert_eq!(
                peer.requests.lock().map_err(|_| "receiver")?[0].body["tool_choice"],
                body["tool_choice"]
            );
            server.stop(false).await;
        }
        result_presence(streaming).await?;
    }
    Ok(())
}

async fn result_presence(streaming: bool) -> TestResult {
    let peer = ModelPeer::start_wire(streaming, wire(1, streaming)?)?;
    let fixture = lifecycle_fixture_qualified(transport(&peer, 31)?, true)?;
    let (url, server) = listener(&fixture)?;
    let mut count = 0;
    for name in [None, Some(Value::Null), Some(json!("lookup"))] {
        for namespace in [None, Some(Value::Null), Some(json!("read_side"))] {
            let mut body = base(streaming);
            body["input"] = history();
            for (key, value) in [("name", name.clone()), ("namespace", namespace)] {
                if let Some(value) = value {
                    body["input"][2][key] = value;
                }
            }
            let (status, bytes) = request(&fixture, &url, &body).await?;
            assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
            terminal(&bytes, streaming)?;
            let sent = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(sent[count].body["input"][2], body["input"][2]);
            count += 1;
        }
    }
    assert_eq!(count, 9);
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 9);
    server.stop(false).await;
    Ok(())
}

#[actix_web::test]
async fn public_build_namespace_result_name_alone_still_requires_field_qualification() -> TestResult
{
    for streaming in [false, true] {
        for qualified in [false, true] {
            let peer = ModelPeer::start_wire(streaming, wire(1, streaming)?)?;
            let fixture = lifecycle_fixture_qualified(transport(&peer, 7)?, qualified)?;
            let (url, server) = listener(&fixture)?;
            for name in [Value::Null, json!("lookup")] {
                let mut body = base(streaming);
                body["tools"] = json!([declarations()[0]]);
                body["input"] = json!([
                    {"type":"function_call","call_id":"flat-history","name":"lookup","arguments":"{}"},
                    {"type":"function_call_output","call_id":"flat-history","name":name,"output":"41"}
                ]);
                let (status, bytes) = request(&fixture, &url, &body).await?;
                assert_eq!(status, if qualified { 200 } else { 400 });
                if qualified {
                    terminal(&bytes, streaming)?;
                    let sent = peer.requests.lock().map_err(|_| "receiver")?;
                    assert_eq!(sent.last().ok_or("sent")?.body["input"], body["input"]);
                } else {
                    assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
                    assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 0);
                    assert_eq!(fixture.events.attempts()?, 0);
                }
                assert_eq!(active_leases(&fixture), 0);
                println!(
                    "CPAR62_NAMESPACE_NEGATIVE {}",
                    json!({"label":"result-name-without-namespace","name":name,"streaming":streaming,"qualified":qualified,"status":status,"receiver_count":peer.accepted.load(Ordering::SeqCst)})
                );
            }
            server.stop(false).await;
        }
    }
    Ok(())
}
