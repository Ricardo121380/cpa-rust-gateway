//! Native metadata passes through the actual projection and public Responses codecs.
use gateway_core::{CanonicalEvent, CanonicalResponse};
use gateway_router::{ProtocolFormat, project_protocol_response};
use protocol_openai_responses::{
    OpenAiResponseMetadata, OpenAiResponsesSseEncoder, decode_request, encode_response,
};
use provider_grok::{
    GrokBuildResponsesStreamDecoder, GrokConsoleResponsesDecoder,
    GrokConsoleResponsesStreamDecoder, GrokOfficialResponsesDecoder,
    GrokOfficialResponsesStreamDecoder,
};
use provider_grok::{
    GrokConsoleResponsesRequestBuilder, GrokConsoleSsoToken, GrokOfficialApiKey,
    GrokOfficialResponsesRequestBuilder,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn native_response() -> Value {
    json!({"id":"resp-c-metadata","status":"completed","output":[
        {"id":"reason-c","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"short"},{"type":"summary_text","text":"second"}],"content":[{"type":"reasoning_text","text":"body"}]},
        {"id":"msg-c","type":"message","role":"assistant","status":"completed","phase":"final_answer","content":[{"type":"output_text","text":"answer","annotations":[{"type":"url_citation","url":"https://example.test/source","title":"Source","start_index":0,"end_index":6}],"logprobs":null},{"type":"output_text","text":"tail","annotations":[]}]}
    ]})
}

fn record(event: &Value) -> String {
    format!(
        "event: {}\ndata: {event}\n\n",
        event["type"].as_str().unwrap_or("invalid")
    )
}

fn native_events(response: &Value) -> Vec<Value> {
    let mut events = vec![json!({"type":"response.created","response":{"id":response["id"]}})];
    for (index, item) in response["output"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let mut start = item.clone();
        start["status"] = json!("in_progress");
        for field in ["summary", "content"] {
            if start.get(field).is_some() {
                start[field] = json!([]);
            }
        }
        events.push(json!({"type":"response.output_item.added","output_index":index,"item":start}));
        for field in ["summary", "content"] {
            for (part_index, part) in item[field].as_array().into_iter().flatten().enumerate() {
                let kind = if field == "summary" {
                    "response.reasoning_summary_text.delta"
                } else if item["type"] == "reasoning" {
                    "response.reasoning_text.delta"
                } else {
                    "response.output_text.delta"
                };
                let index_key = if field == "summary" {
                    "summary_index"
                } else {
                    "content_index"
                };
                let mut event = json!({"type":kind,"item_id":item["id"],"output_index":index,"delta":part["text"]});
                event[index_key] = json!(part_index);
                events.push(event);
            }
        }
        events.push(json!({"type":"response.output_item.done","output_index":index,"item":item}));
    }
    events.push(json!({"type":"response.completed","response":response}));
    events
}

fn events_with_parts(response: &Value, initial_annotations: bool) -> Vec<Value> {
    let mut events = Vec::new();
    for event in native_events(response) {
        if matches!(
            event["type"].as_str(),
            Some(
                "response.output_text.delta"
                    | "response.reasoning_text.delta"
                    | "response.reasoning_summary_text.delta"
            )
        ) {
            let output_index = event["output_index"].clone();
            let field = if event.get("summary_index").is_some() {
                "summary"
            } else {
                "content"
            };
            let index_key = if field == "summary" {
                "summary_index"
            } else {
                "content_index"
            };
            let part_index = event[index_key]
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0);
            let item_index = output_index
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0);
            let part = response["output"][item_index][field][part_index].clone();
            let mut start_part = part.clone();
            start_part["text"] = json!("");
            if start_part.get("annotations").is_some() && !initial_annotations {
                start_part["annotations"] = json!([]);
            }
            let part_kind = if field == "summary" {
                "response.reasoning_summary_part"
            } else {
                "response.content_part"
            };
            let mut added = json!({"type":format!("{part_kind}.added"),"item_id":event["item_id"],"output_index":output_index,"part":start_part});
            added[index_key] = event[index_key].clone();
            events.push(added);
            if !initial_annotations {
                for (annotation_index, annotation) in part["annotations"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    events.push(json!({"type":"response.output_text.annotation.added","item_id":event["item_id"],"output_index":output_index,"content_index":part_index,"annotation_index":annotation_index,"annotation":annotation}));
                }
            }
            events.push(event.clone());
            let mut done = json!({"type":format!("{part_kind}.done"),"item_id":event["item_id"],"output_index":output_index,"part":part});
            done[index_key] = event[index_key].clone();
            events.push(done);
        } else {
            events.push(event);
        }
    }
    events
}

fn decode_stream(
    channel: &str,
    events: &[Value],
    chunk_size: usize,
) -> Result<CanonicalResponse, gateway_core::GatewayError> {
    let wire = events.iter().map(record).collect::<String>();
    let mut output = Vec::new();
    match channel {
        "official" => {
            let mut decoder = GrokOfficialResponsesStreamDecoder::new();
            for chunk in wire.as_bytes().chunks(chunk_size) {
                output.extend(decoder.push_bytes(chunk)?);
            }
            decoder.finish()?;
        }
        "console" => {
            let mut decoder = GrokConsoleResponsesStreamDecoder::new();
            for chunk in wire.as_bytes().chunks(chunk_size) {
                output.extend(decoder.push_bytes(chunk)?);
            }
            decoder.finish()?;
        }
        _ => {
            let mut decoder = GrokBuildResponsesStreamDecoder::new();
            for chunk in wire.as_bytes().chunks(chunk_size) {
                output.extend(decoder.push_bytes(chunk)?);
            }
            decoder.finish()?;
        }
    }
    CanonicalResponse::try_new(output)
}

#[test]
fn official_console_preserve_native_items_parts_phase_and_citations() -> TestResult {
    let upstream = native_response();
    let events = native_events(&upstream);
    for channel in ["official", "console"] {
        let json_source = if channel == "official" {
            GrokOfficialResponsesDecoder::decode_non_streaming(upstream.to_string().as_bytes())?
        } else {
            GrokConsoleResponsesDecoder::decode_non_streaming(upstream.to_string().as_bytes())?
        };
        let sources = [
            json_source,
            decode_stream(channel, &events, 1)?,
            decode_stream(channel, &events, 7)?,
            decode_stream(channel, &events, 4096)?,
        ];
        for source in sources {
            let projected = project_protocol_response(&source, ProtocolFormat::OpenAiResponses)?;
            let metadata = OpenAiResponseMetadata::try_new("grok-c", 0)?;
            let encoded = encode_response(&projected, metadata.clone())?;
            let expected = upstream["output"].clone();
            assert_eq!(encoded["output"], expected, "{channel}");
            let mut stream_encoder = OpenAiResponsesSseEncoder::new(metadata);
            let mut frames = Vec::new();
            for event in projected.events() {
                frames.extend(stream_encoder.encode_event(event)?);
            }
            assert_eq!(
                frames.last().ok_or("terminal")?.data()["response"]["output"],
                expected
            );
            assert_eq!(
                frames
                    .iter()
                    .filter(|frame| frame.event() == "response.completed")
                    .count(),
                1
            );
            let replay =
                decode_request(&json!({"model":"grok-c","input":encoded["output"]}).to_string())?;
            let outbound = if channel == "official" {
                GrokOfficialResponsesRequestBuilder::build(
                    &GrokOfficialApiKey::try_new("synthetic-c-metadata-key")?,
                    "grok-4.3",
                    &replay.request,
                    protocol_openai_responses::ResponseMode::NonStreaming,
                )?
                .body()
                .to_vec()
            } else {
                GrokConsoleResponsesRequestBuilder::build(
                    &GrokConsoleSsoToken::try_from_bytes(b"synthetic-c-metadata-sso")?,
                    "grok-4.3",
                    &replay.request,
                    protocol_openai_responses::ResponseMode::NonStreaming,
                )?
                .body()
                .to_vec()
            };
            let outbound: Value = serde_json::from_slice(&outbound)?;
            assert_eq!(
                outbound["input"], encoded["output"],
                "native history lost for {channel}"
            );
            assert!(
                project_protocol_response(&source, ProtocolFormat::OpenAiChatCompletions).is_err()
            );
            assert!(project_protocol_response(&source, ProtocolFormat::AnthropicMessages).is_err());
            assert_eq!(
                serde_json::from_str::<Vec<CanonicalEvent>>(&serde_json::to_string(
                    source.events()
                )?)?,
                source.events()
            );
        }
    }
    Ok(())
}

#[test]
fn native_event_only_metadata_and_correlation_cannot_disappear() {
    let plain = json!({"id":"resp-c-metadata","status":"completed","output":[{"id":"msg-c","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"answer"}]}]});
    let events = native_events(&plain);
    for channel in ["official", "console", "build"] {
        for (field, value) in [
            ("logprobs", json!([{"token":"answer","logprob":-0.5}])),
            ("signature", json!("synthetic-signature")),
            ("citations", json!([{"url":"https://example.test"}])),
            ("output_index", json!(7)),
        ] {
            let mut corrupted = events.clone();
            corrupted[2][field] = value;
            assert!(
                decode_stream(channel, &corrupted, 7).is_err(),
                "{channel} lost {field}"
            );
        }
    }
}

#[test]
fn official_console_plain_reasoning_keeps_its_existing_bridges() -> TestResult {
    let response = json!({"id":"resp-plain","status":"completed","output":[{"id":"reason-plain","type":"reasoning","status":"completed","content":[{"type":"reasoning_text","text":"thought"}]}]});
    for channel in ["official", "console"] {
        let source = decode_stream(channel, &native_events(&response), 7)?;
        for target in [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::AnthropicMessages,
        ] {
            let projected = project_protocol_response(&source, target)?;
            let thought = projected
                .events()
                .iter()
                .filter_map(|event| match event {
                    CanonicalEvent::ReasoningDelta(delta) => Some(delta.text.as_str()),
                    _ => None,
                })
                .collect::<String>();
            assert_eq!(thought, "thought");
        }
    }
    Ok(())
}

#[test]
fn incremental_annotations_and_indexed_parts_survive_all_native_channels() -> TestResult {
    let response = native_response();
    for channel in ["official", "console", "build"] {
        for initial_annotations in [false, true] {
            let source = decode_stream(
                channel,
                &events_with_parts(&response, initial_annotations),
                1,
            )?;
            let projected = project_protocol_response(&source, ProtocolFormat::OpenAiResponses)?;
            let metadata = OpenAiResponseMetadata::try_new("grok-c", 0)?;
            assert_eq!(
                encode_response(&projected, metadata.clone())?["output"],
                response["output"]
            );
            let mut encoder = OpenAiResponsesSseEncoder::new(metadata);
            let mut frames = Vec::new();
            for event in projected.events() {
                frames.extend(encoder.encode_event(event)?);
            }
            assert_eq!(
                frames
                    .iter()
                    .filter(|frame| frame.event() == "response.output_text.annotation.added")
                    .count(),
                1
            );
            assert_eq!(
                frames.last().ok_or("terminal")?.data()["response"]["output"],
                response["output"]
            );
        }
    }
    Ok(())
}

#[test]
fn intermediate_snapshots_annotations_and_terminal_order_must_agree() -> TestResult {
    let response = native_response();
    let original = events_with_parts(&response, false);
    for channel in ["official", "console", "build"] {
        for case in [
            "annotation_index",
            "annotation_missing",
            "part_snapshot",
            "part_index",
            "phase",
            "terminal_order",
        ] {
            let mut events = original.clone();
            match case {
                "annotation_index" => {
                    let event = events
                        .iter_mut()
                        .find(|event| event["type"] == "response.output_text.annotation.added")
                        .ok_or("annotation event")?;
                    event["annotation_index"] = json!(1);
                }
                "annotation_missing" => {
                    let event = events
                        .iter_mut()
                        .find(|event| {
                            event["type"] == "response.output_item.done"
                                && event["item"]["id"] == "msg-c"
                        })
                        .ok_or("message end")?;
                    event["item"]["content"][0]["annotations"] = json!([]);
                }
                "part_snapshot" | "part_index" => {
                    let event = events
                        .iter_mut()
                        .find(|event| {
                            event["type"] == "response.content_part.done"
                                && event["item_id"] == "msg-c"
                                && event["content_index"] == 0
                        })
                        .ok_or("part end")?;
                    if case == "part_snapshot" {
                        event["part"]["annotations"][0]["title"] = json!("Different");
                    } else {
                        event["content_index"] = json!(1);
                    }
                }
                "phase" => {
                    let event = events
                        .iter_mut()
                        .find(|event| {
                            event["type"] == "response.output_item.done"
                                && event["item"]["id"] == "msg-c"
                        })
                        .ok_or("message end")?;
                    event["item"]["phase"] = json!("commentary");
                }
                _ => {
                    events.last_mut().ok_or("terminal")?["response"]["output"]
                        .as_array_mut()
                        .ok_or("output array")?
                        .swap(0, 1);
                }
            }
            assert!(
                decode_stream(channel, &events, 7).is_err(),
                "{channel} accepted {case}"
            );
        }
        for field in ["signature", "encrypted_content", "logprobs"] {
            let mut events = original.clone();
            let event = events
                .iter_mut()
                .find(|event| event["type"] == "response.reasoning_summary_text.delta")
                .ok_or("summary delta")?;
            event[field] = json!("synthetic-unreviewed-value");
            assert!(
                decode_stream(channel, &events, 7).is_err(),
                "{channel} lost {field}"
            );
        }
    }
    Ok(())
}

#[test]
fn optional_function_event_ids_confirm_the_started_call() -> TestResult {
    let response = json!({"id":"resp-call","status":"completed","output":[{"id":"item-call","type":"function_call","status":"completed","call_id":"call-c","name":"lookup","arguments":"{\"value\":1}"}]});
    let events = vec![
        json!({"type":"response.created","response":{"id":"resp-call"}}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"id":"item-call","type":"function_call","status":"in_progress","call_id":"call-c","name":"lookup","arguments":""}}),
        json!({"type":"response.function_call_arguments.delta","item_id":"item-call","output_index":0,"delta":"{\"value\":1}"}),
        json!({"type":"response.function_call_arguments.done","item_id":"item-call","output_index":0,"name":"lookup","arguments":"{\"value\":1}"}),
        json!({"type":"response.output_item.done","output_index":0,"item":response["output"][0]}),
        json!({"type":"response.completed","response":response}),
    ];
    for channel in ["official", "console", "build"] {
        let source = decode_stream(channel, &events, 1)?;
        assert_eq!(
            encode_response(&source, OpenAiResponseMetadata::try_new("grok-c", 0)?)?["output"],
            response["output"]
        );
        let mut initial_arguments = events.clone();
        initial_arguments[1]["item"]["arguments"] = response["output"][0]["arguments"].clone();
        initial_arguments.remove(2);
        let source = decode_stream(channel, &initial_arguments, 1)?;
        assert_eq!(
            encode_response(&source, OpenAiResponseMetadata::try_new("grok-c", 0)?)?["output"],
            response["output"]
        );
        for (index, field, value) in [
            (2, "call_id", "wrong-call"),
            (3, "call_id", "wrong-call"),
            (3, "name", "wrong-name"),
        ] {
            let mut corrupted = events.clone();
            corrupted[index][field] = json!(value);
            assert!(
                decode_stream(channel, &corrupted, 7).is_err(),
                "{channel} accepted changed {field}"
            );
        }
    }
    Ok(())
}

#[test]
fn completed_parts_cannot_reopen_and_replace_citations() -> TestResult {
    let response = native_response();
    let mut events = events_with_parts(&response, false);
    events.retain(|event| event["type"] != "response.output_text.annotation.added");
    let index = events
        .iter()
        .position(|event| {
            event["type"] == "response.content_part.done"
                && event["item_id"] == "msg-c"
                && event["content_index"] == 0
        })
        .ok_or("part end")?;
    let confirmation = events[index].clone();
    events[index]["part"]["annotations"][0]["title"] = json!("Earlier citation");
    events.insert(index + 1, json!({"type":"response.content_part.added","item_id":"msg-c","output_index":1,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}}));
    events.insert(index + 2, confirmation);
    for channel in ["official", "console", "build"] {
        assert!(
            decode_stream(channel, &events, 4096).is_err(),
            "{channel} replaced a completed part"
        );
    }
    Ok(())
}

#[test]
fn text_and_reasoning_done_seal_the_part_before_item_completion() {
    for (part_kind, field, delta_kind, done_kind) in [
        (
            "output_text",
            "content",
            "response.output_text.delta",
            "response.output_text.done",
        ),
        (
            "reasoning_text",
            "content",
            "response.reasoning_text.delta",
            "response.reasoning_text.done",
        ),
        (
            "summary_text",
            "summary",
            "response.reasoning_summary_text.delta",
            "response.reasoning_summary_text.done",
        ),
    ] {
        let mut item = if part_kind == "output_text" {
            json!({"id":"part-c","type":"message","role":"assistant","status":"completed"})
        } else {
            json!({"id":"part-c","type":"reasoning","status":"completed"})
        };
        item[field] = json!([{"type":part_kind,"text":"ab"}]);
        let response = json!({"id":"resp-part-c","status":"completed","output":[item]});
        let mut events = native_events(&response);
        events[2]["delta"] = json!("a");
        let index_key = if field == "summary" {
            "summary_index"
        } else {
            "content_index"
        };
        let mut done = json!({"type":done_kind,"item_id":"part-c","output_index":0,"text":"a"});
        done[index_key] = json!(0);
        let mut delta = json!({"type":delta_kind,"item_id":"part-c","output_index":0,"delta":"b"});
        delta[index_key] = json!(0);
        events.insert(3, done);
        events.insert(4, delta);
        for channel in ["official", "console", "build"] {
            assert!(
                decode_stream(channel, &events, 7).is_err(),
                "{channel} appended after {done_kind}"
            );
        }
    }
}

#[test]
fn lifecycle_fields_and_events_after_terminal_cannot_disappear() {
    let response = json!({"id":"resp-life","status":"completed","output":[{"id":"msg-life","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"answer"}]}]});
    let original = native_events(&response);
    for channel in ["official", "console", "build"] {
        for kind in [
            "response.created",
            "response.in_progress",
            "response.completed",
        ] {
            for (field, value) in [
                ("signature", json!("synthetic-signature")),
                ("logprobs", json!([{"token":"answer"}])),
                ("sequence_number", json!("invalid")),
            ] {
                let mut events = original.clone();
                let index = if kind == "response.in_progress" {
                    events.insert(1, json!({"type":kind,"response":{"id":"resp-life"}}));
                    1
                } else if kind == "response.created" {
                    0
                } else {
                    events.len() - 1
                };
                events[index][field] = value;
                assert!(
                    decode_stream(channel, &events, 7).is_err(),
                    "{channel} dropped {kind}.{field}"
                );
            }
        }
        let mut events = original.clone();
        events.push(json!({"type":"response.in_progress","response":{"id":"resp-life"}}));
        assert!(
            decode_stream(channel, &events, 7).is_err(),
            "{channel} accepted lifecycle regression"
        );
    }
}

#[test]
fn initial_item_citations_are_preserved_and_must_be_confirmed() -> TestResult {
    let response = native_response();
    let mut events = native_events(&response);
    let index = events
        .iter()
        .position(|event| {
            event["type"] == "response.output_item.added" && event["item"]["id"] == "msg-c"
        })
        .ok_or("message start")?;
    events[index]["item"]["content"] = response["output"][1]["content"].clone();
    events.retain(|event| {
        !(event["type"] == "response.output_text.delta" && event["item_id"] == "msg-c")
    });
    for channel in ["official", "console", "build"] {
        let source = decode_stream(channel, &events, 7)?;
        assert_eq!(
            encode_response(&source, OpenAiResponseMetadata::try_new("grok-c", 0)?)?["output"],
            response["output"]
        );
        let mut changed = events.clone();
        let item = changed
            .iter_mut()
            .find(|event| {
                event["type"] == "response.output_item.done" && event["item"]["id"] == "msg-c"
            })
            .ok_or("message end")?;
        item["item"]["content"][0]["annotations"] = json!([]);
        changed.last_mut().ok_or("terminal")?["response"]["output"][1]["content"][0]["annotations"] =
            json!([]);
        assert!(
            decode_stream(channel, &changed, 7).is_err(),
            "{channel} dropped an initial citation"
        );
    }
    Ok(())
}

#[test]
fn annotations_only_in_completed_snapshots_share_the_retained_budget() -> TestResult {
    let title = "a".repeat(62 * 1024);
    let output = (0..17).map(|index| json!({"id":format!("msg-budget-{index}"),"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"answer","annotations":[{"type":"url_citation","url":"https://example.test/source","title":title,"start_index":0,"end_index":6}]}]})).collect::<Vec<_>>();
    let response = json!({"id":"resp-budget","status":"completed","output":output});
    for completed_parts in [false, true] {
        let mut events = if completed_parts {
            events_with_parts(&response, false)
        } else {
            native_events(&response)
        };
        events.retain(|event| event["type"] != "response.output_text.annotation.added");
        events.last_mut().ok_or("terminal")?["response"]["output"] = json!(
            output
                .iter()
                .map(|item| json!({"id":item["id"]}))
                .collect::<Vec<_>>()
        );
        for channel in ["official", "console", "build"] {
            assert!(
                decode_stream(channel, &events, 4096).is_err(),
                "{channel} bypassed annotation budget, parts={completed_parts}"
            );
        }
    }
    Ok(())
}
