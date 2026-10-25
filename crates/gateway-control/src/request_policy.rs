//! Publication checks for endpoint-owned options and encrypted header credentials.
use crate::control_plane_service::credential_associated_data;
use gateway_core::request_policy::{RequestPolicyError, validate_header_value};
use gateway_store::{
    control_plane::{ControlPlaneConfiguration, CredentialStatus, EndpointConfiguration},
    secret_store::{PlaintextSecret, SecretStore},
};
use std::collections::BTreeMap;

/// Check version-local references without reading or exposing Secret values.
///
/// # Errors
/// Rejects invalid scope, unbound model names and foreign/account header references.
pub fn validate(configuration: &ControlPlaneConfiguration) -> Result<(), RequestPolicyError> {
    for endpoint in &configuration.endpoints {
        endpoint
            .request_policy
            .validate(&endpoint.adapter_id, &endpoint.api_format)?;
        if !endpoint.request_policy.rules.is_empty()
            && configuration.route_candidates.iter().any(|c| {
                c.endpoint_id == endpoint.id
                    && c.transform_mode
                        == gateway_store::control_plane::TransformMode::LosslessBridge
            })
        {
            return Err(RequestPolicyError("request_policy_lossless_target"));
        }
        for rule in &endpoint.request_policy.rules {
            for name in &rule.models {
                let applicable = configuration.public_models.iter().any(|model| {
                    (name == "*" || model.model_name == *name)
                        && configuration.model_routes.iter().any(|route| {
                            route.public_model_id == model.id && configuration.route_candidates.iter().any(|c| {
                                c.route_id == route.id && c.endpoint_id == endpoint.id
                                    && rule.protocol.as_deref().is_none_or(|source|
                                        source == endpoint.api_format || c.transform_mode == gateway_store::control_plane::TransformMode::CanonicalBridge)
                            })
                        })
                });
                if !applicable {
                    return Err(RequestPolicyError("request_policy_model_or_protocol_scope"));
                }
            }
        }
        for reference in endpoint.request_policy.headers.values() {
            if !configuration.credentials.iter().any(|c| {
                c.id.as_str() == reference
                    && c.upstream_id == endpoint.upstream_id
                    && c.kind == "upstream_header"
                    && c.status == CredentialStatus::Active
            }) || configuration
                .endpoint_credential_bindings
                .iter()
                .any(|b| b.credential_id.as_str() == reference)
            {
                return Err(RequestPolicyError("request_policy_header_reference"));
            }
        }
    }
    // A header Secret is not an account and must never enter a serving credential pool.
    if configuration.endpoint_credential_bindings.iter().any(|b| {
        configuration
            .credentials
            .iter()
            .any(|c| c.id == b.credential_id && c.kind == "upstream_header")
    }) {
        return Err(RequestPolicyError("request_policy_header_binding"));
    }
    Ok(())
}

/// Authenticate and validate the exact version's header values while preparing its runtime.
///
/// # Errors
/// Returns value-free errors for invalid references, authentication, size or header bytes.
pub fn prepare_headers(
    configuration: &ControlPlaneConfiguration,
    endpoint: &EndpointConfiguration,
    secrets: &SecretStore,
) -> Result<BTreeMap<String, PlaintextSecret>, RequestPolicyError> {
    let mut result = BTreeMap::new();
    let mut bytes = 0usize;
    for (name, reference) in &endpoint.request_policy.headers {
        if configuration
            .endpoint_credential_bindings
            .iter()
            .any(|b| b.credential_id.as_str() == reference)
        {
            return Err(RequestPolicyError("request_policy_header_binding"));
        }
        let credential = configuration
            .credentials
            .iter()
            .find(|c| {
                c.id.as_str() == reference
                    && c.upstream_id == endpoint.upstream_id
                    && c.kind == "upstream_header"
                    && c.status == CredentialStatus::Active
            })
            .ok_or(RequestPolicyError("request_policy_header_reference"))?;
        let aad = credential_associated_data(
            &configuration.version.id,
            &credential.id,
            &credential.upstream_id,
        )
        .map_err(|_| RequestPolicyError("request_policy_header_secret"))?;
        let value = secrets
            .open(&credential.encrypted_secret, &aad)
            .map_err(|_| RequestPolicyError("request_policy_header_secret"))?;
        validate_header_value(value.as_bytes())?;
        bytes = bytes.saturating_add(name.len() + value.as_bytes().len());
        if bytes > 16 * 1024 {
            return Err(RequestPolicyError("request_policy_headers_size"));
        }
        result.insert(name.clone(), value);
    }
    Ok(result)
}
