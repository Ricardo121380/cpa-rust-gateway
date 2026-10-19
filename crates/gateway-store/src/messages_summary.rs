//! Independent encrypted summary records. No Usage or billing evidence is manufactured here.
use crate::{
    CURRENT_SCHEMA_VERSION, StoreError, StoreResult,
    native_usage::{now_ms, read_event},
    schema_version,
    secret_store::{EncryptedSecret, KeyVersion, SecretStore},
};
use gateway_core::{
    AttemptEvent, AttemptExecutionIdentity, AttemptId, ClientKeyId, GatewayEvent, GatewayProtocol,
    MessagesSummaryEvidence, MessagesSummarySource, RequestId,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, path::Path, time::Duration};
use zeroize::Zeroizing;

/// Exclusive 30-day retention, independent of lifecycle events and billing.
pub const MESSAGES_SUMMARY_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Authenticated private diagnostic result; serialize only to authorized private output.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MessagesSummaryRecord {
    /// Encrypted payload version.
    pub version: u16,
    /// Exact logical Client Key owner.
    pub client_key_id: ClientKeyId,
    /// Original durable Attempt, including Request and selected model identity.
    pub attempt: AttemptEvent,
    /// Actual original Build execution facts, authenticated against the durable source.
    pub execution: AttemptExecutionIdentity,
    /// Confirmed item/part hierarchy, order, source and completeness.
    pub evidence: MessagesSummaryEvidence,
}
impl fmt::Debug for MessagesSummaryRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MessagesSummaryRecord(<private>)")
    }
}
struct Row {
    request: String,
    owner: String,
    created: i64,
    expires: i64,
    version: i64,
    key: i64,
    ciphertext: Option<Vec<u8>>,
}
fn invalid() -> StoreError {
    StoreError::InvalidPersistedGatewayEvent
}
fn load(connection: &Connection, attempt: &str) -> StoreResult<Option<Row>> {
    connection.query_row("SELECT request_id,client_key_id,created_at_ms,expires_at_ms,payload_version,key_version,ciphertext FROM messages_summary_evidence WHERE attempt_id=?1",[attempt],|r|Ok(Row {request:r.get(0)?,owner:r.get(1)?,created:r.get(2)?,expires:r.get(3)?,version:r.get(4)?,key:r.get(5)?,ciphertext:r.get(6)?})).optional().map_err(StoreError::from)
}
fn aad(attempt: &str, row: &Row) -> StoreResult<Vec<u8>> {
    if row.version != 1
        || row.created < 0
        || row.created.checked_add(MESSAGES_SUMMARY_TTL_MS) != Some(row.expires)
    {
        return Err(invalid());
    }
    let mut bytes = b"cpar:messages-summary:v1\0".to_vec();
    for value in [row.owner.as_str(), row.request.as_str(), attempt] {
        bytes.extend_from_slice(
            &u32::try_from(value.len())
                .map_err(|_| invalid())?
                .to_be_bytes(),
        );
        bytes.extend_from_slice(value.as_bytes());
    }
    for value in [row.version, row.created, row.expires] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    Ok(bytes)
}
fn open_row(secrets: &SecretStore, attempt: &str, row: &Row) -> StoreResult<MessagesSummaryRecord> {
    let ciphertext = row.ciphertext.as_ref().ok_or_else(invalid)?;
    if ciphertext.len() > gateway_core::MAX_MESSAGES_SUMMARY_BYTES * 8 {
        return Err(invalid());
    }
    let encrypted = EncryptedSecret::try_from_persisted(
        KeyVersion::try_from_sqlite_i64(row.key).map_err(|_| invalid())?,
        ciphertext.clone(),
    )
    .map_err(|_| invalid())?;
    let plain = secrets
        .open(&encrypted, &aad(attempt, row)?)
        .map_err(|_| invalid())?;
    let record: MessagesSummaryRecord =
        serde_json::from_slice(plain.as_bytes()).map_err(|_| invalid())?;
    if record.version != 1
        || record.client_key_id.as_str() != row.owner
        || record.attempt.request_id().as_str() != row.request
        || record.attempt.attempt_id().as_str() != attempt
    {
        return Err(invalid());
    }
    MessagesSummaryEvidence::try_new(
        record.evidence.source(),
        record.evidence.response_id().clone(),
        record.evidence.completeness(),
        record.evidence.confirmed_at_ms(),
        record.evidence.items().to_vec(),
    )
    .map_err(|_| invalid())?;
    Ok(record)
}
fn binding(
    connection: &Connection,
    attempt: &AttemptEvent,
    evidence: &MessagesSummaryEvidence,
) -> StoreResult<(ClientKeyId, AttemptEvent, AttemptExecutionIdentity)> {
    let (GatewayEvent::Request(request), GatewayEvent::Attempt(durable)) = (
        read_event(connection, "request", attempt.request_id().as_str())?,
        read_event(connection, "attempt", attempt.attempt_id().as_str())?,
    ) else {
        return Err(invalid());
    };
    let payload =
        serde_json::to_string(&GatewayEvent::Attempt(attempt.clone())).map_err(|_| invalid())?;
    if serde_json::to_string(&GatewayEvent::Attempt(durable.clone())).map_err(|_| invalid())?
        != payload
        || durable.request_id() != request.request_id()
        || request.protocol() != GatewayProtocol::AnthropicMessages
        || request.streaming() != (evidence.source() == MessagesSummarySource::BuildSse)
        || request
            .started_at_ms()
            .is_none_or(|start| evidence.confirmed_at_ms() < start)
    {
        return Err(invalid());
    }
    let finished:Option<GatewayEvent>=connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM gateway_event_log WHERE event_type='request_finished' AND event_id=?1)",
        [request.request_id().as_str()],|r|r.get::<_,bool>(0))?
        .then(||read_event(connection,"request_finished",request.request_id().as_str())).transpose()?;
    if let Some(GatewayEvent::RequestFinished(finished)) = finished
        && evidence.confirmed_at_ms() > finished.finished_at_ms
    {
        return Err(invalid());
    }
    let (request_id,checksum,json):(String,String,String)=connection.query_row(
        "SELECT request_id,attempt_payload_sha256,evidence_json FROM gateway_attempt_execution WHERE attempt_id=?1",
        [attempt.attempt_id().as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let execution: AttemptExecutionIdentity = serde_json::from_str(&json).map_err(|_| invalid())?;
    if request_id != request.request_id().as_str()
        || checksum != format!("{:x}", Sha256::digest(payload.as_bytes()))
        || !execution.is_valid()
        || execution.channel != "grok.build"
    {
        return Err(invalid());
    }
    Ok((request.client_key_id().clone(), durable, execution))
}
fn verify(connection: &Connection, record: &MessagesSummaryRecord) -> StoreResult<()> {
    let (owner, attempt, execution) = binding(connection, &record.attempt, &record.evidence)?;
    if owner != record.client_key_id || attempt != record.attempt || execution != record.execution {
        return Err(invalid());
    }
    Ok(())
}

/// Read-only exact-owner diagnostics. Older supported schemas remain UNKNOWN.
pub struct SqliteMessagesSummaryReader {
    connection: Connection,
    secrets: SecretStore,
    schema: i64,
}
impl SqliteMessagesSummaryReader {
    /// Opens an existing database without migration or Provider/network operations.
    /// # Errors
    /// Rejects unsupported schema or inaccessible storage.
    pub fn open(path: impl AsRef<Path>, secrets: SecretStore) -> StoreResult<Self> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        let schema = schema_version(&connection)?.ok_or_else(invalid)?;
        if !(34..=CURRENT_SCHEMA_VERSION).contains(&schema) {
            return Err(invalid());
        }
        Ok(Self {
            connection,
            secrets,
            schema,
        })
    }
    /// Authenticates exact owner, Request, Attempt, original execution and all event indexes.
    /// Missing, foreign, expired and historical companions return the same UNKNOWN.
    /// # Errors
    /// Fails closed on invalid identity, damaged lineage, missing/wrong keys or ciphertext.
    pub fn get_owned(
        &self,
        owner: &ClientKeyId,
        request: &RequestId,
        attempt: &AttemptId,
        now: i64,
    ) -> StoreResult<Option<MessagesSummaryRecord>> {
        if now < 0
            || [owner.as_str(), request.as_str(), attempt.as_str()]
                .iter()
                .any(|id| id.is_empty() || id.len() > 512 || id.contains('\0'))
        {
            return Err(invalid());
        }
        if self.schema < 36 {
            return Ok(None);
        }
        let Some(row) = load(&self.connection, attempt.as_str())? else {
            return Ok(None);
        };
        if row.owner != owner.as_str() || row.request != request.as_str() || now >= row.expires {
            return Ok(None);
        }
        let record = open_row(&self.secrets, attempt.as_str(), &row)?;
        verify(&self.connection, &record)?;
        Ok(Some(record))
    }
}

pub(crate) fn append(
    transaction: &Transaction<'_>,
    attempt: &AttemptEvent,
    evidence: &MessagesSummaryEvidence,
    secrets: &SecretStore,
) -> StoreResult<()> {
    MessagesSummaryEvidence::try_new(
        evidence.source(),
        evidence.response_id().clone(),
        evidence.completeness(),
        evidence.confirmed_at_ms(),
        evidence.items().to_vec(),
    )
    .map_err(|_| invalid())?;
    let (owner, durable, execution) = binding(transaction, attempt, evidence)?;
    let record = MessagesSummaryRecord {
        version: 1,
        client_key_id: owner.clone(),
        attempt: durable,
        execution,
        evidence: evidence.clone(),
    };
    if let Some(row) = load(transaction, attempt.attempt_id().as_str())? {
        if row.ciphertext.is_none()
            || now_ms()? >= row.expires
            || open_row(secrets, attempt.attempt_id().as_str(), &row)? != record
        {
            return Err(StoreError::ConflictingGatewayEventReplay);
        }
        return Ok(());
    }
    // The evidence's confirmation time must belong to the admitted Request lifetime (binding).
    // A queued receipt may finish after cancellation; newly captured historical data may not.
    let eligible:bool=transaction.query_row("SELECT EXISTS(SELECT 1 FROM gateway_request_recording WHERE request_id=?1 AND state IN ('active','finished'))",
        [attempt.request_id().as_str()],|r|r.get(0))?;
    if !eligible {
        return Err(invalid());
    }
    if evidence.confirmed_at_ms() > now_ms()? {
        return Err(invalid());
    }
    let created = now_ms()?;
    let row = Row {
        request: attempt.request_id().as_str().into(),
        owner: owner.as_str().into(),
        created,
        expires: created
            .checked_add(MESSAGES_SUMMARY_TTL_MS)
            .ok_or_else(invalid)?,
        version: 1,
        key: secrets.active_key_version().as_sqlite_i64(),
        ciphertext: None,
    };
    let plain = Zeroizing::new(serde_json::to_vec(&record).map_err(|_| invalid())?);
    let encrypted = secrets
        .seal(plain.as_slice(), &aad(attempt.attempt_id().as_str(), &row)?)
        .map_err(|_| invalid())?;
    transaction.execute("INSERT INTO messages_summary_evidence(attempt_id,request_id,client_key_id,created_at_ms,expires_at_ms,payload_version,key_version,ciphertext) VALUES (?1,?2,?3,?4,?5,1,?6,?7)",
        params![attempt.attempt_id().as_str(),row.request,row.owner,row.created,row.expires,encrypted.key_version().as_sqlite_i64(),encrypted.ciphertext()])?;
    Ok(())
}
pub(crate) fn purge_expired(connection: &Connection, now: i64) -> StoreResult<usize> {
    // Keep the non-payload Attempt receipt after erasing expired ciphertext. Deleting the
    // receipt would let an old confirmed companion be mistaken for its first persistence.
    connection.execute("UPDATE messages_summary_evidence SET ciphertext=NULL WHERE attempt_id IN (SELECT attempt_id FROM messages_summary_evidence WHERE expires_at_ms<=?1 AND ciphertext IS NOT NULL ORDER BY expires_at_ms LIMIT 128)",[now]).map_err(StoreError::from)
}
