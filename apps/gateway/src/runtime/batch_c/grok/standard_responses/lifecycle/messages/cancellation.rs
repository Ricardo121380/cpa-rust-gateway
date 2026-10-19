//! Real socket FIN/RST and shared-pump cancellation, with production listener half-close policy.
use super::*;

fn request() -> Value {
    json!({"model":"p12-test-model","max_tokens":19,"stream":true,"messages":[{"role":"user","content":"controlled cancellation"}]})
}
fn partial() -> String {
    let reason = json!({"id":"reason-cancel-63","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"计划 取消"}]});
    let mut added = reason.clone();
    added["status"] = json!("in_progress");
    added["summary"] = json!([]);
    frame("response.created", &created())
        + &native_event(
            "response.output_item.added",
            json!({"output_index":0,"item":added}),
        )
        + &native_event(
            "response.reasoning_summary_text.delta",
            json!({"output_index":0,"item_id":"reason-cancel-63","summary_index":0,"delta":"计划 取消"}),
        )
        + &native_event(
            "response.output_item.done",
            json!({"output_index":0,"item":reason}),
        )
        + &native_event(
            "response.output_item.added",
            json!({"output_index":1,"item":{"id":"message-cancel-63","type":"message","status":"in_progress","role":"assistant","content":[]}}),
        )
        + &native_event(
            "response.output_text.delta",
            json!({"output_index":1,"item_id":"message-cancel-63","content_index":0,"delta":"cancel-63"}),
        )
}

fn cancelled_rows(
    database: &std::path::Path,
    usage: bool,
    attempts: usize,
) -> Result<String, Box<dyn Error>> {
    let rows = SqliteEventStore::open_read_only(database)?.list_events()?;
    assert_eq!(
        rows.iter()
            .filter(|r| matches!(r.event(), GatewayEvent::Attempt(_)))
            .count(),
        attempts
    );
    assert_eq!(
        rows.iter()
            .filter(|r| matches!(r.event(), GatewayEvent::Usage(_)))
            .count(),
        usize::from(usage)
    );
    let finished: Vec<_> = rows
        .iter()
        .filter_map(|r| match r.event() {
            GatewayEvent::RequestFinished(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].outcome, gateway_core::RequestOutcome::Cancelled);
    Ok(finished[0].request_id.as_str().into())
}

#[actix_web::test]
async fn public_build_messages_fin_rst_cancel_headers_idle_and_keep_confirmed_items() -> TestResult
{
    for headers in [true, false] {
        for reset in [false, true] {
            let peer = SocketPeer::start(
                partial(),
                if headers {
                    PeerEnd::AwaitHeaders
                } else {
                    PeerEnd::Hold
                },
            )?;
            // These deadlines are deliberately longer than the existing one-second resource
            // observation. Their expiry cannot manufacture cancellation success.
            let fixture = lifecycle_fixture(
                peer.transport(Duration::from_secs(30), Duration::from_mins(1))?,
            )?;
            let recording = Recording::start(&fixture, false)?;
            let (mut socket, server) =
                public_socket_request(&fixture, "/v1/messages", request()).await?;
            assert!(
                wait_until(|| active_leases(&fixture) == 1
                    && peer.received.lock().is_ok_and(|r| r.len() == 1))
                .await
            );
            if !headers {
                let bytes = read_public_until(&mut socket, "cancel-63").await?;
                assert!(!bytes.contains("message_stop") && !bytes.contains("计划"));
            }
            let start = std::time::Instant::now();
            if reset {
                socket.set_zero_linger()?;
                drop(socket);
            } else {
                socket.shutdown().await?;
                assert!(
                    wait_until(
                        || peer.closed.load(Ordering::SeqCst) == 1 && active_leases(&fixture) == 0
                    )
                    .await
                );
                // Keep the FIN client's read half alive through independent release observation.
                drop(socket);
            }
            assert!(
                wait_until(
                    || peer.closed.load(Ordering::SeqCst) == 1 && active_leases(&fixture) == 0
                )
                .await
            );
            eprintln!(
                "Messages cancellation headers={headers} reset={reset}: release observed {}ms",
                start.elapsed().as_millis()
            );
            assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
            server.stop(false).await;
            let database = recording.finish(&fixture, 0).await?;
            let id = cancelled_rows(&database, false, usize::from(!headers))?;
            if !headers {
                let record = retained(&database, &id)?;
                assert_eq!(
                    record.evidence.completeness(),
                    gateway_core::MessagesSummaryCompleteness::ConfirmedItems
                );
                assert!(record.evidence.items()[0].item.get().contains("计划 取消"));
            }
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
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_cancel_pending_usage_releases_source_before_same_receipt()
-> TestResult {
    for reset in [false, true] {
        let peer = SocketPeer::start(tool_rounds::wire(1, true)?, PeerEnd::Hold)?;
        let fixture =
            lifecycle_fixture(peer.transport(Duration::from_secs(30), Duration::from_mins(1))?)?;
        let recording = Recording::start(&fixture, true)?;
        let receipt = recording.receipt.as_ref().ok_or("held receipt")?.clone();
        let (mut socket, server) =
            public_socket_request(&fixture, "/v1/messages", request()).await?;
        let visible = read_public_until(&mut socket, "回答").await?;
        assert!(!visible.contains("message_stop"));
        tokio::time::timeout(Duration::from_secs(5), receipt.entered.notified()).await?;
        assert_eq!(active_leases(&fixture), 1);
        if reset {
            socket.set_zero_linger()?;
            drop(socket);
        } else {
            socket.shutdown().await?;
            assert!(
                wait_until(
                    || peer.closed.load(Ordering::SeqCst) == 1 && active_leases(&fixture) == 0
                )
                .await
            );
            drop(socket);
        }
        assert!(
            wait_until(|| peer.closed.load(Ordering::SeqCst) == 1 && active_leases(&fixture) == 0)
                .await
        );
        assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
        receipt.release.notify_one();
        let rows = SqliteEventStore::open_read_only(&recording.database)?.list_events()?;
        let request = rows
            .iter()
            .find_map(|r| match r.event() {
                GatewayEvent::Request(r) => Some(r),
                _ => None,
            })
            .ok_or("request")?;
        assert!(
            wait_until(|| retained(&recording.database, request.request_id().as_str()).is_ok())
                .await,
            "cancel cleanup must confirm summary before server cleanup"
        );
        drop(receipt);
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        let id = cancelled_rows(&database, true, 1)?;
        let record = retained(&database, &id)?;
        assert_eq!(
            record.evidence.completeness(),
            gateway_core::MessagesSummaryCompleteness::ConfirmedItems
        );
        accounting(&database, 1).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_eof_idle_and_total_deadlines_fail_once_without_resend() -> TestResult
{
    for (end, idle, total) in [
        (
            PeerEnd::Truncate,
            Duration::from_secs(3),
            Duration::from_secs(5),
        ),
        (PeerEnd::Eof, Duration::from_secs(3), Duration::from_secs(5)),
        (
            PeerEnd::Hold,
            Duration::from_millis(250),
            Duration::from_secs(3),
        ),
        (
            PeerEnd::Heartbeats,
            Duration::from_millis(200),
            Duration::from_millis(500),
        ),
    ] {
        let peer = SocketPeer::start(partial(), end)?;
        let fixture = lifecycle_fixture(peer.transport(idle, total)?)?;
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
                .set_json(request())
                .to_request(),
        )
        .await;
        let id = reply
            .headers()
            .get("x-request-id")
            .ok_or("id")?
            .to_str()?
            .to_owned();
        let bytes = test::read_body(reply).await;
        failures::assert_failed(&bytes)?;
        assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
        assert!(wait_until(|| active_leases(&fixture) == 0).await);
        let record = retained(&recording.database, &id)?;
        assert_eq!(
            record.evidence.completeness(),
            gateway_core::MessagesSummaryCompleteness::ConfirmedItems
        );
        drop(app);
        let database = recording.finish(&fixture, 0).await?;
        assert!(
            !SqliteEventStore::open_read_only(database)?
                .list_events()?
                .iter()
                .any(|r| matches!(r.event(), GatewayEvent::Usage(_)))
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_cancel_pending_summary_keeps_native_complete_delivery_cancelled()
-> TestResult {
    let peer = SocketPeer::start(tool_rounds::wire(1, true)?, PeerEnd::Hold)?;
    let fixture =
        lifecycle_fixture(peer.transport(Duration::from_secs(30), Duration::from_mins(1))?)?;
    let recording = Recording::start(&fixture, false)?;
    let receipt = failures::wrap(&fixture, &recording.database, failures::Fault::Hold)?;
    let (mut socket, server) = public_socket_request(&fixture, "/v1/messages", request()).await?;
    let mut wire = read_public_until(&mut socket, "回答").await?;
    tokio::time::timeout(Duration::from_secs(5), receipt.entered.notified()).await?;
    assert!(
        wait_until(|| peer.closed.load(Ordering::SeqCst) == 1 && active_leases(&fixture) == 0)
            .await
    );
    let rows = SqliteEventStore::open_read_only(&recording.database)?.list_events()?;
    let request_id = rows
        .iter()
        .find_map(|r| match r.event() {
            GatewayEvent::Request(request) => Some(request.request_id().as_str().to_owned()),
            _ => None,
        })
        .ok_or("request")?;
    let record = retained(&recording.database, &request_id)?;
    assert_eq!(
        record.evidence.completeness(),
        gateway_core::MessagesSummaryCompleteness::CompletedResponse
    );
    assert!(!wire.contains("message_stop"));
    socket.shutdown().await?;
    assert!(wait_until(||SqliteEventStore::open_read_only(&recording.database).and_then(|s|s.list_events())
        .is_ok_and(|rows|rows.iter().any(|r|matches!(r.event(),GatewayEvent::RequestFinished(f) if f.outcome==gateway_core::RequestOutcome::Cancelled)))).await);
    receipt.release.notify_one();
    let mut tail = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), socket.read_to_end(&mut tail)).await??;
    wire += std::str::from_utf8(&tail)?;
    assert!(!wire.contains("message_stop"));
    assert_eq!(receipt.submissions.load(Ordering::SeqCst), 1);
    assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
    drop(socket);
    drop(receipt);
    server.stop(false).await;
    let database = recording.finish(&fixture, 0).await?;
    cancelled_rows(&database, true, 1)?;
    accounting(&database, 1).await?;
    Ok(())
}
