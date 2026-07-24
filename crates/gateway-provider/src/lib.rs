//! Provider capability and execution contracts.

#![deny(unsafe_code)]

mod inference;
mod mock;
mod token_count;

pub use inference::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};
pub use mock::{DeterministicMockProvider, MockEmission, MockFixture};
pub use token_count::{ExactTokenCountAdapter, TokenCountCapability, token_count_unsupported};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-provider";
