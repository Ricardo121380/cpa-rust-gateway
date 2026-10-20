//! Actual, unmodified Claude Code over the production Messages and leased Build assembly.
use super::super::cli::{Captured, Ingress, capture};
use super::*;
use actix_web::middleware::from_fn;
use sha2::{Digest, Sha256};
mod controls;

#[actix_web::test]
#[ignore = "requires the #64 frozen Claude executable and OS isolation runner"]
async fn actual_claude_messages_build_observation() -> TestResult {
    let run = std::path::PathBuf::from(std::env::var("CPAR64_RUN")?);
    assert!(run.is_absolute() && run.is_dir());
    let directory = run.join("standard");
    std::fs::create_dir(&directory)?;
    let peer = ModelPeer::start_wire(true, tool_rounds::wire(1, true)?)?;
    let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
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
    let directory_arg = directory.clone();
    let child = tokio::task::spawn_blocking(move || {
        std::process::Command::new("/usr/bin/python3")
            .arg(driver)
            .arg("client")
            .arg(directory_arg)
            .arg(format!("http://{address}"))
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("CPAR64_TEST_KEY", key)
            .output()
    });
    let result = tokio::time::timeout(Duration::from_secs(90), child).await???;
    std::fs::write(directory.join("driver.stdout"), &result.stdout)?;
    std::fs::write(directory.join("driver.stderr"), &result.stderr)?;
    let released = wait_until(|| active_leases(&fixture) == 0).await;
    handle.stop(false).await;
    let database = recording.finish(&fixture, 0).await?;
    export_capture(&ingress, &directory)?;
    let received: Vec<_> = peer
        .requests
        .lock()
        .map_err(|_| "receiver")?
        .iter()
        .enumerate()
        .map(|(index, v)| {
            let path = format!("upstream-{index}.bin");
            std::fs::write(directory.join(&path), &v.wire)?;
            Ok(
                json!({"body":v.body,"model_override":v.model_override,"wire_file":path,
                "wire_sha256":format!("{:x}",Sha256::digest(&v.wire))}),
            )
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    std::fs::write(
        directory.join("upstream.json"),
        serde_json::to_vec_pretty(&received)?,
    )?;
    let events = SqliteEventStore::open_read_only(&database)?.list_events()?;
    std::fs::write(
        directory.join("events.json"),
        serde_json::to_vec_pretty(
            &events
                .iter()
                .map(gateway_store::event_store::StoredGatewayEvent::event)
                .collect::<Vec<_>>(),
        )?,
    )?;
    let accounting = rejected_accounting(&database).await?;
    let rows = ingress.lock().map_err(|_| "ingress")?;
    assert_eq!(accounting["requests"], json!(rows.len()));
    assert!(
        !rows.is_empty(),
        "missing actual HTTP traffic is a harness failure, not a protocol RED"
    );
    for row in rows.iter() {
        assert_eq!(row.method, "POST");
        assert_eq!(row.uri, "/v1/messages?beta=true");
        assert_eq!(row.status, 400);
        assert!(!row.wire_truncated && row.body_complete);
        assert_eq!(
            serde_json::from_slice::<Value>(&row.wire)?,
            json!({"type":"error",
            "error":{"type":"invalid_request_error","message":"the client request is invalid"}})
        );
    }
    assert!(received.is_empty());
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
    std::fs::write(
        directory.join("observations.json"),
        serde_json::to_vec_pretty(&json!({
            "independent_receives":received.len(),"active_leases":active_leases(&fixture),
            "independent_tcp_accepts":peer.accepted.load(Ordering::SeqCst),
            "public_requests":rows.len(),"accounting":accounting,
            "no_lease_leak_before_cleanup":released,"lease_release_transition":"NOT_OBSERVED_NO_LEASE",
            "cost":"UNKNOWN", "compatibility":"BLOCKED",
            "actual_tool_execution":"BLOCKED_BEFORE_FIRST_RESPONSE",
            "actual_summary_readback":"BLOCKED_NO_ATTEMPT",
            "actual_inflight_cancellation":"BLOCKED_NO_DELTA_OR_LEASE",
            "count_tokens":"NOT_OBSERVED", "title":"NOT_OBSERVED", "head_probe":"NOT_OBSERVED"
        }))?,
    )?;
    assert!(
        result.status.success(),
        "driver: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(released);
    // This is an observation oracle. The Python runner separately fails the compatibility gate.
    Ok(())
}

async fn rejected_accounting(database: &std::path::Path) -> Result<Value, Box<dyn Error>> {
    let events = SqliteEventStore::open_read_only(database)?.list_events()?;
    let requests = events
        .iter()
        .filter(|r| matches!(r.event(), GatewayEvent::Request(_)))
        .count();
    let failures = events
        .iter()
        .filter(|r| {
            matches!(r.event(), GatewayEvent::RequestFinished(v)
        if v.outcome == gateway_core::RequestOutcome::Failed
            && v.error_code == Some(GatewayErrorCode::ClientRequestError))
        })
        .count();
    assert_eq!(requests, failures);
    assert!(
        events
            .iter()
            .all(|r| !matches!(r.event(), GatewayEvent::Attempt(_) | GatewayEvent::Usage(_)))
    );
    let worker = crate::billing_worker::BillingWorker::start(
        database.to_path_buf(),
        Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
    );
    worker.stop().await.map_err(|()| "billing")?;
    assert!(
        SqliteBillingLedger::open_read_only(database)?
            .list_bounded(100)?
            .is_empty()
    );
    let summaries: i64 = gateway_store::open(database)?.query_row(
        "SELECT count(*) FROM messages_summary_evidence",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(summaries, 0);
    Ok(
        json!({"requests":requests,"failed_requests":failures,"attempts":0,"usage":0,
        "ledger":0,"summaries":0,"cost":"UNKNOWN"}),
    )
}

fn export_capture(ingress: &Ingress, directory: &std::path::Path) -> TestResult {
    let rows = ingress.lock().map_err(|_| "ingress")?;
    let mut receipts = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let request = format!("request-{index}.bin");
        let response = format!("response-{index}.bin");
        std::fs::write(directory.join(&request), &row.request_bytes)?;
        std::fs::write(directory.join(&response), &row.wire)?;
        receipts.push(json!({"method":row.method,"path_query":row.uri,"probe":row.probe,
            "request_file":request,"request_sha256":format!("{:x}",Sha256::digest(&row.request_bytes)),
            "body":row.request,"headers":row.headers,"status":row.status,
            "response_headers":row.response_headers,"response_file":response,
            "response_sha256":format!("{:x}",Sha256::digest(&row.wire)),"chunks":row.chunks,
            "chunk_elapsed_ms":row.chunk_elapsed_ms,"capture_truncated":row.wire_truncated,
            "body_complete":row.body_complete}));
    }
    std::fs::write(
        directory.join("ingress.json"),
        serde_json::to_vec_pretty(&receipts)?,
    )?;
    Ok(())
}
