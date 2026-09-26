//! Structured event and metrics boundary outside the request hot path.
//!
//! The producer has finite priority queues and optional asynchronous commit receipts. The store
//! owns persistence on its blocking worker; no database dependency enters this crate.

#![deny(unsafe_code)]

mod log_safety;
mod telemetry;

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use gateway_core::{
    AttemptId, DiagnosticEvent, EventEmission, EventEmissionFuture, GatewayEvent,
    GatewayEventPriority, GatewayEventSink,
};
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    oneshot,
};

pub use log_safety::{
    BodyOmissionReason, BodySamplingPolicy, BodySamplingPolicyError, HTTP_LOG_SCHEMA_VERSION,
    HttpLogDirection, LogRedactionPolicy, LoggedContentType, MAX_BODY_SAMPLE_BYTES,
    REDACTED_LOG_VALUE, SanitizedBodySample, SanitizedHeaderSummary, SanitizedHttpLogRecord,
    try_emit_sanitized_http_log,
};
pub use telemetry::{
    JsonTracingInitError, NoopOpenTelemetryExporter, NoopStructuredJsonExporter,
    OpenTelemetryExportOutcome, OpenTelemetryExporter, OpenTelemetrySpan, OpenTelemetrySpanKind,
    PrometheusMetrics, PrometheusMetricsSnapshot, StructuredJsonExporter, StructuredJsonRecord,
    TelemetryDispatch, TelemetryEventKind, TelemetryPipeline, TracingJsonExporter,
    try_init_json_tracing,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-observability";

/// Default bounded capacity for Request, Attempt, Usage, and Health records.
pub const DEFAULT_REQUIRED_EVENT_CAPACITY: usize = 1_024;
/// Default bounded capacity for low-priority diagnostics.
pub const DEFAULT_DIAGNOSTIC_EVENT_CAPACITY: usize = 128;
/// Hard cap that keeps one configured in-process event queue finite.
pub const MAX_EVENT_QUEUE_CAPACITY: usize = 8_192;

/// Immutable capacity configuration for the P3 in-process event queues.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventQueueConfig {
    required_capacity: usize,
    diagnostic_capacity: usize,
}

impl EventQueueConfig {
    /// Validates finite, positive capacities for both priority classes.
    ///
    /// # Errors
    ///
    /// Returns a safe configuration error before allocating queue storage.
    pub const fn try_new(
        required_capacity: usize,
        diagnostic_capacity: usize,
    ) -> Result<Self, EventQueueConfigError> {
        if required_capacity == 0 {
            return Err(EventQueueConfigError::ZeroRequiredCapacity);
        }
        if diagnostic_capacity == 0 {
            return Err(EventQueueConfigError::ZeroDiagnosticCapacity);
        }
        if required_capacity > MAX_EVENT_QUEUE_CAPACITY {
            return Err(EventQueueConfigError::RequiredCapacityTooLarge);
        }
        if diagnostic_capacity > MAX_EVENT_QUEUE_CAPACITY {
            return Err(EventQueueConfigError::DiagnosticCapacityTooLarge);
        }

        Ok(Self {
            required_capacity,
            diagnostic_capacity,
        })
    }

    /// Returns the bounded Request/Attempt/Usage/Health capacity.
    #[must_use]
    pub const fn required_capacity(self) -> usize {
        self.required_capacity
    }

    /// Returns the bounded low-priority diagnostic capacity.
    #[must_use]
    pub const fn diagnostic_capacity(self) -> usize {
        self.diagnostic_capacity
    }
}

impl Default for EventQueueConfig {
    fn default() -> Self {
        Self {
            required_capacity: DEFAULT_REQUIRED_EVENT_CAPACITY,
            diagnostic_capacity: DEFAULT_DIAGNOSTIC_EVENT_CAPACITY,
        }
    }
}

/// Safe configuration failures for [`EventQueueConfig`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventQueueConfigError {
    /// Required records need at least one bounded queue slot.
    ZeroRequiredCapacity,
    /// Diagnostics need at least one bounded queue slot.
    ZeroDiagnosticCapacity,
    /// Required-record capacity exceeds the frozen finite upper bound.
    RequiredCapacityTooLarge,
    /// Diagnostic capacity exceeds the frozen finite upper bound.
    DiagnosticCapacityTooLarge,
}

impl std::fmt::Display for EventQueueConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroRequiredCapacity => {
                formatter.write_str("required event capacity must be positive")
            }
            Self::ZeroDiagnosticCapacity => {
                formatter.write_str("diagnostic event capacity must be positive")
            }
            Self::RequiredCapacityTooLarge => {
                formatter.write_str("required event capacity exceeds the finite maximum")
            }
            Self::DiagnosticCapacityTooLarge => {
                formatter.write_str("diagnostic event capacity exceeds the finite maximum")
            }
        }
    }
}

impl std::error::Error for EventQueueConfigError {}

/// Observable counters for non-blocking queue admission outcomes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EventQueueMetrics {
    /// Number of required events that could not enter the finite required queue.
    pub required_queue_full: u64,
    /// Number of low-priority diagnostics deliberately dropped under queue pressure.
    pub diagnostics_dropped: u64,
    /// Number of events rejected because their receiving queue was closed.
    pub sink_closed: u64,
}

/// Bounded producer for structured gateway events.
///
/// Required records and diagnostics have independent bounded channels. Therefore an accumulation
/// of diagnostics cannot consume Request/Attempt/Usage capacity. If the required queue itself is
/// saturated, the caller gets an explicit non-blocking outcome and the counter records it; this is
/// deliberately visible to the later P4 writer/metrics path rather than silently waiting or
/// fabricating unbounded storage.
pub struct BoundedEventQueue {
    required: mpsc::Sender<QueuedEvent>,
    diagnostic: mpsc::Sender<QueuedEvent>,
    required_queue_full: AtomicU64,
    diagnostics_dropped: AtomicU64,
    sink_closed: AtomicU64,
    health: Arc<RecordingHealth>,
}

impl BoundedEventQueue {
    /// Creates a finite producer plus the single consumer used by a later asynchronous writer.
    ///
    /// # Errors
    ///
    /// Returns [`EventQueueConfigError`] before any queue is allocated when a capacity is invalid.
    pub fn try_new(
        config: EventQueueConfig,
    ) -> Result<(Self, EventQueueReceiver), EventQueueConfigError> {
        let config =
            EventQueueConfig::try_new(config.required_capacity(), config.diagnostic_capacity())?;
        let (required, required_receiver) = mpsc::channel(config.required_capacity());
        let (diagnostic, diagnostic_receiver) = mpsc::channel(config.diagnostic_capacity());

        let health = Arc::new(RecordingHealth::default());
        Ok((
            Self {
                required,
                diagnostic,
                required_queue_full: AtomicU64::new(0),
                diagnostics_dropped: AtomicU64::new(0),
                sink_closed: AtomicU64::new(0),
                health: health.clone(),
            },
            EventQueueReceiver {
                required: required_receiver,
                diagnostic: diagnostic_receiver,
                health,
            },
        ))
    }

    /// Returns a snapshot of explicit non-blocking admission outcomes.
    #[must_use]
    pub fn metrics(&self) -> EventQueueMetrics {
        EventQueueMetrics {
            required_queue_full: self.required_queue_full.load(Ordering::Relaxed),
            diagnostics_dropped: self.diagnostics_dropped.load(Ordering::Relaxed),
            sink_closed: self.sink_closed.load(Ordering::Relaxed),
        }
    }
}

impl BoundedEventQueue {
    /// Reads recording health without storage I/O.
    pub fn recording_health(&self) -> &RecordingHealth {
        &self.health
    }

    /// Whether a new confirmed request can currently be admitted.
    pub fn accepts_requests(&self) -> bool {
        self.health.state.load(Ordering::Acquire) == 1
            && !self.required.is_closed()
            && self.required.capacity() > 0
    }

    fn enqueue(
        &self,
        event: GatewayEvent,
        receipt: Option<oneshot::Sender<EventEmission>>,
    ) -> EventEmission {
        let required = event.priority() == GatewayEventPriority::Required;
        let sender = if required {
            &self.required
        } else {
            &self.diagnostic
        };
        if required {
            self.health.pending.fetch_add(1, Ordering::AcqRel);
        }
        let entry = QueuedEvent {
            event,
            receipt,
            health: self.health.clone(),
            required,
        };
        match sender.try_send(entry) {
            Ok(()) => EventEmission::Enqueued,
            Err(error) => {
                if required {
                    self.health.pending.fetch_sub(1, Ordering::AcqRel);
                    self.health
                        .confirmation_failures
                        .fetch_add(1, Ordering::Relaxed);
                }
                match error {
                    TrySendError::Full(_) if required => {
                        self.required_queue_full.fetch_add(1, Ordering::Relaxed);
                        EventEmission::RequiredQueueFull
                    }
                    TrySendError::Full(_) => {
                        self.diagnostics_dropped.fetch_add(1, Ordering::Relaxed);
                        EventEmission::DiagnosticDropped
                    }
                    TrySendError::Closed(_) => {
                        self.sink_closed.fetch_add(1, Ordering::Relaxed);
                        EventEmission::SinkClosed
                    }
                }
            }
        }
    }
}

impl GatewayEventSink for BoundedEventQueue {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        self.enqueue(event, None)
    }

    fn emit_confirmed(&self, event: GatewayEvent) -> EventEmissionFuture<'_> {
        Box::pin(async move {
            // Queue-only embeddings explicitly acknowledge admission, not persistence.
            if !self.health.writer_attached.load(Ordering::Acquire) {
                return self.try_emit(event);
            }
            if matches!(event, GatewayEvent::Request(_))
                && self.health.state.load(Ordering::Acquire) >= 2
            {
                return EventEmission::PersistenceUnavailable;
            }
            let (send, receive) = oneshot::channel();
            let result = self.enqueue(event, Some(send));
            if result != EventEmission::Enqueued {
                return result;
            }
            if let Ok(Ok(result)) = tokio::time::timeout(Duration::from_secs(2), receive).await {
                result
            } else {
                self.health
                    .confirmation_failures
                    .fetch_add(1, Ordering::Relaxed);
                EventEmission::PersistenceUnavailable
            }
        })
    }
}

/// Shared bounded recording status. State: 0 starting, 1 ready, 2 storage unavailable, 3 closed.
#[derive(Default)]
pub struct RecordingHealth {
    writer_attached: AtomicBool,
    state: AtomicU64,
    pending: AtomicU64,
    last_commit_ms: AtomicU64,
    confirmation_failures: AtomicU64,
    recovered_unknown: AtomicU64,
}

impl RecordingHealth {
    /// Returns (state, pending confirmations, last commit ms, confirmation failures, recovered unknown).
    pub fn snapshot(&self) -> (u64, u64, u64, u64, u64) {
        (
            self.state.load(Ordering::Acquire),
            self.pending.load(Ordering::Acquire),
            self.last_commit_ms.load(Ordering::Acquire),
            self.confirmation_failures.load(Ordering::Acquire),
            self.recovered_unknown.load(Ordering::Acquire),
        )
    }
    /// Published only by the background writer after a storage result.
    pub fn set_state(&self, state: u64) {
        self.state.store(state, Ordering::Release);
    }
    /// Marks records left unsettled by a previous process; does not invent terminal usage.
    pub fn recovered(&self, count: u64) {
        self.recovered_unknown.store(count, Ordering::Release);
    }
}

/// One finite queue slot plus its optional durable acknowledgement.
pub struct QueuedEvent {
    /// Sanitized event, never request/response bodies or credential material.
    pub event: GatewayEvent,
    receipt: Option<oneshot::Sender<EventEmission>>,
    health: Arc<RecordingHealth>,
    required: bool,
}
impl QueuedEvent {
    /// Releases a pending confirmation only after commit or persistent quarantine.
    pub fn complete(mut self, outcome: EventEmission) {
        if self.required {
            self.health.pending.fetch_sub(1, Ordering::AcqRel);
        }
        if outcome == EventEmission::Persisted {
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |v| u64::try_from(v.as_millis()).unwrap_or(u64::MAX));
            self.health.last_commit_ms.store(ms, Ordering::Release);
        } else if self.required && outcome != EventEmission::Enqueued {
            self.health
                .confirmation_failures
                .fetch_add(1, Ordering::Relaxed);
        }
        if let Some(receipt) = self.receipt.take() {
            let _ = receipt.send(outcome);
        }
    }
    fn into_event(self) -> GatewayEvent {
        let event = self.event.clone();
        self.complete(EventEmission::Enqueued);
        event
    }
}

/// Request-local lineage: never uses a process-global evictable identity lookup.
pub struct RequestEventSink {
    inner: Arc<dyn GatewayEventSink>,
    attempt: Mutex<Option<AttemptId>>,
}
impl RequestEventSink {
    /// Shares one confirmed-attempt identity between router and final usage observer.
    pub fn new(inner: Arc<dyn GatewayEventSink>) -> Self {
        Self {
            inner,
            attempt: Mutex::new(None),
        }
    }
    fn attach(&self, event: GatewayEvent) -> Result<GatewayEvent, EventEmission> {
        if let GatewayEvent::Usage(usage) = event {
            let attempt = self
                .attempt
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let (Some(explicit), Some(confirmed)) = (usage.attempt_id(), attempt.as_ref())
                && explicit != confirmed
            {
                return Err(EventEmission::PersistenceUnavailable);
            }
            Ok(GatewayEvent::Usage(match (usage.attempt_id(), attempt) {
                (None, Some(id)) => usage.with_attempt_id(id),
                _ => usage,
            }))
        } else {
            Ok(event)
        }
    }
}
impl GatewayEventSink for RequestEventSink {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        match self.attach(event) {
            Ok(event) => self.inner.try_emit(event),
            Err(error) => error,
        }
    }
    fn emit_confirmed(&self, event: GatewayEvent) -> EventEmissionFuture<'_> {
        Box::pin(async move {
            let attempt = if let GatewayEvent::Attempt(ref attempt) = event {
                Some(attempt.attempt_id().clone())
            } else {
                None
            };
            let event = match self.attach(event) {
                Ok(event) => event,
                Err(error) => return error,
            };
            let result = self.inner.emit_confirmed(event).await;
            if result.into_result().is_ok()
                && let Some(attempt) = attempt
            {
                *self
                    .attempt
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(attempt);
            }
            result
        })
    }
}

/// Single-consumer endpoint for a [`BoundedEventQueue`].
pub struct EventQueueReceiver {
    required: mpsc::Receiver<QueuedEvent>,
    diagnostic: mpsc::Receiver<QueuedEvent>,
    health: Arc<RecordingHealth>,
}

impl EventQueueReceiver {
    /// True when all producers have gone away.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.required.is_closed() && self.diagnostic.is_closed()
    }

    /// Enables durable acknowledgements before this receiver is moved into its writer.
    #[must_use]
    pub fn attach_writer(&self) -> Arc<RecordingHealth> {
        self.health.writer_attached.store(true, Ordering::Release);
        self.health.clone()
    }
    /// Queue-only consumer; acknowledges admission only.
    pub fn try_recv(&mut self) -> Option<GatewayEvent> {
        self.try_recv_entry().map(QueuedEvent::into_event)
    }
    /// Queue-only asynchronous consumer.
    pub async fn recv(&mut self) -> Option<GatewayEvent> {
        self.recv_entry().await.map(QueuedEvent::into_event)
    }

    /// Returns the next available event without waiting, preferring required records.
    #[must_use]
    pub fn try_recv_entry(&mut self) -> Option<QueuedEvent> {
        self.required
            .try_recv()
            .ok()
            .or_else(|| self.diagnostic.try_recv().ok())
    }

    /// Waits for the next event, preferring required records when both queues are ready.
    pub async fn recv_entry(&mut self) -> Option<QueuedEvent> {
        if let Some(event) = self.try_recv_entry() {
            return Some(event);
        }

        tokio::select! {
            biased;
            event = self.required.recv() => match event {
                Some(event) => Some(event),
                None => self.diagnostic.recv().await,
            },
            event = self.diagnostic.recv() => match event {
                Some(event) => Some(event),
                None => self.required.recv().await,
            },
        }
    }
}

/// Builds one low-priority diagnostic event without accepting arbitrary diagnostic text.
#[must_use]
pub const fn diagnostic_event(error: gateway_core::GatewayError) -> GatewayEvent {
    GatewayEvent::Diagnostic(DiagnosticEvent::new(error))
}

#[cfg(test)]
mod tests {
    use gateway_core::{
        ClientKeyId, EventEmission, GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink,
        GatewayProtocol, RequestEvent, RequestId,
    };

    use super::{BoundedEventQueue, EventQueueConfig, EventQueueConfigError, diagnostic_event};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn request_event(value: &str) -> Result<GatewayEvent, Box<dyn std::error::Error>> {
        Ok(GatewayEvent::Request(RequestEvent::new(
            RequestId::try_new(format!("request-{value}"))?,
            ClientKeyId::try_new("client-key")?,
            None,
            GatewayProtocol::OpenAiResponses,
            "requested-model".to_owned(),
            "public-model".to_owned(),
            None,
            false,
        )))
    }

    #[test]
    fn validates_finite_positive_event_capacities() {
        assert_eq!(
            EventQueueConfig::try_new(0, 1),
            Err(EventQueueConfigError::ZeroRequiredCapacity)
        );
        assert_eq!(
            EventQueueConfig::try_new(1, 0),
            Err(EventQueueConfigError::ZeroDiagnosticCapacity)
        );
    }

    #[test]
    fn diagnostics_cannot_consume_required_event_capacity() -> TestResult {
        let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        assert_eq!(
            queue.try_emit(diagnostic_event(GatewayError::new(
                GatewayErrorCode::InternalError,
                gateway_core::ErrorScope::Internal,
            ))),
            EventEmission::Enqueued
        );
        assert_eq!(
            queue.try_emit(diagnostic_event(GatewayError::new(
                GatewayErrorCode::InternalError,
                gateway_core::ErrorScope::Internal,
            ))),
            EventEmission::DiagnosticDropped
        );
        assert_eq!(
            queue.try_emit(request_event("one")?),
            EventEmission::Enqueued
        );

        assert!(matches!(
            receiver.try_recv(),
            Some(GatewayEvent::Request(_))
        ));
        assert!(matches!(
            receiver.try_recv(),
            Some(GatewayEvent::Diagnostic(_))
        ));
        assert_eq!(queue.metrics().diagnostics_dropped, 1);
        Ok(())
    }

    #[test]
    fn required_queue_saturation_is_explicit_and_non_blocking() -> TestResult {
        let (queue, _receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        assert_eq!(
            queue.try_emit(request_event("one")?),
            EventEmission::Enqueued
        );
        assert_eq!(
            queue.try_emit(request_event("two")?),
            EventEmission::RequiredQueueFull
        );
        assert_eq!(queue.metrics().required_queue_full, 1);
        Ok(())
    }

    #[tokio::test]
    async fn asynchronous_receiver_prefers_required_records_over_ready_diagnostics() -> TestResult {
        let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        assert_eq!(
            queue.try_emit(diagnostic_event(GatewayError::new(
                GatewayErrorCode::InternalError,
                gateway_core::ErrorScope::Internal,
            ))),
            EventEmission::Enqueued
        );
        assert_eq!(
            queue.try_emit(request_event("one")?),
            EventEmission::Enqueued
        );

        assert!(matches!(
            receiver.recv().await,
            Some(GatewayEvent::Request(_))
        ));
        assert!(matches!(
            receiver.recv().await,
            Some(GatewayEvent::Diagnostic(_))
        ));
        Ok(())
    }
}
