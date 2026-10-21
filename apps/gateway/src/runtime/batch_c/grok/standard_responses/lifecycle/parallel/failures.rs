//! A failed batch never exposes executable done, persists a successful root, or retries.
use super::*;

fn corrupt(label: &str, namespaced: bool) -> Result<String, Box<dyn Error>> {
    let valid = wire(0, namespaced, true, false)?;
    let mut records: Vec<Value> = valid
        .split("\n\n")
        .filter_map(|s| s.lines().find_map(|s| s.strip_prefix("data: ")))
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let added = records
        .iter()
        .position(|v| v["type"] == "response.output_item.added" && v["output_index"] == 2)
        .ok_or("added")?;
    let delta = records
        .iter()
        .position(|v| {
            v["type"] == "response.function_call_arguments.delta" && v["output_index"] == 2
        })
        .ok_or("delta")?;
    let args = records
        .iter()
        .position(|v| {
            v["type"] == "response.function_call_arguments.done" && v["output_index"] == 2
        })
        .ok_or("args done")?;
    let done = records
        .iter()
        .position(|v| v["type"] == "response.output_item.done" && v["output_index"] == 2)
        .ok_or("item done")?;
    let first_done = records
        .iter()
        .position(|v| v["type"] == "response.output_item.done" && v["output_index"] == 1)
        .ok_or("first done")?;
    let terminal = records.len() - 1;
    match label {
        "duplicate-item" => records[added]["item"]["id"] = json!("parallel-item-0"),
        "duplicate-call" => records[added]["item"]["call_id"] = json!("parallel-call-0"),
        "duplicate-index" => records[added]["output_index"] = json!(1),
        "delta-index" => records[delta]["output_index"] = json!(1),
        "delta-call" => records[delta]["call_id"] = json!("parallel-call-0"),
        "arguments-conflict" => records[args]["arguments"] = json!(r#"{"changed":true}"#),
        "done-name" => records[done]["item"]["name"] = json!("combine"),
        "done-call" => records[done]["item"]["call_id"] = json!("parallel-call-0"),
        "done-namespace" => records[done]["item"]["namespace"] = json!("left"),
        "terminal-arguments" => {
            records[terminal]["response"]["output"][2]["arguments"] = json!(r#"{"changed":true}"#);
        }
        "terminal-name" => records[terminal]["response"]["output"][2]["name"] = json!("combine"),
        "terminal-namespace" => {
            records[terminal]["response"]["output"][2]["namespace"] = json!("left");
        }
        "terminal-missing" => {
            records[terminal]["response"]["output"]
                .as_array_mut()
                .ok_or("output")?
                .pop();
        }
        "terminal-order" => records[terminal]["response"]["output"]
            .as_array_mut()
            .ok_or("output")?
            .swap(1, 2),
        "duplicate-done" => records.insert(done + 1, records[done].clone()),
        "delta-after-done" => records.insert(done + 1, records[delta].clone()),
        "eof-after-first-done" => records.truncate(first_done + 1),
        "failed-after-first-done" => {
            records.truncate(first_done + 1);
            records.push(json!({"type":"response.failed","response":{"id":"parallel-native-0","model":"grok-4.5","status":"failed","output":[],"error":{"code":"server_error","message":"synthetic failure"}}}));
        }
        "incomplete-batch" => {
            records[terminal]["type"] = json!("response.incomplete");
            records[terminal]["response"]["status"] = json!("incomplete");
            records[terminal]["response"]["incomplete_details"] =
                json!({"reason":"max_output_tokens"});
        }
        "wrong-usage-total" => records[terminal]["response"]["usage"]["total_tokens"] = json!(28),
        "arguments-array" | "arguments-duplicate" => {
            // No delta is removed from an observed capture: these are independent purpose-built counterexamples.
            let bad = if label == "arguments-array" {
                "[]"
            } else {
                r#"{"seed":19,"seed":22}"#
            };
            records[args]["arguments"] = json!(bad);
        }
        _ => return Err("unknown corruption".into()),
    }
    let mut result = String::new();
    for record in records {
        result += &event(record["type"].as_str().ok_or("type")?, record.clone());
    }
    Ok(result)
}

pub(super) fn assert_no_completion(bytes: &[u8]) -> TestResult {
    let frames = public_frames(bytes)?;
    assert_eq!(
        frames
            .iter()
            .filter(|v| v["type"] == "response.failed")
            .count(),
        1,
        "{}",
        String::from_utf8_lossy(bytes)
    );
    assert!(
        frames.iter().all(|v| v["type"] != "response.completed"
            && v["type"] != "response.function_call_arguments.done"
            && !(v["type"] == "response.output_item.done" && v["item"]["type"] == "function_call")),
        "{}",
        String::from_utf8_lossy(bytes)
    );
    // A failed terminal can retain provisional output, but it cannot mark a function completed.
    for item in frames
        .iter()
        .filter(|v| v["type"] == "response.failed")
        .filter_map(|v| v["response"]["output"].as_array())
        .flatten()
    {
        if item["type"] == "function_call" {
            assert_ne!(item["status"], "completed");
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_parallel_conflicts_never_complete_or_store_or_retry() -> TestResult {
    for size in [1, 7, 31] {
        for label in [
            "duplicate-item",
            "duplicate-call",
            "duplicate-index",
            "delta-index",
            "delta-call",
            "arguments-conflict",
            "done-name",
            "done-call",
            "done-namespace",
            "terminal-arguments",
            "terminal-name",
            "terminal-namespace",
            "terminal-missing",
            "terminal-order",
            "duplicate-done",
            "delta-after-done",
            "eof-after-first-done",
            "failed-after-first-done",
            "incomplete-batch",
            "wrong-usage-total",
            "arguments-array",
            "arguments-duplicate",
        ] {
            let namespaced = label.contains("namespace");
            run_failure(
                label,
                corrupt(label, namespaced)?,
                namespaced,
                true,
                size,
                None,
            )
            .await?;
        }
    }
    // The exact same flat batch with unadmitted Usage must hold both functions, in JSON and SSE.
    for streaming in [false, true] {
        for raw in [
            r#"{ "input_tokens":20,"output_tokens":7,"total_tokens":27,"cost_in_usd_ticks":1 }"#,
            r#"{ "input_tokens":20,"output_tokens":7,"total_tokens":27,"parallel_tool_usage":{"count":2} }"#,
        ] {
            let original = wire(0, false, streaming, false)?;
            let bad = original.replace(
                r#""usage":{"input_tokens":20,"output_tokens":7,"total_tokens":27}"#,
                &format!(r#""usage":{raw}"#),
            );
            assert_ne!(bad, original);
            run_failure("unadmitted-usage", bad, false, streaming, 7, Some(raw)).await?;
        }
    }
    Ok(())
}

async fn run_failure(
    label: &str,
    wire: String,
    namespaced: bool,
    streaming: bool,
    size: usize,
    raw: Option<&str>,
) -> TestResult {
    let peer = ModelPeer::start_wire(streaming, wire)?;
    let fixture = fixture_for(namespaces::transport(&peer, size)?, true, true, |p| {
        p.build_parallel_local_controlled = true;
        p.build_namespaces_local_controlled = namespaced;
    })?;
    let recording = Recording::start(&fixture, false)?;
    if label == "lost-confirmation" {
        let mut sink = fixture.events.1.lock().map_err(|_| "sink")?;
        let inner = sink.take().ok_or("real writer")?;
        *sink = Some(Arc::new(LostUsageReceipt(inner)));
    }
    let (url, server) = namespaces::listener(&fixture)?;
    let mut request = body(namespaced, streaming);
    request["store"] = json!(true);
    let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
    if streaming {
        assert_eq!(status, 200, "{label}");
        assert_no_completion(&bytes)?;
    } else {
        assert_eq!(
            status,
            if label == "lost-confirmation" {
                503
            } else {
                502
            },
            "{label}"
        );
        if label == "lost-confirmation" {
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes)?["error"]["code"],
                "RecordingUnavailable"
            );
        }
    }
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 1, "{label}");
    assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
    assert_eq!(fixture.events.attempts()?, 1);
    assert_eq!(active_leases(&fixture), 0);
    assert!(read_stored(&fixture, "parallel-native-0")?.is_none());
    let retry = json!({"model":"p12-test-model","previous_response_id":"parallel-native-0","input":"cannot continue a failed batch"});
    assert_eq!(namespaces::request(&fixture, &url, &retry).await?.0, 404);
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.events.attempts()?, 1);
    server.stop(false).await;
    let database = recording.finish(&fixture, 0).await?;
    let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
    let usages: Vec<_> = rows
        .iter()
        .filter_map(|r| {
            if let GatewayEvent::Usage(v) = r.event() {
                Some(v)
            } else {
                None
            }
        })
        .collect();
    assert!(usages.len() <= 1);
    assert_counts(&database, 1, usages.len()).await?;
    if let Some(raw) = raw {
        assert_eq!(usages.len(), 1);
        let usage = usages[0];
        let record = SqliteNativeUsageReader::open(&database, test_secret_store()?)?
            .get_owned(
                &owner(&fixture)?,
                usage.request_id(),
                usage.attempt_id().ok_or("exact attempt")?,
                system_now_ms_runtime()?,
            )?
            .ok_or("encrypted failed usage")?;
        assert_eq!(record.evidence.raw_json(), raw);
        assert_eq!(record.completeness, "complete_usage_object");
    }
    println!(
        "CPAR62_PARALLEL_POST_REJECT {}",
        json!({"label":label,"streaming":streaming,"fragment_bytes":size,"receiver":1,"attempts":1,
        "function_completion":0,"completed":0,"stored_root":false,"continuation_status":404,"observed_usage":usages.len(),"cost":"UNKNOWN","cost_in_usd_ticks":"NOT_ADMITTED","exact_failed_usage":raw.is_some()})
    );
    Ok(())
}

#[actix_web::test]
async fn public_build_parallel_fin_and_reset_cancel_before_cleanup() -> TestResult {
    for reset in [false, true] {
        let partial = cancellation_wire()?;
        let peer = SocketPeer::start(partial, PeerEnd::Hold)?;
        let fixture = fixture_for(
            peer.transport(Duration::from_secs(5), Duration::from_secs(10))?,
            true,
            true,
            |p| p.build_parallel_local_controlled = true,
        )?;
        let recording = Recording::start(&fixture, false)?;
        let mut request = body(false, true);
        request["store"] = json!(true);
        let (mut socket, server) =
            public_socket_request(&fixture, "/v1/responses", request).await?;
        let wire = read_public_until(&mut socket, "parallel-cancel-after-first-done").await?;
        assert!(
            !wire.contains("response.function_call_arguments.done")
                && !wire.contains("response.completed")
        );
        assert_eq!(active_leases(&fixture), 1);
        let start = std::time::Instant::now();
        let socket = if reset {
            socket.set_zero_linger()?;
            drop(socket);
            None
        } else {
            socket.shutdown().await?;
            Some(socket)
        };
        let fast = tokio::time::timeout(Duration::from_secs(1), async {
            while peer.closed.load(Ordering::SeqCst) != 1 || active_leases(&fixture) != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .is_ok();
        assert!(
            wait_until(|| peer.closed.load(Ordering::SeqCst) == 1 && active_leases(&fixture) == 0)
                .await
        );
        let elapsed = start.elapsed().as_millis();
        assert!(wait_until(||SqliteEventStore::open_read_only(&recording.database).and_then(|s|s.list_events()).is_ok_and(|rows|rows.iter().any(|r|matches!(r.event(),GatewayEvent::RequestFinished(f) if f.outcome==gateway_core::RequestOutcome::Cancelled)))).await);
        assert!(read_stored(&fixture, "parallel-native-0")?.is_none());
        assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
        assert_eq!(fixture.events.attempts()?, 1);
        println!(
            "CPAR62_PARALLEL_CANCEL {}",
            json!({"reset":reset,"real_delta":true,"lease_before":1,"closed_before_cleanup":peer.closed.load(Ordering::SeqCst),
            "lease_after":active_leases(&fixture),"receiver":1,"attempts":1,"fast_probe_passed":fast,"release_observed_ms":elapsed,"usage":"UNKNOWN","cost":"UNKNOWN","stored_root":false})
        );
        assert!(fast, "original one-second release probe");
        drop(socket);
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        assert_counts(&database, 1, 0).await?;
    }
    Ok(())
}

fn cancellation_wire() -> Result<String, Box<dyn Error>> {
    let valid = wire(0, false, true, false)?;
    let mut prefix = String::new();
    for record in valid.split("\n\n").filter(|r| !r.is_empty()) {
        let data = record
            .lines()
            .find_map(|s| s.strip_prefix("data: "))
            .ok_or("data")?;
        let value: Value = serde_json::from_str(data)?;
        if value["type"] == "response.output_item.added" && value["output_index"] == 1 {
            break;
        }
        prefix += record;
        prefix += "\n\n";
    }
    let items = output(0, false);
    for index in [1, 2] {
        let mut start = items[index].clone();
        start["status"] = json!("in_progress");
        start["arguments"] = json!("");
        prefix += &event(
            "response.output_item.added",
            json!({"output_index":index,"item":start}),
        );
    }
    prefix += &event(
        "response.function_call_arguments.delta",
        json!({"item_id":items[1]["id"],"output_index":1,"delta":items[1]["arguments"]}),
    );
    prefix += &event(
        "response.function_call_arguments.done",
        json!({"item_id":items[1]["id"],"output_index":1,"arguments":items[1]["arguments"]}),
    );
    prefix += &event(
        "response.output_item.done",
        json!({"output_index":1,"item":items[1]}),
    );
    prefix += &event(
        "response.function_call_arguments.delta",
        json!({"item_id":items[2]["id"],"output_index":2,"delta":r#"{"seed":22,"marker":"parallel-cancel-after-first-done"#}),
    );
    Ok(prefix)
}

struct LostUsageReceipt(Arc<dyn GatewayEventSink>);
impl GatewayEventSink for LostUsageReceipt {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        self.0.try_emit(event)
    }
    fn emit_confirmed(&self, event: GatewayEvent) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            let usage = matches!(event, GatewayEvent::Usage(_));
            let result = self.0.emit_confirmed(event).await;
            if usage && result == EventEmission::Persisted {
                EventEmission::PersistenceUnavailable
            } else {
                result
            }
        })
    }
}

#[actix_web::test]
async fn public_build_parallel_usage_confirmation_failure_never_completes() -> TestResult {
    for streaming in [false, true] {
        run_failure(
            "lost-confirmation",
            wire(0, false, streaming, false)?,
            false,
            streaming,
            7,
            None,
        )
        .await?;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_parallel_json_conflicts_fail_without_successful_roots() -> TestResult {
    for label in [
        "duplicate-item",
        "duplicate-call",
        "undeclared-function",
        "malformed-arguments",
        "namespace-without-output-permission",
    ] {
        let mut response: Value = serde_json::from_str(&wire(0, false, false, false)?)?;
        match label {
            "duplicate-item" => response["output"][2]["id"] = response["output"][1]["id"].clone(),
            "duplicate-call" => {
                response["output"][2]["call_id"] = response["output"][1]["call_id"].clone();
            }
            "undeclared-function" => response["output"][2]["name"] = json!("unseen_function"),
            "malformed-arguments" => response["output"][2]["arguments"] = json!("["),
            _ => response["output"][2]["namespace"] = json!("unqualified"),
        }
        run_failure(label, response.to_string(), false, false, 7, None).await?;
    }
    Ok(())
}
