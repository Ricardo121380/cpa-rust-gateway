//! Transport-neutral gateway error categories and remediation scopes.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

/// Closed, value-free rejection at the client-protocol response projection stage.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseProjectionRejection {
    /// Structured reasoning has no equivalent target representation.
    ReasoningUnsupported,
    /// Retained response metadata has no reviewed target representation.
    UnknownExtensions,
    /// The target cannot represent the successful stop reason.
    StopReasonUnsupported,
    /// The target cannot retain the stop sequence.
    StopSequenceUnsupported,
    /// Projection violated the canonical lifecycle.
    InvalidCanonicalLifecycle,
    /// The response violated declared tool constraints.
    ToolConstraintViolated,
}

impl fmt::Display for ResponseProjectionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl Error for ResponseProjectionRejection {}

/// Closed, value-free reason for a request rejected by the gateway before dispatch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestRejection {
    /// All applicable conversions reject the Messages overall effort control.
    MessagesEffortUnsupported,
}

/// Closed reasons for rejecting an unproven Build model identity, without native values.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelIdentityRejection {
    /// The admitted public model cannot be bound to the selected upstream before dispatch.
    RequestMismatch,
    /// An applicable native response snapshot omitted its model.
    MissingNativeModel,
    /// The native model has an invalid type or bounded identifier shape.
    InvalidNativeModel,
    /// The native model is not the selected model or an attested alias.
    NativeModelMismatch,
    /// A later response snapshot changed the native model.
    ResponseModelConflict,
    /// A supplied frozen alias proof does not cover this execution.
    AliasNotApplicable,
}

/// Stable internal error category used across gateway layers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum GatewayErrorCode {
    /// The client supplied an invalid request or tool schema.
    ClientRequestError,
    /// The client did not supply valid gateway authentication.
    ClientUnauthorized,
    /// No configured route can serve the requested public model.
    RouteNotFound,
    /// No usable credential is available for the selected endpoint.
    CredentialUnavailable,
    /// A credential requires reauthorization before it can be scheduled again.
    CredentialUnauthorized,
    /// Evidence proves that a credential is forbidden.
    CredentialForbidden,
    /// A credential has exhausted a tracked quota window.
    CredentialQuotaExceeded,
    /// The selected network egress or session was rejected.
    EgressRejected,
    /// The required network egress is unavailable.
    EgressUnavailable,
    /// A provider applied a retryable rate limit.
    ProviderRateLimited,
    /// A provider failed transiently before a terminal semantic event.
    ProviderTransient,
    /// A provider returned a non-retryable failure.
    ProviderPermanent,
    /// The selected route or input is outside the supported token-count capability.
    TokenCountUnsupported,
    /// An upstream response violated its declared protocol.
    UpstreamProtocolError,
    /// A stream ended before its required semantic sequence completed.
    StreamTruncated,
    /// Required request observations cannot be durably confirmed.
    RecordingUnavailable,
    /// The gateway encountered an unexpected internal failure.
    InternalError,
    /// The request was cancelled before completion.
    Cancelled,
}

impl GatewayErrorCode {
    /// All frozen gateway error categories in stable snapshot order.
    pub const ALL: [Self; 18] = [
        Self::ClientRequestError,
        Self::ClientUnauthorized,
        Self::RouteNotFound,
        Self::CredentialUnavailable,
        Self::CredentialUnauthorized,
        Self::CredentialForbidden,
        Self::CredentialQuotaExceeded,
        Self::EgressRejected,
        Self::EgressUnavailable,
        Self::ProviderRateLimited,
        Self::ProviderTransient,
        Self::ProviderPermanent,
        Self::TokenCountUnsupported,
        Self::UpstreamProtocolError,
        Self::StreamTruncated,
        Self::RecordingUnavailable,
        Self::InternalError,
        Self::Cancelled,
    ];

    /// Returns the stable machine-readable encoding for this category.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClientRequestError => "ClientRequestError",
            Self::ClientUnauthorized => "ClientUnauthorized",
            Self::RouteNotFound => "RouteNotFound",
            Self::CredentialUnavailable => "CredentialUnavailable",
            Self::CredentialUnauthorized => "CredentialUnauthorized",
            Self::CredentialForbidden => "CredentialForbidden",
            Self::CredentialQuotaExceeded => "CredentialQuotaExceeded",
            Self::EgressRejected => "EgressRejected",
            Self::EgressUnavailable => "EgressUnavailable",
            Self::ProviderRateLimited => "ProviderRateLimited",
            Self::ProviderTransient => "ProviderTransient",
            Self::ProviderPermanent => "ProviderPermanent",
            Self::TokenCountUnsupported => "TokenCountUnsupported",
            Self::UpstreamProtocolError => "UpstreamProtocolError",
            Self::StreamTruncated => "StreamTruncated",
            Self::RecordingUnavailable => "RecordingUnavailable",
            Self::InternalError => "InternalError",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl fmt::Display for GatewayErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// State owner whose remediation may be affected by a gateway error.
///
/// Error category and scope are deliberately independent: the same observed transport failure can
/// be attributable to an egress session or a provider only after bounded evidence is evaluated.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorScope {
    /// The external request itself cannot continue without a client-side change.
    Request,
    /// One encrypted credential requires attention or reauthorization.
    Credential,
    /// One upstream account has account-level state that requires attention.
    Account,
    /// One model capability or entitlement is affected.
    Model,
    /// One tracked quota window is exhausted or awaiting a controlled probe.
    QuotaWindow,
    /// One stateful egress session is affected and may need to be rebuilt.
    EgressSession,
    /// One egress path is affected.
    Egress,
    /// A provider implementation or upstream service is affected.
    Provider,
    /// A response stream has become invalid after it started.
    Stream,
    /// The failure belongs to gateway-internal state rather than an upstream owner.
    Internal,
}

impl ErrorScope {
    /// Returns the stable machine-readable encoding for this remediation scope.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Credential => "credential",
            Self::Account => "account",
            Self::Model => "model",
            Self::QuotaWindow => "quota_window",
            Self::EgressSession => "egress_session",
            Self::Egress => "egress",
            Self::Provider => "provider",
            Self::Stream => "stream",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for ErrorScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A safe, transport-neutral error returned by a gateway domain operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayError {
    code: GatewayErrorCode,
    scope: ErrorScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    projection_rejection: Option<ResponseProjectionRejection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_identity_rejection: Option<ModelIdentityRejection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request_rejection: Option<RequestRejection>,
}

impl GatewayError {
    /// Creates an error from an explicit category and remediation scope.
    ///
    /// The core type deliberately accepts no caller-supplied diagnostic text, so it cannot retain
    /// credentials, tokens, request bodies, or raw upstream responses.
    #[must_use]
    pub const fn new(code: GatewayErrorCode, scope: ErrorScope) -> Self {
        Self {
            code,
            scope,
            projection_rejection: None,
            model_identity_rejection: None,
            request_rejection: None,
        }
    }

    /// Retains a closed rejection without admitting arbitrary diagnostic text.
    #[must_use]
    pub const fn response_projection(reason: ResponseProjectionRejection) -> Self {
        Self {
            code: GatewayErrorCode::UpstreamProtocolError,
            scope: ErrorScope::Stream,
            projection_rejection: Some(reason),
            model_identity_rejection: None,
            request_rejection: None,
        }
    }

    /// Creates a non-retryable, value-free model admission or native response failure.
    #[must_use]
    pub const fn model_identity(reason: ModelIdentityRejection) -> Self {
        let (code, scope) = match reason {
            ModelIdentityRejection::RequestMismatch
            | ModelIdentityRejection::AliasNotApplicable => {
                (GatewayErrorCode::ClientRequestError, ErrorScope::Request)
            }
            _ => (
                GatewayErrorCode::UpstreamProtocolError,
                ErrorScope::Provider,
            ),
        };
        Self {
            code,
            scope,
            projection_rejection: None,
            model_identity_rejection: Some(reason),
            request_rejection: None,
        }
    }

    /// Reports only a proven local conversion rejection, never an upstream error.
    #[must_use]
    pub const fn messages_effort_unsupported() -> Self {
        Self {
            code: GatewayErrorCode::ClientRequestError,
            scope: ErrorScope::Request,
            projection_rejection: None,
            model_identity_rejection: None,
            request_rejection: Some(RequestRejection::MessagesEffortUnsupported),
        }
    }

    /// Returns a closed model-identity reason, without exposing any model or proof content.
    #[must_use]
    pub const fn model_identity_rejection(&self) -> Option<ModelIdentityRejection> {
        self.model_identity_rejection
    }

    /// Returns the specific response-projection reason, when known.
    #[must_use]
    pub const fn projection_rejection(&self) -> Option<ResponseProjectionRejection> {
        self.projection_rejection
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn code(&self) -> GatewayErrorCode {
        self.code
    }

    /// Returns the owner whose state may require remediation.
    #[must_use]
    pub const fn scope(&self) -> ErrorScope {
        self.scope
    }

    /// Returns the fixed, secret-free diagnostic message for the error category.
    #[must_use]
    pub const fn safe_message(&self) -> &'static str {
        if let Some(RequestRejection::MessagesEffortUnsupported) = self.request_rejection {
            return "output_config.effort: Extra inputs are not permitted by the current gateway conversion; no upstream request was sent";
        }
        if let Some(reason) = self.model_identity_rejection {
            return match reason {
                ModelIdentityRejection::RequestMismatch => {
                    "model identity rejected before dispatch: public and upstream models are unproven"
                }
                ModelIdentityRejection::MissingNativeModel => {
                    "model identity rejected: native model is missing"
                }
                ModelIdentityRejection::InvalidNativeModel => {
                    "model identity rejected: native model has an invalid shape"
                }
                ModelIdentityRejection::NativeModelMismatch => {
                    "model identity rejected: native and upstream models are unproven"
                }
                ModelIdentityRejection::ResponseModelConflict => {
                    "model identity rejected: native response phases contradict"
                }
                ModelIdentityRejection::AliasNotApplicable => {
                    "model identity rejected before dispatch: alias proof is not applicable"
                }
            };
        }
        if let Some(reason) = self.projection_rejection {
            return match reason {
                ResponseProjectionRejection::ReasoningUnsupported => {
                    "response projection cannot represent the reasoning metadata"
                }
                ResponseProjectionRejection::UnknownExtensions => {
                    "response projection cannot represent the response metadata"
                }
                ResponseProjectionRejection::StopReasonUnsupported => {
                    "response projection cannot represent the stop reason"
                }
                ResponseProjectionRejection::StopSequenceUnsupported => {
                    "response projection cannot represent the stop sequence"
                }
                ResponseProjectionRejection::InvalidCanonicalLifecycle => {
                    "response projection rejected an invalid canonical lifecycle"
                }
                ResponseProjectionRejection::ToolConstraintViolated => {
                    "response projection rejected a tool constraint violation"
                }
            };
        }
        match self.code {
            GatewayErrorCode::ClientRequestError => "the client request is invalid",
            GatewayErrorCode::ClientUnauthorized => "the client is not authorized",
            GatewayErrorCode::RouteNotFound => "no route can serve this request",
            GatewayErrorCode::CredentialUnavailable => "no credential is available",
            GatewayErrorCode::CredentialUnauthorized => "the credential requires authorization",
            GatewayErrorCode::CredentialForbidden => "the credential is forbidden",
            GatewayErrorCode::CredentialQuotaExceeded => "the credential quota is exhausted",
            GatewayErrorCode::EgressRejected => "the egress path was rejected",
            GatewayErrorCode::EgressUnavailable => "the egress path is unavailable",
            GatewayErrorCode::ProviderRateLimited => "the provider rate limited the request",
            GatewayErrorCode::ProviderTransient => "the provider failed transiently",
            GatewayErrorCode::ProviderPermanent => "the provider returned a permanent failure",
            GatewayErrorCode::TokenCountUnsupported => {
                "token counting is unsupported for the selected route or input"
            }
            GatewayErrorCode::UpstreamProtocolError => "the upstream protocol was invalid",
            GatewayErrorCode::StreamTruncated => "the stream ended before completion",
            GatewayErrorCode::RecordingUnavailable => "request recording is unavailable",
            GatewayErrorCode::InternalError => "the gateway encountered an internal error",
            GatewayErrorCode::Cancelled => "the request was cancelled",
        }
    }
}

impl fmt::Display for GatewayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({})", self.code, self.scope)
    }
}

impl Error for GatewayError {}

#[cfg(test)]
mod tests {
    use super::{ErrorScope, GatewayError, GatewayErrorCode, ResponseProjectionRejection};

    #[test]
    fn gateway_error_retains_an_explicit_scope() {
        let error = GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress);

        assert_eq!(error.code(), GatewayErrorCode::EgressRejected);
        assert_eq!(error.scope(), ErrorScope::Egress);
        assert_eq!(error.safe_message(), "the egress path was rejected");
    }

    #[test]
    fn egress_rejection_does_not_imply_credential_forbidden() {
        let egress_rejected =
            GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress);
        let credential_forbidden = GatewayError::new(
            GatewayErrorCode::CredentialForbidden,
            ErrorScope::Credential,
        );

        assert_ne!(egress_rejected.code(), credential_forbidden.code());
        assert_ne!(egress_rejected.scope(), credential_forbidden.scope());
    }

    #[test]
    fn gateway_error_code_snapshot_is_stable() {
        let mut actual = String::new();
        for code in GatewayErrorCode::ALL {
            actual.push_str(code.as_str());
            actual.push('\n');
        }

        assert_eq!(
            actual,
            include_str!("../../../tests/fixtures/core/gateway-error-codes.snap")
        );
    }

    #[test]
    fn gateway_error_scopes_remain_distinct() {
        assert_ne!(ErrorScope::Credential, ErrorScope::Account);
        assert_ne!(ErrorScope::Account, ErrorScope::Model);
        assert_ne!(ErrorScope::Account, ErrorScope::QuotaWindow);
        assert_ne!(ErrorScope::EgressSession, ErrorScope::Egress);
        assert_ne!(ErrorScope::Egress, ErrorScope::Provider);
        assert_ne!(ErrorScope::Provider, ErrorScope::Stream);
    }

    #[test]
    fn gateway_error_json_uses_the_stable_machine_encodings() -> Result<(), serde_json::Error> {
        let error = GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider);
        let encoded = serde_json::to_string(&error)?;
        let decoded: GatewayError = serde_json::from_str(&encoded)?;

        assert_eq!(
            encoded,
            r#"{"code":"ProviderTransient","scope":"provider"}"#
        );
        assert_eq!(decoded, error);

        Ok(())
    }

    #[test]
    fn projection_error_retains_only_closed_reason_and_stage() -> Result<(), serde_json::Error> {
        let error =
            GatewayError::response_projection(ResponseProjectionRejection::ReasoningUnsupported);
        assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
        assert_eq!(error.scope(), ErrorScope::Stream);
        assert_eq!(
            error.projection_rejection(),
            Some(ResponseProjectionRejection::ReasoningUnsupported)
        );
        assert_eq!(
            error.safe_message(),
            "response projection cannot represent the reasoning metadata"
        );
        let encoded = serde_json::to_string(&error)?;
        assert_eq!(
            encoded,
            r#"{"code":"UpstreamProtocolError","scope":"stream","projection_rejection":"reasoning_unsupported"}"#
        );
        assert_eq!(serde_json::from_str::<GatewayError>(&encoded)?, error);
        assert!(serde_json::from_str::<GatewayError>(r#"{"code":"UpstreamProtocolError","scope":"stream","projection_rejection":"private response content"}"#).is_err());
        let old: GatewayError =
            serde_json::from_str(r#"{"code":"UpstreamProtocolError","scope":"stream"}"#)?;
        assert_eq!(old.projection_rejection(), None);
        assert_eq!(old.safe_message(), "the upstream protocol was invalid");
        Ok(())
    }
}
