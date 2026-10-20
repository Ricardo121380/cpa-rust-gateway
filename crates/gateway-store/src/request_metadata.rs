//! One encrypted metadata capture per admitted Request, with actual Attempt associations on read.
use crate::{
    CURRENT_SCHEMA_VERSION, StoreError, StoreResult,
    native_usage::{now_ms, read_event},
    schema_version,
    secret_store::{EncryptedSecret, KeyVersion, SecretStore},
};
use gateway_core::{
    AttemptEvent, AttemptId, ClientKeyId, GatewayEvent, GatewayProtocol, RequestEvent, RequestId,
    RequestMetadataEvidence,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path, time::Duration};
use zeroize::Zeroizing;

/// Exclusive retention, independent of Request, Attempt and ledger lifetimes.
pub const REQUEST_METADATA_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// The authenticated immutable encrypted capture. No Attempt is invented at admission.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestMetadataRecord {
    /// New evidence payload version, independent of older payload v1 formats.
    pub version: u16,
    /// Original authenticated Request, including owner and server Request ID.
    pub request: RequestEvent,
    /// Complete original C4 field and internal string.
    pub evidence: RequestMetadataEvidence,
}
impl fmt::Debug for RequestMetadataRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RequestMetadataRecord(<private>)")
    }
}

/// Authorized read-time projection: one capture and zero or more actual durable Attempts.
#[derive(Serialize)]
pub struct RequestMetadataDiagnostic {
    /// Authenticated original Request capture.
    pub record: RequestMetadataRecord,
    /// Current actual Attempt events, separately validated against durable event indexes.
    pub attempts: Vec<AttemptEvent>,
}

struct Row {
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
fn load(connection: &Connection, request: &str) -> StoreResult<Option<Row>> {
    connection.query_row("SELECT client_key_id,created_at_ms,expires_at_ms,payload_version,key_version,ciphertext FROM request_metadata_evidence WHERE request_id=?1",[request],|r|Ok(Row {owner:r.get(0)?,created:r.get(1)?,expires:r.get(2)?,version:r.get(3)?,key:r.get(4)?,ciphertext:r.get(5)?})).optional().map_err(StoreError::from)
}
fn aad(request: &str, row: &Row) -> StoreResult<Vec<u8>> {
    if row.version != 1
        || row.created < 0
        || row.created.checked_add(REQUEST_METADATA_TTL_MS) != Some(row.expires)
    {
        return Err(invalid());
    }
    let mut bytes = b"cpar:request-metadata:v1\0".to_vec();
    for value in [row.owner.as_str(), request] {
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
fn open_row(secrets: &SecretStore, request: &str, row: &Row) -> StoreResult<RequestMetadataRecord> {
    let ciphertext = row.ciphertext.as_ref().ok_or_else(invalid)?;
    if ciphertext.len() > gateway_core::MAX_REQUEST_METADATA_BYTES * 8 {
        return Err(invalid());
    }
    let encrypted = EncryptedSecret::try_from_persisted(
        KeyVersion::try_from_sqlite_i64(row.key).map_err(|_| invalid())?,
        ciphertext.clone(),
    )
    .map_err(|_| invalid())?;
    let plain = secrets
        .open(&encrypted, &aad(request, row)?)
        .map_err(|_| invalid())?;
    let record: RequestMetadataRecord =
        serde_json::from_slice(plain.as_bytes()).map_err(|_| invalid())?;
    if record.version != 1
        || record.request.client_key_id().as_str() != row.owner
        || record.request.request_id().as_str() != request
    {
        return Err(invalid());
    }
    RequestMetadataEvidence::try_new(record.evidence.raw_json().to_owned())
        .map_err(|_| invalid())?;
    Ok(record)
}
fn verify_request(connection: &Connection, request: &RequestEvent) -> StoreResult<()> {
    if request.protocol() != GatewayProtocol::OpenAiResponses
        || request.started_at_ms().is_none()
        || read_event(connection, "request", request.request_id().as_str())?
            != GatewayEvent::Request(request.clone())
    {
        return Err(invalid());
    }
    Ok(())
}

/// Read-only exact-owner local diagnostics, never a migration or inference service.
pub struct SqliteRequestMetadataReader {
    connection: Connection,
    secrets: SecretStore,
    schema: i64,
}
impl SqliteRequestMetadataReader {
    /// Opens a supported existing database without mutation.
    /// # Errors
    /// Rejects inaccessible or unsupported schemas.
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
    /// Reads one original Request and its actual Attempts. An optional selector must be actual.
    /// Missing, foreign, expired and historical evidence all return UNKNOWN.
    /// # Errors
    /// Fails closed on corrupt indexes, ownership, payload, ciphertext or unavailable keys.
    pub fn get_owned(
        &self,
        owner: &ClientKeyId,
        request: &RequestId,
        attempt: Option<&AttemptId>,
        now: i64,
    ) -> StoreResult<Option<RequestMetadataDiagnostic>> {
        if now < 0
            || [owner.as_str(), request.as_str()]
                .into_iter()
                .chain(attempt.map(AttemptId::as_str))
                .any(|id| id.is_empty() || id.len() > 512 || id.contains('\0'))
        {
            return Err(invalid());
        }
        if self.schema < 37 {
            return Ok(None);
        }
        let Some(row) = load(&self.connection, request.as_str())? else {
            return Ok(None);
        };
        if row.owner != owner.as_str() || now >= row.expires {
            return Ok(None);
        }
        let record = open_row(&self.secrets, request.as_str(), &row)?;
        verify_request(&self.connection, &record.request)?;
        let mut statement = self.connection.prepare(
            "SELECT event_id FROM gateway_event_log WHERE event_type='attempt' AND request_id=?1 ORDER BY event_ordinal")?;
        let ids = statement
            .query_map([request.as_str()], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut attempts = Vec::with_capacity(ids.len());
        for id in ids {
            let GatewayEvent::Attempt(actual) = read_event(&self.connection, "attempt", &id)?
            else {
                return Err(invalid());
            };
            if actual.request_id() != request {
                return Err(invalid());
            }
            attempts.push(actual);
        }
        if let Some(selected) = attempt {
            // Read the selected event independently: a damaged correlation index is an error,
            // while a missing or genuinely foreign Attempt is uniformly UNKNOWN.
            let exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM gateway_event_log WHERE event_type='attempt' AND event_id=?1)", [selected.as_str()], |r| r.get(0))?;
            if !exists {
                return Ok(None);
            }
            let GatewayEvent::Attempt(actual) =
                read_event(&self.connection, "attempt", selected.as_str())?
            else {
                return Err(invalid());
            };
            if actual.request_id() != request {
                return Ok(None);
            }
        }
        Ok(Some(RequestMetadataDiagnostic { record, attempts }))
    }
}

pub(crate) fn append(
    transaction: &Transaction<'_>,
    request: &RequestEvent,
    evidence: &RequestMetadataEvidence,
    secrets: &SecretStore,
    source_existed: bool,
) -> StoreResult<()> {
    RequestMetadataEvidence::try_new(evidence.raw_json().to_owned()).map_err(|_| invalid())?;
    verify_request(transaction, request)?;
    let record = RequestMetadataRecord {
        version: 1,
        request: request.clone(),
        evidence: evidence.clone(),
    };
    if let Some(row) = load(transaction, request.request_id().as_str())? {
        if row.ciphertext.is_none()
            || now_ms()? >= row.expires
            || open_row(secrets, request.request_id().as_str(), &row)? != record
        {
            return Err(StoreError::ConflictingGatewayEventReplay);
        }
        return Ok(());
    }
    if source_existed {
        return Err(StoreError::ConflictingGatewayEventReplay);
    }
    let created = now_ms()?;
    if request.started_at_ms().is_none_or(|start| start > created) {
        return Err(invalid());
    }
    let row = Row {
        owner: request.client_key_id().as_str().into(),
        created,
        expires: created
            .checked_add(REQUEST_METADATA_TTL_MS)
            .ok_or_else(invalid)?,
        version: 1,
        key: secrets.active_key_version().as_sqlite_i64(),
        ciphertext: None,
    };
    let plain = Zeroizing::new(serde_json::to_vec(&record).map_err(|_| invalid())?);
    let encrypted = secrets
        .seal(plain.as_slice(), &aad(request.request_id().as_str(), &row)?)
        .map_err(|_| invalid())?;
    transaction.execute("INSERT INTO request_metadata_evidence(request_id,client_key_id,created_at_ms,expires_at_ms,payload_version,key_version,ciphertext) VALUES (?1,?2,?3,?4,1,?5,?6)",
        params![request.request_id().as_str(),row.owner,row.created,row.expires,encrypted.key_version().as_sqlite_i64(),encrypted.ciphertext()])?;
    Ok(())
}

/// Erases a bounded number of expired ciphertexts while retaining anti-resurrection receipts.
/// # Errors
/// Returns a value-free storage error; never deletes lifecycle or billing records.
pub fn purge_expired(connection: &Connection, now: i64) -> StoreResult<usize> {
    connection.execute("UPDATE request_metadata_evidence SET ciphertext=NULL WHERE request_id IN (SELECT request_id FROM request_metadata_evidence WHERE expires_at_ms<=?1 AND ciphertext IS NOT NULL ORDER BY expires_at_ms LIMIT 128)",[now]).map_err(StoreError::from)
}
