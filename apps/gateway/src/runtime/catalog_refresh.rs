//! Metadata-only reads share the serving generation's credential and egress boundaries.
use super::{
    BTreeMap, BTreeSet, CredentialId, CredentialLease, EndpointId, ErrorScope, GatewayError,
    GatewayErrorCode, KIMI_OAUTH_DEVICE_MODEL, KIMI_OAUTH_DEVICE_NAME, KIMI_OAUTH_PLATFORM,
    KIMI_OAUTH_USER_AGENT, ModelCatalogTarget, OpenAiCompatibleRuntimeCredential, Ordering,
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

    #[allow(clippy::too_many_lines)]
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
            if let Some(device_id) = credential.kimi_device_id() {
                if device_id.trim().is_empty() {
                    return Err(invalid());
                }
                headers.push(("user-agent".to_owned(), KIMI_OAUTH_USER_AGENT.to_owned()));
                headers.push(("x-msh-platform".to_owned(), KIMI_OAUTH_PLATFORM.to_owned()));
                headers.push((
                    "x-msh-device-name".to_owned(),
                    KIMI_OAUTH_DEVICE_NAME.to_owned(),
                ));
                headers.push((
                    "x-msh-device-model".to_owned(),
                    KIMI_OAUTH_DEVICE_MODEL.to_owned(),
                ));
                headers.push(("x-msh-device-id".to_owned(), device_id.to_owned()));
            }
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

impl RuntimeModelCatalogWorker {
    pub(super) async fn discover_kiro(
        &self,
        target: &RuntimeCatalogTarget,
        policy: &provider_kiro::endpoint_policy::KiroEndpointPolicy,
        lease: &CredentialLease,
        now: i64,
    ) -> Result<Vec<gateway_catalog::DiscoveredModel>, GatewayError> {
        let invalid = || {
            GatewayError::new(
                GatewayErrorCode::UpstreamProtocolError,
                ErrorScope::Provider,
            )
        };
        let credential = provider_kiro::credential::KiroCredential::import_runtime_secret(
            lease.secret_bytes(),
            now,
        )
        .map_err(|_| {
            GatewayError::new(
                GatewayErrorCode::CredentialUnauthorized,
                ErrorScope::Credential,
            )
        })?;
        kiro_catalog_pages(policy, &credential, now, |url, headers| async move {
            let admitted = target
                .policy
                .admit_url(&url, target.resolver.as_ref())
                .map_err(gateway_upstream::EgressAdmissionError::gateway_error)?;
            let request = UpstreamHttpRequest::try_new(
                admitted,
                UpstreamHttpMethod::Get,
                headers,
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
                if bytes.len().saturating_add(chunk.len()) > 1024 * 1024 {
                    return Err(invalid());
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(bytes)
        })
        .await
    }
}

// Keep the production pagination algorithm independent of HTTP transport so malformed pages
// and exact credential headers can be tested without relaxing Kiro's fixed HTTPS destination.
async fn kiro_catalog_pages<F, Fut>(
    policy: &provider_kiro::endpoint_policy::KiroEndpointPolicy,
    credential: &provider_kiro::credential::KiroCredential,
    now: i64,
    mut fetch: F,
) -> Result<Vec<gateway_catalog::DiscoveredModel>, GatewayError>
where
    F: FnMut(String, Vec<(String, String)>) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>, GatewayError>>,
{
    let invalid = || {
        GatewayError::new(
            GatewayErrorCode::UpstreamProtocolError,
            ErrorScope::Provider,
        )
    };
    let mut next = None;
    let mut seen = BTreeSet::new();
    let mut models = BTreeMap::new();
    for _ in 0..100 {
        let (url, headers) =
            provider_kiro::catalog::catalog_request(policy, credential, now, next.as_deref())?;
        let bytes = fetch(url, headers).await?;
        let page = provider_kiro::catalog::parse_catalog_page(&bytes)?;
        for model in page.models {
            models.insert(model.upstream_model().to_owned(), model);
        }
        if models.len() > 10_000 {
            return Err(invalid());
        }
        match page.next_token {
            None => return Ok(models.into_values().collect()),
            Some(token) => {
                if !seen.insert(token.clone()) {
                    return Err(invalid());
                }
                next = Some(token);
            }
        }
    }
    Err(invalid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use provider_kiro::{
        credential::KiroCredential,
        endpoint_policy::{KiroApiRegion, KiroEndpointKind, KiroEndpointPolicy},
    };
    use std::{cell::RefCell, collections::VecDeque};
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[actix_web::test]
    async fn kiro_pages_keep_exact_identity_ids_and_cursor_until_complete() -> TestResult {
        let policy = KiroEndpointPolicy::try_new(
            KiroEndpointKind::Ide,
            KiroApiRegion::try_new("eu-central-1")?,
        )?;
        for account in ["first", "second"] {
            let bytes = format!(
                r#"{{"kind":"social","access_token":"{account}","refresh_token":"synthetic","expires_at_ms":2000}}"#
            );
            let credential = KiroCredential::import_json(bytes.as_bytes(), 1000)?;
            let pages = RefCell::new(VecDeque::from([
                br#"{"models":[{"modelId":"Exact/Model-1"}],"nextToken":"page+/2"}"#.to_vec(),
                br#"{"models":[{"modelId":"Exact/Model-1"},{"modelId":"other-v2"}]}"#.to_vec(),
            ]));
            let requests = RefCell::new(Vec::new());
            let models = kiro_catalog_pages(&policy, &credential, 1000, |url, headers| {
                requests.borrow_mut().push((url, headers));
                let result = pages.borrow_mut().pop_front().ok_or_else(|| {
                    GatewayError::new(
                        GatewayErrorCode::UpstreamProtocolError,
                        ErrorScope::Provider,
                    )
                });
                std::future::ready(result)
            })
            .await?;
            assert_eq!(
                models
                    .iter()
                    .map(gateway_catalog::DiscoveredModel::upstream_model)
                    .collect::<Vec<_>>(),
                ["Exact/Model-1", "other-v2"]
            );
            let requests = requests.borrow();
            assert_eq!(requests.len(), 2);
            assert!(!requests[0].0.contains("nextToken"));
            assert!(requests[1].0.contains("nextToken=page%2B%2F2"));
            for (url, headers) in requests.iter() {
                assert!(
                    url.starts_with("https://q.eu-central-1.amazonaws.com/ListAvailableModels?")
                );
                assert!(headers.contains(&("authorization".into(), format!("Bearer {account}"))));
            }
        }
        Ok(())
    }

    #[actix_web::test]
    async fn kiro_partial_or_repeated_pages_never_return_a_successful_partial_catalog() -> TestResult
    {
        let policy = KiroEndpointPolicy::try_new(
            KiroEndpointKind::Ide,
            KiroApiRegion::try_new("us-east-1")?,
        )?;
        let credential = KiroCredential::import_json(br#"{"kind":"social","access_token":"synthetic","refresh_token":"synthetic","expires_at_ms":2000}"#, 1000)?;
        for last in [
            Some(br#"{"models":[],"nextToken":"same"}"#.to_vec()),
            Some(b"{}".to_vec()),
            None,
        ] {
            let pages = RefCell::new(VecDeque::from([
                Some(br#"{"models":[{"modelId":"partial"}],"nextToken":"same"}"#.to_vec()),
                last,
            ]));
            let result = kiro_catalog_pages(&policy, &credential, 1000, |_, _| {
                std::future::ready(pages.borrow_mut().pop_front().flatten().ok_or_else(|| {
                    GatewayError::new(
                        GatewayErrorCode::UpstreamProtocolError,
                        ErrorScope::Provider,
                    )
                }))
            })
            .await;
            assert!(result.is_err());
            assert!(pages.borrow().is_empty());
        }
        Ok(())
    }
}
