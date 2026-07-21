//! Secret-safe, transport-neutral observations emitted from the gateway data path.
//!
//! These types deliberately describe only structured lifecycle metadata. They retain no request
//! body, response text, raw headers, URL, presented Client Key, or Credential secret. The
//! [`GatewayEventSink`] port is synchronous and non-blocking; durable persistence and exporters
//! remain outside the request path.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    AccessGroupId, AttemptId, ClientKeyId, CredentialId, EndpointId, GatewayError, HealthEventId,
    RequestId, ResponseId, RouteCandidateId, RouteId, UpstreamId, Usage,
};

/// The public protocol that accepted a request observation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GatewayProtocol {
    /// The `OpenAI`-compatible Responses entrypoint.
    #[serde(rename = "openai_responses")]
    OpenAiResponses,
}

/// Importance class used by a bounded event implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GatewayEventPriority {
    /// Request, Attempt, Usage, and Health records must be kept separate from diagnostics.
    Required,
    /// A safe diagnostic may be discarded under bounded queue pressure.
    Diagnostic,
}

/// Result of a synchronous, non-blocking event admission attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventEmission {
    /// The event entered the receiving implementation's bounded queue.
    Enqueued,
    /// The configured sink intentionally records nothing.
    Disabled,
    /// A required-record queue was saturated; the implementation must expose this loss explicitly.
    RequiredQueueFull,
    /// A low-priority diagnostic was deliberately discarded under bounded queue pressure.
    DiagnosticDropped,
    /// The receiver is no longer available.
    SinkClosed,
}

/// A secret-safe event port that never waits for a database, network exporter, or queue slot.
pub trait GatewayEventSink: Send + Sync {
    /// Attempts to admit one event without blocking the request path.
    fn try_emit(&self, event: GatewayEvent) -> EventEmission;
}

/// Default event sink for embeddings that have not yet attached an event consumer.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopGatewayEventSink;

impl GatewayEventSink for NoopGatewayEventSink {
    fn try_emit(&self, _event: GatewayEvent) -> EventEmission {
        EventEmission::Disabled
    }
}

/// One structured lifecycle event emitted by a gateway boundary.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayEvent {
    /// An authenticated, decoded request entered execution.
    Request(RequestEvent),
    /// One concrete upstream Attempt reached a terminal local decision.
    Attempt(AttemptEvent),
    /// One final token-usage snapshot reached the canonical response path.
    Usage(UsageEvent),
    /// One sanitized runtime-health transition supplied by a background/control component.
    Health(HealthEvent),
    /// A safe, low-priority internal diagnostic.
    Diagnostic(DiagnosticEvent),
}

impl GatewayEvent {
    /// Returns the bounded-queue priority for this event.
    #[must_use]
    pub const fn priority(&self) -> GatewayEventPriority {
        match self {
            Self::Request(_) | Self::Attempt(_) | Self::Usage(_) | Self::Health(_) => {
                GatewayEventPriority::Required
            }
            Self::Diagnostic(_) => GatewayEventPriority::Diagnostic,
        }
    }
}

impl fmt::Debug for GatewayEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(event) => formatter
                .debug_tuple("GatewayEvent::Request")
                .field(event)
                .finish(),
            Self::Attempt(event) => formatter
                .debug_tuple("GatewayEvent::Attempt")
                .field(event)
                .finish(),
            Self::Usage(event) => formatter
                .debug_tuple("GatewayEvent::Usage")
                .field(event)
                .finish(),
            Self::Health(event) => formatter
                .debug_tuple("GatewayEvent::Health")
                .field(event)
                .finish(),
            Self::Diagnostic(event) => formatter
                .debug_tuple("GatewayEvent::Diagnostic")
                .field(event)
                .finish(),
        }
    }
}

/// Immutable request metadata that excludes request content and presented authentication material.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEvent {
    request_id: RequestId,
    client_key_id: ClientKeyId,
    access_group_id: Option<AccessGroupId>,
    protocol: GatewayProtocol,
    requested_model: String,
    public_model: String,
    route_alias: Option<String>,
    streaming: bool,
}

impl RequestEvent {
    /// Creates one accepted-request event from already-authenticated, decoded metadata.
    #[must_use]
    #[allow(clippy::too_many_arguments)] // Mirrors the frozen Request record fields without a mutable builder.
    pub fn new(
        request_id: RequestId,
        client_key_id: ClientKeyId,
        access_group_id: Option<AccessGroupId>,
        protocol: GatewayProtocol,
        requested_model: String,
        public_model: String,
        route_alias: Option<String>,
        streaming: bool,
    ) -> Self {
        Self {
            request_id,
            client_key_id,
            access_group_id,
            protocol,
            requested_model,
            public_model,
            route_alias,
            streaming,
        }
    }

    /// Returns the external request correlation identifier.
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Returns the non-secret authenticated Client Key identity.
    #[must_use]
    pub const fn client_key_id(&self) -> &ClientKeyId {
        &self.client_key_id
    }

    /// Returns the Snapshot access group when this entrypoint had one.
    #[must_use]
    pub fn access_group_id(&self) -> Option<&AccessGroupId> {
        self.access_group_id.as_ref()
    }

    /// Returns the protocol that accepted the request.
    #[must_use]
    pub const fn protocol(&self) -> GatewayProtocol {
        self.protocol
    }

    /// Returns the client-requested model reference.
    #[must_use]
    pub fn requested_model(&self) -> &str {
        &self.requested_model
    }

    /// Returns the stable public model name selected for this request.
    #[must_use]
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// Returns the input Alias when one was force-mapped to the public model.
    #[must_use]
    pub fn route_alias(&self) -> Option<&str> {
        self.route_alias.as_deref()
    }

    /// Returns whether the client requested an SSE response.
    #[must_use]
    pub const fn streaming(&self) -> bool {
        self.streaming
    }
}

impl fmt::Debug for RequestEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestEvent")
            .field("request_id", &self.request_id)
            .field("client_key_id", &self.client_key_id)
            .field("access_group_id", &self.access_group_id)
            .field("protocol", &self.protocol)
            .field("requested_model", &"<redacted>")
            .field("public_model", &"<redacted>")
            .field("route_alias_present", &self.route_alias.is_some())
            .field("streaming", &self.streaming)
            .finish()
    }
}

/// Terminal result observed for one concrete upstream Attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptOutcome {
    /// The driver returned a live successful output.
    Succeeded,
    /// The driver or retry gate returned one safe, secret-free error classification.
    Failed(GatewayError),
}

/// The retry decision evaluated after an Attempt's terminal outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptRetryDecision {
    /// A successful Attempt has no transparent retry decision.
    Completed,
    /// The failure is eligible for a later bounded transparent retry.
    RetryEligible,
    /// The driver failure is explicitly non-retryable.
    NonRetryable,
    /// Downstream semantic delivery closed transparent retry.
    RetryClosed,
    /// Client cancellation ended the request.
    Cancelled,
    /// The gateway could not safely update required runtime state after a failed Attempt.
    InfrastructureFailure,
}

/// One terminal upstream Attempt record with no body, URL, header, or secret material.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptEvent {
    request_id: RequestId,
    attempt_id: AttemptId,
    attempt_number: u64,
    route_id: RouteId,
    route_candidate_id: RouteCandidateId,
    credential_id: CredentialId,
    endpoint_id: EndpointId,
    upstream_id: UpstreamId,
    upstream_model: String,
    started_at_ms: i64,
    ended_at_ms: i64,
    outcome: AttemptOutcome,
    retry_decision: AttemptRetryDecision,
}

impl AttemptEvent {
    /// Creates a terminal observation for one actual driver invocation.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request_id: RequestId,
        attempt_number: u64,
        route_id: RouteId,
        route_candidate_id: RouteCandidateId,
        credential_id: CredentialId,
        endpoint_id: EndpointId,
        upstream_id: UpstreamId,
        upstream_model: String,
        started_at_ms: i64,
        ended_at_ms: i64,
        outcome: AttemptOutcome,
        retry_decision: AttemptRetryDecision,
    ) -> Self {
        let attempt_id = AttemptId::from_request_sequence(&request_id, attempt_number);
        Self {
            request_id,
            attempt_id,
            attempt_number,
            route_id,
            route_candidate_id,
            credential_id,
            endpoint_id,
            upstream_id,
            upstream_model,
            started_at_ms,
            ended_at_ms,
            outcome,
            retry_decision,
        }
    }

    /// Returns the external request correlation identifier.
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Returns the deterministic Attempt identity scoped to this request and sequence.
    #[must_use]
    pub const fn attempt_id(&self) -> &AttemptId {
        &self.attempt_id
    }

    /// Returns the one-based sequence number of this request's actual driver invocation.
    #[must_use]
    pub const fn attempt_number(&self) -> u64 {
        self.attempt_number
    }

    /// Returns the selected Route identity.
    #[must_use]
    pub const fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    /// Returns the selected Route Candidate identity.
    #[must_use]
    pub const fn route_candidate_id(&self) -> &RouteCandidateId {
        &self.route_candidate_id
    }

    /// Returns the selected non-secret Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the selected Endpoint identity.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the selected Upstream identity.
    #[must_use]
    pub const fn upstream_id(&self) -> &UpstreamId {
        &self.upstream_id
    }

    /// Returns the internal upstream model label for durable, access-controlled event consumers.
    #[must_use]
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// Returns the attempt start timestamp from the injected routing clock.
    #[must_use]
    pub const fn started_at_ms(&self) -> i64 {
        self.started_at_ms
    }

    /// Returns the attempt terminal timestamp from the injected routing clock.
    #[must_use]
    pub const fn ended_at_ms(&self) -> i64 {
        self.ended_at_ms
    }

    /// Returns the safe terminal result.
    #[must_use]
    pub const fn outcome(&self) -> &AttemptOutcome {
        &self.outcome
    }

    /// Returns the bounded transparent-retry decision.
    #[must_use]
    pub const fn retry_decision(&self) -> AttemptRetryDecision {
        self.retry_decision
    }
}

impl fmt::Debug for AttemptEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AttemptEvent")
            .field("request_id", &self.request_id)
            .field("attempt_id", &self.attempt_id)
            .field("attempt_number", &self.attempt_number)
            .field("route_id", &self.route_id)
            .field("route_candidate_id", &self.route_candidate_id)
            .field("credential_id", &self.credential_id)
            .field("endpoint_id", &self.endpoint_id)
            .field("upstream_id", &self.upstream_id)
            .field("upstream_model", &"<redacted>")
            .field("started_at_ms", &self.started_at_ms)
            .field("ended_at_ms", &self.ended_at_ms)
            .field("outcome", &self.outcome)
            .field("retry_decision", &self.retry_decision)
            .finish()
    }
}

/// Final token totals copied without raw protocol extensions.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // Names intentionally match the frozen Usage contract.
pub struct UsageSummary {
    /// Input tokens reported by the upstream.
    pub input_tokens: Option<u64>,
    /// Output tokens reported by the upstream.
    pub output_tokens: Option<u64>,
    /// Reasoning tokens reported by the upstream.
    pub reasoning_tokens: Option<u64>,
    /// Cache-read tokens reported by the upstream.
    pub cache_read_tokens: Option<u64>,
    /// Cache-creation tokens reported by the upstream.
    pub cache_creation_tokens: Option<u64>,
    /// Cached tokens when an upstream exposes only that aggregate.
    pub cached_tokens: Option<u64>,
}

impl From<&Usage> for UsageSummary {
    fn from(usage: &Usage) -> Self {
        Self {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            reasoning_tokens: usage.reasoning_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_creation_tokens: usage.cache_creation_tokens,
            cached_tokens: usage.cached_tokens,
        }
    }
}

/// One final Usage observation correlated to an accepted request and response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageEvent {
    request_id: RequestId,
    response_id: ResponseId,
    usage: UsageSummary,
}

impl UsageEvent {
    /// Creates a final Usage event while intentionally dropping protocol-specific extensions.
    #[must_use]
    pub fn from_usage(request_id: RequestId, response_id: ResponseId, usage: &Usage) -> Self {
        Self {
            request_id,
            response_id,
            usage: UsageSummary::from(usage),
        }
    }

    /// Returns the external request correlation identifier.
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Returns the canonical response correlation identifier.
    #[must_use]
    pub const fn response_id(&self) -> &ResponseId {
        &self.response_id
    }

    /// Returns only the standardized token totals.
    #[must_use]
    pub const fn usage(&self) -> &UsageSummary {
        &self.usage
    }
}

/// One bounded non-secret runtime-health transition retained for durable operational correlation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthEventKind {
    /// A controlled Endpoint/Credential/model probe reached its success condition.
    ProbeSucceeded,
    /// A controlled Endpoint/Credential/model probe did not reach its success condition.
    ProbeFailed,
    /// A runtime Circuit opened for a bounded exact target.
    CircuitOpened,
    /// A validated half-open probe closed the exact Circuit.
    CircuitRecovered,
}

/// One runtime-health transition with no URL, Header, body, status text, or Secret material.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // The serialized field is the stable Health-event identifier.
pub struct HealthEvent {
    health_event_id: HealthEventId,
    endpoint_id: EndpointId,
    credential_id: Option<CredentialId>,
    upstream_model: Option<String>,
    occurred_at_ms: i64,
    kind: HealthEventKind,
}

impl HealthEvent {
    /// Creates one pre-classified exact-target health transition.
    #[must_use]
    pub fn new(
        health_event_id: HealthEventId,
        endpoint_id: EndpointId,
        credential_id: Option<CredentialId>,
        upstream_model: Option<String>,
        occurred_at_ms: i64,
        kind: HealthEventKind,
    ) -> Self {
        Self {
            health_event_id,
            endpoint_id,
            credential_id,
            upstream_model,
            occurred_at_ms,
            kind,
        }
    }

    /// Returns the stable idempotence key for this health transition.
    #[must_use]
    pub const fn health_event_id(&self) -> &HealthEventId {
        &self.health_event_id
    }

    /// Returns the exact protocol-specific Endpoint that observed the transition.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }

    /// Returns the optional non-secret Credential scope.
    #[must_use]
    pub fn credential_id(&self) -> Option<&CredentialId> {
        self.credential_id.as_ref()
    }

    /// Returns the optional exact upstream-model scope for access-controlled durable consumers.
    #[must_use]
    pub fn upstream_model(&self) -> Option<&str> {
        self.upstream_model.as_deref()
    }

    /// Returns the explicitly supplied runtime-health transition timestamp.
    #[must_use]
    pub const fn occurred_at_ms(&self) -> i64 {
        self.occurred_at_ms
    }

    /// Returns the sanitized transition category.
    #[must_use]
    pub const fn kind(&self) -> HealthEventKind {
        self.kind
    }
}

impl fmt::Debug for HealthEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HealthEvent")
            .field("health_event_id", &self.health_event_id)
            .field("endpoint_id", &self.endpoint_id)
            .field("credential_id", &self.credential_id)
            .field("upstream_model_present", &self.upstream_model.is_some())
            .field("occurred_at_ms", &self.occurred_at_ms)
            .field("kind", &self.kind)
            .finish()
    }
}

/// A safe low-priority diagnostic that contains no free-form text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticEvent {
    error: GatewayError,
}

impl DiagnosticEvent {
    /// Creates a diagnostic from the stable safe gateway error type.
    #[must_use]
    pub const fn new(error: GatewayError) -> Self {
        Self { error }
    }

    /// Returns the safe error classification.
    #[must_use]
    pub const fn error(&self) -> &GatewayError {
        &self.error
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AttemptEvent, AttemptOutcome, AttemptRetryDecision, GatewayEvent, GatewayEventPriority,
        GatewayProtocol, HealthEvent, HealthEventKind, RequestEvent, UsageEvent,
    };
    use crate::{
        AccessGroupId, ClientKeyId, CredentialId, EndpointId, HealthEventId, RawExtensions,
        RawJson, RequestId, ResponseId, RouteCandidateId, RouteId, UpstreamId, Usage,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn request_and_attempt_debug_forms_redact_model_values() -> TestResult {
        let request = RequestEvent::new(
            RequestId::try_new("request-01")?,
            ClientKeyId::try_new("client-key-01")?,
            Some(AccessGroupId::try_new("group-01")?),
            GatewayProtocol::OpenAiResponses,
            "requested-sensitive-model".to_owned(),
            "public-sensitive-model".to_owned(),
            Some("sensitive-alias".to_owned()),
            true,
        );
        let attempt = AttemptEvent::new(
            RequestId::try_new("request-01")?,
            1,
            RouteId::try_new("route-01")?,
            RouteCandidateId::try_new("candidate-01")?,
            CredentialId::try_new("credential-01")?,
            EndpointId::try_new("endpoint-01")?,
            UpstreamId::try_new("upstream-01")?,
            "private-upstream-model".to_owned(),
            1,
            2,
            AttemptOutcome::Succeeded,
            AttemptRetryDecision::Completed,
        );
        let diagnostic = format!("{request:?}{attempt:?}");

        for sensitive in [
            "requested-sensitive-model",
            "public-sensitive-model",
            "sensitive-alias",
            "private-upstream-model",
        ] {
            assert!(!diagnostic.contains(sensitive));
        }
        Ok(())
    }

    #[test]
    fn usage_drops_raw_extensions_and_event_priorities_are_explicit() -> TestResult {
        let mut extensions = RawExtensions::default();
        extensions.try_insert(
            "private_usage_extension",
            RawJson::from_json_string(r#"{"token":"must-not-persist"}"#.to_owned())?,
        )?;
        let usage = Usage {
            input_tokens: Some(3),
            output_tokens: Some(5),
            extensions,
            ..Usage::default()
        };
        let event = UsageEvent::from_usage(
            RequestId::try_new("request-01")?,
            ResponseId::try_new("response-01")?,
            &usage,
        );
        assert_eq!(event.usage().input_tokens, Some(3));
        assert_eq!(event.usage().output_tokens, Some(5));
        let event = GatewayEvent::Usage(event);
        assert_eq!(event.priority(), GatewayEventPriority::Required);
        let serialized = serde_json::to_string(&event)?;
        assert!(!serialized.contains("must-not-persist"));
        Ok(())
    }

    #[test]
    fn health_events_are_required_and_redact_internal_model_values() -> TestResult {
        let event = HealthEvent::new(
            HealthEventId::try_new("health-01")?,
            EndpointId::try_new("endpoint-01")?,
            Some(CredentialId::try_new("credential-01")?),
            Some("private-upstream-model".to_owned()),
            100,
            HealthEventKind::CircuitRecovered,
        );
        let diagnostic = format!("{event:?}");
        assert!(!diagnostic.contains("private-upstream-model"));
        assert_eq!(
            GatewayEvent::Health(event).priority(),
            GatewayEventPriority::Required
        );
        Ok(())
    }
}
