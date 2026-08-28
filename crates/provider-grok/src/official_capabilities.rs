//! Explicit Grok Official capability declarations.
//!
//! Function Tools and explicit Reasoning have a lossless Canonical representation. Native web
//! search does not: the current Canonical Request and `OpenAI` Responses ingress deliberately reject
//! provider-owned search-tool shapes. P8-04 therefore records it as unavailable instead of making
//! a misleading provider capability claim.

use gateway_catalog::{CapabilitySet, CatalogViewError, SemanticCapability};

/// Native Official web-search status at the current Canonical boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokOfficialSearchCapability {
    /// No canonical request/output contract can carry native Search semantics yet.
    UnavailablePendingCanonicalContract,
}

/// Fixed semantic capability declaration for the implemented Official Responses codec.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokOfficialCapabilities;

impl GrokOfficialCapabilities {
    /// Returns capabilities that the native Official codec can represent without semantic loss.
    ///
    /// # Errors
    ///
    /// Returns `ParallelToolsRequiresTools` only if this fixed declaration is edited into an
    /// internally inconsistent set.
    pub fn semantic_capabilities() -> Result<CapabilitySet, CatalogViewError> {
        CapabilitySet::try_new([
            SemanticCapability::Tools,
            SemanticCapability::ParallelTools,
            SemanticCapability::Reasoning,
            SemanticCapability::JsonSchema,
            SemanticCapability::Streaming,
        ])
    }

    /// Returns the explicit native Search status rather than inferring support from `Tools`.
    #[must_use]
    pub const fn web_search() -> GrokOfficialSearchCapability {
        GrokOfficialSearchCapability::UnavailablePendingCanonicalContract
    }
}
