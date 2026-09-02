//! Local process entry point and the minimal P2-10 management CLI.
//!
//! The `admin` commands are intentionally local and transport-free. P10 owns remote management
//! HTTP/OpenAPI, management authentication, and the Web UI.

#![deny(unsafe_code)]

mod credential_refresh;
mod deployment;
mod grok_admin;
mod provider_account_pool_adapter;
mod provider_egress_status_adapter;
mod runtime;

use std::{collections::BTreeMap, env, error::Error, fmt, process::ExitCode};

use gateway_control::management_service::{
    ConfigVersionId, ManagementActor, ManagementAuditEvent, ManagementService,
    ManagementServiceError,
};

const RELEASE_BUILD_METADATA: &str = concat!(
    "gateway-release-revision=",
    env!("GATEWAY_RELEASE_REVISION"),
    "\n",
    "gateway-release-rust-version=",
    env!("GATEWAY_RELEASE_RUST_VERSION"),
    "\n",
    "gateway-release-target=",
    env!("GATEWAY_RELEASE_TARGET"),
    "\n"
);

// Keep the revision-bound release identity in the stripped executable. P12 verifies this value
// before packaging; the CLI intentionally does not expose a remote version endpoint.
#[used]
static EMBEDDED_RELEASE_BUILD_METADATA: &str = RELEASE_BUILD_METADATA;

fn main() -> ExitCode {
    let _ = [
        gateway_control::COMPONENT,
        gateway_http_actix::COMPONENT,
        gateway_observability::COMPONENT,
    ];
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gateway: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), CliError> {
    if arguments.is_empty()
        || arguments
            .first()
            .is_some_and(|argument| argument == "--help")
        || matches!(arguments.as_slice(), [top_level, action] if matches!(top_level.as_str(), "admin" | "serve") && action == "--help")
    {
        print_usage();
        return Ok(());
    }
    let command = parse_command(arguments)?;
    execute(command)
}

fn execute(command: GatewayCommand) -> Result<(), CliError> {
    match command {
        GatewayCommand::Admin(command) => execute_admin(command),
        GatewayCommand::Serve(command) => deployment::run(command).map_err(CliError::Deployment),
    }
}

fn execute_admin(command: AdminCommand) -> Result<(), CliError> {
    if execute_grok_admin(&command)? {
        return Ok(());
    }
    let actor = ManagementActor::try_new(command.actor().to_owned())
        .map_err(|_| CliError::InvalidValue("--actor"))?;
    let mut service = ManagementService::open_local(command.database(), actor)?;

    match command {
        AdminCommand::Create {
            version,
            parent,
            description,
            ..
        } => {
            let created = service.create_empty_configuration(version, parent, description)?;
            println!(
                "created draft configuration {} (audit #{})",
                created.config_version_id(),
                created.audit_event().id()
            );
        }
        AdminCommand::Validate { version, .. } => {
            let validated = service.validate_configuration(&version)?;
            println!(
                "validated configuration {} as Snapshot {}",
                validated.config_version_id(),
                validated.snapshot_version()
            );
        }
        AdminCommand::Publish { version, .. } => {
            let publication = service.publish_configuration(&version)?;
            let audit_id = publication
                .audit_event()
                .map(ManagementAuditEvent::id)
                .ok_or(CliError::MissingAuditEvent)?;
            println!(
                "published configuration {} (audit #{})",
                publication.transition().current_version(),
                audit_id
            );
        }
        AdminCommand::Rollback { .. } => {
            let publication = service.rollback_configuration()?;
            let audit_id = publication
                .audit_event()
                .map(ManagementAuditEvent::id)
                .ok_or(CliError::MissingAuditEvent)?;
            println!(
                "rolled back to configuration {} (audit #{})",
                publication.transition().current_version(),
                audit_id
            );
        }
        AdminCommand::Audit { .. } => {
            for audit_event in service.audit_events()? {
                let replaced = match audit_event.replaced_config_version_id() {
                    Some(version) => version.as_str(),
                    None => "-",
                };
                println!(
                    "{} {:?} actor={} version={} replaced={} at_ms={}",
                    audit_event.id(),
                    audit_event.action(),
                    audit_event.actor(),
                    audit_event.config_version_id(),
                    replaced,
                    audit_event.occurred_at_ms()
                );
            }
        }
        AdminCommand::GrokImport { .. }
        | AdminCommand::GrokRollback { .. }
        | AdminCommand::GrokProbe { .. }
        | AdminCommand::GrokBuildEntitlementSync { .. } => unreachable!(),
    }
    Ok(())
}

fn execute_grok_admin(command: &AdminCommand) -> Result<bool, CliError> {
    match command {
        AdminCommand::GrokImport {
            database,
            credential_directory,
            batch,
            observed_at_ms,
            ..
        } => {
            grok_admin::import(database, credential_directory, batch, *observed_at_ms)?;
            return Ok(true);
        }
        AdminCommand::GrokRollback {
            database,
            credential_directory,
            batch,
            observed_at_ms,
            ..
        } => {
            grok_admin::rollback(database, credential_directory, batch, *observed_at_ms)?;
            return Ok(true);
        }
        AdminCommand::GrokProbe {
            database,
            credential_directory,
            batch,
            provider,
            observed_at_ms,
            ..
        } => {
            grok_admin::probe(
                database,
                credential_directory,
                batch,
                provider,
                *observed_at_ms,
            )?;
            return Ok(true);
        }
        AdminCommand::GrokBuildEntitlementSync {
            database,
            credential_directory,
            batch,
            observed_at_ms,
            ..
        } => {
            grok_admin::sync_build_entitlement(
                database,
                credential_directory,
                batch,
                *observed_at_ms,
            )?;
            return Ok(true);
        }
        _ => {}
    }
    Ok(false)
}

enum GatewayCommand {
    Admin(AdminCommand),
    Serve(deployment::ServeCommand),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AdminCommand {
    Create {
        database: String,
        actor: String,
        version: ConfigVersionId,
        parent: Option<ConfigVersionId>,
        description: String,
    },
    Validate {
        database: String,
        actor: String,
        version: ConfigVersionId,
    },
    Publish {
        database: String,
        actor: String,
        version: ConfigVersionId,
    },
    Rollback {
        database: String,
        actor: String,
    },
    Audit {
        database: String,
        actor: String,
    },
    GrokImport {
        database: String,
        actor: String,
        credential_directory: String,
        batch: String,
        observed_at_ms: i64,
    },
    GrokRollback {
        database: String,
        actor: String,
        credential_directory: String,
        batch: String,
        observed_at_ms: i64,
    },
    GrokProbe {
        database: String,
        actor: String,
        credential_directory: String,
        batch: String,
        provider: String,
        observed_at_ms: i64,
    },
    GrokBuildEntitlementSync {
        database: String,
        actor: String,
        credential_directory: String,
        batch: String,
        observed_at_ms: i64,
    },
}

impl AdminCommand {
    fn database(&self) -> &str {
        match self {
            Self::Create { database, .. }
            | Self::Validate { database, .. }
            | Self::Publish { database, .. }
            | Self::Rollback { database, .. }
            | Self::Audit { database, .. }
            | Self::GrokImport { database, .. }
            | Self::GrokRollback { database, .. }
            | Self::GrokProbe { database, .. }
            | Self::GrokBuildEntitlementSync { database, .. } => database,
        }
    }

    fn actor(&self) -> &str {
        match self {
            Self::Create { actor, .. }
            | Self::Validate { actor, .. }
            | Self::Publish { actor, .. }
            | Self::Rollback { actor, .. }
            | Self::Audit { actor, .. }
            | Self::GrokImport { actor, .. }
            | Self::GrokRollback { actor, .. }
            | Self::GrokProbe { actor, .. }
            | Self::GrokBuildEntitlementSync { actor, .. } => actor,
        }
    }
}

fn parse_command(arguments: Vec<String>) -> Result<GatewayCommand, CliError> {
    let mut arguments = arguments.into_iter();
    let top_level = arguments.next().ok_or(CliError::Usage)?;
    match top_level.as_str() {
        "admin" => parse_admin_command(arguments.collect()).map(GatewayCommand::Admin),
        "serve" => deployment::parse(arguments.collect())
            .map(GatewayCommand::Serve)
            .map_err(CliError::Deployment),
        _ => Err(CliError::Usage),
    }
}

fn parse_admin_command(arguments: Vec<String>) -> Result<AdminCommand, CliError> {
    let mut arguments = arguments.into_iter();
    let action = arguments.next().ok_or(CliError::Usage)?;
    let mut options = parse_options(arguments.collect())?;
    let database = required_option(&mut options, "--db")?;
    let actor = match options.remove("--actor") {
        Some(actor) => actor,
        None => "local-cli".to_owned(),
    };

    let command = match action.as_str() {
        "create" => {
            let version =
                config_version_id(required_option(&mut options, "--version")?, "--version")?;
            let parent = options
                .remove("--parent")
                .map(|value| config_version_id(value, "--parent"))
                .transpose()?;
            let description = required_option(&mut options, "--description")?;
            AdminCommand::Create {
                database,
                actor,
                version,
                parent,
                description,
            }
        }
        "validate" => AdminCommand::Validate {
            database,
            actor,
            version: config_version_id(required_option(&mut options, "--version")?, "--version")?,
        },
        "publish" => AdminCommand::Publish {
            database,
            actor,
            version: config_version_id(required_option(&mut options, "--version")?, "--version")?,
        },
        "rollback" => AdminCommand::Rollback { database, actor },
        "audit" => AdminCommand::Audit { database, actor },
        "grok-import" => AdminCommand::GrokImport {
            database,
            actor,
            credential_directory: required_option(&mut options, "--credential-dir")?,
            batch: required_option(&mut options, "--batch")?,
            observed_at_ms: parse_i64_option(
                &required_option(&mut options, "--observed-at-ms")?,
                "--observed-at-ms",
            )?,
        },
        "grok-rollback" => AdminCommand::GrokRollback {
            database,
            actor,
            credential_directory: required_option(&mut options, "--credential-dir")?,
            batch: required_option(&mut options, "--batch")?,
            observed_at_ms: parse_i64_option(
                &required_option(&mut options, "--observed-at-ms")?,
                "--observed-at-ms",
            )?,
        },
        "grok-probe" => AdminCommand::GrokProbe {
            database,
            actor,
            credential_directory: required_option(&mut options, "--credential-dir")?,
            batch: required_option(&mut options, "--batch")?,
            provider: required_option(&mut options, "--provider")?,
            observed_at_ms: parse_i64_option(
                &required_option(&mut options, "--observed-at-ms")?,
                "--observed-at-ms",
            )?,
        },
        "grok-build-entitlement-sync" => AdminCommand::GrokBuildEntitlementSync {
            database,
            actor,
            credential_directory: required_option(&mut options, "--credential-dir")?,
            batch: required_option(&mut options, "--batch")?,
            observed_at_ms: parse_i64_option(
                &required_option(&mut options, "--observed-at-ms")?,
                "--observed-at-ms",
            )?,
        },
        _ => return Err(CliError::Usage),
    };
    if options.is_empty() {
        Ok(command)
    } else {
        Err(CliError::UnexpectedOption)
    }
}

fn parse_options(arguments: Vec<String>) -> Result<BTreeMap<String, String>, CliError> {
    let mut options = BTreeMap::new();
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        if !option.starts_with("--") {
            return Err(CliError::Usage);
        }
        let value = arguments.next().ok_or(CliError::MissingValue)?;
        if options.insert(option, value).is_some() {
            return Err(CliError::DuplicateOption);
        }
    }
    Ok(options)
}

fn required_option(
    options: &mut BTreeMap<String, String>,
    option: &'static str,
) -> Result<String, CliError> {
    options
        .remove(option)
        .ok_or(CliError::MissingOption(option))
}

fn config_version_id(value: String, option: &'static str) -> Result<ConfigVersionId, CliError> {
    ConfigVersionId::try_new(value).map_err(|_| CliError::InvalidValue(option))
}

fn parse_i64_option(value: &str, option: &'static str) -> Result<i64, CliError> {
    value.parse().map_err(|_| CliError::InvalidValue(option))
}

fn print_usage() {
    println!(
        "Usage:\n  gateway serve --data-listen <loopback-host:port> --management-listen <loopback-host:port> --state-dir <absolute-dir> --credential-dir <absolute-dir>\n  gateway admin create --db <path> --version <id> --description <text> [--parent <id>] [--actor <label>]\n  gateway admin validate --db <path> --version <id> [--actor <label>]\n  gateway admin publish --db <path> --version <id> [--actor <label>]\n  gateway admin rollback --db <path> [--actor <label>]\n  gateway admin audit --db <path> [--actor <label>]\n  gateway admin grok-import --db <absolute-path> --credential-dir <absolute-dir> --batch <id> --observed-at-ms <unix-ms>\n  gateway admin grok-rollback --db <absolute-path> --credential-dir <absolute-dir> --batch <id> --observed-at-ms <unix-ms>\n  gateway admin grok-probe --db <absolute-path> --credential-dir <absolute-dir> --batch <id> --provider <grok_build|grok_console> --observed-at-ms <unix-ms>\n  gateway admin grok-build-entitlement-sync --db <absolute-path> --credential-dir <absolute-dir> --batch <id> --observed-at-ms <unix-ms>"
    );
    println!(
        "Optional serve flags: [--codex-oauth-proxy socks5://<host>:<port>] [--grok-web-proxy socks5://<host>:<port>] [--grok-web-flaresolverr-proxy socks5://<host>:<port>] [--grok-web-flaresolverr-port <port>]"
    );
}

#[derive(Debug)]
enum CliError {
    Usage,
    MissingOption(&'static str),
    MissingValue,
    DuplicateOption,
    UnexpectedOption,
    InvalidValue(&'static str),
    MissingAuditEvent,
    Deployment(deployment::DeploymentError),
    Management(ManagementServiceError),
    GrokAdmin(grok_admin::GrokAdminError),
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => formatter.write_str("invalid command; pass --help for usage"),
            Self::MissingOption(option) => {
                write!(formatter, "required option is missing: {option}")
            }
            Self::MissingValue => formatter.write_str("an option requires a value"),
            Self::DuplicateOption => formatter.write_str("an option was supplied more than once"),
            Self::UnexpectedOption => {
                formatter.write_str("an option is not valid for this command")
            }
            Self::InvalidValue(option) => write!(formatter, "invalid value for {option}"),
            Self::MissingAuditEvent => {
                formatter.write_str("management mutation did not record an audit event")
            }
            Self::Deployment(error) => write!(formatter, "{error}"),
            Self::Management(error) => write!(formatter, "{error}"),
            Self::GrokAdmin(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Deployment(error) => Some(error),
            Self::Management(error) => Some(error),
            Self::GrokAdmin(error) => Some(error),
            Self::Usage
            | Self::MissingOption(_)
            | Self::MissingValue
            | Self::DuplicateOption
            | Self::UnexpectedOption
            | Self::InvalidValue(_)
            | Self::MissingAuditEvent => None,
        }
    }
}

impl From<ManagementServiceError> for CliError {
    fn from(error: ManagementServiceError) -> Self {
        Self::Management(error)
    }
}

impl From<grok_admin::GrokAdminError> for CliError {
    fn from(error: grok_admin::GrokAdminError) -> Self {
        Self::GrokAdmin(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{AdminCommand, CliError, GatewayCommand, RELEASE_BUILD_METADATA, parse_command};

    #[test]
    fn development_build_embeds_a_non_secret_release_identity() {
        assert!(RELEASE_BUILD_METADATA.contains("gateway-release-revision=development"));
        assert!(RELEASE_BUILD_METADATA.contains("gateway-release-rust-version=development"));
        assert!(RELEASE_BUILD_METADATA.contains("gateway-release-target=development"));
    }

    #[test]
    fn create_command_parses_only_explicit_structured_options() {
        let command = parse_command(vec![
            "admin".to_owned(),
            "create".to_owned(),
            "--db".to_owned(),
            "control.sqlite".to_owned(),
            "--version".to_owned(),
            "version-one".to_owned(),
            "--description".to_owned(),
            "first draft".to_owned(),
            "--actor".to_owned(),
            "operator-a".to_owned(),
        ]);

        assert!(matches!(
            command,
            Ok(GatewayCommand::Admin(AdminCommand::Create {
                database,
                actor,
                version,
                parent: None,
                description,
            })) if database == "control.sqlite"
                && actor == "operator-a"
                && version.as_str() == "version-one"
                && description == "first draft"
        ));
    }

    #[test]
    fn parser_rejects_duplicate_and_unknown_options() {
        let duplicate = parse_command(vec![
            "admin".to_owned(),
            "audit".to_owned(),
            "--db".to_owned(),
            "a.sqlite".to_owned(),
            "--db".to_owned(),
            "b.sqlite".to_owned(),
        ]);
        assert!(matches!(duplicate, Err(CliError::DuplicateOption)));

        let unknown = parse_command(vec![
            "admin".to_owned(),
            "rollback".to_owned(),
            "--db".to_owned(),
            "a.sqlite".to_owned(),
            "--version".to_owned(),
            "version-one".to_owned(),
        ]);
        assert!(matches!(unknown, Err(CliError::UnexpectedOption)));
    }

    #[test]
    fn native_grok_commands_require_explicit_local_inputs() {
        let import = parse_command(vec![
            "admin".to_owned(),
            "grok-import".to_owned(),
            "--db".to_owned(),
            "/state/control.sqlite3".to_owned(),
            "--credential-dir".to_owned(),
            "/run/credentials/gateway".to_owned(),
            "--batch".to_owned(),
            "p12-10g-subset".to_owned(),
            "--observed-at-ms".to_owned(),
            "1".to_owned(),
        ]);
        assert!(matches!(
            import,
            Ok(GatewayCommand::Admin(AdminCommand::GrokImport {
                database,
                credential_directory,
                batch,
                observed_at_ms: 1,
                ..
            })) if database == "/state/control.sqlite3"
                && credential_directory == "/run/credentials/gateway"
                && batch == "p12-10g-subset"
        ));

        let rollback = parse_command(vec![
            "admin".to_owned(),
            "grok-rollback".to_owned(),
            "--db".to_owned(),
            "/state/control.sqlite3".to_owned(),
            "--credential-dir".to_owned(),
            "/run/credentials/gateway".to_owned(),
            "--batch".to_owned(),
            "p12-10g-subset".to_owned(),
            "--observed-at-ms".to_owned(),
            "2".to_owned(),
        ]);
        assert!(matches!(
            rollback,
            Ok(GatewayCommand::Admin(AdminCommand::GrokRollback {
                observed_at_ms: 2,
                ..
            }))
        ));

        let probe = parse_command(vec![
            "admin".to_owned(),
            "grok-probe".to_owned(),
            "--db".to_owned(),
            "/state/control.sqlite3".to_owned(),
            "--credential-dir".to_owned(),
            "/run/credentials/gateway".to_owned(),
            "--batch".to_owned(),
            "p12-10g-subset".to_owned(),
            "--provider".to_owned(),
            "grok_console".to_owned(),
            "--observed-at-ms".to_owned(),
            "3".to_owned(),
        ]);
        assert!(matches!(
            probe,
            Ok(GatewayCommand::Admin(AdminCommand::GrokProbe {
                provider,
                observed_at_ms: 3,
                ..
            })) if provider == "grok_console"
        ));

        let entitlement_sync = parse_command(vec![
            "admin".to_owned(),
            "grok-build-entitlement-sync".to_owned(),
            "--db".to_owned(),
            "/state/control.sqlite3".to_owned(),
            "--credential-dir".to_owned(),
            "/run/credentials/gateway".to_owned(),
            "--batch".to_owned(),
            "local-grok-build".to_owned(),
            "--observed-at-ms".to_owned(),
            "4".to_owned(),
        ]);
        assert!(matches!(
            entitlement_sync,
            Ok(GatewayCommand::Admin(
                AdminCommand::GrokBuildEntitlementSync {
                    batch,
                    observed_at_ms: 4,
                    ..
                }
            )) if batch == "local-grok-build"
        ));
    }
}
