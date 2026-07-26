//! The closed Endpoint `api_format` vocabulary and its adapter registry.
//!
//! BL-06 binds one Endpoint to exactly one API Format, so two independent layers must agree on
//! one table: the management-time Route Compiler, which decides whether a stored format is a
//! format this product can ever serve, and a deployment composition root, which decides whether
//! *this* build holds an adapter for it. Both live here, below every protocol codec, Provider,
//! Router, and HTTP type, so neither layer imports the other in order to dispatch.
//!
//! Nothing in this module encodes a request, opens a socket, reads a Credential, or inspects a
//! canonical request. It names formats and holds caller-supplied adapter values.

use std::{error::Error, fmt};

/// One exact control-plane `api_format` value this product can serve.
///
/// The set is closed deliberately. An unknown stored format is never coerced into a neighbouring
/// protocol by string similarity or model-name inference, and never silently skipped: it is
/// rejected while a Config Version is compiled, before it can be published.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ApiFormat {
    /// `OpenAI`'s Responses API wire format, stored as `openai/responses`.
    OpenAiResponses,
    /// Anthropic's Messages API wire format, stored as `anthropic/messages`.
    AnthropicMessages,
}

impl ApiFormat {
    /// Every API Format this product can serve, in stable declaration order.
    pub const ALL: [Self; 2] = [Self::OpenAiResponses, Self::AnthropicMessages];

    /// Returns the exact stored `api_format` string for this format.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai/responses",
            Self::AnthropicMessages => "anthropic/messages",
        }
    }

    /// Returns the one `adapter_id` this format is served by.
    ///
    /// The pairing lives here so publish-time admission and composition-time binding cannot drift:
    /// a Config Version that names a mismatched pair must be rejected before it is published,
    /// not discovered at the next process start when the composition refuses to build.
    #[must_use]
    pub const fn adapter_id(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai-compatible.responses",
            Self::AnthropicMessages => "anthropic-compatible.messages",
        }
    }

    /// Parses one stored `api_format` value, rejecting every unsupported spelling.
    ///
    /// Only the exact strings behind [`ApiFormat::ALL`] are accepted. `openai_responses`,
    /// `openai/chat_completions`, and any future owner-specific format stay unknown here until
    /// their owning protocol boundary supplies an explicit mapping.
    #[must_use]
    pub fn parse(api_format: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|format| format.as_str() == api_format)
    }
}

impl fmt::Display for ApiFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// An immutable map from [`ApiFormat`] to one caller-chosen adapter binding.
///
/// The registry is generic because "adapter" means something different at each layer: a
/// per-Endpoint execution binding in a deployment binary, a stateless codec in a test. Fixing only
/// the key set keeps this contract free of `gateway-provider`, `gateway-upstream`, and every
/// protocol codec, so dispatching on a stored `api_format` needs no new dependency edge.
///
/// The map is deliberately partial. A product-supported format that *this* build binds no adapter
/// for resolves to `None`, which its composition root must turn into a fail-closed composition
/// error rather than a silently skipped Endpoint. Whether a stored string is a product-supported
/// format at all is a separate, deployment-independent question answered by [`ApiFormat::parse`].
#[derive(Clone)]
pub struct ApiFormatAdapterRegistry<A> {
    openai_responses: Option<A>,
    anthropic_messages: Option<A>,
}

impl<A> ApiFormatAdapterRegistry<A> {
    /// Builds a registry from explicit `(format, adapter)` bindings.
    ///
    /// # Errors
    ///
    /// Returns [`ApiFormatAdapterRegistryError::DuplicateApiFormat`] when one format is bound more
    /// than once, so an ambiguous composition fails at build time instead of resolving to whichever
    /// binding happened to be supplied last.
    pub fn try_new(
        bindings: impl IntoIterator<Item = (ApiFormat, A)>,
    ) -> Result<Self, ApiFormatAdapterRegistryError> {
        let mut registry = Self {
            openai_responses: None,
            anthropic_messages: None,
        };
        for (format, adapter) in bindings {
            let slot = match format {
                ApiFormat::OpenAiResponses => &mut registry.openai_responses,
                ApiFormat::AnthropicMessages => &mut registry.anthropic_messages,
            };
            if slot.is_some() {
                return Err(ApiFormatAdapterRegistryError::DuplicateApiFormat(format));
            }
            *slot = Some(adapter);
        }
        Ok(registry)
    }

    /// Returns the adapter bound to one format, or `None` when this build binds none.
    #[must_use]
    pub fn adapter(&self, format: ApiFormat) -> Option<&A> {
        match format {
            ApiFormat::OpenAiResponses => self.openai_responses.as_ref(),
            ApiFormat::AnthropicMessages => self.anthropic_messages.as_ref(),
        }
    }

    /// Resolves one stored `api_format` string to its parsed format and bound adapter.
    ///
    /// An unsupported spelling and a supported format without a binding are both `None`; a caller
    /// that must distinguish them parses with [`ApiFormat::parse`] first.
    #[must_use]
    pub fn resolve(&self, api_format: &str) -> Option<(ApiFormat, &A)> {
        let format = ApiFormat::parse(api_format)?;
        Some((format, self.adapter(format)?))
    }

    /// Iterates every format this registry actually binds, in stable declaration order.
    pub fn bound_formats(&self) -> impl Iterator<Item = ApiFormat> + '_ {
        ApiFormat::ALL
            .into_iter()
            .filter(|format| self.adapter(*format).is_some())
    }
}

impl<A> fmt::Debug for ApiFormatAdapterRegistry<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiFormatAdapterRegistry")
            .field("openai_responses", &self.openai_responses.is_some())
            .field("anthropic_messages", &self.anthropic_messages.is_some())
            .finish()
    }
}

/// Stable, secret-free failure raised while building an [`ApiFormatAdapterRegistry`].
///
/// The payload is one fixed format literal; it carries no Endpoint identity, URL, Credential, or
/// adapter value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiFormatAdapterRegistryError {
    /// The same API Format was bound more than once.
    DuplicateApiFormat(ApiFormat),
}

impl fmt::Display for ApiFormatAdapterRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateApiFormat(format) => {
                write!(formatter, "duplicate api_format adapter binding: {format}")
            }
        }
    }
}

impl Error for ApiFormatAdapterRegistryError {}
#[cfg(test)]
mod tests {
    use super::{ApiFormat, ApiFormatAdapterRegistry, ApiFormatAdapterRegistryError};

    #[test]
    fn api_format_strings_are_exact_and_reject_unsupported_spellings() {
        assert_eq!(ApiFormat::OpenAiResponses.as_str(), "openai/responses");
        assert_eq!(ApiFormat::AnthropicMessages.as_str(), "anthropic/messages");
        for format in ApiFormat::ALL {
            assert_eq!(ApiFormat::parse(format.as_str()), Some(format));
            assert_eq!(format.to_string(), format.as_str());
        }
        for unsupported in [
            "openai_responses",
            "openai/chat_completions",
            "Anthropic/Messages",
            " openai/responses",
            "",
        ] {
            assert_eq!(ApiFormat::parse(unsupported), None);
        }
    }
    #[test]
    fn registry_binds_each_format_once_and_resolves_only_bound_formats()
    -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            ApiFormatAdapterRegistry::try_new([
                (ApiFormat::OpenAiResponses, "first"),
                (ApiFormat::OpenAiResponses, "second"),
            ])
            .err(),
            Some(ApiFormatAdapterRegistryError::DuplicateApiFormat(
                ApiFormat::OpenAiResponses
            ))
        );

        let partial =
            ApiFormatAdapterRegistry::try_new([(ApiFormat::OpenAiResponses, "responses")])?;
        assert_eq!(
            partial.adapter(ApiFormat::OpenAiResponses),
            Some(&"responses")
        );
        assert_eq!(partial.adapter(ApiFormat::AnthropicMessages), None);
        assert_eq!(
            partial.resolve("openai/responses"),
            Some((ApiFormat::OpenAiResponses, &"responses"))
        );
        assert_eq!(partial.resolve("anthropic/messages"), None);
        assert_eq!(partial.resolve("openai_responses"), None);
        assert_eq!(
            partial.bound_formats().collect::<Vec<_>>(),
            vec![ApiFormat::OpenAiResponses]
        );

        let complete = ApiFormatAdapterRegistry::try_new([
            (ApiFormat::OpenAiResponses, "responses"),
            (ApiFormat::AnthropicMessages, "messages"),
        ])?;
        assert_eq!(
            complete.bound_formats().collect::<Vec<_>>(),
            ApiFormat::ALL.to_vec()
        );
        Ok(())
    }
    #[test]
    fn registry_diagnostics_never_print_an_adapter_value() -> Result<(), Box<dyn std::error::Error>>
    {
        let registry =
            ApiFormatAdapterRegistry::try_new([(ApiFormat::OpenAiResponses, "private-adapter")])?;
        let diagnostic = format!("{registry:?}");

        assert!(!diagnostic.contains("private-adapter"));
        assert!(diagnostic.contains("ApiFormatAdapterRegistry"));
        assert!(diagnostic.contains("openai_responses: true"));
        assert!(diagnostic.contains("anthropic_messages: false"));
        Ok(())
    }
}
