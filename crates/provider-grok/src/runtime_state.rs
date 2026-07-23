//! Durable Grok Build catalog, billing, quota, and failure-classification state.
//!
//! This module intentionally separates source/confidence-bearing account observations from the
//! request decoder. It is also the only P6 module that maps bounded HTTP evidence into a gateway
//! remediation scope; decoding itself remains side-effect free.

use std::{collections::BTreeSet, error::Error, fmt, path::Path, sync::Mutex};

use gateway_core::{CredentialId, ErrorScope, GatewayError, GatewayErrorCode};
use rusqlite::{Connection, OptionalExtension, params};

use crate::GrokBuildResponsesErrorSignal;

const MAX_MODEL_BYTES: usize = 512;
const MAX_WINDOW_TYPE_BYTES: usize = 128;
const MAX_CREDENTIAL_ID_BYTES: usize = 128;

/// The only Build billing plan categories represented by this Provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildBillingPlan {
    /// Included/free Build allowance.
    Free,
    /// Metered Build usage with an on-demand cap.
    PayAsYouGo,
    /// A subscription with a calendar billing allowance.
    Subscription,
}

impl GrokBuildBillingPlan {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::PayAsYouGo => "pay_as_you_go",
            Self::Subscription => "subscription",
        }
    }

    fn from_sql(value: &str) -> Result<Self, GrokBuildRuntimeStateError> {
        match value {
            "free" => Ok(Self::Free),
            "pay_as_you_go" => Ok(Self::PayAsYouGo),
            "subscription" => Ok(Self::Subscription),
            _ => Err(GrokBuildRuntimeStateError::InvalidPersistedState),
        }
    }
}

/// The upstream observation source for one Build model capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildModelSource {
    /// The account's supported-model capability snapshot.
    AccountCapability,
    /// A successful Build response that explicitly identifies the model.
    BuildResponse,
}

impl GrokBuildModelSource {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::AccountCapability => "account_capability",
            Self::BuildResponse => "build_response",
        }
    }

    fn from_sql(value: &str) -> Result<Self, GrokBuildRuntimeStateError> {
        match value {
            "account_capability" => Ok(Self::AccountCapability),
            "build_response" => Ok(Self::BuildResponse),
            _ => Err(GrokBuildRuntimeStateError::InvalidPersistedState),
        }
    }
}

/// One bounded, time-stamped Build model capability observation.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokBuildModelCapability {
    upstream_model: String,
    source: GrokBuildModelSource,
    observed_at_ms: i64,
}

impl GrokBuildModelCapability {
    /// Creates one model capability without rendering the upstream model in diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildRuntimeStateError::InvalidOpaqueValue`] for an invalid model label.
    pub fn try_new(
        upstream_model: impl Into<String>,
        source: GrokBuildModelSource,
        observed_at_ms: i64,
    ) -> Result<Self, GrokBuildRuntimeStateError> {
        if observed_at_ms <= 0 {
            return Err(GrokBuildRuntimeStateError::InvalidCatalogSnapshot);
        }
        let upstream_model = validate_text(upstream_model.into(), MAX_MODEL_BYTES)?;
        Ok(Self {
            upstream_model,
            source,
            observed_at_ms,
        })
    }

    /// Returns the upstream model only to the immediate Provider caller.
    #[must_use]
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// Returns the capability source.
    #[must_use]
    pub const fn source(&self) -> GrokBuildModelSource {
        self.source
    }

    /// Returns the source observation time.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }
}

impl fmt::Debug for GrokBuildModelCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildModelCapability")
            .field("upstream_model", &"<redacted>")
            .field("source", &self.source)
            .field("observed_at_ms", &self.observed_at_ms)
            .finish()
    }
}

/// The upstream source from which a quota value was observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildQuotaSource {
    /// A Build billing endpoint supplied the observation.
    Billing,
    /// A Build response header supplied the observation.
    ResponseHeaders,
    /// A Web REST endpoint supplied the observation.
    WebRest,
    /// A Web gRPC-Web endpoint supplied the observation.
    WebGrpcWeb,
    /// A local arithmetic estimate, never presented as upstream fact.
    LocalEstimate,
}

impl GrokBuildQuotaSource {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Billing => "billing",
            Self::ResponseHeaders => "response_headers",
            Self::WebRest => "web_rest",
            Self::WebGrpcWeb => "web_grpc_web",
            Self::LocalEstimate => "local_estimate",
        }
    }

    fn from_sql(value: &str) -> Result<Self, GrokBuildRuntimeStateError> {
        match value {
            "billing" => Ok(Self::Billing),
            "response_headers" => Ok(Self::ResponseHeaders),
            "web_rest" => Ok(Self::WebRest),
            "web_grpc_web" => Ok(Self::WebGrpcWeb),
            "local_estimate" => Ok(Self::LocalEstimate),
            _ => Err(GrokBuildRuntimeStateError::InvalidPersistedState),
        }
    }

    const fn required_confidence(self) -> GrokBuildQuotaConfidence {
        match self {
            Self::Billing => GrokBuildQuotaConfidence::Authoritative,
            Self::ResponseHeaders | Self::WebRest | Self::WebGrpcWeb => {
                GrokBuildQuotaConfidence::Observed
            }
            Self::LocalEstimate => GrokBuildQuotaConfidence::Estimated,
        }
    }
}

/// The confidence attached to an explicit Build quota source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildQuotaConfidence {
    /// The source owns the reported billing value.
    Authoritative,
    /// The source observed a provider-reported value without owning billing truth.
    Observed,
    /// The value is a local estimate and cannot masquerade as upstream truth.
    Estimated,
}

impl GrokBuildQuotaConfidence {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Authoritative => "authoritative",
            Self::Observed => "observed",
            Self::Estimated => "estimated",
        }
    }

    fn from_sql(value: &str) -> Result<Self, GrokBuildRuntimeStateError> {
        match value {
            "authoritative" => Ok(Self::Authoritative),
            "observed" => Ok(Self::Observed),
            "estimated" => Ok(Self::Estimated),
            _ => Err(GrokBuildRuntimeStateError::InvalidPersistedState),
        }
    }
}

/// A distinct Build quota window; no window kind is inferred from another kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GrokBuildQuotaWindowKind {
    /// Included free usage.
    Free,
    /// Pay-as-you-go on-demand capacity.
    PayAsYouGo,
    /// Subscription month allowance.
    SubscriptionMonthly,
    /// A Web weekly allowance, retained distinctly from Build billing.
    WebWeekly,
}

impl GrokBuildQuotaWindowKind {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::PayAsYouGo => "pay_as_you_go",
            Self::SubscriptionMonthly => "subscription_monthly",
            Self::WebWeekly => "web_weekly",
        }
    }

    fn from_sql(value: &str) -> Result<Self, GrokBuildRuntimeStateError> {
        match value {
            "free" => Ok(Self::Free),
            "pay_as_you_go" => Ok(Self::PayAsYouGo),
            "subscription_monthly" => Ok(Self::SubscriptionMonthly),
            "web_weekly" => Ok(Self::WebWeekly),
            _ => Err(GrokBuildRuntimeStateError::InvalidPersistedState),
        }
    }
}

/// One source-labelled and reset-aware quota snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokBuildQuotaWindow {
    kind: GrokBuildQuotaWindowKind,
    remaining: u64,
    total: u64,
    window_seconds: u64,
    reset_at_ms: i64,
    observed_at_ms: i64,
    source: GrokBuildQuotaSource,
    confidence: GrokBuildQuotaConfidence,
    raw_window_type: String,
}

impl GrokBuildQuotaWindow {
    /// Creates a quota snapshot while requiring its source and confidence to agree.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildRuntimeStateError::InvalidQuotaSnapshot`] for invalid bounds, time
    /// ordering, or source/confidence evidence, and `InvalidOpaqueValue` for its raw window type.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        kind: GrokBuildQuotaWindowKind,
        remaining: u64,
        total: u64,
        window_seconds: u64,
        reset_at_ms: i64,
        observed_at_ms: i64,
        source: GrokBuildQuotaSource,
        confidence: GrokBuildQuotaConfidence,
        raw_window_type: impl Into<String>,
    ) -> Result<Self, GrokBuildRuntimeStateError> {
        if total == 0
            || remaining > total
            || window_seconds == 0
            || observed_at_ms <= 0
            || reset_at_ms <= observed_at_ms
            || source.required_confidence() != confidence
        {
            return Err(GrokBuildRuntimeStateError::InvalidQuotaSnapshot);
        }
        Ok(Self {
            kind,
            remaining,
            total,
            window_seconds,
            reset_at_ms,
            observed_at_ms,
            source,
            confidence,
            raw_window_type: validate_text(raw_window_type.into(), MAX_WINDOW_TYPE_BYTES)?,
        })
    }

    /// Returns the distinct quota window kind.
    #[must_use]
    pub const fn kind(&self) -> GrokBuildQuotaWindowKind {
        self.kind
    }

    /// Returns the remaining capacity in the declared window unit.
    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.remaining
    }

    /// Returns the total capacity in the declared window unit.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.total
    }

    /// Returns the source observation time.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Returns the required reset time.
    #[must_use]
    pub const fn reset_at_ms(&self) -> i64 {
        self.reset_at_ms
    }

    /// Returns the source kind.
    #[must_use]
    pub const fn source(&self) -> GrokBuildQuotaSource {
        self.source
    }

    /// Returns the source confidence.
    #[must_use]
    pub const fn confidence(&self) -> GrokBuildQuotaConfidence {
        self.confidence
    }
}

/// Safe outcome of storing a quota observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildQuotaSyncOutcome {
    /// The observation was durably applied.
    Applied,
    /// A newer observation already existed, so the older value was retained.
    IgnoredStale,
}

/// Safe outcome of synchronizing one Credential's billing and catalog snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildCatalogSyncOutcome {
    /// The new complete snapshot was atomically applied.
    Applied,
    /// A snapshot at least as new already exists, so this observation was retained only as input.
    IgnoredStale,
}

/// Durable P6-04 state store for one process-local Build runtime database.
pub struct GrokBuildRuntimeStateStore {
    connection: Mutex<Connection>,
}

impl GrokBuildRuntimeStateStore {
    /// Opens and migrates a Build runtime database without exposing its path in errors.
    ///
    /// # Errors
    ///
    /// Returns `StoreUnavailable` when the database cannot be safely opened or migrated.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, GrokBuildRuntimeStateError> {
        let mut connection =
            gateway_store::open(path).map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        gateway_store::migrate(&mut connection)
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Opens a migrated in-memory Build runtime database for deterministic tests.
    ///
    /// # Errors
    ///
    /// Returns `StoreUnavailable` when the in-memory database cannot be safely migrated.
    pub fn open_in_memory() -> Result<Self, GrokBuildRuntimeStateError> {
        let mut connection = gateway_store::open_in_memory()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        gateway_store::migrate(&mut connection)
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Replaces one Credential's model capability snapshot atomically.
    ///
    /// # Errors
    ///
    /// Returns `InvalidCatalogSnapshot` for incomplete, duplicate, stale-shaped, or invalid-time
    /// input, and `StoreUnavailable` if the atomic operation cannot complete.
    pub fn sync_model_catalog(
        &self,
        credential_id: &CredentialId,
        billing_plan: GrokBuildBillingPlan,
        billing_observed_at_ms: i64,
        capabilities: &[GrokBuildModelCapability],
    ) -> Result<GrokBuildCatalogSyncOutcome, GrokBuildRuntimeStateError> {
        validate_credential_id(credential_id)?;
        if billing_observed_at_ms <= 0
            || capabilities.is_empty()
            || capabilities
                .iter()
                .any(|capability| capability.observed_at_ms() < billing_observed_at_ms)
        {
            return Err(GrokBuildRuntimeStateError::InvalidCatalogSnapshot);
        }
        let unique_models: BTreeSet<_> = capabilities
            .iter()
            .map(GrokBuildModelCapability::upstream_model)
            .collect();
        if unique_models.len() != capabilities.len() {
            return Err(GrokBuildRuntimeStateError::InvalidCatalogSnapshot);
        }

        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        let current_observed_at_ms: Option<i64> = transaction
            .query_row(
                "SELECT observed_at_ms FROM grok_build_billing_profiles WHERE credential_id = ?1",
                params![credential_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        if current_observed_at_ms.is_some_and(|current| current >= billing_observed_at_ms) {
            transaction
                .commit()
                .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
            return Ok(GrokBuildCatalogSyncOutcome::IgnoredStale);
        }
        transaction
            .execute(
                "INSERT INTO grok_build_billing_profiles (credential_id, plan_kind, observed_at_ms) \
                 VALUES (?1, ?2, ?3) \
                 ON CONFLICT(credential_id) DO UPDATE SET \
                   plan_kind = excluded.plan_kind, observed_at_ms = excluded.observed_at_ms",
                params![credential_id.as_str(), billing_plan.as_sql(), billing_observed_at_ms],
            )
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        transaction
            .execute(
                "DELETE FROM grok_build_model_catalog WHERE credential_id = ?1",
                params![credential_id.as_str()],
            )
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        for capability in capabilities {
            transaction
                .execute(
                    "INSERT INTO grok_build_model_catalog \
                     (credential_id, upstream_model, source, observed_at_ms) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        credential_id.as_str(),
                        capability.upstream_model(),
                        capability.source().as_sql(),
                        capability.observed_at_ms(),
                    ],
                )
                .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        }
        transaction
            .commit()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        Ok(GrokBuildCatalogSyncOutcome::Applied)
    }

    /// Loads the exact Build billing profile and its observation time for one Credential.
    ///
    /// # Errors
    ///
    /// Returns `InvalidPersistedState` for an unrecognized stored plan or `StoreUnavailable` for
    /// a database failure.
    pub fn billing_plan(
        &self,
        credential_id: &CredentialId,
    ) -> Result<Option<(GrokBuildBillingPlan, i64)>, GrokBuildRuntimeStateError> {
        validate_credential_id(credential_id)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        connection
            .query_row(
                "SELECT plan_kind, observed_at_ms FROM grok_build_billing_profiles \
                 WHERE credential_id = ?1",
                params![credential_id.as_str()],
                |row| {
                    let plan_kind: String = row.get(0)?;
                    let observed_at_ms: i64 = row.get(1)?;
                    Ok((plan_kind, observed_at_ms))
                },
            )
            .optional()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?
            .map(|(plan_kind, observed_at_ms)| {
                Ok((GrokBuildBillingPlan::from_sql(&plan_kind)?, observed_at_ms))
            })
            .transpose()
    }

    /// Loads the current complete model-capability snapshot for one Credential.
    ///
    /// # Errors
    ///
    /// Returns `InvalidPersistedState` for a malformed stored capability or `StoreUnavailable` for
    /// a database failure.
    pub fn model_catalog(
        &self,
        credential_id: &CredentialId,
    ) -> Result<Vec<GrokBuildModelCapability>, GrokBuildRuntimeStateError> {
        validate_credential_id(credential_id)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        let mut statement = connection
            .prepare(
                "SELECT upstream_model, source, observed_at_ms FROM grok_build_model_catalog \
                 WHERE credential_id = ?1 ORDER BY upstream_model ASC",
            )
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        let rows = statement
            .query_map(params![credential_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        rows.map(|row| {
            let (upstream_model, source, observed_at_ms) =
                row.map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
            GrokBuildModelCapability::try_new(
                upstream_model,
                GrokBuildModelSource::from_sql(&source)?,
                observed_at_ms,
            )
        })
        .collect()
    }

    /// Stores a newer source-labelled quota window without overwriting a newer observation.
    ///
    /// # Errors
    ///
    /// Returns `InvalidQuotaSnapshot` when a value cannot fit the durable schema or
    /// `StoreUnavailable` when it cannot be persisted.
    pub fn sync_quota_window(
        &self,
        credential_id: &CredentialId,
        window: &GrokBuildQuotaWindow,
    ) -> Result<GrokBuildQuotaSyncOutcome, GrokBuildRuntimeStateError> {
        validate_credential_id(credential_id)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        let current_observed_at_ms: Option<i64> = connection
            .query_row(
                "SELECT observed_at_ms FROM grok_build_quota_windows \
                 WHERE credential_id = ?1 AND window_kind = ?2",
                params![credential_id.as_str(), window.kind.as_sql()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        if current_observed_at_ms.is_some_and(|current| current >= window.observed_at_ms) {
            return Ok(GrokBuildQuotaSyncOutcome::IgnoredStale);
        }
        connection
            .execute(
                "INSERT INTO grok_build_quota_windows \
                 (credential_id, window_kind, remaining, total, window_seconds, reset_at_ms, \
                  observed_at_ms, source, confidence, raw_window_type) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
                 ON CONFLICT(credential_id, window_kind) DO UPDATE SET \
                   remaining = excluded.remaining, total = excluded.total, \
                   window_seconds = excluded.window_seconds, reset_at_ms = excluded.reset_at_ms, \
                   observed_at_ms = excluded.observed_at_ms, source = excluded.source, \
                   confidence = excluded.confidence, raw_window_type = excluded.raw_window_type",
                params![
                    credential_id.as_str(),
                    window.kind.as_sql(),
                    i64::try_from(window.remaining)
                        .map_err(|_| GrokBuildRuntimeStateError::InvalidQuotaSnapshot)?,
                    i64::try_from(window.total)
                        .map_err(|_| GrokBuildRuntimeStateError::InvalidQuotaSnapshot)?,
                    i64::try_from(window.window_seconds)
                        .map_err(|_| GrokBuildRuntimeStateError::InvalidQuotaSnapshot)?,
                    window.reset_at_ms,
                    window.observed_at_ms,
                    window.source.as_sql(),
                    window.confidence.as_sql(),
                    window.raw_window_type,
                ],
            )
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        Ok(GrokBuildQuotaSyncOutcome::Applied)
    }

    /// Loads one exact quota window without substituting a different source or window kind.
    ///
    /// # Errors
    ///
    /// Returns `InvalidPersistedState` for a malformed stored row or `StoreUnavailable` for a
    /// database failure.
    pub fn quota_window(
        &self,
        credential_id: &CredentialId,
        kind: GrokBuildQuotaWindowKind,
    ) -> Result<Option<GrokBuildQuotaWindow>, GrokBuildRuntimeStateError> {
        validate_credential_id(credential_id)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        connection
            .query_row(
                "SELECT remaining, total, window_seconds, reset_at_ms, observed_at_ms, source, \
                 confidence, raw_window_type FROM grok_build_quota_windows \
                 WHERE credential_id = ?1 AND window_kind = ?2",
                params![credential_id.as_str(), kind.as_sql()],
                |row| {
                    let remaining: i64 = row.get(0)?;
                    let total: i64 = row.get(1)?;
                    let window_seconds: i64 = row.get(2)?;
                    let reset_at_ms: i64 = row.get(3)?;
                    let observed_at_ms: i64 = row.get(4)?;
                    let source: String = row.get(5)?;
                    let confidence: String = row.get(6)?;
                    let raw_window_type: String = row.get(7)?;
                    Ok((
                        remaining,
                        total,
                        window_seconds,
                        reset_at_ms,
                        observed_at_ms,
                        source,
                        confidence,
                        raw_window_type,
                    ))
                },
            )
            .optional()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?
            .map(
                |(
                    remaining,
                    total,
                    window_seconds,
                    reset_at_ms,
                    observed_at_ms,
                    source,
                    confidence,
                    raw_window_type,
                )| {
                    GrokBuildQuotaWindow::try_new(
                        kind,
                        u64::try_from(remaining)
                            .map_err(|_| GrokBuildRuntimeStateError::InvalidPersistedState)?,
                        u64::try_from(total)
                            .map_err(|_| GrokBuildRuntimeStateError::InvalidPersistedState)?,
                        u64::try_from(window_seconds)
                            .map_err(|_| GrokBuildRuntimeStateError::InvalidPersistedState)?,
                        reset_at_ms,
                        observed_at_ms,
                        GrokBuildQuotaSource::from_sql(&source)?,
                        GrokBuildQuotaConfidence::from_sql(&confidence)?,
                        raw_window_type,
                    )
                },
            )
            .transpose()
    }

    /// Loads every distinct quota window for one Credential in stable window-kind order.
    ///
    /// # Errors
    ///
    /// Returns `InvalidPersistedState` for a malformed stored row or `StoreUnavailable` for a
    /// database failure.
    pub fn quota_windows(
        &self,
        credential_id: &CredentialId,
    ) -> Result<Vec<GrokBuildQuotaWindow>, GrokBuildRuntimeStateError> {
        validate_credential_id(credential_id)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        let mut statement = connection
            .prepare(
                "SELECT window_kind, remaining, total, window_seconds, reset_at_ms, observed_at_ms, \
                 source, confidence, raw_window_type FROM grok_build_quota_windows \
                 WHERE credential_id = ?1 ORDER BY window_kind ASC",
            )
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        let rows = statement
            .query_map(params![credential_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
        rows.map(|row| {
            let (
                kind,
                remaining,
                total,
                window_seconds,
                reset_at_ms,
                observed_at_ms,
                source,
                confidence,
                raw_window_type,
            ) = row.map_err(|_| GrokBuildRuntimeStateError::StoreUnavailable)?;
            GrokBuildQuotaWindow::try_new(
                GrokBuildQuotaWindowKind::from_sql(&kind)?,
                u64::try_from(remaining)
                    .map_err(|_| GrokBuildRuntimeStateError::InvalidPersistedState)?,
                u64::try_from(total)
                    .map_err(|_| GrokBuildRuntimeStateError::InvalidPersistedState)?,
                u64::try_from(window_seconds)
                    .map_err(|_| GrokBuildRuntimeStateError::InvalidPersistedState)?,
                reset_at_ms,
                observed_at_ms,
                GrokBuildQuotaSource::from_sql(&source)?,
                GrokBuildQuotaConfidence::from_sql(&confidence)?,
                raw_window_type,
            )
        })
        .collect()
    }
}

impl fmt::Debug for GrokBuildRuntimeStateStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildRuntimeStateStore")
            .field("connection", &"<redacted>")
            .finish()
    }
}

/// Account evidence required before a Build 403 can mutate account state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildAccountEvidence {
    /// No account-level evidence is available; a 403 is an egress rejection only.
    None,
    /// The Provider has independent account-level evidence of a forbidden credential.
    ConfirmedForbidden,
}

/// A bounded distinction for Build 429 behaviour.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildRateLimitEvidence {
    /// No additional rate-limit evidence exists.
    None,
    /// The limit is attached to one credential/account.
    Account,
    /// The limit is provider-wide high traffic.
    Provider,
}

/// The only state action P6-07 permits after classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildFailureAction {
    /// Keep credential/account state unchanged.
    None,
    /// Stop scheduling the credential until explicit reauthorization.
    RequireReauthorization,
    /// Mark only the independently evidenced account as forbidden.
    MarkAccountForbidden,
    /// Cool only the named quota window.
    CoolQuotaWindow,
    /// Cool the account without disabling its credential.
    CoolAccount,
    /// Cool the Provider without permanently disabling accounts.
    CoolProvider,
}

/// A safe P6-07 classification plus its permitted state action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokBuildFailureDisposition {
    error: GatewayError,
    action: GrokBuildFailureAction,
}

impl GrokBuildFailureDisposition {
    /// Returns the transport-neutral error.
    #[must_use]
    pub fn error(&self) -> &GatewayError {
        &self.error
    }

    /// Returns the only permitted state transition category.
    #[must_use]
    pub const fn action(&self) -> GrokBuildFailureAction {
        self.action
    }
}

/// Classifies Build status and bounded syntax evidence without retaining upstream text.
#[must_use]
pub fn classify_grok_build_failure(
    status: u16,
    signal: GrokBuildResponsesErrorSignal,
    account_evidence: GrokBuildAccountEvidence,
    rate_limit_evidence: GrokBuildRateLimitEvidence,
) -> GrokBuildFailureDisposition {
    let (code, scope, action) = match signal {
        GrokBuildResponsesErrorSignal::InvalidGrant
        | GrokBuildResponsesErrorSignal::InvalidToken => (
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
            GrokBuildFailureAction::RequireReauthorization,
        ),
        GrokBuildResponsesErrorSignal::FreeUsageExhausted => (
            GatewayErrorCode::CredentialQuotaExceeded,
            ErrorScope::QuotaWindow,
            GrokBuildFailureAction::CoolQuotaWindow,
        ),
        GrokBuildResponsesErrorSignal::None | GrokBuildResponsesErrorSignal::Unrecognized => {
            match status {
                401 => (
                    GatewayErrorCode::CredentialUnauthorized,
                    ErrorScope::Credential,
                    GrokBuildFailureAction::RequireReauthorization,
                ),
                403 if account_evidence == GrokBuildAccountEvidence::ConfirmedForbidden => (
                    GatewayErrorCode::CredentialForbidden,
                    ErrorScope::Account,
                    GrokBuildFailureAction::MarkAccountForbidden,
                ),
                403 => (
                    GatewayErrorCode::EgressRejected,
                    ErrorScope::Egress,
                    GrokBuildFailureAction::None,
                ),
                429 => match rate_limit_evidence {
                    GrokBuildRateLimitEvidence::Account => (
                        GatewayErrorCode::ProviderRateLimited,
                        ErrorScope::Account,
                        GrokBuildFailureAction::CoolAccount,
                    ),
                    GrokBuildRateLimitEvidence::Provider | GrokBuildRateLimitEvidence::None => (
                        GatewayErrorCode::ProviderRateLimited,
                        ErrorScope::Provider,
                        GrokBuildFailureAction::CoolProvider,
                    ),
                },
                408 | 500..=599 => (
                    GatewayErrorCode::ProviderTransient,
                    ErrorScope::Provider,
                    GrokBuildFailureAction::CoolProvider,
                ),
                _ => (
                    GatewayErrorCode::ProviderPermanent,
                    ErrorScope::Provider,
                    GrokBuildFailureAction::None,
                ),
            }
        }
    };
    GrokBuildFailureDisposition {
        error: GatewayError::new(code, scope),
        action,
    }
}

/// Safe failure classes for durable P6 runtime state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildRuntimeStateError {
    /// The state store could not be opened, locked, queried, or committed.
    StoreUnavailable,
    /// A caller supplied an empty, oversized, or NUL-containing opaque value.
    InvalidOpaqueValue,
    /// A catalog snapshot has no models, duplicates a model, or predates its billing snapshot.
    InvalidCatalogSnapshot,
    /// A quota snapshot has invalid bounds, ordering, or source/confidence evidence.
    InvalidQuotaSnapshot,
    /// A stored row cannot be decoded under the strict P6 state schema.
    InvalidPersistedState,
}

impl fmt::Display for GrokBuildRuntimeStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::StoreUnavailable => "Grok Build runtime state store is unavailable",
            Self::InvalidOpaqueValue => "Grok Build runtime state value is invalid",
            Self::InvalidCatalogSnapshot => "Grok Build model catalog snapshot is invalid",
            Self::InvalidQuotaSnapshot => "Grok Build quota snapshot is invalid",
            Self::InvalidPersistedState => "Grok Build runtime state is invalid",
        };
        formatter.write_str(message)
    }
}

impl Error for GrokBuildRuntimeStateError {}

fn validate_text(
    value: String,
    maximum_bytes: usize,
) -> Result<String, GrokBuildRuntimeStateError> {
    if value.trim().is_empty()
        || value.len() > maximum_bytes
        || value.bytes().any(|byte| byte == b'\0')
    {
        return Err(GrokBuildRuntimeStateError::InvalidOpaqueValue);
    }
    Ok(value)
}

fn validate_credential_id(credential_id: &CredentialId) -> Result<(), GrokBuildRuntimeStateError> {
    let value = credential_id.as_str();
    if value.trim().is_empty()
        || value.len() > MAX_CREDENTIAL_ID_BYTES
        || value.bytes().any(|byte| byte == b'\0')
    {
        return Err(GrokBuildRuntimeStateError::InvalidOpaqueValue);
    }
    Ok(())
}
