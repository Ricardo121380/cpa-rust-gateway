//! Synthetic transport, real HTTP continuity store, lease boundary and native Build codec.
use super::*;
use actix_web::{App, http::StatusCode, test, web};
use gateway_auth::{InMemoryClientKey, InMemoryClientKeyAuthenticator};
use gateway_core::{ResponseId, RouteCandidateId};
use gateway_router::SnapshotVersion;
use gateway_store::{
    secret_store::{KeyVersion, MasterKey, MasterKeyRing},
    stored_response::SqliteStoredResponseStore,
};
use gateway_upstream::{
    CredentialMaterialReplacement, CredentialSecret, EndpointCredentialInput,
    EndpointCredentialPool,
};
use provider_grok::{
    GrokBuildResponseBody, GrokBuildResponseContentEncoding, GrokBuildResponseContentType,
    GrokBuildResponsesOutboundRequest, GrokBuildTransport, GrokBuildTransportResponse,
};
use std::fmt::Write as _;

type TestResult = Result<(), Box<dyn Error>>;
const NOW: i64 = 10_000;
const SECRET: &[u8] = br#"{"access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_in":3600,"token_type":"Bearer"}"#;

struct Transport {
    bodies: Mutex<Vec<Value>>,
    streaming: bool,
}
impl GrokBuildTransport for Transport {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            let request: Value =
                serde_json::from_slice(request.body()).map_err(|_| internal_error())?;
            let mut bodies = self.bodies.lock().map_err(|_| internal_error())?;
            bodies.push(request);
            let id = format!("m2-native-{}", bodies.len());
            let response = serde_json::json!({"id":id,"object":"response","status":"completed","model":"grok-model","output":[
                {"id":"reason","type":"reasoning","summary":[{"type":"summary_text","text":"synthetic reasoning"}],"status":"completed"},
                {"id":"call-item","type":"function_call","call_id":"tool-call","name":"local_tool","arguments":"{}","status":"completed"}
            ],"usage":{"input_tokens":3,"output_tokens":4,"total_tokens":7}});
            let (content_type, bytes) = if self.streaming {
                let start = serde_json::json!({"type":"response.created","response":{"id":id,"status":"in_progress","output":[]}});
                let mut frames = format!("event: response.created\ndata: {start}\n\n");
                for (index, item) in response["output"]
                    .as_array()
                    .ok_or_else(internal_error)?
                    .iter()
                    .enumerate()
                {
                    for event in ["response.output_item.added", "response.output_item.done"] {
                        let value =
                            serde_json::json!({"type":event,"output_index":index,"item":item});
                        let _ = write!(frames, "event: {event}\ndata: {value}\n\n");
                    }
                }
                let end = serde_json::json!({"type":"response.completed","response":response});
                let _ = write!(frames, "event: response.completed\ndata: {end}\n\n");
                (
                    GrokBuildResponseContentType::EventStream,
                    frames.into_bytes(),
                )
            } else {
                (
                    GrokBuildResponseContentType::Json,
                    serde_json::to_vec(&response).map_err(|_| internal_error())?,
                )
            };
            Ok(GrokBuildTransportResponse::new(
                200,
                content_type,
                GrokBuildResponseContentEncoding::Identity,
                Box::new(Body { bytes: Some(bytes) }),
            ))
        })
    }
}
struct Body {
    bytes: Option<Vec<u8>>,
}
impl GrokBuildResponseBody for Body {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async { Ok(self.bytes.take()) })
    }
}
struct Source {
    inner: Box<dyn CanonicalEventSource>,
    _lease: CredentialLease,
}
impl ResponsesEventSource for Source {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        self.inner.next_event()
    }
}
struct Executor {
    pool: Arc<EndpointCredentialPool>,
    transport: Arc<Transport>,
}
impl ResponsesExecutor for Executor {
    fn supports_stored_response_lineage(&self) -> bool {
        true
    }
    fn supports_stored_response_continuity(&self) -> bool {
        true
    }
    fn execute(
        &self,
        _: RequestContext,
        _: CanonicalRequest,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        Box::pin(async { Err(internal_error()) })
    }
    fn execute_routed(
        &self,
        execution: ResponsesExecution,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        Box::pin(async move {
            let credential = CredentialId::try_new("m2-account").map_err(|_| internal_error())?;
            let lease = if let Some(pin) = execution.continuation_pin() {
                self.pool.try_lease_exact_revision_eligible_at(
                    pin.lineage().credential_id(),
                    pin.lineage().credential_revision(),
                    NOW,
                    |_| true,
                )
            } else {
                self.pool
                    .try_lease_exact_eligible_at(&credential, NOW, |_| true)
            }
            .ok_or_else(credential_unavailable_error)?;
            if let Some(recorder) = execution.lineage_recorder() {
                recorder.record(ResponsesExecutionLineage::new(
                    SnapshotVersion::try_new("m2-config").map_err(|_| internal_error())?,
                    ProviderId::try_new("m2-provider").map_err(|_| internal_error())?,
                    gateway_core::UpstreamId::try_new("m2-provider")
                        .map_err(|_| internal_error())?,
                    EndpointId::try_new("m2-channel").map_err(|_| internal_error())?,
                    RouteId::try_new("m2-route").map_err(|_| internal_error())?,
                    RouteCandidateId::try_new("m2-candidate").map_err(|_| internal_error())?,
                    credential,
                    lease.credential_revision(),
                ))?;
            }
            let mode = if self.transport.streaming {
                GrokBuildExecutionMode::Streaming
            } else {
                GrokBuildExecutionMode::NonStreaming
            };
            let adapter = GrokBuildInferenceAdapter::try_new(
                GrokBuildCredential::import_json(lease.secret_bytes(), NOW)
                    .map_err(|_| credential_unavailable_error())?,
                "grok-model",
                mode,
                self.transport.clone(),
            )?;
            let inner = adapter
                .execute(execution.context().clone(), execution.request().clone())
                .await?;
            Ok(Box::new(Source {
                inner,
                _lease: lease,
            }) as Box<dyn ResponsesEventSource>)
        })
    }
}
#[actix_web::test]
async fn native_build_http_history_survives_proven_rotation_in_json_and_sse() -> TestResult {
    for streaming in [false, true] {
        let credential = CredentialId::try_new("m2-account")?;
        let pool = Arc::new(EndpointCredentialPool::try_new(
            EndpointId::try_new("m2-channel")?,
            [EndpointCredentialInput {
                credential_id: credential.clone(),
                credential_kind: "grok_build_oauth".into(),
                credential_revision: 1,
                priority: 0,
                weight: 1,
                concurrency: 1,
                expires_at_ms: Some(1_000_000),
                secret: CredentialSecret::try_new(SECRET.to_vec())?,
            }],
        )?);
        let transport = Arc::new(Transport {
            bodies: Mutex::new(Vec::new()),
            streaming,
        });
        let version = KeyVersion::try_new(1)?;
        let store = Arc::new(SqliteStoredResponseStore::open_in_memory(
            SecretStore::new(MasterKeyRing::try_new(
                version,
                [(version, MasterKey::try_from_bytes([61; 32])?)],
            )?),
        )?);
        let auth = Arc::new(InMemoryClientKeyAuthenticator::try_new([
            InMemoryClientKey::try_new("m2-owner-secret", ClientKeyId::try_new("owner")?, true)?,
            InMemoryClientKey::try_new(
                "m2-foreign-secret",
                ClientKeyId::try_new("foreign")?,
                true,
            )?,
        ])?);
        let state = ResponsesHttpState::new(
            Arc::new(Executor {
                pool: pool.clone(),
                transport: transport.clone(),
            }),
            auth,
            default_stream_capacity()?,
        )
        .with_stored_response_store(store.clone());
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let first = test::TestRequest::post().uri("/v1/responses").insert_header(("authorization","Bearer m2-owner-secret")).set_json(serde_json::json!({"model":"grok-model","input":"synthetic first turn","store":true,"stream":streaming,"reasoning":{"effort":"low","summary":"auto"},"tools":[{"type":"function","name":"local_tool","parameters":{"type":"object","properties":{}}}]})).to_request();
        let response = test::call_service(&app, first).await;
        let status = response.status();
        let bytes = test::read_body(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "streaming={streaming}: {}",
            String::from_utf8_lossy(&bytes)
        );
        let now = system_now_ms_runtime()?;
        let persisted = store
            .get_owned(
                &ClientKeyId::try_new("owner")?,
                &ResponseId::try_new("m2-native-1")?,
                now,
            )?
            .ok_or("first native response not stored")?;
        assert_eq!(
            persisted
                .payload()
                .lineage()
                .credential()
                .credential_revision(),
            1
        );
        pool.replace_credential_if_revision(
            &credential,
            1,
            CredentialMaterialReplacement {
                credential_revision: 2,
                expires_at_ms: Some(1_000_000),
                secret: CredentialSecret::try_new(SECRET.to_vec())?,
            },
        )?;
        pool.set_build_continuation_range(&credential, 1, 2)?;
        let next_body = serde_json::json!({"model":"grok-model","input":[{"type":"function_call_output","call_id":"tool-call","output":"synthetic tool result"}],"previous_response_id":"m2-native-1","store":true,"stream":streaming,"reasoning":{"effort":"low","summary":"auto"}});
        let continuation = test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", "Bearer m2-owner-secret"))
            .set_json(&next_body)
            .to_request();
        let response = test::call_service(&app, continuation).await;
        let status = response.status();
        let bytes = test::read_body(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "streaming={streaming}: {}",
            String::from_utf8_lossy(&bytes)
        );
        {
            let recorded = transport.bodies.lock().map_err(|_| "capture")?;
            assert_eq!(recorded.len(), 2);
            let replay = recorded[1].to_string();
            for expected in [
                "synthetic first turn",
                "synthetic tool result",
                "synthetic reasoning",
                "local_tool",
                "function_call_output",
            ] {
                assert!(replay.contains(expected), "missing {expected}");
            }
            assert!(!replay.contains("previous_response_id"));
        }
        let continued = store
            .get_owned(
                &ClientKeyId::try_new("owner")?,
                &ResponseId::try_new("m2-native-2")?,
                now,
            )?
            .ok_or("continued response not stored")?;
        assert_eq!(
            continued
                .payload()
                .lineage()
                .credential()
                .credential_revision(),
            2
        );
        let foreign = test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", "Bearer m2-foreign-secret"))
            .set_json(&next_body)
            .to_request();
        assert_eq!(
            test::call_service(&app, foreign).await.status(),
            StatusCode::NOT_FOUND
        );
        pool.replace_credential_if_revision(
            &credential,
            2,
            CredentialMaterialReplacement {
                credential_revision: 3,
                expires_at_ms: Some(1_000_000),
                secret: CredentialSecret::try_new(SECRET.to_vec())?,
            },
        )?;
        let replaced = test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", "Bearer m2-owner-secret"))
            .set_json(&next_body)
            .to_request();
        assert_eq!(
            test::call_service(&app, replaced).await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(transport.bodies.lock().map_err(|_| "capture")?.len(), 2);
        assert_eq!(pool.active_lease_count(&credential), Some(0));
    }
    Ok(())
}
