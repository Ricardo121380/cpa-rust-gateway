//! Saved native bytes are processed only by the explicitly selected, ignored local probe.
//! Ordinary regressions use a synthetic fixture and the real public runtime/handlers.
use super::*;
use gateway_core::{InputTokenAccounting, RequestOutcome, UsageProvenance, UsageSummary};
use gateway_store::{
    billing_ledger::SqliteBillingLedger,
    event_store::{AsyncSqliteEventWriter, EventWriterConfig, SqliteEventStore},
};
use provider_grok::{GrokBuildResponseContentEncoding, GrokBuildResponsesDecoder};
use std::path::PathBuf;

const SYNTHETIC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/reports/assets/cpar-native-chat-offline-20261017/minimal-native.json"
));

struct OfflinePeer {
    json: Vec<u8>,
    sse: Vec<u8>,
    encoding: GrokBuildResponseContentEncoding,
    received: AtomicUsize,
}

impl GrokBuildTransport for OfflinePeer {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            self.received.fetch_add(1, Ordering::Relaxed);
            let body: Value =
                serde_json::from_slice(request.body()).map_err(|_| internal_error())?;
            if body["model"] != "grok-4.5" || body["max_output_tokens"] != 4096 {
                return Err(internal_error());
            }
            let streaming = body["stream"] == true;
            Ok(GrokBuildTransportResponse::new(
                200,
                if streaming {
                    GrokBuildResponseContentType::EventStream
                } else {
                    GrokBuildResponseContentType::Json
                },
                if streaming {
                    GrokBuildResponseContentEncoding::Identity
                } else {
                    self.encoding
                },
                Box::new(Body(Some(if streaming {
                    self.sse.clone()
                } else {
                    self.json.clone()
                }))),
            ))
        })
    }
}

fn offline_fixture(peer: Arc<dyn GrokBuildTransport>) -> Result<Fixture, Box<dyn Error>> {
    let (mut channel, secret) = native_channels()[0];
    channel.model = "grok-4.5";
    fixture_with_configuration(
        channel,
        secret.as_bytes(),
        false,
        |configuration| {
            configuration.public_models[0].model_name = "grok-4.5".into();
            Ok(())
        },
        |endpoints| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.build = Some(peer);
            endpoints.values_mut().next().ok_or("endpoint")?.transports = Arc::new(profiles);
            Ok(())
        },
    )
}

fn synthetic_peer(native: &Value) -> Result<Arc<OfflinePeer>, Box<dyn Error>> {
    let start = json!({"type":"response.created","response":{"id":native["id"],"model":native["model"],"status":"in_progress","output":[]}});
    let mut sse = format!("event: response.created\ndata: {start}\n\n");
    for (index, item) in native["output"]
        .as_array()
        .ok_or("output")?
        .iter()
        .enumerate()
    {
        for kind in ["response.output_item.added", "response.output_item.done"] {
            let event = json!({"type":kind,"output_index":index,"item":item});
            write!(sse, "event: {kind}\ndata: {event}\n\n")?;
        }
    }
    let end = json!({"type":"response.completed","response":native});
    write!(sse, "event: response.completed\ndata: {end}\n\n")?;
    Ok(Arc::new(OfflinePeer {
        json: serde_json::to_vec(native)?,
        sse: sse.into_bytes(),
        encoding: GrokBuildResponseContentEncoding::Identity,
        received: AtomicUsize::new(0),
    }))
}

#[actix_web::test]
async fn grok_build_native_summary_and_usage_public_matrix() -> TestResult {
    for extra_usage in [false, true] {
        for protocol in ["chat", "messages", "responses"] {
            for streaming in [false, true] {
                let mut native: Value = serde_json::from_str(SYNTHETIC)?;
                // Independent synthetic model identity for the projection/Usage oracle. The
                // saved fixture and ignored real-capture oracle remain byte-for-byte unchanged.
                native["model"] = json!("grok-4.5");
                if extra_usage {
                    native["usage"]["cost_in_usd_ticks"] = json!(123_000_000);
                    native["usage"]["num_server_side_tools_used"] = json!(0);
                    native["usage"]["synthetic_cost_details"] =
                        json!({"input_tokens":11,"output_tokens":17});
                }
                let peer = synthetic_peer(&native)?;
                let fixture = offline_fixture(peer.clone())?;
                let database = fixture.directory.path().join("batch-c.sqlite3");
                let (queue, receiver) =
                    BoundedEventQueue::try_new(EventQueueConfig::try_new(16, 1)?)?;
                let sink: Arc<dyn GatewayEventSink> = Arc::new(P12FanoutEventSink::new(
                    Arc::new(P12AttemptStageStore::new()),
                    Arc::new(queue),
                ));
                *fixture.events.1.lock().map_err(|_| "sink")? =
                    Some(Arc::new(super::projection_usage::ReplayUsageSink(sink)));
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
                let (uri, mut body) = super::projection_usage::request(protocol, streaming);
                body["model"] = json!("grok-4.5");
                let response = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(uri)
                        .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                        .insert_header(("x-request-id", "untrusted-client-id"))
                        .set_json(body)
                        .to_request(),
                )
                .await;
                let success = matches!(protocol, "responses" | "messages") && !extra_usage;
                let status = response.status();
                let id = response
                    .headers()
                    .get("x-request-id")
                    .ok_or("Request-ID")?
                    .to_str()?
                    .to_owned();
                assert_ne!(id, "untrusted-client-id");
                let wire = String::from_utf8(test::read_body(response).await.to_vec())?;
                assert_eq!(
                    status,
                    if success || streaming {
                        StatusCode::OK
                    } else {
                        StatusCode::BAD_GATEWAY
                    }
                );
                if success && protocol == "responses" {
                    let value: Value = if streaming {
                        wire.lines()
                            .filter_map(|line| line.strip_prefix("data: "))
                            .filter_map(|data| serde_json::from_str::<Value>(data).ok())
                            .find(|event| event["type"] == "response.completed")
                            .ok_or("completed")?["response"]
                            .clone()
                    } else {
                        serde_json::from_str(&wire)?
                    };
                    assert_eq!(value["output"], native["output"]);
                    assert_eq!(value["status"], "completed");
                    assert_eq!(value["usage"]["cpar_usage"]["reasoning_tokens"], 158);
                    assert_eq!(
                        value["usage"]["cpar_usage"]["cache_creation_tokens"],
                        Value::Null
                    );
                } else if success {
                    assert!(
                        !wire.contains("SYNTHETIC_SUMMARY")
                            && !wire.contains("thinking")
                            && !wire.contains("signature")
                    );
                    assert!(wire.contains("SYNTHETIC_ANSWER"));
                    let usage = if streaming {
                        let frames = public_frames(wire.as_bytes())?;
                        assert_eq!(
                            frames
                                .iter()
                                .filter(|v| v["type"] == "message_stop")
                                .count(),
                            1
                        );
                        frames
                            .iter()
                            .find(|v| v["type"] == "message_delta")
                            .ok_or("usage")?["usage"]
                            .clone()
                    } else {
                        serde_json::from_str::<Value>(&wire)?["usage"].clone()
                    };
                    assert_eq!(usage["input_tokens"], 100);
                    assert_eq!(usage["output_tokens"], 159);
                    assert_eq!(usage["cpar_usage"]["reasoning_tokens"], 158);
                } else {
                    let error: Value = wire
                        .lines()
                        .filter_map(|line| {
                            serde_json::from_str::<Value>(
                                line.strip_prefix("data: ").unwrap_or(line),
                            )
                            .ok()
                        })
                        .find(|value| {
                            value["error"].is_object() || value["response"]["error"].is_object()
                        })
                        .ok_or("safe error object")?;
                    let detail = if error["error"].is_object() {
                        &error["error"]
                    } else {
                        &error["response"]["error"]
                    };
                    assert!(
                        detail["message"]
                            .as_str()
                            .is_some_and(|message| message.contains("response projection"))
                    );
                    assert!(
                        !detail.to_string().contains("SYNTHETIC_")
                            && !detail.to_string().contains("synthetic-")
                    );
                    if protocol != "responses" {
                        assert!(!wire.contains("SYNTHETIC_SUMMARY"));
                    }
                    assert!(!wire.contains("[DONE]") && !wire.contains("message_stop"));
                    assert!(!wire.contains("\"type\":\"response.completed\""));
                    if !streaming {
                        let error: Value = serde_json::from_str(&wire)?;
                        if protocol == "messages" {
                            assert_eq!(error["type"], "error");
                            assert_eq!(error["error"]["type"], "overloaded_error");
                        } else {
                            assert_eq!(error["error"]["code"], "UpstreamProtocolError");
                        }
                    }
                }
                assert_eq!(peer.received.load(Ordering::Relaxed), 1);
                drop(app);
                fixture.events.1.lock().map_err(|_| "sink")?.take();
                let metrics = tokio::time::timeout(Duration::from_secs(5), writer).await??;
                assert_eq!(metrics.pending_required, 0);
                assert_eq!(metrics.required_events_quarantined, 0);
                let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
                let attempts: Vec<_> = rows
                    .iter()
                    .filter_map(|row| match row.event() {
                        GatewayEvent::Attempt(value) => Some(value),
                        _ => None,
                    })
                    .collect();
                let usage: Vec<_> = rows
                    .iter()
                    .filter_map(|row| match row.event() {
                        GatewayEvent::Usage(value) => Some((row.event_id(), value)),
                        _ => None,
                    })
                    .collect();
                assert_eq!(attempts.len(), 1);
                if success && protocol == "messages" {
                    let request = rows
                        .iter()
                        .find_map(|r| match r.event() {
                            GatewayEvent::Request(v) => Some(v),
                            _ => None,
                        })
                        .ok_or("request")?;
                    let record =
                        gateway_store::messages_summary::SqliteMessagesSummaryReader::open(
                            &database,
                            test_secret_store()?,
                        )?
                        .get_owned(
                            request.client_key_id(),
                            request.request_id(),
                            attempts[0].attempt_id(),
                            system_now_ms_runtime()?,
                        )?
                        .ok_or("confirmed summary")?;
                    assert_eq!(record.evidence.items().len(), 1);
                    assert_eq!(
                        serde_json::from_str::<Value>(record.evidence.items()[0].item.get())?,
                        native["output"][0]
                    );
                }
                assert_eq!(
                    usage.len(),
                    1,
                    "{protocol} stream={streaming} extra_usage={extra_usage}"
                );
                assert!(matches!(attempts[0].outcome(), AttemptOutcome::Succeeded));
                assert_eq!(attempts[0].request_id().as_str(), id);
                assert_eq!(usage[0].1.attempt_id(), Some(attempts[0].attempt_id()));
                assert_eq!(rows.iter().filter(|row| matches!(row.event(), GatewayEvent::RequestFinished(value) if value.request_id.as_str()==id && value.outcome==if success {RequestOutcome::Succeeded} else {RequestOutcome::Failed})).count(), 1);
                let expected = UsageSummary::from(&Usage {
                    input_tokens: Some(228),
                    output_tokens: Some(159),
                    reasoning_tokens: Some(158),
                    cached_tokens: Some(128),
                    provenance: UsageProvenance::Measured,
                    input_accounting: InputTokenAccounting::Inclusive,
                    ..Usage::default()
                });
                assert_eq!(usage[0].1.usage(), &expected);
                let worker = crate::billing_worker::BillingWorker::start(
                    database.clone(),
                    Arc::new(
                        gateway_control::billing_processing::BillingProcessingMonitor::default(),
                    ),
                );
                worker.stop().await.map_err(|()| "billing")?;
                let ledger = SqliteBillingLedger::open_read_only(&database)?.list_bounded(10)?;
                assert_eq!(ledger.len(), 1);
                assert_eq!(ledger[0].usage, expected);
                assert_eq!(ledger[0].source_event_id, usage[0].0);
                assert_eq!(ledger[0].request_id, id);
                assert_eq!(ledger[0].cost_microunits, None);
            }
        }
    }
    Ok(())
}

fn event_kind(event: &CanonicalEvent) -> &'static str {
    match event {
        CanonicalEvent::ResponseStart(_) => "response_start",
        CanonicalEvent::MessageStart(_) => "message_start",
        CanonicalEvent::OutputItemStart(_) => "output_item_start",
        CanonicalEvent::OutputItemEnd(_) => "output_item_end",
        CanonicalEvent::ReasoningDelta(_) => "reasoning_delta",
        CanonicalEvent::TextDelta(_) => "text_delta",
        CanonicalEvent::UsageDelta(_) => "usage_delta",
        CanonicalEvent::MessageEnd(_) => "message_end",
        CanonicalEvent::ResponseEnd(_) => "response_end",
        _ => "other",
    }
}

#[actix_web::test]
#[ignore = "requires explicitly selected local private capture; no real Provider"]
#[allow(
    clippy::manual_assert_eq,
    reason = "private input must never be rendered on assertion failure"
)]
async fn grok_build_private_capture_offline_projection() -> TestResult {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;
    let directory = PathBuf::from(std::env::var("CPAR_OFFLINE_CAPTURE_DIR")?);
    assert!(directory.is_absolute() && directory.canonicalize()? == directory);
    assert_eq!(
        std::fs::metadata(&directory)?.permissions().mode() & 0o777,
        0o700
    );
    let raw = std::fs::read(directory.join("native-capture/native-response.bin"))?;
    assert!(
        raw.len() == 673
            && format!("{:x}", Sha256::digest(&raw))
                == "7b5a3c2766766397af638665aaa7160f351780232a2c06f11d8f784ebbfdaa96"
    );
    let payload = std::fs::read(directory.join("selected-request.json"))?;
    assert!(
        format!("{:x}", Sha256::digest(&payload))
            == "245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062"
    );
    let canonical =
        GrokBuildResponsesDecoder::decode_non_streaming_with_content_encoding(Some("gzip"), &raw)?;
    let mut projections = Vec::new();
    for (name, target) in [
        ("chat", ProtocolFormat::OpenAiChatCompletions),
        ("messages", ProtocolFormat::AnthropicMessages),
        ("responses", ProtocolFormat::OpenAiResponses),
    ] {
        let mut projector = ProtocolResponseProjector::new(target);
        let mut result = json!({"protocol":name,"status":"PASS"});
        for (ordinal, event) in canonical.events().iter().enumerate() {
            if let Err(reason) = projector.project_event(event) {
                result = json!({"protocol":name,"status":"REJECTED","ordinal_zero_based":ordinal,
                    "event":event_kind(event),"reason":reason.to_string()});
                break;
            }
        }
        projections.push(result);
    }
    let mut events = Vec::new();
    for (ordinal, event) in canonical.events().iter().enumerate() {
        let mut facts = json!({"ordinal_zero_based":ordinal,"kind":event_kind(event)});
        if let CanonicalEvent::ReasoningDelta(delta) = event {
            let part: Value = serde_json::from_str(
                delta
                    .extensions
                    .get("openai.responses.output_part")
                    .ok_or("part")?
                    .get(),
            )?;
            assert!(part["field"] == "summary" || part["field"] == "content");
            facts["part_field"] = part["field"].clone();
            facts["part_index"] = part["index"].clone();
        }
        if let CanonicalEvent::UsageDelta(delta) = event {
            facts["unrepresented_usage_extension_count"] =
                json!(delta.usage.extensions.iter().len());
        }
        events.push(facts);
    }
    let peer = Arc::new(OfflinePeer {
        json: raw,
        sse: Vec::new(),
        encoding: GrokBuildResponseContentEncoding::Gzip,
        received: AtomicUsize::new(0),
    });
    let fixture = offline_fixture(peer.clone())?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/chat/completions")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .insert_header(("content-type", "application/json"))
            .set_payload(payload)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let request_id_present = response.headers().get("x-request-id").is_some();
    let wire = test::read_body(response).await;
    let error: Value = serde_json::from_slice(&wire)?;
    assert!(error["error"]["code"] == "UpstreamProtocolError");
    assert!(
        error["error"]["message"] == "response projection cannot represent the response metadata"
    );
    assert_eq!(peer.received.load(Ordering::Relaxed), 1);
    let receipt = json!({"status":"PASS; original failure reproduced offline","native_decode":"PASS",
        "canonical_events":events,"projections":projections,"public_chat_http_status":502,
        "public_safe_error_verified":true,"request_id_present":request_id_present,
        "mock_transport_receives":1,"external_provider_calls":0,"original_request_unchanged":true});
    std::fs::write(
        std::env::var("CPAR_OFFLINE_RECEIPT")?,
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(())
}
