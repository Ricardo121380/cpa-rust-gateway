//! Upstream, endpoint, binding, and credential lease boundary.

#![deny(unsafe_code)]

/// Provider-neutral egress profile for generic compatible endpoints.
pub mod compatible_egress;
/// Bounded Endpoint Credential pools and automatically released runtime leases.
pub mod credential_pool;
/// Immutable outbound URL admission and DNS-pinning policy.
pub mod egress_policy;
/// Deterministic configured Base URL and inference-path composition.
pub mod endpoint_url;
/// Bounded DNS-pinned shared HTTP client transport.
pub mod upstream_client;

pub use compatible_egress::{
    CompatibleEgressError, CompatibleEgressTarget, CompatibleEndpointEgressInput,
    CompatibleEndpointEgressProfile, CompatibleFailureScope, CompatibleRetryPolicy,
    CompatibleStickiness, MAX_COMPATIBLE_EGRESS_LABEL_LENGTH, MAX_COMPATIBLE_PRE_SUBMIT_ATTEMPTS,
};
pub use credential_pool::{
    CredentialLease, CredentialMaterialReplacement, CredentialPoolBuildError,
    CredentialPoolEntrySnapshot, CredentialSecret, EndpointCredentialInput, EndpointCredentialPool,
    EndpointCredentialPools, MAX_CREDENTIAL_SCHEDULE_SLOTS_PER_PRIORITY_TIER,
};
pub use egress_policy::{
    AdmittedEgressTarget, EgressAdmissionError, EgressAdmissionErrorCode, EgressCidr,
    EgressCidrError, EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy,
    EgressPolicyError, EgressPolicyErrorCode, EgressPolicyInput, EgressScheme, RedirectPolicy,
    SystemEgressDnsResolver,
};
pub use endpoint_url::{EndpointUrl, EndpointUrlError};
pub use upstream_client::{
    Socks5ProxyEndpoint, UpstreamClientPool, UpstreamHttpMethod, UpstreamHttpRequest,
    UpstreamHttpRequestError, UpstreamHttpRequestErrorCode, UpstreamHttpResponse, UpstreamProxy,
    UpstreamProxyError, UpstreamProxyErrorCode, UpstreamTimeoutError, UpstreamTimeoutErrorCode,
    UpstreamTimeouts, UpstreamTransportProfile,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-upstream";
