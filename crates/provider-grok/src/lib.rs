//! `Grok` Official, Build, and Web adapters with isolated runtime namespaces.

#![deny(unsafe_code)]

mod account_pool;
mod account_worker;
mod build_responses;
mod console_dpop;
mod console_quota;
mod console_responses;
mod continuity_state;
mod credential_runtime;
mod grok2api_migration;
mod inference;
mod oauth;
mod official;
mod official_capabilities;
mod official_metadata;
mod official_responses;
mod official_runtime;
mod provider_egress;
mod reauth;
mod runtime_state;
mod strict_json;
mod web_canary;
mod web_chat;
mod web_conversation;
mod web_credential;
mod web_egress_session;
mod web_failure;
mod web_flaresolverr;
mod web_live;
mod web_production;
mod web_quota;
mod web_statsig;
mod web_tool_emulation;

pub use account_pool::{
    GrokAccountAuthStatus, GrokAccountCredential, GrokAccountEndpointBinding, GrokAccountIdentity,
    GrokAccountImport, GrokAccountImportOutcome, GrokAccountImportRelation, GrokAccountMetadata,
    GrokAccountPoolError, GrokAccountPoolStore, GrokAccountProvider, GrokAccountRollbackOutcome,
    GrokNativeAccountCompileError, GrokNativeAccountPoolCompilation,
};
pub use account_worker::{
    GrokAccountQuotaConfidence, GrokAccountQuotaScope, GrokAccountQuotaSource,
    GrokAccountQuotaWindow, GrokAccountWorkerCoordinator, GrokAccountWorkerError,
    GrokAccountWorkerExecutor, GrokAccountWorkerJob, GrokAccountWorkerKind,
    GrokAccountWorkerResult, GrokAccountWorkerRunSummary, deterministic_refresh_due_at,
};
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
pub use console_dpop::{
    GrokConsoleDpopError, GrokConsoleDpopSession, GrokConsoleDpopSessionCache,
    GrokConsoleDpopToken, grok_console_dpop_cache_key,
};
pub use console_quota::{
    GrokConsoleQuotaError, GrokConsoleQuotaKind, GrokConsoleQuotaWindow, GrokConsoleUsageSnapshot,
    parse_grok_console_usage,
};
pub use console_responses::{
    GROK_CONSOLE_CLUSTER, GROK_CONSOLE_PROVIDER_ID, GROK_CONSOLE_RESPONSES_BASE_URL,
    GROK_CONSOLE_RESPONSES_PATH, GROK_CONSOLE_RESPONSES_URL, GROK_CONSOLE_USER_AGENT,
    GrokConsoleExecutionMode, GrokConsoleFailureOwner, GrokConsoleInferenceAdapter,
    GrokConsoleRequestError, GrokConsoleResponseBody, GrokConsoleResponseContentType,
    GrokConsoleResponsesDecoder, GrokConsoleResponsesOutboundRequest,
    GrokConsoleResponsesRequestBuilder, GrokConsoleResponsesStreamDecoder, GrokConsoleSsoToken,
    GrokConsoleTransport, GrokConsoleTransportResponse, GrokConsoleUpstreamTransport,
    classify_grok_console_http_failure, grok_console_retry_after_due_at,
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
pub use grok2api_migration::{
    Grok2ApiMemoryStreamMigration, Grok2ApiMigrationError, Grok2ApiMigrationFailureKind,
    Grok2ApiMigrationReceipt, MAX_GROK2API_MIGRATION_RECORD_BYTES, MAX_GROK2API_MIGRATION_RECORDS,
    MAX_GROK2API_MIGRATION_STREAM_BYTES,
};
pub use inference::{
    GROK_BUILD_PROVIDER_ID, GrokBuildExecutionMode, GrokBuildInferenceAdapter,
    GrokBuildResponseBody, GrokBuildResponseContentEncoding, GrokBuildResponseContentType,
    GrokBuildTransport, GrokBuildTransportResponse, GrokBuildUpstreamTransport,
};
pub use oauth::{
    GROK_BUILD_DEVICE_AUTHORIZATION_URL, GROK_BUILD_OAUTH_ISSUER, GROK_BUILD_OAUTH_SCOPE,
    GROK_BUILD_PUBLIC_CLIENT_ID, GROK_BUILD_TOKEN_URL, GrokBuildCredential,
    GrokBuildCredentialSource, GrokBuildDeviceAuthorization, GrokBuildDevicePollOutcome,
    GrokBuildDevicePoller, GrokBuildOAuthEndpoint, GrokBuildOAuthError, GrokBuildOAuthFlow,
    GrokBuildOAuthHttpResponse, GrokBuildOAuthRequest, GrokBuildOAuthRequestKind,
    GrokBuildOAuthTransport, GrokBuildOAuthTransportError,
};
pub use official::{
    GROK_OFFICIAL_API_BASE_URL, GROK_OFFICIAL_MODELS_PATH, GROK_OFFICIAL_MODELS_URL,
    GROK_OFFICIAL_PROVIDER_ID, GrokOfficialApiKey, GrokOfficialCatalogAdapter,
    GrokOfficialCatalogRequest, GrokOfficialCatalogTransport, GrokOfficialCatalogTransportResponse,
    GrokOfficialModelsEndpoint, GrokOfficialUpstreamCatalogTransport,
    MAX_GROK_OFFICIAL_CATALOG_RESPONSE_BYTES,
};
pub use official_capabilities::{GrokOfficialCapabilities, GrokOfficialSearchCapability};
pub use official_metadata::{
    GrokOfficialBillingMetadata, GrokOfficialRateLimitKind, GrokOfficialRateLimitMetadata,
    GrokOfficialRateLimitWindow, MAX_GROK_OFFICIAL_RATE_LIMIT_RESET,
    MAX_GROK_OFFICIAL_RATE_LIMIT_VALUE_BYTES,
};
pub use official_responses::{
    GROK_OFFICIAL_RESPONSES_PATH, GROK_OFFICIAL_RESPONSES_URL, GrokOfficialExecutionMode,
    GrokOfficialInferenceAdapter, GrokOfficialResponseBody, GrokOfficialResponseContentType,
    GrokOfficialResponsesDecoder, GrokOfficialResponsesEndpoint,
    GrokOfficialResponsesOutboundRequest, GrokOfficialResponsesRequestBuilder,
    GrokOfficialResponsesStreamDecoder, GrokOfficialTransport, GrokOfficialTransportResponse,
    GrokOfficialUpstreamTransport, MAX_GROK_OFFICIAL_ERROR_BODY_BYTES,
    MAX_GROK_OFFICIAL_NON_STREAMING_RESPONSE_BYTES, MAX_GROK_OFFICIAL_SSE_FRAME_BYTES,
};
pub use official_runtime::{
    GrokOfficialContinuityPolicy, GrokOfficialFailureAction, GrokOfficialFailureDisposition,
    GrokOfficialRuntimeState, GrokOfficialRuntimeStateError, classify_grok_official_http_failure,
};
pub use provider_egress::{
    GrokNativeEgressAttempt, GrokNativeEgressAttemptError, GrokNativeEgressAttemptSnapshot,
    GrokNativeEgressClock, SystemGrokNativeEgressClock,
};
pub use reauth::{
    GrokReauthAttempt, GrokReauthCoordinator, GrokReauthError, GrokReauthExecutor, GrokReauthJob,
    GrokReauthResult, GrokReauthRunSummary, GrokReauthStrategy, MAX_GROK_REAUTH_BATCH,
};
pub use runtime_state::{
    GrokBuildAccountEvidence, GrokBuildBillingPlan, GrokBuildCatalogSyncOutcome,
    GrokBuildFailureAction, GrokBuildFailureDisposition, GrokBuildModelCapability,
    GrokBuildModelSource, GrokBuildQuotaConfidence, GrokBuildQuotaSource,
    GrokBuildQuotaSyncOutcome, GrokBuildQuotaWindow, GrokBuildQuotaWindowKind,
    GrokBuildRateLimitEvidence, GrokBuildRuntimeStateError, GrokBuildRuntimeStateStore,
    classify_grok_build_failure,
};
pub use web_canary::{
    GROK_WEB_CANARY_HOST, GROK_WEB_CANARY_PATH, GROK_WEB_CANARY_URL, GrokWebCanaryOutboundRequest,
    GrokWebCanaryRequestBuilder, GrokWebCanaryRequestError, MAX_GROK_WEB_CANARY_REQUEST_BYTES,
};
pub use web_chat::{
    GROK_WEB_CHAT_FIXTURE_HOST, GROK_WEB_CHAT_FIXTURE_PATH, GrokWebChatFixtureTarget,
    GrokWebChatOutboundRequest, GrokWebChatRequestBuilder, GrokWebChatRequestError,
    GrokWebChatStreamDecoder, MAX_GROK_WEB_SSE_FRAME_BYTES,
};
pub use web_conversation::{
    GrokWebConversationAvailability, GrokWebConversationError, GrokWebConversationId,
    GrokWebConversationState, GrokWebConversationTurn, GrokWebParentMessageId,
};
pub use web_credential::{
    GROK_WEB_PROVIDER_ID, GrokWebCredential, GrokWebCredentialCasOutcome,
    GrokWebCredentialEnvelope, GrokWebCredentialError, GrokWebCredentialLineage,
    GrokWebCredentialSlot, GrokWebCredentialSource, GrokWebSessionCookie,
};
pub use web_egress_session::{
    GrokWebBrowserEgressSession, GrokWebBrowserEgressSessionError, GrokWebBrowserUserAgent,
    GrokWebEgressSessionId, GrokWebTlsProfile,
};
pub use web_failure::{
    GrokWebAccountAvailability, GrokWebAccountEvidence, GrokWebAccountFailureState,
    GrokWebEgressAvailability, GrokWebEgressFailureState, GrokWebFailureAction,
    GrokWebFailureDisposition, GrokWebFailureError, GrokWebFailureStateError,
    classify_grok_web_http_failure,
};
pub use web_flaresolverr::{
    GROK_WEB_FLARESOLVERR_URL, GrokWebFlareSolverrClearance, GrokWebFlareSolverrError,
    GrokWebFlareSolverrRequest, GrokWebFlareSolverrTransport, GrokWebFlareSolverrTransportResponse,
};
pub use web_live::{GrokWebLiveStreamDecoder, MAX_GROK_WEB_LIVE_FRAME_BYTES};
pub use web_production::{
    GROK_WEB_PRODUCTION_BASE_URL, GROK_WEB_PRODUCTION_PROVIDER_ID, GROK_WEB_PRODUCTION_USER_AGENT,
    GrokWebEgressRefresher, GrokWebProductionInferenceAdapter, GrokWebProductionOutboundRequest,
    GrokWebProductionRequestBuilder, GrokWebProductionRequestError, GrokWebProductionResponseBody,
    GrokWebProductionStreamDecoder, GrokWebProductionTransport, GrokWebProductionTransportResponse,
    GrokWebProductionUpstreamTransport, MAX_GROK_WEB_PRODUCTION_MESSAGE_BYTES,
    MAX_GROK_WEB_PRODUCTION_REQUEST_BYTES,
};
pub use web_quota::{
    GrokWebQuotaConfidence, GrokWebQuotaError, GrokWebQuotaFixtureDecoder, GrokWebQuotaSource,
    GrokWebQuotaState, GrokWebQuotaSyncOutcome, GrokWebQuotaTier, GrokWebQuotaWindow,
    GrokWebQuotaWindowKind, MAX_GROK_WEB_QUOTA_FIXTURE_BYTES,
};
pub use web_statsig::{
    GROK_WEB_DEFAULT_STATSIG_SIGNER_URL, GrokWebStatsigError, GrokWebStatsigRuntime,
    GrokWebStatsigSignature, GrokWebStatsigSignatureCache, GrokWebStatsigSignatureKey,
    GrokWebStatsigSignerBoundary, GrokWebStatsigSignerTarget, GrokWebStatsigTransport,
    GrokWebStatsigUpstreamTransport,
};
pub use web_tool_emulation::{
    GrokWebToolCapability, GrokWebToolEmulation, GrokWebToolEmulationError,
    GrokWebToolEmulationPrompt,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "provider-grok";
