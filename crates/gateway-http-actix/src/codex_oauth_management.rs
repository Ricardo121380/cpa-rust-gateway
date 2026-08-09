//! Backend-only Codex OAuth session state.
//!
//! The state machine mirrors the incumbent CPA/Sub2API lifecycle while keeping replay material
//! out of durable state and HTTP projections. Token exchange is deliberately injected later.

#![allow(missing_docs)]

use std::{fmt, time::Duration};

use gateway_core::CredentialId;
use getrandom::fill;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

const SESSION_TTL: Duration = Duration::from_mins(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexOAuthSessionState {
    Pending,
    Complete,
    Cancelled,
    Expired,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexOAuthSessionView {
    pub credential_id: CredentialId,
    pub state: CodexOAuthSessionState,
    pub expires_at_ms: i64,
    pub failure_class: Option<&'static str>,
}

/// One-time process-local OAuth material. It is never serializable or logged.
pub struct CodexOAuthSession {
    credential_id: CredentialId,
    state_digest: [u8; 32],
    verifier_digest: [u8; 32],
    transient_state: Zeroizing<Vec<u8>>,
    transient_verifier: Zeroizing<Vec<u8>>,
    expires_at_ms: i64,
    state: CodexOAuthSessionState,
    failure_class: Option<&'static str>,
    completion_claimed: bool,
}

impl CodexOAuthSession {
    /// Creates a session with random state and PKCE verifier; only their digests are retained.
    ///
    /// # Errors
    ///
    /// Returns an error if the bounded expiry cannot be represented or secure randomness is
    /// unavailable.
    pub fn start(credential_id: CredentialId, now_ms: i64) -> Result<Self, CodexOAuthSessionError> {
        let expires_at_ms = now_ms
            .checked_add(
                i64::try_from(SESSION_TTL.as_millis())
                    .map_err(|_| CodexOAuthSessionError::InvalidTime)?,
            )
            .ok_or(CodexOAuthSessionError::InvalidTime)?;
        let mut state = [0_u8; 32];
        // Match the incumbent CLIProxyAPI verifier entropy/length (96 random bytes encoded as
        // unpadded base64url, 128 printable characters) instead of relying on the RFC minimum.
        // This avoids provider-specific PKCE length heuristics while keeping the raw value only in
        // the short-lived zeroizing session.
        let mut verifier = [0_u8; 96];
        fill(&mut state).map_err(|_| CodexOAuthSessionError::RandomUnavailable)?;
        fill(&mut verifier).map_err(|_| CodexOAuthSessionError::RandomUnavailable)?;
        Ok(Self {
            credential_id,
            state_digest: digest(&state),
            verifier_digest: digest(&verifier),
            transient_state: Zeroizing::new(state.to_vec()),
            transient_verifier: Zeroizing::new(verifier.to_vec()),
            expires_at_ms,
            state: CodexOAuthSessionState::Pending,
            failure_class: None,
            completion_claimed: false,
        })
    }

    /// Returns the one-time raw state/verifier to the caller that will build the authorization URL.
    /// The caller must keep them transient; the session stores only digests.
    ///
    /// # Errors
    ///
    /// Returns `Terminal` after the session has ended and its transient material has been cleared.
    pub fn transient_challenge(&self) -> Result<(Vec<u8>, Vec<u8>), CodexOAuthSessionError> {
        if self.state != CodexOAuthSessionState::Pending
            || self.transient_state.is_empty()
            || self.transient_verifier.is_empty()
        {
            return Err(CodexOAuthSessionError::Terminal);
        }
        Ok((
            self.transient_state.to_vec(),
            self.transient_verifier.to_vec(),
        ))
    }

    /// Verifies a callback state against the stored digest without retaining the raw value.
    #[must_use]
    pub fn verify_state(&self, state: &[u8]) -> bool {
        self.state_digest == digest(state)
    }

    /// Verifies a PKCE verifier against the stored digest without retaining the raw value.
    #[must_use]
    pub fn verify_verifier(&self, verifier: &[u8]) -> bool {
        self.verifier_digest == digest(verifier)
    }

    /// Returns the transient PKCE verifier for the one callback that owns this session.
    #[must_use]
    pub fn transient_verifier(&self) -> Vec<u8> {
        self.transient_verifier.to_vec()
    }

    /// Claims the one callback exchange before any network work begins.
    ///
    /// The claim closes the replay window between token exchange and encrypted persistence. A
    /// second callback for the same session is rejected while the first caller is still saving.
    ///
    /// # Errors
    ///
    /// Returns `Expired` or `Terminal` when the session cannot accept another callback.
    pub fn claim_completion(&mut self, now_ms: i64) -> Result<(), CodexOAuthSessionError> {
        self.ensure_pending(now_ms)?;
        if self.completion_claimed {
            return Err(CodexOAuthSessionError::Terminal);
        }
        self.completion_claimed = true;
        Ok(())
    }

    /// Marks the claimed callback complete after durable credential persistence.
    ///
    /// # Errors
    ///
    /// Returns `Terminal` unless a callback was claimed for the pending session.
    pub fn complete(&mut self, now_ms: i64) -> Result<(), CodexOAuthSessionError> {
        if self.state != CodexOAuthSessionState::Pending || !self.completion_claimed {
            return Err(CodexOAuthSessionError::Terminal);
        }
        // Once the exchange has been claimed, the bounded provider request is allowed to finish
        // even if the ten-minute browser session expires while the token endpoint is in flight.
        let _ = now_ms;
        self.state = CodexOAuthSessionState::Complete;
        self.clear_transient_material();
        Ok(())
    }

    /// Marks an exchange or persistence failure with a fixed safe category.
    ///
    /// # Errors
    ///
    /// Returns `Expired` for an unclaimed expired session or `Terminal` after a terminal state.
    pub fn fail(&mut self, now_ms: i64, class: &'static str) -> Result<(), CodexOAuthSessionError> {
        if self.state != CodexOAuthSessionState::Pending {
            return Err(CodexOAuthSessionError::Terminal);
        }
        if !self.completion_claimed && now_ms >= self.expires_at_ms {
            self.state = CodexOAuthSessionState::Expired;
            self.clear_transient_material();
            return Err(CodexOAuthSessionError::Expired);
        }
        self.failure_class = Some(class);
        self.state = CodexOAuthSessionState::Failed;
        self.clear_transient_material();
        Ok(())
    }

    /// Cancels a pending session before callback completion is claimed.
    ///
    /// # Errors
    ///
    /// Returns `Expired` for an expired session or `Terminal` after a claim/terminal transition.
    pub fn cancel(&mut self, now_ms: i64) -> Result<(), CodexOAuthSessionError> {
        self.ensure_pending(now_ms)?;
        if self.completion_claimed {
            return Err(CodexOAuthSessionError::Terminal);
        }
        self.state = CodexOAuthSessionState::Cancelled;
        self.clear_transient_material();
        Ok(())
    }

    pub fn view(&mut self, now_ms: i64) -> CodexOAuthSessionView {
        if self.state == CodexOAuthSessionState::Pending
            && !self.completion_claimed
            && now_ms >= self.expires_at_ms
        {
            self.state = CodexOAuthSessionState::Expired;
            self.clear_transient_material();
        }
        CodexOAuthSessionView {
            credential_id: self.credential_id.clone(),
            state: self.state,
            expires_at_ms: self.expires_at_ms,
            failure_class: self.failure_class,
        }
    }

    fn ensure_pending(&mut self, now_ms: i64) -> Result<(), CodexOAuthSessionError> {
        if self.state != CodexOAuthSessionState::Pending {
            return Err(CodexOAuthSessionError::Terminal);
        }
        if now_ms >= self.expires_at_ms {
            self.state = CodexOAuthSessionState::Expired;
            self.clear_transient_material();
            return Err(CodexOAuthSessionError::Expired);
        }
        Ok(())
    }

    fn clear_transient_material(&mut self) {
        self.transient_state.zeroize();
        self.transient_state = Zeroizing::new(Vec::new());
        self.transient_verifier.zeroize();
        self.transient_verifier = Zeroizing::new(Vec::new());
    }
}

fn digest(value: &[u8]) -> [u8; 32] {
    Sha256::digest(value).into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexOAuthSessionError {
    InvalidTime,
    RandomUnavailable,
    Expired,
    Terminal,
}

impl fmt::Display for CodexOAuthSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidTime => "invalid OAuth session time",
            Self::RandomUnavailable => "OAuth session randomness unavailable",
            Self::Expired => "OAuth session expired",
            Self::Terminal => "OAuth session is terminal",
        })
    }
}

impl std::error::Error for CodexOAuthSessionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_is_bound_and_expires_without_replay_material()
    -> Result<(), Box<dyn std::error::Error>> {
        let credential_id = CredentialId::try_new("cred-codex")?;
        let mut session = CodexOAuthSession::start(credential_id.clone(), 1_000)?;
        assert_eq!(session.view(1_001).state, CodexOAuthSessionState::Pending);
        assert!(!session.verify_state(b"wrong-state"));
        assert!(!session.verify_verifier(b"wrong-verifier"));
        let (state, verifier) = session.transient_challenge()?;
        assert!(session.verify_state(&state));
        assert!(session.verify_verifier(&verifier));
        assert_eq!(session.view(601_001).state, CodexOAuthSessionState::Expired);
        assert_eq!(
            session.transient_challenge(),
            Err(CodexOAuthSessionError::Terminal)
        );
        assert_eq!(
            session.cancel(601_002),
            Err(CodexOAuthSessionError::Terminal)
        );
        Ok(())
    }

    #[test]
    fn terminal_transitions_are_one_way() -> Result<(), Box<dyn std::error::Error>> {
        let mut session = CodexOAuthSession::start(CredentialId::try_new("cred-codex")?, 1_000)?;
        session.fail(1_001, "token_exchange_denied")?;
        assert_eq!(session.view(1_002).state, CodexOAuthSessionState::Failed);
        assert_eq!(
            session.transient_challenge(),
            Err(CodexOAuthSessionError::Terminal)
        );
        assert_eq!(
            session.complete(1_003),
            Err(CodexOAuthSessionError::Terminal)
        );
        assert_eq!(
            session.view(1_004).failure_class,
            Some("token_exchange_denied")
        );
        Ok(())
    }

    #[test]
    fn completion_claim_is_single_use_and_survives_expiry_while_persisting()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut session = CodexOAuthSession::start(CredentialId::try_new("cred-codex")?, 1_000)?;
        session.claim_completion(1_001)?;
        assert_eq!(
            session.claim_completion(1_002),
            Err(CodexOAuthSessionError::Terminal)
        );
        assert_eq!(session.cancel(1_003), Err(CodexOAuthSessionError::Terminal));
        assert_eq!(session.view(601_001).state, CodexOAuthSessionState::Pending);
        session.complete(601_002)?;
        assert_eq!(
            session.view(601_003).state,
            CodexOAuthSessionState::Complete
        );
        assert_eq!(
            session.transient_challenge(),
            Err(CodexOAuthSessionError::Terminal)
        );
        Ok(())
    }
}
