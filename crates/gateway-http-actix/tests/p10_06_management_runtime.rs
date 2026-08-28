//! P10-06 protected runtime-management workflow regression tests.

#![deny(unsafe_code)]

use std::{
    error::Error,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use actix_web::{
    App,
    dev::ServiceResponse,
    http::{StatusCode, header},
    test, web,
};
use gateway_auth::client_key::{ClientKeyPepper, ClientKeyService};
use gateway_control::management_mutation_service::{
    ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration, KeyVersion,
    ManagementMutationService, MasterKey, MasterKeyRing, SecretStore, SqliteControlPlaneRepository,
};
use gateway_core::{CredentialId, EndpointId, RequestId, RouteCandidateId};
use gateway_http_actix::{
    management_resources::{
        ManagementCatalogFreshness, ManagementCatalogStatus, ManagementQuotaRecoveryState,
        ManagementRequestAttempt, ManagementRequestAttemptStage, ManagementRequestProtocol,
        ManagementResourceHttpState, ManagementRouteExplain, ManagementRouteExplainCandidate,
        ManagementRouteExplainRequest, ManagementRuntimeAvailability,
        ManagementRuntimeAvailabilityStatus, ManagementRuntimeClock, ManagementRuntimeError,
        ManagementRuntimeFacade, ManagementRuntimeTarget, RejectingManagementEndpointWorkflow,
        configure_management_resources,
    },
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";
const VERSION: &str = "draft-p10-runtime";
const ENDPOINT: &str = "endpoint-runtime";
const CREDENTIAL: &str = "credential-runtime";

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 44_406))
}

fn security_state() -> Result<ManagementHttpState, Box<dyn Error>> {
    Ok(ManagementHttpState::new(
        ManagementKey::try_new(MANAGEMENT_KEY)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?)
}

fn mutation_service() -> Result<ManagementMutationService, Box<dyn Error>> {
    let version_id = ConfigVersionId::try_new(VERSION)?;
    let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
    repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
        id: version_id,
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision: 0,
        created_at_ms: 1,
        description: "P10-06 HTTP fixture".to_owned(),
    }))?;
    let key_version = KeyVersion::try_new(1)?;
    let key_ring = MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x64_u8; 32])?)],
    )?;
    let issuer = ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x53_u8; 32])?);
    Ok(ManagementMutationService::with_client_key_service(
        repository,
        SecretStore::new(key_ring),
        issuer,
    ))
}

fn authorized(request: test::TestRequest, revision: Option<&str>) -> test::TestRequest {
    let request = request
        .peer_addr(loopback())
        .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
        .insert_header(("X-Config-Version", VERSION));
    match revision {
        Some(revision) => request.insert_header(("If-Match", revision)),
        None => request,
    }
}

fn request_only_authorized(request: test::TestRequest) -> test::TestRequest {
    request
        .peer_addr(loopback())
        .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
}

fn assert_revision(response: &ServiceResponse, revision: i64) {
    let expected = format!("\"rev-{revision}\"");
    assert_eq!(
        response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok()),
        Some(expected.as_str())
    );
}

#[derive(Default)]
struct RuntimeCalls {
    catalog_reads: usize,
    availability_reads: usize,
    recovery_requests: usize,
    explain_reads: usize,
    attempt_reads: usize,
    observed_at: Vec<i64>,
}

struct FixtureRuntimeFacade {
    calls: Arc<Mutex<RuntimeCalls>>,
}

impl FixtureRuntimeFacade {
    fn calls(&self) -> Result<std::sync::MutexGuard<'_, RuntimeCalls>, ManagementRuntimeError> {
        self.calls
            .lock()
            .map_err(|_| ManagementRuntimeError::Unavailable)
    }

    fn endpoint() -> Result<EndpointId, ManagementRuntimeError> {
        EndpointId::try_new(ENDPOINT).map_err(|_| ManagementRuntimeError::Unavailable)
    }

    fn credential() -> Result<CredentialId, ManagementRuntimeError> {
        CredentialId::try_new(CREDENTIAL).map_err(|_| ManagementRuntimeError::Unavailable)
    }
}

impl ManagementRuntimeFacade for FixtureRuntimeFacade {
    fn catalog_status(
        &mut self,
        config_version_id: &ConfigVersionId,
        observed_at_ms: i64,
    ) -> Result<Vec<ManagementCatalogStatus>, ManagementRuntimeError> {
        if config_version_id.as_str() != VERSION {
            return Err(ManagementRuntimeError::Unavailable);
        }
        let mut calls = self.calls()?;
        calls.catalog_reads += 1;
        calls.observed_at.push(observed_at_ms);
        drop(calls);
        Ok(vec![ManagementCatalogStatus::new(
            Self::endpoint()?,
            Self::credential()?,
            ManagementCatalogFreshness::Fresh,
            observed_at_ms,
        )])
    }

    fn runtime_availability(
        &mut self,
        config_version_id: &ConfigVersionId,
        observed_at_ms: i64,
    ) -> Result<Vec<ManagementRuntimeAvailabilityStatus>, ManagementRuntimeError> {
        if config_version_id.as_str() != VERSION {
            return Err(ManagementRuntimeError::Unavailable);
        }
        let mut calls = self.calls()?;
        calls.availability_reads += 1;
        calls.observed_at.push(observed_at_ms);
        drop(calls);
        Ok(vec![ManagementRuntimeAvailabilityStatus::new(
            Self::endpoint()?,
            Self::credential()?,
            ManagementRuntimeAvailability::RecoveryRequired,
        )])
    }

    fn request_quota_recovery(
        &mut self,
        config_version_id: &ConfigVersionId,
        target: &ManagementRuntimeTarget,
        observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        if config_version_id.as_str() != VERSION
            || target.endpoint_id().as_str() != ENDPOINT
            || target.credential_id().as_str() != CREDENTIAL
            || target.upstream_model() != Some("runtime-model")
        {
            return Err(ManagementRuntimeError::Unavailable);
        }
        let mut calls = self.calls()?;
        calls.recovery_requests += 1;
        calls.observed_at.push(observed_at_ms);
        Ok(ManagementQuotaRecoveryState::ProbeScheduled)
    }

    fn explain_route(
        &mut self,
        request: &ManagementRouteExplainRequest,
    ) -> Result<ManagementRouteExplain, ManagementRuntimeError> {
        if request.config_version_id().as_str() != VERSION
            || request.route_id().as_str() != "route-runtime"
            || request.requested_model() != "public-runtime"
            || request.protocol() != ManagementRequestProtocol::OpenAiResponses
            || request.observed_at_ms() != 1_700_000_000_000
        {
            return Err(ManagementRuntimeError::Unavailable);
        }
        let mut calls = self.calls()?;
        calls.explain_reads += 1;
        calls.observed_at.push(request.observed_at_ms());
        drop(calls);
        ManagementRouteExplain::try_new(
            request.route_id().clone(),
            vec![
                ManagementRouteExplainCandidate::excluded(
                    RouteCandidateId::try_new("candidate-blocked")
                        .map_err(|_| ManagementRuntimeError::Unavailable)?,
                    "endpoint_cooldown",
                ),
                ManagementRouteExplainCandidate::selected(
                    RouteCandidateId::try_new("candidate-selected")
                        .map_err(|_| ManagementRuntimeError::Unavailable)?,
                ),
            ],
        )
    }

    fn list_request_attempts(
        &mut self,
        request_id: &RequestId,
    ) -> Result<Vec<ManagementRequestAttempt>, ManagementRuntimeError> {
        if request_id.as_str() != "request-runtime" {
            return Err(ManagementRuntimeError::Unavailable);
        }
        self.calls()?.attempt_reads += 1;
        Ok(vec![
            ManagementRequestAttempt::try_new(
                "attempt-runtime-1".to_owned(),
                "succeeded",
                Some(Self::endpoint()?),
                Some(Self::credential()?),
            )?
            .with_stage(ManagementRequestAttemptStage::Decoder),
        ])
    }
}

struct FixedRuntimeClock;

impl ManagementRuntimeClock for FixedRuntimeClock {
    fn now_ms(&self) -> Result<i64, ManagementRuntimeError> {
        Ok(1_700_000_000_000)
    }
}

fn runtime_state(
    calls: Arc<Mutex<RuntimeCalls>>,
) -> Result<ManagementResourceHttpState, Box<dyn Error>> {
    Ok(ManagementResourceHttpState::with_workflow_and_runtime(
        mutation_service()?,
        Box::new(RejectingManagementEndpointWorkflow::new()),
        Box::new(FixtureRuntimeFacade { calls }),
        Box::new(FixedRuntimeClock),
    ))
}

#[actix_web::test]
async fn protected_runtime_views_are_value_free_and_recovery_only_requests_controller_work()
-> TestResult {
    let calls = Arc::new(Mutex::new(RuntimeCalls::default()));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(runtime_state(Arc::clone(&calls))?))
            .configure(configure_management_resources),
    )
    .await;

    let denied = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/catalog/status")
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);

    let catalog = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/catalog/status"), None).to_request(),
    )
    .await;
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog_body: Value = serde_json::from_slice(&test::read_body(catalog).await)?;
    assert_eq!(
        catalog_body,
        json!([{
            "endpoint_id": ENDPOINT, "credential_id": CREDENTIAL,
            "freshness": "fresh", "observed_at_ms": 1_700_000_000_000_i64
        }])
    );

    let availability = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/runtime/availability"),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(availability.status(), StatusCode::OK);
    let availability_body: Value = serde_json::from_slice(&test::read_body(availability).await)?;
    assert_eq!(
        availability_body,
        json!([{
            "endpoint_id": ENDPOINT, "credential_id": CREDENTIAL,
            "availability": "recovery_required"
        }])
    );

    let explain = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(
                "/admin/routes/route-runtime/explain?requested_model=public-runtime&protocol=openai_responses",
            ),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(explain.status(), StatusCode::OK);
    let explain_body: Value = serde_json::from_slice(&test::read_body(explain).await)?;
    assert_eq!(
        explain_body,
        json!({
            "route_id": "route-runtime",
            "candidates": [
                {"candidate_id":"candidate-blocked", "decision":"excluded", "reason":"endpoint_cooldown"},
                {"candidate_id":"candidate-selected", "decision":"selected"}
            ]
        })
    );
    assert!(!explain_body.to_string().contains("public-runtime"));

    let attempts = test::call_service(
        &app,
        request_only_authorized(
            test::TestRequest::get().uri("/admin/requests/request-runtime/attempts"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(attempts.status(), StatusCode::OK);
    let attempts_body: Value = serde_json::from_slice(&test::read_body(attempts).await)?;
    assert_eq!(
        attempts_body,
        json!([{
            "attempt_id":"attempt-runtime-1", "outcome":"succeeded", "stage":"decoder",
            "endpoint_id": ENDPOINT, "credential_id": CREDENTIAL
        }])
    );
    assert!(!attempts_body.to_string().contains("upstream_model"));

    let policy = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/egress-policies")
                .set_json(json!({
                    "id":"runtime-policy", "name":"runtime policy", "allowed_schemes":["https"],
                    "allowed_hosts":["api.example.test"], "allowed_ports":[443], "allowed_cidrs":[],
                    "redirect_mode":"deny", "max_redirects":0
                })),
            Some("rev-0"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(policy.status(), StatusCode::CREATED);
    assert_revision(&policy, 1);

    let upstream = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams")
                .set_json(json!({
                    "id":"runtime-upstream", "name":"runtime upstream", "kind":"fixture",
                    "enabled":true, "tags":[], "egress_policy_id":"runtime-policy"
                })),
            Some("rev-1"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(upstream.status(), StatusCode::CREATED);
    assert_revision(&upstream, 2);

    let endpoint = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams/runtime-upstream/endpoints")
                .set_json(json!({
                    "id": ENDPOINT, "adapter_id":"fixture", "api_format":"openai/responses",
                    "base_url":"https://api.example.test/v1", "inference_path":"/responses",
                    "models_path":null, "transport":"https", "enabled":true
                })),
            Some("rev-2"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(endpoint.status(), StatusCode::CREATED);
    assert_revision(&endpoint, 3);

    let credential = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/upstreams/runtime-upstream/credentials")
                .set_json(json!({
                    "id": CREDENTIAL, "kind":"api_key", "secret":"fixture-only-secret", "status":"active"
                })),
            Some("rev-3"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(credential.status(), StatusCode::CREATED);
    assert_revision(&credential, 4);

    let binding = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/endpoints/endpoint-runtime/credential-bindings")
                .set_json(json!({
                    "credential_id": CREDENTIAL, "enabled":true, "priority":0, "weight":1, "concurrency":1
                })),
            Some("rev-4"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(binding.status(), StatusCode::CREATED);
    assert_revision(&binding, 5);

    let recovery = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/runtime/quota/reset")
                .set_json(json!({
                    "endpoint_id": ENDPOINT, "credential_id": CREDENTIAL, "upstream_model":"runtime-model"
                })),
            None,
        )
        .to_request(),
    )
    .await;
    assert_eq!(recovery.status(), StatusCode::ACCEPTED);
    assert_eq!(
        serde_json::from_slice::<Value>(&test::read_body(recovery).await)?,
        json!({"state":"probe_scheduled"})
    );

    let revisions = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/egress-policies"), None).to_request(),
    )
    .await;
    assert_eq!(revisions.status(), StatusCode::OK);
    assert_revision(&revisions, 5);

    let calls = calls
        .lock()
        .map_err(|_| "runtime call inspection lock poisoned")?;
    assert_eq!(calls.catalog_reads, 1);
    assert_eq!(calls.availability_reads, 1);
    assert_eq!(calls.explain_reads, 1);
    assert_eq!(calls.attempt_reads, 1);
    assert_eq!(calls.recovery_requests, 1);
    assert_eq!(calls.observed_at, vec![1_700_000_000_000; 4]);
    Ok(())
}

#[actix_web::test]
async fn default_runtime_facade_is_fail_closed_without_runtime_dependencies() -> TestResult {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(ManagementResourceHttpState::new(
                mutation_service()?,
            )))
            .configure(configure_management_resources),
    )
    .await;

    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri("/admin/catalog/status"), None).to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        serde_json::from_slice::<Value>(&test::read_body(response).await)?,
        json!({"error":{
            "code":"management_runtime_unavailable",
            "message":"Management runtime observation is unavailable"
        }})
    );
    Ok(())
}
