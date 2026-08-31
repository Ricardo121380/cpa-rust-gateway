//! Strict Grok Build subscription and signed-token entitlement classification.

use std::{error::Error, fmt};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_core::{
    ProviderAccountEntitlement, ProviderAccountEntitlementConfidence,
    ProviderAccountEntitlementSource, ProviderAccountEntitlementTier,
};
use serde_json::Value;

use crate::strict_json::parse_strict_json;

/// Fixed, non-inference endpoint used by the controlled entitlement sync command.
pub const GROK_BUILD_SUBSCRIPTION_URL: &str =
    "https://cli-chat-proxy.grok.com/v1/user?include=subscription";

const MAX_SUBSCRIPTION_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_TOKEN_BYTES: usize = 64 * 1024;
const MAX_TIER_LABEL_BYTES: usize = 64;

/// Parses one successful Build user/subscription response.
///
/// The response must contain an explicit `subscriptionTier` either at the root or under `user`.
/// Unknown labels remain `grok_build/unknown`; absence is an error so callers may use the signed
/// token fallback without inventing a free plan.
///
/// # Errors
///
/// Returns a value-free error for a missing, oversized, malformed or unsafe observation.
pub fn grok_build_entitlement_from_subscription_response(
    input: &[u8],
    observed_at_ms: i64,
) -> Result<ProviderAccountEntitlement, GrokBuildEntitlementError> {
    if input.is_empty() || input.len() > MAX_SUBSCRIPTION_RESPONSE_BYTES {
        return Err(GrokBuildEntitlementError);
    }
    let value = parse_strict_json(input, MAX_SUBSCRIPTION_RESPONSE_BYTES)
        .map_err(|()| GrokBuildEntitlementError)?;
    let root = value.as_object().ok_or(GrokBuildEntitlementError)?;
    let root_tier = tier_field(root)?;
    let user_tier = match root.get("user") {
        None => None,
        Some(Value::Object(user)) => tier_field(user)?,
        Some(_) => return Err(GrokBuildEntitlementError),
    };
    let tier = match (root_tier, user_tier) {
        (Some(root), Some(user)) if root != user => return Err(GrokBuildEntitlementError),
        (Some(tier), _) | (_, Some(tier)) => tier,
        (None, None) => return Err(GrokBuildEntitlementError),
    };
    ProviderAccountEntitlement::try_new(
        tier,
        ProviderAccountEntitlementSource::ProviderSubscription,
        ProviderAccountEntitlementConfidence::Authoritative,
        observed_at_ms,
    )
    .map_err(|_| GrokBuildEntitlementError)
}

/// Derives the Build entitlement from the OAuth access token's bounded `tier` claim.
///
/// This is a fallback only. CPAR treats it as derived evidence and never upgrades its confidence
/// to authoritative merely because the token accompanied a successful inference.
///
/// # Errors
///
/// Returns a value-free error for a malformed/oversized token or absent/unsafe `tier` claim.
pub fn grok_build_entitlement_from_access_token(
    token: &str,
    observed_at_ms: i64,
) -> Result<ProviderAccountEntitlement, GrokBuildEntitlementError> {
    if token.is_empty() || token.len() > MAX_TOKEN_BYTES {
        return Err(GrokBuildEntitlementError);
    }
    let mut parts = token.split('.');
    let header = parts.next().ok_or(GrokBuildEntitlementError)?;
    let payload = parts.next().ok_or(GrokBuildEntitlementError)?;
    let signature = parts.next().ok_or(GrokBuildEntitlementError)?;
    if parts.next().is_some() || header.is_empty() || payload.is_empty() || signature.is_empty() {
        return Err(GrokBuildEntitlementError);
    }
    let payload = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| GrokBuildEntitlementError)?;
    if payload.len() > MAX_SUBSCRIPTION_RESPONSE_BYTES {
        return Err(GrokBuildEntitlementError);
    }
    let value = parse_strict_json(&payload, MAX_SUBSCRIPTION_RESPONSE_BYTES)
        .map_err(|()| GrokBuildEntitlementError)?;
    let claim = value
        .as_object()
        .and_then(|claims| claims.get("tier"))
        .ok_or(GrokBuildEntitlementError)?;
    let tier = match claim {
        Value::Number(value) => value
            .as_i64()
            .map(normalize_numeric_build_tier)
            .ok_or(GrokBuildEntitlementError)?,
        Value::String(value) => value.parse::<i64>().map_or_else(
            |_| normalize_build_tier(value),
            |value| Ok(normalize_numeric_build_tier(value)),
        )?,
        _ => return Err(GrokBuildEntitlementError),
    };
    ProviderAccountEntitlement::try_new(
        tier,
        ProviderAccountEntitlementSource::SignedToken,
        ProviderAccountEntitlementConfidence::Derived,
        observed_at_ms,
    )
    .map_err(|_| GrokBuildEntitlementError)
}

fn tier_field(
    values: &serde_json::Map<String, Value>,
) -> Result<Option<ProviderAccountEntitlementTier>, GrokBuildEntitlementError> {
    let camel = tier_value(values.get("subscriptionTier"))?;
    let snake = tier_value(values.get("subscription_tier"))?;
    match (camel, snake) {
        (Some(camel), Some(snake)) if camel != snake => Err(GrokBuildEntitlementError),
        (Some(tier), _) | (_, Some(tier)) => Ok(Some(tier)),
        (None, None) => Ok(None),
    }
}

fn tier_value(
    value: Option<&Value>,
) -> Result<Option<ProviderAccountEntitlementTier>, GrokBuildEntitlementError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.as_str().ok_or(GrokBuildEntitlementError)?.trim();
    if value.is_empty() {
        return Err(GrokBuildEntitlementError);
    }
    normalize_build_tier(value).map(Some)
}

fn normalize_build_tier(
    value: &str,
) -> Result<ProviderAccountEntitlementTier, GrokBuildEntitlementError> {
    if value.is_empty() || value.len() > MAX_TIER_LABEL_BYTES || !value.is_ascii() {
        return Err(GrokBuildEntitlementError);
    }
    let canonical = value
        .bytes()
        .filter(|byte| !matches!(byte, b' ' | b'_' | b'-'))
        .map(|byte| byte.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let canonical = std::str::from_utf8(&canonical).map_err(|_| GrokBuildEntitlementError)?;
    Ok(match canonical {
        "free" | "grokfree" => ProviderAccountEntitlementTier::GrokBuildFree,
        "grokpro" | "supergrok" | "supergrokpro" | "supergroklite" => {
            ProviderAccountEntitlementTier::GrokBuildSupergrok
        }
        "heavy" | "supergrokheavy" => ProviderAccountEntitlementTier::GrokBuildHeavy,
        _ => ProviderAccountEntitlementTier::GrokBuildUnknown,
    })
}

const fn normalize_numeric_build_tier(value: i64) -> ProviderAccountEntitlementTier {
    match value {
        0 => ProviderAccountEntitlementTier::GrokBuildFree,
        1 | 6 => ProviderAccountEntitlementTier::GrokBuildSupergrok,
        5 => ProviderAccountEntitlementTier::GrokBuildHeavy,
        _ => ProviderAccountEntitlementTier::GrokBuildUnknown,
    }
}

/// Value-free malformed or absent Build entitlement evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrokBuildEntitlementError;

impl fmt::Display for GrokBuildEntitlementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Grok Build entitlement evidence is invalid")
    }
}

impl Error for GrokBuildEntitlementError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt(claim: &str) -> String {
        let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"tier":{claim}}}"#));
        format!("header.{payload}.signature")
    }

    #[test]
    fn subscription_response_maps_only_build_vocabulary() {
        for (raw, expected) in [
            ("Free", ProviderAccountEntitlementTier::GrokBuildFree),
            (
                "GrokPro",
                ProviderAccountEntitlementTier::GrokBuildSupergrok,
            ),
            (
                "SuperGrokPro",
                ProviderAccountEntitlementTier::GrokBuildSupergrok,
            ),
            (
                "SuperGrok Heavy",
                ProviderAccountEntitlementTier::GrokBuildHeavy,
            ),
            (
                "X Premium Plus",
                ProviderAccountEntitlementTier::GrokBuildUnknown,
            ),
        ] {
            let body = format!(r#"{{"user":{{"subscriptionTier":"{raw}"}}}}"#);
            let entitlement = grok_build_entitlement_from_subscription_response(body.as_bytes(), 7)
                .unwrap_or_else(|_| std::process::abort());
            assert_eq!(entitlement.tier(), expected);
            assert_eq!(
                entitlement.source(),
                ProviderAccountEntitlementSource::ProviderSubscription
            );
        }
    }

    #[test]
    fn signed_token_numeric_mapping_is_derived_and_bounded() {
        for (raw, expected) in [
            ("0", ProviderAccountEntitlementTier::GrokBuildFree),
            ("1", ProviderAccountEntitlementTier::GrokBuildSupergrok),
            ("5", ProviderAccountEntitlementTier::GrokBuildHeavy),
            ("3", ProviderAccountEntitlementTier::GrokBuildUnknown),
        ] {
            let entitlement = grok_build_entitlement_from_access_token(&jwt(raw), 8)
                .unwrap_or_else(|_| std::process::abort());
            assert_eq!(entitlement.tier(), expected);
            assert_eq!(
                entitlement.confidence(),
                ProviderAccountEntitlementConfidence::Derived
            );
        }
    }

    #[test]
    fn absent_subscription_label_does_not_become_free() {
        assert!(grok_build_entitlement_from_subscription_response(b"{\"user\":{}}", 1).is_err());
    }

    #[test]
    fn conflicting_subscription_fields_fail_closed() {
        assert!(
            grok_build_entitlement_from_subscription_response(
                br#"{"subscriptionTier":"Free","user":{"subscriptionTier":"Heavy"}}"#,
                1,
            )
            .is_err()
        );
        assert!(
            grok_build_entitlement_from_subscription_response(
                br#"{"subscriptionTier":"Free","subscription_tier":"Heavy"}"#,
                1,
            )
            .is_err()
        );
    }

    #[test]
    fn token_fallback_requires_three_nonempty_segments() {
        for token in [".eyJ0aWVyIjoxfQ.signature", "header.eyJ0aWVyIjoxfQ."] {
            assert!(grok_build_entitlement_from_access_token(token, 1).is_err());
        }
    }
}
