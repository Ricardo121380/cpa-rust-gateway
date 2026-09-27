//! Fixed native Grok metadata destinations; no inference or retry fallback.
mod grpc;
use gateway_core::EgressPolicyId;
use gateway_http_actix::management_resources::{
    account_quota::{AccountQuotaObservation, AccountQuotaWindow},
    native_accounts::usage::{NativeUsageError as Error, NativeUsageTransport},
};
use gateway_upstream::{
    EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy,
    SystemEgressDnsResolver, UpstreamClientPool, UpstreamHttpMethod, UpstreamHttpRequest,
    UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};
use provider_grok::{
    GrokAccountIdentitySnapshot, GrokAccountProvider, GrokBuildCredential, GrokConsoleDpopSession,
    GrokSessionIdentityRequest,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    num::NonZeroUsize,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

pub(crate) struct NativeUsageReader {
    policy: EgressPolicy,
    pool: UpstreamClientPool,
    direct: UpstreamTransportProfile,
    web: UpstreamTransportProfile,
}
impl NativeUsageReader {
    pub(crate) fn new(web_proxy: Option<UpstreamProxy>) -> Result<Self, Error> {
        let one = NonZeroUsize::new(1).ok_or(Error::Unavailable)?;
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("native-account-usage").map_err(|_| Error::Unavailable)?,
            name: "Native account usage".into(),
            allowed_schemes: BTreeSet::from([EgressScheme::Https]),
            allowed_hosts: ["cli-chat-proxy.grok.com", "console.x.ai", "grok.com"]
                .into_iter()
                .map(EgressHost::try_new)
                .collect::<Result<_, _>>()
                .map_err(|_| Error::Unavailable)?,
            allowed_ports: BTreeSet::from([443]),
            allowed_cidrs: BTreeSet::new(),
            redirect_policy: RedirectPolicy::Deny,
        })
        .map_err(|_| Error::Unavailable)?;
        let timeouts = UpstreamTimeouts::try_new(
            Duration::from_secs(4),
            Duration::from_secs(8),
            Duration::from_secs(8),
            Duration::from_secs(8),
        )
        .map_err(|_| Error::Unavailable)?;
        let direct = UpstreamTransportProfile::new(timeouts, UpstreamProxy::Direct, one);
        let web = UpstreamTransportProfile::new(
            timeouts,
            web_proxy.unwrap_or(UpstreamProxy::Direct),
            one,
        )
        .with_chrome_146_emulation();
        Ok(Self {
            policy,
            pool: UpstreamClientPool::new(NonZeroUsize::new(4).ok_or(Error::Unavailable)?),
            direct,
            web,
        })
    }
    async fn response_bytes(
        &self,
        url: &'static str,
        method: UpstreamHttpMethod,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
        web: bool,
    ) -> Result<Zeroizing<Vec<u8>>, Error> {
        let policy = self.policy.clone();
        let admitted =
            tokio::task::spawn_blocking(move || policy.admit_url(url, &SystemEgressDnsResolver))
                .await
                .map_err(|_| Error::Unavailable)?
                .map_err(|_| Error::Unavailable)?;
        let request = UpstreamHttpRequest::try_new(admitted, method, headers, body)
            .map_err(|_| Error::Unavailable)?;
        let mut response = self
            .pool
            .send(request, if web { &self.web } else { &self.direct })
            .await
            .map_err(|_| Error::Unavailable)?;
        match response.status() {
            401 => return Err(Error::Unauthorized),
            403 => return Err(Error::Forbidden),
            200..=299 => {}
            _ => return Err(Error::Unavailable),
        }
        let mut bytes = Zeroizing::new(Vec::new());
        while let Some(chunk) = response
            .next_chunk()
            .await
            .map_err(|_| Error::Unavailable)?
        {
            if bytes.len().saturating_add(chunk.len()) > 65_536 {
                return Err(Error::InvalidResponse);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    async fn document(
        &self,
        url: &'static str,
        method: UpstreamHttpMethod,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
        web: bool,
    ) -> Result<Value, Error> {
        let bytes = self.response_bytes(url, method, headers, body, web).await?;
        serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)
    }
    // This extra window is optional: missing path-scoped cookies or a failed
    // billing request must not discard the successful REST observations.
    async fn web_weekly(
        &self,
        credential: &provider_grok::GrokWebCredential,
        mut headers: Vec<(String, String)>,
    ) -> Option<AccountQuotaWindow> {
        let cookie = web_cookie(
            credential,
            "/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig",
        )
        .ok()?;
        headers.retain(|(name, _)| name != "content-type" && name != "accept" && name != "cookie");
        headers.extend([
            ("cookie".into(), cookie.to_string()),
            ("content-type".into(), "application/grpc-web+proto".into()),
            ("accept".into(), "application/grpc-web+proto".into()),
            ("x-grpc-web".into(), "1".into()),
            ("x-user-agent".into(), "connect-es/2.1.1".into()),
        ]);
        let bytes = tokio::time::timeout(
            Duration::from_secs(2),
            self.response_bytes(
                "https://grok.com/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig",
                UpstreamHttpMethod::Post,
                headers,
                vec![0; 5],
                true,
            ),
        )
        .await
        .ok()?
        .ok()?;
        grpc::weekly(&bytes).ok()
    }
    // Each provider has a distinct fixed credential exchange; keep the no-fallback match explicit.
    #[allow(clippy::too_many_lines)]
    async fn fetch(
        &self,
        snapshot: GrokAccountIdentitySnapshot,
    ) -> Result<AccountQuotaObservation, Error> {
        let now = now_ms()?;
        match snapshot.provider {
            GrokAccountProvider::Build => {
                let credential = GrokBuildCredential::import_refreshable_runtime(
                    snapshot.credential.as_bytes(),
                    now,
                )
                .map_err(|_| Error::Unauthorized)?;
                if credential.is_expired_at(now) {
                    return Err(Error::Unauthorized);
                }
                let headers = vec![
                    ("accept".into(), "application/json".into()),
                    (
                        "authorization".into(),
                        format!("Bearer {}", credential.access_token()),
                    ),
                    ("x-xai-token-auth".into(), "xai-grok-cli".into()),
                    (
                        "x-grok-client-version".into(),
                        provider_grok::GROK_BUILD_CLIENT_VERSION.into(),
                    ),
                    (
                        "user-agent".into(),
                        provider_grok::GROK_BUILD_USER_AGENT.into(),
                    ),
                ];
                let value = self
                    .document(
                        "https://cli-chat-proxy.grok.com/v1/billing?format=credits",
                        UpstreamHttpMethod::Get,
                        headers,
                        vec![],
                        false,
                    )
                    .await?;
                parse_build(&value, now_ms()?)
            }
            GrokAccountProvider::Console => {
                let credential = GrokSessionIdentityRequest::from_credential(
                    snapshot.provider,
                    snapshot.credential.as_bytes(),
                    now,
                )
                .map_err(|_| Error::Unauthorized)?;
                let mut headers = vec![
                    ("accept".into(), "application/json".into()),
                    ("content-type".into(), "application/json".into()),
                    ("cookie".into(), credential.cookie().into()),
                    ("origin".into(), "https://console.x.ai".into()),
                    ("referer".into(), "https://console.x.ai/".into()),
                    (
                        "user-agent".into(),
                        provider_grok::GROK_CONSOLE_USER_AGENT.into(),
                    ),
                ];
                let key = GrokConsoleDpopSession::generate_key();
                let body = GrokConsoleDpopSession::token_exchange_body(&key)
                    .map_err(|_| Error::Unavailable)?;
                let value = self
                    .document(
                        "https://console.x.ai/v1/dpop/token",
                        UpstreamHttpMethod::Post,
                        headers.clone(),
                        body,
                        true,
                    )
                    .await?;
                let token: DpopGrant =
                    serde_json::from_value(value).map_err(|_| Error::InvalidResponse)?;
                let session = GrokConsoleDpopSession::from_token_response(
                    token.access_token,
                    &token.token_type,
                    token.expires_in,
                    key,
                    SystemTime::now(),
                )
                .map_err(|_| Error::InvalidResponse)?;
                headers.push((
                    "authorization".into(),
                    format!("DPoP {}", session.access_token()),
                ));
                headers.push((
                    "dpop".into(),
                    session
                        .proof("GET", "https://console.x.ai/v1/usage", SystemTime::now())
                        .map_err(|_| Error::Unavailable)?,
                ));
                let value = self
                    .document(
                        "https://console.x.ai/v1/usage",
                        UpstreamHttpMethod::Get,
                        headers,
                        vec![],
                        true,
                    )
                    .await?;
                parse_console(&value, now_ms()?)
            }
            GrokAccountProvider::Web => {
                let credential = provider_grok::GrokWebCredential::import_sso_json(
                    snapshot.credential.as_bytes(),
                    now,
                )
                .map_err(|_| Error::Unauthorized)?;
                let cookie = web_cookie(&credential, "/rest/rate-limits")?;
                let headers = vec![
                    ("accept".into(), "application/json".into()),
                    ("content-type".into(), "application/json".into()),
                    ("cookie".into(), cookie.to_string()),
                    ("origin".into(), "https://grok.com".into()),
                    ("referer".into(), "https://grok.com/".into()),
                    (
                        "user-agent".into(),
                        provider_grok::GROK_WEB_PRODUCTION_USER_AGENT.into(),
                    ),
                ];
                let mut result = empty("grok_web", now);
                for mode in ["auto", "fast"] {
                    let body = serde_json::to_vec(&serde_json::json!({"modelName":mode}))
                        .map_err(|_| Error::Unavailable)?;
                    let value = self
                        .document(
                            "https://grok.com/rest/rate-limits",
                            UpstreamHttpMethod::Post,
                            headers.clone(),
                            body,
                            true,
                        )
                        .await?;
                    result.windows.push(parse_web_window(mode, &value)?);
                }
                if let Some(window) = self.web_weekly(&credential, headers).await {
                    result.windows.push(window);
                }
                result.observed_at_ms = now_ms()?;
                // Media quotas and product sub-pools are not part of this observation.
                result.partial = true;
                Ok(result)
            }
        }
    }
}
impl NativeUsageTransport for NativeUsageReader {
    fn read(
        &self,
        snapshot: GrokAccountIdentitySnapshot,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<AccountQuotaObservation, Error>> + Send + '_>,
    > {
        Box::pin(async move {
            tokio::time::timeout(Duration::from_secs(19), self.fetch(snapshot))
                .await
                .map_err(|_| Error::Unavailable)?
        })
    }
}
#[derive(Deserialize)]
struct DpopGrant {
    access_token: String,
    token_type: String,
    expires_in: u64,
}
fn now_ms() -> Result<i64, Error> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Unavailable)?
            .as_millis(),
    )
    .map_err(|_| Error::Unavailable)
}
fn web_cookie(
    credential: &provider_grok::GrokWebCredential,
    path: &str,
) -> Result<Zeroizing<String>, Error> {
    let mut header = Zeroizing::new(String::new());
    for c in credential.cookies().iter().filter(|c| {
        c.domain().trim_start_matches('.') == "grok.com"
            && (path == c.path()
                || path
                    .strip_prefix(c.path())
                    .is_some_and(|tail| c.path().ends_with('/') || tail.starts_with('/')))
    }) {
        if !header.is_empty() {
            header.push_str("; ");
        }
        header.push_str(c.name());
        header.push('=');
        header.push_str(c.value());
    }
    if header.is_empty() {
        Err(Error::Unauthorized)
    } else {
        Ok(header)
    }
}
fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|v| v.is_finite() && *v >= 0.0)
}
fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control))
        .map(str::to_owned)
}
fn empty(source: &str, now: i64) -> AccountQuotaObservation {
    AccountQuotaObservation {
        source: source.into(),
        observed_at_ms: now,
        windows: vec![],
        email: None,
        plan: None,
        partial: false,
    }
}
fn window(
    name: &str,
    used: Option<f64>,
    limit: Option<f64>,
    percent: Option<f64>,
    unit: &str,
) -> AccountQuotaWindow {
    AccountQuotaWindow {
        name: name.into(),
        used_percent: percent.or_else(|| {
            let (u, l) = (used?, limit?);
            (l > 0.0).then_some(u / l * 100.0).filter(|v| v.is_finite())
        }),
        used,
        limit,
        unit: Some(unit.into()),
        reset_at: None,
        reset_at_ms: None,
        duration_seconds: None,
    }
}
fn parse_build(value: &Value, now: i64) -> Result<AccountQuotaObservation, Error> {
    let value = value.get("config").unwrap_or(value);
    let mut result = empty("grok_build", now);
    if let Some(percent) =
        number(&value["credit_usage_percent"]).or_else(|| number(&value["creditUsagePercent"]))
    {
        let mut quota = window("build.credits", None, None, Some(percent), "percent");
        quota.reset_at =
            text(&value["current_period"]["end"]).or_else(|| text(&value["currentPeriod"]["end"]));
        result.windows.push(quota);
    }
    if let Some(items) = value
        .get("product_usage")
        .or_else(|| value.get("productUsage"))
        .and_then(Value::as_array)
    {
        result.partial = items.len() > 5;
        for entry in items.iter().take(5) {
            if let (Some(name), Some(percent)) = (
                text(&entry["product"]),
                number(&entry["usage_percent"]).or_else(|| number(&entry["usagePercent"])),
            ) {
                result
                    .windows
                    .push(window(&name, None, None, Some(percent), "percent"));
            } else {
                result.partial = true;
            }
        }
    }
    if result.windows.is_empty() {
        return Err(Error::InvalidResponse);
    }
    Ok(result)
}
fn parse_console(value: &Value, now: i64) -> Result<AccountQuotaObservation, Error> {
    let entries = value["quotas"].as_array().ok_or(Error::InvalidResponse)?;
    let mut result = empty("grok_console", now);
    for name in ["chat", "image", "video"] {
        let matching: Vec<_> = entries
            .iter()
            .filter(|v| v["kind"].as_str() == Some(name))
            .collect();
        if matching.len() != 1 {
            return Err(Error::InvalidResponse);
        }
        let entry = matching[0];
        let used = number(&entry["used"]).ok_or(Error::InvalidResponse)?;
        let limit = number(&entry["limit"]).ok_or(Error::InvalidResponse)?;
        let remaining = number(&entry["remaining"]).ok_or(Error::InvalidResponse)?;
        if remaining > limit {
            return Err(Error::InvalidResponse);
        }
        result.windows.push(window(
            &format!("console.{name}"),
            Some(used),
            Some(limit),
            None,
            "requests",
        ));
    }
    Ok(result)
}
fn parse_web_window(mode: &str, value: &Value) -> Result<AccountQuotaWindow, Error> {
    let total = number(&value["totalQueries"]).ok_or(Error::InvalidResponse)?;
    let remaining = number(&value["remainingQueries"]).ok_or(Error::InvalidResponse)?;
    if remaining > total {
        return Err(Error::InvalidResponse);
    }
    let mut result = window(
        &format!("web.{mode}"),
        Some(total - remaining),
        Some(total),
        None,
        "requests",
    );
    result.duration_seconds = value["windowSizeSeconds"].as_i64().filter(|v| *v > 0);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_preserves_observed_zero_and_rejects_missing_usage() {
        let result = parse_build(&json!({"config":{"credit_usage_percent":0}}), 123);
        assert!(result.is_ok());
        let result = result.unwrap_or_else(|_| unreachable!());
        assert_eq!(result.windows[0].used_percent, Some(0.0));
        assert_eq!(result.windows[0].reset_at, None);
        assert!(parse_build(&json!({"plan":"premium"}), 123).is_err());
        assert!(parse_build(&json!({"credit_usage_percent":-1}), 123).is_err());
    }

    #[test]
    fn build_bounds_breakdowns_and_flags_partial_data() {
        let result = parse_build(
            &json!({"credit_usage_percent":25,"product_usage":[{"product":"chat","usage_percent":10},{"product":"invalid"}]}),
            123,
        );
        let result = result.unwrap_or_else(|_| unreachable!());
        assert!(result.partial);
        assert_eq!(result.windows.len(), 2);
    }

    #[test]
    fn console_zero_limit_is_not_zero_percent_and_duplicate_kinds_fail() {
        let mut value = json!({"quotas":[
            {"kind":"chat","used":0,"limit":0,"remaining":0},
            {"kind":"image","used":3,"limit":10,"remaining":7},
            {"kind":"video","used":0,"limit":10,"remaining":10}
        ]});
        let result = parse_console(&value, 123).unwrap_or_else(|_| unreachable!());
        assert_eq!(result.windows[0].used_percent, None);
        assert_eq!(result.windows[1].used_percent, Some(30.0));
        assert!(result.windows.iter().all(|w| w.reset_at_ms.is_none()));
        value["quotas"][2]["kind"] = json!("chat");
        assert!(parse_console(&value, 123).is_err());
        assert!(parse_console(&json!({}), 123).is_err());
    }

    #[tokio::test]
    async fn web_rest_scoped_cookie_skips_optional_billing_without_network() {
        let input = json!({
            "kind":"grok_web_sso", "account_ref":"fixture_account",
            "lineage_ref":"fixture_import", "revision":1, "expires_at_ms":10000,
            "cookies":[{"name":"sso", "value":"synthetic-session",
                "domain":"grok.com", "path":"/rest/rate-limits",
                "secure":true, "http_only":true}]
        })
        .to_string();
        let credential = provider_grok::GrokWebCredential::import_sso_json(input.as_bytes(), 1)
            .unwrap_or_else(|_| unreachable!());
        assert!(web_cookie(&credential, "/rest/rate-limits").is_ok());
        assert!(web_cookie(&credential, "/rest/rate-limits-other").is_err());
        let reader = NativeUsageReader::new(None).unwrap_or_else(|_| unreachable!());
        assert!(reader.web_weekly(&credential, vec![]).await.is_none());
    }

    #[test]
    fn web_requires_observed_counts_and_does_not_invent_reset() {
        let result = parse_web_window(
            "auto",
            &json!({"totalQueries":20,"remainingQueries":15,"windowSizeSeconds":3600}),
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(result.used_percent, Some(25.0));
        assert_eq!(result.duration_seconds, Some(3600));
        assert_eq!(result.reset_at_ms, None);
        assert!(parse_web_window("auto", &json!({"totalQueries":20})).is_err());
        assert!(
            parse_web_window("auto", &json!({"totalQueries":20,"remainingQueries":21})).is_err()
        );
        let zero = parse_web_window("auto", &json!({"totalQueries":0,"remainingQueries":0}))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(zero.used_percent, None);
    }
}
