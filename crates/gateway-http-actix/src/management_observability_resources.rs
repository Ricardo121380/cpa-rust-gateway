//! Protected P12 observability exposition for the management listener.
//!
//! This module renders only the frozen bounded Prometheus counters that the background telemetry
//! consumer aggregates after events leave the request path. It never reads the durable event log,
//! never blocks on `SQLite`, and adds no request-scoped or target-scoped label.

#![deny(unsafe_code)]

use std::sync::Arc;

use actix_web::{HttpResponse, web};
use gateway_observability::{BoundedEventQueue, PrometheusMetrics};

use crate::management_security::configure_management;

/// Prometheus text exposition content type served to management scrapes.
const PROMETHEUS_TEXT_CONTENT_TYPE: &str = "text/plain; version=0.0.4";

/// Durable-writer counters a scrape may mirror without depending on the store crate.
///
/// The composition owns the writer; this port only reads its already-published atomics, so an
/// implementation must never touch `SQLite` or block.
pub trait DurabilityMetricsSource: Send + Sync {
    /// Returns `(required_events_quarantined, retryable_write_failures, pending_required)`.
    fn durability_counters(&self) -> (u64, u64, u64);
}

/// Management-listener state for the read-only bounded metrics exposition.
pub struct ManagementObservabilityHttpState {
    metrics: Arc<PrometheusMetrics>,
    event_queue: Arc<BoundedEventQueue>,
    durability: Option<Arc<dyn DurabilityMetricsSource>>,
}

impl ManagementObservabilityHttpState {
    /// Creates the exposition state from the serve composition's shared telemetry registry and
    /// its bounded producer queue.
    ///
    /// The queue handle is read-only here: scrapes mirror its explicit admission counters into
    /// the Prometheus snapshot and never emit, drain, or close the queue.
    #[must_use]
    pub fn new(metrics: Arc<PrometheusMetrics>, event_queue: Arc<BoundedEventQueue>) -> Self {
        Self {
            metrics,
            event_queue,
            durability: None,
        }
    }

    /// Adds the durable writer's counter handle so scrapes can observe quarantined Required
    /// events, retryable write failures, and the pending batch depth.
    ///
    /// The handle reads writer-owned atomics only: a scrape still never touches `SQLite` and
    /// never blocks the writer.
    #[must_use]
    pub fn with_durability(mut self, durability: Arc<dyn DurabilityMetricsSource>) -> Self {
        self.durability = Some(durability);
        self
    }
}

/// Registers the protected observability routes behind the management security middleware.
pub fn configure_management_observability(config: &mut web::ServiceConfig) {
    configure_management(config, configure_protected_observability_routes);
}

pub(crate) fn configure_protected_observability_routes(config: &mut web::ServiceConfig) {
    config.route("/observability/metrics", web::get().to(metrics_exposition));
}

async fn metrics_exposition(state: web::Data<ManagementObservabilityHttpState>) -> HttpResponse {
    state
        .metrics
        .observe_queue_metrics(state.event_queue.metrics());
    if let Some(durability) = state.durability.as_ref() {
        let (quarantined, write_failures, pending) = durability.durability_counters();
        state
            .metrics
            .observe_durability(quarantined, write_failures, pending);
    }
    HttpResponse::Ok()
        .content_type(PROMETHEUS_TEXT_CONTENT_TYPE)
        .body(state.metrics.render_prometheus())
}
