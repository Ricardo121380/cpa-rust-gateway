//! Value-free storage observations published outside listener threads.
use std::sync::Mutex;

/// One successful storage observation. No paths or device names cross this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageCapacitySnapshot {
    /// Wall time of the successful sample.
    pub observed_at_ms: u64,
    /// `SQLite` main file bytes, excluding WAL and shared-memory files.
    pub database_bytes: u64,
    /// `SQLite` write-ahead log bytes; absent WAL is zero.
    pub wal_bytes: u64,
    /// Filesystem bytes available to the service user.
    pub available_bytes: u64,
    /// Filesystem total bytes, not a per-service allocation.
    pub total_bytes: u64,
}

impl StorageCapacitySnapshot {
    /// Early warning only; actual durable admission is governed by commit confirmation.
    #[must_use]
    pub fn disk_low(self) -> bool {
        self.available_bytes < 1024 * 1024 * 1024 || self.available_bytes < self.total_bytes / 20
    }

    /// Warn about a growing WAL; never performs an automatic checkpoint or deletion.
    #[must_use]
    pub fn wal_high(self) -> bool {
        self.wal_bytes > 256 * 1024 * 1024
    }
}

/// A short-lock snapshot: failure preserves the age and values of the last observation.
#[derive(Default)]
pub struct StorageCapacityMonitor(Mutex<(Option<StorageCapacitySnapshot>, bool)>);

impl StorageCapacityMonitor {
    /// Publishes a completed bounded sample, or records failure without inventing values.
    pub fn observe(&self, result: Option<StorageCapacitySnapshot>) {
        if let Ok(mut state) = self.0.lock() {
            state.1 = result.is_none();
            if result.is_some() {
                state.0 = result;
            }
        }
    }

    /// Reads last observation and collection failure; a poisoned monitor fails closed.
    #[must_use]
    pub fn snapshot(&self) -> (Option<StorageCapacitySnapshot>, bool) {
        self.0.lock().map_or((None, true), |state| *state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_failure_never_turns_unknown_or_stale_values_into_healthy() {
        let monitor = StorageCapacityMonitor::default();
        assert_eq!(monitor.snapshot(), (None, false));
        let normal = StorageCapacitySnapshot {
            observed_at_ms: 100,
            database_bytes: 100,
            wal_bytes: 0,
            available_bytes: 2 * 1024 * 1024 * 1024,
            total_bytes: 10 * 1024 * 1024 * 1024,
        };
        assert!(!normal.disk_low());
        assert!(!normal.wal_high());
        monitor.observe(Some(normal));
        monitor.observe(None);
        assert_eq!(monitor.snapshot(), (Some(normal), true));
        let critical = StorageCapacitySnapshot {
            observed_at_ms: 200,
            available_bytes: 0,
            wal_bytes: 257 * 1024 * 1024,
            ..normal
        };
        assert!(critical.disk_low());
        assert!(critical.wal_high());
        monitor.observe(Some(critical));
        assert_eq!(monitor.snapshot(), (Some(critical), false));
    }
}
