//! Allowlisted account usage observations, never model grants or billing balances.
use serde::Serialize;
use serde_json::Value;

/// Provider-reported quota windows; absent values remain absent.
#[derive(Clone, Serialize)]
pub struct AccountQuotaObservation {
    /// Channel whose private account usage API was read.
    pub source: String,
    /// Completed observation timestamp.
    pub observed_at_ms: i64,
    /// Independently reported windows.
    pub windows: Vec<AccountQuotaWindow>,
    /// Explicit human identity from the same provider observation, never an opaque subject.
    pub email: Option<String>,
    /// Provider-reported subscription label, not a model grant.
    pub plan: Option<String>,
    /// Some upstream quota components could not be projected; never a complete balance.
    pub partial: bool,
}
/// A usage percentage in an explicitly identified window.
#[derive(Clone, Serialize)]
pub struct AccountQuotaWindow {
    /// Stable upstream window identity; no inferred 5h/weekly positional naming.
    pub name: String,
    /// Used percentage, not remaining tokens or dollars.
    pub used_percent: f64,
    /// Provider numerical usage and limit, when both are observed.
    pub used: Option<f64>,
    /// Numerical limit in the stated unit, not a monetary balance.
    pub limit: Option<f64>,
    /// Provider-reported unit; no conversion to tokens or currency.
    pub unit: Option<String>,
    /// Provider reset timestamp, if supplied.
    pub reset_at: Option<String>,
    /// Provider reset Unix epoch milliseconds, if supplied.
    pub reset_at_ms: Option<i64>,
    /// Explicit duration, if supplied.
    pub duration_seconds: Option<i64>,
}
impl AccountQuotaObservation {
    /// Projects only recognized numerical usage windows from a bounded JSON response.
    #[must_use]
    pub fn parse(source: &str, value: &Value, now: i64) -> Self {
        let mut windows = Vec::new();
        if source == "codex" {
            for group in ["rate_limit", "code_review_rate_limit"] {
                for name in ["primary_window", "secondary_window"] {
                    let entry = &value[group][name];
                    if let Some(used_percent) = percentage(&entry["used_percent"]) {
                        windows.push(AccountQuotaWindow {
                            name: format!("{group}.{name}"),
                            used_percent,
                            used: None,
                            limit: None,
                            unit: None,
                            reset_at: None,
                            reset_at_ms: entry["reset_at"]
                                .as_i64()
                                .filter(|v| *v >= 0)
                                .and_then(|v| v.checked_mul(1000)),
                            duration_seconds: entry["limit_window_seconds"]
                                .as_i64()
                                .filter(|v| *v > 0),
                        });
                    }
                }
            }
        } else if source == "claude" {
            for name in [
                "five_hour",
                "seven_day",
                "seven_day_oauth_apps",
                "seven_day_opus",
                "seven_day_sonnet",
                "seven_day_cowork",
            ] {
                let entry = &value[name];
                if let Some(used_percent) = percentage(&entry["utilization"]) {
                    windows.push(AccountQuotaWindow {
                        name: name.to_owned(),
                        used_percent,
                        used: None,
                        limit: None,
                        unit: None,
                        reset_at: entry["resets_at"]
                            .as_str()
                            .filter(|s| s.len() <= 64 && !s.chars().any(char::is_control))
                            .map(str::to_owned),
                        reset_at_ms: None,
                        duration_seconds: None,
                    });
                }
            }
        }
        let (email, plan) = if source == "kiro" {
            kiro_usage(value, &mut windows)
        } else {
            (None, None)
        };
        let partial = source == "kiro"
            && value["usageBreakdownList"]
                .as_array()
                .is_none_or(|entries| {
                    entries.len() != windows.len()
                        || entries.iter().any(|v| {
                            !v["freeTrialInfo"].is_null()
                                || v["bonuses"].as_array().is_some_and(|a| !a.is_empty())
                        })
                });
        Self {
            partial,
            email,
            plan,
            source: source.to_owned(),
            observed_at_ms: now,
            windows,
        }
    }
}
fn kiro_usage(
    value: &Value,
    windows: &mut Vec<AccountQuotaWindow>,
) -> (Option<String>, Option<String>) {
    if let Some(entries) = value["usageBreakdownList"].as_array() {
        for (index, entry) in entries.iter().take(6).enumerate() {
            let used = entry
                .get("currentUsageWithPrecision")
                .or_else(|| entry.get("currentUsage"))
                .and_then(percentage);
            let limit = entry
                .get("usageLimitWithPrecision")
                .or_else(|| entry.get("usageLimit"))
                .and_then(percentage);
            if let (Some(used), Some(limit)) = (used, limit) {
                let used_percent = used / limit * 100.0;
                if limit > 0.0 && used_percent.is_finite() {
                    windows.push(AccountQuotaWindow {
                        name: format!("kiro.resource.{index}"),
                        used_percent,
                        used: Some(used),
                        limit: Some(limit),
                        unit: bounded_text(&entry["unit"]),
                        reset_at: bounded_text(&value["nextDateReset"]).filter(|v| v.len() <= 64),
                        reset_at_ms: value["nextDateReset"]
                            .as_i64()
                            .filter(|v| *v >= 0)
                            .and_then(|v| v.checked_mul(1000)),
                        duration_seconds: None,
                    });
                }
            }
        }
    }
    let identity =
        serde_json::json!({"email":value["userInfo"].get("email").unwrap_or(&value["email"])});
    let display = gateway_store::account_identity::CredentialDisplay::from_credential(
        identity.to_string().as_bytes(),
    );
    (
        display.identity.email,
        bounded_text(&value["subscriptionInfo"]["subscriptionTitle"]),
    )
}
fn bounded_text(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control))
        .map(str::to_owned)
}
fn percentage(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
}
pub(super) async fn read(
    state: &super::ManagementResourceHttpState,
    version: gateway_store::control_plane::ConfigVersionId,
    credential_id: gateway_core::CredentialId,
) -> (Option<AccountQuotaObservation>, Option<&'static str>) {
    match state.catalog_refresh.as_ref() {
        Some(source) => match state.catalog_refresh_slots.clone().try_acquire_owned() {
            Ok(_permit) => match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                source.account_quota(version, credential_id),
            )
            .await
            {
                Ok(Ok(value)) => (Some(value), None),
                Ok(Err(super::catalog_refresh::CatalogRefreshError::Unsupported)) => {
                    (None, Some("not_implemented_or_not_connected"))
                }
                Ok(Err(super::catalog_refresh::CatalogRefreshError::Conflict)) => {
                    (None, Some("configuration_changed"))
                }
                _ => (None, Some("unavailable")),
            },
            Err(_) => (None, Some("unavailable")),
        },
        None => (None, Some("not_implemented_or_not_connected")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn kiro_identity_precision_and_partial_quota_remain_distinct()
    -> Result<(), Box<dyn std::error::Error>> {
        let observation = AccountQuotaObservation::parse(
            "kiro",
            &json!({
                "userInfo":{"email":"person@example.com","userId":"opaque"},
                "subscriptionInfo":{"subscriptionTitle":"KIRO PRO"},
                "usageBreakdownList":[{"currentUsage":12,"currentUsageWithPrecision":12.5,"usageLimit":100,"unit":"CREDITS","freeTrialInfo":{"usageLimit":10}}],
                "accessToken":"never expose"
            }),
            10,
        );
        assert_eq!(observation.email.as_deref(), Some("person@example.com"));
        assert_eq!(observation.windows[0].used, Some(12.5));
        assert!(observation.partial);
        let encoded = serde_json::to_string(&observation)?;
        assert!(!encoded.contains("never expose"));
        let missing = AccountQuotaObservation::parse(
            "kiro",
            &json!({"usageBreakdownList":[{"usageLimit":100},{"currentUsage":0,"usageLimit":0}]}),
            10,
        );
        assert!(missing.windows.is_empty());
        assert!(missing.partial);
        assert!(missing.email.is_none());
        Ok(())
    }
    #[test]
    fn codex_missing_window_is_not_zero_or_assumed_weekly() {
        let result = AccountQuotaObservation::parse(
            "codex",
            &json!({"rate_limit":{"primary_window":{"used_percent":0,"limit_window_seconds":3600,"reset_at":100}}}),
            1,
        );
        assert_eq!(result.windows.len(), 1);
        assert_eq!(result.windows[0].duration_seconds, Some(3600));
        assert_eq!(result.windows[0].reset_at_ms, Some(100_000));
    }
    #[test]
    fn claude_omits_missing_invalid_and_unknown_windows() {
        let result = AccountQuotaObservation::parse(
            "claude",
            &json!({"five_hour":{"utilization":"25"},"seven_day":{"utilization":-1},"secret":"not exposed"}),
            1,
        );
        assert_eq!(result.windows.len(), 1);
        assert!((result.windows[0].used_percent - 25.0).abs() < f64::EPSILON);
        assert!(
            AccountQuotaObservation::parse("claude", &json!({}), 1)
                .windows
                .is_empty()
        );
    }
}
