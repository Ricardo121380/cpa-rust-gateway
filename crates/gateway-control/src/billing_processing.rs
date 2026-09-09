//! Value-free in-memory billing worker status; persistence remains owned by the stores.
use gateway_store::billing_ledger::BillingMaterializationProgress;
use std::sync::Mutex;

/// Closed lifecycle state of the process-local billing owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BillingProcessingPhase {
    /// No worker has been attached.
    Disabled,
    /// Worker has started but has no successful observation yet.
    Starting,
    /// Source checkpoint is current and no unresolved failures remain.
    Current,
    /// Durable source remains ahead of the checkpoint.
    CatchingUp,
    /// Source rows require later repair, even if the checkpoint is current.
    NeedsRepair,
    /// Last batch could not safely complete.
    Failed,
    /// Owner has stopped; durable progress remains available for the next process.
    Stopped,
}
impl BillingProcessingPhase {
    /// Returns the closed management wire label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Starting => "starting",
            Self::Current => "current",
            Self::CatchingUp => "catching_up",
            Self::NeedsRepair => "needs_repair",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

/// Safe snapshot containing only watermarks, counts and worker lifecycle.
#[derive(Clone, Debug)]
pub struct BillingProcessingSnapshot {
    /// Current process-local lifecycle.
    pub phase: BillingProcessingPhase,
    /// Last successful progress observation time; retained on failure, never fabricated.
    pub observed_at_ms: Option<u64>,
    /// Last successful consistent durable progress read.
    pub progress: Option<BillingMaterializationProgress>,
}

/// Short-lock shared monitor; HTTP reads never enter `SQLite` or wait for billing work.
#[derive(Debug)]
pub struct BillingProcessingMonitor(Mutex<BillingProcessingSnapshot>);
impl Default for BillingProcessingMonitor {
    fn default() -> Self {
        Self(Mutex::new(BillingProcessingSnapshot {
            phase: BillingProcessingPhase::Disabled,
            observed_at_ms: None,
            progress: None,
        }))
    }
}
impl BillingProcessingMonitor {
    /// Reads the last safe snapshot; poisoned state fails closed.
    #[must_use]
    pub fn snapshot(&self) -> Option<BillingProcessingSnapshot> {
        self.0.lock().ok().map(|state| state.clone())
    }
    /// Marks owner startup without inventing source observations.
    pub fn starting(&self) {
        self.set_phase(BillingProcessingPhase::Starting);
    }
    /// Marks a failed batch while preserving the age of the last successful observation.
    pub fn failed(&self) {
        self.set_phase(BillingProcessingPhase::Failed);
    }
    /// Marks a joined or terminated owner.
    pub fn stopped(&self) {
        self.set_phase(BillingProcessingPhase::Stopped);
    }
    /// Publishes one consistent source/ledger observation after blocking work has ended.
    pub fn observe(&self, progress: BillingMaterializationProgress, observed_at_ms: u64) {
        if let Ok(mut state) = self.0.lock() {
            let phase = if progress.unresolved_failures > 0 {
                BillingProcessingPhase::NeedsRepair
            } else if progress.checkpoint_ordinal.unwrap_or(0) < progress.source_ordinal {
                BillingProcessingPhase::CatchingUp
            } else {
                BillingProcessingPhase::Current
            };
            *state = BillingProcessingSnapshot {
                phase,
                observed_at_ms: Some(observed_at_ms),
                progress: Some(progress),
            };
        }
    }
    fn set_phase(&self, phase: BillingProcessingPhase) {
        if let Ok(mut state) = self.0.lock() {
            state.phase = phase;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phase_preserves_unknowns_and_keeps_last_observation_on_failure() -> Result<(), &'static str>
    {
        let monitor = BillingProcessingMonitor::default();
        assert_eq!(
            monitor.snapshot().ok_or("missing")?.phase,
            BillingProcessingPhase::Disabled
        );
        monitor.starting();
        assert!(monitor.snapshot().ok_or("missing")?.progress.is_none());
        let mut progress = BillingMaterializationProgress {
            source_ordinal: 3,
            checkpoint_ordinal: Some(2),
            checkpoint_updated_at_ms: Some(100),
            unresolved_failures: 0,
        };
        monitor.observe(progress.clone(), 100);
        assert_eq!(
            monitor.snapshot().ok_or("missing")?.phase,
            BillingProcessingPhase::CatchingUp
        );
        progress.checkpoint_ordinal = Some(3);
        progress.unresolved_failures = 1;
        monitor.observe(progress.clone(), 200);
        assert_eq!(
            monitor.snapshot().ok_or("missing")?.phase,
            BillingProcessingPhase::NeedsRepair
        );
        monitor.failed();
        assert_eq!(
            monitor.snapshot().ok_or("missing")?.observed_at_ms,
            Some(200)
        );
        progress.unresolved_failures = 0;
        monitor.observe(progress, 300);
        assert_eq!(
            monitor.snapshot().ok_or("missing")?.phase,
            BillingProcessingPhase::Current
        );
        monitor.stopped();
        assert_eq!(
            monitor.snapshot().ok_or("missing")?.phase,
            BillingProcessingPhase::Stopped
        );
        Ok(())
    }
}
