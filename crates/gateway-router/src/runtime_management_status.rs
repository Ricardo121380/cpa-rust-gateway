//! Read-only management projection for exact runtime account, Health, and Quota state.
//!
//! This P4 boundary has no HTTP, authentication, `SQLite`, Provider, exporter, or request-path
//! dependency. A P10 management transport may call it later, but this module neither registers a
//! route nor decides how an operator performs a recovery check.

use std::{error::Error, fmt, sync::Arc};

use gateway_core::{CredentialId, EndpointId};

use crate::{
    QuotaConfidence, QuotaSource, RuntimeCredentialAccountStatus, RuntimeHealthAvailability,
    RuntimeHealthKey, RuntimeHealthRegistry, RuntimeQuotaAvailability, RuntimeQuotaRegistry,
    RuntimeQuotaStatusSnapshot, RuntimeQuotaTarget,
};

/// One exact management-query target with an optional caller-supplied upstream-model scope.
#[derive(Clone, Eq, PartialEq)]
pub struct RuntimeManagementStatusTarget {
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    upstream_model: Option<String>,
}

impl RuntimeManagementStatusTarget {
    /// Creates one binding target, optionally narrowed to an exact non-empty upstream model.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeManagementStatusTargetError::EmptyUpstreamModel`] before any registry read
    /// when a caller supplies an empty model scope.
    pub fn try_new(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        upstream_model: Option<String>,
    ) -> Result<Self, RuntimeManagementStatusTargetError> {
        if upstream_model.as_deref() == Some("") {
            return Err(RuntimeManagementStatusTargetError::EmptyUpstreamModel);
        }
        Ok(Self {
            endpoint_id,
            credential_id,
            upstream_model,
        })
    }

    /// Returns the exact Endpoint identity.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the exact non-secret Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns whether this query includes a model-scoped Health and Quota projection.
    #[must_use]
    pub const fn has_model_scope(&self) -> bool {
        self.upstream_model.is_some()
    }
}

impl fmt::Debug for RuntimeManagementStatusTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeManagementStatusTarget")
            .field("endpoint_id", &self.endpoint_id)
            .field("credential_id", &self.credential_id)
            .field("upstream_model_present", &self.upstream_model.is_some())
            .finish()
    }
}

/// Safe target-construction failure for a management status query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeManagementStatusTargetError {
    /// A model-scoped query must not use an empty model label.
    EmptyUpstreamModel,
}

impl fmt::Display for RuntimeManagementStatusTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyUpstreamModel => {
                formatter.write_str("runtime management status model scope is empty")
            }
        }
    }
}

impl Error for RuntimeManagementStatusTargetError {}

/// One fixed safe projection of an exact target's latest quota state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeManagementQuotaStatus {
    availability: RuntimeQuotaAvailability,
    source: Option<QuotaSource>,
    confidence: Option<QuotaConfidence>,
    observed_at_ms: Option<i64>,
    blocking_reset_at_ms: Option<i64>,
}

impl RuntimeManagementQuotaStatus {
    /// Returns effective ordinary-scheduling availability at the supplied observation time.
    #[must_use]
    pub const fn availability(&self) -> RuntimeQuotaAvailability {
        self.availability
    }

    /// Returns the safe quota evidence source when this exact target has an observation.
    #[must_use]
    pub const fn source(&self) -> Option<QuotaSource> {
        self.source
    }

    /// Returns the safe confidence class when this exact target has an observation.
    #[must_use]
    pub const fn confidence(&self) -> Option<QuotaConfidence> {
        self.confidence
    }

    /// Returns the classifier-supplied observation time when one exists.
    #[must_use]
    pub const fn observed_at_ms(&self) -> Option<i64> {
        self.observed_at_ms
    }

    /// Returns the latest exhausted-window reset that still blocks this target, when any.
    #[must_use]
    pub const fn blocking_reset_at_ms(&self) -> Option<i64> {
        self.blocking_reset_at_ms
    }
}

/// One read-only exact binding projection for a fixed observation time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeManagementStatusSnapshot {
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    observed_at_ms: i64,
    account_status: RuntimeCredentialAccountStatus,
    endpoint_health: RuntimeHealthAvailability,
    credential_health: RuntimeHealthAvailability,
    model_health: Option<RuntimeHealthAvailability>,
    binding_quota: RuntimeManagementQuotaStatus,
    model_quota: Option<RuntimeManagementQuotaStatus>,
}

impl RuntimeManagementStatusSnapshot {
    /// Returns the exact Endpoint identity queried.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the exact Credential identity queried.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the fixed time used by every Health and Quota lookup in this projection.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Returns the provider-classified 403/account recovery state for this exact binding.
    #[must_use]
    pub const fn account_status(&self) -> RuntimeCredentialAccountStatus {
        self.account_status
    }

    /// Returns Endpoint-wide Cooldown/Circuit availability.
    #[must_use]
    pub const fn endpoint_health(&self) -> RuntimeHealthAvailability {
        self.endpoint_health
    }

    /// Returns exact Endpoint/Credential Health availability, including 403/recovery blocks.
    #[must_use]
    pub const fn credential_health(&self) -> RuntimeHealthAvailability {
        self.credential_health
    }

    /// Returns exact model-scoped Health availability when the query had a model scope.
    #[must_use]
    pub const fn model_health(&self) -> Option<RuntimeHealthAvailability> {
        self.model_health
    }

    /// Returns binding-wide 429/Quota evidence and availability.
    #[must_use]
    pub const fn binding_quota(&self) -> &RuntimeManagementQuotaStatus {
        &self.binding_quota
    }

    /// Returns model-scoped 429/Quota evidence and availability when the query had a model scope.
    #[must_use]
    pub const fn model_quota(&self) -> Option<&RuntimeManagementQuotaStatus> {
        self.model_quota.as_ref()
    }
}

/// Safe failures from a read-only management state query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeManagementStatusQueryError {
    /// One isolated Health shard could not be read.
    HealthUnavailable,
    /// One isolated Quota shard could not be read.
    QuotaUnavailable,
}

impl fmt::Display for RuntimeManagementStatusQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HealthUnavailable => formatter.write_str("runtime management health unavailable"),
            Self::QuotaUnavailable => formatter.write_str("runtime management quota unavailable"),
        }
    }
}

impl Error for RuntimeManagementStatusQueryError {}

/// In-process, read-only management query over existing Health and Quota registries.
pub struct RuntimeManagementStatusQuery {
    runtime_health: Arc<RuntimeHealthRegistry>,
    runtime_quota: Arc<RuntimeQuotaRegistry>,
}

impl RuntimeManagementStatusQuery {
    /// Creates one query boundary over externally owned, process-local runtime registries.
    #[must_use]
    pub fn new(
        runtime_health: Arc<RuntimeHealthRegistry>,
        runtime_quota: Arc<RuntimeQuotaRegistry>,
    ) -> Self {
        Self {
            runtime_health,
            runtime_quota,
        }
    }

    /// Reads one fixed-time exact binding projection without starting recovery or mutating state.
    ///
    /// The caller controls the observation time so a management process can correlate this result
    /// with P4-06's fixed-input Route Explain. Every component uses that same explicit instant,
    /// but independently locked registry reads are not a cross-registry atomic snapshot. No URL,
    /// Header, Body, Secret, Provider diagnostic, or upstream-model value is rendered by this API.
    ///
    /// # Errors
    ///
    /// Returns a target-free fail-closed error when a required isolated state shard is unavailable.
    pub fn binding_status(
        &self,
        target: &RuntimeManagementStatusTarget,
        observed_at_ms: i64,
    ) -> Result<RuntimeManagementStatusSnapshot, RuntimeManagementStatusQueryError> {
        let endpoint_key = RuntimeHealthKey::endpoint(target.endpoint_id.clone());
        let credential_key = RuntimeHealthKey::endpoint_credential(
            target.endpoint_id.clone(),
            target.credential_id.clone(),
        );
        let endpoint_health = self
            .runtime_health
            .availability_at(&endpoint_key, observed_at_ms)
            .map_err(|_| RuntimeManagementStatusQueryError::HealthUnavailable)?;
        let credential_health = self
            .runtime_health
            .availability_at(&credential_key, observed_at_ms)
            .map_err(|_| RuntimeManagementStatusQueryError::HealthUnavailable)?;
        let account_status = self
            .runtime_health
            .credential_account_status_at(
                &target.endpoint_id,
                &target.credential_id,
                observed_at_ms,
            )
            .map_err(|_| RuntimeManagementStatusQueryError::HealthUnavailable)?;
        let binding_quota_target = RuntimeQuotaTarget::endpoint_credential(
            target.endpoint_id.clone(),
            target.credential_id.clone(),
        );
        let binding_quota = quota_status(
            &self
                .runtime_quota
                .status_at(&binding_quota_target, observed_at_ms)
                .map_err(|_| RuntimeManagementStatusQueryError::QuotaUnavailable)?,
        );

        let (model_health, model_quota) =
            if let Some(upstream_model) = target.upstream_model.as_deref() {
                let model_health_key = RuntimeHealthKey::endpoint_credential_model(
                    target.endpoint_id.clone(),
                    target.credential_id.clone(),
                    upstream_model,
                );
                let model_health = self
                    .runtime_health
                    .availability_at(&model_health_key, observed_at_ms)
                    .map_err(|_| RuntimeManagementStatusQueryError::HealthUnavailable)?;
                let model_quota_target = RuntimeQuotaTarget::endpoint_credential_model(
                    target.endpoint_id.clone(),
                    target.credential_id.clone(),
                    upstream_model,
                )
                .map_err(|_| RuntimeManagementStatusQueryError::QuotaUnavailable)?;
                let model_quota = quota_status(
                    &self
                        .runtime_quota
                        .status_at(&model_quota_target, observed_at_ms)
                        .map_err(|_| RuntimeManagementStatusQueryError::QuotaUnavailable)?,
                );
                (Some(model_health), Some(model_quota))
            } else {
                (None, None)
            };

        Ok(RuntimeManagementStatusSnapshot {
            endpoint_id: target.endpoint_id.clone(),
            credential_id: target.credential_id.clone(),
            observed_at_ms,
            account_status,
            endpoint_health,
            credential_health,
            model_health,
            binding_quota,
            model_quota,
        })
    }
}

impl fmt::Debug for RuntimeManagementStatusQuery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeManagementStatusQuery")
            .field("runtime_health", &"<shared>")
            .field("runtime_quota", &"<shared>")
            .finish()
    }
}

fn quota_status(snapshot: &RuntimeQuotaStatusSnapshot) -> RuntimeManagementQuotaStatus {
    let availability = snapshot.availability();
    let Some(snapshot) = snapshot.snapshot() else {
        return RuntimeManagementQuotaStatus {
            availability,
            source: None,
            confidence: None,
            observed_at_ms: None,
            blocking_reset_at_ms: None,
        };
    };
    RuntimeManagementQuotaStatus {
        availability,
        source: Some(snapshot.source()),
        confidence: Some(snapshot.confidence()),
        observed_at_ms: Some(snapshot.observed_at_ms()),
        blocking_reset_at_ms: snapshot.blocking_reset_at_ms(),
    }
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
        RuntimeManagementStatusQuery, RuntimeManagementStatusTarget,
        RuntimeManagementStatusTargetError,
    };
    use crate::{
        QuotaConfidence, QuotaSnapshot, QuotaSource, QuotaWindow, RuntimeCredentialAccountStatus,
        RuntimeHealthAccountRecoveryResult, RuntimeHealthAvailability, RuntimeHealthClock,
        RuntimeHealthClockError, RuntimeHealthKey, RuntimeHealthRegistry, RuntimeQuotaAvailability,
        RuntimeQuotaRegistry, RuntimeQuotaTarget,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    #[allow(clippy::too_many_lines)] // One end-to-end projection is intentionally kept together.
    fn exact_read_only_projection_shows_403_quota_circuit_and_controlled_recovery() -> TestResult {
        let clock = Arc::new(FixedClock::new(100));
        let runtime_health = Arc::new(RuntimeHealthRegistry::with_clock(clock.clone()));
        let runtime_quota = Arc::new(RuntimeQuotaRegistry::with_clock(clock.clone()));
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential = CredentialId::try_new("credential-a")?;
        let target = RuntimeManagementStatusTarget::try_new(
            endpoint.clone(),
            credential.clone(),
            Some("private-model-must-not-render".to_owned()),
        )?;
        let quota_target =
            RuntimeQuotaTarget::endpoint_credential(endpoint.clone(), credential.clone());
        let model_quota_target = RuntimeQuotaTarget::endpoint_credential_model(
            endpoint.clone(),
            credential.clone(),
            "private-model-must-not-render",
        )?;

        runtime_health.mark_credential_forbidden(endpoint.clone(), credential.clone())?;
        runtime_health.open_circuit_until(
            RuntimeHealthKey::endpoint_credential_model(
                endpoint.clone(),
                credential.clone(),
                "private-model-must-not-render",
            ),
            200,
        )?;
        runtime_quota.record_rate_limited(
            quota_target,
            100,
            Some(Duration::from_millis(100)),
            Duration::from_secs(30),
        )?;
        runtime_quota.record_snapshot(QuotaSnapshot::try_new(
            model_quota_target,
            vec![QuotaWindow::try_new(
                "rate_limit",
                None,
                Some(0),
                Some(225),
            )?],
            QuotaSource::Estimated,
            QuotaConfidence::Estimated,
            100,
        )?)?;
        let query = RuntimeManagementStatusQuery::new(
            Arc::clone(&runtime_health),
            Arc::clone(&runtime_quota),
        );
        let entries_before = runtime_health.entry_count()?;
        let quota_entries_before = runtime_quota.entry_count()?;
        let snapshot = query.binding_status(&target, 100)?;

        assert_eq!(snapshot.observed_at_ms(), 100);
        assert_eq!(
            snapshot.account_status(),
            RuntimeCredentialAccountStatus::Forbidden
        );
        assert_eq!(
            snapshot.credential_health(),
            RuntimeHealthAvailability::AccountForbidden
        );
        assert_eq!(
            snapshot.model_health(),
            Some(RuntimeHealthAvailability::CircuitOpen {
                retry_after_ms: 200
            })
        );
        assert_eq!(
            snapshot.binding_quota().availability(),
            RuntimeQuotaAvailability::Exhausted { reset_at_ms: 200 }
        );
        assert_eq!(snapshot.binding_quota().source(), Some(QuotaSource::Header));
        assert_eq!(
            snapshot.binding_quota().confidence(),
            Some(QuotaConfidence::Observed)
        );
        assert_eq!(snapshot.binding_quota().blocking_reset_at_ms(), Some(200));
        let model_quota = snapshot
            .model_quota()
            .ok_or("model-scoped quota projection was unexpectedly absent")?;
        assert_eq!(
            model_quota.availability(),
            RuntimeQuotaAvailability::Exhausted { reset_at_ms: 225 }
        );
        assert_eq!(model_quota.source(), Some(QuotaSource::Estimated));
        assert_eq!(model_quota.confidence(), Some(QuotaConfidence::Estimated));
        assert_eq!(runtime_health.entry_count()?, entries_before);
        assert_eq!(runtime_quota.entry_count()?, quota_entries_before);
        assert!(!format!("{target:?}").contains("private-model-must-not-render"));
        assert!(!format!("{snapshot:?}").contains("private-model-must-not-render"));

        let recovery = runtime_health
            .begin_account_recovery(&endpoint, &credential, 150)?
            .ok_or("forbidden account did not issue a recovery ticket")?;
        let during_recovery = query.binding_status(&target, 100)?;
        assert_eq!(
            during_recovery.account_status(),
            RuntimeCredentialAccountStatus::RecoveryInFlight { expires_at_ms: 150 }
        );
        assert_eq!(
            during_recovery.credential_health(),
            RuntimeHealthAvailability::AccountRecoveryInFlight { expires_at_ms: 150 }
        );
        runtime_health
            .complete_account_recovery(recovery, RuntimeHealthAccountRecoveryResult::Allowed)?;
        assert_eq!(
            query.binding_status(&target, 100)?.account_status(),
            RuntimeCredentialAccountStatus::Available
        );
        Ok(())
    }

    #[test]
    fn target_scope_is_validated_and_explicit_query_never_reads_the_shared_clock() -> TestResult {
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let credential = CredentialId::try_new("credential-a")?;
        assert_eq!(
            RuntimeManagementStatusTarget::try_new(
                endpoint.clone(),
                credential.clone(),
                Some(String::new())
            ),
            Err(RuntimeManagementStatusTargetError::EmptyUpstreamModel)
        );

        let unavailable: Arc<dyn RuntimeHealthClock> = Arc::new(UnavailableClock);
        let query = RuntimeManagementStatusQuery::new(
            Arc::new(RuntimeHealthRegistry::with_clock(unavailable)),
            Arc::new(RuntimeQuotaRegistry::new()),
        );
        let target = RuntimeManagementStatusTarget::try_new(
            endpoint,
            credential,
            Some("private-model-must-not-render".to_owned()),
        )?;
        let snapshot = query.binding_status(&target, 100)?;
        assert_eq!(
            snapshot.account_status(),
            RuntimeCredentialAccountStatus::Available
        );
        assert!(!format!("{snapshot:?}").contains("private-model-must-not-render"));
        Ok(())
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
    }

    impl RuntimeHealthClock for FixedClock {
        fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
            Ok(self.now_ms.load(Ordering::Acquire))
        }
    }

    #[derive(Debug)]
    struct UnavailableClock;

    impl RuntimeHealthClock for UnavailableClock {
        fn now_ms(&self) -> Result<i64, RuntimeHealthClockError> {
            Err(RuntimeHealthClockError::Unavailable)
        }
    }
}
