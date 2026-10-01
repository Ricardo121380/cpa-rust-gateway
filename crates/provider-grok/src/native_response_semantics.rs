//! Closed native Responses event schemas shared by the isolated Grok decoders.
use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode, RawExtensions};
use serde_json::{Map, Value, json};

fn protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

pub(crate) fn validate_event(
    event: &Map<String, Value>,
    item_order: &[String],
) -> Result<(), GatewayError> {
    let kind = event
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(protocol_error)?;
    let fields: &[&str] = match kind {
        "response.output_item.added" | "response.output_item.done" => &["item", "output_index"],
        "response.output_text.delta" => &[
            "item_id",
            "output_index",
            "content_index",
            "delta",
            "logprobs",
        ],
        "response.output_text.done" => &[
            "item_id",
            "output_index",
            "content_index",
            "text",
            "logprobs",
        ],
        "response.output_text.annotation.added" => &[
            "item_id",
            "output_index",
            "content_index",
            "annotation_index",
            "annotation",
        ],
        "response.content_part.added" | "response.content_part.done" => {
            &["item_id", "output_index", "content_index", "part"]
        }
        "response.reasoning.delta" | "response.reasoning_text.delta" => {
            &["item_id", "output_index", "content_index", "delta"]
        }
        "response.reasoning.done" | "response.reasoning_text.done" => {
            &["item_id", "output_index", "content_index", "text"]
        }
        "response.reasoning_summary_text.delta" => {
            &["item_id", "output_index", "summary_index", "delta"]
        }
        "response.reasoning_summary_text.done" => {
            &["item_id", "output_index", "summary_index", "text"]
        }
        "response.reasoning_summary_part.added" | "response.reasoning_summary_part.done" => {
            &["item_id", "output_index", "summary_index", "part"]
        }
        "response.function_call_arguments.delta" => {
            &["item_id", "output_index", "call_id", "delta"]
        }
        "response.function_call_arguments.done" => {
            &["item_id", "output_index", "call_id", "name", "arguments"]
        }
        _ => return Ok(()),
    };
    if event.keys().any(|key| {
        !matches!(key.as_str(), "type" | "sequence_number") && !fields.contains(&key.as_str())
    }) {
        return Err(protocol_error());
    }
    if event
        .get("sequence_number")
        .is_some_and(|value| value.as_u64().is_none())
        || event
            .get("logprobs")
            .is_some_and(|value| !value.is_null() && value != &json!([]))
    {
        return Err(protocol_error());
    }
    if let Some(index) = event.get("output_index") {
        let index = index
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(protocol_error)?;
        if kind == "response.output_item.added" {
            if index != item_order.len() {
                return Err(protocol_error());
            }
        } else {
            let id = event
                .get("item_id")
                .or_else(|| event.get("item").and_then(|item| item.get("id")))
                .and_then(Value::as_str)
                .ok_or_else(protocol_error)?;
            if item_order.get(index).map(String::as_str) != Some(id) {
                return Err(protocol_error());
            }
        }
    }
    Ok(())
}

pub(crate) fn record_annotation(
    event: &Map<String, Value>,
    annotations: &mut std::collections::BTreeMap<(String, usize), Vec<Value>>,
) -> Result<RawExtensions, GatewayError> {
    let id = event
        .get("item_id")
        .and_then(Value::as_str)
        .ok_or_else(protocol_error)?;
    let index = part_index(event, "content_index")?;
    let annotation_index = part_index(event, "annotation_index")?;
    let annotation = event.get("annotation").ok_or_else(protocol_error)?;
    record_annotation_value(id, index, annotation_index, annotation, annotations)
}

fn record_annotation_value(
    id: &str,
    index: usize,
    annotation_index: usize,
    annotation: &Value,
    annotations: &mut std::collections::BTreeMap<(String, usize), Vec<Value>>,
) -> Result<RawExtensions, GatewayError> {
    let extensions = protocol_openai_responses::native_annotation_extensions(
        id,
        index,
        annotation_index,
        annotation,
    )?;
    let retained: usize = annotations
        .values()
        .flatten()
        .map(|value| value.to_string().len())
        .sum();
    if retained.saturating_add(annotation.to_string().len()) > 1024 * 1024 {
        return Err(protocol_error());
    }
    let observed = annotations.entry((id.to_owned(), index)).or_default();
    if annotation_index != observed.len() {
        return Err(protocol_error());
    }
    observed.push(annotation.clone());
    Ok(extensions)
}

pub(crate) fn record_part_annotations(
    id: &str,
    index: usize,
    part: &Map<String, Value>,
    annotations: &mut std::collections::BTreeMap<(String, usize), Vec<Value>>,
) -> Result<Vec<RawExtensions>, GatewayError> {
    part.get("annotations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(annotation_index, annotation)| {
            record_annotation_value(id, index, annotation_index, annotation, annotations)
        })
        .collect()
}

pub(crate) fn confirm_annotations(
    item: &Map<String, Value>,
    annotations: &std::collections::BTreeMap<(String, usize), Vec<Value>>,
) -> Result<(), GatewayError> {
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(protocol_error)?;
    for ((item_id, index), observed) in annotations {
        if item_id == id
            && item
                .get("content")
                .and_then(Value::as_array)
                .and_then(|parts| parts.get(*index))
                .and_then(|part| part.get("annotations"))
                .and_then(Value::as_array)
                .is_none_or(|final_annotations| !final_annotations.starts_with(observed))
        {
            return Err(protocol_error());
        }
    }
    Ok(())
}

pub(crate) fn part_index(event: &Map<String, Value>, field: &str) -> Result<usize, GatewayError> {
    match event.get(field) {
        None => Ok(0),
        Some(value) => value
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .filter(|index| *index < 64)
            .ok_or_else(protocol_error),
    }
}

pub(crate) fn validate_part(part: &Map<String, Value>) -> Result<(), GatewayError> {
    let item = match part.get("type").and_then(Value::as_str) {
        Some("output_text") => {
            json!({"id":"part-validation","type":"message","role":"assistant","content":[part]})
        }
        Some("reasoning_text") => {
            json!({"id":"part-validation","type":"reasoning","content":[part]})
        }
        Some("summary_text") => json!({"id":"part-validation","type":"reasoning","summary":[part]}),
        _ => return Err(protocol_error()),
    };
    protocol_openai_responses::native_item_metadata(&item, true).map(|_| ())
}

pub(crate) fn confirm_part_snapshots(
    item: &Map<String, Value>,
    snapshots: &std::collections::BTreeMap<(String, String, usize), Value>,
) -> Result<(), GatewayError> {
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(protocol_error)?;
    for ((item_id, field, index), snapshot) in snapshots {
        if item_id == id
            && item
                .get(field)
                .and_then(Value::as_array)
                .and_then(|parts| parts.get(*index))
                != Some(snapshot)
        {
            return Err(protocol_error());
        }
    }
    Ok(())
}
