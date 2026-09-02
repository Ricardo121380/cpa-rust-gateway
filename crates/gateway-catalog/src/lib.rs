//! Immutable model Catalog and Endpoint-capability evidence for control-plane compilation.
//!
//! P2-06 keeps the domain types storage-neutral and explicitly injected. P4-01 owns exact
//! Endpoint/Credential discovery singleflight; P4-02 owns snapshot freshness/diffs, while
//! P13-15C adds the explicitly constructed `SQLite` last-success repository used by production.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::{Arc, Mutex as StdMutex, MutexGuard},
};

use gateway_core::{CredentialId, EndpointId, ErrorScope, GatewayError, GatewayErrorCode};
use gateway_provider::{ProviderAdapter, ProviderFuture};
use tokio::sync::{Mutex, watch};

mod durable;

pub use durable::{
    CatalogDiscoveryFailureClass, DurableCatalogError, DurableCatalogFailureStatus,
    DurableCatalogModel, DurableCatalogSnapshotStatus, SqliteCatalogSnapshotStore,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-catalog";

/// One validated model value returned by a Provider Catalog source before snapshot freshness is
/// assigned.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiscoveredModel {
    upstream_model: String,
}

impl DiscoveredModel {
    /// Creates one non-empty upstream model value.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::EmptyUpstreamModel`] when `upstream_model` is empty.
    pub fn try_new(upstream_model: impl Into<String>) -> Result<Self, CatalogViewError> {
        let upstream_model = upstream_model.into();
        if upstream_model.is_empty() {
            return Err(CatalogViewError::EmptyUpstreamModel);
        }
        Ok(Self { upstream_model })
    }

    /// Returns the exact source-provided upstream model identity.
    #[must_use]
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }
}

/// The one discovery identity that may share an in-flight model lookup.
///
/// It contains stable Endpoint and Credential identifiers only. Concrete Providers keep endpoint
/// address and credential material on their own side of the source boundary.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ModelCatalogTarget {
    endpoint_id: EndpointId,
    credential_id: CredentialId,
}

impl ModelCatalogTarget {
    /// Creates one exact Endpoint/Credential discovery identity.
    #[must_use]
    pub const fn new(endpoint_id: EndpointId, credential_id: CredentialId) -> Self {
        Self {
            endpoint_id,
            credential_id,
        }
    }

    /// Returns the Endpoint portion of this discovery identity.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the Credential portion of this discovery identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }
}

/// Provider-owned source for one Endpoint/Credential model discovery request.
///
/// The source may perform Provider-specific discovery, but it must not turn an Endpoint-wide
/// result into a Credential-wide entitlement. The scheduler below shares only calls with the exact
/// same [`ModelCatalogTarget`].
pub trait ModelCatalogSource: ProviderAdapter {
    /// Discovers models for one exact Endpoint/Credential identity.
    fn models(
        &self,
        target: ModelCatalogTarget,
    ) -> ProviderFuture<'_, Result<Vec<DiscoveredModel>, GatewayError>>;
}

type ModelCatalogResult = Result<Vec<DiscoveredModel>, GatewayError>;

struct InFlightModelCatalogSync {
    result: watch::Sender<Option<ModelCatalogResult>>,
}

/// Asynchronous per-Endpoint/Credential discovery scheduler with singleflight sharing.
///
/// One scheduler owns one Provider source. The first caller for a target starts one detached
/// discovery task; concurrent callers for the same exact target await its shared result. Different
/// Credentials, even on the same Endpoint, always receive independent source calls. The background
/// task continues if an initiating caller is cancelled so remaining followers cannot be stranded.
pub struct ModelCatalogScheduler {
    source: Arc<dyn ModelCatalogSource>,
    in_flight: Arc<Mutex<BTreeMap<ModelCatalogTarget, Arc<InFlightModelCatalogSync>>>>,
}

impl ModelCatalogScheduler {
    /// Creates a scheduler for one Provider-owned Catalog source.
    #[must_use]
    pub fn new(source: Arc<dyn ModelCatalogSource>) -> Self {
        Self {
            source,
            in_flight: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Starts or joins one discovery operation for `target`.
    ///
    /// Source output is sorted and deduplicated before every caller receives it. Source failures
    /// are shared verbatim as the existing safe [`GatewayError`] value and are not cached after the
    /// in-flight operation completes.
    ///
    /// # Errors
    ///
    /// Returns the safe source-provided error for this exact in-flight discovery, or an internal
    /// error if its detached task exits without publishing a result.
    pub async fn synchronize(
        &self,
        target: ModelCatalogTarget,
    ) -> Result<Vec<DiscoveredModel>, GatewayError> {
        let receiver = {
            let mut in_flight = self.in_flight.lock().await;
            if let Some(flight) = in_flight.get(&target) {
                flight.result.subscribe()
            } else {
                let (sender, receiver) = watch::channel::<Option<ModelCatalogResult>>(None);
                let flight = Arc::new(InFlightModelCatalogSync { result: sender });
                in_flight.insert(target.clone(), Arc::clone(&flight));
                Self::spawn_discovery(
                    Arc::clone(&self.source),
                    target,
                    Arc::clone(&self.in_flight),
                    flight,
                );
                receiver
            }
        };

        wait_for_result(receiver).await
    }

    fn spawn_discovery(
        source: Arc<dyn ModelCatalogSource>,
        target: ModelCatalogTarget,
        in_flight: Arc<Mutex<BTreeMap<ModelCatalogTarget, Arc<InFlightModelCatalogSync>>>>,
        flight: Arc<InFlightModelCatalogSync>,
    ) {
        let task = tokio::spawn(async move {
            let result = source.models(target.clone()).await.map(normalize_models);

            let mut in_flight = in_flight.lock().await;
            if in_flight
                .get(&target)
                .is_some_and(|current| Arc::ptr_eq(current, &flight))
            {
                in_flight.remove(&target);
            }
            drop(in_flight);

            // Remove the completed flight before publishing its result. Existing subscribers still
            // receive it, while a caller that arrives after completion starts a fresh discovery
            // instead of observing a result cache owned by this P4-01 scheduler.
            flight.result.send_replace(Some(result));
        });
        drop(task);
    }
}

async fn wait_for_result(
    mut receiver: watch::Receiver<Option<ModelCatalogResult>>,
) -> ModelCatalogResult {
    loop {
        if let Some(result) = receiver.borrow_and_update().as_ref().cloned() {
            return result;
        }
        if receiver.changed().await.is_err() {
            return Err(internal_error());
        }
    }
}

fn normalize_models(mut models: Vec<DiscoveredModel>) -> Vec<DiscoveredModel> {
    models.sort_unstable();
    models.dedup();
    models
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

/// Default period for which a successfully discovered Catalog is current.
pub const DEFAULT_CATALOG_FRESH_FOR_MS: i64 = 6 * 60 * 60 * 1_000;
/// Default interval after a success at which a background refresh is due.
pub const DEFAULT_CATALOG_REFRESH_DUE_AFTER_MS: i64 = 24 * 60 * 60 * 1_000;
/// Default maximum retention period for a last-success Catalog.
pub const DEFAULT_CATALOG_EXPIRES_AFTER_MS: i64 = 72 * 60 * 60 * 1_000;

/// Validated timing boundaries for discovery-backed Catalog snapshots.
///
/// `fresh_for_ms` is the `Fresh` period. `refresh_due_after_ms` is a background scheduling
/// deadline, not an additional visible state: a snapshot remains `Stale` after its Fresh period
/// until it expires. This makes the architecture's default `Fresh 6h / Stale 24h / Expired 72h`
/// explicit without silently treating the 24-hour refresh target as early expiry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogFreshnessPolicy {
    fresh_for: i64,
    refresh_due_after: i64,
    expires_after: i64,
}

impl Default for CatalogFreshnessPolicy {
    fn default() -> Self {
        Self {
            fresh_for: DEFAULT_CATALOG_FRESH_FOR_MS,
            refresh_due_after: DEFAULT_CATALOG_REFRESH_DUE_AFTER_MS,
            expires_after: DEFAULT_CATALOG_EXPIRES_AFTER_MS,
        }
    }
}

impl CatalogFreshnessPolicy {
    /// Creates one ordered, positive snapshot-timing policy.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogSnapshotError`] when any duration is non-positive or its ordering would
    /// make a snapshot deadline ambiguous.
    pub fn try_new(
        fresh_for_ms: i64,
        refresh_due_after_ms: i64,
        expires_after_ms: i64,
    ) -> Result<Self, CatalogSnapshotError> {
        if fresh_for_ms <= 0 {
            return Err(CatalogSnapshotError::FreshDurationNotPositive);
        }
        if refresh_due_after_ms < fresh_for_ms {
            return Err(CatalogSnapshotError::RefreshDueBeforeFresh);
        }
        if expires_after_ms <= refresh_due_after_ms {
            return Err(CatalogSnapshotError::ExpiryNotAfterRefreshDue);
        }
        Ok(Self {
            fresh_for: fresh_for_ms,
            refresh_due_after: refresh_due_after_ms,
            expires_after: expires_after_ms,
        })
    }

    /// Returns the duration for which a success is `Fresh`.
    #[must_use]
    pub const fn fresh_for_ms(self) -> i64 {
        self.fresh_for
    }

    /// Returns the deadline after which background refresh work is due.
    #[must_use]
    pub const fn refresh_due_after_ms(self) -> i64 {
        self.refresh_due_after
    }

    /// Returns the maximum retention duration for a last-success snapshot.
    #[must_use]
    pub const fn expires_after_ms(self) -> i64 {
        self.expires_after
    }
}

/// Visible freshness of a discovery-backed Catalog snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogSnapshotFreshness {
    /// The last successful discovery is within its Fresh window.
    Fresh,
    /// The last successful discovery is retained after its Fresh window.
    Stale,
    /// The retained discovery is beyond its hard expiry and must not be treated as eligible.
    Expired,
}

impl CatalogSnapshotFreshness {
    /// Returns whether this snapshot can remain eligible without a later explicit exception.
    #[must_use]
    pub const fn is_hard_eligible(self) -> bool {
        matches!(self, Self::Fresh | Self::Stale)
    }
}

/// Immutable successful discovery result for one exact Endpoint/Credential target.
///
/// The snapshot stores only stable identifiers, normalized discovered Model names, monotonically
/// increasing version data, and explicit Unix-millisecond deadlines. It deliberately does not
/// contain Endpoint URLs, Credential material, failure diagnostics, static allowlists, or a diff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogSnapshot {
    target: ModelCatalogTarget,
    models: Vec<DiscoveredModel>,
    version: u64,
    observed_at_ms: i64,
    stale_at_ms: i64,
    refresh_due_at_ms: i64,
    expires_at_ms: i64,
}

impl CatalogSnapshot {
    /// Creates one immutable, normalized successful discovery snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogSnapshotError`] for a zero version, a pre-epoch timestamp, or a timestamp
    /// whose freshness deadlines cannot be represented safely. An empty model list is valid: it is
    /// a successful source result and P4-03 owns its later removal semantics.
    pub fn try_new(
        target: ModelCatalogTarget,
        models: impl IntoIterator<Item = DiscoveredModel>,
        version: u64,
        observed_at_ms: i64,
        policy: CatalogFreshnessPolicy,
    ) -> Result<Self, CatalogSnapshotError> {
        if version == 0 {
            return Err(CatalogSnapshotError::SnapshotVersionZero);
        }
        if observed_at_ms < 0 {
            return Err(CatalogSnapshotError::TimestampBeforeUnixEpoch);
        }
        let stale_at_ms = checked_deadline(observed_at_ms, policy.fresh_for)?;
        let refresh_due_at_ms = checked_deadline(observed_at_ms, policy.refresh_due_after)?;
        let expires_at_ms = checked_deadline(observed_at_ms, policy.expires_after)?;
        let models = normalize_models(models.into_iter().collect());

        Ok(Self {
            target,
            models,
            version,
            observed_at_ms,
            stale_at_ms,
            refresh_due_at_ms,
            expires_at_ms,
        })
    }

    /// Returns the exact Endpoint/Credential identity that owns this success.
    #[must_use]
    pub fn target(&self) -> &ModelCatalogTarget {
        &self.target
    }

    /// Returns source Model names in deterministic sorted, deduplicated order.
    #[must_use]
    pub fn models(&self) -> &[DiscoveredModel] {
        &self.models
    }

    /// Returns the target-local, monotonically increasing successful snapshot version.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Returns the Unix-millisecond instant at which this success was observed.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Returns the first Unix-millisecond instant at which this snapshot is `Stale`.
    #[must_use]
    pub const fn stale_at_ms(&self) -> i64 {
        self.stale_at_ms
    }

    /// Returns the first Unix-millisecond instant at which refresh work is due.
    #[must_use]
    pub const fn refresh_due_at_ms(&self) -> i64 {
        self.refresh_due_at_ms
    }

    /// Returns the first Unix-millisecond instant at which this snapshot is `Expired`.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }

    /// Computes freshness using an explicit Unix-millisecond timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogSnapshotError::ClockBeforeSnapshot`] when the caller gives a timestamp
    /// before this success. This rejects a non-monotonic observation instead of making a future
    /// snapshot look Fresh retroactively.
    pub fn freshness_at(
        &self,
        now_ms: i64,
    ) -> Result<CatalogSnapshotFreshness, CatalogSnapshotError> {
        self.validate_now_ms(now_ms)?;
        if now_ms < self.stale_at_ms {
            Ok(CatalogSnapshotFreshness::Fresh)
        } else if now_ms < self.expires_at_ms {
            Ok(CatalogSnapshotFreshness::Stale)
        } else {
            Ok(CatalogSnapshotFreshness::Expired)
        }
    }

    /// Returns whether background refresh work is due at an explicit timestamp.
    ///
    /// # Errors
    ///
    /// Returns the same non-monotonic-clock error as [`Self::freshness_at`].
    pub fn is_refresh_due_at(&self, now_ms: i64) -> Result<bool, CatalogSnapshotError> {
        self.validate_now_ms(now_ms)?;
        Ok(now_ms >= self.refresh_due_at_ms)
    }

    fn validate_now_ms(&self, now_ms: i64) -> Result<(), CatalogSnapshotError> {
        if now_ms < self.observed_at_ms {
            return Err(CatalogSnapshotError::ClockBeforeSnapshot);
        }
        Ok(())
    }
}

/// A snapshot plus its freshness at one explicit observation time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogSnapshotStatus {
    snapshot: CatalogSnapshot,
    freshness: CatalogSnapshotFreshness,
    refresh_due: bool,
}

impl CatalogSnapshotStatus {
    pub(crate) fn at(snapshot: CatalogSnapshot, now_ms: i64) -> Result<Self, CatalogSnapshotError> {
        let freshness = snapshot.freshness_at(now_ms)?;
        let refresh_due = snapshot.is_refresh_due_at(now_ms)?;
        Ok(Self {
            snapshot,
            freshness,
            refresh_due,
        })
    }

    /// Returns the immutable last-success snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &CatalogSnapshot {
        &self.snapshot
    }

    /// Returns the visible Fresh/Stale/Expired state at the requested timestamp.
    #[must_use]
    pub const fn freshness(&self) -> CatalogSnapshotFreshness {
        self.freshness
    }

    /// Returns whether the independent 24-hour background refresh deadline has elapsed.
    #[must_use]
    pub const fn is_refresh_due(&self) -> bool {
        self.refresh_due
    }
}

/// Process-local last-success snapshot registry keyed by exact Catalog target.
///
/// A short control-plane mutex makes successful replacement atomic. It never joins Models across
/// Credentials: a successful discovery can replace exactly its own `(EndpointId, CredentialId)`
/// key, while failure retrieval leaves every retained snapshot unchanged.
pub struct CatalogSnapshotStore {
    policy: CatalogFreshnessPolicy,
    snapshots: StdMutex<BTreeMap<ModelCatalogTarget, CatalogSnapshot>>,
}

impl Default for CatalogSnapshotStore {
    fn default() -> Self {
        Self::new(CatalogFreshnessPolicy::default())
    }
}

impl CatalogSnapshotStore {
    /// Creates an empty process-local registry with one already validated timing policy.
    #[must_use]
    pub fn new(policy: CatalogFreshnessPolicy) -> Self {
        Self {
            policy,
            snapshots: StdMutex::new(BTreeMap::new()),
        }
    }

    /// Returns this registry's immutable freshness policy.
    #[must_use]
    pub const fn policy(&self) -> CatalogFreshnessPolicy {
        self.policy
    }

    /// Atomically accepts one successful discovery for its exact target.
    ///
    /// The target's version starts at one and increases only after a later non-decreasing success.
    /// Any construction error leaves the prior snapshot untouched.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogSnapshotError`] for an unavailable registry, unsafe timestamp, timestamp
    /// overflow, version exhaustion, or a success whose observation time is earlier than the
    /// target's current last success.
    pub fn record_success(
        &self,
        target: ModelCatalogTarget,
        models: impl IntoIterator<Item = DiscoveredModel>,
        observed_at_ms: i64,
    ) -> Result<CatalogSnapshot, CatalogSnapshotError> {
        // Consume caller-provided iteration before taking the registry lock. This prevents an
        // arbitrary iterator from extending the snapshot replacement critical section.
        let models: Vec<_> = models.into_iter().collect();
        let mut snapshots = self.lock_snapshots()?;
        let version = match snapshots.get(&target) {
            Some(previous) => {
                if observed_at_ms < previous.observed_at_ms {
                    return Err(CatalogSnapshotError::TimestampNotMonotonic);
                }
                previous
                    .version
                    .checked_add(1)
                    .ok_or(CatalogSnapshotError::SnapshotVersionOverflow)?
            }
            None => 1,
        };
        let snapshot =
            CatalogSnapshot::try_new(target.clone(), models, version, observed_at_ms, self.policy)?;
        snapshots.insert(target, snapshot.clone());
        Ok(snapshot)
    }

    /// Returns the retained last success after a discovery failure without mutating the registry.
    ///
    /// The caller may record a failure Run in a later observability/persistence component, but this
    /// P4-02 boundary deliberately stores no failure payload or diagnostic and cannot overwrite
    /// source success evidence with a failed attempt.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogSnapshotError::StoreLockPoisoned`] if the process-local registry cannot
    /// be read safely.
    pub fn retain_last_success_on_failure(
        &self,
        target: &ModelCatalogTarget,
    ) -> Result<Option<CatalogSnapshot>, CatalogSnapshotError> {
        Ok(self.lock_snapshots()?.get(target).cloned())
    }

    /// Returns the unclassified last success for one exact target.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogSnapshotError::StoreLockPoisoned`] if the registry cannot be read safely.
    pub fn last_success(
        &self,
        target: &ModelCatalogTarget,
    ) -> Result<Option<CatalogSnapshot>, CatalogSnapshotError> {
        Ok(self.lock_snapshots()?.get(target).cloned())
    }

    /// Returns the retained snapshot together with Fresh/Stale/Expired at an explicit timestamp.
    ///
    /// # Errors
    ///
    /// Returns a safe registry or timestamp error. An absent exact target is not an error.
    pub fn status_at(
        &self,
        target: &ModelCatalogTarget,
        now_ms: i64,
    ) -> Result<Option<CatalogSnapshotStatus>, CatalogSnapshotError> {
        self.last_success(target)?
            .map(|snapshot| CatalogSnapshotStatus::at(snapshot, now_ms))
            .transpose()
    }

    fn lock_snapshots(
        &self,
    ) -> Result<MutexGuard<'_, BTreeMap<ModelCatalogTarget, CatalogSnapshot>>, CatalogSnapshotError>
    {
        self.snapshots
            .lock()
            .map_err(|_| CatalogSnapshotError::StoreLockPoisoned)
    }
}

/// Safe construction, timestamp, and process-local registry failures for Catalog snapshots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogSnapshotError {
    /// The Fresh duration was zero or negative.
    FreshDurationNotPositive,
    /// The refresh deadline was before the Fresh window ended.
    RefreshDueBeforeFresh,
    /// The hard expiry was not strictly after the refresh deadline.
    ExpiryNotAfterRefreshDue,
    /// A snapshot version must begin at one.
    SnapshotVersionZero,
    /// A later successful snapshot could not advance the finite version counter.
    SnapshotVersionOverflow,
    /// A timestamp was before the Unix epoch and outside this snapshot domain.
    TimestampBeforeUnixEpoch,
    /// Adding a validated duration to an observation time overflowed Unix milliseconds.
    TimestampOverflow,
    /// A caller evaluated a snapshot before it was observed.
    ClockBeforeSnapshot,
    /// A later success was older than the target's retained last success.
    TimestampNotMonotonic,
    /// The process-local registry was poisoned by a prior panic and therefore fails closed.
    StoreLockPoisoned,
}

impl fmt::Display for CatalogSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::FreshDurationNotPositive => "Catalog Fresh duration must be positive",
            Self::RefreshDueBeforeFresh => "Catalog refresh deadline must not precede Fresh expiry",
            Self::ExpiryNotAfterRefreshDue => {
                "Catalog hard expiry must be after the refresh deadline"
            }
            Self::SnapshotVersionZero => "Catalog snapshot version must start at one",
            Self::SnapshotVersionOverflow => "Catalog snapshot version cannot advance safely",
            Self::TimestampBeforeUnixEpoch => "Catalog timestamp is before the Unix epoch",
            Self::TimestampOverflow => "Catalog timestamp deadline cannot be represented safely",
            Self::ClockBeforeSnapshot => "Catalog clock precedes the retained snapshot",
            Self::TimestampNotMonotonic => {
                "Catalog success timestamp precedes the retained last success"
            }
            Self::StoreLockPoisoned => "Catalog snapshot registry is unavailable",
        };
        formatter.write_str(message)
    }
}

impl Error for CatalogSnapshotError {}

fn checked_deadline(observed_at_ms: i64, duration_ms: i64) -> Result<i64, CatalogSnapshotError> {
    observed_at_ms
        .checked_add(duration_ms)
        .ok_or(CatalogSnapshotError::TimestampOverflow)
}

/// Minimum number of consecutive successful discovery absences before a model may be removed.
pub const MIN_CATALOG_SUCCESSFUL_MISSES_FOR_REMOVAL: u64 = 3;
/// Minimum time a model must remain missing before removal is eligible.
pub const MIN_CATALOG_REMOVAL_ISOLATION_MS: i64 = 24 * 60 * 60 * 1_000;

/// One externally visible, target-local result of comparing a successful Catalog snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatalogDiffEvent {
    /// A source model has not appeared in the accepted target-local Catalog before.
    Added {
        /// Exact source-provided model identity, retaining its original case.
        model: DiscoveredModel,
    },
    /// A previously accepted model was absent from a successful discovery but remains retained.
    SuspectedRemoved {
        /// Exact source-provided model identity.
        model: DiscoveredModel,
        /// Number of consecutive successful snapshots that omitted this model.
        consecutive_successful_misses: u64,
        /// Unix-millisecond time of the first successful absence in this sequence.
        first_missing_at_ms: i64,
        /// Unix-millisecond time at which time isolation for removal is satisfied.
        removal_eligible_at_ms: i64,
    },
    /// A model has reached both the successful-miss and isolation requirements for removal.
    Removed {
        /// Exact source-provided model identity.
        model: DiscoveredModel,
        /// Number of consecutive successful snapshots that omitted this model.
        consecutive_successful_misses: u64,
        /// Unix-millisecond time of the first successful absence in this sequence.
        first_missing_at_ms: i64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CatalogDiffModelState {
    Present,
    SuspectedRemoved {
        consecutive_successful_misses: u64,
        first_missing_at_ms: i64,
        removal_eligible_at_ms: i64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CatalogDiffTargetState {
    generation: u64,
    last_snapshot_version: u64,
    last_observed_at_ms: i64,
    models: BTreeMap<DiscoveredModel, CatalogDiffModelState>,
}

/// Immutable, non-mutating removal plan for one exact successful Catalog snapshot.
///
/// A preview contains the target-local generation on which it was based. It can be inspected or
/// discarded freely, but [`CatalogDiffRegistry::apply`] accepts it only while that generation is
/// still current. This prevents an older preview from overwriting a later applied discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogDiffPreview {
    target: ModelCatalogTarget,
    snapshot_version: u64,
    base_generation: u64,
    events: Vec<CatalogDiffEvent>,
    next_state: CatalogDiffTargetState,
}

impl CatalogDiffPreview {
    /// Returns the exact Endpoint/Credential target represented by this plan.
    #[must_use]
    pub fn target(&self) -> &ModelCatalogTarget {
        &self.target
    }

    /// Returns the successful Catalog snapshot version compared by this plan.
    #[must_use]
    pub const fn snapshot_version(&self) -> u64 {
        self.snapshot_version
    }

    /// Returns stable, target-local diff events in deterministic model order.
    #[must_use]
    pub fn events(&self) -> &[CatalogDiffEvent] {
        &self.events
    }
}

/// Immutable result of applying one target-local Catalog diff preview.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogDiffApplyResult {
    target: ModelCatalogTarget,
    snapshot_version: u64,
    generation: u64,
    events: Vec<CatalogDiffEvent>,
}

impl CatalogDiffApplyResult {
    /// Returns the exact target whose diff state was atomically updated.
    #[must_use]
    pub fn target(&self) -> &ModelCatalogTarget {
        &self.target
    }

    /// Returns the successful Catalog snapshot version now reflected by this registry.
    #[must_use]
    pub const fn snapshot_version(&self) -> u64 {
        self.snapshot_version
    }

    /// Returns the target-local generation after this apply operation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the events actually applied in deterministic model order.
    #[must_use]
    pub fn events(&self) -> &[CatalogDiffEvent] {
        &self.events
    }
}

/// Process-local Preview/Apply registry for target-local Catalog removal evidence.
///
/// Every state key is the exact [`ModelCatalogTarget`] from P4-01/P4-02. Only callers that present
/// a successful [`CatalogSnapshot`] may produce a preview, so a discovery failure cannot increment
/// a missing counter or clear a retained model. Static/manual Catalog records are outside this
/// discovery-only registry and therefore cannot be removed by it.
pub struct CatalogDiffRegistry {
    targets: StdMutex<BTreeMap<ModelCatalogTarget, CatalogDiffTargetState>>,
}

impl Default for CatalogDiffRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CatalogDiffRegistry {
    /// Creates an empty, process-local target-isolated Catalog diff registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            targets: StdMutex::new(BTreeMap::new()),
        }
    }

    /// Compares a successful snapshot without mutating retained diff state.
    ///
    /// The first accepted snapshot emits `Added` for each source model. Later successful snapshots
    /// emit `SuspectedRemoved` for every consecutive absence and `Removed` only after at least
    /// [`MIN_CATALOG_SUCCESSFUL_MISSES_FOR_REMOVAL`] successful absences plus
    /// [`MIN_CATALOG_REMOVAL_ISOLATION_MS`] of isolation.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogDiffError`] when the registry is unavailable, the supplied snapshot is not
    /// newer than the target-local applied snapshot, its observation time regresses, or a finite
    /// counter/deadline cannot advance safely.
    pub fn preview(
        &self,
        snapshot: &CatalogSnapshot,
    ) -> Result<CatalogDiffPreview, CatalogDiffError> {
        let previous = self.lock_targets()?.get(snapshot.target()).cloned();
        validate_diff_snapshot(previous.as_ref(), snapshot)?;
        let base_generation = previous.as_ref().map_or(0, |state| state.generation);
        let (models, events) = build_catalog_diff(previous.as_ref(), snapshot)?;

        Ok(CatalogDiffPreview {
            target: snapshot.target().clone(),
            snapshot_version: snapshot.version(),
            base_generation,
            events,
            next_state: CatalogDiffTargetState {
                generation: base_generation,
                last_snapshot_version: snapshot.version(),
                last_observed_at_ms: snapshot.observed_at_ms(),
                models,
            },
        })
    }

    /// Atomically applies one current preview for its exact target.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogDiffError::StalePreview`] when another preview for the target has already
    /// changed its generation. The stale plan is not partially applied.
    pub fn apply(
        &self,
        preview: CatalogDiffPreview,
    ) -> Result<CatalogDiffApplyResult, CatalogDiffError> {
        let CatalogDiffPreview {
            target,
            snapshot_version,
            base_generation,
            events,
            mut next_state,
        } = preview;
        let mut targets = self.lock_targets()?;
        let current_generation = targets.get(&target).map_or(0, |state| state.generation);
        if current_generation != base_generation {
            return Err(CatalogDiffError::StalePreview);
        }
        let generation = current_generation
            .checked_add(1)
            .ok_or(CatalogDiffError::GenerationOverflow)?;
        next_state.generation = generation;
        targets.insert(target.clone(), next_state);

        Ok(CatalogDiffApplyResult {
            target,
            snapshot_version,
            generation,
            events,
        })
    }

    fn lock_targets(
        &self,
    ) -> Result<
        MutexGuard<'_, BTreeMap<ModelCatalogTarget, CatalogDiffTargetState>>,
        CatalogDiffError,
    > {
        self.targets
            .lock()
            .map_err(|_| CatalogDiffError::RegistryLockPoisoned)
    }
}

/// Safe Preview/Apply failures for discovery-backed Catalog diffs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogDiffError {
    /// The incoming successful snapshot version was not strictly newer than the applied version.
    SnapshotVersionNotNewer,
    /// The incoming successful snapshot observation time regressed for this exact target.
    SnapshotObservedAtNotMonotonic,
    /// Another preview was applied after this preview was created.
    StalePreview,
    /// The finite target-local apply generation cannot advance safely.
    GenerationOverflow,
    /// A finite missing-count counter cannot advance safely.
    ConsecutiveMissingOverflow,
    /// The 24-hour removal-isolation deadline cannot be represented safely.
    RemovalDeadlineOverflow,
    /// The process-local diff registry was poisoned by a prior panic and fails closed.
    RegistryLockPoisoned,
}

impl fmt::Display for CatalogDiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::SnapshotVersionNotNewer => {
                "Catalog diff snapshot version is not newer than the applied snapshot"
            }
            Self::SnapshotObservedAtNotMonotonic => {
                "Catalog diff snapshot observation time regressed"
            }
            Self::StalePreview => "Catalog diff preview is stale",
            Self::GenerationOverflow => "Catalog diff generation cannot advance safely",
            Self::ConsecutiveMissingOverflow => {
                "Catalog diff consecutive-missing counter cannot advance safely"
            }
            Self::RemovalDeadlineOverflow => {
                "Catalog diff removal-isolation deadline cannot be represented safely"
            }
            Self::RegistryLockPoisoned => "Catalog diff registry is unavailable",
        };
        formatter.write_str(message)
    }
}

impl Error for CatalogDiffError {}

fn validate_diff_snapshot(
    previous: Option<&CatalogDiffTargetState>,
    snapshot: &CatalogSnapshot,
) -> Result<(), CatalogDiffError> {
    if let Some(previous) = previous {
        if snapshot.version() <= previous.last_snapshot_version {
            return Err(CatalogDiffError::SnapshotVersionNotNewer);
        }
        if snapshot.observed_at_ms() < previous.last_observed_at_ms {
            return Err(CatalogDiffError::SnapshotObservedAtNotMonotonic);
        }
    }
    Ok(())
}

fn build_catalog_diff(
    previous: Option<&CatalogDiffTargetState>,
    snapshot: &CatalogSnapshot,
) -> Result<
    (
        BTreeMap<DiscoveredModel, CatalogDiffModelState>,
        Vec<CatalogDiffEvent>,
    ),
    CatalogDiffError,
> {
    let source_models = snapshot.models().iter().cloned().collect::<BTreeSet<_>>();
    let mut models = previous.map_or_else(BTreeMap::new, |state| state.models.clone());
    let mut events = Vec::new();

    for model in &source_models {
        if models
            .insert(model.clone(), CatalogDiffModelState::Present)
            .is_none()
        {
            events.push(CatalogDiffEvent::Added {
                model: model.clone(),
            });
        }
    }

    if let Some(previous) = previous {
        for (model, state) in &previous.models {
            if source_models.contains(model) {
                continue;
            }
            match state {
                CatalogDiffModelState::Present => {
                    let first_missing_at_ms = snapshot.observed_at_ms();
                    let removal_eligible_at_ms = first_missing_at_ms
                        .checked_add(MIN_CATALOG_REMOVAL_ISOLATION_MS)
                        .ok_or(CatalogDiffError::RemovalDeadlineOverflow)?;
                    models.insert(
                        model.clone(),
                        CatalogDiffModelState::SuspectedRemoved {
                            consecutive_successful_misses: 1,
                            first_missing_at_ms,
                            removal_eligible_at_ms,
                        },
                    );
                    events.push(CatalogDiffEvent::SuspectedRemoved {
                        model: model.clone(),
                        consecutive_successful_misses: 1,
                        first_missing_at_ms,
                        removal_eligible_at_ms,
                    });
                }
                CatalogDiffModelState::SuspectedRemoved {
                    consecutive_successful_misses,
                    first_missing_at_ms,
                    removal_eligible_at_ms,
                } => {
                    let next_misses = consecutive_successful_misses
                        .checked_add(1)
                        .ok_or(CatalogDiffError::ConsecutiveMissingOverflow)?;
                    if next_misses >= MIN_CATALOG_SUCCESSFUL_MISSES_FOR_REMOVAL
                        && snapshot.observed_at_ms() >= *removal_eligible_at_ms
                    {
                        models.remove(model);
                        events.push(CatalogDiffEvent::Removed {
                            model: model.clone(),
                            consecutive_successful_misses: next_misses,
                            first_missing_at_ms: *first_missing_at_ms,
                        });
                    } else {
                        models.insert(
                            model.clone(),
                            CatalogDiffModelState::SuspectedRemoved {
                                consecutive_successful_misses: next_misses,
                                first_missing_at_ms: *first_missing_at_ms,
                                removal_eligible_at_ms: *removal_eligible_at_ms,
                            },
                        );
                        events.push(CatalogDiffEvent::SuspectedRemoved {
                            model: model.clone(),
                            consecutive_successful_misses: next_misses,
                            first_missing_at_ms: *first_missing_at_ms,
                            removal_eligible_at_ms: *removal_eligible_at_ms,
                        });
                    }
                }
            }
        }
    }

    Ok((models, events))
}

/// One semantic capability relevant to public-model Route compilation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticCapability {
    /// Tool call support.
    Tools,
    /// Multiple independent Tool calls in one request/response.
    ParallelTools,
    /// Explicit Thinking or Reasoning support.
    Reasoning,
    /// JSON Schema response or Tool support.
    JsonSchema,
    /// Vision input support.
    Vision,
    /// Streaming response support.
    Streaming,
    /// Public `OpenAI` Responses WebSocket ingress may use this exact channel.
    ///
    /// This is a downstream transport capability. It does not claim that the Provider itself
    /// speaks WebSocket; the runtime may still project the bounded Canonical stream from HTTP/SSE.
    ResponsesWebSocket,
    /// Gateway-owned stored Response history may be replayed through this exact channel.
    StoredResponses,
    /// Gateway-owned Response compaction may execute through this exact channel.
    ResponseCompaction,
}

impl SemanticCapability {
    /// Returns the fixed configuration key used in P2-06 capability JSON.
    #[must_use]
    pub const fn json_key(self) -> &'static str {
        match self {
            Self::Tools => "tools",
            Self::ParallelTools => "parallel_tools",
            Self::Reasoning => "reasoning",
            Self::JsonSchema => "json_schema",
            Self::Vision => "vision",
            Self::Streaming => "streaming",
            Self::ResponsesWebSocket => "responses_websocket",
            Self::StoredResponses => "stored_responses",
            Self::ResponseCompaction => "response_compaction",
        }
    }

    /// Parses one fixed P2-06 configuration key.
    #[must_use]
    pub fn from_json_key(value: &str) -> Option<Self> {
        match value {
            "tools" => Some(Self::Tools),
            "parallel_tools" => Some(Self::ParallelTools),
            "reasoning" => Some(Self::Reasoning),
            "json_schema" => Some(Self::JsonSchema),
            "vision" => Some(Self::Vision),
            "streaming" => Some(Self::Streaming),
            "responses_websocket" => Some(Self::ResponsesWebSocket),
            "stored_responses" => Some(Self::StoredResponses),
            "response_compaction" => Some(Self::ResponseCompaction),
            _ => None,
        }
    }
}

/// A validated set of Endpoint or Route semantic capabilities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilitySet {
    capabilities: BTreeSet<SemanticCapability>,
}

impl CapabilitySet {
    /// Builds a capability set and enforces its intra-set invariants.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::ParallelToolsRequiresTools`] when `parallel_tools` is present
    /// without `tools`.
    pub fn try_new(
        capabilities: impl IntoIterator<Item = SemanticCapability>,
    ) -> Result<Self, CatalogViewError> {
        let capabilities = capabilities.into_iter().collect();
        let set = Self { capabilities };
        set.validate()?;
        Ok(set)
    }

    /// Returns an empty capability set.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            capabilities: BTreeSet::new(),
        }
    }

    /// Returns whether this set includes one capability.
    #[must_use]
    pub fn supports(&self, capability: SemanticCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Returns whether every capability in `required` is available in this set.
    #[must_use]
    pub fn supports_all(&self, required: &Self) -> bool {
        required.capabilities.is_subset(&self.capabilities)
    }

    /// Produces a narrowed set without ever adding a capability.
    ///
    /// Removing `tools` also removes `parallel_tools`, preserving the mandatory implication.
    #[must_use]
    pub fn without(&self, removed: impl IntoIterator<Item = SemanticCapability>) -> Self {
        let mut capabilities = self.capabilities.clone();
        for capability in removed {
            capabilities.remove(&capability);
            if capability == SemanticCapability::Tools {
                capabilities.remove(&SemanticCapability::ParallelTools);
            }
            if capability == SemanticCapability::StoredResponses {
                capabilities.remove(&SemanticCapability::ResponseCompaction);
            }
            if capability == SemanticCapability::Streaming {
                capabilities.remove(&SemanticCapability::ResponsesWebSocket);
            }
        }
        Self { capabilities }
    }

    /// Produces a widened set while revalidating all capability implications.
    ///
    /// This is reserved for an explicit configuration assertion that has already passed its
    /// adapter-specific admission check. Callers must not use it to infer Provider support.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError`] when the resulting set violates a capability implication.
    pub fn try_with(
        &self,
        added: impl IntoIterator<Item = SemanticCapability>,
    ) -> Result<Self, CatalogViewError> {
        let mut capabilities = self.capabilities.clone();
        capabilities.extend(added);
        let set = Self { capabilities };
        set.validate()?;
        Ok(set)
    }

    /// Iterates over supported capabilities in stable enum order.
    pub fn iter(&self) -> impl Iterator<Item = SemanticCapability> + '_ {
        self.capabilities.iter().copied()
    }

    fn validate(&self) -> Result<(), CatalogViewError> {
        if self.supports(SemanticCapability::ParallelTools)
            && !self.supports(SemanticCapability::Tools)
        {
            return Err(CatalogViewError::ParallelToolsRequiresTools);
        }
        if self.supports(SemanticCapability::ResponseCompaction)
            && !self.supports(SemanticCapability::StoredResponses)
        {
            return Err(CatalogViewError::ResponseCompactionRequiresStoredResponses);
        }
        if self.supports(SemanticCapability::ResponsesWebSocket)
            && !self.supports(SemanticCapability::Streaming)
        {
            return Err(CatalogViewError::ResponsesWebSocketRequiresStreaming);
        }
        Ok(())
    }
}

/// Freshness/provenance state of one `(Endpoint, upstream model)` Catalog record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogModelState {
    /// An explicit manual model allowlist entry.
    Manual,
    /// A current discovery-backed Catalog entry.
    Fresh,
    /// A retained last-success Catalog entry that has not expired.
    Stale,
    /// A Catalog entry no longer accepted without an explicit compiler exception.
    Expired,
}

impl CatalogModelState {
    /// Returns whether this state is hard-eligible without an explicit exception.
    #[must_use]
    pub const fn is_hard_eligible(self) -> bool {
        matches!(self, Self::Manual | Self::Fresh | Self::Stale)
    }
}

/// One storage-neutral model Catalog record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogModelEntry {
    /// Endpoint that exposes the model.
    pub endpoint_id: EndpointId,
    /// Exact non-empty upstream model string.
    pub upstream_model: String,
    /// Catalog freshness/provenance state.
    pub state: CatalogModelState,
}

impl CatalogModelEntry {
    /// Creates one model Catalog record.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::EmptyUpstreamModel`] when `upstream_model` is empty.
    pub fn try_new(
        endpoint_id: EndpointId,
        upstream_model: impl Into<String>,
        state: CatalogModelState,
    ) -> Result<Self, CatalogViewError> {
        let upstream_model = upstream_model.into();
        if upstream_model.is_empty() {
            return Err(CatalogViewError::EmptyUpstreamModel);
        }
        Ok(Self {
            endpoint_id,
            upstream_model,
            state,
        })
    }
}

/// Immutable lookup of model Catalog state by Endpoint and upstream model.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CatalogView {
    entries: BTreeMap<(EndpointId, String), CatalogModelState>,
}

impl CatalogView {
    /// Builds a Catalog lookup and rejects ambiguous duplicate records.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::DuplicateCatalogModel`] for duplicate endpoint/model pairs.
    pub fn try_new(
        entries: impl IntoIterator<Item = CatalogModelEntry>,
    ) -> Result<Self, CatalogViewError> {
        let mut view = Self::default();
        for entry in entries {
            let key = (entry.endpoint_id, entry.upstream_model);
            if view.entries.insert(key, entry.state).is_some() {
                return Err(CatalogViewError::DuplicateCatalogModel);
            }
        }
        Ok(view)
    }

    /// Returns the stored Catalog state for one exact Endpoint/model pair.
    #[must_use]
    pub fn model_state(
        &self,
        endpoint_id: &EndpointId,
        upstream_model: &str,
    ) -> Option<CatalogModelState> {
        self.entries
            .get(&(endpoint_id.clone(), upstream_model.to_owned()))
            .copied()
    }
}

/// One injected semantic-capability profile for a concrete Endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointCapabilityEntry {
    /// Endpoint whose profile is described.
    pub endpoint_id: EndpointId,
    /// All semantic capabilities supported by that Endpoint.
    pub capabilities: CapabilitySet,
}

/// Immutable lookup of semantic capabilities by Endpoint.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EndpointCapabilityView {
    entries: BTreeMap<EndpointId, CapabilitySet>,
}

impl EndpointCapabilityView {
    /// Builds an Endpoint capability lookup and rejects duplicate Endpoint profiles.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::DuplicateEndpointCapabilityProfile`] for duplicate Endpoint
    /// identities.
    pub fn try_new(
        entries: impl IntoIterator<Item = EndpointCapabilityEntry>,
    ) -> Result<Self, CatalogViewError> {
        let mut view = Self::default();
        for entry in entries {
            if view
                .entries
                .insert(entry.endpoint_id, entry.capabilities)
                .is_some()
            {
                return Err(CatalogViewError::DuplicateEndpointCapabilityProfile);
            }
        }
        Ok(view)
    }

    /// Returns the injected profile for one Endpoint.
    #[must_use]
    pub fn capabilities_for(&self, endpoint_id: &EndpointId) -> Option<&CapabilitySet> {
        self.entries.get(endpoint_id)
    }
}

/// Safe construction errors for injected Catalog/capability evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogViewError {
    /// A Catalog model string was empty.
    EmptyUpstreamModel,
    /// More than one Catalog record described the same Endpoint/model pair.
    DuplicateCatalogModel,
    /// More than one capability record described the same Endpoint.
    DuplicateEndpointCapabilityProfile,
    /// Parallel Tool support appeared without ordinary Tool support.
    ParallelToolsRequiresTools,
    /// Response compaction appeared without stored-response continuity.
    ResponseCompactionRequiresStoredResponses,
    /// Responses WebSocket ingress appeared without streaming support.
    ResponsesWebSocketRequiresStreaming,
}

impl fmt::Display for CatalogViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyUpstreamModel => {
                formatter.write_str("Catalog upstream model must not be empty")
            }
            Self::DuplicateCatalogModel => {
                formatter.write_str("Catalog contains a duplicate Endpoint/model record")
            }
            Self::DuplicateEndpointCapabilityProfile => formatter
                .write_str("Endpoint capability view contains a duplicate Endpoint profile"),
            Self::ParallelToolsRequiresTools => {
                formatter.write_str("parallel Tool capability requires Tool capability")
            }
            Self::ResponseCompactionRequiresStoredResponses => {
                formatter.write_str("Response compaction requires stored-response continuity")
            }
            Self::ResponsesWebSocketRequiresStreaming => {
                formatter.write_str("Responses WebSocket requires streaming capability")
            }
        }
    }
}

impl Error for CatalogViewError {}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use gateway_core::{
        CredentialId, EndpointId, ErrorScope, GatewayError, GatewayErrorCode, ProviderId,
    };
    use gateway_provider::{ProviderAdapter, ProviderFuture};
    use tokio::{
        sync::Notify,
        task::yield_now,
        time::{error::Elapsed, timeout},
    };

    use super::{
        CapabilitySet, CatalogDiffError, CatalogDiffEvent, CatalogDiffRegistry,
        CatalogFreshnessPolicy, CatalogModelEntry, CatalogModelState, CatalogSnapshot,
        CatalogSnapshotError, CatalogSnapshotFreshness, CatalogSnapshotStore, CatalogView,
        CatalogViewError, DEFAULT_CATALOG_EXPIRES_AFTER_MS, DEFAULT_CATALOG_FRESH_FOR_MS,
        DEFAULT_CATALOG_REFRESH_DUE_AFTER_MS, DiscoveredModel, EndpointCapabilityEntry,
        EndpointCapabilityView, MIN_CATALOG_REMOVAL_ISOLATION_MS, ModelCatalogScheduler,
        ModelCatalogSource, ModelCatalogTarget, SemanticCapability,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[derive(Debug)]
    struct BlockingCatalogSource {
        provider_id: ProviderId,
        calls: AtomicUsize,
        waiting: AtomicUsize,
        failures_remaining: AtomicUsize,
        started: Notify,
        ready_to_release: Notify,
        release: Notify,
    }

    impl BlockingCatalogSource {
        fn new(failures_remaining: usize) -> Result<Self, Box<dyn Error>> {
            Ok(Self {
                provider_id: ProviderId::try_new("catalog-test-provider")?,
                calls: AtomicUsize::new(0),
                waiting: AtomicUsize::new(0),
                failures_remaining: AtomicUsize::new(failures_remaining),
                started: Notify::new(),
                ready_to_release: Notify::new(),
                release: Notify::new(),
            })
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        async fn wait_for_started(&self) -> Result<(), Elapsed> {
            timeout(Duration::from_secs(1), self.started.notified()).await
        }

        async fn wait_until_waiting(&self, expected: usize) -> Result<(), Elapsed> {
            timeout(Duration::from_secs(1), async {
                while self.waiting.load(Ordering::SeqCst) < expected {
                    self.ready_to_release.notified().await;
                }
            })
            .await
        }
    }

    impl ProviderAdapter for BlockingCatalogSource {
        fn provider_id(&self) -> &ProviderId {
            &self.provider_id
        }
    }

    impl ModelCatalogSource for BlockingCatalogSource {
        fn models(
            &self,
            target: ModelCatalogTarget,
        ) -> ProviderFuture<'_, Result<Vec<DiscoveredModel>, GatewayError>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let should_fail = self
                .failures_remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok();
            let started = &self.started;
            let waiting = &self.waiting;
            let ready_to_release = &self.ready_to_release;
            let release = &self.release;

            Box::pin(async move {
                started.notify_one();
                waiting.fetch_add(1, Ordering::SeqCst);
                ready_to_release.notify_one();
                release.notified().await;

                if should_fail {
                    return Err(GatewayError::new(
                        GatewayErrorCode::ProviderTransient,
                        ErrorScope::Provider,
                    ));
                }

                let credential_model = DiscoveredModel::try_new(format!(
                    "credential-model-{}",
                    target.credential_id().as_str()
                ))
                .map_err(|_| {
                    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
                })?;
                let shared_model = DiscoveredModel::try_new("shared-model").map_err(|_| {
                    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
                })?;

                Ok(vec![
                    credential_model.clone(),
                    shared_model,
                    credential_model,
                ])
            })
        }
    }

    fn target(credential: &str) -> Result<ModelCatalogTarget, Box<dyn Error>> {
        Ok(ModelCatalogTarget::new(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new(credential)?,
        ))
    }

    fn model_names(models: &[DiscoveredModel]) -> Vec<&str> {
        models.iter().map(DiscoveredModel::upstream_model).collect()
    }

    fn discovered_models(names: &[&str]) -> Result<Vec<DiscoveredModel>, Box<dyn Error>> {
        names
            .iter()
            .map(|name| DiscoveredModel::try_new(*name).map_err(Into::into))
            .collect()
    }

    fn successful_snapshot(
        store: &CatalogSnapshotStore,
        catalog_target: &ModelCatalogTarget,
        names: &[&str],
        observed_at_ms: i64,
    ) -> Result<CatalogSnapshot, Box<dyn Error>> {
        Ok(store.record_success(
            catalog_target.clone(),
            discovered_models(names)?,
            observed_at_ms,
        )?)
    }

    async fn wait_for_receiver_count(
        scheduler: &ModelCatalogScheduler,
        target: &ModelCatalogTarget,
        expected: usize,
    ) -> Result<(), Elapsed> {
        timeout(Duration::from_secs(1), async {
            loop {
                let count = {
                    let in_flight = scheduler.in_flight.lock().await;
                    in_flight
                        .get(target)
                        .map_or(0, |flight| flight.result.receiver_count())
                };
                if count >= expected {
                    return;
                }
                yield_now().await;
            }
        })
        .await
    }

    async fn wait_until_not_in_flight(
        scheduler: &ModelCatalogScheduler,
        target: &ModelCatalogTarget,
    ) -> Result<(), Elapsed> {
        timeout(Duration::from_secs(1), async {
            loop {
                if !scheduler.in_flight.lock().await.contains_key(target) {
                    return;
                }
                yield_now().await;
            }
        })
        .await
    }

    async fn receiver_count(
        scheduler: &ModelCatalogScheduler,
        target: &ModelCatalogTarget,
    ) -> Option<usize> {
        scheduler
            .in_flight
            .lock()
            .await
            .get(target)
            .map(|flight| flight.result.receiver_count())
    }

    #[tokio::test]
    async fn same_endpoint_and_credential_share_one_concurrent_discovery() -> TestResult {
        let source = Arc::new(BlockingCatalogSource::new(0)?);
        let scheduler = Arc::new(ModelCatalogScheduler::new(source.clone()));
        let catalog_target = target("credential-a")?;

        let first_scheduler = Arc::clone(&scheduler);
        let first_target = catalog_target.clone();
        let first = tokio::spawn(async move { first_scheduler.synchronize(first_target).await });
        source.wait_for_started().await?;
        source.wait_until_waiting(1).await?;

        let second_scheduler = Arc::clone(&scheduler);
        let second_target = catalog_target.clone();
        let second = tokio::spawn(async move { second_scheduler.synchronize(second_target).await });
        wait_for_receiver_count(&scheduler, &catalog_target, 2).await?;
        assert_eq!(source.call_count(), 1);

        source.release.notify_waiters();
        let first_models = first.await??;
        let second_models = second.await??;

        assert_eq!(first_models, second_models);
        assert_eq!(
            model_names(&first_models),
            vec!["credential-model-credential-a", "shared-model"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn same_endpoint_with_different_credentials_never_share_discovery() -> TestResult {
        let source = Arc::new(BlockingCatalogSource::new(0)?);
        let scheduler = Arc::new(ModelCatalogScheduler::new(source.clone()));
        let first_target = target("credential-a")?;
        let second_target = target("credential-b")?;

        let first_scheduler = Arc::clone(&scheduler);
        let first = tokio::spawn(async move { first_scheduler.synchronize(first_target).await });
        source.wait_for_started().await?;

        let second_scheduler = Arc::clone(&scheduler);
        let second = tokio::spawn(async move { second_scheduler.synchronize(second_target).await });
        source.wait_for_started().await?;
        source.wait_until_waiting(2).await?;
        assert_eq!(source.call_count(), 2);

        source.release.notify_waiters();
        let first_models = first.await??;
        let second_models = second.await??;

        assert_eq!(
            model_names(&first_models),
            vec!["credential-model-credential-a", "shared-model"]
        );
        assert_eq!(
            model_names(&second_models),
            vec!["credential-model-credential-b", "shared-model"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn initiating_caller_cancellation_does_not_strand_a_later_follower() -> TestResult {
        let source = Arc::new(BlockingCatalogSource::new(0)?);
        let scheduler = Arc::new(ModelCatalogScheduler::new(source.clone()));
        let catalog_target = target("credential-a")?;

        let leader_scheduler = Arc::clone(&scheduler);
        let leader_target = catalog_target.clone();
        let leader = tokio::spawn(async move { leader_scheduler.synchronize(leader_target).await });
        source.wait_for_started().await?;
        source.wait_until_waiting(1).await?;
        leader.abort();
        match leader.await {
            Err(error) if error.is_cancelled() => {}
            Err(error) => return Err(error.into()),
            Ok(_) => {
                return Err(
                    std::io::Error::other("initiating caller unexpectedly completed").into(),
                );
            }
        }
        assert_eq!(receiver_count(&scheduler, &catalog_target).await, Some(0));

        let follower_scheduler = Arc::clone(&scheduler);
        let follower_target = catalog_target.clone();
        let follower =
            tokio::spawn(async move { follower_scheduler.synchronize(follower_target).await });
        wait_for_receiver_count(&scheduler, &catalog_target, 1).await?;
        assert_eq!(source.call_count(), 1);

        source.release.notify_waiters();
        let models = follower.await??;
        assert_eq!(
            model_names(&models),
            vec!["credential-model-credential-a", "shared-model"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn failed_discovery_is_shared_but_not_retained_as_a_result_cache() -> TestResult {
        let source = Arc::new(BlockingCatalogSource::new(1)?);
        let scheduler = Arc::new(ModelCatalogScheduler::new(source.clone()));
        let catalog_target = target("credential-a")?;

        let first_scheduler = Arc::clone(&scheduler);
        let first_target = catalog_target.clone();
        let first = tokio::spawn(async move { first_scheduler.synchronize(first_target).await });
        source.wait_for_started().await?;
        source.wait_until_waiting(1).await?;

        let second_scheduler = Arc::clone(&scheduler);
        let second_target = catalog_target.clone();
        let second = tokio::spawn(async move { second_scheduler.synchronize(second_target).await });
        wait_for_receiver_count(&scheduler, &catalog_target, 2).await?;
        source.release.notify_waiters();

        let Err(first_error) = first.await? else {
            return Err(std::io::Error::other("first discovery unexpectedly succeeded").into());
        };
        let Err(second_error) = second.await? else {
            return Err(std::io::Error::other("joined discovery unexpectedly succeeded").into());
        };
        assert_eq!(first_error, second_error);
        assert_eq!(source.call_count(), 1);
        wait_until_not_in_flight(&scheduler, &catalog_target).await?;

        let retry_scheduler = Arc::clone(&scheduler);
        let retry_target = catalog_target.clone();
        let retry = tokio::spawn(async move { retry_scheduler.synchronize(retry_target).await });
        source.wait_for_started().await?;
        source.wait_until_waiting(2).await?;
        assert_eq!(source.call_count(), 2);
        source.release.notify_waiters();

        let retry_models = retry.await??;
        assert_eq!(
            model_names(&retry_models),
            vec!["credential-model-credential-a", "shared-model"]
        );
        Ok(())
    }

    #[test]
    fn parallel_tools_requires_tools_and_narrowing_keeps_the_invariant() -> TestResult {
        assert_eq!(
            CapabilitySet::try_new([SemanticCapability::ParallelTools]),
            Err(CatalogViewError::ParallelToolsRequiresTools)
        );
        let capabilities = CapabilitySet::try_new([
            SemanticCapability::Tools,
            SemanticCapability::ParallelTools,
            SemanticCapability::Streaming,
        ])?;
        let narrowed = capabilities.without([SemanticCapability::Tools]);
        assert!(!narrowed.supports(SemanticCapability::Tools));
        assert!(!narrowed.supports(SemanticCapability::ParallelTools));
        assert!(narrowed.supports(SemanticCapability::Streaming));

        assert_eq!(
            CapabilitySet::try_new([SemanticCapability::ResponseCompaction]),
            Err(CatalogViewError::ResponseCompactionRequiresStoredResponses)
        );
        let continuity = CapabilitySet::try_new([
            SemanticCapability::StoredResponses,
            SemanticCapability::ResponseCompaction,
        ])?;
        let narrowed = continuity.without([SemanticCapability::StoredResponses]);
        assert!(!narrowed.supports(SemanticCapability::StoredResponses));
        assert!(!narrowed.supports(SemanticCapability::ResponseCompaction));

        assert_eq!(
            CapabilitySet::try_new([SemanticCapability::ResponsesWebSocket]),
            Err(CatalogViewError::ResponsesWebSocketRequiresStreaming)
        );
        let websocket = CapabilitySet::try_new([
            SemanticCapability::Streaming,
            SemanticCapability::ResponsesWebSocket,
        ])?;
        let narrowed = websocket.without([SemanticCapability::Streaming]);
        assert!(!narrowed.supports(SemanticCapability::Streaming));
        assert!(!narrowed.supports(SemanticCapability::ResponsesWebSocket));
        Ok(())
    }

    #[test]
    fn catalog_states_and_duplicate_records_are_explicit() -> TestResult {
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let catalog = CatalogView::try_new([CatalogModelEntry::try_new(
            endpoint.clone(),
            "upstream-model-a",
            CatalogModelState::Stale,
        )?])?;
        assert_eq!(
            catalog.model_state(&endpoint, "upstream-model-a"),
            Some(CatalogModelState::Stale)
        );
        assert!(CatalogModelState::Stale.is_hard_eligible());
        assert!(!CatalogModelState::Expired.is_hard_eligible());
        assert_eq!(
            CatalogView::try_new([
                CatalogModelEntry::try_new(
                    endpoint.clone(),
                    "upstream-model-a",
                    CatalogModelState::Fresh,
                )?,
                CatalogModelEntry::try_new(
                    endpoint,
                    "upstream-model-a",
                    CatalogModelState::Manual,
                )?,
            ]),
            Err(CatalogViewError::DuplicateCatalogModel)
        );
        Ok(())
    }

    #[test]
    fn endpoint_capability_view_rejects_ambiguous_profiles() -> TestResult {
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let capabilities = CapabilitySet::try_new([SemanticCapability::Tools])?;
        assert_eq!(
            EndpointCapabilityView::try_new([
                EndpointCapabilityEntry {
                    endpoint_id: endpoint.clone(),
                    capabilities: capabilities.clone(),
                },
                EndpointCapabilityEntry {
                    endpoint_id: endpoint,
                    capabilities,
                },
            ]),
            Err(CatalogViewError::DuplicateEndpointCapabilityProfile)
        );
        Ok(())
    }

    #[test]
    fn catalog_snapshot_uses_explicit_fresh_stale_refresh_and_expiry_boundaries() -> TestResult {
        let store = CatalogSnapshotStore::default();
        let catalog_target = target("credential-a")?;
        let observed_at_ms = 1_000;
        let snapshot = store.record_success(
            catalog_target.clone(),
            discovered_models(&["Model-Z", "Model-A", "Model-Z"])?,
            observed_at_ms,
        )?;

        assert_eq!(snapshot.version(), 1);
        assert_eq!(model_names(snapshot.models()), vec!["Model-A", "Model-Z"]);
        assert_eq!(snapshot.observed_at_ms(), observed_at_ms);
        assert_eq!(
            snapshot.stale_at_ms(),
            observed_at_ms + DEFAULT_CATALOG_FRESH_FOR_MS
        );
        assert_eq!(
            snapshot.refresh_due_at_ms(),
            observed_at_ms + DEFAULT_CATALOG_REFRESH_DUE_AFTER_MS
        );
        assert_eq!(
            snapshot.expires_at_ms(),
            observed_at_ms + DEFAULT_CATALOG_EXPIRES_AFTER_MS
        );
        assert_eq!(
            snapshot.freshness_at(snapshot.stale_at_ms() - 1)?,
            CatalogSnapshotFreshness::Fresh
        );
        assert_eq!(
            snapshot.freshness_at(snapshot.stale_at_ms())?,
            CatalogSnapshotFreshness::Stale
        );
        assert_eq!(
            snapshot.freshness_at(snapshot.refresh_due_at_ms())?,
            CatalogSnapshotFreshness::Stale
        );
        assert!(snapshot.is_refresh_due_at(snapshot.refresh_due_at_ms())?);
        assert_eq!(
            snapshot.freshness_at(snapshot.expires_at_ms() - 1)?,
            CatalogSnapshotFreshness::Stale
        );
        assert_eq!(
            snapshot.freshness_at(snapshot.expires_at_ms())?,
            CatalogSnapshotFreshness::Expired
        );

        let status = store
            .status_at(&catalog_target, snapshot.refresh_due_at_ms())?
            .ok_or_else(|| std::io::Error::other("snapshot unexpectedly absent"))?;
        assert_eq!(status.snapshot(), &snapshot);
        assert_eq!(status.freshness(), CatalogSnapshotFreshness::Stale);
        assert!(status.is_refresh_due());
        assert!(CatalogSnapshotFreshness::Stale.is_hard_eligible());
        assert!(!CatalogSnapshotFreshness::Expired.is_hard_eligible());
        Ok(())
    }

    #[test]
    fn discovery_failure_retains_only_its_target_last_success() -> TestResult {
        let store = CatalogSnapshotStore::default();
        let first_target = target("credential-a")?;
        let second_target = target("credential-b")?;
        let first_snapshot = store.record_success(
            first_target.clone(),
            discovered_models(&["credential-a-model"])?,
            10_000,
        )?;
        let second_snapshot = store.record_success(
            second_target.clone(),
            discovered_models(&["credential-b-model"])?,
            10_000,
        )?;

        assert_eq!(
            store.retain_last_success_on_failure(&first_target)?,
            Some(first_snapshot.clone())
        );
        assert_eq!(store.last_success(&second_target)?, Some(second_snapshot));

        let replacement = store.record_success(
            first_target.clone(),
            discovered_models(&["credential-a-next-model"])?,
            10_001,
        )?;
        assert_eq!(replacement.version(), 2);
        assert_eq!(
            model_names(replacement.models()),
            vec!["credential-a-next-model"]
        );
        assert_eq!(
            model_names(
                store
                    .last_success(&second_target)?
                    .ok_or_else(|| std::io::Error::other("second target unexpectedly absent"))?
                    .models(),
            ),
            vec!["credential-b-model"]
        );
        Ok(())
    }

    #[test]
    fn successful_empty_catalog_is_retained_without_inventing_removal_semantics() -> TestResult {
        let store = CatalogSnapshotStore::default();
        let catalog_target = target("credential-a")?;
        let snapshot = store.record_success(catalog_target.clone(), Vec::new(), 10_000)?;

        assert!(snapshot.models().is_empty());
        assert_eq!(snapshot.version(), 1);
        assert_eq!(
            store
                .status_at(&catalog_target, 10_000)?
                .map(|status| status.freshness()),
            Some(CatalogSnapshotFreshness::Fresh)
        );
        Ok(())
    }

    #[test]
    fn catalog_snapshot_rejects_invalid_or_non_monotonic_times_without_replacing_success()
    -> TestResult {
        assert_eq!(
            CatalogFreshnessPolicy::try_new(0, 1, 2),
            Err(CatalogSnapshotError::FreshDurationNotPositive)
        );
        assert_eq!(
            CatalogFreshnessPolicy::try_new(2, 1, 3),
            Err(CatalogSnapshotError::RefreshDueBeforeFresh)
        );
        assert_eq!(
            CatalogFreshnessPolicy::try_new(1, 2, 2),
            Err(CatalogSnapshotError::ExpiryNotAfterRefreshDue)
        );

        let store = CatalogSnapshotStore::default();
        let catalog_target = target("credential-a")?;
        assert_eq!(
            store.record_success(catalog_target.clone(), discovered_models(&["model"])?, -1,),
            Err(CatalogSnapshotError::TimestampBeforeUnixEpoch)
        );
        assert_eq!(
            store.record_success(
                catalog_target.clone(),
                discovered_models(&["model"])?,
                i64::MAX,
            ),
            Err(CatalogSnapshotError::TimestampOverflow)
        );

        let retained = store.record_success(
            catalog_target.clone(),
            discovered_models(&["first-model"])?,
            10_000,
        )?;
        assert_eq!(
            retained.freshness_at(9_999),
            Err(CatalogSnapshotError::ClockBeforeSnapshot)
        );
        assert_eq!(
            store.record_success(
                catalog_target.clone(),
                discovered_models(&["older-model"])?,
                9_999,
            ),
            Err(CatalogSnapshotError::TimestampNotMonotonic)
        );
        assert_eq!(store.last_success(&catalog_target)?, Some(retained));
        Ok(())
    }

    #[test]
    fn catalog_diff_preview_is_non_mutating_and_apply_rejects_a_stale_plan() -> TestResult {
        let snapshots = CatalogSnapshotStore::default();
        let diffs = CatalogDiffRegistry::new();
        let catalog_target = target("credential-a")?;
        let snapshot =
            successful_snapshot(&snapshots, &catalog_target, &["model-b", "model-a"], 100)?;

        let preview = diffs.preview(&snapshot)?;
        assert_eq!(preview.target(), &catalog_target);
        assert_eq!(preview.snapshot_version(), 1);
        assert_eq!(
            preview.events(),
            &[
                CatalogDiffEvent::Added {
                    model: DiscoveredModel::try_new("model-a")?,
                },
                CatalogDiffEvent::Added {
                    model: DiscoveredModel::try_new("model-b")?,
                },
            ]
        );

        let equivalent_preview = diffs.preview(&snapshot)?;
        assert_eq!(equivalent_preview, preview);
        let stale_preview = preview.clone();
        let applied = diffs.apply(preview)?;
        assert_eq!(applied.generation(), 1);
        assert_eq!(applied.snapshot_version(), 1);
        assert_eq!(applied.events(), stale_preview.events());
        assert_eq!(
            diffs.apply(stale_preview),
            Err(CatalogDiffError::StalePreview)
        );
        assert_eq!(
            diffs.preview(&snapshot),
            Err(CatalogDiffError::SnapshotVersionNotNewer)
        );
        Ok(())
    }

    #[test]
    fn catalog_diff_removes_only_after_three_successful_misses_and_24h() -> TestResult {
        let snapshots = CatalogSnapshotStore::default();
        let diffs = CatalogDiffRegistry::new();
        let catalog_target = target("credential-a")?;
        let initial = successful_snapshot(&snapshots, &catalog_target, &["model-a"], 100)?;
        diffs.apply(diffs.preview(&initial)?)?;

        let first_missing_at_ms = 1_000;
        let first = successful_snapshot(&snapshots, &catalog_target, &[], first_missing_at_ms)?;
        let first_preview = diffs.preview(&first)?;
        assert_eq!(
            first_preview.events(),
            &[CatalogDiffEvent::SuspectedRemoved {
                model: DiscoveredModel::try_new("model-a")?,
                consecutive_successful_misses: 1,
                first_missing_at_ms,
                removal_eligible_at_ms: first_missing_at_ms + MIN_CATALOG_REMOVAL_ISOLATION_MS,
            }]
        );
        diffs.apply(first_preview)?;

        let second =
            successful_snapshot(&snapshots, &catalog_target, &[], first_missing_at_ms + 1)?;
        let second_preview = diffs.preview(&second)?;
        assert!(matches!(
            second_preview.events(),
            [CatalogDiffEvent::SuspectedRemoved {
                consecutive_successful_misses: 2,
                ..
            }]
        ));
        diffs.apply(second_preview)?;

        let removal_at_ms = first_missing_at_ms + MIN_CATALOG_REMOVAL_ISOLATION_MS;
        let third = successful_snapshot(&snapshots, &catalog_target, &[], removal_at_ms)?;
        let third_preview = diffs.preview(&third)?;
        assert_eq!(
            third_preview.events(),
            &[CatalogDiffEvent::Removed {
                model: DiscoveredModel::try_new("model-a")?,
                consecutive_successful_misses: 3,
                first_missing_at_ms,
            }]
        );
        diffs.apply(third_preview)?;

        let reappeared =
            successful_snapshot(&snapshots, &catalog_target, &["model-a"], removal_at_ms + 1)?;
        let reappeared_preview = diffs.preview(&reappeared)?;
        assert_eq!(
            reappeared_preview.events(),
            &[CatalogDiffEvent::Added {
                model: DiscoveredModel::try_new("model-a")?,
            }]
        );
        Ok(())
    }

    #[test]
    fn catalog_diff_reappearance_resets_a_suspected_removal_sequence() -> TestResult {
        let snapshots = CatalogSnapshotStore::default();
        let diffs = CatalogDiffRegistry::new();
        let catalog_target = target("credential-a")?;
        let initial = successful_snapshot(&snapshots, &catalog_target, &["model-a"], 100)?;
        diffs.apply(diffs.preview(&initial)?)?;

        let first_missing = successful_snapshot(&snapshots, &catalog_target, &[], 1_000)?;
        diffs.apply(diffs.preview(&first_missing)?)?;

        let reappeared = successful_snapshot(&snapshots, &catalog_target, &["model-a"], 1_001)?;
        let reappeared_preview = diffs.preview(&reappeared)?;
        assert!(reappeared_preview.events().is_empty());
        diffs.apply(reappeared_preview)?;

        let missing_again = successful_snapshot(&snapshots, &catalog_target, &[], 1_002)?;
        let missing_again_preview = diffs.preview(&missing_again)?;
        assert!(matches!(
            missing_again_preview.events(),
            [CatalogDiffEvent::SuspectedRemoved {
                consecutive_successful_misses: 1,
                ..
            }]
        ));
        Ok(())
    }

    #[test]
    fn catalog_diff_never_mixes_same_endpoint_credential_targets() -> TestResult {
        let snapshots = CatalogSnapshotStore::default();
        let diffs = CatalogDiffRegistry::new();
        let first_target = target("credential-a")?;
        let second_target = target("credential-b")?;

        let first_initial = successful_snapshot(&snapshots, &first_target, &["model-a"], 100)?;
        let second_initial = successful_snapshot(&snapshots, &second_target, &["model-b"], 100)?;
        diffs.apply(diffs.preview(&first_initial)?)?;
        diffs.apply(diffs.preview(&second_initial)?)?;

        let first_missing = successful_snapshot(&snapshots, &first_target, &[], 1_000)?;
        let first_preview = diffs.preview(&first_missing)?;
        assert!(matches!(
            first_preview.events(),
            [CatalogDiffEvent::SuspectedRemoved {
                model,
                consecutive_successful_misses: 1,
                ..
            }] if model.upstream_model() == "model-a"
        ));

        let second_unchanged =
            successful_snapshot(&snapshots, &second_target, &["model-b"], 1_000)?;
        let second_preview = diffs.preview(&second_unchanged)?;
        assert!(second_preview.events().is_empty());
        Ok(())
    }
}
