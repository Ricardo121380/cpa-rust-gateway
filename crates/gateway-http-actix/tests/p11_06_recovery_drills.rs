//! P11-06 loopback-only graceful-shutdown recovery drill.
//!
//! This test deliberately proves the HTTP lifecycle boundary only. `SQLite` recovery and queue
//! degradation are verified by the named `gateway-store` and library regressions in the P11-06
//! Recovery Report. No provider, configuration, environment credential, or non-loopback socket is
//! used here.

#![deny(unsafe_code)]

use std::{
    collections::VecDeque,
    error::Error,
    io,
    net::{Ipv4Addr, TcpListener},
    sync::Arc,
    time::Duration,
};

use actix_web::{App, HttpServer, dev::ServerHandle, web};
use gateway_auth::{InMemoryClientKey, InMemoryClientKeyAuthenticator};
use gateway_core::{
    CanonicalEvent, CanonicalRequest, ClientKeyId, ErrorScope, GatewayError, GatewayErrorCode,
    MessageEnd, MessageRole, MessageStart, RawExtensions, ResponseEnd, ResponseId, ResponseStart,
    TextDelta,
};
use gateway_http_actix::{ResponsesHttpState, configure};
use gateway_router::{ResponsesEventSource, ResponsesExecutor, ResponsesFuture};
use gateway_stream::StreamCapacity;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Notify,
    task::JoinHandle,
    time,
};

type TestResult = Result<(), Box<dyn Error>>;

const LOCAL_CLIENT_KEY: &str = "p11-06-loopback-client-key";

#[derive(Clone)]
struct GatedExecutor {
    stream_blocked: Arc<Notify>,
    release_stream: Arc<Notify>,
}

impl GatedExecutor {
    fn new() -> Self {
        Self {
            stream_blocked: Arc::new(Notify::new()),
            release_stream: Arc::new(Notify::new()),
        }
    }

    async fn wait_until_stream_is_blocked(&self) -> Result<(), io::Error> {
        time::timeout(Duration::from_secs(1), self.stream_blocked.notified())
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "stream did not reach gate"))
    }

    fn release(&self) {
        self.release_stream.notify_one();
    }
}

impl ResponsesExecutor for GatedExecutor {
    fn execute(
        &self,
        _context: gateway_core::RequestContext,
        _request: CanonicalRequest,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        let events = response_events();
        let stream_blocked = Arc::clone(&self.stream_blocked);
        let release_stream = Arc::clone(&self.release_stream);
        Box::pin(async move {
            Ok(Box::new(GatedEventSource {
                events: events?.into(),
                stream_blocked,
                release_stream,
                first_event_delivered: false,
                gate_released: false,
            }) as Box<dyn ResponsesEventSource>)
        })
    }
}

struct GatedEventSource {
    events: VecDeque<CanonicalEvent>,
    stream_blocked: Arc<Notify>,
    release_stream: Arc<Notify>,
    first_event_delivered: bool,
    gate_released: bool,
}

impl ResponsesEventSource for GatedEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move {
            if !self.first_event_delivered {
                self.first_event_delivered = true;
                return Ok(self.events.pop_front());
            }
            if !self.gate_released {
                self.stream_blocked.notify_one();
                self.release_stream.notified().await;
                self.gate_released = true;
            }
            Ok(self.events.pop_front())
        })
    }
}

struct LoopbackGateway {
    port: u16,
    handle: ServerHandle,
    task: JoinHandle<io::Result<()>>,
}

impl LoopbackGateway {
    async fn start(executor: GatedExecutor) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let client_key = InMemoryClientKey::try_new(
            LOCAL_CLIENT_KEY,
            ClientKeyId::try_new("p11-06-loopback-client")
                .map_err(|_| io::Error::other("invalid test Client Key ID"))?,
            true,
        )
        .map_err(|_| io::Error::other("invalid local Client Key"))?;
        let authenticator = InMemoryClientKeyAuthenticator::try_new([client_key])
            .map_err(|_| io::Error::other("invalid local Client Key configuration"))?;
        let state = ResponsesHttpState::new(
            Arc::new(executor),
            Arc::new(authenticator),
            StreamCapacity::try_new(4)
                .map_err(|_| io::Error::other("invalid local stream capacity"))?,
        );
        let server_state = state.clone();
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(server_state.clone()))
                .configure(configure)
        })
        .workers(1)
        .listen(listener)?
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::task::yield_now().await;
        Ok(Self { port, handle, task })
    }
}

#[tokio::test]
async fn graceful_stop_drains_an_already_started_stream_before_joining() -> TestResult {
    let executor = GatedExecutor::new();
    let gateway = LoopbackGateway::start(executor.clone()).await?;
    let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, gateway.port)).await?;
    let request_body = r#"{"model":"p11-06-local","input":"drain","stream":true}"#;
    client
        .write_all(
            format!(
                "POST /v1/responses HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {LOCAL_CLIENT_KEY}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{request_body}",
                request_body.len()
            )
            .as_bytes(),
        )
        .await?;
    client.flush().await?;

    executor.wait_until_stream_is_blocked().await?;
    let stop_handle = gateway.handle.clone();
    let stop = tokio::spawn(async move { stop_handle.stop(true).await });
    time::sleep(Duration::from_millis(25)).await;
    assert!(
        !gateway.task.is_finished(),
        "graceful shutdown joined while its active stream was still gated"
    );

    executor.release();
    let mut response = Vec::new();
    time::timeout(Duration::from_secs(1), client.read_to_end(&mut response))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "drained client did not close"))??;
    let response = String::from_utf8(response)?;
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("response.completed"));

    time::timeout(Duration::from_secs(1), stop)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "stop signal did not finish"))??;
    time::timeout(Duration::from_secs(1), gateway.task)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "server did not join"))???;
    Ok(())
}

fn response_events() -> Result<Vec<CanonicalEvent>, GatewayError> {
    Ok(vec![
        CanonicalEvent::ResponseStart(ResponseStart {
            response_id: ResponseId::try_new("p11-06-drained-response")
                .map_err(|_| protocol_error())?,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageStart(MessageStart {
            role: MessageRole("assistant".to_owned()),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::TextDelta(TextDelta {
            text: "P11-06_DRAINED".to_owned(),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageEnd(MessageEnd::default()),
        CanonicalEvent::ResponseEnd(ResponseEnd {
            stop_reason: Some("end_turn".to_owned()),
            stop_sequence: None,
            extensions: RawExtensions::default(),
        }),
    ])
}

fn protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}
