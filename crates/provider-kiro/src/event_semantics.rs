//! Kiro `EventStream` payload semantics and Claude Code-compatible Tool mapping.
//!
//! The P7-05 framing decoder owns byte boundaries and CRC validation. This module starts only
//! after a frame is intact: it interprets the small Kiro event vocabulary, keeps Tool arguments
//! bounded until their explicit `stop`, and produces ordered Canonical events. It never opens a
//! socket, chooses a Credential, classifies an HTTP failure, or exposes upstream payload values in
//! diagnostics.

use std::{collections::BTreeMap, error::Error, fmt};

use gateway_core::{
    CanonicalEvent, MessageEnd, MessageRole, MessageStart, RawExtensions, RawJson, ReasoningDelta,
    ResponseEnd, ResponseId, ResponseStart, TextDelta, ToolCallArgumentsDelta, ToolCallEnd,
    ToolCallStart,
};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};

use crate::event_stream::KiroEventStreamFrame;

const MAX_EVENT_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_TOOL_ARGUMENT_BYTES: usize = 1024 * 1024;
const MAX_TOOL_CALL_ID_BYTES: usize = 512;
const MAX_TOOL_NAME_BYTES: usize = 256;

const ENTER_PLAN_MODE: &str = "EnterPlanMode";
const EXIT_PLAN_MODE: &str = "ExitPlanMode";
const ASK_USER_QUESTION: &str = "AskUserQuestion";

/// A Kiro semantic-mapping failure that never includes provider payload values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroEventSemanticError {
    /// A caller attempted to map a non-event AWS `EventStream` frame.
    UnexpectedFrame,
    /// The Kiro event name is not part of this narrow semantic contract.
    UnsupportedEvent,
    /// An event payload was oversized, malformed, duplicated a JSON field, or had an invalid shape.
    InvalidPayload,
    /// A Tool event was malformed, out of order, or used an unsafe identifier.
    InvalidToolState,
    /// One Tool's accumulated argument JSON exceeded the fixed bound.
    ToolArgumentsTooLarge,
    /// A stream ended while a Kiro Tool remained open.
    UnfinishedTool,
    /// The caller attempted an invalid mapper lifecycle transition.
    InvalidLifecycle,
}

impl fmt::Display for KiroEventSemanticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnexpectedFrame => "Kiro EventStream frame is not an event",
            Self::UnsupportedEvent => "Kiro EventStream event is unsupported",
            Self::InvalidPayload => "Kiro EventStream payload is invalid",
            Self::InvalidToolState => "Kiro Tool event state is invalid",
            Self::ToolArgumentsTooLarge => "Kiro Tool arguments exceed the fixed limit",
            Self::UnfinishedTool => "Kiro EventStream ended with an unfinished Tool",
            Self::InvalidLifecycle => "Kiro semantic mapper lifecycle is invalid",
        })
    }
}

impl Error for KiroEventSemanticError {}

/// Stateful semantic mapper for one already-framed Kiro response.
///
/// Call [`Self::start`] once, feed CRC-verified frames through [`Self::push_frame`], and complete
/// the Canonical lifecycle with [`Self::finish`]. The mapper emits a `ToolCallEnd` only after the
/// complete buffered input is a strict JSON object; it never invents braces for a partial Tool.
pub struct KiroEventSemanticMapper {
    response_id: ResponseId,
    started: bool,
    finished: bool,
    open_tools: BTreeMap<String, OpenTool>,
}

impl KiroEventSemanticMapper {
    /// Creates an unstarted mapper for one caller-owned Canonical response identity.
    #[must_use]
    pub fn new(response_id: ResponseId) -> Self {
        Self {
            response_id,
            started: false,
            finished: false,
            open_tools: BTreeMap::new(),
        }
    }

    /// Starts the Canonical response and its assistant message exactly once.
    ///
    /// # Errors
    ///
    /// Returns a value-only error if the mapper was already started or finished.
    pub fn start(&mut self) -> Result<Vec<CanonicalEvent>, KiroEventSemanticError> {
        if self.started || self.finished {
            return Err(KiroEventSemanticError::InvalidLifecycle);
        }
        self.started = true;
        Ok(vec![
            CanonicalEvent::ResponseStart(ResponseStart {
                response_id: self.response_id.clone(),
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::MessageStart(MessageStart {
                role: MessageRole("assistant".to_owned()),
                extensions: RawExtensions::default(),
            }),
        ])
    }

    /// Maps one CRC-verified Kiro frame into zero or more Canonical semantic events.
    ///
    /// `assistantResponseEvent` and `codeEvent` append visible text, while
    /// `reasoningContentEvent` appends separate reasoning. `toolUseEvent` retains its exact
    /// decoded fragment order until an explicit stop, then validates its complete object input.
    /// `contextUsageEvent` is accepted as non-semantic because this phase has no Kiro usage rule.
    ///
    /// # Errors
    ///
    /// Returns a safe error for bad framing metadata, a payload shape mismatch, unknown events,
    /// or an invalid Tool transition. Errors do not include IDs, names, JSON, or text.
    pub fn push_frame(
        &mut self,
        frame: &KiroEventStreamFrame,
    ) -> Result<Vec<CanonicalEvent>, KiroEventSemanticError> {
        if !self.started || self.finished {
            return Err(KiroEventSemanticError::InvalidLifecycle);
        }
        if frame.headers().message_type() != Some("event") {
            return Err(KiroEventSemanticError::UnexpectedFrame);
        }
        let event_type = frame
            .headers()
            .event_type()
            .ok_or(KiroEventSemanticError::UnexpectedFrame)?;
        let payload = strict_payload_object(frame.payload())?;
        match event_type {
            "assistantResponseEvent" | "codeEvent" => text_events(&payload),
            "reasoningContentEvent" => reasoning_events(&payload),
            "toolUseEvent" => self.tool_events(&payload),
            "contextUsageEvent" => Ok(Vec::new()),
            _ => Err(KiroEventSemanticError::UnsupportedEvent),
        }
    }

    /// Ends the open Canonical message and response once no Tool remains open.
    ///
    /// # Errors
    ///
    /// Returns [`KiroEventSemanticError::UnfinishedTool`] instead of silently completing an
    /// incomplete Tool call.
    pub fn finish(&mut self) -> Result<Vec<CanonicalEvent>, KiroEventSemanticError> {
        if !self.started || self.finished {
            return Err(KiroEventSemanticError::InvalidLifecycle);
        }
        if !self.open_tools.is_empty() {
            return Err(KiroEventSemanticError::UnfinishedTool);
        }
        self.finished = true;
        Ok(vec![
            CanonicalEvent::MessageEnd(MessageEnd {
                extensions: RawExtensions::default(),
            }),
            CanonicalEvent::ResponseEnd(ResponseEnd {
                stop_reason: None,
                stop_sequence: None,
                extensions: RawExtensions::default(),
            }),
        ])
    }

    fn tool_events(
        &mut self,
        payload: &Map<String, Value>,
    ) -> Result<Vec<CanonicalEvent>, KiroEventSemanticError> {
        let call_id = required_string(payload, "toolUseId")?;
        if !is_valid_identifier(call_id, MAX_TOOL_CALL_ID_BYTES) {
            return Err(KiroEventSemanticError::InvalidToolState);
        }
        let is_new = !self.open_tools.contains_key(call_id);
        if is_new {
            let name = required_string(payload, "name")?;
            if !is_valid_identifier(name, MAX_TOOL_NAME_BYTES) {
                return Err(KiroEventSemanticError::InvalidToolState);
            }
            self.open_tools.insert(
                call_id.to_owned(),
                OpenTool {
                    name: name.to_owned(),
                    input: String::new(),
                },
            );
        } else if payload.get("name").is_some_and(|value| {
            value.as_str() != self.open_tools.get(call_id).map(|tool| tool.name.as_str())
        }) {
            return Err(KiroEventSemanticError::InvalidToolState);
        }

        let stop = match payload.get("stop") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err(KiroEventSemanticError::InvalidToolState),
        };
        let input = match payload.get("input") {
            None => "",
            Some(Value::String(value)) => value.as_str(),
            Some(_) => return Err(KiroEventSemanticError::InvalidToolState),
        };
        let tool = self
            .open_tools
            .get_mut(call_id)
            .ok_or(KiroEventSemanticError::InvalidToolState)?;
        let next_len = tool
            .input
            .len()
            .checked_add(input.len())
            .ok_or(KiroEventSemanticError::ToolArgumentsTooLarge)?;
        if next_len > MAX_TOOL_ARGUMENT_BYTES {
            return Err(KiroEventSemanticError::ToolArgumentsTooLarge);
        }
        tool.input.push_str(input);

        let mut events = Vec::new();
        if is_new {
            events.push(CanonicalEvent::ToolCallStart(ToolCallStart {
                call_id: call_id.to_owned(),
                name: tool.name.clone(),
                extensions: RawExtensions::default(),
            }));
        }
        if !input.is_empty() {
            events.push(CanonicalEvent::ToolCallArgumentsDelta(
                ToolCallArgumentsDelta {
                    call_id: call_id.to_owned(),
                    delta: input.to_owned(),
                    extensions: RawExtensions::default(),
                },
            ));
        }
        if stop {
            let tool = self
                .open_tools
                .remove(call_id)
                .ok_or(KiroEventSemanticError::InvalidToolState)?;
            let arguments = completed_tool_arguments(&tool)?;
            events.push(CanonicalEvent::ToolCallEnd(ToolCallEnd {
                call_id: call_id.to_owned(),
                arguments,
                extensions: RawExtensions::default(),
            }));
        }
        Ok(events)
    }
}

impl fmt::Debug for KiroEventSemanticMapper {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroEventSemanticMapper")
            .field("response_id", &"<redacted>")
            .field("started", &self.started)
            .field("finished", &self.finished)
            .field("open_tool_count", &self.open_tools.len())
            .finish()
    }
}

struct OpenTool {
    name: String,
    input: String,
}

fn strict_payload_object(bytes: &[u8]) -> Result<Map<String, Value>, KiroEventSemanticError> {
    if bytes.len() > MAX_EVENT_PAYLOAD_BYTES {
        return Err(KiroEventSemanticError::InvalidPayload);
    }
    let value = deserialize_strict_json(bytes)?;
    value
        .as_object()
        .cloned()
        .ok_or(KiroEventSemanticError::InvalidPayload)
}

fn text_events(
    payload: &Map<String, Value>,
) -> Result<Vec<CanonicalEvent>, KiroEventSemanticError> {
    let mut events = Vec::new();
    for key in ["content", "code"] {
        match payload.get(key) {
            Some(Value::String(value)) if !value.is_empty() => {
                events.push(CanonicalEvent::TextDelta(TextDelta {
                    text: value.clone(),
                    extensions: RawExtensions::default(),
                }));
            }
            None | Some(Value::String(_)) => {}
            Some(_) => return Err(KiroEventSemanticError::InvalidPayload),
        }
    }
    Ok(events)
}

fn reasoning_events(
    payload: &Map<String, Value>,
) -> Result<Vec<CanonicalEvent>, KiroEventSemanticError> {
    let content = optional_same_string(payload, "content", "reasoningContent")?
        .ok_or(KiroEventSemanticError::InvalidPayload)?;
    if content.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![CanonicalEvent::ReasoningDelta(ReasoningDelta {
        text: content.to_owned(),
        extensions: RawExtensions::default(),
    })])
}

fn optional_same_string<'a>(
    payload: &'a Map<String, Value>,
    first: &str,
    second: &str,
) -> Result<Option<&'a str>, KiroEventSemanticError> {
    let first = optional_string(payload, first)?;
    let second = optional_string(payload, second)?;
    match (first, second) {
        (Some(first), Some(second)) if first != second => {
            Err(KiroEventSemanticError::InvalidPayload)
        }
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn optional_string<'a>(
    payload: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>, KiroEventSemanticError> {
    match payload.get(field) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(KiroEventSemanticError::InvalidPayload),
    }
}

fn required_string<'a>(
    payload: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, KiroEventSemanticError> {
    optional_string(payload, field)?.ok_or(KiroEventSemanticError::InvalidToolState)
}

fn completed_tool_arguments(tool: &OpenTool) -> Result<RawJson, KiroEventSemanticError> {
    if tool.input.is_empty() {
        if matches!(tool.name.as_str(), ENTER_PLAN_MODE | EXIT_PLAN_MODE) {
            return RawJson::from_json_string("{}".to_owned())
                .map_err(|_| KiroEventSemanticError::InvalidToolState);
        }
        return Err(KiroEventSemanticError::InvalidToolState);
    }
    let mut value = deserialize_strict_json(tool.input.as_bytes())?;
    if !value.is_object() {
        return Err(KiroEventSemanticError::InvalidToolState);
    }
    if tool.name == ASK_USER_QUESTION {
        normalize_ask_user_question(&mut value)?;
        return RawJson::from_json_string(
            serde_json::to_string(&value).map_err(|_| KiroEventSemanticError::InvalidToolState)?,
        )
        .map_err(|_| KiroEventSemanticError::InvalidToolState);
    }
    RawJson::from_json_string(tool.input.clone())
        .map_err(|_| KiroEventSemanticError::InvalidToolState)
}

fn normalize_ask_user_question(value: &mut Value) -> Result<(), KiroEventSemanticError> {
    let questions = value
        .as_object_mut()
        .and_then(|object| object.get_mut("questions"))
        .and_then(Value::as_array_mut)
        .ok_or(KiroEventSemanticError::InvalidToolState)?;
    if questions.is_empty() {
        return Err(KiroEventSemanticError::InvalidToolState);
    }
    for question in questions {
        let question = question
            .as_object_mut()
            .ok_or(KiroEventSemanticError::InvalidToolState)?;
        let normalized = match question.get("question") {
            Some(Value::String(value)) if !value.is_empty() => None,
            Some(_) => return Err(KiroEventSemanticError::InvalidToolState),
            None => match question.get("header") {
                Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
                _ => return Err(KiroEventSemanticError::InvalidToolState),
            },
        };
        if !matches!(question.get("options"), Some(Value::Array(_)))
            || !matches!(question.get("multiSelect"), Some(Value::Bool(_)))
        {
            return Err(KiroEventSemanticError::InvalidToolState);
        }
        if let Some(normalized) = normalized {
            question.insert("question".to_owned(), Value::String(normalized));
        }
    }
    Ok(())
}

fn is_valid_identifier(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn deserialize_strict_json(input: &[u8]) -> Result<Value, KiroEventSemanticError> {
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let value = <StrictJsonValue as serde::Deserialize>::deserialize(&mut deserializer)
        .map_err(|_| KiroEventSemanticError::InvalidPayload)?;
    deserializer
        .end()
        .map_err(|_| KiroEventSemanticError::InvalidPayload)?;
    Ok(value.0)
}

struct StrictJsonValue(Value);

impl<'de> serde::Deserialize<'de> for StrictJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictJsonVisitor)
    }
}

struct StrictJsonVisitor;

impl<'de> Visitor<'de> for StrictJsonVisitor {
    type Value = StrictJsonValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("strict JSON without duplicate object fields")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrictJsonValue(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrictJsonValue(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let number = serde_json::Number::from_f64(value)
            .ok_or_else(|| E::custom("JSON number is not finite"))?;
        Ok(StrictJsonValue(Value::Number(number)))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrictJsonValue(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::Null))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<StrictJsonValue>()? {
            values.push(value.0);
        }
        Ok(StrictJsonValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON object field"));
            }
            let value = map.next_value::<StrictJsonValue>()?;
            values.insert(key, value.0);
        }
        Ok(StrictJsonValue(Value::Object(values)))
    }
}
