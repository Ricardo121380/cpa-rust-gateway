//! Control-plane persistence boundary; never queried by the route hot path.

#![deny(unsafe_code)]

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-store";
