//! Cache, response ownership, replay, and conversation continuity boundary.

#![deny(unsafe_code)]

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-continuity";
