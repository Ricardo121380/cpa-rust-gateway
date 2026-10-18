//! A real conflicting encrypted companion at the event port; no field admission is implied.
use super::*;

const ORIGINAL: &str =
    r#"{"input_tokens":20,"output_tokens":7,"total_tokens":27,"cost_in_usd_ticks":1}"#;

struct ConflictingEvidence(Arc<dyn GatewayEventSink>);
impl GatewayEventSink for ConflictingEvidence {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        self.0.try_emit(event)
    }
    fn emit_confirmed(&self, event: GatewayEvent) -> gateway_core::EventEmissionFuture<'_> {
        self.0.emit_confirmed(event)
    }
    fn emit_usage_evidence_confirmed(
        &self,
        event: gateway_core::UsageEvent,
        evidence: gateway_core::NativeUsageEvidence,
    ) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            let first = self
                .0
                .emit_usage_evidence_confirmed(event.clone(), evidence.clone())
                .await;
            if first != EventEmission::Persisted {
                return first;
            }
            // Model a competing same-Attempt companion with changed bytes but identical known
            // counters. Both submissions use the real queue, encryption and SQLite transaction.
            let changed = gateway_core::NativeUsageEvidence::try_new(
                evidence.source(),
                evidence
                    .raw_json()
                    .replace("\"cost_in_usd_ticks\":1", "\"cost_in_usd_ticks\":2"),
            );
            match changed {
                Ok(changed) => self.0.emit_usage_evidence_confirmed(event, changed).await,
                Err(_) => EventEmission::PersistenceUnavailable,
            }
        })
    }
}

#[actix_web::test]
async fn public_build_evidence_conflict_fails_once_and_keeps_original_exact_usage() -> TestResult {
    let peer = SocketPeer::start(complete_wire(Some(ORIGINAL)), PeerEnd::Hold)?;
    let fixture =
        lifecycle_fixture(peer.transport(Duration::from_secs(5), Duration::from_secs(10))?)?;
    let recording = Recording::start(&fixture, false)?;
    {
        let mut sink = fixture.events.1.lock().map_err(|_| "sink")?;
        let inner = sink.take().ok_or("writer sink")?;
        *sink = Some(Arc::new(ConflictingEvidence(inner)));
    }
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let reply = test::call_service(&app, test::TestRequest::post().uri("/v1/responses")
        .insert_header(("authorization",format!("Bearer {}",fixture.key)))
        .set_json(json!({"model":"p12-test-model","input":"controlled evidence conflict","stream":true})).to_request()).await;
    assert_eq!(reply.status(), StatusCode::OK);
    let bytes = test::read_body(reply).await;
    let wire = std::str::from_utf8(&bytes)?;
    let frames = public_frames(&bytes)?;
    assert!(wire.contains("partial-61") && wire.contains("RecordingUnavailable"));
    assert_eq!(
        frames
            .iter()
            .filter(|v| v["type"] == "response.failed")
            .count(),
        1
    );
    assert!(frames.iter().all(|v| v["type"] != "response.completed"));
    assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
    assert!(wait_until(|| peer.closed.load(Ordering::SeqCst) == 1).await);
    assert_eq!(active_leases(&fixture), 0);
    drop(app);
    let database = recording.finish(&fixture, 1).await?;
    let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
    let requests: Vec<_> = rows
        .iter()
        .filter_map(|r| {
            if let GatewayEvent::Request(v) = r.event() {
                Some(v)
            } else {
                None
            }
        })
        .collect();
    let attempts: Vec<_> = rows
        .iter()
        .filter_map(|r| {
            if let GatewayEvent::Attempt(v) = r.event() {
                Some(v)
            } else {
                None
            }
        })
        .collect();
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
    assert_eq!((requests.len(), attempts.len(), usages.len()), (1, 1, 1));
    let record = SqliteNativeUsageReader::open(&database, test_secret_store()?)?
        .get_owned(
            requests[0].client_key_id(),
            requests[0].request_id(),
            attempts[0].attempt_id(),
            system_now_ms_runtime()?,
        )?
        .ok_or("original exact evidence")?;
    assert_eq!(record.evidence.raw_json(), ORIGINAL);
    assert_eq!(record.usage, *usages[0]);
    assert_eq!(record.completeness, "complete_usage_object");
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
    assert_eq!(
        finished[0].error_code,
        Some(GatewayErrorCode::RecordingUnavailable)
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
    Ok(())
}
