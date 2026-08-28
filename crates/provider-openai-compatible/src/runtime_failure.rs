//! Value-free Codex and generic OpenAI-compatible HTTP failure classification.

use std::time::Duration;

use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};
use serde_json::Value;

/// The only runtime owner permitted to react to one classified failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAiRuntimeFailureAction {
    /// No retained runtime state may change.
    None,
    /// This exact credential requires explicit reauthorization or replacement.
    RequireCredentialReauthorization,
    /// Record a temporary quota block only for this Endpoint/Credential binding.
    RecordExactQuota,
    /// Cool only the selected Endpoint.
    CoolEndpoint,
}

/// Secret-free failure classification and bounded retry timing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiRuntimeFailureDisposition {
    error: GatewayError,
    action: OpenAiRuntimeFailureAction,
    retry_after: Option<Duration>,
}

impl OpenAiRuntimeFailureDisposition {
    /// Returns the public-safe transport-neutral error.
    #[must_use]
    pub const fn error(&self) -> &GatewayError {
        &self.error
    }
    /// Returns the sole permitted runtime state owner.
    #[must_use]
    pub const fn action(&self) -> OpenAiRuntimeFailureAction {
        self.action
    }
    /// Returns bounded upstream reset timing when it was unambiguous.
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }
}

/// Classifies an HTTP failure using status plus a bounded, never-retained structured body.
///
/// Codex `usage_limit_reached` may arrive as a 4xx other than 429; it is still quota-owned. Raw
/// messages are deliberately ignored so arbitrary prose cannot mutate Credential state.
#[must_use]
pub fn classify_openai_runtime_failure(
    status: u16,
    body: &[u8],
    retry_after_seconds: Option<u64>,
    now_epoch_seconds: u64,
) -> OpenAiRuntimeFailureDisposition {
    let structured = serde_json::from_slice::<Value>(body).ok();
    let quota_signal = structured.as_ref().is_some_and(|value| {
        ["/error/type", "/error/code", "/type"]
            .iter()
            .filter_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
            .any(|value| value.eq_ignore_ascii_case("usage_limit_reached"))
    });
    let quota = status == 429 || quota_signal;
    let retry_after = retry_after_seconds
        .or_else(|| {
            structured
                .as_ref()
                .and_then(|value| reset_delay(value, now_epoch_seconds))
        })
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs);
    let (error_code, scope, action) = if quota {
        (
            GatewayErrorCode::ProviderRateLimited,
            ErrorScope::QuotaWindow,
            OpenAiRuntimeFailureAction::RecordExactQuota,
        )
    } else {
        match status {
            401 => (
                GatewayErrorCode::CredentialUnauthorized,
                ErrorScope::Credential,
                OpenAiRuntimeFailureAction::RequireCredentialReauthorization,
            ),
            // An unknown 403 is not proof that the key/account itself is permanently forbidden.
            403 => (
                GatewayErrorCode::EgressRejected,
                ErrorScope::Egress,
                OpenAiRuntimeFailureAction::None,
            ),
            408 | 500..=599 => (
                GatewayErrorCode::ProviderTransient,
                ErrorScope::Provider,
                OpenAiRuntimeFailureAction::CoolEndpoint,
            ),
            _ => (
                GatewayErrorCode::ProviderPermanent,
                ErrorScope::Provider,
                OpenAiRuntimeFailureAction::None,
            ),
        }
    };
    OpenAiRuntimeFailureDisposition {
        error: GatewayError::new(error_code, scope),
        action,
        retry_after,
    }
}

fn reset_delay(value: &Value, now_epoch_seconds: u64) -> Option<u64> {
    value
        .pointer("/error/resets_in_seconds")
        .and_then(Value::as_u64)
        .or_else(|| {
            value
                .pointer("/error/resets_at")
                .and_then(Value::as_u64)
                .map(|reset| reset.saturating_sub(now_epoch_seconds))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_and_structured_quota_signals_keep_distinct_owners() {
        let unauthorized = classify_openai_runtime_failure(401, b"{}", None, 100);
        assert_eq!(
            unauthorized.error().code(),
            GatewayErrorCode::CredentialUnauthorized
        );
        assert_eq!(
            unauthorized.action(),
            OpenAiRuntimeFailureAction::RequireCredentialReauthorization
        );

        let forbidden = classify_openai_runtime_failure(403, b"{}", None, 100);
        assert_eq!(forbidden.error().code(), GatewayErrorCode::EgressRejected);
        assert_eq!(forbidden.action(), OpenAiRuntimeFailureAction::None);

        let quota = classify_openai_runtime_failure(
            400,
            br#"{"error":{"type":"usage_limit_reached","resets_in_seconds":30}}"#,
            None,
            100,
        );
        assert_eq!(quota.error().code(), GatewayErrorCode::ProviderRateLimited);
        assert_eq!(quota.action(), OpenAiRuntimeFailureAction::RecordExactQuota);
        assert_eq!(quota.retry_after(), Some(Duration::from_secs(30)));
    }

    #[test]
    fn malformed_body_cannot_invent_credential_or_quota_state() {
        let failure = classify_openai_runtime_failure(400, b"usage_limit_reached", None, 0);
        assert_eq!(failure.error().code(), GatewayErrorCode::ProviderPermanent);
        assert_eq!(failure.action(), OpenAiRuntimeFailureAction::None);
    }
}
