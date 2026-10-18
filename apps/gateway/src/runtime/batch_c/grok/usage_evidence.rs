//! Issue #59: public rejection and durable private evidence are separate outcomes.
use super::*;
use gateway_store::event_store::{AsyncSqliteEventWriter, EventWriterConfig};
use gateway_store::{
    billing_ledger::SqliteBillingLedger, event_store::SqliteEventStore,
    native_usage::SqliteNativeUsageReader,
};
use std::io::{Read, Write};

// Literal source bytes, deliberately outside Value/to_string: absent, null, zero, a number
// above u64, and a decimal whose digits must survive diagnostic readback.
const RAW_USAGE: &str = r#"{ "input_tokens":20,"output_tokens":7,"total_tokens":27,"input_tokens_details":{"cached_tokens":0,"unknown":null},"output_tokens_details":{"reasoning_tokens":3},"cost_in_usd_ticks":18446744073709551616123,"fraction":0.123456789012345678901234567890,"opaque":{"null":null,"zero":0,"array":[],"marker":"PRIVATE_USAGE_59"} }"#;

struct LoopbackBuild {
    url: String,
    received: Arc<AtomicUsize>,
    reads_after_body: Arc<AtomicUsize>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl LoopbackBuild {
    fn start(streaming: bool) -> Result<Self, Box<dyn Error>> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let url = format!("http://{}/responses", listener.local_addr()?);
        let received = Arc::new(AtomicUsize::new(0));
        let counter = received.clone();
        let thread = std::thread::spawn(move || {
            let Ok((mut socket, _)) = listener.accept() else {
                return;
            };
            let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
            let mut input = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let Ok(n) = socket.read(&mut buffer) else {
                    return;
                };
                if n == 0 {
                    return;
                }
                input.extend_from_slice(&buffer[..n]);
                if let Some(end) = input.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&input[..end]).to_ascii_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if input.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            counter.fetch_add(1, Ordering::SeqCst);
            let response = format!(
                r#"{{"id":"evidence-response","object":"response","status":"completed","output":[],"usage":{RAW_USAGE}}}"#
            );
            let (kind, bytes) = if streaming {
                (
                    "text/event-stream",
                    format!(
                        "event: response.created\ndata: {{\"type\":\"response.created\",\"response\":{{\"id\":\"evidence-response\",\"status\":\"in_progress\",\"output\":[]}}}}\n\nevent: response.completed\ndata: {{\"type\":\"response.completed\",\"response\":{response}}}\n\n"
                    ),
                )
            } else {
                ("application/json", response)
            };
            let _ = write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{bytes}",
                bytes.len()
            );
        });
        Ok(Self {
            url,
            received,
            reads_after_body: Arc::new(AtomicUsize::new(0)),
            thread: Some(thread),
        })
    }
}
impl Drop for LoopbackBuild {
    fn drop(&mut self) {
        if self.received.load(Ordering::SeqCst) == 0
            && let Ok(url) = reqwest::Url::parse(&self.url)
        {
            let _ = std::net::TcpStream::connect(("127.0.0.1", url.port().unwrap_or(80)));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
struct LoopbackBody {
    response: reqwest::Response,
    remaining: u64,
    reads_after_body: Arc<AtomicUsize>,
}
impl GrokBuildResponseBody for LoopbackBody {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            if self.remaining == 0 {
                self.reads_after_body.fetch_add(1, Ordering::SeqCst);
            }
            let chunk = self.response.chunk().await.map_err(|_| internal_error())?;
            if let Some(chunk) = &chunk {
                self.remaining = self
                    .remaining
                    .saturating_sub(u64::try_from(chunk.len()).map_err(|_| internal_error())?);
            }
            Ok(chunk.map(|b| b.to_vec()))
        })
    }
}
impl GrokBuildTransport for LoopbackBuild {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            // Test-only boundary: fixed Build assembly still occurs, but the only socket target
            // is this owned numeric loopback address. No credentials or production headers leave.
            let response = reqwest::Client::builder()
                .no_proxy()
                .build()
                .map_err(|_| internal_error())?
                .post(&self.url)
                .body(request.body().to_vec())
                .send()
                .await
                .map_err(|_| internal_error())?;
            let streaming = response
                .headers()
                .get("content-type")
                .is_some_and(|v| v == "text/event-stream");
            Ok(GrokBuildTransportResponse::new(
                200,
                if streaming {
                    GrokBuildResponseContentType::EventStream
                } else {
                    GrokBuildResponseContentType::Json
                },
                provider_grok::GrokBuildResponseContentEncoding::Identity,
                Box::new(LoopbackBody {
                    remaining: response.content_length().ok_or_else(internal_error)?,
                    response,
                    reads_after_body: self.reads_after_body.clone(),
                }),
            ))
        })
    }
}

#[actix_web::test]
async fn public_responses_rejection_retains_complete_encrypted_usage() -> TestResult {
    let mut retained = 0;
    for streaming in [false, true] {
        let peer = Arc::new(LoopbackBuild::start(streaming)?);
        let (channel, secret) = native_channels()[0];
        let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.build = Some(peer.clone());
            endpoint.transports = Arc::new(profiles);
            Ok(())
        })?;
        let database = fixture.directory.path().join("batch-c.sqlite3");
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(16, 1)?)?;
        *fixture.events.1.lock().map_err(|_| "sink")? =
            Some(Arc::new(projection_usage::ReplayUsageSink(Arc::new(
                P12FanoutEventSink::new(Arc::new(P12AttemptStageStore::new()), Arc::new(queue)),
            ))));
        let writer = tokio::spawn(
            AsyncSqliteEventWriter::new(
                &database,
                receiver,
                EventWriterConfig::try_new(4, Duration::from_millis(5))?,
            )
            .with_native_usage_secret_store(test_secret_store()?)
            .run(),
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let (_, body) = projection_usage::request("responses", streaming);
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(
            response.status(),
            if streaming {
                StatusCode::OK
            } else {
                StatusCode::BAD_GATEWAY
            }
        );
        let public_request_id = response
            .headers()
            .get("x-request-id")
            .ok_or("Request-ID")?
            .to_str()?
            .to_owned();
        let wire = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(wire.contains("response projection"), "{wire}");
        assert!(!wire.contains("PRIVATE_USAGE_59") && !wire.contains("response.completed"));
        assert_eq!(peer.received.load(Ordering::SeqCst), 1);
        if streaming {
            assert_eq!(
                peer.reads_after_body.load(Ordering::SeqCst),
                0,
                "rejection must not read ahead even to EOF"
            );
        }
        // The response has returned, but the writer is still alive: readback proves commit
        // confirmation, rather than relying on its later shutdown flush.
        let store = SqliteEventStore::open_read_only(&database)?;
        let rows = store.list_events()?;
        let request = rows
            .iter()
            .find_map(|row| {
                if let GatewayEvent::Request(v) = row.event() {
                    Some(v)
                } else {
                    None
                }
            })
            .ok_or("request")?;
        let attempt = rows
            .iter()
            .find_map(|row| {
                if let GatewayEvent::Attempt(v) = row.event() {
                    Some(v)
                } else {
                    None
                }
            })
            .ok_or("attempt")?;
        let usages: Vec<_> = rows
            .iter()
            .filter_map(|row| {
                if let GatewayEvent::Usage(v) = row.event() {
                    Some((row.event_id(), v))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(usages.len(), 1);
        let reader = SqliteNativeUsageReader::open(&database, test_secret_store()?)?;
        let record = reader
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                attempt.attempt_id(),
                system_now_ms_runtime()?,
            )?
            .ok_or("missing confirmed evidence")?;
        assert_eq!(record.usage.request_id().as_str(), public_request_id);
        assert!(matches!(
            attempt.outcome(),
            gateway_core::AttemptOutcome::Succeeded
        ));
        assert_eq!(record.evidence.raw_json(), RAW_USAGE);
        assert_eq!(
            record.evidence.source(),
            if streaming {
                gateway_core::NativeUsageSource::BuildSseTerminal
            } else {
                gateway_core::NativeUsageSource::BuildJson
            }
        );
        assert_eq!(record.completeness, "complete_usage_object");
        assert_eq!(&record.usage, usages[0].1);
        assert_eq!(record.source_event_id, usages[0].0);
        assert_eq!(record.usage.usage().input_tokens, Some(20));
        assert_eq!(record.usage.usage().cached_tokens, Some(0));
        assert_eq!(record.usage.usage().cache_creation_tokens, None);
        assert!(
            reader
                .get_owned(
                    &ClientKeyId::try_new("sibling-owner")?,
                    request.request_id(),
                    attempt.attempt_id(),
                    system_now_ms_runtime()?
                )?
                .is_none()
        );
        for row in &rows {
            assert!(!serde_json::to_string(row.event())?.contains("PRIVATE_USAGE_59"));
        }
        drop(app);
        fixture.events.1.lock().map_err(|_| "sink")?.take();
        let metrics = tokio::time::timeout(Duration::from_secs(5), writer).await??;
        assert_eq!(metrics.pending_required, 0);
        assert_eq!(metrics.required_events_quarantined, 0);
        assert_eq!(SqliteEventStore::open_read_only(&database)?.list_events()?.iter().filter(|r|matches!(r.event(),GatewayEvent::RequestFinished(v) if v.request_id.as_str()==public_request_id && v.outcome==gateway_core::RequestOutcome::Failed)).count(),1);
        let connection = gateway_store::open(&database)?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='native_usage_evidence')",
            [],
            |r| r.get(0),
        )?;
        if exists {
            retained +=
                connection.query_row("SELECT COUNT(*) FROM native_usage_evidence", [], |r| {
                    r.get::<_, i64>(0)
                })?;
        }
        assert!(
            !std::fs::read(&database)?
                .windows(b"PRIVATE_USAGE_59".len())
                .any(|w| w == b"PRIVATE_USAGE_59")
        );
        let worker = crate::billing_worker::BillingWorker::start(
            database.clone(),
            Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
        );
        worker.stop().await.map_err(|()| "billing worker")?;
        let ledger = SqliteBillingLedger::open_read_only(&database)?.list_bounded(10)?;
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].source_event_id, record.source_event_id);
        assert_eq!(ledger[0].usage, *record.usage.usage());
        assert_eq!(ledger[0].cost_microunits, None);
    }
    assert_eq!(
        retained, 2,
        "both rejected Attempts need confirmed complete private evidence"
    );
    Ok(())
}

struct LockUsageSink {
    inner: Arc<dyn GatewayEventSink>,
    database: std::path::PathBuf,
    unlock: Mutex<Option<std::sync::mpsc::Sender<()>>>,
}
impl LockUsageSink {
    fn release(&self) -> TestResult {
        if let Some(send) = self.unlock.lock().map_err(|_| "unlock")?.take() {
            let _ = send.send(());
        }
        Ok(())
    }
}
impl GatewayEventSink for LockUsageSink {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        self.inner.try_emit(event)
    }
    fn emit_confirmed(&self, event: GatewayEvent) -> gateway_core::EventEmissionFuture<'_> {
        self.inner.emit_confirmed(event)
    }
    fn emit_usage_evidence_confirmed(
        &self,
        event: gateway_core::UsageEvent,
        evidence: gateway_core::NativeUsageEvidence,
    ) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            let (release, waiting) = std::sync::mpsc::channel();
            let (held, ready) = tokio::sync::oneshot::channel();
            {
                let Ok(mut unlock) = self.unlock.lock() else {
                    return EventEmission::PersistenceUnavailable;
                };
                *unlock = Some(release);
            }
            let database = self.database.clone();
            std::thread::spawn(move || {
                if let Ok(connection) = gateway_store::open(database)
                    && connection.execute_batch("BEGIN IMMEDIATE").is_ok()
                {
                    let _ = held.send(true);
                    let _ = waiting.recv_timeout(Duration::from_secs(10));
                    return;
                }
                let _ = held.send(false);
            });
            if ready.await != Ok(true) {
                return EventEmission::PersistenceUnavailable;
            }
            self.inner
                .emit_usage_evidence_confirmed(event, evidence)
                .await
        })
    }
}

#[actix_web::test]
async fn public_responses_reports_private_recording_failure_without_losing_counters() -> TestResult
{
    for held_database in [false, true] {
        for streaming in [false, true] {
            let peer = Arc::new(LoopbackBuild::start(streaming)?);
            let (channel, secret) = native_channels()[0];
            let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
                let mut profiles = P12TransportProfiles::try_new()?;
                profiles.build = Some(peer.clone());
                endpoint.transports = Arc::new(profiles);
                Ok(())
            })?;
            let database = fixture.directory.path().join("batch-c.sqlite3");
            let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(16, 1)?)?;
            let inner: Arc<dyn GatewayEventSink> = Arc::new(P12FanoutEventSink::new(
                Arc::new(P12AttemptStageStore::new()),
                Arc::new(queue),
            ));
            let locker = Arc::new(LockUsageSink {
                inner: inner.clone(),
                database: database.clone(),
                unlock: Mutex::new(None),
            });
            *fixture.events.1.lock().map_err(|_| "sink")? = Some(if held_database {
                locker.clone()
            } else {
                inner.clone()
            });
            drop(inner);
            let mut writer =
                AsyncSqliteEventWriter::new(&database, receiver, EventWriterConfig::default());
            if held_database {
                writer = writer.with_native_usage_secret_store(test_secret_store()?);
            }
            let writer = tokio::spawn(writer.run());
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let (_, body) = projection_usage::request("responses", streaming);
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert_eq!(
                response.status(),
                if streaming {
                    StatusCode::OK
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            );
            let wire = String::from_utf8(test::read_body(response).await.to_vec())?;
            assert!(wire.contains("RecordingUnavailable"), "{wire}");
            if streaming {
                assert_eq!(
                    wire.matches("event: response.failed\n").count(),
                    1,
                    "{wire}"
                );
            }
            assert!(!wire.contains("PRIVATE_USAGE_59") && !wire.contains("response.completed"));
            assert_eq!(peer.received.load(Ordering::SeqCst), 1);
            if streaming {
                assert_eq!(
                    peer.reads_after_body.load(Ordering::SeqCst),
                    0,
                    "rejection must not read ahead even to EOF"
                );
            }
            if held_database {
                let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
                let request = rows
                    .iter()
                    .find_map(|r| {
                        if let GatewayEvent::Request(v) = r.event() {
                            Some(v)
                        } else {
                            None
                        }
                    })
                    .ok_or("request")?;
                let attempt = rows
                    .iter()
                    .find_map(|r| {
                        if let GatewayEvent::Attempt(v) = r.event() {
                            Some(v)
                        } else {
                            None
                        }
                    })
                    .ok_or("attempt")?;
                assert!(
                    SqliteNativeUsageReader::open(&database, test_secret_store()?)?
                        .get_owned(
                            request.client_key_id(),
                            request.request_id(),
                            attempt.attempt_id(),
                            system_now_ms_runtime()?
                        )?
                        .is_none(),
                    "timeout cannot acknowledge the pending private record"
                );
                locker.release()?;
            }
            drop(app);
            fixture.events.1.lock().map_err(|_| "sink")?.take();
            drop(locker);
            tokio::time::timeout(Duration::from_secs(5), writer).await??;
            let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
            assert_eq!(
                rows.iter()
                    .filter(|r| matches!(r.event(), GatewayEvent::Usage(_)))
                    .count(),
                1
            );
            let connection = gateway_store::open(&database)?;
            assert_eq!(
                connection.query_row("SELECT count(*) FROM native_usage_evidence", [], |r| r
                    .get::<_, i64>(0))?,
                i64::from(held_database)
            );
        }
    }
    Ok(())
}
