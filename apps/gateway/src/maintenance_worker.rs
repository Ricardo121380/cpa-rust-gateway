//! Bounded owner for encrypted response, compaction and native Usage TTL, without decryption.
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;

pub(crate) struct MaintenanceWorker {
    stop: watch::Sender<bool>,
    task: actix_web::rt::task::JoinHandle<()>,
}
impl MaintenanceWorker {
    pub(crate) fn start(database: PathBuf) -> Self {
        let (stop, mut stopping) = watch::channel(false);
        let task = actix_web::rt::spawn(async move {
            loop {
                if *stopping.borrow() {
                    break;
                }
                let database = database.clone();
                let result = actix_web::rt::task::spawn_blocking(move || {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(|_| ())?;
                    let now_ms = i64::try_from(now.as_millis()).map_err(|_| ())?;
                    let (responses, compactions) =
                        gateway_store::stored_response::maintain_expired_stored_data(
                            &database, now_ms, 256,
                        )
                        .map_err(|_| ())?;
                    let connection = gateway_store::open(database).map_err(|_| ())?;
                    let native_usage =
                        gateway_store::native_usage::purge_expired(&connection, now_ms)
                            .map_err(|_| ())?;
                    let request_metadata =
                        gateway_store::request_metadata::purge_expired(&connection, now_ms)
                            .map_err(|_| ())?;
                    Ok::<_, ()>((responses, compactions, native_usage, request_metadata))
                })
                .await;
                if let Ok(Ok((responses, compactions, native_usage, request_metadata))) = result {
                    if responses + compactions + native_usage + request_metadata > 0 {
                        tracing::info!(target: "stored_data_maintenance", responses, compactions, native_usage, request_metadata, "expired stored data removed");
                    }
                } else {
                    tracing::warn!(target: "stored_data_maintenance", "stored data maintenance unavailable");
                }
                tokio::select! {
                    () = actix_web::rt::time::sleep(Duration::from_mins(1)) => {},
                    _ = stopping.changed() => {},
                }
            }
        });
        Self { stop, task }
    }
    pub(crate) async fn stop(mut self) -> Result<(), ()> {
        let _ = self.stop.send(true);
        match actix_web::rt::time::timeout(Duration::from_secs(30), &mut self.task).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(()),
            Err(_) => {
                self.task.abort();
                Err(())
            }
        }
    }
}
impl Drop for MaintenanceWorker {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

#[cfg(test)]
mod tests {
    use super::MaintenanceWorker;
    use std::{error::Error, time::Duration};

    #[actix_web::test]
    async fn worker_cleans_expired_rows_and_stops_before_next_batch() -> Result<(), Box<dyn Error>>
    {
        let path =
            std::env::temp_dir().join(format!("prism-maintenance-{}.sqlite", std::process::id()));
        let mut connection = gateway_store::open(&path)?;
        gateway_store::migrate(&mut connection)?;
        // GC is index-only: even unreadable old payloads must not block expiry cleanup.
        connection.execute(
            "INSERT INTO stored_responses VALUES ('owner', 'expired', 0, 1, 1, 1, zeroblob(41))",
            [],
        )?;
        connection.execute("INSERT INTO stored_responses VALUES ('owner', 'live', 0, 9223372036854775807, 1, 1, zeroblob(41))", [])?;
        connection.execute("INSERT INTO stored_response_compactions VALUES ('owner', 'expired', 0, 1, 1, 1, zeroblob(41))", [])?;
        connection.execute("INSERT INTO native_usage_evidence VALUES ('expired','request','owner','source-expired',0,1,1,1,zeroblob(41))",[])?;
        connection.execute("INSERT INTO native_usage_evidence VALUES ('live','request','owner','source-live',0,9223372036854775807,1,1,zeroblob(41))",[])?;
        let worker = MaintenanceWorker::start(path.clone());
        actix_web::rt::time::timeout(Duration::from_secs(5), async {
            loop {
                let expired: i64 = connection.query_row("SELECT (SELECT count(*) FROM stored_responses WHERE expires_at_ms = 1) + (SELECT count(*) FROM stored_response_compactions) + (SELECT count(*) FROM native_usage_evidence WHERE expires_at_ms=1)", [], |row| row.get(0))?;
                if expired == 0 { return Ok::<(), Box<dyn Error>>(()); }
                actix_web::rt::time::sleep(Duration::from_millis(10)).await;
            }
        }).await??;
        worker
            .stop()
            .await
            .map_err(|()| "maintenance did not stop")?;
        let live: i64 = connection.query_row(
            "SELECT count(*) FROM stored_responses WHERE response_id = 'live'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(live, 1);
        assert_eq!(
            connection.query_row(
                "SELECT count(*) FROM native_usage_evidence WHERE attempt_id='live'",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            1
        );
        drop(connection);
        std::fs::remove_file(path)?;
        Ok(())
    }
}
