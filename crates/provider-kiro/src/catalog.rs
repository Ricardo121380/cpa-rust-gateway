//! Bounded metadata operations for the existing Kiro IDE OAuth channel.
//! No guessed model IDs, profile ARN, region fallback, or inference calls.
use crate::{
    credential::{KiroCredential, KiroCredentialKind},
    endpoint_policy::{KiroEndpointKind, KiroEndpointPolicy},
};
use gateway_catalog::DiscoveredModel;
use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};
use serde::Deserialize;

/// One complete metadata page. The caller must finish pagination before publishing it.
pub struct KiroCatalogPage {
    /// Exact upstream identifiers.
    pub models: Vec<DiscoveredModel>,
    /// Opaque bounded continuation, never rendered or logged.
    pub next_token: Option<String>,
}

/// Fixed metadata destination for an IDE OAuth credential. CLI/API-key metadata lacks a
/// verified common contract and must not be silently routed through the IDE service.
/// # Errors
/// Rejects unsupported families, expired credentials and malformed cursors.
pub fn catalog_request(
    policy: &KiroEndpointPolicy,
    credential: &KiroCredential,
    now: i64,
    next: Option<&str>,
) -> Result<(String, Vec<(String, String)>), GatewayError> {
    if policy.kind() != KiroEndpointKind::Ide
        || credential.kind() == KiroCredentialKind::ApiKey
        || credential.is_expired_at(now)
    {
        return Err(invalid());
    }
    let mut url = policy.url().clone();
    url.set_path("/ListAvailableModels");
    url.set_query(None);
    url.query_pairs_mut()
        .append_pair("origin", "AI_EDITOR")
        .append_pair("maxResults", "50");
    if let Some(next) = next {
        validate_cursor(next)?;
        url.query_pairs_mut().append_pair("nextToken", next);
    }
    let token = credential.access_token().map_err(|_| invalid())?;
    Ok((
        url.to_string(),
        vec![
            ("accept".into(), "application/json".into()),
            ("authorization".into(), format!("Bearer {token}")),
        ],
    ))
}

/// Decodes only exact model identifiers and pagination. A partial/malformed page is not success.
/// # Errors
/// Rejects oversized pages, invalid IDs and malformed pagination.
pub fn parse_catalog_page(bytes: &[u8]) -> Result<KiroCatalogPage, GatewayError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Page {
        models: Vec<Model>,
        next_token: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Model {
        model_id: String,
    }
    if bytes.len() > 1024 * 1024 {
        return Err(invalid());
    }
    let page: Page = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if page.models.len() > 256 {
        return Err(invalid());
    }
    let next_token = page.next_token.filter(|v| !v.is_empty());
    if let Some(next) = &next_token {
        validate_cursor(next)?;
    }
    let models = page
        .models
        .into_iter()
        .map(|m| {
            if m.model_id.trim() != m.model_id
                || m.model_id.len() > 256
                || m.model_id.chars().any(char::is_control)
            {
                return Err(invalid());
            }
            DiscoveredModel::try_new(m.model_id).map_err(|_| invalid())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(KiroCatalogPage { models, next_token })
}
fn validate_cursor(value: &str) -> Result<(), GatewayError> {
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn invalid() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        ErrorScope::Provider,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint_policy::KiroApiRegion;
    #[test]
    fn metadata_request_stays_in_exact_region_and_credential_family()
    -> Result<(), Box<dyn std::error::Error>> {
        let credential = KiroCredential::import_json(br#"{"kind":"social","access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_at_ms":2000}"#, 1000)?;
        let policy = KiroEndpointPolicy::try_new(
            KiroEndpointKind::Ide,
            KiroApiRegion::try_new("eu-central-1")?,
        )?;
        let (url, headers) = catalog_request(&policy, &credential, 1000, Some("opaque+/=?"))?;
        assert!(url.starts_with("https://q.eu-central-1.amazonaws.com/ListAvailableModels?origin=AI_EDITOR&maxResults=50&nextToken="));
        assert!(!url.contains("profileArn"));
        assert_eq!(headers[1].1, "Bearer synthetic-access");
        assert!(catalog_request(&policy, &credential, 2000, None).is_err());
        let cli = KiroEndpointPolicy::try_new(
            KiroEndpointKind::Cli,
            KiroApiRegion::try_new("us-east-1")?,
        )?;
        assert!(catalog_request(&cli, &credential, 1000, None).is_err());
        Ok(())
    }
    #[test]
    fn metadata_preserves_ids_and_rejects_partial_or_malformed_pages()
    -> Result<(), Box<dyn std::error::Error>> {
        let page = parse_catalog_page(br#"{"models":[{"modelId":"claude.vendor/Exact-v2"},{"modelId":"another"}],"nextToken":"opaque"}"#)?;
        assert_eq!(page.models[0].upstream_model(), "claude.vendor/Exact-v2");
        assert_eq!(page.next_token.as_deref(), Some("opaque"));
        for input in [
            b"{}".as_slice(),
            br#"{"models":[{"modelId":" x "}]}"#,
            br#"{"models":[],"nextToken":42}"#,
        ] {
            assert!(parse_catalog_page(input).is_err());
        }
        assert!(parse_catalog_page(br#"{"models":[]}"#)?.models.is_empty());
        Ok(())
    }
}
