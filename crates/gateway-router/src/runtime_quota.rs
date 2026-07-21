//! Bounded, target-local runtime quota snapshots and controlled reset recovery.
//!
//! This module receives only sanitized quota observations. It does not issue HTTP requests,
//! parse Headers, read Secrets, persist data, or admit ordinary traffic after a reset instant
//! without one explicit recovery ticket.

use std::{
    collections::{BTreeMap, BTreeSet, hash_map::DefaultHasher},
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    sync::{
        Arc, RwLock, RwLockReadGuard, RwLockWriteGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use gateway_core::{CredentialId, EndpointId};

use crate::{RuntimeHealthClock, RuntimeHealthClockError, SystemRuntimeHealthClock};

/// Fixed default number of independently locked runtime-quota shards.
pub const DEFAULT_RUNTIME_QUOTA_SHARD_COUNT: usize = 64;
/// Largest accepted shard count, preventing construction-time allocation amplification.
pub const MAX_RUNTIME_QUOTA_SHARD_COUNT: usize = 1_024;
/// Largest retained quota targets in any one shard.
pub const MAX_RUNTIME_QUOTA_ENTRIES_PER_SHARD: usize = 1_024;
/// Largest number of independently described windows in one sanitized snapshot.
pub const MAX_QUOTA_WINDOWS_PER_SNAPSHOT: usize = 8;
/// Largest accepted structural window label length.
pub const MAX_QUOTA_WINDOW_LABEL_BYTES: usize = 64;

/// Exact non-secret target whose quota can affect a runtime binding.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeQuotaTarget {
    /// A quota shared by every model through one Endpoint/Credential binding.
    EndpointCredential {
        /// The protocol-specific Endpoint that owns this quota observation.
        endpoint_id: EndpointId,
        /// The non-secret Credential identity at that Endpoint.
        credential_id: CredentialId,
    },
    /// A quota scoped to one exact upstream model through one Endpoint/Credential binding.
    EndpointCredentialModel {
        /// The protocol-specific Endpoint that owns this quota observation.
        endpoint_id: EndpointId,
        /// The non-secret Credential identity at that Endpoint.
        credential_id: CredentialId,
        /// The exact compiler-approved upstream model label.
        upstream_model: String,
    },
}

impl RuntimeQuotaTarget {
    /// Creates a binding-wide quota target.
    #[must_use]
    pub fn endpoint_credential(endpoint_id: EndpointId, credential_id: CredentialId) -> Self {
        Self::EndpointCredential {
            endpoint_id,
            credential_id,
        }
    }

    /// Creates one model-scoped quota target without accepting an empty model label.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeQuotaTargetError::EmptyUpstreamModel`] when the model label is empty.
    pub fn endpoint_credential_model(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        upstream_model: impl Into<String>,
    ) -> Result<Self, RuntimeQuotaTargetError> {
        let upstream_model = upstream_model.into();
        if upstream_model.is_empty() {
            return Err(RuntimeQuotaTargetError::EmptyUpstreamModel);
        }
        Ok(Self::EndpointCredentialModel {
            endpoint_id,
            credential_id,
            upstream_model,
        })
    }

    /// Returns the Endpoint owning this target.
    #[must_use]
    pub fn endpoint_id(&self) -> &EndpointId {
        match self {
            Self::EndpointCredential { endpoint_id, .. }
            | Self::EndpointCredentialModel { endpoint_id, .. } => endpoint_id,
        }
    }

    /// Returns the Credential owning this target.
    #[must_use]
    pub fn credential_id(&self) -> &CredentialId {
        match self {
            Self::EndpointCredential { credential_id, .. }
            | Self::EndpointCredentialModel { credential_id, .. } => credential_id,
        }
    }

    /// Returns the exact model only for model-scoped quota observations.
    #[must_use]
    pub fn upstream_model(&self) -> Option<&str> {
        match self {
            Self::EndpointCredential { .. } => None,
            Self::EndpointCredentialModel { upstream_model, .. } => Some(upstream_model),
        }
    }
}

/// Construction failure for [`RuntimeQuotaTarget`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeQuotaTargetError {
    /// A model-scoped target requires an exact non-empty model label.
    EmptyUpstreamModel,
}

impl fmt::Display for RuntimeQuotaTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyUpstreamModel => formatter.write_str("runtime quota model label is empty"),
        }
    }
}

impl Error for RuntimeQuotaTargetError {}

/// Origin of one sanitized quota observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaSource {
    /// A bounded allow-listed upstream response header supplied the observation.
    Header,
    /// A provider billing or account endpoint supplied the observation.
    Billing,
    /// A provider REST status endpoint supplied the observation.
    Rest,
    /// A provider gRPC status endpoint supplied the observation.
    Grpc,
    /// The gateway inferred a bounded temporary window from a 429 without reset metadata.
    Estimated,
}

/// Confidence assigned to one quota snapshot without claiming unsupported precision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaConfidence {
    /// The source is explicitly authoritative for the reported quota window.
    Authoritative,
    /// The source reported a direct but not independently authoritative observation.
    Observed,
    /// The gateway used only a bounded fallback estimate.
    Estimated,
}

/// One bounded structural quota window without Header, body, or free-form diagnostic retention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaWindow {
    label: String,
    limit: Option<u64>,
    remaining: Option<u64>,
    reset_at_ms: Option<i64>,
}

impl QuotaWindow {
    /// Validates one structural quota window.
    ///
    /// # Errors
    ///
    /// Returns [`QuotaWindowError`] for an empty/oversized label or an impossible remaining count.
    pub fn try_new(
        label: impl Into<String>,
        limit: Option<u64>,
        remaining: Option<u64>,
        reset_at_ms: Option<i64>,
    ) -> Result<Self, QuotaWindowError> {
        let label = label.into();
        if label.is_empty() {
            return Err(QuotaWindowError::EmptyLabel);
        }
        if label.len() > MAX_QUOTA_WINDOW_LABEL_BYTES {
            return Err(QuotaWindowError::LabelTooLong);
        }
        if let (Some(limit), Some(remaining)) = (limit, remaining)
            && remaining > limit
        {
            return Err(QuotaWindowError::RemainingExceedsLimit);
        }
        Ok(Self {
            label,
            limit,
            remaining,
            reset_at_ms,
        })
    }

    /// Returns the stable structural window label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the reported total limit when the source exposed it.
    #[must_use]
    pub const fn limit(&self) -> Option<u64> {
        self.limit
    }

    /// Returns the reported remaining capacity when the source exposed it.
    #[must_use]
    pub const fn remaining(&self) -> Option<u64> {
        self.remaining
    }

    /// Returns the source's explicit reset instant when available.
    #[must_use]
    pub const fn reset_at_ms(&self) -> Option<i64> {
        self.reset_at_ms
    }

    const fn is_exhausted(&self) -> bool {
        matches!(self.remaining, Some(0))
    }
}

/// Validation error for [`QuotaWindow`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaWindowError {
    /// Window labels must be non-empty structural identifiers.
    EmptyLabel,
    /// Window labels exceed the bounded structural metadata limit.
    LabelTooLong,
    /// A remaining count cannot exceed a reported limit.
    RemainingExceedsLimit,
}

impl fmt::Display for QuotaWindowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyLabel => "quota window label is empty",
            Self::LabelTooLong => "quota window label exceeds the finite maximum",
            Self::RemainingExceedsLimit => "quota window remaining exceeds its limit",
        };
        formatter.write_str(message)
    }
}

impl Error for QuotaWindowError {}

/// One immutable, sanitized quota snapshot for an exact target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaSnapshot {
    target: RuntimeQuotaTarget,
    windows: Vec<QuotaWindow>,
    source: QuotaSource,
    confidence: QuotaConfidence,
    observed_at_ms: i64,
}

impl QuotaSnapshot {
    /// Validates an immutable quota snapshot supplied by a non-request-path classifier.
    ///
    /// Exhausted windows require a reset instant strictly after `observed_at_ms`; an observation
    /// without such evidence must use the registry's explicit 429 fallback instead of creating an
    /// unbounded or permanent block.
    ///
    /// # Errors
    ///
    /// Returns [`QuotaSnapshotError`] for duplicate/oversized windows or invalid reset evidence.
    pub fn try_new(
        target: RuntimeQuotaTarget,
        windows: Vec<QuotaWindow>,
        source: QuotaSource,
        confidence: QuotaConfidence,
        observed_at_ms: i64,
    ) -> Result<Self, QuotaSnapshotError> {
        if windows.len() > MAX_QUOTA_WINDOWS_PER_SNAPSHOT {
            return Err(QuotaSnapshotError::TooManyWindows);
        }
        let mut labels = BTreeSet::new();
        for window in &windows {
            if !labels.insert(window.label().to_owned()) {
                return Err(QuotaSnapshotError::DuplicateWindowLabel);
            }
            if window.is_exhausted()
                && window
                    .reset_at_ms()
                    .is_none_or(|reset_at_ms| reset_at_ms <= observed_at_ms)
            {
                return Err(QuotaSnapshotError::ExhaustedWindowWithoutFutureReset);
            }
        }
        if (source == QuotaSource::Estimated) != (confidence == QuotaConfidence::Estimated) {
            return Err(QuotaSnapshotError::EstimatedSourceConfidenceMismatch);
        }
        Ok(Self {
            target,
            windows,
            source,
            confidence,
            observed_at_ms,
        })
    }

    /// Returns the exact quota target.
    #[must_use]
    pub fn target(&self) -> &RuntimeQuotaTarget {
        &self.target
    }

    /// Returns the bounded source windows.
    #[must_use]
    pub fn windows(&self) -> &[QuotaWindow] {
        &self.windows
    }

    /// Returns the distinct evidence source.
    #[must_use]
    pub const fn source(&self) -> QuotaSource {
        self.source
    }

    /// Returns the explicitly retained confidence level.
    #[must_use]
    pub const fn confidence(&self) -> QuotaConfidence {
        self.confidence
    }

    /// Returns the classifier-supplied observation time.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Returns the latest reset instant that must pass before a controlled recovery can begin.
    #[must_use]
    pub fn blocking_reset_at_ms(&self) -> Option<i64> {
        self.windows
            .iter()
            .filter(|window| window.is_exhausted())
            .filter_map(QuotaWindow::reset_at_ms)
            .max()
    }
}

/// Validation error for [`QuotaSnapshot`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaSnapshotError {
    /// A snapshot exceeded its finite number of windows.
    TooManyWindows,
    /// Two windows used the same structural label.
    DuplicateWindowLabel,
    /// An exhausted window did not establish a strictly future reset instant.
    ExhaustedWindowWithoutFutureReset,
    /// Estimated source and confidence must appear together; neither may overstate the other.
    EstimatedSourceConfidenceMismatch,
}

impl fmt::Display for QuotaSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::TooManyWindows => "quota snapshot exceeds its finite window limit",
            Self::DuplicateWindowLabel => "quota snapshot contains duplicate window labels",
            Self::ExhaustedWindowWithoutFutureReset => {
                "exhausted quota window lacks a future reset instant"
            }
            Self::EstimatedSourceConfidenceMismatch => {
                "estimated quota source and confidence must agree"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for QuotaSnapshotError {}

/// Effective request-time quota state for one exact target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeQuotaAvailability {
    /// No retained snapshot blocks ordinary scheduling.
    Available,
    /// A known exhausted window has not reached its reset instant.
    Exhausted {
        /// Latest reset instant across every currently exhausted quota window.
        reset_at_ms: i64,
    },
    /// Reset evidence has arrived, but ordinary traffic still awaits one controlled recovery probe.
    RecoveryRequired {
        /// Reset instant that made the target eligible for a controlled probe.
        reset_at_ms: i64,
    },
    /// Exactly one controlled recovery probe is outstanding; ordinary traffic remains blocked.
    RecoveryProbeInFlight {
        /// Reset instant for the still-blocked target.
        reset_at_ms: i64,
        /// Exclusive probe-ticket deadline.
        expires_at_ms: i64,
    },
}

impl RuntimeQuotaAvailability {
    /// Returns whether ordinary request scheduling may use the target now.
    #[must_use]
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// One non-cloneable controlled recovery ticket for an exhausted quota target.
#[derive(Debug, Eq, PartialEq)]
pub struct RuntimeQuotaRecoveryProbe {
    target: RuntimeQuotaTarget,
    probe_id: u64,
    expires_at_ms: i64,
}

impl RuntimeQuotaRecoveryProbe {
    /// Returns the exact non-secret target being recovered.
    #[must_use]
    pub fn target(&self) -> &RuntimeQuotaTarget {
        &self.target
    }

    /// Returns the ticket's exclusive deadline.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
}

/// Safe construction failures for [`RuntimeQuotaRegistry`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeQuotaRegistryBuildError {
    /// A registry requires at least one independently locked shard.
    ZeroShardCount,
    /// A non-power-of-two count would make fixed hash partitioning ambiguous.
    NonPowerOfTwoShardCount,
    /// The requested count would exceed the finite runtime allocation limit.
    TooManyShards,
}

impl fmt::Display for RuntimeQuotaRegistryBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ZeroShardCount => "runtime quota shard count must be positive",
            Self::NonPowerOfTwoShardCount => "runtime quota shard count must be a power of two",
            Self::TooManyShards => "runtime quota shard count exceeds its finite limit",
        };
        formatter.write_str(message)
    }
}

impl Error for RuntimeQuotaRegistryBuildError {}

/// Safe mutation, lookup, or recovery failure from [`RuntimeQuotaRegistry`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeQuotaError {
    /// The shared runtime clock was unavailable.
    Clock(RuntimeHealthClockError),
    /// A single isolated quota shard was poisoned by a prior panic.
    ShardLockPoisoned,
    /// A new target could not fit within one finite shard.
    ShardCapacityExceeded,
    /// A supplied snapshot would move one target's observation time backwards.
    ObservationTimeRegressed,
    /// A recovery ticket deadline was not strictly in the future.
    RecoveryProbeDeadlineNotInFuture,
    /// The finite non-zero recovery-ticket sequence cannot advance safely.
    RecoveryProbeIdOverflow,
    /// A ticket is expired, superseded, or no longer matches the exact target state.
    StaleRecoveryProbe,
    /// A recovery snapshot did not describe the ticket's exact target.
    RecoverySnapshotTargetMismatch,
    /// A bounded fallback duration could not be represented in the runtime timestamp domain.
    FallbackDurationOverflow,
    /// A missing retry-after requires a strictly positive bounded fallback duration.
    FallbackDurationNotPositive,
}

impl fmt::Display for RuntimeQuotaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Clock(_) => "runtime quota clock is unavailable",
            Self::ShardLockPoisoned => "runtime quota shard is unavailable",
            Self::ShardCapacityExceeded => "runtime quota shard is at capacity",
            Self::ObservationTimeRegressed => "runtime quota observation time regressed",
            Self::RecoveryProbeDeadlineNotInFuture => {
                "runtime quota recovery probe deadline is not in the future"
            }
            Self::RecoveryProbeIdOverflow => {
                "runtime quota recovery probe identifier cannot advance safely"
            }
            Self::StaleRecoveryProbe => "runtime quota recovery probe is stale",
            Self::RecoverySnapshotTargetMismatch => {
                "runtime quota recovery snapshot target does not match its ticket"
            }
            Self::FallbackDurationOverflow => {
                "runtime quota fallback duration cannot be represented safely"
            }
            Self::FallbackDurationNotPositive => {
                "runtime quota fallback duration must be strictly positive"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for RuntimeQuotaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RuntimeHealthClockError> for RuntimeQuotaError {
    fn from(error: RuntimeHealthClockError) -> Self {
        Self::Clock(error)
    }
}

#[derive(Clone, Debug)]
struct RuntimeQuotaState {
    snapshot: QuotaSnapshot,
    recovery_probe: Option<RuntimeQuotaRecoveryProbeState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeQuotaRecoveryProbeState {
    probe_id: u64,
    expires_at_ms: i64,
}

/// Bounded, sharded process-local quota state outside the request persistence path.
pub struct RuntimeQuotaRegistry {
    clock: Arc<dyn RuntimeHealthClock>,
    shards: Box<[RwLock<BTreeMap<RuntimeQuotaTarget, RuntimeQuotaState>>]>,
    next_recovery_probe_id: AtomicU64,
}

impl RuntimeQuotaRegistry {
    /// Creates the default fixed-shard registry with the system runtime clock.
    #[must_use]
    pub fn new() -> Self {
        Self::with_clock(Arc::new(SystemRuntimeHealthClock))
    }

    /// Creates the default fixed-shard registry with an injected runtime clock.
    #[must_use]
    pub fn with_clock(clock: Arc<dyn RuntimeHealthClock>) -> Self {
        Self {
            clock,
            shards: build_shards(DEFAULT_RUNTIME_QUOTA_SHARD_COUNT),
            next_recovery_probe_id: AtomicU64::new(1),
        }
    }

    /// Creates a bounded registry with an explicit power-of-two shard count.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeQuotaRegistryBuildError`] before allocating state when the count is unsafe.
    pub fn try_with_clock_and_shards(
        clock: Arc<dyn RuntimeHealthClock>,
        shard_count: usize,
    ) -> Result<Self, RuntimeQuotaRegistryBuildError> {
        validate_shard_count(shard_count)?;
        Ok(Self {
            clock,
            shards: build_shards(shard_count),
            next_recovery_probe_id: AtomicU64::new(1),
        })
    }

    /// Returns the number of fixed independently locked shards.
    #[must_use]
    pub const fn shard_count(&self) -> usize {
        self.shards.len()
    }

    /// Stores one sanitized snapshot, replacing only an equal-or-newer observation for its target.
    ///
    /// A newer observation invalidates any prior recovery ticket so a late probe cannot overwrite
    /// fresh quota evidence.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeQuotaError`] without partial mutation when a shard is unavailable, full,
    /// or the observation timestamp regresses.
    pub fn record_snapshot(
        &self,
        snapshot: QuotaSnapshot,
    ) -> Result<QuotaSnapshot, RuntimeQuotaError> {
        let target = snapshot.target().clone();
        let now_ms = self.clock.now_ms()?;
        let mut states = self.write_shard(&target)?;
        if let Some(previous) = states.get(&target) {
            if snapshot.observed_at_ms() < previous.snapshot.observed_at_ms() {
                return Err(RuntimeQuotaError::ObservationTimeRegressed);
            }
        } else {
            ensure_insert_capacity(&mut states, now_ms)?;
        }
        states.insert(
            target,
            RuntimeQuotaState {
                snapshot: snapshot.clone(),
                recovery_probe: None,
            },
        );
        Ok(snapshot)
    }

    /// Records a 429 as an exact binding-wide quota snapshot with explicit source/confidence.
    ///
    /// A positive retry-after becomes Header/Observed reset evidence. Missing or zero retry-after
    /// uses the caller's positive finite fallback and is explicitly Estimated rather than claimed
    /// as an upstream reset Header.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeQuotaError`] when the fallback cannot be represented or the target cannot
    /// be retained.
    pub fn record_rate_limited(
        &self,
        target: RuntimeQuotaTarget,
        observed_at_ms: i64,
        retry_after: Option<Duration>,
        fallback: Duration,
    ) -> Result<QuotaSnapshot, RuntimeQuotaError> {
        let (duration, source, confidence) =
            match retry_after.filter(|duration| !duration.is_zero()) {
                Some(duration) => (duration, QuotaSource::Header, QuotaConfidence::Observed),
                None => (fallback, QuotaSource::Estimated, QuotaConfidence::Estimated),
            };
        if duration.is_zero() {
            return Err(RuntimeQuotaError::FallbackDurationNotPositive);
        }
        let reset_at_ms = add_duration_to_timestamp(observed_at_ms, duration)?;
        let window = QuotaWindow::try_new("rate_limit", None, Some(0), Some(reset_at_ms))
            .map_err(|_| RuntimeQuotaError::FallbackDurationOverflow)?;
        let snapshot =
            QuotaSnapshot::try_new(target, vec![window], source, confidence, observed_at_ms)
                .map_err(|_| RuntimeQuotaError::FallbackDurationOverflow)?;
        self.record_snapshot(snapshot)
    }

    /// Returns the retained latest snapshot for one exact target.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeQuotaError::ShardLockPoisoned`] when the target's shard is unavailable.
    pub fn snapshot(
        &self,
        target: &RuntimeQuotaTarget,
    ) -> Result<Option<QuotaSnapshot>, RuntimeQuotaError> {
        Ok(self
            .read_shard(target)?
            .get(target)
            .map(|state| state.snapshot.clone()))
    }

    /// Returns request-time availability at the shared runtime clock instant.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeQuotaError`] when the clock or target shard is unavailable.
    pub fn availability(
        &self,
        target: &RuntimeQuotaTarget,
    ) -> Result<RuntimeQuotaAvailability, RuntimeQuotaError> {
        let now_ms = self.clock.now_ms()?;
        self.availability_at(target, now_ms)
    }

    /// Returns request-time availability using an explicit timestamp for deterministic tests.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeQuotaError::ShardLockPoisoned`] when the target shard is unavailable.
    pub fn availability_at(
        &self,
        target: &RuntimeQuotaTarget,
        now_ms: i64,
    ) -> Result<RuntimeQuotaAvailability, RuntimeQuotaError> {
        let states = self.read_shard(target)?;
        Ok(states
            .get(target)
            .map_or(RuntimeQuotaAvailability::Available, |state| {
                state.availability_at(now_ms)
            }))
    }

    /// Returns whether a binding-wide quota permits ordinary scheduling now.
    ///
    /// Clock and shard failures fail closed and therefore return `false`.
    #[must_use]
    pub fn endpoint_credential_is_available(
        &self,
        endpoint_id: &EndpointId,
        credential_id: &CredentialId,
    ) -> bool {
        self.availability(&RuntimeQuotaTarget::endpoint_credential(
            endpoint_id.clone(),
            credential_id.clone(),
        ))
        .is_ok_and(RuntimeQuotaAvailability::is_available)
    }

    /// Returns whether a model-scoped quota permits ordinary scheduling now.
    ///
    /// Empty model labels, clock failures, and shard failures all fail closed.
    #[must_use]
    pub fn endpoint_credential_model_is_available(
        &self,
        endpoint_id: &EndpointId,
        credential_id: &CredentialId,
        upstream_model: &str,
    ) -> bool {
        RuntimeQuotaTarget::endpoint_credential_model(
            endpoint_id.clone(),
            credential_id.clone(),
            upstream_model,
        )
        .is_ok_and(|target| {
            self.availability(&target)
                .is_ok_and(RuntimeQuotaAvailability::is_available)
        })
    }

    /// Acquires at most one controlled recovery ticket after all exhausted windows reset.
    ///
    /// Ordinary scheduling remains unavailable while the ticket exists. An expired ticket may be
    /// replaced by one later controlled ticket; it never reopens ordinary traffic.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeQuotaError`] when the clock/shard is unavailable or `expires_at_ms` is not
    /// strictly later than the current clock instant.
    pub fn begin_recovery_probe(
        &self,
        target: &RuntimeQuotaTarget,
        expires_at_ms: i64,
    ) -> Result<Option<RuntimeQuotaRecoveryProbe>, RuntimeQuotaError> {
        let now_ms = self.clock.now_ms()?;
        if expires_at_ms <= now_ms {
            return Err(RuntimeQuotaError::RecoveryProbeDeadlineNotInFuture);
        }
        let mut states = self.write_shard(target)?;
        let Some(state) = states.get_mut(target) else {
            return Ok(None);
        };
        let Some(reset_at_ms) = state.snapshot.blocking_reset_at_ms() else {
            return Ok(None);
        };
        if now_ms < reset_at_ms {
            return Ok(None);
        }
        if state
            .recovery_probe
            .is_some_and(|probe| now_ms < probe.expires_at_ms)
        {
            return Ok(None);
        }
        let probe_id = self.next_recovery_probe_id()?;
        state.recovery_probe = Some(RuntimeQuotaRecoveryProbeState {
            probe_id,
            expires_at_ms,
        });
        Ok(Some(RuntimeQuotaRecoveryProbe {
            target: target.clone(),
            probe_id,
            expires_at_ms,
        }))
    }

    /// Atomically accepts a current recovery ticket and its exact sanitized follow-up snapshot.
    ///
    /// A non-exhausted snapshot reopens ordinary scheduling. A still-exhausted snapshot retains
    /// the new reset evidence and remains blocked. A stale ticket cannot overwrite any newer
    /// observation or ticket state.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeQuotaError`] without mutating state when the ticket is stale, the snapshot
    /// target differs, or the observation time regresses.
    pub fn complete_recovery_probe(
        &self,
        probe: RuntimeQuotaRecoveryProbe,
        snapshot: QuotaSnapshot,
    ) -> Result<QuotaSnapshot, RuntimeQuotaError> {
        let RuntimeQuotaRecoveryProbe {
            target,
            probe_id,
            expires_at_ms,
        } = probe;
        if target != *snapshot.target() {
            return Err(RuntimeQuotaError::RecoverySnapshotTargetMismatch);
        }
        let now_ms = self.clock.now_ms()?;
        let mut states = self.write_shard(&target)?;
        let Some(state) = states.get(&target) else {
            return Err(RuntimeQuotaError::StaleRecoveryProbe);
        };
        let Some(current_probe) = state.recovery_probe else {
            return Err(RuntimeQuotaError::StaleRecoveryProbe);
        };
        if current_probe.probe_id != probe_id
            || current_probe.expires_at_ms != expires_at_ms
            || now_ms >= current_probe.expires_at_ms
            || snapshot.observed_at_ms() < state.snapshot.observed_at_ms()
        {
            return Err(RuntimeQuotaError::StaleRecoveryProbe);
        }
        states.insert(
            target,
            RuntimeQuotaState {
                snapshot: snapshot.clone(),
                recovery_probe: None,
            },
        );
        Ok(snapshot)
    }

    /// Counts retained targets for bounded diagnostics and tests.
    ///
    /// This reads every shard and is not a request-time operation.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeQuotaError::ShardLockPoisoned`] if any shard is unavailable.
    pub fn entry_count(&self) -> Result<usize, RuntimeQuotaError> {
        let mut count = 0_usize;
        for shard in &self.shards {
            let states = shard
                .read()
                .map_err(|_| RuntimeQuotaError::ShardLockPoisoned)?;
            count = count.saturating_add(states.len());
        }
        Ok(count)
    }

    fn read_shard(
        &self,
        target: &RuntimeQuotaTarget,
    ) -> Result<
        RwLockReadGuard<'_, BTreeMap<RuntimeQuotaTarget, RuntimeQuotaState>>,
        RuntimeQuotaError,
    > {
        self.shards[self.shard_index(target)]
            .read()
            .map_err(|_| RuntimeQuotaError::ShardLockPoisoned)
    }

    fn write_shard(
        &self,
        target: &RuntimeQuotaTarget,
    ) -> Result<
        RwLockWriteGuard<'_, BTreeMap<RuntimeQuotaTarget, RuntimeQuotaState>>,
        RuntimeQuotaError,
    > {
        self.shards[self.shard_index(target)]
            .write()
            .map_err(|_| RuntimeQuotaError::ShardLockPoisoned)
    }

    fn next_recovery_probe_id(&self) -> Result<u64, RuntimeQuotaError> {
        self.next_recovery_probe_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1).filter(|next| *next != 0)
            })
            .map_err(|_| RuntimeQuotaError::RecoveryProbeIdOverflow)
    }

    fn shard_index(&self, target: &RuntimeQuotaTarget) -> usize {
        let mut hasher = DefaultHasher::new();
        target.hash(&mut hasher);
        let Ok(mask) = u64::try_from(self.shards.len() - 1) else {
            return 0;
        };
        usize::try_from(hasher.finish() & mask).unwrap_or_default()
    }
}

impl Default for RuntimeQuotaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RuntimeQuotaRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeQuotaRegistry")
            .field("shard_count", &self.shard_count())
            .field(
                "entries_per_shard_limit",
                &MAX_RUNTIME_QUOTA_ENTRIES_PER_SHARD,
            )
            .finish_non_exhaustive()
    }
}

impl RuntimeQuotaState {
    fn availability_at(&self, now_ms: i64) -> RuntimeQuotaAvailability {
        let Some(reset_at_ms) = self.snapshot.blocking_reset_at_ms() else {
            return RuntimeQuotaAvailability::Available;
        };
        if now_ms < reset_at_ms {
            return RuntimeQuotaAvailability::Exhausted { reset_at_ms };
        }
        if let Some(probe) = self.recovery_probe
            && now_ms < probe.expires_at_ms
        {
            return RuntimeQuotaAvailability::RecoveryProbeInFlight {
                reset_at_ms,
                expires_at_ms: probe.expires_at_ms,
            };
        }
        RuntimeQuotaAvailability::RecoveryRequired { reset_at_ms }
    }
}

/// Keeps bounded storage from retaining a completed quota recovery forever.
///
/// Only state that is already available may be reclaimed: dropping a due reset that still needs
/// a controlled recovery ticket would turn an absent entry into ordinary scheduling availability.
fn ensure_insert_capacity(
    states: &mut BTreeMap<RuntimeQuotaTarget, RuntimeQuotaState>,
    now_ms: i64,
) -> Result<(), RuntimeQuotaError> {
    if states.len() < MAX_RUNTIME_QUOTA_ENTRIES_PER_SHARD {
        return Ok(());
    }
    states.retain(|_, state| !state.availability_at(now_ms).is_available());
    if states.len() >= MAX_RUNTIME_QUOTA_ENTRIES_PER_SHARD {
        return Err(RuntimeQuotaError::ShardCapacityExceeded);
    }
    Ok(())
}

fn build_shards(
    shard_count: usize,
) -> Box<[RwLock<BTreeMap<RuntimeQuotaTarget, RuntimeQuotaState>>]> {
    std::iter::repeat_with(|| RwLock::new(BTreeMap::new()))
        .take(shard_count)
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn validate_shard_count(shard_count: usize) -> Result<(), RuntimeQuotaRegistryBuildError> {
    if shard_count == 0 {
        return Err(RuntimeQuotaRegistryBuildError::ZeroShardCount);
    }
    if !shard_count.is_power_of_two() {
        return Err(RuntimeQuotaRegistryBuildError::NonPowerOfTwoShardCount);
    }
    if shard_count > MAX_RUNTIME_QUOTA_SHARD_COUNT {
        return Err(RuntimeQuotaRegistryBuildError::TooManyShards);
    }
    Ok(())
}

fn add_duration_to_timestamp(now_ms: i64, duration: Duration) -> Result<i64, RuntimeQuotaError> {
    let duration_ms = i64::try_from(duration.as_millis())
        .map_err(|_| RuntimeQuotaError::FallbackDurationOverflow)?;
    now_ms
        .checked_add(duration_ms)
        .ok_or(RuntimeQuotaError::FallbackDurationOverflow)
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        sync::{
            Arc,
            atomic::{AtomicI64, Ordering},
        },
        time::Duration,
    };

    use gateway_core::{CredentialId, EndpointId};

    use super::{
        MAX_RUNTIME_QUOTA_ENTRIES_PER_SHARD, QuotaConfidence, QuotaSnapshot, QuotaSnapshotError,
        QuotaSource, QuotaWindow, RuntimeQuotaAvailability, RuntimeQuotaError,
        RuntimeQuotaRegistry, RuntimeQuotaTarget, RuntimeQuotaTargetError,
    };
    use crate::{RuntimeHealthClock, RuntimeHealthClockError};

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn source_confidence_reset_and_model_scope_are_exact_target_isolated() -> TestResult {
        let clock = Arc::new(FixedClock::new(100));
        let registry = RuntimeQuotaRegistry::with_clock(clock.clone());
        let model_a = target("endpoint-a", "credential-a", "model-a")?;
        let model_b = target("endpoint-a", "credential-a", "model-b")?;
        let snapshot = exhausted_snapshot(
            model_a.clone(),
            100,
            200,
            QuotaSource::Billing,
            QuotaConfidence::Authoritative,
        )?;
        registry.record_snapshot(snapshot.clone())?;

        let retained = registry
            .snapshot(&model_a)?
            .ok_or("missing quota snapshot")?;
        assert_eq!(retained.source(), QuotaSource::Billing);
        assert_eq!(retained.confidence(), QuotaConfidence::Authoritative);
        assert_eq!(retained.blocking_reset_at_ms(), Some(200));
        assert_eq!(
            registry.availability(&model_a)?,
            RuntimeQuotaAvailability::Exhausted { reset_at_ms: 200 }
        );
        assert_eq!(
            registry.availability(&model_b)?,
            RuntimeQuotaAvailability::Available
        );
        Ok(())
    }

    #[test]
    fn rate_limit_records_header_or_explicit_estimate_without_conflation() -> TestResult {
        let registry = RuntimeQuotaRegistry::new();
        let header_target = RuntimeQuotaTarget::endpoint_credential(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new("credential-a")?,
        );
        let estimate_target = RuntimeQuotaTarget::endpoint_credential(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new("credential-b")?,
        );
        let header = registry.record_rate_limited(
            header_target,
            100,
            Some(Duration::from_millis(20)),
            Duration::from_millis(30),
        )?;
        let estimate =
            registry.record_rate_limited(estimate_target, 100, None, Duration::from_millis(30))?;
        assert_eq!(header.source(), QuotaSource::Header);
        assert_eq!(header.confidence(), QuotaConfidence::Observed);
        assert_eq!(header.blocking_reset_at_ms(), Some(120));
        assert_eq!(estimate.source(), QuotaSource::Estimated);
        assert_eq!(estimate.confidence(), QuotaConfidence::Estimated);
        assert_eq!(estimate.blocking_reset_at_ms(), Some(130));
        assert_eq!(
            registry.record_rate_limited(
                RuntimeQuotaTarget::endpoint_credential(
                    EndpointId::try_new("endpoint-a")?,
                    CredentialId::try_new("credential-zero-fallback")?,
                ),
                100,
                None,
                Duration::ZERO,
            ),
            Err(RuntimeQuotaError::FallbackDurationNotPositive)
        );
        Ok(())
    }

    #[test]
    fn reset_requires_one_controlled_recovery_probe_before_ordinary_scheduling() -> TestResult {
        let clock = Arc::new(FixedClock::new(100));
        let registry = RuntimeQuotaRegistry::with_clock(clock.clone());
        let target = target("endpoint-a", "credential-a", "model-a")?;
        registry.record_snapshot(exhausted_snapshot(
            target.clone(),
            100,
            200,
            QuotaSource::Header,
            QuotaConfidence::Observed,
        )?)?;

        assert_eq!(registry.begin_recovery_probe(&target, 250)?, None);
        clock.set_now_ms(200);
        let ticket = registry
            .begin_recovery_probe(&target, 250)?
            .ok_or("due reset did not issue a controlled probe")?;
        assert_eq!(registry.begin_recovery_probe(&target, 250)?, None);
        assert_eq!(
            registry.availability(&target)?,
            RuntimeQuotaAvailability::RecoveryProbeInFlight {
                reset_at_ms: 200,
                expires_at_ms: 250,
            }
        );

        let recovered = available_snapshot(
            target.clone(),
            201,
            QuotaSource::Rest,
            QuotaConfidence::Observed,
        )?;
        registry.complete_recovery_probe(ticket, recovered)?;
        assert_eq!(
            registry.availability(&target)?,
            RuntimeQuotaAvailability::Available
        );
        Ok(())
    }

    #[test]
    fn stale_probe_cannot_overwrite_newer_quota_observation() -> TestResult {
        let clock = Arc::new(FixedClock::new(100));
        let registry = RuntimeQuotaRegistry::with_clock(clock.clone());
        let target = target("endpoint-a", "credential-a", "model-a")?;
        registry.record_snapshot(exhausted_snapshot(
            target.clone(),
            100,
            200,
            QuotaSource::Header,
            QuotaConfidence::Observed,
        )?)?;
        clock.set_now_ms(200);
        let ticket = registry
            .begin_recovery_probe(&target, 250)?
            .ok_or("due reset did not issue a controlled probe")?;
        let newer = exhausted_snapshot(
            target.clone(),
            201,
            300,
            QuotaSource::Header,
            QuotaConfidence::Observed,
        )?;
        registry.record_snapshot(newer.clone())?;
        assert_eq!(
            registry.complete_recovery_probe(
                ticket,
                available_snapshot(
                    target.clone(),
                    202,
                    QuotaSource::Rest,
                    QuotaConfidence::Observed,
                )?
            ),
            Err(RuntimeQuotaError::StaleRecoveryProbe)
        );
        assert_eq!(registry.snapshot(&target)?, Some(newer));
        Ok(())
    }

    #[test]
    fn quota_inputs_reject_ambiguous_or_unsupported_precision() -> TestResult {
        assert_eq!(
            RuntimeQuotaTarget::endpoint_credential_model(
                EndpointId::try_new("endpoint-a")?,
                CredentialId::try_new("credential-a")?,
                ""
            ),
            Err(RuntimeQuotaTargetError::EmptyUpstreamModel)
        );
        let quota_target = target("endpoint-a", "credential-a", "model-a")?;
        let exhausted_without_reset = QuotaSnapshot::try_new(
            quota_target,
            vec![QuotaWindow::try_new("requests", Some(10), Some(0), None)?],
            QuotaSource::Header,
            QuotaConfidence::Observed,
            100,
        );
        assert!(exhausted_without_reset.is_err());
        assert_eq!(
            QuotaSnapshot::try_new(
                target("endpoint-a", "credential-a", "model-a")?,
                Vec::new(),
                QuotaSource::Estimated,
                QuotaConfidence::Observed,
                100,
            ),
            Err(QuotaSnapshotError::EstimatedSourceConfidenceMismatch)
        );
        Ok(())
    }

    #[test]
    fn a_full_shard_reclaims_available_snapshots_but_never_blocking_quota() -> TestResult {
        let clock: Arc<dyn RuntimeHealthClock> = Arc::new(FixedClock::new(100));
        let available_registry =
            RuntimeQuotaRegistry::try_with_clock_and_shards(Arc::clone(&clock), 1)?;
        for index in 0..MAX_RUNTIME_QUOTA_ENTRIES_PER_SHARD {
            available_registry.record_snapshot(available_snapshot(
                target(
                    &format!("endpoint-available-{index}"),
                    "credential-a",
                    "model-a",
                )?,
                100,
                QuotaSource::Rest,
                QuotaConfidence::Observed,
            )?)?;
        }
        available_registry.record_snapshot(exhausted_snapshot(
            target("endpoint-reclaimed", "credential-a", "model-a")?,
            100,
            200,
            QuotaSource::Header,
            QuotaConfidence::Observed,
        )?)?;
        assert_eq!(available_registry.entry_count()?, 1);

        let blocking_registry = RuntimeQuotaRegistry::try_with_clock_and_shards(clock, 1)?;
        for index in 0..MAX_RUNTIME_QUOTA_ENTRIES_PER_SHARD {
            blocking_registry.record_snapshot(exhausted_snapshot(
                target(
                    &format!("endpoint-blocking-{index}"),
                    "credential-a",
                    "model-a",
                )?,
                100,
                200,
                QuotaSource::Header,
                QuotaConfidence::Observed,
            )?)?;
        }
        assert_eq!(
            blocking_registry.record_snapshot(exhausted_snapshot(
                target("endpoint-overflow", "credential-a", "model-a")?,
                100,
                200,
                QuotaSource::Header,
                QuotaConfidence::Observed,
            )?),
            Err(RuntimeQuotaError::ShardCapacityExceeded)
        );
        assert_eq!(
            blocking_registry.entry_count()?,
            MAX_RUNTIME_QUOTA_ENTRIES_PER_SHARD
        );
        Ok(())
    }

    fn target(
        endpoint: &str,
        credential: &str,
        model: &str,
    ) -> Result<RuntimeQuotaTarget, Box<dyn Error>> {
        Ok(RuntimeQuotaTarget::endpoint_credential_model(
            EndpointId::try_new(endpoint)?,
            CredentialId::try_new(credential)?,
            model,
        )?)
    }

    fn exhausted_snapshot(
        target: RuntimeQuotaTarget,
        observed_at_ms: i64,
        reset_at_ms: i64,
        source: QuotaSource,
        confidence: QuotaConfidence,
    ) -> Result<QuotaSnapshot, Box<dyn Error>> {
        Ok(QuotaSnapshot::try_new(
            target,
            vec![QuotaWindow::try_new(
                "requests",
                Some(10),
                Some(0),
                Some(reset_at_ms),
            )?],
            source,
            confidence,
            observed_at_ms,
        )?)
    }

    fn available_snapshot(
        target: RuntimeQuotaTarget,
        observed_at_ms: i64,
        source: QuotaSource,
        confidence: QuotaConfidence,
    ) -> Result<QuotaSnapshot, Box<dyn Error>> {
        Ok(QuotaSnapshot::try_new(
            target,
            vec![QuotaWindow::try_new("requests", Some(10), Some(10), None)?],
            source,
            confidence,
            observed_at_ms,
        )?)
    }

    #[derive(Debug)]
    struct FixedClock {
        now_ms: AtomicI64,
    }

    impl FixedClock {
        const fn new(now_ms: i64) -> Self {
            Self {
                now_ms: AtomicI64::new(now_ms),
            }
        }

        fn set_now_ms(&self, now_ms: i64) {
            self.now_ms.store(now_ms, Ordering::Release);
        }
    }

    impl RuntimeHealthClock for FixedClock {
        fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
            Ok(self.now_ms.load(Ordering::Acquire))
        }
    }
}
