//! Per-Credential Kiro model discovery and subscription-capability snapshots.
//!
//! Kiro exposes a Credential's currently entitled models and its subscription information through
//! separate upstream operations. This module deliberately receives already-bounded response bytes
//! through an injected probe rather than owning HTTP, refresh, retry, or endpoint construction.
//! A success is retained per Credential; one failed Credential can reuse only its own eligible
//! last success and cannot erase or contaminate another Credential's model list.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::{Mutex, MutexGuard},
};

use gateway_catalog::{CatalogFreshnessPolicy, CatalogSnapshotFreshness};
use gateway_core::CredentialId;
use serde_json::Value;

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_MODELS_PER_CREDENTIAL: usize = 256;
const MAX_MODEL_ID_BYTES: usize = 256;
const MAX_SUBSCRIPTION_TITLE_BYTES: usize = 256;

/// One normalized dynamic model returned for a Kiro Credential.
///
/// The model identity is preserved exactly after surrounding ASCII/Unicode whitespace is trimmed.
/// This boundary never creates a virtual `-thinking` model: Thinking is a request capability owned
/// by P7-07, so the final union contains only discovered source IDs.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct KiroDynamicModel {
    model_id: String,
    max_input_tokens: Option<u64>,
}

impl KiroDynamicModel {
    /// Validates one bounded Kiro source model and its optional input-token limit.
    ///
    /// # Errors
    ///
    /// Returns [`KiroDynamicCatalogError::InvalidResponse`] when the source model ID is empty,
    /// overlong, or contains a control character.
    pub fn try_new(
        model_id: impl Into<String>,
        max_input_tokens: Option<u64>,
    ) -> Result<Self, KiroDynamicCatalogError> {
        let model_id = model_id.into();
        let model_id = model_id.trim();
        if model_id.is_empty()
            || model_id.len() > MAX_MODEL_ID_BYTES
            || model_id.chars().any(char::is_control)
        {
            return Err(KiroDynamicCatalogError::InvalidResponse);
        }
        Ok(Self {
            model_id: model_id.to_owned(),
            max_input_tokens,
        })
    }

    /// Returns the exact normalized Kiro source model ID.
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Returns the source-advertised maximum input-token limit, if present and valid.
    #[must_use]
    pub const fn max_input_tokens(&self) -> Option<u64> {
        self.max_input_tokens
    }
}

impl fmt::Debug for KiroDynamicModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroDynamicModel")
            .field("model_id", &"<redacted>")
            .field("max_input_tokens", &self.max_input_tokens)
            .finish()
    }
}

/// Coarse, safe subscription classification derived from a transient Kiro subscription title.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroSubscriptionPlan {
    /// The source explicitly identified a free plan.
    Free,
    /// The source explicitly identified a recognized paid Kiro plan.
    Paid,
    /// The source omitted or used an unrecognized subscription title.
    Unknown,
}

/// Whether Kiro explicitly advertised overage support for a Credential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroOverageCapability {
    /// The source explicitly advertised overage support.
    Supported,
    /// The source explicitly advertised no overage support.
    Unsupported,
    /// The source omitted or used an unrecognized overage value.
    Unknown,
}

/// A redacted, scheduler-safe projection of one Credential's subscription capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KiroSubscriptionCapabilities {
    plan: KiroSubscriptionPlan,
    overage: KiroOverageCapability,
}

impl KiroSubscriptionCapabilities {
    /// Returns a projection with no source subscription evidence.
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            plan: KiroSubscriptionPlan::Unknown,
            overage: KiroOverageCapability::Unknown,
        }
    }

    /// Parses only the safe subscription projection from a bounded `getUsageLimits` response.
    ///
    /// The raw subscription title is intentionally discarded. Unrecognized values remain
    /// [`KiroSubscriptionPlan::Unknown`] instead of being promoted to paid entitlement.
    ///
    /// # Errors
    ///
    /// Returns [`KiroDynamicCatalogError::InvalidResponse`] for a malformed JSON root or fields
    /// with the wrong type. A missing `subscriptionInfo` is valid and produces `Unknown` values.
    pub fn from_usage_json(input: &[u8]) -> Result<Self, KiroDynamicCatalogError> {
        let object = parse_json_object(input)?;
        let Some(subscription_info) = object.get("subscriptionInfo") else {
            return Ok(Self::unknown());
        };
        let subscription_info = subscription_info
            .as_object()
            .ok_or(KiroDynamicCatalogError::InvalidResponse)?;

        let plan = match subscription_info.get("subscriptionTitle") {
            None | Some(Value::Null) => KiroSubscriptionPlan::Unknown,
            Some(Value::String(value)) => subscription_plan(value)?,
            Some(_) => return Err(KiroDynamicCatalogError::InvalidResponse),
        };
        let overage = match subscription_info.get("overageCapability") {
            None | Some(Value::Null) => KiroOverageCapability::Unknown,
            Some(Value::String(value)) => match value.trim() {
                "OVERAGE_CAPABLE" => KiroOverageCapability::Supported,
                "NOT_OVERAGE_CAPABLE" | "NOT_AVAILABLE" => KiroOverageCapability::Unsupported,
                _ => KiroOverageCapability::Unknown,
            },
            Some(_) => return Err(KiroDynamicCatalogError::InvalidResponse),
        };
        Ok(Self { plan, overage })
    }

    /// Returns the safe plan classification.
    #[must_use]
    pub const fn plan(self) -> KiroSubscriptionPlan {
        self.plan
    }

    /// Returns the safe overage-support classification.
    #[must_use]
    pub const fn overage(self) -> KiroOverageCapability {
        self.overage
    }
}

/// One complete, time-stamped Credential capability observation.
#[derive(Clone, Eq, PartialEq)]
pub struct KiroCredentialCapabilityObservation {
    models: Vec<KiroDynamicModel>,
    subscription: KiroSubscriptionCapabilities,
    observed_at_ms: i64,
}

impl KiroCredentialCapabilityObservation {
    /// Parses paired bounded Kiro model and usage responses into one immutable observation.
    ///
    /// The caller is responsible for making both source operations against the same current
    /// Credential. Neither response bytes nor the subscription title are retained after parsing.
    ///
    /// # Errors
    ///
    /// Returns a safe response or timestamp classification and leaves existing snapshots untouched.
    pub fn from_json(
        models_response: &[u8],
        usage_response: &[u8],
        observed_at_ms: i64,
    ) -> Result<Self, KiroDynamicCatalogError> {
        let models = parse_models_response(models_response)?;
        let subscription = KiroSubscriptionCapabilities::from_usage_json(usage_response)?;
        Self::try_new(models, subscription, observed_at_ms)
    }

    /// Validates one already-decoded complete observation.
    ///
    /// # Errors
    ///
    /// Returns a safe error for an invalid timestamp, too many models, or duplicate model IDs
    /// with conflicting source limits.
    pub fn try_new(
        models: impl IntoIterator<Item = KiroDynamicModel>,
        subscription: KiroSubscriptionCapabilities,
        observed_at_ms: i64,
    ) -> Result<Self, KiroDynamicCatalogError> {
        if observed_at_ms < 0 {
            return Err(KiroDynamicCatalogError::TimestampBeforeUnixEpoch);
        }
        let models = normalize_models(models.into_iter().collect())?;
        Ok(Self {
            models,
            subscription,
            observed_at_ms,
        })
    }

    /// Returns normalized models in deterministic source-ID order.
    #[must_use]
    pub fn models(&self) -> &[KiroDynamicModel] {
        &self.models
    }

    /// Returns the raw-title-free subscription projection.
    #[must_use]
    pub const fn subscription(&self) -> KiroSubscriptionCapabilities {
        self.subscription
    }

    /// Returns the explicit Unix-millisecond source-observation time.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }
}

impl fmt::Debug for KiroCredentialCapabilityObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroCredentialCapabilityObservation")
            .field("model_count", &self.models.len())
            .field("subscription", &self.subscription)
            .field("observed_at_ms", &self.observed_at_ms)
            .finish()
    }
}

/// Injected paired Kiro capability lookup for one exact Credential.
///
/// Implementations own endpoint construction, auth injection, I/O, and retries. They must not
/// return raw bodies or error text through this boundary. P7-08 owns network failure
/// classification; P7-09 supplies the bounded real adapter.
pub trait KiroCredentialCapabilityProbe {
    /// Looks up one current paired model/subscription observation without implicit failover.
    ///
    /// # Errors
    ///
    /// Returns one safe, content-free failure classification. The caller will preserve only that
    /// Credential's eligible last success and continue with every other Credential.
    fn discover(
        &self,
        credential_id: &CredentialId,
    ) -> Result<KiroCredentialCapabilityObservation, KiroDynamicCatalogError>;
}

/// Immutable last-success capability result for one exact Credential.
#[derive(Clone, Eq, PartialEq)]
pub struct KiroCredentialCapabilitySnapshot {
    credential_id: CredentialId,
    models: Vec<KiroDynamicModel>,
    subscription: KiroSubscriptionCapabilities,
    version: u64,
    observed_at_ms: i64,
    stale_at_ms: i64,
    refresh_due_at_ms: i64,
    expires_at_ms: i64,
}

impl KiroCredentialCapabilitySnapshot {
    fn try_new(
        credential_id: CredentialId,
        observation: KiroCredentialCapabilityObservation,
        version: u64,
        policy: CatalogFreshnessPolicy,
    ) -> Result<Self, KiroDynamicCatalogError> {
        if version == 0 {
            return Err(KiroDynamicCatalogError::SnapshotVersionOverflow);
        }
        let observed_at_ms = observation.observed_at_ms;
        let stale_at_ms = checked_deadline(observed_at_ms, policy.fresh_for_ms())?;
        let refresh_due_at_ms = checked_deadline(observed_at_ms, policy.refresh_due_after_ms())?;
        let expires_at_ms = checked_deadline(observed_at_ms, policy.expires_after_ms())?;
        Ok(Self {
            credential_id,
            models: observation.models,
            subscription: observation.subscription,
            version,
            observed_at_ms,
            stale_at_ms,
            refresh_due_at_ms,
            expires_at_ms,
        })
    }

    /// Returns the exact Credential that owns this source success.
    #[must_use]
    pub fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns deterministic model IDs discovered for exactly this Credential.
    #[must_use]
    pub fn models(&self) -> &[KiroDynamicModel] {
        &self.models
    }

    /// Returns the associated raw-title-free subscription projection.
    #[must_use]
    pub const fn subscription(&self) -> KiroSubscriptionCapabilities {
        self.subscription
    }

    /// Returns this Credential-local monotonically increasing success version.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Returns the explicit source-observation time.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Returns the first instant at which this success is retained as stale.
    #[must_use]
    pub const fn stale_at_ms(&self) -> i64 {
        self.stale_at_ms
    }

    /// Returns the independent background-refresh deadline.
    #[must_use]
    pub const fn refresh_due_at_ms(&self) -> i64 {
        self.refresh_due_at_ms
    }

    /// Returns the hard last-success retention deadline.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }

    /// Evaluates the P4 freshness policy without consulting an ambient clock.
    ///
    /// # Errors
    ///
    /// Returns [`KiroDynamicCatalogError::ClockBeforeSnapshot`] when `now_ms` precedes this
    /// snapshot's source observation.
    pub fn freshness_at(
        &self,
        now_ms: i64,
    ) -> Result<CatalogSnapshotFreshness, KiroDynamicCatalogError> {
        if now_ms < self.observed_at_ms {
            return Err(KiroDynamicCatalogError::ClockBeforeSnapshot);
        }
        if now_ms < self.stale_at_ms {
            Ok(CatalogSnapshotFreshness::Fresh)
        } else if now_ms < self.expires_at_ms {
            Ok(CatalogSnapshotFreshness::Stale)
        } else {
            Ok(CatalogSnapshotFreshness::Expired)
        }
    }

    /// Returns whether background refresh is due at one explicit time.
    ///
    /// # Errors
    ///
    /// Returns [`KiroDynamicCatalogError::ClockBeforeSnapshot`] for a non-monotonic caller clock.
    pub fn is_refresh_due_at(&self, now_ms: i64) -> Result<bool, KiroDynamicCatalogError> {
        if now_ms < self.observed_at_ms {
            return Err(KiroDynamicCatalogError::ClockBeforeSnapshot);
        }
        Ok(now_ms >= self.refresh_due_at_ms)
    }
}

impl fmt::Debug for KiroCredentialCapabilitySnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroCredentialCapabilitySnapshot")
            .field("credential_id", &self.credential_id)
            .field("model_count", &self.models.len())
            .field("subscription", &self.subscription)
            .field("version", &self.version)
            .field("observed_at_ms", &self.observed_at_ms)
            .field("stale_at_ms", &self.stale_at_ms)
            .field("refresh_due_at_ms", &self.refresh_due_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

/// One capability snapshot evaluated at an explicit time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KiroCredentialCapabilityStatus {
    snapshot: KiroCredentialCapabilitySnapshot,
    freshness: CatalogSnapshotFreshness,
    refresh_due: bool,
}

impl KiroCredentialCapabilityStatus {
    fn at(
        snapshot: KiroCredentialCapabilitySnapshot,
        now_ms: i64,
    ) -> Result<Self, KiroDynamicCatalogError> {
        Ok(Self {
            freshness: snapshot.freshness_at(now_ms)?,
            refresh_due: snapshot.is_refresh_due_at(now_ms)?,
            snapshot,
        })
    }

    /// Returns the immutable last-success source snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &KiroCredentialCapabilitySnapshot {
        &self.snapshot
    }

    /// Returns `Fresh`, `Stale`, or `Expired` at the supplied time.
    #[must_use]
    pub const fn freshness(&self) -> CatalogSnapshotFreshness {
        self.freshness
    }

    /// Returns whether a background capability refresh is due.
    #[must_use]
    pub const fn is_refresh_due(&self) -> bool {
        self.refresh_due
    }
}

/// Process-local registry of atomic per-Credential model and subscription successes.
///
/// The implementation follows P4-02's explicit Fresh/Stale/Expired deadlines and non-mutating
/// last-success fallback, while retaining the Kiro-specific subscription projection in the same
/// immutable success. No failed observation or upstream error is stored.
pub struct KiroCredentialCapabilityStore {
    policy: CatalogFreshnessPolicy,
    snapshots: Mutex<BTreeMap<CredentialId, KiroCredentialCapabilitySnapshot>>,
}

impl Default for KiroCredentialCapabilityStore {
    fn default() -> Self {
        Self::new(CatalogFreshnessPolicy::default())
    }
}

impl KiroCredentialCapabilityStore {
    /// Creates an empty registry with an already-validated P4 catalog freshness policy.
    #[must_use]
    pub fn new(policy: CatalogFreshnessPolicy) -> Self {
        Self {
            policy,
            snapshots: Mutex::new(BTreeMap::new()),
        }
    }

    /// Returns the immutable P4 freshness policy used for all Kiro last-success values.
    #[must_use]
    pub const fn policy(&self) -> CatalogFreshnessPolicy {
        self.policy
    }

    /// Atomically records one complete successful observation for exactly one Credential.
    ///
    /// A success can replace only its same-Credential predecessor. Its observation time must not
    /// move backwards, so an old in-flight result cannot erase newer dynamic entitlement evidence.
    ///
    /// # Errors
    ///
    /// Returns a safe timestamp, version, or registry classification without changing a prior
    /// snapshot on error.
    pub fn record_success(
        &self,
        credential_id: CredentialId,
        observation: KiroCredentialCapabilityObservation,
    ) -> Result<KiroCredentialCapabilitySnapshot, KiroDynamicCatalogError> {
        let mut snapshots = self.lock_snapshots()?;
        let version = match snapshots.get(&credential_id) {
            Some(previous) => {
                if observation.observed_at_ms < previous.observed_at_ms {
                    return Err(KiroDynamicCatalogError::TimestampNotMonotonic);
                }
                previous
                    .version
                    .checked_add(1)
                    .ok_or(KiroDynamicCatalogError::SnapshotVersionOverflow)?
            }
            None => 1,
        };
        let snapshot = KiroCredentialCapabilitySnapshot::try_new(
            credential_id.clone(),
            observation,
            version,
            self.policy,
        )?;
        snapshots.insert(credential_id, snapshot.clone());
        Ok(snapshot)
    }

    /// Returns exactly one Credential's retained last success after an independent probe failure.
    ///
    /// This operation is intentionally non-mutating: a source failure never clears Models,
    /// changes subscription capabilities, records error text, or affects another Credential.
    ///
    /// # Errors
    ///
    /// Returns [`KiroDynamicCatalogError::StoreUnavailable`] if the process-local registry cannot
    /// be read safely.
    pub fn retain_last_success_on_failure(
        &self,
        credential_id: &CredentialId,
    ) -> Result<Option<KiroCredentialCapabilitySnapshot>, KiroDynamicCatalogError> {
        Ok(self.lock_snapshots()?.get(credential_id).cloned())
    }

    /// Returns the unclassified immutable success for exactly one Credential.
    ///
    /// # Errors
    ///
    /// Returns [`KiroDynamicCatalogError::StoreUnavailable`] if the registry cannot be read.
    pub fn last_success(
        &self,
        credential_id: &CredentialId,
    ) -> Result<Option<KiroCredentialCapabilitySnapshot>, KiroDynamicCatalogError> {
        Ok(self.lock_snapshots()?.get(credential_id).cloned())
    }

    /// Returns a last success plus its explicit freshness state for one Credential.
    ///
    /// # Errors
    ///
    /// Returns a safe registry or non-monotonic-clock classification; an absent Credential is not
    /// an error.
    pub fn status_at(
        &self,
        credential_id: &CredentialId,
        now_ms: i64,
    ) -> Result<Option<KiroCredentialCapabilityStatus>, KiroDynamicCatalogError> {
        self.last_success(credential_id)?
            .map(|snapshot| KiroCredentialCapabilityStatus::at(snapshot, now_ms))
            .transpose()
    }

    /// Discovers each supplied Credential once and returns a deterministic, failure-isolated union.
    ///
    /// A failed Credential contributes only its own non-expired retained success. Other
    /// Credentials continue normally, and source model IDs are deduplicated without generating
    /// synthetic `-thinking` variants. If every lookup fails and no eligible last success exists,
    /// the result is an empty union with a nonzero unavailable count rather than a leaked error.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate input IDs, a caller clock before a successful observation,
    /// unsafe snapshot state, or an unavailable registry. Per-Credential probe failures are
    /// intentionally isolated in the returned counters.
    pub fn aggregate<P: KiroCredentialCapabilityProbe>(
        &self,
        credential_ids: impl IntoIterator<Item = CredentialId>,
        now_ms: i64,
        probe: &P,
    ) -> Result<KiroCapabilityAggregate, KiroDynamicCatalogError> {
        let credential_ids: Vec<_> = credential_ids.into_iter().collect();
        if credential_ids.is_empty() {
            return Err(KiroDynamicCatalogError::NoCredentials);
        }
        let unique: BTreeSet<_> = credential_ids.iter().cloned().collect();
        if unique.len() != credential_ids.len() {
            return Err(KiroDynamicCatalogError::DuplicateCredentialId);
        }

        let mut current_successes = 0_usize;
        let mut retained_successes = 0_usize;
        let mut unavailable_credentials = 0_usize;
        let mut snapshots = Vec::new();

        for credential_id in credential_ids {
            match probe.discover(&credential_id) {
                Ok(observation) => {
                    if observation.observed_at_ms() > now_ms {
                        return Err(KiroDynamicCatalogError::ClockBeforeSnapshot);
                    }
                    let snapshot = self.record_success(credential_id, observation)?;
                    current_successes = current_successes.saturating_add(1);
                    snapshots.push(KiroCredentialCapabilityStatus::at(snapshot, now_ms)?);
                }
                Err(_) => match self.status_at(&credential_id, now_ms)? {
                    Some(status) if status.freshness().is_hard_eligible() => {
                        retained_successes = retained_successes.saturating_add(1);
                        snapshots.push(status);
                    }
                    Some(_) | None => {
                        unavailable_credentials = unavailable_credentials.saturating_add(1);
                    }
                },
            }
        }

        let models = aggregate_models(&snapshots);
        Ok(KiroCapabilityAggregate {
            models,
            credential_statuses: snapshots,
            current_successes,
            retained_successes,
            unavailable_credentials,
        })
    }

    fn lock_snapshots(
        &self,
    ) -> Result<
        MutexGuard<'_, BTreeMap<CredentialId, KiroCredentialCapabilitySnapshot>>,
        KiroDynamicCatalogError,
    > {
        self.snapshots
            .lock()
            .map_err(|_| KiroDynamicCatalogError::StoreUnavailable)
    }
}

impl fmt::Debug for KiroCredentialCapabilityStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.snapshots.lock().map_or(0, |snapshots| snapshots.len());
        formatter
            .debug_struct("KiroCredentialCapabilityStore")
            .field("policy", &self.policy)
            .field("credential_snapshot_count", &count)
            .finish()
    }
}

/// One source-model capability after unioning all usable Credential snapshots.
#[derive(Clone, Eq, PartialEq)]
pub struct KiroUnionModelCapability {
    model: KiroDynamicModel,
    eligible_credential_count: usize,
}

impl KiroUnionModelCapability {
    /// Returns the source model. It is never a generated Thinking alias.
    #[must_use]
    pub fn model(&self) -> &KiroDynamicModel {
        &self.model
    }

    /// Returns how many current or eligible-stale Credentials explicitly listed this model.
    #[must_use]
    pub const fn eligible_credential_count(&self) -> usize {
        self.eligible_credential_count
    }
}

impl fmt::Debug for KiroUnionModelCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroUnionModelCapability")
            .field("model", &self.model)
            .field("eligible_credential_count", &self.eligible_credential_count)
            .finish()
    }
}

/// A deterministic Kiro capability union plus failure-isolation accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KiroCapabilityAggregate {
    models: Vec<KiroUnionModelCapability>,
    credential_statuses: Vec<KiroCredentialCapabilityStatus>,
    current_successes: usize,
    retained_successes: usize,
    unavailable_credentials: usize,
}

impl KiroCapabilityAggregate {
    /// Returns source models in deterministic ID order with no synthetic Thinking aliases.
    #[must_use]
    pub fn models(&self) -> &[KiroUnionModelCapability] {
        &self.models
    }

    /// Returns one non-expired status per Credential that contributed to this union.
    #[must_use]
    pub fn credential_statuses(&self) -> &[KiroCredentialCapabilityStatus] {
        &self.credential_statuses
    }

    /// Returns the number of Credentials whose current paired probe succeeded.
    #[must_use]
    pub const fn current_successes(&self) -> usize {
        self.current_successes
    }

    /// Returns the number of failed probes that reused an eligible exact-Credential last success.
    #[must_use]
    pub const fn retained_successes(&self) -> usize {
        self.retained_successes
    }

    /// Returns the number of Credentials that could not contribute current or eligible stale data.
    #[must_use]
    pub const fn unavailable_credentials(&self) -> usize {
        self.unavailable_credentials
    }
}

/// Safe Kiro dynamic-catalog failures. They contain no upstream body, URL, token, or title.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroDynamicCatalogError {
    /// A response is oversized, malformed, or structurally invalid.
    InvalidResponse,
    /// A source observation time is before the Unix epoch.
    TimestampBeforeUnixEpoch,
    /// A successful source observation was older than this Credential's retained success.
    TimestampNotMonotonic,
    /// A caller evaluated a snapshot before it was observed.
    ClockBeforeSnapshot,
    /// A freshness deadline cannot be represented safely.
    TimestampOverflow,
    /// A Credential-local snapshot version cannot advance safely.
    SnapshotVersionOverflow,
    /// The same Credential was supplied to one aggregate more than once.
    DuplicateCredentialId,
    /// Capability aggregation requires at least one explicit Credential.
    NoCredentials,
    /// The process-local snapshot registry is unavailable.
    StoreUnavailable,
}

impl fmt::Display for KiroDynamicCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidResponse => "Kiro dynamic catalog response is invalid",
            Self::TimestampBeforeUnixEpoch => {
                "Kiro dynamic catalog timestamp is before the Unix epoch"
            }
            Self::TimestampNotMonotonic => {
                "Kiro dynamic catalog success precedes the retained last success"
            }
            Self::ClockBeforeSnapshot => {
                "Kiro dynamic catalog clock precedes the retained snapshot"
            }
            Self::TimestampOverflow => "Kiro dynamic catalog deadline cannot be represented safely",
            Self::SnapshotVersionOverflow => "Kiro dynamic catalog version cannot advance safely",
            Self::DuplicateCredentialId => "Kiro dynamic catalog Credential ID is duplicated",
            Self::NoCredentials => "Kiro dynamic catalog needs at least one Credential",
            Self::StoreUnavailable => "Kiro dynamic catalog snapshot registry is unavailable",
        })
    }
}

impl Error for KiroDynamicCatalogError {}

fn parse_models_response(input: &[u8]) -> Result<Vec<KiroDynamicModel>, KiroDynamicCatalogError> {
    let object = parse_json_object(input)?;
    let Some(models) = object.get("models") else {
        return Ok(Vec::new());
    };
    let models = models
        .as_array()
        .ok_or(KiroDynamicCatalogError::InvalidResponse)?;
    if models.len() > MAX_MODELS_PER_CREDENTIAL {
        return Err(KiroDynamicCatalogError::InvalidResponse);
    }
    let mut result = Vec::with_capacity(models.len());
    for model in models {
        let model = model
            .as_object()
            .ok_or(KiroDynamicCatalogError::InvalidResponse)?;
        let model_id = model
            .get("modelId")
            .and_then(Value::as_str)
            .ok_or(KiroDynamicCatalogError::InvalidResponse)?;
        let max_input_tokens = match model.get("tokenLimits") {
            None | Some(Value::Null) => None,
            Some(Value::Object(token_limits)) => match token_limits.get("maxInputTokens") {
                None | Some(Value::Null) => None,
                Some(value) => value
                    .as_u64()
                    .ok_or(KiroDynamicCatalogError::InvalidResponse)
                    .map(Some)?,
            },
            Some(_) => return Err(KiroDynamicCatalogError::InvalidResponse),
        };
        result.push(KiroDynamicModel::try_new(model_id, max_input_tokens)?);
    }
    normalize_models(result)
}

fn parse_json_object(
    input: &[u8],
) -> Result<serde_json::Map<String, Value>, KiroDynamicCatalogError> {
    if input.len() > MAX_RESPONSE_BYTES {
        return Err(KiroDynamicCatalogError::InvalidResponse);
    }
    let value: Value =
        serde_json::from_slice(input).map_err(|_| KiroDynamicCatalogError::InvalidResponse)?;
    value
        .as_object()
        .cloned()
        .ok_or(KiroDynamicCatalogError::InvalidResponse)
}

fn subscription_plan(value: &str) -> Result<KiroSubscriptionPlan, KiroDynamicCatalogError> {
    if value.len() > MAX_SUBSCRIPTION_TITLE_BYTES || value.chars().any(char::is_control) {
        return Err(KiroDynamicCatalogError::InvalidResponse);
    }
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.contains("FREE") {
        Ok(KiroSubscriptionPlan::Free)
    } else if normalized.contains("PRO") {
        Ok(KiroSubscriptionPlan::Paid)
    } else {
        Ok(KiroSubscriptionPlan::Unknown)
    }
}

fn normalize_models(
    models: Vec<KiroDynamicModel>,
) -> Result<Vec<KiroDynamicModel>, KiroDynamicCatalogError> {
    if models.len() > MAX_MODELS_PER_CREDENTIAL {
        return Err(KiroDynamicCatalogError::InvalidResponse);
    }
    let mut normalized = BTreeMap::new();
    for model in models {
        match normalized.entry(model.model_id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(model);
            }
            std::collections::btree_map::Entry::Occupied(entry)
                if entry.get().max_input_tokens == model.max_input_tokens => {}
            std::collections::btree_map::Entry::Occupied(_) => {
                return Err(KiroDynamicCatalogError::InvalidResponse);
            }
        }
    }
    Ok(normalized.into_values().collect())
}

fn checked_deadline(observed_at_ms: i64, duration_ms: i64) -> Result<i64, KiroDynamicCatalogError> {
    observed_at_ms
        .checked_add(duration_ms)
        .ok_or(KiroDynamicCatalogError::TimestampOverflow)
}

fn aggregate_models(snapshots: &[KiroCredentialCapabilityStatus]) -> Vec<KiroUnionModelCapability> {
    let mut models: BTreeMap<String, (KiroDynamicModel, usize)> = BTreeMap::new();
    for status in snapshots {
        for model in status.snapshot().models() {
            models
                .entry(model.model_id().to_owned())
                .and_modify(|(existing, count)| {
                    *count = count.saturating_add(1);
                    if existing.max_input_tokens != model.max_input_tokens() {
                        // A union must never advertise the larger of two incompatible source
                        // limits. `None` carries the safe "not globally known" projection.
                        existing.max_input_tokens = None;
                    }
                })
                .or_insert_with(|| (model.clone(), 1));
        }
    }
    models
        .into_values()
        .map(
            |(model, eligible_credential_count)| KiroUnionModelCapability {
                model,
                eligible_credential_count,
            },
        )
        .collect()
}
