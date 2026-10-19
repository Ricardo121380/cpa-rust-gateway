//! Public pre-send, strict response, persistence, confirmation and cancellation boundaries.
use super::*;

fn body(streaming: bool) -> Value {
    json!({"model":"p12-test-model","max_tokens":19,"stream":streaming,
    "messages":[{"role":"user","content":"question"}]})
}

async fn refused(streaming: bool, wire: String, request: Value, received: usize) -> TestResult {
    let peer = ModelPeer::start_wire(streaming, wire)?;
    let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
    let recording = Recording::start(&fixture, false)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let reply = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(request)
            .to_request(),
    )
    .await;
    let status = reply.status();
    let bytes = test::read_body(reply).await;
    assert_eq!(
        peer.requests.lock().map_err(|_| "receiver")?.len(),
        received
    );
    if received == 0 {
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
    } else if streaming {
        assert_eq!(status, StatusCode::OK);
        assert_failed(&bytes)?;
    } else {
        assert_ne!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
    }
    assert!(!String::from_utf8_lossy(&bytes).contains("计划"));
    assert_eq!(active_leases(&fixture), 0);
    drop(app);
    let database = recording.finish(&fixture, 0).await?;
    if received == 0 {
        let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r.event(), GatewayEvent::Attempt(_) | GatewayEvent::Usage(_)))
        );
        let worker = crate::billing_worker::BillingWorker::start(
            database.clone(),
            Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
        );
        worker.stop().await.map_err(|()| "billing")?;
        assert!(
            SqliteBillingLedger::open_read_only(&database)?
                .list_bounded(10)?
                .is_empty()
        );
    }
    Ok(())
}
pub(super) fn assert_failed(bytes: &[u8]) -> TestResult {
    let frames = public_frames(bytes)?;
    assert_eq!(
        frames.iter().filter(|v| v["type"] == "error").count(),
        1,
        "{}",
        String::from_utf8_lossy(bytes)
    );
    assert!(
        frames
            .iter()
            .all(|v| v["type"] != "message_stop" && v["type"] != "message_delta")
    );
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_known_controls_and_histories_reject_before_send() -> TestResult {
    for streaming in [false, true] {
        let mut requests = Vec::new();
        let mut thinking = body(streaming);
        thinking["max_tokens"] = json!(4096);
        thinking["thinking"] = json!({"type":"enabled","budget_tokens":1024});
        requests.push(thinking);
        let mut control = body(streaming);
        control["top_k"] = json!(3);
        requests.push(control);
        let mut orphan = body(streaming);
        orphan["messages"] = json!([{"role":"user","content":[{"type":"tool_result","tool_use_id":"missing","content":"result"}]}]);
        requests.push(orphan);
        let mut signature = body(streaming);
        signature["messages"] = json!([{"role":"assistant","content":[{"type":"thinking","thinking":"private history","signature":"unowned"}]},{"role":"user","content":"next"}]);
        requests.push(signature);
        for request in requests {
            refused(streaming, tool_rounds::wire(1, streaming)?, request, 0).await?;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_unknown_parts_empty_state_and_identity_conflicts_fail() -> TestResult
{
    for streaming in [false, true] {
        for case in [
            "unknown_part",
            "null_summary",
            "duplicate",
            "cipher",
            "signature",
            "body",
            "extra_usage",
            "model",
        ] {
            eprintln!("Messages summary fault: {case}, streaming={streaming}");
            let mut value: Value = serde_json::from_str(&tool_rounds::wire(1, false)?)?;
            match case {
                "unknown_part" => {
                    value["output"][0]["summary"][0]["type"] = json!("unreviewed_part");
                }
                "null_summary" => value["output"][0]["summary"] = Value::Null,
                "duplicate" => value["output"][2]["id"] = value["output"][0]["id"].clone(),
                "cipher" => value["output"][0]["encrypted_content"] = json!("unowned ciphertext"),
                "signature" => value["output"][0]["signature"] = json!("unowned signature"),
                "body" => {
                    value["output"][0]["content"] =
                        json!([{"type":"reasoning_text","text":"unreviewed body"}]);
                }
                "extra_usage" => value["usage"]["cost_in_usd_ticks"] = json!(0),
                _ => value["model"] = json!("unproven-native-model"),
            }
            let wire = if streaming {
                frame("response.created", &created()) + &frame("response.completed", &value)
            } else {
                value.to_string()
            };
            refused(streaming, wire, body(streaming), 1).await?;
        }
    }
    // Contradictions discovered only after valid deltas and done items, including an illegal
    // empty snapshot. All use the same public production path and one independent receive.
    for case in [
        "empty_terminal",
        "text_conflict",
        "done_twice",
        "wrong_part_index",
    ] {
        let mut wire = tool_rounds::wire(1, true)?;
        let terminal = wire.rfind("event: response.completed").ok_or("terminal")?;
        let mut value: Value = serde_json::from_str(&tool_rounds::wire(1, false)?)?;
        match case {
            "empty_terminal" => value["output"][0]["summary"] = json!([]),
            "text_conflict" => value["output"][0]["summary"][0]["text"] = json!("contradiction"),
            "done_twice" => {
                wire.insert_str(
                    terminal,
                    &native_event(
                        "response.output_item.done",
                        json!({"output_index":0,"item":value["output"][0]}),
                    ),
                );
            }
            _ => {
                wire = wire.replacen("\"summary_index\":1", "\"summary_index\":63", 1);
            }
        }
        if matches!(case, "empty_terminal" | "text_conflict") {
            wire.truncate(terminal);
            wire += &frame("response.completed", &value);
        }
        refused(true, wire, body(true), 1).await?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) enum Fault {
    Hold,
    QueueOnly,
    Conflict,
    Identity,
    Timeout,
}
pub(super) struct SummaryReceipt {
    inner: Arc<dyn GatewayEventSink>,
    database: std::path::PathBuf,
    fault: Fault,
    pub(super) entered: tokio::sync::Notify,
    pub(super) release: tokio::sync::Notify,
    pub(super) submissions: AtomicUsize,
}
impl GatewayEventSink for SummaryReceipt {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        self.inner.try_emit(event)
    }
    fn emit_confirmed(&self, event: GatewayEvent) -> gateway_core::EventEmissionFuture<'_> {
        self.inner.emit_confirmed(event)
    }
    fn emit_summary_evidence_confirmed(
        &self,
        attempt: gateway_core::AttemptEvent,
        evidence: gateway_core::MessagesSummaryEvidence,
    ) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            self.submissions.fetch_add(1, Ordering::SeqCst);
            match self.fault {
                Fault::QueueOnly => EventEmission::Enqueued,
                Fault::Hold => {
                    let result = self
                        .inner
                        .emit_summary_evidence_confirmed(attempt, evidence)
                        .await;
                    self.entered.notify_one();
                    self.release.notified().await;
                    result
                }
                Fault::Conflict => {
                    assert_eq!(
                        self.inner
                            .emit_summary_evidence_confirmed(attempt.clone(), evidence.clone())
                            .await,
                        EventEmission::Persisted
                    );
                    let mut items = evidence.items().to_vec();
                    let mut item: Value = serde_json::from_str(items[0].item.get())
                        .map_err(|_| ())
                        .unwrap_or(Value::Null);
                    item["summary"][0]["text"] = json!("competing summary");
                    let changed = gateway_core::RawJson::from_json_string(item.to_string());
                    let Ok(changed) = changed else {
                        return EventEmission::PersistenceUnavailable;
                    };
                    items[0].item = changed;
                    let changed = gateway_core::MessagesSummaryEvidence::try_new(
                        evidence.source(),
                        evidence.response_id().clone(),
                        evidence.completeness(),
                        evidence.confirmed_at_ms(),
                        items,
                    );
                    let Ok(changed) = changed else {
                        return EventEmission::PersistenceUnavailable;
                    };
                    self.inner
                        .emit_summary_evidence_confirmed(attempt, changed)
                        .await
                }
                Fault::Identity => {
                    let Some(mut identity) = attempt.execution_identity().cloned() else {
                        return EventEmission::PersistenceUnavailable;
                    };
                    identity.channel = "grok.official".into();
                    self.inner
                        .emit_summary_evidence_confirmed(
                            attempt.with_execution_identity(identity),
                            evidence,
                        )
                        .await
                }
                Fault::Timeout => {
                    let Ok(connection) = gateway_store::open(&self.database) else {
                        return EventEmission::PersistenceUnavailable;
                    };
                    if connection.execute_batch("BEGIN IMMEDIATE").is_err() {
                        return EventEmission::PersistenceUnavailable;
                    }
                    let result = self
                        .inner
                        .emit_summary_evidence_confirmed(attempt, evidence)
                        .await;
                    let _ = connection.execute_batch("ROLLBACK");
                    result
                }
            }
        })
    }
}
pub(super) fn wrap(
    fixture: &Fixture,
    database: &std::path::Path,
    fault: Fault,
) -> Result<Arc<SummaryReceipt>, Box<dyn Error>> {
    let inner = fixture
        .events
        .1
        .lock()
        .map_err(|_| "sink")?
        .take()
        .ok_or("writer")?;
    let receipt = Arc::new(SummaryReceipt {
        inner,
        database: database.to_path_buf(),
        fault,
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        submissions: AtomicUsize::new(0),
    });
    *fixture.events.1.lock().map_err(|_| "sink")? = Some(receipt.clone());
    Ok(receipt)
}

#[actix_web::test]
async fn public_build_messages_storage_crypto_conflict_and_confirmation_fail_closed() -> TestResult
{
    for streaming in [false, true] {
        for case in [
            "missing_keys",
            "write_failure",
            "queue_only",
            "conflict",
            "identity",
            "timeout",
        ] {
            let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
            let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
            let recording = Recording::with_keys(
                &fixture,
                false,
                if case == "missing_keys" {
                    None
                } else {
                    Some(test_secret_store()?)
                },
            )?;
            if case == "write_failure" {
                gateway_store::open(&recording.database)?.execute_batch("CREATE TRIGGER fail_summary BEFORE INSERT ON messages_summary_evidence BEGIN SELECT RAISE(ABORT,'controlled summary write failure'); END;")?;
            }
            let receipt = match case {
                "queue_only" => Some(wrap(&fixture, &recording.database, Fault::QueueOnly)?),
                "conflict" => Some(wrap(&fixture, &recording.database, Fault::Conflict)?),
                "identity" => Some(wrap(&fixture, &recording.database, Fault::Identity)?),
                "timeout" => Some(wrap(&fixture, &recording.database, Fault::Timeout)?),
                _ => None,
            };
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/messages")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(body(streaming))
                    .to_request(),
            )
            .await;
            let status = reply.status();
            let request_id = reply
                .headers()
                .get("x-request-id")
                .ok_or("id")?
                .to_str()?
                .to_owned();
            let bytes = test::read_body(reply).await;
            assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
            if streaming {
                assert_eq!(status, StatusCode::OK);
                assert_failed(&bytes)?;
            } else {
                assert_ne!(status, StatusCode::OK);
            }
            assert!(
                String::from_utf8_lossy(&bytes).contains("recording"),
                "{case}: {}",
                String::from_utf8_lossy(&bytes)
            );
            assert!(!String::from_utf8_lossy(&bytes).contains("计划"));
            if let Some(receipt) = &receipt {
                assert_eq!(receipt.submissions.load(Ordering::SeqCst), 1);
            }
            if case == "conflict" {
                let record = retained(&recording.database, &request_id)?;
                assert!(record.evidence.items()[0].item.get().contains("计划"));
                assert!(!record.evidence.items()[0].item.get().contains("competing"));
            }
            drop(app);
            drop(receipt);
            if case == "write_failure" {
                gateway_store::open(&recording.database)?
                    .execute_batch("DROP TRIGGER fail_summary")?;
            }
            let quarantined =
                u64::from(!matches!(case, "queue_only" | "timeout" | "write_failure"));
            let database = recording.finish(&fixture, quarantined).await?;
            let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
            assert_eq!(
                rows.iter()
                    .filter(
                        |row| matches!(row.event(),GatewayEvent::RequestFinished(finished)
                if finished.outcome==gateway_core::RequestOutcome::Failed
                && finished.error_code==Some(GatewayErrorCode::RecordingUnavailable))
                    )
                    .count(),
                1
            );
            accounting(&database, 1).await?;
            assert_eq!(active_leases(&fixture), 0);
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_success_waits_for_confirmed_summary_receipt() -> TestResult {
    for streaming in [false, true] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
        let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
        let recording = Recording::start(&fixture, false)?;
        let receipt = wrap(&fixture, &recording.database, Fault::Hold)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let request = test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(body(streaming))
            .to_request();
        if streaming {
            let reply = test::call_service(&app, request).await;
            let request_id = reply
                .headers()
                .get("x-request-id")
                .ok_or("id")?
                .to_str()?
                .to_owned();
            let read = test::read_body(reply);
            tokio::pin!(read);
            tokio::select! {biased;_=&mut read=>return Err("message_stop before summary receipt".into()),()=receipt.entered.notified()=>{}}
            assert_eq!(active_leases(&fixture), 0);
            retained(&recording.database, &request_id)?;
            receipt.release.notify_one();
            let bytes = read.await;
            tool_rounds::client_result(&bytes, true)?;
        } else {
            let reply = test::call_service(&app, request);
            tokio::pin!(reply);
            tokio::select! {biased;_=&mut reply=>return Err("JSON success before summary receipt".into()),()=receipt.entered.notified()=>{}}
            assert_eq!(active_leases(&fixture), 0);
            let rows = SqliteEventStore::open_read_only(&recording.database)?.list_events()?;
            let request = rows
                .iter()
                .find_map(|r| match r.event() {
                    GatewayEvent::Request(v) => Some(v),
                    _ => None,
                })
                .ok_or("request")?;
            retained(&recording.database, request.request_id().as_str())?;
            receipt.release.notify_one();
            let reply = reply.await;
            assert_eq!(reply.status(), StatusCode::OK);
            test::read_body(reply).await;
        }
        assert_eq!(receipt.submissions.load(Ordering::SeqCst), 1);
        drop(app);
        drop(receipt);
        let database = recording.finish(&fixture, 0).await?;
        accounting(&database, 1).await?;
    }
    Ok(())
}
