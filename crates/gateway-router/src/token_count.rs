//! Router-owned input-token-count execution seam.

use std::{fmt, future::Future, pin::Pin, sync::Arc};

use crate::{
    AttemptExclusionSet, ProviderScopedRouteExplainInput, RouteCredentialScheduler,
    RouteExplainInput, RouteSnapshot, RuntimeHealthRegistry, RuntimeQuotaRegistry,
    SelectedRouteCredential, SnapshotRouteCandidate,
};
use gateway_core::{
    CanonicalRequest, ExactInputTokenCount, GatewayError, InputTokenCount, RequestContext, RouteId,
};
use gateway_provider::{TokenCountCapability, token_count_unsupported};

/// A boxed, sendable input-token-count operation without coupling the Router facade to a Provider
/// implementation or HTTP type.
pub type CountTokensFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One authenticated, decoded `count_tokens` request handed to the Router boundary.
///
/// The optional Route identity is the Snapshot-approved route selected by ingress. Production
/// selection retains this exact snapshot and the admitted raw-model constraint; direct Provider
/// embeddings can continue to consume only the canonical request.
#[derive(Clone)]
pub struct CountTokensExecution {
    context: RequestContext,
    request: CanonicalRequest,
    route_id: Option<RouteId>,
    snapshot: Option<Arc<RouteSnapshot>>,
    exact_upstream_model: Option<String>,
    native_payload: Option<Arc<str>>,
}

impl CountTokensExecution {
    /// Creates a canonical input-token-count handoff after ingress admission and model resolution.
    #[must_use]
    pub fn new(
        context: RequestContext,
        request: CanonicalRequest,
        route_id: Option<RouteId>,
    ) -> Self {
        Self {
            context,
            request,
            route_id,
            snapshot: None,
            exact_upstream_model: None,
            native_payload: None,
        }
    }

    /// Returns the ingress correlation context.
    #[must_use]
    pub fn context(&self) -> &RequestContext {
        &self.context
    }

    /// Returns the decoded protocol-neutral request.
    #[must_use]
    pub fn request(&self) -> &CanonicalRequest {
        &self.request
    }

    /// Returns the Snapshot-approved Route when ingress resolved one.
    #[must_use]
    pub fn route_id(&self) -> Option<&RouteId> {
        self.route_id.as_ref()
    }

    /// Pins the same model visibility snapshot and raw model constraint admitted by HTTP.
    #[must_use]
    pub fn with_route_snapshot(
        mut self,
        snapshot: Option<Arc<RouteSnapshot>>,
        exact_upstream_model: Option<String>,
    ) -> Self {
        self.snapshot = snapshot;
        self.exact_upstream_model = exact_upstream_model;
        self
    }

    /// Retains the duplicate-checked Messages input for the counting provider.
    #[must_use]
    pub fn with_native_payload(mut self, payload: Arc<str>) -> Self {
        self.native_payload = Some(payload);
        self
    }

    /// Borrows input solely for counting; callers must not log it.
    #[must_use]
    pub fn native_payload(&self) -> Option<&str> {
        self.native_payload.as_deref()
    }

    /// Selects a live route credential without starting an inference Attempt or emitting Usage.
    ///
    /// # Errors
    /// Rejects stale snapshots, ambiguous provider scopes and unavailable credentials.
    pub fn select(
        &self,
        scheduler: &RouteCredentialScheduler,
        health: &RuntimeHealthRegistry,
        quota: &RuntimeQuotaRegistry,
        now_ms: i64,
        eligible: impl Fn(&SnapshotRouteCandidate) -> bool + Sync,
    ) -> Result<SelectedRouteCredential, GatewayError> {
        let unavailable = || {
            gateway_core::GatewayError::new(
                gateway_core::GatewayErrorCode::CredentialUnavailable,
                gateway_core::ErrorScope::Credential,
            )
        };
        let route_id = self.route_id.as_ref().ok_or_else(unavailable)?;
        let snapshot = self.snapshot.as_ref().ok_or_else(unavailable)?;
        let route = scheduler
            .route_from_snapshot(Some(snapshot), route_id)?
            .ok_or_else(unavailable)?;
        let eligible = |candidate: &SnapshotRouteCandidate| {
            candidate.is_hard_eligible_at(now_ms)
                && eligible(candidate)
                && self
                    .exact_upstream_model
                    .as_deref()
                    .is_none_or(|model| candidate.upstream_model() == model)
        };
        if self.exact_upstream_model.is_some() {
            return scheduler
                .select_eligible_and_lease_with_runtime_health_quota_and_binding_at_from_snapshot(
                    Some(snapshot),
                    route_id,
                    health,
                    quota,
                    now_ms,
                    eligible,
                    |_, _| true,
                );
        }
        let provider = crate::attempt_orchestrator::unique_provider_scope(&route, &eligible)?;
        let ids = route
            .candidates()
            .iter()
            .filter(|candidate| candidate.is_hard_eligible() && eligible(candidate))
            .map(|candidate| candidate.id().clone())
            .collect::<std::collections::BTreeSet<_>>();
        let prices = ids
            .iter()
            .filter_map(|id| {
                scheduler
                    .provider_price_rates_for_candidate(id)
                    .map(|rates| (id.clone(), rates))
            })
            .collect();
        let input = ProviderScopedRouteExplainInput::try_new(
            RouteExplainInput::new(route_id.clone(), now_ms),
            provider,
            ids,
            prices,
        )
        .map_err(|_| unavailable())?;
        let explain = scheduler
            .explain_provider_scoped_from_snapshot(
                Some(snapshot),
                &input,
                health,
                quota,
                &AttemptExclusionSet::new(),
            )
            .map_err(|_| unavailable())?;
        scheduler.select_provider_scoped_and_lease_at_from_snapshot(
            Some(snapshot),
            route_id,
            explain.provider_selection(),
            health,
            quota,
            now_ms,
            eligible,
            |_, _| true,
        )
    }

    fn into_legacy_parts(self) -> (RequestContext, CanonicalRequest) {
        (self.context, self.request)
    }
}

impl fmt::Debug for CountTokensExecution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CountTokensExecution")
            .field("context", &self.context)
            .field("request", &self.request)
            .field("route_id", &self.route_id)
            .finish_non_exhaustive()
    }
}

/// Executes a count request with explicit provenance, independently of inference.
pub trait CountTokensExecutor: Send + Sync {
    /// Returns a source-labelled input-token count or a safe explicit error.
    fn count_tokens(
        &self,
        execution: CountTokensExecution,
    ) -> CountTokensFuture<'_, Result<InputTokenCount, GatewayError>>;
}

/// Explicit provenance for a legacy exact capability, which does not carry its own source.
#[derive(Clone, Copy, Debug, Default)]
pub enum ExactTokenCountSource {
    /// An upstream counting capability reported the value.
    #[default]
    Upstream,
    /// An embedding has proven its local tokenizer compatible with the selected model.
    ProvenLocal,
}

impl ExactTokenCountSource {
    const fn label(self, count: ExactInputTokenCount) -> InputTokenCount {
        match self {
            Self::Upstream => InputTokenCount::ProviderReported(count.input_tokens()),
            Self::ProvenLocal => InputTokenCount::ExactLocal(count),
        }
    }
}

/// A direct Router executor backed by one explicit Provider token-count capability.
///
/// It preserves explicitly exact Provider embeddings. Production installs a routed executor
/// separately; embeddings explicitly declare upstream or proven local provenance.
#[derive(Clone, Debug, Default)]
pub struct ProviderCountTokensExecutor {
    capability: TokenCountCapability,
    source: ExactTokenCountSource,
}

impl ProviderCountTokensExecutor {
    /// Creates an executor with caller-declared provenance; exactness alone does not imply source.
    #[must_use]
    pub const fn new(capability: TokenCountCapability, source: ExactTokenCountSource) -> Self {
        Self { capability, source }
    }
}

impl CountTokensExecutor for ProviderCountTokensExecutor {
    fn count_tokens(
        &self,
        execution: CountTokensExecution,
    ) -> CountTokensFuture<'_, Result<InputTokenCount, GatewayError>> {
        let (context, request) = execution.into_legacy_parts();
        Box::pin(async move {
            self.capability
                .count_tokens(context, request)
                .await
                .map(|count| self.source.label(count))
        })
    }
}

/// A default executor that always declares the exact-count capability unavailable.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnsupportedCountTokensExecutor;

impl CountTokensExecutor for UnsupportedCountTokensExecutor {
    fn count_tokens(
        &self,
        _execution: CountTokensExecution,
    ) -> CountTokensFuture<'_, Result<InputTokenCount, GatewayError>> {
        // Do not invoke a tokenizer or construct a synthetic request for this branch: the
        // rejection itself is the proof that no approximate fallback exists.
        Box::pin(async { Err(token_count_unsupported()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_legacy_exact_provenance_requires_explicit_source() {
        let count = ExactInputTokenCount::new(17);
        assert_eq!(
            ExactTokenCountSource::Upstream.label(count).method(),
            "provider-reported"
        );
        assert_eq!(
            ExactTokenCountSource::ProvenLocal.label(count).method(),
            "proven-exact-local"
        );
    }
}
