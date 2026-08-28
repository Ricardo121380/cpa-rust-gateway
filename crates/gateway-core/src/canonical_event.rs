//! Canonical outbound events and their protocol-neutral lifecycle validation.

use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    ErrorScope, GatewayError, GatewayErrorCode, MessageRole, RawExtensions, RawJson, ResponseId,
};

/// One protocol-neutral event emitted while constructing a gateway response.
///
/// The externally tagged JSON representation keeps each payload structurally explicit and leaves
/// protocol adapters responsible for their own wire encoding.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalEvent {
    /// Begins one client-visible response.
    ResponseStart(ResponseStart),
    /// Begins one assistant or other protocol-neutral output message.
    MessageStart(MessageStart),
    /// Appends visible text to the active message.
    TextDelta(TextDelta),
    /// Appends provider reasoning text to the active message.
    ReasoningDelta(ReasoningDelta),
    /// Declares one Tool call in the active message.
    ToolCallStart(ToolCallStart),
    /// Appends one already-decoded fragment of Tool arguments.
    ToolCallArgumentsDelta(ToolCallArgumentsDelta),
    /// Completes one Tool call with its fully assembled JSON arguments.
    ToolCallEnd(ToolCallEnd),
    /// Reports a partial or final token-usage snapshot.
    UsageDelta(UsageDelta),
    /// Ends the active output message.
    MessageEnd(MessageEnd),
    /// Ends one successfully completed response.
    ResponseEnd(ResponseEnd),
    /// Terminates a response that cannot complete normally.
    StreamError(StreamError),
}

impl fmt::Debug for CanonicalEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResponseStart(_) => {
                formatter.write_str("CanonicalEvent::ResponseStart(<redacted>)")
            }
            Self::MessageStart(_) => {
                formatter.write_str("CanonicalEvent::MessageStart(<redacted>)")
            }
            Self::TextDelta(_) => formatter.write_str("CanonicalEvent::TextDelta(<redacted>)"),
            Self::ReasoningDelta(_) => {
                formatter.write_str("CanonicalEvent::ReasoningDelta(<redacted>)")
            }
            Self::ToolCallStart(_) => {
                formatter.write_str("CanonicalEvent::ToolCallStart(<redacted>)")
            }
            Self::ToolCallArgumentsDelta(_) => {
                formatter.write_str("CanonicalEvent::ToolCallArgumentsDelta(<redacted>)")
            }
            Self::ToolCallEnd(_) => formatter.write_str("CanonicalEvent::ToolCallEnd(<redacted>)"),
            Self::UsageDelta(_) => formatter.write_str("CanonicalEvent::UsageDelta(<redacted>)"),
            Self::MessageEnd(_) => formatter.write_str("CanonicalEvent::MessageEnd(<redacted>)"),
            Self::ResponseEnd(_) => formatter.write_str("CanonicalEvent::ResponseEnd(<redacted>)"),
            Self::StreamError(_) => {
                formatter.write_str("CanonicalEvent::StreamError(<safe error>)")
            }
        }
    }
}

/// Begins one client-visible response.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseStart {
    /// Opaque identifier retained for the response lifecycle.
    pub response_id: ResponseId,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for ResponseStart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseStart")
            .field("response_id", &"<redacted>")
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// Begins one output message.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageStart {
    /// Role carried by the output message without freezing a protocol enum.
    pub role: MessageRole,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for MessageStart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageStart")
            .field("role", &"<redacted>")
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// A visible non-empty text fragment for the active message.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextDelta {
    /// Text fragment retained without protocol-specific framing.
    pub text: String,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for TextDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextDelta")
            .field("text", &"<redacted>")
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// A non-empty reasoning fragment for the active message.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasoningDelta {
    /// Reasoning fragment retained separately from visible text.
    pub text: String,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for ReasoningDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReasoningDelta")
            .field("text", &"<redacted>")
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// Declares one Tool call inside the active message.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCallStart {
    /// Stable Tool call correlation identifier.
    pub call_id: String,
    /// Client-visible Tool name.
    pub name: String,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for ToolCallStart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolCallStart")
            .field("call_id", &"<redacted>")
            .field("name", &"<redacted>")
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// An already-decoded argument fragment for one open Tool call.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCallArgumentsDelta {
    /// Correlation identifier of the Tool call receiving this fragment.
    pub call_id: String,
    /// Argument fragment retained without JSON assembly or normalization.
    pub delta: String,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for ToolCallArgumentsDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolCallArgumentsDelta")
            .field("call_id", &"<redacted>")
            .field("delta", &"<redacted>")
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// Completes one Tool call with fully assembled valid JSON arguments.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCallEnd {
    /// Correlation identifier of the completed Tool call.
    pub call_id: String,
    /// Complete valid JSON arguments supplied by a later protocol decoder.
    pub arguments: RawJson,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for ToolCallEnd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolCallEnd")
            .field("call_id", &"<redacted>")
            .field("arguments", &self.arguments)
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// Token usage values reported by an upstream without core-side estimation.
#[derive(Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    /// Input tokens, when reported by an upstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Output tokens, when reported by an upstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Reasoning tokens, when reported by an upstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    /// Cache-read tokens, when reported by an upstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    /// Cache-creation tokens, when reported by an upstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u64>,
    /// Cached tokens for providers that expose only that aggregate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u64>,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for Usage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Usage")
            .field("input_tokens_reported", &self.input_tokens.is_some())
            .field("output_tokens_reported", &self.output_tokens.is_some())
            .field(
                "reasoning_tokens_reported",
                &self.reasoning_tokens.is_some(),
            )
            .field(
                "cache_read_tokens_reported",
                &self.cache_read_tokens.is_some(),
            )
            .field(
                "cache_creation_tokens_reported",
                &self.cache_creation_tokens.is_some(),
            )
            .field("cached_tokens_reported", &self.cached_tokens.is_some())
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// One partial or final usage report in a response lifecycle.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageDelta {
    /// Current upstream-reported usage snapshot.
    pub usage: Usage,
    /// Whether this is the one final usage report for the response.
    #[serde(default)]
    pub is_final: bool,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for UsageDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UsageDelta")
            .field("usage", &self.usage)
            .field("is_final", &self.is_final)
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// Ends the active output message.
#[derive(Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageEnd {
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for MessageEnd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageEnd")
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// Ends one successfully completed response.
#[derive(Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseEnd {
    /// Explicit upstream completion reason when the protocol exposes one.
    ///
    /// The open string preserves protocol-owned labels such as `end_turn`, `tool_use`, or
    /// `max_tokens` without making the Canonical core depend on one vendor's closed enum. `None`
    /// remains valid for protocol surfaces that do not expose a stop reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Client-visible stop sequence when the explicit completion reason carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    /// Provider- or protocol-specific fields retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for ResponseEnd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseEnd")
            .field("stop_reason_reported", &self.stop_reason.is_some())
            .field("stop_sequence_reported", &self.stop_sequence.is_some())
            .field("extensions", &self.extensions)
            .finish()
    }
}

/// Safely terminates a response that cannot complete normally.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamError {
    /// Stable, secret-free error classification for the terminated response.
    pub error: GatewayError,
}

/// Validates one ordered canonical response event sequence.
#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalEventState {
    lifecycle: ResponseLifecycle,
}

impl Default for CanonicalEventState {
    fn default() -> Self {
        Self {
            lifecycle: ResponseLifecycle::Initial,
        }
    }
}

impl fmt::Debug for CanonicalEventState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.lifecycle {
            ResponseLifecycle::Initial => formatter.write_str("CanonicalEventState::Initial"),
            ResponseLifecycle::Open(open) => formatter
                .debug_struct("CanonicalEventState::Open")
                .field("message_open", &open.message_open)
                .field("tool_call_count", &open.tool_calls.len())
                .field("final_usage_seen", &(open.usage == UsageLifecycle::Final))
                .finish(),
            ResponseLifecycle::Completed => formatter.write_str("CanonicalEventState::Completed"),
            ResponseLifecycle::Failed(error) => formatter
                .debug_tuple("CanonicalEventState::Failed")
                .field(error)
                .finish(),
        }
    }
}

impl CanonicalEventState {
    /// Applies one event while preserving the state if the event is invalid.
    ///
    /// # Errors
    ///
    /// Returns an `UpstreamProtocolError` scoped to `Stream` for an invalid sequence and a
    /// `StreamTruncated` scoped to `Stream` when normal termination leaves work unclosed.
    pub fn apply(&mut self, event: &CanonicalEvent) -> Result<(), GatewayError> {
        if self.is_terminal() {
            return Err(stream_protocol_error());
        }

        if matches!(&self.lifecycle, ResponseLifecycle::Initial) {
            if matches!(event, CanonicalEvent::ResponseStart(_)) {
                self.lifecycle = ResponseLifecycle::Open(OpenResponse::default());
                return Ok(());
            }

            return Err(stream_protocol_error());
        }

        if let CanonicalEvent::StreamError(stream_error) = event {
            self.lifecycle = ResponseLifecycle::Failed(stream_error.error.clone());
            return Ok(());
        }

        if let CanonicalEvent::ResponseEnd(response_end) = event {
            let ResponseLifecycle::Open(open) = &self.lifecycle else {
                return Err(stream_protocol_error());
            };
            if !open.is_ready_to_end() {
                return Err(stream_truncated_error());
            }
            if response_end
                .stop_reason
                .as_deref()
                .is_some_and(str::is_empty)
                || response_end
                    .stop_sequence
                    .as_deref()
                    .is_some_and(str::is_empty)
            {
                return Err(stream_protocol_error());
            }

            self.lifecycle = ResponseLifecycle::Completed;
            return Ok(());
        }

        if matches!(event, CanonicalEvent::ResponseStart(_)) {
            return Err(stream_protocol_error());
        }

        let ResponseLifecycle::Open(open) = &mut self.lifecycle else {
            return Err(stream_protocol_error());
        };
        open.apply(event)
    }

    /// Verifies that an input source ended only after a terminal canonical event.
    ///
    /// # Errors
    ///
    /// Returns `StreamTruncated` scoped to `Stream` when the source ends before `ResponseEnd` or
    /// `StreamError`.
    pub fn finish(&self) -> Result<(), GatewayError> {
        if self.is_terminal() {
            Ok(())
        } else {
            Err(stream_truncated_error())
        }
    }

    /// Returns whether the response reached a normal or error terminal event.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            &self.lifecycle,
            ResponseLifecycle::Completed | ResponseLifecycle::Failed(_)
        )
    }

    /// Returns whether the response reached `ResponseEnd` rather than `StreamError`.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(&self.lifecycle, ResponseLifecycle::Completed)
    }

    fn require_success(&self) -> Result<(), GatewayError> {
        match &self.lifecycle {
            ResponseLifecycle::Completed => Ok(()),
            ResponseLifecycle::Failed(error) => Err(error.clone()),
            ResponseLifecycle::Initial | ResponseLifecycle::Open(_) => {
                Err(stream_truncated_error())
            }
        }
    }
}

/// A finite, already-validated successful canonical response.
///
/// This is a non-streaming envelope over a caller-provided finite sequence. Bounded transport and
/// delivery remain the responsibility of P1-04 and later.
#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalResponse {
    events: Vec<CanonicalEvent>,
}

impl fmt::Debug for CanonicalResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalResponse")
            .field("event_count", &self.events.len())
            .finish()
    }
}

impl CanonicalResponse {
    /// Validates and retains a finite response that ends successfully.
    ///
    /// # Errors
    ///
    /// Returns a state-machine error for an invalid or incomplete sequence. A sequence terminated
    /// by `StreamError` returns the safe error carried by that terminal event.
    pub fn try_new(events: Vec<CanonicalEvent>) -> Result<Self, GatewayError> {
        let mut state = CanonicalEventState::default();
        for event in &events {
            state.apply(event)?;
        }
        state.require_success()?;

        Ok(Self { events })
    }

    /// Returns the validated events in their semantic order.
    #[must_use]
    pub fn events(&self) -> &[CanonicalEvent] {
        &self.events
    }

    /// Consumes the response and returns its validated event sequence.
    #[must_use]
    pub fn into_events(self) -> Vec<CanonicalEvent> {
        self.events
    }
}

#[derive(Clone, Eq, PartialEq)]
enum ResponseLifecycle {
    Initial,
    Open(OpenResponse),
    Completed,
    Failed(GatewayError),
}

#[derive(Clone, Default, Eq, PartialEq)]
struct OpenResponse {
    message_open: bool,
    tool_calls: BTreeMap<String, ToolCallLifecycle>,
    usage: UsageLifecycle,
}

impl OpenResponse {
    fn apply(&mut self, event: &CanonicalEvent) -> Result<(), GatewayError> {
        match event {
            CanonicalEvent::MessageStart(_) => {
                if self.message_open {
                    return Err(stream_protocol_error());
                }

                self.message_open = true;
                Ok(())
            }
            CanonicalEvent::TextDelta(delta) => {
                if !self.message_open || delta.text.is_empty() {
                    return Err(stream_protocol_error());
                }

                Ok(())
            }
            CanonicalEvent::ReasoningDelta(delta) => {
                if !self.message_open || delta.text.is_empty() {
                    return Err(stream_protocol_error());
                }

                Ok(())
            }
            CanonicalEvent::ToolCallStart(start) => self.start_tool_call(start),
            CanonicalEvent::ToolCallArgumentsDelta(delta) => self.append_tool_arguments(delta),
            CanonicalEvent::ToolCallEnd(end) => self.end_tool_call(end),
            CanonicalEvent::UsageDelta(delta) => self.record_usage(delta),
            CanonicalEvent::MessageEnd(_) => self.end_message(),
            CanonicalEvent::ResponseStart(_)
            | CanonicalEvent::ResponseEnd(_)
            | CanonicalEvent::StreamError(_) => Err(stream_protocol_error()),
        }
    }

    fn start_tool_call(&mut self, start: &ToolCallStart) -> Result<(), GatewayError> {
        if !self.message_open
            || start.call_id.is_empty()
            || start.name.is_empty()
            || self.tool_calls.contains_key(&start.call_id)
        {
            return Err(stream_protocol_error());
        }

        self.tool_calls
            .insert(start.call_id.clone(), ToolCallLifecycle::Declared);
        Ok(())
    }

    fn append_tool_arguments(
        &mut self,
        delta: &ToolCallArgumentsDelta,
    ) -> Result<(), GatewayError> {
        if !self.message_open {
            return Err(stream_protocol_error());
        }

        let Some(lifecycle) = self.tool_calls.get_mut(&delta.call_id) else {
            return Err(stream_protocol_error());
        };
        match lifecycle {
            ToolCallLifecycle::Declared => {
                *lifecycle = ToolCallLifecycle::ArgumentsStreaming;
                Ok(())
            }
            ToolCallLifecycle::ArgumentsStreaming => Ok(()),
            ToolCallLifecycle::Emitted => Err(stream_protocol_error()),
        }
    }

    fn end_tool_call(&mut self, end: &ToolCallEnd) -> Result<(), GatewayError> {
        if !self.message_open {
            return Err(stream_protocol_error());
        }

        let Some(lifecycle) = self.tool_calls.get_mut(&end.call_id) else {
            return Err(stream_protocol_error());
        };
        match lifecycle {
            ToolCallLifecycle::Declared | ToolCallLifecycle::ArgumentsStreaming => {
                *lifecycle = ToolCallLifecycle::Emitted;
                Ok(())
            }
            ToolCallLifecycle::Emitted => Err(stream_protocol_error()),
        }
    }

    fn record_usage(&mut self, delta: &UsageDelta) -> Result<(), GatewayError> {
        if self.usage == UsageLifecycle::Final {
            return Err(stream_protocol_error());
        }

        self.usage = if delta.is_final {
            UsageLifecycle::Final
        } else {
            UsageLifecycle::Interim
        };
        Ok(())
    }

    fn end_message(&mut self) -> Result<(), GatewayError> {
        if !self.message_open {
            return Err(stream_protocol_error());
        }
        if self.has_open_tool_call() {
            return Err(stream_protocol_error());
        }

        self.message_open = false;
        Ok(())
    }

    fn is_ready_to_end(&self) -> bool {
        !self.message_open && !self.has_open_tool_call()
    }

    fn has_open_tool_call(&self) -> bool {
        self.tool_calls
            .values()
            .any(|lifecycle| *lifecycle != ToolCallLifecycle::Emitted)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ToolCallLifecycle {
    Declared,
    ArgumentsStreaming,
    Emitted,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum UsageLifecycle {
    #[default]
    NotReported,
    Interim,
    Final,
}

const fn stream_protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

const fn stream_truncated_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::StreamTruncated, ErrorScope::Stream)
}

#[cfg(test)]
mod tests {
    use super::{CanonicalEvent, CanonicalEventState, CanonicalResponse};
    use crate::{ErrorScope, GatewayErrorCode, RawExtensions, ResponseEnd};

    fn parse_event(value: &str) -> Result<CanonicalEvent, serde_json::Error> {
        serde_json::from_str(value)
    }

    fn assert_protocol_error(result: Result<(), crate::GatewayError>) {
        assert!(matches!(
            result,
            Err(error)
                if error.code() == GatewayErrorCode::UpstreamProtocolError
                    && error.scope() == ErrorScope::Stream
        ));
    }

    fn assert_truncated_error(result: Result<(), crate::GatewayError>) {
        assert!(matches!(
            result,
            Err(error)
                if error.code() == GatewayErrorCode::StreamTruncated
                    && error.scope() == ErrorScope::Stream
        ));
    }

    #[test]
    fn canonical_events_round_trip_and_validate_a_full_sequence() -> Result<(), serde_json::Error> {
        let events: Vec<CanonicalEvent> = serde_json::from_str(include_str!(
            "../../../tests/fixtures/core/canonical-event-sequence.json"
        ))?;
        let serialized = serde_json::to_string(&events)?;
        let restored: Vec<CanonicalEvent> = serde_json::from_str(&serialized)?;
        let response = CanonicalResponse::try_new(events.clone());

        assert_eq!(events, restored);
        assert!(response.is_ok());
        if let Ok(response) = response {
            assert_eq!(response.events().len(), 18);
        }
        assert!(matches!(
            &events[13],
            CanonicalEvent::ToolCallEnd(end) if end.arguments.get() == r#"{"query":"weather"}"#
        ));
        let tool_argument_call_ids = events
            .iter()
            .filter_map(|event| match event {
                CanonicalEvent::ToolCallArgumentsDelta(delta) => Some(delta.call_id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            tool_argument_call_ids,
            vec!["call-alpha", "call-beta", "call-alpha", "call-beta"]
        );

        Ok(())
    }

    #[test]
    fn invalid_event_does_not_advance_state() -> Result<(), serde_json::Error> {
        let text = parse_event(r#"{"text_delta":{"text":"secret text","extensions":{}}}"#)?;
        let response_start =
            parse_event(r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#)?;
        let message_start =
            parse_event(r#"{"message_start":{"role":"assistant","extensions":{}}}"#)?;
        let mut state = CanonicalEventState::default();

        assert_protocol_error(state.apply(&text));
        assert!(!state.is_terminal());
        assert!(state.apply(&response_start).is_ok());
        assert!(state.apply(&message_start).is_ok());
        assert!(state.apply(&text).is_ok());

        Ok(())
    }

    #[test]
    fn usage_final_can_only_be_reported_once() -> Result<(), serde_json::Error> {
        let response_start =
            parse_event(r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#)?;
        let final_usage = parse_event(
            r#"{"usage_delta":{"usage":{"output_tokens":3,"extensions":{}},"is_final":true,"extensions":{}}}"#,
        )?;
        let later_usage = parse_event(
            r#"{"usage_delta":{"usage":{"output_tokens":4,"extensions":{}},"is_final":false,"extensions":{}}}"#,
        )?;
        let mut state = CanonicalEventState::default();

        assert!(state.apply(&response_start).is_ok());
        assert!(state.apply(&final_usage).is_ok());
        assert_protocol_error(state.apply(&later_usage));

        let response_end = parse_event(r#"{"response_end":{"extensions":{}}}"#)?;
        assert!(state.apply(&response_end).is_ok());
        assert!(state.is_success());

        Ok(())
    }

    #[test]
    fn interim_usage_does_not_require_a_final_update_before_normal_response_end()
    -> Result<(), serde_json::Error> {
        let response_start =
            parse_event(r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#)?;
        let interim_usage = parse_event(
            r#"{"usage_delta":{"usage":{"output_tokens":2,"extensions":{}},"is_final":false,"extensions":{}}}"#,
        )?;
        let response_end = parse_event(r#"{"response_end":{"extensions":{}}}"#)?;
        let mut state = CanonicalEventState::default();

        assert!(state.apply(&response_start).is_ok());
        assert!(state.apply(&interim_usage).is_ok());
        assert!(state.apply(&response_end).is_ok());
        assert!(state.is_success());

        Ok(())
    }

    #[test]
    fn tool_lifecycle_rejects_unknown_duplicate_and_empty_identifiers()
    -> Result<(), serde_json::Error> {
        let response_start =
            parse_event(r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#)?;
        let message_start =
            parse_event(r#"{"message_start":{"role":"assistant","extensions":{}}}"#)?;
        let unknown_delta = parse_event(
            r#"{"tool_call_arguments_delta":{"call_id":"unknown","delta":"{","extensions":{}}}"#,
        )?;
        let empty_name = parse_event(
            r#"{"tool_call_start":{"call_id":"call-empty","name":"","extensions":{}}}"#,
        )?;
        let tool_start = parse_event(
            r#"{"tool_call_start":{"call_id":"call-01","name":"lookup","extensions":{}}}"#,
        )?;
        let tool_end = parse_event(
            r#"{"tool_call_end":{"call_id":"call-01","arguments":{},"extensions":{}}}"#,
        )?;
        let message_end = parse_event(r#"{"message_end":{"extensions":{}}}"#)?;
        let response_end = parse_event(r#"{"response_end":{"extensions":{}}}"#)?;
        let mut state = CanonicalEventState::default();

        assert!(state.apply(&response_start).is_ok());
        assert!(state.apply(&message_start).is_ok());
        assert_protocol_error(state.apply(&unknown_delta));
        assert_protocol_error(state.apply(&empty_name));
        assert!(state.apply(&tool_start).is_ok());
        assert_protocol_error(state.apply(&tool_start));
        assert!(state.apply(&tool_end).is_ok());
        assert_protocol_error(state.apply(&tool_end));
        assert!(state.apply(&message_end).is_ok());
        assert!(state.apply(&response_end).is_ok());

        Ok(())
    }

    #[test]
    fn message_end_with_an_open_tool_is_a_protocol_error() -> Result<(), serde_json::Error> {
        let response_start =
            parse_event(r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#)?;
        let message_start =
            parse_event(r#"{"message_start":{"role":"assistant","extensions":{}}}"#)?;
        let tool_start = parse_event(
            r#"{"tool_call_start":{"call_id":"call-01","name":"lookup","extensions":{}}}"#,
        )?;
        let message_end = parse_event(r#"{"message_end":{"extensions":{}}}"#)?;
        let tool_end = parse_event(
            r#"{"tool_call_end":{"call_id":"call-01","arguments":{},"extensions":{}}}"#,
        )?;
        let response_end = parse_event(r#"{"response_end":{"extensions":{}}}"#)?;
        let mut state = CanonicalEventState::default();

        assert!(state.apply(&response_start).is_ok());
        assert!(state.apply(&message_start).is_ok());
        assert!(state.apply(&tool_start).is_ok());
        assert_protocol_error(state.apply(&message_end));
        assert!(state.apply(&tool_end).is_ok());
        assert!(state.apply(&message_end).is_ok());
        assert!(state.apply(&response_end).is_ok());

        Ok(())
    }

    #[test]
    fn text_and_reasoning_require_an_open_message_and_non_empty_fragments()
    -> Result<(), serde_json::Error> {
        let response_start =
            parse_event(r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#)?;
        let message_start =
            parse_event(r#"{"message_start":{"role":"assistant","extensions":{}}}"#)?;
        let empty_text = parse_event(r#"{"text_delta":{"text":"","extensions":{}}}"#)?;
        let empty_reasoning = parse_event(r#"{"reasoning_delta":{"text":"","extensions":{}}}"#)?;
        let text = parse_event(r#"{"text_delta":{"text":"safe","extensions":{}}}"#)?;
        let message_end = parse_event(r#"{"message_end":{"extensions":{}}}"#)?;
        let second_message_start =
            parse_event(r#"{"message_start":{"role":"assistant","extensions":{}}}"#)?;
        let reasoning = parse_event(r#"{"reasoning_delta":{"text":"separate","extensions":{}}}"#)?;
        let response_end = parse_event(r#"{"response_end":{"extensions":{}}}"#)?;
        let mut state = CanonicalEventState::default();

        assert!(state.apply(&response_start).is_ok());
        assert_protocol_error(state.apply(&empty_text));
        assert!(state.apply(&message_start).is_ok());
        assert_protocol_error(state.apply(&empty_text));
        assert_protocol_error(state.apply(&empty_reasoning));
        assert!(state.apply(&text).is_ok());
        assert!(state.apply(&message_end).is_ok());
        assert!(state.apply(&second_message_start).is_ok());
        assert!(state.apply(&reasoning).is_ok());
        assert!(state.apply(&message_end).is_ok());
        assert!(state.apply(&response_end).is_ok());

        Ok(())
    }

    #[test]
    fn normal_end_and_source_eof_reject_unclosed_work() -> Result<(), serde_json::Error> {
        let response_start =
            parse_event(r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#)?;
        let message_start =
            parse_event(r#"{"message_start":{"role":"assistant","extensions":{}}}"#)?;
        let tool_start = parse_event(
            r#"{"tool_call_start":{"call_id":"call-01","name":"lookup","extensions":{}}}"#,
        )?;
        let response_end = parse_event(r#"{"response_end":{"extensions":{}}}"#)?;
        let mut state = CanonicalEventState::default();

        assert_truncated_error(state.finish());
        assert!(state.apply(&response_start).is_ok());
        assert!(state.apply(&message_start).is_ok());
        assert!(state.apply(&tool_start).is_ok());
        assert_truncated_error(state.apply(&response_end));
        assert_truncated_error(state.finish());

        Ok(())
    }

    #[test]
    fn stream_error_is_terminal_and_cannot_form_a_success_response() -> Result<(), serde_json::Error>
    {
        let response_start =
            parse_event(r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#)?;
        let message_start =
            parse_event(r#"{"message_start":{"role":"assistant","extensions":{}}}"#)?;
        let tool_start = parse_event(
            r#"{"tool_call_start":{"call_id":"call-01","name":"lookup","extensions":{}}}"#,
        )?;
        let stream_error = parse_event(
            r#"{"stream_error":{"error":{"code":"ProviderTransient","scope":"provider"}}}"#,
        )?;
        let response_end = parse_event(r#"{"response_end":{"extensions":{}}}"#)?;
        let mut state = CanonicalEventState::default();

        assert!(state.apply(&response_start).is_ok());
        assert!(state.apply(&message_start).is_ok());
        assert!(state.apply(&tool_start).is_ok());
        assert!(state.apply(&stream_error).is_ok());
        assert!(state.finish().is_ok());
        assert!(state.is_terminal());
        assert!(!state.is_success());
        assert_protocol_error(state.apply(&response_end));

        let result = CanonicalResponse::try_new(vec![response_start, stream_error]);
        assert!(matches!(
            result,
            Err(error)
                if error.code() == GatewayErrorCode::ProviderTransient
                    && error.scope() == ErrorScope::Provider
        ));

        Ok(())
    }

    #[test]
    fn event_json_rejects_invalid_ids_and_unknown_payload_fields() {
        let empty_response_id = serde_json::from_str::<CanonicalEvent>(
            r#"{"response_start":{"response_id":"","extensions":{}}}"#,
        );
        let unknown_payload_field = serde_json::from_str::<CanonicalEvent>(
            r#"{"text_delta":{"text":"safe","extensions":{},"unexpected":true}}"#,
        );

        assert!(empty_response_id.is_err());
        assert!(unknown_payload_field.is_err());
    }

    #[test]
    fn response_end_validates_non_empty_reported_semantics_and_redacts_them()
    -> Result<(), serde_json::Error> {
        let response_start =
            parse_event(r#"{"response_start":{"response_id":"response-01","extensions":{}}}"#)?;
        let invalid_end = parse_event(
            r#"{"response_end":{"stop_reason":"","stop_sequence":"","extensions":{}}}"#,
        )?;
        let mut state = CanonicalEventState::default();
        assert!(state.apply(&response_start).is_ok());
        assert_protocol_error(state.apply(&invalid_end));

        let end = ResponseEnd {
            stop_reason: Some("private-stop-reason".to_owned()),
            stop_sequence: Some("private-stop-sequence".to_owned()),
            extensions: RawExtensions::default(),
        };
        let diagnostic = format!("{end:?}");
        assert!(!diagnostic.contains("private-stop-reason"));
        assert!(!diagnostic.contains("private-stop-sequence"));
        assert!(diagnostic.contains("stop_reason_reported: true"));
        Ok(())
    }

    #[test]
    fn event_debug_forms_redact_text_tool_and_raw_extension_values() -> Result<(), serde_json::Error>
    {
        let events: Vec<CanonicalEvent> = serde_json::from_str(include_str!(
            "../../../tests/fixtures/core/canonical-event-sequence.json"
        ))?;
        let payload_diagnostic = events
            .iter()
            .map(|event| match event {
                CanonicalEvent::ResponseStart(value) => format!("{value:?}"),
                CanonicalEvent::MessageStart(value) => format!("{value:?}"),
                CanonicalEvent::TextDelta(value) => format!("{value:?}"),
                CanonicalEvent::ReasoningDelta(value) => format!("{value:?}"),
                CanonicalEvent::ToolCallStart(value) => format!("{value:?}"),
                CanonicalEvent::ToolCallArgumentsDelta(value) => format!("{value:?}"),
                CanonicalEvent::ToolCallEnd(value) => format!("{value:?}"),
                CanonicalEvent::UsageDelta(value) => format!("{value:?}"),
                CanonicalEvent::MessageEnd(value) => format!("{value:?}"),
                CanonicalEvent::ResponseEnd(value) => format!("{value:?}"),
                CanonicalEvent::StreamError(value) => format!("{value:?}"),
            })
            .collect::<String>();
        let diagnostic = format!("{events:?}{payload_diagnostic}");

        for sensitive_value in [
            "response-01",
            "Visible answer fragment.",
            "Reasoning fragment.",
            "call-alpha",
            "lookup_weather",
            "weather",
            "vendor event data",
        ] {
            assert!(!diagnostic.contains(sensitive_value));
        }

        Ok(())
    }

    #[test]
    fn tool_call_end_keeps_complete_json_without_normalizing_it() -> Result<(), serde_json::Error> {
        let event = parse_event(
            r#"{"tool_call_end":{"call_id":"call-01","arguments":{ "query" : "weather" },"extensions":{}}}"#,
        )?;

        assert!(matches!(
            event,
            CanonicalEvent::ToolCallEnd(end)
                if end.arguments.get() == r#"{ "query" : "weather" }"#
        ));

        Ok(())
    }
}
