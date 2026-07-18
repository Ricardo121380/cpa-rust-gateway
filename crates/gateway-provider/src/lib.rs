//! Provider capability and execution contracts.

#![deny(unsafe_code)]

mod inference;
mod mock;

pub use inference::{CanonicalEventSource, InferenceAdapter, ProviderAdapter, ProviderFuture};
pub use mock::{DeterministicMockProvider, MockEmission, MockFixture};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-provider";
