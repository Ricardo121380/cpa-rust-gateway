//! Generic `OpenAI`-compatible upstream provider boundary.

#![deny(unsafe_code)]

mod account_entitlement;
mod oauth_transport;
mod openai_chat_completions;
mod openai_responses;
mod runtime_credential;
mod runtime_failure;

pub use oauth_transport::{
    CodexOAuthRefreshCoordinator, CodexOAuthTokenTransport, CodexOAuthTransportError,
    refresh_with_transport,
};
pub use openai_chat_completions::{
    OpenAiChatCompletionsApiKey, OpenAiChatCompletionsEndpoint,
    OpenAiChatCompletionsOutboundRequest, OpenAiChatCompletionsRequestBuilder,
};
pub use openai_responses::{
    OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesOutboundRequest,
    OpenAiResponsesRequestBuilder,
};
pub use runtime_credential::{
    CODEX_OAUTH_CLIENT_ID, CODEX_OAUTH_TOKEN_URL, CodexCredentialExportFormat,
    CodexCredentialMetadata, CodexOAuthRefreshRequest, CodexOAuthRevisionedCredential,
    OpenAiCompatibleRuntimeCredential, OpenAiRuntimeCredentialError,
};
pub use runtime_failure::{
    OpenAiRuntimeFailureAction, OpenAiRuntimeFailureDisposition, classify_openai_runtime_failure,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "provider-openai-compatible";
