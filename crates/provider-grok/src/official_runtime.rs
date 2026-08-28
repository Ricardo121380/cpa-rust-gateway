//! Isolated Grok Official runtime quota and failure boundary.
//!
//! The Official API-key adapter may write only sanitized quota observations to its exact
//! Endpoint/Credential target. It deliberately has no Build OAuth, cache-affinity, response
//! ownership, or reasoning-replay dependency. HTTP failure classification is likewise an
//! ownership decision, not a credential/account mutation.

use std::{error::Error, fmt, sync::Arc, time::Duration};

use gateway_core::{
    CredentialId, EndpointId, ErrorScope, GatewayError, GatewayErrorCode, ProviderId,
};
use gateway_router::{
    QuotaConfidence, QuotaSnapshot, QuotaSnapshotError, QuotaSource, QuotaWindow,
    RuntimeHealthClock, RuntimeQuotaError, RuntimeQuotaRegistry, RuntimeQuotaTarget,
    SystemRuntimeHealthClock,
};

use crate::{GROK_OFFICIAL_PROVIDER_ID, GrokOfficialRateLimitKind, GrokOfficialRateLimitMetadata};

const OFFICIAL_REQUESTS_WINDOW: &str = "official.requests";
const OFFICIAL_TOKENS_WINDOW: &str = "official.tokens";
const DEFAULT_OFFICIAL_RATE_LIMIT_FALLBACK: Duration = Duration::from_secs(30);

/// Official does not retain any provider-private continuity state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokOfficialContinuityPolicy {
    /// API-key Requests are stateless; Build-only affinity/ownership/replay cannot be reused.
    Stateless,
}

/// The single remediation owner permitted by a classified Official HTTP failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokOfficialFailureAction {
    /// Preserve all provider runtime state.
    None,
    /// Stop only this Official API key until an explicit replacement succeeds.
    RequireCredentialReplacement,
    /// Record only this Official binding's temporary quota observation.
    RecordExactQuota,
    /// Cool only the selected Official endpoint through the later health-state owner.
    CoolOfficialEndpoint,
}

/// A value-free Official HTTP failure and its only permitted remediation owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokOfficialFailureDisposition {
    error: GatewayError,
    action: GrokOfficialFailureAction,
}

impl GrokOfficialFailureDisposition {
    /// Returns the transport-neutral client-safe error.
    #[must_use]
    pub fn error(&self) -> &GatewayError {
        &self.error
    }

    /// Returns the only state owner allowed to react to the failure.
    #[must_use]
    pub const fn action(&self) -> GrokOfficialFailureAction {
        self.action
    }
}

/// Classifies an Official status without reading a body, header value, Build state, or Web state.
#[must_use]
pub const fn classify_grok_official_http_failure(status: u16) -> GrokOfficialFailureDisposition {
    let (code, scope, action) = match status {
        401 => (
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
            GrokOfficialFailureAction::RequireCredentialReplacement,
        ),
        // Without independent account evidence, an Official 403 may be a policy or egress denial.
        // It must not permanently disable an API key or modify any Build/Web account state.
        403 => (
            GatewayErrorCode::EgressRejected,
            ErrorScope::Egress,
            GrokOfficialFailureAction::None,
        ),
        429 => (
            GatewayErrorCode::ProviderRateLimited,
            ErrorScope::QuotaWindow,
            GrokOfficialFailureAction::RecordExactQuota,
        ),
        408 | 500..=599 => (
            GatewayErrorCode::ProviderTransient,
            ErrorScope::Provider,
            GrokOfficialFailureAction::CoolOfficialEndpoint,
        ),
        _ => (
            GatewayErrorCode::ProviderPermanent,
            ErrorScope::Provider,
            GrokOfficialFailureAction::None,
        ),
    };
    GrokOfficialFailureDisposition {
        error: GatewayError::new(code, scope),
        action,
    }
}

/// Exact Official runtime handoff for sanitized Header quota evidence.
///
/// Its injected registry remains the router-owned state implementation. This type cannot accept a
/// Build continuity store or a raw HTTP response, and it always forms a binding-wide Official
/// target from its own explicit Endpoint/Credential identities.
#[derive(Clone)]
pub struct GrokOfficialRuntimeState {
    provider_id: ProviderId,
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    runtime_quota: Arc<RuntimeQuotaRegistry>,
    clock: Arc<dyn RuntimeHealthClock>,
}

impl GrokOfficialRuntimeState {
    /// Creates one state handoff for an already selected Official Endpoint/API-key binding.
    ///
    /// # Errors
    ///
    /// Returns `InvalidProviderIdentity` only if the compiled fixed Official provider identifier
    /// is changed into an invalid value.
    pub fn try_new(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        runtime_quota: Arc<RuntimeQuotaRegistry>,
    ) -> Result<Self, GrokOfficialRuntimeStateError> {
        Self::try_new_with_clock(
            endpoint_id,
            credential_id,
            runtime_quota,
            Arc::new(SystemRuntimeHealthClock),
        )
    }

    /// Creates one Official state handoff with an injected Router timestamp source.
    ///
    /// Tests and outer runtime composition can supply a deterministic clock; the default
    /// constructor uses the Router's system-clock implementation.
    ///
    /// # Errors
    ///
    /// Returns `InvalidProviderIdentity` only if the compiled fixed Official provider identifier
    /// is changed into an invalid value.
    pub fn try_new_with_clock(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        runtime_quota: Arc<RuntimeQuotaRegistry>,
        clock: Arc<dyn RuntimeHealthClock>,
    ) -> Result<Self, GrokOfficialRuntimeStateError> {
        let provider_id = ProviderId::try_new(GROK_OFFICIAL_PROVIDER_ID)
            .map_err(|_| GrokOfficialRuntimeStateError::InvalidProviderIdentity)?;
        Ok(Self {
            provider_id,
            endpoint_id,
            credential_id,
            runtime_quota,
            clock,
        })
    }

    /// Returns the fixed Official provider identity.
    #[must_use]
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns Official's explicit lack of cache-affinity/ownership/replay state.
    #[must_use]
    pub const fn continuity_policy() -> GrokOfficialContinuityPolicy {
        GrokOfficialContinuityPolicy::Stateless
    }

    /// Projects complete Official resource windows into the exact binding-wide quota target.
    ///
    /// A Header set without a complete request/token resource tuple has no quota-state effect.
    /// `Retry-After` is intentionally handled only with a classified `429`, because it alone does
    /// not identify a quota resource or establish that an otherwise successful response is blocked.
    ///
    /// # Errors
    ///
    /// Returns a value-free error before a malformed time or impossible snapshot can mutate the
    /// router registry. Registry failures retain their existing fail-closed behavior.
    pub fn record_rate_limit_metadata(
        &self,
        metadata: &GrokOfficialRateLimitMetadata,
        observed_at_ms: i64,
    ) -> Result<Option<QuotaSnapshot>, GrokOfficialRuntimeStateError> {
        if observed_at_ms <= 0 {
            return Err(GrokOfficialRuntimeStateError::InvalidObservationTime);
        }
        if metadata.windows().is_empty() {
            return Ok(None);
        }

        let mut windows = Vec::with_capacity(metadata.windows().len());
        for window in metadata.windows() {
            let label = match window.kind() {
                GrokOfficialRateLimitKind::Requests => OFFICIAL_REQUESTS_WINDOW,
                GrokOfficialRateLimitKind::Tokens => OFFICIAL_TOKENS_WINDOW,
            };
            let reset_at_ms = add_duration(observed_at_ms, window.reset_after())?;
            windows.push(
                QuotaWindow::try_new(
                    label,
                    Some(window.limit()),
                    Some(window.remaining()),
                    Some(reset_at_ms),
                )
                .map_err(GrokOfficialRuntimeStateError::from)?,
            );
        }
        let snapshot = QuotaSnapshot::try_new(
            self.quota_target(),
            windows,
            QuotaSource::Header,
            QuotaConfidence::Observed,
            observed_at_ms,
        )
        .map_err(GrokOfficialRuntimeStateError::from)?;
        self.runtime_quota
            .record_snapshot(snapshot)
            .map(Some)
            .map_err(GrokOfficialRuntimeStateError::from)
    }

    /// Classifies one Official HTTP failure and records a quota only for its own `429` binding.
    ///
    /// `401`, `403`, `408`, `5xx`, and permanent failures are classification-only here: their
    /// separately owned credential/health transitions must never reach a Build/Web namespace.
    ///
    /// # Errors
    ///
    /// Returns a value-free runtime error only when the exact `429` quota handoff cannot be
    /// represented or recorded. Non-`429` classification never mutates runtime quota state.
    pub fn observe_http_failure(
        &self,
        status: u16,
        metadata: &GrokOfficialRateLimitMetadata,
        observed_at_ms: i64,
    ) -> Result<GrokOfficialFailureDisposition, GrokOfficialRuntimeStateError> {
        let disposition = classify_grok_official_http_failure(status);
        if disposition.action() == GrokOfficialFailureAction::RecordExactQuota {
            if observed_at_ms <= 0 {
                return Err(GrokOfficialRuntimeStateError::InvalidObservationTime);
            }
            self.runtime_quota
                .record_rate_limited(
                    self.quota_target(),
                    observed_at_ms,
                    metadata.retry_after(),
                    DEFAULT_OFFICIAL_RATE_LIMIT_FALLBACK,
                )
                .map_err(GrokOfficialRuntimeStateError::from)?;
        }
        Ok(disposition)
    }

    /// Applies one complete transport status observation at the injected current timestamp.
    ///
    /// Successful responses may record complete Header quota metadata. Non-success responses
    /// return their value-free disposition and only a `429` writes the exact temporary quota.
    /// This is the adapter-facing hook; it never reads a body or chooses a route.
    ///
    /// # Errors
    ///
    /// Returns a value-free error when the Router clock or exact quota handoff is unavailable.
    pub fn observe_transport_response(
        &self,
        status: u16,
        metadata: &GrokOfficialRateLimitMetadata,
    ) -> Result<Option<GrokOfficialFailureDisposition>, GrokOfficialRuntimeStateError> {
        let observed_at_ms = self
            .clock
            .now_ms()
            .map_err(|_| GrokOfficialRuntimeStateError::ClockUnavailable)?;
        if (200..=299).contains(&status) {
            self.record_rate_limit_metadata(metadata, observed_at_ms)?;
            Ok(None)
        } else {
            self.observe_http_failure(status, metadata, observed_at_ms)
                .map(Some)
        }
    }

    fn quota_target(&self) -> RuntimeQuotaTarget {
        RuntimeQuotaTarget::endpoint_credential(
            self.endpoint_id.clone(),
            self.credential_id.clone(),
        )
    }
}

impl fmt::Debug for GrokOfficialRuntimeState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialRuntimeState")
            .field("provider_id", &self.provider_id)
            .field("endpoint_id", &"<redacted>")
            .field("credential_id", &"<redacted>")
            .field("runtime_quota", &"<injected>")
            .field("clock", &"<injected>")
            .finish_non_exhaustive()
    }
}

/// Value-free failures from the Official state-isolation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokOfficialRuntimeStateError {
    /// The compiled fixed Official provider identifier was invalid.
    InvalidProviderIdentity,
    /// The caller did not supply a strictly positive observation time.
    InvalidObservationTime,
    /// The injected Router timestamp source was unavailable or out of range.
    ClockUnavailable,
    /// A bounded reset duration could not be added to the supplied observation time.
    ResetTimeOverflow,
    /// A sanitized window could not satisfy the generic quota shape.
    InvalidQuotaWindow,
    /// A sanitized snapshot could not satisfy exact-target quota rules.
    InvalidQuotaSnapshot,
    /// The injected router quota registry rejected or could not retain the exact target.
    RuntimeQuotaUnavailable,
}

impl From<gateway_router::QuotaWindowError> for GrokOfficialRuntimeStateError {
    fn from(_: gateway_router::QuotaWindowError) -> Self {
        Self::InvalidQuotaWindow
    }
}

impl From<QuotaSnapshotError> for GrokOfficialRuntimeStateError {
    fn from(_: QuotaSnapshotError) -> Self {
        Self::InvalidQuotaSnapshot
    }
}

impl From<RuntimeQuotaError> for GrokOfficialRuntimeStateError {
    fn from(_: RuntimeQuotaError) -> Self {
        Self::RuntimeQuotaUnavailable
    }
}

impl fmt::Display for GrokOfficialRuntimeStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidProviderIdentity => "Grok Official provider identity is invalid",
            Self::InvalidObservationTime => "Grok Official quota observation time is invalid",
            Self::ClockUnavailable => "Grok Official runtime clock is unavailable",
            Self::ResetTimeOverflow => "Grok Official quota reset time is invalid",
            Self::InvalidQuotaWindow => "Grok Official quota window is invalid",
            Self::InvalidQuotaSnapshot => "Grok Official quota snapshot is invalid",
            Self::RuntimeQuotaUnavailable => "Grok Official runtime quota state is unavailable",
        };
        formatter.write_str(message)
    }
}

impl Error for GrokOfficialRuntimeStateError {}

fn add_duration(
    observed_at_ms: i64,
    duration: Duration,
) -> Result<i64, GrokOfficialRuntimeStateError> {
    let duration_ms = i64::try_from(duration.as_millis())
        .map_err(|_| GrokOfficialRuntimeStateError::ResetTimeOverflow)?;
    observed_at_ms
        .checked_add(duration_ms)
        .ok_or(GrokOfficialRuntimeStateError::ResetTimeOverflow)
}
