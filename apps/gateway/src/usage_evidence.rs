//! Local authorized diagnostic reader. Plaintext goes only to an exclusive private file.
use gateway_core::{AttemptId, ClientKeyId, RequestId};
use gateway_store::{
    messages_summary::SqliteMessagesSummaryReader,
    native_usage::SqliteNativeUsageReader,
    secret_store::{KeyVersion, MasterKeyRing, SecretStore},
};
use rustix::fs::{Mode, OFlags, openat};
use std::{fs::File, io::Write, os::unix::fs::MetadataExt, path::Path};
use zeroize::Zeroizing;

pub(crate) struct Command {
    database: String,
    key_directory: String,
    active: u32,
    owner: ClientKeyId,
    request: RequestId,
    attempt: AttemptId,
    output: String,
}
pub(crate) fn parse(arguments: Vec<String>) -> Result<Command, super::CliError> {
    let mut options = super::parse_options(arguments)?;
    let mut take = |name| super::required_option(&mut options, name);
    let command = Command {
        database: take("--db")?,
        key_directory: take("--master-key-dir")?,
        active: take("--active-master-key-version")?
            .parse()
            .map_err(|_| super::CliError::InvalidValue("--active-master-key-version"))?,
        owner: ClientKeyId::try_new(take("--client-key-id")?)
            .map_err(|_| super::CliError::InvalidValue("--client-key-id"))?,
        request: RequestId::try_new(take("--request-id")?)
            .map_err(|_| super::CliError::InvalidValue("--request-id"))?,
        attempt: AttemptId::try_new(take("--attempt-id")?)
            .map_err(|_| super::CliError::InvalidValue("--attempt-id"))?,
        output: take("--output")?,
    };
    if !options.is_empty() {
        return Err(super::CliError::UnexpectedOption);
    }
    Ok(command)
}

pub(crate) fn run(command: &Command) -> Result<(), ()> {
    run_kind(command, false)
}

pub(crate) fn run_messages_summary(command: &Command) -> Result<(), ()> {
    run_kind(command, true)
}

fn run_kind(command: &Command, summary: bool) -> Result<(), ()> {
    let key = KeyVersion::try_new(command.active).map_err(|_| ())?;
    let secrets = SecretStore::new(
        MasterKeyRing::load_from_directory(&command.key_directory, key).map_err(|_| ())?,
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ())?;
    let now = i64::try_from(now.as_millis()).map_err(|_| ())?;
    let record = if summary {
        SqliteMessagesSummaryReader::open(&command.database, secrets)
            .map_err(|_| ())?
            .get_owned(&command.owner, &command.request, &command.attempt, now)
            .map_err(|_| ())?
            .map(serde_json::to_value)
            .transpose()
            .map_err(|_| ())?
    } else {
        SqliteNativeUsageReader::open(&command.database, secrets)
            .map_err(|_| ())?
            .get_owned(&command.owner, &command.request, &command.attempt, now)
            .map_err(|_| ())?
            .map(serde_json::to_value)
            .transpose()
            .map_err(|_| ())?
    };
    let result = match record {
        Some(record) => serde_json::json!({"status":"retained","record":record}),
        None => serde_json::json!({"status":"unknown"}),
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&result).map_err(|_| ())?);
    write_private(Path::new(&command.output), bytes.as_slice())?;
    println!("evidence diagnostic written to private output");
    Ok(())
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), ()> {
    if !path.is_absolute() {
        return Err(());
    }
    let parent = path.parent().ok_or(())?;
    if parent.canonicalize().map_err(|_| ())? != parent {
        return Err(());
    }
    let directory = File::open(parent).map_err(|_| ())?;
    let metadata = directory.metadata().map_err(|_| ())?;
    if !metadata.is_dir() || metadata.mode() & 0o777 != 0o700 {
        return Err(());
    }
    let name = path.file_name().ok_or(())?;
    let fd = openat(
        &directory,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| ())?;
    let mut output = File::from(fd);
    let created = output.metadata().map_err(|_| ())?;
    if created.uid() != metadata.uid() || created.nlink() != 1 || created.mode() & 0o777 != 0o600 {
        return Err(());
    }
    output
        .write_all(bytes)
        .and_then(|()| output.sync_all())
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn diagnostic_options_are_explicit_and_closed() {
        assert!(parse(Vec::new()).is_err());
        let options: BTreeMap<_, _> = [
            ("--db", "/tmp/unused"),
            ("--master-key-dir", "/tmp/unused-keys"),
            ("--active-master-key-version", "1"),
            ("--client-key-id", "owner"),
            ("--request-id", "request"),
            ("--attempt-id", "attempt"),
            ("--output", "/tmp/private/result.json"),
        ]
        .into_iter()
        .collect();
        assert!(
            parse(
                options
                    .into_iter()
                    .flat_map(|(a, b)| [a.into(), b.into()])
                    .collect()
            )
            .is_ok()
        );
    }
}
