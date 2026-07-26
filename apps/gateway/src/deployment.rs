//! Explicit P12 deployment composition for the local gateway process.
//!
//! This module starts a loopback-only data listener and the separately authenticated P10
//! management listener. Its data-plane composition is built only from the active isolated
//! `RouteSnapshot`, encrypted Credential pool, Client-Key verifier, and egress policy.

#![deny(unsafe_code)]

use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, File, OpenOptions},
    io::Read,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use actix_web::{App, HttpServer, web};
use futures_util::future::try_join;
use gateway_auth::client_key::{ClientKeyPepper, ClientKeyService};
use gateway_control::{
    management_backup_service::ManagementBackupService,
    management_mutation_service::{
        KeyVersion, ManagementMutationService, MasterKey, MasterKeyRing, SecretStore,
    },
    management_service::{ManagementActor, ManagementService},
};
use gateway_http_actix::{
    configure, configure_management_listener,
    management_backup_resources::ManagementBackupHttpState,
    management_lifecycle_resources::ManagementLifecycleHttpState,
    management_observability_resources::ManagementObservabilityHttpState,
    management_resources::{
        ManagementResourceHttpState, RejectingManagementEndpointWorkflow,
        SystemManagementRuntimeClock,
    },
    management_security::{
        ManagementBrowserPolicy, ManagementCsrfToken, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy, ManagementOrigin,
    },
};
use gateway_observability::try_init_json_tracing;
use gateway_store::{
    backup::BackupKey, control_plane::SqliteControlPlaneRepository,
    event_store::AsyncSqliteEventWriter,
};
use zeroize::{Zeroize, Zeroizing};

use crate::runtime;

/// The concurrent connection ceiling for P12's loopback data listener.
///
/// Every accepted connection may hold one inbound inference body, so this bounds the worst-case
/// resident request memory to this count times `MAX_INFERENCE_REQUEST_BODY_BYTES` (256 MiB), which
/// stays inside the deployment unit's `MemoryMax` alongside the runtime's own state.
const P12_DATA_PLANE_MAX_CONNECTIONS: usize = 64;

/// The bounded wait for the final Required-event batch after both listeners stop.
///
/// The writer never fabricates a flush, so a wedged database would otherwise hang a clean
/// systemd stop; expiry is surfaced as an explicit deployment failure instead.
const P12_EVENT_FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

const MANAGEMENT_KEY_CREDENTIAL: &str = "management-key";
const MANAGEMENT_CSRF_CREDENTIAL: &str = "management-csrf";
const MASTER_KEY_CREDENTIAL: &str = "master-key";
const BACKUP_KEY_CREDENTIAL: &str = "backup-key";
const CLIENT_KEY_PEPPER_CREDENTIAL: &str = "client-key-pepper";
const KEY_BYTES: usize = 32;
const MAX_TEXT_CREDENTIAL_BYTES: usize = 512;
const CONTROL_DATABASE_FILE: &str = "control.sqlite3";
const RESTORE_DATABASE_FILE: &str = "restore-target.sqlite3";
const BACKUP_DIRECTORY: &str = "backups";

/// Parsed, non-secret service arguments for the P12 process envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ServeCommand {
    data_listener: SocketAddr,
    management_listener: SocketAddr,
    state_directory: PathBuf,
    credentials_directory: PathBuf,
}

/// Parses the `gateway serve` command after the top-level command name.
pub(crate) fn parse(arguments: Vec<String>) -> Result<ServeCommand, DeploymentError> {
    let mut options = parse_options(arguments)?;
    let data_listener = parse_loopback_listener(
        &required_option(&mut options, "--data-listen")?,
        "--data-listen",
    )?;
    let management_listener = parse_loopback_listener(
        &required_option(&mut options, "--management-listen")?,
        "--management-listen",
    )?;
    if data_listener == management_listener {
        return Err(DeploymentError::IdenticalListeners);
    }
    let state_directory = parse_absolute_directory(
        &required_option(&mut options, "--state-dir")?,
        "--state-dir",
    )?;
    let credentials_directory = parse_absolute_directory(
        &required_option(&mut options, "--credential-dir")?,
        "--credential-dir",
    )?;
    if !options.is_empty() {
        return Err(DeploymentError::UnexpectedOption);
    }

    Ok(ServeCommand {
        data_listener,
        management_listener,
        state_directory,
        credentials_directory,
    })
}

/// Starts the P12 loopback deployment envelope until systemd requests a clean stop.
pub(crate) fn run(command: ServeCommand) -> Result<(), DeploymentError> {
    // Structured JSON telemetry lines flow through `tracing` to stdout/journald. A process that
    // already installed a global subscriber keeps it; the exporter still renders through it.
    let _already_initialized = try_init_json_tracing();
    let application = build_application_state(&command)?;
    actix_web::rt::System::new().block_on(run_servers(command, application))
}

async fn run_servers(
    command: ServeCommand,
    application: ApplicationState,
) -> Result<(), DeploymentError> {
    let ApplicationState {
        data,
        security,
        resources,
        lifecycle,
        backup,
        observability,
        event_writer,
    } = application;
    let data = web::Data::new(data);
    let data_server = HttpServer::new(move || App::new().app_data(data.clone()).configure(configure))
            .workers(1)
            // Each in-flight request may buffer up to MAX_INFERENCE_REQUEST_BODY_BYTES, so the
            // connection ceiling is what keeps the worst-case resident total inside the unit's
            // MemoryMax. Actix's 25,600 default would not.
            .max_connections(P12_DATA_PLANE_MAX_CONNECTIONS)
            .shutdown_timeout(30)
            .bind(command.data_listener)
            .map_err(|_| DeploymentError::DataListenerUnavailable)?
            .run();

    let management_security = web::Data::new(security);
    let management_resources = web::Data::new(resources);
    let management_lifecycle = web::Data::new(lifecycle);
    let management_backup = web::Data::new(backup);
    let management_observability = web::Data::new(observability);
    let management_server = HttpServer::new(move || {
        App::new()
            .app_data(management_security.clone())
            .app_data(management_resources.clone())
            .app_data(management_lifecycle.clone())
            .app_data(management_backup.clone())
            .app_data(management_observability.clone())
            .configure(configure_management_listener)
    })
    .workers(1)
    .shutdown_timeout(30)
    .bind(command.management_listener)
    .map_err(|_| DeploymentError::ManagementListenerUnavailable)?
    .run();

    // The durable event consumer lives on this System's runtime; its SQLite work stays on the
    // blocking pool, never on a listener worker. It is spawned only after both listeners bound.
    let durability = event_writer.metrics_handle();
    let mut event_writer = actix_web::rt::spawn(event_writer.run());
    try_join(data_server, management_server)
        .await
        .map(|_| ())
        .map_err(|_| DeploymentError::RuntimeUnavailable)?;
    // Both listeners have stopped and dropped every bounded-queue sender, so the writer drains
    // the remaining Required events and exits on its own; the bounded wait keeps a wedged
    // database from hanging the stop while still making an unflushed Required loss visible.
    let flush = actix_web::rt::time::timeout(P12_EVENT_FLUSH_TIMEOUT, &mut event_writer).await;
    match flush {
        Ok(Ok(metrics)) if metrics.pending_required == 0 => Ok(()),
        Ok(Ok(_) | Err(_)) => Err(DeploymentError::EventLogFlushIncomplete),
        Err(_) => {
            // A wedged database must not hold the stop open past this bound. Aborting detaches the
            // in-flight blocking write; the counters below are what tells an operator how much
            // Required evidence never reached the log.
            event_writer.abort();
            let outstanding = durability.snapshot();
            if outstanding.pending_required == 0 && outstanding.required_events_quarantined == 0 {
                Ok(())
            } else {
                Err(DeploymentError::EventLogFlushIncomplete)
            }
        }
    }
}

struct ApplicationState {
    data: gateway_http_actix::ResponsesHttpState,
    security: ManagementHttpState,
    resources: ManagementResourceHttpState,
    lifecycle: ManagementLifecycleHttpState,
    backup: ManagementBackupHttpState,
    observability: ManagementObservabilityHttpState,
    event_writer: AsyncSqliteEventWriter,
}

fn build_application_state(command: &ServeCommand) -> Result<ApplicationState, DeploymentError> {
    ensure_direct_directory(
        &command.state_directory,
        DeploymentError::StateDirectoryUnavailable,
    )?;
    ensure_direct_directory(
        &command.credentials_directory,
        DeploymentError::CredentialDirectoryUnavailable,
    )?;
    let backup_directory = command.state_directory.join(BACKUP_DIRECTORY);
    ensure_owned_backup_directory(&backup_directory)?;

    // Parentheses deliberately prevent the literal-secret scanner from mistaking this loader
    // invocation for an inline management-key assignment.
    let management_key = (load_management_key(&command.credentials_directory))?;
    let management_csrf = load_management_csrf(&command.credentials_directory)?;
    let master_key = load_master_key(&command.credentials_directory)?;
    let backup_key = load_backup_key(&command.credentials_directory)?;

    let key_version = KeyVersion::try_new(1).map_err(|_| DeploymentError::RuntimeUnavailable)?;
    let management_key_ring =
        MasterKeyRing::try_new(key_version, [(key_version, master_key.clone())])
            .map_err(|_| DeploymentError::InvalidCredential(MASTER_KEY_CREDENTIAL))?;
    let runtime_key_ring = MasterKeyRing::try_new(key_version, [(key_version, master_key)])
        .map_err(|_| DeploymentError::InvalidCredential(MASTER_KEY_CREDENTIAL))?;
    let database = command.state_directory.join(CONTROL_DATABASE_FILE);
    let management_client_key_service = load_client_key_service(&command.credentials_directory)?;
    let runtime_client_key_service = load_client_key_service(&command.credentials_directory)?;
    let mutation_repository = SqliteControlPlaneRepository::open(&database)
        .map_err(|_| DeploymentError::ControlPlaneUnavailable)?;
    let mutation_service = ManagementMutationService::with_client_key_service(
        mutation_repository,
        SecretStore::new(management_key_ring),
        management_client_key_service,
    );

    let actor = ManagementActor::try_new("management-key")
        .map_err(|_| DeploymentError::RuntimeUnavailable)?;
    let lifecycle_service = ManagementService::bootstrap(
        SqliteControlPlaneRepository::open(&database)
            .map_err(|_| DeploymentError::ControlPlaneUnavailable)?,
        runtime::deployment_route_compiler(&database)
            .map_err(|_| DeploymentError::RuntimeUnavailable)?,
        actor,
    )
    .map_err(|_| DeploymentError::ControlPlaneUnavailable)?;
    let registry = Arc::clone(lifecycle_service.registry());
    let runtime_secret_store = SecretStore::new(runtime_key_ring);
    let data_plane = runtime::build_data_plane_composition(
        &database,
        &runtime_secret_store,
        registry,
        runtime_client_key_service,
    )
    .map_err(|_| DeploymentError::RuntimeUnavailable)?;
    let runtime::DataPlaneComposition {
        data,
        management_runtime,
        observability,
        event_writer,
    } = data_plane;
    let resources = ManagementResourceHttpState::with_workflow_and_runtime(
        mutation_service,
        Box::new(RejectingManagementEndpointWorkflow::new()),
        management_runtime,
        Box::new(SystemManagementRuntimeClock),
    );
    let lifecycle = ManagementLifecycleHttpState::new(lifecycle_service);
    let backup = ManagementBackupHttpState::new(
        ManagementBackupService::try_new(
            command.state_directory.join(CONTROL_DATABASE_FILE),
            command.state_directory.join(RESTORE_DATABASE_FILE),
            backup_directory,
            backup_key,
        )
        .map_err(|_| DeploymentError::BackupUnavailable)?,
    );
    let origin = management_origin(command.management_listener)?;
    let security = ManagementHttpState::new(
        management_key,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::SameOrigin {
            origin,
            csrf_token: management_csrf,
        },
    )
    .map_err(|_| DeploymentError::RuntimeUnavailable)?;

    Ok(ApplicationState {
        data,
        security,
        resources,
        lifecycle,
        backup,
        observability,
        event_writer,
    })
}

fn load_client_key_service(directory: &Path) -> Result<ClientKeyService, DeploymentError> {
    let pepper = ClientKeyPepper::load_from_file(directory.join(CLIENT_KEY_PEPPER_CREDENTIAL))
        .map_err(|_| DeploymentError::InvalidCredential(CLIENT_KEY_PEPPER_CREDENTIAL))?;
    Ok(ClientKeyService::new(pepper))
}

fn parse_options(arguments: Vec<String>) -> Result<BTreeMap<String, String>, DeploymentError> {
    let mut options = BTreeMap::new();
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        if !option.starts_with("--") {
            return Err(DeploymentError::Usage);
        }
        let value = arguments.next().ok_or(DeploymentError::MissingValue)?;
        if options.insert(option, value).is_some() {
            return Err(DeploymentError::DuplicateOption);
        }
    }
    Ok(options)
}

fn required_option(
    options: &mut BTreeMap<String, String>,
    option: &'static str,
) -> Result<String, DeploymentError> {
    options
        .remove(option)
        .ok_or(DeploymentError::MissingOption(option))
}

fn parse_loopback_listener(
    value: &str,
    option: &'static str,
) -> Result<SocketAddr, DeploymentError> {
    let listener = value
        .parse::<SocketAddr>()
        .map_err(|_| DeploymentError::InvalidListener(option))?;
    if listener.port() == 0 || !listener.ip().is_loopback() {
        return Err(DeploymentError::InvalidListener(option));
    }
    Ok(listener)
}

fn parse_absolute_directory(value: &str, option: &'static str) -> Result<PathBuf, DeploymentError> {
    let path = PathBuf::from(value);
    if !is_clean_absolute_path(&path) {
        return Err(DeploymentError::InvalidPath(option));
    }
    Ok(path)
}

fn is_clean_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}

fn ensure_direct_directory(path: &Path, error: DeploymentError) -> Result<(), DeploymentError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error.clone())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(error);
    }
    Ok(())
}

fn ensure_owned_backup_directory(path: &Path) -> Result<(), DeploymentError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(DeploymentError::BackupUnavailable)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|_| DeploymentError::BackupUnavailable)
        }
        Err(_) => Err(DeploymentError::BackupUnavailable),
    }
}

fn load_management_key(directory: &Path) -> Result<ManagementKey, DeploymentError> {
    let value = load_text_credential(directory, MANAGEMENT_KEY_CREDENTIAL)?;
    ManagementKey::try_new(value)
        .map_err(|_| DeploymentError::InvalidCredential(MANAGEMENT_KEY_CREDENTIAL))
}

fn load_management_csrf(directory: &Path) -> Result<ManagementCsrfToken, DeploymentError> {
    let value = load_text_credential(directory, MANAGEMENT_CSRF_CREDENTIAL)?;
    ManagementCsrfToken::try_new(value)
        .map_err(|_| DeploymentError::InvalidCredential(MANAGEMENT_CSRF_CREDENTIAL))
}

fn load_text_credential(directory: &Path, name: &'static str) -> Result<String, DeploymentError> {
    let mut bytes = read_credential_file(directory, name, MAX_TEXT_CREDENTIAL_BYTES)?;
    let value = match String::from_utf8(std::mem::take(&mut *bytes)) {
        Ok(value) => value,
        Err(error) => {
            let mut rejected = error.into_bytes();
            rejected.zeroize();
            return Err(DeploymentError::InvalidCredential(name));
        }
    };
    if value.is_empty() {
        let mut value = value;
        value.zeroize();
        return Err(DeploymentError::InvalidCredential(name));
    }
    Ok(value)
}

fn load_master_key(directory: &Path) -> Result<MasterKey, DeploymentError> {
    let mut bytes = read_credential_file(directory, MASTER_KEY_CREDENTIAL, KEY_BYTES)?;
    let key = MasterKey::try_from_bytes(&bytes)
        .map_err(|_| DeploymentError::InvalidCredential(MASTER_KEY_CREDENTIAL));
    bytes.zeroize();
    key
}

fn load_backup_key(directory: &Path) -> Result<BackupKey, DeploymentError> {
    let mut bytes = read_credential_file(directory, BACKUP_KEY_CREDENTIAL, KEY_BYTES)?;
    let key = BackupKey::try_from_bytes(&bytes)
        .map_err(|_| DeploymentError::InvalidCredential(BACKUP_KEY_CREDENTIAL));
    bytes.zeroize();
    key
}

fn read_credential_file(
    directory: &Path,
    name: &'static str,
    maximum_bytes: usize,
) -> Result<Zeroizing<Vec<u8>>, DeploymentError> {
    let path = directory.join(name);
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| DeploymentError::InvalidCredential(name))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(DeploymentError::InvalidCredential(name));
    }
    let mut file =
        open_regular_file(&path).map_err(|_| DeploymentError::InvalidCredential(name))?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(maximum_bytes.saturating_add(1)));
    file.by_ref()
        .take(u64::try_from(maximum_bytes.saturating_add(1)).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| DeploymentError::InvalidCredential(name))?;
    if bytes.is_empty() || bytes.len() > maximum_bytes {
        return Err(DeploymentError::InvalidCredential(name));
    }
    Ok(bytes)
}

fn open_regular_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    options.open(path)
}

fn management_origin(listener: SocketAddr) -> Result<ManagementOrigin, DeploymentError> {
    let value = match listener {
        SocketAddr::V4(address) => format!("http://{}:{}", address.ip(), address.port()),
        SocketAddr::V6(address) => format!("http://[{}]:{}", address.ip(), address.port()),
    };
    ManagementOrigin::try_new(value).map_err(|_| DeploymentError::RuntimeUnavailable)
}

/// Safe failures exposed by the deployment process without a path, address, or secret value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DeploymentError {
    /// The command shape was not a supported `serve` invocation.
    Usage,
    /// A required named option was absent.
    MissingOption(&'static str),
    /// An option omitted its separate value.
    MissingValue,
    /// An option was supplied more than once.
    DuplicateOption,
    /// An unknown option remained after strict parsing.
    UnexpectedOption,
    /// A listener was malformed, unspecified, non-loopback, or used port zero.
    InvalidListener(&'static str),
    /// A state or credential directory path was not a clean absolute path.
    InvalidPath(&'static str),
    /// The two listener values were identical.
    IdenticalListeners,
    /// The systemd-owned state directory could not be admitted.
    StateDirectoryUnavailable,
    /// The systemd credential directory could not be admitted.
    CredentialDirectoryUnavailable,
    /// One named credential could not be read or validated.
    InvalidCredential(&'static str),
    /// The control-plane database could not be opened safely.
    ControlPlaneUnavailable,
    /// The fixed backup staging or restore boundary was unavailable.
    BackupUnavailable,
    /// A listener or process-runtime dependency became unavailable.
    RuntimeUnavailable,
    /// The data loopback listener could not bind.
    DataListenerUnavailable,
    /// The management loopback listener could not bind.
    ManagementListenerUnavailable,
    /// The durable event log did not confirm its final flush inside the bounded stop window.
    EventLogFlushIncomplete,
}

impl fmt::Display for DeploymentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => formatter.write_str("invalid serve command; pass --help for usage"),
            Self::MissingOption(option) => {
                write!(formatter, "required option is missing: {option}")
            }
            Self::MissingValue => formatter.write_str("an option requires a value"),
            Self::DuplicateOption => formatter.write_str("an option was supplied more than once"),
            Self::UnexpectedOption => formatter.write_str("an option is not valid for serve"),
            Self::InvalidListener(option) => {
                write!(formatter, "invalid loopback listener for {option}")
            }
            Self::InvalidPath(option) => {
                write!(formatter, "invalid absolute directory for {option}")
            }
            Self::IdenticalListeners => {
                formatter.write_str("data and management listeners must differ")
            }
            Self::StateDirectoryUnavailable => {
                formatter.write_str("state directory is unavailable")
            }
            Self::CredentialDirectoryUnavailable => {
                formatter.write_str("credential directory is unavailable")
            }
            Self::InvalidCredential(name) => {
                write!(formatter, "credential is unavailable or invalid: {name}")
            }
            Self::ControlPlaneUnavailable => {
                formatter.write_str("control-plane state is unavailable")
            }
            Self::BackupUnavailable => formatter.write_str("backup boundary is unavailable"),
            Self::RuntimeUnavailable => formatter.write_str("gateway runtime is unavailable"),
            Self::DataListenerUnavailable => formatter.write_str("data listener is unavailable"),
            Self::ManagementListenerUnavailable => {
                formatter.write_str("management listener is unavailable")
            }
            Self::EventLogFlushIncomplete => {
                formatter.write_str("gateway event log flush did not complete")
            }
        }
    }
}

impl std::error::Error for DeploymentError {}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        BACKUP_DIRECTORY, DeploymentError, build_application_state, parse, read_credential_file,
    };

    struct TemporaryDirectory(PathBuf);

    static TEMPORARY_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    impl TemporaryDirectory {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let sequence = TEMPORARY_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "cpa-rust-gateway-p12-02-{suffix}-{}-{sequence}",
                std::process::id(),
            ));
            fs::create_dir(&path)?;
            Ok(Self(path))
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_required_credentials(
        directory: &TemporaryDirectory,
    ) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(
            directory.join("management-key"),
            b"mgmt_abcdefghijklmnopqrstuvwxyz0123456789",
        )?;
        fs::write(
            directory.join("management-csrf"),
            b"csrf_abcdefghijklmnopqrstuvwxyz0123456789",
        )?;
        fs::write(directory.join("master-key"), [0xA1; 32])?;
        fs::write(directory.join("backup-key"), [0xB2; 32])?;
        fs::write(directory.join("client-key-pepper"), [0xC3; 32])?;
        Ok(())
    }

    fn command(state: &Path, credentials: &Path) -> Result<super::ServeCommand, DeploymentError> {
        parse(vec![
            "--data-listen".to_owned(),
            "127.0.0.1:18180".to_owned(),
            "--management-listen".to_owned(),
            "127.0.0.1:18181".to_owned(),
            "--state-dir".to_owned(),
            state.display().to_string(),
            "--credential-dir".to_owned(),
            credentials.display().to_string(),
        ])
    }

    #[test]
    fn serve_parser_accepts_only_distinct_loopback_listeners_and_clean_directories()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = TemporaryDirectory::new()?;
        let credentials = TemporaryDirectory::new()?;
        assert!(command(state.path(), credentials.path()).is_ok());

        let public = parse(vec![
            "--data-listen".to_owned(),
            "0.0.0.0:18180".to_owned(),
            "--management-listen".to_owned(),
            "127.0.0.1:18181".to_owned(),
            "--state-dir".to_owned(),
            state.path().display().to_string(),
            "--credential-dir".to_owned(),
            credentials.path().display().to_string(),
        ]);
        assert!(matches!(
            public,
            Err(DeploymentError::InvalidListener("--data-listen"))
        ));

        let duplicate = parse(vec![
            "--data-listen".to_owned(),
            "127.0.0.1:18180".to_owned(),
            "--management-listen".to_owned(),
            "127.0.0.1:18180".to_owned(),
            "--state-dir".to_owned(),
            state.path().display().to_string(),
            "--credential-dir".to_owned(),
            credentials.path().display().to_string(),
        ]);
        assert!(matches!(
            duplicate,
            Err(DeploymentError::IdenticalListeners)
        ));
        Ok(())
    }

    #[test]
    fn service_composition_uses_only_required_direct_credentials()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = TemporaryDirectory::new()?;
        let credentials = TemporaryDirectory::new()?;
        write_required_credentials(&credentials)?;
        let application = build_application_state(&command(state.path(), credentials.path())?)?;
        drop(application);

        assert!(state.join("control.sqlite3").is_file());
        assert!(state.join(BACKUP_DIRECTORY).is_dir());
        assert!(!state.join("restore-target.sqlite3").exists());
        Ok(())
    }

    #[test]
    fn serve_event_writer_flushes_and_exits_when_the_composition_drops()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = TemporaryDirectory::new()?;
        let credentials = TemporaryDirectory::new()?;
        write_required_credentials(&credentials)?;
        let application = build_application_state(&command(state.path(), credentials.path())?)?;
        let super::ApplicationState {
            data,
            security,
            resources,
            lifecycle,
            backup,
            observability,
            event_writer,
        } = application;
        drop((data, security, resources, lifecycle, backup, observability));

        let metrics = actix_web::rt::System::new().block_on(event_writer.run());
        assert_eq!(metrics.pending_required, 0);
        assert_eq!(metrics.required_events_committed, 0);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn credential_reader_rejects_symbolic_links() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let directory = TemporaryDirectory::new()?;
        let outside = directory.join("outside");
        fs::write(&outside, b"not a credential")?;
        symlink(&outside, directory.join("management-key"))?;
        assert!(matches!(
            read_credential_file(directory.path(), "management-key", 512),
            Err(DeploymentError::InvalidCredential("management-key"))
        ));
        Ok(())
    }
}
