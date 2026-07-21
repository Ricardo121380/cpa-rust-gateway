//! Structured event and metrics boundary outside the request hot path.
//!
//! P3 provides only the bounded non-blocking producer and its consumer-facing receiver. P4 owns
//! `SQLite` batching, persistence, exporters, and production metrics; no data-path caller waits on
//! those future consumers.

#![deny(unsafe_code)]

mod telemetry;

use std::sync::atomic::{AtomicU64, Ordering};

use gateway_core::{
    DiagnosticEvent, EventEmission, GatewayEvent, GatewayEventPriority, GatewayEventSink,
};
use tokio::sync::mpsc::{self, error::TrySendError};

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
    required: mpsc::Sender<GatewayEvent>,
    diagnostic: mpsc::Sender<GatewayEvent>,
    required_queue_full: AtomicU64,
    diagnostics_dropped: AtomicU64,
    sink_closed: AtomicU64,
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

        Ok((
            Self {
                required,
                diagnostic,
                required_queue_full: AtomicU64::new(0),
                diagnostics_dropped: AtomicU64::new(0),
                sink_closed: AtomicU64::new(0),
            },
            EventQueueReceiver {
                required: required_receiver,
                diagnostic: diagnostic_receiver,
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

impl GatewayEventSink for BoundedEventQueue {
    fn try_emit(&self, event: GatewayEvent) -> EventEmission {
        let (sender, is_required) = match event.priority() {
            GatewayEventPriority::Required => (&self.required, true),
            GatewayEventPriority::Diagnostic => (&self.diagnostic, false),
        };

        match sender.try_send(event) {
            Ok(()) => EventEmission::Enqueued,
            Err(TrySendError::Full(_)) if is_required => {
                self.required_queue_full.fetch_add(1, Ordering::Relaxed);
                EventEmission::RequiredQueueFull
            }
            Err(TrySendError::Full(_)) => {
                self.diagnostics_dropped.fetch_add(1, Ordering::Relaxed);
                EventEmission::DiagnosticDropped
            }
            Err(TrySendError::Closed(_)) => {
                self.sink_closed.fetch_add(1, Ordering::Relaxed);
                EventEmission::SinkClosed
            }
        }
    }
}

/// Single-consumer endpoint for a [`BoundedEventQueue`].
pub struct EventQueueReceiver {
    required: mpsc::Receiver<GatewayEvent>,
    diagnostic: mpsc::Receiver<GatewayEvent>,
}

impl EventQueueReceiver {
    /// Returns the next available event without waiting, preferring required records.
    #[must_use]
    pub fn try_recv(&mut self) -> Option<GatewayEvent> {
        self.required
            .try_recv()
            .ok()
            .or_else(|| self.diagnostic.try_recv().ok())
    }

    /// Waits for the next event, preferring required records when both queues are ready.
    pub async fn recv(&mut self) -> Option<GatewayEvent> {
        if let Some(event) = self.try_recv() {
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
