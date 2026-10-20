//! C1–C3 only: owned field qualification, real listener, compiler/lease/pool, receiver and writer.
use super::*;
mod negatives;

struct FragmentTransport {
    inner: Arc<dyn GrokBuildTransport>,
    size: usize,
}

impl GrokBuildTransport for FragmentTransport {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            let (status, kind, encoding, body) = self.inner.send(request).await?.into_parts();
            Ok(GrokBuildTransportResponse::new(
                status,
                kind,
                encoding,
                Box::new(super::super::tool_rounds::FragmentBody {
                    inner: body,
                    pending: Vec::new(),
                    size: self.size,
                }),
            ))
        })
    }
}

fn declarations() -> Value {
    let function = json!({"type":"function","name":"lookup","description":"exact child",
        "parameters":{"type":"object","properties":{"previous":{"type":"integer"}},"additionalProperties":false},"strict":false});
    json!([function.clone(),
        {"type":"namespace","name":"read_side","description":"Read the seed without changing it.","tools":[function.clone()]},
        {"type":"namespace","name":"write_side","description":"Use the seed to compute the result.","tools":[function]}])
}

fn output(round: usize) -> Value {
    let mut items = super::super::tool_rounds::output(round);
    if round == 0 || round == 2 {
        items[1]["namespace"] = json!(if round == 0 {
            "read_side"
        } else {
            "write_side"
        });
        items[1]["arguments"] = json!(if round == 0 {
            "{}"
        } else {
            r#"{"previous":41}"#
        });
    } else {
        items[1]["content"][0]["text"] = json!(if round == 1 {
            "Seed is 41."
        } else {
            "Result is 42."
        });
    }
    items
}

fn wire(round: usize, streaming: bool) -> Result<String, Box<dyn Error>> {
    let items = output(round);
    let mut terminal = response();
    terminal["id"] = json!(format!("namespace-response-{round}"));
    terminal["output"] = items.clone();
    if !streaming {
        return Ok(terminal.to_string());
    }
    let mut start = created();
    start["id"] = terminal["id"].clone();
    let mut wire = frame("response.created", &start);
    for (index, item) in items.as_array().ok_or("items")?.iter().enumerate() {
        let mut added = item.clone();
        added["status"] = json!("in_progress");
        let tool = item["type"] == "function_call";
        if index == 0 {
            added["summary"] = json!([]);
        } else if tool {
            added["arguments"] = json!("");
        } else {
            added["content"] = json!([]);
        }
        wire += &native_event(
            "response.output_item.added",
            json!({"output_index":index,"item":added}),
        );
        let (kind, text) = if index == 0 {
            (
                "response.reasoning_summary_text.delta",
                item["summary"][0]["text"].as_str().ok_or("summary")?,
            )
        } else if tool {
            (
                "response.function_call_arguments.delta",
                item["arguments"].as_str().ok_or("args")?,
            )
        } else {
            (
                "response.output_text.delta",
                item["content"][0]["text"].as_str().ok_or("text")?,
            )
        };
        for character in text.chars() {
            let mut payload =
                json!({"output_index":index,"item_id":item["id"],"delta":character.to_string()});
            if index == 0 {
                payload["summary_index"] = json!(0);
            } else if !tool {
                payload["content_index"] = json!(0);
            }
            wire += &native_event(kind, payload);
        }
        if tool {
            wire += &native_event(
                "response.function_call_arguments.done",
                json!({"output_index":index,
                "item_id":item["id"],"call_id":item["call_id"],"name":item["name"],"arguments":item["arguments"]}),
            );
        }
        wire += &native_event(
            "response.output_item.done",
            json!({"output_index":index,"item":item}),
        );
    }
    Ok(wire + &frame("response.completed", &terminal))
}

fn transport(peer: &ModelPeer, size: usize) -> Result<Arc<dyn GrokBuildTransport>, Box<dyn Error>> {
    let url = reqwest::Url::parse(&peer.url)?;
    let port = url.port().ok_or("owned port")?;
    let policy = EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("namespace-owned-loopback")?,
        name: "Owned namespace receiver".into(),
        allowed_schemes: BTreeSet::from([EgressScheme::Http]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new("127.0.0.1")?]),
        allowed_ports: BTreeSet::from([port]),
        allowed_cidrs: BTreeSet::from([EgressCidr::try_new("127.0.0.1".parse()?, 32)?]),
        redirect_policy: RedirectPolicy::Deny,
    })?;
    let inner = SocketTransport {
        target: policy.admit_url(&peer.url, &SystemEgressDnsResolver)?,
        pool: UpstreamClientPool::new(NonZeroUsize::new(2).ok_or("pool")?),
        profile: UpstreamTransportProfile::new(
            UpstreamTimeouts::try_new(
                Duration::from_secs(2),
                Duration::from_secs(2),
                Duration::from_secs(5),
                Duration::from_secs(10),
            )?,
            UpstreamProxy::Direct,
            NonZeroUsize::new(1).ok_or("pool")?,
        ),
    };
    Ok(Arc::new(FragmentTransport {
        inner: Arc::new(inner),
        size,
    }))
}

fn listener(fixture: &Fixture) -> Result<(String, actix_web::dev::ServerHandle), Box<dyn Error>> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let state = fixture.state.clone();
    let server = actix_web::HttpServer::new(move || {
        App::new()
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
    Ok((format!("http://{address}/v1/responses"), handle))
}

async fn request(
    fixture: &Fixture,
    url: &str,
    body: &Value,
) -> Result<(u16, Vec<u8>), Box<dyn Error>> {
    let reply = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()?
        .post(url)
        .bearer_auth(&fixture.key)
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await?;
    let status = reply.status().as_u16();
    Ok((status, reply.bytes().await?.to_vec()))
}

fn terminal(bytes: &[u8], streaming: bool) -> Result<Value, Box<dyn Error>> {
    if !streaming {
        return Ok(serde_json::from_slice(bytes)?);
    }
    let frames = public_frames(bytes)?;
    assert_eq!(
        frames
            .iter()
            .filter(|v| v["type"] == "response.completed")
            .count(),
        1
    );
    assert!(frames.iter().all(|v| v["type"] != "response.failed"));
    Ok(frames.last().ok_or("terminal")?["response"].clone())
}

#[actix_web::test]
async fn public_build_namespace_json_and_fragmented_sse_keep_two_related_cycles() -> TestResult {
    for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
        let peer = ModelPeer::start_sequence(
            streaming,
            (0..5)
                .map(|round| wire(round, streaming))
                .collect::<Result<_, _>>()?,
        )?;
        let fixture = lifecycle_fixture_qualified(transport(&peer, size)?, true)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = listener(&fixture)?;
        let mut history =
            vec![json!({"role":"user","content":"Read seed 41, then calculate its successor."})];
        let mut public = Vec::new();
        for round in 0..5 {
            let body = json!({"model":"p12-test-model","input":history,"stream":streaming,"tools":declarations(),
                "tool_choice":"auto","parallel_tool_calls":false,"reasoning":{"summary":"auto"}});
            let decoded = protocol_openai_responses::decode_request(&body.to_string())?;
            gateway_router::ToolExecutionConstraints::from_request(
                &decoded.request,
                ProtocolFormat::OpenAiResponses,
            )?;
            let (status, bytes) = request(&fixture, &url, &body).await?;
            assert_eq!(
                status,
                200,
                "round {round}: {}",
                String::from_utf8_lossy(&bytes)
            );
            let reply = terminal(&bytes, streaming)?;
            public.push(if streaming {
                json!({"round":round,"frames":public_frames(&bytes)?})
            } else {
                json!({"round":round,"response":reply})
            });
            assert_eq!(reply["output"], output(round));
            assert_eq!(reply["usage"]["input_tokens"], 20);
            assert_eq!(reply["usage"]["output_tokens"], 7);
            let sent = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(sent.len(), round + 1);
            assert_eq!(sent[round].body["tools"], declarations());
            assert_eq!(sent[round].body["tool_choice"], "auto");
            assert_eq!(sent[round].body["parallel_tool_calls"], false);
            assert_history(&sent[round].body["input"], &history);
            drop(sent);
            append_history(&mut history, &reply, round)?;
        }
        server.stop(false).await;
        assert_eq!(active_leases(&fixture), 0);
        let database = recording.finish(&fixture, 0).await?;
        assert_recorded(&database, 5).await?;
        let received: Vec<_> = peer
            .requests
            .lock()
            .map_err(|_| "receiver")?
            .iter()
            .map(|row| row.body.clone())
            .collect();
        println!(
            "CPAR62_NAMESPACE_RECEIPT {}",
            json!({"streaming":streaming,"fragment_bytes":size,
            "qualification":"LOCAL_CONTROLLED","activation":"BUILD_ACTIVATION_BLOCKED","requests":received,"public":public,
            "receiver_count":received.len(),"attempt_count":fixture.events.attempts()?,"usage_count":5,"ledger_count":5,"lease_after":active_leases(&fixture)})
        );
    }
    Ok(())
}

fn append_history(history: &mut Vec<Value>, reply: &Value, round: usize) -> TestResult {
    history.extend(reply["output"].as_array().ok_or("output")?.iter().cloned());
    if round == 0 || round == 2 {
        let call = &reply["output"][1];
        let arguments: Value = serde_json::from_str(call["arguments"].as_str().ok_or("args")?)?;
        let result = match (call["namespace"].as_str(), call["name"].as_str()) {
            (Some("read_side"), Some("lookup")) => 41,
            (Some("write_side"), Some("lookup")) => {
                arguments["previous"].as_i64().ok_or("related seed")? + 1
            }
            _ => return Err("wrong full tool identity".into()),
        };
        assert_eq!(result, if round == 0 { 41 } else { 42 });
        let mut result = json!({"type":"function_call_output","call_id":call["call_id"],"output":result.to_string()});
        if round == 0 {
            result["name"] = Value::Null;
            result["namespace"] = Value::Null;
        } else {
            result["name"] = call["name"].clone();
            result["namespace"] = call["namespace"].clone();
        }
        history.push(result);
    } else if round == 1 || round == 3 {
        history.push(json!({"role":"user","content":if round == 1 {"Now use the read seed."} else {"What was the computed result?"}}));
    }
    Ok(())
}

fn assert_history(sent: &Value, history: &[Value]) {
    if let Some(text) = sent.as_str() {
        assert_eq!(history.len(), 1);
        assert_eq!(text, history[0]["content"]);
        return;
    }
    let expected:Vec<_> = history.iter().map(|item| {
        if item["role"]=="user" && item["content"].is_string() {
            json!({"type":"message","role":"user","content":[{"type":"input_text","text":item["content"]}]})
        } else { item.clone() }
    }).collect();
    assert_eq!(sent, &json!(expected));
}

async fn assert_recorded(database: &std::path::Path, count: usize) -> TestResult {
    let rows = SqliteEventStore::open_read_only(database)?.list_events()?;
    for kind in ["request", "attempt", "usage", "finished"] {
        assert_eq!(
            rows.iter()
                .filter(|row| matches!(
                    (kind, row.event()),
                    ("request", GatewayEvent::Request(_))
                        | ("attempt", GatewayEvent::Attempt(_))
                        | ("usage", GatewayEvent::Usage(_))
                        | ("finished", GatewayEvent::RequestFinished(_))
                ))
                .count(),
            count,
            "{kind}"
        );
    }
    let worker = crate::billing_worker::BillingWorker::start(
        database.to_owned(),
        Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
    );
    worker.stop().await.map_err(|()| "billing")?;
    let ledger = SqliteBillingLedger::open_read_only(database)?.list_bounded(20)?;
    assert_eq!(ledger.len(), count);
    assert!(ledger.iter().all(|v| v.usage.input_tokens == Some(20)
        && v.usage.output_tokens == Some(7)
        && v.cost_microunits.is_none()));
    Ok(())
}
