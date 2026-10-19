//! Private confirmed summary snapshots, separate from Usage and client-visible thinking.
use crate::{ErrorScope, GatewayError, GatewayErrorCode, RawJson, ResponseId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, fmt};

/// Maximum complete summary payload per Attempt; exceeding it fails rather than truncates.
pub const MAX_MESSAGES_SUMMARY_BYTES: usize = 64 * 1024;

/// Fixed Build transport that supplied the validated native items.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessagesSummarySource {
    /// Completed JSON document.
    BuildJson,
    /// Incrementally validated SSE items.
    BuildSse,
}

/// What the decoder has actually confirmed, independent of downstream delivery.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessagesSummaryCompleteness {
    /// All native items and the completed native response were confirmed.
    CompletedResponse,
    /// Only the listed done items were confirmed before failure or cancellation.
    ConfirmedItems,
}

/// An entire reviewed reasoning item and its position among all native output items.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MessagesSummaryItem {
    /// Native output order, including intervening answer/tool items.
    pub output_index: usize,
    /// Confirmed item identity/status and ordered summary array. Absent body stays absent.
    pub item: RawJson,
}

/// Sensitive companion. It never belongs to a `GatewayEvent` or billing payload.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MessagesSummaryEvidence {
    source: MessagesSummarySource,
    response_id: ResponseId,
    completeness: MessagesSummaryCompleteness,
    confirmed_at_ms: i64,
    items: Vec<MessagesSummaryItem>,
}
impl MessagesSummaryEvidence {
    /// Validates the closed summary-only representation after native lifecycle confirmation.
    ///
    /// # Errors
    /// Rejects unsupported reasoning, invalid identity/order, or an over-bound payload.
    pub fn try_new(
        source: MessagesSummarySource,
        response_id: ResponseId,
        completeness: MessagesSummaryCompleteness,
        confirmed_at_ms: i64,
        items: Vec<MessagesSummaryItem>,
    ) -> Result<Self, GatewayError> {
        let invalid =
            || GatewayError::new(GatewayErrorCode::RecordingUnavailable, ErrorScope::Internal);
        let mut ids = BTreeSet::new();
        let mut previous = None;
        let mut size = 0usize;
        if confirmed_at_ms < 0 || items.is_empty() || items.len() > 256 {
            return Err(invalid());
        }
        for entry in &items {
            size = size
                .checked_add(entry.item.get().len())
                .ok_or_else(invalid)?;
            if size > MAX_MESSAGES_SUMMARY_BYTES
                || entry.output_index >= 256
                || previous.is_some_and(|index| index >= entry.output_index)
            {
                return Err(invalid());
            }
            previous = Some(entry.output_index);
            let value: Value = serde_json::from_str(entry.item.get()).map_err(|_| invalid())?;
            let object = value.as_object().ok_or_else(invalid)?;
            let id = value["id"].as_str().ok_or_else(invalid)?;
            if id.is_empty()
                || id.len() > 512
                || !id.bytes().all(|v| v.is_ascii_graphic())
                || !ids.insert(id.to_owned())
                || value["type"] != "reasoning"
                || value["status"] != "completed"
                || object.keys().any(|key| {
                    !matches!(
                        key.as_str(),
                        "id" | "type" | "status" | "summary" | "content"
                    )
                })
                || value
                    .get("content")
                    .is_some_and(|v| !v.as_array().is_some_and(Vec::is_empty))
            {
                return Err(invalid());
            }
            let parts = value["summary"].as_array().ok_or_else(invalid)?;
            if parts.len() > 64
                || parts.iter().any(|part| {
                    part["type"] != "summary_text"
                        || part.as_object().is_none_or(|v| v.len() != 2)
                        || part["text"].as_str().is_none_or(|v| v.contains('\0'))
                })
            {
                return Err(invalid());
            }
        }
        Ok(Self {
            source,
            response_id,
            completeness,
            confirmed_at_ms,
            items,
        })
    }
    /// Decoder transport provenance.
    #[must_use]
    pub const fn source(&self) -> MessagesSummarySource {
        self.source
    }
    /// Exact native response identity.
    #[must_use]
    pub const fn response_id(&self) -> &ResponseId {
        &self.response_id
    }
    /// Explicit completeness without inferring unread data or client delivery.
    #[must_use]
    pub const fn completeness(&self) -> MessagesSummaryCompleteness {
        self.completeness
    }
    /// Local time when the listed native snapshots were confirmed, before delivery cleanup.
    #[must_use]
    pub const fn confirmed_at_ms(&self) -> i64 {
        self.confirmed_at_ms
    }
    /// Ordered confirmed items, for encryption and authorized diagnostics only.
    #[must_use]
    pub fn items(&self) -> &[MessagesSummaryItem] {
        &self.items
    }
}
impl fmt::Debug for MessagesSummaryEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MessagesSummaryEvidence(<private>)")
    }
}
