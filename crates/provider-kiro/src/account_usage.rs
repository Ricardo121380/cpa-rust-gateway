//! Fixed, read-only Kiro usage request; no inferred profile ARN or region fallback.
use crate::{
    credential::{KiroCredential, KiroCredentialKind},
    endpoint_policy::{KiroEndpointKind, KiroEndpointPolicy},
};
use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};

/// Constructs the channel-specific GET usage request, including an explicit email request.
/// # Errors
/// Rejects expired credentials. No token, URL or region is accepted from a browser request.
pub fn usage_request(
    policy: &KiroEndpointPolicy,
    credential: &KiroCredential,
    now: i64,
) -> Result<(String, Vec<(String, String)>), GatewayError> {
    let invalid = || {
        GatewayError::new(
            GatewayErrorCode::UpstreamProtocolError,
            ErrorScope::Provider,
        )
    };
    if credential.is_expired_at(now) {
        return Err(invalid());
    }
    let region = policy.api_region().as_str();
    let host = if policy.kind() == KiroEndpointKind::Ide && region == "us-east-1" {
        "codewhisperer.us-east-1.amazonaws.com".to_owned()
    } else {
        format!("q.{region}.amazonaws.com")
    };
    let mut url = policy.url().clone();
    url.set_host(Some(&host)).map_err(|_| invalid())?;
    url.set_path("/getUsageLimits");
    url.set_query(None);
    url.query_pairs_mut()
        .append_pair("isEmailRequired", "true")
        .append_pair("origin", policy.origin().as_header_value())
        .append_pair("resourceType", "AGENTIC_REQUEST");
    let token = if credential.kind() == KiroCredentialKind::ApiKey {
        credential.api_key()
    } else {
        credential.access_token()
    }
    .map_err(|_| invalid())?;
    let mut headers = vec![
        ("accept".into(), "application/json".into()),
        ("authorization".into(), format!("Bearer {token}")),
    ];
    if credential.kind() == KiroCredentialKind::ApiKey {
        headers.push(("tokentype".into(), "API_KEY".into()));
    }
    Ok((url.to_string(), headers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint_policy::KiroApiRegion;
    #[test]
    fn usage_preserves_region_origin_and_expiry() -> Result<(), Box<dyn std::error::Error>> {
        let credential = KiroCredential::import_json(br#"{"kind":"social","access_token":"synthetic","refresh_token":"synthetic-refresh","expires_at_ms":2000}"#,1000)?;
        for (kind, region, host, origin) in [
            (
                KiroEndpointKind::Ide,
                "us-east-1",
                "codewhisperer.us-east-1.amazonaws.com",
                "AI_EDITOR",
            ),
            (
                KiroEndpointKind::Cli,
                "us-east-1",
                "q.us-east-1.amazonaws.com",
                "KIRO_CLI",
            ),
            (
                KiroEndpointKind::Cli,
                "eu-central-1",
                "q.eu-central-1.amazonaws.com",
                "KIRO_CLI",
            ),
        ] {
            let policy = KiroEndpointPolicy::try_new(kind, KiroApiRegion::try_new(region)?)?;
            let (url, headers) = usage_request(&policy, &credential, 1000)?;
            assert!(url.starts_with(&format!("https://{host}/getUsageLimits?")));
            assert!(url.contains(&format!("origin={origin}")));
            assert!(url.contains("isEmailRequired=true"));
            assert!(!url.contains("profileArn"));
            assert_eq!(headers[1].1, "Bearer synthetic");
            assert!(usage_request(&policy, &credential, 2000).is_err());
        }
        let key = KiroCredential::import_runtime_secret(b"ksk_fixture_nonlive", 1000)?;
        let policy = KiroEndpointPolicy::try_new(
            KiroEndpointKind::Cli,
            KiroApiRegion::try_new("us-east-1")?,
        )?;
        let (_, headers) = usage_request(&policy, &key, 1000)?;
        assert!(
            headers
                .iter()
                .any(|(name, value)| name == "tokentype" && value == "API_KEY")
        );
        Ok(())
    }
}
