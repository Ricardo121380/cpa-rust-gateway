//! P12-10I-14 native Web inference, dynamic Statsig refresh, and live decode evidence.

#![deny(unsafe_code)]

use std::{
    collections::VecDeque,
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use gateway_core::{
    CanonicalEvent, ErrorScope, GatewayError, GatewayErrorCode, RequestContext, RequestId,
};
use gateway_provider::{InferenceAdapter, ProviderFuture};
use gateway_upstream::UpstreamProxy;
use protocol_openai_responses::decode_request;
use provider_grok::{
    GrokWebBrowserEgressSession, GrokWebBrowserUserAgent, GrokWebCredential,
    GrokWebEgressRefresher, GrokWebEgressSessionId, GrokWebFlareSolverrClearance,
    GrokWebProductionInferenceAdapter, GrokWebProductionOutboundRequest,
    GrokWebProductionResponseBody, GrokWebProductionTransport, GrokWebProductionTransportResponse,
    GrokWebStatsigRuntime, GrokWebStatsigSignature, GrokWebStatsigTransport, GrokWebTlsProfile,
};
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn Error>>;

#[tokio::test]
async fn pre_start_403_refreshes_statsig_once_then_projects_the_live_stream() -> TestResult {
    let now_ms = current_ms()?;
    let statsig_transport = Arc::new(FixtureStatsigTransport::default());
    let statsig = Arc::new(GrokWebStatsigRuntime::try_new(statsig_transport.clone())?);
    let inference_transport = Arc::new(FixtureInferenceTransport::default());
    let session = Arc::new(web_session(now_ms)?);
    let egress_refresher = Arc::new(FixtureEgressRefresher {
        calls: AtomicUsize::new(0),
        session: Arc::clone(&session),
    });
    let adapter = GrokWebProductionInferenceAdapter::try_new(
        session,
        "grok-chat-fast",
        statsig,
        inference_transport.clone(),
    )?
    .with_egress_refresher(egress_refresher.clone());
    let request = decode_request(r#"{"model":"public","input":"ready"}"#)?.request;
    let mut source = adapter
        .execute(
            RequestContext::new(RequestId::try_new("p12-10i-web-runtime")?),
            request,
        )
        .await?;
    let mut events = Vec::new();
    while let Some(event) = source.next_event().await? {
        events.push(event);
    }

    assert_eq!(inference_transport.calls.load(Ordering::SeqCst), 2);
    assert_eq!(egress_refresher.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        statsig_transport.environment_calls.load(Ordering::SeqCst),
        2
    );
    assert_eq!(statsig_transport.sign_calls.load(Ordering::SeqCst), 2);
    assert!(matches!(
        events.first(),
        Some(CanonicalEvent::ResponseStart(_))
    ));
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::ResponseEnd(_))
    ));
    assert!(
        events.iter().any(
            |event| matches!(event, CanonicalEvent::TextDelta(delta) if delta.text == "ready")
        )
    );
    assert!(events.iter().any(|event| matches!(
        event,
        CanonicalEvent::UsageDelta(delta)
            if delta.is_final
                && delta.usage.input_tokens.is_some_and(|value| value > 0)
                && delta.usage.output_tokens.is_some_and(|value| value > 0)
    )));
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::ResponseEnd(end))
            if end.stop_reason.as_deref() == Some("end_turn")
    ));
    Ok(())
}

#[tokio::test]
async fn statsig_egress_rejection_falls_back_to_unsigned_clearance_retry() -> TestResult {
    let now_ms = current_ms()?;
    let statsig_transport = Arc::new(RejectingStatsigTransport::default());
    let statsig = Arc::new(GrokWebStatsigRuntime::try_new(statsig_transport.clone())?);
    let inference_transport = Arc::new(UnsignedRetryTransport::default());
    let session = Arc::new(web_session(now_ms)?);
    let egress_refresher = Arc::new(FixtureEgressRefresher {
        calls: AtomicUsize::new(0),
        session: Arc::clone(&session),
    });
    let adapter = GrokWebProductionInferenceAdapter::try_new(
        session,
        "grok-chat-fast",
        statsig,
        inference_transport.clone(),
    )?
    .with_egress_refresher(egress_refresher.clone());
    let request = decode_request(r#"{"model":"public","input":"ready"}"#)?.request;
    let mut source = adapter
        .execute(
            RequestContext::new(RequestId::try_new("p12-10i-web-unsigned-retry")?),
            request,
        )
        .await?;
    let mut events = Vec::new();
    while let Some(event) = source.next_event().await? {
        events.push(event);
    }

    assert_eq!(statsig_transport.calls.load(Ordering::SeqCst), 2);
    assert_eq!(inference_transport.calls.load(Ordering::SeqCst), 2);
    assert_eq!(egress_refresher.calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::ResponseEnd(_))
    ));
    Ok(())
}

#[test]
fn flaresolverr_clearance_rebuild_preserves_account_binding() -> TestResult {
    let now_ms = current_ms()?;
    let session = web_session(now_ms)?;
    let clearance = GrokWebFlareSolverrClearance::parse(
        br#"{"status":"ok","solution":{"userAgent":"Chrome","cookies":[{"name":"cf_clearance","value":"fresh"}]}}"#,
    )?;
    let before = session.credential_snapshot();
    let refreshed = before.with_flaresolverr_clearance(&clearance, now_ms)?;
    assert_eq!(refreshed.account_reference(), before.account_reference());
    assert_eq!(refreshed.lineage(), before.lineage());
    assert_eq!(refreshed.revision(), before.revision() + 1);
    assert!(
        refreshed
            .cookies()
            .iter()
            .any(|cookie| cookie.name() == "cf_clearance" && cookie.value() == "fresh")
    );
    Ok(())
}

struct FixtureEgressRefresher {
    calls: AtomicUsize,
    session: Arc<GrokWebBrowserEgressSession>,
}

impl GrokWebEgressRefresher for FixtureEgressRefresher {
    fn refresh<'a>(
        &'a self,
        current: &'a GrokWebBrowserEgressSession,
    ) -> ProviderFuture<'a, Result<Arc<GrokWebBrowserEgressSession>, GatewayError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // The fixture keeps the exact account/session binding. Production
            // implementations may return a newly rebuilt session with refreshed
            // cookies or a validated proxy, but never a different account.
            assert_eq!(
                current.egress_session_id(),
                self.session.egress_session_id()
            );
            Ok(Arc::clone(&self.session))
        })
    }
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
            GrokWebStatsigSignature::try_new(&format!("fixture-signature-{sequence}"))
                .map_err(|_| internal_error())
        })
    }
}

#[derive(Default)]
struct RejectingStatsigTransport {
    calls: AtomicUsize,
}

impl GrokWebStatsigTransport for RejectingStatsigTransport {
    fn fetch_environment<'a>(
        &'a self,
        _: &'a GrokWebBrowserEgressSession,
        _: i64,
    ) -> ProviderFuture<'a, Result<Zeroizing<String>, GatewayError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(GatewayError::new(
                GatewayErrorCode::EgressRejected,
                ErrorScope::Egress,
            ))
        })
    }

    fn sign<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        _: &'a str,
    ) -> ProviderFuture<'a, Result<GrokWebStatsigSignature, GatewayError>> {
        Box::pin(async { Err(internal_error()) })
    }
}

#[derive(Default)]
struct FixtureInferenceTransport {
    calls: AtomicUsize,
}

impl GrokWebProductionTransport for FixtureInferenceTransport {
    fn send(
        &self,
        request: GrokWebProductionOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokWebProductionTransportResponse, GatewayError>> {
        assert!(request.header("x-statsig-id").is_some());
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if call == 0 {
                return Ok(GrokWebProductionTransportResponse::new(
                    403,
                    Box::new(FixtureBody::new(vec![b"forbidden".to_vec()])),
                ));
            }
            Ok(GrokWebProductionTransportResponse::new(
                200,
                Box::new(FixtureBody::new(
                    concat!(
                        "{\"result\":{\"conversation\":{\"conversationId\":\"conv-i14\"}}}",
                        "{\"result\":{\"response\":{\"token\":\"ready\",\"isThinking\":false}}}",
                        "{\"result\":{\"response\":{\"modelResponse\":{\"message\":\"ready\"}}}}"
                    )
                    .as_bytes()
                    .chunks(7)
                    .map(ToOwned::to_owned)
                    .collect(),
                )),
            ))
        })
    }
}

#[derive(Default)]
struct UnsignedRetryTransport {
    calls: AtomicUsize,
}

impl GrokWebProductionTransport for UnsignedRetryTransport {
    fn send(
        &self,
        request: GrokWebProductionOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokWebProductionTransportResponse, GatewayError>> {
        assert!(request.header("x-statsig-id").is_none());
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if call == 0 {
                return Ok(GrokWebProductionTransportResponse::new(
                    403,
                    Box::new(FixtureBody::new(vec![b"forbidden".to_vec()])),
                ));
            }
            Ok(GrokWebProductionTransportResponse::new(
                200,
                Box::new(FixtureBody::new(
                    concat!(
                        "{\"result\":{\"conversation\":{\"conversationId\":\"conv-unsigned\"}}}",
                        "{\"result\":{\"response\":{\"token\":\"ready\",\"isThinking\":false}}}",
                        "{\"result\":{\"response\":{\"modelResponse\":{\"message\":\"ready\"}}}}"
                    )
                    .as_bytes()
                    .chunks(7)
                    .map(ToOwned::to_owned)
                    .collect(),
                )),
            ))
        })
    }
}

struct FixtureBody {
    chunks: VecDeque<Vec<u8>>,
}

impl FixtureBody {
    fn new(chunks: Vec<Vec<u8>>) -> Self {
        Self {
            chunks: chunks.into(),
        }
    }
}

impl GrokWebProductionResponseBody for FixtureBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move { Ok(self.chunks.pop_front()) })
    }
}

fn web_session(now_ms: i64) -> Result<GrokWebBrowserEgressSession, Box<dyn Error>> {
    let credential = GrokWebCredential::import_sso_json(
        serde_json::json!({
            "kind":"grok_web_sso",
            "account_ref":"web-inference-account",
            "lineage_ref":"web-inference-lineage",
            "revision":1,
            "expires_at_ms":now_ms + 60_000,
            "cookies":[{
                "name":"sso", "value":"fixture-cookie", "domain":"grok.com", "path":"/",
                "secure":true, "http_only":true
            }]
        })
        .to_string()
        .as_bytes(),
        now_ms,
    )?;
    Ok(GrokWebBrowserEgressSession::try_new(
        GrokWebEgressSessionId::try_new("web-inference-session")?,
        credential,
        GrokWebBrowserUserAgent::try_new("Mozilla/5.0 Chrome/146.0.0.0")?,
        GrokWebTlsProfile::try_new("chrome_146")?,
        UpstreamProxy::Direct,
        now_ms,
    )?)
}

fn current_ms() -> Result<i64, Box<dyn Error>> {
    Ok(i64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(
        gateway_core::GatewayErrorCode::InternalError,
        gateway_core::ErrorScope::Internal,
    )
}
