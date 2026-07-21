//! Bounded, sharded runtime availability state for Endpoint and Credential scheduling.
//!
//! This P3/P4 primitive records bounded Cooldown, Circuit, and exact Credential-account state.
//! It has no HTTP handler, retry budget, persistence, or Provider behavior. Each lookup takes at
//! most one fixed shard lock; it never touches `SQLite`, a configuration file, or a global
//! scheduling lock.

use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_core::{CredentialId, EndpointId};

/// Fixed default number of independently locked runtime-health shards.
pub const DEFAULT_RUNTIME_HEALTH_SHARD_COUNT: usize = 64;
/// Largest accepted shard count, preventing construction-time allocation amplification.
pub const MAX_RUNTIME_HEALTH_SHARD_COUNT: usize = 1024;
/// Largest retained Endpoint or Endpoint/Credential state entries in one shard.
pub const MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD: usize = 1024;

/// Non-secret runtime state identity.
///
/// An Endpoint-level state isolates protocol-specific availability. An Endpoint/Credential state
/// further isolates a Credential's transient availability at that Endpoint, so a shared Credential
/// does not make another Endpoint unavailable by association.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeHealthKey {
    /// State shared by all Candidate selections for one Endpoint.
    Endpoint(EndpointId),
    /// State affecting one bound Credential at one Endpoint.
    EndpointCredential {
        /// The protocol-specific Endpoint that owns this transient state.
        endpoint_id: EndpointId,
        /// The non-secret stable Credential identity within the Endpoint scope.
        credential_id: CredentialId,
    },
    /// State affecting one exact upstream model through one Endpoint/Credential binding.
    ///
    /// The model label is the non-secret exact value selected by a compiler-approved Candidate.
    /// It is deliberately not a global model key: another Endpoint, Credential, or differently
    /// cased upstream label remains isolated.
    EndpointCredentialModel {
        /// The protocol-specific Endpoint that owns this transient state.
        endpoint_id: EndpointId,
        /// The non-secret stable Credential identity within the Endpoint scope.
        credential_id: CredentialId,
        /// The exact non-secret upstream model label.
        upstream_model: String,
    },
}

impl RuntimeHealthKey {
    /// Creates an Endpoint-wide runtime state key.
    #[must_use]
    pub fn endpoint(endpoint_id: EndpointId) -> Self {
        Self::Endpoint(endpoint_id)
    }

    /// Creates a Credential state key isolated to one Endpoint.
    #[must_use]
    pub fn endpoint_credential(endpoint_id: EndpointId, credential_id: CredentialId) -> Self {
        Self::EndpointCredential {
            endpoint_id,
            credential_id,
        }
    }

    /// Creates state for one exact Endpoint/Credential/upstream-model binding.
    #[must_use]
    pub fn endpoint_credential_model(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        upstream_model: impl Into<String>,
    ) -> Self {
        Self::EndpointCredentialModel {
            endpoint_id,
            credential_id,
            upstream_model: upstream_model.into(),
        }
    }

    /// Returns the Endpoint owning this runtime state.
    #[must_use]
    pub fn endpoint_id(&self) -> &EndpointId {
        match self {
            Self::Endpoint(endpoint_id)
            | Self::EndpointCredential { endpoint_id, .. }
            | Self::EndpointCredentialModel { endpoint_id, .. } => endpoint_id,
        }
    }

    /// Returns a Credential only for Endpoint/Credential-scoped state.
    #[must_use]
    pub fn credential_id(&self) -> Option<&CredentialId> {
        match self {
            Self::Endpoint(_) => None,
            Self::EndpointCredential { credential_id, .. }
            | Self::EndpointCredentialModel { credential_id, .. } => Some(credential_id),
        }
    }

    /// Returns the exact upstream model only for an Endpoint/Credential/model key.
    #[must_use]
    pub fn upstream_model(&self) -> Option<&str> {
        match self {
            Self::Endpoint(_) | Self::EndpointCredential { .. } => None,
            Self::EndpointCredentialModel { upstream_model, .. } => Some(upstream_model),
        }
    }
}

/// Effective request-time eligibility for one runtime state key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthAvailability {
    /// No active temporary state blocks selection.
    Available,
    /// A short transient Cooldown blocks selection until its deadline.
    CoolingDown {
        /// Unix-millisecond instant at which ordinary scheduling may resume.
        until_ms: i64,
    },
    /// A Circuit is open and requires an explicit later recovery/probe decision.
    CircuitOpen {
        /// Earliest Unix-millisecond instant at which a later component may consider recovery.
        retry_after_ms: i64,
    },
    /// A provider-classified 403 blocks this exact Endpoint/Credential account until a controlled
    /// background recovery completes.
    AccountForbidden,
    /// One controlled recovery owns this exact forbidden Endpoint/Credential account.
    AccountRecoveryInFlight {
        /// Exclusive recovery-ticket deadline.
        expires_at_ms: i64,
    },
}

/// Exact Endpoint/Credential account status for a read-only management query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCredentialAccountStatus {
    /// No retained provider-forbidden state blocks this binding.
    Available,
    /// A provider-classified 403 blocked this exact binding.
    Forbidden,
    /// A non-cloneable controlled recovery ticket is outstanding for this exact binding.
    RecoveryInFlight {
        /// Exclusive recovery-ticket deadline.
        expires_at_ms: i64,
    },
}

/// One exclusive, bounded half-open recovery probe acquired from an open Circuit.
///
/// The ticket has no Credential material and is intentionally non-cloneable. Its private
/// generation prevents a late result from one abandoned probe from closing a newer Circuit.
#[derive(Debug, Eq, PartialEq)]
pub struct RuntimeHealthCircuitProbe {
    key: RuntimeHealthKey,
    probe_id: u64,
    expires_at_ms: i64,
}

/// One exclusive, bounded account-recovery ticket for an exact forbidden Endpoint/Credential.
///
/// A ticket carries no Credential bytes and may only be completed once while it is current and
/// unexpired. It is issued by a background/controller owner, never by request-time scheduling.
#[derive(Debug, Eq, PartialEq)]
pub struct RuntimeHealthAccountRecoveryProbe {
    key: RuntimeHealthKey,
    probe_id: u64,
    expires_at_ms: i64,
}

impl RuntimeHealthAccountRecoveryProbe {
    /// Returns the exact non-secret binding being recovered.
    #[must_use]
    pub fn key(&self) -> &RuntimeHealthKey {
        &self.key
    }

    /// Returns the exclusive recovery-ticket deadline.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
}

impl RuntimeHealthCircuitProbe {
    /// Returns the exact non-secret runtime key being recovered.
    #[must_use]
    pub fn key(&self) -> &RuntimeHealthKey {
        &self.key
    }

    /// Returns the inclusive-failure-exclusive-success deadline for the probe ticket.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
}

/// Sanitized outcome used to finish one controlled Circuit recovery probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthCircuitProbeResult {
    /// The controlled probe completed successfully and may close the Circuit.
    Healthy,
    /// The controlled probe failed; the Circuit remains open until this future retry instant.
    Unhealthy {
        /// Future Unix-millisecond earliest time at which another recovery probe may start.
        retry_after_ms: i64,
    },
}

/// Sanitized terminal outcome for one account recovery ticket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthAccountRecoveryResult {
    /// The controlled background check established that this exact account may schedule again.
    Allowed,
    /// The account remains forbidden; ordinary scheduling stays closed.
    Forbidden,
}

impl RuntimeHealthAvailability {
    /// Returns whether ordinary scheduling may use this state now.
    #[must_use]
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Supplies a Unix-millisecond timestamp for runtime availability checks.
pub trait RuntimeHealthClock: Send + Sync {
    /// Returns the current Unix-millisecond timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthClockError::Unavailable`] when local time cannot be represented in
    /// the runtime timestamp domain.
    fn now_ms(&self) -> Result<i64, RuntimeHealthClockError>;
}

/// System clock implementation for normal request-time availability checks.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemRuntimeHealthClock;

impl RuntimeHealthClock for SystemRuntimeHealthClock {
    fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| RuntimeHealthClockError::Unavailable)?;
        i64::try_from(elapsed.as_millis()).map_err(|_| RuntimeHealthClockError::Unavailable)
    }
}

/// Safe failures from the runtime-health clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthClockError {
    /// System time was before the Unix epoch or outside the supported millisecond range.
    Unavailable,
}

impl fmt::Display for RuntimeHealthClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("runtime health clock is unavailable"),
        }
    }
}

impl Error for RuntimeHealthClockError {}

/// Safe construction failures for a sharded runtime-health registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthRegistryBuildError {
    /// A registry requires at least one independently locked shard.
    ZeroShardCount,
    /// A non-power-of-two count would make the fixed hash partitioning ambiguous.
    NonPowerOfTwoShardCount,
    /// The requested count would exceed the finite runtime allocation limit.
    TooManyShards,
}

impl fmt::Display for RuntimeHealthRegistryBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ZeroShardCount => "runtime health shard count must be positive",
            Self::NonPowerOfTwoShardCount => "runtime health shard count must be a power of two",
            Self::TooManyShards => "runtime health shard count exceeds its finite limit",
        };
        formatter.write_str(message)
    }
}

impl Error for RuntimeHealthRegistryBuildError {}

/// Safe runtime-health mutation or lookup failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealthError {
    /// The current time could not be obtained safely.
    Clock(RuntimeHealthClockError),
    /// A Cooldown or Circuit deadline was not strictly after the current time.
    DeadlineNotInFuture,
    /// A single independently locked state shard was poisoned by a prior panic.
    ShardLockPoisoned,
    /// A shard could not retain a new state without exceeding its bounded entry limit.
    ShardCapacityExceeded,
    /// The finite non-zero Circuit probe generation sequence cannot advance safely.
    CircuitProbeIdOverflow,
    /// A recovery ticket is no longer the current, unexpired half-open probe for its key.
    StaleCircuitProbe,
    /// The finite non-zero account-recovery ticket sequence cannot advance safely.
    AccountRecoveryProbeIdOverflow,
    /// An account-recovery ticket is expired, superseded, or no longer owns its exact binding.
    StaleAccountRecoveryProbe,
}

impl fmt::Display for RuntimeHealthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Clock(_) => "runtime health clock is unavailable",
            Self::DeadlineNotInFuture => "runtime health deadline is not in the future",
            Self::ShardLockPoisoned => "runtime health shard is unavailable",
            Self::ShardCapacityExceeded => "runtime health shard is at capacity",
            Self::CircuitProbeIdOverflow => {
                "runtime health circuit probe identifier cannot advance safely"
            }
            Self::StaleCircuitProbe => "runtime health circuit probe is stale",
            Self::AccountRecoveryProbeIdOverflow => {
                "runtime health account recovery probe identifier cannot advance safely"
            }
            Self::StaleAccountRecoveryProbe => "runtime health account recovery probe is stale",
        };
        formatter.write_str(message)
    }
}

impl Error for RuntimeHealthError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RuntimeHealthClockError> for RuntimeHealthError {
    fn from(error: RuntimeHealthClockError) -> Self {
        Self::Clock(error)
    }
}

/// Fixed-shard, process-local runtime state for transient scheduling eligibility.
///
/// New state entries are bounded per shard. Ordinary availability reads take a read lock for exactly
/// one deterministic shard; writes take a write lock for that same shard only. There is no global
/// lock, persistence handle, task queue, or network operation.
pub struct RuntimeHealthRegistry {
    clock: Arc<dyn RuntimeHealthClock>,
    shards: Box<[RwLock<BTreeMap<RuntimeHealthKey, RuntimeHealthState>>]>,
    next_circuit_probe_id: AtomicU64,
    next_account_recovery_probe_id: AtomicU64,
}

impl RuntimeHealthRegistry {
    /// Creates the default fixed-shard registry with the local system clock.
    #[must_use]
    pub fn new() -> Self {
        Self::with_clock(Arc::new(SystemRuntimeHealthClock))
    }

    /// Creates the default fixed-shard registry with an injected clock.
    #[must_use]
    pub fn with_clock(clock: Arc<dyn RuntimeHealthClock>) -> Self {
        Self {
            clock,
            shards: build_shards(DEFAULT_RUNTIME_HEALTH_SHARD_COUNT),
            next_circuit_probe_id: AtomicU64::new(1),
            next_account_recovery_probe_id: AtomicU64::new(1),
        }
    }

    /// Creates a bounded registry with an explicit power-of-two shard count.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthRegistryBuildError`] before allocating state when `shard_count` is
    /// zero, not a power of two, or exceeds the finite limit.
    pub fn try_with_clock_and_shards(
        clock: Arc<dyn RuntimeHealthClock>,
        shard_count: usize,
    ) -> Result<Self, RuntimeHealthRegistryBuildError> {
        validate_shard_count(shard_count)?;
        Ok(Self {
            clock,
            shards: build_shards(shard_count),
            next_circuit_probe_id: AtomicU64::new(1),
            next_account_recovery_probe_id: AtomicU64::new(1),
        })
    }

    /// Returns the fixed number of independently locked shards.
    #[must_use]
    pub const fn shard_count(&self) -> usize {
        self.shards.len()
    }

    /// Returns the effective availability for a state key at the current clock instant.
    ///
    /// An absent key is available. An expired Cooldown becomes available without mutating the
    /// request path. A Circuit remains open until a later component records successful recovery;
    /// P4 owns active probing and half-open behavior.
    ///
    /// # Errors
    ///
    /// Returns a safe [`RuntimeHealthError`] when the clock is unavailable or this key's isolated
    /// shard cannot be read.
    pub fn availability(
        &self,
        key: &RuntimeHealthKey,
    ) -> Result<RuntimeHealthAvailability, RuntimeHealthError> {
        let now_ms = self.clock.now_ms()?;
        self.availability_at(key, now_ms)
    }

    /// Returns availability using an explicit timestamp for deterministic callers and tests.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthError::ShardLockPoisoned`] when this key's isolated shard cannot be
    /// read.
    pub fn availability_at(
        &self,
        key: &RuntimeHealthKey,
        now_ms: i64,
    ) -> Result<RuntimeHealthAvailability, RuntimeHealthError> {
        let states = self.read_shard(key)?;
        Ok(states
            .get(key)
            .map_or(RuntimeHealthAvailability::Available, |state| {
                state.availability_at(now_ms)
            }))
    }

    /// Returns whether an Endpoint has no active transient runtime block.
    ///
    /// Clock and shard failures are fail-closed and therefore return `false`.
    #[must_use]
    pub fn endpoint_is_available(&self, endpoint_id: &EndpointId) -> bool {
        self.availability(&RuntimeHealthKey::endpoint(endpoint_id.clone()))
            .is_ok_and(RuntimeHealthAvailability::is_available)
    }

    /// Returns whether a Credential remains available at one Endpoint.
    ///
    /// Clock and shard failures are fail-closed and therefore return `false`.
    #[must_use]
    pub fn endpoint_credential_is_available(
        &self,
        endpoint_id: &EndpointId,
        credential_id: &CredentialId,
    ) -> bool {
        self.availability(&RuntimeHealthKey::endpoint_credential(
            endpoint_id.clone(),
            credential_id.clone(),
        ))
        .is_ok_and(RuntimeHealthAvailability::is_available)
    }

    /// Returns the exact provider-account state for one Endpoint/Credential binding at an explicit
    /// observation time.
    ///
    /// This is a read-only management query. Endpoint-wide and model-only Health states do not
    /// masquerade as an account status; only the exact binding-level 403/recovery state appears.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthError::ShardLockPoisoned`] when the exact state shard is unavailable.
    pub fn credential_account_status_at(
        &self,
        endpoint_id: &EndpointId,
        credential_id: &CredentialId,
        now_ms: i64,
    ) -> Result<RuntimeCredentialAccountStatus, RuntimeHealthError> {
        let key = RuntimeHealthKey::endpoint_credential(endpoint_id.clone(), credential_id.clone());
        let states = self.read_shard(&key)?;
        Ok(states
            .get(&key)
            .map_or(RuntimeCredentialAccountStatus::Available, |state| {
                state.account_status_at(now_ms)
            }))
    }

    /// Returns whether an exact Endpoint/Credential/upstream-model binding is schedulable.
    ///
    /// Clock and shard failures are fail-closed and therefore return `false`. This precise scope
    /// lets a model-specific probe exclude only the failed binding while preserving another model,
    /// Credential, or Endpoint.
    #[must_use]
    pub fn endpoint_credential_model_is_available(
        &self,
        endpoint_id: &EndpointId,
        credential_id: &CredentialId,
        upstream_model: &str,
    ) -> bool {
        self.availability(&RuntimeHealthKey::endpoint_credential_model(
            endpoint_id.clone(),
            credential_id.clone(),
            upstream_model,
        ))
        .is_ok_and(RuntimeHealthAvailability::is_available)
    }

    /// Records a provider-classified 403 for one exact Endpoint/Credential binding.
    ///
    /// The state is not time-expiring: a generic cooldown, successful unrelated Circuit probe, or
    /// another model's result cannot silently reopen a forbidden account. Only
    /// [`Self::complete_account_recovery`] may remove this block.
    ///
    /// # Errors
    ///
    /// Returns a safe [`RuntimeHealthError`] without retaining a partial state when the clock or
    /// binding's isolated shard is unavailable or full.
    pub fn mark_credential_forbidden(
        &self,
        endpoint_id: EndpointId,
        credential_id: CredentialId,
    ) -> Result<(), RuntimeHealthError> {
        let key = RuntimeHealthKey::endpoint_credential(endpoint_id, credential_id);
        let now_ms = self.clock.now_ms()?;
        let mut states = self.write_shard(&key)?;
        if let Some(state) = states.get_mut(&key) {
            *state = RuntimeHealthState::AccountForbidden;
            return Ok(());
        }
        ensure_insert_capacity(&mut states, now_ms)?;
        states.insert(key, RuntimeHealthState::AccountForbidden);
        Ok(())
    }

    /// Starts one controlled recovery for an exact forbidden Endpoint/Credential binding.
    ///
    /// Ordinary scheduling remains closed while the returned non-cloneable ticket exists. An
    /// expired ticket may be replaced, but a late completion can never overwrite its successor.
    /// This method does not send a Provider request; a background/controller owner chooses how to
    /// perform an authorized recovery check.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthError`] for unavailable time, an invalid deadline, a poisoned shard,
    /// or finite ticket-sequence exhaustion. `Ok(None)` means this exact binding is not forbidden
    /// or an unexpired recovery already owns it.
    pub fn begin_account_recovery(
        &self,
        endpoint_id: &EndpointId,
        credential_id: &CredentialId,
        recovery_expires_at_ms: i64,
    ) -> Result<Option<RuntimeHealthAccountRecoveryProbe>, RuntimeHealthError> {
        let key = RuntimeHealthKey::endpoint_credential(endpoint_id.clone(), credential_id.clone());
        let now_ms = self.clock.now_ms()?;
        validate_deadline(recovery_expires_at_ms, now_ms)?;
        let mut states = self.write_shard(&key)?;
        let Some(state) = states.get(&key).copied() else {
            return Ok(None);
        };
        let due_for_recovery = match state {
            RuntimeHealthState::AccountForbidden => true,
            RuntimeHealthState::AccountRecoveryInFlight {
                recovery_expires_at_ms: existing_expires_at_ms,
                ..
            } => now_ms >= existing_expires_at_ms,
            RuntimeHealthState::CoolingDown { .. }
            | RuntimeHealthState::CircuitOpen { .. }
            | RuntimeHealthState::CircuitHalfOpen { .. } => false,
        };
        if !due_for_recovery {
            return Ok(None);
        }
        let probe_id = self
            .next_account_recovery_probe_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .map_err(|_| RuntimeHealthError::AccountRecoveryProbeIdOverflow)?;
        states.insert(
            key.clone(),
            RuntimeHealthState::AccountRecoveryInFlight {
                probe_id,
                recovery_expires_at_ms,
            },
        );
        Ok(Some(RuntimeHealthAccountRecoveryProbe {
            key,
            probe_id,
            expires_at_ms: recovery_expires_at_ms,
        }))
    }

    /// Completes one current account-recovery ticket.
    ///
    /// An `Allowed` result removes only the exact binding's forbidden state. A `Forbidden` result
    /// keeps the exact binding blocked. Both paths are local state transitions and never make a
    /// network call or write persistence.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthError::StaleAccountRecoveryProbe`] when the ticket is expired,
    /// superseded, or does not own the current exact binding state.
    pub fn complete_account_recovery(
        &self,
        probe: RuntimeHealthAccountRecoveryProbe,
        result: RuntimeHealthAccountRecoveryResult,
    ) -> Result<(), RuntimeHealthError> {
        let now_ms = self.clock.now_ms()?;
        let mut states = self.write_shard(&probe.key)?;
        let Some(RuntimeHealthState::AccountRecoveryInFlight {
            probe_id,
            recovery_expires_at_ms,
        }) = states.get(&probe.key).copied()
        else {
            return Err(RuntimeHealthError::StaleAccountRecoveryProbe);
        };
        if probe_id != probe.probe_id
            || recovery_expires_at_ms != probe.expires_at_ms
            || now_ms >= recovery_expires_at_ms
        {
            return Err(RuntimeHealthError::StaleAccountRecoveryProbe);
        }
        match result {
            RuntimeHealthAccountRecoveryResult::Allowed => {
                states.remove(&probe.key);
            }
            RuntimeHealthAccountRecoveryResult::Forbidden => {
                states.insert(probe.key, RuntimeHealthState::AccountForbidden);
            }
        }
        Ok(())
    }

    /// Records a short transient Cooldown until a future Unix-millisecond deadline.
    ///
    /// An existing longer Cooldown is never shortened. An existing Circuit remains open, because a
    /// generic transient update must not silently recover it.
    ///
    /// # Errors
    ///
    /// Returns a safe [`RuntimeHealthError`] for an unavailable clock, non-future deadline,
    /// poisoned shard, or bounded-shard capacity failure.
    pub fn cool_down_until(
        &self,
        key: RuntimeHealthKey,
        until_ms: i64,
    ) -> Result<(), RuntimeHealthError> {
        let now_ms = self.clock.now_ms()?;
        validate_deadline(until_ms, now_ms)?;
        let mut states = self.write_shard(&key)?;
        match states.get(&key).copied() {
            Some(
                RuntimeHealthState::CircuitOpen { .. }
                | RuntimeHealthState::CircuitHalfOpen { .. }
                | RuntimeHealthState::AccountForbidden
                | RuntimeHealthState::AccountRecoveryInFlight { .. },
            ) => Ok(()),
            Some(RuntimeHealthState::CoolingDown {
                until_ms: existing_until_ms,
            }) => {
                states.insert(
                    key,
                    RuntimeHealthState::CoolingDown {
                        until_ms: existing_until_ms.max(until_ms),
                    },
                );
                Ok(())
            }
            None => {
                ensure_insert_capacity(&mut states, now_ms)?;
                states.insert(key, RuntimeHealthState::CoolingDown { until_ms });
                Ok(())
            }
        }
    }

    /// Opens a Circuit until a future earliest-recovery instant.
    ///
    /// An existing longer Circuit deadline is never shortened. A Circuit overrides a Cooldown; P4
    /// will later own half-open probes and richer recovery policy.
    ///
    /// # Errors
    ///
    /// Returns the same safe failures as [`Self::cool_down_until`].
    pub fn open_circuit_until(
        &self,
        key: RuntimeHealthKey,
        retry_after_ms: i64,
    ) -> Result<(), RuntimeHealthError> {
        let now_ms = self.clock.now_ms()?;
        validate_deadline(retry_after_ms, now_ms)?;
        let mut states = self.write_shard(&key)?;
        match states.get(&key).copied() {
            Some(
                RuntimeHealthState::CircuitOpen {
                    retry_after_ms: existing_retry_after_ms,
                }
                | RuntimeHealthState::CircuitHalfOpen {
                    retry_after_ms: existing_retry_after_ms,
                    ..
                },
            ) => {
                states.insert(
                    key,
                    RuntimeHealthState::CircuitOpen {
                        retry_after_ms: existing_retry_after_ms.max(retry_after_ms),
                    },
                );
                Ok(())
            }
            Some(RuntimeHealthState::CoolingDown { .. }) => {
                states.insert(key, RuntimeHealthState::CircuitOpen { retry_after_ms });
                Ok(())
            }
            Some(
                RuntimeHealthState::AccountForbidden
                | RuntimeHealthState::AccountRecoveryInFlight { .. },
            ) => Ok(()),
            None => {
                ensure_insert_capacity(&mut states, now_ms)?;
                states.insert(key, RuntimeHealthState::CircuitOpen { retry_after_ms });
                Ok(())
            }
        }
    }

    /// Records explicit successful recovery and removes transient state for this key.
    ///
    /// This is intentionally explicit: an expired Circuit deadline alone does not silently reopen
    /// ordinary traffic before P4's controlled probe/recovery policy exists.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthError::ShardLockPoisoned`] if this key's isolated write shard is
    /// unavailable.
    pub fn mark_healthy(&self, key: &RuntimeHealthKey) -> Result<(), RuntimeHealthError> {
        let mut states = self.write_shard(key)?;
        if matches!(
            states.get(key),
            Some(
                RuntimeHealthState::AccountForbidden
                    | RuntimeHealthState::AccountRecoveryInFlight { .. }
            )
        ) {
            return Ok(());
        }
        states.remove(key);
        Ok(())
    }

    /// Starts at most one bounded half-open recovery probe once an open Circuit reaches its
    /// earliest retry instant.
    ///
    /// Ordinary scheduling remains unavailable while the returned ticket exists. A later caller
    /// can replace an abandoned ticket only at or after `probe_expires_at_ms`; a late ticket then
    /// cannot overwrite the replacement because completion checks its private generation.
    ///
    /// # Errors
    ///
    /// Returns a safe [`RuntimeHealthError`] for an unavailable clock, non-future ticket deadline,
    /// poisoned shard, or finite ticket-generation exhaustion. `Ok(None)` means the key has no
    /// due Circuit or another unexpired half-open probe already owns recovery.
    pub fn begin_circuit_probe(
        &self,
        key: &RuntimeHealthKey,
        probe_expires_at_ms: i64,
    ) -> Result<Option<RuntimeHealthCircuitProbe>, RuntimeHealthError> {
        let now_ms = self.clock.now_ms()?;
        validate_deadline(probe_expires_at_ms, now_ms)?;
        let mut states = self.write_shard(key)?;
        let Some(state) = states.get(key).copied() else {
            return Ok(None);
        };
        let due_for_probe = match state {
            RuntimeHealthState::CircuitOpen { retry_after_ms } => now_ms >= retry_after_ms,
            RuntimeHealthState::CircuitHalfOpen {
                probe_expires_at_ms,
                ..
            } => now_ms >= probe_expires_at_ms,
            RuntimeHealthState::CoolingDown { .. }
            | RuntimeHealthState::AccountForbidden
            | RuntimeHealthState::AccountRecoveryInFlight { .. } => false,
        };
        if !due_for_probe {
            return Ok(None);
        }
        let probe_id = self
            .next_circuit_probe_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .map_err(|_| RuntimeHealthError::CircuitProbeIdOverflow)?;
        states.insert(
            key.clone(),
            RuntimeHealthState::CircuitHalfOpen {
                retry_after_ms: state.retry_after_ms(),
                probe_id,
                probe_expires_at_ms,
            },
        );
        Ok(Some(RuntimeHealthCircuitProbe {
            key: key.clone(),
            probe_id,
            expires_at_ms: probe_expires_at_ms,
        }))
    }

    /// Finishes the current half-open probe and either closes or reopens its Circuit.
    ///
    /// The ticket is consumed, so a caller cannot accidentally reuse a successful result. An
    /// unexpired matching ticket is the only outcome allowed to change the Circuit. A stale or
    /// timed-out ticket fails closed and leaves the current state untouched.
    ///
    /// # Errors
    ///
    /// Returns a safe [`RuntimeHealthError`] for unavailable time, a stale ticket, a non-future
    /// retry deadline, or a poisoned shard.
    pub fn complete_circuit_probe(
        &self,
        probe: RuntimeHealthCircuitProbe,
        result: RuntimeHealthCircuitProbeResult,
    ) -> Result<(), RuntimeHealthError> {
        let now_ms = self.clock.now_ms()?;
        let mut states = self.write_shard(&probe.key)?;
        let Some(RuntimeHealthState::CircuitHalfOpen {
            probe_id,
            probe_expires_at_ms,
            ..
        }) = states.get(&probe.key).copied()
        else {
            return Err(RuntimeHealthError::StaleCircuitProbe);
        };
        if probe_id != probe.probe_id
            || probe_expires_at_ms != probe.expires_at_ms
            || now_ms >= probe_expires_at_ms
        {
            return Err(RuntimeHealthError::StaleCircuitProbe);
        }
        match result {
            RuntimeHealthCircuitProbeResult::Healthy => {
                states.remove(&probe.key);
            }
            RuntimeHealthCircuitProbeResult::Unhealthy { retry_after_ms } => {
                validate_deadline(retry_after_ms, now_ms)?;
                states.insert(
                    probe.key,
                    RuntimeHealthState::CircuitOpen { retry_after_ms },
                );
            }
        }
        Ok(())
    }

    /// Counts retained state entries across all shards for bounded diagnostics and tests.
    ///
    /// This management/testing helper is not used by request-time selection and intentionally reads
    /// every shard.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHealthError::ShardLockPoisoned`] when any shard cannot be read.
    pub fn entry_count(&self) -> Result<usize, RuntimeHealthError> {
        let mut count = 0_usize;
        for shard in &self.shards {
            let states = shard
                .read()
                .map_err(|_| RuntimeHealthError::ShardLockPoisoned)?;
            count = count.saturating_add(states.len());
        }
        Ok(count)
    }

    fn read_shard(
        &self,
        key: &RuntimeHealthKey,
    ) -> Result<
        std::sync::RwLockReadGuard<'_, BTreeMap<RuntimeHealthKey, RuntimeHealthState>>,
        RuntimeHealthError,
    > {
        self.shards[self.shard_index(key)]
            .read()
            .map_err(|_| RuntimeHealthError::ShardLockPoisoned)
    }

    fn write_shard(
        &self,
        key: &RuntimeHealthKey,
    ) -> Result<
        std::sync::RwLockWriteGuard<'_, BTreeMap<RuntimeHealthKey, RuntimeHealthState>>,
        RuntimeHealthError,
    > {
        self.shards[self.shard_index(key)]
            .write()
            .map_err(|_| RuntimeHealthError::ShardLockPoisoned)
    }

    fn shard_index(&self, key: &RuntimeHealthKey) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let Ok(mask) = u64::try_from(self.shards.len() - 1) else {
            return 0;
        };
        usize::try_from(hasher.finish() & mask).unwrap_or_default()
    }
}

impl Default for RuntimeHealthRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RuntimeHealthRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeHealthRegistry")
            .field("shard_count", &self.shard_count())
            .field(
                "entries_per_shard_limit",
                &MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD,
            )
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeHealthState {
    CoolingDown {
        until_ms: i64,
    },
    CircuitOpen {
        retry_after_ms: i64,
    },
    CircuitHalfOpen {
        retry_after_ms: i64,
        probe_id: u64,
        probe_expires_at_ms: i64,
    },
    AccountForbidden,
    AccountRecoveryInFlight {
        probe_id: u64,
        recovery_expires_at_ms: i64,
    },
}

impl RuntimeHealthState {
    const fn availability_at(self, now_ms: i64) -> RuntimeHealthAvailability {
        match self {
            Self::CoolingDown { until_ms } if now_ms < until_ms => {
                RuntimeHealthAvailability::CoolingDown { until_ms }
            }
            Self::CoolingDown { .. } => RuntimeHealthAvailability::Available,
            Self::CircuitOpen { retry_after_ms } | Self::CircuitHalfOpen { retry_after_ms, .. } => {
                RuntimeHealthAvailability::CircuitOpen { retry_after_ms }
            }
            Self::AccountRecoveryInFlight {
                recovery_expires_at_ms,
                ..
            } if now_ms < recovery_expires_at_ms => {
                RuntimeHealthAvailability::AccountRecoveryInFlight {
                    expires_at_ms: recovery_expires_at_ms,
                }
            }
            Self::AccountForbidden | Self::AccountRecoveryInFlight { .. } => {
                RuntimeHealthAvailability::AccountForbidden
            }
        }
    }

    const fn account_status_at(self, now_ms: i64) -> RuntimeCredentialAccountStatus {
        match self {
            Self::AccountRecoveryInFlight {
                recovery_expires_at_ms,
                ..
            } if now_ms < recovery_expires_at_ms => {
                RuntimeCredentialAccountStatus::RecoveryInFlight {
                    expires_at_ms: recovery_expires_at_ms,
                }
            }
            Self::AccountForbidden | Self::AccountRecoveryInFlight { .. } => {
                RuntimeCredentialAccountStatus::Forbidden
            }
            Self::CoolingDown { .. } | Self::CircuitOpen { .. } | Self::CircuitHalfOpen { .. } => {
                RuntimeCredentialAccountStatus::Available
            }
        }
    }

    const fn is_expired_cooldown(self, now_ms: i64) -> bool {
        matches!(self, Self::CoolingDown { until_ms } if until_ms <= now_ms)
    }

    const fn retry_after_ms(self) -> i64 {
        match self {
            Self::CircuitOpen { retry_after_ms } | Self::CircuitHalfOpen { retry_after_ms, .. } => {
                retry_after_ms
            }
            Self::CoolingDown { until_ms } => until_ms,
            Self::AccountForbidden => 0,
            Self::AccountRecoveryInFlight {
                recovery_expires_at_ms,
                ..
            } => recovery_expires_at_ms,
        }
    }
}

fn build_shards(
    shard_count: usize,
) -> Box<[RwLock<BTreeMap<RuntimeHealthKey, RuntimeHealthState>>]> {
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

fn validate_deadline(until_ms: i64, now_ms: i64) -> Result<(), RuntimeHealthError> {
    if until_ms <= now_ms {
        return Err(RuntimeHealthError::DeadlineNotInFuture);
    }
    Ok(())
}

fn ensure_insert_capacity(
    states: &mut BTreeMap<RuntimeHealthKey, RuntimeHealthState>,
    now_ms: i64,
) -> Result<(), RuntimeHealthError> {
    if states.len() < MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD {
        return Ok(());
    }
    states.retain(|_, state| !state.is_expired_cooldown(now_ms));
    if states.len() >= MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD {
        return Err(RuntimeHealthError::ShardCapacityExceeded);
    }
    Ok(())
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
        MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD, RuntimeCredentialAccountStatus,
        RuntimeHealthAccountRecoveryResult, RuntimeHealthAvailability,
        RuntimeHealthCircuitProbeResult, RuntimeHealthClock, RuntimeHealthClockError,
        RuntimeHealthError, RuntimeHealthKey, RuntimeHealthRegistry,
        RuntimeHealthRegistryBuildError,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn cooldown_is_isolated_and_automatically_expires() -> TestResult {
        let (clock, registry) = registry(100);
        let endpoint_a = endpoint("endpoint-a")?;
        let endpoint_b = endpoint("endpoint-b")?;
        let key_a = RuntimeHealthKey::endpoint(endpoint_a.clone());

        registry.cool_down_until(key_a.clone(), 200)?;
        assert_eq!(
            registry.availability(&key_a)?,
            RuntimeHealthAvailability::CoolingDown { until_ms: 200 }
        );
        assert!(!registry.endpoint_is_available(&endpoint_a));
        assert!(registry.endpoint_is_available(&endpoint_b));

        clock.set_now_ms(200);
        assert_eq!(
            registry.availability(&key_a)?,
            RuntimeHealthAvailability::Available
        );
        assert!(registry.endpoint_is_available(&endpoint_a));
        Ok(())
    }

    #[test]
    fn circuit_is_endpoint_credential_scoped_and_requires_explicit_recovery() -> TestResult {
        let (clock, registry) = registry(100);
        let endpoint_a = endpoint("endpoint-a")?;
        let endpoint_b = endpoint("endpoint-b")?;
        let credential_a = credential("credential-a")?;
        let credential_b = credential("credential-b")?;
        let circuit_key =
            RuntimeHealthKey::endpoint_credential(endpoint_a.clone(), credential_a.clone());

        registry.open_circuit_until(circuit_key.clone(), 200)?;
        assert_eq!(
            registry.availability(&circuit_key)?,
            RuntimeHealthAvailability::CircuitOpen {
                retry_after_ms: 200
            }
        );
        assert!(!registry.endpoint_credential_is_available(&endpoint_a, &credential_a));
        assert!(registry.endpoint_credential_is_available(&endpoint_a, &credential_b));
        assert!(registry.endpoint_credential_is_available(&endpoint_b, &credential_a));
        assert!(registry.endpoint_is_available(&endpoint_a));

        clock.set_now_ms(300);
        assert!(!registry.endpoint_credential_is_available(&endpoint_a, &credential_a));
        registry.mark_healthy(&circuit_key)?;
        assert!(registry.endpoint_credential_is_available(&endpoint_a, &credential_a));
        Ok(())
    }

    #[test]
    fn model_circuit_isolated_and_half_open_recovery_is_single_ticket() -> TestResult {
        let (clock, registry) = registry(100);
        let endpoint = endpoint("endpoint-a")?;
        let credential = credential("credential-a")?;
        let model_a = RuntimeHealthKey::endpoint_credential_model(
            endpoint.clone(),
            credential.clone(),
            "model-a",
        );
        registry.open_circuit_until(model_a.clone(), 200)?;
        assert!(!registry.endpoint_credential_model_is_available(
            &endpoint,
            &credential,
            "model-a"
        ));
        assert!(registry.endpoint_credential_model_is_available(&endpoint, &credential, "model-b"));
        assert_eq!(registry.begin_circuit_probe(&model_a, 250)?, None);

        clock.set_now_ms(200);
        let ticket = registry
            .begin_circuit_probe(&model_a, 250)?
            .ok_or("due Circuit did not create a recovery ticket")?;
        assert_eq!(ticket.key(), &model_a);
        assert_eq!(ticket.expires_at_ms(), 250);
        assert_eq!(registry.begin_circuit_probe(&model_a, 250)?, None);
        assert_eq!(
            registry.availability(&model_a)?,
            RuntimeHealthAvailability::CircuitOpen {
                retry_after_ms: 200
            }
        );

        clock.set_now_ms(201);
        registry.complete_circuit_probe(ticket, RuntimeHealthCircuitProbeResult::Healthy)?;
        assert!(registry.endpoint_credential_model_is_available(&endpoint, &credential, "model-a"));
        assert!(registry.endpoint_credential_model_is_available(&endpoint, &credential, "model-b"));
        Ok(())
    }

    #[test]
    fn forbidden_account_is_binding_scoped_and_needs_its_own_recovery_ticket() -> TestResult {
        let (clock, registry) = registry(100);
        let endpoint_a = endpoint("endpoint-a")?;
        let endpoint_b = endpoint("endpoint-b")?;
        let credential_a = credential("credential-a")?;
        let credential_b = credential("credential-b")?;
        let binding_key =
            RuntimeHealthKey::endpoint_credential(endpoint_a.clone(), credential_a.clone());

        registry.mark_credential_forbidden(endpoint_a.clone(), credential_a.clone())?;
        assert_eq!(
            registry.credential_account_status_at(&endpoint_a, &credential_a, 100)?,
            RuntimeCredentialAccountStatus::Forbidden
        );
        assert_eq!(
            registry.availability(&binding_key)?,
            RuntimeHealthAvailability::AccountForbidden
        );
        assert!(!registry.endpoint_credential_is_available(&endpoint_a, &credential_a));
        assert!(registry.endpoint_credential_is_available(&endpoint_a, &credential_b));
        assert!(registry.endpoint_credential_is_available(&endpoint_b, &credential_a));

        registry.cool_down_until(binding_key.clone(), 200)?;
        registry.open_circuit_until(binding_key.clone(), 200)?;
        registry.mark_healthy(&binding_key)?;
        assert_eq!(
            registry.credential_account_status_at(&endpoint_a, &credential_a, 100)?,
            RuntimeCredentialAccountStatus::Forbidden
        );

        let ticket = registry
            .begin_account_recovery(&endpoint_a, &credential_a, 150)?
            .ok_or("forbidden account did not create a recovery ticket")?;
        assert_eq!(ticket.key(), &binding_key);
        assert_eq!(
            registry.credential_account_status_at(&endpoint_a, &credential_a, 100)?,
            RuntimeCredentialAccountStatus::RecoveryInFlight { expires_at_ms: 150 }
        );
        assert_eq!(
            registry.begin_account_recovery(&endpoint_a, &credential_a, 150)?,
            None
        );

        clock.set_now_ms(101);
        registry.complete_account_recovery(ticket, RuntimeHealthAccountRecoveryResult::Allowed)?;
        assert_eq!(
            registry.credential_account_status_at(&endpoint_a, &credential_a, 101)?,
            RuntimeCredentialAccountStatus::Available
        );
        assert!(registry.endpoint_credential_is_available(&endpoint_a, &credential_a));
        Ok(())
    }

    #[test]
    fn rejected_or_stale_account_recovery_ticket_cannot_reopen_a_forbidden_binding() -> TestResult {
        let (clock, registry) = registry(100);
        let endpoint = endpoint("endpoint-a")?;
        let credential = credential("credential-a")?;

        registry.mark_credential_forbidden(endpoint.clone(), credential.clone())?;
        let rejected_ticket = registry
            .begin_account_recovery(&endpoint, &credential, 110)?
            .ok_or("forbidden account did not create a recovery ticket")?;
        registry.complete_account_recovery(
            rejected_ticket,
            RuntimeHealthAccountRecoveryResult::Forbidden,
        )?;
        assert_eq!(
            registry.credential_account_status_at(&endpoint, &credential, 100)?,
            RuntimeCredentialAccountStatus::Forbidden
        );

        let expired_ticket = registry
            .begin_account_recovery(&endpoint, &credential, 120)?
            .ok_or("forbidden account did not replace its recovery ticket")?;
        clock.set_now_ms(120);
        assert_eq!(
            registry.complete_account_recovery(
                expired_ticket,
                RuntimeHealthAccountRecoveryResult::Allowed
            ),
            Err(RuntimeHealthError::StaleAccountRecoveryProbe)
        );
        assert_eq!(
            registry.credential_account_status_at(&endpoint, &credential, 120)?,
            RuntimeCredentialAccountStatus::Forbidden
        );

        let current_ticket = registry
            .begin_account_recovery(&endpoint, &credential, 130)?
            .ok_or("expired recovery ticket was not replaced")?;
        clock.set_now_ms(121);
        registry.complete_account_recovery(
            current_ticket,
            RuntimeHealthAccountRecoveryResult::Allowed,
        )?;
        assert!(registry.endpoint_credential_is_available(&endpoint, &credential));
        Ok(())
    }

    #[test]
    fn expired_half_open_ticket_cannot_overwrite_a_reopened_circuit() -> TestResult {
        let (clock, registry) = registry(100);
        let key = RuntimeHealthKey::endpoint(endpoint("endpoint-a")?);
        registry.open_circuit_until(key.clone(), 200)?;
        clock.set_now_ms(200);
        let expired_ticket = registry
            .begin_circuit_probe(&key, 250)?
            .ok_or("due Circuit did not create a recovery ticket")?;

        clock.set_now_ms(250);
        assert_eq!(
            registry
                .complete_circuit_probe(expired_ticket, RuntimeHealthCircuitProbeResult::Healthy),
            Err(RuntimeHealthError::StaleCircuitProbe)
        );
        let active_ticket = registry
            .begin_circuit_probe(&key, 300)?
            .ok_or("expired recovery ticket was not reclaimed")?;
        clock.set_now_ms(251);
        registry.complete_circuit_probe(
            active_ticket,
            RuntimeHealthCircuitProbeResult::Unhealthy {
                retry_after_ms: 400,
            },
        )?;
        assert_eq!(
            registry.availability(&key)?,
            RuntimeHealthAvailability::CircuitOpen {
                retry_after_ms: 400
            }
        );
        Ok(())
    }

    #[test]
    fn longer_transient_state_never_shortens_and_circuit_outranks_cooldown() -> TestResult {
        let (_clock, registry) = registry(100);
        let key = RuntimeHealthKey::endpoint(endpoint("endpoint-a")?);

        registry.cool_down_until(key.clone(), 300)?;
        registry.cool_down_until(key.clone(), 200)?;
        assert_eq!(
            registry.availability(&key)?,
            RuntimeHealthAvailability::CoolingDown { until_ms: 300 }
        );

        registry.open_circuit_until(key.clone(), 400)?;
        registry.cool_down_until(key.clone(), 500)?;
        assert_eq!(
            registry.availability(&key)?,
            RuntimeHealthAvailability::CircuitOpen {
                retry_after_ms: 400
            }
        );
        registry.open_circuit_until(key.clone(), 350)?;
        assert_eq!(
            registry.availability(&key)?,
            RuntimeHealthAvailability::CircuitOpen {
                retry_after_ms: 400
            }
        );
        Ok(())
    }

    #[test]
    fn unsafe_shard_counts_and_deadlines_fail_closed() -> TestResult {
        let clock: Arc<dyn RuntimeHealthClock> = Arc::new(FixedRuntimeHealthClock::new(100));
        assert!(matches!(
            RuntimeHealthRegistry::try_with_clock_and_shards(Arc::clone(&clock), 0),
            Err(RuntimeHealthRegistryBuildError::ZeroShardCount)
        ));
        assert!(matches!(
            RuntimeHealthRegistry::try_with_clock_and_shards(Arc::clone(&clock), 3),
            Err(RuntimeHealthRegistryBuildError::NonPowerOfTwoShardCount)
        ));

        let registry = RuntimeHealthRegistry::try_with_clock_and_shards(clock, 1)?;
        let result =
            registry.cool_down_until(RuntimeHealthKey::endpoint(endpoint("endpoint-a")?), 100);
        assert_eq!(result, Err(RuntimeHealthError::DeadlineNotInFuture));
        assert_eq!(registry.entry_count()?, 0);
        Ok(())
    }

    #[test]
    fn a_full_shard_evicts_expired_cooldowns_but_never_evicts_live_state() -> TestResult {
        let (clock, registry) = registry_with_shards(100, 1)?;
        for index in 0..MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD {
            registry.cool_down_until(
                RuntimeHealthKey::endpoint(endpoint(&format!("endpoint-{index}"))?),
                200,
            )?;
        }
        assert_eq!(
            registry.cool_down_until(
                RuntimeHealthKey::endpoint(endpoint("endpoint-overflow")?),
                200,
            ),
            Err(RuntimeHealthError::ShardCapacityExceeded)
        );

        clock.set_now_ms(200);
        registry.cool_down_until(
            RuntimeHealthKey::endpoint(endpoint("endpoint-reclaimed")?),
            300,
        )?;
        assert_eq!(registry.entry_count()?, 1);
        Ok(())
    }

    #[test]
    fn system_clock_contract_is_safe_for_normal_runtime() -> TestResult {
        let registry = RuntimeHealthRegistry::new();
        let availability =
            registry.availability(&RuntimeHealthKey::endpoint(endpoint("endpoint-a")?));
        assert!(matches!(
            availability,
            Ok(RuntimeHealthAvailability::Available)
        ));
        Ok(())
    }

    fn registry(now_ms: i64) -> (Arc<FixedRuntimeHealthClock>, RuntimeHealthRegistry) {
        let clock = Arc::new(FixedRuntimeHealthClock::new(now_ms));
        let registry = RuntimeHealthRegistry::with_clock(clock.clone());
        (clock, registry)
    }

    fn registry_with_shards(
        now_ms: i64,
        shard_count: usize,
    ) -> Result<(Arc<FixedRuntimeHealthClock>, RuntimeHealthRegistry), Box<dyn Error>> {
        let clock = Arc::new(FixedRuntimeHealthClock::new(now_ms));
        let registry =
            RuntimeHealthRegistry::try_with_clock_and_shards(clock.clone(), shard_count)?;
        Ok((clock, registry))
    }

    fn endpoint(value: &str) -> Result<EndpointId, Box<dyn Error>> {
        Ok(EndpointId::try_new(value)?)
    }

    fn credential(value: &str) -> Result<CredentialId, Box<dyn Error>> {
        Ok(CredentialId::try_new(value)?)
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
