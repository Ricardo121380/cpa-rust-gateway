//! Accepted-request timing shared with the source; only the delivery owner emits a terminal event.
use gateway_core::{
    CanonicalEvent, GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink,
    RequestFinishedEvent, RequestId, RequestOutcome,
};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Default)]
struct Observation {
    first_content_ms: Option<u64>,
    has_content: bool,
    error: Option<GatewayErrorCode>,
    delivery_ready: bool,
}
struct Shared {
    request_id: RequestId,
    sink: Arc<dyn GatewayEventSink>,
    started: Instant,
    started_at_ms: i64,
    observation: Mutex<Observation>,
}
#[derive(Clone)]
pub(super) struct RequestObservation(Arc<Shared>);
impl RequestObservation {
    pub(super) fn observe(&self, event: &CanonicalEvent) {
        if is_content(event)
            && let Ok(mut value) = self.0.observation.lock()
        {
            value.has_content = true;
        }
        if let CanonicalEvent::StreamError(error) = event {
            self.fail(&error.error);
        }
    }
    pub(super) fn delivered(&self, event: &CanonicalEvent) {
        if is_content(event) {
            self.delivered_content();
        }
    }
    fn delivered_content(&self) {
        if let Ok(mut value) = self.0.observation.lock() {
            value
                .first_content_ms
                .get_or_insert_with(|| elapsed_ms(self.0.started));
        }
    }
    pub(super) fn delivered_json(&self) {
        let has_content = self
            .0
            .observation
            .lock()
            .is_ok_and(|value| value.has_content);
        if has_content {
            self.delivered_content();
        }
    }
    pub(super) fn ready(&self) {
        if let Ok(mut value) = self.0.observation.lock() {
            value.delivery_ready = true;
        }
    }
    pub(super) fn fail(&self, error: &GatewayError) {
        if let Ok(mut value) = self.0.observation.lock() {
            value.error.get_or_insert(error.code());
        }
    }
}
/// Single owner moves from the handler to JSON/SSE body or WebSocket delivery future.
/// Source tasks hold only observations and cannot declare successful client delivery.
pub(super) struct RequestGuard {
    pub(super) observation: RequestObservation,
    emitted: bool,
}
impl RequestGuard {
    #[cfg(test)]
    pub(super) fn new(request_id: RequestId, sink: Arc<dyn GatewayEventSink>) -> Self {
        Self::new_at(
            request_id,
            sink,
            (Instant::now(), super::system_now_ms().unwrap_or(0)),
        )
    }
    pub(super) fn new_at(
        request_id: RequestId,
        sink: Arc<dyn GatewayEventSink>,
        start: (Instant, i64),
    ) -> Self {
        Self {
            observation: RequestObservation(Arc::new(Shared {
                request_id,
                sink,
                started: start.0,
                started_at_ms: start.1,
                observation: Mutex::new(Observation::default()),
            })),
            emitted: false,
        }
    }
    pub(super) fn complete_if_ready(&mut self) {
        let ready = self
            .observation
            .0
            .observation
            .lock()
            .is_ok_and(|value| value.delivery_ready);
        if ready {
            self.complete();
        }
    }
    pub(super) fn complete(&mut self) {
        self.emit(RequestOutcome::Succeeded);
    }
    fn emit(&mut self, requested_outcome: RequestOutcome) {
        if self.emitted {
            return;
        }
        self.emitted = true;
        let source = &self.observation.0;
        let (first_content_ms, error_code) = source
            .observation
            .lock()
            .map_or((None, Some(GatewayErrorCode::InternalError)), |value| {
                (value.first_content_ms, value.error)
            });
        let _ = source
            .sink
            .try_emit(GatewayEvent::RequestFinished(RequestFinishedEvent {
                request_id: source.request_id.clone(),
                started_at_ms: source.started_at_ms,
                finished_at_ms: super::system_now_ms().unwrap_or(source.started_at_ms),
                duration_ms: elapsed_ms(source.started),
                first_content_ms,
                outcome: if error_code.is_some() {
                    RequestOutcome::Failed
                } else {
                    requested_outcome
                },
                error_code,
            }));
    }
}
fn is_content(event: &CanonicalEvent) -> bool {
    match event {
        CanonicalEvent::TextDelta(value) => !value.text.is_empty(),
        CanonicalEvent::ReasoningDelta(value) => !value.text.is_empty(),
        CanonicalEvent::ToolCallStart(_) => true,
        CanonicalEvent::ToolCallArgumentsDelta(value) => !value.delta.is_empty(),
        _ => false,
    }
}
impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.emit(RequestOutcome::Cancelled);
    }
}
fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Compaction has no streaming retry gate, but still observes the Actix JSON handoff.
pub(super) fn json_response(
    bytes: actix_web::web::Bytes,
    guard: Option<RequestGuard>,
) -> actix_web::HttpResponse {
    struct Body {
        bytes: Option<actix_web::web::Bytes>,
        guard: Option<RequestGuard>,
    }
    impl actix_web::body::MessageBody for Body {
        type Error = std::convert::Infallible;
        fn size(&self) -> actix_web::body::BodySize {
            self.bytes
                .as_ref()
                .map_or(actix_web::body::BodySize::None, |b| {
                    actix_web::body::BodySize::Sized(b.len() as u64)
                })
        }
        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<actix_web::web::Bytes, Self::Error>>> {
            let body = self.get_mut();
            std::task::Poll::Ready(body.bytes.take().map(|bytes| {
                if let Some(guard) = &mut body.guard {
                    guard.observation.delivered_json();
                    guard.complete();
                }
                Ok(bytes)
            }))
        }
    }
    actix_web::HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .content_type("application/json")
        .message_body(Body {
            bytes: Some(bytes),
            guard,
        })
        .map_or_else(
            |_| super::pre_header_error(&super::internal_error()),
            actix_web::HttpResponse::map_into_boxed_body,
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_observability::{BoundedEventQueue, EventQueueConfig};
    #[test]
    fn source_completion_alone_is_not_delivery_and_failure_wins()
    -> Result<(), Box<dyn std::error::Error>> {
        let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(4, 1)?)?;
        let queue = Arc::new(queue);
        let guard = RequestGuard::new(RequestId::try_new("cancelled-request")?, queue.clone());
        let source = guard.observation.clone();
        assert!(receiver.try_recv().is_none());
        drop(guard);
        let Some(GatewayEvent::RequestFinished(event)) = receiver.try_recv() else {
            return Err("missing cancellation".into());
        };
        assert_eq!(event.outcome, RequestOutcome::Cancelled);
        source.fail(&GatewayError::new(
            GatewayErrorCode::StreamTruncated,
            gateway_core::ErrorScope::Stream,
        ));
        assert!(receiver.try_recv().is_none());
        let mut guard = RequestGuard::new(RequestId::try_new("encoder-failure")?, queue);
        guard.observation.fail(&GatewayError::new(
            GatewayErrorCode::UpstreamProtocolError,
            gateway_core::ErrorScope::Stream,
        ));
        guard.complete();
        drop(guard);
        let Some(GatewayEvent::RequestFinished(event)) = receiver.try_recv() else {
            return Err("missing failure".into());
        };
        assert_eq!(event.outcome, RequestOutcome::Failed);
        assert!(event.first_content_ms.is_none());
        assert!(receiver.try_recv().is_none());
        Ok(())
    }
}
