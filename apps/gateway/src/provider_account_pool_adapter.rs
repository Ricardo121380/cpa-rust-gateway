//! Composition-layer Provider account-pool projection.
//!
//! This adapter is intentionally narrower than a Provider implementation. The serving
//! composition supplies already validated, secret-free account descriptors together with the
//! exact immutable credential pools and the runtime Health/Quota registries used by request
//! scheduling. The adapter only observes those objects and produces the Provider-neutral
//! management snapshot consumed by `GET /admin/operations/provider-account-pools`.
//!
//! In particular, this module does not open `SQLite`, decrypt credentials, contact a Provider,
//! acquire a lease, refresh OAuth, or start a worker. A short bounded cache keeps a cursor's second
//! page on the same observation snapshot without retaining unbounded historical state.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gateway_control::provider_account_pool_service::{
    MAX_PROVIDER_ACCOUNT_COOLDOWN_MS, MIN_PROVIDER_ACCOUNT_COOLDOWN_MS, ProviderAccountAuthStatus,
    ProviderAccountOperatorAction, ProviderAccountOperatorActionKind,
    ProviderAccountOperatorReceipt, ProviderAccountOperatorState, ProviderAccountPoolError,
    ProviderAccountPoolFacade, ProviderAccountPoolItem, ProviderAccountPoolPage,
    ProviderAccountPoolQuery, ProviderAccountPoolSnapshot, ProviderAccountRuntimeStatus,
};
use gateway_core::{CredentialId, EndpointId, ProviderId};
use gateway_router::{
    QuotaConfidence, QuotaSnapshot, QuotaSource, RuntimeCredentialAccountStatus,
    RuntimeHealthAccountRecoveryResult, RuntimeHealthAvailability, RuntimeHealthKey,
    RuntimeHealthRegistry, RuntimeQuotaAvailability, RuntimeQuotaRegistry, RuntimeQuotaTarget,
};
use gateway_upstream::{CredentialPoolEntrySnapshot, EndpointCredentialPools};
#[cfg(test)]
use gateway_upstream::{CredentialSecret, EndpointCredentialInput, EndpointCredentialPool};

/// Maximum number of descriptors retained by one process-local adapter.
///
/// The management facade already bounds one response page. This additional bound prevents a
/// malformed composition call from turning one snapshot rebuild into an unbounded allocation.
pub(crate) const MAX_PROVIDER_ACCOUNT_DESCRIPTORS: usize = 100_000;
/// Maximum number of exact model labels consulted for one account row.
pub(crate) const MAX_PROVIDER_ACCOUNT_MODELS: usize = 256;
/// Maximum cache lifetime accepted by the composition layer.
pub(crate) const MAX_PROVIDER_ACCOUNT_POOL_TTL: Duration = Duration::from_mins(1);
/// Maximum time an immutable snapshot may remain available to an already issued cursor.
pub(crate) const MAX_PROVIDER_ACCOUNT_POOL_CURSOR_RETENTION: Duration = Duration::from_mins(10);
/// Maximum old snapshots retained for concurrent bounded pagination sequences.
pub(crate) const MAX_RETAINED_PROVIDER_ACCOUNT_POOL_SNAPSHOTS: usize = 8;
const MAX_PROVIDER_ACCOUNT_ID_CHARS: usize = 128;
const SNAPSHOT_INSTANCE_NONCE_BYTES: usize = 16;
const OPERATOR_RECOVERY_TTL_MS: i64 = 30_000;

/// A clock supplied by the serving composition and tests.
pub(crate) trait ProviderAccountPoolClock: Send + Sync {
    /// Returns the current non-negative Unix-millisecond observation time.
    fn now_ms(&self) -> Result<i64, ProviderAccountPoolClockError>;
}

/// Safe failure from an account-pool observation clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProviderAccountPoolClockError;

/// The process clock used by the production composition.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SystemProviderAccountPoolClock;

impl ProviderAccountPoolClock for SystemProviderAccountPoolClock {
    fn now_ms(&self) -> Result<i64, ProviderAccountPoolClockError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ProviderAccountPoolClockError)?;
        i64::try_from(elapsed.as_millis()).map_err(|_| ProviderAccountPoolClockError)
    }
}

/// Whether a descriptor came from a Provider-native pool or the ordinary control-plane pool.
///
/// The source is deliberately not rendered in the management response. Keeping it in the
/// composition descriptor makes it impossible for a future adapter to accidentally merge native
/// and ordinary credential semantics while still allowing both sources to use this facade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderAccountDescriptorSource {
    /// Provider-owned native account store (for example Grok Build/Console/Web).
    Native,
    /// Ordinary Endpoint/Credential bindings compiled by the control plane.
    Ordinary,
}

/// Explicit, secret-free descriptor supplied by a Provider composition adapter.
///
/// All fields are copied from already validated metadata. The adapter never treats an opaque
/// identifier as credential material and never opens a Store to fill a missing field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProviderAccountDescriptor {
    pub(crate) source: ProviderAccountDescriptorSource,
    pub(crate) provider_id: ProviderId,
    pub(crate) channel_id: EndpointId,
    pub(crate) account_id: CredentialId,
    pub(crate) account_kind: String,
    pub(crate) auth_status: ProviderAccountAuthStatus,
    /// Persisted non-secret runtime hint for entries that are deliberately absent from the
    /// compiled pool, such as a control-plane Credential retained in Cooling state.
    pub(crate) runtime_status_hint: ProviderAccountRuntimeStatus,
    pub(crate) enabled: bool,
    pub(crate) priority: i64,
    pub(crate) weight: u32,
    pub(crate) max_concurrency: u32,
    pub(crate) expires_at_ms: Option<i64>,
    pub(crate) refresh_due_at_ms: Option<i64>,
    pub(crate) quota_sync_due_at_ms: Option<i64>,
    /// Exact compiler-approved upstream model labels used for model-scoped Health/Quota reads.
    pub(crate) upstream_models: Vec<String>,
}

impl ProviderAccountDescriptor {
    fn key(&self) -> (&str, &str, &str) {
        (
            self.provider_id.as_str(),
            self.channel_id.as_str(),
            self.account_id.as_str(),
        )
    }

    fn validate(&self) -> Result<(), ProviderAccountPoolAdapterBuildError> {
        if !valid_opaque_id(self.provider_id.as_str())
            || !valid_opaque_id(self.channel_id.as_str())
            || !valid_opaque_id(self.account_id.as_str())
            || self.account_kind.trim().is_empty()
            || self.account_kind.chars().count() > 128
            || self.priority < 0
            || !(1..=10_000).contains(&self.weight)
            || !(1..=100_000).contains(&self.max_concurrency)
            || self.expires_at_ms.is_some_and(|value| value < 0)
            || self.refresh_due_at_ms.is_some_and(|value| value < 0)
            || self.quota_sync_due_at_ms.is_some_and(|value| value < 0)
            || self.upstream_models.len() > MAX_PROVIDER_ACCOUNT_MODELS
        {
            return Err(ProviderAccountPoolAdapterBuildError::InvalidDescriptor);
        }
        let mut models = BTreeSet::new();
        for model in &self.upstream_models {
            if model.trim().is_empty()
                || model.chars().count() > 256
                || !models.insert(model.as_str())
            {
                return Err(ProviderAccountPoolAdapterBuildError::InvalidDescriptor);
            }
        }
        Ok(())
    }
}

/// Safe construction failures for [`ProviderAccountPoolAdapter`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderAccountPoolAdapterBuildError {
    /// A descriptor, identity, or scheduling field exceeded its finite bound.
    InvalidDescriptor,
    /// Two descriptors used the same Provider/Channel/Account key.
    DuplicateDescriptor,
    /// The cache lifetime was zero or exceeded the process bound.
    InvalidTtl,
    /// The process could not create an unguessable per-adapter snapshot namespace.
    EntropyUnavailable,
}

impl fmt::Display for ProviderAccountPoolAdapterBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidDescriptor => "provider account-pool descriptor is invalid",
            Self::DuplicateDescriptor => "provider account-pool descriptor is duplicated",
            Self::InvalidTtl => "provider account-pool cache ttl is invalid",
            Self::EntropyUnavailable => "provider account-pool snapshot namespace is unavailable",
        })
    }
}

impl std::error::Error for ProviderAccountPoolAdapterBuildError {}

/// A bounded cache entry. The freshness deadline controls when a cursorless request may create a
/// newer snapshot; the retention deadline keeps an already-issued cursor on this immutable view
/// long enough to finish bounded pagination without serving mixed observations.
struct CachedSnapshot {
    snapshot: ProviderAccountPoolSnapshot,
    fresh_until_ms: i64,
    retain_until_ms: i64,
}

#[derive(Default)]
struct SnapshotCache {
    current: Option<CachedSnapshot>,
    retained: VecDeque<CachedSnapshot>,
}

/// Read-only Provider account-pool facade assembled by the serving composition.
pub(crate) struct ProviderAccountPoolAdapter {
    descriptors: Arc<[ProviderAccountDescriptor]>,
    credential_pools: Arc<EndpointCredentialPools>,
    runtime_health: Arc<RuntimeHealthRegistry>,
    runtime_quota: Arc<RuntimeQuotaRegistry>,
    clock: Arc<dyn ProviderAccountPoolClock>,
    ttl_ms: i64,
    cursor_retention_ms: i64,
    config_version_id: Option<String>,
    instance_nonce: String,
    next_snapshot_generation: AtomicU64,
    cache: Mutex<SnapshotCache>,
}

impl ProviderAccountPoolAdapter {
    /// Creates an adapter from explicit descriptors and the exact runtime objects used by routing.
    ///
    /// This constructor performs no I/O. In particular, the caller must supply descriptors that
    /// were already obtained from its Provider-specific store/compilation boundary.
    pub(crate) fn try_new(
        descriptors: Vec<ProviderAccountDescriptor>,
        credential_pools: Arc<EndpointCredentialPools>,
        runtime_health: Arc<RuntimeHealthRegistry>,
        runtime_quota: Arc<RuntimeQuotaRegistry>,
        clock: Arc<dyn ProviderAccountPoolClock>,
        ttl: Duration,
        cursor_retention: Duration,
    ) -> Result<Self, ProviderAccountPoolAdapterBuildError> {
        if ttl.is_zero()
            || ttl > MAX_PROVIDER_ACCOUNT_POOL_TTL
            || cursor_retention < ttl
            || cursor_retention > MAX_PROVIDER_ACCOUNT_POOL_CURSOR_RETENTION
        {
            return Err(ProviderAccountPoolAdapterBuildError::InvalidTtl);
        }
        if descriptors.len() > MAX_PROVIDER_ACCOUNT_DESCRIPTORS {
            return Err(ProviderAccountPoolAdapterBuildError::InvalidDescriptor);
        }
        let mut keys = BTreeSet::new();
        for descriptor in &descriptors {
            descriptor.validate()?;
            if !keys.insert(descriptor.key()) {
                return Err(ProviderAccountPoolAdapterBuildError::DuplicateDescriptor);
            }
        }
        let ttl_ms = i64::try_from(ttl.as_millis())
            .map_err(|_| ProviderAccountPoolAdapterBuildError::InvalidTtl)?;
        let cursor_retention_ms = i64::try_from(cursor_retention.as_millis())
            .map_err(|_| ProviderAccountPoolAdapterBuildError::InvalidTtl)?;
        Ok(Self {
            descriptors: descriptors.into(),
            credential_pools,
            runtime_health,
            runtime_quota,
            clock,
            ttl_ms,
            cursor_retention_ms,
            config_version_id: None,
            instance_nonce: random_instance_nonce()?,
            next_snapshot_generation: AtomicU64::new(1),
            cache: Mutex::new(SnapshotCache::default()),
        })
    }

    /// Binds an already validated adapter to the exact serving Config Version.
    ///
    /// # Errors
    ///
    /// Returns an invalid-descriptor error when the Config Version identity is blank or overlong.
    pub(crate) fn with_config_version(
        mut self,
        config_version_id: String,
    ) -> Result<Self, ProviderAccountPoolAdapterBuildError> {
        if !valid_opaque_id(&config_version_id) {
            return Err(ProviderAccountPoolAdapterBuildError::InvalidDescriptor);
        }
        self.config_version_id = Some(config_version_id);
        Ok(self)
    }

    fn descriptor_for_action(
        &self,
        action: &ProviderAccountOperatorAction,
    ) -> Result<&ProviderAccountDescriptor, ProviderAccountPoolError> {
        if self.config_version_id.as_deref() != Some(action.config_version_id.as_str()) {
            return Err(ProviderAccountPoolError::ActionTargetUnavailable);
        }
        let descriptor = self
            .descriptors
            .iter()
            .find(|descriptor| {
                descriptor.provider_id == action.provider_id
                    && descriptor.channel_id == action.channel_id
                    && descriptor.account_id == action.account_id
            })
            .ok_or(ProviderAccountPoolError::ActionTargetUnavailable)?;
        if action.upstream_model.as_ref().is_some_and(|model| {
            !descriptor
                .upstream_models
                .iter()
                .any(|candidate| candidate == model)
        }) {
            return Err(ProviderAccountPoolError::ActionTargetUnavailable);
        }
        Ok(descriptor)
    }

    fn invalidate_current_snapshot(
        &self,
        observed_at_ms: i64,
    ) -> Result<(), ProviderAccountPoolError> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
        if let Some(previous) = cache.current.take()
            && previous.retain_until_ms > observed_at_ms
        {
            cache.retained.push_front(previous);
            cache
                .retained
                .truncate(MAX_RETAINED_PROVIDER_ACCOUNT_POOL_SNAPSHOTS);
        }
        Ok(())
    }

    fn apply_recovery(
        &self,
        action: &ProviderAccountOperatorAction,
        observed_at_ms: i64,
    ) -> Result<ProviderAccountOperatorReceipt, ProviderAccountPoolError> {
        let account_status = self
            .runtime_health
            .credential_account_status_at(&action.channel_id, &action.account_id, observed_at_ms)
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
        match account_status {
            RuntimeCredentialAccountStatus::Unauthorized
            | RuntimeCredentialAccountStatus::Forbidden
                if action.upstream_model.is_none() =>
            {
                let expires_at_ms = observed_at_ms
                    .checked_add(OPERATOR_RECOVERY_TTL_MS)
                    .ok_or(ProviderAccountPoolError::SourceUnavailable)?;
                let ticket = self
                    .runtime_health
                    .begin_account_recovery(&action.channel_id, &action.account_id, expires_at_ms)
                    .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
                if let Some(ticket) = ticket {
                    self.runtime_health
                        .complete_account_recovery(
                            ticket,
                            RuntimeHealthAccountRecoveryResult::Allowed,
                        )
                        .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
                }
                Ok(operator_receipt(
                    ProviderAccountOperatorState::ProbeScheduled,
                    observed_at_ms,
                    None,
                ))
            }
            RuntimeCredentialAccountStatus::RecoveryInFlight { .. } => Ok(operator_receipt(
                ProviderAccountOperatorState::ProbeScheduled,
                observed_at_ms,
                None,
            )),
            RuntimeCredentialAccountStatus::Forbidden
            | RuntimeCredentialAccountStatus::Unauthorized
            | RuntimeCredentialAccountStatus::Available => {
                self.apply_quota_recovery(action, observed_at_ms)
            }
        }
    }

    fn apply_quota_recovery(
        &self,
        action: &ProviderAccountOperatorAction,
        observed_at_ms: i64,
    ) -> Result<ProviderAccountOperatorReceipt, ProviderAccountPoolError> {
        let target = match action.upstream_model.clone() {
            Some(model) => RuntimeQuotaTarget::endpoint_credential_model(
                action.channel_id.clone(),
                action.account_id.clone(),
                model,
            )
            .map_err(|_| ProviderAccountPoolError::InvalidAction)?,
            None => RuntimeQuotaTarget::endpoint_credential(
                action.channel_id.clone(),
                action.account_id.clone(),
            ),
        };
        let availability = self
            .runtime_quota
            .availability_at(&target, observed_at_ms)
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
        let state = match availability {
            RuntimeQuotaAvailability::Available => ProviderAccountOperatorState::Rejected,
            RuntimeQuotaAvailability::Exhausted { .. } => {
                ProviderAccountOperatorState::RecoveryRequired
            }
            RuntimeQuotaAvailability::RecoveryProbeInFlight { .. } => {
                ProviderAccountOperatorState::ProbeScheduled
            }
            RuntimeQuotaAvailability::RecoveryRequired { .. } => {
                self.complete_quota_recovery(target, observed_at_ms)?;
                ProviderAccountOperatorState::ProbeScheduled
            }
        };
        Ok(operator_receipt(state, observed_at_ms, None))
    }

    fn complete_quota_recovery(
        &self,
        target: RuntimeQuotaTarget,
        observed_at_ms: i64,
    ) -> Result<(), ProviderAccountPoolError> {
        let expires_at_ms = observed_at_ms
            .checked_add(OPERATOR_RECOVERY_TTL_MS)
            .ok_or(ProviderAccountPoolError::SourceUnavailable)?;
        let ticket = self
            .runtime_quota
            .begin_recovery_probe(&target, expires_at_ms)
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
        if let Some(ticket) = ticket {
            let snapshot = QuotaSnapshot::try_new(
                target,
                Vec::new(),
                QuotaSource::Estimated,
                QuotaConfidence::Estimated,
                observed_at_ms,
            )
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
            self.runtime_quota
                .complete_recovery_probe(ticket, snapshot)
                .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
        }
        Ok(())
    }

    fn apply_cooldown(
        &self,
        action: &ProviderAccountOperatorAction,
        observed_at_ms: i64,
    ) -> Result<ProviderAccountOperatorReceipt, ProviderAccountPoolError> {
        let cooldown_ms = action
            .cooldown_ms
            .filter(|value| {
                (MIN_PROVIDER_ACCOUNT_COOLDOWN_MS..=MAX_PROVIDER_ACCOUNT_COOLDOWN_MS)
                    .contains(value)
            })
            .ok_or(ProviderAccountPoolError::InvalidAction)?;
        let until_ms = observed_at_ms
            .checked_add(cooldown_ms)
            .ok_or(ProviderAccountPoolError::SourceUnavailable)?;
        let key = match action.upstream_model.clone() {
            Some(model) => RuntimeHealthKey::endpoint_credential_model(
                action.channel_id.clone(),
                action.account_id.clone(),
                model,
            ),
            None => RuntimeHealthKey::endpoint_credential(
                action.channel_id.clone(),
                action.account_id.clone(),
            ),
        };
        self.runtime_health
            .cool_down_until(key.clone(), until_ms)
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
        let state = self
            .runtime_health
            .availability_at(&key, observed_at_ms)
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
        let cooldown_until_ms = match state {
            RuntimeHealthAvailability::CoolingDown { until_ms } if until_ms >= observed_at_ms => {
                Some(until_ms)
            }
            _ => None,
        };
        let receipt_state = if cooldown_until_ms.is_some() {
            ProviderAccountOperatorState::Cooling
        } else {
            ProviderAccountOperatorState::Rejected
        };
        Ok(operator_receipt(
            receipt_state,
            observed_at_ms,
            cooldown_until_ms,
        ))
    }

    fn next_snapshot_id(&self, observed_at_ms: i64) -> Result<String, ProviderAccountPoolError> {
        let generation = self
            .next_snapshot_generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
        Ok(format!(
            "runtime-account-pools:{}:{observed_at_ms}:{generation}",
            self.instance_nonce
        ))
    }

    fn refresh_snapshot(
        &self,
        observed_at_ms: i64,
    ) -> Result<CachedSnapshot, ProviderAccountPoolError> {
        let snapshot_id = self.next_snapshot_id(observed_at_ms)?;
        let items = self.compile_items(observed_at_ms)?;
        let snapshot = ProviderAccountPoolSnapshot::try_new(snapshot_id, observed_at_ms, items)
            .map_err(|_| ProviderAccountPoolError::InvalidSnapshot)?;
        let fresh_until_ms = observed_at_ms
            .checked_add(self.ttl_ms)
            .ok_or(ProviderAccountPoolError::SourceUnavailable)?;
        let retain_until_ms = observed_at_ms
            .checked_add(self.cursor_retention_ms)
            .ok_or(ProviderAccountPoolError::SourceUnavailable)?;
        Ok(CachedSnapshot {
            snapshot,
            fresh_until_ms,
            retain_until_ms,
        })
    }

    fn compile_items(
        &self,
        observed_at_ms: i64,
    ) -> Result<Vec<ProviderAccountPoolItem>, ProviderAccountPoolError> {
        let mut diagnostics_by_endpoint: BTreeMap<
            EndpointId,
            BTreeMap<CredentialId, CredentialPoolEntrySnapshot>,
        > = BTreeMap::new();
        let mut items = Vec::with_capacity(self.descriptors.len());
        for descriptor in self.descriptors.iter() {
            let endpoint_diagnostics = diagnostics_by_endpoint
                .entry(descriptor.channel_id.clone())
                .or_insert_with(|| {
                    self.credential_pools
                        .pool(&descriptor.channel_id)
                        .map_or_else(BTreeMap::new, |pool| {
                            pool.diagnostic_entries()
                                .into_iter()
                                .map(|entry| (entry.credential_id().clone(), entry))
                                .collect()
                        })
                });
            let diagnostic = endpoint_diagnostics.get(&descriptor.account_id);
            let (priority, weight, max_concurrency, active_leases) = if let Some(entry) = diagnostic
            {
                let weight = u32::try_from(entry.weight())
                    .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
                let max_concurrency = u32::try_from(entry.maximum_concurrency())
                    .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
                let active_leases = u32::try_from(entry.active_leases())
                    .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
                if entry.credential_kind() != descriptor.account_kind
                    || entry.priority() != descriptor.priority
                    || weight != descriptor.weight
                    || max_concurrency != descriptor.max_concurrency
                {
                    return Err(ProviderAccountPoolError::SourceUnavailable);
                }
                (entry.priority(), weight, max_concurrency, active_leases)
            } else {
                if descriptor.enabled
                    && descriptor.auth_status == ProviderAccountAuthStatus::Active
                    && descriptor.runtime_status_hint == ProviderAccountRuntimeStatus::Available
                {
                    return Err(ProviderAccountPoolError::SourceUnavailable);
                }
                (
                    descriptor.priority,
                    descriptor.weight,
                    descriptor.max_concurrency,
                    0,
                )
            };
            let expires_at_ms = diagnostic
                .and_then(CredentialPoolEntrySnapshot::expires_at_ms)
                .or(descriptor.expires_at_ms);
            let auth_status = effective_auth_status(descriptor, expires_at_ms, observed_at_ms);
            let runtime_status = self.runtime_status(descriptor, auth_status, observed_at_ms)?;
            items.push(ProviderAccountPoolItem {
                provider_id: descriptor.provider_id.clone(),
                channel_id: descriptor.channel_id.clone(),
                account_id: descriptor.account_id.clone(),
                account_kind: descriptor.account_kind.clone(),
                auth_status,
                runtime_status,
                enabled: descriptor.enabled,
                priority,
                weight,
                max_concurrency,
                active_leases,
                expires_at_ms,
                refresh_due_at_ms: descriptor.refresh_due_at_ms,
                quota_sync_due_at_ms: descriptor.quota_sync_due_at_ms,
            });
        }
        Ok(items)
    }

    fn runtime_status(
        &self,
        descriptor: &ProviderAccountDescriptor,
        auth_status: ProviderAccountAuthStatus,
        observed_at_ms: i64,
    ) -> Result<ProviderAccountRuntimeStatus, ProviderAccountPoolError> {
        let mut status = merge_runtime_status(
            descriptor.runtime_status_hint,
            match auth_status {
                ProviderAccountAuthStatus::Expired => ProviderAccountRuntimeStatus::Expired,
                ProviderAccountAuthStatus::ReauthRequired => {
                    ProviderAccountRuntimeStatus::Unauthorized
                }
                ProviderAccountAuthStatus::Active | ProviderAccountAuthStatus::Disabled => {
                    ProviderAccountRuntimeStatus::Available
                }
            },
        );
        if status == ProviderAccountRuntimeStatus::Expired {
            return Ok(status);
        }

        let endpoint_key = RuntimeHealthKey::endpoint(descriptor.channel_id.clone());
        status = merge_runtime_status(
            status,
            health_status(
                self.runtime_health
                    .availability_at(&endpoint_key, observed_at_ms)
                    .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?,
            ),
        );
        let binding_key = RuntimeHealthKey::endpoint_credential(
            descriptor.channel_id.clone(),
            descriptor.account_id.clone(),
        );
        status = merge_runtime_status(
            status,
            health_status(
                self.runtime_health
                    .availability_at(&binding_key, observed_at_ms)
                    .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?,
            ),
        );
        status = merge_runtime_status(
            status,
            quota_status(
                self.runtime_quota
                    .status_at(
                        &RuntimeQuotaTarget::endpoint_credential(
                            descriptor.channel_id.clone(),
                            descriptor.account_id.clone(),
                        ),
                        observed_at_ms,
                    )
                    .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?
                    .availability(),
            ),
        );

        for model in &descriptor.upstream_models {
            let model_key = RuntimeHealthKey::endpoint_credential_model(
                descriptor.channel_id.clone(),
                descriptor.account_id.clone(),
                model,
            );
            status = merge_runtime_status(
                status,
                health_status(
                    self.runtime_health
                        .availability_at(&model_key, observed_at_ms)
                        .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?,
                ),
            );
            let quota_target = RuntimeQuotaTarget::endpoint_credential_model(
                descriptor.channel_id.clone(),
                descriptor.account_id.clone(),
                model,
            )
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
            status = merge_runtime_status(
                status,
                quota_status(
                    self.runtime_quota
                        .status_at(&quota_target, observed_at_ms)
                        .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?
                        .availability(),
                ),
            );
        }
        Ok(status)
    }
}

impl ProviderAccountPoolFacade for ProviderAccountPoolAdapter {
    fn list_provider_account_pools(
        &self,
        query: &ProviderAccountPoolQuery,
    ) -> Result<ProviderAccountPoolPage, ProviderAccountPoolError> {
        let observed_at_ms = self
            .clock
            .now_ms()
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
        if observed_at_ms < 0 {
            return Err(ProviderAccountPoolError::SourceUnavailable);
        }
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| ProviderAccountPoolError::SourceUnavailable)?;
        cache
            .retained
            .retain(|cached| cached.retain_until_ms > observed_at_ms);
        if cache
            .current
            .as_ref()
            .is_some_and(|cached| cached.retain_until_ms <= observed_at_ms)
        {
            cache.current = None;
        }

        if let Some(cursor) = query.cursor.as_ref() {
            if let Some(current) = cache
                .current
                .as_ref()
                .filter(|cached| cached.snapshot.snapshot_id() == cursor.snapshot_id())
            {
                return current.snapshot.page(query);
            }
            if let Some(retained) = cache
                .retained
                .iter()
                .find(|cached| cached.snapshot.snapshot_id() == cursor.snapshot_id())
            {
                return retained.snapshot.page(query);
            }
        }

        let needs_refresh = cache
            .current
            .as_ref()
            .is_none_or(|cached| cached.fresh_until_ms <= observed_at_ms);
        if needs_refresh {
            if let Some(previous) = cache.current.take()
                && previous.retain_until_ms > observed_at_ms
            {
                cache.retained.push_front(previous);
                cache
                    .retained
                    .truncate(MAX_RETAINED_PROVIDER_ACCOUNT_POOL_SNAPSHOTS);
            }
            cache.current = Some(self.refresh_snapshot(observed_at_ms)?);
        }
        cache
            .current
            .as_ref()
            .ok_or(ProviderAccountPoolError::SourceUnavailable)?
            .snapshot
            .page(query)
    }

    fn apply_operator_action(
        &self,
        action: &ProviderAccountOperatorAction,
        observed_at_ms: i64,
    ) -> Result<ProviderAccountOperatorReceipt, ProviderAccountPoolError> {
        if observed_at_ms < 0 {
            return Err(ProviderAccountPoolError::InvalidAction);
        }
        let descriptor = self.descriptor_for_action(action)?;
        ProviderAccountOperatorAction::try_new(
            action.config_version_id.clone(),
            action.provider_id.clone(),
            action.channel_id.clone(),
            action.account_id.clone(),
            action.upstream_model.clone(),
            action.kind,
            action.cooldown_ms,
        )?;
        if !descriptor.enabled
            || descriptor.auth_status != ProviderAccountAuthStatus::Active
            || descriptor
                .expires_at_ms
                .is_some_and(|expires_at_ms| expires_at_ms <= observed_at_ms)
        {
            return Ok(ProviderAccountOperatorReceipt {
                state: ProviderAccountOperatorState::Rejected,
                observed_at_ms,
                cooldown_until_ms: None,
            });
        }
        let receipt = match action.kind {
            ProviderAccountOperatorActionKind::CoolDown => {
                self.apply_cooldown(action, observed_at_ms)
            }
            ProviderAccountOperatorActionKind::RequestRecovery => {
                self.apply_recovery(action, observed_at_ms)
            }
        }?;
        if matches!(
            receipt.state,
            ProviderAccountOperatorState::Cooling | ProviderAccountOperatorState::ProbeScheduled
        ) {
            self.invalidate_current_snapshot(observed_at_ms)?;
        }
        Ok(receipt)
    }
}

const fn operator_receipt(
    state: ProviderAccountOperatorState,
    observed_at_ms: i64,
    cooldown_until_ms: Option<i64>,
) -> ProviderAccountOperatorReceipt {
    ProviderAccountOperatorReceipt {
        state,
        observed_at_ms,
        cooldown_until_ms,
    }
}

fn valid_opaque_id(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= MAX_PROVIDER_ACCOUNT_ID_CHARS
}

fn random_instance_nonce() -> Result<String, ProviderAccountPoolAdapterBuildError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0_u8; SNAPSHOT_INSTANCE_NONCE_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|_| ProviderAccountPoolAdapterBuildError::EntropyUnavailable)?;
    let mut encoded = String::with_capacity(SNAPSHOT_INSTANCE_NONCE_BYTES * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

fn effective_auth_status(
    descriptor: &ProviderAccountDescriptor,
    expires_at_ms: Option<i64>,
    observed_at_ms: i64,
) -> ProviderAccountAuthStatus {
    if descriptor.auth_status == ProviderAccountAuthStatus::Disabled {
        return ProviderAccountAuthStatus::Disabled;
    }
    if descriptor.auth_status == ProviderAccountAuthStatus::Expired
        || expires_at_ms.is_some_and(|expires_at_ms| expires_at_ms <= observed_at_ms)
    {
        ProviderAccountAuthStatus::Expired
    } else {
        descriptor.auth_status
    }
}

fn health_status(availability: RuntimeHealthAvailability) -> ProviderAccountRuntimeStatus {
    match availability {
        RuntimeHealthAvailability::Available => ProviderAccountRuntimeStatus::Available,
        RuntimeHealthAvailability::CoolingDown { .. } => ProviderAccountRuntimeStatus::Cooling,
        RuntimeHealthAvailability::CircuitOpen { .. } => ProviderAccountRuntimeStatus::CircuitOpen,
        RuntimeHealthAvailability::AccountForbidden
        | RuntimeHealthAvailability::CredentialUnauthorized => {
            ProviderAccountRuntimeStatus::Unauthorized
        }
        RuntimeHealthAvailability::AccountRecoveryInFlight { .. } => {
            ProviderAccountRuntimeStatus::RecoveryInFlight
        }
    }
}

fn quota_status(availability: RuntimeQuotaAvailability) -> ProviderAccountRuntimeStatus {
    match availability {
        RuntimeQuotaAvailability::Available => ProviderAccountRuntimeStatus::Available,
        RuntimeQuotaAvailability::Exhausted { .. }
        | RuntimeQuotaAvailability::RecoveryRequired { .. } => {
            ProviderAccountRuntimeStatus::QuotaBlocked
        }
        RuntimeQuotaAvailability::RecoveryProbeInFlight { .. } => {
            ProviderAccountRuntimeStatus::RecoveryInFlight
        }
    }
}

fn merge_runtime_status(
    current: ProviderAccountRuntimeStatus,
    observed: ProviderAccountRuntimeStatus,
) -> ProviderAccountRuntimeStatus {
    if runtime_status_rank(observed) > runtime_status_rank(current) {
        observed
    } else {
        current
    }
}

fn runtime_status_rank(status: ProviderAccountRuntimeStatus) -> u8 {
    match status {
        ProviderAccountRuntimeStatus::Available => 0,
        ProviderAccountRuntimeStatus::QuotaBlocked => 1,
        ProviderAccountRuntimeStatus::Cooling => 2,
        ProviderAccountRuntimeStatus::CircuitOpen => 3,
        ProviderAccountRuntimeStatus::Unauthorized => 4,
        ProviderAccountRuntimeStatus::RecoveryInFlight => 5,
        ProviderAccountRuntimeStatus::Expired => 6,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use gateway_router::{RuntimeHealthClock, RuntimeHealthClockError};
    use std::sync::atomic::AtomicI64;

    #[derive(Debug)]
    struct TestClock {
        now_ms: AtomicI64,
    }

    impl TestClock {
        fn new(now_ms: i64) -> Self {
            Self {
                now_ms: AtomicI64::new(now_ms),
            }
        }

        fn set(&self, now_ms: i64) {
            self.now_ms.store(now_ms, Ordering::Release);
        }
    }

    impl ProviderAccountPoolClock for TestClock {
        fn now_ms(&self) -> Result<i64, ProviderAccountPoolClockError> {
            Ok(self.now_ms.load(Ordering::Acquire))
        }
    }

    impl RuntimeHealthClock for TestClock {
        fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
            Ok(self.now_ms.load(Ordering::Acquire))
        }
    }

    fn descriptor(provider: &str, channel: &str, account: &str) -> ProviderAccountDescriptor {
        ProviderAccountDescriptor {
            source: ProviderAccountDescriptorSource::Ordinary,
            provider_id: ProviderId::try_new(provider).expect("provider"),
            channel_id: EndpointId::try_new(channel).expect("channel"),
            account_id: CredentialId::try_new(account).expect("account"),
            account_kind: "bearer".to_owned(),
            auth_status: ProviderAccountAuthStatus::Active,
            runtime_status_hint: ProviderAccountRuntimeStatus::Available,
            enabled: true,
            priority: 0,
            weight: 1,
            max_concurrency: 2,
            expires_at_ms: None,
            refresh_due_at_ms: None,
            quota_sync_due_at_ms: None,
            upstream_models: vec!["model-a".to_owned()],
        }
    }

    fn pools(
        descriptors: &[ProviderAccountDescriptor],
    ) -> Result<Arc<EndpointCredentialPools>, Box<dyn std::error::Error>> {
        let mut by_endpoint: BTreeMap<EndpointId, Vec<EndpointCredentialInput>> = BTreeMap::new();
        for descriptor in descriptors {
            by_endpoint
                .entry(descriptor.channel_id.clone())
                .or_default()
                .push(EndpointCredentialInput {
                    credential_id: descriptor.account_id.clone(),
                    credential_kind: descriptor.account_kind.clone(),
                    credential_revision: 1,
                    priority: descriptor.priority,
                    weight: i64::from(descriptor.weight),
                    concurrency: i64::from(descriptor.max_concurrency),
                    expires_at_ms: descriptor.expires_at_ms,
                    secret: CredentialSecret::try_new(b"test-secret".to_vec())?,
                });
        }
        let endpoint_pools = by_endpoint
            .into_iter()
            .map(|(endpoint, entries)| EndpointCredentialPool::try_new(endpoint, entries))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Arc::new(EndpointCredentialPools::try_new(endpoint_pools)?))
    }

    fn query(limit: usize) -> ProviderAccountPoolQuery {
        ProviderAccountPoolQuery::try_new(None, None, None, None, None, limit, None).expect("query")
    }

    fn assert_runtime_status(
        states: &BTreeMap<String, ProviderAccountPoolItem>,
        account_id: &str,
        expected: ProviderAccountRuntimeStatus,
    ) {
        assert_eq!(states[account_id].runtime_status, expected);
    }

    fn assert_projected_states(states: &BTreeMap<String, ProviderAccountPoolItem>) {
        assert_runtime_status(states, "available", ProviderAccountRuntimeStatus::Available);
        assert_runtime_status(states, "cooling", ProviderAccountRuntimeStatus::Cooling);
        assert_runtime_status(states, "circuit", ProviderAccountRuntimeStatus::CircuitOpen);
        assert_runtime_status(
            states,
            "unauthorized",
            ProviderAccountRuntimeStatus::Unauthorized,
        );
        assert_runtime_status(states, "quota", ProviderAccountRuntimeStatus::QuotaBlocked);
        assert_runtime_status(
            states,
            "recovery",
            ProviderAccountRuntimeStatus::RecoveryInFlight,
        );
        assert_eq!(
            states["expired"].auth_status,
            ProviderAccountAuthStatus::Expired
        );
        assert_runtime_status(states, "expired", ProviderAccountRuntimeStatus::Expired);
        assert_eq!(
            states["disabled"].auth_status,
            ProviderAccountAuthStatus::Disabled
        );
        assert_runtime_status(states, "disabled", ProviderAccountRuntimeStatus::Available);
    }

    #[test]
    fn projects_health_quota_auth_expiry_and_disabled_states()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        let endpoint = EndpointId::try_new("channel")?;
        let available = descriptor("provider-a", "channel", "available");
        let cooling = descriptor("provider-a", "channel", "cooling");
        let circuit = descriptor("provider-a", "channel", "circuit");
        let unauthorized = descriptor("provider-a", "channel", "unauthorized");
        let quota = descriptor("provider-a", "channel", "quota");
        let recovery = descriptor("provider-a", "channel", "recovery");
        let mut expired = descriptor("provider-a", "channel", "expired");
        let mut disabled = descriptor("provider-a", "channel", "disabled");
        expired.expires_at_ms = Some(100);
        disabled.auth_status = ProviderAccountAuthStatus::Disabled;
        disabled.enabled = false;
        let descriptors = vec![
            available.clone(),
            cooling.clone(),
            circuit.clone(),
            unauthorized.clone(),
            quota.clone(),
            recovery.clone(),
            expired.clone(),
            disabled.clone(),
        ];
        let health = Arc::new(RuntimeHealthRegistry::with_clock(clock.clone()));
        health.cool_down_until(
            RuntimeHealthKey::endpoint_credential(endpoint.clone(), cooling.account_id.clone()),
            500,
        )?;
        health.open_circuit_until(
            RuntimeHealthKey::endpoint_credential(endpoint.clone(), circuit.account_id.clone()),
            500,
        )?;
        health.mark_credential_unauthorized(endpoint.clone(), unauthorized.account_id.clone())?;
        health.mark_credential_unauthorized(endpoint.clone(), recovery.account_id.clone())?;
        let _ticket = health
            .begin_account_recovery(&endpoint, &recovery.account_id, 500)?
            .expect("recovery ticket");
        let quota_registry = Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone()));
        quota_registry.record_rate_limited(
            RuntimeQuotaTarget::endpoint_credential(endpoint.clone(), quota.account_id.clone()),
            100,
            Some(Duration::from_millis(400)),
            Duration::from_millis(400),
        )?;
        let adapter = ProviderAccountPoolAdapter::try_new(
            descriptors,
            pools(&[
                available,
                cooling,
                circuit,
                unauthorized,
                quota,
                recovery,
                expired,
                disabled,
            ])?,
            health,
            quota_registry,
            clock,
            Duration::from_secs(10),
            Duration::from_secs(20),
        )?;
        let page = adapter.list_provider_account_pools(&query(20))?;
        let states = page
            .items
            .into_iter()
            .map(|item| (item.account_id.to_string(), item))
            .collect::<BTreeMap<_, _>>();
        assert_projected_states(&states);
        Ok(())
    }

    #[test]
    fn preserves_lease_counts_and_provider_isolation() -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        let mut first = descriptor("provider-a", "channel-a", "account-a");
        let second = descriptor("provider-a", "channel-a", "account-b");
        let other = descriptor("provider-b", "channel-b", "account-a");
        first.max_concurrency = 3;
        let descriptors = vec![first.clone(), second.clone(), other.clone()];
        let pools = pools(&descriptors)?;
        let lease = pools
            .pool(&first.channel_id)
            .expect("channel pool")
            .try_lease()
            .expect("lease");
        let adapter = ProviderAccountPoolAdapter::try_new(
            descriptors,
            pools,
            Arc::new(RuntimeHealthRegistry::with_clock(clock.clone())),
            Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone())),
            clock,
            Duration::from_secs(10),
            Duration::from_secs(20),
        )?;
        let page = adapter.list_provider_account_pools(&query(20))?;
        let account_a = page
            .items
            .iter()
            .find(|item| {
                item.channel_id.as_str() == "channel-a" && item.account_id.as_str() == "account-a"
            })
            .expect("account-a");
        assert_eq!(account_a.active_leases, 1);
        assert_eq!(account_a.max_concurrency, 3);
        drop(lease);
        let provider_query = ProviderAccountPoolQuery::try_new(
            Some(ProviderId::try_new("provider-b")?),
            None,
            None,
            None,
            None,
            20,
            None,
        )?;
        let filtered = adapter.list_provider_account_pools(&provider_query)?;
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.items[0].provider_id.as_str(), "provider-b");
        Ok(())
    }

    #[test]
    fn retains_cursor_snapshot_after_refresh_and_rejects_it_after_retention()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        let descriptors = (0..120)
            .map(|index| descriptor("provider-a", "channel", &format!("account-{index:03}")))
            .collect::<Vec<_>>();
        let adapter = ProviderAccountPoolAdapter::try_new(
            descriptors.clone(),
            pools(&descriptors)?,
            Arc::new(RuntimeHealthRegistry::with_clock(clock.clone())),
            Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone())),
            clock.clone(),
            Duration::from_millis(50),
            Duration::from_millis(500),
        )?;
        let first = adapter.list_provider_account_pools(&query(50))?;
        assert_eq!(first.items.len(), 50);
        let cursor = first.next_cursor.clone().expect("next cursor");
        let second_query = ProviderAccountPoolQuery::try_new(
            None,
            None,
            None,
            None,
            None,
            50,
            Some(cursor.clone()),
        )?;
        clock.set(151);
        let refreshed = adapter.list_provider_account_pools(&query(50))?;
        assert_ne!(refreshed.snapshot_id, first.snapshot_id);
        let second = adapter.list_provider_account_pools(&second_query)?;
        assert_eq!(second.snapshot_id, first.snapshot_id);
        assert_eq!(second.items.len(), 50);
        assert_eq!(second.items[0].account_id.as_str(), "account-050");
        let third_query = ProviderAccountPoolQuery::try_new(
            None,
            None,
            None,
            None,
            None,
            50,
            second.next_cursor,
        )?;
        let third = adapter.list_provider_account_pools(&third_query)?;
        assert_eq!(third.snapshot_id, first.snapshot_id);
        assert_eq!(third.items.len(), 20);
        assert_eq!(third.items[0].account_id.as_str(), "account-100");
        assert!(third.next_cursor.is_none());
        clock.set(601);
        assert_eq!(
            adapter.list_provider_account_pools(&second_query),
            Err(ProviderAccountPoolError::CursorConflict)
        );
        Ok(())
    }

    #[test]
    fn runtime_cooldown_expires_against_the_live_observation_clock()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        let account = descriptor("provider-a", "channel", "account");
        let health = Arc::new(RuntimeHealthRegistry::with_clock(clock.clone()));
        health.cool_down_until(
            RuntimeHealthKey::endpoint_credential(
                account.channel_id.clone(),
                account.account_id.clone(),
            ),
            200,
        )?;
        let adapter = ProviderAccountPoolAdapter::try_new(
            vec![account.clone()],
            pools(&[account])?,
            health,
            Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone())),
            clock.clone(),
            Duration::from_millis(50),
            Duration::from_millis(500),
        )?;
        let cooling = adapter.list_provider_account_pools(&query(10))?;
        assert_eq!(
            cooling.items[0].runtime_status,
            ProviderAccountRuntimeStatus::Cooling
        );

        clock.set(201);
        let available = adapter.list_provider_account_pools(&query(10))?;
        assert_eq!(
            available.items[0].runtime_status,
            ProviderAccountRuntimeStatus::Available
        );
        Ok(())
    }

    #[test]
    fn active_available_descriptor_requires_an_exact_compiled_pool_entry()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        let active = descriptor("provider-a", "channel", "account");
        let empty_pools = Arc::new(EndpointCredentialPools::try_new(Vec::new())?);
        let missing = ProviderAccountPoolAdapter::try_new(
            vec![active.clone()],
            Arc::clone(&empty_pools),
            Arc::new(RuntimeHealthRegistry::with_clock(clock.clone())),
            Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone())),
            clock.clone(),
            Duration::from_secs(1),
            Duration::from_secs(2),
        )?;
        assert_eq!(
            missing.list_provider_account_pools(&query(10)),
            Err(ProviderAccountPoolError::SourceUnavailable)
        );

        let mut mismatched = active.clone();
        mismatched.weight = 2;
        let drifted = ProviderAccountPoolAdapter::try_new(
            vec![active],
            pools(&[mismatched])?,
            Arc::new(RuntimeHealthRegistry::with_clock(clock.clone())),
            Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone())),
            clock.clone(),
            Duration::from_secs(1),
            Duration::from_secs(2),
        )?;
        assert_eq!(
            drifted.list_provider_account_pools(&query(10)),
            Err(ProviderAccountPoolError::SourceUnavailable)
        );

        let mut disabled = descriptor("provider-a", "channel", "disabled");
        disabled.auth_status = ProviderAccountAuthStatus::Disabled;
        disabled.enabled = false;
        let retained_metadata = ProviderAccountPoolAdapter::try_new(
            vec![disabled],
            empty_pools,
            Arc::new(RuntimeHealthRegistry::with_clock(clock.clone())),
            Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone())),
            clock,
            Duration::from_secs(1),
            Duration::from_secs(2),
        )?;
        assert_eq!(
            retained_metadata
                .list_provider_account_pools(&query(10))?
                .items[0]
                .auth_status,
            ProviderAccountAuthStatus::Disabled
        );
        Ok(())
    }

    #[test]
    fn snapshot_namespace_is_unique_across_adapters_at_the_same_millisecond()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        let descriptors = vec![
            descriptor("provider-a", "channel", "account-a"),
            descriptor("provider-a", "channel", "account-b"),
        ];
        let pools = pools(&descriptors)?;
        let build = || {
            ProviderAccountPoolAdapter::try_new(
                descriptors.clone(),
                Arc::clone(&pools),
                Arc::new(RuntimeHealthRegistry::with_clock(clock.clone())),
                Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone())),
                clock.clone(),
                Duration::from_secs(1),
                Duration::from_secs(2),
            )
        };
        let first_adapter = build()?;
        let second_adapter = build()?;
        let first = first_adapter.list_provider_account_pools(&query(1))?;
        let second = second_adapter.list_provider_account_pools(&query(1))?;
        assert_ne!(first.snapshot_id, second.snapshot_id);
        let old_cursor =
            ProviderAccountPoolQuery::try_new(None, None, None, None, None, 1, first.next_cursor)?;
        assert_eq!(
            second_adapter.list_provider_account_pools(&old_cursor),
            Err(ProviderAccountPoolError::CursorConflict)
        );
        Ok(())
    }

    #[test]
    fn exact_model_health_and_quota_are_projected_without_affecting_a_sibling()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        let health_account = descriptor("provider-a", "channel", "health");
        let quota_account = descriptor("provider-a", "channel", "quota");
        let sibling = descriptor("provider-a", "channel", "sibling");
        let descriptors = vec![health_account.clone(), quota_account.clone(), sibling];
        let health = Arc::new(RuntimeHealthRegistry::with_clock(clock.clone()));
        health.open_circuit_until(
            RuntimeHealthKey::endpoint_credential_model(
                health_account.channel_id.clone(),
                health_account.account_id.clone(),
                "model-a",
            ),
            500,
        )?;
        let quota = Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone()));
        quota.record_rate_limited(
            RuntimeQuotaTarget::endpoint_credential_model(
                quota_account.channel_id.clone(),
                quota_account.account_id.clone(),
                "model-a",
            )?,
            100,
            Some(Duration::from_millis(400)),
            Duration::from_millis(400),
        )?;
        let adapter = ProviderAccountPoolAdapter::try_new(
            descriptors.clone(),
            pools(&descriptors)?,
            health,
            quota,
            clock,
            Duration::from_secs(1),
            Duration::from_secs(2),
        )?;
        let states = adapter
            .list_provider_account_pools(&query(10))?
            .items
            .into_iter()
            .map(|item| (item.account_id.to_string(), item.runtime_status))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(states["health"], ProviderAccountRuntimeStatus::CircuitOpen);
        assert_eq!(states["quota"], ProviderAccountRuntimeStatus::QuotaBlocked);
        assert_eq!(states["sibling"], ProviderAccountRuntimeStatus::Available);
        Ok(())
    }

    #[test]
    fn operator_cooldown_is_version_bound_and_exact_to_one_model()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        let target = descriptor("provider-a", "channel", "target");
        let mut sibling = descriptor("provider-a", "channel", "sibling");
        sibling.auth_status = ProviderAccountAuthStatus::Disabled;
        sibling.enabled = false;
        let descriptors = vec![target.clone(), sibling.clone()];
        let health = Arc::new(RuntimeHealthRegistry::with_clock(clock.clone()));
        let adapter = ProviderAccountPoolAdapter::try_new(
            descriptors.clone(),
            pools(&descriptors)?,
            Arc::clone(&health),
            Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone())),
            clock,
            Duration::from_secs(1),
            Duration::from_secs(2),
        )?
        .with_config_version("config-v1".to_owned())?;
        let before = adapter.list_provider_account_pools(&query(10))?;
        let action = ProviderAccountOperatorAction::try_new(
            "config-v1",
            target.provider_id.clone(),
            target.channel_id.clone(),
            target.account_id.clone(),
            Some("model-a".to_owned()),
            ProviderAccountOperatorActionKind::CoolDown,
            Some(MIN_PROVIDER_ACCOUNT_COOLDOWN_MS),
        )?;
        let receipt = adapter.apply_operator_action(&action, 100)?;
        assert_eq!(receipt.state, ProviderAccountOperatorState::Cooling);
        assert_eq!(receipt.cooldown_until_ms, Some(1_100));
        let after = adapter.list_provider_account_pools(&query(10))?;
        assert_ne!(after.snapshot_id, before.snapshot_id);
        assert_eq!(
            after
                .items
                .iter()
                .find(|item| item.account_id == target.account_id)
                .expect("target row")
                .runtime_status,
            ProviderAccountRuntimeStatus::Cooling
        );
        assert!(matches!(
            health.availability_at(
                &RuntimeHealthKey::endpoint_credential_model(
                    target.channel_id.clone(),
                    target.account_id.clone(),
                    "model-a",
                ),
                100,
            )?,
            RuntimeHealthAvailability::CoolingDown { until_ms: 1_100 }
        ));
        assert_eq!(
            health.availability_at(
                &RuntimeHealthKey::endpoint_credential(
                    target.channel_id.clone(),
                    target.account_id.clone(),
                ),
                100,
            )?,
            RuntimeHealthAvailability::Available
        );
        assert_eq!(
            health.availability_at(
                &RuntimeHealthKey::endpoint_credential_model(
                    sibling.channel_id.clone(),
                    sibling.account_id.clone(),
                    "model-a",
                ),
                100,
            )?,
            RuntimeHealthAvailability::Available
        );

        Ok(())
    }

    #[test]
    fn operator_action_rejects_stale_unknown_model_and_disabled_targets()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        let target = descriptor("provider-a", "channel", "target");
        let mut disabled_target = descriptor("provider-a", "channel", "disabled");
        disabled_target.auth_status = ProviderAccountAuthStatus::Disabled;
        disabled_target.enabled = false;
        let descriptors = vec![target.clone(), disabled_target.clone()];
        let adapter = ProviderAccountPoolAdapter::try_new(
            descriptors.clone(),
            pools(&descriptors)?,
            Arc::new(RuntimeHealthRegistry::with_clock(clock.clone())),
            Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone())),
            clock,
            Duration::from_secs(1),
            Duration::from_secs(2),
        )?
        .with_config_version("config-v1".to_owned())?;
        let stale = ProviderAccountOperatorAction::try_new(
            "config-v2",
            target.provider_id.clone(),
            target.channel_id.clone(),
            target.account_id.clone(),
            None,
            ProviderAccountOperatorActionKind::CoolDown,
            Some(MIN_PROVIDER_ACCOUNT_COOLDOWN_MS),
        )?;
        assert_eq!(
            adapter.apply_operator_action(&stale, 100),
            Err(ProviderAccountPoolError::ActionTargetUnavailable)
        );
        let unknown_model = ProviderAccountOperatorAction::try_new(
            "config-v1",
            target.provider_id,
            target.channel_id,
            target.account_id,
            Some("model-not-bound".to_owned()),
            ProviderAccountOperatorActionKind::CoolDown,
            Some(MIN_PROVIDER_ACCOUNT_COOLDOWN_MS),
        )?;
        assert_eq!(
            adapter.apply_operator_action(&unknown_model, 100),
            Err(ProviderAccountPoolError::ActionTargetUnavailable)
        );
        let disabled = ProviderAccountOperatorAction::try_new(
            "config-v1",
            disabled_target.provider_id,
            disabled_target.channel_id,
            disabled_target.account_id,
            None,
            ProviderAccountOperatorActionKind::RequestRecovery,
            None,
        )?;
        assert_eq!(
            adapter.apply_operator_action(&disabled, 100)?.state,
            ProviderAccountOperatorState::Rejected
        );
        Ok(())
    }

    #[test]
    fn operator_recovery_obeys_existing_quota_reset_state_machine()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        let target = descriptor("provider-a", "channel", "target");
        let quota = Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone()));
        let quota_target = RuntimeQuotaTarget::endpoint_credential(
            target.channel_id.clone(),
            target.account_id.clone(),
        );
        quota.record_rate_limited(
            quota_target.clone(),
            100,
            Some(Duration::from_millis(50)),
            Duration::from_millis(50),
        )?;
        let adapter = ProviderAccountPoolAdapter::try_new(
            vec![target.clone()],
            pools(std::slice::from_ref(&target))?,
            Arc::new(RuntimeHealthRegistry::with_clock(clock.clone())),
            Arc::clone(&quota),
            clock.clone(),
            Duration::from_secs(1),
            Duration::from_secs(2),
        )?
        .with_config_version("config-v1".to_owned())?;
        let action = ProviderAccountOperatorAction::try_new(
            "config-v1",
            target.provider_id,
            target.channel_id,
            target.account_id,
            None,
            ProviderAccountOperatorActionKind::RequestRecovery,
            None,
        )?;
        let still_blocked = adapter.apply_operator_action(&action, 100)?;
        assert_eq!(
            still_blocked.state,
            ProviderAccountOperatorState::RecoveryRequired
        );

        clock.set(151);
        let recovered = adapter.apply_operator_action(&action, 151)?;
        assert_eq!(
            recovered.state,
            ProviderAccountOperatorState::ProbeScheduled
        );
        assert_eq!(
            quota.availability_at(&quota_target, 151)?,
            RuntimeQuotaAvailability::Available
        );
        Ok(())
    }

    #[test]
    fn descriptor_rejects_overlong_or_blank_opaque_ids() -> Result<(), Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::new(100));
        for provider_id in [
            " ".to_owned(),
            "x".repeat(MAX_PROVIDER_ACCOUNT_ID_CHARS + 1),
        ] {
            let mut invalid = descriptor("provider-a", "channel", "account");
            invalid.provider_id = ProviderId::try_new(provider_id)?;
            assert!(matches!(
                ProviderAccountPoolAdapter::try_new(
                    vec![invalid],
                    Arc::new(EndpointCredentialPools::try_new(Vec::new())?),
                    Arc::new(RuntimeHealthRegistry::with_clock(clock.clone())),
                    Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone())),
                    clock.clone(),
                    Duration::from_secs(1),
                    Duration::from_secs(2),
                ),
                Err(ProviderAccountPoolAdapterBuildError::InvalidDescriptor)
            ));
        }
        Ok(())
    }
}
