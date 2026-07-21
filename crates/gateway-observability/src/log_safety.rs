//! Default-deny HTTP log records with bounded, explicitly sampled JSON bodies.
//!
//! This module never receives a `GatewayEvent` queue or a request-path sink. Callers construct a
//! [`SanitizedHttpLogRecord`] only from an already-owned background observation path. With the
//! default policy, it retains no body bytes and no header values. Explicit sampling remains
//! bounded, JSON-only, and redacts sensitive field names plus recognizable secret-like strings.

use std::fmt;

use gateway_core::RequestId;
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::OpenTelemetryExportOutcome;

/// Maximum byte length that one explicitly enabled JSON body sample may parse.
pub const MAX_BODY_SAMPLE_BYTES: usize = 16 * 1024;
/// Stable replacement used instead of a sensitive body value or object key.
pub const REDACTED_LOG_VALUE: &str = "[REDACTED]";
/// Schema version for [`SanitizedHttpLogRecord`] JSON output.
pub const HTTP_LOG_SCHEMA_VERSION: u8 = 1;

/// A bounded deterministic sampling configuration for optional body logging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodySamplingPolicy {
    numerator: u32,
    denominator: u32,
    max_bytes: usize,
}

impl BodySamplingPolicy {
    /// Returns the default deny-by-default policy, which retains no body bytes.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            numerator: 0,
            denominator: 1,
            max_bytes: 0,
        }
    }

    /// Creates an explicitly enabled deterministic sample policy.
    ///
    /// A request is selected when its stable `RequestId` bucket is lower than `numerator`; no
    /// Client Key, Credential, Endpoint, URL, header, or body value contributes to sampling.
    ///
    /// # Errors
    ///
    /// Returns a configuration error before any log body can be retained when the ratio or byte
    /// bound is invalid.
    pub const fn try_sampled(
        numerator: u32,
        denominator: u32,
        max_bytes: usize,
    ) -> Result<Self, BodySamplingPolicyError> {
        if denominator == 0 {
            return Err(BodySamplingPolicyError::ZeroDenominator);
        }
        if numerator == 0 {
            return Err(BodySamplingPolicyError::ZeroNumerator);
        }
        if numerator > denominator {
            return Err(BodySamplingPolicyError::NumeratorExceedsDenominator);
        }
        if max_bytes == 0 {
            return Err(BodySamplingPolicyError::ZeroMaxBytes);
        }
        if max_bytes > MAX_BODY_SAMPLE_BYTES {
            return Err(BodySamplingPolicyError::MaxBytesTooLarge);
        }
        Ok(Self {
            numerator,
            denominator,
            max_bytes,
        })
    }

    /// Returns whether this policy retains any body sample at all.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        self.numerator != 0
    }

    /// Returns the configured maximum bytes when sampling is enabled.
    #[must_use]
    pub const fn max_bytes(self) -> Option<usize> {
        if self.is_enabled() {
            Some(self.max_bytes)
        } else {
            None
        }
    }

    /// Returns the deterministic selection result for one request.
    #[must_use]
    pub fn selects(self, request_id: &RequestId) -> bool {
        if !self.is_enabled() {
            return false;
        }
        let mut hasher = Sha256::new();
        hasher.update(b"cpa-rust-gateway/body-sampling/v1\0");
        hasher.update(request_id.as_str().as_bytes());
        let digest = hasher.finalize();
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(&digest[..8]);
        let bucket = u64::from_be_bytes(bytes) % u64::from(self.denominator);
        bucket < u64::from(self.numerator)
    }
}

impl Default for BodySamplingPolicy {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Invalid settings for an explicit [`BodySamplingPolicy`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodySamplingPolicyError {
    /// A sample denominator must be positive.
    ZeroDenominator,
    /// An enabled sample numerator must be positive.
    ZeroNumerator,
    /// A sample numerator cannot exceed its denominator.
    NumeratorExceedsDenominator,
    /// An enabled body sample needs a finite positive byte limit.
    ZeroMaxBytes,
    /// An enabled body sample exceeds [`MAX_BODY_SAMPLE_BYTES`].
    MaxBytesTooLarge,
}

impl fmt::Display for BodySamplingPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDenominator => {
                formatter.write_str("body sample denominator must be positive")
            }
            Self::ZeroNumerator => formatter.write_str("body sample numerator must be positive"),
            Self::NumeratorExceedsDenominator => {
                formatter.write_str("body sample numerator cannot exceed its denominator")
            }
            Self::ZeroMaxBytes => formatter.write_str("body sample maximum bytes must be positive"),
            Self::MaxBytesTooLarge => {
                formatter.write_str("body sample maximum bytes exceeds the finite limit")
            }
        }
    }
}

impl std::error::Error for BodySamplingPolicyError {}

/// The sole P4-09 configuration for a sanitized HTTP log record.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LogRedactionPolicy {
    body_sampling: BodySamplingPolicy,
}

impl LogRedactionPolicy {
    /// Creates one policy with the supplied explicit body sampling choice.
    #[must_use]
    pub const fn new(body_sampling: BodySamplingPolicy) -> Self {
        Self { body_sampling }
    }

    /// Returns the configured body sampling choice.
    #[must_use]
    pub const fn body_sampling(self) -> BodySamplingPolicy {
        self.body_sampling
    }

    /// Builds a serialized-safe HTTP log record without retaining raw headers or raw body bytes.
    pub fn capture_http_record<'a>(
        self,
        request_id: &RequestId,
        direction: HttpLogDirection,
        status: Option<u16>,
        headers: impl IntoIterator<Item = (&'a str, &'a str)>,
        body: &[u8],
    ) -> SanitizedHttpLogRecord {
        let headers = SanitizedHeaderSummary::from_headers(headers);
        let body = capture_body(self.body_sampling, request_id, headers.is_json(), body);
        SanitizedHttpLogRecord {
            schema_version: HTTP_LOG_SCHEMA_VERSION,
            request_id: request_id.as_str().to_owned(),
            direction,
            status,
            headers,
            body,
        }
    }
}

/// Stable context for one safe HTTP log record without a target, URL, or credential identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpLogDirection {
    /// One client request accepted by the gateway.
    InboundRequest,
    /// One request sent from the gateway to an already-selected upstream.
    UpstreamRequest,
    /// One response received from an upstream.
    UpstreamResponse,
    /// One response returned from the gateway to its client.
    DownstreamResponse,
}

/// One safe summary of a recognized content type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoggedContentType {
    /// Exactly `application/json`, with optional parameters omitted.
    Json,
    /// An `application/*+json` vendor media type, with its vendor value omitted.
    JsonSuffix,
    /// Exactly `text/event-stream`, with optional parameters omitted.
    EventStream,
}

/// Summary of headers that keeps only fixed safe metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SanitizedHeaderSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<LoggedContentType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_length: Option<u64>,
    redacted_header_count: u16,
    omitted_header_count: u16,
}

impl SanitizedHeaderSummary {
    fn from_headers<'a>(headers: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut summary = Self::default();
        let mut content_type_seen = false;
        let mut content_length_seen = false;
        for (name, value) in headers {
            match name.to_ascii_lowercase().as_str() {
                "content-type" if content_type_seen => {
                    summary.content_type = None;
                    increment_u16(&mut summary.redacted_header_count);
                }
                "content-type" => {
                    content_type_seen = true;
                    match recognized_content_type(value) {
                        Some(content_type) => summary.content_type = Some(content_type),
                        None => increment_u16(&mut summary.redacted_header_count),
                    }
                }
                "content-length" if content_length_seen => {
                    summary.content_length = None;
                    increment_u16(&mut summary.redacted_header_count);
                }
                "content-length" => {
                    content_length_seen = true;
                    match value.parse::<u64>() {
                        Ok(content_length) => summary.content_length = Some(content_length),
                        Err(_) => increment_u16(&mut summary.redacted_header_count),
                    }
                }
                sensitive_header_name => {
                    if is_sensitive_header_name(sensitive_header_name) {
                        increment_u16(&mut summary.redacted_header_count);
                    } else {
                        increment_u16(&mut summary.omitted_header_count);
                    }
                }
            }
        }
        summary
    }

    fn is_json(&self) -> bool {
        matches!(
            self.content_type,
            Some(LoggedContentType::Json | LoggedContentType::JsonSuffix)
        )
    }

    /// Returns the fixed safe content type classification when one was recognized.
    #[must_use]
    pub const fn content_type(&self) -> Option<LoggedContentType> {
        self.content_type
    }

    /// Returns the parsed `Content-Length` only when it was a valid unsigned integer.
    #[must_use]
    pub const fn content_length(&self) -> Option<u64> {
        self.content_length
    }

    /// Returns how many sensitive or malformed header values were redacted.
    #[must_use]
    pub const fn redacted_header_count(&self) -> u16 {
        self.redacted_header_count
    }

    /// Returns how many non-allowlisted headers were omitted without retaining their names or values.
    #[must_use]
    pub const fn omitted_header_count(&self) -> u16 {
        self.omitted_header_count
    }
}

/// Reason a body had no log sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyOmissionReason {
    /// The default policy does not retain any body bytes.
    Disabled,
    /// The explicit bounded sampling policy did not select this request.
    NotSelected,
    /// The supplied body did not have an allowlisted JSON content type.
    NotJson,
    /// The supplied body exceeded the finite sample maximum before parsing.
    TooLarge,
    /// The supplied body was not valid UTF-8.
    InvalidUtf8,
    /// The supplied JSON body could not be parsed safely.
    MalformedJson,
}

/// A body projection that is either absent for one safe reason or recursively redacted JSON.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SanitizedBodySample {
    /// No body content was retained.
    Omitted {
        /// The safe reason no content was retained.
        reason: BodyOmissionReason,
    },
    /// One bounded JSON body was retained only after recursive redaction.
    Json {
        /// The recursively redacted JSON projection.
        value: Value,
    },
}

impl SanitizedBodySample {
    /// Returns the omission reason when this record retained no JSON sample.
    #[must_use]
    pub const fn omission_reason(&self) -> Option<BodyOmissionReason> {
        match self {
            Self::Omitted { reason } => Some(*reason),
            Self::Json { .. } => None,
        }
    }

    /// Returns the recursively redacted JSON only when an explicit sample was retained.
    #[must_use]
    pub const fn json(&self) -> Option<&Value> {
        match self {
            Self::Omitted { .. } => None,
            Self::Json { value } => Some(value),
        }
    }
}

/// One serializable HTTP record that cannot retain a raw header or raw body byte sequence.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SanitizedHttpLogRecord {
    schema_version: u8,
    request_id: String,
    direction: HttpLogDirection,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    headers: SanitizedHeaderSummary,
    body: SanitizedBodySample,
}

impl SanitizedHttpLogRecord {
    /// Returns the stable log schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u8 {
        self.schema_version
    }

    /// Returns the safe request correlation identifier.
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Returns the fixed safe traffic direction.
    #[must_use]
    pub const fn direction(&self) -> HttpLogDirection {
        self.direction
    }

    /// Returns the HTTP status when this record describes a response.
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        self.status
    }

    /// Returns the safe header summary.
    #[must_use]
    pub const fn headers(&self) -> &SanitizedHeaderSummary {
        &self.headers
    }

    /// Returns the body omission or redacted JSON projection.
    #[must_use]
    pub const fn body(&self) -> &SanitizedBodySample {
        &self.body
    }

    /// Serializes one safe JSON log line.
    ///
    /// # Errors
    ///
    /// Returns a serialization error without returning a partial line.
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Emits an already-sanitized HTTP log record through the configured `tracing` subscriber.
///
/// Call this only from the same background observation boundary that owns the record. The supplied
/// record has already applied the default-deny body policy and header summary policy.
#[must_use]
pub fn try_emit_sanitized_http_log(record: &SanitizedHttpLogRecord) -> OpenTelemetryExportOutcome {
    let Ok(payload) = record.to_json_line() else {
        return OpenTelemetryExportOutcome::Rejected;
    };
    tracing::info!(
        target: "gateway_observability::http_json",
        schema_version = record.schema_version(),
        request_id = record.request_id(),
        direction = ?record.direction(),
        status = ?record.status(),
        http_log_json = %payload,
        "gateway sanitized HTTP record"
    );
    OpenTelemetryExportOutcome::Emitted
}

fn capture_body(
    policy: BodySamplingPolicy,
    request_id: &RequestId,
    is_json: bool,
    body: &[u8],
) -> SanitizedBodySample {
    if !policy.is_enabled() {
        return omitted(BodyOmissionReason::Disabled);
    }
    if !policy.selects(request_id) {
        return omitted(BodyOmissionReason::NotSelected);
    }
    if !is_json {
        return omitted(BodyOmissionReason::NotJson);
    }
    if body.len() > policy.max_bytes {
        return omitted(BodyOmissionReason::TooLarge);
    }
    if std::str::from_utf8(body).is_err() {
        return omitted(BodyOmissionReason::InvalidUtf8);
    }
    let Ok(mut value) = serde_json::from_slice::<Value>(body) else {
        return omitted(BodyOmissionReason::MalformedJson);
    };
    redact_json_value(&mut value);
    SanitizedBodySample::Json { value }
}

const fn omitted(reason: BodyOmissionReason) -> SanitizedBodySample {
    SanitizedBodySample::Omitted { reason }
}

fn recognized_content_type(value: &str) -> Option<LoggedContentType> {
    let media_type = value
        .split_once(';')
        .map_or(value, |(media_type, _)| media_type)
        .trim()
        .to_ascii_lowercase();
    match media_type.as_str() {
        "application/json" => Some(LoggedContentType::Json),
        "text/event-stream" => Some(LoggedContentType::EventStream),
        _ if media_type.starts_with("application/") && media_type.ends_with("+json") => {
            Some(LoggedContentType::JsonSuffix)
        }
        _ => None,
    }
}

fn is_sensitive_header_name(name: &str) -> bool {
    matches!(
        name,
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "set-cookie"
            | "x-api-key"
            | "api-key"
            | "x-auth-token"
            | "x-access-token"
    )
}

fn increment_u16(counter: &mut u16) {
    *counter = counter.saturating_add(1);
}

fn redact_json_value(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                redact_json_value(value);
            }
        }
        Value::Object(values) => {
            let original = std::mem::take(values);
            let mut redacted = Map::with_capacity(original.len());
            for (index, (key, mut nested_value)) in original.into_iter().enumerate() {
                let sensitive_key = is_sensitive_json_key(&key);
                let safe_key = if sensitive_key || is_secret_like_text(&key) {
                    format!("[REDACTED_KEY_{index}]")
                } else {
                    key
                };
                if sensitive_key {
                    nested_value = Value::String(REDACTED_LOG_VALUE.to_owned());
                } else {
                    redact_json_value(&mut nested_value);
                }
                redacted.insert(safe_key, nested_value);
            }
            *values = redacted;
        }
        Value::String(text) if is_secret_like_text(text) => {
            REDACTED_LOG_VALUE.clone_into(text);
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn is_sensitive_json_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect();
    matches!(
        normalized.as_str(),
        "authorization"
            | "apikey"
            | "accesstoken"
            | "refreshtoken"
            | "clientsecret"
            | "password"
            | "cookie"
            | "credential"
            | "token"
            | "secret"
    ) || normalized.contains("token")
        || normalized.contains("key")
        || normalized.contains("secret")
        || normalized.contains("password")
}

fn is_secret_like_text(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "authorization",
        "bearer",
        "api_key",
        "api-key",
        "api key",
        "apikey",
        "access_token",
        "access token",
        "refresh_token",
        "refresh token",
        "client_secret",
        "client secret",
        "password",
        "token",
        "secret",
        "cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || value.contains("sk-")
        || value.contains("ghp_")
        || value.contains("github_pat_")
        || value.contains("AIza")
        || value.contains("AKIA")
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::RequestId;

    use super::{
        BodyOmissionReason, BodySamplingPolicy, HttpLogDirection, LogRedactionPolicy,
        LoggedContentType, REDACTED_LOG_VALUE, SanitizedBodySample,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn default_policy_retains_no_body_or_header_values() -> TestResult {
        let request_id = RequestId::try_new("request-log-default")?;
        let secret = "header-secret-must-not-appear";
        let record = LogRedactionPolicy::default().capture_http_record(
            &request_id,
            HttpLogDirection::InboundRequest,
            None,
            [
                ("Content-Type", "application/json"),
                ("Authorization", secret),
                ("X-Custom-Header", "untrusted-value-must-not-appear"),
            ],
            br#"{"api_key":"body-secret-must-not-appear"}"#,
        );
        let serialized = record.to_json_line()?;
        let debug = format!("{record:?}");

        assert_eq!(
            record.body().omission_reason(),
            Some(BodyOmissionReason::Disabled)
        );
        assert_eq!(
            record.headers().content_type(),
            Some(LoggedContentType::Json)
        );
        assert_eq!(record.headers().redacted_header_count(), 1);
        assert_eq!(record.headers().omitted_header_count(), 1);
        for forbidden in [
            secret,
            "untrusted-value-must-not-appear",
            "body-secret-must-not-appear",
        ] {
            assert!(!serialized.contains(forbidden));
            assert!(!debug.contains(forbidden));
        }
        Ok(())
    }

    #[test]
    fn explicit_json_sampling_is_bounded_and_recursively_redacts_secrets() -> TestResult {
        let request_id = RequestId::try_new("request-log-sampled")?;
        let policy = LogRedactionPolicy::new(BodySamplingPolicy::try_sampled(1, 1, 1_024)?);
        let body = br#"{
            "message":"ordinary visible sample text",
            "api_key":"body-secret-must-not-appear",
            "private_key":"private-key-must-not-appear",
            "nested":{"access_token":"nested-secret-must-not-appear"},
            "array":["Bearer inline-secret-must-not-appear"]
        }"#;
        let record = policy.capture_http_record(
            &request_id,
            HttpLogDirection::UpstreamRequest,
            None,
            [
                (
                    "Content-Type",
                    "application/vnd.gateway+json; charset=utf-8",
                ),
                ("Content-Length", "221"),
                ("X-Api-Key", "header-secret-must-not-appear"),
                ("X-Trace-Private", "untrusted-header-must-not-appear"),
            ],
            body,
        );
        let serialized = record.to_json_line()?;
        let debug = format!("{record:?}");

        assert!(matches!(record.body(), SanitizedBodySample::Json { .. }));
        assert_eq!(
            record.headers().content_type(),
            Some(LoggedContentType::JsonSuffix)
        );
        assert_eq!(record.headers().content_length(), Some(221));
        assert_eq!(record.headers().redacted_header_count(), 1);
        assert_eq!(record.headers().omitted_header_count(), 1);
        assert!(serialized.contains("ordinary visible sample text"));
        assert!(serialized.contains(REDACTED_LOG_VALUE));
        for forbidden in [
            "body-secret-must-not-appear",
            "private-key-must-not-appear",
            "nested-secret-must-not-appear",
            "inline-secret-must-not-appear",
            "header-secret-must-not-appear",
            "untrusted-header-must-not-appear",
            "api_key",
            "access_token",
        ] {
            assert!(!serialized.contains(forbidden));
            assert!(!debug.contains(forbidden));
        }
        Ok(())
    }

    #[test]
    fn sampling_never_parses_oversize_or_non_json_bodies() -> TestResult {
        let request_id = RequestId::try_new("request-log-bounds")?;
        let policy = LogRedactionPolicy::new(BodySamplingPolicy::try_sampled(1, 1, 8)?);
        let oversize = policy.capture_http_record(
            &request_id,
            HttpLogDirection::UpstreamResponse,
            Some(500),
            [("Content-Type", "application/json")],
            b"{\"text\":\"too-long\"}",
        );
        let non_json = policy.capture_http_record(
            &request_id,
            HttpLogDirection::UpstreamResponse,
            Some(500),
            [("Content-Type", "text/plain")],
            b"text-secret-must-not-appear",
        );

        assert_eq!(
            oversize.body().omission_reason(),
            Some(BodyOmissionReason::TooLarge)
        );
        assert_eq!(
            non_json.body().omission_reason(),
            Some(BodyOmissionReason::NotJson)
        );
        assert!(
            !non_json
                .to_json_line()?
                .contains("text-secret-must-not-appear")
        );
        Ok(())
    }

    #[test]
    fn duplicate_content_type_fails_closed_before_body_sampling() -> TestResult {
        let request_id = RequestId::try_new("request-log-duplicate-content-type")?;
        let policy = LogRedactionPolicy::new(BodySamplingPolicy::try_sampled(1, 1, 64)?);
        let record = policy.capture_http_record(
            &request_id,
            HttpLogDirection::InboundRequest,
            None,
            [
                ("Content-Type", "application/json"),
                ("Content-Type", "application/json"),
            ],
            br#"{"secret":"must-not-appear"}"#,
        );

        assert_eq!(record.headers().content_type(), None);
        assert_eq!(record.headers().redacted_header_count(), 1);
        assert_eq!(
            record.body().omission_reason(),
            Some(BodyOmissionReason::NotJson)
        );
        assert!(!record.to_json_line()?.contains("must-not-appear"));
        Ok(())
    }

    #[test]
    fn invalid_or_unselected_bodies_never_retain_a_prefix() -> TestResult {
        let always_selected = LogRedactionPolicy::new(BodySamplingPolicy::try_sampled(1, 1, 64)?);
        let invalid_utf8_request = RequestId::try_new("request-log-invalid-utf8")?;
        let invalid_utf8 = always_selected.capture_http_record(
            &invalid_utf8_request,
            HttpLogDirection::UpstreamResponse,
            Some(500),
            [("Content-Type", "application/json")],
            &[0xff, 0xfe],
        );
        let malformed_request = RequestId::try_new("request-log-malformed-json")?;
        let malformed = always_selected.capture_http_record(
            &malformed_request,
            HttpLogDirection::UpstreamResponse,
            Some(500),
            [("Content-Type", "application/json")],
            br#"{"api_key":"malformed-secret-must-not-appear"#,
        );

        let half_sample = BodySamplingPolicy::try_sampled(1, 2, 64)?;
        let mut not_selected_request = None;
        for sequence in 0..128 {
            let candidate = RequestId::try_new(format!("request-log-not-selected-{sequence}"))?;
            if !half_sample.selects(&candidate) {
                not_selected_request = Some(candidate);
                break;
            }
        }
        let not_selected_request = not_selected_request.ok_or_else(|| {
            std::io::Error::other("expected one deterministic non-selected bucket")
        })?;
        let not_selected = LogRedactionPolicy::new(half_sample).capture_http_record(
            &not_selected_request,
            HttpLogDirection::InboundRequest,
            None,
            [("Content-Type", "application/json")],
            br#"{"api_key":"not-selected-secret-must-not-appear"}"#,
        );

        assert_eq!(
            invalid_utf8.body().omission_reason(),
            Some(BodyOmissionReason::InvalidUtf8)
        );
        assert_eq!(
            malformed.body().omission_reason(),
            Some(BodyOmissionReason::MalformedJson)
        );
        assert_eq!(
            not_selected.body().omission_reason(),
            Some(BodyOmissionReason::NotSelected)
        );
        for (record, forbidden) in [
            (&malformed, "malformed-secret-must-not-appear"),
            (&not_selected, "not-selected-secret-must-not-appear"),
        ] {
            assert!(!record.to_json_line()?.contains(forbidden));
            assert!(!format!("{record:?}").contains(forbidden));
        }
        Ok(())
    }

    #[test]
    fn sample_selection_is_stable_and_invalid_configurations_fail_closed() -> TestResult {
        let request_id = RequestId::try_new("request-log-stable")?;
        let policy = BodySamplingPolicy::try_sampled(1, 2, 64)?;
        assert_eq!(policy.selects(&request_id), policy.selects(&request_id));
        assert!(BodySamplingPolicy::try_sampled(0, 1, 64).is_err());
        assert!(BodySamplingPolicy::try_sampled(2, 1, 64).is_err());
        assert!(BodySamplingPolicy::try_sampled(1, 1, 0).is_err());
        assert!(BodySamplingPolicy::try_sampled(1, 1, 16_385).is_err());
        Ok(())
    }
}
