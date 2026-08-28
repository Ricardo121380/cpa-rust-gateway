//! Fixed, injectable Codex OAuth refresh transport boundary.

#![allow(missing_docs)]

use std::{error::Error, fmt};

use zeroize::Zeroizing;

use crate::{
    CodexOAuthRefreshRequest, CodexOAuthRevisionedCredential, OpenAiRuntimeCredentialError,
};

/// Safe transport failure classification; no upstream body is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexOAuthTransportError {
    Unavailable,
    Rejected,
    InvalidResponse,
    RevisionConflict,
}

impl fmt::Display for CodexOAuthTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "OAuth transport unavailable",
            Self::Rejected => "OAuth response rejected",
            Self::InvalidResponse => "OAuth response invalid",
            Self::RevisionConflict => "OAuth revision conflict",
        })
    }
}

impl Error for CodexOAuthTransportError {}

/// Minimal transport seam used by management code and deterministic tests.
pub trait CodexOAuthTokenTransport {
    /// Posts the fixed OAuth form to the already-admitted token URL.
    ///
    /// # Errors
    ///
    /// Returns a value-free transport classification when the request cannot be sent or the
    /// response is rejected.
    fn post_form(
        &mut self,
        url: &str,
        body: Zeroizing<String>,
    ) -> Result<Zeroizing<Vec<u8>>, CodexOAuthTransportError>;
}

/// Small backend coordinator that keeps the credential and transport in one refresh boundary.
pub struct CodexOAuthRefreshCoordinator<T> {
    credential: CodexOAuthRevisionedCredential,
    transport: T,
}

impl<T: CodexOAuthTokenTransport> CodexOAuthRefreshCoordinator<T> {
    /// Creates a coordinator for one persisted credential revision.
    pub fn new(credential: CodexOAuthRevisionedCredential, transport: T) -> Self {
        Self {
            credential,
            transport,
        }
    }

    /// Refreshes only the revision captured at the start of this operation.
    ///
    /// # Errors
    ///
    /// Returns a value-free transport classification; a stale revision never mutates the
    /// credential.
    pub fn refresh(&mut self, now_ms: i64) -> Result<u64, CodexOAuthTransportError> {
        let revision = self.credential.revision();
        refresh_with_transport(&mut self.credential, revision, &mut self.transport, now_ms)
    }

    /// Returns the current revision for persistence/audit correlation.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.credential.revision()
    }

    /// Borrows the value-redacted runtime credential for request composition.
    #[must_use]
    pub const fn credential(&self) -> &crate::OpenAiCompatibleRuntimeCredential {
        self.credential.credential()
    }
}

/// Executes one refresh and applies it with revision CAS.
///
/// # Errors
///
/// Returns a value-free transport classification when the request, response validation, or
/// revision compare-and-swap fails.
pub fn refresh_with_transport<T: CodexOAuthTokenTransport>(
    credential: &mut CodexOAuthRevisionedCredential,
    expected_revision: u64,
    transport: &mut T,
    now_ms: i64,
) -> Result<u64, CodexOAuthTransportError> {
    if credential.revision() != expected_revision {
        return Err(CodexOAuthTransportError::RevisionConflict);
    }
    let request = credential
        .refresh_request()
        .map_err(|_| CodexOAuthTransportError::RevisionConflict)?;
    let body = request.form_body();
    let response = transport.post_form(CodexOAuthRefreshRequest::token_url(), body)?;
    credential
        .apply_refresh_if_revision(expected_revision, response.as_slice(), now_ms)
        .map_err(|error| match error {
            OpenAiRuntimeCredentialError::Conflict => CodexOAuthTransportError::RevisionConflict,
            OpenAiRuntimeCredentialError::Invalid => CodexOAuthTransportError::InvalidResponse,
            OpenAiRuntimeCredentialError::NotRefreshable => CodexOAuthTransportError::Rejected,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenAiCompatibleRuntimeCredential;

    struct Mock {
        calls: usize,
        response: Zeroizing<Vec<u8>>,
    }

    impl CodexOAuthTokenTransport for Mock {
        fn post_form(
            &mut self,
            url: &str,
            body: Zeroizing<String>,
        ) -> Result<Zeroizing<Vec<u8>>, CodexOAuthTransportError> {
            assert_eq!(url, "https://auth.openai.com/oauth/token");
            assert!(body.contains("grant_type=refresh_token"));
            assert!(body.contains("client_id="));
            assert!(!body.contains("redirect_uri="));
            assert!(body.contains("scope=openid%20profile%20email"));
            self.calls += 1;
            Ok(std::mem::take(&mut self.response))
        }
    }

    #[test]
    fn refresh_transport_is_fixed_and_revision_guarded() -> Result<(), Box<dyn std::error::Error>> {
        let credential = OpenAiCompatibleRuntimeCredential::import(
            br#"{"kind":"codex_oauth","access_token":"old","refresh_token":"refresh","expires_at_ms":100}"#,
        )?;
        let mut revisioned = CodexOAuthRevisionedCredential::new(credential, 4)?;
        let mut mock = Mock {
            calls: 0,
            response: Zeroizing::new(
                br#"{"access_token":"new","refresh_token":"rotated","expires_in":60}"#.to_vec(),
            ),
        };
        assert_eq!(
            refresh_with_transport(&mut revisioned, 4, &mut mock, 1_000)?,
            5
        );
        assert_eq!(mock.calls, 1);
        assert_eq!(revisioned.credential().bearer_at(60_999)?, "new");
        assert_eq!(
            refresh_with_transport(&mut revisioned, 4, &mut mock, 2_000),
            Err(CodexOAuthTransportError::RevisionConflict)
        );
        assert_eq!(mock.calls, 1);
        Ok(())
    }

    #[test]
    fn coordinator_advances_only_after_valid_response() -> Result<(), Box<dyn std::error::Error>> {
        let credential = OpenAiCompatibleRuntimeCredential::import(
            br#"{"kind":"codex_oauth","access_token":"old","refresh_token":"refresh","expires_at_ms":100}"#,
        )?;
        let revisioned = CodexOAuthRevisionedCredential::new(credential, 10)?;
        let mock = Mock {
            calls: 0,
            response: Zeroizing::new(
                br#"{"access_token":"new","expires_in":30,"token_type":"bearer"}"#.to_vec(),
            ),
        };
        let mut coordinator = CodexOAuthRefreshCoordinator::new(revisioned, mock);
        assert_eq!(coordinator.refresh(1_000)?, 11);
        assert_eq!(coordinator.revision(), 11);
        assert_eq!(coordinator.credential().bearer_at(30_999)?, "new");
        Ok(())
    }
}
