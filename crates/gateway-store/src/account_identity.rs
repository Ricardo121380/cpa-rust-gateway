//! Display-only identity evidence. It never grants access or interprets opaque subjects as names.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use serde_json::Value;
use zeroize::Zeroize;

/// Allowlisted human identity, available only to authenticated management callers.
#[derive(Clone, Default, Serialize, PartialEq, Eq)]
pub struct AccountIdentity {
    /// Account email, when explicitly observed.
    pub email: Option<String>,
    /// Account phone, when explicitly observed.
    pub phone: Option<String>,
    /// Account username, excluding opaque IDs and machine-generated labels.
    pub username: Option<String>,
}
impl std::fmt::Debug for AccountIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccountIdentity(<redacted>)")
    }
}
impl AccountIdentity {
    /// Projects only human identity fields from bounded stored credential JSON and JWT claims.
    /// JWT fields are display evidence, not verified authentication or entitlement claims.
    #[must_use]
    pub fn from_credential(bytes: &[u8]) -> Self {
        let mut result = Self::default();
        if bytes.len() > 65_536 {
            return result;
        }
        if let Ok(mut value) = serde_json::from_slice::<Value>(bytes) {
            result.visit(&value, 0);
            wipe_json(&mut value);
        }
        result
    }
    /// Extracts display fields from a JWT payload without using its claims for authorization.
    #[must_use]
    pub fn from_token(token: &str) -> Self {
        let mut parts = token.split('.');
        if let (Some(_), Some(payload), Some(_), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
            && payload.len() <= 16_384
            && let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload)
        {
            return Self::from_credential(&zeroize::Zeroizing::new(bytes));
        }
        Self::default()
    }
    /// Whether the provider supplied any human identity.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.email.is_none() && self.phone.is_none() && self.username.is_none()
    }
    /// Retains earlier provider observations when a refresh omits profile fields.
    pub fn retain_missing(&mut self, previous: &Self) {
        self.email = self.email.take().or_else(|| previous.email.clone());
        self.phone = self.phone.take().or_else(|| previous.phone.clone());
        self.username = self.username.take().or_else(|| previous.username.clone());
    }
    fn visit(&mut self, value: &Value, depth: usize) {
        if depth > 4 {
            return;
        }
        let text = |keys: &[&str]| keys.iter().find_map(|key| value.get(key)?.as_str());
        if self.email.is_none() {
            self.email = text(&["email", "email_address"]).and_then(email);
        }
        if self.phone.is_none() {
            self.phone = text(&["phone_number", "phone"]).and_then(phone);
        }
        if self.username.is_none() {
            self.username = text(&["preferred_username", "username", "display_name", "name"])
                .and_then(human_name);
        }
        for key in [
            "user",
            "profile",
            "account",
            "metadata",
            "_meta",
            "extra",
            "credentials",
            "tokens",
            "https://api.openai.com/profile",
        ] {
            if let Some(nested) = value.get(key) {
                self.visit(nested, depth + 1);
            }
        }
        if depth < 2 {
            for key in ["id_token", "access_token", "accessToken"] {
                if let Some(token) = value.get(key).and_then(Value::as_str) {
                    let mut parts = token.split('.');
                    if let (Some(_), Some(payload), Some(_), None) =
                        (parts.next(), parts.next(), parts.next(), parts.next())
                        && payload.len() <= 16_384
                        && let Some(mut claims) = URL_SAFE_NO_PAD
                            .decode(payload)
                            .ok()
                            .and_then(|v| serde_json::from_slice::<Value>(&v).ok())
                    {
                        self.visit(&claims, depth + 2);
                        wipe_json(&mut claims);
                    }
                }
            }
        }
    }
}
fn wipe_json(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Array(items) => items.iter_mut().for_each(wipe_json),
        Value::Object(fields) => fields.values_mut().for_each(wipe_json),
        _ => (),
    }
}
fn bounded(value: &str) -> Option<&str> {
    let text = value.trim();
    (!text.is_empty() && text.len() <= 254 && !text.chars().any(char::is_control)).then_some(text)
}
fn email(value: &str) -> Option<String> {
    let value = bounded(value)?;
    let (local, domain) = value.split_once('@')?;
    (!local.is_empty()
        && domain.contains('.')
        && !value.chars().any(char::is_whitespace)
        && !domain.contains('@'))
    .then(|| value.to_owned())
}
fn phone(value: &str) -> Option<String> {
    let value = bounded(value)?;
    let digits = value.chars().filter(char::is_ascii_digit).count();
    ((7..=15).contains(&digits)
        && value
            .chars()
            .all(|c| c.is_ascii_digit() || "+- ()".contains(c)))
    .then(|| value.to_owned())
}
/// Accepts explicit operator usernames while rejecting phase IDs, hashes and import batch labels.
#[must_use]
pub fn human_name(value: &str) -> Option<String> {
    let value = bounded(value)?;
    let lower = value.to_ascii_lowercase();
    if value.len() > 80
        || lower.contains("test")
        || value.contains("测试")
        || value.contains("验收")
        || lower.contains("autoreg")
        || lower.starts_with("p12")
        || lower.starts_with("p13")
        || lower.starts_with("grok-")
        || ["sk-", "ksk_", "mgmt_", "csrf_", "bearer ", "eyj"]
            .iter()
            .any(|p| lower.starts_with(p))
        || value.split('.').count() == 3
        || value
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|p| p.len() >= 8 && p.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return None;
    }
    Some(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_only_human_fields_and_never_opaque_subject_or_secret()
    -> Result<(), Box<dyn std::error::Error>> {
        let identity=AccountIdentity::from_credential(br#"{"email":"member@example.test","access_token":"fixture-token","refresh_token":"fixture-refresh","sub":"opaque-user","extra":{"plan":"pro"}}"#);
        assert_eq!(identity.email.as_deref(), Some("member@example.test"));
        let encoded = serde_json::to_string(&identity)?;
        assert!(!encoded.contains("fixture-token") && !encoded.contains("opaque-user"));
        assert_eq!(
            AccountIdentity::from_credential(br#"{"sub":"opaque-user","account_id":"opaque-id"}"#),
            AccountIdentity::default()
        );
        assert!(human_name("autoreg-production-batch").is_none());
        assert!(human_name("p12-06-codex-bridge-credential").is_none());
        assert!(human_name("A8CD43F1").is_none());
        assert!(!format!("{identity:?}").contains("member@"));
        Ok(())
    }
    #[test]
    fn projects_jwt_email_without_returning_tokens_or_auth_claims()
    -> Result<(), Box<dyn std::error::Error>> {
        let claims = URL_SAFE_NO_PAD
            .encode(br#"{"email":"person@example.test","sub":"id","scope":"admin"}"#);
        let value = serde_json::json!({"tokens":{"id_token":format!("header.{claims}.signature")}});
        let result = AccountIdentity::from_credential(&serde_json::to_vec(&value)?);
        assert_eq!(result.email.as_deref(), Some("person@example.test"));
        assert_eq!(result.username, None);
        Ok(())
    }
}
