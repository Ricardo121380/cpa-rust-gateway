//! Framework-independent canonical gateway domain types.

#![deny(unsafe_code)]

mod canonical_event;
mod canonical_request;
mod error;
mod gateway_event;
mod id;
mod message;
mod raw_extension;
mod request_context;
mod retry_gate;
mod thinking;
mod tool;

pub use canonical_event::{
    CanonicalEvent, CanonicalEventState, CanonicalResponse, MessageEnd, MessageStart,
    ReasoningDelta, ResponseEnd, ResponseStart, StreamError, TextDelta, ToolCallArgumentsDelta,
    ToolCallEnd, ToolCallStart, Usage, UsageDelta,
};
pub use canonical_request::CanonicalRequest;
pub use error::{ErrorScope, GatewayError, GatewayErrorCode};
pub use gateway_event::{
    AttemptEvent, AttemptOutcome, AttemptRetryDecision, DiagnosticEvent, EventEmission,
    GatewayEvent, GatewayEventPriority, GatewayEventSink, GatewayProtocol, HealthEvent,
    HealthEventKind, NoopGatewayEventSink, RequestEvent, UsageEvent, UsageSummary,
};
pub use id::{
    AccessGroupId, AttemptId, AuthId, ClientKeyId, CredentialId, EgressPolicyId, EndpointId,
    HealthEventId, InvalidIdentifier, ProviderId, PublicModelId, RequestId, ResponseId,
    RouteCandidateId, RouteId, UpstreamId,
};
pub use message::{
    CanonicalMessage, MessageContent, MessageRole, OpaqueContent, TextContent, ToolCall, ToolResult,
};
pub use raw_extension::{RawExtensionError, RawExtensions, RawJson};
pub use request_context::RequestContext;
pub use retry_gate::{TransparentRetryGate, TransparentRetryGateFuture};
pub use thinking::{InvalidThinkingEffort, Thinking, ThinkingEffort};
pub use tool::ToolDefinition;

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-core";
