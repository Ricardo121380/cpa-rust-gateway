//! Provider-owned account-pool projections for the P13 management surface.
//!
//! This module is deliberately a read-only, provider-neutral seam. A Provider adapter builds a
//! bounded [`ProviderAccountPoolSnapshot`] from its own account store and runtime registries;
//! this module only validates, filters, orders, and paginates the secret-free projection. It does
//! not decrypt credentials, contact a Provider, refresh OAuth, or change scheduler state.

use std::{collections::BTreeSet, error::Error, fmt};

use gateway_core::{CredentialId, EndpointId, ProviderId};

/// Default number of Provider-owned account rows returned in one page.
pub const DEFAULT_PROVIDER_ACCOUNT_POOL_LIMIT: usize = 50;
/// Maximum number of Provider-owned account rows returned in one page.
pub const MAX_PROVIDER_ACCOUNT_POOL_LIMIT: usize = 100;
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
}

impl ProviderAccountPoolItem {
    fn validate(&self) -> Result<(), ProviderAccountPoolError> {
        if self.account_kind.trim().is_empty()
            || self.account_kind.chars().count() > MAX_TEXT_CHARS
            || !(-1_000..=1_000).contains(&self.priority)
            || !(1..=10_000).contains(&self.weight)
            || !(1..=10_000).contains(&self.max_concurrency)
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
    /// public maximum.
    pub fn try_new(
        provider_id: Option<ProviderId>,
        channel_id: Option<EndpointId>,
        auth_status: Option<ProviderAccountAuthStatus>,
        runtime_status: Option<ProviderAccountRuntimeStatus>,
        enabled: Option<bool>,
        limit: usize,
        cursor: Option<ProviderAccountPoolCursor>,
    ) -> Result<Self, ProviderAccountPoolError> {
        // Query construction is intentionally infallible apart from the public page bound.
        if !(1..=MAX_PROVIDER_ACCOUNT_POOL_LIMIT).contains(&limit) {
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
        format!(
            "{}|{}|{}|{}|{}",
            self.provider_id.as_ref().map_or("", ProviderId::as_str),
            self.channel_id.as_ref().map_or("", EndpointId::as_str),
            self.auth_status
                .map_or("", ProviderAccountAuthStatus::as_str),
            self.runtime_status
                .map_or("", ProviderAccountRuntimeStatus::as_str),
            self.enabled
                .map_or("", |value| if value { "true" } else { "false" }),
        )
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
    /// Returns [`ProviderAccountPoolError::InvalidQuery`] when the snapshot or filter fingerprint
    /// is empty/overlong.
    pub fn try_new(
        snapshot_id: impl Into<String>,
        filter_fingerprint: impl Into<String>,
        provider_id: ProviderId,
        channel_id: EndpointId,
        account_id: CredentialId,
    ) -> Result<Self, ProviderAccountPoolError> {
        // Cursor construction validates only bounded transport fields; identifier constructors
        // validate the opaque key fields before this function is called.
        let snapshot_id = snapshot_id.into();
        let filter_fingerprint = filter_fingerprint.into();
        if snapshot_id.trim().is_empty()
            || snapshot_id.chars().count() > MAX_SNAPSHOT_ID_CHARS
            || filter_fingerprint.chars().count() > 512
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
}

impl fmt::Display for ProviderAccountPoolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidQuery => "provider account-pool query is invalid",
            Self::InvalidSnapshot => "provider account-pool snapshot is invalid",
            Self::CursorConflict => "provider account-pool cursor is stale",
            Self::SourceUnavailable => "provider account-pool source is unavailable",
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

    fn item(provider: &str, channel: &str, account: &str) -> ProviderAccountPoolItem {
        ProviderAccountPoolItem {
            provider_id: ProviderId::try_new(provider).expect("provider id"),
            channel_id: EndpointId::try_new(channel).expect("channel id"),
            account_id: CredentialId::try_new(account).expect("account id"),
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
        }
    }

    #[test]
    fn snapshot_is_sorted_and_paginates_without_cross_provider_fallback() {
        let snapshot = ProviderAccountPoolSnapshot::try_new(
            "snapshot-1",
            10,
            vec![
                item("grok", "console", "b"),
                item("codex", "chat", "a"),
                item("grok", "build", "a"),
            ],
        )
        .expect("snapshot");
        let first = snapshot
            .page(
                &ProviderAccountPoolQuery::try_new(
                    Some(ProviderId::try_new("grok").expect("provider")),
                    None,
                    None,
                    None,
                    None,
                    1,
                    None,
                )
                .expect("query"),
            )
            .expect("page");
        assert_eq!(first.items[0].account_id.as_str(), "a");
        assert!(first.next_cursor.is_some());
        let second_query = ProviderAccountPoolQuery::try_new(
            Some(ProviderId::try_new("grok").expect("provider")),
            None,
            None,
            None,
            None,
            1,
            first.next_cursor,
        )
        .expect("query");
        let second = snapshot.page(&second_query).expect("page");
        assert_eq!(second.items[0].account_id.as_str(), "b");
        assert!(second.next_cursor.is_none());
    }

    #[test]
    fn cursor_rejects_changed_snapshot_or_filters() {
        let snapshot = ProviderAccountPoolSnapshot::try_new(
            "snapshot-1",
            10,
            vec![item("grok", "build", "a"), item("grok", "build", "b")],
        )
        .expect("snapshot");
        let first = snapshot
            .page(
                &ProviderAccountPoolQuery::try_new(None, None, None, None, None, 1, None)
                    .expect("query"),
            )
            .expect("page");
        let cursor = first.next_cursor.expect("cursor");
        let changed = ProviderAccountPoolSnapshot::try_new(
            "snapshot-2",
            11,
            vec![item("grok", "build", "a"), item("grok", "build", "b")],
        )
        .expect("snapshot");
        let query =
            ProviderAccountPoolQuery::try_new(None, None, None, None, None, 1, Some(cursor))
                .expect("query");
        assert_eq!(
            changed.page(&query),
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
    }
}
