//! C5 hosted search through the public listener and an independent owned receiver.
use super::*;
mod failures;
mod negatives;

fn declarations() -> Value {
    json!([{"type":"web_search","external_web_access":false}])
}

fn answer(id: &str, text: &str) -> Value {
    let title = if id == "answer-1" {
        ""
    } else {
        "Seed reference"
    };
    json!({"id":id,"type":"message","status":"completed","role":"assistant","content":[{
        "type":"output_text","text":text,"annotations":[{"type":"url_citation","start_index":0,"end_index":14,
            "url":"https://example.invalid/seed","title":title}]}]})
}

fn output(round: usize) -> Value {
    match round {
        0 => json!([
            {"id":"ws-search-0","type":"web_search_call","status":"completed","action":{
                "type":"search","query":"seed successor","queries":["seed successor"],
                "sources":[{"type":"url","url":"https://example.invalid/seed"}]}},
            {"id":"ws-open-0","type":"web_search_call","status":"completed","action":{"type":"open_page","url":null}},
            {"id":"ws-find-0","type":"web_search_call","status":"completed","action":{
                "type":"find_in_page","pattern":"seed","url":"https://example.invalid/seed"}},
            answer("answer-0", "The seed is 41.")
        ]),
        1 => json!([
            {"id":"ws-search-1","type":"web_search_call","status":"completed","action":{"type":"search","queries":[],"sources":[]}},
            {"id":"ws-open-1","type":"web_search_call","status":"completed","action":{"type":"open_page"}},
            {"id":"ws-find-1","type":"web_search_call","status":"completed","action":{"type":"find_in_page","pattern":"41","url":null}},
            {"id":"ws-absent-1","type":"web_search_call","status":"completed"},
            answer("answer-1", "The result is 42.")
        ]),
        _ => json!([answer("answer-2", "The seed was 41; its successor is 42.")]),
    }
}

fn wire_output(items: &Value, streaming: bool) -> Result<String, Box<dyn Error>> {
    let mut terminal = response();
    terminal["output"] = items.clone();
    if !streaming {
        return Ok(terminal.to_string());
    }
    let mut wire = frame("response.created", &created());
    let mut sequence = 1_u64;
    for (index, item) in items.as_array().ok_or("items")?.iter().enumerate() {
        let mut added = item.clone();
        added["status"] = json!("in_progress");
        match item["type"].as_str() {
            Some("web_search_call") => {
                if index % 2 == 0 {
                    added.as_object_mut().ok_or("item")?.remove("action");
                }
                wire += &native_event(
                    "response.output_item.added",
                    json!({"output_index":index,"item":added}),
                );
                for kind in [
                    "response.web_search_call.in_progress",
                    "response.web_search_call.searching",
                    "response.web_search_call.completed",
                ] {
                    if item.get("action").is_none() {
                        break;
                    }
                    wire += &native_event(
                        kind,
                        json!({"output_index":index,"item_id":item["id"],"sequence_number":sequence}),
                    );
                    sequence += 1;
                }
            }
            Some("function_call") => {
                added["arguments"] = json!("");
                wire += &native_event(
                    "response.output_item.added",
                    json!({"output_index":index,"item":added}),
                );
                wire += &native_event(
                    "response.function_call_arguments.delta",
                    json!({"output_index":index,"item_id":item["id"],"delta":item["arguments"]}),
                );
            }
            Some("message") => {
                added["content"] = json!([]);
                wire += &native_event(
                    "response.output_item.added",
                    json!({"output_index":index,"item":added}),
                );
                for ch in item["content"][0]["text"].as_str().ok_or("text")?.chars() {
                    wire += &native_event(
                        "response.output_text.delta",
                        json!({"output_index":index,"item_id":item["id"],"content_index":0,"delta":ch.to_string()}),
                    );
                }
                for (annotation_index, annotation) in item["content"][0]["annotations"]
                    .as_array()
                    .ok_or("annotations")?
                    .iter()
                    .enumerate()
                {
                    wire += &native_event(
                        "response.output_text.annotation.added",
                        json!({"output_index":index,"item_id":item["id"],"content_index":0,"annotation_index":annotation_index,"annotation":annotation}),
                    );
                }
            }
            _ => return Err("unexpected item".into()),
        }
        wire += &native_event(
            "response.output_item.done",
            json!({"output_index":index,"item":item}),
        );
    }
    Ok(wire + &frame("response.completed", &terminal))
}

fn body(streaming: bool) -> Value {
    json!({"model":"p12-test-model","input":"Find the seed and cite it.","stream":streaming,
        "tools":declarations(),"tool_choice":"auto","parallel_tool_calls":false})
}

#[actix_web::test]
async fn public_build_search_json_and_fragmented_sse_keep_explicit_history() -> TestResult {
    for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
        let peer = ModelPeer::start_sequence(
            streaming,
            (0..3)
                .map(|round| wire_output(&output(round), streaming))
                .collect::<Result<_, _>>()?,
        )?;
        let fixture =
            lifecycle_fixture_qualifications(namespaces::transport(&peer, size)?, false, true)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let mut history = vec![json!({"role":"user","content":"Find the seed and cite it."})];
        let mut public = Vec::new();
        for round in 0..3 {
            let mut request = body(streaming);
            request["input"] = json!(history);
            request["tool_choice"] = json!(if round == 2 { "none" } else { "required" });
            if round == 2 {
                request.as_object_mut().ok_or("request")?.remove("tools");
            }
            let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
            assert_eq!(status, 200, "{round}: {}", String::from_utf8_lossy(&bytes));
            let reply = namespaces::terminal(&bytes, streaming)?;
            assert_eq!(reply["output"], output(round));
            assert_eq!(reply["usage"]["input_tokens"], 20);
            assert_eq!(reply["usage"]["output_tokens"], 7);
            assert!(reply["usage"]["cpar_usage"]["cache_read_tokens"].is_null());
            if streaming {
                let frames = public_frames(&bytes)?;
                let phases: Vec<_> = frames
                    .iter()
                    .filter(|v| {
                        v["type"]
                            .as_str()
                            .is_some_and(|v| v.starts_with("response.web_search_call."))
                    })
                    .cloned()
                    .collect();
                assert_eq!(phases.len(), if round < 2 { 9 } else { 0 });
                let seq: Vec<_> = frames
                    .iter()
                    .filter_map(|v| v["sequence_number"].as_u64())
                    .collect();
                assert!(seq.windows(2).all(|v| v[0] < v[1]));
                assert!(
                    frames
                        .iter()
                        .all(|v| v["type"] != "response.function_call_arguments.done")
                );
                public.push(json!({"round":round,"phases":phases,"response":reply}));
            } else {
                public.push(reply.clone());
            }
            let sent = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(sent.len(), round + 1);
            if round == 2 {
                assert!(sent[round].body.get("tools").is_none());
            } else {
                assert_eq!(sent[round].body["tools"], declarations());
            }
            assert_eq!(sent[round].body["parallel_tool_calls"], false);
            namespaces::assert_history(&sent[round].body["input"], &history);
            drop(sent);
            history.extend(reply["output"].as_array().ok_or("output")?.iter().cloned());
            history.push(json!({"role":"user","content":if round==0 {"Find the cached successor of that seed."} else {"What were both values?"}}));
        }
        server.stop(false).await;
        assert_eq!(active_leases(&fixture), 0);
        let database = recording.finish(&fixture, 0).await?;
        assert_recorded(&database, 3).await?;
        let requests: Vec<_> = peer
            .requests
            .lock()
            .map_err(|_| "receiver")?
            .iter()
            .map(|v| v.body.clone())
            .collect();
        println!(
            "CPAR62_SEARCH_RECEIPT {}",
            json!({"streaming":streaming,"fragment_bytes":size,"qualification":"LOCAL_CONTROLLED",
            "activation":"BUILD_ACTIVATION_BLOCKED","receiver_count":requests.len(),"attempt_count":fixture.events.attempts()?,
            "usage_count":3,"ledger_count":3,"cost":"UNKNOWN","lease_after":0,"requests":requests,"public":public})
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_search_coexists_with_flat_and_namespaced_functions() -> TestResult {
    for streaming in [false, true] {
        let function = json!({"type":"function","name":"lookup","parameters":{"type":"object"},"strict":false});
        let tools = json!([declarations()[0],function,{"type":"namespace","name":"read_side","description":"Use the prior seed.","tools":[function]}]);
        let outputs:Vec<_>=(0..3).map(|round| {
            if round==2 { return output(2); }
            let mut items=if round==0 {output(0)} else {json!([])};
            let mut call=json!({"type":"function_call","id":format!("fn-{round}"),"call_id":format!("call-{round}"),"name":"lookup","arguments":if round==0 {"{}"} else {r#"{"previous":41}"#},"status":"completed"});
            if round==1 {call["namespace"]=json!("read_side");}
            if let Some(items)=items.as_array_mut() {items.push(call);}
            items
        }).collect();
        let peer = ModelPeer::start_sequence(
            streaming,
            outputs
                .iter()
                .map(|items| wire_output(items, streaming))
                .collect::<Result<_, _>>()?,
        )?;
        let fixture =
            lifecycle_fixture_qualifications(namespaces::transport(&peer, 7)?, true, true)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let mut history =
            vec![json!({"role":"user","content":"Search, read, then compute the successor."})];
        for (round, expected) in outputs.iter().enumerate() {
            let mut request = body(streaming);
            request["tools"] = tools.clone();
            request["input"] = json!(history);
            let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
            assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
            let reply = namespaces::terminal(&bytes, streaming)?;
            assert_eq!(&reply["output"], expected);
            let sent = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(sent[round].body["tools"], tools);
            namespaces::assert_history(&sent[round].body["input"], &history);
            drop(sent);
            history.extend(expected.as_array().ok_or("output")?.iter().cloned());
            if round < 2 {
                history.push(json!({"type":"function_call_output","call_id":format!("call-{round}"),"output":if round==0 {"41"} else {"42"}}));
            }
        }
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        assert_recorded(&database, 3).await?;
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 3);
        assert_eq!(active_leases(&fixture), 0);
        println!(
            "CPAR62_SEARCH_COEXIST {}",
            json!({"streaming":streaming,"receiver_count":3,"attempt_count":3,"lease_after":0})
        );
    }
    Ok(())
}

async fn assert_recorded(database: &std::path::Path, count: usize) -> TestResult {
    namespaces::assert_recorded(database, count).await?;
    let rows = SqliteEventStore::open_read_only(database)?.list_events()?;
    let requests: Vec<_> = rows
        .iter()
        .filter_map(|v| {
            if let GatewayEvent::Request(r) = v.event() {
                Some(r)
            } else {
                None
            }
        })
        .collect();
    let attempts: Vec<_> = rows
        .iter()
        .filter_map(|v| {
            if let GatewayEvent::Attempt(a) = v.event() {
                Some(a)
            } else {
                None
            }
        })
        .collect();
    let mut usage_attempts = BTreeSet::new();
    for row in &rows {
        if let GatewayEvent::Usage(usage) = row.event() {
            let id = usage.attempt_id().ok_or("actual Attempt")?;
            assert!(usage_attempts.insert(id));
            assert!(
                attempts
                    .iter()
                    .any(|a| a.attempt_id() == id && a.request_id() == usage.request_id())
            );
            assert!(
                requests
                    .iter()
                    .any(|r| r.request_id() == usage.request_id())
            );
            assert_eq!(
                usage.usage().provenance,
                gateway_core::UsageProvenance::Measured
            );
            assert_eq!(
                usage.usage().input_accounting,
                gateway_core::InputTokenAccounting::Inclusive
            );
        }
    }
    assert_eq!(usage_attempts.len(), count);
    Ok(())
}
