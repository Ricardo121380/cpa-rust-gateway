//! Semantic completion is guarded independently of downstream encoding.
use super::*;
use serde_json::json;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn event(kind: &str, mut v: Value) -> String {
    v["type"] = json!(kind);
    format!("event: {kind}\ndata: {v}\n\n")
}
fn call(index: usize) -> Value {
    json!({"id":format!("item-{index}"),"type":"function_call","status":"completed","name":"lookup","call_id":format!("call-{index}"),"arguments":format!("{{\"seed\":{index}}}")})
}
fn prefix() -> String {
    let mut wire = event(
        "response.created",
        json!({"response":{"id":"parallel-response","status":"in_progress","output":[]}}),
    );
    for index in [0, 1] {
        let mut start = call(index);
        start["arguments"] = json!("");
        start["status"] = json!("in_progress");
        wire += &event(
            "response.output_item.added",
            json!({"output_index":index,"item":start}),
        );
    }
    for index in [1, 0] {
        wire += &event(
            "response.function_call_arguments.delta",
            json!({"output_index":index,"item_id":format!("item-{index}"),"delta":call(index)["arguments"]}),
        );
        wire += &event(
            "response.function_call_arguments.done",
            json!({"output_index":index,"item_id":format!("item-{index}"),"arguments":call(index)["arguments"]}),
        );
        wire += &event(
            "response.output_item.done",
            json!({"output_index":index,"item":call(index)}),
        );
    }
    wire
}
#[test]
fn parallel_completion_barrier_holds_semantic_and_native_ends() -> TestResult {
    for size in [1, 7, 31] {
        for failed in [false, true] {
            let mut decoder =
                GrokBuildResponsesStreamDecoder::new().with_parallel_qualification(true);
            let mut before = Vec::new();
            for chunk in prefix().as_bytes().chunks(size) {
                before.extend(decoder.push_bytes(chunk)?);
            }
            assert_eq!(
                before
                    .iter()
                    .filter(|v| matches!(v, CanonicalEvent::ToolCallStart(_)))
                    .count(),
                2
            );
            assert!(
                before
                    .iter()
                    .any(|v| matches!(v, CanonicalEvent::ToolCallArgumentsDelta(_)))
            );
            assert!(before.iter().all(|v| !matches!(
                v,
                CanonicalEvent::ToolCallEnd(_) | CanonicalEvent::OutputItemEnd(_)
            )));
            let mut terminal = json!({"id":"parallel-response","status":"completed","output":[call(0),call(1)],"usage":{"input_tokens":20,"output_tokens":7,"total_tokens":27}});
            if failed {
                terminal["output"][1]["arguments"] = json!("{}");
            }
            let wire = event("response.completed", json!({"response":terminal}));
            let result = decoder.push_bytes(wire.as_bytes());
            if failed {
                assert!(result.is_err());
            } else {
                let events = result?;
                assert_eq!(
                    events
                        .iter()
                        .filter(|v| matches!(v, CanonicalEvent::ToolCallEnd(_)))
                        .count(),
                    2
                );
                assert_eq!(
                    events
                        .iter()
                        .filter(|v| matches!(v, CanonicalEvent::OutputItemEnd(_)))
                        .count(),
                    2
                );
                assert!(matches!(
                    events.first(),
                    Some(CanonicalEvent::UsageDelta(_))
                ));
                decoder.finish()?;
            }
        }
    }
    Ok(())
}
#[test]
fn build_parallel_requires_independent_qualification() -> TestResult {
    let credential=GrokBuildCredential::import_json(br#"{"access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_in":3600,"token_type":"Bearer"}"#,0)?;
    for parallel in [None, Some(false), Some(true)] {
        let mut body = json!({"model":"public-model","input":"hello"});
        if let Some(value) = parallel {
            body["parallel_tool_calls"] = json!(value);
        }
        let decoded = protocol_openai_responses::decode_request(&body.to_string())?;
        for qualified in [false, true] {
            let result = GrokBuildResponsesRequestBuilder::build_owned(
                &credential,
                "grok-4.5",
                &decoded.request,
                ResponseMode::NonStreaming,
                None,
                None,
                false,
                false,
                false,
                qualified,
            );
            if parallel == Some(true) && !qualified {
                assert_eq!(
                    result.err().ok_or("rejection")?.code(),
                    GatewayErrorCode::ClientRequestError
                );
            } else {
                let encoded: Value = serde_json::from_slice(result?.body())?;
                assert_eq!(
                    encoded.get("parallel_tool_calls"),
                    body.get("parallel_tool_calls")
                );
            }
        }
    }
    Ok(())
}
