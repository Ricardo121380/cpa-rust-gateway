//! Kiro network and HTTP failure classification without retained response values.
//!
//! The eventual transport maps only already-recognized syntax into [`KiroFailureSignal`] or a
//! concrete [`KiroNetworkFailure`]. This module then selects the sole owner that a caller may
//! cool or disable. It deliberately does not parse raw error bodies, retain headers, mutate a
//! Credential, or retry a request.

use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};

/// A value-free network failure observed before a Kiro response is semantically usable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroNetworkFailure {
    /// DNS resolution could not produce an admitted address.
    Dns,
    /// A TCP connection could not be established.
    Connect,
    /// TLS negotiation failed before any response semantics.
    TlsHandshake,
    /// No first byte arrived before the request's bounded deadline.
    FirstByteTimeout,
    /// A response stream ended after semantic delivery began.
    StreamInterrupted,
}

/// A safe, bounded Kiro signal extracted by a future HTTP/EventStream boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroFailureSignal {
    /// No independent provider-specific evidence was recognized.
    None,
    /// The Credential's authentication is invalid or expired.
    CredentialUnauthorized,
    /// Independent account evidence proves the current account is forbidden.
    AccountForbidden,
    /// The selected model is unavailable to this exact Credential.
    ModelUnavailable,
    /// A concrete Kiro quota window is exhausted.
    QuotaExhausted,
    /// A 429 is independently attached to the current Kiro account.
    AccountRateLimited,
    /// A 429 is independently attached to the Kiro provider service.
    ProviderRateLimited,
}

/// The only state transition class that Kiro failure classification permits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroFailureAction {
    /// Retain all Provider runtime state.
    None,
    /// Remove this Credential from scheduling until a later explicit reauthorization succeeds.
    RequireReauthorization,
    /// Mark only the independently evidenced account forbidden.
    MarkAccountForbidden,
    /// Cool only the selected model capability.
    CoolModel,
    /// Cool the exact exhausted quota window until its separate recovery policy permits a probe.
    CoolQuotaWindow,
    /// Cool the current account without disabling other accounts or the Provider.
    CoolAccount,
    /// Cool the Kiro Provider without mutating account or Credential state.
    CoolProvider,
    /// Rebuild or replace only the affected egress path/session.
    RebuildEgress,
    /// End the already-visible stream; transparent retry is not permitted.
    TerminateStream,
}

/// A transport-neutral Kiro failure plus its single permitted remediation owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KiroFailureDisposition {
    error: GatewayError,
    action: KiroFailureAction,
}

impl KiroFailureDisposition {
    /// Returns the stable public-safe gateway error.
    #[must_use]
    pub const fn error(&self) -> &GatewayError {
        &self.error
    }

    /// Returns the only state transition category that a later runtime may perform.
    #[must_use]
    pub const fn action(&self) -> KiroFailureAction {
        self.action
    }
}

/// Classifies a Kiro network failure before any semantic response event exists.
#[must_use]
pub const fn classify_kiro_network_failure(failure: KiroNetworkFailure) -> KiroFailureDisposition {
    let (code, scope, action) = match failure {
        KiroNetworkFailure::Dns
        | KiroNetworkFailure::Connect
        | KiroNetworkFailure::TlsHandshake
        | KiroNetworkFailure::FirstByteTimeout => (
            GatewayErrorCode::EgressUnavailable,
            ErrorScope::Egress,
            KiroFailureAction::RebuildEgress,
        ),
        KiroNetworkFailure::StreamInterrupted => (
            GatewayErrorCode::StreamTruncated,
            ErrorScope::Stream,
            KiroFailureAction::TerminateStream,
        ),
    };
    KiroFailureDisposition {
        error: GatewayError::new(code, scope),
        action,
    }
}

/// Classifies a Kiro HTTP status plus only independently established, safe evidence.
///
/// Provider-specific evidence has precedence over a generic status. In particular, unknown 403
/// never disables a Credential, unknown 429 stays Provider-scoped, and a model signal affects
/// only that model. A caller must not manufacture a signal from raw response text at this boundary.
#[must_use]
pub const fn classify_kiro_http_failure(
    status: u16,
    signal: KiroFailureSignal,
) -> KiroFailureDisposition {
    let (code, scope, action) = match signal {
        KiroFailureSignal::CredentialUnauthorized => (
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
            KiroFailureAction::RequireReauthorization,
        ),
        KiroFailureSignal::AccountForbidden => (
            GatewayErrorCode::CredentialForbidden,
            ErrorScope::Account,
            KiroFailureAction::MarkAccountForbidden,
        ),
        KiroFailureSignal::ModelUnavailable => (
            GatewayErrorCode::RouteNotFound,
            ErrorScope::Model,
            KiroFailureAction::CoolModel,
        ),
        KiroFailureSignal::QuotaExhausted => (
            GatewayErrorCode::CredentialQuotaExceeded,
            ErrorScope::QuotaWindow,
            KiroFailureAction::CoolQuotaWindow,
        ),
        KiroFailureSignal::AccountRateLimited => (
            GatewayErrorCode::ProviderRateLimited,
            ErrorScope::Account,
            KiroFailureAction::CoolAccount,
        ),
        KiroFailureSignal::ProviderRateLimited => (
            GatewayErrorCode::ProviderRateLimited,
            ErrorScope::Provider,
            KiroFailureAction::CoolProvider,
        ),
        KiroFailureSignal::None => match status {
            401 => (
                GatewayErrorCode::CredentialUnauthorized,
                ErrorScope::Credential,
                KiroFailureAction::RequireReauthorization,
            ),
            403 => (
                GatewayErrorCode::EgressRejected,
                ErrorScope::Egress,
                KiroFailureAction::None,
            ),
            404 => (
                GatewayErrorCode::RouteNotFound,
                ErrorScope::Model,
                KiroFailureAction::CoolModel,
            ),
            408 | 500..=599 => (
                GatewayErrorCode::ProviderTransient,
                ErrorScope::Provider,
                KiroFailureAction::CoolProvider,
            ),
            429 => (
                GatewayErrorCode::ProviderRateLimited,
                ErrorScope::Provider,
                KiroFailureAction::CoolProvider,
            ),
            _ => (
                GatewayErrorCode::ProviderPermanent,
                ErrorScope::Provider,
                KiroFailureAction::None,
            ),
        },
    };
    KiroFailureDisposition {
        error: GatewayError::new(code, scope),
        action,
    }
}
