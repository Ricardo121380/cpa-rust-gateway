//! P13-04 management operations HTTP regression tests.

#![deny(unsafe_code)]

use std::{error::Error, net::SocketAddr};

use actix_web::{
    App,
    http::{StatusCode, header},
    test, web,
};
use gateway_control::management_mutation_service::{
    ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
    CredentialConfiguration, EndpointConfiguration, EndpointCredentialBindingConfiguration,
    EndpointTransport, KeyVersion, ManagementMutationService, MasterKey, MasterKeyRing,
    SecretStore, SqliteControlPlaneRepository,
};
use gateway_control::management_operations_service::{
    FailureFeedbackPage, FailureFeedbackQuery, ManagementOperationsError, OperationalBillingPage,
    OperationalBillingQuery, OperationalUsagePage, OperationalUsageQuery,
    compile_failure_feedback_page, compile_operational_billing_page,
    compile_operational_usage_page,
};
use gateway_control::provider_account_pool_service::{
    ProviderAccountAuthStatus, ProviderAccountOperatorAction, ProviderAccountOperatorActionKind,
    ProviderAccountOperatorReceipt, ProviderAccountOperatorState, ProviderAccountPoolError,
    ProviderAccountPoolFacade, ProviderAccountPoolItem, ProviderAccountPoolPage,
    ProviderAccountPoolQuery, ProviderAccountPoolSnapshot, ProviderAccountRuntimeStatus,
};
use gateway_core::{
    AttemptEvent, AttemptOutcome, AttemptRetryDecision, ClientKeyId, CredentialId, EndpointId,
    ErrorScope, GatewayError, GatewayErrorCode, GatewayEvent, GatewayProtocol, ProviderId,
    RequestEvent, ResponseId, RouteCandidateId, RouteId, UpstreamId, Usage, UsageEvent,
};
use gateway_http_actix::{
    management_resources::{
        ManagementFailureFeedbackFacade, ManagementResourceHttpState, ManagementUsageFacade,
        configure_management_resources,
    },
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use gateway_store::{
    billing_ledger::{BillingCostConfidence, BillingLedgerEntry},
    control_plane::{CredentialStatus, UpstreamConfiguration},
    event_store::SqliteEventStore,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";

struct FixtureUsageFacade {
    events: Vec<gateway_store::event_store::StoredGatewayEvent>,
    billing_entries: Vec<BillingLedgerEntry>,
}

impl ManagementUsageFacade for FixtureUsageFacade {
    fn list_usage(
        &self,
        query: &OperationalUsageQuery,
    ) -> Result<OperationalUsagePage, ManagementOperationsError> {
        compile_operational_usage_page(&self.events, query)
    }

    fn list_billing(
        &self,
        query: &OperationalBillingQuery,
    ) -> Result<OperationalBillingPage, ManagementOperationsError> {
        compile_operational_billing_page(&self.billing_entries, query)
    }
}

impl ManagementFailureFeedbackFacade for FixtureUsageFacade {
    fn list_failure_feedback(
        &self,
        query: &FailureFeedbackQuery,
    ) -> Result<FailureFeedbackPage, ManagementOperationsError> {
        compile_failure_feedback_page(&self.events, query)
    }
}

struct FixtureProviderAccountPoolFacade {
    snapshot: ProviderAccountPoolSnapshot,
}

impl ProviderAccountPoolFacade for FixtureProviderAccountPoolFacade {
    fn list_provider_account_pools(
        &self,
        query: &ProviderAccountPoolQuery,
    ) -> Result<ProviderAccountPoolPage, ProviderAccountPoolError> {
        self.snapshot.page(query)
    }

    fn apply_operator_action(
        &self,
        action: &ProviderAccountOperatorAction,
        observed_at_ms: i64,
    ) -> Result<ProviderAccountOperatorReceipt, ProviderAccountPoolError> {
        if action.config_version_id != "inventory-v1"
            || action.provider_id.as_str() != "grok"
            || action.channel_id.as_str() != "channel-build"
            || action.account_id.as_str() != "grok-account-a"
        {
            return Err(ProviderAccountPoolError::ActionTargetUnavailable);
        }
        let (state, cooldown_until_ms) = match action.kind {
            ProviderAccountOperatorActionKind::CoolDown => (
                ProviderAccountOperatorState::Cooling,
                Some(
                    observed_at_ms
                        .checked_add(action.cooldown_ms.unwrap_or_default())
                        .ok_or(ProviderAccountPoolError::SourceUnavailable)?,
                ),
            ),
            ProviderAccountOperatorActionKind::RequestRecovery => {
                (ProviderAccountOperatorState::ProbeScheduled, None)
            }
        };
        Ok(ProviderAccountOperatorReceipt {
            state,
            observed_at_ms,
            cooldown_until_ms,
        })
    }
}

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 45_405))
}

fn security_state() -> Result<ManagementHttpState, Box<dyn Error>> {
    Ok(ManagementHttpState::new(
        ManagementKey::try_new(MANAGEMENT_KEY)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?)
}

fn authorized(request: test::TestRequest, version: &str) -> test::TestRequest {
    request
        .peer_addr(loopback())
        .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
        .insert_header(("X-Config-Version", version))
}

#[allow(clippy::too_many_lines)]
fn resource_state() -> Result<ManagementResourceHttpState, Box<dyn Error>> {
    let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
    let key_version = KeyVersion::try_new(1)?;
    let key_ring = MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x5a_u8; 32])?)],
    )?;
    let secret_store = SecretStore::new(key_ring);
    for (version, revision) in [("inventory-v1", 4_i64), ("inventory-v2", 9_i64)] {
        repository.write_configuration(&fixture(
            ConfigVersionId::try_new(version)?,
            revision,
            &secret_store,
        )?)?;
    }
    let mut event_store = SqliteEventStore::open_in_memory()?;
    let request = RequestEvent::new(
        gateway_core::RequestId::try_new("usage-http-request")?,
        ClientKeyId::try_new("usage-http-client")?,
        None,
        GatewayProtocol::OpenAiResponses,
        "usage-public-model".to_owned(),
        "usage-public-model".to_owned(),
        None,
        false,
    );
    let attempt = AttemptEvent::new(
        request.request_id().clone(),
        1,
        RouteId::try_new("usage-http-route")?,
        RouteCandidateId::try_new("usage-http-candidate")?,
        CredentialId::try_new("account-a")?,
        EndpointId::try_new("channel-inventory")?,
        UpstreamId::try_new("provider-inventory")?,
        "usage-private-model".to_owned(),
        99,
        100,
        AttemptOutcome::Succeeded,
        AttemptRetryDecision::Completed,
    );
    let usage = UsageEvent::from_usage(
        request.request_id().clone(),
        ResponseId::try_new("usage-http-response")?,
        &Usage {
            input_tokens: Some(11),
            output_tokens: Some(7),
            ..Usage::default()
        },
    );
    let failed_request = RequestEvent::new(
        gateway_core::RequestId::try_new("failure-http-request")?,
        ClientKeyId::try_new("failure-http-client")?,
        None,
        GatewayProtocol::OpenAiResponses,
        "failure-public-model".to_owned(),
        "failure-public-model".to_owned(),
        None,
        false,
    );
    let failed_attempt = AttemptEvent::new(
        failed_request.request_id().clone(),
        1,
        RouteId::try_new("failure-http-route")?,
        RouteCandidateId::try_new("failure-http-candidate")?,
        CredentialId::try_new("account-b")?,
        EndpointId::try_new("channel-inventory")?,
        UpstreamId::try_new("provider-inventory")?,
        "private-model-must-not-leak".to_owned(),
        101,
        102,
        AttemptOutcome::Failed(GatewayError::new(
            GatewayErrorCode::CredentialUnauthorized,
            ErrorScope::Credential,
        )),
        AttemptRetryDecision::NonRetryable,
    );
    event_store.append_batch(&[
        GatewayEvent::Request(request),
        GatewayEvent::Attempt(attempt),
        GatewayEvent::Usage(usage),
        GatewayEvent::Request(failed_request),
        GatewayEvent::Attempt(failed_attempt),
    ])?;
    let usage_events = event_store.list_events()?;
    let billing_entries = vec![BillingLedgerEntry {
        ledger_id: 1,
        source_event_id: "billing-source-1".to_owned(),
        source_fingerprint: "a".repeat(64),
        request_id: "usage-http-request".to_owned(),
        response_id: "usage-http-response".to_owned(),
        provider_id: "provider-inventory".to_owned(),
        channel_id: "channel-inventory".to_owned(),
        account_id: "account-a".to_owned(),
        model: "usage-public-model".to_owned(),
        occurred_at_ms: 100,
        catalog_version_id: None,
        usage: gateway_core::UsageSummary {
            input_tokens: Some(11),
            output_tokens: Some(7),
            ..gateway_core::UsageSummary::default()
        },
        cost_microunits: None,
        cost_confidence: BillingCostConfidence::Unpriced,
        retention_expires_at_ms: 10_000,
        recorded_at_ms: 101,
    }];
    let provider_snapshot = ProviderAccountPoolSnapshot::try_new(
        "provider-snapshot-1",
        123,
        vec![
            ProviderAccountPoolItem {
                provider_id: ProviderId::try_new("grok")?,
                channel_id: EndpointId::try_new("channel-build")?,
                account_id: CredentialId::try_new("grok-account-a")?,
                account_kind: "grok_build_oauth".to_owned(),
                auth_status: ProviderAccountAuthStatus::Active,
                runtime_status: ProviderAccountRuntimeStatus::Available,
                enabled: true,
                priority: 1,
                weight: 1,
                max_concurrency: 4,
                active_leases: 1,
                expires_at_ms: Some(200),
                refresh_due_at_ms: Some(150),
                quota_sync_due_at_ms: Some(160),
            },
            ProviderAccountPoolItem {
                provider_id: ProviderId::try_new("grok")?,
                channel_id: EndpointId::try_new("channel-build")?,
                account_id: CredentialId::try_new("grok-account-b")?,
                account_kind: "grok_build_oauth".to_owned(),
                auth_status: ProviderAccountAuthStatus::ReauthRequired,
                runtime_status: ProviderAccountRuntimeStatus::Unauthorized,
                enabled: true,
                priority: 2,
                weight: 1,
                max_concurrency: 4,
                active_leases: 0,
                expires_at_ms: None,
                refresh_due_at_ms: Some(140),
                quota_sync_due_at_ms: None,
            },
            ProviderAccountPoolItem {
                provider_id: ProviderId::try_new("codex")?,
                channel_id: EndpointId::try_new("channel-chat")?,
                account_id: CredentialId::try_new("codex-account-a")?,
                account_kind: "codex_oauth".to_owned(),
                auth_status: ProviderAccountAuthStatus::Disabled,
                runtime_status: ProviderAccountRuntimeStatus::CircuitOpen,
                enabled: false,
                priority: 3,
                weight: 1,
                max_concurrency: 2,
                active_leases: 0,
                expires_at_ms: None,
                refresh_due_at_ms: None,
                quota_sync_due_at_ms: None,
            },
        ],
    )?;
    Ok(
        ManagementResourceHttpState::new(ManagementMutationService::new(repository, secret_store))
            .with_usage(Box::new(FixtureUsageFacade {
                events: usage_events.clone(),
                billing_entries,
            }))
            .with_failure_feedback(Box::new(FixtureUsageFacade {
                events: usage_events,
                billing_entries: Vec::new(),
            }))
            .with_provider_account_pools(Box::new(FixtureProviderAccountPoolFacade {
                snapshot: provider_snapshot,
            })),
    )
}

fn fixture(
    version_id: ConfigVersionId,
    revision: i64,
    secret_store: &SecretStore,
) -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
    let provider_id = UpstreamId::try_new("provider-inventory")?;
    let channel_id = EndpointId::try_new("channel-inventory")?;
    let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
        id: version_id,
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision,
        created_at_ms: 1,
        description: "P13-04A HTTP fixture".to_owned(),
    });
    configuration.upstreams.push(UpstreamConfiguration {
        id: provider_id.clone(),
        name: "Inventory Provider".to_owned(),
        kind: "openai-compatible".to_owned(),
        enabled: true,
        tags_json: "[]".to_owned(),
        egress_policy_id: None,
    });
    configuration.endpoints.push(EndpointConfiguration {
        id: channel_id.clone(),
        upstream_id: provider_id.clone(),
        adapter_id: "inventory.responses".to_owned(),
        api_format: "openai/responses".to_owned(),
        base_url: "https://secret-upstream.example/v1".to_owned(),
        inference_path: "/responses".to_owned(),
        models_path: None,
        transport: EndpointTransport::Sse,
        enabled: true,
    });
    for account_name in ["account-a", "account-b"] {
        let account_id = CredentialId::try_new(account_name)?;
        configuration.credentials.push(CredentialConfiguration {
            id: account_id.clone(),
            upstream_id: provider_id.clone(),
            kind: "oauth_json".to_owned(),
            encrypted_secret: secret_store.seal(b"secret-must-not-leak", b"p13-04a-http")?,
            status: if account_name == "account-a" {
                CredentialStatus::Active
            } else {
                CredentialStatus::Cooling
            },
            revision: 2,
        });
        configuration
            .endpoint_credential_bindings
            .push(EndpointCredentialBindingConfiguration {
                endpoint_id: channel_id.clone(),
                credential_id: account_id,
                upstream_id: provider_id.clone(),
                enabled: true,
                priority: 0,
                weight: 1,
                concurrency: 2,
            });
    }
    Ok(configuration)
}

#[actix_web::test]
async fn inventory_is_protected_paginated_and_value_free() -> TestResult {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(resource_state()?))
            .configure(configure_management_resources),
    )
    .await;

    let first = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/account-pools?limit=1"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(
        first.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-4\""))
    );
    let first_body: Value = test::read_body_json(first).await;
    assert_eq!(first_body["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(first_body["items"][0]["account_id"], "account-a");
    let cursor = first_body["next_cursor"]
        .as_str()
        .ok_or("first page missing cursor")?;
    let serialized = serde_json::to_string(&first_body)?;
    for forbidden in [
        "secret-must-not-leak",
        "secret-upstream.example",
        "encrypted_secret",
        "ciphertext",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "response leaked {forbidden}"
        );
    }

    let second = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(&format!(
                "/admin/operations/account-pools?limit=1&cursor={cursor}"
            )),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    let second_body: Value = test::read_body_json(second).await;
    assert_eq!(second_body["items"][0]["account_id"], "account-b");
    assert!(second_body["next_cursor"].is_null());

    let provider_first = test::call_service(
        &app,
        authorized(
            test::TestRequest::get()
                .uri("/admin/operations/provider-account-pools?provider_id=grok&limit=1"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(provider_first.status(), StatusCode::OK);
    assert_eq!(
        provider_first.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    let provider_first_body: Value = test::read_body_json(provider_first).await;
    assert_eq!(provider_first_body["snapshot_id"], "provider-snapshot-1");
    assert_eq!(provider_first_body["observed_at_ms"], 123);
    assert_eq!(
        provider_first_body["items"][0]["account_id"],
        "grok-account-a"
    );
    assert_eq!(provider_first_body["items"][0]["auth_status"], "active");
    assert_eq!(
        provider_first_body["items"][0]["runtime_status"],
        "available"
    );
    let provider_cursor = provider_first_body["next_cursor"]
        .as_str()
        .ok_or("provider first page missing cursor")?;
    let provider_second = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(&format!(
                "/admin/operations/provider-account-pools?provider_id=grok&limit=1&cursor={provider_cursor}"
            )),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(provider_second.status(), StatusCode::OK);
    let provider_second_body: Value = test::read_body_json(provider_second).await;
    assert_eq!(
        provider_second_body["items"][0]["account_id"],
        "grok-account-b"
    );
    assert_eq!(
        provider_second_body["items"][0]["auth_status"],
        "reauth_required"
    );
    assert_eq!(
        provider_second_body["items"][0]["runtime_status"],
        "unauthorized"
    );
    assert!(provider_second_body["next_cursor"].is_null());

    let provider_stale = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(&format!(
                "/admin/operations/provider-account-pools?provider_id=codex&limit=1&cursor={provider_cursor}"
            )),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(provider_stale.status(), StatusCode::CONFLICT);
    let provider_serialized = serde_json::to_string(&provider_first_body)?;
    for forbidden in [
        "ciphertext",
        "secret",
        "base_url",
        "client_key_digest",
        "request_body",
    ] {
        assert!(!provider_serialized.contains(forbidden));
    }

    let action = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/operations/provider-account-pools/actions")
                .set_json(serde_json::json!({
                    "provider_id": "grok",
                    "channel_id": "channel-build",
                    "account_id": "grok-account-a",
                    "action": "cool_down",
                    "cooldown_ms": 1000
                })),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(action.status(), StatusCode::ACCEPTED);
    assert_eq!(
        action.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    let action_body: Value = test::read_body_json(action).await;
    assert_eq!(action_body["state"], "cooling");
    assert!(action_body["observed_at_ms"].as_i64().is_some());
    assert_eq!(
        action_body["cooldown_until_ms"].as_i64(),
        action_body["observed_at_ms"]
            .as_i64()
            .and_then(|value| value.checked_add(1000))
    );

    let stale_action = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/operations/provider-account-pools/actions")
                .set_json(serde_json::json!({
                    "provider_id": "grok",
                    "channel_id": "channel-build",
                    "account_id": "grok-account-a",
                    "action": "request_recovery"
                })),
            "inventory-v2",
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale_action.status(), StatusCode::CONFLICT);

    let invalid_action = test::call_service(
        &app,
        authorized(
            test::TestRequest::post()
                .uri("/admin/operations/provider-account-pools/actions")
                .set_json(serde_json::json!({
                    "provider_id": "grok",
                    "channel_id": "channel-build",
                    "account_id": "grok-account-a",
                    "action": "cool_down",
                    "cooldown_ms": 999
                })),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(invalid_action.status(), StatusCode::BAD_REQUEST);

    let failures = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(
                "/admin/operations/provider-account-pools/failures?provider_id=provider-inventory&channel_id=channel-inventory&account_id=account-b",
            ),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(failures.status(), StatusCode::OK);
    assert_eq!(
        failures.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    let failures_body: Value = test::read_body_json(failures).await;
    assert_eq!(failures_body["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        failures_body["items"][0]["provider_id"],
        "provider-inventory"
    );
    assert_eq!(failures_body["items"][0]["channel_id"], "channel-inventory");
    assert_eq!(failures_body["items"][0]["account_id"], "account-b");
    assert_eq!(
        failures_body["items"][0]["error_code"],
        "CredentialUnauthorized"
    );
    assert_eq!(failures_body["items"][0]["error_scope"], "credential");
    assert_eq!(failures_body["items"][0]["retry_decision"], "non_retryable");
    let failures_serialized = serde_json::to_string(&failures_body)?;
    for forbidden in [
        "private-model-must-not-leak",
        "secret",
        "base_url",
        "header",
        "cookie",
        "request_body",
    ] {
        assert!(!failures_serialized.contains(forbidden));
    }

    let failures_without_version = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/operations/provider-account-pools/failures")
            .peer_addr(loopback())
            .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
            .to_request(),
    )
    .await;
    assert_eq!(failures_without_version.status(), StatusCode::BAD_REQUEST);

    let failures_unknown_version = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/provider-account-pools/failures"),
            "missing-version",
        )
        .to_request(),
    )
    .await;
    assert_eq!(failures_unknown_version.status(), StatusCode::NOT_FOUND);

    let failures_unknown_query = test::call_service(
        &app,
        authorized(
            test::TestRequest::get()
                .uri("/admin/operations/provider-account-pools/failures?unknown=true"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(failures_unknown_query.status(), StatusCode::BAD_REQUEST);

    let usage = test::call_service(
        &app,
        authorized(
            test::TestRequest::get()
                .uri("/admin/operations/usage?protocol=openai_responses&model=usage-public-model"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(usage.status(), StatusCode::OK);
    assert_eq!(
        usage.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    let usage_body: Value = test::read_body_json(usage).await;
    assert_eq!(usage_body["items"][0]["request_count"], 1);
    assert_eq!(usage_body["items"][0]["input_tokens"]["total"], 11);
    assert_eq!(
        usage_body["items"][0]["input_tokens"]["confidence"],
        "exact"
    );
    assert_eq!(usage_body["items"][0]["cost_microunits"], Value::Null);
    assert_eq!(usage_body["items"][0]["cost_confidence"], "unpriced");
    let usage_serialized = serde_json::to_string(&usage_body)?;
    for forbidden in [
        "usage-private-model",
        "secret-upstream.example",
        "secret-must-not-leak",
        "request body",
    ] {
        assert!(!usage_serialized.contains(forbidden));
    }

    let stale = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(&format!(
                "/admin/operations/account-pools?limit=1&cursor={cursor}"
            )),
            "inventory-v2",
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);

    let invalid_query = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/account-pools?unknown=true"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(invalid_query.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid_query.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );

    let duplicate_query = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/account-pools?limit=1&limit=2"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(duplicate_query.status(), StatusCode::BAD_REQUEST);

    let invalid_usage_query = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/usage?limit=0"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(invalid_usage_query.status(), StatusCode::BAD_REQUEST);

    let unknown_usage_query = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/usage?unknown=true"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(unknown_usage_query.status(), StatusCode::BAD_REQUEST);

    let duplicate_usage_query = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/usage?limit=1&limit=2"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(duplicate_usage_query.status(), StatusCode::BAD_REQUEST);

    let billing = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/billing?status=unpriced"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(billing.status(), StatusCode::OK);
    assert_eq!(
        billing.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    let billing_body: Value = test::read_body_json(billing).await;
    assert_eq!(
        billing_body["items"][0]["provider_id"],
        "provider-inventory"
    );
    assert_eq!(billing_body["items"][0]["cost_confidence"], "unpriced");
    assert_eq!(billing_body["summary"]["unpriced_records"], 1);
    let billing_serialized = serde_json::to_string(&billing_body)?;
    for forbidden in ["source_event_id", "source_fingerprint", "encrypted_secret"] {
        assert!(!billing_serialized.contains(forbidden));
    }

    let invalid_billing_query = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/billing?status=not-a-status"),
            "inventory-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(invalid_billing_query.status(), StatusCode::BAD_REQUEST);

    let usage_denied = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/operations/usage")
            .peer_addr(loopback())
            .to_request(),
    )
    .await;
    assert_eq!(usage_denied.status(), StatusCode::NOT_FOUND);

    let denied = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/operations/account-pools")
            .peer_addr(loopback())
            .insert_header(("X-Config-Version", "inventory-v1"))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);
    Ok(())
}
