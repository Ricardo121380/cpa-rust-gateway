//! Actual lease/transport observations; no database or inventory lookup on the execution path.

use super::{
    AttemptFailure, CompatibleEgressSelection, ControlPlaneConfiguration, CredentialId,
    CredentialLease, EndpointAdapter, EndpointAttemptDriver, EndpointConfiguration,
    EndpointRuntime, RuntimeCompositionError, SemanticCapability, SnapshotRouteCandidate,
    UpstreamProxy, composed_endpoint_url, internal_error, stale_runtime_error,
};
use gateway_core::{AttemptExecutionIdentity, EffectiveCapabilities, ExecutionEgress};
use sha2::{Digest, Sha256};
use std::sync::atomic::Ordering;

fn transport_identity(proxy: &UpstreamProxy) -> ExecutionEgress {
    match proxy {
        UpstreamProxy::Direct => ExecutionEgress::Direct,
        UpstreamProxy::Socks5(_) => {
            proxy
                .canonical_url()
                .map_or(ExecutionEgress::Unknown, |identity| {
                    ExecutionEgress::ProcessProxy {
                        transport_fingerprint: format!("{:x}", Sha256::digest(identity.as_bytes())),
                    }
                })
        }
    }
}

pub(super) struct CapturedEgress {
    candidate_id: gateway_core::RouteCandidateId,
    credential_id: CredentialId,
    credential_revision: u64,
    egress: ExecutionEgress,
    channel: String,
}

pub(super) fn effective_capabilities(
    candidate: &SnapshotRouteCandidate,
    owned_reasoning: bool,
) -> EffectiveCapabilities {
    let set = candidate.effective_capabilities();
    let stored = set.supports(SemanticCapability::StoredResponses);
    let compact = set.supports(SemanticCapability::ResponseCompaction);
    let websocket = set.supports(SemanticCapability::ResponsesWebSocket);
    EffectiveCapabilities {
        reasoning: set.supports(SemanticCapability::Reasoning),
        parallel: set.supports(SemanticCapability::ParallelTools),
        stored,
        continuation: stored
            || compact
            || websocket
            || (owned_reasoning && set.supports(SemanticCapability::Reasoning)),
        compact,
        websocket,
    }
}

impl EndpointAttemptDriver {
    pub(super) fn begin_execution_attempt(
        &self,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
    ) -> Result<(), AttemptFailure> {
        *self
            .execution_egress
            .lock()
            .map_err(|_| AttemptFailure::NonRetryable(internal_error()))? = None;
        if self
            .generation_active
            .as_ref()
            .is_some_and(|active| !active.load(Ordering::Acquire))
        {
            return Err(AttemptFailure::NonRetryable(stale_runtime_error()));
        }
        if let Some(observation) = &self.channel_pin_observation {
            if observation
                .expected_credential_revision
                .is_some_and(|revision| {
                    u64::try_from(revision).ok() != Some(credential.credential_revision())
                })
            {
                return Err(AttemptFailure::NonRetryable(stale_runtime_error()));
            }
            observation.mark_attempted();
            self.record_pin_execution(candidate, credential)?;
        }
        Ok(())
    }

    pub(super) fn capture_egress(
        &self,
        runtime: &EndpointRuntime,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
        compatible: Option<&CompatibleEgressSelection>,
    ) -> Result<(), AttemptFailure> {
        let egress = if let Some(selection) = compatible {
            match selection.lease.profile().target() {
                gateway_upstream::CompatibleEgressTarget::Direct => {
                    transport_identity(selection.lease.transport_profile().proxy())
                }
                target => match (
                    &self.execution_configuration,
                    selection.lease.selected_egress_node_id(),
                ) {
                    (Some((version, revision)), Some(node))
                        if selection.lease.snapshot_version() == version =>
                    {
                        ExecutionEgress::ConfiguredProxy {
                            target_id: target
                                .profile_id()
                                .ok_or_else(|| AttemptFailure::NonRetryable(internal_error()))?
                                .to_owned(),
                            node_id: node.to_owned(),
                            config_version_id: version.as_str().to_owned(),
                            config_revision: revision.as_i64(),
                        }
                    }
                    _ => ExecutionEgress::Unknown,
                },
            }
        } else {
            let transport = if matches!(&runtime.adapter, EndpointAdapter::GrokWebResponses) {
                runtime.transports.for_web_mode(self.mode)
            } else {
                runtime.transports.for_mode(self.mode)
            };
            transport_identity(transport.proxy())
        };
        *self
            .execution_egress
            .lock()
            .map_err(|_| AttemptFailure::NonRetryable(internal_error()))? = Some(CapturedEgress {
            candidate_id: candidate.id().clone(),
            credential_id: credential.credential_id().clone(),
            credential_revision: credential.credential_revision(),
            egress,
            channel: runtime.channel.clone(),
        });
        self.record_pin_execution(candidate, credential)?;
        Ok(())
    }

    pub(super) fn actual_execution_identity(
        &self,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
    ) -> Option<AttemptExecutionIdentity> {
        let (version, revision) = self.execution_configuration.as_ref()?;
        let runtime = self.endpoints.get(candidate.endpoint_id())?;
        let (egress, channel) = match self.execution_egress.lock() {
            Ok(observed) => observed
                .as_ref()
                .filter(|observed| {
                    observed.candidate_id == *candidate.id()
                        && observed.credential_id == *credential.credential_id()
                        && observed.credential_revision == credential.credential_revision()
                })
                .map_or(
                    (ExecutionEgress::NotSelected, runtime.channel.clone()),
                    |observed| (observed.egress.clone(), observed.channel.clone()),
                ),
            Err(_) => (ExecutionEgress::Unknown, runtime.channel.clone()),
        };
        Some(AttemptExecutionIdentity {
            channel,
            credential_revision: credential.credential_revision(),
            config_version_id: version.as_str().to_owned(),
            config_revision: revision.as_i64(),
            egress,
            capabilities: effective_capabilities(
                candidate,
                matches!(runtime.adapter, EndpointAdapter::GrokBuildResponses),
            ),
        })
    }

    pub(super) fn record_claude_channel(&self) -> Result<(), AttemptFailure> {
        if let Some(observed) = self
            .execution_egress
            .lock()
            .map_err(|_| AttemptFailure::NonRetryable(internal_error()))?
            .as_mut()
        {
            "claude".clone_into(&mut observed.channel);
        }
        if let Some(observation) = &self.channel_pin_observation
            && let Some(actual) = observation
                .execution
                .lock()
                .map_err(|_| AttemptFailure::NonRetryable(internal_error()))?
                .as_mut()
        {
            "claude".clone_into(&mut actual.identity.channel);
        }
        Ok(())
    }

    pub(super) fn record_pin_execution(
        &self,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
    ) -> Result<(), AttemptFailure> {
        if let Some(observation) = &self.channel_pin_observation {
            let identity = self
                .actual_execution_identity(candidate, credential)
                .ok_or_else(|| AttemptFailure::NonRetryable(internal_error()))?;
            *observation
                .execution
                .lock()
                .map_err(|_| AttemptFailure::NonRetryable(internal_error()))? = Some(
                gateway_http_actix::management_resources::ManagementChannelPinExecution {
                    attempt_id: gateway_core::AttemptId::from_request_sequence(&self.request_id, 1)
                        .as_str()
                        .to_owned(),
                    route_candidate_id: candidate.id().as_str().to_owned(),
                    upstream_model: candidate.upstream_model().to_owned(),
                    identity,
                },
            );
        }
        Ok(())
    }
}

pub(super) fn endpoint_channel(
    configuration: &ControlPlaneConfiguration,
    configured: &EndpointConfiguration,
) -> Result<String, RuntimeCompositionError> {
    let kind = configuration
        .upstreams
        .iter()
        .find(|upstream| upstream.id == configured.upstream_id)
        .ok_or(RuntimeCompositionError::Unavailable)?
        .kind
        .as_str();
    let url = composed_endpoint_url(configured);
    let channel = match configured.adapter_id.as_str() {
        "grok.build.responses" => "grok.build",
        "grok.console.responses" => "grok.console",
        "grok.web.responses" => "grok.web",
        "grok.official.responses" => "grok.official",
        "kiro.messages" => "kiro",
        "openai-compatible.responses"
            if url.starts_with("https://chatgpt.com/backend-api/codex/") =>
        {
            "codex"
        }
        "openai-compatible.responses" | "openai-compatible.chat-completions"
            if url.starts_with("https://api.kimi.com/coding/") =>
        {
            "kimi-coding"
        }
        "openai-compatible.responses" | "openai-compatible.chat-completions"
            if url.starts_with("https://api.moonshot.cn/v1/")
                || url.starts_with("https://api.moonshot.ai/v1/") =>
        {
            "kimi-api"
        }
        "openai-compatible.responses" | "openai-compatible.chat-completions" => match kind {
            "kimi" => "kimi-api",
            "kimi-coding" => "kimi-coding",
            "codex" | "chatgpt" => "codex",
            _ => "openai-compatible",
        },
        "anthropic-compatible.messages" => {
            if kind == "claude" {
                "claude"
            } else {
                "anthropic-compatible"
            }
        }
        _ => return Err(RuntimeCompositionError::Unavailable),
    };
    Ok(channel.to_owned())
}

#[cfg(test)]
mod tests {
    use super::super::{
        Arc, BTreeMap, BTreeSet, CapabilitySet, ConfigRevision, EgressHost, EndpointId, Error,
        GrokBuildCacheIdentityDeriver, Mutex, NonZeroUsize, OnceLock, OpenAiResponsesEndpoint,
        P12AttemptStageStore, P12TransportProfiles, ProtocolFormat, RequestId,
        ResponsesClientTransport, ResponsesResponseMode, RuntimeQuotaRegistry, SnapshotVersion,
        SystemEgressDnsResolver, UpstreamClientPool,
    };
    use super::*;
    use gateway_router::{
        SnapshotCatalogAdmission, SnapshotRouteCandidateInput, SnapshotTransformMode,
    };
    use gateway_upstream::{
        CredentialMaterialReplacement, CredentialSecret, EgressCidr, EgressPolicy,
        EgressPolicyInput, EgressScheme, EndpointCredentialInput, EndpointCredentialPool,
        RedirectPolicy,
    };
    use std::net::{IpAddr, Ipv4Addr};

    type TestResult = Result<(), Box<dyn Error>>;

    fn fixture() -> Result<
        (
            EndpointAttemptDriver,
            SnapshotRouteCandidate,
            EndpointCredentialPool,
        ),
        Box<dyn Error>,
    > {
        let endpoint_id = EndpointId::try_new("execution-endpoint")?;
        let runtime = EndpointRuntime {
            channel: "openai-compatible".to_owned(),
            adapter: EndpointAdapter::OpenAiResponses(OpenAiResponsesEndpoint::try_new(
                "https://127.0.0.1/v1",
                "/responses",
            )?),
            policy: EgressPolicy::try_new(EgressPolicyInput {
                id: gateway_core::EgressPolicyId::try_new("execution-policy")?,
                name: "Isolated identity fixture".to_owned(),
                allowed_schemes: BTreeSet::from([EgressScheme::Https]),
                allowed_hosts: BTreeSet::from([EgressHost::try_new("127.0.0.1")?]),
                allowed_ports: BTreeSet::from([443]),
                allowed_cidrs: BTreeSet::from([EgressCidr::try_new(
                    IpAddr::V4(Ipv4Addr::LOCALHOST),
                    32,
                )?]),
                redirect_policy: RedirectPolicy::Deny,
            })?,
            resolver: Arc::new(SystemEgressDnsResolver),
            transports: Arc::new(P12TransportProfiles::try_new()?),
            web_statsig: OnceLock::new(),
        };
        let candidate = SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
            id: gateway_core::RouteCandidateId::try_new("execution-candidate")?,
            endpoint_id: endpoint_id.clone(),
            upstream_id: gateway_core::UpstreamId::try_new("execution-upstream")?,
            endpoint_api_format: "openai/responses".to_owned(),
            upstream_model: "execution-model".to_owned(),
            transform_mode: SnapshotTransformMode::Canonical,
            priority: 0,
            weight: 1,
            effective_capabilities: CapabilitySet::try_new([
                SemanticCapability::Tools,
                SemanticCapability::Reasoning,
                SemanticCapability::ParallelTools,
            ])?,
            catalog_admission: SnapshotCatalogAdmission::AllowedUnlisted,
            active_binding_count: 1,
        });
        let pool = EndpointCredentialPool::try_new(
            endpoint_id.clone(),
            [EndpointCredentialInput {
                credential_id: CredentialId::try_new("execution-credential")?,
                credential_kind: "bearer".to_owned(),
                credential_revision: 3,
                priority: 0,
                weight: 1,
                concurrency: 2,
                expires_at_ms: None,
                secret: CredentialSecret::try_new(b"synthetic-old".to_vec())?,
            }],
        )?;
        let request = protocol_openai_responses::decode_request(
            r#"{"model":"execution-model","input":"synthetic private body"}"#,
        )?
        .request;
        let driver = EndpointAttemptDriver {
            execution_configuration: Some((
                SnapshotVersion::try_new("execution-before")?,
                ConfigRevision::try_new(7)?,
            )),
            execution_egress: Mutex::new(None),
            runtime_quota: Arc::new(RuntimeQuotaRegistry::new()),
            kiro_profiles: Arc::new(Mutex::new(BTreeMap::new())),
            generation_active: None,
            request_id: RequestId::try_new("execution-request")?,
            client_key_id: None,
            request,
            client_protocol: ProtocolFormat::OpenAiResponses,
            native_payload: None,
            mode: ResponsesResponseMode::NonStreaming,
            client_transport: ResponsesClientTransport::Http,
            requires_stored_response: false,
            endpoints: Arc::new(BTreeMap::from([(endpoint_id, runtime)])),
            compatible_endpoints: Arc::new(BTreeMap::new()),
            client_pool: Arc::new(UpstreamClientPool::new(
                NonZeroUsize::new(1).ok_or("capacity")?,
            )),
            attempt_stages: Arc::new(P12AttemptStageStore::new()),
            allow_egress_refresh: true,
            channel_pin_observation: None,
            native_grok_egress: None,
            grok_build_cache_identity_deriver: Arc::new(GrokBuildCacheIdentityDeriver::new(
                [0xC4; 32],
            )),
            reasoning_binding: None,
        };
        Ok((driver, candidate, pool))
    }

    #[test]
    fn actual_execution_identity_retains_refreshed_lease_and_configuration() -> TestResult {
        let (mut driver, candidate, pool) = fixture()?;
        let old_lease = pool.try_lease().ok_or("old lease")?;
        let runtime = driver
            .endpoints
            .get(candidate.endpoint_id())
            .ok_or("runtime")?;
        driver
            .capture_egress(runtime, &candidate, &old_lease, None)
            .map_err(|_| "capture")?;
        let old = driver
            .actual_execution_identity(&candidate, &old_lease)
            .ok_or("identity")?;
        assert!(pool.replace_credential_if_revision(
            old_lease.credential_id(),
            3,
            CredentialMaterialReplacement {
                credential_revision: 4,
                expires_at_ms: None,
                secret: CredentialSecret::try_new(b"synthetic-new".to_vec())?
            }
        )?);
        let new_lease = pool.try_lease().ok_or("new lease")?;
        assert_eq!(
            driver.actual_execution_identity(&candidate, &old_lease),
            Some(old.clone())
        );
        assert_eq!(old.credential_revision, 3);
        assert_eq!(
            driver
                .actual_execution_identity(&candidate, &new_lease)
                .ok_or("identity")?
                .credential_revision,
            4
        );
        assert_eq!(
            driver
                .actual_execution_identity(&candidate, &new_lease)
                .ok_or("identity")?
                .egress,
            ExecutionEgress::NotSelected
        );
        driver.execution_configuration = Some((
            SnapshotVersion::try_new("execution-after")?,
            ConfigRevision::try_new(8)?,
        ));
        let runtime = driver
            .endpoints
            .get(candidate.endpoint_id())
            .ok_or("runtime")?;
        driver
            .capture_egress(runtime, &candidate, &new_lease, None)
            .map_err(|_| "capture")?;
        let new = driver
            .actual_execution_identity(&candidate, &new_lease)
            .ok_or("identity")?;
        assert_eq!(new.config_version_id, "execution-after");
        assert_eq!(new.config_revision, 8);
        assert_eq!(new.credential_revision, 4);
        assert_eq!(old.config_version_id, "execution-before");
        assert_eq!(old.config_revision, 7);
        assert_eq!(old.egress, ExecutionEgress::Direct);
        let json = serde_json::to_string(&old)?;
        for private in ["synthetic private body", "synthetic-old", "synthetic-new"] {
            assert!(!json.contains(private));
        }
        assert!(!format!("{old:?}").contains("execution-before"));
        Ok(())
    }

    #[test]
    fn actual_execution_identity_proxy_fingerprint_and_owned_reasoning_are_explicit() -> TestResult
    {
        let (_, candidate, _) = fixture()?;
        let first = transport_identity(&UpstreamProxy::try_socks5("socks5://127.0.0.1:19081")?);
        let next = transport_identity(&UpstreamProxy::try_socks5("socks5://127.0.0.1:19082")?);
        assert_ne!(first, next);
        assert!(matches!(first, ExecutionEgress::ProcessProxy { .. }));
        assert!(!serde_json::to_string(&first)?.contains("127.0.0.1"));
        assert!(!effective_capabilities(&candidate, false).continuation);
        assert!(effective_capabilities(&candidate, true).continuation);
        assert!(effective_capabilities(&candidate, false).reasoning);
        assert!(effective_capabilities(&candidate, false).parallel);
        Ok(())
    }
}
