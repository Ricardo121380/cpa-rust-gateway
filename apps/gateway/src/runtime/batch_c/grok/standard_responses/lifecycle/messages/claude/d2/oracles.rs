//! Joins actual public Requests to independent receives, private receipts and exact accounting.
use super::*;

#[allow(
    clippy::too_many_lines,
    reason = "one actual-client evidence join keeps all counts and identities auditable"
)]
pub(super) async fn verify(
    fixture: &Fixture,
    peer: &SocketPeer,
    ingress: &web::Data<Ingress>,
    directory: &std::path::Path,
    database: &std::path::Path,
    scenario: &str,
) -> TestResult {
    let expected = if scenario == "standard" { 5 } else { 1 };
    let worker = crate::billing_worker::BillingWorker::start(
        database.to_owned(),
        Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
    );
    worker.stop().await.map_err(|()| "billing")?;
    let received = peer.received.lock().map_err(|_| "receiver")?;
    let receipts = peer.receipts.lock().map_err(|_| "receiver receipts")?;
    let rows = ingress.lock().map_err(|_| "capture")?;
    assert_eq!(received.len(), expected);
    assert_eq!(receipts.len(), expected);
    assert_eq!(rows.len(), expected + 1);
    let events = SqliteEventStore::open_read_only(database)?.list_events()?;
    let ledger = SqliteBillingLedger::open_read_only(database)?.list_bounded(20)?;
    let count = |kind| {
        events
            .iter()
            .filter(|row| {
                matches!(
                    (kind, row.event()),
                    (0, GatewayEvent::Request(_))
                        | (1, GatewayEvent::Attempt(_))
                        | (2, GatewayEvent::Usage(_))
                        | (3, GatewayEvent::RequestFinished(_))
                )
            })
            .count()
    };
    assert_eq!(
        (count(0), count(1), count(2), count(3)),
        (
            expected + 1,
            expected,
            if scenario == "cancel" { 0 } else { expected },
            expected + 1
        )
    );
    assert_eq!(ledger.len(), count(2));
    let mut joins = Vec::new();
    let mut seen_receivers = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.method, "POST");
        assert_eq!(row.uri, "/v1/messages?beta=true");
        assert_eq!(row.status, if index == 0 { 400 } else { 200 });
        assert!(!row.wire_truncated);
        assert_eq!(row.request["model"], "grok-4.5");
        assert_eq!(row.request["max_tokens"], 32000);
        assert_eq!(row.request["stream"], true);
        for key in ["thinking", "context_management"] {
            assert!(row.request.get(key).is_none());
        }
        assert!(!has_cache(&row.request));
        assert_eq!(
            row.request.get("output_config").cloned(),
            (index == 0).then(|| json!({"effort":"high"}))
        );
        let expected_history = expected_input(&row.request)?;
        let headers: Vec<(String, Vec<u8>)> = serde_json::from_value(row.response_headers.clone())?;
        let id = std::str::from_utf8(
            &headers
                .iter()
                .find(|(name, _)| name == "x-request-id")
                .ok_or("request id")?
                .1,
        )?;
        let requests: Vec<_> = events
            .iter()
            .filter_map(|event| match event.event() {
                GatewayEvent::Request(r) if r.request_id().as_str() == id => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(requests.len(), 1);
        let request = requests[0];
        let attempts: Vec<_> = events
            .iter()
            .filter_map(|event| match event.event() {
                GatewayEvent::Attempt(a) if a.request_id() == request.request_id() => Some(a),
                _ => None,
            })
            .collect();
        assert_eq!(attempts.len(), usize::from(index > 0));
        let raw: gateway_core::RawExtensions = serde_json::from_slice(&row.request_bytes)?;
        let metadata = raw.get("metadata").ok_or("actual M4")?;
        local_controls::assert_metadata(fixture, id, metadata.get(), attempts.len())?;
        let archived = gateway_store::request_metadata::SqliteRequestMetadataReader::open(
            database,
            test_secret_store()?,
        )?
        .get_owned(
            request.client_key_id(),
            request.request_id(),
            attempts.first().map(|a| a.attempt_id()),
            system_now_ms_runtime()?,
        )?
        .ok_or("archived private M4 evidence")?;
        assert_eq!(archived.record.evidence.raw_json(), metadata.get());
        assert_eq!(archived.attempts.len(), attempts.len());
        let private: Value = serde_json::from_str(metadata.get())?;
        let user = private["user_id"].as_str().ok_or("opaque user")?;
        assert!(!String::from_utf8_lossy(&row.wire).contains(user));
        assert!(
            !serde_json::to_string(
                &events
                    .iter()
                    .map(gateway_store::event_store::StoredGatewayEvent::event)
                    .collect::<Vec<_>>()
            )?
            .contains(user)
        );
        let finished: Vec<_> = events
            .iter()
            .filter_map(|event| match event.event() {
                GatewayEvent::RequestFinished(f) if f.request_id == *request.request_id() => {
                    Some(f)
                }
                _ => None,
            })
            .collect();
        assert_eq!(finished.len(), 1);
        if index == 0 {
            assert_eq!(
                serde_json::from_slice::<Value>(&row.wire)?,
                json!({"type":"error","error":{"type":"invalid_request_error","message":"output_config.effort: Extra inputs are not permitted by the current gateway conversion; no upstream request was sent"}})
            );
            assert_eq!(finished[0].outcome, gateway_core::RequestOutcome::Failed);
            assert_eq!(
                finished[0].error_code,
                Some(GatewayErrorCode::ClientRequestError)
            );
            assert!(!events.iter().any(|e|matches!(e.event(),GatewayEvent::Usage(v) if v.request_id()==request.request_id())));
            joins.push(json!({"request_id":id,"kind":"negotiation","attempts":0,"usage":0,"ledger":0,"receiver":null,"m4_raw_readback":true}));
            continue;
        }
        let frames = public_frames(&row.wire)?;
        let start = frames
            .iter()
            .find(|frame| frame["type"] == "message_start")
            .ok_or("message_start")?;
        let response_id = start["message"]["id"].as_str().ok_or("response id")?;
        let matched: Vec<_> = receipts
            .iter()
            .filter(|receipt| receipt["response_id"] == response_id)
            .collect();
        assert_eq!(
            matched.len(),
            1,
            "independent receive joins public response id {response_id}"
        );
        let ordinal = usize::try_from(matched[0]["ordinal"].as_u64().ok_or("ordinal")?)?;
        assert!(seen_receivers.insert(ordinal));
        let upstream = &received[ordinal];
        assert_eq!(upstream["model"], "grok-4.5");
        assert_eq!(upstream["max_output_tokens"], 32000);
        let tools:Vec<_>=row.request["tools"].as_array().ok_or("actual tools")?.iter().map(|tool|
            json!({"type":"function","name":tool["name"],"description":tool["description"],"parameters":tool["input_schema"]})
        ).collect();
        assert_eq!(upstream["tools"], json!(tools));
        assert_eq!(upstream["input"], expected_history);
        assert!(!upstream.to_string().contains(user));
        for key in [
            "metadata",
            "output_config",
            "context_management",
            "thinking",
        ] {
            assert!(upstream.get(key).is_none());
        }
        let received_headers: Vec<(String, String)> =
            serde_json::from_value(matched[0]["headers"].clone())?;
        assert!(
            received_headers
                .iter()
                .any(|(name, value)| name == "x-grok-model-override" && value == "grok-4.5")
        );
        assert!(
            received_headers
                .iter()
                .all(|(name, value)| !name.contains("metadata")
                    && !name.contains("user-id")
                    && !value.contains(user))
        );
        let attempt = attempts[0];
        assert_eq!(attempt.upstream_model(), "grok-4.5");
        assert_eq!(
            attempt
                .execution_identity()
                .ok_or("actual execution identity")?
                .channel,
            "grok.build"
        );
        let usages: Vec<_> = events
            .iter()
            .filter_map(|e| match e.event() {
                GatewayEvent::Usage(u) if u.request_id() == request.request_id() => {
                    Some((e.event_id(), u))
                }
                _ => None,
            })
            .collect();
        assert_eq!(usages.len(), usize::from(scenario != "cancel"));
        for (source, usage) in &usages {
            assert_eq!(usage.attempt_id(), Some(attempt.attempt_id()));
            assert_eq!(usage.response_id().as_str(), response_id);
            assert_eq!(
                (usage.usage().input_tokens, usage.usage().output_tokens),
                (Some(20), Some(7))
            );
            let entries: Vec<_> = ledger
                .iter()
                .filter(|entry| entry.source_event_id == *source)
                .collect();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].request_id, id);
            assert_eq!(entries[0].usage, *usage.usage());
            assert_eq!(entries[0].cost_microunits, None);
        }
        let summaries = if scenario == "standard" {
            let public = tool_rounds::client_result(&row.wire, true)?;
            assert_eq!(public["usage"]["input_tokens"], 20);
            assert_eq!(public["usage"]["output_tokens"], 7);
            let record = retained(database, id)?;
            assert_eq!(record.evidence.response_id().as_str(), response_id);
            assert_eq!(record.evidence.items().len(), 2);
            for item in record.evidence.items() {
                assert_eq!(
                    serde_json::from_str::<Value>(item.item.get())?,
                    matched[0]["output"][item.output_index]
                );
            }
            assert!(!String::from_utf8_lossy(&row.wire).contains("计划"));
            assert_eq!(finished[0].outcome, gateway_core::RequestOutcome::Succeeded);
            2
        } else {
            assert!(frames.iter().all(|frame| frame["type"] != "message_stop"));
            assert_eq!(
                finished[0].outcome,
                if scenario == "cancel" {
                    gateway_core::RequestOutcome::Cancelled
                } else {
                    gateway_core::RequestOutcome::Failed
                }
            );
            if scenario == "failure" {
                assert_eq!(
                    frames
                        .iter()
                        .filter(|frame| frame["type"] == "error")
                        .count(),
                    1
                );
            }
            0
        };
        joins.push(json!({"request_id":id,"attempt_id":attempt.attempt_id(),"receiver_ordinal":ordinal,"response_id":response_id,
            "credential_id":attempt.credential_id(),"endpoint_id":attempt.endpoint_id(),"upstream_id":attempt.upstream_id(),"upstream_model":attempt.upstream_model(),
            "owner":request.client_key_id(),"execution_identity":attempt.execution_identity(),"usage":usages.first().map(|(_,u)|u.usage()),
            "usage_source_event_id":usages.first().map(|(source,_)|source),"ledger":usages.len(),"summary_items":summaries,"m4_raw_readback":true,
            "history_predicate":true,"outcome":format!("{:?}",finished[0].outcome),"cost":"UNKNOWN"}));
    }
    assert_eq!(seen_receivers.len(), received.len());
    let client: Value = serde_json::from_slice(&std::fs::read(directory.join("client.json"))?)?;
    assert!(client["driver_error"].is_null());
    let results = client["results"].as_array().ok_or("native results")?;
    if scenario == "standard" {
        assert_eq!(client["exit"], 0);
        assert_eq!(results.len(), 3);
        let native_events: Vec<Value> = client["stdout"]
            .as_str()
            .ok_or("native stdout")?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        assert!(
            native_events.iter().any(|row| row["type"] == "stream_event"
                && row["event"]["type"] == "content_block_delta")
        );
        let actual_tools: Vec<_> = native_events
            .iter()
            .filter(|row| row["type"] == "user" && row.get("tool_use_result").is_some())
            .collect();
        assert_eq!(actual_tools.len(), 2);
        assert_eq!(actual_tools[1]["tool_use_result"]["stdout"], "42");
        assert_eq!(actual_tools[1]["tool_use_result"]["stderr"], "");
        assert_eq!(actual_tools[1]["tool_use_result"]["interrupted"], false);
        assert!(
            results
                .iter()
                .all(|r| r["subtype"] == "success" && r["is_error"] == false)
        );
        assert_eq!(
            results[2]["result"],
            "result.txt contains 42, one more than the seed 41."
        );
        assert_eq!(
            std::fs::read_to_string(directory.join("work/result.txt"))?,
            "42\n"
        );
        for (call_id, name, arguments, needle) in [
            (
                "call-64-0",
                "Read",
                json!({"file_path":directory.join("work/seed.txt")}),
                "41",
            ),
            (
                "call-64-2",
                "Bash",
                json!({"command":peer::BASH,"description":"Add one to the observed seed and write result.txt"}),
                "42",
            ),
        ] {
            let input = received[4]["input"].as_array().ok_or("final history")?;
            let call = input
                .iter()
                .position(|v| v["type"] == "function_call" && v["call_id"] == call_id)
                .ok_or("actual historical tool")?;
            let result = input
                .iter()
                .position(|v| v["type"] == "function_call_output" && v["call_id"] == call_id)
                .ok_or("actual result")?;
            assert!(call < result);
            assert_eq!(input[call]["name"], name);
            assert_eq!(
                serde_json::from_str::<Value>(input[call]["arguments"].as_str().ok_or("args")?)?,
                arguments
            );
            assert!(input[result]["output"].to_string().contains(needle));
        }
    } else {
        assert!(!directory.join("work/result.txt").exists());
        if scenario == "failure" {
            assert!(!results.is_empty() && results.iter().all(|r| r["is_error"] == true));
        }
        if scenario == "cancel" {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0]["is_error"], true);
            assert_eq!(results[0]["terminal_reason"], "aborted_streaming");
            let alive: Value =
                serde_json::from_slice(&std::fs::read(directory.join("alive-through-release"))?)?;
            assert_eq!(alive["pid"], client["pid"]);
            assert_eq!(alive["alive"], true);
            assert!(results.iter().all(|r| r["terminal_reason"] != "completed"));
        }
    }
    std::fs::write(
        directory.join("observations.json"),
        serde_json::to_vec_pretty(
            &json!({"qualification":"PASS_LOCAL_CONTROLLED","scenario":scenario,
        "requests":rows.len(),"negotiation_rejections":1,"background_requests":0,"attempts":expected,"independent_receives":received.len(),"usage":count(2),"ledger":ledger.len(),
        "joins":joins,"client_exit":client["exit"],"client_pid":client["pid"],"native_results":results.iter().map(|r|json!({"subtype":r["subtype"],"is_error":r["is_error"],"terminal_reason":r["terminal_reason"]})).collect::<Vec<_>>(),
        "active_leases":active_leases(fixture),"summary":"encrypted full readback on all successful Attempts","metadata":"raw v2 readback on every Request",
        "cost":"UNKNOWN","fixed_service":"NOT_RUN","real_provider":"NOT_RUN"}),
        )?,
    )?;
    Ok(())
}

fn has_cache(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.contains_key("cache_control") || map.values().any(has_cache),
        Value::Array(items) => items.iter().any(has_cache),
        _ => false,
    }
}

fn expected_input(request: &Value) -> Result<Value, Box<dyn Error>> {
    let mut input = Vec::new();
    let mut calls = BTreeSet::new();
    let mut results = BTreeSet::new();
    let text =
        |role: &str, content: Vec<Value>| json!({"type":"message","role":role,"content":content});
    if let Some(system) = request.get("system") {
        let blocks = system.as_array().ok_or("actual system blocks")?;
        input.push(text(
            "system",
            blocks
                .iter()
                .map(|b| json!({"type":"input_text","text":b["text"]}))
                .collect(),
        ));
    }
    for message in request["messages"].as_array().ok_or("actual messages")? {
        let role = message["role"].as_str().ok_or("role")?;
        let kind = if role == "assistant" {
            "output_text"
        } else {
            "input_text"
        };
        if let Some(value) = message["content"].as_str() {
            input.push(text(role, vec![json!({"type":kind,"text":value})]));
            continue;
        }
        let mut pending = Vec::new();
        for block in message["content"].as_array().ok_or("blocks")? {
            if block["type"] == "text" {
                pending.push(json!({"type":kind,"text":block["text"]}));
                continue;
            }
            if !pending.is_empty() {
                input.push(text(role, std::mem::take(&mut pending)));
            }
            input.push(match block["type"].as_str() {
                Some("tool_use") => {
                    let id=block["id"].as_str().ok_or("actual call id")?;
                    assert!(calls.insert(id));
                    json!({"type":"function_call","call_id":id,"name":block["name"],"arguments":block["input"].to_string()})
                }
                Some("tool_result") => {
                    let id=block["tool_use_id"].as_str().ok_or("actual result id")?;
                    assert!(calls.contains(id)&&results.insert(id));
                    assert_ne!(block["is_error"],true);
                    json!({"type":"function_call_output","call_id":id,"output":block["content"]})
                }
                _ => return Err("unrepresented actual-client history block".into()),
            });
        }
        if !pending.is_empty() {
            input.push(text(role, pending));
        }
    }
    Ok(json!(input))
}
