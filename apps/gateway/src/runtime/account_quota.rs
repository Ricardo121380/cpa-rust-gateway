//! Read-only, exact-account quota queries for fixed Codex, Claude and Kiro endpoints.
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
    pub(super) async fn read_account_quota(
        &self,
        credential_id: CredentialId,
    ) -> Result<AccountQuotaObservation, CatalogRefreshError> {
        let provider = self
            .metadata_targets
            .iter()
            .chain(self.targets.iter())
            .find_map(|target| {
                if !self.pools.pool(&target.endpoint_id).is_some_and(|p| {
                    p.diagnostic_entries()
                        .iter()
                        .any(|e| e.credential_id() == &credential_id)
                }) {
                    return None;
                }
                match &target.provider {
                    RuntimeCatalogProvider::Codex => Some("codex"),
                    RuntimeCatalogProvider::Kiro(_) => Some("kiro"),
                    RuntimeCatalogProvider::Compatible {
                        url,
                        anthropic: true,
                    } if url.as_url().as_str() == "https://api.anthropic.com/v1/models" => {
                        Some("claude")
                    }
                    _ => None,
                }
            })
            .ok_or(CatalogRefreshError::Unsupported)?;
        let fence = self.quota_fence(&credential_id).await?;
        let result = self.fetch_account_quota(credential_id).await;
        if matches!(result, Err(CatalogRefreshError::Conflict)) {
            return result;
        }
        let samples = match &result {
            Ok(value) => value.history_samples(),
            Err(_) => vec![gateway_store::quota_history::Sample::missing(
                provider,
                system_now_ms_runtime().map_err(|_| CatalogRefreshError::Upstream)?,
                true,
            )],
        };
        self.save_quota_history(fence, samples).await?;
        result
    }
    pub(super) fn quota_revision(&self, id: &CredentialId) -> Option<u64> {
        self.metadata_targets
            .iter()
            .chain(self.targets.iter())
            .find_map(|target| {
                self.pools
                    .pool(&target.endpoint_id)?
                    .diagnostic_entries()
                    .into_iter()
                    .find(|entry| entry.credential_id() == id)
                    .map(|entry| entry.credential_revision())
            })
    }
    pub(super) async fn quota_fence(
        &self,
        id: &CredentialId,
    ) -> Result<gateway_store::quota_history::Fence, CatalogRefreshError> {
        let store = self.quota_history.clone();
        let account = gateway_store::quota_history::AccountScope {
            account_kind: "credential".into(),
            config_version_id: self.config_version_id.clone(),
            account_id: id.to_string(),
        };
        let revision = i64::try_from(
            self.quota_revision(id)
                .ok_or(CatalogRefreshError::Conflict)?,
        )
        .map_err(|_| CatalogRefreshError::Conflict)?;
        history_job(&self.quota_history_slots, move || {
            store.capture(account, Some(revision))
        })
        .await
    }
    pub(super) async fn save_quota_history(
        &self,
        fence: gateway_store::quota_history::Fence,
        samples: Vec<gateway_store::quota_history::Sample>,
    ) -> Result<(), CatalogRefreshError> {
        let store = self.quota_history.clone();
        history_job(&self.quota_history_slots, move || {
            store.append(&fence, &samples)
        })
        .await
    }
    #[allow(clippy::too_many_lines)] // Keep the generation guard and exact credential lease in one request scope.
    async fn fetch_account_quota(
        &self,
        credential_id: CredentialId,
    ) -> Result<AccountQuotaObservation, CatalogRefreshError> {
        let target = self.metadata_targets.iter().chain(self.targets.iter()).find(|target| {
            let supported = matches!(&target.provider, RuntimeCatalogProvider::Codex | RuntimeCatalogProvider::Kiro(_))
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
        let revision = self
            .quota_revision(&credential_id)
            .ok_or(CatalogRefreshError::Conflict)?;
        if let Some(cached) = self
            .account_quotas
            .lock()
            .map_err(|_| CatalogRefreshError::Upstream)?
            .get(&credential_id)
            .filter(|(r, o)| {
                *r == revision && now >= o.observed_at_ms && now - o.observed_at_ms < 300_000
            })
            .map(|(_, o)| o.clone())
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
            (
                "codex",
                "https://chatgpt.com/backend-api/wham/usage".to_owned(),
            )
        } else if let RuntimeCatalogProvider::Kiro(policy) = &target.provider {
            let credential = provider_kiro::credential::KiroCredential::import_runtime_secret(
                lease.secret_bytes(),
                now,
            )
            .map_err(|_| CatalogRefreshError::Upstream)?;
            let (url, request_headers) =
                provider_kiro::account_usage::usage_request(policy, &credential, now)
                    .map_err(|_| CatalogRefreshError::Unsupported)?;
            headers = request_headers;
            ("kiro", url)
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
            (
                "claude",
                "https://api.anthropic.com/api/oauth/usage".to_owned(),
            )
        };
        let admitted = target
            .policy
            .admit_url(&url, target.resolver.as_ref())
            .map_err(|_| CatalogRefreshError::EgressDenied)?;
        let request =
            UpstreamHttpRequest::try_new(admitted, UpstreamHttpMethod::Get, headers, Vec::new())
                .map_err(|_| CatalogRefreshError::Upstream)?;
        let payload = tokio::time::timeout(std::time::Duration::from_secs(8), async {
            let mut response = self
                .client_pool
                .send(request, &target.profile)
                .await
                .map_err(|_| CatalogRefreshError::Upstream)?;
            match response.status() {
                200..=299 => {}
                401 => return Err(CatalogRefreshError::Unauthorized),
                403 => return Err(CatalogRefreshError::Forbidden),
                429 => return Err(CatalogRefreshError::Busy),
                _ => return Err(CatalogRefreshError::Upstream),
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
                .map_err(|_| CatalogRefreshError::InvalidResponse)
        })
        .await
        .map_err(|_| CatalogRefreshError::Upstream)??;
        let observed_at = system_now_ms_runtime().map_err(|_| CatalogRefreshError::Upstream)?;
        let observation = AccountQuotaObservation::parse(source, &payload, observed_at);
        if observation.windows.is_empty()
            && observation.email.is_none()
            && observation.plan.is_none()
        {
            return Err(CatalogRefreshError::InvalidResponse);
        }
        let mut cache = self
            .account_quotas
            .lock()
            .map_err(|_| CatalogRefreshError::Upstream)?;
        cache.insert(
            credential_id,
            (lease.credential_revision(), observation.clone()),
        );
        self.metadata_revision.fetch_add(1, Ordering::AcqRel);
        drop(cache);
        Ok(observation)
    }
}

fn history_error(error: gateway_store::quota_history::Error) -> CatalogRefreshError {
    match error {
        gateway_store::quota_history::Error::Conflict => CatalogRefreshError::Conflict,
        _ => CatalogRefreshError::Upstream,
    }
}

// Both ordinary and Kimi observations use these jobs. The permit follows the blocking work,
// including SQLite/mutex waits, rather than the cancellable management or worker future.
async fn history_job<T, F>(
    slots: &std::sync::Arc<tokio::sync::Semaphore>,
    job: F,
) -> Result<T, CatalogRefreshError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, gateway_store::quota_history::Error> + Send + 'static,
{
    let permit = slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| CatalogRefreshError::Busy)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        job()
    })
    .await
    .map_err(|_| CatalogRefreshError::Upstream)?
    .map_err(history_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn quota_history_cancelled_waiter_keeps_blocking_capacity()
    -> Result<(), Box<dyn std::error::Error>> {
        let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let (started, start) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let worker_slots = slots.clone();
        let waiting = tokio::spawn(async move {
            history_job(&worker_slots, move || {
                let _ = started.send(());
                held.recv()
                    .map_err(|_| gateway_store::quota_history::Error::Unavailable)
            })
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), start).await??;
        waiting.abort();
        assert!(waiting.await.is_err());
        assert!(matches!(
            history_job(&slots, || Ok(())).await,
            Err(CatalogRefreshError::Busy)
        ));
        release.send(())?;
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while slots.available_permits() != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        history_job(&slots, || Ok(()))
            .await
            .map_err(|_| "capacity was not restored")?;
        Ok(())
    }
}
