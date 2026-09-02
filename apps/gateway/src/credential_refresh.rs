//! Production refresh workers for credentials already imported into CPAR.
//!
//! Registration, interactive OAuth, and account repair remain outside this module. It owns only
//! proactive refresh-token execution, durable CAS rotation, and atomic replacement of the exact
//! request material already admitted into one running process.

use std::{
    cell::Cell,
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gateway_control::{
    control_plane_service::credential_associated_data,
    management_mutation_service::{ConfigRevision, ManagementMutationService},
    management_service::ManagementActor,
};
use gateway_core::{CredentialId, EndpointId};
use gateway_http_actix::management_resources::{
    ManagementCodexOAuthExchange, OpenAiCodexOAuthExchange,
};
use gateway_router::{RuntimeHealthAccountRecoveryResult, RuntimeHealthRegistry};
use gateway_store::{
    control_plane::{
        ConfigVersionId, ControlPlaneConfiguration, CredentialStatus, SqliteControlPlaneRepository,
    },
    secret_store::SecretStore,
};
use gateway_upstream::{
    CredentialMaterialReplacement, CredentialSecret, EndpointCredentialPools, UpstreamProxy,
};
use provider_grok::{
    GrokAccountAuthStatus, GrokAccountCredential, GrokAccountPoolStore, GrokAccountProvider,
    GrokAccountWorkerCoordinator, GrokAccountWorkerError, GrokAccountWorkerExecutor,
    GrokAccountWorkerJob, GrokAccountWorkerKind, GrokAccountWorkerResult,
    GrokAccountWorkerRunSummary, GrokBuildCredential, GrokBuildOAuthError, GrokBuildOAuthFlow,
    GrokBuildOAuthHttpResponse, GrokBuildOAuthRequest, GrokBuildOAuthTransport,
    GrokBuildOAuthTransportError, MAX_GROK_BUILD_OAUTH_HTTP_RESPONSE_BYTES,
};
use provider_openai_compatible::{
    CODEX_RESPONSES_BASE_URL, CODEX_RESPONSES_PATH, OpenAiCompatibleRuntimeCredential,
};
use zeroize::Zeroizing;

const REFRESH_WORKER_CONCURRENCY: usize = 4;
const REFRESH_CLAIM_LEASE_MS: i64 = 60_000;
const REFRESH_INTERVAL: Duration = Duration::from_mins(1);
const OAUTH_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const OAUTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const RECOVERY_TICKET_MS: i64 = 30_000;
const CODEX_REFRESH_SKEW_MS: i64 = 8 * 60 * 1_000;
const CODEX_RESPONSES_ADAPTER_ID: &str = "openai-compatible.responses";

/// A redacted result from one runtime refresh pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeCredentialRefreshSummary {
    pub(crate) claimed: usize,
    pub(crate) succeeded: usize,
    pub(crate) backed_off: usize,
    pub(crate) reauth_required: usize,
    pub(crate) panicked: usize,
    pub(crate) runtime_replaced: usize,
    pub(crate) codex_due: usize,
    pub(crate) codex_succeeded: usize,
    pub(crate) codex_backed_off: usize,
}

impl RuntimeCredentialRefreshSummary {
    fn with_runtime_replaced(
        summary: GrokAccountWorkerRunSummary,
        runtime_replaced: usize,
        codex: CodexRefreshSummary,
    ) -> Self {
        Self {
            claimed: summary.claimed,
            succeeded: summary.succeeded,
            backed_off: summary.backed_off,
            reauth_required: summary.reauth_required,
            panicked: summary.panicked,
            runtime_replaced,
            codex_due: codex.due,
            codex_succeeded: codex.succeeded,
            codex_backed_off: codex.backed_off,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CodexRefreshSummary {
    due: usize,
    succeeded: usize,
    backed_off: usize,
    runtime_replaced: usize,
}

/// Runs one startup catch-up before immutable graph metadata is compiled.
pub(crate) fn refresh_due_credentials_before_compile(
    database: &Path,
    secret_store: &SecretStore,
    codex_proxy: UpstreamProxy,
) -> Result<RuntimeCredentialRefreshSummary, GrokAccountWorkerError> {
    let Some(scope) = active_refresh_scope(database)? else {
        return Ok(RuntimeCredentialRefreshSummary::with_runtime_replaced(
            empty_grok_summary(),
            0,
            CodexRefreshSummary::default(),
        ));
    };
    let store = Arc::new(
        GrokAccountPoolStore::try_open(database, secret_store.clone())
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?,
    );
    let observed_at_ms = now_ms()?;
    let summary = if scope.has_build {
        coordinator()?.run_once_for_provider(
            &store,
            GrokAccountWorkerKind::Refresh,
            GrokAccountProvider::Build,
            observed_at_ms,
            &GrokBuildRefreshExecutor::try_new()?,
        )?
    } else {
        empty_grok_summary()
    };
    let codex = refresh_codex_credentials(
        database,
        secret_store,
        codex_proxy,
        &scope.config_version_id,
        &scope.codex_credential_ids,
        None,
        None,
        observed_at_ms,
    )?;
    Ok(RuntimeCredentialRefreshSummary::with_runtime_replaced(
        summary, 0, codex,
    ))
}

/// Periodic refresh owner bound to one running data-plane pool set.
pub(crate) struct RuntimeCredentialRefreshWorker {
    store: Arc<GrokAccountPoolStore>,
    pools: Arc<EndpointCredentialPools>,
    runtime_health: Arc<RuntimeHealthRegistry>,
    build_endpoints: Vec<EndpointId>,
    executor: GrokBuildRefreshExecutor,
    database: PathBuf,
    secret_store: SecretStore,
    codex_proxy: UpstreamProxy,
    config_version_id: ConfigVersionId,
    codex_credential_ids: BTreeSet<CredentialId>,
}

impl RuntimeCredentialRefreshWorker {
    /// Creates a worker only when the active graph has a supported refreshable OAuth channel.
    pub(crate) fn try_new(
        database: &Path,
        secret_store: SecretStore,
        pools: Arc<EndpointCredentialPools>,
        runtime_health: Arc<RuntimeHealthRegistry>,
        build_endpoints: Vec<EndpointId>,
        codex_proxy: UpstreamProxy,
        config_version_id: ConfigVersionId,
    ) -> Result<Option<Self>, GrokAccountWorkerError> {
        let codex_credential_ids =
            refresh_scope_for_configuration(database, &config_version_id)?.codex_credential_ids;
        if build_endpoints.is_empty() && codex_credential_ids.is_empty() {
            return Ok(None);
        }
        let store = GrokAccountPoolStore::try_open(database, secret_store.clone())
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        Ok(Some(Self {
            store: Arc::new(store),
            pools,
            runtime_health,
            build_endpoints,
            executor: GrokBuildRefreshExecutor::try_new()?,
            database: database.to_path_buf(),
            secret_store,
            codex_proxy,
            config_version_id,
            codex_credential_ids,
        }))
    }

    /// Runs until the process runtime stops. Each network/store pass stays on the blocking pool.
    pub(crate) async fn run(self) {
        let worker = Arc::new(self);
        let mut interval = actix_web::rt::time::interval(REFRESH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let worker = Arc::clone(&worker);
            let result = actix_web::rt::task::spawn_blocking(move || worker.run_once()).await;
            if let Ok(Ok(summary)) = result {
                tracing::info!(
                    target: "credential_refresh",
                    provider = "refreshable_oauth",
                    claimed = summary.claimed,
                    succeeded = summary.succeeded,
                    backed_off = summary.backed_off,
                    reauth_required = summary.reauth_required,
                    panicked = summary.panicked,
                    runtime_replaced = summary.runtime_replaced,
                    codex_due = summary.codex_due,
                    codex_succeeded = summary.codex_succeeded,
                    codex_backed_off = summary.codex_backed_off,
                    "credential refresh pass completed"
                );
            } else {
                tracing::warn!(
                    target: "credential_refresh",
                    provider = "refreshable_oauth",
                    "credential refresh pass unavailable"
                );
            }
        }
    }

    fn run_once(&self) -> Result<RuntimeCredentialRefreshSummary, GrokAccountWorkerError> {
        let observed_at_ms = now_ms()?;
        let summary = if self.build_endpoints.is_empty() {
            empty_grok_summary()
        } else {
            coordinator()?.run_once_for_provider(
                &self.store,
                GrokAccountWorkerKind::Refresh,
                GrokAccountProvider::Build,
                observed_at_ms,
                &self.executor,
            )?
        };
        let mut runtime_replaced = self.sync_runtime_material(observed_at_ms)?;
        let codex = refresh_codex_credentials(
            &self.database,
            &self.secret_store,
            self.codex_proxy.clone(),
            &self.config_version_id,
            &self.codex_credential_ids,
            Some(self.pools.as_ref()),
            Some(self.runtime_health.as_ref()),
            observed_at_ms,
        )?;
        runtime_replaced = runtime_replaced.saturating_add(codex.runtime_replaced);
        Ok(RuntimeCredentialRefreshSummary::with_runtime_replaced(
            summary,
            runtime_replaced,
            codex,
        ))
    }

    fn sync_runtime_material(&self, observed_at_ms: i64) -> Result<usize, GrokAccountWorkerError> {
        let mut replaced = 0_usize;
        for account in self
            .store
            .list_accounts()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?
            .into_iter()
            .filter(|account| {
                account.provider == GrokAccountProvider::Build
                    && account.enabled
                    && account.auth_status == GrokAccountAuthStatus::Active
            })
        {
            let runtime_revision = account
                .revision
                .checked_add(1)
                .ok_or(GrokAccountWorkerError::InvalidPersistedState)?;
            let credential_id = gateway_core::CredentialId::try_new(account.id.clone())
                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
            let targets = self
                .build_endpoints
                .iter()
                .filter_map(|endpoint_id| {
                    self.pools.pool(endpoint_id).and_then(|pool| {
                        pool.diagnostic_entries()
                            .into_iter()
                            .find(|entry| entry.credential_id() == &credential_id)
                            .map(|current| (endpoint_id, current))
                    })
                })
                .filter(|(_, current)| current.credential_revision() != runtime_revision)
                .collect::<Vec<_>>();
            if targets.is_empty() {
                // An old expired account may remain disabled by runtime expiry while its durable
                // revision is unchanged. Do not reopen or reject that retired material merely to
                // prove there is nothing to publish into the live pool.
                continue;
            }
            let plaintext = self
                .store
                .open_credential(&account.id)
                .map_err(|_| GrokAccountWorkerError::SecretStoreFailure)?;
            let credential =
                GrokBuildCredential::import_active_runtime(plaintext.as_bytes(), observed_at_ms)
                    .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
            for (endpoint_id, current) in targets {
                if current.credential_revision() > runtime_revision {
                    return Err(GrokAccountWorkerError::InvalidPersistedState);
                }
                let did_replace = self
                    .pools
                    .replace_credential_if_revision(
                        endpoint_id,
                        &credential_id,
                        current.credential_revision(),
                        CredentialMaterialReplacement {
                            credential_revision: i64::try_from(runtime_revision)
                                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?,
                            expires_at_ms: Some(credential.expires_at_ms()),
                            secret: CredentialSecret::try_new(plaintext.as_bytes().to_vec())
                                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?,
                        },
                    )
                    .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
                if did_replace {
                    replaced += 1;
                    complete_runtime_recovery(
                        self.runtime_health.as_ref(),
                        endpoint_id,
                        &credential_id,
                        observed_at_ms,
                    )?;
                }
            }
        }
        Ok(replaced)
    }
}

struct RefreshScope {
    config_version_id: ConfigVersionId,
    has_build: bool,
    codex_credential_ids: BTreeSet<CredentialId>,
}

fn active_refresh_scope(database: &Path) -> Result<Option<RefreshScope>, GrokAccountWorkerError> {
    let mut repository = SqliteControlPlaneRepository::open(database)
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
    let configuration = repository
        .load_active_configuration()
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
    Ok(configuration.as_ref().map(refresh_scope))
}

fn refresh_scope_for_configuration(
    database: &Path,
    config_version_id: &ConfigVersionId,
) -> Result<RefreshScope, GrokAccountWorkerError> {
    let mut repository = SqliteControlPlaneRepository::open(database)
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
    let configuration = repository
        .load_configuration(config_version_id)
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?
        .ok_or(GrokAccountWorkerError::InvalidPersistedState)?;
    Ok(refresh_scope(&configuration))
}

fn refresh_scope(configuration: &ControlPlaneConfiguration) -> RefreshScope {
    let has_build = configuration
        .endpoints
        .iter()
        .any(|endpoint| endpoint.enabled && endpoint.adapter_id == "grok.build.responses");
    let codex_endpoints = configuration
        .endpoints
        .iter()
        .filter(|endpoint| {
            endpoint.enabled
                && endpoint.adapter_id == CODEX_RESPONSES_ADAPTER_ID
                && endpoint.base_url.trim_end_matches('/') == CODEX_RESPONSES_BASE_URL
                && endpoint.inference_path == CODEX_RESPONSES_PATH
        })
        .map(|endpoint| (&endpoint.id, &endpoint.upstream_id))
        .collect::<BTreeSet<_>>();
    let bound_credentials = configuration
        .endpoint_credential_bindings
        .iter()
        .filter(|binding| {
            binding.enabled
                && codex_endpoints.contains(&(&binding.endpoint_id, &binding.upstream_id))
        })
        .map(|binding| &binding.credential_id)
        .collect::<BTreeSet<_>>();
    let codex_credential_ids = configuration
        .credentials
        .iter()
        .filter(|credential| {
            credential.kind == "oauth_json"
                && credential.status == CredentialStatus::Active
                && bound_credentials.contains(&credential.id)
        })
        .map(|credential| credential.id.clone())
        .collect();
    RefreshScope {
        config_version_id: configuration.version.id.clone(),
        has_build,
        codex_credential_ids,
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh_codex_credentials(
    database: &Path,
    secret_store: &SecretStore,
    codex_proxy: UpstreamProxy,
    config_version_id: &ConfigVersionId,
    codex_credential_ids: &BTreeSet<CredentialId>,
    pools: Option<&EndpointCredentialPools>,
    runtime_health: Option<&RuntimeHealthRegistry>,
    observed_at_ms: i64,
) -> Result<CodexRefreshSummary, GrokAccountWorkerError> {
    let mut repository = SqliteControlPlaneRepository::open(database)
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
    let Some(configuration) = repository
        .load_configuration(config_version_id)
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?
    else {
        return Ok(CodexRefreshSummary::default());
    };
    let config_revision = ConfigRevision::try_new(configuration.version.revision)
        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
    let actor = ManagementActor::try_new("runtime-credential-refresh")
        .map_err(|_| GrokAccountWorkerError::InvalidRequest)?;
    let mutation_repository = SqliteControlPlaneRepository::open(database)
        .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
    let mut mutation_service =
        ManagementMutationService::new(mutation_repository, secret_store.clone());
    let mut exchange = OpenAiCodexOAuthExchange::new(codex_proxy);
    let refresh_cutoff = observed_at_ms
        .checked_add(CODEX_REFRESH_SKEW_MS)
        .ok_or(GrokAccountWorkerError::InvalidRequest)?;
    let mut summary = CodexRefreshSummary::default();

    for credential in configuration
        .credentials
        .iter()
        .filter(|credential| codex_credential_ids.contains(&credential.id))
    {
        let associated_data = credential_associated_data(
            &configuration.version.id,
            &credential.id,
            &credential.upstream_id,
        )
        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
        let plaintext = secret_store
            .open(&credential.encrypted_secret, &associated_data)
            .map_err(|_| GrokAccountWorkerError::SecretStoreFailure)?;
        let mut runtime_bytes = Zeroizing::new(plaintext.as_bytes().to_vec());
        let mut runtime_credential = OpenAiCompatibleRuntimeCredential::import_compatible(
            runtime_bytes.as_slice(),
            observed_at_ms,
        )
        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
        let mut runtime_revision = u64::try_from(credential.revision)
            .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;

        if runtime_credential
            .expires_at_ms()
            .is_some_and(|expires_at_ms| expires_at_ms <= refresh_cutoff)
        {
            summary.due += 1;
            let Some(refreshed) = exchange.refresh(
                &credential.id,
                Zeroizing::new(runtime_bytes.to_vec()),
                observed_at_ms,
            ) else {
                summary.backed_off += 1;
                continue;
            };
            let Ok(refreshed_credential) = OpenAiCompatibleRuntimeCredential::import_compatible(
                refreshed.as_slice(),
                observed_at_ms,
            ) else {
                summary.backed_off += 1;
                continue;
            };
            if mutation_service
                .persist_oauth_credential_if_revision(
                    &actor,
                    &configuration.version.id,
                    config_revision,
                    credential.id.clone(),
                    credential.revision,
                    refreshed.as_slice(),
                )
                .is_err()
            {
                summary.backed_off += 1;
                continue;
            }
            runtime_revision = runtime_revision
                .checked_add(1)
                .ok_or(GrokAccountWorkerError::InvalidPersistedState)?;
            runtime_bytes = refreshed;
            runtime_credential = refreshed_credential;
            summary.succeeded += 1;
        }

        summary.runtime_replaced =
            summary
                .runtime_replaced
                .saturating_add(sync_codex_runtime_material(
                    &configuration,
                    &credential.id,
                    runtime_bytes.as_slice(),
                    &runtime_credential,
                    runtime_revision,
                    pools,
                    runtime_health,
                    observed_at_ms,
                )?);
    }
    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
fn sync_codex_runtime_material(
    configuration: &ControlPlaneConfiguration,
    credential_id: &CredentialId,
    runtime_bytes: &[u8],
    runtime_credential: &OpenAiCompatibleRuntimeCredential,
    runtime_revision: u64,
    pools: Option<&EndpointCredentialPools>,
    runtime_health: Option<&RuntimeHealthRegistry>,
    observed_at_ms: i64,
) -> Result<usize, GrokAccountWorkerError> {
    let Some(pools) = pools else {
        return Ok(0);
    };
    let mut replaced_count = 0_usize;
    for binding in configuration
        .endpoint_credential_bindings
        .iter()
        .filter(|binding| binding.enabled && &binding.credential_id == credential_id)
    {
        let Some(current) = pools.pool(&binding.endpoint_id).and_then(|pool| {
            pool.diagnostic_entries()
                .into_iter()
                .find(|entry| entry.credential_id() == credential_id)
        }) else {
            continue;
        };
        if current.credential_revision() == runtime_revision {
            continue;
        }
        if current.credential_revision() > runtime_revision {
            return Err(GrokAccountWorkerError::InvalidPersistedState);
        }
        let replaced = pools
            .replace_credential_if_revision(
                &binding.endpoint_id,
                credential_id,
                current.credential_revision(),
                CredentialMaterialReplacement {
                    credential_revision: i64::try_from(runtime_revision)
                        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?,
                    expires_at_ms: runtime_credential.expires_at_ms(),
                    secret: CredentialSecret::try_new(runtime_bytes.to_vec())
                        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?,
                },
            )
            .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
        if replaced {
            replaced_count += 1;
            if let Some(runtime_health) = runtime_health {
                complete_runtime_recovery(
                    runtime_health,
                    &binding.endpoint_id,
                    credential_id,
                    observed_at_ms,
                )?;
            }
        }
    }
    Ok(replaced_count)
}

fn complete_runtime_recovery(
    runtime_health: &RuntimeHealthRegistry,
    endpoint_id: &EndpointId,
    credential_id: &CredentialId,
    observed_at_ms: i64,
) -> Result<(), GrokAccountWorkerError> {
    let recovery_deadline = observed_at_ms
        .checked_add(RECOVERY_TICKET_MS)
        .ok_or(GrokAccountWorkerError::InvalidRequest)?;
    if let Some(ticket) = runtime_health
        .begin_account_recovery(endpoint_id, credential_id, recovery_deadline)
        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?
    {
        runtime_health
            .complete_account_recovery(ticket, RuntimeHealthAccountRecoveryResult::Allowed)
            .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
    }
    Ok(())
}

fn coordinator() -> Result<GrokAccountWorkerCoordinator, GrokAccountWorkerError> {
    GrokAccountWorkerCoordinator::try_new(REFRESH_WORKER_CONCURRENCY, REFRESH_CLAIM_LEASE_MS)
}

const fn empty_grok_summary() -> GrokAccountWorkerRunSummary {
    GrokAccountWorkerRunSummary {
        claimed: 0,
        succeeded: 0,
        backed_off: 0,
        reauth_required: 0,
        panicked: 0,
    }
}

fn now_ms() -> Result<i64, GrokAccountWorkerError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .ok_or(GrokAccountWorkerError::InvalidRequest)
}

struct GrokBuildRefreshExecutor {
    client: reqwest::blocking::Client,
}

impl GrokBuildRefreshExecutor {
    fn try_new() -> Result<Self, GrokAccountWorkerError> {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(OAUTH_CONNECT_TIMEOUT)
            .timeout(OAUTH_REQUEST_TIMEOUT)
            .build()
            .map_err(|_| GrokAccountWorkerError::InvalidRequest)?;
        Ok(Self { client })
    }
}

impl GrokAccountWorkerExecutor for GrokBuildRefreshExecutor {
    fn execute(&self, job: &GrokAccountWorkerJob) -> GrokAccountWorkerResult {
        if job.kind() != GrokAccountWorkerKind::Refresh
            || job.provider() != GrokAccountProvider::Build
        {
            return GrokAccountWorkerResult::TransientFailure;
        }
        let Ok(observed_at_ms) = now_ms() else {
            return GrokAccountWorkerResult::TransientFailure;
        };
        let Ok(current) =
            GrokBuildCredential::import_refreshable_runtime(job.credential_bytes(), observed_at_ms)
        else {
            return GrokAccountWorkerResult::ReauthRequired;
        };
        let transport = GrokBuildRefreshTransport {
            client: self.client.clone(),
            status: Cell::new(None),
        };
        match GrokBuildOAuthFlow::default().refresh(&transport, &current, observed_at_ms) {
            Ok(refreshed) => {
                let expires_at_ms = refreshed.expires_at_ms();
                match GrokAccountCredential::try_from_build_credential(&refreshed) {
                    Ok(credential) => GrokAccountWorkerResult::Refreshed {
                        credential,
                        expires_at_ms,
                    },
                    Err(_) => GrokAccountWorkerResult::TransientFailure,
                }
            }
            Err(GrokBuildOAuthError::TransportUnavailable) => {
                GrokAccountWorkerResult::TransientFailure
            }
            Err(_) if matches!(transport.status.get(), Some(400 | 401 | 403)) => {
                GrokAccountWorkerResult::ReauthRequired
            }
            Err(_) => GrokAccountWorkerResult::TransientFailure,
        }
    }
}

struct GrokBuildRefreshTransport {
    client: reqwest::blocking::Client,
    status: Cell<Option<u16>>,
}

impl GrokBuildOAuthTransport for GrokBuildRefreshTransport {
    fn send(
        &self,
        request: GrokBuildOAuthRequest,
    ) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthTransportError> {
        let endpoint = request.endpoint().url();
        let body = request.into_form_body();
        let mut response = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .body(body.to_vec())
            .send()
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?;
        let status = response.status().as_u16();
        self.status.set(Some(status));
        if status == 429 || status >= 500 {
            return Err(GrokBuildOAuthTransportError::Unavailable);
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_GROK_BUILD_OAUTH_HTTP_RESPONSE_BYTES as u64)
        {
            return Err(GrokBuildOAuthTransportError::Unavailable);
        }
        let mut bounded = response
            .by_ref()
            .take((MAX_GROK_BUILD_OAUTH_HTTP_RESPONSE_BYTES + 1) as u64);
        let mut response_body = Vec::new();
        bounded
            .read_to_end(&mut response_body)
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?;
        if response_body.len() > MAX_GROK_BUILD_OAUTH_HTTP_RESPONSE_BYTES {
            return Err(GrokBuildOAuthTransportError::Unavailable);
        }
        GrokBuildOAuthHttpResponse::try_new(status, response_body)
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{CredentialId, EndpointId, UpstreamId};
    use gateway_store::{
        control_plane::{
            ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialConfiguration, CredentialStatus, EndpointConfiguration,
            EndpointCredentialBindingConfiguration, EndpointTransport, UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };

    use super::{CODEX_RESPONSES_ADAPTER_ID, CODEX_RESPONSES_BASE_URL, refresh_scope};

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn refresh_scope_uses_exact_channel_binding_not_the_oauth_storage_label() -> TestResult {
        let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
            id: ConfigVersionId::try_new("refresh-scope-config")?,
            parent_id: None,
            status: ConfigVersionStatus::Active,
            revision: 7,
            created_at_ms: 1,
            description: "refresh scope test".to_owned(),
        });
        let codex_upstream = UpstreamId::try_new("codex-upstream")?;
        let generic_upstream = UpstreamId::try_new("generic-upstream")?;
        configuration.upstreams.extend([
            upstream(codex_upstream.clone(), "codex"),
            upstream(generic_upstream.clone(), "generic"),
        ]);
        let codex_endpoint = EndpointId::try_new("codex-endpoint")?;
        let generic_endpoint = EndpointId::try_new("generic-endpoint")?;
        let build_endpoint = EndpointId::try_new("build-endpoint")?;
        configuration.endpoints.extend([
            endpoint(
                codex_endpoint.clone(),
                codex_upstream.clone(),
                CODEX_RESPONSES_ADAPTER_ID,
                CODEX_RESPONSES_BASE_URL,
                true,
            ),
            endpoint(
                generic_endpoint.clone(),
                generic_upstream.clone(),
                CODEX_RESPONSES_ADAPTER_ID,
                "https://relay.example.test/v1",
                true,
            ),
            endpoint(
                build_endpoint,
                generic_upstream.clone(),
                "grok.build.responses",
                "https://cli-chat-proxy.grok.com/v1",
                true,
            ),
        ]);

        let secret_store = secret_store()?;
        let codex_credential = CredentialId::try_new("codex-oauth")?;
        let generic_oauth = CredentialId::try_new("generic-oauth")?;
        let unbound_oauth = CredentialId::try_new("unbound-oauth")?;
        configuration.credentials.extend([
            credential(
                &configuration,
                &secret_store,
                codex_credential.clone(),
                codex_upstream.clone(),
                "oauth_json",
            )?,
            credential(
                &configuration,
                &secret_store,
                generic_oauth.clone(),
                generic_upstream.clone(),
                "oauth_json",
            )?,
            credential(
                &configuration,
                &secret_store,
                unbound_oauth,
                codex_upstream.clone(),
                "oauth_json",
            )?,
        ]);
        configuration.endpoint_credential_bindings.extend([
            binding(
                codex_endpoint,
                codex_credential.clone(),
                codex_upstream,
                true,
            ),
            binding(generic_endpoint, generic_oauth, generic_upstream, true),
        ]);

        let scope = refresh_scope(&configuration);
        assert!(scope.has_build);
        assert_eq!(scope.codex_credential_ids.len(), 1);
        assert!(scope.codex_credential_ids.contains(&codex_credential));
        Ok(())
    }

    fn upstream(id: UpstreamId, name: &str) -> UpstreamConfiguration {
        UpstreamConfiguration {
            id,
            name: name.to_owned(),
            kind: "openai-compatible".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: None,
        }
    }

    fn endpoint(
        id: EndpointId,
        upstream_id: UpstreamId,
        adapter_id: &str,
        base_url: &str,
        enabled: bool,
    ) -> EndpointConfiguration {
        EndpointConfiguration {
            id,
            upstream_id,
            adapter_id: adapter_id.to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: base_url.to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: None,
            transport: EndpointTransport::Http,
            enabled,
        }
    }

    fn credential(
        configuration: &ControlPlaneConfiguration,
        secret_store: &SecretStore,
        id: CredentialId,
        upstream_id: UpstreamId,
        kind: &str,
    ) -> Result<CredentialConfiguration, Box<dyn Error>> {
        let associated_data = gateway_control::control_plane_service::credential_associated_data(
            &configuration.version.id,
            &id,
            &upstream_id,
        )?;
        Ok(CredentialConfiguration {
            id,
            upstream_id,
            kind: kind.to_owned(),
            encrypted_secret: secret_store.seal(b"opaque-test-secret", &associated_data)?,
            status: CredentialStatus::Active,
            revision: 1,
        })
    }

    fn binding(
        endpoint_id: EndpointId,
        credential_id: CredentialId,
        upstream_id: UpstreamId,
        enabled: bool,
    ) -> EndpointCredentialBindingConfiguration {
        EndpointCredentialBindingConfiguration {
            endpoint_id,
            credential_id,
            upstream_id,
            enabled,
            priority: 0,
            weight: 1,
            concurrency: 1,
        }
    }

    fn secret_store() -> Result<SecretStore, Box<dyn Error>> {
        let version = KeyVersion::try_new(1)?;
        Ok(SecretStore::new(MasterKeyRing::try_new(
            version,
            [(version, MasterKey::try_from_bytes([0x51_u8; 32])?)],
        )?))
    }
}
