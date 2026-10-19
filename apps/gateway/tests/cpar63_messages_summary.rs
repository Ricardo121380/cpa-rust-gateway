//! Synthetic storage lifecycle and an actual separate authorized reader process.
use gateway_core::*;
use gateway_observability::{BoundedEventQueue, EventQueueConfig};
use gateway_store::backup::{
    BackupKey, create_encrypted_backup, restore_encrypted_backup_to_empty_target,
};
use gateway_store::{
    event_store::{AsyncSqliteEventWriter, EventWriterConfig, SqliteEventStore},
    messages_summary::{MessagesSummaryRecord, SqliteMessagesSummaryReader},
    secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
};
use serde_json::{Value, json};
use std::{
    error::Error,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
type TestResult = Result<(), Box<dyn Error>>;
struct Directory(PathBuf);
impl Directory {
    fn new() -> Result<Self, Box<dyn Error>> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "cpar63-{}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        Ok(Self(path.canonicalize()?))
    }
    fn db(&self) -> PathBuf {
        self.0.join("events.sqlite3")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn keys(active: u32, versions: &[u32], wrong: bool) -> Result<SecretStore, Box<dyn Error>> {
    Ok(SecretStore::new(MasterKeyRing::try_new(
        KeyVersion::try_new(active)?,
        versions
            .iter()
            .map(|v| {
                Ok((
                    KeyVersion::try_new(*v)?,
                    MasterKey::try_from_bytes([if wrong { 0xEE } else { u8::try_from(*v)? }; 32])?,
                ))
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?,
    )?))
}
fn events(label: &str, owner: &str) -> Result<(RequestEvent, AttemptEvent), Box<dyn Error>> {
    let id = RequestId::try_new(format!("request-{label}"))?;
    let request = RequestEvent::new(
        id.clone(),
        ClientKeyId::try_new(owner)?,
        None,
        GatewayProtocol::AnthropicMessages,
        "grok-4.5".into(),
        "grok-4.5".into(),
        None,
        false,
    )
    .with_started_at_ms(1000);
    let attempt = AttemptEvent::new(
        id,
        1,
        RouteId::try_new("route")?,
        RouteCandidateId::try_new("candidate")?,
        CredentialId::try_new("synthetic-account")?,
        EndpointId::try_new("build")?,
        UpstreamId::try_new("grok")?,
        "grok-4.5".into(),
        1000,
        1001,
        AttemptOutcome::Succeeded,
        AttemptRetryDecision::Completed,
    )
    .with_execution_identity(AttemptExecutionIdentity {
        channel: "grok.build".into(),
        credential_revision: 1,
        config_version_id: "synthetic-config".into(),
        config_revision: 1,
        egress: ExecutionEgress::Direct,
        capabilities: EffectiveCapabilities {
            reasoning: true,
            parallel: false,
            stored: false,
            continuation: false,
            compact: false,
            websocket: false,
        },
    });
    Ok((request, attempt))
}
fn evidence(text: &str) -> Result<MessagesSummaryEvidence, Box<dyn Error>> {
    Ok(MessagesSummaryEvidence::try_new(MessagesSummarySource::BuildJson,ResponseId::try_new("response-63")?,MessagesSummaryCompleteness::CompletedResponse,1002,
        vec![MessagesSummaryItem {output_index:0,item:RawJson::from_json_string(json!({"id":"reason-a","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":text},{"type":"summary_text","text":"第二段 🐦"}]}).to_string())?},
            MessagesSummaryItem {output_index:2,item:RawJson::from_json_string(json!({"id":"reason-b","type":"reasoning","status":"completed","content":[],"summary":[]}).to_string())?}])?)
}
async fn writer(
    dir: &Directory,
    secrets: SecretStore,
) -> Result<
    (
        BoundedEventQueue,
        tokio::task::JoinHandle<gateway_store::event_store::EventWriterMetrics>,
    ),
    Box<dyn Error>,
> {
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(16, 1)?)?;
    let task = tokio::spawn(
        AsyncSqliteEventWriter::new(
            dir.db(),
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
    Ok((queue, task))
}
async fn admit(queue: &BoundedEventQueue, request: &RequestEvent, attempt: &AttemptEvent) {
    assert_eq!(
        queue
            .emit_confirmed(GatewayEvent::Request(request.clone()))
            .await,
        EventEmission::Persisted
    );
    assert_eq!(
        queue
            .emit_confirmed(GatewayEvent::Attempt(attempt.clone()))
            .await,
        EventEmission::Persisted
    );
}
fn read(
    dir: &Directory,
    request: &RequestEvent,
    attempt: &AttemptEvent,
    secrets: SecretStore,
) -> Result<Option<MessagesSummaryRecord>, Box<dyn Error>> {
    Ok(
        SqliteMessagesSummaryReader::open(dir.db(), secrets)?.get_owned(
            request.client_key_id(),
            request.request_id(),
            attempt.attempt_id(),
            now()?,
        )?,
    )
}
fn now() -> Result<i64, Box<dyn Error>> {
    Ok(i64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "one storage history proves replay, conflict, key rotation and exclusive expiry without resetting evidence"
)]
async fn messages_summary_owner_replay_conflict_rotation_expiry_and_reopen() -> TestResult {
    let dir = Directory::new()?;
    let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
    let (request, attempt) = events("first", "owner-a")?;
    admit(&queue, &request, &attempt).await;
    let original = evidence("PRIVATE_CPAR63_SUMMARY")?;
    for _ in 0..2 {
        assert_eq!(
            queue
                .emit_summary_evidence_confirmed(attempt.clone(), original.clone())
                .await,
            EventEmission::Persisted
        );
    }
    assert_eq!(
        queue
            .emit_summary_evidence_confirmed(attempt.clone(), evidence("conflict")?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    let record = read(&dir, &request, &attempt, keys(1, &[1], false)?)?.ok_or("retained")?;
    assert_eq!(record.evidence, original);
    assert!(read(&dir, &request, &attempt, keys(1, &[1], true)?).is_err());
    let reader = SqliteMessagesSummaryReader::open(dir.db(), keys(1, &[1], false)?)?;
    assert!(
        reader
            .get_owned(
                &ClientKeyId::try_new("sibling-owner")?,
                request.request_id(),
                attempt.attempt_id(),
                now()?
            )?
            .is_none()
    );
    assert!(
        reader
            .get_owned(
                request.client_key_id(),
                &RequestId::try_new("other-request")?,
                attempt.attempt_id(),
                now()?
            )?
            .is_none()
    );
    drop(reader);
    drop(queue);
    assert_eq!(task.await?.required_events_quarantined, 1);
    let rows = SqliteEventStore::open_read_only(dir.db())?.list_events()?;
    assert_eq!(rows.len(), 2);
    assert!(
        !rows
            .iter()
            .any(|r| matches!(r.event(), GatewayEvent::Usage(_)))
    );
    assert!(
        !serde_json::to_string(
            &rows
                .iter()
                .map(gateway_store::event_store::StoredGatewayEvent::event)
                .collect::<Vec<_>>()
        )?
        .contains("PRIVATE_CPAR63")
    );
    assert!(
        !fs::read(dir.db())?
            .windows(b"PRIVATE_CPAR63".len())
            .any(|v| v == b"PRIVATE_CPAR63")
    );
    let (queue, task) = writer(&dir, keys(2, &[1, 2], false)?).await?;
    let (request_b, attempt_b) = events("second", "owner-b")?;
    admit(&queue, &request_b, &attempt_b).await;
    assert_eq!(
        queue
            .emit_summary_evidence_confirmed(attempt_b.clone(), evidence("new-key")?)
            .await,
        EventEmission::Persisted
    );
    drop(queue);
    task.await?;
    assert_eq!(
        read(&dir, &request, &attempt, keys(2, &[1, 2], false)?)?.ok_or("old key retained")?,
        record
    );
    assert!(read(&dir, &request, &attempt, keys(2, &[2], false)?).is_err());
    assert!(read(&dir, &request_b, &attempt_b, keys(2, &[2], false)?)?.is_some());
    let connection = gateway_store::open(dir.db())?;
    let expires: i64 = connection.query_row(
        "SELECT expires_at_ms FROM messages_summary_evidence WHERE attempt_id=?1",
        [attempt.attempt_id().as_str()],
        |r| r.get(0),
    )?;
    let reader = SqliteMessagesSummaryReader::open(dir.db(), keys(2, &[1, 2], false)?)?;
    assert!(
        reader
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                attempt.attempt_id(),
                expires - 1
            )?
            .is_some()
    );
    assert!(
        reader
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                attempt.attempt_id(),
                expires
            )?
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn messages_summary_ciphertext_swap_tamper_and_event_indexes_fail_closed() -> TestResult {
    for corruption in [
        "swap",
        "tamper",
        "owner",
        "request",
        "expiry",
        "event_index",
        "execution",
    ] {
        let dir = Directory::new()?;
        let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
        let (request, attempt) = events("a", "owner-a")?;
        let (request_b, attempt_b) = events("b", "owner-b")?;
        for (request, attempt) in [(&request, &attempt), (&request_b, &attempt_b)] {
            admit(&queue, request, attempt).await;
            assert_eq!(
                queue
                    .emit_summary_evidence_confirmed(attempt.clone(), evidence("PRIVATE_CPAR63")?)
                    .await,
                EventEmission::Persisted
            );
        }
        drop(queue);
        task.await?;
        let connection = gateway_store::open(dir.db())?;
        match corruption {
            "swap" => {
                connection.execute("UPDATE messages_summary_evidence SET ciphertext=(SELECT ciphertext FROM messages_summary_evidence WHERE attempt_id=?1) WHERE attempt_id=?2",[attempt_b.attempt_id().as_str(),attempt.attempt_id().as_str()])?;
            }
            "tamper" => {
                connection.execute("UPDATE messages_summary_evidence SET ciphertext=randomblob(length(ciphertext)) WHERE attempt_id=?1",[attempt.attempt_id().as_str()])?;
            }
            "owner" => {
                connection.execute("UPDATE messages_summary_evidence SET client_key_id='different-owner' WHERE attempt_id=?1",[attempt.attempt_id().as_str()])?;
            }
            "request" => {
                connection.execute("UPDATE messages_summary_evidence SET request_id='different-request' WHERE attempt_id=?1",[attempt.attempt_id().as_str()])?;
            }
            "expiry" => {
                connection.execute("UPDATE messages_summary_evidence SET expires_at_ms=expires_at_ms+1 WHERE attempt_id=?1",[attempt.attempt_id().as_str()])?;
            }
            "execution" => {
                connection.execute_batch("DROP TRIGGER gateway_attempt_execution_no_update")?;
                connection.execute("UPDATE gateway_attempt_execution SET evidence_json=replace(evidence_json,'grok.build','grok.official') WHERE attempt_id=?1",[attempt.attempt_id().as_str()])?;
            }
            _ => {
                connection.execute_batch("DROP TRIGGER gateway_event_log_no_update;")?;
                connection.execute("UPDATE gateway_event_log SET occurred_at_ms=occurred_at_ms+1 WHERE event_type='attempt' AND event_id=?1",[attempt.attempt_id().as_str()])?;
            }
        }
        let result = read(&dir, &request, &attempt, keys(1, &[1], false)?);
        if matches!(corruption, "owner" | "request") {
            assert!(result?.is_none());
        } else {
            assert!(result.is_err(), "{corruption}");
        }
    }
    Ok(())
}

#[tokio::test]
async fn messages_summary_schema35_unknown_backup_restore_and_private_cli_reopen() -> TestResult {
    let dir = Directory::new()?;
    let mut connection = gateway_store::open(dir.db())?;
    gateway_store::migrate(&mut connection)?;
    gateway_store::rollback_to_version(&mut connection, 35)?;
    let (request, attempt) = events("restore", "owner")?;
    assert!(read(&dir, &request, &attempt, keys(1, &[1], false)?)?.is_none());
    gateway_store::migrate(&mut connection)?;
    drop(connection);
    assert!(read(&dir, &request, &attempt, keys(1, &[1], false)?)?.is_none());
    let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
    admit(&queue, &request, &attempt).await;
    assert_eq!(
        queue
            .emit_summary_evidence_confirmed(attempt.clone(), evidence("PRIVATE_CPAR63_RESTORE")?)
            .await,
        EventEmission::Persisted
    );
    drop(queue);
    task.await?;
    let source = gateway_store::open(dir.db())?;
    let backup = BackupKey::try_from_bytes([0xBA; 32])?;
    let artifact = create_encrypted_backup(&source, &dir.0, &backup)?;
    let restored = dir.0.join("restored.sqlite3");
    restore_encrypted_backup_to_empty_target(&artifact, &restored, &backup)?;
    let expected = SqliteMessagesSummaryReader::open(&restored, keys(1, &[1], false)?)?
        .get_owned(
            request.client_key_id(),
            request.request_id(),
            attempt.attempt_id(),
            now()?,
        )?
        .ok_or("restored")?;
    let key_dir = dir.0.join("keys");
    fs::create_dir(&key_dir)?;
    fs::set_permissions(&key_dir, fs::Permissions::from_mode(0o700))?;
    let key = key_dir.join("1");
    fs::write(&key, [1u8; 32])?;
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600))?;
    // Match the configured versioned key-file convention rather than reading ambient secrets.
    fs::rename(&key, key_dir.join("1.key"))?;
    let run = |output: &std::path::Path, owner: &str| {
        Command::new(env!("CARGO_BIN_EXE_gateway"))
            .args(["messages-summary-evidence", "--db"])
            .arg(&restored)
            .arg("--master-key-dir")
            .arg(&key_dir)
            .args([
                "--active-master-key-version",
                "1",
                "--client-key-id",
                owner,
                "--request-id",
                request.request_id().as_str(),
                "--attempt-id",
                attempt.attempt_id().as_str(),
                "--output",
            ])
            .arg(output)
            .output()
    };
    for name in ["first.json", "after-process-restart.json"] {
        let output = dir.0.join(name);
        let result = run(&output, request.client_key_id().as_str())?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!String::from_utf8_lossy(&result.stdout).contains("PRIVATE_CPAR63"));
        assert!(!String::from_utf8_lossy(&result.stderr).contains("PRIVATE_CPAR63"));
        let value: Value = serde_json::from_slice(&fs::read(&output)?)?;
        assert_eq!(value["status"], "retained");
        assert_eq!(value["record"], serde_json::to_value(&expected)?);
        assert_eq!(fs::metadata(&output)?.mode() & 0o777, 0o600);
        assert!(
            !run(&output, request.client_key_id().as_str())?
                .status
                .success()
        );
    }
    let foreign = dir.0.join("foreign.json");
    assert!(run(&foreign, "foreign-owner")?.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(foreign)?)?,
        json!({"status":"unknown"})
    );
    gateway_store::open(dir.db())?.execute(
        "INSERT INTO schema_migrations(version) VALUES (?1)",
        [gateway_store::CURRENT_SCHEMA_VERSION + 1],
    )?;
    assert!(SqliteMessagesSummaryReader::open(dir.db(), keys(1, &[1], false)?).is_err());
    Ok(())
}

#[tokio::test]
async fn messages_summary_requires_writer_exact_live_lineage_and_bounded_payload() -> TestResult {
    let (queue, _receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(2, 1)?)?;
    let (_, attempt) = events("no-writer", "owner")?;
    assert_eq!(
        queue
            .emit_summary_evidence_confirmed(attempt, evidence("private")?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    let dir = Directory::new()?;
    let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
    let (request, attempt) = events("old", "owner")?;
    admit(&queue, &request, &attempt).await;
    assert_eq!(
        queue
            .emit_confirmed(GatewayEvent::RequestFinished(RequestFinishedEvent {
                request_id: request.request_id().clone(),
                started_at_ms: 1000,
                finished_at_ms: 1001,
                duration_ms: 1,
                first_content_ms: None,
                outcome: RequestOutcome::Succeeded,
                error_code: None
            }))
            .await,
        EventEmission::Persisted
    );
    assert_eq!(
        queue
            .emit_summary_evidence_confirmed(
                attempt,
                evidence("captured after historical completion")?
            )
            .await,
        EventEmission::PersistenceUnavailable
    );
    assert!(evidence(&"x".repeat(MAX_MESSAGES_SUMMARY_BYTES)).is_err());
    drop(queue);
    assert_eq!(task.await?.required_events_quarantined, 1);
    Ok(())
}
