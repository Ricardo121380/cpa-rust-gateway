//! Grok Web 403 attribution with exact egress and account-state isolation.
//!
//! A generic Web 403 is egress-local. Only separately established, exact account evidence can
//! produce a credential-forbidden disposition. This module contains no HTTP/body parser, browser,
//! Cookie, network, transport, scheduler, or persistent state operation.

use std::{error::Error, fmt};

use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};

use crate::GrokWebBrowserEgressSession;

/// The only independently established account evidence accepted by the Web 403 classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebAccountEvidence {
    /// No account-level evidence is available; an HTTP status alone cannot disable an account.
    None,
    /// A separately validated source proved that this exact active account is forbidden.
    ConfirmedForbidden,
}

/// The sole local remediation owner selected for a Web HTTP failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebFailureAction {
    /// Preserve local account and egress-session state.
    None,
    /// Require an explicit reauthorization of the exact credential lifecycle.
    RequireReauthorization,
    /// Rebuild or rotate only the exact rejected browser egress session.
    RebuildEgressSession,
    /// Mark only the independently evidenced account lifecycle unavailable.
    MarkExactAccountForbidden,
    /// Cool the Web provider without changing account or egress-session identity.
    CoolProvider,
}

/// A value-free Web failure and its one permitted remediation owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokWebFailureDisposition {
    error: GatewayError,
    action: GrokWebFailureAction,
}

impl GrokWebFailureDisposition {
    /// Returns the safe public error classification.
    #[must_use]
    pub const fn error(&self) -> &GatewayError {
        &self.error
    }

    /// Returns the only state owner allowed to react to this failure.
    #[must_use]
    pub const fn action(&self) -> GrokWebFailureAction {
        self.action
    }
}

/// Classifies one Web HTTP status with only already-established account evidence.
///
/// # Errors
///
/// Fails closed when the status is outside the HTTP range or account-forbidden evidence is not
/// attached to a 403. It does not parse a body or mutate state.
pub fn classify_grok_web_http_failure(
    status: u16,
    account_evidence: GrokWebAccountEvidence,
) -> Result<GrokWebFailureDisposition, GrokWebFailureError> {
    if !(100..=599).contains(&status) {
        return Err(GrokWebFailureError::InvalidHttpStatus);
    }
    if account_evidence == GrokWebAccountEvidence::ConfirmedForbidden && status != 403 {
        return Err(GrokWebFailureError::InvalidAccountEvidence);
    }
    let (code, scope, action) = match status {
        401 => (
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
            GrokWebFailureAction::RequireReauthorization,
        ),
        403 if account_evidence == GrokWebAccountEvidence::ConfirmedForbidden => (
            GatewayErrorCode::CredentialForbidden,
            ErrorScope::Account,
            GrokWebFailureAction::MarkExactAccountForbidden,
        ),
        403 => (
            GatewayErrorCode::EgressRejected,
            ErrorScope::Egress,
            GrokWebFailureAction::RebuildEgressSession,
        ),
        429 => (
            GatewayErrorCode::ProviderRateLimited,
            ErrorScope::Provider,
            GrokWebFailureAction::CoolProvider,
        ),
        408 | 500..=599 => (
            GatewayErrorCode::ProviderTransient,
            ErrorScope::Provider,
            GrokWebFailureAction::CoolProvider,
        ),
        _ => (
            GatewayErrorCode::ProviderPermanent,
            ErrorScope::Provider,
            GrokWebFailureAction::None,
        ),
    };
    Ok(GrokWebFailureDisposition {
        error: GatewayError::new(code, scope),
        action,
    })
}

/// Availability projection for one exact browser egress session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebEgressAvailability {
    /// No egress-local rejection has been recorded for this exact session lifecycle.
    Available,
    /// A generic 403 requires only this browser egress session to be rebuilt or rotated.
    Rejected,
}

/// Local egress rejection state bound to one exact Web credential and egress session.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebEgressFailureState {
    binding: GrokWebAccountBinding,
    egress_session_id: String,
    availability: GrokWebEgressAvailability,
}

impl GrokWebEgressFailureState {
    /// Starts with an available, unexpired exact Web egress-session binding.
    ///
    /// # Errors
    ///
    /// Returns a value-free error for negative time or an expired session.
    pub fn try_new(
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<Self, GrokWebFailureStateError> {
        let binding = GrokWebAccountBinding::from_session(session, now_ms)?;
        Ok(Self {
            binding,
            egress_session_id: session.egress_session_id().as_str().to_owned(),
            availability: GrokWebEgressAvailability::Available,
        })
    }

    /// Returns whether this exact session is locally available for a later request boundary.
    #[must_use]
    pub const fn availability(&self) -> GrokWebEgressAvailability {
        self.availability
    }

    /// Records only an egress-owned generic-403 disposition for this exact session.
    ///
    /// # Errors
    ///
    /// Rejects a mismatched/expired session or every non-egress disposition without mutation.
    pub fn observe_egress_rejection(
        &mut self,
        session: &GrokWebBrowserEgressSession,
        disposition: &GrokWebFailureDisposition,
        now_ms: i64,
    ) -> Result<(), GrokWebFailureStateError> {
        self.require_current_session(session, now_ms)?;
        if disposition.action() != GrokWebFailureAction::RebuildEgressSession {
            return Err(GrokWebFailureStateError::InvalidEgressAction);
        }
        self.availability = GrokWebEgressAvailability::Rejected;
        Ok(())
    }

    /// Ensures a later local request boundary cannot reuse a rejected egress session.
    ///
    /// # Errors
    ///
    /// Returns a safe binding, expiry, or egress-rejected category without changing state.
    pub fn require_available(
        &self,
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<(), GrokWebFailureStateError> {
        self.require_current_session(session, now_ms)?;
        if self.availability == GrokWebEgressAvailability::Rejected {
            return Err(GrokWebFailureStateError::EgressRejected);
        }
        Ok(())
    }

    fn require_current_session(
        &self,
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<(), GrokWebFailureStateError> {
        self.binding.require_current_session(session, now_ms)?;
        if self.egress_session_id != session.egress_session_id().as_str() {
            return Err(GrokWebFailureStateError::SessionBindingMismatch);
        }
        Ok(())
    }
}

impl fmt::Debug for GrokWebEgressFailureState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebEgressFailureState")
            .field("binding", &self.binding)
            .field("egress_session_id", &"<redacted>")
            .field("availability", &self.availability)
            .finish()
    }
}

/// Availability projection for one exact Web account credential lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebAccountAvailability {
    /// No separately validated account-forbidden evidence has been recorded.
    Available,
    /// Exact account evidence made this credential lifecycle unavailable.
    Forbidden,
}

/// Local account-forbidden state independent from a browser egress session identity.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebAccountFailureState {
    binding: GrokWebAccountBinding,
    availability: GrokWebAccountAvailability,
}

impl GrokWebAccountFailureState {
    /// Starts with an available, unexpired exact Web account credential lifecycle.
    ///
    /// # Errors
    ///
    /// Returns a value-free error for negative time or an expired session.
    pub fn try_new(
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<Self, GrokWebFailureStateError> {
        Ok(Self {
            binding: GrokWebAccountBinding::from_session(session, now_ms)?,
            availability: GrokWebAccountAvailability::Available,
        })
    }

    /// Returns the safe account availability projection.
    #[must_use]
    pub const fn availability(&self) -> GrokWebAccountAvailability {
        self.availability
    }

    /// Records only an independently evidenced account-forbidden disposition.
    ///
    /// # Errors
    ///
    /// Rejects a mismatched/expired session or every non-account disposition without mutation.
    pub fn observe_account_forbidden(
        &mut self,
        session: &GrokWebBrowserEgressSession,
        disposition: &GrokWebFailureDisposition,
        now_ms: i64,
    ) -> Result<(), GrokWebFailureStateError> {
        self.binding.require_current_session(session, now_ms)?;
        if disposition.action() != GrokWebFailureAction::MarkExactAccountForbidden {
            return Err(GrokWebFailureStateError::InvalidAccountAction);
        }
        self.availability = GrokWebAccountAvailability::Forbidden;
        Ok(())
    }

    /// Ensures a later local request boundary cannot reuse an account lifecycle marked forbidden.
    ///
    /// # Errors
    ///
    /// Returns a safe binding, expiry, or account-forbidden category without changing state.
    pub fn require_available(
        &self,
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<(), GrokWebFailureStateError> {
        self.binding.require_current_session(session, now_ms)?;
        if self.availability == GrokWebAccountAvailability::Forbidden {
            return Err(GrokWebFailureStateError::AccountForbidden);
        }
        Ok(())
    }
}

impl fmt::Debug for GrokWebAccountFailureState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebAccountFailureState")
            .field("binding", &self.binding)
            .field("availability", &self.availability)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
struct GrokWebAccountBinding {
    account_reference: String,
    lineage_reference: String,
    credential_revision: u64,
    credential_expires_at_ms: i64,
}

impl GrokWebAccountBinding {
    fn from_session(
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<Self, GrokWebFailureStateError> {
        if now_ms < 0 {
            return Err(GrokWebFailureStateError::InvalidObservationTime);
        }
        if session.is_expired_at(now_ms) {
            return Err(GrokWebFailureStateError::ExpiredEgressSession);
        }
        Ok(Self {
            account_reference: session.account_reference().to_owned(),
            lineage_reference: session.lineage_reference().to_owned(),
            credential_revision: session.credential_revision(),
            credential_expires_at_ms: session.credential_expires_at_ms(),
        })
    }

    fn require_current_session(
        &self,
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<(), GrokWebFailureStateError> {
        if now_ms < 0 {
            return Err(GrokWebFailureStateError::InvalidObservationTime);
        }
        if now_ms >= self.credential_expires_at_ms || session.is_expired_at(now_ms) {
            return Err(GrokWebFailureStateError::ExpiredEgressSession);
        }
        if self.account_reference != session.account_reference()
            || self.lineage_reference != session.lineage_reference()
            || self.credential_revision != session.credential_revision()
            || self.credential_expires_at_ms != session.credential_expires_at_ms()
        {
            return Err(GrokWebFailureStateError::SessionBindingMismatch);
        }
        Ok(())
    }
}

impl fmt::Debug for GrokWebAccountBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebAccountBinding")
            .field("account_reference", &"<redacted>")
            .field("lineage_reference", &"<redacted>")
            .field("credential_revision", &self.credential_revision)
            .field("credential_expires_at_ms", &self.credential_expires_at_ms)
            .finish()
    }
}

/// Safe Web failure-classification or exact-local-state error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebFailureError {
    /// The supplied status is outside the HTTP status range.
    InvalidHttpStatus,
    /// Account-forbidden evidence was not attached to a 403 response.
    InvalidAccountEvidence,
}

impl fmt::Display for GrokWebFailureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidHttpStatus => "Grok Web HTTP status is invalid",
            Self::InvalidAccountEvidence => "Grok Web account evidence is invalid",
        })
    }
}

impl Error for GrokWebFailureError {}

/// Safe exact-session failure-state error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebFailureStateError {
    /// Supplied state time was negative.
    InvalidObservationTime,
    /// The browser egress session or bound credential lifecycle is expired.
    ExpiredEgressSession,
    /// Account/lineage/revision/expiry/egress binding did not exactly match.
    SessionBindingMismatch,
    /// A state transition attempted to apply a non-egress failure disposition.
    InvalidEgressAction,
    /// A state transition attempted to apply a non-account failure disposition.
    InvalidAccountAction,
    /// This exact egress session was rejected and must be rebuilt or rotated.
    EgressRejected,
    /// This exact account credential lifecycle was independently marked forbidden.
    AccountForbidden,
}

impl fmt::Display for GrokWebFailureStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidObservationTime => "Grok Web failure observation time is invalid",
            Self::ExpiredEgressSession => "Grok Web failure egress session is expired",
            Self::SessionBindingMismatch => "Grok Web failure session binding does not match",
            Self::InvalidEgressAction => "Grok Web failure action cannot reject egress",
            Self::InvalidAccountAction => "Grok Web failure action cannot forbid account",
            Self::EgressRejected => "Grok Web egress session is rejected",
            Self::AccountForbidden => "Grok Web account is forbidden",
        })
    }
}

impl Error for GrokWebFailureStateError {}
