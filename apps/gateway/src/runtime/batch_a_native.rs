//! Synthetic native pull transport, production decoder/lease and real store/runtime rebuild.
//! No fixed Provider destination is contacted by this test.
use super::tests::{TemporaryDirectory, p12_configuration_for, test_secret_store};
use super::*;
use gateway_auth::client_key::ClientKeyPepper;
use gateway_control::management_service::{ManagementActor, ManagementService};
use gateway_http_actix::{
    ResponsesStateSource, management_resources::native_accounts::NativeAccountRuntime,
};
use gateway_store::control_plane::{StoredClientKey, StoredClientKeyStatus};
use provider_grok::{
    GrokBuildResponseBody, GrokBuildResponseContentEncoding, GrokBuildResponseContentType,
    GrokBuildResponsesOutboundRequest, GrokBuildTransport, GrokBuildTransportResponse,
};
use std::sync::atomic::AtomicUsize;

struct HeldTransport {
    calls: AtomicUsize,
    release: Arc<tokio::sync::Notify>,
}
impl GrokBuildTransport for HeldTransport {
    fn send(
        &self,
        _: GrokBuildResponsesOutboundRequest,
    ) -> BoxFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let fixture = &[
                include_bytes!("../../../../tests/fixtures/grok-build/p6-03-stream.sse").as_slice(),
                b"\n",
            ]
            .concat();
            let needle = b"event: response.output_text.delta";
            let split = fixture
                .windows(needle.len())
                .enumerate()
                .filter(|(_, row)| *row == needle)
                .nth(1)
                .map(|(index, _)| index)
                .ok_or_else(internal_error)?;
            Ok(GrokBuildTransportResponse::new(
                200,
                GrokBuildResponseContentType::EventStream,
                GrokBuildResponseContentEncoding::Identity,
                Box::new(HeldBody {
                    first: Some(fixture[..split].to_vec()),
                    tail: Some(fixture[split..].to_vec()),
                    release: self.release.clone(),
                }),
            ))
        })
    }
}
struct HeldBody {
    first: Option<Vec<u8>>,
    tail: Option<Vec<u8>>,
    release: Arc<tokio::sync::Notify>,
}
impl GrokBuildResponseBody for HeldBody {
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            if let Some(first) = self.first.take() {
                return Ok(Some(first));
            }
            if self.tail.is_some() {
                self.release.notified().await;
                return Ok(self.tail.take());
            }
            Ok(None)
        })
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one held-stream scenario checks retirement, rebuild, retry suppression and natural completion"
)]
fn native_disable_rebuild_preserves_an_open_stream_and_retires_new_execution()
-> Result<(), Box<dyn Error>> {
    let directory = TemporaryDirectory::new()?;
    let database = directory.path().join("batch-a-native.sqlite3");
    let secrets = test_secret_store()?;
    let mut config = p12_configuration_for(&secrets, "batch-a-native")?;
    config.upstreams[0].kind = "grok-build-native".into();
    config.endpoints[0].adapter_id = "grok.build.responses".into();
    config.endpoints[0].base_url = provider_grok::GROK_BUILD_RESPONSES_BASE_URL.into();
    config.endpoints[0].inference_path = provider_grok::GROK_BUILD_RESPONSES_PATH.into();
    config.egress_policies[0].allowed_hosts_json = r#"["cli-chat-proxy.grok.com"]"#.into();
    config.credentials.clear();
    config.endpoint_credential_bindings.clear();
    let issuer = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x77; 32])?);
    let (key, presented) = issuer
        .issue(
            ClientKeyId::try_new("batch-a-native-key")?,
            config.access_groups[0].id.clone(),
            None,
        )?
        .into_parts();
    config.client_keys = vec![StoredClientKey::try_new(
        key.client_key_id().clone(),
        key.access_group_id().clone(),
        key.prefix().as_str(),
        key.secret_digest().as_bytes(),
        StoredClientKeyStatus::Active,
        None,
    )?];
    let mut repository = SqliteControlPlaneRepository::open(&database)?;
    repository.write_configuration(&config)?;
    let mut lifecycle = ManagementService::bootstrap(
        SqliteControlPlaneRepository::open(&database)?,
        deployment_route_compiler(&database)?,
        ManagementActor::try_new("batch-a-native-test")?,
    )?;
    let composition = build_data_plane_composition(
        &database,
        &secrets,
        Arc::clone(lifecycle.registry()),
        issuer,
    )?;
    lifecycle.set_runtime_preparer(composition.reload.clone());
    lifecycle.publish_configuration(&config.version.id)?;
    let now = system_now_ms_runtime()?;
    let store = provider_grok::GrokAccountPoolStore::try_open(&database, secrets.clone())?;
    let material=GrokBuildCredential::import_json(br#"{"access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_in":3600,"token_type":"Bearer"}"#,now)?;
    let credential = provider_grok::GrokAccountCredential::try_from_build_credential(&material)?;
    let account = provider_grok::GrokAccountImport {
        provider: provider_grok::GrokAccountProvider::Build,
        identity: credential.enrollment_identity(
            provider_grok::GrokAccountProvider::Build,
            now,
            None,
        )?,
        credential,
        auth_status: provider_grok::GrokAccountAuthStatus::Active,
        enabled: true,
        priority: 0,
        weight: 1,
        max_concurrency: 1,
        refresh_due_at_ms: None,
        quota_sync_due_at_ms: None,
        cooldown_until_ms: None,
    };
    store.import_batch("batch-a-native-account", &[account], now)?;
    assert!(composition.reload.apply(None));
    let captured = composition.reload.capture();
    let account = store.single_import_account("batch-a-native-account")?;
    let credential_id = CredentialId::try_new(account.clone())?;
    let binding = provider_grok::GrokAccountEndpointBinding::new(
        provider_grok::GrokAccountProvider::Build,
        config.endpoints[0].id.clone(),
    );
    let compiled = store.compile_native_runtime(&[binding], now)?;
    let pools = compiled.credential_pools();
    let pool = pools.pool(&config.endpoints[0].id).ok_or("native pool")?;
    let scheduler = RouteCredentialScheduler::new(lifecycle.registry().load(), pools.clone());
    let selected = scheduler.select_and_lease(&config.model_routes[0].id)?;
    let release = Arc::new(tokio::sync::Notify::new());
    let transport = Arc::new(HeldTransport {
        calls: AtomicUsize::new(0),
        release: release.clone(),
    });
    let adapter = GrokBuildInferenceAdapter::try_new(
        GrokBuildCredential::import_active_runtime(selected.lease().secret_bytes(), now)?,
        "grok-4.5-build",
        GrokBuildExecutionMode::Streaming,
        transport.clone(),
    )?;
    let request = protocol_openai_responses::decode_request(
        r#"{"model":"p12-test-model","input":"synthetic held stream","stream":true}"#,
    )?
    .request;
    let context = RequestContext::new(RequestId::try_new("batch-a-native-request")?);
    let mut source = actix_web::rt::System::new().block_on(async {
        let inner = adapter.execute(context, request).await?;
        let mut source = LeaseHoldingEventSource {
            source: Box::new(P12ProviderEventSource::new(inner)),
            _selection: selected,
        };
        loop {
            match source.next_event().await? {
                Some(CanonicalEvent::TextDelta(_)) => break,
                Some(CanonicalEvent::ResponseEnd(_)) | None => return Err(internal_error()),
                _ => {}
            }
        }
        Ok::<_, GatewayError>(source)
    })?;
    assert_eq!(pool.active_lease_count(&credential_id), Some(1));
    store.manage_account(
        &account,
        0,
        provider_grok::GrokManagedAccountChange::SetEnabled(false),
        "batch-a",
        now + 1,
    )?;
    assert!(composition.reload.apply(None));
    let page = composition
        .provider_account_pools
        .list_provider_account_pools(
            &gateway_control::provider_account_pool_service::ProviderAccountPoolQuery::default(),
        )?;
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].enabled);
    let mut runtime: Box<dyn gateway_http_actix::management_resources::ManagementRuntimeFacade> =
        Box::new(composition.reload.as_ref().clone());
    let effective = runtime
        .effective_models(
            &config.version.id,
            &gateway_http_actix::management_resources::ManagementEffectiveModelContext::AccessGroup(
                config.access_groups[0].id.clone(),
            ),
            now + 1,
        )
        .map_err(|_| "effective model projection")?
        .ok_or("effective models")?;
    assert!(effective.models.is_empty());
    actix_web::rt::System::new().block_on(async {
        use actix_web::{App,http::StatusCode,test,web};
        let held=test::init_service(App::new().app_data(web::Data::from(captured)).configure(gateway_http_actix::configure)).await;
        let response=test::call_service(&held,test::TestRequest::post().uri("/v1/responses").insert_header(("Authorization",format!("Bearer {}",presented.as_str()))).set_json(serde_json::json!({"model":"p12-test-model","input":"never send","stream":false})).to_request()).await;
        assert!(matches!(response.status(),StatusCode::INTERNAL_SERVER_ERROR|StatusCode::SERVICE_UNAVAILABLE));
        release.notify_one();
        let mut ended=false;
        while let Some(event)=source.next_event().await?{assert!(!matches!(event,CanonicalEvent::StreamError(_)),"unexpected held stream event: {event:?}");if matches!(event,CanonicalEvent::ResponseEnd(_)){ended=true;}}
        assert!(ended);Ok::<(),GatewayError>(())
    })?;
    assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
    drop(source);
    assert_eq!(pool.active_lease_count(&credential_id), Some(0));
    Ok(())
}
