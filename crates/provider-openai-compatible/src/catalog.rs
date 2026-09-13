//! Bounded OpenAI-compatible model-list decoding. Network admission belongs to the caller.
use gateway_catalog::DiscoveredModel;
use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};
use serde_json::Value;
use std::collections::BTreeSet;

/// One complete upstream page and its optional opaque `after_id` continuation.
#[derive(Debug)]
pub struct CompatibleCatalogPage {
    /// Exact source model identifiers, without generated aliases or capabilities.
    pub models: Vec<DiscoveredModel>,
    /// Present only when the upstream explicitly reports another page.
    pub after: Option<String>,
}

/// Parses a bounded compatible `/models` response without inferring model permissions.
/// # Errors
/// Rejects malformed, oversized or incomplete pagination rather than reporting partial success.
pub fn parse_compatible_catalog(bytes: &[u8]) -> Result<CompatibleCatalogPage, GatewayError> {
    let invalid = || {
        GatewayError::new(
            GatewayErrorCode::UpstreamProtocolError,
            ErrorScope::Provider,
        )
    };
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(invalid());
    }
    let root: Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let entries = root
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if entries.len() > 10_000 {
        return Err(invalid());
    }
    let mut seen = BTreeSet::new();
    let mut models = Vec::new();
    for entry in entries {
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= 256
                    && id.trim() == *id
                    && !id.chars().any(char::is_control)
            })
            .ok_or_else(invalid)?;
        if seen.insert(id) {
            models.push(DiscoveredModel::try_new(id.to_owned()).map_err(|_| invalid())?);
        }
    }
    let has_more = match root.get("has_more") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err(invalid()),
    };
    let after = if has_more {
        Some(
            root.get("last_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty() && id.len() <= 256 && seen.contains(id))
                .ok_or_else(invalid)?
                .to_owned(),
        )
    } else {
        None
    };
    Ok(CompatibleCatalogPage { models, after })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_preserves_exact_ids_and_requires_complete_pagination()
    -> Result<(), Box<dyn std::error::Error>> {
        let page = parse_compatible_catalog(br#"{"data":[{"id":"Vendor/Exact-v2"},{"id":"other"},{"id":"other"}],"has_more":true,"last_id":"other"}"#)?;
        assert_eq!(page.models.len(), 2);
        assert_eq!(page.after.as_deref(), Some("other"));
        assert!(parse_compatible_catalog(br#"{"data":[{"id":"x"}],"has_more":true}"#).is_err());
        assert!(parse_compatible_catalog(br#"{"data":[{"id":" x "}]}"#).is_err());
        assert!(
            parse_compatible_catalog(br#"{"data":[]}"#)?
                .models
                .is_empty()
        );
        Ok(())
    }
}
