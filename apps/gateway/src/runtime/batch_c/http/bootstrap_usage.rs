//! B-04 public bootstrap failures and the actual transport-read boundary.
use super::*;
use gateway_core::{AttemptOutcome, InputTokenAccounting, RequestOutcome, UsageProvenance};
use gateway_store::{
    billing_ledger::{BillingLedgerEntry, SqliteBillingLedger},
    event_store::{AsyncSqliteEventWriter, EventWriterConfig, SqliteEventStore, UsageEventQuery},
};

struct ReplayUsageSink(Arc<dyn GatewayEventSink>);
impl GatewayEventSink for ReplayUsageSink {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        self.0.try_emit(event)
    }
    fn emit_confirmed(&self, event: GatewayEvent) -> gateway_core::EventEmissionFuture<'_> {
        Box::pin(async move {
            if matches!(event, GatewayEvent::Usage(_)) {
                let result = self.0.emit_confirmed(event.clone()).await;
                if result.into_result().is_err() {
                    return result;
                }
            }
            self.0.emit_confirmed(event).await
        })
    }
}

type TransportReads = Vec<(&'static str, Vec<u8>)>;
std::thread_local! {
    static READS: std::cell::RefCell<Option<TransportReads>> = const { std::cell::RefCell::new(None) };
}

pub(in crate::runtime) fn record_read(protocol: &'static str, chunk: &[u8]) {
    READS.with_borrow_mut(|reads| {
        if let Some(reads) = reads {
            reads.push((protocol, chunk.to_vec()));
        }
    });
}

fn responses_wire() -> String {
    concat!(
        "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-b04\",\"object\":\"response\",\"status\":\"in_progress\",\"output\":[],\"usage\":{\"input_tokens\":20,\"input_tokens_details\":{\"cached_tokens\":5}}}}\n\n",
        "event: response.in_progress\ndata: malformed\n\n"
    ).into()
}

#[actix_web::test]
async fn bootstrap_failure_preserves_responses_usage() -> TestResult {
    let expected = Usage {
        input_tokens: Some(20),
        cached_tokens: Some(5),
        provenance: UsageProvenance::Measured,
        input_accounting: InputTokenAccounting::Inclusive,
        ..Usage::default()
    };
    check_failure("responses", responses_wire(), Some(expected), true).await
}

fn chat_wire() -> String {
    concat!(
        "data: {\"id\":\"chat-b04\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"chat-b04\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: {\"id\":\"chat-b04\",\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":20,\"completion_tokens\":10,\"total_tokens\":30,\"prompt_tokens_details\":{\"cached_tokens\":5},\"completion_tokens_details\":{\"reasoning_tokens\":3}}}\n\n",
        "data: malformed\n\n"
    ).into()
}

#[actix_web::test]
async fn bootstrap_failure_preserves_chat_usage() -> TestResult {
    check_failure(
        "chat",
        chat_wire(),
        Some(Usage {
            input_tokens: Some(20),
            output_tokens: Some(10),
            cached_tokens: Some(5),
            reasoning_tokens: Some(3),
            provenance: UsageProvenance::Measured,
            input_accounting: InputTokenAccounting::Inclusive,
            ..Usage::default()
        }),
        true,
    )
    .await
}

#[actix_web::test]
async fn messages_hands_off_before_a_later_bad_frame_and_keeps_known_input_usage() -> TestResult {
    let wire = concat!(
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg-b04\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"upstream-c\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":11,\"output_tokens\":0,\"cache_read_input_tokens\":5,\"cache_creation_input_tokens\":2}}}\n\n",
        "event: content_block_delta\ndata: malformed\n\n"
    ).into();
    check_failure(
        "messages",
        wire,
        Some(Usage {
            input_tokens: Some(11),
            cache_read_tokens: Some(5),
            cache_creation_tokens: Some(2),
            provenance: UsageProvenance::Measured,
            input_accounting: InputTokenAccounting::Exclusive,
            ..Usage::default()
        }),
        false,
    )
    .await
}

#[actix_web::test]
async fn missing_or_invalid_bootstrap_usage_never_becomes_a_record() -> TestResult {
    for wire in [
        responses_wire().replace(
            ",\"usage\":{\"input_tokens\":20,\"input_tokens_details\":{\"cached_tokens\":5}}",
            "",
        ),
        responses_wire().replace("\"input_tokens\":20", "\"input_tokens\":-1"),
    ] {
        check_failure("responses", wire, None, true).await?;
    }
    for wire in [
        chat_wire()
            .split("data: {\"id\":\"chat-b04\",\"object\":\"chat.completion.chunk\",\"choices\":[]")
            .next()
            .ok_or("chat prefix")?
            .to_owned()
            + "data: malformed\n\n",
        chat_wire().replace("\"total_tokens\":30", "\"total_tokens\":31"),
    ] {
        check_failure("chat", wire, None, true).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn valid_bootstrap_snapshots_merge_without_addition_before_failure() -> TestResult {
    let item = json!({"id":"msg-b04","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"ok","annotations":[],"logprobs":[]}]});
    let frames = [
        json!({"type":"response.output_item.added","output_index":0,"item":{"id":"msg-b04","type":"message","role":"assistant","status":"in_progress","content":[]}}),
        json!({"type":"response.output_text.delta","item_id":"msg-b04","output_index":0,"content_index":0,"delta":"ok"}),
        json!({"type":"response.output_item.done","output_index":0,"item":item}),
        json!({"type":"response.completed","response":{"id":"resp-b04","status":"completed","output":[item],"usage":{"input_tokens":22,"output_tokens":10,"total_tokens":32,"input_tokens_details":{"cached_tokens":7},"output_tokens_details":{"reasoning_tokens":3}}}}),
    ];
    let mut wire = responses_wire()
        .split("event: response.in_progress")
        .next()
        .ok_or("created")?
        .to_owned();
    for frame in frames {
        write!(
            wire,
            "event: {}\ndata: {frame}\n\n",
            frame["type"].as_str().ok_or("type")?
        )?;
    }
    wire.push_str("event: response.in_progress\ndata: malformed\n\n");
    check_failure(
        "responses",
        wire,
        Some(Usage {
            input_tokens: Some(22),
            output_tokens: Some(10),
            cached_tokens: Some(7),
            reasoning_tokens: Some(3),
            provenance: UsageProvenance::Measured,
            input_accounting: InputTokenAccounting::Inclusive,
            ..Usage::default()
        }),
        true,
    )
    .await
}

#[actix_web::test]
#[allow(
    clippy::too_many_lines,
    reason = "two usable candidates prove retry policy and separate durable attempt facts"
)]
async fn failed_attempt_usage_stays_separate_from_a_usable_fallback() -> TestResult {
    for protocol in ["chat", "responses"] {
        let wire = if protocol == "chat" {
            chat_wire()
        } else {
            responses_wire()
        };
        let first = Peer::start_scripted(false, None, Some(vec![wire.clone().into_bytes()]))?;
        let fallback = if protocol == "chat" {
            Peer::start_scripted(false, None, Some(vec![concat!(
                "data: {\"id\":\"chat-b04\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"answer\"},\"finish_reason\":null}]}\n\n",
                "data: {\"id\":\"chat-b04\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                "data: {\"id\":\"chat-b04\",\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":4,\"total_tokens\":7}}\n\n",
                "data: [DONE]\n\n"
            ).as_bytes().to_vec()]))?
        } else {
            Peer::start(false, None)?
        };
        let (adapter, format, path, uri, payload) = if protocol == "chat" {
            (
                "openai-compatible.chat-completions",
                "openai/chat-completions",
                "/chat/completions",
                "/v1/chat/completions",
                json!({"model":"upstream-c","messages":[{"role":"user","content":"synthetic"}],"stream":true}),
            )
        } else {
            (
                "openai-compatible.responses",
                "openai/responses",
                "/responses",
                "/v1/responses",
                json!({"model":"upstream-c","input":"synthetic","stream":true,"store":true}),
            )
        };
        let fixture = responses_done::fallback_fixture(
            &first,
            &fallback,
            Channel {
                adapter,
                format,
                base: "https://gateway.example.test/v1",
                path,
                model: "upstream-c",
            },
        )?;
        let database = fixture.directory.path().join("batch-c.sqlite3");
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(16, 1)?)?;
        let writer = AsyncSqliteEventWriter::new(
            &database,
            receiver,
            EventWriterConfig::try_new(4, Duration::from_millis(5))?,
        );
        let sink: Arc<dyn GatewayEventSink> = Arc::new(P12FanoutEventSink::new(
            Arc::new(P12AttemptStageStore::new()),
            Arc::new(queue),
        ));
        *fixture.events.1.lock().map_err(|_| "sink")? = Some(Arc::new(ReplayUsageSink(sink)));
        let writer = tokio::spawn(writer.run());
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        READS.with_borrow_mut(|reads| *reads = Some(Vec::new()));
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(uri)
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(payload)
                .to_request(),
        )
        .await;
        let status = response.status();
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        let reads = READS.with_borrow_mut(Option::take).ok_or("read capture")?;
        assert_eq!(reads.first(), Some(&(protocol, wire.into_bytes())));
        let retried = protocol == "chat";
        for peer in [&first, &fallback] {
            let expected = if Arc::ptr_eq(peer, &first) {
                1
            } else {
                usize::from(retried)
            };
            assert_eq!(
                peer.received.lock().map_err(|_| "received")?.len(),
                expected
            );
            assert_eq!(peer.requests.lock().map_err(|_| "handoff")?.len(), expected);
        }
        drop(app);
        fixture.events.1.lock().map_err(|_| "sink")?.take();
        let metrics = tokio::time::timeout(Duration::from_secs(5), writer).await??;
        let source = SqliteEventStore::open_read_only(&database)?;
        let events = source.list_events()?;
        eprintln!(
            "B-04 fallback protocol={protocol}; status={status}; body={body}; writer={metrics:?}; events={events:?}"
        );
        assert_eq!(metrics.required_events_quarantined, 0);
        let attempts = events
            .iter()
            .filter_map(|row| match row.event() {
                GatewayEvent::Attempt(attempt) => Some(attempt),
                _ => None,
            })
            .collect::<Vec<_>>();
        let usages = events
            .iter()
            .filter_map(|row| match row.event() {
                GatewayEvent::Usage(usage) => Some(usage),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(attempts.len(), if retried { 2 } else { 1 });
        assert_eq!(usages.len(), attempts.len());
        assert!(matches!(attempts[0].outcome(), AttemptOutcome::Failed(_)));
        assert_eq!(usages[0].attempt_id(), Some(attempts[0].attempt_id()));
        assert_eq!(usages[0].usage().input_tokens, Some(20));
        assert_eq!(usages[0].usage().cached_tokens, Some(5));
        if retried {
            assert_eq!(status, StatusCode::OK);
            assert!(
                body.contains("[DONE]") && !body.contains("RecordingUnavailable"),
                "{body}"
            );
            assert!(matches!(attempts[1].outcome(), AttemptOutcome::Succeeded));
            assert_eq!(usages[1].attempt_id(), Some(attempts[1].attempt_id()));
            assert_eq!(usages[1].usage().input_tokens, Some(3));
            assert_eq!(usages[1].usage().output_tokens, Some(4));
            assert_eq!(usages[1].usage().cached_tokens, None);
            assert_eq!(usages[0].response_id(), usages[1].response_id());
        } else {
            assert_eq!(status, StatusCode::BAD_GATEWAY);
            assert!(body.contains("UpstreamProtocolError") && !body.contains("response.completed"));
        }
        assert!(events.iter().any(|row| matches!(row.event(), GatewayEvent::RequestFinished(request) if request.outcome == if retried { RequestOutcome::Succeeded } else { RequestOutcome::Failed })));
        let query =
            gateway_control::management_operations_service::OperationalUsageQuery::default();
        let read = gateway_control::management_operations_service::read_operational_usage_page(
            &source, &query,
        )?;
        let compiled =
            gateway_control::management_operations_service::compile_operational_usage_page(
                &events, &query,
            )?;
        assert_eq!(read, compiled);
        assert_eq!(read.items.len(), usages.len());
        assert_eq!(read.excluded_usage_events, 0);
        let worker = crate::billing_worker::BillingWorker::start(
            database.clone(),
            Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
        );
        worker.stop().await.map_err(|()| "billing worker")?;
        let ledger = SqliteBillingLedger::open_read_only(&database)?.list_bounded(10)?;
        assert_eq!(ledger.len(), usages.len());
        for usage in &usages {
            let attempt = attempts
                .iter()
                .find(|attempt| Some(attempt.attempt_id()) == usage.attempt_id())
                .ok_or("owned attempt")?;
            let row = ledger
                .iter()
                .find(|row| row.account_id == attempt.credential_id().as_str())
                .ok_or("owned ledger row")?;
            assert_eq!(&row.usage, usage.usage());
            assert_eq!(row.request_id, usage.request_id().as_str());
            assert_eq!(row.response_id, usage.response_id().as_str());
            let source_id = events
                .iter()
                .find_map(|stored| match stored.event() {
                    GatewayEvent::Usage(value) if value == *usage => Some(stored.event_id()),
                    _ => None,
                })
                .ok_or("usage source")?;
            assert_eq!(row.source_event_id, source_id);
            assert_eq!(row.cost_microunits, None);
        }
        assert_eq!(
            ledger
                .iter()
                .map(|row| &row.source_event_id)
                .collect::<BTreeSet<_>>()
                .len(),
            usages.len()
        );
        check_protected_usage(&database, &ledger).await?;
        eprintln!("B-04 fallback read={read:?}; ledger={ledger:?}");
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "one public request binds failure, read boundary, durable lineage and ledger facts"
)]
async fn check_failure(
    protocol: &'static str,
    wire: String,
    expected: Option<Usage>,
    bootstrap_failure: bool,
) -> TestResult {
    let peer = Peer::start_scripted(false, None, Some(vec![wire.clone().into_bytes()]))?;
    let (adapter, format, path, uri, payload) = match protocol {
        "responses" => (
            "openai-compatible.responses",
            "openai/responses",
            "/responses",
            "/v1/responses",
            json!({"model":"upstream-c","input":"synthetic","stream":true,"store":true}),
        ),
        "chat" => (
            "openai-compatible.chat-completions",
            "openai/chat-completions",
            "/chat/completions",
            "/v1/chat/completions",
            json!({"model":"upstream-c","messages":[{"role":"user","content":"synthetic"}],"stream":true}),
        ),
        "messages" => (
            "anthropic-compatible.messages",
            "anthropic/messages",
            "/messages",
            "/v1/messages",
            json!({"model":"upstream-c","messages":[{"role":"user","content":"synthetic"}],"max_tokens":64,"stream":true}),
        ),
        _ => return Err("protocol".into()),
    };
    let fixture = fixture_with_storage(
        Channel {
            adapter,
            format,
            base: "https://gateway.example.test/v1",
            path,
            model: "upstream-c",
        },
        b"synthetic-key",
        protocol == "responses",
        |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.http = Some(peer.clone());
            endpoint.transports = Arc::new(profiles);
            endpoint.resolver = Arc::new(PublicResolver);
            Ok(())
        },
    )?;
    let database = fixture.directory.path().join("batch-c.sqlite3");
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(16, 1)?)?;
    let queue = Arc::new(queue);
    let writer = AsyncSqliteEventWriter::new(
        &database,
        receiver,
        EventWriterConfig::try_new(4, Duration::from_millis(5))?,
    );
    let sink: Arc<dyn GatewayEventSink> = Arc::new(P12FanoutEventSink::new(
        Arc::new(P12AttemptStageStore::new()),
        queue,
    ));
    *fixture.events.1.lock().map_err(|_| "sink")? = Some(Arc::new(ReplayUsageSink(sink)));
    let writer = tokio::spawn(writer.run());
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    READS.with_borrow_mut(|reads| *reads = Some(Vec::new()));
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(uri)
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(payload)
            .to_request(),
    )
    .await;
    let status = response.status();
    let body = String::from_utf8(test::read_body(response).await.to_vec())?;
    let reads = READS.with_borrow_mut(Option::take).ok_or("read capture")?;
    eprintln!(
        "B-04 protocol={protocol}; status={status}; reads={:?}; body={body}",
        reads
            .iter()
            .map(|(kind, bytes)| (*kind, bytes.len()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        reads,
        vec![(protocol, wire.into_bytes())],
        "must hit one actual body read, no TCP-write assumption"
    );
    assert_eq!(
        status,
        if bootstrap_failure {
            StatusCode::BAD_GATEWAY
        } else {
            StatusCode::OK
        }
    );
    assert!(
        !body.contains("response.completed") && !body.contains("[DONE]"),
        "{body}"
    );
    if protocol == "responses" {
        let get = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/responses/resp-b04")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .to_request(),
        )
        .await;
        assert_eq!(get.status(), StatusCode::NOT_FOUND);
        let continuation = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(
                    json!({"model":"upstream-c","input":"next","previous_response_id":"resp-b04"}),
                )
                .to_request(),
        )
        .await;
        assert!(!continuation.status().is_success());
    }
    assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), 1);
    assert_eq!(peer.requests.lock().map_err(|_| "handoff")?.len(), 1);
    drop(app);
    fixture.events.1.lock().map_err(|_| "sink")?.take();
    let metrics = tokio::time::timeout(Duration::from_secs(5), writer).await??;
    assert_eq!(metrics.pending_required, 0);
    assert_eq!(metrics.required_events_quarantined, 0);
    let source = SqliteEventStore::open_read_only(&database)?;
    let events = source.list_events()?;
    let attempt = events
        .iter()
        .find_map(|row| match row.event() {
            GatewayEvent::Attempt(attempt) => Some(attempt),
            _ => None,
        })
        .ok_or("attempt")?;
    assert_eq!(attempt.attempt_number(), 1);
    assert_eq!(
        matches!(attempt.outcome(), AttemptOutcome::Failed(_)),
        bootstrap_failure
    );
    assert!(events.iter().any(|row| matches!(row.event(), GatewayEvent::RequestFinished(request) if request.request_id == *attempt.request_id() && request.outcome == RequestOutcome::Failed)));
    let usage = events
        .iter()
        .filter_map(|row| match row.event() {
            GatewayEvent::Usage(usage) => Some(usage),
            _ => None,
        })
        .collect::<Vec<_>>();
    eprintln!("B-04 durable: {events:?}");
    assert_eq!(
        usage.len(),
        usize::from(expected.is_some()),
        "validated bootstrap Usage must survive before source handoff"
    );
    if let Some(expected) = &expected {
        assert_eq!(usage[0].request_id(), attempt.request_id());
        assert_eq!(usage[0].attempt_id(), Some(attempt.attempt_id()));
        assert_eq!(
            usage[0].usage(),
            &gateway_core::UsageSummary::from(expected)
        );
    }
    let mut visited = 0;
    let read = source.visit_usage_lineages(
        &UsageEventQuery::default(),
        |_, read_attempt, read_usage| {
            assert_eq!(read_attempt.attempt_id(), attempt.attempt_id());
            assert_eq!(Some(read_usage), usage.first().copied());
            visited += 1;
            Ok(true)
        },
    )?;
    assert_eq!(visited, usage.len());
    assert_eq!(read.excluded_usage_events, 0);
    let worker = crate::billing_worker::BillingWorker::start(
        database.clone(),
        Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
    );
    worker.stop().await.map_err(|()| "billing worker")?;
    let ledger = SqliteBillingLedger::open_read_only(&database)?.list_bounded(10)?;
    assert_eq!(ledger.len(), usage.len());
    if let Some(expected) = expected {
        assert_eq!(ledger[0].usage, gateway_core::UsageSummary::from(&expected));
        assert_eq!(ledger[0].request_id, attempt.request_id().as_str());
        assert_eq!(ledger[0].account_id, attempt.credential_id().as_str());
        assert_eq!(ledger[0].response_id, usage[0].response_id().as_str());
        let source_id = events
            .iter()
            .find_map(|stored| match stored.event() {
                GatewayEvent::Usage(value) if value == usage[0] => Some(stored.event_id()),
                _ => None,
            })
            .ok_or("usage source")?;
        assert_eq!(ledger[0].source_event_id, source_id);
        assert_eq!(ledger[0].cost_microunits, None);
    }
    check_protected_usage(&database, &ledger).await?;
    eprintln!("B-04 writer={metrics:?}; read={read:?}; ledger={ledger:?}");
    Ok(())
}

struct DurableUsageFacade(std::path::PathBuf);
impl gateway_http_actix::management_resources::ManagementUsageFacade for DurableUsageFacade {
    fn list_usage(
        &self,
        query: &gateway_control::management_operations_service::OperationalUsageQuery,
    ) -> Result<
        gateway_control::management_operations_service::OperationalUsagePage,
        gateway_control::management_operations_service::ManagementOperationsError,
    > {
        let source = SqliteEventStore::open_read_only(&self.0).map_err(|_| {
            gateway_control::management_operations_service::ManagementOperationsError::SourceUnavailable
        })?;
        gateway_control::management_operations_service::read_operational_usage_page(&source, query)
    }
    fn list_billing(
        &self,
        query: &gateway_control::management_operations_service::OperationalBillingQuery,
    ) -> Result<
        gateway_control::management_operations_service::OperationalBillingPage,
        gateway_control::management_operations_service::ManagementOperationsError,
    > {
        let ledger = SqliteBillingLedger::open_read_only(&self.0).map_err(|_| {
            gateway_control::management_operations_service::ManagementOperationsError::SourceUnavailable
        })?;
        gateway_control::management_operations_service::read_operational_billing_page(
            &ledger, query,
        )
    }
}

async fn check_protected_usage(
    database: &std::path::Path,
    ledger: &[BillingLedgerEntry],
) -> TestResult {
    use gateway_control::management_mutation_service::{
        ManagementMutationService, SqliteControlPlaneRepository,
    };
    use gateway_http_actix::{
        management_resources::{ManagementResourceHttpState, configure_management_resources},
        management_security::{
            MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
            ManagementNetworkPolicy,
        },
    };
    let key = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";
    let security = ManagementHttpState::new(
        ManagementKey::try_new(key)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?;
    let resources = ManagementResourceHttpState::new(ManagementMutationService::new(
        SqliteControlPlaneRepository::open(database)?,
        test_secret_store()?,
    ))
    .with_usage(Box::new(DurableUsageFacade(database.to_path_buf())));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security))
            .app_data(web::Data::new(resources))
            .configure(configure_management_resources),
    )
    .await;
    for kind in ["usage", "billing"] {
        let uri = format!("/admin/operations/{kind}");
        let denied = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&uri)
                .peer_addr(([127, 0, 0, 1], 45004).into())
                .to_request(),
        )
        .await;
        assert_eq!(denied.status(), StatusCode::NOT_FOUND);
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&uri)
                .peer_addr(([127, 0, 0, 1], 45004).into())
                .insert_header((MANAGEMENT_KEY_HEADER, key))
                .insert_header(("X-Config-Version", "batch-c"))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let body: Value = test::read_body_json(response).await;
        check_protected_items(kind, &body, ledger)?;
        let serialized = serde_json::to_string(&body)?;
        for forbidden in [
            "source_event_id",
            "source_fingerprint",
            "encrypted_secret",
            key,
        ] {
            assert!(!serialized.contains(forbidden));
        }
        eprintln!("B-04 protected {kind}: {body}");
    }
    Ok(())
}

fn check_protected_items(kind: &str, body: &Value, ledger: &[BillingLedgerEntry]) -> TestResult {
    let items = body["items"].as_array().ok_or("items")?;
    assert_eq!(items.len(), ledger.len());
    for row in ledger {
        let item = items
            .iter()
            .find(|item| item["account_id"] == row.account_id)
            .ok_or("account")?;
        for (field, expected) in [
            ("input_tokens", row.usage.input_tokens),
            ("output_tokens", row.usage.output_tokens),
            ("cached_tokens", row.usage.cached_tokens),
            ("reasoning_tokens", row.usage.reasoning_tokens),
            ("cache_read_tokens", row.usage.cache_read_tokens),
            ("cache_creation_tokens", row.usage.cache_creation_tokens),
        ] {
            let value = if kind == "usage" {
                &item[field]["total"]
            } else {
                &item[field]
            };
            assert_eq!(*value, json!(expected));
        }
        assert_eq!(item["cost_microunits"], Value::Null);
        assert_eq!(item["cost_confidence"], "unpriced");
        if kind == "usage" {
            assert_eq!(item["request_count"], 1);
            assert_eq!(item["usage_observations"], 1);
            assert_eq!(
                item["input_tokens"]["provenance"],
                json!(row.usage.provenance)
            );
        } else {
            assert_eq!(item["request_id"], row.request_id);
            assert_eq!(item["response_id"], row.response_id);
            assert_eq!(item["usage_provenance"], json!(row.usage.provenance));
            assert_eq!(item["input_accounting"], json!(row.usage.input_accounting));
        }
    }
    Ok(())
}
