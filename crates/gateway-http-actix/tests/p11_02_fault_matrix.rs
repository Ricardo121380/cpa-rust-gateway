//! P11-02 deterministic, loopback-only transport and bounded-stream fault injection.
//!
//! This suite does not resolve public DNS, contact a provider, read proxy/environment state, or
//! handle a real credential. Its only socket peers are ephemeral loopback listeners.

#![deny(unsafe_code)]

use std::{
    collections::BTreeSet,
    error::Error,
    io,
    net::{IpAddr, Ipv4Addr},
    num::NonZeroUsize,
    time::Duration,
};

use gateway_core::{
    CanonicalEvent, EgressPolicyId, ErrorScope, GatewayError, GatewayErrorCode, MessageRole,
    MessageStart, RawExtensions, ResponseId, ResponseStart,
};
use gateway_stream::{StreamCapacity, bounded_canonical_stream};
use gateway_upstream::{
    AdmittedEgressTarget, EgressAdmissionErrorCode, EgressCidr, EgressDnsError, EgressDnsResolver,
    EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy, UpstreamClientPool,
    UpstreamHttpMethod, UpstreamHttpRequest, UpstreamProxy, UpstreamTimeouts,
    UpstreamTransportProfile,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time,
};

type TestResult = Result<(), Box<dyn Error>>;

const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

#[derive(Clone, Copy)]
struct LoopbackResolver;

impl EgressDnsResolver for LoopbackResolver {
    fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![LOOPBACK])
    }
}

#[derive(Clone, Copy)]
struct UnavailableResolver;

impl EgressDnsResolver for UnavailableResolver {
    fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Err(EgressDnsError)
    }
}

#[test]
fn dns_failure_stops_before_dial_with_the_egress_unavailable_classification() -> TestResult {
    let policy = policy(443, EgressScheme::Https)?;
    let error = policy
        .admit_url("https://relay.test:443/p11-02", &UnavailableResolver)
        .err()
        .ok_or("DNS fault unexpectedly admitted a target")?;
    assert_eq!(error.code(), EgressAdmissionErrorCode::DnsUnavailable);
    assert_gateway_error(
        &error.gateway_error(),
        GatewayErrorCode::EgressUnavailable,
        ErrorScope::Egress,
    );
    Ok(())
}

#[tokio::test]
async fn refused_network_and_plaintext_tls_handshake_are_one_egress_failure_each() -> TestResult {
    let closed_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let closed_port = closed_listener.local_addr()?.port();
    drop(closed_listener);

    let refused = pool()?
        .send(
            request(target(closed_port, EgressScheme::Http)?)?,
            &profile(Duration::from_millis(250))?,
        )
        .await
        .err()
        .ok_or("refused loopback connection unexpectedly succeeded")?;
    assert_gateway_error(
        &refused,
        GatewayErrorCode::EgressUnavailable,
        ErrorScope::Egress,
    );

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await?;
        let mut first_byte = [0_u8; 1];
        let _read = socket.read(&mut first_byte).await?;
        drop(socket);
        Ok::<(), io::Error>(())
    });

    let tls = pool()?
        .send(
            request(target(port, EgressScheme::Https)?)?,
            &profile(Duration::from_millis(350))?,
        )
        .await
        .err()
        .ok_or("plaintext listener unexpectedly completed a TLS handshake")?;
    assert_gateway_error(
        &tls,
        GatewayErrorCode::EgressUnavailable,
        ErrorScope::Egress,
    );
    server.await??;
    Ok(())
}

#[tokio::test]
async fn rate_and_server_status_faults_are_returned_once_without_transport_retry() -> TestResult {
    for status in [429_u16, 503_u16] {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            read_headers(&mut socket).await?;
            let head = format!(
                "HTTP/1.1 {status} fault\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            socket.write_all(head.as_bytes()).await?;
            socket.flush().await?;
            drop(socket);
            Ok::<bool, io::Error>(
                time::timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err(),
            )
        });

        let response = pool()?
            .send(
                request(target(port, EgressScheme::Http)?)?,
                &profile(Duration::from_millis(350))?,
            )
            .await?;
        assert_eq!(response.status(), status);
        assert!(server.await??, "status fault dispatched more than once");
    }
    Ok(())
}

#[tokio::test]
async fn truncated_stream_slow_client_and_cancellation_preserve_their_own_boundaries() -> TestResult
{
    let (sender, mut truncated_stream) = bounded_canonical_stream(StreamCapacity::try_new(1)?);
    let mut sender = sender;
    let start = response_start()?;
    sender.send(start.clone()).await?;
    drop(sender);
    assert_eq!(truncated_stream.recv().await?, Some(start));
    let truncated = truncated_stream
        .recv()
        .await
        .err()
        .ok_or("incomplete stream unexpectedly ended cleanly")?;
    assert_gateway_error(
        &truncated,
        GatewayErrorCode::StreamTruncated,
        ErrorScope::Stream,
    );
    assert_eq!(truncated_stream.recv().await?, None);

    let (mut slow_sender, mut slow_stream) = bounded_canonical_stream(StreamCapacity::try_new(1)?);
    let slow_start = response_start()?;
    let message = message_start();
    slow_sender.send(slow_start.clone()).await?;
    let mut blocked = Box::pin(slow_sender.send(message.clone()));
    assert!(
        time::timeout(Duration::from_millis(20), &mut blocked)
            .await
            .is_err(),
        "slow downstream consumer did not apply bounded backpressure"
    );
    assert_eq!(slow_stream.recv().await?, Some(slow_start));
    blocked.await?;
    assert_eq!(slow_stream.recv().await?, Some(message));

    let (mut cancelled_sender, cancelled_stream) =
        bounded_canonical_stream(StreamCapacity::try_new(1)?);
    let control = cancelled_stream.control();
    cancelled_sender.send(response_start()?).await?;
    let mut blocked = Box::pin(cancelled_sender.send(message_start()));
    assert!(
        time::timeout(Duration::from_millis(20), &mut blocked)
            .await
            .is_err(),
        "producer was not waiting for bounded capacity before cancellation"
    );
    control.cancel();
    let cancelled = blocked
        .await
        .err()
        .ok_or("cancelled producer unexpectedly sent a second event")?;
    assert_gateway_error(&cancelled, GatewayErrorCode::Cancelled, ErrorScope::Request);
    assert!(!control.allows_transparent_retry());
    Ok(())
}

fn policy(port: u16, scheme: EgressScheme) -> Result<EgressPolicy, Box<dyn Error>> {
    Ok(EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("p11-02-fault-policy")?,
        name: "P11-02 loopback fault policy".to_owned(),
        allowed_schemes: BTreeSet::from([scheme]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new("relay.test")?]),
        allowed_ports: BTreeSet::from([port]),
        allowed_cidrs: BTreeSet::from([EgressCidr::try_new(LOOPBACK, 32)?]),
        redirect_policy: RedirectPolicy::Deny,
    })?)
}

fn target(port: u16, scheme: EgressScheme) -> Result<AdmittedEgressTarget, Box<dyn Error>> {
    let policy = policy(port, scheme)?;
    Ok(policy.admit_url(
        &format!("{}://relay.test:{port}/p11-02", scheme.as_str()),
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

fn pool() -> Result<UpstreamClientPool, Box<dyn Error>> {
    let capacity = NonZeroUsize::new(4).ok_or("test pool capacity must be non-zero")?;
    Ok(UpstreamClientPool::new(capacity))
}

fn profile(total: Duration) -> Result<UpstreamTransportProfile, Box<dyn Error>> {
    let idle = total.min(Duration::from_millis(150));
    let timeouts = UpstreamTimeouts::try_new(idle, total, idle, total)?;
    let capacity = NonZeroUsize::new(2).ok_or("test idle capacity must be non-zero")?;
    Ok(UpstreamTransportProfile::new(
        timeouts,
        UpstreamProxy::Direct,
        capacity,
    ))
}

fn response_start() -> Result<CanonicalEvent, Box<dyn Error>> {
    Ok(CanonicalEvent::ResponseStart(ResponseStart {
        response_id: ResponseId::try_new("p11-02-fault-response")?,
        extensions: RawExtensions::default(),
    }))
}

fn message_start() -> CanonicalEvent {
    CanonicalEvent::MessageStart(MessageStart {
        role: MessageRole("assistant".to_owned()),
        extensions: RawExtensions::default(),
    })
}

async fn read_headers(socket: &mut TcpStream) -> io::Result<()> {
    const MAX_HEADER_BYTES: usize = 16 * 1024;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 512];
    loop {
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "loopback peer closed before headers",
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(());
        }
        if bytes.len() > MAX_HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "loopback request exceeded header bound",
            ));
        }
    }
}

fn assert_gateway_error(error: &GatewayError, code: GatewayErrorCode, scope: ErrorScope) {
    assert_eq!(error.code(), code);
    assert_eq!(error.scope(), scope);
}
