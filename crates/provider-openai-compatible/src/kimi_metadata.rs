//! Allowlisted Kimi Coding account observations. Never used as model authorization.
use serde::Serialize;
use serde_json::Value;

/// Provider-returned human identity; masked phone numbers remain masked.
#[derive(Clone, Default, Serialize)]
pub struct KimiIdentity {
    /// Observed email.
    pub email: Option<String>,
    /// Observed phone, including provider masking.
    pub phone: Option<String>,
    /// Observed nickname or username.
    pub username: Option<String>,
}
/// One independently reported quota window, not a token balance.
#[derive(Clone, Serialize)]
pub struct KimiQuotaWindow {
    /// Provider window identifier.
    pub window: String,
    /// Provider used ratio; not inferred from local requests.
    pub used_ratio: f64,
    /// Provider reset timestamp, when present.
    pub reset_at: Option<String>,
}
/// Bounded account metadata snapshot, independent of credential secrets.
#[derive(Clone, Serialize)]
pub struct KimiAccountObservation {
    /// Time at which these reads completed.
    pub observed_at_ms: i64,
    /// Profile read succeeded; a successful profile may omit individual fields.
    pub profile_available: bool,
    /// Usage read succeeded and supplied at least one recognized quota window.
    pub quota_available: bool,
    /// Explicitly observed identity.
    pub identity: KimiIdentity,
    /// Official plan label, when supplied.
    pub plan: Option<String>,
    /// Individually reported quota windows. Missing windows stay unknown.
    pub quota_windows: Vec<KimiQuotaWindow>,
}
fn text(value: &Value) -> Option<String> {
    let s = value.as_str()?.trim();
    (!s.is_empty() && s.len() <= 254 && !s.chars().any(char::is_control)).then(|| s.to_owned())
}
impl KimiAccountObservation {
    /// Projects only human identity and quota fields from the two official metadata APIs.
    #[must_use]
    pub fn from_payloads(
        profile: Option<&Value>,
        usage: Option<&Value>,
        observed_at_ms: i64,
    ) -> Self {
        let profile = profile.filter(|p| text(&p["user_id"]).is_some());
        let identity = profile.map_or_else(KimiIdentity::default, |p| {
            let phone = text(&p["phone"]["number"])
                .filter(|s| {
                    s.len() <= 32
                        && s.chars()
                            .all(|c| c.is_ascii_digit() || "+- ()*".contains(c))
                })
                .map(|number| {
                    match text(&p["phone"]["country_code"])
                        .filter(|s| s.len() <= 4 && s.chars().all(|c| c.is_ascii_digit()))
                    {
                        Some(code) => format!("+{code} {number}"),
                        None => number,
                    }
                });
            KimiIdentity {
                email: text(&p["email"]).filter(|s| s.contains('@')),
                phone,
                username: text(&p["username"]).or_else(|| text(&p["nickname"])),
            }
        });
        let mut quota_windows = Vec::new();
        if let Some(usage) = usage {
            for window in [
                "limit_5h",
                "limit_7d",
                "limit_month_total",
                "limit_month_code",
            ] {
                let entry = &usage["usages"][window];
                let ratio = entry["used_ratio"]
                    .as_f64()
                    .or_else(|| entry["used_ratio"].as_str()?.parse().ok());
                if let Some(used_ratio) = ratio.filter(|r| r.is_finite() && *r >= 0.0) {
                    quota_windows.push(KimiQuotaWindow {
                        window: window.to_owned(),
                        used_ratio,
                        reset_at: text(&entry["reset_time"]),
                    });
                }
            }
        }
        Self {
            observed_at_ms,
            profile_available: profile.is_some(),
            quota_available: !quota_windows.is_empty(),
            identity,
            plan: profile
                .and_then(|p| text(&p["user_level_name"]))
                .filter(|s| s.len() <= 128),
            quota_windows,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn projects_masked_phone_and_independent_quota_windows() -> Result<(), serde_json::Error> {
        let profile = json!({"user_id":"opaque-id","phone":{"country_code":"86","number":"176****0000"},"nickname":"Moonwalker","user_level_name":"Vivace","access_token":"never expose"});
        let usage = json!({"usages":{"limit_5h":{"used_ratio":"0.25","reset_time":"2026-09-27T12:00:00Z"},"limit_7d":{"used_ratio":0},"limit_month_code":{"used_ratio":-1}}});
        let observation = KimiAccountObservation::from_payloads(Some(&profile), Some(&usage), 100);
        assert_eq!(
            observation.identity.phone.as_deref(),
            Some("+86 176****0000")
        );
        assert_eq!(observation.identity.email, None);
        assert_eq!(observation.quota_windows.len(), 2);
        assert!((observation.quota_windows[0].used_ratio - 0.25).abs() < f64::EPSILON);
        let encoded = serde_json::to_string(&observation)?;
        assert!(!encoded.contains("opaque-id") && !encoded.contains("never expose"));
        Ok(())
    }
    #[test]
    fn failures_and_unrecognized_usage_are_not_zero_balances() {
        let observation = KimiAccountObservation::from_payloads(None, Some(&json!({})), 100);
        assert!(!observation.profile_available && !observation.quota_available);
        assert!(observation.identity.email.is_none() && observation.quota_windows.is_empty());
    }
}
