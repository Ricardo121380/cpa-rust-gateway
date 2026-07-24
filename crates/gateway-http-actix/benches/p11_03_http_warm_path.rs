//! P11-03 offline Criterion baseline for the in-process Actix Responses warm path.
//!
//! The service uses the real public route, parser, Client-Key admission, Canonical response
//! encoding, Router facade, and deterministic Mock Provider. No socket, upstream transport,
//! Credential store, account, environment variable, or server state participates.

#![deny(unsafe_code)]
// Criterion's macro creates an internal entry point with no place for a doc comment.
#![allow(missing_docs)]

use std::{hint::black_box, sync::Arc, time::Duration};

use actix_web::{
    App,
    http::{StatusCode, header},
    test, web,
};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use gateway_auth::{ClientKeyAuthenticator, InMemoryClientKey, InMemoryClientKeyAuthenticator};
use gateway_core::{
    CanonicalEvent, ClientKeyId, ErrorScope, GatewayError, GatewayErrorCode, MessageEnd,
    MessageRole, MessageStart, ProviderId, RawExtensions, RequestContext, RequestId, ResponseEnd,
    ResponseId, ResponseStart, TextDelta,
};
use gateway_http_actix::{
    ResponsesHttpState, ResponsesMetadataFactory, configure, default_stream_capacity,
};
use gateway_router::{DeterministicMockEmission, DeterministicMockResponsesExecutor};
use protocol_openai_responses::OpenAiResponseMetadata;

const CLIENT_KEY: &str = "p11-03-benchmark-client-key";
const REQUEST_PAYLOAD: &str = r#"{"model":"p11-03-model","input":"benchmark"}"#;

fn abort_on_error<T, E>(value: Result<T, E>) -> T {
    match value {
        Ok(value) => value,
        Err(_) => std::process::abort(),
    }
}

#[derive(Debug)]
struct FixedMetadata;

impl ResponsesMetadataFactory for FixedMetadata {
    fn request_context(&self) -> Result<RequestContext, GatewayError> {
        let request_id = RequestId::try_new("p11-03-http-request").map_err(|_| {
            GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
        })?;
        Ok(RequestContext::new(request_id))
    }

    fn response_metadata(
        &self,
        public_model: &str,
    ) -> Result<OpenAiResponseMetadata, GatewayError> {
        OpenAiResponseMetadata::try_new(public_model, 1)
    }
}

fn text_lifecycle() -> Vec<CanonicalEvent> {
    vec![
        CanonicalEvent::ResponseStart(ResponseStart {
            response_id: abort_on_error(ResponseId::try_new("p11-03-http-response")),
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
    ]
}

fn state() -> ResponsesHttpState {
    let emissions = text_lifecycle()
        .into_iter()
        .map(|event| DeterministicMockEmission::new(Duration::ZERO, event))
        .collect();
    let executor = abort_on_error(DeterministicMockResponsesExecutor::try_new(
        abort_on_error(ProviderId::try_new("p11-03-http-provider")),
        emissions,
    ));
    let key = abort_on_error(InMemoryClientKey::try_new(
        CLIENT_KEY,
        abort_on_error(ClientKeyId::try_new("p11-03-http-client")),
        true,
    ));
    let authenticator: Arc<dyn ClientKeyAuthenticator> =
        Arc::new(abort_on_error(InMemoryClientKeyAuthenticator::try_new([
            key,
        ])));
    ResponsesHttpState::with_metadata(
        Arc::new(executor),
        Arc::new(FixedMetadata),
        authenticator,
        abort_on_error(default_stream_capacity()),
    )
}

fn http_responses_warm_path(criterion: &mut Criterion) {
    let runtime = abort_on_error(
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build(),
    );
    let app = runtime.block_on(async {
        test::init_service(
            App::new()
                .app_data(web::Data::new(state()))
                .configure(configure),
        )
        .await
    });

    let mut group = criterion.benchmark_group("p11_03_http_responses_warm_path");
    group.throughput(Throughput::Elements(1));
    group.bench_function("non_streaming_text_response", |bencher| {
        bencher.to_async(&runtime).iter(|| async {
            let request = test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header((header::AUTHORIZATION, format!("Bearer {CLIENT_KEY}")))
                .set_payload(REQUEST_PAYLOAD)
                .to_request();
            let response = test::call_service(&app, request).await;
            if response.status() != StatusCode::OK {
                std::process::abort();
            }
            let body = test::read_body(response).await;
            if body.is_empty() {
                std::process::abort();
            }
            black_box(body)
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
    targets = http_responses_warm_path
}
criterion_main!(benchmarks);
