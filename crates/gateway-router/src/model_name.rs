//! CPA v8.0.40 name suffix syntax (internal/thinking/suffix.go), with exact names admitted first.
//! MIT attribution and fixed source: THIRD_PARTY_NOTICES.md / ADR-0130.
use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode, ModelThinking, ThinkingEffort};

/// Parses a final thinking suffix after the caller has ruled out an exact registered name.
///
/// # Errors
/// Rejects malformed/unknown suffixes; never invents a new base model or permission.
pub fn parse_thinking_suffix(name: &str) -> Result<Option<(&str, ModelThinking)>, GatewayError> {
    let invalid = || GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Model);
    let Some((base, tail)) = name.rsplit_once('(') else {
        return Ok(None);
    };
    let suffix = tail.strip_suffix(')').ok_or_else(invalid)?;
    if base.is_empty() || suffix.is_empty() {
        return Err(invalid());
    }
    let lower = suffix.to_ascii_lowercase();
    let control = if matches!(
        lower.as_str(),
        "none" | "auto" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
    ) {
        ModelThinking::Effort(ThinkingEffort::try_new(lower).map_err(|_| invalid())?)
    } else {
        let budget: i64 = suffix.parse().map_err(|_| invalid())?;
        if budget < -1 {
            return Err(invalid());
        }
        ModelThinking::Budget(budget)
    };
    Ok(Some((base, control)))
}

/// Materializes one explicitly configured prefix using CPA's prefix/model spelling.
///
/// # Errors
/// Rejects empty/multiple segments, whitespace, control characters or an oversized final alias.
/// Existing exact aliases supplied without this optional prefix keep their original spelling.
pub fn prefixed_model_alias(prefix: &str, alias: &str) -> Result<String, GatewayError> {
    let prefix = prefix.trim().trim_matches('/');
    if prefix.is_empty()
        || prefix.len() > 128
        || !prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        || alias.is_empty()
        || alias.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(GatewayError::new(
            GatewayErrorCode::ClientRequestError,
            ErrorScope::Model,
        ));
    }
    let name = format!("{prefix}/{alias}");
    if name.len() > 256 {
        return Err(GatewayError::new(
            GatewayErrorCode::ClientRequestError,
            ErrorScope::Model,
        ));
    }
    Ok(name)
}

/// Recognizes the qualified aliases materialized by the explicit prefix input.
#[must_use]
pub fn is_prefixed_model_alias(name: &str) -> bool {
    name.split_once('/')
        .is_some_and(|(prefix, alias)| prefixed_model_alias(prefix, alias).is_ok_and(|v| v == name))
}
