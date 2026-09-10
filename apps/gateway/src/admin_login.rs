//! Explicit local-only bootstrap; plaintext never appears in argv, stdout or logs.
use gateway_auth::admin_password::{hash_password, random_secret};
use gateway_store::admin_login::{ADMIN_DATABASE_FILE, AdminAccountStore};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};
use zeroize::Zeroizing;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InitCommand {
    state: PathBuf,
    password_file: PathBuf,
    username: String,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct AdminInitError;
impl fmt::Display for AdminInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("admin-login init requires an existing state directory, a new private password file, and an uninitialized administrator store")
    }
}
impl Error for AdminInitError {}
pub(crate) fn parse(arguments: Vec<String>) -> Result<InitCommand, AdminInitError> {
    let mut args = arguments.into_iter();
    if args.next().as_deref() != Some("init") {
        return Err(AdminInitError);
    }
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        let value = args.next().ok_or(AdminInitError)?;
        if options.insert(key, value).is_some() {
            return Err(AdminInitError);
        }
    }
    let state = PathBuf::from(options.remove("--state-dir").ok_or(AdminInitError)?);
    let password_file = PathBuf::from(options.remove("--password-file").ok_or(AdminInitError)?);
    let username = options
        .remove("--username")
        .unwrap_or_else(|| "admin".into());
    if !options.is_empty()
        || username.is_empty()
        || username.len() > 64
        || !username
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
        || !clean_path(&state)
        || !clean_path(&password_file)
    {
        return Err(AdminInitError);
    }
    Ok(InitCommand {
        state,
        password_file,
        username,
    })
}
fn clean_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
}
pub(crate) fn run(command: &InitCommand) -> Result<(), AdminInitError> {
    let database = command.state.join(ADMIN_DATABASE_FILE);
    if fs::symlink_metadata(&database).is_ok() {
        return Err(AdminInitError);
    }
    for directory in [
        command.state.as_path(),
        command.password_file.parent().ok_or(AdminInitError)?,
    ] {
        let metadata = fs::symlink_metadata(directory).map_err(|_| AdminInitError)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(AdminInitError);
        }
    }
    let password = random_secret("").map_err(|_| AdminInitError)?;
    let hash = Zeroizing::new(hash_password(&password).map_err(|_| AdminInitError)?);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    let mut file = options
        .open(&command.password_file)
        .map_err(|_| AdminInitError)?;
    // Persist the recovery material before creating the account; no lost generated credential.
    file.write_all(password.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|_| AdminInitError)?;
    AdminAccountStore::initialize(database, &command.username, &hash)
        .map_err(|_| AdminInitError)?;
    println!(
        "Administrator initialized; initial password saved to the requested private file. First login requires a password change."
    );
    Ok(())
}
