//! P5-08 deterministic cancellation schedules for the bounded Canonical stream.
//!
//! The property covers cancellation before and after first semantic delivery, including repeated
//! cancellation. It uses no HTTP, Provider, filesystem, or external configuration.

#![deny(unsafe_code)]

use std::{error::Error, io};

use gateway_core::{
    CanonicalEvent, ErrorScope, GatewayError, GatewayErrorCode, MessageRole, MessageStart,
    RawExtensions, ResponseId, ResponseStart,
};
use gateway_stream::{FirstSemanticEvent, StreamCapacity, bounded_canonical_stream};
use proptest::{
    prelude::any,
    test_runner::{Config as ProptestConfig, RngSeed, TestCaseError, TestRunner},
};

type TestResult = Result<(), Box<dyn Error>>;

const FIXED_CANCELLATION_SEED: u64 = 0x5035_3038_4341_4E43;
const FIXED_CASES: u32 = 128;

#[test]
fn fixed_seed_cancellation_schedules_preserve_the_delivery_boundary() -> TestResult {
    let strategy = (any::<bool>(), 1_u8..=16_u8);
    let config = ProptestConfig {
        cases: FIXED_CASES,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(FIXED_CANCELLATION_SEED),
        ..ProptestConfig::default()
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?;
    let mut runner = TestRunner::new(config);
    let result = runner.run(&strategy, |(deliver_first, cancellation_count)| {
        runtime
            .block_on(run_cancellation_case(deliver_first, cancellation_count))
            .map_err(|error| TestCaseError::fail(error.to_string()))
    });

    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(failure(format!(
            "cancellation property suite seed {FIXED_CANCELLATION_SEED} failed: {error}"
        ))),
    }
}

async fn run_cancellation_case(deliver_first: bool, cancellation_count: u8) -> TestResult {
    let (mut sender, mut stream) = bounded_canonical_stream(StreamCapacity::try_new(1)?);
    let control = stream.control();
    let tracker = control.first_semantic_event_tracker();
    let start = response_start()?;
    sender.send(start.clone()).await?;

    if deliver_first {
        assert_eq!(stream.recv().await?, Some(start.clone()));
        assert_eq!(tracker.mark_delivered(&start), FirstSemanticEvent::First);
        assert!(!tracker.is_uncommitted());
    } else {
        assert!(tracker.is_uncommitted());
    }

    for _ in 0..cancellation_count {
        control.cancel();
    }
    assert!(control.is_cancelled());
    assert!(!control.allows_transparent_retry());
    assert_cancelled(sender.send(message_start()).await);
    assert_cancelled(stream.recv().await);
    assert_eq!(stream.recv().await?, None);
    Ok(())
}

fn response_start() -> Result<CanonicalEvent, Box<dyn Error>> {
    Ok(CanonicalEvent::ResponseStart(ResponseStart {
        response_id: ResponseId::try_new("p5-08-cancellation")?,
        extensions: RawExtensions::default(),
    }))
}

fn message_start() -> CanonicalEvent {
    CanonicalEvent::MessageStart(MessageStart {
        role: MessageRole("assistant".to_owned()),
        extensions: RawExtensions::default(),
    })
}

fn assert_cancelled<T>(result: Result<T, GatewayError>) {
    assert!(matches!(
        result,
        Err(error)
            if error.code() == GatewayErrorCode::Cancelled
                && error.scope() == ErrorScope::Request
    ));
}

fn failure(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::other(message.into()))
}
