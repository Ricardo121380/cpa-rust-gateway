//! Token-count provenance and the preserved exact Provider capability value.

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
    use super::{ExactInputTokenCount, InputTokenCount};

    #[test]
    fn count_sources_remain_distinct_with_the_same_total() {
        let counts = [
            InputTokenCount::ProviderReported(17),
            InputTokenCount::ExactLocal(ExactInputTokenCount::new(17)),
            InputTokenCount::Estimated(17),
        ];
        assert!(counts.iter().all(|count| count.input_tokens() == 17));
        assert_ne!(counts[0], counts[1]);
        assert_ne!(counts[1], counts[2]);
        assert_ne!(counts[0].method(), counts[2].method());
        assert_eq!(
            InputTokenCount::from(ExactInputTokenCount::new(17)),
            counts[0]
        );
        assert!(!format!("{:?}", counts[2]).contains("17"));
    }

    #[test]
    fn retains_the_provider_reported_total_without_an_estimator() {
        let count = ExactInputTokenCount::new(17);

        assert_eq!(count.input_tokens(), 17);
        assert_eq!(format!("{count:?}"), "ExactInputTokenCount(<redacted>)");
    }
}

/// A pre-inference count with explicit provenance, never an inference Usage value.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum InputTokenCount {
    /// A value reported by the selected upstream's counting API; not a future usage promise.
    ProviderReported(u64),
    /// A tokenizer explicitly proven compatible with the selected model.
    ExactLocal(ExactInputTokenCount),
    /// A documented approximation using a tokenizer not proven exact for the selected model.
    Estimated(u64),
}

impl InputTokenCount {
    /// Returns the standard client-visible total independently of provenance.
    #[must_use]
    pub const fn input_tokens(self) -> u64 {
        match self {
            Self::ProviderReported(value) | Self::Estimated(value) => value,
            Self::ExactLocal(value) => value.input_tokens(),
        }
    }

    /// Returns a fixed, non-secret diagnostic method identifier.
    #[must_use]
    pub const fn method(self) -> &'static str {
        match self {
            Self::ProviderReported(_) => "provider-reported",
            Self::ExactLocal(_) => "proven-exact-local",
            Self::Estimated(_) => "estimated-cpa-o200k-base",
        }
    }
}

impl From<ExactInputTokenCount> for InputTokenCount {
    fn from(value: ExactInputTokenCount) -> Self {
        // Legacy exact capabilities attest the total, not local tokenizer provenance.
        Self::ProviderReported(value.input_tokens())
    }
}

impl fmt::Debug for InputTokenCount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InputTokenCount")
            .field("method", &self.method())
            .finish_non_exhaustive()
    }
}
