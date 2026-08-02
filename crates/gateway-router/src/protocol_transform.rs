//! Fail-closed admission analysis for protocol transformation candidates.
//!
//! A Route may declare a native pass-through, a same-protocol Canonical conversion, or a
//! cross-protocol lossless bridge. This module does not serialize a request, choose an Endpoint,
//! or execute a Provider call. It only determines whether the requested conversion has enough
//! evidence to participate in routing without silently erasing semantics.

use std::fmt;

use gateway_catalog::{CapabilitySet, SemanticCapability};
use gateway_core::{CanonicalRequest, MessageContent};
use gateway_protocol::ApiFormat;

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
    /// Historical Tool calls are not yet covered by the bridge contract.
    HistoricalToolCall,
    /// Historical Tool results are not yet covered by the bridge contract.
    HistoricalToolResult,
    /// Thinking requires the explicit P5-06 mapping contract.
    ThinkingUnsupported,
    /// Prompt-cache controls require the explicit P5-06 mapping contract.
    CacheControlUnsupported,
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
}

/// Analyzes whether one Route candidate can preserve this request's semantics.
///
/// `Passthrough` is intentionally narrow: it accepts only an exact native body to the same
/// protocol, so opaque native fields are still preserved byte-for-byte by the later transport.
/// `Canonical` and `LosslessBridge` reconstruct a body and therefore reject every retained
/// unknown or unsupported semantic rather than guessing a conversion.
#[must_use]
pub fn analyze_protocol_transform(input: ProtocolTransformInput<'_>) -> ProtocolTransformAdmission {
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
        return ProtocolTransformAdmission::Rejected(rejection);
    }

    if input.mode != SnapshotTransformMode::Passthrough
        && let Some(rejection) = canonical_rejection(input.request, input.target)
    {
        return ProtocolTransformAdmission::Rejected(rejection);
    }

    if let Some(rejection) = capability_rejection(input) {
        return ProtocolTransformAdmission::Rejected(rejection);
    }

    ProtocolTransformAdmission::Approved
}

fn canonical_rejection(
    request: &CanonicalRequest,
    target: ProtocolFormat,
) -> Option<ProtocolTransformRejection> {
    if !request.extensions.is_empty() {
        return Some(ProtocolTransformRejection::UnknownRequestExtensions);
    }
    if request.thinking.is_some() {
        return Some(ProtocolTransformRejection::ThinkingUnsupported);
    }
    if request.prompt_cache_key.is_some() || request.prompt_cache_retention.is_some() {
        return Some(ProtocolTransformRejection::CacheControlUnsupported);
    }

    for message in &request.messages {
        if !message.extensions.is_empty() {
            return Some(ProtocolTransformRejection::UnknownMessageExtensions);
        }
        for content in &message.content {
            match content {
                MessageContent::Text(text) if !text.extensions.is_empty() => {
                    return Some(ProtocolTransformRejection::UnknownContentExtensions);
                }
                MessageContent::Opaque(_) => {
                    return Some(ProtocolTransformRejection::OpaqueContent);
                }
                MessageContent::ToolCall(_) => {
                    return Some(ProtocolTransformRejection::HistoricalToolCall);
                }
                MessageContent::ToolResult(_) => {
                    return Some(ProtocolTransformRejection::HistoricalToolResult);
                }
                MessageContent::Text(_) => {}
            }
        }
        if !target_supports_role(target, &message.role.0) {
            return Some(ProtocolTransformRejection::IncompatibleRole);
        }
    }

    for tool in &request.tools {
        if !tool.extensions.is_empty() {
            return Some(ProtocolTransformRejection::UnknownToolDefinitionExtensions);
        }
    }

    None
}

fn target_supports_role(target: ProtocolFormat, role: &str) -> bool {
    match target {
        ProtocolFormat::OpenAiChatCompletions => {
            matches!(role, "system" | "developer" | "user" | "assistant" | "tool")
        }
        ProtocolFormat::OpenAiResponses => {
            matches!(role, "system" | "developer" | "user" | "assistant")
        }
        ProtocolFormat::AnthropicMessages => matches!(role, "system" | "user" | "assistant"),
    }
}

fn capability_rejection(input: ProtocolTransformInput<'_>) -> Option<ProtocolTransformRejection> {
    let capabilities = input.target_capabilities;
    if input.streaming && !capabilities.supports(SemanticCapability::Streaming) {
        return Some(ProtocolTransformRejection::StreamingUnsupported);
    }

    let has_tools = !input.request.tools.is_empty();
    if (has_tools || input.requires_parallel_tools)
        && !capabilities.supports(SemanticCapability::Tools)
    {
        return Some(ProtocolTransformRejection::ToolsUnsupported);
    }
    if (has_tools || input.requires_json_schema)
        && !capabilities.supports(SemanticCapability::JsonSchema)
    {
        return Some(ProtocolTransformRejection::JsonSchemaUnsupported);
    }
    if input.requires_parallel_tools && !capabilities.supports(SemanticCapability::ParallelTools) {
        return Some(ProtocolTransformRejection::ParallelToolsUnsupported);
    }

    None
}

#[cfg(test)]
mod tests {
    use gateway_catalog::{CapabilitySet, SemanticCapability};
    use gateway_core::{
        CanonicalMessage, CanonicalRequest, MessageContent, MessageRole, OpaqueContent,
        RawExtensions, RawJson, TextContent, Thinking, ThinkingEffort, ToolCall, ToolDefinition,
        ToolResult,
    };
    use gateway_protocol::ApiFormat;

    use super::{
        NativePayloadAvailability, ProtocolFormat, ProtocolTransformAdmission,
        ProtocolTransformInput, ProtocolTransformRejection, analyze_protocol_transform,
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
    fn canonical_and_bridge_reject_extensions_and_opaque_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let all_capabilities = all_capabilities()?;

        let mut with_request_extension = request();
        with_request_extension.extensions = extension()?;
        assert_rejection_for_bridge(
            &with_request_extension,
            &all_capabilities,
            ProtocolTransformRejection::UnknownRequestExtensions,
        );

        let mut with_message_extension = request();
        with_message_extension.messages[0].extensions = extension()?;
        assert_rejection_for_bridge(
            &with_message_extension,
            &all_capabilities,
            ProtocolTransformRejection::UnknownMessageExtensions,
        );

        let mut with_content_extension = request();
        with_content_extension.messages[0].content = vec![MessageContent::Text(TextContent {
            text: "private prompt".to_owned(),
            extensions: extension()?,
        })];
        assert_rejection_for_bridge(
            &with_content_extension,
            &all_capabilities,
            ProtocolTransformRejection::UnknownContentExtensions,
        );

        let mut with_opaque_content = request();
        with_opaque_content.messages[0].content = vec![MessageContent::Opaque(OpaqueContent::new(
            RawJson::from_json_string(r#"{"type":"private_content"}"#.to_owned())?,
        ))];
        assert_rejection_for_bridge(
            &with_opaque_content,
            &all_capabilities,
            ProtocolTransformRejection::OpaqueContent,
        );

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
        );
        Ok(())
    }

    #[test]
    fn canonical_and_bridge_reject_historical_and_deferred_semantics()
    -> Result<(), Box<dyn std::error::Error>> {
        let all_capabilities = all_capabilities()?;

        let mut with_historical_tool_call = request();
        with_historical_tool_call.messages[0].content = vec![MessageContent::ToolCall(ToolCall {
            id: "private-call".to_owned(),
            name: "private-tool".to_owned(),
            arguments: RawJson::from_json_string("{}".to_owned())?,
            extensions: RawExtensions::default(),
        })];
        assert_rejection_for_bridge(
            &with_historical_tool_call,
            &all_capabilities,
            ProtocolTransformRejection::HistoricalToolCall,
        );

        let mut with_historical_tool_result = request();
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
            ProtocolTransformRejection::HistoricalToolResult,
        );

        let mut with_thinking = request();
        with_thinking.thinking = Some(Thinking {
            effort: ThinkingEffort::try_new("private-thinking")?,
            extensions: RawExtensions::default(),
        });
        assert_rejection_for_bridge(
            &with_thinking,
            &all_capabilities,
            ProtocolTransformRejection::ThinkingUnsupported,
        );

        let mut with_cache_control = request();
        with_cache_control.prompt_cache_key = Some("private-cache-key".to_owned());
        assert_rejection_for_bridge(
            &with_cache_control,
            &all_capabilities,
            ProtocolTransformRejection::CacheControlUnsupported,
        );

        let mut with_incompatible_role = request();
        with_incompatible_role.messages[0].role = MessageRole("developer".to_owned());
        assert_rejection_for_bridge(
            &with_incompatible_role,
            &all_capabilities,
            ProtocolTransformRejection::IncompatibleRole,
        );
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
    fn a_clean_cross_protocol_bridge_is_admitted() {
        let request = request();
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
            SemanticCapability::Streaming,
            SemanticCapability::ParallelTools,
        ])
    }

    fn assert_rejection_for_bridge(
        request: &CanonicalRequest,
        capabilities: &CapabilitySet,
        expected: ProtocolTransformRejection,
    ) {
        assert_rejected(
            analyze_protocol_transform(input(
                request,
                ProtocolFormat::OpenAiResponses,
                ProtocolFormat::AnthropicMessages,
                SnapshotTransformMode::LosslessBridge,
                NativePayloadAvailability::Unavailable,
                capabilities,
            )),
            expected,
        );
    }
}
