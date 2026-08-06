//! Console v3.1.1 usage/quota projection.

use serde::Deserialize;
use std::{
    fmt,
    time::{Duration, SystemTime},
};

const RECOVERY_WINDOW: Duration = Duration::from_hours(24);

/// One validated Console quota window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokConsoleQuotaWindow {
    /// Console quota kind (`chat`, `image`, or `video`).
    pub kind: GrokConsoleQuotaKind,
    /// Configured quota limit.
    pub limit: u64,
    /// Consumed units reported by the upstream.
    pub used: u64,
    /// Remaining units reported by the upstream.
    pub remaining: u64,
    /// Predicted recovery deadline for an exhausted chat quota.
    pub reset_at: Option<SystemTime>,
}

/// Supported Console quota kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokConsoleQuotaKind {
    /// Conversation quota.
    Chat,
    /// Image quota.
    Image,
    /// Video quota.
    Video,
}

/// A complete usage response projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokConsoleUsageSnapshot {
    /// All three required quota windows in deterministic order.
    pub windows: Vec<GrokConsoleQuotaWindow>,
    /// Observation time supplied by the caller.
    pub observed_at: SystemTime,
}

/// Stable value-free usage parsing failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokConsoleQuotaError {
    /// The response body is not valid JSON.
    InvalidJson,
    /// One of chat, image, or video is absent.
    MissingWindow,
    /// A counter is negative or exceeds its limit.
    InvalidBounds,
    /// A quota kind appears more than once.
    DuplicateWindow,
}

impl fmt::Display for GrokConsoleQuotaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidJson => "Console usage JSON is invalid",
            Self::MissingWindow => "Console usage is missing a required window",
            Self::InvalidBounds => "Console usage quota bounds are invalid",
            Self::DuplicateWindow => "Console usage contains a duplicate window",
        })
    }
}
impl std::error::Error for GrokConsoleQuotaError {}

/// Parses the v3.1.1 `/usage` response without retaining its raw body.
#[allow(clippy::missing_errors_doc)]
pub fn parse_grok_console_usage(
    body: &[u8],
    observed_at: SystemTime,
) -> Result<GrokConsoleUsageSnapshot, GrokConsoleQuotaError> {
    let payload = serde_json::from_slice::<UsagePayload>(body)
        .map_err(|_| GrokConsoleQuotaError::InvalidJson)?;
    let mut windows = Vec::with_capacity(3);
    for quota in payload.quotas {
        let kind = match quota.kind.trim().to_ascii_lowercase().as_str() {
            "chat" => GrokConsoleQuotaKind::Chat,
            "image" => GrokConsoleQuotaKind::Image,
            "video" => GrokConsoleQuotaKind::Video,
            _ => continue,
        };
        if quota.limit < 0 || quota.used < 0 || quota.remaining < 0 || quota.remaining > quota.limit
        {
            return Err(GrokConsoleQuotaError::InvalidBounds);
        }
        if windows
            .iter()
            .any(|window: &GrokConsoleQuotaWindow| window.kind == kind)
        {
            return Err(GrokConsoleQuotaError::DuplicateWindow);
        }
        windows.push(GrokConsoleQuotaWindow {
            kind,
            limit: quota.limit.cast_unsigned(),
            used: quota.used.cast_unsigned(),
            remaining: quota.remaining.cast_unsigned(),
            reset_at: (kind == GrokConsoleQuotaKind::Chat && quota.remaining == 0)
                .then(|| observed_at + RECOVERY_WINDOW),
        });
    }
    if ![
        GrokConsoleQuotaKind::Chat,
        GrokConsoleQuotaKind::Image,
        GrokConsoleQuotaKind::Video,
    ]
    .iter()
    .all(|kind| windows.iter().any(|window| window.kind == *kind))
    {
        return Err(GrokConsoleQuotaError::MissingWindow);
    }
    windows.sort_by_key(|window| match window.kind {
        GrokConsoleQuotaKind::Chat => 0,
        GrokConsoleQuotaKind::Image => 1,
        GrokConsoleQuotaKind::Video => 2,
    });
    Ok(GrokConsoleUsageSnapshot {
        windows,
        observed_at,
    })
}

#[derive(Deserialize)]
struct UsagePayload {
    quotas: Vec<UsageQuota>,
}
#[derive(Deserialize)]
struct UsageQuota {
    kind: String,
    limit: i64,
    used: i64,
    remaining: i64,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn projects_complete_usage_and_predicts_chat_recovery() {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let body = br#"{"quotas":[{"kind":"video","limit":3,"used":1,"remaining":2},{"kind":"chat","limit":10,"used":10,"remaining":0},{"kind":"image","limit":5,"used":2,"remaining":3}]}"#;
        let snapshot = parse_grok_console_usage(body, now).expect("usage");
        assert_eq!(snapshot.windows.len(), 3);
        assert_eq!(snapshot.windows[0].kind, GrokConsoleQuotaKind::Chat);
        assert_eq!(snapshot.windows[0].reset_at, Some(now + RECOVERY_WINDOW));
    }

    #[test]
    fn rejects_incomplete_or_invalid_usage() {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(
            parse_grok_console_usage(br#"{"quotas":[]}"#, now),
            Err(GrokConsoleQuotaError::MissingWindow)
        );
        assert_eq!(parse_grok_console_usage(br#"{"quotas":[{"kind":"chat","limit":1,"used":0,"remaining":-1},{"kind":"image","limit":1,"used":0,"remaining":1},{"kind":"video","limit":1,"used":0,"remaining":1}]}"#, now), Err(GrokConsoleQuotaError::InvalidBounds));
    }
}
