//! Immutable route snapshot and two-stage scheduling boundary.
//!
//! P1 also keeps the Provider execution contract behind this crate so an HTTP transport can
//! execute a canonical request without importing Provider traits or concrete Provider types.

#![deny(unsafe_code)]

mod credential_scheduler;
mod route_scheduler;
mod route_snapshot;
mod runtime_health;

use std::{future::Future, pin::Pin, time::Duration};

use gateway_core::{CanonicalEvent, CanonicalRequest, GatewayError, ProviderId, RequestContext};
use gateway_provider::{
    CanonicalEventSource, DeterministicMockProvider, InferenceAdapter, MockEmission, MockFixture,
    ProviderFuture,
};

pub use credential_scheduler::{RouteCredentialScheduler, SelectedRouteCredential};
pub use route_scheduler::RouteCandidateScheduler;
pub use route_snapshot::{
    MAX_SCHEDULE_SLOTS_PER_PRIORITY_TIER, PreparedSnapshotPublication, RouteSnapshot,
    RouteSnapshotBuildError, RouteSnapshotInput, RouteSnapshotRegistry, SnapshotAccessGroup,
    SnapshotCatalogAdmission, SnapshotClientKeyAuthenticator, SnapshotClientKeyClock,
    SnapshotClientKeyClockError, SnapshotClientKeyView, SnapshotPriorityTierSchedule,
    SnapshotPublicModel, SnapshotRegistryError, SnapshotRoute, SnapshotRouteCandidate,
    SnapshotRouteCandidateInput, SnapshotRoutePolicy, SnapshotRouteSchedule, SnapshotTransformMode,
    SnapshotTransition, SnapshotVersion, SystemSnapshotClientKeyClock,
};
pub use runtime_health::{
    DEFAULT_RUNTIME_HEALTH_SHARD_COUNT, MAX_RUNTIME_HEALTH_ENTRIES_PER_SHARD,
    MAX_RUNTIME_HEALTH_SHARD_COUNT, RuntimeHealthAvailability, RuntimeHealthClock,
    RuntimeHealthClockError, RuntimeHealthError, RuntimeHealthKey, RuntimeHealthRegistry,
    RuntimeHealthRegistryBuildError, SystemRuntimeHealthClock,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-router";

/// A boxed, sendable route-execution operation without coupling this facade to an async-trait
/// macro or a Provider type.
pub type ResponsesFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Starts one selected Responses execution without exposing a Provider boundary to transport
/// crates.
///
/// P1 deliberately has no catalog lookup, retry policy, credential selection, or route snapshot.
/// A later router implementation can add those internals while preserving this core-only surface.
pub trait ResponsesExecutor: Send + Sync {
    /// Starts one execution and returns its pull-only canonical event source.
    fn execute(
        &self,
        context: RequestContext,
        request: CanonicalRequest,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>>;
}

/// Pull-only canonical output available to an HTTP or other downstream transport.
///
/// It is intentionally distinct from `gateway_provider::CanonicalEventSource`: transports see
/// only router-owned canonical types, never Provider traits or concrete Provider implementations.
pub trait ResponsesEventSource: Send {
    /// Returns the next canonical event or normal end-of-source.
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>>;
}

/// One deterministic P1 mock event scheduled relative to the preceding source pull.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeterministicMockEmission {
    after: Duration,
    event: CanonicalEvent,
}

impl DeterministicMockEmission {
    /// Creates one scheduled event for [`DeterministicMockResponsesExecutor`].
    #[must_use]
    pub const fn new(after: Duration, event: CanonicalEvent) -> Self {
        Self { after, event }
    }

    /// Returns the delay before this event becomes available.
    #[must_use]
    pub const fn after(&self) -> Duration {
        self.after
    }

    /// Returns the canonical event retained by this fixture entry.
    #[must_use]
    pub const fn event(&self) -> &CanonicalEvent {
        &self.event
    }
}

/// P1's router-facing deterministic executor backed internally by the P1-06 Mock Provider.
///
/// Its public constructor accepts only canonical data, so `gateway-http-actix` can use the P1
/// vertical slice without a direct dependency on `gateway-provider` or leaked Provider types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeterministicMockResponsesExecutor {
    provider: DeterministicMockProvider,
}

impl DeterministicMockResponsesExecutor {
    /// Validates a canonical mock script and creates a reusable P1 executor.
    ///
    /// # Errors
    ///
    /// Returns the existing canonical stream lifecycle error when `emissions` is malformed or
    /// incomplete.
    pub fn try_new(
        provider_id: ProviderId,
        emissions: Vec<DeterministicMockEmission>,
    ) -> Result<Self, GatewayError> {
        let emissions = emissions
            .into_iter()
            .map(|emission| MockEmission::new(emission.after, emission.event))
            .collect();
        let fixture = MockFixture::try_events(emissions)?;

        Ok(Self {
            provider: DeterministicMockProvider::new(provider_id, fixture),
        })
    }
}

impl ResponsesExecutor for DeterministicMockResponsesExecutor {
    fn execute(
        &self,
        context: RequestContext,
        request: CanonicalRequest,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        let provider = self.provider.clone();

        Box::pin(async move {
            let source = provider.execute(context, request).await?;
            Ok(Box::new(ProviderResponsesEventSource { source }) as Box<dyn ResponsesEventSource>)
        })
    }
}

/// Private adapter that keeps the Provider source behind the router facade.
struct ProviderResponsesEventSource {
    source: Box<dyn CanonicalEventSource>,
}

impl ResponsesEventSource for ProviderResponsesEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        let future: ProviderFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> =
            self.source.next_event();
        future
    }
}
