//! Generic `Anthropic`-compatible upstream provider boundary.

#![deny(unsafe_code)]

mod anthropic_messages;

pub use anthropic_messages::{
    ANTHROPIC_MESSAGES_INFERENCE_PATH, ANTHROPIC_VERSION, AnthropicMessagesApiKey,
    AnthropicMessagesEndpoint, AnthropicMessagesOutboundRequest, AnthropicMessagesRequestBuilder,
};
// The wire codec itself belongs to the protocol crate, symmetrically with the OpenAI Responses
// side. This crate is the format's provider boundary, so it re-exports the response direction a
// composition needs: a composition root selects a Provider, and must not have to name the codec
// crate to consume what that Provider returns.
pub use protocol_anthropic::{
    AnthropicMessagesSseDecoder, ResponseMode as AnthropicResponseMode, decode_upstream_response,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "provider-anthropic-compatible";
