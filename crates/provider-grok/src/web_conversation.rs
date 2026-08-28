//! Exact account and browser-egress binding for one Grok Web conversation.
//!
//! The state in this module is entirely local. It does not discover browser history, read a Web
//! account, create a request, or classify HTTP failures. P9-07 may later decide that account
//! evidence exists and mark this state unavailable; P9-09 alone can validate live Web behavior.

use std::{error::Error, fmt};

use crate::GrokWebBrowserEgressSession;

const MAX_CONVERSATION_ID_BYTES: usize = 512;
const MAX_MESSAGE_ID_BYTES: usize = 512;

/// Opaque identity of one Web conversation, supplied by the caller rather than inferred from
/// request text or an account.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GrokWebConversationId(String);

impl GrokWebConversationId {
    /// Creates a bounded visible-ASCII conversation identifier.
    ///
    /// # Errors
    ///
    /// Returns a safe value-free error without retaining invalid input.
    pub fn try_new(value: &str) -> Result<Self, GrokWebConversationError> {
        validate_identifier(value, MAX_CONVERSATION_ID_BYTES)
            .map_err(|()| GrokWebConversationError::InvalidConversationId)?;
        Ok(Self(value.to_owned()))
    }

    /// Returns the opaque value only to a later request composition boundary.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GrokWebConversationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokWebConversationId(<redacted>)")
    }
}

/// Opaque parent message identity used to continue one already-bound Web conversation.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GrokWebParentMessageId(String);

impl GrokWebParentMessageId {
    /// Creates a bounded visible-ASCII parent message identifier.
    ///
    /// # Errors
    ///
    /// Returns a safe value-free error without retaining invalid input.
    pub fn try_new(value: &str) -> Result<Self, GrokWebConversationError> {
        validate_identifier(value, MAX_MESSAGE_ID_BYTES)
            .map_err(|()| GrokWebConversationError::InvalidParentMessageId)?;
        Ok(Self(value.to_owned()))
    }

    /// Returns the opaque value only to a later request composition boundary.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GrokWebParentMessageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokWebParentMessageId(<redacted>)")
    }
}

/// Safe lifecycle projection for one explicitly bound Web conversation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebConversationAvailability {
    /// The exact egress-session binding remains locally usable.
    Available,
    /// Account-level evidence from a later classifier made this conversation unusable.
    AccountUnavailable,
}

/// One immutable snapshot used to compose a single next turn.
///
/// It intentionally carries only opaque conversation/parent identity and never retains Cookie,
/// User-Agent, proxy, TLS-profile, model, prompt, or provider response text.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebConversationTurn {
    conversation_id: GrokWebConversationId,
    parent_message_id: Option<GrokWebParentMessageId>,
}

impl GrokWebConversationTurn {
    /// Returns the exact bound conversation identity for a later narrow request composer.
    #[must_use]
    pub const fn conversation_id(&self) -> &GrokWebConversationId {
        &self.conversation_id
    }

    /// Returns the latest recorded parent identity, if this is a continuation turn.
    #[must_use]
    pub const fn parent_message_id(&self) -> Option<&GrokWebParentMessageId> {
        self.parent_message_id.as_ref()
    }
}

impl fmt::Debug for GrokWebConversationTurn {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebConversationTurn")
            .field("conversation_id", &self.conversation_id)
            .field("has_parent_message", &self.parent_message_id.is_some())
            .finish()
    }
}

/// Locally owned Web conversation state with an exact account/egress/credential binding.
///
/// A refresh or egress rotation must construct a new state. It cannot mutate this state across a
/// different account, SSO lineage, credential revision, or egress-session identity.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebConversationState {
    conversation_id: GrokWebConversationId,
    parent_message_id: Option<GrokWebParentMessageId>,
    account_reference: String,
    lineage_reference: String,
    credential_revision: u64,
    egress_session_id: String,
    expires_at_ms: i64,
    availability: GrokWebConversationAvailability,
}

impl GrokWebConversationState {
    /// Binds one caller-supplied conversation identity to the exact currently usable egress session.
    ///
    /// # Errors
    ///
    /// Returns a safe expiry/time category and makes no external state change.
    pub fn try_new(
        conversation_id: GrokWebConversationId,
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<Self, GrokWebConversationError> {
        if now_ms < 0 {
            return Err(GrokWebConversationError::InvalidObservationTime);
        }
        if session.is_expired_at(now_ms) {
            return Err(GrokWebConversationError::ExpiredEgressSession);
        }
        Ok(Self {
            conversation_id,
            parent_message_id: None,
            account_reference: session.account_reference().to_owned(),
            lineage_reference: session.lineage_reference().to_owned(),
            credential_revision: session.credential_revision(),
            egress_session_id: session.egress_session_id().as_str().to_owned(),
            expires_at_ms: session.credential_expires_at_ms(),
            availability: GrokWebConversationAvailability::Available,
        })
    }

    /// Returns the opaque conversation identity without exposing session secrets.
    #[must_use]
    pub const fn conversation_id(&self) -> &GrokWebConversationId {
        &self.conversation_id
    }

    /// Returns the safe lifecycle projection of this exact conversation only.
    #[must_use]
    pub const fn availability(&self) -> GrokWebConversationAvailability {
        self.availability
    }

    /// Validates the exact bound session and returns a snapshot for the next turn.
    ///
    /// # Errors
    ///
    /// Returns a safe mismatch, expiry, or account-unavailable category without rendering
    /// conversation, account, lineage, egress, Cookie, User-Agent, or parent values.
    pub fn prepare_turn(
        &self,
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<GrokWebConversationTurn, GrokWebConversationError> {
        self.require_current_session(session, now_ms)?;
        if self.availability == GrokWebConversationAvailability::AccountUnavailable {
            return Err(GrokWebConversationError::AccountUnavailable);
        }
        Ok(GrokWebConversationTurn {
            conversation_id: self.conversation_id.clone(),
            parent_message_id: self.parent_message_id.clone(),
        })
    }

    /// Records one newly completed parent message after verifying the exact original session.
    ///
    /// # Errors
    ///
    /// Returns a safe mismatch, expiry, unavailable-account, or duplicate-parent category. State
    /// remains unchanged when validation fails.
    pub fn record_parent_message(
        &mut self,
        session: &GrokWebBrowserEgressSession,
        parent_message_id: GrokWebParentMessageId,
        now_ms: i64,
    ) -> Result<(), GrokWebConversationError> {
        self.prepare_turn(session, now_ms)?;
        if self.parent_message_id.as_ref() == Some(&parent_message_id) {
            return Err(GrokWebConversationError::DuplicateParentMessageId);
        }
        self.parent_message_id = Some(parent_message_id);
        Ok(())
    }

    /// Marks this exact conversation unusable only after the caller already holds account-level
    /// evidence. P9-07 owns classification; this state object only prevents future continuation.
    ///
    /// # Errors
    ///
    /// Returns a safe mismatch or expiry category and leaves the availability projection unchanged
    /// when the supplied session is not the state-bound session.
    pub fn mark_account_unavailable(
        &mut self,
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<(), GrokWebConversationError> {
        self.require_current_session(session, now_ms)?;
        self.availability = GrokWebConversationAvailability::AccountUnavailable;
        Ok(())
    }

    fn require_current_session(
        &self,
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<(), GrokWebConversationError> {
        if now_ms < 0 {
            return Err(GrokWebConversationError::InvalidObservationTime);
        }
        if now_ms >= self.expires_at_ms || session.is_expired_at(now_ms) {
            return Err(GrokWebConversationError::ExpiredEgressSession);
        }
        if self.account_reference != session.account_reference() {
            return Err(GrokWebConversationError::AccountMismatch);
        }
        if self.lineage_reference != session.lineage_reference() {
            return Err(GrokWebConversationError::LineageMismatch);
        }
        if self.credential_revision != session.credential_revision() {
            return Err(GrokWebConversationError::CredentialRevisionMismatch);
        }
        if self.expires_at_ms != session.credential_expires_at_ms() {
            return Err(GrokWebConversationError::CredentialExpiryMismatch);
        }
        if self.egress_session_id != session.egress_session_id().as_str() {
            return Err(GrokWebConversationError::EgressSessionMismatch);
        }
        Ok(())
    }
}

impl fmt::Debug for GrokWebConversationState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebConversationState")
            .field("conversation_id", &self.conversation_id)
            .field("has_parent_message", &self.parent_message_id.is_some())
            .field("account_reference", &"<redacted>")
            .field("lineage_reference", &"<redacted>")
            .field("credential_revision", &self.credential_revision)
            .field("egress_session_id", &"<redacted>")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("availability", &self.availability)
            .finish()
    }
}

/// Safe local Conversation binding failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebConversationError {
    /// Conversation identity was empty, oversized, or not visible ASCII.
    InvalidConversationId,
    /// Parent message identity was empty, oversized, or not visible ASCII.
    InvalidParentMessageId,
    /// The supplied observation time was negative.
    InvalidObservationTime,
    /// The bound or supplied browser egress session has expired.
    ExpiredEgressSession,
    /// The supplied session belongs to a different opaque Web account.
    AccountMismatch,
    /// The supplied session has a different opaque SSO lineage.
    LineageMismatch,
    /// The supplied session was created from a different credential revision.
    CredentialRevisionMismatch,
    /// The supplied session has a different absolute credential expiry.
    CredentialExpiryMismatch,
    /// The supplied session has a distinct egress-session identity.
    EgressSessionMismatch,
    /// Later account-level evidence blocked continuation of this exact conversation.
    AccountUnavailable,
    /// A caller attempted to record the currently retained parent twice.
    DuplicateParentMessageId,
}

impl fmt::Display for GrokWebConversationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConversationId => "Grok Web conversation identity is invalid",
            Self::InvalidParentMessageId => "Grok Web parent message identity is invalid",
            Self::InvalidObservationTime => "Grok Web conversation observation time is invalid",
            Self::ExpiredEgressSession => "Grok Web conversation egress session is expired",
            Self::AccountMismatch => "Grok Web conversation account does not match",
            Self::LineageMismatch => "Grok Web conversation lineage does not match",
            Self::CredentialRevisionMismatch => {
                "Grok Web conversation credential revision does not match"
            }
            Self::CredentialExpiryMismatch => {
                "Grok Web conversation credential expiry does not match"
            }
            Self::EgressSessionMismatch => "Grok Web conversation egress session does not match",
            Self::AccountUnavailable => "Grok Web conversation account is unavailable",
            Self::DuplicateParentMessageId => "Grok Web conversation parent message is duplicated",
        })
    }
}

impl Error for GrokWebConversationError {}

fn validate_identifier(value: &str, maximum_bytes: usize) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(());
    }
    Ok(())
}
