//! M4 recording barriers are exercised through public HTTP and the existing finite writer.
use super::super::super::client_metadata::{HeldMetadata, MetadataFault, ReceiptFault};
use super::*;

#[actix_web::test]
async fn public_build_messages_m4_recording_failures_never_send_or_acknowledge_success()
-> TestResult {
    for streaming in [false, true] {
        for fault in [
            "missing-writer",
            "missing-key",
            "conflict",
            "lost",
            "locked",
            "replay",
            "enqueued",
            "disabled",
            "queue-full",
        ] {
            let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
            let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
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
                "conflict" | "lost" | "replay" | "enqueued" | "disabled" | "queue-full"
            ) {
                let mut sink = fixture.events.1.lock().map_err(|_| "sink")?;
                *sink = Some(Arc::new(MetadataFault {
                    inner: sink.take().ok_or("writer")?,
                    fault: match fault {
                        "conflict" => ReceiptFault::Conflict,
                        "lost" => ReceiptFault::Lost,
                        "enqueued" => ReceiptFault::Enqueued,
                        "disabled" => ReceiptFault::Disabled,
                        "queue-full" => ReceiptFault::QueueFull,
                        _ => ReceiptFault::Replay,
                    },
                }));
            }
            let lock = if fault == "locked" {
                let connection =
                    gateway_store::open(&recording.as_ref().ok_or("writer")?.database)?;
                connection.execute_batch("BEGIN IMMEDIATE")?;
                Some(connection)
            } else {
                None
            };
            let (url, server) = namespaces::listener(&fixture)?;
            let (status, id, bytes) = send(&fixture, &url, body(streaming, true, true)).await?;
            let sent = usize::from(fault == "replay");
            assert_eq!(status, if sent == 1 { 200 } else { 503 }, "{fault}");
            assert_eq!(peer.accepted.load(Ordering::SeqCst), sent, "{fault}");
            assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), sent);
            assert_eq!(active_leases(&fixture), 0);
            assert!(!String::from_utf8_lossy(&bytes).contains("M4_PRIVATE"));
            if sent == 0 {
                assert!(!String::from_utf8_lossy(&bytes).contains("message_stop"));
            }
            if let Some(lock) = lock {
                lock.execute_batch("ROLLBACK")?;
            }
            server.stop(false).await;
            if let Some(recording) = recording {
                let database = recording
                    .finish(
                        &fixture,
                        u64::from(matches!(fault, "missing-key" | "conflict")),
                    )
                    .await?;
                if fault != "missing-key" {
                    assert_metadata(&fixture, &id, METADATA, sent)?;
                }
                if sent == 0 {
                    let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
                    assert!(rows.iter().all(|r| !matches!(
                        r.event(),
                        GatewayEvent::Attempt(_) | GatewayEvent::Usage(_)
                    )));
                    assert!(rows.iter().any(|r| matches!(r.event(),GatewayEvent::RequestFinished(f)
                        if f.outcome==gateway_core::RequestOutcome::Failed && f.error_code==Some(GatewayErrorCode::RecordingUnavailable))));
                }
            }
            eprintln!(
                "CPAR64_M4_FAULT {}",
                json!({"fault":fault,"streaming":streaming,"status":status,"receive":sent,
                "late_commit_cannot_change_failure":fault=="locked","confirmation_timeout":if fault=="locked" {"UNKNOWN"} else {"NOT_APPLICABLE"}})
            );
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_m4_confirmation_precedes_lease_and_only_captures_once() -> TestResult
{
    let peer = ModelPeer::start_wire(false, tool_rounds::wire(1, false)?)?;
    let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
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
    let fixture_ref = &fixture;
    let request = send(fixture_ref, &url, body(false, true, true));
    tokio::pin!(request);
    tokio::select! {
        ()=held.entered.notified()=>{},
        result=&mut request=>return Err(format!("request passed confirmation barrier: {}",result?.0).into()),
        ()=tokio::time::sleep(Duration::from_secs(5))=>return Err("missing confirmation entry".into()),
    }
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
    assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
    assert_eq!(active_leases(&fixture), 0);
    let rows = SqliteEventStore::open_read_only(&recording.database)?.list_events()?;
    let id = rows
        .iter()
        .find_map(|r| match r.event() {
            GatewayEvent::Request(r) => Some(r.request_id().as_str()),
            _ => None,
        })
        .ok_or("request")?;
    assert_metadata(&fixture, id, METADATA, 0)?;
    held.release.notify_one();
    let (status, id, _) = request.await?;
    assert_eq!(status, 200);
    assert_eq!(held.submissions.load(Ordering::SeqCst), 1);
    assert_metadata(&fixture, &id, METADATA, 1)?;
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
    server.stop(false).await;
    drop(held);
    recording.finish(&fixture, 0).await?;
    Ok(())
}
