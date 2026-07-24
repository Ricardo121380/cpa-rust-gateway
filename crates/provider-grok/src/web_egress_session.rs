//! Immutable, credential-bound browser egress sessions for `grok.web`.
//!
//! This module validates and binds an already imported Web credential, one operator-supplied
//! browser User-Agent, one TLS profile label, and one admitted proxy choice. It neither opens a
//! browser/profile nor creates a client, socket, TLS handshake, DNS lookup, proxy change, or Web
//! request. P9-03 owns the later fixed Web request boundary.

use std::{error::Error, fmt, net::IpAddr};

use gateway_upstream::UpstreamProxy;
use zeroize::Zeroizing;

use crate::{GROK_WEB_PROVIDER_ID, GrokWebCredential};

const MAX_EGRESS_SESSION_ID_BYTES: usize = 128;
const MAX_USER_AGENT_BYTES: usize = 512;
const MAX_TLS_PROFILE_ID_BYTES: usize = 128;
const MAX_REQUEST_HOST_BYTES: usize = 253;
const MAX_REQUEST_PATH_BYTES: usize = 2_048;
const MAX_COOKIE_HEADER_BYTES: usize = 64 * 1024;

/// A stable, opaque identifier for one independently scheduled Web egress session.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GrokWebEgressSessionId(String);

impl GrokWebEgressSessionId {
    /// Validates a caller-assigned opaque egress-session identifier.
    ///
    /// # Errors
    ///
    /// Returns a safe error without retaining an invalid value.
    pub fn try_new(value: &str) -> Result<Self, GrokWebBrowserEgressSessionError> {
        validate_opaque_identifier(value, MAX_EGRESS_SESSION_ID_BYTES)
            .map_err(|()| GrokWebBrowserEgressSessionError::InvalidEgressSessionId)?;
        Ok(Self(value.to_owned()))
    }

    /// Returns the stable opaque identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GrokWebEgressSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("GrokWebEgressSessionId")
            .field(&self.0)
            .finish()
    }
}

/// A validated, redacted User-Agent that participates in a Web egress fingerprint.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebBrowserUserAgent(Zeroizing<String>);

impl GrokWebBrowserUserAgent {
    /// Validates one explicit browser User-Agent without discovering a host/browser default.
    ///
    /// # Errors
    ///
    /// Returns a safe error for an empty, oversized, non-ASCII, or header-injection value.
    pub fn try_new(value: &str) -> Result<Self, GrokWebBrowserEgressSessionError> {
        if value.is_empty()
            || value.len() > MAX_USER_AGENT_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_graphic() || byte == b' ')
        {
            return Err(GrokWebBrowserEgressSessionError::InvalidUserAgent);
        }
        Ok(Self(Zeroizing::new(value.to_owned())))
    }

    /// Borrows this exact value only while a later Web request assembles its User-Agent Header.
    #[must_use]
    pub fn header_value(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for GrokWebBrowserUserAgent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokWebBrowserUserAgent(<redacted>)")
    }
}

/// An explicit, bounded browser TLS-profile label.
///
/// It identifies the TLS implementation/profile selected by a later transport integration; it
/// does not attempt TLS impersonation or infer a profile from the User-Agent.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GrokWebTlsProfile(String);

impl GrokWebTlsProfile {
    /// Validates one explicit TLS-profile label.
    ///
    /// # Errors
    ///
    /// Returns a safe error without retaining an invalid label.
    pub fn try_new(value: &str) -> Result<Self, GrokWebBrowserEgressSessionError> {
        validate_opaque_identifier(value, MAX_TLS_PROFILE_ID_BYTES)
            .map_err(|()| GrokWebBrowserEgressSessionError::InvalidTlsProfile)?;
        Ok(Self(value.to_owned()))
    }

    /// Returns the selected explicit TLS-profile label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GrokWebTlsProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("GrokWebTlsProfile")
            .field(&self.0)
            .finish()
    }
}

/// One immutable `grok.web` browser egress-session fingerprint.
///
/// Its credential account/lineage/revision, Cookie scopes, User-Agent, TLS profile, proxy
/// choice, and egress-session identifier cannot be swapped after construction. The generic
/// `UpstreamProxy` type is accepted only as an already explicit `Direct` or validated local-DNS
/// SOCKS5 selection; this boundary never consults environment or system proxy configuration.
pub struct GrokWebBrowserEgressSession {
    egress_session_id: GrokWebEgressSessionId,
    credential: GrokWebCredential,
    user_agent: GrokWebBrowserUserAgent,
    tls_profile: GrokWebTlsProfile,
    proxy: UpstreamProxy,
    created_at_ms: i64,
}

impl GrokWebBrowserEgressSession {
    /// Binds one currently usable Web credential to an exact browser egress fingerprint.
    ///
    /// # Errors
    ///
    /// Returns a safe error when the supplied observation time or credential is unusable. This
    /// method performs no filesystem, browser, proxy, DNS, TLS, or network action.
    pub fn try_new(
        egress_session_id: GrokWebEgressSessionId,
        credential: GrokWebCredential,
        user_agent: GrokWebBrowserUserAgent,
        tls_profile: GrokWebTlsProfile,
        proxy: UpstreamProxy,
        created_at_ms: i64,
    ) -> Result<Self, GrokWebBrowserEgressSessionError> {
        if created_at_ms < 0 {
            return Err(GrokWebBrowserEgressSessionError::InvalidObservationTime);
        }
        if credential.is_expired_at(created_at_ms) {
            return Err(GrokWebBrowserEgressSessionError::ExpiredCredential);
        }
        Ok(Self {
            egress_session_id,
            credential,
            user_agent,
            tls_profile,
            proxy,
            created_at_ms,
        })
    }

    /// Returns the fixed Provider identity for this browser session.
    #[must_use]
    pub const fn provider_id() -> &'static str {
        GROK_WEB_PROVIDER_ID
    }

    /// Returns the opaque identifier of this exact egress session.
    #[must_use]
    pub const fn egress_session_id(&self) -> &GrokWebEgressSessionId {
        &self.egress_session_id
    }

    /// Returns the credential's opaque account reference without exposing Cookie values.
    #[must_use]
    pub fn account_reference(&self) -> &str {
        self.credential.account_reference()
    }

    /// Returns the credential's opaque SSO lineage reference without exposing Cookie values.
    #[must_use]
    pub fn lineage_reference(&self) -> &str {
        self.credential.lineage().reference()
    }

    /// Returns the exact credential revision bound into this egress session.
    #[must_use]
    pub const fn credential_revision(&self) -> u64 {
        self.credential.revision()
    }

    /// Returns the absolute credential expiry bound into this egress session.
    #[must_use]
    pub const fn credential_expires_at_ms(&self) -> i64 {
        self.credential.expires_at_ms()
    }

    /// Returns the exact User-Agent component of the immutable session fingerprint.
    #[must_use]
    pub const fn user_agent(&self) -> &GrokWebBrowserUserAgent {
        &self.user_agent
    }

    /// Returns the exact TLS-profile component of the immutable session fingerprint.
    #[must_use]
    pub const fn tls_profile(&self) -> &GrokWebTlsProfile {
        &self.tls_profile
    }

    /// Returns the exact already-admitted proxy component of the immutable session fingerprint.
    #[must_use]
    pub const fn proxy(&self) -> &UpstreamProxy {
        &self.proxy
    }

    /// Returns the supplied construction instant without consulting a clock.
    #[must_use]
    pub const fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    /// Returns whether this immutable session may start work at the supplied instant.
    #[must_use]
    pub const fn is_expired_at(&self, now_ms: i64) -> bool {
        self.credential.is_expired_at(now_ms)
    }

    /// Verifies a currently loaded credential still exactly matches this session's identity.
    ///
    /// P9-03 and P9-04 must call this before continuing a request or conversation. A refreshed
    /// SSO credential creates a new egress session instead of mutating this session in place.
    ///
    /// # Errors
    ///
    /// Returns a safe mismatch or expiry category without rendering account, lineage, Cookie, or
    /// proxy values.
    pub fn require_current_credential(
        &self,
        credential: &GrokWebCredential,
        now_ms: i64,
    ) -> Result<(), GrokWebBrowserEgressSessionError> {
        if now_ms < 0 {
            return Err(GrokWebBrowserEgressSessionError::InvalidObservationTime);
        }
        if self.is_expired_at(now_ms) || credential.is_expired_at(now_ms) {
            return Err(GrokWebBrowserEgressSessionError::ExpiredCredential);
        }
        if self.credential.account_reference() != credential.account_reference() {
            return Err(GrokWebBrowserEgressSessionError::AccountMismatch);
        }
        if self.credential.lineage() != credential.lineage() {
            return Err(GrokWebBrowserEgressSessionError::LineageMismatch);
        }
        if self.credential.revision() != credential.revision() {
            return Err(GrokWebBrowserEgressSessionError::RevisionMismatch);
        }
        Ok(())
    }

    /// Builds a bounded secure Cookie header only for the requested HTTPS host/path scope.
    ///
    /// The caller must provide a host and path from a separately admitted fixed Web target. This
    /// method never parses a URL, resolves DNS, or sends a request. It returns zeroizing header
    /// storage and rejects a host/path without an eligible scoped Cookie.
    ///
    /// # Errors
    ///
    /// Returns a safe scope or expiry classification without rendering Cookie values.
    pub fn cookie_header_for_https(
        &self,
        request_host: &str,
        request_path: &str,
        now_ms: i64,
    ) -> Result<Zeroizing<String>, GrokWebBrowserEgressSessionError> {
        if now_ms < 0 {
            return Err(GrokWebBrowserEgressSessionError::InvalidObservationTime);
        }
        if self.is_expired_at(now_ms) {
            return Err(GrokWebBrowserEgressSessionError::ExpiredCredential);
        }
        let request_host = validate_request_host(request_host)?;
        validate_request_path(request_path)?;
        let mut cookies = self
            .credential
            .cookies()
            .iter()
            .filter(|cookie| {
                host_matches_cookie_domain(&request_host, cookie.domain())
                    && path_matches_cookie_path(request_path, cookie.path())
            })
            .collect::<Vec<_>>();
        if cookies.is_empty() {
            return Err(GrokWebBrowserEgressSessionError::CookieScopeMismatch);
        }
        cookies.sort_by(|left, right| {
            right
                .path()
                .len()
                .cmp(&left.path().len())
                .then_with(|| left.name().cmp(right.name()))
                .then_with(|| left.domain().cmp(right.domain()))
        });
        let mut header = Zeroizing::new(String::new());
        for (index, cookie) in cookies.iter().enumerate() {
            let separator_len = usize::from(index != 0) * 2;
            let next_length = header
                .len()
                .checked_add(separator_len)
                .and_then(|length| length.checked_add(cookie.name().len()))
                .and_then(|length| length.checked_add(1))
                .and_then(|length| length.checked_add(cookie.value().len()))
                .ok_or(GrokWebBrowserEgressSessionError::CookieHeaderTooLarge)?;
            if next_length > MAX_COOKIE_HEADER_BYTES {
                return Err(GrokWebBrowserEgressSessionError::CookieHeaderTooLarge);
            }
            if index != 0 {
                header.push_str("; ");
            }
            header.push_str(cookie.name());
            header.push('=');
            header.push_str(cookie.value());
        }
        Ok(header)
    }
}

impl fmt::Debug for GrokWebBrowserEgressSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebBrowserEgressSession")
            .field("provider_id", &GROK_WEB_PROVIDER_ID)
            .field("egress_session_id", &self.egress_session_id)
            .field("account_reference", &self.credential.account_reference())
            .field("lineage", &self.credential.lineage())
            .field("credential_revision", &self.credential.revision())
            .field("credential_expires_at_ms", &self.credential.expires_at_ms())
            .field("cookie_count", &self.credential.cookies().len())
            .field("user_agent", &self.user_agent)
            .field("tls_profile", &self.tls_profile)
            .field("proxy", &self.proxy)
            .field("created_at_ms", &self.created_at_ms)
            .finish()
    }
}

/// Safe validation, binding, or Cookie-scope failure for a Web browser egress session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebBrowserEgressSessionError {
    /// The opaque egress-session identifier was invalid.
    InvalidEgressSessionId,
    /// The explicit browser User-Agent was invalid.
    InvalidUserAgent,
    /// The explicit TLS-profile label was invalid.
    InvalidTlsProfile,
    /// A caller-supplied observation time was negative.
    InvalidObservationTime,
    /// The current or supplied Web credential session is expired.
    ExpiredCredential,
    /// The supplied credential belongs to a different opaque Web account reference.
    AccountMismatch,
    /// The supplied credential has a different SSO lineage.
    LineageMismatch,
    /// The supplied credential has a different revision.
    RevisionMismatch,
    /// The caller supplied an invalid fixed request host or path scope.
    InvalidRequestScope,
    /// The fixed request host/path matched no secure Cookie scope.
    CookieScopeMismatch,
    /// The selected Cookie header would exceed its finite request bound.
    CookieHeaderTooLarge,
}

impl fmt::Display for GrokWebBrowserEgressSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEgressSessionId => "Grok Web egress session identity is invalid",
            Self::InvalidUserAgent => "Grok Web browser User-Agent is invalid",
            Self::InvalidTlsProfile => "Grok Web TLS profile is invalid",
            Self::InvalidObservationTime => "Grok Web egress session observation time is invalid",
            Self::ExpiredCredential => "Grok Web egress session credential is expired",
            Self::AccountMismatch => "Grok Web egress session account does not match",
            Self::LineageMismatch => "Grok Web egress session lineage does not match",
            Self::RevisionMismatch => "Grok Web egress session credential revision does not match",
            Self::InvalidRequestScope => "Grok Web request Cookie scope is invalid",
            Self::CookieScopeMismatch => "Grok Web request has no matching Cookie scope",
            Self::CookieHeaderTooLarge => "Grok Web Cookie header exceeds its limit",
        })
    }
}

impl Error for GrokWebBrowserEgressSessionError {}

fn validate_opaque_identifier(value: &str, maximum_bytes: usize) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(());
    }
    Ok(())
}

fn validate_request_host(value: &str) -> Result<String, GrokWebBrowserEgressSessionError> {
    let normalized = value.to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > MAX_REQUEST_HOST_BYTES
        || normalized.parse::<IpAddr>().is_ok()
        || !normalized
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
        || normalized.starts_with('.')
        || normalized.ends_with('.')
        || normalized.contains("..")
        || !normalized.contains('.')
    {
        return Err(GrokWebBrowserEgressSessionError::InvalidRequestScope);
    }
    Ok(normalized)
}

fn validate_request_path(value: &str) -> Result<(), GrokWebBrowserEgressSessionError> {
    if value.is_empty()
        || value.len() > MAX_REQUEST_PATH_BYTES
        || !value.starts_with('/')
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || matches!(byte, b';' | b'?' | b'#'))
    {
        return Err(GrokWebBrowserEgressSessionError::InvalidRequestScope);
    }
    Ok(())
}

fn host_matches_cookie_domain(request_host: &str, cookie_domain: &str) -> bool {
    request_host == cookie_domain
        || request_host
            .strip_suffix(cookie_domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn path_matches_cookie_path(request_path: &str, cookie_path: &str) -> bool {
    request_path == cookie_path
        || request_path
            .strip_prefix(cookie_path)
            .is_some_and(|remainder| cookie_path.ends_with('/') || remainder.starts_with('/'))
}
