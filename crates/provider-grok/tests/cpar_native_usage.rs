//! Complete native usage must not disappear behind an otherwise encodable response.
use gateway_core::{CanonicalEvent, GatewayErrorCode};
use gateway_router::{ProtocolFormat, ProtocolResponseRejection, project_protocol_response};
use provider_grok::GrokBuildResponsesDecoder;
use serde_json::{Value, json};

fn fixture() -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(include_str!(
        "../../../docs/reports/assets/cpar-native-chat-offline-20261017/minimal-native.json"
    ))?)
}

#[test]
fn native_usage_unknown_fields_are_retained_and_fail_closed()
-> Result<(), Box<dyn std::error::Error>> {
    for (field, extra) in [
        ("cost_in_usd_ticks", json!(123_000_000)),
        ("num_sources_used", json!(0)),
        ("num_server_side_tools_used", json!(0)),
        (
            "synthetic_cost_details",
            json!({"input_tokens":11,"output_tokens":17}),
        ),
        (
            "input_tokens_details",
            json!({"cached_tokens":128,"synthetic_counter":null}),
        ),
        (
            "output_tokens_details",
            json!({"reasoning_tokens":158,"synthetic_counter":[]}),
        ),
    ] {
        let mut value = fixture()?;
        value["usage"][field] = extra;
        let response =
            GrokBuildResponsesDecoder::decode_non_streaming(&serde_json::to_vec(&value)?)?;
        let usage = response
            .events()
            .iter()
            .find_map(|event| match event {
                CanonicalEvent::UsageDelta(delta) => Some(&delta.usage),
                _ => None,
            })
            .ok_or("missing validated usage")?;
        assert_eq!(usage.input_tokens, Some(228));
        assert_eq!(usage.output_tokens, Some(159));
        assert!(
            !usage.extensions.is_empty(),
            "native usage field was discarded: {field}"
        );
        let retained: Value = serde_json::from_str(
            usage
                .extensions
                .get("grok.build.usage")
                .ok_or("retained native usage")?
                .get(),
        )?;
        assert_eq!(retained, value["usage"]);
        assert_eq!(
            project_protocol_response(&response, ProtocolFormat::OpenAiResponses).err(),
            Some(ProtocolResponseRejection::UnknownExtensions)
        );
    }
    Ok(())
}

#[test]
fn native_usage_totals_must_confirm_known_counters() -> Result<(), Box<dyn std::error::Error>> {
    for total in [json!(386), json!(-1), json!("387"), json!(false)] {
        let mut value = fixture()?;
        value["usage"]["total_tokens"] = total;
        assert_eq!(
            GrokBuildResponsesDecoder::decode_non_streaming(&serde_json::to_vec(&value)?)
                .err()
                .ok_or("invalid total accepted")?
                .code(),
            GatewayErrorCode::UpstreamProtocolError
        );
    }
    for absent in [false, true] {
        let mut value = fixture()?;
        if absent {
            value["usage"]
                .as_object_mut()
                .ok_or("usage")?
                .remove("output_tokens");
        }
        value["usage"]["total_tokens"] = json!(387);
        let response =
            GrokBuildResponsesDecoder::decode_non_streaming(&serde_json::to_vec(&value)?)?;
        assert_eq!(
            project_protocol_response(&response, ProtocolFormat::OpenAiResponses).is_ok(),
            !absent
        );
    }
    let mut overflow = fixture()?;
    overflow["usage"]["input_tokens"] = json!(u64::MAX);
    overflow["usage"]["output_tokens"] = json!(1);
    overflow["usage"]["total_tokens"] = json!(0);
    assert!(
        GrokBuildResponsesDecoder::decode_non_streaming(&serde_json::to_vec(&overflow)?).is_err()
    );
    for total in [None, Some(Value::Null)] {
        let mut value = fixture()?;
        if let Some(total) = total {
            value["usage"]["total_tokens"] = total;
        } else {
            value["usage"]
                .as_object_mut()
                .ok_or("usage")?
                .remove("total_tokens");
        }
        let response =
            GrokBuildResponsesDecoder::decode_non_streaming(&serde_json::to_vec(&value)?)?;
        assert!(project_protocol_response(&response, ProtocolFormat::OpenAiResponses).is_ok());
    }
    Ok(())
}

#[test]
fn native_summary_is_not_a_reasoning_body_and_item_metadata_stays_strict()
-> Result<(), Box<dyn std::error::Error>> {
    let value = fixture()?;
    let response = GrokBuildResponsesDecoder::decode_non_streaming(&serde_json::to_vec(&value)?)?;
    assert!(project_protocol_response(&response, ProtocolFormat::OpenAiResponses).is_ok());
    for target in [
        ProtocolFormat::OpenAiChatCompletions,
        ProtocolFormat::AnthropicMessages,
    ] {
        assert_eq!(
            project_protocol_response(&response, target).err(),
            Some(ProtocolResponseRejection::UnknownExtensions)
        );
        let mut empty = value.clone();
        empty["output"][0]["summary"][0]["text"] = json!("");
        let empty = GrokBuildResponsesDecoder::decode_non_streaming(&serde_json::to_vec(&empty)?)?;
        assert_eq!(
            project_protocol_response(&empty, target).err(),
            Some(ProtocolResponseRejection::ReasoningUnsupported)
        );
        // An independent, supported input; never a rewrite of the captured response.
        let mut plain = value.clone();
        plain["output"][0]["summary"] = json!([]);
        plain["output"][0]["content"] = json!([{"type":"reasoning_text","text":"SYNTHETIC_BODY"}]);
        let plain = GrokBuildResponsesDecoder::decode_non_streaming(&serde_json::to_vec(&plain)?)?;
        assert!(project_protocol_response(&plain, target).is_ok());
    }
    for (path, extra) in [
        ("item", json!({"unexpected":null})),
        ("part", json!({"unexpected":[]})),
        ("status", json!("in_progress")),
        ("identity", json!("synthetic-reasoning")),
    ] {
        let mut bad = value.clone();
        match path {
            "item" => bad["output"][0]["unexpected"] = extra,
            "part" => bad["output"][1]["content"][0]["unexpected"] = extra,
            "status" => bad["output"][1]["status"] = extra,
            _ => bad["output"][1]["id"] = extra,
        }
        assert!(
            GrokBuildResponsesDecoder::decode_non_streaming(&serde_json::to_vec(&bad)?).is_err()
        );
    }
    Ok(())
}
