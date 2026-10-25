//! Ordinary Build cache affinity at the existing candidate/lease boundary.
//! Exact continuation and diagnostic pins never attach this store to their driver.

use super::{
    CredentialLease, EndpointAdapter, EndpointAttemptDriver, GatewayError, SnapshotRouteCandidate,
    internal_error, system_now_ms,
};
use gateway_core::EgressPolicyId;
use provider_grok::{
    GrokBuildAffinityBreakInput, GrokBuildAffinityBreakReason, GrokBuildAffinityReason,
    GrokBuildCacheAffinity, GrokBuildCacheAffinityKey,
};
use sha2::{Digest, Sha256};

const AFFINITY_TTL_MS: i64 = 15 * 60 * 1000;

impl EndpointAttemptDriver {
    fn build_affinity_key(
        &self,
        candidate: &SnapshotRouteCandidate,
    ) -> Result<Option<GrokBuildCacheAffinityKey>, GatewayError> {
        if self.build_affinity.is_none()
            || self.channel_pin_observation.is_some()
            || !self
                .endpoints
                .get(candidate.endpoint_id())
                .is_some_and(|runtime| {
                    matches!(runtime.adapter, EndpointAdapter::GrokBuildResponses)
                })
        {
            return Ok(None);
        }
        let (Some(client), Some(cache_key)) = (
            &self.client_key_id,
            self.request.prompt_cache_key.as_deref(),
        ) else {
            return Ok(None);
        };
        let identity = self
            .grok_build_cache_identity_deriver
            .derive(client, candidate.upstream_model(), cache_key)
            .map_err(|_| internal_error())?;
        GrokBuildCacheAffinityKey::try_new(client.clone(), candidate.upstream_model(), identity)
            .map(Some)
            .map_err(|_| internal_error())
    }

    fn build_binding_identity(
        &self,
        candidate: &SnapshotRouteCandidate,
        revision: u64,
    ) -> Result<(EgressPolicyId, String), GatewayError> {
        let runtime = self
            .endpoints
            .get(candidate.endpoint_id())
            .ok_or_else(internal_error)?;
        let transport = runtime.transports.for_mode(self.mode);
        let egress = transport.proxy().canonical_url().unwrap_or("direct");
        let egress = format!("build-egress:v1:{:x}", Sha256::digest(egress.as_bytes()));
        let (version, config_revision) = self
            .execution_configuration
            .as_ref()
            .ok_or_else(internal_error)?;
        let mut digest = Sha256::new();
        for field in [
            "build-affinity:v1",
            version.as_str(),
            &config_revision.as_i64().to_string(),
            candidate.id().as_str(),
            candidate.endpoint_id().as_str(),
            candidate.upstream_id().as_str(),
            self.admitted_public_model
                .as_deref()
                .ok_or_else(internal_error)?,
            candidate.upstream_model(),
            &revision.to_string(),
            &egress,
        ] {
            digest.update((field.len() as u64).to_be_bytes());
            digest.update(field.as_bytes());
        }
        Ok((
            EgressPolicyId::try_new(egress).map_err(|_| internal_error())?,
            format!("{:x}", digest.finalize()),
        ))
    }

    pub(super) async fn prepare_build_affinity(
        &mut self,
        route: &gateway_router::SnapshotRoute,
    ) -> Result<(), GatewayError> {
        let Some(store) = self.build_affinity.clone() else {
            return Ok(());
        };
        let mut keys = Vec::new();
        for candidate in route.candidates() {
            if let Some(key) = self.build_affinity_key(candidate)? {
                keys.push((candidate.clone(), key));
            }
        }
        if keys.is_empty() {
            return Ok(());
        }
        let now = system_now_ms().map_err(|_| internal_error())?;
        let bindings = tokio::task::spawn_blocking(move || {
            keys.into_iter()
                .map(|(candidate, key)| {
                    store
                        .cache_affinity(&key, now)
                        .map(|binding| (candidate, binding))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|_| internal_error())?
        .map_err(|_| internal_error())?;
        for (candidate, binding) in bindings {
            let Some(binding) = binding else {
                continue;
            };
            let Some(revision) = binding.credential_revision() else {
                continue;
            };
            let (egress, fingerprint) = self.build_binding_identity(&candidate, revision)?;
            if binding.egress_policy_id() == Some(&egress)
                && binding.binding_fingerprint() == Some(fingerprint.as_str())
            {
                self.build_preferences.insert(
                    candidate.id().clone(),
                    (binding.credential_id().clone(), revision),
                );
            }
        }
        Ok(())
    }

    pub(super) async fn bind_build_affinity(
        &self,
        candidate: &SnapshotRouteCandidate,
        credential: &CredentialLease,
    ) -> Result<(), GatewayError> {
        let Some(key) = self.build_affinity_key(candidate)? else {
            return Ok(());
        };
        let store = self.build_affinity.clone().ok_or_else(internal_error)?;
        let now = system_now_ms().map_err(|_| internal_error())?;
        let (egress, fingerprint) =
            self.build_binding_identity(candidate, credential.credential_revision())?;
        let binding = GrokBuildCacheAffinity::try_new(
            credential.credential_id().clone(),
            Some(egress),
            now.checked_add(AFFINITY_TTL_MS)
                .ok_or_else(internal_error)?,
            GrokBuildAffinityReason::PromptCache,
        )
        .and_then(|binding| {
            binding.with_serving_binding(credential.credential_revision(), fingerprint)
        })
        .map_err(|_| internal_error())?;
        // Each successful bootstrap atomically refreshes or replaces the then-current binding.
        // Concurrent misses may select independently; the last successful bootstrap owns the next
        // preference, with a durable break for every actual change and no fabricated token loss.
        tokio::task::spawn_blocking(move || {
            store.bind_cache_affinity(
                &key,
                &binding,
                now,
                Some(GrokBuildAffinityBreakInput::unknown(
                    GrokBuildAffinityBreakReason::PolicyReselection,
                    now,
                )),
            )
        })
        .await
        .map_err(|_| internal_error())?
        .map_err(|_| internal_error())?;
        Ok(())
    }
}
