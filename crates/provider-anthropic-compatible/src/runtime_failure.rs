//! Value-free Anthropic-compatible HTTP failure classification.

use std::time::Duration;

use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};
use serde_json::Value;

/// Sole state owner permitted for one classified failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnthropicRuntimeFailureAction {
    /// Preserve retained runtime state.
    None,
    /// Block only the exact Endpoint/Credential until reauthorization.
    RequireCredentialReauthorization,
    /// Record temporary quota only on the exact Endpoint/Credential.
    RecordExactQuota,
    /// Cool only the selected Endpoint.
    CoolEndpoint,
}

/// Secret-free Anthropic failure and optional reset delay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnthropicRuntimeFailureDisposition {
    error: GatewayError,
    action: AnthropicRuntimeFailureAction,
    retry_after: Option<Duration>,
}

impl AnthropicRuntimeFailureDisposition {
    /// Returns the stable client-safe error.
    #[must_use]
    pub const fn error(&self) -> &GatewayError {
        &self.error
    }
    /// Returns the sole permitted state action.
    #[must_use]
    pub const fn action(&self) -> AnthropicRuntimeFailureAction {
        self.action
    }
    /// Returns a positive integer reset delay when supplied unambiguously.
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }
}

/// Classifies status and a bounded structured Anthropic error envelope.
#[must_use]
pub fn classify_anthropic_runtime_failure(
    status: u16,
    body: &[u8],
    retry_after_seconds: Option<u64>,
) -> AnthropicRuntimeFailureDisposition {
    let kind = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/type")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let kind = kind.as_deref();
    let unauthorized = status == 401 || kind == Some("authentication_error");
    let quota = status == 429 || kind == Some("rate_limit_error");
    let forbidden = status == 403 || kind == Some("permission_error");
    let transient = matches!(status, 408 | 500..=599) || kind == Some("overloaded_error");
    let (code, scope, action) = if unauthorized {
        (
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
            AnthropicRuntimeFailureAction::RequireCredentialReauthorization,
        )
    } else if quota {
        (
            GatewayErrorCode::ProviderRateLimited,
            ErrorScope::QuotaWindow,
            AnthropicRuntimeFailureAction::RecordExactQuota,
        )
    } else if forbidden {
        (
            GatewayErrorCode::EgressRejected,
            ErrorScope::Egress,
            AnthropicRuntimeFailureAction::None,
        )
    } else if transient {
        (
            GatewayErrorCode::ProviderTransient,
            ErrorScope::Provider,
            AnthropicRuntimeFailureAction::CoolEndpoint,
        )
    } else {
        (
            GatewayErrorCode::ProviderPermanent,
            ErrorScope::Provider,
            AnthropicRuntimeFailureAction::None,
        )
    };
    AnthropicRuntimeFailureDisposition {
        error: GatewayError::new(code, scope),
        action,
        retry_after: retry_after_seconds
            .filter(|seconds| *seconds > 0)
            .map(Duration::from_secs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_error_types_keep_credential_quota_and_endpoint_distinct() {
        let auth = classify_anthropic_runtime_failure(
            400,
            br#"{"type":"error","error":{"type":"authentication_error"}}"#,
            None,
        );
        assert_eq!(
            auth.error().code(),
            GatewayErrorCode::CredentialUnauthorized
        );
        assert_eq!(
            auth.action(),
            AnthropicRuntimeFailureAction::RequireCredentialReauthorization
        );

        let quota = classify_anthropic_runtime_failure(
            429,
            br#"{"type":"error","error":{"type":"rate_limit_error"}}"#,
            Some(11),
        );
        assert_eq!(quota.error().code(), GatewayErrorCode::ProviderRateLimited);
        assert_eq!(quota.retry_after(), Some(Duration::from_secs(11)));

        let overloaded = classify_anthropic_runtime_failure(
            529,
            br#"{"type":"error","error":{"type":"overloaded_error"}}"#,
            None,
        );
        assert_eq!(
            overloaded.action(),
            AnthropicRuntimeFailureAction::CoolEndpoint
        );

        let permission = classify_anthropic_runtime_failure(
            400,
            br#"{"type":"error","error":{"type":"permission_error"}}"#,
            None,
        );
        assert_eq!(permission.error().code(), GatewayErrorCode::EgressRejected);
        assert_eq!(permission.action(), AnthropicRuntimeFailureAction::None);
    }

    #[test]
    fn malformed_prose_cannot_invent_a_state_owner() {
        let failure = classify_anthropic_runtime_failure(400, b"authentication_error", None);
        assert_eq!(failure.error().code(), GatewayErrorCode::ProviderPermanent);
        assert_eq!(failure.action(), AnthropicRuntimeFailureAction::None);
    }
}
