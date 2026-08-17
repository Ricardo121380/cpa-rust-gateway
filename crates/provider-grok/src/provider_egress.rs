//! Exact Grok Build/Console adapter handoff for Provider-local egress state.
//!
//! This module consumes the provider-neutral P13-11E capability/state seam without creating a
//! second Credential scheduler or a second Health/Quota owner.  A request must already hold the
//! exact CPAR [`CredentialLease`]; this handoff only proves that the lease belongs to the declared
//! Build or Console channel, rechecks the exact egress/session state, and accounts for hidden
//! Provider HTTP before the adapter submits inference.

use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_core::{CredentialId, GatewayError, ProviderId};
use gateway_provider::{CanonicalEventSource, ProviderFuture};
use gateway_router::{
    ProviderChannelCapability, ProviderChannelIdentity, ProviderEgressChannel,
    ProviderEgressFailureDisposition, ProviderEgressFailureEvidence, ProviderEgressRuntime,
    ProviderEgressRuntimeError, ProviderEgressStateKey, ProviderEgressTargetIdentity,
    ProviderSessionRuntimeState, ProviderSessionStateKey, ProviderTransportAttemptBudget,
    ProviderTransportAttemptBudgetError,
};
use gateway_upstream::CredentialLease;

use crate::{GROK_BUILD_PROVIDER_ID, GROK_CONSOLE_PROVIDER_ID};

const GROK_BUILD_CREDENTIAL_KIND: &str = "grok_build_oauth";
const GROK_CONSOLE_CREDENTIAL_KIND: &str = "grok_console_sso";

/// Clock used only for deterministic local state checks and receipts.
pub trait GrokNativeEgressClock: Send + Sync {
    /// Returns Unix milliseconds in the non-negative timestamp domain.
    ///
    /// # Errors
    ///
    /// Returns a closed error when the clock cannot be represented in the runtime timestamp
    /// domain.
    fn now_ms(&self) -> Result<i64, GrokNativeEgressAttemptError>;
}

/// Production wall clock for one native Provider attempt.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemGrokNativeEgressClock;

impl GrokNativeEgressClock for SystemGrokNativeEgressClock {
    fn now_ms(&self) -> Result<i64, GrokNativeEgressAttemptError> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| GrokNativeEgressAttemptError::ClockUnavailable)?
            .as_millis();
        i64::try_from(millis).map_err(|_| GrokNativeEgressAttemptError::ClockUnavailable)
    }
}

/// Exact Build/Console attempt created from an already-owned CPAR Credential lease.
#[derive(Clone)]
pub struct GrokNativeEgressAttempt {
    runtime: Arc<ProviderEgressRuntime>,
    capability: ProviderChannelCapability,
    egress_key: ProviderEgressStateKey,
    credential_id: CredentialId,
    credential_revision: u64,
    session_key: Option<ProviderSessionStateKey>,
    budget: Arc<Mutex<ProviderTransportAttemptBudget>>,
    clock: Arc<dyn GrokNativeEgressClock>,
}

impl GrokNativeEgressAttempt {
    /// Creates a Build-local attempt from one exact live Credential lease.
    ///
    /// # Errors
    ///
    /// Fails closed for a foreign channel, wrong Credential kind, missing capability, or blocked
    /// exact egress identity.  No Secret bytes are read or retained.
    pub fn try_new_build(
        runtime: Arc<ProviderEgressRuntime>,
        channel: ProviderChannelIdentity,
        target: ProviderEgressTargetIdentity,
        lease: &CredentialLease,
        clock: Arc<dyn GrokNativeEgressClock>,
    ) -> Result<Self, GrokNativeEgressAttemptError> {
        Self::try_new(
            runtime,
            channel,
            target,
            lease,
            None,
            ProviderEgressChannel::GrokBuild,
            GROK_BUILD_PROVIDER_ID,
            GROK_BUILD_CREDENTIAL_KIND,
            clock,
        )
    }

    /// Creates a Console-local attempt and pins one exact `DPoP` session lineage.
    ///
    /// # Errors
    ///
    /// Fails closed for a foreign channel, wrong Credential kind/revision, missing session state,
    /// or blocked exact egress identity.  `session_revision` must be non-zero.
    pub fn try_new_console(
        runtime: Arc<ProviderEgressRuntime>,
        channel: ProviderChannelIdentity,
        target: ProviderEgressTargetIdentity,
        lease: &CredentialLease,
        session_revision: u64,
        clock: Arc<dyn GrokNativeEgressClock>,
    ) -> Result<Self, GrokNativeEgressAttemptError> {
        let session_key = ProviderSessionStateKey::try_new(
            channel.clone(),
            lease.credential_id().clone(),
            lease.credential_revision(),
            session_revision,
        )?;
        Self::try_new(
            runtime,
            channel,
            target,
            lease,
            Some(session_key),
            ProviderEgressChannel::GrokConsole,
            GROK_CONSOLE_PROVIDER_ID,
            GROK_CONSOLE_CREDENTIAL_KIND,
            clock,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_new(
        runtime: Arc<ProviderEgressRuntime>,
        channel: ProviderChannelIdentity,
        target: ProviderEgressTargetIdentity,
        lease: &CredentialLease,
        session_key: Option<ProviderSessionStateKey>,
        expected_channel: ProviderEgressChannel,
        expected_provider: &str,
        expected_credential_kind: &str,
        clock: Arc<dyn GrokNativeEgressClock>,
    ) -> Result<Self, GrokNativeEgressAttemptError> {
        if channel.provider_id().as_str() != expected_provider {
            return Err(GrokNativeEgressAttemptError::ProviderMismatch);
        }
        if lease.endpoint_id() != channel.endpoint_id() {
            return Err(GrokNativeEgressAttemptError::EndpointMismatch);
        }
        if lease.credential_kind() != expected_credential_kind || lease.credential_revision() == 0 {
            return Err(GrokNativeEgressAttemptError::CredentialMismatch);
        }
        let capability = runtime
            .capabilities()
            .capability(&channel)
            .cloned()
            .ok_or(GrokNativeEgressAttemptError::CapabilityUnavailable)?;
        if capability.channel() != expected_channel {
            return Err(GrokNativeEgressAttemptError::ChannelMismatch);
        }
        let egress_key = ProviderEgressStateKey::new(channel, target);
        let now_ms = clock.now_ms()?;
        runtime.require_exact_egress_available(&egress_key, now_ms)?;
        if let Some(key) = &session_key {
            let _state = runtime.session_state_at(key, now_ms)?;
        }
        let budget = ProviderTransportAttemptBudget::for_capability(&capability);
        Ok(Self {
            runtime,
            capability,
            egress_key,
            credential_id: lease.credential_id().clone(),
            credential_revision: lease.credential_revision(),
            session_key,
            budget: Arc::new(Mutex::new(budget)),
            clock,
        })
    }

    /// Returns the exact immutable Provider/Upstream/Endpoint identity.
    #[must_use]
    pub const fn channel(&self) -> &ProviderChannelIdentity {
        self.capability.identity()
    }

    /// Returns the exact Credential ID retained without Secret material.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the exact Credential revision retained by the live lease.
    #[must_use]
    pub const fn credential_revision(&self) -> u64 {
        self.credential_revision
    }

    /// Records a Console DPoP/bootstrap HTTP before inference submission.
    ///
    /// An absent session consumes only one auxiliary slot.  A lost, expired, or challenged
    /// lineage also consumes the sole pre-submit recovery slot.  An invalid session fails closed.
    ///
    /// # Errors
    ///
    /// Returns a closed state or budget error without performing network I/O.
    pub fn begin_console_session_bootstrap(&self) -> Result<(), GrokNativeEgressAttemptError> {
        if self.capability.channel() != ProviderEgressChannel::GrokConsole {
            return Err(GrokNativeEgressAttemptError::ChannelMismatch);
        }
        let key = self
            .session_key
            .as_ref()
            .ok_or(GrokNativeEgressAttemptError::SessionUnavailable)?;
        let now_ms = self.clock.now_ms()?;
        let state = self.runtime.session_state_at(key, now_ms)?;
        let mut budget = self.lock_budget()?;
        match state {
            ProviderSessionRuntimeState::Absent => {}
            ProviderSessionRuntimeState::Active { .. }
            | ProviderSessionRuntimeState::Expired
            | ProviderSessionRuntimeState::ChallengeRequired => {
                budget.record_pre_submit_recovery()?;
            }
            ProviderSessionRuntimeState::Invalid => {
                return Err(GrokNativeEgressAttemptError::SessionUnavailable);
            }
        }
        budget.record_auxiliary_request()?;
        Ok(())
    }

    /// Publishes one successfully constructed Console `DPoP` session for the exact lineage.
    ///
    /// # Errors
    ///
    /// Fails closed for a foreign channel, invalid deadline, or unavailable state registry.
    pub fn complete_console_session_bootstrap(
        &self,
        expires_at_ms: i64,
    ) -> Result<(), GrokNativeEgressAttemptError> {
        let key = self
            .session_key
            .clone()
            .ok_or(GrokNativeEgressAttemptError::SessionUnavailable)?;
        let now_ms = self.clock.now_ms()?;
        self.runtime.set_session_state(
            key,
            ProviderSessionRuntimeState::Active { expires_at_ms },
            now_ms,
        )?;
        Ok(())
    }

    /// Marks only this Console session lineage as requiring a later bounded rebuild.
    ///
    /// # Errors
    ///
    /// Fails closed for a foreign channel or unavailable state registry.
    pub fn require_console_session_rebuild(&self) -> Result<(), GrokNativeEgressAttemptError> {
        let key = self
            .session_key
            .clone()
            .ok_or(GrokNativeEgressAttemptError::SessionUnavailable)?;
        let now_ms = self.clock.now_ms()?;
        self.runtime.set_session_state(
            key,
            ProviderSessionRuntimeState::ChallengeRequired,
            now_ms,
        )?;
        Ok(())
    }

    /// Rechecks exact egress/session state and records the sole inference submission.
    ///
    /// # Errors
    ///
    /// Returns a closed error when the exact egress/session became unavailable or inference was
    /// already submitted.  It never selects a sibling Credential, egress, channel, or Provider.
    pub fn record_inference_submission(&self) -> Result<(), GrokNativeEgressAttemptError> {
        let now_ms = self.clock.now_ms()?;
        self.runtime
            .require_exact_egress_available(&self.egress_key, now_ms)?;
        if let Some(key) = &self.session_key
            && !matches!(
                self.runtime.session_state_at(key, now_ms)?,
                ProviderSessionRuntimeState::Active { .. }
            )
        {
            return Err(GrokNativeEgressAttemptError::SessionUnavailable);
        }
        self.lock_budget()?.record_inference_submission()?;
        Ok(())
    }

    /// Closes every recovery/replay path after the first Canonical event.
    pub fn observe_semantic_event(&self) {
        if let Ok(mut budget) = self.budget.lock() {
            budget.observe_semantic_event();
        }
    }

    /// Classifies sanitized evidence under this exact channel capability.
    ///
    /// # Errors
    ///
    /// Fails closed when the evidence requires a capability this channel does not declare.
    pub fn classify_failure(
        &self,
        evidence: ProviderEgressFailureEvidence,
    ) -> Result<ProviderEgressFailureDisposition, GrokNativeEgressAttemptError> {
        Ok(self.capability.classify_failure(evidence)?)
    }

    /// Returns one value-free point-in-time attempt receipt.
    ///
    /// # Errors
    ///
    /// Returns a closed error if the local ledger lock was poisoned.
    pub fn snapshot(
        &self,
    ) -> Result<GrokNativeEgressAttemptSnapshot, GrokNativeEgressAttemptError> {
        let budget = self.lock_budget()?;
        Ok(GrokNativeEgressAttemptSnapshot {
            channel: self.capability.channel(),
            provider_id: self.capability.identity().provider_id().clone(),
            credential_id: self.credential_id.clone(),
            credential_revision: self.credential_revision,
            auxiliary_requests: budget.auxiliary_requests(),
            pre_submit_recoveries: budget.pre_submit_recoveries(),
            inference_submitted: budget.inference_submitted(),
            semantic_event_observed: budget.semantic_event_observed(),
        })
    }

    fn lock_budget(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, ProviderTransportAttemptBudget>,
        GrokNativeEgressAttemptError,
    > {
        self.budget
            .lock()
            .map_err(|_| GrokNativeEgressAttemptError::LedgerUnavailable)
    }
}

impl fmt::Debug for GrokNativeEgressAttempt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokNativeEgressAttempt")
            .field("channel", &self.capability.channel())
            .field("provider_id", self.capability.identity().provider_id())
            .field("upstream_id", self.capability.identity().upstream_id())
            .field("endpoint_id", self.capability.identity().endpoint_id())
            .field("credential_id", &self.credential_id)
            .field("credential_revision", &self.credential_revision)
            .field("egress_target", self.egress_key.target())
            .field("session_bound", &self.session_key.is_some())
            .field("budget", &"<value-free>")
            .finish_non_exhaustive()
    }
}

/// Value-free audit/test projection for one bounded native attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokNativeEgressAttemptSnapshot {
    channel: ProviderEgressChannel,
    provider_id: ProviderId,
    credential_id: CredentialId,
    credential_revision: u64,
    auxiliary_requests: u8,
    pre_submit_recoveries: u8,
    inference_submitted: bool,
    semantic_event_observed: bool,
}

impl GrokNativeEgressAttemptSnapshot {
    /// Returns the exact closed channel family.
    #[must_use]
    pub const fn channel(&self) -> ProviderEgressChannel {
        self.channel
    }

    /// Returns the fixed adapter Provider identity.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns the exact leased Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the exact leased Credential revision.
    #[must_use]
    pub const fn credential_revision(&self) -> u64 {
        self.credential_revision
    }

    /// Returns counted hidden Provider auxiliary submissions.
    #[must_use]
    pub const fn auxiliary_requests(&self) -> u8 {
        self.auxiliary_requests
    }

    /// Returns counted explicit pre-submit recovery actions.
    #[must_use]
    pub const fn pre_submit_recoveries(&self) -> u8 {
        self.pre_submit_recoveries
    }

    /// Returns whether the sole inference request was submitted.
    #[must_use]
    pub const fn inference_submitted(&self) -> bool {
        self.inference_submitted
    }

    /// Returns whether Canonical output permanently closed replay/recovery.
    #[must_use]
    pub const fn semantic_event_observed(&self) -> bool {
        self.semantic_event_observed
    }
}

/// Canonical source wrapper that closes the attempt budget on the first emitted event.
pub(crate) struct GrokNativeEgressEventSource {
    inner: Box<dyn CanonicalEventSource>,
    attempt: Arc<GrokNativeEgressAttempt>,
    observed: bool,
}

impl GrokNativeEgressEventSource {
    pub(crate) fn new(
        inner: Box<dyn CanonicalEventSource>,
        attempt: Arc<GrokNativeEgressAttempt>,
    ) -> Self {
        Self {
            inner,
            attempt,
            observed: false,
        }
    }
}

impl CanonicalEventSource for GrokNativeEgressEventSource {
    fn next_event(
        &mut self,
    ) -> ProviderFuture<'_, Result<Option<gateway_core::CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            let event = self.inner.next_event().await?;
            if event.is_some() && !self.observed {
                self.attempt.observe_semantic_event();
                self.observed = true;
            }
            Ok(event)
        })
    }
}

/// Closed Build/Console egress-handoff failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokNativeEgressAttemptError {
    /// The fixed adapter Provider identity did not match the declared channel.
    ProviderMismatch,
    /// The registered channel family did not match Build or Console.
    ChannelMismatch,
    /// The exact channel capability was absent.
    CapabilityUnavailable,
    /// The live Credential lease belongs to a different Endpoint-local pool.
    EndpointMismatch,
    /// Credential kind/revision did not match the selected native channel.
    CredentialMismatch,
    /// The exact Console session cannot be used or rebuilt in this attempt.
    SessionUnavailable,
    /// Runtime state rejected the exact identity or deadline.
    Runtime(ProviderEgressRuntimeError),
    /// The finite auxiliary/recovery/inference budget rejected the operation.
    Budget(ProviderTransportAttemptBudgetError),
    /// The value-free local attempt ledger lock was unavailable.
    LedgerUnavailable,
    /// The wall clock could not be projected into Unix milliseconds.
    ClockUnavailable,
}

impl From<ProviderEgressRuntimeError> for GrokNativeEgressAttemptError {
    fn from(error: ProviderEgressRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

impl From<ProviderTransportAttemptBudgetError> for GrokNativeEgressAttemptError {
    fn from(error: ProviderTransportAttemptBudgetError) -> Self {
        Self::Budget(error)
    }
}

impl fmt::Display for GrokNativeEgressAttemptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ProviderMismatch => "native Grok provider identity mismatch",
            Self::ChannelMismatch => "native Grok channel identity mismatch",
            Self::CapabilityUnavailable => "native Grok channel capability unavailable",
            Self::EndpointMismatch => "native Grok Endpoint lease mismatch",
            Self::CredentialMismatch => "native Grok credential binding mismatch",
            Self::SessionUnavailable => "native Grok session unavailable",
            Self::Runtime(_) => "native Grok egress runtime unavailable",
            Self::Budget(_) => "native Grok transport attempt budget exhausted",
            Self::LedgerUnavailable => "native Grok attempt ledger unavailable",
            Self::ClockUnavailable => "native Grok attempt clock unavailable",
        };
        formatter.write_str(message)
    }
}

impl Error for GrokNativeEgressAttemptError {}
