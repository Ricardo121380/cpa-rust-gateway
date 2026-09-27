//! Kimi metadata is account scoped, bounded, and never authorizes model access.
use super::{
    CredentialId, OpenAiCompatibleRuntimeCredential, Ordering, RuntimeCatalogProvider,
    RuntimeCatalogTarget, RuntimeModelCatalogWorker, system_now_ms_runtime,
};
use gateway_http_actix::management_resources::catalog_refresh::CatalogRefreshError;
use gateway_upstream::{UpstreamHttpMethod, UpstreamHttpRequest};
use provider_openai_compatible::KimiAccountObservation;
use std::time::Duration;

impl RuntimeModelCatalogWorker {
    pub(super) fn is_kimi_metadata_target(target: &RuntimeCatalogTarget) -> bool {
        matches!(&target.provider, RuntimeCatalogProvider::Compatible {url, anthropic: false}
            if url.as_url().as_str() == "https://api.kimi.com/coding/v1/models")
    }
    pub(super) async fn read_kimi_metadata(
        &self,
        credential_id: CredentialId,
    ) -> Result<KimiAccountObservation, CatalogRefreshError> {
        let target = self
            .targets
            .iter()
            .find(|target| {
                Self::is_kimi_metadata_target(target)
                    && self.pools.pool(&target.endpoint_id).is_some_and(|pool| {
                        pool.diagnostic_entries()
                            .iter()
                            .any(|entry| entry.credential_id() == &credential_id)
                    })
            })
            .ok_or(CatalogRefreshError::Unsupported)?;
        let _gate = match &self.generation_guard {
            Some((gate, active)) => {
                let guard = gate.lock().await;
                if !active.load(Ordering::Acquire) {
                    return Err(CatalogRefreshError::Conflict);
                }
                Some(guard)
            }
            None => None,
        };
        let now = system_now_ms_runtime().map_err(|_| CatalogRefreshError::Upstream)?;
        if let Some(cached) = self
            .kimi_metadata
            .lock()
            .map_err(|_| CatalogRefreshError::Upstream)?
            .get(&credential_id)
            .filter(|value| {
                let ttl = if value.profile_available && value.quota_available {
                    300_000
                } else {
                    30_000
                };
                now >= value.observed_at_ms && now - value.observed_at_ms < ttl
            })
            .cloned()
        {
            return Ok(cached);
        }
        let lease = self
            .pools
            .try_lease_exact_eligible_at(&target.endpoint_id, &credential_id, now, |_| true)
            .ok_or(CatalogRefreshError::Busy)?;
        let credential =
            OpenAiCompatibleRuntimeCredential::import_compatible(lease.secret_bytes(), now)
                .map_err(|_| CatalogRefreshError::Upstream)?;
        if credential.kimi_device_id().is_none() {
            return Err(CatalogRefreshError::Unsupported);
        }
        let bearer = credential
            .bearer_at(now)
            .map_err(|_| CatalogRefreshError::Upstream)?;
        let (profile, usages) = tokio::join!(
            self.read_kimi_document(target, bearer, "me"),
            self.read_kimi_document(target, bearer, "usages")
        );
        let completed = system_now_ms_runtime().map_err(|_| CatalogRefreshError::Upstream)?;
        let observation = KimiAccountObservation::from_payloads(
            profile.as_ref().ok(),
            usages.as_ref().ok(),
            completed,
        );
        let mut cache = self
            .kimi_metadata
            .lock()
            .map_err(|_| CatalogRefreshError::Upstream)?;
        cache.insert(credential_id, observation.clone());
        // Publish the revision while holding the same lock as profile readers.
        self.metadata_revision.fetch_add(1, Ordering::AcqRel);
        drop(cache);
        Ok(observation)
    }
    async fn read_kimi_document(
        &self,
        target: &RuntimeCatalogTarget,
        bearer: &str,
        path: &str,
    ) -> Result<serde_json::Value, CatalogRefreshError> {
        tokio::time::timeout(Duration::from_secs(8), async {
            let url = format!("https://api.kimi.com/coding/v1/{path}");
            let admitted = target
                .policy
                .admit_url(&url, target.resolver.as_ref())
                .map_err(|_| CatalogRefreshError::Upstream)?;
            let request = UpstreamHttpRequest::try_new(
                admitted,
                UpstreamHttpMethod::Get,
                vec![
                    ("accept".to_owned(), "application/json".to_owned()),
                    ("authorization".to_owned(), format!("Bearer {bearer}")),
                ],
                Vec::new(),
            )
            .map_err(|_| CatalogRefreshError::Upstream)?;
            let mut response = self
                .client_pool
                .send(request, &target.profile)
                .await
                .map_err(|_| CatalogRefreshError::Upstream)?;
            if !(200..300).contains(&response.status()) {
                return Err(CatalogRefreshError::Upstream);
            }
            let mut bytes = zeroize::Zeroizing::new(Vec::new());
            while let Some(chunk) = response
                .next_chunk()
                .await
                .map_err(|_| CatalogRefreshError::Upstream)?
            {
                if bytes.len().saturating_add(chunk.len()) > 65_536 {
                    return Err(CatalogRefreshError::Upstream);
                }
                bytes.extend_from_slice(&chunk);
            }
            serde_json::from_slice(&bytes).map_err(|_| CatalogRefreshError::Upstream)
        })
        .await
        .map_err(|_| CatalogRefreshError::Upstream)?
    }
}
