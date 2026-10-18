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
                {"id":"reason","type":"reasoning","summary":[{"type":"summary_text","text":"synthetic reasoning"}],"encrypted_content":"synthetic-cipher","status":"completed"},
                {"id":"call-item","type":"function_call","call_id":"tool-call","name":"local_tool","arguments":"{}","status":"completed"}
            ],"usage":{"input_tokens":3,"output_tokens":4,"total_tokens":7}});
            let (content_type, bytes) = if self.streaming {
                let start = serde_json::json!({"type":"response.created","response":{"id":id,"model":"grok-model","status":"in_progress","output":[]}});
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
    codec: provider_grok::GrokBuildReasoningCodec,
    pool: Arc<EndpointCredentialPool>,
    transport: Arc<Transport>,
    credential: CredentialId,
    now: i64,
}

fn durable_state(
    pool: Arc<EndpointCredentialPool>,
    credential: CredentialId,
    transport: Arc<Transport>,
    secrets: &SecretStore,
    store: Arc<SqliteStoredResponseStore>,
) -> TestResultState {
    let auth = Arc::new(InMemoryClientKeyAuthenticator::try_new([
        InMemoryClientKey::try_new("m2-owner-secret", ClientKeyId::try_new("owner")?, true)?,
        InMemoryClientKey::try_new("m2-foreign-secret", ClientKeyId::try_new("foreign")?, true)?,
    ])?);
    Ok(ResponsesHttpState::new(
        Arc::new(Executor {
            codec: provider_grok::GrokBuildReasoningCodec::new(secrets.clone()),
            pool,
            credential,
            transport,
            now: system_now_ms_runtime()?,
        }),
        auth,
        default_stream_capacity()?,
    )
    .with_stored_response_store(store))
}
type TestResultState = Result<ResponsesHttpState, Box<dyn Error>>;

/// Joins the production refresh worker to HTTP history, reopened encrypted stores and native pools.
/// The caller owns the fixed-target loopback worker; this helper never manufactures a proof range.
#[allow(
    clippy::too_many_lines,
    reason = "one receipt chain proves JSON/SSE roots, durable refresh, reload and zero-send rejection"
)]
pub(crate) async fn verify_worker_generated_continuity<F>(
    database: &std::path::Path,
    secrets: &SecretStore,
    original: Arc<EndpointCredentialPool>,
    account: &str,
    refresh: F,
) -> TestResult
where
    F: FnOnce() -> TestResult,
{
    let credential = CredentialId::try_new(account)?;
    let mut scenarios = Vec::new();
    for streaming in [false, true] {
        let path = database.with_file_name(format!("responses-{streaming}.sqlite3"));
        let store = Arc::new(SqliteStoredResponseStore::open(&path, secrets.clone())?);
        let transport = Arc::new(Transport {
            bodies: Mutex::new(Vec::new()),
            streaming,
        });
        let state = durable_state(
            original.clone(),
            credential.clone(),
            transport.clone(),
            secrets,
            store.clone(),
        )?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let request=test::TestRequest::post().uri("/v1/responses").insert_header(("authorization","Bearer m2-owner-secret")).set_json(serde_json::json!({"model":"grok-model","input":"durable first turn","store":true,"stream":streaming,"reasoning":{"effort":"low","summary":"auto"},"tools":[{"type":"function","name":"local_tool","parameters":{"type":"object"}}]})).to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        assert!(String::from_utf8_lossy(&body).contains(if streaming {
            "response.completed"
        } else {
            "completed"
        }));
        let root = store
            .get_owned(
                &ClientKeyId::try_new("owner")?,
                &ResponseId::try_new("m2-native-1")?,
                system_now_ms_runtime()?,
            )?
            .ok_or("durable root")?;
        assert_eq!(
            root.payload().lineage().credential().credential_revision(),
            1
        );
        assert_eq!(original.active_lease_count(&credential), Some(0));
        scenarios.push((streaming, path, transport));
    }
    refresh()?;
    // All objects used to read response history and native proof below are newly opened.
    let native = provider_grok::GrokAccountPoolStore::try_open(database, secrets.clone())?;
    assert_eq!(
        native.build_continuation_ranges()?.get(account).copied(),
        Some((1, 2))
    );
    let binding = provider_grok::GrokAccountEndpointBinding::new(
        provider_grok::GrokAccountProvider::Build,
        original.endpoint_id().clone(),
    );
    let compiled =
        native.compile_native_runtime(std::slice::from_ref(&binding), system_now_ms_runtime()?)?;
    let pools = compiled.credential_pools();
    let rotated = pools
        .pool(original.endpoint_id())
        .ok_or("reloaded pool")?
        .clone();
    for (streaming, path, transport) in scenarios {
        let store = Arc::new(SqliteStoredResponseStore::open(&path, secrets.clone())?);
        let state = durable_state(
            rotated.clone(),
            credential.clone(),
            transport.clone(),
            secrets,
            store.clone(),
        )?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let body = serde_json::json!({"model":"grok-model","input":[{"type":"function_call_output","call_id":"tool-call","output":"durable result"}],"previous_response_id":"m2-native-1","store":true,"stream":streaming,"reasoning":{"effort":"low","summary":"auto"}});
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", "Bearer m2-owner-secret"))
                .set_json(&body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = test::read_body(response).await;
        assert!(String::from_utf8_lossy(&bytes).contains("completed"));
        let continued = store
            .get_owned(
                &ClientKeyId::try_new("owner")?,
                &ResponseId::try_new("m2-native-2")?,
                system_now_ms_runtime()?,
            )?
            .ok_or("continued durable root")?;
        assert_eq!(
            continued
                .payload()
                .lineage()
                .credential()
                .credential_revision(),
            2
        );
        assert_eq!(transport.bodies.lock().map_err(|_| "capture")?.len(), 2);
        for (secret, model) in [
            ("m2-foreign-secret", "grok-model"),
            ("m2-owner-secret", "changed-model"),
        ] {
            let mut changed = body.clone();
            changed["model"] = serde_json::json!(model);
            let rejected = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", format!("Bearer {secret}")))
                    .set_json(changed)
                    .to_request(),
            )
            .await;
            assert!(rejected.status().is_client_error());
        }
        // Corrupt only owned copies: recompile from durable range metadata and
        // prove that missing, conflicting or noncontiguous proof cannot reuse
        // revision 1 even though this current grant is still active.
        for (condition, floor, current) in [
            ("missing", None, None),
            ("noncontiguous", Some(2), Some(2)),
            ("conflicting", Some(1), Some(3)),
        ] {
            let copied = database.with_file_name(format!("{condition}-{streaming}.sqlite3"));
            gateway_store::open(database)?.execute(
                "VACUUM INTO ?1",
                [copied.to_str().ok_or("proof copy path")?],
            )?;
            gateway_store::open(&copied)?.execute(
                "UPDATE grok_accounts SET continuation_floor_revision=?1,continuation_current_revision=?2 WHERE id=?3",
                (floor, current, account),
            )?;
            let reopened = provider_grok::GrokAccountPoolStore::try_open(&copied, secrets.clone())?;
            let compiled = reopened
                .compile_native_runtime(std::slice::from_ref(&binding), system_now_ms_runtime()?)?;
            let pools = compiled.credential_pools();
            let pool = pools
                .pool(original.endpoint_id())
                .ok_or("negative proof pool")?
                .clone();
            let state = durable_state(
                pool.clone(),
                credential.clone(),
                transport.clone(),
                secrets,
                store.clone(),
            )?;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state))
                    .configure(gateway_http_actix::configure),
            )
            .await;
            let rejected = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", "Bearer m2-owner-secret"))
                    .set_json(&body)
                    .to_request(),
            )
            .await;
            assert_eq!(
                rejected.status(),
                StatusCode::SERVICE_UNAVAILABLE,
                "{condition}/stream={streaming}"
            );
            let bytes = test::read_body(rejected).await;
            assert!(!String::from_utf8_lossy(&bytes).contains("response.completed"));
            assert!(
                store
                    .get_owned(
                        &ClientKeyId::try_new("owner")?,
                        &ResponseId::try_new("m2-native-3")?,
                        system_now_ms_runtime()?
                    )?
                    .is_none()
            );
            assert_eq!(transport.bodies.lock().map_err(|_| "capture")?.len(), 2);
            assert_eq!(pool.active_lease_count(&credential), Some(0));
            println!(
                "E05 public {condition}/stream={streaming}: rejected_send=0 fallback=0 erroneous_root=0"
            );
        }
        // Isolate destructive test changes from the production-worker caller's original database.
        let manual = database.with_file_name(format!("manual-{streaming}.sqlite3"));
        gateway_store::open(database)?
            .execute("VACUUM INTO ?1", [manual.to_str().ok_or("manual path")?])?;
        let reopened = provider_grok::GrokAccountPoolStore::try_open(&manual, secrets.clone())?;
        let material = GrokBuildCredential::import_refreshable_runtime(
            reopened.open_credential(account)?.as_bytes(),
            system_now_ms_runtime()?,
        )?;
        reopened.replace_build_authorization(account, 1, &material, system_now_ms_runtime()?)?;
        assert!(!reopened.build_continuation_ranges()?.contains_key(account));
        let replaced = reopened
            .compile_native_runtime(std::slice::from_ref(&binding), system_now_ms_runtime()?)?;
        let replacement_pools = replaced.credential_pools();
        let replacement = replacement_pools
            .pool(original.endpoint_id())
            .ok_or("replacement pool")?
            .clone();
        let state = durable_state(
            replacement.clone(),
            credential.clone(),
            transport.clone(),
            secrets,
            store.clone(),
        )?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let rejected = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", "Bearer m2-owner-secret"))
                .set_json(&body)
                .to_request(),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            store
                .get_owned(
                    &ClientKeyId::try_new("owner")?,
                    &ResponseId::try_new("m2-native-3")?,
                    system_now_ms_runtime()?
                )?
                .is_none()
        );
        let mut phantom = body.clone();
        phantom["previous_response_id"] = serde_json::json!("m2-native-3");
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", "Bearer m2-owner-secret"))
                    .set_json(phantom)
                    .to_request()
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            transport.bodies.lock().map_err(|_| "capture")?.len(),
            2,
            "rejection cannot send, fallback or retry"
        );
        assert_eq!(replacement.active_lease_count(&credential), Some(0));
        println!(
            "E05 durable worker + reopened store stream={streaming}: first=1 continuation=1 rejected_send=0 fallback=0 erroneous_root=0"
        );
    }
    Ok(())
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
            let credential = self.credential.clone();
            let client = execution
                .context()
                .client_key_id()
                .ok_or_else(internal_error)?;
            let pin = self.codec.continuation_pin(
                execution.request(),
                client,
                u64::try_from(self.now).map_err(|_| internal_error())?,
                execution.continuation_pin().cloned(),
            )?;
            let lease = if let Some(pin) = pin.as_ref() {
                self.pool.try_lease_exact_revision_eligible_at(
                    pin.lineage().credential_id(),
                    pin.lineage().credential_revision(),
                    self.now,
                    |_| true,
                )
            } else {
                self.pool
                    .try_lease_exact_eligible_at(&credential, self.now, |_| true)
            }
            .ok_or_else(credential_unavailable_error)?;
            let lineage = ResponsesExecutionLineage::new(
                SnapshotVersion::try_new("m2-config").map_err(|_| internal_error())?,
                ProviderId::try_new("m2-provider").map_err(|_| internal_error())?,
                gateway_core::UpstreamId::try_new("m2-provider").map_err(|_| internal_error())?,
                self.pool.endpoint_id().clone(),
                RouteId::try_new("m2-route").map_err(|_| internal_error())?,
                RouteCandidateId::try_new("m2-candidate").map_err(|_| internal_error())?,
                credential,
                lease.credential_revision(),
            );
            if let Some(recorder) = execution.lineage_recorder() {
                recorder.record(lineage.clone())?;
            }
            let mode = if self.transport.streaming {
                GrokBuildExecutionMode::Streaming
            } else {
                GrokBuildExecutionMode::NonStreaming
            };
            let adapter = GrokBuildInferenceAdapter::try_new(
                GrokBuildCredential::import_active_runtime(lease.secret_bytes(), self.now)
                    .or_else(|_| GrokBuildCredential::import_json(lease.secret_bytes(), self.now))
                    .map_err(|_| credential_unavailable_error())?,
                "grok-model",
                mode,
                self.transport.clone(),
            )?
            .with_reasoning_owner(provider_grok::GrokBuildReasoningOwner::new(
                self.codec.clone(),
                client.clone(),
                execution.request().requested_model.clone(),
                "grok-model".into(),
                lineage,
                u64::try_from(self.now).map_err(|_| internal_error())?,
            ));
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
                codec: provider_grok::GrokBuildReasoningCodec::new(SecretStore::new(
                    MasterKeyRing::try_new(
                        version,
                        [(version, MasterKey::try_from_bytes([61; 32])?)],
                    )?,
                )),
                pool: pool.clone(),
                transport: transport.clone(),
                credential: credential.clone(),
                now: NOW,
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

#[actix_web::test]
async fn native_build_stateless_cipher_history_preserves_tools_without_storing_responses()
-> TestResult {
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
        let secret_store = SecretStore::new(MasterKeyRing::try_new(
            version,
            [(version, MasterKey::try_from_bytes([61; 32])?)],
        )?);
        let store = Arc::new(SqliteStoredResponseStore::open_in_memory(
            secret_store.clone(),
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
                codec: provider_grok::GrokBuildReasoningCodec::new(secret_store),
                pool,
                transport: transport.clone(),
                credential: credential.clone(),
                now: NOW,
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
        let first = test::TestRequest::post().uri("/v1/responses").insert_header(("authorization","Bearer m2-owner-secret"))
            .set_json(serde_json::json!({"model":"grok-model","input":"synthetic","store":false,"stream":streaming,
                "reasoning":{"effort":"low"}})).to_request();
        let response = test::call_service(&app, first).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = test::read_body(response).await;
        assert!(!String::from_utf8_lossy(&bytes).contains("synthetic-cipher"));
        let terminal = if streaming {
            let wire = std::str::from_utf8(&bytes)?;
            let data = wire
                .split("\n\n")
                .find(|v| v.starts_with("event: response.completed"))
                .ok_or("completed event")?
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .ok_or("terminal data")?;
            serde_json::from_str::<Value>(data)?["response"].clone()
        } else {
            serde_json::from_slice::<Value>(&bytes)?
        };
        let mut history = terminal["output"].as_array().ok_or("output")?.clone();
        assert!(
            history[0]["encrypted_content"]
                .as_str()
                .is_some_and(gateway_core::is_owned_reasoning_token)
        );
        history.push(serde_json::json!({"type":"function_call_output","call_id":"tool-call","output":"synthetic result"}));
        let continuation = serde_json::json!({"model":"grok-model","input":history,"store":false,"stream":streaming,"reasoning":{"effort":"low"}});
        let next = test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", "Bearer m2-owner-secret"))
            .set_json(&continuation)
            .to_request();
        let response = test::call_service(&app, next).await;
        assert_eq!(response.status(), StatusCode::OK);
        let _ = test::read_body(response).await;
        let foreign = test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", "Bearer m2-foreign-secret"))
            .set_json(&continuation)
            .to_request();
        assert_eq!(
            test::call_service(&app, foreign).await.status(),
            StatusCode::BAD_REQUEST
        );
        let requests = transport.bodies.lock().map_err(|_| "capture")?;
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[1]["input"][0]["encrypted_content"],
            "synthetic-cipher"
        );
        assert_eq!(requests[1]["input"][2]["output"], "synthetic result");
        for id in ["m2-native-1", "m2-native-2"] {
            assert!(
                store
                    .get_owned(
                        &ClientKeyId::try_new("owner")?,
                        &ResponseId::try_new(id)?,
                        system_now_ms_runtime()?
                    )?
                    .is_none()
            );
        }
    }
    Ok(())
}
