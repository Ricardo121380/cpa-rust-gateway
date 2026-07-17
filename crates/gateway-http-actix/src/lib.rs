//! Actix Web transport shell. Core crates must not depend on this crate.

#![deny(unsafe_code)]

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-http-actix";
