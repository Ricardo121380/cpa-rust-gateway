//! Request-lifecycle correlation state shared by later gateway stages.

use crate::{ClientKeyId, RequestId};

/// Immutable context for one externally accepted gateway request.
///
/// The context intentionally contains only request-level identity. Attempt, credential, endpoint,
/// and provider selection remain outside it because they can change before the first semantic
/// event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestContext {
    request_id: RequestId,
    client_key_id: Option<ClientKeyId>,
}

impl RequestContext {
    /// Creates context for an accepted external request.
    #[must_use]
    pub fn new(request_id: RequestId) -> Self {
        Self {
            request_id,
            client_key_id: None,
        }
    }

    /// Returns the correlation identifier retained for the whole request lifecycle.
    #[must_use]
    pub fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Attaches the authenticated Client Key identity without retaining the presented secret.
    #[must_use]
    pub fn with_client_key_id(mut self, client_key_id: ClientKeyId) -> Self {
        self.client_key_id = Some(client_key_id);
        self
    }

    /// Returns the authenticated Client Key identity when the ingress supplied one.
    #[must_use]
    pub fn client_key_id(&self) -> Option<&ClientKeyId> {
        self.client_key_id.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::RequestContext;
    use crate::{ClientKeyId, RequestId};

    #[test]
    fn request_context_retains_the_request_identifier() {
        let result = RequestId::try_new("request-01");

        assert!(result.is_ok());
        if let Ok(request_id) = result {
            let context = RequestContext::new(request_id);
            assert_eq!(context.request_id().as_str(), "request-01");
            assert!(context.client_key_id().is_none());
        }
    }

    #[test]
    fn request_context_retains_only_the_authenticated_client_key_identity() {
        let request_id = RequestId::try_new("request-02");
        let client_key_id = ClientKeyId::try_new("client-key-02");

        assert!(request_id.is_ok());
        assert!(client_key_id.is_ok());
        if let (Ok(request_id), Ok(client_key_id)) = (request_id, client_key_id) {
            let context = RequestContext::new(request_id).with_client_key_id(client_key_id);
            assert_eq!(
                context.client_key_id().map(ClientKeyId::as_str),
                Some("client-key-02")
            );
        }
    }
}
