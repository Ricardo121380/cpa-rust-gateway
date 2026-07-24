//! Local process entry point and the minimal P2-10 management CLI.
//!
//! The `admin` commands are intentionally local and transport-free. P10 owns remote management
//! HTTP/OpenAPI, management authentication, and the Web UI.

#![deny(unsafe_code)]

mod deployment;

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
    }
    Ok(())
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
}

impl AdminCommand {
    fn database(&self) -> &str {
        match self {
            Self::Create { database, .. }
            | Self::Validate { database, .. }
            | Self::Publish { database, .. }
            | Self::Rollback { database, .. }
            | Self::Audit { database, .. } => database,
        }
    }

    fn actor(&self) -> &str {
        match self {
            Self::Create { actor, .. }
            | Self::Validate { actor, .. }
            | Self::Publish { actor, .. }
            | Self::Rollback { actor, .. }
            | Self::Audit { actor, .. } => actor,
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

fn print_usage() {
    println!(
        "Usage:\n  gateway serve --data-listen <loopback-host:port> --management-listen <loopback-host:port> --state-dir <absolute-dir> --credential-dir <absolute-dir>\n  gateway admin create --db <path> --version <id> --description <text> [--parent <id>] [--actor <label>]\n  gateway admin validate --db <path> --version <id> [--actor <label>]\n  gateway admin publish --db <path> --version <id> [--actor <label>]\n  gateway admin rollback --db <path> [--actor <label>]\n  gateway admin audit --db <path> [--actor <label>]"
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
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Deployment(error) => Some(error),
            Self::Management(error) => Some(error),
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
}
