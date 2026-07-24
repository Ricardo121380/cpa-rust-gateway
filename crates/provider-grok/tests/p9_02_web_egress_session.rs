//! P9-02 synthetic Grok Web browser egress-session evidence.

use std::error::Error;

use gateway_upstream::UpstreamProxy;
use provider_grok::{
    GROK_WEB_PROVIDER_ID, GrokWebBrowserEgressSession, GrokWebBrowserEgressSessionError,
    GrokWebBrowserUserAgent, GrokWebCredential, GrokWebEgressSessionId, GrokWebTlsProfile,
};

const OBSERVED_AT_MS: i64 = 1_000_000;
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36";

#[test]
fn browser_egress_session_binds_exact_credential_fingerprint_and_cookie_scopes()
-> Result<(), Box<dyn Error>> {
    let initial_credential =
        credential("web_account_01", "sso_import_01", 7, "session_value_alpha")?;
    let session = session(initial_credential.clone(), UpstreamProxy::Direct)?;

    assert_eq!(
        GrokWebBrowserEgressSession::provider_id(),
        GROK_WEB_PROVIDER_ID
    );
    assert_eq!(session.egress_session_id().as_str(), "web_egress_01");
    assert_eq!(session.account_reference(), "web_account_01");
    assert_eq!(session.lineage_reference(), "sso_import_01");
    assert_eq!(session.credential_revision(), 7);
    assert_eq!(session.credential_expires_at_ms(), 2_000_000);
    assert_eq!(session.created_at_ms(), OBSERVED_AT_MS);
    assert_eq!(session.user_agent().header_value(), USER_AGENT);
    assert_eq!(session.tls_profile().as_str(), "chrome_136_macos");
    assert_eq!(session.proxy(), &UpstreamProxy::Direct);
    session.require_current_credential(&initial_credential, OBSERVED_AT_MS)?;

    let cookie_header =
        session.cookie_header_for_https("grok.example.test", "/chat/next", OBSERVED_AT_MS)?;
    assert_eq!(
        cookie_header.as_str(),
        "csrf=csrf_value; sso_session=session_value_alpha"
    );
    let debug = format!("{session:?}");
    assert!(!debug.contains("session_value_alpha"));
    assert!(!debug.contains(USER_AGENT));
    assert!(debug.contains("<redacted>"));
    Ok(())
}

#[test]
fn browser_egress_session_rejects_expiry_scope_and_unsafe_fingerprint_inputs()
-> Result<(), Box<dyn Error>> {
    let initial_credential =
        credential("web_account_01", "sso_import_01", 7, "session_value_alpha")?;
    let session = session(initial_credential.clone(), UpstreamProxy::Direct)?;
    assert_eq!(
        session.cookie_header_for_https("unrelated.example.test", "/chat", OBSERVED_AT_MS),
        Err(GrokWebBrowserEgressSessionError::CookieScopeMismatch)
    );
    assert_eq!(
        session.cookie_header_for_https("grok.example.test", "/chat?query", OBSERVED_AT_MS),
        Err(GrokWebBrowserEgressSessionError::InvalidRequestScope)
    );
    assert_eq!(
        session.cookie_header_for_https("grok.example.test", "/chat", 2_000_000),
        Err(GrokWebBrowserEgressSessionError::ExpiredCredential)
    );
    assert_eq!(
        session.cookie_header_for_https("grok.example.test", "/chat", -1),
        Err(GrokWebBrowserEgressSessionError::InvalidObservationTime)
    );
    assert!(matches!(
        GrokWebBrowserEgressSession::try_new(
            GrokWebEgressSessionId::try_new("web_egress_02")?,
            initial_credential,
            GrokWebBrowserUserAgent::try_new(USER_AGENT)?,
            GrokWebTlsProfile::try_new("chrome_136_macos")?,
            UpstreamProxy::Direct,
            2_000_000,
        ),
        Err(GrokWebBrowserEgressSessionError::ExpiredCredential)
    ));
    assert!(matches!(
        GrokWebBrowserEgressSession::try_new(
            GrokWebEgressSessionId::try_new("web_egress_02")?,
            credential("web_account_01", "sso_import_01", 7, "session_value_alpha")?,
            GrokWebBrowserUserAgent::try_new(USER_AGENT)?,
            GrokWebTlsProfile::try_new("chrome_136_macos")?,
            UpstreamProxy::Direct,
            -1,
        ),
        Err(GrokWebBrowserEgressSessionError::InvalidObservationTime)
    ));
    assert_eq!(
        GrokWebBrowserUserAgent::try_new("Mozilla/5.0\r\nCookie: injected"),
        Err(GrokWebBrowserEgressSessionError::InvalidUserAgent)
    );
    assert_eq!(
        GrokWebTlsProfile::try_new("chrome profile"),
        Err(GrokWebBrowserEgressSessionError::InvalidTlsProfile)
    );
    Ok(())
}

#[test]
fn browser_egress_session_cannot_follow_account_lineage_or_revision_replacement()
-> Result<(), Box<dyn Error>> {
    let session = session(
        credential("web_account_01", "sso_import_01", 7, "session_value_alpha")?,
        UpstreamProxy::try_socks5("socks5://127.0.0.1:7897")?,
    )?;
    assert_eq!(
        session.require_current_credential(
            &credential("web_account_02", "sso_import_01", 7, "session_other")?,
            OBSERVED_AT_MS,
        ),
        Err(GrokWebBrowserEgressSessionError::AccountMismatch)
    );
    assert_eq!(
        session.require_current_credential(
            &credential("web_account_01", "sso_import_02", 7, "session_other")?,
            OBSERVED_AT_MS,
        ),
        Err(GrokWebBrowserEgressSessionError::LineageMismatch)
    );
    assert_eq!(
        session.require_current_credential(
            &credential("web_account_01", "sso_import_01", 8, "session_other")?,
            OBSERVED_AT_MS,
        ),
        Err(GrokWebBrowserEgressSessionError::RevisionMismatch)
    );
    let debug = format!("{session:?}");
    assert!(!debug.contains("127.0.0.1"));
    assert!(!debug.contains("7897"));
    Ok(())
}

fn session(
    credential: GrokWebCredential,
    proxy: UpstreamProxy,
) -> Result<GrokWebBrowserEgressSession, GrokWebBrowserEgressSessionError> {
    GrokWebBrowserEgressSession::try_new(
        GrokWebEgressSessionId::try_new("web_egress_01")?,
        credential,
        GrokWebBrowserUserAgent::try_new(USER_AGENT)?,
        GrokWebTlsProfile::try_new("chrome_136_macos")?,
        proxy,
        OBSERVED_AT_MS,
    )
}

fn credential(
    account_ref: &str,
    lineage_ref: &str,
    revision: u64,
    session_value: &str,
) -> Result<GrokWebCredential, provider_grok::GrokWebCredentialError> {
    let export = format!(
        r#"{{
            "kind":"grok_web_sso",
            "account_ref":"{account_ref}",
            "lineage_ref":"{lineage_ref}",
            "revision":{revision},
            "expires_at_ms":2000000,
            "cookies":[
                {{"name":"sso_session","value":"{session_value}","domain":".grok.example.test","path":"/","secure":true,"http_only":true}},
                {{"name":"csrf","value":"csrf_value","domain":"grok.example.test","path":"/chat","secure":true,"http_only":false}}
            ]
        }}"#
    );
    GrokWebCredential::import_sso_json(export.as_bytes(), OBSERVED_AT_MS)
}
