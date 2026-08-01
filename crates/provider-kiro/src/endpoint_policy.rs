//! Fixed Kiro IDE/CLI endpoint policy.
//!
//! This module is deliberately a pure request-head builder. It validates a caller-supplied API
//! Region and derives the only supported Kiro hosts, paths, headers, origin, and future Thinking
//! placement. It neither reads local configuration nor performs I/O.

use std::{collections::BTreeMap, error::Error, fmt};

use url::Url;

use crate::credential::KiroCredentialKind;

const MAX_REGION_BYTES: usize = 128;
const IDE_PATH: &str = "/generateAssistantResponse";
const CLI_PATH: &str = "/";
const CLI_TARGET: &str = "AmazonCodeWhispererStreamingService.GenerateAssistantResponse";
/// Both Kiro endpoints take a plain JSON body; see [`KiroEndpointPolicy::request_headers`] for the
/// measured evidence that the CLI endpoint rejects an `x-amz-json` content type outright.
const CONTENT_TYPE: &str = "application/json";

/// The two protocol-distinct Kiro request endpoints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroEndpointKind {
    /// Amazon Q IDE/Editor request surface.
    Ide,
    /// Kiro CLI runtime request surface.
    Cli,
}

/// A bounded, host-safe Kiro API Region.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct KiroApiRegion(String);

impl KiroApiRegion {
    /// Validates one API Region before it can contribute to a derived endpoint hostname.
    ///
    /// # Errors
    ///
    /// Returns a safe classification for empty, oversized, or non-AWS-Region-like values.
    pub fn try_new(value: impl Into<String>) -> Result<Self, KiroEndpointPolicyError> {
        let value = value.into();
        if !is_valid_region(&value) {
            return Err(KiroEndpointPolicyError::InvalidRegion);
        }
        Ok(Self(value))
    }

    /// Returns the validated Region for safe hostname derivation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for KiroApiRegion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("KiroApiRegion")
            .field(&self.0)
            .finish()
    }
}

/// The endpoint-specific request origin recognized by Kiro.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroRequestOrigin {
    /// IDE/editor invocation.
    AiEditor,
    /// Kiro CLI invocation.
    KiroCli,
}

impl KiroRequestOrigin {
    /// Returns Kiro's exact wire value.
    #[must_use]
    pub const fn as_header_value(self) -> &'static str {
        match self {
            Self::AiEditor => "AI_EDITOR",
            Self::KiroCli => "KIRO_CLI",
        }
    }
}

/// Where a later request transformer may place Thinking controls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroThinkingPlacement {
    /// IDE supports the Kiro `thinking` wrapper only when a model supports it.
    IdeThinkingWrapper,
    /// CLI preserves only native `output_config.effort`.
    CliOutputConfigEffort,
}

/// An immutable, credential-free Kiro endpoint policy.
#[derive(Clone)]
pub struct KiroEndpointPolicy {
    kind: KiroEndpointKind,
    api_region: KiroApiRegion,
    url: Url,
}

impl KiroEndpointPolicy {
    /// Derives one fixed IDE or CLI endpoint from a validated API Region.
    ///
    /// # Errors
    ///
    /// Returns a safe error when the Region or a fixed derived URL is invalid.
    pub fn try_new(
        kind: KiroEndpointKind,
        api_region: KiroApiRegion,
    ) -> Result<Self, KiroEndpointPolicyError> {
        let (host, path) = match kind {
            KiroEndpointKind::Ide => (format!("q.{}.amazonaws.com", api_region.as_str()), IDE_PATH),
            KiroEndpointKind::Cli => (
                format!("runtime.{}.kiro.dev", api_region.as_str()),
                CLI_PATH,
            ),
        };
        let url = Url::parse(&format!("https://{host}{path}"))
            .map_err(|_| KiroEndpointPolicyError::InvalidDerivedUrl)?;
        if url.scheme() != "https" || url.host_str() != Some(host.as_str()) || url.path() != path {
            return Err(KiroEndpointPolicyError::InvalidDerivedUrl);
        }
        Ok(Self {
            kind,
            api_region,
            url,
        })
    }

    /// Returns the selected protocol-distinct endpoint kind.
    #[must_use]
    pub const fn kind(&self) -> KiroEndpointKind {
        self.kind
    }

    /// Returns the API Region that produced this exact host.
    #[must_use]
    pub const fn api_region(&self) -> &KiroApiRegion {
        &self.api_region
    }

    /// Returns the exact fixed HTTPS endpoint URL.
    #[must_use]
    pub const fn url(&self) -> &Url {
        &self.url
    }

    /// Returns the endpoint-specific Kiro origin.
    #[must_use]
    pub const fn origin(&self) -> KiroRequestOrigin {
        match self.kind {
            KiroEndpointKind::Ide => KiroRequestOrigin::AiEditor,
            KiroEndpointKind::Cli => KiroRequestOrigin::KiroCli,
        }
    }

    /// Returns the later request-transform placement for Thinking fields.
    #[must_use]
    pub const fn thinking_placement(&self) -> KiroThinkingPlacement {
        match self.kind {
            KiroEndpointKind::Ide => KiroThinkingPlacement::IdeThinkingWrapper,
            KiroEndpointKind::Cli => KiroThinkingPlacement::CliOutputConfigEffort,
        }
    }

    /// Produces the complete fixed, non-secret request-header policy for one credential family.
    ///
    /// OAuth/API-key values themselves remain in the P7-01 credential boundary and are injected by
    /// a later request builder. This method sets only non-secret endpoint semantics.
    ///
    /// Both endpoint kinds send `application/json`. The CLI endpoint additionally carries the
    /// `x-amz-target` operation label, which is how an AWS JSON-RPC surface selects its operation --
    /// but the request body is still plain JSON, not an `x-amz-json` framing. Measured against the
    /// live CLI endpoint, holding every other header fixed and varying only this one:
    /// `application/json` returns `200`, `application/x-amz-json-1.0` returns `403`, and
    /// `application/x-amz-json-1.1` returns `404`. The reference implementation agrees -- it sends
    /// `application/x-amz-json-1.0` only for the non-streaming `ListAvailableProfiles` control call,
    /// never for `GenerateAssistantResponse`.
    #[must_use]
    pub fn request_headers(&self, credential_kind: KiroCredentialKind) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::from([
            (
                "origin".to_owned(),
                self.origin().as_header_value().to_owned(),
            ),
            ("content-type".to_owned(), CONTENT_TYPE.to_owned()),
        ]);
        if self.kind == KiroEndpointKind::Cli {
            headers.insert("x-amz-target".to_owned(), CLI_TARGET.to_owned());
        }
        if credential_kind == KiroCredentialKind::ApiKey {
            headers.insert("tokentype".to_owned(), "API_KEY".to_owned());
        }
        headers
    }
}

impl fmt::Debug for KiroEndpointPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroEndpointPolicy")
            .field("kind", &self.kind)
            .field("api_region", &self.api_region)
            .field("url", &self.url)
            .field("origin", &self.origin())
            .field("thinking_placement", &self.thinking_placement())
            .finish()
    }
}

/// Safe endpoint-policy validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroEndpointPolicyError {
    /// The API Region could not safely contribute to a Kiro hostname.
    InvalidRegion,
    /// A fixed endpoint failed structural HTTPS/host/path verification.
    InvalidDerivedUrl,
}

impl fmt::Display for KiroEndpointPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRegion => "Kiro API Region is invalid",
            Self::InvalidDerivedUrl => "Kiro derived endpoint is invalid",
        })
    }
}

impl Error for KiroEndpointPolicyError {}

fn is_valid_region(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_REGION_BYTES {
        return false;
    }
    let parts: Vec<_> = value.split('-').collect();
    if parts.len() < 3 {
        return false;
    }
    let (name_parts, number) = parts.split_at(parts.len() - 1);
    name_parts
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_lowercase()))
        && !number[0].is_empty()
        && number[0].bytes().all(|byte| byte.is_ascii_digit())
}
