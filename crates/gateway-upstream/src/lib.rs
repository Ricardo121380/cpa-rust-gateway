//! Upstream, endpoint, binding, and credential lease boundary.

#![deny(unsafe_code)]

/// Immutable outbound URL admission and DNS-pinning policy.
pub mod egress_policy;

pub use egress_policy::{
    AdmittedEgressTarget, EgressAdmissionError, EgressAdmissionErrorCode, EgressCidr,
    EgressCidrError, EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy,
    EgressPolicyError, EgressPolicyErrorCode, EgressPolicyInput, EgressScheme, RedirectPolicy,
    SystemEgressDnsResolver,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-upstream";
