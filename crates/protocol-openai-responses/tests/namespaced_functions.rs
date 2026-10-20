//! Native namespace declarations, collisions and optional result identity presence.
use gateway_core::{RawExtensions, RawJson, ToolDefinition};
use protocol_openai_responses::{RESPONSES_TOOLS, ResponsesToolSet, decode_request};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn namespaced_semantic_call_without_native_identity_envelope_cannot_flatten() -> TestResult {
    use gateway_core::{
        CanonicalEvent, CanonicalResponse, MessageEnd, MessageRole, MessageStart, ResponseEnd,
        ResponseId, ResponseStart, ToolCallEnd, ToolCallStart,
    };
    let mut extensions = RawExtensions::default();
    extensions.try_insert("namespace", RawJson::from_json_string(r#""alpha""#.into())?)?;
    let response = CanonicalResponse::try_new(vec![
        CanonicalEvent::ResponseStart(ResponseStart {
            response_id: ResponseId::try_new("response")?,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".into()),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::ToolCallStart(ToolCallStart {
            call_id: "call".into(),
            name: "lookup".into(),
            extensions,
        }),
        CanonicalEvent::ToolCallEnd(ToolCallEnd {
            call_id: "call".into(),
            arguments: RawJson::from_json_string("{}".into())?,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageEnd(MessageEnd::default()),
        CanonicalEvent::ResponseEnd(ResponseEnd::default()),
    ])?;
    assert!(
        protocol_openai_responses::encode_response(
            &response,
            protocol_openai_responses::OpenAiResponseMetadata::try_new("test", 1)?
        )
        .is_err()
    );
    Ok(())
}

fn tools() -> Value {
    json!([{"type":"namespace","name":"alpha","description":"first group","tools":[
        {"type":"function","name":"lookup","description":null,"parameters":{"type":"object","properties":{"id":{"type":"integer"}}},"strict":false}]},
        {"type":"namespace","name":"beta","description":"second group","tools":[
        {"type":"function","name":"lookup","parameters":{"type":"object"},"strict":true}]}])
}

#[test]
fn native_namespace_envelope_preserves_order_and_rejects_collisions_and_duplicate_json()
-> TestResult {
    let decoded =
        decode_request(&json!({"model":"test","input":"question","tools":tools()}).to_string())?;
    assert!(decoded.request.tools.is_empty());
    let set = ResponsesToolSet::from_request(&decoded.request)?.ok_or("set")?;
    assert_eq!(set.wire(), &tools());
    assert_eq!(set.functions().len(), 2);
    assert!(!format!("{set:?}").contains("lookup"));
    let bytes = serde_json::to_string(&decoded.request)?;
    let reopened = serde_json::from_str(&bytes)?;
    assert_eq!(decoded.request, reopened);
    let mut collision = decoded.request.clone();
    collision.tools.push(ToolDefinition {
        name: "other".into(),
        description: None,
        input_schema: RawJson::from_json_string("{}".into())?,
        extensions: RawExtensions::default(),
    });
    assert!(ResponsesToolSet::from_request(&collision).is_err());
    let duplicate = decoded
        .request
        .extensions
        .get(RESPONSES_TOOLS)
        .ok_or("raw")?
        .get()
        .replace("\"strict\":false", "\"strict\":false,\"strict\":true");
    assert!(ResponsesToolSet::decode(&RawJson::from_json_string(duplicate)?).is_err());
    let duplicate_schema = tools().to_string().replace(
        "\"type\":\"integer\"",
        "\"type\":\"integer\",\"type\":\"string\"",
    );
    assert!(ResponsesToolSet::decode(&RawJson::from_json_string(duplicate_schema)?).is_err());
    Ok(())
}

#[test]
fn namespace_results_retain_independent_absent_null_and_value_identity_members() -> TestResult {
    for name in [None, Some(Value::Null), Some(json!("lookup"))] {
        for namespace in [None, Some(Value::Null), Some(json!("alpha"))] {
            let mut result =
                json!({"type":"function_call_output","call_id":"call-a","output":"41"});
            for (key, value) in [("name", name.clone()), ("namespace", namespace)] {
                if let Some(value) = value {
                    result[key] = value;
                }
            }
            let request=decode_request(&json!({"model":"test","tools":tools(),"input":[
                {"type":"function_call","call_id":"call-a","name":"lookup","namespace":"alpha","arguments":"{}"},result.clone()
            ]}).to_string())?.request;
            ResponsesToolSet::from_request(&request)?
                .ok_or("set")?
                .validate_history(&request)?;
            let gateway_core::MessageContent::ToolResult(stored) = &request.messages[1].content[0]
            else {
                return Err("result".into());
            };
            for key in ["name", "namespace"] {
                let retained = stored
                    .extensions
                    .get(key)
                    .map(|raw| serde_json::from_str::<Value>(raw.get()))
                    .transpose()?;
                assert_eq!(retained, result.get(key).cloned());
            }
        }
    }
    Ok(())
}
