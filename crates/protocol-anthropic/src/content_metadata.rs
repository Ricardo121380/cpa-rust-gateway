//! Retained Messages content-block identity, signature fragments and citations.

use gateway_core::{GatewayError, RawExtensions, RawJson};
use serde_json::Value;

use crate::json::stream_protocol_error;

pub(crate) const BLOCK_INDEX: &str = "anthropic.content_block.index";
pub(crate) const SIGNATURE: &str = "anthropic.content_block.signature";
pub(crate) const CITATIONS: &str = "anthropic.content_block.citations";

#[derive(Default)]
pub(crate) struct ContentMetadata {
    pub index: Option<u64>,
    pub signature: Option<String>,
    pub citations: Vec<Value>,
}

pub(crate) fn decode(
    extensions: &RawExtensions,
    thinking: bool,
) -> Result<ContentMetadata, GatewayError> {
    let mut metadata = ContentMetadata::default();
    for (key, raw) in extensions.iter() {
        let value: Value = serde_json::from_str(raw.get()).map_err(|_| stream_protocol_error())?;
        match key {
            BLOCK_INDEX => {
                metadata.index = Some(
                    value
                        .as_u64()
                        .filter(|index| *index < 4096)
                        .ok_or_else(stream_protocol_error)?,
                );
            }
            SIGNATURE if thinking => {
                metadata.signature = Some(
                    value
                        .as_str()
                        .filter(|signature| !signature.is_empty())
                        .ok_or_else(stream_protocol_error)?
                        .to_owned(),
                );
            }
            CITATIONS if !thinking => {
                metadata
                    .citations
                    .clone_from(value.as_array().ok_or_else(stream_protocol_error)?);
                validate_citations(&value)?;
            }
            _ => return Err(stream_protocol_error()),
        }
    }
    Ok(metadata)
}

/// Validates bounded citation objects without fetching their referenced resources.
///
/// # Errors
/// Rejects unknown citation types, fields, malformed ranges or ambiguous scalar values.
pub fn validate_citations(value: &Value) -> Result<(), GatewayError> {
    let citations = value
        .as_array()
        .filter(|items| items.len() <= 64)
        .ok_or_else(stream_protocol_error)?;
    for citation in citations {
        let fields = citation.as_object().ok_or_else(stream_protocol_error)?;
        let (required, range): (&[&str], Option<(&str, &str)>) =
            match fields.get("type").and_then(Value::as_str) {
                Some("char_location") => (
                    &[
                        "type",
                        "cited_text",
                        "document_index",
                        "document_title",
                        "start_char_index",
                        "end_char_index",
                    ],
                    Some(("start_char_index", "end_char_index")),
                ),
                Some("page_location") => (
                    &[
                        "type",
                        "cited_text",
                        "document_index",
                        "document_title",
                        "start_page_number",
                        "end_page_number",
                    ],
                    Some(("start_page_number", "end_page_number")),
                ),
                Some("content_block_location") => (
                    &[
                        "type",
                        "cited_text",
                        "document_index",
                        "document_title",
                        "start_block_index",
                        "end_block_index",
                    ],
                    Some(("start_block_index", "end_block_index")),
                ),
                Some("search_result_location") => (
                    &[
                        "type",
                        "cited_text",
                        "search_result_index",
                        "source",
                        "title",
                        "start_block_index",
                        "end_block_index",
                    ],
                    Some(("start_block_index", "end_block_index")),
                ),
                Some("web_search_result_location") => (
                    &["type", "cited_text", "encrypted_index", "title", "url"],
                    None,
                ),
                _ => return Err(stream_protocol_error()),
            };
        let document = fields.contains_key("document_index");
        if fields
            .keys()
            .any(|key| !(required.contains(&key.as_str()) || document && key == "file_id"))
            || required
                .iter()
                .filter(|key| !matches!(**key, "title" | "document_title"))
                .any(|key| !fields.contains_key(*key))
        {
            return Err(stream_protocol_error());
        }
        for (key, value) in fields {
            let valid = if key.ends_with("_index") && key != "encrypted_index"
                || key.ends_with("_number")
            {
                value.as_u64().is_some()
            } else if matches!(key.as_str(), "title" | "document_title" | "file_id") {
                value.is_null() || value.is_string()
            } else {
                value.is_string()
            };
            if !valid {
                return Err(stream_protocol_error());
            }
        }
        if let Some((start, end)) = range {
            let start = fields[start].as_u64().ok_or_else(stream_protocol_error)?;
            let end = fields[end].as_u64().ok_or_else(stream_protocol_error)?;
            if end <= start
                || start == 0 && fields.get("type").and_then(Value::as_str) == Some("page_location")
            {
                return Err(stream_protocol_error());
            }
        }
    }
    Ok(())
}

pub(crate) fn encode(
    index: usize,
    signature: Option<&Value>,
    citations: Option<&Value>,
) -> Result<RawExtensions, GatewayError> {
    let mut extensions = RawExtensions::default();
    for (key, value) in [
        (BLOCK_INDEX, Some(&Value::from(index))),
        (SIGNATURE, signature),
        (CITATIONS, citations),
    ] {
        if let Some(value) = value {
            extensions
                .try_insert(
                    key,
                    RawJson::from_json_string(value.to_string())
                        .map_err(|_| stream_protocol_error())?,
                )
                .map_err(|_| stream_protocol_error())?;
        }
    }
    // Validate the retained shape before handing it to Canonical or a client encoder.
    decode(&extensions, signature.is_some())?;
    Ok(extensions)
}
