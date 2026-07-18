//! Deterministic, pull-only Provider fixtures for the P1 vertical slice.

use std::time::Duration;

use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalRequest, ErrorScope, GatewayError,
    GatewayErrorCode, ProviderId, RequestContext,
};

use crate::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};

/// One immutable canonical event scheduled relative to the preceding pull.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockEmission {
    after: Duration,
    event: CanonicalEvent,
}

impl MockEmission {
    /// Creates one scheduled canonical event.
    #[must_use]
    pub const fn new(after: Duration, event: CanonicalEvent) -> Self {
        Self { after, event }
    }

    /// Returns the deterministic delay before this event becomes available.
    #[must_use]
    pub const fn after(&self) -> Duration {
        self.after
    }

    /// Returns the retained canonical event.
    #[must_use]
    pub const fn event(&self) -> &CanonicalEvent {
        &self.event
    }
}

/// A validated immutable script for [`DeterministicMockProvider`].
///
/// Event scripts are validated fully at construction time, so a source cannot discover a malformed
/// lifecycle only after it has started emitting output. A separate pre-start error fixture models
/// failures that happen before any canonical response event exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockFixture(MockFixtureKind);

#[derive(Clone, Debug, Eq, PartialEq)]
enum MockFixtureKind {
    Events(Vec<MockEmission>),
    PreStartError {
        after: Duration,
        error: GatewayError,
    },
}

impl MockFixture {
    /// Validates and stores a complete canonical event script.
    ///
    /// # Errors
    ///
    /// Returns the existing canonical stream lifecycle error when `emissions` is incomplete or
    /// invalid. A valid script finishes with either `ResponseEnd` or `StreamError`.
    pub fn try_events(emissions: Vec<MockEmission>) -> Result<Self, GatewayError> {
        let mut state = CanonicalEventState::default();
        for emission in &emissions {
            state.apply(emission.event())?;
        }
        state.finish()?;

        Ok(Self(MockFixtureKind::Events(emissions)))
    }

    /// Creates a deterministic failure that occurs before a canonical response starts.
    #[must_use]
    pub const fn pre_start_error(after: Duration, error: GatewayError) -> Self {
        Self(MockFixtureKind::PreStartError { after, error })
    }
}

/// A Provider that returns the same validated fixture for every execution.
///
/// It does not inspect models, route requests, spawn background tasks, or use random state. The
/// request and context remain inputs so P1-07 can exercise the real boundary without a second mock
/// interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeterministicMockProvider {
    provider_id: ProviderId,
    fixture: MockFixture,
}

impl DeterministicMockProvider {
    /// Creates a deterministic Provider from a stable family identifier and validated fixture.
    #[must_use]
    pub const fn new(provider_id: ProviderId, fixture: MockFixture) -> Self {
        Self {
            provider_id,
            fixture,
        }
    }
}

impl ProviderAdapter for DeterministicMockProvider {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
}

impl InferenceAdapter for DeterministicMockProvider {
    fn execute(
        &self,
        _context: RequestContext,
        _request: CanonicalRequest,
    ) -> ProviderFuture<'_, Result<Box<dyn CanonicalEventSource>, GatewayError>> {
        let fixture = self.fixture.clone();
        Box::pin(async move {
            match fixture.0 {
                MockFixtureKind::Events(emissions) => {
                    let source: Box<dyn CanonicalEventSource> = Box::new(DeterministicMockSource {
                        emissions,
                        next_index: 0,
                    });
                    Ok(source)
                }
                MockFixtureKind::PreStartError { after, error } => {
                    if !after.is_zero() {
                        tokio::time::sleep(after).await;
                    }
                    Err(error)
                }
            }
        })
    }
}

#[derive(Debug)]
struct DeterministicMockSource {
    emissions: Vec<MockEmission>,
    next_index: usize,
}

impl CanonicalEventSource for DeterministicMockSource {
    fn next_event(&mut self) -> ProviderFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            let Some(emission) = self.emissions.get(self.next_index).cloned() else {
                return Ok(None);
            };

            if !emission.after().is_zero() {
                tokio::time::sleep(emission.after()).await;
            }
            self.next_index = self.next_index.checked_add(1).ok_or_else(internal_error)?;
            Ok(Some(emission.event))
        })
    }
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        task::{Context, Poll, Waker},
        time::Duration,
    };

    use gateway_core::{
        CanonicalEvent, CanonicalRequest, ErrorScope, GatewayError, GatewayErrorCode, ProviderId,
        RawExtensions, RequestContext, RequestId,
    };
    use serde::Deserialize;

    use super::{DeterministicMockProvider, MockEmission, MockFixture};
    use crate::{CanonicalEventSource, InferenceAdapter, ProviderAdapter};

    type TestResult = Result<(), Box<dyn Error>>;

    #[derive(Deserialize)]
    struct FixtureFile {
        emissions: Vec<FixtureEmission>,
    }

    #[derive(Deserialize)]
    struct FixtureEmission {
        after_ms: u64,
        event: CanonicalEvent,
    }

    fn fixture(input: &str) -> Result<MockFixture, Box<dyn Error>> {
        let fixture: FixtureFile = serde_json::from_str(input)?;
        let emissions = fixture
            .emissions
            .into_iter()
            .map(|emission| {
                MockEmission::new(Duration::from_millis(emission.after_ms), emission.event)
            })
            .collect();
        Ok(MockFixture::try_events(emissions)?)
    }

    fn provider(fixture: MockFixture) -> Result<DeterministicMockProvider, Box<dyn Error>> {
        Ok(DeterministicMockProvider::new(
            ProviderId::try_new("deterministic-mock")?,
            fixture,
        ))
    }

    fn context() -> Result<RequestContext, Box<dyn Error>> {
        Ok(RequestContext::new(RequestId::try_new("request-01")?))
    }

    fn request() -> CanonicalRequest {
        CanonicalRequest {
            requested_model: "mock-model".to_owned(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            extensions: RawExtensions::default(),
        }
    }

    async fn collect(
        mut source: Box<dyn CanonicalEventSource>,
    ) -> Result<Vec<CanonicalEvent>, GatewayError> {
        let mut events = Vec::new();
        while let Some(event) = source.next_event().await? {
            events.push(event);
        }
        Ok(events)
    }

    #[tokio::test]
    async fn text_fixture_is_reusable_and_ends_cleanly() -> TestResult {
        let provider = provider(fixture(include_str!(
            "../../../tests/fixtures/provider/mock-text.json"
        ))?)?;
        let identity: &dyn ProviderAdapter = &provider;
        let adapter: &dyn InferenceAdapter = &provider;
        assert_eq!(identity.provider_id().as_str(), "deterministic-mock");

        let first = collect(adapter.execute(context()?, request()).await?).await?;
        let second = collect(adapter.execute(context()?, request()).await?).await?;

        assert_eq!(first, second);
        assert_eq!(first.len(), 6);
        assert!(matches!(first[2], CanonicalEvent::TextDelta(_)));
        assert!(matches!(first.last(), Some(CanonicalEvent::ResponseEnd(_))));
        Ok(())
    }

    #[tokio::test]
    async fn tool_fixture_preserves_call_order_and_complete_arguments() -> TestResult {
        let provider = provider(fixture(include_str!(
            "../../../tests/fixtures/provider/mock-tool.json"
        ))?)?;
        let events = collect(provider.execute(context()?, request()).await?).await?;

        assert!(matches!(
            &events[2],
            CanonicalEvent::ToolCallStart(value)
                if value.call_id == "mock-weather-call" && value.name == "lookup_weather"
        ));
        assert!(matches!(
            &events[3],
            CanonicalEvent::ToolCallArgumentsDelta(value)
                if value.delta == r#"{"city":"Jakarta"}"#
        ));
        assert!(matches!(
            &events[4],
            CanonicalEvent::ToolCallEnd(value)
                if value.arguments.get() == r#"{"city":"Jakarta"}"#
        ));
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::ResponseEnd(_))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn stream_error_stays_in_the_source_while_pre_start_error_is_execute_error() -> TestResult
    {
        let stream_provider = provider(fixture(include_str!(
            "../../../tests/fixtures/provider/mock-stream-error.json"
        ))?)?;
        let events = collect(stream_provider.execute(context()?, request()).await?).await?;
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::StreamError(error))
                if error.error.code() == GatewayErrorCode::ProviderTransient
                    && error.error.scope() == ErrorScope::Provider
        ));

        let pre_start_provider = provider(MockFixture::pre_start_error(
            Duration::ZERO,
            GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider),
        ))?;
        assert!(matches!(
            pre_start_provider.execute(context()?, request()).await,
            Err(error)
                if error.code() == GatewayErrorCode::ProviderTransient
                    && error.scope() == ErrorScope::Provider
        ));
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn delay_fixture_waits_before_each_emission() -> TestResult {
        let provider = provider(fixture(include_str!(
            "../../../tests/fixtures/provider/mock-delay.json"
        ))?)?;
        let mut source = provider.execute(context()?, request()).await?;

        let mut first = source.next_event();
        let mut task_context = Context::from_waker(Waker::noop());
        assert!(matches!(
            first.as_mut().poll(&mut task_context),
            Poll::Pending
        ));
        tokio::time::advance(Duration::from_millis(24)).await;
        assert!(matches!(
            first.as_mut().poll(&mut task_context),
            Poll::Pending
        ));
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(matches!(
            first.await?,
            Some(CanonicalEvent::ResponseStart(_))
        ));

        let mut second = source.next_event();
        assert!(matches!(
            second.as_mut().poll(&mut task_context),
            Poll::Pending
        ));
        tokio::time::advance(Duration::from_millis(9)).await;
        assert!(matches!(
            second.as_mut().poll(&mut task_context),
            Poll::Pending
        ));
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(matches!(
            second.await?,
            Some(CanonicalEvent::ResponseEnd(_))
        ));
        assert!(source.next_event().await?.is_none());
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn dropping_a_pending_pull_does_not_consume_or_spawn_work() -> TestResult {
        let provider = provider(fixture(include_str!(
            "../../../tests/fixtures/provider/mock-delay.json"
        ))?)?;
        let mut source = provider.execute(context()?, request()).await?;
        let mut task_context = Context::from_waker(Waker::noop());

        let mut cancelled_pull = source.next_event();
        assert!(matches!(
            cancelled_pull.as_mut().poll(&mut task_context),
            Poll::Pending
        ));
        drop(cancelled_pull);
        tokio::time::advance(Duration::from_millis(25)).await;

        let mut retry = source.next_event();
        assert!(matches!(
            retry.as_mut().poll(&mut task_context),
            Poll::Pending
        ));
        tokio::time::advance(Duration::from_millis(25)).await;
        assert!(matches!(
            retry.await?,
            Some(CanonicalEvent::ResponseStart(_))
        ));
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn dropping_a_delayed_pre_start_execution_leaves_no_shared_work() -> TestResult {
        let provider = provider(MockFixture::pre_start_error(
            Duration::from_millis(25),
            GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider),
        ))?;
        let mut task_context = Context::from_waker(Waker::noop());

        let mut cancelled_execution = provider.execute(context()?, request());
        assert!(matches!(
            cancelled_execution.as_mut().poll(&mut task_context),
            Poll::Pending
        ));
        drop(cancelled_execution);
        tokio::time::advance(Duration::from_millis(25)).await;

        let mut retry = provider.execute(context()?, request());
        assert!(matches!(
            retry.as_mut().poll(&mut task_context),
            Poll::Pending
        ));
        tokio::time::advance(Duration::from_millis(25)).await;
        assert!(matches!(
            retry.await,
            Err(error)
                if error.code() == GatewayErrorCode::ProviderTransient
                    && error.scope() == ErrorScope::Provider
        ));
        Ok(())
    }

    #[test]
    fn malformed_or_truncated_scripts_are_rejected_before_execution() -> TestResult {
        let text_only: CanonicalEvent =
            serde_json::from_str(r#"{"text_delta":{"text":"not started","extensions":{}}}"#)?;
        let incomplete_start: CanonicalEvent = serde_json::from_str(
            r#"{"response_start":{"response_id":"unfinished","extensions":{}}}"#,
        )?;

        for emissions in [
            vec![MockEmission::new(Duration::ZERO, text_only)],
            vec![MockEmission::new(Duration::ZERO, incomplete_start)],
        ] {
            assert!(matches!(
                MockFixture::try_events(emissions),
                Err(error)
                    if (error.code() == GatewayErrorCode::UpstreamProtocolError
                        || error.code() == GatewayErrorCode::StreamTruncated)
                        && error.scope() == ErrorScope::Stream
            ));
        }
        Ok(())
    }
}
