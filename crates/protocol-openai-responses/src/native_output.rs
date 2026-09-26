//! Explicit Responses item metadata. Ciphertext is never admitted without an ownership path.
use super::{
    OpenAiResponsesSseEncoder, OutputItem, SseFrame, frame, function_arguments_delta_frame,
    function_arguments_done_frame, function_data, output_item_added_frame, output_item_done_frame,
    stream_protocol_error,
};
use gateway_core::{CanonicalEvent, GatewayError, OutputItemMetadata, RawExtensions, RawJson};
use serde::Deserialize;
use serde_json::{Value, json};

const ITEM: &str = "openai.responses.output_item";
const PART: &str = "openai.responses.output_part";

/// Builds bounded, redacted item metadata for a Responses-native producer.
///
/// # Errors
/// Rejects unrepresentable items, unsafe identities, and unowned encrypted reasoning.
#[allow(clippy::too_many_lines)] // Keep the closed item schema and opaque-token boundary together.
pub fn native_item_metadata(
    item: &Value,
    completed: bool,
) -> Result<OutputItemMetadata, GatewayError> {
    let mut item = item
        .as_object()
        .cloned()
        .ok_or_else(stream_protocol_error)?;
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(stream_protocol_error)?
        .to_owned();
    if id.is_empty() || id.len() > 512 || !id.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(stream_protocol_error());
    }
    let kind = item
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(stream_protocol_error)?;
    let allowed: &[&str] = match kind {
        "reasoning" => &[
            "id",
            "type",
            "status",
            "summary",
            "content",
            "encrypted_content",
        ],
        "message" => &["id", "type", "status", "role", "content"],
        "function_call" => &["id", "type", "status", "call_id", "name", "arguments"],
        _ => return Err(stream_protocol_error()),
    };
    if item.keys().any(|key| !allowed.contains(&key.as_str()))
        || item.get("encrypted_content").is_some_and(|v| {
            !v.is_null()
                && v.as_str()
                    .is_none_or(|v| !gateway_core::is_owned_reasoning_token(v))
        })
    {
        return Err(stream_protocol_error());
    }
    let incomplete = completed
        && kind != "function_call"
        && item.get("status").and_then(Value::as_str) == Some("incomplete");
    if item.get("encrypted_content").is_some_and(Value::is_null) {
        item.remove("encrypted_content");
    }
    item.insert(
        "status".into(),
        json!(if incomplete {
            "incomplete"
        } else if completed {
            "completed"
        } else {
            "in_progress"
        }),
    );
    match item["type"].as_str() {
        Some("reasoning") => {
            item.entry("summary").or_insert_with(|| json!([]));
            for (field, kind) in [("summary", "summary_text"), ("content", "reasoning_text")] {
                if let Some(parts) = item.get(field) {
                    validate_parts(parts, kind)?;
                }
            }
        }
        Some("message") => {
            item.entry("role").or_insert_with(|| json!("assistant"));
            if item["role"] != "assistant" {
                return Err(stream_protocol_error());
            }
            item.entry("content").or_insert_with(|| json!([]));
            validate_parts(&item["content"], "output_text")?;
        }
        Some("function_call") => {
            for field in ["call_id", "name"] {
                if item
                    .get(field)
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                {
                    return Err(stream_protocol_error());
                }
            }
            item.entry("arguments").or_insert_with(|| json!(""));
            if !item["arguments"].is_string() {
                return Err(stream_protocol_error());
            }
        }
        _ => return Err(stream_protocol_error()),
    }
    if !completed {
        clear_item_content(&mut item);
    }
    let serialized = Value::Object(item).to_string();
    if serialized.len() > 1024 * 1024 {
        return Err(stream_protocol_error());
    }
    let mut extensions = RawExtensions::default();
    extensions
        .try_insert(
            ITEM,
            RawJson::from_json_string(serialized).map_err(|_| stream_protocol_error())?,
        )
        .map_err(|_| stream_protocol_error())?;
    Ok(OutputItemMetadata {
        item_id: id,
        extensions,
    })
}

fn clear_item_content(item: &mut serde_json::Map<String, Value>) {
    item.remove("encrypted_content");
    for field in ["summary", "content"] {
        if item.contains_key(field) {
            item.insert(field.into(), json!([]));
        }
    }
    if item.contains_key("arguments") {
        item.insert("arguments".into(), json!(""));
    }
}

fn validate_parts(value: &Value, kind: &str) -> Result<(), GatewayError> {
    let parts = value.as_array().ok_or_else(stream_protocol_error)?;
    if parts.len() > 64 {
        return Err(stream_protocol_error());
    }
    for part in parts {
        let part = part.as_object().ok_or_else(stream_protocol_error)?;
        if part.get("type") != Some(&json!(kind))
            || part
                .get("text")
                .and_then(Value::as_str)
                .is_none_or(|v| v.contains('\0'))
            || part
                .keys()
                .any(|key| !matches!(key.as_str(), "type" | "text" | "annotations"))
            || part
                .get("annotations")
                .is_some_and(|v| kind != "output_text" || v != &json!([]))
        {
            return Err(stream_protocol_error());
        }
    }
    Ok(())
}

/// Associates a semantic delta with its original output item and part.
///
/// # Errors
/// Rejects unsupported fields or unbounded indices.
pub fn native_part_extensions(
    id: &str,
    field: &str,
    index: usize,
) -> Result<RawExtensions, GatewayError> {
    if !matches!(field, "content" | "summary") || index >= 64 {
        return Err(stream_protocol_error());
    }
    let mut ext = RawExtensions::default();
    ext.try_insert(
        PART,
        RawJson::from_json_string(json!({"id":id,"field":field,"index":index}).to_string())
            .map_err(|_| stream_protocol_error())?,
    )
    .map_err(|_| stream_protocol_error())?;
    Ok(ext)
}

/// Checks the closed namespace carried by native output lifecycle/delta events.
#[must_use]
pub fn has_native_output_metadata(event: &CanonicalEvent) -> bool {
    let (ext, key) = match event {
        CanonicalEvent::OutputItemStart(v) | CanonicalEvent::OutputItemEnd(v) => {
            (&v.extensions, ITEM)
        }
        CanonicalEvent::TextDelta(v) => (&v.extensions, PART),
        CanonicalEvent::ReasoningDelta(v) => (&v.extensions, PART),
        _ => return false,
    };
    ext.iter().len() == 1 && ext.get(key).is_some()
}

fn is_empty_tool_object(arguments: &str) -> bool {
    arguments.trim().is_empty()
        || serde_json::from_str::<Value>(arguments)
            .is_ok_and(|value| value.as_object().is_some_and(serde_json::Map::is_empty))
}

fn metadata_value(meta: &OutputItemMetadata, completed: bool) -> Result<Value, GatewayError> {
    let raw = meta
        .extensions
        .get(ITEM)
        .ok_or_else(stream_protocol_error)?;
    let value: Value = serde_json::from_str(raw.get()).map_err(|_| stream_protocol_error())?;
    if native_item_metadata(&value, completed)? != *meta {
        return Err(stream_protocol_error());
    }
    Ok(value)
}

impl OpenAiResponsesSseEncoder {
    pub(super) fn encode_native_event(
        &mut self,
        event: &CanonicalEvent,
    ) -> Result<Option<Vec<SseFrame>>, GatewayError> {
        match event {
            CanonicalEvent::OutputItemStart(meta) => {
                let value = metadata_value(meta, false)?;
                let state = self.response_mut()?;
                if state.output.iter().any(|v| v.id() == meta.item_id) {
                    return Err(stream_protocol_error());
                }
                let index = state.output.len();
                state.output.push(OutputItem::Native {
                    id: meta.item_id.clone(),
                    value: value.clone(),
                });
                Ok(Some(vec![output_item_added_frame(
                    &mut self.next_sequence_number,
                    index,
                    value,
                )?]))
            }
            CanonicalEvent::OutputItemEnd(meta) => self.finish_native(meta).map(Some),
            CanonicalEvent::TextDelta(v) if has_native_output_metadata(event) => {
                self.append_native(&v.extensions, &v.text, false).map(Some)
            }
            CanonicalEvent::ReasoningDelta(v) if has_native_output_metadata(event) => {
                self.append_native(&v.extensions, &v.text, true).map(Some)
            }
            CanonicalEvent::ToolCallStart(v) if self.native_tool_index(&v.call_id).is_some() => {
                let index = self
                    .native_tool_index(&v.call_id)
                    .ok_or_else(stream_protocol_error)?;
                if self.native_value(index)?["name"] != v.name {
                    return Err(stream_protocol_error());
                }
                Ok(Some(Vec::new()))
            }
            CanonicalEvent::ToolCallArgumentsDelta(v)
                if self.native_tool_index(&v.call_id).is_some() =>
            {
                let index = self
                    .native_tool_index(&v.call_id)
                    .ok_or_else(stream_protocol_error)?;
                let item = self.native_value(index)?;
                let id = item["id"]
                    .as_str()
                    .ok_or_else(stream_protocol_error)?
                    .to_owned();
                let mut args = item["arguments"]
                    .as_str()
                    .ok_or_else(stream_protocol_error)?
                    .to_owned();
                args.push_str(&v.delta);
                self.native_value_mut(index)?["arguments"] = json!(args);
                Ok(Some(vec![function_arguments_delta_frame(
                    &mut self.next_sequence_number,
                    index,
                    &id,
                    &v.delta,
                )?]))
            }
            CanonicalEvent::ToolCallEnd(v) if self.native_tool_index(&v.call_id).is_some() => {
                let index = self
                    .native_tool_index(&v.call_id)
                    .ok_or_else(stream_protocol_error)?;
                let item = self.native_value_mut(index)?;
                let old = item["arguments"]
                    .as_str()
                    .ok_or_else(stream_protocol_error)?;
                // The native Build contract normalizes blank/empty objects to `{}`.
                // Preserve that exception across streamed deltas without accepting other drift.
                if !old.is_empty()
                    && old != v.arguments.get()
                    && !(v.arguments.get() == "{}" && is_empty_tool_object(old))
                {
                    return Err(stream_protocol_error());
                }
                item["arguments"] = json!(v.arguments.get());
                Ok(Some(Vec::new()))
            }
            _ => Ok(None),
        }
    }

    fn native_tool_index(&self, call: &str) -> Option<usize> {
        self.response.as_ref()?.output.iter().position(|item| matches!(item,
            OutputItem::Native { value, .. } if value["type"] == "function_call" && value["call_id"] == call))
    }
    fn native_value(&self, index: usize) -> Result<&Value, GatewayError> {
        match self
            .response
            .as_ref()
            .and_then(|state| state.output.get(index))
        {
            Some(OutputItem::Native { value, .. }) => Ok(value),
            _ => Err(stream_protocol_error()),
        }
    }
    fn native_value_mut(&mut self, index: usize) -> Result<&mut Value, GatewayError> {
        match self.response_mut()?.output.get_mut(index) {
            Some(OutputItem::Native { value, .. }) => Ok(value),
            _ => Err(stream_protocol_error()),
        }
    }

    fn append_native(
        &mut self,
        ext: &RawExtensions,
        text: &str,
        reasoning: bool,
    ) -> Result<Vec<SseFrame>, GatewayError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Part {
            id: String,
            field: String,
            index: usize,
        }
        let part: Part =
            serde_json::from_str(ext.get(PART).ok_or_else(stream_protocol_error)?.get())
                .map_err(|_| stream_protocol_error())?;
        if part.index >= 64
            || !matches!(part.field.as_str(), "content" | "summary")
            || (!reasoning && part.field != "content")
        {
            return Err(stream_protocol_error());
        }
        let index = self
            .response_mut()?
            .output
            .iter()
            .position(|v| v.id() == part.id)
            .ok_or_else(stream_protocol_error)?;
        let value = self.native_value_mut(index)?;
        if value["status"] != "in_progress"
            || value["type"] != if reasoning { "reasoning" } else { "message" }
        {
            return Err(stream_protocol_error());
        }
        let kind = if part.field == "summary" {
            "summary_text"
        } else if reasoning {
            "reasoning_text"
        } else {
            "output_text"
        };
        let parts = value
            .as_object_mut()
            .ok_or_else(stream_protocol_error)?
            .entry(part.field.clone())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(stream_protocol_error)?;
        let first_new = parts.len();
        while part.index >= parts.len() {
            parts.push(json!({"type":kind,"text":""}));
        }
        let mut content = parts[part.index]["text"]
            .as_str()
            .ok_or_else(stream_protocol_error)?
            .to_owned();
        content.push_str(text);
        parts[part.index]["text"] = json!(content);
        let mut frames = Vec::new();
        let index_key = if part.field == "summary" {
            "summary_index"
        } else {
            "content_index"
        };
        let mut data = function_data(index, &part.id);
        data.insert(index_key.into(), json!(part.index));
        for new_index in first_new..=part.index {
            let mut added_data = data.clone();
            added_data.insert(index_key.into(), json!(new_index));
            added_data.insert("part".into(), json!({"type":kind,"text":""}));
            let event = if part.field == "summary" {
                "response.reasoning_summary_part.added"
            } else {
                "response.content_part.added"
            };
            frames.push(frame(&mut self.next_sequence_number, event, added_data)?);
        }
        data.insert("delta".into(), json!(text));
        let event = if part.field == "summary" {
            "response.reasoning_summary_text.delta"
        } else if reasoning {
            "response.reasoning_text.delta"
        } else {
            "response.output_text.delta"
        };
        frames.push(frame(&mut self.next_sequence_number, event, data)?);
        Ok(frames)
    }

    fn finish_native(&mut self, meta: &OutputItemMetadata) -> Result<Vec<SseFrame>, GatewayError> {
        let value = metadata_value(meta, true)?;
        let index = self
            .response_mut()?
            .output
            .iter()
            .position(|v| v.id() == meta.item_id)
            .ok_or_else(stream_protocol_error)?;
        let old = self.native_value(index)?.clone();
        if old["status"] != "in_progress" || old["type"] != value["type"] {
            return Err(stream_protocol_error());
        }
        let mut frames = Vec::new();
        if value["type"] == "function_call" {
            if old["call_id"] != value["call_id"]
                || old["name"] != value["name"]
                || old["arguments"] != value["arguments"]
            {
                return Err(stream_protocol_error());
            }
            frames.push(function_arguments_done_frame(
                &mut self.next_sequence_number,
                index,
                &meta.item_id,
                value["name"].as_str().ok_or_else(stream_protocol_error)?,
                value["arguments"]
                    .as_str()
                    .ok_or_else(stream_protocol_error)?,
            )?);
        } else {
            for field in ["content", "summary"] {
                let empty = Vec::new();
                let previous = old.get(field).and_then(Value::as_array).unwrap_or(&empty);
                let final_parts = value.get(field).and_then(Value::as_array).unwrap_or(&empty);
                if previous.len() > final_parts.len() {
                    return Err(stream_protocol_error());
                }
                for (part_index, part) in final_parts.iter().enumerate() {
                    let text = part["text"].as_str().ok_or_else(stream_protocol_error)?;
                    let emitted = previous
                        .get(part_index)
                        .and_then(|v| v["text"].as_str())
                        .unwrap_or("");
                    let suffix = text
                        .strip_prefix(emitted)
                        .ok_or_else(stream_protocol_error)?;
                    if !suffix.is_empty() || part_index >= previous.len() {
                        frames.extend(self.append_native(
                            &native_part_extensions(&meta.item_id, field, part_index)?,
                            suffix,
                            value["type"] == "reasoning",
                        )?);
                    }
                    let mut data = function_data(index, &meta.item_id);
                    data.insert(
                        if field == "summary" {
                            "summary_index"
                        } else {
                            "content_index"
                        }
                        .into(),
                        json!(part_index),
                    );
                    data.insert("text".into(), json!(text));
                    let event = if field == "summary" {
                        "response.reasoning_summary_text.done"
                    } else if value["type"] == "reasoning" {
                        "response.reasoning_text.done"
                    } else {
                        "response.output_text.done"
                    };
                    frames.push(frame(&mut self.next_sequence_number, event, data.clone())?);
                    data.remove("text");
                    data.insert("part".into(), part.clone());
                    frames.push(frame(
                        &mut self.next_sequence_number,
                        if field == "summary" {
                            "response.reasoning_summary_part.done"
                        } else {
                            "response.content_part.done"
                        },
                        data,
                    )?);
                }
            }
        }
        *self.native_value_mut(index)? = value.clone();
        frames.push(output_item_done_frame(
            &mut self.next_sequence_number,
            index,
            value,
        )?);
        Ok(frames)
    }
}
