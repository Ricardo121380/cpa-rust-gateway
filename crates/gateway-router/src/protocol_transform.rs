//! Fail-closed admission analysis for protocol transformation candidates.
//!
//! A Route may declare a native pass-through, a same-protocol Canonical conversion, or a
//! cross-protocol lossless bridge. This module does not serialize a request, choose an Endpoint,
//! or execute a Provider call. It only determines whether the requested conversion has enough
//! evidence to participate in routing without silently erasing semantics.

use std::fmt;

use gateway_catalog::{CapabilitySet, SemanticCapability};
use gateway_core::{
    CanonicalRequest, MessageContent, MessageRole, RawExtensions, RawJson, Thinking, ThinkingEffort,
};
use gateway_protocol::ApiFormat;
use serde_json::Value;

use crate::SnapshotTransformMode;

/// Client or Endpoint wire format considered by the P5 transformation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolFormat {
    /// `OpenAI`'s Chat Completions representation.
    OpenAiChatCompletions,
    /// `OpenAI`'s Responses API representation.
    OpenAiResponses,
    /// Anthropic's Messages API representation.
    AnthropicMessages,
}

impl ProtocolFormat {
    /// Returns the exact control-plane `api_format` value for this protocol boundary.
    #[must_use]
    pub const fn api_format(self) -> &'static str {
        self.as_api_format().as_str()
    }

    /// Returns the shared [`ApiFormat`] vocabulary value for this protocol boundary.
    ///
    /// The spelling table lives once, in `gateway-protocol`, so a Config Version admitted by the
    /// management-time compiler and a Candidate filtered by this transform boundary can never
    /// disagree about which stored string means which protocol.
    #[must_use]
    pub const fn as_api_format(self) -> ApiFormat {
        match self {
            Self::OpenAiChatCompletions => ApiFormat::OpenAiChatCompletions,
            Self::OpenAiResponses => ApiFormat::OpenAiResponses,
            Self::AnthropicMessages => ApiFormat::AnthropicMessages,
        }
    }

    /// Parses an Endpoint's exact declared API format when it belongs to the P5 boundary.
    ///
    /// Other formats intentionally remain unknown here. They can be retained in the shared
    /// Snapshot for their owning future Provider without becoming eligible for either P5 client
    /// protocol by string similarity or model-name inference. The exhaustive match is deliberate:
    /// a new [`ApiFormat`] variant must be given an explicit P5 decision, never a default one.
    #[must_use]
    pub fn from_api_format(api_format: &str) -> Option<Self> {
        match ApiFormat::parse(api_format) {
            Some(ApiFormat::OpenAiChatCompletions) => Some(Self::OpenAiChatCompletions),
            Some(ApiFormat::OpenAiResponses) => Some(Self::OpenAiResponses),
            Some(ApiFormat::AnthropicMessages) => Some(Self::AnthropicMessages),
            None => None,
        }
    }
}

/// Whether the Router still holds an exact native payload for a pass-through candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePayloadAvailability {
    /// The original complete native payload is available and will be forwarded unchanged.
    Exact,
    /// Only Canonical data remains, so native pass-through would require a reconstruction.
    Unavailable,
}

/// Fixed, request-local facts used to admit one protocol transformation candidate.
///
/// The caller supplies capability requirements that are not represented directly by the current
/// Canonical request shape, such as an explicit streaming response or parallel-Tool request.
/// This keeps raw extension data opaque: an unknown extension is admission evidence for an exact
/// pass-through only, never an instruction to infer a bridge mapping.
#[derive(Clone, Copy)]
pub struct ProtocolTransformInput<'a> {
    /// Protocol spoken by the client-facing request.
    pub source: ProtocolFormat,
    /// Protocol spoken by the selected Endpoint.
    pub target: ProtocolFormat,
    /// Route-configured conversion mode.
    pub mode: SnapshotTransformMode,
    /// Whether an exact native request body remains available.
    pub native_payload: NativePayloadAvailability,
    /// Request semantics that would otherwise be transformed.
    pub request: &'a CanonicalRequest,
    /// Whether the client requested an SSE response.
    pub streaming: bool,
    /// Whether the request requires JSON Schema support beyond the ordinary Tool schemas.
    pub requires_json_schema: bool,
    /// Whether the request explicitly requires multiple parallel Tool calls.
    pub requires_parallel_tools: bool,
    /// Compiler-approved semantic capability profile of the selected Endpoint.
    pub target_capabilities: &'a CapabilitySet,
}

impl fmt::Debug for ProtocolTransformInput<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtocolTransformInput")
            .field("source", &self.source)
            .field("target", &self.target)
            .field("mode", &self.mode)
            .field("native_payload", &self.native_payload)
            .field("request", &"<redacted>")
            .field("streaming", &self.streaming)
            .field("requires_json_schema", &self.requires_json_schema)
            .field("requires_parallel_tools", &self.requires_parallel_tools)
            .field("target_capabilities", &self.target_capabilities)
            .finish()
    }
}

/// Stable outcome of [`analyze_protocol_transform`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolTransformAdmission {
    /// The candidate can preserve the request semantics for its configured conversion mode.
    Approved,
    /// The candidate must be excluded without exposing client request data.
    Rejected(ProtocolTransformRejection),
}

impl ProtocolTransformAdmission {
    /// Returns whether this candidate is safe to retain for later scheduling.
    #[must_use]
    pub const fn is_approved(self) -> bool {
        matches!(self, Self::Approved)
    }
}

/// Stable, secret-free reason why a protocol conversion candidate is ineligible.
///
/// These variants intentionally carry no model names, URLs, request text, Tool names, IDs, or raw
/// extension values. They are safe for Route Explain and management-time diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolTransformRejection {
    /// The source/target pair is absent from the reviewed three-protocol registry.
    PairUnregistered,
    /// A native pass-through was requested between different protocols.
    PassthroughProtocolMismatch,
    /// A native pass-through would need to reconstruct a body from Canonical data.
    PassthroughNativePayloadUnavailable,
    /// Canonical conversion is not a cross-protocol bridge.
    CanonicalProtocolMismatch,
    /// A lossless bridge is unnecessary and unsupported for identical protocols.
    LosslessBridgeProtocolMatch,
    /// A request-level opaque extension would be dropped by Canonical conversion.
    UnknownRequestExtensions,
    /// A message-level opaque extension would be dropped by Canonical conversion.
    UnknownMessageExtensions,
    /// A text content-level opaque extension would be dropped by Canonical conversion.
    UnknownContentExtensions,
    /// A Tool-definition opaque extension would be dropped by Canonical conversion.
    UnknownToolDefinitionExtensions,
    /// Opaque content has no proven target-protocol representation.
    OpaqueContent,
    /// Thinking requires the explicit P5-06 mapping contract.
    ThinkingUnsupported,
    /// Prompt-cache controls require the explicit P5-06 mapping contract.
    CacheControlUnsupported,
    /// A required output-token limit is absent from the typed source request.
    OutputLimitMissing,
    /// An output-token limit is not a positive integer.
    OutputLimitInvalid,
    /// More than one output-token limit would compete for the target field.
    OutputLimitCollision,
    /// A historical Tool shape cannot be represented by the target protocol.
    ToolHistoryUnsupported,
    /// A Canonical role cannot be represented by the target protocol slice.
    IncompatibleRole,
    /// The Endpoint cannot provide the requested streaming response.
    StreamingUnsupported,
    /// The Endpoint cannot provide declared Tool calls.
    ToolsUnsupported,
    /// The Endpoint cannot provide the required JSON Schema semantics.
    JsonSchemaUnsupported,
    /// The Endpoint cannot provide the requested parallel Tool semantics.
    ParallelToolsUnsupported,
    /// The Endpoint cannot provide explicit Thinking or Reasoning semantics.
    ReasoningUnsupported,
    /// The client protocol cannot safely carry Reasoning this Endpoint may return.
    ResponseReasoningUnsupported,
}

impl fmt::Display for ProtocolTransformRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProtocolTransformRejection {}

const REGISTERED_PROTOCOL_PAIRS: [(ProtocolFormat, ProtocolFormat); 9] = [
    (
        ProtocolFormat::OpenAiChatCompletions,
        ProtocolFormat::OpenAiChatCompletions,
    ),
    (
        ProtocolFormat::OpenAiChatCompletions,
        ProtocolFormat::OpenAiResponses,
    ),
    (
        ProtocolFormat::OpenAiChatCompletions,
        ProtocolFormat::AnthropicMessages,
    ),
    (
        ProtocolFormat::OpenAiResponses,
        ProtocolFormat::OpenAiChatCompletions,
    ),
    (
        ProtocolFormat::OpenAiResponses,
        ProtocolFormat::OpenAiResponses,
    ),
    (
        ProtocolFormat::OpenAiResponses,
        ProtocolFormat::AnthropicMessages,
    ),
    (
        ProtocolFormat::AnthropicMessages,
        ProtocolFormat::OpenAiChatCompletions,
    ),
    (
        ProtocolFormat::AnthropicMessages,
        ProtocolFormat::OpenAiResponses,
    ),
    (
        ProtocolFormat::AnthropicMessages,
        ProtocolFormat::AnthropicMessages,
    ),
];

/// Returns whether one source/target pair is explicitly present in the reviewed registry.
#[must_use]
pub fn protocol_pair_is_registered(source: ProtocolFormat, target: ProtocolFormat) -> bool {
    REGISTERED_PROTOCOL_PAIRS.contains(&(source, target))
}

/// Returns whether a pair, configured transform mode, and response capability can be published.
///
/// This is intentionally request-free so protected Route Explain can report topology admission
/// without receiving prompts, Tools, native bodies, or extensions. Request-local projection still
/// runs later before any Credential lease.
#[must_use]
pub fn protocol_pair_is_publishable(
    source: ProtocolFormat,
    target: ProtocolFormat,
    mode: SnapshotTransformMode,
    target_capabilities: &CapabilitySet,
) -> bool {
    protocol_pair_is_registered(source, target)
        && match mode {
            SnapshotTransformMode::Passthrough | SnapshotTransformMode::Canonical => {
                source == target
            }
            SnapshotTransformMode::LosslessBridge => source != target,
            SnapshotTransformMode::CanonicalBridge => true,
        }
        && !(source == ProtocolFormat::OpenAiChatCompletions
            && target_capabilities.supports(SemanticCapability::Reasoning))
}

/// Projects one request only after pair registration and response-side capability admission.
///
/// A Chat client is excluded when the selected Endpoint advertises Reasoning: D2 deliberately has
/// no private-reasoning-to-visible-Chat degradation, so this proof must happen before a lease or
/// upstream Attempt rather than after a streamed Reasoning event arrives.
///
/// # Errors
///
/// Returns a stable value-free rejection for an unregistered pair, an unsafe response capability,
/// or any request-side rejection from [`project_protocol_request`].
pub fn project_registered_protocol_request(
    input: ProtocolTransformInput<'_>,
) -> Result<ProjectedProtocolRequest, ProtocolTransformRejection> {
    if !protocol_pair_is_registered(input.source, input.target) {
        return Err(ProtocolTransformRejection::PairUnregistered);
    }
    if input.source == ProtocolFormat::OpenAiChatCompletions
        && input
            .target_capabilities
            .supports(SemanticCapability::Reasoning)
    {
        return Err(ProtocolTransformRejection::ResponseReasoningUnsupported);
    }
    project_protocol_request(input)
}

/// Request material prepared for the selected protocol transformation mode.
///
/// Native payload bytes remain owned by the caller. A Canonical projection contains only fields
/// whose target representation is proven by this boundary; diagnostics never print its values.
#[derive(Clone, Eq, PartialEq)]
pub enum ProjectedProtocolRequest {
    /// Forward the caller-owned exact native body after replacing only separately controlled
    /// transport fields such as the selected model.
    NativeExact,
    /// Encode this target-shaped Canonical request with the target protocol's typed builder.
    Canonical(CanonicalRequest),
}

impl fmt::Debug for ProjectedProtocolRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NativeExact => formatter.write_str("ProjectedProtocolRequest::NativeExact"),
            Self::Canonical(_) => {
                formatter.write_str("ProjectedProtocolRequest::Canonical(<redacted>)")
            }
        }
    }
}

/// Analyzes whether one Route candidate can preserve this request's semantics.
///
/// `Passthrough` is intentionally narrow: it accepts only an exact native body to the same
/// protocol, so opaque native fields are still preserved byte-for-byte by the later transport.
/// `Canonical`, `LosslessBridge`, and `CanonicalBridge` reconstruct a body and therefore reject
/// every retained unknown or unsupported semantic rather than guessing a conversion.
#[must_use]
pub fn analyze_protocol_transform(input: ProtocolTransformInput<'_>) -> ProtocolTransformAdmission {
    match project_protocol_request(input) {
        Ok(_) => ProtocolTransformAdmission::Approved,
        Err(rejection) => ProtocolTransformAdmission::Rejected(rejection),
    }
}

/// Produces the typed request material consumed by the target protocol builder.
///
/// Cross-protocol conversion starts from a clean extension namespace and explicitly maps only
/// output limits, ordered text/Tool history, and the fixed legacy Reasoning levels. Anything else
/// fails before Provider selection, so no encoder can accidentally receive foreign raw fields.
///
/// # Errors
///
/// Returns a stable, secret-free rejection when the configured topology, target capabilities, or
/// request shape cannot preserve the requested semantics.
pub fn project_protocol_request(
    input: ProtocolTransformInput<'_>,
) -> Result<ProjectedProtocolRequest, ProtocolTransformRejection> {
    let mode_rejection = match input.mode {
        SnapshotTransformMode::Passthrough if input.source != input.target => {
            Some(ProtocolTransformRejection::PassthroughProtocolMismatch)
        }
        SnapshotTransformMode::Passthrough
            if input.native_payload != NativePayloadAvailability::Exact =>
        {
            Some(ProtocolTransformRejection::PassthroughNativePayloadUnavailable)
        }
        SnapshotTransformMode::Canonical if input.source != input.target => {
            Some(ProtocolTransformRejection::CanonicalProtocolMismatch)
        }
        SnapshotTransformMode::LosslessBridge if input.source == input.target => {
            Some(ProtocolTransformRejection::LosslessBridgeProtocolMatch)
        }
        _ => None,
    };
    if let Some(rejection) = mode_rejection {
        return Err(rejection);
    }

    if input.mode == SnapshotTransformMode::Passthrough {
        capability_rejection(input, input.request)?;
        return Ok(ProjectedProtocolRequest::NativeExact);
    }

    let projected = if input.source == input.target {
        input.request.clone()
    } else {
        project_cross_protocol(input.request, input.source, input.target)?
    };
    canonical_rejection(&projected, input.target)?;
    capability_rejection(input, &projected)?;

    Ok(ProjectedProtocolRequest::Canonical(projected))
}

const CHAT_OUTPUT_LIMIT: &str = "openai.chat.max_tokens";
const RESPONSES_OUTPUT_LIMIT: &str = "openai.responses.max_output_tokens";
const MESSAGES_OUTPUT_LIMIT: &str = "anthropic.messages.max_tokens";
const CHAT_TOOL_CHOICE: &str = "openai.chat.tool_choice";
const RESPONSES_TOOL_CHOICE: &str = "openai.responses.tool_choice";
const MESSAGES_TOOL_CHOICE: &str = "anthropic.messages.tool_choice";
const ANTHROPIC_THINKING_BUDGET: &str = "anthropic.thinking.budget_tokens";

fn project_cross_protocol(
    request: &CanonicalRequest,
    source: ProtocolFormat,
    target: ProtocolFormat,
) -> Result<CanonicalRequest, ProtocolTransformRejection> {
    reject_nested_extensions_and_opaque(request)?;
    if request.prompt_cache_key.is_some() || request.prompt_cache_retention.is_some() {
        return Err(ProtocolTransformRejection::CacheControlUnsupported);
    }

    let mut projected = request.clone();
    projected.extensions = project_root_extensions(
        &request.extensions,
        source,
        target,
        !request.tools.is_empty(),
    )?;
    projected.thinking = project_thinking(request.thinking.as_ref(), source, target)?;
    if target == ProtocolFormat::AnthropicMessages {
        project_messages_roles(&mut projected)?;
    }
    Ok(projected)
}

fn reject_nested_extensions_and_opaque(
    request: &CanonicalRequest,
) -> Result<(), ProtocolTransformRejection> {
    for message in &request.messages {
        if !message.extensions.is_empty() {
            return Err(ProtocolTransformRejection::UnknownMessageExtensions);
        }
        for content in &message.content {
            match content {
                MessageContent::Text(text) if !text.extensions.is_empty() => {
                    return Err(ProtocolTransformRejection::UnknownContentExtensions);
                }
                MessageContent::ToolCall(call) if !call.extensions.is_empty() => {
                    return Err(ProtocolTransformRejection::UnknownContentExtensions);
                }
                MessageContent::ToolResult(result) if !result.extensions.is_empty() => {
                    return Err(ProtocolTransformRejection::UnknownContentExtensions);
                }
                MessageContent::Opaque(_) => {
                    return Err(ProtocolTransformRejection::OpaqueContent);
                }
                MessageContent::Text(_)
                | MessageContent::ToolCall(_)
                | MessageContent::ToolResult(_) => {}
            }
        }
    }
    if request.tools.iter().any(|tool| !tool.extensions.is_empty()) {
        return Err(ProtocolTransformRejection::UnknownToolDefinitionExtensions);
    }
    Ok(())
}

fn project_root_extensions(
    extensions: &RawExtensions,
    source: ProtocolFormat,
    target: ProtocolFormat,
    has_tools: bool,
) -> Result<RawExtensions, ProtocolTransformRejection> {
    let source_output_limit = output_limit_name(source);
    let source_tool_choice = tool_choice_name(source);
    let mut output_limit = None;
    let mut tool_choice = None;
    for (name, raw) in extensions.iter() {
        if name == source_output_limit {
            if output_limit.replace(raw.clone()).is_some() {
                return Err(ProtocolTransformRejection::OutputLimitCollision);
            }
        } else if name == source_tool_choice {
            if tool_choice.replace(raw.clone()).is_some() {
                return Err(ProtocolTransformRejection::UnknownRequestExtensions);
            }
        } else {
            if matches!(
                name,
                CHAT_OUTPUT_LIMIT | RESPONSES_OUTPUT_LIMIT | MESSAGES_OUTPUT_LIMIT
            ) {
                return Err(ProtocolTransformRejection::OutputLimitCollision);
            }
            return Err(ProtocolTransformRejection::UnknownRequestExtensions);
        }
    }

    let mut projected = RawExtensions::default();
    match output_limit {
        Some(raw) => {
            validate_output_limit(&raw)?;
            projected
                .try_insert(output_limit_name(target), raw)
                .map_err(|_| ProtocolTransformRejection::OutputLimitCollision)?;
        }
        None if target == ProtocolFormat::AnthropicMessages => {
            return Err(ProtocolTransformRejection::OutputLimitMissing);
        }
        None => {}
    }
    if let Some(raw) = tool_choice {
        let mapped = project_forced_tool_choice(&raw, source, target, has_tools)?;
        projected
            .try_insert(tool_choice_name(target), mapped)
            .map_err(|_| ProtocolTransformRejection::UnknownRequestExtensions)?;
    }
    Ok(projected)
}

fn project_forced_tool_choice(
    raw: &RawJson,
    source: ProtocolFormat,
    target: ProtocolFormat,
    has_tools: bool,
) -> Result<RawJson, ProtocolTransformRejection> {
    if !has_tools {
        return Err(ProtocolTransformRejection::UnknownRequestExtensions);
    }
    let value = serde_json::from_str::<Value>(raw.get())
        .map_err(|_| ProtocolTransformRejection::UnknownRequestExtensions)?;
    let forced = match source {
        ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses => {
            value.as_str() == Some("required")
        }
        ProtocolFormat::AnthropicMessages => value.as_object().is_some_and(|choice| {
            choice.len() == 1 && choice.get("type").and_then(Value::as_str) == Some("any")
        }),
    };
    if !forced {
        return Err(ProtocolTransformRejection::UnknownRequestExtensions);
    }
    let mapped = match target {
        ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses => {
            r#""required""#.to_owned()
        }
        ProtocolFormat::AnthropicMessages => r#"{"type":"any"}"#.to_owned(),
    };
    RawJson::from_json_string(mapped)
        .map_err(|_| ProtocolTransformRejection::UnknownRequestExtensions)
}

const fn output_limit_name(protocol: ProtocolFormat) -> &'static str {
    match protocol {
        ProtocolFormat::OpenAiChatCompletions => CHAT_OUTPUT_LIMIT,
        ProtocolFormat::OpenAiResponses => RESPONSES_OUTPUT_LIMIT,
        ProtocolFormat::AnthropicMessages => MESSAGES_OUTPUT_LIMIT,
    }
}

fn validate_output_limit(raw: &RawJson) -> Result<(), ProtocolTransformRejection> {
    match serde_json::from_str::<Value>(raw.get()) {
        Ok(Value::Number(value)) if value.as_u64().is_some_and(|value| value > 0) => Ok(()),
        _ => Err(ProtocolTransformRejection::OutputLimitInvalid),
    }
}

fn project_messages_roles(
    request: &mut CanonicalRequest,
) -> Result<(), ProtocolTransformRejection> {
    for (position, message) in request.messages.iter_mut().enumerate() {
        if message.role.0 == "developer" {
            if position != 0 {
                return Err(ProtocolTransformRejection::IncompatibleRole);
            }
            message.role = MessageRole("system".to_owned());
        }
    }
    Ok(())
}

fn project_thinking(
    thinking: Option<&Thinking>,
    source: ProtocolFormat,
    target: ProtocolFormat,
) -> Result<Option<Thinking>, ProtocolTransformRejection> {
    let Some(thinking) = thinking else {
        return Ok(None);
    };
    if source == ProtocolFormat::OpenAiChatCompletions
        || target == ProtocolFormat::OpenAiChatCompletions
    {
        return Err(ProtocolTransformRejection::ThinkingUnsupported);
    }

    match (source, target) {
        (ProtocolFormat::OpenAiResponses, ProtocolFormat::AnthropicMessages) => {
            if !thinking.extensions.is_empty() {
                return Err(ProtocolTransformRejection::ThinkingUnsupported);
            }
            responses_thinking_to_messages(thinking)
        }
        (ProtocolFormat::AnthropicMessages, ProtocolFormat::OpenAiResponses) => {
            messages_thinking_to_responses(thinking)
        }
        _ => Err(ProtocolTransformRejection::ThinkingUnsupported),
    }
    .map(Some)
}

fn responses_thinking_to_messages(
    thinking: &Thinking,
) -> Result<Thinking, ProtocolTransformRejection> {
    let (effort, budget) = match thinking.effort.as_str() {
        "none" => ("disabled", None),
        "auto" => ("adaptive", None),
        "minimal" => ("enabled", Some(512)),
        "low" => ("enabled", Some(1_024)),
        "medium" => ("enabled", Some(8_192)),
        "high" => ("enabled", Some(24_576)),
        "xhigh" => ("enabled", Some(32_768)),
        "max" => ("enabled", Some(128_000)),
        _ => return Err(ProtocolTransformRejection::ThinkingUnsupported),
    };
    let mut extensions = RawExtensions::default();
    if let Some(budget) = budget {
        let raw = RawJson::from_json_string(budget.to_string())
            .map_err(|_| ProtocolTransformRejection::ThinkingUnsupported)?;
        extensions
            .try_insert(ANTHROPIC_THINKING_BUDGET, raw)
            .map_err(|_| ProtocolTransformRejection::ThinkingUnsupported)?;
    }
    Ok(Thinking {
        effort: ThinkingEffort::try_new(effort)
            .map_err(|_| ProtocolTransformRejection::ThinkingUnsupported)?,
        extensions,
    })
}

fn messages_thinking_to_responses(
    thinking: &Thinking,
) -> Result<Thinking, ProtocolTransformRejection> {
    let mut budget = None;
    for (name, raw) in thinking.extensions.iter() {
        if name != ANTHROPIC_THINKING_BUDGET || budget.is_some() {
            return Err(ProtocolTransformRejection::ThinkingUnsupported);
        }
        budget = match serde_json::from_str::<Value>(raw.get()) {
            Ok(Value::Number(value)) if value.as_u64().is_some_and(|value| value > 0) => {
                value.as_u64()
            }
            _ => return Err(ProtocolTransformRejection::ThinkingUnsupported),
        };
    }
    let effort = match thinking.effort.as_str() {
        "disabled" if budget.is_none() => "none",
        "adaptive" if budget.is_none() => "auto",
        "enabled" => budget.map_or("auto", budget_to_responses_effort),
        _ => return Err(ProtocolTransformRejection::ThinkingUnsupported),
    };
    Ok(Thinking {
        effort: ThinkingEffort::try_new(effort)
            .map_err(|_| ProtocolTransformRejection::ThinkingUnsupported)?,
        extensions: RawExtensions::default(),
    })
}

const fn budget_to_responses_effort(budget: u64) -> &'static str {
    match budget {
        1..=512 => "minimal",
        513..=1_024 => "low",
        1_025..=8_192 => "medium",
        8_193..=24_576 => "high",
        _ => "xhigh",
    }
}

fn canonical_rejection(
    request: &CanonicalRequest,
    target: ProtocolFormat,
) -> Result<(), ProtocolTransformRejection> {
    validate_target_root_extensions(request, target)?;
    if request.prompt_cache_key.is_some() || request.prompt_cache_retention.is_some() {
        return Err(ProtocolTransformRejection::CacheControlUnsupported);
    }
    if request.messages.is_empty() {
        return Err(ProtocolTransformRejection::IncompatibleRole);
    }

    for (position, message) in request.messages.iter().enumerate() {
        if !message.extensions.is_empty() {
            return Err(ProtocolTransformRejection::UnknownMessageExtensions);
        }
        for content in &message.content {
            match content {
                MessageContent::Text(text) if !text.extensions.is_empty() => {
                    return Err(ProtocolTransformRejection::UnknownContentExtensions);
                }
                MessageContent::Opaque(_) => {
                    return Err(ProtocolTransformRejection::OpaqueContent);
                }
                MessageContent::ToolCall(call) if !call.extensions.is_empty() => {
                    return Err(ProtocolTransformRejection::UnknownContentExtensions);
                }
                MessageContent::ToolResult(result) if !result.extensions.is_empty() => {
                    return Err(ProtocolTransformRejection::UnknownContentExtensions);
                }
                MessageContent::Text(_)
                | MessageContent::ToolCall(_)
                | MessageContent::ToolResult(_) => {}
            }
        }
        if !target_supports_role(target, &message.role.0) {
            return Err(ProtocolTransformRejection::IncompatibleRole);
        }
        validate_target_message(message, position, target)?;
    }

    for tool in &request.tools {
        if !tool.extensions.is_empty() {
            return Err(ProtocolTransformRejection::UnknownToolDefinitionExtensions);
        }
        if tool.name.is_empty()
            || !serde_json::from_str::<Value>(tool.input_schema.get())
                .is_ok_and(|value| value.is_object())
        {
            return Err(ProtocolTransformRejection::ToolHistoryUnsupported);
        }
    }

    validate_target_thinking(request.thinking.as_ref(), target)
}

fn validate_target_root_extensions(
    request: &CanonicalRequest,
    target: ProtocolFormat,
) -> Result<(), ProtocolTransformRejection> {
    let expected_output_limit = output_limit_name(target);
    let expected_tool_choice = tool_choice_name(target);
    for (name, raw) in request.extensions.iter() {
        if name == expected_output_limit {
            validate_output_limit(raw)?;
        } else if name == expected_tool_choice {
            validate_tool_choice(raw, target, !request.tools.is_empty())?;
        } else {
            if matches!(
                name,
                CHAT_OUTPUT_LIMIT | RESPONSES_OUTPUT_LIMIT | MESSAGES_OUTPUT_LIMIT
            ) {
                return Err(ProtocolTransformRejection::OutputLimitCollision);
            }
            return Err(ProtocolTransformRejection::UnknownRequestExtensions);
        }
    }
    if target == ProtocolFormat::AnthropicMessages
        && request.extensions.get(expected_output_limit).is_none()
    {
        return Err(ProtocolTransformRejection::OutputLimitMissing);
    }
    Ok(())
}

const fn tool_choice_name(protocol: ProtocolFormat) -> &'static str {
    match protocol {
        ProtocolFormat::OpenAiChatCompletions => CHAT_TOOL_CHOICE,
        ProtocolFormat::OpenAiResponses => RESPONSES_TOOL_CHOICE,
        ProtocolFormat::AnthropicMessages => MESSAGES_TOOL_CHOICE,
    }
}

fn validate_tool_choice(
    raw: &RawJson,
    target: ProtocolFormat,
    has_tools: bool,
) -> Result<(), ProtocolTransformRejection> {
    let value = serde_json::from_str::<Value>(raw.get())
        .map_err(|_| ProtocolTransformRejection::UnknownRequestExtensions)?;
    let valid = match target {
        ProtocolFormat::OpenAiChatCompletions => value.as_str() == Some("required") && has_tools,
        ProtocolFormat::OpenAiResponses => match value.as_str() {
            Some("auto") => true,
            Some("required") => has_tools,
            _ => false,
        },
        ProtocolFormat::AnthropicMessages => match value.as_object() {
            Some(choice) if choice.len() == 1 => match choice.get("type").and_then(Value::as_str) {
                Some("auto") => true,
                Some("any") => has_tools,
                _ => false,
            },
            _ => false,
        },
    };
    if valid {
        Ok(())
    } else {
        Err(ProtocolTransformRejection::UnknownRequestExtensions)
    }
}

fn validate_target_thinking(
    thinking: Option<&Thinking>,
    target: ProtocolFormat,
) -> Result<(), ProtocolTransformRejection> {
    let Some(thinking) = thinking else {
        return Ok(());
    };
    match target {
        ProtocolFormat::OpenAiChatCompletions => {
            Err(ProtocolTransformRejection::ThinkingUnsupported)
        }
        ProtocolFormat::OpenAiResponses if thinking.extensions.is_empty() => {
            match thinking.effort.as_str() {
                "none" | "auto" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" => Ok(()),
                _ => Err(ProtocolTransformRejection::ThinkingUnsupported),
            }
        }
        ProtocolFormat::AnthropicMessages => messages_thinking_to_responses(thinking).map(|_| ()),
        ProtocolFormat::OpenAiResponses => Err(ProtocolTransformRejection::ThinkingUnsupported),
    }
}

fn validate_target_message(
    message: &gateway_core::CanonicalMessage,
    position: usize,
    target: ProtocolFormat,
) -> Result<(), ProtocolTransformRejection> {
    if message.content.is_empty() {
        return Err(ProtocolTransformRejection::IncompatibleRole);
    }
    let role = message.role.0.as_str();
    match target {
        ProtocolFormat::OpenAiChatCompletions => match role {
            "system" | "developer" | "user"
                if matches!(message.content.as_slice(), [MessageContent::Text(_)]) => {}
            "assistant" if valid_assistant_tool_history(&message.content) => {}
            "tool" if valid_tool_result_message(&message.content, target) => {}
            _ => return Err(ProtocolTransformRejection::ToolHistoryUnsupported),
        },
        ProtocolFormat::OpenAiResponses => {
            if !valid_responses_content(role, &message.content) {
                return Err(ProtocolTransformRejection::ToolHistoryUnsupported);
            }
        }
        ProtocolFormat::AnthropicMessages => {
            if role == "system" && position != 0 {
                return Err(ProtocolTransformRejection::IncompatibleRole);
            }
            if role == "tool" && !valid_tool_result_message(&message.content, target) {
                return Err(ProtocolTransformRejection::ToolHistoryUnsupported);
            }
            if role == "assistant" && !valid_assistant_tool_history(&message.content) {
                return Err(ProtocolTransformRejection::ToolHistoryUnsupported);
            }
            if matches!(role, "system" | "user")
                && message
                    .content
                    .iter()
                    .any(|part| !matches!(part, MessageContent::Text(_)))
            {
                return Err(ProtocolTransformRejection::ToolHistoryUnsupported);
            }
        }
    }
    Ok(())
}

fn valid_assistant_tool_history(content: &[MessageContent]) -> bool {
    let mut saw_text = false;
    content.iter().enumerate().all(|(index, part)| match part {
        MessageContent::Text(_) if index == 0 && !saw_text => {
            saw_text = true;
            true
        }
        MessageContent::ToolCall(call) => !call.id.is_empty() && !call.name.is_empty(),
        _ => false,
    })
}

fn valid_tool_result_message(content: &[MessageContent], target: ProtocolFormat) -> bool {
    let [MessageContent::ToolResult(result)] = content else {
        return false;
    };
    if result.call_id.is_empty() {
        return false;
    }
    match target {
        ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses => {
            !result.is_error
                && serde_json::from_str::<Value>(result.output.get()).is_ok_and(|v| v.is_string())
        }
        ProtocolFormat::AnthropicMessages => true,
    }
}

fn valid_responses_content(role: &str, content: &[MessageContent]) -> bool {
    match role {
        "system" | "developer" | "user" => content
            .iter()
            .all(|part| matches!(part, MessageContent::Text(_))),
        "assistant" => valid_assistant_tool_history(content),
        "tool" => valid_tool_result_message(content, ProtocolFormat::OpenAiResponses),
        _ => false,
    }
}

fn target_supports_role(target: ProtocolFormat, role: &str) -> bool {
    match target {
        ProtocolFormat::OpenAiChatCompletions => {
            matches!(role, "system" | "developer" | "user" | "assistant" | "tool")
        }
        ProtocolFormat::OpenAiResponses => {
            matches!(role, "system" | "developer" | "user" | "assistant" | "tool")
        }
        ProtocolFormat::AnthropicMessages => {
            matches!(role, "system" | "user" | "assistant" | "tool")
        }
    }
}

fn capability_rejection(
    input: ProtocolTransformInput<'_>,
    request: &CanonicalRequest,
) -> Result<(), ProtocolTransformRejection> {
    let capabilities = input.target_capabilities;
    if input.streaming && !capabilities.supports(SemanticCapability::Streaming) {
        return Err(ProtocolTransformRejection::StreamingUnsupported);
    }

    let has_tools = !request.tools.is_empty()
        || request.messages.iter().any(|message| {
            message.content.iter().any(|content| {
                matches!(
                    content,
                    MessageContent::ToolCall(_) | MessageContent::ToolResult(_)
                )
            })
        });
    if (has_tools || input.requires_parallel_tools)
        && !capabilities.supports(SemanticCapability::Tools)
    {
        return Err(ProtocolTransformRejection::ToolsUnsupported);
    }
    if (has_tools || input.requires_json_schema)
        && !capabilities.supports(SemanticCapability::JsonSchema)
    {
        return Err(ProtocolTransformRejection::JsonSchemaUnsupported);
    }
    if input.requires_parallel_tools && !capabilities.supports(SemanticCapability::ParallelTools) {
        return Err(ProtocolTransformRejection::ParallelToolsUnsupported);
    }
    if request.thinking.is_some() && !capabilities.supports(SemanticCapability::Reasoning) {
        return Err(ProtocolTransformRejection::ReasoningUnsupported);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_catalog::{CapabilitySet, SemanticCapability};
    use gateway_core::{
        CanonicalMessage, CanonicalRequest, MessageContent, MessageRole, OpaqueContent,
        RawExtensions, RawJson, TextContent, Thinking, ThinkingEffort, ToolCall, ToolDefinition,
        ToolResult,
    };
    use gateway_protocol::ApiFormat;
    use proptest::prelude::*;
    use protocol_anthropic::ResponseMode as AnthropicResponseMode;
    use protocol_openai_chat::ResponseMode as ChatResponseMode;
    use protocol_openai_responses::ResponseMode as ResponsesResponseMode;
    use provider_anthropic_compatible::{
        AnthropicMessagesApiKey, AnthropicMessagesEndpoint, AnthropicMessagesRequestBuilder,
    };
    use provider_openai_compatible::{
        OpenAiChatCompletionsApiKey, OpenAiChatCompletionsEndpoint,
        OpenAiChatCompletionsRequestBuilder, OpenAiResponsesApiKey, OpenAiResponsesEndpoint,
        OpenAiResponsesRequestBuilder,
    };
    use serde_json::Value;

    use super::{
        NativePayloadAvailability, ProjectedProtocolRequest, ProtocolFormat,
        ProtocolTransformAdmission, ProtocolTransformInput, ProtocolTransformRejection,
        analyze_protocol_transform, project_protocol_request, project_registered_protocol_request,
        protocol_pair_is_publishable, protocol_pair_is_registered,
    };
    use crate::SnapshotTransformMode;

    fn request() -> CanonicalRequest {
        CanonicalRequest {
            requested_model: "private-model".to_owned(),
            messages: vec![text_message("user", "private prompt")],
            tools: Vec::new(),
            thinking: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            extensions: RawExtensions::default(),
        }
    }

    fn text_message(role: &str, text: &str) -> CanonicalMessage {
        CanonicalMessage {
            role: MessageRole(role.to_owned()),
            content: vec![MessageContent::Text(TextContent {
                text: text.to_owned(),
                extensions: RawExtensions::default(),
            })],
            extensions: RawExtensions::default(),
        }
    }

    fn extension() -> Result<RawExtensions, Box<dyn std::error::Error>> {
        let mut extensions = RawExtensions::default();
        extensions.try_insert(
            "vendor.private",
            RawJson::from_json_string(r#"{"private":true}"#.to_owned())?,
        )?;
        Ok(extensions)
    }

    fn with_output_limit(
        mut request: CanonicalRequest,
        protocol: ProtocolFormat,
        value: u64,
    ) -> Result<CanonicalRequest, Box<dyn std::error::Error>> {
        let name = super::output_limit_name(protocol);
        request
            .extensions
            .try_insert(name, RawJson::from_json_string(value.to_string())?)?;
        Ok(request)
    }

    fn capabilities(
        capabilities: impl IntoIterator<Item = SemanticCapability>,
    ) -> Result<CapabilitySet, Box<dyn std::error::Error>> {
        Ok(CapabilitySet::try_new(capabilities)?)
    }

    fn input<'a>(
        request: &'a CanonicalRequest,
        source: ProtocolFormat,
        target: ProtocolFormat,
        mode: SnapshotTransformMode,
        native_payload: NativePayloadAvailability,
        target_capabilities: &'a CapabilitySet,
    ) -> ProtocolTransformInput<'a> {
        ProtocolTransformInput {
            source,
            target,
            mode,
            native_payload,
            request,
            streaming: false,
            requires_json_schema: false,
            requires_parallel_tools: false,
            target_capabilities,
        }
    }

    fn assert_rejected(
        admission: ProtocolTransformAdmission,
        expected: ProtocolTransformRejection,
    ) {
        assert_eq!(admission, ProtocolTransformAdmission::Rejected(expected));
    }

    #[test]
    fn endpoint_api_formats_are_exact_and_do_not_infer_unknown_protocols() {
        assert_eq!(
            ProtocolFormat::OpenAiChatCompletions.api_format(),
            "openai/chat-completions"
        );
        assert_eq!(
            ProtocolFormat::from_api_format("openai/chat-completions"),
            Some(ProtocolFormat::OpenAiChatCompletions)
        );
        assert_eq!(
            ProtocolFormat::OpenAiResponses.api_format(),
            "openai/responses"
        );
        assert_eq!(
            ProtocolFormat::OpenAiChatCompletions.as_api_format(),
            ApiFormat::OpenAiChatCompletions
        );
        assert_eq!(
            ProtocolFormat::AnthropicMessages.api_format(),
            "anthropic/messages"
        );
        assert_eq!(
            ProtocolFormat::from_api_format("openai/responses"),
            Some(ProtocolFormat::OpenAiResponses)
        );
        assert_eq!(
            ProtocolFormat::from_api_format("anthropic/messages"),
            Some(ProtocolFormat::AnthropicMessages)
        );
        assert_eq!(ProtocolFormat::from_api_format("openai_responses"), None);
        assert_eq!(
            ProtocolFormat::from_api_format("openai/chat_completions"),
            None
        );
    }

    #[test]
    fn reviewed_registry_publishes_only_the_nine_pairs_with_their_exact_modes() {
        let protocols = [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
        ];
        for source in protocols {
            for target in protocols {
                assert!(protocol_pair_is_registered(source, target));
                for mode in [
                    SnapshotTransformMode::Passthrough,
                    SnapshotTransformMode::Canonical,
                    SnapshotTransformMode::LosslessBridge,
                    SnapshotTransformMode::CanonicalBridge,
                ] {
                    assert_eq!(
                        protocol_pair_is_publishable(source, target, mode, &CapabilitySet::empty(),),
                        if mode == SnapshotTransformMode::CanonicalBridge {
                            true
                        } else if source == target {
                            matches!(
                                mode,
                                SnapshotTransformMode::Passthrough
                                    | SnapshotTransformMode::Canonical
                            )
                        } else {
                            mode == SnapshotTransformMode::LosslessBridge
                        },
                    );
                }
            }
        }
    }

    #[test]
    fn chat_response_reasoning_is_rejected_before_request_projection()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = request();
        let reasoning = capabilities([SemanticCapability::Reasoning])?;
        assert_eq!(
            project_registered_protocol_request(input(
                &request,
                ProtocolFormat::OpenAiChatCompletions,
                ProtocolFormat::OpenAiChatCompletions,
                SnapshotTransformMode::Canonical,
                NativePayloadAvailability::Exact,
                &reasoning,
            )),
            Err(ProtocolTransformRejection::ResponseReasoningUnsupported),
        );
        assert!(!protocol_pair_is_publishable(
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiChatCompletions,
            SnapshotTransformMode::Canonical,
            &reasoning,
        ));
        Ok(())
    }

    #[test]
    fn protocol_formats_agree_with_the_shared_api_format_table() {
        use gateway_protocol::ApiFormat;

        for format in ApiFormat::ALL {
            let protocol = ProtocolFormat::from_api_format(format.as_str());
            assert_eq!(protocol.map(ProtocolFormat::as_api_format), Some(format));
            assert_eq!(
                protocol.map(ProtocolFormat::api_format),
                Some(format.as_str())
            );
        }
        assert_eq!(
            ProtocolFormat::OpenAiResponses.as_api_format(),
            ApiFormat::OpenAiResponses
        );
        assert_eq!(
            ProtocolFormat::AnthropicMessages.as_api_format(),
            ApiFormat::AnthropicMessages
        );
    }

    #[test]
    fn passthrough_requires_same_protocol_and_an_exact_native_payload()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut request = request();
        request.extensions = extension()?;
        let no_capabilities = CapabilitySet::empty();

        assert_rejected(
            analyze_protocol_transform(input(
                &request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::AnthropicMessages,
                SnapshotTransformMode::Passthrough,
                NativePayloadAvailability::Exact,
                &no_capabilities,
            )),
            ProtocolTransformRejection::PassthroughProtocolMismatch,
        );
        assert_rejected(
            analyze_protocol_transform(input(
                &request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::OpenAiResponses,
                SnapshotTransformMode::Passthrough,
                NativePayloadAvailability::Unavailable,
                &no_capabilities,
            )),
            ProtocolTransformRejection::PassthroughNativePayloadUnavailable,
        );
        assert_eq!(
            analyze_protocol_transform(input(
                &request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::OpenAiResponses,
                SnapshotTransformMode::Passthrough,
                NativePayloadAvailability::Exact,
                &no_capabilities,
            )),
            ProtocolTransformAdmission::Approved
        );
        Ok(())
    }

    #[test]
    fn canonical_and_lossless_bridge_enforce_their_topology() {
        let request = request();
        let no_capabilities = CapabilitySet::empty();

        assert_rejected(
            analyze_protocol_transform(input(
                &request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::AnthropicMessages,
                SnapshotTransformMode::Canonical,
                NativePayloadAvailability::Unavailable,
                &no_capabilities,
            )),
            ProtocolTransformRejection::CanonicalProtocolMismatch,
        );
        assert_rejected(
            analyze_protocol_transform(input(
                &request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::OpenAiResponses,
                SnapshotTransformMode::LosslessBridge,
                NativePayloadAvailability::Unavailable,
                &no_capabilities,
            )),
            ProtocolTransformRejection::LosslessBridgeProtocolMatch,
        );
    }

    #[test]
    fn canonical_bridge_supports_native_and_cross_protocol_projection() -> Result<(), Box<dyn Error>>
    {
        let request = request();
        let capabilities = all_capabilities()?;
        for (source, target) in [
            (
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::OpenAiResponses,
            ),
            (
                ProtocolFormat::OpenAiChatCompletions,
                ProtocolFormat::OpenAiResponses,
            ),
            (
                ProtocolFormat::AnthropicMessages,
                ProtocolFormat::OpenAiResponses,
            ),
        ] {
            assert_eq!(
                analyze_protocol_transform(input(
                    &request,
                    source,
                    target,
                    SnapshotTransformMode::CanonicalBridge,
                    NativePayloadAvailability::Unavailable,
                    &capabilities,
                )),
                ProtocolTransformAdmission::Approved,
            );
        }
        Ok(())
    }

    #[test]
    fn same_protocol_canonical_admits_only_valid_target_tool_choice()
    -> Result<(), Box<dyn std::error::Error>> {
        let all_capabilities = all_capabilities()?;
        for (protocol, raw_choice) in [
            (ProtocolFormat::OpenAiChatCompletions, r#""required""#),
            (ProtocolFormat::OpenAiResponses, r#""required""#),
            (ProtocolFormat::AnthropicMessages, r#"{"type":"any"}"#),
        ] {
            let mut request = request();
            request.tools.push(ToolDefinition {
                name: "private-tool".to_owned(),
                description: None,
                input_schema: RawJson::from_json_string(r#"{"type":"object"}"#.to_owned())?,
                extensions: RawExtensions::default(),
            });
            if protocol == ProtocolFormat::AnthropicMessages {
                request = with_output_limit(request, protocol, 64)?;
            }
            request.extensions.try_insert(
                super::tool_choice_name(protocol),
                RawJson::from_json_string(raw_choice.to_owned())?,
            )?;

            let projected = canonical_projection(project_protocol_request(input(
                &request,
                protocol,
                protocol,
                SnapshotTransformMode::Canonical,
                NativePayloadAvailability::Unavailable,
                &all_capabilities,
            )))?;
            assert_eq!(projected, request);
            let body = match protocol {
                ProtocolFormat::OpenAiChatCompletions => {
                    OpenAiChatCompletionsRequestBuilder::build(
                        &OpenAiChatCompletionsEndpoint::try_new(
                            "https://relay.example",
                            "/v1/chat/completions",
                        )?,
                        &OpenAiChatCompletionsApiKey::try_new("secret")?,
                        "upstream-model",
                        &projected,
                        ChatResponseMode::NonStreaming,
                    )?
                    .body()
                    .to_vec()
                }
                ProtocolFormat::OpenAiResponses => OpenAiResponsesRequestBuilder::build(
                    &OpenAiResponsesEndpoint::try_new("https://relay.example", "/v1/responses")?,
                    &OpenAiResponsesApiKey::try_new("secret")?,
                    "upstream-model",
                    &projected,
                    ResponsesResponseMode::NonStreaming,
                )?
                .body()
                .to_vec(),
                ProtocolFormat::AnthropicMessages => AnthropicMessagesRequestBuilder::build(
                    &AnthropicMessagesEndpoint::try_new("https://relay.example", "/v1/messages")?,
                    &AnthropicMessagesApiKey::try_new("secret")?,
                    "upstream-model",
                    &projected,
                    AnthropicResponseMode::NonStreaming,
                )?
                .body()
                .to_vec(),
            };
            let wire: serde_json::Value = serde_json::from_slice(&body)?;
            assert_eq!(
                wire.get("tool_choice"),
                Some(&serde_json::from_str(raw_choice)?)
            );
        }
        Ok(())
    }

    #[test]
    fn target_tool_choice_validation_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
        let all_capabilities = all_capabilities()?;
        for (protocol, raw_choice) in [
            (ProtocolFormat::OpenAiChatCompletions, r#""required""#),
            (ProtocolFormat::OpenAiResponses, r#""required""#),
            (ProtocolFormat::AnthropicMessages, r#"{"type":"any"}"#),
        ] {
            let mut request = request();
            if protocol == ProtocolFormat::AnthropicMessages {
                request = with_output_limit(request, protocol, 64)?;
            }
            request.extensions.try_insert(
                super::tool_choice_name(protocol),
                RawJson::from_json_string(raw_choice.to_owned())?,
            )?;
            assert_eq!(
                project_protocol_request(input(
                    &request,
                    protocol,
                    protocol,
                    SnapshotTransformMode::Canonical,
                    NativePayloadAvailability::Unavailable,
                    &all_capabilities,
                )),
                Err(ProtocolTransformRejection::UnknownRequestExtensions),
            );
        }

        let mut malformed_messages =
            with_output_limit(request(), ProtocolFormat::AnthropicMessages, 64)?;
        malformed_messages.tools.push(ToolDefinition {
            name: "private-tool".to_owned(),
            description: None,
            input_schema: RawJson::from_json_string("{}".to_owned())?,
            extensions: RawExtensions::default(),
        });
        malformed_messages.extensions.try_insert(
            super::MESSAGES_TOOL_CHOICE,
            RawJson::from_json_string(r#"{"type":"any","extra":true}"#.to_owned())?,
        )?;
        assert_eq!(
            project_protocol_request(input(
                &malformed_messages,
                ProtocolFormat::AnthropicMessages,
                ProtocolFormat::AnthropicMessages,
                SnapshotTransformMode::Canonical,
                NativePayloadAvailability::Unavailable,
                &all_capabilities,
            )),
            Err(ProtocolTransformRejection::UnknownRequestExtensions),
        );
        Ok(())
    }

    #[test]
    fn forced_tool_choice_uses_the_reviewed_cross_protocol_mapping()
    -> Result<(), Box<dyn std::error::Error>> {
        let all_capabilities = all_capabilities()?;
        for source in [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
        ] {
            for target in [
                ProtocolFormat::OpenAiChatCompletions,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::AnthropicMessages,
            ] {
                if source == target {
                    continue;
                }
                let mut request = with_output_limit(request(), source, 64)?;
                request.tools.push(ToolDefinition {
                    name: "private-tool".to_owned(),
                    description: None,
                    input_schema: RawJson::from_json_string(r#"{"type":"object"}"#.to_owned())?,
                    extensions: RawExtensions::default(),
                });
                let source_choice = match source {
                    ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses => {
                        r#""required""#
                    }
                    ProtocolFormat::AnthropicMessages => r#"{"type":"any"}"#,
                };
                request.extensions.try_insert(
                    super::tool_choice_name(source),
                    RawJson::from_json_string(source_choice.to_owned())?,
                )?;
                let projected = canonical_projection(project_protocol_request(input(
                    &request,
                    source,
                    target,
                    SnapshotTransformMode::LosslessBridge,
                    NativePayloadAvailability::Unavailable,
                    &all_capabilities,
                )))?;
                let expected = match target {
                    ProtocolFormat::OpenAiChatCompletions | ProtocolFormat::OpenAiResponses => {
                        serde_json::json!("required")
                    }
                    ProtocolFormat::AnthropicMessages => serde_json::json!({"type": "any"}),
                };
                let mapped = projected
                    .extensions
                    .get(super::tool_choice_name(target))
                    .ok_or("missing mapped tool choice")?;
                assert_eq!(serde_json::from_str::<Value>(mapped.get())?, expected);

                let body = match target {
                    ProtocolFormat::OpenAiChatCompletions => {
                        OpenAiChatCompletionsRequestBuilder::build(
                            &OpenAiChatCompletionsEndpoint::try_new(
                                "https://relay.example",
                                "/v1/chat/completions",
                            )?,
                            &OpenAiChatCompletionsApiKey::try_new("secret")?,
                            "upstream-model",
                            &projected,
                            ChatResponseMode::NonStreaming,
                        )?
                        .body()
                        .to_vec()
                    }
                    ProtocolFormat::OpenAiResponses => OpenAiResponsesRequestBuilder::build(
                        &OpenAiResponsesEndpoint::try_new(
                            "https://relay.example",
                            "/v1/responses",
                        )?,
                        &OpenAiResponsesApiKey::try_new("secret")?,
                        "upstream-model",
                        &projected,
                        ResponsesResponseMode::NonStreaming,
                    )?
                    .body()
                    .to_vec(),
                    ProtocolFormat::AnthropicMessages => AnthropicMessagesRequestBuilder::build(
                        &AnthropicMessagesEndpoint::try_new(
                            "https://relay.example",
                            "/v1/messages",
                        )?,
                        &AnthropicMessagesApiKey::try_new("secret")?,
                        "upstream-model",
                        &projected,
                        AnthropicResponseMode::NonStreaming,
                    )?
                    .body()
                    .to_vec(),
                };
                let wire: Value = serde_json::from_slice(&body)?;
                assert_eq!(wire.get("tool_choice"), Some(&expected));
            }
        }
        Ok(())
    }

    #[test]
    fn automatic_tool_choice_still_fails_closed_across_protocols()
    -> Result<(), Box<dyn std::error::Error>> {
        let all_capabilities = all_capabilities()?;
        let mut with_tools = with_output_limit(request(), ProtocolFormat::OpenAiResponses, 64)?;
        with_tools.tools.push(ToolDefinition {
            name: "private-tool".to_owned(),
            description: None,
            input_schema: RawJson::from_json_string("{}".to_owned())?,
            extensions: RawExtensions::default(),
        });
        with_tools.extensions.try_insert(
            super::RESPONSES_TOOL_CHOICE,
            RawJson::from_json_string(r#""auto""#.to_owned())?,
        )?;
        assert_eq!(
            project_protocol_request(input(
                &with_tools,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::OpenAiChatCompletions,
                SnapshotTransformMode::LosslessBridge,
                NativePayloadAvailability::Unavailable,
                &all_capabilities,
            )),
            Err(ProtocolTransformRejection::UnknownRequestExtensions),
        );

        let mut without_tools = with_output_limit(request(), ProtocolFormat::OpenAiResponses, 64)?;
        without_tools.extensions.try_insert(
            super::RESPONSES_TOOL_CHOICE,
            RawJson::from_json_string(r#""required""#.to_owned())?,
        )?;
        assert_eq!(
            project_protocol_request(input(
                &without_tools,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::AnthropicMessages,
                SnapshotTransformMode::LosslessBridge,
                NativePayloadAvailability::Unavailable,
                &all_capabilities,
            )),
            Err(ProtocolTransformRejection::UnknownRequestExtensions),
        );
        Ok(())
    }

    #[test]
    fn canonical_and_bridge_reject_extensions_and_opaque_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let all_capabilities = all_capabilities()?;

        let mut with_request_extension = request();
        with_request_extension.extensions = extension()?;
        assert_rejection_for_bridge(
            &with_request_extension,
            &all_capabilities,
            ProtocolTransformRejection::UnknownRequestExtensions,
        )?;

        let mut with_message_extension = request();
        with_message_extension.messages[0].extensions = extension()?;
        assert_rejection_for_bridge(
            &with_message_extension,
            &all_capabilities,
            ProtocolTransformRejection::UnknownMessageExtensions,
        )?;

        let mut with_content_extension = request();
        with_content_extension.messages[0].content = vec![MessageContent::Text(TextContent {
            text: "private prompt".to_owned(),
            extensions: extension()?,
        })];
        assert_rejection_for_bridge(
            &with_content_extension,
            &all_capabilities,
            ProtocolTransformRejection::UnknownContentExtensions,
        )?;

        let mut with_opaque_content = request();
        with_opaque_content.messages[0].content = vec![MessageContent::Opaque(OpaqueContent::new(
            RawJson::from_json_string(r#"{"type":"private_content"}"#.to_owned())?,
        ))];
        assert_rejection_for_bridge(
            &with_opaque_content,
            &all_capabilities,
            ProtocolTransformRejection::OpaqueContent,
        )?;

        let mut with_tool_extension = request();
        with_tool_extension.tools.push(ToolDefinition {
            name: "private-tool".to_owned(),
            description: None,
            input_schema: RawJson::from_json_string("{}".to_owned())?,
            extensions: extension()?,
        });
        assert_rejection_for_bridge(
            &with_tool_extension,
            &all_capabilities,
            ProtocolTransformRejection::UnknownToolDefinitionExtensions,
        )?;
        Ok(())
    }

    #[test]
    fn bridge_rejects_unrepresentable_tool_cache_thinking_and_roles()
    -> Result<(), Box<dyn std::error::Error>> {
        let all_capabilities = all_capabilities()?;

        let mut with_historical_tool_call =
            with_output_limit(request(), ProtocolFormat::OpenAiResponses, 64)?;
        with_historical_tool_call.messages[0].content = vec![MessageContent::ToolCall(ToolCall {
            id: "private-call".to_owned(),
            name: "private-tool".to_owned(),
            arguments: RawJson::from_json_string("{}".to_owned())?,
            extensions: RawExtensions::default(),
        })];
        assert_rejection_for_bridge(
            &with_historical_tool_call,
            &all_capabilities,
            ProtocolTransformRejection::ToolHistoryUnsupported,
        )?;

        let mut with_historical_tool_result =
            with_output_limit(request(), ProtocolFormat::OpenAiResponses, 64)?;
        with_historical_tool_result.messages[0].content =
            vec![MessageContent::ToolResult(ToolResult {
                call_id: "private-call".to_owned(),
                output: RawJson::from_json_string(r#"{"private":"result"}"#.to_owned())?,
                is_error: true,
                extensions: RawExtensions::default(),
            })];
        assert_rejection_for_bridge(
            &with_historical_tool_result,
            &all_capabilities,
            ProtocolTransformRejection::ToolHistoryUnsupported,
        )?;

        let mut with_thinking = with_output_limit(request(), ProtocolFormat::OpenAiResponses, 64)?;
        with_thinking.thinking = Some(Thinking {
            effort: ThinkingEffort::try_new("private-thinking")?,
            extensions: RawExtensions::default(),
        });
        assert_rejection_for_bridge(
            &with_thinking,
            &all_capabilities,
            ProtocolTransformRejection::ThinkingUnsupported,
        )?;

        let mut with_cache_control =
            with_output_limit(request(), ProtocolFormat::OpenAiResponses, 64)?;
        with_cache_control.prompt_cache_key = Some("private-cache-key".to_owned());
        assert_rejection_for_bridge(
            &with_cache_control,
            &all_capabilities,
            ProtocolTransformRejection::CacheControlUnsupported,
        )?;

        let mut with_incompatible_role =
            with_output_limit(request(), ProtocolFormat::OpenAiResponses, 64)?;
        with_incompatible_role
            .messages
            .insert(0, text_message("user", "first"));
        with_incompatible_role.messages[1].role = MessageRole("developer".to_owned());
        assert_rejection_for_bridge(
            &with_incompatible_role,
            &all_capabilities,
            ProtocolTransformRejection::IncompatibleRole,
        )?;
        Ok(())
    }

    #[test]
    fn requires_declared_target_capabilities_before_admission()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut tool_request = request();
        tool_request.tools.push(ToolDefinition {
            name: "private-tool".to_owned(),
            description: None,
            input_schema: RawJson::from_json_string("{}".to_owned())?,
            extensions: RawExtensions::default(),
        });
        let no_capabilities = CapabilitySet::empty();
        let tools_only = capabilities([SemanticCapability::Tools])?;
        let tool_schema =
            capabilities([SemanticCapability::Tools, SemanticCapability::JsonSchema])?;
        let all_capabilities = all_capabilities()?;

        assert_rejected(
            analyze_protocol_transform(input(
                &tool_request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::OpenAiResponses,
                SnapshotTransformMode::Canonical,
                NativePayloadAvailability::Unavailable,
                &no_capabilities,
            )),
            ProtocolTransformRejection::ToolsUnsupported,
        );
        assert_rejected(
            analyze_protocol_transform(input(
                &tool_request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::OpenAiResponses,
                SnapshotTransformMode::Canonical,
                NativePayloadAvailability::Unavailable,
                &tools_only,
            )),
            ProtocolTransformRejection::JsonSchemaUnsupported,
        );

        let schema_only_request = request();
        let mut explicit_schema_input = input(
            &schema_only_request,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::OpenAiResponses,
            SnapshotTransformMode::Canonical,
            NativePayloadAvailability::Unavailable,
            &tools_only,
        );
        explicit_schema_input.requires_json_schema = true;
        assert_rejected(
            analyze_protocol_transform(explicit_schema_input),
            ProtocolTransformRejection::JsonSchemaUnsupported,
        );

        let mut streaming_input = input(
            &tool_request,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::OpenAiResponses,
            SnapshotTransformMode::Canonical,
            NativePayloadAvailability::Unavailable,
            &tool_schema,
        );
        streaming_input.streaming = true;
        assert_rejected(
            analyze_protocol_transform(streaming_input),
            ProtocolTransformRejection::StreamingUnsupported,
        );

        let mut parallel_input = input(
            &tool_request,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::OpenAiResponses,
            SnapshotTransformMode::Canonical,
            NativePayloadAvailability::Unavailable,
            &tool_schema,
        );
        parallel_input.requires_parallel_tools = true;
        assert_rejected(
            analyze_protocol_transform(parallel_input),
            ProtocolTransformRejection::ParallelToolsUnsupported,
        );

        assert_eq!(
            analyze_protocol_transform(input(
                &tool_request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::OpenAiResponses,
                SnapshotTransformMode::Canonical,
                NativePayloadAvailability::Unavailable,
                &all_capabilities,
            )),
            ProtocolTransformAdmission::Approved
        );
        Ok(())
    }

    #[test]
    fn a_clean_cross_protocol_bridge_is_admitted() -> Result<(), Box<dyn std::error::Error>> {
        let request = with_output_limit(request(), ProtocolFormat::OpenAiResponses, 64)?;
        let no_capabilities = CapabilitySet::empty();

        assert_eq!(
            analyze_protocol_transform(input(
                &request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::AnthropicMessages,
                SnapshotTransformMode::LosslessBridge,
                NativePayloadAvailability::Unavailable,
                &no_capabilities,
            )),
            ProtocolTransformAdmission::Approved
        );
        Ok(())
    }

    #[test]
    fn diagnostics_redact_client_values() {
        let request = request();
        let no_capabilities = CapabilitySet::empty();
        let input = input(
            &request,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
            SnapshotTransformMode::LosslessBridge,
            NativePayloadAvailability::Unavailable,
            &no_capabilities,
        );
        let diagnostic = format!("{input:?}");

        assert!(!diagnostic.contains("private-model"));
        assert!(!diagnostic.contains("private prompt"));
        assert!(diagnostic.contains("ProtocolTransformInput"));
    }

    fn all_capabilities() -> Result<CapabilitySet, Box<dyn std::error::Error>> {
        capabilities([
            SemanticCapability::Tools,
            SemanticCapability::JsonSchema,
            SemanticCapability::Reasoning,
            SemanticCapability::Streaming,
            SemanticCapability::ParallelTools,
        ])
    }

    fn assert_rejection_for_bridge(
        request: &CanonicalRequest,
        capabilities: &CapabilitySet,
        expected: ProtocolTransformRejection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut request = request.clone();
        if request.extensions.is_empty() {
            request.extensions.try_insert(
                super::RESPONSES_OUTPUT_LIMIT,
                RawJson::from_json_string("64".to_owned())?,
            )?;
        }
        assert_rejected(
            analyze_protocol_transform(input(
                &request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::AnthropicMessages,
                SnapshotTransformMode::LosslessBridge,
                NativePayloadAvailability::Unavailable,
                capabilities,
            )),
            expected,
        );
        Ok(())
    }

    fn canonical_projection(
        projection: Result<ProjectedProtocolRequest, ProtocolTransformRejection>,
    ) -> Result<CanonicalRequest, Box<dyn std::error::Error>> {
        match projection? {
            ProjectedProtocolRequest::Canonical(request) => Ok(request),
            ProjectedProtocolRequest::NativeExact => {
                Err(std::io::Error::other("typed mode returned native projection").into())
            }
        }
    }

    #[test]
    fn all_nine_protocol_pairs_prepare_typed_requests() -> Result<(), Box<dyn std::error::Error>> {
        let protocols = [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
        ];
        let no_capabilities = CapabilitySet::empty();

        for source in protocols {
            let request = with_output_limit(request(), source, 64)?;
            for target in protocols {
                let mode = if source == target {
                    SnapshotTransformMode::Canonical
                } else {
                    SnapshotTransformMode::LosslessBridge
                };
                let projected = canonical_projection(project_protocol_request(input(
                    &request,
                    source,
                    target,
                    mode,
                    NativePayloadAvailability::Unavailable,
                    &no_capabilities,
                )))?;
                assert_eq!(projected.messages, request.messages);
                assert!(
                    projected
                        .extensions
                        .get(super::output_limit_name(target))
                        .is_some()
                );
                assert_eq!(projected.extensions.iter().len(), 1);
            }
        }
        Ok(())
    }

    #[test]
    fn all_three_native_pairs_keep_exact_payload_ownership()
    -> Result<(), Box<dyn std::error::Error>> {
        let no_capabilities = CapabilitySet::empty();
        for protocol in [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
        ] {
            let request = request();
            assert_eq!(
                project_protocol_request(input(
                    &request,
                    protocol,
                    protocol,
                    SnapshotTransformMode::Passthrough,
                    NativePayloadAvailability::Exact,
                    &no_capabilities,
                ))?,
                ProjectedProtocolRequest::NativeExact
            );
        }
        Ok(())
    }

    #[test]
    fn ordered_tool_history_is_preserved_when_every_target_can_represent_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let all_capabilities = all_capabilities()?;
        let mut request = request();
        request.messages = vec![
            text_message("user", "call the tool"),
            CanonicalMessage {
                role: MessageRole("assistant".to_owned()),
                content: vec![MessageContent::ToolCall(ToolCall {
                    id: "private-call".to_owned(),
                    name: "private-tool".to_owned(),
                    arguments: RawJson::from_json_string(r#"{"value":1}"#.to_owned())?,
                    extensions: RawExtensions::default(),
                })],
                extensions: RawExtensions::default(),
            },
            CanonicalMessage {
                role: MessageRole("tool".to_owned()),
                content: vec![MessageContent::ToolResult(ToolResult {
                    call_id: "private-call".to_owned(),
                    output: RawJson::from_json_string(r#""done""#.to_owned())?,
                    is_error: false,
                    extensions: RawExtensions::default(),
                })],
                extensions: RawExtensions::default(),
            },
        ];
        request.tools.push(ToolDefinition {
            name: "private-tool".to_owned(),
            description: None,
            input_schema: RawJson::from_json_string(r#"{"type":"object"}"#.to_owned())?,
            extensions: RawExtensions::default(),
        });
        let request = with_output_limit(request, ProtocolFormat::OpenAiResponses, 64)?;

        for target in [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
        ] {
            let projected = canonical_projection(project_protocol_request(input(
                &request,
                ProtocolFormat::OpenAiResponses,
                target,
                if target == ProtocolFormat::OpenAiResponses {
                    SnapshotTransformMode::Canonical
                } else {
                    SnapshotTransformMode::LosslessBridge
                },
                NativePayloadAvailability::Unavailable,
                &all_capabilities,
            )))?;
            assert_eq!(projected.messages, request.messages);
            assert_eq!(projected.tools, request.tools);
        }
        Ok(())
    }

    #[test]
    fn reasoning_levels_follow_the_pinned_legacy_budget_table()
    -> Result<(), Box<dyn std::error::Error>> {
        let reasoning = capabilities([SemanticCapability::Reasoning])?;
        let mut responses = with_output_limit(request(), ProtocolFormat::OpenAiResponses, 64)?;
        responses.thinking = Some(Thinking {
            effort: ThinkingEffort::try_new("high")?,
            extensions: RawExtensions::default(),
        });
        let messages = canonical_projection(project_protocol_request(input(
            &responses,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
            SnapshotTransformMode::LosslessBridge,
            NativePayloadAvailability::Unavailable,
            &reasoning,
        )))?;
        let thinking = messages
            .thinking
            .as_ref()
            .ok_or_else(|| std::io::Error::other("thinking was not projected"))?;
        assert_eq!(thinking.effort.as_str(), "enabled");
        assert_eq!(
            thinking
                .extensions
                .get(super::ANTHROPIC_THINKING_BUDGET)
                .map(RawJson::get),
            Some("24576")
        );

        let round_trip = canonical_projection(project_protocol_request(input(
            &messages,
            ProtocolFormat::AnthropicMessages,
            ProtocolFormat::OpenAiResponses,
            SnapshotTransformMode::LosslessBridge,
            NativePayloadAvailability::Unavailable,
            &reasoning,
        )))?;
        assert_eq!(
            round_trip
                .thinking
                .as_ref()
                .map(|thinking| thinking.effort.as_str()),
            Some("high")
        );
        Ok(())
    }

    #[test]
    fn typed_projection_rejects_missing_invalid_and_colliding_output_limits()
    -> Result<(), Box<dyn std::error::Error>> {
        let no_capabilities = CapabilitySet::empty();
        let plain = request();
        assert_rejected(
            analyze_protocol_transform(input(
                &plain,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::AnthropicMessages,
                SnapshotTransformMode::LosslessBridge,
                NativePayloadAvailability::Unavailable,
                &no_capabilities,
            )),
            ProtocolTransformRejection::OutputLimitMissing,
        );

        let invalid = with_output_limit(request(), ProtocolFormat::OpenAiResponses, 0)?;
        assert_rejected(
            analyze_protocol_transform(input(
                &invalid,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::OpenAiChatCompletions,
                SnapshotTransformMode::LosslessBridge,
                NativePayloadAvailability::Unavailable,
                &no_capabilities,
            )),
            ProtocolTransformRejection::OutputLimitInvalid,
        );

        let mut collision = with_output_limit(request(), ProtocolFormat::OpenAiResponses, 64)?;
        collision.extensions.try_insert(
            super::MESSAGES_OUTPUT_LIMIT,
            RawJson::from_json_string("64".to_owned())?,
        )?;
        assert_rejected(
            analyze_protocol_transform(input(
                &collision,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::OpenAiChatCompletions,
                SnapshotTransformMode::LosslessBridge,
                NativePayloadAvailability::Unavailable,
                &no_capabilities,
            )),
            ProtocolTransformRejection::OutputLimitCollision,
        );
        Ok(())
    }

    #[test]
    fn every_target_builder_accepts_the_router_approved_tool_projection()
    -> Result<(), Box<dyn std::error::Error>> {
        let all_capabilities = all_capabilities()?;
        let request: CanonicalRequest = serde_json::from_str(include_str!(
            "../../../tests/fixtures/router/p12-08d1-typed-tool-history.json"
        ))?;

        for target in [
            ProtocolFormat::OpenAiChatCompletions,
            ProtocolFormat::OpenAiResponses,
            ProtocolFormat::AnthropicMessages,
        ] {
            let projected = canonical_projection(project_protocol_request(input(
                &request,
                ProtocolFormat::OpenAiResponses,
                target,
                if target == ProtocolFormat::OpenAiResponses {
                    SnapshotTransformMode::Canonical
                } else {
                    SnapshotTransformMode::LosslessBridge
                },
                NativePayloadAvailability::Unavailable,
                &all_capabilities,
            )))?;

            match target {
                ProtocolFormat::OpenAiChatCompletions => {
                    OpenAiChatCompletionsRequestBuilder::build(
                        &OpenAiChatCompletionsEndpoint::try_new(
                            "https://relay.example",
                            "/v1/chat/completions",
                        )?,
                        &OpenAiChatCompletionsApiKey::try_new("secret")?,
                        "upstream-model",
                        &projected,
                        ChatResponseMode::NonStreaming,
                    )?;
                }
                ProtocolFormat::OpenAiResponses => {
                    OpenAiResponsesRequestBuilder::build(
                        &OpenAiResponsesEndpoint::try_new(
                            "https://relay.example",
                            "/v1/responses",
                        )?,
                        &OpenAiResponsesApiKey::try_new("secret")?,
                        "upstream-model",
                        &projected,
                        ResponsesResponseMode::NonStreaming,
                    )?;
                }
                ProtocolFormat::AnthropicMessages => {
                    AnthropicMessagesRequestBuilder::build(
                        &AnthropicMessagesEndpoint::try_new(
                            "https://relay.example",
                            "/v1/messages",
                        )?,
                        &AnthropicMessagesApiKey::try_new("secret")?,
                        "upstream-model",
                        &projected,
                        AnthropicResponseMode::NonStreaming,
                    )?;
                }
            }
        }
        Ok(())
    }

    proptest! {
        #[test]
        fn every_positive_output_limit_maps_exactly_across_all_pairs(limit in 1_u64..=u32::MAX.into()) {
            let protocols = [
                ProtocolFormat::OpenAiChatCompletions,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::AnthropicMessages,
            ];
            let no_capabilities = CapabilitySet::empty();
            let expected = limit.to_string();
            for source in protocols {
                let request = with_output_limit(request(), source, limit)
                    .map_err(|error| TestCaseError::fail(error.to_string()))?;
                for target in protocols {
                    let mode = if source == target {
                        SnapshotTransformMode::Canonical
                    } else {
                        SnapshotTransformMode::LosslessBridge
                    };
                    let projection = project_protocol_request(input(
                        &request,
                        source,
                        target,
                        mode,
                        NativePayloadAvailability::Unavailable,
                        &no_capabilities,
                    )).map_err(|error| TestCaseError::fail(error.to_string()))?;
                    let ProjectedProtocolRequest::Canonical(projected) = projection else {
                        return Err(TestCaseError::fail("typed mode returned native projection"));
                    };
                    prop_assert_eq!(
                        projected.extensions.get(super::output_limit_name(target)).map(RawJson::get),
                        Some(expected.as_str())
                    );
                }
            }
        }
    }
}
