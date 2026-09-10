//! Authentication state is deliberately independent of restorable gateway configuration.
use rusqlite::{Connection, OpenFlags, params};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};
use zeroize::Zeroizing;

/// File held by the gateway service account; never included in configuration export.
pub const ADMIN_DATABASE_FILE: &str = "admin-auth.sqlite3";
/// Safe credential-store failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdminStoreError;
impl fmt::Display for AdminStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("administrator store is unavailable or already initialized")
    }
}
impl Error for AdminStoreError {}
impl From<rusqlite::Error> for AdminStoreError {
    fn from(_: rusqlite::Error) -> Self {
        Self
    }
}

/// Single persisted account. Hashes intentionally have no Debug implementation.
#[derive(Clone)]
pub struct AdminAccount {
    /// Non-secret login identity.
    pub username: String,
    /// Salted PHC string, never a plaintext password.
    pub password_hash: Zeroizing<String>,
    /// Changed atomically with the password.
    pub revision: i64,
    /// Initial password grants only password-change/logout access.
    pub password_change_required: bool,
}

/// A path only; connections and writes are made on the caller's blocking thread.
#[derive(Clone)]
pub struct AdminAccountStore {
    path: PathBuf,
}
impl AdminAccountStore {
    /// Open an existing initialized, private regular file without creating a default account.
    /// # Errors
    /// Missing/unsafe files, unknown schema or malformed singleton rows fail closed.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AdminStoreError> {
        let store = Self {
            path: direct_path(path.as_ref())?,
        };
        store.load()?;
        Ok(store)
    }

    /// Initialize once; never overwrite an existing file, even an empty one.
    /// # Errors
    /// Unsafe directories, existing files, or persistence failure reject initialization.
    pub fn initialize(
        path: impl AsRef<Path>,
        username: &str,
        hash: &str,
    ) -> Result<Self, AdminStoreError> {
        let path = path.as_ref();
        let parent = path.parent().ok_or(AdminStoreError)?;
        let metadata = fs::symlink_metadata(parent).map_err(|_| AdminStoreError)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(AdminStoreError);
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        let file = options.open(path).map_err(|_| AdminStoreError)?;
        file.sync_all().map_err(|_| AdminStoreError)?;
        let store = Self {
            path: direct_path(path)?,
        };
        let mut connection = store.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute_batch("CREATE TABLE admin_account (singleton INTEGER PRIMARY KEY CHECK(singleton = 1), username TEXT NOT NULL, password_hash TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision > 0), password_change_required INTEGER NOT NULL CHECK(password_change_required IN (0,1))) STRICT; PRAGMA user_version = 1;")?;
        transaction.execute(
            "INSERT INTO admin_account VALUES (1, ?1, ?2, 1, 1)",
            params![username, hash],
        )?;
        transaction.commit()?;
        Ok(store)
    }

    fn connection(&self) -> Result<Connection, AdminStoreError> {
        let metadata = fs::symlink_metadata(&self.path).map_err(|_| AdminStoreError)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(AdminStoreError);
        }
        #[cfg(unix)]
        if metadata.mode() & 0o077 != 0 {
            return Err(AdminStoreError);
        }
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.busy_timeout(Duration::from_millis(250))?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        Ok(connection)
    }

    /// Load the singleton account on a blocking thread.
    /// # Errors
    /// Unknown schema and malformed rows are unavailable, never reset automatically.
    pub fn load(&self) -> Result<AdminAccount, AdminStoreError> {
        let connection = self.connection()?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version != 1 {
            return Err(AdminStoreError);
        }
        let account = connection.query_row("SELECT username, password_hash, revision, password_change_required FROM admin_account WHERE singleton=1", [], |row| Ok(AdminAccount {
            username: row.get(0)?, password_hash: Zeroizing::new(row.get(1)?), revision: row.get(2)?, password_change_required: row.get(3)?,
        }))?;
        if account.username.is_empty()
            || account.username.len() > 64
            || account.password_hash.len() > 256
            || account.revision < 1
        {
            return Err(AdminStoreError);
        }
        Ok(account)
    }

    /// Compare-and-swap the hash, clearing initial-password restriction durably.
    /// # Errors
    /// Concurrent changes or storage failures are not replayed.
    pub fn change_password(&self, revision: i64, hash: &str) -> Result<(), AdminStoreError> {
        if revision == i64::MAX {
            return Err(AdminStoreError);
        }
        let changed = self.connection()?.execute("UPDATE admin_account SET password_hash=?1, revision=revision+1, password_change_required=0 WHERE singleton=1 AND revision=?2", params![hash, revision])?;
        if changed != 1 {
            return Err(AdminStoreError);
        }
        Ok(())
    }
}

// Resolve parent aliases (macOS /var -> /private/var) while leaving the final component
// untouched for SQLite's NOFOLLOW admission. A final symlink is still rejected.
fn direct_path(path: &Path) -> Result<PathBuf, AdminStoreError> {
    let parent = path
        .parent()
        .ok_or(AdminStoreError)?
        .canonicalize()
        .map_err(|_| AdminStoreError)?;
    Ok(parent.join(path.file_name().ok_or(AdminStoreError)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[test]
    fn initialization_is_one_time_private_and_password_update_is_compare_and_swap()
    -> Result<(), Box<dyn Error>> {
        let dir = std::env::temp_dir().join(format!(
            "admin-store-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        fs::create_dir(&dir)?;
        let path = dir.join(ADMIN_DATABASE_FILE);
        let store = AdminAccountStore::initialize(&path, "admin", "synthetic-phc")?;
        assert!(AdminAccountStore::initialize(&path, "replacement", "another-hash").is_err());
        store.change_password(1, "new-phc")?;
        assert!(store.change_password(1, "stale-write").is_err());
        assert_eq!(store.load()?.revision, 2);
        assert_eq!(store.load()?.username, "admin");
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt, symlink};
            assert_eq!(fs::metadata(&path)?.mode() & 0o777, 0o600);
            let alias = dir.join("alias.sqlite3");
            symlink(&path, &alias)?;
            assert!(AdminAccountStore::open(&alias).is_err());
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
            assert!(AdminAccountStore::open(&path).is_err());
        }
        fs::remove_dir_all(dir)?;
        Ok(())
    }
}
