//! Framework-independent canonical gateway domain types.

#![deny(unsafe_code)]

mod canonical_request;
mod error;
mod id;
mod message;
mod raw_extension;
mod request_context;
mod thinking;
mod tool;

pub use canonical_request::CanonicalRequest;
pub use error::{ErrorScope, GatewayError, GatewayErrorCode};
pub use id::{
    AccessGroupId, AttemptId, AuthId, ClientKeyId, CredentialId, EndpointId, InvalidIdentifier,
    ProviderId, PublicModelId, RequestId, RouteCandidateId, RouteId, UpstreamId,
};
pub use message::{
    CanonicalMessage, MessageContent, MessageRole, OpaqueContent, TextContent, ToolCall, ToolResult,
};
pub use raw_extension::{RawExtensionError, RawExtensions, RawJson};
pub use request_context::RequestContext;
pub use thinking::{InvalidThinkingEffort, Thinking, ThinkingEffort};
pub use tool::ToolDefinition;

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-core";
