//! Explicit export of existing synthetic fixtures for a separately built stock reader.
use std::{
    error::Error, fs, io::Write, os::unix::fs::OpenOptionsExt, os::unix::fs::PermissionsExt,
    path::Path,
};

pub fn export(
    database: &Path,
    name: &str,
    expected: &serde_json::Value,
) -> Result<(), Box<dyn Error>> {
    let root = std::path::PathBuf::from(std::env::var("CPAR66_EXPORT_ROOT")?);
    if !root.is_absolute()
        || root.symlink_metadata()?.file_type().is_symlink()
        || root.metadata()?.permissions().mode() & 0o777 != 0o700
    {
        return Err("fixture export requires an explicit private absolute directory".into());
    }
    let output = root.join(name);
    fs::create_dir(&output)?;
    fs::set_permissions(&output, fs::Permissions::from_mode(0o700))?;
    let connection = gateway_store::open(database)?;
    connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    drop(connection);
    fs::copy(database, output.join("control.sqlite3"))?;
    fs::set_permissions(
        output.join("control.sqlite3"),
        fs::Permissions::from_mode(0o600),
    )?;
    for (filename, bytes) in [
        ("1.key", vec![1_u8; 32]),
        ("expected.json", serde_json::to_vec_pretty(expected)?),
    ] {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(output.join(filename))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    Ok(())
}
