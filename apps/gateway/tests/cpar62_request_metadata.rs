//! Synthetic request evidence lifecycle; public HTTP behavior lives in the owning runtime tests.
use gateway_core::*;
use gateway_observability::{BoundedEventQueue, EventQueueConfig};
use gateway_store::{
    backup::{BackupKey, create_encrypted_backup, restore_encrypted_backup_to_empty_target},
    event_store::{AsyncSqliteEventWriter, EventWriterConfig, SqliteEventStore},
    request_metadata::{RequestMetadataDiagnostic, SqliteRequestMetadataReader, purge_expired},
    secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
};
use serde_json::{Value, json};
use std::{
    error::Error,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
type TestResult = Result<(), Box<dyn Error>>;
const RAW: &str = r#"{ "session_id":"PRIVATE_C4_STORAGE", "x-codex-turn-metadata":"{ \"session_id\":\"PRIVATE_C4_STORAGE\", \"request_kind\":\"turn\", \"auto_review_enabled\":true, \"node_repl_disabled\":false, \"model\":\"client-assertion\" }" }"#;
#[path = "request_metadata/messages.rs"]
mod messages;
struct Directory(PathBuf);
impl Directory {
    fn new() -> Result<Self, Box<dyn Error>> {
        Self::at(SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos())
    }
    fn at(timestamp_ns: u128) -> Result<Self, Box<dyn Error>> {
        static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "cpar62-meta-{}-{timestamp_ns}-{}",
            std::process::id(),
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

#[test]
fn request_metadata_fixture_directories_do_not_collide_at_the_same_timestamp() -> TestResult {
    let first = Directory::at(1_000_000)?;
    let second = Directory::at(1_000_000)?;
    assert_ne!(first.0, second.0);
    for directory in [&first, &second] {
        assert_eq!(fs::metadata(&directory.0)?.mode() & 0o777, 0o700);
        assert!(!directory.db().exists());
    }
    Ok(())
}

fn keys(active: u32, versions: &[u32], wrong: bool) -> Result<SecretStore, Box<dyn Error>> {
    Ok(SecretStore::new(MasterKeyRing::try_new(
        KeyVersion::try_new(active)?,
        versions
            .iter()
            .map(|v| {
                Ok((
                    KeyVersion::try_new(*v)?,
                    MasterKey::try_from_bytes([if wrong { 0xee } else { u8::try_from(*v)? }; 32])?,
                ))
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?,
    )?))
}
fn now() -> Result<i64, Box<dyn Error>> {
    Ok(i64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}
fn request(id: &str, owner: &str) -> Result<RequestEvent, Box<dyn Error>> {
    Ok(RequestEvent::new(
        RequestId::try_new(id)?,
        ClientKeyId::try_new(owner)?,
        None,
        GatewayProtocol::OpenAiResponses,
        "grok-4.5".into(),
        "grok-4.5".into(),
        None,
        false,
    )
    .with_started_at_ms(1000))
}
fn evidence() -> Result<RequestMetadataEvidence, GatewayError> {
    RequestMetadataEvidence::try_new(RAW.into())
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
fn read(
    dir: &Directory,
    request: &RequestEvent,
    secrets: SecretStore,
) -> Result<Option<RequestMetadataDiagnostic>, Box<dyn Error>> {
    Ok(
        SqliteRequestMetadataReader::open(dir.db(), secrets)?.get_owned(
            request.client_key_id(),
            request.request_id(),
            None,
            now()?,
        )?,
    )
}
fn attempt(request: &RequestEvent, sequence: u64) -> Result<AttemptEvent, Box<dyn Error>> {
    Ok(AttemptEvent::new(
        request.request_id().clone(),
        sequence,
        RouteId::try_new("route")?,
        RouteCandidateId::try_new("candidate")?,
        CredentialId::try_new("synthetic-account")?,
        EndpointId::try_new("build")?,
        UpstreamId::try_new("grok")?,
        "grok-4.5".into(),
        1001,
        1002,
        AttemptOutcome::Succeeded,
        AttemptRetryDecision::Completed,
    ))
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "one retained history checks replay, actual association, key rotation and expiry without replacing evidence"
)]
async fn request_metadata_owner_replay_conflict_rotation_expiry_and_reopen() -> TestResult {
    let dir = Directory::new()?;
    let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
    let original = request("request-a", "owner-a")?;
    for _ in 0..2 {
        assert_eq!(
            queue
                .emit_request_metadata_confirmed(original.clone(), evidence()?)
                .await,
            EventEmission::Persisted
        );
    }
    for sequence in [1, 2] {
        assert_eq!(
            queue
                .emit_confirmed(GatewayEvent::Attempt(attempt(&original, sequence)?))
                .await,
            EventEmission::Persisted
        );
    }
    let result = read(&dir, &original, keys(1, &[1], false)?)?.ok_or("retained")?;
    assert_eq!(result.record.evidence.raw_json(), RAW);
    assert_eq!(result.attempts.len(), 2);
    let changed = RequestMetadataEvidence::try_new(format!(" {RAW}"))?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(original.clone(), changed)
            .await,
        EventEmission::PersistenceUnavailable
    );
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(request("request-a", "foreign-owner")?, evidence()?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    assert!(read(&dir, &original, keys(1, &[1], true)?).is_err());
    assert!(
        read(
            &dir,
            &request("request-a", "foreign-owner")?,
            keys(1, &[1], false)?
        )?
        .is_none()
    );
    let foreign = request("request-foreign", "foreign-owner")?;
    assert_eq!(
        queue
            .emit_confirmed(GatewayEvent::Request(foreign.clone()))
            .await,
        EventEmission::Persisted
    );
    let foreign_attempt = attempt(&foreign, 1)?;
    assert_eq!(
        queue
            .emit_confirmed(GatewayEvent::Attempt(foreign_attempt.clone()))
            .await,
        EventEmission::Persisted
    );
    let reader = SqliteRequestMetadataReader::open(dir.db(), keys(1, &[1], false)?)?;
    assert!(
        reader
            .get_owned(
                original.client_key_id(),
                original.request_id(),
                Some(foreign_attempt.attempt_id()),
                now()?
            )?
            .is_none()
    );
    for actual in &result.attempts {
        assert!(
            reader
                .get_owned(
                    original.client_key_id(),
                    original.request_id(),
                    Some(actual.attempt_id()),
                    now()?
                )?
                .is_some()
        );
    }
    drop(queue);
    task.await?;
    let (queue, task) = writer(&dir, keys(2, &[1, 2], false)?).await?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(original.clone(), evidence()?)
            .await,
        EventEmission::Persisted
    );
    let newer = request("request-b", "owner-a")?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(newer.clone(), evidence()?)
            .await,
        EventEmission::Persisted
    );
    assert!(read(&dir, &original, keys(2, &[2], false)?).is_err());
    assert!(read(&dir, &newer, keys(2, &[2], false)?)?.is_some());
    let connection = gateway_store::open(dir.db())?;
    let versions: Vec<i64> = connection
        .prepare("SELECT key_version FROM request_metadata_evidence ORDER BY request_id")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    assert_eq!(versions, vec![1, 2]);
    let expiry: i64 = connection.query_row(
        "SELECT expires_at_ms FROM request_metadata_evidence WHERE request_id='request-a'",
        [],
        |r| r.get(0),
    )?;
    let reader = SqliteRequestMetadataReader::open(dir.db(), keys(2, &[1, 2], false)?)?;
    assert!(
        reader
            .get_owned(
                original.client_key_id(),
                original.request_id(),
                None,
                expiry - 1
            )?
            .is_some()
    );
    assert!(
        reader
            .get_owned(
                original.client_key_id(),
                original.request_id(),
                None,
                expiry
            )?
            .is_none()
    );
    assert!(purge_expired(&connection, expiry)? >= 1);
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(original.clone(), evidence()?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    assert_eq!(connection.query_row("SELECT count(*) FROM request_metadata_evidence WHERE request_id='request-a' AND ciphertext IS NULL",[],|r|r.get::<_,i64>(0))?,1);
    drop(queue);
    task.await?;
    let (queue, task) = writer(&dir, keys(2, &[1, 2], false)?).await?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(original.clone(), evidence()?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    drop(queue);
    task.await?;
    let rows = SqliteEventStore::open_read_only(dir.db())?.list_events()?;
    assert_eq!(
        rows.iter()
            .filter(|r| matches!(r.event(), GatewayEvent::Usage(_)))
            .count(),
        0
    );
    assert!(rows.iter().all(|r| {
        !serde_json::to_string(r.event()).is_ok_and(|v| v.contains("PRIVATE_C4_STORAGE"))
    }));
    println!(
        "CPAR62_C4_STORAGE {}",
        json!({"captures":2,"request_a_attempts":2,"usage":0,"rotation":true,"expired_replay_blocked_after_restart":true})
    );
    Ok(())
}

#[tokio::test]
async fn request_metadata_ciphertext_and_source_indexes_fail_closed() -> TestResult {
    for corruption in ["swap", "expiry", "owner", "request-index"] {
        let dir = Directory::new()?;
        let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
        let first = request("first", "owner")?;
        for id in ["first", "second"] {
            assert_eq!(
                queue
                    .emit_request_metadata_confirmed(request(id, "owner")?, evidence()?)
                    .await,
                EventEmission::Persisted
            );
        }
        drop(queue);
        task.await?;
        let connection = gateway_store::open(dir.db())?;
        match corruption {
            "swap" => {
                connection.execute("UPDATE request_metadata_evidence SET ciphertext=(SELECT ciphertext FROM request_metadata_evidence WHERE request_id='second') WHERE request_id='first'",[])?;
            }
            "expiry" => {
                connection.execute("UPDATE request_metadata_evidence SET created_at_ms=created_at_ms+1,expires_at_ms=expires_at_ms+1 WHERE request_id='first'",[])?;
            }
            "owner" => {
                connection.execute("UPDATE request_metadata_evidence SET client_key_id='foreign' WHERE request_id='first'",[])?;
            }
            _ => {
                connection.execute_batch("DROP TRIGGER gateway_event_log_no_update; UPDATE gateway_event_log SET occurred_at_ms=occurred_at_ms+1 WHERE event_type='request' AND event_id='first';")?;
            }
        }
        let result = read(&dir, &first, keys(1, &[1], false)?);
        if corruption == "owner" {
            assert!(result?.is_none());
            assert!(read(&dir, &request("first", "foreign")?, keys(1, &[1], false)?).is_err());
        } else {
            assert!(result.is_err(), "{corruption}");
        }
    }
    Ok(())
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "migration, coherent encrypted restore and independent private readers verify one retained history"
)]
async fn request_metadata_legacy_unknown_restore_and_private_cli_reopen() -> TestResult {
    let dir = Directory::new()?;
    let mut connection = gateway_store::open(dir.db())?;
    gateway_store::migrate(&mut connection)?;
    gateway_store::rollback_to_version(&mut connection, 36)?;
    let legacy = request("legacy", "owner")?;
    assert!(read(&dir, &legacy, keys(1, &[1], false)?)?.is_none());
    let payload = serde_json::to_string(&GatewayEvent::Request(legacy.clone()))?;
    connection.execute("INSERT INTO gateway_event_log(event_type,event_id,request_id,occurred_at_ms,payload_json) VALUES ('request',?1,?1,1000,?2)", [legacy.request_id().as_str(),payload.as_str()])?;
    connection.execute("INSERT INTO gateway_request_recording(request_id,started_at_ms,state) VALUES (?1,1000,'active')",[legacy.request_id().as_str()])?;
    gateway_store::migrate(&mut connection)?;
    assert_eq!(connection.query_row("SELECT payload_json FROM gateway_event_log WHERE event_type='request' AND event_id='legacy'",[],|r|r.get::<_,String>(0))?,payload);
    drop(connection);
    let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(legacy, evidence()?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    let expired = request("expired", "owner")?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(expired.clone(), evidence()?)
            .await,
        EventEmission::Persisted
    );
    let connection = gateway_store::open(dir.db())?;
    let expiry: i64 = connection.query_row(
        "SELECT expires_at_ms FROM request_metadata_evidence WHERE request_id='expired'",
        [],
        |r| r.get(0),
    )?;
    purge_expired(&connection, expiry)?;
    let live = request("live", "owner")?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(live.clone(), evidence()?)
            .await,
        EventEmission::Persisted
    );
    drop(queue);
    task.await?;
    let backup = BackupKey::try_from_bytes([0xba; 32])?;
    let artifact = create_encrypted_backup(&gateway_store::open(dir.db())?, &dir.0, &backup)?;
    let restored = dir.0.join("restored.sqlite3");
    restore_encrypted_backup_to_empty_target(&artifact, &restored, &backup)?;
    assert_eq!(gateway_store::open(&restored)?.query_row("SELECT count(*) FROM request_metadata_evidence WHERE request_id='expired' AND ciphertext IS NULL",[],|r|r.get::<_,i64>(0))?,1);
    let expected = SqliteRequestMetadataReader::open(&restored, keys(1, &[1], false)?)?
        .get_owned(live.client_key_id(), live.request_id(), None, now()?)?
        .ok_or("restored")?;
    assert!(expected.attempts.is_empty());
    let key_dir = dir.0.join("keys");
    fs::create_dir(&key_dir)?;
    fs::set_permissions(&key_dir, fs::Permissions::from_mode(0o700))?;
    let key = key_dir.join("1.key");
    fs::write(&key, [1u8; 32])?;
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600))?;
    let run = |output: &std::path::Path, owner: &str| {
        Command::new(env!("CARGO_BIN_EXE_gateway"))
            .args(["request-metadata-evidence", "--db"])
            .arg(&restored)
            .arg("--master-key-dir")
            .arg(&key_dir)
            .args([
                "--active-master-key-version",
                "1",
                "--client-key-id",
                owner,
                "--request-id",
                live.request_id().as_str(),
                "--output",
            ])
            .arg(output)
            .output()
    };
    for name in ["first.json", "after-reader-process-restart.json"] {
        let output = dir.0.join(name);
        let result = run(&output, "owner")?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!String::from_utf8_lossy(&result.stdout).contains("PRIVATE_C4"));
        assert!(!String::from_utf8_lossy(&result.stderr).contains("PRIVATE_C4"));
        let value: Value = serde_json::from_slice(&fs::read(&output)?)?;
        assert_eq!(value["status"], "retained");
        assert_eq!(value["record"], serde_json::to_value(&expected)?);
        assert_eq!(fs::metadata(&output)?.mode() & 0o777, 0o600);
        assert!(!run(&output, "owner")?.status.success());
    }
    let foreign = dir.0.join("foreign.json");
    assert!(run(&foreign, "foreign")?.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(foreign)?)?,
        json!({"status":"unknown"})
    );
    fs::write(&key, [0xee; 32])?;
    let bad = run(&dir.0.join("bad-key.json"), "owner")?;
    assert!(!bad.status.success());
    assert!(!String::from_utf8_lossy(&bad.stderr).contains("PRIVATE_C4"));
    gateway_store::open(&restored)?.execute(
        "INSERT INTO schema_migrations(version) VALUES (?1)",
        [gateway_store::CURRENT_SCHEMA_VERSION + 1],
    )?;
    assert!(SqliteRequestMetadataReader::open(&restored, keys(1, &[1], false)?).is_err());
    Ok(())
}

#[tokio::test]
async fn request_metadata_missing_writer_and_confirmation_timeout_fail_closed() -> TestResult {
    let dir = Directory::new()?;
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
    let req = request("no-writer", "owner")?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(req.clone(), evidence()?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    let unused = AsyncSqliteEventWriter::new(
        dir.db(),
        receiver,
        EventWriterConfig::try_new(1, Duration::from_millis(5))?,
    )
    .with_native_usage_secret_store(keys(1, &[1], false)?);
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(req.clone(), evidence()?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(req, evidence()?)
            .await,
        EventEmission::RequiredQueueFull
    );
    drop(unused);
    assert!(!dir.db().exists());
    Ok(())
}

#[tokio::test]
#[ignore = "only launched by the bounded parent process test"]
async fn request_metadata_crash_writer_child() -> TestResult {
    let dir = Directory(PathBuf::from(std::env::var("CPAR62_METADATA_CHILD_DIR")?));
    let (queue, _task) = writer(&dir, keys(1, &[1], false)?).await?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(request("crash-request", "owner")?, evidence()?)
            .await,
        EventEmission::Persisted
    );
    fs::write(dir.0.join("committed.ready"), b"persisted")?;
    std::future::pending::<()>().await;
    Ok(())
}

#[tokio::test]
async fn request_metadata_actual_writer_kill_restart_preserves_capture() -> TestResult {
    let dir = Directory::new()?;
    let mut child = Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "request_metadata_crash_writer_child",
            "--ignored",
            "--nocapture",
        ])
        .env("CPAR62_METADATA_CHILD_DIR", &dir.0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let ready = tokio::time::timeout(Duration::from_secs(10), async {
        while !dir.0.join("committed.ready").exists() {
            if let Some(status) = child.try_wait()? {
                return Err(std::io::Error::other(format!("child exited {status}")));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok::<_, std::io::Error>(())
    })
    .await;
    let killed = child.kill();
    let exited = child.wait()?;
    ready??;
    killed?;
    assert!(!exited.success());
    let req = request("crash-request", "owner")?;
    let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
    assert_eq!(queue.recording_health().snapshot().4, 1);
    let restored =
        read(&dir, &req, keys(1, &[1], false)?)?.ok_or("retained after killed writer restart")?;
    assert_eq!(restored.record.evidence.raw_json(), RAW);
    assert!(restored.attempts.is_empty());
    drop(queue);
    task.await?;
    println!(
        "CPAR62_C4_RESTART {}",
        json!({"confirmed_before_kill":true,"writer_restarted":true,"recovered_interrupted":1,"capture_readback":true,"attempt":0,"usage":0})
    );
    Ok(())
}
