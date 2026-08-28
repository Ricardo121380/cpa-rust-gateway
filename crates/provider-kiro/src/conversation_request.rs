//! Pure conversion from a Canonical request into Kiro's conversation request envelope.
//!
//! The converter deliberately has no ambient machine, credential, or network dependencies.
//! Callers supply the Kiro conversation id and the environment context that Kiro requires on the
//! wire.  Profile-ARN injection remains the narrow P7-03 responsibility, while EventStream,
//! historical Tool execution, Thinking, and transport remain later P7 work.

use std::{collections::BTreeSet, error::Error, fmt};

use gateway_core::{
    CanonicalMessage, CanonicalRequest, MessageContent, RawJson, ToolCall, ToolDefinition,
    ToolResult,
};
use serde_json::{Map, Value};

use crate::endpoint_policy::KiroEndpointPolicy;

const MAX_CONVERSATION_ID_BYTES: usize = 512;
const MAX_MODEL_ID_BYTES: usize = 512;
const MAX_OPERATING_SYSTEM_BYTES: usize = 128;
const MAX_WORKING_DIRECTORY_BYTES: usize = 4_096;
const MAX_TOOL_CALL_ID_BYTES: usize = 512;
const MAX_TOOL_NAME_BYTES: usize = 256;
/// Kiro's required conversation trigger.
///
/// `MANUAL` is the only correct value for a gateway: every request originates from an explicit
/// client call, never from an editor's automatic trigger. Omitting the field entirely makes the CLI
/// endpoint reject the request with `REQUEST_BODY_INVALID`.
const CHAT_TRIGGER_TYPE: &str = "MANUAL";

/// A caller-owned Kiro conversation identifier, retained only for request construction.
#[derive(Clone, Eq, PartialEq)]
pub struct KiroConversationId(String);

impl KiroConversationId {
    /// Creates a bounded non-empty conversation identifier.
    ///
    /// # Errors
    ///
    /// Returns a safe classification when the supplied identifier is absent or oversized.
    pub fn try_new(value: impl Into<String>) -> Result<Self, KiroConversationRequestError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_CONVERSATION_ID_BYTES {
            return Err(KiroConversationRequestError::InvalidConversationId);
        }
        Ok(Self(value))
    }

    /// Returns the identifier only for immediate Kiro request construction.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for KiroConversationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KiroConversationId(<redacted>)")
    }
}

/// Explicit, request-scoped machine context that Kiro places in `userInputMessageContext`.
///
/// This is supplied by an outer runtime.  The converter never reads a working directory, OS
/// metadata, environment variable, or other ambient host state.
#[derive(Clone, Eq, PartialEq)]
pub struct KiroEnvironmentState {
    operating_system: String,
    current_working_directory: String,
}

impl KiroEnvironmentState {
    /// Creates bounded non-empty Kiro environment metadata.
    ///
    /// # Errors
    ///
    /// Returns a safe classification when either required Kiro context field is invalid.
    pub fn try_new(
        operating_system: impl Into<String>,
        current_working_directory: impl Into<String>,
    ) -> Result<Self, KiroConversationRequestError> {
        let operating_system = operating_system.into();
        let current_working_directory = current_working_directory.into();
        if operating_system.is_empty() || operating_system.len() > MAX_OPERATING_SYSTEM_BYTES {
            return Err(KiroConversationRequestError::InvalidEnvironmentState);
        }
        if current_working_directory.is_empty()
            || current_working_directory.len() > MAX_WORKING_DIRECTORY_BYTES
        {
            return Err(KiroConversationRequestError::InvalidEnvironmentState);
        }
        Ok(Self {
            operating_system,
            current_working_directory,
        })
    }
}

impl fmt::Debug for KiroEnvironmentState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KiroEnvironmentState(<redacted>)")
    }
}

/// Explicit non-secret context for one Kiro conversation conversion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KiroConversationContext {
    conversation_id: KiroConversationId,
    environment: KiroEnvironmentState,
}

impl KiroConversationContext {
    /// Combines the caller-owned conversation identity and explicit environment projection.
    #[must_use]
    pub fn new(conversation_id: KiroConversationId, environment: KiroEnvironmentState) -> Self {
        Self {
            conversation_id,
            environment,
        }
    }
}

/// Opaque Kiro-ready JSON body with redacted diagnostics.
#[derive(Clone, Eq, PartialEq)]
pub struct KiroConversationRequest {
    body: Value,
}

impl KiroConversationRequest {
    /// Returns the generated request body for a later narrow composition step.
    #[must_use]
    pub const fn body(&self) -> &Value {
        &self.body
    }

    /// Transfers the body to a later narrow composition step, such as P7-03 profile injection.
    #[must_use]
    pub fn into_body(self) -> Value {
        self.body
    }
}

impl fmt::Debug for KiroConversationRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroConversationRequest")
            .field("body", &"<redacted>")
            .finish()
    }
}

/// Stateless, fail-closed Kiro conversation-envelope converter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KiroConversationRequestBuilder;

impl KiroConversationRequestBuilder {
    /// Converts one narrow Canonical text conversation into Kiro's request envelope.
    ///
    /// The final Canonical message must be a user message. Earlier user/assistant messages become
    /// ordered Kiro history, including paired historical Tool calls and results. Declared Tools
    /// are placed only on the current user context. Thinking follows the explicit IDE/CLI policy.
    /// Opaque content, cache controls, and unscoped Canonical extensions still fail closed.
    ///
    /// # Errors
    ///
    /// Returns a value-only classification for unsupported Canonical semantics or an invalid
    /// selected Kiro model. No request values are included in the error.
    pub fn build(
        policy: &KiroEndpointPolicy,
        context: &KiroConversationContext,
        selected_model: &str,
        request: &CanonicalRequest,
    ) -> Result<KiroConversationRequest, KiroConversationRequestError> {
        if selected_model.is_empty() || selected_model.len() > MAX_MODEL_ID_BYTES {
            return Err(KiroConversationRequestError::InvalidSelectedModel);
        }
        reject_unsupported_request_fields(request)?;

        let (current, history) = split_current_message(&request.messages)?;
        let mut historical_tools = HistoricalToolState::default();
        let history = history
            .iter()
            .map(|message| {
                encode_history_message(message, selected_model, policy, &mut historical_tools)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tools = encode_tools(&request.tools)?;
        let current = encode_user_message(current, &mut historical_tools)?;

        let mut user_input_context = Map::new();
        user_input_context.insert(
            "envState".to_owned(),
            Value::Object(map_from([
                (
                    "operatingSystem".to_owned(),
                    Value::String(context.environment.operating_system.clone()),
                ),
                (
                    "currentWorkingDirectory".to_owned(),
                    Value::String(context.environment.current_working_directory.clone()),
                ),
            ])),
        );
        if !tools.is_empty() {
            user_input_context.insert("tools".to_owned(), Value::Array(tools));
        }
        if !current.tool_results.is_empty() {
            user_input_context.insert("toolResults".to_owned(), Value::Array(current.tool_results));
        }
        if let Some(thinking) = &request.thinking {
            if !thinking.extensions.is_empty() {
                return Err(KiroConversationRequestError::UnsupportedCanonicalField);
            }
            match policy.thinking_placement() {
                crate::endpoint_policy::KiroThinkingPlacement::IdeThinkingWrapper => {
                    user_input_context.insert(
                        "additionalModelRequestFields".to_owned(),
                        Value::Object(map_from([(
                            "thinking".to_owned(),
                            Value::Object(map_from([(
                                "effort".to_owned(),
                                Value::String(thinking.effort.as_str().to_owned()),
                            )])),
                        )])),
                    );
                }
                crate::endpoint_policy::KiroThinkingPlacement::CliOutputConfigEffort => {
                    user_input_context.insert(
                        "outputConfig".to_owned(),
                        Value::Object(map_from([(
                            "effort".to_owned(),
                            Value::String(thinking.effort.as_str().to_owned()),
                        )])),
                    );
                }
            }
        }

        let mut current_user = Map::new();
        if let Some(content) = current.content {
            current_user.insert("content".to_owned(), Value::String(content));
        }
        current_user.insert(
            "modelId".to_owned(),
            Value::String(selected_model.to_owned()),
        );
        current_user.insert(
            "origin".to_owned(),
            Value::String(policy.origin().as_header_value().to_owned()),
        );
        current_user.insert(
            "userInputMessageContext".to_owned(),
            Value::Object(user_input_context),
        );

        Ok(KiroConversationRequest {
            body: Value::Object(map_from([(
                "conversationState".to_owned(),
                Value::Object(conversation_state(context, current_user, history)),
            )])),
        })
    }
}

/// Assembles the `conversationState` envelope around one already-encoded current message.
///
/// Split out from the builder so the builder's own length stays within the crate's limit; it is also
/// the single place the required trigger is emitted.
fn conversation_state(
    context: &KiroConversationContext,
    current_user: Map<String, Value>,
    history: Vec<Value>,
) -> Map<String, Value> {
    let mut conversation_state = Map::new();
    // Kiro requires `chatTriggerType` on every conversation: without it the CLI endpoint returns
    // 400 `ValidationException` with `reason: REQUEST_BODY_INVALID`, and adding it is the single
    // change that makes an otherwise identical request valid (measured against the live endpoint).
    // `MANUAL` is the correct value for a gateway: every request this product serves originates from
    // an explicit client call, never from an editor's automatic trigger.
    conversation_state.insert(
        "chatTriggerType".to_owned(),
        Value::String(CHAT_TRIGGER_TYPE.to_owned()),
    );
    conversation_state.insert(
        "conversationId".to_owned(),
        Value::String(context.conversation_id.as_str().to_owned()),
    );
    conversation_state.insert(
        "currentMessage".to_owned(),
        Value::Object(map_from([(
            "userInputMessage".to_owned(),
            Value::Object(current_user),
        )])),
    );
    if !history.is_empty() {
        conversation_state.insert("history".to_owned(), Value::Array(history));
    }
    conversation_state
}

fn reject_unsupported_request_fields(
    request: &CanonicalRequest,
) -> Result<(), KiroConversationRequestError> {
    if !request.extensions.is_empty()
        || request.prompt_cache_key.is_some()
        || request.prompt_cache_retention.is_some()
    {
        return Err(KiroConversationRequestError::UnsupportedCanonicalField);
    }
    Ok(())
}

fn split_current_message(
    messages: &[CanonicalMessage],
) -> Result<(&CanonicalMessage, &[CanonicalMessage]), KiroConversationRequestError> {
    let Some((current, history)) = messages.split_last() else {
        return Err(KiroConversationRequestError::MissingCurrentUserMessage);
    };
    if current.role.0 != "user" {
        return Err(KiroConversationRequestError::CurrentMessageMustBeUser);
    }
    Ok((current, history))
}

fn encode_history_message(
    message: &CanonicalMessage,
    selected_model: &str,
    policy: &KiroEndpointPolicy,
    historical_tools: &mut HistoricalToolState,
) -> Result<Value, KiroConversationRequestError> {
    match message.role.0.as_str() {
        "user" => {
            let user = encode_user_message(message, historical_tools)?;
            let mut user_input = Map::new();
            if let Some(content) = user.content {
                user_input.insert("content".to_owned(), Value::String(content));
            }
            user_input.insert(
                "modelId".to_owned(),
                Value::String(selected_model.to_owned()),
            );
            user_input.insert(
                "origin".to_owned(),
                Value::String(policy.origin().as_header_value().to_owned()),
            );
            if !user.tool_results.is_empty() {
                user_input.insert(
                    "userInputMessageContext".to_owned(),
                    Value::Object(map_from([(
                        "toolResults".to_owned(),
                        Value::Array(user.tool_results),
                    )])),
                );
            }
            Ok(Value::Object(map_from([(
                "userInputMessage".to_owned(),
                Value::Object(user_input),
            )])))
        }
        "assistant" => {
            let assistant = encode_assistant_message(message, historical_tools)?;
            let mut response = Map::new();
            if let Some(content) = assistant.content {
                response.insert("content".to_owned(), Value::String(content));
            }
            if !assistant.tool_uses.is_empty() {
                response.insert("toolUses".to_owned(), Value::Array(assistant.tool_uses));
            }
            Ok(Value::Object(map_from([(
                "assistantResponseMessage".to_owned(),
                Value::Object(response),
            )])))
        }
        _ => Err(KiroConversationRequestError::UnsupportedMessageRole),
    }
}

struct EncodedUserMessage {
    content: Option<String>,
    tool_results: Vec<Value>,
}

struct EncodedAssistantMessage {
    content: Option<String>,
    tool_uses: Vec<Value>,
}

#[derive(Default)]
struct HistoricalToolState {
    declared_ids: BTreeSet<String>,
    resolved_ids: BTreeSet<String>,
}

fn encode_user_message(
    message: &CanonicalMessage,
    historical_tools: &mut HistoricalToolState,
) -> Result<EncodedUserMessage, KiroConversationRequestError> {
    if !message.extensions.is_empty() || message.content.is_empty() {
        return Err(KiroConversationRequestError::UnsupportedMessageContent);
    }

    let mut content = String::new();
    let mut tool_results = Vec::new();
    for part in &message.content {
        match part {
            MessageContent::Text(text) if text.extensions.is_empty() => {
                content.push_str(&text.text);
            }
            MessageContent::ToolResult(result) => {
                tool_results.push(encode_tool_result(result, historical_tools)?);
            }
            MessageContent::Text(_) | MessageContent::Opaque(_) | MessageContent::ToolCall(_) => {
                return Err(KiroConversationRequestError::UnsupportedMessageContent);
            }
        }
    }
    if content.is_empty() && tool_results.is_empty() {
        return Err(KiroConversationRequestError::UnsupportedMessageContent);
    }
    Ok(EncodedUserMessage {
        content: (!content.is_empty()).then_some(content),
        tool_results,
    })
}

fn encode_assistant_message(
    message: &CanonicalMessage,
    historical_tools: &mut HistoricalToolState,
) -> Result<EncodedAssistantMessage, KiroConversationRequestError> {
    if !message.extensions.is_empty() || message.content.is_empty() {
        return Err(KiroConversationRequestError::UnsupportedMessageContent);
    }

    let mut content = String::new();
    let mut tool_uses = Vec::new();
    for part in &message.content {
        match part {
            MessageContent::Text(text) if text.extensions.is_empty() => {
                content.push_str(&text.text);
            }
            MessageContent::ToolCall(call) => {
                tool_uses.push(encode_tool_call(call, historical_tools)?);
            }
            MessageContent::Text(_) | MessageContent::Opaque(_) | MessageContent::ToolResult(_) => {
                return Err(KiroConversationRequestError::UnsupportedMessageContent);
            }
        }
    }
    if content.is_empty() && tool_uses.is_empty() {
        return Err(KiroConversationRequestError::UnsupportedMessageContent);
    }
    Ok(EncodedAssistantMessage {
        content: (!content.is_empty()).then_some(content),
        tool_uses,
    })
}

fn encode_tool_call(
    call: &ToolCall,
    historical_tools: &mut HistoricalToolState,
) -> Result<Value, KiroConversationRequestError> {
    if call.extensions.is_empty()
        && is_valid_tool_identifier(&call.id, MAX_TOOL_CALL_ID_BYTES)
        && is_valid_tool_identifier(&call.name, MAX_TOOL_NAME_BYTES)
        && historical_tools.declared_ids.insert(call.id.clone())
    {
        let input = parse_json_object(&call.arguments)?;
        return Ok(Value::Object(map_from([
            ("name".to_owned(), Value::String(call.name.clone())),
            ("toolUseId".to_owned(), Value::String(call.id.clone())),
            ("input".to_owned(), input),
        ])));
    }
    Err(KiroConversationRequestError::InvalidHistoricalTool)
}

fn encode_tool_result(
    result: &ToolResult,
    historical_tools: &mut HistoricalToolState,
) -> Result<Value, KiroConversationRequestError> {
    if !result.extensions.is_empty()
        || !is_valid_tool_identifier(&result.call_id, MAX_TOOL_CALL_ID_BYTES)
        || !historical_tools.declared_ids.contains(&result.call_id)
        || !historical_tools.resolved_ids.insert(result.call_id.clone())
    {
        return Err(KiroConversationRequestError::InvalidHistoricalTool);
    }
    let content = serde_json::from_str(result.output.get())
        .map_err(|_| KiroConversationRequestError::InvalidHistoricalTool)?;
    Ok(Value::Object(map_from([
        (
            "toolUseId".to_owned(),
            Value::String(result.call_id.clone()),
        ),
        ("content".to_owned(), content),
        (
            "status".to_owned(),
            Value::String(if result.is_error { "error" } else { "success" }.to_owned()),
        ),
    ])))
}

fn parse_json_object(value: &RawJson) -> Result<Value, KiroConversationRequestError> {
    let value: Value = serde_json::from_str(value.get())
        .map_err(|_| KiroConversationRequestError::InvalidHistoricalTool)?;
    value
        .is_object()
        .then_some(value)
        .ok_or(KiroConversationRequestError::InvalidHistoricalTool)
}

fn is_valid_tool_identifier(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn encode_tools(tools: &[ToolDefinition]) -> Result<Vec<Value>, KiroConversationRequestError> {
    tools.iter().map(encode_tool).collect()
}

fn encode_tool(tool: &ToolDefinition) -> Result<Value, KiroConversationRequestError> {
    if tool.name.is_empty() || !tool.extensions.is_empty() {
        return Err(KiroConversationRequestError::InvalidToolDefinition);
    }
    let input_schema: Value = serde_json::from_str(tool.input_schema.get())
        .map_err(|_| KiroConversationRequestError::InvalidToolDefinition)?;
    if !input_schema.is_object() {
        return Err(KiroConversationRequestError::InvalidToolDefinition);
    }

    Ok(Value::Object(map_from([(
        "toolSpecification".to_owned(),
        Value::Object(map_from([
            ("name".to_owned(), Value::String(tool.name.clone())),
            (
                "description".to_owned(),
                Value::String(tool.description.clone().unwrap_or_default()),
            ),
            (
                "inputSchema".to_owned(),
                Value::Object(map_from([("json".to_owned(), input_schema)])),
            ),
        ])),
    )])))
}

fn map_from(entries: impl IntoIterator<Item = (String, Value)>) -> Map<String, Value> {
    entries.into_iter().collect()
}

/// Value-only failures from Kiro conversation request construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroConversationRequestError {
    /// The caller-supplied conversation identifier was empty or too large.
    InvalidConversationId,
    /// The explicit Kiro environment projection was incomplete or too large.
    InvalidEnvironmentState,
    /// The selected upstream Kiro model was empty or too large.
    InvalidSelectedModel,
    /// Canonical input supplied no final current message.
    MissingCurrentUserMessage,
    /// The final Canonical message was not a user message.
    CurrentMessageMustBeUser,
    /// A Canonical role does not map losslessly to this narrow Kiro conversation form.
    UnsupportedMessageRole,
    /// A Canonical message contains an extension, no text, or unsupported content.
    UnsupportedMessageContent,
    /// A Canonical field belongs to later dedicated Kiro work.
    UnsupportedCanonicalField,
    /// A declared Tool could not map to Kiro's required Tool specification envelope.
    InvalidToolDefinition,
    /// Historical Tool calls/results were malformed, unpaired, or unsafe to map.
    InvalidHistoricalTool,
}

impl fmt::Display for KiroConversationRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConversationId => "Kiro conversation id is invalid",
            Self::InvalidEnvironmentState => "Kiro environment state is invalid",
            Self::InvalidSelectedModel => "Kiro selected model is invalid",
            Self::MissingCurrentUserMessage => "Kiro request needs a current user message",
            Self::CurrentMessageMustBeUser => "Kiro current message must be a user message",
            Self::UnsupportedMessageRole => "Kiro message role is unsupported",
            Self::UnsupportedMessageContent => "Kiro message content is unsupported",
            Self::UnsupportedCanonicalField => "Kiro Canonical field is unsupported",
            Self::InvalidToolDefinition => "Kiro Tool definition is invalid",
            Self::InvalidHistoricalTool => "Kiro historical Tool state is invalid",
        })
    }
}

impl Error for KiroConversationRequestError {}
