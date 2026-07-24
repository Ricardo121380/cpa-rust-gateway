//! `Kiro` IDE/CLI endpoint-policy and AWS `EventStream` adapter boundary.

#![deny(unsafe_code)]

/// Strict Kiro credential import, encrypted sealing, and injected refresh boundary.
pub mod credential;

/// Per-Credential dynamic Kiro model and subscription capability snapshots.
pub mod dynamic_catalog;

/// Pure Canonical-request conversion to the Kiro conversation envelope.
pub mod conversation_request;

/// Bounded incremental AWS EventStream framing and checksum validation.
pub mod event_stream;

/// Semantic EventStream mapping for Kiro text, reasoning, and Claude Code Tools.
pub mod event_semantics;

/// Value-free Kiro network, account, model, quota, and rate-limit classification.
pub mod failure_classification;

/// Executable native Kiro request, EventStream, and injected transport boundary.
pub mod inference;

/// Fixed Kiro IDE/CLI host, header, origin, and thinking-placement policy.
pub mod endpoint_policy;

/// Kiro `profileArn` lookup, region-aware fallback, body injection, and safe provenance.
pub mod profile_arn;

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "provider-kiro";
