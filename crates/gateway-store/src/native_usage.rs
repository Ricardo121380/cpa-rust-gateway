//! AEAD-sealed, exact-owner native Usage. Ordinary events contain no private companion.
use crate::{
    CURRENT_SCHEMA_VERSION, StoreError, StoreResult, schema_version,
    secret_store::{EncryptedSecret, KeyVersion, SecretStore},
};
use gateway_core::{
    AttemptId, ClientKeyId, GatewayEvent, NativeUsageEvidence, RequestId, UsageEvent,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

/// Fixed retention of the received Usage object, independent of event/ledger retention.
pub const NATIVE_USAGE_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Maximum expired rows removed in one transaction.
pub const NATIVE_USAGE_GC_BATCH: i64 = 128;

/// Authenticated diagnostic result. Serialize only to an explicitly authorized private output.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeUsageRecord {
    /// Explicit encrypted record format version.
    pub version: u16,
    /// Exact admitted Client Key owner, not its Access Group.
    pub client_key_id: ClientKeyId,
    /// Stable source-event identifier shared with billing.
    pub source_event_id: String,
    /// Exact Request/Attempt/response and verified counters.
    pub usage: UsageEvent,
    /// Completeness describes this received object only; never unread upstream or a bill.
    pub completeness: String,
    /// Complete received native object and decoder source.
    pub evidence: NativeUsageEvidence,
}
impl fmt::Debug for NativeUsageRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativeUsageRecord(<private>)")
    }
}

struct Row {
    request: String,
    owner: String,
    source: String,
    created: i64,
    expires: i64,
    version: i64,
    key_version: i64,
    ciphertext: Vec<u8>,
}
fn load(connection: &Connection, attempt: &str) -> StoreResult<Option<Row>> {
    connection.query_row("SELECT request_id,client_key_id,source_event_id,created_at_ms,expires_at_ms,payload_version,key_version,ciphertext FROM native_usage_evidence WHERE attempt_id=?1", [attempt], |r| Ok(Row {request:r.get(0)?,owner:r.get(1)?,source:r.get(2)?,created:r.get(3)?,expires:r.get(4)?,version:r.get(5)?,key_version:r.get(6)?,ciphertext:r.get(7)?})).optional().map_err(StoreError::from)
}
fn invalid() -> StoreError {
    StoreError::InvalidPersistedGatewayEvent
}
fn aad(attempt: &str, row: &Row) -> StoreResult<Vec<u8>> {
    if row.version != 1
        || row.created < 0
        || row.created.checked_add(NATIVE_USAGE_TTL_MS) != Some(row.expires)
    {
        return Err(invalid());
    }
    let mut aad = b"cpar:native-usage:v1\0".to_vec();
    for value in [&row.owner, &row.request, &attempt.to_owned(), &row.source] {
        let length = u32::try_from(value.len()).map_err(|_| invalid())?;
        aad.extend_from_slice(&length.to_be_bytes());
        aad.extend_from_slice(value.as_bytes());
    }
    for value in [row.version, row.created, row.expires] {
        aad.extend_from_slice(&value.to_be_bytes());
    }
    Ok(aad)
}
fn open_row(secrets: &SecretStore, attempt: &str, row: &Row) -> StoreResult<NativeUsageRecord> {
    if row.ciphertext.len() > gateway_core::MAX_NATIVE_USAGE_BYTES * 8 {
        return Err(invalid());
    }
    let encrypted = EncryptedSecret::try_from_persisted(
        KeyVersion::try_from_sqlite_i64(row.key_version).map_err(|_| invalid())?,
        row.ciphertext.clone(),
    )
    .map_err(|_| invalid())?;
    let plaintext = secrets
        .open(&encrypted, &aad(attempt, row)?)
        .map_err(|_| invalid())?;
    let record: NativeUsageRecord =
        serde_json::from_slice(plaintext.as_bytes()).map_err(|_| invalid())?;
    if record.version != 1
        || record.client_key_id.as_str() != row.owner
        || record.source_event_id != row.source
        || record.usage.request_id().as_str() != row.request
        || record.usage.attempt_id().map(AttemptId::as_str) != Some(attempt)
        || record.completeness != "complete_usage_object"
    {
        return Err(invalid());
    }
    NativeUsageEvidence::try_new(
        record.evidence.source(),
        record.evidence.raw_json().to_owned(),
    )
    .map_err(|_| invalid())?;
    Ok(record)
}

/// Read-only diagnostic access; no migration, routing, Provider request or key rotation.
pub struct SqliteNativeUsageReader {
    connection: Connection,
    secrets: SecretStore,
    schema: i64,
}
impl SqliteNativeUsageReader {
    /// Opens only a supported, already-existing database. Legacy schema-34 evidence is UNKNOWN.
    ///
    /// # Errors
    /// Fails closed on unsupported schema, inaccessible storage or authentication failure.
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
    /// Authenticates exact owner, Request, Attempt and lifecycle before exposing any plaintext.
    /// Missing, foreign, expired and historical evidence all return the same UNKNOWN (`None`).
    ///
    /// # Errors
    /// Returns a value-free error on corrupted metadata, wrong/missing keys or ciphertext.
    pub fn get_owned(
        &self,
        owner: &ClientKeyId,
        request: &RequestId,
        attempt: &AttemptId,
        now_ms: i64,
    ) -> StoreResult<Option<NativeUsageRecord>> {
        for id in [owner.as_str(), request.as_str(), attempt.as_str()] {
            if id.is_empty() || id.len() > 512 || id.as_bytes().contains(&0) {
                return Err(invalid());
            }
        }
        if now_ms < 0 {
            return Err(invalid());
        }
        if self.schema < 35 {
            return Ok(None);
        }
        let Some(row) = load(&self.connection, attempt.as_str())? else {
            return Ok(None);
        };
        if row.owner != owner.as_str() || row.request != request.as_str() || now_ms >= row.expires {
            return Ok(None);
        }
        let record = open_row(&self.secrets, attempt.as_str(), &row)?;
        verify_lineage(&self.connection, &record)?;
        Ok(Some(record))
    }
}

fn read_event(connection: &Connection, kind: &str, id: &str) -> StoreResult<GatewayEvent> {
    let row = connection
        .query_row(
            "SELECT event_ordinal,event_type,event_id,request_id,occurred_at_ms,payload_json \
             FROM gateway_event_log WHERE event_type=?1 AND event_id=?2",
            params![kind, id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get::<_, String>(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(invalid)?;
    let stored =
        crate::event_store::decode_stored_event(row.0, &row.1, row.2, row.3, row.4, &row.5)?;
    Ok(stored.event().clone())
}

fn verify_lineage(connection: &Connection, record: &NativeUsageRecord) -> StoreResult<()> {
    let attempt_id = record.usage.attempt_id().ok_or_else(invalid)?;
    let (
        GatewayEvent::Request(request),
        GatewayEvent::Attempt(attempt),
        GatewayEvent::Usage(usage),
    ) = (
        read_event(connection, "request", record.usage.request_id().as_str())?,
        read_event(connection, "attempt", attempt_id.as_str())?,
        read_event(connection, "usage", &record.source_event_id)?,
    )
    else {
        return Err(invalid());
    };
    if request.client_key_id() != &record.client_key_id
        || request.request_id() != record.usage.request_id()
        || attempt.request_id() != request.request_id()
        || attempt.attempt_id() != attempt_id
        || usage != record.usage
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn verify_usage_binding(
    connection: &Connection,
    usage: &UsageEvent,
) -> StoreResult<ClientKeyId> {
    let id = usage.attempt_id().ok_or_else(invalid)?;
    let (GatewayEvent::Request(request), GatewayEvent::Attempt(attempt)) = (
        read_event(connection, "request", usage.request_id().as_str())?,
        read_event(connection, "attempt", id.as_str())?,
    ) else {
        return Err(invalid());
    };
    if request.request_id() != usage.request_id()
        || attempt.request_id() != usage.request_id()
        || attempt.attempt_id() != id
    {
        return Err(invalid());
    }
    Ok(request.client_key_id().clone())
}

pub(crate) fn now_ms() -> StoreResult<i64> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid())?
            .as_millis(),
    )
    .map_err(|_| invalid())
}

pub(crate) fn append(
    transaction: &Transaction<'_>,
    usage: &UsageEvent,
    source: &str,
    evidence: &NativeUsageEvidence,
    secrets: &SecretStore,
    source_existed: bool,
) -> StoreResult<()> {
    let attempt = usage.attempt_id().ok_or_else(invalid)?;
    let owner = verify_usage_binding(transaction, usage)?;
    let record = NativeUsageRecord {
        version: 1,
        client_key_id: owner.clone(),
        source_event_id: source.to_owned(),
        usage: usage.clone(),
        completeness: "complete_usage_object".into(),
        evidence: evidence.clone(),
    };
    verify_lineage(transaction, &record)?;
    if let Some(row) = load(transaction, attempt.as_str())? {
        if now_ms()? >= row.expires || open_row(secrets, attempt.as_str(), &row)? != record {
            return Err(StoreError::ConflictingGatewayEventReplay);
        }
        return Ok(());
    }
    // An older sanitized observation never acquires invented raw evidence by replay.
    if source_existed {
        return Err(StoreError::ConflictingGatewayEventReplay);
    }
    let created = now_ms()?;
    let row = Row {
        request: usage.request_id().as_str().into(),
        owner: owner.as_str().to_owned(),
        source: source.into(),
        created,
        expires: created
            .checked_add(NATIVE_USAGE_TTL_MS)
            .ok_or_else(invalid)?,
        version: 1,
        key_version: secrets.active_key_version().as_sqlite_i64(),
        ciphertext: Vec::new(),
    };
    let plaintext = Zeroizing::new(serde_json::to_vec(&record).map_err(|_| invalid())?);
    let encrypted = secrets
        .seal(plaintext.as_slice(), &aad(attempt.as_str(), &row)?)
        .map_err(|_| invalid())?;
    transaction.execute("INSERT INTO native_usage_evidence(attempt_id,request_id,client_key_id,source_event_id,created_at_ms,expires_at_ms,payload_version,key_version,ciphertext) VALUES (?1,?2,?3,?4,?5,?6,1,?7,?8)",params![attempt.as_str(),row.request,row.owner,row.source,row.created,row.expires,encrypted.key_version().as_sqlite_i64(),encrypted.ciphertext()])?;
    Ok(())
}

/// Removes at most the fixed GC bound; expiry is enforced even before physical deletion.
///
/// # Errors
/// Returns a storage error without changing ordinary Usage events or ledger rows.
pub fn purge_expired(connection: &Connection, now_ms: i64) -> StoreResult<usize> {
    if now_ms < 0 {
        return Err(invalid());
    }
    connection.execute("DELETE FROM native_usage_evidence WHERE attempt_id IN (SELECT attempt_id FROM native_usage_evidence WHERE expires_at_ms<=?1 ORDER BY expires_at_ms LIMIT ?2)",params![now_ms,NATIVE_USAGE_GC_BATCH]).map_err(StoreError::from)
}
