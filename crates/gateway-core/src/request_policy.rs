//! Bounded target-body configuration, independent of routing, credentials and history.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

/// Versioned options belonging to one exact endpoint (and therefore one channel/target protocol).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestPolicy {
    /// Ordered rules. Models name resolved public identities, never aliases or upstream models.
    #[serde(default)]
    pub rules: Vec<RequestRule>,
    /// Lowercase header name to same-upstream encrypted `upstream_header` credential ID.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

/// Explicit sampling/output controls only; no JSON path language or executable expressions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestRule {
    /// Exact resolved public model names, or the sole entry `*` for every admitted model.
    pub models: Vec<String>,
    /// Optional downstream protocol; absent matches all three public protocols.
    #[serde(default)]
    pub protocol: Option<String>,
    /// Fill absent target fields; first matching default wins.
    #[serde(default)]
    pub default: BTreeMap<String, Value>,
    /// Replace target values; last matching override wins.
    #[serde(default)]
    pub r#override: BTreeMap<String, Value>,
    /// Explicitly remove optional target fields after every default and override.
    #[serde(default)]
    pub filter: Vec<String>,
}

/// Value-free configuration error suitable for protected management responses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestPolicyError(pub &'static str);
impl fmt::Display for RequestPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl Error for RequestPolicyError {}

impl RequestPolicy {
    /// Whether this endpoint has any configured behavior.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.headers.is_empty()
    }

    /// Validate the closed supported configuration at its input boundary.
    ///
    /// # Errors
    /// Rejects unsupported targets, fields, values, ambiguous operations and unsafe header names.
    pub fn validate(&self, adapter: &str, target: &str) -> Result<(), RequestPolicyError> {
        if self.is_empty() {
            return Ok(());
        }
        let expected = match adapter {
            "openai-compatible.chat-completions" => "openai/chat-completions",
            "openai-compatible.responses" => "openai/responses",
            "anthropic-compatible.messages" => "anthropic/messages",
            _ => return Err(RequestPolicyError("request_policy_unsupported_adapter")),
        };
        if target != expected || self.rules.len() > 32 || self.headers.len() > 16 {
            return Err(RequestPolicyError("request_policy_bounds_or_target"));
        }
        for (name, reference) in &self.headers {
            validate_header_name(name)?;
            if crate::CredentialId::try_new(reference).is_err() {
                return Err(RequestPolicyError("request_policy_header_reference"));
            }
        }
        for rule in &self.rules {
            if rule.models.is_empty()
                || rule.models.len() > 64
                || rule
                    .models
                    .iter()
                    .any(|m| m.is_empty() || m.len() > 256 || m.trim() != m)
                || rule.models.iter().collect::<BTreeSet<_>>().len() != rule.models.len()
                || (rule.models.iter().any(|m| m == "*") && rule.models.len() != 1)
                || rule.protocol.as_deref().is_some_and(|p| {
                    !matches!(
                        p,
                        "openai/chat-completions" | "openai/responses" | "anthropic/messages"
                    )
                })
            {
                return Err(RequestPolicyError("request_policy_scope"));
            }
            let mut fields = BTreeSet::new();
            for (name, value) in rule.default.iter().chain(&rule.r#override) {
                if !fields.insert(name.as_str()) {
                    return Err(RequestPolicyError("request_policy_conflict"));
                }
                validate_field(target, name, Some(value))?;
            }
            for name in &rule.filter {
                if !fields.insert(name.as_str()) {
                    return Err(RequestPolicyError("request_policy_conflict"));
                }
                validate_field(target, name, None)?;
            }
            if fields.contains("max_tokens") && fields.contains("max_completion_tokens") {
                return Err(RequestPolicyError("request_policy_output_limit_conflict"));
            }
            if fields.is_empty() {
                return Err(RequestPolicyError("request_policy_empty_rule"));
            }
        }
        Ok(())
    }

    /// Apply three stable phases to a translated target body. No rule inspects client headers.
    ///
    /// # Errors
    /// Rejects a non-object body; callers must validate the resulting target protocol again.
    pub fn apply(
        &self,
        body: &mut Value,
        model: &str,
        source: &str,
    ) -> Result<(), RequestPolicyError> {
        let root = body
            .as_object_mut()
            .ok_or(RequestPolicyError("request_policy_body"))?;
        let matching: Vec<_> = self
            .rules
            .iter()
            .filter(|r| {
                r.protocol.as_deref().is_none_or(|p| p == source)
                    && r.models.iter().any(|m| m == "*" || m == model)
            })
            .collect();
        for rule in &matching {
            for (name, value) in &rule.default {
                root.entry(name.clone()).or_insert_with(|| value.clone());
            }
        }
        for rule in &matching {
            for (name, value) in &rule.r#override {
                root.insert(name.clone(), value.clone());
            }
        }
        for rule in &matching {
            for name in &rule.filter {
                root.remove(name);
            }
        }
        Ok(())
    }
}

fn validate_field(
    target: &str,
    name: &str,
    value: Option<&Value>,
) -> Result<(), RequestPolicyError> {
    let allowed = match target {
        "openai/chat-completions" => matches!(
            name,
            "temperature"
                | "top_p"
                | "stop"
                | "seed"
                | "presence_penalty"
                | "frequency_penalty"
                | "max_tokens"
                | "max_completion_tokens"
        ),
        "openai/responses" => matches!(name, "temperature" | "top_p" | "max_output_tokens"),
        "anthropic/messages" => matches!(
            name,
            "temperature" | "top_p" | "top_k" | "stop_sequences" | "max_tokens"
        ),
        _ => false,
    };
    if !allowed {
        return Err(RequestPolicyError("request_policy_forbidden_field"));
    }
    let Some(value) = value else {
        return if target == "anthropic/messages" && name == "max_tokens" {
            Err(RequestPolicyError("request_policy_required_field"))
        } else {
            Ok(())
        };
    };
    let valid = match name {
        "temperature" => value.as_f64().is_some_and(|v| {
            (0.0..=if target == "anthropic/messages" {
                1.0
            } else {
                2.0
            })
                .contains(&v)
        }),
        "top_p" => value.as_f64().is_some_and(|v| (0.0..=1.0).contains(&v)),
        "presence_penalty" | "frequency_penalty" => {
            value.as_f64().is_some_and(|v| (-2.0..=2.0).contains(&v))
        }
        "seed" => value.as_i64().is_some(),
        "top_k" | "max_tokens" | "max_output_tokens" | "max_completion_tokens" => value
            .as_u64()
            .is_some_and(|v| v > 0 && u32::try_from(v).is_ok()),
        "stop" if value.is_string() => value
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 1024),
        "stop" | "stop_sequences" => value.as_array().is_some_and(|a| {
            !a.is_empty()
                && a.len() <= 4
                && a.iter()
                    .all(|v| v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 1024))
        }),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(RequestPolicyError("request_policy_invalid_value"))
    }
}

/// Custom headers cannot replace account, internal, negotiation, origin or framing headers.
///
/// # Errors
/// Rejects noncanonical names and provider/transport-owned names without reflecting input.
pub fn validate_header_name(name: &str) -> Result<(), RequestPolicyError> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || [
            "auth",
            "token",
            "key",
            "cookie",
            "secret",
            "credential",
            "proxy",
            "internal",
        ]
        .iter()
        .any(|s| name.contains(s))
        || name.starts_with("x-cpar-")
        || name.starts_with("x-msh-")
        || name.starts_with("anthropic-")
        || name.starts_with("openai-")
        || matches!(
            name,
            "accept"
                | "accept-encoding"
                | "content-type"
                | "content-encoding"
                | "content-length"
                | "host"
                | "connection"
                | "expect"
                | "keep-alive"
                | "te"
                | "trailer"
                | "transfer-encoding"
                | "upgrade"
                | "chatgpt-account-id"
                | "originator"
                | "version"
                | "origin"
                | "referer"
                | "user-agent"
        )
    {
        return Err(RequestPolicyError("request_policy_forbidden_header"));
    }
    Ok(())
}

/// Values remain in the existing encrypted Secret Store and are validated before publication.
///
/// # Errors
/// Rejects empty/oversize/non-ASCII values, CR/LF and all control characters.
pub fn validate_header_value(value: &[u8]) -> Result<(), RequestPolicyError> {
    if value.is_empty() || value.len() > 4096 || !value.iter().all(|b| (32..=126).contains(b)) {
        return Err(RequestPolicyError("request_policy_invalid_header_value"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_policy_phases_scope_and_protected_fields() -> Result<(), Box<dyn Error>> {
        let policy: RequestPolicy = serde_json::from_value(json!({"rules":[
            {"models":["public"],"protocol":"openai/responses","default":{"temperature":0.2,"top_p":0.7}},
            {"models":["public"],"default":{"temperature":0.3},"override":{"top_p":0.4}},
            {"models":["public"],"override":{"top_p":0.5},"filter":["temperature"]}
        ]}))?;
        policy.validate("openai-compatible.responses", "openai/responses")?;
        let mut body = json!({"model":"actual","input":"question","temperature":0.9});
        policy.apply(&mut body, "public", "openai/responses")?;
        assert_eq!(
            body,
            json!({"model":"actual","input":"question","top_p":0.5})
        );
        let mut other = json!({"model":"actual"});
        policy.apply(&mut other, "other", "openai/responses")?;
        assert_eq!(other, json!({"model":"actual"}));
        for field in [
            "model",
            "input",
            "messages",
            "metadata",
            "client_metadata",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "previous_response_id",
            "store",
            "stream",
            "reasoning",
            "service_tier",
            "authorization",
            "owner",
            "user",
            "max_output_tokens.nested",
            "input.0",
            "/temperature",
        ] {
            let mut policy = policy.clone();
            policy.rules[0].r#override.insert(field.into(), json!(1));
            assert!(
                policy
                    .validate("openai-compatible.responses", "openai/responses")
                    .is_err(),
                "{field}"
            );
        }
        Ok(())
    }

    #[test]
    fn request_policy_rejects_invalid_values_conflicts_and_headers() -> Result<(), Box<dyn Error>> {
        for (field, value) in [
            ("temperature", json!(3)),
            ("top_p", json!(-1)),
            ("max_output_tokens", json!(0)),
            ("max_output_tokens", json!(1.5)),
            ("temperature", Value::Null),
        ] {
            let policy: RequestPolicy = serde_json::from_value(
                json!({"rules":[{"models":["*"],"override":{field:value}}]}),
            )?;
            assert!(
                policy
                    .validate("openai-compatible.responses", "openai/responses")
                    .is_err()
            );
        }
        let conflict: RequestPolicy = serde_json::from_value(
            json!({"rules":[{"models":["*"],"default":{"temperature":0.1},"filter":["temperature"]}]}),
        )?;
        assert!(
            conflict
                .validate("openai-compatible.responses", "openai/responses")
                .is_err()
        );
        for name in [
            "Authorization",
            "authorization",
            "x-api-key",
            "content-type",
            "host",
            "accept",
            "connection",
            "x-cpar-owner",
            "x-msh-device-id",
            "anthropic-beta",
            "openai-beta",
            "bad name",
            "bad\r\nname",
        ] {
            assert!(validate_header_name(name).is_err());
        }
        validate_header_name("x-request-tag")?;
        for value in [b"".as_slice(), b"a\r\nb", b"a\0b", b"a\tb", &[0xff]] {
            assert!(validate_header_value(value).is_err());
        }
        assert!(validate_header_value(&vec![b'a'; 4097]).is_err());
        validate_header_value(b"opaque-value")?;
        Ok(())
    }
}
