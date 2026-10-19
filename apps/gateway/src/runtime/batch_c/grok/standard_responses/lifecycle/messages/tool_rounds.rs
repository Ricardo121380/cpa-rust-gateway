//! The client replays only standard Messages content; no helper restores hidden summary.
use super::*;

fn output(round: usize) -> Value {
    let mut items = super::super::super::tool_rounds::output(round)
        .as_array()
        .cloned()
        .unwrap_or_default();
    items[0]["summary"] = json!([{"type":"summary_text","text":format!("计划 {round}")},
        {"type":"summary_text","text":format!("辅助 🐦 {round}")}]);
    items.push(json!({"id":format!("reason-b-{round}"),"type":"reasoning","status":"completed","content":[],
        "summary":[{"type":"summary_text","text":format!("核对 {round}")}]}));
    json!(items)
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed interleaved wire fixture keeps native part/item order auditable"
)]
pub(super) fn wire(round: usize, streaming: bool) -> Result<String, Box<dyn Error>> {
    let items = output(round);
    let items = items.as_array().ok_or("items")?;
    let mut terminal = response();
    terminal["usage"]["input_tokens_details"] = json!({"cached_tokens":0});
    terminal["output"] = json!(items);
    if !streaming {
        return Ok(terminal.to_string());
    }
    let mut bytes = frame("response.created", &created());
    for (index, item) in items.iter().enumerate() {
        let mut item = item.clone();
        item["status"] = json!("in_progress");
        match item["type"].as_str() {
            Some("reasoning") => {
                item["summary"] = json!([]);
            }
            Some("function_call") => {
                item["arguments"] = json!("");
            }
            _ => {
                item["content"] = json!([]);
            }
        }
        bytes += &native_event(
            "response.output_item.added",
            json!({"output_index":index,"item":item}),
        );
    }
    let a = items[0]["id"].as_str().ok_or("id")?;
    bytes += &native_event(
        "response.reasoning_summary_part.added",
        json!({"item_id":a,"output_index":0,"summary_index":0,"part":{"type":"summary_text","text":""}}),
    );
    bytes += &native_event(
        "response.reasoning_summary_text.delta",
        json!({"item_id":a,"output_index":0,"summary_index":0,"delta":"计"}),
    );
    let tool = items[1]["type"] == "function_call";
    let value = if tool {
        items[1]["arguments"].as_str()
    } else {
        items[1]["content"][0]["text"].as_str()
    }
    .ok_or("text")?;
    let split = value
        .char_indices()
        .nth(2)
        .map_or(value.len(), |(index, _)| index);
    let kind = if tool {
        "response.function_call_arguments.delta"
    } else {
        "response.output_text.delta"
    };
    bytes += &native_event(
        kind,
        json!({"item_id":items[1]["id"],"output_index":1,"delta":&value[..split]}),
    );
    for (item_index, part_index) in [(0, 1), (2, 0)] {
        let id = items[item_index]["id"].as_str().ok_or("id")?;
        let part = &items[item_index]["summary"][part_index];
        bytes += &native_event(
            "response.reasoning_summary_part.added",
            json!({"item_id":id,"output_index":item_index,"summary_index":part_index,"part":{"type":"summary_text","text":""}}),
        );
        bytes += &native_event(
            "response.reasoning_summary_text.delta",
            json!({"item_id":id,"output_index":item_index,"summary_index":part_index,"delta":part["text"]}),
        );
        bytes += &native_event(
            "response.reasoning_summary_part.done",
            json!({"item_id":id,"output_index":item_index,"summary_index":part_index,"part":part}),
        );
    }
    bytes += &native_event(
        kind,
        json!({"item_id":items[1]["id"],"output_index":1,"delta":&value[split..]}),
    );
    bytes += &native_event(
        "response.reasoning_summary_text.delta",
        json!({"item_id":a,"output_index":0,"summary_index":0,"delta":format!("划 {round}")}),
    );
    bytes += &native_event(
        "response.reasoning_summary_part.done",
        json!({"item_id":a,"output_index":0,"summary_index":0,"part":items[0]["summary"][0]}),
    );
    let mut done = json!({"item_id":items[1]["id"],"output_index":1});
    done[if tool { "arguments" } else { "text" }] = json!(value);
    bytes += &native_event(
        if tool {
            "response.function_call_arguments.done"
        } else {
            "response.output_text.done"
        },
        done,
    );
    for index in [2, 1, 0] {
        bytes += &native_event(
            "response.output_item.done",
            json!({"output_index":index,"item":items[index]}),
        );
    }
    bytes += &frame("response.completed", &terminal);
    Ok(bytes)
}

pub(super) fn client_result(bytes: &[u8], streaming: bool) -> Result<Value, Box<dyn Error>> {
    if !streaming {
        return Ok(serde_json::from_slice(bytes)?);
    }
    let frames = public_frames(bytes)?;
    assert_eq!(
        frames
            .iter()
            .filter(|v| v["type"] == "message_start")
            .count(),
        1
    );
    assert_eq!(
        frames
            .iter()
            .filter(|v| v["type"] == "message_delta")
            .count(),
        1
    );
    assert_eq!(
        frames
            .iter()
            .filter(|v| v["type"] == "message_stop")
            .count(),
        1,
        "{}",
        String::from_utf8_lossy(bytes)
    );
    assert!(frames.iter().all(|v| v["type"] != "error"));
    assert_eq!(frames[0]["message"]["model"], "grok-4.5");
    assert!(
        frames
            .iter()
            .filter(|v| v["type"] == "content_block_delta")
            .count()
            >= 2
    );
    let mut decoder = provider_anthropic_compatible::AnthropicMessagesSseDecoder::new();
    let mut events = Vec::new();
    for chunk in bytes.chunks(3) {
        decoder.push_chunk(chunk)?;
        loop {
            decoder.drain_buffered_frames()?;
            if let Some(event) = decoder.take_event() {
                events.push(event);
            } else {
                break;
            }
        }
    }
    decoder.finish()?;
    gateway_core::CanonicalResponse::try_new(events)?;
    let mut result = frames[0]["message"].clone();
    let mut content = Vec::<Value>::new();
    let mut arguments = BTreeMap::<usize, String>::new();
    for frame in &frames {
        let index = frame["index"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(0);
        match frame["type"].as_str() {
            Some("content_block_start") => {
                assert_eq!(index, content.len());
                content.push(frame["content_block"].clone());
            }
            Some("content_block_delta") => match frame["delta"]["type"].as_str() {
                Some("text_delta") => {
                    let text = content[index]["text"].as_str().ok_or("text")?.to_owned()
                        + frame["delta"]["text"].as_str().ok_or("delta")?;
                    content[index]["text"] = json!(text);
                }
                Some("input_json_delta") => arguments
                    .entry(index)
                    .or_default()
                    .push_str(frame["delta"]["partial_json"].as_str().ok_or("arguments")?),
                _ => return Err("unexpected client block".into()),
            },
            Some("content_block_stop") if content[index]["type"] == "tool_use" => {
                content[index]["input"] =
                    serde_json::from_str(arguments.get(&index).ok_or("argument fragments")?)?;
            }
            Some("message_delta") => {
                result["stop_reason"] = frame["delta"]["stop_reason"].clone();
                result["stop_sequence"] = frame["delta"]["stop_sequence"].clone();
                result["usage"] = frame["usage"].clone();
            }
            _ => {}
        }
    }
    result["content"] = json!(content);
    Ok(result)
}

#[actix_web::test]
async fn public_build_messages_json_and_sse_keep_two_standard_tool_cycles() -> TestResult {
    let mut json_results = None;
    for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
        let peer = ModelPeer::start_sequence(
            streaming,
            (0..5)
                .map(|round| wire(round, streaming))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let fixture = lifecycle_fixture(MessagesTransport::new(&peer, size)?)?;
        let recording = Recording::start(&fixture, false)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let mut history = vec![json!({"role":"user","content":"question"})];
        let mut results = Vec::new();
        for round in 0..5 {
            let body = json!({"model":"p12-test-model","max_tokens":19,"stream":streaming,"messages":history,
                "tools":[{"name":"lookup","input_schema":{"type":"object"}}]});
            let reply = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/messages")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(body)
                    .to_request(),
            )
            .await;
            let status = reply.status();
            let request_id = reply
                .headers()
                .get("x-request-id")
                .ok_or("id")?
                .to_str()?
                .to_owned();
            let bytes = test::read_body(reply).await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(&bytes)
            );
            assert!(!String::from_utf8_lossy(&bytes).contains("计划"));
            assert!(!String::from_utf8_lossy(&bytes).contains("thinking"));
            let result = client_result(&bytes, streaming)?;
            assert_eq!(result["model"], "grok-4.5");
            assert_eq!(result["usage"]["input_tokens"], 20);
            assert_eq!(result["usage"]["output_tokens"], 7);
            let tool = round == 0 || round == 2;
            if tool {
                assert_eq!(result["stop_reason"], "tool_use");
                assert_eq!(result["content"][0]["id"], format!("call-{round}"));
                assert_eq!(result["content"][0]["name"], "lookup");
                assert_eq!(
                    result["content"][0]["input"],
                    json!({"city":"上海","n":9_007_199_254_740_993_u64})
                );
            } else {
                assert_eq!(result["content"][0]["text"], format!("回答 {round}"));
                assert_eq!(result["stop_reason"], "end_turn");
            }
            let record = retained(&recording.database, &request_id)?;
            let summaries = record.evidence.items();
            assert_eq!(summaries.len(), 2);
            for (entry, index) in summaries.iter().zip([0, 2]) {
                assert_eq!(entry.output_index, index);
                assert_eq!(
                    serde_json::from_str::<Value>(entry.item.get())?,
                    output(round)[index]
                );
            }
            assert_eq!(
                record.evidence.completeness(),
                gateway_core::MessagesSummaryCompleteness::CompletedResponse
            );
            history.push(json!({"role":"assistant","content":result["content"]}));
            if round < 4 {
                history.push(if tool {json!({"role":"user","content":[{"type":"tool_result","tool_use_id":format!("call-{round}"),"content":format!("result-{round}")}]})}
                else {json!({"role":"user","content":if round==1 {"next-question"} else {"follow-up"}})});
            }
            let records = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(records.len(), round + 1);
            let request = &records[round];
            assert_eq!(request.body["model"], "grok-4.5");
            assert_eq!(request.model_override, "grok-4.5");
            assert_eq!(request.body["max_output_tokens"], 19);
            let history_wire = request.body["input"].to_string();
            assert!(
                !history_wire.contains("reason-")
                    && !history_wire.contains("计划")
                    && !history_wire.contains("summary")
            );
            if round == 1 || round == 3 {
                let previous = round - 1;
                let native = request.body["input"].as_array().ok_or("native history")?;
                assert!(native.iter().any(|v| v["type"] == "function_call"
                    && v["call_id"] == format!("call-{previous}")
                    && v["name"] == "lookup"));
                assert!(native.iter().any(|v| v["type"] == "function_call_output"
                    && v["call_id"] == format!("call-{previous}")
                    && v["output"] == format!("result-{previous}")));
            }
            results.push(result);
        }
        if let Some(expected) = &json_results {
            assert_eq!(&results, expected);
        } else {
            json_results = Some(results);
        }
        drop(app);
        let database = recording.finish(&fixture, 0).await?;
        accounting(&database, 5).await?;
        assert_eq!(active_leases(&fixture), 0);
    }
    Ok(())
}
