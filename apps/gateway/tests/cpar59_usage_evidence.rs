//! Public queue/writer + owner reader + actual CLI process. Synthetic, transport-free storage
//! qualification supplements (and does not replace) the public Build HTTP regressions.
use gateway_core::*;
use gateway_observability::{BoundedEventQueue, EventQueueConfig};
use gateway_store::{
    event_store::{AsyncSqliteEventWriter, EventWriterConfig, SqliteEventStore},
    native_usage::{SqliteNativeUsageReader, purge_expired},
    secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
};
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
const RAW: &str = r#"{ "input_tokens":3,"output_tokens":5,"zero":0,"null":null,"number":18446744073709551616123456789,"decimal":0.12345678901234567890123456789,"private":"PRIVATE_CPAR59" }"#;
struct Directory(PathBuf);
impl Directory {
    fn new() -> Result<Self, Box<dyn Error>> {
        static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "cpar59-{}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed),
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
fn secrets(active: u32, versions: &[u32], wrong: bool) -> Result<SecretStore, Box<dyn Error>> {
    Ok(SecretStore::new(MasterKeyRing::try_new(
        KeyVersion::try_new(active)?,
        versions
            .iter()
            .map(|v| {
                Ok((
                    KeyVersion::try_new(*v)?,
                    MasterKey::try_from_bytes([if wrong { 0xD0 } else { u8::try_from(*v)? }; 32])?,
                ))
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?,
    )?))
}
fn evidence(raw: &str) -> Result<NativeUsageEvidence, GatewayError> {
    NativeUsageEvidence::try_new(NativeUsageSource::BuildJson, raw.into())
}
fn events(
    label: &str,
    owner: &str,
) -> Result<(RequestEvent, AttemptEvent, UsageEvent), Box<dyn Error>> {
    let request = RequestId::try_new(format!("request-{label}"))?;
    let attempt = AttemptEvent::new(
        request.clone(),
        1,
        RouteId::try_new("route")?,
        RouteCandidateId::try_new("candidate")?,
        CredentialId::try_new("account")?,
        EndpointId::try_new("build")?,
        UpstreamId::try_new("grok")?,
        "model".into(),
        1000,
        1001,
        AttemptOutcome::Succeeded,
        AttemptRetryDecision::Completed,
    );
    let usage = UsageEvent::from_usage(
        request.clone(),
        ResponseId::try_new(format!("response-{label}"))?,
        &Usage {
            input_tokens: Some(3),
            output_tokens: Some(5),
            provenance: UsageProvenance::Measured,
            input_accounting: InputTokenAccounting::Inclusive,
            ..Usage::default()
        },
    )
    .with_attempt_id(attempt.attempt_id().clone());
    Ok((
        RequestEvent::new(
            request,
            ClientKeyId::try_new(owner)?,
            None,
            GatewayProtocol::OpenAiResponses,
            "model".into(),
            "model".into(),
            None,
            false,
        )
        .with_started_at_ms(1000),
        attempt,
        usage,
    ))
}
async fn start(
    dir: &Directory,
    key: Option<SecretStore>,
) -> Result<
    (
        BoundedEventQueue,
        tokio::task::JoinHandle<gateway_store::event_store::EventWriterMetrics>,
    ),
    Box<dyn Error>,
> {
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(8, 1)?)?;
    let mut writer = AsyncSqliteEventWriter::new(
        dir.db(),
        receiver,
        EventWriterConfig::try_new(4, Duration::from_millis(5))?,
    );
    if let Some(key) = key {
        writer = writer.with_native_usage_secret_store(key);
    }
    let task = tokio::spawn(writer.run());
    tokio::time::timeout(Duration::from_secs(5), async {
        while queue.recording_health().snapshot().0 != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
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
fn now() -> Result<i64, Box<dyn Error>> {
    Ok(i64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

#[tokio::test]
async fn native_usage_exact_owner_replay_conflict_and_file_reopen() -> TestResult {
    let dir = Directory::new()?;
    let (queue, task) = start(&dir, Some(secrets(1, &[1], false)?)).await?;
    let (request, attempt, usage) = events("first", "owner-a")?;
    admit(&queue, &request, &attempt).await;
    for _ in 0..2 {
        assert_eq!(
            queue
                .emit_usage_evidence_confirmed(usage.clone(), evidence(RAW)?)
                .await,
            EventEmission::Persisted
        );
    }
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(
                usage.clone(),
                evidence(&RAW.replace("PRIVATE_CPAR59", "CHANGED"))?
            )
            .await,
        EventEmission::PersistenceUnavailable
    );
    let (other, other_attempt, _) = events("other", "owner-b")?;
    admit(&queue, &other, &other_attempt).await;
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(
                usage
                    .clone()
                    .with_attempt_id(other_attempt.attempt_id().clone()),
                evidence(RAW)?
            )
            .await,
        EventEmission::PersistenceUnavailable
    );
    drop(queue);
    task.await?;
    let reader = SqliteNativeUsageReader::open(dir.db(), secrets(1, &[1], false)?)?;
    let record = reader
        .get_owned(
            request.client_key_id(),
            request.request_id(),
            attempt.attempt_id(),
            now()?,
        )?
        .ok_or("missing")?;
    assert_eq!(record.evidence.raw_json(), RAW);
    assert_eq!(record.usage, usage);
    assert!(
        reader
            .get_owned(
                other.client_key_id(),
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
                other.request_id(),
                attempt.attempt_id(),
                now()?
            )?
            .is_none()
    );
    let connection = gateway_store::open(dir.db())?;
    assert_eq!(
        connection.query_row("SELECT count(*) FROM native_usage_evidence", [], |r| r
            .get::<_, i64>(0))?,
        1
    );
    assert_eq!(
        SqliteEventStore::open_read_only(dir.db())?
            .list_events()?
            .iter()
            .filter(|r| matches!(r.event(), GatewayEvent::Usage(_)))
            .count(),
        1
    );
    for table in ["gateway_event_log", "gateway_event_quarantine"] {
        let count: i64 = connection.query_row(
            &format!("SELECT count(*) FROM {table} WHERE payload_json LIKE '%PRIVATE_CPAR59%'"),
            [],
            |r| r.get(0),
        )?;
        assert_eq!(count, 0);
    }
    Ok(())
}

#[tokio::test]
async fn native_usage_rejects_corrupted_source_event_indexes() -> TestResult {
    let dir = Directory::new()?;
    let (queue, task) = start(&dir, Some(secrets(1, &[1], false)?)).await?;
    let (request, attempt, usage) = events("index-integrity", "owner")?;
    admit(&queue, &request, &attempt).await;
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(usage, evidence(RAW)?)
            .await,
        EventEmission::Persisted
    );
    drop(queue);
    task.await?;
    let connection = gateway_store::open(dir.db())?;
    // Simulate file corruption beyond the normal append-only SQL admission guard.
    connection.execute_batch("DROP TRIGGER gateway_event_log_no_update")?;
    let reader = SqliteNativeUsageReader::open(dir.db(), secrets(1, &[1], false)?)?;
    for kind in ["request", "attempt", "usage"] {
        let original: (Option<String>, Option<i64>) = connection.query_row(
            "SELECT request_id,occurred_at_ms FROM gateway_event_log WHERE event_type=?1",
            [kind],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        for change in [
            "request_id='corrupted-index'",
            "occurred_at_ms=COALESCE(occurred_at_ms,0)+1",
        ] {
            assert_eq!(
                connection.execute(
                    &format!("UPDATE gateway_event_log SET {change} WHERE event_type=?1"),
                    [kind]
                )?,
                1
            );
            assert!(
                SqliteEventStore::open_read_only(dir.db())?
                    .list_events()
                    .is_err()
            );
            assert!(
                reader
                    .get_owned(
                        request.client_key_id(),
                        request.request_id(),
                        attempt.attempt_id(),
                        now()?
                    )
                    .is_err(),
                "private reader accepted corrupted {kind} index: {change}"
            );
            connection.execute(
                "UPDATE gateway_event_log SET request_id=?1,occurred_at_ms=?2 WHERE event_type=?3",
                (&original.0, original.1, kind),
            )?;
            assert!(
                reader
                    .get_owned(
                        request.client_key_id(),
                        request.request_id(),
                        attempt.attempt_id(),
                        now()?
                    )?
                    .is_some()
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn native_usage_ciphertext_swap_tamper_and_key_authentication() -> TestResult {
    let dir = Directory::new()?;
    let (queue, task) = start(&dir, Some(secrets(1, &[1], false)?)).await?;
    let mut identities = Vec::new();
    for label in ["a", "b"] {
        let (request, attempt, usage) = events(label, label)?;
        admit(&queue, &request, &attempt).await;
        assert_eq!(
            queue
                .emit_usage_evidence_confirmed(usage, evidence(RAW)?)
                .await,
            EventEmission::Persisted
        );
        identities.push((request, attempt));
    }
    drop(queue);
    task.await?;
    let (request, attempt) = &identities[0];
    for keys in [secrets(1, &[1], true)?, secrets(2, &[2], false)?] {
        assert!(
            SqliteNativeUsageReader::open(dir.db(), keys)?
                .get_owned(
                    request.client_key_id(),
                    request.request_id(),
                    attempt.attempt_id(),
                    now()?
                )
                .is_err()
        );
    }
    let connection = gateway_store::open(dir.db())?;
    let get = |id: &AttemptId| -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(connection.query_row(
            "SELECT ciphertext FROM native_usage_evidence WHERE attempt_id=?1",
            [id.as_str()],
            |r| r.get(0),
        )?)
    };
    let original = get(attempt.attempt_id())?;
    let sibling = get(identities[1].1.attempt_id())?;
    let mut damaged = original.clone();
    if let Some(byte) = damaged.last_mut() {
        *byte ^= 1;
    }
    for cipher in [sibling, damaged] {
        connection.execute(
            "UPDATE native_usage_evidence SET ciphertext=?1 WHERE attempt_id=?2",
            (&cipher, attempt.attempt_id().as_str()),
        )?;
        assert!(
            SqliteNativeUsageReader::open(dir.db(), secrets(1, &[1], false)?)?
                .get_owned(
                    request.client_key_id(),
                    request.request_id(),
                    attempt.attempt_id(),
                    now()?
                )
                .is_err()
        );
    }
    connection.execute("UPDATE native_usage_evidence SET ciphertext=?1,expires_at_ms=expires_at_ms+1 WHERE attempt_id=?2",(&original,attempt.attempt_id().as_str()))?;
    assert!(
        SqliteNativeUsageReader::open(dir.db(), secrets(1, &[1], false)?)?
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                attempt.attempt_id(),
                now()?
            )
            .is_err()
    );
    connection.execute("UPDATE native_usage_evidence SET expires_at_ms=expires_at_ms-1,client_key_id='intruder' WHERE attempt_id=?1",[attempt.attempt_id().as_str()])?;
    assert!(
        SqliteNativeUsageReader::open(dir.db(), secrets(1, &[1], false)?)?
            .get_owned(
                &ClientKeyId::try_new("intruder")?,
                request.request_id(),
                attempt.attempt_id(),
                now()?
            )
            .is_err()
    );
    Ok(())
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "one retained history verifies migration, rotation and exclusive expiry without inventing old evidence"
)]
async fn native_usage_legacy_unknown_rotation_and_exclusive_expiry() -> TestResult {
    let dir = Directory::new()?;
    let mut connection = gateway_store::open(dir.db())?;
    gateway_store::migrate(&mut connection)?;
    gateway_store::rollback_to_version(&mut connection, 34)?;
    let (legacy, legacy_attempt, legacy_usage) = events("legacy", "owner")?;
    assert!(
        SqliteNativeUsageReader::open(dir.db(), secrets(1, &[1], false)?)?
            .get_owned(
                legacy.client_key_id(),
                legacy.request_id(),
                legacy_attempt.attempt_id(),
                now()?
            )?
            .is_none()
    );
    gateway_store::migrate(&mut connection)?;
    SqliteEventStore::open(dir.db())?.append_batch(&[
        GatewayEvent::Request(legacy.clone()),
        GatewayEvent::Attempt(legacy_attempt.clone()),
        GatewayEvent::Usage(legacy_usage.clone()),
    ])?;
    let (queue, task) = start(&dir, Some(secrets(1, &[1], false)?)).await?;
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(legacy_usage, evidence(RAW)?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    let (request, attempt, usage) = events("old-key", "owner")?;
    admit(&queue, &request, &attempt).await;
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(usage, evidence(RAW)?)
            .await,
        EventEmission::Persisted
    );
    drop(queue);
    task.await?;
    let (queue, task) = start(&dir, Some(secrets(2, &[1, 2], false)?)).await?;
    let (new, new_attempt, new_usage) = events("new-key", "owner")?;
    admit(&queue, &new, &new_attempt).await;
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(new_usage, evidence(RAW)?)
            .await,
        EventEmission::Persisted
    );
    drop(queue);
    task.await?;
    let reader = SqliteNativeUsageReader::open(dir.db(), secrets(2, &[1, 2], false)?)?;
    assert!(
        reader
            .get_owned(
                legacy.client_key_id(),
                legacy.request_id(),
                legacy_attempt.attempt_id(),
                now()?
            )?
            .is_none()
    );
    assert!(
        reader
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                attempt.attempt_id(),
                now()?
            )?
            .is_some()
    );
    assert!(
        reader
            .get_owned(
                new.client_key_id(),
                new.request_id(),
                new_attempt.attempt_id(),
                now()?
            )?
            .is_some()
    );
    let expiry: i64 = connection.query_row(
        "SELECT expires_at_ms FROM native_usage_evidence WHERE attempt_id=?1",
        [attempt.attempt_id().as_str()],
        |r| r.get(0),
    )?;
    assert!(
        reader
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                attempt.attempt_id(),
                expiry - 1
            )?
            .is_some()
    );
    assert!(
        reader
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                attempt.attempt_id(),
                expiry
            )?
            .is_none()
    );
    let count_before = SqliteEventStore::open_read_only(dir.db())?
        .list_events()?
        .len();
    assert!(purge_expired(&connection, expiry)? > 0);
    assert_eq!(
        SqliteEventStore::open_read_only(dir.db())?
            .list_events()?
            .len(),
        count_before
    );
    connection.execute(
        "INSERT INTO schema_migrations(version) VALUES (?1)",
        [gateway_store::CURRENT_SCHEMA_VERSION + 1],
    )?;
    assert!(SqliteNativeUsageReader::open(dir.db(), secrets(2, &[1, 2], false)?).is_err());
    Ok(())
}

#[tokio::test]
async fn native_usage_encrypted_snapshot_restores_with_original_keys() -> TestResult {
    use gateway_store::backup::{
        BackupKey, create_encrypted_backup, preflight_encrypted_backup,
        restore_encrypted_backup_to_empty_target,
    };
    let dir = Directory::new()?;
    let (queue, task) = start(&dir, Some(secrets(1, &[1], false)?)).await?;
    let (request, attempt, usage) = events("backup", "owner")?;
    admit(&queue, &request, &attempt).await;
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(usage.clone(), evidence(RAW)?)
            .await,
        EventEmission::Persisted
    );
    drop(queue);
    task.await?;
    let source = gateway_store::open(dir.db())?;
    let backup_key = BackupKey::try_from_bytes([0xBA; 32])?;
    let artifact = create_encrypted_backup(&source, &dir.0, &backup_key)?;
    assert!(
        !artifact
            .windows(b"PRIVATE_CPAR59".len())
            .any(|w| w == b"PRIVATE_CPAR59")
    );
    assert_eq!(
        preflight_encrypted_backup(&artifact, &dir.0, &backup_key)?.source_schema_version(),
        gateway_store::CURRENT_SCHEMA_VERSION
    );
    let restored = dir.0.join("restored.sqlite3");
    restore_encrypted_backup_to_empty_target(&artifact, &restored, &backup_key)?;
    let record = SqliteNativeUsageReader::open(&restored, secrets(1, &[1], false)?)?
        .get_owned(
            request.client_key_id(),
            request.request_id(),
            attempt.attempt_id(),
            now()?,
        )?
        .ok_or("restored evidence missing")?;
    assert_eq!(record.usage, usage);
    assert_eq!(record.evidence.raw_json(), RAW);
    assert!(
        SqliteNativeUsageReader::open(&restored, secrets(1, &[1], true)?)?
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                attempt.attempt_id(),
                now()?
            )
            .is_err()
    );
    assert!(
        restore_encrypted_backup_to_empty_target(
            &artifact,
            dir.0.join("wrong-backup-key.sqlite3"),
            &BackupKey::try_from_bytes([0xBB; 32])?
        )
        .is_err()
    );
    assert!(!dir.0.join("wrong-backup-key.sqlite3").exists());
    Ok(())
}

#[tokio::test]
async fn native_usage_confirmation_timeout_and_missing_writer_fail_closed() -> TestResult {
    let dir = Directory::new()?;
    let (queue, task) = start(&dir, Some(secrets(1, &[1], false)?)).await?;
    let (request, attempt, usage) = events("locked", "owner")?;
    admit(&queue, &request, &attempt).await;
    let connection = gateway_store::open(dir.db())?;
    connection.execute_batch("BEGIN IMMEDIATE")?;
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(usage.clone(), evidence(RAW)?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    let reader = SqliteNativeUsageReader::open(dir.db(), secrets(1, &[1], false)?)?;
    assert!(
        reader
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                attempt.attempt_id(),
                now()?
            )?
            .is_none()
    );
    connection.execute_batch("ROLLBACK")?;
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if reader
                .get_owned(
                    request.client_key_id(),
                    request.request_id(),
                    attempt.attempt_id(),
                    now()?,
                )?
                .is_some()
            {
                return Ok::<_, Box<dyn Error>>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    drop(queue);
    task.await?;
    let (queue, _receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(usage, evidence(RAW)?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    Ok(())
}

#[tokio::test]
async fn native_usage_missing_attempt_is_quarantined_without_blocking_new_requests() -> TestResult {
    let dir = Directory::new()?;
    let (queue, task) = start(&dir, Some(secrets(1, &[1], false)?)).await?;
    let (request, attempt, usage) = events("missing-attempt", "owner")?;
    admit(&queue, &request, &attempt).await;
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(
                usage.with_attempt_id(AttemptId::try_new("never-admitted-attempt")?),
                evidence(RAW)?
            )
            .await,
        EventEmission::PersistenceUnavailable
    );
    let (next, _, _) = events("after-missing", "owner")?;
    assert_eq!(
        queue.emit_confirmed(GatewayEvent::Request(next)).await,
        EventEmission::Persisted
    );
    drop(queue);
    task.await?;
    assert_eq!(
        gateway_store::open(dir.db())?.query_row(
            "SELECT count(*) FROM native_usage_evidence",
            [],
            |r| r.get::<_, i64>(0)
        )?,
        0
    );
    Ok(())
}

#[tokio::test]
async fn native_usage_cli_reopens_authenticates_and_writes_only_private_output() -> TestResult {
    let dir = Directory::new()?;
    let (queue, task) = start(&dir, Some(secrets(1, &[1], false)?)).await?;
    let (request, attempt, usage) = events("cli", "owner")?;
    admit(&queue, &request, &attempt).await;
    assert_eq!(
        queue
            .emit_usage_evidence_confirmed(usage, evidence(RAW)?)
            .await,
        EventEmission::Persisted
    );
    drop(queue);
    task.await?;
    let keys = dir.0.join("keys");
    fs::create_dir(&keys)?;
    fs::write(keys.join("1.key"), [1_u8; 32])?;
    let run = |owner: &str, output: &str| {
        Command::new(env!("CARGO_BIN_EXE_gateway"))
            .arg("usage-evidence")
            .arg("--db")
            .arg(dir.db())
            .arg("--master-key-dir")
            .arg(&keys)
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
            .arg(dir.0.join(output))
            .output()
    };
    let result = run("owner", "retained.json")?;
    assert!(result.status.success());
    assert!(!String::from_utf8(result.stdout)?.contains("PRIVATE_CPAR59"));
    assert!(result.stderr.is_empty());
    let output = dir.0.join("retained.json");
    assert_eq!(fs::metadata(&output)?.mode() & 0o777, 0o600);
    let record: serde_json::Value = serde_json::from_slice(&fs::read(&output)?)?;
    assert_eq!(record["status"], "retained");
    assert_eq!(record["record"]["evidence"]["raw_json"], RAW);
    assert!(!run("owner", "retained.json")?.status.success());
    assert!(run("sibling", "unknown.json")?.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(dir.0.join("unknown.json"))?)?,
        serde_json::json!({"status":"unknown"})
    );
    fs::write(keys.join("1.key"), [9_u8; 32])?;
    let result = run("owner", "bad-key.json")?;
    assert!(!result.status.success());
    assert!(!dir.0.join("bad-key.json").exists());
    assert!(!String::from_utf8(result.stderr)?.contains("PRIVATE_CPAR59"));
    Ok(())
}
