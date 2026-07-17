//! Structured event and metrics boundary outside the request hot path.

#![deny(unsafe_code)]

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-observability";
