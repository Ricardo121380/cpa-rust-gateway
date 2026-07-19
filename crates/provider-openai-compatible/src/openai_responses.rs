//! Pure OpenAI-compatible Responses request construction.
//!
//! This module deliberately stops before any socket, TLS, proxy, timeout, connection-pool,
//! credential-lease, routing, retry, response-decoding, or stream-delivery behavior. P3-02 and
//! later own those runtime concerns.

use std::fmt;

use gateway_core::{
    CanonicalMessage, CanonicalRequest, ErrorScope, GatewayError, GatewayErrorCode, MessageContent,
    RawExtensions, RawJson, ToolDefinition,
};
use gateway_upstream::{
    AdmittedEgressTarget, EndpointUrl, UpstreamHttpMethod, UpstreamHttpRequest,
};
use protocol_openai_responses::ResponseMode;
use serde_json::{Map, Value};
use zeroize::Zeroizing;

const OPENAI_RESPONSES_EXTENSION_PREFIX: &str = "openai.responses.";
const ROOT_RESERVED_FIELDS: &[&str] = &[
    "model",
    "stream",
    "input",
    "tools",
    "reasoning",
    "prompt_cache_key",
    "prompt_cache_retention",
];
const MESSAGE_RESERVED_FIELDS: &[&str] = &["type", "role", "content"];
const TEXT_RESERVED_FIELDS: &[&str] = &["type", "text"];
const TOOL_CALL_RESERVED_FIELDS: &[&str] = &["type", "call_id", "name", "arguments"];
const TOOL_RESULT_RESERVED_FIELDS: &[&str] = &["type", "call_id", "output"];
const TOOL_RESERVED_FIELDS: &[&str] = &["type", "name", "description", "parameters"];
const REASONING_RESERVED_FIELDS: &[&str] = &["effort"];

/// A short-lived upstream bearer credential accepted by the OpenAI-compatible request builder.
///
/// The builder never reads encrypted storage. P3-04 will own obtaining this request-scoped value
/// from a credential lease; until then this type keeps the construction boundary explicit and
/// redacts the credential from diagnostics.
pub struct OpenAiResponsesApiKey(Zeroizing<String>);

impl OpenAiResponsesApiKey {
    /// Creates one non-empty HTTP-header-safe bearer credential.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` for an empty or non-visible-ASCII value, before
    /// a malformed Authorization header could reach a later transport.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GatewayError> {
        let value = value.into();
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(credential_unavailable_error());
        }

        Ok(Self(Zeroizing::new(value)))
    }

    /// Returns the credential only for request-scoped Authorization header construction.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for OpenAiResponsesApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpenAiResponsesApiKey(<redacted>)")
    }
}

/// One validated OpenAI-compatible Responses endpoint target.
#[derive(Clone, Eq, PartialEq)]
pub struct OpenAiResponsesEndpoint {
    target: EndpointUrl,
}

impl OpenAiResponsesEndpoint {
    /// Creates an endpoint by retaining the Base URL path and appending the inference path.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` for an invalid configured URL/path shape. P2-09 still owns
    /// actual scheme/host/CIDR/DNS admission for every later dial attempt.
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

impl fmt::Debug for OpenAiResponsesEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpenAiResponsesEndpoint(<redacted>)")
    }
}

/// One request-ready OpenAI-compatible Responses target, headers, and JSON body.
///
/// This is not an HTTP-client request type. P3-02 consumes its typed target and exact
/// request-scoped header values only through [`Self::into_transport_request`], after P2-09 admits
/// the same target and P3-02 applies its shared-client transport constraints.
#[derive(Eq, PartialEq)]
pub struct OpenAiResponsesOutboundRequest {
    target: EndpointUrl,
    authorization: Zeroizing<String>,
    accept: &'static str,
    body: Vec<u8>,
}

impl OpenAiResponsesOutboundRequest {
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
    /// The Authorization value is request-scoped and must not be logged or persisted by callers.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("accept") {
            Some(self.accept)
        } else if name.eq_ignore_ascii_case("authorization") {
            Some(self.authorization.as_str())
        } else if name.eq_ignore_ascii_case("content-type") {
            Some("application/json")
        } else {
            None
        }
    }

    /// Returns the complete fixed header set in deterministic transport order.
    #[must_use]
    pub fn headers(&self) -> [(&'static str, &str); 3] {
        [
            ("accept", self.accept),
            ("authorization", self.authorization.as_str()),
            ("content-type", "application/json"),
        ]
    }

    /// Returns the complete JSON request body without rendering it to a log representation.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Consumes this request into a DNS-pinned P3-02 transport request.
    ///
    /// The caller must first obtain `admitted_target` by applying P2-09 `EgressPolicy` to this
    /// exact target. A different allowed URL is not interchangeable: it is rejected before the
    /// Authorization header or body can be handed to the HTTP client.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` when the admitted URL does not exactly match this request
    /// target, or `InternalError/Internal` if the fixed P3-01 headers cannot satisfy P3-02's
    /// transport request invariant.
    pub fn into_transport_request(
        self,
        admitted_target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GatewayError> {
        let Self {
            target,
            authorization,
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
                (
                    "authorization".to_owned(),
                    authorization.as_str().to_owned(),
                ),
                ("content-type".to_owned(), "application/json".to_owned()),
            ],
            body,
        )
        .map_err(|_| internal_error())
    }
}

impl fmt::Debug for OpenAiResponsesOutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiResponsesOutboundRequest")
            .field("target", &"<redacted>")
            .field("header_names", &["accept", "authorization", "content-type"])
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Stateless encoder for the first OpenAI-compatible Responses upstream request slice.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OpenAiResponsesRequestBuilder;

impl OpenAiResponsesRequestBuilder {
    /// Builds a request-ready OpenAI-compatible Responses target, standard headers, and JSON body.
    ///
    /// The caller supplies the already selected upstream model; the client-visible requested model
    /// is never forwarded. All extension collisions and unsupported Canonical fields fail closed
    /// rather than being silently discarded.
    ///
    /// # Errors
    ///
    /// Returns a safe `GatewayError` for an invalid upstream model, an unsupported Canonical
    /// representation, extension collision, or JSON construction invariant failure.
    pub fn build(
        endpoint: &OpenAiResponsesEndpoint,
        api_key: &OpenAiResponsesApiKey,
        upstream_model: &str,
        request: &CanonicalRequest,
        mode: ResponseMode,
    ) -> Result<OpenAiResponsesOutboundRequest, GatewayError> {
        if upstream_model.is_empty() {
            return Err(provider_protocol_error());
        }

        let body = encode_body(upstream_model, request, mode)?;
        let accept = match mode {
            ResponseMode::NonStreaming => "application/json",
            ResponseMode::Streaming => "text/event-stream",
        };

        Ok(OpenAiResponsesOutboundRequest {
            target: endpoint.target().clone(),
            authorization: Zeroizing::new(format!("Bearer {}", api_key.as_str())),
            accept,
            body,
        })
    }
}

fn encode_body(
    upstream_model: &str,
    request: &CanonicalRequest,
    mode: ResponseMode,
) -> Result<Vec<u8>, GatewayError> {
    let mut root = Map::new();
    root.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    root.insert(
        "stream".to_owned(),
        Value::Bool(matches!(mode, ResponseMode::Streaming)),
    );

    let input = encode_input(&request.messages)?;
    if !input.is_empty() {
        root.insert("input".to_owned(), Value::Array(input));
    }

    if !request.tools.is_empty() {
        root.insert(
            "tools".to_owned(),
            Value::Array(encode_tools(&request.tools)?),
        );
    }
    if let Some(thinking) = &request.thinking {
        root.insert("reasoning".to_owned(), encode_reasoning(thinking)?);
    }
    if let Some(prompt_cache_key) = &request.prompt_cache_key {
        root.insert(
            "prompt_cache_key".to_owned(),
            Value::String(prompt_cache_key.clone()),
        );
    }
    if let Some(prompt_cache_retention) = &request.prompt_cache_retention {
        root.insert(
            "prompt_cache_retention".to_owned(),
            Value::String(prompt_cache_retention.clone()),
        );
    }

    insert_root_extensions(&mut root, &request.extensions)?;
    serde_json::to_vec(&Value::Object(root)).map_err(|_| internal_error())
}

fn encode_input(messages: &[CanonicalMessage]) -> Result<Vec<Value>, GatewayError> {
    let mut input = Vec::new();

    for message in messages {
        let role = message.role.0.as_str();
        if !matches!(role, "assistant" | "developer" | "system" | "tool" | "user")
            || message.content.is_empty()
        {
            return Err(provider_protocol_error());
        }

        let contains_tool_item = message.content.iter().any(|content| {
            matches!(
                content,
                MessageContent::ToolCall(_) | MessageContent::ToolResult(_)
            )
        });
        if contains_tool_item && !message.extensions.is_empty() {
            return Err(provider_protocol_error());
        }

        let mut message_parts = Vec::new();
        for content in &message.content {
            match content {
                MessageContent::Text(text) => {
                    message_parts.push(encode_text_part(role, text)?);
                }
                MessageContent::Opaque(opaque) => {
                    message_parts.push(encode_opaque_part(opaque.raw(), &opaque.extensions)?);
                }
                MessageContent::ToolCall(call) => {
                    flush_message_parts(&mut input, role, &message.extensions, &mut message_parts)?;
                    if role != "assistant" {
                        return Err(provider_protocol_error());
                    }
                    input.push(encode_tool_call(call)?);
                }
                MessageContent::ToolResult(result) => {
                    flush_message_parts(&mut input, role, &message.extensions, &mut message_parts)?;
                    if role != "tool" || result.is_error {
                        return Err(provider_protocol_error());
                    }
                    input.push(encode_tool_result(result)?);
                }
            }
        }
        flush_message_parts(&mut input, role, &message.extensions, &mut message_parts)?;
    }

    Ok(input)
}

fn flush_message_parts(
    input: &mut Vec<Value>,
    role: &str,
    extensions: &RawExtensions,
    message_parts: &mut Vec<Value>,
) -> Result<(), GatewayError> {
    if message_parts.is_empty() {
        return Ok(());
    }
    if role == "tool" {
        return Err(provider_protocol_error());
    }

    let mut message = Map::new();
    message.insert("type".to_owned(), Value::String("message".to_owned()));
    message.insert("role".to_owned(), Value::String(role.to_owned()));
    message.insert(
        "content".to_owned(),
        Value::Array(std::mem::take(message_parts)),
    );
    insert_extensions(&mut message, extensions, MESSAGE_RESERVED_FIELDS)?;
    input.push(Value::Object(message));
    Ok(())
}

fn encode_text_part(role: &str, text: &gateway_core::TextContent) -> Result<Value, GatewayError> {
    let part_type = match role {
        "assistant" => "output_text",
        "developer" | "system" | "user" => "input_text",
        _ => return Err(provider_protocol_error()),
    };

    let mut part = Map::new();
    part.insert("type".to_owned(), Value::String(part_type.to_owned()));
    part.insert("text".to_owned(), Value::String(text.text.clone()));
    insert_extensions(&mut part, &text.extensions, TEXT_RESERVED_FIELDS)?;
    Ok(Value::Object(part))
}

fn encode_opaque_part(raw: &RawJson, extensions: &RawExtensions) -> Result<Value, GatewayError> {
    let mut part = raw_value(raw)?;
    let Value::Object(ref mut part_object) = part else {
        return Err(provider_protocol_error());
    };
    insert_extensions(part_object, extensions, &[])?;
    Ok(part)
}

fn encode_tool_call(call: &gateway_core::ToolCall) -> Result<Value, GatewayError> {
    if call.id.is_empty() || call.name.is_empty() {
        return Err(provider_protocol_error());
    }

    let mut item = Map::new();
    item.insert("type".to_owned(), Value::String("function_call".to_owned()));
    item.insert("call_id".to_owned(), Value::String(call.id.clone()));
    item.insert("name".to_owned(), Value::String(call.name.clone()));
    item.insert(
        "arguments".to_owned(),
        Value::String(call.arguments.get().to_owned()),
    );
    insert_extensions(&mut item, &call.extensions, TOOL_CALL_RESERVED_FIELDS)?;
    Ok(Value::Object(item))
}

fn encode_tool_result(result: &gateway_core::ToolResult) -> Result<Value, GatewayError> {
    if result.call_id.is_empty() {
        return Err(provider_protocol_error());
    }

    let mut item = Map::new();
    item.insert(
        "type".to_owned(),
        Value::String("function_call_output".to_owned()),
    );
    item.insert("call_id".to_owned(), Value::String(result.call_id.clone()));
    item.insert(
        "output".to_owned(),
        encode_tool_result_output(&result.output)?,
    );
    insert_extensions(&mut item, &result.extensions, TOOL_RESULT_RESERVED_FIELDS)?;
    Ok(Value::Object(item))
}

fn encode_tool_result_output(raw: &RawJson) -> Result<Value, GatewayError> {
    let output = raw_value(raw)?;
    match &output {
        Value::String(_) => Ok(output),
        Value::Array(parts) if parts.iter().all(is_supported_tool_result_content) => Ok(output),
        _ => Err(provider_protocol_error()),
    }
}

fn is_supported_tool_result_content(part: &Value) -> bool {
    let Some(part) = part.as_object() else {
        return false;
    };

    match part.get("type").and_then(Value::as_str) {
        Some("input_text") => part.get("text").is_some_and(Value::is_string),
        Some("input_image") => {
            has_string_field(part, "image_url") || has_string_field(part, "file_id")
        }
        Some("input_file") => {
            has_string_field(part, "file_data")
                || has_string_field(part, "file_id")
                || has_string_field(part, "file_url")
        }
        _ => false,
    }
}

fn has_string_field(object: &Map<String, Value>, name: &str) -> bool {
    object.get(name).is_some_and(Value::is_string)
}

fn encode_tools(tools: &[ToolDefinition]) -> Result<Vec<Value>, GatewayError> {
    tools.iter().map(encode_tool).collect()
}

fn encode_tool(tool: &ToolDefinition) -> Result<Value, GatewayError> {
    if tool.name.is_empty() {
        return Err(provider_protocol_error());
    }
    let parameters = raw_value(&tool.input_schema)?;
    if !parameters.is_object() {
        return Err(provider_protocol_error());
    }

    let mut encoded = Map::new();
    encoded.insert("type".to_owned(), Value::String("function".to_owned()));
    encoded.insert("name".to_owned(), Value::String(tool.name.clone()));
    if let Some(description) = &tool.description {
        encoded.insert("description".to_owned(), Value::String(description.clone()));
    }
    encoded.insert("parameters".to_owned(), parameters);
    insert_extensions(&mut encoded, &tool.extensions, TOOL_RESERVED_FIELDS)?;
    Ok(Value::Object(encoded))
}

fn encode_reasoning(thinking: &gateway_core::Thinking) -> Result<Value, GatewayError> {
    let mut reasoning = Map::new();
    reasoning.insert(
        "effort".to_owned(),
        Value::String(thinking.effort.as_str().to_owned()),
    );
    insert_extensions(
        &mut reasoning,
        &thinking.extensions,
        REASONING_RESERVED_FIELDS,
    )?;
    Ok(Value::Object(reasoning))
}

fn insert_root_extensions(
    root: &mut Map<String, Value>,
    extensions: &RawExtensions,
) -> Result<(), GatewayError> {
    for (name, raw) in extensions.iter() {
        let Some(name) = name.strip_prefix(OPENAI_RESPONSES_EXTENSION_PREFIX) else {
            return Err(provider_protocol_error());
        };
        insert_extension(root, name, raw, ROOT_RESERVED_FIELDS)?;
    }
    Ok(())
}

fn insert_extensions(
    object: &mut Map<String, Value>,
    extensions: &RawExtensions,
    reserved: &[&str],
) -> Result<(), GatewayError> {
    for (name, raw) in extensions.iter() {
        insert_extension(object, name, raw, reserved)?;
    }
    Ok(())
}

fn insert_extension(
    object: &mut Map<String, Value>,
    name: &str,
    raw: &RawJson,
    reserved: &[&str],
) -> Result<(), GatewayError> {
    if name.is_empty() || reserved.contains(&name) || object.contains_key(name) {
        return Err(provider_protocol_error());
    }
    object.insert(name.to_owned(), raw_value(raw)?);
    Ok(())
}

fn raw_value(raw: &RawJson) -> Result<Value, GatewayError> {
    serde_json::from_str(raw.get()).map_err(|_| provider_protocol_error())
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

    use gateway_core::{EgressPolicyId, GatewayErrorCode, MessageContent, RawJson};
    use gateway_upstream::{
        EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput,
        EgressScheme, RedirectPolicy, UpstreamHttpMethod,
    };
    use protocol_openai_responses::{ResponseMode, decode_request};

    use super::{OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesRequestBuilder};

    fn endpoint() -> Result<OpenAiResponsesEndpoint, Box<dyn Error>> {
        Ok(OpenAiResponsesEndpoint::try_new(
            "https://relay.example/v1",
            "/responses",
        )?)
    }

    fn api_key() -> Result<OpenAiResponsesApiKey, Box<dyn Error>> {
        Ok(OpenAiResponsesApiKey::try_new("p3_01_test_credential")?)
    }

    struct StaticPublicResolver;

    impl EgressDnsResolver for StaticPublicResolver {
        fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
            Ok(vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
        }
    }

    fn policy() -> Result<EgressPolicy, Box<dyn Error>> {
        Ok(EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("p3-02-provider-test-policy")?,
            name: "provider test policy".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Https]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("relay.example")?]),
            allowed_ports: BTreeSet::from([443]),
            allowed_cidrs: BTreeSet::new(),
            redirect_policy: RedirectPolicy::Deny,
        })?)
    }

    #[test]
    fn builds_a_lossless_canonical_openai_responses_body_with_an_upstream_model()
    -> Result<(), Box<dyn Error>> {
        let decoded = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;
        let outbound = OpenAiResponsesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            "MiniMax-M3-upstream",
            &decoded.request,
            decoded.mode,
        )?;
        let encoded = std::str::from_utf8(outbound.body())?;
        let rebuilt = decode_request(encoded)?;
        let mut expected = decoded.request.clone();
        expected.requested_model = "MiniMax-M3-upstream".to_owned();

        assert_eq!(outbound.url(), "https://relay.example/v1/responses");
        assert!(!encoded.contains("gateway-model"));
        assert_eq!(rebuilt.request, expected);
        assert_eq!(rebuilt.mode, ResponseMode::Streaming);
        Ok(())
    }

    #[test]
    fn selects_standard_headers_and_redacts_credential_target_and_body()
    -> Result<(), Box<dyn Error>> {
        let decoded = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;
        let streaming = OpenAiResponsesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            "upstream-model",
            &decoded.request,
            ResponseMode::Streaming,
        )?;
        let non_streaming = OpenAiResponsesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            "upstream-model",
            &decoded.request,
            ResponseMode::NonStreaming,
        )?;

        assert_eq!(streaming.header("accept"), Some("text/event-stream"));
        assert_eq!(non_streaming.header("Accept"), Some("application/json"));
        assert_eq!(streaming.header("content-type"), Some("application/json"));
        assert_eq!(
            streaming.header("authorization"),
            Some("Bearer p3_01_test_credential")
        );
        assert_eq!(streaming.header("x-not-configured"), None);

        let debug = format!("{streaming:?}{:?}{:?}", endpoint()?, api_key()?);
        for sensitive in [
            "p3_01_test_credential",
            "relay.example",
            "What is the weather?",
            "upstream-model",
        ] {
            assert!(!debug.contains(sensitive));
        }
        Ok(())
    }

    #[test]
    fn hands_only_the_exact_egress_admitted_request_to_the_shared_transport()
    -> Result<(), Box<dyn Error>> {
        let decoded = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;
        let outbound = OpenAiResponsesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            "upstream-model",
            &decoded.request,
            decoded.mode,
        )?;
        let admitted = policy()?.admit_url(outbound.url(), &StaticPublicResolver)?;
        let transport = outbound.into_transport_request(admitted)?;

        assert_eq!(transport.method(), UpstreamHttpMethod::Post);
        assert_eq!(
            transport
                .header("authorization")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer p3_01_test_credential")
        );
        assert!(std::str::from_utf8(transport.body())?.contains("upstream-model"));
        let debug = format!("{transport:?}");
        assert!(!debug.contains("p3_01_test_credential"));
        assert!(!debug.contains("relay.example"));

        let mismatched_outbound = OpenAiResponsesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            "upstream-model",
            &decoded.request,
            decoded.mode,
        )?;
        let mismatched_target = policy()?.admit_url(
            "https://relay.example/v1/not-responses",
            &StaticPublicResolver,
        )?;
        let mismatch = mismatched_outbound.into_transport_request(mismatched_target);
        assert_eq!(
            mismatch.err().map(|error| error.code()),
            Some(GatewayErrorCode::EgressRejected)
        );
        Ok(())
    }

    #[test]
    fn rejects_unsafe_credential_and_extension_collisions_without_dropping_data()
    -> Result<(), Box<dyn Error>> {
        let invalid_credential = OpenAiResponsesApiKey::try_new("key\r\nmalformed");
        assert_eq!(
            invalid_credential.err().map(|error| error.code()),
            Some(GatewayErrorCode::CredentialUnavailable)
        );

        let mut decoded = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;
        decoded.request.extensions.try_insert(
            "openai.responses.model",
            RawJson::from_json_string("\"override\"".to_owned())?,
        )?;
        let collision = OpenAiResponsesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            "upstream-model",
            &decoded.request,
            decoded.mode,
        );
        assert_eq!(
            collision.err().map(|error| error.code()),
            Some(GatewayErrorCode::UpstreamProtocolError)
        );

        let mut foreign = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;
        foreign.request.extensions.try_insert(
            "foreign.provider.mode",
            RawJson::from_json_string("true".to_owned())?,
        )?;
        let foreign_result = OpenAiResponsesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            "upstream-model",
            &foreign.request,
            foreign.mode,
        );
        assert_eq!(
            foreign_result.err().map(|error| error.code()),
            Some(GatewayErrorCode::UpstreamProtocolError)
        );
        Ok(())
    }

    #[test]
    fn rejects_tool_errors_that_responses_input_cannot_represent_losslessly()
    -> Result<(), Box<dyn Error>> {
        let mut decoded = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;
        let mut found_tool_result = false;
        for message in &mut decoded.request.messages {
            for content in &mut message.content {
                if let MessageContent::ToolResult(result) = content {
                    result.is_error = true;
                    found_tool_result = true;
                }
            }
        }
        assert!(found_tool_result);

        let result = OpenAiResponsesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            "upstream-model",
            &decoded.request,
            decoded.mode,
        );
        assert_eq!(
            result.err().map(|error| error.code()),
            Some(GatewayErrorCode::UpstreamProtocolError)
        );
        Ok(())
    }

    #[test]
    fn preserves_supported_tool_result_content_and_rejects_unmapped_result_values()
    -> Result<(), Box<dyn Error>> {
        let mut decoded = decode_request(include_str!(
            "../../../tests/fixtures/openai-responses/request-canonical.json"
        ))?;
        let rich_output = RawJson::from_json_string(
            r#"[{"type":"input_text","text":"clear"},{"type":"input_file","file_id":"file-01"}]"#
                .to_owned(),
        )?;
        let mut found_tool_result = false;
        for message in &mut decoded.request.messages {
            for content in &mut message.content {
                if let MessageContent::ToolResult(result) = content {
                    result.output = rich_output.clone();
                    found_tool_result = true;
                }
            }
        }
        assert!(found_tool_result);

        let outbound = OpenAiResponsesRequestBuilder::build(
            &endpoint()?,
            &api_key()?,
            "upstream-model",
            &decoded.request,
            decoded.mode,
        )?;
        let rebuilt = decode_request(std::str::from_utf8(outbound.body())?)?;
        let mut expected = decoded.request.clone();
        expected.requested_model = "upstream-model".to_owned();
        assert_eq!(
            serde_json::to_value(&rebuilt.request)?,
            serde_json::to_value(&expected)?
        );

        for unsupported_output in [
            r#"{"forecast":"clear"}"#,
            r#"[{"type":"input_text"}]"#,
            r#"[{"type":"input_image"}]"#,
            r#"[{"type":"input_file"}]"#,
            r#"[{"type":"reasoning","text":"not a tool result"}]"#,
        ] {
            for message in &mut decoded.request.messages {
                for content in &mut message.content {
                    if let MessageContent::ToolResult(result) = content {
                        result.output = RawJson::from_json_string(unsupported_output.to_owned())?;
                    }
                }
            }

            let unsupported = OpenAiResponsesRequestBuilder::build(
                &endpoint()?,
                &api_key()?,
                "upstream-model",
                &decoded.request,
                decoded.mode,
            );
            assert_eq!(
                unsupported.err().map(|error| error.code()),
                Some(GatewayErrorCode::UpstreamProtocolError)
            );
        }
        Ok(())
    }
}
