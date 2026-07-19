//! Versioned `SQLite` control-plane persistence boundary; never queried by the route hot path.

#![deny(unsafe_code)]

/// AEAD Secret storage, external Master Key loading, and key-rotation primitives.
pub mod control_plane;
pub mod secret_store;

use std::{error::Error, fmt, path::Path};

use rusqlite::{Connection, Error as SqliteError};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-store";

const VERSIONED_CONTROL_PLANE_SCHEMA_VERSION: i64 = 1;
const VERSIONED_ROUTE_ACCESS_SCHEMA_VERSION: i64 = 2;

/// Most recent schema version understood by this build.
pub const CURRENT_SCHEMA_VERSION: i64 = VERSIONED_ROUTE_ACCESS_SCHEMA_VERSION;

const CREATE_SCHEMA_MIGRATIONS: &str = "
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY CHECK (version > 0),
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;
";

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: VERSIONED_CONTROL_PLANE_SCHEMA_VERSION,
        up: include_str!("../migrations/0001_versioned_control_plane.up.sql"),
        down: include_str!("../migrations/0001_versioned_control_plane.down.sql"),
    },
    Migration {
        version: VERSIONED_ROUTE_ACCESS_SCHEMA_VERSION,
        up: include_str!("../migrations/0002_versioned_route_access.up.sql"),
        down: include_str!("../migrations/0002_versioned_route_access.down.sql"),
    },
];

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
    /// A persisted control-plane row violates the Repository's fail-closed decoding rules.
    ///
    /// The table name is structural metadata only; no row contents are included.
    InvalidPersistedControlPlaneRecord {
        /// The table containing the malformed record.
        table: &'static str,
    },
    /// A persisted Client Key digest was not the required opaque 32-byte HMAC value.
    InvalidClientKeyDigestLength {
        /// Observed byte count, never digest contents.
        actual: usize,
    },
    /// A P2-05 mutation attempted to create or change a non-draft Config Version.
    ///
    /// Publication and archival transitions are deferred to the P2-07 Snapshot publisher.
    ControlPlaneMutationRequiresDraft,
    /// A requested Config Version does not exist in the Repository.
    ConfigVersionNotFound,
    /// A requested Config Version is already the sole active Version.
    ConfigVersionAlreadyActive,
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
            Self::InvalidPersistedControlPlaneRecord { table } => {
                write!(
                    formatter,
                    "persisted control-plane record is malformed in table: {table}"
                )
            }
            Self::InvalidClientKeyDigestLength { actual } => {
                write!(
                    formatter,
                    "persisted Client Key digest has invalid length: {actual} bytes"
                )
            }
            Self::ControlPlaneMutationRequiresDraft => {
                formatter.write_str("control-plane mutation requires a draft Config Version")
            }
            Self::ConfigVersionNotFound => {
                formatter.write_str("requested Config Version does not exist")
            }
            Self::ConfigVersionAlreadyActive => {
                formatter.write_str("requested Config Version is already active")
            }
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::ForeignKeysDisabled
            | Self::UnsupportedMigrationHistory { .. }
            | Self::InvalidPersistedControlPlaneRecord { .. }
            | Self::InvalidClientKeyDigestLength { .. }
            | Self::ControlPlaneMutationRequiresDraft
            | Self::ConfigVersionNotFound
            | Self::ConfigVersionAlreadyActive => None,
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

    use rusqlite::{Connection, Error as SqliteError, ffi, params};

    use super::{
        CREATE_SCHEMA_MIGRATIONS, CURRENT_SCHEMA_VERSION, MIGRATIONS,
        VERSIONED_CONTROL_PLANE_SCHEMA_VERSION, migrate, open_in_memory, rollback_all,
        schema_version,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    const TEST_CLIENT_KEY_DIGEST_A: [u8; 32] = [0xA5; 32];
    const TEST_CLIENT_KEY_DIGEST_B: [u8; 32] = [0x5A; 32];

    #[test]
    fn migrations_are_idempotent_and_create_all_p2_schema_tables() -> TestResult {
        let mut connection = Connection::open_in_memory()?;

        migrate(&mut connection)?;
        migrate(&mut connection)?;

        assert_eq!(schema_version(&connection)?, Some(CURRENT_SCHEMA_VERSION));
        assert!(foreign_keys_enabled(&connection)?);
        assert_eq!(
            control_plane_tables(&connection)?,
            vec![
                "access_group_routes",
                "access_groups",
                "client_keys",
                "config_versions",
                "endpoint_credential_bindings",
                "model_aliases",
                "model_routes",
                "public_models",
                "route_candidates",
                "upstream_credentials",
                "upstream_endpoints",
                "upstreams",
            ]
        );
        Ok(())
    }

    #[test]
    fn version_one_database_upgrades_to_version_two_without_rewriting_history() -> TestResult {
        let mut connection = Connection::open_in_memory()?;
        install_version_one_schema(&connection)?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        insert_valid_tree(&connection)?;

        assert_eq!(
            schema_version(&connection)?,
            Some(VERSIONED_CONTROL_PLANE_SCHEMA_VERSION)
        );
        migrate(&mut connection)?;

        assert_eq!(schema_version(&connection)?, Some(CURRENT_SCHEMA_VERSION));
        let upstream_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM upstreams WHERE config_version_id = ?1",
            ["v1"],
            |row| row.get(0),
        )?;
        assert_eq!(upstream_count, 1);
        assert!(super::table_exists(&connection, "client_keys")?);
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
    fn routing_access_rows_reject_missing_references() -> TestResult {
        let mut connection = open_in_memory()?;
        migrate(&mut connection)?;
        insert_valid_tree(&connection)?;
        insert_valid_routing_access_tree(&connection)?;

        let missing_alias_target = connection.execute(
            "INSERT INTO model_aliases (config_version_id, alias, public_model_id) \
             VALUES (?1, ?2, ?3)",
            params!["v1", "missing-model", "missing-public-model"],
        );
        assert!(is_foreign_key_violation(&missing_alias_target));

        let missing_candidate_route = connection.execute(
            "INSERT INTO route_candidates (\
                config_version_id, id, route_id, endpoint_id, upstream_model, credential_scope, \
                transform_mode, enabled, priority, weight, capability_override_json\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                "v1",
                "missing-route-candidate",
                "missing-route",
                "endpoint-a",
                "minimax-m3",
                "endpoint_bindings",
                "passthrough",
                1_i64,
                0_i64,
                1_i64,
                "{}",
            ],
        );
        assert!(is_foreign_key_violation(&missing_candidate_route));
        assert!(foreign_key_check_is_clean(&connection)?);
        Ok(())
    }

    #[test]
    fn routing_access_schema_rejects_duplicate_values_and_invalid_digests() -> TestResult {
        let mut connection = open_in_memory()?;
        migrate(&mut connection)?;
        insert_valid_tree(&connection)?;
        insert_valid_routing_access_tree(&connection)?;

        let duplicate_model_name = connection.execute(
            "INSERT INTO public_models (\
                config_version_id, id, model_name, status, display_name, capabilities_json\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "v1",
                "public-model-duplicate",
                "minimax-m3",
                "active",
                "Duplicate MiniMax M3",
                "{}",
            ],
        );
        assert!(is_uniqueness_violation(&duplicate_model_name));

        let duplicate_alias = connection.execute(
            "INSERT INTO model_aliases (config_version_id, alias, public_model_id) \
             VALUES (?1, ?2, ?3)",
            params!["v1", "mm3", "public-model-a"],
        );
        assert!(is_uniqueness_violation(&duplicate_alias));

        let duplicate_route_for_model = connection.execute(
            "INSERT INTO model_routes (\
                config_version_id, id, public_model_id, policy, max_attempts, bootstrap_timeout_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "v1",
                "route-duplicate",
                "public-model-a",
                "round_robin",
                1_i64,
                1_000_i64,
            ],
        );
        assert!(is_uniqueness_violation(&duplicate_route_for_model));

        let duplicate_candidate = connection.execute(
            "INSERT INTO route_candidates (\
                config_version_id, id, route_id, endpoint_id, upstream_model, credential_scope, \
                transform_mode, enabled, priority, weight, capability_override_json\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                "v1",
                "candidate-duplicate",
                "route-a",
                "endpoint-a",
                "minimax-m3",
                "endpoint_bindings",
                "passthrough",
                1_i64,
                0_i64,
                100_i64,
                "{}",
            ],
        );
        assert!(is_uniqueness_violation(&duplicate_candidate));

        let duplicate_access_route = connection.execute(
            "INSERT INTO access_group_routes (config_version_id, access_group_id, route_id, enabled) \
             VALUES (?1, ?2, ?3, ?4)",
            params!["v1", "access-group-a", "route-a", 1_i64],
        );
        assert!(is_uniqueness_violation(&duplicate_access_route));

        assert_client_key_constraints(&connection);
        assert_route_policy_and_scope_constraints(&connection)?;
        assert!(foreign_key_check_is_clean(&connection)?);
        Ok(())
    }

    fn assert_client_key_constraints(connection: &Connection) {
        let duplicate_client_prefix = connection.execute(
            "INSERT INTO client_keys (\
                config_version_id, id, prefix, secret_digest, access_group_id, status, expires_at_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "v1",
                "client-key-prefix-duplicate",
                "rgw_test_a",
                &TEST_CLIENT_KEY_DIGEST_B,
                "access-group-a",
                "active",
                Option::<i64>::None,
            ],
        );
        assert!(is_uniqueness_violation(&duplicate_client_prefix));

        let duplicate_client_digest = connection.execute(
            "INSERT INTO client_keys (\
                config_version_id, id, prefix, secret_digest, access_group_id, status, expires_at_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "v1",
                "client-key-digest-duplicate",
                "rgw_test_digest_duplicate",
                &TEST_CLIENT_KEY_DIGEST_A,
                "access-group-a",
                "active",
                Option::<i64>::None,
            ],
        );
        assert!(is_uniqueness_violation(&duplicate_client_digest));

        let invalid_digest_length = connection.execute(
            "INSERT INTO client_keys (\
                config_version_id, id, prefix, secret_digest, access_group_id, status, expires_at_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "v1",
                "client-key-invalid-digest",
                "rgw_test_invalid_digest",
                &[1_u8],
                "access-group-a",
                "active",
                Option::<i64>::None,
            ],
        );
        assert!(is_check_violation(&invalid_digest_length));
    }

    fn assert_route_policy_and_scope_constraints(connection: &Connection) -> TestResult {
        connection.execute(
            "INSERT INTO public_models (\
                config_version_id, id, model_name, status, display_name, capabilities_json\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "v1",
                "public-model-policy",
                "policy-test-model",
                "active",
                "Policy Test Model",
                "{}",
            ],
        )?;
        let invalid_policy = connection.execute(
            "INSERT INTO model_routes (\
                config_version_id, id, public_model_id, policy, max_attempts, bootstrap_timeout_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "v1",
                "route-invalid-policy",
                "public-model-policy",
                "least_loaded",
                1_i64,
                1_000_i64,
            ],
        );
        assert!(is_check_violation(&invalid_policy));

        let invalid_scope = connection.execute(
            "INSERT INTO route_candidates (\
                config_version_id, id, route_id, endpoint_id, upstream_model, credential_scope, \
                transform_mode, enabled, priority, weight, capability_override_json\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                "v1",
                "candidate-invalid-scope",
                "route-a",
                "endpoint-a",
                "minimax-m3",
                "specific_credentials",
                "passthrough",
                1_i64,
                0_i64,
                1_i64,
                "{}",
            ],
        );
        assert!(is_check_violation(&invalid_scope));
        Ok(())
    }

    #[test]
    fn migration_up_then_down_restores_the_original_user_tables() -> TestResult {
        let mut connection = open_in_memory()?;
        connection
            .execute_batch("CREATE TABLE caller_owned_baseline (id INTEGER PRIMARY KEY) STRICT;")?;
        let baseline = user_tables(&connection)?;

        migrate(&mut connection)?;
        insert_valid_tree(&connection)?;
        insert_valid_routing_access_tree(&connection)?;
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

    fn insert_valid_routing_access_tree(connection: &Connection) -> TestResult {
        connection.execute(
            "INSERT INTO public_models (\
                config_version_id, id, model_name, status, display_name, capabilities_json\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "v1",
                "public-model-a",
                "minimax-m3",
                "active",
                "MiniMax M3",
                "{\"tools\":true}",
            ],
        )?;
        connection.execute(
            "INSERT INTO model_aliases (config_version_id, alias, public_model_id) \
             VALUES (?1, ?2, ?3)",
            params!["v1", "mm3", "public-model-a"],
        )?;
        connection.execute(
            "INSERT INTO model_routes (\
                config_version_id, id, public_model_id, policy, max_attempts, bootstrap_timeout_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "v1",
                "route-a",
                "public-model-a",
                "smooth_weighted_round_robin",
                3_i64,
                20_000_i64,
            ],
        )?;
        connection.execute(
            "INSERT INTO route_candidates (\
                config_version_id, id, route_id, endpoint_id, upstream_model, credential_scope, \
                transform_mode, enabled, priority, weight, capability_override_json\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                "v1",
                "candidate-a",
                "route-a",
                "endpoint-a",
                "minimax-m3",
                "endpoint_bindings",
                "passthrough",
                1_i64,
                0_i64,
                100_i64,
                "{}",
            ],
        )?;
        connection.execute(
            "INSERT INTO access_groups (config_version_id, id, name, status, limits_json) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "v1",
                "access-group-a",
                "default",
                "active",
                "{\"max_concurrency\":4}",
            ],
        )?;
        connection.execute(
            "INSERT INTO access_group_routes (config_version_id, access_group_id, route_id, enabled) \
             VALUES (?1, ?2, ?3, ?4)",
            params!["v1", "access-group-a", "route-a", 1_i64],
        )?;
        connection.execute(
            "INSERT INTO client_keys (\
                config_version_id, id, prefix, secret_digest, access_group_id, status, expires_at_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "v1",
                "client-key-a",
                "rgw_test_a",
                &TEST_CLIENT_KEY_DIGEST_A,
                "access-group-a",
                "active",
                Option::<i64>::None,
            ],
        )?;
        Ok(())
    }

    fn install_version_one_schema(connection: &Connection) -> Result<(), SqliteError> {
        connection.execute_batch(CREATE_SCHEMA_MIGRATIONS)?;
        connection.execute_batch(MIGRATIONS[0].up)?;
        connection.execute(
            "INSERT INTO schema_migrations (version) VALUES (?1)",
            [VERSIONED_CONTROL_PLANE_SCHEMA_VERSION],
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
            Err(SqliteError::SqliteFailure(code, _))
                if code.extended_code == ffi::SQLITE_CONSTRAINT_FOREIGNKEY
        )
    }

    fn is_uniqueness_violation(result: &Result<usize, SqliteError>) -> bool {
        matches!(
            result,
            Err(SqliteError::SqliteFailure(code, _))
                if code.extended_code == ffi::SQLITE_CONSTRAINT_UNIQUE
                    || code.extended_code == ffi::SQLITE_CONSTRAINT_PRIMARYKEY
        )
    }

    fn is_check_violation(result: &Result<usize, SqliteError>) -> bool {
        matches!(
            result,
            Err(SqliteError::SqliteFailure(code, _))
                if code.extended_code == ffi::SQLITE_CONSTRAINT_CHECK
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
