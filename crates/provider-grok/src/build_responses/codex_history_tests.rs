use super::*;
use serde_json::json;

#[test]
fn build_codex_history_requires_independent_qualification() -> Result<(), Box<dyn std::error::Error>>
{
    let credential=GrokBuildCredential::import_json(br#"{"access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_in":3600,"token_type":"Bearer"}"#,0)?;
    for (reasoning, result_id) in [(true, false), (false, true), (true, true)] {
        let mut input = vec![
            json!({"type":"function_call","id":"call-item","call_id":"call","name":"lookup","arguments":"{}"}),
            json!({"type":"function_call_output","call_id":"call","output":"exact"}),
        ];
        if result_id {
            input[1]["id"] = json!("r".repeat(512));
        }
        if reasoning {
            input.insert(
                0,
                json!({"type":"reasoning","summary":[],"content":null,"encrypted_content":null}),
            );
        }
        let decoded = protocol_openai_responses::decode_request(
            &json!({"model":"m","input":input}).to_string(),
        )?;
        assert!(
            GrokBuildResponsesRequestBuilder::build(
                &credential,
                "grok-4.5",
                &decoded.request,
                ResponseMode::NonStreaming
            )
            .is_err()
        );
        assert!(
            crate::official_responses::encode_responses_body(
                "grok-4.5",
                &decoded.request,
                ResponseMode::NonStreaming,
                &["low", "medium", "high"]
            )
            .is_err()
        );
        for qualified in [false, true] {
            let result = GrokBuildResponsesRequestBuilder::build_owned(
                &credential,
                "grok-4.5",
                &decoded.request,
                ResponseMode::NonStreaming,
                None,
                None,
                true,
                true,
                true,
                true,
                qualified,
            );
            if qualified {
                let sent: Value = serde_json::from_slice(result?.body())?;
                assert_eq!(sent["input"], json!(input));
            } else {
                assert_eq!(
                    result.err().ok_or("independent rejection")?.code(),
                    GatewayErrorCode::ClientRequestError
                );
            }
        }
    }
    Ok(())
}
