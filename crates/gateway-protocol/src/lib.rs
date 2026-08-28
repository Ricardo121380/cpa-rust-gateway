//! Shared protocol adapter contracts over canonical core types.
//!
//! The crate currently owns exactly one contract: the closed Endpoint `api_format` vocabulary and
//! the registry that maps a validated format to the adapter serving it. It stays free of Provider,
//! transport, Router, and HTTP types so both the management-time compiler and a deployment
//! composition root can depend on it without depending on each other.

#![deny(unsafe_code)]

mod api_format;

pub use api_format::{ApiFormat, ApiFormatAdapterRegistry, ApiFormatAdapterRegistryError};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-protocol";
