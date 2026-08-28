//! Strict OpenAI-compatible Chat Completions request construction.

use std::fmt;

use gateway_core::{
    CanonicalMessage, CanonicalRequest, ErrorScope, GatewayError, GatewayErrorCode, MessageContent,
    RawExtensions, RawJson, ToolDefinition,
};
use gateway_upstream::{
    AdmittedEgressTarget, EndpointUrl, UpstreamHttpMethod, UpstreamHttpRequest,
};
use protocol_openai_chat::ResponseMode;
use serde_json::{Map, Value};
use zeroize::Zeroizing;

const ROOT_PREFIX: &str = "openai.chat.";
const MESSAGE_PREFIX: &str = "openai.chat.message.";
const TOOL_PREFIX: &str = "openai.chat.tool.";
const TOOL_CALL_PREFIX: &str = "openai.chat.tool_call.";
const ROOT_RESERVED: &[&str] = &["model", "messages", "stream", "stream_options", "tools"];
const MESSAGE_RESERVED: &[&str] = &["role", "content", "tool_calls", "tool_call_id"];
const TOOL_RESERVED: &[&str] = &["name", "description", "parameters"];
const TOOL_CALL_RESERVED: &[&str] = &["id", "type", "function"];

/// A request-scoped upstream bearer credential.
pub struct OpenAiChatCompletionsApiKey(Zeroizing<String>);

impl OpenAiChatCompletionsApiKey {
    /// Creates one non-empty HTTP-header-safe credential.
    ///
    /// # Errors
    ///
    /// Returns `CredentialUnavailable/Credential` for empty or non-visible-ASCII input.
    pub fn try_new(value: impl Into<String>) -> Result<Self, GatewayError> {
        let value = value.into();
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(credential_error());
        }
        Ok(Self(Zeroizing::new(value)))
    }

    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for OpenAiChatCompletionsApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpenAiChatCompletionsApiKey(<redacted>)")
    }
}

/// One validated OpenAI-compatible Chat Completions endpoint target.
#[derive(Clone, Eq, PartialEq)]
pub struct OpenAiChatCompletionsEndpoint {
    target: EndpointUrl,
}

impl OpenAiChatCompletionsEndpoint {
    /// Retains the configured Base URL path and appends the exact inference path.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` for an invalid URL or path composition.
    pub fn try_new(base_url: &str, inference_path: &str) -> Result<Self, GatewayError> {
        let target = EndpointUrl::compose(base_url, inference_path).map_err(|_| egress_error())?;
        Ok(Self { target })
    }

    /// Returns the complete target URL for egress admission.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }

    /// Returns the parsed target without re-parsing it.
    #[must_use]
    pub fn target(&self) -> &EndpointUrl {
        &self.target
    }
}

impl fmt::Debug for OpenAiChatCompletionsEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpenAiChatCompletionsEndpoint(<redacted>)")
    }
}

/// Request-ready Chat target, headers and JSON body.
#[derive(Eq, PartialEq)]
pub struct OpenAiChatCompletionsOutboundRequest {
    target: EndpointUrl,
    authorization: Zeroizing<String>,
    accept: &'static str,
    body: Vec<u8>,
}

impl OpenAiChatCompletionsOutboundRequest {
    /// Returns the complete endpoint URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.target.as_str()
    }

    /// Returns the exact request body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Returns one fixed header by case-insensitive name.
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

    /// Hands this request to the shared transport only when admission covered this exact URL.
    ///
    /// # Errors
    ///
    /// Returns `EgressRejected/Egress` for a mismatched admitted target or `InternalError` when
    /// the fixed request cannot satisfy the shared transport invariant.
    pub fn into_transport_request(
        self,
        admitted_target: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GatewayError> {
        if admitted_target.request_url() != self.target.as_url() {
            return Err(egress_error());
        }
        UpstreamHttpRequest::try_new(
            admitted_target,
            UpstreamHttpMethod::Post,
            [
                ("accept".to_owned(), self.accept.to_owned()),
                (
                    "authorization".to_owned(),
                    self.authorization.as_str().to_owned(),
                ),
                ("content-type".to_owned(), "application/json".to_owned()),
            ],
            self.body,
        )
        .map_err(|_| internal_error())
    }
}

impl fmt::Debug for OpenAiChatCompletionsOutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiChatCompletionsOutboundRequest")
            .field("target", &"<redacted>")
            .field("header_names", &["accept", "authorization", "content-type"])
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Stateless Canonical-to-Chat request encoder.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OpenAiChatCompletionsRequestBuilder;

impl OpenAiChatCompletionsRequestBuilder {
    /// Preserves a strictly decoded native Chat request while replacing only its upstream model.
    ///
    /// This mirrors the incumbent CPA's native OpenAI-compatible path: protocol-specific fields
    /// remain intact instead of taking a lossy round-trip through Canonical. Unlike the incumbent,
    /// this boundary re-runs the strict Chat decoder before accepting the payload and fails closed
    /// when the caller's response mode disagrees with the body.
    ///
    /// # Errors
    ///
    /// Returns a safe Provider protocol error for malformed or mode-mismatched native input.
    pub fn build_native(
        endpoint: &OpenAiChatCompletionsEndpoint,
        api_key: &OpenAiChatCompletionsApiKey,
        upstream_model: &str,
        native_body: &[u8],
        mode: ResponseMode,
    ) -> Result<OpenAiChatCompletionsOutboundRequest, GatewayError> {
        if upstream_model.is_empty() {
            return Err(protocol_error());
        }
        let native_body = std::str::from_utf8(native_body).map_err(|_| protocol_error())?;
        let decoded =
            protocol_openai_chat::decode_request(native_body).map_err(|_| protocol_error())?;
        if decoded.mode != mode {
            return Err(protocol_error());
        }
        let mut root: Value = serde_json::from_str(native_body).map_err(|_| protocol_error())?;
        let root = root.as_object_mut().ok_or_else(protocol_error)?;
        root.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
        let body = serde_json::to_vec(root).map_err(|_| internal_error())?;
        Ok(outbound(endpoint, api_key, mode, body))
    }

    /// Builds one fail-closed Chat Completions request using the selected upstream model.
    ///
    /// # Errors
    ///
    /// Returns a safe Provider protocol error for unsupported Canonical fields, foreign
    /// extensions, reserved collisions, or an invalid selected model.
    pub fn build(
        endpoint: &OpenAiChatCompletionsEndpoint,
        api_key: &OpenAiChatCompletionsApiKey,
        upstream_model: &str,
        request: &CanonicalRequest,
        mode: ResponseMode,
    ) -> Result<OpenAiChatCompletionsOutboundRequest, GatewayError> {
        if upstream_model.is_empty()
            || request.thinking.is_some()
            || request.prompt_cache_key.is_some()
            || request.prompt_cache_retention.is_some()
            || request.messages.is_empty()
        {
            return Err(protocol_error());
        }

        let mut root = Map::new();
        root.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
        root.insert(
            "messages".to_owned(),
            Value::Array(
                request
                    .messages
                    .iter()
                    .map(encode_message)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        );
        root.insert(
            "stream".to_owned(),
            Value::Bool(matches!(mode, ResponseMode::Streaming)),
        );
        if matches!(mode, ResponseMode::Streaming) {
            root.insert(
                "stream_options".to_owned(),
                Value::Object(Map::from_iter([(
                    "include_usage".to_owned(),
                    Value::Bool(true),
                )])),
            );
        }
        if !request.tools.is_empty() {
            root.insert(
                "tools".to_owned(),
                Value::Array(
                    request
                        .tools
                        .iter()
                        .map(encode_tool)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            );
        }
        insert_prefixed(&mut root, &request.extensions, ROOT_PREFIX, ROOT_RESERVED)?;
        let body = serde_json::to_vec(&Value::Object(root)).map_err(|_| internal_error())?;

        Ok(outbound(endpoint, api_key, mode, body))
    }
}

fn outbound(
    endpoint: &OpenAiChatCompletionsEndpoint,
    api_key: &OpenAiChatCompletionsApiKey,
    mode: ResponseMode,
    body: Vec<u8>,
) -> OpenAiChatCompletionsOutboundRequest {
    OpenAiChatCompletionsOutboundRequest {
        target: endpoint.target().clone(),
        authorization: Zeroizing::new(format!("Bearer {}", api_key.as_str())),
        accept: match mode {
            ResponseMode::NonStreaming => "application/json",
            ResponseMode::Streaming => "text/event-stream",
        },
        body,
    }
}

fn encode_message(message: &CanonicalMessage) -> Result<Value, GatewayError> {
    let role = message.role.0.as_str();
    if !matches!(role, "system" | "developer" | "user" | "assistant" | "tool") {
        return Err(protocol_error());
    }
    let mut encoded = Map::new();
    encoded.insert("role".to_owned(), Value::String(role.to_owned()));

    match role {
        "system" | "developer" | "user" => {
            let [MessageContent::Text(text)] = message.content.as_slice() else {
                return Err(protocol_error());
            };
            if !text.extensions.is_empty() {
                return Err(protocol_error());
            }
            encoded.insert("content".to_owned(), Value::String(text.text.clone()));
        }
        "assistant" => encode_assistant_content(&mut encoded, &message.content)?,
        "tool" => {
            let [MessageContent::ToolResult(result)] = message.content.as_slice() else {
                return Err(protocol_error());
            };
            if result.call_id.is_empty() || result.is_error || !result.extensions.is_empty() {
                return Err(protocol_error());
            }
            let Value::String(output) = raw_value(&result.output)? else {
                return Err(protocol_error());
            };
            encoded.insert(
                "tool_call_id".to_owned(),
                Value::String(result.call_id.clone()),
            );
            encoded.insert("content".to_owned(), Value::String(output));
        }
        _ => unreachable!(),
    }
    insert_prefixed(
        &mut encoded,
        &message.extensions,
        MESSAGE_PREFIX,
        MESSAGE_RESERVED,
    )?;
    Ok(Value::Object(encoded))
}

fn encode_assistant_content(
    encoded: &mut Map<String, Value>,
    content: &[MessageContent],
) -> Result<(), GatewayError> {
    let mut text = None;
    let mut calls = Vec::new();
    for (index, part) in content.iter().enumerate() {
        match part {
            MessageContent::Text(value) if index == 0 && text.is_none() => {
                if !value.extensions.is_empty() {
                    return Err(protocol_error());
                }
                text = Some(value.text.clone());
            }
            MessageContent::ToolCall(call) => calls.push(encode_tool_call(call)?),
            _ => return Err(protocol_error()),
        }
    }
    if text.is_none() && calls.is_empty() {
        return Err(protocol_error());
    }
    encoded.insert(
        "content".to_owned(),
        text.map_or(Value::Null, Value::String),
    );
    if !calls.is_empty() {
        encoded.insert("tool_calls".to_owned(), Value::Array(calls));
    }
    Ok(())
}

fn encode_tool_call(call: &gateway_core::ToolCall) -> Result<Value, GatewayError> {
    if call.id.is_empty() || call.name.is_empty() {
        return Err(protocol_error());
    }
    let mut encoded = Map::new();
    encoded.insert("id".to_owned(), Value::String(call.id.clone()));
    encoded.insert("type".to_owned(), Value::String("function".to_owned()));
    encoded.insert(
        "function".to_owned(),
        Value::Object(Map::from_iter([
            ("name".to_owned(), Value::String(call.name.clone())),
            (
                "arguments".to_owned(),
                Value::String(call.arguments.get().to_owned()),
            ),
        ])),
    );
    insert_prefixed(
        &mut encoded,
        &call.extensions,
        TOOL_CALL_PREFIX,
        TOOL_CALL_RESERVED,
    )?;
    Ok(Value::Object(encoded))
}

fn encode_tool(tool: &ToolDefinition) -> Result<Value, GatewayError> {
    if tool.name.is_empty() {
        return Err(protocol_error());
    }
    let parameters = raw_value(&tool.input_schema)?;
    if !parameters.is_object() {
        return Err(protocol_error());
    }
    let mut function = Map::new();
    function.insert("name".to_owned(), Value::String(tool.name.clone()));
    if let Some(description) = &tool.description {
        function.insert("description".to_owned(), Value::String(description.clone()));
    }
    function.insert("parameters".to_owned(), parameters);
    insert_prefixed(&mut function, &tool.extensions, TOOL_PREFIX, TOOL_RESERVED)?;
    Ok(Value::Object(Map::from_iter([
        ("type".to_owned(), Value::String("function".to_owned())),
        ("function".to_owned(), Value::Object(function)),
    ])))
}

fn insert_prefixed(
    object: &mut Map<String, Value>,
    extensions: &RawExtensions,
    prefix: &str,
    reserved: &[&str],
) -> Result<(), GatewayError> {
    for (name, raw) in extensions.iter() {
        let Some(name) = name.strip_prefix(prefix) else {
            return Err(protocol_error());
        };
        if name.is_empty() || reserved.contains(&name) || object.contains_key(name) {
            return Err(protocol_error());
        }
        object.insert(name.to_owned(), raw_value(raw)?);
    }
    Ok(())
}

fn raw_value(raw: &RawJson) -> Result<Value, GatewayError> {
    serde_json::from_str(raw.get()).map_err(|_| protocol_error())
}

const fn credential_error() -> GatewayError {
    GatewayError::new(
        GatewayErrorCode::CredentialUnavailable,
        ErrorScope::Credential,
    )
}

const fn egress_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
}

const fn protocol_error() -> GatewayError {
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

    use gateway_core::{EgressPolicyId, GatewayError, GatewayErrorCode, RawExtensions, RawJson};
    use gateway_upstream::{
        EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput,
        EgressScheme, RedirectPolicy, UpstreamHttpMethod,
    };
    use protocol_openai_chat::{ResponseMode, decode_request};

    use super::{
        OpenAiChatCompletionsApiKey, OpenAiChatCompletionsEndpoint,
        OpenAiChatCompletionsRequestBuilder,
    };

    struct Resolver;
    impl EgressDnsResolver for Resolver {
        fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
            Ok(vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
        }
    }

    fn endpoint() -> Result<OpenAiChatCompletionsEndpoint, GatewayError> {
        OpenAiChatCompletionsEndpoint::try_new("https://relay.example/v1", "/chat/completions")
    }

    fn policy() -> Result<EgressPolicy, Box<dyn Error>> {
        Ok(EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("p12-08c-chat-policy")?,
            name: "chat test".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Https]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("relay.example")?]),
            allowed_ports: BTreeSet::from([443]),
            allowed_cidrs: BTreeSet::new(),
            redirect_policy: RedirectPolicy::Deny,
        })?)
    }

    fn decoded() -> Result<protocol_openai_chat::DecodedChatRequest, GatewayError> {
        decode_request(
            r#"{
          "model":"public-model","stream":true,"stream_options":{"include_usage":true},
          "temperature":0.2,
          "messages":[
            {"role":"system","content":"be exact"},
            {"role":"assistant","content":null,"tool_calls":[{"id":"call-1","type":"function","function":{"name":"lookup","arguments":"{\"q\":\"x\"}"}}]},
            {"role":"tool","tool_call_id":"call-1","content":"done"},
            {"role":"user","content":"continue"}
          ],
          "tools":[{"type":"function","function":{"name":"lookup","description":"lookup","parameters":{"type":"object"}}}]
        }"#,
        )
    }

    #[test]
    fn round_trips_native_chat_semantics_with_selected_model() -> Result<(), Box<dyn Error>> {
        let decoded = decoded()?;
        let outbound = OpenAiChatCompletionsRequestBuilder::build(
            &endpoint()?,
            &OpenAiChatCompletionsApiKey::try_new("secret")?,
            "upstream-model",
            &decoded.request,
            decoded.mode,
        )?;
        let rebuilt = decode_request(std::str::from_utf8(outbound.body())?)?;
        let mut expected = decoded.request;
        expected.requested_model = "upstream-model".to_owned();
        assert_eq!(rebuilt.request, expected);
        assert_eq!(rebuilt.mode, ResponseMode::Streaming);
        assert!(rebuilt.include_usage);
        assert_eq!(outbound.url(), "https://relay.example/v1/chat/completions");
        Ok(())
    }

    #[test]
    fn native_path_preserves_incumbent_compatible_fields_and_changes_only_model()
    -> Result<(), Box<dyn Error>> {
        let original = br#"{
          "model":"public-model","stream":false,"temperature":0.2,"seed":7,
          "messages":[{"role":"user","content":"hello"}]
        }"#;
        let outbound = OpenAiChatCompletionsRequestBuilder::build_native(
            &endpoint()?,
            &OpenAiChatCompletionsApiKey::try_new("secret")?,
            "upstream-model",
            original,
            ResponseMode::NonStreaming,
        )?;
        let value: serde_json::Value = serde_json::from_slice(outbound.body())?;
        assert_eq!(value["model"], "upstream-model");
        assert_eq!(value["temperature"], 0.2);
        assert_eq!(value["seed"], 7);
        assert_eq!(value["messages"][0]["content"], "hello");

        assert!(
            OpenAiChatCompletionsRequestBuilder::build_native(
                &endpoint()?,
                &OpenAiChatCompletionsApiKey::try_new("secret")?,
                "upstream-model",
                original,
                ResponseMode::Streaming,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn exact_admitted_target_is_required_and_values_are_redacted() -> Result<(), Box<dyn Error>> {
        let decoded = decoded()?;
        let endpoint = endpoint()?;
        let credential = OpenAiChatCompletionsApiKey::try_new("secret")?;
        let outbound = OpenAiChatCompletionsRequestBuilder::build(
            &endpoint,
            &credential,
            "upstream-model",
            &decoded.request,
            decoded.mode,
        )?;
        assert_eq!(outbound.header("accept"), Some("text/event-stream"));
        assert_eq!(outbound.header("authorization"), Some("Bearer secret"));
        let admitted = policy()?.admit_url(outbound.url(), &Resolver)?;
        let transport = outbound.into_transport_request(admitted)?;
        assert_eq!(transport.method(), UpstreamHttpMethod::Post);

        let debug = format!("{endpoint:?}{credential:?}{transport:?}");
        for secret in ["secret", "relay.example", "upstream-model", "continue"] {
            assert!(!debug.contains(secret));
        }

        let outbound = OpenAiChatCompletionsRequestBuilder::build(
            &endpoint,
            &OpenAiChatCompletionsApiKey::try_new("secret")?,
            "upstream-model",
            &decoded.request,
            ResponseMode::NonStreaming,
        )?;
        let mismatch = policy()?.admit_url("https://relay.example/v1/responses", &Resolver)?;
        assert_eq!(
            outbound
                .into_transport_request(mismatch)
                .err()
                .map(|error| error.code()),
            Some(GatewayErrorCode::EgressRejected)
        );
        Ok(())
    }

    #[test]
    fn rejects_unsafe_credentials_foreign_extensions_and_reserved_collisions()
    -> Result<(), Box<dyn Error>> {
        assert_eq!(
            OpenAiChatCompletionsApiKey::try_new("bad\r\nkey")
                .err()
                .map(|error| error.code()),
            Some(GatewayErrorCode::CredentialUnavailable)
        );
        let mut decoded = decoded()?;
        decoded.request.extensions.try_insert(
            "anthropic.temperature",
            RawJson::from_json_string("0.3".to_owned())?,
        )?;
        assert_eq!(
            OpenAiChatCompletionsRequestBuilder::build(
                &endpoint()?,
                &OpenAiChatCompletionsApiKey::try_new("secret")?,
                "upstream-model",
                &decoded.request,
                ResponseMode::NonStreaming,
            )
            .err()
            .map(|error| error.code()),
            Some(GatewayErrorCode::UpstreamProtocolError)
        );
        decoded.request.extensions = RawExtensions::default();
        decoded.request.extensions.try_insert(
            "openai.chat.model",
            RawJson::from_json_string("\"override\"".to_owned())?,
        )?;
        assert!(
            OpenAiChatCompletionsRequestBuilder::build(
                &endpoint()?,
                &OpenAiChatCompletionsApiKey::try_new("secret")?,
                "upstream-model",
                &decoded.request,
                ResponseMode::NonStreaming,
            )
            .is_err()
        );
        Ok(())
    }
}
