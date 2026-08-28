//! The closed, value-free vocabulary shared by fixtures and gateway probes.

use serde::Deserialize;

/// Frozen references allowed in the P11 corpus.
///
/// Every variant names an external system that cannot be executed in this repository. Its
/// projection is therefore a recorded clean-room observation, not a computed one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Reference {
    /// CPA `v7.2.80`.
    #[serde(rename = "cpa-v7.2.80")]
    CpaV7_2_80,
    /// grok2api `v3.0.0` / `ec6cddca7`.
    #[serde(rename = "grok2api-v3.0.0-ec6cddca7")]
    Grok2ApiV3,
    /// Kiro-RS `c49c75e`.
    #[serde(rename = "kiro-rs-c49c75e")]
    KiroRs,
}

impl Reference {
    pub(crate) fn allows(self, subject: Subject) -> bool {
        matches!(
            (self, subject),
            (
                Self::CpaV7_2_80,
                Subject::CanonicalLifecycle | Subject::ConfigurationAuthority
            ) | (
                Self::Grok2ApiV3,
                Subject::ProviderPoolIsolation | Subject::WebToolDefault
            ) | (
                Self::KiroRs,
                Subject::EndpointPolicy | Subject::EventStreamIntegrity
            )
        )
    }
}

/// Semantic property families with no body-bearing representation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Subject {
    /// Protocol-neutral response lifecycle ordering.
    CanonicalLifecycle,
    /// Where configuration authority lives.
    ConfigurationAuthority,
    /// Isolation between the Grok Build and Grok Web credential pools.
    ProviderPoolIsolation,
    /// The default state of Grok Web Tool Emulation.
    WebToolDefault,
    /// Kiro CLI/IDE endpoint policy separation.
    EndpointPolicy,
    /// AWS `EventStream` CRC and chunk-boundary integrity.
    EventStreamIntegrity,
}

impl Subject {
    pub(crate) fn allows(self, marker: ProjectionMarker) -> bool {
        matches!(
            (self, marker),
            (
                Self::CanonicalLifecycle,
                ProjectionMarker::ResponseStart
                    | ProjectionMarker::TextDelta
                    | ProjectionMarker::ResponseEnd
            ) | (
                Self::ConfigurationAuthority,
                ProjectionMarker::FileWatcherAuthority | ProjectionMarker::VersionedSqliteSnapshot
            ) | (
                Self::ProviderPoolIsolation,
                ProjectionMarker::BuildWebPoolSeparation
                    | ProjectionMarker::BrowserEgressBoundConversation
            ) | (
                Self::WebToolDefault,
                ProjectionMarker::ToolEmulationDefaultEnabled
                    | ProjectionMarker::ToolEmulationDefaultDisabled
            ) | (Self::EndpointPolicy, ProjectionMarker::CliIdeEndpointPolicy)
                | (
                    Self::EventStreamIntegrity,
                    ProjectionMarker::EventStreamCrcValidation
                        | ProjectionMarker::ChunkInvariantCanonicalEvents
                )
        )
    }
}

/// Closed, value-free markers that may appear in a semantic projection.
///
/// The declaration order is the canonical projection order. A projection is accepted only when it
/// is strictly ascending in this order, so a computed and a recorded projection are comparable
/// without normalizing either side.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectionMarker {
    /// A canonical response opened.
    ResponseStart,
    /// Visible text was appended to the open message.
    TextDelta,
    /// The canonical response closed successfully.
    ResponseEnd,
    /// Configuration authority is a watched configuration file.
    FileWatcherAuthority,
    /// Configuration authority is an activated, versioned `SQLite` snapshot.
    VersionedSqliteSnapshot,
    /// The Build and Web credential pools hold no shared state.
    BuildWebPoolSeparation,
    /// A Web conversation is bound to exactly one browser egress session.
    BrowserEgressBoundConversation,
    /// Tool Emulation is enabled unless a caller disables it.
    ToolEmulationDefaultEnabled,
    /// Tool Emulation is disabled unless a caller enables it.
    ToolEmulationDefaultDisabled,
    /// CLI and IDE are distinct endpoint policies of one provider.
    CliIdeEndpointPolicy,
    /// `EventStream` frames are accepted only with a matching CRC.
    EventStreamCrcValidation,
    /// Canonical events do not depend on transport chunk boundaries.
    ChunkInvariantCanonicalEvents,
}

/// Whether a marker can be produced at all by driving this repository's code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GatewayObservability {
    /// A gateway probe decides this marker by executing real code.
    Observable,
    /// No gateway probe can ever emit this marker offline.
    ReferenceOnly,
}

impl ProjectionMarker {
    /// Returns whether a gateway probe can emit this marker.
    ///
    /// `FileWatcherAuthority` is the single `ReferenceOnly` marker: this repository has no
    /// configuration-file watcher, so no execution of its code can produce that authority. A
    /// fixture that expects the gateway to emit it is rejected rather than silently accepted.
    #[must_use]
    pub const fn gateway_observability(self) -> GatewayObservability {
        match self {
            Self::FileWatcherAuthority => GatewayObservability::ReferenceOnly,
            Self::ResponseStart
            | Self::TextDelta
            | Self::ResponseEnd
            | Self::VersionedSqliteSnapshot
            | Self::BuildWebPoolSeparation
            | Self::BrowserEgressBoundConversation
            | Self::ToolEmulationDefaultEnabled
            | Self::ToolEmulationDefaultDisabled
            | Self::CliIdeEndpointPolicy
            | Self::EventStreamCrcValidation
            | Self::ChunkInvariantCanonicalEvents => GatewayObservability::Observable,
        }
    }
}
