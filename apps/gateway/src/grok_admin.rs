//! Root-only native Grok account migration and bounded live-probe commands.

use std::{
    collections::BTreeSet,
    error::Error,
    fmt, fs,
    io::{self, BufReader, IsTerminal},
    net::IpAddr,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::deployment;
use gateway_core::{
    CanonicalRequest, CanonicalResponse, EgressPolicyId, EndpointId, ErrorScope, GatewayError,
    GatewayErrorCode, RequestContext, RequestId,
};
use gateway_router::{
    ProtocolFormat, ProtocolResponseRejection, RuntimeCredentialAccountStatus,
    RuntimeHealthRegistry, RuntimeQuotaRegistry, project_protocol_response,
};
use gateway_store::secret_store::{KeyVersion, MasterKeyRing, SecretStore};
use gateway_upstream::{
    EgressAdmissionErrorCode, EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost,
    EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy, SystemEgressDnsResolver,
    UpstreamClientPool, UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};
use protocol_openai_responses::{ResponseMode, decode_request};
use provider_grok::{
    GROK_CONSOLE_RESPONSES_URL, Grok2ApiMemoryStreamMigration, Grok2ApiMigrationError,
    GrokAccountEndpointBinding, GrokAccountPoolError, GrokAccountPoolStore, GrokAccountProvider,
    GrokBuildCredential, GrokBuildExecutionMode, GrokBuildInferenceAdapter,
    GrokBuildUpstreamTransport, GrokConsoleExecutionMode, GrokConsoleInferenceAdapter,
    GrokConsoleRequestError, GrokConsoleResponsesRequestBuilder, GrokConsoleSsoToken,
    GrokConsoleUpstreamTransport,
};
use provider_kiro::InferenceAdapter;

/// Safe, value-free failure for a local migration operation.
#[derive(Debug)]
pub(crate) enum GrokAdminError {
    RootRequired,
    InteractiveInputRejected,
    InvalidPath,
    CredentialUnavailable,
    StoreUnavailable,
    ProbeRejected,
    ProbeUnavailable,
    ProbeGateway {
        stage: &'static str,
        code: GatewayErrorCode,
        scope: ErrorScope,
    },
    ProbeEgress {
        code: EgressAdmissionErrorCode,
    },
    ProbeConsoleTransport {
        category: GrokConsoleRequestError,
    },
    ProbeProjection {
        chat: ProtocolResponseRejection,
        responses: ProtocolResponseRejection,
        messages: ProtocolResponseRejection,
    },
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
            Self::ProbeRejected => "native Grok probe request was rejected",
            Self::ProbeUnavailable => "native Grok probe failed",
            Self::ProbeGateway { stage, code, scope } => {
                return write!(
                    formatter,
                    "native Grok probe failed: stage={stage} code={code:?} scope={scope:?}"
                );
            }
            Self::ProbeEgress { code } => {
                return write!(
                    formatter,
                    "native Grok probe failed: stage=admission category={code:?}"
                );
            }
            Self::ProbeConsoleTransport { category } => {
                return write!(
                    formatter,
                    "native Grok probe failed: stage=transport_prepare category={category:?}"
                );
            }
            Self::ProbeProjection {
                chat,
                responses,
                messages,
            } => {
                return write!(
                    formatter,
                    "native Grok probe failed: stage=projection chat={chat:?} responses={responses:?} messages={messages:?}"
                );
            }
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

pub(crate) fn probe(
    database: &str,
    credential_directory: &str,
    batch: &str,
    provider: &str,
    observed_at_ms: i64,
) -> Result<(), GrokAdminError> {
    require_root()?;
    let provider = match provider {
        "grok_build" => GrokAccountProvider::Build,
        "grok_console" => GrokAccountProvider::Console,
        _ => return Err(GrokAdminError::ProbeRejected),
    };
    let store = open_store(database, credential_directory)?;
    let accounts = store
        .list_accounts()
        .map_err(|_| GrokAdminError::ProbeUnavailable)?;
    let selected = accounts
        .iter()
        .filter(|account| account.import_batch_id == batch && account.provider == provider)
        .collect::<Vec<_>>();
    if selected.len() != 1 || !selected[0].enabled {
        return Err(GrokAdminError::ProbeRejected);
    }
    let selected = selected[0];
    let endpoint = EndpointId::try_new("p12-10g-native-grok".to_owned())
        .map_err(|_| GrokAdminError::ProbeUnavailable)?;
    let compilation = store
        .compile_native_runtime(
            &[GrokAccountEndpointBinding::new(provider, endpoint.clone())],
            observed_at_ms,
        )
        .map_err(|_| GrokAdminError::ProbeUnavailable)?;
    if compilation.account_count() != 1 {
        return Err(GrokAdminError::ProbeRejected);
    }
    let health = RuntimeHealthRegistry::new();
    let quota = RuntimeQuotaRegistry::new();
    compilation
        .seed_runtime_health(&health)
        .map_err(|_| GrokAdminError::ProbeUnavailable)?;
    compilation
        .seed_runtime_quota(&quota)
        .map_err(|_| GrokAdminError::ProbeUnavailable)?;
    let lease = compilation
        .credential_pools()
        .try_lease(&endpoint)
        .ok_or(GrokAdminError::ProbeRejected)?;
    if lease.credential_id().as_str() != selected.id
        || health
            .credential_account_status_at(&endpoint, lease.credential_id(), observed_at_ms)
            .map_err(|_| GrokAdminError::ProbeUnavailable)?
            != RuntimeCredentialAccountStatus::Available
        || !quota.endpoint_credential_is_available(&endpoint, lease.credential_id())
    {
        return Err(GrokAdminError::ProbeRejected);
    }
    let credential = store
        .open_credential(&selected.id)
        .map_err(|_| GrokAdminError::ProbeUnavailable)?;
    let response = actix_web::rt::System::new().block_on(execute_probe(
        provider,
        credential.as_bytes(),
        observed_at_ms,
    ))?;
    let chat = project_protocol_response(&response, ProtocolFormat::OpenAiChatCompletions);
    let responses = project_protocol_response(&response, ProtocolFormat::OpenAiResponses);
    let messages = project_protocol_response(&response, ProtocolFormat::AnthropicMessages);
    if let (Err(chat), Err(responses), Err(messages)) = (&chat, &responses, &messages) {
        return Err(GrokAdminError::ProbeProjection {
            chat: *chat,
            responses: *responses,
            messages: *messages,
        });
    }
    if chat.is_err() || responses.is_err() || messages.is_err() {
        return Err(GrokAdminError::ProbeUnavailable);
    }
    let provider_label = match provider {
        GrokAccountProvider::Build => "grok_build",
        GrokAccountProvider::Console => "grok_console",
        GrokAccountProvider::Web => return Err(GrokAdminError::ProbeRejected),
    };
    println!(
        "native_grok_probe=PASS provider={provider_label} account_attributed=true health=available quota=available canonical_complete=true chat=true responses=true messages=true"
    );
    Ok(())
}

async fn execute_probe(
    provider: GrokAccountProvider,
    credential: &[u8],
    observed_at_ms: i64,
) -> Result<CanonicalResponse, GrokAdminError> {
    let request = probe_request(provider)?;
    let context = RequestContext::new(
        RequestId::try_new("p12-10g-native-grok-probe")
            .map_err(|_| GrokAdminError::ProbeUnavailable)?,
    );
    let pool =
        UpstreamClientPool::new(NonZeroUsize::new(1).ok_or(GrokAdminError::ProbeUnavailable)?);
    let profile = UpstreamTransportProfile::new(
        UpstreamTimeouts::try_new(
            Duration::from_secs(10),
            Duration::from_mins(1),
            Duration::from_mins(1),
            Duration::from_mins(3),
        )
        .map_err(|_| GrokAdminError::ProbeUnavailable)?,
        UpstreamProxy::Direct,
        NonZeroUsize::new(1).ok_or(GrokAdminError::ProbeUnavailable)?,
    );
    let mut source = match provider {
        GrokAccountProvider::Build => {
            let (policy, resolver) = probe_egress(
                "cli-chat-proxy.grok.com",
                "https://cli-chat-proxy.grok.com/v1/responses",
            )?;
            let adapter = GrokBuildInferenceAdapter::try_new(
                GrokBuildCredential::import_runtime_json(credential, observed_at_ms)
                    .map_err(|_| GrokAdminError::ProbeUnavailable)?,
                "grok-4.5",
                GrokBuildExecutionMode::NonStreaming,
                Arc::new(GrokBuildUpstreamTransport::new(
                    policy, resolver, pool, profile,
                )),
            )
            .map_err(|_| GrokAdminError::ProbeUnavailable)?;
            adapter
                .execute(context, request)
                .await
                .map_err(|error| probe_gateway("start", &error))?
        }
        GrokAccountProvider::Console => {
            let (policy, resolver) = probe_egress("console.x.ai", GROK_CONSOLE_RESPONSES_URL)?;
            let token = GrokConsoleSsoToken::try_from_bytes(credential)
                .map_err(|_| GrokAdminError::ProbeUnavailable)?;
            let outbound = GrokConsoleResponsesRequestBuilder::build(
                &token,
                "grok-build-0.1",
                &request,
                ResponseMode::NonStreaming,
            )
            .map_err(|category| GrokAdminError::ProbeConsoleTransport { category })?;
            let admitted = policy
                .admit_url(outbound.url(), resolver.as_ref())
                .map_err(|error| GrokAdminError::ProbeEgress { code: error.code() })?;
            outbound
                .into_transport_request(admitted)
                .map_err(|category| GrokAdminError::ProbeConsoleTransport { category })?;
            let adapter = GrokConsoleInferenceAdapter::try_new(
                token,
                "grok-build-0.1",
                GrokConsoleExecutionMode::NonStreaming,
                Arc::new(GrokConsoleUpstreamTransport::new(
                    policy, resolver, pool, profile,
                )),
            )
            .map_err(|_| GrokAdminError::ProbeUnavailable)?;
            adapter
                .execute(context, request)
                .await
                .map_err(|error| probe_gateway("start", &error))?
        }
        GrokAccountProvider::Web => return Err(GrokAdminError::ProbeRejected),
    };
    let mut events = Vec::new();
    while let Some(event) = source
        .next_event()
        .await
        .map_err(|error| probe_gateway("stream", &error))?
    {
        events.push(event);
    }
    CanonicalResponse::try_new(events).map_err(|_| GrokAdminError::ProbeUnavailable)
}

fn probe_request(provider: GrokAccountProvider) -> Result<CanonicalRequest, GrokAdminError> {
    let body = match provider {
        GrokAccountProvider::Build => {
            r#"{"model":"cpar-native-grok","input":"Reply with exactly: ready","max_output_tokens":32}"#
        }
        // Console owns a fixed, model-specific output ceiling and rejects an inbound Responses
        // extension that its Canonical builder cannot prove it preserves.
        GrokAccountProvider::Console => {
            r#"{"model":"cpar-native-grok","input":"Reply with exactly: ready"}"#
        }
        GrokAccountProvider::Web => return Err(GrokAdminError::ProbeRejected),
    };
    decode_request(body)
        .map(|decoded| decoded.request)
        .map_err(|_| GrokAdminError::ProbeUnavailable)
}

fn probe_gateway(stage: &'static str, error: &GatewayError) -> GrokAdminError {
    GrokAdminError::ProbeGateway {
        stage,
        code: error.code(),
        scope: error.scope(),
    }
}

fn probe_egress(
    host: &str,
    target_url: &str,
) -> Result<(EgressPolicy, Arc<dyn EgressDnsResolver>), GrokAdminError> {
    let host = EgressHost::try_new(host).map_err(|_| GrokAdminError::ProbeUnavailable)?;
    let addresses = SystemEgressDnsResolver
        .resolve(&host)
        .map_err(|_| GrokAdminError::ProbeUnavailable)?;
    if addresses.is_empty() {
        return Err(GrokAdminError::ProbeUnavailable);
    }
    let allowed_cidrs = addresses
        .iter()
        .copied()
        .map(|address| {
            EgressCidr::try_new(address, if address.is_ipv4() { 32 } else { 128 })
                .map_err(|_| GrokAdminError::ProbeUnavailable)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let policy = EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("p12-10g-native-grok-probe".to_owned())
            .map_err(|_| GrokAdminError::ProbeUnavailable)?,
        name: "P12-10G native Grok probe".to_owned(),
        allowed_schemes: BTreeSet::from([EgressScheme::Https]),
        allowed_hosts: BTreeSet::from([host.clone()]),
        allowed_ports: BTreeSet::from([443]),
        allowed_cidrs,
        redirect_policy: RedirectPolicy::Deny,
    })
    .map_err(|_| GrokAdminError::ProbeUnavailable)?;
    let resolver: Arc<dyn EgressDnsResolver> = Arc::new(PinnedResolver { host, addresses });
    policy
        .admit_url(target_url, resolver.as_ref())
        .map_err(|error| GrokAdminError::ProbeEgress { code: error.code() })?;
    Ok((policy, resolver))
}

struct PinnedResolver {
    host: EgressHost,
    addresses: Vec<IpAddr>,
}

impl EgressDnsResolver for PinnedResolver {
    fn resolve(&self, host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        if host != &self.host {
            return Err(EgressDnsError);
        }
        Ok(self.addresses.clone())
    }
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
