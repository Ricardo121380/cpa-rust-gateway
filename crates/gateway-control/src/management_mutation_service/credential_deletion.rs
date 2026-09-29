//! Read-only local deletion impact over one exact configuration graph.
use super::{ConfigVersionId, CredentialId, ManagementMutationService, ManagementResourceError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
impl ManagementMutationService {
    /// Previews removed bindings and shared resources retained by a local credential deletion.
    /// # Errors
    /// Returns a safe missing-resource or storage error; never reads plaintext or contacts a Provider.
    pub fn credential_deletion_impact(
        &mut self,
        version: &ConfigVersionId,
        id: &CredentialId,
    ) -> Result<Value, ManagementResourceError> {
        let graph = self.configuration(version)?;
        let credential = graph
            .credentials
            .iter()
            .find(|credential| credential.id == *id)
            .ok_or(ManagementResourceError::ResourceNotFound)?;
        let bindings = graph
            .endpoint_credential_bindings
            .iter()
            .filter(|binding| binding.credential_id == *id)
            .collect::<Vec<_>>();
        let endpoints=graph.endpoints.iter().filter(|endpoint|bindings.iter().any(|binding|binding.endpoint_id==endpoint.id)).map(|endpoint|json!({"id":endpoint.id.as_str(),"enabled":endpoint.enabled,"remaining_credentials":graph.endpoint_credential_bindings.iter().filter(|binding|binding.endpoint_id==endpoint.id&&binding.credential_id!=*id).map(|binding|binding.credential_id.as_str()).collect::<Vec<_>>()})).collect::<Vec<_>>();
        let routes = graph
            .model_routes
            .iter()
            .filter(|route| {
                graph.route_candidates.iter().any(|candidate| {
                    candidate.route_id == route.id
                        && bindings
                            .iter()
                            .any(|binding| binding.endpoint_id == candidate.endpoint_id)
                })
            })
            .map(|route| route.id.as_str())
            .collect::<Vec<_>>();
        let groups = graph
            .access_group_routes
            .iter()
            .filter(|grant| routes.contains(&grant.route_id.as_str()))
            .map(|grant| grant.access_group_id.as_str())
            .collect::<Vec<_>>();
        let mut impact = json!({"config_version":version.as_str(),"revision":format!("rev-{}",graph.version.revision),"credential_id":id.as_str(),"upstream_id":credential.upstream_id.as_str(),"credential_revision":credential.revision,"removed_bindings":bindings.iter().map(|binding|json!({"endpoint_id":binding.endpoint_id.as_str(),"enabled":binding.enabled})).collect::<Vec<_>>(),"removed_egress_profiles":graph.compatible_egress_bindings.iter().filter(|binding|binding.credential_id==*id).count(),"retained_endpoints":endpoints,"retained_routes":routes,"retained_client_keys":graph.client_keys.iter().filter(|key|groups.contains(&key.access_group_id().as_str())).map(|key|key.id().as_str()).collect::<Vec<_>>(),"history_retained":true,"upstream_account_revoked":false});
        let bytes =
            serde_json::to_vec(&impact).map_err(|_| ManagementResourceError::InvalidRevision)?;
        impact["review_token"] = json!(format!("{:x}", Sha256::digest(bytes)));
        Ok(impact)
    }
}
