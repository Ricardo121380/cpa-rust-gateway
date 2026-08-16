//! Provider-neutral runtime composition for one generic compatible Endpoint.
//!
//! The composition reuses the existing Endpoint Credential pool and Runtime Health/Quota
//! registries. It never creates a second scheduler, reads Store after construction, resolves DNS,
//! opens a socket, or calls a Provider. A later serving adapter may use the returned lease to
//! construct one admitted request, but remains responsible for `EgressPolicy::admit_url` at dial
//! time.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::Arc,
};

use gateway_core::{CredentialId, EndpointId, GatewayProtocol, UpstreamId};
use gateway_upstream::{
    CompatibleEgressError, CompatibleEgressTarget, CompatibleEndpointEgressProfile,
    CredentialLease, CredentialPoolEntrySnapshot, EgressPolicy, EndpointCredentialPools,
    EndpointUrl,
};

use crate::{
    CompatibleEgressTargetObservation, CompatibleEgressTransportLease,
    CompatibleEgressTransportRegistry, RuntimeCredentialAccountStatus, RuntimeHealthAvailability,
    RuntimeHealthKey, RuntimeHealthRegistry, RuntimeQuotaAvailability, RuntimeQuotaRegistry,
    RuntimeQuotaTarget, SnapshotVersion,
};

/// Maximum exact Credential bindings in one generic Endpoint composition.
pub const MAX_COMPATIBLE_ENDPOINT_BINDINGS: usize = 1024;

/// One profile plus the owner identities supplied by the selected Config Version graph.
#[derive(Clone, Debug)]
pub struct CompatibleEndpointRuntimeBindingInput {
    /// P13-11A exact profile.
    pub profile: CompatibleEndpointEgressProfile,
    /// Upstream owner recorded on the Endpoint row.
    pub endpoint_upstream_id: UpstreamId,
    /// Upstream owner recorded on the Credential row.
    pub credential_upstream_id: UpstreamId,
    /// Persistent Credential revision expected from the selected Config Version.
    pub credential_revision: u64,
    /// Lower-is-better scheduling priority expected from the selected binding.
    pub priority: i64,
    /// Positive scheduling weight expected from the selected binding.
    pub weight: usize,
    /// Positive concurrency limit expected from the selected binding.
    pub maximum_concurrency: usize,
}

/// Complete input for one active Config-Version-bound compatible Endpoint composition.
pub struct CompatibleEndpointRuntimeInput {
    /// Immutable runtime Snapshot identity selected by the caller.
    pub snapshot_version: SnapshotVersion,
    /// Every active profile bound to this exact Endpoint.
    pub bindings: Vec<CompatibleEndpointRuntimeBindingInput>,
    /// Compiled version-scoped URL/SSRF policy.
    pub egress_policy: Arc<EgressPolicy>,
    /// Base URL consumed once during static composition; not retained as a public string.
    pub base_url: String,
    /// Path consumed once during static composition; not retained as a public string.
    pub inference_path: String,
    /// Existing immutable Endpoint Credential pools used by serving.
    pub credential_pools: Arc<EndpointCredentialPools>,
    /// Existing shared runtime Health registry.
    pub runtime_health: Arc<RuntimeHealthRegistry>,
    /// Existing shared runtime Quota registry.
    pub runtime_quota: Arc<RuntimeQuotaRegistry>,
    /// Bounded local transport-node registry.
    pub transport_registry: CompatibleEgressTransportRegistry,
}

/// Safe construction failures for one active Endpoint runtime composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleEndpointRuntimeBuildError {
    /// No profile binding was supplied.
    EmptyBindings,
    /// The profile count exceeded the finite bound.
    TooManyBindings,
    /// Profiles belonged to different Upstreams.
    MixedUpstream,
    /// Profiles belonged to different Endpoints.
    MixedEndpoint,
    /// Profiles used different public wire protocols.
    MixedProtocol,
    /// Profiles used different `EgressPolicy` identities.
    MixedEgressPolicy,
    /// Endpoint owner did not match the profile owner.
    EndpointUpstreamMismatch,
    /// Credential owner did not match the profile owner.
    CredentialUpstreamMismatch,
    /// A profile failed its bounded local validation.
    ProfileRejected,
    /// The base URL/path failed static `EndpointUrl` or `EgressPolicy` validation.
    EndpointTargetRejected,
    /// The existing runtime pool did not contain the selected Endpoint.
    MissingEndpointPool,
    /// The existing runtime pool did not contain one selected Credential.
    MissingCredentialPoolEntry,
    /// The profile's source label drifted from the compiled pool entry.
    CredentialKindMismatch,
    /// More than one profile targeted one Credential identity.
    DuplicateCredentialProfile,
    /// The target was not registered in the bounded transport registry.
    UnknownTransportTarget,
    /// The transport registry belongs to a different Upstream/Provider instance.
    TransportRegistryOwnerMismatch,
    /// The existing pool carries a different persistent Credential revision.
    CredentialRevisionMismatch,
    /// The existing pool carries different binding priority/weight/concurrency values.
    CredentialScheduleMismatch,
}

impl fmt::Display for CompatibleEndpointRuntimeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyBindings => "compatible Endpoint runtime has no bindings",
            Self::TooManyBindings => "compatible Endpoint runtime binding count exceeds its bound",
            Self::MixedUpstream => "compatible Endpoint runtime mixes Upstreams",
            Self::MixedEndpoint => "compatible Endpoint runtime mixes Endpoints",
            Self::MixedProtocol => "compatible Endpoint runtime mixes protocols",
            Self::MixedEgressPolicy => "compatible Endpoint runtime mixes EgressPolicies",
            Self::EndpointUpstreamMismatch => {
                "compatible Endpoint runtime Endpoint owner mismatches profile"
            }
            Self::CredentialUpstreamMismatch => {
                "compatible Endpoint runtime Credential owner mismatches profile"
            }
            Self::ProfileRejected => "compatible Endpoint runtime profile is rejected",
            Self::EndpointTargetRejected => "compatible Endpoint runtime target is rejected",
            Self::MissingEndpointPool => "compatible Endpoint runtime pool is missing",
            Self::MissingCredentialPoolEntry => {
                "compatible Endpoint runtime Credential pool entry is missing"
            }
            Self::CredentialKindMismatch => "compatible Endpoint runtime Credential kind drifted",
            Self::DuplicateCredentialProfile => {
                "compatible Endpoint runtime has a duplicate Credential profile"
            }
            Self::UnknownTransportTarget => "compatible Endpoint runtime target is unregistered",
            Self::TransportRegistryOwnerMismatch => {
                "compatible Endpoint runtime transport registry belongs to another upstream"
            }
            Self::CredentialRevisionMismatch => {
                "compatible Endpoint runtime Credential revision drifted"
            }
            Self::CredentialScheduleMismatch => {
                "compatible Endpoint runtime Credential schedule drifted"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for CompatibleEndpointRuntimeBuildError {}

/// Safe request-time failures from the composed runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleEndpointRuntimeError {
    /// A shared Health shard could not be read.
    HealthUnavailable,
    /// A shared Quota shard could not be read or model target was invalid.
    QuotaUnavailable,
    /// The Endpoint-wide Health state blocks ordinary scheduling.
    EndpointBlocked,
    /// No Credential passed all exact Health/Quota/expiry/capacity predicates.
    NoEligibleCredential,
    /// The selected target lost egress capacity/state between observation and acquisition.
    EgressUnavailable,
    /// The transport registry returned an unexpected local state error.
    EgressRegistryUnavailable,
}

impl fmt::Display for CompatibleEndpointRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::HealthUnavailable => "compatible Endpoint runtime Health is unavailable",
            Self::QuotaUnavailable => "compatible Endpoint runtime Quota is unavailable",
            Self::EndpointBlocked => "compatible Endpoint runtime Endpoint is blocked",
            Self::NoEligibleCredential => "no compatible Credential is eligible",
            Self::EgressUnavailable => "compatible egress target is unavailable",
            Self::EgressRegistryUnavailable => "compatible egress registry is unavailable",
        };
        formatter.write_str(message)
    }
}

impl Error for CompatibleEndpointRuntimeError {}

/// One secret-free observation of an exact compatible Credential binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibleEndpointBindingObservation {
    snapshot_version: SnapshotVersion,
    upstream_id: UpstreamId,
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    credential_kind: String,
    protocol: GatewayProtocol,
    target: CompatibleEgressTarget,
    observed_at_ms: i64,
    endpoint_health: RuntimeHealthAvailability,
    credential_health: RuntimeHealthAvailability,
    account_status: RuntimeCredentialAccountStatus,
    model_health: Option<RuntimeHealthAvailability>,
    binding_quota: RuntimeQuotaAvailability,
    model_quota: Option<RuntimeQuotaAvailability>,
    expires_at_ms: Option<i64>,
    active_leases: usize,
    maximum_concurrency: usize,
    egress: CompatibleEgressTargetObservation,
    eligible: bool,
}

impl CompatibleEndpointBindingObservation {
    /// Returns the immutable Config Version identity used by the observation.
    #[must_use]
    pub fn snapshot_version(&self) -> &SnapshotVersion {
        &self.snapshot_version
    }

    /// Returns the owning Upstream identity.
    #[must_use]
    pub fn upstream_id(&self) -> &UpstreamId {
        &self.upstream_id
    }

    /// Returns the exact Endpoint identity.
    #[must_use]
    pub fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the exact Credential identity.
    #[must_use]
    pub fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the non-secret source label retained by the compiled pool.
    #[must_use]
    pub fn credential_kind(&self) -> &str {
        &self.credential_kind
    }

    /// Returns the public wire protocol.
    #[must_use]
    pub const fn protocol(&self) -> GatewayProtocol {
        self.protocol
    }

    /// Returns the configured direct/fixed/pool target.
    #[must_use]
    pub fn target(&self) -> &CompatibleEgressTarget {
        &self.target
    }

    /// Returns the fixed observation timestamp shared by all registry reads.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Returns Endpoint-wide Health.
    #[must_use]
    pub const fn endpoint_health(&self) -> RuntimeHealthAvailability {
        self.endpoint_health
    }

    /// Returns exact Credential Health.
    #[must_use]
    pub const fn credential_health(&self) -> RuntimeHealthAvailability {
        self.credential_health
    }

    /// Returns exact provider-account status.
    #[must_use]
    pub const fn account_status(&self) -> RuntimeCredentialAccountStatus {
        self.account_status
    }

    /// Returns model-scoped Health, if the caller supplied a model.
    #[must_use]
    pub const fn model_health(&self) -> Option<RuntimeHealthAvailability> {
        self.model_health
    }

    /// Returns binding Quota availability.
    #[must_use]
    pub const fn binding_quota(&self) -> RuntimeQuotaAvailability {
        self.binding_quota
    }

    /// Returns model-scoped Quota availability, if the caller supplied a model.
    #[must_use]
    pub const fn model_quota(&self) -> Option<RuntimeQuotaAvailability> {
        self.model_quota
    }

    /// Returns the compiled non-secret Credential expiry.
    #[must_use]
    pub const fn expires_at_ms(&self) -> Option<i64> {
        self.expires_at_ms
    }

    /// Returns current active Credential leases.
    #[must_use]
    pub const fn active_leases(&self) -> usize {
        self.active_leases
    }

    /// Returns the Credential concurrency limit.
    #[must_use]
    pub const fn maximum_concurrency(&self) -> usize {
        self.maximum_concurrency
    }

    /// Returns the separate egress-node observation.
    #[must_use]
    pub const fn egress(&self) -> &CompatibleEgressTargetObservation {
        &self.egress
    }

    /// Returns whether every exact local predicate passed at the observation instant.
    #[must_use]
    pub const fn is_eligible(&self) -> bool {
        self.eligible
    }
}

/// One composed runtime for one exact generic Endpoint.
pub struct CompatibleEndpointRuntime {
    snapshot_version: SnapshotVersion,
    upstream_id: UpstreamId,
    endpoint_id: EndpointId,
    protocol: GatewayProtocol,
    egress_policy: Arc<EgressPolicy>,
    endpoint_url: Arc<EndpointUrl>,
    profiles: BTreeMap<CredentialId, CompatibleEndpointEgressProfile>,
    credential_pools: Arc<EndpointCredentialPools>,
    runtime_health: Arc<RuntimeHealthRegistry>,
    runtime_quota: Arc<RuntimeQuotaRegistry>,
    transport_registry: CompatibleEgressTransportRegistry,
}

impl CompatibleEndpointRuntime {
    /// Constructs one active Config-Version-bound generic Endpoint composition.
    ///
    /// All profile bindings are validated before the runtime becomes visible. The existing pool
    /// is only inspected for secret-free metadata; no Credential bytes are decrypted here.
    ///
    /// # Errors
    ///
    /// Returns a closed [`CompatibleEndpointRuntimeBuildError`] for mixed ownership, missing
    /// pool entries, target drift, or static URL/policy rejection.
    pub fn try_new(
        input: CompatibleEndpointRuntimeInput,
    ) -> Result<Self, CompatibleEndpointRuntimeBuildError> {
        if input.bindings.is_empty() {
            return Err(CompatibleEndpointRuntimeBuildError::EmptyBindings);
        }
        if input.bindings.len() > MAX_COMPATIBLE_ENDPOINT_BINDINGS {
            return Err(CompatibleEndpointRuntimeBuildError::TooManyBindings);
        }

        let first = input
            .bindings
            .first()
            .ok_or(CompatibleEndpointRuntimeBuildError::EmptyBindings)?;
        let upstream_id = first.profile.upstream_id().clone();
        let endpoint_id = first.profile.endpoint_id().clone();
        let protocol = first.profile.protocol();
        let egress_policy_id = first.profile.egress_policy_id().clone();
        if input.credential_pools.pool(&endpoint_id).is_none() {
            return Err(CompatibleEndpointRuntimeBuildError::MissingEndpointPool);
        }
        if input.egress_policy.id() != &egress_policy_id {
            return Err(CompatibleEndpointRuntimeBuildError::MixedEgressPolicy);
        }
        if input.transport_registry.owner_upstream_id() != &upstream_id {
            return Err(CompatibleEndpointRuntimeBuildError::TransportRegistryOwnerMismatch);
        }

        let endpoint_url = first
            .profile
            .validate_endpoint_target(&input.egress_policy, &input.base_url, &input.inference_path)
            .map_err(|_| CompatibleEndpointRuntimeBuildError::EndpointTargetRejected)?;
        let endpoint_url = Arc::new(endpoint_url);
        let pool_entries = input
            .credential_pools
            .pool(&endpoint_id)
            .ok_or(CompatibleEndpointRuntimeBuildError::MissingEndpointPool)?
            .diagnostic_entries();
        let profiles = validate_profiles(
            &input,
            &upstream_id,
            &endpoint_id,
            protocol,
            &egress_policy_id,
            &pool_entries,
        )?;

        Ok(Self {
            snapshot_version: input.snapshot_version,
            upstream_id,
            endpoint_id,
            protocol,
            egress_policy: input.egress_policy,
            endpoint_url,
            profiles,
            credential_pools: input.credential_pools,
            runtime_health: input.runtime_health,
            runtime_quota: input.runtime_quota,
            transport_registry: input.transport_registry,
        })
    }

    /// Returns the exact immutable Config Version identity.
    #[must_use]
    pub fn snapshot_version(&self) -> &SnapshotVersion {
        &self.snapshot_version
    }

    /// Returns the owning Upstream identity.
    #[must_use]
    pub fn upstream_id(&self) -> &UpstreamId {
        &self.upstream_id
    }

    /// Returns the exact Endpoint identity.
    #[must_use]
    pub fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the public protocol bound to this Endpoint.
    #[must_use]
    pub const fn protocol(&self) -> GatewayProtocol {
        self.protocol
    }

    /// Returns the internal URL object; its Debug representation is redacted.
    #[must_use]
    pub fn endpoint_url(&self) -> &EndpointUrl {
        self.endpoint_url.as_ref()
    }

    /// Returns the compiled URL/SSRF policy for the later per-attempt admission call.
    #[must_use]
    pub fn egress_policy(&self) -> &EgressPolicy {
        self.egress_policy.as_ref()
    }

    /// Produces one deterministic, secret-free observation per Credential at one timestamp.
    ///
    /// `upstream_model` is used only as an exact Health/Quota lookup key and is never retained in
    /// the returned projection.
    ///
    /// # Errors
    ///
    /// Returns a target-free fail-closed error when either shared registry or the local transport
    /// registry cannot be read.
    pub fn observations_at(
        &self,
        upstream_model: Option<&str>,
        observed_at_ms: i64,
    ) -> Result<Vec<CompatibleEndpointBindingObservation>, CompatibleEndpointRuntimeError> {
        let endpoint_health = self
            .runtime_health
            .availability_at(
                &RuntimeHealthKey::endpoint(self.endpoint_id.clone()),
                observed_at_ms,
            )
            .map_err(|_| CompatibleEndpointRuntimeError::HealthUnavailable)?;
        let pool = self
            .credential_pools
            .pool(&self.endpoint_id)
            .ok_or(CompatibleEndpointRuntimeError::NoEligibleCredential)?;
        let entries = pool.diagnostic_entries();
        let mut observations = Vec::with_capacity(self.profiles.len());
        for profile in self.profiles.values() {
            let entry = entries
                .iter()
                .find(|entry| entry.credential_id() == profile.credential_id())
                .ok_or(CompatibleEndpointRuntimeError::NoEligibleCredential)?;
            observations.push(self.observation_for_profile(
                profile,
                entry,
                endpoint_health,
                upstream_model,
                observed_at_ms,
            )?);
        }
        Ok(observations)
    }

    /// Acquires one exact Credential plus one egress-node lease after all local predicates pass.
    ///
    /// Credential scheduling remains delegated to the existing Endpoint pool, so this operation
    /// does not introduce a second cursor or a cross-Upstream fallback.
    ///
    /// # Errors
    ///
    /// Returns a closed [`CompatibleEndpointRuntimeError`] and never returns a partial lease.
    pub fn try_lease_at(
        &self,
        upstream_model: Option<&str>,
        observed_at_ms: i64,
    ) -> Result<CompatibleEndpointRuntimeLease, CompatibleEndpointRuntimeError> {
        let observations = self.observations_at(upstream_model, observed_at_ms)?;
        let endpoint_health = observations
            .first()
            .map_or(RuntimeHealthAvailability::Available, |observation| {
                observation.endpoint_health
            });
        if !endpoint_health.is_available() {
            return Err(CompatibleEndpointRuntimeError::EndpointBlocked);
        }
        let eligible: BTreeSet<_> = observations
            .iter()
            .filter(|observation| observation.is_eligible())
            .map(|observation| observation.credential_id().clone())
            .collect();
        if eligible.is_empty() {
            return Err(CompatibleEndpointRuntimeError::NoEligibleCredential);
        }
        let pool = self
            .credential_pools
            .pool(&self.endpoint_id)
            .ok_or(CompatibleEndpointRuntimeError::NoEligibleCredential)?;
        let entries = pool.diagnostic_entries();
        let credential_lease = self
            .credential_pools
            .try_lease_eligible_at(&self.endpoint_id, observed_at_ms, |credential_id| {
                if !eligible.contains(credential_id) {
                    return false;
                }
                let Some(profile) = self.profiles.get(credential_id) else {
                    return false;
                };
                let Some(entry) = entries
                    .iter()
                    .find(|entry| entry.credential_id() == credential_id)
                else {
                    return false;
                };
                let Ok(endpoint_health) = self.runtime_health.availability_at(
                    &RuntimeHealthKey::endpoint(self.endpoint_id.clone()),
                    observed_at_ms,
                ) else {
                    return false;
                };
                self.observation_for_profile(
                    profile,
                    entry,
                    endpoint_health,
                    upstream_model,
                    observed_at_ms,
                )
                .is_ok_and(|observation| observation.is_eligible())
            })
            .ok_or(CompatibleEndpointRuntimeError::NoEligibleCredential)?;
        let profile = self
            .profiles
            .get(credential_lease.credential_id())
            .cloned()
            .ok_or(CompatibleEndpointRuntimeError::NoEligibleCredential)?;
        let Ok(egress_lease) = self
            .transport_registry
            .try_acquire(profile.target(), observed_at_ms)
        else {
            return Err(CompatibleEndpointRuntimeError::EgressUnavailable);
        };
        Ok(CompatibleEndpointRuntimeLease {
            snapshot_version: self.snapshot_version.clone(),
            profile,
            endpoint_url: Arc::clone(&self.endpoint_url),
            egress_policy: Arc::clone(&self.egress_policy),
            credential_lease,
            egress_lease,
        })
    }

    fn observation_for_profile(
        &self,
        profile: &CompatibleEndpointEgressProfile,
        entry: &CredentialPoolEntrySnapshot,
        endpoint_health: RuntimeHealthAvailability,
        upstream_model: Option<&str>,
        observed_at_ms: i64,
    ) -> Result<CompatibleEndpointBindingObservation, CompatibleEndpointRuntimeError> {
        let credential_health = self
            .runtime_health
            .availability_at(
                &RuntimeHealthKey::endpoint_credential(
                    self.endpoint_id.clone(),
                    profile.credential_id().clone(),
                ),
                observed_at_ms,
            )
            .map_err(|_| CompatibleEndpointRuntimeError::HealthUnavailable)?;
        let account_status = self
            .runtime_health
            .credential_account_status_at(
                &self.endpoint_id,
                profile.credential_id(),
                observed_at_ms,
            )
            .map_err(|_| CompatibleEndpointRuntimeError::HealthUnavailable)?;
        let binding_quota_target = RuntimeQuotaTarget::endpoint_credential(
            self.endpoint_id.clone(),
            profile.credential_id().clone(),
        );
        let binding_quota = self
            .runtime_quota
            .status_at(&binding_quota_target, observed_at_ms)
            .map_err(|_| CompatibleEndpointRuntimeError::QuotaUnavailable)?
            .availability();
        let (model_health, model_quota) = if let Some(upstream_model) = upstream_model {
            let model_health = self
                .runtime_health
                .availability_at(
                    &RuntimeHealthKey::endpoint_credential_model(
                        self.endpoint_id.clone(),
                        profile.credential_id().clone(),
                        upstream_model,
                    ),
                    observed_at_ms,
                )
                .map_err(|_| CompatibleEndpointRuntimeError::HealthUnavailable)?;
            let model_quota_target = RuntimeQuotaTarget::endpoint_credential_model(
                self.endpoint_id.clone(),
                profile.credential_id().clone(),
                upstream_model,
            )
            .map_err(|_| CompatibleEndpointRuntimeError::QuotaUnavailable)?;
            let model_quota = self
                .runtime_quota
                .status_at(&model_quota_target, observed_at_ms)
                .map_err(|_| CompatibleEndpointRuntimeError::QuotaUnavailable)?
                .availability();
            (Some(model_health), Some(model_quota))
        } else {
            (None, None)
        };
        let egress = self
            .transport_registry
            .observe(profile.target(), observed_at_ms)
            .map_err(|_| CompatibleEndpointRuntimeError::EgressRegistryUnavailable)?;
        let expired = entry
            .expires_at_ms()
            .is_some_and(|expires_at_ms| expires_at_ms <= observed_at_ms);
        let eligible = endpoint_health.is_available()
            && credential_health.is_available()
            && matches!(account_status, RuntimeCredentialAccountStatus::Available)
            && matches!(binding_quota, RuntimeQuotaAvailability::Available)
            && model_health.is_none_or(RuntimeHealthAvailability::is_available)
            && model_quota.is_none_or(|availability| {
                matches!(availability, RuntimeQuotaAvailability::Available)
            })
            && !expired
            && !entry.is_saturated()
            && egress.availability().is_available();
        Ok(CompatibleEndpointBindingObservation {
            snapshot_version: self.snapshot_version.clone(),
            upstream_id: self.upstream_id.clone(),
            endpoint_id: self.endpoint_id.clone(),
            credential_id: profile.credential_id().clone(),
            credential_kind: entry.credential_kind().to_owned(),
            protocol: self.protocol,
            target: profile.target().clone(),
            observed_at_ms,
            endpoint_health,
            credential_health,
            account_status,
            model_health,
            binding_quota,
            model_quota,
            expires_at_ms: entry.expires_at_ms(),
            active_leases: entry.active_leases(),
            maximum_concurrency: entry.maximum_concurrency(),
            egress,
            eligible,
        })
    }
}

fn validate_profiles(
    input: &CompatibleEndpointRuntimeInput,
    upstream_id: &UpstreamId,
    endpoint_id: &EndpointId,
    protocol: GatewayProtocol,
    egress_policy_id: &gateway_core::EgressPolicyId,
    pool_entries: &[CredentialPoolEntrySnapshot],
) -> Result<
    BTreeMap<CredentialId, CompatibleEndpointEgressProfile>,
    CompatibleEndpointRuntimeBuildError,
> {
    let mut profiles = BTreeMap::new();
    for binding in &input.bindings {
        let profile = &binding.profile;
        if profile.upstream_id() != upstream_id {
            return Err(CompatibleEndpointRuntimeBuildError::MixedUpstream);
        }
        if profile.endpoint_id() != endpoint_id {
            return Err(CompatibleEndpointRuntimeBuildError::MixedEndpoint);
        }
        if profile.protocol() != protocol {
            return Err(CompatibleEndpointRuntimeBuildError::MixedProtocol);
        }
        if profile.egress_policy_id() != egress_policy_id {
            return Err(CompatibleEndpointRuntimeBuildError::MixedEgressPolicy);
        }
        match profile.validate_binding(
            &binding.endpoint_upstream_id,
            &binding.credential_upstream_id,
        ) {
            Ok(()) => {}
            Err(CompatibleEgressError::EndpointUpstreamMismatch) => {
                return Err(CompatibleEndpointRuntimeBuildError::EndpointUpstreamMismatch);
            }
            Err(CompatibleEgressError::CredentialUpstreamMismatch) => {
                return Err(CompatibleEndpointRuntimeBuildError::CredentialUpstreamMismatch);
            }
            Err(_) => return Err(CompatibleEndpointRuntimeBuildError::ProfileRejected),
        }
        if !input.transport_registry.contains_target(profile.target()) {
            return Err(CompatibleEndpointRuntimeBuildError::UnknownTransportTarget);
        }
        if profile
            .validate_endpoint_target(&input.egress_policy, &input.base_url, &input.inference_path)
            .is_err()
        {
            return Err(CompatibleEndpointRuntimeBuildError::EndpointTargetRejected);
        }
        let entry = pool_entries
            .iter()
            .find(|entry| entry.credential_id() == profile.credential_id())
            .ok_or(CompatibleEndpointRuntimeBuildError::MissingCredentialPoolEntry)?;
        if entry.credential_kind() != profile.credential_kind() {
            return Err(CompatibleEndpointRuntimeBuildError::CredentialKindMismatch);
        }
        if entry.credential_revision() != binding.credential_revision {
            return Err(CompatibleEndpointRuntimeBuildError::CredentialRevisionMismatch);
        }
        if entry.priority() != binding.priority
            || entry.weight() != binding.weight
            || entry.maximum_concurrency() != binding.maximum_concurrency
        {
            return Err(CompatibleEndpointRuntimeBuildError::CredentialScheduleMismatch);
        }
        if profiles
            .insert(profile.credential_id().clone(), profile.clone())
            .is_some()
        {
            return Err(CompatibleEndpointRuntimeBuildError::DuplicateCredentialProfile);
        }
    }
    Ok(profiles)
}

impl fmt::Debug for CompatibleEndpointRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompatibleEndpointRuntime")
            .field("snapshot_version", &self.snapshot_version)
            .field("upstream_id", &self.upstream_id)
            .field("endpoint_id", &self.endpoint_id)
            .field("protocol", &self.protocol)
            .field("egress_policy", &self.egress_policy)
            .field("endpoint_url", &self.endpoint_url)
            .field("profile_count", &self.profiles.len())
            .field("credential_pools", &"<shared>")
            .field("runtime_health", &"<shared>")
            .field("runtime_quota", &"<shared>")
            .field("transport_registry", &self.transport_registry)
            .finish()
    }
}

/// One request-scoped lease over exact Credential and egress-node capacity.
pub struct CompatibleEndpointRuntimeLease {
    snapshot_version: SnapshotVersion,
    profile: CompatibleEndpointEgressProfile,
    endpoint_url: Arc<EndpointUrl>,
    egress_policy: Arc<EgressPolicy>,
    credential_lease: CredentialLease,
    egress_lease: CompatibleEgressTransportLease,
}

impl CompatibleEndpointRuntimeLease {
    /// Returns the immutable Config Version identity.
    #[must_use]
    pub fn snapshot_version(&self) -> &SnapshotVersion {
        &self.snapshot_version
    }

    /// Returns the exact selected profile.
    #[must_use]
    pub fn profile(&self) -> &CompatibleEndpointEgressProfile {
        &self.profile
    }

    /// Returns the selected Credential identity.
    #[must_use]
    pub fn credential_id(&self) -> &CredentialId {
        self.credential_lease.credential_id()
    }

    /// Returns the selected Credential kind.
    #[must_use]
    pub fn credential_kind(&self) -> &str {
        self.credential_lease.credential_kind()
    }

    /// Returns the selected Credential revision.
    #[must_use]
    pub fn credential_revision(&self) -> u64 {
        self.credential_lease.credential_revision()
    }

    /// Returns secret bytes only to the immediate authorized adapter while the lease is alive.
    #[must_use]
    pub fn secret_bytes(&self) -> &[u8] {
        self.credential_lease.secret_bytes()
    }

    /// Returns the redacted internal Endpoint URL for the later transport owner.
    #[must_use]
    pub fn endpoint_url(&self) -> &EndpointUrl {
        self.endpoint_url.as_ref()
    }

    /// Returns the compiled `EgressPolicy` for the later per-attempt admission call.
    #[must_use]
    pub fn egress_policy(&self) -> &EgressPolicy {
        self.egress_policy.as_ref()
    }

    /// Returns the selected transport profile.
    #[must_use]
    pub fn transport_profile(&self) -> &gateway_upstream::UpstreamTransportProfile {
        self.egress_lease.transport_profile()
    }

    /// Returns the selected fixed/pool node identity, if any.
    #[must_use]
    pub fn selected_egress_node_id(&self) -> Option<&str> {
        self.egress_lease.selected_node_id()
    }

    /// Explicitly releases both Credential and egress capacity.
    pub fn release(self) {
        drop(self);
    }
}

impl fmt::Debug for CompatibleEndpointRuntimeLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompatibleEndpointRuntimeLease")
            .field("snapshot_version", &self.snapshot_version)
            .field("profile", &self.profile)
            .field("endpoint_url", &self.endpoint_url)
            .field("egress_policy", &self.egress_policy)
            .field("credential_lease", &self.credential_lease)
            .field("egress_lease", &self.egress_lease)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, error::Error, num::NonZeroUsize, sync::Arc, time::Duration};

    use gateway_core::{CredentialId, EgressPolicyId, EndpointId, GatewayProtocol, UpstreamId};
    use gateway_upstream::{
        CompatibleEgressTarget, CompatibleEndpointEgressInput, CompatibleFailureScope,
        CompatibleRetryPolicy, CompatibleStickiness, CredentialSecret, EgressHost, EgressPolicy,
        EgressPolicyInput, EgressScheme, EndpointCredentialInput, EndpointCredentialPool,
        EndpointCredentialPools, RedirectPolicy, UpstreamProxy, UpstreamTimeouts,
        UpstreamTransportProfile,
    };

    use super::{
        CompatibleEndpointBindingObservation, CompatibleEndpointRuntime,
        CompatibleEndpointRuntimeBindingInput, CompatibleEndpointRuntimeBuildError,
        CompatibleEndpointRuntimeError, CompatibleEndpointRuntimeInput,
    };
    use crate::{
        CompatibleEgressNodeInput, CompatibleEgressTransportRegistry,
        CompatibleEgressTransportRegistryInput, CompatibleProxyPoolInput, RuntimeHealthRegistry,
        RuntimeQuotaAvailability, RuntimeQuotaRegistry, RuntimeQuotaTarget, SnapshotVersion,
    };

    fn timeouts() -> Result<UpstreamTimeouts, Box<dyn Error>> {
        Ok(UpstreamTimeouts::try_new(
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(3),
            Duration::from_secs(5),
        )?)
    }

    fn direct_profile() -> Result<UpstreamTransportProfile, Box<dyn Error>> {
        Ok(UpstreamTransportProfile::new(
            timeouts()?,
            UpstreamProxy::Direct,
            NonZeroUsize::new(1).ok_or("nonzero")?,
        ))
    }

    fn socks_profile(port: u16) -> Result<UpstreamTransportProfile, Box<dyn Error>> {
        Ok(UpstreamTransportProfile::new(
            timeouts()?,
            UpstreamProxy::try_socks5(&format!("socks5://127.0.0.1:{port}"))?,
            NonZeroUsize::new(1).ok_or("nonzero")?,
        ))
    }

    fn policy() -> Result<Arc<EgressPolicy>, Box<dyn Error>> {
        Ok(Arc::new(EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("policy")?,
            name: "policy".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Https]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("relay.example")?]),
            allowed_ports: BTreeSet::from([443]),
            allowed_cidrs: BTreeSet::new(),
            redirect_policy: RedirectPolicy::Deny,
        })?))
    }

    fn pools(
        endpoint: &EndpointId,
        entries: Vec<(&str, i64, Option<i64>)>,
    ) -> Result<Arc<EndpointCredentialPools>, Box<dyn Error>> {
        let inputs = entries
            .into_iter()
            .map(|(id, concurrency, expires_at_ms)| {
                Ok(EndpointCredentialInput {
                    credential_id: CredentialId::try_new(id)?,
                    credential_kind: "api_key".to_owned(),
                    credential_revision: 1,
                    priority: 0,
                    weight: 1,
                    concurrency,
                    expires_at_ms,
                    secret: CredentialSecret::try_new(format!("secret-{id}"))?,
                })
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(Arc::new(EndpointCredentialPools::try_new([
            EndpointCredentialPool::try_new(endpoint.clone(), inputs)?,
        ])?))
    }

    fn registry() -> Result<CompatibleEgressTransportRegistry, Box<dyn Error>> {
        Ok(CompatibleEgressTransportRegistry::try_new(
            CompatibleEgressTransportRegistryInput {
                owner_upstream_id: UpstreamId::try_new("upstream")?,
                direct_profile: direct_profile()?,
                fixed_proxies: Vec::new(),
                proxy_pools: vec![CompatibleProxyPoolInput {
                    pool_id: "pool".to_owned(),
                    nodes: vec![CompatibleEgressNodeInput {
                        node_id: "node-a".to_owned(),
                        transport_profile: socks_profile(19081)?,
                        weight: 1,
                        maximum_concurrency: 1,
                    }],
                }],
            },
        )?)
    }

    fn input(
        endpoint: &EndpointId,
        profile_targets: Vec<(&str, CompatibleEgressTarget)>,
        expires_at_ms: Option<i64>,
    ) -> Result<CompatibleEndpointRuntimeInput, Box<dyn Error>> {
        let upstream = UpstreamId::try_new("upstream")?;
        let policy_id = EgressPolicyId::try_new("policy")?;
        let policy = policy()?;
        let credential_pools = pools(
            endpoint,
            profile_targets
                .iter()
                .map(|(id, _)| (*id, 1, expires_at_ms))
                .collect(),
        )?;
        let bindings = profile_targets
            .into_iter()
            .map(|(credential, target)| {
                Ok(CompatibleEndpointRuntimeBindingInput {
                    profile: gateway_upstream::CompatibleEndpointEgressProfile::try_new(
                        CompatibleEndpointEgressInput {
                            upstream_id: upstream.clone(),
                            endpoint_id: endpoint.clone(),
                            credential_id: CredentialId::try_new(credential)?,
                            credential_kind: "api_key".to_owned(),
                            protocol: GatewayProtocol::OpenAiResponses,
                            egress_policy_id: policy_id.clone(),
                            target,
                            failure_scope: CompatibleFailureScope::Credential,
                            stickiness: CompatibleStickiness::Credential,
                            retry_policy: CompatibleRetryPolicy::pre_submit(2)?,
                        },
                    )?,
                    endpoint_upstream_id: upstream.clone(),
                    credential_upstream_id: upstream.clone(),
                    credential_revision: 1,
                    priority: 0,
                    weight: 1,
                    maximum_concurrency: 1,
                })
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(CompatibleEndpointRuntimeInput {
            snapshot_version: SnapshotVersion::try_new("version-1")?,
            bindings,
            egress_policy: policy,
            base_url: "https://relay.example/v1".to_owned(),
            inference_path: "/responses".to_owned(),
            credential_pools,
            runtime_health: Arc::new(RuntimeHealthRegistry::new()),
            runtime_quota: Arc::new(RuntimeQuotaRegistry::new()),
            transport_registry: registry()?,
        })
    }

    #[test]
    fn runtime_selects_same_upstream_credential_and_releases_both_leases()
    -> Result<(), Box<dyn Error>> {
        let endpoint = EndpointId::try_new("endpoint")?;
        let runtime = CompatibleEndpointRuntime::try_new(input(
            &endpoint,
            vec![
                ("credential-a", CompatibleEgressTarget::Direct),
                (
                    "credential-b",
                    CompatibleEgressTarget::ProxyPool {
                        pool_id: "pool".to_owned(),
                    },
                ),
            ],
            None,
        )?)?;
        let observations = runtime.observations_at(Some("model"), 100)?;
        assert_eq!(observations.len(), 2);
        assert!(
            observations
                .iter()
                .all(CompatibleEndpointBindingObservation::is_eligible)
        );
        let lease = runtime.try_lease_at(Some("model"), 100)?;
        assert_eq!(lease.snapshot_version().as_str(), "version-1");
        assert!(lease.secret_bytes().starts_with(b"secret-"));
        let selected_id = lease.credential_id().clone();
        let selected_node = lease.selected_egress_node_id().map(str::to_owned);
        assert!(
            selected_id == CredentialId::try_new("credential-a")?
                || selected_id == CredentialId::try_new("credential-b")?
        );
        drop(lease);
        assert!(
            runtime
                .observations_at(Some("model"), 100)?
                .iter()
                .all(|observation| observation.active_leases() == 0)
        );
        assert!(selected_node.is_none() || selected_node.as_deref() == Some("node-a"));
        Ok(())
    }

    #[test]
    fn health_quota_expiry_and_egress_are_independent_fail_closed_states()
    -> Result<(), Box<dyn Error>> {
        let endpoint = EndpointId::try_new("endpoint")?;
        let input = input(
            &endpoint,
            vec![(
                "credential-a",
                CompatibleEgressTarget::ProxyPool {
                    pool_id: "pool".to_owned(),
                },
            )],
            Some(100),
        )?;
        let runtime_health = Arc::clone(&input.runtime_health);
        let runtime = CompatibleEndpointRuntime::try_new(input)?;
        let observation = runtime
            .observations_at(None, 100)?
            .pop()
            .ok_or("observation")?;
        assert!(!observation.is_eligible());
        assert_eq!(observation.expires_at_ms(), Some(100));
        runtime_health.mark_credential_unauthorized(
            endpoint.clone(),
            CredentialId::try_new("credential-a")?,
        )?;
        let observation = runtime
            .observations_at(None, 99)?
            .pop()
            .ok_or("observation")?;
        assert_eq!(
            observation.account_status(),
            crate::RuntimeCredentialAccountStatus::Unauthorized
        );
        assert!(observation.egress().availability().is_available());
        Ok(())
    }

    #[test]
    fn exact_binding_quota_blocks_without_mutating_egress_state() -> Result<(), Box<dyn Error>> {
        let endpoint = EndpointId::try_new("endpoint")?;
        let input = input(
            &endpoint,
            vec![("credential-a", CompatibleEgressTarget::Direct)],
            None,
        )?;
        let runtime_quota = Arc::clone(&input.runtime_quota);
        let runtime = CompatibleEndpointRuntime::try_new(input)?;
        runtime_quota.record_rate_limited(
            RuntimeQuotaTarget::endpoint_credential(
                endpoint,
                CredentialId::try_new("credential-a")?,
            ),
            100,
            Some(Duration::from_secs(30)),
            Duration::from_secs(1),
        )?;
        let observation = runtime
            .observations_at(None, 100)?
            .pop()
            .ok_or("observation")?;
        assert!(matches!(
            observation.binding_quota(),
            RuntimeQuotaAvailability::Exhausted { .. }
        ));
        assert!(observation.egress().availability().is_available());
        assert!(matches!(
            runtime.try_lease_at(None, 100),
            Err(CompatibleEndpointRuntimeError::NoEligibleCredential)
        ));
        Ok(())
    }

    #[test]
    fn mixed_owner_and_kind_drift_rejects_before_runtime_visibility() -> Result<(), Box<dyn Error>>
    {
        let endpoint = EndpointId::try_new("endpoint")?;
        let mut input = input(
            &endpoint,
            vec![("credential-a", CompatibleEgressTarget::Direct)],
            None,
        )?;
        input.bindings[0].credential_upstream_id = UpstreamId::try_new("foreign")?;
        assert!(matches!(
            CompatibleEndpointRuntime::try_new(input),
            Err(CompatibleEndpointRuntimeBuildError::CredentialUpstreamMismatch)
        ));
        Ok(())
    }

    #[test]
    fn stale_revision_and_schedule_reject_before_runtime_visibility() -> Result<(), Box<dyn Error>>
    {
        let endpoint = EndpointId::try_new("endpoint")?;
        let mut stale_revision = input(
            &endpoint,
            vec![("credential-a", CompatibleEgressTarget::Direct)],
            None,
        )?;
        stale_revision.bindings[0].credential_revision = 2;
        assert!(matches!(
            CompatibleEndpointRuntime::try_new(stale_revision),
            Err(CompatibleEndpointRuntimeBuildError::CredentialRevisionMismatch)
        ));

        let mut stale_schedule = input(
            &endpoint,
            vec![("credential-a", CompatibleEgressTarget::Direct)],
            None,
        )?;
        stale_schedule.bindings[0].weight = 2;
        assert!(matches!(
            CompatibleEndpointRuntime::try_new(stale_schedule),
            Err(CompatibleEndpointRuntimeBuildError::CredentialScheduleMismatch)
        ));
        Ok(())
    }

    #[test]
    fn runtime_debug_is_value_free() -> Result<(), Box<dyn Error>> {
        let runtime = CompatibleEndpointRuntime::try_new(input(
            &EndpointId::try_new("endpoint")?,
            vec![("credential-a", CompatibleEgressTarget::Direct)],
            None,
        )?)?;
        let debug = format!("{runtime:?}");
        assert!(!debug.contains("https://relay.example"));
        assert!(!debug.contains("secret-credential-a"));
        Ok(())
    }
}
