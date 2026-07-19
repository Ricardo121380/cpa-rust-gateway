//! Generic `OpenAI`-compatible upstream provider boundary.

#![deny(unsafe_code)]

mod openai_responses;

pub use openai_responses::{
    OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesOutboundRequest,
    OpenAiResponsesRequestBuilder,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "provider-openai-compatible";
