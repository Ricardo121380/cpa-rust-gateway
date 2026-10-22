//! Observable CLI, native history, exact Attempt and durable ledger oracles.
use super::*;
pub(in super::super) fn assert_accounting(
    database: &std::path::Path,
    client: &str,
    scenario: &str,
) -> Result<Value, Box<dyn Error>> {
    let rows = SqliteEventStore::open_read_only(database)?.list_events()?;
    let (mut requests, mut attempts, mut usages, mut finished) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for row in &rows {
        match row.event() {
            GatewayEvent::Request(v) => requests.push(v),
            GatewayEvent::Attempt(v) => attempts.push(v),
            GatewayEvent::Usage(v) => usages.push((row.event_id(), v)),
            GatewayEvent::RequestFinished(v) => finished.push(v),
            _ => {}
        }
    }
    let expected = if scenario == "standard" { 5 } else { 1 };
    let probe_requests = usize::from(client == "pi" && scenario == "standard");
    let expected_attempts = if scenario == "unqualified" {
        0
    } else {
        expected
    };
    assert_eq!(
        (requests.len(), attempts.len(), finished.len()),
        (
            expected + probe_requests,
            expected_attempts,
            expected + probe_requests
        )
    );
    let usage_count = if scenario == "cancel" || scenario == "unqualified" {
        0
    } else {
        expected
    };
    assert_eq!(usages.len(), usage_count);
    let expected_outcome = match (client, scenario) {
        (_, "unqualified" | "failure") => gateway_core::RequestOutcome::Failed,
        (_, "cancel") => gateway_core::RequestOutcome::Cancelled,
        _ => gateway_core::RequestOutcome::Succeeded,
    };
    assert_eq!(
        finished
            .iter()
            .filter(|v| v.outcome == expected_outcome)
            .count(),
        expected
    );
    assert_eq!(
        finished
            .iter()
            .filter(|v| v.outcome != expected_outcome)
            .count(),
        probe_requests
    );
    if probe_requests == 1 {
        assert!(
            finished
                .iter()
                .any(|v| v.outcome == gateway_core::RequestOutcome::Failed
                    && v.error_code == Some(GatewayErrorCode::ClientRequestError))
        );
    }
    let reader = SqliteNativeUsageReader::open(database, test_secret_store()?)?;
    let mut lineage = Vec::new();
    for (source_event, usage) in &usages {
        let attempt = attempts
            .iter()
            .find(|v| Some(v.attempt_id()) == usage.attempt_id())
            .ok_or("Usage exact Attempt")?;
        let request = requests
            .iter()
            .find(|v| v.request_id() == usage.request_id())
            .ok_or("Usage exact Request")?;
        assert_eq!(attempt.request_id(), request.request_id());
        assert_eq!(attempt.upstream_model(), "grok-4.5");
        assert_eq!(
            (usage.usage().input_tokens, usage.usage().output_tokens),
            (Some(20), Some(7))
        );
        assert!(
            reader
                .get_owned(
                    request.client_key_id(),
                    request.request_id(),
                    attempt.attempt_id(),
                    system_now_ms_runtime()?
                )?
                .is_none(),
            "standard Usage has no #61 success metadata companion; absence is not full encrypted retention"
        );
        lineage.push(json!({"request_id":request.request_id().as_str(),"attempt_id":attempt.attempt_id().as_str(),
            "owner":request.client_key_id().as_str(),"source_event_id":source_event,"native_companion":"UNKNOWN"}));
    }
    assert_ledger(database, &usages)?;
    Ok(
        json!({"requests_including_negative_probe":requests.len(),"client_requests":expected,"attempts":attempts.len(),"finished":finished.len(),"usage":usages.len(),"ledger":usage_count,
        "outcome":format!("{expected_outcome:?}"),"lineage":lineage,"cost":"UNKNOWN"}),
    )
}

pub(super) async fn materialize_ledger(database: &std::path::Path) -> TestResult {
    let worker = crate::billing_worker::BillingWorker::start(
        database.to_owned(),
        Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
    );
    worker.stop().await.map_err(|()| "billing worker")?;
    Ok(())
}

fn assert_ledger(
    database: &std::path::Path,
    usages: &[(&str, &gateway_core::UsageEvent)],
) -> TestResult {
    let ledger = SqliteBillingLedger::open_read_only(database)?.list_bounded(10)?;
    assert_eq!(ledger.len(), usages.len());
    for entry in &ledger {
        let (_, usage) = usages
            .iter()
            .find(|(source, _)| *source == entry.source_event_id)
            .ok_or("ledger source identity")?;
        assert_eq!(entry.request_id, usage.request_id().as_str());
        assert_eq!(entry.usage, *usage.usage());
        assert_eq!(entry.cost_microunits, None);
    }
    assert_eq!(
        ledger
            .iter()
            .map(|v| &v.source_event_id)
            .collect::<BTreeSet<_>>()
            .len(),
        usages.len()
    );
    Ok(())
}

pub(super) fn assert_public(
    ingress: &Ingress,
    client: &str,
    scenario: &str,
) -> Result<Value, Box<dyn Error>> {
    let rows = ingress.lock().map_err(|_| "ingress")?;
    let rows: Vec<_> = rows.iter().filter(|v| !v.probe).collect();
    assert_eq!(rows.len(), if scenario == "standard" { 5 } else { 1 });
    if client == "codex" && scenario == "standard" {
        assert!(
            rows.iter().any(|row| combined_codex_request(&row.request)),
            "at least one actual positive request combines C1/C4/C5/C6 and parallel=true; later presence may vary"
        );
    }
    let mut results = Vec::new();
    for row in &rows {
        assert_eq!(row.path, "/v1/responses");
        if scenario == "unqualified" {
            assert_eq!(row.status, 400);
            assert_eq!(
                serde_json::from_slice::<Value>(&row.wire)?["error"]["code"],
                "ClientRequestError"
            );
            let tools = row.request["tools"]
                .as_array()
                .ok_or("actual Codex tools")?;
            assert!(tools.iter().any(|v| v["type"] == "namespace"));
            assert!(tools.iter().any(|v| v["type"] == "web_search"));
            assert!(row.request["reasoning"].get("effort").is_none());
            results.push(json!({"http":400,"client_usable":false}));
            continue;
        }
        assert_eq!(row.status, 200);
        assert!(row.chunks > 2);
        assert_eq!(
            row.request["reasoning"],
            if client == "pi" {
                json!({"effort":"medium","summary":"auto"})
            } else {
                json!({"summary":"auto"})
            }
        );
        assert_eq!(
            row.request["include"],
            json!(["reasoning.encrypted_content"])
        );
        assert_eq!(row.request["store"], false);
        assert_eq!(row.request["stream"], true);
        // The production stream includes legal transport keepalive comments during cancellation.
        // Keep unknown/non-comment records visible to the strict existing frame oracle.
        let frames = evidence::frames(row)?;
        let completed = frames
            .iter()
            .filter(|v| v["type"] == "response.completed")
            .count();
        let failed = frames
            .iter()
            .filter(|v| v["type"] == "response.failed")
            .count();
        assert!(frames.iter().all(|v| v["type"] != "error"));
        assert_eq!(
            (completed, failed),
            match scenario {
                "failure" => (0, 1),
                "cancel" => (0, 0),
                _ => (1, 0),
            }
        );
        if scenario == "standard" || scenario == "failure" {
            let terminal = frames.last().ok_or("terminal")?;
            let (kind, status) = if scenario == "failure" {
                ("response.failed", "failed")
            } else {
                ("response.completed", "completed")
            };
            assert_eq!(terminal["type"], kind);
            assert_eq!(terminal["response"]["status"], status);
            assert_eq!(terminal["response"]["model"], "grok-4.5");
            assert_eq!(terminal["response"]["usage"]["input_tokens"], 20);
            assert_eq!(terminal["response"]["usage"]["output_tokens"], 7);
        }
        results.push(
            json!({"http":row.status,"chunks":row.chunks,"completed":completed,"failed":failed,
                "complete_terminal":scenario == "standard" || scenario == "failure",
                "http_body_eof_observed":row.body_complete}),
        );
    }
    Ok(json!(results))
}

fn combined_codex_request(request: &Value) -> bool {
    let Some(tools) = request["tools"].as_array() else {
        return false;
    };
    tools
        .iter()
        .any(|tool| tool["type"] == "namespace" && tool["name"] == "multi_agent_v1")
        && tools
            .iter()
            .any(|tool| *tool == json!({"type":"web_search","external_web_access":false}))
        && request["parallel_tool_calls"] == true
        && request.get("client_metadata").is_some()
        && request["input"].as_array().is_some_and(|input| {
            input.iter().any(|item| {
                matches!(item["role"].as_str(), Some("user" | "developer" | "system"))
                    && item.get("id").is_some()
            })
        })
}

#[actix_web::test]
async fn combined_codex_gate_requires_actual_field_coexistence() -> TestResult {
    let request = json!({"tools":[
        {"type":"namespace","name":"multi_agent_v1","tools":[{"type":"function","name":"spawn_agent","parameters":{"type":"object"},"strict":false}]},
        {"type":"web_search","external_web_access":false}],"parallel_tool_calls":true,
        "client_metadata":{},"input":[{"type":"message","role":"user","id":"gate-user","content":"task"}]});
    assert!(combined_codex_request(&request));
    for field in ["tools", "parallel_tool_calls", "client_metadata", "input"] {
        let mut missing = request.clone();
        missing
            .as_object_mut()
            .ok_or("fixture object")?
            .remove(field);
        assert!(!combined_codex_request(&missing), "missing {field}");
    }
    for index in [0, 1] {
        let mut missing = request.clone();
        missing["tools"]
            .as_array_mut()
            .ok_or("fixture tools")?
            .remove(index);
        assert!(!combined_codex_request(&missing));
    }
    let mut changed = request.clone();
    changed["tools"][1]["external_web_access"] = json!(true);
    assert!(!combined_codex_request(&changed));
    changed = request.clone();
    changed["parallel_tool_calls"] = json!(false);
    assert!(!combined_codex_request(&changed));
    changed = request;
    changed["input"][0]
        .as_object_mut()
        .ok_or("fixture message")?
        .remove("id");
    assert!(!combined_codex_request(&changed));
    Ok(())
}

pub(super) fn assert_history(received: &[Value], directory: &std::path::Path) -> TestResult {
    assert_eq!(
        std::fs::read_to_string(directory.join("work/result.txt"))?,
        "42\n"
    );
    for (round, body) in received.iter().enumerate() {
        assert_eq!(body["model"], "grok-4.5");
        assert_eq!(
            body["reasoning"],
            json!({"effort":"medium","summary":"auto"})
        );
        assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
        let input = body["input"].as_array().ok_or("history")?;
        let reasoning: Vec<_> = input.iter().filter(|v| v["type"] == "reasoning").collect();
        assert_eq!(reasoning.len(), round);
        for (index, item) in reasoning.iter().enumerate() {
            assert_eq!(item["id"], format!("reason-62-{index}"));
            assert_eq!(
                item["summary"],
                json!([{"type":"summary_text","text":format!("Plan {index}")} ])
            );
        }
    }
    let input = received[4]["input"].as_array().ok_or("follow-up history")?;
    for (id, name, args, result) in [
        ("call-62-1", "read", json!({"path":"seed.txt"}), "41\n"),
        (
            "call-62-2",
            "bash",
            json!({"command":"n=$(cat seed.txt); echo $((n + 1)) > result.txt; cat result.txt"}),
            "42\n",
        ),
    ] {
        let call = input
            .iter()
            .position(|v| v["type"] == "function_call" && v["call_id"] == id)
            .ok_or("exact call ID")?;
        let output = input
            .iter()
            .position(|v| v["type"] == "function_call_output" && v["call_id"] == id)
            .ok_or("exact result ID")?;
        assert!(call < output);
        assert_eq!(input[call]["name"], name);
        assert_eq!(
            serde_json::from_str::<Value>(input[call]["arguments"].as_str().ok_or("arguments")?)?,
            args
        );
        assert_eq!(input[output]["output"], result);
    }
    let receipt: Value = serde_json::from_slice(&std::fs::read(directory.join("client.json"))?)?;
    assert_eq!(receipt["turn_outcomes"], json!(["stop", "stop", "stop"]));
    Ok(())
}

pub(super) fn assert_codex_terminal(directory: &std::path::Path, scenario: &str) -> TestResult {
    let receipt: Value = serde_json::from_slice(&std::fs::read(directory.join("client.json"))?)?;
    assert_eq!(
        receipt["exit"], 0,
        "native client shutdown must be normal after observation"
    );
    assert_eq!(
        receipt["turn_outcomes"],
        json!([if scenario == "failure" {
            "failed"
        } else {
            "interrupted"
        }])
    );
    assert!(
        !directory.join("work/result.txt").exists(),
        "failed/cancelled turns execute no scripted tool"
    );
    let events: Vec<Value> = receipt["stdout"]
        .as_str()
        .ok_or("actual Codex RPC output")?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let terminals: Vec<_> = events
        .iter()
        .filter(|v| v["method"] == "turn/completed")
        .collect();
    assert_eq!(terminals.len(), 1);
    assert!(
        events
            .iter()
            .all(|v| v["params"]["item"]["type"] != "commandExecution")
    );
    let errors: Vec<_> = events.iter().filter(|v| v["method"] == "error").collect();
    if scenario == "failure" {
        assert_eq!(
            errors.len(),
            1,
            "actual Codex observes a single controlled failure"
        );
        assert_eq!(errors[0]["params"]["willRetry"], false);
    } else {
        assert!(errors.is_empty());
    }
    Ok(())
}
