//! Router-owned exact-token-count execution seam.

use std::{fmt, future::Future, pin::Pin};

use gateway_core::{CanonicalRequest, ExactInputTokenCount, GatewayError, RequestContext, RouteId};
use gateway_provider::{TokenCountCapability, token_count_unsupported};

/// A boxed, sendable exact-token-count operation without coupling the Router facade to a Provider
/// implementation or HTTP type.
pub type CountTokensFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One authenticated, decoded `count_tokens` request handed to the Router boundary.
///
/// The optional Route identity is the Snapshot-approved route selected by ingress. A later
/// aggregation executor can use it to choose a compatible endpoint, while the initial Provider
/// adapter preserves the existing direct capability seam.
#[derive(Clone)]
pub struct CountTokensExecution {
    context: RequestContext,
    request: CanonicalRequest,
    route_id: Option<RouteId>,
}

impl CountTokensExecution {
    /// Creates a canonical exact-token-count handoff after ingress admission and model resolution.
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
            .finish()
    }
}

/// Executes a `count_tokens` request only when it can return an exact result.
pub trait CountTokensExecutor: Send + Sync {
    /// Returns the exact input-token count or a safe explicit error.
    fn count_tokens(
        &self,
        execution: CountTokensExecution,
    ) -> CountTokensFuture<'_, Result<ExactInputTokenCount, GatewayError>>;
}

/// A direct Router executor backed by one explicit Provider token-count capability.
///
/// It exists for local/exact embeddings. P5-05 can replace it with an aggregation executor that
/// consumes [`CountTokensExecution::route_id`] to choose an Endpoint while keeping the same HTTP
/// and protocol seam.
#[derive(Clone, Debug, Default)]
pub struct ProviderCountTokensExecutor {
    capability: TokenCountCapability,
}

impl ProviderCountTokensExecutor {
    /// Creates an executor from an explicitly exact or explicitly unsupported capability.
    #[must_use]
    pub const fn new(capability: TokenCountCapability) -> Self {
        Self { capability }
    }
}

impl CountTokensExecutor for ProviderCountTokensExecutor {
    fn count_tokens(
        &self,
        execution: CountTokensExecution,
    ) -> CountTokensFuture<'_, Result<ExactInputTokenCount, GatewayError>> {
        let (context, request) = execution.into_legacy_parts();
        self.capability.count_tokens(context, request)
    }
}

/// A default executor that always declares the exact-count capability unavailable.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnsupportedCountTokensExecutor;

impl CountTokensExecutor for UnsupportedCountTokensExecutor {
    fn count_tokens(
        &self,
        _execution: CountTokensExecution,
    ) -> CountTokensFuture<'_, Result<ExactInputTokenCount, GatewayError>> {
        // Do not invoke a tokenizer or construct a synthetic request for this branch: the
        // rejection itself is the proof that no approximate fallback exists.
        Box::pin(async { Err(token_count_unsupported()) })
    }
}
