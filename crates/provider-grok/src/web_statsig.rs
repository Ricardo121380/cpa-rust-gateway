//! Local Statsig signer admission and exact signature-cache isolation for `grok.web`.
//!
//! This module never sends a signer request. It delegates all URL, DNS, CIDR, and redirect
//! admission to an injected P2 `EgressPolicy`, then provides a bounded in-memory cache whose 403
//! remediation can invalidate only one exact method/path/environment key.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};

use gateway_upstream::{AdmittedEgressTarget, EgressDnsResolver, EgressPolicy, EgressScheme};
use url::Url;
use zeroize::Zeroizing;

const MAX_SIGNATURE_METHOD_BYTES: usize = 16;
const MAX_SIGNATURE_PATH_BYTES: usize = 2_048;
const MAX_ENVIRONMENT_VERSION_BYTES: usize = 128;
const MAX_SIGNATURE_VALUE_BYTES: usize = 16 * 1024;
const MAX_SIGNATURE_CACHE_ENTRIES: usize = 256;

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
