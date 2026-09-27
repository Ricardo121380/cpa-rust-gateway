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
}
/// A usage percentage in an explicitly identified window.
#[derive(Clone, Serialize)]
pub struct AccountQuotaWindow {
    /// Stable upstream window identity; no inferred 5h/weekly positional naming.
    pub name: String,
    /// Used percentage, not remaining tokens or dollars.
    pub used_percent: f64,
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
        Self {
            source: source.to_owned(),
            observed_at_ms: now,
            windows,
        }
    }
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
