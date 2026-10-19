//! #61 public production Build boundaries. All wire values and accounts are synthetic.
use super::{model_identity::ModelPeer, *};
mod codex_controls;
mod lifecycle;
mod tool_rounds;
use gateway_store::{
    billing_ledger::SqliteBillingLedger,
    event_store::{AsyncSqliteEventWriter, EventWriterConfig, SqliteEventStore},
    native_usage::SqliteNativeUsageReader,
};

fn response() -> Value {
    json!({"id":"response-61","model":"grok-4.5","object":"response",
        "status":"completed","output":[],
        "usage":{"input_tokens":20,"output_tokens":7,"total_tokens":27}})
}

fn created() -> Value {
    json!({"id":"response-61","model":"grok-4.5","status":"in_progress","output":[]})
}

fn frame(kind: &str, response: &Value) -> String {
    format!(
        "event: {kind}\ndata: {}\n\n",
        json!({"type":kind,"response":response})
    )
}

struct CoalescedFailure {
    peer: Arc<ModelPeer>,
}

struct CoalescedFailureBody {
    inner: Box<dyn GrokBuildResponseBody>,
    first: bool,
}

impl GrokBuildResponseBody for CoalescedFailureBody {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            let chunk = self.inner.next_chunk().await?;
            if std::mem::take(&mut self.first) {
                return Ok(chunk);
            }
            let Some(mut chunk) = chunk else {
                return Ok(None);
            };
            if let Some(tail) = self.inner.next_chunk().await? {
                chunk.extend(tail);
            }
            Ok(Some(chunk))
        })
    }
}

impl GrokBuildTransport for CoalescedFailure {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            let (status, kind, encoding, body) = self.peer.send(request).await?.into_parts();
            Ok(GrokBuildTransportResponse::new(
                status,
                kind,
                encoding,
                Box::new(CoalescedFailureBody {
                    inner: body,
                    first: true,
                }),
            ))
        })
    }
}

#[actix_web::test]
async fn public_build_rejects_unreviewed_root_fields_and_shapes() -> TestResult {
    for streaming in [false, true] {
        for (field, value) in [
            (
                "unreviewed_root_61",
                json!({"private":"SYNTHETIC_PRIVATE_61"}),
            ),
            ("metadata", json!({"unreviewed":0})),
            ("object", json!(42)),
        ] {
            let mut terminal = response();
            terminal[field] = value;
            let bytes = if streaming {
                frame("response.created", &created()) + &frame("response.completed", &terminal)
            } else {
                terminal.to_string()
            };
            let peer = Arc::new(ModelPeer::start_wire(streaming, bytes)?);
            let (channel, secret) = native_channels()[0];
            let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
                let mut profiles = P12TransportProfiles::try_new()?;
                profiles.build = Some(peer.clone());
                endpoint.transports = Arc::new(profiles);
                Ok(())
            })?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let reply = test::call_service(&app, test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization",format!("Bearer {}",fixture.key)))
                .set_json(json!({"model":"p12-test-model","input":"synthetic #61","stream":streaming}))
                .to_request()).await;
            let status = reply.status();
            let bytes = test::read_body(reply).await;
            let wire = std::str::from_utf8(&bytes)?;
            assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
            assert!(!wire.contains("SYNTHETIC_PRIVATE_61"));
            if streaming {
                assert_eq!(status, StatusCode::OK);
                let frames = public_frames(&bytes)?;
                assert_eq!(
                    frames
                        .iter()
                        .filter(|v| v["type"] == "response.failed")
                        .count(),
                    1,
                    "{field}: {wire}"
                );
                assert!(!wire.contains("response.completed"), "{field}: {wire}");
                assert_eq!(peer.reads_after_body.load(Ordering::SeqCst), 0);
            } else {
                assert_eq!(status, StatusCode::BAD_GATEWAY, "{field}: {wire}");
                assert!(
                    serde_json::from_slice::<Value>(&bytes)?
                        .get("error")
                        .is_some()
                );
            }
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_rejects_invalid_added_item_status() -> TestResult {
    let item = json!({"id":"message-61","type":"message","status":42,
        "role":"assistant","content":[]});
    let added = format!(
        "event: response.output_item.added\ndata: {}\n\n",
        json!({"type":"response.output_item.added","output_index":0,"item":item})
    );
    let mut done = item;
    done["status"] = json!("completed");
    let finished = format!(
        "event: response.output_item.done\ndata: {}\n\n",
        json!({"type":"response.output_item.done","output_index":0,"item":done})
    );
    let mut terminal = response();
    terminal["output"] = json!([done]);
    let peer = Arc::new(ModelPeer::start_wire(
        true,
        frame("response.created", &created())
            + &added
            + &finished
            + &frame("response.completed", &terminal),
    )?);
    let (channel, secret) = native_channels()[0];
    let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
        let mut profiles = P12TransportProfiles::try_new()?;
        profiles.build = Some(peer.clone());
        endpoint.transports = Arc::new(profiles);
        Ok(())
    })?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let reply = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(json!({"model":"p12-test-model","input":"synthetic #61","stream":true}))
            .to_request(),
    )
    .await;
    assert_eq!(reply.status(), StatusCode::OK);
    let bytes = test::read_body(reply).await;
    let frames = public_frames(&bytes)?;
    assert_eq!(
        frames
            .iter()
            .filter(|v| v["type"] == "response.failed")
            .count(),
        1,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(frames.iter().all(|v| v["type"] != "response.completed"));
    assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
    assert_eq!(
        peer.delivered.load(Ordering::SeqCst),
        2,
        "stop at invalid added item"
    );
    Ok(())
}

#[actix_web::test]
async fn public_build_failed_sse_retains_received_usage_on_exact_attempt() -> TestResult {
    for (raw, coalesced) in [
        (
            r#"{"input_tokens":20,"output_tokens":7,"total_tokens":27}"#,
            true,
        ),
        (usage_evidence::RAW_USAGE, true),
        (
            r#"{"input_tokens":20,"output_tokens":7,"total_tokens":27}"#,
            false,
        ),
        (usage_evidence::RAW_USAGE, false),
    ] {
        let terminal = format!(
            r#"{{"id":"response-61","model":"grok-4.5","status":"failed","output":[],"usage":{raw},"error":{{"code":"server_error","message":"SYNTHETIC_PRIVATE_61"}}}}"#
        );
        let wire = frame("response.created", &created())
            + &format!(
                "event: response.failed\ndata: {{\"type\":\"response.failed\",\"response\":{terminal}}}\n\n"
            )
            + &frame("response.completed", &response());
        let peer = Arc::new(ModelPeer::start_wire(true, wire)?);
        let (channel, secret) = native_channels()[0];
        let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.build = Some(if coalesced {
                Arc::new(CoalescedFailure { peer: peer.clone() })
            } else {
                peer.clone()
            });
            endpoint.transports = Arc::new(profiles);
            Ok(())
        })?;
        let database = fixture.directory.path().join("batch-c.sqlite3");
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(64, 1)?)?;
        *fixture.events.1.lock().map_err(|_| "sink")? = Some(Arc::new(queue));
        let writer = tokio::spawn(
            AsyncSqliteEventWriter::new(
                &database,
                receiver,
                EventWriterConfig::try_new(4, Duration::from_millis(5))?,
            )
            .with_native_usage_secret_store(test_secret_store()?)
            .run(),
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let reply = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(json!({"model":"p12-test-model","input":"synthetic #61","stream":true}))
                .to_request(),
        )
        .await;
        assert_eq!(reply.status(), StatusCode::OK);
        let request_id = reply
            .headers()
            .get("x-request-id")
            .ok_or("request id")?
            .to_str()?
            .to_owned();
        let bytes = test::read_body(reply).await;
        let wire = std::str::from_utf8(&bytes)?;
        let frames = public_frames(&bytes)?;
        assert_eq!(
            frames
                .iter()
                .filter(|v| v["type"] == "response.failed")
                .count(),
            1,
            "{wire}"
        );
        assert!(!wire.contains("response.completed") && !wire.contains("SYNTHETIC_PRIVATE_61"));
        assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
        assert_eq!(
            peer.delivered.load(Ordering::SeqCst),
            if coalesced { 3 } else { 2 },
            "the coalesced tail is already received; a separate tail stays unread"
        );
        assert_eq!(peer.reads_after_body.load(Ordering::SeqCst), 0);
        let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
        let usages: Vec<_> = rows
            .iter()
            .filter_map(|row| match row.event() {
                GatewayEvent::Usage(usage) => Some(usage),
                _ => None,
            })
            .collect();
        assert_eq!(
            usages.len(),
            1,
            "a failed terminal must retain already received valid counters"
        );
        assert_eq!(usages[0].request_id().as_str(), request_id);
        assert_eq!(usages[0].usage().input_tokens, Some(20));
        assert_eq!(usages[0].usage().output_tokens, Some(7));
        let admitted = rows
            .iter()
            .find_map(|row| match row.event() {
                GatewayEvent::Request(request) => Some(request),
                _ => None,
            })
            .ok_or("admitted request")?;
        let attempt = usages[0].attempt_id().ok_or("exact Attempt")?;
        let evidence = SqliteNativeUsageReader::open(&database, test_secret_store()?)?.get_owned(
            admitted.client_key_id(),
            admitted.request_id(),
            attempt,
            system_now_ms_runtime()?,
        )?;
        if raw == usage_evidence::RAW_USAGE {
            let evidence = evidence.ok_or("confirmed encrypted failed evidence")?;
            assert_eq!(evidence.evidence.raw_json(), raw);
            assert_eq!(evidence.completeness, "complete_usage_object");
        } else {
            assert!(evidence.is_none());
        }
        drop(app);
        fixture.events.1.lock().map_err(|_| "sink")?.take();
        let metrics = tokio::time::timeout(Duration::from_secs(5), writer).await??;
        assert_eq!(metrics.required_events_quarantined, 0);
        let worker = crate::billing_worker::BillingWorker::start(
            database.clone(),
            Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
        );
        worker.stop().await.map_err(|()| "billing worker")?;
        let ledger = SqliteBillingLedger::open_read_only(&database)?.list_bounded(10)?;
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].usage, *usages[0].usage());
        assert_eq!(ledger[0].cost_microunits, None);
        let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
        assert_eq!(rows.iter().filter(|row|matches!(row.event(),GatewayEvent::RequestFinished(v)
            if v.request_id.as_str()==request_id && v.outcome==gateway_core::RequestOutcome::Failed)).count(),1);
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_rejects_conflicting_sse_snapshot_status() -> TestResult {
    for phase in ["response.created", "response.in_progress"] {
        let mut snapshot = created();
        snapshot["status"] = json!("completed");
        let wire = if phase == "response.created" {
            String::new()
        } else {
            frame("response.created", &created())
        } + &frame(phase, &snapshot)
            + &frame("response.completed", &response());
        let peer = Arc::new(ModelPeer::start_wire(true, wire)?);
        let (channel, secret) = native_channels()[0];
        let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.build = Some(peer.clone());
            endpoint.transports = Arc::new(profiles);
            Ok(())
        })?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let reply = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(json!({"model":"p12-test-model","input":"synthetic #61","stream":true}))
                .to_request(),
        )
        .await;
        let status = reply.status();
        let bytes = test::read_body(reply).await;
        let wire = std::str::from_utf8(&bytes)?;
        assert!(!wire.contains("response.completed"), "{phase}: {wire}");
        if phase == "response.created" {
            assert_eq!(status, StatusCode::BAD_GATEWAY);
        } else {
            assert_eq!(
                public_frames(&bytes)?
                    .iter()
                    .filter(|v| v["type"] == "response.failed")
                    .count(),
                1
            );
        }
        assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_rejects_known_request_controls_before_any_send() -> TestResult {
    for streaming in [false, true] {
        for (field, value) in [
            ("background", json!(true)),
            ("conversation", json!("synthetic-conversation")),
            ("stream_options", json!({"include_obfuscation":true})),
        ] {
            let peer = Arc::new(ModelPeer::start(streaming, response())?);
            let (channel, secret) = native_channels()[0];
            let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
                let mut profiles = P12TransportProfiles::try_new()?;
                profiles.build = Some(peer.clone());
                endpoint.transports = Arc::new(profiles);
                Ok(())
            })?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(fixture.state.clone()))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let mut body =
                json!({"model":"p12-test-model","input":"synthetic #61","stream":streaming});
            body[field] = value;
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert_eq!(reply.status(), StatusCode::BAD_REQUEST, "{field}");
            let bytes = test::read_body(reply).await;
            assert!(
                serde_json::from_slice::<Value>(&bytes)?
                    .get("error")
                    .is_some()
            );
            assert_eq!(
                peer.requests.lock().map_err(|_| "receiver")?.len(),
                0,
                "{field}"
            );
            assert_eq!(fixture.events.attempts()?, 0);
            assert!(
                fixture
                    .events
                    .0
                    .lock()
                    .map_err(|_| "events")?
                    .iter()
                    .all(|v| !matches!(v, GatewayEvent::Usage(_)))
            );
        }
    }
    Ok(())
}
