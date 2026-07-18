//! Transport-neutral gateway error categories and remediation scopes.

use std::{error::Error, fmt};

/// Stable internal error category used across gateway layers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    /// An upstream response violated its declared protocol.
    UpstreamProtocolError,
    /// A stream ended before its required semantic sequence completed.
    StreamTruncated,
    /// The gateway encountered an unexpected internal failure.
    InternalError,
    /// The request was cancelled before completion.
    Cancelled,
}

impl GatewayErrorCode {
    /// All frozen gateway error categories in stable snapshot order.
    pub const ALL: [Self; 16] = [
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
        Self::UpstreamProtocolError,
        Self::StreamTruncated,
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
            Self::UpstreamProtocolError => "UpstreamProtocolError",
            Self::StreamTruncated => "StreamTruncated",
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayError {
    code: GatewayErrorCode,
    scope: ErrorScope,
}

impl GatewayError {
    /// Creates an error from an explicit category and remediation scope.
    ///
    /// The core type deliberately accepts no caller-supplied diagnostic text, so it cannot retain
    /// credentials, tokens, request bodies, or raw upstream responses.
    #[must_use]
    pub const fn new(code: GatewayErrorCode, scope: ErrorScope) -> Self {
        Self { code, scope }
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
            GatewayErrorCode::UpstreamProtocolError => "the upstream protocol was invalid",
            GatewayErrorCode::StreamTruncated => "the stream ended before completion",
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
    use super::{ErrorScope, GatewayError, GatewayErrorCode};

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
}
