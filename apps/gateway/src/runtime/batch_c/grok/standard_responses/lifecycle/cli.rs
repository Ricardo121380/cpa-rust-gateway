//! Opt-in actual CLI qualification through the production public HTTP and leased Build chain.
use super::*;
use actix_web::middleware::from_fn;
mod http;
mod oracles;
mod peer;
use http::{assert_presend_rejections, capture};
use oracles::{assert_accounting, assert_history, assert_public};
use peer::scripted_peer;

#[derive(Default)]
struct Captured {
    path: String,
    probe: bool,
    request: Value,
    headers: Value,
    status: u16,
    chunks: usize,
    wire: Vec<u8>,
}
type Ingress = Mutex<Vec<Captured>>;

#[actix_web::test]
#[ignore = "requires fixed local Codex/Pi installs and the macOS loopback-only runner"]
async fn actual_cli_public_build_qualification() -> TestResult {
    let run = std::path::PathBuf::from(std::env::var("CPAR62_RUN")?);
    assert!(run.is_absolute() && run.is_dir());
    for (client, scenario) in [
        ("codex", "standard"),
        ("pi", "standard"),
        ("pi", "failure"),
        ("pi", "cancel"),
    ] {
        let peer = match scenario {
            "failure" => {
                let mut failed = response();
                failed["status"] = json!("failed");
                failed["error"] = json!({"code":"server_error","message":"synthetic #62 failure"});
                SocketPeer::start(
                    partial_wire() + &frame("response.failed", &failed),
                    PeerEnd::Eof,
                )?
            }
            "cancel" => SocketPeer::start(partial_wire(), PeerEnd::Hold)?,
            _ => scripted_peer()?,
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
        .listen(listener)?
        .run();
        let handle = server.handle();
        actix_web::rt::spawn(server);
        let directory = run.join(format!("{client}-{scenario}"));
        std::fs::create_dir(&directory)?;
        let driver = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/acceptance/cpar62-cli.py");
        let key = fixture.key.clone();
        let directory_arg = directory.clone();
        let child = tokio::task::spawn_blocking(move || {
            std::process::Command::new("/usr/bin/python3")
                .arg(driver)
                .arg("client")
                .arg(client)
                .arg(scenario)
                .arg(directory_arg)
                .arg(format!("http://{address}/v1"))
                .env("CPAR62_TEST_KEY", key)
                .output()
        });
        if scenario == "cancel" {
            tokio::time::timeout(Duration::from_secs(10), async {
                while !directory.join("cancel-ready").exists() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await?;
            assert_eq!(
                active_leases(&fixture),
                1,
                "actual Pi has received text while the real lease is held"
            );
            std::fs::write(directory.join("cancel-proceed"), b"lease observed at one\n")?;
        }
        let live_cancel = if scenario == "cancel" {
            Some(observe_live_cancel(&fixture, &peer, &directory).await?)
        } else {
            None
        };
        let result = tokio::time::timeout(Duration::from_secs(90), child).await???;
        std::fs::write(directory.join("driver.stdout"), &result.stdout)?;
        std::fs::write(directory.join("driver.stderr"), &result.stderr)?;
        let (released_before_stop, eventually_released_before_stop) =
            if let Some(observed) = live_cancel {
                observed
            } else {
                let released = wait_until(|| active_leases(&fixture) == 0).await;
                (released, released)
            };
        if scenario == "standard" {
            assert_presend_rejections(&fixture, address, &ingress, &peer, client).await?;
        }
        handle.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        let received = peer.received.lock().map_err(|_| "receiver")?.clone();
        std::fs::write(
            directory.join("upstream.json"),
            serde_json::to_vec_pretty(&received)?,
        )?;
        let captured: Vec<_> = ingress.lock().map_err(|_|"ingress")?.iter().map(|v|json!({"path":v.path,"probe":v.probe,"body":v.request,"headers":v.headers,"status":v.status,"chunks":v.chunks,"wire":String::from_utf8_lossy(&v.wire)})).collect();
        std::fs::write(
            directory.join("ingress.json"),
            serde_json::to_vec_pretty(&captured)?,
        )?;
        let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
        std::fs::write(directory.join("events.txt"), format!("{rows:#?}"))?;
        assert!(
            result.status.success(),
            "driver: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(active_leases(&fixture), 0);
        if scenario == "cancel" {
            assert!(
                wait_until(|| peer.closed.load(Ordering::SeqCst) == 1).await,
                "actual Pi abort closes the upstream socket"
            );
        }
        if client == "pi" {
            assert_eq!(
                received.len(),
                if scenario == "standard" { 5 } else { 1 },
                "actual Pi receives, including tool cycles and follow-up"
            );
        } else {
            assert!(
                received.is_empty(),
                "Codex's unsupported controls must fail before send"
            );
        }
        let accounting = assert_accounting(&database, client, scenario).await?;
        let public = assert_public(&ingress, client, scenario)?;
        if client == "pi" && scenario != "standard" {
            let receipt: Value =
                serde_json::from_slice(&std::fs::read(directory.join("client.json"))?)?;
            assert_eq!(
                receipt["turn_outcomes"],
                json!([if scenario == "failure" {
                    "error"
                } else {
                    "aborted"
                }])
            );
        }
        if client == "pi" && scenario == "standard" {
            assert_history(&received, &directory)?;
        }
        std::fs::write(
            directory.join("observations.json"),
            serde_json::to_vec_pretty(&json!({
                "client":client,"scenario":scenario,"received":received.len(),"released_within_one_second":released_before_stop,
                "released_before_server_stop":eventually_released_before_stop,
                "accounting":accounting,"public":public,"success_metadata_retention":"BLOCKED_BY_61"
            }))?,
        )?;
        assert!(
            eventually_released_before_stop,
            "bounded actual-client release must be observed before server cleanup"
        );
        // Preserve the actual-client RED separately from eventual keepalive-driven release.
        // This is a one-second diagnostic probe, not a newly approved product latency SLA.
        if std::env::var_os("CPAR62_REQUIRE_PROMPT_RELEASE").is_some() {
            assert!(
                released_before_stop,
                "one-second actual-client release probe failed; inspect observations and cancel-diagnostic.json; do not relabel eventual release as prompt cancellation"
            );
        }
    }
    Ok(())
}

async fn observe_live_cancel(
    fixture: &Fixture,
    peer: &SocketPeer,
    directory: &std::path::Path,
) -> Result<(bool, bool), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !directory.join("abort-complete").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let rpc: Value = serde_json::from_slice(&std::fs::read(directory.join("abort-complete"))?)?;
    assert_eq!(
        rpc["rpc_alive"], true,
        "CLI must stay alive after abort while release is measured"
    );
    let started = std::time::Instant::now();
    let immediate =
        wait_until(|| active_leases(fixture) == 0 && peer.closed.load(Ordering::SeqCst) == 1).await;
    let leases_after_first_second = active_leases(fixture);
    let eventual = tokio::time::timeout(Duration::from_secs(20), async {
        while active_leases(fixture) > 0 || peer.closed.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok();
    std::fs::write(
        directory.join("cancel-diagnostic.json"),
        serde_json::to_vec_pretty(&json!({
            "cli_kept_alive":true,"released_within_first_second":immediate,"leases_after_first_second":leases_after_first_second,
            "eventually_released_before_server_stop":eventual,"total_observation_ms":started.elapsed().as_millis(),
            "leases_before_cli_exit":active_leases(fixture),"peer_close_before_cli_exit":peer.closed.load(Ordering::SeqCst)
        }))?,
    )?;
    std::fs::write(
        directory.join("cancel-observed"),
        b"live CLI observation finished; process cleanup may begin\n",
    )?;
    Ok((immediate, eventual))
}
