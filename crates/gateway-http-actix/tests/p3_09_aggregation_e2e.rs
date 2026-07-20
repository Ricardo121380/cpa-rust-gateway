//! P3-09 composition tests for the bounded `OpenAI` Responses aggregation slice.
//!
//! The test owns two loopback-only deterministic peers. Shared routing, admitted transport,
//! decoding, and event composition lives in `tests/support/p3_aggregation.rs` so P3-10 exercises
//! exactly the same test-only path against its explicitly authorized real targets.

#![deny(unsafe_code)]

mod support;

use std::{
    error::Error,
    io,
    net::{IpAddr, Ipv4Addr},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use actix_web::{
    App,
    http::{StatusCode, header},
    test, web,
};
use gateway_core::{
    AttemptOutcome, AttemptRetryDecision, EgressPolicyId, GatewayErrorCode, GatewayEvent,
    GatewayEventSink,
};
use gateway_http_actix::configure;
use gateway_observability::{BoundedEventQueue, EventQueueConfig};
use gateway_upstream::{
    EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput,
    EgressScheme, RedirectPolicy, UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};
use provider_openai_compatible::OpenAiResponsesEndpoint;
use serde_json::Value;
use support::p3_aggregation::{AggregationEndpoint, RequestIdMode, build_aggregation_harness};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, Notify},
    task::JoinHandle,
    time,
};

type TestResult = Result<(), Box<dyn Error>>;

const LOOPBACK_ADDRESS: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const PUBLIC_MODEL: &str = "minimax-m3";
const MODEL_ALIAS: &str = "minimax-m3-alias";
const ROUTE_ID: &str = "p3-09-route";
const REQUEST_ID: &str = "p3-09-request";
const MAX_HTTP_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy)]
struct StaticLoopbackResolver;

impl EgressDnsResolver for StaticLoopbackResolver {
    fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![LOOPBACK_ADDRESS])
    }
}

#[derive(Clone)]
struct MockEndpoint {
    label: String,
    host: String,
    port: u16,
}

#[derive(Clone)]
enum MockBehavior {
    JsonSuccess { text: String },
    Status { code: u16 },
    StreamingStall(StreamingCancellation),
}

#[derive(Clone)]
struct StreamingCancellation {
    closed: Arc<AtomicBool>,
    closed_notify: Arc<Notify>,
}

impl StreamingCancellation {
    async fn wait_for_close(&self) -> bool {
        if self.closed.load(Ordering::Acquire) {
            return true;
        }
        time::timeout(Duration::from_secs(1), self.closed_notify.notified())
            .await
            .is_ok()
    }
}

struct MockUpstream {
    endpoint: MockEndpoint,
    models: Arc<Mutex<Vec<String>>>,
    requests: Arc<AtomicUsize>,
    task: JoinHandle<()>,
    streaming_cancellation: Option<StreamingCancellation>,
}

impl MockUpstream {
    async fn json_success(label: &str, text: &str) -> Result<Self, io::Error> {
        Self::spawn(
            label,
            MockBehavior::JsonSuccess {
                text: text.to_owned(),
            },
        )
        .await
    }

    async fn status(label: &str, code: u16) -> Result<Self, io::Error> {
        Self::spawn(label, MockBehavior::Status { code }).await
    }

    async fn streaming_stall(label: &str) -> Result<Self, io::Error> {
        let cancellation = StreamingCancellation {
            closed: Arc::new(AtomicBool::new(false)),
            closed_notify: Arc::new(Notify::new()),
        };
        let mut upstream =
            Self::spawn(label, MockBehavior::StreamingStall(cancellation.clone())).await?;
        upstream.streaming_cancellation = Some(cancellation);
        Ok(upstream)
    }

    async fn spawn(label: &str, behavior: MockBehavior) -> Result<Self, io::Error> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let endpoint = MockEndpoint {
            label: label.to_owned(),
            host: format!("p3-09-{label}.test"),
            port,
        };
        let models = Arc::new(Mutex::new(Vec::new()));
        let requests = Arc::new(AtomicUsize::new(0));
        let task_models = Arc::clone(&models);
        let task_requests = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            loop {
                let accepted = listener.accept().await;
                let Ok((socket, _address)) = accepted else {
                    return;
                };
                let models = Arc::clone(&task_models);
                let requests = Arc::clone(&task_requests);
                let behavior = behavior.clone();
                let _connection = tokio::spawn(async move {
                    let _result = serve_mock_connection(socket, behavior, models, requests).await;
                });
            }
        });

        Ok(Self {
            endpoint,
            models,
            requests,
            task,
            streaming_cancellation: None,
        })
    }

    fn endpoint(&self) -> MockEndpoint {
        self.endpoint.clone()
    }

    fn request_count(&self) -> usize {
        self.requests.load(Ordering::Acquire)
    }

    async fn received_models(&self) -> Vec<String> {
        self.models.lock().await.clone()
    }

    async fn wait_for_stream_close(&self) -> bool {
        match &self.streaming_cancellation {
            Some(cancellation) => cancellation.wait_for_close().await,
            None => false,
        }
    }
}

impl Drop for MockUpstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve_mock_connection(
    mut socket: TcpStream,
    behavior: MockBehavior,
    models: Arc<Mutex<Vec<String>>>,
    requests: Arc<AtomicUsize>,
) -> Result<(), io::Error> {
    let model = read_request_model(&mut socket).await?;
    requests.fetch_add(1, Ordering::AcqRel);
    models.lock().await.push(model);

    match behavior {
        MockBehavior::JsonSuccess { text } => write_json_success(&mut socket, &text).await,
        MockBehavior::Status { code } => write_status(&mut socket, code).await,
        MockBehavior::StreamingStall(cancellation) => {
            write_stream_start_and_wait_for_close(&mut socket, cancellation).await
        }
    }
}

async fn read_request_model(socket: &mut TcpStream) -> Result<String, io::Error> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(end) = find_header_end(&bytes) {
            break end;
        }
        let mut buffer = [0_u8; 1024];
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "mock upstream peer closed before request headers",
            ));
        }
        if bytes.len().saturating_add(read) > MAX_HTTP_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "mock upstream request exceeds the bounded test limit",
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    };
    let header = std::str::from_utf8(&bytes[..header_end]).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "mock upstream request headers are not UTF-8",
        )
    })?;
    if !header.starts_with("POST /v1/responses ") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mock upstream received an unexpected request target",
        ));
    }
    let content_length = request_content_length(header)?;
    let total_length = header_end.checked_add(content_length).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "mock upstream request length overflowed",
        )
    })?;
    if total_length > MAX_HTTP_REQUEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mock upstream request body exceeds the bounded test limit",
        ));
    }
    while bytes.len() < total_length {
        let mut buffer = [0_u8; 1024];
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "mock upstream peer closed before request body",
            ));
        }
        if bytes.len().saturating_add(read) > MAX_HTTP_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "mock upstream request exceeds the bounded test limit",
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    let value: Value = serde_json::from_slice(&bytes[header_end..total_length]).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "mock upstream request body is not JSON",
        )
    })?;
    value
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "mock upstream request has no model",
            )
        })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

fn request_content_length(header: &str) -> Result<usize, io::Error> {
    header
        .lines()
        .skip(1)
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "mock upstream needs content-length",
            )
        })
}

async fn write_json_success(socket: &mut TcpStream, text: &str) -> Result<(), io::Error> {
    let body = serde_json::json!({
        "id": "upstream-response",
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "output_text", "text": text }]
        }],
        "usage": {
            "input_tokens": 2,
            "output_tokens": 3,
            "output_tokens_details": { "reasoning_tokens": 1 }
        }
    })
    .to_string();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(head.as_bytes()).await?;
    socket.write_all(body.as_bytes()).await?;
    socket.flush().await
}

async fn write_status(socket: &mut TcpStream, code: u16) -> Result<(), io::Error> {
    let head =
        format!("HTTP/1.1 {code} Mock Failure\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    socket.write_all(head.as_bytes()).await?;
    socket.flush().await
}

async fn write_stream_start_and_wait_for_close(
    socket: &mut TcpStream,
    cancellation: StreamingCancellation,
) -> Result<(), io::Error> {
    let body = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"upstream-stream-response\"}}\n\n"
    );
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
    socket.write_all(head.as_bytes()).await?;
    socket.write_all(body.as_bytes()).await?;
    socket.flush().await?;

    let mut buffer = [0_u8; 256];
    loop {
        match socket.read(&mut buffer).await {
            Ok(0) | Err(_) => {
                cancellation.closed.store(true, Ordering::Release);
                cancellation.closed_notify.notify_waiters();
                return Ok(());
            }
            Ok(_) => {}
        }
    }
}

fn loopback_endpoint(
    endpoint: &MockEndpoint,
    index: usize,
) -> Result<AggregationEndpoint, Box<dyn Error>> {
    let base_url = format!("http://{}:{}/v1", endpoint.host, endpoint.port);
    let policy = EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new(format!("p3-09-egress-{index}"))?,
        name: "P3-09 loopback test policy".to_owned(),
        allowed_schemes: std::collections::BTreeSet::from([EgressScheme::Http]),
        allowed_hosts: std::collections::BTreeSet::from([EgressHost::try_new(&endpoint.host)?]),
        allowed_ports: std::collections::BTreeSet::from([endpoint.port]),
        allowed_cidrs: std::collections::BTreeSet::from([EgressCidr::try_new(
            LOOPBACK_ADDRESS,
            32,
        )?]),
        redirect_policy: RedirectPolicy::Deny,
    })?;
    let timeouts = UpstreamTimeouts::try_new(
        Duration::from_millis(250),
        Duration::from_millis(250),
        Duration::from_millis(250),
        Duration::from_secs(1),
    )?;
    let maximum_idle_connections = std::num::NonZeroUsize::new(1).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "test value must be non-zero")
    })?;
    Ok(AggregationEndpoint::new(
        endpoint.label.clone(),
        OpenAiResponsesEndpoint::try_new(&base_url, "/responses")?,
        format!("minimax-m3-upstream-{}", endpoint.label),
        format!("p3-09-synthetic-credential-{index}").into_bytes(),
        policy,
        Arc::new(StaticLoopbackResolver),
        UpstreamTransportProfile::new(timeouts, UpstreamProxy::Direct, maximum_idle_connections),
    ))
}

fn authorized(request: test::TestRequest, presented_key: &str) -> test::TestRequest {
    request.insert_header((header::AUTHORIZATION, format!("Bearer {presented_key}")))
}

#[actix_web::test]
async fn round_robin_reaches_each_controlled_http_upstream() -> TestResult {
    let upstream_a = MockUpstream::json_success("a", "reply from A").await?;
    let upstream_b = MockUpstream::json_success("b", "reply from B").await?;
    let event_sink: Arc<dyn GatewayEventSink> = Arc::new(gateway_core::NoopGatewayEventSink);
    let harness = build_aggregation_harness(
        "p3-09",
        PUBLIC_MODEL,
        MODEL_ALIAS,
        RequestIdMode::Fixed,
        2,
        Duration::from_secs(1),
        vec![
            loopback_endpoint(&upstream_a.endpoint(), 0)?,
            loopback_endpoint(&upstream_b.endpoint(), 1)?,
        ],
        event_sink,
    )?;
    let presented_key = harness.presented_key().to_owned();
    let observed_routes = harness.observed_routes();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(harness.state()))
            .configure(configure),
    )
    .await;

    for _ in 0..12 {
        let request = authorized(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_payload(format!(r#"{{"model":"{MODEL_ALIAS}","input":"hello"}}"#)),
            &presented_key,
        )
        .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(body.contains(r#""model":"minimax-m3""#));
        assert!(!body.contains(MODEL_ALIAS));
        assert!(!body.contains("minimax-m3-upstream-"));
    }

    assert_eq!(upstream_a.request_count(), 6);
    assert_eq!(upstream_b.request_count(), 6);
    assert_eq!(
        upstream_a.received_models().await,
        vec!["minimax-m3-upstream-a".to_owned(); 6]
    );
    assert_eq!(
        upstream_b.received_models().await,
        vec!["minimax-m3-upstream-b".to_owned(); 6]
    );
    assert_eq!(
        observed_routes.lock().await.as_slice(),
        vec![ROUTE_ID.to_owned(); 12].as_slice()
    );
    Ok(())
}

#[actix_web::test]
async fn pre_semantic_http_5xx_fails_over_to_the_second_upstream() -> TestResult {
    let failing = MockUpstream::status("a", 503).await?;
    let healthy = MockUpstream::json_success("b", "fallback reply").await?;
    let (queue, mut receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(8, 1)?)?;
    let queue = Arc::new(queue);
    let event_sink: Arc<dyn GatewayEventSink> = queue.clone();
    let harness = build_aggregation_harness(
        "p3-09",
        PUBLIC_MODEL,
        MODEL_ALIAS,
        RequestIdMode::Fixed,
        2,
        Duration::from_secs(1),
        vec![
            loopback_endpoint(&failing.endpoint(), 0)?,
            loopback_endpoint(&healthy.endpoint(), 1)?,
        ],
        event_sink,
    )?;
    let presented_key = harness.presented_key().to_owned();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(harness.state()))
            .configure(configure),
    )
    .await;

    let request = authorized(
        test::TestRequest::post()
            .uri("/v1/responses")
            .set_payload(format!(
                r#"{{"model":"{MODEL_ALIAS}","input":"retry safely"}}"#
            )),
        &presented_key,
    )
    .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(response).await.to_vec())?;
    assert!(body.contains("fallback reply"));
    assert!(!body.contains("p3-09-synthetic-credential"));
    assert_eq!(failing.request_count(), 1);
    assert_eq!(healthy.request_count(), 1);

    let mut attempts = Vec::new();
    let mut saw_request = false;
    let mut saw_usage = false;
    while let Some(event) = receiver.try_recv() {
        match event {
            GatewayEvent::Request(event) => {
                saw_request = true;
                assert_eq!(event.request_id().as_str(), REQUEST_ID);
                assert_eq!(event.public_model(), PUBLIC_MODEL);
                assert_eq!(event.route_alias(), Some(MODEL_ALIAS));
            }
            GatewayEvent::Attempt(event) => {
                assert_eq!(event.request_id().as_str(), REQUEST_ID);
                attempts.push(event);
            }
            GatewayEvent::Usage(event) => {
                saw_usage = true;
                assert_eq!(event.request_id().as_str(), REQUEST_ID);
                assert_eq!(event.usage().input_tokens, Some(2));
                assert_eq!(event.usage().output_tokens, Some(3));
            }
            GatewayEvent::Diagnostic(_) => {}
        }
    }
    assert!(saw_request);
    assert!(saw_usage);
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].attempt_number(), 1);
    assert_eq!(
        attempts[0].route_candidate_id().as_str(),
        "p3-09-candidate-a"
    );
    assert!(matches!(
        attempts[0].outcome(),
        AttemptOutcome::Failed(error) if error.code() == GatewayErrorCode::ProviderTransient
    ));
    assert_eq!(
        attempts[0].retry_decision(),
        AttemptRetryDecision::RetryEligible
    );
    assert_eq!(attempts[1].attempt_number(), 2);
    assert_eq!(
        attempts[1].route_candidate_id().as_str(),
        "p3-09-candidate-b"
    );
    assert!(matches!(attempts[1].outcome(), AttemptOutcome::Succeeded));
    assert_eq!(
        attempts[1].retry_decision(),
        AttemptRetryDecision::Completed
    );
    Ok(())
}

#[actix_web::test]
async fn dropping_the_gateway_sse_body_closes_the_live_mock_upstream_attempt() -> TestResult {
    let streaming = MockUpstream::streaming_stall("a").await?;
    let unused_fallback = MockUpstream::json_success("b", "must not run").await?;
    let event_sink: Arc<dyn GatewayEventSink> = Arc::new(gateway_core::NoopGatewayEventSink);
    let harness = build_aggregation_harness(
        "p3-09",
        PUBLIC_MODEL,
        MODEL_ALIAS,
        RequestIdMode::Fixed,
        2,
        Duration::from_secs(1),
        vec![
            loopback_endpoint(&streaming.endpoint(), 0)?,
            loopback_endpoint(&unused_fallback.endpoint(), 1)?,
        ],
        event_sink,
    )?;
    let presented_key = harness.presented_key().to_owned();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(harness.state()))
            .configure(configure),
    )
    .await;

    let request = authorized(
        test::TestRequest::post()
            .uri("/v1/responses")
            .set_payload(format!(
                r#"{{"model":"{MODEL_ALIAS}","input":"cancel me","stream":true}}"#
            )),
        &presented_key,
    )
    .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    drop(response);

    assert!(streaming.wait_for_stream_close().await);
    assert_eq!(unused_fallback.request_count(), 0);
    Ok(())
}
