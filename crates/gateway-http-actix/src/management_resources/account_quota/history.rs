//! History projection deliberately excludes identity, raw labels, reset strings and credentials.
use super::AccountQuotaObservation;
use gateway_store::quota_history::Sample;
use sha2::{Digest, Sha256};

impl AccountQuotaObservation {
    /// Produces bounded non-secret quota evidence; unknown units and timestamps stay unknown.
    #[must_use]
    pub fn history_samples(&self) -> Vec<Sample> {
        let mut samples = Vec::new();
        for window in self.windows.iter().take(8) {
            let mut sample = Sample::missing(&self.source, self.observed_at_ms, false);
            sample.source = if self.source == "grok_web" && window.name == "web.credits" {
                "grpc"
            } else if self.source == "grok_build" {
                "billing"
            } else {
                "rest"
            }
            .into();
            sample.window = if self.source == "grok_build" && window.name != "build.credits" {
                // Product labels are free-form upstream text. Retain identity as a digest only.
                format!("build.product.{:x}", Sha256::digest(window.name.as_bytes()))
            } else {
                window.name.clone()
            };
            sample.used_percent = window.used_percent;
            sample.used = window.used;
            sample.limit = window.limit;
            sample.unit = window
                .unit
                .as_deref()
                .and_then(|u| match u.to_ascii_lowercase().as_str() {
                    "percent" => Some("percent"),
                    "requests" => Some("requests"),
                    "credits" => Some("credits"),
                    "tokens" => Some("tokens"),
                    _ => None,
                })
                .map(str::to_owned);
            sample.reset_at_ms = window
                .reset_at_ms
                .or_else(|| parse_reset(window.reset_at.as_deref()));
            sample.duration_seconds = window.duration_seconds;
            sample.confidence = "observed".into();
            sample.completeness =
                if self.partial || self.windows.len() > 8 || self.source == "kiro" {
                    "partial"
                } else {
                    "projected"
                }
                .into();
            sample.status = if window.used_percent.is_some_and(|p| p >= 100.0)
                || matches!((window.used,window.limit),(Some(u),Some(l)) if l>0.0 && u>=l)
            {
                "exhausted"
            } else if window.used_percent.is_some() || window.used.is_some() {
                "observed"
            } else {
                "unknown"
            }
            .into();
            samples.push(sample);
        }
        if samples.is_empty() {
            samples.push(Sample::missing(&self.source, self.observed_at_ms, false));
        }
        samples
    }
    /// Converts the existing Kimi observation without retaining profile data or error payloads.
    #[must_use]
    pub fn from_kimi(value: &provider_openai_compatible::KimiAccountObservation) -> Self {
        Self {
            source: "kimi".into(),
            observed_at_ms: value.observed_at_ms,
            email: None,
            plan: None,
            partial: !value.quota_available,
            windows: value
                .quota_windows
                .iter()
                .map(|w| super::AccountQuotaWindow {
                    name: w.window.clone(),
                    used_percent: Some(w.used_ratio * 100.0),
                    used: None,
                    limit: None,
                    unit: Some("percent".into()),
                    reset_at: w.reset_at.clone(),
                    reset_at_ms: None,
                    duration_seconds: None,
                })
                .collect(),
        }
    }
}
fn parse_reset(value: Option<&str>) -> Option<i64> {
    let parsed =
        time::OffsetDateTime::parse(value?, &time::format_description::well_known::Rfc3339).ok()?;
    i64::try_from(parsed.unix_timestamp_nanos() / 1_000_000)
        .ok()
        .filter(|v| *v >= 0)
}
