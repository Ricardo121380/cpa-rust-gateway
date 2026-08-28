//! P9-08 explicit Grok Web Tool-emulation flag evidence.

#![deny(unsafe_code)]

use std::error::Error;

use gateway_catalog::SemanticCapability;
use gateway_core::RawJson;
use gateway_upstream::UpstreamProxy;
use provider_grok::{
    GrokWebBrowserEgressSession, GrokWebBrowserUserAgent, GrokWebChatRequestBuilder,
    GrokWebChatRequestError, GrokWebCredential, GrokWebEgressSessionId, GrokWebTlsProfile,
    GrokWebToolCapability, GrokWebToolEmulation, GrokWebToolEmulationError,
};

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 1_000_000;
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/537.36";

#[test]
fn default_flag_is_disabled_has_no_native_tools_and_never_injects_prompt_bytes() -> TestResult {
    let session = session()?;
    let disabled = GrokWebToolEmulation::default();
    assert!(!disabled.is_enabled());
    assert_eq!(disabled.tool_capability(), GrokWebToolCapability::Disabled);
    let native_capabilities = GrokWebToolEmulation::native_semantic_capabilities()?;
    assert!(native_capabilities.supports(SemanticCapability::Streaming));
    assert!(!native_capabilities.supports(SemanticCapability::Tools));
    assert!(!native_capabilities.supports(SemanticCapability::ParallelTools));

    let tool_request = tool_request("lookup_weather")?;
    let prepared = disabled.prepare(&tool_request.tools)?;
    assert_eq!(prepared.capability(), GrokWebToolCapability::Disabled);
    assert!(!prepared.has_addendum());
    assert_eq!(
        GrokWebChatRequestBuilder::build_with_tool_emulation(
            &session,
            "grok-web-fixture",
            &tool_request,
            disabled,
            NOW_MS,
        ),
        Err(GrokWebChatRequestError::UnsupportedCanonicalRequest)
    );

    let text_request = text_request("No hidden Tool instruction")?;
    let plain =
        GrokWebChatRequestBuilder::build(&session, "grok-web-fixture", &text_request, NOW_MS)?;
    let disabled_outbound = GrokWebChatRequestBuilder::build_with_tool_emulation(
        &session,
        "grok-web-fixture",
        &text_request,
        disabled,
        NOW_MS,
    )?;
    assert_eq!(plain.body(), disabled_outbound.body());
    let body: serde_json::Value = serde_json::from_slice(disabled_outbound.body())?;
    assert_eq!(body["message"], "No hidden Tool instruction");
    assert!(
        !body["message"]
            .as_str()
            .ok_or("fixture message was not text")?
            .contains("gateway.tool_emulation")
    );
    Ok(())
}

#[test]
fn enabled_flag_injects_only_a_bounded_visible_emulated_convention() -> TestResult {
    let session = session()?;
    let enabled = GrokWebToolEmulation::new(true);
    let request = tool_request("lookup_weather")?;
    let prepared = enabled.prepare(&request.tools)?;
    assert_eq!(prepared.capability(), GrokWebToolCapability::Emulated);
    assert!(prepared.has_addendum());
    let native_capabilities = GrokWebToolEmulation::native_semantic_capabilities()?;
    assert!(!native_capabilities.supports(SemanticCapability::Tools));
    assert!(!native_capabilities.supports(SemanticCapability::ParallelTools));

    let outbound = GrokWebChatRequestBuilder::build_with_tool_emulation(
        &session,
        "grok-web-fixture",
        &request,
        enabled,
        NOW_MS,
    )?;
    let body: serde_json::Value = serde_json::from_slice(outbound.body())?;
    let message = body["message"]
        .as_str()
        .ok_or("fixture message was not text")?;
    assert!(message.starts_with("[[gateway.tool_emulation.v1]]\n"));
    assert!(message.contains(r#""mode":"emulated""#));
    assert!(message.contains("lookup_weather"));
    assert!(message.ends_with("Weather in Shanghai?"));
    assert!(body.get("tools").is_none());
    let diagnostic = format!("{outbound:?}");
    for private_value in ["lookup_weather", "Weather in Shanghai?"] {
        assert!(!diagnostic.contains(private_value));
    }
    Ok(())
}

#[test]
fn enabled_unsafe_tool_fails_closed_while_disabled_path_still_injects_nothing() -> TestResult {
    let session = session()?;
    let unsafe_request = tool_request("unsafe\nname")?;
    let disabled = GrokWebToolEmulation::default();
    assert!(!disabled.prepare(&unsafe_request.tools)?.has_addendum());
    let enabled = GrokWebToolEmulation::new(true);
    assert_eq!(
        enabled.prepare(&unsafe_request.tools),
        Err(GrokWebToolEmulationError::InvalidToolName)
    );
    assert_eq!(
        GrokWebChatRequestBuilder::build_with_tool_emulation(
            &session,
            "grok-web-fixture",
            &unsafe_request,
            enabled,
            NOW_MS,
        ),
        Err(GrokWebChatRequestError::InvalidToolEmulation)
    );

    let mut duplicate_schema_request = tool_request("duplicate_schema")?;
    duplicate_schema_request.tools[0].input_schema =
        RawJson::from_json_string(r#"{"type":"object","type":"array"}"#.to_owned())?;
    assert_eq!(
        enabled.prepare(&duplicate_schema_request.tools),
        Err(GrokWebToolEmulationError::InvalidToolSchema)
    );
    Ok(())
}

fn text_request(text: &str) -> Result<gateway_core::CanonicalRequest, serde_json::Error> {
    serde_json::from_value(serde_json::json!({
        "requested_model": "gateway-web",
        "messages": [{
            "role": "user",
            "content": [{"text": {"text": text, "extensions": {}}}],
            "extensions": {}
        }],
        "extensions": {}
    }))
}

fn tool_request(tool_name: &str) -> Result<gateway_core::CanonicalRequest, serde_json::Error> {
    serde_json::from_value(serde_json::json!({
        "requested_model": "gateway-web",
        "messages": [{
            "role": "user",
            "content": [{"text": {"text": "Weather in Shanghai?", "extensions": {}}}],
            "extensions": {}
        }],
        "tools": [{
            "name": tool_name,
            "description": "Look up weather by city.",
            "input_schema": {"type": "object", "properties": {"city": {"type": "string"}}},
            "extensions": {}
        }],
        "extensions": {}
    }))
}

fn session() -> Result<GrokWebBrowserEgressSession, Box<dyn Error>> {
    let credential = GrokWebCredential::import_sso_json(
        br#"{
            "kind":"grok_web_sso",
            "account_ref":"web_account_01",
            "lineage_ref":"sso_import_01",
            "revision":7,
            "expires_at_ms":1500000,
            "cookies":[{"name":"sso_session","value":"session_value","domain":".grok.example.test","path":"/","secure":true,"http_only":true}]
        }"#,
        NOW_MS,
    )?;
    Ok(GrokWebBrowserEgressSession::try_new(
        GrokWebEgressSessionId::try_new("web_egress_01")?,
        credential,
        GrokWebBrowserUserAgent::try_new(USER_AGENT)?,
        GrokWebTlsProfile::try_new("chrome_136_macos")?,
        UpstreamProxy::Direct,
        NOW_MS,
    )?)
}
