//! P13-11E E2 local Build/Console exact-lease and bounded adapter evidence.

#![deny(unsafe_code)]

use std::{collections::VecDeque, error::Error, sync::Arc};

use gateway_core::{
    CanonicalRequest, CredentialId, EndpointId, GatewayError, ProviderId, UpstreamId,
};
use gateway_provider::{InferenceAdapter, ProviderFuture};
use gateway_router::{
    ProviderAccountEvidence, ProviderChannelCapability, ProviderChannelCapabilityRegistry,
    ProviderChannelIdentity, ProviderEgressChannel, ProviderEgressFailureEvidence,
    ProviderEgressRuntime, ProviderEgressRuntimeState, ProviderEgressStateKey,
    ProviderEgressTargetIdentity, ProviderSessionRuntimeState, ProviderSessionStateKey,
};
use gateway_upstream::{CredentialSecret, EndpointCredentialInput, EndpointCredentialPool};
use protocol_openai_responses::decode_request;
use provider_grok::{
    GrokBuildCredential, GrokBuildExecutionMode, GrokBuildInferenceAdapter, GrokBuildResponseBody,
    GrokBuildResponseContentEncoding, GrokBuildResponseContentType,
    GrokBuildResponsesOutboundRequest, GrokBuildTransport, GrokBuildTransportResponse,
    GrokConsoleExecutionMode, GrokConsoleInferenceAdapter, GrokConsoleResponseBody,
    GrokConsoleResponseContentType, GrokConsoleResponsesOutboundRequest, GrokConsoleSsoToken,
    GrokConsoleTransport, GrokConsoleTransportResponse, GrokNativeEgressAttempt,
    GrokNativeEgressAttemptError, GrokNativeEgressClock,
};

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 10_000;

#[derive(Clone, Copy, Debug)]
struct FixedClock(i64);

impl GrokNativeEgressClock for FixedClock {
    fn now_ms(&self) -> Result<i64, GrokNativeEgressAttemptError> {
        Ok(self.0)
    }
}

#[tokio::test]
async fn build_adapter_uses_exact_lease_once_and_closes_after_semantic_event() -> TestResult {
    let endpoint = EndpointId::try_new("e2-build-endpoint")?;
    let upstream = UpstreamId::try_new("e2-build-upstream")?;
    let credential_id = CredentialId::try_new("e2-build-account")?;
    let identity = identity("grok.build", &upstream, &endpoint)?;
    let runtime = runtime_with_capability(identity.clone(), ProviderEgressChannel::GrokBuild)?;
    runtime.set_egress_state(
        ProviderEgressStateKey::new(identity.clone(), ProviderEgressTargetIdentity::Direct),
        ProviderEgressRuntimeState::Available,
        NOW_MS,
    )?;
    let pool = EndpointCredentialPool::try_new(
        endpoint,
        [EndpointCredentialInput {
            credential_id,
            credential_kind: "grok_build_oauth".to_owned(),
            credential_revision: 7,
            priority: 0,
            weight: 1,
            concurrency: 1,
            expires_at_ms: None,
            secret: CredentialSecret::try_new(build_secret_json())?,
        }],
    )?;
    let lease = pool.try_lease().ok_or("Build exact lease unavailable")?;
    let attempt = GrokNativeEgressAttempt::try_new_build(
        Arc::clone(&runtime),
        identity,
        ProviderEgressTargetIdentity::Direct,
        &lease,
        Arc::new(FixedClock(NOW_MS)),
    )?;
    let transport = Arc::new(BuildFixtureTransport::new(include_bytes!(
        "../../../tests/fixtures/grok-build/p6-03-non-streaming.json"
    )));
    let adapter = GrokBuildInferenceAdapter::try_new(
        GrokBuildCredential::import_json(build_secret_json(), 0)?,
        "grok-4.5-build",
        GrokBuildExecutionMode::NonStreaming,
        transport.clone(),
    )?
    .with_provider_egress_attempt(Arc::new(attempt.clone()));
    let mut source = adapter.execute(request_context()?, request()?).await?;
    while source.next_event().await?.is_some() {}

    let snapshot = attempt.snapshot()?;
    assert_eq!(snapshot.channel(), ProviderEgressChannel::GrokBuild);
    assert_eq!(snapshot.credential_revision(), 7);
    assert_eq!(snapshot.auxiliary_requests(), 0);
    assert!(snapshot.inference_submitted());
    assert!(snapshot.semantic_event_observed());
    assert_eq!(transport.calls(), 1);
    assert!(attempt.record_inference_submission().is_err());
    drop(lease);
    assert_eq!(pool.diagnostic_entries()[0].active_leases(), 0);
    Ok(())
}

#[tokio::test]
async fn console_adapter_uses_active_session_and_one_inference() -> TestResult {
    let endpoint = EndpointId::try_new("e2-console-endpoint")?;
    let upstream = UpstreamId::try_new("e2-console-upstream")?;
    let credential_id = CredentialId::try_new("e2-console-account")?;
    let identity = identity("grok.console", &upstream, &endpoint)?;
    let runtime = runtime_with_capability(identity.clone(), ProviderEgressChannel::GrokConsole)?;
    runtime.set_egress_state(
        ProviderEgressStateKey::new(identity.clone(), ProviderEgressTargetIdentity::Direct),
        ProviderEgressRuntimeState::Available,
        NOW_MS,
    )?;
    let session_key =
        ProviderSessionStateKey::try_new(identity.clone(), credential_id.clone(), 9, 9)?;
    runtime.set_session_state(
        session_key,
        ProviderSessionRuntimeState::Active {
            expires_at_ms: NOW_MS + 60_000,
        },
        NOW_MS,
    )?;
    let pool = EndpointCredentialPool::try_new(
        endpoint,
        [EndpointCredentialInput {
            credential_id,
            credential_kind: "grok_console_sso".to_owned(),
            credential_revision: 9,
            priority: 0,
            weight: 1,
            concurrency: 1,
            expires_at_ms: None,
            secret: CredentialSecret::try_new(b"synthetic-console-sso".to_vec())?,
        }],
    )?;
    let lease = pool.try_lease().ok_or("Console exact lease unavailable")?;
    let attempt = GrokNativeEgressAttempt::try_new_console(
        Arc::clone(&runtime),
        identity,
        ProviderEgressTargetIdentity::Direct,
        &lease,
        9,
        Arc::new(FixedClock(NOW_MS)),
    )?;
    let transport = Arc::new(ConsoleFixtureTransport::new(console_json()));
    let adapter = GrokConsoleInferenceAdapter::try_new(
        GrokConsoleSsoToken::try_from_bytes(b"synthetic-console-sso")?,
        "grok-4.3",
        GrokConsoleExecutionMode::NonStreaming,
        transport.clone(),
    )?
    .with_provider_egress_attempt(Arc::new(attempt.clone()));
    let mut source = adapter.execute(request_context()?, request()?).await?;
    while source.next_event().await?.is_some() {}

    let snapshot = attempt.snapshot()?;
    assert_eq!(snapshot.channel(), ProviderEgressChannel::GrokConsole);
    assert_eq!(snapshot.auxiliary_requests(), 0);
    assert_eq!(snapshot.pre_submit_recoveries(), 0);
    assert!(snapshot.inference_submitted());
    assert!(snapshot.semantic_event_observed());
    assert_eq!(transport.calls(), 1);
    assert!(attempt.record_inference_submission().is_err());
    Ok(())
}

#[test]
fn console_bootstrap_budget_and_failure_ownership_stay_exact() -> TestResult {
    let endpoint = EndpointId::try_new("e2-console-isolation")?;
    let upstream = UpstreamId::try_new("e2-console-isolation-upstream")?;
    let credential_id = CredentialId::try_new("e2-console-isolation-account")?;
    let identity = identity("grok.console", &upstream, &endpoint)?;
    let runtime = runtime_with_capability(identity.clone(), ProviderEgressChannel::GrokConsole)?;
    runtime.set_egress_state(
        ProviderEgressStateKey::new(identity.clone(), ProviderEgressTargetIdentity::Direct),
        ProviderEgressRuntimeState::Available,
        NOW_MS,
    )?;
    let session_key =
        ProviderSessionStateKey::try_new(identity.clone(), credential_id.clone(), 11, 11)?;
    runtime.set_session_state(session_key, ProviderSessionRuntimeState::Absent, NOW_MS)?;
    let pool = EndpointCredentialPool::try_new(
        endpoint,
        [EndpointCredentialInput {
            credential_id,
            credential_kind: "grok_console_sso".to_owned(),
            credential_revision: 11,
            priority: 0,
            weight: 1,
            concurrency: 1,
            expires_at_ms: None,
            secret: CredentialSecret::try_new(b"synthetic-console-sso".to_vec())?,
        }],
    )?;
    let lease = pool
        .try_lease()
        .ok_or("Console bootstrap lease unavailable")?;
    let attempt = GrokNativeEgressAttempt::try_new_console(
        Arc::clone(&runtime),
        identity.clone(),
        ProviderEgressTargetIdentity::Direct,
        &lease,
        11,
        Arc::new(FixedClock(NOW_MS)),
    )?;
    attempt.begin_console_session_bootstrap()?;
    attempt.complete_console_session_bootstrap(NOW_MS + 30_000)?;
    let first = attempt.snapshot()?;
    assert_eq!(first.auxiliary_requests(), 1);
    assert_eq!(first.pre_submit_recoveries(), 0);
    assert_eq!(
        attempt
            .classify_failure(ProviderEgressFailureEvidence::HttpForbidden {
                account_evidence: ProviderAccountEvidence::None,
            })?
            .owner(),
        gateway_router::ProviderEgressFailureOwner::AmbiguousProvider
    );
    assert_eq!(
        attempt
            .classify_failure(ProviderEgressFailureEvidence::HttpForbidden {
                account_evidence: ProviderAccountEvidence::ConfirmedForbidden,
            })?
            .action(),
        gateway_router::ProviderEgressRecoveryAction::RequireCredentialReplacement
    );
    drop(lease);
    Ok(())
}

#[test]
fn build_and_console_namespaces_cannot_cross_use_each_other() -> TestResult {
    let endpoint = EndpointId::try_new("e2-cross-endpoint")?;
    let upstream = UpstreamId::try_new("e2-cross-upstream")?;
    let build_identity = identity("grok.build", &upstream, &endpoint)?;
    let console_identity = identity("grok.console", &upstream, &endpoint)?;
    let build_runtime =
        runtime_with_capability(build_identity.clone(), ProviderEgressChannel::GrokBuild)?;
    build_runtime.set_egress_state(
        ProviderEgressStateKey::new(build_identity.clone(), ProviderEgressTargetIdentity::Direct),
        ProviderEgressRuntimeState::Available,
        NOW_MS,
    )?;
    let pool = EndpointCredentialPool::try_new(
        endpoint,
        [EndpointCredentialInput {
            credential_id: CredentialId::try_new("e2-cross-account")?,
            credential_kind: "grok_build_oauth".to_owned(),
            credential_revision: 1,
            priority: 0,
            weight: 1,
            concurrency: 1,
            expires_at_ms: None,
            secret: CredentialSecret::try_new(build_secret_json())?,
        }],
    )?;
    let lease = pool
        .try_lease()
        .ok_or("cross namespace lease unavailable")?;
    assert!(
        GrokNativeEgressAttempt::try_new_console(
            build_runtime,
            console_identity,
            ProviderEgressTargetIdentity::Direct,
            &lease,
            1,
            Arc::new(FixedClock(NOW_MS)),
        )
        .is_err()
    );
    Ok(())
}

fn identity(
    provider: &str,
    upstream: &UpstreamId,
    endpoint: &EndpointId,
) -> Result<ProviderChannelIdentity, Box<dyn Error>> {
    Ok(ProviderChannelIdentity::try_new(
        ProviderId::try_new(provider.to_owned())?,
        upstream.clone(),
        endpoint.clone(),
    )?)
}

fn runtime_with_capability(
    identity: ProviderChannelIdentity,
    channel: ProviderEgressChannel,
) -> Result<Arc<ProviderEgressRuntime>, Box<dyn Error>> {
    let registry =
        ProviderChannelCapabilityRegistry::try_new(vec![ProviderChannelCapability::new(
            identity, channel,
        )])?;
    Ok(Arc::new(ProviderEgressRuntime::new(registry)))
}

fn request_context() -> Result<gateway_core::RequestContext, Box<dyn Error>> {
    Ok(gateway_core::RequestContext::new(
        gateway_core::RequestId::try_new("e2-native-request")?,
    ))
}

fn request() -> Result<CanonicalRequest, Box<dyn Error>> {
    Ok(decode_request(
        r#"{"model":"e2-model","input":"Reply with exactly: ready","max_output_tokens":8}"#,
    )?
    .request)
}

fn build_secret_json() -> &'static [u8] {
    br#"{"access_token":"synthetic-build-access-012345","refresh_token":"synthetic-build-refresh-012345","expires_in":3600,"token_type":"Bearer"}"#
}

fn console_json() -> &'static [u8] {
    br#"{"id":"e2-console-response","status":"completed","output":[{"id":"e2-message","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"ready"}]}],"usage":{"input_tokens":1,"output_tokens":1}}"#
}

struct BuildFixtureTransport {
    body: Vec<u8>,
    calls: std::sync::atomic::AtomicUsize,
}

impl BuildFixtureTransport {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::Acquire)
    }
}

impl GrokBuildTransport for BuildFixtureTransport {
    fn send(
        &self,
        _request: GrokBuildResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let body = self.body.clone();
        Box::pin(async move {
            Ok(GrokBuildTransportResponse::new(
                200,
                GrokBuildResponseContentType::Json,
                GrokBuildResponseContentEncoding::Identity,
                Box::new(BytesBody::new(body)),
            ))
        })
    }
}

struct ConsoleFixtureTransport {
    body: Vec<u8>,
    calls: std::sync::atomic::AtomicUsize,
}

impl ConsoleFixtureTransport {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::Acquire)
    }
}

impl GrokConsoleTransport for ConsoleFixtureTransport {
    fn send(
        &self,
        _request: GrokConsoleResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokConsoleTransportResponse, GatewayError>> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let body = self.body.clone();
        Box::pin(async move {
            Ok(GrokConsoleTransportResponse::new(
                200,
                GrokConsoleResponseContentType::Json,
                Box::new(BytesBody::new(body)),
            ))
        })
    }
}

struct BytesBody {
    chunks: VecDeque<Vec<u8>>,
}

impl BytesBody {
    fn new(body: Vec<u8>) -> Self {
        Self {
            chunks: VecDeque::from([body]),
        }
    }
}

impl GrokBuildResponseBody for BytesBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move { Ok(self.chunks.pop_front()) })
    }
}

impl GrokConsoleResponseBody for BytesBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move { Ok(self.chunks.pop_front()) })
    }
}
