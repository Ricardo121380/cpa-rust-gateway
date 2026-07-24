//! P11-04 deterministic, loopback-only concurrency, long-stream, and connection-reuse checks.
//!
//! The soak runner added by this task reuses these exact primitives. No public DNS, proxy,
//! environment Credential, Provider account, or configured upstream is admitted here.

#![deny(unsafe_code)]

use std::{
    collections::BTreeSet,
    env,
    error::Error,
    fs::{File, OpenOptions},
    io::{self, Write},
    net::{IpAddr, Ipv4Addr},
    num::NonZeroUsize,
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use gateway_core::{
    CanonicalEvent, EgressPolicyId, GatewayErrorCode, MessageEnd, MessageRole, MessageStart,
    RawExtensions, ResponseEnd, ResponseId, ResponseStart, TextDelta,
};
use gateway_stream::{StreamCapacity, bounded_canonical_stream};
use gateway_upstream::{
    AdmittedEgressTarget, EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy,
    EgressPolicyInput, EgressScheme, RedirectPolicy, UpstreamClientPool, UpstreamHttpMethod,
    UpstreamHttpRequest, UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
    time,
};

type TestResult = Result<(), Box<dyn Error>>;

const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const CONCURRENT_STREAMS: usize = 12;
const TEXT_DELTAS_PER_STREAM: usize = 128;
const POOLED_REQUESTS: usize = 24;
const MAX_HTTP_REQUEST_BYTES: usize = 16 * 1024;
const SMOKE_SOAK_SECONDS: u64 = 10;
const FULL_SOAK_SECONDS: u64 = 24 * 60 * 60;
const SOAK_AUTHORIZATION: &str = "P11-04-LOOPBACK-SOAK-v1";
const SOAK_STATUS_INTERVAL: Duration = Duration::from_mins(5);
const SOAK_PAUSE: Duration = Duration::from_secs(2);
const SOAK_CONCURRENCY: usize = 4;
const RSS_WARM_UP_SAMPLES: usize = 2;
const RSS_GROWTH_WINDOW_SAMPLES: usize = 6;

#[derive(Clone, Copy)]
struct LoopbackResolver;

impl EgressDnsResolver for LoopbackResolver {
    fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![LOOPBACK])
    }
}

#[tokio::test]
async fn concurrent_long_streams_complete_with_a_slow_consumer_and_no_join_leak() -> TestResult {
    let mut streams = JoinSet::new();
    for sequence in 0..CONCURRENT_STREAMS {
        streams.spawn(async move {
            run_long_stream(sequence, sequence == 0)
                .await
                .map_err(|_| ())
        });
    }

    let delivered_events = join_all_long_streams(&mut streams).await?;
    let mut completed = 0_usize;
    for delivered in delivered_events {
        assert_eq!(delivered, TEXT_DELTAS_PER_STREAM.saturating_add(4));
        completed = completed.saturating_add(1);
    }
    assert_eq!(completed, CONCURRENT_STREAMS);
    Ok(())
}

#[tokio::test]
async fn cancellation_joins_all_blocked_concurrent_workloads() -> TestResult {
    let mut streams = JoinSet::new();
    let mut controls = Vec::new();
    for sequence in 0..CONCURRENT_STREAMS {
        let (mut sender, stream) = bounded_canonical_stream(StreamCapacity::try_new(1)?);
        sender.send(response_start(sequence)?).await?;
        controls.push(stream.control());
        streams.spawn(async move { sender.send(message_start()).await });
    }

    tokio::task::yield_now().await;
    for control in &controls {
        control.cancel();
    }

    let joined = time::timeout(Duration::from_secs(1), async {
        let mut joined = 0_usize;
        while let Some(result) = streams.join_next().await {
            let Err(error) = result? else {
                return Err(io::Error::other(
                    "a workload blocked on bounded capacity completed after cancellation",
                )
                .into());
            };
            assert_eq!(error.code(), GatewayErrorCode::Cancelled);
            joined = joined.saturating_add(1);
        }
        Ok::<usize, Box<dyn Error>>(joined)
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "cancelled workloads did not join"))??;
    assert_eq!(joined, CONCURRENT_STREAMS);
    Ok(())
}

#[test]
fn sustained_rss_growth_checks_each_post_warm_up_window() {
    assert!(sustained_rss_growth(&[
        100, 100, 100, 104, 108, 112, 116, 120, 119, 118,
    ]));
    assert!(!sustained_rss_growth(&[
        100, 110, 100, 102, 104, 106, 108, 110,
    ]));
    assert!(!sustained_rss_growth(&[
        100, 100, 100, 104, 103, 110, 115, 120,
    ]));
}

#[tokio::test]
async fn loopback_keep_alive_pool_reuses_connections_after_warm_up() -> TestResult {
    let peer = KeepAlivePeer::spawn().await?;
    let pool = UpstreamClientPool::new(non_zero(2)?);
    let profile = profile()?;
    let target = target(peer.port())?;

    for _ in 0..POOLED_REQUESTS {
        let mut response = pool.send(request(target.clone())?, &profile).await?;
        assert_eq!(response.status(), 200);
        while response.next_chunk().await?.is_some() {}
    }

    peer.wait_for_requests(POOLED_REQUESTS).await?;
    let connection_count = peer.connection_count();
    assert!(
        connection_count < POOLED_REQUESTS,
        "one loopback connection per request would not be a warmed pool"
    );
    assert_eq!(peer.request_count(), POOLED_REQUESTS);
    Ok(())
}

async fn run_long_stream(sequence: usize, slow_consumer: bool) -> Result<usize, Box<dyn Error>> {
    let (mut sender, mut stream) = bounded_canonical_stream(StreamCapacity::try_new(2)?);
    let start = response_start(sequence)?;
    let producer = tokio::spawn(async move {
        sender.send(start).await?;
        sender.send(message_start()).await?;
        for _ in 0..TEXT_DELTAS_PER_STREAM {
            sender.send(text_delta()).await?;
        }
        sender
            .send(CanonicalEvent::MessageEnd(MessageEnd::default()))
            .await?;
        sender
            .send(CanonicalEvent::ResponseEnd(ResponseEnd::default()))
            .await?;
        Ok::<(), gateway_core::GatewayError>(())
    });

    let mut delivered = 0_usize;
    while let Some(event) = stream.recv().await? {
        if slow_consumer && delivered.is_multiple_of(16) {
            time::sleep(Duration::from_millis(1)).await;
        }
        let _ = stream
            .control()
            .first_semantic_event_tracker()
            .mark_delivered(&event);
        delivered = delivered.saturating_add(1);
    }
    producer.await??;
    Ok(delivered)
}

#[derive(Debug)]
struct SoakConfig {
    duration: Duration,
    status_path: PathBuf,
}

#[tokio::test]
#[ignore = "P11-04 runs only through scripts/run-p11-04-soak.sh"]
async fn authorized_loopback_soak_writes_a_value_free_receipt() -> TestResult {
    let config = soak_config()?;
    let mut status = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&config.status_path)?;
    let started = Instant::now();
    let mut batches = 0_usize;
    let mut streams = 0_usize;
    let mut rss_samples = Vec::new();
    let mut next_status = Duration::ZERO;

    loop {
        let elapsed = started.elapsed();
        if elapsed >= config.duration {
            break;
        }
        streams =
            streams.saturating_add(run_soak_batch(batches.saturating_mul(SOAK_CONCURRENCY)).await?);
        batches = batches.saturating_add(1);
        if elapsed >= next_status {
            let rss_bytes = process_rss_bytes()?;
            rss_samples.push(rss_bytes);
            write_soak_status(&mut status, "RUNNING", elapsed, batches, streams, rss_bytes)?;
            next_status = elapsed.saturating_add(SOAK_STATUS_INTERVAL);
        }
        time::sleep(SOAK_PAUSE).await;
    }

    let elapsed = started.elapsed();
    let final_rss_bytes = process_rss_bytes()?;
    rss_samples.push(final_rss_bytes);
    if config.duration.as_secs() == FULL_SOAK_SECONDS && sustained_rss_growth(&rss_samples) {
        write_soak_status(
            &mut status,
            "FAILED_RSS_GROWTH",
            elapsed,
            batches,
            streams,
            final_rss_bytes,
        )?;
        return Err(io::Error::other("loopback soak RSS grew monotonically after warm-up").into());
    }
    write_soak_status(
        &mut status,
        "COMPLETED",
        elapsed,
        batches,
        streams,
        final_rss_bytes,
    )?;
    Ok(())
}

async fn run_soak_batch(sequence_base: usize) -> Result<usize, Box<dyn Error>> {
    let mut streams = JoinSet::new();
    for offset in 0..SOAK_CONCURRENCY {
        streams.spawn(async move {
            run_long_stream(sequence_base.saturating_add(offset), offset == 0)
                .await
                .map_err(|_| ())
        });
    }

    let delivered_events = join_all_long_streams(&mut streams).await?;
    let mut completed = 0_usize;
    for delivered in delivered_events {
        if delivered != TEXT_DELTAS_PER_STREAM.saturating_add(4) {
            return Err(
                io::Error::other("soak long stream did not deliver its full lifecycle").into(),
            );
        }
        completed = completed.saturating_add(1);
    }
    Ok(completed)
}

async fn join_all_long_streams(
    streams: &mut JoinSet<Result<usize, ()>>,
) -> Result<Vec<usize>, io::Error> {
    let mut delivered_events = Vec::new();
    let mut first_failure = None;
    while let Some(result) = streams.join_next().await {
        match result {
            Ok(Ok(delivered)) => delivered_events.push(delivered),
            Ok(Err(())) => {
                first_failure.get_or_insert("long stream failed without a value");
            }
            Err(_) => {
                first_failure.get_or_insert("long stream task did not join cleanly");
            }
        }
    }
    if let Some(message) = first_failure {
        return Err(io::Error::other(message));
    }
    Ok(delivered_events)
}

fn soak_config() -> Result<SoakConfig, io::Error> {
    let authorization = env::var("P11_04_SOAK_AUTH").map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "missing loopback soak authorization",
        )
    })?;
    if authorization != SOAK_AUTHORIZATION {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "invalid loopback soak authorization",
        ));
    }
    let seconds = env::var("P11_04_SOAK_SECONDS")
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "missing soak duration"))?
        .parse::<u64>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid soak duration"))?;
    if seconds != SMOKE_SOAK_SECONDS && seconds != FULL_SOAK_SECONDS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "soak duration is not an approved smoke or 24-hour value",
        ));
    }
    let status_path =
        PathBuf::from(env::var("P11_04_STATUS_PATH").map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "missing soak status path")
        })?);
    if !status_path.is_absolute() || status_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "soak status path must be a new absolute receipt path",
        ));
    }
    Ok(SoakConfig {
        duration: Duration::from_secs(seconds),
        status_path,
    })
}

fn process_rss_bytes() -> Result<u64, io::Error> {
    let process_id = std::process::id().to_string();
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &process_id])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("could not sample the loopback soak RSS"));
    }
    let output = String::from_utf8(output.stdout)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid RSS sample output"))?;
    let kibibytes = output
        .split_whitespace()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty RSS sample output"))?
        .parse::<u64>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid RSS sample value"))?;
    kibibytes
        .checked_mul(1024)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "RSS sample overflow"))
}

fn sustained_rss_growth(samples: &[u64]) -> bool {
    if samples.len() < RSS_WARM_UP_SAMPLES.saturating_add(RSS_GROWTH_WINDOW_SAMPLES) {
        return false;
    }
    samples[RSS_WARM_UP_SAMPLES..]
        .windows(RSS_GROWTH_WINDOW_SAMPLES)
        .any(|window| {
            let monotonic = window.windows(2).all(|pair| pair[1] >= pair[0]);
            let minimum = window[0];
            let maximum = window[RSS_GROWTH_WINDOW_SAMPLES.saturating_sub(1)];
            monotonic && maximum > minimum.saturating_mul(115) / 100
        })
}

fn write_soak_status(
    status: &mut File,
    state: &str,
    elapsed: Duration,
    batches: usize,
    streams: usize,
    rss_bytes: u64,
) -> Result<(), io::Error> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| io::Error::other("clock before Unix epoch"))?
        .as_secs();
    writeln!(
        status,
        "timestamp_unix={timestamp} state={state} elapsed_seconds={} batches={batches} streams={streams} rss_bytes={rss_bytes}",
        elapsed.as_secs()
    )?;
    status.flush()
}

struct KeepAlivePeer {
    port: u16,
    connections: Arc<AtomicUsize>,
    requests: Arc<AtomicUsize>,
    accept_task: JoinHandle<()>,
}

impl KeepAlivePeer {
    async fn spawn() -> Result<Self, io::Error> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let connections = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(AtomicUsize::new(0));
        let task_connections = Arc::clone(&connections);
        let task_requests = Arc::clone(&requests);
        let accept_task = tokio::spawn(async move {
            loop {
                let accepted = listener.accept().await;
                let Ok((socket, _address)) = accepted else {
                    return;
                };
                task_connections.fetch_add(1, Ordering::AcqRel);
                let requests = Arc::clone(&task_requests);
                let _connection = tokio::spawn(async move {
                    let _result = serve_keep_alive_connection(socket, requests).await;
                });
            }
        });
        Ok(Self {
            port,
            connections,
            requests,
            accept_task,
        })
    }

    const fn port(&self) -> u16 {
        self.port
    }

    fn connection_count(&self) -> usize {
        self.connections.load(Ordering::Acquire)
    }

    fn request_count(&self) -> usize {
        self.requests.load(Ordering::Acquire)
    }

    async fn wait_for_requests(&self, expected: usize) -> Result<(), io::Error> {
        time::timeout(Duration::from_secs(1), async {
            loop {
                if self.request_count() == expected {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "loopback requests did not finish"))
    }
}

impl Drop for KeepAlivePeer {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

async fn serve_keep_alive_connection(
    mut socket: TcpStream,
    requests: Arc<AtomicUsize>,
) -> Result<(), io::Error> {
    let mut buffer = Vec::new();
    while read_request(&mut socket, &mut buffer).await? {
        requests.fetch_add(1, Ordering::AcqRel);
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\n{}",
            )
            .await?;
        socket.flush().await?;
    }
    Ok(())
}

async fn read_request(socket: &mut TcpStream, buffer: &mut Vec<u8>) -> Result<bool, io::Error> {
    loop {
        if let Some(header_end) = find_header_end(buffer) {
            let content_length = content_length(&buffer[..header_end])?;
            let request_end = header_end.saturating_add(content_length);
            if buffer.len() >= request_end {
                buffer.drain(..request_end);
                return Ok(true);
            }
        }
        let mut chunk = [0_u8; 1024];
        let read = socket.read(&mut chunk).await?;
        if read == 0 {
            return Ok(false);
        }
        if buffer.len().saturating_add(read) > MAX_HTTP_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "loopback request exceeded bounded test input",
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position.saturating_add(4))
}

fn content_length(headers: &[u8]) -> Result<usize, io::Error> {
    let headers = std::str::from_utf8(headers)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid loopback headers"))?;
    headers
        .lines()
        .find_map(|line| {
            line.strip_prefix("content-length:")
                .or_else(|| line.strip_prefix("Content-Length:"))
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "missing loopback content length",
            )
        })?
        .trim()
        .parse::<usize>()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid loopback content length",
            )
        })
}

fn response_start(sequence: usize) -> Result<CanonicalEvent, Box<dyn Error>> {
    Ok(CanonicalEvent::ResponseStart(ResponseStart {
        response_id: ResponseId::try_new(format!("p11-04-response-{sequence}"))?,
        extensions: RawExtensions::default(),
    }))
}

fn message_start() -> CanonicalEvent {
    CanonicalEvent::MessageStart(MessageStart {
        role: MessageRole("assistant".to_owned()),
        extensions: RawExtensions::default(),
    })
}

fn text_delta() -> CanonicalEvent {
    CanonicalEvent::TextDelta(TextDelta {
        text: "p11-04-long-stream".to_owned(),
        extensions: RawExtensions::default(),
    })
}

fn target(port: u16) -> Result<AdmittedEgressTarget, Box<dyn Error>> {
    let policy = EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("p11-04-loopback-policy")?,
        name: "P11-04 loopback test policy".to_owned(),
        allowed_schemes: BTreeSet::from([EgressScheme::Http]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new("p11-04-loopback.test")?]),
        allowed_ports: BTreeSet::from([port]),
        allowed_cidrs: BTreeSet::from([EgressCidr::try_new(LOOPBACK, 32)?]),
        redirect_policy: RedirectPolicy::Deny,
    })?;
    Ok(policy.admit_url(
        &format!("http://p11-04-loopback.test:{port}/responses"),
        &LoopbackResolver,
    )?)
}

fn request(target: AdmittedEgressTarget) -> Result<UpstreamHttpRequest, Box<dyn Error>> {
    Ok(UpstreamHttpRequest::try_new(
        target,
        UpstreamHttpMethod::Post,
        [("content-type".to_owned(), "application/json".to_owned())],
        b"{}".to_vec(),
    )?)
}

fn profile() -> Result<UpstreamTransportProfile, Box<dyn Error>> {
    let total = Duration::from_secs(1);
    let timeouts = UpstreamTimeouts::try_new(
        Duration::from_millis(250),
        Duration::from_millis(250),
        Duration::from_millis(250),
        total,
    )?;
    Ok(UpstreamTransportProfile::new(
        timeouts,
        UpstreamProxy::Direct,
        non_zero(1)?,
    ))
}

fn non_zero(value: usize) -> Result<NonZeroUsize, Box<dyn Error>> {
    NonZeroUsize::new(value).ok_or_else(|| "test value must be non-zero".into())
}
