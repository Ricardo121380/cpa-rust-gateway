//! Generic `OpenAI`-compatible upstream provider boundary.

#![deny(unsafe_code)]

mod account_entitlement;
mod codex_catalog;
mod oauth_transport;
mod openai_chat_completions;
mod openai_responses;
mod runtime_credential;
mod runtime_failure;

pub use codex_catalog::{
    CODEX_CATALOG_CLIENT_VERSION, CODEX_CATALOG_PROVIDER_ID, CODEX_MODELS_URL, CODEX_ORIGINATOR,
    CODEX_RESPONSES_BASE_URL, CODEX_RESPONSES_PATH, CODEX_USER_AGENT, CodexCatalogAdapter,
    CodexCatalogCredential, CodexCatalogRequest, CodexCatalogTransport,
    CodexCatalogTransportResponse, CodexUpstreamCatalogTransport, MAX_CODEX_CATALOG_RESPONSE_BYTES,
};
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
