//! Configuration compilation, publication, rollback, and management boundary.

#![deny(unsafe_code)]

/// Durable Usage-to-billing materialization with restart-safe checkpoints.
pub mod billing_materializer;
/// Provider-scoped fixed-point pricing for the durable P13-05 billing ledger.
pub mod billing_service;
/// Transactional management-only Credential and Client Key provisioning service.
pub mod control_plane_service;
/// Management-time decryption and compilation of Endpoint Credential runtime pools.
pub mod credential_pool_compiler;
/// Management-time EgressPolicy compilation and endpoint static admission.
pub mod egress_policy_compiler;
/// Encrypted control-plane backup and empty-target restore facade for the P10 management surface.
pub mod management_backup_service;
/// Versioned draft-resource mutations used by the protected P10 management HTTP surface.
pub mod management_mutation_service;
/// Secret-free, version-scoped operational read models for the management surface.
pub mod management_operations_service;
/// Local transport-neutral lifecycle API for configuration validation and publication.
pub mod management_service;
/// Provider-owned, snapshot-scoped account-pool operational projections.
pub mod provider_account_pool_service;
/// Semantic Config Version validation and secret-free Route compilation.
pub mod route_compiler;
/// Immutable billing-catalog projection into request-time Provider price evidence.
pub mod routing_price_policy_service;
/// Atomic publication and rollback of compiler-approved RouteSnapshots.
pub mod snapshot_publisher;

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-control";
