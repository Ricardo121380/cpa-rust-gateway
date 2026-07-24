//! Explicit exact-token-count Provider capability.

use std::{fmt, sync::Arc};

use gateway_core::{
    CanonicalRequest, ErrorScope, ExactInputTokenCount, GatewayError, GatewayErrorCode,
    RequestContext,
};

use crate::{ProviderAdapter, ProviderFuture};

/// A Provider capability that can count a canonical request with the selected model's exact
/// tokenizer semantics.
///
/// Implementors must return a value only when they can make that exactness claim for the selected
/// route. They must return a safe error rather than estimate from bytes, characters, or a different
/// tokenizer.
pub trait ExactTokenCountAdapter: ProviderAdapter {
    /// Returns the selected Provider's exact input-token count for one canonical request.
    fn count_exact_tokens(
        &self,
        context: RequestContext,
        request: CanonicalRequest,
    ) -> ProviderFuture<'_, Result<ExactInputTokenCount, GatewayError>>;
}

/// A route-selected token-count capability that is either explicitly exact or explicitly absent.
///
/// There is deliberately no best-effort or estimated variant. Callers can therefore return a count
/// only through an [`ExactTokenCountAdapter`] or reject the request without inventing a value.
#[derive(Clone, Default)]
pub struct TokenCountCapability {
    adapter: Option<Arc<dyn ExactTokenCountAdapter>>,
}

impl TokenCountCapability {
    /// Creates an explicit absence of exact token-count support.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self { adapter: None }
    }

    /// Creates a capability backed by an adapter that attests to exact tokenizer compatibility.
    #[must_use]
    pub fn exact(adapter: Arc<dyn ExactTokenCountAdapter>) -> Self {
        Self {
            adapter: Some(adapter),
        }
    }

    /// Returns whether this route can return an exact count.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        self.adapter.is_some()
    }

    /// Counts one request exactly or returns an explicit unsupported-capability error.
    #[must_use]
    pub fn count_tokens(
        &self,
        context: RequestContext,
        request: CanonicalRequest,
    ) -> ProviderFuture<'_, Result<ExactInputTokenCount, GatewayError>> {
        match &self.adapter {
            Some(adapter) => adapter.count_exact_tokens(context, request),
            None => Box::pin(async { Err(token_count_unsupported()) }),
        }
    }
}

impl fmt::Debug for TokenCountCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokenCountCapability")
            .field("exact_supported", &self.is_exact())
            .finish()
    }
}

/// Returns the stable error used when no exact count capability is available.
#[must_use]
pub const fn token_count_unsupported() -> GatewayError {
    GatewayError::new(GatewayErrorCode::TokenCountUnsupported, ErrorScope::Model)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gateway_core::{
        CanonicalRequest, ExactInputTokenCount, ProviderId, RawExtensions, RequestContext,
        RequestId,
    };

    use super::{ExactTokenCountAdapter, TokenCountCapability};
    use crate::{ProviderAdapter, ProviderFuture};

    #[derive(Debug)]
    struct FixedExactCounter {
        id: ProviderId,
    }

    impl ProviderAdapter for FixedExactCounter {
        fn provider_id(&self) -> &ProviderId {
            &self.id
        }
    }

    impl ExactTokenCountAdapter for FixedExactCounter {
        fn count_exact_tokens(
            &self,
            _context: RequestContext,
            _request: CanonicalRequest,
        ) -> ProviderFuture<'_, Result<ExactInputTokenCount, gateway_core::GatewayError>> {
            Box::pin(async { Ok(ExactInputTokenCount::new(17)) })
        }
    }

    fn request_context() -> Result<RequestContext, gateway_core::InvalidIdentifier> {
        Ok(RequestContext::new(RequestId::try_new("count-test")?))
    }

    fn request() -> CanonicalRequest {
        CanonicalRequest {
            requested_model: "count-test-model".to_owned(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            extensions: RawExtensions::default(),
        }
    }

    #[tokio::test]
    async fn exact_capability_never_falls_back_to_an_estimate()
    -> Result<(), Box<dyn std::error::Error>> {
        let unsupported = TokenCountCapability::unsupported();
        let result = unsupported
            .count_tokens(request_context()?, request())
            .await;
        let Err(error) = result else {
            return Err(
                std::io::Error::other("unsupported capability must not invent a count").into(),
            );
        };
        assert_eq!(
            error.code(),
            gateway_core::GatewayErrorCode::TokenCountUnsupported
        );

        let capability = TokenCountCapability::exact(Arc::new(FixedExactCounter {
            id: ProviderId::try_new("fixed-counter")?,
        }));
        let result = capability
            .count_tokens(request_context()?, request())
            .await?;
        assert_eq!(result.input_tokens(), 17);
        Ok(())
    }
}
