//! P13-11E4 protected Provider-specific egress status projection tests.

#![deny(unsafe_code)]

use std::{error::Error, net::SocketAddr};

use actix_web::{
    App,
    http::{StatusCode, header},
    test, web,
};
use gateway_control::{
    management_mutation_service::{
        ConfigRevision, ConfigVersion, ConfigVersionId, ConfigVersionStatus,
        ControlPlaneConfiguration, KeyVersion, ManagementMutationService, MasterKey, MasterKeyRing,
        SecretStore, SqliteControlPlaneRepository,
    },
    provider_egress_status_service::{
        ProviderEgressStatusChannelIdentity, ProviderEgressStatusChannelKind,
        ProviderEgressStatusClearanceItem, ProviderEgressStatusEgressItem,
        ProviderEgressStatusError, ProviderEgressStatusFacade, ProviderEgressStatusItem,
        ProviderEgressStatusPage, ProviderEgressStatusQuery, ProviderEgressStatusSessionItem,
        ProviderEgressStatusSnapshot, ProviderEgressStatusState, ProviderEgressStatusTarget,
        SnapshotProviderEgressStatusFacade,
    },
};
use gateway_core::{CredentialId, EndpointId, ProviderId, UpstreamId};
use gateway_http_actix::{
    management_resources::{ManagementResourceHttpState, configure_management_resources},
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";

struct InvalidSnapshotFacade;

impl ProviderEgressStatusFacade for InvalidSnapshotFacade {
    fn list_provider_egress_status(
        &self,
        _config_version_id: &ConfigVersionId,
        _config_revision: ConfigRevision,
        _query: &ProviderEgressStatusQuery,
    ) -> Result<ProviderEgressStatusPage, ProviderEgressStatusError> {
        Err(ProviderEgressStatusError::InvalidSnapshot)
    }
}

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 45_407))
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

fn assert_value_free_json(value: &Value) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                assert!(
                    ![
                        "url",
                        "proxy_url",
                        "proxy_endpoint",
                        "headers",
                        "cookie",
                        "token",
                        "secret",
                        "plaintext",
                        "ciphertext",
                        "request_body",
                        "raw_error",
                        "ticket",
                        "generation",
                    ]
                    .contains(&key.as_str()),
                    "forbidden response field {key}"
                );
                assert_value_free_json(child);
            }
        }
        Value::Array(values) => values.iter().for_each(assert_value_free_json),
        Value::String(value) => {
            assert!(!value.contains("sentinel-secret-material"));
            assert!(!value.starts_with("http://"));
            assert!(!value.starts_with("https://"));
            assert!(!value.starts_with("socks5://"));
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn resource_state() -> Result<ManagementResourceHttpState, Box<dyn Error>> {
    let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
    let key_version = KeyVersion::try_new(1)?;
    let key_ring = MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x4a_u8; 32])?)],
    )?;
    let secret_store = SecretStore::new(key_ring);
    repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
        id: ConfigVersionId::try_new("egress-status-v1")?,
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision: 7,
        created_at_ms: 1,
        description: "P13-11E4 HTTP fixture".to_owned(),
    }))?;
    repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
        id: ConfigVersionId::try_new("egress-status-v2")?,
        parent_id: Some(ConfigVersionId::try_new("egress-status-v1")?),
        status: ConfigVersionStatus::Draft,
        revision: 8,
        created_at_ms: 2,
        description: "P13-11E4 conflicting HTTP fixture".to_owned(),
    }))?;

    let build_channel = ProviderEgressStatusChannelIdentity::try_new(
        ProviderId::try_new("grok.build")?,
        UpstreamId::try_new("upstream-build")?,
        EndpointId::try_new("channel-build")?,
        ProviderEgressStatusChannelKind::GrokBuild,
    )?;
    let console_channel = ProviderEgressStatusChannelIdentity::try_new(
        ProviderId::try_new("grok.console")?,
        UpstreamId::try_new("upstream-console")?,
        EndpointId::try_new("channel-console")?,
        ProviderEgressStatusChannelKind::GrokConsole,
    )?;
    let web_channel = ProviderEgressStatusChannelIdentity::try_new(
        ProviderId::try_new("grok.web")?,
        UpstreamId::try_new("upstream-web")?,
        EndpointId::try_new("channel-web")?,
        ProviderEgressStatusChannelKind::GrokWeb,
    )?;
    let build_item = ProviderEgressStatusItem::Egress(ProviderEgressStatusEgressItem::try_new(
        build_channel,
        ProviderEgressStatusTarget::direct(),
        ProviderEgressStatusState::Available,
        None,
    )?);
    let session_item = ProviderEgressStatusItem::Session(ProviderEgressStatusSessionItem::try_new(
        console_channel,
        CredentialId::try_new("console-account")?,
        3,
        4,
        ProviderEgressStatusState::Absent,
        None,
    )?);
    let clearance_item =
        ProviderEgressStatusItem::Clearance(ProviderEgressStatusClearanceItem::try_new(
            web_channel,
            CredentialId::try_new("web-account")?,
            5,
            6,
            ProviderEgressStatusTarget::named("opaque-target-1")?,
            7,
            ProviderEgressStatusState::Fresh,
            Some(1_000),
        )?);
    let snapshot = ProviderEgressStatusSnapshot::try_new(
        ConfigVersionId::try_new("egress-status-v1")?,
        ConfigRevision::try_new(7)?,
        2,
        "status-snapshot-1",
        100,
        vec![build_item, session_item, clearance_item],
    )?;
    Ok(
        ManagementResourceHttpState::new(ManagementMutationService::new(repository, secret_store))
            .with_provider_egress_status(Box::new(SnapshotProviderEgressStatusFacade::new(
                snapshot,
            ))),
    )
}

#[actix_web::test]
async fn status_is_version_bound_paginated_and_value_free() -> TestResult {
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
            test::TestRequest::get().uri("/admin/operations/provider-egress-status?limit=1"),
            "egress-status-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(
        first.headers().get(header::ETAG),
        Some(&header::HeaderValue::from_static("\"rev-7\""))
    );
    assert_eq!(
        first.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    let first_body: Value = test::read_body_json(first).await;
    assert_eq!(first_body["config_version_id"], "egress-status-v1");
    assert_eq!(first_body["config_revision"], 7);
    assert_eq!(first_body["runtime_revision"], 2);
    assert_eq!(first_body["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(first_body["items"][0]["domain"], "egress");
    assert_eq!(first_body["items"][0]["target_kind"], "direct");
    assert!(first_body["items"][0]["target_id"].is_null());
    let cursor = first_body["next_cursor"]
        .as_str()
        .ok_or("missing status cursor")?;

    let second_uri = format!("/admin/operations/provider-egress-status?limit=1&cursor={cursor}");
    let second = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(&second_uri),
            "egress-status-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    let second_body: Value = test::read_body_json(second).await;
    assert_eq!(second_body["items"][0]["domain"], "session");
    assert_eq!(second_body["items"][0]["credential_id"], "console-account");
    assert_eq!(second_body["snapshot_id"], first_body["snapshot_id"]);
    assert_eq!(second_body["sampled_at_ms"], first_body["sampled_at_ms"]);

    let clearance = test::call_service(
        &app,
        authorized(
            test::TestRequest::get()
                .uri("/admin/operations/provider-egress-status?domain=clearance"),
            "egress-status-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(clearance.status(), StatusCode::OK);
    let clearance_body: Value = test::read_body_json(clearance).await;
    assert_eq!(clearance_body["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(clearance_body["items"][0]["domain"], "clearance");
    assert_eq!(clearance_body["items"][0]["channel_kind"], "grok_web");
    assert_eq!(clearance_body["items"][0]["target_kind"], "named");
    assert_eq!(clearance_body["items"][0]["target_id"], "opaque-target-1");
    assert_value_free_json(&first_body);
    assert_value_free_json(&second_body);
    assert_value_free_json(&clearance_body);
    Ok(())
}

#[actix_web::test]
async fn status_rejects_duplicate_or_incompatible_queries_and_stale_config() -> TestResult {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(resource_state()?))
            .configure(configure_management_resources),
    )
    .await;

    for uri in [
        "/admin/operations/provider-egress-status?limit=1&limit=2",
        "/admin/operations/provider-egress-status?domain=egress&state=active",
    ] {
        let response = test::call_service(
            &app,
            authorized(test::TestRequest::get().uri(uri), "egress-status-v1").to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
    }

    let stale = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/provider-egress-status"),
            "missing-version",
        )
        .to_request(),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        stale.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    Ok(())
}

#[actix_web::test]
async fn status_enforces_admission_exact_filters_and_snapshot_cursor_conflicts() -> TestResult {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(resource_state()?))
            .configure(configure_management_resources),
    )
    .await;

    let missing_key = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/operations/provider-egress-status")
            .peer_addr(loopback())
            .insert_header(("X-Config-Version", "egress-status-v1"))
            .to_request(),
    )
    .await;
    assert_eq!(missing_key.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        missing_key.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );

    let denied_origin = test::call_service(
        &app,
        authorized(
            test::TestRequest::get()
                .uri("/admin/operations/provider-egress-status")
                .insert_header((header::ORIGIN, "https://untrusted.invalid")),
            "egress-status-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(denied_origin.status(), StatusCode::NOT_FOUND);

    let exact = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(
                "/admin/operations/provider-egress-status?provider_id=grok.console&upstream_id=upstream-console&channel_id=channel-console&domain=session&state=absent&credential_id=console-account",
            ),
            "egress-status-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(exact.status(), StatusCode::OK);
    let exact_body: Value = test::read_body_json(exact).await;
    assert_eq!(exact_body["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(exact_body["items"][0]["domain"], "session");
    assert_eq!(exact_body["items"][0]["provider_id"], "grok.console");
    assert_eq!(exact_body["items"][0]["channel_id"], "channel-console");
    assert_eq!(exact_body["items"][0]["state"], "absent");

    let first = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/provider-egress-status?limit=1"),
            "egress-status-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_body: Value = test::read_body_json(first).await;
    let cursor = first_body["next_cursor"]
        .as_str()
        .ok_or("missing status cursor")?;
    let incompatible_uri =
        format!("/admin/operations/provider-egress-status?limit=1&domain=session&cursor={cursor}");
    let incompatible = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(&incompatible_uri),
            "egress-status-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(incompatible.status(), StatusCode::CONFLICT);
    assert_eq!(
        incompatible.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );

    let config_conflict = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/provider-egress-status"),
            "egress-status-v2",
        )
        .to_request(),
    )
    .await;
    assert_eq!(config_conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        config_conflict.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );

    for uri in [
        "/admin/operations/provider-egress-status?unknown=value",
        "/admin/operations/provider-egress-status?limit=101",
        "/admin/operations/provider-egress-status?cursor=not-base64!",
    ] {
        let response = test::call_service(
            &app,
            authorized(test::TestRequest::get().uri(uri), "egress-status-v1").to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
    }

    Ok(())
}

#[actix_web::test]
async fn status_default_facade_is_fail_closed_without_provider_calls() -> TestResult {
    let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
    let key_version = KeyVersion::try_new(1)?;
    let key_ring = MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x4b_u8; 32])?)],
    )?;
    let secret_store = SecretStore::new(key_ring);
    repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
        id: ConfigVersionId::try_new("rejecting-v1")?,
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision: 1,
        created_at_ms: 1,
        description: "rejecting fixture".to_owned(),
    }))?;
    let state =
        ManagementResourceHttpState::new(ManagementMutationService::new(repository, secret_store));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_resources),
    )
    .await;
    let response = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/provider-egress-status"),
            "rejecting-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );

    let invalid_app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(
                resource_state()?.with_provider_egress_status(Box::new(InvalidSnapshotFacade)),
            ))
            .configure(configure_management_resources),
    )
    .await;
    let invalid = test::call_service(
        &invalid_app,
        authorized(
            test::TestRequest::get().uri("/admin/operations/provider-egress-status"),
            "egress-status-v1",
        )
        .to_request(),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        invalid.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
    Ok(())
}
