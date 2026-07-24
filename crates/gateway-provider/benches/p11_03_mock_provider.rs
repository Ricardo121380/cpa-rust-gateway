//! P11-03 offline Criterion baseline for the real deterministic Mock Provider.
//!
//! This intentionally covers only the Provider's zero-delay Canonical lifecycle. It has no
//! network, listener, ambient configuration, Credential, or upstream transport input.

#![deny(unsafe_code)]
// Criterion's macro creates an internal entry point with no place for a doc comment.
#![allow(missing_docs)]

use std::{hint::black_box, time::Duration};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use gateway_core::{
    CanonicalEvent, CanonicalRequest, MessageEnd, MessageRole, MessageStart, ProviderId,
    RawExtensions, RequestContext, RequestId, ResponseEnd, ResponseId, ResponseStart, TextDelta,
};
use gateway_provider::{
    CanonicalEventSource, DeterministicMockProvider, InferenceAdapter, MockEmission, MockFixture,
};

const EVENT_COUNT: usize = 5;
const EVENT_THROUGHPUT_ELEMENTS: u64 = 5;

fn abort_on_error<T, E>(value: Result<T, E>) -> T {
    match value {
        Ok(value) => value,
        Err(_) => std::process::abort(),
    }
}

fn provider() -> DeterministicMockProvider {
    let events = vec![
        CanonicalEvent::ResponseStart(ResponseStart {
            response_id: abort_on_error(ResponseId::try_new("p11-03-provider-response")),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::TextDelta(TextDelta {
            text: "deterministic benchmark response".to_owned(),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageEnd(MessageEnd::default()),
        CanonicalEvent::ResponseEnd(ResponseEnd::default()),
    ];
    let emissions = events
        .into_iter()
        .map(|event| MockEmission::new(Duration::ZERO, event))
        .collect();
    let fixture = abort_on_error(MockFixture::try_events(emissions));
    DeterministicMockProvider::new(
        abort_on_error(ProviderId::try_new("p11-03-mock-provider")),
        fixture,
    )
}

fn context() -> RequestContext {
    RequestContext::new(abort_on_error(RequestId::try_new(
        "p11-03-provider-request",
    )))
}

fn request() -> CanonicalRequest {
    CanonicalRequest {
        requested_model: "p11-03-model".to_owned(),
        messages: Vec::new(),
        tools: Vec::new(),
        thinking: None,
        prompt_cache_key: None,
        prompt_cache_retention: None,
        extensions: RawExtensions::default(),
    }
}

async fn drain(source: Box<dyn CanonicalEventSource>) -> usize {
    let mut source = source;
    let mut count = 0_usize;
    loop {
        match source.next_event().await {
            Ok(Some(event)) => {
                black_box(event);
                count = count.saturating_add(1);
            }
            Ok(None) => return count,
            Err(_) => std::process::abort(),
        }
    }
}

fn mock_provider_canonical_drain(criterion: &mut Criterion) {
    let runtime = abort_on_error(
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build(),
    );
    let provider = provider();
    let context = context();
    let request = request();
    let mut group = criterion.benchmark_group("p11_03_mock_provider_canonical_drain");
    group.throughput(Throughput::Elements(EVENT_THROUGHPUT_ELEMENTS));
    group.bench_function("zero_delay_text_lifecycle", |bencher| {
        bencher.to_async(&runtime).iter(|| async {
            let Ok(source) = provider
                .execute(black_box(context.clone()), black_box(request.clone()))
                .await
            else {
                std::process::abort();
            };
            let count = drain(source).await;
            if count != EVENT_COUNT {
                std::process::abort();
            }
            black_box(count)
        });
    });
    group.finish();
}

criterion_group! {
    name = benchmarks;
    config = Criterion::default()
        .sample_size(30)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = mock_provider_canonical_drain
}
criterion_main!(benchmarks);
