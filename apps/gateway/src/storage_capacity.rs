//! Minute-scale local filesystem sampling on one dedicated thread, separate from TTL cleanup.
use gateway_observability::StorageCapacitySnapshot;
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
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
    // POSIX df is already part of the supported host images. No shell or user command is run.
    // A missing command, stalled filesystem or output/parse failure means unobserved capacity.
    let mut child = Command::new("/bin/df")
        .args(["-Pk"])
        .arg(database)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) => return None,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut output = String::new();
    child
        .stdout
        .take()?
        .take(8193)
        .read_to_string(&mut output)
        .ok()?;
    if output.len() > 8192 {
        return None;
    }
    let (total_bytes, available_bytes) = parse_df(&output)?;
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

fn parse_df(text: &str) -> Option<(u64, u64)> {
    let row: Vec<_> = text.lines().nth(1)?.split_whitespace().collect();
    // Locate the percent field: filesystem and mount point may contain spaces.
    let capacity = row.iter().position(|field| {
        field
            .strip_suffix('%')
            .is_some_and(|value| value.parse::<u64>().is_ok())
    })?;
    let total = row
        .get(capacity.checked_sub(3)?)?
        .parse::<u64>()
        .ok()?
        .checked_mul(1024)?;
    // On a full reserved filesystem df can report negative availability. It means no allocatable bytes.
    let available = row
        .get(capacity.checked_sub(1)?)?
        .parse::<i64>()
        .ok()?
        .max(0);
    if total == 0 {
        return None;
    }
    Some((total, u64::try_from(available).ok()?.checked_mul(1024)?))
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
    fn parse_capacity_never_invents_space_on_full_or_invalid_filesystems() {
        assert_eq!(
            parse_df(
                "Filesystem 1024-blocks Used Available Capacity Mounted\n/dev/test 1000 800 200 80% /\n"
            ),
            Some((1_024_000, 204_800))
        );
        assert_eq!(
            parse_df("header\n/dev/test 1000 1001 -1 100% /\n"),
            Some((1_024_000, 0))
        );
        assert_eq!(
            parse_df("header\nvolume with spaces 1000 800 200 80% /mount with spaces\n"),
            Some((1_024_000, 204_800))
        );
        assert_eq!(parse_df("header\nbroken"), None);
        assert_eq!(parse_df("header\n/dev/test 0 0 0 0% /\n"), None);
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
