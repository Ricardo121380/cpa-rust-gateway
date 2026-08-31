//! Versioned `SQLite` control-plane persistence boundary; never queried by the route hot path.

#![deny(unsafe_code)]

/// Encrypted control-plane backup artifacts and empty-target restoration primitives.
pub mod backup;
/// Versioned integer-rate price catalog and idempotent, retention-bounded billing ledger.
pub mod billing_ledger;
/// AEAD Secret storage, external Master Key loading, and key-rotation primitives.
pub mod control_plane;
/// Append-only durable lifecycle event storage and its asynchronous bounded-queue consumer.
pub mod event_store;
pub mod secret_store;
/// Client-key-owned, AEAD-sealed Canonical Responses with bounded TTL and garbage collection.
pub mod stored_response;

use std::{error::Error, fmt, path::Path, time::Duration};

use rusqlite::{Connection, Error as SqliteError};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-store";

const VERSIONED_CONTROL_PLANE_SCHEMA_VERSION: i64 = 1;
const VERSIONED_ROUTE_ACCESS_SCHEMA_VERSION: i64 = 2;
const VERSIONED_EGRESS_POLICY_SCHEMA_VERSION: i64 = 3;
const MANAGEMENT_AUDIT_SCHEMA_VERSION: i64 = 4;
const GATEWAY_EVENT_LOG_SCHEMA_VERSION: i64 = 5;
const GROK_BUILD_CREDENTIAL_RUNTIME_SCHEMA_VERSION: i64 = 6;
const GROK_BUILD_RUNTIME_STATE_SCHEMA_VERSION: i64 = 7;
const CONFIG_VERSION_REVISION_SCHEMA_VERSION: i64 = 8;
const MANAGEMENT_RESOURCE_AUDIT_SCHEMA_VERSION: i64 = 9;
const NATIVE_GROK_ACCOUNT_POOL_SCHEMA_VERSION: i64 = 10;
const NATIVE_GROK_WORKER_STATE_SCHEMA_VERSION: i64 = 11;
const CANONICAL_BRIDGE_TRANSFORM_MODE_SCHEMA_VERSION: i64 = 12;
const NATIVE_GROK_REAUTH_SCHEMA_VERSION: i64 = 13;
const BILLING_LEDGER_SCHEMA_VERSION: i64 = 14;
const BILLING_MATERIALIZER_CHECKPOINT_SCHEMA_VERSION: i64 = 15;
const ROUTING_PRICE_POLICY_SCHEMA_VERSION: i64 = 16;
const STORED_RESPONSE_SCHEMA_VERSION: i64 = 17;
const STORED_RESPONSE_COMPACTION_SCHEMA_VERSION: i64 = 18;
const COMPATIBLE_EGRESS_POOL_SCHEMA_VERSION: i64 = 19;
const GROK_ACCOUNT_ENTITLEMENT_SCHEMA_VERSION: i64 = 20;

/// Most recent schema version understood by this build.
pub const CURRENT_SCHEMA_VERSION: i64 = GROK_ACCOUNT_ENTITLEMENT_SCHEMA_VERSION;

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
    Migration {
        version: VERSIONED_EGRESS_POLICY_SCHEMA_VERSION,
        up: include_str!("../migrations/0003_egress_policy.up.sql"),
        down: include_str!("../migrations/0003_egress_policy.down.sql"),
    },
    Migration {
        version: MANAGEMENT_AUDIT_SCHEMA_VERSION,
        up: include_str!("../migrations/0004_management_audit.up.sql"),
        down: include_str!("../migrations/0004_management_audit.down.sql"),
    },
    Migration {
        version: GATEWAY_EVENT_LOG_SCHEMA_VERSION,
        up: include_str!("../migrations/0005_gateway_event_log.up.sql"),
        down: include_str!("../migrations/0005_gateway_event_log.down.sql"),
    },
    Migration {
        version: GROK_BUILD_CREDENTIAL_RUNTIME_SCHEMA_VERSION,
        up: include_str!("../migrations/0006_grok_build_credential_runtime.up.sql"),
        down: include_str!("../migrations/0006_grok_build_credential_runtime.down.sql"),
    },
    Migration {
        version: GROK_BUILD_RUNTIME_STATE_SCHEMA_VERSION,
        up: include_str!("../migrations/0007_grok_build_runtime_state.up.sql"),
        down: include_str!("../migrations/0007_grok_build_runtime_state.down.sql"),
    },
    Migration {
        version: CONFIG_VERSION_REVISION_SCHEMA_VERSION,
        up: include_str!("../migrations/0008_config_version_revision.up.sql"),
        down: include_str!("../migrations/0008_config_version_revision.down.sql"),
    },
    Migration {
        version: MANAGEMENT_RESOURCE_AUDIT_SCHEMA_VERSION,
        up: include_str!("../migrations/0009_management_resource_audit.up.sql"),
        down: include_str!("../migrations/0009_management_resource_audit.down.sql"),
    },
    Migration {
        version: NATIVE_GROK_ACCOUNT_POOL_SCHEMA_VERSION,
        up: include_str!("../migrations/0010_native_grok_account_pool.up.sql"),
        down: include_str!("../migrations/0010_native_grok_account_pool.down.sql"),
    },
    Migration {
        version: NATIVE_GROK_WORKER_STATE_SCHEMA_VERSION,
        up: include_str!("../migrations/0011_native_grok_worker_state.up.sql"),
        down: include_str!("../migrations/0011_native_grok_worker_state.down.sql"),
    },
    Migration {
        version: CANONICAL_BRIDGE_TRANSFORM_MODE_SCHEMA_VERSION,
        up: include_str!("../migrations/0012_canonical_bridge_transform_mode.up.sql"),
        down: include_str!("../migrations/0012_canonical_bridge_transform_mode.down.sql"),
    },
    Migration {
        version: NATIVE_GROK_REAUTH_SCHEMA_VERSION,
        up: include_str!("../migrations/0013_native_grok_reauth.up.sql"),
        down: include_str!("../migrations/0013_native_grok_reauth.down.sql"),
    },
    Migration {
        version: BILLING_LEDGER_SCHEMA_VERSION,
        up: include_str!("../migrations/0014_billing_ledger.up.sql"),
        down: include_str!("../migrations/0014_billing_ledger.down.sql"),
    },
    Migration {
        version: BILLING_MATERIALIZER_CHECKPOINT_SCHEMA_VERSION,
        up: include_str!("../migrations/0015_billing_materializer_checkpoint.up.sql"),
        down: include_str!("../migrations/0015_billing_materializer_checkpoint.down.sql"),
    },
    Migration {
        version: ROUTING_PRICE_POLICY_SCHEMA_VERSION,
        up: include_str!("../migrations/0016_routing_price_policy.up.sql"),
        down: include_str!("../migrations/0016_routing_price_policy.down.sql"),
    },
    Migration {
        version: STORED_RESPONSE_SCHEMA_VERSION,
        up: include_str!("../migrations/0017_stored_responses.up.sql"),
        down: include_str!("../migrations/0017_stored_responses.down.sql"),
    },
    Migration {
        version: STORED_RESPONSE_COMPACTION_SCHEMA_VERSION,
        up: include_str!("../migrations/0018_stored_response_compactions.up.sql"),
        down: include_str!("../migrations/0018_stored_response_compactions.down.sql"),
    },
    Migration {
        version: COMPATIBLE_EGRESS_POOL_SCHEMA_VERSION,
        up: include_str!("../migrations/0019_compatible_egress_pool.up.sql"),
        down: include_str!("../migrations/0019_compatible_egress_pool.down.sql"),
    },
    Migration {
        version: GROK_ACCOUNT_ENTITLEMENT_SCHEMA_VERSION,
        up: include_str!("../migrations/0020_grok_account_entitlements.up.sql"),
        down: include_str!("../migrations/0020_grok_account_entitlements.down.sql"),
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
    /// A requested downgrade target is not a known applied schema prefix.
    UnsupportedRollbackTarget {
        /// Requested schema version, with zero representing the unmigrated base state.
        target: i64,
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
    /// A management write supplied a stale opaque Config Version revision.
    ConfigVersionRevisionConflict,
    /// A Version-scoped management resource did not exist for a requested mutation.
    ControlPlaneResourceNotFound,
    /// A management audit event did not match its bounded transaction context.
    ///
    /// Event values are deliberately not included because caller-provided actor labels must not
    /// be echoed through storage errors.
    InvalidManagementAuditEvent,
    /// A serialized durable gateway event could not be encoded, decoded, or structurally matched.
    ///
    /// Event contents are deliberately omitted because internal model labels are access-controlled.
    InvalidPersistedGatewayEvent,
    /// A replay reused one stable `(event_type, event_id)` with different event contents.
    ConflictingGatewayEventReplay,
    /// A billing source event was replayed with different value-free identity or usage data.
    ConflictingBillingLedgerReplay,
    /// A persisted billing row or price catalog entry failed the bounded decoding contract.
    InvalidPersistedBillingRecord,
    /// A billing price catalog version already exists with different entries or metadata.
    ConflictingBillingCatalogVersion,
    /// A Config-Version routing price policy failed its bounded typed admission contract.
    InvalidRoutingPricePolicyConfiguration,
    /// A compatible proxy pool, node, or binding profile failed bounded typed validation.
    InvalidCompatibleEgressConfiguration,
    /// A low-priority diagnostic was offered to the durable Required-event store.
    DiagnosticEventNotPersistable,
    /// `PRAGMA quick_check` returned a non-`ok` integrity result.
    GatewayEventLogIntegrityCheckFailed,
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
            Self::UnsupportedRollbackTarget { .. } => {
                formatter.write_str("database rollback target is not a supported applied schema")
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
            Self::ConfigVersionRevisionConflict => {
                formatter.write_str("management Config Version revision does not match")
            }
            Self::ControlPlaneResourceNotFound => {
                formatter.write_str("requested control-plane resource does not exist")
            }
            Self::InvalidManagementAuditEvent => {
                formatter.write_str("management audit event is invalid for this operation")
            }
            Self::InvalidPersistedGatewayEvent => {
                formatter.write_str("persisted gateway event is malformed")
            }
            Self::ConflictingGatewayEventReplay => {
                formatter.write_str("gateway event replay conflicts with an existing durable event")
            }
            Self::ConflictingBillingLedgerReplay => {
                formatter.write_str("billing ledger replay conflicts with an existing source event")
            }
            Self::InvalidPersistedBillingRecord => {
                formatter.write_str("persisted billing record is malformed")
            }
            Self::ConflictingBillingCatalogVersion => {
                formatter.write_str("billing catalog version conflicts with existing entries")
            }
            Self::InvalidRoutingPricePolicyConfiguration => {
                formatter.write_str("routing price policy configuration is invalid")
            }
            Self::InvalidCompatibleEgressConfiguration => {
                formatter.write_str("compatible egress configuration is invalid")
            }
            Self::DiagnosticEventNotPersistable => {
                formatter.write_str("diagnostic events are not persisted in the required event log")
            }
            Self::GatewayEventLogIntegrityCheckFailed => {
                formatter.write_str("gateway event log integrity check failed")
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
            | Self::UnsupportedRollbackTarget { .. }
            | Self::InvalidPersistedControlPlaneRecord { .. }
            | Self::InvalidClientKeyDigestLength { .. }
            | Self::ControlPlaneMutationRequiresDraft
            | Self::ConfigVersionNotFound
            | Self::ConfigVersionAlreadyActive
            | Self::ConfigVersionRevisionConflict
            | Self::ControlPlaneResourceNotFound
            | Self::InvalidManagementAuditEvent
            | Self::InvalidPersistedGatewayEvent
            | Self::ConflictingGatewayEventReplay
            | Self::ConflictingBillingLedgerReplay
            | Self::InvalidPersistedBillingRecord
            | Self::ConflictingBillingCatalogVersion
            | Self::InvalidRoutingPricePolicyConfiguration
            | Self::InvalidCompatibleEgressConfiguration
            | Self::DiagnosticEventNotPersistable
            | Self::GatewayEventLogIntegrityCheckFailed => None,
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
    enable_shared_access(&connection)?;
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
    rollback_to_version(connection, 0)
}

/// Downgrades one supported migration prefix to an explicitly named earlier schema version.
///
/// A target of zero represents the unmigrated user-table base state. The target must be an exact
/// schema version known to this build and may not be newer than the database's currently applied
/// prefix. Every down migration and its bookkeeping deletion commits in its own transaction, so
/// an unsupported history or rejected down step cannot be silently skipped.
///
/// # Errors
///
/// Returns [`StoreError::UnsupportedRollbackTarget`] if `target_version` is unknown, negative, or
/// above the current applied prefix. Returns [`StoreError::UnsupportedMigrationHistory`] rather
/// than guessing when the stored history is not a supported prefix.
pub fn rollback_to_version(connection: &mut Connection, target_version: i64) -> StoreResult<()> {
    enable_foreign_keys(connection)?;
    let applied = applied_migration_versions(connection)?;
    ensure_supported_prefix(&applied)?;

    let target_is_known = target_version == 0
        || MIGRATIONS
            .iter()
            .any(|migration| migration.version == target_version);
    let current_version = applied.last().copied().unwrap_or(0);
    if !target_is_known || target_version < 0 || target_version > current_version {
        return Err(StoreError::UnsupportedRollbackTarget {
            target: target_version,
            applied,
        });
    }

    for migration in MIGRATIONS.iter().take(applied.len()).rev() {
        if migration.version <= target_version {
            break;
        }
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.down)?;
        transaction.execute(
            "DELETE FROM schema_migrations WHERE version = ?1",
            [migration.version],
        )?;
        transaction.commit()?;
    }

    if target_version == 0
        && table_exists(connection, "schema_migrations")?
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

/// The bounded wait a connection spends on a lock held by another connection to this file.
///
/// The serve process runs a continuous durable event writer alongside management reads and
/// control-plane writes against one file. Without a busy handler `SQLite` fails a contended
/// statement immediately, surfacing as an operator-facing error for work that would have
/// succeeded milliseconds later.
const BUSY_TIMEOUT_MILLISECONDS: u32 = 5_000;

/// Enables the journal mode and busy handler that let one writer coexist with concurrent readers.
///
/// Write-ahead logging keeps management reads from blocking the event writer's commits and the
/// reverse; the busy timeout bounds the remaining writer-versus-writer contention. A database that
/// cannot adopt WAL (an unusual filesystem) keeps its previous journal mode rather than failing
/// the open: the busy timeout alone still removes the immediate-failure behavior.
fn enable_shared_access(connection: &Connection) -> StoreResult<()> {
    connection.busy_timeout(Duration::from_millis(u64::from(BUSY_TIMEOUT_MILLISECONDS)))?;
    let _journal_mode: String =
        connection.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    Ok(())
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
        CANONICAL_BRIDGE_TRANSFORM_MODE_SCHEMA_VERSION, COMPATIBLE_EGRESS_POOL_SCHEMA_VERSION,
        CREATE_SCHEMA_MIGRATIONS, CURRENT_SCHEMA_VERSION, MIGRATIONS,
        ROUTING_PRICE_POLICY_SCHEMA_VERSION, STORED_RESPONSE_COMPACTION_SCHEMA_VERSION,
        STORED_RESPONSE_SCHEMA_VERSION, VERSIONED_CONTROL_PLANE_SCHEMA_VERSION, migrate,
        open_in_memory, rollback_all, rollback_to_version, schema_version,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    const TEST_CLIENT_KEY_DIGEST_A: [u8; 32] = [0xA5; 32];
    const TEST_CLIENT_KEY_DIGEST_B: [u8; 32] = [0x5A; 32];

    #[test]
    fn migrations_are_idempotent_and_create_all_known_schema_tables() -> TestResult {
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
                "billing_ledger_entries",
                "billing_materializer_checkpoints",
                "billing_price_catalog_entries",
                "billing_price_catalog_versions",
                "client_keys",
                "compatible_egress_binding_profiles",
                "compatible_egress_proxy_nodes",
                "compatible_egress_proxy_pools",
                "config_versions",
                "egress_policies",
                "endpoint_credential_bindings",
                "gateway_event_log",
                "grok_account_entitlements",
                "grok_account_import_batches",
                "grok_account_links",
                "grok_account_quota_windows",
                "grok_account_reauth_state",
                "grok_accounts",
                "grok_build_affinity_breaks",
                "grok_build_billing_profiles",
                "grok_build_cache_affinities",
                "grok_build_credential_runtime",
                "grok_build_model_catalog",
                "grok_build_quota_windows",
                "grok_build_reasoning_replay",
                "grok_build_response_ownership",
                "management_audit_events",
                "management_resource_audit_events",
                "model_aliases",
                "model_routes",
                "public_models",
                "route_candidates",
                "routing_price_policies",
                "stored_response_compactions",
                "stored_responses",
                "upstream_credentials",
                "upstream_endpoints",
                "upstreams",
            ]
        );
        Ok(())
    }

    #[test]
    fn canonical_bridge_transform_mode_is_persistable() -> TestResult {
        let mut connection = open_in_memory()?;
        migrate(&mut connection)?;
        rollback_to_version(
            &mut connection,
            CANONICAL_BRIDGE_TRANSFORM_MODE_SCHEMA_VERSION - 1,
        )?;
        insert_valid_tree(&connection)?;
        insert_valid_routing_access_tree(&connection)?;

        migrate(&mut connection)?;

        connection.execute(
            "UPDATE route_candidates SET transform_mode = 'canonical_bridge' \
             WHERE config_version_id = 'v1' AND id = 'candidate-a'",
            [],
        )?;
        let mode: String = connection.query_row(
            "SELECT transform_mode FROM route_candidates \
             WHERE config_version_id = 'v1' AND id = 'candidate-a'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(mode, "canonical_bridge");
        Ok(())
    }

    #[test]
    fn routing_price_policy_migration_up_and_down_preserves_prior_schema() -> TestResult {
        let mut connection = open_in_memory()?;
        migrate(&mut connection)?;
        assert!(super::table_exists(&connection, "routing_price_policies")?);

        insert_valid_tree(&connection)?;
        connection.execute(
            "INSERT INTO billing_price_catalog_versions \
             (catalog_version_id, effective_at_ms, source, created_at_ms) \
             VALUES (?1, ?2, ?3, ?4)",
            params!["catalog-v1", 1_i64, "test", 1_i64],
        )?;
        connection.execute(
            "INSERT INTO routing_price_policies \
             (config_version_id, catalog_version_id, comparison) VALUES (?1, ?2, ?3)",
            params!["v1", "catalog-v1", "rate_dominance_v1"],
        )?;

        rollback_to_version(&mut connection, ROUTING_PRICE_POLICY_SCHEMA_VERSION - 1)?;
        assert_eq!(schema_version(&connection)?, Some(15));
        assert!(!super::table_exists(&connection, "routing_price_policies")?);
        assert!(super::table_exists(
            &connection,
            "billing_price_catalog_versions"
        )?);
        let catalog_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM billing_price_catalog_versions \
             WHERE catalog_version_id = 'catalog-v1'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(catalog_count, 1);

        migrate(&mut connection)?;
        assert_eq!(schema_version(&connection)?, Some(CURRENT_SCHEMA_VERSION));
        assert!(super::table_exists(&connection, "routing_price_policies")?);
        let policy_count: i64 =
            connection.query_row("SELECT COUNT(*) FROM routing_price_policies", [], |row| {
                row.get(0)
            })?;
        assert_eq!(policy_count, 0);
        Ok(())
    }

    #[test]
    fn stored_response_migration_up_and_down_preserves_prior_schema() -> TestResult {
        let mut connection = open_in_memory()?;
        migrate(&mut connection)?;
        assert!(super::table_exists(&connection, "stored_responses")?);
        connection.execute(
            "INSERT INTO stored_responses \
             (client_key_id, response_id, created_at_ms, expires_at_ms, payload_version, \
              key_version, ciphertext) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "client-a",
                "resp-a",
                1_i64,
                2_i64,
                1_i64,
                1_i64,
                vec![1_u8; 41],
            ],
        )?;

        rollback_to_version(&mut connection, ROUTING_PRICE_POLICY_SCHEMA_VERSION)?;
        assert_eq!(
            schema_version(&connection)?,
            Some(ROUTING_PRICE_POLICY_SCHEMA_VERSION)
        );
        assert!(!super::table_exists(&connection, "stored_responses")?);
        assert!(super::table_exists(&connection, "routing_price_policies")?);

        migrate(&mut connection)?;
        assert_eq!(schema_version(&connection)?, Some(CURRENT_SCHEMA_VERSION));
        assert!(super::table_exists(&connection, "stored_responses")?);
        let count: i64 =
            connection.query_row("SELECT COUNT(*) FROM stored_responses", [], |row| {
                row.get(0)
            })?;
        assert_eq!(count, 0);
        Ok(())
    }

    #[test]
    fn stored_response_compaction_migration_up_and_down_preserves_responses() -> TestResult {
        let mut connection = open_in_memory()?;
        migrate(&mut connection)?;
        assert!(super::table_exists(
            &connection,
            "stored_response_compactions"
        )?);
        connection.execute(
            "INSERT INTO stored_response_compactions \
             (client_key_id, compact_id, created_at_ms, expires_at_ms, payload_version, \
              key_version, ciphertext) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "client-a",
                "cpar_compact_v1.test",
                1_i64,
                2_i64,
                1_i64,
                1_i64,
                vec![1_u8; 41],
            ],
        )?;

        rollback_to_version(&mut connection, STORED_RESPONSE_SCHEMA_VERSION)?;
        assert_eq!(
            schema_version(&connection)?,
            Some(STORED_RESPONSE_SCHEMA_VERSION)
        );
        assert!(!super::table_exists(
            &connection,
            "stored_response_compactions"
        )?);
        assert!(super::table_exists(&connection, "stored_responses")?);

        migrate(&mut connection)?;
        assert_eq!(schema_version(&connection)?, Some(CURRENT_SCHEMA_VERSION));
        assert!(super::table_exists(
            &connection,
            "stored_response_compactions"
        )?);
        Ok(())
    }

    #[test]
    fn compatible_egress_migration_up_and_down_preserves_prior_schema() -> TestResult {
        let mut connection = open_in_memory()?;
        migrate(&mut connection)?;
        assert!(super::table_exists(
            &connection,
            "compatible_egress_proxy_pools"
        )?);
        assert!(super::table_exists(
            &connection,
            "compatible_egress_proxy_nodes"
        )?);
        assert!(super::table_exists(
            &connection,
            "compatible_egress_binding_profiles"
        )?);

        insert_valid_tree(&connection)?;
        connection.execute(
            "INSERT INTO compatible_egress_proxy_pools \
             (config_version_id, id, upstream_id, name, enabled) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["v1", "pool-a", "upstream-a", "pool", 1_i64],
        )?;

        rollback_to_version(&mut connection, STORED_RESPONSE_COMPACTION_SCHEMA_VERSION)?;
        assert_eq!(
            schema_version(&connection)?,
            Some(STORED_RESPONSE_COMPACTION_SCHEMA_VERSION)
        );
        assert!(!super::table_exists(
            &connection,
            "compatible_egress_proxy_pools"
        )?);
        assert!(!super::table_exists(
            &connection,
            "compatible_egress_proxy_nodes"
        )?);
        assert!(!super::table_exists(
            &connection,
            "compatible_egress_binding_profiles"
        )?);
        assert!(super::table_exists(
            &connection,
            "stored_response_compactions"
        )?);

        migrate(&mut connection)?;
        assert_eq!(schema_version(&connection)?, Some(CURRENT_SCHEMA_VERSION));
        assert!(super::table_exists(
            &connection,
            "compatible_egress_proxy_pools"
        )?);
        let pool_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM compatible_egress_proxy_pools",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(pool_count, 0);
        Ok(())
    }

    #[test]
    fn grok_account_entitlement_migration_is_additive_and_domain_checked() -> TestResult {
        let mut connection = open_in_memory()?;
        migrate(&mut connection)?;
        connection.execute(
            "INSERT INTO grok_account_import_batches \
             (id, status, created_count, unchanged_count, created_at_ms, rolled_back_at_ms) \
             VALUES ('batch-a', 'applied', 1, 0, 1, NULL)",
            [],
        )?;
        connection.execute(
            "INSERT INTO grok_accounts \
             (id, provider, identity_digest, credential_ciphertext, credential_key_version, \
              auth_status, enabled, priority, weight, max_concurrency, refresh_due_at_ms, \
              last_refresh_at_ms, refresh_failure_count, cooldown_until_ms, revision, \
              import_batch_id, created_at_ms, updated_at_ms, quota_sync_due_at_ms) \
             VALUES ('account-a', 'build', ?1, ?2, 1, 'active', 1, 1, 1, 1, NULL, NULL, 0, \
                     NULL, 0, 'batch-a', 1, 1, NULL)",
            params![vec![7_u8; 32], vec![8_u8; 41]],
        )?;
        connection.execute(
            "INSERT INTO grok_account_entitlements \
             (account_id, domain, tier, source, confidence, observed_at_ms) \
             VALUES ('account-a', 'grok_build', 'supergrok', 'provider_subscription', \
                     'authoritative', 2)",
            [],
        )?;
        assert!(
            connection
                .execute(
                    "UPDATE grok_account_entitlements SET domain = 'grok_web', tier = 'super' \
                     WHERE account_id = 'account-a'",
                    [],
                )
                .is_err()
        );

        rollback_to_version(&mut connection, COMPATIBLE_EGRESS_POOL_SCHEMA_VERSION)?;
        assert!(!super::table_exists(
            &connection,
            "grok_account_entitlements"
        )?);
        assert!(super::table_exists(&connection, "grok_accounts")?);
        migrate(&mut connection)?;
        assert_eq!(schema_version(&connection)?, Some(CURRENT_SCHEMA_VERSION));
        Ok(())
    }

    #[test]
    fn version_one_database_upgrades_to_current_without_rewriting_history() -> TestResult {
        let mut connection = Connection::open_in_memory()?;
        install_version_one_schema(&connection)?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        insert_valid_tree(&connection)?;
        connection.execute(
            "UPDATE upstreams SET egress_policy_id = ?1 WHERE config_version_id = ?2 AND id = ?3",
            params!["legacy-policy", "v1", "upstream-a"],
        )?;

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
        assert!(super::table_exists(&connection, "egress_policies")?);
        let legacy_policy: (String, String, String) = connection.query_row(
            "SELECT id, name, allowed_schemes_json FROM egress_policies \
             WHERE config_version_id = ?1 AND id = ?2",
            params!["v1", "legacy-policy"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        assert_eq!(
            legacy_policy,
            (
                "legacy-policy".to_owned(),
                "legacy-unconfigured-legacy-policy".to_owned(),
                "[]".to_owned(),
            )
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

    #[test]
    fn egress_policy_references_are_version_scoped_and_cannot_be_orphaned() -> TestResult {
        let mut connection = open_in_memory()?;
        migrate(&mut connection)?;
        insert_config_version(&connection, "egress-v1")?;
        insert_config_version(&connection, "egress-v2")?;
        insert_valid_egress_policy(&connection, "egress-v1", "policy-a")?;

        let missing_policy = connection.execute(
            "INSERT INTO upstreams (\
                config_version_id, id, name, kind, enabled, tags_json, egress_policy_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "egress-v1",
                "upstream-missing-policy",
                "missing-policy-station",
                "relay",
                1_i64,
                "[]",
                "missing-policy",
            ],
        );
        assert!(is_trigger_violation(&missing_policy));

        connection.execute(
            "INSERT INTO upstreams (\
                config_version_id, id, name, kind, enabled, tags_json, egress_policy_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "egress-v1",
                "upstream-valid-policy",
                "valid-policy-station",
                "relay",
                1_i64,
                "[]",
                "policy-a",
            ],
        )?;

        let cross_version_policy = connection.execute(
            "INSERT INTO upstreams (\
                config_version_id, id, name, kind, enabled, tags_json, egress_policy_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "egress-v2",
                "upstream-cross-version-policy",
                "cross-version-policy-station",
                "relay",
                1_i64,
                "[]",
                "policy-a",
            ],
        );
        assert!(is_trigger_violation(&cross_version_policy));

        let renamed_referenced_policy = connection.execute(
            "UPDATE egress_policies SET id = ?1 WHERE config_version_id = ?2 AND id = ?3",
            params!["policy-renamed", "egress-v1", "policy-a"],
        );
        assert!(is_trigger_violation(&renamed_referenced_policy));

        connection.execute(
            "DELETE FROM egress_policies WHERE config_version_id = ?1 AND id = ?2",
            params!["egress-v1", "policy-a"],
        )?;
        let cleared_reference: Option<String> = connection.query_row(
            "SELECT egress_policy_id FROM upstreams WHERE config_version_id = ?1 AND id = ?2",
            params!["egress-v1", "upstream-valid-policy"],
            |row| row.get(0),
        )?;
        assert!(cleared_reference.is_none());

        connection.execute("DELETE FROM config_versions WHERE id = ?1", ["egress-v1"])?;
        let remaining_upstreams: i64 = connection.query_row(
            "SELECT COUNT(*) FROM upstreams WHERE config_version_id = ?1",
            ["egress-v1"],
            |row| row.get(0),
        )?;
        assert_eq!(remaining_upstreams, 0);
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

    fn insert_config_version(connection: &Connection, id: &str) -> Result<(), SqliteError> {
        connection.execute(
            "INSERT INTO config_versions (id, parent_id, status, created_at_ms, description) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                Option::<&str>::None,
                "draft",
                1_i64,
                "egress policy fixture"
            ],
        )?;
        Ok(())
    }

    fn insert_valid_egress_policy(
        connection: &Connection,
        config_version_id: &str,
        id: &str,
    ) -> Result<(), SqliteError> {
        connection.execute(
            "INSERT INTO egress_policies (\
                config_version_id, id, name, allowed_schemes_json, allowed_hosts_json, \
                allowed_ports_json, allowed_cidrs_json, redirect_mode, max_redirects\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                config_version_id,
                id,
                "egress-policy",
                r#"["https"]"#,
                r#"["api.example.test"]"#,
                "[443]",
                "[]",
                "deny",
                0_i64,
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

    fn is_trigger_violation(result: &Result<usize, SqliteError>) -> bool {
        matches!(
            result,
            Err(SqliteError::SqliteFailure(code, _))
                if code.extended_code == ffi::SQLITE_CONSTRAINT_TRIGGER
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
