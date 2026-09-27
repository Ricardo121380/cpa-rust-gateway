//! Allowlisted Kimi Coding account observations. Never used as model authorization.
use serde::Serialize;
use serde_json::Value;

/// Safe metadata failure classification; never carries provider bodies or credentials.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum KimiMetadataFailure {
    /// No request result was supplied.
    NotObserved,
    /// The configured egress boundary rejected the destination.
    EgressDenied,
    /// Connection or response streaming failed.
    Transport,
    /// The bounded request deadline elapsed.
    Timeout,
    /// The upstream returned a non-success HTTP status.
    Http {
        /// Numeric HTTP status only; no response text.
        status: u16,
    },
    /// Response exceeded the metadata size limit.
    ResponseTooLarge,
    /// Response body was not JSON.
    InvalidJson,
    /// Upstream returned a successful but empty JSON object.
    EmptyResponse,
    /// JSON contained no recognized identity or quota observation.
    UnrecognizedResponse,
}

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
    /// Independent profile failure, when present.
    pub profile_error: Option<KimiMetadataFailure>,
    /// Independent quota failure, when present.
    pub quota_error: Option<KimiMetadataFailure>,
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
// Older Kimi OAuth responses report counts instead of ratios. Missing/invalid
// values and a zero denominator must remain unknown, never become a zero balance.
fn quota_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|number| number.is_finite() && *number >= 0.0)
}
fn counted_quota(entry: &Value, window: &str) -> Option<KimiQuotaWindow> {
    let limit = quota_number(&entry["limit"]).filter(|limit| *limit > 0.0)?;
    let used = if entry.get("used").is_some() {
        quota_number(&entry["used"])?
    } else {
        limit - quota_number(&entry["remaining"]).filter(|remaining| *remaining <= limit)?
    };
    let used_ratio = used / limit;
    used_ratio.is_finite().then(|| KimiQuotaWindow {
        window: window.to_owned(),
        used_ratio,
        reset_at: text(&entry["resetTime"]).or_else(|| text(&entry["reset_time"])),
    })
}
fn counted_windows(usage: &Value, windows: &mut Vec<KimiQuotaWindow>) {
    // The official Python Kimi CLI defines the top-level usage as the weekly summary.
    let mut candidates = Vec::new();
    if let Some(summary) = counted_quota(&usage["usage"], "limit_7d") {
        candidates.push(summary);
    }
    if let Some(limits) = usage["limits"].as_array() {
        for item in limits.iter().take(32) {
            let duration = quota_number(&item["window"]["duration"]);
            let seconds = match item["window"]["timeUnit"].as_str() {
                Some("TIME_UNIT_SECOND") => duration,
                Some("TIME_UNIT_MINUTE") => duration.map(|n| n * 60.0),
                Some("TIME_UNIT_HOUR") => duration.map(|n| n * 3600.0),
                Some("TIME_UNIT_DAY") => duration.map(|n| n * 86_400.0),
                _ => None,
            };
            let window = match seconds {
                Some(seconds) if (seconds - 18_000.0).abs() < f64::EPSILON => "limit_5h",
                Some(seconds) if (seconds - 604_800.0).abs() < f64::EPSILON => "limit_7d",
                _ => continue,
            };
            if let Some(value) = counted_quota(&item["detail"], window) {
                candidates.push(value);
            }
        }
    }
    for candidate in candidates {
        if !windows
            .iter()
            .any(|window| window.window == candidate.window)
        {
            windows.push(candidate);
        }
    }
}
impl KimiAccountObservation {
    /// Retains independent safe failures while projecting any successful counterpart.
    #[must_use]
    pub fn from_results(
        profile: Result<Value, KimiMetadataFailure>,
        usage: Result<Value, KimiMetadataFailure>,
        observed_at_ms: i64,
    ) -> Self {
        let mut observation =
            Self::from_payloads(profile.as_ref().ok(), usage.as_ref().ok(), observed_at_ms);
        if let Err(error) = profile {
            observation.profile_error = Some(error);
        }
        if let Err(error) = usage {
            observation.quota_error = Some(error);
        }
        observation
    }

    /// Projects only human identity and quota fields from the two official metadata APIs.
    #[must_use]
    pub fn from_payloads(
        profile: Option<&Value>,
        usage: Option<&Value>,
        observed_at_ms: i64,
    ) -> Self {
        let profile_supplied = profile.is_some();
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
        if let Some(usage) = usage {
            counted_windows(usage, &mut quota_windows);
        }
        Self {
            observed_at_ms,
            profile_available: profile.is_some(),
            quota_available: !quota_windows.is_empty(),
            profile_error: if profile.is_some() {
                None
            } else {
                Some(if profile_supplied {
                    KimiMetadataFailure::UnrecognizedResponse
                } else {
                    KimiMetadataFailure::NotObserved
                })
            },
            quota_error: if quota_windows.is_empty() {
                Some(
                    if usage.is_some_and(|value| {
                        value.as_object().is_some_and(serde_json::Map::is_empty)
                    }) {
                        KimiMetadataFailure::EmptyResponse
                    } else if usage.is_some() {
                        KimiMetadataFailure::UnrecognizedResponse
                    } else {
                        KimiMetadataFailure::NotObserved
                    },
                )
            } else {
                None
            },
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
    fn keeps_usage_failure_without_discarding_profile() -> Result<(), serde_json::Error> {
        for failure in [
            KimiMetadataFailure::Timeout,
            KimiMetadataFailure::Http { status: 403 },
            KimiMetadataFailure::Transport,
            KimiMetadataFailure::EgressDenied,
            KimiMetadataFailure::InvalidJson,
            KimiMetadataFailure::ResponseTooLarge,
        ] {
            let observation = KimiAccountObservation::from_results(
                Ok(json!({"user_id":"subject","email":"member@example.test"})),
                Err(failure.clone()),
                100,
            );
            assert!(observation.profile_available);
            assert_eq!(observation.profile_error, None);
            assert_eq!(observation.quota_error, Some(failure));
            assert!(!observation.quota_available);
            assert_eq!(
                observation.identity.email.as_deref(),
                Some("member@example.test")
            );
        }
        let encoded = serde_json::to_string(&KimiMetadataFailure::Http { status: 429 })?;
        assert_eq!(encoded, r#"{"code":"http","status":429}"#);
        Ok(())
    }
    #[test]
    fn preserves_zero_usage_when_profile_fails_and_distinguishes_missing_shape() {
        let observation = KimiAccountObservation::from_results(
            Err(KimiMetadataFailure::Timeout),
            Ok(json!({"usages":{"limit_5h":{"used_ratio":0}}})),
            100,
        );
        assert!(observation.quota_available);
        assert_eq!(observation.quota_error, None);
        assert_eq!(
            observation.profile_error,
            Some(KimiMetadataFailure::Timeout)
        );
        let empty =
            KimiAccountObservation::from_results(Ok(json!({})), Ok(json!({"usages":{}})), 100);
        assert_eq!(
            empty.quota_error,
            Some(KimiMetadataFailure::UnrecognizedResponse)
        );
        assert_eq!(
            empty.profile_error,
            Some(KimiMetadataFailure::UnrecognizedResponse)
        );
    }
    #[test]
    fn parses_oauth_counts_and_remaining_without_inventing_missing_usage() {
        let usage = json!({"usage":{"limit":"100","remaining":"75","resetTime":"2026-10-01T00:00:00Z"},
            "limits":[{"window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE"},
                "detail":{"limit":"200","used":"0"}}]});
        let observation = KimiAccountObservation::from_payloads(None, Some(&usage), 100);
        assert!(observation.quota_available);
        assert_eq!(observation.quota_windows.len(), 2);
        assert_eq!(observation.quota_windows[0].window, "limit_7d");
        assert!((observation.quota_windows[0].used_ratio - 0.25).abs() < f64::EPSILON);
        assert_eq!(
            observation.quota_windows[0].reset_at.as_deref(),
            Some("2026-10-01T00:00:00Z")
        );
        assert!(observation.quota_windows[1].used_ratio.abs() < f64::EPSILON);
        for invalid in [
            json!({"limit":0,"used":0}),
            json!({"limit":100}),
            json!({"limit":100,"remaining":101}),
            json!({"limit":100,"used":"NaN"}),
            json!({"limit":100,"used":null,"remaining":100}),
        ] {
            let invalid = json!({"usage":invalid});
            assert!(
                !KimiAccountObservation::from_payloads(None, Some(&invalid), 100).quota_available
            );
        }
    }
    #[test]
    fn ratio_format_wins_over_duplicate_counted_window() {
        let usage =
            json!({"usages":{"limit_7d":{"used_ratio":0.5}},"usage":{"limit":100,"used":25}});
        let observation = KimiAccountObservation::from_payloads(None, Some(&usage), 100);
        assert_eq!(observation.quota_windows.len(), 1);
        assert!((observation.quota_windows[0].used_ratio - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn failures_and_unrecognized_usage_are_not_zero_balances() {
        let observation = KimiAccountObservation::from_payloads(None, Some(&json!({})), 100);
        assert!(!observation.profile_available && !observation.quota_available);
        assert!(observation.identity.email.is_none() && observation.quota_windows.is_empty());
        assert_eq!(
            observation.quota_error,
            Some(KimiMetadataFailure::EmptyResponse)
        );
    }
}
