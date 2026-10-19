//! Public socket cancellation through the production leased Build adapter and real bounded I/O.
//! The sole substituted boundary hands the fixed native request to an owned numeric loopback peer.
use super::*;
use gateway_upstream::{
    AdmittedEgressTarget, UpstreamHttpMethod, UpstreamHttpRequest, UpstreamHttpResponse,
    UpstreamTimeouts,
};
use std::num::NonZeroUsize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
mod cli;
mod disconnect;
mod messages;
mod recording_faults;

struct SocketPeer {
    target: AdmittedEgressTarget,
    received: Arc<Mutex<Vec<Value>>>,
    closed: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}

#[derive(Clone, Copy)]
enum PeerEnd {
    AwaitHeaders,
    Hold,
    Truncate,
    Eof,
    Heartbeats,
}

impl SocketPeer {
    fn start(bytes: String, end: PeerEnd) -> Result<Self, Box<dyn Error>> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let listener = tokio::net::TcpListener::from_std(listener)?;
        let received = Arc::new(Mutex::new(Vec::new()));
        let requests = received.clone();
        let closed = Arc::new(AtomicUsize::new(0));
        let closure = closed.clone();
        let task: tokio::task::JoinHandle<std::io::Result<()>> = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await?;
                let request = read_request(&mut socket).await?;
                let first = {
                    let mut received = requests
                        .lock()
                        .map_err(|_| std::io::Error::other("receiver"))?;
                    received.push(request);
                    received.len() == 1
                };
                if first && matches!(end, PeerEnd::AwaitHeaders) {
                    let mut byte = [0];
                    let read = socket.read(&mut byte).await;
                    if matches!(read, Ok(0)) || read.as_ref().is_err_and(closed_socket) {
                        closure.fetch_add(1, Ordering::SeqCst);
                    }
                    continue;
                }
                // A later explicitly issued request proves that the same capacity is usable.
                let body = if first {
                    bytes.clone()
                } else {
                    complete_wire(None)
                };
                let length = body.len()
                    + if first && !matches!(end, PeerEnd::Eof) {
                        64 * 1024
                    } else {
                        0
                    };
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
                );
                socket.write_all(head.as_bytes()).await?;
                socket.write_all(body.as_bytes()).await?;
                if !first || matches!(end, PeerEnd::Truncate | PeerEnd::Eof) {
                    continue;
                }
                // The first response remains incomplete at the HTTP layer. Only source drop
                // can close it; reads instead meet the real bounded UpstreamClientPool deadline.
                let mut byte = [0];
                loop {
                    tokio::select! {
                        read = socket.read(&mut byte) => {
                            if matches!(read, Ok(0)) || read.as_ref().is_err_and(closed_socket) {
                                closure.fetch_add(1, Ordering::SeqCst);
                            }
                            break;
                        }
                        () = tokio::time::sleep(Duration::from_millis(25)), if matches!(end, PeerEnd::Heartbeats) => {
                            if let Err(error) = socket.write_all(b": heartbeat\n\n").await {
                                if closed_socket(&error) { closure.fetch_add(1, Ordering::SeqCst); }
                                break;
                            }
                        }
                    }
                }
            }
        });
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("build-61-owned-loopback")?,
            name: "Build lifecycle owned peer".into(),
            allowed_schemes: BTreeSet::from([EgressScheme::Http]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("127.0.0.1")?]),
            allowed_ports: BTreeSet::from([port]),
            allowed_cidrs: BTreeSet::from([EgressCidr::try_new("127.0.0.1".parse()?, 32)?]),
            redirect_policy: RedirectPolicy::Deny,
        })?;
        Ok(Self {
            target: policy.admit_url(
                &format!("http://127.0.0.1:{port}/v1/responses"),
                &SystemEgressDnsResolver,
            )?,
            received,
            closed,
            task,
        })
    }

    fn transport(
        &self,
        idle: Duration,
        total: Duration,
    ) -> Result<Arc<dyn GrokBuildTransport>, Box<dyn Error>> {
        Ok(Arc::new(SocketTransport {
            target: self.target.clone(),
            pool: UpstreamClientPool::new(NonZeroUsize::new(2).ok_or("pool bound")?),
            profile: UpstreamTransportProfile::new(
                UpstreamTimeouts::try_new(
                    Duration::from_secs(2).min(total),
                    Duration::from_secs(2).min(total),
                    idle,
                    total,
                )?,
                UpstreamProxy::Direct,
                NonZeroUsize::new(1).ok_or("connection bound")?,
            ),
        }))
    }
}

impl Drop for SocketPeer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn closed_socket(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::BrokenPipe
    )
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Result<Value, std::io::Error> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 4096];
    while bytes.len() < 64 * 1024 {
        let n = socket.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
            let length = headers
                .lines()
                .find_map(|v| v.strip_prefix("content-length: "))
                .and_then(|v| v.parse::<usize>().ok())
                .ok_or_else(|| std::io::Error::other("request length"))?;
            if bytes.len() >= end + 4 + length {
                return Ok(serde_json::from_slice(&bytes[end + 4..end + 4 + length])?);
            }
        }
    }
    Err(std::io::Error::other("incomplete bounded request"))
}

struct SocketTransport {
    target: AdmittedEgressTarget,
    pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
}

struct SocketBody(UpstreamHttpResponse);
impl GrokBuildResponseBody for SocketBody {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move { self.0.next_chunk().await.map(|v| v.map(|b| b.to_vec())) })
    }
}

impl GrokBuildTransport for SocketTransport {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            assert_eq!(
                request.url(),
                "https://cli-chat-proxy.grok.com/v1/responses"
            );
            let handoff = UpstreamHttpRequest::try_new(
                self.target.clone(),
                UpstreamHttpMethod::Post,
                [("content-type".to_owned(), "application/json".to_owned())],
                request.body().to_vec(),
            )
            .map_err(|_| internal_error())?;
            let response = self.pool.send(handoff, &self.profile).await?;
            Ok(GrokBuildTransportResponse::new(
                response.status(),
                GrokBuildResponseContentType::EventStream,
                provider_grok::GrokBuildResponseContentEncoding::Identity,
                Box::new(SocketBody(response)),
            ))
        })
    }
}

/// The transaction really commits; the receipt is deliberately held at the event-port boundary.
struct HeldReceipt {
    inner: Arc<dyn GatewayEventSink>,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
impl GatewayEventSink for HeldReceipt {
    fn emit_summary_evidence_confirmed(
        &self,
        attempt: gateway_core::AttemptEvent,
        evidence: gateway_core::MessagesSummaryEvidence,
    ) -> gateway_core::EventEmissionFuture<'_> {
        self.inner
            .emit_summary_evidence_confirmed(attempt, evidence)
    }
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        self.inner.try_emit(event)
    }
    fn emit_confirmed(&self, event: GatewayEvent) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            let usage = matches!(&event, GatewayEvent::Usage(_));
            let result = self.inner.emit_confirmed(event).await;
            if usage {
                self.entered.notify_one();
                self.release.notified().await;
            }
            result
        })
    }
    fn emit_usage_evidence_confirmed(
        &self,
        event: gateway_core::UsageEvent,
        evidence: gateway_core::NativeUsageEvidence,
    ) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            let result = self
                .inner
                .emit_usage_evidence_confirmed(event, evidence)
                .await;
            self.entered.notify_one();
            self.release.notified().await;
            result
        })
    }
}

async fn wait_until(mut condition: impl FnMut() -> bool) -> bool {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .is_ok()
}

async fn public_socket(
    fixture: &Fixture,
) -> Result<(tokio::net::TcpStream, actix_web::dev::ServerHandle), Box<dyn Error>> {
    public_socket_request(
        fixture,
        "/v1/responses",
        json!({"model":"p12-test-model","input":"controlled lifecycle","stream":true}),
    )
    .await
}

async fn public_socket_request(
    fixture: &Fixture,
    uri: &str,
    body: Value,
) -> Result<(tokio::net::TcpStream, actix_web::dev::ServerHandle), Box<dyn Error>> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let state = fixture.state.clone();
    let server = actix_web::HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(gateway_http_actix::configure)
    })
    .disable_signals()
    .workers(1)
    .h1_allow_half_closed(false)
    .listen(listener)?
    .run();
    let handle = server.handle();
    actix_web::rt::spawn(server);
    let mut socket = tokio::net::TcpStream::connect(address).await?;
    let body = body.to_string();
    let request = format!(
        "POST {uri} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        fixture.key,
        body.len()
    );
    socket.write_all(request.as_bytes()).await?;
    Ok((socket, handle))
}

fn native_event(kind: &str, mut payload: Value) -> String {
    payload["type"] = json!(kind);
    format!("event: {kind}\ndata: {payload}\n\n")
}

fn partial_wire() -> String {
    frame("response.created", &created())
        + &native_event(
            "response.output_item.added",
            json!({"output_index":0,"item":{
                "id":"message-61","type":"message","status":"in_progress","role":"assistant","content":[]
            }}),
        )
        + &native_event(
            "response.output_text.delta",
            json!({
                "output_index":0,"item_id":"message-61","content_index":0,"delta":"partial-61"
            }),
        )
}

fn complete_wire(raw: Option<&str>) -> String {
    let item = json!({"id":"message-61","type":"message","status":"completed","role":"assistant",
        "content":[{"type":"output_text","text":"partial-61","annotations":[]}]});
    let raw = raw.unwrap_or(r#"{"input_tokens":20,"output_tokens":7,"total_tokens":27}"#);
    let terminal = format!(
        r#"{{"id":"response-61","model":"grok-4.5","status":"completed","output":[{item}],"usage":{raw}}}"#
    );
    partial_wire()
        + &native_event(
            "response.output_item.done",
            json!({"output_index":0,"item":item}),
        )
        + &format!(
            "event: response.completed\ndata: {{\"type\":\"response.completed\",\"response\":{terminal}}}\n\n"
        )
}

fn lifecycle_fixture(transport: Arc<dyn GrokBuildTransport>) -> Result<Fixture, Box<dyn Error>> {
    let (channel, secret) = native_channels()[0];
    fixture_with_configuration(
        channel,
        secret.as_bytes(),
        false,
        |configuration| {
            configuration.public_models[0].model_name = channel.model.into();
            configuration.model_aliases.push(
                gateway_store::control_plane::ModelAliasConfiguration {
                    alias: "p12-test-model".into(),
                    public_model_id: configuration.public_models[0].id.clone(),
                },
            );
            configuration.model_routes[0].max_attempts = 3;
            let mut spare = configuration.route_candidates[0].clone();
            spare.id = gateway_core::RouteCandidateId::try_new("build-61-spare-candidate")?;
            spare.upstream_model = "unselected-spare-model".into();
            spare.priority = 1;
            configuration.route_candidates.push(spare);
            Ok(())
        },
        |endpoints| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.build = Some(transport);
            endpoints.values_mut().next().ok_or("endpoint")?.transports = Arc::new(profiles);
            Ok(())
        },
    )
}

struct Recording {
    database: std::path::PathBuf,
    receipt: Option<Arc<HeldReceipt>>,
    writer: tokio::task::JoinHandle<gateway_store::event_store::EventWriterMetrics>,
}

impl Recording {
    fn start(fixture: &Fixture, hold_receipt: bool) -> Result<Self, Box<dyn Error>> {
        Self::with_keys(fixture, hold_receipt, Some(test_secret_store()?))
    }

    fn with_keys(
        fixture: &Fixture,
        hold_receipt: bool,
        secrets: Option<SecretStore>,
    ) -> Result<Self, Box<dyn Error>> {
        let database = fixture.directory.path().join("batch-c.sqlite3");
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(64, 1)?)?;
        let queue: Arc<dyn GatewayEventSink> = Arc::new(queue);
        let receipt = hold_receipt.then(|| {
            Arc::new(HeldReceipt {
                inner: queue.clone(),
                entered: tokio::sync::Notify::new(),
                release: tokio::sync::Notify::new(),
            })
        });
        *fixture.events.1.lock().map_err(|_| "sink")? =
            Some(receipt.as_ref().map_or(queue, |v| v.clone()));
        let writer = AsyncSqliteEventWriter::new(
            &database,
            receiver,
            EventWriterConfig::try_new(4, Duration::from_millis(5))?,
        );
        let writer = if let Some(secrets) = secrets {
            writer.with_native_usage_secret_store(secrets)
        } else {
            writer
        };
        let writer = tokio::spawn(writer.run());
        Ok(Self {
            database,
            receipt,
            writer,
        })
    }

    async fn finish(
        self,
        fixture: &Fixture,
        quarantined: u64,
    ) -> Result<std::path::PathBuf, Box<dyn Error>> {
        fixture.events.1.lock().map_err(|_| "sink")?.take();
        drop(self.receipt);
        let metrics = tokio::time::timeout(Duration::from_secs(5), self.writer).await??;
        assert_eq!(metrics.required_events_quarantined, quarantined);
        assert_eq!(metrics.pending_required, 0);
        Ok(self.database)
    }
}

fn active_leases(fixture: &Fixture) -> usize {
    fixture
        .executor
        .scheduler
        .diagnostic_bindings()
        .map(|(_, v)| v.active_leases())
        .sum()
}

async fn read_public_until(
    socket: &mut tokio::net::TcpStream,
    marker: &str,
) -> Result<String, Box<dyn Error>> {
    Ok(tokio::time::timeout(Duration::from_secs(5), async {
        let mut wire = Vec::new();
        let mut bytes = [0; 4096];
        while wire.len() < 64 * 1024 {
            let n = socket.read(&mut bytes).await?;
            if n == 0 {
                return Err(std::io::Error::other("public socket ended before marker"));
            }
            wire.extend_from_slice(&bytes[..n]);
            if String::from_utf8_lossy(&wire).contains(marker) {
                return Ok(String::from_utf8_lossy(&wire).into_owned());
            }
        }
        Err(std::io::Error::other("public socket exceeded bound"))
    })
    .await??)
}

#[actix_web::test]
async fn public_build_disconnect_releases_upstream_before_pending_usage_receipt() -> TestResult {
    for (extra, reset) in [(false, false), (false, true), (true, false), (true, true)] {
        let raw = if extra {
            r#"{"input_tokens":20,"output_tokens":7,"total_tokens":27,"cost_in_usd_ticks":1}"#
        } else {
            r#"{"input_tokens":20,"output_tokens":7,"total_tokens":27}"#
        };
        let peer = SocketPeer::start(complete_wire(Some(raw)), PeerEnd::Hold)?;
        let fixture =
            lifecycle_fixture(peer.transport(Duration::from_secs(5), Duration::from_secs(10))?)?;
        let recording = Recording::start(&fixture, true)?;
        let receipt = recording.receipt.as_ref().ok_or("held receipt")?;
        let (mut socket, server) = public_socket(&fixture).await?;
        tokio::time::timeout(Duration::from_secs(5), receipt.entered.notified()).await?;
        let wire = read_public_until(&mut socket, "partial-61").await?;
        assert!(wire.contains("200 OK") && !wire.contains("response.completed"));
        if !extra {
            assert_eq!(
                active_leases(&fixture),
                1,
                "real serving lease is held before cancellation"
            );
        }
        // The deployed inference listener cancels both orderly FIN and abortive RST.
        // Management and clients that deliberately keep both halves open are separate.
        if reset {
            socket.set_zero_linger()?;
        } else {
            socket.shutdown().await?;
        }
        drop(socket);
        let released = wait_until(|| peer.closed.load(Ordering::SeqCst) == 1).await;
        let leases = active_leases(&fixture);
        receipt.release.notify_one();
        // The body guard records cancellation asynchronously. Observe that durable outcome
        // before abrupt test-server cleanup can discard its pending task under parallel load.
        assert!(
            wait_until(|| SqliteEventStore::open_read_only(&recording.database)
                .and_then(|store| store.list_events())
                .is_ok_and(|rows| rows.iter().any(
                    |row| matches!(row.event(),GatewayEvent::RequestFinished(event)
                    if event.outcome==gateway_core::RequestOutcome::Cancelled)
                )))
            .await
        );
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        assert!(
            released,
            "disconnect must release the upstream while the local Usage receipt is pending"
        );
        assert_eq!(
            leases, 0,
            "pending evidence cannot keep the credential occupied"
        );
        assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
        assert_cancelled_usage(&database, extra.then_some(raw)).await?;
    }
    Ok(())
}

async fn assert_cancelled_usage(database: &std::path::Path, raw: Option<&str>) -> TestResult {
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
            if let GatewayEvent::Attempt(r) = v.event() {
                Some(r)
            } else {
                None
            }
        })
        .collect();
    let usages: Vec<_> = rows
        .iter()
        .filter_map(|v| {
            if let GatewayEvent::Usage(r) = v.event() {
                Some(r)
            } else {
                None
            }
        })
        .collect();
    assert_eq!((requests.len(), attempts.len(), usages.len()), (1, 1, 1));
    assert_eq!(usages[0].attempt_id(), Some(attempts[0].attempt_id()));
    assert_eq!(usages[0].request_id(), requests[0].request_id());
    assert_eq!(usages[0].usage().input_tokens, Some(20));
    assert_eq!(usages[0].usage().output_tokens, Some(7));
    let finished: Vec<_> = rows
        .iter()
        .filter_map(|v| {
            if let GatewayEvent::RequestFinished(r) = v.event() {
                Some(r)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].outcome, gateway_core::RequestOutcome::Cancelled);
    let evidence = SqliteNativeUsageReader::open(database, test_secret_store()?)?.get_owned(
        requests[0].client_key_id(),
        requests[0].request_id(),
        attempts[0].attempt_id(),
        system_now_ms_runtime()?,
    )?;
    if let Some(raw) = raw {
        let record =
            evidence.ok_or("complete rejected Usage retained despite cancelled receipt")?;
        assert_eq!(record.evidence.raw_json(), raw);
        assert_eq!(record.completeness, "complete_usage_object");
    } else {
        assert!(evidence.is_none());
    }
    let worker = crate::billing_worker::BillingWorker::start(
        database.to_owned(),
        Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
    );
    worker.stop().await.map_err(|()| "billing worker")?;
    let ledger = SqliteBillingLedger::open_read_only(database)?.list_bounded(10)?;
    assert_eq!(ledger.len(), 1);
    assert_eq!(ledger[0].usage, *usages[0].usage());
    assert_eq!(ledger[0].cost_microunits, None);
    Ok(())
}

#[actix_web::test]
async fn public_build_cancellation_and_disconnect_stop_reads_and_restore_capacity() -> TestResult {
    use actix_web::body::MessageBody;
    for socket_abort in [false, true] {
        let peer = SocketPeer::start(partial_wire(), PeerEnd::Hold)?;
        let fixture =
            lifecycle_fixture(peer.transport(Duration::from_secs(5), Duration::from_secs(10))?)?;
        let recording = Recording::start(&fixture, false)?;
        if socket_abort {
            let (mut socket, server) = public_socket(&fixture).await?;
            let wire = read_public_until(&mut socket, "partial-61").await?;
            assert!(wire.contains("200 OK") && !wire.contains("response.completed"));
            assert_eq!(active_leases(&fixture), 1);
            socket.set_zero_linger()?;
            drop(socket);
            assert!(wait_until(|| peer.closed.load(Ordering::SeqCst) == 1).await);
            server.stop(false).await;
        } else {
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(
                        json!({"model":"p12-test-model","input":"body cancellation","stream":true}),
                    )
                    .to_request(),
            )
            .await;
            assert_eq!(reply.status(), StatusCode::OK);
            let mut body = Box::pin(reply.into_body());
            let mut wire = String::new();
            while let Some(chunk) = std::future::poll_fn(|cx| body.as_mut().poll_next(cx)).await {
                wire.push_str(std::str::from_utf8(&chunk?)?);
                if wire.contains("partial-61") {
                    break;
                }
            }
            assert!(wire.contains("partial-61") && !wire.contains("response.completed"));
            assert_eq!(active_leases(&fixture), 1);
            drop(body);
            assert!(wait_until(|| peer.closed.load(Ordering::SeqCst) == 1).await);
            drop(app);
        }
        assert_eq!(active_leases(&fixture), 0);
        assert_eq!(
            peer.received.lock().map_err(|_| "receiver")?.len(),
            1,
            "cancellation cannot resend"
        );
        let cancelled_rows =
            SqliteEventStore::open_read_only(&recording.database)?.list_events()?;
        assert!(
            cancelled_rows
                .iter()
                .all(|r| !matches!(r.event(), GatewayEvent::Usage(_))),
            "no terminal Usage has been received or validated"
        );
        // A new explicit public request, on the same single-capacity native account, succeeds.
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let reply=test::call_service(&app,test::TestRequest::post().uri("/v1/responses")
            .insert_header(("authorization",format!("Bearer {}",fixture.key)))
            .set_json(json!({"model":"p12-test-model","input":"explicit later request","stream":true})).to_request()).await;
        assert_eq!(reply.status(), StatusCode::OK);
        let bytes = test::read_body(reply).await;
        let frames = public_frames(&bytes)?;
        let completed: Vec<_> = frames
            .iter()
            .filter(|v| v["type"] == "response.completed")
            .collect();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0]["response"]["model"], "grok-4.5");
        assert_eq!(completed[0]["response"]["usage"]["input_tokens"], 20);
        assert!(frames.iter().all(|v| v["type"] != "response.failed"));
        assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 2);
        assert!(
            peer.received
                .lock()
                .map_err(|_| "receiver")?
                .iter()
                .all(|v| v["model"] == "grok-4.5")
        );
        drop(app);
        let database = recording.finish(&fixture, 0).await?;
        let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r.event(), GatewayEvent::Request(_)))
                .count(),
            2
        );
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r.event(), GatewayEvent::Attempt(_)))
                .count(),
            2
        );
        let finished: Vec<_> = rows
            .iter()
            .filter_map(|r| {
                if let GatewayEvent::RequestFinished(v) = r.event() {
                    Some(v)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(finished.len(), 2);
        assert_eq!(
            finished
                .iter()
                .filter(|v| v.outcome == gateway_core::RequestOutcome::Cancelled)
                .count(),
            1
        );
        assert_eq!(
            finished
                .iter()
                .filter(|v| v.outcome == gateway_core::RequestOutcome::Succeeded)
                .count(),
            1
        );
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
        assert_eq!(usages.len(), 1);
        assert_eq!(
            usages[0].request_id(),
            &finished
                .iter()
                .find(|v| v.outcome == gateway_core::RequestOutcome::Succeeded)
                .ok_or("successful explicit request")?
                .request_id
        );
        let worker = crate::billing_worker::BillingWorker::start(
            database.clone(),
            Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
        );
        worker.stop().await.map_err(|()| "billing worker")?;
        let ledger = SqliteBillingLedger::open_read_only(&database)?.list_bounded(10)?;
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].usage, *usages[0].usage());
        assert_eq!(ledger[0].cost_microunits, None);
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_upstream_eof_idle_and_total_timeout_fail_once_without_resend() -> TestResult {
    for (end, idle, total, expected) in [
        (
            PeerEnd::Truncate,
            Duration::from_secs(2),
            Duration::from_secs(10),
            GatewayErrorCode::EgressUnavailable,
        ),
        (
            PeerEnd::Eof,
            Duration::from_secs(2),
            Duration::from_secs(10),
            GatewayErrorCode::StreamTruncated,
        ),
        (
            PeerEnd::Hold,
            Duration::from_millis(200),
            Duration::from_secs(10),
            GatewayErrorCode::EgressUnavailable,
        ),
        (
            PeerEnd::Heartbeats,
            Duration::from_millis(200),
            Duration::from_millis(500),
            GatewayErrorCode::EgressUnavailable,
        ),
    ] {
        let peer = SocketPeer::start(partial_wire(), end)?;
        let fixture = lifecycle_fixture(peer.transport(idle, total)?)?;
        let recording = Recording::start(&fixture, false)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let reply=test::call_service(&app,test::TestRequest::post().uri("/v1/responses")
            .insert_header(("authorization",format!("Bearer {}",fixture.key)))
            .set_json(json!({"model":"p12-test-model","input":"bounded upstream failure","stream":true})).to_request()).await;
        assert_eq!(reply.status(), StatusCode::OK, "headers already committed");
        let bytes = tokio::time::timeout(Duration::from_secs(5), test::read_body(reply)).await?;
        let wire = std::str::from_utf8(&bytes)?;
        let frames = public_frames(&bytes)?;
        assert!(wire.contains("partial-61"));
        assert_eq!(
            frames
                .iter()
                .filter(|v| v["type"] == "response.failed")
                .count(),
            1,
            "{wire}"
        );
        assert!(frames.iter().all(|v| v["type"] != "response.completed"));
        assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
        assert_eq!(active_leases(&fixture), 0);
        if matches!(end, PeerEnd::Hold | PeerEnd::Heartbeats) {
            assert!(wait_until(|| peer.closed.load(Ordering::SeqCst) == 1).await);
        }
        drop(app);
        let database = recording.finish(&fixture, 0).await?;
        let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r.event(), GatewayEvent::Request(_)))
                .count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r.event(), GatewayEvent::Attempt(_)))
                .count(),
            1
        );
        assert!(
            rows.iter()
                .all(|r| !matches!(r.event(), GatewayEvent::Usage(_))),
            "unreceived Usage stays unknown"
        );
        let finished: Vec<_> = rows
            .iter()
            .filter_map(|r| {
                if let GatewayEvent::RequestFinished(v) = r.event() {
                    Some(v)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].outcome, gateway_core::RequestOutcome::Failed);
        assert_eq!(finished[0].error_code, Some(expected));
        assert!(finished[0].first_content_ms.is_some());
        let worker = crate::billing_worker::BillingWorker::start(
            database.clone(),
            Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
        );
        worker.stop().await.map_err(|()| "billing worker")?;
        assert!(
            SqliteBillingLedger::open_read_only(&database)?
                .list_bounded(10)?
                .is_empty()
        );
    }
    Ok(())
}
