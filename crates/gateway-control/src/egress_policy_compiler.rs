//! Management-time compilation of persisted `EgressPolicy` configuration.
//!
//! The compiler parses structural JSON allowlists into `gateway-upstream` values and verifies that
//! every enabled Upstream/Endpoint has a usable policy and URL shape. It intentionally does not
//! resolve DNS; the transport owner must call the compiled policy for each connection attempt.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use gateway_core::{EgressPolicyId, UpstreamId};
use gateway_store::control_plane::{
    ControlPlaneConfiguration, EgressPolicyConfiguration, StoredEgressRedirectMode,
};
use gateway_upstream::{
    EgressCidr, EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy,
};

/// Stateless compiler for one version-scoped `EgressPolicy` configuration graph.
#[derive(Clone, Copy, Debug, Default)]
pub struct EgressPolicyCompiler;

impl EgressPolicyCompiler {
    /// Creates the management-time `EgressPolicy` compiler.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles persisted policies and verifies enabled Upstream/Endpoint static admission.
    ///
    /// # Errors
    ///
    /// Returns [`EgressPolicyCompileError`] for duplicate policy identities/names, malformed or
    /// semantically invalid allowlist values, dangling policy references, or enabled endpoint URL
    /// shapes that do not meet their owning policy.
    pub fn compile(
        configuration: &ControlPlaneConfiguration,
    ) -> Result<CompiledEgressPolicies, EgressPolicyCompileError> {
        let mut policies = BTreeMap::new();
        let mut policy_names = BTreeSet::new();
        for policy in &configuration.egress_policies {
            if policies.contains_key(&policy.id) {
                return Err(egress_error(
                    EgressPolicyCompileErrorCode::DuplicatePolicy,
                    policy.id.as_str(),
                ));
            }
            if !policy_names.insert(policy.name.clone()) {
                return Err(egress_error(
                    EgressPolicyCompileErrorCode::DuplicatePolicyName,
                    policy.name.as_str(),
                ));
            }
            let compiled = compile_policy(policy)?;
            policies.insert(policy.id.clone(), compiled);
        }

        let mut upstreams = BTreeMap::new();
        let mut policy_by_upstream = BTreeMap::new();
        for upstream in &configuration.upstreams {
            if upstreams.insert(upstream.id.clone(), upstream).is_some() {
                return Err(egress_error(
                    EgressPolicyCompileErrorCode::DuplicateUpstream,
                    upstream.id.as_str(),
                ));
            }
            match &upstream.egress_policy_id {
                Some(policy_id) => {
                    if !policies.contains_key(policy_id) {
                        return Err(egress_error(
                            EgressPolicyCompileErrorCode::MissingReferencedPolicy,
                            upstream.id.as_str(),
                        ));
                    }
                    policy_by_upstream.insert(upstream.id.clone(), policy_id.clone());
                }
                None if upstream.enabled => {
                    return Err(egress_error(
                        EgressPolicyCompileErrorCode::EnabledUpstreamMissingPolicy,
                        upstream.id.as_str(),
                    ));
                }
                None => {}
            }
        }

        let mut endpoint_ids = BTreeSet::new();
        for endpoint in &configuration.endpoints {
            if !endpoint_ids.insert(endpoint.id.clone()) {
                return Err(egress_error(
                    EgressPolicyCompileErrorCode::DuplicateEndpoint,
                    endpoint.id.as_str(),
                ));
            }
            let upstream = upstreams.get(&endpoint.upstream_id).ok_or_else(|| {
                egress_error(
                    EgressPolicyCompileErrorCode::MissingEndpointUpstream,
                    endpoint.id.as_str(),
                )
            })?;
            if !endpoint.enabled || !upstream.enabled {
                continue;
            }
            let policy_id = policy_by_upstream
                .get(&endpoint.upstream_id)
                .ok_or_else(|| {
                    egress_error(
                        EgressPolicyCompileErrorCode::EnabledUpstreamMissingPolicy,
                        upstream.id.as_str(),
                    )
                })?;
            let policy = policies.get(policy_id).ok_or_else(|| {
                egress_error(
                    EgressPolicyCompileErrorCode::MissingReferencedPolicy,
                    upstream.id.as_str(),
                )
            })?;
            policy.validate_url_shape(&endpoint.base_url).map_err(|_| {
                egress_error(
                    EgressPolicyCompileErrorCode::EndpointUrlRejected,
                    endpoint.id.as_str(),
                )
            })?;
        }

        Ok(CompiledEgressPolicies {
            policies,
            policy_by_upstream,
        })
    }
}

/// Immutable version-scoped `EgressPolicies` indexed by their owning Upstream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledEgressPolicies {
    policies: BTreeMap<EgressPolicyId, EgressPolicy>,
    policy_by_upstream: BTreeMap<UpstreamId, EgressPolicyId>,
}

impl CompiledEgressPolicies {
    /// Returns the compiled policy referenced by one Upstream.
    #[must_use]
    pub fn policy_for_upstream(&self, upstream_id: &UpstreamId) -> Option<&EgressPolicy> {
        self.policy_by_upstream
            .get(upstream_id)
            .and_then(|policy_id| self.policies.get(policy_id))
    }

    /// Returns the compiled policy by its stable version-scoped identity.
    #[must_use]
    pub fn policy(&self, policy_id: &EgressPolicyId) -> Option<&EgressPolicy> {
        self.policies.get(policy_id)
    }

    /// Iterates the compiled policies in stable identifier order.
    pub fn policies(&self) -> impl Iterator<Item = &EgressPolicy> {
        self.policies.values()
    }
}

/// Stable safe error code for `EgressPolicy` configuration compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EgressPolicyCompileErrorCode {
    /// More than one policy used the same stable identity.
    DuplicatePolicy,
    /// More than one policy used the same non-secret name.
    DuplicatePolicyName,
    /// More than one Upstream used the same stable identity.
    DuplicateUpstream,
    /// More than one Endpoint used the same stable identity.
    DuplicateEndpoint,
    /// A JSON allowlist did not have the required array shape.
    InvalidPolicyJson,
    /// A parsed scheme, Host, port, CIDR, redirect setting, or policy invariant was invalid.
    InvalidPolicyValue,
    /// An enabled Upstream did not declare an `EgressPolicy` reference.
    EnabledUpstreamMissingPolicy,
    /// A non-null Upstream policy reference did not resolve in the same Config Version.
    MissingReferencedPolicy,
    /// An Endpoint referred to an absent Upstream.
    MissingEndpointUpstream,
    /// An enabled Endpoint Base URL failed its owning policy's static admission.
    EndpointUrlRejected,
}

impl EgressPolicyCompileErrorCode {
    /// Returns the stable machine-readable lower-snake-case error code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DuplicatePolicy => "duplicate_egress_policy",
            Self::DuplicatePolicyName => "duplicate_egress_policy_name",
            Self::DuplicateUpstream => "duplicate_upstream",
            Self::DuplicateEndpoint => "duplicate_endpoint",
            Self::InvalidPolicyJson => "invalid_egress_policy_json",
            Self::InvalidPolicyValue => "invalid_egress_policy_value",
            Self::EnabledUpstreamMissingPolicy => "enabled_upstream_missing_egress_policy",
            Self::MissingReferencedPolicy => "missing_egress_policy",
            Self::MissingEndpointUpstream => "missing_endpoint_upstream",
            Self::EndpointUrlRejected => "endpoint_egress_url_rejected",
        }
    }
}

/// A safe `EgressPolicy` compilation error with a non-secret stable subject only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EgressPolicyCompileError {
    code: EgressPolicyCompileErrorCode,
    subject: String,
}

impl EgressPolicyCompileError {
    /// Returns the stable failure code.
    #[must_use]
    pub const fn code(&self) -> EgressPolicyCompileErrorCode {
        self.code
    }

    /// Returns the non-secret stable entity subject.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }
}

impl fmt::Display for EgressPolicyCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "EgressPolicy compilation failed: {} ({})",
            self.code.as_str(),
            self.subject
        )
    }
}

impl Error for EgressPolicyCompileError {}

fn compile_policy(
    configuration: &EgressPolicyConfiguration,
) -> Result<EgressPolicy, EgressPolicyCompileError> {
    let allowed_schemes = parse_schemes(configuration)?;
    let allowed_hosts = parse_hosts(configuration)?;
    let allowed_ports = parse_ports(configuration)?;
    let allowed_cidrs = parse_cidrs(configuration)?;
    let redirect_policy = parse_redirect_policy(configuration)?;
    EgressPolicy::try_new(EgressPolicyInput {
        id: configuration.id.clone(),
        name: configuration.name.clone(),
        allowed_schemes,
        allowed_hosts,
        allowed_ports,
        allowed_cidrs,
        redirect_policy,
    })
    .map_err(|_| {
        egress_error(
            EgressPolicyCompileErrorCode::InvalidPolicyValue,
            configuration.id.as_str(),
        )
    })
}

fn parse_schemes(
    configuration: &EgressPolicyConfiguration,
) -> Result<BTreeSet<EgressScheme>, EgressPolicyCompileError> {
    let values = parse_string_array(&configuration.allowed_schemes_json, configuration)?;
    let mut schemes = BTreeSet::new();
    for value in values {
        let scheme = EgressScheme::try_from_url_scheme(&value).map_err(|_| {
            egress_error(
                EgressPolicyCompileErrorCode::InvalidPolicyValue,
                configuration.id.as_str(),
            )
        })?;
        if !schemes.insert(scheme) {
            return Err(egress_error(
                EgressPolicyCompileErrorCode::InvalidPolicyValue,
                configuration.id.as_str(),
            ));
        }
    }
    Ok(schemes)
}

fn parse_hosts(
    configuration: &EgressPolicyConfiguration,
) -> Result<BTreeSet<EgressHost>, EgressPolicyCompileError> {
    let values = parse_string_array(&configuration.allowed_hosts_json, configuration)?;
    let mut hosts = BTreeSet::new();
    for value in values {
        let host = EgressHost::try_new(&value).map_err(|_| {
            egress_error(
                EgressPolicyCompileErrorCode::InvalidPolicyValue,
                configuration.id.as_str(),
            )
        })?;
        if !hosts.insert(host) {
            return Err(egress_error(
                EgressPolicyCompileErrorCode::InvalidPolicyValue,
                configuration.id.as_str(),
            ));
        }
    }
    Ok(hosts)
}

fn parse_ports(
    configuration: &EgressPolicyConfiguration,
) -> Result<BTreeSet<u16>, EgressPolicyCompileError> {
    let values =
        serde_json::from_str::<Vec<u16>>(&configuration.allowed_ports_json).map_err(|_| {
            egress_error(
                EgressPolicyCompileErrorCode::InvalidPolicyJson,
                configuration.id.as_str(),
            )
        })?;
    let mut ports = BTreeSet::new();
    for value in values {
        if !ports.insert(value) {
            return Err(egress_error(
                EgressPolicyCompileErrorCode::InvalidPolicyValue,
                configuration.id.as_str(),
            ));
        }
    }
    Ok(ports)
}

fn parse_cidrs(
    configuration: &EgressPolicyConfiguration,
) -> Result<BTreeSet<EgressCidr>, EgressPolicyCompileError> {
    let values = parse_string_array(&configuration.allowed_cidrs_json, configuration)?;
    let mut cidrs = BTreeSet::new();
    for value in values {
        let cidr = EgressCidr::try_parse(&value).map_err(|_| {
            egress_error(
                EgressPolicyCompileErrorCode::InvalidPolicyValue,
                configuration.id.as_str(),
            )
        })?;
        if !cidrs.insert(cidr) {
            return Err(egress_error(
                EgressPolicyCompileErrorCode::InvalidPolicyValue,
                configuration.id.as_str(),
            ));
        }
    }
    Ok(cidrs)
}

fn parse_string_array(
    encoded: &str,
    configuration: &EgressPolicyConfiguration,
) -> Result<Vec<String>, EgressPolicyCompileError> {
    serde_json::from_str(encoded).map_err(|_| {
        egress_error(
            EgressPolicyCompileErrorCode::InvalidPolicyJson,
            configuration.id.as_str(),
        )
    })
}

fn parse_redirect_policy(
    configuration: &EgressPolicyConfiguration,
) -> Result<RedirectPolicy, EgressPolicyCompileError> {
    let max_redirects = u8::try_from(configuration.max_redirects).map_err(|_| {
        egress_error(
            EgressPolicyCompileErrorCode::InvalidPolicyValue,
            configuration.id.as_str(),
        )
    })?;
    let redirect_policy = match configuration.redirect_mode {
        StoredEgressRedirectMode::Deny if max_redirects == 0 => RedirectPolicy::Deny,
        StoredEgressRedirectMode::SameOrigin => RedirectPolicy::SameOrigin { max_redirects },
        StoredEgressRedirectMode::Revalidate => RedirectPolicy::Revalidate { max_redirects },
        StoredEgressRedirectMode::Deny => {
            return Err(egress_error(
                EgressPolicyCompileErrorCode::InvalidPolicyValue,
                configuration.id.as_str(),
            ));
        }
    };
    Ok(redirect_policy)
}

fn egress_error(code: EgressPolicyCompileErrorCode, subject: &str) -> EgressPolicyCompileError {
    EgressPolicyCompileError {
        code,
        subject: subject.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{EgressPolicyId, EndpointId, UpstreamId};
    use gateway_store::control_plane::{
        ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
        EgressPolicyConfiguration, EndpointConfiguration, EndpointTransport,
        StoredEgressRedirectMode, UpstreamConfiguration,
    };

    use super::{EgressPolicyCompileErrorCode, EgressPolicyCompiler};

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn compiles_an_enabled_upstream_with_a_valid_exact_policy() -> TestResult {
        let configuration = configuration(Some("egress-a"), "https://api.example.test/v1")?;
        let compiled = EgressPolicyCompiler::compile(&configuration)?;
        let upstream_id = UpstreamId::try_new("upstream-a")?;
        let policy = compiled
            .policy_for_upstream(&upstream_id)
            .ok_or("expected compiled upstream egress policy")?;
        assert_eq!(policy.id().as_str(), "egress-a");
        assert_eq!(compiled.policies().count(), 1);
        Ok(())
    }

    #[test]
    fn rejects_missing_and_dangling_policy_references_before_endpoint_use() -> TestResult {
        let missing = configuration(None, "https://api.example.test/v1")?;
        assert_compile_error(
            EgressPolicyCompiler::compile(&missing),
            EgressPolicyCompileErrorCode::EnabledUpstreamMissingPolicy,
        )?;

        let dangling = configuration(Some("missing-policy"), "https://api.example.test/v1")?;
        assert_compile_error(
            EgressPolicyCompiler::compile(&dangling),
            EgressPolicyCompileErrorCode::MissingReferencedPolicy,
        )?;
        Ok(())
    }

    #[test]
    fn rejects_malformed_policy_values_and_endpoint_urls() -> TestResult {
        let mut malformed = configuration(Some("egress-a"), "https://api.example.test/v1")?;
        malformed.egress_policies[0].allowed_cidrs_json = r#"["not-a-cidr"]"#.to_owned();
        assert_compile_error(
            EgressPolicyCompiler::compile(&malformed),
            EgressPolicyCompileErrorCode::InvalidPolicyValue,
        )?;

        let rejected_url = configuration(Some("egress-a"), "http://api.example.test/v1")?;
        assert_compile_error(
            EgressPolicyCompiler::compile(&rejected_url),
            EgressPolicyCompileErrorCode::EndpointUrlRejected,
        )?;
        Ok(())
    }

    fn configuration(
        upstream_policy_id: Option<&str>,
        base_url: &str,
    ) -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
        let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
            id: ConfigVersionId::try_new("egress-version")?,
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 1,
            description: "egress compiler test".to_owned(),
        });
        configuration
            .egress_policies
            .push(EgressPolicyConfiguration {
                id: EgressPolicyId::try_new("egress-a")?,
                name: "default".to_owned(),
                allowed_schemes_json: r#"["https"]"#.to_owned(),
                allowed_hosts_json: r#"["api.example.test"]"#.to_owned(),
                allowed_ports_json: "[443]".to_owned(),
                allowed_cidrs_json: "[]".to_owned(),
                redirect_mode: StoredEgressRedirectMode::Deny,
                max_redirects: 0,
            });
        configuration.upstreams.push(UpstreamConfiguration {
            id: UpstreamId::try_new("upstream-a")?,
            name: "upstream".to_owned(),
            kind: "openai-compatible".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: upstream_policy_id
                .map(EgressPolicyId::try_new)
                .transpose()?,
        });
        configuration.endpoints.push(EndpointConfiguration {
            id: EndpointId::try_new("endpoint-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            adapter_id: "openai-compatible.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: base_url.to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: Some("/models".to_owned()),
            transport: EndpointTransport::Http,
            enabled: true,
        });
        Ok(configuration)
    }

    fn assert_compile_error(
        result: Result<super::CompiledEgressPolicies, super::EgressPolicyCompileError>,
        expected: EgressPolicyCompileErrorCode,
    ) -> TestResult {
        let Err(error) = result else {
            return Err("egress policy compilation unexpectedly succeeded".into());
        };
        assert_eq!(error.code(), expected);
        Ok(())
    }
}
