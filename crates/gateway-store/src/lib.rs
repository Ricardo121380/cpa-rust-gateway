//! Versioned `SQLite` control-plane persistence boundary; never queried by the route hot path.

#![deny(unsafe_code)]

use std::{error::Error, fmt, path::Path};

use rusqlite::{Connection, Error as SqliteError};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-store";

/// Most recent schema version understood by this build.
pub const CURRENT_SCHEMA_VERSION: i64 = 1;

const CREATE_SCHEMA_MIGRATIONS: &str = "
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY CHECK (version > 0),
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;
";

const MIGRATIONS: &[Migration] = &[Migration {
    version: CURRENT_SCHEMA_VERSION,
    up: include_str!("../migrations/0001_versioned_control_plane.up.sql"),
    down: include_str!("../migrations/0001_versioned_control_plane.down.sql"),
}];

struct Migration {
    version: i64,
    up: &'static str,
    down: &'static str,
}

/// Failure returned by the control-plane migration boundary.
#[derive(Debug)]
pub enum StoreError {
    /// `SQLite` rejected an operation or could not open the requested database.
    Sqlite(SqliteError),
    /// Foreign-key enforcement could not be enabled on this connection.
    ForeignKeysDisabled,
    /// The database migration history is not a supported prefix of this build's migrations.
    UnsupportedMigrationHistory {
        /// Ordered migration versions found in the database.
        applied: Vec<i64>,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "SQLite store operation failed: {error}"),
            Self::ForeignKeysDisabled => {
                formatter.write_str("SQLite foreign-key enforcement could not be enabled")
            }
            Self::UnsupportedMigrationHistory { .. } => {
                formatter.write_str("database migration history is not supported by this build")
            }
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::ForeignKeysDisabled | Self::UnsupportedMigrationHistory { .. } => None,
        }
    }
}

impl From<SqliteError> for StoreError {
    fn from(error: SqliteError) -> Self {
        Self::Sqlite(error)
    }
}

/// Result type returned by the Store migration boundary.
pub type StoreResult<T> = Result<T, StoreError>;

/// Opens a file-backed control-plane database with foreign-key enforcement enabled.
///
/// # Errors
///
/// Returns [`StoreError`] when `SQLite` cannot open the path or foreign-key enforcement cannot be
/// enabled. The caller must still call [`migrate`] before using control-plane tables.
pub fn open(path: impl AsRef<Path>) -> StoreResult<Connection> {
    let connection = Connection::open(path)?;
    enable_foreign_keys(&connection)?;
    Ok(connection)
}

/// Opens an in-memory control-plane database with foreign-key enforcement enabled.
///
/// # Errors
///
/// Returns [`StoreError`] when `SQLite` cannot create the connection or foreign-key enforcement
/// cannot be enabled.
pub fn open_in_memory() -> StoreResult<Connection> {
    let connection = Connection::open_in_memory()?;
    enable_foreign_keys(&connection)?;
    Ok(connection)
}

/// Applies every migration not yet recorded by the database.
///
/// The migration bookkeeping and each migration are committed atomically. The function enables
/// and verifies `SQLite` foreign keys before it writes any schema state.
///
/// # Errors
///
/// Returns [`StoreError::UnsupportedMigrationHistory`] rather than guessing how to upgrade a
/// database whose applied versions are not a prefix of this build's migrations. `SQLite` failures
/// are returned without exposing configured ciphertext values.
pub fn migrate(connection: &mut Connection) -> StoreResult<()> {
    enable_foreign_keys(connection)?;
    let applied = applied_migration_versions(connection)?;
    ensure_supported_prefix(&applied)?;

    for migration in MIGRATIONS.iter().skip(applied.len()) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(CREATE_SCHEMA_MIGRATIONS)?;
        transaction.execute_batch(migration.up)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version) VALUES (?1)",
            [migration.version],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

/// Rolls every migration known to this build back to the original user-table set.
///
/// Only this crate's migration tables are dropped. If no migrations remain, the internal
/// `schema_migrations` bookkeeping table is removed as well.
///
/// # Errors
///
/// Returns [`StoreError::UnsupportedMigrationHistory`] instead of applying a partial or guessed
/// rollback when the stored migration sequence is not supported by this build.
pub fn rollback_all(connection: &mut Connection) -> StoreResult<()> {
    enable_foreign_keys(connection)?;
    let applied = applied_migration_versions(connection)?;
    ensure_supported_prefix(&applied)?;

    for migration in MIGRATIONS.iter().take(applied.len()).rev() {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.down)?;
        transaction.execute(
            "DELETE FROM schema_migrations WHERE version = ?1",
            [migration.version],
        )?;
        transaction.commit()?;
    }

    if table_exists(connection, "schema_migrations")?
        && applied_migration_versions(connection)?.is_empty()
    {
        connection.execute_batch("DROP TABLE schema_migrations;")?;
    }

    Ok(())
}

/// Returns the latest applied schema version, or `None` for an unmigrated database.
///
/// # Errors
///
/// Returns [`StoreError::UnsupportedMigrationHistory`] if the database does not contain a prefix
/// of the migrations known to this build.
pub fn schema_version(connection: &Connection) -> StoreResult<Option<i64>> {
    let applied = applied_migration_versions(connection)?;
    ensure_supported_prefix(&applied)?;
    Ok(applied.last().copied())
}

fn enable_foreign_keys(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    let enabled: i64 = connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    if enabled == 1 {
        Ok(())
    } else {
        Err(StoreError::ForeignKeysDisabled)
    }
}

fn applied_migration_versions(connection: &Connection) -> StoreResult<Vec<i64>> {
    if !table_exists(connection, "schema_migrations")? {
        return Ok(Vec::new());
    }

    let mut statement =
        connection.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let versions = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<i64>, SqliteError>>()?;
    Ok(versions)
}

fn ensure_supported_prefix(applied: &[i64]) -> StoreResult<()> {
    let expected: Vec<_> = MIGRATIONS
        .iter()
        .take(applied.len())
        .map(|migration| migration.version)
        .collect();
    if applied == expected {
        Ok(())
    } else {
        Err(StoreError::UnsupportedMigrationHistory {
            applied: applied.to_vec(),
        })
    }
}

fn table_exists(connection: &Connection, table_name: &str) -> StoreResult<bool> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
        [table_name],
        |row| row.get(0),
    )?;
    Ok(exists)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use rusqlite::{Connection, Error as SqliteError, ErrorCode, params};

    use super::{CURRENT_SCHEMA_VERSION, migrate, open_in_memory, rollback_all, schema_version};

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn migration_up_is_idempotent_and_creates_the_p2_01_tables() -> TestResult {
        let mut connection = Connection::open_in_memory()?;

        migrate(&mut connection)?;
        migrate(&mut connection)?;

        assert_eq!(schema_version(&connection)?, Some(CURRENT_SCHEMA_VERSION));
        assert!(foreign_keys_enabled(&connection)?);
        assert_eq!(
            control_plane_tables(&connection)?,
            vec![
                "config_versions",
                "endpoint_credential_bindings",
                "upstream_credentials",
                "upstream_endpoints",
                "upstreams",
            ]
        );
        Ok(())
    }

    #[test]
    fn valid_tree_succeeds_and_foreign_keys_reject_orphans_and_cross_upstream_bindings()
    -> TestResult {
        let mut connection = open_in_memory()?;
        migrate(&mut connection)?;
        insert_valid_tree(&connection)?;

        let missing_version = connection.execute(
            "INSERT INTO upstreams (\
                config_version_id, id, name, kind, enabled, tags_json, egress_policy_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "missing-version",
                "orphan-upstream",
                "orphan-station",
                "relay",
                1_i64,
                "[]",
                Option::<&str>::None
            ],
        );
        assert!(is_foreign_key_violation(&missing_version));

        let missing_upstream = connection.execute(
            "INSERT INTO upstream_endpoints (\
                config_version_id, id, upstream_id, adapter_id, api_format, base_url, \
                inference_path, models_path, transport, enabled\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                "v1",
                "orphan-endpoint",
                "missing-upstream",
                "openai-compatible.responses",
                "openai/responses",
                "https://missing.example/v1",
                "/responses",
                "/models",
                "http",
                1_i64,
            ],
        );
        assert!(is_foreign_key_violation(&missing_upstream));

        connection.execute(
            "INSERT INTO upstreams (\
                config_version_id, id, name, kind, enabled, tags_json, egress_policy_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "v1",
                "upstream-b",
                "station-b",
                "relay",
                1_i64,
                "[]",
                Option::<&str>::None
            ],
        )?;
        connection.execute(
            "INSERT INTO upstream_endpoints (\
                config_version_id, id, upstream_id, adapter_id, api_format, base_url, \
                inference_path, models_path, transport, enabled\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                "v1",
                "endpoint-b",
                "upstream-b",
                "openai-compatible.responses",
                "openai/responses",
                "https://station-b.example/v1",
                "/responses",
                "/models",
                "http",
                1_i64,
            ],
        )?;

        let cross_upstream_binding = connection.execute(
            "INSERT INTO endpoint_credential_bindings (\
                config_version_id, endpoint_id, credential_id, upstream_id, enabled, priority, weight, concurrency\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "v1",
                "endpoint-b",
                "credential-a",
                "upstream-b",
                1_i64,
                0_i64,
                1_i64,
                4_i64,
            ],
        );
        assert!(is_foreign_key_violation(&cross_upstream_binding));
        assert!(foreign_key_check_is_clean(&connection)?);
        Ok(())
    }

    #[test]
    fn migration_up_then_down_restores_the_original_user_tables() -> TestResult {
        let mut connection = open_in_memory()?;
        connection
            .execute_batch("CREATE TABLE caller_owned_baseline (id INTEGER PRIMARY KEY) STRICT;")?;
        let baseline = user_tables(&connection)?;

        migrate(&mut connection)?;
        insert_parent_versions(&connection)?;
        rollback_all(&mut connection)?;

        assert_eq!(schema_version(&connection)?, None);
        assert_eq!(user_tables(&connection)?, baseline);
        Ok(())
    }

    fn insert_valid_tree(connection: &Connection) -> TestResult {
        connection.execute(
            "INSERT INTO config_versions (id, parent_id, status, created_at_ms, description) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "v1",
                Option::<&str>::None,
                "draft",
                1_i64,
                "P2-01 FK fixture"
            ],
        )?;
        connection.execute(
            "INSERT INTO upstreams (\
                config_version_id, id, name, kind, enabled, tags_json, egress_policy_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "v1",
                "upstream-a",
                "station-a",
                "relay",
                1_i64,
                "[\"test\"]",
                Option::<&str>::None
            ],
        )?;
        connection.execute(
            "INSERT INTO upstream_endpoints (\
                config_version_id, id, upstream_id, adapter_id, api_format, base_url, \
                inference_path, models_path, transport, enabled\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                "v1",
                "endpoint-a",
                "upstream-a",
                "openai-compatible.responses",
                "openai/responses",
                "https://station-a.example/v1",
                "/responses",
                "/models",
                "http",
                1_i64,
            ],
        )?;
        connection.execute(
            "INSERT INTO upstream_credentials (\
                config_version_id, id, upstream_id, kind, ciphertext, key_version, status, revision\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "v1",
                "credential-a",
                "upstream-a",
                "api_key",
                &[1_u8, 2, 3],
                1_i64,
                "active",
                0_i64,
            ],
        )?;
        connection.execute(
            "INSERT INTO endpoint_credential_bindings (\
                config_version_id, endpoint_id, credential_id, upstream_id, enabled, priority, weight, concurrency\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "v1",
                "endpoint-a",
                "credential-a",
                "upstream-a",
                1_i64,
                0_i64,
                1_i64,
                4_i64,
            ],
        )?;
        Ok(())
    }

    fn insert_parent_versions(connection: &Connection) -> TestResult {
        connection.execute(
            "INSERT INTO config_versions (id, parent_id, status, created_at_ms, description) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "rollback-parent",
                Option::<&str>::None,
                "archived",
                1_i64,
                "rollback parent"
            ],
        )?;
        connection.execute(
            "INSERT INTO config_versions (id, parent_id, status, created_at_ms, description) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "rollback-child",
                "rollback-parent",
                "draft",
                2_i64,
                "rollback child"
            ],
        )?;
        Ok(())
    }

    fn is_foreign_key_violation(result: &Result<usize, SqliteError>) -> bool {
        matches!(
            result,
            Err(SqliteError::SqliteFailure(code, _)) if code.code == ErrorCode::ConstraintViolation
        )
    }

    fn foreign_keys_enabled(connection: &Connection) -> Result<bool, SqliteError> {
        connection
            .pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))
            .map(|value| value == 1)
    }

    fn foreign_key_check_is_clean(connection: &Connection) -> Result<bool, SqliteError> {
        let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
        Ok(statement.query([])?.next()?.is_none())
    }

    fn control_plane_tables(connection: &Connection) -> Result<Vec<String>, SqliteError> {
        let mut tables = user_tables(connection)?;
        tables.retain(|table| table != "schema_migrations");
        Ok(tables)
    }

    fn user_tables(connection: &Connection) -> Result<Vec<String>, SqliteError> {
        let mut statement = connection.prepare(
            "SELECT name FROM sqlite_schema \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, SqliteError>>()
    }
}
