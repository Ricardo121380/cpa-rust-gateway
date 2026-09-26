//! Fault tests exercise the real queue/writer and kill an independent process.
use super::*;
use gateway_core::{
    AttemptEvent, AttemptOutcome, AttemptRetryDecision, ClientKeyId, CredentialId, EndpointId,
    GatewayEventSink, GatewayProtocol, RequestEvent, RequestFinishedEvent, RequestOutcome,
    ResponseId, RouteCandidateId, RouteId, UpstreamId, Usage, UsageEvent,
};
use gateway_observability::{BoundedEventQueue, EventQueueConfig};
use std::{
    fs,
    process::{Command, Stdio},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    std::env::temp_dir().join(format!("cpar-m1-{}-{stamp}-{label}", std::process::id()))
}
fn events() -> Result<Vec<GatewayEvent>, Box<dyn std::error::Error>> {
    let id = RequestId::try_new("durable-request")?;
    let attempt = AttemptEvent::new(
        id.clone(),
        1,
        RouteId::try_new("route")?,
        RouteCandidateId::try_new("candidate")?,
        CredentialId::try_new("account")?,
        EndpointId::try_new("endpoint")?,
        UpstreamId::try_new("provider")?,
        "exact-model".into(),
        1000,
        1002,
        AttemptOutcome::Succeeded,
        AttemptRetryDecision::Completed,
    );
    Ok(vec![
        GatewayEvent::Request(
            RequestEvent::new(
                id.clone(),
                ClientKeyId::try_new("client")?,
                None,
                GatewayProtocol::OpenAiResponses,
                "exact-model".into(),
                "exact-model".into(),
                None,
                true,
            )
            .with_started_at_ms(1000),
        ),
        GatewayEvent::Attempt(attempt.clone()),
        GatewayEvent::Usage(
            UsageEvent::from_usage(
                id.clone(),
                ResponseId::try_new("response")?,
                &Usage {
                    input_tokens: Some(3),
                    output_tokens: Some(5),
                    ..Usage::default()
                },
            )
            .with_attempt_id(attempt.attempt_id().clone()),
        ),
        GatewayEvent::RequestFinished(RequestFinishedEvent {
            request_id: id,
            started_at_ms: 1000,
            finished_at_ms: 1010,
            duration_ms: 10,
            first_content_ms: Some(3),
            outcome: RequestOutcome::Succeeded,
            error_code: None,
        }),
    ])
}
async fn ready(queue: &BoundedEventQueue) -> TestResult {
    tokio::time::timeout(Duration::from_secs(5), async {
        while queue.recording_health().snapshot().0 != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn acknowledgements_follow_commit_and_poison_survives_restart() -> TestResult {
    let file = path("ack.sqlite");
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(4, 1)?)?;
    let writer = AsyncSqliteEventWriter::new(&file, receiver, EventWriterConfig::default());
    let task = tokio::spawn(writer.run());
    ready(&queue).await?;
    let fixture = events()?;
    for event in &fixture {
        assert_eq!(
            queue.emit_confirmed(event.clone()).await,
            EventEmission::Persisted
        );
        // Observe the actual database, not a queue counter, after every confirmation.
        assert!(
            !SqliteEventStore::open_read_only(&file)?
                .list_events()?
                .is_empty()
        );
    }
    let poison = GatewayEvent::Usage(UsageEvent::from_usage(
        RequestId::try_new("durable-request")?,
        ResponseId::try_new("different-response")?,
        &Usage::default(),
    ));
    assert_eq!(
        queue.emit_confirmed(poison.clone()).await,
        EventEmission::PersistenceUnavailable
    );
    assert_eq!(
        queue.emit_confirmed(poison).await,
        EventEmission::PersistenceUnavailable
    );
    assert_eq!(
        queue.emit_confirmed(fixture[0].clone()).await,
        EventEmission::Persisted
    );
    let snapshot = queue.recording_health().snapshot();
    assert_eq!(snapshot.1, 0);
    assert!(snapshot.2 > 0);
    assert_eq!(snapshot.3, 2);
    drop(queue);
    let metrics = task.await?;
    assert_eq!(metrics.required_events_quarantined, 2);
    let mut store = SqliteEventStore::open(&file)?;
    assert_eq!(store.list_events()?.len(), 4);
    let count: i64 =
        store
            .connection
            .query_row("SELECT count(*) FROM gateway_event_quarantine", [], |r| {
                r.get(0)
            })?;
    assert_eq!(count, 1);
    assert_eq!(store.recover_interrupted_requests()?, 0);
    drop(store);
    fs::remove_file(file)?;
    Ok(())
}

#[tokio::test]
async fn locked_database_fails_closed_then_recovers_without_replaying_upstream() -> TestResult {
    let file = path("locked.sqlite");
    let mut store = SqliteEventStore::open(&file)?;
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(2, 1)?)?;
    let writer = AsyncSqliteEventWriter::new(&file, receiver, EventWriterConfig::default());
    let task = tokio::spawn(writer.run());
    ready(&queue).await?;
    let lock = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let request = events()?.remove(0);
    assert_eq!(
        queue.emit_confirmed(request).await,
        EventEmission::PersistenceUnavailable
    );
    assert!(
        SqliteEventStore::open_read_only(&file)?
            .list_events()?
            .is_empty()
    );
    // Timed-out admission is never evidence to invoke/replay a provider. Commit may be late.
    lock.rollback()?;
    tokio::time::timeout(Duration::from_secs(5), async {
        while queue.recording_health().snapshot().1 != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(store.list_events()?.len(), 1);
    drop(queue);
    task.await?;
    assert_eq!(store.recover_interrupted_requests()?, 1);
    assert_eq!(store.list_events()?.len(), 1); // no fabricated RequestFinished or Usage
    drop(store);
    fs::remove_file(file)?;
    Ok(())
}

#[tokio::test]
async fn full_queue_and_closed_writer_never_acknowledge_durability() -> TestResult {
    let file = path("full.sqlite");
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
    let writer = AsyncSqliteEventWriter::new(&file, receiver, EventWriterConfig::default());
    let request = events()?.remove(0);
    assert_eq!(queue.try_emit(request.clone()), EventEmission::Enqueued);
    assert_eq!(
        queue.emit_confirmed(request.clone()).await,
        EventEmission::RequiredQueueFull
    );
    drop(writer);
    assert_eq!(
        queue.emit_confirmed(request).await,
        EventEmission::PersistenceUnavailable
    );
    assert!(!queue.accepts_requests());
    Ok(())
}

#[test]
fn out_of_order_explicit_lineage_and_state_are_idempotent() -> TestResult {
    let fixture = events()?;
    let mut store = SqliteEventStore::open_in_memory()?;
    for i in [2, 3, 1, 0] {
        assert_eq!(store.append_batch(&[fixture[i].clone()])?, 1);
    }
    assert_eq!(store.append_batch(&fixture)?, 0);
    assert_eq!(store.recover_interrupted_requests()?, 0);
    let mut usage_count = 0;
    store.visit_usage_lineages(&UsageEventQuery::default(), |_, attempt, usage| {
        assert_eq!(usage.attempt_id(), Some(attempt.attempt_id()));
        usage_count += 1;
        Ok(true)
    })?;
    assert_eq!(usage_count, 1);
    Ok(())
}

#[tokio::test]
async fn request_scope_attaches_confirmed_attempt_and_rejects_conflicting_usage() -> TestResult {
    let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(4, 1)?)?;
    let sink = gateway_observability::RequestEventSink::new(Arc::new(queue));
    let fixture = events()?;
    assert_eq!(
        sink.emit_confirmed(fixture[1].clone()).await,
        EventEmission::Enqueued
    );
    let GatewayEvent::Usage(usage) = &fixture[2] else {
        return Err("missing usage".into());
    };
    let missing = UsageEvent::from_usage(
        usage.request_id().clone(),
        usage.response_id().clone(),
        &Usage::default(),
    );
    assert_eq!(
        sink.emit_confirmed(GatewayEvent::Usage(missing)).await,
        EventEmission::Enqueued
    );
    let conflicting = usage
        .clone()
        .with_attempt_id(gateway_core::AttemptId::try_new("foreign-attempt")?);
    assert_eq!(
        sink.emit_confirmed(GatewayEvent::Usage(conflicting)).await,
        EventEmission::PersistenceUnavailable
    );
    assert!(matches!(
        receiver.try_recv(),
        Some(GatewayEvent::Attempt(_))
    ));
    let Some(GatewayEvent::Usage(recorded)) = receiver.try_recv() else {
        return Err("missing scoped usage".into());
    };
    assert_eq!(recorded.attempt_id(), usage.attempt_id());
    assert!(receiver.try_recv().is_none());
    Ok(())
}

// Spawned only by the parent below, never run as part of a normal test sweep.
#[tokio::test]
#[ignore = "subprocess crash fixture"]
async fn crash_child() -> TestResult {
    let file = std::env::var("CPAR_M1_CRASH_DB")?;
    let boundary: usize = std::env::var("CPAR_M1_CRASH_BOUNDARY")?.parse()?;
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(4, 1)?)?;
    let writer = AsyncSqliteEventWriter::new(&file, receiver, EventWriterConfig::default());
    let _task = tokio::spawn(writer.run());
    ready(&queue).await?;
    for event in events()?.into_iter().take(boundary) {
        assert_eq!(queue.emit_confirmed(event).await, EventEmission::Persisted);
    }
    fs::write(format!("{file}.ready"), b"committed")?;
    std::future::pending::<()>().await;
    Ok(())
}

#[test]
fn process_kill_at_each_confirmed_boundary_recovers_only_known_facts() -> TestResult {
    for boundary in 1..=4 {
        let file = path("crash.sqlite");
        let signal = file.with_extension("sqlite.ready");
        let mut child = Command::new(std::env::current_exe()?)
            .args([
                "--ignored",
                "--exact",
                "event_store::durability_tests::crash_child",
                "--nocapture",
            ])
            .env("CPAR_M1_CRASH_DB", &file)
            .env("CPAR_M1_CRASH_BOUNDARY", boundary.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while !signal.exists() && Instant::now() < deadline {
            if let Some(status) = child.try_wait()? {
                return Err(format!("child exited before commit: {status}").into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let committed = signal.exists();
        child.kill()?;
        let _ = child.wait()?;
        assert!(
            committed,
            "child did not acknowledge the selected commit boundary"
        );
        let mut store = SqliteEventStore::open(&file)?;
        assert_eq!(store.list_events()?.len(), boundary);
        assert_eq!(
            store.recover_interrupted_requests()?,
            u64::from(boundary < 4)
        );
        assert_eq!(
            store.recover_interrupted_requests()?,
            u64::from(boundary < 4)
        );
        assert_eq!(store.append_batch(&events()?[..boundary])?, 0);
        assert_eq!(store.list_events()?.len(), boundary);
        let state: String =
            store
                .connection
                .query_row("SELECT state FROM gateway_request_recording", [], |r| {
                    r.get(0)
                })?;
        assert_eq!(
            state,
            if boundary < 4 {
                "interrupted"
            } else {
                "finished"
            }
        );
        drop(store);
        fs::remove_file(file)?;
        fs::remove_file(signal)?;
    }
    Ok(())
}

/// Manual run uses an owner-readable, metadata-only production projection, never credentials.
#[test]
#[ignore = "requires CPAR_M1_HISTORY_PROJECTION owner-readable input"]
fn historical_production_projection_survives_migration_and_partial_read() -> TestResult {
    let source = std::env::var("CPAR_M1_HISTORY_PROJECTION")?;
    let value: serde_json::Value = serde_json::from_slice(&fs::read(source)?)?;
    let mut store = SqliteEventStore::open_in_memory()?;
    crate::rollback_to_version(&mut store.connection, 28)?;
    let tables = [
        "gateway_event_log",
        "billing_price_catalog_versions",
        "billing_price_catalog_entries",
        "billing_ledger_entries",
        "billing_materializer_checkpoints",
        "billing_materializer_failures",
    ];
    import_history_projection(&mut store.connection, &value, &tables)?;
    let fingerprint = |c: &Connection| -> StoreResult<Vec<String>> {
        tables
            .iter()
            .map(|table| {
                let mut statement = c.prepare(&format!("SELECT * FROM {table} ORDER BY 1,2"))?;
                let cols = statement.column_count();
                let mut rows = statement.query([])?;
                let mut hash = Sha256::new();
                while let Some(row) = rows.next()? {
                    for col in 0..cols {
                        hash.update(format!("{:?};", row.get_ref(col)?).as_bytes());
                    }
                }
                Ok(format!("{:x}", hash.finalize()))
            })
            .collect()
    };
    let before = fingerprint(&store.connection)?;
    crate::migrate(&mut store.connection)?;
    assert_eq!(crate::schema_version(&store.connection)?, Some(29));
    assert_eq!(store.recover_interrupted_requests()?, 0); // old observations are not invented lifecycles
    let count: i64 =
        store
            .connection
            .query_row("SELECT count(*) FROM gateway_event_log", [], |r| r.get(0))?;
    assert_eq!(
        usize::try_from(count)?,
        value["gateway_event_log"]["rows"]
            .as_array()
            .ok_or("rows")?
            .len()
    );
    assert!(
        store
            .visit_usage_lineages(&UsageEventQuery::default(), |_, _, _| Ok(true))
            .is_err()
    );
    let mut verified = 0;
    let coverage = store.visit_usage_lineages(
        &UsageEventQuery {
            allow_partial: true,
            ..UsageEventQuery::default()
        },
        |_, _, _| {
            verified += 1;
            Ok(true)
        },
    )?;
    assert_eq!(coverage.excluded_usage_events, 11);
    assert_eq!(coverage.excluded_request_groups, 4);
    assert_eq!(verified, 699);
    assert_eq!(fingerprint(&store.connection)?, before);
    crate::rollback_to_version(&mut store.connection, 28)?;
    assert_eq!(fingerprint(&store.connection)?, before);
    crate::migrate(&mut store.connection)?;
    assert_eq!(fingerprint(&store.connection)?, before);
    eprintln!(
        "M1 historical projection: schema 28 -> 29 -> 28 -> 29; {count} events retained; 699 verified usages; 11 excluded / 4 groups; six source tables byte-value fingerprints unchanged"
    );
    Ok(())
}

fn import_history_projection(
    connection: &mut Connection,
    value: &serde_json::Value,
    tables: &[&str],
) -> TestResult {
    for table in tables {
        let columns = value[table]["columns"]
            .as_array()
            .ok_or("missing columns")?;
        let columns = columns
            .iter()
            .map(|v| v.as_str().ok_or("invalid column"))
            .collect::<Result<Vec<_>, _>>()?;
        if columns
            .iter()
            .any(|v| !v.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
        {
            return Err("unsafe column".into());
        }
        let insert = format!(
            "INSERT INTO {table}({}) VALUES ({})",
            columns.join(","),
            vec!["?"; columns.len()].join(",")
        );
        let tx = connection.transaction()?;
        for row in value[table]["rows"].as_array().ok_or("missing rows")? {
            let cells = row
                .as_array()
                .ok_or("invalid row")?
                .iter()
                .map(|v| match v {
                    serde_json::Value::Null => Ok(rusqlite::types::Value::Null),
                    serde_json::Value::String(v) => Ok(rusqlite::types::Value::Text(v.clone())),
                    serde_json::Value::Number(v) => v
                        .as_i64()
                        .map(rusqlite::types::Value::Integer)
                        .ok_or("invalid integer"),
                    _ => Err("invalid value"),
                })
                .collect::<Result<Vec<_>, _>>()?;
            tx.execute(&insert, rusqlite::params_from_iter(cells))?;
        }
        tx.commit()?;
    }
    Ok(())
}
