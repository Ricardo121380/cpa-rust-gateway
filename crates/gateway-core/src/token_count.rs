//! Exact token-count values reported by a declared Provider capability.

use std::fmt;

/// An exact input-token count returned by a Provider or explicitly proven local tokenizer.
///
/// This type intentionally does not expose an estimator or a conversion from request text. Its
/// constructor is the point at which an implementing capability attests that the value was counted
/// by the selected model's compatible tokenizer.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExactInputTokenCount(u64);

impl ExactInputTokenCount {
    /// Marks one Provider-reported input-token count as exact for the selected route.
    #[must_use]
    pub const fn new(input_tokens: u64) -> Self {
        Self(input_tokens)
    }

    /// Returns the client-visible exact input-token total.
    #[must_use]
    pub const fn input_tokens(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for ExactInputTokenCount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ExactInputTokenCount(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::ExactInputTokenCount;

    #[test]
    fn retains_the_provider_reported_total_without_an_estimator() {
        let count = ExactInputTokenCount::new(17);

        assert_eq!(count.input_tokens(), 17);
        assert_eq!(format!("{count:?}"), "ExactInputTokenCount(<redacted>)");
    }
}
