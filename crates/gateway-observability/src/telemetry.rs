//! Bounded structured JSON, Prometheus, and OpenTelemetry export from gateway events.
//!
//! The request path continues to call only [`gateway_core::GatewayEventSink::try_emit`]. This
//! module converts events after the existing bounded receiver has admitted them, and calls
//! explicitly non-blocking export sinks. It deliberately does not read request bodies, headers,
//! URLs, credentials, raw model labels, or free-form upstream diagnostics.

use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use gateway_core::{
    AttemptOutcome, AttemptRetryDecision, ErrorScope, GatewayErrorCode, GatewayEvent,
    GatewayProtocol, HealthEventKind,
};
use opentelemetry::trace::{SpanContext, SpanId, TraceFlags, TraceId, TraceState};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::EventQueueMetrics;

/// Schema version of [`StructuredJsonRecord`] and [`OpenTelemetrySpan`] JSON output.
pub const TELEMETRY_SCHEMA_VERSION: u8 = 1;
/// Stable service name attached to every OpenTelemetry-compatible span record.
pub const OTEL_SERVICE_NAME: &str = "cpa-rust-gateway";
/// Stable instrumentation scope attached to every OpenTelemetry-compatible span record.
pub const OTEL_SCOPE_NAME: &str = "gateway-observability";

/// One secret-safe telemetry event category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventKind {
    /// A client request entered gateway execution.
    Request,
    /// An upstream Attempt reached a terminal decision.
    Attempt,
    /// A final usage observation reached the canonical response path.
    Usage,
    /// A sanitized runtime-health transition occurred.
    Health,
    /// A low-priority safe diagnostic occurred.
    Diagnostic,
}

impl TelemetryEventKind {
    /// Returns the frozen Prometheus label and structured-log encoding.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Attempt => "attempt",
            Self::Usage => "usage",
            Self::Health => "health",
            Self::Diagnostic => "diagnostic",
        }
    }
}

/// Sanitized terminal Attempt result retained by telemetry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryAttemptOutcome {
    /// The upstream Attempt completed successfully.
    Succeeded,
    /// The upstream Attempt ended in one stable safe error category.
    Failed,
}

/// Structured fields shared by JSON logs and OpenTelemetry spans.
///
/// This type intentionally omits request body, response body, headers, URLs, Client Key material,
/// Credential material, endpoint identity, and model labels. Prometheus never uses these fields as
/// labels, preventing request-scoped cardinality from entering its registry.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryAttributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<GatewayProtocol>,
    #[serde(skip_serializing_if = "Option::is_none")]
    streaming: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attempt_outcome: Option<TelemetryAttemptOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_decision: Option<AttemptRetryDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<GatewayErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_scope: Option<ErrorScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_read_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_creation_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cached_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    health_kind: Option<HealthEventKind>,
}

impl TelemetryAttributes {
    /// Returns the request correlation identifier when this event belongs to a request trace.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
}

/// W3C-compatible trace identifiers rendered as fixed-width lowercase hexadecimal strings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TraceCorrelation {
    trace_id: String,
    span_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_span_id: Option<String>,
    sampled: bool,
}

impl TraceCorrelation {
    fn from_span(span: &OpenTelemetrySpan) -> Self {
        Self {
            trace_id: span.span_context.trace_id().to_string(),
            span_id: span.span_context.span_id().to_string(),
            parent_span_id: span.parent_span_id.map(|value| value.to_string()),
            sampled: span.span_context.is_sampled(),
        }
    }
}

/// One JSON record passed to a structured tracing sink.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredJsonRecord {
    schema_version: u8,
    event_kind: TelemetryEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<TraceCorrelation>,
    attributes: TelemetryAttributes,
}

impl StructuredJsonRecord {
    /// Translates one already-sanitized gateway event into a JSON-safe record.
    #[must_use]
    pub fn from_event(event: &GatewayEvent) -> Self {
        let span = OpenTelemetrySpan::from_event(event);
        Self::from_event_and_span(event, span.as_ref())
    }

    fn from_event_and_span(event: &GatewayEvent, span: Option<&OpenTelemetrySpan>) -> Self {
        let (event_kind, attributes) = event_kind_and_attributes(event);
        Self {
            schema_version: TELEMETRY_SCHEMA_VERSION,
            event_kind,
            trace: span.map(TraceCorrelation::from_span),
            attributes,
        }
    }

    /// Returns the stable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u8 {
        self.schema_version
    }

    /// Returns the stable event category.
    #[must_use]
    pub const fn event_kind(&self) -> TelemetryEventKind {
        self.event_kind
    }

    /// Returns the request identifier only when it is part of the safe event contract.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.attributes.request_id()
    }

    /// Returns the W3C trace identifier when this event has trace correlation.
    #[must_use]
    pub fn trace_id(&self) -> Option<&str> {
        self.trace.as_ref().map(|trace| trace.trace_id.as_str())
    }

    /// Serializes a single structured JSON line without adding arbitrary text fields.
    ///
    /// # Errors
    ///
    /// Returns a serialization error without emitting a partial record.
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// OpenTelemetry span kind emitted by the bounded telemetry pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenTelemetrySpanKind {
    /// A client-facing request entered the gateway.
    Server,
    /// The gateway made or completed one upstream Attempt.
    Client,
    /// A local usage or health transition occurred.
    Internal,
}

/// An OpenTelemetry-compatible span export record.
///
/// It owns a real OpenTelemetry [`SpanContext`] while leaving actual transport to an injected,
/// non-blocking [`OpenTelemetryExporter`]. This makes local tests deterministic and prevents an
/// HTTP exporter from entering the request path.
#[derive(Clone, Debug)]
pub struct OpenTelemetrySpan {
    span_context: SpanContext,
    parent_span_id: Option<SpanId>,
    name: &'static str,
    kind: OpenTelemetrySpanKind,
    attributes: TelemetryAttributes,
}

impl OpenTelemetrySpan {
    /// Translates a gateway event into one OpenTelemetry-compatible span when it has a stable
    /// correlation key. Diagnostics deliberately remain log-and-metric-only.
    #[must_use]
    pub fn from_event(event: &GatewayEvent) -> Option<Self> {
        match event {
            GatewayEvent::Request(request) => {
                let request_id = request.request_id().as_str();
                Some(Self::new(
                    trace_id_for("request", request_id),
                    root_span_id_for(request_id),
                    None,
                    "gateway.request",
                    OpenTelemetrySpanKind::Server,
                    attributes_for_request(request),
                ))
            }
            GatewayEvent::Attempt(attempt) => {
                let request_id = attempt.request_id().as_str();
                Some(Self::new(
                    trace_id_for("request", request_id),
                    span_id_for("attempt", attempt.attempt_id().as_str()),
                    Some(root_span_id_for(request_id)),
                    "gateway.upstream.attempt",
                    OpenTelemetrySpanKind::Client,
                    attributes_for_attempt(attempt),
                ))
            }
            GatewayEvent::Usage(usage) => {
                let request_id = usage.request_id().as_str();
                Some(Self::new(
                    trace_id_for("request", request_id),
                    span_id_for("usage", usage.response_id().as_str()),
                    Some(root_span_id_for(request_id)),
                    "gateway.usage",
                    OpenTelemetrySpanKind::Internal,
                    attributes_for_usage(usage),
                ))
            }
            GatewayEvent::Health(health) => {
                let health_id = health.health_event_id().as_str();
                Some(Self::new(
                    trace_id_for("health", health_id),
                    span_id_for("health", health_id),
                    None,
                    "gateway.health",
                    OpenTelemetrySpanKind::Internal,
                    attributes_for_health(health),
                ))
            }
            GatewayEvent::Diagnostic(_) => None,
        }
    }

    fn new(
        trace_id: TraceId,
        span_id: SpanId,
        parent_span_id: Option<SpanId>,
        name: &'static str,
        kind: OpenTelemetrySpanKind,
        attributes: TelemetryAttributes,
    ) -> Self {
        Self {
            span_context: SpanContext::new(
                trace_id,
                span_id,
                TraceFlags::SAMPLED,
                false,
                TraceState::NONE,
            ),
            parent_span_id,
            name,
            kind,
            attributes,
        }
    }

    /// Returns the W3C-compatible OpenTelemetry context for this export.
    #[must_use]
    pub const fn span_context(&self) -> &SpanContext {
        &self.span_context
    }

    /// Returns the request-root parent when this is a child span.
    #[must_use]
    pub const fn parent_span_id(&self) -> Option<SpanId> {
        self.parent_span_id
    }

    /// Returns the stable operation name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the OpenTelemetry span kind.
    #[must_use]
    pub const fn kind(&self) -> OpenTelemetrySpanKind {
        self.kind
    }

    /// Returns the secret-safe attributes shared with structured logs.
    #[must_use]
    pub const fn attributes(&self) -> &TelemetryAttributes {
        &self.attributes
    }

    /// Serializes one exporter-ready OpenTelemetry JSON record.
    ///
    /// The record contains W3C `trace_id`/`span_id` fields plus stable service and instrumentation
    /// scope names. A transport adapter can enqueue it to OTLP without touching the request path.
    ///
    /// # Errors
    ///
    /// Returns a serialization error without emitting a partial span record.
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        #[derive(Serialize)]
        struct ExportRecord<'a> {
            schema_version: u8,
            service_name: &'static str,
            instrumentation_scope: &'static str,
            trace_id: String,
            span_id: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            parent_span_id: Option<String>,
            name: &'static str,
            kind: OpenTelemetrySpanKind,
            attributes: &'a TelemetryAttributes,
        }

        serde_json::to_string(&ExportRecord {
            schema_version: TELEMETRY_SCHEMA_VERSION,
            service_name: OTEL_SERVICE_NAME,
            instrumentation_scope: OTEL_SCOPE_NAME,
            trace_id: self.span_context.trace_id().to_string(),
            span_id: self.span_context.span_id().to_string(),
            parent_span_id: self.parent_span_id.map(|value| value.to_string()),
            name: self.name,
            kind: self.kind,
            attributes: &self.attributes,
        })
    }
}

/// Outcome of a non-blocking structured or OpenTelemetry export attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenTelemetryExportOutcome {
    /// The sink accepted the record for logging or a bounded later export.
    Emitted,
    /// The sink is intentionally disabled.
    Disabled,
    /// The sink rejected the record without blocking the background event consumer.
    Rejected,
}

/// Non-blocking structured JSON destination owned outside the request path.
pub trait StructuredJsonExporter: Send + Sync {
    /// Attempts to accept a fully sanitized JSON record without waiting for I/O.
    fn try_export(&self, record: &StructuredJsonRecord) -> OpenTelemetryExportOutcome;
}

/// Non-blocking OpenTelemetry destination owned outside the request path.
pub trait OpenTelemetryExporter: Send + Sync {
    /// Attempts to accept one span without waiting for network or a queue slot.
    fn try_export(&self, span: &OpenTelemetrySpan) -> OpenTelemetryExportOutcome;
}

/// Disabled structured JSON destination for embeddings that do not configure tracing output.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopStructuredJsonExporter;

impl StructuredJsonExporter for NoopStructuredJsonExporter {
    fn try_export(&self, _record: &StructuredJsonRecord) -> OpenTelemetryExportOutcome {
        OpenTelemetryExportOutcome::Disabled
    }
}

/// Disabled OpenTelemetry destination for embeddings that do not configure an exporter.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopOpenTelemetryExporter;

impl OpenTelemetryExporter for NoopOpenTelemetryExporter {
    fn try_export(&self, _span: &OpenTelemetrySpan) -> OpenTelemetryExportOutcome {
        OpenTelemetryExportOutcome::Disabled
    }
}

/// JSON `tracing` destination that logs only the serialized safe record.
#[derive(Clone, Copy, Debug, Default)]
pub struct TracingJsonExporter;

impl StructuredJsonExporter for TracingJsonExporter {
    fn try_export(&self, record: &StructuredJsonRecord) -> OpenTelemetryExportOutcome {
        let Ok(payload) = record.to_json_line() else {
            return OpenTelemetryExportOutcome::Rejected;
        };
        tracing::info!(
            target: "gateway_observability::json",
            schema_version = record.schema_version(),
            event_kind = record.event_kind().as_str(),
            request_id = record.request_id().unwrap_or("-"),
            trace_id = record.trace_id().unwrap_or("-"),
            telemetry_json = %payload,
            "gateway structured event"
        );
        OpenTelemetryExportOutcome::Emitted
    }
}

/// Error returned when process-global tracing has already been initialized elsewhere.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonTracingInitError {
    /// A subscriber was already installed by the embedding application or test process.
    AlreadyInitialized,
}

impl fmt::Display for JsonTracingInitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON tracing subscriber is already initialized")
    }
}

impl std::error::Error for JsonTracingInitError {}

/// Installs a process-global JSON `tracing` subscriber when the embedding has not installed one.
///
/// Applications that own tracing globally may install their own subscriber and use
/// [`TracingJsonExporter`] without calling this helper.
///
/// # Errors
///
/// Returns [`JsonTracingInitError::AlreadyInitialized`] if another subscriber already owns the
/// process-global dispatcher.
pub fn try_init_json_tracing() -> Result<(), JsonTracingInitError> {
    tracing_subscriber::fmt()
        .json()
        .with_target(true)
        .with_current_span(false)
        .with_span_list(false)
        .try_init()
        .map_err(|_| JsonTracingInitError::AlreadyInitialized)
}

/// Thread-safe Prometheus counter registry without high-cardinality request labels.
pub struct PrometheusMetrics {
    request_events: AtomicU64,
    attempt_events: AtomicU64,
    usage_events: AtomicU64,
    health_events: AtomicU64,
    diagnostic_events: AtomicU64,
    attempts_succeeded: AtomicU64,
    attempts_failed: AtomicU64,
    input_tokens: AtomicU64,
    output_tokens: AtomicU64,
    reasoning_tokens: AtomicU64,
    cache_read_tokens: AtomicU64,
    cache_creation_tokens: AtomicU64,
    cached_tokens: AtomicU64,
    json_emitted: AtomicU64,
    json_disabled: AtomicU64,
    json_rejected: AtomicU64,
    otel_emitted: AtomicU64,
    otel_disabled: AtomicU64,
    otel_rejected: AtomicU64,
    required_queue_full: AtomicU64,
    required_events_quarantined: AtomicU64,
    durable_write_failures: AtomicU64,
    pending_required: AtomicU64,
    diagnostics_dropped: AtomicU64,
    sink_closed: AtomicU64,
}

impl Default for PrometheusMetrics {
    fn default() -> Self {
        Self {
            request_events: AtomicU64::new(0),
            attempt_events: AtomicU64::new(0),
            usage_events: AtomicU64::new(0),
            health_events: AtomicU64::new(0),
            diagnostic_events: AtomicU64::new(0),
            attempts_succeeded: AtomicU64::new(0),
            attempts_failed: AtomicU64::new(0),
            input_tokens: AtomicU64::new(0),
            output_tokens: AtomicU64::new(0),
            reasoning_tokens: AtomicU64::new(0),
            cache_read_tokens: AtomicU64::new(0),
            cache_creation_tokens: AtomicU64::new(0),
            cached_tokens: AtomicU64::new(0),
            json_emitted: AtomicU64::new(0),
            json_disabled: AtomicU64::new(0),
            json_rejected: AtomicU64::new(0),
            otel_emitted: AtomicU64::new(0),
            otel_disabled: AtomicU64::new(0),
            otel_rejected: AtomicU64::new(0),
            required_queue_full: AtomicU64::new(0),
            required_events_quarantined: AtomicU64::new(0),
            durable_write_failures: AtomicU64::new(0),
            pending_required: AtomicU64::new(0),
            diagnostics_dropped: AtomicU64::new(0),
            sink_closed: AtomicU64::new(0),
        }
    }
}

/// Immutable point-in-time snapshot of Prometheus counter values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PrometheusMetricsSnapshot {
    /// Count by event kind.
    pub request_events: u64,
    /// Count by event kind.
    pub attempt_events: u64,
    /// Count by event kind.
    pub usage_events: u64,
    /// Count by event kind.
    pub health_events: u64,
    /// Count by event kind.
    pub diagnostic_events: u64,
    /// Terminal successful Attempt count.
    pub attempts_succeeded: u64,
    /// Terminal failed Attempt count.
    pub attempts_failed: u64,
    /// Aggregated input tokens.
    pub input_tokens: u64,
    /// Aggregated output tokens.
    pub output_tokens: u64,
    /// Aggregated reasoning tokens.
    pub reasoning_tokens: u64,
    /// Aggregated cache-read tokens.
    pub cache_read_tokens: u64,
    /// Aggregated cache-creation tokens.
    pub cache_creation_tokens: u64,
    /// Aggregated cached-token totals.
    pub cached_tokens: u64,
    /// Structured JSON sink outcomes.
    pub json_emitted: u64,
    /// Structured JSON sink outcomes.
    pub json_disabled: u64,
    /// Structured JSON sink outcomes.
    pub json_rejected: u64,
    /// OpenTelemetry sink outcomes.
    pub otel_emitted: u64,
    /// OpenTelemetry sink outcomes.
    pub otel_disabled: u64,
    /// OpenTelemetry sink outcomes.
    pub otel_rejected: u64,
    /// Latest explicit required queue-full counter from the producer.
    pub required_queue_full: u64,
    /// Required events the durable writer dropped because their identity can never append.
    pub required_events_quarantined: u64,
    /// Retryable durable-writer failures observed since start.
    pub durable_write_failures: u64,
    /// Required events currently retained in the durable writer's pending batch.
    pub pending_required: u64,
    /// Latest explicit diagnostic-drop counter from the producer.
    pub diagnostics_dropped: u64,
    /// Latest explicit closed-sink counter from the producer.
    pub sink_closed: u64,
}

impl PrometheusMetrics {
    /// Records one already-sanitized event after it left the request queue.
    pub fn observe_event(&self, event: &GatewayEvent) {
        match event {
            GatewayEvent::Request(_) => increment(&self.request_events, 1),
            GatewayEvent::Attempt(attempt) => {
                increment(&self.attempt_events, 1);
                match attempt.outcome() {
                    AttemptOutcome::Succeeded => increment(&self.attempts_succeeded, 1),
                    AttemptOutcome::Failed(_) => increment(&self.attempts_failed, 1),
                }
            }
            GatewayEvent::Usage(usage_event) => {
                increment(&self.usage_events, 1);
                let usage = usage_event.usage();
                add_optional(&self.input_tokens, usage.input_tokens);
                add_optional(&self.output_tokens, usage.output_tokens);
                add_optional(&self.reasoning_tokens, usage.reasoning_tokens);
                add_optional(&self.cache_read_tokens, usage.cache_read_tokens);
                add_optional(&self.cache_creation_tokens, usage.cache_creation_tokens);
                add_optional(&self.cached_tokens, usage.cached_tokens);
            }
            GatewayEvent::Health(_) => increment(&self.health_events, 1),
            GatewayEvent::Diagnostic(_) => increment(&self.diagnostic_events, 1),
        }
    }

    /// Mirrors producer-side queue counters into the Prometheus snapshot without modifying them.
    pub fn observe_queue_metrics(&self, metrics: EventQueueMetrics) {
        self.required_queue_full
            .store(metrics.required_queue_full, Ordering::Relaxed);
        self.diagnostics_dropped
            .store(metrics.diagnostics_dropped, Ordering::Relaxed);
        self.sink_closed
            .store(metrics.sink_closed, Ordering::Relaxed);
    }

    /// Mirrors the durable writer's counters into the scraped snapshot.
    ///
    /// The writer owns these atomics; this only copies them, so a scrape still never touches
    /// `SQLite` or blocks on the writer.
    pub fn observe_durability(
        &self,
        required_events_quarantined: u64,
        durable_write_failures: u64,
        pending_required: u64,
    ) {
        self.required_events_quarantined
            .store(required_events_quarantined, Ordering::Relaxed);
        self.durable_write_failures
            .store(durable_write_failures, Ordering::Relaxed);
        self.pending_required
            .store(pending_required, Ordering::Relaxed);
    }

    fn observe_export(&self, sink: TelemetrySink, outcome: OpenTelemetryExportOutcome) {
        let counter = match (sink, outcome) {
            (TelemetrySink::Json, OpenTelemetryExportOutcome::Emitted) => &self.json_emitted,
            (TelemetrySink::Json, OpenTelemetryExportOutcome::Disabled) => &self.json_disabled,
            (TelemetrySink::Json, OpenTelemetryExportOutcome::Rejected) => &self.json_rejected,
            (TelemetrySink::OpenTelemetry, OpenTelemetryExportOutcome::Emitted) => {
                &self.otel_emitted
            }
            (TelemetrySink::OpenTelemetry, OpenTelemetryExportOutcome::Disabled) => {
                &self.otel_disabled
            }
            (TelemetrySink::OpenTelemetry, OpenTelemetryExportOutcome::Rejected) => {
                &self.otel_rejected
            }
        };
        increment(counter, 1);
    }

    /// Returns an immutable metrics snapshot without resetting any counter.
    #[must_use]
    pub fn snapshot(&self) -> PrometheusMetricsSnapshot {
        PrometheusMetricsSnapshot {
            request_events: self.request_events.load(Ordering::Relaxed),
            attempt_events: self.attempt_events.load(Ordering::Relaxed),
            usage_events: self.usage_events.load(Ordering::Relaxed),
            health_events: self.health_events.load(Ordering::Relaxed),
            diagnostic_events: self.diagnostic_events.load(Ordering::Relaxed),
            attempts_succeeded: self.attempts_succeeded.load(Ordering::Relaxed),
            attempts_failed: self.attempts_failed.load(Ordering::Relaxed),
            input_tokens: self.input_tokens.load(Ordering::Relaxed),
            output_tokens: self.output_tokens.load(Ordering::Relaxed),
            reasoning_tokens: self.reasoning_tokens.load(Ordering::Relaxed),
            cache_read_tokens: self.cache_read_tokens.load(Ordering::Relaxed),
            cache_creation_tokens: self.cache_creation_tokens.load(Ordering::Relaxed),
            cached_tokens: self.cached_tokens.load(Ordering::Relaxed),
            json_emitted: self.json_emitted.load(Ordering::Relaxed),
            json_disabled: self.json_disabled.load(Ordering::Relaxed),
            json_rejected: self.json_rejected.load(Ordering::Relaxed),
            otel_emitted: self.otel_emitted.load(Ordering::Relaxed),
            otel_disabled: self.otel_disabled.load(Ordering::Relaxed),
            otel_rejected: self.otel_rejected.load(Ordering::Relaxed),
            required_queue_full: self.required_queue_full.load(Ordering::Relaxed),
            required_events_quarantined: self.required_events_quarantined.load(Ordering::Relaxed),
            durable_write_failures: self.durable_write_failures.load(Ordering::Relaxed),
            pending_required: self.pending_required.load(Ordering::Relaxed),
            diagnostics_dropped: self.diagnostics_dropped.load(Ordering::Relaxed),
            sink_closed: self.sink_closed.load(Ordering::Relaxed),
        }
    }

    /// Renders the stable Prometheus text exposition without request-scoped labels.
    #[must_use]
    #[allow(clippy::too_many_lines)] // Frozen metric names are intentionally rendered together for auditability.
    pub fn render_prometheus(&self) -> String {
        let snapshot = self.snapshot();
        let mut output = String::new();

        push_counter_header(
            &mut output,
            "gateway_observability_events_total",
            "Gateway lifecycle events processed by the background event consumer.",
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_events_total",
            "kind",
            "request",
            snapshot.request_events,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_events_total",
            "kind",
            "attempt",
            snapshot.attempt_events,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_events_total",
            "kind",
            "usage",
            snapshot.usage_events,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_events_total",
            "kind",
            "health",
            snapshot.health_events,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_events_total",
            "kind",
            "diagnostic",
            snapshot.diagnostic_events,
        );

        push_counter_header(
            &mut output,
            "gateway_observability_attempts_total",
            "Terminal upstream Attempts observed by outcome.",
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_attempts_total",
            "outcome",
            "succeeded",
            snapshot.attempts_succeeded,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_attempts_total",
            "outcome",
            "failed",
            snapshot.attempts_failed,
        );

        push_counter_header(
            &mut output,
            "gateway_observability_usage_tokens_total",
            "Sanitized usage token totals by standard field.",
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_usage_tokens_total",
            "kind",
            "input",
            snapshot.input_tokens,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_usage_tokens_total",
            "kind",
            "output",
            snapshot.output_tokens,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_usage_tokens_total",
            "kind",
            "reasoning",
            snapshot.reasoning_tokens,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_usage_tokens_total",
            "kind",
            "cache_read",
            snapshot.cache_read_tokens,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_usage_tokens_total",
            "kind",
            "cache_creation",
            snapshot.cache_creation_tokens,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_usage_tokens_total",
            "kind",
            "cached",
            snapshot.cached_tokens,
        );

        push_counter_header(
            &mut output,
            "gateway_observability_exports_total",
            "Bounded export outcomes by sink and result.",
        );
        push_two_label_counter(
            &mut output,
            "gateway_observability_exports_total",
            "sink",
            "json",
            "outcome",
            "emitted",
            snapshot.json_emitted,
        );
        push_two_label_counter(
            &mut output,
            "gateway_observability_exports_total",
            "sink",
            "json",
            "outcome",
            "disabled",
            snapshot.json_disabled,
        );
        push_two_label_counter(
            &mut output,
            "gateway_observability_exports_total",
            "sink",
            "json",
            "outcome",
            "rejected",
            snapshot.json_rejected,
        );
        push_two_label_counter(
            &mut output,
            "gateway_observability_exports_total",
            "sink",
            "opentelemetry",
            "outcome",
            "emitted",
            snapshot.otel_emitted,
        );
        push_two_label_counter(
            &mut output,
            "gateway_observability_exports_total",
            "sink",
            "opentelemetry",
            "outcome",
            "disabled",
            snapshot.otel_disabled,
        );
        push_two_label_counter(
            &mut output,
            "gateway_observability_exports_total",
            "sink",
            "opentelemetry",
            "outcome",
            "rejected",
            snapshot.otel_rejected,
        );

        push_counter_header(
            &mut output,
            "gateway_observability_queue_admission_total",
            "Producer-side bounded queue admission outcomes.",
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_queue_admission_total",
            "outcome",
            "required_queue_full",
            snapshot.required_queue_full,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_queue_admission_total",
            "outcome",
            "diagnostic_dropped",
            snapshot.diagnostics_dropped,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_queue_admission_total",
            "outcome",
            "sink_closed",
            snapshot.sink_closed,
        );

        push_counter_header(
            &mut output,
            "gateway_observability_durable_events_total",
            "Durable event-writer outcomes observed after events left the request path.",
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_durable_events_total",
            "outcome",
            "required_quarantined",
            snapshot.required_events_quarantined,
        );
        push_labeled_counter(
            &mut output,
            "gateway_observability_durable_events_total",
            "outcome",
            "write_failed",
            snapshot.durable_write_failures,
        );
        push_gauge(
            &mut output,
            "gateway_observability_durable_pending_required",
            "Required events retained in the durable writer's one bounded pending batch.",
            snapshot.pending_required,
        );

        output
    }
}

#[derive(Clone, Copy)]
enum TelemetrySink {
    Json,
    OpenTelemetry,
}

/// One dispatch result from the background event consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TelemetryDispatch {
    /// Structured JSON export outcome.
    pub json: OpenTelemetryExportOutcome,
    /// OpenTelemetry outcome when the event has a traceable span.
    pub open_telemetry: Option<OpenTelemetryExportOutcome>,
}

/// Bounded fan-out from already-queued gateway events to JSON, Prometheus, and OpenTelemetry.
pub struct TelemetryPipeline {
    metrics: Arc<PrometheusMetrics>,
    json_exporter: Arc<dyn StructuredJsonExporter>,
    open_telemetry_exporter: Arc<dyn OpenTelemetryExporter>,
}

impl TelemetryPipeline {
    /// Creates a pipeline whose exporters are invoked only by the existing background event
    /// consumer.
    #[must_use]
    pub fn new(
        metrics: Arc<PrometheusMetrics>,
        json_exporter: Arc<dyn StructuredJsonExporter>,
        open_telemetry_exporter: Arc<dyn OpenTelemetryExporter>,
    ) -> Self {
        Self {
            metrics,
            json_exporter,
            open_telemetry_exporter,
        }
    }

    /// Returns the shared Prometheus registry for a later management or metrics endpoint.
    #[must_use]
    pub fn metrics(&self) -> &Arc<PrometheusMetrics> {
        &self.metrics
    }

    /// Processes one event without performing request-path I/O.
    #[must_use]
    pub fn observe_event(&self, event: &GatewayEvent) -> TelemetryDispatch {
        self.metrics.observe_event(event);
        let span = OpenTelemetrySpan::from_event(event);
        let record = StructuredJsonRecord::from_event_and_span(event, span.as_ref());
        let json = self.json_exporter.try_export(&record);
        self.metrics.observe_export(TelemetrySink::Json, json);
        let open_telemetry = span.as_ref().map(|span| {
            let outcome = self.open_telemetry_exporter.try_export(span);
            self.metrics
                .observe_export(TelemetrySink::OpenTelemetry, outcome);
            outcome
        });
        TelemetryDispatch {
            json,
            open_telemetry,
        }
    }
}

fn event_kind_and_attributes(event: &GatewayEvent) -> (TelemetryEventKind, TelemetryAttributes) {
    match event {
        GatewayEvent::Request(request) => {
            (TelemetryEventKind::Request, attributes_for_request(request))
        }
        GatewayEvent::Attempt(attempt) => {
            (TelemetryEventKind::Attempt, attributes_for_attempt(attempt))
        }
        GatewayEvent::Usage(usage) => (TelemetryEventKind::Usage, attributes_for_usage(usage)),
        GatewayEvent::Health(health) => (TelemetryEventKind::Health, attributes_for_health(health)),
        GatewayEvent::Diagnostic(diagnostic) => {
            let error = diagnostic.error();
            (
                TelemetryEventKind::Diagnostic,
                TelemetryAttributes {
                    error_code: Some(error.code()),
                    error_scope: Some(error.scope()),
                    ..TelemetryAttributes::default()
                },
            )
        }
    }
}

fn attributes_for_request(request: &gateway_core::RequestEvent) -> TelemetryAttributes {
    TelemetryAttributes {
        request_id: Some(request.request_id().as_str().to_owned()),
        protocol: Some(request.protocol()),
        streaming: Some(request.streaming()),
        ..TelemetryAttributes::default()
    }
}

fn attributes_for_attempt(attempt: &gateway_core::AttemptEvent) -> TelemetryAttributes {
    let (attempt_outcome, error_code, error_scope) = match attempt.outcome() {
        AttemptOutcome::Succeeded => (TelemetryAttemptOutcome::Succeeded, None, None),
        AttemptOutcome::Failed(error) => (
            TelemetryAttemptOutcome::Failed,
            Some(error.code()),
            Some(error.scope()),
        ),
    };
    TelemetryAttributes {
        request_id: Some(attempt.request_id().as_str().to_owned()),
        attempt_outcome: Some(attempt_outcome),
        retry_decision: Some(attempt.retry_decision()),
        error_code,
        error_scope,
        ..TelemetryAttributes::default()
    }
}

fn attributes_for_usage(event: &gateway_core::UsageEvent) -> TelemetryAttributes {
    let usage = event.usage();
    TelemetryAttributes {
        request_id: Some(event.request_id().as_str().to_owned()),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        reasoning_tokens: usage.reasoning_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_creation_tokens: usage.cache_creation_tokens,
        cached_tokens: usage.cached_tokens,
        ..TelemetryAttributes::default()
    }
}

fn attributes_for_health(health: &gateway_core::HealthEvent) -> TelemetryAttributes {
    TelemetryAttributes {
        health_kind: Some(health.kind()),
        ..TelemetryAttributes::default()
    }
}

fn trace_id_for(domain: &str, value: &str) -> TraceId {
    let digest = stable_digest(domain, value);
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    if bytes.iter().all(|byte| *byte == 0) {
        bytes[15] = 1;
    }
    TraceId::from_bytes(bytes)
}

fn root_span_id_for(request_id: &str) -> SpanId {
    span_id_for("request-root", request_id)
}

fn span_id_for(domain: &str, value: &str) -> SpanId {
    let digest = stable_digest(domain, value);
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    if bytes.iter().all(|byte| *byte == 0) {
        bytes[7] = 1;
    }
    SpanId::from_bytes(bytes)
}

fn stable_digest(domain: &str, value: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"cpa-rust-gateway/telemetry/v1\0");
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest);
    bytes
}

fn increment(counter: &AtomicU64, value: u64) {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.saturating_add(value);
        match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

fn add_optional(counter: &AtomicU64, value: Option<u64>) {
    if let Some(value) = value {
        increment(counter, value);
    }
}

fn push_counter_header(output: &mut String, name: &str, help: &str) {
    output.push_str("# HELP ");
    output.push_str(name);
    output.push(' ');
    output.push_str(help);
    output.push('\n');
    output.push_str("# TYPE ");
    output.push_str(name);
    output.push_str(" counter\n");
}

fn push_gauge(output: &mut String, name: &str, help: &str, value: u64) {
    output.push_str("# HELP ");
    output.push_str(name);
    output.push(' ');
    output.push_str(help);
    output.push('\n');
    output.push_str("# TYPE ");
    output.push_str(name);
    output.push_str(" gauge\n");
    output.push_str(name);
    output.push(' ');
    output.push_str(&value.to_string());
    output.push('\n');
}

fn push_labeled_counter(output: &mut String, name: &str, label: &str, value: &str, count: u64) {
    output.push_str(name);
    output.push('{');
    output.push_str(label);
    output.push_str("=\"");
    output.push_str(value);
    output.push_str("\"} ");
    output.push_str(&count.to_string());
    output.push('\n');
}

#[allow(clippy::too_many_arguments)] // Prometheus exposition needs two explicit fixed labels.
fn push_two_label_counter(
    output: &mut String,
    name: &str,
    first_label: &str,
    first_value: &str,
    second_label: &str,
    second_value: &str,
    count: u64,
) {
    output.push_str(name);
    output.push('{');
    output.push_str(first_label);
    output.push_str("=\"");
    output.push_str(first_value);
    output.push_str("\",");
    output.push_str(second_label);
    output.push_str("=\"");
    output.push_str(second_value);
    output.push_str("\"} ");
    output.push_str(&count.to_string());
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        sync::{Arc, Mutex},
    };

    use gateway_core::{
        AttemptEvent, AttemptOutcome, AttemptRetryDecision, ClientKeyId, CredentialId, EndpointId,
        ErrorScope, GatewayError, GatewayErrorCode, GatewayEvent, GatewayProtocol, RequestEvent,
        RequestId, ResponseId, RouteCandidateId, RouteId, UpstreamId, Usage, UsageEvent,
    };

    use super::{
        NoopOpenTelemetryExporter, OpenTelemetryExportOutcome, OpenTelemetryExporter,
        OpenTelemetrySpan, PrometheusMetrics, StructuredJsonExporter, StructuredJsonRecord,
        TelemetryPipeline,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[derive(Default)]
    struct CollectingJsonExporter {
        records: Mutex<Vec<StructuredJsonRecord>>,
    }

    impl StructuredJsonExporter for CollectingJsonExporter {
        fn try_export(&self, record: &StructuredJsonRecord) -> OpenTelemetryExportOutcome {
            match self.records.lock() {
                Ok(mut records) => {
                    records.push(record.clone());
                    OpenTelemetryExportOutcome::Emitted
                }
                Err(_) => OpenTelemetryExportOutcome::Rejected,
            }
        }
    }

    #[derive(Default)]
    struct CollectingOpenTelemetryExporter {
        spans: Mutex<Vec<OpenTelemetrySpan>>,
    }

    impl OpenTelemetryExporter for CollectingOpenTelemetryExporter {
        fn try_export(&self, span: &OpenTelemetrySpan) -> OpenTelemetryExportOutcome {
            match self.spans.lock() {
                Ok(mut spans) => {
                    spans.push(span.clone());
                    OpenTelemetryExportOutcome::Emitted
                }
                Err(_) => OpenTelemetryExportOutcome::Rejected,
            }
        }
    }

    impl CollectingOpenTelemetryExporter {
        fn spans(&self) -> Result<Vec<OpenTelemetrySpan>, std::io::Error> {
            self.spans.lock().map(|spans| spans.clone()).map_err(|_| {
                std::io::Error::other("collecting OpenTelemetry exporter mutex poisoned")
            })
        }
    }

    fn request_event() -> Result<GatewayEvent, Box<dyn Error>> {
        Ok(GatewayEvent::Request(RequestEvent::new(
            RequestId::try_new("request-telemetry-01")?,
            ClientKeyId::try_new("client-key-telemetry")?,
            None,
            GatewayProtocol::OpenAiResponses,
            "requested-model-must-not-appear".to_owned(),
            "public-model-must-not-appear".to_owned(),
            None,
            true,
        )))
    }

    fn failed_attempt_event() -> Result<GatewayEvent, Box<dyn Error>> {
        Ok(GatewayEvent::Attempt(AttemptEvent::new(
            RequestId::try_new("request-telemetry-01")?,
            1,
            RouteId::try_new("route-telemetry")?,
            RouteCandidateId::try_new("candidate-telemetry")?,
            CredentialId::try_new("credential-telemetry")?,
            EndpointId::try_new("endpoint-telemetry")?,
            UpstreamId::try_new("upstream-telemetry")?,
            "upstream-model-must-not-appear".to_owned(),
            100,
            140,
            AttemptOutcome::Failed(GatewayError::new(
                GatewayErrorCode::ProviderTransient,
                ErrorScope::Provider,
            )),
            AttemptRetryDecision::RetryEligible,
        )))
    }

    fn usage_event() -> Result<GatewayEvent, Box<dyn Error>> {
        let usage = Usage {
            input_tokens: Some(11),
            output_tokens: Some(7),
            reasoning_tokens: Some(3),
            cache_read_tokens: Some(5),
            cache_creation_tokens: Some(2),
            cached_tokens: Some(5),
            ..Usage::default()
        };
        Ok(GatewayEvent::Usage(UsageEvent::from_usage(
            RequestId::try_new("request-telemetry-01")?,
            ResponseId::try_new("response-telemetry-01")?,
            &usage,
        )))
    }

    #[test]
    fn request_attempt_and_usage_share_one_w3c_trace() -> TestResult {
        let metrics = Arc::new(PrometheusMetrics::default());
        let json = Arc::new(CollectingJsonExporter::default());
        let otel = Arc::new(CollectingOpenTelemetryExporter::default());
        let pipeline = TelemetryPipeline::new(metrics, json, otel.clone());

        let _ = pipeline.observe_event(&request_event()?);
        let _ = pipeline.observe_event(&failed_attempt_event()?);
        let _ = pipeline.observe_event(&usage_event()?);

        let spans = otel.spans()?;
        assert_eq!(spans.len(), 3);
        let request_span = &spans[0];
        let attempt_span = &spans[1];
        let usage_span = &spans[2];
        assert_eq!(request_span.name(), "gateway.request");
        assert_eq!(attempt_span.name(), "gateway.upstream.attempt");
        assert_eq!(usage_span.name(), "gateway.usage");
        assert_eq!(
            attempt_span.span_context().trace_id(),
            request_span.span_context().trace_id()
        );
        assert_eq!(
            usage_span.span_context().trace_id(),
            request_span.span_context().trace_id()
        );
        assert_eq!(
            attempt_span.parent_span_id(),
            Some(request_span.span_context().span_id())
        );
        assert_eq!(
            usage_span.parent_span_id(),
            Some(request_span.span_context().span_id())
        );
        assert!(request_span.span_context().is_valid());
        assert!(request_span.span_context().is_sampled());
        Ok(())
    }

    #[test]
    fn structured_json_and_otel_exclude_target_and_identity_material() -> TestResult {
        let request_json = StructuredJsonRecord::from_event(&request_event()?).to_json_line()?;
        let attempt = failed_attempt_event()?;
        let attempt_json = StructuredJsonRecord::from_event(&attempt).to_json_line()?;
        let span = OpenTelemetrySpan::from_event(&attempt)
            .ok_or_else(|| std::io::Error::other("Attempt must create one span"))?;
        let otel_json = span.to_json_line()?;

        assert!(request_json.contains("request-telemetry-01"));
        for forbidden in [
            "requested-model-must-not-appear",
            "public-model-must-not-appear",
            "client-key-telemetry",
            "route-telemetry",
            "candidate-telemetry",
            "credential-telemetry",
            "endpoint-telemetry",
            "upstream-telemetry",
            "upstream-model-must-not-appear",
        ] {
            assert!(!request_json.contains(forbidden));
            assert!(!attempt_json.contains(forbidden));
            assert!(!otel_json.contains(forbidden));
        }
        Ok(())
    }

    #[test]
    fn prometheus_rendering_uses_only_bounded_labels() -> TestResult {
        let metrics = Arc::new(PrometheusMetrics::default());
        let json = Arc::new(CollectingJsonExporter::default());
        let pipeline =
            TelemetryPipeline::new(metrics.clone(), json, Arc::new(NoopOpenTelemetryExporter));
        let _ = pipeline.observe_event(&request_event()?);
        let _ = pipeline.observe_event(&failed_attempt_event()?);
        let _ = pipeline.observe_event(&usage_event()?);

        let rendered = metrics.render_prometheus();
        assert!(rendered.contains("gateway_observability_events_total{kind=\"request\"} 1"));
        assert!(rendered.contains("gateway_observability_attempts_total{outcome=\"failed\"} 1"));
        assert!(rendered.contains("gateway_observability_usage_tokens_total{kind=\"input\"} 11"));
        assert!(!rendered.contains("request-telemetry-01"));
        assert!(!rendered.contains("must-not-appear"));
        Ok(())
    }
}
