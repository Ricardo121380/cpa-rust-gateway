//! Fixture-only Grok Web REST and gRPC-Web quota observations.
//!
//! The module keeps source-labelled quota projections isolated to one exact browser egress
//! session. It decodes bounded synthetic fixtures only; it does not claim a live Web response
//! grammar, query a quota endpoint, or change scheduling/account state.

use std::{collections::BTreeMap, error::Error, fmt};

use serde_json::{Map, Value};

use crate::{GrokWebBrowserEgressSession, strict_json::parse_strict_json};

/// Maximum bytes accepted for one local REST or gRPC-Web quota fixture.
pub const MAX_GROK_WEB_QUOTA_FIXTURE_BYTES: usize = 64 * 1024;

const MAX_TIER_BYTES: usize = 128;
const MAX_RAW_WINDOW_TYPE_BYTES: usize = 128;

/// The Web protocol surface that supplied one quota observation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GrokWebQuotaSource {
    /// A Web REST fixture supplied this observation.
    Rest,
    /// A Web gRPC-Web fixture supplied this observation.
    GrpcWeb,
}

/// Confidence attached to a Web quota observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebQuotaConfidence {
    /// The Web surface reported a value observed by this provider; it is not billing authority.
    Observed,
}

/// The coarse duration category of one Web quota window.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GrokWebQuotaWindowKind {
    /// A provider-reported hourly window.
    Hourly,
    /// A provider-reported daily window.
    Daily,
    /// A provider-reported weekly window.
    Weekly,
    /// A provider-reported monthly window.
    Monthly,
    /// An explicit provider-defined duration retained with its raw window type.
    ProviderDefined,
}

impl GrokWebQuotaWindowKind {
    fn parse(value: &str) -> Result<Self, GrokWebQuotaError> {
        match value {
            "hourly" => Ok(Self::Hourly),
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            "provider_defined" => Ok(Self::ProviderDefined),
            _ => Err(GrokWebQuotaError::InvalidQuotaFixture),
        }
    }
}

/// An opaque provider-reported Web subscription tier label.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct GrokWebQuotaTier(String);

impl GrokWebQuotaTier {
    /// Validates an opaque tier label without assigning unsupported billing semantics.
    ///
    /// # Errors
    ///
    /// Returns a safe category without retaining an invalid label.
    pub fn try_new(value: &str) -> Result<Self, GrokWebQuotaError> {
        validate_opaque(value, MAX_TIER_BYTES).map_err(|()| GrokWebQuotaError::InvalidTier)?;
        Ok(Self(value.to_owned()))
    }

    /// Returns the opaque tier label for a later explicit management projection.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GrokWebQuotaTier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokWebQuotaTier(<redacted>)")
    }
}

/// One source-labelled and reset-aware Web quota window.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebQuotaWindow {
    tier: GrokWebQuotaTier,
    kind: GrokWebQuotaWindowKind,
    raw_window_type: String,
    remaining: u64,
    total: u64,
    window_seconds: u64,
    reset_at_ms: i64,
    observed_at_ms: i64,
    source: GrokWebQuotaSource,
    confidence: GrokWebQuotaConfidence,
}

impl GrokWebQuotaWindow {
    /// Returns the opaque provider-reported tier.
    #[must_use]
    pub const fn tier(&self) -> &GrokWebQuotaTier {
        &self.tier
    }

    /// Returns the distinct coarse window kind.
    #[must_use]
    pub const fn kind(&self) -> GrokWebQuotaWindowKind {
        self.kind
    }

    /// Returns remaining capacity in the provider-declared window unit.
    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.remaining
    }

    /// Returns total capacity in the provider-declared window unit.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.total
    }

    /// Returns the declared window duration.
    #[must_use]
    pub const fn window_seconds(&self) -> u64 {
        self.window_seconds
    }

    /// Returns the source reset instant.
    #[must_use]
    pub const fn reset_at_ms(&self) -> i64 {
        self.reset_at_ms
    }

    /// Returns the source observation instant.
    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    /// Returns whether REST or gRPC-Web supplied this exact window.
    #[must_use]
    pub const fn source(&self) -> GrokWebQuotaSource {
        self.source
    }

    /// Returns the fixed meaning of this local source projection.
    #[must_use]
    pub const fn confidence(&self) -> GrokWebQuotaConfidence {
        self.confidence
    }
}

impl fmt::Debug for GrokWebQuotaWindow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebQuotaWindow")
            .field("tier", &self.tier)
            .field("kind", &self.kind)
            .field("raw_window_type", &"<redacted>")
            .field("remaining", &self.remaining)
            .field("total", &self.total)
            .field("window_seconds", &self.window_seconds)
            .field("reset_at_ms", &self.reset_at_ms)
            .field("observed_at_ms", &self.observed_at_ms)
            .field("source", &self.source)
            .field("confidence", &self.confidence)
            .finish()
    }
}

/// Stateless decoder for deliberately synthetic local quota fixtures.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokWebQuotaFixtureDecoder;

impl GrokWebQuotaFixtureDecoder {
    /// Decodes the P9-06 REST fixture shape: `{tier, window}`.
    ///
    /// # Errors
    ///
    /// Returns a safe value-free error for malformed, duplicate, unknown, oversized, or invalid
    /// quota fields. It does not interpret a live REST response.
    pub fn decode_rest_fixture(input: &[u8]) -> Result<GrokWebQuotaWindow, GrokWebQuotaError> {
        decode_fixture(input, GrokWebQuotaSource::Rest, false)
    }

    /// Decodes the P9-06 gRPC-Web fixture shape: `{quota: {tier, window}}`.
    ///
    /// # Errors
    ///
    /// Returns a safe value-free error for malformed, duplicate, unknown, oversized, or invalid
    /// quota fields. It does not interpret a live gRPC-Web frame.
    pub fn decode_grpc_web_fixture(input: &[u8]) -> Result<GrokWebQuotaWindow, GrokWebQuotaError> {
        decode_fixture(input, GrokWebQuotaSource::GrpcWeb, true)
    }
}

/// One in-memory exact-session Web quota projection.
///
/// REST and gRPC-Web values are stored independently by `(source, window kind)`. A later source
/// observation cannot overwrite another source's quota claim.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokWebQuotaState {
    account_reference: String,
    lineage_reference: String,
    credential_revision: u64,
    credential_expires_at_ms: i64,
    egress_session_id: String,
    windows: BTreeMap<(GrokWebQuotaSource, GrokWebQuotaWindowKind), GrokWebQuotaWindow>,
}

impl GrokWebQuotaState {
    /// Creates empty Web quota state bound to one unexpired egress-session fingerprint.
    ///
    /// # Errors
    ///
    /// Returns a safe category for invalid observation time or expired egress session.
    pub fn try_new(
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<Self, GrokWebQuotaError> {
        if now_ms < 0 {
            return Err(GrokWebQuotaError::InvalidObservationTime);
        }
        if session.is_expired_at(now_ms) {
            return Err(GrokWebQuotaError::ExpiredEgressSession);
        }
        Ok(Self {
            account_reference: session.account_reference().to_owned(),
            lineage_reference: session.lineage_reference().to_owned(),
            credential_revision: session.credential_revision(),
            credential_expires_at_ms: session.credential_expires_at_ms(),
            egress_session_id: session.egress_session_id().as_str().to_owned(),
            windows: BTreeMap::new(),
        })
    }

    /// Applies a newer source-labelled quota observation after exact session validation.
    ///
    /// # Errors
    ///
    /// Returns a safe binding/expiry/category error. A same-time conflicting window is rejected
    /// without mutation; an older observation is retained as `IgnoredStale`.
    pub fn sync(
        &mut self,
        session: &GrokWebBrowserEgressSession,
        window: GrokWebQuotaWindow,
        now_ms: i64,
    ) -> Result<GrokWebQuotaSyncOutcome, GrokWebQuotaError> {
        self.require_current_session(session, now_ms)?;
        let key = (window.source, window.kind);
        if let Some(current) = self.windows.get(&key) {
            if current.observed_at_ms > window.observed_at_ms {
                return Ok(GrokWebQuotaSyncOutcome::IgnoredStale);
            }
            if current.observed_at_ms == window.observed_at_ms {
                if current == &window {
                    return Ok(GrokWebQuotaSyncOutcome::IgnoredStale);
                }
                return Err(GrokWebQuotaError::ConflictingObservation);
            }
        }
        self.windows.insert(key, window);
        Ok(GrokWebQuotaSyncOutcome::Applied)
    }

    /// Returns one locally retained source/window snapshot without inferring cross-source truth.
    #[must_use]
    pub fn window(
        &self,
        source: GrokWebQuotaSource,
        kind: GrokWebQuotaWindowKind,
    ) -> Option<&GrokWebQuotaWindow> {
        self.windows.get(&(source, kind))
    }

    fn require_current_session(
        &self,
        session: &GrokWebBrowserEgressSession,
        now_ms: i64,
    ) -> Result<(), GrokWebQuotaError> {
        if now_ms < 0 {
            return Err(GrokWebQuotaError::InvalidObservationTime);
        }
        if now_ms >= self.credential_expires_at_ms || session.is_expired_at(now_ms) {
            return Err(GrokWebQuotaError::ExpiredEgressSession);
        }
        if self.account_reference != session.account_reference()
            || self.lineage_reference != session.lineage_reference()
            || self.credential_revision != session.credential_revision()
            || self.credential_expires_at_ms != session.credential_expires_at_ms()
            || self.egress_session_id != session.egress_session_id().as_str()
        {
            return Err(GrokWebQuotaError::SessionBindingMismatch);
        }
        Ok(())
    }
}

impl fmt::Debug for GrokWebQuotaState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokWebQuotaState")
            .field("account_reference", &"<redacted>")
            .field("lineage_reference", &"<redacted>")
            .field("credential_revision", &self.credential_revision)
            .field("credential_expires_at_ms", &self.credential_expires_at_ms)
            .field("egress_session_id", &"<redacted>")
            .field("window_count", &self.windows.len())
            .finish()
    }
}

/// Safe outcome from synchronizing one local Web quota observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebQuotaSyncOutcome {
    /// The newer exact source/window snapshot was applied.
    Applied,
    /// A same or newer source/window snapshot was already retained.
    IgnoredStale,
}

/// Safe quota fixture, lifecycle, or exact-session failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokWebQuotaError {
    /// Fixture JSON or its fixed schema was invalid.
    InvalidQuotaFixture,
    /// Provider-reported tier label was invalid.
    InvalidTier,
    /// Supplied state time was negative.
    InvalidObservationTime,
    /// Browser egress session is expired.
    ExpiredEgressSession,
    /// An update came from a different account/lineage/revision/expiry/egress session.
    SessionBindingMismatch,
    /// Two distinct source/window values claimed the exact same observation instant.
    ConflictingObservation,
}

impl fmt::Display for GrokWebQuotaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidQuotaFixture => "Grok Web quota fixture is invalid",
            Self::InvalidTier => "Grok Web quota tier is invalid",
            Self::InvalidObservationTime => "Grok Web quota observation time is invalid",
            Self::ExpiredEgressSession => "Grok Web quota egress session is expired",
            Self::SessionBindingMismatch => "Grok Web quota session binding does not match",
            Self::ConflictingObservation => "Grok Web quota observation conflicts",
        })
    }
}

impl Error for GrokWebQuotaError {}

fn decode_fixture(
    input: &[u8],
    source: GrokWebQuotaSource,
    grpc_wrapper: bool,
) -> Result<GrokWebQuotaWindow, GrokWebQuotaError> {
    let value = parse_strict_json(input, MAX_GROK_WEB_QUOTA_FIXTURE_BYTES)
        .map_err(|()| GrokWebQuotaError::InvalidQuotaFixture)?;
    let root = value
        .as_object()
        .ok_or(GrokWebQuotaError::InvalidQuotaFixture)?;
    let body = if grpc_wrapper {
        ensure_fields(root, &["quota"])?;
        required_object(root, "quota")?
    } else {
        root
    };
    ensure_fields(body, &["tier", "window"])?;
    let tier = GrokWebQuotaTier::try_new(required_string(body, "tier")?)?;
    let window = required_object(body, "window")?;
    ensure_fields(
        window,
        &[
            "kind",
            "raw_type",
            "remaining",
            "total",
            "window_seconds",
            "reset_at_ms",
            "observed_at_ms",
        ],
    )?;
    let kind = GrokWebQuotaWindowKind::parse(required_string(window, "kind")?)?;
    let raw_window_type = required_string(window, "raw_type")?;
    validate_opaque(raw_window_type, MAX_RAW_WINDOW_TYPE_BYTES)
        .map_err(|()| GrokWebQuotaError::InvalidQuotaFixture)?;
    let remaining = required_u64(window, "remaining")?;
    let total = required_u64(window, "total")?;
    let window_seconds = required_u64(window, "window_seconds")?;
    let reset_at_ms = required_i64(window, "reset_at_ms")?;
    let observed_at_ms = required_i64(window, "observed_at_ms")?;
    if total == 0
        || remaining > total
        || window_seconds == 0
        || observed_at_ms < 0
        || reset_at_ms <= observed_at_ms
    {
        return Err(GrokWebQuotaError::InvalidQuotaFixture);
    }
    Ok(GrokWebQuotaWindow {
        tier,
        kind,
        raw_window_type: raw_window_type.to_owned(),
        remaining,
        total,
        window_seconds,
        reset_at_ms,
        observed_at_ms,
        source,
        confidence: GrokWebQuotaConfidence::Observed,
    })
}

fn ensure_fields(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), GrokWebQuotaError> {
    if object.len() != allowed.len() || allowed.iter().any(|field| !object.contains_key(*field)) {
        return Err(GrokWebQuotaError::InvalidQuotaFixture);
    }
    Ok(())
}

fn required_object<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, GrokWebQuotaError> {
    object
        .get(field)
        .and_then(Value::as_object)
        .ok_or(GrokWebQuotaError::InvalidQuotaFixture)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, GrokWebQuotaError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(GrokWebQuotaError::InvalidQuotaFixture)
}

fn required_u64(object: &Map<String, Value>, field: &str) -> Result<u64, GrokWebQuotaError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(GrokWebQuotaError::InvalidQuotaFixture)
}

fn required_i64(object: &Map<String, Value>, field: &str) -> Result<i64, GrokWebQuotaError> {
    object
        .get(field)
        .and_then(Value::as_i64)
        .ok_or(GrokWebQuotaError::InvalidQuotaFixture)
}

fn validate_opaque(value: &str, maximum_bytes: usize) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(());
    }
    Ok(())
}
