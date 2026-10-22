//! Opt-in actual CLI qualification through the production public HTTP and leased Build chain.
use super::*;
use actix_web::middleware::from_fn;
mod evidence;
mod http;
mod oracles;
mod peer;
mod tool_loop;
use http::assert_presend_rejections;
pub(super) use http::capture;
pub(super) use oracles::assert_accounting;
use oracles::{assert_history, assert_public};
use peer::scripted_peer;

#[derive(Default)]
pub(super) struct Captured {
    pub(super) path: String,
    pub(super) method: String,
    pub(super) uri: String,
    pub(super) request_bytes: Vec<u8>,
    pub(super) probe: bool,
    pub(super) request: Value,
    pub(super) headers: Value,
    pub(super) response_headers: Value,
    pub(super) status: u16,
    pub(super) chunks: usize,
    pub(super) chunk_elapsed_ms: Vec<u128>,
    started: Option<std::time::Instant>,
    pub(super) wire: Vec<u8>,
    pub(super) wire_truncated: bool,
    pub(super) body_complete: bool,
}
pub(super) type Ingress = Mutex<Vec<Captured>>;

#[actix_web::test]
#[ignore = "requires fixed local Codex/Pi installs and the macOS loopback-only runner"]
async fn actual_cli_public_build_qualification() -> TestResult {
    let run = std::path::PathBuf::from(std::env::var("CPAR62_RUN")?);
    assert!(run.is_absolute() && run.is_dir());
    for (client, scenario) in [
        ("pi", "standard"),
        ("pi", "failure"),
        ("pi", "cancel"),
        ("codex", "unqualified"),
        ("codex", "standard"),
        ("codex", "failure"),
        ("codex", "cancel"),
    ] {
        if std::env::var("CPAR62_CASE").is_ok_and(|case| case != format!("{client}-{scenario}")) {
            continue;
        }
        let peer = match scenario {
            "failure" => {
                let mut failed = response();
                failed["status"] = json!("failed");
                failed["error"] = json!({"code":"server_error","message":"synthetic #62 failure"});
                SocketPeer::start(
                    partial_wire() + &frame("response.failed", &failed),
                    PeerEnd::Failure,
                )?
            }
            "cancel" => SocketPeer::start(partial_wire(), PeerEnd::Hold)?,
            _ => scripted_peer(client)?,
        };
        let transport = peer.transport(
            Duration::from_secs(if scenario == "cancel" { 30 } else { 5 }),
            Duration::from_secs(if scenario == "cancel" { 60 } else { 15 }),
        )?;
        let fixture = if client == "codex" && scenario != "unqualified" {
            parallel::fixture_for(transport, false, true, |p| {
                p.build_parallel_local_controlled = true;
                p.build_namespaces_local_controlled = true;
                p.build_search_local_controlled = true;
                p.build_input_message_ids_local_controlled = true;
                p.build_input_history_local_controlled = true;
            })?
        } else {
            lifecycle_fixture(transport)?
        };
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
        let mut cancel_ready = true;
        if scenario == "cancel" {
            cancel_ready = tokio::time::timeout(Duration::from_secs(10), async {
                while !directory.join("cancel-ready").exists() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .is_ok();
        }
        if scenario == "cancel" && cancel_ready {
            assert_eq!(
                active_leases(&fixture),
                1,
                "actual CLI has received text while the real lease is held"
            );
            std::fs::write(
                directory.join("cancel-proceed"),
                serde_json::to_vec(&json!({
                    "lease":1,"observed_at_ms":system_now_ms_runtime()?
                }))?,
            )?;
        }
        let live_cancel = if scenario == "cancel" && cancel_ready {
            Some(observe_live_cancel(&fixture, &peer, &directory).await)
        } else {
            None
        };
        let result = tokio::time::timeout(Duration::from_secs(90), child).await???;
        std::fs::write(directory.join("driver.stdout"), &result.stdout)?;
        std::fs::write(directory.join("driver.stderr"), &result.stderr)?;
        let (released_before_stop, eventually_released_before_stop) =
            if let Some(observed) = live_cancel {
                match observed {
                    Ok(value) => value,
                    Err(error) => {
                        std::fs::write(
                            directory.join("cancel-observation-error.txt"),
                            error.to_string(),
                        )?;
                        (false, false)
                    }
                }
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
        std::fs::write(
            directory.join("receiver-receipts.json"),
            serde_json::to_vec_pretty(&*peer.receipts.lock().map_err(|_| "receiver receipts")?)?,
        )?;
        let captured: Vec<_> = ingress.lock().map_err(|_|"ingress")?.iter().enumerate().map(|(index,v)| {
            std::fs::write(directory.join(format!("request-{index}.bin")), &v.request_bytes)?;
            std::fs::write(directory.join(format!("response-{index}.bin")), &v.wire)?;
            Ok(json!({"path":v.path,"probe":v.probe,"body":v.request,"headers":v.headers,"status":v.status,"chunks":v.chunks,"chunk_elapsed_ms":v.chunk_elapsed_ms,"wire":String::from_utf8_lossy(&v.wire),"response_headers":v.response_headers,"wire_truncated":v.wire_truncated,"body_complete":v.body_complete}))
        }).collect::<Result<_,std::io::Error>>()?;
        std::fs::write(
            directory.join("ingress.json"),
            serde_json::to_vec_pretty(&captured)?,
        )?;
        let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
        std::fs::write(directory.join("events.txt"), format!("{rows:#?}"))?;
        std::fs::write(
            directory.join("events.json"),
            serde_json::to_vec_pretty(
                &rows
                    .iter()
                    .map(|row| json!({"event_id":row.event_id(),"event":row.event()}))
                    .collect::<Vec<_>>(),
            )?,
        )?;
        oracles::materialize_ledger(&database).await?;
        let correspondence = evidence::correspondence(&database, &ingress, &peer)?;
        std::fs::write(
            directory.join("correspondence.json"),
            serde_json::to_vec_pretty(&correspondence)?,
        )?;
        assert!(
            cancel_ready,
            "actual CLI did not reach the cancellation delta/lease barrier; raw receipts retained"
        );
        assert!(
            result.status.success(),
            "driver: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(active_leases(&fixture), 0);
        if scenario == "cancel" {
            let alive: Value =
                serde_json::from_slice(&std::fs::read(directory.join("cancel-client-alive"))?)?;
            assert_eq!(
                alive["rpc_alive"], true,
                "same CLI must still be alive after the release observation"
            );
            let diagnostic_path = directory.join("cancel-diagnostic.json");
            let mut diagnostic: Value = serde_json::from_slice(&std::fs::read(&diagnostic_path)?)?;
            assert_eq!(alive["pid"], diagnostic["cli_pid"]);
            diagnostic["cli_kept_alive"] = alive["rpc_alive"].clone();
            diagnostic["alive_after_observation"] = alive;
            std::fs::write(diagnostic_path, serde_json::to_vec_pretty(&diagnostic)?)?;
            assert!(
                wait_until(|| peer.closed.load(Ordering::SeqCst) == 1).await,
                "actual CLI abort closes the upstream socket"
            );
        }
        if scenario == "unqualified" {
            assert!(
                received.is_empty(),
                "Codex's unsupported controls must fail before send"
            );
        } else {
            assert_eq!(
                received.len(),
                if scenario == "standard" { 5 } else { 1 },
                "actual CLI receives, including tool cycles and follow-up"
            );
        }
        let accounting = assert_accounting(&database, client, scenario)?;
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
        if client == "codex" && matches!(scenario, "failure" | "cancel") {
            oracles::assert_codex_terminal(&directory, scenario)?;
        }
        if client == "pi" && scenario == "standard" {
            assert_history(&received, &directory)?;
        }
        let tool_loop = if client == "codex" && scenario == "standard" {
            tool_loop::assert_codex(&received, &directory)?
        } else {
            Value::Null
        };
        std::fs::write(
            directory.join("observations.json"),
            serde_json::to_vec_pretty(&json!({
                "client":client,"scenario":scenario,"received":received.len(),"released_within_one_second":released_before_stop,
                "released_before_server_stop":eventually_released_before_stop,
                "accounting":accounting,"public":public,"correspondence":correspondence,"tool_loop":tool_loop,"success_metadata_retention":"BLOCKED_BY_61"
            }))?,
        )?;
        assert!(
            eventually_released_before_stop,
            "bounded actual-client release must be observed before server cleanup"
        );
        // Preserve the actual-client RED separately from eventual release.
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
    let released_at_ms = system_now_ms_runtime()?;
    let lease_before_abort: Value =
        serde_json::from_slice(&std::fs::read(directory.join("cancel-proceed"))?)?;
    std::fs::write(
        directory.join("cancel-diagnostic.json"),
        serde_json::to_vec_pretty(&json!({
            "cli_alive_at_abort":rpc["rpc_alive"],"cli_pid":rpc["pid"],
            "rpc_timeline":rpc,"lease_before_abort":lease_before_abort,
            "observation_end_at_ms":released_at_ms,
            "release_observed_at_ms":eventual.then_some(released_at_ms),
            "abort_to_release_upper_bound_ms":rpc["abort_sent_at_ms"].as_i64().filter(|_| eventual).map(|sent| released_at_ms.saturating_sub(sent)),
            "released_within_first_second":immediate,"leases_after_first_second":leases_after_first_second,
            "eventually_released_before_server_stop":eventual,"total_observation_ms":started.elapsed().as_millis(),
            "leases_at_observation_end":active_leases(fixture),"peer_close_at_observation_end":peer.closed.load(Ordering::SeqCst)
        }))?,
    )?;
    std::fs::write(
        directory.join("cancel-observed"),
        b"live CLI observation finished; process cleanup may begin\n",
    )?;
    Ok((immediate, eventual))
}
