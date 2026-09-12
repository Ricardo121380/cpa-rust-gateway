//! Complete serving generations, prepared before publication and captured once per request.
use super::{
    Arc, AtomicBool, ClientKeyService, ConfigRevision, ConfigVersionId, ControlPlaneConfiguration,
    GatewayEventSink, GrokBuildCacheIdentityDeriver, ManagementChannelPinFacade,
    ManagementChannelPinRequest, ManagementRuntimeError, ManagementRuntimeFacade, Mutex,
    NoActiveConfigurationExecutor, Ordering, P12AttemptStageStore, P12ChannelPinFacade,
    P12RoutedResponsesExecutor, ProviderAccountPoolComposition, ProviderAccountPoolFacade,
    ProviderEgressStatusFacade, RejectingManagementChannelPinFacade,
    RejectingProviderAccountPoolFacade, RejectingProviderEgressStatusFacade, RequestId,
    ResponsesHttpState, RouteSnapshotRegistry, RoutingPriceSnapshot, RuntimeCompositionError,
    RuntimeCompositionStage, RuntimeCredentialRefreshWorker, RuntimeHealthRegistry,
    RuntimeModelCatalogWorker, RuntimeQuotaRegistry, SecretStore, SnapshotManagementRuntimeFacade,
    SqliteCatalogSnapshotStore, SqliteControlPlaneRepository, SqliteEventStore,
    SqliteStoredResponseStore, SystemResponsesMetadataFactory, UpstreamProxy,
    compile_routing_price_snapshot, default_stream_capacity, system_now_ms_runtime,
};
use arc_swap::ArcSwap;
use gateway_control::snapshot_publisher::{
    PreparedRuntimePublication, RuntimePublicationPreparer, SnapshotPublicationError,
};
use gateway_http_actix::ResponsesStateSource;
use gateway_router::{RouteSnapshot, SnapshotClientKeyAuthenticator};
use std::path::PathBuf;

pub(super) struct RuntimeFactory {
    pub(super) database: PathBuf,
    pub(super) secret_store: SecretStore,
    pub(super) client_keys: Arc<ClientKeyService>,
    pub(super) cache_identity: Arc<GrokBuildCacheIdentityDeriver>,
    pub(super) codex_proxy: UpstreamProxy,
    pub(super) web_proxy: Option<UpstreamProxy>,
    pub(super) flaresolverr_proxy: Option<UpstreamProxy>,
    pub(super) flaresolverr_port: u16,
    pub(super) attempt_stages: Arc<P12AttemptStageStore>,
    pub(super) runtime_health: Arc<RuntimeHealthRegistry>,
    pub(super) runtime_quota: Arc<RuntimeQuotaRegistry>,
    pub(super) event_sink: Arc<dyn GatewayEventSink>,
    pub(super) stored_responses: Arc<SqliteStoredResponseStore>,
    pub(super) concurrency: gateway_upstream::CredentialConcurrencyRegistry,
    pub(super) publication_gate: Arc<tokio::sync::Mutex<()>>,
    pub(super) lifecycle_registry: Arc<RouteSnapshotRegistry>,
}

struct Generation {
    active: Arc<AtomicBool>,
    data: Arc<ResponsesHttpState>,
    runtime: Mutex<Box<dyn ManagementRuntimeFacade>>,
    pools: Box<dyn ProviderAccountPoolFacade>,
    egress: Box<dyn ProviderEgressStatusFacade>,
    channel_pin: Box<dyn ManagementChannelPinFacade>,
}

struct BuiltGeneration {
    generation: Arc<Generation>,
    workers: Workers,
    native_generation: i64,
}

struct Workers {
    refresh: Option<RuntimeCredentialRefreshWorker>,
    catalog: Option<RuntimeModelCatalogWorker>,
}

impl RuntimeFactory {
    #[allow(clippy::too_many_lines)] // Keep one complete, unpublished generation in one assembly path.
    fn build(
        &self,
        configuration: Option<&ControlPlaneConfiguration>,
        snapshot: Arc<RouteSnapshot>,
    ) -> Result<BuiltGeneration, RuntimeCompositionError> {
        let database = self.database.as_path();
        let secret_store = &self.secret_store;
        let native = provider_grok::GrokAccountPoolStore::try_open(database, secret_store.clone())
            .map_err(|_| RuntimeCompositionError::Unavailable)?;
        let stamp = native
            .managed_account_page(1, "", "", None)
            .map_err(|_| RuntimeCompositionError::Unavailable)?
            .stamp;
        let registry = Arc::new(RouteSnapshotRegistry::new(snapshot));
        let mut repository = SqliteControlPlaneRepository::open(database)
            .map_err(|_| RuntimeCompositionError::Unavailable)?;
        let attempt_stages = Arc::clone(&self.attempt_stages);
        let runtime_health = Arc::clone(&self.runtime_health);
        let runtime_quota = Arc::clone(&self.runtime_quota);
        let event_sink = Arc::clone(&self.event_sink);
        let grok_build_cache_identity_deriver = Arc::clone(&self.cache_identity);
        let codex_oauth_proxy = self.codex_proxy.clone();
        let web_proxy = self.web_proxy.clone();
        let flaresolverr_proxy = self.flaresolverr_proxy.clone();
        let flaresolverr_port = self.flaresolverr_port;
        let mut routing_price_snapshot: Option<Arc<RoutingPriceSnapshot>> = None;
        let active = Arc::new(AtomicBool::new(false));
        let (
            executor,
            provider_account_pools,
            route_explain_scheduler,
            provider_egress_status,
            channel_pin,
            credential_refresh_worker,
            model_catalog_worker,
        ): ProviderAccountPoolComposition = match configuration {
            Some(configuration) => {
                let observed_at_ms = system_now_ms_runtime()?;
                if let Some(policy) = configuration.routing_price_policy.as_ref() {
                    let catalog = repository
                        .load_billing_catalog(&policy.catalog_version_id)
                        .map_err(|_| {
                            RuntimeCompositionError::Stage(
                                RuntimeCompositionStage::RoutingPricePolicy,
                            )
                        })?
                        .ok_or(RuntimeCompositionError::Stage(
                            RuntimeCompositionStage::RoutingPricePolicy,
                        ))?;
                    let snapshot = registry.load();
                    let compiled = compile_routing_price_snapshot(
                        &snapshot,
                        &configuration.version.id,
                        policy,
                        &catalog,
                        u64::try_from(observed_at_ms).map_err(|_| {
                            RuntimeCompositionError::Stage(
                                RuntimeCompositionStage::RoutingPricePolicy,
                            )
                        })?,
                    )
                    .map_err(|_| {
                        RuntimeCompositionError::Stage(RuntimeCompositionStage::RoutingPricePolicy)
                    })?;
                    routing_price_snapshot = Some(Arc::new(compiled));
                }
                let (
                    executor,
                    provider_account_pools,
                    route_explain_scheduler,
                    provider_egress_status,
                    credential_refresh_worker,
                    model_catalog_worker,
                ) = P12RoutedResponsesExecutor::try_new(
                    database,
                    configuration,
                    secret_store,
                    Arc::clone(&registry),
                    Arc::clone(&attempt_stages),
                    Arc::clone(&event_sink),
                    Arc::clone(&runtime_health),
                    Arc::clone(&runtime_quota),
                    grok_build_cache_identity_deriver,
                    codex_oauth_proxy,
                    web_proxy,
                    flaresolverr_proxy,
                    flaresolverr_port,
                    routing_price_snapshot.as_ref(),
                    &self.concurrency,
                )?;
                let executor = Arc::new(executor);
                let channel_pin: Box<dyn ManagementChannelPinFacade> =
                    Box::new(P12ChannelPinFacade::new(Arc::clone(&executor)));
                (
                    executor,
                    provider_account_pools,
                    Some(route_explain_scheduler),
                    provider_egress_status,
                    channel_pin,
                    credential_refresh_worker,
                    model_catalog_worker,
                )
            }
            None => (
                Arc::new(NoActiveConfigurationExecutor),
                Box::new(RejectingProviderAccountPoolFacade::new()),
                None,
                Box::new(RejectingProviderEgressStatusFacade::new()),
                Box::new(RejectingManagementChannelPinFacade::new()),
                None,
                None,
            ),
        };
        let authenticator = Arc::new(SnapshotClientKeyAuthenticator::with_shared_verifier(
            Arc::clone(&registry),
            route_explain_scheduler.clone(),
            Arc::clone(&self.client_keys),
        ));
        let data = ResponsesHttpState::with_snapshot_metadata_and_event_sink(
            executor,
            Arc::new(SystemResponsesMetadataFactory::new()),
            authenticator,
            event_sink,
            default_stream_capacity().map_err(|_| RuntimeCompositionError::Unavailable)?,
        )
        .with_stored_response_store(Arc::clone(&self.stored_responses));
        let event_store = SqliteEventStore::open(database)
            .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::EventStore))?;
        let catalog_store = SqliteCatalogSnapshotStore::open(database)
            .map_err(|_| RuntimeCompositionError::Stage(RuntimeCompositionStage::Snapshot))?;
        native
            .managed_account_page(1, "", "", Some(stamp))
            .map_err(|_| RuntimeCompositionError::Unavailable)?;
        Ok(BuiltGeneration {
            generation: Arc::new(Generation {
                active: Arc::clone(&active),
                data: Arc::new(data),
                runtime: Mutex::new(Box::new(SnapshotManagementRuntimeFacade {
                    registry,
                    attempt_stages,
                    runtime_health,
                    runtime_quota,
                    route_explain_scheduler,
                    routing_price_snapshot,
                    event_store,
                    catalog_store,
                })),
                pools: provider_account_pools,
                egress: provider_egress_status,
                channel_pin,
            }),
            workers: Workers {
                refresh: credential_refresh_worker.map(|worker| {
                    worker.with_generation_guard(
                        Arc::clone(&self.publication_gate),
                        Arc::clone(&active),
                    )
                }),
                catalog: model_catalog_worker.map(|mut worker| {
                    worker.generation_guard = Some((Arc::clone(&self.publication_gate), active));
                    worker
                }),
            },
            native_generation: stamp,
        })
    }
}

type WorkerUpdate = Arc<Mutex<Option<Workers>>>;
type Bootstrap = (
    Arc<RuntimePublicationController>,
    ResponsesHttpState,
    Box<dyn ManagementRuntimeFacade>,
    Box<dyn ProviderAccountPoolFacade>,
    Box<dyn ProviderEgressStatusFacade>,
    Box<dyn ManagementChannelPinFacade>,
    Option<RuntimeCredentialRefreshWorker>,
    Option<RuntimeModelCatalogWorker>,
);

/// One publication pointer for HTTP admission, execution and management projections.
#[derive(Clone)]
pub(crate) struct RuntimePublicationController {
    available: Arc<AtomicBool>,
    current: Arc<ArcSwap<Generation>>,
    factory: Arc<RuntimeFactory>,
    workers: tokio::sync::watch::Sender<Option<WorkerUpdate>>,
}
impl std::fmt::Debug for RuntimePublicationController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RuntimePublicationController(<serving generation>)")
    }
}
impl RuntimePublicationController {
    pub(super) fn bootstrap(
        factory: RuntimeFactory,
        configuration: Option<&ControlPlaneConfiguration>,
    ) -> Result<Bootstrap, RuntimeCompositionError> {
        let initial = factory.build(configuration, factory.lifecycle_registry.load())?;
        initial.generation.active.store(true, Ordering::Release);
        let data = initial.generation.data.as_ref().clone();
        let (workers, _) = tokio::sync::watch::channel(None);
        let controller = Arc::new(Self {
            available: Arc::new(AtomicBool::new(true)),
            current: Arc::new(ArcSwap::from(initial.generation)),
            factory: Arc::new(factory),
            workers,
        });
        Ok((
            Arc::clone(&controller),
            data,
            Box::new(controller.as_ref().clone()),
            Box::new(controller.as_ref().clone()),
            Box::new(controller.as_ref().clone()),
            Box::new(controller.as_ref().clone()),
            initial.workers.refresh,
            initial.workers.catalog,
        ))
    }

    pub(crate) fn start_workers(
        &self,
        refresh: Option<RuntimeCredentialRefreshWorker>,
        catalog: Option<RuntimeModelCatalogWorker>,
    ) -> tokio::task::JoinHandle<()> {
        let mut updates = self.workers.subscribe();
        actix_web::rt::spawn(async move {
            let mut running = RunningWorkers::start(Workers { refresh, catalog });
            while updates.changed().await.is_ok() {
                let next = updates.borrow_and_update().clone();
                if let Some(next) = next {
                    let workers = next.lock().ok().and_then(|mut value| value.take());
                    if let Some(workers) = workers {
                        running.stop().await;
                        running = RunningWorkers::start(workers);
                    }
                }
            }
        })
    }
}

struct RunningWorkers(Vec<tokio::task::JoinHandle<()>>);
impl RunningWorkers {
    fn start(workers: Workers) -> Self {
        let mut handles = Vec::new();
        if let Some(worker) = workers.refresh {
            handles.push(actix_web::rt::spawn(async move {
                worker.run().await;
            }));
        }
        if let Some(worker) = workers.catalog {
            handles.push(actix_web::rt::spawn(async move {
                worker.run().await;
            }));
        }
        Self(handles)
    }
    async fn stop(&mut self) {
        for worker in &self.0 {
            worker.abort();
        }
        while let Some(worker) = self.0.pop() {
            let _ = worker.await;
        }
    }
}
impl Drop for RunningWorkers {
    fn drop(&mut self) {
        for worker in &self.0 {
            worker.abort();
        }
    }
}

struct PreparedGeneration {
    _guard: tokio::sync::OwnedMutexGuard<()>,
    controller: RuntimePublicationController,
    built: BuiltGeneration,
}
impl PreparedRuntimePublication for PreparedGeneration {
    fn native_inventory_generation(&self) -> Option<i64> {
        Some(self.built.native_generation)
    }
    fn commit(self: Box<Self>) {
        self.controller
            .current
            .load()
            .active
            .store(false, Ordering::Release);
        self.built.generation.active.store(true, Ordering::Release);
        self.controller.current.store(self.built.generation);
        self.controller.available.store(true, Ordering::Release);
        self.controller
            .workers
            .send_replace(Some(Arc::new(Mutex::new(Some(self.built.workers)))));
    }
}
impl RuntimePublicationPreparer for RuntimePublicationController {
    fn prepare(
        &self,
        configuration: &ControlPlaneConfiguration,
        snapshot: Arc<RouteSnapshot>,
    ) -> Result<Box<dyn PreparedRuntimePublication>, SnapshotPublicationError> {
        let guard = Arc::clone(&self.factory.publication_gate).blocking_lock_owned();
        let built = self
            .factory
            .build(Some(configuration), snapshot)
            .map_err(|_| SnapshotPublicationError::RuntimePreparation)?;
        Ok(Box::new(PreparedGeneration {
            _guard: guard,
            controller: self.clone(),
            built,
        }))
    }
}
impl ResponsesStateSource for RuntimePublicationController {
    fn try_capture(&self) -> Option<Arc<ResponsesHttpState>> {
        self.available
            .load(Ordering::Acquire)
            .then(|| self.capture())
    }
    fn capture(&self) -> Arc<ResponsesHttpState> {
        Arc::clone(&self.current.load().data)
    }
}

impl gateway_http_actix::management_resources::native_accounts::NativeAccountRuntime
    for RuntimePublicationController
{
    fn accepting_requests(&self) -> Option<bool> {
        Some(self.available.load(Ordering::Acquire))
    }

    fn apply(&self, recovered: Option<&str>) -> bool {
        let guard = Arc::clone(&self.factory.publication_gate).blocking_lock_owned();
        self.available.store(false, Ordering::Release);
        self.current.load().active.store(false, Ordering::Release);
        let prepared = (|| -> Result<BuiltGeneration, RuntimeCompositionError> {
            let mut repository = SqliteControlPlaneRepository::open(&self.factory.database)
                .map_err(|_| RuntimeCompositionError::Unavailable)?;
            let configuration = repository
                .load_active_configuration()
                .map_err(|_| RuntimeCompositionError::Unavailable)?;
            let snapshot = self.factory.lifecycle_registry.load();
            if configuration
                .as_ref()
                .is_some_and(|cfg| cfg.version.id.as_str() != snapshot.version().as_str())
            {
                return Err(RuntimeCompositionError::Unavailable);
            }
            let built = self.factory.build(configuration.as_ref(), snapshot)?;
            repository
                .check_runtime_sources(configuration.as_ref(), built.native_generation)
                .map_err(|_| RuntimeCompositionError::Unavailable)?;
            if let (Some(account), Some(configuration)) = (recovered, &configuration) {
                let credential = gateway_core::CredentialId::try_new(account)
                    .map_err(|_| RuntimeCompositionError::Unavailable)?;
                for endpoint in configuration
                    .endpoints
                    .iter()
                    .filter(|endpoint| super::is_native_grok_endpoint(endpoint))
                {
                    crate::credential_refresh::complete_runtime_recovery(
                        &self.factory.runtime_health,
                        &endpoint.id,
                        &credential,
                        system_now_ms_runtime()?,
                    )
                    .map_err(|_| RuntimeCompositionError::Unavailable)?;
                }
            }
            Ok(built)
        })();
        let Ok(built) = prepared else {
            return false;
        };
        Box::new(PreparedGeneration {
            _guard: guard,
            controller: self.clone(),
            built,
        })
        .commit();
        true
    }
}

use gateway_control::provider_account_pool_service as pools;
use gateway_control::provider_egress_status_service as egress;
use gateway_http_actix::management_resources as management;

impl ProviderAccountPoolFacade for RuntimePublicationController {
    fn list_provider_account_pools(
        &self,
        query: &pools::ProviderAccountPoolQuery,
    ) -> Result<pools::ProviderAccountPoolPage, pools::ProviderAccountPoolError> {
        self.current.load().pools.list_provider_account_pools(query)
    }
    fn apply_operator_action(
        &self,
        action: &pools::ProviderAccountOperatorAction,
        now: i64,
    ) -> Result<pools::ProviderAccountOperatorReceipt, pools::ProviderAccountPoolError> {
        self.current.load().pools.apply_operator_action(action, now)
    }
}
impl ProviderEgressStatusFacade for RuntimePublicationController {
    fn list_provider_egress_status(
        &self,
        version: &ConfigVersionId,
        revision: ConfigRevision,
        query: &egress::ProviderEgressStatusQuery,
    ) -> Result<egress::ProviderEgressStatusPage, egress::ProviderEgressStatusError> {
        self.current
            .load()
            .egress
            .list_provider_egress_status(version, revision, query)
    }
}
impl ManagementChannelPinFacade for RuntimePublicationController {
    fn execute(
        &self,
        request: ManagementChannelPinRequest,
    ) -> management::ManagementChannelPinFuture {
        let generation = self.current.load_full();
        Box::pin(async move { generation.channel_pin.execute(request).await })
    }
}
impl ManagementRuntimeFacade for RuntimePublicationController {
    fn effective_models(
        &mut self,
        version: &ConfigVersionId,
        context: &management::ManagementEffectiveModelContext,
        now: i64,
    ) -> Result<Option<management::ManagementEffectiveModels>, ManagementRuntimeError> {
        self.current
            .load()
            .runtime
            .lock()
            .map_err(|_| ManagementRuntimeError::Unavailable)?
            .effective_models(version, context, now)
    }
    fn catalog_status(
        &mut self,
        version: &ConfigVersionId,
        now: i64,
    ) -> Result<Vec<management::ManagementCatalogStatus>, ManagementRuntimeError> {
        self.current
            .load()
            .runtime
            .lock()
            .map_err(|_| ManagementRuntimeError::Unavailable)?
            .catalog_status(version, now)
    }
    fn runtime_availability(
        &mut self,
        version: &ConfigVersionId,
        now: i64,
    ) -> Result<Vec<management::ManagementRuntimeAvailabilityStatus>, ManagementRuntimeError> {
        self.current
            .load()
            .runtime
            .lock()
            .map_err(|_| ManagementRuntimeError::Unavailable)?
            .runtime_availability(version, now)
    }
    fn request_quota_recovery(
        &mut self,
        version: &ConfigVersionId,
        target: &management::ManagementRuntimeTarget,
        now: i64,
    ) -> Result<management::ManagementQuotaRecoveryState, ManagementRuntimeError> {
        self.current
            .load()
            .runtime
            .lock()
            .map_err(|_| ManagementRuntimeError::Unavailable)?
            .request_quota_recovery(version, target, now)
    }
    fn explain_route(
        &mut self,
        request: &management::ManagementRouteExplainRequest,
    ) -> Result<management::ManagementRouteExplain, ManagementRuntimeError> {
        self.current
            .load()
            .runtime
            .lock()
            .map_err(|_| ManagementRuntimeError::Unavailable)?
            .explain_route(request)
    }
    fn list_request_attempts(
        &mut self,
        request: &RequestId,
    ) -> Result<Vec<management::ManagementRequestAttempt>, ManagementRuntimeError> {
        self.current
            .load()
            .runtime
            .lock()
            .map_err(|_| ManagementRuntimeError::Unavailable)?
            .list_request_attempts(request)
    }
}
