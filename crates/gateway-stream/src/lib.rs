//! Bounded canonical-event delivery, cancellation, and semantic-output tracking.

#![deny(unsafe_code)]

use std::{
    error::Error,
    fmt,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use gateway_core::{
    CanonicalEvent, CanonicalEventState, ErrorScope, GatewayError, GatewayErrorCode,
    TransparentRetryGate, TransparentRetryGateFuture,
};
use tokio::sync::{Notify, Semaphore, mpsc};
use tokio_util::sync::CancellationToken;

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-stream";

/// Error returned when a bounded stream capacity cannot be constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamCapacityError {
    /// Tokio channels require at least one queued event slot.
    Zero,
    /// Tokio channels cannot safely allocate more permits than their semaphore supports.
    TooLarge,
}

impl fmt::Display for StreamCapacityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => formatter.write_str("stream capacity must be greater than zero"),
            Self::TooLarge => {
                formatter.write_str("stream capacity exceeds the Tokio channel limit")
            }
        }
    }
}

impl Error for StreamCapacityError {}

/// A validated non-zero bound measured in canonical events, rather than bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamCapacity(NonZeroUsize);

impl StreamCapacity {
    /// Creates a capacity that a bounded Tokio channel can safely accept.
    ///
    /// # Errors
    ///
    /// Returns [`StreamCapacityError::Zero`] when `value` is zero or
    /// [`StreamCapacityError::TooLarge`] when `value` would make Tokio panic.
    pub fn try_new(value: usize) -> Result<Self, StreamCapacityError> {
        let Some(value) = NonZeroUsize::new(value) else {
            return Err(StreamCapacityError::Zero);
        };
        if value.get() > Semaphore::MAX_PERMITS {
            return Err(StreamCapacityError::TooLarge);
        }

        Ok(Self(value))
    }

    /// Returns the exact number of canonical events that may be queued.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0.get()
    }
}

/// Whether one downstream semantic delivery committed the transparent-retry boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirstSemanticEvent {
    /// This delivery is the first client-visible canonical event for the request.
    First,
    /// A prior client-visible canonical event already committed the boundary.
    AlreadyCommitted,
}

/// The event that ended a producer's wait for a downstream retry boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirstSemanticEventWait {
    /// A semantic canonical event was actually handed to the downstream HTTP body.
    Delivered,
    /// The request was cancelled before any semantic event became client-visible.
    Cancelled,
}

/// Monotonic state that records a downstream client-visible canonical-event delivery.
///
/// P1-07 and later's downstream HTTP writer must call [`Self::mark_delivered`] only after the
/// target protocol adapter has successfully written the event to client-visible output.
/// Enqueueing, dequeueing, and pure protocol encoding do not commit this state, and SSE keepalive
/// comments cannot be passed to this API.
#[derive(Clone, Debug, Default)]
pub struct FirstSemanticEventTracker {
    state: Arc<FirstSemanticEventState>,
}

#[derive(Debug, Default)]
struct FirstSemanticEventState {
    committed: AtomicBool,
    committed_notification: Notify,
}

impl FirstSemanticEventTracker {
    /// Records one actually delivered canonical event and returns whether it was the first.
    #[must_use]
    pub fn mark_delivered(&self, _event: &CanonicalEvent) -> FirstSemanticEvent {
        if self.state.committed.swap(true, Ordering::AcqRel) {
            FirstSemanticEvent::AlreadyCommitted
        } else {
            self.state.committed_notification.notify_waiters();
            FirstSemanticEvent::First
        }
    }

    /// Returns whether any canonical event has crossed the client-visible output boundary.
    #[must_use]
    pub fn is_committed(&self) -> bool {
        self.state.committed.load(Ordering::Acquire)
    }

    /// Returns whether no canonical event has crossed the downstream output boundary yet.
    ///
    /// This is only the first-semantic-event portion of a retry decision. Call
    /// [`StreamControl::allows_transparent_retry`] when the cancellation state must also be
    /// considered.
    #[must_use]
    pub fn is_uncommitted(&self) -> bool {
        !self.is_committed()
    }

    /// Waits until a downstream semantic event has actually been delivered.
    ///
    /// A producer uses this after handing its first semantic event to a bounded downstream path
    /// and before pulling later upstream output. It prevents an upstream failure from triggering
    /// a transparent retry while that initial event is still queued but cannot be withdrawn.
    pub async fn wait_until_committed(&self) {
        loop {
            let notified = self.state.committed_notification.notified();
            if self.is_committed() {
                return;
            }
            notified.await;
            if self.is_committed() {
                return;
            }
        }
    }
}

/// The cancellation capability shared by the producer and downstream consumer of one stream.
#[derive(Clone, Debug, Default)]
pub struct StreamCancellation {
    cancellation: CancellationToken,
}

impl StreamCancellation {
    /// Requests cancellation for the producer and consumer of this stream.
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Returns whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Waits until cancellation is requested.
    pub async fn cancelled(&self) {
        self.cancellation.cancelled().await;
    }
}

/// Downstream-only controls for one bounded stream.
///
/// A [`CanonicalEventSender`] has only [`StreamCancellation`], so an upstream source cannot
/// commit the first-semantic-event boundary before the downstream adapter writes an event.
#[derive(Clone, Debug, Default)]
pub struct StreamControl {
    cancellation: StreamCancellation,
    first_semantic_event: FirstSemanticEventTracker,
}

impl StreamControl {
    /// Requests cancellation for the producer and consumer of this stream.
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Returns whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Waits until cancellation is requested.
    pub async fn cancelled(&self) {
        self.cancellation.cancelled().await;
    }

    /// Returns the shared cancellation capability without exposing the delivery tracker.
    #[must_use]
    pub fn cancellation(&self) -> StreamCancellation {
        self.cancellation.clone()
    }

    /// Returns the shared tracker for actual downstream semantic deliveries.
    #[must_use]
    pub fn first_semantic_event_tracker(&self) -> FirstSemanticEventTracker {
        self.first_semantic_event.clone()
    }

    /// Returns whether an attempt may be transparently retried before client-visible output.
    ///
    /// Both conditions are required: no client cancellation and no downstream semantic delivery.
    #[must_use]
    pub fn allows_transparent_retry(&self) -> bool {
        !self.is_cancelled() && self.first_semantic_event.is_uncommitted()
    }

    /// Waits until the first semantic event reaches downstream output or the request is cancelled.
    ///
    /// This is intentionally separate from [`Self::allows_transparent_retry`]. A producer that
    /// has already handed off its first semantic event must use this wait before it reads further
    /// upstream output, because a queued-but-undelivered event cannot be safely followed by a
    /// retried duplicate start.
    pub async fn wait_for_first_semantic_event_or_cancellation(&self) -> FirstSemanticEventWait {
        if self.is_cancelled() {
            return FirstSemanticEventWait::Cancelled;
        }
        if self.first_semantic_event.is_committed() {
            return FirstSemanticEventWait::Delivered;
        }

        tokio::select! {
            biased;
            () = self.cancelled() => FirstSemanticEventWait::Cancelled,
            () = self.first_semantic_event.wait_until_committed() => FirstSemanticEventWait::Delivered,
        }
    }
}

impl TransparentRetryGate for StreamControl {
    fn is_cancelled(&self) -> bool {
        StreamControl::is_cancelled(self)
    }

    fn allows_transparent_retry(&self) -> bool {
        StreamControl::allows_transparent_retry(self)
    }

    fn cancelled(&self) -> TransparentRetryGateFuture<'_> {
        Box::pin(StreamControl::cancelled(self))
    }
}

/// Creates a single-producer, single-consumer bounded canonical-event stream.
///
/// The returned sender is intentionally not cloneable, preserving one ordered canonical source.
/// The receiver must be retained by the downstream consumer until it observes a terminal event or
/// cancels the request.
#[must_use]
pub fn bounded_canonical_stream(
    capacity: StreamCapacity,
) -> (CanonicalEventSender, CanonicalEventStream) {
    let (sender, receiver) = mpsc::channel(capacity.get());
    let control = StreamControl::default();

    (
        CanonicalEventSender {
            sender: Some(sender),
            state: CanonicalEventState::default(),
            cancellation: control.cancellation(),
        },
        CanonicalEventStream {
            receiver,
            state: CanonicalEventState::default(),
            control,
            finished: false,
        },
    )
}

/// The single source that validates and enqueues canonical events with natural backpressure.
#[derive(Debug)]
pub struct CanonicalEventSender {
    sender: Option<mpsc::Sender<CanonicalEvent>>,
    state: CanonicalEventState,
    cancellation: StreamCancellation,
}

impl CanonicalEventSender {
    /// Returns the cancellation handle shared with the downstream consumer.
    ///
    /// The producer intentionally cannot obtain a first-semantic-event tracker; only a
    /// downstream client-visible adapter may commit that boundary after it writes an event.
    #[must_use]
    pub fn cancellation(&self) -> StreamCancellation {
        self.cancellation.clone()
    }

    /// Validates and enqueues one canonical event, waiting for available bounded capacity.
    ///
    /// # Errors
    ///
    /// Returns the P1-03 stream lifecycle error when `event` is invalid, or `Cancelled` with
    /// `Request` scope when cancellation or a downstream disconnect prevents delivery.
    pub async fn send(&mut self, event: CanonicalEvent) -> Result<(), GatewayError> {
        if self.cancellation.is_cancelled() {
            return Err(request_cancelled_error());
        }

        let mut next_state = self.state.clone();
        next_state.apply(&event)?;
        let Some(sender) = self.sender.clone() else {
            return Err(stream_protocol_error());
        };
        let is_terminal = next_state.is_terminal();

        tokio::select! {
            biased;
            () = self.cancellation.cancelled() => Err(request_cancelled_error()),
            result = sender.send(event) => match result {
                Ok(()) => {
                    self.state = next_state;
                    if is_terminal {
                        self.sender = None;
                    }
                    Ok(())
                }
                Err(_) => Err(request_cancelled_error()),
            },
        }
    }
}

/// The bounded downstream view of one ordered canonical-event source.
#[derive(Debug)]
pub struct CanonicalEventStream {
    receiver: mpsc::Receiver<CanonicalEvent>,
    state: CanonicalEventState,
    control: StreamControl,
    finished: bool,
}

impl CanonicalEventStream {
    /// Returns the control handle shared with the upstream producer.
    #[must_use]
    pub fn control(&self) -> StreamControl {
        self.control.clone()
    }

    /// Receives the next canonical event in source order.
    ///
    /// # Errors
    ///
    /// Returns `Cancelled` with `Request` scope after client cancellation, and returns the P1-03
    /// source-completion error once if the source closes before a terminal event.
    pub async fn recv(&mut self) -> Result<Option<CanonicalEvent>, GatewayError> {
        if self.finished {
            return Ok(None);
        }

        tokio::select! {
            biased;
            () = self.control.cancelled() => {
                self.finished = true;
                Err(request_cancelled_error())
            }
            event = self.receiver.recv() => self.receive(event),
        }
    }

    fn receive(
        &mut self,
        event: Option<CanonicalEvent>,
    ) -> Result<Option<CanonicalEvent>, GatewayError> {
        let Some(event) = event else {
            self.finished = true;
            self.state.finish()?;
            return Ok(None);
        };

        if let Err(error) = self.state.apply(&event) {
            self.finished = true;
            return Err(error);
        }
        self.finished = self.state.is_terminal();

        Ok(Some(event))
    }
}

impl Drop for CanonicalEventStream {
    fn drop(&mut self) {
        if !self.state.is_terminal() {
            self.control.cancel();
        }
    }
}

const fn request_cancelled_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::Cancelled, ErrorScope::Request)
}

const fn stream_protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        future::{Future, poll_fn},
        task::Poll,
    };

    use super::{
        FirstSemanticEvent, FirstSemanticEventWait, StreamCapacity, StreamCapacityError,
        bounded_canonical_stream,
    };
    use gateway_core::{
        CanonicalEvent, ErrorScope, GatewayError, GatewayErrorCode, MessageRole, MessageStart,
        RawExtensions, ResponseEnd, ResponseId, ResponseStart, StreamError,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    fn response_start() -> Result<CanonicalEvent, Box<dyn Error>> {
        let response_id = ResponseId::try_new("response-01")?;

        Ok(CanonicalEvent::ResponseStart(ResponseStart {
            response_id,
            extensions: RawExtensions::default(),
        }))
    }

    fn message_start() -> CanonicalEvent {
        CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
            extensions: RawExtensions::default(),
        })
    }

    fn assert_error<T>(result: Result<T, GatewayError>, code: GatewayErrorCode, scope: ErrorScope) {
        assert!(matches!(
            result,
            Err(error) if error.code() == code && error.scope() == scope
        ));
    }

    #[test]
    fn stream_capacity_rejects_unsafe_values() {
        assert_eq!(StreamCapacity::try_new(0), Err(StreamCapacityError::Zero));
        assert_eq!(
            StreamCapacity::try_new(usize::MAX),
            Err(StreamCapacityError::TooLarge)
        );
    }

    #[tokio::test]
    async fn valid_terminal_sequence_is_delivered_in_order_and_finishes_normally() -> TestResult {
        let capacity = StreamCapacity::try_new(2)?;
        let (mut sender, mut stream) = bounded_canonical_stream(capacity);
        let control = stream.control();
        let start = response_start()?;
        let end = CanonicalEvent::ResponseEnd(ResponseEnd::default());

        sender.send(start.clone()).await?;
        sender.send(end.clone()).await?;

        assert_eq!(stream.recv().await?, Some(start));
        assert_eq!(stream.recv().await?, Some(end));
        assert_eq!(stream.recv().await?, None);
        drop(stream);
        assert!(!control.is_cancelled());
        assert_error(
            sender.send(message_start()).await,
            GatewayErrorCode::UpstreamProtocolError,
            ErrorScope::Stream,
        );

        Ok(())
    }

    #[tokio::test]
    async fn invalid_events_do_not_occupy_capacity_or_advance_the_source() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (mut sender, mut stream) = bounded_canonical_stream(capacity);
        let start = response_start()?;

        assert_error(
            sender.send(message_start()).await,
            GatewayErrorCode::UpstreamProtocolError,
            ErrorScope::Stream,
        );
        sender.send(start.clone()).await?;
        assert_eq!(stream.recv().await?, Some(start));

        Ok(())
    }

    #[tokio::test]
    async fn source_close_without_a_terminal_event_is_reported_once() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (sender, mut stream) = bounded_canonical_stream(capacity);

        drop(sender);
        assert_error(
            stream.recv().await,
            GatewayErrorCode::StreamTruncated,
            ErrorScope::Stream,
        );
        assert_eq!(stream.recv().await?, None);

        Ok(())
    }

    #[tokio::test]
    async fn source_close_after_a_delivered_event_remains_truncated() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (mut sender, mut stream) = bounded_canonical_stream(capacity);
        let start = response_start()?;

        sender.send(start.clone()).await?;
        drop(sender);

        assert_eq!(stream.recv().await?, Some(start));
        assert_error(
            stream.recv().await,
            GatewayErrorCode::StreamTruncated,
            ErrorScope::Stream,
        );
        assert_eq!(stream.recv().await?, None);

        Ok(())
    }

    #[tokio::test]
    async fn stream_error_remains_a_terminal_canonical_event() -> TestResult {
        let capacity = StreamCapacity::try_new(2)?;
        let (mut sender, mut stream) = bounded_canonical_stream(capacity);
        let start = response_start()?;
        let stream_error = CanonicalEvent::StreamError(StreamError {
            error: GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider),
        });

        sender.send(start.clone()).await?;
        sender.send(stream_error.clone()).await?;

        assert_eq!(stream.recv().await?, Some(start));
        assert_eq!(stream.recv().await?, Some(stream_error));
        assert_eq!(stream.recv().await?, None);

        Ok(())
    }

    #[tokio::test]
    async fn slow_consumer_blocks_the_next_send_until_capacity_is_released() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (mut sender, mut stream) = bounded_canonical_stream(capacity);
        let start = response_start()?;
        let second_event = message_start();
        let expected_second_event = second_event.clone();

        sender.send(start.clone()).await?;
        let mut blocked_send = Box::pin(sender.send(second_event));
        let first_poll = poll_fn(|context| Poll::Ready(blocked_send.as_mut().poll(context))).await;
        assert!(matches!(first_poll, Poll::Pending));

        assert_eq!(stream.recv().await?, Some(start));
        blocked_send.await?;
        assert_eq!(stream.recv().await?, Some(expected_second_event));

        Ok(())
    }

    #[tokio::test]
    async fn explicit_cancellation_unblocks_a_producer_waiting_for_capacity() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (mut sender, mut stream) = bounded_canonical_stream(capacity);
        let control = stream.control();
        let start = response_start()?;
        let second_event = message_start();

        sender.send(start).await?;
        let mut blocked_send = Box::pin(sender.send(second_event));
        let first_poll = poll_fn(|context| Poll::Ready(blocked_send.as_mut().poll(context))).await;
        assert!(matches!(first_poll, Poll::Pending));

        control.cancel();
        assert_error(
            blocked_send.await,
            GatewayErrorCode::Cancelled,
            ErrorScope::Request,
        );
        assert_error(
            stream.recv().await,
            GatewayErrorCode::Cancelled,
            ErrorScope::Request,
        );
        assert_eq!(stream.recv().await?, None);

        Ok(())
    }

    #[tokio::test]
    async fn dropping_a_consumer_unblocks_a_producer_waiting_for_capacity() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (mut sender, stream) = bounded_canonical_stream(capacity);

        sender.send(response_start()?).await?;
        let mut blocked_send = Box::pin(sender.send(message_start()));
        let first_poll = poll_fn(|context| Poll::Ready(blocked_send.as_mut().poll(context))).await;
        assert!(matches!(first_poll, Poll::Pending));

        drop(stream);
        assert_error(
            blocked_send.await,
            GatewayErrorCode::Cancelled,
            ErrorScope::Request,
        );

        Ok(())
    }

    #[tokio::test]
    async fn dropping_the_consumer_cancels_later_producer_delivery() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (mut sender, stream) = bounded_canonical_stream(capacity);
        let control = stream.control();

        drop(stream);
        assert!(control.is_cancelled());
        assert_error(
            sender.send(response_start()?).await,
            GatewayErrorCode::Cancelled,
            ErrorScope::Request,
        );

        Ok(())
    }

    #[tokio::test]
    async fn only_explicit_downstream_delivery_commits_the_retry_boundary() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (mut sender, mut stream) = bounded_canonical_stream(capacity);
        let tracker = stream.control().first_semantic_event_tracker();
        let start = response_start()?;

        sender.send(start.clone()).await?;
        assert!(tracker.is_uncommitted());
        assert!(stream.control().allows_transparent_retry());
        let received = stream.recv().await?;
        assert_eq!(received, Some(start));
        assert!(tracker.is_uncommitted());
        assert!(stream.control().allows_transparent_retry());
        assert!(received.is_some());
        if let Some(event) = received {
            assert_eq!(tracker.mark_delivered(&event), FirstSemanticEvent::First);
            assert!(!tracker.is_uncommitted());
            assert!(!stream.control().allows_transparent_retry());
            assert_eq!(
                tracker.mark_delivered(&event),
                FirstSemanticEvent::AlreadyCommitted
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn cancellation_keeps_fse_uncommitted_but_forbids_transparent_retry() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (mut sender, stream) = bounded_canonical_stream(capacity);
        let control = stream.control();
        let tracker = control.first_semantic_event_tracker();

        sender.send(response_start()?).await?;
        control.cancel();

        assert!(tracker.is_uncommitted());
        assert!(!control.allows_transparent_retry());

        Ok(())
    }

    #[tokio::test]
    async fn first_semantic_wait_resolves_only_after_delivery() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (mut sender, stream) = bounded_canonical_stream(capacity);
        let control = stream.control();
        let tracker = control.first_semantic_event_tracker();
        let start = response_start()?;

        sender.send(start.clone()).await?;
        let wait_control = control.clone();
        let waiter = tokio::spawn(async move {
            wait_control
                .wait_for_first_semantic_event_or_cancellation()
                .await
        });
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());

        assert_eq!(tracker.mark_delivered(&start), FirstSemanticEvent::First);
        assert_eq!(waiter.await?, FirstSemanticEventWait::Delivered);
        Ok(())
    }

    #[tokio::test]
    async fn first_semantic_wait_resolves_on_cancellation_without_delivery() -> TestResult {
        let capacity = StreamCapacity::try_new(1)?;
        let (_sender, stream) = bounded_canonical_stream(capacity);
        let control = stream.control();
        let wait_control = control.clone();
        let waiter = tokio::spawn(async move {
            wait_control
                .wait_for_first_semantic_event_or_cancellation()
                .await
        });
        tokio::task::yield_now().await;

        control.cancel();
        assert_eq!(waiter.await?, FirstSemanticEventWait::Cancelled);
        Ok(())
    }
}
