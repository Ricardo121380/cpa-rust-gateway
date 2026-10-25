//! Opt-in operator support for the stock-serve recovery driver; never an inference server.
//! Jobs and diagnostic plaintext remain in the driver's private synthetic directory.
use gateway_core::{EventEmission, GatewayEventSink};
use gateway_observability::{BoundedEventQueue, EventQueueConfig};
use gateway_store::{
    backup::{
        BackupKey, create_encrypted_backup_file, restore_encrypted_backup_reader_to_empty_target,
    },
    event_store::{AsyncSqliteEventWriter, EventWriterConfig},
    messages_summary::MessagesSummaryRecord,
    native_usage::NativeUsageRecord,
    request_metadata::RequestMetadataRecord,
    secret_store::{KeyVersion, MasterKeyRing, SecretStore},
};
use serde::Deserialize;
use std::{error::Error, fs, path::PathBuf, time::Duration};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    scope: String,
    action: String,
    database: PathBuf,
    keys: PathBuf,
    active: u32,
    input: PathBuf,
    output: PathBuf,
    expected_persisted: bool,
}

#[tokio::test]
#[ignore = "operator helper; invoked only by the isolated #65 real serve process driver"]
async fn cpar65_operator_job() -> Result<(), Box<dyn Error>> {
    let job: Job = serde_json::from_slice(&fs::read(std::env::var("CPAR65_OPERATOR_JOB")?)?)?;
    assert_eq!(job.scope, "isolated_synthetic_cpar65");
    match job.action.as_str() {
        "encrypted_backup" => {
            let key = BackupKey::try_from_bytes(fs::read(&job.input)?)?;
            create_encrypted_backup_file(
                &gateway_store::open(&job.database)?,
                job.output.parent().ok_or("missing staging directory")?,
                &job.output,
                &key,
            )?;
        }
        "restore_encrypted_backup" => {
            let key = BackupKey::try_from_bytes(fs::read(&job.input)?)?;
            restore_encrypted_backup_reader_to_empty_target(
                &mut fs::File::open(&job.database)?,
                &job.output,
                &key,
            )?;
        }
        "replay_summary" | "replay_usage" | "replay_metadata" => replay(&job).await?,
        _ => return Err("unsupported operator action".into()),
    }
    Ok(())
}

async fn replay(job: &Job) -> Result<(), Box<dyn Error>> {
    let secrets = SecretStore::new(MasterKeyRing::load_from_directory(
        &job.keys,
        KeyVersion::try_new(job.active)?,
    )?);
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(8, 1)?)?;
    let writer = tokio::spawn(
        AsyncSqliteEventWriter::new(
            &job.database,
            receiver,
            EventWriterConfig::try_new(4, Duration::from_millis(5))?,
        )
        .with_native_usage_secret_store(secrets)
        .run(),
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while queue.recording_health().snapshot().0 != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    let private: serde_json::Value = serde_json::from_slice(&fs::read(&job.input)?)?;
    let result = if job.action == "replay_summary" {
        let record: MessagesSummaryRecord = serde_json::from_value(private["record"].clone())?;
        queue
            .emit_summary_evidence_confirmed(
                record.attempt.with_execution_identity(record.execution),
                record.evidence,
            )
            .await
    } else if job.action == "replay_metadata" {
        let record: RequestMetadataRecord =
            serde_json::from_value(private["record"]["record"].clone())?;
        queue
            .emit_request_metadata_confirmed(record.request, record.evidence)
            .await
    } else {
        let record: NativeUsageRecord = serde_json::from_value(private["record"].clone())?;
        queue
            .emit_usage_evidence_confirmed(record.usage, record.evidence)
            .await
    };
    let expected = if job.expected_persisted {
        EventEmission::Persisted
    } else {
        EventEmission::PersistenceUnavailable
    };
    // Compare only the value-free receipt; never format a private payload in a failed assert.
    assert_eq!(result, expected);
    drop(queue);
    let metrics = writer.await?;
    assert_eq!(
        metrics.required_events_quarantined,
        u64::from(!job.expected_persisted)
    );
    Ok(())
}
