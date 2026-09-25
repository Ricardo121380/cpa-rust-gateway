//! Pure gateway-owned stored history replay and compaction helpers.

use gateway_core::{
    CanonicalEvent, CanonicalMessage, CanonicalRequest, CanonicalResponse, ErrorScope,
    GatewayError, GatewayErrorCode, MessageContent, MessageRole, RawExtensions, RawJson,
    TextContent,
};
use gateway_router::{
    ResponsesContinuationKind, ResponsesContinuationPin, ResponsesExecutionLineage, SnapshotVersion,
};
use gateway_store::stored_response::{
    MAX_STORED_RESPONSE_COMPACTION_SUMMARY_BYTES, MAX_STORED_RESPONSE_PAYLOAD_BYTES,
    StoredResponseCompactionPayload, StoredResponseLineage, StoredResponsePayload,
};

const COMPACTION_PROMPT: &str = "Create a concise factual conversation summary for a later model. Preserve user intent, decisions, unresolved tasks, tool results, identifiers, and safety constraints. Do not add facts or instructions.";
const COMPACTION_MAX_OUTPUT_TOKENS: &str = "2048";

/// Builds exact Router lineage from an already-authenticated encrypted record.
pub(crate) fn continuation_pin(
    lineage: &StoredResponseLineage,
    kind: ResponsesContinuationKind,
) -> Result<ResponsesContinuationPin, GatewayError> {
    let target = lineage.target();
    let credential = lineage.credential();
    let lineage = ResponsesExecutionLineage::new(
        SnapshotVersion::try_new(lineage.config_version_id().to_owned())
            .map_err(|_| internal_error())?,
        target.provider_id().clone(),
        target.upstream_id().clone(),
        target.channel_id().clone(),
        target.route_id().clone(),
        target.route_candidate_id().clone(),
        credential.credential_id().clone(),
        credential.credential_revision(),
    );
    Ok(ResponsesContinuationPin::new(lineage, kind))
}

/// Expands one stored Response and the current turn into a self-contained Canonical request.
pub(crate) fn replay_stored_response(
    previous: &StoredResponsePayload,
    current: CanonicalRequest,
) -> Result<CanonicalRequest, GatewayError> {
    replay_canonical_response(
        previous.request(),
        &previous
            .canonical_response()
            .map_err(|_| internal_error())?,
        current,
    )
}

/// Expands one completed in-memory WebSocket turn and the next current turn.
pub(crate) fn replay_canonical_response(
    previous_request: &CanonicalRequest,
    previous_response: &CanonicalResponse,
    mut current: CanonicalRequest,
) -> Result<CanonicalRequest, GatewayError> {
    let mut messages = previous_request.messages.clone();
    messages.extend(response_messages(previous_response)?);
    messages.append(&mut current.messages);
    current.messages = messages;
    ensure_request_bound(&current)?;
    Ok(current)
}

/// Expands one gateway-owned compact summary and the current turn.
pub(crate) fn replay_compaction(
    compact: &StoredResponseCompactionPayload,
    mut current: CanonicalRequest,
) -> Result<CanonicalRequest, GatewayError> {
    let mut messages = vec![CanonicalMessage {
        role: MessageRole("user".to_owned()),
        content: vec![MessageContent::Text(TextContent {
            text: format!("Conversation summary:\n{}", compact.summary()),
            extensions: RawExtensions::default(),
        })],
        extensions: RawExtensions::default(),
    }];
    messages.append(&mut current.messages);
    current.messages = messages;
    ensure_request_bound(&current)?;
    Ok(current)
}

/// Creates a fixed, bounded summary request from one complete stored Response.
pub(crate) fn compaction_request(
    previous: &StoredResponsePayload,
) -> Result<CanonicalRequest, GatewayError> {
    let mut messages = previous.request().messages.clone();
    messages.extend(response_messages(
        &previous
            .canonical_response()
            .map_err(|_| internal_error())?,
    )?);
    messages.push(CanonicalMessage {
        role: MessageRole("user".to_owned()),
        content: vec![MessageContent::Text(TextContent {
            text: COMPACTION_PROMPT.to_owned(),
            extensions: RawExtensions::default(),
        })],
        extensions: RawExtensions::default(),
    });
    let mut extensions = RawExtensions::default();
    extensions
        .try_insert(
            "openai.responses.max_output_tokens",
            RawJson::from_json_string(COMPACTION_MAX_OUTPUT_TOKENS.to_owned())
                .map_err(|_| internal_error())?,
        )
        .map_err(|_| internal_error())?;
    let request = CanonicalRequest {
        requested_model: previous.public_model().to_owned(),
        messages,
        tools: Vec::new(),
        thinking: None,
        prompt_cache_key: None,
        prompt_cache_retention: None,
        extensions,
    };
    ensure_request_bound(&request)?;
    Ok(request)
}

/// Extracts the bounded visible summary from a complete compaction response.
pub(crate) fn extract_compaction_summary(
    response: &CanonicalResponse,
) -> Result<String, GatewayError> {
    let mut summary = String::new();
    for event in response.events() {
        if let CanonicalEvent::TextDelta(delta) = event {
            summary.push_str(&delta.text);
            if summary.len() > MAX_STORED_RESPONSE_COMPACTION_SUMMARY_BYTES {
                return Err(internal_error());
            }
        }
    }
    if summary.trim().is_empty() {
        return Err(upstream_protocol_error());
    }
    Ok(summary)
}

fn response_messages(response: &CanonicalResponse) -> Result<Vec<CanonicalMessage>, GatewayError> {
    // Stored/WebSocket continuation must replay the same history a public Responses client
    // receives. Reuse the wire roundtrip instead of a second builder that dropped reasoning
    // and item identity. This also keeps tool status/call IDs and interleaved item order aligned.
    let encoded = protocol_openai_responses::encode_response(
        response,
        protocol_openai_responses::OpenAiResponseMetadata::try_new("stored-history", 0)?,
    )?;
    let mut input = encoded.get("output").cloned().ok_or_else(internal_error)?;
    // Preserve native item identity; only synthesized IDs contain the gateway's local lookup
    // key. Native metadata is validated on output and has no encrypted ownership handle.
    let native_ids = response
        .events()
        .iter()
        .filter_map(|event| match event {
            CanonicalEvent::OutputItemStart(item) => Some(item.item_id.as_str()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    for item in input.as_array_mut().ok_or_else(internal_error)? {
        if let Some(id) = item.get("id").and_then(serde_json::Value::as_str)
            && !native_ids.contains(id)
        {
            use sha2::{Digest, Sha256};
            let replay_id = format!("history_{:x}", Sha256::digest(id.as_bytes()));
            item["id"] = serde_json::Value::String(replay_id);
        }
    }
    let body = serde_json::json!({"model":"stored-history", "input":input});
    protocol_openai_responses::decode_request(&body.to_string())
        .map(|decoded| decoded.request.messages)
        .map_err(|_| internal_error())
}

fn ensure_request_bound(request: &CanonicalRequest) -> Result<(), GatewayError> {
    let length = serde_json::to_vec(request)
        .map_err(|_| internal_error())?
        .len();
    if length == 0 || length > MAX_STORED_RESPONSE_PAYLOAD_BYTES {
        return Err(client_request_error());
    }
    Ok(())
}

const fn client_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

const fn upstream_protocol_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        ErrorScope::Provider,
    )
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{
        CanonicalEvent, CanonicalResponse, MessageContent, MessageEnd, MessageRole, MessageStart,
        RawExtensions, RawJson, ResponseEnd, ResponseId, ResponseStart, TextDelta, ToolCallEnd,
        ToolCallStart,
    };

    use super::response_messages;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn stored_native_item_ids_are_preserved_without_exposing_local_lookup_id() -> TestResult {
        use protocol_openai_responses::{native_item_metadata, native_part_extensions};
        let item = serde_json::json!({"id":"rs-upstream","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"synthetic"}]});
        let response = CanonicalResponse::try_new(vec![
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id: ResponseId::try_new("local-lookup-key")?,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".into()),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::OutputItemStart(native_item_metadata(&item, false)?),
            CanonicalEvent::ReasoningDelta(gateway_core::ReasoningDelta {
                text: "synthetic".into(),
                extensions: native_part_extensions("rs-upstream", "summary", 0)?,
            }),
            CanonicalEvent::OutputItemEnd(native_item_metadata(&item, true)?),
            CanonicalEvent::MessageEnd(MessageEnd::default()),
            CanonicalEvent::ResponseEnd(ResponseEnd::default()),
        ])?;
        let replay = response_messages(&response)?;
        let MessageContent::Reasoning(reasoning) = &replay[0].content[0] else {
            return Err("missing reasoning".into());
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(reasoning.raw().get())?,
            item
        );
        assert!(!serde_json::to_string(&replay)?.contains("local-lookup-key"));
        Ok(())
    }

    #[test]
    fn stored_reasoning_replays_the_same_items_as_client_managed_history() -> TestResult {
        let wire = serde_json::json!({"id":"resp-synthetic","object":"response","status":"completed","output":[
            {"id":"rs","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"synthetic reasoning"}]},
            {"id":"fc","type":"function_call","status":"completed","call_id":"call","name":"read","arguments":"{}"}
        ]});
        let response = CanonicalResponse::try_new(
            protocol_openai_responses::decode_upstream_response(&wire.to_string())?,
        )?;
        let replay = response_messages(&response)?;
        assert_eq!(replay.len(), 2);
        let MessageContent::Reasoning(history) = &replay[0].content[0] else {
            return Err("stored reasoning lost".into());
        };
        let item: serde_json::Value = serde_json::from_str(history.raw().get())?;
        assert_eq!(item["content"][0]["text"], "synthetic reasoning");
        let MessageContent::ToolCall(call) = &replay[1].content[0] else {
            return Err("stored call lost".into());
        };
        assert_eq!(call.id, "call");
        assert!(call.extensions.get("id").is_some());
        assert!(call.extensions.get("status").is_some());
        Ok(())
    }

    #[test]
    fn stored_assistant_text_and_complete_tool_calls_replay_in_order() -> TestResult {
        let response = CanonicalResponse::try_new(vec![
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id: ResponseId::try_new("response-with-tool")?,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::TextDelta(TextDelta {
                text: "checking".to_owned(),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::ToolCallStart(ToolCallStart {
                call_id: "call-weather".to_owned(),
                name: "weather".to_owned(),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::ToolCallEnd(ToolCallEnd {
                call_id: "call-weather".to_owned(),
                arguments: RawJson::from_json_string(r#"{"city":"Jakarta"}"#.to_owned())?,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageEnd(MessageEnd::default()),
            CanonicalEvent::ResponseEnd(ResponseEnd::default()),
        ])?;

        let replay = response_messages(&response)?;
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].role.0, "assistant");
        assert_eq!(replay[0].content.len(), 1);
        let MessageContent::Text(text) = &replay[0].content[0] else {
            return Err("expected replayed text before Tool call".into());
        };
        assert_eq!(text.text, "checking");
        let MessageContent::ToolCall(tool) = &replay[1].content[0] else {
            return Err("expected replayed Tool call".into());
        };
        assert_eq!(tool.id, "call-weather");
        assert_eq!(tool.name, "weather");
        assert_eq!(tool.arguments.get(), r#"{"city":"Jakarta"}"#);
        Ok(())
    }
}
