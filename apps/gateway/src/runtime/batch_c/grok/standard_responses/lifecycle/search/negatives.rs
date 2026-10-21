//! Public pre-send admission and post-send protocol refusals. Fixtures remain unmodified evidence.
use super::*;

fn invalid_declarations(streaming: bool) -> Vec<(&'static str, Value)> {
    let mut cases = Vec::new();
    for (label, value) in [
        ("true", json!(true)),
        ("null", Value::Null),
        ("string", json!("false")),
        ("number", json!(0)),
    ] {
        let mut request = body(streaming);
        request["tools"][0]["external_web_access"] = value;
        cases.push((label, request));
    }
    for label in [
        "missing",
        "extra",
        "duplicate",
        "preview",
        "dated",
        "other",
        "forced-search",
        "parallel",
        "node-bound",
        "byte-bound",
        "namespace-child",
        "input-history-duplicate",
        "unfinished-history",
        "nested-history",
        "mixed-history",
        "cross-kind-history-id",
    ] {
        let mut request = body(streaming);
        match label {
            "missing" => {
                request["tools"][0]
                    .as_object_mut()
                    .map(|v| v.remove("external_web_access"));
            }
            "extra" => {
                request["tools"][0]["filters"] = json!({"allowed_domains":["example.invalid"]});
            }
            "duplicate" => request["tools"] = json!([declarations()[0], declarations()[0]]),
            "preview" => request["tools"][0]["type"] = json!("web_search_preview"),
            "dated" => request["tools"][0]["type"] = json!("web_search_2025_08_26"),
            "other" => request["tools"][0]["type"] = json!("x_search"),
            "forced-search" => request["tool_choice"] = json!({"type":"web_search"}),
            "parallel" => request["parallel_tool_calls"] = json!(true),
            "node-bound" => {
                let mut tools = vec![declarations()[0].clone()];
                tools.extend((0..128).map(|i|json!({"type":"function","name":format!("tool_{i}"),"parameters":{"type":"object"}})));
                request["tools"] = json!(tools);
            }
            "byte-bound" => {
                request["tools"] = json!([declarations()[0],{"type":"function","name":"big","description":"x".repeat(256*1024),"parameters":{"type":"object"}}]);
            }
            "namespace-child" => {
                request["tools"] = json!([{"type":"namespace","name":"nested","description":"not a function","tools":declarations()}]);
            }
            "input-history-duplicate" => {
                request["input"] =
                    json!([output(0)[0],output(0)[0],{"role":"user","content":"next"}]);
            }
            "unfinished-history" => {
                request["input"] = json!([output(0)[0],{"role":"user","content":"next"}]);
                request["input"][0]["status"] = json!("in_progress");
            }
            "nested-history" => {
                request["input"] = json!([{"role":"assistant","content":[output(0)[0]]},{"role":"user","content":"next"}]);
            }
            "mixed-history" => {
                request["input"] = json!([{"role":"assistant","content":[output(0)[0],{"type":"output_text","text":"mixed"}]},{"role":"user","content":"next"}]);
            }
            "cross-kind-history-id" => {
                request["input"] = json!([output(0)[0],{"type":"message","role":"assistant","id":"ws-search-0","status":"completed","content":[{"type":"output_text","text":"old"}]},{"role":"user","content":"next"}]);
            }
            _ => unreachable!(),
        }
        cases.push((label, request));
    }
    cases
}

#[actix_web::test]
async fn public_build_search_invalid_and_unqualified_requests_send_zero() -> TestResult {
    for streaming in [false, true] {
        for (namespace, search) in [(false, false), (true, false), (false, true), (true, true)] {
            let peer = ModelPeer::start_wire(streaming, wire_output(&output(0), streaming)?)?;
            let fixture = lifecycle_fixture_qualifications(
                namespaces::transport(&peer, 7)?,
                namespace,
                search,
            )?;
            let recording = Recording::start(&fixture, false)?;
            let (url, server) = namespaces::listener(&fixture)?;
            let mut cases = invalid_declarations(streaming);
            if !search {
                cases.push(("valid-search-unqualified", body(streaming)));
                let reply = reqwest::Client::builder()
                    .no_proxy()
                    .build()?
                    .post(&url)
                    .bearer_auth(&fixture.key)
                    .header("user-agent", "codex-qualified-search")
                    .header("x-cpar-search-qualified", "true")
                    .header("content-type", "application/json")
                    .body(body(streaming).to_string())
                    .send()
                    .await?;
                assert_eq!(reply.status(), 400);
                assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
            }
            if !namespace {
                let mut mixed = body(streaming);
                mixed["tools"] = json!([declarations()[0],{"type":"namespace","name":"n","description":"group","tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]}]);
                cases.push(("search-cannot-qualify-namespace", mixed));
            }
            for (label, request) in cases {
                let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
                assert_eq!(
                    status,
                    400,
                    "{namespace}/{search}/{label}: {}",
                    String::from_utf8_lossy(&bytes)
                );
                assert_eq!(peer.accepted.load(Ordering::SeqCst), 0, "{label}");
                assert_eq!(fixture.events.attempts()?, 0);
                assert_eq!(active_leases(&fixture), 0);
                println!(
                    "CPAR62_SEARCH_PRESEND {}",
                    json!({"streaming":streaming,"namespace_qualified":namespace,"search_qualified":search,"label":label,"status":status,"receiver_count":0,"attempt_count":0})
                );
            }
            for raw in [
                r#"{"model":"p12-test-model","input":"duplicate","tools":[{"type":"web_search","external_web_access":true,"external_web_access":false}]}"#,
                r#"{"model":"p12-test-model","input":"duplicate","tools":[{"type":"web_search","type":"web_search","external_web_access":false}]}"#,
            ] {
                let reply = reqwest::Client::builder()
                    .no_proxy()
                    .build()?
                    .post(&url)
                    .bearer_auth(&fixture.key)
                    .header("content-type", "application/json")
                    .header("user-agent", "codex-qualified-search")
                    .header("x-cpar-search-qualified", "true")
                    .body(raw)
                    .send()
                    .await?;
                assert_eq!(reply.status(), 400);
                assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
            }
            server.stop(false).await;
            let database = recording.finish(&fixture, 0).await?;
            let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
            assert!(
                rows.iter().all(|v| !matches!(
                    v.event(),
                    GatewayEvent::Attempt(_) | GatewayEvent::Usage(_)
                ))
            );
        }
    }
    Ok(())
}

fn corrupt_items(label: &str) -> Value {
    let mut items = output(0);
    match label {
        "duplicate-id" => items[1]["id"] = items[0]["id"].clone(),
        "unknown-item-field" => items[0]["result"] = json!([]),
        "unknown-action" => items[0]["action"]["type"] = json!("find"),
        "unknown-action-field" => items[0]["action"]["count"] = json!(0),
        "null-action" => items[0]["action"] = Value::Null,
        "null-queries" => items[0]["action"]["queries"] = Value::Null,
        "bad-sources" => items[0]["action"]["sources"][0]["type"] = json!("unknown"),
        "source-bound" => {
            items[0]["action"]["sources"] = json!(vec![
                json!({"type":"url","url":"https://example.invalid"});
                129
            ]);
        }
        "query-bound" => items[0]["action"]["query"] = json!("x".repeat(4097)),
        "bad-url" => items[1]["action"]["url"] = json!("file:///private/secret"),
        "missing-pattern" => {
            items[2]["action"]
                .as_object_mut()
                .map(|v| v.remove("pattern"));
        }
        "failed-status" => items[0]["status"] = json!("failed"),
        "incomplete-status" => items[0]["status"] = json!("incomplete"),
        "citation-range" => items[3]["content"][0]["annotations"][0]["end_index"] = json!(100),
        "citation-inverted" => items[3]["content"][0]["annotations"][0]["start_index"] = json!(15),
        "citation-url" => {
            items[3]["content"][0]["annotations"][0]["url"] = json!("javascript:fetch(1)");
        }
        "citation-extra" => items[3]["content"][0]["annotations"][0]["unknown"] = json!(true),
        _ => unreachable!(),
    }
    items
}

pub(super) async fn assert_failure(
    streaming: bool,
    label: &str,
    wire: String,
    request: Value,
) -> TestResult {
    let peer = ModelPeer::start_wire(streaming, wire)?;
    let fixture = lifecycle_fixture_qualifications(namespaces::transport(&peer, 7)?, false, true)?;
    let (url, server) = namespaces::listener(&fixture)?;
    let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
    if streaming {
        assert_eq!(status, 200, "{label}: {}", String::from_utf8_lossy(&bytes));
        let frames = public_frames(&bytes)?;
        assert_eq!(
            frames
                .iter()
                .filter(|v| v["type"] == "response.failed")
                .count(),
            1,
            "{label}: {}",
            String::from_utf8_lossy(&bytes)
        );
        assert!(
            frames.iter().all(|v| v["type"] != "response.completed"
                && !(v["type"] == "response.output_item.done"
                    && v["item"]["type"] == "web_search_call")),
            "{label}"
        );
    } else {
        assert_eq!(status, 502, "{label}: {}", String::from_utf8_lossy(&bytes));
    }
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 1, "{label}");
    assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
    assert_eq!(fixture.events.attempts()?, 1);
    assert_eq!(active_leases(&fixture), 0);
    println!(
        "CPAR62_SEARCH_REJECTION {}",
        json!({"streaming":streaming,"label":label,"receiver_count":1,"attempt_count":1,"status":status,"lease_after":0})
    );
    server.stop(false).await;
    Ok(())
}

#[actix_web::test]
async fn public_build_search_json_and_sse_reject_malformed_native_items_and_citations() -> TestResult
{
    for streaming in [false, true] {
        for label in [
            "duplicate-id",
            "unknown-item-field",
            "unknown-action",
            "unknown-action-field",
            "null-action",
            "null-queries",
            "bad-sources",
            "source-bound",
            "query-bound",
            "bad-url",
            "missing-pattern",
            "failed-status",
            "incomplete-status",
            "citation-range",
            "citation-inverted",
            "citation-url",
            "citation-extra",
        ] {
            assert_failure(
                streaming,
                label,
                wire_output(&corrupt_items(label), streaming)?,
                body(streaming),
            )
            .await?;
        }
        for label in ["undeclared", "none", "named-function"] {
            let mut request = body(streaming);
            if label == "undeclared" {
                request.as_object_mut().map(|v| v.remove("tools"));
            } else if label == "none" {
                request["tool_choice"] = json!("none");
            } else {
                request["tools"] = json!([declarations()[0],{"type":"function","name":"lookup","parameters":{"type":"object"}}]);
                request["tool_choice"] = json!({"type":"function","name":"lookup"});
            }
            assert_failure(
                streaming,
                label,
                wire_output(&output(0), streaming)?,
                request,
            )
            .await?;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_search_sse_rejects_phase_and_terminal_conflicts_once() -> TestResult {
    for label in [
        "phase-id",
        "phase-index",
        "phase-sequence",
        "phase-missing-sequence",
        "phase-extra",
        "phase-repeat",
        "phase-regress",
        "phase-missing-complete",
        "done-action",
        "terminal-action",
        "terminal-missing-action",
        "terminal-reorder",
        "duplicate-done",
        "eof-after-done",
    ] {
        let mut wire = String::new();
        for frame_text in wire_output(&output(0), true)?
            .split("\n\n")
            .filter(|v| !v.is_empty())
        {
            let mut value: Value = serde_json::from_str(
                frame_text
                    .lines()
                    .find_map(|v| v.strip_prefix("data: "))
                    .ok_or("data")?,
            )?;
            let kind = value["type"].as_str().ok_or("type")?.to_owned();
            if kind == "response.web_search_call.searching" && value["item_id"] == "ws-search-0" {
                match label {
                    "phase-id" => value["item_id"] = json!("wrong"),
                    "phase-index" => value["output_index"] = json!(1),
                    "phase-sequence" => value["sequence_number"] = json!(1),
                    "phase-missing-sequence" => {
                        value.as_object_mut().map(|v| v.remove("sequence_number"));
                    }
                    "phase-extra" => value["action"] = json!({"type":"search"}),
                    "phase-regress" => {
                        value["type"] = json!("response.web_search_call.in_progress");
                    }
                    _ => {}
                }
            }
            if label == "phase-missing-complete"
                && kind == "response.web_search_call.completed"
                && value["item_id"] == "ws-search-0"
            {
                continue;
            }
            if label == "done-action"
                && kind == "response.output_item.done"
                && value["item"]["id"] == "ws-open-0"
            {
                value["item"]["action"]["url"] = json!("https://example.invalid/changed");
            }
            if kind == "response.completed" {
                match label {
                    "terminal-action" => {
                        value["response"]["output"][0]["action"]["query"] = json!("rewritten");
                    }
                    "terminal-missing-action" => {
                        value["response"]["output"][0]
                            .as_object_mut()
                            .map(|v| v.remove("action"));
                    }
                    "terminal-reorder" => {
                        if let Some(items) = value["response"]["output"].as_array_mut() {
                            items.swap(0, 1);
                        }
                    }
                    "eof-after-done" => continue,
                    _ => {}
                }
            }
            let emitted_kind = value["type"].as_str().ok_or("type")?.to_owned();
            wire += &native_event(&emitted_kind, value.clone());
            if label == "phase-repeat" && kind == "response.web_search_call.searching" {
                wire += &native_event(&emitted_kind, value.clone());
            }
            if label == "duplicate-done"
                && kind == "response.output_item.done"
                && value["item"]["id"] == "ws-search-0"
            {
                wire += &native_event(&emitted_kind, value);
            }
        }
        assert_failure(true, label, wire, body(true)).await?;
    }
    Ok(())
}
