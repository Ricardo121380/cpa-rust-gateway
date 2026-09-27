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

/// CLI management requests use the selected API region and an actual returned profile ARN.
pub struct KiroCliCatalogRequest {
    /// Fixed HTTPS management destination.
    pub url: String,
    /// Provider authentication and RPC target headers; never log these values.
    pub headers: Vec<(String, String)>,
    /// RPC payload, including the observed profile when reading models.
    pub body: Vec<u8>,
}
/// Constructs either profile discovery or a model page, without a guessed profile fallback.
/// # Errors
/// Rejects IDE/API-key/expired credentials, invalid profiles and cursors.
pub fn cli_catalog_request(
    policy: &KiroEndpointPolicy,
    credential: &KiroCredential,
    now: i64,
    profile: Option<&str>,
    next: Option<&str>,
) -> Result<KiroCliCatalogRequest, GatewayError> {
    if policy.kind() != KiroEndpointKind::Cli
        || credential.kind() == KiroCredentialKind::ApiKey
        || credential.is_expired_at(now)
    {
        return Err(invalid());
    }
    let mut url = policy.url().clone();
    url.set_host(Some(&format!(
        "management.{}.kiro.dev",
        policy.api_region().as_str()
    )))
    .map_err(|_| invalid())?;
    let (target, body) = if let Some(profile) = profile {
        validate_profile(profile, policy)?;
        url.query_pairs_mut()
            .append_pair("origin", "KIRO_CLI")
            .append_pair("profileArn", profile);
        let mut body = serde_json::json!({"origin":"KIRO_CLI","profileArn":profile});
        if let Some(next) = next {
            validate_cursor(next)?;
            body["nextToken"] = next.into();
        }
        ("AmazonCodeWhispererService.ListAvailableModels", body)
    } else {
        if next.is_some() {
            return Err(invalid());
        }
        (
            "AmazonCodeWhispererService.ListAvailableProfiles",
            serde_json::json!({}),
        )
    };
    let mut headers = policy
        .request_headers(credential.kind())
        .into_iter()
        .collect::<Vec<_>>();
    headers.retain(|(name, _)| !name.eq_ignore_ascii_case("x-amz-target"));
    headers.push(("x-amz-target".into(), target.into()));
    headers.push((
        "authorization".into(),
        format!(
            "Bearer {}",
            credential.access_token().map_err(|_| invalid())?
        ),
    ));
    Ok(KiroCliCatalogRequest {
        url: url.to_string(),
        headers,
        body: serde_json::to_vec(&body).map_err(|_| invalid())?,
    })
}
/// Selects one unambiguous active Kiro profile. Never selects an arbitrary first profile.
/// # Errors
/// Rejects missing, ambiguous, paginated or invalid profile evidence.
pub fn parse_cli_profile(
    bytes: &[u8],
    policy: &KiroEndpointPolicy,
) -> Result<String, GatewayError> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if bytes.len() > 1024 * 1024
        || value
            .get("nextToken")
            .is_some_and(|v| !v.is_null() && v.as_str() != Some(""))
    {
        return Err(invalid());
    }
    let profiles = value["profiles"]
        .as_array()
        .filter(|p| p.len() <= 100)
        .ok_or_else(invalid)?;
    let mut active = profiles
        .iter()
        .filter(|p| p["profileType"] == "KIRO" && p["status"] == "ACTIVE");
    let arn = active
        .next()
        .and_then(|p| p["arn"].as_str())
        .ok_or_else(invalid)?;
    if active.next().is_some() {
        return Err(invalid());
    }
    validate_profile(arn, policy)?;
    Ok(arn.to_owned())
}
fn validate_profile(arn: &str, policy: &KiroEndpointPolicy) -> Result<(), GatewayError> {
    let prefix = format!("arn:aws:codewhisperer:{}:", policy.api_region().as_str());
    if arn.len() > 512
        || !arn.starts_with(&prefix)
        || !arn.contains(":profile/")
        || arn.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        Err(invalid())
    } else {
        Ok(())
    }
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
    fn cli_catalog_requires_observed_unambiguous_profile() -> Result<(), Box<dyn std::error::Error>>
    {
        let policy = KiroEndpointPolicy::try_new(
            KiroEndpointKind::Cli,
            KiroApiRegion::try_new("eu-central-1")?,
        )?;
        let credential=KiroCredential::import_json(br#"{"kind":"social","access_token":"synthetic","refresh_token":"synthetic","expires_at_ms":2000}"#,1000)?;
        let profile = cli_catalog_request(&policy, &credential, 1000, None, None)?;
        assert_eq!(profile.url, "https://management.eu-central-1.kiro.dev/");
        assert_eq!(profile.body, b"{}");
        let arn=parse_cli_profile(br#"{"profiles":[{"arn":"arn:aws:codewhisperer:eu-central-1:123456789012:profile/observed","profileType":"KIRO","status":"ACTIVE"}]}"#,&policy)?;
        let models = cli_catalog_request(&policy, &credential, 1000, Some(&arn), Some("cursor+2"))?;
        assert!(
            models.headers.iter().any(|(k, v)| k == "x-amz-target"
                && v == "AmazonCodeWhispererService.ListAvailableModels")
        );
        let body: serde_json::Value = serde_json::from_slice(&models.body)?;
        assert_eq!(body["profileArn"], arn);
        assert_eq!(body["nextToken"], "cursor+2");
        assert!(parse_cli_profile(br#"{"profiles":[]}"#, &policy).is_err());
        assert!(
            parse_cli_profile(
                br#"{"profiles":[{"arn":"invalid","profileType":"KIRO","status":"ACTIVE"}]}"#,
                &policy
            )
            .is_err()
        );
        let key = KiroCredential::import_runtime_secret(b"ksk_synthetic", 1000)?;
        assert!(cli_catalog_request(&policy, &key, 1000, None, None).is_err());
        Ok(())
    }
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
