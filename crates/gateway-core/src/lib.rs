//! Framework-independent canonical gateway domain types.

#![deny(unsafe_code)]

mod error;
mod id;
mod request_context;

pub use error::{ErrorScope, GatewayError, GatewayErrorCode};
pub use id::{
    AccessGroupId, AttemptId, AuthId, ClientKeyId, CredentialId, EndpointId, InvalidIdentifier,
    ProviderId, PublicModelId, RequestId, RouteCandidateId, RouteId, UpstreamId,
};
pub use request_context::RequestContext;

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-core";
