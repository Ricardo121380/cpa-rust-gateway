//! P12-08D4 clean-room differential for the three public protocol families.

use std::{collections::BTreeSet, fmt};

use gateway_core::{
    CanonicalEvent, CanonicalResponse, MessageEnd, MessageRole, MessageStart, RawExtensions,
    ReasoningDelta, ResponseEnd, ResponseId, ResponseStart,
};
use gateway_router::{ProtocolFormat, project_protocol_response};
use protocol_anthropic::{
    AnthropicMessagesSseDecoder, decode_upstream_response as decode_messages,
};
use protocol_openai_chat::{OpenAiChatSseDecoder, decode_upstream_response as decode_chat};
use protocol_openai_responses::{
    OpenAiResponsesSseDecoder, decode_upstream_response as decode_responses,
};
use serde::Deserialize;

const CORPUS_VERSION: u8 = 1;
const LEGACY_REFERENCE: &str = "cliproxyapi-v7.2.101-42a00a2a6521b867c27f7ad096d08699db8e6d19";
const MAX_CORPUS_BYTES: usize = 24 * 1024;
const EXPECTED_CASES: usize = 10;
const MAX_MARKERS_PER_PROJECTION: usize = 16;
const FORBIDDEN_FIELD_NAMES: &[&str] = &[
    "authorization",
    "cookie",
    "endpoint",
    "header",
    "oauth",
    "secret",
    "token",
    "url",
];

/// Closed D4 difference taxonomy. There is deliberately no accepted regression variant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LegacyProtocolClassification {
    /// The legacy semantic projection and CPAR projection agree.
    Parity,
    /// CPAR deliberately rejects a permissive or synthetic-success legacy behavior.
    IntentionalHardening,
    /// Release 1 Canonical has no lossless representation and rejects the behavior.
    UnsupportedFailClosed,
}

/// Aggregate result of one complete D4 corpus evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyProtocolOutcome {
    /// Number of accepted parity cases.
    pub parity: usize,
    /// Number of accepted hardening differences.
    pub intentional_hardening: usize,
    /// Number of accepted unsupported/fail-closed differences.
    pub unsupported_fail_closed: usize,
}

impl LegacyProtocolOutcome {
    /// Returns the number of classified cases.
    #[must_use]
    pub const fn total(self) -> usize {
        self.parity + self.intentional_hardening + self.unsupported_fail_closed
    }
}

/// Validates the committed value-free corpus against freshly executed CPAR protocol code.
///
/// # Errors
///
/// Returns a stable value-free error if the corpus is unsafe, incomplete, misclassified, or has
/// drifted from the current implementation.
pub fn validate_legacy_protocol_corpus(
    input: &str,
) -> Result<LegacyProtocolOutcome, LegacyProtocolError> {
    reject_unsafe_shape(input)?;
    let corpus: LegacyProtocolCorpus =
        serde_json::from_str(input).map_err(|_| LegacyProtocolError::MalformedCorpus)?;
    corpus.validate_and_execute()
}

fn reject_unsafe_shape(input: &str) -> Result<(), LegacyProtocolError> {
    if input.len() > MAX_CORPUS_BYTES {
        return Err(LegacyProtocolError::CorpusTooLarge);
    }
    let lower = input.to_ascii_lowercase();
    if FORBIDDEN_FIELD_NAMES
        .iter()
        .any(|field| lower.contains(&format!(r#""{field}""#)))
    {
        return Err(LegacyProtocolError::ForbiddenCorpusShape);
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyProtocolCorpus {
    corpus_version: u8,
    legacy_reference: String,
    cases: Vec<LegacyProtocolCase>,
}

impl LegacyProtocolCorpus {
    fn validate_and_execute(self) -> Result<LegacyProtocolOutcome, LegacyProtocolError> {
        if self.corpus_version != CORPUS_VERSION
            || self.legacy_reference != LEGACY_REFERENCE
            || self.cases.len() != EXPECTED_CASES
        {
            return Err(LegacyProtocolError::IncompleteCorpus);
        }
        let mut subjects = BTreeSet::new();
        let mut outcome = LegacyProtocolOutcome {
            parity: 0,
            intentional_hardening: 0,
            unsupported_fail_closed: 0,
        };
        for case in self.cases {
            if !valid_case_id(&case.id) || !subjects.insert(case.subject) {
                return Err(LegacyProtocolError::IncompleteCorpus);
            }
            case.validate_and_execute(&mut outcome)?;
        }
        if subjects != Subject::all().into_iter().collect() || outcome.total() != EXPECTED_CASES {
            return Err(LegacyProtocolError::IncompleteCorpus);
        }
        Ok(outcome)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyProtocolCase {
    id: String,
    subject: Subject,
    reference_projection: Vec<Marker>,
    expected_gateway_projection: Vec<Marker>,
    classification: LegacyProtocolClassification,
    decision: Option<Decision>,
}

impl LegacyProtocolCase {
    fn validate_and_execute(
        self,
        outcome: &mut LegacyProtocolOutcome,
    ) -> Result<(), LegacyProtocolError> {
        let (required_classification, required_decision) = self.subject.required_decision();
        if self.classification != required_classification
            || self.decision != required_decision
            || self.reference_projection.is_empty()
            || self.expected_gateway_projection.is_empty()
            || self.reference_projection.len() > MAX_MARKERS_PER_PROJECTION
            || self.expected_gateway_projection.len() > MAX_MARKERS_PER_PROJECTION
            || !contains_all(
                &self.reference_projection,
                self.subject.required_reference_markers(),
            )
            || !contains_all(
                &self.expected_gateway_projection,
                self.subject.required_gateway_markers(),
            )
        {
            return Err(LegacyProtocolError::MisclassifiedDifference);
        }
        let observed = observe(self.subject)?;
        if observed != self.expected_gateway_projection {
            return Err(LegacyProtocolError::GatewayProjectionMismatch);
        }
        match self.classification {
            LegacyProtocolClassification::Parity
                if self.reference_projection == observed && self.decision.is_none() =>
            {
                outcome.parity += 1;
            }
            LegacyProtocolClassification::IntentionalHardening
                if self.reference_projection != observed && self.decision.is_some() =>
            {
                outcome.intentional_hardening += 1;
            }
            LegacyProtocolClassification::UnsupportedFailClosed
                if self.reference_projection != observed && self.decision.is_some() =>
            {
                outcome.unsupported_fail_closed += 1;
            }
            LegacyProtocolClassification::Parity
            | LegacyProtocolClassification::IntentionalHardening
            | LegacyProtocolClassification::UnsupportedFailClosed => {
                return Err(LegacyProtocolError::MisclassifiedDifference);
            }
        }
        Ok(())
    }
}

fn contains_all(projection: &[Marker], required: &[Marker]) -> bool {
    required.iter().all(|marker| projection.contains(marker))
}

fn valid_case_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum Subject {
    ChatJsonToolUsage,
    ChatSseToolUsage,
    ResponsesJsonReasoningToolUsage,
    ResponsesSseReasoningUsage,
    MessagesJsonReasoningToolUsage,
    MessagesSseReasoningUsage,
    DuplicateJsonMember,
    MissingSseTerminal,
    ReasoningToChat,
    MultipleChatChoices,
}

impl Subject {
    const fn all() -> [Self; EXPECTED_CASES] {
        [
            Self::ChatJsonToolUsage,
            Self::ChatSseToolUsage,
            Self::ResponsesJsonReasoningToolUsage,
            Self::ResponsesSseReasoningUsage,
            Self::MessagesJsonReasoningToolUsage,
            Self::MessagesSseReasoningUsage,
            Self::DuplicateJsonMember,
            Self::MissingSseTerminal,
            Self::ReasoningToChat,
            Self::MultipleChatChoices,
        ]
    }

    const fn required_decision(self) -> (LegacyProtocolClassification, Option<Decision>) {
        match self {
            Self::ChatJsonToolUsage
            | Self::ChatSseToolUsage
            | Self::ResponsesJsonReasoningToolUsage
            | Self::ResponsesSseReasoningUsage
            | Self::MessagesJsonReasoningToolUsage
            | Self::MessagesSseReasoningUsage => (LegacyProtocolClassification::Parity, None),
            Self::DuplicateJsonMember => (
                LegacyProtocolClassification::IntentionalHardening,
                Some(Decision::StrictDuplicateJson),
            ),
            Self::MissingSseTerminal => (
                LegacyProtocolClassification::IntentionalHardening,
                Some(Decision::ExplicitTerminalRequired),
            ),
            Self::ReasoningToChat => (
                LegacyProtocolClassification::UnsupportedFailClosed,
                Some(Decision::ReasoningHasNoChatProjection),
            ),
            Self::MultipleChatChoices => (
                LegacyProtocolClassification::UnsupportedFailClosed,
                Some(Decision::SingleGenerationCanonical),
            ),
        }
    }

    const fn required_reference_markers(self) -> &'static [Marker] {
        match self {
            Self::ChatJsonToolUsage | Self::ChatSseToolUsage => &[
                Marker::ToolCallStart,
                Marker::ToolArguments,
                Marker::UsageFinal,
            ],
            Self::ResponsesJsonReasoningToolUsage | Self::MessagesJsonReasoningToolUsage => &[
                Marker::ReasoningDelta,
                Marker::ToolCallStart,
                Marker::ToolArguments,
                Marker::UsageFinal,
            ],
            Self::ResponsesSseReasoningUsage | Self::MessagesSseReasoningUsage => {
                &[Marker::ReasoningDelta, Marker::UsageFinal]
            }
            Self::DuplicateJsonMember => &[Marker::LegacyPermissiveJson],
            Self::MissingSseTerminal => &[Marker::LegacySyntheticTerminal],
            Self::ReasoningToChat => &[Marker::ReasoningDelta],
            Self::MultipleChatChoices => &[Marker::LegacyMultipleChoices],
        }
    }

    const fn required_gateway_markers(self) -> &'static [Marker] {
        match self {
            Self::ChatJsonToolUsage | Self::ChatSseToolUsage => &[
                Marker::ToolCallStart,
                Marker::ToolArguments,
                Marker::UsageFinal,
            ],
            Self::ResponsesJsonReasoningToolUsage | Self::MessagesJsonReasoningToolUsage => &[
                Marker::ReasoningDelta,
                Marker::ToolCallStart,
                Marker::ToolArguments,
                Marker::UsageFinal,
            ],
            Self::ResponsesSseReasoningUsage | Self::MessagesSseReasoningUsage => {
                &[Marker::ReasoningDelta, Marker::UsageFinal]
            }
            Self::DuplicateJsonMember => &[Marker::RejectedDuplicateJson],
            Self::MissingSseTerminal => &[Marker::RejectedTruncatedStream],
            Self::ReasoningToChat => &[Marker::RejectedReasoningToChat],
            Self::MultipleChatChoices => &[Marker::RejectedMultipleChoices],
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Decision {
    StrictDuplicateJson,
    ExplicitTerminalRequired,
    ReasoningHasNoChatProjection,
    SingleGenerationCanonical,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Marker {
    ResponseStart,
    MessageStart,
    TextDelta,
    ReasoningDelta,
    ToolCallStart,
    ToolArguments,
    ToolCallEnd,
    MessageEnd,
    UsageFinal,
    ResponseEnd,
    LegacyPermissiveJson,
    LegacySyntheticTerminal,
    LegacyMultipleChoices,
    RejectedDuplicateJson,
    RejectedTruncatedStream,
    RejectedReasoningToChat,
    RejectedMultipleChoices,
}

fn observe(subject: Subject) -> Result<Vec<Marker>, LegacyProtocolError> {
    match subject {
        Subject::ChatJsonToolUsage => summarize(decode_chat(CHAT_JSON).map_err(probe_error)?),
        Subject::ChatSseToolUsage => summarize(decode_chat_sse(CHAT_SSE)?),
        Subject::ResponsesJsonReasoningToolUsage => {
            summarize(decode_responses(RESPONSES_JSON).map_err(probe_error)?)
        }
        Subject::ResponsesSseReasoningUsage => summarize(decode_responses_sse(RESPONSES_SSE)?),
        Subject::MessagesJsonReasoningToolUsage => {
            summarize(decode_messages(MESSAGES_JSON).map_err(probe_error)?)
        }
        Subject::MessagesSseReasoningUsage => summarize(decode_messages_sse(MESSAGES_SSE)?),
        Subject::DuplicateJsonMember => {
            if decode_chat(DUPLICATE_JSON).is_err() {
                Ok(vec![Marker::RejectedDuplicateJson])
            } else {
                Err(LegacyProtocolError::GatewayProbeUnavailable)
            }
        }
        Subject::MissingSseTerminal => {
            let mut decoder = OpenAiChatSseDecoder::new();
            let _ = decoder
                .push(MISSING_TERMINAL_SSE.as_bytes())
                .map_err(probe_error)?;
            if decoder.finish().is_err() {
                Ok(vec![Marker::RejectedTruncatedStream])
            } else {
                Err(LegacyProtocolError::GatewayProbeUnavailable)
            }
        }
        Subject::ReasoningToChat => {
            let response = reasoning_response()?;
            if project_protocol_response(&response, ProtocolFormat::OpenAiChatCompletions).is_err()
            {
                Ok(vec![Marker::RejectedReasoningToChat])
            } else {
                Err(LegacyProtocolError::GatewayProbeUnavailable)
            }
        }
        Subject::MultipleChatChoices => {
            if decode_chat(MULTIPLE_CHOICES_JSON).is_err() {
                Ok(vec![Marker::RejectedMultipleChoices])
            } else {
                Err(LegacyProtocolError::GatewayProbeUnavailable)
            }
        }
    }
}

fn probe_error(_error: gateway_core::GatewayError) -> LegacyProtocolError {
    LegacyProtocolError::GatewayProbeUnavailable
}

fn summarize(events: Vec<CanonicalEvent>) -> Result<Vec<Marker>, LegacyProtocolError> {
    CanonicalResponse::try_new(events.clone())
        .map_err(|_| LegacyProtocolError::GatewayProbeUnavailable)?;
    let mut markers = Vec::with_capacity(events.len());
    for event in events {
        markers.push(match event {
            CanonicalEvent::ResponseStart(_) => Marker::ResponseStart,
            CanonicalEvent::MessageStart(_) => Marker::MessageStart,
            CanonicalEvent::TextDelta(delta) if !delta.text.is_empty() => Marker::TextDelta,
            CanonicalEvent::ReasoningDelta(delta) if !delta.text.is_empty() => {
                Marker::ReasoningDelta
            }
            CanonicalEvent::ToolCallStart(_) => Marker::ToolCallStart,
            CanonicalEvent::ToolCallArgumentsDelta(delta) if !delta.delta.is_empty() => {
                Marker::ToolArguments
            }
            CanonicalEvent::ToolCallEnd(_) => Marker::ToolCallEnd,
            CanonicalEvent::MessageEnd(_) => Marker::MessageEnd,
            CanonicalEvent::UsageDelta(delta) if delta.is_final => Marker::UsageFinal,
            CanonicalEvent::ResponseEnd(_) => Marker::ResponseEnd,
            CanonicalEvent::UsageDelta(_)
            | CanonicalEvent::StreamError(_)
            | CanonicalEvent::TextDelta(_)
            | CanonicalEvent::ReasoningDelta(_)
            | CanonicalEvent::ToolCallArgumentsDelta(_) => continue,
        });
    }
    Ok(markers)
}

fn decode_chat_sse(input: &str) -> Result<Vec<CanonicalEvent>, LegacyProtocolError> {
    let mut decoder = OpenAiChatSseDecoder::new();
    let mut events = Vec::new();
    for chunk in input.as_bytes().chunks(7) {
        events.extend(decoder.push(chunk).map_err(probe_error)?);
    }
    events.extend(decoder.finish().map_err(probe_error)?);
    Ok(events)
}

fn decode_responses_sse(input: &str) -> Result<Vec<CanonicalEvent>, LegacyProtocolError> {
    let mut decoder = OpenAiResponsesSseDecoder::new();
    let mut events = Vec::new();
    for chunk in input.as_bytes().chunks(5) {
        events.extend(decoder.push(chunk).map_err(probe_error)?);
    }
    events.extend(decoder.finish().map_err(probe_error)?);
    Ok(events)
}

fn decode_messages_sse(input: &str) -> Result<Vec<CanonicalEvent>, LegacyProtocolError> {
    let mut decoder = AnthropicMessagesSseDecoder::new();
    let mut events = Vec::new();
    for chunk in input.as_bytes().chunks(9) {
        decoder.push_chunk(chunk).map_err(probe_error)?;
        loop {
            decoder.drain_buffered_frames().map_err(probe_error)?;
            let Some(event) = decoder.take_event() else {
                break;
            };
            events.push(event);
        }
    }
    decoder.finish().map_err(probe_error)?;
    while let Some(event) = decoder.take_event() {
        events.push(event);
    }
    Ok(events)
}

fn reasoning_response() -> Result<CanonicalResponse, LegacyProtocolError> {
    CanonicalResponse::try_new(vec![
        CanonicalEvent::ResponseStart(ResponseStart {
            response_id: ResponseId::try_new("d4-reasoning")
                .map_err(|_| LegacyProtocolError::GatewayProbeUnavailable)?,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::ReasoningDelta(ReasoningDelta {
            text: "reasoning".to_owned(),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageEnd(MessageEnd::default()),
        CanonicalEvent::ResponseEnd(ResponseEnd {
            stop_reason: Some("end_turn".to_owned()),
            stop_sequence: None,
            extensions: RawExtensions::default(),
        }),
    ])
    .map_err(|_| LegacyProtocolError::GatewayProbeUnavailable)
}

const CHAT_JSON: &str = r#"{"id":"d4-chat","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"ok","tool_calls":[{"id":"d4-call","type":"function","function":{"name":"lookup","arguments":"{\"v\":1}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}"#;
const CHAT_SSE: &str = concat!(
    "data: {\"id\":\"d4-chat-sse\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"d4-call\",\"type\":\"function\",\"function\":{\"name\":\"lookup\",\"arguments\":\"{\\\"v\\\":\"}}]},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"d4-chat-sse\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1}\"}}]},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"d4-chat-sse\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
    "data: {\"id\":\"d4-chat-sse\",\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}\n\n",
    "data: [DONE]\n\n"
);
const RESPONSES_JSON: &str = r#"{"id":"d4-response","object":"response","status":"completed","error":null,"incomplete_details":null,"output":[{"id":"d4-reason","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"think"}]},{"id":"d4-message","type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"ok","annotations":[],"logprobs":[]}]},{"id":"d4-tool","type":"function_call","status":"completed","call_id":"d4-call","name":"lookup","arguments":"{\"v\":1}"}],"usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}}"#;
const RESPONSES_SSE: &str = concat!(
    "data: {\"type\":\"response.created\",\"response\":{\"id\":\"d4-response-sse\",\"usage\":{\"input_tokens\":2}}}\n\n",
    "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"d4-reason\",\"type\":\"reasoning\",\"status\":\"in_progress\"}}\n\n",
    "data: {\"type\":\"response.reasoning_summary_text.delta\",\"item_id\":\"d4-reason\",\"delta\":\"think\"}\n\n",
    "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"d4-message\",\"type\":\"message\",\"status\":\"in_progress\",\"role\":\"assistant\"}}\n\n",
    "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"d4-message\",\"delta\":\"ok\"}\n\n",
    "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"d4-response-sse\",\"status\":\"completed\",\"usage\":{\"input_tokens\":2,\"output_tokens\":3,\"total_tokens\":5}}}\n\n"
);
const MESSAGES_JSON: &str = r#"{"id":"d4-message","type":"message","role":"assistant","model":"d4-model","content":[{"type":"thinking","thinking":"think"},{"type":"text","text":"ok"},{"type":"tool_use","id":"d4-call","name":"lookup","input":{"v":1}}],"stop_reason":"tool_use","stop_sequence":null,"usage":{"input_tokens":2,"output_tokens":3}}"#;
const MESSAGES_SSE: &str = concat!(
    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"d4-message-sse\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"d4-model\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":2,\"output_tokens\":0}}}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"think\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":3}}\n\n",
    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
);
const DUPLICATE_JSON: &str =
    r#"{"id":"first","id":"second","object":"chat.completion","choices":[]}"#;
const MISSING_TERMINAL_SSE: &str = "data: {\"id\":\"d4-truncated\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"},\"finish_reason\":null}]}\n\n";
const MULTIPLE_CHOICES_JSON: &str = r#"{"id":"d4-multi","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"one"},"finish_reason":"stop"},{"index":1,"message":{"role":"assistant","content":"two"},"finish_reason":"stop"}]}"#;

/// Stable, value-free D4 corpus failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyProtocolError {
    /// Corpus bytes exceeded the fixed local bound.
    CorpusTooLarge,
    /// Corpus metadata used a credential- or target-bearing field name.
    ForbiddenCorpusShape,
    /// JSON or its closed vocabulary was malformed.
    MalformedCorpus,
    /// Required scenario coverage or the pinned reference was incomplete.
    IncompleteCorpus,
    /// The committed expected projection drifted from current CPAR behavior.
    GatewayProjectionMismatch,
    /// A difference used the wrong closed classification or decision.
    MisclassifiedDifference,
    /// Current protocol code could not produce the scenario's safe projection.
    GatewayProbeUnavailable,
}

impl fmt::Display for LegacyProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::CorpusTooLarge => "corpus_too_large",
            Self::ForbiddenCorpusShape => "forbidden_corpus_shape",
            Self::MalformedCorpus => "malformed_corpus",
            Self::IncompleteCorpus => "incomplete_corpus",
            Self::GatewayProjectionMismatch => "gateway_projection_mismatch",
            Self::MisclassifiedDifference => "misclassified_difference",
            Self::GatewayProbeUnavailable => "gateway_probe_unavailable",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for LegacyProtocolError {}
