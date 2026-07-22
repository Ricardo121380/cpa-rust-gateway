//! `Grok` Official, Build, and Web adapters with isolated runtime namespaces.

#![deny(unsafe_code)]

mod oauth;

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
