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
    let mut kimi = ["kimi", "kimi-coding", "moonshot"].contains(&upstream);
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
    } else if kind == "oauth_json" && ["codex", "chatgpt", "openai-compatible"].contains(&upstream)
    {
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
    } else if upstream == "grok.official" {
        ("api", "Grok Official")
    } else {
        ("api", "API")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn channel_names_do_not_depend_on_opaque_credential_ids_or_oauth_storage_kind() {
        assert_eq!(
            ordinary_channel("oauth_json", "kimi-coding", []),
            ("kimi", "Kimi")
        );
        assert_eq!(ordinary_channel("oauth_json", "kiro", []), ("kiro", "Kiro"));
        assert_eq!(
            ordinary_channel("oauth_json", "claude", []),
            ("claude", "Claude")
        );
        assert_eq!(
            ordinary_channel("oauth_json", "codex", []),
            ("codex", "Codex")
        );
        assert_eq!(
            ordinary_channel("bearer", "grok.official", []),
            ("api", "Grok Official")
        );
        assert_eq!(
            ordinary_channel("bearer", "openai-compatible", []),
            ("api", "API")
        );
    }
}
