//! Pure Anthropic-compatible Messages request construction.
//!
//! This module deliberately stops before any socket, TLS, proxy, timeout, connection-pool,
//! credential-lease, routing, retry, response-decoding, or stream-delivery behavior. It is the
//! Anthropic-format sibling of `provider-openai-compatible`: the runtime composition root owns
//! every one of those concerns for both formats.

use std::fmt;

use gateway_core::{CanonicalRequest, ErrorScope, GatewayError, GatewayErrorCode};
use gateway_upstream::{
    AdmittedEgressTarget, EndpointUrl, UpstreamHttpMethod, UpstreamHttpRequest,
};
use protocol_anthropic::{ResponseMode, encode_upstream_request};
use zeroize::Zeroizing;

/// The Anthropic Messages API version sent on every outbound request.
///
/// The value is a dated wire-format contract rather than a tuning knob: changing it changes what
/// the upstream returns, and therefore what the paired response decoder must accept. Pinning it in
/// code keeps one Endpoint's API Format and its version versioned together.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// The Anthropic-convention inference path appended to an Endpoint Base URL.
///
/// A control-plane Endpoint still supplies its own path to [`AnthropicMessagesEndpoint::try_new`];
/// this constant only records the convention used by the aggregation design for an
/// `anthropic/messages` Endpoint.
pub const ANTHROPIC_MESSAGES_INFERENCE_PATH: &str = "/v1/messages";

const JSON_MEDIA_TYPE: &str = "application/json";
const SSE_MEDIA_TYPE: &str = "text/event-stream";

/// A short-lived upstream `x-api-key` credential accepted by the Anthropic-compatible builder.
///
/// The builder never reads encrypted storage. A request-scoped credential lease supplies this
/// value; this type keeps the construction boundary explicit and redacts the credential from
/// diagnostics.
pub struct AnthropicMessagesApiKey(Zeroizing<String>);

impl AnthropicMessagesApiKey {
    /// Creates one non-empty HTTP-header-safe `x-api-key` credential.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` for an empty or non-visible-ASCII value, before
    /// a malformed `x-api-key` header could reach a later transport.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GatewayError> {
        let value = value.into();
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(credential_unavailable_error());
        }

        Ok(Self(Zeroizing::new(value)))
    }

    /// Returns the credential only for request-scoped `x-api-key` header construction.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AnthropicMessagesApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AnthropicMessagesApiKey(<redacted>)")
    }
}

/// One validated Anthropic-compatible Messages endpoint target.
#[derive(Clone, Eq, PartialEq)]
pub struct AnthropicMessagesEndpoint {
    target: EndpointUrl,
}

impl AnthropicMessagesEndpoint {
    /// Creates an endpoint by retaining the Base URL path and appending the messages path.
    ///
    /// A configured Endpoint supplies both values; [`ANTHROPIC_MESSAGES_INFERENCE_PATH`] records
    /// the Anthropic convention for the second one.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` for an invalid configured URL/path shape. The shared egress
    /// policy still owns actual scheme/host/CIDR/DNS admission for every later dial attempt.
    pub fn try_new(base_url: &str, inference_path: &str) -> Result<Self, GatewayError> {
        let target =
            EndpointUrl::compose(base_url, inference_path).map_err(|_| egress_rejected_error())?;
        Ok(Self { target })
    }

    /// Returns the complete endpoint target for a later HTTP transport.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }

    /// Returns the typed endpoint target without re-parsing it.
    #[must_use]
    pub fn target(&self) -> &EndpointUrl {
        &self.target
    }
}

impl fmt::Debug for AnthropicMessagesEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AnthropicMessagesEndpoint(<redacted>)")
    }
}

/// One request-ready Anthropic-compatible Messages target, headers, and JSON body.
///
/// This is not an HTTP-client request type. A transport consumes its typed target and exact
/// request-scoped header values only through [`Self::into_transport_request`], after the shared
/// egress policy admits the same target.
#[derive(Eq, PartialEq)]
pub struct AnthropicMessagesOutboundRequest {
    target: EndpointUrl,
    x_api_key: Zeroizing<String>,
    accept: &'static str,
    body: Vec<u8>,
}

impl AnthropicMessagesOutboundRequest {
    /// Returns the complete configured endpoint URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }

    /// Returns the parsed endpoint target for a later HTTP transport.
    #[must_use]
    pub fn target(&self) -> &EndpointUrl {
        &self.target
    }

    /// Returns one standard request header by case-insensitive name.
    ///
    /// The `x-api-key` value is request-scoped and must not be logged or persisted by callers.
    /// `authorization` is deliberately absent: Anthropic authenticates an API key with `x-api-key`,
    /// and a relay that accepts exactly one presentation scheme rejects a request carrying both.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("accept") {
            Some(self.accept)
        } else if name.eq_ignore_ascii_case("anthropic-version") {
            Some(ANTHROPIC_VERSION)
        } else if name.eq_ignore_ascii_case("content-type") {
            Some(JSON_MEDIA_TYPE)
        } else if name.eq_ignore_ascii_case("x-api-key") {
            Some(self.x_api_key.as_str())
        } else {
            None
        }
    }

    /// Returns the complete fixed header set in deterministic transport order.
    ///
    /// The set is exactly these four headers. A caller that needs an additional header composes
    /// above this boundary instead of widening it.
    #[must_use]
    pub fn headers(&self) -> [(&'static str, &str); 4] {
        [
            ("accept", self.accept),
            ("anthropic-version", ANTHROPIC_VERSION),
            ("content-type", JSON_MEDIA_TYPE),
            ("x-api-key", self.x_api_key.as_str()),
        ]
    }

    /// Returns the complete JSON request body without rendering it to a log representation.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Consumes this request into a DNS-pinned shared-transport request.
    ///
    /// The caller must first obtain `admitted_target` by applying the shared `EgressPolicy` to this
    /// exact target. A different allowed URL is not interchangeable: it is rejected before the
    /// credential or body can be handed to the HTTP client.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` when the admitted URL does not exactly match this request
    /// target, or `InternalError/Internal` if the fixed header set cannot satisfy the shared
    /// transport request invariant.
    pub fn into_transport_request(
        self,
        admitted_target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GatewayError> {
        let Self {
            target,
            x_api_key,
            accept,
            body,
        } = self;
        if admitted_target.request_url() != target.as_url() {
            return Err(egress_rejected_error());
        }

        UpstreamHttpRequest::try_new(
            admitted_target,
            UpstreamHttpMethod::Post,
            [
                ("accept".to_owned(), accept.to_owned()),
                ("anthropic-version".to_owned(), ANTHROPIC_VERSION.to_owned()),
                ("content-type".to_owned(), JSON_MEDIA_TYPE.to_owned()),
                ("x-api-key".to_owned(), x_api_key.as_str().to_owned()),
            ],
            body,
        )
        .map_err(|_| internal_error())
    }
}

impl fmt::Debug for AnthropicMessagesOutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnthropicMessagesOutboundRequest")
            .field("target", &"<redacted>")
            .field(
                "header_names",
                &["accept", "anthropic-version", "content-type", "x-api-key"],
            )
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Stateless encoder for one Anthropic-compatible Messages upstream request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AnthropicMessagesRequestBuilder;

impl AnthropicMessagesRequestBuilder {
    /// Builds a request-ready Anthropic-compatible Messages target, standard headers, and JSON body.
    ///
    /// The caller supplies the already selected upstream model; the client-visible requested model
    /// is never forwarded. Every Canonical shape the Anthropic Messages wire format cannot express
    /// losslessly fails closed in the Anthropic codec rather than being silently degraded.
    ///
    /// # Errors
    ///
    /// Returns a safe `GatewayError` for an empty upstream model, an unsupported Canonical
    /// representation, or a JSON construction invariant failure.
    pub fn build(
        endpoint: &AnthropicMessagesEndpoint,
        api_key: &AnthropicMessagesApiKey,
        upstream_model: &str,
        request: &CanonicalRequest,
        mode: ResponseMode,
    ) -> Result<AnthropicMessagesOutboundRequest, GatewayError> {
        if upstream_model.is_empty() {
            return Err(provider_protocol_error());
        }

        let body = encode_body(upstream_model, request, mode)?;
        let accept = match mode {
            ResponseMode::NonStreaming => JSON_MEDIA_TYPE,
            ResponseMode::Streaming => SSE_MEDIA_TYPE,
        };

        Ok(AnthropicMessagesOutboundRequest {
            target: endpoint.target().clone(),
            x_api_key: Zeroizing::new(api_key.as_str().to_owned()),
            accept,
            body,
        })
    }
}

/// Delegates the whole body to the Anthropic outbound codec and forwards its bytes verbatim.
///
/// The codec returns one complete serialized Anthropic Messages JSON body, so this boundary adds
/// no member, drops none, and re-serializes nothing.
fn encode_body(
    upstream_model: &str,
    request: &CanonicalRequest,
    mode: ResponseMode,
) -> Result<Vec<u8>, GatewayError> {
    Ok(encode_upstream_request(upstream_model, request, mode)?.into_bytes())
}

const fn credential_unavailable_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::CredentialUnavailable,
        ErrorScope::Credential,
    )
}

const fn egress_rejected_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
}

const fn provider_protocol_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::UpstreamProtocolError,
        ErrorScope::Provider,
    )
}

const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        error::Error,
        net::{IpAddr, Ipv4Addr},
    };

    use gateway_core::{CanonicalRequest, EgressPolicyId, GatewayErrorCode, RawJson};
    use gateway_upstream::{
        EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput,
        EgressScheme, RedirectPolicy, UpstreamHttpMethod,
    };
    use protocol_anthropic::{
        DecodedMessagesRequest, ResponseMode, decode_count_tokens_request, decode_request,
        encode_upstream_request,
    };

    use super::{
        ANTHROPIC_MESSAGES_INFERENCE_PATH, ANTHROPIC_VERSION, AnthropicMessagesApiKey,
        AnthropicMessagesEndpoint, AnthropicMessagesRequestBuilder,
    };

    const UPSTREAM_MODEL: &str = "claude-upstream-model";
    const TEST_CREDENTIAL: &str = "anthropic_compatible_test_credential";

    const TEXT_REQUEST: &str = r#"{
        "model": "gateway-claude",
        "max_tokens": 64,
        "messages": [{"role": "user", "content": "ping"}]
    }"#;

    const TOOL_REQUEST: &str = r#"{
        "model": "gateway-claude",
        "max_tokens": 64,
        "messages": [
            {"role": "user", "content": "check the weather"},
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "call-01",
                        "name": "lookup",
                        "input": {"query": "weather"}
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "call-01",
                        "content": {"forecast": "clear"},
                        "is_error": false
                    }
                ]
            }
        ],
        "tools": [
            {
                "name": "lookup",
                "description": "Look up a value.",
                "input_schema": {
                    "type": "object",
                    "properties": {"query": {"type": "string"}},
                    "required": ["query"]
                }
            }
        ]
    }"#;

    const STREAMING_REQUEST: &str = r#"{
        "model": "gateway-claude",
        "max_tokens": 64,
        "stream": true,
        "messages": [{"role": "user", "content": "ping"}]
    }"#;

    fn endpoint() -> Result<AnthropicMessagesEndpoint, Box<dyn Error>> {
        Ok(AnthropicMessagesEndpoint::try_new(
            "https://relay.example",
            ANTHROPIC_MESSAGES_INFERENCE_PATH,
        )?)
    }

    fn api_key() -> Result<AnthropicMessagesApiKey, Box<dyn Error>> {
        Ok(AnthropicMessagesApiKey::try_new(TEST_CREDENTIAL)?)
    }

    fn upstream(decoded: &DecodedMessagesRequest) -> CanonicalRequest {
        let mut request = decoded.request.clone();
        request.requested_model = UPSTREAM_MODEL.to_owned();
        request
    }

    struct StaticPublicResolver;

    impl EgressDnsResolver for StaticPublicResolver {
        fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
            Ok(vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
        }
    }

    fn policy() -> Result<EgressPolicy, Box<dyn Error>> {
        Ok(EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("anthropic-compatible-test-policy")?,
            name: "anthropic compatible test policy".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Https]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("relay.example")?]),
            allowed_ports: BTreeSet::from([443]),
            allowed_cidrs: BTreeSet::new(),
            redirect_policy: RedirectPolicy::Deny,
        })?)
    }

    #[test]
    fn builds_the_exact_text_request_target_headers_and_body() -> Result<(), Box<dyn Error>> {
        let decoded = decode_request(TEXT_REQUEST)?;
        let outbound = AnthropicMessagesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            UPSTREAM_MODEL,
            &decoded.request,
            decoded.mode,
        )?;

        assert_eq!(decoded.mode, ResponseMode::NonStreaming);
        assert_eq!(outbound.url(), "https://relay.example/v1/messages");
        assert_eq!(
            outbound.headers(),
            [
                ("accept", "application/json"),
                ("anthropic-version", "2023-06-01"),
                ("content-type", "application/json"),
                ("x-api-key", TEST_CREDENTIAL),
            ]
        );
        assert_eq!(
            outbound.header("Anthropic-Version"),
            Some(ANTHROPIC_VERSION)
        );
        assert_eq!(outbound.header("X-Api-Key"), Some(TEST_CREDENTIAL));
        assert_eq!(outbound.header("authorization"), None);
        assert_eq!(outbound.header("x-not-configured"), None);

        let expected_body =
            encode_upstream_request(UPSTREAM_MODEL, &decoded.request, decoded.mode)?;
        assert_eq!(outbound.body(), expected_body.as_bytes());

        let encoded = std::str::from_utf8(outbound.body())?;
        let rebuilt = decode_request(encoded)?;
        assert_eq!(rebuilt.request, upstream(&decoded));
        assert_eq!(rebuilt.mode, ResponseMode::NonStreaming);
        assert!(encoded.contains(UPSTREAM_MODEL));
        assert!(!encoded.contains("gateway-claude"));
        Ok(())
    }

    #[test]
    fn builds_the_exact_tool_request_body_without_dropping_tool_semantics()
    -> Result<(), Box<dyn Error>> {
        let decoded = decode_request(TOOL_REQUEST)?;
        let outbound = AnthropicMessagesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            UPSTREAM_MODEL,
            &decoded.request,
            decoded.mode,
        )?;

        let expected_body =
            encode_upstream_request(UPSTREAM_MODEL, &decoded.request, decoded.mode)?;
        assert_eq!(outbound.body(), expected_body.as_bytes());

        let encoded = std::str::from_utf8(outbound.body())?;
        assert!(encoded.contains("\"tool_use\""));
        assert!(encoded.contains("\"tool_result\""));
        assert!(encoded.contains("\"input_schema\""));

        let rebuilt = decode_request(encoded)?;
        assert_eq!(rebuilt.request, upstream(&decoded));
        assert_eq!(rebuilt.request.tools.len(), 1);
        assert_eq!(outbound.header("accept"), Some("application/json"));
        assert_eq!(outbound.header("authorization"), None);
        Ok(())
    }

    #[test]
    fn builds_the_exact_streaming_request_headers_and_stream_flag() -> Result<(), Box<dyn Error>> {
        let decoded = decode_request(STREAMING_REQUEST)?;
        assert_eq!(decoded.mode, ResponseMode::Streaming);

        let streaming = AnthropicMessagesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            UPSTREAM_MODEL,
            &decoded.request,
            decoded.mode,
        )?;

        assert_eq!(
            streaming.headers(),
            [
                ("accept", "text/event-stream"),
                ("anthropic-version", "2023-06-01"),
                ("content-type", "application/json"),
                ("x-api-key", TEST_CREDENTIAL),
            ]
        );

        let expected_body =
            encode_upstream_request(UPSTREAM_MODEL, &decoded.request, ResponseMode::Streaming)?;
        assert_eq!(streaming.body(), expected_body.as_bytes());

        let encoded = std::str::from_utf8(streaming.body())?;
        assert!(encoded.contains("\"stream\":true"));
        assert_eq!(decode_request(encoded)?.mode, ResponseMode::Streaming);

        let non_streaming = AnthropicMessagesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            UPSTREAM_MODEL,
            &decoded.request,
            ResponseMode::NonStreaming,
        )?;
        assert_eq!(non_streaming.header("accept"), Some("application/json"));
        assert_eq!(
            non_streaming.header("anthropic-version"),
            Some(ANTHROPIC_VERSION)
        );
        let non_streaming_encoded = std::str::from_utf8(non_streaming.body())?;
        assert!(non_streaming_encoded.contains("\"stream\":false"));
        assert_eq!(
            decode_request(non_streaming_encoded)?.mode,
            ResponseMode::NonStreaming
        );
        Ok(())
    }

    #[test]
    fn redacts_the_credential_target_and_body_in_every_diagnostic() -> Result<(), Box<dyn Error>> {
        let decoded = decode_request(TOOL_REQUEST)?;
        let outbound = AnthropicMessagesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            UPSTREAM_MODEL,
            &decoded.request,
            decoded.mode,
        )?;

        let debug = format!("{outbound:?}{:?}{:?}", endpoint()?, api_key()?);
        for sensitive in [
            TEST_CREDENTIAL,
            "relay.example",
            "check the weather",
            "forecast",
            UPSTREAM_MODEL,
            "lookup",
        ] {
            assert!(!debug.contains(sensitive));
        }
        assert!(debug.contains("<redacted>"));
        assert_eq!(
            format!("{:?}", api_key()?),
            "AnthropicMessagesApiKey(<redacted>)"
        );
        assert_eq!(
            format!("{:?}", endpoint()?),
            "AnthropicMessagesEndpoint(<redacted>)"
        );
        Ok(())
    }

    #[test]
    fn composes_the_messages_target_and_rejects_malformed_endpoint_configuration()
    -> Result<(), Box<dyn Error>> {
        assert_eq!(endpoint()?.url(), "https://relay.example/v1/messages");
        assert_eq!(
            AnthropicMessagesEndpoint::try_new("https://relay.example/v1", "/messages")?.url(),
            "https://relay.example/v1/messages"
        );
        assert_eq!(
            AnthropicMessagesEndpoint::try_new("https://relay.example/v1/", "/messages")?.url(),
            "https://relay.example/v1/messages"
        );
        assert_eq!(
            AnthropicMessagesEndpoint::try_new(
                "http://127.0.0.1:8080",
                ANTHROPIC_MESSAGES_INFERENCE_PATH
            )?
            .url(),
            "http://127.0.0.1:8080/v1/messages"
        );

        for base_url in [
            "relay.example",
            "ftp://relay.example",
            "mailto:operator@example.test",
            "https://user:password@relay.example",
            "https://@relay.example",
            "https://relay.example?token=secret",
            "https://relay.example#fragment",
            "https://relay.example/v1/../admin",
            "https://relay.example/v1/%2e%2e/admin",
            r"https://relay.example/v1\..\admin",
        ] {
            assert_eq!(
                AnthropicMessagesEndpoint::try_new(base_url, ANTHROPIC_MESSAGES_INFERENCE_PATH)
                    .err()
                    .map(|error| error.code()),
                Some(GatewayErrorCode::EgressRejected)
            );
        }

        for inference_path in [
            "v1/messages",
            "/v1//messages",
            "/v1/../messages",
            "/v1/messages?x=1",
        ] {
            assert_eq!(
                AnthropicMessagesEndpoint::try_new("https://relay.example", inference_path)
                    .err()
                    .map(|error| error.code()),
                Some(GatewayErrorCode::EgressRejected)
            );
        }
        Ok(())
    }

    #[test]
    fn fails_closed_on_credentials_models_and_requests_the_format_cannot_express()
    -> Result<(), Box<dyn Error>> {
        for invalid in ["", "key\r\nmalformed", "key with space"] {
            assert_eq!(
                AnthropicMessagesApiKey::try_new(invalid)
                    .err()
                    .map(|error| error.code()),
                Some(GatewayErrorCode::CredentialUnavailable)
            );
        }

        let decoded = decode_request(TEXT_REQUEST)?;
        assert_eq!(
            AnthropicMessagesRequestBuilder::build(
                &endpoint()?,
                &api_key()?,
                "",
                &decoded.request,
                decoded.mode,
            )
            .err()
            .map(|error| error.code()),
            Some(GatewayErrorCode::UpstreamProtocolError)
        );

        // Anthropic Messages requires `max_tokens` and the Canonical core has no shared
        // output-limit field, so a request that never carried the namespaced extension has no
        // lossless Anthropic Messages encoding.
        let without_output_limit = decode_count_tokens_request(
            r#"{"model":"gateway-claude","messages":[{"role":"user","content":"ping"}]}"#,
        )?;
        assert!(
            without_output_limit
                .request
                .extensions
                .get("anthropic.messages.max_tokens")
                .is_none()
        );
        assert_eq!(
            AnthropicMessagesRequestBuilder::build(
                &endpoint()?,
                &api_key()?,
                UPSTREAM_MODEL,
                &without_output_limit.request,
                ResponseMode::NonStreaming,
            )
            .err()
            .map(|error| error.code()),
            Some(GatewayErrorCode::UpstreamProtocolError)
        );

        // A foreign Provider namespace has no proven Anthropic root member.
        let mut foreign = decoded.request.clone();
        foreign.extensions.try_insert(
            "openai.responses.max_output_tokens",
            RawJson::from_json_string("64".to_owned())?,
        )?;
        assert_eq!(
            AnthropicMessagesRequestBuilder::build(
                &endpoint()?,
                &api_key()?,
                UPSTREAM_MODEL,
                &foreign,
                decoded.mode,
            )
            .err()
            .map(|error| error.code()),
            Some(GatewayErrorCode::UpstreamProtocolError)
        );
        Ok(())
    }

    #[test]
    fn hands_only_the_exact_egress_admitted_request_to_the_shared_transport()
    -> Result<(), Box<dyn Error>> {
        let decoded = decode_request(STREAMING_REQUEST)?;
        let outbound = AnthropicMessagesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            UPSTREAM_MODEL,
            &decoded.request,
            decoded.mode,
        )?;
        let admitted = policy()?.admit_url(outbound.url(), &StaticPublicResolver)?;
        let transport = outbound.into_transport_request(admitted)?;

        assert_eq!(transport.method(), UpstreamHttpMethod::Post);
        for (name, value) in [
            ("accept", "text/event-stream"),
            ("anthropic-version", "2023-06-01"),
            ("content-type", "application/json"),
            ("x-api-key", TEST_CREDENTIAL),
        ] {
            assert_eq!(
                transport
                    .header(name)
                    .and_then(|header| header.to_str().ok()),
                Some(value)
            );
        }
        assert!(transport.header("authorization").is_none());
        assert!(std::str::from_utf8(transport.body())?.contains(UPSTREAM_MODEL));

        let debug = format!("{transport:?}");
        assert!(!debug.contains(TEST_CREDENTIAL));
        assert!(!debug.contains("relay.example"));

        let mismatched_outbound = AnthropicMessagesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            UPSTREAM_MODEL,
            &decoded.request,
            decoded.mode,
        )?;
        let mismatched_target = policy()?.admit_url(
            "https://relay.example/v1/not-messages",
            &StaticPublicResolver,
        )?;
        assert_eq!(
            mismatched_outbound
                .into_transport_request(mismatched_target)
                .err()
                .map(|error| error.code()),
            Some(GatewayErrorCode::EgressRejected)
        );
        Ok(())
    }
}
