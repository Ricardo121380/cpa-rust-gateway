//! `Kiro` IDE/CLI endpoint-policy and AWS `EventStream` adapter boundary.

#![deny(unsafe_code)]

/// Strict Kiro credential import, encrypted sealing, and injected refresh boundary.
pub mod credential;

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "provider-kiro";
