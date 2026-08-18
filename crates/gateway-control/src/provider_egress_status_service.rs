//! Provider-specific egress, session, and clearance runtime projections.
//!
//! This module is a provider-neutral, read-only management seam. A serving composition supplies
//! one already-retained, atomic runtime snapshot; this module validates its safe shape and only
//! filters, orders, and paginates that immutable value. It never reads a Credential secret,
//! contacts a Provider, resolves DNS, opens a proxy, or starts a recovery/refresh operation.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    fmt::Write as _,
    str::FromStr,
};

use gateway_core::{CredentialId, EndpointId, ProviderId, UpstreamId};
use gateway_store::control_plane::ConfigVersionId;
use sha2::{Digest, Sha256};

use crate::management_mutation_service::ConfigRevision;

/// Default number of Provider-specific runtime rows returned in one page.
pub const DEFAULT_PROVIDER_EGRESS_STATUS_LIMIT: usize = 50;
/// Maximum number of Provider-specific runtime rows returned in one page.
pub const MAX_PROVIDER_EGRESS_STATUS_LIMIT: usize = 100;
/// Maximum URL-safe Base64 cursor length admitted by the management HTTP contract.
pub const MAX_PROVIDER_EGRESS_STATUS_CURSOR_LENGTH: usize = 4_096;
/// Maximum rows admitted to one atomic three-domain runtime snapshot.
pub const MAX_PROVIDER_EGRESS_STATUS_SNAPSHOT_ITEMS: usize = 49_152;
/// Largest integer represented exactly by JavaScript and therefore admitted on the wire.
pub const MAX_PROVIDER_EGRESS_STATUS_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

const MAX_OPAQUE_ID_BYTES: usize = 128;
const MAX_SNAPSHOT_ID_BYTES: usize = 128;
const MAX_CURSOR_KEY_BYTES: usize = 2_048;
const FILTER_FINGERPRINT_HEX_CHARS: usize = 64;

/// Closed runtime-state domain. These domains are intentionally not collapsed into one Health.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderEgressStatusDomain {
    /// Direct or named outbound transport availability.
    Egress,
    /// Credential-revision-bound Provider session lineage.
    Session,
    /// Session-and-egress-bound browser clearance lineage.
    Clearance,
}

impl ProviderEgressStatusDomain {
    /// Stable management wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Egress => "egress",
            Self::Session => "session",
            Self::Clearance => "clearance",
        }
    }
}

impl FromStr for ProviderEgressStatusDomain {
    type Err = ProviderEgressStatusError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "egress" => Ok(Self::Egress),
            "session" => Ok(Self::Session),
            "clearance" => Ok(Self::Clearance),
            _ => Err(ProviderEgressStatusError::InvalidQuery),
        }
    }
}

/// Closed Provider/Channel behavior family exposed by the management projection.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderEgressStatusChannelKind {
    /// Arbitrary OpenAI/Anthropic-compatible `base_url + credential` channel.
    GenericCompatible,
    /// Native Grok Build channel.
    GrokBuild,
    /// Native Grok Console channel.
    GrokConsole,
    /// Native Grok Web channel.
    GrokWeb,
    /// Provider official API-key endpoint.
    OfficialApi,
    /// Official Codex/ChatGPT account channel, independent of import envelope.
    CodexChatGpt,
    /// Native Kiro channel.
    Kiro,
    /// Claude-compatible endpoint without Grok browser behavior.
    ClaudeCompatible,
    /// Conservative compatible behavior for another explicitly declared adapter.
    OtherCompatible,
}

impl ProviderEgressStatusChannelKind {
    /// Stable management wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GenericCompatible => "generic_compatible",
            Self::GrokBuild => "grok_build",
            Self::GrokConsole => "grok_console",
            Self::GrokWeb => "grok_web",
            Self::OfficialApi => "official_api",
            Self::CodexChatGpt => "codex_chatgpt",
            Self::Kiro => "kiro",
            Self::ClaudeCompatible => "claude_compatible",
            Self::OtherCompatible => "other_compatible",
        }
    }

    const fn supports_provider_session(self) -> bool {
        matches!(self, Self::GrokConsole | Self::GrokWeb)
    }

    const fn supports_clearance(self) -> bool {
        matches!(self, Self::GrokWeb)
    }

    const fn requires_named_target(self) -> bool {
        matches!(self, Self::GrokWeb)
    }
}

impl FromStr for ProviderEgressStatusChannelKind {
    type Err = ProviderEgressStatusError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "generic_compatible" => Ok(Self::GenericCompatible),
            "grok_build" => Ok(Self::GrokBuild),
            "grok_console" => Ok(Self::GrokConsole),
            "grok_web" => Ok(Self::GrokWeb),
            "official_api" => Ok(Self::OfficialApi),
            "codex_chatgpt" => Ok(Self::CodexChatGpt),
            "kiro" => Ok(Self::Kiro),
            "claude_compatible" => Ok(Self::ClaudeCompatible),
            "other_compatible" => Ok(Self::OtherCompatible),
            _ => Err(ProviderEgressStatusError::InvalidQuery),
        }
    }
}

/// Closed union of every public state, with domain compatibility checked explicitly.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderEgressStatusState {
    /// Egress may be selected by its exact owning channel.
    Available,
    /// Egress is cooling until its exclusive deadline.
    CoolingDown,
    /// Egress circuit is open until a controlled probe becomes due.
    CircuitOpen,
    /// One controlled egress probe may be started.
    ProbeDue,
    /// One controlled egress probe is currently owned.
    ProbeInFlight,
    /// Egress is administratively disabled.
    Disabled,
    /// Session or clearance has not been established.
    Absent,
    /// Provider session remains active.
    Active,
    /// Session or clearance has expired.
    Expired,
    /// Provider session requires an explicit challenge.
    ChallengeRequired,
    /// Session or clearance lineage is invalid.
    Invalid,
    /// Browser clearance remains fresh.
    Fresh,
    /// Browser clearance requires an explicit refresh.
    RefreshRequired,
    /// One bounded browser-clearance refresh is currently owned.
    RefreshInFlight,
}

impl ProviderEgressStatusState {
    /// Stable management wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::CoolingDown => "cooling_down",
            Self::CircuitOpen => "circuit_open",
            Self::ProbeDue => "probe_due",
            Self::ProbeInFlight => "probe_in_flight",
            Self::Disabled => "disabled",
            Self::Absent => "absent",
            Self::Active => "active",
            Self::Expired => "expired",
            Self::ChallengeRequired => "challenge_required",
            Self::Invalid => "invalid",
            Self::Fresh => "fresh",
            Self::RefreshRequired => "refresh_required",
            Self::RefreshInFlight => "refresh_in_flight",
        }
    }

    /// Returns whether this state belongs to the requested distinct runtime domain.
    #[must_use]
    pub const fn supports_domain(self, domain: ProviderEgressStatusDomain) -> bool {
        match domain {
            ProviderEgressStatusDomain::Egress => matches!(
                self,
                Self::Available
                    | Self::CoolingDown
                    | Self::CircuitOpen
                    | Self::ProbeDue
                    | Self::ProbeInFlight
                    | Self::Disabled
            ),
            ProviderEgressStatusDomain::Session => matches!(
                self,
                Self::Absent
                    | Self::Active
                    | Self::Expired
                    | Self::ChallengeRequired
                    | Self::Invalid
            ),
            ProviderEgressStatusDomain::Clearance => matches!(
                self,
                Self::Absent
                    | Self::Fresh
                    | Self::Expired
                    | Self::RefreshRequired
                    | Self::RefreshInFlight
                    | Self::Invalid
            ),
        }
    }
}

impl FromStr for ProviderEgressStatusState {
    type Err = ProviderEgressStatusError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "available" => Ok(Self::Available),
            "cooling_down" => Ok(Self::CoolingDown),
            "circuit_open" => Ok(Self::CircuitOpen),
            "probe_due" => Ok(Self::ProbeDue),
            "probe_in_flight" => Ok(Self::ProbeInFlight),
            "disabled" => Ok(Self::Disabled),
            "absent" => Ok(Self::Absent),
            "active" => Ok(Self::Active),
            "expired" => Ok(Self::Expired),
            "challenge_required" => Ok(Self::ChallengeRequired),
            "invalid" => Ok(Self::Invalid),
            "fresh" => Ok(Self::Fresh),
            "refresh_required" => Ok(Self::RefreshRequired),
            "refresh_in_flight" => Ok(Self::RefreshInFlight),
            _ => Err(ProviderEgressStatusError::InvalidQuery),
        }
    }
}

/// Closed public shape of an egress target. A named value is an opaque label, never a URL.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderEgressStatusTargetKind {
    /// The exact direct transport profile.
    Direct,
    /// An opaque fixed-profile or pool-node identity.
    Named,
}

impl ProviderEgressStatusTargetKind {
    /// Stable management wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Named => "named",
        }
    }
}

impl FromStr for ProviderEgressStatusTargetKind {
    type Err = ProviderEgressStatusError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "direct" => Ok(Self::Direct),
            "named" => Ok(Self::Named),
            _ => Err(ProviderEgressStatusError::InvalidQuery),
        }
    }
}

/// Exact safe Provider/Upstream/Endpoint identity shared by every runtime-state item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEgressStatusChannelIdentity {
    /// Provider implementation family.
    pub provider_id: ProviderId,
    /// Exact configured Upstream instance.
    pub upstream_id: UpstreamId,
    /// Exact configured Endpoint used as the public channel identity.
    pub channel_id: EndpointId,
    /// Explicit channel capability; never inferred from an ID or endpoint URL.
    pub channel_kind: ProviderEgressStatusChannelKind,
}

impl ProviderEgressStatusChannelIdentity {
    /// Creates one exact bounded Provider/Upstream/Channel identity.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderEgressStatusError::InvalidSnapshot`] when any identity is blank,
    /// unbounded, contains control characters, or has surrounding whitespace.
    pub fn try_new(
        provider_id: ProviderId,
        upstream_id: UpstreamId,
        channel_id: EndpointId,
        channel_kind: ProviderEgressStatusChannelKind,
    ) -> Result<Self, ProviderEgressStatusError> {
        if !valid_channel_identity_parts(&provider_id, &upstream_id, &channel_id) {
            return Err(ProviderEgressStatusError::InvalidSnapshot);
        }
        Ok(Self {
            provider_id,
            upstream_id,
            channel_id,
            channel_kind,
        })
    }
}

/// Exact direct or opaque named egress target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEgressStatusTarget {
    /// Closed target kind.
    pub kind: ProviderEgressStatusTargetKind,
    /// Opaque named identity; always absent for direct transport.
    pub id: Option<String>,
}

impl ProviderEgressStatusTarget {
    /// Creates the direct target shape.
    #[must_use]
    pub const fn direct() -> Self {
        Self {
            kind: ProviderEgressStatusTargetKind::Direct,
            id: None,
        }
    }

    /// Creates a bounded opaque named target.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderEgressStatusError::InvalidSnapshot`] for a blank, unbounded, controlled,
    /// or whitespace-surrounded identity.
    pub fn named(id: impl Into<String>) -> Result<Self, ProviderEgressStatusError> {
        let id = id.into();
        if !valid_opaque_id(&id) {
            return Err(ProviderEgressStatusError::InvalidSnapshot);
        }
        Ok(Self {
            kind: ProviderEgressStatusTargetKind::Named,
            id: Some(id),
        })
    }

    /// Reconstructs a decoded target while enforcing direct/named nullability.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderEgressStatusError::InvalidSnapshot`] when `kind` and `id` disagree.
    pub fn try_new(
        kind: ProviderEgressStatusTargetKind,
        id: Option<String>,
    ) -> Result<Self, ProviderEgressStatusError> {
        match (kind, id) {
            (ProviderEgressStatusTargetKind::Direct, None) => Ok(Self::direct()),
            (ProviderEgressStatusTargetKind::Named, Some(id)) => Self::named(id),
            _ => Err(ProviderEgressStatusError::InvalidSnapshot),
        }
    }
}

/// Secret-free egress-domain item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEgressStatusEgressItem {
    /// Exact channel namespace.
    pub channel: ProviderEgressStatusChannelIdentity,
    /// Direct or opaque named target.
    pub target: ProviderEgressStatusTarget,
    /// Closed egress-domain state.
    pub state: ProviderEgressStatusState,
    /// Exclusive cooldown, probe-due, or probe-ticket deadline when the state owns one.
    pub deadline_ms: Option<i64>,
}

impl ProviderEgressStatusEgressItem {
    /// Builds one egress item with exact state/deadline semantics.
    ///
    /// # Errors
    ///
    /// Rejects non-egress states, missing required deadlines, and deadlines on states that do not
    /// own one.
    pub fn try_new(
        channel: ProviderEgressStatusChannelIdentity,
        target: ProviderEgressStatusTarget,
        state: ProviderEgressStatusState,
        deadline_ms: Option<i64>,
    ) -> Result<Self, ProviderEgressStatusError> {
        if !valid_channel_identity(&channel)
            || !valid_channel_target(&channel, &target)
            || !state.supports_domain(ProviderEgressStatusDomain::Egress)
            || !deadline_shape_is_valid(
                state,
                deadline_ms,
                &[
                    ProviderEgressStatusState::CoolingDown,
                    ProviderEgressStatusState::CircuitOpen,
                    ProviderEgressStatusState::ProbeInFlight,
                ],
            )
        {
            return Err(ProviderEgressStatusError::InvalidSnapshot);
        }
        Ok(Self {
            channel,
            target,
            state,
            deadline_ms,
        })
    }
}

/// Secret-free Provider-session-domain item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEgressStatusSessionItem {
    /// Exact channel namespace.
    pub channel: ProviderEgressStatusChannelIdentity,
    /// Non-secret Credential identity.
    pub credential_id: CredentialId,
    /// Exact Credential revision owning this lineage.
    pub credential_revision: u64,
    /// Exact Provider-session lineage revision.
    pub session_revision: u64,
    /// Closed session-domain state.
    pub state: ProviderEgressStatusState,
    /// Exclusive session expiry, present only while active.
    pub expires_at_ms: Option<i64>,
}

impl ProviderEgressStatusSessionItem {
    /// Builds one Provider-session item with exact lineage and expiry semantics.
    ///
    /// # Errors
    ///
    /// Rejects invalid Credential identities, unsafe/zero revisions, non-session states, and
    /// inconsistent expiry fields.
    pub fn try_new(
        channel: ProviderEgressStatusChannelIdentity,
        credential_id: CredentialId,
        credential_revision: u64,
        session_revision: u64,
        state: ProviderEgressStatusState,
        expires_at_ms: Option<i64>,
    ) -> Result<Self, ProviderEgressStatusError> {
        if !valid_channel_identity(&channel)
            || !channel.channel_kind.supports_provider_session()
            || !valid_opaque_id(credential_id.as_str())
            || !valid_lineage_revision(credential_revision)
            || !valid_lineage_revision(session_revision)
            || !state.supports_domain(ProviderEgressStatusDomain::Session)
            || !deadline_shape_is_valid(state, expires_at_ms, &[ProviderEgressStatusState::Active])
        {
            return Err(ProviderEgressStatusError::InvalidSnapshot);
        }
        Ok(Self {
            channel,
            credential_id,
            credential_revision,
            session_revision,
            state,
            expires_at_ms,
        })
    }
}

/// Secret-free browser-clearance-domain item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEgressStatusClearanceItem {
    /// Exact channel namespace.
    pub channel: ProviderEgressStatusChannelIdentity,
    /// Non-secret Credential identity.
    pub credential_id: CredentialId,
    /// Exact Credential revision owning this lineage.
    pub credential_revision: u64,
    /// Exact Provider-session lineage revision.
    pub session_revision: u64,
    /// Direct or opaque named target bound to the clearance.
    pub target: ProviderEgressStatusTarget,
    /// Exact browser-clearance lineage revision.
    pub clearance_revision: u64,
    /// Closed clearance-domain state.
    pub state: ProviderEgressStatusState,
    /// Exclusive freshness or refresh-ticket deadline when the state owns one.
    pub expires_at_ms: Option<i64>,
}

impl ProviderEgressStatusClearanceItem {
    /// Builds one clearance item with exact lineage, target, and expiry semantics.
    ///
    /// # Errors
    ///
    /// Rejects invalid identities/revisions, non-clearance states, and inconsistent expiry fields.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        channel: ProviderEgressStatusChannelIdentity,
        credential_id: CredentialId,
        credential_revision: u64,
        session_revision: u64,
        target: ProviderEgressStatusTarget,
        clearance_revision: u64,
        state: ProviderEgressStatusState,
        expires_at_ms: Option<i64>,
    ) -> Result<Self, ProviderEgressStatusError> {
        if !valid_channel_identity(&channel)
            || !channel.channel_kind.supports_clearance()
            || !valid_opaque_id(credential_id.as_str())
            || !valid_lineage_revision(credential_revision)
            || !valid_lineage_revision(session_revision)
            || !valid_channel_target(&channel, &target)
            || !valid_lineage_revision(clearance_revision)
            || !state.supports_domain(ProviderEgressStatusDomain::Clearance)
            || !deadline_shape_is_valid(
                state,
                expires_at_ms,
                &[
                    ProviderEgressStatusState::Fresh,
                    ProviderEgressStatusState::RefreshInFlight,
                ],
            )
        {
            return Err(ProviderEgressStatusError::InvalidSnapshot);
        }
        Ok(Self {
            channel,
            credential_id,
            credential_revision,
            session_revision,
            target,
            clearance_revision,
            state,
            expires_at_ms,
        })
    }
}

/// Strict tagged union preserving the three independently meaningful runtime domains.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderEgressStatusItem {
    /// Egress availability item.
    Egress(ProviderEgressStatusEgressItem),
    /// Provider-session lineage item.
    Session(ProviderEgressStatusSessionItem),
    /// Browser-clearance lineage item.
    Clearance(ProviderEgressStatusClearanceItem),
}

impl ProviderEgressStatusItem {
    /// Returns the distinct runtime domain.
    #[must_use]
    pub const fn domain(&self) -> ProviderEgressStatusDomain {
        match self {
            Self::Egress(_) => ProviderEgressStatusDomain::Egress,
            Self::Session(_) => ProviderEgressStatusDomain::Session,
            Self::Clearance(_) => ProviderEgressStatusDomain::Clearance,
        }
    }

    /// Returns the exact shared channel identity.
    #[must_use]
    pub const fn channel(&self) -> &ProviderEgressStatusChannelIdentity {
        match self {
            Self::Egress(item) => &item.channel,
            Self::Session(item) => &item.channel,
            Self::Clearance(item) => &item.channel,
        }
    }

    /// Returns the closed state.
    #[must_use]
    pub const fn state(&self) -> ProviderEgressStatusState {
        match self {
            Self::Egress(item) => item.state,
            Self::Session(item) => item.state,
            Self::Clearance(item) => item.state,
        }
    }

    /// Returns the safe Credential identity for session/clearance rows only.
    #[must_use]
    pub const fn credential_id(&self) -> Option<&CredentialId> {
        match self {
            Self::Egress(_) => None,
            Self::Session(item) => Some(&item.credential_id),
            Self::Clearance(item) => Some(&item.credential_id),
        }
    }

    fn validate_at(&self, sampled_at_ms: i64) -> Result<(), ProviderEgressStatusError> {
        let (shape_is_valid, deadline) = match self {
            Self::Egress(item) => (
                valid_channel_identity(&item.channel)
                    && valid_channel_target(&item.channel, &item.target)
                    && item
                        .state
                        .supports_domain(ProviderEgressStatusDomain::Egress)
                    && deadline_shape_is_valid(
                        item.state,
                        item.deadline_ms,
                        &[
                            ProviderEgressStatusState::CoolingDown,
                            ProviderEgressStatusState::CircuitOpen,
                            ProviderEgressStatusState::ProbeInFlight,
                        ],
                    ),
                item.deadline_ms,
            ),
            Self::Session(item) => (
                valid_channel_identity(&item.channel)
                    && item.channel.channel_kind.supports_provider_session()
                    && valid_opaque_id(item.credential_id.as_str())
                    && valid_lineage_revision(item.credential_revision)
                    && valid_lineage_revision(item.session_revision)
                    && item
                        .state
                        .supports_domain(ProviderEgressStatusDomain::Session)
                    && deadline_shape_is_valid(
                        item.state,
                        item.expires_at_ms,
                        &[ProviderEgressStatusState::Active],
                    ),
                item.expires_at_ms,
            ),
            Self::Clearance(item) => (
                valid_channel_identity(&item.channel)
                    && item.channel.channel_kind.supports_clearance()
                    && valid_opaque_id(item.credential_id.as_str())
                    && valid_lineage_revision(item.credential_revision)
                    && valid_lineage_revision(item.session_revision)
                    && valid_channel_target(&item.channel, &item.target)
                    && valid_lineage_revision(item.clearance_revision)
                    && item
                        .state
                        .supports_domain(ProviderEgressStatusDomain::Clearance)
                    && deadline_shape_is_valid(
                        item.state,
                        item.expires_at_ms,
                        &[
                            ProviderEgressStatusState::Fresh,
                            ProviderEgressStatusState::RefreshInFlight,
                        ],
                    ),
                item.expires_at_ms,
            ),
        };
        if !shape_is_valid || deadline.is_some_and(|value| value <= sampled_at_ms) {
            return Err(ProviderEgressStatusError::InvalidSnapshot);
        }
        Ok(())
    }

    fn stable_key(&self) -> ProviderEgressStatusItemKey {
        let channel = self.channel();
        match self {
            Self::Egress(item) => ProviderEgressStatusItemKey {
                provider_id: channel.provider_id.clone(),
                upstream_id: channel.upstream_id.clone(),
                channel_id: channel.channel_id.clone(),
                domain: ProviderEgressStatusDomain::Egress,
                credential_id: None,
                credential_revision: None,
                session_revision: None,
                target_kind: Some(item.target.kind),
                target_id: item.target.id.clone(),
                clearance_revision: None,
            },
            Self::Session(item) => ProviderEgressStatusItemKey {
                provider_id: channel.provider_id.clone(),
                upstream_id: channel.upstream_id.clone(),
                channel_id: channel.channel_id.clone(),
                domain: ProviderEgressStatusDomain::Session,
                credential_id: Some(item.credential_id.clone()),
                credential_revision: Some(item.credential_revision),
                session_revision: Some(item.session_revision),
                target_kind: None,
                target_id: None,
                clearance_revision: None,
            },
            Self::Clearance(item) => ProviderEgressStatusItemKey {
                provider_id: channel.provider_id.clone(),
                upstream_id: channel.upstream_id.clone(),
                channel_id: channel.channel_id.clone(),
                domain: ProviderEgressStatusDomain::Clearance,
                credential_id: Some(item.credential_id.clone()),
                credential_revision: Some(item.credential_revision),
                session_revision: Some(item.session_revision),
                target_kind: Some(item.target.kind),
                target_id: item.target.id.clone(),
                clearance_revision: Some(item.clearance_revision),
            },
        }
    }
}

/// Typed stable key retained inside a snapshot-bound keyset cursor.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProviderEgressStatusItemKey {
    provider_id: ProviderId,
    upstream_id: UpstreamId,
    channel_id: EndpointId,
    domain: ProviderEgressStatusDomain,
    credential_id: Option<CredentialId>,
    credential_revision: Option<u64>,
    session_revision: Option<u64>,
    target_kind: Option<ProviderEgressStatusTargetKind>,
    target_id: Option<String>,
    clearance_revision: Option<u64>,
}

impl ProviderEgressStatusItemKey {
    /// Reconstructs a decoded cursor key and enforces its domain-specific shape.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderEgressStatusError::InvalidQuery`] for invalid identities, revisions, or
    /// a key shape that does not match its domain.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        provider_id: ProviderId,
        upstream_id: UpstreamId,
        channel_id: EndpointId,
        domain: ProviderEgressStatusDomain,
        credential_id: Option<CredentialId>,
        credential_revision: Option<u64>,
        session_revision: Option<u64>,
        target_kind: Option<ProviderEgressStatusTargetKind>,
        target_id: Option<String>,
        clearance_revision: Option<u64>,
    ) -> Result<Self, ProviderEgressStatusError> {
        if !valid_opaque_id(provider_id.as_str())
            || !valid_opaque_id(upstream_id.as_str())
            || !valid_opaque_id(channel_id.as_str())
            || credential_id
                .as_ref()
                .is_some_and(|value| !valid_opaque_id(value.as_str()))
            || credential_revision.is_some_and(|value| !valid_lineage_revision(value))
            || session_revision.is_some_and(|value| !valid_lineage_revision(value))
            || clearance_revision.is_some_and(|value| !valid_lineage_revision(value))
            || !target_shape_is_valid(target_kind, target_id.as_deref())
        {
            return Err(ProviderEgressStatusError::InvalidQuery);
        }
        let shape_is_valid = match domain {
            ProviderEgressStatusDomain::Egress => {
                credential_id.is_none()
                    && credential_revision.is_none()
                    && session_revision.is_none()
                    && target_kind.is_some()
                    && clearance_revision.is_none()
            }
            ProviderEgressStatusDomain::Session => {
                credential_id.is_some()
                    && credential_revision.is_some()
                    && session_revision.is_some()
                    && target_kind.is_none()
                    && target_id.is_none()
                    && clearance_revision.is_none()
            }
            ProviderEgressStatusDomain::Clearance => {
                credential_id.is_some()
                    && credential_revision.is_some()
                    && session_revision.is_some()
                    && target_kind.is_some()
                    && clearance_revision.is_some()
            }
        };
        if !shape_is_valid {
            return Err(ProviderEgressStatusError::InvalidQuery);
        }
        Ok(Self {
            provider_id,
            upstream_id,
            channel_id,
            domain,
            credential_id,
            credential_revision,
            session_revision,
            target_kind,
            target_id,
            clearance_revision,
        })
    }

    /// Exact Provider identity.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Exact Upstream identity.
    #[must_use]
    pub const fn upstream_id(&self) -> &UpstreamId {
        &self.upstream_id
    }

    /// Exact channel/Endpoint identity.
    #[must_use]
    pub const fn channel_id(&self) -> &EndpointId {
        &self.channel_id
    }

    /// Distinct runtime domain.
    #[must_use]
    pub const fn domain(&self) -> ProviderEgressStatusDomain {
        self.domain
    }

    /// Optional safe Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> Option<&CredentialId> {
        self.credential_id.as_ref()
    }

    /// Optional exact Credential revision.
    #[must_use]
    pub const fn credential_revision(&self) -> Option<u64> {
        self.credential_revision
    }

    /// Optional exact Provider-session lineage revision.
    #[must_use]
    pub const fn session_revision(&self) -> Option<u64> {
        self.session_revision
    }

    /// Optional target kind.
    #[must_use]
    pub const fn target_kind(&self) -> Option<ProviderEgressStatusTargetKind> {
        self.target_kind
    }

    /// Optional opaque named target identity.
    #[must_use]
    pub fn target_id(&self) -> Option<&str> {
        self.target_id.as_deref()
    }

    /// Optional exact browser-clearance lineage revision.
    #[must_use]
    pub const fn clearance_revision(&self) -> Option<u64> {
        self.clearance_revision
    }

    fn encoded_len(&self) -> usize {
        self.provider_id.as_str().len()
            + self.upstream_id.as_str().len()
            + self.channel_id.as_str().len()
            + self
                .credential_id
                .as_ref()
                .map_or(0, |value| value.as_str().len())
            + self.target_id.as_ref().map_or(0, String::len)
            + 256
    }
}

/// Bounded filters and an opaque immutable-snapshot keyset position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEgressStatusQuery {
    provider_id: Option<ProviderId>,
    upstream_id: Option<UpstreamId>,
    channel_id: Option<EndpointId>,
    domain: Option<ProviderEgressStatusDomain>,
    state: Option<ProviderEgressStatusState>,
    credential_id: Option<CredentialId>,
    limit: usize,
    cursor: Option<ProviderEgressStatusCursor>,
}

impl Default for ProviderEgressStatusQuery {
    fn default() -> Self {
        Self {
            provider_id: None,
            upstream_id: None,
            channel_id: None,
            domain: None,
            state: None,
            credential_id: None,
            limit: DEFAULT_PROVIDER_EGRESS_STATUS_LIMIT,
            cursor: None,
        }
    }
}

impl ProviderEgressStatusQuery {
    /// Creates one strict, bounded query.
    ///
    /// # Errors
    ///
    /// Rejects invalid identities, a zero/oversized page, a state incompatible with an explicit
    /// domain, and a Credential filter explicitly paired with the egress-only domain.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        provider_id: Option<ProviderId>,
        upstream_id: Option<UpstreamId>,
        channel_id: Option<EndpointId>,
        domain: Option<ProviderEgressStatusDomain>,
        state: Option<ProviderEgressStatusState>,
        credential_id: Option<CredentialId>,
        limit: usize,
        cursor: Option<ProviderEgressStatusCursor>,
    ) -> Result<Self, ProviderEgressStatusError> {
        if !(1..=MAX_PROVIDER_EGRESS_STATUS_LIMIT).contains(&limit)
            || provider_id
                .as_ref()
                .is_some_and(|value| !valid_opaque_id(value.as_str()))
            || upstream_id
                .as_ref()
                .is_some_and(|value| !valid_opaque_id(value.as_str()))
            || channel_id
                .as_ref()
                .is_some_and(|value| !valid_opaque_id(value.as_str()))
            || credential_id
                .as_ref()
                .is_some_and(|value| !valid_opaque_id(value.as_str()))
            || domain
                .zip(state)
                .is_some_and(|(domain, state)| !state.supports_domain(domain))
            || (domain == Some(ProviderEgressStatusDomain::Egress) && credential_id.is_some())
        {
            return Err(ProviderEgressStatusError::InvalidQuery);
        }
        Ok(Self {
            provider_id,
            upstream_id,
            channel_id,
            domain,
            state,
            credential_id,
            limit,
            cursor,
        })
    }

    /// Exact Provider filter.
    #[must_use]
    pub const fn provider_id(&self) -> Option<&ProviderId> {
        self.provider_id.as_ref()
    }

    /// Exact Upstream filter.
    #[must_use]
    pub const fn upstream_id(&self) -> Option<&UpstreamId> {
        self.upstream_id.as_ref()
    }

    /// Exact channel/Endpoint filter.
    #[must_use]
    pub const fn channel_id(&self) -> Option<&EndpointId> {
        self.channel_id.as_ref()
    }

    /// Distinct domain filter.
    #[must_use]
    pub const fn domain(&self) -> Option<ProviderEgressStatusDomain> {
        self.domain
    }

    /// Closed state filter.
    #[must_use]
    pub const fn state(&self) -> Option<ProviderEgressStatusState> {
        self.state
    }

    /// Exact safe Credential filter.
    #[must_use]
    pub const fn credential_id(&self) -> Option<&CredentialId> {
        self.credential_id.as_ref()
    }

    /// Requested bounded page size.
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Immutable-snapshot keyset position.
    #[must_use]
    pub const fn cursor(&self) -> Option<&ProviderEgressStatusCursor> {
        self.cursor.as_ref()
    }

    fn matches(&self, item: &ProviderEgressStatusItem) -> bool {
        let channel = item.channel();
        self.provider_id
            .as_ref()
            .is_none_or(|value| value == &channel.provider_id)
            && self
                .upstream_id
                .as_ref()
                .is_none_or(|value| value == &channel.upstream_id)
            && self
                .channel_id
                .as_ref()
                .is_none_or(|value| value == &channel.channel_id)
            && self.domain.is_none_or(|value| value == item.domain())
            && self.state.is_none_or(|value| value == item.state())
            && self
                .credential_id
                .as_ref()
                .is_none_or(|value| item.credential_id() == Some(value))
    }

    fn filter_fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        update_optional_fingerprint(
            &mut hasher,
            "provider_id",
            self.provider_id.as_ref().map(ProviderId::as_str),
        );
        update_optional_fingerprint(
            &mut hasher,
            "upstream_id",
            self.upstream_id.as_ref().map(UpstreamId::as_str),
        );
        update_optional_fingerprint(
            &mut hasher,
            "channel_id",
            self.channel_id.as_ref().map(EndpointId::as_str),
        );
        update_optional_fingerprint(
            &mut hasher,
            "domain",
            self.domain.map(ProviderEgressStatusDomain::as_str),
        );
        update_optional_fingerprint(
            &mut hasher,
            "state",
            self.state.map(ProviderEgressStatusState::as_str),
        );
        update_optional_fingerprint(
            &mut hasher,
            "credential_id",
            self.credential_id.as_ref().map(CredentialId::as_str),
        );
        let digest = hasher.finalize();
        let mut output = String::with_capacity(FILTER_FINGERPRINT_HEX_CHARS);
        for byte in digest {
            let _ = write!(&mut output, "{byte:02x}");
        }
        output
    }
}

/// Immutable-snapshot-bound keyset cursor. The HTTP adapter owns its transport encoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEgressStatusCursor {
    config_version_id: ConfigVersionId,
    config_revision: ConfigRevision,
    runtime_revision: u64,
    snapshot_id: String,
    sampled_at_ms: i64,
    filter_fingerprint: String,
    last_key: ProviderEgressStatusItemKey,
}

impl ProviderEgressStatusCursor {
    /// Reconstructs one decoded cursor after bounded transport decoding.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderEgressStatusError::InvalidQuery`] when identity, revision, timestamp,
    /// fingerprint, or key bounds are invalid.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        config_version_id: ConfigVersionId,
        config_revision: ConfigRevision,
        runtime_revision: u64,
        snapshot_id: impl Into<String>,
        sampled_at_ms: i64,
        filter_fingerprint: impl Into<String>,
        last_key: ProviderEgressStatusItemKey,
    ) -> Result<Self, ProviderEgressStatusError> {
        let snapshot_id = snapshot_id.into();
        let filter_fingerprint = filter_fingerprint.into();
        if !valid_opaque_id(config_version_id.as_str())
            || !valid_config_revision(config_revision)
            || !valid_runtime_revision(runtime_revision)
            || !valid_snapshot_id(&snapshot_id)
            || !valid_timestamp(sampled_at_ms)
            || !valid_filter_fingerprint(&filter_fingerprint)
            || last_key.encoded_len() > MAX_CURSOR_KEY_BYTES
        {
            return Err(ProviderEgressStatusError::InvalidQuery);
        }
        Ok(Self {
            config_version_id,
            config_revision,
            runtime_revision,
            snapshot_id,
            sampled_at_ms,
            filter_fingerprint,
            last_key,
        })
    }

    /// Exact Config Version identity.
    #[must_use]
    pub const fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Exact Config Version revision.
    #[must_use]
    pub const fn config_revision(&self) -> ConfigRevision {
        self.config_revision
    }

    /// Exact monotonic runtime-state revision.
    #[must_use]
    pub const fn runtime_revision(&self) -> u64 {
        self.runtime_revision
    }

    /// Retained atomic snapshot identity.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Fixed snapshot sample time.
    #[must_use]
    pub const fn sampled_at_ms(&self) -> i64 {
        self.sampled_at_ms
    }

    /// SHA-256 filter fingerprint.
    #[must_use]
    pub fn filter_fingerprint(&self) -> &str {
        &self.filter_fingerprint
    }

    /// Last stable item key emitted by the prior page.
    #[must_use]
    pub const fn last_key(&self) -> &ProviderEgressStatusItemKey {
        &self.last_key
    }
}

/// One externally identified, atomic and immutable runtime snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEgressStatusSnapshot {
    config_version_id: ConfigVersionId,
    config_revision: ConfigRevision,
    runtime_revision: u64,
    snapshot_id: String,
    sampled_at_ms: i64,
    items: Vec<ProviderEgressStatusItem>,
}

impl ProviderEgressStatusSnapshot {
    /// Validates and deterministically orders one externally retained snapshot.
    ///
    /// This constructor does not create a cache or synthesize an identity. The adapter supplying
    /// the atomic snapshot must also supply its retained `snapshot_id` and `runtime_revision`.
    ///
    /// # Errors
    ///
    /// Rejects invalid configuration/snapshot identity, unsafe revisions/timestamps, oversized
    /// snapshots, inconsistent effective deadlines, or duplicate stable keys.
    pub fn try_new(
        config_version_id: ConfigVersionId,
        config_revision: ConfigRevision,
        runtime_revision: u64,
        snapshot_id: impl Into<String>,
        sampled_at_ms: i64,
        mut items: Vec<ProviderEgressStatusItem>,
    ) -> Result<Self, ProviderEgressStatusError> {
        let snapshot_id = snapshot_id.into();
        if !valid_opaque_id(config_version_id.as_str())
            || !valid_config_revision(config_revision)
            || !valid_runtime_revision(runtime_revision)
            || !valid_snapshot_id(&snapshot_id)
            || !valid_timestamp(sampled_at_ms)
            || items.len() > MAX_PROVIDER_EGRESS_STATUS_SNAPSHOT_ITEMS
        {
            return Err(ProviderEgressStatusError::InvalidSnapshot);
        }
        let mut keys = BTreeSet::new();
        let mut channels = BTreeMap::new();
        for item in &items {
            item.validate_at(sampled_at_ms)?;
            let channel = item.channel();
            let channel_key = (
                channel.provider_id.clone(),
                channel.upstream_id.clone(),
                channel.channel_id.clone(),
            );
            if channels
                .insert(channel_key, channel.channel_kind)
                .is_some_and(|kind| kind != channel.channel_kind)
            {
                return Err(ProviderEgressStatusError::InvalidSnapshot);
            }
            if !keys.insert(item.stable_key()) {
                return Err(ProviderEgressStatusError::InvalidSnapshot);
            }
        }
        items.sort_by_key(ProviderEgressStatusItem::stable_key);
        Ok(Self {
            config_version_id,
            config_revision,
            runtime_revision,
            snapshot_id,
            sampled_at_ms,
            items,
        })
    }

    /// Exact Config Version identity bound to this runtime composition.
    #[must_use]
    pub const fn config_version_id(&self) -> &ConfigVersionId {
        &self.config_version_id
    }

    /// Exact Config Version revision bound to this runtime composition.
    #[must_use]
    pub const fn config_revision(&self) -> ConfigRevision {
        self.config_revision
    }

    /// Monotonic runtime-state revision captured atomically with all rows.
    #[must_use]
    pub const fn runtime_revision(&self) -> u64 {
        self.runtime_revision
    }

    /// Externally supplied retained snapshot identity.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Fixed observation time used for every effective state.
    #[must_use]
    pub const fn sampled_at_ms(&self) -> i64 {
        self.sampled_at_ms
    }

    /// Builds one bounded page without observing or mutating live state.
    ///
    /// # Errors
    ///
    /// Returns `ConfigConflict` when the selected Config Version is not this snapshot's source,
    /// or `CursorConflict` when the cursor belongs to another configuration, runtime snapshot, or
    /// filter set.
    pub fn page(
        &self,
        config_version_id: &ConfigVersionId,
        config_revision: ConfigRevision,
        query: &ProviderEgressStatusQuery,
    ) -> Result<ProviderEgressStatusPage, ProviderEgressStatusError> {
        if config_version_id != &self.config_version_id || config_revision != self.config_revision {
            return Err(ProviderEgressStatusError::ConfigConflict);
        }
        let filter_fingerprint = query.filter_fingerprint();
        if let Some(cursor) = &query.cursor
            && (cursor.config_version_id != *config_version_id
                || cursor.config_revision != config_revision
                || cursor.runtime_revision != self.runtime_revision
                || cursor.snapshot_id != self.snapshot_id
                || cursor.sampled_at_ms != self.sampled_at_ms
                || cursor.filter_fingerprint != filter_fingerprint
                || !self
                    .items
                    .iter()
                    .any(|item| item.stable_key() == cursor.last_key && query.matches(item)))
        {
            return Err(ProviderEgressStatusError::CursorConflict);
        }

        let after_key = query.cursor.as_ref().map(|cursor| &cursor.last_key);
        let mut matching = self.items.iter().filter(|item| {
            query.matches(item) && after_key.is_none_or(|last_key| item.stable_key() > *last_key)
        });
        let mut items = Vec::with_capacity(query.limit);
        for item in matching.by_ref().take(query.limit) {
            items.push(item.clone());
        }
        let has_more = matching.next().is_some();
        let next_cursor = if has_more {
            let last_key = items
                .last()
                .map(ProviderEgressStatusItem::stable_key)
                .ok_or(ProviderEgressStatusError::InvalidSnapshot)?;
            Some(
                ProviderEgressStatusCursor::try_new(
                    self.config_version_id.clone(),
                    self.config_revision,
                    self.runtime_revision,
                    self.snapshot_id.clone(),
                    self.sampled_at_ms,
                    filter_fingerprint,
                    last_key,
                )
                .map_err(|_| ProviderEgressStatusError::InvalidSnapshot)?,
            )
        } else {
            None
        };
        Ok(ProviderEgressStatusPage {
            config_version_id: self.config_version_id.clone(),
            config_revision: self.config_revision,
            runtime_revision: self.runtime_revision,
            snapshot_id: self.snapshot_id.clone(),
            sampled_at_ms: self.sampled_at_ms,
            items,
            next_cursor,
        })
    }
}

/// One bounded page from exactly one immutable runtime snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEgressStatusPage {
    /// Exact Config Version identity.
    pub config_version_id: ConfigVersionId,
    /// Exact Config Version revision.
    pub config_revision: ConfigRevision,
    /// Exact runtime-state revision.
    pub runtime_revision: u64,
    /// Retained source snapshot identity.
    pub snapshot_id: String,
    /// Fixed source snapshot sample time.
    pub sampled_at_ms: i64,
    /// Strict secret-free union rows.
    pub items: Vec<ProviderEgressStatusItem>,
    /// Immutable-snapshot-bound next-page position.
    pub next_cursor: Option<ProviderEgressStatusCursor>,
}

/// Safe failures exposed by the Provider-specific runtime-status seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderEgressStatusError {
    /// Query or decoded cursor is outside the bounded public contract.
    InvalidQuery,
    /// Provider adapter supplied an invalid or internally inconsistent safe snapshot.
    InvalidSnapshot,
    /// Cursor belongs to another immutable snapshot or filter set.
    CursorConflict,
    /// Caller-selected Config Version does not own the source runtime composition.
    ConfigConflict,
    /// No safe runtime snapshot source is available.
    SourceUnavailable,
}

impl fmt::Display for ProviderEgressStatusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidQuery => "provider egress-status query is invalid",
            Self::InvalidSnapshot => "provider egress-status snapshot is invalid",
            Self::CursorConflict => "provider egress-status cursor is stale",
            Self::ConfigConflict => "provider egress-status configuration is not serving",
            Self::SourceUnavailable => "provider egress-status source is unavailable",
        })
    }
}

impl Error for ProviderEgressStatusError {}

/// Provider-neutral read-only seam consumed by the protected management HTTP surface.
pub trait ProviderEgressStatusFacade: Send + Sync {
    /// Returns one page for the exact caller-selected Config Version and revision.
    ///
    /// Implementations must not contact a Provider, resolve an endpoint, refresh a credential or
    /// clearance, mutate runtime state, or silently substitute another Config Version.
    ///
    /// # Errors
    ///
    /// Returns one closed safe error when the exact snapshot cannot be projected.
    fn list_provider_egress_status(
        &self,
        config_version_id: &ConfigVersionId,
        config_revision: ConfigRevision,
        query: &ProviderEgressStatusQuery,
    ) -> Result<ProviderEgressStatusPage, ProviderEgressStatusError>;
}

/// Fail-closed default until a serving composition injects an exact snapshot facade.
pub struct RejectingProviderEgressStatusFacade;

impl RejectingProviderEgressStatusFacade {
    /// Creates the no-send, no-state default.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RejectingProviderEgressStatusFacade {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderEgressStatusFacade for RejectingProviderEgressStatusFacade {
    fn list_provider_egress_status(
        &self,
        _config_version_id: &ConfigVersionId,
        _config_revision: ConfigRevision,
        _query: &ProviderEgressStatusQuery,
    ) -> Result<ProviderEgressStatusPage, ProviderEgressStatusError> {
        Err(ProviderEgressStatusError::SourceUnavailable)
    }
}

/// Read-only facade around one already-retained immutable snapshot.
pub struct SnapshotProviderEgressStatusFacade {
    snapshot: ProviderEgressStatusSnapshot,
}

impl SnapshotProviderEgressStatusFacade {
    /// Wraps one validated immutable snapshot without observing live runtime state.
    #[must_use]
    pub const fn new(snapshot: ProviderEgressStatusSnapshot) -> Self {
        Self { snapshot }
    }
}

impl ProviderEgressStatusFacade for SnapshotProviderEgressStatusFacade {
    fn list_provider_egress_status(
        &self,
        config_version_id: &ConfigVersionId,
        config_revision: ConfigRevision,
        query: &ProviderEgressStatusQuery,
    ) -> Result<ProviderEgressStatusPage, ProviderEgressStatusError> {
        self.snapshot
            .page(config_version_id, config_revision, query)
    }
}

fn valid_opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= MAX_OPAQUE_ID_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_channel_identity_parts(
    provider_id: &ProviderId,
    upstream_id: &UpstreamId,
    channel_id: &EndpointId,
) -> bool {
    valid_opaque_id(provider_id.as_str())
        && valid_opaque_id(upstream_id.as_str())
        && valid_opaque_id(channel_id.as_str())
}

fn valid_channel_identity(channel: &ProviderEgressStatusChannelIdentity) -> bool {
    valid_channel_identity_parts(
        &channel.provider_id,
        &channel.upstream_id,
        &channel.channel_id,
    )
}

fn valid_target(target: &ProviderEgressStatusTarget) -> bool {
    target_shape_is_valid(target.kind.into(), target.id.as_deref())
}

fn valid_channel_target(
    channel: &ProviderEgressStatusChannelIdentity,
    target: &ProviderEgressStatusTarget,
) -> bool {
    valid_target(target)
        && (!channel.channel_kind.requires_named_target()
            || target.kind == ProviderEgressStatusTargetKind::Named)
}

fn valid_snapshot_id(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= MAX_SNAPSHOT_ID_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_lineage_revision(value: u64) -> bool {
    (1..=MAX_PROVIDER_EGRESS_STATUS_SAFE_INTEGER).contains(&value)
}

fn valid_runtime_revision(value: u64) -> bool {
    value <= MAX_PROVIDER_EGRESS_STATUS_SAFE_INTEGER
}

fn valid_config_revision(value: ConfigRevision) -> bool {
    u64::try_from(value.as_i64()).is_ok_and(valid_runtime_revision)
}

fn valid_timestamp(value: i64) -> bool {
    u64::try_from(value).is_ok_and(valid_runtime_revision)
}

fn deadline_shape_is_valid(
    state: ProviderEgressStatusState,
    deadline_ms: Option<i64>,
    required_for: &[ProviderEgressStatusState],
) -> bool {
    if required_for.contains(&state) {
        deadline_ms.is_some_and(valid_timestamp)
    } else {
        deadline_ms.is_none()
    }
}

fn target_shape_is_valid(
    target_kind: Option<ProviderEgressStatusTargetKind>,
    target_id: Option<&str>,
) -> bool {
    match (target_kind, target_id) {
        (None | Some(ProviderEgressStatusTargetKind::Direct), None) => true,
        (Some(ProviderEgressStatusTargetKind::Named), Some(value)) => valid_opaque_id(value),
        _ => false,
    }
}

fn update_optional_fingerprint(hasher: &mut Sha256, name: &str, value: Option<&str>) {
    hasher.update(name.len().to_be_bytes());
    hasher.update(name.as_bytes());
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.len().to_be_bytes());
            hasher.update(value.as_bytes());
        }
        None => hasher.update([0]),
    }
}

fn valid_filter_fingerprint(value: &str) -> bool {
    value.len() == FILTER_FINGERPRINT_HEX_CHARS
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn must<T, E>(value: Result<T, E>) -> T {
        match value {
            Ok(value) => value,
            Err(_) => std::process::abort(),
        }
    }

    fn must_some<T>(value: Option<T>) -> T {
        match value {
            Some(value) => value,
            None => std::process::abort(),
        }
    }

    fn config_id(value: &str) -> ConfigVersionId {
        must(ConfigVersionId::try_new(value))
    }

    fn revision(value: i64) -> ConfigRevision {
        must(ConfigRevision::try_new(value))
    }

    fn channel(
        provider: &str,
        upstream: &str,
        endpoint: &str,
        kind: ProviderEgressStatusChannelKind,
    ) -> ProviderEgressStatusChannelIdentity {
        must(ProviderEgressStatusChannelIdentity::try_new(
            must(ProviderId::try_new(provider)),
            must(UpstreamId::try_new(upstream)),
            must(EndpointId::try_new(endpoint)),
            kind,
        ))
    }

    fn egress(
        provider: &str,
        upstream: &str,
        endpoint: &str,
        target: ProviderEgressStatusTarget,
        state: ProviderEgressStatusState,
        deadline_ms: Option<i64>,
    ) -> ProviderEgressStatusItem {
        ProviderEgressStatusItem::Egress(must(ProviderEgressStatusEgressItem::try_new(
            channel(
                provider,
                upstream,
                endpoint,
                ProviderEgressStatusChannelKind::GrokBuild,
            ),
            target,
            state,
            deadline_ms,
        )))
    }

    fn session(account: &str, state: ProviderEgressStatusState) -> ProviderEgressStatusItem {
        ProviderEgressStatusItem::Session(must(ProviderEgressStatusSessionItem::try_new(
            channel(
                "grok",
                "grok-primary",
                "console",
                ProviderEgressStatusChannelKind::GrokConsole,
            ),
            must(CredentialId::try_new(account)),
            7,
            9,
            state,
            (state == ProviderEgressStatusState::Active).then_some(2_000),
        )))
    }

    fn clearance(account: &str, target: &str) -> ProviderEgressStatusItem {
        ProviderEgressStatusItem::Clearance(must(ProviderEgressStatusClearanceItem::try_new(
            channel(
                "grok",
                "grok-primary",
                "web",
                ProviderEgressStatusChannelKind::GrokWeb,
            ),
            must(CredentialId::try_new(account)),
            7,
            9,
            must(ProviderEgressStatusTarget::named(target)),
            11,
            ProviderEgressStatusState::Fresh,
            Some(2_000),
        )))
    }

    fn snapshot(items: Vec<ProviderEgressStatusItem>) -> ProviderEgressStatusSnapshot {
        must(ProviderEgressStatusSnapshot::try_new(
            config_id("config-a"),
            revision(4),
            12,
            "snapshot-a",
            1_000,
            items,
        ))
    }

    #[test]
    fn strict_union_retains_all_three_domains_and_direct_named_semantics() {
        let value = snapshot(vec![
            clearance("credential-c", "pool-node-c"),
            session("credential-s", ProviderEgressStatusState::Active),
            egress(
                "grok",
                "grok-primary",
                "build",
                ProviderEgressStatusTarget::direct(),
                ProviderEgressStatusState::Available,
                None,
            ),
        ]);
        assert_eq!(value.items[0].domain(), ProviderEgressStatusDomain::Egress);
        assert_eq!(value.items[1].domain(), ProviderEgressStatusDomain::Session);
        assert_eq!(
            value.items[2].domain(),
            ProviderEgressStatusDomain::Clearance
        );
        assert_eq!(
            ProviderEgressStatusTarget::try_new(
                ProviderEgressStatusTargetKind::Direct,
                Some("must-not-exist".to_owned())
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
        assert_eq!(
            ProviderEgressStatusTarget::named(" proxy-node "),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
    }

    #[test]
    fn domain_state_and_effective_deadline_invariants_fail_closed() {
        assert_eq!(
            ProviderEgressStatusEgressItem::try_new(
                channel(
                    "grok",
                    "grok-primary",
                    "build",
                    ProviderEgressStatusChannelKind::GrokBuild,
                ),
                ProviderEgressStatusTarget::direct(),
                ProviderEgressStatusState::Active,
                Some(2_000),
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
        assert_eq!(
            ProviderEgressStatusSessionItem::try_new(
                channel(
                    "grok",
                    "grok-primary",
                    "console",
                    ProviderEgressStatusChannelKind::GrokConsole,
                ),
                must(CredentialId::try_new("credential-a")),
                1,
                1,
                ProviderEgressStatusState::Active,
                None,
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
        let stale = session("credential-a", ProviderEgressStatusState::Active);
        assert_eq!(
            ProviderEgressStatusSnapshot::try_new(
                config_id("config-a"),
                revision(4),
                12,
                "snapshot-a",
                2_000,
                vec![stale]
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
        let forged = ProviderEgressStatusItem::Egress(ProviderEgressStatusEgressItem {
            channel: channel(
                "grok",
                "grok-primary",
                "build",
                ProviderEgressStatusChannelKind::GrokBuild,
            ),
            target: ProviderEgressStatusTarget {
                kind: ProviderEgressStatusTargetKind::Direct,
                id: Some("forged-named-id".to_owned()),
            },
            state: ProviderEgressStatusState::Available,
            deadline_ms: None,
        });
        assert_eq!(
            ProviderEgressStatusSnapshot::try_new(
                config_id("config-a"),
                revision(4),
                12,
                "snapshot-a",
                1_000,
                vec![forged]
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
    }

    #[test]
    fn channel_capabilities_and_identity_byte_bounds_fail_closed() {
        assert_eq!(
            ProviderEgressStatusSessionItem::try_new(
                channel(
                    "grok",
                    "grok-primary",
                    "build",
                    ProviderEgressStatusChannelKind::GrokBuild,
                ),
                must(CredentialId::try_new("credential-a")),
                1,
                1,
                ProviderEgressStatusState::Absent,
                None,
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
        assert_eq!(
            ProviderEgressStatusClearanceItem::try_new(
                channel(
                    "grok",
                    "grok-primary",
                    "console",
                    ProviderEgressStatusChannelKind::GrokConsole,
                ),
                must(CredentialId::try_new("credential-a")),
                1,
                1,
                must(ProviderEgressStatusTarget::named("sticky-node")),
                1,
                ProviderEgressStatusState::Absent,
                None,
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
        assert_eq!(
            ProviderEgressStatusEgressItem::try_new(
                channel(
                    "grok",
                    "grok-primary",
                    "web",
                    ProviderEgressStatusChannelKind::GrokWeb,
                ),
                ProviderEgressStatusTarget::direct(),
                ProviderEgressStatusState::Available,
                None,
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
        assert_eq!(
            ProviderEgressStatusTarget::named("🧪".repeat(128)),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
    }

    #[test]
    fn stable_order_filters_and_keyset_pages_do_not_cross_provider_ownership() {
        let value = snapshot(vec![
            egress(
                "z-provider",
                "z-upstream",
                "build",
                ProviderEgressStatusTarget::direct(),
                ProviderEgressStatusState::Available,
                None,
            ),
            session("credential-b", ProviderEgressStatusState::Absent),
            session("credential-a", ProviderEgressStatusState::Active),
            clearance("credential-c", "pool-node-c"),
        ]);
        let first_query = must(ProviderEgressStatusQuery::try_new(
            Some(must(ProviderId::try_new("grok"))),
            Some(must(UpstreamId::try_new("grok-primary"))),
            None,
            None,
            None,
            None,
            1,
            None,
        ));
        let first = must(value.page(&config_id("config-a"), revision(4), &first_query));
        assert_eq!(first.items.len(), 1);
        assert_eq!(
            first.items[0].credential_id().map(CredentialId::as_str),
            Some("credential-a")
        );
        assert!(first.next_cursor.is_some());
        let second_query = must(ProviderEgressStatusQuery::try_new(
            Some(must(ProviderId::try_new("grok"))),
            Some(must(UpstreamId::try_new("grok-primary"))),
            None,
            None,
            None,
            None,
            2,
            first.next_cursor,
        ));
        let second = must(value.page(&config_id("config-a"), revision(4), &second_query));
        assert_eq!(second.items.len(), 2);
        assert!(
            second
                .items
                .iter()
                .all(|item| item.channel().provider_id.as_str() == "grok")
        );
        assert!(second.next_cursor.is_none());
    }

    #[test]
    fn every_exact_filter_is_composed_and_duplicate_keys_fail_closed() {
        let active_session = session("credential-a", ProviderEgressStatusState::Active);
        let value = snapshot(vec![
            active_session.clone(),
            session("credential-b", ProviderEgressStatusState::Absent),
            clearance("credential-c", "pool-node-c"),
        ]);
        let query = must(ProviderEgressStatusQuery::try_new(
            Some(must(ProviderId::try_new("grok"))),
            Some(must(UpstreamId::try_new("grok-primary"))),
            Some(must(EndpointId::try_new("console"))),
            Some(ProviderEgressStatusDomain::Session),
            Some(ProviderEgressStatusState::Active),
            Some(must(CredentialId::try_new("credential-a"))),
            MAX_PROVIDER_EGRESS_STATUS_LIMIT,
            None,
        ));
        let page = must(value.page(&config_id("config-a"), revision(4), &query));
        assert_eq!(page.items, vec![active_session.clone()]);
        assert!(page.next_cursor.is_none());

        assert_eq!(
            ProviderEgressStatusSnapshot::try_new(
                config_id("config-a"),
                revision(4),
                12,
                "snapshot-a",
                1_000,
                vec![active_session.clone(), active_session],
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
        assert_eq!(
            ProviderEgressStatusQuery::try_new(
                None,
                None,
                None,
                None,
                None,
                None,
                MAX_PROVIDER_EGRESS_STATUS_LIMIT + 1,
                None,
            ),
            Err(ProviderEgressStatusError::InvalidQuery)
        );
    }

    #[test]
    fn config_mismatch_and_stale_filter_cursor_are_distinct_failures() {
        let value = snapshot(vec![
            session("credential-a", ProviderEgressStatusState::Active),
            session("credential-b", ProviderEgressStatusState::Absent),
        ]);
        let facade = SnapshotProviderEgressStatusFacade::new(value);
        assert_eq!(
            facade.list_provider_egress_status(
                &config_id("config-b"),
                revision(4),
                &ProviderEgressStatusQuery::default()
            ),
            Err(ProviderEgressStatusError::ConfigConflict)
        );
        let first = must(facade.list_provider_egress_status(
            &config_id("config-a"),
            revision(4),
            &must(ProviderEgressStatusQuery::try_new(
                None, None, None, None, None, None, 1, None,
            )),
        ));
        let mut stale_config_cursor = must_some(first.next_cursor.clone());
        stale_config_cursor.config_revision = revision(3);
        let stale_config_query = must(ProviderEgressStatusQuery::try_new(
            None,
            None,
            None,
            None,
            None,
            None,
            1,
            Some(stale_config_cursor),
        ));
        assert_eq!(
            facade.list_provider_egress_status(
                &config_id("config-a"),
                revision(4),
                &stale_config_query
            ),
            Err(ProviderEgressStatusError::CursorConflict)
        );
        let changed_filter = must(ProviderEgressStatusQuery::try_new(
            None,
            None,
            None,
            Some(ProviderEgressStatusDomain::Session),
            Some(ProviderEgressStatusState::Active),
            None,
            1,
            first.next_cursor,
        ));
        assert_eq!(
            facade.list_provider_egress_status(
                &config_id("config-a"),
                revision(4),
                &changed_filter
            ),
            Err(ProviderEgressStatusError::CursorConflict)
        );
    }

    #[test]
    fn query_rejects_cross_domain_state_and_egress_credential_filter() {
        assert_eq!(
            ProviderEgressStatusQuery::try_new(
                None,
                None,
                None,
                Some(ProviderEgressStatusDomain::Egress),
                Some(ProviderEgressStatusState::Fresh),
                None,
                50,
                None,
            ),
            Err(ProviderEgressStatusError::InvalidQuery)
        );
        assert_eq!(
            ProviderEgressStatusQuery::try_new(
                None,
                None,
                None,
                Some(ProviderEgressStatusDomain::Egress),
                None,
                Some(must(CredentialId::try_new("credential-a"))),
                50,
                None,
            ),
            Err(ProviderEgressStatusError::InvalidQuery)
        );
    }

    #[test]
    fn revisions_are_javascript_safe_and_lineage_revisions_are_nonzero() {
        assert_eq!(
            ProviderEgressStatusSnapshot::try_new(
                config_id("config-a"),
                revision(4),
                MAX_PROVIDER_EGRESS_STATUS_SAFE_INTEGER + 1,
                "snapshot-a",
                1_000,
                Vec::new(),
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
        assert_eq!(
            ProviderEgressStatusSessionItem::try_new(
                channel(
                    "grok",
                    "grok-primary",
                    "console",
                    ProviderEgressStatusChannelKind::GrokConsole,
                ),
                must(CredentialId::try_new("credential-a")),
                0,
                1,
                ProviderEgressStatusState::Absent,
                None,
            ),
            Err(ProviderEgressStatusError::InvalidSnapshot)
        );
    }

    #[test]
    fn debug_projection_contains_only_the_declared_safe_shape() {
        let value = snapshot(vec![
            egress(
                "grok",
                "grok-primary",
                "build",
                ProviderEgressStatusTarget::direct(),
                ProviderEgressStatusState::Available,
                None,
            ),
            session("credential-a", ProviderEgressStatusState::Active),
        ]);
        let page = must(value.page(
            &config_id("config-a"),
            revision(4),
            &ProviderEgressStatusQuery::default(),
        ));
        let debug = format!("{page:?}");
        for forbidden in [
            "endpoint_url",
            "proxy_url",
            "ciphertext",
            "plaintext",
            "cookie",
            "access_token",
            "refresh_token",
            "request_body",
        ] {
            assert!(!debug.contains(forbidden));
        }
        assert!(debug.contains("ProviderEgressStatusEgressItem"));
        assert!(debug.contains("ProviderEgressStatusSessionItem"));
    }

    #[test]
    fn rejecting_facade_never_returns_partial_state() {
        assert_eq!(
            RejectingProviderEgressStatusFacade::new().list_provider_egress_status(
                &config_id("config-a"),
                revision(4),
                &ProviderEgressStatusQuery::default(),
            ),
            Err(ProviderEgressStatusError::SourceUnavailable)
        );
    }
}
