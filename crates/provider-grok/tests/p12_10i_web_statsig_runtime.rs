//! P12-10I-14 frozen grok2api Web Statsig runtime parity evidence.

#![deny(unsafe_code)]

use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use gateway_core::GatewayError;
use gateway_provider::ProviderFuture;
use gateway_upstream::UpstreamProxy;
use provider_grok::{
    GrokWebBrowserEgressSession, GrokWebBrowserUserAgent, GrokWebCredential,
    GrokWebEgressSessionId, GrokWebStatsigRuntime, GrokWebStatsigSignature,
    GrokWebStatsigTransport, GrokWebTlsProfile,
};
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 1_800_000_000_000;

#[tokio::test]
async fn cache_is_singleflight_and_a_403_refreshes_only_the_current_signature() -> TestResult {
    let transport = Arc::new(FixtureStatsigTransport::default());
    let runtime = GrokWebStatsigRuntime::try_new(transport.clone())?;
    let session = web_session()?;

    let (first, second) = tokio::join!(
        runtime.signature(&session, NOW_MS),
        runtime.signature(&session, NOW_MS)
    );
    let first = first?;
    let second = second?;
    assert_eq!(first.as_str(), "fixture-signature-1");
    assert_eq!(second.as_str(), "fixture-signature-1");
    assert_eq!(transport.environment_calls.load(Ordering::SeqCst), 1);
    assert_eq!(transport.sign_calls.load(Ordering::SeqCst), 1);

    assert!(runtime.invalidate_after_403()?);
    assert!(!runtime.invalidate_after_403()?);
    let replacement = runtime.signature(&session, NOW_MS + 1).await?;
    assert_eq!(replacement.as_str(), "fixture-signature-2");
    assert_eq!(transport.environment_calls.load(Ordering::SeqCst), 2);
    assert_eq!(transport.sign_calls.load(Ordering::SeqCst), 2);
    assert!(!runtime.invalidate_signature_after_403(&first)?);
    assert_eq!(
        runtime.signature(&session, NOW_MS + 2).await?.as_str(),
        replacement.as_str()
    );
    assert_eq!(transport.sign_calls.load(Ordering::SeqCst), 2);

    let debug = format!("{runtime:?} {replacement:?}");
    assert!(!debug.contains("fixture-environment"));
    assert!(!debug.contains("fixture-signature"));
    Ok(())
}

#[derive(Default)]
struct FixtureStatsigTransport {
    environment_calls: AtomicUsize,
    sign_calls: AtomicUsize,
}

impl GrokWebStatsigTransport for FixtureStatsigTransport {
    fn fetch_environment<'a>(
        &'a self,
        _: &'a GrokWebBrowserEgressSession,
        _: i64,
    ) -> ProviderFuture<'a, Result<Zeroizing<String>, GatewayError>> {
        Box::pin(async move {
            let sequence = self.environment_calls.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(Zeroizing::new(format!("fixture-environment-{sequence}")))
        })
    }

    fn sign<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        _: &'a str,
    ) -> ProviderFuture<'a, Result<GrokWebStatsigSignature, GatewayError>> {
        Box::pin(async move {
            assert_eq!(method, "POST");
            assert_eq!(path, "/rest/app-chat/conversations/new");
            let sequence = self.sign_calls.fetch_add(1, Ordering::SeqCst) + 1;
            GrokWebStatsigSignature::try_new(&format!("fixture-signature-{sequence}")).map_err(
                |_| {
                    GatewayError::new(
                        gateway_core::GatewayErrorCode::InternalError,
                        gateway_core::ErrorScope::Internal,
                    )
                },
            )
        })
    }
}

fn web_session() -> Result<GrokWebBrowserEgressSession, Box<dyn Error>> {
    let credential = GrokWebCredential::import_sso_json(
        serde_json::json!({
            "kind":"grok_web_sso",
            "account_ref":"web-runtime-account",
            "lineage_ref":"web-runtime-lineage",
            "revision":1,
            "expires_at_ms":NOW_MS + 60_000,
            "cookies":[{
                "name":"sso", "value":"fixture-cookie", "domain":"grok.com", "path":"/",
                "secure":true, "http_only":true
            }]
        })
        .to_string()
        .as_bytes(),
        NOW_MS,
    )?;
    Ok(GrokWebBrowserEgressSession::try_new(
        GrokWebEgressSessionId::try_new("web-runtime-session")?,
        credential,
        GrokWebBrowserUserAgent::try_new("Mozilla/5.0 Chrome/146.0.0.0")?,
        GrokWebTlsProfile::try_new("chrome_146")?,
        UpstreamProxy::Direct,
        NOW_MS,
    )?)
}
