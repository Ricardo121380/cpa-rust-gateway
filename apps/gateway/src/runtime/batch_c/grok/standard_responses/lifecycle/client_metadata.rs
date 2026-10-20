//! C4 is a field-isolated synthetic request, never an original Codex compatibility claim.
use super::*;
use gateway_store::request_metadata::SqliteRequestMetadataReader;

const BLOB: &str = r#"{ "installation_id":"C4_PRIVATE_INSTALL", "session_id":"C4_PRIVATE_SESSION", "thread_id":"C4_PRIVATE_THREAD", "turn_id":"C4_PRIVATE_TURN", "root_turn_id":"C4_PRIVATE_ROOT", "window_id":"C4_PRIVATE_WINDOW", "window_number":0, "context_window_id":"C4_PRIVATE_CONTEXT", "agent_name":"/root", "request_kind":"turn", "thread_source":"user", "turn_trigger":"exec", "sandbox":"seatbelt", "sandbox_mode":"workspace-write", "auto_review_enabled":false, "node_repl_auto_review_required":false, "node_repl_disabled":false, "turn_started_at_unix_ms":1792342567139, "analytics_enabled":true, "model":"C4_PRIVATE_MODEL" }"#;

fn metadata() -> Value {
    json!({"root_turn_id":"C4_PRIVATE_ROOT","session_id":"C4_PRIVATE_SESSION",
        "thread_id":"C4_PRIVATE_THREAD","turn_id":"C4_PRIVATE_TURN",
        "x-codex-installation-id":"C4_PRIVATE_INSTALL","x-codex-window-id":"C4_PRIVATE_WINDOW",
        "x-codex-turn-metadata":BLOB})
}

#[actix_web::test]
async fn public_build_c4_metadata_is_local_and_answer_usage_are_preserved() -> TestResult {
    for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire_response(1, streaming)?)?;
        let fixture = lifecycle_fixture(namespaces::transport(&peer, size)?)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let body = json!({"model":"p12-test-model","input":"C4 synthetic question",
            "stream":streaming,"client_metadata":metadata()});
        let (status, bytes) = namespaces::request(&fixture, &url, &body).await?;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
        let terminal = namespaces::terminal(&bytes, streaming)?;
        assert_eq!(terminal["usage"]["input_tokens"], 20);
        assert_eq!(terminal["usage"]["output_tokens"], 7);
        assert_eq!(terminal["output"], tool_rounds::output(1));
        {
            let sent = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(sent.len(), 1);
            assert!(sent[0].body.get("client_metadata").is_none());
            assert!(!String::from_utf8_lossy(&sent[0].wire).contains("C4_PRIVATE"));
            assert!(!String::from_utf8_lossy(&sent[0].wire).contains("client_metadata"));
            assert_eq!(sent[0].model_override, "grok-4.5");
            assert_eq!(
                sent[0].body,
                json!({"model":"grok-4.5","input":"C4 synthetic question","stream":streaming})
            );
        }
        assert_capture(&fixture, &metadata().to_string(), 1)?;
        assert!(!String::from_utf8_lossy(&bytes).contains("C4_PRIVATE"));
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        namespaces::assert_recorded(&database, 1).await?;
        println!(
            "CPAR62_C4_RECEIPT {}",
            json!({"streaming":streaming,"fragment_bytes":size,
            "receive":1,"attempt":1,"usage":1,"ledger":1,"capture":1,"raw_readback":true,"upstream_metadata":false,"headers_checked":14})
        );
    }
    Ok(())
}

fn assert_capture(fixture: &Fixture, raw: &str, attempts: usize) -> TestResult {
    let database = fixture.directory.path().join("batch-c.sqlite3");
    let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
    let reader = SqliteRequestMetadataReader::open(&database, test_secret_store()?)?;
    let requests: Vec<_> = rows
        .iter()
        .filter_map(|row| match row.event() {
            GatewayEvent::Request(request) => Some(request),
            _ => None,
        })
        .collect();
    assert_eq!(requests.len(), 1);
    let request = requests[0];
    let diagnostic = reader
        .get_owned(
            request.client_key_id(),
            request.request_id(),
            None,
            system_now_ms_runtime()?,
        )?
        .ok_or("C4 retained before cleanup")?;
    assert_eq!(&diagnostic.record.request, request);
    assert_eq!(diagnostic.record.evidence.raw_json(), raw);
    assert_eq!(diagnostic.attempts.len(), attempts);
    for attempt in &diagnostic.attempts {
        assert_eq!(attempt.request_id(), request.request_id());
        assert!(
            reader
                .get_owned(
                    request.client_key_id(),
                    request.request_id(),
                    Some(attempt.attempt_id()),
                    system_now_ms_runtime()?
                )?
                .is_some()
        );
    }
    assert!(
        reader
            .get_owned(
                &ClientKeyId::try_new("C4-foreign-owner")?,
                request.request_id(),
                None,
                system_now_ms_runtime()?
            )?
            .is_none()
    );
    assert!(
        reader
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                Some(&gateway_core::AttemptId::try_new("C4-not-an-attempt")?),
                system_now_ms_runtime()?
            )?
            .is_none()
    );
    assert!(
        rows.iter()
            .all(|row| !serde_json::to_string(row.event()).is_ok_and(|s| s.contains("C4_PRIVATE")))
    );
    Ok(())
}

#[actix_web::test]
async fn public_build_c4_stored_response_excludes_local_metadata() -> TestResult {
    for streaming in [false, true] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire_response(1, streaming)?)?;
        let mut fixture = lifecycle_fixture(namespaces::transport(&peer, 7)?)?;
        let database = fixture.directory.path().join("batch-c.sqlite3");
        let stored = Arc::new(
            gateway_store::stored_response::SqliteStoredResponseStore::open(
                &database,
                test_secret_store()?,
            )?,
        );
        fixture.state = fixture
            .state
            .clone()
            .with_stored_response_store(stored.clone());
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let body = json!({"model":"p12-test-model","input":"C4 stored answer",
            "stream":streaming,"store":true,"client_metadata":metadata()});
        let (status, bytes) = namespaces::request(&fixture, &url, &body).await?;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
        let terminal = namespaces::terminal(&bytes, streaming)?;
        assert_eq!(terminal["output"], tool_rounds::output(1));
        assert_eq!(terminal["usage"]["input_tokens"], 20);
        assert_capture(&fixture, &metadata().to_string(), 1)?;
        let requests = SqliteEventStore::open_read_only(&database)?.list_events()?;
        let request = requests
            .iter()
            .find_map(|row| match row.event() {
                GatewayEvent::Request(request) => Some(request),
                _ => None,
            })
            .ok_or("authenticated Request")?;
        let response_id =
            gateway_core::ResponseId::try_new(terminal["id"].as_str().ok_or("response id")?)?;
        let record = stored
            .get_owned(
                request.client_key_id(),
                &response_id,
                system_now_ms_runtime()?,
            )?
            .ok_or("owned stored Response")?;
        let replay = record.payload().request();
        assert!(
            replay
                .extensions
                .get(gateway_core::CODEX_CLIENT_METADATA)
                .is_none()
        );
        assert!(!serde_json::to_string(record.payload())?.contains("C4_PRIVATE"));
        assert!(record.payload().canonical_response()?.events().len() > 1);
        let expires: i64 = gateway_store::open(&database)?.query_row(
            "SELECT expires_at_ms FROM request_metadata_evidence",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(
            gateway_store::request_metadata::purge_expired(
                &gateway_store::open(&database)?,
                expires,
            )?,
            1
        );
        assert!(
            SqliteRequestMetadataReader::open(&database, test_secret_store()?)?
                .get_owned(request.client_key_id(), request.request_id(), None, expires)?
                .is_none()
        );
        assert!(
            !serde_json::to_string(
                stored
                    .get_owned(
                        request.client_key_id(),
                        &response_id,
                        record.created_at_ms(),
                    )?
                    .ok_or("independent StoredResponse lifetime")?
                    .payload()
            )?
            .contains("C4_PRIVATE")
        );
        assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
        assert!(
            !String::from_utf8_lossy(&peer.requests.lock().map_err(|_| "receiver")?[0].wire)
                .contains("C4_PRIVATE")
        );
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        namespaces::assert_recorded(&database, 1).await?;
        println!(
            "CPAR62_C4_STORED {}",
            json!({"streaming":streaming,
            "receive":1,"attempt":1,"usage":1,"ledger":1,"capture":1,
            "stored_response_metadata":false,"metadata_expiry_purged":true})
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_c4_request_only_rejection_retains_no_attempt() -> TestResult {
    for streaming in [false, true] {
        let peer = ModelPeer::start(streaming, response())?;
        let fixture = lifecycle_fixture(namespaces::transport(&peer, 7)?)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        // A legal C4 field does not qualify the independent ParallelTools blocker.
        let body = json!({"model":"p12-test-model","input":"pre-send rejection","stream":streaming,
            "client_metadata":metadata(),"parallel_tool_calls":true,
            "tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]});
        let (status, bytes) = namespaces::request(&fixture, &url, &body).await?;
        assert_eq!(status, 400);
        assert!(!String::from_utf8_lossy(&bytes).contains("C4_PRIVATE"));
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.events.attempts()?, 0);
        assert_capture(&fixture, &metadata().to_string(), 0)?;
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        assert_eq!(
            SqliteEventStore::open_read_only(database)?
                .list_events()?
                .iter()
                .filter(|row| matches!(row.event(), GatewayEvent::Usage(_)))
                .count(),
            0
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_c4_rejects_unknown_invalid_duplicate_and_overbound_metadata() -> TestResult {
    for streaming in [false, true] {
        let peer = ModelPeer::start(streaming, response())?;
        let fixture = lifecycle_fixture(namespaces::transport(&peer, 7)?)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let mut invalid = vec![
            "null".into(),
            "[]".into(),
            "false".into(),
            r#"{"unknown":"C4_PRIVATE_UNKNOWN"}"#.into(),
            r#"{"session_id":null}"#.into(),
            r#"{"session_id":"a","session_id":"b"}"#.into(),
            r#"{"session_id":"a","\u0073ession_id":"b"}"#.into(),
            json!({"session_id":"x".repeat(1025)}).to_string(),
            format!(
                "{{{} }}",
                " ".repeat(gateway_core::MAX_REQUEST_METADATA_BYTES)
            ),
        ];
        invalid.push(format!(
            "{{{}}}",
            " ".repeat(gateway_core::MAX_REQUEST_METADATA_BYTES - 1)
        ));
        for blob in [
            "",
            "null",
            "[]",
            r#"{"session_id":null}"#,
            r#"{"session_id":1}"#,
            r#"{"session_id":"a","session_id":"b"}"#,
            r#"{"session_id":"a","\u0073ession_id":"b"}"#,
            r#"{"request_kind":"agent"}"#,
            r#"{"request_kind":"turn","history_ingest_requested":true}"#,
            r#"{"workspaces":{}}"#,
            r#"{"tool_namespaces_info":[]}"#,
            r#"{"compaction":{}}"#,
            r#"{"sandbox":true}"#,
            r#"{"sandbox":"unknown"}"#,
            r#"{"auto_review_enabled":"true"}"#,
            r#"{"node_repl_disabled":null}"#,
            r#"{"window_number":-1}"#,
            r#"{"window_number":0.0}"#,
            r#"{"turn_started_at_unix_ms":-1}"#,
            r#"{"turn_started_at_unix_ms":9223372036854775808}"#,
            r#"{"model":{}}"#,
        ] {
            invalid.push(json!({"x-codex-turn-metadata":blob}).to_string());
        }
        invalid.push(
            json!({"session_id":"a","x-codex-turn-metadata":r#"{"session_id":"b"}"#}).to_string(),
        );
        for raw in &invalid {
            let body = format!(
                r#"{{"model":"p12-test-model","input":"C4 negative","stream":{streaming},"client_metadata":{raw}}}"#
            );
            let reply = reqwest::Client::builder()
                .no_proxy()
                .build()?
                .post(&url)
                .bearer_auth(&fixture.key)
                .header("content-type", "application/json")
                .body(body)
                .send()
                .await?;
            assert_eq!(reply.status().as_u16(), 400);
            let bytes = reply.bytes().await?;
            assert!(!String::from_utf8_lossy(&bytes).contains("C4_PRIVATE"));
        }
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
        assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
        assert_eq!(fixture.events.attempts()?, 0);
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        assert_eq!(
            gateway_store::open(database)?.query_row(
                "SELECT count(*) FROM request_metadata_evidence",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            0
        );
        println!(
            "CPAR62_C4_NEGATIVE {}",
            json!({"streaming":streaming,"cases":invalid.len(),"receive":0,"attempt":0,"capture":0})
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ReceiptFault {
    Replay,
    Conflict,
    Lost,
}
struct MetadataFault {
    inner: Arc<dyn GatewayEventSink>,
    fault: ReceiptFault,
}
impl GatewayEventSink for MetadataFault {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        self.inner.try_emit(event)
    }
    fn emit_confirmed(&self, event: GatewayEvent) -> gateway_core::EventEmissionFuture<'_> {
        self.inner.emit_confirmed(event)
    }
    fn emit_request_metadata_confirmed(
        &self,
        request: gateway_core::RequestEvent,
        evidence: gateway_core::RequestMetadataEvidence,
    ) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            let first = self
                .inner
                .emit_request_metadata_confirmed(request.clone(), evidence.clone())
                .await;
            if first != EventEmission::Persisted {
                return first;
            }
            match self.fault {
                ReceiptFault::Lost => EventEmission::PersistenceUnavailable,
                ReceiptFault::Replay => {
                    self.inner
                        .emit_request_metadata_confirmed(request, evidence)
                        .await
                }
                ReceiptFault::Conflict => {
                    let changed = gateway_core::RequestMetadataEvidence::try_new(format!(
                        " {}",
                        evidence.raw_json()
                    ));
                    match changed {
                        Ok(changed) => {
                            self.inner
                                .emit_request_metadata_confirmed(request, changed)
                                .await
                        }
                        Err(_) => EventEmission::PersistenceUnavailable,
                    }
                }
            }
        })
    }
}

#[actix_web::test]
async fn public_build_c4_recording_faults_never_send_or_report_success() -> TestResult {
    for streaming in [false, true] {
        for fault in [
            "missing-writer",
            "missing-key",
            "conflict",
            "lost-confirmation",
            "locked-database",
            "idempotent-replay",
        ] {
            let peer = ModelPeer::start_wire(streaming, tool_rounds::wire_response(1, streaming)?)?;
            let fixture = lifecycle_fixture(namespaces::transport(&peer, 7)?)?;
            let recording = if fault == "missing-writer" {
                None
            } else {
                Some(Recording::with_keys(
                    &fixture,
                    false,
                    if fault == "missing-key" {
                        None
                    } else {
                        Some(test_secret_store()?)
                    },
                )?)
            };
            if matches!(
                fault,
                "conflict" | "lost-confirmation" | "idempotent-replay"
            ) {
                let mut sink = fixture.events.1.lock().map_err(|_| "sink")?;
                let inner = sink.take().ok_or("writer")?;
                *sink = Some(Arc::new(MetadataFault {
                    inner,
                    fault: match fault {
                        "conflict" => ReceiptFault::Conflict,
                        "lost-confirmation" => ReceiptFault::Lost,
                        _ => ReceiptFault::Replay,
                    },
                }));
            }
            let lock = if fault == "locked-database" {
                // The fixture database already exists. This deterministic write outage is
                // released after the public failure; only the original pending pair may commit.
                let connection =
                    gateway_store::open(fixture.directory.path().join("batch-c.sqlite3"))?;
                connection.execute_batch("BEGIN IMMEDIATE")?;
                Some(connection)
            } else {
                None
            };
            let (url, server) = namespaces::listener(&fixture)?;
            let body = json!({"model":"p12-test-model","input":"C4 fault","stream":streaming,"client_metadata":metadata()});
            let (status, bytes) = namespaces::request(&fixture, &url, &body).await?;
            let expected = if fault == "idempotent-replay" {
                200
            } else {
                503
            };
            assert_eq!(
                status,
                expected,
                "{fault}: {}",
                String::from_utf8_lossy(&bytes)
            );
            assert!(!String::from_utf8_lossy(&bytes).contains("C4_PRIVATE"));
            assert_eq!(
                peer.accepted.load(Ordering::SeqCst),
                usize::from(expected == 200),
                "{fault}"
            );
            if expected != 200 {
                assert!(!String::from_utf8_lossy(&bytes).contains("response.completed"));
            }
            if let Some(lock) = lock {
                lock.execute_batch("ROLLBACK")?;
            }
            server.stop(false).await;
            if let Some(recording) = recording {
                recording
                    .finish(
                        &fixture,
                        u64::from(matches!(fault, "conflict" | "missing-key")),
                    )
                    .await?;
                if !matches!(fault, "missing-key") {
                    assert_capture(
                        &fixture,
                        &metadata().to_string(),
                        usize::from(expected == 200),
                    )?;
                }
            }
        }
    }
    Ok(())
}

struct HeldMetadata {
    inner: Arc<dyn GatewayEventSink>,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    submissions: AtomicUsize,
}
impl GatewayEventSink for HeldMetadata {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        self.inner.try_emit(event)
    }
    fn emit_confirmed(&self, event: GatewayEvent) -> gateway_core::EventEmissionFuture<'_> {
        self.inner.emit_confirmed(event)
    }
    fn emit_request_metadata_confirmed(
        &self,
        request: gateway_core::RequestEvent,
        evidence: gateway_core::RequestMetadataEvidence,
    ) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            self.submissions.fetch_add(1, Ordering::SeqCst);
            let result = self
                .inner
                .emit_request_metadata_confirmed(request, evidence)
                .await;
            self.entered.notify_one();
            self.release.notified().await;
            result
        })
    }
}

#[actix_web::test]
async fn public_build_c4_confirmation_precedes_execution_and_preserves_single_capture() -> TestResult
{
    let peer = ModelPeer::start_wire(false, tool_rounds::wire_response(1, false)?)?;
    let fixture = lifecycle_fixture(namespaces::transport(&peer, 7)?)?;
    let recording = Recording::start(&fixture, false)?;
    let held = {
        let mut sink = fixture.events.1.lock().map_err(|_| "sink")?;
        let held = Arc::new(HeldMetadata {
            inner: sink.take().ok_or("writer")?,
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            submissions: AtomicUsize::new(0),
        });
        *sink = Some(held.clone());
        held
    };
    let (url, server) = namespaces::listener(&fixture)?;
    let key = fixture.key.clone();
    let pending = tokio::spawn(async move {
        reqwest::Client::builder()
            .no_proxy()
            .build()?
            .post(url)
            .bearer_auth(key)
            .header("content-type", "application/json")
            .body(
                json!({"model":"p12-test-model","input":"C4 waiting","client_metadata":metadata()})
                    .to_string(),
            )
            .send()
            .await?
            .bytes()
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), held.entered.notified()).await?;
    assert!(!pending.is_finished());
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
    assert_eq!(active_leases(&fixture), 0);
    assert_capture(&fixture, &metadata().to_string(), 0)?;
    held.release.notify_one();
    let bytes = pending.await??;
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes)?["status"],
        "completed"
    );
    assert_eq!(held.submissions.load(Ordering::SeqCst), 1);
    assert_capture(&fixture, &metadata().to_string(), 1)?;
    server.stop(false).await;
    drop(held);
    recording.finish(&fixture, 0).await?;
    Ok(())
}

fn c4_output(round: usize) -> Value {
    let mut output = tool_rounds::output(round);
    if round == 0 || round == 2 {
        output[1]["arguments"] = json!(if round == 0 {
            "{}"
        } else {
            r#"{"previous":41}"#
        });
    } else {
        output[1]["content"][0]["text"] = json!(if round == 1 {
            "The seed is 41."
        } else {
            "The result is 42."
        });
    }
    output
}

#[actix_web::test]
async fn public_build_c4_metadata_keeps_two_related_tool_cycles() -> TestResult {
    for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
        let peer = ModelPeer::start_sequence(
            streaming,
            (0..5)
                .map(|r| {
                    namespaces::wire_output(&c4_output(r), &format!("C4-response-{r}"), streaming)
                })
                .collect::<Result<_, _>>()?,
        )?;
        let fixture = lifecycle_fixture(namespaces::transport(&peer, size)?)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let mut history =
            vec![json!({"role":"user","content":"Read 41, compute its successor, then answer."})];
        for round in 0..5 {
            let body = json!({"model":"p12-test-model","input":history,"stream":streaming,"client_metadata":metadata(),
                "reasoning":{"summary":"auto"},"tool_choice":"auto","parallel_tool_calls":false,
                "tools":[{"type":"function","name":"lookup","parameters":{"type":"object","properties":{"previous":{"type":"integer"}}}}]});
            let (status, bytes) = namespaces::request(&fixture, &url, &body).await?;
            assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
            let terminal = namespaces::terminal(&bytes, streaming)?;
            assert_eq!(terminal["output"], c4_output(round));
            assert_eq!(terminal["usage"]["input_tokens"], 20);
            assert_eq!(terminal["usage"]["output_tokens"], 7);
            {
                let received = peer.requests.lock().map_err(|_| "receiver")?;
                assert_eq!(received.len(), round + 1);
                assert!(!String::from_utf8_lossy(&received[round].wire).contains("C4_PRIVATE"));
                namespaces::assert_history(&received[round].body["input"], &history);
            }
            history.extend(
                terminal["output"]
                    .as_array()
                    .ok_or("output")?
                    .iter()
                    .cloned(),
            );
            if round == 0 || round == 2 {
                let call = &terminal["output"][1];
                let args: Value = serde_json::from_str(call["arguments"].as_str().ok_or("args")?)?;
                let value = if round == 0 {
                    41
                } else {
                    args["previous"].as_i64().ok_or("prior seed")? + 1
                };
                assert_eq!(value, if round == 0 { 41 } else { 42 });
                history.push(json!({"type":"function_call_output","call_id":call["call_id"],"output":value.to_string()}));
            } else if round == 1 || round == 3 {
                history.push(json!({"role":"user","content":if round==1 {"Now calculate."} else {"What was the result?"}}));
            }
        }
        let db = fixture.directory.path().join("batch-c.sqlite3");
        let rows = SqliteEventStore::open_read_only(&db)?.list_events()?;
        let reader = SqliteRequestMetadataReader::open(&db, test_secret_store()?)?;
        let mut ids = BTreeSet::new();
        for row in &rows {
            if let GatewayEvent::Request(request) = row.event() {
                ids.insert(request.request_id().clone());
                let diagnostic = reader
                    .get_owned(
                        request.client_key_id(),
                        request.request_id(),
                        None,
                        system_now_ms_runtime()?,
                    )?
                    .ok_or("C4 retained")?;
                assert_eq!(diagnostic.attempts.len(), 1);
                assert_eq!(
                    diagnostic.record.evidence.raw_json(),
                    metadata().to_string()
                );
            }
        }
        assert_eq!(ids.len(), 5);
        server.stop(false).await;
        let db = recording.finish(&fixture, 0).await?;
        namespaces::assert_recorded(&db, 5).await?;
        println!(
            "CPAR62_C4_TOOLS {}",
            json!({"streaming":streaming,"fragment_bytes":size,"receive":5,"request":5,"attempt":5,"usage":5,"ledger":5,"capture":5,"result":42})
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_c4_raw_presence_empty_strings_and_exact_size_boundary_are_preserved()
-> TestResult {
    let exact = format!(
        "{{{}}}",
        " ".repeat(gateway_core::MAX_REQUEST_METADATA_BYTES - 2)
    );
    for raw in ["{}".to_owned(),r#"{ "session_id" : "", "turn_id":"第二轮 🐦" }"#.to_owned(),
        r#"{"x-codex-turn-metadata":"{}"}"#.to_owned(),
        r#"{ "session_id":"C4_PRIVATE_SESSION", "x-codex-turn-metadata":"{ \"session_id\" : \"C4_PRIVATE_SESSION\" }" }"#.to_owned(),exact] {
        let peer=ModelPeer::start_wire(false,tool_rounds::wire_response(1,false)?)?;
        let fixture=lifecycle_fixture(namespaces::transport(&peer,7)?)?;
        let recording=Recording::start(&fixture,false)?;
        let (url,server)=namespaces::listener(&fixture)?;
        let body=format!(r#"{{"model":"p12-test-model","input":"C4 raw field","client_metadata":{raw}}}"#);
        let reply=reqwest::Client::builder().no_proxy().build()?.post(url).bearer_auth(&fixture.key)
            .header("content-type","application/json").body(body).send().await?;
        let status=reply.status().as_u16();
        let bytes=reply.bytes().await?;
        assert_eq!(status,200,"{}",String::from_utf8_lossy(&bytes));
        assert_capture(&fixture,&raw,1)?;
        server.stop(false).await;
        recording.finish(&fixture,0).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_c4_nonqualified_adapters_and_modes_reject_before_send() -> TestResult {
    for (channel, secret) in native_channels().into_iter().skip(1) {
        let peer = Arc::new(Peer::default());
        let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
            install_peer(endpoint, channel.adapter, &peer)
        })?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let reply=reqwest::Client::builder().no_proxy().build()?.post(url).bearer_auth(&fixture.key)
            .header("content-type","application/json").header("user-agent","Codex")
            .body(json!({"model":"p12-test-model","input":"C4 unqualified","client_metadata":metadata()}).to_string()).send().await?;
        assert_eq!(reply.status().as_u16(), 400);
        assert!(peer.bodies.lock().map_err(|_| "peer")?.is_empty());
        assert_eq!(fixture.events.attempts()?, 0);
        server.stop(false).await;
        let db = recording.finish(&fixture, 0).await?;
        assert_eq!(
            gateway_store::open(db)?.query_row(
                "SELECT count(*) FROM request_metadata_evidence",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            0
        );
    }
    for mode in [TransformMode::Passthrough, TransformMode::LosslessBridge] {
        let peer = ModelPeer::start(false, response())?;
        let transport = namespaces::transport(&peer, 7)?;
        let (channel, secret) = native_channels()[0];
        let fixture = fixture_with_configuration(
            channel,
            secret.as_bytes(),
            false,
            |config| {
                config.route_candidates[0].transform_mode = mode;
                Ok(())
            },
            |endpoints| {
                let mut profiles = P12TransportProfiles::try_new()?;
                profiles.build = Some(transport);
                endpoints.values_mut().next().ok_or("endpoint")?.transports = Arc::new(profiles);
                Ok(())
            },
        )?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let (status, _) = namespaces::request(
            &fixture,
            &url,
            &json!({"model":"p12-test-model","input":"C4 mode","client_metadata":metadata()}),
        )
        .await?;
        assert_eq!(status, 400);
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.events.attempts()?, 0);
        server.stop(false).await;
        recording.finish(&fixture, 0).await?;
    }
    Ok(())
}
