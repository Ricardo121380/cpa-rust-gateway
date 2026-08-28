//! Small, core-only Provider execution contracts.

use std::{future::Future, pin::Pin};

use gateway_core::{CanonicalEvent, CanonicalRequest, GatewayError, ProviderId, RequestContext};

/// A boxed, sendable Provider operation without coupling the public trait to an async-trait macro.
pub type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Stable identity shared by all capability traits implemented by one Provider family.
pub trait ProviderAdapter: Send + Sync {
    /// Returns the stable Provider family identifier without exposing credentials or endpoints.
    fn provider_id(&self) -> &ProviderId;
}

/// The sole P1 Provider capability: execute one canonical request and supply canonical events.
///
/// Catalog discovery, routing, credential selection, retries, continuations, and transport are
/// deliberately outside this trait. A pre-response failure is returned from [`Self::execute`]; a
/// failure after response start is represented by a terminal [`CanonicalEvent::StreamError`] from
/// the returned source.
pub trait InferenceAdapter: ProviderAdapter {
    /// Starts one inference attempt and returns a pull-only canonical event source.
    fn execute(
        &self,
        context: RequestContext,
        request: CanonicalRequest,
    ) -> ProviderFuture<'_, Result<Box<dyn CanonicalEventSource>, GatewayError>>;
}

/// Pull-only source of ordered canonical events for one Provider attempt.
///
/// The consumer owns polling and may drop the source or an outstanding future to cancel local
/// work. This boundary has no sender, bounded queue, HTTP byte writer, or `FirstSemanticEvent`
/// capability; P1-07 composes those concerns.
pub trait CanonicalEventSource: Send {
    /// Returns the next canonical event or normal end-of-source.
    ///
    /// The P1 Mock never returns an out-of-band error after it has supplied a source. A later
    /// source must represent a failure after `ResponseStart` with terminal `StreamError`, rather
    /// than returning an error from this method.
    fn next_event(&mut self) -> ProviderFuture<'_, Result<Option<CanonicalEvent>, GatewayError>>;
}
