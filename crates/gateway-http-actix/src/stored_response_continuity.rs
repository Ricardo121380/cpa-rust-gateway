//! Pure gateway-owned stored history replay and compaction helpers.

use std::collections::BTreeMap;

use gateway_core::{
    CanonicalEvent, CanonicalMessage, CanonicalRequest, CanonicalResponse, ErrorScope,
    GatewayError, GatewayErrorCode, MessageContent, MessageRole, RawExtensions, RawJson,
    TextContent, ToolCall,
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
    mut current: CanonicalRequest,
) -> Result<CanonicalRequest, GatewayError> {
    let mut messages = previous.request().messages.clone();
    messages.extend(response_messages(
        &previous
            .canonical_response()
            .map_err(|_| internal_error())?,
    )?);
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
    let mut completed = Vec::new();
    let mut current: Option<MessageBuilder> = None;
    for event in response.events() {
        match event {
            CanonicalEvent::MessageStart(start) => {
                if current.is_some() {
                    return Err(internal_error());
                }
                current = Some(MessageBuilder::new(start.role.clone()));
            }
            CanonicalEvent::TextDelta(delta) => current
                .as_mut()
                .ok_or_else(internal_error)?
                .push_text(&delta.text),
            CanonicalEvent::ToolCallStart(start) => current
                .as_mut()
                .ok_or_else(internal_error)?
                .start_tool(&start.call_id, &start.name)?,
            CanonicalEvent::ToolCallEnd(end) => current
                .as_mut()
                .ok_or_else(internal_error)?
                .finish_tool(&end.call_id, end.arguments.clone())?,
            CanonicalEvent::MessageEnd(_) => {
                let message = current.take().ok_or_else(internal_error)?.finish()?;
                if !message.content.is_empty() {
                    completed.push(message);
                }
            }
            CanonicalEvent::ResponseStart(_)
            | CanonicalEvent::ReasoningDelta(_)
            | CanonicalEvent::ToolCallArgumentsDelta(_)
            | CanonicalEvent::UsageDelta(_)
            | CanonicalEvent::ResponseEnd(_) => {}
            CanonicalEvent::StreamError(_) => return Err(internal_error()),
        }
    }
    if current.is_some() {
        return Err(internal_error());
    }
    Ok(completed)
}

struct MessageBuilder {
    role: MessageRole,
    content: Vec<PendingContent>,
    tools: BTreeMap<String, usize>,
}

enum PendingContent {
    Text(String),
    Tool {
        id: String,
        name: String,
        arguments: Option<RawJson>,
    },
}

impl MessageBuilder {
    fn new(role: MessageRole) -> Self {
        Self {
            role,
            content: Vec::new(),
            tools: BTreeMap::new(),
        }
    }

    fn push_text(&mut self, text: &str) {
        if let Some(PendingContent::Text(existing)) = self.content.last_mut() {
            existing.push_str(text);
        } else {
            self.content.push(PendingContent::Text(text.to_owned()));
        }
    }

    fn start_tool(&mut self, id: &str, name: &str) -> Result<(), GatewayError> {
        if id.is_empty() || name.is_empty() || self.tools.contains_key(id) {
            return Err(internal_error());
        }
        let index = self.content.len();
        self.content.push(PendingContent::Tool {
            id: id.to_owned(),
            name: name.to_owned(),
            arguments: None,
        });
        self.tools.insert(id.to_owned(), index);
        Ok(())
    }

    fn finish_tool(&mut self, id: &str, arguments: RawJson) -> Result<(), GatewayError> {
        let index = self.tools.remove(id).ok_or_else(internal_error)?;
        let Some(PendingContent::Tool {
            arguments: retained,
            ..
        }) = self.content.get_mut(index)
        else {
            return Err(internal_error());
        };
        if retained.replace(arguments).is_some() {
            return Err(internal_error());
        }
        Ok(())
    }

    fn finish(self) -> Result<CanonicalMessage, GatewayError> {
        if !self.tools.is_empty() {
            return Err(internal_error());
        }
        let content = self
            .content
            .into_iter()
            .map(|content| match content {
                PendingContent::Text(text) => Ok(MessageContent::Text(TextContent {
                    text,
                    extensions: RawExtensions::default(),
                })),
                PendingContent::Tool {
                    id,
                    name,
                    arguments: Some(arguments),
                } => Ok(MessageContent::ToolCall(ToolCall {
                    id,
                    name,
                    arguments,
                    extensions: RawExtensions::default(),
                })),
                PendingContent::Tool {
                    arguments: None, ..
                } => Err(internal_error()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CanonicalMessage {
            role: self.role,
            content,
            extensions: RawExtensions::default(),
        })
    }
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
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].role.0, "assistant");
        assert_eq!(replay[0].content.len(), 2);
        let MessageContent::Text(text) = &replay[0].content[0] else {
            return Err("expected replayed text before Tool call".into());
        };
        assert_eq!(text.text, "checking");
        let MessageContent::ToolCall(tool) = &replay[0].content[1] else {
            return Err("expected replayed Tool call".into());
        };
        assert_eq!(tool.id, "call-weather");
        assert_eq!(tool.name, "weather");
        assert_eq!(tool.arguments.get(), r#"{"city":"Jakarta"}"#);
        Ok(())
    }
}
