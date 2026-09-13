//! Metadata-only reads share the serving generation's credential and egress boundaries.
use super::{
    BTreeMap, BTreeSet, CredentialId, CredentialLease, EndpointId, ErrorScope, GatewayError,
    GatewayErrorCode, ModelCatalogTarget, OpenAiCompatibleRuntimeCredential, Ordering,
    RuntimeCatalogTarget, RuntimeModelCatalogWorker, catalog_failure_class, system_now_ms_runtime,
};
use gateway_http_actix::management_resources::catalog_refresh::{
    CatalogRefreshError, CatalogRefreshReceipt,
};
use gateway_upstream::{EndpointUrl, UpstreamHttpMethod, UpstreamHttpRequest};

impl RuntimeModelCatalogWorker {
    pub(super) async fn refresh_target(
        &self,
        endpoint: EndpointId,
        credential: CredentialId,
    ) -> Result<CatalogRefreshReceipt, CatalogRefreshError> {
        let target = self
            .targets
            .iter()
            .find(|t| t.endpoint_id == endpoint)
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
        let catalog = ModelCatalogTarget::new(endpoint.clone(), credential.clone());
        let lease = self
            .pools
            .try_lease_exact_eligible_at(&endpoint, &credential, now, |_| true)
            .ok_or(CatalogRefreshError::Busy)?;
        let result = self.discover(target, catalog.clone(), &lease, now).await;
        drop(lease);
        let completed = system_now_ms_runtime().map_err(|_| CatalogRefreshError::Upstream)?;
        let worker = self.clone();
        let count = actix_web::web::block(move || match result {
            Ok(models) => {
                let count = models.len();
                worker
                    .store
                    .record_success(&worker.config_version_id, &catalog, models, completed)
                    .map_err(|_| CatalogRefreshError::Upstream)?;
                worker
                    .publish_durable(completed)
                    .map_err(|_| CatalogRefreshError::Conflict)?;
                Ok(count)
            }
            Err(error) => {
                worker
                    .store
                    .record_failure(
                        &worker.config_version_id,
                        &catalog,
                        completed,
                        catalog_failure_class(error.code()),
                    )
                    .map_err(|_| CatalogRefreshError::Upstream)?;
                Err(CatalogRefreshError::Upstream)
            }
        })
        .await
        .map_err(|_| CatalogRefreshError::Upstream)??;
        Ok(CatalogRefreshReceipt {
            config_version: self.config_version_id.clone(),
            endpoint_id: endpoint.to_string(),
            credential_id: credential.to_string(),
            observed_at_ms: completed,
            model_count: count,
        })
    }

    pub(super) async fn discover_compatible(
        &self,
        target: &RuntimeCatalogTarget,
        url: &EndpointUrl,
        anthropic: bool,
        lease: &CredentialLease,
        now: i64,
    ) -> Result<Vec<gateway_catalog::DiscoveredModel>, GatewayError> {
        let invalid = || {
            GatewayError::new(
                GatewayErrorCode::UpstreamProtocolError,
                ErrorScope::Provider,
            )
        };
        let mut headers = vec![("accept".to_owned(), "application/json".to_owned())];
        if anthropic {
            let credential = provider_anthropic_compatible::ClaudeRuntimeCredential::import_at(
                lease.secret_bytes(),
                now,
            )
            .map_err(|_| invalid())?;
            let auth = credential.authorization_at(now).map_err(|_| invalid())?;
            headers.push((
                auth.header_name().to_owned(),
                auth.header_value().to_owned(),
            ));
            headers.push(("anthropic-version".to_owned(), "2023-06-01".to_owned()));
        } else {
            let credential =
                OpenAiCompatibleRuntimeCredential::import_compatible(lease.secret_bytes(), now)
                    .map_err(|_| invalid())?;
            headers.push((
                "authorization".to_owned(),
                format!(
                    "Bearer {}",
                    credential.bearer_at(now).map_err(|_| invalid())?
                ),
            ));
        }
        let mut models = BTreeMap::new();
        let mut after: Option<String> = None;
        let mut seen = BTreeSet::new();
        for _ in 0..100 {
            let mut page_url = url.as_url().clone();
            if let Some(after) = &after {
                page_url.query_pairs_mut().append_pair("after_id", after);
            }
            let admitted = target
                .policy
                .admit_url(page_url.as_str(), target.resolver.as_ref())
                .map_err(gateway_upstream::EgressAdmissionError::gateway_error)?;
            let request = UpstreamHttpRequest::try_new(
                admitted,
                UpstreamHttpMethod::Get,
                headers.clone(),
                Vec::new(),
            )
            .map_err(|_| invalid())?;
            let mut response = self.client_pool.send(request, &target.profile).await?;
            match response.status() {
                200..=299 => {}
                401 => {
                    return Err(GatewayError::new(
                        GatewayErrorCode::CredentialUnauthorized,
                        ErrorScope::Credential,
                    ));
                }
                403 => {
                    return Err(GatewayError::new(
                        GatewayErrorCode::CredentialForbidden,
                        ErrorScope::Credential,
                    ));
                }
                429 => {
                    return Err(GatewayError::new(
                        GatewayErrorCode::ProviderRateLimited,
                        ErrorScope::Provider,
                    ));
                }
                _ => return Err(invalid()),
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.next_chunk().await? {
                if bytes.len().saturating_add(chunk.len()) > 2 * 1024 * 1024 {
                    return Err(invalid());
                }
                bytes.extend_from_slice(&chunk);
            }
            let page = provider_openai_compatible::parse_compatible_catalog(&bytes)?;
            for model in page.models {
                models.insert(model.upstream_model().to_owned(), model);
            }
            if models.len() > 10_000 {
                return Err(invalid());
            }
            match page.after {
                None => return Ok(models.into_values().collect()),
                Some(next) => {
                    if !seen.insert(next.clone()) {
                        return Err(invalid());
                    }
                    after = Some(next);
                }
            }
        }
        Err(invalid())
    }
}
