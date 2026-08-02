//! Target-protocol projection for already validated Canonical responses.
//!
//! Upstream codecs retain every typed Usage counter and stop reason they can prove. Client
//! protocols do not all expose the same vocabulary, so this boundary narrows only fields whose
//! aggregate remains represented and rejects semantic content that the target cannot carry.

use std::fmt;

use gateway_core::{CanonicalEvent, CanonicalEventState, CanonicalResponse, ResponseEnd};

use crate::ProtocolFormat;

/// Stable, value-free reason why a Canonical response cannot be encoded for one client protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolResponseRejection {
    /// Chat Completions has no supported public Reasoning content representation.
    ReasoningUnsupported,
    /// An event or Usage value carries an opaque extension.
    UnknownExtensions,
    /// The target protocol cannot represent the supplied successful stop reason.
    StopReasonUnsupported,
    /// The target protocol cannot retain the supplied stop sequence.
    StopSequenceUnsupported,
    /// Target-specific projection unexpectedly violated the Canonical lifecycle.
    InvalidCanonicalLifecycle,
}

/// Stateful target projection for one streamed Canonical response.
///
/// Both the upstream sequence and every emitted target sequence are validated transactionally.
/// An error leaves both lifecycle states unchanged, allowing the caller to terminate without
/// observing a half-applied Tool, Usage, or terminal transition.
#[derive(Clone)]
pub struct ProtocolResponseProjector {
    target: ProtocolFormat,
    input_state: CanonicalEventState,
    output_state: CanonicalEventState,
}

impl ProtocolResponseProjector {
    /// Creates an empty projector for one fixed client protocol.
    #[must_use]
    pub fn new(target: ProtocolFormat) -> Self {
        Self {
            target,
            input_state: CanonicalEventState::default(),
            output_state: CanonicalEventState::default(),
        }
    }

    /// Validates and projects one event, returning `None` only for a non-final Usage snapshot that
    /// Chat cannot encode and whose counters remain owed by the final snapshot.
    ///
    /// # Errors
    ///
    /// Returns a stable rejection without committing either lifecycle state.
    pub fn project_event(
        &mut self,
        event: &CanonicalEvent,
    ) -> Result<Option<CanonicalEvent>, ProtocolResponseRejection> {
        let mut next_input = self.input_state.clone();
        next_input
            .apply(event)
            .map_err(|_| ProtocolResponseRejection::InvalidCanonicalLifecycle)?;
        let projected = project_event(event, self.target)?;
        let mut next_output = self.output_state.clone();
        if let Some(projected) = &projected {
            next_output
                .apply(projected)
                .map_err(|_| ProtocolResponseRejection::InvalidCanonicalLifecycle)?;
        }
        self.input_state = next_input;
        self.output_state = next_output;
        Ok(projected)
    }
}

impl fmt::Debug for ProtocolResponseProjector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtocolResponseProjector")
            .field("target", &self.target)
            .field("input_state", &self.input_state)
            .field("output_state", &self.output_state)
            .finish()
    }
}

impl fmt::Display for ProtocolResponseRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProtocolResponseRejection {}

/// Narrows one successful Canonical response to the exact target protocol vocabulary.
///
/// Usage projection preserves input/output totals while removing only provider-specific detail
/// counters the target cannot name. Reasoning content is never folded into visible Chat text.
///
/// # Errors
///
/// Returns a stable rejection for opaque extensions, unsupported Reasoning, stop reason, stop
/// sequence, or an internal lifecycle mismatch.
pub fn project_protocol_response(
    response: &CanonicalResponse,
    target: ProtocolFormat,
) -> Result<CanonicalResponse, ProtocolResponseRejection> {
    let mut projector = ProtocolResponseProjector::new(target);
    let mut projected = Vec::with_capacity(response.events().len());
    for event in response.events() {
        if let Some(event) = projector.project_event(event)? {
            projected.push(event);
        }
    }
    CanonicalResponse::try_new(projected)
        .map_err(|_| ProtocolResponseRejection::InvalidCanonicalLifecycle)
}

fn project_event(
    event: &CanonicalEvent,
    target: ProtocolFormat,
) -> Result<Option<CanonicalEvent>, ProtocolResponseRejection> {
    reject_extensions(event)?;
    match event {
        CanonicalEvent::ReasoningDelta(_) if target == ProtocolFormat::OpenAiChatCompletions => {
            Err(ProtocolResponseRejection::ReasoningUnsupported)
        }
        CanonicalEvent::UsageDelta(delta) => {
            let mut delta = delta.clone();
            match target {
                ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses => {
                    delta.usage.cache_read_tokens = None;
                    delta.usage.cache_creation_tokens = None;
                }
                ProtocolFormat::AnthropicMessages => {
                    delta.usage.reasoning_tokens = None;
                    delta.usage.cached_tokens = None;
                }
            }
            // Chat's usage object is terminal and requires both aggregate counts. Earlier partial
            // snapshots have no Chat representation and carry no counter absent from the later
            // final snapshot.
            Ok(
                (target != ProtocolFormat::OpenAiChatCompletions || delta.is_final)
                    .then_some(CanonicalEvent::UsageDelta(delta)),
            )
        }
        CanonicalEvent::ResponseEnd(end) => Ok(Some(CanonicalEvent::ResponseEnd(
            project_response_end(end, target)?,
        ))),
        _ => Ok(Some(event.clone())),
    }
}

fn reject_extensions(event: &CanonicalEvent) -> Result<(), ProtocolResponseRejection> {
    let empty = match event {
        CanonicalEvent::ResponseStart(value) => value.extensions.is_empty(),
        CanonicalEvent::MessageStart(value) => value.extensions.is_empty(),
        CanonicalEvent::TextDelta(value) => value.extensions.is_empty(),
        CanonicalEvent::ReasoningDelta(value) => value.extensions.is_empty(),
        CanonicalEvent::ToolCallStart(value) => value.extensions.is_empty(),
        CanonicalEvent::ToolCallArgumentsDelta(value) => value.extensions.is_empty(),
        CanonicalEvent::ToolCallEnd(value) => value.extensions.is_empty(),
        CanonicalEvent::UsageDelta(value) => {
            value.extensions.is_empty() && value.usage.extensions.is_empty()
        }
        CanonicalEvent::MessageEnd(value) => value.extensions.is_empty(),
        CanonicalEvent::ResponseEnd(value) => value.extensions.is_empty(),
        CanonicalEvent::StreamError(_) => true,
    };
    if empty {
        Ok(())
    } else {
        Err(ProtocolResponseRejection::UnknownExtensions)
    }
}

fn project_response_end(
    end: &ResponseEnd,
    target: ProtocolFormat,
) -> Result<ResponseEnd, ProtocolResponseRejection> {
    if end.stop_sequence.is_some() && target != ProtocolFormat::AnthropicMessages {
        return Err(ProtocolResponseRejection::StopSequenceUnsupported);
    }
    let reason = end
        .stop_reason
        .as_deref()
        .ok_or(ProtocolResponseRejection::StopReasonUnsupported)?;
    let reason = match target {
        ProtocolFormat::OpenAiChatCompletions => match reason {
            "end_turn" | "stop" => "stop",
            "tool_use" | "tool_calls" => "tool_calls",
            "max_tokens" | "length" => "length",
            "refusal" | "content_filter" => "content_filter",
            _ => return Err(ProtocolResponseRejection::StopReasonUnsupported),
        },
        ProtocolFormat::OpenAiResponses => match reason {
            "end_turn" | "stop" => "end_turn",
            "tool_use" | "tool_calls" => "tool_use",
            "max_tokens" | "length" => "max_tokens",
            "refusal" | "content_filter" => "refusal",
            _ => return Err(ProtocolResponseRejection::StopReasonUnsupported),
        },
        ProtocolFormat::AnthropicMessages => match reason {
            "end_turn" | "stop" => "end_turn",
            "tool_use" | "tool_calls" => "tool_use",
            "max_tokens" | "length" => "max_tokens",
            "refusal" | "content_filter" => "refusal",
            "stop_sequence" => "stop_sequence",
            "model_context_window_exceeded" => "model_context_window_exceeded",
            "pause_turn" => "pause_turn",
            _ => return Err(ProtocolResponseRejection::StopReasonUnsupported),
        },
    };
    Ok(ResponseEnd {
        stop_reason: Some(reason.to_owned()),
        stop_sequence: end.stop_sequence.clone(),
        extensions: end.extensions.clone(),
    })
}

#[cfg(test)]
mod tests {
    use gateway_core::{
        CanonicalEvent, CanonicalResponse, ErrorScope, GatewayError, GatewayErrorCode, MessageEnd,
        MessageRole, MessageStart, RawExtensions, ReasoningDelta, ResponseEnd, ResponseId,
        ResponseStart, StreamError, TextDelta, Usage, UsageDelta,
    };
    use protocol_anthropic::{
        AnthropicMessagesSseEncoder, AnthropicResponseMetadata,
        decode_upstream_response as decode_anthropic, encode_response as encode_anthropic,
    };
    use protocol_openai_chat::{
        ChatResponseMetadata, ChatSseEncoder, decode_upstream_response as decode_chat,
        encode_response as encode_chat,
    };
    use protocol_openai_responses::{
        OpenAiResponseMetadata, OpenAiResponsesSseEncoder,
        decode_upstream_response as decode_responses, encode_response as encode_responses,
    };

    use super::{ProtocolResponseProjector, ProtocolResponseRejection, project_protocol_response};
    use crate::ProtocolFormat;

    fn response(reasoning: bool) -> Result<CanonicalResponse, Box<dyn std::error::Error>> {
        let mut events = vec![
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id: ResponseId::try_new("fixture-response")?,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::UsageDelta(UsageDelta {
                usage: Usage {
                    input_tokens: Some(10),
                    cache_read_tokens: Some(3),
                    cache_creation_tokens: Some(2),
                    cached_tokens: Some(4),
                    ..Usage::default()
                },
                is_final: false,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }),
        ];
        if reasoning {
            events.push(CanonicalEvent::ReasoningDelta(ReasoningDelta {
                text: "fixture reasoning".to_owned(),
                extensions: RawExtensions::default(),
            }));
        }
        events.extend([
            CanonicalEvent::TextDelta(TextDelta {
                text: "fixture answer".to_owned(),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageEnd(MessageEnd::default()),
            CanonicalEvent::UsageDelta(UsageDelta {
                usage: Usage {
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                    reasoning_tokens: Some(1),
                    cache_read_tokens: Some(3),
                    cache_creation_tokens: Some(2),
                    cached_tokens: Some(4),
                    ..Usage::default()
                },
                is_final: true,
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::ResponseEnd(ResponseEnd {
                stop_reason: Some("end_turn".to_owned()),
                stop_sequence: None,
                extensions: RawExtensions::default(),
            }),
        ]);
        Ok(CanonicalResponse::try_new(events)?)
    }

    #[test]
    fn target_usage_projection_preserves_aggregate_counts() -> Result<(), Box<dyn std::error::Error>>
    {
        for target in [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
        ] {
            let projected = project_protocol_response(&response(false)?, target)?;
            let usage = projected
                .events()
                .iter()
                .find_map(|event| match event {
                    CanonicalEvent::UsageDelta(delta) if delta.is_final => Some(&delta.usage),
                    _ => None,
                })
                .ok_or_else(|| std::io::Error::other("missing final usage"))?;
            assert_eq!(usage.input_tokens, Some(10));
            assert_eq!(usage.output_tokens, Some(5));
            match target {
                ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses => {
                    assert!(usage.cache_read_tokens.is_none());
                    assert!(usage.cache_creation_tokens.is_none());
                }
                ProtocolFormat::AnthropicMessages => {
                    assert!(usage.reasoning_tokens.is_none());
                    assert!(usage.cached_tokens.is_none());
                }
            }
        }
        Ok(())
    }

    #[test]
    fn reasoning_never_degrades_into_chat_text() -> Result<(), Box<dyn std::error::Error>> {
        let response = response(true)?;
        assert_eq!(
            project_protocol_response(&response, ProtocolFormat::OpenAiChatCompletions),
            Err(ProtocolResponseRejection::ReasoningUnsupported)
        );
        assert!(project_protocol_response(&response, ProtocolFormat::OpenAiResponses).is_ok());
        assert!(project_protocol_response(&response, ProtocolFormat::AnthropicMessages).is_ok());
        Ok(())
    }

    #[test]
    fn stateful_projection_matches_finite_projection() -> Result<(), Box<dyn std::error::Error>> {
        for target in [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
        ] {
            let source = response(false)?;
            let expected = project_protocol_response(&source, target)?;
            let mut projector = ProtocolResponseProjector::new(target);
            let actual = source
                .events()
                .iter()
                .filter_map(|event| projector.project_event(event).transpose())
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(actual, expected.events());
        }
        Ok(())
    }

    #[test]
    fn rejected_stream_event_does_not_mutate_projector_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut projector = ProtocolResponseProjector::new(ProtocolFormat::OpenAiChatCompletions);
        for event in response(false)?.events().iter().take(3) {
            let _ = projector.project_event(event)?;
        }
        let reasoning = CanonicalEvent::ReasoningDelta(ReasoningDelta {
            text: "must remain private".to_owned(),
            extensions: RawExtensions::default(),
        });
        assert_eq!(
            projector.project_event(&reasoning),
            Err(ProtocolResponseRejection::ReasoningUnsupported)
        );
        let text = CanonicalEvent::TextDelta(TextDelta {
            text: "still valid after rejection".to_owned(),
            extensions: RawExtensions::default(),
        });
        assert_eq!(projector.project_event(&text)?, Some(text));
        Ok(())
    }

    #[test]
    fn stream_error_passes_through_as_terminal() -> Result<(), Box<dyn std::error::Error>> {
        let mut projector = ProtocolResponseProjector::new(ProtocolFormat::OpenAiResponses);
        let start = CanonicalEvent::ResponseStart(ResponseStart {
            response_id: ResponseId::try_new("failed-response")?,
            extensions: RawExtensions::default(),
        });
        assert_eq!(projector.project_event(&start)?, Some(start));
        let terminal = CanonicalEvent::StreamError(StreamError {
            error: GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider),
        });
        assert_eq!(projector.project_event(&terminal)?, Some(terminal));
        assert_eq!(
            projector.project_event(&CanonicalEvent::MessageEnd(MessageEnd::default())),
            Err(ProtocolResponseRejection::InvalidCanonicalLifecycle)
        );
        Ok(())
    }

    #[test]
    fn all_nine_decoded_response_pairs_reach_their_real_target_encoder()
    -> Result<(), Box<dyn std::error::Error>> {
        let sources = [
            CanonicalResponse::try_new(decode_chat(
                r#"{
                  "id":"chat_fixture","object":"chat.completion",
                  "choices":[{"index":0,"message":{"role":"assistant","content":"fixture answer"},"finish_reason":"stop"}],
                  "usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}
                }"#,
            )?)?,
            CanonicalResponse::try_new(decode_responses(
                r#"{
                  "id":"responses_fixture","object":"response","status":"completed",
                  "error":null,"incomplete_details":null,
                  "output":[{"id":"message_fixture","type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"fixture answer","annotations":[],"logprobs":[]}]}],
                  "usage":{"input_tokens":10,"output_tokens":5,"total_tokens":15}
                }"#,
            )?)?,
            CanonicalResponse::try_new(decode_anthropic(
                r#"{
                  "id":"messages_fixture","type":"message","role":"assistant","model":"fixture-model",
                  "content":[{"type":"text","text":"fixture answer"}],
                  "stop_reason":"end_turn","stop_sequence":null,
                  "usage":{"input_tokens":10,"output_tokens":5}
                }"#,
            )?)?,
        ];

        for source in &sources {
            for target in [
                ProtocolFormat::OpenAiChatCompletions,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::AnthropicMessages,
            ] {
                let projected = project_protocol_response(source, target)?;
                match target {
                    ProtocolFormat::OpenAiChatCompletions => {
                        let _ = encode_chat(
                            &projected,
                            ChatResponseMetadata::try_new("fixture-model", 1, true)?,
                        )?;
                        let mut stream_encoder = ChatSseEncoder::new(
                            ChatResponseMetadata::try_new("fixture-model", 1, true)?,
                        );
                        for event in projected.events() {
                            let _ = stream_encoder.encode_event(event)?;
                        }
                    }
                    ProtocolFormat::OpenAiResponses => {
                        let _ = encode_responses(
                            &projected,
                            OpenAiResponseMetadata::try_new("fixture-model", 1)?,
                        )?;
                        let mut stream_encoder = OpenAiResponsesSseEncoder::new(
                            OpenAiResponseMetadata::try_new("fixture-model", 1)?,
                        );
                        for event in projected.events() {
                            let _ = stream_encoder.encode_event(event)?;
                        }
                    }
                    ProtocolFormat::AnthropicMessages => {
                        let _ = encode_anthropic(
                            &projected,
                            AnthropicResponseMetadata::try_new("fixture-model")?,
                        )?;
                        let mut stream_encoder = AnthropicMessagesSseEncoder::new(
                            AnthropicResponseMetadata::try_new("fixture-model")?,
                        );
                        for event in projected.events() {
                            let _ = stream_encoder.encode_event(event)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
