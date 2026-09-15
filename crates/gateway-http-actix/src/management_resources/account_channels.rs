//! Explicit channel entry points and validated, single-account imports.
use super::{
    CredentialId, CredentialResponse, CredentialStatus, CredentialUpsert,
    EgressPolicyConfiguration, EgressPolicyId, EndpointConfiguration, EndpointId,
    EndpointTransport, ManagementOperationsError, ManagementResourceError,
    ManagementResourceHttpState, StoredEgressRedirectMode, UpstreamConfiguration, UpstreamId,
    invalid_input, management_error, parse_json, principal, read_operations, revisioned_json,
    write_context,
};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use gateway_store::control_plane::ConfigVersionStatus;
use provider_openai_compatible::{CodexCredentialExportFormat, OpenAiCompatibleRuntimeCredential};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Serialize)]
struct Channel {
    id: &'static str,
    name: &'static str,
    credential_format: &'static str,
    import_available: bool,
    authorization_flow: &'static str,
    authorization_available: bool,
    upstream_kinds: Vec<&'static str>,
}

pub(super) async fn list(state: web::Data<ManagementResourceHttpState>) -> HttpResponse {
    let codex_authorization = state
        .workflow
        .lock()
        .is_ok_and(|workflow| workflow.codex_enrollment_available());
    let claude_authorization = state
        .claude_workflow
        .lock()
        .is_ok_and(|workflow| workflow.codex_enrollment_available());
    let kimi_authorization = state.kimi_workflow.is_some();
    let entries = [
        (
            "openai-compatible",
            "OpenAI 兼容 / 中转",
            "api_key",
            true,
            "none",
        ),
        (
            "anthropic-compatible",
            "Anthropic 兼容 / 中转",
            "api_key",
            true,
            "none",
        ),
        (
            "codex",
            "Codex / ChatGPT",
            "cpa_sub2api_json",
            true,
            "authorization_code",
        ),
        (
            "claude",
            "Claude",
            "claude_json",
            true,
            "authorization_code",
        ),
        ("grok.official", "Grok Official", "api_key", true, "none"),
        (
            "kimi-coding",
            "Kimi Coding",
            "kimi_oauth",
            true,
            "device_code",
        ),
        ("kimi-api", "Kimi API", "api_key", true, "none"),
        (
            "grok.build",
            "Grok Build",
            "grok_build_json",
            false,
            "device_code",
        ),
        ("grok.console", "Grok Console", "sso", false, "none"),
        ("grok.web", "Grok Web", "sso", false, "none"),
        ("kiro", "Kiro", "kiro_json_or_key", true, "device_code"),
    ]
    .map(
        |(id, name, credential_format, import_available, authorization_flow)| Channel {
            id,
            name,
            credential_format,
            import_available: import_available
                || (id.starts_with("grok.") && state.native_accounts.is_some()),
            authorization_flow,
            // This catalog describes NEW account enrollment; legacy Codex reauth is separate.
            authorization_available: (id == "grok.build" && state.native_accounts.is_some())
                || (id == "codex" && codex_authorization)
                || (id == "claude" && claude_authorization)
                || (id == "kimi-coding" && kimi_authorization)
                || (id == "kiro" && state.kiro_workflow.is_some()),
            upstream_kinds: upstream_kinds(id),
        },
    );
    HttpResponse::Ok().json(entries)
}

#[derive(Serialize)]
struct PreparedKimiTarget {
    upstream_id: String,
    endpoint_id: String,
    prepared: bool,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct KiroTargetInput {
    region: Option<String>,
}

#[derive(Serialize)]
struct PreparedKiroTarget {
    upstream_id: String,
    endpoint_id: String,
    prepared: bool,
}

struct FixedAccountTarget {
    channel: &'static str,
    name: &'static str,
    upstream_id: &'static str,
    egress_id: &'static str,
    endpoint_id: &'static str,
    upstream_kind: &'static str,
    tags_json: &'static str,
    host: &'static str,
    adapter_id: &'static str,
    api_format: &'static str,
    base_url: &'static str,
    inference_path: &'static str,
    models_path: Option<&'static str>,
}

const CODEX_TARGET: FixedAccountTarget = FixedAccountTarget {
    channel: "codex",
    name: "Codex / ChatGPT",
    upstream_id: "codex",
    egress_id: "codex-egress",
    endpoint_id: "codex-responses",
    upstream_kind: "codex",
    tags_json: "[\"codex\",\"chatgpt\"]",
    host: "chatgpt.com",
    adapter_id: "openai-compatible.responses",
    api_format: "openai/responses",
    base_url: "https://chatgpt.com/backend-api/codex",
    inference_path: "/responses",
    models_path: None,
};

const CLAUDE_TARGET: FixedAccountTarget = FixedAccountTarget {
    channel: "claude",
    name: "Claude",
    upstream_id: "claude",
    egress_id: "claude-egress",
    endpoint_id: "claude-messages",
    upstream_kind: "claude",
    tags_json: "[\"claude\"]",
    host: "api.anthropic.com",
    adapter_id: "anthropic-compatible.messages",
    api_format: "anthropic/messages",
    base_url: "https://api.anthropic.com/v1",
    inference_path: "/messages",
    models_path: None,
};

/// Resolves exactly one existing Kimi Coding target or prepares CPAR's canonical dedicated target.
/// It never considers a compatible relay, visible name, or array position as channel ownership.
#[allow(clippy::too_many_lines)]
pub(super) async fn prepare_kimi_target(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut service = match super::service(&state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let configuration = match service
        .repository_mut()
        .load_configuration(&context.version)
    {
        Ok(Some(value)) => value,
        Ok(None) => return conflict_target(),
        Err(error) => return management_error(error.into()),
    };
    if configuration.version.status != ConfigVersionStatus::Draft
        || configuration.version.revision != context.revision.as_i64()
    {
        return conflict_target();
    }
    let kimi_upstreams = configuration
        .upstreams
        .iter()
        .filter(|upstream| upstream.kind == "kimi-coding")
        .collect::<Vec<_>>();
    if kimi_upstreams.len() > 1 {
        return conflict_target();
    }
    if kimi_upstreams.is_empty()
        && (configuration
            .egress_policies
            .iter()
            .any(|policy| policy.id.as_str() == "kimi-coding-egress")
            || configuration
                .upstreams
                .iter()
                .any(|upstream| upstream.id.as_str() == "kimi-coding")
            || configuration
                .endpoints
                .iter()
                .any(|endpoint| endpoint.id.as_str() == "kimi-coding-responses"))
    {
        return conflict_target();
    }
    let mut revision = context.revision;
    let mut prepared = false;
    let upstream_id = if let Some(upstream) = kimi_upstreams.first() {
        if !valid_kimi_coding_egress(&configuration, &upstream.id) {
            return conflict_target();
        }
        upstream.id.clone()
    } else {
        let Ok(policy_id) = EgressPolicyId::try_new("kimi-coding-egress") else {
            return invalid_input();
        };
        let policy = EgressPolicyConfiguration {
            id: policy_id.clone(),
            name: "Kimi Coding".to_owned(),
            allowed_schemes_json: "[\"https\"]".to_owned(),
            allowed_hosts_json: "[\"api.kimi.com\"]".to_owned(),
            allowed_ports_json: "[443]".to_owned(),
            allowed_cidrs_json: "[]".to_owned(),
            redirect_mode: StoredEgressRedirectMode::Deny,
            max_redirects: 0,
        };
        let policy = match service.create_egress_policy(&actor, &context.version, revision, policy)
        {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
        revision = policy.revision();
        let Ok(id) = UpstreamId::try_new("kimi-coding") else {
            return invalid_input();
        };
        let upstream = UpstreamConfiguration {
            id: id.clone(),
            name: "Kimi Coding".to_owned(),
            kind: "kimi-coding".to_owned(),
            enabled: true,
            tags_json: "[\"kimi\",\"coding\"]".to_owned(),
            egress_policy_id: Some(policy_id),
        };
        let upstream = match service.create_upstream(&actor, &context.version, revision, upstream) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
        revision = upstream.revision();
        prepared = true;
        id
    };
    let canonical = configuration
        .endpoints
        .iter()
        .filter(|endpoint| is_kimi_coding_endpoint(endpoint, &upstream_id))
        .collect::<Vec<_>>();
    if canonical.len() > 1 {
        return conflict_target();
    }
    let endpoint_id = if let Some(endpoint) = canonical.first() {
        if !endpoint.enabled || endpoint.models_path.as_deref() != Some("/v1/models") {
            return conflict_target();
        }
        endpoint.id.clone()
    } else {
        if configuration
            .endpoints
            .iter()
            .any(|endpoint| endpoint.id.as_str() == "kimi-coding-responses")
        {
            return conflict_target();
        }
        let Ok(id) = EndpointId::try_new("kimi-coding-responses") else {
            return invalid_input();
        };
        let endpoint = EndpointConfiguration {
            id: id.clone(),
            upstream_id: upstream_id.clone(),
            adapter_id: "openai-compatible.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: "https://api.kimi.com/coding".to_owned(),
            inference_path: "/v1/responses".to_owned(),
            models_path: Some("/v1/models".to_owned()),
            transport: EndpointTransport::Http,
            enabled: true,
        };
        let endpoint = match service.create_endpoint(&actor, &context.version, revision, endpoint) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
        revision = endpoint.revision();
        prepared = true;
        id
    };
    if !prepared {
        revision = match service.record_draft_resource_action(
            &actor,
            &context.version,
            revision,
            "kimi_target_resolved",
            "upstream",
            upstream_id.as_str(),
        ) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
    }
    HttpResponse::Ok()
        .insert_header(("ETag", format!("\"{}\"", revision.as_token())))
        .json(PreparedKimiTarget {
            upstream_id: upstream_id.as_str().to_owned(),
            endpoint_id: endpoint_id.as_str().to_owned(),
            prepared,
        })
}

/// The first official Codex journey owns its fixed `ChatGPT` target; the browser owns no IDs.
#[allow(clippy::unused_async)]
pub(super) async fn prepare_codex_target(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    prepare_fixed_target(&request, &state, &CODEX_TARGET)
}

/// The first official Claude journey owns its fixed Anthropic Messages target.
#[allow(clippy::unused_async)]
pub(super) async fn prepare_claude_target(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    prepare_fixed_target(&request, &state, &CLAUDE_TARGET)
}

#[allow(clippy::too_many_lines)]
fn prepare_fixed_target(
    request: &HttpRequest,
    state: &web::Data<ManagementResourceHttpState>,
    target: &FixedAccountTarget,
) -> HttpResponse {
    let context = match write_context(request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut service = match super::service(state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let configuration = match service
        .repository_mut()
        .load_configuration(&context.version)
    {
        Ok(Some(value)) => value,
        Ok(None) => return conflict_fixed_target(target),
        Err(error) => return management_error(error.into()),
    };
    if configuration.version.status != ConfigVersionStatus::Draft
        || configuration.version.revision != context.revision.as_i64()
    {
        return conflict_fixed_target(target);
    }
    let configured = configuration
        .upstreams
        .iter()
        .filter(|upstream| upstream.id.as_str() == target.upstream_id)
        .collect::<Vec<_>>();
    if configured.len() > 1
        || configured
            .first()
            .is_some_and(|upstream| upstream.kind != target.upstream_kind)
    {
        return conflict_fixed_target(target);
    }
    if configured.is_empty()
        && (configuration
            .egress_policies
            .iter()
            .any(|policy| policy.id.as_str() == target.egress_id)
            || configuration
                .endpoints
                .iter()
                .any(|endpoint| endpoint.id.as_str() == target.endpoint_id))
    {
        return conflict_fixed_target(target);
    }
    let mut revision = context.revision;
    let mut prepared = false;
    let upstream_id = if let Some(upstream) = configured.first() {
        if !valid_fixed_egress(&configuration, &upstream.id, target.host) {
            return conflict_fixed_target(target);
        }
        upstream.id.clone()
    } else {
        let (Ok(policy_id), Ok(upstream_id)) = (
            EgressPolicyId::try_new(target.egress_id),
            UpstreamId::try_new(target.upstream_id),
        ) else {
            return invalid_input();
        };
        let policy = EgressPolicyConfiguration {
            id: policy_id.clone(),
            name: target.name.to_owned(),
            allowed_schemes_json: "[\"https\"]".to_owned(),
            allowed_hosts_json: serde_json::json!([target.host]).to_string(),
            allowed_ports_json: "[443]".to_owned(),
            allowed_cidrs_json: "[]".to_owned(),
            redirect_mode: StoredEgressRedirectMode::Deny,
            max_redirects: 0,
        };
        let policy = match service.create_egress_policy(&actor, &context.version, revision, policy)
        {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
        revision = policy.revision();
        let upstream = UpstreamConfiguration {
            id: upstream_id.clone(),
            name: target.name.to_owned(),
            kind: target.upstream_kind.to_owned(),
            enabled: true,
            tags_json: target.tags_json.to_owned(),
            egress_policy_id: Some(policy_id),
        };
        let upstream = match service.create_upstream(&actor, &context.version, revision, upstream) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
        revision = upstream.revision();
        prepared = true;
        upstream_id
    };
    let canonical = configuration
        .endpoints
        .iter()
        .filter(|endpoint| is_fixed_endpoint(endpoint, &upstream_id, target))
        .collect::<Vec<_>>();
    if canonical.len() > 1 {
        return conflict_fixed_target(target);
    }
    let endpoint_id = if let Some(endpoint) = canonical.first() {
        if !endpoint.enabled || endpoint.models_path.as_deref() != target.models_path {
            return conflict_fixed_target(target);
        }
        endpoint.id.clone()
    } else {
        if configuration
            .endpoints
            .iter()
            .any(|endpoint| endpoint.id.as_str() == target.endpoint_id)
        {
            return conflict_fixed_target(target);
        }
        let Ok(endpoint_id) = EndpointId::try_new(target.endpoint_id) else {
            return invalid_input();
        };
        let endpoint = EndpointConfiguration {
            id: endpoint_id.clone(),
            upstream_id: upstream_id.clone(),
            adapter_id: target.adapter_id.to_owned(),
            api_format: target.api_format.to_owned(),
            base_url: target.base_url.to_owned(),
            inference_path: target.inference_path.to_owned(),
            models_path: target.models_path.map(str::to_owned),
            transport: EndpointTransport::Http,
            enabled: true,
        };
        let endpoint = match service.create_endpoint(&actor, &context.version, revision, endpoint) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
        revision = endpoint.revision();
        prepared = true;
        endpoint_id
    };
    if !prepared {
        revision = match service.record_draft_resource_action(
            &actor,
            &context.version,
            revision,
            &format!("{}_target_resolved", target.channel),
            "upstream",
            upstream_id.as_str(),
        ) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
    }
    HttpResponse::Ok()
        .insert_header(("ETag", format!("\"{}\"", revision.as_token())))
        .json(PreparedKiroTarget {
            upstream_id: upstream_id.as_str().to_owned(),
            endpoint_id: endpoint_id.as_str().to_owned(),
            prepared,
        })
}

/// Resolves one region-owned Kiro target or creates CPAR's fixed Kiro endpoint shape.
/// The region is the only advanced input; provider and endpoint identities never cross the browser.
#[allow(clippy::too_many_lines)]
pub(super) async fn prepare_kiro_target(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let input = match parse_json::<KiroTargetInput>(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let region = input.region.unwrap_or_else(|| "us-east-1".to_owned());
    if !valid_kiro_region(&region) {
        return invalid_input();
    }
    let context = match write_context(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let actor = match principal(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut service = match super::service(&state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let configuration = match service
        .repository_mut()
        .load_configuration(&context.version)
    {
        Ok(Some(value)) => value,
        Ok(None) => return conflict_kiro_target(),
        Err(error) => return management_error(error.into()),
    };
    if configuration.version.status != ConfigVersionStatus::Draft
        || configuration.version.revision != context.revision.as_i64()
    {
        return conflict_kiro_target();
    }
    let upstream_name = format!("kiro-{region}");
    let endpoint_name = format!("{upstream_name}-messages");
    let egress_name = format!("{upstream_name}-egress");
    let host = format!("runtime.{region}.kiro.dev");
    let configured = configuration
        .upstreams
        .iter()
        .filter(|upstream| upstream.id.as_str() == upstream_name)
        .collect::<Vec<_>>();
    if configured.len() > 1
        || configured
            .first()
            .is_some_and(|upstream| upstream.kind != "kiro")
    {
        return conflict_kiro_target();
    }
    if configured.is_empty()
        && (configuration
            .egress_policies
            .iter()
            .any(|policy| policy.id.as_str() == egress_name)
            || configuration
                .endpoints
                .iter()
                .any(|endpoint| endpoint.id.as_str() == endpoint_name))
    {
        return conflict_kiro_target();
    }
    let mut revision = context.revision;
    let mut prepared = false;
    let upstream_id = if let Some(upstream) = configured.first() {
        if !valid_kiro_egress(&configuration, &upstream.id, &host) {
            return conflict_kiro_target();
        }
        upstream.id.clone()
    } else {
        let (Ok(policy_id), Ok(upstream_id)) = (
            EgressPolicyId::try_new(egress_name),
            UpstreamId::try_new(upstream_name),
        ) else {
            return invalid_input();
        };
        let policy = EgressPolicyConfiguration {
            id: policy_id.clone(),
            name: "Kiro".to_owned(),
            allowed_schemes_json: "[\"https\"]".to_owned(),
            allowed_hosts_json: serde_json::json!([host]).to_string(),
            allowed_ports_json: "[443]".to_owned(),
            allowed_cidrs_json: "[]".to_owned(),
            redirect_mode: StoredEgressRedirectMode::Deny,
            max_redirects: 0,
        };
        let policy = match service.create_egress_policy(&actor, &context.version, revision, policy)
        {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
        revision = policy.revision();
        let upstream = UpstreamConfiguration {
            id: upstream_id.clone(),
            name: "Kiro".to_owned(),
            kind: "kiro".to_owned(),
            enabled: true,
            tags_json: serde_json::json!(["kiro", region]).to_string(),
            egress_policy_id: Some(policy_id),
        };
        let upstream = match service.create_upstream(&actor, &context.version, revision, upstream) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
        revision = upstream.revision();
        prepared = true;
        upstream_id
    };
    let canonical = configuration
        .endpoints
        .iter()
        .filter(|endpoint| is_kiro_endpoint(endpoint, &upstream_id, &host))
        .collect::<Vec<_>>();
    if canonical.len() > 1 {
        return conflict_kiro_target();
    }
    let endpoint_id = if let Some(endpoint) = canonical.first() {
        if !endpoint.enabled || endpoint.models_path.is_some() {
            return conflict_kiro_target();
        }
        endpoint.id.clone()
    } else {
        if configuration
            .endpoints
            .iter()
            .any(|endpoint| endpoint.id.as_str() == endpoint_name)
        {
            return conflict_kiro_target();
        }
        let Ok(endpoint_id) = EndpointId::try_new(endpoint_name) else {
            return invalid_input();
        };
        let endpoint = EndpointConfiguration {
            id: endpoint_id.clone(),
            upstream_id: upstream_id.clone(),
            adapter_id: "kiro.messages".to_owned(),
            api_format: "anthropic/messages".to_owned(),
            base_url: format!("https://{host}"),
            inference_path: "/".to_owned(),
            models_path: None,
            transport: EndpointTransport::Http,
            enabled: true,
        };
        let endpoint = match service.create_endpoint(&actor, &context.version, revision, endpoint) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
        revision = endpoint.revision();
        prepared = true;
        endpoint_id
    };
    if !prepared {
        revision = match service.record_draft_resource_action(
            &actor,
            &context.version,
            revision,
            "kiro_target_resolved",
            "upstream",
            upstream_id.as_str(),
        ) {
            Ok(value) => value,
            Err(error) => return management_error(error),
        };
    }
    HttpResponse::Ok()
        .insert_header(("ETag", format!("\"{}\"", revision.as_token())))
        .json(PreparedKiroTarget {
            upstream_id: upstream_id.as_str().to_owned(),
            endpoint_id: endpoint_id.as_str().to_owned(),
            prepared,
        })
}

fn is_kimi_coding_endpoint(endpoint: &EndpointConfiguration, upstream_id: &UpstreamId) -> bool {
    endpoint.upstream_id == *upstream_id
        && endpoint.adapter_id == "openai-compatible.responses"
        && endpoint.api_format == "openai/responses"
        && endpoint.base_url.trim_end_matches('/') == "https://api.kimi.com/coding"
        && endpoint.inference_path == "/v1/responses"
}

fn valid_kimi_coding_egress(
    configuration: &gateway_store::control_plane::ControlPlaneConfiguration,
    upstream_id: &UpstreamId,
) -> bool {
    let Some(upstream) = configuration
        .upstreams
        .iter()
        .find(|upstream| &upstream.id == upstream_id)
    else {
        return false;
    };
    let Some(policy_id) = upstream.egress_policy_id.as_ref() else {
        return false;
    };
    let Some(policy) = configuration
        .egress_policies
        .iter()
        .find(|policy| &policy.id == policy_id)
    else {
        return false;
    };
    let schemes = serde_json::from_str::<Vec<String>>(&policy.allowed_schemes_json);
    let hosts = serde_json::from_str::<Vec<String>>(&policy.allowed_hosts_json);
    let ports = serde_json::from_str::<Vec<u16>>(&policy.allowed_ports_json);
    matches!(schemes, Ok(values) if values == ["https"])
        && matches!(hosts, Ok(values) if values == ["api.kimi.com"])
        && matches!(ports, Ok(values) if values == [443])
        && policy.redirect_mode == StoredEgressRedirectMode::Deny
        && policy.max_redirects == 0
}

fn valid_kiro_region(region: &str) -> bool {
    matches!(
        region,
        "us-east-1"
            | "us-east-2"
            | "us-west-1"
            | "us-west-2"
            | "ap-southeast-1"
            | "ap-southeast-2"
            | "ap-northeast-1"
            | "ap-northeast-2"
            | "ap-south-1"
            | "eu-west-1"
            | "eu-west-2"
            | "eu-west-3"
            | "eu-central-1"
            | "eu-north-1"
            | "ca-central-1"
    )
}

fn is_kiro_endpoint(
    endpoint: &EndpointConfiguration,
    upstream_id: &UpstreamId,
    host: &str,
) -> bool {
    endpoint.upstream_id == *upstream_id
        && endpoint.adapter_id == "kiro.messages"
        && endpoint.api_format == "anthropic/messages"
        && endpoint.base_url.trim_end_matches('/') == format!("https://{host}")
        && endpoint.inference_path == "/"
}

fn is_fixed_endpoint(
    endpoint: &EndpointConfiguration,
    upstream_id: &UpstreamId,
    target: &FixedAccountTarget,
) -> bool {
    endpoint.upstream_id == *upstream_id
        && endpoint.adapter_id == target.adapter_id
        && endpoint.api_format == target.api_format
        && endpoint.base_url.trim_end_matches('/') == target.base_url.trim_end_matches('/')
        && endpoint.inference_path == target.inference_path
}

fn valid_kiro_egress(
    configuration: &gateway_store::control_plane::ControlPlaneConfiguration,
    upstream_id: &UpstreamId,
    host: &str,
) -> bool {
    let Some(upstream) = configuration
        .upstreams
        .iter()
        .find(|upstream| &upstream.id == upstream_id)
    else {
        return false;
    };
    let Some(policy_id) = upstream.egress_policy_id.as_ref() else {
        return false;
    };
    let Some(policy) = configuration
        .egress_policies
        .iter()
        .find(|policy| &policy.id == policy_id)
    else {
        return false;
    };
    let schemes = serde_json::from_str::<Vec<String>>(&policy.allowed_schemes_json);
    let hosts = serde_json::from_str::<Vec<String>>(&policy.allowed_hosts_json);
    let ports = serde_json::from_str::<Vec<u16>>(&policy.allowed_ports_json);
    matches!(schemes, Ok(values) if values == ["https"])
        && matches!(hosts, Ok(values) if values == [host])
        && matches!(ports, Ok(values) if values == [443])
        && policy.redirect_mode == StoredEgressRedirectMode::Deny
        && policy.max_redirects == 0
}

fn valid_fixed_egress(
    configuration: &gateway_store::control_plane::ControlPlaneConfiguration,
    upstream_id: &UpstreamId,
    host: &str,
) -> bool {
    let Some(upstream) = configuration
        .upstreams
        .iter()
        .find(|upstream| &upstream.id == upstream_id)
    else {
        return false;
    };
    let Some(policy_id) = upstream.egress_policy_id.as_ref() else {
        return false;
    };
    let Some(policy) = configuration
        .egress_policies
        .iter()
        .find(|policy| &policy.id == policy_id)
    else {
        return false;
    };
    let schemes = serde_json::from_str::<Vec<String>>(&policy.allowed_schemes_json);
    let hosts = serde_json::from_str::<Vec<String>>(&policy.allowed_hosts_json);
    let ports = serde_json::from_str::<Vec<u16>>(&policy.allowed_ports_json);
    matches!(schemes, Ok(values) if values == ["https"])
        && matches!(hosts, Ok(values) if values == [host])
        && matches!(ports, Ok(values) if values == [443])
        && policy.redirect_mode == StoredEgressRedirectMode::Deny
        && policy.max_redirects == 0
}

fn conflict_target() -> HttpResponse {
    super::error_response(
        StatusCode::CONFLICT,
        "management_kimi_target_conflict",
        "Kimi 接入配置已改变或存在多个专用配置，请在高级维护中整理后重试",
    )
}

fn conflict_kiro_target() -> HttpResponse {
    super::error_response(
        StatusCode::CONFLICT,
        "management_kiro_target_conflict",
        "Kiro 接入配置已改变或不符合该地区的标准配置，请在高级维护中整理后重试",
    )
}

fn conflict_fixed_target(target: &FixedAccountTarget) -> HttpResponse {
    let message = match target.channel {
        "codex" => "Codex / ChatGPT 接入配置已改变或不符合标准配置，请在高级维护中整理后重试",
        "claude" => "Claude 接入配置已改变或不符合标准配置，请在高级维护中整理后重试",
        _ => "渠道接入配置已改变或不符合标准配置，请在高级维护中整理后重试",
    };
    super::error_response(
        StatusCode::CONFLICT,
        "management_channel_target_conflict",
        message,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    id: String,
    channel: String,
    #[serde(deserialize_with = "secret_input")]
    secret: Zeroizing<String>,
}

fn secret_input<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Zeroizing<String>, D::Error> {
    String::deserialize(deserializer).map(Zeroizing::new)
}

fn upstream_kinds(channel: &str) -> Vec<&'static str> {
    match channel {
        // Account onboarding must not treat protocol compatibility as channel
        // ownership.  In particular, accepting an arbitrary OpenAI-compatible
        // upstream here made a Kimi import appear to belong to Codex or a relay.
        // API-key setup has its own explicit channel entries below.
        "codex" => vec!["codex", "chatgpt"],
        "claude" => vec!["claude"],
        "kimi-coding" => vec!["kimi-coding"],
        "kimi-api" => vec!["kimi"],
        "openai-compatible" => vec!["openai-compatible"],
        "anthropic-compatible" => vec!["anthropic-compatible"],
        "grok.official" => vec!["grok.official"],
        "grok.build" => vec!["grok.build"],
        "grok.console" => vec!["grok.console"],
        "grok.web" => vec!["grok.web"],
        "kiro" => vec!["kiro"],
        _ => vec![],
    }
}

fn normalize(
    channel: &str,
    secret: &str,
    now: i64,
) -> Result<(&'static str, Zeroizing<Vec<u8>>), ()> {
    if secret.is_empty() || secret.len() > 65_536 {
        return Err(());
    }
    match channel {
        "openai-compatible" | "anthropic-compatible" | "grok.official" | "kimi-api" => {
            if !secret.bytes().all(|b| b.is_ascii_graphic()) || secret.starts_with('{') {
                return Err(());
            }
        }
        "codex" => {
            let value =
                OpenAiCompatibleRuntimeCredential::import_compatible(secret.as_bytes(), now)
                    .map_err(|_| ())?;
            if !matches!(value, OpenAiCompatibleRuntimeCredential::CodexOAuth(_))
                || !value.has_account_binding()
            {
                return Err(());
            }
            value.bearer_at(now).map_err(|_| ())?;
            return Ok((
                "oauth_json",
                value
                    .export_json(CodexCredentialExportFormat::Cpa)
                    .map_err(|_| ())?,
            ));
        }
        "claude" => {
            let value = provider_anthropic_compatible::ClaudeRuntimeCredential::import_at(
                secret.as_bytes(),
                now,
            )
            .map_err(|_| ())?;
            value.authorization_at(now).map_err(|_| ())?;
        }
        "kimi-coding" => {
            let value =
                OpenAiCompatibleRuntimeCredential::import_compatible(secret.as_bytes(), now)
                    .map_err(|_| ())?;
            if !matches!(value, OpenAiCompatibleRuntimeCredential::KimiOAuth(_)) {
                return Err(());
            }
            value.bearer_at(now).map_err(|_| ())?;
            return Ok((
                "oauth_json",
                value.export_kimi_oauth_json().map_err(|_| ())?,
            ));
        }
        "kiro" => {
            provider_kiro::credential::KiroCredential::import_runtime_secret(
                secret.as_bytes(),
                now,
            )
            .map_err(|_| ())?;
        }
        _ => return Err(()),
    }
    Ok(("bearer", Zeroizing::new(secret.as_bytes().to_vec())))
}

pub(super) async fn import(
    request: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let input: Input = match parse_json(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if input.id.trim().is_empty() || input.id.len() > 128 {
        return invalid_input();
    }
    let Ok(id) = CredentialId::try_new(input.id) else {
        return invalid_input();
    };
    let Ok(owner) = UpstreamId::try_new(path.into_inner()) else {
        return invalid_input();
    };
    let actor = match principal(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let worker = state.clone();
    let result = read_operations(&state, move || {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|v| i64::try_from(v.as_millis()).ok())
            .ok_or(ManagementOperationsError::SourceUnavailable)?;
        let Ok((kind, secret)) = normalize(&input.channel, &input.secret, now) else {
            return Ok(Err(ManagementResourceError::InvalidCredentialInput));
        };
        let mut service = worker
            .service
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        match service.get_upstream(&context.version, &owner) {
            Ok(value) if upstream_kinds(&input.channel).contains(&value.value().kind.as_str()) => {}
            Ok(_) => return Ok(Err(ManagementResourceError::InvalidCredentialInput)),
            Err(error) => return Ok(Err(error)),
        }
        Ok(service.import_credential(
            &actor,
            &context.version,
            context.revision,
            owner,
            CredentialUpsert {
                id,
                kind: kind.to_owned(),
                plaintext_secret: secret.as_slice(),
                status: CredentialStatus::Active,
            },
        ))
    })
    .await;
    match result {
        Ok(Ok((value, created))) => revisioned_json(
            if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            value,
            CredentialResponse::from,
        ),
        Ok(Err(error)) => management_error(error),
        Err(error) => management_error(ManagementResourceError::from(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize, upstream_kinds};

    #[test]
    fn named_account_channels_do_not_borrow_compatible_relays() {
        assert_eq!(upstream_kinds("codex"), ["codex", "chatgpt"]);
        assert_eq!(upstream_kinds("claude"), ["claude"]);
        assert_eq!(upstream_kinds("kimi-coding"), ["kimi-coding"]);
        assert_eq!(upstream_kinds("kimi-api"), ["kimi"]);
    }

    #[test]
    fn channel_material_is_validated_before_storage() {
        for channel in ["openai-compatible", "anthropic-compatible", "grok.official"] {
            assert!(normalize(channel, "synthetic-api-key", 1_000).is_ok());
            assert!(normalize(channel, "{\"access_token\":\"x\"}", 1_000).is_err());
        }
        assert!(normalize("kiro", "ksk_synthetic-key", 1_000).is_ok());
        assert!(normalize("kiro", "arbitrary-bearer", 1_000).is_err());
        assert!(normalize("claude", r#"{"kind":"claude_oauth","access_token":"a","refresh_token":"r","expires_at_ms":2000}"#, 1_000).is_ok());
        assert!(normalize("claude", r#"{"kind":"claude_oauth","access_token":"a","refresh_token":"r","expires_at_ms":2000}"#, 3_000).is_err());
        assert!(normalize("codex", "plain-api-key", 1_000).is_err());
        assert!(normalize("kimi-api", "synthetic-kimi-api-key", 1_000).is_ok());
        assert!(normalize("kimi-coding", "synthetic-kimi-api-key", 1_000).is_err());
        assert!(normalize("kimi-coding", r#"{"kind":"kimi_oauth","access_token":"a","refresh_token":"r","expires_at_ms":2000,"device_id":"d"}"#, 1_000).is_ok());
        for channel in ["grok.build", "grok.console", "grok.web", "unknown"] {
            assert!(normalize(channel, "not-an-ordinary-credential", 1_000).is_err());
        }
    }
}
