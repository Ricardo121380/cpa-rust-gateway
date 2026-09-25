//! Regression for the native Grok 422 caused by an omitted required reasoning summary.
use gateway_core::{RawJson, ReasoningHistory};
use protocol_openai_responses::encode_reasoning_history;
use serde_json::{Value, json};

#[test]
fn legacy_history_gains_only_a_missing_summary_without_mutating_the_source()
-> Result<(), Box<dyn std::error::Error>> {
    for summary in [
        None,
        Some(Value::Null),
        Some(json!([])),
        Some(json!([{"type":"summary_text","text":"Keep this summary."}])),
    ] {
        let mut item = json!({"type":"reasoning","id":"rs-original","status":"completed","content":[{"type":"reasoning_text","text":"Preserve content verbatim."}]});
        if let Some(summary) = summary {
            item["summary"] = summary;
        }
        let raw = RawJson::from_json_string(item.to_string())?;
        let history = ReasoningHistory::try_from(raw)?;
        let encoded = encode_reasoning_history(&history)?;
        assert_eq!(serde_json::from_str::<Value>(history.raw().get())?, item);
        if item.get("summary").is_none_or(Value::is_null) {
            item["summary"] = json!([]);
        }
        assert_eq!(encoded, item);
    }
    Ok(())
}
