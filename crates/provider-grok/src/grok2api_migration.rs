//! Bounded memory-stream migration from the frozen grok2api account exporter contract.
//!
//! The adapter accepts NDJSON from an already-authenticated source process through `BufRead`; it
//! has no path, database, process-spawn or network API. Account identity and credential bytes are
//! zeroized after validation and are immediately handed to the native CPAR atomic import boundary.

use std::{collections::BTreeMap, error::Error, fmt, io::BufRead};

use serde::{Deserialize, Deserializer};
use zeroize::Zeroizing;

use crate::{
    GrokAccountAuthStatus, GrokAccountCredential, GrokAccountIdentity, GrokAccountImport,
    GrokAccountImportRelation, GrokAccountPoolError, GrokAccountPoolStore, GrokAccountProvider,
    GrokBuildCredential, GrokConsoleSsoToken, GrokWebCredential,
};

/// Maximum encoded NDJSON record size, including its line terminator.
pub const MAX_GROK2API_MIGRATION_RECORD_BYTES: usize = 1024 * 1024;
/// Maximum records accepted in one migration transaction.
pub const MAX_GROK2API_MIGRATION_RECORDS: usize = 8_192;
/// Maximum complete plaintext stream consumed by one migration transaction.
pub const MAX_GROK2API_MIGRATION_STREAM_BYTES: usize = 64 * 1024 * 1024;

/// Value-free migration receipt safe for an operator log.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Grok2ApiMigrationReceipt {
    /// All non-empty NDJSON records observed.
    pub source_records: usize,
    /// Structurally and provider-semantically accepted account records.
    pub accepted_accounts: usize,
    /// Accepted Web records whose source expiry was conservatively reduced to the local maximum.
    pub capped_web_expiries: usize,
    /// Records rejected before any database transaction.
    pub rejected_records: usize,
    /// Accepted Web-to-Build or Web-to-Console links.
    pub accepted_links: usize,
    /// Accounts created by the committed CPAR batch.
    pub created_accounts: usize,
    /// Exact existing CPAR accounts retained unchanged.
    pub unchanged_accounts: usize,
}

/// Stable, value-free migration failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Grok2ApiMigrationFailureKind {
    /// The source pipe could not be read.
    SourceUnavailable,
    /// A line, record count, or complete stream exceeded its fixed bound.
    SourceTooLarge,
    /// At least one record was malformed, unsupported, duplicated or unresolved.
    RejectedRecords,
    /// The atomic CPAR import transaction failed.
    ImportFailed,
}

/// Safe migration failure carrying only counts and a fixed category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Grok2ApiMigrationError {
    kind: Grok2ApiMigrationFailureKind,
    receipt: Grok2ApiMigrationReceipt,
}

impl Grok2ApiMigrationError {
    /// Returns the fixed failure category.
    #[must_use]
    pub const fn kind(&self) -> Grok2ApiMigrationFailureKind {
        self.kind
    }

    /// Returns value-free counts observed before the failure.
    #[must_use]
    pub const fn receipt(&self) -> Grok2ApiMigrationReceipt {
        self.receipt
    }
}

impl fmt::Display for Grok2ApiMigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            Grok2ApiMigrationFailureKind::SourceUnavailable => {
                "grok2api migration source is unavailable"
            }
            Grok2ApiMigrationFailureKind::SourceTooLarge => {
                "grok2api migration source exceeds a fixed bound"
            }
            Grok2ApiMigrationFailureKind::RejectedRecords => {
                "grok2api migration source contains rejected records"
            }
            Grok2ApiMigrationFailureKind::ImportFailed => "grok2api migration transaction failed",
        })
    }
}

impl Error for Grok2ApiMigrationError {}

/// Stateless importer for the root-owned grok2api plaintext pipe contract.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Grok2ApiMemoryStreamMigration;

impl Grok2ApiMemoryStreamMigration {
    /// Consumes, validates and atomically imports one bounded NDJSON stream.
    ///
    /// The source side must decrypt grok2api credentials in its own process and write canonical
    /// provider credential payloads to a root-only pipe. This API never opens a file or source
    /// database. Any rejected record prevents the database transaction entirely.
    ///
    /// # Errors
    ///
    /// Returns only a fixed category and value-free counts. Credential, identity, source reference
    /// and parsing details are never rendered.
    #[allow(clippy::too_many_lines)] // One pass keeps bounded read, zeroization and receipt counts auditable.
    pub fn import<R: BufRead>(
        store: &GrokAccountPoolStore,
        batch_id: &str,
        mut source: R,
        observed_at_ms: i64,
    ) -> Result<Grok2ApiMigrationReceipt, Grok2ApiMigrationError> {
        let mut receipt = Grok2ApiMigrationReceipt::default();
        let mut total_bytes = 0_usize;
        let mut accounts = Vec::new();
        let mut source_refs = BTreeMap::new();
        let mut pending_links = Vec::new();

        loop {
            let mut line = Zeroizing::new(Vec::new());
            let read = std::io::Read::take(
                &mut source,
                (MAX_GROK2API_MIGRATION_RECORD_BYTES + 1) as u64,
            )
            .read_until(b'\n', &mut line)
            .map_err(|_| {
                migration_error(Grok2ApiMigrationFailureKind::SourceUnavailable, receipt)
            })?;
            if read == 0 {
                break;
            }
            total_bytes = total_bytes.checked_add(read).ok_or_else(|| {
                migration_error(Grok2ApiMigrationFailureKind::SourceTooLarge, receipt)
            })?;
            if read > MAX_GROK2API_MIGRATION_RECORD_BYTES
                || total_bytes > MAX_GROK2API_MIGRATION_STREAM_BYTES
            {
                return Err(migration_error(
                    Grok2ApiMigrationFailureKind::SourceTooLarge,
                    receipt,
                ));
            }
            while matches!(line.last(), Some(b'\n' | b'\r')) {
                line.pop();
            }
            if line.is_empty() {
                continue;
            }
            receipt.source_records += 1;
            if receipt.source_records > MAX_GROK2API_MIGRATION_RECORDS {
                return Err(migration_error(
                    Grok2ApiMigrationFailureKind::SourceTooLarge,
                    receipt,
                ));
            }
            match serde_json::from_slice::<TransferRecord>(&line) {
                Ok(TransferRecord::Account {
                    source_ref,
                    provider,
                    identity_key,
                    credential,
                    auth_status,
                    enabled,
                    priority,
                    weight,
                    max_concurrency,
                    refresh_due_at_ms,
                    quota_sync_due_at_ms,
                    cooldown_until_ms,
                }) => {
                    let converted = convert_account(
                        &provider,
                        identity_key.as_str(),
                        credential.as_str(),
                        &auth_status,
                        enabled,
                        priority,
                        weight,
                        max_concurrency,
                        refresh_due_at_ms,
                        quota_sync_due_at_ms,
                        cooldown_until_ms,
                        observed_at_ms,
                    );
                    match converted {
                        Ok((account, capped_web_expiry)) if valid_source_ref(&source_ref) => {
                            match source_refs.entry(source_ref) {
                                std::collections::btree_map::Entry::Vacant(entry) => {
                                    entry.insert(accounts.len());
                                    accounts.push(account);
                                    receipt.accepted_accounts += 1;
                                    receipt.capped_web_expiries += usize::from(capped_web_expiry);
                                }
                                std::collections::btree_map::Entry::Occupied(_) => {
                                    receipt.rejected_records += 1;
                                }
                            }
                        }
                        Ok(_) | Err(()) => receipt.rejected_records += 1,
                    }
                }
                Ok(TransferRecord::Link {
                    source_ref,
                    target_ref,
                    relation,
                }) => pending_links.push((source_ref, target_ref, relation)),
                Err(_) => receipt.rejected_records += 1,
            }
        }

        let mut relations = Vec::new();
        for (source_ref, target_ref, relation) in pending_links {
            let converted = source_refs
                .get(&source_ref)
                .copied()
                .zip(source_refs.get(&target_ref).copied())
                .and_then(|(source_entry, target_entry)| {
                    valid_link(&accounts, source_entry, target_entry, &relation).then_some(
                        GrokAccountImportRelation {
                            source_entry,
                            target_entry,
                            relation,
                        },
                    )
                });
            if let Some(relation) = converted {
                relations.push(relation);
                receipt.accepted_links += 1;
            } else {
                receipt.rejected_records += 1;
            }
        }

        if receipt.rejected_records != 0 || accounts.is_empty() {
            return Err(migration_error(
                Grok2ApiMigrationFailureKind::RejectedRecords,
                receipt,
            ));
        }
        let outcome = store
            .import_batch_with_relations(batch_id, &accounts, &relations, observed_at_ms)
            .map_err(|_: GrokAccountPoolError| {
                migration_error(Grok2ApiMigrationFailureKind::ImportFailed, receipt)
            })?;
        receipt.created_accounts = outcome.created;
        receipt.unchanged_accounts = outcome.unchanged;
        Ok(receipt)
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum TransferRecord {
    #[serde(rename = "account")]
    Account {
        source_ref: String,
        provider: String,
        identity_key: SecretString,
        credential: SecretString,
        auth_status: String,
        enabled: bool,
        priority: i64,
        weight: u32,
        max_concurrency: u32,
        refresh_due_at_ms: Option<i64>,
        quota_sync_due_at_ms: Option<i64>,
        cooldown_until_ms: Option<i64>,
    },
    #[serde(rename = "link")]
    Link {
        source_ref: String,
        target_ref: String,
        relation: String,
    },
}

struct SecretString(Zeroizing<String>);

impl SecretString {
    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|value| Self(Zeroizing::new(value)))
    }
}

#[allow(clippy::too_many_arguments)]
fn convert_account(
    provider: &str,
    identity_key: &str,
    credential: &str,
    auth_status: &str,
    enabled: bool,
    priority: i64,
    weight: u32,
    max_concurrency: u32,
    refresh_due_at_ms: Option<i64>,
    quota_sync_due_at_ms: Option<i64>,
    cooldown_until_ms: Option<i64>,
    observed_at_ms: i64,
) -> Result<(GrokAccountImport, bool), ()> {
    let provider = match provider {
        "grok_build" => GrokAccountProvider::Build,
        "grok_web" => GrokAccountProvider::Web,
        "grok_console" => GrokAccountProvider::Console,
        _ => return Err(()),
    };
    let (credential, capped_web_expiry) = match provider {
        GrokAccountProvider::Build => {
            GrokBuildCredential::import_runtime_json(credential.as_bytes(), observed_at_ms)
                .map_err(|_| ())?;
            (Zeroizing::new(credential.as_bytes().to_vec()), false)
        }
        GrokAccountProvider::Web => GrokWebCredential::normalize_sso_json_for_migration(
            credential.as_bytes(),
            observed_at_ms,
        )
        .map_err(|_| ())?,
        GrokAccountProvider::Console => {
            GrokConsoleSsoToken::try_from_bytes(credential.as_bytes()).map_err(|_| ())?;
            (Zeroizing::new(credential.as_bytes().to_vec()), false)
        }
    };
    let auth_status = match auth_status {
        "active" => GrokAccountAuthStatus::Active,
        "reauthRequired" => GrokAccountAuthStatus::ReauthRequired,
        _ => return Err(()),
    };
    Ok((
        GrokAccountImport {
            provider,
            identity: GrokAccountIdentity::try_from_bytes(identity_key).map_err(|_| ())?,
            credential: GrokAccountCredential::try_from_bytes(credential.as_slice())
                .map_err(|_| ())?,
            auth_status,
            enabled,
            priority,
            weight,
            max_concurrency,
            refresh_due_at_ms,
            quota_sync_due_at_ms,
            cooldown_until_ms,
        },
        capped_web_expiry,
    ))
}

fn valid_link(
    accounts: &[GrokAccountImport],
    source: usize,
    target: usize,
    relation: &str,
) -> bool {
    matches!(
        (
            accounts[source].provider,
            accounts[target].provider,
            relation
        ),
        (
            GrokAccountProvider::Web,
            GrokAccountProvider::Build,
            "web_build"
        ) | (
            GrokAccountProvider::Web,
            GrokAccountProvider::Console,
            "web_console"
        )
    )
}

fn valid_source_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.trim() == value
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

const fn migration_error(
    kind: Grok2ApiMigrationFailureKind,
    receipt: Grok2ApiMigrationReceipt,
) -> Grok2ApiMigrationError {
    Grok2ApiMigrationError { kind, receipt }
}
