//! Explicit channel entry points and validated, single-account imports.
use super::{
    CredentialId, CredentialResponse, CredentialStatus, CredentialUpsert,
    ManagementOperationsError, ManagementResourceError, ManagementResourceHttpState, UpstreamId,
    invalid_input, management_error, parse_json, principal, read_operations, revisioned_json,
    write_context,
};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
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

pub(super) async fn list() -> HttpResponse {
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
            import_available,
            authorization_flow,
            // This catalog describes NEW account enrollment; legacy Codex reauth is separate.
            authorization_available: false,
            upstream_kinds: upstream_kinds(id),
        },
    );
    HttpResponse::Ok().json(entries)
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
        "codex" => vec!["codex", "chatgpt", "openai-compatible"],
        "claude" => vec!["claude", "anthropic-compatible"],
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
        "openai-compatible" | "anthropic-compatible" | "grok.official" => {
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
        Ok(service.create_credential(
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
        Ok(Ok(value)) => revisioned_json(StatusCode::CREATED, value, CredentialResponse::from),
        Ok(Err(error)) => management_error(error),
        Err(error) => management_error(ManagementResourceError::from(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize;
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
        for channel in ["grok.build", "grok.console", "grok.web", "unknown"] {
            assert!(normalize(channel, "not-an-ordinary-credential", 1_000).is_err());
        }
    }
}
