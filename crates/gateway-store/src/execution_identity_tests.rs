//! Atomic, additive evidence storage and explicit legacy/rollback behavior.
use super::*;
use gateway_core::{AttemptExecutionIdentity, EffectiveCapabilities, ExecutionEgress};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn execution_identity_noncanonical_json_and_overflow_are_invalid() -> TestResult {
    let mut store = SqliteEventStore::open_in_memory()?;
    store.append_batch(&[observed_attempt(3)?])?;
    let canonical: String = store.connection.query_row(
        "SELECT evidence_json FROM gateway_attempt_execution",
        [],
        |row| row.get(0),
    )?;
    store
        .connection
        .execute_batch("DROP TRIGGER gateway_attempt_execution_no_update;")?;
    for invalid in [
        canonical.replacen(
            "\"credential_revision\":3",
            "\"credential_revision\":99,\"credential_revision\":3",
            1,
        ),
        canonical.replacen(
            "\"credential_revision\":3",
            "\"credential_revision\":18446744073709551616",
            1,
        ),
        format!(" {canonical}"),
    ] {
        store.connection.execute(
            "UPDATE gateway_attempt_execution SET evidence_json=?1",
            [invalid],
        )?;
        assert!(matches!(
            store.list_events(),
            Err(StoreError::InvalidPersistedGatewayEvent)
        ));
    }
    Ok(())
}

fn identity(revision: u64) -> AttemptExecutionIdentity {
    AttemptExecutionIdentity {
        channel: "openai-compatible".to_owned(),
        credential_revision: revision,
        config_version_id: "execution-v1".to_owned(),
        config_revision: 7,
        egress: ExecutionEgress::ConfiguredProxy {
            target_id: "pool-v1".to_owned(),
            node_id: "node-v1".to_owned(),
            config_version_id: "execution-v1".to_owned(),
            config_revision: 7,
        },
        capabilities: EffectiveCapabilities {
            reasoning: true,
            parallel: true,
            stored: false,
            continuation: false,
            compact: false,
            websocket: false,
        },
    }
}

fn observed_attempt(revision: u64) -> Result<GatewayEvent, Box<dyn std::error::Error>> {
    let GatewayEvent::Attempt(attempt) = tests::attempt_event(1)? else {
        return Err("fixture is not attempt".into());
    };
    Ok(GatewayEvent::Attempt(
        attempt.with_execution_identity(identity(revision)),
    ))
}

#[test]
fn execution_identity_is_atomic_bound_and_replay_exact() -> TestResult {
    let mut store = SqliteEventStore::open_in_memory()?;
    let event = observed_attempt(3)?;
    store.connection.execute_batch("CREATE TRIGGER execution_test_abort BEFORE INSERT ON gateway_attempt_execution BEGIN SELECT RAISE(ABORT,'fixture rollback'); END;")?;
    assert!(store.append_batch(std::slice::from_ref(&event)).is_err());
    assert!(store.list_events()?.is_empty());
    store
        .connection
        .execute_batch("DROP TRIGGER execution_test_abort;")?;
    assert_eq!(store.append_batch(std::slice::from_ref(&event))?, 1);
    assert_eq!(store.append_batch(std::slice::from_ref(&event))?, 0);
    let events = store.list_events()?;
    assert_eq!(events[0].event(), &event);
    let raw: String =
        store
            .connection
            .query_row("SELECT payload_json FROM gateway_event_log", [], |row| {
                row.get(0)
            })?;
    assert_eq!(raw, serde_json::to_string(&tests::attempt_event(1)?)?);
    // The pre-34 strict decoder still accepts the unmodified Attempt bytes.
    let legacy: GatewayEvent = serde_json::from_str(&raw)?;
    let GatewayEvent::Attempt(legacy) = legacy else {
        return Err("not attempt".into());
    };
    assert!(legacy.execution_identity().is_none());
    assert!(matches!(
        store.append_batch(&[observed_attempt(4)?]),
        Err(StoreError::ConflictingGatewayEventReplay)
    ));
    assert!(
        store
            .connection
            .execute(
                "UPDATE gateway_attempt_execution SET request_id='changed'",
                []
            )
            .is_err()
    );
    assert!(
        store
            .connection
            .execute("DELETE FROM gateway_attempt_execution", [])
            .is_err()
    );
    Ok(())
}

#[test]
fn execution_identity_legacy_unknown_and_schema_rollback_preserve_attempts() -> TestResult {
    let mut store = SqliteEventStore::open_in_memory()?;
    let legacy = tests::attempt_event(1)?;
    store.append_batch(std::slice::from_ref(&legacy))?;
    assert!(matches!(
        store.append_batch(&[observed_attempt(3)?]),
        Err(StoreError::ConflictingGatewayEventReplay)
    ));
    assert_eq!(store.list_events()?[0].event(), &legacy);
    crate::rollback_to_version(&mut store.connection, 33)?;
    assert_eq!(store.list_events()?[0].event(), &legacy);
    crate::migrate(&mut store.connection)?;
    assert_eq!(store.list_events()?[0].event(), &legacy);
    assert!(matches!(
        store.append_batch(&[observed_attempt(3)?]),
        Err(StoreError::ConflictingGatewayEventReplay)
    ));
    // A state rollback keeps original Attempt/Usage sources. New execution evidence is
    // deliberately lost by down migration, so deployment rollback requires a full backup.
    let observed = match tests::attempt_event(2)? {
        GatewayEvent::Attempt(attempt) => {
            GatewayEvent::Attempt(attempt.with_execution_identity(identity(8)))
        }
        _ => return Err("fixture".into()),
    };
    store.append_batch(&[observed])?;
    crate::rollback_to_version(&mut store.connection, 33)?;
    crate::migrate(&mut store.connection)?;
    for event in store.list_events()? {
        let GatewayEvent::Attempt(attempt) = event.event() else {
            return Err("fixture".into());
        };
        assert!(attempt.execution_identity().is_none());
    }
    Ok(())
}

#[test]
fn execution_identity_invalid_revision_and_corrupt_binding_fail_closed() -> TestResult {
    let mut store = SqliteEventStore::open_in_memory()?;
    let mut invalid = identity(3);
    invalid.config_revision = 8; // The selected egress belongs to revision 7.
    let GatewayEvent::Attempt(attempt) = tests::attempt_event(1)? else {
        return Err("fixture".into());
    };
    assert!(matches!(
        store.append_batch(&[GatewayEvent::Attempt(
            attempt.with_execution_identity(invalid)
        )]),
        Err(StoreError::InvalidPersistedGatewayEvent)
    ));
    assert!(store.list_events()?.is_empty());
    let valid = observed_attempt(3)?;
    store.append_batch(std::slice::from_ref(&valid))?;
    store
        .connection
        .execute_batch("DROP TRIGGER gateway_attempt_execution_no_update;")?;
    store.connection.execute(
        "UPDATE gateway_attempt_execution SET attempt_payload_sha256=?1",
        ["0".repeat(64)],
    )?;
    assert!(matches!(
        store.list_events(),
        Err(StoreError::InvalidPersistedGatewayEvent)
    ));
    assert!(matches!(
        store.append_batch(&[valid]),
        Err(StoreError::InvalidPersistedGatewayEvent)
    ));
    Ok(())
}
