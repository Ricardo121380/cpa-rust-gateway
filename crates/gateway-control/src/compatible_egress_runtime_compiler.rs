//! Active Config-Version composition for generic compatible Endpoints.
//!
//! This is the control-plane side of P13-11B. It joins one already validated configuration
//! graph to the existing decrypted Credential pools, compiled [`EgressPolicy`] values, and the
//! shared runtime Health/Quota registries. The compiler never decrypts, resolves DNS, opens a
//! socket, or contacts a Provider. Native Provider adapters are deliberately left to their own
//! composition paths.

use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

use gateway_core::{CredentialId, EndpointId, GatewayProtocol, UpstreamId};
use gateway_protocol::ApiFormat;
use gateway_router::{
    CompatibleEgressTransportRegistry, CompatibleEndpointRuntime,
    CompatibleEndpointRuntimeBindingInput, CompatibleEndpointRuntimeBuildError,
    CompatibleEndpointRuntimeInput, RuntimeHealthRegistry, RuntimeQuotaRegistry, SnapshotVersion,
};
use gateway_store::control_plane::{
    ConfigVersionStatus, ControlPlaneConfiguration, CredentialStatus, EndpointConfiguration,
    EndpointCredentialBindingConfiguration, UpstreamConfiguration,
};
use gateway_upstream::{
    CompatibleEgressTarget, CompatibleEndpointEgressInput, CompatibleEndpointEgressProfile,
    CompatibleFailureScope, CompatibleRetryPolicy, CompatibleStickiness, EndpointCredentialPools,
};

use crate::egress_policy_compiler::{CompiledEgressPolicies, EgressPolicyCompiler};

/// The generic adapter families composed by this slice.
const GENERIC_ADAPTERS: [&str; 3] = [
    "openai-compatible.chat-completions",
    "openai-compatible.responses",
    "anthropic-compatible.messages",
];

/// Maximum number of generic Endpoint runtimes emitted by one composition.
pub const MAX_COMPATIBLE_ENDPOINT_RUNTIMES: usize = 256;

/// Per-binding runtime settings supplied by the deployment composition.
///
/// The durable control-plane schema does not yet carry proxy-pool membership. Keeping these
/// settings explicit at the composition boundary lets a later config field be added without
/// changing the runtime contract or inferring proxy behavior from a Provider name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibleEndpointBindingRuntimeSettings {
    /// Direct, fixed-proxy, or named proxy-pool target.
    pub target: CompatibleEgressTarget,
    /// Identity that receives transport-failure state.
    pub failure_scope: CompatibleFailureScope,
    /// Optional account/egress stickiness.
    pub stickiness: CompatibleStickiness,
    /// Bounded ordinary pre-submit retry policy.
    pub retry_policy: CompatibleRetryPolicy,
}

impl CompatibleEndpointBindingRuntimeSettings {
    /// The conservative default for a generic credential with no proxy override.
    #[must_use]
    pub const fn direct() -> Self {
        Self {
            target: CompatibleEgressTarget::Direct,
            failure_scope: CompatibleFailureScope::Credential,
            stickiness: CompatibleStickiness::Credential,
            retry_policy: CompatibleRetryPolicy::None,
        }
    }
}

/// Build-time failures for the active generic Endpoint composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleEgressRuntimeCompileError {
    /// The Config Version identity could not enter the Router snapshot type.
    InvalidSnapshotVersion,
    /// The supplied graph is not the active Config Version used by serving.
    ConfigVersionNotActive,
    /// The graph has more generic Endpoint runtimes than the finite bound.
    TooManyEndpoints,
    /// An Endpoint references an unknown Upstream.
    MissingUpstream,
    /// An enabled generic Endpoint has no compiled policy.
    MissingEgressPolicy,
    /// The supplied compiled policies do not belong to this exact configuration graph.
    EgressPolicySnapshotMismatch,
    /// The owning Upstream has no dedicated transport registry.
    MissingTransportRegistry,
    /// The stored API format/adapter pair is not a generic family in this slice.
    UnsupportedGenericEndpoint,
    /// The existing Credential pool has no active Endpoint pool.
    MissingEndpointPool,
    /// An enabled binding references an unknown Credential.
    MissingCredential,
    /// A binding crossed its Endpoint/Credential Upstream boundary.
    BindingUpstreamMismatch,
    /// A binding's persisted numeric scheduling value is outside the runtime range.
    InvalidBindingSchedule,
    /// Existing pool revision or scheduling metadata drifted from the selected graph.
    RuntimePoolDrift,
    /// A transport registry was reused across Upstream/Provider ownership.
    TransportRegistryOwnerMismatch,
    /// The selected transport target was not registered by the deployment.
    UnknownTransportTarget,
    /// One generic Endpoint could not be composed into a runtime object.
    RuntimeBuild,
}

impl fmt::Display for CompatibleEgressRuntimeCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidSnapshotVersion => "compatible egress snapshot version is invalid",
            Self::ConfigVersionNotActive => "compatible egress config version is not active",
            Self::TooManyEndpoints => "compatible egress endpoint count exceeds its bound",
            Self::MissingUpstream => "compatible egress endpoint upstream is missing",
            Self::MissingEgressPolicy => "compatible egress endpoint policy is missing",
            Self::EgressPolicySnapshotMismatch => "compatible egress policies drifted from config",
            Self::MissingTransportRegistry => {
                "compatible egress upstream transport registry is missing"
            }
            Self::UnsupportedGenericEndpoint => "endpoint is not a supported generic adapter",
            Self::MissingEndpointPool => "generic endpoint credential pool is missing",
            Self::MissingCredential => "generic endpoint credential is missing",
            Self::BindingUpstreamMismatch => "generic endpoint binding crosses upstream boundary",
            Self::InvalidBindingSchedule => "generic endpoint binding schedule is invalid",
            Self::RuntimePoolDrift => "generic endpoint runtime pool drifted from config",
            Self::TransportRegistryOwnerMismatch => {
                "generic endpoint transport registry crosses upstream ownership"
            }
            Self::UnknownTransportTarget => "generic endpoint transport target is unknown",
            Self::RuntimeBuild => "generic endpoint runtime construction failed",
        };
        formatter.write_str(message)
    }
}

impl Error for CompatibleEgressRuntimeCompileError {}

/// Compiles all enabled generic compatible Endpoints from one active graph.
pub struct CompatibleEgressRuntimeCompiler<'a> {
    configuration: &'a ControlPlaneConfiguration,
    policies: &'a CompiledEgressPolicies,
    credential_pools: Arc<EndpointCredentialPools>,
    runtime_health: Arc<RuntimeHealthRegistry>,
    runtime_quota: Arc<RuntimeQuotaRegistry>,
    transport_registries: BTreeMap<UpstreamId, CompatibleEgressTransportRegistry>,
    target_settings: BTreeMap<(EndpointId, CredentialId), CompatibleEndpointBindingRuntimeSettings>,
}

impl<'a> CompatibleEgressRuntimeCompiler<'a> {
    /// Creates a compiler for one selected Config Version and one shared runtime pool graph.
    #[must_use]
    pub fn new(
        configuration: &'a ControlPlaneConfiguration,
        policies: &'a CompiledEgressPolicies,
        credential_pools: Arc<EndpointCredentialPools>,
        runtime_health: Arc<RuntimeHealthRegistry>,
        runtime_quota: Arc<RuntimeQuotaRegistry>,
        transport_registries: BTreeMap<UpstreamId, CompatibleEgressTransportRegistry>,
        target_settings: BTreeMap<
            (EndpointId, CredentialId),
            CompatibleEndpointBindingRuntimeSettings,
        >,
    ) -> Self {
        Self {
            configuration,
            policies,
            credential_pools,
            runtime_health,
            runtime_quota,
            transport_registries,
            target_settings,
        }
    }

    /// Compiles every enabled generic Endpoint in deterministic ID order.
    ///
    /// Disabled Endpoints, disabled Upstreams, inactive Credentials, and disabled bindings are
    /// omitted exactly as they are from [`gateway_control::credential_pool_compiler`]. Native
    /// Grok/Kiro/other Provider-specific adapter families are not coerced into this generic path.
    ///
    /// # Errors
    ///
    /// Returns a closed error before publishing any partial runtime map.
    pub fn compile(
        &self,
    ) -> Result<BTreeMap<EndpointId, CompatibleEndpointRuntime>, CompatibleEgressRuntimeCompileError>
    {
        if self.configuration.version.status != ConfigVersionStatus::Active {
            return Err(CompatibleEgressRuntimeCompileError::ConfigVersionNotActive);
        }
        let expected_policies = EgressPolicyCompiler::compile(self.configuration)
            .map_err(|_| CompatibleEgressRuntimeCompileError::EgressPolicySnapshotMismatch)?;
        if &expected_policies != self.policies {
            return Err(CompatibleEgressRuntimeCompileError::EgressPolicySnapshotMismatch);
        }
        let snapshot_version = SnapshotVersion::try_new(self.configuration.version.id.as_str())
            .map_err(|_| CompatibleEgressRuntimeCompileError::InvalidSnapshotVersion)?;
        let upstreams = index_upstreams(&self.configuration.upstreams);
        let credentials = self
            .configuration
            .credentials
            .iter()
            .map(|credential| (credential.id.clone(), credential))
            .collect::<BTreeMap<_, _>>();
        let bindings = index_bindings(&self.configuration.endpoint_credential_bindings);
        let mut runtimes = BTreeMap::new();

        let mut endpoints = self
            .configuration
            .endpoints
            .iter()
            .filter(|endpoint| endpoint.enabled)
            .collect::<Vec<_>>();
        endpoints.sort_by(|left, right| left.id.cmp(&right.id));
        for endpoint in endpoints {
            let Some(upstream) = upstreams.get(&endpoint.upstream_id) else {
                return Err(CompatibleEgressRuntimeCompileError::MissingUpstream);
            };
            if !upstream.enabled {
                continue;
            }
            let Some(runtime) = self.compile_endpoint(
                endpoint,
                snapshot_version.clone(),
                upstream,
                &credentials,
                &bindings,
            )?
            else {
                continue;
            };
            if runtimes.len() >= MAX_COMPATIBLE_ENDPOINT_RUNTIMES {
                return Err(CompatibleEgressRuntimeCompileError::TooManyEndpoints);
            }
            if runtimes.insert(endpoint.id.clone(), runtime).is_some() {
                return Err(CompatibleEgressRuntimeCompileError::RuntimeBuild);
            }
        }
        Ok(runtimes)
    }

    fn compile_endpoint(
        &self,
        endpoint: &EndpointConfiguration,
        snapshot_version: SnapshotVersion,
        upstream: &UpstreamConfiguration,
        credentials: &BTreeMap<
            CredentialId,
            &gateway_store::control_plane::CredentialConfiguration,
        >,
        bindings: &BTreeMap<EndpointId, Vec<EndpointCredentialBindingConfiguration>>,
    ) -> Result<Option<CompatibleEndpointRuntime>, CompatibleEgressRuntimeCompileError> {
        let Some(format) = ApiFormat::parse(&endpoint.api_format) else {
            return Err(CompatibleEgressRuntimeCompileError::UnsupportedGenericEndpoint);
        };
        if !format.serves(&endpoint.adapter_id) {
            return Err(CompatibleEgressRuntimeCompileError::UnsupportedGenericEndpoint);
        }
        let Some(protocol) = generic_protocol(endpoint) else {
            // Native and Provider-specific adapters remain on their dedicated composition path.
            return Ok(None);
        };
        let policy = self
            .policies
            .policy_for_upstream(&endpoint.upstream_id)
            .ok_or(CompatibleEgressRuntimeCompileError::MissingEgressPolicy)?;
        let transport_registry = self
            .transport_registries
            .get(&endpoint.upstream_id)
            .cloned()
            .ok_or(CompatibleEgressRuntimeCompileError::MissingTransportRegistry)?;
        if transport_registry.owner_upstream_id() != &endpoint.upstream_id {
            return Err(CompatibleEgressRuntimeCompileError::TransportRegistryOwnerMismatch);
        }
        let mut runtime_bindings = Vec::new();
        for binding in bindings.get(&endpoint.id).into_iter().flatten() {
            let Some(credential) = credentials.get(&binding.credential_id) else {
                return Err(CompatibleEgressRuntimeCompileError::MissingCredential);
            };
            if binding.upstream_id != endpoint.upstream_id
                || credential.upstream_id != endpoint.upstream_id
                || upstream.id != endpoint.upstream_id
            {
                return Err(CompatibleEgressRuntimeCompileError::BindingUpstreamMismatch);
            }
            if !binding.enabled || credential.status != CredentialStatus::Active {
                continue;
            }
            if binding.priority < 0 || binding.weight <= 0 || binding.concurrency <= 0 {
                return Err(CompatibleEgressRuntimeCompileError::InvalidBindingSchedule);
            }
            let credential_revision = u64::try_from(credential.revision)
                .map_err(|_| CompatibleEgressRuntimeCompileError::InvalidBindingSchedule)?;
            let weight = usize::try_from(binding.weight)
                .map_err(|_| CompatibleEgressRuntimeCompileError::InvalidBindingSchedule)?;
            let maximum_concurrency = usize::try_from(binding.concurrency)
                .map_err(|_| CompatibleEgressRuntimeCompileError::InvalidBindingSchedule)?;
            let settings = self
                .target_settings
                .get(&(endpoint.id.clone(), binding.credential_id.clone()))
                .cloned()
                .unwrap_or_else(CompatibleEndpointBindingRuntimeSettings::direct);
            let profile = CompatibleEndpointEgressProfile::try_new(CompatibleEndpointEgressInput {
                upstream_id: endpoint.upstream_id.clone(),
                endpoint_id: endpoint.id.clone(),
                credential_id: binding.credential_id.clone(),
                credential_kind: credential.kind.clone(),
                protocol,
                egress_policy_id: policy.id().clone(),
                target: settings.target,
                failure_scope: settings.failure_scope,
                stickiness: settings.stickiness,
                retry_policy: settings.retry_policy,
            })
            .map_err(|_| CompatibleEgressRuntimeCompileError::RuntimeBuild)?;
            if !transport_registry.contains_target(profile.target()) {
                return Err(CompatibleEgressRuntimeCompileError::UnknownTransportTarget);
            }
            runtime_bindings.push(CompatibleEndpointRuntimeBindingInput {
                profile,
                endpoint_upstream_id: endpoint.upstream_id.clone(),
                credential_upstream_id: credential.upstream_id.clone(),
                credential_revision,
                priority: binding.priority,
                weight,
                maximum_concurrency,
            });
        }
        if runtime_bindings.is_empty() {
            if self.credential_pools.pool(&endpoint.id).is_some() {
                return Err(CompatibleEgressRuntimeCompileError::MissingEndpointPool);
            }
            return Ok(None);
        }
        CompatibleEndpointRuntime::try_new(CompatibleEndpointRuntimeInput {
            snapshot_version,
            bindings: runtime_bindings,
            egress_policy: Arc::new(policy.clone()),
            base_url: endpoint.base_url.clone(),
            inference_path: endpoint.inference_path.clone(),
            credential_pools: Arc::clone(&self.credential_pools),
            runtime_health: Arc::clone(&self.runtime_health),
            runtime_quota: Arc::clone(&self.runtime_quota),
            transport_registry,
        })
        .map(Some)
        .map_err(map_runtime_build_error)
    }
}

fn generic_protocol(endpoint: &EndpointConfiguration) -> Option<GatewayProtocol> {
    let format = ApiFormat::parse(&endpoint.api_format)?;
    let expected = match format {
        ApiFormat::OpenAiChatCompletions if endpoint.adapter_id == GENERIC_ADAPTERS[0] => {
            GatewayProtocol::OpenAiChatCompletions
        }
        ApiFormat::OpenAiResponses if endpoint.adapter_id == GENERIC_ADAPTERS[1] => {
            GatewayProtocol::OpenAiResponses
        }
        ApiFormat::AnthropicMessages if endpoint.adapter_id == GENERIC_ADAPTERS[2] => {
            GatewayProtocol::AnthropicMessages
        }
        _ => return None,
    };
    Some(expected)
}

fn index_upstreams(
    upstreams: &[UpstreamConfiguration],
) -> BTreeMap<UpstreamId, &UpstreamConfiguration> {
    upstreams
        .iter()
        .map(|upstream| (upstream.id.clone(), upstream))
        .collect()
}

fn index_bindings(
    bindings: &[EndpointCredentialBindingConfiguration],
) -> BTreeMap<EndpointId, Vec<EndpointCredentialBindingConfiguration>> {
    let mut indexed: BTreeMap<EndpointId, Vec<_>> = BTreeMap::new();
    for binding in bindings {
        indexed
            .entry(binding.endpoint_id.clone())
            .or_default()
            .push(binding.clone());
    }
    indexed
}

fn map_runtime_build_error(
    error: CompatibleEndpointRuntimeBuildError,
) -> CompatibleEgressRuntimeCompileError {
    match error {
        CompatibleEndpointRuntimeBuildError::MissingEndpointPool
        | CompatibleEndpointRuntimeBuildError::MissingCredentialPoolEntry => {
            CompatibleEgressRuntimeCompileError::MissingEndpointPool
        }
        CompatibleEndpointRuntimeBuildError::UnknownTransportTarget => {
            CompatibleEgressRuntimeCompileError::UnknownTransportTarget
        }
        CompatibleEndpointRuntimeBuildError::TransportRegistryOwnerMismatch => {
            CompatibleEgressRuntimeCompileError::TransportRegistryOwnerMismatch
        }
        CompatibleEndpointRuntimeBuildError::CredentialRevisionMismatch
        | CompatibleEndpointRuntimeBuildError::CredentialScheduleMismatch => {
            CompatibleEgressRuntimeCompileError::RuntimePoolDrift
        }
        CompatibleEndpointRuntimeBuildError::EndpointUpstreamMismatch
        | CompatibleEndpointRuntimeBuildError::CredentialUpstreamMismatch
        | CompatibleEndpointRuntimeBuildError::MixedUpstream
        | CompatibleEndpointRuntimeBuildError::MixedEndpoint => {
            CompatibleEgressRuntimeCompileError::BindingUpstreamMismatch
        }
        _ => CompatibleEgressRuntimeCompileError::RuntimeBuild,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, error::Error, num::NonZeroUsize, sync::Arc, time::Duration};

    use gateway_core::{CredentialId, EgressPolicyId, EndpointId, UpstreamId};
    use gateway_store::{
        control_plane::{
            ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialConfiguration, CredentialStatus, EgressPolicyConfiguration,
            EndpointConfiguration, EndpointCredentialBindingConfiguration, EndpointTransport,
            StoredEgressRedirectMode, UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };
    use gateway_upstream::{
        CompatibleEgressTarget, CompatibleFailureScope, CompatibleRetryPolicy,
        CompatibleStickiness, CredentialSecret, EndpointCredentialInput, EndpointCredentialPool,
        EndpointCredentialPools, UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
    };

    use super::{CompatibleEgressRuntimeCompiler, CompatibleEndpointBindingRuntimeSettings};
    use crate::egress_policy_compiler::EgressPolicyCompiler;
    use gateway_router::{CompatibleEgressNodeInput, CompatibleProxyPoolInput};
    use gateway_router::{
        CompatibleEgressTransportRegistry, CompatibleEgressTransportRegistryInput,
        RuntimeHealthRegistry, RuntimeQuotaRegistry,
    };

    fn secret_store() -> Result<SecretStore, Box<dyn Error>> {
        let version = KeyVersion::try_new(1)?;
        Ok(SecretStore::new(MasterKeyRing::try_new(
            version,
            [(version, MasterKey::try_from_bytes([0x11_u8; 32])?)],
        )?))
    }

    fn config(store: &SecretStore) -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
        let version = ConfigVersion {
            id: ConfigVersionId::try_new("active-v1")?,
            parent_id: None,
            status: ConfigVersionStatus::Active,
            revision: 7,
            created_at_ms: 1,
            description: "test".to_owned(),
        };
        let mut config = ControlPlaneConfiguration::new(version);
        let upstream = UpstreamId::try_new("provider-a")?;
        let endpoint = EndpointId::try_new("channel-a")?;
        let credential = CredentialId::try_new("account-a")?;
        let policy = EgressPolicyId::try_new("policy-a")?;
        config.egress_policies.push(EgressPolicyConfiguration {
            id: policy.clone(),
            name: "policy".to_owned(),
            allowed_schemes_json: r#"["https"]"#.to_owned(),
            allowed_hosts_json: r#"["relay.example"]"#.to_owned(),
            allowed_ports_json: "[443]".to_owned(),
            allowed_cidrs_json: "[]".to_owned(),
            redirect_mode: StoredEgressRedirectMode::Deny,
            max_redirects: 0,
        });
        config.upstreams.push(UpstreamConfiguration {
            id: upstream.clone(),
            name: "provider".to_owned(),
            kind: "compatible".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: Some(policy),
        });
        config.endpoints.push(EndpointConfiguration {
            id: endpoint.clone(),
            upstream_id: upstream.clone(),
            adapter_id: "openai-compatible.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: "https://relay.example/v1".to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: None,
            transport: EndpointTransport::Http,
            enabled: true,
        });
        let aad = crate::control_plane_service::credential_associated_data(
            &config.version.id,
            &credential,
            &upstream,
        )?;
        config.credentials.push(CredentialConfiguration {
            id: credential.clone(),
            upstream_id: upstream.clone(),
            kind: "sub2api_json".to_owned(),
            encrypted_secret: store.seal(b"secret", &aad)?,
            status: CredentialStatus::Active,
            revision: 1,
        });
        config
            .endpoint_credential_bindings
            .push(EndpointCredentialBindingConfiguration {
                endpoint_id: endpoint,
                credential_id: credential,
                upstream_id: upstream,
                enabled: true,
                priority: 0,
                weight: 1,
                concurrency: 1,
            });
        Ok(config)
    }

    fn pools() -> Result<Arc<EndpointCredentialPools>, Box<dyn Error>> {
        let endpoint = EndpointId::try_new("channel-a")?;
        Ok(Arc::new(EndpointCredentialPools::try_new([
            EndpointCredentialPool::try_new(
                endpoint,
                [EndpointCredentialInput {
                    credential_id: CredentialId::try_new("account-a")?,
                    credential_kind: "sub2api_json".to_owned(),
                    credential_revision: 1,
                    priority: 0,
                    weight: 1,
                    concurrency: 1,
                    expires_at_ms: None,
                    secret: CredentialSecret::try_new(b"secret".to_vec())?,
                }],
            )?,
        ])?))
    }

    fn registry(owner: &str) -> Result<CompatibleEgressTransportRegistry, Box<dyn Error>> {
        let timeouts = UpstreamTimeouts::try_new(
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(3),
            Duration::from_secs(4),
        )?;
        let direct = UpstreamTransportProfile::new(
            timeouts,
            UpstreamProxy::Direct,
            NonZeroUsize::new(1).ok_or("nonzero")?,
        );
        let socks = UpstreamTransportProfile::new(
            timeouts,
            UpstreamProxy::try_socks5("socks5://127.0.0.1:19081")?,
            NonZeroUsize::new(1).ok_or("nonzero")?,
        );
        Ok(CompatibleEgressTransportRegistry::try_new(
            CompatibleEgressTransportRegistryInput {
                owner_upstream_id: UpstreamId::try_new(owner)?,
                direct_profile: direct,
                fixed_proxies: Vec::new(),
                proxy_pools: vec![CompatibleProxyPoolInput {
                    pool_id: "pool-a".to_owned(),
                    nodes: vec![CompatibleEgressNodeInput {
                        node_id: "node-a".to_owned(),
                        transport_profile: socks,
                        weight: 1,
                        maximum_concurrency: 1,
                    }],
                }],
            },
        )?)
    }

    #[test]
    fn compiles_generic_endpoint_with_shared_pool_and_direct_default() -> Result<(), Box<dyn Error>>
    {
        let store = secret_store()?;
        let config = config(&store)?;
        let policies = EgressPolicyCompiler::compile(&config)?;
        let compiler = CompatibleEgressRuntimeCompiler::new(
            &config,
            &policies,
            pools()?,
            Arc::new(RuntimeHealthRegistry::new()),
            Arc::new(RuntimeQuotaRegistry::new()),
            BTreeMap::from([(UpstreamId::try_new("provider-a")?, registry("provider-a")?)]),
            BTreeMap::new(),
        );
        let runtimes = compiler.compile()?;
        let runtime = runtimes
            .get(&EndpointId::try_new("channel-a")?)
            .ok_or("runtime")?;
        assert_eq!(runtime.upstream_id().as_str(), "provider-a");
        assert_eq!(
            runtime.protocol(),
            gateway_core::GatewayProtocol::OpenAiResponses
        );
        assert!(
            runtime
                .observations_at(None, 10)?
                .iter()
                .all(gateway_router::CompatibleEndpointBindingObservation::is_eligible)
        );
        Ok(())
    }

    #[test]
    fn native_adapter_is_not_coerced_into_generic_runtime() -> Result<(), Box<dyn Error>> {
        let store = secret_store()?;
        let mut config = config(&store)?;
        config.endpoints[0].adapter_id = "grok.build.responses".to_owned();
        let policies = EgressPolicyCompiler::compile(&config)?;
        let compiler = CompatibleEgressRuntimeCompiler::new(
            &config,
            &policies,
            pools()?,
            Arc::new(RuntimeHealthRegistry::new()),
            Arc::new(RuntimeQuotaRegistry::new()),
            BTreeMap::from([(UpstreamId::try_new("provider-a")?, registry("provider-a")?)]),
            BTreeMap::new(),
        );
        assert!(compiler.compile()?.is_empty());
        Ok(())
    }

    #[test]
    fn explicit_proxy_pool_override_reserves_only_the_configured_node() -> Result<(), Box<dyn Error>>
    {
        let store = secret_store()?;
        let config = config(&store)?;
        let policies = EgressPolicyCompiler::compile(&config)?;
        let mut settings = BTreeMap::new();
        settings.insert(
            (
                EndpointId::try_new("channel-a")?,
                CredentialId::try_new("account-a")?,
            ),
            CompatibleEndpointBindingRuntimeSettings {
                target: CompatibleEgressTarget::ProxyPool {
                    pool_id: "pool-a".to_owned(),
                },
                failure_scope: CompatibleFailureScope::EgressNode,
                stickiness: CompatibleStickiness::CredentialAndEgress,
                retry_policy: CompatibleRetryPolicy::pre_submit(2)?,
            },
        );
        let compiler = CompatibleEgressRuntimeCompiler::new(
            &config,
            &policies,
            pools()?,
            Arc::new(RuntimeHealthRegistry::new()),
            Arc::new(RuntimeQuotaRegistry::new()),
            BTreeMap::from([(UpstreamId::try_new("provider-a")?, registry("provider-a")?)]),
            settings,
        );
        let runtimes = compiler.compile()?;
        let runtime = runtimes
            .get(&EndpointId::try_new("channel-a")?)
            .ok_or("runtime")?;
        let lease = runtime.try_lease_at(None, 10)?;
        assert_eq!(lease.selected_egress_node_id(), Some("node-a"));
        assert!(matches!(
            lease.transport_profile().proxy(),
            UpstreamProxy::Socks5(_)
        ));
        drop(lease);
        let observations = runtime.observations_at(None, 10)?;
        assert_eq!(
            observations.first().ok_or("observation")?.active_leases(),
            0
        );
        Ok(())
    }

    #[test]
    fn draft_graph_and_foreign_registry_fail_before_publication() -> Result<(), Box<dyn Error>> {
        let store = secret_store()?;
        let mut draft = config(&store)?;
        draft.version.status = ConfigVersionStatus::Draft;
        let policies = EgressPolicyCompiler::compile(&draft)?;
        let draft_compiler = CompatibleEgressRuntimeCompiler::new(
            &draft,
            &policies,
            pools()?,
            Arc::new(RuntimeHealthRegistry::new()),
            Arc::new(RuntimeQuotaRegistry::new()),
            BTreeMap::from([(UpstreamId::try_new("provider-a")?, registry("provider-a")?)]),
            BTreeMap::new(),
        );
        assert!(matches!(
            draft_compiler.compile(),
            Err(super::CompatibleEgressRuntimeCompileError::ConfigVersionNotActive)
        ));

        let active = config(&store)?;
        let policies = EgressPolicyCompiler::compile(&active)?;
        let foreign_compiler = CompatibleEgressRuntimeCompiler::new(
            &active,
            &policies,
            pools()?,
            Arc::new(RuntimeHealthRegistry::new()),
            Arc::new(RuntimeQuotaRegistry::new()),
            BTreeMap::from([(UpstreamId::try_new("provider-a")?, registry("provider-b")?)]),
            BTreeMap::new(),
        );
        assert!(matches!(
            foreign_compiler.compile(),
            Err(super::CompatibleEgressRuntimeCompileError::TransportRegistryOwnerMismatch)
        ));
        Ok(())
    }

    #[test]
    fn stale_credential_pool_revision_fails_before_publication() -> Result<(), Box<dyn Error>> {
        let store = secret_store()?;
        let mut config = config(&store)?;
        config.credentials[0].revision = 2;
        let policies = EgressPolicyCompiler::compile(&config)?;
        let compiler = CompatibleEgressRuntimeCompiler::new(
            &config,
            &policies,
            pools()?,
            Arc::new(RuntimeHealthRegistry::new()),
            Arc::new(RuntimeQuotaRegistry::new()),
            BTreeMap::from([(UpstreamId::try_new("provider-a")?, registry("provider-a")?)]),
            BTreeMap::new(),
        );
        assert!(matches!(
            compiler.compile(),
            Err(super::CompatibleEgressRuntimeCompileError::RuntimePoolDrift)
        ));
        Ok(())
    }

    #[test]
    fn foreign_compiled_policy_snapshot_fails_before_publication() -> Result<(), Box<dyn Error>> {
        let store = secret_store()?;
        let configuration = config(&store)?;
        let mut other = config(&store)?;
        other.egress_policies[0].allowed_hosts_json =
            r#"["relay.example","other.example"]"#.to_owned();
        let foreign_policies = EgressPolicyCompiler::compile(&other)?;
        let compiler = CompatibleEgressRuntimeCompiler::new(
            &configuration,
            &foreign_policies,
            pools()?,
            Arc::new(RuntimeHealthRegistry::new()),
            Arc::new(RuntimeQuotaRegistry::new()),
            BTreeMap::from([(UpstreamId::try_new("provider-a")?, registry("provider-a")?)]),
            BTreeMap::new(),
        );
        assert!(matches!(
            compiler.compile(),
            Err(super::CompatibleEgressRuntimeCompileError::EgressPolicySnapshotMismatch)
        ));
        Ok(())
    }
}
