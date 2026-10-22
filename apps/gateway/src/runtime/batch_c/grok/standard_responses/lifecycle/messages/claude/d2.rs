//! Independent D2 candidate, never a qualification of original Claude or the fixed service.
use super::*;
mod oracles;
mod peer;

#[actix_web::test]
#[ignore = "requires frozen unmodified Claude and the isolated #64 D2 runner"]
#[allow(
    clippy::too_many_lines,
    reason = "the three fixed actual-client scenarios share one assembly and cleanup sequence"
)]
async fn actual_claude_messages_d2_qualification() -> TestResult {
    let run = std::path::PathBuf::from(std::env::var("CPAR64_RUN")?);
    assert!(run.is_absolute() && run.is_dir());
    for scenario in ["standard", "failure", "cancel"] {
        let directory = run.join(format!("d2-{scenario}"));
        std::fs::create_dir(&directory)?;
        let peer = match scenario {
            "standard" => peer::start(&directory)?,
            "failure" => {
                let mut failed = response();
                failed["status"] = json!("failed");
                failed["error"] =
                    json!({"code":"server_error","message":"synthetic D2 execution failure"});
                SocketPeer::start(
                    partial_wire() + &frame("response.failed", &failed),
                    PeerEnd::Failure,
                )?
            }
            _ => SocketPeer::start(partial_wire(), PeerEnd::Hold)?,
        };
        let fixture = lifecycle_fixture(peer.transport(
            Duration::from_secs(if scenario == "cancel" { 30 } else { 5 }),
            Duration::from_secs(if scenario == "cancel" { 60 } else { 15 }),
        )?)?;
        let recording = Recording::start(&fixture, false)?;
        let ingress = web::Data::new(Mutex::new(Vec::<Captured>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let state = fixture.state.clone();
        let requests = ingress.clone();
        let server = actix_web::HttpServer::new(move || {
            App::new()
                .wrap(from_fn(capture))
                .app_data(requests.clone())
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
        let driver = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/acceptance/cpar64-claude.py");
        let key = fixture.key.clone();
        let child_directory = directory.clone();
        let child = tokio::task::spawn_blocking(move || {
            std::process::Command::new("/usr/bin/python3")
                .arg(driver)
                .arg("client")
                .arg(child_directory)
                .arg(format!("http://{address}"))
                .arg(scenario)
                .env_clear()
                .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
                .env("CPAR64_TEST_KEY", key)
                .env("CPAR64_D2", "1")
                .output()
        });
        let cancellation = if scenario == "cancel" {
            Some(observe_cancel(&fixture, &peer, &directory, &recording.database).await)
        } else {
            None
        };
        let result = tokio::time::timeout(Duration::from_secs(100), child).await???;
        std::fs::write(directory.join("driver.stdout"), &result.stdout)?;
        std::fs::write(directory.join("driver.stderr"), &result.stderr)?;
        let released = wait_until(|| active_leases(&fixture) == 0).await;
        handle.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        export_capture(&ingress, &directory)?;
        std::fs::write(
            directory.join("upstream.json"),
            serde_json::to_vec_pretty(&*peer.received.lock().map_err(|_| "receiver")?)?,
        )?;
        std::fs::write(
            directory.join("receiver-receipts.json"),
            serde_json::to_vec_pretty(&*peer.receipts.lock().map_err(|_| "receiver")?)?,
        )?;
        std::fs::write(
            directory.join("events.json"),
            serde_json::to_vec_pretty(
                &SqliteEventStore::open_read_only(&database)?
                    .list_events()?
                    .iter()
                    .map(gateway_store::event_store::StoredGatewayEvent::event)
                    .collect::<Vec<_>>(),
            )?,
        )?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        if let Some(cancelled) = cancellation {
            cancelled?;
        }
        assert!(released);
        oracles::verify(&fixture, &peer, &ingress, &directory, &database, scenario).await?;
    }
    Ok(())
}

async fn observe_cancel(
    fixture: &Fixture,
    peer: &SocketPeer,
    directory: &std::path::Path,
    database: &std::path::Path,
) -> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        while !directory.join("cancel-ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(active_leases(fixture), 1);
    let ready: Value = serde_json::from_slice(&std::fs::read(directory.join("cancel-ready"))?)?;
    assert_eq!(ready["alive"], true);
    std::fs::write(
        directory.join("cancel-proceed.pending"),
        b"lease=1; native interrupt may proceed\n",
    )?;
    std::fs::rename(
        directory.join("cancel-proceed.pending"),
        directory.join("cancel-proceed"),
    )?;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !directory.join("abort-complete").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let aborted: Value = serde_json::from_slice(&std::fs::read(directory.join("abort-complete"))?)?;
    assert_eq!(aborted["rpc_alive"], true);
    assert_eq!(aborted["pid"], ready["pid"]);
    tokio::time::timeout(Duration::from_secs(20),async {
        loop {
            let rows = SqliteEventStore::open_read_only(database)?.list_events()?;
            let cancelled = rows.iter().any(|row| matches!(row.event(),GatewayEvent::RequestFinished(f) if f.outcome==gateway_core::RequestOutcome::Cancelled));
            if cancelled && active_leases(fixture)==0 && peer.closed.load(Ordering::SeqCst)==1 { return Ok::<(),Box<dyn Error>>(()); }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await??;
    std::fs::write(
        directory.join("cancel-diagnostic.json"),
        serde_json::to_vec_pretty(&json!({"pid":ready["pid"],"same_pid_alive_at_abort":true,
        "lease_before":1,"lease_after":active_leases(fixture),"peer_closed":peer.closed.load(Ordering::SeqCst),"durable_outcome":"Cancelled",
        "observed_at_ms":system_now_ms_runtime()?,"abort":aborted}))?,
    )?;
    std::fs::write(
        directory.join("cancel-observed.pending"),
        b"peer closed, lease=0, durable Cancelled; cleanup permitted\n",
    )?;
    std::fs::rename(
        directory.join("cancel-observed.pending"),
        directory.join("cancel-observed"),
    )?;
    Ok(())
}
