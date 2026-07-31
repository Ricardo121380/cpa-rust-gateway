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

use std::{collections::BTreeMap, error::Error, fmt};

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

    /// Returns every `adapter_id` allowed to serve this format, in stable declaration order.
    ///
    /// One wire format can be served by more than one implementation: Kiro speaks Anthropic's
    /// Messages format on the wire, but reaches it through its own credential families, endpoint
    /// hosts and `profileArn` injection rather than a generic Anthropic upstream. So the format
    /// alone cannot pick an implementation — the Endpoint's `adapter_id` selects one from this set.
    ///
    /// The set lives here so publish-time admission and composition-time binding cannot drift: a
    /// Config Version naming an `adapter_id` outside its format's set must be rejected before it is
    /// published, not discovered at the next process start when the composition refuses to build.
    #[must_use]
    pub const fn adapter_ids(self) -> &'static [&'static str] {
        match self {
            Self::OpenAiResponses => &["openai-compatible.responses"],
            Self::AnthropicMessages => &["anthropic-compatible.messages", "kiro.messages"],
        }
    }

    /// Returns whether one `adapter_id` may serve this format.
    #[must_use]
    pub fn serves(self, adapter_id: &str) -> bool {
        self.adapter_ids().contains(&adapter_id)
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

/// An immutable map from one `adapter_id` to its caller-chosen adapter binding.
///
/// The registry is generic because "adapter" means something different at each layer: a
/// per-Endpoint execution binding in a deployment binary, a stateless codec in a test. Fixing only
/// the key set keeps this contract free of `gateway-provider`, `gateway-upstream`, and every
/// protocol codec, so dispatching on a stored `api_format` plus `adapter_id` needs no new
/// dependency edge.
///
/// Keys are `adapter_id`, not [`ApiFormat`], because one wire format may be served by several
/// implementations (see [`ApiFormat::adapter_ids`]). Resolution therefore takes both: the format
/// says what the product can serve, the `adapter_id` says which implementation serves it here.
///
/// The map is deliberately partial. A product-supported format whose `adapter_id` *this* build
/// binds nothing for resolves to `None`, which its composition root must turn into a fail-closed
/// composition error rather than a silently skipped Endpoint. Whether a stored string is a
/// product-supported format at all is a separate, deployment-independent question answered by
/// [`ApiFormat::parse`].
#[derive(Clone)]
pub struct ApiFormatAdapterRegistry<A> {
    bindings: BTreeMap<&'static str, A>,
}

impl<A> ApiFormatAdapterRegistry<A> {
    /// Builds a registry from explicit `(format, adapter_id, adapter)` bindings.
    ///
    /// # Errors
    ///
    /// Returns [`ApiFormatAdapterRegistryError::UnsupportedAdapterId`] when an `adapter_id` is not
    /// one its format may be served by, so a composition cannot bind an implementation the Route
    /// Compiler would reject at publish time. Returns
    /// [`ApiFormatAdapterRegistryError::DuplicateAdapterId`] when one `adapter_id` is bound more
    /// than once, so an ambiguous composition fails at build time instead of resolving to whichever
    /// binding happened to be supplied last.
    pub fn try_new(
        bindings: impl IntoIterator<Item = (ApiFormat, &'static str, A)>,
    ) -> Result<Self, ApiFormatAdapterRegistryError> {
        let mut registry = Self {
            bindings: BTreeMap::new(),
        };
        for (format, adapter_id, adapter) in bindings {
            if !format.serves(adapter_id) {
                return Err(ApiFormatAdapterRegistryError::UnsupportedAdapterId(format));
            }
            if registry.bindings.insert(adapter_id, adapter).is_some() {
                return Err(ApiFormatAdapterRegistryError::DuplicateAdapterId(format));
            }
        }
        Ok(registry)
    }

    /// Returns the adapter bound to one `adapter_id`, or `None` when this build binds none.
    #[must_use]
    pub fn adapter(&self, adapter_id: &str) -> Option<&A> {
        self.bindings.get(adapter_id)
    }

    /// Resolves one stored `(api_format, adapter_id)` pair to its parsed format and bound adapter.
    ///
    /// Returns `None` for an unsupported format spelling, for an `adapter_id` the format may not be
    /// served by, and for a legal pair this build binds nothing for. A caller that must distinguish
    /// them parses with [`ApiFormat::parse`] and checks [`ApiFormat::serves`] first.
    #[must_use]
    pub fn resolve(&self, api_format: &str, adapter_id: &str) -> Option<(ApiFormat, &A)> {
        let format = ApiFormat::parse(api_format)?;
        if !format.serves(adapter_id) {
            return None;
        }
        Some((format, self.adapter(adapter_id)?))
    }

    /// Iterates every format this registry actually binds at least one adapter for, in stable
    /// declaration order.
    pub fn bound_formats(&self) -> impl Iterator<Item = ApiFormat> + '_ {
        ApiFormat::ALL.into_iter().filter(|format| {
            format
                .adapter_ids()
                .iter()
                .any(|adapter_id| self.adapter(adapter_id).is_some())
        })
    }
}

impl<A> fmt::Debug for ApiFormatAdapterRegistry<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Presence flags keyed by adapter_id, never an adapter value (BL-11). An adapter_id is a
        // fixed non-secret label from the table above, so printing which ones are bound reveals
        // nothing about an Endpoint, URL, or Credential.
        let mut debug = formatter.debug_struct("ApiFormatAdapterRegistry");
        for format in ApiFormat::ALL {
            for adapter_id in format.adapter_ids() {
                debug.field(adapter_id, &self.bindings.contains_key(adapter_id));
            }
        }
        debug.finish()
    }
}

/// Stable, secret-free failure raised while building an [`ApiFormatAdapterRegistry`].
///
/// The payload is one fixed format literal; it carries no Endpoint identity, URL, Credential, or
/// adapter value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiFormatAdapterRegistryError {
    /// The same `adapter_id` was bound more than once.
    DuplicateAdapterId(ApiFormat),
    /// An `adapter_id` was bound under a format it may not serve.
    UnsupportedAdapterId(ApiFormat),
}

impl fmt::Display for ApiFormatAdapterRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateAdapterId(format) => {
                write!(formatter, "duplicate adapter_id binding for {format}")
            }
            Self::UnsupportedAdapterId(format) => {
                write!(formatter, "adapter_id may not serve {format}")
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
    fn adapter_ids_are_per_format_sets_and_reject_foreign_labels() {
        assert_eq!(
            ApiFormat::OpenAiResponses.adapter_ids(),
            &["openai-compatible.responses"]
        );
        // One wire format, several implementations: Kiro speaks Anthropic Messages but reaches it
        // through its own credential families, endpoint hosts and profileArn injection.
        assert_eq!(
            ApiFormat::AnthropicMessages.adapter_ids(),
            &["anthropic-compatible.messages", "kiro.messages"]
        );
        assert!(ApiFormat::AnthropicMessages.serves("kiro.messages"));
        assert!(ApiFormat::AnthropicMessages.serves("anthropic-compatible.messages"));
        // A label belonging to another format, or to nothing, may not serve this one.
        assert!(!ApiFormat::AnthropicMessages.serves("openai-compatible.responses"));
        assert!(!ApiFormat::OpenAiResponses.serves("kiro.messages"));
        assert!(!ApiFormat::OpenAiResponses.serves("unknown.adapter"));
        assert!(!ApiFormat::AnthropicMessages.serves(""));
    }

    #[test]
    fn registry_binds_each_adapter_once_and_resolves_only_bound_pairs()
    -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            ApiFormatAdapterRegistry::try_new([
                (
                    ApiFormat::OpenAiResponses,
                    "openai-compatible.responses",
                    "first"
                ),
                (
                    ApiFormat::OpenAiResponses,
                    "openai-compatible.responses",
                    "second"
                ),
            ])
            .err(),
            Some(ApiFormatAdapterRegistryError::DuplicateAdapterId(
                ApiFormat::OpenAiResponses
            ))
        );

        // Binding an adapter under a format it may not serve fails at build time, so a composition
        // can never hold a pairing the Route Compiler would reject at publish time.
        assert_eq!(
            ApiFormatAdapterRegistry::try_new([(
                ApiFormat::OpenAiResponses,
                "kiro.messages",
                "wrong"
            )])
            .err(),
            Some(ApiFormatAdapterRegistryError::UnsupportedAdapterId(
                ApiFormat::OpenAiResponses
            ))
        );

        let partial = ApiFormatAdapterRegistry::try_new([(
            ApiFormat::OpenAiResponses,
            "openai-compatible.responses",
            "responses",
        )])?;
        assert_eq!(
            partial.adapter("openai-compatible.responses"),
            Some(&"responses")
        );
        assert_eq!(partial.adapter("anthropic-compatible.messages"), None);
        assert_eq!(
            partial.resolve("openai/responses", "openai-compatible.responses"),
            Some((ApiFormat::OpenAiResponses, &"responses"))
        );
        assert_eq!(
            partial.resolve("anthropic/messages", "anthropic-compatible.messages"),
            None
        );
        // A legal format paired with a foreign adapter_id resolves to None even when that
        // adapter_id is bound, so an Endpoint cannot borrow another format's implementation.
        assert_eq!(
            partial.resolve("anthropic/messages", "openai-compatible.responses"),
            None
        );
        assert_eq!(
            partial.resolve("openai_responses", "openai-compatible.responses"),
            None
        );
        assert_eq!(
            partial.bound_formats().collect::<Vec<_>>(),
            vec![ApiFormat::OpenAiResponses]
        );

        // Two adapters under one format both resolve, selected by adapter_id alone.
        let complete = ApiFormatAdapterRegistry::try_new([
            (
                ApiFormat::OpenAiResponses,
                "openai-compatible.responses",
                "responses",
            ),
            (
                ApiFormat::AnthropicMessages,
                "anthropic-compatible.messages",
                "messages",
            ),
            (ApiFormat::AnthropicMessages, "kiro.messages", "kiro"),
        ])?;
        assert_eq!(
            complete.resolve("anthropic/messages", "anthropic-compatible.messages"),
            Some((ApiFormat::AnthropicMessages, &"messages"))
        );
        assert_eq!(
            complete.resolve("anthropic/messages", "kiro.messages"),
            Some((ApiFormat::AnthropicMessages, &"kiro"))
        );
        assert_eq!(
            complete.bound_formats().collect::<Vec<_>>(),
            ApiFormat::ALL.to_vec()
        );
        Ok(())
    }
    #[test]
    fn registry_diagnostics_never_print_an_adapter_value() -> Result<(), Box<dyn std::error::Error>>
    {
        let registry = ApiFormatAdapterRegistry::try_new([(
            ApiFormat::OpenAiResponses,
            "openai-compatible.responses",
            "private-adapter",
        )])?;
        let diagnostic = format!("{registry:?}");

        assert!(!diagnostic.contains("private-adapter"));
        assert!(diagnostic.contains("ApiFormatAdapterRegistry"));
        assert!(diagnostic.contains("openai-compatible.responses: true"));
        assert!(diagnostic.contains("anthropic-compatible.messages: false"));
        assert!(diagnostic.contains("kiro.messages: false"));
        Ok(())
    }
}
