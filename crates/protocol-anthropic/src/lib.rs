//! Pure `Anthropic` Messages request, response, and Server-Sent Events codec.
//!
//! HTTP authentication, routing, Provider execution, bounded delivery, and client-write tracking
//! remain outside this crate.

#![deny(unsafe_code)]

mod json;
mod request;
mod response;

pub use request::{
    DecodedCountTokensRequest, DecodedMessagesRequest, ResponseMode, decode_count_tokens_request,
    decode_request,
};
pub use response::{
    AnthropicMessagesSseEncoder, AnthropicResponseMetadata, SseFrame, encode_count_tokens,
    encode_error, encode_response,
};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "protocol-anthropic";
