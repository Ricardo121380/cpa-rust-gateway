//! P12 management-listener observability exposition regression tests.

#![deny(unsafe_code)]

use std::{error::Error, net::SocketAddr, sync::Arc};

use actix_web::{
    App,
    http::{StatusCode, header},
    test, web,
};
use gateway_core::{
    ClientKeyId, EventEmission, GatewayEvent, GatewayEventSink, GatewayProtocol, RequestEvent,
    RequestId,
};
use gateway_http_actix::{
    management_observability_resources::{
        ManagementObservabilityHttpState, configure_management_observability,
    },
    management_security::{
        MANAGEMENT_KEY_HEADER, ManagementBrowserPolicy, ManagementHttpState, ManagementKey,
        ManagementNetworkPolicy,
    },
};
use gateway_observability::{BoundedEventQueue, EventQueueConfig, PrometheusMetrics};

type TestResult = Result<(), Box<dyn Error>>;

const MANAGEMENT_KEY: &str = "mgmt_0123456789abcdefghijklmnopqrstuvwxyz";

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 44_412))
}

fn security_state() -> Result<ManagementHttpState, Box<dyn Error>> {
    Ok(ManagementHttpState::new(
        ManagementKey::try_new(MANAGEMENT_KEY)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?)
}

fn request_event(value: &str) -> Result<GatewayEvent, Box<dyn Error>> {
    Ok(GatewayEvent::Request(RequestEvent::new(
        RequestId::try_new(format!("p12-metrics-request-{value}"))?,
        ClientKeyId::try_new("p12-metrics-client")?,
        None,
        GatewayProtocol::OpenAiResponses,
        "p12-metrics-requested-model".to_owned(),
        "p12-metrics-public-model".to_owned(),
        None,
        false,
    )))
}

#[actix_web::test]
async fn metrics_exposition_serves_bounded_counters_after_traffic_and_overflow() -> TestResult {
    let metrics = Arc::new(PrometheusMetrics::default());
    let (queue, _receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
    let queue = Arc::new(queue);
    assert_eq!(
        queue.try_emit(request_event("one")?),
        EventEmission::Enqueued
    );
    assert_eq!(
        queue.try_emit(request_event("two")?),
        EventEmission::RequiredQueueFull
    );
    metrics.observe_event(&request_event("one")?);
    let state = ManagementObservabilityHttpState::new(Arc::clone(&metrics), Arc::clone(&queue));
    let storage = state.storage_monitor();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security_state()?))
            .app_data(web::Data::new(state))
            .configure(configure_management_observability),
    )
    .await;

    let unauthorized = test::TestRequest::get()
        .uri("/admin/observability/metrics")
        .peer_addr(loopback())
        .to_request();
    assert_eq!(
        test::call_service(&app, unauthorized).await.status(),
        StatusCode::NOT_FOUND
    );

    let authorized = test::TestRequest::get()
        .uri("/admin/observability/metrics")
        .peer_addr(loopback())
        .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
        .to_request();
    let response = test::call_service(&app, authorized).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/plain; version=0.0.4")
    );
    let body = String::from_utf8(test::read_body(response).await.to_vec())?;
    assert!(body.contains("gateway_observability_events_total{kind=\"request\"} 1"));
    assert!(body.contains(
        "gateway_observability_queue_admission_total{outcome=\"required_queue_full\"} 1"
    ));
    for expected in [
        "gateway_recording_accepting_requests 0",
        "gateway_recording_state 0",
        "gateway_recording_pending_required 1",
        "gateway_recording_last_commit_ms 0",
        "gateway_recording_confirmation_failures_total 1",
        "gateway_recording_recovered_unknown 0",
        "gateway_recording_queue_capacity 1",
        "gateway_recording_queue_remaining 0",
        "gateway_recording_queue_used 1",
        "gateway_storage_collection_failed 0",
        "gateway_storage_observed 0",
    ] {
        assert!(body.contains(expected), "missing {expected}");
    }
    assert!(!body.contains("p12-metrics-request-one"));
    assert!(!body.contains("p12-metrics-requested-model"));
    assert!(!body.contains("gateway_storage_available_bytes"));
    storage.observe(Some(gateway_observability::StorageCapacitySnapshot {
        observed_at_ms: 1234,
        database_bytes: 100,
        wal_bytes: 300 * 1024 * 1024,
        available_bytes: 0,
        total_bytes: 1024 * 1024 * 1024,
    }));
    storage.observe(None);
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/observability/metrics")
            .peer_addr(loopback())
            .insert_header((MANAGEMENT_KEY_HEADER, MANAGEMENT_KEY))
            .to_request(),
    )
    .await;
    let body = String::from_utf8(test::read_body(response).await.to_vec())?;
    for expected in [
        "gateway_storage_observed_at_ms 1234",
        "gateway_storage_collection_failed 1",
        "gateway_storage_observed 1",
        "gateway_storage_available_bytes 0",
        "gateway_storage_disk_low 1",
        "gateway_storage_wal_high 1",
    ] {
        assert!(body.contains(expected), "missing {expected}");
    }
    Ok(())
}
