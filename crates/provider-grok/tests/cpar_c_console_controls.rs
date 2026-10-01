//! CPAR C: Console retains the caller's exact function and selection controls.

use std::{error::Error, fmt::Write as _};

use gateway_core::CanonicalEvent;
use protocol_openai_responses::{ResponseMode, decode_request};
use provider_grok::{
    GrokConsoleResponsesRequestBuilder, GrokConsoleSsoToken, GrokOfficialResponsesDecoder,
    GrokOfficialResponsesStreamDecoder,
};
use serde_json::{Value, json};

#[test]
fn console_preserves_selection_without_adding_undeclared_search_tools() -> Result<(), Box<dyn Error>>
{
    let token = GrokConsoleSsoToken::try_from_bytes(b"synthetic-cpar-c-console")?;
    for selection in [
        json!("none"),
        json!("required"),
        json!({"type":"function","name":"lookup"}),
    ] {
        let input = json!({
            "model":"public-model", "input":"Look this up.",
            "tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}],
            "tool_choice":selection, "parallel_tool_calls":false
        });
        let decoded = decode_request(&input.to_string())?;
        let outbound = GrokConsoleResponsesRequestBuilder::build(
            &token,
            "grok-4.3",
            &decoded.request,
            ResponseMode::NonStreaming,
        )?;
        let body: Value = serde_json::from_slice(outbound.body())?;
        assert_eq!(body["tool_choice"], selection);
        assert_eq!(body["parallel_tool_calls"], false);
        assert_eq!(body["tools"].as_array().map(Vec::len), Some(1));
        assert_eq!(body["tools"][0]["name"], "lookup");
    }
    Ok(())
}

#[test]
fn official_and_console_reject_unrepresented_metadata_and_changed_final_snapshots()
-> Result<(), Box<dyn Error>> {
    let plain = json!({"id":"msg-c","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"answer","annotations":[]}]});
    let mut citation = plain.clone();
    citation["content"][0]["annotations"] =
        json!([{"type":"url_citation","url":"https://example.test","start_index":0,"end_index":6}]);
    for item in [
        citation,
        json!({"id":"reason-c","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"summary","signature":"synthetic-signature"}]}),
        json!({"id":"reason-c","type":"reasoning","status":"completed","content":[{"type":"reasoning_text","text":"thought"}],"encrypted_content":"synthetic-unowned"}),
    ] {
        let response = json!({"id":"resp-c","status":"completed","output":[item]});
        assert!(
            GrokOfficialResponsesDecoder::decode_non_streaming(response.to_string().as_bytes())
                .is_err()
        );
        assert!(
            provider_grok::GrokConsoleResponsesDecoder::decode_non_streaming(
                response.to_string().as_bytes()
            )
            .is_err()
        );
    }
    let mut changed = plain.clone();
    changed["content"][0]["text"] = json!("different");
    for final_item in [
        changed,
        json!({"id":"msg-c","type":"reasoning","status":"completed","content":[{"type":"reasoning_text","text":"answer"}]}),
    ] {
        let mut wire = String::new();
        for event in [
            json!({"type":"response.created","response":{"id":"resp-c"}}),
            json!({"type":"response.output_item.added","item":plain}),
            json!({"type":"response.output_item.done","item":plain}),
            json!({"type":"response.completed","response":{"id":"resp-c","status":"completed","output":[final_item]}}),
        ] {
            write!(
                wire,
                "event: {}\ndata: {event}\n\n",
                event["type"].as_str().ok_or("event")?
            )?;
        }
        assert!(
            GrokOfficialResponsesStreamDecoder::new()
                .push_bytes(wire.as_bytes())
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn official_and_console_reject_logprobs_on_text_events() -> Result<(), Box<dyn Error>> {
    let plain = json!({"id":"msg-c","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"answer","annotations":[]}]});
    for corrupted_kind in ["response.output_text.delta", "response.output_text.done"] {
        let mut wire = String::new();
        for mut event in [
            json!({"type":"response.created","response":{"id":"resp-c"}}),
            json!({"type":"response.output_item.added","item":{"id":"msg-c","type":"message","role":"assistant","status":"in_progress","content":[]}}),
            json!({"type":"response.output_text.delta","item_id":"msg-c","delta":"answer"}),
            json!({"type":"response.output_text.done","item_id":"msg-c","text":"answer"}),
            json!({"type":"response.output_item.done","item":plain}),
            json!({"type":"response.completed","response":{"id":"resp-c","status":"completed","output":[plain]}}),
        ] {
            if event["type"] == corrupted_kind {
                event["logprobs"] = json!([{"token":"answer","logprob":-0.25}]);
            }
            write!(
                wire,
                "event: {}\ndata: {event}\n\n",
                event["type"].as_str().ok_or("event")?
            )?;
        }
        assert!(
            GrokOfficialResponsesStreamDecoder::new()
                .push_bytes(wire.as_bytes())
                .is_err(),
            "{corrupted_kind} lost metadata"
        );
        assert!(
            provider_grok::GrokConsoleResponsesStreamDecoder::new()
                .push_bytes(wire.as_bytes())
                .is_err(),
            "{corrupted_kind} lost metadata"
        );
    }
    Ok(())
}

#[test]
fn official_and_console_tool_deltas_match_normalized_final_arguments() -> Result<(), Box<dyn Error>>
{
    for arguments in ["", " \t", "{}", "{ }", " {\"value\":1} "] {
        let item = json!({"id":"fc-c","type":"function_call","status":"completed","call_id":"call-c","name":"lookup","arguments":arguments});
        let response = json!({"id":"resp-c","status":"completed","output":[item]});
        let mut wire = String::new();
        for event in [
            json!({"type":"response.created","response":{"id":"resp-c"}}),
            json!({"type":"response.output_item.added","item":{"id":"fc-c","type":"function_call","call_id":"call-c","name":"lookup"}}),
        ] {
            write!(
                wire,
                "event: {}\ndata: {event}\n\n",
                event["type"].as_str().ok_or("event")?
            )?;
        }
        for character in arguments.chars() {
            let event = json!({"type":"response.function_call_arguments.delta","item_id":"fc-c","call_id":"call-c","delta":character.to_string()});
            write!(
                wire,
                "event: response.function_call_arguments.delta\ndata: {event}\n\n"
            )?;
        }
        for event in [
            json!({"type":"response.function_call_arguments.done","item_id":"fc-c","call_id":"call-c","arguments":arguments}),
            json!({"type":"response.output_item.done","item":item}),
            json!({"type":"response.completed","response":response}),
        ] {
            write!(
                wire,
                "event: {}\ndata: {event}\n\n",
                event["type"].as_str().ok_or("event")?
            )?;
        }
        let mut decoder = GrokOfficialResponsesStreamDecoder::new();
        let mut streamed = Vec::new();
        for bytes in wire.as_bytes().chunks(7) {
            streamed.extend(decoder.push_bytes(bytes)?);
        }
        decoder.finish()?;
        for events in [
            GrokOfficialResponsesDecoder::decode_non_streaming(response.to_string().as_bytes())?
                .into_events(),
            streamed,
        ] {
            let deltas = events
                .iter()
                .filter_map(|event| match event {
                    CanonicalEvent::ToolCallArgumentsDelta(delta) => Some(delta.delta.as_str()),
                    _ => None,
                })
                .collect::<String>();
            let end = events
                .iter()
                .find_map(|event| match event {
                    CanonicalEvent::ToolCallEnd(end) => Some(end.arguments.get()),
                    _ => None,
                })
                .ok_or("tool end")?;
            let expected = if arguments.contains("value") {
                arguments
            } else {
                "{}"
            };
            assert_eq!(deltas, expected);
            assert_eq!(end, expected.trim());
        }
    }
    Ok(())
}
