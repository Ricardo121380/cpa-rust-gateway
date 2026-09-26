//! Accepted-request timing shared with the source; only the delivery owner emits a terminal event.
use gateway_core::{
    CanonicalEvent, GatewayError, GatewayErrorCode, GatewayEvent, GatewayEventSink,
    RequestFinishedEvent, RequestId, RequestOutcome,
};
use std::time::Instant;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

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
    delivery_complete: bool,
    confirmation: Option<Pin<Box<dyn Future<Output = gateway_core::EventEmission> + Send>>>,
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
            delivery_complete: false,
            confirmation: None,
        }
    }
    pub(super) fn delivered_complete(&mut self) {
        self.delivery_complete = true;
    }

    pub(super) fn poll_complete(
        &mut self,
        cx: &mut Context<'_>,
        only_if_ready: bool,
    ) -> Poll<Result<(), GatewayError>> {
        if only_if_ready
            && !self
                .observation
                .0
                .observation
                .lock()
                .is_ok_and(|v| v.delivery_ready)
        {
            return Poll::Ready(Ok(()));
        }
        if self.confirmation.is_none() {
            if self.emitted {
                return Poll::Ready(Ok(()));
            }
            self.emitted = true;
            let event = self.terminal_event(RequestOutcome::Succeeded);
            let sink = self.observation.0.sink.clone();
            self.confirmation = Some(Box::pin(async move { sink.emit_confirmed(event).await }));
        }
        let Some(confirmation) = self.confirmation.as_mut() else {
            return Poll::Ready(Err(super::internal_error()));
        };
        let result = confirmation.as_mut().poll(cx);
        match result {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                self.confirmation = None;
                let result = result.into_result();
                if let Err(error) = &result {
                    self.observation.fail(error);
                }
                Poll::Ready(result)
            }
        }
    }
    pub(super) async fn complete_confirmed(&mut self) -> Result<(), GatewayError> {
        std::future::poll_fn(|cx| self.poll_complete(cx, false)).await
    }
    #[cfg(test)]
    pub(super) fn complete(&mut self) {
        self.emit(RequestOutcome::Succeeded);
    }
    fn terminal_event(&self, requested_outcome: RequestOutcome) -> GatewayEvent {
        let source = &self.observation.0;
        let (first_content_ms, error_code) = source
            .observation
            .lock()
            .map_or((None, Some(GatewayErrorCode::InternalError)), |value| {
                (value.first_content_ms, value.error)
            });
        GatewayEvent::RequestFinished(RequestFinishedEvent {
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
        })
    }
    fn emit(&mut self, outcome: RequestOutcome) {
        if self.emitted {
            return;
        }
        self.emitted = true;
        // Drop cannot await. The bounded writer drains this slot; rejection is observed and the
        // already-persisted Request stays unknown, never replaced by fabricated success.
        if let Err(error) = self
            .observation
            .0
            .sink
            .try_emit(self.terminal_event(outcome))
            .into_result()
        {
            self.observation.fail(&error);
        }
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
        self.emit(if self.delivery_complete {
            RequestOutcome::Succeeded
        } else {
            RequestOutcome::Cancelled
        });
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
        type Error = GatewayError;
        fn size(&self) -> actix_web::body::BodySize {
            self.bytes
                .as_ref()
                .map_or(actix_web::body::BodySize::None, |b| {
                    actix_web::body::BodySize::Sized(b.len() as u64)
                })
        }
        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            context: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<actix_web::web::Bytes, Self::Error>>> {
            let body = self.get_mut();
            if let Some(bytes) = body.bytes.take() {
                if let Some(guard) = &mut body.guard {
                    guard.observation.delivered_json();
                    guard.delivered_complete();
                }
                return Poll::Ready(Some(Ok(bytes)));
            }
            if let Some(guard) = &mut body.guard {
                match guard.poll_complete(context, false) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(error)) => return Poll::Ready(Some(Err(error))),
                    Poll::Ready(Ok(())) => {}
                }
            }
            Poll::Ready(None)
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
    #[actix_web::test]
    async fn json_drop_records_success_only_after_body_handoff()
    -> Result<(), Box<dyn std::error::Error>> {
        use actix_web::body::MessageBody;
        for delivered in [false, true] {
            let (queue, mut receiver) =
                BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
            let guard = RequestGuard::new(RequestId::try_new("json-handoff")?, Arc::new(queue));
            let mut body =
                json_response(actix_web::web::Bytes::from_static(b"{}"), Some(guard)).into_body();
            assert!(receiver.try_recv().is_none());
            if delivered {
                let chunk = std::future::poll_fn(|cx| Pin::new(&mut body).poll_next(cx)).await;
                assert!(matches!(chunk, Some(Ok(bytes)) if bytes.as_ref() == b"{}"));
                assert!(receiver.try_recv().is_none());
            }
            // Sized bodies may be dropped without a final poll after their only chunk.
            drop(body);
            let Some(GatewayEvent::RequestFinished(event)) = receiver.try_recv() else {
                return Err("missing terminal".into());
            };
            assert_eq!(
                event.outcome,
                if delivered {
                    RequestOutcome::Succeeded
                } else {
                    RequestOutcome::Cancelled
                }
            );
            assert!(receiver.try_recv().is_none());
        }
        Ok(())
    }

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
