//! FIN and RST at the public listener, before headers and during an idle response.
use super::*;

#[actix_web::test]
async fn public_build_fin_and_reset_cancel_headers_and_idle_sse() -> TestResult {
    for headers in [true, false] {
        for reset in [false, true] {
            let peer = SocketPeer::start(
                partial_wire(),
                if headers {
                    PeerEnd::AwaitHeaders
                } else {
                    PeerEnd::Hold
                },
            )?;
            let fixture = lifecycle_fixture(
                peer.transport(Duration::from_secs(30), Duration::from_mins(1))?,
            )?;
            let recording = Recording::start(&fixture, false)?;
            let (mut socket, server) = public_socket(&fixture).await?;
            assert!(
                wait_until(|| {
                    active_leases(&fixture) == 1
                        && peer.received.lock().is_ok_and(|rows| rows.len() == 1)
                })
                .await
            );
            if !headers {
                let wire = read_public_until(&mut socket, "partial-61").await?;
                assert!(wire.contains("200 OK") && !wire.contains("response.completed"));
            }
            let started = std::time::Instant::now();
            if reset {
                socket.set_zero_linger()?;
                drop(socket);
            } else {
                // Send FIN but retain the read half. Public inference requires both halves
                // open; this must cancel even though no response write is currently ready.
                socket.shutdown().await?;
                // Keep the socket alive throughout the resource observation below.
                let released = wait_until(|| {
                    peer.closed.load(Ordering::SeqCst) == 1 && active_leases(&fixture) == 0
                })
                .await;
                eprintln!(
                    "public FIN headers={headers}: released={released}, elapsed_ms={}, lease={}, peer_close={}",
                    started.elapsed().as_millis(),
                    active_leases(&fixture),
                    peer.closed.load(Ordering::SeqCst)
                );
                assert!(
                    released,
                    "public FIN must release before client/server cleanup; headers={headers}"
                );
                drop(socket);
            }
            assert!(
                wait_until(|| {
                    peer.closed.load(Ordering::SeqCst) == 1 && active_leases(&fixture) == 0
                })
                .await,
                "public disconnect must release before server cleanup"
            );
            assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
            server.stop(false).await;
            let database = recording.finish(&fixture, 0).await?;
            let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
            assert!(
                rows.iter()
                    .all(|row| !matches!(row.event(), GatewayEvent::Usage(_)))
            );
            let finished: Vec<_> = rows
                .iter()
                .filter_map(|row| {
                    if let GatewayEvent::RequestFinished(event) = row.event() {
                        Some(event)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(finished.len(), 1);
            assert_eq!(finished[0].outcome, gateway_core::RequestOutcome::Cancelled);
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_open_connection_completes_without_half_close() -> TestResult {
    let peer = SocketPeer::start(complete_wire(None), PeerEnd::Eof)?;
    let fixture =
        lifecycle_fixture(peer.transport(Duration::from_secs(30), Duration::from_mins(1))?)?;
    let recording = Recording::start(&fixture, false)?;
    let (mut socket, server) = public_socket(&fixture).await?;
    let wire = read_public_until(&mut socket, "response.completed").await?;
    assert!(wire.contains("200 OK") && wire.contains("partial-61"));
    assert_eq!(wire.matches("event: response.completed").count(), 1);
    assert!(!wire.contains("event: response.failed"));
    assert!(wait_until(|| active_leases(&fixture) == 0).await);
    assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
    server.stop(false).await;
    let database = recording.finish(&fixture, 0).await?;
    let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
    assert_eq!(
        rows.iter()
            .filter(|row| matches!(row.event(), GatewayEvent::Usage(_)))
            .count(),
        1
    );
    Ok(())
}
