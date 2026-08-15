//! Secret-free successful-attempt lineage captured for opt-in stored Responses.

use std::{fmt, sync::Mutex};

use gateway_core::{
    CredentialId, EndpointId, ErrorScope, GatewayError, GatewayErrorCode, ProviderId,
    RouteCandidateId, RouteId, UpstreamId,
};

use crate::SnapshotVersion;

/// Exact immutable routing and Credential identity of the Attempt whose source was returned.
#[derive(Clone, Eq, PartialEq)]
pub struct ResponsesExecutionLineage {
    snapshot_version: SnapshotVersion,
    provider_id: ProviderId,
    upstream_id: UpstreamId,
    channel_id: EndpointId,
    route_id: RouteId,
    route_candidate_id: RouteCandidateId,
    credential_id: CredentialId,
    credential_revision: u64,
}

impl ResponsesExecutionLineage {
    /// Creates one lineage value from the already-selected serving binding.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        snapshot_version: SnapshotVersion,
        provider_id: ProviderId,
        upstream_id: UpstreamId,
        channel_id: EndpointId,
        route_id: RouteId,
        route_candidate_id: RouteCandidateId,
        credential_id: CredentialId,
        credential_revision: u64,
    ) -> Self {
        Self {
            snapshot_version,
            provider_id,
            upstream_id,
            channel_id,
            route_id,
            route_candidate_id,
            credential_id,
            credential_revision,
        }
    }

    /// Returns the exact Config Version pinned by the runtime executor.
    #[must_use]
    pub const fn snapshot_version(&self) -> &SnapshotVersion {
        &self.snapshot_version
    }

    /// Returns the exact owning Provider identity.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns the configured Upstream identity.
    #[must_use]
    pub const fn upstream_id(&self) -> &UpstreamId {
        &self.upstream_id
    }

    /// Returns the selected Endpoint/Channel identity.
    #[must_use]
    pub const fn channel_id(&self) -> &EndpointId {
        &self.channel_id
    }

    /// Returns the selected public-model Route identity.
    #[must_use]
    pub const fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    /// Returns the exact Route Candidate identity.
    #[must_use]
    pub const fn route_candidate_id(&self) -> &RouteCandidateId {
        &self.route_candidate_id
    }

    /// Returns the selected Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the persistent Credential revision held by the successful lease.
    #[must_use]
    pub const fn credential_revision(&self) -> u64 {
        self.credential_revision
    }
}

impl fmt::Debug for ResponsesExecutionLineage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponsesExecutionLineage")
            .field("snapshot_version", &"<redacted>")
            .field("provider_id", &"<redacted>")
            .field("upstream_id", &"<redacted>")
            .field("channel_id", &"<redacted>")
            .field("route_id", &"<redacted>")
            .field("route_candidate_id", &"<redacted>")
            .field("credential_id", &"<redacted>")
            .field("credential_revision", &self.credential_revision)
            .finish()
    }
}

/// Request-local single-assignment recorder shared by HTTP and the serving executor.
pub struct ResponsesExecutionLineageRecorder {
    lineage: Mutex<Option<ResponsesExecutionLineage>>,
}

impl ResponsesExecutionLineageRecorder {
    /// Creates an empty request-local recorder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lineage: Mutex::new(None),
        }
    }

    /// Records the successful serving binding exactly once.
    ///
    /// Identical replay is harmless, while a conflicting second binding or poisoned lock fails
    /// closed before a stored Response can be written under ambiguous lineage.
    ///
    /// # Errors
    ///
    /// Returns a safe internal error for conflicting or unavailable recorder state.
    pub fn record(&self, lineage: ResponsesExecutionLineage) -> Result<(), GatewayError> {
        let mut retained = self.lineage.lock().map_err(|_| internal_error())?;
        match retained.as_ref() {
            None => *retained = Some(lineage),
            Some(existing) if existing == &lineage => {}
            Some(_) => return Err(internal_error()),
        }
        Ok(())
    }

    /// Returns the recorded successful binding without exposing a mutable state handle.
    ///
    /// # Errors
    ///
    /// Returns a safe internal error if recorder state is unavailable.
    pub fn lineage(&self) -> Result<Option<ResponsesExecutionLineage>, GatewayError> {
        self.lineage
            .lock()
            .map(|lineage| lineage.clone())
            .map_err(|_| internal_error())
    }
}

impl Default for ResponsesExecutionLineageRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ResponsesExecutionLineageRecorder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let populated = self.lineage.lock().is_ok_and(|lineage| lineage.is_some());
        formatter
            .debug_struct("ResponsesExecutionLineageRecorder")
            .field("populated", &populated)
            .finish()
    }
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use gateway_core::{
        CredentialId, EndpointId, ProviderId, RouteCandidateId, RouteId, UpstreamId,
    };

    use super::{ResponsesExecutionLineage, ResponsesExecutionLineageRecorder};
    use crate::SnapshotVersion;

    #[test]
    fn recorder_is_single_assignment_and_debug_is_value_free()
    -> Result<(), Box<dyn std::error::Error>> {
        let recorder = ResponsesExecutionLineageRecorder::new();
        let lineage = fixture("credential-a", 7)?;
        recorder.record(lineage.clone())?;
        recorder.record(lineage.clone())?;
        assert_eq!(recorder.lineage()?, Some(lineage));

        let conflicting = fixture("credential-b", 8)?;
        assert!(recorder.record(conflicting).is_err());
        let debug = format!("{recorder:?}");
        assert!(debug.contains("populated"));
        assert!(!debug.contains("credential-a"));
        Ok(())
    }

    fn fixture(
        credential_id: &str,
        revision: u64,
    ) -> Result<ResponsesExecutionLineage, gateway_core::InvalidIdentifier> {
        Ok(ResponsesExecutionLineage::new(
            SnapshotVersion::try_new("config-v1")?,
            ProviderId::try_new("provider-a")?,
            UpstreamId::try_new("provider-a")?,
            EndpointId::try_new("channel-a")?,
            RouteId::try_new("route-a")?,
            RouteCandidateId::try_new("candidate-a")?,
            CredentialId::try_new(credential_id)?,
            revision,
        ))
    }
}
