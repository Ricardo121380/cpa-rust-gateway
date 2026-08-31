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
    GatewayErrorCode, ProviderAccountEntitlement, RequestContext, RequestId,
};
use gateway_router::{
    ProtocolFormat, ProtocolResponseRejection, RuntimeCredentialAccountStatus,
    RuntimeHealthRegistry, RuntimeQuotaRegistry, project_protocol_response,
};
use gateway_store::secret_store::{KeyVersion, MasterKeyRing, SecretStore};
use gateway_upstream::{
    EgressAdmissionErrorCode, EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost,
    EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy, SystemEgressDnsResolver,
    UpstreamClientPool, UpstreamHttpMethod, UpstreamHttpRequest, UpstreamProxy, UpstreamTimeouts,
    UpstreamTransportProfile,
};
use protocol_openai_responses::{ResponseMode, decode_request};
use provider_grok::{
    GROK_BUILD_CLIENT_IDENTIFIER, GROK_BUILD_CLIENT_IDENTIFIER_HEADER, GROK_BUILD_CLIENT_MODE,
    GROK_BUILD_CLIENT_MODE_HEADER, GROK_BUILD_CLIENT_VERSION, GROK_BUILD_CLIENT_VERSION_HEADER,
    GROK_BUILD_SUBSCRIPTION_URL, GROK_BUILD_TOKEN_AUTH_HEADER, GROK_BUILD_TOKEN_AUTH_VALUE,
    GROK_BUILD_USER_AGENT, GROK_CONSOLE_RESPONSES_URL, Grok2ApiMemoryStreamMigration,
    Grok2ApiMigrationError, GrokAccountEndpointBinding, GrokAccountEntitlementUpdateOutcome,
    GrokAccountPoolError, GrokAccountPoolStore, GrokAccountProvider, GrokBuildCredential,
    GrokBuildExecutionMode, GrokBuildInferenceAdapter, GrokBuildUpstreamTransport,
    GrokConsoleExecutionMode, GrokConsoleInferenceAdapter, GrokConsoleRequestError,
    GrokConsoleResponsesRequestBuilder, GrokConsoleSsoToken, GrokConsoleUpstreamTransport,
    grok_build_entitlement_from_access_token, grok_build_entitlement_from_subscription_response,
};
use provider_kiro::InferenceAdapter;
use serde::Deserialize;

const MAX_BUILD_SUBSCRIPTION_RESPONSE_BYTES: usize = 64 * 1024;

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
    EntitlementRejected,
    Entitlement(GrokAccountPoolError),
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
            Self::EntitlementRejected => "native Grok Build entitlement sync was rejected",
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
                    "native Grok migration failed: category={:?} source_records={} accepted_accounts={} capped_web_expiries={} rejected_records={} accepted_links={}",
                    error.kind(),
                    receipt.source_records,
                    receipt.accepted_accounts,
                    receipt.capped_web_expiries,
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
            Self::Entitlement(error) => {
                return write!(
                    formatter,
                    "native Grok Build entitlement sync failed: category={error:?}"
                );
            }
        })
    }
}

impl Error for GrokAdminError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Migration(error) => Some(error),
            Self::Rollback(error) | Self::Entitlement(error) => Some(error),
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
        "native_grok_import=PASS source_records={} accepted_accounts={} capped_web_expiries={} rejected_records={} accepted_links={} created_accounts={} unchanged_accounts={}",
        receipt.source_records,
        receipt.accepted_accounts,
        receipt.capped_web_expiries,
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

/// Synchronizes the exact enabled Build account in one import batch.
///
/// The command performs at most one bodyless subscription request. A valid successful response is
/// authoritative; every other live result may only fall back to the already-held access-token
/// claim. It never selects Web or Console accounts, sends inference, refreshes, or retries.
pub(crate) fn sync_build_entitlement(
    database: &str,
    credential_directory: &str,
    batch: &str,
    observed_at_ms: i64,
) -> Result<(), GrokAdminError> {
    require_root()?;
    let store = open_store(database, credential_directory)?;
    let accounts = store.list_accounts().map_err(GrokAdminError::Entitlement)?;
    let selected = accounts
        .iter()
        .filter(|account| {
            account.import_batch_id == batch
                && account.provider == GrokAccountProvider::Build
                && account.enabled
        })
        .collect::<Vec<_>>();
    if selected.len() != 1 {
        return Err(GrokAdminError::EntitlementRejected);
    }
    let selected = selected[0];
    let credential_bytes = store
        .open_credential(&selected.id)
        .map_err(GrokAdminError::Entitlement)?;
    let credential =
        GrokBuildCredential::import_runtime_json(credential_bytes.as_bytes(), observed_at_ms)
            .map_err(|_| GrokAdminError::EntitlementRejected)?;
    let live_body =
        actix_web::rt::System::new().block_on(fetch_build_subscription(credential.access_token()));
    let entitlement = select_build_entitlement(
        live_body.as_deref(),
        credential.access_token(),
        observed_at_ms,
    )?;
    let outcome = store
        .set_account_entitlement(&selected.id, entitlement)
        .map_err(GrokAdminError::Entitlement)?;
    let outcome = match outcome {
        GrokAccountEntitlementUpdateOutcome::Created => "created",
        GrokAccountEntitlementUpdateOutcome::Updated => "updated",
        GrokAccountEntitlementUpdateOutcome::Unchanged => "unchanged",
    };
    println!(
        "native_grok_entitlement_sync=PASS provider=grok_build domain={} tier={} source={} confidence={} outcome={outcome}",
        entitlement.domain().as_str(),
        entitlement.tier().as_str(),
        entitlement.source().as_str(),
        entitlement.confidence().as_str(),
    );
    Ok(())
}

fn select_build_entitlement(
    live_body: Option<&[u8]>,
    access_token: &str,
    observed_at_ms: i64,
) -> Result<ProviderAccountEntitlement, GrokAdminError> {
    live_body
        .and_then(|body| {
            grok_build_entitlement_from_subscription_response(body, observed_at_ms).ok()
        })
        .or_else(|| grok_build_entitlement_from_access_token(access_token, observed_at_ms).ok())
        .ok_or(GrokAdminError::EntitlementRejected)
}

async fn fetch_build_subscription(access_token: &str) -> Option<Vec<u8>> {
    let (policy, resolver) =
        probe_egress("cli-chat-proxy.grok.com", GROK_BUILD_SUBSCRIPTION_URL).ok()?;
    let admitted = policy
        .admit_url(GROK_BUILD_SUBSCRIPTION_URL, resolver.as_ref())
        .ok()?;
    let headers = vec![
        ("accept".to_owned(), "application/json".to_owned()),
        ("authorization".to_owned(), format!("Bearer {access_token}")),
        (
            GROK_BUILD_TOKEN_AUTH_HEADER.to_owned(),
            GROK_BUILD_TOKEN_AUTH_VALUE.to_owned(),
        ),
        (
            GROK_BUILD_CLIENT_VERSION_HEADER.to_owned(),
            GROK_BUILD_CLIENT_VERSION.to_owned(),
        ),
        (
            GROK_BUILD_CLIENT_IDENTIFIER_HEADER.to_owned(),
            GROK_BUILD_CLIENT_IDENTIFIER.to_owned(),
        ),
        (
            GROK_BUILD_CLIENT_MODE_HEADER.to_owned(),
            GROK_BUILD_CLIENT_MODE.to_owned(),
        ),
        ("user-agent".to_owned(), GROK_BUILD_USER_AGENT.to_owned()),
    ];
    let request =
        UpstreamHttpRequest::try_new(admitted, UpstreamHttpMethod::Get, headers, Vec::new())
            .ok()?;
    let profile = UpstreamTransportProfile::new(
        UpstreamTimeouts::try_new(
            Duration::from_secs(10),
            Duration::from_secs(30),
            Duration::from_secs(30),
            Duration::from_secs(45),
        )
        .ok()?,
        UpstreamProxy::Direct,
        NonZeroUsize::new(1)?,
    );
    let pool = UpstreamClientPool::new(NonZeroUsize::new(1)?);
    let mut response = pool.send(request, &profile).await.ok()?;
    if !(200..300).contains(&response.status()) {
        return None;
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.next_chunk().await.ok()? {
        if body.len().saturating_add(chunk.len()) > MAX_BUILD_SUBSCRIPTION_RESPONSE_BYTES {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    (!body.is_empty()).then_some(body)
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
        .try_lease_eligible(&endpoint, |credential_id| {
            credential_id.as_str() == selected.id
        })
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
            let migrated: ConsoleProbeCredential =
                serde_json::from_slice(credential).map_err(|_| GrokAdminError::ProbeUnavailable)?;
            let token = GrokConsoleSsoToken::try_from_bytes(migrated.sso_token.as_bytes())
                .map_err(|_| GrokAdminError::ProbeUnavailable)?;
            let outbound = GrokConsoleResponsesRequestBuilder::build_observed_probe(
                &token,
                &migrated.probe_model,
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
            let adapter = GrokConsoleInferenceAdapter::try_new_observed_probe(
                token,
                migrated.probe_model,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsoleProbeCredential {
    sso_token: String,
    probe_model: String,
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

#[cfg(test)]
mod tests {
    use gateway_core::{
        ProviderAccountEntitlementConfidence, ProviderAccountEntitlementSource,
        ProviderAccountEntitlementTier,
    };

    use super::select_build_entitlement;

    const HEAVY_TOKEN: &str = "e30.eyJ0aWVyIjo1fQ.signature";

    #[test]
    fn successful_subscription_observation_wins_over_token_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let entitlement =
            select_build_entitlement(Some(br#"{"subscriptionTier":"GrokPro"}"#), HEAVY_TOKEN, 42)?;
        assert_eq!(
            entitlement.tier(),
            ProviderAccountEntitlementTier::GrokBuildSupergrok
        );
        assert_eq!(
            entitlement.source(),
            ProviderAccountEntitlementSource::ProviderSubscription
        );
        assert_eq!(
            entitlement.confidence(),
            ProviderAccountEntitlementConfidence::Authoritative
        );
        Ok(())
    }

    #[test]
    fn missing_or_invalid_live_observation_uses_only_the_token_claim()
    -> Result<(), Box<dyn std::error::Error>> {
        for live in [None, Some(b"{}".as_slice())] {
            let entitlement = select_build_entitlement(live, HEAVY_TOKEN, 42)?;
            assert_eq!(
                entitlement.tier(),
                ProviderAccountEntitlementTier::GrokBuildHeavy
            );
            assert_eq!(
                entitlement.source(),
                ProviderAccountEntitlementSource::SignedToken
            );
            assert_eq!(
                entitlement.confidence(),
                ProviderAccountEntitlementConfidence::Derived
            );
        }
        Ok(())
    }
}
