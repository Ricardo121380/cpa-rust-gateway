//! Local Statsig signer admission and exact signature-cache isolation for `grok.web`.
//!
//! This module never sends a signer request. It delegates all URL, DNS, CIDR, and redirect
//! admission to an injected P2 `EgressPolicy`, then provides a bounded in-memory cache whose 403
//! remediation can invalidate only one exact method/path/environment key.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
};

use base64::{Engine as _, engine::general_purpose};
use gateway_core::{EgressPolicyId, ErrorScope, GatewayError, GatewayErrorCode};
use gateway_provider::ProviderFuture;
use gateway_upstream::{
    AdmittedEgressTarget, EgressCidr, EgressDnsResolver, EgressHost, EgressPolicy,
    EgressPolicyInput, EgressScheme, RedirectPolicy, UpstreamClientPool, UpstreamHttpMethod,
    UpstreamHttpRequest, UpstreamHttpResponse, UpstreamProxy, UpstreamTransportProfile,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use url::Url;
use zeroize::Zeroizing;

use crate::{GROK_WEB_CANARY_HOST, GROK_WEB_CANARY_PATH, GrokWebBrowserEgressSession};

const MAX_SIGNATURE_METHOD_BYTES: usize = 16;
const MAX_SIGNATURE_PATH_BYTES: usize = 2_048;
const MAX_ENVIRONMENT_VERSION_BYTES: usize = 128;
const MAX_SIGNATURE_VALUE_BYTES: usize = 16 * 1024;
const MAX_SIGNATURE_CACHE_ENTRIES: usize = 256;
const STATSIG_CACHE_TTL_MS: i64 = 60 * 60 * 1_000;
const MAX_STATSIG_META_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_STATSIG_META_VALUE_BYTES: usize = 16 * 1024;
const MAX_STATSIG_RESPONSE_BYTES: usize = 4 * 1024;
const GROK_WEB_INDEX_URL: &str = "https://grok.com/index";
const GROK_WEB_INDEX_PATH: &str = "/index";

/// Frozen grok2api-compatible signer service used to derive current Web request signatures.
pub const GROK_WEB_DEFAULT_STATSIG_SIGNER_URL: &str = "https://grok.wodf.de/sign";

/// One exact request shape used to scope a Statsig signature cache entry.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct GrokWebStatsigSignatureKey {
    method: String,
    path: String,
    environment_version: String,
}

impl GrokWebStatsigSignatureKey {
    /// Creates one bounded method/path/environment signature cache key.
    ///
    /// # Errors
    ///
    /// Returns a safe category without retaining an invalid path or environment value.
    pub fn try_new(
        method: &str,
        path: &str,
        environment_version: &str,
    ) -> Result<Self, GrokWebStatsigError> {
        if method.is_empty()
            || method.len() > MAX_SIGNATURE_METHOD_BYTES
            || !method.bytes().all(|byte| byte.is_ascii_uppercase())
        {
            return Err(GrokWebStatsigError::InvalidMethod);
        }
        if path.is_empty()
            || path.len() > MAX_SIGNATURE_PATH_BYTES
            || !path.starts_with('/')
            || path.starts_with("//")
            || path.contains(['?', '#'])
            || !path
                .bytes()
                .all(|byte| byte.is_ascii_graphic() && byte != b'\\')
        {
            return Err(GrokWebStatsigError::InvalidPath);
        }
        if environment_version.is_empty()
            || environment_version.len() > MAX_ENVIRONMENT_VERSION_BYTES
            || !environment_version
                .bytes()
                .all(|byte| byte.is_ascii_graphic())
        {
            return Err(GrokWebStatsigError::InvalidEnvironmentVersion);
        }
        Ok(Self {
            method: method.to_owned(),
            path: path.to_owned(),
            environment_version: environment_version.to_owned(),
        })
    }
}

impl fmt::Debug for GrokWebStatsigSignatureKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebStatsigSignatureKey")
            .field("method", &self.method)
            .field("path", &"<redacted>")
            .field("environment_version", &"<redacted>")
            .finish()
    }
}

/// Bounded zeroizing signature value for one already-admitted signer response.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebStatsigSignature(Zeroizing<String>);

impl GrokWebStatsigSignature {
    /// Creates a bounded printable signature value without retaining invalid input.
    ///
    /// # Errors
    ///
    /// Returns a safe category for empty, oversized, or header-injection input.
    pub fn try_new(value: &str) -> Result<Self, GrokWebStatsigError> {
        if value.is_empty()
            || value.len() > MAX_SIGNATURE_VALUE_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_graphic() || byte == b' ')
        {
            return Err(GrokWebStatsigError::InvalidSignature);
        }
        Ok(Self(Zeroizing::new(value.to_owned())))
    }

    /// Borrows the value only for a later explicit signer-header composition boundary.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for GrokWebStatsigSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokWebStatsigSignature(<redacted>)")
    }
}

/// A bounded local cache of exact Statsig signatures.
///
/// The cache has no URL, HTTP, browser, Cookie, proxy, or account state. A later caller must
/// first obtain a signature through an independently admitted request boundary before insertion.
pub struct GrokWebStatsigSignatureCache {
    capacity: usize,
    entries: Mutex<BTreeMap<GrokWebStatsigSignatureKey, GrokWebStatsigSignatureEntry>>,
}

impl GrokWebStatsigSignatureCache {
    /// Creates one finite in-memory signature cache.
    ///
    /// # Errors
    ///
    /// Returns a safe category if the requested capacity is zero or exceeds the fixed bound.
    pub fn try_new(capacity: usize) -> Result<Self, GrokWebStatsigError> {
        if capacity == 0 || capacity > MAX_SIGNATURE_CACHE_ENTRIES {
            return Err(GrokWebStatsigError::InvalidCacheCapacity);
        }
        Ok(Self {
            capacity,
            entries: Mutex::new(BTreeMap::new()),
        })
    }

    /// Returns the current unexpired value for one exact key, if present.
    ///
    /// An expired entry is removed only for this exact key. The caller supplies time; this method
    /// does not read a system clock.
    ///
    /// # Errors
    ///
    /// Returns a safe category for invalid time or unavailable local cache state.
    pub fn get(
        &self,
        key: &GrokWebStatsigSignatureKey,
        now_ms: i64,
    ) -> Result<Option<GrokWebStatsigSignature>, GrokWebStatsigError> {
        if now_ms < 0 {
            return Err(GrokWebStatsigError::InvalidObservationTime);
        }
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| GrokWebStatsigError::CacheUnavailable)?;
        let Some(entry) = entries.get(key) else {
            return Ok(None);
        };
        if now_ms >= entry.expires_at_ms {
            entries.remove(key);
            return Ok(None);
        }
        Ok(Some(entry.signature.clone()))
    }

    /// Inserts or replaces exactly one signature after removing globally expired entries.
    ///
    /// # Errors
    ///
    /// Returns a safe category for invalid time/expiry, unavailable cache state, or exhausted
    /// bounded capacity. No existing unexpired different key is evicted.
    pub fn insert(
        &self,
        key: GrokWebStatsigSignatureKey,
        signature: GrokWebStatsigSignature,
        expires_at_ms: i64,
        now_ms: i64,
    ) -> Result<(), GrokWebStatsigError> {
        if now_ms < 0 || expires_at_ms <= now_ms {
            return Err(GrokWebStatsigError::InvalidExpiry);
        }
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| GrokWebStatsigError::CacheUnavailable)?;
        entries.retain(|_, entry| entry.expires_at_ms > now_ms);
        if !entries.contains_key(&key) && entries.len() >= self.capacity {
            return Err(GrokWebStatsigError::CacheCapacityExhausted);
        }
        entries.insert(
            key,
            GrokWebStatsigSignatureEntry {
                signature,
                expires_at_ms,
            },
        );
        Ok(())
    }

    /// Invalidates only the exact signer cache key associated with an observed HTTP 403.
    ///
    /// P9-07 owns deciding whether a 403 is WAF/egress/account evidence. This narrow primitive
    /// cannot clear another environment, path, method, account, or the whole cache.
    ///
    /// # Errors
    ///
    /// Returns a safe category only when local cache state is unavailable.
    pub fn invalidate_after_403(
        &self,
        key: &GrokWebStatsigSignatureKey,
    ) -> Result<bool, GrokWebStatsigError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| GrokWebStatsigError::CacheUnavailable)?;
        Ok(entries.remove(key).is_some())
    }
}

impl fmt::Debug for GrokWebStatsigSignatureCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entry_count = self.entries.lock().map_or(0, |entries| entries.len());
        formatter
            .debug_struct("GrokWebStatsigSignatureCache")
            .field("capacity", &self.capacity)
            .field("entry_count", &entry_count)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
struct GrokWebStatsigSignatureEntry {
    signature: GrokWebStatsigSignature,
    expires_at_ms: i64,
}

/// An exact signer target that passed injected P2 URL, DNS, CIDR, and redirect admission.
///
/// It deliberately does not expose the raw URL. A later P9-09 transport integration may consume
/// the wrapped admitted target inside this crate after explicit Canary authorization.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebStatsigSignerTarget {
    admitted: AdmittedEgressTarget,
}

impl GrokWebStatsigSignerTarget {
    /// Returns the admitted scheme for safe local policy assertions.
    #[must_use]
    pub const fn scheme(&self) -> EgressScheme {
        self.admitted.scheme()
    }

    /// Returns the admitted exact Host for safe policy assertions.
    #[must_use]
    pub fn host(&self) -> String {
        self.admitted.host().as_str()
    }

    /// Returns the admitted effective port for safe policy assertions.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.admitted.port()
    }

    pub(crate) const fn admitted_target(&self) -> &AdmittedEgressTarget {
        &self.admitted
    }
}

impl fmt::Debug for GrokWebStatsigSignerTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokWebStatsigSignerTarget(<redacted>)")
    }
}

/// Dependency-injected SSRF and redirect admission boundary for a future Statsig signer request.
pub struct GrokWebStatsigSignerBoundary {
    egress_policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
}

impl GrokWebStatsigSignerBoundary {
    /// Combines a caller-owned exact P2 egress policy and resolver without sending any request.
    #[must_use]
    pub fn new(egress_policy: EgressPolicy, resolver: Arc<dyn EgressDnsResolver>) -> Self {
        Self {
            egress_policy,
            resolver,
        }
    }

    /// Fully admits one initial HTTPS signer URL through the injected P2 policy.
    ///
    /// # Errors
    ///
    /// Returns a safe rejection category for malformed, non-HTTPS, untrusted, private/DNS,
    /// policy, or resolver failures. It does not create an HTTP client or send a request.
    pub fn admit_initial(
        &self,
        url: &str,
    ) -> Result<GrokWebStatsigSignerTarget, GrokWebStatsigError> {
        let parsed = Url::parse(url).map_err(|_| GrokWebStatsigError::SignerAdmissionRejected)?;
        if parsed.scheme() != "https" {
            return Err(GrokWebStatsigError::SignerMustUseHttps);
        }
        let admitted = self
            .egress_policy
            .admit_url(url, self.resolver.as_ref())
            .map_err(|_| GrokWebStatsigError::SignerAdmissionRejected)?;
        require_https(admitted)
    }

    /// Re-admits one supplied redirect Location through the same exact P2 policy.
    ///
    /// # Errors
    ///
    /// Returns a safe rejection category when redirects are disabled, exhausted, cross-origin, or
    /// otherwise not fully re-admitted. It never follows or sends the redirect itself.
    pub fn admit_redirect(
        &self,
        current: &GrokWebStatsigSignerTarget,
        location: &str,
        followed_redirects: u8,
    ) -> Result<GrokWebStatsigSignerTarget, GrokWebStatsigError> {
        let admitted = self
            .egress_policy
            .admit_redirect(
                current.admitted_target(),
                location,
                followed_redirects,
                self.resolver.as_ref(),
            )
            .map_err(|_| GrokWebStatsigError::SignerAdmissionRejected)?;
        require_https(admitted)
    }
}

impl fmt::Debug for GrokWebStatsigSignerBoundary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokWebStatsigSignerBoundary(<redacted policy and resolver>)")
    }
}

/// Injected Statsig environment/signature transport.
///
/// Production uses one Chrome-emulated request to the Web origin for the current environment and
/// one independently admitted standard HTTPS request to the signer. Tests can inject fixtures
/// without DNS or network access.
pub trait GrokWebStatsigTransport: Send + Sync {
    /// Fetches the current bounded Web environment value with the exact browser session.
    fn fetch_environment<'a>(
        &'a self,
        session: &'a GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> ProviderFuture<'a, Result<Zeroizing<String>, GatewayError>>;

    /// Signs one exact method/path/environment tuple.
    fn sign<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        environment: &'a str,
    ) -> ProviderFuture<'a, Result<GrokWebStatsigSignature, GatewayError>>;
}

/// Singleflight, one-hour Statsig cache matching the frozen grok2api production behavior.
pub struct GrokWebStatsigRuntime {
    cache: GrokWebStatsigSignatureCache,
    current: Mutex<Option<(GrokWebStatsigSignatureKey, GrokWebStatsigSignature)>>,
    refresh: tokio::sync::Mutex<()>,
    transport: Arc<dyn GrokWebStatsigTransport>,
}

impl GrokWebStatsigRuntime {
    /// Creates one bounded runtime around an injected transport.
    ///
    /// # Errors
    ///
    /// Fails if the fixed cache capacity cannot be constructed.
    pub fn try_new(
        transport: Arc<dyn GrokWebStatsigTransport>,
    ) -> Result<Self, GrokWebStatsigError> {
        Ok(Self {
            cache: GrokWebStatsigSignatureCache::try_new(16)?,
            current: Mutex::new(None),
            refresh: tokio::sync::Mutex::new(()),
            transport,
        })
    }

    /// Returns the current signature, refreshing the environment and signer at most once across
    /// concurrent callers when no unexpired cache entry exists.
    ///
    /// # Errors
    ///
    /// Returns a value-free transport, protocol, cache, time, or signer failure.
    pub async fn signature(
        &self,
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<GrokWebStatsigSignature, GatewayError> {
        if let Some(signature) = self.cached(now_ms)? {
            return Ok(signature);
        }
        let _guard = self.refresh.lock().await;
        if let Some(signature) = self.cached(now_ms)? {
            return Ok(signature);
        }
        let mut environment = self.transport.fetch_environment(session, now_ms).await?;
        let signature = if let Ok(signature) = self
            .transport
            .sign("POST", GROK_WEB_CANARY_PATH, environment.as_str())
            .await
        {
            signature
        } else {
            environment = self.transport.fetch_environment(session, now_ms).await?;
            self.transport
                .sign("POST", GROK_WEB_CANARY_PATH, environment.as_str())
                .await?
        };
        let environment_version = environment_digest(environment.as_str());
        let key =
            GrokWebStatsigSignatureKey::try_new("POST", GROK_WEB_CANARY_PATH, &environment_version)
                .map_err(|_| statsig_internal_error())?;
        let expires_at_ms = now_ms
            .checked_add(STATSIG_CACHE_TTL_MS)
            .ok_or_else(statsig_internal_error)?;
        self.cache
            .insert(key.clone(), signature.clone(), expires_at_ms, now_ms)
            .map_err(|_| statsig_internal_error())?;
        *self.current.lock().map_err(|_| statsig_internal_error())? =
            Some((key, signature.clone()));
        Ok(signature)
    }

    /// Invalidates only the currently active method/path/environment key after a pre-start 403.
    ///
    /// # Errors
    ///
    /// Returns a value-free local state failure.
    pub fn invalidate_after_403(&self) -> Result<bool, GatewayError> {
        let current = self
            .current
            .lock()
            .map_err(|_| statsig_internal_error())?
            .take();
        current.map_or(Ok(false), |(key, _)| {
            self.cache
                .invalidate_after_403(&key)
                .map_err(|_| statsig_internal_error())
        })
    }

    /// Invalidates the current key only when it produced the exact rejected request signature.
    ///
    /// A concurrent request may already have refreshed the shared key before an older 403 arrives;
    /// that stale response must not delete the replacement signature.
    ///
    /// # Errors
    ///
    /// Returns a value-free local state failure.
    pub fn invalidate_signature_after_403(
        &self,
        rejected: &GrokWebStatsigSignature,
    ) -> Result<bool, GatewayError> {
        let mut current = self.current.lock().map_err(|_| statsig_internal_error())?;
        let Some((key, signature)) = current.as_ref() else {
            return Ok(false);
        };
        if signature != rejected {
            return Ok(false);
        }
        let key = key.clone();
        current.take();
        self.cache
            .invalidate_after_403(&key)
            .map_err(|_| statsig_internal_error())
    }

    fn cached(&self, now_ms: i64) -> Result<Option<GrokWebStatsigSignature>, GatewayError> {
        let key = self
            .current
            .lock()
            .map_err(|_| statsig_internal_error())?
            .as_ref()
            .map(|(key, _)| key.clone());
        key.map_or(Ok(None), |key| {
            self.cache
                .get(&key, now_ms)
                .map_err(|_| statsig_internal_error())
        })
    }
}

impl fmt::Debug for GrokWebStatsigRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebStatsigRuntime")
            .field("cache", &self.cache)
            .field("transport", &"<injected>")
            .finish_non_exhaustive()
    }
}

/// Production Statsig transport through the shared DNS-pinned client pool.
pub struct GrokWebStatsigUpstreamTransport {
    egress_policy: EgressPolicy,
    resolver: Arc<dyn EgressDnsResolver>,
    client_pool: UpstreamClientPool,
    browser_profile: UpstreamTransportProfile,
    signer_profile: UpstreamTransportProfile,
    signer_url: String,
    local_signer_policy: Option<EgressPolicy>,
}

impl GrokWebStatsigUpstreamTransport {
    /// Creates one fixed Web-origin and signer transport binding.
    #[must_use]
    pub fn new(
        egress_policy: EgressPolicy,
        resolver: Arc<dyn EgressDnsResolver>,
        client_pool: UpstreamClientPool,
        profile: &UpstreamTransportProfile,
    ) -> Self {
        Self::new_with_signer_url(egress_policy, resolver, client_pool, profile, None)
    }

    /// Creates one fixed Web-origin binding and an optional explicitly local browser signer.
    ///
    /// The local override is limited to loopback HTTP so a staging process can use a co-located
    /// Playwright signer without widening the persisted upstream policy or sending Web traffic to
    /// an arbitrary operator-supplied URL. The production default remains the frozen HTTPS signer.
    #[must_use]
    pub fn new_with_signer_url(
        egress_policy: EgressPolicy,
        resolver: Arc<dyn EgressDnsResolver>,
        client_pool: UpstreamClientPool,
        profile: &UpstreamTransportProfile,
        signer_url: Option<String>,
    ) -> Self {
        let (signer_url, local_signer_policy) = signer_url
            .and_then(|url| {
                local_signer_policy(&url)
                    .ok()
                    .map(|policy| (url, Some(policy)))
            })
            .unwrap_or_else(|| (GROK_WEB_DEFAULT_STATSIG_SIGNER_URL.to_owned(), None));
        let signer_profile = if local_signer_policy.is_some() {
            UpstreamTransportProfile::new(
                profile.timeouts(),
                UpstreamProxy::Direct,
                profile.maximum_idle_connections_per_host(),
            )
        } else {
            profile.clone()
        };
        Self {
            egress_policy,
            resolver,
            client_pool,
            browser_profile: profile.clone().with_chrome_146_emulation(),
            signer_profile,
            signer_url,
            local_signer_policy,
        }
    }
}

impl fmt::Debug for GrokWebStatsigUpstreamTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebStatsigUpstreamTransport")
            .field("egress_policy", self.egress_policy.id())
            .field("resolver", &"<injected>")
            .field("client_pool", &self.client_pool)
            .field("browser_profile", &self.browser_profile)
            .field("signer_profile", &self.signer_profile)
            .field("signer_url", &"<redacted>")
            .field("local_signer_policy", &self.local_signer_policy.is_some())
            .finish()
    }
}

impl GrokWebStatsigTransport for GrokWebStatsigUpstreamTransport {
    fn fetch_environment<'a>(
        &'a self,
        session: &'a GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> ProviderFuture<'a, Result<Zeroizing<String>, GatewayError>> {
        if self.local_signer_policy.is_some() {
            // A loopback browser signer already loaded the authenticated Grok page and captured
            // the browser-generated signature. Avoid repeating that page load through the Rust
            // transport: Cloudflare may reject the non-browser TLS fingerprint even though the
            // co-located browser session is valid. The placeholder is cache-key material only;
            // it never leaves the loopback signer boundary as an account or Cookie value.
            return Box::pin(async {
                Ok(Zeroizing::new(
                    "loopback-browser-observed-environment".to_owned(),
                ))
            });
        }
        let admitted = self
            .egress_policy
            .admit_url(GROK_WEB_INDEX_URL, self.resolver.as_ref())
            .map_err(|_| statsig_egress_error());
        let request = admitted.and_then(|target| web_index_request(session, target, now_ms));
        Box::pin(async move {
            let mut response = self
                .client_pool
                .send(request?, &self.browser_profile)
                .await?;
            if !(200..300).contains(&response.status()) {
                return Err(statsig_http_error(response.status()));
            }
            let body = read_bounded_body(&mut response, MAX_STATSIG_META_BODY_BYTES).await?;
            extract_statsig_meta_content(&body)
        })
    }

    fn sign<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        environment: &'a str,
    ) -> ProviderFuture<'a, Result<GrokWebStatsigSignature, GatewayError>> {
        let admitted = if let Some(policy) = &self.local_signer_policy {
            policy
                .admit_url(&self.signer_url, self.resolver.as_ref())
                .map(|admitted| GrokWebStatsigSignerTarget { admitted })
                .map_err(|_| statsig_egress_error())
        } else {
            GrokWebStatsigSignerBoundary::new(
                self.egress_policy.clone(),
                Arc::clone(&self.resolver),
            )
            .admit_initial(&self.signer_url)
            .map_err(|_| statsig_egress_error())
        };
        let request =
            admitted.and_then(|target| signer_request(&target, method, path, environment));
        Box::pin(async move {
            let mut response = self
                .client_pool
                .send(request?, &self.signer_profile)
                .await?;
            if !(200..300).contains(&response.status()) {
                return Err(statsig_http_error(response.status()));
            }
            let body = read_bounded_body(&mut response, MAX_STATSIG_RESPONSE_BYTES).await?;
            decode_signer_response(&body)
        })
    }
}

fn web_index_request(
    session: &GrokWebBrowserEgressSession,
    target: AdmittedEgressTarget,
    now_ms: i64,
) -> Result<UpstreamHttpRequest, GatewayError> {
    if target.scheme() != EgressScheme::Https
        || target.host().as_str() != GROK_WEB_CANARY_HOST
        || target.port() != 443
        || target.request_url().as_str() != GROK_WEB_INDEX_URL
    {
        return Err(statsig_egress_error());
    }
    let cookie = session
        .cookie_header_for_https(GROK_WEB_CANARY_HOST, GROK_WEB_INDEX_PATH, now_ms)
        .map_err(|_| statsig_credential_error())?;
    UpstreamHttpRequest::try_new(
        target,
        UpstreamHttpMethod::Get,
        [
            (
                "accept".to_owned(),
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_owned(),
            ),
            (
                "accept-encoding".to_owned(),
                "gzip, deflate, br, zstd".to_owned(),
            ),
            (
                "accept-language".to_owned(),
                "zh-CN,zh;q=0.9,en;q=0.8".to_owned(),
            ),
            ("cache-control".to_owned(), "no-cache".to_owned()),
            ("pragma".to_owned(), "no-cache".to_owned()),
            ("sec-fetch-dest".to_owned(), "document".to_owned()),
            ("sec-fetch-mode".to_owned(), "navigate".to_owned()),
            ("sec-fetch-site".to_owned(), "same-origin".to_owned()),
            ("upgrade-insecure-requests".to_owned(), "1".to_owned()),
            (
                "user-agent".to_owned(),
                session.user_agent().header_value().to_owned(),
            ),
            ("cookie".to_owned(), cookie.to_string()),
        ],
        Vec::new(),
    )
    .map_err(|_| statsig_egress_error())
}

fn signer_request(
    target: &GrokWebStatsigSignerTarget,
    method: &str,
    path: &str,
    environment: &str,
) -> Result<UpstreamHttpRequest, GatewayError> {
    let key = GrokWebStatsigSignatureKey::try_new(method, path, "request")
        .map_err(|_| statsig_protocol_error())?;
    let _ = key;
    if environment.is_empty()
        || environment.len() > MAX_STATSIG_META_VALUE_BYTES
        || environment.chars().any(char::is_control)
    {
        return Err(statsig_protocol_error());
    }
    let body = serde_json::to_vec(&Value::Object(Map::from_iter([
        ("method".to_owned(), Value::String(method.to_owned())),
        ("path".to_owned(), Value::String(path.to_owned())),
        (
            "environment".to_owned(),
            Value::Object(Map::from_iter([(
                "metaContent".to_owned(),
                Value::String(environment.to_owned()),
            )])),
        ),
    ])))
    .map_err(|_| statsig_internal_error())?;
    UpstreamHttpRequest::try_new(
        target.admitted_target().clone(),
        UpstreamHttpMethod::Post,
        [
            ("accept".to_owned(), "application/json".to_owned()),
            ("content-type".to_owned(), "application/json".to_owned()),
        ],
        body,
    )
    .map_err(|_| statsig_egress_error())
}

async fn read_bounded_body(
    response: &mut UpstreamHttpResponse,
    maximum_bytes: usize,
) -> Result<Vec<u8>, GatewayError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.next_chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(statsig_protocol_error());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn extract_statsig_meta_content(body: &[u8]) -> Result<Zeroizing<String>, GatewayError> {
    let text = std::str::from_utf8(body).map_err(|_| statsig_protocol_error())?;
    let lower = text.to_ascii_lowercase();
    let mut offset = 0_usize;
    while let Some(relative) = lower[offset..].find("<meta") {
        let start = offset + relative;
        let Some(relative_end) = lower[start..].find('>') else {
            return Err(statsig_protocol_error());
        };
        let end = start + relative_end + 1;
        if end.saturating_sub(start) > 64 * 1024 {
            return Err(statsig_protocol_error());
        }
        let tag = &text[start + 5..end - 1];
        let attributes = parse_meta_attributes(tag)?;
        let name = attributes
            .get("name")
            .map(|value| normalize_meta_name(value));
        if name.as_deref() == Some("grok-site-verification") {
            let content = attributes
                .get("content")
                .filter(|value| {
                    !value.is_empty()
                        && value.len() <= MAX_STATSIG_META_VALUE_BYTES
                        && !value.chars().any(char::is_control)
                })
                .ok_or_else(statsig_protocol_error)?;
            return Ok(Zeroizing::new(content.to_owned()));
        }
        offset = end;
    }
    Err(statsig_protocol_error())
}

fn parse_meta_attributes(tag: &str) -> Result<BTreeMap<String, String>, GatewayError> {
    let bytes = tag.as_bytes();
    let mut index = 0_usize;
    let mut attributes = BTreeMap::new();
    while index < bytes.len() {
        while index < bytes.len() && (bytes[index].is_ascii_whitespace() || bytes[index] == b'/') {
            index += 1;
        }
        if index == bytes.len() {
            break;
        }
        let name_start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'-' | b'_' | b':'))
        {
            index += 1;
        }
        if index == name_start {
            return Err(statsig_protocol_error());
        }
        let name = tag[name_start..index].to_ascii_lowercase();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index == bytes.len() || bytes[index] != b'=' {
            if attributes.insert(name, String::new()).is_some() {
                return Err(statsig_protocol_error());
            }
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index == bytes.len() {
            return Err(statsig_protocol_error());
        }
        let quote = bytes[index];
        let (value_start, value_end) = if matches!(quote, b'\'' | b'"') {
            index += 1;
            let value_start = index;
            while index < bytes.len() && bytes[index] != quote {
                index += 1;
            }
            if index == bytes.len() {
                return Err(statsig_protocol_error());
            }
            let value_end = index;
            index += 1;
            (value_start, value_end)
        } else {
            let value_start = index;
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'/'
            {
                index += 1;
            }
            (value_start, index)
        };
        if attributes
            .insert(name, tag[value_start..value_end].to_owned())
            .is_some()
        {
            return Err(statsig_protocol_error());
        }
    }
    Ok(attributes)
}

fn normalize_meta_name(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .replace(['‐', '‑', '‒', '–', '—', '―'], "-")
}

fn decode_signer_response(body: &[u8]) -> Result<GrokWebStatsigSignature, GatewayError> {
    let value = crate::strict_json::parse_strict_json(body, MAX_STATSIG_RESPONSE_BYTES)
        .map_err(|()| statsig_protocol_error())?;
    let object = value.as_object().ok_or_else(statsig_protocol_error)?;
    if object.len() != 1 {
        return Err(statsig_protocol_error());
    }
    let signature = object
        .get("x-statsig-id")
        .or_else(|| object.get("statsig"))
        .and_then(Value::as_str)
        .ok_or_else(statsig_protocol_error)?;
    let decoded = general_purpose::STANDARD_NO_PAD
        .decode(signature)
        .or_else(|_| general_purpose::STANDARD.decode(signature))
        .map(Zeroizing::new)
        .map_err(|_| statsig_protocol_error())?;
    if decoded.len() != 70 {
        return Err(statsig_protocol_error());
    }
    GrokWebStatsigSignature::try_new(signature).map_err(|_| statsig_protocol_error())
}

fn local_signer_policy(value: &str) -> Result<EgressPolicy, Box<dyn Error + Send + Sync>> {
    let parsed = Url::parse(value)?;
    let host = parsed.host_str().ok_or("missing host")?;
    if parsed.scheme() != "http"
        || !matches!(host, "127.0.0.1" | "localhost")
        || parsed.port().unwrap_or(80) == 0
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("local signer must be loopback HTTP".into());
    }
    let port = parsed.port().ok_or("local signer requires explicit port")?;
    let address = IpAddr::V4(Ipv4Addr::LOCALHOST);
    Ok(EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("p12-local-grok-web-statsig")?,
        name: "P12 local Grok Web Statsig signer".to_owned(),
        allowed_schemes: std::collections::BTreeSet::from([EgressScheme::Http]),
        allowed_hosts: std::collections::BTreeSet::from([EgressHost::try_new(host)?]),
        allowed_ports: std::collections::BTreeSet::from([port]),
        allowed_cidrs: std::collections::BTreeSet::from([EgressCidr::try_new(address, 32)?]),
        redirect_policy: RedirectPolicy::Deny,
    })?)
}

fn environment_digest(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

const fn statsig_http_error(status: u16) -> GatewayError {
    match status {
        401 => statsig_credential_error(),
        403 => statsig_egress_error(),
        _ => GatewayError::new(GatewayErrorCode::ProviderTransient, ErrorScope::Provider),
    }
}

const fn statsig_credential_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::CredentialUnauthorized,
        ErrorScope::Credential,
    )
}

const fn statsig_egress_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
}

const fn statsig_protocol_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        ErrorScope::Provider,
    )
}

const fn statsig_internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

fn require_https(
    admitted: AdmittedEgressTarget,
) -> Result<GrokWebStatsigSignerTarget, GrokWebStatsigError> {
    if admitted.scheme() != EgressScheme::Https {
        return Err(GrokWebStatsigError::SignerMustUseHttps);
    }
    Ok(GrokWebStatsigSignerTarget { admitted })
}

/// Safe cache or signer-admission failure without raw URL, path, environment, DNS, or signature text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebStatsigError {
    /// Request method was invalid for an exact signature key.
    InvalidMethod,
    /// Request path was invalid for an exact signature key.
    InvalidPath,
    /// Environment-version value was invalid for an exact signature key.
    InvalidEnvironmentVersion,
    /// Signature value was invalid for bounded header composition.
    InvalidSignature,
    /// Cache capacity was outside the fixed local bounds.
    InvalidCacheCapacity,
    /// Supplied observation time was negative.
    InvalidObservationTime,
    /// A signature expiry was not strictly later than supplied observation time.
    InvalidExpiry,
    /// The bounded cache has no expired slot and cannot evict another live key.
    CacheCapacityExhausted,
    /// The local cache mutex was unavailable.
    CacheUnavailable,
    /// P2 target or redirect admission rejected the signer URL without retaining its details.
    SignerAdmissionRejected,
    /// The injected egress policy admitted a signer target without HTTPS.
    SignerMustUseHttps,
}

impl fmt::Display for GrokWebStatsigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidMethod => "Grok Web Statsig signature method is invalid",
            Self::InvalidPath => "Grok Web Statsig signature path is invalid",
            Self::InvalidEnvironmentVersion => "Grok Web Statsig environment version is invalid",
            Self::InvalidSignature => "Grok Web Statsig signature is invalid",
            Self::InvalidCacheCapacity => "Grok Web Statsig cache capacity is invalid",
            Self::InvalidObservationTime => "Grok Web Statsig observation time is invalid",
            Self::InvalidExpiry => "Grok Web Statsig signature expiry is invalid",
            Self::CacheCapacityExhausted => "Grok Web Statsig cache capacity is exhausted",
            Self::CacheUnavailable => "Grok Web Statsig cache is unavailable",
            Self::SignerAdmissionRejected => "Grok Web Statsig signer target was rejected",
            Self::SignerMustUseHttps => "Grok Web Statsig signer target must use HTTPS",
        })
    }
}

impl Error for GrokWebStatsigError {}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use base64::{Engine as _, engine::general_purpose};

    use super::{decode_signer_response, extract_statsig_meta_content};

    #[test]
    fn production_meta_and_signature_shapes_match_the_frozen_reference()
    -> Result<(), Box<dyn Error>> {
        let meta = extract_statsig_meta_content(
            r"<html><meta charset=utf-8><meta content='fixture-env' name='Grok‑Site‑Verification'/></html>"
                .as_bytes(),
        )?;
        assert_eq!(meta.as_str(), "fixture-env");

        let signature = general_purpose::STANDARD_NO_PAD.encode([7_u8; 70]);
        let body = serde_json::to_vec(&serde_json::json!({"x-statsig-id":signature}))?;
        assert!(decode_signer_response(&body).is_ok());

        let short = general_purpose::STANDARD_NO_PAD.encode([7_u8; 69]);
        let short_body = serde_json::to_vec(&serde_json::json!({"x-statsig-id":short}))?;
        assert!(decode_signer_response(&short_body).is_err());
        let ambiguous = serde_json::to_vec(&serde_json::json!({
            "x-statsig-id":signature,
            "unexpected":true
        }))?;
        assert!(decode_signer_response(&ambiguous).is_err());
        Ok(())
    }
}
