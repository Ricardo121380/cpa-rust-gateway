//! Claude v2 shares the existing queue, encryption, reader and recovery boundary with unchanged C4.
use super::*;

const M4: &str = r#"{ "user_id" : "{ \"device_id\":\"PRIVATE_M4_DEVICE\", \"session_id\":\"PRIVATE_M4_SESSION\" }\u9e1f" }"#;

fn request(id: &str) -> Result<RequestEvent, Box<dyn Error>> {
    Ok(RequestEvent::new(
        RequestId::try_new(id)?,
        ClientKeyId::try_new("owner")?,
        None,
        GatewayProtocol::AnthropicMessages,
        "grok-4.5".into(),
        "grok-4.5".into(),
        None,
        false,
    )
    .with_started_at_ms(1000))
}
fn evidence() -> Result<RequestMetadataEvidence, GatewayError> {
    RequestMetadataEvidence::try_new_anthropic(M4.into())
}

#[tokio::test]
#[ignore = "explicit synthetic fixture export for #66 stock Linux readers"]
async fn cpar66_export_metadata_fixture() -> TestResult {
    let dir = Directory::new()?;
    let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
    let mut expected = Vec::new();
    for version in [1, 2] {
        let req = if version == 1 {
            super::request("cpar66-v1", "owner")?
        } else {
            request("cpar66-v2")?
        };
        let value = if version == 1 {
            super::evidence()?
        } else {
            evidence()?
        };
        assert_eq!(
            queue
                .emit_request_metadata_confirmed(req.clone(), value)
                .await,
            EventEmission::Persisted
        );
        let attempt = attempt(&req, 1)?;
        assert_eq!(
            queue
                .emit_confirmed(GatewayEvent::Attempt(attempt.clone()))
                .await,
            EventEmission::Persisted
        );
        expected.push(
            json!({"command":"request-metadata-evidence", "version":version,
            "owner":"owner", "request":req.request_id(), "attempt":attempt.attempt_id(),
            "raw":if version == 1 { RAW } else { M4 }}),
        );
    }
    drop(queue);
    assert_eq!(task.await?.required_events_quarantined, 0);
    cpar66_export::export(&dir.db(), "metadata", &json!(expected))
}
fn metadata_rows(dir: &Directory) -> Result<Vec<Value>, Box<dyn Error>> {
    let db = gateway_store::open(dir.db())?;
    let mut statement=db.prepare("SELECT request_id,client_key_id,created_at_ms,expires_at_ms,payload_version,key_version,ciphertext FROM request_metadata_evidence ORDER BY request_id")?;
    let rows = statement
        .query_map([], |row| {
            Ok(json!([
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<Vec<u8>>>(6)?
            ]))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "one coherent v1/v2 history proves migration, ownership, expiry, keys and restore without replacing evidence"
)]
async fn messages_metadata_v2_owner_recovery_and_legacy_coexistence() -> TestResult {
    let dir = Directory::new()?;
    let legacy = super::request("old-live", "owner")?;
    let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(legacy.clone(), super::evidence()?)
            .await,
        EventEmission::Persisted
    );
    let old_receipt = super::request("old-receipt", "owner")?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(old_receipt, super::evidence()?)
            .await,
        EventEmission::Persisted
    );
    drop(queue);
    task.await?;
    let mut connection = gateway_store::open(dir.db())?;
    connection.execute(
        "UPDATE request_metadata_evidence SET ciphertext=NULL WHERE request_id='old-receipt'",
        [],
    )?;
    gateway_store::rollback_to_version(&mut connection, 37)?;
    let before = metadata_rows(&dir)?;
    gateway_store::migrate(&mut connection)?;
    assert_eq!(
        metadata_rows(&dir)?,
        before,
        "v1 ciphertext and expired receipts survive migration exactly"
    );
    drop(connection);
    assert_eq!(
        read(&dir, &legacy, keys(1, &[1], false)?)?
            .ok_or("old v1")?
            .record
            .evidence,
        super::evidence()?
    );
    let req = request("messages-v2")?;
    let request_only = request("messages-request-only")?;
    let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
    for r in [&req, &request_only] {
        assert_eq!(
            queue
                .emit_request_metadata_confirmed(r.clone(), evidence()?)
                .await,
            EventEmission::Persisted
        );
    }
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(req.clone(), evidence()?)
            .await,
        EventEmission::Persisted
    );
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(
                req.clone(),
                RequestMetadataEvidence::try_new_anthropic(format!(" {M4}"))?
            )
            .await,
        EventEmission::PersistenceUnavailable
    );
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(
                super::request("wrong-protocol", "owner")?,
                evidence()?
            )
            .await,
        EventEmission::PersistenceUnavailable
    );
    for sequence in [1, 2] {
        assert_eq!(
            queue
                .emit_confirmed(GatewayEvent::Attempt(attempt(&req, sequence)?))
                .await,
            EventEmission::Persisted
        );
    }
    let record = read(&dir, &req, keys(1, &[1], false)?)?.ok_or("v2")?;
    assert_eq!(record.record.version, 2);
    assert_eq!(
        record.record.evidence.source(),
        RequestMetadataSource::AnthropicMessagesMetadata
    );
    assert_eq!(record.record.evidence.raw_json(), M4);
    assert_eq!(record.attempts.len(), 2);
    assert!(
        read(&dir, &request_only, keys(1, &[1], false)?)?
            .ok_or("Request-only")?
            .attempts
            .is_empty()
    );
    let reader = SqliteRequestMetadataReader::open(dir.db(), keys(1, &[1], false)?)?;
    assert!(
        reader
            .get_owned(
                &ClientKeyId::try_new("foreign")?,
                req.request_id(),
                None,
                now()?
            )?
            .is_none()
    );
    assert!(
        reader
            .get_owned(
                req.client_key_id(),
                request_only.request_id(),
                Some(record.attempts[0].attempt_id()),
                now()?
            )?
            .is_none()
    );
    for a in &record.attempts {
        assert!(
            reader
                .get_owned(
                    req.client_key_id(),
                    req.request_id(),
                    Some(a.attempt_id()),
                    now()?
                )?
                .is_some()
        );
    }
    drop(reader);
    drop(queue);
    assert_eq!(task.await?.required_events_quarantined, 2);
    let connection = gateway_store::open(dir.db())?;
    let expires: i64 = connection.query_row(
        "SELECT expires_at_ms FROM request_metadata_evidence WHERE request_id=?1",
        [req.request_id().as_str()],
        |r| r.get(0),
    )?;
    let reader = SqliteRequestMetadataReader::open(dir.db(), keys(1, &[1], false)?)?;
    assert!(
        reader
            .get_owned(req.client_key_id(), req.request_id(), None, expires - 1)?
            .is_some()
    );
    assert!(
        reader
            .get_owned(req.client_key_id(), req.request_id(), None, expires)?
            .is_none()
    );
    purge_expired(&connection, expires)?;
    drop(reader);
    drop(connection);
    let rotated = request("messages-key2")?;
    let (queue, task) = writer(&dir, keys(2, &[1, 2], false)?).await?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(req.clone(), evidence()?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(rotated.clone(), evidence()?)
            .await,
        EventEmission::Persisted
    );
    assert_eq!(
        read(&dir, &rotated, keys(2, &[1, 2], false)?)?
            .ok_or("new key")?
            .record
            .evidence
            .raw_json(),
        M4
    );
    assert!(read(&dir, &rotated, keys(1, &[1], false)?).is_err());
    assert!(read(&dir, &rotated, keys(2, &[2], true)?).is_err());
    drop(queue);
    assert_eq!(task.await?.required_events_quarantined, 1);
    let before = metadata_rows(&dir)?;
    let mut connection = gateway_store::open(dir.db())?;
    assert!(gateway_store::rollback_to_version(&mut connection, 37).is_err());
    assert_eq!(
        metadata_rows(&dir)?,
        before,
        "down migration cannot erase v2 receipts"
    );
    drop(connection);
    let backup = BackupKey::try_from_bytes([0xba; 32])?;
    let artifact = create_encrypted_backup(&gateway_store::open(dir.db())?, &dir.0, &backup)?;
    let restored = Directory::new()?;
    restore_encrypted_backup_to_empty_target(&artifact, restored.db(), &backup)?;
    assert_eq!(metadata_rows(&restored)?, before);
    assert!(
        SqliteRequestMetadataReader::open(restored.db(), keys(2, &[1, 2], false)?)?
            .get_owned(req.client_key_id(), req.request_id(), None, expires)?
            .is_none()
    );
    assert_eq!(
        read(&restored, &rotated, keys(2, &[1, 2], false)?)?
            .ok_or("restored authorized v2")?
            .record
            .evidence
            .raw_json(),
        M4
    );
    let (queue, task) = writer(&restored, keys(2, &[1, 2], false)?).await?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(req, evidence()?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(rotated, evidence()?)
            .await,
        EventEmission::Persisted
    );
    drop(queue);
    assert_eq!(task.await?.required_events_quarantined, 1);
    println!(
        "CPAR64_M4_RECOVERY {}",
        json!({"v1_migration_bytes_preserved":true,"v2_owner_attempts":2,
        "request_only":true,"rotation":true,"exclusive_expiry":true,"receipt_restore_replay_rejected":true})
    );
    Ok(())
}

#[tokio::test]
async fn messages_metadata_v2_aad_source_protocol_and_index_corruption_fail_closed() -> TestResult {
    for fault in [
        "payload-version",
        "ciphertext-swap",
        "request-protocol",
        "source-conflict",
    ] {
        let dir = Directory::new()?;
        let req = request("messages")?;
        let (queue, task) = writer(&dir, keys(1, &[1], false)?).await?;
        assert_eq!(
            queue
                .emit_request_metadata_confirmed(req.clone(), evidence()?)
                .await,
            EventEmission::Persisted
        );
        let legacy = super::request("codex", "owner")?;
        assert_eq!(
            queue
                .emit_request_metadata_confirmed(legacy, super::evidence()?)
                .await,
            EventEmission::Persisted
        );
        drop(queue);
        task.await?;
        let connection = gateway_store::open(dir.db())?;
        match fault {
            "payload-version" => {
                connection.execute("UPDATE request_metadata_evidence SET payload_version=1 WHERE request_id='messages'",[])?;
            }
            "ciphertext-swap" => {
                connection.execute("UPDATE request_metadata_evidence SET ciphertext=(SELECT ciphertext FROM request_metadata_evidence WHERE request_id='codex') WHERE request_id='messages'",[])?;
            }
            "request-protocol" => {
                connection.execute_batch("DROP TRIGGER gateway_event_log_no_update")?;
                let mut event = serde_json::to_value(GatewayEvent::Request(req.clone()))?;
                event["request"]["protocol"] =
                    serde_json::to_value(GatewayProtocol::OpenAiResponses)?;
                connection.execute("UPDATE gateway_event_log SET payload_json=?1 WHERE event_type='request' AND event_id='messages'",[event.to_string()])?;
            }
            _ => {
                let bad: RequestMetadataEvidence = serde_json::from_value(
                    json!({"raw_json":M4,"source":"codex_client_metadata"}),
                )?;
                assert!(bad.validate().is_err());
                continue;
            }
        }
        assert!(read(&dir, &req, keys(1, &[1], false)?).is_err(), "{fault}");
    }
    Ok(())
}

#[tokio::test]
async fn messages_metadata_v2_finite_queue_and_timeout_are_unconfirmed() -> TestResult {
    let dir = Directory::new()?;
    let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(request("no-writer")?, evidence()?)
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
            .emit_request_metadata_confirmed(request("timeout")?, evidence()?)
            .await,
        EventEmission::PersistenceUnavailable
    );
    assert_eq!(
        queue
            .emit_request_metadata_confirmed(request("queue-full")?, evidence()?)
            .await,
        EventEmission::RequiredQueueFull
    );
    drop(unused);
    assert!(!dir.db().exists());
    Ok(())
}
