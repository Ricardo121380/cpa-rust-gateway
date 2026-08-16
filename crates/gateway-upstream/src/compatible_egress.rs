//! Provider-neutral egress profiles for generic compatible endpoints.
//!
//! A configured relay such as a CPA export, a Sub2API export, a Krill endpoint, or an operator's
//! own `base_url + api_key` endpoint has the same egress problem: one exact endpoint, one exact
//! credential binding, and one independently admitted transport profile. The relay's display name
//! must not select a different proxy, retry, or fallback path.
//!
//! This module is deliberately a local, side-effect-free foundation. It does not resolve DNS,
//! open a proxy, read a credential, choose a pool member, or contact a Provider. `EgressPolicy`
//! and `EndpointUrl` remain the owners of URL/SSRF admission; a later composition layer owns the
//! actual proxy pool and runtime Health/Quota updates.

use std::{error::Error, fmt};

use gateway_core::{CredentialId, EgressPolicyId, EndpointId, GatewayProtocol, UpstreamId};

use crate::{EgressPolicy, EndpointUrl};

/// Maximum length of an opaque identity or non-secret profile label.
pub const MAX_COMPATIBLE_EGRESS_LABEL_LENGTH: usize = 128;

/// Maximum number of total attempts permitted by one normal pre-submit retry policy.
pub const MAX_COMPATIBLE_PRE_SUBMIT_ATTEMPTS: u8 = 3;

/// One transport target selected for a generic compatible endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompatibleEgressTarget {
    /// Connect directly to the admitted endpoint without an ambient/system proxy.
    Direct,
    /// Use one named, operator-configured proxy profile.
    FixedProxy {
        /// Opaque non-secret proxy profile identity.
        profile_id: String,
    },
    /// Select a member from one named, Provider-scoped proxy pool.
    ProxyPool {
        /// Opaque non-secret proxy pool identity.
        pool_id: String,
    },
}

impl CompatibleEgressTarget {
    /// Returns the selected target's stable non-secret identifier, if it has one.
    #[must_use]
    pub fn profile_id(&self) -> Option<&str> {
        match self {
            Self::Direct => None,
            Self::FixedProxy { profile_id } => Some(profile_id),
            Self::ProxyPool { pool_id } => Some(pool_id),
        }
    }

    fn validate(&self) -> Result<(), CompatibleEgressError> {
        if let Some(profile_id) = self.profile_id() {
            validate_label(profile_id)?;
        }
        Ok(())
    }

    fn has_egress_node(&self) -> bool {
        !matches!(self, Self::Direct)
    }
}

/// Which identity owns a transport failure observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleFailureScope {
    /// Cool down only the exact configured endpoint/channel.
    Endpoint,
    /// Mark only the exact endpoint-credential binding.
    Credential,
    /// Mark only the selected fixed proxy or proxy-pool node.
    EgressNode,
}

/// Sticky assignment policy for a compatible endpoint binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleStickiness {
    /// Do not retain an account-to-egress assignment after the request.
    None,
    /// Keep selection stable for the exact credential while it remains eligible.
    Credential,
    /// Keep the exact credential and selected egress node together.
    CredentialAndEgress,
}

/// Bounded retry policy for ordinary serving requests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleRetryPolicy {
    /// Do not retry this binding.
    None,
    /// Retry only before a request has been submitted to the upstream.
    PreSubmit {
        /// Total attempts, including the first submission; bounded to 1..=3.
        max_attempts: u8,
    },
}

impl CompatibleRetryPolicy {
    /// Creates a bounded pre-submit policy.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibleEgressError::InvalidPreSubmitAttempts`] when `max_attempts` is zero
    /// or exceeds [`MAX_COMPATIBLE_PRE_SUBMIT_ATTEMPTS`].
    pub fn pre_submit(max_attempts: u8) -> Result<Self, CompatibleEgressError> {
        if !(1..=MAX_COMPATIBLE_PRE_SUBMIT_ATTEMPTS).contains(&max_attempts) {
            return Err(CompatibleEgressError::InvalidPreSubmitAttempts);
        }
        Ok(Self::PreSubmit { max_attempts })
    }

    /// Returns the total attempt budget without implying post-submit replay.
    #[must_use]
    pub const fn max_attempts(self) -> u8 {
        match self {
            Self::None => 1,
            Self::PreSubmit { max_attempts } => max_attempts,
        }
    }
}

/// One exact, Provider-isolated egress profile for a compatible endpoint binding.
///
/// The profile carries no URL, API key, OAuth token, proxy credential, cookie, or response body.
/// `credential_kind` is a non-secret source label only; `CPA` JSON, `Sub2API` JSON, direct `OAuth`,
/// and a plain API-key record are all transport-equivalent inputs at this layer.
#[derive(Clone, Eq, PartialEq)]
pub struct CompatibleEndpointEgressProfile {
    upstream_id: UpstreamId,
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    credential_kind: String,
    protocol: GatewayProtocol,
    egress_policy_id: EgressPolicyId,
    target: CompatibleEgressTarget,
    failure_scope: CompatibleFailureScope,
    stickiness: CompatibleStickiness,
    retry_policy: CompatibleRetryPolicy,
}

/// Construction input for one [`CompatibleEndpointEgressProfile`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibleEndpointEgressInput {
    /// Owning configured upstream/provider instance.
    pub upstream_id: UpstreamId,
    /// Exact protocol-specific endpoint/channel identity.
    pub endpoint_id: EndpointId,
    /// Exact encrypted credential identity.
    pub credential_id: CredentialId,
    /// Non-secret source label such as `api_key`, `cpa_json`, or `sub2api_json`.
    pub credential_kind: String,
    /// Downstream wire protocol for this endpoint.
    pub protocol: GatewayProtocol,
    /// Version-scoped URL/SSRF policy identity.
    pub egress_policy_id: EgressPolicyId,
    /// Direct, fixed-proxy, or proxy-pool selection.
    pub target: CompatibleEgressTarget,
    /// Identity that owns a transport failure observation.
    pub failure_scope: CompatibleFailureScope,
    /// Account/egress stickiness policy.
    pub stickiness: CompatibleStickiness,
    /// Bounded pre-submit retry policy.
    pub retry_policy: CompatibleRetryPolicy,
}

impl CompatibleEndpointEgressProfile {
    /// Builds one validated profile for one exact upstream/endpoint/credential binding.
    ///
    /// The three identities are retained together so a later pool implementation cannot silently
    /// borrow a credential or egress member from another Provider/channel. This constructor does
    /// not contact a Provider or validate the endpoint URL; call [`Self::validate_endpoint_target`]
    /// with the already compiled [`EgressPolicy`] before dialing.
    ///
    /// # Errors
    ///
    /// Returns a closed [`CompatibleEgressError`] for an unbounded label, invalid retry budget,
    /// or a stickiness/failure scope that cannot describe the selected target.
    pub fn try_new(input: CompatibleEndpointEgressInput) -> Result<Self, CompatibleEgressError> {
        validate_id(input.upstream_id.as_str())?;
        validate_id(input.endpoint_id.as_str())?;
        validate_id(input.credential_id.as_str())?;
        validate_id(input.egress_policy_id.as_str())?;
        validate_label(&input.credential_kind)?;
        input.target.validate()?;

        if matches!(input.failure_scope, CompatibleFailureScope::EgressNode)
            && !input.target.has_egress_node()
        {
            return Err(CompatibleEgressError::EgressNodeScopeRequiresProxy);
        }
        if matches!(input.stickiness, CompatibleStickiness::CredentialAndEgress)
            && !input.target.has_egress_node()
        {
            return Err(CompatibleEgressError::EgressStickinessRequiresProxy);
        }

        Ok(Self {
            upstream_id: input.upstream_id,
            endpoint_id: input.endpoint_id,
            credential_id: input.credential_id,
            credential_kind: input.credential_kind,
            protocol: input.protocol,
            egress_policy_id: input.egress_policy_id,
            target: input.target,
            failure_scope: input.failure_scope,
            stickiness: input.stickiness,
            retry_policy: input.retry_policy,
        })
    }

    /// Validates that endpoint and credential owners agree with this profile's upstream.
    ///
    /// This is a pure graph check. It intentionally accepts the two owner IDs from the caller so
    /// it can be used by both the control-plane compiler and a runtime composition without
    /// introducing a Store dependency into this crate.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibleEgressError::EndpointUpstreamMismatch`] or
    /// [`CompatibleEgressError::CredentialUpstreamMismatch`] when either binding crosses its
    /// Provider/upstream boundary.
    pub fn validate_binding(
        &self,
        endpoint_upstream_id: &UpstreamId,
        credential_upstream_id: &UpstreamId,
    ) -> Result<(), CompatibleEgressError> {
        if endpoint_upstream_id != &self.upstream_id {
            return Err(CompatibleEgressError::EndpointUpstreamMismatch);
        }
        if credential_upstream_id != &self.upstream_id {
            return Err(CompatibleEgressError::CredentialUpstreamMismatch);
        }
        Ok(())
    }

    /// Reuses the existing URL and SSRF policy owners for a static endpoint-target check.
    ///
    /// No DNS lookup or socket is performed. The returned [`EndpointUrl`] is safe to hand to the
    /// later transport layer only after that layer performs its normal per-attempt
    /// [`EgressPolicy::admit_url`] call and address pinning.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibleEgressError::EndpointTargetRejected`] for either URL composition or
    /// policy-shape rejection, without retaining the supplied URL text.
    pub fn validate_endpoint_target(
        &self,
        policy: &EgressPolicy,
        base_url: &str,
        inference_path: &str,
    ) -> Result<EndpointUrl, CompatibleEgressError> {
        if policy.id() != &self.egress_policy_id {
            return Err(CompatibleEgressError::EndpointTargetRejected);
        }
        let endpoint_url = EndpointUrl::compose(base_url, inference_path)
            .map_err(|_| CompatibleEgressError::EndpointTargetRejected)?;
        policy
            .validate_url_shape(endpoint_url.as_str())
            .map_err(|_| CompatibleEgressError::EndpointTargetRejected)?;
        Ok(endpoint_url)
    }

    /// Returns the owning configured upstream identity.
    #[must_use]
    pub fn upstream_id(&self) -> &UpstreamId {
        &self.upstream_id
    }

    /// Returns the exact endpoint/channel identity.
    #[must_use]
    pub fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the exact credential identity.
    #[must_use]
    pub fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the non-secret credential source label.
    #[must_use]
    pub fn credential_kind(&self) -> &str {
        &self.credential_kind
    }

    /// Returns the downstream wire protocol used by this compatible endpoint.
    #[must_use]
    pub const fn protocol(&self) -> GatewayProtocol {
        self.protocol
    }

    /// Returns the version-scoped egress policy identity.
    #[must_use]
    pub fn egress_policy_id(&self) -> &EgressPolicyId {
        &self.egress_policy_id
    }

    /// Returns the selected direct/fixed/pool target.
    #[must_use]
    pub fn target(&self) -> &CompatibleEgressTarget {
        &self.target
    }

    /// Returns the identity that owns transport failures.
    #[must_use]
    pub const fn failure_scope(&self) -> CompatibleFailureScope {
        self.failure_scope
    }

    /// Returns the sticky assignment policy.
    #[must_use]
    pub const fn stickiness(&self) -> CompatibleStickiness {
        self.stickiness
    }

    /// Returns the bounded pre-submit retry policy.
    #[must_use]
    pub const fn retry_policy(&self) -> CompatibleRetryPolicy {
        self.retry_policy
    }

    /// Returns whether two profiles refer to the same exact Provider/channel/credential binding.
    #[must_use]
    pub fn same_binding(&self, other: &Self) -> bool {
        self.upstream_id == other.upstream_id
            && self.endpoint_id == other.endpoint_id
            && self.credential_id == other.credential_id
    }
}

impl fmt::Debug for CompatibleEndpointEgressProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompatibleEndpointEgressProfile")
            .field("upstream_id", &self.upstream_id)
            .field("endpoint_id", &self.endpoint_id)
            .field("credential_id", &self.credential_id)
            .field("credential_kind", &self.credential_kind)
            .field("protocol", &self.protocol)
            .field("egress_policy_id", &self.egress_policy_id)
            .field("target", &self.target)
            .field("failure_scope", &self.failure_scope)
            .field("stickiness", &self.stickiness)
            .field("retry_policy", &self.retry_policy)
            .finish()
    }
}

/// Closed, value-free construction and graph-validation failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleEgressError {
    /// An identity or non-secret label was empty or only whitespace.
    EmptyLabel,
    /// An identity or non-secret label exceeded the bounded profile size.
    LabelTooLong,
    /// An identity or non-secret label contained control or surrounding whitespace.
    InvalidLabelShape,
    /// A pre-submit attempt budget was zero or above the bounded maximum.
    InvalidPreSubmitAttempts,
    /// A failure scope named an egress node while direct transport was selected.
    EgressNodeScopeRequiresProxy,
    /// Credential-and-egress stickiness requires a fixed proxy or pool target.
    EgressStickinessRequiresProxy,
    /// The endpoint belongs to a different upstream than the profile.
    EndpointUpstreamMismatch,
    /// The credential belongs to a different upstream than the profile.
    CredentialUpstreamMismatch,
    /// The composed URL or static egress policy rejected the endpoint target.
    EndpointTargetRejected,
}

impl fmt::Display for CompatibleEgressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyLabel => "compatible egress label must not be empty",
            Self::LabelTooLong => "compatible egress label exceeds its bound",
            Self::InvalidLabelShape => "compatible egress label has an invalid shape",
            Self::InvalidPreSubmitAttempts => "compatible pre-submit attempts are out of bounds",
            Self::EgressNodeScopeRequiresProxy => {
                "egress-node failure scope requires a proxy target"
            }
            Self::EgressStickinessRequiresProxy => {
                "credential-and-egress stickiness requires a proxy target"
            }
            Self::EndpointUpstreamMismatch => "endpoint upstream does not match the profile",
            Self::CredentialUpstreamMismatch => "credential upstream does not match the profile",
            Self::EndpointTargetRejected => "compatible endpoint target was rejected",
        };
        formatter.write_str(message)
    }
}

impl Error for CompatibleEgressError {}

fn validate_id(value: &str) -> Result<(), CompatibleEgressError> {
    validate_label(value)
}

fn validate_label(value: &str) -> Result<(), CompatibleEgressError> {
    if value.trim().is_empty() {
        return Err(CompatibleEgressError::EmptyLabel);
    }
    if value != value.trim() || value.chars().any(char::is_control) {
        return Err(CompatibleEgressError::InvalidLabelShape);
    }
    if value.len() > MAX_COMPATIBLE_EGRESS_LABEL_LENGTH {
        return Err(CompatibleEgressError::LabelTooLong);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, error::Error};

    use gateway_core::{
        CredentialId, EgressPolicyId, EgressPolicyId as PolicyId, EndpointId, GatewayProtocol,
        UpstreamId,
    };

    use super::{
        CompatibleEgressError, CompatibleEgressTarget, CompatibleEndpointEgressInput,
        CompatibleEndpointEgressProfile, CompatibleFailureScope, CompatibleRetryPolicy,
        CompatibleStickiness, MAX_COMPATIBLE_EGRESS_LABEL_LENGTH,
    };
    use crate::{EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy};

    fn profile(
        upstream: &str,
        credential_kind: &str,
        target: CompatibleEgressTarget,
        scope: CompatibleFailureScope,
        stickiness: CompatibleStickiness,
    ) -> Result<CompatibleEndpointEgressProfile, Box<dyn Error>> {
        Ok(CompatibleEndpointEgressProfile::try_new(
            CompatibleEndpointEgressInput {
                upstream_id: UpstreamId::try_new(upstream)?,
                endpoint_id: EndpointId::try_new("endpoint")?,
                credential_id: CredentialId::try_new("credential")?,
                credential_kind: credential_kind.to_owned(),
                protocol: GatewayProtocol::OpenAiResponses,
                egress_policy_id: EgressPolicyId::try_new("egress-policy")?,
                target,
                failure_scope: scope,
                stickiness,
                retry_policy: CompatibleRetryPolicy::pre_submit(2)?,
            },
        )?)
    }

    fn policy() -> Result<EgressPolicy, Box<dyn Error>> {
        policy_with_id("egress-policy")
    }

    fn policy_with_id(id: &str) -> Result<EgressPolicy, Box<dyn Error>> {
        Ok(EgressPolicy::try_new(EgressPolicyInput {
            id: PolicyId::try_new(id)?,
            name: "relay-policy".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Https]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("relay.example")?]),
            allowed_ports: BTreeSet::from([443]),
            allowed_cidrs: BTreeSet::new(),
            redirect_policy: RedirectPolicy::Deny,
        })?)
    }

    #[test]
    fn relay_names_do_not_change_the_transport_profile() -> Result<(), Box<dyn Error>> {
        let krill = profile(
            "relay-krill",
            "api_key",
            CompatibleEgressTarget::Direct,
            CompatibleFailureScope::Credential,
            CompatibleStickiness::Credential,
        )?;
        let custom = profile(
            "relay-custom",
            "sub2api_json",
            CompatibleEgressTarget::Direct,
            CompatibleFailureScope::Credential,
            CompatibleStickiness::Credential,
        )?;

        assert_eq!(krill.protocol(), custom.protocol());
        assert_eq!(krill.target(), custom.target());
        assert_eq!(krill.failure_scope(), custom.failure_scope());
        assert_eq!(krill.stickiness(), custom.stickiness());
        assert_eq!(krill.retry_policy(), custom.retry_policy());
        assert!(!krill.same_binding(&custom));
        Ok(())
    }

    #[test]
    fn cpa_and_sub2api_labels_are_metadata_only() -> Result<(), Box<dyn Error>> {
        let cpa = profile(
            "relay",
            "cpa_json",
            CompatibleEgressTarget::ProxyPool {
                pool_id: "pool-a".to_owned(),
            },
            CompatibleFailureScope::EgressNode,
            CompatibleStickiness::CredentialAndEgress,
        )?;
        let sub2api = profile(
            "relay",
            "sub2api_json",
            CompatibleEgressTarget::ProxyPool {
                pool_id: "pool-a".to_owned(),
            },
            CompatibleFailureScope::EgressNode,
            CompatibleStickiness::CredentialAndEgress,
        )?;

        assert_eq!(cpa.target(), sub2api.target());
        assert_eq!(cpa.protocol(), GatewayProtocol::OpenAiResponses);
        assert_eq!(cpa.credential_kind(), "cpa_json");
        assert_eq!(sub2api.credential_kind(), "sub2api_json");
        let debug = format!("{cpa:?}");
        assert!(!debug.contains("api-key-secret"));
        assert!(!debug.contains("https://"));
        Ok(())
    }

    #[test]
    fn binding_ownership_is_exact_and_endpoint_shape_reuses_ssrf_policy()
    -> Result<(), Box<dyn Error>> {
        let profile = profile(
            "relay",
            "api_key",
            CompatibleEgressTarget::FixedProxy {
                profile_id: "proxy-a".to_owned(),
            },
            CompatibleFailureScope::Endpoint,
            CompatibleStickiness::Credential,
        )?;
        let upstream = UpstreamId::try_new("relay")?;
        let foreign = UpstreamId::try_new("foreign")?;
        assert!(profile.validate_binding(&upstream, &upstream).is_ok());
        assert_eq!(
            profile.validate_binding(&foreign, &upstream),
            Err(CompatibleEgressError::EndpointUpstreamMismatch)
        );
        assert_eq!(
            profile.validate_binding(&upstream, &foreign),
            Err(CompatibleEgressError::CredentialUpstreamMismatch)
        );

        let endpoint = profile.validate_endpoint_target(
            &policy()?,
            "https://relay.example/v1",
            "/responses",
        )?;
        assert_eq!(endpoint.as_url().path(), "/v1/responses");
        assert_eq!(
            profile.validate_endpoint_target(&policy()?, "https://other.example/v1", "/responses",),
            Err(CompatibleEgressError::EndpointTargetRejected)
        );
        assert_eq!(
            profile.validate_endpoint_target(
                &policy_with_id("other-policy")?,
                "https://relay.example/v1",
                "/responses",
            ),
            Err(CompatibleEgressError::EndpointTargetRejected)
        );
        Ok(())
    }

    #[test]
    fn invalid_retry_and_proxy_semantics_fail_closed() {
        assert_eq!(
            CompatibleRetryPolicy::pre_submit(0),
            Err(CompatibleEgressError::InvalidPreSubmitAttempts)
        );
        assert_eq!(
            CompatibleRetryPolicy::pre_submit(4),
            Err(CompatibleEgressError::InvalidPreSubmitAttempts)
        );
        let direct = profile(
            "relay",
            "api_key",
            CompatibleEgressTarget::Direct,
            CompatibleFailureScope::EgressNode,
            CompatibleStickiness::None,
        );
        assert_profile_error(direct, CompatibleEgressError::EgressNodeScopeRequiresProxy);
        let direct_sticky = profile(
            "relay",
            "api_key",
            CompatibleEgressTarget::Direct,
            CompatibleFailureScope::Endpoint,
            CompatibleStickiness::CredentialAndEgress,
        );
        assert_profile_error(
            direct_sticky,
            CompatibleEgressError::EgressStickinessRequiresProxy,
        );
    }

    #[test]
    fn labels_are_bounded_without_normalizing_opaque_ids() {
        let oversized = "x".repeat(MAX_COMPATIBLE_EGRESS_LABEL_LENGTH + 1);
        let result = profile(
            "relay",
            &oversized,
            CompatibleEgressTarget::Direct,
            CompatibleFailureScope::Endpoint,
            CompatibleStickiness::None,
        );
        assert_profile_error(result, CompatibleEgressError::LabelTooLong);
        let whitespace = profile(
            "relay",
            " api_key",
            CompatibleEgressTarget::Direct,
            CompatibleFailureScope::Endpoint,
            CompatibleStickiness::None,
        );
        assert_profile_error(whitespace, CompatibleEgressError::InvalidLabelShape);
    }

    fn assert_profile_error(
        result: Result<CompatibleEndpointEgressProfile, Box<dyn Error>>,
        expected: CompatibleEgressError,
    ) {
        assert!(
            result.is_err(),
            "profile construction unexpectedly succeeded"
        );
        let Some(error) = result.err() else {
            return;
        };
        assert_eq!(
            error.downcast_ref::<CompatibleEgressError>(),
            Some(&expected)
        );
    }
}
