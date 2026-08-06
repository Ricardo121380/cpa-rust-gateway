//! Strict, value-free boundary for an operator-managed `FlareSolverr` instance.
//!
//! This module only builds the fixed loopback request and parses the bounded response. It does
//! not discover a service, open sockets, mutate credentials, or select a proxy.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

/// Fixed local `FlareSolverr` endpoint; never configurable from a public request.
pub const GROK_WEB_FLARESOLVERR_URL: &str = "http://127.0.0.1:8191/v1";
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_COOKIE_BYTES: usize = 16 * 1024;
const MAX_USER_AGENT_BYTES: usize = 512;

/// One bounded request to the local solver.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GrokWebFlareSolverrRequest {
    /// Fixed `FlareSolverr` command.
    pub cmd: &'static str,
    /// Fixed Grok origin URL.
    pub url: &'static str,
    /// Bounded solver timeout in milliseconds.
    pub max_timeout: u32,
}

impl Default for GrokWebFlareSolverrRequest {
    fn default() -> Self {
        Self {
            cmd: "request.get",
            url: "https://grok.com/",
            max_timeout: 20_000,
        }
    }
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
            if cookie.name != "cf_clearance" && cookie.name != "sso" && cookie.name != "sso-rw" {
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
    fn rejects_solver_without_clearance() {
        let raw = br#"{"status":"ok","solution":{"userAgent":"Chrome","cookies":[]}}"#;
        assert_eq!(
            GrokWebFlareSolverrClearance::parse(raw),
            Err(GrokWebFlareSolverrError::MissingClearance)
        );
    }
}
