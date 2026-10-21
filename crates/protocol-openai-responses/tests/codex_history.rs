//! Closed unidentified Codex history keeps native presence without generic service admission.
use gateway_core::MessageContent;
use protocol_openai_responses::{decode_request, encode_reasoning_history, input_history};
use serde_json::{Value, json};

#[test]
fn codex_unidentified_reasoning_is_closed_and_preserves_presence()
-> Result<(), Box<dyn std::error::Error>> {
    for content in [None, Some(Value::Null)] {
        for encrypted in [None, Some(Value::Null)] {
            let mut item = json!({"type":"reasoning","summary":[{"type":"summary_text","text":"exact summary"}]});
            if let Some(value) = &content {
                item["content"] = value.clone();
            }
            if let Some(value) = &encrypted {
                item["encrypted_content"] = value.clone();
            }
            let decoded = decode_request(&json!({"model":"m","input":[item.clone()]}).to_string())?;
            assert!(input_history::requires_qualification(&decoded.request));
            let MessageContent::Reasoning(history) = &decoded.request.messages[0].content[0] else {
                return Err("reasoning representation".into());
            };
            assert_eq!(serde_json::from_str::<Value>(history.raw().get())?, item);
            assert!(
                encode_reasoning_history(history).is_err(),
                "generic encoding is unqualified"
            );
            for (key, value) in [
                ("id", Value::Null),
                ("id", json!("")),
                ("content", json!([])),
                (
                    "encrypted_content",
                    json!("cpar_reasoning_v1.".to_owned() + &"x".repeat(60)),
                ),
                ("status", json!("completed")),
                ("summary", Value::Null),
                ("unknown", json!(true)),
            ] {
                let mut invalid = item.clone();
                invalid[key] = value;
                assert!(
                    decode_request(&json!({"model":"m","input":[invalid]}).to_string()).is_err(),
                    "{key}"
                );
            }
        }
    }
    Ok(())
}
