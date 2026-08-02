//! Offline clean-room differential gate for P11.
//!
//! The reference side of every fixture is a recorded clean-room observation of an external system
//! (CPA `v7.2.80`, grok2api `v3.0.0`/`ec6cddca7`, Kiro-RS `c49c75e`). Those systems cannot be
//! executed here, so that side legitimately stays a committed literal. The gateway side is never a
//! literal: [`probe::observe`] computes it by driving this repository's real code, and the fixture
//! only records the projection the gateway is expected to produce.
//!
//! The gate is test-only. It has no transport, filesystem traversal, environment lookup,
//! reference-source reader, or credential type, and every error is value-free so a malformed
//! fixture cannot surface captured request or response material through a test failure.

#![deny(unsafe_code)]

mod fixture;
mod legacy_protocol;
mod probe;
mod vocabulary;

pub use fixture::{Classification, FixtureError, FixtureOutcome, validate_fixture};
pub use legacy_protocol::{
    LegacyProtocolClassification, LegacyProtocolError, LegacyProtocolOutcome,
    validate_legacy_protocol_corpus,
};
pub use probe::{ProbeError, observe};
pub use vocabulary::{GatewayObservability, ProjectionMarker, Reference, Subject};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "differential-gate";
