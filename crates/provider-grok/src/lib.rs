//! `Grok` Official, Build, and Web adapters with isolated runtime namespaces.

#![deny(unsafe_code)]

mod build_responses;
mod continuity_state;
mod credential_runtime;
mod oauth;
mod runtime_state;

pub use build_responses::{
    GROK_BUILD_AGENT_ID_HEADER, GROK_BUILD_AUTHENTICATE_RESPONSE_HEADER,
    GROK_BUILD_AUTHENTICATE_RESPONSE_VALUE, GROK_BUILD_CLIENT_IDENTIFIER,
    GROK_BUILD_CLIENT_IDENTIFIER_HEADER, GROK_BUILD_CLIENT_MODE, GROK_BUILD_CLIENT_MODE_HEADER,
    GROK_BUILD_CLIENT_VERSION, GROK_BUILD_CLIENT_VERSION_HEADER, GROK_BUILD_MODEL_OVERRIDE_HEADER,
    GROK_BUILD_REQUEST_ID_HEADER, GROK_BUILD_RESPONSES_BASE_URL, GROK_BUILD_RESPONSES_PATH,
    GROK_BUILD_RESPONSES_URL, GROK_BUILD_TOKEN_AUTH_HEADER, GROK_BUILD_TOKEN_AUTH_VALUE,
    GROK_BUILD_TRACEPARENT_HEADER, GROK_BUILD_USER_AGENT, GrokBuildResponsesDecoder,
    GrokBuildResponsesEndpoint, GrokBuildResponsesErrorSignal, GrokBuildResponsesHttpError,
    GrokBuildResponsesOutboundRequest, GrokBuildResponsesRequestBuilder,
    GrokBuildResponsesStreamDecoder, MAX_GROK_BUILD_ERROR_BODY_BYTES,
    MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES, MAX_GROK_BUILD_SSE_FRAME_BYTES,
};
pub use continuity_state::{
    GrokBuildAffinityBindOutcome, GrokBuildAffinityBreak, GrokBuildAffinityBreakInput,
    GrokBuildAffinityBreakReason, GrokBuildAffinityReason, GrokBuildCacheAffinity,
    GrokBuildCacheAffinityKey, GrokBuildCacheIdentity, GrokBuildCacheIdentityDeriver,
    GrokBuildContinuityError, GrokBuildContinuityStore, GrokBuildReasoningReplay,
    GrokBuildReplayKey, GrokBuildReplayWriteOutcome, GrokBuildResponseOwnership,
    GrokBuildUpstreamResponseId,
};
pub use credential_runtime::{
    DEFAULT_GROK_BUILD_REFRESH_WAIT_TIMEOUT, GrokBuildCredentialCasOutcome,
    GrokBuildCredentialInsertOutcome, GrokBuildCredentialKey, GrokBuildCredentialKeyError,
    GrokBuildCredentialPersistence, GrokBuildCredentialPersistenceError,
    GrokBuildCredentialRefreshCoordinator, GrokBuildCredentialRefreshCoordinatorConfigError,
    GrokBuildCredentialRefreshError, GrokBuildCredentialRefreshOutcome,
    GrokBuildCredentialSqliteStore, GrokBuildCredentialVersion,
};
pub use oauth::{
    GROK_BUILD_DEVICE_AUTHORIZATION_URL, GROK_BUILD_OAUTH_ISSUER, GROK_BUILD_OAUTH_SCOPE,
    GROK_BUILD_PUBLIC_CLIENT_ID, GROK_BUILD_TOKEN_URL, GrokBuildCredential,
    GrokBuildCredentialSource, GrokBuildDeviceAuthorization, GrokBuildDevicePollOutcome,
    GrokBuildDevicePoller, GrokBuildOAuthEndpoint, GrokBuildOAuthError, GrokBuildOAuthFlow,
    GrokBuildOAuthHttpResponse, GrokBuildOAuthRequest, GrokBuildOAuthRequestKind,
    GrokBuildOAuthTransport, GrokBuildOAuthTransportError,
};
pub use runtime_state::{
    GrokBuildAccountEvidence, GrokBuildBillingPlan, GrokBuildCatalogSyncOutcome,
    GrokBuildFailureAction, GrokBuildFailureDisposition, GrokBuildModelCapability,
    GrokBuildModelSource, GrokBuildQuotaConfidence, GrokBuildQuotaSource,
    GrokBuildQuotaSyncOutcome, GrokBuildQuotaWindow, GrokBuildQuotaWindowKind,
    GrokBuildRateLimitEvidence, GrokBuildRuntimeStateError, GrokBuildRuntimeStateStore,
    classify_grok_build_failure,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "provider-grok";
