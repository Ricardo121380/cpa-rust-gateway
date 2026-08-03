//! Root-only, transport-free native Grok account migration commands.

use std::{
    error::Error,
    fmt, fs,
    io::{self, BufReader, IsTerminal},
    path::{Path, PathBuf},
};

use crate::deployment;
use gateway_store::secret_store::{KeyVersion, MasterKeyRing, SecretStore};
use provider_grok::{
    Grok2ApiMemoryStreamMigration, Grok2ApiMigrationError, GrokAccountPoolError,
    GrokAccountPoolStore,
};

/// Safe, value-free failure for a local migration operation.
#[derive(Debug)]
pub(crate) enum GrokAdminError {
    RootRequired,
    InteractiveInputRejected,
    InvalidPath,
    CredentialUnavailable,
    StoreUnavailable,
    Migration(Grok2ApiMigrationError),
    Rollback(GrokAccountPoolError),
}

impl fmt::Display for GrokAdminError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RootRequired => "native Grok migration requires effective uid 0",
            Self::InteractiveInputRejected => {
                "native Grok migration requires a non-terminal stdin pipe"
            }
            Self::InvalidPath => "native Grok migration path is unavailable or unsafe",
            Self::CredentialUnavailable => "native Grok migration key is unavailable",
            Self::StoreUnavailable => "native Grok migration store is unavailable",
            Self::Migration(error) => {
                let receipt = error.receipt();
                return write!(
                    formatter,
                    "native Grok migration failed: category={:?} source_records={} accepted_accounts={} rejected_records={} accepted_links={}",
                    error.kind(),
                    receipt.source_records,
                    receipt.accepted_accounts,
                    receipt.rejected_records,
                    receipt.accepted_links,
                );
            }
            Self::Rollback(error) => {
                return write!(
                    formatter,
                    "native Grok migration rollback failed: category={error:?}"
                );
            }
        })
    }
}

impl Error for GrokAdminError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Migration(error) => Some(error),
            Self::Rollback(error) => Some(error),
            _ => None,
        }
    }
}

pub(crate) fn import(
    database: &str,
    credential_directory: &str,
    batch: &str,
    observed_at_ms: i64,
) -> Result<(), GrokAdminError> {
    require_root()?;
    if io::stdin().is_terminal() {
        return Err(GrokAdminError::InteractiveInputRejected);
    }
    let store = open_store(database, credential_directory)?;
    let receipt = Grok2ApiMemoryStreamMigration::import(
        &store,
        batch,
        BufReader::new(io::stdin().lock()),
        observed_at_ms,
    )
    .map_err(GrokAdminError::Migration)?;
    println!(
        "native_grok_import=PASS source_records={} accepted_accounts={} rejected_records={} accepted_links={} created_accounts={} unchanged_accounts={}",
        receipt.source_records,
        receipt.accepted_accounts,
        receipt.rejected_records,
        receipt.accepted_links,
        receipt.created_accounts,
        receipt.unchanged_accounts,
    );
    Ok(())
}

pub(crate) fn rollback(
    database: &str,
    credential_directory: &str,
    batch: &str,
    observed_at_ms: i64,
) -> Result<(), GrokAdminError> {
    require_root()?;
    let store = open_store(database, credential_directory)?;
    let outcome = store
        .rollback_import_batch(batch, observed_at_ms)
        .map_err(GrokAdminError::Rollback)?;
    println!(
        "native_grok_rollback=PASS removed_accounts={} already_rolled_back={}",
        outcome.removed, outcome.already_rolled_back,
    );
    Ok(())
}

fn open_store(
    database: &str,
    credential_directory: &str,
) -> Result<GrokAccountPoolStore, GrokAdminError> {
    let database = direct_regular_file(database)?;
    let credential_directory = direct_directory(credential_directory)?;
    let master_key = deployment::load_master_key(&credential_directory)
        .map_err(|_| GrokAdminError::CredentialUnavailable)?;
    let key_version = KeyVersion::try_new(1).map_err(|_| GrokAdminError::CredentialUnavailable)?;
    let key_ring = MasterKeyRing::try_new(key_version, [(key_version, master_key)])
        .map_err(|_| GrokAdminError::CredentialUnavailable)?;
    GrokAccountPoolStore::try_open(database, SecretStore::new(key_ring))
        .map_err(|_| GrokAdminError::StoreUnavailable)
}

fn direct_regular_file(value: &str) -> Result<PathBuf, GrokAdminError> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(GrokAdminError::InvalidPath);
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| GrokAdminError::InvalidPath)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(GrokAdminError::InvalidPath);
    }
    Ok(path)
}

fn direct_directory(value: &str) -> Result<PathBuf, GrokAdminError> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(GrokAdminError::InvalidPath);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| GrokAdminError::InvalidPath)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(GrokAdminError::InvalidPath);
    }
    Ok(path.to_path_buf())
}

fn require_root() -> Result<(), GrokAdminError> {
    let status =
        fs::read_to_string("/proc/self/status").map_err(|_| GrokAdminError::RootRequired)?;
    let effective_uid = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|uids| uids.split_whitespace().nth(1))
        .and_then(|uid| uid.parse::<u32>().ok());
    if effective_uid == Some(0) {
        Ok(())
    } else {
        Err(GrokAdminError::RootRequired)
    }
}
