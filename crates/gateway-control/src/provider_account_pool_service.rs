//! Provider-owned account-pool projections for the P13 management surface.
//!
//! This module is deliberately a read-only, provider-neutral seam. A Provider adapter builds a
//! bounded [`ProviderAccountPoolSnapshot`] from its own account store and runtime registries;
//! this module only validates, filters, orders, and paginates the secret-free projection. It does
//! not decrypt credentials, contact a Provider, refresh OAuth, or change scheduler state.

use std::{collections::BTreeSet, error::Error, fmt};

use gateway_core::{CredentialId, EndpointId, ProviderAccountEntitlement, ProviderId};

/// Default number of Provider-owned account rows returned in one page.
pub const DEFAULT_PROVIDER_ACCOUNT_POOL_LIMIT: usize = 50;
/// Maximum number of Provider-owned account rows returned in one page.
pub const MAX_PROVIDER_ACCOUNT_POOL_LIMIT: usize = 100;
/// Smallest operator cooldown accepted by the protected action boundary.
pub const MIN_PROVIDER_ACCOUNT_COOLDOWN_MS: i64 = 1_000;
/// Largest operator cooldown accepted by the protected action boundary.
pub const MAX_PROVIDER_ACCOUNT_COOLDOWN_MS: i64 = 86_400_000;
const MAX_TEXT_CHARS: usize = 128;
const MAX_SNAPSHOT_ID_CHARS: usize = 128;

/// Authentication lifecycle, kept separate from runtime availability.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderAccountAuthStatus {
    /// Authentication material is currently accepted by the Provider adapter.
    Active,
    /// An explicit OAuth/SSO repair is required; no automatic repair is implied here.
    ReauthRequired,
    /// Operator-disabled account.
    Disabled,
    /// Credential has an expired or otherwise unusable lifetime.
    Expired,
}

impl ProviderAccountAuthStatus {
    /// Stable wire value used by the management adapter.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::ReauthRequired => "reauth_required",
            Self::Disabled => "disabled",
            Self::Expired => "expired",
        }
    }
}

/// Runtime availability, kept separate from authentication lifecycle and quota observations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderAccountRuntimeStatus {
    /// Eligible for a new lease under the provider adapter's remaining gates.
    Available,
    /// Temporarily cooling down after a bounded failure or operator action.
    Cooling,
    /// Circuit is open and requires a probe/recovery decision.
    CircuitOpen,
    /// Quota state currently blocks a new lease.
    QuotaBlocked,
    /// Provider rejected the account or credential.
    Unauthorized,
    /// A recovery operation owns the account at this instant.
    RecoveryInFlight,
    /// Runtime will not lease an expired credential.
    Expired,
}

/// One explicitly requested, local Provider-account action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAccountOperatorAction {
    /// Selected Config Version that authorized this runtime action.
    pub config_version_id: String,
    /// Exact Provider family identity.
    pub provider_id: ProviderId,
    /// Exact Provider channel/Endpoint identity.
    pub channel_id: EndpointId,
    /// Exact opaque account identity.
    pub account_id: CredentialId,
    /// Optional exact model scope for Health/Quota state.
    pub upstream_model: Option<String>,
    /// Requested action kind.
    pub kind: ProviderAccountOperatorActionKind,
    /// Required only for [`ProviderAccountOperatorActionKind::CoolDown`].
    pub cooldown_ms: Option<i64>,
}

/// Closed set of runtime actions admitted by P13-06C.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderAccountOperatorActionKind {
    /// Put the exact Health key into a bounded transient cooldown.
    CoolDown,
    /// Ask the existing controlled account/quota recovery state machine to advance locally.
    RequestRecovery,
}

impl ProviderAccountOperatorActionKind {
    /// Stable wire value used by the management contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CoolDown => "cool_down",
            Self::RequestRecovery => "request_recovery",
        }
    }
}

impl ProviderAccountOperatorAction {
    /// Validates one action at the provider-neutral boundary.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderAccountPoolError::InvalidAction`] when an identity/model is outside the
    /// bounded domain or the action-specific cooldown fields are inconsistent.
    pub fn try_new(
        config_version_id: impl Into<String>,
        provider_id: ProviderId,
        channel_id: EndpointId,
        account_id: CredentialId,
        upstream_model: Option<String>,
        kind: ProviderAccountOperatorActionKind,
        cooldown_ms: Option<i64>,
    ) -> Result<Self, ProviderAccountPoolError> {
        let config_version_id = config_version_id.into();
        if !valid_opaque_id(&config_version_id)
            || !valid_opaque_id(provider_id.as_str())
            || !valid_opaque_id(channel_id.as_str())
            || !valid_opaque_id(account_id.as_str())
            || upstream_model
                .as_deref()
                .is_some_and(|value| value.trim().is_empty() || value.chars().count() > 256)
        {
            return Err(ProviderAccountPoolError::InvalidAction);
        }
        match kind {
            ProviderAccountOperatorActionKind::CoolDown
                if !cooldown_ms.is_some_and(|value| {
                    (MIN_PROVIDER_ACCOUNT_COOLDOWN_MS..=MAX_PROVIDER_ACCOUNT_COOLDOWN_MS)
                        .contains(&value)
                }) =>
            {
                return Err(ProviderAccountPoolError::InvalidAction);
            }
            ProviderAccountOperatorActionKind::RequestRecovery if cooldown_ms.is_some() => {
                return Err(ProviderAccountPoolError::InvalidAction);
            }
            _ => {}
        }
        Ok(Self {
            config_version_id,
            provider_id,
            channel_id,
            account_id,
            upstream_model,
            kind,
            cooldown_ms,
        })
    }
}

/// Safe state returned after one operator action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderAccountOperatorState {
    /// The action placed an exact Health key into cooldown.
    Cooling,
    /// A controlled account or quota probe now owns the exact target.
    ProbeScheduled,
    /// The quota window has not reset and cannot be moved by an operator action.
    RecoveryRequired,
    /// The requested transition is not applicable to the current exact state.
    Rejected,
}

impl ProviderAccountOperatorState {
    /// Stable wire value used by the management contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cooling => "cooling",
            Self::ProbeScheduled => "probe_scheduled",
            Self::RecoveryRequired => "recovery_required",
            Self::Rejected => "rejected",
        }
    }
}

/// Value-free receipt for one accepted operator action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAccountOperatorReceipt {
    /// Closed action state.
    pub state: ProviderAccountOperatorState,
    /// The sampled action time.
    pub observed_at_ms: i64,
    /// Effective cooldown deadline when the action was `cool_down`.
    pub cooldown_until_ms: Option<i64>,
}

impl ProviderAccountRuntimeStatus {
    /// Stable wire value used by the management adapter.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Cooling => "cooling",
            Self::CircuitOpen => "circuit_open",
            Self::QuotaBlocked => "quota_blocked",
            Self::Unauthorized => "unauthorized",
            Self::RecoveryInFlight => "recovery_in_flight",
            Self::Expired => "expired",
        }
    }
}

/// One secret-free Provider-owned account row.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProviderAccountPoolItem {
    /// Provider implementation family, not an endpoint URL or credential.
    pub provider_id: ProviderId,
    /// Provider-specific channel represented by its compiler-approved Endpoint identity.
    pub channel_id: EndpointId,
    /// Opaque CPAR account identity.
    pub account_id: CredentialId,
    /// Provider-specific account kind such as `grok_build_oauth`.
    pub account_kind: String,
    /// Authentication lifecycle state.
    pub auth_status: ProviderAccountAuthStatus,
    /// Current runtime state projection.
    pub runtime_status: ProviderAccountRuntimeStatus,
    /// Operator eligibility switch.
    pub enabled: bool,
    /// Lower values are preferred by the shared scheduler's normalized priority domain.
    pub priority: i64,
    /// Relative scheduler weight.
    pub weight: u32,
    /// Maximum concurrent leases admitted by the provider adapter.
    pub max_concurrency: u32,
    /// Number of currently held leases observed by the provider adapter.
    pub active_leases: u32,
    /// Provider credential expiry, if known.
    pub expires_at_ms: Option<i64>,
    /// Provider-specific refresh deadline, if known.
    pub refresh_due_at_ms: Option<i64>,
    /// Provider-specific quota synchronization deadline, if known.
    pub quota_sync_due_at_ms: Option<i64>,
    /// Provider/channel-scoped entitlement observation, if explicit evidence exists.
    pub entitlement: Option<ProviderAccountEntitlement>,
}

impl ProviderAccountPoolItem {
    fn validate(&self) -> Result<(), ProviderAccountPoolError> {
        if !valid_opaque_id(self.provider_id.as_str())
            || !valid_opaque_id(self.channel_id.as_str())
            || !valid_opaque_id(self.account_id.as_str())
            || self.account_kind.trim().is_empty()
            || self.account_kind.chars().count() > MAX_TEXT_CHARS
            || self.priority < 0
            || !(1..=10_000).contains(&self.weight)
            || !(1..=100_000).contains(&self.max_concurrency)
            || self.active_leases > self.max_concurrency
            || self.expires_at_ms.is_some_and(|value| value < 0)
            || self.refresh_due_at_ms.is_some_and(|value| value < 0)
            || self.quota_sync_due_at_ms.is_some_and(|value| value < 0)
        {
            return Err(ProviderAccountPoolError::InvalidSnapshot);
        }
        Ok(())
    }

    fn key(&self) -> (&str, &str, &str) {
        (
            self.provider_id.as_str(),
            self.channel_id.as_str(),
            self.account_id.as_str(),
        )
    }
}

/// A bounded, observed Provider-owned pool snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAccountPoolSnapshot {
    snapshot_id: String,
    observed_at_ms: i64,
    items: Vec<ProviderAccountPoolItem>,
}

impl ProviderAccountPoolSnapshot {
    /// Validates and deterministically orders a Provider snapshot.
    ///
    /// # Errors
    ///
    /// Rejects empty/overlong identities, invalid scheduling metadata, duplicate account keys,
    /// negative observation times, and oversized snapshots.
    pub fn try_new(
        snapshot_id: impl Into<String>,
        observed_at_ms: i64,
        mut items: Vec<ProviderAccountPoolItem>,
    ) -> Result<Self, ProviderAccountPoolError> {
        let snapshot_id = snapshot_id.into();
        if snapshot_id.trim().is_empty()
            || snapshot_id.chars().count() > MAX_SNAPSHOT_ID_CHARS
            || observed_at_ms < 0
        {
            return Err(ProviderAccountPoolError::InvalidSnapshot);
        }
        if items.len() > MAX_PROVIDER_ACCOUNT_POOL_LIMIT.saturating_mul(1_000) {
            return Err(ProviderAccountPoolError::InvalidSnapshot);
        }
        let mut keys = BTreeSet::new();
        for item in &items {
            item.validate()?;
            if !keys.insert(item.key()) {
                return Err(ProviderAccountPoolError::InvalidSnapshot);
            }
        }
        items.sort_by(|left, right| left.key().cmp(&right.key()));
        Ok(Self {
            snapshot_id,
            observed_at_ms,
            items,
        })
    }

    /// Returns the stable snapshot identity.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Returns the observation time shared by every row in the snapshot.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Builds a bounded page without calling a Provider or changing state.
    ///
    /// # Errors
    ///
    /// Returns an invalid-query error for an out-of-range limit and a cursor-conflict error when
    /// the cursor belongs to another snapshot or filter set.
    pub fn page(
        &self,
        query: &ProviderAccountPoolQuery,
    ) -> Result<ProviderAccountPoolPage, ProviderAccountPoolError> {
        if !(1..=MAX_PROVIDER_ACCOUNT_POOL_LIMIT).contains(&query.limit) {
            return Err(ProviderAccountPoolError::InvalidQuery);
        }
        if let Some(cursor) = &query.cursor
            && (cursor.snapshot_id != self.snapshot_id
                || cursor.filter_fingerprint != query.filter_fingerprint())
        {
            return Err(ProviderAccountPoolError::CursorConflict);
        }
        let mut filtered = self.items.iter().filter(|item| query.matches(item));
        let mut items = Vec::with_capacity(query.limit);
        for item in filtered.by_ref() {
            if query
                .cursor
                .as_ref()
                .is_some_and(|cursor| item.key() <= cursor.key())
            {
                continue;
            }
            items.push(item.clone());
            if items.len() == query.limit {
                break;
            }
        }
        let next_cursor = (items.len() == query.limit)
            .then(|| items.last())
            .flatten()
            .and_then(|last| {
                filtered.next().map(|_| ProviderAccountPoolCursor {
                    snapshot_id: self.snapshot_id.clone(),
                    filter_fingerprint: query.filter_fingerprint(),
                    provider_id: last.provider_id.clone(),
                    channel_id: last.channel_id.clone(),
                    account_id: last.account_id.clone(),
                })
            });
        Ok(ProviderAccountPoolPage {
            snapshot_id: self.snapshot_id.clone(),
            observed_at_ms: self.observed_at_ms,
            items,
            next_cursor,
        })
    }
}

/// Bounded filters and an opaque snapshot-bound page position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAccountPoolQuery {
    /// Exact Provider family filter.
    pub provider_id: Option<ProviderId>,
    /// Exact Provider channel/Endpoint filter.
    pub channel_id: Option<EndpointId>,
    /// Exact authentication lifecycle filter.
    pub auth_status: Option<ProviderAccountAuthStatus>,
    /// Exact runtime availability filter.
    pub runtime_status: Option<ProviderAccountRuntimeStatus>,
    /// Exact operator eligibility filter.
    pub enabled: Option<bool>,
    /// Bounded page size.
    pub limit: usize,
    /// Snapshot-bound keyset cursor.
    pub cursor: Option<ProviderAccountPoolCursor>,
}

impl Default for ProviderAccountPoolQuery {
    fn default() -> Self {
        Self {
            provider_id: None,
            channel_id: None,
            auth_status: None,
            runtime_status: None,
            enabled: None,
            limit: DEFAULT_PROVIDER_ACCOUNT_POOL_LIMIT,
            cursor: None,
        }
    }
}

impl ProviderAccountPoolQuery {
    /// Validates a public query before it reaches a Provider facade.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderAccountPoolError::InvalidQuery`] when `limit` is zero or exceeds the
    /// public maximum, or an optional opaque filter identity is outside the bounded ID domain.
    pub fn try_new(
        provider_id: Option<ProviderId>,
        channel_id: Option<EndpointId>,
        auth_status: Option<ProviderAccountAuthStatus>,
        runtime_status: Option<ProviderAccountRuntimeStatus>,
        enabled: Option<bool>,
        limit: usize,
        cursor: Option<ProviderAccountPoolCursor>,
    ) -> Result<Self, ProviderAccountPoolError> {
        if !(1..=MAX_PROVIDER_ACCOUNT_POOL_LIMIT).contains(&limit)
            || provider_id
                .as_ref()
                .is_some_and(|value| !valid_opaque_id(value.as_str()))
            || channel_id
                .as_ref()
                .is_some_and(|value| !valid_opaque_id(value.as_str()))
        {
            return Err(ProviderAccountPoolError::InvalidQuery);
        }
        Ok(Self {
            provider_id,
            channel_id,
            auth_status,
            runtime_status,
            enabled,
            limit,
            cursor,
        })
    }

    fn matches(&self, item: &ProviderAccountPoolItem) -> bool {
        self.provider_id
            .as_ref()
            .is_none_or(|value| value == &item.provider_id)
            && self
                .channel_id
                .as_ref()
                .is_none_or(|value| value == &item.channel_id)
            && self
                .auth_status
                .is_none_or(|value| value == item.auth_status)
            && self
                .runtime_status
                .is_none_or(|value| value == item.runtime_status)
            && self.enabled.is_none_or(|value| value == item.enabled)
    }

    fn filter_fingerprint(&self) -> String {
        [
            self.provider_id.as_ref().map_or("", ProviderId::as_str),
            self.channel_id.as_ref().map_or("", EndpointId::as_str),
            self.auth_status
                .map_or("", ProviderAccountAuthStatus::as_str),
            self.runtime_status
                .map_or("", ProviderAccountRuntimeStatus::as_str),
            self.enabled
                .map_or("", |value| if value { "true" } else { "false" }),
        ]
        .map(|value| format!("{}:{value}", value.len()))
        .join("|")
    }
}

/// Opaque keyset cursor. The HTTP adapter is responsible for transport encoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAccountPoolCursor {
    snapshot_id: String,
    filter_fingerprint: String,
    provider_id: ProviderId,
    channel_id: EndpointId,
    account_id: CredentialId,
}

impl ProviderAccountPoolCursor {
    /// Reconstructs a bounded wire-decoded cursor.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderAccountPoolError::InvalidQuery`] when a snapshot, filter fingerprint, or
    /// cursor identity is outside its bounded transport/ID domain.
    pub fn try_new(
        snapshot_id: impl Into<String>,
        filter_fingerprint: impl Into<String>,
        provider_id: ProviderId,
        channel_id: EndpointId,
        account_id: CredentialId,
    ) -> Result<Self, ProviderAccountPoolError> {
        let snapshot_id = snapshot_id.into();
        let filter_fingerprint = filter_fingerprint.into();
        if snapshot_id.trim().is_empty()
            || snapshot_id.chars().count() > MAX_SNAPSHOT_ID_CHARS
            || filter_fingerprint.chars().count() > 512
            || !valid_opaque_id(provider_id.as_str())
            || !valid_opaque_id(channel_id.as_str())
            || !valid_opaque_id(account_id.as_str())
        {
            return Err(ProviderAccountPoolError::InvalidQuery);
        }
        Ok(Self {
            snapshot_id,
            filter_fingerprint,
            provider_id,
            channel_id,
            account_id,
        })
    }

    /// Returns the source snapshot identity.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Returns the query filter fingerprint.
    #[must_use]
    pub fn filter_fingerprint(&self) -> &str {
        &self.filter_fingerprint
    }

    /// Returns the last Provider key.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns the last channel key.
    #[must_use]
    pub const fn channel_id(&self) -> &EndpointId {
        &self.channel_id
    }

    /// Returns the last account key.
    #[must_use]
    pub const fn account_id(&self) -> &CredentialId {
        &self.account_id
    }

    fn key(&self) -> (&str, &str, &str) {
        (
            self.provider_id.as_str(),
            self.channel_id.as_str(),
            self.account_id.as_str(),
        )
    }
}

fn valid_opaque_id(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= MAX_TEXT_CHARS
}

/// One page of a single observed Provider-owned snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAccountPoolPage {
    /// Stable observed snapshot identity.
    pub snapshot_id: String,
    /// Shared observation time for the page.
    pub observed_at_ms: i64,
    /// Secret-free account rows.
    pub items: Vec<ProviderAccountPoolItem>,
    /// Cursor for the next page, if more filtered rows exist.
    pub next_cursor: Option<ProviderAccountPoolCursor>,
}

/// Safe failures exposed by the Provider-owned account-pool facade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderAccountPoolError {
    /// Query or decoded cursor is invalid.
    InvalidQuery,
    /// Snapshot or row metadata violates the bounded contract.
    InvalidSnapshot,
    /// Cursor belongs to a different snapshot or filter set.
    CursorConflict,
    /// Provider-specific source is unavailable; no partial data is returned.
    SourceUnavailable,
    /// The requested operator action is malformed.
    InvalidAction,
    /// The exact action target is not part of the serving Provider pool.
    ActionTargetUnavailable,
}

impl fmt::Display for ProviderAccountPoolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidQuery => "provider account-pool query is invalid",
            Self::InvalidSnapshot => "provider account-pool snapshot is invalid",
            Self::CursorConflict => "provider account-pool cursor is stale",
            Self::SourceUnavailable => "provider account-pool source is unavailable",
            Self::InvalidAction => "provider account-pool operator action is invalid",
            Self::ActionTargetUnavailable => {
                "provider account-pool operator action target is unavailable"
            }
        })
    }
}

impl Error for ProviderAccountPoolError {}

/// Provider-neutral read-only seam consumed by the protected management HTTP surface.
pub trait ProviderAccountPoolFacade: Send + Sync {
    /// Returns one bounded page from one provider-owned observed snapshot.
    ///
    /// # Errors
    ///
    /// Returns a safe provider-pool error when the query is invalid, the cursor is stale, or the
    /// Provider-owned observation source is unavailable.
    fn list_provider_account_pools(
        &self,
        query: &ProviderAccountPoolQuery,
    ) -> Result<ProviderAccountPoolPage, ProviderAccountPoolError>;

    /// Applies one bounded local action to an exact Provider account.
    ///
    /// Implementations must not contact a Provider, acquire a lease, mutate a Config Version, or
    /// refresh/reauthenticate a credential. The default is fail-closed for read-only fixtures.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderAccountPoolError::ActionTargetUnavailable`] when the facade does not
    /// implement operator actions; provider adapters may return other bounded pool errors.
    fn apply_operator_action(
        &self,
        _action: &ProviderAccountOperatorAction,
        _observed_at_ms: i64,
    ) -> Result<ProviderAccountOperatorReceipt, ProviderAccountPoolError> {
        Err(ProviderAccountPoolError::ActionTargetUnavailable)
    }
}

/// Fail-closed facade used until the serving composition injects Provider adapters.
pub struct RejectingProviderAccountPoolFacade;

impl RejectingProviderAccountPoolFacade {
    /// Creates a no-send, no-provider default.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RejectingProviderAccountPoolFacade {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAccountPoolFacade for RejectingProviderAccountPoolFacade {
    fn list_provider_account_pools(
        &self,
        _query: &ProviderAccountPoolQuery,
    ) -> Result<ProviderAccountPoolPage, ProviderAccountPoolError> {
        Err(ProviderAccountPoolError::SourceUnavailable)
    }
}

/// Small adapter useful for tests and for future provider compositions that already own a
/// snapshot cache. It never contacts the Provider and only pages the supplied snapshot.
pub struct SnapshotProviderAccountPoolFacade {
    snapshot: ProviderAccountPoolSnapshot,
}

impl SnapshotProviderAccountPoolFacade {
    /// Creates a read-only facade around a validated snapshot.
    #[must_use]
    pub const fn new(snapshot: ProviderAccountPoolSnapshot) -> Self {
        Self { snapshot }
    }
}

impl ProviderAccountPoolFacade for SnapshotProviderAccountPoolFacade {
    fn list_provider_account_pools(
        &self,
        query: &ProviderAccountPoolQuery,
    ) -> Result<ProviderAccountPoolPage, ProviderAccountPoolError> {
        self.snapshot.page(query)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::manual_string_new)]
mod tests {
    use super::*;

    fn must<T, E>(value: Result<T, E>) -> T {
        match value {
            Ok(value) => value,
            Err(_) => std::process::abort(),
        }
    }

    fn must_some<T>(value: Option<T>) -> T {
        match value {
            Some(value) => value,
            None => std::process::abort(),
        }
    }

    fn item(provider: &str, channel: &str, account: &str) -> ProviderAccountPoolItem {
        ProviderAccountPoolItem {
            provider_id: must(ProviderId::try_new(provider)),
            channel_id: must(EndpointId::try_new(channel)),
            account_id: must(CredentialId::try_new(account)),
            account_kind: "synthetic".to_owned(),
            auth_status: ProviderAccountAuthStatus::Active,
            runtime_status: ProviderAccountRuntimeStatus::Available,
            enabled: true,
            priority: 1,
            weight: 1,
            max_concurrency: 4,
            active_leases: 0,
            expires_at_ms: None,
            refresh_due_at_ms: None,
            quota_sync_due_at_ms: None,
            entitlement: None,
        }
    }

    #[test]
    fn snapshot_is_sorted_and_paginates_without_cross_provider_fallback() {
        let snapshot = must(ProviderAccountPoolSnapshot::try_new(
            "snapshot-1",
            10,
            vec![
                item("grok", "console", "b"),
                item("codex", "chat", "a"),
                item("grok", "build", "a"),
            ],
        ));
        let first = must(snapshot.page(&must(ProviderAccountPoolQuery::try_new(
            Some(must(ProviderId::try_new("grok"))),
            None,
            None,
            None,
            None,
            1,
            None,
        ))));
        assert_eq!(first.items[0].account_id.as_str(), "a");
        assert!(first.next_cursor.is_some());
        let second_query = must(ProviderAccountPoolQuery::try_new(
            Some(must(ProviderId::try_new("grok"))),
            None,
            None,
            None,
            None,
            1,
            first.next_cursor,
        ));
        let second = must(snapshot.page(&second_query));
        assert_eq!(second.items[0].account_id.as_str(), "b");
        assert!(second.next_cursor.is_none());
    }

    #[test]
    fn cursor_rejects_changed_snapshot_or_filters() {
        let snapshot = must(ProviderAccountPoolSnapshot::try_new(
            "snapshot-1",
            10,
            vec![item("grok", "build", "a"), item("grok", "build", "b")],
        ));
        let first = must(snapshot.page(&must(ProviderAccountPoolQuery::try_new(
            None, None, None, None, None, 1, None,
        ))));
        let cursor = must_some(first.next_cursor);
        let changed = must(ProviderAccountPoolSnapshot::try_new(
            "snapshot-2",
            11,
            vec![item("grok", "build", "a"), item("grok", "build", "b")],
        ));
        let query = must(ProviderAccountPoolQuery::try_new(
            None,
            None,
            None,
            None,
            None,
            1,
            Some(cursor),
        ));
        assert_eq!(
            changed.page(&query),
            Err(ProviderAccountPoolError::CursorConflict)
        );
    }

    #[test]
    fn cursor_filter_fingerprint_is_unambiguous_for_delimiter_bearing_ids() {
        let snapshot = must(ProviderAccountPoolSnapshot::try_new(
            "snapshot-1",
            10,
            vec![
                item("a|b", "c", "a"),
                item("a|b", "c", "b"),
                item("a", "b|c", "a"),
                item("a", "b|c", "b"),
            ],
        ));
        let first = must(snapshot.page(&must(ProviderAccountPoolQuery::try_new(
            Some(must(ProviderId::try_new("a|b"))),
            Some(must(EndpointId::try_new("c"))),
            None,
            None,
            None,
            1,
            None,
        ))));
        let changed_filter = must(ProviderAccountPoolQuery::try_new(
            Some(must(ProviderId::try_new("a"))),
            Some(must(EndpointId::try_new("b|c"))),
            None,
            None,
            None,
            1,
            first.next_cursor,
        ));
        assert_eq!(
            snapshot.page(&changed_filter),
            Err(ProviderAccountPoolError::CursorConflict)
        );
    }

    #[test]
    fn invalid_or_secret_bearing_metadata_is_rejected_at_snapshot_boundary() {
        let mut invalid = item("grok", "build", "a");
        invalid.account_kind = "".to_owned();
        assert_eq!(
            ProviderAccountPoolSnapshot::try_new("snapshot-1", 10, vec![invalid]),
            Err(ProviderAccountPoolError::InvalidSnapshot)
        );

        let overlong = "x".repeat(MAX_TEXT_CHARS + 1);
        assert_eq!(
            ProviderAccountPoolSnapshot::try_new(
                "snapshot-2",
                10,
                vec![item(&overlong, "build", "a")],
            ),
            Err(ProviderAccountPoolError::InvalidSnapshot)
        );
        assert_eq!(
            ProviderAccountPoolQuery::try_new(
                Some(must(ProviderId::try_new(overlong))),
                None,
                None,
                None,
                None,
                1,
                None,
            ),
            Err(ProviderAccountPoolError::InvalidQuery)
        );
    }

    #[test]
    fn scheduler_normalized_priority_and_existing_binding_concurrency_are_representable() {
        let mut high_capacity = item("grok", "build", "a");
        // Native Grok priority -1000 normalizes to the shared lower-is-better tier 2000, while
        // ordinary management bindings already admit concurrency up to 100000.
        high_capacity.priority = 2_000;
        high_capacity.max_concurrency = 100_000;
        high_capacity.active_leases = 100_000;
        assert!(
            ProviderAccountPoolSnapshot::try_new("snapshot-1", 10, vec![high_capacity]).is_ok()
        );

        let mut invalid = item("grok", "build", "negative");
        invalid.priority = -1;
        assert_eq!(
            ProviderAccountPoolSnapshot::try_new("snapshot-2", 10, vec![invalid]),
            Err(ProviderAccountPoolError::InvalidSnapshot)
        );
    }
}
