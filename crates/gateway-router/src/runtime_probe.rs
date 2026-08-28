//! Transport-neutral, bounded health-probe measurements.
//!
//! P4-04 records sanitized outcomes supplied by an authorized probe runner. It deliberately does
//! not construct HTTP, retain URLs, headers, request/response bodies, or Credential material. The
//! resulting target-local EWMA snapshots are management/diagnostic input; request-time selection
//! continues to consult [`crate::RuntimeHealthRegistry`] only.

use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use gateway_core::{CredentialId, EndpointId};

use crate::{
    DEFAULT_RUNTIME_HEALTH_SHARD_COUNT, MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD,
    MAX_RUNTIME_HEALTH_SHARD_COUNT, RuntimeHealthCircuitProbe, RuntimeHealthCircuitProbeResult,
    RuntimeHealthError, RuntimeHealthKey, RuntimeHealthRegistry, RuntimeHealthRegistryBuildError,
};

/// Fixed-point scale used for health-success EWMA values.
pub const RUNTIME_HEALTH_PROBE_EWMA_SCALE_PER_MILLE: u16 = 1_000;
/// Default observation weight: twenty percent of the next EWMA value comes from the new sample.
pub const DEFAULT_RUNTIME_HEALTH_PROBE_EWMA_ALPHA_PER_MILLE: u16 = 200;

/// Exact non-secret scope for one health probe measurement.
///
/// Model-specific probes always include both Endpoint and Credential: a model entitlement or
/// transient failure must not become a global model verdict or contaminate a sibling Credential.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeHealthProbeTarget {
    /// Probe one protocol-specific Endpoint independently of Credential/model results.
    Endpoint(EndpointId),
    /// Probe one exact Endpoint/Credential binding.
    EndpointCredential {
        /// The protocol-specific Endpoint being measured.
        endpoint_id: EndpointId,
        /// The non-secret stable Credential identity being measured.
        credential_id: CredentialId,
    },
    /// Probe one exact Endpoint/Credential/upstream-model binding.
    EndpointCredentialModel {
        /// The protocol-specific Endpoint being measured.
        endpoint_id: EndpointId,
        /// The non-secret stable Credential identity being measured.
        credential_id: CredentialId,
        /// Exact non-empty upstream model label.
        upstream_model: String,
    },
}

impl RuntimeHealthProbeTarget {
    /// Creates an Endpoint-wide probe target.
    #[must_use]
    pub fn endpoint(endpoint_id: EndpointId) -> Self {
        Self::Endpoint(endpoint_id)
    }

    /// Creates an Endpoint/Credential probe target.
    #[must_use]
    pub fn endpoint_credential(endpoint_id: EndpointId, credential_id: CredentialId) -> Self {
        Self::EndpointCredential {
            endpoint_id,
            credential_id,
        }
    }

    /// Creates an exact Endpoint/Credential/upstream-model probe target.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthProbeTargetError::EmptyUpstreamModel`] before an ambiguous
    /// model-scoped target can be retained.
    pub fn endpoint_credential_model(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        upstream_model: impl Into<String>,
    ) -> Result<Self, RuntimeHealthProbeTargetError> {
        let upstream_model = upstream_model.into();
        if upstream_model.is_empty() {
            return Err(RuntimeHealthProbeTargetError::EmptyUpstreamModel);
        }
        Ok(Self::EndpointCredentialModel {
            endpoint_id,
            credential_id,
            upstream_model,
        })
    }

    /// Returns the protocol-specific Endpoint that owns this probe target.
    #[must_use]
    pub fn endpoint_id(&self) -> &EndpointId {
        match self {
            Self::Endpoint(endpoint_id)
            | Self::EndpointCredential { endpoint_id, .. }
            | Self::EndpointCredentialModel { endpoint_id, .. } => endpoint_id,
        }
    }

    /// Returns the Credential only for a Credential-scoped target.
    #[must_use]
    pub fn credential_id(&self) -> Option<&CredentialId> {
        match self {
            Self::Endpoint(_) => None,
            Self::EndpointCredential { credential_id, .. }
            | Self::EndpointCredentialModel { credential_id, .. } => Some(credential_id),
        }
    }

    /// Returns the exact upstream model only for a model-scoped target.
    #[must_use]
    pub fn upstream_model(&self) -> Option<&str> {
        match self {
            Self::Endpoint(_) | Self::EndpointCredential { .. } => None,
            Self::EndpointCredentialModel { upstream_model, .. } => Some(upstream_model),
        }
    }

    /// Converts this probe target to the matching request-time Circuit/Cooldown key.
    #[must_use]
    pub fn runtime_health_key(&self) -> RuntimeHealthKey {
        match self {
            Self::Endpoint(endpoint_id) => RuntimeHealthKey::endpoint(endpoint_id.clone()),
            Self::EndpointCredential {
                endpoint_id,
                credential_id,
            } => RuntimeHealthKey::endpoint_credential(endpoint_id.clone(), credential_id.clone()),
            Self::EndpointCredentialModel {
                endpoint_id,
                credential_id,
                upstream_model,
            } => RuntimeHealthKey::endpoint_credential_model(
                endpoint_id.clone(),
                credential_id.clone(),
                upstream_model.clone(),
            ),
        }
    }
}

/// Safe construction failure for a model-scoped probe target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthProbeTargetError {
    /// An empty label cannot be an exact upstream-model identity.
    EmptyUpstreamModel,
}

impl fmt::Display for RuntimeHealthProbeTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyUpstreamModel => formatter.write_str("runtime health probe model is empty"),
        }
    }
}

impl Error for RuntimeHealthProbeTargetError {}

/// Sanitized terminal outcome from one completed probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthProbeOutcome {
    /// The probe reached its defined success condition.
    Succeeded {
        /// End-to-end probe latency in whole milliseconds.
        latency_ms: u64,
    },
    /// The probe did not reach its defined success condition.
    Failed {
        /// End-to-end probe latency in whole milliseconds.
        latency_ms: u64,
    },
}

impl RuntimeHealthProbeOutcome {
    const fn latency_ms(self) -> u64 {
        match self {
            Self::Succeeded { latency_ms } | Self::Failed { latency_ms } => latency_ms,
        }
    }

    const fn success_sample_per_mille(self) -> u16 {
        match self {
            Self::Succeeded { .. } => RUNTIME_HEALTH_PROBE_EWMA_SCALE_PER_MILLE,
            Self::Failed { .. } => 0,
        }
    }
}

/// One terminal result that can safely finish a half-open Circuit recovery probe.
///
/// A failed recovery carries its explicit next retry instant. It contains no status code, URL,
/// upstream diagnostic, Credential material, or response data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthCircuitProbeOutcome {
    /// The probe reached its defined success condition and may close the Circuit.
    Succeeded {
        /// End-to-end probe latency in whole milliseconds.
        latency_ms: u64,
    },
    /// The probe did not reach its defined success condition and must reopen the Circuit.
    Failed {
        /// End-to-end probe latency in whole milliseconds.
        latency_ms: u64,
        /// Future Unix-millisecond earliest time for a later controlled recovery probe.
        retry_after_ms: i64,
    },
}

impl RuntimeHealthCircuitProbeOutcome {
    const fn probe_outcome(self) -> RuntimeHealthProbeOutcome {
        match self {
            Self::Succeeded { latency_ms } => RuntimeHealthProbeOutcome::Succeeded { latency_ms },
            Self::Failed { latency_ms, .. } => RuntimeHealthProbeOutcome::Failed { latency_ms },
        }
    }

    const fn circuit_result(self) -> RuntimeHealthCircuitProbeResult {
        match self {
            Self::Succeeded { .. } => RuntimeHealthCircuitProbeResult::Healthy,
            Self::Failed { retry_after_ms, .. } => {
                RuntimeHealthCircuitProbeResult::Unhealthy { retry_after_ms }
            }
        }
    }
}

/// Immutable diagnostic snapshot for one probe target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeHealthProbeSnapshot {
    target: RuntimeHealthProbeTarget,
    observations: u64,
    last_observed_at_ms: i64,
    last_outcome: RuntimeHealthProbeOutcome,
    success_ewma_per_mille: u16,
    latency_ewma_ms: u64,
}

impl RuntimeHealthProbeSnapshot {
    /// Returns the exact non-secret target represented by this snapshot.
    #[must_use]
    pub fn target(&self) -> &RuntimeHealthProbeTarget {
        &self.target
    }

    /// Returns the number of terminal probe observations retained in this target's EWMA.
    #[must_use]
    pub const fn observations(&self) -> u64 {
        self.observations
    }

    /// Returns the explicit Unix-millisecond time of the latest accepted observation.
    #[must_use]
    pub const fn last_observed_at_ms(&self) -> i64 {
        self.last_observed_at_ms
    }

    /// Returns the sanitized latest terminal outcome.
    #[must_use]
    pub const fn last_outcome(&self) -> RuntimeHealthProbeOutcome {
        self.last_outcome
    }

    /// Returns success EWMA from `0` (all failure) to `1000` (all success).
    #[must_use]
    pub const fn success_ewma_per_mille(&self) -> u16 {
        self.success_ewma_per_mille
    }

    /// Returns end-to-end latency EWMA in whole milliseconds.
    #[must_use]
    pub const fn latency_ewma_ms(&self) -> u64 {
        self.latency_ewma_ms
    }
}

/// Validated fixed-point EWMA policy for a probe registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeHealthProbePolicy {
    ewma_alpha_per_mille: u16,
}

impl RuntimeHealthProbePolicy {
    /// Creates a policy whose next EWMA value includes this many per-mille of the new sample.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthProbePolicyError::ZeroEwmaAlpha`] or
    /// [`RuntimeHealthProbePolicyError::EwmaAlphaTooLarge`] before a non-decaying or invalid
    /// policy can be used.
    pub fn try_new(ewma_alpha_per_mille: u16) -> Result<Self, RuntimeHealthProbePolicyError> {
        if ewma_alpha_per_mille == 0 {
            return Err(RuntimeHealthProbePolicyError::ZeroEwmaAlpha);
        }
        if ewma_alpha_per_mille > RUNTIME_HEALTH_PROBE_EWMA_SCALE_PER_MILLE {
            return Err(RuntimeHealthProbePolicyError::EwmaAlphaTooLarge);
        }
        Ok(Self {
            ewma_alpha_per_mille,
        })
    }

    /// Returns the new-sample EWMA weight in per-mille.
    #[must_use]
    pub const fn ewma_alpha_per_mille(self) -> u16 {
        self.ewma_alpha_per_mille
    }
}

impl Default for RuntimeHealthProbePolicy {
    fn default() -> Self {
        // The constant is statically within the checked range.
        Self {
            ewma_alpha_per_mille: DEFAULT_RUNTIME_HEALTH_PROBE_EWMA_ALPHA_PER_MILLE,
        }
    }
}

/// Safe invalid-EWMA policy error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthProbePolicyError {
    /// Zero would make the next observation unable to affect an existing EWMA.
    ZeroEwmaAlpha,
    /// A value above 1000 cannot be a convex fixed-point weight.
    EwmaAlphaTooLarge,
}

impl fmt::Display for RuntimeHealthProbePolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroEwmaAlpha => formatter.write_str("runtime health probe EWMA alpha is zero"),
            Self::EwmaAlphaTooLarge => {
                formatter.write_str("runtime health probe EWMA alpha exceeds one thousand")
            }
        }
    }
}

impl Error for RuntimeHealthProbePolicyError {}

/// Safe probe observation/lookup failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthProbeError {
    /// A target-local observation time regressed and was rejected without mutation.
    ObservationTimeRegressed,
    /// The finite observation counter cannot advance safely.
    ObservationCountOverflow,
    /// A target's deterministic shard is unavailable after a prior panic.
    ShardLockPoisoned,
    /// A new target cannot be retained without exceeding the bounded shard entry limit.
    ShardCapacityExceeded,
}

impl fmt::Display for RuntimeHealthProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObservationTimeRegressed => {
                formatter.write_str("runtime health probe observation time regressed")
            }
            Self::ObservationCountOverflow => {
                formatter.write_str("runtime health probe observation count cannot advance safely")
            }
            Self::ShardLockPoisoned => {
                formatter.write_str("runtime health probe shard is unavailable")
            }
            Self::ShardCapacityExceeded => {
                formatter.write_str("runtime health probe shard is at capacity")
            }
        }
    }
}

impl Error for RuntimeHealthProbeError {}

/// Safe failure while atomically recording and applying a controlled Circuit probe result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthProbeCompletionError {
    /// The target did not match the exact Endpoint/Credential/model identity in the ticket.
    TargetDoesNotMatchCircuitProbe,
    /// The Circuit ticket could not safely change runtime availability state.
    Circuit(RuntimeHealthError),
    /// The target-local EWMA record could not be safely retained.
    Metrics(RuntimeHealthProbeError),
}

impl fmt::Display for RuntimeHealthProbeCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetDoesNotMatchCircuitProbe => {
                formatter.write_str("runtime health probe target does not match its Circuit ticket")
            }
            Self::Circuit(_) => {
                formatter.write_str("runtime health Circuit probe could not complete")
            }
            Self::Metrics(_) => {
                formatter.write_str("runtime health probe metrics could not be recorded")
            }
        }
    }
}

impl Error for RuntimeHealthProbeCompletionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TargetDoesNotMatchCircuitProbe => None,
            Self::Circuit(error) => Some(error),
            Self::Metrics(error) => Some(error),
        }
    }
}

/// Bounded, sharded non-secret EWMA measurement registry.
///
/// This registry records terminal results only. It cannot perform a probe, open a Circuit, or
/// silently close one: callers use the matching [`RuntimeHealthKey`] and the controlled
/// `RuntimeHealthRegistry` Circuit-probe ticket APIs for state transitions.
pub struct RuntimeHealthProbeRegistry {
    policy: RuntimeHealthProbePolicy,
    shards: Box<[RwLock<BTreeMap<RuntimeHealthProbeTarget, RuntimeHealthProbeSnapshot>>]>,
}

impl RuntimeHealthProbeRegistry {
    /// Creates the default bounded sharded probe registry.
    #[must_use]
    pub fn new() -> Self {
        Self::with_policy(RuntimeHealthProbePolicy::default())
    }

    /// Creates the default-shard probe registry with one validated EWMA policy.
    #[must_use]
    pub fn with_policy(policy: RuntimeHealthProbePolicy) -> Self {
        Self {
            policy,
            shards: build_shards(DEFAULT_RUNTIME_HEALTH_SHARD_COUNT),
        }
    }

    /// Creates a probe registry with an explicit bounded runtime-shard count.
    ///
    /// # Errors
    ///
    /// Returns the same safe count error as the paired runtime-health registry before allocation.
    pub fn try_with_policy_and_shards(
        policy: RuntimeHealthProbePolicy,
        shard_count: usize,
    ) -> Result<Self, RuntimeHealthRegistryBuildError> {
        validate_shard_count(shard_count)?;
        Ok(Self {
            policy,
            shards: build_shards(shard_count),
        })
    }

    /// Returns the validated fixed-point policy used by this registry.
    #[must_use]
    pub const fn policy(&self) -> RuntimeHealthProbePolicy {
        self.policy
    }

    /// Returns the fixed number of independently locked probe shards.
    #[must_use]
    pub const fn shard_count(&self) -> usize {
        self.shards.len()
    }

    /// Records one terminal probe outcome using an explicit deterministic observation time.
    ///
    /// The first observation initializes both EWMAs exactly to its samples. Later observations
    /// must not move backwards in time for the same exact target. The result contains no URL,
    /// header, Credential material, response body, status text, or transport diagnostic.
    ///
    /// # Errors
    ///
    /// Returns a safe [`RuntimeHealthProbeError`] before mutation for a regressed timestamp, a
    /// finite-counter overflow, poisoned shard, or full bounded shard.
    pub fn record(
        &self,
        target: RuntimeHealthProbeTarget,
        outcome: RuntimeHealthProbeOutcome,
        observed_at_ms: i64,
    ) -> Result<RuntimeHealthProbeSnapshot, RuntimeHealthProbeError> {
        let mut snapshots = self.write_shard(&target)?;
        let next = Self::next_snapshot(&snapshots, &target, outcome, observed_at_ms, self.policy)?;
        snapshots.insert(target, next.clone());
        Ok(next)
    }

    /// Starts one bounded half-open recovery probe for the exact supplied target, if due.
    ///
    /// This merely obtains the exclusive non-secret ticket. The caller must finish it through
    /// [`Self::complete_circuit_probe`] with a terminal sanitized result; ordinary scheduling stays
    /// unavailable throughout the half-open interval.
    ///
    /// # Errors
    ///
    /// Returns the underlying safe runtime-health error. `Ok(None)` means no Circuit is due or a
    /// current ticket still owns recovery.
    pub fn begin_circuit_probe(
        runtime_health: &RuntimeHealthRegistry,
        target: &RuntimeHealthProbeTarget,
        probe_expires_at_ms: i64,
    ) -> Result<Option<RuntimeHealthCircuitProbe>, RuntimeHealthError> {
        runtime_health.begin_circuit_probe(&target.runtime_health_key(), probe_expires_at_ms)
    }

    /// Records one terminal half-open outcome and changes the exact matching Circuit as one
    /// target-local operation.
    ///
    /// Metrics are prevalidated while their target shard is exclusively locked. Only then is the
    /// matching ticket consumed by the runtime-health registry; a successful Circuit transition is
    /// followed immediately by the already-validated snapshot insertion. The target shard is not
    /// used on request-time selection, and no HTTP, Provider, `SQLite`, body, header, URL, or Secret
    /// enters this path.
    ///
    /// # Errors
    ///
    /// Returns a safe [`RuntimeHealthProbeCompletionError`] without partially inserting a metric
    /// snapshot when the target/ticket mismatch, the observation is invalid, or the Circuit ticket
    /// cannot safely complete.
    pub fn complete_circuit_probe(
        &self,
        runtime_health: &RuntimeHealthRegistry,
        target: RuntimeHealthProbeTarget,
        probe: RuntimeHealthCircuitProbe,
        outcome: RuntimeHealthCircuitProbeOutcome,
        observed_at_ms: i64,
    ) -> Result<RuntimeHealthProbeSnapshot, RuntimeHealthProbeCompletionError> {
        let target_key = target.runtime_health_key();
        if probe.key() != &target_key {
            return Err(RuntimeHealthProbeCompletionError::TargetDoesNotMatchCircuitProbe);
        }
        let probe_outcome = outcome.probe_outcome();
        let mut snapshots = self
            .write_shard(&target)
            .map_err(RuntimeHealthProbeCompletionError::Metrics)?;
        let next = Self::next_snapshot(
            &snapshots,
            &target,
            probe_outcome,
            observed_at_ms,
            self.policy,
        )
        .map_err(RuntimeHealthProbeCompletionError::Metrics)?;
        runtime_health
            .complete_circuit_probe(probe, outcome.circuit_result())
            .map_err(RuntimeHealthProbeCompletionError::Circuit)?;
        snapshots.insert(target, next.clone());
        Ok(next)
    }

    /// Returns the immutable EWMA snapshot for one exact target, if it has terminal observations.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthProbeError::ShardLockPoisoned`] if the target's fixed shard is
    /// unavailable.
    pub fn snapshot(
        &self,
        target: &RuntimeHealthProbeTarget,
    ) -> Result<Option<RuntimeHealthProbeSnapshot>, RuntimeHealthProbeError> {
        Ok(self.read_shard(target)?.get(target).cloned())
    }

    /// Counts retained target snapshots for bounded diagnostics and tests.
    ///
    /// This management/testing helper reads every shard and is not a request-time operation.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthProbeError::ShardLockPoisoned`] when any shard is unavailable.
    pub fn entry_count(&self) -> Result<usize, RuntimeHealthProbeError> {
        let mut count = 0_usize;
        for shard in &self.shards {
            let snapshots = shard
                .read()
                .map_err(|_| RuntimeHealthProbeError::ShardLockPoisoned)?;
            count = count.saturating_add(snapshots.len());
        }
        Ok(count)
    }

    fn read_shard(
        &self,
        target: &RuntimeHealthProbeTarget,
    ) -> Result<
        RwLockReadGuard<'_, BTreeMap<RuntimeHealthProbeTarget, RuntimeHealthProbeSnapshot>>,
        RuntimeHealthProbeError,
    > {
        self.shards[self.shard_index(target)]
            .read()
            .map_err(|_| RuntimeHealthProbeError::ShardLockPoisoned)
    }

    fn write_shard(
        &self,
        target: &RuntimeHealthProbeTarget,
    ) -> Result<
        RwLockWriteGuard<'_, BTreeMap<RuntimeHealthProbeTarget, RuntimeHealthProbeSnapshot>>,
        RuntimeHealthProbeError,
    > {
        self.shards[self.shard_index(target)]
            .write()
            .map_err(|_| RuntimeHealthProbeError::ShardLockPoisoned)
    }

    fn next_snapshot(
        snapshots: &BTreeMap<RuntimeHealthProbeTarget, RuntimeHealthProbeSnapshot>,
        target: &RuntimeHealthProbeTarget,
        outcome: RuntimeHealthProbeOutcome,
        observed_at_ms: i64,
        policy: RuntimeHealthProbePolicy,
    ) -> Result<RuntimeHealthProbeSnapshot, RuntimeHealthProbeError> {
        if let Some(previous) = snapshots.get(target) {
            if observed_at_ms < previous.last_observed_at_ms {
                return Err(RuntimeHealthProbeError::ObservationTimeRegressed);
            }
            Ok(RuntimeHealthProbeSnapshot {
                target: target.clone(),
                observations: previous
                    .observations
                    .checked_add(1)
                    .ok_or(RuntimeHealthProbeError::ObservationCountOverflow)?,
                last_observed_at_ms: observed_at_ms,
                last_outcome: outcome,
                success_ewma_per_mille: ewma_u16(
                    previous.success_ewma_per_mille,
                    outcome.success_sample_per_mille(),
                    policy.ewma_alpha_per_mille,
                ),
                latency_ewma_ms: ewma_u64(
                    previous.latency_ewma_ms,
                    outcome.latency_ms(),
                    policy.ewma_alpha_per_mille,
                ),
            })
        } else {
            ensure_insert_capacity(snapshots)?;
            Ok(RuntimeHealthProbeSnapshot {
                target: target.clone(),
                observations: 1,
                last_observed_at_ms: observed_at_ms,
                last_outcome: outcome,
                success_ewma_per_mille: outcome.success_sample_per_mille(),
                latency_ewma_ms: outcome.latency_ms(),
            })
        }
    }

    fn shard_index(&self, target: &RuntimeHealthProbeTarget) -> usize {
        let mut hasher = DefaultHasher::new();
        target.hash(&mut hasher);
        let Ok(mask) = u64::try_from(self.shards.len() - 1) else {
            return 0;
        };
        usize::try_from(hasher.finish() & mask).unwrap_or_default()
    }
}

impl Default for RuntimeHealthProbeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RuntimeHealthProbeRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeHealthProbeRegistry")
            .field("policy", &self.policy)
            .field("shard_count", &self.shard_count())
            .field(
                "entries_per_shard_limit",
                &MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD,
            )
            .finish_non_exhaustive()
    }
}

fn build_shards(
    shard_count: usize,
) -> Box<[RwLock<BTreeMap<RuntimeHealthProbeTarget, RuntimeHealthProbeSnapshot>>]> {
    std::iter::repeat_with(|| RwLock::new(BTreeMap::new()))
        .take(shard_count)
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn validate_shard_count(shard_count: usize) -> Result<(), RuntimeHealthRegistryBuildError> {
    if shard_count == 0 {
        return Err(RuntimeHealthRegistryBuildError::ZeroShardCount);
    }
    if !shard_count.is_power_of_two() {
        return Err(RuntimeHealthRegistryBuildError::NonPowerOfTwoShardCount);
    }
    if shard_count > MAX_RUNTIME_HEALTH_SHARD_COUNT {
        return Err(RuntimeHealthRegistryBuildError::TooManyShards);
    }
    Ok(())
}

fn ensure_insert_capacity(
    snapshots: &BTreeMap<RuntimeHealthProbeTarget, RuntimeHealthProbeSnapshot>,
) -> Result<(), RuntimeHealthProbeError> {
    if snapshots.len() >= MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD {
        return Err(RuntimeHealthProbeError::ShardCapacityExceeded);
    }
    Ok(())
}

fn ewma_u16(previous: u16, sample: u16, alpha_per_mille: u16) -> u16 {
    u16::try_from(ewma_u64(
        u64::from(previous),
        u64::from(sample),
        alpha_per_mille,
    ))
    .unwrap_or(RUNTIME_HEALTH_PROBE_EWMA_SCALE_PER_MILLE)
}

fn ewma_u64(previous: u64, sample: u64, alpha_per_mille: u16) -> u64 {
    let alpha = u128::from(alpha_per_mille);
    let retained = u128::from(RUNTIME_HEALTH_PROBE_EWMA_SCALE_PER_MILLE) - alpha;
    let numerator = u128::from(previous)
        .saturating_mul(retained)
        .saturating_add(u128::from(sample).saturating_mul(alpha));
    let rounded = numerator
        .saturating_add(u128::from(RUNTIME_HEALTH_PROBE_EWMA_SCALE_PER_MILLE) / 2)
        / u128::from(RUNTIME_HEALTH_PROBE_EWMA_SCALE_PER_MILLE);
    u64::try_from(rounded).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        sync::{
            Arc,
            atomic::{AtomicI64, Ordering},
        },
    };

    use gateway_core::{CredentialId, EndpointId};

    use super::{
        RuntimeHealthCircuitProbeOutcome, RuntimeHealthProbeError, RuntimeHealthProbeOutcome,
        RuntimeHealthProbePolicy, RuntimeHealthProbePolicyError, RuntimeHealthProbeRegistry,
        RuntimeHealthProbeTarget, RuntimeHealthProbeTargetError,
    };
    use crate::{
        RuntimeHealthAvailability, RuntimeHealthClock, RuntimeHealthClockError,
        RuntimeHealthRegistry,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn model_scoped_probe_ewma_is_exact_target_isolated() -> TestResult {
        let probes =
            RuntimeHealthProbeRegistry::with_policy(RuntimeHealthProbePolicy::try_new(200)?);
        let target_a = model_target("endpoint-a", "credential-a", "model-a")?;
        let target_b = model_target("endpoint-a", "credential-a", "model-b")?;

        let first = probes.record(
            target_a.clone(),
            RuntimeHealthProbeOutcome::Succeeded { latency_ms: 100 },
            1_000,
        )?;
        assert_eq!(first.observations(), 1);
        assert_eq!(first.success_ewma_per_mille(), 1_000);
        assert_eq!(first.latency_ewma_ms(), 100);

        let second = probes.record(
            target_a.clone(),
            RuntimeHealthProbeOutcome::Failed { latency_ms: 300 },
            1_001,
        )?;
        assert_eq!(second.observations(), 2);
        assert_eq!(second.success_ewma_per_mille(), 800);
        assert_eq!(second.latency_ewma_ms(), 140);
        assert_eq!(probes.snapshot(&target_b)?, None);

        let other = probes.record(
            target_b.clone(),
            RuntimeHealthProbeOutcome::Succeeded { latency_ms: 20 },
            1_000,
        )?;
        assert_eq!(other.success_ewma_per_mille(), 1_000);
        assert_eq!(other.latency_ewma_ms(), 20);
        assert_eq!(probes.snapshot(&target_a)?, Some(second));
        Ok(())
    }

    #[test]
    fn probes_reject_ambiguous_targets_and_time_regression_without_mutation() -> TestResult {
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential = CredentialId::try_new("credential-a")?;
        assert_eq!(
            RuntimeHealthProbeTarget::endpoint_credential_model(
                endpoint.clone(),
                credential.clone(),
                ""
            ),
            Err(RuntimeHealthProbeTargetError::EmptyUpstreamModel)
        );
        let probes = RuntimeHealthProbeRegistry::new();
        let target = RuntimeHealthProbeTarget::endpoint_credential(endpoint, credential);
        let recorded = probes.record(
            target.clone(),
            RuntimeHealthProbeOutcome::Succeeded { latency_ms: 10 },
            200,
        )?;
        assert_eq!(
            probes.record(
                target.clone(),
                RuntimeHealthProbeOutcome::Failed { latency_ms: 11 },
                199,
            ),
            Err(RuntimeHealthProbeError::ObservationTimeRegressed)
        );
        assert_eq!(probes.snapshot(&target)?, Some(recorded));
        Ok(())
    }

    #[test]
    fn controlled_circuit_probe_updates_exact_ewma_and_recovery_state() -> TestResult {
        let clock = Arc::new(FixedRuntimeHealthClock::new(100));
        let runtime_health = RuntimeHealthRegistry::with_clock(clock.clone());
        let probes = RuntimeHealthProbeRegistry::new();
        let target = model_target("endpoint-a", "credential-a", "model-a")?;
        let key = target.runtime_health_key();

        runtime_health.open_circuit_until(key.clone(), 200)?;
        clock.set_now_ms(200);
        let ticket =
            RuntimeHealthProbeRegistry::begin_circuit_probe(&runtime_health, &target, 250)?
                .ok_or("due Circuit did not create a probe ticket")?;
        let recovered = probes.complete_circuit_probe(
            &runtime_health,
            target.clone(),
            ticket,
            RuntimeHealthCircuitProbeOutcome::Succeeded { latency_ms: 25 },
            201,
        )?;
        assert_eq!(recovered.observations(), 1);
        assert_eq!(recovered.success_ewma_per_mille(), 1_000);
        assert_eq!(recovered.latency_ewma_ms(), 25);
        assert_eq!(
            runtime_health.availability(&key)?,
            RuntimeHealthAvailability::Available
        );

        runtime_health.open_circuit_until(key.clone(), 300)?;
        clock.set_now_ms(300);
        let ticket =
            RuntimeHealthProbeRegistry::begin_circuit_probe(&runtime_health, &target, 350)?
                .ok_or("reopened Circuit did not create a probe ticket")?;
        let failed = probes.complete_circuit_probe(
            &runtime_health,
            target.clone(),
            ticket,
            RuntimeHealthCircuitProbeOutcome::Failed {
                latency_ms: 40,
                retry_after_ms: 400,
            },
            301,
        )?;
        assert_eq!(failed.observations(), 2);
        assert_eq!(failed.success_ewma_per_mille(), 800);
        assert_eq!(failed.latency_ewma_ms(), 28);
        assert_eq!(
            runtime_health.availability(&key)?,
            RuntimeHealthAvailability::CircuitOpen {
                retry_after_ms: 400
            }
        );
        Ok(())
    }

    #[test]
    fn probe_policy_rejects_non_convex_ewma_weights() {
        assert_eq!(
            RuntimeHealthProbePolicy::try_new(0),
            Err(RuntimeHealthProbePolicyError::ZeroEwmaAlpha)
        );
        assert_eq!(
            RuntimeHealthProbePolicy::try_new(1_001),
            Err(RuntimeHealthProbePolicyError::EwmaAlphaTooLarge)
        );
    }

    fn model_target(
        endpoint: &str,
        credential: &str,
        model: &str,
    ) -> Result<RuntimeHealthProbeTarget, Box<dyn Error>> {
        Ok(RuntimeHealthProbeTarget::endpoint_credential_model(
            EndpointId::try_new(endpoint)?,
            CredentialId::try_new(credential)?,
            model,
        )?)
    }

    #[derive(Debug)]
    struct FixedRuntimeHealthClock {
        now_ms: AtomicI64,
    }

    impl FixedRuntimeHealthClock {
        const fn new(now_ms: i64) -> Self {
            Self {
                now_ms: AtomicI64::new(now_ms),
            }
        }

        fn set_now_ms(&self, now_ms: i64) {
            self.now_ms.store(now_ms, Ordering::Release);
        }
    }

    impl RuntimeHealthClock for FixedRuntimeHealthClock {
        fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
            Ok(self.now_ms.load(Ordering::Acquire))
        }
    }
}
