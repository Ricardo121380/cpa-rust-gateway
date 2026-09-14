//! Fixed Claude OAuth endpoints from the pinned `CLIProxyAPI` reference.
use super::*;
use provider_anthropic_compatible::{
    CLAUDE_OAUTH_CLIENT_ID, CLAUDE_OAUTH_TOKEN_URL, ClaudeRuntimeCredential,
};
use std::io::Read;
use zeroize::Zeroize;

const REDIRECT: &str = "http://localhost:54545/callback";
const PROFILE: &str = "https://api.anthropic.com/api/oauth/profile";
const SCOPE: &str =
    "user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";

/// Creates a Claude-specific exchange without inheriting ambient proxy settings.
#[must_use]
pub fn workflow(proxy: UpstreamProxy) -> Box<dyn ManagementEndpointWorkflow> {
    Box::new(
        CodexOAuthManagementWorkflow::with_exchange(Box::new(Exchange { proxy }))
            .with_url_builder(authorization_url),
    )
}

fn authorization_url(session: &CodexOAuthSession) -> String {
    let Ok((state, verifier)) = session.transient_challenge() else {
        return String::new();
    };
    let verifier = URL_SAFE_NO_PAD.encode(verifier);
    let challenge = URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("code", "true")
        .append_pair("client_id", CLAUDE_OAUTH_CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT)
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &URL_SAFE_NO_PAD.encode(state))
        .finish();
    format!("https://claude.ai/oauth/authorize?{query}")
}

struct Exchange {
    proxy: UpstreamProxy,
}
#[derive(Serialize)]
struct Request<'a> {
    grant_type: &'static str,
    code: &'a str,
    redirect_uri: &'static str,
    client_id: &'static str,
    code_verifier: &'a str,
    state: &'a str,
}
#[derive(Default, Deserialize)]
struct Account {
    #[serde(default)]
    uuid: String,
    #[serde(default, alias = "email")]
    email_address: String,
}
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    #[serde(default)]
    account: Account,
}
impl Drop for TokenResponse {
    fn drop(&mut self) {
        self.access_token.zeroize();
        self.refresh_token.zeroize();
    }
}
#[derive(Deserialize)]
struct Profile {
    account: Account,
}
#[derive(Serialize)]
struct Envelope<'a> {
    kind: &'static str,
    access_token: &'a str,
    refresh_token: &'a str,
    expires_at_ms: i64,
    account_id: Option<&'a str>,
    email: Option<&'a str>,
}

fn bounded(response: reqwest::blocking::Response) -> Option<Zeroizing<Vec<u8>>> {
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|n| n > MAX_CODEX_OAUTH_RESPONSE_BYTES)
    {
        return None;
    }
    let mut bytes = Zeroizing::new(Vec::new());
    response
        .take(MAX_CODEX_OAUTH_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_CODEX_OAUTH_RESPONSE_BYTES).then_some(bytes)
}

fn normalize(
    token: &TokenResponse,
    profile: Option<&Profile>,
    now: i64,
) -> Option<Zeroizing<Vec<u8>>> {
    if now < 0 || token.expires_in <= 0 {
        return None;
    }
    let expires = now.checked_add(token.expires_in.checked_mul(1000)?)?;
    let profile =
        profile.filter(|p| token.account.uuid.is_empty() || p.account.uuid == token.account.uuid);
    let account = if token.account.uuid.is_empty() {
        profile.map(|p| p.account.uuid.as_str())
    } else {
        Some(token.account.uuid.as_str())
    };
    let email = if token.account.email_address.is_empty() {
        profile.map(|p| p.account.email_address.as_str())
    } else {
        Some(token.account.email_address.as_str())
    };
    let envelope = Envelope {
        kind: "claude_oauth",
        access_token: &token.access_token,
        refresh_token: &token.refresh_token,
        expires_at_ms: expires,
        account_id: account.filter(|v| !v.is_empty()),
        email: email.filter(|v| !v.is_empty()),
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&envelope).ok()?);
    ClaudeRuntimeCredential::import_at(&bytes, now)
        .ok()?
        .authorization_at(now)
        .ok()?;
    Some(bytes)
}

impl ManagementCodexOAuthExchange for Exchange {
    fn exchange(
        &mut self,
        _: &CredentialId,
        _: Zeroizing<String>,
        _: Zeroizing<Vec<u8>>,
    ) -> Option<Zeroizing<Vec<u8>>> {
        None
    }
    fn exchange_with_state(
        &mut self,
        _: &CredentialId,
        code: Zeroizing<String>,
        verifier: Zeroizing<Vec<u8>>,
        state: &[u8],
    ) -> Option<Zeroizing<Vec<u8>>> {
        let verifier = std::str::from_utf8(&verifier).ok()?;
        let state = URL_SAFE_NO_PAD.encode(state);
        let body = Zeroizing::new(
            serde_json::to_vec(&Request {
                grant_type: "authorization_code",
                code: &code,
                redirect_uri: REDIRECT,
                client_id: CLAUDE_OAUTH_CLIENT_ID,
                code_verifier: verifier,
                state: &state,
            })
            .ok()?,
        );
        let client = codex_oauth_http_client(&self.proxy).ok()?;
        let response = client
            .post(CLAUDE_OAUTH_TOKEN_URL)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("User-Agent", "axios/1.15.2")
            .body(body.to_vec())
            .send()
            .ok()?;
        let body = bounded(response)?;
        let token: TokenResponse = serde_json::from_slice(&body).ok()?;
        // Identity lookup is advisory; a failure remains unobserved, never fabricated.
        let profile = client
            .get(PROFILE)
            .bearer_auth(&token.access_token)
            .header("Accept", "application/json")
            .header("User-Agent", "axios/1.15.2")
            .send()
            .ok()
            .and_then(bounded)
            .and_then(|bytes| serde_json::from_slice::<Profile>(&bytes).ok());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_millis()).ok())?;
        normalize(&token, profile.as_ref(), now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn claude_flow_uses_its_own_client_redirect_and_preserves_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let session = CodexOAuthSession::start(CredentialId::try_new("claude")?, 1000)?;
        let url = url::Url::parse(&authorization_url(&session))?;
        assert_eq!(url.host_str(), Some("claude.ai"));
        let params: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params["redirect_uri"], REDIRECT);
        assert_eq!(params["client_id"], CLAUDE_OAUTH_CLIENT_ID);
        let token: TokenResponse = serde_json::from_str(
            r#"{"access_token":"a","refresh_token":"r","expires_in":3600,"account":{"uuid":"owner"}}"#,
        )?;
        let profile: Profile =
            serde_json::from_str(r#"{"account":{"uuid":"owner","email":"owner@example.test"}}"#)?;
        let bytes = normalize(&token, Some(&profile), 1000).ok_or("normalize")?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(value["email"], "owner@example.test");
        assert_eq!(value["expires_at_ms"], 3_601_000);
        let other: Profile =
            serde_json::from_str(r#"{"account":{"uuid":"other","email":"wrong@example.test"}}"#)?;
        let value: serde_json::Value =
            serde_json::from_slice(&normalize(&token, Some(&other), 1000).ok_or("normalize")?)?;
        assert!(value["email"].is_null());
        assert!(normalize(&token, None, i64::MAX).is_none());
        Ok(())
    }
}
