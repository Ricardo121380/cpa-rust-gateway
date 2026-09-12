//! Shared management display metadata; never used for authorization or scheduling.
use gateway_store::account_identity::AccountIdentity;

/// Human identity and the meaning of one exact operational connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountPresentation {
    /// Provider-observed email, phone or username.
    pub identity: AccountIdentity,
    /// Stable management category, independent of opaque resource IDs.
    pub category: String,
    /// Actual provider/channel name.
    pub provider: String,
    /// Exact protocol identifier; the frontend localizes its display label.
    pub api_format: String,
    /// Sanitized host only, without URL credentials, path or query.
    pub host: Option<String>,
    /// Import provenance, never used as the account name.
    pub source: Option<String>,
}

/// Classifies ordinary accounts from provider kind and configured protocol/host evidence.
pub fn ordinary_channel<'a>(
    kind: &str,
    upstream: &str,
    connections: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
) -> (&'static str, &'static str) {
    let mut kimi = ["kimi", "moonshot"].contains(&upstream);
    let mut kiro = upstream == "kiro";
    let mut claude = ["claude", "anthropic-compatible"].contains(&upstream);
    for (adapter, host) in connections {
        kimi |= host.is_some_and(|host| {
            ["api.moonshot.cn", "api.moonshot.ai", "api.kimi.com"].contains(&host)
        });
        kiro |= adapter.starts_with("kiro");
        claude |= adapter.starts_with("anthropic");
    }
    if kimi {
        ("kimi", "Kimi")
    } else if kind == "oauth_json" {
        (
            "codex",
            if upstream == "chatgpt" {
                "ChatGPT"
            } else {
                "Codex"
            },
        )
    } else if kiro {
        ("kiro", "Kiro")
    } else if claude {
        ("claude", "Claude")
    } else {
        ("api", "API")
    }
}
