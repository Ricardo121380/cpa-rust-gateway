//! Pure `Anthropic` Messages request, response, and Server-Sent Events codec.
//!
//! Both protocol directions live here. `decode_request`/`encode_response` serve a client that
//! speaks Anthropic Messages to this gateway; `encode_upstream_request`/`decode_upstream_response`
//! serve a gateway Provider that speaks Anthropic Messages to an upstream. HTTP authentication,
//! routing, Provider execution, bounded delivery, and client-write tracking remain outside this
//! crate.

#![deny(unsafe_code)]

mod json;
mod request;
mod response;
mod upstream_request;
mod upstream_response;

pub use request::{
    DecodedCountTokensRequest, DecodedMessagesRequest, ResponseMode, decode_count_tokens_request,
    decode_request,
};
pub use response::{
    AnthropicMessagesSseEncoder, AnthropicResponseMetadata, SseFrame, encode_count_tokens,
    encode_error, encode_response,
};
pub use upstream_request::encode_upstream_request;
pub use upstream_response::{AnthropicMessagesSseDecoder, decode_upstream_response};

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "protocol-anthropic";
