//! Request-lifecycle correlation state shared by later gateway stages.

use crate::RequestId;

/// Immutable context for one externally accepted gateway request.
///
/// The context intentionally contains only request-level identity. Attempt, credential, endpoint,
/// and provider selection remain outside it because they can change before the first semantic
/// event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestContext {
    request_id: RequestId,
}

impl RequestContext {
    /// Creates context for an accepted external request.
    #[must_use]
    pub fn new(request_id: RequestId) -> Self {
        Self { request_id }
    }

    /// Returns the correlation identifier retained for the whole request lifecycle.
    #[must_use]
    pub fn request_id(&self) -> &RequestId {
        &self.request_id
    }
}

#[cfg(test)]
mod tests {
    use super::RequestContext;
    use crate::RequestId;

    #[test]
    fn request_context_retains_the_request_identifier() {
        let result = RequestId::try_new("request-01");

        assert!(result.is_ok());
        if let Ok(request_id) = result {
            let context = RequestContext::new(request_id);
            assert_eq!(context.request_id().as_str(), "request-01");
        }
    }
}
