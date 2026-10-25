//! The client replays only standard Messages content; no helper restores hidden summary.
use super::*;
use sha2::{Digest, Sha256};

pub(super) fn output(round: usize) -> Value {
    let original = super::super::super::tool_rounds::output(round);
    let mut items = vec![original[0].clone(), original[1].clone()];
    if round == 2 {
        items[1]["arguments"] =
            json!(r#"{"city":"上海","n":9007199254740993,"prior_result":"result-0"}"#);
    }
    items[0]["summary"] = json!([{"type":"summary_text","text":format!("计划 {round}")},
        {"type":"summary_text","text":format!("辅助 🐦 {round}")}]);
    items.push(json!({"id":format!("reason-b-{round}"),"type":"reasoning","status":"completed","content":[],
        "summary":[{"type":"summary_text","text":format!("核对 {round}")}]}));
    json!(items)
}

pub(super) fn wire(round: usize, streaming: bool) -> Result<String, Box<dyn Error>> {
    wire_output(round, streaming, &output(round))
}

#[allow(
    clippy::too_many_lines,
    reason = "shared closed Messages wire fixture renderer"
)]
pub(super) fn wire_output(
    round: usize,
    streaming: bool,
    items: &Value,
) -> Result<String, Box<dyn Error>> {
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
                if let Some(context) = frame.get("context_management") {
                    result["context_management"] = context.clone();
                }
            }
            _ => {}
        }
    }
    result["content"] = json!(content);
    Ok(result)
}

#[actix_web::test]
async fn public_build_messages_json_and_sse_keep_two_standard_tool_cycles() -> TestResult {
    run_cycles(false, false).await
}

#[allow(
    clippy::too_many_lines,
    reason = "one public dialogue retains ordered native histories, summary receipts and JSON/SSE parity together"
)]
pub(super) async fn run_cycles(local: bool, compatible: bool) -> TestResult {
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
        let (url, server) = namespaces::listener(&fixture)?;
        let mut history = vec![json!({"role":"user","content":"question"})];
        let mut results = Vec::new();
        for round in 0..5 {
            let mut body = json!({"model":"p12-test-model","max_tokens":19,"stream":streaming,"messages":history,
                "tools":[{"name":"lookup","input_schema":{"type":"object"}}]});
            if compatible {
                body["thinking"] = json!({"type":"adaptive","display":"updates"});
                body["output_config"] = json!({"effort":"high"});
                body["tools"][0]["cache_control"] = json!({"type":"ephemeral"});
                for message in body["messages"].as_array_mut().ok_or("messages")? {
                    if let Some(parts) = message["content"].as_array_mut() {
                        for part in parts {
                            part["cache_control"] = json!({"type":"ephemeral"});
                        }
                    }
                }
            }
            let raw_metadata = local_controls::METADATA
                .replace("PRIVATE_SESSION", &format!("PRIVATE_SESSION_{round}"));
            let encoded = if local {
                body["context_management"] = local_controls::keep_all();
                local_controls::with_metadata(&body, &raw_metadata)
            } else {
                body.to_string()
            };
            let reply = reqwest::Client::builder()
                .no_proxy()
                .build()?
                .post(url.replace("/v1/responses", "/v1/messages"))
                .bearer_auth(&fixture.key)
                .header("content-type", "application/json")
                .body(encoded)
                .send()
                .await?;
            let status = reply.status();
            let request_id = reply
                .headers()
                .get("x-request-id")
                .ok_or("id")?
                .to_str()?
                .to_owned();
            let bytes = reply.bytes().await?;
            assert_eq!(
                status,
                reqwest::StatusCode::OK,
                "{}",
                String::from_utf8_lossy(&bytes)
            );
            assert!(!String::from_utf8_lossy(&bytes).contains("计划"));
            assert!(!String::from_utf8_lossy(&bytes).contains("thinking"));
            let result = client_result(&bytes, streaming)?;
            local_controls::assert_context(&bytes, streaming, local)?;
            assert!(!String::from_utf8_lossy(&bytes).contains("M4_PRIVATE"));
            if local {
                local_controls::assert_metadata(&fixture, &request_id, &raw_metadata, 1)?;
            }
            assert_eq!(result["model"], "grok-4.5");
            assert_eq!(result["usage"]["input_tokens"], 20);
            assert_eq!(result["usage"]["output_tokens"], 7);
            let tool = round == 0 || round == 2;
            if tool {
                assert_eq!(result["stop_reason"], "tool_use");
                assert_eq!(result["content"][0]["id"], format!("call-{round}"));
                assert_eq!(result["content"][0]["name"], "lookup");
                let mut expected = json!({"city":"上海","n":9_007_199_254_740_993_u64});
                if round == 2 {
                    expected["prior_result"] = json!("result-0");
                }
                assert_eq!(result["content"][0]["input"], expected);
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
                // The second controlled function consumes the first result from arguments
                // actually delivered to this standard client, without any private lookup.
                let tool_output = if round == 2 {
                    format!(
                        "{} -> result-2",
                        result["content"][0]["input"]["prior_result"]
                            .as_str()
                            .ok_or("prior result")?
                    )
                } else {
                    format!("result-{round}")
                };
                history.push(if tool {json!({"role":"user","content":[{"type":"tool_result","tool_use_id":format!("call-{round}"),"content":tool_output}]})}
                else {json!({"role":"user","content":if round==1 {"next-question"} else {"follow-up"}})});
            }
            let records = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(records.len(), round + 1);
            let request = &records[round];
            assert_eq!(request.body["model"], "grok-4.5");
            assert_eq!(request.model_override, "grok-4.5");
            assert_eq!(request.body["max_output_tokens"], 19);
            if compatible {
                assert_eq!(request.body["reasoning"], json!({"effort":"high"}));
                assert!(!request.body.to_string().contains("cache_control"));
            }
            assert_eq!(request.body["input"], local_controls::expected_input(round));
            assert!(
                request.body.get("metadata").is_none()
                    && request.body.get("context_management").is_none()
            );
            assert!(!String::from_utf8_lossy(&request.wire).contains("M4_PRIVATE"));
            let history_wire = request.body["input"].to_string();
            assert!(
                !history_wire.contains("reason-")
                    && !history_wire.contains("计划")
                    && !history_wire.contains("summary")
            );
            for previous in [0, 2].into_iter().filter(|previous| round > *previous) {
                let native = request.body["input"].as_array().ok_or("native history")?;
                let call = native
                    .iter()
                    .find(|v| {
                        v["type"] == "function_call"
                            && v["call_id"] == format!("call-{previous}")
                            && v["name"] == "lookup"
                    })
                    .ok_or("retained historical call")?;
                assert_eq!(
                    serde_json::from_str::<Value>(call["arguments"].as_str().ok_or("arguments")?)?,
                    serde_json::from_str::<Value>(
                        output(previous)[1]["arguments"]
                            .as_str()
                            .ok_or("expected arguments")?
                    )?
                );
                assert!(native.iter().any(|v| v["type"] == "function_call_output"
                    && v["call_id"] == format!("call-{previous}")
                    && v["output"]
                        == if previous == 2 {
                            "result-0 -> result-2"
                        } else {
                            "result-0"
                        }));
            }
            if round == 4 {
                assert!(
                    history_wire.contains("回答 1")
                        && history_wire.contains("回答 3")
                        && history_wire.contains("follow-up")
                );
            }
            let summary_items = record
                .evidence
                .items()
                .iter()
                .map(|item| {
                    let value: Value = serde_json::from_str(item.item.get())?;
                    Ok(
                        json!({"output_index":item.output_index,"item_id":value["id"],
                    "part_count":value["summary"].as_array().ok_or("summary")?.len(),
                    "body_present":value.get("content").is_some(),
                    "item_sha256":format!("{:x}",Sha256::digest(item.item.get().as_bytes()))}),
                    )
                })
                .collect::<Result<Vec<Value>, Box<dyn Error>>>()?;
            let (key_version, ciphertext): (i64, Vec<u8>) = gateway_store::open(
                &recording.database,
            )?
            .query_row(
                "SELECT key_version,ciphertext FROM messages_summary_evidence WHERE attempt_id=?1",
                [record.attempt.attempt_id().as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            eprintln!(
                "CPAR63_RECEIPT {}",
                json!({"synthetic":true,"local_m3_m4":local,"metadata_raw_readback":local,"context_kept":local,"streaming":streaming,"chunk_bytes":size,"round":round,
                "request_id":request_id,"attempt_id":record.attempt.attempt_id(),"owner":record.client_key_id,
                "response_id":record.evidence.response_id(),"source":record.evidence.source(),
                "completeness":record.evidence.completeness(),"summary_items":summary_items,"reader_authenticated":true,
                "key_version":key_version,"ciphertext_bytes":ciphertext.len(),"ciphertext_sha256":format!("{:x}",Sha256::digest(&ciphertext)),
                "public_model":result["model"],"received_upstream_model":request.body["model"],"model_override":request.model_override,
                "input_tokens":result["usage"]["input_tokens"],"output_tokens":result["usage"]["output_tokens"],
                "receive_count":records.len(),"visible_history_only_verified":true,"cost":"UNKNOWN"})
            );
            results.push(result);
        }
        if let Some(expected) = &json_results {
            assert_eq!(&results, expected);
        } else {
            json_results = Some(results);
        }
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        accounting(&database, 5).await?;
        let joins = super::super::cli::assert_accounting(&database, "messages", "standard")?;
        eprintln!(
            "CPAR64_LOCAL_JOINS {}",
            json!({"local_m3_m4":local,"streaming":streaming,"fragment_bytes":size,"joins":joins})
        );
        assert_eq!(active_leases(&fixture), 0);
        eprintln!(
            "CPAR63_ACCOUNTING {}",
            json!({"synthetic":true,"local_m3_m4":local,"streaming":streaming,"chunk_bytes":size,
            "receives":peer.requests.lock().map_err(|_|"receiver")?.len(),"requests":5,"attempts":5,"usage_rows":5,
            "ledger_rows":5,"active_leases":active_leases(&fixture),"cost":"UNKNOWN"})
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_cpa_controls_keep_two_tool_cycles() -> TestResult {
    run_cycles(true, true).await
}
