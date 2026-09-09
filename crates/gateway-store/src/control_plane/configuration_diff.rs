//! Record-level version differences. Only identities and changed field names leave `SQLite`.
use super::{ConfigVersion, ConfigVersionId, SqliteControlPlaneRepository, load_config_version};
use rusqlite::{Transaction, params};
use std::{error::Error, fmt};

/// Safe metadata for an added, removed or changed resource; never contains field values.
#[derive(Debug, Eq, PartialEq)]
pub struct ConfigurationResourceChange {
    /// Versioned resource category.
    pub resource_kind: String,
    /// JSON array of primary-key identity components (excluding the version ID).
    pub resource_key: String,
    /// One of added, removed or changed.
    pub change: String,
    /// Column names only; protected values never leave the store.
    pub changed_fields: Vec<String>,
}
/// A bounded page from one `SQLite` read transaction.
#[derive(Debug)]
pub struct ConfigurationDiffPage {
    /// Base metadata read in the comparison transaction.
    pub base: ConfigVersion,
    /// Target metadata read in the comparison transaction.
    pub target: ConfigVersion,
    /// At most the requested number of resource differences.
    pub items: Vec<ConfigurationResourceChange>,
    /// Another bounded page exists in this snapshot.
    pub has_more: bool,
}
/// Page bounds and optional revision-pinned continuation.
#[derive(Clone, Copy)]
pub struct ConfigurationDiffQuery<'a> {
    /// Existing base version identity.
    pub base: &'a ConfigVersionId,
    /// Existing target version identity.
    pub target: &'a ConfigVersionId,
    /// Both previously observed revisions, required for continuation.
    pub expected_revisions: Option<(i64, i64)>,
    /// Exclusive ordered resource-kind/key position.
    pub after: Option<(&'a str, &'a str)>,
    /// Requested page size, 1 through 200.
    pub limit: u16,
}
/// Closed errors; callers must not expose internal `SQLite` error text.
#[derive(Debug)]
pub enum ConfigurationDiffError {
    /// Page arguments are outside the accepted bounds.
    InvalidQuery,
    /// Either requested version does not exist.
    MissingVersion,
    /// A version changed since the previous page.
    RevisionChanged,
    /// Persistence or schema could not be read.
    Store(crate::StoreError),
}
impl fmt::Display for ConfigurationDiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("configuration comparison unavailable")
    }
}
impl Error for ConfigurationDiffError {}
impl From<crate::StoreError> for ConfigurationDiffError {
    fn from(error: crate::StoreError) -> Self {
        Self::Store(error)
    }
}
impl From<rusqlite::Error> for ConfigurationDiffError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

// Every version-scoped resource in ControlPlaneConfiguration. Runtime observations and global
// billing catalogs are not configuration resources; the routing-price binding is included.
const TABLES: &[(&str, &str)] = &[
    ("egress_policy", "egress_policies"),
    ("upstream", "upstreams"),
    ("proxy_pool", "compatible_egress_proxy_pools"),
    ("proxy_node", "compatible_egress_proxy_nodes"),
    ("endpoint", "upstream_endpoints"),
    ("credential", "upstream_credentials"),
    ("credential_binding", "endpoint_credential_bindings"),
    ("egress_binding", "compatible_egress_binding_profiles"),
    ("public_model", "public_models"),
    ("alias", "model_aliases"),
    ("route", "model_routes"),
    ("candidate", "route_candidates"),
    ("access_group", "access_groups"),
    ("route_grant", "access_group_routes"),
    ("client_key", "client_keys"),
    ("routing_price_policy", "routing_price_policies"),
];

/// A file-backed comparison source that opens an independent read-only connection per read.
#[derive(Clone, Debug)]
pub struct ConfigurationDiffReader {
    path: std::path::PathBuf,
}
impl ConfigurationDiffReader {
    /// Reads without retaining the management mutation connection or running migrations.
    ///
    /// # Errors
    /// Returns comparison errors or a closed persistence error for an unavailable database.
    pub fn read(
        &self,
        query: ConfigurationDiffQuery<'_>,
    ) -> Result<ConfigurationDiffPage, ConfigurationDiffError> {
        let connection = rusqlite::Connection::open_with_flags(
            &self.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        SqliteControlPlaneRepository { connection }.configuration_diff(query)
    }
}

impl SqliteControlPlaneRepository {
    /// Creates a file-backed reader without sharing this repository's connection.
    /// In-memory repositories have no independently openable file and return None.
    #[must_use]
    pub fn configuration_diff_reader(&self) -> Option<ConfigurationDiffReader> {
        self.connection
            .path()
            .filter(|path| !path.is_empty() && *path != ":memory:")
            .map(|path| ConfigurationDiffReader { path: path.into() })
    }

    /// Compares complete stored resources with bounded output and revision-pinned continuation.
    ///
    /// # Errors
    /// Rejects invalid bounds, absent versions, changed revisions or malformed stored metadata.
    pub fn configuration_diff(
        &mut self,
        query: ConfigurationDiffQuery<'_>,
    ) -> Result<ConfigurationDiffPage, ConfigurationDiffError> {
        if !(1..=200).contains(&query.limit)
            || (query.after.is_some() && query.expected_revisions.is_none())
        {
            return Err(ConfigurationDiffError::InvalidQuery);
        }
        let transaction = self.connection.transaction()?;
        let base = load_config_version(&transaction, query.base)?
            .ok_or(ConfigurationDiffError::MissingVersion)?;
        let target = load_config_version(&transaction, query.target)?
            .ok_or(ConfigurationDiffError::MissingVersion)?;
        if query
            .expected_revisions
            .is_some_and(|expected| expected != (base.revision, target.revision))
        {
            return Err(ConfigurationDiffError::RevisionChanged);
        }
        let mut parts = Vec::new();
        for &(kind, table) in TABLES {
            parts.push(table_diff_sql(&transaction, kind, table)?);
        }
        let sql = format!(
            "SELECT resource_kind, resource_key, change, changed_fields FROM ({}) \
            WHERE (?3 IS NULL OR resource_kind > ?3 OR (resource_kind = ?3 AND resource_key > ?4)) \
            ORDER BY resource_kind, resource_key LIMIT ?5",
            parts.join(" UNION ALL ")
        );
        let mut items = Vec::new();
        {
            let mut statement = transaction.prepare(&sql)?;
            let mut rows = statement.query(params![
                query.base.as_str(),
                query.target.as_str(),
                query.after.map(|after| after.0),
                query.after.map(|after| after.1),
                i64::from(query.limit) + 1
            ])?;
            while let Some(row) = rows.next()? {
                let fields: String = row.get(3)?;
                items.push(ConfigurationResourceChange {
                    resource_kind: row.get(0)?,
                    resource_key: row.get(1)?,
                    change: row.get(2)?,
                    changed_fields: serde_json::from_str(&fields)
                        .map_err(|_| super::malformed("configuration_diff"))?,
                });
            }
        }
        let has_more = items.len() > usize::from(query.limit);
        items.truncate(usize::from(query.limit));
        transaction.commit()?;
        Ok(ConfigurationDiffPage {
            base,
            target,
            items,
            has_more,
        })
    }
}

fn identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn table_diff_sql(
    transaction: &Transaction<'_>,
    kind: &str,
    table: &str,
) -> Result<String, ConfigurationDiffError> {
    let table = identifier(table);
    let mut statement = transaction.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let keys = columns
        .iter()
        .filter(|(name, pk)| *pk > 0 && name != "config_version_id")
        .map(|(name, _)| identifier(name))
        .collect::<Vec<_>>();
    let fields = columns
        .iter()
        .filter(|(name, pk)| *pk == 0 && name != "config_version_id")
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    let join = if keys.is_empty() {
        "1".to_owned()
    } else {
        keys.iter()
            .map(|key| format!("t.{key} = b.{key}"))
            .collect::<Vec<_>>()
            .join(" AND ")
    };
    let key = |alias| {
        if keys.is_empty() {
            "json_array('singleton')".to_owned()
        } else {
            format!(
                "json_array({})",
                keys.iter()
                    .map(|key| format!("{alias}.{key}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    };
    let differs = fields
        .iter()
        .map(|field| {
            let field = identifier(field);
            format!("t.{field} IS NOT b.{field}")
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    let field_names = fields
        .iter()
        .map(|field| {
            format!(
                "SELECT {} AS name WHERE t.{} IS NOT b.{}",
                literal(field),
                identifier(field),
                identifier(field)
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let kind = literal(kind);
    Ok(format!(
        "SELECT {kind} AS resource_kind, {} AS resource_key, \
        CASE WHEN b.config_version_id IS NULL THEN 'added' ELSE 'changed' END AS change, \
        CASE WHEN b.config_version_id IS NULL THEN '[]' ELSE (SELECT json_group_array(name) FROM ({field_names})) END AS changed_fields \
        FROM {table} t LEFT JOIN {table} b ON b.config_version_id = ?1 AND {join} \
        WHERE t.config_version_id = ?2 AND (b.config_version_id IS NULL OR {differs}) \
        UNION ALL SELECT {kind}, {}, 'removed', '[]' FROM {table} b \
        LEFT JOIN {table} t ON t.config_version_id = ?2 AND {join} \
        WHERE b.config_version_id = ?1 AND t.config_version_id IS NULL",
        key("t"),
        key("b")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_plane::{ConfigVersionStatus, ControlPlaneConfiguration};

    #[test]
    fn file_reader_observes_committed_snapshot_while_writer_is_open() -> Result<(), Box<dyn Error>>
    {
        let directory = std::env::temp_dir().join(format!(
            "prism-diff-reader-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir(&directory)?;
        let mut repository = SqliteControlPlaneRepository::open(directory.join("control.sqlite3"))?;
        let base = ConfigVersionId::try_new("reader-base")?;
        let target = ConfigVersionId::try_new("reader-target")?;
        for id in [&base, &target] {
            repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
                id: id.clone(),
                parent_id: None,
                status: ConfigVersionStatus::Draft,
                revision: 0,
                created_at_ms: 0,
                description: String::new(),
            }))?;
        }
        let reader = repository
            .configuration_diff_reader()
            .ok_or("file reader unavailable")?;
        let writer = repository.connection.transaction()?;
        writer.execute(
            "UPDATE config_versions SET revision=1 WHERE id=?1",
            [target.as_str()],
        )?;
        let query = ConfigurationDiffQuery {
            base: &base,
            target: &target,
            expected_revisions: Some((0, 0)),
            after: None,
            limit: 1,
        };
        assert_eq!(reader.read(query)?.target.revision, 0);
        writer.commit()?;
        assert!(matches!(
            reader.read(query),
            Err(ConfigurationDiffError::RevisionChanged)
        ));
        drop(repository);
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn composite_grants_and_singleton_price_policy_compare_by_identity()
    -> Result<(), Box<dyn Error>> {
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        let base = ConfigVersionId::try_new("base")?;
        let target = ConfigVersionId::try_new("target")?;
        for (id, enabled, catalog) in [(&base, 1, "old-prices"), (&target, 0, "new-prices")] {
            repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
                id: id.clone(),
                parent_id: None,
                status: ConfigVersionStatus::Draft,
                revision: 0,
                created_at_ms: 0,
                description: String::new(),
            }))?;
            repository.connection.execute(
                "INSERT INTO public_models VALUES (?1,'model','model','active','model','{}')",
                [id.as_str()],
            )?;
            repository.connection.execute("INSERT INTO model_routes VALUES (?1,'route','model','smooth_weighted_round_robin',1,1000)", [id.as_str()])?;
            repository.connection.execute(
                "INSERT INTO access_groups VALUES (?1,'group','group','active','{}')",
                [id.as_str()],
            )?;
            repository.connection.execute(
                "INSERT INTO access_group_routes VALUES (?1,'group','route',?2)",
                params![id.as_str(), enabled],
            )?;
            repository.connection.execute(
                "INSERT INTO billing_price_catalog_versions VALUES (?1,0,'test',0)",
                [catalog],
            )?;
            repository.connection.execute(
                "INSERT INTO routing_price_policies VALUES (?1,?2,'rate_dominance_v1')",
                params![id.as_str(), catalog],
            )?;
        }
        let page = repository.configuration_diff(ConfigurationDiffQuery {
            base: &base,
            target: &target,
            expected_revisions: None,
            after: None,
            limit: 10,
        })?;
        assert_eq!(page.items.len(), 2);
        assert!(
            page.items
                .iter()
                .any(|row| row.resource_key == "[\"group\",\"route\"]"
                    && row.changed_fields == ["enabled"])
        );
        assert!(
            page.items
                .iter()
                .any(|row| row.resource_kind == "routing_price_policy"
                    && row.changed_fields == ["catalog_version_id"])
        );
        assert!(matches!(
            repository.configuration_diff(ConfigurationDiffQuery {
                base: &base,
                target: &target,
                expected_revisions: None,
                after: Some(("route", "key")),
                limit: 1
            }),
            Err(ConfigurationDiffError::InvalidQuery)
        ));
        assert!(matches!(
            repository.configuration_diff(ConfigurationDiffQuery {
                base: &base,
                target: &target,
                expected_revisions: None,
                after: None,
                limit: 201
            }),
            Err(ConfigurationDiffError::InvalidQuery)
        ));
        Ok(())
    }

    #[test]
    fn pages_compare_records_without_values_and_pin_both_revisions() -> Result<(), Box<dyn Error>> {
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        let base = ConfigVersionId::try_new("diff-base")?;
        let target = ConfigVersionId::try_new("diff-target")?;
        for id in [&base, &target] {
            repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
                id: id.clone(),
                parent_id: None,
                status: ConfigVersionStatus::Draft,
                revision: 0,
                created_at_ms: 0,
                description: String::new(),
            }))?;
            repository.connection.execute(
                "INSERT INTO public_models VALUES (?1, 'common', 'common', 'active', 'same', '{}')",
                [id.as_str()],
            )?;
            repository.connection.execute("INSERT INTO upstreams (config_version_id,id,name,kind,enabled,tags_json) VALUES (?1,'upstream','same','compatible',1,'[]')", [id.as_str()])?;
            repository.connection.execute("INSERT INTO upstream_credentials VALUES (?1,'credential','upstream','bearer',?2,1,'active',0)", params![id.as_str(), if id == &base { b"opaque-test-a".as_slice() } else { b"opaque-test-b".as_slice() }])?;
        }
        repository.connection.execute("UPDATE public_models SET display_name='changed-value-not-returned' WHERE config_version_id=?1", [target.as_str()])?;
        repository.connection.execute(
            "INSERT INTO public_models VALUES (?1,'removed','removed','disabled','removed','{}')",
            [base.as_str()],
        )?;
        for index in 0..205 {
            let id = format!("added-{index:03}");
            repository.connection.execute(
                "INSERT INTO public_models VALUES (?1,?2,?2,'disabled','added','{}')",
                params![target.as_str(), id],
            )?;
        }
        let mut after: Option<(String, String)> = None;
        let mut all = Vec::new();
        loop {
            let page = repository.configuration_diff(ConfigurationDiffQuery {
                base: &base,
                target: &target,
                expected_revisions: Some((0, 0)),
                after: after
                    .as_ref()
                    .map(|(kind, key)| (kind.as_str(), key.as_str())),
                limit: 100,
            })?;
            assert!(page.items.len() <= 100);
            let more = page.has_more;
            after = page
                .items
                .last()
                .map(|row| (row.resource_kind.clone(), row.resource_key.clone()));
            all.extend(page.items);
            if !more {
                break;
            }
        }
        assert_eq!(all.len(), 208);
        assert_eq!(all.iter().filter(|row| row.change == "added").count(), 205);
        let credential = all
            .iter()
            .find(|row| row.resource_kind == "credential")
            .ok_or("credential diff missing")?;
        assert_eq!(credential.changed_fields, vec!["ciphertext"]);
        let common = all
            .iter()
            .find(|row| row.resource_key == "[\"common\"]")
            .ok_or("model diff missing")?;
        assert_eq!(common.changed_fields, vec!["display_name"]);
        let projected = format!("{all:?}");
        assert!(!projected.contains("opaque-test"));
        assert!(!projected.contains("changed-value-not-returned"));
        let same = repository.configuration_diff(ConfigurationDiffQuery {
            base: &base,
            target: &base,
            expected_revisions: None,
            after: None,
            limit: 1,
        })?;
        assert!(same.items.is_empty());
        repository.connection.execute(
            "UPDATE config_versions SET revision=1 WHERE id=?1",
            [target.as_str()],
        )?;
        assert!(matches!(
            repository.configuration_diff(ConfigurationDiffQuery {
                base: &base,
                target: &target,
                expected_revisions: Some((0, 0)),
                after: None,
                limit: 1
            }),
            Err(ConfigurationDiffError::RevisionChanged)
        ));
        Ok(())
    }
}
