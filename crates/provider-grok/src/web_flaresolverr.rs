//! Strict, value-free boundary for an operator-managed `FlareSolverr` instance.
//!
//! This module only builds the fixed loopback request and parses the bounded response. It does
//! not discover a service, open sockets, mutate credentials, or select a proxy.

use std::{collections::BTreeMap, error::Error, fmt};

use gateway_core::GatewayError;
use gateway_provider::ProviderFuture;
use serde::{Deserialize, Serialize};

/// Fixed local `FlareSolverr` endpoint; never configurable from a public request.
pub const GROK_WEB_FLARESOLVERR_URL: &str = "http://127.0.0.1:8191/v1";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_COOKIE_BYTES: usize = 16 * 1024;
const MAX_USER_AGENT_BYTES: usize = 512;

/// One bounded request to the local solver.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct GrokWebFlareSolverrRequest {
    /// Fixed `FlareSolverr` command.
    pub cmd: &'static str,
    /// Fixed Grok origin URL.
    pub url: &'static str,
    /// Bounded solver timeout in milliseconds.
    pub max_timeout: u32,
    /// Optional scoped Cookie header used only for the exact Grok origin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    /// Optional browser proxy URL used by `FlareSolverr` for the fixed Grok origin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
}

impl Default for GrokWebFlareSolverrRequest {
    fn default() -> Self {
        Self {
            cmd: "request.get",
            url: "https://grok.com/",
            max_timeout: 20_000,
            headers: None,
            proxy: None,
        }
    }
}

impl fmt::Debug for GrokWebFlareSolverrRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebFlareSolverrRequest")
            .field("cmd", &self.cmd)
            .field("url", &self.url)
            .field("max_timeout", &self.max_timeout)
            .field("has_headers", &self.headers.is_some())
            .field("has_proxy", &self.proxy.is_some())
            .finish()
    }
}

impl GrokWebFlareSolverrRequest {
    /// Adds the already scoped Cookie header for the fixed Grok origin.
    #[must_use]
    pub fn with_cookie_header(mut self, cookie_header: &str) -> Self {
        let mut headers = BTreeMap::new();
        headers.insert("Cookie".to_owned(), cookie_header.to_owned());
        self.headers = Some(headers);
        self
    }

    /// Adds a previously validated proxy URL for the solver's browser egress.
    #[must_use]
    pub fn with_proxy_url(mut self, proxy_url: &str) -> Self {
        self.proxy = Some(proxy_url.to_owned());
        self
    }

    /// Encodes the fixed request without retaining or logging credential material.
    ///
    /// # Errors
    ///
    /// Returns an encoding error if the fixed request cannot be serialized.
    pub fn to_json(&self) -> Result<Vec<u8>, GrokWebFlareSolverrError> {
        serde_json::to_vec(self).map_err(|_| GrokWebFlareSolverrError::InvalidRequest)
    }
}

/// Response returned by a bounded, injected loopback `FlareSolverr` transport.
pub struct GrokWebFlareSolverrTransportResponse {
    status: u16,
    body: Vec<u8>,
}

impl GrokWebFlareSolverrTransportResponse {
    /// Creates one bounded transport response.
    ///
    /// # Errors
    ///
    /// Returns an error if the body exceeds the fixed response bound.
    pub fn new(status: u16, body: Vec<u8>) -> Result<Self, GrokWebFlareSolverrError> {
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(GrokWebFlareSolverrError::TooLarge);
        }
        Ok(Self { status, body })
    }

    /// Returns the HTTP status.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Consumes the response into its bounded body.
    #[must_use]
    pub fn into_body(self) -> Vec<u8> {
        self.body
    }
}

/// Explicit transport boundary for the local `FlareSolverr` service.
pub trait GrokWebFlareSolverrTransport: Send + Sync {
    /// Sends one fixed request to the already-admitted loopback endpoint.
    fn send(
        &self,
        request: GrokWebFlareSolverrRequest,
    ) -> ProviderFuture<'_, Result<GrokWebFlareSolverrTransportResponse, GatewayError>>;
}

/// Sanitized clearance material returned by the solver.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebFlareSolverrClearance {
    user_agent: String,
    cookies: Vec<(String, String)>,
}

impl GrokWebFlareSolverrClearance {
    /// Parses only the solver response fields needed for the next Grok Web attempt.
    ///
    /// # Errors
    ///
    /// Returns a bounded classification error when the solver response is malformed, rejected,
    /// or does not contain an allowlisted clearance cookie.
    pub fn parse(input: &[u8]) -> Result<Self, GrokWebFlareSolverrError> {
        if input.len() > MAX_RESPONSE_BYTES {
            return Err(GrokWebFlareSolverrError::TooLarge);
        }
        let response: SolverResponse =
            serde_json::from_slice(input).map_err(|_| GrokWebFlareSolverrError::InvalidResponse)?;
        if response.status != "ok" {
            return Err(GrokWebFlareSolverrError::SolverRejected);
        }
        let solution = response
            .solution
            .ok_or(GrokWebFlareSolverrError::MissingSolution)?;
        if solution.user_agent.is_empty() || solution.user_agent.len() > MAX_USER_AGENT_BYTES {
            return Err(GrokWebFlareSolverrError::InvalidUserAgent);
        }
        let mut cookies = Vec::new();
        for cookie in solution.cookies {
            if !is_allowed_clearance_cookie(&cookie.name) {
                continue;
            }
            if cookie.value.is_empty() || cookie.value.len() > MAX_COOKIE_BYTES {
                return Err(GrokWebFlareSolverrError::InvalidCookie);
            }
            if !cookie
                .value
                .bytes()
                .all(|b| b.is_ascii_graphic() && b != b';')
            {
                return Err(GrokWebFlareSolverrError::InvalidCookie);
            }
            cookies.push((cookie.name, cookie.value));
        }
        if cookies.is_empty() {
            return Err(GrokWebFlareSolverrError::MissingClearance);
        }
        Ok(Self {
            user_agent: solution.user_agent,
            cookies,
        })
    }

    /// Returns the solver-provided browser User-Agent.
    #[must_use]
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Returns the allowlisted clearance cookies.
    #[must_use]
    pub fn cookies(&self) -> &[(String, String)] {
        &self.cookies
    }
}

fn is_allowed_clearance_cookie(name: &str) -> bool {
    name == "cf_clearance"
        || name == "__cf_bm"
        || name == "_cfuvid"
        || name.starts_with("cf_chl_")
        || name == "sso"
        || name == "sso-rw"
}

impl fmt::Debug for GrokWebFlareSolverrClearance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebFlareSolverrClearance")
            .field("user_agent", &"<redacted>")
            .field("cookie_count", &self.cookies.len())
            .finish()
    }
}

#[derive(Deserialize)]
struct SolverResponse {
    status: String,
    solution: Option<SolverSolution>,
}

#[derive(Deserialize)]
struct SolverSolution {
    #[serde(rename = "userAgent")]
    user_agent: String,
    cookies: Vec<SolverCookie>,
}

#[derive(Deserialize)]
struct SolverCookie {
    name: String,
    value: String,
}

/// Safe parser failures without response values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebFlareSolverrError {
    /// Request serialization failed.
    InvalidRequest,
    /// Response exceeded the parser bound.
    TooLarge,
    /// Response was not valid JSON of the expected shape.
    InvalidResponse,
    /// Solver returned a non-success status.
    SolverRejected,
    /// Success response omitted its solution.
    MissingSolution,
    /// Solver User-Agent was empty or oversized.
    InvalidUserAgent,
    /// Solver cookie value was unsafe or oversized.
    InvalidCookie,
    /// No allowlisted clearance cookie was returned.
    MissingClearance,
}

impl fmt::Display for GrokWebFlareSolverrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "FlareSolverr request encoding failed",
            Self::TooLarge => "FlareSolverr response too large",
            Self::InvalidResponse => "FlareSolverr response invalid",
            Self::SolverRejected => "FlareSolverr rejected request",
            Self::MissingSolution => "FlareSolverr solution missing",
            Self::InvalidUserAgent => "FlareSolverr user-agent invalid",
            Self::InvalidCookie => "FlareSolverr cookie invalid",
            Self::MissingClearance => "FlareSolverr clearance missing",
        })
    }
}

impl Error for GrokWebFlareSolverrError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_grok_clearance_material() {
        let raw = br#"{"status":"ok","solution":{"userAgent":"Chrome","cookies":[{"name":"cf_clearance","value":"abc"},{"name":"other","value":"ignored"}]}}"#;
        let result = GrokWebFlareSolverrClearance::parse(raw);
        assert!(result.is_ok());
        if let Ok(parsed) = result {
            assert_eq!(parsed.user_agent(), "Chrome");
            assert_eq!(parsed.cookies(), &[("cf_clearance".into(), "abc".into())]);
        }
    }

    #[test]
    fn retains_cloudflare_challenge_cookie_family() {
        let raw = br#"{"status":"ok","solution":{"userAgent":"Chrome","cookies":[{"name":"cf_clearance","value":"abc"},{"name":"__cf_bm","value":"bm"},{"name":"_cfuvid","value":"uvid"},{"name":"cf_chl_test","value":"chl"},{"name":"other","value":"ignored"}]}}"#;
        let parsed = GrokWebFlareSolverrClearance::parse(raw).expect("clearance");
        assert_eq!(parsed.cookies().len(), 4);
        assert!(parsed.cookies().iter().any(|(name, _)| name == "__cf_bm"));
        assert!(
            parsed
                .cookies()
                .iter()
                .any(|(name, _)| name == "cf_chl_test")
        );
    }

    #[test]
    fn rejects_solver_without_clearance() {
        let raw = br#"{"status":"ok","solution":{"userAgent":"Chrome","cookies":[]}}"#;
        assert_eq!(
            GrokWebFlareSolverrClearance::parse(raw),
            Err(GrokWebFlareSolverrError::MissingClearance)
        );
    }

    #[test]
    fn request_serializes_scoped_cookie_without_debug_value() {
        let request = GrokWebFlareSolverrRequest::default().with_cookie_header("sso=secret");
        let Ok(encoded) = request.to_json() else {
            return;
        };
        let encoded = String::from_utf8_lossy(&encoded);
        assert!(encoded.contains("Cookie"));
        assert!(encoded.contains("sso=secret"));
        let debug = format!("{request:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("has_headers: true"));
    }
}
