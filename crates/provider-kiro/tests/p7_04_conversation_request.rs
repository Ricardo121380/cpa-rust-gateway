//! P7-04 Canonical-to-Kiro conversation request fixtures.

use std::error::Error;

use gateway_core::CanonicalRequest;
use provider_kiro::{
    conversation_request::{
        KiroConversationContext, KiroConversationId, KiroConversationRequestBuilder,
        KiroConversationRequestError, KiroEnvironmentState,
    },
    endpoint_policy::{KiroApiRegion, KiroEndpointKind, KiroEndpointPolicy},
};
use serde_json::json;

type TestResult = Result<(), Box<dyn Error>>;

fn context() -> Result<KiroConversationContext, KiroConversationRequestError> {
    Ok(KiroConversationContext::new(
        KiroConversationId::try_new("fixture-conversation-01")?,
        KiroEnvironmentState::try_new("linux", "/workspace/fixture")?,
    ))
}

fn decode_request(value: serde_json::Value) -> Result<CanonicalRequest, serde_json::Error> {
    serde_json::from_value(value)
}

#[test]
fn ide_fixture_preserves_ordered_text_history_and_declared_tools() -> TestResult {
    let request = decode_request(json!({
        "requested_model": "public-kiro-alias-must-not-leak",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"text": {"text": "first ", "extensions": {}}},
                    {"text": {"text": "user", "extensions": {}}}
                ],
                "extensions": {}
            },
            {
                "role": "assistant",
                "content": [{"text": {"text": "first assistant", "extensions": {}}}],
                "extensions": {}
            },
            {
                "role": "user",
                "content": [{"text": {"text": "current user", "extensions": {}}}],
                "extensions": {}
            }
        ],
        "tools": [{
            "name": "lookup",
            "description": "Look up a forecast.",
            "input_schema": {
                "type": "object",
                "properties": {"city": {"type": "string"}},
                "required": ["city"]
            },
            "extensions": {}
        }],
        "extensions": {}
    }))?;
    let policy =
        KiroEndpointPolicy::try_new(KiroEndpointKind::Ide, KiroApiRegion::try_new("us-east-1")?)?;

    let actual = KiroConversationRequestBuilder::build(
        &policy,
        &context()?,
        "selected-kiro-model",
        &request,
    )?;

    assert_eq!(
        actual.body(),
        &json!({
            "conversationState": {
                "conversationId": "fixture-conversation-01",
                "history": [
                    {"userInputMessage": {
                        "content": "first user",
                        "modelId": "selected-kiro-model",
                        "origin": "AI_EDITOR"
                    }},
                    {"assistantResponseMessage": {"content": "first assistant"}}
                ],
                "currentMessage": {"userInputMessage": {
                    "content": "current user",
                    "modelId": "selected-kiro-model",
                    "origin": "AI_EDITOR",
                    "userInputMessageContext": {
                        "envState": {
                            "operatingSystem": "linux",
                            "currentWorkingDirectory": "/workspace/fixture"
                        },
                        "tools": [{"toolSpecification": {
                            "name": "lookup",
                            "description": "Look up a forecast.",
                            "inputSchema": {"json": {
                                "type": "object",
                                "properties": {"city": {"type": "string"}},
                                "required": ["city"]
                            }}
                        }}]
                    }
                }}
            }
        })
    );
    let serialized = serde_json::to_string(actual.body())?;
    assert!(!serialized.contains("public-kiro-alias-must-not-leak"));
    Ok(())
}

#[test]
fn cli_fixture_uses_cli_origin_for_current_and_history_users() -> TestResult {
    let request = decode_request(json!({
        "requested_model": "public-model",
        "messages": [
            {"role": "user", "content": [{"text": {"text": "previous", "extensions": {}}}], "extensions": {}},
            {"role": "assistant", "content": [{"text": {"text": "answer", "extensions": {}}}], "extensions": {}},
            {"role": "user", "content": [{"text": {"text": "next", "extensions": {}}}], "extensions": {}}
        ],
        "extensions": {}
    }))?;
    let policy =
        KiroEndpointPolicy::try_new(KiroEndpointKind::Cli, KiroApiRegion::try_new("us-west-2")?)?;

    let actual =
        KiroConversationRequestBuilder::build(&policy, &context()?, "selected-model", &request)?;

    assert_eq!(
        actual.body()["conversationState"]["history"][0]["userInputMessage"]["origin"],
        "KIRO_CLI"
    );
    assert_eq!(
        actual.body()["conversationState"]["currentMessage"]["userInputMessage"]["origin"],
        "KIRO_CLI"
    );
    assert_eq!(
        actual.body()["conversationState"]["history"][1]["assistantResponseMessage"]["content"],
        "answer"
    );
    Ok(())
}

#[test]
fn unsupported_canonical_semantics_fail_closed_until_their_own_kiro_task() -> TestResult {
    let policy = KiroEndpointPolicy::try_new(
        KiroEndpointKind::Ide,
        KiroApiRegion::try_new("eu-central-1")?,
    )?;
    let context = context()?;

    let no_current =
        decode_request(json!({"requested_model": "m", "messages": [], "extensions": {}}))?;
    assert_eq!(
        KiroConversationRequestBuilder::build(&policy, &context, "m", &no_current),
        Err(KiroConversationRequestError::MissingCurrentUserMessage)
    );

    let current_assistant = decode_request(json!({
        "requested_model": "m",
        "messages": [{"role": "assistant", "content": [{"text": {"text": "x", "extensions": {}}}], "extensions": {}}],
        "extensions": {}
    }))?;
    assert_eq!(
        KiroConversationRequestBuilder::build(&policy, &context, "m", &current_assistant),
        Err(KiroConversationRequestError::CurrentMessageMustBeUser)
    );

    let empty_current = decode_request(json!({
        "requested_model": "m",
        "messages": [{"role": "user", "content": [{"text": {"text": "", "extensions": {}}}], "extensions": {}}],
        "extensions": {}
    }))?;
    assert_eq!(
        KiroConversationRequestBuilder::build(&policy, &context, "m", &empty_current),
        Err(KiroConversationRequestError::UnsupportedMessageContent)
    );

    let system_history = decode_request(json!({
        "requested_model": "m",
        "messages": [
            {"role": "system", "content": [{"text": {"text": "do not collapse roles", "extensions": {}}}], "extensions": {}},
            {"role": "user", "content": [{"text": {"text": "x", "extensions": {}}}], "extensions": {}}
        ],
        "extensions": {}
    }))?;
    assert_eq!(
        KiroConversationRequestBuilder::build(&policy, &context, "m", &system_history),
        Err(KiroConversationRequestError::UnsupportedMessageRole)
    );

    let malformed_historical_tool_call = decode_request(json!({
        "requested_model": "m",
        "messages": [
            {"role": "assistant", "content": [{"tool_call": {"id": "call-1", "name": "lookup", "arguments": [], "extensions": {}}}], "extensions": {}},
            {"role": "user", "content": [{"text": {"text": "x", "extensions": {}}}], "extensions": {}}
        ],
        "extensions": {}
    }))?;
    assert_eq!(
        KiroConversationRequestBuilder::build(
            &policy,
            &context,
            "m",
            &malformed_historical_tool_call
        ),
        Err(KiroConversationRequestError::InvalidHistoricalTool)
    );

    let unsupported_thinking_extension = decode_request(json!({
        "requested_model": "m",
        "messages": [{"role": "user", "content": [{"text": {"text": "x", "extensions": {}}}], "extensions": {}}],
        "thinking": {"effort": "high", "extensions": {"unscoped": true}},
        "extensions": {}
    }))?;
    assert_eq!(
        KiroConversationRequestBuilder::build(
            &policy,
            &context,
            "m",
            &unsupported_thinking_extension
        ),
        Err(KiroConversationRequestError::UnsupportedCanonicalField)
    );
    Ok(())
}

#[test]
fn invalid_tool_or_ambient_values_are_never_silently_coerced_or_logged() -> TestResult {
    assert_eq!(
        KiroConversationId::try_new(""),
        Err(KiroConversationRequestError::InvalidConversationId)
    );
    assert_eq!(
        KiroEnvironmentState::try_new("", "/workspace"),
        Err(KiroConversationRequestError::InvalidEnvironmentState)
    );

    let request = decode_request(json!({
        "requested_model": "m",
        "messages": [{"role": "user", "content": [{"text": {"text": "secret prompt", "extensions": {}}}], "extensions": {}}],
        "tools": [{"name": "lookup", "input_schema": ["not", "an", "object"], "extensions": {}}],
        "extensions": {}
    }))?;
    let policy =
        KiroEndpointPolicy::try_new(KiroEndpointKind::Ide, KiroApiRegion::try_new("us-east-1")?)?;
    assert_eq!(
        KiroConversationRequestBuilder::build(&policy, &context()?, "m", &request),
        Err(KiroConversationRequestError::InvalidToolDefinition)
    );

    let valid = decode_request(json!({
        "requested_model": "do-not-log-public-model",
        "messages": [{"role": "user", "content": [{"text": {"text": "do-not-log-prompt", "extensions": {}}}], "extensions": {}}],
        "extensions": {}
    }))?;
    let built = KiroConversationRequestBuilder::build(
        &policy,
        &context()?,
        "do-not-log-selected-model",
        &valid,
    )?;
    let diagnostic = format!(
        "{built:?}{:?}{:?}",
        context()?,
        KiroConversationId::try_new("do-not-log-conversation")?
    );
    for value in [
        "do-not-log-public-model",
        "do-not-log-prompt",
        "do-not-log-selected-model",
        "fixture-conversation-01",
        "/workspace/fixture",
        "do-not-log-conversation",
    ] {
        assert!(!diagnostic.contains(value));
    }
    Ok(())
}
