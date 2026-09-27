//! Read-only, exact-account quota queries for verified Codex and Claude endpoints.
use super::{
    CredentialId, OpenAiCompatibleRuntimeCredential, Ordering, RuntimeCatalogProvider,
    RuntimeModelCatalogWorker, system_now_ms_runtime,
};
use gateway_http_actix::management_resources::{
    account_quota::AccountQuotaObservation, catalog_refresh::CatalogRefreshError,
};
use gateway_upstream::{UpstreamHttpMethod, UpstreamHttpRequest};
use provider_anthropic_compatible::ClaudeRuntimeCredential;

impl RuntimeModelCatalogWorker {
    #[allow(clippy::too_many_lines)] // Keep the generation guard and exact credential lease in one request scope.
    pub(super) async fn read_account_quota(
        &self,
        credential_id: CredentialId,
    ) -> Result<AccountQuotaObservation, CatalogRefreshError> {
        let target = self.targets.iter().find(|target| {
            let supported = matches!(&target.provider, RuntimeCatalogProvider::Codex)
                || matches!(&target.provider, RuntimeCatalogProvider::Compatible {url, anthropic:true} if url.as_url().as_str() == "https://api.anthropic.com/v1/models");
            supported && self.pools.pool(&target.endpoint_id).is_some_and(|pool|pool.diagnostic_entries().iter().any(|entry| entry.credential_id() == &credential_id))
        }).ok_or(CatalogRefreshError::Unsupported)?;
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
            .account_quotas
            .lock()
            .map_err(|_| CatalogRefreshError::Upstream)?
            .get(&credential_id)
            .filter(|o| now >= o.observed_at_ms && now - o.observed_at_ms < 300_000)
            .cloned()
        {
            return Ok(cached);
        }
        let lease = self
            .pools
            .try_lease_exact_eligible_at(&target.endpoint_id, &credential_id, now, |_| true)
            .ok_or(CatalogRefreshError::Busy)?;
        let mut headers = vec![("accept".to_owned(), "application/json".to_owned())];
        let (source, url) = if matches!(target.provider, RuntimeCatalogProvider::Codex) {
            let credential =
                OpenAiCompatibleRuntimeCredential::import_compatible(lease.secret_bytes(), now)
                    .map_err(|_| CatalogRefreshError::Upstream)?;
            let account = credential
                .account_id()
                .ok_or(CatalogRefreshError::Unsupported)?;
            headers.push((
                "authorization".into(),
                format!(
                    "Bearer {}",
                    credential
                        .bearer_at(now)
                        .map_err(|_| CatalogRefreshError::Upstream)?
                ),
            ));
            headers.push(("chatgpt-account-id".into(), account.to_owned()));
            headers.push((
                "user-agent".into(),
                provider_openai_compatible::CODEX_USER_AGENT.into(),
            ));
            ("codex", "https://chatgpt.com/backend-api/wham/usage")
        } else {
            let credential = ClaudeRuntimeCredential::import_at(lease.secret_bytes(), now)
                .map_err(|_| CatalogRefreshError::Upstream)?;
            if !matches!(credential, ClaudeRuntimeCredential::OAuth(_)) {
                return Err(CatalogRefreshError::Unsupported);
            }
            let auth = credential
                .authorization_at(now)
                .map_err(|_| CatalogRefreshError::Upstream)?;
            headers.push((
                auth.header_name().to_owned(),
                auth.header_value().to_owned(),
            ));
            headers.push(("anthropic-beta".into(), "oauth-2025-04-20".into()));
            ("claude", "https://api.anthropic.com/api/oauth/usage")
        };
        let admitted = target
            .policy
            .admit_url(url, target.resolver.as_ref())
            .map_err(|_| CatalogRefreshError::Upstream)?;
        let request =
            UpstreamHttpRequest::try_new(admitted, UpstreamHttpMethod::Get, headers, Vec::new())
                .map_err(|_| CatalogRefreshError::Upstream)?;
        let payload = tokio::time::timeout(std::time::Duration::from_secs(8), async {
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
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .map_err(|_| CatalogRefreshError::Upstream)
        })
        .await
        .map_err(|_| CatalogRefreshError::Upstream)??;
        let observed_at = system_now_ms_runtime().map_err(|_| CatalogRefreshError::Upstream)?;
        let observation = AccountQuotaObservation::parse(source, &payload, observed_at);
        if observation.windows.is_empty() {
            return Err(CatalogRefreshError::Upstream);
        }
        self.account_quotas
            .lock()
            .map_err(|_| CatalogRefreshError::Upstream)?
            .insert(credential_id, observation.clone());
        Ok(observation)
    }
}
