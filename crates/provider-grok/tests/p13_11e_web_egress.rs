//! P13-11E E3 local-only Grok Web sticky egress/session/clearance evidence.

#![deny(unsafe_code)]

use std::{
    error::Error,
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use gateway_core::{CredentialId, EndpointId, ProviderId, UpstreamId};
use gateway_router::{
    ProviderAccountEvidence, ProviderChannelCapability, ProviderChannelCapabilityRegistry,
    ProviderChannelIdentity, ProviderClearanceRuntimeState, ProviderClearanceStateKey,
    ProviderEgressChannel, ProviderEgressFailureEvidence, ProviderEgressFailureOwner,
    ProviderEgressRecoveryAction, ProviderEgressRuntime, ProviderEgressRuntimeState,
    ProviderEgressStateKey, ProviderEgressTargetIdentity, ProviderSessionRuntimeState,
    ProviderSessionStateKey, ProviderTransportAttemptBudgetError,
};
use gateway_upstream::{
    CredentialLease, CredentialSecret, EndpointCredentialInput, EndpointCredentialPool,
};
use provider_grok::{
    GrokWebProviderEgressAttempt, GrokWebProviderEgressAttemptError, GrokWebProviderEgressClock,
};

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 50_000;
const SECRET_SENTINEL: &[u8] = b"p13-e3-synthetic-secret-must-not-appear";

#[derive(Clone, Copy, Debug)]
struct FixedClock(i64);

impl GrokWebProviderEgressClock for FixedClock {
    fn now_ms(&self) -> Result<i64, GrokWebProviderEgressAttemptError> {
        Ok(self.0)
    }
}

#[derive(Debug)]
struct BlockingClock {
    calls: AtomicUsize,
    block_on_call: usize,
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
}

impl GrokWebProviderEgressClock for BlockingClock {
    fn now_ms(&self) -> Result<i64, GrokWebProviderEgressAttemptError> {
        let call = self.calls.fetch_add(1, Ordering::AcqRel) + 1;
        if call == self.block_on_call {
            self.entered.wait();
            self.release.wait();
        }
        Ok(NOW_MS)
    }
}

#[test]
fn statsig_auxiliary_budget_is_zero_then_two_then_four_and_fifth_is_closed() -> TestResult {
    let fixture = WebFixture::new("aux", ProviderClearanceRuntimeState::Absent)?;
    let attempt = fixture.attempt()?;
    assert_eq!(attempt.snapshot()?.auxiliary_requests(), 0);

    attempt.record_statsig_environment_request()?;
    attempt.record_statsig_signer_request()?;
    let two = attempt.snapshot()?;
    assert_eq!(two.auxiliary_requests(), 2);
    assert_eq!(two.statsig_environment_requests(), 1);
    assert_eq!(two.statsig_signer_requests(), 1);

    attempt.record_statsig_environment_request()?;
    attempt.record_statsig_signer_request()?;
    assert_eq!(attempt.snapshot()?.auxiliary_requests(), 4);
    assert_eq!(
        attempt.record_statsig_environment_request(),
        Err(GrokWebProviderEgressAttemptError::Budget(
            ProviderTransportAttemptBudgetError::AuxiliaryRequestLimit
        ))
    );
    assert_eq!(attempt.snapshot()?.auxiliary_requests(), 4);
    Ok(())
}

#[test]
fn clearance_refresh_has_one_atomic_ticket_and_one_attempt_budget() -> TestResult {
    let fixture = WebFixture::new("refresh", ProviderClearanceRuntimeState::RefreshRequired)?;
    let attempt = fixture.attempt()?;
    attempt.begin_clearance_refresh(NOW_MS + 1_000)?;
    assert_eq!(
        fixture
            .runtime
            .clearance_state_at(&fixture.clearance_key, NOW_MS)?,
        ProviderClearanceRuntimeState::RefreshInFlight {
            expires_at_ms: NOW_MS + 1_000
        }
    );
    assert_eq!(
        attempt.begin_clearance_refresh(NOW_MS + 2_000),
        Err(GrokWebProviderEgressAttemptError::ClearanceRecoveryInFlight)
    );
    attempt.fail_clearance_refresh()?;
    assert_eq!(
        fixture
            .runtime
            .clearance_state_at(&fixture.clearance_key, NOW_MS)?,
        ProviderClearanceRuntimeState::RefreshRequired
    );
    assert_eq!(
        attempt.begin_clearance_refresh(NOW_MS + 2_000),
        Err(GrokWebProviderEgressAttemptError::Budget(
            ProviderTransportAttemptBudgetError::RecoveryLimit
        ))
    );
    let snapshot = attempt.snapshot()?;
    assert_eq!(snapshot.clearance_refresh_requests(), 1);
    assert_eq!(snapshot.pre_submit_recoveries(), 1);
    assert_eq!(snapshot.auxiliary_requests(), 1);
    Ok(())
}

#[test]
fn completed_clearance_admits_exactly_one_inference_and_semantic_closure() -> TestResult {
    let fixture = WebFixture::new("complete", ProviderClearanceRuntimeState::RefreshRequired)?;
    let attempt = fixture.attempt()?;
    attempt.begin_clearance_refresh(NOW_MS + 1_000)?;
    attempt.complete_clearance_refresh(NOW_MS + 10_000)?;
    attempt.record_inference_submission()?;
    assert_eq!(
        attempt.record_inference_submission(),
        Err(GrokWebProviderEgressAttemptError::Budget(
            ProviderTransportAttemptBudgetError::InferenceAlreadySubmitted
        ))
    );
    attempt.observe_semantic_event();
    assert_eq!(
        attempt.record_statsig_signer_request(),
        Err(GrokWebProviderEgressAttemptError::Budget(
            ProviderTransportAttemptBudgetError::SemanticEventClosed
        ))
    );
    assert_eq!(
        attempt.record_sanitized_failure(ProviderEgressFailureEvidence::ClearanceChallenge),
        Err(GrokWebProviderEgressAttemptError::FailureAfterSemanticEvent)
    );
    assert!(attempt.snapshot()?.semantic_event_observed());
    Ok(())
}

#[test]
fn semantic_event_closes_inflight_clearance_completion_and_failure() -> TestResult {
    let fixture = WebFixture::new(
        "refresh-semantic-closure",
        ProviderClearanceRuntimeState::RefreshRequired,
    )?;
    let attempt = fixture.attempt()?;
    attempt.begin_clearance_refresh(NOW_MS + 1_000)?;
    attempt.observe_semantic_event();

    let closed = Err(GrokWebProviderEgressAttemptError::Budget(
        ProviderTransportAttemptBudgetError::SemanticEventClosed,
    ));
    assert_eq!(attempt.complete_clearance_refresh(NOW_MS + 10_000), closed);
    assert_eq!(attempt.fail_clearance_refresh(), closed);
    assert_eq!(
        fixture
            .runtime
            .clearance_state_at(&fixture.clearance_key, NOW_MS)?,
        ProviderClearanceRuntimeState::RefreshInFlight {
            expires_at_ms: NOW_MS + 1_000
        }
    );
    assert!(attempt.snapshot()?.semantic_event_observed());
    Ok(())
}

#[test]
fn unknown_and_confirmed_forbidden_evidence_do_not_mutate_runtime_state() -> TestResult {
    let unknown_fixture = WebFixture::new(
        "unknown-403",
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 10_000,
        },
    )?;
    let unknown = unknown_fixture.attempt()?;
    unknown.record_inference_submission()?;
    let disposition =
        unknown.record_sanitized_failure(ProviderEgressFailureEvidence::HttpForbidden {
            account_evidence: ProviderAccountEvidence::None,
        })?;
    assert_eq!(
        disposition.owner(),
        ProviderEgressFailureOwner::AmbiguousProvider
    );
    assert_eq!(disposition.action(), ProviderEgressRecoveryAction::None);
    assert_eq!(
        unknown_fixture
            .runtime
            .clearance_state_at(&unknown_fixture.clearance_key, NOW_MS)?,
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 10_000
        }
    );
    assert_eq!(
        unknown_fixture
            .runtime
            .session_state_at(&unknown_fixture.session_key, NOW_MS,)?,
        ProviderSessionRuntimeState::Active {
            expires_at_ms: NOW_MS + 20_000
        }
    );
    assert_eq!(
        unknown_fixture.runtime.egress_state_at(
            &ProviderEgressStateKey::new(
                unknown_fixture.identity.clone(),
                unknown_fixture.target.clone(),
            ),
            NOW_MS,
        )?,
        ProviderEgressRuntimeState::Available
    );

    let confirmed_fixture = WebFixture::new(
        "confirmed-403",
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 10_000,
        },
    )?;
    let confirmed = confirmed_fixture.attempt()?;
    confirmed.record_inference_submission()?;
    let disposition =
        confirmed.record_sanitized_failure(ProviderEgressFailureEvidence::HttpForbidden {
            account_evidence: ProviderAccountEvidence::ConfirmedForbidden,
        })?;
    assert_eq!(disposition.owner(), ProviderEgressFailureOwner::Credential);
    assert_eq!(
        disposition.action(),
        ProviderEgressRecoveryAction::RequireCredentialReplacement
    );
    assert_eq!(
        confirmed_fixture
            .runtime
            .clearance_state_at(&confirmed_fixture.clearance_key, NOW_MS)?,
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 10_000
        }
    );
    assert_eq!(
        confirmed_fixture
            .runtime
            .session_state_at(&confirmed_fixture.session_key, NOW_MS,)?,
        ProviderSessionRuntimeState::Active {
            expires_at_ms: NOW_MS + 20_000
        }
    );
    Ok(())
}

#[test]
fn first_successful_failure_latches_terminal_and_failed_mutation_does_not() -> TestResult {
    let terminal = WebFixture::new(
        "terminal-failure",
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 10_000,
        },
    )?;
    let terminal_attempt = terminal.attempt()?;
    terminal_attempt.record_inference_submission()?;
    terminal_attempt.record_sanitized_failure(ProviderEgressFailureEvidence::HttpForbidden {
        account_evidence: ProviderAccountEvidence::None,
    })?;
    assert!(terminal_attempt.snapshot()?.terminal_failure_recorded());
    assert_eq!(
        terminal_attempt.record_sanitized_failure(ProviderEgressFailureEvidence::HttpForbidden {
            account_evidence: ProviderAccountEvidence::ConfirmedForbidden,
        },),
        Err(GrokWebProviderEgressAttemptError::FailureAlreadyRecorded)
    );
    terminal_attempt.observe_semantic_event();
    assert_eq!(
        terminal_attempt
            .record_sanitized_failure(ProviderEgressFailureEvidence::ClearanceChallenge),
        Err(GrokWebProviderEgressAttemptError::FailureAlreadyRecorded)
    );

    let failed_mutation = WebFixture::new(
        "failed-mutation",
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 10_000,
        },
    )?;
    let failed_attempt = failed_mutation.attempt()?;
    failed_attempt.record_inference_submission()?;
    failed_mutation.runtime.set_clearance_state(
        failed_mutation.clearance_key.clone(),
        ProviderClearanceRuntimeState::Invalid,
        NOW_MS,
    )?;
    assert_eq!(
        failed_attempt.record_sanitized_failure(ProviderEgressFailureEvidence::ClearanceChallenge),
        Err(GrokWebProviderEgressAttemptError::ClearanceUnavailable)
    );
    assert!(!failed_attempt.snapshot()?.terminal_failure_recorded());
    failed_attempt.record_sanitized_failure(ProviderEgressFailureEvidence::HttpForbidden {
        account_evidence: ProviderAccountEvidence::None,
    })?;
    assert!(failed_attempt.snapshot()?.terminal_failure_recorded());
    Ok(())
}

#[test]
fn semantic_observation_cannot_cross_challenge_mutation_critical_section() -> TestResult {
    let fixture = WebFixture::new(
        "failure-lock",
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 10_000,
        },
    )?;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let attempt = fixture.attempt_with_clock(Arc::new(BlockingClock {
        calls: AtomicUsize::new(0),
        block_on_call: 3,
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    }))?;
    attempt.record_inference_submission()?;

    let failure_attempt = attempt.clone();
    let failure = thread::spawn(move || {
        failure_attempt.record_sanitized_failure(ProviderEgressFailureEvidence::ClearanceChallenge)
    });
    entered.wait();

    let semantic_attempt = attempt.clone();
    let (semantic_tx, semantic_rx) = mpsc::channel();
    let semantic = thread::spawn(move || {
        semantic_attempt.observe_semantic_event();
        let _ = semantic_tx.send(());
    });
    assert_eq!(
        semantic_rx.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    release.wait();
    let disposition = failure.join().map_err(|_| "failure thread panicked")??;
    semantic_rx.recv_timeout(Duration::from_secs(1))?;
    semantic.join().map_err(|_| "semantic thread panicked")?;
    assert_eq!(disposition.owner(), ProviderEgressFailureOwner::Clearance);
    assert_eq!(
        fixture
            .runtime
            .clearance_state_at(&fixture.clearance_key, NOW_MS)?,
        ProviderClearanceRuntimeState::RefreshRequired
    );
    let snapshot = attempt.snapshot()?;
    assert!(snapshot.terminal_failure_recorded());
    assert!(snapshot.semantic_event_observed());
    Ok(())
}

#[test]
fn explicit_challenge_marks_only_exact_clearance_for_next_attempt() -> TestResult {
    let first = WebFixture::new(
        "challenge-first",
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 10_000,
        },
    )?;
    let sibling_target = ProviderEgressTargetIdentity::named("challenge-sibling-target")?;
    let sibling_session = ProviderSessionStateKey::try_new(
        first.identity.clone(),
        CredentialId::try_new("challenge-sibling-account")?,
        12,
        12,
    )?;
    let sibling_clearance =
        ProviderClearanceStateKey::try_new(sibling_session.clone(), sibling_target.clone(), 12)?;
    first.runtime.set_egress_state(
        ProviderEgressStateKey::new(first.identity.clone(), sibling_target),
        ProviderEgressRuntimeState::Available,
        NOW_MS,
    )?;
    first.runtime.set_session_state(
        sibling_session,
        ProviderSessionRuntimeState::Active {
            expires_at_ms: NOW_MS + 20_000,
        },
        NOW_MS,
    )?;
    first.runtime.set_clearance_state(
        sibling_clearance.clone(),
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 20_000,
        },
        NOW_MS,
    )?;

    let attempt = first.attempt()?;
    attempt.record_inference_submission()?;
    let disposition =
        attempt.record_sanitized_failure(ProviderEgressFailureEvidence::ClearanceChallenge)?;
    assert_eq!(disposition.owner(), ProviderEgressFailureOwner::Clearance);
    assert_eq!(
        disposition.action(),
        ProviderEgressRecoveryAction::RefreshExactClearance
    );
    assert_eq!(
        first
            .runtime
            .clearance_state_at(&first.clearance_key, NOW_MS)?,
        ProviderClearanceRuntimeState::RefreshRequired
    );
    assert_eq!(
        first
            .runtime
            .clearance_state_at(&sibling_clearance, NOW_MS)?,
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 20_000
        }
    );
    assert_eq!(
        attempt.begin_clearance_refresh(NOW_MS + 1_000),
        Err(GrokWebProviderEgressAttemptError::Budget(
            ProviderTransportAttemptBudgetError::InferenceAlreadySubmitted
        ))
    );
    assert_eq!(
        attempt.record_inference_submission(),
        Err(GrokWebProviderEgressAttemptError::ClearanceUnavailable)
    );
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)]
fn direct_wrong_namespace_kind_revision_and_foreign_keys_fail_closed() -> TestResult {
    let fixture = WebFixture::new("identity", ProviderClearanceRuntimeState::Absent)?;
    assert_eq!(
        GrokWebProviderEgressAttempt::try_new(
            Arc::clone(&fixture.runtime),
            fixture.identity.clone(),
            ProviderEgressTargetIdentity::Direct,
            &fixture.lease,
            fixture.session_key.clone(),
            fixture.clearance_key.clone(),
            Arc::new(FixedClock(NOW_MS)),
        )
        .err(),
        Some(GrokWebProviderEgressAttemptError::StickyTargetRequired)
    );

    let foreign_identity = ProviderChannelIdentity::try_new(
        ProviderId::try_new("grok.console")?,
        fixture.identity.upstream_id().clone(),
        fixture.identity.endpoint_id().clone(),
    )?;
    let foreign_runtime = runtime(foreign_identity.clone(), ProviderEgressChannel::GrokWeb)?;
    assert_eq!(
        GrokWebProviderEgressAttempt::try_new(
            foreign_runtime,
            foreign_identity,
            fixture.target.clone(),
            &fixture.lease,
            fixture.session_key.clone(),
            fixture.clearance_key.clone(),
            Arc::new(FixedClock(NOW_MS)),
        )
        .err(),
        Some(GrokWebProviderEgressAttemptError::ProviderMismatch)
    );

    let wrong_channel_runtime =
        runtime(fixture.identity.clone(), ProviderEgressChannel::GrokBuild)?;
    assert_eq!(
        GrokWebProviderEgressAttempt::try_new(
            wrong_channel_runtime,
            fixture.identity.clone(),
            fixture.target.clone(),
            &fixture.lease,
            fixture.session_key.clone(),
            fixture.clearance_key.clone(),
            Arc::new(FixedClock(NOW_MS)),
        )
        .err(),
        Some(GrokWebProviderEgressAttemptError::ChannelMismatch)
    );

    let wrong_kind_pool = pool(
        fixture.identity.endpoint_id().clone(),
        CredentialId::try_new("identity-wrong-kind")?,
        "grok_build_oauth",
        7,
    )?;
    let wrong_kind = wrong_kind_pool.try_lease().ok_or("wrong kind lease")?;
    assert_eq!(
        GrokWebProviderEgressAttempt::try_new(
            Arc::clone(&fixture.runtime),
            fixture.identity.clone(),
            fixture.target.clone(),
            &wrong_kind,
            fixture.session_key.clone(),
            fixture.clearance_key.clone(),
            Arc::new(FixedClock(NOW_MS)),
        )
        .err(),
        Some(GrokWebProviderEgressAttemptError::CredentialMismatch)
    );

    let foreign_endpoint_pool = pool(
        EndpointId::try_new("identity-foreign-endpoint")?,
        fixture.lease.credential_id().clone(),
        "grok_web_sso",
        i64::try_from(fixture.lease.credential_revision())?,
    )?;
    let foreign_endpoint_lease = foreign_endpoint_pool
        .try_lease()
        .ok_or("foreign Endpoint lease unavailable")?;
    assert_eq!(
        GrokWebProviderEgressAttempt::try_new(
            Arc::clone(&fixture.runtime),
            fixture.identity.clone(),
            fixture.target.clone(),
            &foreign_endpoint_lease,
            fixture.session_key.clone(),
            fixture.clearance_key.clone(),
            Arc::new(FixedClock(NOW_MS)),
        )
        .err(),
        Some(GrokWebProviderEgressAttemptError::EndpointMismatch)
    );

    let wrong_revision_session = ProviderSessionStateKey::try_new(
        fixture.identity.clone(),
        fixture.lease.credential_id().clone(),
        fixture.lease.credential_revision() + 1,
        7,
    )?;
    let wrong_revision_clearance = ProviderClearanceStateKey::try_new(
        wrong_revision_session.clone(),
        fixture.target.clone(),
        7,
    )?;
    assert_eq!(
        GrokWebProviderEgressAttempt::try_new(
            Arc::clone(&fixture.runtime),
            fixture.identity.clone(),
            fixture.target.clone(),
            &fixture.lease,
            wrong_revision_session,
            wrong_revision_clearance,
            Arc::new(FixedClock(NOW_MS)),
        )
        .err(),
        Some(GrokWebProviderEgressAttemptError::SessionKeyMismatch)
    );

    let foreign_target = ProviderEgressTargetIdentity::named("identity-foreign-target")?;
    let foreign_clearance = ProviderClearanceStateKey::try_new(
        fixture.session_key.clone(),
        foreign_target,
        fixture.clearance_key.clearance_revision(),
    )?;
    assert_eq!(
        GrokWebProviderEgressAttempt::try_new(
            Arc::clone(&fixture.runtime),
            fixture.identity.clone(),
            fixture.target.clone(),
            &fixture.lease,
            fixture.session_key.clone(),
            foreign_clearance,
            Arc::new(FixedClock(NOW_MS)),
        )
        .err(),
        Some(GrokWebProviderEgressAttemptError::ClearanceKeyMismatch)
    );
    Ok(())
}

#[test]
fn blocked_egress_inactive_session_and_invalid_clearance_stop_before_accounting() -> TestResult {
    let egress = WebFixture::new("blocked-egress", ProviderClearanceRuntimeState::Absent)?;
    let egress_attempt = egress.attempt()?;
    egress.runtime.set_egress_state(
        ProviderEgressStateKey::new(egress.identity.clone(), egress.target.clone()),
        ProviderEgressRuntimeState::Disabled,
        NOW_MS,
    )?;
    assert_eq!(
        egress_attempt.record_statsig_environment_request(),
        Err(GrokWebProviderEgressAttemptError::Runtime(
            gateway_router::ProviderEgressRuntimeError::EgressUnavailable
        ))
    );
    assert_eq!(egress_attempt.snapshot()?.auxiliary_requests(), 0);

    let session = WebFixture::new("expired-session", ProviderClearanceRuntimeState::Absent)?;
    let session_attempt = session.attempt()?;
    session.runtime.set_session_state(
        session.session_key.clone(),
        ProviderSessionRuntimeState::Expired,
        NOW_MS,
    )?;
    assert_eq!(
        session_attempt.record_statsig_signer_request(),
        Err(GrokWebProviderEgressAttemptError::SessionUnavailable)
    );
    assert_eq!(session_attempt.snapshot()?.auxiliary_requests(), 0);

    let clearance = WebFixture::new(
        "invalid-clearance",
        ProviderClearanceRuntimeState::Fresh {
            expires_at_ms: NOW_MS + 10_000,
        },
    )?;
    let clearance_attempt = clearance.attempt()?;
    clearance.runtime.set_clearance_state(
        clearance.clearance_key.clone(),
        ProviderClearanceRuntimeState::Invalid,
        NOW_MS,
    )?;
    assert_eq!(
        clearance_attempt.record_statsig_environment_request(),
        Err(GrokWebProviderEgressAttemptError::ClearanceUnavailable)
    );
    assert_eq!(clearance_attempt.snapshot()?.auxiliary_requests(), 0);
    Ok(())
}

#[test]
fn receipts_and_debug_are_value_free_and_transport_free_api_starts_zero() -> TestResult {
    let fixture = WebFixture::new("redaction", ProviderClearanceRuntimeState::Absent)?;
    let attempt = fixture.attempt()?;
    let rendered = format!("{attempt:?} {:?}", attempt.snapshot()?);
    assert!(!rendered.contains(std::str::from_utf8(SECRET_SENTINEL)?));
    assert!(!rendered.contains("redaction-sticky-target"));
    assert!(rendered.contains("<exact named target>"));
    assert!(rendered.contains("<value-free>"));
    // No transport is supplied to this state/accounting-only type; its fresh receipt starts at
    // zero until the eventual adapter explicitly records a hidden request.
    assert_eq!(attempt.snapshot()?.auxiliary_requests(), 0);
    assert!(!attempt.snapshot()?.inference_submitted());
    Ok(())
}

struct WebFixture {
    runtime: Arc<ProviderEgressRuntime>,
    identity: ProviderChannelIdentity,
    target: ProviderEgressTargetIdentity,
    session_key: ProviderSessionStateKey,
    clearance_key: ProviderClearanceStateKey,
    _pool: EndpointCredentialPool,
    lease: CredentialLease,
}

impl WebFixture {
    fn new(
        label: &str,
        clearance_state: ProviderClearanceRuntimeState,
    ) -> Result<Self, Box<dyn Error>> {
        let endpoint = EndpointId::try_new(format!("{label}-endpoint"))?;
        let upstream = UpstreamId::try_new(format!("{label}-upstream"))?;
        let identity = ProviderChannelIdentity::try_new(
            ProviderId::try_new("grok.web")?,
            upstream,
            endpoint.clone(),
        )?;
        let runtime = runtime(identity.clone(), ProviderEgressChannel::GrokWeb)?;
        let target = ProviderEgressTargetIdentity::named(format!("{label}-sticky-target"))?;
        runtime.set_egress_state(
            ProviderEgressStateKey::new(identity.clone(), target.clone()),
            ProviderEgressRuntimeState::Available,
            NOW_MS,
        )?;
        let credential_id = CredentialId::try_new(format!("{label}-account"))?;
        let session_key =
            ProviderSessionStateKey::try_new(identity.clone(), credential_id.clone(), 7, 9)?;
        runtime.set_session_state(
            session_key.clone(),
            ProviderSessionRuntimeState::Active {
                expires_at_ms: NOW_MS + 20_000,
            },
            NOW_MS,
        )?;
        let clearance_key =
            ProviderClearanceStateKey::try_new(session_key.clone(), target.clone(), 11)?;
        runtime.set_clearance_state(clearance_key.clone(), clearance_state, NOW_MS)?;
        let pool = pool(endpoint, credential_id, "grok_web_sso", 7)?;
        let lease = pool.try_lease().ok_or("Web exact lease unavailable")?;
        Ok(Self {
            runtime,
            identity,
            target,
            session_key,
            clearance_key,
            _pool: pool,
            lease,
        })
    }

    fn attempt(&self) -> Result<GrokWebProviderEgressAttempt, GrokWebProviderEgressAttemptError> {
        self.attempt_with_clock(Arc::new(FixedClock(NOW_MS)))
    }

    fn attempt_with_clock(
        &self,
        clock: Arc<dyn GrokWebProviderEgressClock>,
    ) -> Result<GrokWebProviderEgressAttempt, GrokWebProviderEgressAttemptError> {
        GrokWebProviderEgressAttempt::try_new(
            Arc::clone(&self.runtime),
            self.identity.clone(),
            self.target.clone(),
            &self.lease,
            self.session_key.clone(),
            self.clearance_key.clone(),
            clock,
        )
    }
}

fn runtime(
    identity: ProviderChannelIdentity,
    channel: ProviderEgressChannel,
) -> Result<Arc<ProviderEgressRuntime>, Box<dyn Error>> {
    let registry =
        ProviderChannelCapabilityRegistry::try_new(vec![ProviderChannelCapability::new(
            identity, channel,
        )])?;
    Ok(Arc::new(ProviderEgressRuntime::new(registry)))
}

fn pool(
    endpoint_id: EndpointId,
    credential_id: CredentialId,
    kind: &str,
    revision: i64,
) -> Result<EndpointCredentialPool, Box<dyn Error>> {
    Ok(EndpointCredentialPool::try_new(
        endpoint_id,
        [EndpointCredentialInput {
            credential_id,
            credential_kind: kind.to_owned(),
            credential_revision: revision,
            priority: 0,
            weight: 1,
            concurrency: 1,
            expires_at_ms: None,
            secret: CredentialSecret::try_new(SECRET_SENTINEL.to_vec())?,
        }],
    )?)
}
