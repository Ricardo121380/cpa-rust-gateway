//! Configuration compilation, publication, rollback, and management boundary.

#![deny(unsafe_code)]

/// Transactional management-only Credential and Client Key provisioning service.
pub mod control_plane_service;

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-control";
