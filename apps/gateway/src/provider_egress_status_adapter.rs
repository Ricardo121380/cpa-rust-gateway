//! Composition-layer adapter for Provider-specific egress runtime status.
//!
//! The adapter observes only the already-composed [`ProviderEgressRuntime`] used by native
//! Provider execution. It retains a bounded number of immutable, Config-Version-bound snapshots
//! so keyset pagination cannot mix runtime revisions or observation times. It never opens a
//! Store, reads a Credential secret, resolves DNS, contacts a Provider, leases an account, or
//! advances an egress/session/clearance recovery state machine.

#![deny(unsafe_code)]

use std::{
    collections::VecDeque,
    fmt::{self, Write as _},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gateway_control::{
    management_mutation_service::ConfigRevision,
    provider_egress_status_service::{
        ProviderEgressStatusChannelIdentity, ProviderEgressStatusChannelKind,
        ProviderEgressStatusClearanceItem, ProviderEgressStatusEgressItem,
        ProviderEgressStatusError, ProviderEgressStatusFacade, ProviderEgressStatusItem,
        ProviderEgressStatusPage, ProviderEgressStatusQuery, ProviderEgressStatusSessionItem,
        ProviderEgressStatusSnapshot, ProviderEgressStatusState, ProviderEgressStatusTarget,
    },
};
use gateway_router::{
    ProviderClearanceRuntimeState, ProviderEgressChannel, ProviderEgressRuntime,
    ProviderEgressRuntimeSnapshot, ProviderEgressRuntimeState, ProviderEgressTargetIdentity,
    ProviderSessionRuntimeState,
};
use gateway_store::control_plane::ConfigVersionId;

const INSTANCE_NONCE_BYTES: usize = 16;
const MAX_CACHE_TTL: Duration = Duration::from_mins(1);
const MAX_CURSOR_RETENTION: Duration = Duration::from_mins(10);
const MAX_RETAINED_SNAPSHOTS: usize = 8;

/// Explicit observation clock used by production and deterministic tests.
pub(crate) trait ProviderEgressStatusClock: Send + Sync {
    /// Returns a non-negative Unix millisecond value.
    fn now_ms(&self) -> Result<i64, ProviderEgressStatusAdapterError>;
}

/// Process wall clock for the production composition.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SystemProviderEgressStatusClock;

impl ProviderEgressStatusClock for SystemProviderEgressStatusClock {
    fn now_ms(&self) -> Result<i64, ProviderEgressStatusAdapterError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ProviderEgressStatusAdapterError::ClockUnavailable)?;
        i64::try_from(elapsed.as_millis())
            .map_err(|_| ProviderEgressStatusAdapterError::ClockUnavailable)
    }
}

/// Safe construction/observation failures for the composition adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderEgressStatusAdapterError {
    /// Config identity, revision, or cache bounds are invalid.
    InvalidConfiguration,
    /// Snapshot namespace entropy is unavailable.
    EntropyUnavailable,
    /// Wall-clock sampling is unavailable.
    ClockUnavailable,
}

impl fmt::Display for ProviderEgressStatusAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "provider egress-status configuration is invalid",
            Self::EntropyUnavailable => "provider egress-status namespace is unavailable",
            Self::ClockUnavailable => "provider egress-status clock is unavailable",
        })
    }
}

impl std::error::Error for ProviderEgressStatusAdapterError {}

struct CachedSnapshot {
    snapshot: ProviderEgressStatusSnapshot,
    fresh_until_ms: i64,
    retain_until_ms: i64,
}

#[derive(Default)]
struct SnapshotCache {
    current: Option<CachedSnapshot>,
    retained: VecDeque<CachedSnapshot>,
}

/// Read-only facade over one exact serving Config Version and its Provider-local runtime.
pub(crate) struct ProviderEgressStatusAdapter {
    config_version_id: ConfigVersionId,
    config_revision: ConfigRevision,
    runtime: Option<Arc<ProviderEgressRuntime>>,
    clock: Arc<dyn ProviderEgressStatusClock>,
    freshness_ms: i64,
    retention_ms: i64,
    instance_nonce: String,
    next_generation: AtomicU64,
    cache: Mutex<SnapshotCache>,
}

impl ProviderEgressStatusAdapter {
    /// Builds a bounded adapter. `runtime=None` is a truthful empty Provider-specific source, not
    /// an unavailable source: the selected serving graph has no composed E1 channel namespace.
    ///
    /// # Errors
    ///
    /// Rejects invalid Config identities/revisions, zero or oversized cache durations, and
    /// unavailable entropy.
    pub(crate) fn try_new(
        config_version_id: ConfigVersionId,
        config_revision: ConfigRevision,
        runtime: Option<Arc<ProviderEgressRuntime>>,
        clock: Arc<dyn ProviderEgressStatusClock>,
        freshness: Duration,
        retention: Duration,
    ) -> Result<Self, ProviderEgressStatusAdapterError> {
        if config_version_id.as_str().trim() != config_version_id.as_str()
            || config_version_id.as_str().is_empty()
            || config_version_id.as_str().chars().count() > 128
            || config_revision.as_i64() < 0
            || freshness.is_zero()
            || freshness > MAX_CACHE_TTL
            || retention < freshness
            || retention > MAX_CURSOR_RETENTION
        {
            return Err(ProviderEgressStatusAdapterError::InvalidConfiguration);
        }
        let freshness_ms = i64::try_from(freshness.as_millis())
            .map_err(|_| ProviderEgressStatusAdapterError::InvalidConfiguration)?;
        let retention_ms = i64::try_from(retention.as_millis())
            .map_err(|_| ProviderEgressStatusAdapterError::InvalidConfiguration)?;
        Ok(Self {
            config_version_id,
            config_revision,
            runtime,
            clock,
            freshness_ms,
            retention_ms,
            instance_nonce: random_instance_nonce()?,
            next_generation: AtomicU64::new(1),
            cache: Mutex::new(SnapshotCache::default()),
        })
    }

    fn build_snapshot(
        &self,
        sampled_at_ms: i64,
    ) -> Result<ProviderEgressStatusSnapshot, ProviderEgressStatusError> {
        if sampled_at_ms < 0 {
            return Err(ProviderEgressStatusError::SourceUnavailable);
        }
        let (runtime_revision, items) = match self.runtime.as_ref() {
            Some(runtime) => {
                let snapshot = runtime
                    .snapshot_at(sampled_at_ms)
                    .map_err(|_| ProviderEgressStatusError::SourceUnavailable)?;
                let revision = snapshot.revision();
                let items = provider_egress_status_items(&snapshot)?;
                (revision, items)
            }
            None => (0, Vec::new()),
        };
        let generation = self
            .next_generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| ProviderEgressStatusError::SourceUnavailable)?;
        let snapshot_id = format!("{}-{generation}", self.instance_nonce);
        ProviderEgressStatusSnapshot::try_new(
            self.config_version_id.clone(),
            self.config_revision,
            runtime_revision,
            snapshot_id,
            sampled_at_ms,
            items,
        )
    }

    fn page_from_cache(
        &self,
        config_version_id: &ConfigVersionId,
        config_revision: ConfigRevision,
        query: &ProviderEgressStatusQuery,
    ) -> Result<ProviderEgressStatusPage, ProviderEgressStatusError> {
        if config_version_id != &self.config_version_id || config_revision != self.config_revision {
            return Err(ProviderEgressStatusError::ConfigConflict);
        }
        let now_ms = self
            .clock
            .now_ms()
            .map_err(|_| ProviderEgressStatusError::SourceUnavailable)?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| ProviderEgressStatusError::SourceUnavailable)?;
        cache
            .retained
            .retain(|entry| entry.retain_until_ms > now_ms);

        if let Some(cursor) = query.cursor() {
            let snapshot_id = cursor.snapshot_id();
            let entry = cache
                .current
                .as_ref()
                .filter(|entry| {
                    entry.retain_until_ms > now_ms && entry.snapshot.snapshot_id() == snapshot_id
                })
                .or_else(|| {
                    cache
                        .retained
                        .iter()
                        .find(|entry| entry.snapshot.snapshot_id() == snapshot_id)
                })
                .ok_or(ProviderEgressStatusError::CursorConflict)?;
            return entry
                .snapshot
                .page(config_version_id, config_revision, query);
        }

        if cache
            .current
            .as_ref()
            .is_some_and(|entry| entry.fresh_until_ms > now_ms)
        {
            return cache
                .current
                .as_ref()
                .ok_or(ProviderEgressStatusError::SourceUnavailable)?
                .snapshot
                .page(config_version_id, config_revision, query);
        }

        let snapshot = self.build_snapshot(now_ms)?;
        let fresh_until_ms = now_ms
            .checked_add(self.freshness_ms)
            .ok_or(ProviderEgressStatusError::SourceUnavailable)?;
        let retain_until_ms = now_ms
            .checked_add(self.retention_ms)
            .ok_or(ProviderEgressStatusError::SourceUnavailable)?;
        if let Some(previous) = cache.current.take()
            && previous.retain_until_ms > now_ms
        {
            cache.retained.push_front(previous);
            cache.retained.truncate(MAX_RETAINED_SNAPSHOTS);
        }
        cache.current = Some(CachedSnapshot {
            snapshot,
            fresh_until_ms,
            retain_until_ms,
        });
        cache
            .current
            .as_ref()
            .ok_or(ProviderEgressStatusError::SourceUnavailable)?
            .snapshot
            .page(config_version_id, config_revision, query)
    }
}

impl ProviderEgressStatusFacade for ProviderEgressStatusAdapter {
    fn list_provider_egress_status(
        &self,
        config_version_id: &ConfigVersionId,
        config_revision: ConfigRevision,
        query: &ProviderEgressStatusQuery,
    ) -> Result<ProviderEgressStatusPage, ProviderEgressStatusError> {
        self.page_from_cache(config_version_id, config_revision, query)
    }
}

fn provider_egress_status_items(
    snapshot: &ProviderEgressRuntimeSnapshot,
) -> Result<Vec<ProviderEgressStatusItem>, ProviderEgressStatusError> {
    let mut items = Vec::with_capacity(
        snapshot.egress().len() + snapshot.sessions().len() + snapshot.clearances().len(),
    );
    for observation in snapshot.egress() {
        let (state, deadline_ms) = egress_state(observation.state());
        items.push(ProviderEgressStatusItem::Egress(
            ProviderEgressStatusEgressItem::try_new(
                channel_identity(observation.key().channel(), observation.channel())?,
                target(observation.key().target())?,
                state,
                deadline_ms,
            )?,
        ));
    }
    for observation in snapshot.sessions() {
        let key = observation.key();
        let (state, expires_at_ms) = session_state(observation.state());
        items.push(ProviderEgressStatusItem::Session(
            ProviderEgressStatusSessionItem::try_new(
                channel_identity(key.channel(), observation.channel())?,
                key.credential_id().clone(),
                key.credential_revision(),
                key.session_revision(),
                state,
                expires_at_ms,
            )?,
        ));
    }
    for observation in snapshot.clearances() {
        let key = observation.key();
        let session = key.session();
        let (state, expires_at_ms) = clearance_state(observation.state());
        items.push(ProviderEgressStatusItem::Clearance(
            ProviderEgressStatusClearanceItem::try_new(
                channel_identity(session.channel(), observation.channel())?,
                session.credential_id().clone(),
                session.credential_revision(),
                session.session_revision(),
                target(key.target())?,
                key.clearance_revision(),
                state,
                expires_at_ms,
            )?,
        ));
    }
    Ok(items)
}

fn channel_identity(
    identity: &gateway_router::ProviderChannelIdentity,
    channel: ProviderEgressChannel,
) -> Result<ProviderEgressStatusChannelIdentity, ProviderEgressStatusError> {
    ProviderEgressStatusChannelIdentity::try_new(
        identity.provider_id().clone(),
        identity.upstream_id().clone(),
        identity.endpoint_id().clone(),
        channel_kind(channel),
    )
}

const fn channel_kind(channel: ProviderEgressChannel) -> ProviderEgressStatusChannelKind {
    match channel {
        ProviderEgressChannel::GenericCompatible => {
            ProviderEgressStatusChannelKind::GenericCompatible
        }
        ProviderEgressChannel::GrokBuild => ProviderEgressStatusChannelKind::GrokBuild,
        ProviderEgressChannel::GrokConsole => ProviderEgressStatusChannelKind::GrokConsole,
        ProviderEgressChannel::GrokWeb => ProviderEgressStatusChannelKind::GrokWeb,
        ProviderEgressChannel::OfficialApi => ProviderEgressStatusChannelKind::OfficialApi,
        ProviderEgressChannel::CodexChatGpt => ProviderEgressStatusChannelKind::CodexChatGpt,
        ProviderEgressChannel::Kiro => ProviderEgressStatusChannelKind::Kiro,
        ProviderEgressChannel::ClaudeCompatible => {
            ProviderEgressStatusChannelKind::ClaudeCompatible
        }
        ProviderEgressChannel::OtherCompatible => ProviderEgressStatusChannelKind::OtherCompatible,
    }
}

fn target(
    target: &ProviderEgressTargetIdentity,
) -> Result<ProviderEgressStatusTarget, ProviderEgressStatusError> {
    target.as_named().map_or_else(
        || Ok(ProviderEgressStatusTarget::direct()),
        |value| ProviderEgressStatusTarget::named(value.to_owned()),
    )
}

const fn egress_state(
    state: ProviderEgressRuntimeState,
) -> (ProviderEgressStatusState, Option<i64>) {
    match state {
        ProviderEgressRuntimeState::Available => (ProviderEgressStatusState::Available, None),
        ProviderEgressRuntimeState::CoolingDown { until_ms } => {
            (ProviderEgressStatusState::CoolingDown, Some(until_ms))
        }
        ProviderEgressRuntimeState::CircuitOpen { probe_due_at_ms } => (
            ProviderEgressStatusState::CircuitOpen,
            Some(probe_due_at_ms),
        ),
        ProviderEgressRuntimeState::ProbeDue => (ProviderEgressStatusState::ProbeDue, None),
        ProviderEgressRuntimeState::ProbeInFlight { expires_at_ms } => (
            ProviderEgressStatusState::ProbeInFlight,
            Some(expires_at_ms),
        ),
        ProviderEgressRuntimeState::Disabled => (ProviderEgressStatusState::Disabled, None),
    }
}

const fn session_state(
    state: ProviderSessionRuntimeState,
) -> (ProviderEgressStatusState, Option<i64>) {
    match state {
        ProviderSessionRuntimeState::Absent => (ProviderEgressStatusState::Absent, None),
        ProviderSessionRuntimeState::Active { expires_at_ms } => {
            (ProviderEgressStatusState::Active, Some(expires_at_ms))
        }
        ProviderSessionRuntimeState::Expired => (ProviderEgressStatusState::Expired, None),
        ProviderSessionRuntimeState::ChallengeRequired => {
            (ProviderEgressStatusState::ChallengeRequired, None)
        }
        ProviderSessionRuntimeState::Invalid => (ProviderEgressStatusState::Invalid, None),
    }
}

const fn clearance_state(
    state: ProviderClearanceRuntimeState,
) -> (ProviderEgressStatusState, Option<i64>) {
    match state {
        ProviderClearanceRuntimeState::Absent => (ProviderEgressStatusState::Absent, None),
        ProviderClearanceRuntimeState::Fresh { expires_at_ms } => {
            (ProviderEgressStatusState::Fresh, Some(expires_at_ms))
        }
        ProviderClearanceRuntimeState::Expired => (ProviderEgressStatusState::Expired, None),
        ProviderClearanceRuntimeState::RefreshRequired => {
            (ProviderEgressStatusState::RefreshRequired, None)
        }
        ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms } => (
            ProviderEgressStatusState::RefreshInFlight,
            Some(expires_at_ms),
        ),
        ProviderClearanceRuntimeState::Invalid => (ProviderEgressStatusState::Invalid, None),
    }
}

fn random_instance_nonce() -> Result<String, ProviderEgressStatusAdapterError> {
    let mut bytes = [0_u8; INSTANCE_NONCE_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|_| ProviderEgressStatusAdapterError::EntropyUnavailable)?;
    let mut output = String::with_capacity(INSTANCE_NONCE_BYTES * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}")
            .map_err(|_| ProviderEgressStatusAdapterError::EntropyUnavailable)?;
    }
    Ok(output)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicI64;

    use gateway_control::provider_egress_status_service::{
        ProviderEgressStatusDomain, ProviderEgressStatusQuery,
    };
    use gateway_core::{CredentialId, EndpointId, ProviderId, UpstreamId};
    use gateway_router::{
        ProviderChannelCapability, ProviderChannelCapabilityRegistry, ProviderChannelIdentity,
        ProviderEgressRuntimeState, ProviderEgressStateKey, ProviderEgressTargetIdentity,
        ProviderSessionRuntimeState, ProviderSessionStateKey,
    };

    #[derive(Clone, Copy)]
    struct FixedClock(i64);

    impl ProviderEgressStatusClock for FixedClock {
        fn now_ms(&self) -> Result<i64, ProviderEgressStatusAdapterError> {
            Ok(self.0)
        }
    }

    struct MutableClock(AtomicI64);

    impl MutableClock {
        const fn new(value: i64) -> Self {
            Self(AtomicI64::new(value))
        }

        fn set(&self, value: i64) {
            self.0.store(value, Ordering::Release);
        }
    }

    impl ProviderEgressStatusClock for MutableClock {
        fn now_ms(&self) -> Result<i64, ProviderEgressStatusAdapterError> {
            Ok(self.0.load(Ordering::Acquire))
        }
    }

    fn id<T, E: fmt::Debug>(value: &str, constructor: impl FnOnce(String) -> Result<T, E>) -> T {
        constructor(value.to_owned()).expect("valid test id")
    }

    fn channel(
        provider: &str,
        upstream: &str,
        endpoint: &str,
        kind: ProviderEgressChannel,
    ) -> ProviderChannelCapability {
        let identity = ProviderChannelIdentity::try_new(
            id(provider, ProviderId::try_new),
            id(upstream, UpstreamId::try_new),
            id(endpoint, EndpointId::try_new),
        )
        .expect("valid channel identity");
        ProviderChannelCapability::new(identity, kind)
    }

    fn config(value: &str) -> ConfigVersionId {
        ConfigVersionId::try_new(value).expect("valid config")
    }

    fn revision(value: i64) -> ConfigRevision {
        ConfigRevision::try_new(value).expect("valid revision")
    }

    fn populated_runtime() -> Arc<ProviderEgressRuntime> {
        let build = channel(
            "grok.build",
            "upstream-build",
            "channel-build",
            ProviderEgressChannel::GrokBuild,
        );
        let console = channel(
            "grok.console",
            "upstream-console",
            "channel-console",
            ProviderEgressChannel::GrokConsole,
        );
        let build_identity = build.identity().clone();
        let console_identity = console.identity().clone();
        let registry = ProviderChannelCapabilityRegistry::try_new(vec![build, console])
            .expect("valid registry");
        let runtime = Arc::new(ProviderEgressRuntime::new(registry));
        runtime
            .set_egress_state(
                ProviderEgressStateKey::new(build_identity, ProviderEgressTargetIdentity::Direct),
                ProviderEgressRuntimeState::Available,
                100,
            )
            .expect("build egress");
        let credential = id("console-account", CredentialId::try_new);
        let session = ProviderSessionStateKey::try_new(console_identity, credential, 3, 3)
            .expect("session key");
        runtime
            .set_session_state(session, ProviderSessionRuntimeState::Absent, 100)
            .expect("console session");
        runtime
    }

    fn adapter_with_clock(
        runtime: Arc<ProviderEgressRuntime>,
        clock: Arc<MutableClock>,
    ) -> ProviderEgressStatusAdapter {
        ProviderEgressStatusAdapter::try_new(
            config("active-v1"),
            revision(4),
            Some(runtime),
            clock as Arc<dyn ProviderEgressStatusClock>,
            Duration::from_secs(5),
            Duration::from_secs(30),
        )
        .expect("adapter")
    }

    #[test]
    fn projects_exact_build_and_console_status_without_web_or_clearance() {
        let runtime = populated_runtime();
        let clock = Arc::new(MutableClock::new(100));
        let adapter = adapter_with_clock(runtime, clock);
        let page = adapter
            .list_provider_egress_status(
                &config("active-v1"),
                revision(4),
                &ProviderEgressStatusQuery::default(),
            )
            .expect("status page");
        assert_eq!(page.items.len(), 2);
        assert!(page.next_cursor.is_none());
        assert_eq!(page.items[0].domain(), ProviderEgressStatusDomain::Egress);
        assert_eq!(
            page.items[0].channel().channel_kind,
            ProviderEgressStatusChannelKind::GrokBuild
        );
        assert_eq!(page.items[0].state(), ProviderEgressStatusState::Available);
        assert_eq!(page.items[0].channel().provider_id.as_str(), "grok.build");
        assert_eq!(
            page.items[0].channel().upstream_id.as_str(),
            "upstream-build"
        );
        assert_eq!(page.items[0].channel().channel_id.as_str(), "channel-build");
        assert_eq!(page.items[1].domain(), ProviderEgressStatusDomain::Session);
        assert_eq!(
            page.items[1].channel().channel_kind,
            ProviderEgressStatusChannelKind::GrokConsole
        );
        assert_eq!(page.items[1].state(), ProviderEgressStatusState::Absent);
        assert_eq!(
            page.items[1].credential_id().map(CredentialId::as_str),
            Some("console-account")
        );
        assert!(
            page.items
                .iter()
                .all(|item| item.domain() != ProviderEgressStatusDomain::Clearance)
        );
        assert!(page.items.iter().all(|item| {
            item.channel().channel_kind != ProviderEgressStatusChannelKind::GrokWeb
        }));
    }

    #[test]
    fn retains_cursor_snapshot_until_its_bounded_expiry() {
        let runtime = populated_runtime();
        let clock = Arc::new(MutableClock::new(100));
        let adapter = adapter_with_clock(runtime, Arc::clone(&clock));
        let first_query =
            ProviderEgressStatusQuery::try_new(None, None, None, None, None, None, 1, None)
                .expect("query");
        let first = adapter
            .list_provider_egress_status(&config("active-v1"), revision(4), &first_query)
            .expect("first page");
        assert_eq!(first.items.len(), 1);
        assert!(first.next_cursor.is_some());

        clock.set(6_000);
        let refreshed = adapter
            .list_provider_egress_status(
                &config("active-v1"),
                revision(4),
                &ProviderEgressStatusQuery::default(),
            )
            .expect("refreshed snapshot");
        assert_ne!(refreshed.snapshot_id, first.snapshot_id);

        let second_query = ProviderEgressStatusQuery::try_new(
            None,
            None,
            None,
            None,
            None,
            None,
            1,
            first.next_cursor,
        )
        .expect("second query");
        let second = adapter
            .list_provider_egress_status(&config("active-v1"), revision(4), &second_query)
            .expect("second page");
        assert_eq!(second.items.len(), 1);
        assert_eq!(
            second.items[0].domain(),
            ProviderEgressStatusDomain::Session
        );
        assert_eq!(
            second.items[0].channel().channel_kind,
            ProviderEgressStatusChannelKind::GrokConsole
        );
        assert_eq!(second.items[0].state(), ProviderEgressStatusState::Absent);
        assert_eq!(
            second.items[0].credential_id().map(CredentialId::as_str),
            Some("console-account")
        );
        assert_eq!(second.snapshot_id, first.snapshot_id);
        assert_eq!(second.sampled_at_ms, first.sampled_at_ms);
        assert_eq!(second.runtime_revision, first.runtime_revision);
        assert!(second.next_cursor.is_none());

        clock.set(30_100);
        assert!(matches!(
            adapter.list_provider_egress_status(&config("active-v1"), revision(4), &second_query,),
            Err(ProviderEgressStatusError::CursorConflict)
        ));
    }

    #[test]
    fn current_cursor_expires_even_without_a_snapshot_rollover() {
        let runtime = populated_runtime();
        let current_clock = Arc::new(MutableClock::new(100));
        let current_adapter = adapter_with_clock(runtime, Arc::clone(&current_clock));
        let first_query =
            ProviderEgressStatusQuery::try_new(None, None, None, None, None, None, 1, None)
                .expect("query");
        let current_first = current_adapter
            .list_provider_egress_status(&config("active-v1"), revision(4), &first_query)
            .expect("current first page");
        let current_cursor_query = ProviderEgressStatusQuery::try_new(
            None,
            None,
            None,
            None,
            None,
            None,
            1,
            current_first.next_cursor,
        )
        .expect("current cursor query");
        current_clock.set(30_100);
        assert!(matches!(
            current_adapter.list_provider_egress_status(
                &config("active-v1"),
                revision(4),
                &current_cursor_query,
            ),
            Err(ProviderEgressStatusError::CursorConflict)
        ));
    }

    #[test]
    fn absent_runtime_is_empty_not_healthy_and_wrong_config_fails_closed() {
        let adapter = ProviderEgressStatusAdapter::try_new(
            config("active-v1"),
            revision(0),
            None,
            Arc::new(FixedClock(100)),
            Duration::from_secs(5),
            Duration::from_secs(30),
        )
        .expect("empty adapter");
        let query = ProviderEgressStatusQuery::default();
        let page = adapter
            .list_provider_egress_status(&config("active-v1"), revision(0), &query)
            .expect("empty page");
        assert!(page.items.is_empty());
        assert!(page.next_cursor.is_none());
        assert!(matches!(
            adapter.list_provider_egress_status(&config("other"), revision(0), &query),
            Err(ProviderEgressStatusError::ConfigConflict)
        ));
    }
}
