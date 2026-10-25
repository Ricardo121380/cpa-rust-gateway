//! CPA v8.0.40 counting, adapted from `claude_executor_tokens.go` and
//! `helps/claude_input_tokens.go` at d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683.
//! Copyright (c) 2025-2005.9 Luis Pater; (c) 2025.9-present Router-For.ME.
//! MIT license and attribution: `THIRD_PARTY_NOTICES.md` at the repository root.

use std::{collections::BTreeMap, sync::OnceLock};

use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode, InputTokenCount, RawJson};
use gateway_provider::token_count_unsupported;
use gateway_upstream::{AdmittedEgressTarget, UpstreamHttpMethod, UpstreamHttpRequest};
use serde_json::value::RawValue;

use crate::{ANTHROPIC_VERSION, AnthropicMessagesAuthorization, AnthropicMessagesEndpoint};

type Object<'a> = BTreeMap<String, &'a RawValue>;

/// Validated supported Messages input, retaining original JSON spelling and object order.
/// Only text, thinking, ordinary tool definitions/calls and text tool results are supported.
/// Its debug representation never reveals prompt content.
pub struct MessagesCountInput {
    payload: String,
    segments: Vec<String>,
}

impl std::fmt::Debug for MessagesCountInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MessagesCountInput(<redacted>)")
    }
}

impl MessagesCountInput {
    /// Validates a duplicate-checked public Messages count payload without silently losing content.
    ///
    /// # Errors
    /// Malformed inputs fail as client errors; unsupported fields/content fail explicitly.
    pub fn parse(payload: &str) -> Result<Self, GatewayError> {
        if payload.len() > 256 * 1024 {
            return Err(client_error());
        }
        // Also protects direct callers; public ingress has already performed this decode.
        protocol_anthropic::decode_count_tokens_request(payload)?;
        let root = object(payload)?;
        fields(
            &root,
            &[
                "model",
                "messages",
                "system",
                "tools",
                "tool_choice",
                "thinking",
                "stream",
                "max_tokens",
                "temperature",
                "top_p",
                "top_k",
                "stop_sequences",
                "metadata",
                "context_management",
                "diagnostics",
                "output_config",
            ],
        )?;
        validate_controls(&root)?;
        let mut segments = Vec::new();
        if let Some(system) = root.get("system") {
            content(system, &mut segments, true)?;
        }
        for message in array(required(&root, "messages")?)? {
            let message = object(message.get())?;
            fields(&message, &["role", "content"])?;
            push_string(required(&message, "role")?, &mut segments)?;
            content(required(&message, "content")?, &mut segments, false)?;
        }
        if let Some(tools) = root.get("tools") {
            for tool in array(tools)? {
                let tool = object(tool.get())?;
                fields(
                    &tool,
                    &["name", "description", "input_schema", "cache_control"],
                )?;
                push_string(required(&tool, "name")?, &mut segments)?;
                if let Some(description) = tool.get("description") {
                    push_string(description, &mut segments)?;
                }
                let schema = required(&tool, "input_schema")?;
                object(schema.get())?;
                segments.push(compact(schema.get()));
            }
        }
        if let Some(choice) = root.get("tool_choice") {
            if choice.get().starts_with('"') {
                push_string(choice, &mut segments)?;
            } else {
                let choice = object(choice.get())?;
                fields(&choice, &["type", "name", "disable_parallel_tool_use"])?;
                push_string(required(&choice, "type")?, &mut segments)?;
                if let Some(name) = choice.get("name") {
                    push_string(name, &mut segments)?;
                }
            }
        }
        Ok(Self {
            payload: payload.to_owned(),
            segments,
        })
    }

    /// Applies CPA's fixed segment joining and `O200kBase` tokenizer as an explicit estimate.
    ///
    /// # Errors
    /// Tokenizer initialization/count conversion failures are safe errors, never zero fallbacks.
    pub fn estimate(&self) -> Result<InputTokenCount, GatewayError> {
        static TOKENIZER: OnceLock<Result<tiktoken_rs::CoreBPE, GatewayError>> = OnceLock::new();
        let tokenizer =
            TOKENIZER.get_or_init(|| tiktoken_rs::o200k_base().map_err(|_| internal_error()));
        let tokenizer = tokenizer.as_ref().map_err(Clone::clone)?;
        let count = tokenizer.count_ordinary(&self.segments.join("\n"));
        Ok(InputTokenCount::Estimated(
            u64::try_from(count).map_err(|_| internal_error())?,
        ))
    }

    /// Builds the count API request after the exact URL has passed shared egress admission.
    /// No inference request is constructed or sent.
    ///
    /// # Errors
    /// Rejects target mismatch, unsupported first-party path or malformed header/body construction.
    pub fn upstream_request(
        &self,
        endpoint: &AnthropicMessagesEndpoint,
        authorization: &AnthropicMessagesAuthorization,
        model: &str,
        admitted: AdmittedEgressTarget,
    ) -> Result<UpstreamHttpRequest, GatewayError> {
        let target = upstream_count_target(endpoint)?.ok_or_else(token_count_unsupported)?;
        if target != admitted.request_url().as_str() {
            return Err(GatewayError::new(
                GatewayErrorCode::EgressRejected,
                ErrorScope::Egress,
            ));
        }
        let mut root: BTreeMap<String, RawJson> =
            serde_json::from_str(&self.payload).map_err(|_| client_error())?;
        for name in [
            "metadata",
            "context_management",
            "diagnostics",
            "stream",
            "max_tokens",
            "temperature",
            "top_p",
            "top_k",
            "stop_sequences",
            "output_config",
        ] {
            root.remove(name);
        }
        root.insert(
            "model".into(),
            RawJson::from_json_string(serde_json::to_string(model).map_err(|_| internal_error())?)
                .map_err(|_| internal_error())?,
        );
        let mut beta = "token-counting-2024-11-01".to_owned();
        if authorization.header_name() == "authorization" {
            beta.push_str(",oauth-2025-04-20");
        }
        let headers = [
            ("content-type".into(), "application/json".into()),
            ("accept".into(), "application/json".into()),
            ("anthropic-version".into(), ANTHROPIC_VERSION.into()),
            ("anthropic-beta".into(), beta),
            (
                authorization.header_name().into(),
                authorization.header_value().into(),
            ),
        ];
        UpstreamHttpRequest::try_new(
            admitted,
            UpstreamHttpMethod::Post,
            headers,
            serde_json::to_vec(&root).map_err(|_| internal_error())?,
        )
        .map_err(|_| internal_error())
    }
}

/// Identifies the supported first-party Anthropic count target by origin, never credential text.
/// Custom origins use local estimation. Nonstandard first-party paths are explicitly unsupported.
///
/// # Errors
/// Rejects unsupported first-party paths instead of falling back to an estimate.
pub fn upstream_count_target(
    endpoint: &AnthropicMessagesEndpoint,
) -> Result<Option<&'static str>, GatewayError> {
    let url = endpoint.target().as_url();
    if url.scheme() != "https"
        || url.host_str() != Some("api.anthropic.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Ok(None);
    }
    if url.path() != "/v1/messages" || url.query().is_some() {
        return Err(token_count_unsupported());
    }
    Ok(Some(
        "https://api.anthropic.com/v1/messages/count_tokens?beta=true",
    ))
}

/// Validates an upstream count response without treating it as Usage or accepting an absent zero.
///
/// # Errors
/// Missing, duplicate, negative, fractional or overflowing input counts fail safely.
pub fn decode_upstream_count(body: &[u8]) -> Result<InputTokenCount, GatewayError> {
    let error = || {
        GatewayError::new(
            GatewayErrorCode::UpstreamProtocolError,
            ErrorScope::Provider,
        )
    };
    let root: gateway_core::RawExtensions = serde_json::from_slice(body).map_err(|_| error())?;
    let count = root.get("input_tokens").ok_or_else(error)?;
    let count: u64 = serde_json::from_str(count.get()).map_err(|_| error())?;
    Ok(InputTokenCount::ProviderReported(count))
}

fn validate_controls(root: &Object<'_>) -> Result<(), GatewayError> {
    for name in [
        "max_tokens",
        "temperature",
        "top_p",
        "top_k",
        "stop_sequences",
        "metadata",
        "context_management",
        "diagnostics",
        "output_config",
    ] {
        let Some(raw) = root.get(name) else { continue };
        let value: serde_json::Value =
            serde_json::from_str(raw.get()).map_err(|_| client_error())?;
        let valid = match name {
            "max_tokens" => value.as_u64().is_some_and(|n| n > 0),
            "top_k" => value.as_u64().is_some(),
            "temperature" | "top_p" => value.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)),
            "stop_sequences" => value
                .as_array()
                .is_some_and(|items| items.iter().all(serde_json::Value::is_string)),
            _ => value.is_object(),
        };
        if !valid {
            return Err(client_error());
        }
    }
    if let Some(raw) = root.get("tool_choice") {
        let value: serde_json::Value =
            serde_json::from_str(raw.get()).map_err(|_| client_error())?;
        let kind = value
            .as_str()
            .or_else(|| value.get("type").and_then(serde_json::Value::as_str))
            .ok_or_else(client_error)?;
        if !matches!(kind, "auto" | "any" | "none" | "tool")
            || kind == "tool"
                && value
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(str::is_empty)
            || value
                .get("disable_parallel_tool_use")
                .is_some_and(|v| !v.is_boolean())
        {
            return Err(client_error());
        }
    }
    Ok(())
}

fn object(raw: &str) -> Result<Object<'_>, GatewayError> {
    serde_json::from_str(raw).map_err(|_| client_error())
}
fn array(raw: &RawValue) -> Result<Vec<&RawValue>, GatewayError> {
    serde_json::from_str(raw.get()).map_err(|_| client_error())
}
fn required<'a>(object: &Object<'a>, name: &str) -> Result<&'a RawValue, GatewayError> {
    object.get(name).copied().ok_or_else(client_error)
}
fn fields(object: &Object<'_>, allowed: &[&str]) -> Result<(), GatewayError> {
    if object.keys().any(|name| !allowed.contains(&name.as_str())) {
        return Err(token_count_unsupported());
    }
    Ok(())
}
fn push_string(raw: &RawValue, segments: &mut Vec<String>) -> Result<(), GatewayError> {
    let value: String = serde_json::from_str(raw.get()).map_err(|_| client_error())?;
    let value = value.trim();
    if !value.is_empty() {
        segments.push(value.into());
    }
    Ok(())
}
fn content(raw: &RawValue, segments: &mut Vec<String>, system: bool) -> Result<(), GatewayError> {
    if raw.get().starts_with('"') {
        return push_string(raw, segments);
    }
    for part in array(raw)? {
        content_block(part, segments, system)?;
    }
    Ok(())
}

fn content_block(
    raw: &RawValue,
    segments: &mut Vec<String>,
    system: bool,
) -> Result<(), GatewayError> {
    let block = object(raw.get())?;
    let kind: String =
        serde_json::from_str(required(&block, "type")?.get()).map_err(|_| client_error())?;
    match kind.as_str() {
        "text" => {
            fields(&block, &["type", "text", "cache_control"])?;
            push_string(required(&block, "text")?, segments)?;
        }
        "thinking" if !system => {
            fields(&block, &["type", "thinking", "signature"])?;
            push_string(required(&block, "thinking")?, segments)?;
        }
        "tool_use" if !system => {
            fields(&block, &["type", "id", "name", "input", "cache_control"])?;
            push_string(required(&block, "id")?, segments)?;
            push_string(required(&block, "name")?, segments)?;
            let input = required(&block, "input")?;
            object(input.get())?;
            segments.push(compact(input.get()));
        }
        "tool_result" if !system => {
            fields(
                &block,
                &[
                    "type",
                    "tool_use_id",
                    "content",
                    "is_error",
                    "cache_control",
                ],
            )?;
            push_string(required(&block, "tool_use_id")?, segments)?;
            // Tool results contain text only in this slice; nested tool/thinking/image blocks fail.
            content(required(&block, "content")?, segments, true)?;
        }
        _ => return Err(token_count_unsupported()),
    }
    Ok(())
}

// Like Go json.Compact: retain object ordering, number spelling and string escapes.
fn compact(raw: &str) -> String {
    let mut quoted = false;
    let mut escaped = false;
    raw.chars()
        .filter(|&ch| {
            if quoted {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    quoted = false;
                }
                true
            } else {
                if ch == '"' {
                    quoted = true;
                }
                !matches!(ch, ' ' | '\n' | '\r' | '\t')
            }
        })
        .collect()
}
const fn client_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}
const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn count_estimates_match_measured_cpa_v8_fixtures() -> Result<(), Box<dyn std::error::Error>> {
        let fixtures: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/count-tokens/cpa-v8.json"
        ))?;
        for case in fixtures["cases"].as_array().ok_or("cases")? {
            let input = MessagesCountInput::parse(case["request"].as_str().ok_or("request")?)?;
            assert_eq!(
                input.estimate()?.input_tokens(),
                case["expected_input_tokens"].as_u64().ok_or("count")?,
                "{}",
                case["name"]
            );
        }
        Ok(())
    }

    #[test]
    fn count_provenance_and_strict_upstream_response() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            decode_upstream_count(br#"{"input_tokens":17}"#)?,
            InputTokenCount::ProviderReported(17)
        );
        assert_eq!(
            decode_upstream_count(br#"{"input_tokens":0}"#)?,
            InputTokenCount::ProviderReported(0)
        );
        for raw in [
            "{}",
            r#"{"input_tokens":-1}"#,
            r#"{"input_tokens":1.5}"#,
            r#"{"input_tokens":17,"input_tokens":18}"#,
            r#"{"input_tokens":18446744073709551616}"#,
        ] {
            assert!(decode_upstream_count(raw.as_bytes()).is_err());
        }
        Ok(())
    }
    #[test]
    fn count_segments_preserve_cpa_order_and_reject_unsupported_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let input = MessagesCountInput::parse(
            r#"{"model":"alias","system":"  Be brief  ","messages":[{"role":"user","content":"hi"},{"role":"assistant","content":[{"type":"tool_use","id":"call_1","name":"read","input":{ "z":1e2, "a":"x y" }}]},{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_1","content":[{"type":"text","text":"ok"}]}]}],"tools":[{"name":"read","description":" file ","input_schema":{"type":"object"}}],"tool_choice":{"type":"auto"}}"#,
        )?;
        assert_eq!(
            input.segments.join("\n"),
            "Be brief\nuser\nhi\nassistant\ncall_1\nread\n{\"z\":1e2,\"a\":\"x y\"}\nuser\ncall_1\nok\nread\nfile\n{\"type\":\"object\"}\nauto"
        );
        assert!(matches!(input.estimate()?, InputTokenCount::Estimated(n) if n > 0));
        let escaped = MessagesCountInput::parse(
            r#"{"mo\u0064el":"m","messages":[{"role":"user","content":"hi"}]}"#,
        )?;
        assert_eq!(escaped.segments, ["user", "hi"]);
        for kind in ["image", "document", "audio", "redacted_thinking", "unknown"] {
            let raw = serde_json::json!({"model":"m","messages":[{"role":"user","content":[{"type":kind,"source":{"type":"text","data":"do not skip"}}]}]}).to_string();
            assert!(MessagesCountInput::parse(&raw).is_err());
        }
        Ok(())
    }
    #[test]
    fn count_origin_is_exact_and_never_falls_back_on_first_party_path()
    -> Result<(), Box<dyn std::error::Error>> {
        assert!(
            upstream_count_target(&AnthropicMessagesEndpoint::try_new(
                "https://api.anthropic.com",
                "/v1/messages"
            )?)?
            .is_some()
        );
        for base in [
            "https://relay.test",
            "https://api.anthropic.com.evil.test",
            "http://api.anthropic.com",
            "https://api.anthropic.com:8443",
        ] {
            assert!(
                upstream_count_target(&AnthropicMessagesEndpoint::try_new(base, "/v1/messages")?)?
                    .is_none()
            );
        }
        assert!(
            upstream_count_target(&AnthropicMessagesEndpoint::try_new(
                "https://api.anthropic.com",
                "/custom/messages"
            )?)
            .is_err()
        );
        Ok(())
    }
}
