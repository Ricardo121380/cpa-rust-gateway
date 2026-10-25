//! Endpoint policies run after conversion/model controls, before provider transport dispatch.
use super::{
    BTreeMap, ControlPlaneConfiguration, EndpointAttemptDriver, EndpointId, EndpointRuntime,
    ErrorScope, GatewayError, GatewayErrorCode, NativePayloadAvailability, ProtocolFormat,
    ProtocolTransformInput, ResponsesResponseMode, RuntimeCompositionError, SecretStore,
    SnapshotRouteCandidate, UpstreamHttpRequest, internal_error,
    project_registered_protocol_request,
};

pub(super) fn prepare_headers(
    configuration: &ControlPlaneConfiguration,
    secrets: &SecretStore,
    endpoints: &mut BTreeMap<EndpointId, EndpointRuntime>,
) -> Result<(), RuntimeCompositionError> {
    gateway_control::request_policy::validate(configuration)
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
    for configured in &configuration.endpoints {
        // Validate encrypted values even for disabled endpoints during candidate preparation.
        let headers =
            gateway_control::request_policy::prepare_headers(configuration, configured, secrets)
                .map_err(|_| RuntimeCompositionError::Unavailable)?;
        if let Some(runtime) = endpoints.get_mut(&configured.id) {
            runtime.custom_headers = headers;
        }
    }
    Ok(())
}

// The two inference/count callers share the same authenticated, bounded header material.
pub(super) fn headers(runtime: &EndpointRuntime) -> impl Iterator<Item = (String, String)> + '_ {
    runtime.custom_headers.iter().map(|(name, value)| {
        (
            name.clone(),
            String::from_utf8_lossy(value.as_bytes()).into_owned(),
        )
    })
}

impl EndpointAttemptDriver {
    pub(super) fn apply_request_policy(
        &self,
        runtime: &EndpointRuntime,
        candidate: &SnapshotRouteCandidate,
        request: UpstreamHttpRequest,
    ) -> Result<UpstreamHttpRequest, GatewayError> {
        if runtime.request_policy.is_empty() {
            return Ok(request);
        }
        let mut body: serde_json::Value =
            serde_json::from_slice(request.body()).map_err(|_| internal_error())?;
        let original = body.clone();
        runtime
            .request_policy
            .apply(
                &mut body,
                self.admitted_public_model
                    .as_deref()
                    .unwrap_or(&self.request.requested_model),
                self.client_protocol.api_format(),
            )
            .map_err(|_| policy_request_error())?;
        let bytes = if body == original {
            request.body().to_vec()
        } else {
            let json = serde_json::to_string(&body).map_err(|_| internal_error())?;
            let target = candidate.protocol_format().ok_or_else(internal_error)?;
            if target == ProtocolFormat::AnthropicMessages
                && body["thinking"]["type"] == "enabled"
                && body["thinking"]["budget_tokens"]
                    .as_u64()
                    .zip(body["max_tokens"].as_u64())
                    .is_some_and(|(budget, limit)| budget >= limit)
            {
                // A rule may lower the output limit, but cannot silently rewrite protected
                // thinking intent to make the resulting request acceptable.
                return Err(policy_request_error());
            }

            let decoded = match target {
                ProtocolFormat::OpenAiChatCompletions => {
                    protocol_openai_chat::decode_request(&json)?.request
                }
                ProtocolFormat::OpenAiResponses => {
                    protocol_openai_responses::decode_request(&json)?.request
                }
                ProtocolFormat::AnthropicMessages => {
                    protocol_anthropic::decode_request(&json)?.request
                }
            };
            // Recheck target control collisions, capability and tool history. Configured options
            // cannot repair invalid ingress, which has already passed through its own admission.
            project_registered_protocol_request(ProtocolTransformInput {
                source: target,
                target,
                target_model: Some(candidate.upstream_model()),
                target_service_tier: runtime.adapter.service_tier_mapping(),
                mode: gateway_router::SnapshotTransformMode::Passthrough,
                native_payload: NativePayloadAvailability::Exact,
                request: &decoded,
                streaming: self.mode == ResponsesResponseMode::Streaming,
                requires_json_schema: false,
                requires_parallel_tools: gateway_router::ToolExecutionConstraints::from_request(
                    &decoded, target,
                )
                .map_err(|_| policy_request_error())?
                .requires_parallel_tools(),
                target_capabilities: candidate.effective_capabilities(),
            })
            .map_err(|_| policy_request_error())?;
            json.into_bytes()
        };
        request
            .with_body_and_additional_headers(bytes, headers(runtime))
            .map_err(|_| internal_error())
    }
}

fn policy_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}
