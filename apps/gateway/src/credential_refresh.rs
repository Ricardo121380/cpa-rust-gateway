//! Production refresh workers for credentials already imported into CPAR.
//!
//! Registration, interactive OAuth, and account repair remain outside this module. It owns only
//! proactive refresh-token execution, durable CAS rotation, and atomic replacement of the exact
//! request material already admitted into one running process.

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gateway_control::control_plane_service::credential_associated_data;
use gateway_core::{CredentialId, EndpointId};
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
    CODEX_RESPONSES_BASE_URL, CODEX_RESPONSES_PATH, KIMI_OAUTH_TOKEN_URL,
    OpenAiCompatibleRuntimeCredential,
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
mod ordinary;
use ordinary::{Channel, Pass};

/// A redacted result from one runtime refresh pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeCredentialRefreshSummary {
    pub(crate) claimed: usize,
    pub(crate) succeeded: usize,
    pub(crate) backed_off: usize,
    pub(crate) reauth_required: usize,
    pub(crate) panicked: usize,
    pub(crate) runtime_replaced: usize,
    pub(crate) oauth_due: usize,
    pub(crate) oauth_succeeded: usize,
    pub(crate) oauth_backed_off: usize,
    pub(crate) oauth_conflicted: usize,
}
impl RuntimeCredentialRefreshSummary {
    fn combined(
        summary: GrokAccountWorkerRunSummary,
        runtime_replaced: usize,
        oauth: OAuthRefreshSummary,
    ) -> Self {
        Self {
            claimed: summary.claimed,
            succeeded: summary.succeeded,
            backed_off: summary.backed_off,
            reauth_required: summary.reauth_required + oauth.reauth_required,
            panicked: summary.panicked,
            runtime_replaced: runtime_replaced + oauth.runtime_replaced,
            oauth_due: oauth.due,
            oauth_succeeded: oauth.succeeded,
            oauth_backed_off: oauth.backed_off,
            oauth_conflicted: oauth.conflicted,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct OAuthRefreshSummary {
    due: usize,
    succeeded: usize,
    backed_off: usize,
    runtime_replaced: usize,
    reauth_required: usize,
    conflicted: usize,
}

/// Runs one startup catch-up before immutable graph metadata is compiled.
pub(crate) fn refresh_due_credentials_before_compile(
    database: &Path,
    secret_store: &SecretStore,
    codex_proxy: UpstreamProxy,
) -> Result<RuntimeCredentialRefreshSummary, GrokAccountWorkerError> {
    let Some(scope) = active_refresh_scope(database)? else {
        return Ok(RuntimeCredentialRefreshSummary::combined(
            empty_grok_summary(),
            0,
            OAuthRefreshSummary::default(),
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
    let oauth = Pass {
        database,
        secrets: secret_store,
        version: &scope.config_version_id,
        channels: &scope.channels,
        pools: None,
        health: None,
        guard: None,
        stopped: None,
    }
    .run(codex_proxy)?;
    Ok(RuntimeCredentialRefreshSummary::combined(summary, 0, oauth))
}

/// Periodic refresh owner bound to one running data-plane pool set.
pub(crate) struct RuntimeCredentialRefreshWorker {
    generation_guard: Option<(
        Arc<tokio::sync::Mutex<()>>,
        Arc<std::sync::atomic::AtomicBool>,
    )>,
    store: Arc<GrokAccountPoolStore>,
    pools: Arc<EndpointCredentialPools>,
    runtime_health: Arc<RuntimeHealthRegistry>,
    build_endpoints: Vec<EndpointId>,
    executor: GrokBuildRefreshExecutor,
    database: PathBuf,
    secret_store: SecretStore,
    codex_proxy: UpstreamProxy,
    config_version_id: ConfigVersionId,
    channels: BTreeMap<CredentialId, Channel>,
    stopped: Arc<std::sync::atomic::AtomicBool>,
    stop_notify: Arc<tokio::sync::Notify>,
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
        let scope = refresh_scope_for_configuration(database, &config_version_id)?;
        if build_endpoints.is_empty() && scope.channels.is_empty() {
            return Ok(None);
        }
        let store = GrokAccountPoolStore::try_open(database, secret_store.clone())
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        Ok(Some(Self {
            generation_guard: None,
            store: Arc::new(store),
            pools,
            runtime_health,
            build_endpoints,
            executor: GrokBuildRefreshExecutor::try_new()?,
            database: database.to_path_buf(),
            secret_store,
            codex_proxy,
            config_version_id,
            channels: scope.channels,
            stopped: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            stop_notify: Arc::new(tokio::sync::Notify::new()),
        }))
    }

    pub(crate) fn with_generation_guard(
        mut self,
        gate: Arc<tokio::sync::Mutex<()>>,
        active: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        self.generation_guard = Some((gate, active));
        self
    }

    pub(crate) fn stop_signal(&self) -> RefreshStop {
        RefreshStop {
            stopped: Arc::clone(&self.stopped),
            notify: Arc::clone(&self.stop_notify),
        }
    }

    /// Runs until the process runtime stops. Each network/store pass stays on the blocking pool.
    pub(crate) async fn run(self) {
        let _stop = StopOnDrop(Arc::clone(&self.stopped));
        let worker = Arc::new(self);
        let start_at = tokio::time::Instant::now() + REFRESH_INTERVAL;
        let mut interval = actix_web::rt::time::interval_at(start_at, REFRESH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            if worker.stopped.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
            tokio::select! { _ = interval.tick() => {}, () = worker.stop_notify.notified() => break }
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
                    oauth_due = summary.oauth_due,
                    oauth_succeeded = summary.oauth_succeeded,
                    oauth_backed_off = summary.oauth_backed_off,
                    oauth_conflicted = summary.oauth_conflicted,
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
        if self.stopped.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(RuntimeCredentialRefreshSummary::combined(
                empty_grok_summary(),
                0,
                OAuthRefreshSummary::default(),
            ));
        }
        // Publication waits for any refresh exchange and durable CAS already in progress.
        // A retired worker can never start another exchange after the serving pointer changes.
        let guard = match &self.generation_guard {
            Some((gate, active)) => {
                let guard = gate.blocking_lock();
                if !active.load(std::sync::atomic::Ordering::Acquire)
                    || self.stopped.load(std::sync::atomic::Ordering::Acquire)
                {
                    return Ok(RuntimeCredentialRefreshSummary::combined(
                        empty_grok_summary(),
                        0,
                        OAuthRefreshSummary::default(),
                    ));
                }
                Some(guard)
            }
            None => None,
        };
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
        let runtime_replaced = self.sync_runtime_material(observed_at_ms)?;
        drop(guard);
        let oauth = Pass {
            database: &self.database,
            secrets: &self.secret_store,
            version: &self.config_version_id,
            channels: &self.channels,
            pools: Some(self.pools.as_ref()),
            health: Some(self.runtime_health.as_ref()),
            guard: self.generation_guard.as_ref(),
            stopped: Some(&self.stopped),
        }
        .run(self.codex_proxy.clone())?;
        Ok(RuntimeCredentialRefreshSummary::combined(
            summary,
            runtime_replaced,
            oauth,
        ))
    }

    fn sync_runtime_material(&self, observed_at_ms: i64) -> Result<usize, GrokAccountWorkerError> {
        let mut replaced = 0_usize;
        let ranges = self
            .store
            .build_continuation_ranges()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?;
        for account in self
            .store
            .list_accounts()
            .map_err(|_| GrokAccountWorkerError::StoreUnavailable)?
            .into_iter()
            .filter(|account| account.provider == GrokAccountProvider::Build)
        {
            let runtime_revision = account
                .revision
                .checked_add(1)
                .ok_or(GrokAccountWorkerError::InvalidPersistedState)?;
            let credential_id = gateway_core::CredentialId::try_new(account.id.clone())
                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
            if !account.enabled || account.auth_status != GrokAccountAuthStatus::Active {
                for endpoint in &self.build_endpoints {
                    self.runtime_health
                        .mark_credential_unauthorized(endpoint.clone(), credential_id.clone())
                        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
                }
                continue;
            }
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
            if let Some((floor, current)) = ranges.get(&account.id) {
                for endpoint in &self.build_endpoints {
                    if let Some(pool) = self.pools.pool(endpoint)
                        && pool.diagnostic_entries().iter().any(|row| {
                            row.credential_id() == &credential_id
                                && row.credential_revision() == *current
                        })
                    {
                        pool.set_build_continuation_range(&credential_id, *floor, *current)
                            .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
                    }
                }
            }
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
                if let Some((floor, current)) = ranges.get(&account.id)
                    && let Some(pool) = self.pools.pool(endpoint_id)
                {
                    pool.set_build_continuation_range(&credential_id, *floor, *current)
                        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
                }
            }
        }
        Ok(replaced)
    }
}

struct RefreshScope {
    config_version_id: ConfigVersionId,
    has_build: bool,
    channels: BTreeMap<CredentialId, Channel>,
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
    let enabled_upstreams: BTreeSet<_> = configuration
        .upstreams
        .iter()
        .filter(|u| u.enabled)
        .map(|u| &u.id)
        .collect();
    let has_build = configuration.endpoints.iter().any(|endpoint| {
        endpoint.enabled
            && enabled_upstreams.contains(&endpoint.upstream_id)
            && endpoint.adapter_id == "grok.build.responses"
    });
    let codex_endpoints = configuration
        .endpoints
        .iter()
        .filter(|endpoint| {
            endpoint.enabled
                && enabled_upstreams.contains(&endpoint.upstream_id)
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
    let codex_credential_ids: BTreeSet<_> = configuration
        .credentials
        .iter()
        .filter(|credential| {
            credential.kind == "oauth_json"
                && credential.status == CredentialStatus::Active
                && bound_credentials.contains(&credential.id)
        })
        .map(|credential| credential.id.clone())
        .collect();
    let kimi_upstreams = configuration
        .upstreams
        .iter()
        .filter(|upstream| upstream.enabled && upstream.kind == "kimi-coding")
        .map(|upstream| &upstream.id)
        .collect::<BTreeSet<_>>();
    let kimi_endpoints = configuration
        .endpoints
        .iter()
        .filter(|endpoint| {
            endpoint.enabled
                && kimi_upstreams.contains(&endpoint.upstream_id)
                && endpoint.adapter_id == CODEX_RESPONSES_ADAPTER_ID
                && endpoint.base_url.trim_end_matches('/') == "https://api.kimi.com/coding"
                && endpoint.inference_path == "/v1/responses"
        })
        .map(|endpoint| (&endpoint.id, &endpoint.upstream_id))
        .collect::<BTreeSet<_>>();
    let kimi_bound_credentials = configuration
        .endpoint_credential_bindings
        .iter()
        .filter(|binding| {
            binding.enabled
                && kimi_endpoints.contains(&(&binding.endpoint_id, &binding.upstream_id))
        })
        .map(|binding| &binding.credential_id)
        .collect::<BTreeSet<_>>();
    let kimi_credential_ids: BTreeSet<_> = configuration
        .credentials
        .iter()
        .filter(|credential| {
            credential.kind == "oauth_json"
                && credential.status == CredentialStatus::Active
                && kimi_upstreams.contains(&credential.upstream_id)
                && kimi_bound_credentials.contains(&credential.id)
        })
        .map(|credential| credential.id.clone())
        .collect();
    let mut channels: BTreeMap<_, _> = codex_credential_ids
        .into_iter()
        .map(|id| (id, Channel::Codex))
        .chain(
            kimi_credential_ids
                .into_iter()
                .map(|id| (id, Channel::Kimi)),
        )
        .collect();
    extend_bearer_channels(configuration, &mut channels);
    RefreshScope {
        config_version_id: configuration.version.id.clone(),
        has_build,
        channels,
    }
}

fn extend_bearer_channels(
    configuration: &ControlPlaneConfiguration,
    channels: &mut BTreeMap<CredentialId, Channel>,
) {
    for upstream in configuration.upstreams.iter().filter(|u| u.enabled) {
        let channel = match upstream.kind.as_str() {
            "claude" => Channel::Claude,
            "kiro" => Channel::Kiro,
            _ => continue,
        };
        for endpoint in configuration
            .endpoints
            .iter()
            .filter(|e| e.enabled && e.upstream_id == upstream.id)
        {
            let supported = match channel {
                Channel::Claude => {
                    endpoint.adapter_id == "anthropic-compatible.messages"
                        && endpoint.base_url.trim_end_matches('/') == "https://api.anthropic.com/v1"
                        && endpoint.inference_path == "/messages"
                }
                Channel::Kiro => endpoint.adapter_id == "kiro.messages",
                _ => false,
            };
            if !supported {
                continue;
            }
            for binding in configuration
                .endpoint_credential_bindings
                .iter()
                .filter(|b| {
                    b.enabled && b.endpoint_id == endpoint.id && b.upstream_id == upstream.id
                })
            {
                if let Some(credential) = configuration.credentials.iter().find(|c| {
                    c.id == binding.credential_id
                        && c.upstream_id == upstream.id
                        && c.status == CredentialStatus::Active
                        && c.kind == "bearer"
                }) {
                    channels.insert(credential.id.clone(), channel);
                }
            }
        }
    }
}

pub(crate) struct RefreshStop {
    stopped: Arc<std::sync::atomic::AtomicBool>,
    notify: Arc<tokio::sync::Notify>,
}
impl RefreshStop {
    pub(crate) fn stop(&self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Release);
        self.notify.notify_one();
    }
}
struct StopOnDrop(Arc<std::sync::atomic::AtomicBool>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_runtime_material(
    configuration: &ControlPlaneConfiguration,
    credential_id: &CredentialId,
    runtime_bytes: &[u8],
    expires_at_ms: Option<i64>,
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
                    expires_at_ms,
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

pub(crate) fn complete_runtime_recovery(
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
            reauth_required: Cell::new(false),
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
            Err(_) if transport.reauth_required.get() => GrokAccountWorkerResult::ReauthRequired,
            Err(_) => GrokAccountWorkerResult::TransientFailure,
        }
    }
}

struct GrokBuildRefreshTransport {
    client: reqwest::blocking::Client,
    reauth_required: Cell<bool>,
}

impl GrokBuildOAuthTransport for GrokBuildRefreshTransport {
    fn user_info(
        &self,
        access_token: &str,
    ) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthTransportError> {
        let response = self
            .client
            .get(provider_grok::GROK_BUILD_USERINFO_URL)
            .timeout(Duration::from_secs(10))
            .bearer_auth(access_token)
            .send()
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        response
            .take((MAX_GROK_BUILD_OAUTH_HTTP_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)?;
        // Profile failures must not overwrite the token exchange status or revoke a valid grant.
        GrokBuildOAuthHttpResponse::try_new(status, bytes)
            .map_err(|_| GrokBuildOAuthTransportError::Unavailable)
    }

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
        self.reauth_required.set(
            ordinary::classify_rejection(status, &response_body)
                == gateway_store::credential_refresh::RefreshFailure::ReauthRequired,
        );
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

    use super::{CODEX_RESPONSES_ADAPTER_ID, CODEX_RESPONSES_BASE_URL, Channel, refresh_scope};
    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    #[allow(clippy::too_many_lines)]
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
        let kimi_upstream = UpstreamId::try_new("kimi-upstream")?;
        configuration.upstreams.extend([
            upstream(codex_upstream.clone(), "codex"),
            upstream(generic_upstream.clone(), "generic"),
            UpstreamConfiguration {
                id: kimi_upstream.clone(),
                name: "Kimi Coding".to_owned(),
                kind: "kimi-coding".to_owned(),
                enabled: true,
                tags_json: "[]".to_owned(),
                egress_policy_id: None,
            },
        ]);
        let codex_endpoint = EndpointId::try_new("codex-endpoint")?;
        let generic_endpoint = EndpointId::try_new("generic-endpoint")?;
        let build_endpoint = EndpointId::try_new("build-endpoint")?;
        let kimi_endpoint = EndpointId::try_new("kimi-endpoint")?;
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
            EndpointConfiguration {
                id: kimi_endpoint.clone(),
                upstream_id: kimi_upstream.clone(),
                adapter_id: CODEX_RESPONSES_ADAPTER_ID.to_owned(),
                api_format: "openai/responses".to_owned(),
                base_url: "https://api.kimi.com/coding".to_owned(),
                inference_path: "/v1/responses".to_owned(),
                models_path: Some("/v1/models".to_owned()),
                transport: EndpointTransport::Http,
                enabled: true,
            },
        ]);

        let secret_store = secret_store()?;
        let codex_credential = CredentialId::try_new("codex-oauth")?;
        let generic_oauth = CredentialId::try_new("generic-oauth")?;
        let unbound_oauth = CredentialId::try_new("unbound-oauth")?;
        let kimi_oauth = CredentialId::try_new("kimi-oauth")?;
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
            credential(
                &configuration,
                &secret_store,
                kimi_oauth.clone(),
                kimi_upstream.clone(),
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
            binding(kimi_endpoint, kimi_oauth.clone(), kimi_upstream, true),
        ]);

        let scope = refresh_scope(&configuration);
        assert!(scope.has_build);
        assert_eq!(scope.channels.len(), 2);
        assert_eq!(scope.channels.get(&codex_credential), Some(&Channel::Codex));
        assert_eq!(scope.channels.get(&kimi_oauth), Some(&Channel::Kimi));
        Ok(())
    }

    #[test]
    fn claude_and_kiro_scope_requires_enabled_exact_owner_binding() -> TestResult {
        let secrets = secret_store()?;
        for (kind, adapter, base, path, channel) in [
            (
                "claude",
                "anthropic-compatible.messages",
                "https://api.anthropic.com/v1",
                "/messages",
                Channel::Claude,
            ),
            (
                "kiro",
                "kiro.messages",
                "https://q.us-east-1.amazonaws.com",
                "/generateAssistantResponse",
                Channel::Kiro,
            ),
        ] {
            let mut config = ControlPlaneConfiguration::new(ConfigVersion {
                id: ConfigVersionId::try_new("scope")?,
                parent_id: None,
                status: ConfigVersionStatus::Active,
                revision: 1,
                created_at_ms: 0,
                description: String::new(),
            });
            let owner = UpstreamId::try_new("owner")?;
            let endpoint_id = EndpointId::try_new("endpoint")?;
            let credential_id = CredentialId::try_new("credential")?;
            let mut upstream = upstream(owner.clone(), kind);
            upstream.kind = kind.into();
            config.upstreams.push(upstream);
            let mut endpoint = endpoint(endpoint_id.clone(), owner.clone(), adapter, base, true);
            endpoint.inference_path = path.into();
            config.endpoints.push(endpoint);
            config.credentials.push(credential(
                &config,
                &secrets,
                credential_id.clone(),
                owner.clone(),
                "bearer",
            )?);
            config.endpoint_credential_bindings.push(binding(
                endpoint_id,
                credential_id.clone(),
                owner,
                true,
            ));
            assert_eq!(
                refresh_scope(&config).channels.get(&credential_id),
                Some(&channel)
            );
            config.endpoint_credential_bindings[0].enabled = false;
            assert!(refresh_scope(&config).channels.is_empty());
            config.endpoint_credential_bindings[0].enabled = true;
            config.credentials[0].status = CredentialStatus::Unauthorized;
            assert!(refresh_scope(&config).channels.is_empty());
            config.credentials[0].status = CredentialStatus::Active;
            config.upstreams[0].enabled = false;
            assert!(refresh_scope(&config).channels.is_empty());
        }
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
