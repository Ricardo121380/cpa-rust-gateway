use super::*;
use crate::{
    CapabilitySet, NativePayloadAvailability, ProtocolResponseProjector, ProtocolTransformInput,
    SnapshotTransformMode, project_registered_protocol_request,
};
use gateway_catalog::SemanticCapability;
use gateway_core::{
    CanonicalEvent, MessageRole, MessageStart, RawExtensions, ResponseId, ResponseStart,
    ToolCallStart,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn request(parallel: bool, choice: &Value) -> Result<CanonicalRequest, gateway_core::GatewayError> {
    protocol_openai_responses::decode_request(&json!({"model":"test","input":"question","parallel_tool_calls":parallel,"tool_choice":choice,
        "tools":[{"type":"function","name":"lookup","parameters":{}},
            {"type":"namespace","name":"alpha","description":"first","tools":[{"type":"function","name":"lookup","parameters":{}}]},
            {"type":"namespace","name":"beta","description":"second","tools":[{"type":"function","name":"lookup","parameters":{}}]}]
    }).to_string()).map(|v|v.request)
}

fn start(namespace: Option<&str>, id: &str) -> Result<CanonicalEvent, Box<dyn std::error::Error>> {
    let mut extensions = RawExtensions::default();
    if let Some(namespace) = namespace {
        extensions.try_insert(
            "namespace",
            RawJson::from_json_string(json!(namespace).to_string())?,
        )?;
    }
    Ok(CanonicalEvent::ToolCallStart(ToolCallStart {
        call_id: id.into(),
        name: "lookup".into(),
        extensions,
    }))
}

fn projector(
    request: &CanonicalRequest,
) -> Result<ProtocolResponseProjector, Box<dyn std::error::Error>> {
    let mut p =
        ProtocolResponseProjector::new(ProtocolFormat::OpenAiResponses).with_tool_constraints(
            ToolExecutionConstraints::from_request(request, ProtocolFormat::OpenAiResponses)?,
        );
    p.project_event(&CanonicalEvent::ResponseStart(ResponseStart {
        response_id: ResponseId::try_new("response")?,
        extensions: RawExtensions::default(),
    }))?;
    p.project_event(&CanonicalEvent::MessageStart(MessageStart {
        role: MessageRole("assistant".into()),
        extensions: RawExtensions::default(),
    }))?;
    Ok(p)
}

#[test]
fn namespace_controls_use_complete_identities_and_actual_parallel_capability() -> TestResult {
    let format = ProtocolFormat::OpenAiResponses;
    for parallel in [false, true] {
        let request = request(parallel, &json!("auto"))?;
        let mut p = projector(&request)?;
        assert!(p.project_event(&start(Some("alpha"), "a")?).is_ok());
        assert_eq!(
            p.project_event(&start(Some("beta"), "b")?).is_ok(),
            parallel
        );
        assert!(
            projector(&request)?
                .project_event(&start(Some("unknown"), "unknown")?)
                .is_err()
        );
        let mut capabilities = vec![SemanticCapability::Tools, SemanticCapability::JsonSchema];
        for qualified in [false, true] {
            if qualified {
                capabilities.push(SemanticCapability::ParallelTools);
            }
            let caps = CapabilitySet::try_new(capabilities.iter().copied())?;
            let result = project_registered_protocol_request(ProtocolTransformInput {
                target_supports_service_tier: true,
                target_model: None,
                source: format,
                target: format,
                mode: SnapshotTransformMode::Canonical,
                native_payload: NativePayloadAvailability::Unavailable,
                request: &request,
                streaming: false,
                requires_json_schema: false,
                requires_parallel_tools: parallel,
                target_capabilities: &caps,
            });
            assert_eq!(result.is_ok(), !parallel || qualified);
        }
    }
    assert!(
        projector(&request(false, &json!("none"))?)?
            .project_event(&start(Some("alpha"), "a")?)
            .is_err()
    );
    let flat = request(false, &json!({"type":"function","name":"lookup"}))?;
    assert!(
        projector(&flat)?
            .project_event(&start(None, "flat")?)
            .is_ok()
    );
    assert!(
        projector(&flat)?
            .project_event(&start(Some("alpha"), "child")?)
            .is_err()
    );
    Ok(())
}
