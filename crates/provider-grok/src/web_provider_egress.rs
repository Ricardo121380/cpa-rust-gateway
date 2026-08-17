//! Exact Grok Web sticky-egress, browser-session, and clearance attempt seam.
//!
//! This boundary is deliberately transport-free.  It consumes an already-owned CPAR
//! [`CredentialLease`], exact Provider runtime keys, and sanitized failure evidence.  It never
//! selects an account, performs DNS, opens a proxy, invokes `FlareSolverr`, or retries inference.
//! The sole inference submission and every hidden Statsig/clearance request are instead recorded
//! against the finite P13-11E Provider attempt budget.

use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_core::{CredentialId, ProviderId};
use gateway_router::{
    ProviderChannelCapability, ProviderChannelIdentity, ProviderClearanceRefreshFailure,
    ProviderClearanceRefreshTicket, ProviderClearanceRuntimeState, ProviderClearanceStateKey,
    ProviderEgressChannel, ProviderEgressFailureDisposition, ProviderEgressFailureEvidence,
    ProviderEgressRuntime, ProviderEgressRuntimeError, ProviderEgressStateKey,
    ProviderEgressTargetIdentity, ProviderSessionRuntimeState, ProviderSessionStateKey,
    ProviderTransportAttemptBudget, ProviderTransportAttemptBudgetError,
};
use gateway_upstream::CredentialLease;

use crate::GROK_WEB_PRODUCTION_PROVIDER_ID;

const GROK_WEB_CREDENTIAL_KIND: &str = "grok_web_sso";

/// Clock used only for deterministic local state checks and value-free receipts.
pub trait GrokWebProviderEgressClock: Send + Sync {
    /// Returns Unix milliseconds in the non-negative Provider runtime domain.
    ///
    /// # Errors
    ///
    /// Returns a closed error when the timestamp cannot be represented.
    fn now_ms(&self) -> Result<i64, GrokWebProviderEgressAttemptError>;
}

/// Production wall clock for one Grok Web logical attempt.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemGrokWebProviderEgressClock;

impl GrokWebProviderEgressClock for SystemGrokWebProviderEgressClock {
    fn now_ms(&self) -> Result<i64, GrokWebProviderEgressAttemptError> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| GrokWebProviderEgressAttemptError::ClockUnavailable)?
            .as_millis();
        i64::try_from(millis).map_err(|_| GrokWebProviderEgressAttemptError::ClockUnavailable)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GrokWebProviderEgressLedger {
    budget: ProviderTransportAttemptBudget,
    statsig_environment_requests: u8,
    statsig_signer_requests: u8,
    clearance_refresh_requests: u8,
    clearance_refresh_ticket: Option<ProviderClearanceRefreshTicket>,
    terminal_failure_recorded: bool,
}

/// One exact Grok Web attempt bound to a live CPAR Credential lease and sticky egress lineage.
#[derive(Clone)]
pub struct GrokWebProviderEgressAttempt {
    runtime: Arc<ProviderEgressRuntime>,
    capability: ProviderChannelCapability,
    egress_key: ProviderEgressStateKey,
    session_key: ProviderSessionStateKey,
    clearance_key: ProviderClearanceStateKey,
    credential_id: CredentialId,
    credential_revision: u64,
    ledger: Arc<Mutex<GrokWebProviderEgressLedger>>,
    clock: Arc<dyn GrokWebProviderEgressClock>,
}

impl GrokWebProviderEgressAttempt {
    /// Creates one exact, transport-free Web attempt from an already-owned Credential lease.
    ///
    /// The caller supplies the exact session and clearance keys so a foreign Provider, Endpoint,
    /// account revision, session lineage, or sticky target cannot be inferred or silently
    /// substituted.  `Direct` is never a valid Web target.
    ///
    /// # Errors
    ///
    /// Fails closed for every identity mismatch, missing capability/state, inactive session,
    /// invalid clearance lineage, or unavailable exact sticky egress.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        runtime: Arc<ProviderEgressRuntime>,
        channel: ProviderChannelIdentity,
        target: ProviderEgressTargetIdentity,
        lease: &CredentialLease,
        session_key: ProviderSessionStateKey,
        clearance_key: ProviderClearanceStateKey,
        clock: Arc<dyn GrokWebProviderEgressClock>,
    ) -> Result<Self, GrokWebProviderEgressAttemptError> {
        if channel.provider_id().as_str() != GROK_WEB_PRODUCTION_PROVIDER_ID {
            return Err(GrokWebProviderEgressAttemptError::ProviderMismatch);
        }
        if target.as_named().is_none() {
            return Err(GrokWebProviderEgressAttemptError::StickyTargetRequired);
        }
        if lease.endpoint_id() != channel.endpoint_id() {
            return Err(GrokWebProviderEgressAttemptError::EndpointMismatch);
        }
        if lease.credential_kind() != GROK_WEB_CREDENTIAL_KIND || lease.credential_revision() == 0 {
            return Err(GrokWebProviderEgressAttemptError::CredentialMismatch);
        }
        let capability = runtime
            .capabilities()
            .capability(&channel)
            .cloned()
            .ok_or(GrokWebProviderEgressAttemptError::CapabilityUnavailable)?;
        if capability.channel() != ProviderEgressChannel::GrokWeb {
            return Err(GrokWebProviderEgressAttemptError::ChannelMismatch);
        }
        if session_key.channel() != &channel
            || session_key.credential_id() != lease.credential_id()
            || session_key.credential_revision() != lease.credential_revision()
        {
            return Err(GrokWebProviderEgressAttemptError::SessionKeyMismatch);
        }
        if clearance_key.session() != &session_key || clearance_key.target() != &target {
            return Err(GrokWebProviderEgressAttemptError::ClearanceKeyMismatch);
        }

        let egress_key = ProviderEgressStateKey::new(channel, target);
        let now_ms = clock.now_ms()?;
        runtime.require_exact_egress_available(&egress_key, now_ms)?;
        if !matches!(
            runtime.session_state_at(&session_key, now_ms)?,
            ProviderSessionRuntimeState::Active { .. }
        ) {
            return Err(GrokWebProviderEgressAttemptError::SessionUnavailable);
        }
        match runtime.clearance_state_at(&clearance_key, now_ms)? {
            ProviderClearanceRuntimeState::Absent
            | ProviderClearanceRuntimeState::Fresh { .. }
            | ProviderClearanceRuntimeState::Expired
            | ProviderClearanceRuntimeState::RefreshRequired => {}
            ProviderClearanceRuntimeState::RefreshInFlight { .. } => {
                return Err(GrokWebProviderEgressAttemptError::ClearanceRecoveryInFlight);
            }
            ProviderClearanceRuntimeState::Invalid => {
                return Err(GrokWebProviderEgressAttemptError::ClearanceUnavailable);
            }
        }

        let budget = ProviderTransportAttemptBudget::for_capability(&capability);
        Ok(Self {
            runtime,
            capability,
            egress_key,
            session_key,
            clearance_key,
            credential_id: lease.credential_id().clone(),
            credential_revision: lease.credential_revision(),
            ledger: Arc::new(Mutex::new(GrokWebProviderEgressLedger {
                budget,
                statsig_environment_requests: 0,
                statsig_signer_requests: 0,
                clearance_refresh_requests: 0,
                clearance_refresh_ticket: None,
                terminal_failure_recorded: false,
            })),
            clock,
        })
    }

    /// Returns the immutable Provider/Upstream/Endpoint namespace.
    #[must_use]
    pub const fn channel(&self) -> &ProviderChannelIdentity {
        self.capability.identity()
    }

    /// Returns the exact non-secret Credential identity retained from the live lease.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the exact leased Credential revision.
    #[must_use]
    pub const fn credential_revision(&self) -> u64 {
        self.credential_revision
    }

    /// Records one hidden Statsig environment request before inference.
    ///
    /// # Errors
    ///
    /// Fails closed after the finite four-request Web bound, inference submission, or semantic
    /// output.  No HTTP call is performed here.
    pub fn record_statsig_environment_request(
        &self,
    ) -> Result<(), GrokWebProviderEgressAttemptError> {
        self.require_statsig_pre_inference_state()?;
        let mut ledger = self.lock_ledger()?;
        ledger.budget.record_auxiliary_request()?;
        ledger.statsig_environment_requests = ledger.statsig_environment_requests.saturating_add(1);
        Ok(())
    }

    /// Records one hidden Statsig signer request before inference.
    ///
    /// # Errors
    ///
    /// Fails closed after the finite four-request Web bound, inference submission, or semantic
    /// output.  No HTTP call is performed here.
    pub fn record_statsig_signer_request(&self) -> Result<(), GrokWebProviderEgressAttemptError> {
        self.require_statsig_pre_inference_state()?;
        let mut ledger = self.lock_ledger()?;
        ledger.budget.record_auxiliary_request()?;
        ledger.statsig_signer_requests = ledger.statsig_signer_requests.saturating_add(1);
        Ok(())
    }

    /// Claims the sole bounded pre-inference refresh for this exact clearance lineage.
    ///
    /// The transition is admitted only from `Expired` or `RefreshRequired`.  It accounts for one
    /// recovery and one hidden clearance request before publishing `RefreshInFlight`; callers may
    /// then run an injected fake/production refresher outside this transport-free seam.
    ///
    /// # Errors
    ///
    /// Fails closed for a foreign/unavailable lineage, invalid ticket deadline, exhausted budget,
    /// or an already submitted inference.  It never rotates to a sibling target or account.
    pub fn begin_clearance_refresh(
        &self,
        ticket_expires_at_ms: i64,
    ) -> Result<(), GrokWebProviderEgressAttemptError> {
        let now_ms = self.require_exact_pre_inference_state()?;
        let mut ledger = self.lock_ledger()?;
        if ledger.clearance_refresh_ticket.is_some() {
            return Err(GrokWebProviderEgressAttemptError::ClearanceRecoveryInFlight);
        }
        let mut next_budget = ledger.budget.clone();
        next_budget.record_pre_submit_recovery()?;
        next_budget.record_auxiliary_request()?;
        let ticket = self.runtime.begin_exact_clearance_refresh(
            &self.clearance_key,
            now_ms,
            ticket_expires_at_ms,
        )?;
        ledger.budget = next_budget;
        ledger.clearance_refresh_requests = ledger.clearance_refresh_requests.saturating_add(1);
        ledger.clearance_refresh_ticket = Some(ticket);
        Ok(())
    }

    /// Publishes one fresh result for the exact in-flight clearance lineage.
    ///
    /// # Errors
    ///
    /// Fails closed when no unexpired exact refresh is in flight or the new deadline is invalid.
    pub fn complete_clearance_refresh(
        &self,
        clearance_expires_at_ms: i64,
    ) -> Result<(), GrokWebProviderEgressAttemptError> {
        let now_ms = self.clock.now_ms()?;
        let mut ledger = self.lock_ledger()?;
        if ledger.budget.semantic_event_observed() {
            return Err(GrokWebProviderEgressAttemptError::Budget(
                ProviderTransportAttemptBudgetError::SemanticEventClosed,
            ));
        }
        let ticket = ledger
            .clearance_refresh_ticket
            .as_ref()
            .ok_or(GrokWebProviderEgressAttemptError::ClearanceRecoveryUnavailable)?;
        self.runtime
            .complete_exact_clearance_refresh(ticket, now_ms, clearance_expires_at_ms)?;
        ledger.clearance_refresh_ticket = None;
        Ok(())
    }

    /// Returns one failed exact refresh to `RefreshRequired` for a later logical attempt.
    ///
    /// # Errors
    ///
    /// Fails closed when this exact lineage has no unexpired refresh in flight.
    pub fn fail_clearance_refresh(&self) -> Result<(), GrokWebProviderEgressAttemptError> {
        let now_ms = self.clock.now_ms()?;
        let mut ledger = self.lock_ledger()?;
        if ledger.budget.semantic_event_observed() {
            return Err(GrokWebProviderEgressAttemptError::Budget(
                ProviderTransportAttemptBudgetError::SemanticEventClosed,
            ));
        }
        let ticket = ledger
            .clearance_refresh_ticket
            .as_ref()
            .ok_or(GrokWebProviderEgressAttemptError::ClearanceRecoveryUnavailable)?;
        self.runtime.fail_exact_clearance_refresh(
            ticket,
            now_ms,
            ProviderClearanceRefreshFailure::RetryRequired,
        )?;
        ledger.clearance_refresh_ticket = None;
        Ok(())
    }

    /// Rechecks exact sticky egress/session/clearance state and records the sole inference.
    ///
    /// `Absent` clearance is admissible because not every valid Web session needs a clearance
    /// cookie.  `Expired`, challenged, in-flight, or invalid clearance must fail before transport.
    ///
    /// # Errors
    ///
    /// Rejects blocked exact state, a second inference, or any submission after semantic output.
    pub fn record_inference_submission(&self) -> Result<(), GrokWebProviderEgressAttemptError> {
        let now_ms = self.require_exact_pre_inference_state()?;
        if !matches!(
            self.runtime
                .clearance_state_at(&self.clearance_key, now_ms)?,
            ProviderClearanceRuntimeState::Absent | ProviderClearanceRuntimeState::Fresh { .. }
        ) {
            return Err(GrokWebProviderEgressAttemptError::ClearanceUnavailable);
        }
        self.lock_ledger()?.budget.record_inference_submission()?;
        Ok(())
    }

    /// Irreversibly closes auxiliary, recovery, and replay after the first Canonical event.
    pub fn observe_semantic_event(&self) {
        if let Ok(mut ledger) = self.ledger.lock() {
            ledger.budget.observe_semantic_event();
        }
    }

    /// Classifies and records one sanitized failure after the sole inference submission.
    ///
    /// Unknown 403 remains ambiguous and changes no state.  Confirmed forbidden evidence belongs
    /// only to the Credential owner and changes no egress/session/clearance state.  An explicit
    /// clearance challenge marks only this exact clearance `RefreshRequired` for a *later* logical
    /// attempt; the current budget already forbids an inference retry.  The first successful
    /// terminal observation latches this attempt, so later failure evidence cannot be reclassified.
    ///
    /// # Errors
    ///
    /// Rejects evidence before inference, after semantic output, after a prior terminal failure,
    /// or against unavailable exact runtime state.
    pub fn record_sanitized_failure(
        &self,
        evidence: ProviderEgressFailureEvidence,
    ) -> Result<ProviderEgressFailureDisposition, GrokWebProviderEgressAttemptError> {
        let disposition = self.capability.classify_failure(evidence)?;
        let mut ledger = self.lock_ledger()?;
        if ledger.terminal_failure_recorded {
            return Err(GrokWebProviderEgressAttemptError::FailureAlreadyRecorded);
        }
        if ledger.budget.semantic_event_observed() {
            return Err(GrokWebProviderEgressAttemptError::FailureAfterSemanticEvent);
        }
        if !ledger.budget.inference_submitted() {
            return Err(GrokWebProviderEgressAttemptError::FailureBeforeInference);
        }
        if matches!(evidence, ProviderEgressFailureEvidence::ClearanceChallenge) {
            let now_ms = self.clock.now_ms()?;
            self.runtime
                .require_exact_clearance_refresh(&self.clearance_key, now_ms)?;
        }
        ledger.terminal_failure_recorded = true;
        Ok(disposition)
    }

    /// Returns one value-free point-in-time attempt receipt.
    ///
    /// # Errors
    ///
    /// Returns a closed error if the local ledger lock is unavailable.
    pub fn snapshot(
        &self,
    ) -> Result<GrokWebProviderEgressAttemptSnapshot, GrokWebProviderEgressAttemptError> {
        let ledger = self.lock_ledger()?;
        Ok(GrokWebProviderEgressAttemptSnapshot {
            provider_id: self.capability.identity().provider_id().clone(),
            credential_id: self.credential_id.clone(),
            credential_revision: self.credential_revision,
            session_revision: self.session_key.session_revision(),
            clearance_revision: self.clearance_key.clearance_revision(),
            statsig_environment_requests: ledger.statsig_environment_requests,
            statsig_signer_requests: ledger.statsig_signer_requests,
            clearance_refresh_requests: ledger.clearance_refresh_requests,
            auxiliary_requests: ledger.budget.auxiliary_requests(),
            pre_submit_recoveries: ledger.budget.pre_submit_recoveries(),
            inference_submitted: ledger.budget.inference_submitted(),
            semantic_event_observed: ledger.budget.semantic_event_observed(),
            terminal_failure_recorded: ledger.terminal_failure_recorded,
        })
    }

    fn require_exact_pre_inference_state(&self) -> Result<i64, GrokWebProviderEgressAttemptError> {
        let now_ms = self.clock.now_ms()?;
        self.runtime
            .require_exact_egress_available(&self.egress_key, now_ms)?;
        if !matches!(
            self.runtime.session_state_at(&self.session_key, now_ms)?,
            ProviderSessionRuntimeState::Active { .. }
        ) {
            return Err(GrokWebProviderEgressAttemptError::SessionUnavailable);
        }
        Ok(now_ms)
    }

    fn require_statsig_pre_inference_state(&self) -> Result<(), GrokWebProviderEgressAttemptError> {
        let now_ms = self.require_exact_pre_inference_state()?;
        if !matches!(
            self.runtime
                .clearance_state_at(&self.clearance_key, now_ms)?,
            ProviderClearanceRuntimeState::Absent | ProviderClearanceRuntimeState::Fresh { .. }
        ) {
            return Err(GrokWebProviderEgressAttemptError::ClearanceUnavailable);
        }
        Ok(())
    }

    fn lock_ledger(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, GrokWebProviderEgressLedger>,
        GrokWebProviderEgressAttemptError,
    > {
        self.ledger
            .lock()
            .map_err(|_| GrokWebProviderEgressAttemptError::LedgerUnavailable)
    }
}

impl fmt::Debug for GrokWebProviderEgressAttempt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebProviderEgressAttempt")
            .field("channel", &ProviderEgressChannel::GrokWeb)
            .field("provider_id", self.capability.identity().provider_id())
            .field("upstream_id", self.capability.identity().upstream_id())
            .field("endpoint_id", self.capability.identity().endpoint_id())
            .field("credential_id", &self.credential_id)
            .field("credential_revision", &self.credential_revision)
            .field("sticky_target", &"<exact named target>")
            .field("session_revision", &self.session_key.session_revision())
            .field(
                "clearance_revision",
                &self.clearance_key.clearance_revision(),
            )
            .field("ledger", &"<value-free>")
            .finish_non_exhaustive()
    }
}

/// Value-free audit/test projection for one bounded Web attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokWebProviderEgressAttemptSnapshot {
    provider_id: ProviderId,
    credential_id: CredentialId,
    credential_revision: u64,
    session_revision: u64,
    clearance_revision: u64,
    statsig_environment_requests: u8,
    statsig_signer_requests: u8,
    clearance_refresh_requests: u8,
    auxiliary_requests: u8,
    pre_submit_recoveries: u8,
    inference_submitted: bool,
    semantic_event_observed: bool,
    terminal_failure_recorded: bool,
}

impl GrokWebProviderEgressAttemptSnapshot {
    /// Returns the fixed Web Provider identity.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns the exact non-secret Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the exact leased Credential revision.
    #[must_use]
    pub const fn credential_revision(&self) -> u64 {
        self.credential_revision
    }

    /// Returns the exact Provider-session lineage revision.
    #[must_use]
    pub const fn session_revision(&self) -> u64 {
        self.session_revision
    }

    /// Returns the exact clearance lineage revision.
    #[must_use]
    pub const fn clearance_revision(&self) -> u64 {
        self.clearance_revision
    }

    /// Returns counted Statsig environment requests.
    #[must_use]
    pub const fn statsig_environment_requests(&self) -> u8 {
        self.statsig_environment_requests
    }

    /// Returns counted Statsig signer requests.
    #[must_use]
    pub const fn statsig_signer_requests(&self) -> u8 {
        self.statsig_signer_requests
    }

    /// Returns counted clearance refresh transport requests.
    #[must_use]
    pub const fn clearance_refresh_requests(&self) -> u8 {
        self.clearance_refresh_requests
    }

    /// Returns all hidden Provider auxiliary submissions.
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

    /// Returns whether semantic output permanently closed the attempt.
    #[must_use]
    pub const fn semantic_event_observed(&self) -> bool {
        self.semantic_event_observed
    }

    /// Returns whether one sanitized terminal failure has been recorded successfully.
    #[must_use]
    pub const fn terminal_failure_recorded(&self) -> bool {
        self.terminal_failure_recorded
    }
}

/// Closed exact Web attempt failure; no variant contains Provider values or Secret material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebProviderEgressAttemptError {
    /// The declared channel did not use the fixed `grok.web` Provider namespace.
    ProviderMismatch,
    /// The registered channel capability was not Web.
    ChannelMismatch,
    /// The exact channel capability was absent.
    CapabilityUnavailable,
    /// The live Credential lease belongs to a different Endpoint-local pool.
    EndpointMismatch,
    /// Credential kind or revision was not an exact Web SSO lease.
    CredentialMismatch,
    /// Web requires an explicit named sticky egress target.
    StickyTargetRequired,
    /// The session key did not exactly match channel, Credential, or revision.
    SessionKeyMismatch,
    /// The clearance key did not exactly match session or sticky target.
    ClearanceKeyMismatch,
    /// The exact Web session is not active.
    SessionUnavailable,
    /// The exact clearance cannot be used or refreshed.
    ClearanceUnavailable,
    /// Another bounded refresh is already in flight for the exact lineage.
    ClearanceRecoveryInFlight,
    /// The exact clearance does not currently require a refresh.
    ClearanceRefreshNotRequired,
    /// No unexpired exact clearance refresh can be completed or failed.
    ClearanceRecoveryUnavailable,
    /// Failure evidence was supplied before the inference transport boundary.
    FailureBeforeInference,
    /// Failure evidence was supplied after semantic output closed the attempt.
    FailureAfterSemanticEvent,
    /// One successful terminal failure already closed this exact logical attempt.
    FailureAlreadyRecorded,
    /// Runtime state rejected the exact identity or deadline.
    Runtime(ProviderEgressRuntimeError),
    /// The finite auxiliary/recovery/inference budget rejected the operation.
    Budget(ProviderTransportAttemptBudgetError),
    /// The value-free local ledger lock was unavailable.
    LedgerUnavailable,
    /// The wall clock could not be projected into Unix milliseconds.
    ClockUnavailable,
}

impl From<ProviderEgressRuntimeError> for GrokWebProviderEgressAttemptError {
    fn from(error: ProviderEgressRuntimeError) -> Self {
        match error {
            ProviderEgressRuntimeError::ClearanceRefreshInFlight => Self::ClearanceRecoveryInFlight,
            ProviderEgressRuntimeError::ClearanceRefreshNotRequired => {
                Self::ClearanceRefreshNotRequired
            }
            ProviderEgressRuntimeError::ClearanceInvalid => Self::ClearanceUnavailable,
            ProviderEgressRuntimeError::ClearanceRefreshTicketMismatch
            | ProviderEgressRuntimeError::ClearanceRefreshTicketExpired
            | ProviderEgressRuntimeError::ClearanceRefreshNotInFlight => {
                Self::ClearanceRecoveryUnavailable
            }
            other => Self::Runtime(other),
        }
    }
}

impl From<ProviderTransportAttemptBudgetError> for GrokWebProviderEgressAttemptError {
    fn from(error: ProviderTransportAttemptBudgetError) -> Self {
        Self::Budget(error)
    }
}

impl fmt::Display for GrokWebProviderEgressAttemptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ProviderMismatch => "Grok Web provider identity mismatch",
            Self::ChannelMismatch => "Grok Web channel identity mismatch",
            Self::CapabilityUnavailable => "Grok Web channel capability unavailable",
            Self::EndpointMismatch => "Grok Web Endpoint lease mismatch",
            Self::CredentialMismatch => "Grok Web credential binding mismatch",
            Self::StickyTargetRequired => "Grok Web named sticky target required",
            Self::SessionKeyMismatch => "Grok Web session key mismatch",
            Self::ClearanceKeyMismatch => "Grok Web clearance key mismatch",
            Self::SessionUnavailable => "Grok Web session unavailable",
            Self::ClearanceUnavailable => "Grok Web clearance unavailable",
            Self::ClearanceRecoveryInFlight => "Grok Web clearance recovery is already in flight",
            Self::ClearanceRefreshNotRequired => "Grok Web clearance refresh is not required",
            Self::ClearanceRecoveryUnavailable => "Grok Web clearance recovery unavailable",
            Self::FailureBeforeInference => "Grok Web failure preceded inference submission",
            Self::FailureAfterSemanticEvent => "Grok Web semantic event closed failure handling",
            Self::FailureAlreadyRecorded => "Grok Web terminal failure was already recorded",
            Self::Runtime(_) => "Grok Web egress runtime unavailable",
            Self::Budget(_) => "Grok Web transport attempt budget exhausted",
            Self::LedgerUnavailable => "Grok Web attempt ledger unavailable",
            Self::ClockUnavailable => "Grok Web attempt clock unavailable",
        })
    }
}

impl Error for GrokWebProviderEgressAttemptError {}
