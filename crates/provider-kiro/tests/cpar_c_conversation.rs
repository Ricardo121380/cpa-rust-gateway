//! CPAR C: public Canonical tool-result roles reach the native Kiro conversation.

use std::error::Error;

use gateway_core::CanonicalRequest;
use provider_kiro::{
    conversation_request::{
        KiroConversationContext, KiroConversationId, KiroConversationRequestBuilder,
        KiroEnvironmentState,
    },
    endpoint_policy::{KiroApiRegion, KiroEndpointKind, KiroEndpointPolicy},
};
use serde_json::json;

#[test]
fn native_effort_retains_qualitative_levels_and_refuses_budget_surrogates()
-> Result<(), Box<dyn Error>> {
    let context = KiroConversationContext::new(
        KiroConversationId::try_new("cpar-c-thinking")?,
        KiroEnvironmentState::try_new("linux", "/")?,
    );
    for kind in [KiroEndpointKind::Ide, KiroEndpointKind::Cli] {
        let policy = KiroEndpointPolicy::try_new(kind, KiroApiRegion::try_new("eu-west-1")?)?;
        for effort in [
            "low", "medium", "high", "xhigh", "max", "enabled", "disabled", "1024", "private",
        ] {
            let request: CanonicalRequest = serde_json::from_value(json!({
                "requested_model":"public", "messages":[{"role":"user","content":[{"text":{"text":"question","extensions":{}}}],"extensions":{}}],
                "thinking":{"effort":effort,"extensions":{}},"extensions":{}
            }))?;
            let outbound =
                KiroConversationRequestBuilder::build(&policy, &context, "native", &request);
            if matches!(effort, "enabled" | "disabled" | "1024" | "private") {
                assert!(outbound.is_err(), "unproven effort {effort} was accepted");
            } else {
                let outbound = outbound?;
                let metadata = &outbound.body()["conversationState"]["currentMessage"]["userInputMessage"]
                    ["userInputMessageContext"];
                let encoded = match kind {
                    KiroEndpointKind::Ide => {
                        &metadata["additionalModelRequestFields"]["thinking"]["effort"]
                    }
                    KiroEndpointKind::Cli => &metadata["outputConfig"]["effort"],
                };
                assert_eq!(encoded, effort);
            }
        }
    }
    Ok(())
}

#[test]
fn final_canonical_tool_result_keeps_its_call_identity_output_and_error_status()
-> Result<(), Box<dyn Error>> {
    let context = KiroConversationContext::new(
        KiroConversationId::try_new("cpar-c-conversation")?,
        KiroEnvironmentState::try_new("linux", "/")?,
    );
    for kind in [KiroEndpointKind::Ide, KiroEndpointKind::Cli] {
        let policy = KiroEndpointPolicy::try_new(kind, KiroApiRegion::try_new("us-east-1")?)?;
        for is_error in [false, true] {
            let request: CanonicalRequest = serde_json::from_value(json!({
                "requested_model":"public-model", "messages":[
                    {"role":"user","content":[{"text":{"text":"Question","extensions":{}}}],"extensions":{}},
                    {"role":"assistant","content":[{"tool_call":{"id":"call-c","name":"lookup","arguments":{},"extensions":{}}}],"extensions":{}},
                    {"role":"tool","content":[{"tool_result":{"call_id":"call-c","output":{"answer":"result"},"is_error":is_error,"extensions":{}}}],"extensions":{}}
                ],"extensions":{}
            }))?;
            let outbound =
                KiroConversationRequestBuilder::build(&policy, &context, "native-model", &request)?;
            let result = &outbound.body()["conversationState"]["currentMessage"]["userInputMessage"]
                ["userInputMessageContext"]["toolResults"][0];
            assert_eq!(result["toolUseId"], "call-c");
            assert_eq!(result["content"], json!({"answer":"result"}));
            assert_eq!(result["status"], if is_error { "error" } else { "success" });
            assert_eq!(
                outbound.body()["conversationState"]["history"][1]["assistantResponseMessage"]["toolUses"]
                    [0]["toolUseId"],
                "call-c"
            );
        }
    }
    Ok(())
}
