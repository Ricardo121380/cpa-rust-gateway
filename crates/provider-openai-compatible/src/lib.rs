//! Generic `OpenAI`-compatible upstream provider boundary.

#![deny(unsafe_code)]

mod openai_chat_completions;
mod openai_responses;

pub use openai_chat_completions::{
    OpenAiChatCompletionsApiKey, OpenAiChatCompletionsEndpoint,
    OpenAiChatCompletionsOutboundRequest, OpenAiChatCompletionsRequestBuilder,
};
pub use openai_responses::{
    OpenAiResponsesApiKey, OpenAiResponsesEndpoint, OpenAiResponsesOutboundRequest,
    OpenAiResponsesRequestBuilder,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "provider-openai-compatible";
