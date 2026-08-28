//! Open-ended thinking configuration retained independently from provider parameters.

use std::{error::Error, fmt};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::RawExtensions;

/// Error returned when a thinking effort has no bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidThinkingEffort {
    /// The effort string was empty.
    Empty,
}

impl fmt::Display for InvalidThinkingEffort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("thinking effort must not be empty"),
        }
    }
}

impl Error for InvalidThinkingEffort {}

/// An open-ended client-requested thinking effort.
///
/// The core intentionally accepts effort labels outside a fixed vendor enum. Provider adapters
/// decide whether and how a retained label maps to their own capabilities.
#[derive(Clone, Eq, PartialEq)]
pub struct ThinkingEffort(String);

impl fmt::Debug for ThinkingEffort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ThinkingEffort(<redacted>)")
    }
}

impl ThinkingEffort {
    /// Creates a non-empty effort label while preserving its supplied representation.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidThinkingEffort::Empty`] when `value` is empty.
    pub fn try_new(value: impl Into<String>) -> Result<Self, InvalidThinkingEffort> {
        let value = value.into();
        if value.is_empty() {
            return Err(InvalidThinkingEffort::Empty);
        }

        Ok(Self(value))
    }

    /// Returns the client-requested effort label without interpreting it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ThinkingEffort {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<String> for ThinkingEffort {
    type Error = InvalidThinkingEffort;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for ThinkingEffort {
    type Error = InvalidThinkingEffort;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl Serialize for ThinkingEffort {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ThinkingEffort {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(de::Error::custom)
    }
}

/// Requested thinking behavior without provider-specific token budgets or mode switches.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thinking {
    /// Open-ended effort label supplied explicitly by the client.
    ///
    /// The enclosing optional `CanonicalRequest::thinking` distinguishes no explicit Thinking
    /// request from a request that chooses an effort level.
    pub effort: ThinkingEffort,
    /// Provider- or protocol-specific data retained without core interpretation.
    #[serde(default)]
    pub extensions: RawExtensions,
}

impl fmt::Debug for Thinking {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Thinking")
            .field("effort", &"<redacted>")
            .field("extensions", &self.extensions)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{InvalidThinkingEffort, Thinking, ThinkingEffort};
    use crate::RawExtensions;

    #[test]
    fn thinking_effort_preserves_an_open_label() -> Result<(), InvalidThinkingEffort> {
        let effort = ThinkingEffort::try_new("future-provider-level")?;

        assert_eq!(effort.as_str(), "future-provider-level");
        Ok(())
    }

    #[test]
    fn thinking_effort_rejects_an_empty_label() {
        assert_eq!(
            ThinkingEffort::try_new(""),
            Err(InvalidThinkingEffort::Empty)
        );
    }

    #[test]
    fn thinking_effort_rejects_empty_json() {
        let decoded = serde_json::from_str::<ThinkingEffort>("\"\"");

        assert!(decoded.is_err());
    }

    #[test]
    fn thinking_requires_an_explicit_effort() {
        let decoded = serde_json::from_str::<Thinking>("{\"extensions\":{}}");

        assert!(decoded.is_err());
    }

    #[test]
    fn thinking_debug_redacts_effort() -> Result<(), InvalidThinkingEffort> {
        let thinking = Thinking {
            effort: ThinkingEffort::try_new("internal-investigation")?,
            extensions: RawExtensions::default(),
        };

        let diagnostic = format!("{thinking:?}{:?}", thinking.effort);

        assert!(!diagnostic.contains("internal-investigation"));
        Ok(())
    }
}
