//! Bounded per-Endpoint Credential scheduling and concurrency leases.
//!
//! This module is a runtime-only primitive. A control-path compiler decrypts and validates
//! Credentials before constructing a pool; selection only performs bounded reads and atomic
//! counter transitions. It has no `SQLite`, HTTP, retry, health, cooldown, circuit, or Provider
//! behavior.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use gateway_core::{CredentialId, EndpointId};
use zeroize::Zeroizing;

/// Maximum slots in one precompiled Endpoint Credential priority tier.
///
/// Smooth weighted scheduling consumes one slot for every configured weight unit. Keeping this
/// finite prevents a malformed control-plane value from making construction or a saturated scan
/// unbounded.
pub const MAX_CREDENTIAL_SCHEDULE_SLOTS_PER_PRIORITY_TIER: usize = 1024;

/// Zeroizing plaintext material retained only in an in-memory Credential pool.
///
/// The type is intentionally opaque and has a redacted `Debug` representation. It is created by
/// the control path after AEAD authentication and may be read only through a live
/// [`CredentialLease`].
pub struct CredentialSecret(Zeroizing<Vec<u8>>);

impl CredentialSecret {
    /// Validates and retains a non-empty Credential secret for one runtime pool entry.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialPoolBuildError::EmptyCredentialSecret`] without retaining an empty
    /// value.
    pub fn try_new(value: impl Into<Vec<u8>>) -> Result<Self, CredentialPoolBuildError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CredentialPoolBuildError::EmptyCredentialSecret);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    /// Returns the secret bytes only to the immediate authorized runtime consumer.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl fmt::Debug for CredentialSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialSecret")
            .field("bytes", &"<redacted>")
            .field("length", &self.0.len())
            .finish()
    }
}

/// Control-path input for one Endpoint-bound Credential.
///
/// `priority`, `weight`, and `concurrency` preserve the corresponding binding configuration.
/// Construction validates their finite runtime representation before any pool becomes available.
pub struct EndpointCredentialInput {
    /// Stable encrypted Credential record identity.
    pub credential_id: CredentialId,
    /// Non-secret Credential kind for a later Provider-specific request builder.
    pub credential_kind: String,
    /// Non-negative persistent Credential revision observed by this pool build.
    pub credential_revision: i64,
    /// Lower values are preferred before falling back to a later tier.
    pub priority: i64,
    /// Positive smooth-weighted selection weight inside one priority tier.
    pub weight: i64,
    /// Positive maximum number of concurrent live leases.
    pub concurrency: i64,
    /// Optional absolute Unix-millisecond expiry for a runtime credential.
    ///
    /// This is non-secret scheduling metadata. `None` preserves the historical non-expiring
    /// behavior used by API keys and Provider credentials whose lifetime is managed elsewhere.
    pub expires_at_ms: Option<i64>,
    /// Decrypted, redacted, zeroizing Credential material.
    pub secret: CredentialSecret,
}

impl fmt::Debug for EndpointCredentialInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EndpointCredentialInput")
            .field("credential_id", &self.credential_id)
            .field("credential_kind", &self.credential_kind)
            .field("credential_revision", &self.credential_revision)
            .field("priority", &self.priority)
            .field("weight", &self.weight)
            .field("concurrency", &self.concurrency)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("secret", &"<redacted>")
            .finish()
    }
}

/// A bounded, immutable Credential schedule for one Endpoint.
///
/// The schedule and entry metadata are constructed once. Each selection uses one independent
/// atomic cursor per priority tier and a bounded compare-and-exchange acquisition attempt for
/// each considered Credential.
pub struct EndpointCredentialPool {
    endpoint_id: EndpointId,
    credentials: Vec<Arc<CredentialSlot>>,
    priority_tiers: Vec<CredentialPriorityTier>,
    cursors: Vec<AtomicUsize>,
}

/// One bounded, secret-free observation of a Credential pool entry.
///
/// This is a point-in-time diagnostic view only. It never reserves capacity, exposes Secret
/// material, or promises that a later lease acquisition will observe the same active count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialPoolEntrySnapshot {
    credential_id: CredentialId,
    credential_kind: String,
    credential_revision: u64,
    priority: i64,
    weight: usize,
    maximum_concurrency: usize,
    expires_at_ms: Option<i64>,
    active_leases: usize,
}

impl CredentialPoolEntrySnapshot {
    /// Returns the stable non-secret Credential identity.
    #[must_use]
    pub fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the non-secret Provider credential kind retained by this pool entry.
    #[must_use]
    pub fn credential_kind(&self) -> &str {
        &self.credential_kind
    }

    /// Returns the persistent Credential revision captured by this pool build.
    #[must_use]
    pub const fn credential_revision(&self) -> u64 {
        self.credential_revision
    }

    /// Returns the immutable lower-is-better pool priority.
    #[must_use]
    pub const fn priority(&self) -> i64 {
        self.priority
    }

    /// Returns the immutable positive pool scheduling weight.
    #[must_use]
    pub const fn weight(&self) -> usize {
        self.weight
    }

    /// Returns the immutable maximum number of concurrently held leases.
    #[must_use]
    pub const fn maximum_concurrency(&self) -> usize {
        self.maximum_concurrency
    }

    /// Returns the optional non-secret absolute credential expiry observed at pool compilation.
    #[must_use]
    pub const fn expires_at_ms(&self) -> Option<i64> {
        self.expires_at_ms
    }

    /// Returns the point-in-time active lease count.
    #[must_use]
    pub const fn active_leases(&self) -> usize {
        self.active_leases
    }

    /// Returns whether this point-in-time observation is at its concurrency limit.
    #[must_use]
    pub const fn is_saturated(&self) -> bool {
        self.active_leases >= self.maximum_concurrency
    }
}

impl EndpointCredentialPool {
    /// Validates and constructs one Endpoint-local Credential pool.
    ///
    /// Credential IDs are sorted before schedules are built, so equal scores have stable order
    /// independent of control-plane insertion order. A pool never shares a cursor with another
    /// Endpoint.
    ///
    /// # Errors
    ///
    /// Returns a safe [`CredentialPoolBuildError`] for malformed, duplicate, or unbounded input.
    pub fn try_new(
        endpoint_id: EndpointId,
        entries: impl IntoIterator<Item = EndpointCredentialInput>,
    ) -> Result<Self, CredentialPoolBuildError> {
        let mut entries: Vec<_> = entries.into_iter().collect();
        if entries.is_empty() {
            return Err(CredentialPoolBuildError::EmptyCredentialPool);
        }
        entries.sort_by(|left, right| left.credential_id.cmp(&right.credential_id));

        let mut credentials = Vec::with_capacity(entries.len());
        let mut credentials_by_priority: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
        let mut previous_id: Option<CredentialId> = None;
        for entry in entries {
            validate_entry(&entry)?;
            if previous_id.as_ref() == Some(&entry.credential_id) {
                return Err(CredentialPoolBuildError::DuplicateCredential);
            }
            previous_id = Some(entry.credential_id.clone());

            let priority = entry.priority;
            let credential_index = credentials.len();
            credentials.push(Arc::new(CredentialSlot {
                credential_id: entry.credential_id,
                credential_kind: entry.credential_kind,
                credential_revision: u64::try_from(entry.credential_revision)
                    .map_err(|_| CredentialPoolBuildError::InvalidCredentialRevision)?,
                priority,
                weight: usize::try_from(entry.weight)
                    .map_err(|_| CredentialPoolBuildError::InvalidCredentialWeight)?,
                maximum_concurrency: usize::try_from(entry.concurrency)
                    .map_err(|_| CredentialPoolBuildError::InvalidCredentialConcurrency)?,
                expires_at_ms: entry.expires_at_ms,
                secret: entry.secret,
                active_leases: AtomicUsize::new(0),
            }));
            credentials_by_priority
                .entry(priority)
                .or_default()
                .push(credential_index);
        }

        let mut priority_tiers = Vec::with_capacity(credentials_by_priority.len());
        for (priority, credential_indexes) in credentials_by_priority {
            let slot_indexes = smooth_weighted_slots(&credentials, &credential_indexes)?;
            priority_tiers.push(CredentialPriorityTier {
                priority,
                slot_indexes,
            });
        }
        let cursors = priority_tiers.iter().map(|_| AtomicUsize::new(0)).collect();

        Ok(Self {
            endpoint_id,
            credentials,
            priority_tiers,
            cursors,
        })
    }

    /// Returns the Endpoint identity that owns this isolated pool.
    #[must_use]
    pub fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the number of immutable Credential entries in this pool.
    #[must_use]
    pub const fn credential_count(&self) -> usize {
        self.credentials.len()
    }

    /// Attempts to acquire one Credential lease from the highest available priority tier.
    ///
    /// A saturated Credential is skipped. Lower-priority tiers are considered only after all
    /// bounded slots of every higher tier fail acquisition. `None` contains no Endpoint,
    /// Credential, or Secret diagnostic and means no lease is currently available.
    #[must_use]
    pub fn try_lease(&self) -> Option<CredentialLease> {
        self.try_lease_eligible(|_| true)
    }

    /// Attempts to acquire one lease while a caller supplies a non-secret Credential eligibility
    /// predicate.
    ///
    /// The predicate runs only on a stable Credential ID before a capacity reservation. It lets a
    /// later runtime state layer skip a Cooling or Circuit-open Credential while preserving this
    /// Endpoint-local pool's bounded priority and weighted scheduling behavior. It must not block,
    /// query a Store, or inspect Secret material.
    #[must_use]
    pub fn try_lease_eligible<F>(&self, mut is_eligible: F) -> Option<CredentialLease>
    where
        F: FnMut(&CredentialId) -> bool,
    {
        self.try_lease_slots(|credential| is_eligible(&credential.credential_id))
    }

    /// Attempts to acquire one lease while applying an absolute expiry at the supplied time.
    ///
    /// The expiry check runs before capacity reservation and is intentionally separate from
    /// [`Self::try_lease_eligible`], preserving the historical API's behavior for callers that do
    /// not provide an observation time. Credentials with `None` expiry never expire here.
    #[must_use]
    pub fn try_lease_eligible_at<F>(
        &self,
        now_ms: i64,
        mut is_eligible: F,
    ) -> Option<CredentialLease>
    where
        F: FnMut(&CredentialId) -> bool,
    {
        self.try_lease_slots(|credential| {
            credential
                .expires_at_ms
                .is_none_or(|expires_at_ms| expires_at_ms > now_ms)
                && is_eligible(&credential.credential_id)
        })
    }

    /// Attempts to acquire one exact Credential lease without advancing a weighted cursor.
    ///
    /// This is the management/diagnostic counterpart to [`Self::try_lease_eligible_at`].  The
    /// target slot is resolved directly by its stable ID, then expiry, caller eligibility, and
    /// the atomic concurrency limit are checked.  No priority-tier cursor is read or mutated, so
    /// an operator pin cannot perturb ordinary weighted scheduling for later user requests.
    #[must_use]
    pub fn try_lease_exact_eligible_at<F>(
        &self,
        credential_id: &CredentialId,
        now_ms: i64,
        mut is_eligible: F,
    ) -> Option<CredentialLease>
    where
        F: FnMut(&CredentialId) -> bool,
    {
        let credential = self
            .credentials
            .iter()
            .find(|credential| &credential.credential_id == credential_id)?;
        if credential
            .expires_at_ms
            .is_some_and(|expires_at_ms| expires_at_ms <= now_ms)
            || !is_eligible(&credential.credential_id)
            || !credential.try_acquire()
        {
            return None;
        }
        Some(CredentialLease {
            credential: Arc::clone(credential),
        })
    }

    /// Attempts to lease one exact Credential revision without touching the weighted cursor.
    ///
    /// This is the stored-continuity counterpart to [`Self::try_lease_exact_eligible_at`]. A
    /// rotated Credential is a different durable owner and therefore fails before capacity is
    /// reserved, even when its stable Credential ID is unchanged.
    #[must_use]
    pub fn try_lease_exact_revision_eligible_at<F>(
        &self,
        credential_id: &CredentialId,
        credential_revision: u64,
        now_ms: i64,
        mut is_eligible: F,
    ) -> Option<CredentialLease>
    where
        F: FnMut(&CredentialId) -> bool,
    {
        let credential = self.credentials.iter().find(|credential| {
            &credential.credential_id == credential_id
                && credential.credential_revision == credential_revision
        })?;
        if credential
            .expires_at_ms
            .is_some_and(|expires_at_ms| expires_at_ms <= now_ms)
            || !is_eligible(&credential.credential_id)
            || !credential.try_acquire()
        {
            return None;
        }
        Some(CredentialLease {
            credential: Arc::clone(credential),
        })
    }

    fn try_lease_slots<F>(&self, mut is_eligible: F) -> Option<CredentialLease>
    where
        F: FnMut(&CredentialSlot) -> bool,
    {
        for (priority_tier, cursor) in self.priority_tiers.iter().zip(&self.cursors) {
            let slot_indexes = &priority_tier.slot_indexes;
            let start = cursor.fetch_add(1, Ordering::Relaxed);
            for offset in 0..slot_indexes.len() {
                let slot_index = start.wrapping_add(offset) % slot_indexes.len();
                let credential = self.credentials.get(slot_indexes[slot_index])?;
                if !is_eligible(credential) {
                    continue;
                }
                if credential.try_acquire() {
                    return Some(CredentialLease {
                        credential: Arc::clone(credential),
                    });
                }
            }
        }
        None
    }

    /// Returns the current active lease count for one known Credential.
    ///
    /// This is intended for bounded runtime diagnostics and tests; it never exposes secret data.
    #[must_use]
    pub fn active_lease_count(&self, credential_id: &CredentialId) -> Option<usize> {
        self.credentials
            .iter()
            .find(|credential| &credential.credential_id == credential_id)
            .map(|credential| credential.active_leases.load(Ordering::Acquire))
    }

    /// Returns stable secret-free entry observations in Credential-ID order.
    ///
    /// This bounded diagnostic helper does not move any pool cursor or reserve a lease.
    #[must_use]
    pub fn diagnostic_entries(&self) -> Vec<CredentialPoolEntrySnapshot> {
        self.credentials
            .iter()
            .map(|credential| credential.snapshot())
            .collect()
    }

    /// Peeks one currently eligible Credential using an explicit diagnostic schedule start.
    ///
    /// Unlike [`Self::try_lease_eligible`], this never advances a cursor or reserves capacity.
    /// The result is an instantaneous, bounded diagnostic projection only: another request may
    /// acquire capacity before a subsequent real selection.
    #[must_use]
    pub fn peek_eligible_from<F>(
        &self,
        start: usize,
        mut is_eligible: F,
    ) -> Option<CredentialPoolEntrySnapshot>
    where
        F: FnMut(&CredentialId) -> bool,
    {
        for priority_tier in &self.priority_tiers {
            let slot_indexes = &priority_tier.slot_indexes;
            for offset in 0..slot_indexes.len() {
                let slot_index = start.wrapping_add(offset) % slot_indexes.len();
                let credential = self.credentials.get(slot_indexes[slot_index])?;
                if is_eligible(&credential.credential_id) && credential.has_capacity() {
                    return Some(credential.snapshot());
                }
            }
        }
        None
    }
}

impl fmt::Debug for EndpointCredentialPool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EndpointCredentialPool")
            .field("endpoint_id", &self.endpoint_id)
            .field("credential_count", &self.credentials.len())
            .field("priority_tier_count", &self.priority_tiers.len())
            .finish_non_exhaustive()
    }
}

/// Immutable collection of independently scheduled Endpoint Credential pools.
#[derive(Clone, Debug)]
pub struct EndpointCredentialPools {
    pools: BTreeMap<EndpointId, Arc<EndpointCredentialPool>>,
}

impl EndpointCredentialPools {
    /// Creates an Endpoint-indexed set and rejects duplicate Endpoint pools.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialPoolBuildError::DuplicateEndpointPool`] rather than silently replacing
    /// an existing Endpoint scheduler.
    pub fn try_new(
        pools: impl IntoIterator<Item = EndpointCredentialPool>,
    ) -> Result<Self, CredentialPoolBuildError> {
        let mut indexed = BTreeMap::new();
        for pool in pools {
            let endpoint_id = pool.endpoint_id.clone();
            if indexed.insert(endpoint_id, Arc::new(pool)).is_some() {
                return Err(CredentialPoolBuildError::DuplicateEndpointPool);
            }
        }
        Ok(Self { pools: indexed })
    }

    /// Merges two independently compiled pool sets without replacing an Endpoint identity.
    ///
    /// Native provider account pools and ordinary control-plane Credential pools are compiled by
    /// different stores, but the request scheduler must see one immutable Endpoint index. A
    /// duplicate Endpoint is rejected rather than silently choosing one source.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialPoolBuildError::DuplicateEndpointPool`] when both sets contain the
    /// same Endpoint identity.
    pub fn merge(mut self, other: Self) -> Result<Self, CredentialPoolBuildError> {
        for (endpoint_id, pool) in other.pools {
            if self.pools.insert(endpoint_id, pool).is_some() {
                return Err(CredentialPoolBuildError::DuplicateEndpointPool);
            }
        }
        Ok(self)
    }

    /// Returns one Endpoint-local pool without constructing or querying runtime state.
    #[must_use]
    pub fn pool(&self, endpoint_id: &EndpointId) -> Option<&Arc<EndpointCredentialPool>> {
        self.pools.get(endpoint_id)
    }

    /// Attempts to acquire a Credential lease from one Endpoint-local pool.
    #[must_use]
    pub fn try_lease(&self, endpoint_id: &EndpointId) -> Option<CredentialLease> {
        self.pool(endpoint_id)?.try_lease()
    }

    /// Attempts to acquire an Endpoint-local Credential lease after a non-secret eligibility
    /// predicate approves the candidate Credential ID.
    #[must_use]
    pub fn try_lease_eligible<F>(
        &self,
        endpoint_id: &EndpointId,
        is_eligible: F,
    ) -> Option<CredentialLease>
    where
        F: FnMut(&CredentialId) -> bool,
    {
        self.pool(endpoint_id)?.try_lease_eligible(is_eligible)
    }

    /// Attempts to acquire one Endpoint-local lease after applying an absolute expiry and a
    /// non-secret Credential-ID eligibility predicate.
    #[must_use]
    pub fn try_lease_eligible_at<F>(
        &self,
        endpoint_id: &EndpointId,
        now_ms: i64,
        is_eligible: F,
    ) -> Option<CredentialLease>
    where
        F: FnMut(&CredentialId) -> bool,
    {
        self.pool(endpoint_id)?
            .try_lease_eligible_at(now_ms, is_eligible)
    }

    /// Attempts to acquire one exact Endpoint Credential lease without moving that pool's
    /// weighted cursor.
    #[must_use]
    pub fn try_lease_exact_eligible_at<F>(
        &self,
        endpoint_id: &EndpointId,
        credential_id: &CredentialId,
        now_ms: i64,
        is_eligible: F,
    ) -> Option<CredentialLease>
    where
        F: FnMut(&CredentialId) -> bool,
    {
        self.pool(endpoint_id)?
            .try_lease_exact_eligible_at(credential_id, now_ms, is_eligible)
    }

    /// Attempts to acquire one exact Credential revision without advancing a weighted cursor.
    #[must_use]
    pub fn try_lease_exact_revision_eligible_at<F>(
        &self,
        endpoint_id: &EndpointId,
        credential_id: &CredentialId,
        credential_revision: u64,
        now_ms: i64,
        is_eligible: F,
    ) -> Option<CredentialLease>
    where
        F: FnMut(&CredentialId) -> bool,
    {
        self.pool(endpoint_id)?
            .try_lease_exact_revision_eligible_at(
                credential_id,
                credential_revision,
                now_ms,
                is_eligible,
            )
    }

    /// Returns the number of Endpoint pools in this immutable set.
    #[must_use]
    pub fn endpoint_count(&self) -> usize {
        self.pools.len()
    }
}

/// One request-scoped, automatically released concurrent Credential lease.
///
/// The lease is intentionally non-cloneable. Dropping it, including when a request Future is
/// cancelled, releases exactly its one successful atomic acquisition. [`Self::release`] consumes
/// the lease for callers that want an explicit end-of-use boundary.
pub struct CredentialLease {
    credential: Arc<CredentialSlot>,
}

impl CredentialLease {
    /// Returns the stable selected Credential identity.
    #[must_use]
    pub fn credential_id(&self) -> &CredentialId {
        &self.credential.credential_id
    }

    /// Returns the non-secret Credential kind selected by this lease.
    #[must_use]
    pub fn credential_kind(&self) -> &str {
        &self.credential.credential_kind
    }

    /// Returns the persistent Credential revision observed when this pool was compiled.
    #[must_use]
    pub fn credential_revision(&self) -> u64 {
        self.credential.credential_revision
    }

    /// Returns Credential bytes only while this live lease remains owned by the caller.
    #[must_use]
    pub fn secret_bytes(&self) -> &[u8] {
        self.credential.secret.as_bytes()
    }

    /// Explicitly ends this lease.
    ///
    /// Consuming `self` ensures the Secret cannot be used after capacity becomes available to a
    /// different request. Dropping without this call has the same release effect.
    pub fn release(self) {
        drop(self);
    }
}

impl Drop for CredentialLease {
    fn drop(&mut self) {
        self.credential.release();
    }
}

impl fmt::Debug for CredentialLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialLease")
            .field("credential_id", &self.credential.credential_id)
            .field("credential_kind", &self.credential.credential_kind)
            .field("credential_revision", &self.credential.credential_revision)
            .field("secret", &"<redacted>")
            .finish()
    }
}

struct CredentialSlot {
    credential_id: CredentialId,
    credential_kind: String,
    credential_revision: u64,
    priority: i64,
    weight: usize,
    maximum_concurrency: usize,
    expires_at_ms: Option<i64>,
    secret: CredentialSecret,
    active_leases: AtomicUsize,
}

impl CredentialSlot {
    fn snapshot(&self) -> CredentialPoolEntrySnapshot {
        CredentialPoolEntrySnapshot {
            credential_id: self.credential_id.clone(),
            credential_kind: self.credential_kind.clone(),
            credential_revision: self.credential_revision,
            priority: self.priority,
            weight: self.weight,
            maximum_concurrency: self.maximum_concurrency,
            expires_at_ms: self.expires_at_ms,
            active_leases: self.active_leases.load(Ordering::Acquire),
        }
    }

    fn has_capacity(&self) -> bool {
        self.active_leases.load(Ordering::Acquire) < self.maximum_concurrency
    }

    fn try_acquire(&self) -> bool {
        let mut active_leases = self.active_leases.load(Ordering::Acquire);
        loop {
            if active_leases >= self.maximum_concurrency {
                return false;
            }
            match self.active_leases.compare_exchange_weak(
                active_leases,
                active_leases + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(observed) => active_leases = observed,
            }
        }
    }

    fn release(&self) {
        let previous = self.active_leases.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(
            previous > 0,
            "a Credential lease must release one acquisition"
        );
    }
}

struct CredentialPriorityTier {
    priority: i64,
    slot_indexes: Vec<usize>,
}

impl fmt::Debug for CredentialPriorityTier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialPriorityTier")
            .field("priority", &self.priority)
            .field("slot_count", &self.slot_indexes.len())
            .finish_non_exhaustive()
    }
}

fn validate_entry(entry: &EndpointCredentialInput) -> Result<(), CredentialPoolBuildError> {
    if entry.credential_kind.is_empty() {
        return Err(CredentialPoolBuildError::EmptyCredentialKind);
    }
    if entry.credential_revision < 0 {
        return Err(CredentialPoolBuildError::InvalidCredentialRevision);
    }
    if entry.priority < 0 {
        return Err(CredentialPoolBuildError::InvalidCredentialPriority);
    }
    if entry.weight <= 0 {
        return Err(CredentialPoolBuildError::InvalidCredentialWeight);
    }
    if entry.concurrency <= 0 {
        return Err(CredentialPoolBuildError::InvalidCredentialConcurrency);
    }
    if entry
        .expires_at_ms
        .is_some_and(|expires_at_ms| expires_at_ms < 0)
    {
        return Err(CredentialPoolBuildError::InvalidCredentialExpiry);
    }
    Ok(())
}

fn smooth_weighted_slots(
    credentials: &[Arc<CredentialSlot>],
    credential_indexes: &[usize],
) -> Result<Vec<usize>, CredentialPoolBuildError> {
    let mut total_slots = 0_usize;
    let mut weights = Vec::with_capacity(credential_indexes.len());
    for credential_index in credential_indexes {
        let credential = credentials
            .get(*credential_index)
            .ok_or(CredentialPoolBuildError::InconsistentCredentialPool)?;
        total_slots = total_slots
            .checked_add(credential.weight)
            .ok_or(CredentialPoolBuildError::CredentialScheduleTooLarge)?;
        if total_slots > MAX_CREDENTIAL_SCHEDULE_SLOTS_PER_PRIORITY_TIER {
            return Err(CredentialPoolBuildError::CredentialScheduleTooLarge);
        }
        weights.push(
            i64::try_from(credential.weight)
                .map_err(|_| CredentialPoolBuildError::CredentialScheduleTooLarge)?,
        );
    }

    let total_weight = i64::try_from(total_slots)
        .map_err(|_| CredentialPoolBuildError::CredentialScheduleTooLarge)?;
    let mut current_weights = vec![0_i64; weights.len()];
    let mut slot_indexes = Vec::with_capacity(total_slots);
    for _ in 0..total_slots {
        for (current_weight, weight) in current_weights.iter_mut().zip(&weights) {
            *current_weight = current_weight
                .checked_add(*weight)
                .ok_or(CredentialPoolBuildError::CredentialScheduleTooLarge)?;
        }
        let mut selected = 0_usize;
        for credential_position in 1..current_weights.len() {
            if current_weights[credential_position] > current_weights[selected] {
                selected = credential_position;
            }
        }
        current_weights[selected] = current_weights[selected]
            .checked_sub(total_weight)
            .ok_or(CredentialPoolBuildError::CredentialScheduleTooLarge)?;
        slot_indexes.push(credential_indexes[selected]);
    }
    Ok(slot_indexes)
}

/// Safe configuration failures for runtime Credential pools.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialPoolBuildError {
    /// A pool had no eligible Credential entries.
    EmptyCredentialPool,
    /// A runtime Credential secret was empty.
    EmptyCredentialSecret,
    /// A runtime Credential kind was empty.
    EmptyCredentialKind,
    /// More than one entry had the same Credential identity in one Endpoint pool.
    DuplicateCredential,
    /// A Credential revision was negative or outside the runtime representation.
    InvalidCredentialRevision,
    /// A Credential binding used a negative priority.
    InvalidCredentialPriority,
    /// A Credential binding used a non-positive or unrepresentable weight.
    InvalidCredentialWeight,
    /// A Credential binding used a non-positive or unrepresentable concurrency limit.
    InvalidCredentialConcurrency,
    /// A Credential expiry was negative and cannot represent a Unix-millisecond instant.
    InvalidCredentialExpiry,
    /// A precompiled Credential tier would exceed its finite slot limit.
    CredentialScheduleTooLarge,
    /// Internal construction found an impossible missing sorted slot reference.
    InconsistentCredentialPool,
    /// More than one pool targeted the same Endpoint.
    DuplicateEndpointPool,
}

impl fmt::Display for CredentialPoolBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyCredentialPool => "Endpoint Credential pool has no entries",
            Self::EmptyCredentialSecret => "runtime Credential secret must not be empty",
            Self::EmptyCredentialKind => "runtime Credential kind must not be empty",
            Self::DuplicateCredential => "Endpoint Credential pool has a duplicate Credential",
            Self::InvalidCredentialRevision => "Credential revision is invalid",
            Self::InvalidCredentialPriority => "Credential priority tier is invalid",
            Self::InvalidCredentialWeight => "Credential scheduling weight is invalid",
            Self::InvalidCredentialConcurrency => "Credential concurrency limit is invalid",
            Self::InvalidCredentialExpiry => "Credential expiry is invalid",
            Self::CredentialScheduleTooLarge => {
                "Endpoint Credential schedule exceeds its finite limit"
            }
            Self::InconsistentCredentialPool => {
                "Endpoint Credential pool is internally inconsistent"
            }
            Self::DuplicateEndpointPool => "Endpoint Credential pool set has a duplicate Endpoint",
        };
        formatter.write_str(message)
    }
}

impl Error for CredentialPoolBuildError {}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        error::Error,
        io,
        sync::{Arc, Barrier, mpsc},
        thread,
    };

    use gateway_core::{CredentialId, EndpointId};

    use super::{
        CredentialPoolBuildError, CredentialSecret, EndpointCredentialInput,
        EndpointCredentialPool, MAX_CREDENTIAL_SCHEDULE_SLOTS_PER_PRIORITY_TIER,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn smooth_weighted_tier_has_an_exact_complete_cycle() -> TestResult {
        let pool = pool(
            "endpoint-a",
            vec![
                ("credential-a", 0, 5, 4),
                ("credential-b", 0, 1, 4),
                ("credential-c", 0, 1, 4),
            ],
        )?;

        let mut counts = BTreeMap::new();
        for _ in 0..7 {
            let lease = pool
                .try_lease()
                .ok_or_else(|| io::Error::other("expected a Credential lease"))?;
            *counts
                .entry(lease.credential_id().as_str().to_owned())
                .or_default() += 1;
        }

        assert_eq!(counts.get("credential-a"), Some(&5));
        assert_eq!(counts.get("credential-b"), Some(&1));
        assert_eq!(counts.get("credential-c"), Some(&1));
        Ok(())
    }

    #[test]
    fn saturation_falls_back_to_a_lower_priority_tier() -> TestResult {
        let pool = pool(
            "endpoint-a",
            vec![
                ("credential-preferred", 0, 1, 1),
                ("credential-fallback", 1, 1, 1),
            ],
        )?;
        let preferred = pool
            .try_lease()
            .ok_or_else(|| io::Error::other("expected preferred lease"))?;
        assert_eq!(preferred.credential_id().as_str(), "credential-preferred");

        let fallback = pool
            .try_lease()
            .ok_or_else(|| io::Error::other("expected fallback lease"))?;
        assert_eq!(fallback.credential_id().as_str(), "credential-fallback");
        assert!(pool.try_lease().is_none());

        drop(preferred);
        let released = pool
            .try_lease()
            .ok_or_else(|| io::Error::other("expected released preferred lease"))?;
        assert_eq!(released.credential_id().as_str(), "credential-preferred");
        drop(fallback);
        Ok(())
    }

    #[test]
    fn dropped_or_explicitly_released_lease_restores_capacity() -> TestResult {
        let pool = pool("endpoint-a", vec![("credential-a", 0, 1, 1)])?;
        let credential_id = CredentialId::try_new("credential-a")?;

        let cancelled = pool
            .try_lease()
            .ok_or_else(|| io::Error::other("expected initial lease"))?;
        assert_eq!(pool.active_lease_count(&credential_id), Some(1));
        drop(cancelled);
        assert_eq!(pool.active_lease_count(&credential_id), Some(0));

        let released = pool
            .try_lease()
            .ok_or_else(|| io::Error::other("expected replacement lease"))?;
        released.release();
        assert_eq!(pool.active_lease_count(&credential_id), Some(0));
        Ok(())
    }

    #[test]
    fn diagnostic_snapshot_exposes_kind_expiry_and_live_lease_without_secret() -> TestResult {
        let expired_secret = "expired-codex-secret";
        let current_secret = "current-api-key-secret";
        let pool = EndpointCredentialPool::try_new(
            EndpointId::try_new("endpoint-a")?,
            [
                EndpointCredentialInput {
                    credential_id: CredentialId::try_new("credential-expired")?,
                    credential_kind: "codex_oauth".to_owned(),
                    credential_revision: 7,
                    priority: 0,
                    weight: 1,
                    concurrency: 1,
                    expires_at_ms: Some(100),
                    secret: CredentialSecret::try_new(expired_secret.as_bytes().to_vec())?,
                },
                EndpointCredentialInput {
                    credential_id: CredentialId::try_new("credential-current")?,
                    credential_kind: "api_key".to_owned(),
                    credential_revision: 8,
                    priority: 0,
                    weight: 1,
                    concurrency: 1,
                    expires_at_ms: Some(500),
                    secret: CredentialSecret::try_new(current_secret.as_bytes().to_vec())?,
                },
            ],
        )?;

        let diagnostics = pool.diagnostic_entries();
        let expired = diagnostics
            .iter()
            .find(|entry| entry.credential_id().as_str() == "credential-expired")
            .ok_or_else(|| io::Error::other("expired diagnostic entry missing"))?;
        assert_eq!(expired.credential_kind(), "codex_oauth");
        assert_eq!(expired.credential_revision(), 7);
        assert_eq!(expired.expires_at_ms(), Some(100));
        assert_eq!(expired.active_leases(), 0);
        assert!(!format!("{expired:?}").contains(expired_secret));

        let current = diagnostics
            .iter()
            .find(|entry| entry.credential_id().as_str() == "credential-current")
            .ok_or_else(|| io::Error::other("current diagnostic entry missing"))?;
        assert_eq!(current.credential_kind(), "api_key");
        assert_eq!(current.credential_revision(), 8);
        assert_eq!(current.expires_at_ms(), Some(500));

        let lease = pool
            .try_lease_eligible_at(150, |_| true)
            .ok_or_else(|| io::Error::other("current credential should be leaseable"))?;
        assert_eq!(lease.credential_id().as_str(), "credential-current");
        assert_eq!(
            pool.active_lease_count(&CredentialId::try_new("credential-current")?),
            Some(1)
        );
        let live = pool
            .diagnostic_entries()
            .into_iter()
            .find(|entry| entry.credential_id().as_str() == "credential-current")
            .ok_or_else(|| io::Error::other("live diagnostic entry missing"))?;
        assert_eq!(live.active_leases(), 1);
        drop(lease);
        assert_eq!(
            pool.active_lease_count(&CredentialId::try_new("credential-current")?),
            Some(0)
        );
        let released = pool
            .diagnostic_entries()
            .into_iter()
            .find(|entry| entry.credential_id().as_str() == "credential-current")
            .ok_or_else(|| io::Error::other("released diagnostic entry missing"))?;
        assert_eq!(released.active_leases(), 0);
        assert!(!format!("{current:?}").contains(current_secret));
        Ok(())
    }

    #[test]
    fn eligibility_predicate_skips_one_credential_without_changing_pool_scope() -> TestResult {
        let pool = pool(
            "endpoint-a",
            vec![("credential-a", 0, 1, 1), ("credential-b", 0, 1, 1)],
        )?;
        let credential_a = CredentialId::try_new("credential-a")?;
        let credential_b = CredentialId::try_new("credential-b")?;

        let lease = pool
            .try_lease_eligible(|credential_id| credential_id.as_str() != "credential-a")
            .ok_or_else(|| io::Error::other("expected the eligible Credential lease"))?;
        assert_eq!(lease.credential_id().as_str(), "credential-b");
        assert_eq!(pool.active_lease_count(&credential_a), Some(0));
        assert_eq!(pool.active_lease_count(&credential_b), Some(1));
        drop(lease);
        assert_eq!(pool.active_lease_count(&credential_b), Some(0));
        Ok(())
    }

    #[test]
    fn exact_lease_checks_target_state_without_advancing_weighted_cursor() -> TestResult {
        let entries = vec![("credential-a", 0, 3, 1), ("credential-b", 0, 1, 1)];
        let control = pool("endpoint-a", entries.clone())?;
        let exercised = pool("endpoint-a", entries)?;
        let target = CredentialId::try_new("credential-b")?;

        let control_prefix = control
            .try_lease()
            .ok_or_else(|| io::Error::other("control prefix lease missing"))?;
        let exercised_prefix = exercised
            .try_lease()
            .ok_or_else(|| io::Error::other("exercised prefix lease missing"))?;
        assert_eq!(
            control_prefix.credential_id(),
            exercised_prefix.credential_id()
        );
        drop(control_prefix);
        drop(exercised_prefix);

        assert!(
            exercised
                .try_lease_exact_eligible_at(&target, 100, |_| false)
                .is_none(),
            "caller eligibility must be checked before exact acquisition"
        );
        assert!(
            exercised
                .try_lease_exact_revision_eligible_at(&target, 99, 100, |_| true)
                .is_none(),
            "a stale Credential revision must fail before acquisition"
        );
        let revision_pinned = exercised
            .try_lease_exact_revision_eligible_at(&target, 0, 100, |_| true)
            .ok_or_else(|| io::Error::other("exact revision lease missing"))?;
        assert_eq!(revision_pinned.credential_revision(), 0);
        drop(revision_pinned);
        let pinned = exercised
            .try_lease_exact_eligible_at(&target, 100, |_| true)
            .ok_or_else(|| io::Error::other("exact target lease missing"))?;
        assert_eq!(pinned.credential_id(), &target);
        assert!(
            exercised
                .try_lease_exact_eligible_at(&target, 100, |_| true)
                .is_none(),
            "exact acquisition must preserve the target concurrency ceiling"
        );
        drop(pinned);

        for _ in 0..8 {
            let control_lease = control
                .try_lease()
                .ok_or_else(|| io::Error::other("control weighted lease missing"))?;
            let exercised_lease = exercised
                .try_lease()
                .ok_or_else(|| io::Error::other("exercised weighted lease missing"))?;
            assert_eq!(
                control_lease.credential_id(),
                exercised_lease.credential_id(),
                "exact pin changed the ordinary weighted cursor sequence"
            );
            drop(control_lease);
            drop(exercised_lease);
        }
        Ok(())
    }

    #[test]
    fn concurrent_acquisition_never_exceeds_the_configured_limit() -> TestResult {
        let pool = Arc::new(pool("endpoint-a", vec![("credential-a", 0, 1, 3)])?);
        let worker_count = 16_usize;
        let start = Arc::new(Barrier::new(worker_count + 1));
        let release = Arc::new(Barrier::new(worker_count + 1));
        let (sender, receiver) = mpsc::channel();
        let mut workers = Vec::new();

        for _ in 0..worker_count {
            let pool = Arc::clone(&pool);
            let start = Arc::clone(&start);
            let release = Arc::clone(&release);
            let sender = sender.clone();
            workers.push(thread::spawn(move || {
                start.wait();
                let lease = pool.try_lease();
                let acquired = lease.is_some();
                let _ = sender.send(acquired);
                release.wait();
                drop(lease);
            }));
        }
        drop(sender);

        start.wait();
        let acquired_count = receiver
            .iter()
            .take(worker_count)
            .filter(|acquired| *acquired)
            .count();
        assert_eq!(acquired_count, 3);
        assert_eq!(
            pool.active_lease_count(&CredentialId::try_new("credential-a")?),
            Some(3)
        );
        release.wait();
        for worker in workers {
            worker
                .join()
                .map_err(|_| io::Error::other("Credential worker panicked"))?;
        }
        assert_eq!(
            pool.active_lease_count(&CredentialId::try_new("credential-a")?),
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn rejects_invalid_or_unbounded_pool_input_without_rendering_secrets() -> TestResult {
        let oversized = pool(
            "endpoint-a",
            vec![(
                "credential-a",
                0,
                i64::try_from(MAX_CREDENTIAL_SCHEDULE_SLOTS_PER_PRIORITY_TIER + 1)?,
                1,
            )],
        );
        let Err(error) = oversized else {
            return Err("oversized Credential pool unexpectedly succeeded".into());
        };
        assert!(matches!(
            error.downcast_ref::<CredentialPoolBuildError>(),
            Some(CredentialPoolBuildError::CredentialScheduleTooLarge)
        ));

        let secret = "synthetic-credential-secret";
        let input = EndpointCredentialInput {
            credential_id: CredentialId::try_new("credential-a")?,
            credential_kind: "api_key".to_owned(),
            credential_revision: 0,
            priority: 0,
            weight: 1,
            concurrency: 1,
            expires_at_ms: None,
            secret: CredentialSecret::try_new(secret.as_bytes().to_vec())?,
        };
        assert!(!format!("{input:?}").contains(secret));
        Ok(())
    }

    fn pool(
        endpoint_id: &str,
        entries: Vec<(&str, i64, i64, i64)>,
    ) -> Result<EndpointCredentialPool, Box<dyn Error>> {
        let entries = entries
            .into_iter()
            .map(|(credential_id, priority, weight, concurrency)| {
                Ok(EndpointCredentialInput {
                    credential_id: CredentialId::try_new(credential_id)?,
                    credential_kind: "api_key".to_owned(),
                    credential_revision: 0,
                    priority,
                    weight,
                    concurrency,
                    expires_at_ms: None,
                    secret: CredentialSecret::try_new(
                        format!("synthetic-{credential_id}").into_bytes(),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(EndpointCredentialPool::try_new(
            EndpointId::try_new(endpoint_id)?,
            entries,
        )?)
    }
}
