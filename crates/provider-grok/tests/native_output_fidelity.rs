//! Offline native decoder -> runtime projector -> public codec -> request replay.
use gateway_core::{CanonicalEvent, CanonicalResponse, GatewayErrorCode, MessageContent};
use gateway_router::{ProtocolFormat, project_protocol_response};
use protocol_openai_responses::{
    OpenAiResponseMetadata, OpenAiResponsesSseEncoder, decode_request, encode_response,
};
use provider_grok::{GrokBuildResponsesDecoder, GrokBuildResponsesStreamDecoder};
use serde_json::{Value, json};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn response() -> Value {
    json!({"id":"resp-native","status":"completed","output":[
        {"id":"rs-summary","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"summary one"},{"type":"summary_text","text":"summary two"}]},
        {"id":"rs-both","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"short"}],"content":[{"type":"reasoning_text","text":"body"}]},
        {"id":"fc-native","type":"function_call","status":"completed","call_id":"call-native","name":"read","arguments":"{}"},
        {"id":"msg-native","type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"answer"}]}
    ]})
}
fn record(kind: &str, mut value: Value) -> String {
    value["type"] = json!(kind);
    format!("event: {kind}\ndata: {value}\n\n")
}
fn stream(response: &Value, partial: bool) -> String {
    let mut wire = record(
        "response.created",
        json!({"response":{"id":response["id"]}}),
    );
    for item in response["output"].as_array().into_iter().flatten() {
        let mut start = item.clone();
        start["status"] = json!("in_progress");
        for field in ["summary", "content"] {
            if start.get(field).is_some() {
                start[field] = json!([]);
            }
        }
        if start.get("arguments").is_some() {
            start["arguments"] = json!("");
        }
        wire += &record("response.output_item.added", json!({"item":start}));
        if partial && item["id"] == "rs-summary" {
            wire += &record(
                "response.reasoning_summary_text.delta",
                json!({"item_id":item["id"],"summary_index":0,"delta":"summary "}),
            );
        }
        wire += &record("response.output_item.done", json!({"item":item}));
    }
    wire += &record("response.completed", json!({"response":response}));
    wire
}
fn decode_stream(wire: &str) -> std::result::Result<CanonicalResponse, gateway_core::GatewayError> {
    let mut decoder = GrokBuildResponsesStreamDecoder::new();
    let mut events = Vec::new();
    for chunk in wire.as_bytes().chunks(7) {
        events.extend(decoder.push_bytes(chunk)?);
    }
    decoder.finish()?;
    CanonicalResponse::try_new(events)
}
#[test]
fn native_identity_summary_parts_and_body_survive_all_public_modes() -> Result {
    let upstream = response();
    for source in [
        GrokBuildResponsesDecoder::decode_non_streaming(upstream.to_string().as_bytes())?,
        decode_stream(&stream(&upstream, false))?,
        decode_stream(&stream(&upstream, true))?,
    ] {
        let response = project_protocol_response(&source, ProtocolFormat::OpenAiResponses)?;
        let metadata = OpenAiResponseMetadata::try_new("exact-model", 0)?;
        let json = encode_response(&response, metadata.clone())?;
        assert_eq!(json["output"], upstream["output"]);
        let mut encoder = OpenAiResponsesSseEncoder::new(metadata);
        let mut frames = Vec::new();
        for event in response.events() {
            frames.extend(encoder.encode_event(event)?);
        }
        let last = frames.last().ok_or("no terminal")?;
        assert_eq!(last.data()["response"]["output"], upstream["output"]);
        assert_eq!(
            frames
                .iter()
                .filter(|f| f.event() == "response.output_item.added")
                .count(),
            4
        );
        let summary_deltas = frames
            .iter()
            .filter(|f| f.event() == "response.reasoning_summary_text.delta")
            .filter(|f| f.data()["item_id"] == "rs-summary" && f.data()["summary_index"] == 0)
            .map(|f| f.data()["delta"].as_str().unwrap_or(""))
            .collect::<String>();
        assert_eq!(summary_deltas, "summary one");
        let replay =
            decode_request(&json!({"model":"exact-model","input":json["output"]}).to_string())?;
        let MessageContent::Reasoning(reasoning) = &replay.request.messages[0].content[0] else {
            return Err("reasoning lost".into());
        };
        assert_eq!(
            serde_json::from_str::<Value>(reasoning.raw().get())?,
            upstream["output"][0]
        );
        assert!(project_protocol_response(&source, ProtocolFormat::OpenAiChatCompletions).is_err());
        assert!(project_protocol_response(&source, ProtocolFormat::AnthropicMessages).is_err());
        let persisted = serde_json::to_string(source.events())?;
        assert_eq!(
            serde_json::from_str::<Vec<CanonicalEvent>>(&persisted)?,
            source.events()
        );
        assert!(!format!("{source:?}").contains("summary one"));
    }
    Ok(())
}
#[test]
fn contradictory_and_ciphertext_final_snapshots_fail_closed() -> Result {
    let original = response();
    let wire = stream(&original, true);
    let contradictory = wire.replacen("\"text\":\"summary one\"", "\"text\":\"different\"", 1);
    assert_eq!(
        decode_stream(&contradictory)
            .err()
            .ok_or("contradiction accepted")?
            .code(),
        GatewayErrorCode::UpstreamProtocolError
    );
    for final_only in [false, true] {
        let mut encrypted = original.clone();
        encrypted["output"][0]["encrypted_content"] = json!("synthetic-unowned");
        let wire = if final_only {
            wire.replace(
                &record("response.completed", json!({"response":original})),
                &record("response.completed", json!({"response":encrypted})),
            )
        } else {
            stream(&encrypted, false)
        };
        assert!(decode_stream(&wire).is_err());
        assert!(
            GrokBuildResponsesDecoder::decode_non_streaming(encrypted.to_string().as_bytes())
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn empty_parts_and_empty_items_keep_identity_and_order() -> Result {
    let mut upstream = response();
    upstream["output"][0]["summary"][0]["text"] = json!("");
    upstream["output"][1]["summary"] = json!([]);
    upstream["output"][1]["content"] = json!([]);
    let response = decode_stream(&stream(&upstream, false))?;
    let encoded = encode_response(
        &response,
        OpenAiResponseMetadata::try_new("exact-model", 0)?,
    )?;
    assert_eq!(encoded["output"], upstream["output"]);
    let mut reordered = upstream.clone();
    reordered["output"]
        .as_array_mut()
        .ok_or("output")?
        .swap(0, 1);
    let wire = stream(&upstream, false).replace(
        &record("response.completed", json!({"response":upstream})),
        &record("response.completed", json!({"response":reordered})),
    );
    assert!(decode_stream(&wire).is_err());
    Ok(())
}

#[test]
fn blank_tool_arguments_keep_the_existing_empty_object_contract() -> Result {
    for arguments in ["", "  ", "{ }", "{\n }"] {
        let mut upstream = response();
        upstream["output"][2]["arguments"] = json!(arguments);
        for source in [
            GrokBuildResponsesDecoder::decode_non_streaming(upstream.to_string().as_bytes())?,
            decode_stream(&stream(&upstream, false))?,
            decode_stream(&stream(&upstream, false).replace(
                &record(
                    "response.output_item.done",
                    json!({"item":upstream["output"][2]}),
                ),
                &(record(
                    "response.function_call_arguments.delta",
                    json!({"item_id":"fc-native", "delta":arguments}),
                ) + &record(
                    "response.output_item.done",
                    json!({"item":upstream["output"][2]}),
                )),
            ))?,
        ] {
            let encoded =
                encode_response(&source, OpenAiResponseMetadata::try_new("exact-model", 0)?)?;
            let mut expected = upstream.clone();
            expected["output"][2]["arguments"] = json!("{}");
            assert_eq!(encoded["output"], expected["output"]);
        }
    }
    Ok(())
}

#[test]
fn interleaved_items_complete_in_reverse_order_without_reordering_output() -> Result {
    let upstream = response();
    let output = upstream["output"].as_array().ok_or("output")?;
    let mut wire = record(
        "response.created",
        json!({"response":{"id":upstream["id"]}}),
    );
    for item in output {
        let mut start = item.clone();
        start["status"] = json!("in_progress");
        for field in ["summary", "content"] {
            if start.get(field).is_some() {
                start[field] = json!([]);
            }
        }
        if start.get("arguments").is_some() {
            start["arguments"] = json!("");
        }
        wire += &record("response.output_item.added", json!({"item":start}));
    }
    for item in output.iter().rev() {
        wire += &record("response.output_item.done", json!({"item":item}));
    }
    wire += &record("response.completed", json!({"response":upstream}));
    let encoded = encode_response(
        &decode_stream(&wire)?,
        OpenAiResponseMetadata::try_new("exact-model", 0)?,
    )?;
    assert_eq!(encoded["output"], upstream["output"]);
    Ok(())
}
