//! Bounded stream state-machine and backpressure boundary.

#![deny(unsafe_code)]

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-stream";
