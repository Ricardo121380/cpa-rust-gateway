//! Single-owner billing consumption, isolated from listener threads and Provider execution.
use gateway_control::billing_processing::BillingProcessingMonitor;
use gateway_store::billing_ledger::BillingMaterializationProgress;
use std::sync::Arc;

use gateway_control::billing_materializer::{
    BILLING_MATERIALIZER_ID, BillingMaterializationReceipt,
    materialize_billing_events_with_retention,
};
use gateway_store::{billing_ledger::SqliteBillingLedger, event_store::SqliteEventStore};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;

const BATCH_SIZE: usize = 256;
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const STOP_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct BillingWorker {
    monitor: Arc<BillingProcessingMonitor>,
    stop: watch::Sender<bool>,
    task: actix_web::rt::task::JoinHandle<()>,
}

impl BillingWorker {
    pub(crate) fn start(database: PathBuf, monitor: Arc<BillingProcessingMonitor>) -> Self {
        monitor.starting();
        let owner_monitor = Arc::clone(&monitor);
        let (stop, mut stopping) = watch::channel(false);
        let task = actix_web::rt::spawn(async move {
            loop {
                let final_batch = *stopping.borrow();
                let database = database.clone();
                let result =
                    actix_web::rt::task::spawn_blocking(move || run_batch(&database)).await;
                let full_batch = if let Ok(Ok((receipt, progress, observed_at_ms))) = result {
                    monitor.observe(progress, observed_at_ms);
                    if receipt.failed_events > 0 {
                        tracing::warn!(target: "billing_materializer", failed_events = receipt.failed_events,
                            "billing source rows retained for retry");
                    }
                    receipt.scanned_events == BATCH_SIZE
                } else {
                    monitor.failed();
                    tracing::warn!(target: "billing_materializer", "billing batch unavailable; checkpoint retained");
                    false
                };
                if final_batch {
                    break;
                }
                if full_batch {
                    // Yield between finite batches; stop is sampled before the next dispatch.
                    actix_web::rt::task::yield_now().await;
                    continue;
                }
                tokio::select! {
                    () = actix_web::rt::time::sleep(POLL_INTERVAL) => {},
                    _ = stopping.changed() => {},
                }
            }
            monitor.stopped();
        });
        Self {
            stop,
            task,
            monitor: owner_monitor,
        }
    }

    pub(crate) async fn stop(mut self) -> Result<(), ()> {
        let _ = self.stop.send(true);
        match actix_web::rt::time::timeout(STOP_TIMEOUT, &mut self.task).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => {
                self.monitor.failed();
                Err(())
            }
            Err(_) => {
                // A blocked SQLite call may finish, but no new batch is dispatched after abort.
                self.task.abort();
                self.monitor.failed();
                Err(())
            }
        }
    }
}

impl Drop for BillingWorker {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

fn run_batch(
    database: &std::path::Path,
) -> Result<
    (
        BillingMaterializationReceipt,
        BillingMaterializationProgress,
        u64,
    ),
    (),
> {
    let source = SqliteEventStore::open_read_only(database).map_err(|_| ())?;
    let mut ledger = SqliteBillingLedger::open(database).map_err(|_| ())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ())?;
    let now_ms = u64::try_from(now.as_millis()).map_err(|_| ())?;
    let receipt = materialize_billing_events_with_retention(
        &source,
        &mut ledger,
        BILLING_MATERIALIZER_ID,
        BATCH_SIZE,
        None,
        now_ms,
    )
    .map_err(|_| ())?;
    let progress = ledger
        .materialization_progress(BILLING_MATERIALIZER_ID)
        .map_err(|_| ())?;
    let observed_at_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ())?
            .as_millis(),
    )
    .map_err(|_| ())?;
    Ok((receipt, progress, observed_at_ms))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_core::{
        AttemptEvent, AttemptOutcome, AttemptRetryDecision, ClientKeyId, CredentialId, EndpointId,
        GatewayEvent, GatewayProtocol, RequestEvent, RequestId, ResponseId, RouteCandidateId,
        RouteId, UpstreamId, Usage, UsageEvent,
    };
    use std::error::Error;

    fn append_request(database: &std::path::Path, sequence: u32) -> Result<(), Box<dyn Error>> {
        let mut source = SqliteEventStore::open(database)?;
        let id = RequestId::try_new(format!("worker-request-{sequence}"))?;
        source.append_batch(&[
            GatewayEvent::Request(RequestEvent::new(
                id.clone(),
                ClientKeyId::try_new("key-test")?,
                None,
                GatewayProtocol::OpenAiResponses,
                "exact-model".to_owned(),
                "public-model".to_owned(),
                None,
                false,
            )),
            GatewayEvent::Attempt(AttemptEvent::new(
                id.clone(),
                1,
                RouteId::try_new("route-test")?,
                RouteCandidateId::try_new("candidate-test")?,
                CredentialId::try_new("account-test")?,
                EndpointId::try_new("endpoint-test")?,
                UpstreamId::try_new("provider-test")?,
                "exact-model".to_owned(),
                900,
                1000,
                AttemptOutcome::Succeeded,
                AttemptRetryDecision::Completed,
            )),
            GatewayEvent::Usage(UsageEvent::from_usage(
                id,
                ResponseId::try_new(format!("response-{sequence}"))?,
                &Usage {
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                    ..Usage::default()
                },
            )),
        ])?;
        Ok(())
    }

    #[actix_web::test]
    async fn worker_materializes_stops_and_resumes_without_duplicate_billing()
    -> Result<(), Box<dyn Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let database = std::env::temp_dir().join(format!(
            "prism-billing-worker-{}-{nonce}.sqlite",
            std::process::id()
        ));
        append_request(&database, 1)?;
        let worker = BillingWorker::start(
            database.clone(),
            Arc::new(BillingProcessingMonitor::default()),
        );
        actix_web::rt::time::timeout(Duration::from_secs(5), async {
            loop {
                let ledger = SqliteBillingLedger::open_read_only(&database)?;
                if !ledger.list_bounded(1)?.is_empty() {
                    break;
                }
                actix_web::rt::time::sleep(Duration::from_millis(20)).await;
            }
            Ok::<(), gateway_store::StoreError>(())
        })
        .await??;
        worker.stop().await.map_err(|()| "worker stop failed")?;
        let ledger = SqliteBillingLedger::open_read_only(&database)?;
        let rows = ledger.list_bounded(10)?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].retention_expires_at_ms, i64::MAX as u64);
        assert_eq!(
            rows[0].cost_confidence,
            gateway_store::billing_ledger::BillingCostConfidence::Unpriced
        );
        assert_eq!(
            ledger
                .load_checkpoint(BILLING_MATERIALIZER_ID)?
                .ok_or("missing checkpoint")?
                .event_ordinal,
            3
        );
        drop(ledger);
        append_request(&database, 2)?;
        let worker = BillingWorker::start(
            database.clone(),
            Arc::new(BillingProcessingMonitor::default()),
        );
        worker.stop().await.map_err(|()| "restart stop failed")?;
        let ledger = SqliteBillingLedger::open_read_only(&database)?;
        assert_eq!(ledger.list_bounded(10)?.len(), 2);
        assert_eq!(
            ledger
                .load_checkpoint(BILLING_MATERIALIZER_ID)?
                .ok_or("missing checkpoint")?
                .event_ordinal,
            6
        );
        drop(ledger);
        std::fs::remove_file(database)?;
        Ok(())
    }
}
