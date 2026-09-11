//! Complete, secret-free inventories independent of runtime bindings.
use super::{
    ConfigVersion, ConfigVersionId, CredentialConfiguration, CredentialStatus,
    EndpointConfiguration, EndpointTransport, SqliteControlPlaneRepository, load_config_version,
    malformed, read_boolean, read_identifier,
};
use crate::{
    StoreError, StoreResult,
    account_identity::AccountIdentity,
    secret_store::{EncryptedSecret, KeyVersion},
};
use gateway_core::{CredentialId, EndpointId, UpstreamId};
use rusqlite::{Connection, OpenFlags, Transaction, params};
use std::{path::PathBuf, time::Duration};

/// Read filters and continuation for one configuration's managed resources.
#[derive(Clone, Copy)]
pub struct ResourceInventoryQuery<'a> {
    /// Exact configuration to inspect, including drafts.
    pub version: &'a ConfigVersionId,
    /// Optional owner filter.
    pub upstream_id: Option<&'a str>,
    /// Literal substring of resource identity, kind or owner. Never SQL syntax.
    pub search: &'a str,
    /// Maximum response size, between one and one hundred.
    pub limit: u16,
    /// Revision and resource-audit sequence from the first page.
    pub expected_snapshot: Option<(i64, i64)>,
    /// Exclusive stable resource identity.
    pub after: Option<&'a str>,
}

/// Display-only decryption is confined to the bounded inventory transaction.
pub type CredentialIdentityProjector =
    dyn Fn(&ConfigVersionId, &CredentialConfiguration) -> AccountIdentity + Send + Sync;

/// One configured connection; no URL secret is serialized by the management adapter.
pub struct AccountConnection {
    /// Stable technical reference.
    pub id: String,
    /// Actual adapter, used for classification.
    pub adapter_id: String,
    /// Actual wire protocol.
    pub api_format: String,
    /// Private classification input; the HTTP projection exposes only a sanitized hostname.
    pub base_url: String,
    /// Both binding and endpoint are enabled.
    pub enabled: bool,
}

impl std::fmt::Debug for AccountConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountConnection")
            .field("id", &self.id)
            .field("api_format", &self.api_format)
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

/// Safe credential presentation; plaintext and ciphertext never leave this inventory boundary.
#[derive(Debug)]
pub struct ManagedCredential {
    /// Stable credential identity.
    pub id: CredentialId,
    /// Owning upstream.
    pub upstream_id: UpstreamId,
    /// Runtime credential encoding.
    pub kind: String,
    /// Stored administrative state, not inferred health.
    pub status: CredentialStatus,
    /// Credential CAS revision.
    pub revision: i64,
    /// Whether encrypted material is present.
    pub secret_present: bool,
    /// Number of configured usage locations, including disabled bindings.
    pub binding_count: i64,
    /// Actual upstream family, distinct from its internal identifier.
    pub upstream_kind: String,
    /// Explicit identity evidence, never a digest or opaque subject.
    pub identity: AccountIdentity,
    /// Bounded connection previews; `binding_count` remains the complete count.
    pub connections: Vec<AccountConnection>,
}

/// A page read under one `SQLite` snapshot.
#[derive(Debug)]
pub struct ResourceInventoryPage<T> {
    /// Version metadata for the query.
    pub version: ConfigVersion,
    /// Includes OAuth rotations that do not advance the graph revision.
    pub audit_sequence: i64,
    /// Complete managed rows within the requested page.
    pub items: Vec<T>,
    /// Exclusive continuation key, when more matching rows exist.
    pub next_after: Option<String>,
}

/// Opens independent read-only connections inside the HTTP blocking admission.
#[derive(Clone)]
pub struct ResourceInventoryReader {
    path: PathBuf,
}

impl ResourceInventoryReader {
    fn repository(&self) -> StoreResult<SqliteControlPlaneRepository> {
        let connection = Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        Ok(SqliteControlPlaneRepository { connection })
    }

    /// Reads managed credentials without opening their sealed material.
    /// # Errors
    /// Rejects invalid input, changed snapshots or inaccessible storage.
    pub fn credentials(
        &self,
        query: ResourceInventoryQuery<'_>,
    ) -> StoreResult<ResourceInventoryPage<ManagedCredential>> {
        self.repository()?.managed_credentials(query)
    }

    /// Reads a bounded page with an allowlisted, display-only identity projection.
    /// # Errors
    /// Rejects stale snapshots, malformed persisted data or unavailable storage.
    pub fn credentials_with_identity(
        &self,
        query: ResourceInventoryQuery<'_>,
        project: &CredentialIdentityProjector,
    ) -> StoreResult<ResourceInventoryPage<ManagedCredential>> {
        self.repository()?
            .projected_credentials(query, Some(project))
    }

    /// Reads endpoints even when no account is bound to them.
    /// # Errors
    /// Rejects invalid input, changed snapshots or inaccessible storage.
    pub fn endpoints(
        &self,
        query: ResourceInventoryQuery<'_>,
    ) -> StoreResult<ResourceInventoryPage<EndpointConfiguration>> {
        self.repository()?.managed_endpoints(query)
    }
}

fn snapshot(
    transaction: &Transaction<'_>,
    query: &ResourceInventoryQuery<'_>,
) -> StoreResult<(ConfigVersion, i64)> {
    if !(1..=100).contains(&query.limit)
        || query.search.len() > 256
        || query
            .upstream_id
            .is_some_and(|value| value.is_empty() || value.len() > 128)
        || query.after.is_some_and(|value| {
            value.is_empty() || value.len() > 128 || query.expected_snapshot.is_none()
        })
    {
        return Err(malformed("resource_inventory_query"));
    }
    let version = load_config_version(transaction, query.version)?
        .ok_or(StoreError::ConfigVersionNotFound)?;
    let sequence: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(id), 0) FROM management_resource_audit_events WHERE config_version_id = ?1",
        [query.version.as_str()], |row| row.get(0),
    )?;
    if query
        .expected_snapshot
        .is_some_and(|expected| expected != (version.revision, sequence))
    {
        return Err(StoreError::ConfigVersionRevisionConflict);
    }
    Ok((version, sequence))
}

fn page<T>(
    version: ConfigVersion,
    audit_sequence: i64,
    mut items: Vec<T>,
    limit: u16,
    key: impl Fn(&T) -> &str,
) -> ResourceInventoryPage<T> {
    let next_after = if items.len() > usize::from(limit) {
        items.truncate(usize::from(limit));
        items.last().map(|row| key(row).to_owned())
    } else {
        None
    };
    ResourceInventoryPage {
        version,
        audit_sequence,
        items,
        next_after,
    }
}

impl SqliteControlPlaneRepository {
    /// Returns a reader for file-backed repositories, without exposing a connection.
    #[must_use]
    pub fn resource_inventory_reader(&self) -> Option<ResourceInventoryReader> {
        self.connection
            .path()
            .filter(|path| !path.is_empty() && *path != ":memory:")
            .map(|path| ResourceInventoryReader { path: path.into() })
    }

    /// Lists all matching credentials, including unbound and disabled records.
    /// # Errors
    /// Rejects stale continuations and malformed persisted metadata.
    pub fn managed_credentials(
        &mut self,
        query: ResourceInventoryQuery<'_>,
    ) -> StoreResult<ResourceInventoryPage<ManagedCredential>> {
        self.projected_credentials(query, None)
    }

    fn projected_credentials(
        &mut self,
        query: ResourceInventoryQuery<'_>,
        project: Option<&CredentialIdentityProjector>,
    ) -> StoreResult<ResourceInventoryPage<ManagedCredential>> {
        let transaction = self.connection.transaction()?;
        let (version, sequence) = snapshot(&transaction, &query)?;
        let mut items = Vec::new();
        {
            let mut statement = transaction.prepare(
                "SELECT c.id, c.upstream_id, c.kind, c.status, c.revision, length(c.ciphertext) > 0, \
                (SELECT count(*) FROM endpoint_credential_bindings b WHERE b.config_version_id = c.config_version_id AND b.credential_id = c.id) \
                FROM upstream_credentials c WHERE c.config_version_id = ?1 AND c.id > ?2 \
                AND (?3 IS NULL OR c.upstream_id = ?3) \
                AND (?4 = '' OR instr(lower(c.id || ' ' || c.upstream_id || ' ' || c.kind), lower(?4)) > 0) \
                ORDER BY c.id LIMIT ?5",
            )?;
            let mut rows = statement.query(params![
                query.version.as_str(),
                query.after.unwrap_or(""),
                query.upstream_id,
                query.search,
                i64::from(query.limit) + 1
            ])?;
            while let Some(row) = rows.next()? {
                let revision: i64 = row.get(4)?;
                let status = CredentialStatus::from_sql(&row.get::<_, String>(3)?)
                    .ok_or_else(|| malformed("upstream_credentials"))?;
                if revision < 0 {
                    return Err(malformed("upstream_credentials"));
                }
                let id = read_identifier(row, 0, CredentialId::try_new, "upstream_credentials")?;
                let upstream_id =
                    read_identifier(row, 1, UpstreamId::try_new, "upstream_credentials")?;
                let upstream_kind: String = transaction.query_row(
                    "SELECT kind FROM upstreams WHERE config_version_id=?1 AND id=?2",
                    params![query.version.as_str(), upstream_id.as_str()],
                    |r| r.get(0),
                )?;
                let identity = if let Some(project) =
                    project.filter(|_| items.len() < usize::from(query.limit))
                {
                    let (ciphertext,key_version) = transaction.query_row("SELECT ciphertext,key_version FROM upstream_credentials WHERE config_version_id=?1 AND id=?2",params![query.version.as_str(),id.as_str()],|r|Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,i64>(1)?)))?;
                    let sealed = EncryptedSecret::try_from_persisted(
                        KeyVersion::try_from_sqlite_i64(key_version)
                            .map_err(|_| malformed("upstream_credentials"))?,
                        ciphertext,
                    )
                    .map_err(|_| malformed("upstream_credentials"))?;
                    project(
                        query.version,
                        &CredentialConfiguration {
                            id: id.clone(),
                            upstream_id: upstream_id.clone(),
                            kind: row.get(2)?,
                            encrypted_secret: sealed,
                            status,
                            revision,
                        },
                    )
                } else {
                    AccountIdentity::default()
                };
                let connections = transaction.prepare("SELECT e.id,e.adapter_id,e.api_format,e.base_url,(e.enabled AND b.enabled) FROM endpoint_credential_bindings b JOIN upstream_endpoints e ON e.config_version_id=b.config_version_id AND e.id=b.endpoint_id WHERE b.config_version_id=?1 AND b.credential_id=?2 ORDER BY e.id LIMIT 100")?.query_map(params![query.version.as_str(),id.as_str()],|r|Ok(AccountConnection{id:r.get(0)?,adapter_id:r.get(1)?,api_format:r.get(2)?,base_url:r.get(3)?,enabled:r.get(4)?}))?.collect::<Result<Vec<_>,_>>()?;
                items.push(ManagedCredential {
                    id: read_identifier(row, 0, CredentialId::try_new, "upstream_credentials")?,
                    upstream_id: read_identifier(
                        row,
                        1,
                        UpstreamId::try_new,
                        "upstream_credentials",
                    )?,
                    kind: row.get(2)?,
                    status,
                    revision,
                    secret_present: read_boolean(row, 5, "upstream_credentials")?,
                    binding_count: row.get(6)?,
                    upstream_kind,
                    identity,
                    connections,
                });
            }
        }
        transaction.commit()?;
        Ok(page(version, sequence, items, query.limit, |item| {
            item.id.as_str()
        }))
    }

    /// Lists all matching endpoints without requiring a credential binding.
    /// # Errors
    /// Rejects stale continuations and malformed persisted metadata.
    pub fn managed_endpoints(
        &mut self,
        query: ResourceInventoryQuery<'_>,
    ) -> StoreResult<ResourceInventoryPage<EndpointConfiguration>> {
        let transaction = self.connection.transaction()?;
        let (version, sequence) = snapshot(&transaction, &query)?;
        let mut items = Vec::new();
        {
            let mut statement = transaction.prepare(
                "SELECT id, upstream_id, adapter_id, api_format, base_url, inference_path, models_path, transport, enabled \
                 FROM upstream_endpoints WHERE config_version_id = ?1 AND id > ?2 \
                 AND (?3 IS NULL OR upstream_id = ?3) \
                 AND (?4 = '' OR instr(lower(id || ' ' || upstream_id || ' ' || adapter_id), lower(?4)) > 0) \
                 ORDER BY id LIMIT ?5",
            )?;
            let mut rows = statement.query(params![
                query.version.as_str(),
                query.after.unwrap_or(""),
                query.upstream_id,
                query.search,
                i64::from(query.limit) + 1
            ])?;
            while let Some(row) = rows.next()? {
                items.push(EndpointConfiguration {
                    id: read_identifier(row, 0, EndpointId::try_new, "upstream_endpoints")?,
                    upstream_id: read_identifier(
                        row,
                        1,
                        UpstreamId::try_new,
                        "upstream_endpoints",
                    )?,
                    adapter_id: row.get(2)?,
                    api_format: row.get(3)?,
                    base_url: row.get(4)?,
                    inference_path: row.get(5)?,
                    models_path: row.get(6)?,
                    transport: EndpointTransport::from_sql(&row.get::<_, String>(7)?)
                        .ok_or_else(|| malformed("upstream_endpoints"))?,
                    enabled: read_boolean(row, 8, "upstream_endpoints")?,
                });
            }
        }
        transaction.commit()?;
        Ok(page(version, sequence, items, query.limit, |item| {
            item.id.as_str()
        }))
    }
}
