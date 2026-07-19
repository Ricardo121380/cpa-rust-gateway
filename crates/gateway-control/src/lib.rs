//! Configuration compilation, publication, rollback, and management boundary.

#![deny(unsafe_code)]

/// Transactional management-only Credential and Client Key provisioning service.
pub mod control_plane_service;
/// Management-time decryption and compilation of Endpoint Credential runtime pools.
pub mod credential_pool_compiler;
/// Management-time EgressPolicy compilation and endpoint static admission.
pub mod egress_policy_compiler;
/// Local transport-neutral lifecycle API for configuration validation and publication.
pub mod management_service;
/// Semantic Config Version validation and secret-free Route compilation.
pub mod route_compiler;
/// Atomic publication and rollback of compiler-approved RouteSnapshots.
pub mod snapshot_publisher;

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-control";
