//! Counting uses serving admission and transport, never the inference orchestrator/event writer.
use super::{
    Duration, EndpointAdapter, ErrorScope, GatewayError, GatewayErrorCode, NonZeroUsize,
    P12_CONNECT_TIMEOUT, P12RoutedResponsesExecutor, UpstreamHttpResponse, UpstreamTimeouts,
    UpstreamTransportProfile, anthropic_runtime_credential, classify_anthropic_response_failure,
    egress_rejected_error, internal_error, stale_runtime_error, system_now_ms_runtime,
    upstream_protocol_error,
};
use gateway_core::InputTokenCount;
use gateway_router::{CountTokensExecution, CountTokensExecutor, CountTokensFuture};
use provider_anthropic_compatible::token_count::{
    MessagesCountInput, decode_upstream_count, upstream_count_target,
};

const COUNT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_COUNT_RESPONSE_BYTES: usize = 64 * 1024;

impl CountTokensExecutor for P12RoutedResponsesExecutor {
    fn count_tokens(
        &self,
        execution: CountTokensExecution,
    ) -> CountTokensFuture<'_, Result<InputTokenCount, GatewayError>> {
        Box::pin(async move {
            if !self.snapshot_is_current() {
                return Err(stale_runtime_error());
            }
            let input =
                MessagesCountInput::parse(execution.native_payload().ok_or_else(internal_error)?)?;
            let now = system_now_ms_runtime().map_err(|_| internal_error())?;
            let selected = execution.select(
                &self.scheduler,
                &self.runtime_health,
                &self.runtime_quota,
                now,
                |candidate| {
                    self.endpoints.contains_key(candidate.endpoint_id())
                        && gateway_router::validate_model_name_control(
                            execution.request(),
                            candidate.upstream_model(),
                            candidate.transform_mode(),
                            candidate.effective_capabilities(),
                        )
                        .is_ok()
                },
            )?;
            let (candidate, credential) = selected.into_parts();
            let runtime = self
                .endpoints
                .get(candidate.endpoint_id())
                .ok_or_else(internal_error)?;
            let native = match &runtime.adapter {
                EndpointAdapter::AnthropicMessages(endpoint) => {
                    upstream_count_target(endpoint)?.map(|target| (endpoint, target))
                }
                _ => None,
            };
            let Some((endpoint, target)) = native else {
                // Hold capacity through bounded CPU work, including when the caller disconnects.
                // The blocking task receives only validated count input and the existing lease.
                return tokio::task::spawn_blocking(move || {
                    let _lease = credential;
                    input.estimate()
                })
                .await
                .map_err(|_| internal_error())?;
            };
            let material = anthropic_runtime_credential(credential.secret_bytes())
                .map_err(|failure| failure.safe_error())?;
            let authorization = material.authorization_at(now)?;
            let compatible = self
                .compatible_endpoints
                .get(candidate.endpoint_id())
                .ok_or_else(internal_error)?;
            let egress = compatible
                .try_lease_egress_for_credential(&credential, Some(candidate.upstream_model()), now)
                .map_err(|_| egress_unavailable_error())?;
            // Resolve/admit the counting URL itself, not the inference URL.
            let admitted = runtime
                .policy
                .admit_url(target, runtime.resolver.as_ref())
                .map_err(|_| egress_rejected_error())?;
            let outbound = input.upstream_request(
                endpoint,
                &authorization,
                candidate.upstream_model(),
                admitted,
            )?;
            let timeout = UpstreamTimeouts::try_new(
                P12_CONNECT_TIMEOUT.min(COUNT_TIMEOUT),
                COUNT_TIMEOUT,
                COUNT_TIMEOUT,
                COUNT_TIMEOUT,
            )
            .map_err(|_| internal_error())?;
            let profile = UpstreamTransportProfile::new(
                timeout,
                egress.transport_profile().proxy().clone(),
                NonZeroUsize::new(1).ok_or_else(internal_error)?,
            );
            if !self.snapshot_is_current() {
                return Err(stale_runtime_error());
            }
            #[cfg(test)]
            let outbound = match &runtime.transports.http {
                Some(peer) => peer.handoff(&outbound)?,
                None => outbound,
            };
            tokio::time::timeout(COUNT_TIMEOUT, async {
                let response = self.client_pool.send(outbound, &profile).await?;
                read_count_response(response).await
            })
            .await
            .map_err(|_| egress_unavailable_error())?
        })
    }
}

async fn read_count_response(
    mut response: UpstreamHttpResponse,
) -> Result<InputTokenCount, GatewayError> {
    if response.status() != 200 {
        let status = response.status();
        return Err(classify_anthropic_response_failure(&mut response, status)
            .await
            .safe_error());
    }
    if !response
        .header("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return Err(upstream_protocol_error());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.next_chunk().await? {
        if body.len().saturating_add(chunk.len()) > MAX_COUNT_RESPONSE_BYTES {
            return Err(upstream_protocol_error());
        }
        body.extend_from_slice(&chunk);
    }
    decode_upstream_count(&body)
}

const fn egress_unavailable_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::EgressUnavailable, ErrorScope::Egress)
}
