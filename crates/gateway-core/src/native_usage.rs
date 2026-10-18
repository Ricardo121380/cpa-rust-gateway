//! Sensitive native Usage companion, deliberately outside ordinary lifecycle events.
use crate::RawJson;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Maximum received Usage object retained in one bounded queue slot.
pub const MAX_NATIVE_USAGE_BYTES: usize = 64 * 1024;

/// Exact decoded source of a complete received Build Usage object.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeUsageSource {
    /// Usage in an already received JSON Responses document.
    BuildJson,
    /// Usage in an already received Responses SSE terminal event.
    BuildSseTerminal,
}

/// Private evidence; values are exposed only to encryption and authorized diagnostic readers.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeUsageEvidence {
    source: NativeUsageSource,
    raw_json: String,
}
impl NativeUsageEvidence {
    /// Retains one bounded, strictly decoded object without numeric normalization.
    ///
    /// # Errors
    /// Returns a value-free error for invalid shape or size. The provider must already have
    /// rejected duplicate keys and validated known counters before calling this boundary.
    pub fn try_new(
        source: NativeUsageSource,
        raw_json: String,
    ) -> Result<Self, crate::GatewayError> {
        let invalid = || {
            crate::GatewayError::new(
                crate::GatewayErrorCode::RecordingUnavailable,
                crate::ErrorScope::Internal,
            )
        };
        if raw_json.len() > MAX_NATIVE_USAGE_BYTES || !raw_json.trim_start().starts_with('{') {
            return Err(invalid());
        }
        RawJson::from_json_string(raw_json.clone()).map_err(|_| invalid())?;
        Ok(Self { source, raw_json })
    }
    /// Returns the received Usage slice, including exact numeric lexemes.
    #[must_use]
    pub fn raw_json(&self) -> &str {
        &self.raw_json
    }
    /// Returns its decoder source; this is not a Provider billing assertion.
    #[must_use]
    pub const fn source(&self) -> NativeUsageSource {
        self.source
    }
}
impl fmt::Debug for NativeUsageEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeUsageEvidence")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}
