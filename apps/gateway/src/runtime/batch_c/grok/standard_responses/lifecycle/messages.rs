//! #63 controlled Messages through the real compiler, leased Build adapter and HTTP pool.
use super::*;
use gateway_store::messages_summary::{MessagesSummaryRecord, SqliteMessagesSummaryReader};
mod cancellation;
mod failures;
mod policy;
mod tool_rounds;

struct MessagesTransport {
    target: AdmittedEgressTarget,
    pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
    size: usize,
}

struct MessagesBody {
    inner: SocketBody,
    pending: Vec<u8>,
    size: usize,
}
impl GrokBuildResponseBody for MessagesBody {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            if self.pending.is_empty() {
                let Some(bytes) = self.inner.next_chunk().await? else {
                    return Ok(None);
                };
                self.pending = bytes;
            }
            let length = self.size.min(self.pending.len());
            Ok(Some(self.pending.drain(..length).collect()))
        })
    }
}
impl MessagesTransport {
    fn new(peer: &ModelPeer, size: usize) -> Result<Arc<Self>, Box<dyn Error>> {
        let port = peer
            .url
            .split(':')
            .nth(2)
            .and_then(|v| v.split('/').next())
            .ok_or("port")?
            .parse()?;
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("messages-63-loopback")?,
            name: "Messages controlled peer".into(),
            allowed_schemes: BTreeSet::from([EgressScheme::Http]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("127.0.0.1")?]),
            allowed_ports: BTreeSet::from([port]),
            allowed_cidrs: BTreeSet::from([EgressCidr::try_new("127.0.0.1".parse()?, 32)?]),
            redirect_policy: RedirectPolicy::Deny,
        })?;
        Ok(Arc::new(Self {
            target: policy.admit_url(&peer.url, &SystemEgressDnsResolver)?,
            pool: UpstreamClientPool::new(NonZeroUsize::new(2).ok_or("pool")?),
            profile: UpstreamTransportProfile::new(
                UpstreamTimeouts::try_new(
                    Duration::from_secs(2),
                    Duration::from_secs(2),
                    Duration::from_secs(3),
                    Duration::from_secs(10),
                )?,
                UpstreamProxy::Direct,
                NonZeroUsize::new(1).ok_or("bound")?,
            ),
            size,
        }))
    }
}
impl GrokBuildTransport for MessagesTransport {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            assert_eq!(
                request.url(),
                "https://cli-chat-proxy.grok.com/v1/responses"
            );
            let streaming = serde_json::from_slice::<Value>(request.body())
                .map_err(|_| internal_error())?["stream"]
                == true;
            let handoff = UpstreamHttpRequest::try_new(
                self.target.clone(),
                UpstreamHttpMethod::Post,
                [
                    ("content-type".into(), "application/json".into()),
                    (
                        "x-grok-model-override".into(),
                        request
                            .header("x-grok-model-override")
                            .ok_or_else(internal_error)?
                            .into(),
                    ),
                ],
                request.body().to_vec(),
            )
            .map_err(|_| internal_error())?;
            let response = self.pool.send(handoff, &self.profile).await?;
            Ok(GrokBuildTransportResponse::new(
                response.status(),
                if streaming {
                    GrokBuildResponseContentType::EventStream
                } else {
                    GrokBuildResponseContentType::Json
                },
                provider_grok::GrokBuildResponseContentEncoding::Identity,
                Box::new(MessagesBody {
                    inner: SocketBody(response),
                    pending: Vec::new(),
                    size: self.size,
                }),
            ))
        })
    }
}

#[actix_web::test]
async fn public_build_messages_json_retains_summary_before_success() -> TestResult {
    summary_answer(false).await
}
#[actix_web::test]
async fn public_build_messages_sse_retains_summary_before_success() -> TestResult {
    summary_answer(true).await
}
async fn summary_answer(streaming: bool) -> TestResult {
    let peer = ModelPeer::start_wire(
        streaming,
        super::super::tool_rounds::wire_response(1, streaming)?,
    )?;
    let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
    let recording = Recording::start(&fixture, false)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let reply = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(
                json!({"model":"p12-test-model","max_tokens":19,"stream":streaming,
            "messages":[{"role":"user","content":"question"}]}),
            )
            .to_request(),
    )
    .await;
    let status = reply.status();
    let request_id = reply
        .headers()
        .get("x-request-id")
        .ok_or("request id")?
        .to_str()?
        .to_owned();
    let bytes = test::read_body(reply).await;
    assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    if streaming {
        let frames = public_frames(&bytes)?;
        assert_eq!(
            frames
                .iter()
                .filter(|v| v["type"] == "message_stop")
                .count(),
            1,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
    } else {
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes)?["content"][0]["text"],
            "回答 1"
        );
    }
    assert!(!String::from_utf8_lossy(&bytes).contains("计划"));
    assert!(!String::from_utf8_lossy(&bytes).contains("thinking"));
    let record = retained(&recording.database, &request_id)?;
    assert_eq!(
        record.evidence.completeness(),
        gateway_core::MessagesSummaryCompleteness::CompletedResponse
    );
    assert_eq!(
        record.evidence.source(),
        if streaming {
            gateway_core::MessagesSummarySource::BuildSse
        } else {
            gateway_core::MessagesSummarySource::BuildJson
        }
    );
    let item: Value = serde_json::from_str(record.evidence.items()[0].item.get())?;
    assert_eq!(item["summary"][0]["text"], "计划 1");
    assert!(item.get("content").is_none());
    assert!(
        SqliteMessagesSummaryReader::open(&recording.database, test_secret_store()?)?
            .get_owned(
                &gateway_core::ClientKeyId::try_new("different-owner")?,
                record.attempt.request_id(),
                record.attempt.attempt_id(),
                system_now_ms_runtime()?
            )?
            .is_none()
    );
    drop(app);
    recording.finish(&fixture, 0).await?;
    Ok(())
}

fn retained(
    database: &std::path::Path,
    request_id: &str,
) -> Result<MessagesSummaryRecord, Box<dyn Error>> {
    let rows = SqliteEventStore::open_read_only(database)?.list_events()?;
    let admitted = rows
        .iter()
        .find_map(|row| match row.event() {
            GatewayEvent::Request(request) if request.request_id().as_str() == request_id => {
                Some(request)
            }
            _ => None,
        })
        .ok_or("admitted")?;
    let attempts: Vec<_> = rows
        .iter()
        .filter_map(|row| match row.event() {
            GatewayEvent::Attempt(attempt) if attempt.request_id().as_str() == request_id => {
                Some(attempt)
            }
            _ => None,
        })
        .collect();
    assert_eq!(attempts.len(), 1);
    SqliteMessagesSummaryReader::open(database, test_secret_store()?)?
        .get_owned(
            admitted.client_key_id(),
            admitted.request_id(),
            attempts[0].attempt_id(),
            system_now_ms_runtime()?,
        )?
        .ok_or_else(|| "retained summary".into())
}

async fn accounting(database: &std::path::Path, count: usize) -> TestResult {
    let rows = SqliteEventStore::open_read_only(database)?.list_events()?;
    for predicate in [0, 1, 2] {
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(
                    (predicate, r.event()),
                    (0, GatewayEvent::Request(_))
                        | (1, GatewayEvent::Attempt(_))
                        | (2, GatewayEvent::Usage(_))
                ))
                .count(),
            count
        );
    }
    assert!(
        !serde_json::to_string(
            &rows
                .iter()
                .map(gateway_store::event_store::StoredGatewayEvent::event)
                .collect::<Vec<_>>()
        )?
        .contains("计划")
    );
    let worker = crate::billing_worker::BillingWorker::start(
        database.to_path_buf(),
        Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
    );
    worker.stop().await.map_err(|()| "billing")?;
    let ledger = SqliteBillingLedger::open_read_only(database)?.list_bounded(100)?;
    assert_eq!(ledger.len(), count);
    for row in &ledger {
        assert_eq!(row.usage.input_tokens, Some(20));
        assert_eq!(row.usage.output_tokens, Some(7));
        assert_eq!(row.cost_microunits, None);
        assert!(
            rows.iter().any(|r| r.event_id() == row.source_event_id
                && matches!(r.event(), GatewayEvent::Usage(_)))
        );
    }
    Ok(())
}
