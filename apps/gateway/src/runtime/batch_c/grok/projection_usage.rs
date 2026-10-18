//! Public Build projection failures keep decoded facts on their original Attempt.
use super::*;
use gateway_core::{InputTokenAccounting, RequestOutcome, UsageProvenance, UsageSummary};
use gateway_store::{
    billing_ledger::SqliteBillingLedger,
    event_store::{AsyncSqliteEventWriter, EventWriterConfig, SqliteEventStore, UsageEventQuery},
};

struct ProjectionPeer {
    output: Value,
    usage: Option<Value>,
    received: AtomicUsize,
}
impl GrokBuildTransport for ProjectionPeer {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            self.received.fetch_add(1, Ordering::Relaxed);
            let body: Value =
                serde_json::from_slice(request.body()).map_err(|_| internal_error())?;
            let streaming = body["stream"] == true;
            let mut response = json!({"id":"projection-response","model":body["model"],"object":"response","status":"completed","output":self.output});
            if let Some(usage) = &self.usage {
                response["usage"] = usage.clone();
            }
            let bytes = if streaming {
                let start = json!({"type":"response.created","response":{"id":"projection-response","model":body["model"],"status":"in_progress","output":[]}});
                let mut wire = format!("event: response.created\ndata: {start}\n\n");
                for (index, item) in self
                    .output
                    .as_array()
                    .ok_or_else(internal_error)?
                    .iter()
                    .enumerate()
                {
                    for kind in ["response.output_item.added", "response.output_item.done"] {
                        let event = json!({"type":kind,"output_index":index,"item":item});
                        write!(wire, "event: {kind}\ndata: {event}\n\n")
                            .map_err(|_| internal_error())?;
                    }
                }
                let end = json!({"type":"response.completed","response":response});
                write!(wire, "event: response.completed\ndata: {end}\n\n")
                    .map_err(|_| internal_error())?;
                wire.into_bytes()
            } else {
                serde_json::to_vec(&response).map_err(|_| internal_error())?
            };
            Ok(GrokBuildTransportResponse::new(
                200,
                if streaming {
                    GrokBuildResponseContentType::EventStream
                } else {
                    GrokBuildResponseContentType::Json
                },
                provider_grok::GrokBuildResponseContentEncoding::Identity,
                Box::new(Body(Some(bytes))),
            ))
        })
    }
}

fn rich_output() -> Value {
    json!([
        {"id":"reason-projection","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"PRIVATE_PROJECTED_TEXT"}],"content":[{"type":"reasoning_text","text":"PRIVATE_REASONING_TEXT"}]},
        {"id":"answer-projection","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"answer","annotations":[]}]}
    ])
}

pub(super) fn request(protocol: &str, streaming: bool) -> (&'static str, Value) {
    match protocol {
        "responses" => (
            "/v1/responses",
            json!({"model":"p12-test-model","input":"synthetic","stream":streaming,"max_output_tokens":4096}),
        ),
        "messages" => (
            "/v1/messages",
            json!({"model":"p12-test-model","messages":[{"role":"user","content":"synthetic"}],"stream":streaming,"max_tokens":4096}),
        ),
        _ => (
            "/v1/chat/completions",
            json!({"model":"p12-test-model","messages":[{"role":"user","content":"synthetic"}],"stream":streaming,"max_completion_tokens":4096}),
        ),
    }
}

fn build_fixture(peer: &Arc<ProjectionPeer>) -> Result<Fixture, Box<dyn Error>> {
    let (channel, secret) = native_channels()[0];
    fixture(channel, secret.as_bytes(), |endpoint| {
        let mut profiles = P12TransportProfiles::try_new()?;
        profiles.build = Some(peer.clone());
        endpoint.transports = Arc::new(profiles);
        Ok(())
    })
}

#[actix_web::test]
async fn grok_build_projection_rejection_has_safe_reason_and_request_id() -> TestResult {
    for protocol in ["chat", "messages"] {
        for streaming in [false, true] {
            let peer = Arc::new(ProjectionPeer {
                output: rich_output(),
                usage: None,
                received: AtomicUsize::new(0),
            });
            let fixture = build_fixture(&peer)?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let (uri, body) = request(protocol, streaming);
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(uri)
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .insert_header(("x-request-id", "client-spoofed-id"))
                    .set_json(body)
                    .to_request(),
            )
            .await;
            let id = response
                .headers()
                .get("x-request-id")
                .ok_or("missing admitted Request-ID")?
                .to_str()?
                .to_owned();
            assert_ne!(id, "client-spoofed-id");
            let status = response.status();
            let wire = String::from_utf8(test::read_body(response).await.to_vec())?;
            assert_eq!(
                status,
                if streaming {
                    StatusCode::OK
                } else {
                    StatusCode::BAD_GATEWAY
                }
            );
            assert!(wire.contains("response projection"), "{wire}");
            // A stream may have emitted representable reasoning before later native metadata
            // proves unrepresentable. Content is forbidden in the error, not in semantic data.
            let error_frame = wire
                .lines()
                .find(|line| line.contains("\"error\""))
                .ok_or("missing public error")?;
            assert!(
                !error_frame.contains("PRIVATE_") && !error_frame.contains("reason-projection")
            );
            assert!(!wire.contains("PRIVATE_PROJECTED_TEXT") && !wire.contains("synthetic-build"));
            assert!(!wire.contains("[DONE]") && !wire.contains("message_stop"));
            assert_eq!(peer.received.load(Ordering::Relaxed), 1);
            let events = fixture.events.0.lock().map_err(|_| "events")?;
            assert_eq!(events.iter().filter(|event| matches!(event, GatewayEvent::Request(value) if value.request_id().as_str() == id)).count(), 1);
            assert_eq!(events.iter().filter(|event| matches!(event, GatewayEvent::Attempt(value) if value.request_id().as_str() == id && matches!(value.outcome(), AttemptOutcome::Succeeded))).count(), 1);
            assert_eq!(events.iter().filter(|event| matches!(event, GatewayEvent::RequestFinished(value) if value.request_id.as_str() == id && value.outcome == RequestOutcome::Failed)).count(), 1);
        }
    }
    Ok(())
}

pub(super) struct ReplayUsageSink(pub(super) Arc<dyn GatewayEventSink>);
impl GatewayEventSink for ReplayUsageSink {
    fn emit_usage_evidence_confirmed(
        &self,
        event: gateway_core::UsageEvent,
        evidence: gateway_core::NativeUsageEvidence,
    ) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            let result = self
                .0
                .emit_usage_evidence_confirmed(event.clone(), evidence.clone())
                .await;
            if result != EventEmission::Persisted {
                return result;
            }
            self.0.emit_usage_evidence_confirmed(event, evidence).await
        })
    }
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        self.0.try_emit(event)
    }
    fn emit_confirmed(&self, event: GatewayEvent) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            if matches!(event, GatewayEvent::Usage(_)) {
                let result = self.0.emit_confirmed(event.clone()).await;
                if result.into_result().is_err() {
                    return result;
                }
            }
            self.0.emit_confirmed(event).await
        })
    }
}

#[actix_web::test]
async fn grok_build_projection_failure_preserves_usage_and_ledger() -> TestResult {
    for protocol in ["chat", "messages", "responses"] {
        for streaming in [false, true] {
            for partial in [Some(false), Some(true), None] {
                let usage = partial.map(|partial| if partial {json!({"input_tokens":20,"input_tokens_details":{"cached_tokens":5}})} else {json!({"input_tokens":20,"output_tokens":7,"input_tokens_details":{"cached_tokens":5},"output_tokens_details":{"reasoning_tokens":3}})});
                let expected = partial.map(|partial| Usage {
                    input_tokens: Some(20),
                    output_tokens: if partial { None } else { Some(7) },
                    cached_tokens: Some(5),
                    reasoning_tokens: if partial { None } else { Some(3) },
                    provenance: UsageProvenance::Measured,
                    input_accounting: InputTokenAccounting::Inclusive,
                    ..Usage::default()
                });
                let peer = Arc::new(ProjectionPeer {
                    output: rich_output(),
                    usage,
                    received: AtomicUsize::new(0),
                });
                let fixture = build_fixture(&peer)?;
                let database = fixture.directory.path().join("batch-c.sqlite3");
                let (queue, receiver) =
                    BoundedEventQueue::try_new(EventQueueConfig::try_new(16, 1)?)?;
                let sink: Arc<dyn GatewayEventSink> = Arc::new(P12FanoutEventSink::new(
                    Arc::new(P12AttemptStageStore::new()),
                    Arc::new(queue),
                ));
                *fixture.events.1.lock().map_err(|_| "sink")? =
                    Some(Arc::new(ReplayUsageSink(sink)));
                let writer = tokio::spawn(
                    AsyncSqliteEventWriter::new(
                        &database,
                        receiver,
                        EventWriterConfig::try_new(4, Duration::from_millis(5))?,
                    )
                    .run(),
                );
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(fixture.state.clone()))
                        .configure(gateway_http_actix::configure),
                )
                .await;
                let (uri, body) = request(protocol, streaming);
                let response = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(uri)
                        .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                        .set_json(body)
                        .to_request(),
                )
                .await;
                let status = response.status();
                let wire_id = response
                    .headers()
                    .get("x-request-id")
                    .and_then(|id| id.to_str().ok())
                    .map(str::to_owned);
                let wire = String::from_utf8(test::read_body(response).await.to_vec())?;
                let success = protocol == "responses";
                assert_eq!(
                    status,
                    if success || streaming {
                        StatusCode::OK
                    } else {
                        StatusCode::BAD_GATEWAY
                    }
                );
                if !success {
                    assert!(!wire.contains("[DONE]") && !wire.contains("message_stop"));
                }
                assert_eq!(peer.received.load(Ordering::Relaxed), 1);
                drop(app);
                fixture.events.1.lock().map_err(|_| "sink")?.take();
                let metrics = tokio::time::timeout(Duration::from_secs(5), writer).await??;
                assert_eq!(metrics.pending_required, 0);
                assert_eq!(metrics.required_events_quarantined, 0);
                let store = SqliteEventStore::open_read_only(&database)?;
                let rows = store.list_events()?;
                let attempts: Vec<_> = rows
                    .iter()
                    .filter_map(|row| {
                        if let GatewayEvent::Attempt(value) = row.event() {
                            Some(value)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(attempts.len(), 1);
                let attempt = attempts[0];
                assert!(matches!(attempt.outcome(), AttemptOutcome::Succeeded));
                let usage: Vec<_> = rows
                    .iter()
                    .filter_map(|row| {
                        if let GatewayEvent::Usage(value) = row.event() {
                            Some(value)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(
                    usage.len(),
                    usize::from(expected.is_some()),
                    "{protocol} stream={streaming} partial={partial:?}"
                );
                assert_eq!(wire_id.as_deref(), Some(attempt.request_id().as_str()));
                assert_eq!(rows.iter().filter(|row| matches!(row.event(), GatewayEvent::RequestFinished(value) if value.request_id == *attempt.request_id() && value.outcome == if success {RequestOutcome::Succeeded} else {RequestOutcome::Failed})).count(), 1);
                if let Some(expected) = &expected {
                    assert_eq!(usage[0].request_id(), attempt.request_id());
                    assert_eq!(usage[0].attempt_id(), Some(attempt.attempt_id()));
                    assert_eq!(usage[0].usage(), &UsageSummary::from(expected));
                }
                let read = store.visit_usage_lineages(
                    &UsageEventQuery::default(),
                    |_, owner, value| {
                        assert_eq!(owner.attempt_id(), attempt.attempt_id());
                        assert_eq!(Some(value), usage.first().copied());
                        Ok(true)
                    },
                )?;
                assert_eq!(read.excluded_usage_events, 0);
                let worker = crate::billing_worker::BillingWorker::start(
                    database.clone(),
                    Arc::new(
                        gateway_control::billing_processing::BillingProcessingMonitor::default(),
                    ),
                );
                worker.stop().await.map_err(|()| "billing worker")?;
                let ledger = SqliteBillingLedger::open_read_only(&database)?.list_bounded(10)?;
                assert_eq!(ledger.len(), usage.len());
                if let Some(expected) = expected {
                    assert_eq!(ledger[0].usage, UsageSummary::from(&expected));
                    assert_eq!(ledger[0].request_id, attempt.request_id().as_str());
                    assert_eq!(ledger[0].account_id, attempt.credential_id().as_str());
                    assert_eq!(ledger[0].response_id, usage[0].response_id().as_str());
                    let source_id = rows
                        .iter()
                        .find_map(|row| {
                            if matches!(row.event(), GatewayEvent::Usage(_)) {
                                Some(row.event_id())
                            } else {
                                None
                            }
                        })
                        .ok_or("usage source")?;
                    assert_eq!(ledger[0].source_event_id, source_id);
                    assert_eq!(ledger[0].cost_microunits, None);
                }
            }
        }
    }
    Ok(())
}

#[actix_web::test]
async fn grok_build_diagnostic_is_private_selected_and_one_shot() -> TestResult {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let directory = TemporaryDirectory::new()?;
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))?;
    let (uri, body) = request("chat", false);
    let payload = serde_json::to_vec(&body)?;
    let digest = format!("{:x}", Sha256::digest(&payload));
    let canonical = directory.path().canonicalize()?;
    let diagnostic = Arc::new(build_diagnostic::BuildDiagnostic::open(
        &canonical,
        &digest,
        env!("GATEWAY_RELEASE_REVISION"),
    )?);
    let peer = Arc::new(ProjectionPeer {
        output: rich_output(),
        usage: Some(json!({"input_tokens":20,"output_tokens":7})),
        received: AtomicUsize::new(0),
    });
    let inner: Arc<dyn GrokBuildTransport> = peer.clone();
    let id = RequestId::try_new("selected-diagnostic-test")?;
    for (protocol, mode, bytes) in [
        (
            ProtocolFormat::OpenAiResponses,
            ResponsesResponseMode::NonStreaming,
            Some(payload.as_slice()),
        ),
        (
            ProtocolFormat::OpenAiChatCompletions,
            ResponsesResponseMode::Streaming,
            Some(payload.as_slice()),
        ),
        (
            ProtocolFormat::OpenAiChatCompletions,
            ResponsesResponseMode::NonStreaming,
            None,
        ),
        (
            ProtocolFormat::OpenAiChatCompletions,
            ResponsesResponseMode::NonStreaming,
            Some(b"unselected".as_slice()),
        ),
    ] {
        assert!(Arc::ptr_eq(
            &inner,
            &diagnostic.wrap_selected(inner.clone(), &id, protocol, mode, bytes)?
        ));
    }
    assert!(!directory.path().join("native-response.bin").exists());
    let (channel, secret) = native_channels()[0];
    let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
        let mut profiles = P12TransportProfiles::try_new()?;
        assert!(profiles.build_diagnostic.is_none());
        profiles.build = Some(peer.clone());
        profiles.build_diagnostic = Some(diagnostic.clone());
        endpoint.transports = Arc::new(profiles);
        Ok(())
    })?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let mut selected_id = String::new();
    for (index, status) in [StatusCode::BAD_GATEWAY, StatusCode::INTERNAL_SERVER_ERROR]
        .into_iter()
        .enumerate()
    {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(uri)
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .insert_header(("content-type", "application/json"))
                .set_payload(payload.clone())
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), status);
        if index == 0 {
            selected_id = response
                .headers()
                .get("x-request-id")
                .ok_or("diagnostic Request-ID")?
                .to_str()?
                .to_owned();
        }
        let wire = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(
            !wire.contains("PRIVATE_")
                && !wire.contains("native-response")
                && !wire.contains("synthetic-build")
        );
    }
    assert_eq!(peer.received.load(Ordering::Relaxed), 1);
    let selection: Value =
        serde_json::from_slice(&std::fs::read(directory.path().join("selection.json"))?)?;
    assert_eq!(selection["request_id"], selected_id);
    let raw = std::fs::read(directory.path().join("native-response.bin"))?;
    let native: Value = serde_json::from_slice(&raw)?;
    assert_eq!(native["output"], rich_output());
    let receipt: Value =
        serde_json::from_slice(&std::fs::read(directory.path().join("body-receipt.json"))?)?;
    assert_eq!(receipt["complete"], true);
    assert_eq!(receipt["bytes"], raw.len());
    assert_eq!(receipt["sha256"], format!("{:x}", Sha256::digest(&raw)));
    for name in [
        "native-response.bin",
        "selection.json",
        "transport.json",
        "body-receipt.json",
    ] {
        assert_eq!(
            std::fs::metadata(directory.path().join(name))?.mode() & 0o777,
            0o600
        );
    }
    let events = fixture.events.0.lock().map_err(|_| "events")?;
    assert_eq!(events.iter().filter(|event| matches!(event,GatewayEvent::Attempt(value) if value.request_id().as_str()==selected_id && matches!(value.outcome(),AttemptOutcome::Succeeded))).count(),1);
    Ok(())
}

#[actix_web::test]
async fn grok_build_pre_admission_errors_never_invent_request_id() -> TestResult {
    let peer = Arc::new(ProjectionPeer {
        output: rich_output(),
        usage: None,
        received: AtomicUsize::new(0),
    });
    let fixture = build_fixture(&peer)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    for protocol in ["chat", "messages", "responses"] {
        let (uri, body) = request(protocol, false);
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(uri)
                .insert_header(("x-request-id", "forged"))
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get("x-request-id").is_none());
    }
    assert!(fixture.events.0.lock().map_err(|_| "events")?.is_empty());
    assert_eq!(peer.received.load(Ordering::Relaxed), 0);
    Ok(())
}

#[actix_web::test]
async fn grok_build_diagnostic_rejects_unsafe_directory_or_source() -> TestResult {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let directory = TemporaryDirectory::new()?;
    let hash = "a".repeat(64);
    let canonical = directory.path().canonicalize()?;
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o755))?;
    assert!(
        build_diagnostic::BuildDiagnostic::open(
            &canonical,
            &hash,
            env!("GATEWAY_RELEASE_REVISION")
        )
        .is_err()
    );
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))?;
    assert!(
        build_diagnostic::BuildDiagnostic::open(&canonical, &hash, "unreviewed-source").is_err()
    );
    assert!(
        build_diagnostic::BuildDiagnostic::open(
            &canonical,
            "invalid",
            env!("GATEWAY_RELEASE_REVISION")
        )
        .is_err()
    );
    let alias = directory.path().join("alias");
    symlink(directory.path(), &alias)?;
    assert!(
        build_diagnostic::BuildDiagnostic::open(&alias, &hash, env!("GATEWAY_RELEASE_REVISION"))
            .is_err()
    );
    assert!(
        build_diagnostic::BuildDiagnostic::open(
            &canonical,
            &hash,
            env!("GATEWAY_RELEASE_REVISION")
        )
        .is_ok()
    );
    Ok(())
}
