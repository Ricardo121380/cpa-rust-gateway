//! `Grok` Official, Build, and Web adapters with isolated runtime namespaces.

#![deny(unsafe_code)]

mod credential_runtime;
mod oauth;

pub use credential_runtime::{
    DEFAULT_GROK_BUILD_REFRESH_WAIT_TIMEOUT, GrokBuildCredentialCasOutcome,
    GrokBuildCredentialInsertOutcome, GrokBuildCredentialKey, GrokBuildCredentialKeyError,
    GrokBuildCredentialPersistence, GrokBuildCredentialPersistenceError,
    GrokBuildCredentialRefreshCoordinator, GrokBuildCredentialRefreshCoordinatorConfigError,
    GrokBuildCredentialRefreshError, GrokBuildCredentialRefreshOutcome,
    GrokBuildCredentialSqliteStore, GrokBuildCredentialVersion,
};
pub use oauth::{
    GROK_BUILD_DEVICE_AUTHORIZATION_URL, GROK_BUILD_OAUTH_SCOPE, GROK_BUILD_PUBLIC_CLIENT_ID,
    GROK_BUILD_TOKEN_URL, GrokBuildCredential, GrokBuildCredentialSource,
    GrokBuildDeviceAuthorization, GrokBuildDevicePollOutcome, GrokBuildDevicePoller,
    GrokBuildOAuthEndpoint, GrokBuildOAuthError, GrokBuildOAuthFlow, GrokBuildOAuthHttpResponse,
    GrokBuildOAuthRequest, GrokBuildOAuthRequestKind, GrokBuildOAuthTransport,
    GrokBuildOAuthTransportError,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "provider-grok";
