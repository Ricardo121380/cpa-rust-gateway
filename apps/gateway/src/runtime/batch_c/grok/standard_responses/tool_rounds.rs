//! Existing standard semantics only: no field qualification or metadata separation is implied.
use super::*;

struct Fragments {
    peer: Arc<ModelPeer>,
    size: usize,
}

struct FragmentBody {
    inner: Box<dyn GrokBuildResponseBody>,
    pending: Vec<u8>,
    size: usize,
}

impl GrokBuildResponseBody for FragmentBody {
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

impl GrokBuildTransport for Fragments {
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
                Box::new(FragmentBody {
                    inner: body,
                    pending: Vec::new(),
                    size: self.size,
                }),
            ))
        })
    }
}

fn output(round: usize) -> Value {
    let summary = json!({"id":format!("reason-{round}"),"type":"reasoning","status":"completed",
        "summary":[{"type":"summary_text","text":format!("计划 {round}")} ]});
    let item = if round == 0 || round == 2 {
        json!({"id":format!("function-{round}"),"type":"function_call","status":"completed",
            "call_id":format!("call-{round}"),"name":"lookup",
            "arguments":r#"{"city":"上海","n":9007199254740993}"#})
    } else {
        json!({"id":format!("message-{round}"),"type":"message","status":"completed","role":"assistant",
            "content":[{"type":"output_text","text":format!("回答 {round}"),"annotations":[]}]})
    };
    json!([summary, item])
}

fn event(kind: &str, mut value: Value) -> String {
    value["type"] = json!(kind);
    format!("event: {kind}\ndata: {value}\n\n")
}

pub(super) fn wire_response(round: usize, streaming: bool) -> Result<String, Box<dyn Error>> {
    let output = output(round);
    let mut terminal = response();
    terminal["output"] = output.clone();
    if !streaming {
        return Ok(terminal.to_string());
    }
    let mut wire = frame("response.created", &created());
    let items = output.as_array().ok_or("output")?;
    for (index, item) in items.iter().enumerate() {
        let mut added = item.clone();
        added["status"] = json!("in_progress");
        if index == 0 {
            added["summary"] = json!([]);
        } else if round == 0 || round == 2 {
            added["arguments"] = json!("");
        } else {
            added["content"] = json!([]);
        }
        wire += &event(
            "response.output_item.added",
            json!({"output_index":index,"item":added}),
        );
    }
    let summary_id = format!("reason-{round}");
    wire += &event(
        "response.reasoning_summary_text.delta",
        json!({"item_id":summary_id,"output_index":0,"summary_index":0,"delta":"计"}),
    );
    let tool = round == 0 || round == 2;
    let id = items[1]["id"].as_str().ok_or("item")?;
    let (kind, field, value) = if tool {
        (
            "response.function_call_arguments.delta",
            "arguments",
            items[1]["arguments"].as_str().ok_or("arguments")?,
        )
    } else {
        (
            "response.output_text.delta",
            "text",
            items[1]["content"][0]["text"].as_str().ok_or("text")?,
        )
    };
    let split = value
        .char_indices()
        .nth(3)
        .map_or(value.len(), |(index, _)| index);
    for (index, delta) in [&value[..split], &value[split..]].iter().enumerate() {
        if index == 1 {
            wire += &event(
                "response.reasoning_summary_text.delta",
                json!({"item_id":summary_id,"output_index":0,"summary_index":0,"delta":format!("划 {round}")}),
            );
        }
        wire += &event(kind, json!({"item_id":id,"output_index":1,"delta":delta}));
    }
    let mut done = json!({"item_id":id,"output_index":1});
    done[field] = json!(value);
    wire += &event(
        if tool {
            "response.function_call_arguments.done"
        } else {
            "response.output_text.done"
        },
        done,
    );
    for (index, item) in items.iter().enumerate() {
        wire += &event(
            "response.output_item.done",
            json!({"output_index":index,"item":item}),
        );
    }
    wire += &frame("response.completed", &terminal);
    Ok(wire)
}

#[actix_web::test]
async fn public_build_standard_json_and_fragmented_sse_keep_two_tool_cycles() -> TestResult {
    run_tool_rounds(None).await
}

#[actix_web::test]
async fn public_build_summary_only_json_and_fragmented_sse_keep_two_tool_cycles() -> TestResult {
    run_tool_rounds(Some(json!({"summary":"auto"}))).await
}

fn round_result(bytes: &[u8], streaming: bool, round: usize) -> Result<Value, Box<dyn Error>> {
    if !streaming {
        return Ok(serde_json::from_slice(bytes)?);
    }
    let frames = public_frames(bytes)?;
    assert_eq!(
        frames
            .iter()
            .filter(|v| v["type"] == "response.completed")
            .count(),
        1,
        "{}",
        String::from_utf8_lossy(bytes)
    );
    if round == 0 || round == 2 {
        let done = frames
            .iter()
            .find(|v| v["type"] == "response.function_call_arguments.done")
            .ok_or("complete executable arguments")?;
        assert_eq!(done["arguments"], r#"{"city":"上海","n":9007199254740993}"#);
        serde_json::from_str::<Value>(done["arguments"].as_str().ok_or("arguments")?)?;
    }
    Ok(frames.last().ok_or("terminal")?["response"].clone())
}

async fn run_tool_rounds(reasoning: Option<Value>) -> TestResult {
    for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
        let responses = (0..5)
            .map(|round| wire_response(round, streaming))
            .collect::<Result<Vec<_>, _>>()?;
        let peer = Arc::new(ModelPeer::start_sequence(streaming, responses)?);
        let transport = Arc::new(Fragments {
            peer: peer.clone(),
            size,
        });
        let (channel, secret) = native_channels()[0];
        let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.build = Some(transport);
            endpoint.transports = Arc::new(profiles);
            Ok(())
        })?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let mut history = vec![json!({"role":"user","content":"question"})];
        for round in 0..5 {
            let mut body = json!({"model":"p12-test-model","input":history,"stream":streaming,"max_output_tokens":19,
                "tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]});
            if let Some(reasoning) = &reasoning {
                body["reasoning"] = reasoning.clone();
            }
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(body)
                    .to_request(),
            )
            .await;
            let status = reply.status();
            let bytes = test::read_body(reply).await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(&bytes)
            );
            let result = round_result(&bytes, streaming, round)?;
            assert_eq!(result["output"], output(round));
            assert_eq!(result["model"], "grok-4.5");
            assert_eq!(result["usage"]["input_tokens"], 20);
            assert_eq!(result["usage"]["output_tokens"], 7);
            history.extend(result["output"].as_array().ok_or("output")?.clone());
            history.push(if round==0 || round==2 {
                json!({"type":"function_call_output","call_id":format!("call-{round}"),"output":format!("result-{round}")})
            } else { json!({"role":"user","content":if round==1 {"next-question"} else {"follow-up"}}) });
            let records = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(records.len(), round + 1);
            assert_eq!(records[round].body["model"], "grok-4.5");
            assert_eq!(records[round].model_override, "grok-4.5");
            assert_eq!(records[round].body["max_output_tokens"], 19);
            assert_eq!(records[round].body.get("reasoning"), reasoning.as_ref());
            if round > 0 {
                assert!(
                    records[round]
                        .body
                        .to_string()
                        .contains(&format!("计划 {}", round - 1))
                );
            }
            if round == 1 || round == 3 {
                assert!(
                    records[round]
                        .body
                        .to_string()
                        .contains(&format!("result-{}", round - 1))
                );
            }
        }
        let events = fixture.events.0.lock().map_err(|_| "events")?;
        assert_eq!(
            events
                .iter()
                .filter(|v| matches!(v, GatewayEvent::Attempt(_)))
                .count(),
            5
        );
        assert_eq!(
            events
                .iter()
                .filter(|v| matches!(v, GatewayEvent::Usage(_)))
                .count(),
            5
        );
    }
    Ok(())
}
