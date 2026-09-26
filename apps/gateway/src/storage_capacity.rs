//! Minute-scale local filesystem sampling on one dedicated thread, separate from TTL cleanup.
use gateway_observability::StorageCapacitySnapshot;
use std::{
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) struct CapacityWorker {
    stop: std::sync::mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl CapacityWorker {
    pub(crate) fn start(
        database: std::path::PathBuf,
        monitor: std::sync::Arc<gateway_observability::StorageCapacityMonitor>,
    ) -> std::io::Result<Self> {
        let (stop, receiver) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("storage-capacity".into())
            .spawn(move || {
                loop {
                    monitor.observe(sample(&database));
                    match receiver.recv_timeout(Duration::from_mins(1)) {
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        _ => break,
                    }
                }
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
    pub(crate) async fn stop(mut self) -> Result<(), ()> {
        let _ = self.stop.send(());
        let thread = self.thread.take().ok_or(())?;
        let deadline = Instant::now() + Duration::from_secs(3);
        while !thread.is_finished() {
            if Instant::now() >= deadline {
                return Err(());
            }
            actix_web::rt::time::sleep(Duration::from_millis(10)).await;
        }
        thread.join().map_err(|_| ())
    }
}
impl Drop for CapacityWorker {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

pub(crate) fn sample(database: &Path) -> Option<StorageCapacitySnapshot> {
    let database_bytes = std::fs::metadata(database).ok()?.len();
    let mut wal = database.as_os_str().to_owned();
    wal.push("-wal");
    let wal_bytes = match std::fs::metadata(wal) {
        Ok(value) => value.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(_) => return None,
    };
    // Safe native filesystem query. Avoid spawning df: Rust's process implementation on the
    // ARM build host links newer glibc symbols than the supported Debian runtime provides.
    let space = rustix::fs::statvfs(database).ok()?;
    let total_bytes = space.f_blocks.checked_mul(space.f_frsize)?;
    let available_bytes = space.f_bavail.checked_mul(space.f_frsize)?;
    if total_bytes == 0 {
        return None;
    }
    Some(StorageCapacitySnapshot {
        observed_at_ms: u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_millis(),
        )
        .ok()?,
        database_bytes,
        wal_bytes,
        available_bytes,
        total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[actix_web::test]
    async fn sampler_publishes_and_stops_without_waiting_for_interval()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("cpar-sampler-{}.sqlite", std::process::id()));
        std::fs::write(&path, b"synthetic")?;
        let monitor = std::sync::Arc::new(gateway_observability::StorageCapacityMonitor::default());
        let worker = CapacityWorker::start(path.clone(), monitor.clone())?;
        actix_web::rt::time::timeout(Duration::from_secs(5), async {
            while monitor.snapshot().0.is_none() {
                actix_web::rt::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await?;
        worker.stop().await.map_err(|()| "sampler stop failed")?;
        std::fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn local_capacity_reads_only_file_metadata_and_keeps_missing_files_unknown()
    -> Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("cpar-capacity-{}.sqlite", std::process::id()));
        assert!(sample(&path).is_none());
        std::fs::write(&path, b"synthetic")?;
        let observed = sample(&path).ok_or("capacity sample")?;
        assert_eq!(observed.database_bytes, 9);
        assert_eq!(observed.wal_bytes, 0);
        assert!(observed.observed_at_ms > 0 && observed.total_bytes > 0);
        std::fs::remove_file(path)?;
        Ok(())
    }
}
