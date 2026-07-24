//! Bounded, DNS-pinned HTTP transport for admitted upstream requests.
//!
//! The pool accepts only [`crate::AdmittedEgressTarget`] values, so P2-09 remains the sole owner
//! of URL, DNS, CIDR, and address admission. It disables system proxies, automatic redirects, and
//! client-level retries; P3-06 will own redirect re-admission and attempt policy. Direct transport
//! and local-DNS SOCKS5 are kept in distinct cache entries. HTTP(S) and remote-DNS proxy schemes
//! are rejected because they cannot prove that the proxy dials one of the addresses admitted for
//! this attempt.

use std::{
    error::Error,
    fmt,
    hash::Hash,
    net::{IpAddr, SocketAddr},
    num::NonZeroUsize,
    sync::Arc,
    time::Duration,
};

use bytes::Bytes;
use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode};
use moka::sync::Cache;
use reqwest::{
    Client, Method, Proxy,
    header::{HeaderMap, HeaderName, HeaderValue},
    redirect::Policy as RedirectPolicy,
    retry,
};
use tokio::time::{self, Instant};
use url::Url;

use crate::{AdmittedEgressTarget, EgressHost};

/// The four independently bounded timeouts for one upstream request.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UpstreamTimeouts {
    connect: Duration,
    ttfb: Duration,
    idle: Duration,
    total: Duration,
}

impl UpstreamTimeouts {
    /// Validates connect, time-to-first-byte, response-idle, and total timeouts.
    ///
    /// Every timeout must be positive and no individual stage may exceed the total deadline.
    ///
    /// # Errors
    ///
    /// Returns [`UpstreamTimeoutError`] for an unsafe timeout shape.
    pub fn try_new(
        connect: Duration,
        ttfb: Duration,
        idle: Duration,
        total: Duration,
    ) -> Result<Self, UpstreamTimeoutError> {
        if connect.is_zero() {
            return Err(UpstreamTimeoutError::new(
                UpstreamTimeoutErrorCode::ZeroConnect,
            ));
        }
        if ttfb.is_zero() {
            return Err(UpstreamTimeoutError::new(
                UpstreamTimeoutErrorCode::ZeroTtfb,
            ));
        }
        if idle.is_zero() {
            return Err(UpstreamTimeoutError::new(
                UpstreamTimeoutErrorCode::ZeroIdle,
            ));
        }
        if total.is_zero() {
            return Err(UpstreamTimeoutError::new(
                UpstreamTimeoutErrorCode::ZeroTotal,
            ));
        }
        if connect > total {
            return Err(UpstreamTimeoutError::new(
                UpstreamTimeoutErrorCode::ConnectExceedsTotal,
            ));
        }
        if ttfb > total {
            return Err(UpstreamTimeoutError::new(
                UpstreamTimeoutErrorCode::TtfbExceedsTotal,
            ));
        }
        if idle > total {
            return Err(UpstreamTimeoutError::new(
                UpstreamTimeoutErrorCode::IdleExceedsTotal,
            ));
        }

        Ok(Self {
            connect,
            ttfb,
            idle,
            total,
        })
    }

    /// Returns the TCP/TLS connection deadline.
    #[must_use]
    pub const fn connect(&self) -> Duration {
        self.connect
    }

    /// Returns the response-header deadline, measured from request start.
    #[must_use]
    pub const fn ttfb(&self) -> Duration {
        self.ttfb
    }

    /// Returns the maximum quiet period between response-body reads.
    #[must_use]
    pub const fn idle(&self) -> Duration {
        self.idle
    }

    /// Returns the full request-and-body deadline.
    #[must_use]
    pub const fn total(&self) -> Duration {
        self.total
    }
}

/// Stable timeout-configuration failure categories without raw configuration values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpstreamTimeoutErrorCode {
    /// Connect timeout was zero.
    ZeroConnect,
    /// Time-to-first-byte timeout was zero.
    ZeroTtfb,
    /// Response idle timeout was zero.
    ZeroIdle,
    /// Total timeout was zero.
    ZeroTotal,
    /// Connect timeout was larger than the total deadline.
    ConnectExceedsTotal,
    /// Time-to-first-byte timeout was larger than the total deadline.
    TtfbExceedsTotal,
    /// Response idle timeout was larger than the total deadline.
    IdleExceedsTotal,
}

/// A safe upstream timeout-configuration error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpstreamTimeoutError {
    code: UpstreamTimeoutErrorCode,
}

impl UpstreamTimeoutError {
    const fn new(code: UpstreamTimeoutErrorCode) -> Self {
        Self { code }
    }

    /// Returns the stable timeout failure category.
    #[must_use]
    pub const fn code(self) -> UpstreamTimeoutErrorCode {
        self.code
    }
}

impl fmt::Display for UpstreamTimeoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            UpstreamTimeoutErrorCode::ZeroConnect => "upstream connect timeout must be positive",
            UpstreamTimeoutErrorCode::ZeroTtfb => "upstream TTFB timeout must be positive",
            UpstreamTimeoutErrorCode::ZeroIdle => "upstream idle timeout must be positive",
            UpstreamTimeoutErrorCode::ZeroTotal => "upstream total timeout must be positive",
            UpstreamTimeoutErrorCode::ConnectExceedsTotal => {
                "upstream connect timeout exceeds the total deadline"
            }
            UpstreamTimeoutErrorCode::TtfbExceedsTotal => {
                "upstream TTFB timeout exceeds the total deadline"
            }
            UpstreamTimeoutErrorCode::IdleExceedsTotal => {
                "upstream idle timeout exceeds the total deadline"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for UpstreamTimeoutError {}

/// A proxy mode that preserves the P2-09 admitted address set.
#[derive(Clone, Eq, Hash, PartialEq)]
pub enum UpstreamProxy {
    /// Dial the admitted target directly, without environment or system proxy discovery.
    Direct,
    /// Use a SOCKS5 proxy while resolving the admitted target locally to its pinned address.
    Socks5(Socks5ProxyEndpoint),
}

impl UpstreamProxy {
    /// Parses one SOCKS5 proxy endpoint that is safe for the DNS-pinned transport path.
    ///
    /// The endpoint carries no user-info, query, fragment, or non-root path. `socks5h`, HTTP, and
    /// HTTPS are deliberately rejected because their proxy can resolve the upstream Host itself.
    ///
    /// # Errors
    ///
    /// Returns [`UpstreamProxyError`] without retaining the supplied proxy text.
    pub fn try_socks5(value: &str) -> Result<Self, UpstreamProxyError> {
        let parsed = Url::parse(value)
            .map_err(|_| UpstreamProxyError::new(UpstreamProxyErrorCode::InvalidUrl))?;
        if parsed.scheme() != "socks5" {
            return Err(UpstreamProxyError::new(
                UpstreamProxyErrorCode::UnsupportedScheme,
            ));
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(UpstreamProxyError::new(
                UpstreamProxyErrorCode::UserInfoNotAllowed,
            ));
        }
        if parsed.host_str().is_none() || parsed.port().is_none() {
            return Err(UpstreamProxyError::new(
                UpstreamProxyErrorCode::MissingHostOrPort,
            ));
        }
        if !matches!(parsed.path(), "" | "/") {
            return Err(UpstreamProxyError::new(
                UpstreamProxyErrorCode::PathNotAllowed,
            ));
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(UpstreamProxyError::new(
                UpstreamProxyErrorCode::QueryOrFragmentNotAllowed,
            ));
        }

        Ok(Self::Socks5(Socks5ProxyEndpoint {
            canonical_url: parsed.into(),
        }))
    }

    fn proxy_url(&self) -> Option<&str> {
        match self {
            Self::Direct => None,
            Self::Socks5(endpoint) => Some(&endpoint.canonical_url),
        }
    }
}

impl fmt::Debug for UpstreamProxy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct => formatter.write_str("UpstreamProxy::Direct"),
            Self::Socks5(_) => formatter.write_str("UpstreamProxy::Socks5(<redacted>)"),
        }
    }
}

/// A validated SOCKS5 proxy endpoint with a redacted diagnostic form.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct Socks5ProxyEndpoint {
    canonical_url: String,
}

impl fmt::Debug for Socks5ProxyEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Socks5ProxyEndpoint(<redacted>)")
    }
}

/// Stable proxy-admission failure categories without a raw proxy URL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpstreamProxyErrorCode {
    /// The proxy text was not a parseable URL.
    InvalidUrl,
    /// The proxy had a scheme other than local-DNS SOCKS5.
    UnsupportedScheme,
    /// The proxy URL carried a credential-like user-info component.
    UserInfoNotAllowed,
    /// The proxy URL did not declare both a Host and an explicit port.
    MissingHostOrPort,
    /// The proxy URL had a path other than `/`.
    PathNotAllowed,
    /// The proxy URL had a query or fragment component.
    QueryOrFragmentNotAllowed,
}

/// A safe proxy-admission error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpstreamProxyError {
    code: UpstreamProxyErrorCode,
}

impl UpstreamProxyError {
    const fn new(code: UpstreamProxyErrorCode) -> Self {
        Self { code }
    }

    /// Returns the stable proxy-admission failure category.
    #[must_use]
    pub const fn code(self) -> UpstreamProxyErrorCode {
        self.code
    }
}

impl fmt::Display for UpstreamProxyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            UpstreamProxyErrorCode::InvalidUrl => "upstream proxy URL is invalid",
            UpstreamProxyErrorCode::UnsupportedScheme => "upstream proxy must use local-DNS socks5",
            UpstreamProxyErrorCode::UserInfoNotAllowed => {
                "upstream proxy URL must not contain user-info"
            }
            UpstreamProxyErrorCode::MissingHostOrPort => {
                "upstream proxy URL requires a Host and port"
            }
            UpstreamProxyErrorCode::PathNotAllowed => "upstream proxy URL must not contain a path",
            UpstreamProxyErrorCode::QueryOrFragmentNotAllowed => {
                "upstream proxy URL must not contain a query or fragment"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for UpstreamProxyError {}

/// One immutable transport profile used to select an isolated shared client.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct UpstreamTransportProfile {
    timeouts: UpstreamTimeouts,
    proxy: UpstreamProxy,
    maximum_idle_connections_per_host: NonZeroUsize,
}

impl UpstreamTransportProfile {
    /// Creates one profile from validated timeouts, proxy mode, and a finite per-host idle limit.
    #[must_use]
    pub const fn new(
        timeouts: UpstreamTimeouts,
        proxy: UpstreamProxy,
        maximum_idle_connections_per_host: NonZeroUsize,
    ) -> Self {
        Self {
            timeouts,
            proxy,
            maximum_idle_connections_per_host,
        }
    }

    /// Returns the immutable timeout profile.
    #[must_use]
    pub const fn timeouts(&self) -> UpstreamTimeouts {
        self.timeouts
    }

    /// Returns the isolated proxy mode.
    #[must_use]
    pub const fn proxy(&self) -> &UpstreamProxy {
        &self.proxy
    }

    /// Returns the maximum retained idle connections for each origin in this profile.
    #[must_use]
    pub const fn maximum_idle_connections_per_host(&self) -> NonZeroUsize {
        self.maximum_idle_connections_per_host
    }
}

impl fmt::Debug for UpstreamTransportProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamTransportProfile")
            .field("timeouts", &self.timeouts)
            .field("proxy", &self.proxy)
            .field(
                "maximum_idle_connections_per_host",
                &self.maximum_idle_connections_per_host,
            )
            .finish()
    }
}

/// A bounded process-shared cache of immutable HTTP clients.
///
/// The caller normally owns this behind one `Arc`; every cache entry is keyed by profile, origin,
/// and the exact admitted address set. This prevents a direct request, a proxied request, or a DNS
/// answer from reusing a connection created for another transport identity.
#[derive(Clone)]
pub struct UpstreamClientPool {
    clients: Cache<ClientPoolKey, Arc<Client>>,
    maximum_cached_clients: NonZeroUsize,
}

impl UpstreamClientPool {
    /// Creates one client pool with a finite configured capacity for cached client identities.
    #[must_use]
    pub fn new(maximum_cached_clients: NonZeroUsize) -> Self {
        Self {
            clients: Cache::new(maximum_cached_clients.get() as u64),
            maximum_cached_clients,
        }
    }

    /// Sends one request after P2-09 admission and returns an opaque raw upstream response.
    ///
    /// This performs no status-code classification, redirect follow, response parsing, stream
    /// decoding, credential leasing, candidate selection, or retry. The caller may inspect only
    /// the safe transport envelope and consume body chunks through [`UpstreamHttpResponse`].
    ///
    /// # Errors
    ///
    /// Returns `EgressUnavailable/Egress` for connect, first-byte, body-idle, total, or lower
    /// transport failure. It never retains raw URLs, headers, bodies, proxy values, or diagnostics.
    pub async fn send(
        &self,
        request: UpstreamHttpRequest,
        profile: &UpstreamTransportProfile,
    ) -> Result<UpstreamHttpResponse, GatewayError> {
        let deadline = Instant::now()
            .checked_add(profile.timeouts().total())
            .ok_or_else(internal_error)?;
        let client = self.client_for(request.target(), profile)?;
        let first_byte_budget = remaining_budget(deadline)?.min(profile.timeouts().ttfb());
        let response =
            time::timeout(first_byte_budget, client.execute(request.into_reqwest())).await;
        let response = match response {
            Err(_) | Ok(Err(_)) => return Err(egress_unavailable_error()),
            Ok(Ok(response)) => response,
        };

        Ok(UpstreamHttpResponse {
            status: response.status().as_u16(),
            headers: response.headers().clone(),
            response,
            deadline,
            idle_timeout: profile.timeouts().idle(),
            terminal_failure: false,
        })
    }

    fn client_for(
        &self,
        target: &AdmittedEgressTarget,
        profile: &UpstreamTransportProfile,
    ) -> Result<Arc<Client>, GatewayError> {
        let key = ClientPoolKey::new(target, profile);
        if let Some(client) = self.clients.get(&key) {
            return Ok(client);
        }

        let built = Arc::new(build_client(target, profile)?);
        Ok(self.clients.get_with(key, || built))
    }
}

impl fmt::Debug for UpstreamClientPool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamClientPool")
            .field("maximum_cached_clients", &self.maximum_cached_clients)
            .field("cached_client_count", &self.clients.entry_count())
            .finish()
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct ClientPoolKey {
    scheme: &'static str,
    host: String,
    port: u16,
    addresses: Vec<IpAddr>,
    profile: UpstreamTransportProfile,
}

impl ClientPoolKey {
    fn new(target: &AdmittedEgressTarget, profile: &UpstreamTransportProfile) -> Self {
        Self {
            scheme: target.scheme().as_str(),
            host: target.host().as_str(),
            port: target.port(),
            addresses: target.resolved_addresses().to_vec(),
            profile: profile.clone(),
        }
    }
}

fn build_client(
    target: &AdmittedEgressTarget,
    profile: &UpstreamTransportProfile,
) -> Result<Client, GatewayError> {
    let addresses = target
        .resolved_addresses()
        .iter()
        .map(|address| SocketAddr::new(*address, 0))
        .collect::<Vec<_>>();
    let mut builder = Client::builder()
        .no_proxy()
        .no_hickory_dns()
        .redirect(RedirectPolicy::none())
        .retry(retry::never())
        .connect_timeout(profile.timeouts().connect())
        .read_timeout(profile.timeouts().idle())
        .timeout(profile.timeouts().total())
        .pool_idle_timeout(Some(profile.timeouts().idle()))
        .pool_max_idle_per_host(profile.maximum_idle_connections_per_host().get());

    if matches!(target.host(), EgressHost::Domain(_)) {
        let host = target.host().as_str();
        builder = builder.resolve_to_addrs(&host, &addresses);
    }
    if let Some(proxy_url) = profile.proxy().proxy_url() {
        let proxy = Proxy::all(proxy_url).map_err(|_| egress_unavailable_error())?;
        builder = builder.proxy(proxy);
    }

    builder.build().map_err(|_| egress_unavailable_error())
}

/// The HTTP method allowed by the bounded upstream transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpstreamHttpMethod {
    /// A bodyless fetch request.
    Get,
    /// A body-bearing submission request.
    Post,
}

impl UpstreamHttpMethod {
    const fn as_reqwest(self) -> Method {
        match self {
            Self::Get => Method::GET,
            Self::Post => Method::POST,
        }
    }
}

/// One admitted HTTP request with validated transport headers and opaque body bytes.
pub struct UpstreamHttpRequest {
    target: AdmittedEgressTarget,
    method: UpstreamHttpMethod,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl UpstreamHttpRequest {
    /// Builds one request that can only be sent to its already admitted target.
    ///
    /// Hop-by-hop, Host, framing, and proxy-authentication headers are rejected so the transport
    /// retains control of origin selection, request framing, and proxy isolation.
    ///
    /// # Errors
    ///
    /// Returns [`UpstreamHttpRequestError`] without retaining a header name or value.
    pub fn try_new(
        target: AdmittedEgressTarget,
        method: UpstreamHttpMethod,
        headers: impl IntoIterator<Item = (String, String)>,
        body: Vec<u8>,
    ) -> Result<Self, UpstreamHttpRequestError> {
        let mut validated_headers = HeaderMap::new();
        for (name, value) in headers {
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                UpstreamHttpRequestError::new(UpstreamHttpRequestErrorCode::InvalidHeaderName)
            })?;
            if forbidden_header(&name) {
                return Err(UpstreamHttpRequestError::new(
                    UpstreamHttpRequestErrorCode::ForbiddenHeader,
                ));
            }
            let value = HeaderValue::from_str(&value).map_err(|_| {
                UpstreamHttpRequestError::new(UpstreamHttpRequestErrorCode::InvalidHeaderValue)
            })?;
            if validated_headers.insert(name, value).is_some() {
                return Err(UpstreamHttpRequestError::new(
                    UpstreamHttpRequestErrorCode::DuplicateHeader,
                ));
            }
        }

        Ok(Self {
            target,
            method,
            headers: validated_headers,
            body,
        })
    }

    /// Returns the target admitted by P2-09 for this exact request.
    #[must_use]
    pub const fn target(&self) -> &AdmittedEgressTarget {
        &self.target
    }

    /// Returns the selected HTTP method.
    #[must_use]
    pub const fn method(&self) -> UpstreamHttpMethod {
        self.method
    }

    /// Returns one validated outbound header without creating a diagnostic rendering.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&HeaderValue> {
        self.headers.get(name)
    }

    /// Returns the opaque request body for a later transport handoff.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    fn into_reqwest(self) -> reqwest::Request {
        let mut request =
            reqwest::Request::new(self.method.as_reqwest(), self.target.request_url().clone());
        *request.headers_mut() = self.headers;
        *request.body_mut() = Some(self.body.into());
        request
    }
}

impl fmt::Debug for UpstreamHttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamHttpRequest")
            .field("target", &"<redacted>")
            .field("method", &self.method)
            .field("header_count", &self.headers.len())
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

fn forbidden_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "content-length"
            | "expect"
            | "host"
            | "keep-alive"
            | "proxy-authorization"
            | "proxy-connection"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

/// Stable request-admission failure categories without raw headers or body bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpstreamHttpRequestErrorCode {
    /// A header name could not be parsed.
    InvalidHeaderName,
    /// A header value could not be parsed.
    InvalidHeaderValue,
    /// A header attempted to control host, framing, connection, or proxy behavior.
    ForbiddenHeader,
    /// The same canonical header name occurred more than once.
    DuplicateHeader,
}

/// A safe outbound request-admission error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpstreamHttpRequestError {
    code: UpstreamHttpRequestErrorCode,
}

impl UpstreamHttpRequestError {
    const fn new(code: UpstreamHttpRequestErrorCode) -> Self {
        Self { code }
    }

    /// Returns the stable request-admission failure category.
    #[must_use]
    pub const fn code(self) -> UpstreamHttpRequestErrorCode {
        self.code
    }
}

impl fmt::Display for UpstreamHttpRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            UpstreamHttpRequestErrorCode::InvalidHeaderName => {
                "upstream request header name is invalid"
            }
            UpstreamHttpRequestErrorCode::InvalidHeaderValue => {
                "upstream request header value is invalid"
            }
            UpstreamHttpRequestErrorCode::ForbiddenHeader => {
                "upstream request header is transport-controlled"
            }
            UpstreamHttpRequestErrorCode::DuplicateHeader => {
                "upstream request contains a duplicate header"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for UpstreamHttpRequestError {}

/// A raw upstream response whose body must be consumed under idle and total deadlines.
pub struct UpstreamHttpResponse {
    status: u16,
    headers: HeaderMap,
    response: reqwest::Response,
    deadline: Instant,
    idle_timeout: Duration,
    terminal_failure: bool,
}

impl UpstreamHttpResponse {
    /// Returns the unclassified HTTP status code.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Returns one unclassified raw response header for a later protocol decoder.
    ///
    /// Callers must not render the returned value to logs without an explicit redaction policy.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&HeaderValue> {
        self.headers.get(name)
    }

    /// Returns every raw value for one response Header name without rendering it.
    ///
    /// Callers must validate and redact the values themselves. This preserves duplicate-header
    /// visibility for Provider parsers that need to reject ambiguous security or quota evidence.
    pub fn header_values(&self, name: &str) -> impl Iterator<Item = &HeaderValue> {
        self.headers.get_all(name).iter()
    }

    /// Pulls one raw response-body chunk, enforcing both response-idle and total deadlines.
    ///
    /// A timeout or lower-level read error becomes terminal for this response and maps to
    /// `EgressUnavailable/Egress`. Semantic response parsing is deliberately deferred to P3-06.
    ///
    /// # Errors
    ///
    /// Returns `EgressUnavailable/Egress` after a transport failure or deadline expiration.
    pub async fn next_chunk(&mut self) -> Result<Option<Bytes>, GatewayError> {
        if self.terminal_failure {
            return Err(egress_unavailable_error());
        }
        let remaining = match remaining_budget(self.deadline) {
            Ok(remaining) => remaining,
            Err(error) => {
                self.terminal_failure = true;
                return Err(error);
            }
        };
        let read_budget = remaining.min(self.idle_timeout);
        match time::timeout(read_budget, self.response.chunk()).await {
            Err(_) | Ok(Err(_)) => {
                self.terminal_failure = true;
                Err(egress_unavailable_error())
            }
            Ok(Ok(chunk)) => Ok(chunk),
        }
    }
}

impl fmt::Debug for UpstreamHttpResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamHttpResponse")
            .field("status", &self.status)
            .field("header_count", &self.headers.len())
            .field("body", &"<streaming>")
            .finish_non_exhaustive()
    }
}

fn remaining_budget(deadline: Instant) -> Result<Duration, GatewayError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(egress_unavailable_error)
}

const fn egress_unavailable_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressUnavailable, ErrorScope::Egress)
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        error::Error,
        io,
        net::{IpAddr, Ipv4Addr},
        num::NonZeroUsize,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use gateway_core::{EgressPolicyId, ErrorScope, GatewayErrorCode};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        time,
    };

    use crate::{
        AdmittedEgressTarget, EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost,
        EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy,
    };

    use super::{
        UpstreamClientPool, UpstreamHttpMethod, UpstreamHttpRequest, UpstreamHttpRequestErrorCode,
        UpstreamProxy, UpstreamProxyErrorCode, UpstreamTimeoutErrorCode, UpstreamTimeouts,
        UpstreamTransportProfile,
    };

    const LOCAL_ADDRESS: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    #[derive(Clone, Copy)]
    struct StaticResolver;

    impl EgressDnsResolver for StaticResolver {
        fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
            Ok(vec![LOCAL_ADDRESS])
        }
    }

    fn non_zero(value: usize) -> Result<NonZeroUsize, io::Error> {
        NonZeroUsize::new(value).ok_or_else(|| io::Error::other("test value must be non-zero"))
    }

    fn admitted_target(port: u16) -> Result<AdmittedEgressTarget, Box<dyn Error>> {
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("p3-02-test-policy")?,
            name: "test policy".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Http]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("relay.test")?]),
            allowed_ports: BTreeSet::from([port]),
            allowed_cidrs: BTreeSet::from([EgressCidr::try_new(LOCAL_ADDRESS, 32)?]),
            redirect_policy: RedirectPolicy::Deny,
        })?;
        Ok(policy.admit_url(
            &format!("http://relay.test:{port}/responses"),
            &StaticResolver,
        )?)
    }

    fn request(target: AdmittedEgressTarget) -> Result<UpstreamHttpRequest, Box<dyn Error>> {
        Ok(UpstreamHttpRequest::try_new(
            target,
            UpstreamHttpMethod::Post,
            [("content-type".to_owned(), "application/json".to_owned())],
            br"{}".to_vec(),
        )?)
    }

    fn profile(
        timeouts: UpstreamTimeouts,
        proxy: UpstreamProxy,
    ) -> Result<UpstreamTransportProfile, Box<dyn Error>> {
        Ok(UpstreamTransportProfile::new(timeouts, proxy, non_zero(4)?))
    }

    fn pool() -> Result<UpstreamClientPool, Box<dyn Error>> {
        Ok(UpstreamClientPool::new(non_zero(8)?))
    }

    fn regular_timeouts() -> Result<UpstreamTimeouts, Box<dyn Error>> {
        Ok(UpstreamTimeouts::try_new(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(2),
        )?)
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
                    "peer closed before complete request headers",
                ));
            }
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                return Ok(());
            }
            if bytes.len() > MAX_HEADER_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "test request headers exceeded the bound",
                ));
            }
        }
    }

    async fn write_response(socket: &mut TcpStream, body: &[u8]) -> io::Result<()> {
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
            body.len()
        );
        socket.write_all(head.as_bytes()).await?;
        socket.write_all(body).await?;
        socket.flush().await
    }

    async fn body_bytes(
        response: &mut super::UpstreamHttpResponse,
    ) -> Result<Vec<u8>, gateway_core::GatewayError> {
        let mut body = Vec::new();
        while let Some(chunk) = response.next_chunk().await? {
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    fn assert_egress_unavailable(error: &gateway_core::GatewayError) {
        assert_eq!(error.code(), GatewayErrorCode::EgressUnavailable);
        assert_eq!(error.scope(), ErrorScope::Egress);
    }

    #[test]
    fn timeout_and_proxy_configuration_fail_closed_and_redact_proxy_values()
    -> Result<(), Box<dyn Error>> {
        assert_eq!(
            UpstreamTimeouts::try_new(
                Duration::ZERO,
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
            )
            .err()
            .map(super::UpstreamTimeoutError::code),
            Some(UpstreamTimeoutErrorCode::ZeroConnect)
        );
        assert_eq!(
            UpstreamTimeouts::try_new(
                Duration::from_secs(2),
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
            )
            .err()
            .map(super::UpstreamTimeoutError::code),
            Some(UpstreamTimeoutErrorCode::ConnectExceedsTotal)
        );

        for unsupported in [
            "http://127.0.0.1:7897",
            "https://127.0.0.1:7897",
            "socks5h://127.0.0.1:7897",
        ] {
            assert_eq!(
                UpstreamProxy::try_socks5(unsupported)
                    .err()
                    .map(super::UpstreamProxyError::code),
                Some(UpstreamProxyErrorCode::UnsupportedScheme)
            );
        }
        let proxy = UpstreamProxy::try_socks5("socks5://127.0.0.1:7897")?;
        let debug = format!("{proxy:?}");
        assert!(!debug.contains("127.0.0.1"));
        assert!(!debug.contains("7897"));

        assert_eq!(
            UpstreamHttpRequest::try_new(
                admitted_target(8080)?,
                UpstreamHttpMethod::Post,
                [("te".to_owned(), "trailers".to_owned())],
                Vec::new(),
            )
            .err()
            .map(super::UpstreamHttpRequestError::code),
            Some(UpstreamHttpRequestErrorCode::ForbiddenHeader)
        );
        Ok(())
    }

    #[tokio::test]
    async fn direct_profile_reuses_the_same_dns_pinned_connection() -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let accepted = Arc::new(AtomicUsize::new(0));
        let accepted_for_server = Arc::clone(&accepted);
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            accepted_for_server.fetch_add(1, Ordering::SeqCst);
            for body in [b"one".as_slice(), b"two".as_slice()] {
                read_headers(&mut socket).await?;
                write_response(&mut socket, body).await?;
            }
            Ok::<(), io::Error>(())
        });

        let pool = pool()?;
        let profile = profile(regular_timeouts()?, UpstreamProxy::Direct)?;
        let mut first = pool
            .send(request(admitted_target(port)?)?, &profile)
            .await?;
        assert_eq!(body_bytes(&mut first).await?, b"one");
        let mut second = pool
            .send(request(admitted_target(port)?)?, &profile)
            .await?;
        assert_eq!(body_bytes(&mut second).await?, b"two");
        server.await??;

        assert_eq!(accepted.load(Ordering::SeqCst), 1);
        pool.clients.run_pending_tasks();
        assert_eq!(pool.clients.entry_count(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn automatic_redirects_are_exposed_without_a_second_dispatch()
    -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            read_headers(&mut socket).await?;
            socket
                .write_all(
                    b"HTTP/1.1 302 Found\r\nContent-Length: 0\r\nConnection: close\r\nLocation: /next\r\n\r\n",
                )
                .await?;
            socket.flush().await
        });

        let response = pool()?
            .send(
                request(admitted_target(port)?)?,
                &profile(regular_timeouts()?, UpstreamProxy::Direct)?,
            )
            .await?;
        assert_eq!(response.status(), 302);
        server.await??;
        Ok(())
    }

    #[tokio::test]
    async fn client_protocol_failures_do_not_trigger_a_hidden_retry() -> Result<(), Box<dyn Error>>
    {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            read_headers(&mut socket).await?;
            drop(socket);
            Ok::<bool, io::Error>(
                time::timeout(Duration::from_millis(250), listener.accept())
                    .await
                    .is_err(),
            )
        });

        let error = pool()?
            .send(
                request(admitted_target(port)?)?,
                &profile(regular_timeouts()?, UpstreamProxy::Direct)?,
            )
            .await
            .err()
            .ok_or_else(|| io::Error::other("protocol failure unexpectedly succeeded"))?;
        assert_egress_unavailable(&error);
        assert!(server.await??);
        Ok(())
    }

    #[tokio::test]
    async fn first_byte_timeout_precedes_the_total_deadline() -> Result<(), Box<dyn Error>> {
        let first_byte_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let first_byte_port = first_byte_listener.local_addr()?.port();
        let first_byte_server = tokio::spawn(async move {
            let (mut socket, _) = first_byte_listener.accept().await?;
            read_headers(&mut socket).await?;
            time::sleep(Duration::from_millis(500)).await;
            let _ignored = write_response(&mut socket, b"late").await;
            Ok::<(), io::Error>(())
        });
        let pool = pool()?;
        let first_byte_profile = profile(
            UpstreamTimeouts::try_new(
                Duration::from_millis(500),
                Duration::from_millis(150),
                Duration::from_millis(500),
                Duration::from_millis(800),
            )?,
            UpstreamProxy::Direct,
        )?;
        let first_byte_error = pool
            .send(
                request(admitted_target(first_byte_port)?)?,
                &first_byte_profile,
            )
            .await
            .err()
            .ok_or_else(|| io::Error::other("TTFB timeout unexpectedly succeeded"))?;
        assert_egress_unavailable(&first_byte_error);
        let _joined_first_byte = first_byte_server.await;
        Ok(())
    }

    #[tokio::test]
    async fn connect_timeout_bounds_the_socks_handshake_before_ttfb() -> Result<(), Box<dyn Error>>
    {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let proxy_port = listener.local_addr()?.port();
        let server = tokio::spawn(async move {
            let (_socket, _) = listener.accept().await?;
            time::sleep(Duration::from_millis(500)).await;
            Ok::<(), io::Error>(())
        });
        let profile = profile(
            UpstreamTimeouts::try_new(
                Duration::from_millis(100),
                Duration::from_millis(500),
                Duration::from_millis(500),
                Duration::from_millis(800),
            )?,
            UpstreamProxy::try_socks5(&format!("socks5://127.0.0.1:{proxy_port}"))?,
        )?;
        let error = pool()?
            .send(request(admitted_target(8080)?)?, &profile)
            .await
            .err()
            .ok_or_else(|| io::Error::other("connect timeout unexpectedly succeeded"))?;
        assert_egress_unavailable(&error);
        server.await??;
        Ok(())
    }

    #[tokio::test]
    async fn idle_timeout_applies_between_response_chunks() -> Result<(), Box<dyn Error>> {
        let idle_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let idle_port = idle_listener.local_addr()?.port();
        let idle_server = tokio::spawn(async move {
            let (mut socket, _) = idle_listener.accept().await?;
            read_headers(&mut socket).await?;
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\na")
                .await?;
            socket.flush().await?;
            time::sleep(Duration::from_millis(500)).await;
            let _ignored = socket.write_all(b"b").await;
            Ok::<(), io::Error>(())
        });
        let pool = pool()?;
        let idle_profile = profile(
            UpstreamTimeouts::try_new(
                Duration::from_millis(500),
                Duration::from_millis(500),
                Duration::from_millis(250),
                Duration::from_secs(1),
            )?,
            UpstreamProxy::Direct,
        )?;
        let mut idle_response = pool
            .send(request(admitted_target(idle_port)?)?, &idle_profile)
            .await?;
        assert!(idle_response.next_chunk().await?.is_some());
        let idle_error = idle_response
            .next_chunk()
            .await
            .err()
            .ok_or_else(|| io::Error::other("idle timeout unexpectedly succeeded"))?;
        assert_egress_unavailable(&idle_error);
        let _joined_idle = idle_server.await;
        Ok(())
    }

    #[tokio::test]
    async fn total_timeout_applies_while_chunks_continue_to_arrive() -> Result<(), Box<dyn Error>> {
        let total_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let total_port = total_listener.local_addr()?.port();
        let total_server = tokio::spawn(async move {
            let (mut socket, _) = total_listener.accept().await?;
            read_headers(&mut socket).await?;
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n")
                .await?;
            for byte in *b"abcd" {
                socket.write_all(&[byte]).await?;
                socket.flush().await?;
                time::sleep(Duration::from_millis(220)).await;
            }
            Ok::<(), io::Error>(())
        });
        let pool = pool()?;
        let total_profile = profile(
            UpstreamTimeouts::try_new(
                Duration::from_millis(500),
                Duration::from_millis(500),
                Duration::from_millis(500),
                Duration::from_millis(600),
            )?,
            UpstreamProxy::Direct,
        )?;
        let mut total_response = pool
            .send(request(admitted_target(total_port)?)?, &total_profile)
            .await?;
        let mut total_timed_out = false;
        loop {
            match total_response.next_chunk().await {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(error) => {
                    assert_egress_unavailable(&error);
                    total_timed_out = true;
                    break;
                }
            }
        }
        assert!(total_timed_out);
        let _joined_total = total_server.await;
        Ok(())
    }

    #[tokio::test]
    async fn direct_and_socks5_profiles_are_isolated_and_socks_receives_the_pinned_ip()
    -> Result<(), Box<dyn Error>> {
        let direct_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let target_port = direct_listener.local_addr()?.port();
        let direct_server = tokio::spawn(async move {
            let (mut socket, _) = direct_listener.accept().await?;
            read_headers(&mut socket).await?;
            write_response(&mut socket, b"direct").await
        });

        let socks_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let socks_port = socks_listener.local_addr()?.port();
        let socks_server = tokio::spawn(async move {
            let (mut socket, _) = socks_listener.accept().await?;
            let mut greeting = [0_u8; 2];
            socket.read_exact(&mut greeting).await?;
            if greeting[0] != 5 || greeting[1] == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid SOCKS5 greeting",
                ));
            }
            let mut methods = vec![0_u8; usize::from(greeting[1])];
            socket.read_exact(&mut methods).await?;
            if !methods.contains(&0) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "SOCKS5 client did not offer no-authentication",
                ));
            }
            socket.write_all(&[5, 0]).await?;

            let mut request_head = [0_u8; 4];
            socket.read_exact(&mut request_head).await?;
            if request_head != [5, 1, 0, 1] {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "SOCKS5 target was not an IPv4 CONNECT request",
                ));
            }
            let mut address = [0_u8; 4];
            socket.read_exact(&mut address).await?;
            let mut port = [0_u8; 2];
            socket.read_exact(&mut port).await?;
            if IpAddr::V4(Ipv4Addr::from(address)) != LOCAL_ADDRESS
                || u16::from_be_bytes(port) != target_port
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "SOCKS5 did not receive the admitted address and port",
                ));
            }
            socket.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            read_headers(&mut socket).await?;
            write_response(&mut socket, b"socks").await
        });

        let pool = pool()?;
        let direct_profile = profile(regular_timeouts()?, UpstreamProxy::Direct)?;
        let socks_proxy = UpstreamProxy::try_socks5(&format!("socks5://127.0.0.1:{socks_port}"))?;
        let socks_profile = profile(regular_timeouts()?, socks_proxy)?;

        let mut direct = pool
            .send(request(admitted_target(target_port)?)?, &direct_profile)
            .await?;
        assert_eq!(body_bytes(&mut direct).await?, b"direct");
        let mut socks = pool
            .send(request(admitted_target(target_port)?)?, &socks_profile)
            .await?;
        assert_eq!(body_bytes(&mut socks).await?, b"socks");
        direct_server.await??;
        socks_server.await??;

        pool.clients.run_pending_tasks();
        assert_eq!(pool.clients.entry_count(), 2);
        Ok(())
    }
}
