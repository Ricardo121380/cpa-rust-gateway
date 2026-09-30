//! Closed Responses citation schema, preserved without fetching any referenced resource.

use gateway_core::GatewayError;
use serde_json::Value;

use crate::stream_protocol_error;

/// Validates a bounded array of the four supported Responses citation shapes.
///
/// # Errors
/// Rejects unknown types or fields, malformed indices and inverted ranges.
pub fn validate(value: &Value) -> Result<(), GatewayError> {
    let values = value
        .as_array()
        .filter(|values| values.len() <= 64)
        .ok_or_else(stream_protocol_error)?;
    for value in values {
        validate_one(value)?;
    }
    Ok(())
}

pub(crate) fn validate_one(value: &Value) -> Result<(), GatewayError> {
    let value = value.as_object().ok_or_else(stream_protocol_error)?;
    let fields: &[&str] = match value.get("type").and_then(Value::as_str) {
        Some("url_citation") => &["type", "start_index", "end_index", "url", "title"],
        Some("file_citation") => &["type", "index", "file_id", "filename"],
        Some("container_file_citation") => &[
            "type",
            "start_index",
            "end_index",
            "file_id",
            "filename",
            "container_id",
        ],
        Some("file_path") => &["type", "index", "file_id"],
        _ => return Err(stream_protocol_error()),
    };
    if value.len() != fields.len() || value.keys().any(|key| !fields.contains(&key.as_str())) {
        return Err(stream_protocol_error());
    }
    for field in fields {
        let field_value = value.get(*field).ok_or_else(stream_protocol_error)?;
        if matches!(*field, "index" | "start_index" | "end_index") {
            if field_value.as_u64().is_none() {
                return Err(stream_protocol_error());
            }
        } else if !field_value.is_string() {
            return Err(stream_protocol_error());
        }
    }
    if value
        .get("start_index")
        .zip(value.get("end_index"))
        .is_some_and(|(start, end)| start.as_u64() > end.as_u64())
    {
        return Err(stream_protocol_error());
    }
    Ok(())
}
