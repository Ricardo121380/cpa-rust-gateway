//! Immutable model Catalog and Endpoint-capability evidence for control-plane compilation.
//!
//! P2-06 deliberately keeps these types storage-neutral and explicitly injected. P4 later owns
//! discovery, freshness persistence, and runtime diagnostics; neither concern is hidden here.

#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use gateway_core::EndpointId;

/// Stable component identifier used by architecture smoke tests.
pub const COMPONENT: &str = "gateway-catalog";

/// One semantic capability relevant to public-model Route compilation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticCapability {
    /// Tool call support.
    Tools,
    /// Multiple independent Tool calls in one request/response.
    ParallelTools,
    /// Explicit Thinking or Reasoning support.
    Reasoning,
    /// JSON Schema response or Tool support.
    JsonSchema,
    /// Vision input support.
    Vision,
    /// Streaming response support.
    Streaming,
}

impl SemanticCapability {
    /// Returns the fixed configuration key used in P2-06 capability JSON.
    #[must_use]
    pub const fn json_key(self) -> &'static str {
        match self {
            Self::Tools => "tools",
            Self::ParallelTools => "parallel_tools",
            Self::Reasoning => "reasoning",
            Self::JsonSchema => "json_schema",
            Self::Vision => "vision",
            Self::Streaming => "streaming",
        }
    }

    /// Parses one fixed P2-06 configuration key.
    #[must_use]
    pub fn from_json_key(value: &str) -> Option<Self> {
        match value {
            "tools" => Some(Self::Tools),
            "parallel_tools" => Some(Self::ParallelTools),
            "reasoning" => Some(Self::Reasoning),
            "json_schema" => Some(Self::JsonSchema),
            "vision" => Some(Self::Vision),
            "streaming" => Some(Self::Streaming),
            _ => None,
        }
    }
}

/// A validated set of Endpoint or Route semantic capabilities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilitySet {
    capabilities: BTreeSet<SemanticCapability>,
}

impl CapabilitySet {
    /// Builds a capability set and enforces its intra-set invariants.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::ParallelToolsRequiresTools`] when `parallel_tools` is present
    /// without `tools`.
    pub fn try_new(
        capabilities: impl IntoIterator<Item = SemanticCapability>,
    ) -> Result<Self, CatalogViewError> {
        let capabilities = capabilities.into_iter().collect();
        let set = Self { capabilities };
        set.validate()?;
        Ok(set)
    }

    /// Returns an empty capability set.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            capabilities: BTreeSet::new(),
        }
    }

    /// Returns whether this set includes one capability.
    #[must_use]
    pub fn supports(&self, capability: SemanticCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Returns whether every capability in `required` is available in this set.
    #[must_use]
    pub fn supports_all(&self, required: &Self) -> bool {
        required.capabilities.is_subset(&self.capabilities)
    }

    /// Produces a narrowed set without ever adding a capability.
    ///
    /// Removing `tools` also removes `parallel_tools`, preserving the mandatory implication.
    #[must_use]
    pub fn without(&self, removed: impl IntoIterator<Item = SemanticCapability>) -> Self {
        let mut capabilities = self.capabilities.clone();
        for capability in removed {
            capabilities.remove(&capability);
            if capability == SemanticCapability::Tools {
                capabilities.remove(&SemanticCapability::ParallelTools);
            }
        }
        Self { capabilities }
    }

    /// Iterates over supported capabilities in stable enum order.
    pub fn iter(&self) -> impl Iterator<Item = SemanticCapability> + '_ {
        self.capabilities.iter().copied()
    }

    fn validate(&self) -> Result<(), CatalogViewError> {
        if self.supports(SemanticCapability::ParallelTools)
            && !self.supports(SemanticCapability::Tools)
        {
            return Err(CatalogViewError::ParallelToolsRequiresTools);
        }
        Ok(())
    }
}

/// Freshness/provenance state of one `(Endpoint, upstream model)` Catalog record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogModelState {
    /// An explicit manual model allowlist entry.
    Manual,
    /// A current discovery-backed Catalog entry.
    Fresh,
    /// A retained last-success Catalog entry that has not expired.
    Stale,
    /// A Catalog entry no longer accepted without an explicit compiler exception.
    Expired,
}

impl CatalogModelState {
    /// Returns whether this state is hard-eligible without an explicit exception.
    #[must_use]
    pub const fn is_hard_eligible(self) -> bool {
        matches!(self, Self::Manual | Self::Fresh | Self::Stale)
    }
}

/// One storage-neutral model Catalog record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogModelEntry {
    /// Endpoint that exposes the model.
    pub endpoint_id: EndpointId,
    /// Exact non-empty upstream model string.
    pub upstream_model: String,
    /// Catalog freshness/provenance state.
    pub state: CatalogModelState,
}

impl CatalogModelEntry {
    /// Creates one model Catalog record.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::EmptyUpstreamModel`] when `upstream_model` is empty.
    pub fn try_new(
        endpoint_id: EndpointId,
        upstream_model: impl Into<String>,
        state: CatalogModelState,
    ) -> Result<Self, CatalogViewError> {
        let upstream_model = upstream_model.into();
        if upstream_model.is_empty() {
            return Err(CatalogViewError::EmptyUpstreamModel);
        }
        Ok(Self {
            endpoint_id,
            upstream_model,
            state,
        })
    }
}

/// Immutable lookup of model Catalog state by Endpoint and upstream model.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CatalogView {
    entries: BTreeMap<(EndpointId, String), CatalogModelState>,
}

impl CatalogView {
    /// Builds a Catalog lookup and rejects ambiguous duplicate records.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::DuplicateCatalogModel`] for duplicate endpoint/model pairs.
    pub fn try_new(
        entries: impl IntoIterator<Item = CatalogModelEntry>,
    ) -> Result<Self, CatalogViewError> {
        let mut view = Self::default();
        for entry in entries {
            let key = (entry.endpoint_id, entry.upstream_model);
            if view.entries.insert(key, entry.state).is_some() {
                return Err(CatalogViewError::DuplicateCatalogModel);
            }
        }
        Ok(view)
    }

    /// Returns the stored Catalog state for one exact Endpoint/model pair.
    #[must_use]
    pub fn model_state(
        &self,
        endpoint_id: &EndpointId,
        upstream_model: &str,
    ) -> Option<CatalogModelState> {
        self.entries
            .get(&(endpoint_id.clone(), upstream_model.to_owned()))
            .copied()
    }
}

/// One injected semantic-capability profile for a concrete Endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointCapabilityEntry {
    /// Endpoint whose profile is described.
    pub endpoint_id: EndpointId,
    /// All semantic capabilities supported by that Endpoint.
    pub capabilities: CapabilitySet,
}

/// Immutable lookup of semantic capabilities by Endpoint.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EndpointCapabilityView {
    entries: BTreeMap<EndpointId, CapabilitySet>,
}

impl EndpointCapabilityView {
    /// Builds an Endpoint capability lookup and rejects duplicate Endpoint profiles.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogViewError::DuplicateEndpointCapabilityProfile`] for duplicate Endpoint
    /// identities.
    pub fn try_new(
        entries: impl IntoIterator<Item = EndpointCapabilityEntry>,
    ) -> Result<Self, CatalogViewError> {
        let mut view = Self::default();
        for entry in entries {
            if view
                .entries
                .insert(entry.endpoint_id, entry.capabilities)
                .is_some()
            {
                return Err(CatalogViewError::DuplicateEndpointCapabilityProfile);
            }
        }
        Ok(view)
    }

    /// Returns the injected profile for one Endpoint.
    #[must_use]
    pub fn capabilities_for(&self, endpoint_id: &EndpointId) -> Option<&CapabilitySet> {
        self.entries.get(endpoint_id)
    }
}

/// Safe construction errors for injected Catalog/capability evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogViewError {
    /// A Catalog model string was empty.
    EmptyUpstreamModel,
    /// More than one Catalog record described the same Endpoint/model pair.
    DuplicateCatalogModel,
    /// More than one capability record described the same Endpoint.
    DuplicateEndpointCapabilityProfile,
    /// Parallel Tool support appeared without ordinary Tool support.
    ParallelToolsRequiresTools,
}

impl fmt::Display for CatalogViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyUpstreamModel => {
                formatter.write_str("Catalog upstream model must not be empty")
            }
            Self::DuplicateCatalogModel => {
                formatter.write_str("Catalog contains a duplicate Endpoint/model record")
            }
            Self::DuplicateEndpointCapabilityProfile => formatter
                .write_str("Endpoint capability view contains a duplicate Endpoint profile"),
            Self::ParallelToolsRequiresTools => {
                formatter.write_str("parallel Tool capability requires Tool capability")
            }
        }
    }
}

impl Error for CatalogViewError {}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::EndpointId;

    use super::{
        CapabilitySet, CatalogModelEntry, CatalogModelState, CatalogView, CatalogViewError,
        EndpointCapabilityEntry, EndpointCapabilityView, SemanticCapability,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn parallel_tools_requires_tools_and_narrowing_keeps_the_invariant() -> TestResult {
        assert_eq!(
            CapabilitySet::try_new([SemanticCapability::ParallelTools]),
            Err(CatalogViewError::ParallelToolsRequiresTools)
        );
        let capabilities = CapabilitySet::try_new([
            SemanticCapability::Tools,
            SemanticCapability::ParallelTools,
            SemanticCapability::Streaming,
        ])?;
        let narrowed = capabilities.without([SemanticCapability::Tools]);
        assert!(!narrowed.supports(SemanticCapability::Tools));
        assert!(!narrowed.supports(SemanticCapability::ParallelTools));
        assert!(narrowed.supports(SemanticCapability::Streaming));
        Ok(())
    }

    #[test]
    fn catalog_states_and_duplicate_records_are_explicit() -> TestResult {
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let catalog = CatalogView::try_new([CatalogModelEntry::try_new(
            endpoint.clone(),
            "upstream-model-a",
            CatalogModelState::Stale,
        )?])?;
        assert_eq!(
            catalog.model_state(&endpoint, "upstream-model-a"),
            Some(CatalogModelState::Stale)
        );
        assert!(CatalogModelState::Stale.is_hard_eligible());
        assert!(!CatalogModelState::Expired.is_hard_eligible());
        assert_eq!(
            CatalogView::try_new([
                CatalogModelEntry::try_new(
                    endpoint.clone(),
                    "upstream-model-a",
                    CatalogModelState::Fresh,
                )?,
                CatalogModelEntry::try_new(
                    endpoint,
                    "upstream-model-a",
                    CatalogModelState::Manual,
                )?,
            ]),
            Err(CatalogViewError::DuplicateCatalogModel)
        );
        Ok(())
    }

    #[test]
    fn endpoint_capability_view_rejects_ambiguous_profiles() -> TestResult {
        let endpoint = EndpointId::try_new("endpoint-a")?;
        let capabilities = CapabilitySet::try_new([SemanticCapability::Tools])?;
        assert_eq!(
            EndpointCapabilityView::try_new([
                EndpointCapabilityEntry {
                    endpoint_id: endpoint.clone(),
                    capabilities: capabilities.clone(),
                },
                EndpointCapabilityEntry {
                    endpoint_id: endpoint,
                    capabilities,
                },
            ]),
            Err(CatalogViewError::DuplicateEndpointCapabilityProfile)
        );
        Ok(())
    }
}
