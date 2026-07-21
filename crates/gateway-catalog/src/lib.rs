//! Immutable model Catalog and Endpoint-capability evidence for control-plane compilation.
//!
//! P2-06 deliberately keeps these types storage-neutral and explicitly injected. P4-01 owns
//! per-Endpoint/Credential discovery singleflight; P4-02 and later own snapshot freshness,
//! persistence, diffs, and runtime diagnostics.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::Arc,
};

use gateway_core::{CredentialId, EndpointId, ErrorScope, GatewayError, GatewayErrorCode};
use gateway_provider::{ProviderAdapter, ProviderFuture};
use tokio::sync::{Mutex, watch};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-catalog";

/// One validated model value returned by a Provider Catalog source before snapshot freshness is
/// assigned.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiscoveredModel {
    upstream_model: String,
}

impl DiscoveredModel {
    /// Creates one non-empty upstream model value.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::EmptyUpstreamModel`] when `upstream_model` is empty.
    pub fn try_new(upstream_model: impl Into<String>) -> Result<Self, CatalogViewError> {
        let upstream_model = upstream_model.into();
        if upstream_model.is_empty() {
            return Err(CatalogViewError::EmptyUpstreamModel);
        }
        Ok(Self { upstream_model })
    }

    /// Returns the exact source-provided upstream model identity.
    #[must_use]
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }
}

/// The one discovery identity that may share an in-flight model lookup.
///
/// It contains stable Endpoint and Credential identifiers only. Concrete Providers keep endpoint
/// address and credential material on their own side of the source boundary.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ModelCatalogTarget {
    endpoint_id: EndpointId,
    credential_id: CredentialId,
}

impl ModelCatalogTarget {
    /// Creates one exact Endpoint/Credential discovery identity.
    #[must_use]
    pub const fn new(endpoint_id: EndpointId, credential_id: CredentialId) -> Self {
        Self {
            endpoint_id,
            credential_id,
        }
    }

    /// Returns the Endpoint portion of this discovery identity.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the Credential portion of this discovery identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }
}

/// Provider-owned source for one Endpoint/Credential model discovery request.
///
/// The source may perform Provider-specific discovery, but it must not turn an Endpoint-wide
/// result into a Credential-wide entitlement. The scheduler below shares only calls with the exact
/// same [`ModelCatalogTarget`].
pub trait ModelCatalogSource: ProviderAdapter {
    /// Discovers models for one exact Endpoint/Credential identity.
    fn models(
        &self,
        target: ModelCatalogTarget,
    ) -> ProviderFuture<'_, Result<Vec<DiscoveredModel>, GatewayError>>;
}

type ModelCatalogResult = Result<Vec<DiscoveredModel>, GatewayError>;

struct InFlightModelCatalogSync {
    result: watch::Sender<Option<ModelCatalogResult>>,
}

/// Asynchronous per-Endpoint/Credential discovery scheduler with singleflight sharing.
///
/// One scheduler owns one Provider source. The first caller for a target starts one detached
/// discovery task; concurrent callers for the same exact target await its shared result. Different
/// Credentials, even on the same Endpoint, always receive independent source calls. The background
/// task continues if an initiating caller is cancelled so remaining followers cannot be stranded.
pub struct ModelCatalogScheduler {
    source: Arc<dyn ModelCatalogSource>,
    in_flight: Arc<Mutex<BTreeMap<ModelCatalogTarget, Arc<InFlightModelCatalogSync>>>>,
}

impl ModelCatalogScheduler {
    /// Creates a scheduler for one Provider-owned Catalog source.
    #[must_use]
    pub fn new(source: Arc<dyn ModelCatalogSource>) -> Self {
        Self {
            source,
            in_flight: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Starts or joins one discovery operation for `target`.
    ///
    /// Source output is sorted and deduplicated before every caller receives it. Source failures
    /// are shared verbatim as the existing safe [`GatewayError`] value and are not cached after the
    /// in-flight operation completes.
    ///
    /// # Errors
    ///
    /// Returns the safe source-provided error for this exact in-flight discovery, or an internal
    /// error if its detached task exits without publishing a result.
    pub async fn synchronize(
        &self,
        target: ModelCatalogTarget,
    ) -> Result<Vec<DiscoveredModel>, GatewayError> {
        let receiver = {
            let mut in_flight = self.in_flight.lock().await;
            if let Some(flight) = in_flight.get(&target) {
                flight.result.subscribe()
            } else {
                let (sender, receiver) = watch::channel::<Option<ModelCatalogResult>>(None);
                let flight = Arc::new(InFlightModelCatalogSync { result: sender });
                in_flight.insert(target.clone(), Arc::clone(&flight));
                Self::spawn_discovery(
                    Arc::clone(&self.source),
                    target,
                    Arc::clone(&self.in_flight),
                    flight,
                );
                receiver
            }
        };

        wait_for_result(receiver).await
    }

    fn spawn_discovery(
        source: Arc<dyn ModelCatalogSource>,
        target: ModelCatalogTarget,
        in_flight: Arc<Mutex<BTreeMap<ModelCatalogTarget, Arc<InFlightModelCatalogSync>>>>,
        flight: Arc<InFlightModelCatalogSync>,
    ) {
        let task = tokio::spawn(async move {
            let result = source.models(target.clone()).await.map(normalize_models);

            let mut in_flight = in_flight.lock().await;
            if in_flight
                .get(&target)
                .is_some_and(|current| Arc::ptr_eq(current, &flight))
            {
                in_flight.remove(&target);
            }
            drop(in_flight);

            // Remove the completed flight before publishing its result. Existing subscribers still
            // receive it, while a caller that arrives after completion starts a fresh discovery
            // instead of observing a result cache owned by this P4-01 scheduler.
            flight.result.send_replace(Some(result));
        });
        drop(task);
    }
}

async fn wait_for_result(
    mut receiver: watch::Receiver<Option<ModelCatalogResult>>,
) -> ModelCatalogResult {
    loop {
        if let Some(result) = receiver.borrow_and_update().as_ref().cloned() {
            return result;
        }
        if receiver.changed().await.is_err() {
            return Err(internal_error());
        }
    }
}

fn normalize_models(mut models: Vec<DiscoveredModel>) -> Vec<DiscoveredModel> {
    models.sort_unstable();
    models.dedup();
    models
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

/// One semantic capability relevant to public-model Route compilation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticCapability {
    /// Tool call support.
    Tools,
    /// Multiple independent Tool calls in one request/response.
    ParallelTools,
    /// Explicit Thinking or Reasoning support.
    Reasoning,
    /// JSON Schema response or Tool support.
    JsonSchema,
    /// Vision input support.
    Vision,
    /// Streaming response support.
    Streaming,
}

impl SemanticCapability {
    /// Returns the fixed configuration key used in P2-06 capability JSON.
    #[must_use]
    pub const fn json_key(self) -> &'static str {
        match self {
            Self::Tools => "tools",
            Self::ParallelTools => "parallel_tools",
            Self::Reasoning => "reasoning",
            Self::JsonSchema => "json_schema",
            Self::Vision => "vision",
            Self::Streaming => "streaming",
        }
    }

    /// Parses one fixed P2-06 configuration key.
    #[must_use]
    pub fn from_json_key(value: &str) -> Option<Self> {
        match value {
            "tools" => Some(Self::Tools),
            "parallel_tools" => Some(Self::ParallelTools),
            "reasoning" => Some(Self::Reasoning),
            "json_schema" => Some(Self::JsonSchema),
            "vision" => Some(Self::Vision),
            "streaming" => Some(Self::Streaming),
            _ => None,
        }
    }
}

/// A validated set of Endpoint or Route semantic capabilities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilitySet {
    capabilities: BTreeSet<SemanticCapability>,
}

impl CapabilitySet {
    /// Builds a capability set and enforces its intra-set invariants.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::ParallelToolsRequiresTools`] when `parallel_tools` is present
    /// without `tools`.
    pub fn try_new(
        capabilities: impl IntoIterator<Item = SemanticCapability>,
    ) -> Result<Self, CatalogViewError> {
        let capabilities = capabilities.into_iter().collect();
        let set = Self { capabilities };
        set.validate()?;
        Ok(set)
    }

    /// Returns an empty capability set.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            capabilities: BTreeSet::new(),
        }
    }

    /// Returns whether this set includes one capability.
    #[must_use]
    pub fn supports(&self, capability: SemanticCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Returns whether every capability in `required` is available in this set.
    #[must_use]
    pub fn supports_all(&self, required: &Self) -> bool {
        required.capabilities.is_subset(&self.capabilities)
    }

    /// Produces a narrowed set without ever adding a capability.
    ///
    /// Removing `tools` also removes `parallel_tools`, preserving the mandatory implication.
    #[must_use]
    pub fn without(&self, removed: impl IntoIterator<Item = SemanticCapability>) -> Self {
        let mut capabilities = self.capabilities.clone();
        for capability in removed {
            capabilities.remove(&capability);
            if capability == SemanticCapability::Tools {
                capabilities.remove(&SemanticCapability::ParallelTools);
            }
        }
        Self { capabilities }
    }

    /// Iterates over supported capabilities in stable enum order.
    pub fn iter(&self) -> impl Iterator<Item = SemanticCapability> + '_ {
        self.capabilities.iter().copied()
    }

    fn validate(&self) -> Result<(), CatalogViewError> {
        if self.supports(SemanticCapability::ParallelTools)
            && !self.supports(SemanticCapability::Tools)
        {
            return Err(CatalogViewError::ParallelToolsRequiresTools);
        }
        Ok(())
    }
}

/// Freshness/provenance state of one `(Endpoint, upstream model)` Catalog record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogModelState {
    /// An explicit manual model allowlist entry.
    Manual,
    /// A current discovery-backed Catalog entry.
    Fresh,
    /// A retained last-success Catalog entry that has not expired.
    Stale,
    /// A Catalog entry no longer accepted without an explicit compiler exception.
    Expired,
}

impl CatalogModelState {
    /// Returns whether this state is hard-eligible without an explicit exception.
    #[must_use]
    pub const fn is_hard_eligible(self) -> bool {
        matches!(self, Self::Manual | Self::Fresh | Self::Stale)
    }
}

/// One storage-neutral model Catalog record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogModelEntry {
    /// Endpoint that exposes the model.
    pub endpoint_id: EndpointId,
    /// Exact non-empty upstream model string.
    pub upstream_model: String,
    /// Catalog freshness/provenance state.
    pub state: CatalogModelState,
}

impl CatalogModelEntry {
    /// Creates one model Catalog record.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::EmptyUpstreamModel`] when `upstream_model` is empty.
    pub fn try_new(
        endpoint_id: EndpointId,
        upstream_model: impl Into<String>,
        state: CatalogModelState,
    ) -> Result<Self, CatalogViewError> {
        let upstream_model = upstream_model.into();
        if upstream_model.is_empty() {
            return Err(CatalogViewError::EmptyUpstreamModel);
        }
        Ok(Self {
            endpoint_id,
            upstream_model,
            state,
        })
    }
}

/// Immutable lookup of model Catalog state by Endpoint and upstream model.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CatalogView {
    entries: BTreeMap<(EndpointId, String), CatalogModelState>,
}

impl CatalogView {
    /// Builds a Catalog lookup and rejects ambiguous duplicate records.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::DuplicateCatalogModel`] for duplicate endpoint/model pairs.
    pub fn try_new(
        entries: impl IntoIterator<Item = CatalogModelEntry>,
    ) -> Result<Self, CatalogViewError> {
        let mut view = Self::default();
        for entry in entries {
            let key = (entry.endpoint_id, entry.upstream_model);
            if view.entries.insert(key, entry.state).is_some() {
                return Err(CatalogViewError::DuplicateCatalogModel);
            }
        }
        Ok(view)
    }

    /// Returns the stored Catalog state for one exact Endpoint/model pair.
    #[must_use]
    pub fn model_state(
        &self,
        endpoint_id: &EndpointId,
        upstream_model: &str,
    ) -> Option<CatalogModelState> {
        self.entries
            .get(&(endpoint_id.clone(), upstream_model.to_owned()))
            .copied()
    }
}

/// One injected semantic-capability profile for a concrete Endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointCapabilityEntry {
    /// Endpoint whose profile is described.
    pub endpoint_id: EndpointId,
    /// All semantic capabilities supported by that Endpoint.
    pub capabilities: CapabilitySet,
}

/// Immutable lookup of semantic capabilities by Endpoint.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EndpointCapabilityView {
    entries: BTreeMap<EndpointId, CapabilitySet>,
}

impl EndpointCapabilityView {
    /// Builds an Endpoint capability lookup and rejects duplicate Endpoint profiles.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::DuplicateEndpointCapabilityProfile`] for duplicate Endpoint
    /// identities.
    pub fn try_new(
        entries: impl IntoIterator<Item = EndpointCapabilityEntry>,
    ) -> Result<Self, CatalogViewError> {
        let mut view = Self::default();
        for entry in entries {
            if view
                .entries
                .insert(entry.endpoint_id, entry.capabilities)
                .is_some()
            {
                return Err(CatalogViewError::DuplicateEndpointCapabilityProfile);
            }
        }
        Ok(view)
    }

    /// Returns the injected profile for one Endpoint.
    #[must_use]
    pub fn capabilities_for(&self, endpoint_id: &EndpointId) -> Option<&CapabilitySet> {
        self.entries.get(endpoint_id)
    }
}

/// Safe construction errors for injected Catalog/capability evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogViewError {
    /// A Catalog model string was empty.
    EmptyUpstreamModel,
    /// More than one Catalog record described the same Endpoint/model pair.
    DuplicateCatalogModel,
    /// More than one capability record described the same Endpoint.
    DuplicateEndpointCapabilityProfile,
    /// Parallel Tool support appeared without ordinary Tool support.
    ParallelToolsRequiresTools,
}

impl fmt::Display for CatalogViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyUpstreamModel => {
                formatter.write_str("Catalog upstream model must not be empty")
            }
            Self::DuplicateCatalogModel => {
                formatter.write_str("Catalog contains a duplicate Endpoint/model record")
            }
            Self::DuplicateEndpointCapabilityProfile => formatter
                .write_str("Endpoint capability view contains a duplicate Endpoint profile"),
            Self::ParallelToolsRequiresTools => {
                formatter.write_str("parallel Tool capability requires Tool capability")
            }
        }
    }
}

impl Error for CatalogViewError {}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use gateway_core::{
        CredentialId, EndpointId, ErrorScope, GatewayError, GatewayErrorCode, ProviderId,
    };
    use gateway_provider::{ProviderAdapter, ProviderFuture};
    use tokio::{
        sync::Notify,
        task::yield_now,
        time::{error::Elapsed, timeout},
    };

    use super::{
        CapabilitySet, CatalogModelEntry, CatalogModelState, CatalogView, CatalogViewError,
        DiscoveredModel, EndpointCapabilityEntry, EndpointCapabilityView, ModelCatalogScheduler,
        ModelCatalogSource, ModelCatalogTarget, SemanticCapability,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[derive(Debug)]
    struct BlockingCatalogSource {
        provider_id: ProviderId,
        calls: AtomicUsize,
        waiting: AtomicUsize,
        failures_remaining: AtomicUsize,
        started: Notify,
        ready_to_release: Notify,
        release: Notify,
    }

    impl BlockingCatalogSource {
        fn new(failures_remaining: usize) -> Result<Self, Box<dyn Error>> {
            Ok(Self {
                provider_id: ProviderId::try_new("catalog-test-provider")?,
                calls: AtomicUsize::new(0),
                waiting: AtomicUsize::new(0),
                failures_remaining: AtomicUsize::new(failures_remaining),
                started: Notify::new(),
                ready_to_release: Notify::new(),
                release: Notify::new(),
            })
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        async fn wait_for_started(&self) -> Result<(), Elapsed> {
            timeout(Duration::from_secs(1), self.started.notified()).await
        }

        async fn wait_until_waiting(&self, expected: usize) -> Result<(), Elapsed> {
            timeout(Duration::from_secs(1), async {
                while self.waiting.load(Ordering::SeqCst) < expected {
                    self.ready_to_release.notified().await;
                }
            })
            .await
        }
    }

    impl ProviderAdapter for BlockingCatalogSource {
        fn provider_id(&self) -> &ProviderId {
            &self.provider_id
        }
    }

    impl ModelCatalogSource for BlockingCatalogSource {
        fn models(
            &self,
            target: ModelCatalogTarget,
        ) -> ProviderFuture<'_, Result<Vec<DiscoveredModel>, GatewayError>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let should_fail = self
                .failures_remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok();
            let started = &self.started;
            let waiting = &self.waiting;
            let ready_to_release = &self.ready_to_release;
            let release = &self.release;

            Box::pin(async move {
                started.notify_one();
                waiting.fetch_add(1, Ordering::SeqCst);
                ready_to_release.notify_one();
                release.notified().await;

                if should_fail {
                    return Err(GatewayError::new(
                        GatewayErrorCode::ProviderTransient,
                        ErrorScope::Provider,
                    ));
                }

                let credential_model = DiscoveredModel::try_new(format!(
                    "credential-model-{}",
                    target.credential_id().as_str()
                ))
                .map_err(|_| {
                    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
                })?;
                let shared_model = DiscoveredModel::try_new("shared-model").map_err(|_| {
                    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
                })?;

                Ok(vec![
                    credential_model.clone(),
                    shared_model,
                    credential_model,
                ])
            })
        }
    }

    fn target(credential: &str) -> Result<ModelCatalogTarget, Box<dyn Error>> {
        Ok(ModelCatalogTarget::new(
            EndpointId::try_new("endpoint-a")?,
            CredentialId::try_new(credential)?,
        ))
    }

    fn model_names(models: &[DiscoveredModel]) -> Vec<&str> {
        models.iter().map(DiscoveredModel::upstream_model).collect()
    }

    async fn wait_for_receiver_count(
        scheduler: &ModelCatalogScheduler,
        target: &ModelCatalogTarget,
        expected: usize,
    ) -> Result<(), Elapsed> {
        timeout(Duration::from_secs(1), async {
            loop {
                let count = {
                    let in_flight = scheduler.in_flight.lock().await;
                    in_flight
                        .get(target)
                        .map_or(0, |flight| flight.result.receiver_count())
                };
                if count >= expected {
                    return;
                }
                yield_now().await;
            }
        })
        .await
    }

    async fn wait_until_not_in_flight(
        scheduler: &ModelCatalogScheduler,
        target: &ModelCatalogTarget,
    ) -> Result<(), Elapsed> {
        timeout(Duration::from_secs(1), async {
            loop {
                if !scheduler.in_flight.lock().await.contains_key(target) {
                    return;
                }
                yield_now().await;
            }
        })
        .await
    }

    async fn receiver_count(
        scheduler: &ModelCatalogScheduler,
        target: &ModelCatalogTarget,
    ) -> Option<usize> {
        scheduler
            .in_flight
            .lock()
            .await
            .get(target)
            .map(|flight| flight.result.receiver_count())
    }

    #[tokio::test]
    async fn same_endpoint_and_credential_share_one_concurrent_discovery() -> TestResult {
        let source = Arc::new(BlockingCatalogSource::new(0)?);
        let scheduler = Arc::new(ModelCatalogScheduler::new(source.clone()));
        let catalog_target = target("credential-a")?;

        let first_scheduler = Arc::clone(&scheduler);
        let first_target = catalog_target.clone();
        let first = tokio::spawn(async move { first_scheduler.synchronize(first_target).await });
        source.wait_for_started().await?;
        source.wait_until_waiting(1).await?;

        let second_scheduler = Arc::clone(&scheduler);
        let second_target = catalog_target.clone();
        let second = tokio::spawn(async move { second_scheduler.synchronize(second_target).await });
        wait_for_receiver_count(&scheduler, &catalog_target, 2).await?;
        assert_eq!(source.call_count(), 1);

        source.release.notify_waiters();
        let first_models = first.await??;
        let second_models = second.await??;

        assert_eq!(first_models, second_models);
        assert_eq!(
            model_names(&first_models),
            vec!["credential-model-credential-a", "shared-model"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn same_endpoint_with_different_credentials_never_share_discovery() -> TestResult {
        let source = Arc::new(BlockingCatalogSource::new(0)?);
        let scheduler = Arc::new(ModelCatalogScheduler::new(source.clone()));
        let first_target = target("credential-a")?;
        let second_target = target("credential-b")?;

        let first_scheduler = Arc::clone(&scheduler);
        let first = tokio::spawn(async move { first_scheduler.synchronize(first_target).await });
        source.wait_for_started().await?;

        let second_scheduler = Arc::clone(&scheduler);
        let second = tokio::spawn(async move { second_scheduler.synchronize(second_target).await });
        source.wait_for_started().await?;
        source.wait_until_waiting(2).await?;
        assert_eq!(source.call_count(), 2);

        source.release.notify_waiters();
        let first_models = first.await??;
        let second_models = second.await??;

        assert_eq!(
            model_names(&first_models),
            vec!["credential-model-credential-a", "shared-model"]
        );
        assert_eq!(
            model_names(&second_models),
            vec!["credential-model-credential-b", "shared-model"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn initiating_caller_cancellation_does_not_strand_a_later_follower() -> TestResult {
        let source = Arc::new(BlockingCatalogSource::new(0)?);
        let scheduler = Arc::new(ModelCatalogScheduler::new(source.clone()));
        let catalog_target = target("credential-a")?;

        let leader_scheduler = Arc::clone(&scheduler);
        let leader_target = catalog_target.clone();
        let leader = tokio::spawn(async move { leader_scheduler.synchronize(leader_target).await });
        source.wait_for_started().await?;
        source.wait_until_waiting(1).await?;
        leader.abort();
        match leader.await {
            Err(error) if error.is_cancelled() => {}
            Err(error) => return Err(error.into()),
            Ok(_) => {
                return Err(
                    std::io::Error::other("initiating caller unexpectedly completed").into(),
                );
            }
        }
        assert_eq!(receiver_count(&scheduler, &catalog_target).await, Some(0));

        let follower_scheduler = Arc::clone(&scheduler);
        let follower_target = catalog_target.clone();
        let follower =
            tokio::spawn(async move { follower_scheduler.synchronize(follower_target).await });
        wait_for_receiver_count(&scheduler, &catalog_target, 1).await?;
        assert_eq!(source.call_count(), 1);

        source.release.notify_waiters();
        let models = follower.await??;
        assert_eq!(
            model_names(&models),
            vec!["credential-model-credential-a", "shared-model"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn failed_discovery_is_shared_but_not_retained_as_a_result_cache() -> TestResult {
        let source = Arc::new(BlockingCatalogSource::new(1)?);
        let scheduler = Arc::new(ModelCatalogScheduler::new(source.clone()));
        let catalog_target = target("credential-a")?;

        let first_scheduler = Arc::clone(&scheduler);
        let first_target = catalog_target.clone();
        let first = tokio::spawn(async move { first_scheduler.synchronize(first_target).await });
        source.wait_for_started().await?;
        source.wait_until_waiting(1).await?;

        let second_scheduler = Arc::clone(&scheduler);
        let second_target = catalog_target.clone();
        let second = tokio::spawn(async move { second_scheduler.synchronize(second_target).await });
        wait_for_receiver_count(&scheduler, &catalog_target, 2).await?;
        source.release.notify_waiters();

        let Err(first_error) = first.await? else {
            return Err(std::io::Error::other("first discovery unexpectedly succeeded").into());
        };
        let Err(second_error) = second.await? else {
            return Err(std::io::Error::other("joined discovery unexpectedly succeeded").into());
        };
        assert_eq!(first_error, second_error);
        assert_eq!(source.call_count(), 1);
        wait_until_not_in_flight(&scheduler, &catalog_target).await?;

        let retry_scheduler = Arc::clone(&scheduler);
        let retry_target = catalog_target.clone();
        let retry = tokio::spawn(async move { retry_scheduler.synchronize(retry_target).await });
        source.wait_for_started().await?;
        source.wait_until_waiting(2).await?;
        assert_eq!(source.call_count(), 2);
        source.release.notify_waiters();

        let retry_models = retry.await??;
        assert_eq!(
            model_names(&retry_models),
            vec!["credential-model-credential-a", "shared-model"]
        );
        Ok(())
    }

    #[test]
    fn parallel_tools_requires_tools_and_narrowing_keeps_the_invariant() -> TestResult {
        assert_eq!(
            CapabilitySet::try_new([SemanticCapability::ParallelTools]),
            Err(CatalogViewError::ParallelToolsRequiresTools)
        );
        let capabilities = CapabilitySet::try_new([
            SemanticCapability::Tools,
            SemanticCapability::ParallelTools,
            SemanticCapability::Streaming,
        ])?;
        let narrowed = capabilities.without([SemanticCapability::Tools]);
        assert!(!narrowed.supports(SemanticCapability::Tools));
        assert!(!narrowed.supports(SemanticCapability::ParallelTools));
        assert!(narrowed.supports(SemanticCapability::Streaming));
        Ok(())
    }

    #[test]
    fn catalog_states_and_duplicate_records_are_explicit() -> TestResult {
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let catalog = CatalogView::try_new([CatalogModelEntry::try_new(
            endpoint.clone(),
            "upstream-model-a",
            CatalogModelState::Stale,
        )?])?;
        assert_eq!(
            catalog.model_state(&endpoint, "upstream-model-a"),
            Some(CatalogModelState::Stale)
        );
        assert!(CatalogModelState::Stale.is_hard_eligible());
        assert!(!CatalogModelState::Expired.is_hard_eligible());
        assert_eq!(
            CatalogView::try_new([
                CatalogModelEntry::try_new(
                    endpoint.clone(),
                    "upstream-model-a",
                    CatalogModelState::Fresh,
                )?,
                CatalogModelEntry::try_new(
                    endpoint,
                    "upstream-model-a",
                    CatalogModelState::Manual,
                )?,
            ]),
            Err(CatalogViewError::DuplicateCatalogModel)
        );
        Ok(())
    }

    #[test]
    fn endpoint_capability_view_rejects_ambiguous_profiles() -> TestResult {
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let capabilities = CapabilitySet::try_new([SemanticCapability::Tools])?;
        assert_eq!(
            EndpointCapabilityView::try_new([
                EndpointCapabilityEntry {
                    endpoint_id: endpoint.clone(),
                    capabilities: capabilities.clone(),
                },
                EndpointCapabilityEntry {
                    endpoint_id: endpoint,
                    capabilities,
                },
            ]),
            Err(CatalogViewError::DuplicateEndpointCapabilityProfile)
        );
        Ok(())
    }
}
