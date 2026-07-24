//! Bounded Grok Official rate-limit and token-usage metadata.
//!
//! P8-03 parses only a fixed allow-list of response headers and already canonicalized token
//! usage. It preserves neither raw Header names/values nor a billing plan/cost claim. P8-05 owns
//! the separate Official runtime-state application and P8-06 owns live confirmation.

use std::{collections::BTreeMap, fmt, time::Duration};

use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode, Usage};

/// Maximum retained decimal bytes in a recognized Official rate-limit value.
pub const MAX_GROK_OFFICIAL_RATE_LIMIT_VALUE_BYTES: usize = 32;
/// Largest accepted explicit rate-limit reset/retry delay.
pub const MAX_GROK_OFFICIAL_RATE_LIMIT_RESET: Duration = Duration::from_hours(24);

const REQUEST_LIMIT: &str = "x-ratelimit-limit-requests";
const REQUEST_REMAINING: &str = "x-ratelimit-remaining-requests";
const REQUEST_RESET: &str = "x-ratelimit-reset-requests";
const TOKEN_LIMIT: &str = "x-ratelimit-limit-tokens";
const TOKEN_REMAINING: &str = "x-ratelimit-remaining-tokens";
const TOKEN_RESET: &str = "x-ratelimit-reset-tokens";
const RETRY_AFTER: &str = "retry-after";

/// The distinct rate-limit resource from which an Official observation was derived.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GrokOfficialRateLimitKind {
    /// The request-count rate limit.
    Requests,
    /// The token-count rate limit.
    Tokens,
}

/// One complete bounded Official rate-limit window.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokOfficialRateLimitWindow {
    kind: GrokOfficialRateLimitKind,
    limit: u64,
    remaining: u64,
    reset_after: Duration,
}

impl GrokOfficialRateLimitWindow {
    fn try_new(
        kind: GrokOfficialRateLimitKind,
        limit: u64,
        remaining: u64,
        reset_after: Duration,
    ) -> Result<Self, GatewayError> {
        if limit == 0
            || remaining > limit
            || reset_after.is_zero()
            || reset_after > MAX_GROK_OFFICIAL_RATE_LIMIT_RESET
        {
            return Err(provider_protocol_error());
        }
        Ok(Self {
            kind,
            limit,
            remaining,
            reset_after,
        })
    }

    /// Returns the window's independent resource kind.
    #[must_use]
    pub const fn kind(&self) -> GrokOfficialRateLimitKind {
        self.kind
    }

    /// Returns the provider-reported total window capacity.
    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    /// Returns the provider-reported remaining capacity.
    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.remaining
    }

    /// Returns the explicit delay before this window can reset.
    #[must_use]
    pub const fn reset_after(&self) -> Duration {
        self.reset_after
    }
}

impl fmt::Debug for GrokOfficialRateLimitWindow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialRateLimitWindow")
            .field("kind", &self.kind)
            .field("limit_reported", &true)
            .field("remaining_reported", &true)
            .field("reset_reported", &true)
            .finish_non_exhaustive()
    }
}

/// Safe header-derived Official rate-limit observation.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct GrokOfficialRateLimitMetadata {
    windows: Vec<GrokOfficialRateLimitWindow>,
    retry_after: Option<Duration>,
}

impl GrokOfficialRateLimitMetadata {
    /// Parses the fixed allow-list of Official rate-limit response headers.
    ///
    /// Unknown headers are not retained or interpreted. Each known request/token resource must be
    /// absent as a whole or have an exact one-each limit/remaining/reset triplet. Header names are
    /// case-insensitive; a duplicate recognized name is ambiguous and fails closed.
    ///
    /// # Errors
    ///
    /// Returns `UpstreamProtocolError/Provider` for duplicate, partial, malformed, unsafe,
    /// impossible, zero/over-limit reset, or unsupported known-header evidence.
    pub fn parse<'a>(
        headers: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Self, GatewayError> {
        let mut known = BTreeMap::new();
        for (name, value) in headers {
            let lower_name = name.to_ascii_lowercase();
            if !is_known_header(&lower_name) {
                continue;
            }
            if known.insert(lower_name, value).is_some() {
                return Err(provider_protocol_error());
            }
        }

        let mut windows = Vec::new();
        for (kind, limit, remaining, reset) in [
            (
                GrokOfficialRateLimitKind::Requests,
                REQUEST_LIMIT,
                REQUEST_REMAINING,
                REQUEST_RESET,
            ),
            (
                GrokOfficialRateLimitKind::Tokens,
                TOKEN_LIMIT,
                TOKEN_REMAINING,
                TOKEN_RESET,
            ),
        ] {
            let values = (known.get(limit), known.get(remaining), known.get(reset));
            match values {
                (None, None, None) => {}
                (Some(limit), Some(remaining), Some(reset)) => {
                    windows.push(GrokOfficialRateLimitWindow::try_new(
                        kind,
                        parse_count(limit)?,
                        parse_count(remaining)?,
                        parse_duration(reset)?,
                    )?);
                }
                _ => return Err(provider_protocol_error()),
            }
        }
        let retry_after = known.get(RETRY_AFTER).map(parse_retry_after).transpose()?;
        Ok(Self {
            windows,
            retry_after,
        })
    }

    /// Returns complete fixed-kind windows in Requests-then-Tokens order.
    #[must_use]
    pub fn windows(&self) -> &[GrokOfficialRateLimitWindow] {
        &self.windows
    }

    /// Returns an explicit `Retry-After` delay when the header carried a bounded delta-seconds
    /// value. A date form is intentionally not converted without a caller-supplied clock.
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }

    /// Returns whether no recognized upstream rate-limit evidence was present.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.windows.is_empty() && self.retry_after.is_none()
    }
}

impl fmt::Debug for GrokOfficialRateLimitMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialRateLimitMetadata")
            .field("window_count", &self.windows.len())
            .field("retry_after_reported", &self.retry_after.is_some())
            .finish()
    }
}

/// Safe direct-usage metadata that can inform later Official billing state.
///
/// It deliberately reports token counters only. A rate-limit Header does not establish a billing
/// plan, account balance, price, currency, or charge, so none is invented here.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct GrokOfficialBillingMetadata {
    input: Option<u64>,
    output: Option<u64>,
    reasoning: Option<u64>,
    cached: Option<u64>,
}

impl GrokOfficialBillingMetadata {
    /// Projects provider-reported Canonical usage without estimating prices or token values.
    #[must_use]
    pub const fn from_usage(usage: &Usage) -> Self {
        Self {
            input: usage.input_tokens,
            output: usage.output_tokens,
            reasoning: usage.reasoning_tokens,
            cached: usage.cached_tokens,
        }
    }

    /// Returns the reported input-token count, if any.
    #[must_use]
    pub const fn input_tokens(&self) -> Option<u64> {
        self.input
    }

    /// Returns the reported output-token count, if any.
    #[must_use]
    pub const fn output_tokens(&self) -> Option<u64> {
        self.output
    }

    /// Returns the reported reasoning-token count, if any.
    #[must_use]
    pub const fn reasoning_tokens(&self) -> Option<u64> {
        self.reasoning
    }

    /// Returns the reported cached-token count, if any.
    #[must_use]
    pub const fn cached_tokens(&self) -> Option<u64> {
        self.cached
    }

    /// Returns whether no provider-reported token counter was available.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.input.is_none()
            && self.output.is_none()
            && self.reasoning.is_none()
            && self.cached.is_none()
    }
}

impl fmt::Debug for GrokOfficialBillingMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokOfficialBillingMetadata")
            .field("input_tokens_reported", &self.input.is_some())
            .field("output_tokens_reported", &self.output.is_some())
            .field("reasoning_tokens_reported", &self.reasoning.is_some())
            .field("cached_tokens_reported", &self.cached.is_some())
            .finish()
    }
}

fn is_known_header(name: &str) -> bool {
    matches!(
        name,
        REQUEST_LIMIT
            | REQUEST_REMAINING
            | REQUEST_RESET
            | TOKEN_LIMIT
            | TOKEN_REMAINING
            | TOKEN_RESET
            | RETRY_AFTER
    )
}

fn parse_count(value: &str) -> Result<u64, GatewayError> {
    if value.is_empty()
        || value.len() > MAX_GROK_OFFICIAL_RATE_LIMIT_VALUE_BYTES
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(provider_protocol_error());
    }
    value.parse::<u64>().map_err(|_| provider_protocol_error())
}

fn parse_duration(value: &str) -> Result<Duration, GatewayError> {
    let Some((number, suffix)) = split_duration(value) else {
        return Err(provider_protocol_error());
    };
    let value = parse_count(number)?;
    let milliseconds = match suffix {
        "ms" => Some(value),
        "s" => value.checked_mul(1_000),
        "m" => value.checked_mul(60_000),
        "h" => value.checked_mul(3_600_000),
        _ => None,
    }
    .ok_or_else(provider_protocol_error)?;
    let duration = Duration::from_millis(milliseconds);
    if duration.is_zero() || duration > MAX_GROK_OFFICIAL_RATE_LIMIT_RESET {
        return Err(provider_protocol_error());
    }
    Ok(duration)
}

fn split_duration(value: &str) -> Option<(&str, &str)> {
    if value.len() > MAX_GROK_OFFICIAL_RATE_LIMIT_VALUE_BYTES || value.is_empty() {
        return None;
    }
    for suffix in ["ms", "s", "m", "h"] {
        if let Some(number) = value.strip_suffix(suffix) {
            return Some((number, suffix));
        }
    }
    None
}

fn parse_retry_after(value: &&str) -> Result<Duration, GatewayError> {
    let seconds = parse_count(value)?;
    let duration = Duration::from_secs(seconds);
    if duration > MAX_GROK_OFFICIAL_RATE_LIMIT_RESET {
        return Err(provider_protocol_error());
    }
    Ok(duration)
}

const fn provider_protocol_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        ErrorScope::Provider,
    )
}
