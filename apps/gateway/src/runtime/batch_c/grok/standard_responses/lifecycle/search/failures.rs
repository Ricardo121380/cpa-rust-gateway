//! C5 unknown metering, premature transport end, and cancellation stay fail-closed.
use super::*;

#[actix_web::test]
async fn public_build_search_unknown_usage_retains_exact_failed_evidence_and_never_completes()
-> TestResult {
    for streaming in [false, true] {
        for raw in [
            r#"{ "input_tokens":20,"output_tokens":7,"total_tokens":27,"web_search":{"num_requests":1,"unreviewed":null} }"#,
            r#"{ "input_tokens":20,"output_tokens":7,"total_tokens":27,"cost_in_usd_ticks":1 }"#,
        ] {
            let mut items = output(0);
            items.as_array_mut().ok_or("items")?.push(json!({"id":"flat-with-search","type":"function_call","status":"completed","name":"lookup","call_id":"flat-with-search-call","arguments":"{}"}));
            let original = wire_output(&items, streaming)?;
            let wire = original.replace(
                r#""usage":{"input_tokens":20,"output_tokens":7,"total_tokens":27}"#,
                &format!(r#""usage":{raw}"#),
            );
            assert_ne!(
                wire, original,
                "retain the actual unknown object in the fixture"
            );
            let peer = ModelPeer::start_wire(streaming, wire)?;
            let fixture =
                lifecycle_fixture_qualifications(namespaces::transport(&peer, 7)?, false, true)?;
            let recording = Recording::start(&fixture, false)?;
            let (url, server) = namespaces::listener(&fixture)?;
            let mut request_body = body(streaming);
            request_body["tools"] = json!([declarations()[0],{"type":"function","name":"lookup","parameters":{"type":"object"}}]);
            let (status, bytes) = namespaces::request(&fixture, &url, &request_body).await?;
            if streaming {
                assert_eq!(status, 200);
                let frames = public_frames(&bytes)?;
                assert_eq!(
                    frames
                        .iter()
                        .filter(|v| v["type"] == "response.failed")
                        .count(),
                    1
                );
                println!("CPAR62_SEARCH_MIXED_USAGE_FRAMES {}", json!(frames));
                assert!(frames.iter().all(|v| v["type"] != "response.completed"
                    && v["type"] != "response.function_call_arguments.done"
                    && !(v["type"] == "response.output_item.done"
                        && v["item"]["type"] == "web_search_call")));
            } else {
                assert_eq!(status, 502, "{}", String::from_utf8_lossy(&bytes));
            }
            assert!(!String::from_utf8_lossy(&bytes).contains("unreviewed"));
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
            assert_eq!(active_leases(&fixture), 0);
            server.stop(false).await;
            let database = recording.finish(&fixture, 0).await?;
            let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
            let request = rows
                .iter()
                .find_map(|v| {
                    if let GatewayEvent::Request(r) = v.event() {
                        Some(r)
                    } else {
                        None
                    }
                })
                .ok_or("request")?;
            let usages: Vec<_> = rows
                .iter()
                .filter_map(|v| {
                    if let GatewayEvent::Usage(u) = v.event() {
                        Some(u)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(usages.len(), 1);
            let usage = usages[0];
            let attempt = usage.attempt_id().ok_or("exact attempt")?;
            let record = SqliteNativeUsageReader::open(&database, test_secret_store()?)?
                .get_owned(
                    request.client_key_id(),
                    request.request_id(),
                    attempt,
                    system_now_ms_runtime()?,
                )?
                .ok_or("encrypted usage")?;
            assert_eq!(record.evidence.raw_json(), raw);
            assert_eq!(record.completeness, "complete_usage_object");
            namespaces::assert_recorded(&database, 1).await?;
            println!(
                "CPAR62_SEARCH_USAGE_BLOCKED {}",
                json!({"streaming":streaming,"raw_usage":raw,"receiver_count":1,"attempt_count":1,
                "usage_count":1,"ledger_count":1,"encrypted_readback":"EXACT","activation":"BLOCKED","cost":"UNKNOWN","cost_in_usd_ticks":"NOT_ADMITTED"})
            );
        }
    }
    Ok(())
}

fn partial() -> String {
    frame("response.created", &created())
        + &native_event(
            "response.output_item.added",
            json!({"output_index":0,"item":{"id":"ws-pending","type":"web_search_call","status":"in_progress"}}),
        )
        + &native_event(
            "response.web_search_call.in_progress",
            json!({"output_index":0,"item_id":"ws-pending","sequence_number":1}),
        )
        + &native_event(
            "response.web_search_call.searching",
            json!({"output_index":0,"item_id":"ws-pending","sequence_number":2}),
        )
        + &native_event(
            "response.output_item.added",
            json!({"output_index":1,"item":{"id":"answer-pending","type":"message","status":"in_progress","role":"assistant","content":[]}}),
        )
        + &native_event(
            "response.output_text.delta",
            json!({"output_index":1,"item_id":"answer-pending","content_index":0,"delta":"search-in-progress"}),
        )
}

#[actix_web::test]
async fn public_build_search_truncation_and_eof_fail_once_without_retry() -> TestResult {
    for end in [PeerEnd::Truncate, PeerEnd::Eof] {
        let peer = SocketPeer::start(partial(), end)?;
        let fixture = lifecycle_fixture_qualifications(
            peer.transport(Duration::from_secs(5), Duration::from_secs(10))?,
            false,
            true,
        )?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let (status, bytes) = namespaces::request(&fixture, &url, &body(true)).await?;
        assert_eq!(status, 200);
        let frames = public_frames(&bytes)?;
        assert_eq!(
            frames
                .iter()
                .filter(|v| v["type"] == "response.failed")
                .count(),
            1
        );
        assert!(
            frames
                .iter()
                .all(|v| v["type"] != "response.completed"
                    && v["type"] != "response.output_item.done")
        );
        assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
        assert_eq!(fixture.events.attempts()?, 1);
        assert_eq!(active_leases(&fixture), 0);
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
        assert!(
            rows.iter()
                .all(|v| !matches!(v.event(), GatewayEvent::Usage(_)))
        );
        println!(
            "CPAR62_SEARCH_TRANSPORT {}",
            json!({"end":format!("{end:?}"),"receiver_count":1,"attempt_count":1,"usage":"UNKNOWN","cost":"UNKNOWN","lease_after":0})
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_search_fin_and_reset_cancel_release_and_reuse_capacity() -> TestResult {
    for reset in [false, true] {
        let peer = SocketPeer::start(partial(), PeerEnd::Hold)?;
        let fixture = lifecycle_fixture_qualifications(
            peer.transport(Duration::from_secs(5), Duration::from_secs(10))?,
            false,
            true,
        )?;
        let recording = Recording::start(&fixture, false)?;
        let (mut socket, server) =
            public_socket_request(&fixture, "/v1/responses", body(true)).await?;
        let wire = read_public_until(&mut socket, "search-in-progress").await?;
        assert!(
            wire.contains("response.web_search_call.searching")
                && !wire.contains("response.completed")
        );
        assert_eq!(active_leases(&fixture), 1);
        let socket = if reset {
            socket.set_zero_linger()?;
            drop(socket);
            None
        } else {
            socket.shutdown().await?;
            Some(socket)
        };
        assert!(
            wait_until(|| peer.closed.load(Ordering::SeqCst) == 1 && active_leases(&fixture) == 0)
                .await
        );
        assert!(wait_until(||SqliteEventStore::open_read_only(&recording.database).and_then(|v|v.list_events())
            .is_ok_and(|rows|rows.iter().any(|v|matches!(v.event(),GatewayEvent::RequestFinished(f) if f.outcome==gateway_core::RequestOutcome::Cancelled)))).await);
        let rows = SqliteEventStore::open_read_only(&recording.database)?.list_events()?;
        assert!(
            rows.iter()
                .all(|v| !matches!(v.event(), GatewayEvent::Usage(_)))
        );
        assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 1);
        assert_eq!(fixture.events.attempts()?, 1);
        drop(socket);
        let (url, second_server) = namespaces::listener(&fixture)?;
        let (status, bytes) = namespaces::request(&fixture, &url, &body(true)).await?;
        assert_eq!(status, 200);
        let _reply = namespaces::terminal(&bytes, true)?;
        assert_eq!(peer.received.lock().map_err(|_| "receiver")?.len(), 2);
        assert_eq!(fixture.events.attempts()?, 2);
        assert_eq!(active_leases(&fixture), 0);
        second_server.stop(false).await;
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
        assert_eq!(
            rows.iter()
                .filter(|v| matches!(v.event(), GatewayEvent::Usage(_)))
                .count(),
            1
        );
        println!(
            "CPAR62_SEARCH_CANCEL {}",
            json!({"reset":reset,"incremental_observed":true,"lease_before":1,"upstream_closed_before_cleanup":1,
            "lease_after":0,"cancelled_receiver_count":1,"cancelled_attempt_count":1,"cancelled_usage":"UNKNOWN","cancelled_cost":"UNKNOWN","explicit_reuse_receive_count":2})
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_search_root_statistics_remain_blocked_in_json_and_sse() -> TestResult {
    for streaming in [false, true] {
        let mut terminal = response();
        terminal["output"] = output(0);
        terminal["tool_usage"] =
            json!({"web_search":{"num_requests":1},"unreviewed_search_stat":null});
        let wire = if streaming {
            let complete = wire_output(&output(0), true)?;
            let prefix = complete
                .split("event: response.completed\n")
                .next()
                .ok_or("prefix")?;
            prefix.to_owned() + &frame("response.completed", &terminal)
        } else {
            terminal.to_string()
        };
        println!(
            "CPAR62_SEARCH_ROOT_STAT_SAMPLE {}",
            json!({"streaming":streaming,"sample":terminal,"qualification":"BLOCKED"})
        );
        super::negatives::assert_failure(
            streaming,
            "unreviewed-root-search-stat",
            wire,
            body(streaming),
        )
        .await?;
    }
    Ok(())
}
