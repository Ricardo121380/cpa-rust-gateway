//! Client and upstream credential boundary without storage implementation.
//!
//! P1 exposes an immutable in-memory Client Key authentication port. It deliberately does not
//! issue keys, persist keys, hash secrets, apply access-group policy, or decide routes; P2 replaces
//! the implementation with a compiled persistent/snapshot-backed view.

#![deny(unsafe_code)]

use std::{collections::BTreeMap, error::Error, fmt};

use gateway_core::{ClientKeyId, ErrorScope, GatewayError, GatewayErrorCode};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-auth";

/// Authenticates one client-presented API key without coupling callers to its storage mechanism.
///
/// P1 implementations are synchronous immutable lookups. P2 may replace the implementation with
/// a prefix/HMAC-validated immutable snapshot while retaining this transport-neutral interface.
pub trait ClientKeyAuthenticator: Send + Sync {
    /// Authenticates a complete client-presented key.
    ///
    /// # Errors
    ///
    /// Returns `ClientUnauthorized/Request` when the key is unknown or disabled. Later
    /// implementations may return another existing safe gateway error for an implementation
    /// failure, but must not expose the presented secret in it.
    fn authenticate(&self, presented_key: &str) -> Result<AuthenticatedClient, GatewayError>;
}

/// The stable non-secret identity produced by a successful Client Key authentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedClient {
    client_key_id: ClientKeyId,
}

impl AuthenticatedClient {
    /// Creates an authenticated identity from its stable Client Key identifier.
    #[must_use]
    pub const fn new(client_key_id: ClientKeyId) -> Self {
        Self { client_key_id }
    }

    /// Returns the authenticated Client Key identifier without exposing its secret.
    #[must_use]
    pub const fn client_key_id(&self) -> &ClientKeyId {
        &self.client_key_id
    }
}

/// Immutable configuration for one P1 in-memory Client Key.
#[derive(Clone, Eq, PartialEq)]
pub struct InMemoryClientKey {
    presented_key: String,
    client_key_id: ClientKeyId,
    enabled: bool,
}

impl InMemoryClientKey {
    /// Creates one enabled or disabled in-memory Client Key record.
    ///
    /// # Errors
    ///
    /// Returns [`InMemoryClientKeyConfigError::EmptyPresentedKey`] if `presented_key` is empty.
    /// The full key is retained only by the in-memory test/development implementation and is
    /// never exposed by this type's `Debug` representation.
    pub fn try_new(
        presented_key: impl Into<String>,
        client_key_id: ClientKeyId,
        enabled: bool,
    ) -> Result<Self, InMemoryClientKeyConfigError> {
        let presented_key = presented_key.into();
        if presented_key.is_empty() {
            return Err(InMemoryClientKeyConfigError::EmptyPresentedKey);
        }

        Ok(Self {
            presented_key,
            client_key_id,
            enabled,
        })
    }
}

impl fmt::Debug for InMemoryClientKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InMemoryClientKey")
            .field("presented_key", &"<redacted>")
            .field("client_key_id", &self.client_key_id)
            .field("enabled", &self.enabled)
            .finish()
    }
}

/// Configuration error for P1's immutable in-memory Client Key records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InMemoryClientKeyConfigError {
    /// A configured full key was empty.
    EmptyPresentedKey,
    /// More than one record used the same configured full key.
    DuplicatePresentedKey,
}

impl fmt::Display for InMemoryClientKeyConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPresentedKey => {
                formatter.write_str("in-memory client key must not be empty")
            }
            Self::DuplicatePresentedKey => {
                formatter.write_str("in-memory client key configuration contains a duplicate key")
            }
        }
    }
}

impl Error for InMemoryClientKeyConfigError {}

/// Immutable P1 Client Key authenticator backed by an in-memory ordered key set.
///
/// This exists solely to exercise the HTTP authentication boundary before P2 supplies persisted
/// key material and a precompiled runtime snapshot. Its `Debug` implementation reveals only the
/// number of configured entries, never a complete Client Key.
#[derive(Clone, Eq, PartialEq)]
pub struct InMemoryClientKeyAuthenticator {
    entries: BTreeMap<String, InMemoryClientKeyEntry>,
}

impl InMemoryClientKeyAuthenticator {
    /// Validates records and constructs an immutable in-memory authenticator.
    ///
    /// # Errors
    ///
    /// Returns [`InMemoryClientKeyConfigError::DuplicatePresentedKey`] instead of silently
    /// replacing an earlier configuration record.
    pub fn try_new(
        records: impl IntoIterator<Item = InMemoryClientKey>,
    ) -> Result<Self, InMemoryClientKeyConfigError> {
        let mut entries = BTreeMap::new();
        for record in records {
            let entry = InMemoryClientKeyEntry {
                client_key_id: record.client_key_id,
                enabled: record.enabled,
            };
            if entries.insert(record.presented_key, entry).is_some() {
                return Err(InMemoryClientKeyConfigError::DuplicatePresentedKey);
            }
        }

        Ok(Self { entries })
    }
}

impl fmt::Debug for InMemoryClientKeyAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InMemoryClientKeyAuthenticator")
            .field("entry_count", &self.entries.len())
            .finish()
    }
}

impl ClientKeyAuthenticator for InMemoryClientKeyAuthenticator {
    fn authenticate(&self, presented_key: &str) -> Result<AuthenticatedClient, GatewayError> {
        let Some(entry) = self.entries.get(presented_key) else {
            return Err(client_unauthorized_error());
        };
        if !entry.enabled {
            return Err(client_unauthorized_error());
        }

        Ok(AuthenticatedClient::new(entry.client_key_id.clone()))
    }
}

#[derive(Clone, Eq, PartialEq)]
struct InMemoryClientKeyEntry {
    client_key_id: ClientKeyId,
    enabled: bool,
}

const fn client_unauthorized_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientUnauthorized, ErrorScope::Request)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{ClientKeyId, ErrorScope, GatewayErrorCode};

    use super::{
        ClientKeyAuthenticator, InMemoryClientKey, InMemoryClientKeyAuthenticator,
        InMemoryClientKeyConfigError,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    fn key(
        presented_key: &str,
        client_key_id: &str,
        enabled: bool,
    ) -> Result<InMemoryClientKey, Box<dyn Error>> {
        Ok(InMemoryClientKey::try_new(
            presented_key,
            ClientKeyId::try_new(client_key_id)?,
            enabled,
        )?)
    }

    #[test]
    fn valid_key_returns_only_stable_client_identity() -> TestResult {
        let authenticator = InMemoryClientKeyAuthenticator::try_new([key(
            "generated-test-key",
            "client-key-enabled",
            true,
        )?])?;

        let authenticated = authenticator.authenticate("generated-test-key")?;
        assert_eq!(authenticated.client_key_id().as_str(), "client-key-enabled");
        Ok(())
    }

    #[test]
    fn unknown_and_disabled_keys_share_the_same_safe_rejection() -> TestResult {
        let authenticator = InMemoryClientKeyAuthenticator::try_new([key(
            "disabled-test-key",
            "client-key-disabled",
            false,
        )?])?;

        for presented_key in ["unknown-test-key", "disabled-test-key"] {
            let Err(error) = authenticator.authenticate(presented_key) else {
                return Err("unauthorized test key unexpectedly authenticated".into());
            };
            assert_eq!(error.code(), GatewayErrorCode::ClientUnauthorized);
            assert_eq!(error.scope(), ErrorScope::Request);
            assert_eq!(error.safe_message(), "the client is not authorized");
        }
        Ok(())
    }

    #[test]
    fn configuration_rejects_empty_and_duplicate_keys_without_revealing_them() -> TestResult {
        let id = ClientKeyId::try_new("client-key-id")?;
        assert_eq!(
            InMemoryClientKey::try_new("", id.clone(), true),
            Err(InMemoryClientKeyConfigError::EmptyPresentedKey)
        );

        let duplicate = "duplicate-test-key";
        let result = InMemoryClientKeyAuthenticator::try_new([
            key(duplicate, "client-key-first", true)?,
            key(duplicate, "client-key-second", false)?,
        ]);
        let Err(error) = result else {
            return Err("duplicate in-memory client key unexpectedly configured".into());
        };
        assert_eq!(error, InMemoryClientKeyConfigError::DuplicatePresentedKey);
        assert!(!error.to_string().contains(duplicate));
        Ok(())
    }

    #[test]
    fn debug_representations_redact_full_keys() -> TestResult {
        let secret = "redacted-test-key";
        let record = key(secret, "client-key-id", true)?;
        let authenticator = InMemoryClientKeyAuthenticator::try_new([record.clone()])?;

        assert!(!format!("{record:?}").contains(secret));
        assert!(!format!("{authenticator:?}").contains(secret));
        Ok(())
    }
}
