//! Provider-aware, value-free egress/session/clearance runtime state.
//!
//! The generic compatible-endpoint runtime already owns concrete direct/fixed/pool transport
//! leases. This module adds the narrower Provider/Channel capability and state boundary needed by
//! native adapters without importing a Provider implementation, opening a network connection, or
//! duplicating Credential Health/Quota state. Build, Console, Web, official APIs, Codex/ChatGPT,
//! Kiro, Claude-compatible endpoints, and arbitrary compatible relays remain separate namespaces.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    sync::{Arc, RwLock},
};

use gateway_core::{CredentialId, EndpointId, ProviderId, UpstreamId};

/// Maximum bytes accepted for one non-secret opaque identity at this seam.
pub const MAX_PROVIDER_EGRESS_IDENTITY_LENGTH: usize = 128;
/// Maximum explicitly declared Provider/Channel capabilities in one runtime composition.
pub const MAX_PROVIDER_EGRESS_CAPABILITIES: usize = 4_096;
/// Maximum retained states in each independently bounded runtime domain.
pub const MAX_PROVIDER_EGRESS_STATES_PER_DOMAIN: usize = 16_384;
/// Maximum hidden auxiliary HTTP submissions declared by one logical Provider attempt.
pub const MAX_PROVIDER_EGRESS_AUXILIARY_REQUESTS: u8 = 8;
/// Maximum pre-submit recovery actions declared by one logical Provider attempt.
pub const MAX_PROVIDER_EGRESS_PRE_SUBMIT_RECOVERIES: u8 = 2;

/// Closed Provider/Channel behavior families admitted by the E1 capability seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderEgressChannel {
    /// An arbitrary OpenAI/Anthropic-compatible `base_url + credential` endpoint.
    GenericCompatible,
    /// Native Grok Build execution.
    GrokBuild,
    /// Native Grok Console execution with a channel-local provider session.
    GrokConsole,
    /// Native Grok Web execution with sticky browser session and clearance state.
    GrokWeb,
    /// A provider's official API-key endpoint using ordinary HTTP egress.
    OfficialApi,
    /// Official Codex/ChatGPT account execution, regardless of imported credential envelope.
    CodexChatGpt,
    /// Native Kiro execution.
    Kiro,
    /// A Claude-compatible endpoint that does not inherit Grok browser behavior.
    ClaudeCompatible,
    /// Another explicitly declared adapter using the conservative compatible baseline.
    OtherCompatible,
}

/// Whether an adapter may retain a Credential-to-egress association.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderStickinessCapability {
    /// The channel must not depend on a retained egress identity.
    None,
    /// The channel may retain an exact Credential-to-egress assignment when configured.
    Optional,
    /// The channel requires one exact sticky egress identity and fails closed when it is blocked.
    Required,
}

/// Exact, bounded identity of one Provider/Upstream/Endpoint channel namespace.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
#[allow(clippy::struct_field_names)] // The three exact identity components are intentionally explicit.
pub struct ProviderChannelIdentity {
    provider_id: ProviderId,
    upstream_id: UpstreamId,
    endpoint_id: EndpointId,
}

impl ProviderChannelIdentity {
    /// Creates one exact Provider/Channel namespace.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderEgressRuntimeError::InvalidIdentity`] when any opaque identifier is
    /// blank, unbounded, contains controls, or has surrounding whitespace.
    pub fn try_new(
        provider_id: ProviderId,
        upstream_id: UpstreamId,
        endpoint_id: EndpointId,
    ) -> Result<Self, ProviderEgressRuntimeError> {
        validate_identity(provider_id.as_str())?;
        validate_identity(upstream_id.as_str())?;
        validate_identity(endpoint_id.as_str())?;
        Ok(Self {
            provider_id,
            upstream_id,
            endpoint_id,
        })
    }

    /// Returns the provider implementation identity.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Returns the configured upstream instance identity.
    #[must_use]
    pub const fn upstream_id(&self) -> &UpstreamId {
        &self.upstream_id
    }

    /// Returns the protocol/channel Endpoint identity.
    #[must_use]
    pub const fn endpoint_id(&self) -> &EndpointId {
        &self.endpoint_id
    }
}

impl fmt::Debug for ProviderChannelIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderChannelIdentity")
            .field("provider_id", &self.provider_id)
            .field("upstream_id", &self.upstream_id)
            .field("endpoint_id", &self.endpoint_id)
            .finish()
    }
}

/// One explicit channel capability declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderChannelCapability {
    identity: ProviderChannelIdentity,
    channel: ProviderEgressChannel,
}

impl ProviderChannelCapability {
    /// Declares one closed behavior family for one exact Provider/Channel namespace.
    #[must_use]
    pub const fn new(identity: ProviderChannelIdentity, channel: ProviderEgressChannel) -> Self {
        Self { identity, channel }
    }

    /// Returns the exact namespace owning this capability.
    #[must_use]
    pub const fn identity(&self) -> &ProviderChannelIdentity {
        &self.identity
    }

    /// Returns the declared behavior family.
    #[must_use]
    pub const fn channel(&self) -> ProviderEgressChannel {
        self.channel
    }

    /// Returns the channel's sticky-egress capability.
    #[must_use]
    pub const fn stickiness(&self) -> ProviderStickinessCapability {
        match self.channel {
            ProviderEgressChannel::GrokWeb => ProviderStickinessCapability::Required,
            ProviderEgressChannel::GenericCompatible
            | ProviderEgressChannel::GrokBuild
            | ProviderEgressChannel::GrokConsole => ProviderStickinessCapability::Optional,
            ProviderEgressChannel::OfficialApi
            | ProviderEgressChannel::CodexChatGpt
            | ProviderEgressChannel::Kiro
            | ProviderEgressChannel::ClaudeCompatible
            | ProviderEgressChannel::OtherCompatible => ProviderStickinessCapability::None,
        }
    }

    /// Returns whether the channel owns a Provider-session state namespace.
    #[must_use]
    pub const fn supports_provider_session(&self) -> bool {
        matches!(
            self.channel,
            ProviderEgressChannel::GrokConsole | ProviderEgressChannel::GrokWeb
        )
    }

    /// Returns whether the channel owns a clearance state namespace.
    #[must_use]
    pub const fn supports_clearance(&self) -> bool {
        matches!(self.channel, ProviderEgressChannel::GrokWeb)
    }

    /// Returns the maximum hidden auxiliary HTTP calls before inference submission.
    #[must_use]
    pub const fn max_auxiliary_requests(&self) -> u8 {
        match self.channel {
            ProviderEgressChannel::GrokConsole => 2,
            ProviderEgressChannel::GrokWeb => 4,
            _ => 0,
        }
    }

    /// Returns the maximum explicit pre-submit recovery actions.
    #[must_use]
    pub const fn max_pre_submit_recoveries(&self) -> u8 {
        match self.channel {
            ProviderEgressChannel::GenericCompatible
            | ProviderEgressChannel::GrokBuild
            | ProviderEgressChannel::GrokConsole
            | ProviderEgressChannel::GrokWeb => 1,
            _ => 0,
        }
    }

    /// Returns whether a one-shot diagnostic may execute without hidden Provider HTTP.
    #[must_use]
    pub const fn supports_one_shot_diagnostic(&self) -> bool {
        !matches!(
            self.channel,
            ProviderEgressChannel::GrokConsole | ProviderEgressChannel::GrokWeb
        )
    }

    /// Classifies one sanitized failure using this exact channel's declared capabilities.
    ///
    /// # Errors
    ///
    /// Returns a closed error when session or clearance evidence is not declared by this channel.
    pub fn classify_failure(
        &self,
        evidence: ProviderEgressFailureEvidence,
    ) -> Result<ProviderEgressFailureDisposition, ProviderEgressRuntimeError> {
        classify_provider_egress_failure(self, evidence)
    }
}

/// Immutable bounded registry of explicit Provider/Channel capabilities.
#[derive(Clone, Debug)]
pub struct ProviderChannelCapabilityRegistry {
    entries: Arc<BTreeMap<ProviderChannelIdentity, ProviderChannelCapability>>,
}

impl ProviderChannelCapabilityRegistry {
    /// Builds an immutable registry without deriving capabilities from a Provider name, URL, or
    /// credential format.
    ///
    /// # Errors
    ///
    /// Returns a closed error for an empty/oversized registry or duplicate exact namespace.
    pub fn try_new(
        capabilities: Vec<ProviderChannelCapability>,
    ) -> Result<Self, ProviderEgressRuntimeError> {
        if capabilities.is_empty() {
            return Err(ProviderEgressRuntimeError::EmptyCapabilities);
        }
        if capabilities.len() > MAX_PROVIDER_EGRESS_CAPABILITIES {
            return Err(ProviderEgressRuntimeError::TooManyCapabilities);
        }
        let mut entries = BTreeMap::new();
        for capability in capabilities {
            let identity = capability.identity.clone();
            if entries.insert(identity, capability).is_some() {
                return Err(ProviderEgressRuntimeError::DuplicateCapability);
            }
        }
        Ok(Self {
            entries: Arc::new(entries),
        })
    }

    /// Returns the exact declared capability, if present.
    #[must_use]
    pub fn capability(
        &self,
        identity: &ProviderChannelIdentity,
    ) -> Option<&ProviderChannelCapability> {
        self.entries.get(identity)
    }

    /// Returns the bounded number of exact channel namespaces.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether no capability is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Exact direct or named egress identity. Named values are safe labels, not proxy endpoints.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderEgressTargetIdentity {
    /// The admitted direct transport profile.
    Direct,
    /// One fixed-proxy or pool-node identity within the owning Upstream.
    Named(String),
}

impl ProviderEgressTargetIdentity {
    /// Creates a bounded named identity.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderEgressRuntimeError::InvalidIdentity`] for an invalid label.
    pub fn named(value: impl Into<String>) -> Result<Self, ProviderEgressRuntimeError> {
        let value = value.into();
        validate_identity(&value)?;
        Ok(Self::Named(value))
    }

    /// Returns the named label; direct transport has no label.
    #[must_use]
    pub fn as_named(&self) -> Option<&str> {
        match self {
            Self::Direct => None,
            Self::Named(value) => Some(value),
        }
    }
}

/// State key for one exact Provider/Channel-owned egress identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProviderEgressStateKey {
    channel: ProviderChannelIdentity,
    target: ProviderEgressTargetIdentity,
}

impl ProviderEgressStateKey {
    /// Creates one exact egress state key.
    #[must_use]
    pub const fn new(
        channel: ProviderChannelIdentity,
        target: ProviderEgressTargetIdentity,
    ) -> Self {
        Self { channel, target }
    }

    /// Returns the owning Provider/Channel namespace.
    #[must_use]
    pub const fn channel(&self) -> &ProviderChannelIdentity {
        &self.channel
    }

    /// Returns the direct or named egress identity.
    #[must_use]
    pub const fn target(&self) -> &ProviderEgressTargetIdentity {
        &self.target
    }
}

/// State key for one exact Credential revision and Provider-session lineage.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProviderSessionStateKey {
    channel: ProviderChannelIdentity,
    credential_id: CredentialId,
    credential_revision: u64,
    session_revision: u64,
}

impl ProviderSessionStateKey {
    /// Creates one session key without retaining tokens, cookies, or key material.
    ///
    /// # Errors
    ///
    /// Returns a closed error for an invalid Credential identity or zero revision.
    pub fn try_new(
        channel: ProviderChannelIdentity,
        credential_id: CredentialId,
        credential_revision: u64,
        session_revision: u64,
    ) -> Result<Self, ProviderEgressRuntimeError> {
        validate_identity(credential_id.as_str())?;
        if credential_revision == 0 || session_revision == 0 {
            return Err(ProviderEgressRuntimeError::InvalidRevision);
        }
        Ok(Self {
            channel,
            credential_id,
            credential_revision,
            session_revision,
        })
    }

    /// Returns the owning Provider/Channel namespace.
    #[must_use]
    pub const fn channel(&self) -> &ProviderChannelIdentity {
        &self.channel
    }

    /// Returns the exact non-secret Credential identity.
    #[must_use]
    pub const fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the exact Credential revision.
    #[must_use]
    pub const fn credential_revision(&self) -> u64 {
        self.credential_revision
    }

    /// Returns the local Provider-session lineage revision.
    #[must_use]
    pub const fn session_revision(&self) -> u64 {
        self.session_revision
    }
}

/// State key for one exact clearance lineage bound to session and egress identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProviderClearanceStateKey {
    session: ProviderSessionStateKey,
    target: ProviderEgressTargetIdentity,
    clearance_revision: u64,
}

impl ProviderClearanceStateKey {
    /// Creates one value-free clearance key.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderEgressRuntimeError::InvalidRevision`] for revision zero.
    pub fn try_new(
        session: ProviderSessionStateKey,
        target: ProviderEgressTargetIdentity,
        clearance_revision: u64,
    ) -> Result<Self, ProviderEgressRuntimeError> {
        if clearance_revision == 0 {
            return Err(ProviderEgressRuntimeError::InvalidRevision);
        }
        Ok(Self {
            session,
            target,
            clearance_revision,
        })
    }

    /// Returns the exact owning session lineage.
    #[must_use]
    pub const fn session(&self) -> &ProviderSessionStateKey {
        &self.session
    }

    /// Returns the exact egress identity bound to the clearance.
    #[must_use]
    pub const fn target(&self) -> &ProviderEgressTargetIdentity {
        &self.target
    }

    /// Returns the local clearance lineage revision.
    #[must_use]
    pub const fn clearance_revision(&self) -> u64 {
        self.clearance_revision
    }
}

/// Effective local state for one exact egress identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderEgressRuntimeState {
    /// The exact egress identity may be considered by its owning channel.
    Available,
    /// The exact identity is cooling until the exclusive deadline.
    CoolingDown {
        /// Unix-millisecond exclusive cooldown deadline.
        until_ms: i64,
    },
    /// The exact identity's circuit is open until a probe becomes due.
    CircuitOpen {
        /// Earliest Unix-millisecond instant for one controlled probe.
        probe_due_at_ms: i64,
    },
    /// One controlled probe may be started by a later owner.
    ProbeDue,
    /// One controlled probe is in flight for this exact identity.
    ProbeInFlight {
        /// Exclusive ticket deadline.
        expires_at_ms: i64,
    },
    /// The exact egress identity is administratively disabled.
    Disabled,
}

impl ProviderEgressRuntimeState {
    /// Returns the deterministic effective state at one explicit timestamp.
    #[must_use]
    pub const fn at(self, now_ms: i64) -> Self {
        match self {
            Self::CoolingDown { until_ms } if until_ms <= now_ms => Self::Available,
            Self::CircuitOpen { probe_due_at_ms } if probe_due_at_ms <= now_ms => Self::ProbeDue,
            Self::ProbeInFlight { expires_at_ms } if expires_at_ms <= now_ms => Self::ProbeDue,
            other => other,
        }
    }

    /// Returns whether ordinary selection may use this exact identity.
    #[must_use]
    pub const fn is_available_at(self, now_ms: i64) -> bool {
        matches!(self.at(now_ms), Self::Available)
    }
}

/// Effective state of one exact Provider-session lineage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderSessionRuntimeState {
    /// No session has been established.
    Absent,
    /// The exact session is active until the exclusive deadline.
    Active {
        /// Unix-millisecond exclusive expiry deadline.
        expires_at_ms: i64,
    },
    /// The exact session expired.
    Expired,
    /// The exact session needs a Provider challenge flow before reuse.
    ChallengeRequired,
    /// The exact session lineage is invalid and must not be reused.
    Invalid,
}

impl ProviderSessionRuntimeState {
    /// Returns the deterministic effective state at one explicit timestamp.
    #[must_use]
    pub const fn at(self, now_ms: i64) -> Self {
        match self {
            Self::Active { expires_at_ms } if expires_at_ms <= now_ms => Self::Expired,
            other => other,
        }
    }
}

/// Effective state of one exact clearance lineage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderClearanceRuntimeState {
    /// No clearance has been established.
    Absent,
    /// The exact clearance remains fresh until the exclusive deadline.
    Fresh {
        /// Unix-millisecond exclusive expiry deadline.
        expires_at_ms: i64,
    },
    /// The exact clearance expired.
    Expired,
    /// An explicit bounded refresh is required.
    RefreshRequired,
    /// One bounded refresh owns this lineage until its exclusive deadline.
    RefreshInFlight {
        /// Exclusive refresh-ticket deadline.
        expires_at_ms: i64,
    },
    /// The exact clearance lineage is invalid and must not be reused.
    Invalid,
}

impl ProviderClearanceRuntimeState {
    /// Returns the deterministic effective state at one explicit timestamp.
    #[must_use]
    pub const fn at(self, now_ms: i64) -> Self {
        match self {
            Self::Fresh { expires_at_ms } if expires_at_ms <= now_ms => Self::Expired,
            Self::RefreshInFlight { expires_at_ms } if expires_at_ms <= now_ms => {
                Self::RefreshRequired
            }
            other => other,
        }
    }
}

/// Bounded local state registry with independent egress, session, and clearance maps.
#[derive(Clone)]
pub struct ProviderEgressRuntime {
    capabilities: ProviderChannelCapabilityRegistry,
    state: Arc<RwLock<ProviderEgressRuntimeInner>>,
}

#[derive(Default)]
struct ProviderEgressRuntimeInner {
    egress: BTreeMap<ProviderEgressStateKey, ProviderEgressRuntimeState>,
    sessions: BTreeMap<ProviderSessionStateKey, ProviderSessionRuntimeState>,
    clearances: BTreeMap<ProviderClearanceStateKey, ProviderClearanceRuntimeState>,
}

impl ProviderEgressRuntime {
    /// Creates an empty local state registry backed by one immutable capability registry.
    #[must_use]
    pub fn new(capabilities: ProviderChannelCapabilityRegistry) -> Self {
        Self {
            capabilities,
            state: Arc::new(RwLock::new(ProviderEgressRuntimeInner::default())),
        }
    }

    /// Returns the immutable channel capability registry.
    #[must_use]
    pub const fn capabilities(&self) -> &ProviderChannelCapabilityRegistry {
        &self.capabilities
    }

    /// Sets one exact egress state after validating its explicit observation/deadline.
    ///
    /// # Errors
    ///
    /// Fails closed for an unknown channel, invalid time/deadline, capacity exhaustion, or poisoned
    /// state lock.
    pub fn set_egress_state(
        &self,
        key: ProviderEgressStateKey,
        state: ProviderEgressRuntimeState,
        observed_at_ms: i64,
    ) -> Result<(), ProviderEgressRuntimeError> {
        self.require_capability(key.channel())?;
        self.validate_target(key.channel(), key.target())?;
        validate_observation_time(observed_at_ms)?;
        validate_egress_state(state, observed_at_ms)?;
        let mut runtime = self
            .state
            .write()
            .map_err(|_| ProviderEgressRuntimeError::RuntimeUnavailable)?;
        insert_bounded(
            &mut runtime.egress,
            key,
            state,
            MAX_PROVIDER_EGRESS_STATES_PER_DOMAIN,
        )
    }

    /// Returns one exact egress state at the supplied deterministic time.
    ///
    /// # Errors
    ///
    /// Fails closed for an unknown channel/state, invalid time, or poisoned state lock.
    pub fn egress_state_at(
        &self,
        key: &ProviderEgressStateKey,
        now_ms: i64,
    ) -> Result<ProviderEgressRuntimeState, ProviderEgressRuntimeError> {
        self.require_capability(key.channel())?;
        self.validate_target(key.channel(), key.target())?;
        validate_observation_time(now_ms)?;
        let runtime = self
            .state
            .read()
            .map_err(|_| ProviderEgressRuntimeError::RuntimeUnavailable)?;
        runtime
            .egress
            .get(key)
            .copied()
            .map(|state| state.at(now_ms))
            .ok_or(ProviderEgressRuntimeError::UnknownEgressState)
    }

    /// Requires that one exact egress identity is available without rotating to a sibling.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderEgressRuntimeError::EgressUnavailable`] for every retained non-available
    /// state; unknown state remains a distinct fail-closed error.
    pub fn require_exact_egress_available(
        &self,
        key: &ProviderEgressStateKey,
        now_ms: i64,
    ) -> Result<(), ProviderEgressRuntimeError> {
        if self.egress_state_at(key, now_ms)?.is_available_at(now_ms) {
            Ok(())
        } else {
            Err(ProviderEgressRuntimeError::EgressUnavailable)
        }
    }

    /// Sets one exact Provider-session state for a channel that declared session support.
    ///
    /// # Errors
    ///
    /// Fails closed for unsupported session state, invalid time/deadline, capacity exhaustion, or
    /// poisoned state.
    pub fn set_session_state(
        &self,
        key: ProviderSessionStateKey,
        state: ProviderSessionRuntimeState,
        observed_at_ms: i64,
    ) -> Result<(), ProviderEgressRuntimeError> {
        let capability = self.require_capability(key.channel())?;
        if !capability.supports_provider_session() {
            return Err(ProviderEgressRuntimeError::ProviderSessionUnsupported);
        }
        validate_observation_time(observed_at_ms)?;
        validate_session_state(state, observed_at_ms)?;
        let mut runtime = self
            .state
            .write()
            .map_err(|_| ProviderEgressRuntimeError::RuntimeUnavailable)?;
        insert_bounded(
            &mut runtime.sessions,
            key,
            state,
            MAX_PROVIDER_EGRESS_STATES_PER_DOMAIN,
        )
    }

    /// Returns one exact Provider-session state at the supplied deterministic time.
    ///
    /// # Errors
    ///
    /// Fails closed for an unknown/unsupported session, invalid time, or poisoned state.
    pub fn session_state_at(
        &self,
        key: &ProviderSessionStateKey,
        now_ms: i64,
    ) -> Result<ProviderSessionRuntimeState, ProviderEgressRuntimeError> {
        let capability = self.require_capability(key.channel())?;
        if !capability.supports_provider_session() {
            return Err(ProviderEgressRuntimeError::ProviderSessionUnsupported);
        }
        validate_observation_time(now_ms)?;
        let runtime = self
            .state
            .read()
            .map_err(|_| ProviderEgressRuntimeError::RuntimeUnavailable)?;
        runtime
            .sessions
            .get(key)
            .copied()
            .map(|state| state.at(now_ms))
            .ok_or(ProviderEgressRuntimeError::UnknownSessionState)
    }

    /// Sets one exact clearance state for a channel that declared clearance support.
    ///
    /// # Errors
    ///
    /// Fails closed for unsupported clearance state, invalid time/deadline, capacity exhaustion,
    /// or poisoned state.
    pub fn set_clearance_state(
        &self,
        key: ProviderClearanceStateKey,
        state: ProviderClearanceRuntimeState,
        observed_at_ms: i64,
    ) -> Result<(), ProviderEgressRuntimeError> {
        let capability = self.require_capability(key.session().channel())?;
        self.validate_target(key.session().channel(), key.target())?;
        if !capability.supports_clearance() {
            return Err(ProviderEgressRuntimeError::ClearanceUnsupported);
        }
        validate_observation_time(observed_at_ms)?;
        validate_clearance_state(state, observed_at_ms)?;
        let mut runtime = self
            .state
            .write()
            .map_err(|_| ProviderEgressRuntimeError::RuntimeUnavailable)?;
        insert_bounded(
            &mut runtime.clearances,
            key,
            state,
            MAX_PROVIDER_EGRESS_STATES_PER_DOMAIN,
        )
    }

    /// Returns one exact clearance state at the supplied deterministic time.
    ///
    /// # Errors
    ///
    /// Fails closed for an unknown/unsupported clearance, invalid time, or poisoned state.
    pub fn clearance_state_at(
        &self,
        key: &ProviderClearanceStateKey,
        now_ms: i64,
    ) -> Result<ProviderClearanceRuntimeState, ProviderEgressRuntimeError> {
        let capability = self.require_capability(key.session().channel())?;
        self.validate_target(key.session().channel(), key.target())?;
        if !capability.supports_clearance() {
            return Err(ProviderEgressRuntimeError::ClearanceUnsupported);
        }
        validate_observation_time(now_ms)?;
        let runtime = self
            .state
            .read()
            .map_err(|_| ProviderEgressRuntimeError::RuntimeUnavailable)?;
        runtime
            .clearances
            .get(key)
            .copied()
            .map(|state| state.at(now_ms))
            .ok_or(ProviderEgressRuntimeError::UnknownClearanceState)
    }

    fn require_capability(
        &self,
        identity: &ProviderChannelIdentity,
    ) -> Result<&ProviderChannelCapability, ProviderEgressRuntimeError> {
        self.capabilities
            .capability(identity)
            .ok_or(ProviderEgressRuntimeError::UnknownChannelCapability)
    }

    fn validate_target(
        &self,
        identity: &ProviderChannelIdentity,
        target: &ProviderEgressTargetIdentity,
    ) -> Result<(), ProviderEgressRuntimeError> {
        let capability = self.require_capability(identity)?;
        if capability.stickiness() == ProviderStickinessCapability::Required
            && matches!(target, ProviderEgressTargetIdentity::Direct)
        {
            return Err(ProviderEgressRuntimeError::StickyEgressRequired);
        }
        Ok(())
    }
}

impl fmt::Debug for ProviderEgressRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderEgressRuntime")
            .field("capability_count", &self.capabilities.len())
            .field("state", &"<value-free runtime state>")
            .finish()
    }
}

/// Account evidence accepted by the bounded 403 classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderAccountEvidence {
    /// No independently validated account-level evidence exists.
    None,
    /// A separate exact-account signal confirmed the active revision is forbidden.
    ConfirmedForbidden,
}

/// Sanitized failure evidence supplied by a Provider adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderEgressFailureEvidence {
    /// DNS/TLS/connect/proxy handshake failed before inference submission.
    PreSubmitEgress,
    /// The exact credential or session was explicitly unauthorized/expired.
    CredentialUnauthorized,
    /// The exact credential/model quota target was limited.
    QuotaLimited,
    /// A 403 was observed with only separately established account evidence.
    HttpForbidden {
        /// Independent account evidence; raw response text is never accepted.
        account_evidence: ProviderAccountEvidence,
    },
    /// The exact declared Provider session expired or became invalid.
    SessionInvalid,
    /// The exact declared clearance requires a bounded refresh.
    ClearanceChallenge,
    /// Protocol conversion, decoder, or Canonical lifecycle failed.
    AdapterProtocol,
    /// A failure occurred after the first semantic event.
    PostSemantic,
}

/// Exact state domain that owns one sanitized failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderEgressFailureOwner {
    /// Exact Provider-owned egress node/profile.
    Egress,
    /// Exact Credential/account revision.
    Credential,
    /// Exact Credential/model quota target.
    Quota,
    /// Ambiguous Provider/egress evidence that cannot mutate an account.
    AmbiguousProvider,
    /// Exact declared Provider-session lineage.
    Session,
    /// Exact declared clearance lineage.
    Clearance,
    /// Adapter/protocol implementation.
    AdapterProtocol,
    /// Terminal request/Provider outcome after semantic output.
    RequestOutcome,
}

/// Sole bounded recovery action allowed by a sanitized failure classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderEgressRecoveryAction {
    /// No local recovery may be inferred.
    None,
    /// Cool or open the circuit for the exact egress identity.
    CoolExactEgress,
    /// Require replacement/reauthorization of only the exact Credential revision.
    RequireCredentialReplacement,
    /// Cool only the exact quota target.
    CoolExactQuota,
    /// Rebuild only the exact declared Provider-session lineage.
    RebuildExactSession,
    /// Refresh only the exact declared clearance lineage.
    RefreshExactClearance,
    /// Fail the request without mutating egress/session/clearance state.
    FailRequest,
}

/// Value-free failure owner and sole permitted local action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderEgressFailureDisposition {
    owner: ProviderEgressFailureOwner,
    action: ProviderEgressRecoveryAction,
}

impl ProviderEgressFailureDisposition {
    /// Returns the exact state domain that owns the observation.
    #[must_use]
    pub const fn owner(self) -> ProviderEgressFailureOwner {
        self.owner
    }

    /// Returns the only allowed local recovery action.
    #[must_use]
    pub const fn action(self) -> ProviderEgressRecoveryAction {
        self.action
    }
}

/// Classifies value-free Provider evidence under one explicit channel capability.
///
/// # Errors
///
/// Fails closed when session or clearance evidence is supplied to a channel that did not declare
/// that capability.
pub fn classify_provider_egress_failure(
    capability: &ProviderChannelCapability,
    evidence: ProviderEgressFailureEvidence,
) -> Result<ProviderEgressFailureDisposition, ProviderEgressRuntimeError> {
    let (owner, action) = match evidence {
        ProviderEgressFailureEvidence::PreSubmitEgress => (
            ProviderEgressFailureOwner::Egress,
            ProviderEgressRecoveryAction::CoolExactEgress,
        ),
        ProviderEgressFailureEvidence::CredentialUnauthorized
        | ProviderEgressFailureEvidence::HttpForbidden {
            account_evidence: ProviderAccountEvidence::ConfirmedForbidden,
        } => (
            ProviderEgressFailureOwner::Credential,
            ProviderEgressRecoveryAction::RequireCredentialReplacement,
        ),
        ProviderEgressFailureEvidence::QuotaLimited => (
            ProviderEgressFailureOwner::Quota,
            ProviderEgressRecoveryAction::CoolExactQuota,
        ),
        ProviderEgressFailureEvidence::HttpForbidden {
            account_evidence: ProviderAccountEvidence::None,
        } => (
            ProviderEgressFailureOwner::AmbiguousProvider,
            ProviderEgressRecoveryAction::None,
        ),
        ProviderEgressFailureEvidence::SessionInvalid => {
            if !capability.supports_provider_session() {
                return Err(ProviderEgressRuntimeError::ProviderSessionUnsupported);
            }
            (
                ProviderEgressFailureOwner::Session,
                ProviderEgressRecoveryAction::RebuildExactSession,
            )
        }
        ProviderEgressFailureEvidence::ClearanceChallenge => {
            if !capability.supports_clearance() {
                return Err(ProviderEgressRuntimeError::ClearanceUnsupported);
            }
            (
                ProviderEgressFailureOwner::Clearance,
                ProviderEgressRecoveryAction::RefreshExactClearance,
            )
        }
        ProviderEgressFailureEvidence::AdapterProtocol => (
            ProviderEgressFailureOwner::AdapterProtocol,
            ProviderEgressRecoveryAction::FailRequest,
        ),
        ProviderEgressFailureEvidence::PostSemantic => (
            ProviderEgressFailureOwner::RequestOutcome,
            ProviderEgressRecoveryAction::None,
        ),
    };
    Ok(ProviderEgressFailureDisposition { owner, action })
}

/// Local ledger that makes hidden auxiliary HTTP and recovery bounds auditable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderTransportAttemptBudget {
    maximum_auxiliary_requests: u8,
    maximum_pre_submit_recoveries: u8,
    auxiliary_requests: u8,
    pre_submit_recoveries: u8,
    inference_submitted: bool,
    semantic_event_observed: bool,
}

impl ProviderTransportAttemptBudget {
    /// Creates a fresh logical-attempt budget from one explicit channel capability.
    #[must_use]
    pub const fn for_capability(capability: &ProviderChannelCapability) -> Self {
        Self {
            maximum_auxiliary_requests: capability.max_auxiliary_requests(),
            maximum_pre_submit_recoveries: capability.max_pre_submit_recoveries(),
            auxiliary_requests: 0,
            pre_submit_recoveries: 0,
            inference_submitted: false,
            semantic_event_observed: false,
        }
    }

    /// Records one hidden Provider auxiliary HTTP submission before inference.
    ///
    /// # Errors
    ///
    /// Fails closed after inference/semantic output or when the declared finite bound is exhausted.
    pub fn record_auxiliary_request(&mut self) -> Result<(), ProviderTransportAttemptBudgetError> {
        self.require_pre_submit()?;
        if self.auxiliary_requests >= self.maximum_auxiliary_requests {
            return Err(ProviderTransportAttemptBudgetError::AuxiliaryRequestLimit);
        }
        self.auxiliary_requests = self.auxiliary_requests.saturating_add(1);
        Ok(())
    }

    /// Records one explicit pre-submit recovery action.
    ///
    /// # Errors
    ///
    /// Fails closed after inference/semantic output or when the declared finite bound is exhausted.
    pub fn record_pre_submit_recovery(
        &mut self,
    ) -> Result<(), ProviderTransportAttemptBudgetError> {
        self.require_pre_submit()?;
        if self.pre_submit_recoveries >= self.maximum_pre_submit_recoveries {
            return Err(ProviderTransportAttemptBudgetError::RecoveryLimit);
        }
        self.pre_submit_recoveries = self.pre_submit_recoveries.saturating_add(1);
        Ok(())
    }

    /// Records the sole inference submission for this logical attempt.
    ///
    /// # Errors
    ///
    /// Rejects a second submission or any submission after a semantic event.
    pub fn record_inference_submission(
        &mut self,
    ) -> Result<(), ProviderTransportAttemptBudgetError> {
        if self.semantic_event_observed {
            return Err(ProviderTransportAttemptBudgetError::SemanticEventClosed);
        }
        if self.inference_submitted {
            return Err(ProviderTransportAttemptBudgetError::InferenceAlreadySubmitted);
        }
        self.inference_submitted = true;
        Ok(())
    }

    /// Irreversibly closes auxiliary/recovery/replay after the first semantic event.
    pub fn observe_semantic_event(&mut self) {
        self.semantic_event_observed = true;
    }

    /// Returns the counted hidden auxiliary HTTP submissions.
    #[must_use]
    pub const fn auxiliary_requests(&self) -> u8 {
        self.auxiliary_requests
    }

    /// Returns the counted pre-submit recovery actions.
    #[must_use]
    pub const fn pre_submit_recoveries(&self) -> u8 {
        self.pre_submit_recoveries
    }

    /// Returns whether inference was submitted once.
    #[must_use]
    pub const fn inference_submitted(&self) -> bool {
        self.inference_submitted
    }

    /// Returns whether a semantic event permanently closed recovery/replay.
    #[must_use]
    pub const fn semantic_event_observed(&self) -> bool {
        self.semantic_event_observed
    }

    fn require_pre_submit(&self) -> Result<(), ProviderTransportAttemptBudgetError> {
        if self.semantic_event_observed {
            return Err(ProviderTransportAttemptBudgetError::SemanticEventClosed);
        }
        if self.inference_submitted {
            return Err(ProviderTransportAttemptBudgetError::InferenceAlreadySubmitted);
        }
        Ok(())
    }
}

/// Closed local attempt-ledger failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderTransportAttemptBudgetError {
    /// The channel's hidden auxiliary HTTP bound was exhausted or zero.
    AuxiliaryRequestLimit,
    /// The channel's explicit pre-submit recovery bound was exhausted or zero.
    RecoveryLimit,
    /// Inference was already submitted; auxiliary/recovery/replay is closed.
    InferenceAlreadySubmitted,
    /// A semantic event was observed; every replay/recovery path is closed.
    SemanticEventClosed,
}

impl fmt::Display for ProviderTransportAttemptBudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::AuxiliaryRequestLimit => "provider auxiliary request bound is exhausted",
            Self::RecoveryLimit => "provider pre-submit recovery bound is exhausted",
            Self::InferenceAlreadySubmitted => "provider inference was already submitted",
            Self::SemanticEventClosed => "provider semantic event closed recovery",
        };
        formatter.write_str(message)
    }
}

impl Error for ProviderTransportAttemptBudgetError {}

/// Closed capability, identity, or local runtime-state failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderEgressRuntimeError {
    /// No Provider/Channel capability was declared.
    EmptyCapabilities,
    /// The runtime capability count exceeded its finite bound.
    TooManyCapabilities,
    /// The exact Provider/Channel namespace was declared twice.
    DuplicateCapability,
    /// An opaque identity was blank, unbounded, contained controls, or had surrounding whitespace.
    InvalidIdentity,
    /// A Credential/session/clearance revision was zero.
    InvalidRevision,
    /// An explicit observation time was negative.
    InvalidObservationTime,
    /// A retained deadline was not strictly after its observation time.
    InvalidDeadline,
    /// No capability exists for the exact Provider/Channel namespace.
    UnknownChannelCapability,
    /// The channel did not declare Provider-session support.
    ProviderSessionUnsupported,
    /// The channel did not declare clearance support.
    ClearanceUnsupported,
    /// No exact egress state exists.
    UnknownEgressState,
    /// No exact Provider-session state exists.
    UnknownSessionState,
    /// No exact clearance state exists.
    UnknownClearanceState,
    /// The exact egress state is not currently available.
    EgressUnavailable,
    /// This channel requires a named sticky egress identity; direct transport is not admissible.
    StickyEgressRequired,
    /// One independent state domain reached its finite retained-entry bound.
    StateCapacityExceeded,
    /// The local runtime state lock was poisoned.
    RuntimeUnavailable,
}

impl fmt::Display for ProviderEgressRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyCapabilities => "provider egress capability registry is empty",
            Self::TooManyCapabilities => "provider egress capability count exceeds its bound",
            Self::DuplicateCapability => "provider egress capability is duplicated",
            Self::InvalidIdentity => "provider egress identity is invalid",
            Self::InvalidRevision => "provider egress revision is invalid",
            Self::InvalidObservationTime => "provider egress observation time is invalid",
            Self::InvalidDeadline => "provider egress state deadline is invalid",
            Self::UnknownChannelCapability => "provider channel capability is unknown",
            Self::ProviderSessionUnsupported => "provider channel session capability is disabled",
            Self::ClearanceUnsupported => "provider channel clearance capability is disabled",
            Self::UnknownEgressState => "provider egress state is unknown",
            Self::UnknownSessionState => "provider session state is unknown",
            Self::UnknownClearanceState => "provider clearance state is unknown",
            Self::EgressUnavailable => "provider egress identity is unavailable",
            Self::StickyEgressRequired => "provider channel requires a named sticky egress",
            Self::StateCapacityExceeded => "provider egress state capacity is exhausted",
            Self::RuntimeUnavailable => "provider egress runtime is unavailable",
        };
        formatter.write_str(message)
    }
}

impl Error for ProviderEgressRuntimeError {}

fn insert_bounded<K: Ord, V>(
    entries: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    maximum_entries: usize,
) -> Result<(), ProviderEgressRuntimeError> {
    if !entries.contains_key(&key) && entries.len() >= maximum_entries {
        return Err(ProviderEgressRuntimeError::StateCapacityExceeded);
    }
    entries.insert(key, value);
    Ok(())
}

fn validate_identity(value: &str) -> Result<(), ProviderEgressRuntimeError> {
    if value.is_empty()
        || value.len() > MAX_PROVIDER_EGRESS_IDENTITY_LENGTH
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(ProviderEgressRuntimeError::InvalidIdentity);
    }
    Ok(())
}

const fn validate_observation_time(now_ms: i64) -> Result<(), ProviderEgressRuntimeError> {
    if now_ms < 0 {
        Err(ProviderEgressRuntimeError::InvalidObservationTime)
    } else {
        Ok(())
    }
}

const fn validate_deadline(
    deadline_ms: i64,
    observed_at_ms: i64,
) -> Result<(), ProviderEgressRuntimeError> {
    if deadline_ms <= observed_at_ms {
        Err(ProviderEgressRuntimeError::InvalidDeadline)
    } else {
        Ok(())
    }
}

const fn validate_egress_state(
    state: ProviderEgressRuntimeState,
    observed_at_ms: i64,
) -> Result<(), ProviderEgressRuntimeError> {
    match state {
        ProviderEgressRuntimeState::CoolingDown { until_ms } => {
            validate_deadline(until_ms, observed_at_ms)
        }
        ProviderEgressRuntimeState::CircuitOpen { probe_due_at_ms } => {
            validate_deadline(probe_due_at_ms, observed_at_ms)
        }
        ProviderEgressRuntimeState::ProbeInFlight { expires_at_ms } => {
            validate_deadline(expires_at_ms, observed_at_ms)
        }
        ProviderEgressRuntimeState::Available
        | ProviderEgressRuntimeState::ProbeDue
        | ProviderEgressRuntimeState::Disabled => Ok(()),
    }
}

const fn validate_session_state(
    state: ProviderSessionRuntimeState,
    observed_at_ms: i64,
) -> Result<(), ProviderEgressRuntimeError> {
    match state {
        ProviderSessionRuntimeState::Active { expires_at_ms } => {
            validate_deadline(expires_at_ms, observed_at_ms)
        }
        ProviderSessionRuntimeState::Absent
        | ProviderSessionRuntimeState::Expired
        | ProviderSessionRuntimeState::ChallengeRequired
        | ProviderSessionRuntimeState::Invalid => Ok(()),
    }
}

const fn validate_clearance_state(
    state: ProviderClearanceRuntimeState,
    observed_at_ms: i64,
) -> Result<(), ProviderEgressRuntimeError> {
    match state {
        ProviderClearanceRuntimeState::Fresh { expires_at_ms }
        | ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms } => {
            validate_deadline(expires_at_ms, observed_at_ms)
        }
        ProviderClearanceRuntimeState::Absent
        | ProviderClearanceRuntimeState::Expired
        | ProviderClearanceRuntimeState::RefreshRequired
        | ProviderClearanceRuntimeState::Invalid => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use gateway_core::{CredentialId, EndpointId, ProviderId, UpstreamId};

    use super::{
        ProviderAccountEvidence, ProviderChannelCapability, ProviderChannelCapabilityRegistry,
        ProviderChannelIdentity, ProviderClearanceRuntimeState, ProviderClearanceStateKey,
        ProviderEgressChannel, ProviderEgressFailureEvidence, ProviderEgressFailureOwner,
        ProviderEgressRecoveryAction, ProviderEgressRuntime, ProviderEgressRuntimeError,
        ProviderEgressRuntimeState, ProviderEgressStateKey, ProviderEgressTargetIdentity,
        ProviderSessionRuntimeState, ProviderSessionStateKey, ProviderStickinessCapability,
        ProviderTransportAttemptBudget, ProviderTransportAttemptBudgetError,
        classify_provider_egress_failure,
    };

    fn identity(
        provider: &str,
        upstream: &str,
        endpoint: &str,
    ) -> Result<ProviderChannelIdentity, Box<dyn std::error::Error>> {
        Ok(ProviderChannelIdentity::try_new(
            ProviderId::try_new(provider)?,
            UpstreamId::try_new(upstream)?,
            EndpointId::try_new(endpoint)?,
        )?)
    }

    fn capability(
        provider: &str,
        upstream: &str,
        endpoint: &str,
        channel: ProviderEgressChannel,
    ) -> Result<ProviderChannelCapability, Box<dyn std::error::Error>> {
        Ok(ProviderChannelCapability::new(
            identity(provider, upstream, endpoint)?,
            channel,
        ))
    }

    #[test]
    fn capability_matrix_is_explicit_and_conservative() -> Result<(), Box<dyn std::error::Error>> {
        let generic = capability(
            "generic",
            "relay-a",
            "responses",
            ProviderEgressChannel::GenericCompatible,
        )?;
        let build = capability("grok", "grok-a", "build", ProviderEgressChannel::GrokBuild)?;
        let console = capability(
            "grok",
            "grok-a",
            "console",
            ProviderEgressChannel::GrokConsole,
        )?;
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let official = capability(
            "official",
            "official-a",
            "responses",
            ProviderEgressChannel::OfficialApi,
        )?;
        let codex = capability(
            "openai",
            "chatgpt-a",
            "codex",
            ProviderEgressChannel::CodexChatGpt,
        )?;
        let kiro = capability("kiro", "kiro-a", "messages", ProviderEgressChannel::Kiro)?;
        let claude = capability(
            "claude-compatible",
            "claude-a",
            "messages",
            ProviderEgressChannel::ClaudeCompatible,
        )?;
        let other = capability(
            "other",
            "other-a",
            "chat",
            ProviderEgressChannel::OtherCompatible,
        )?;

        assert_eq!(generic.stickiness(), ProviderStickinessCapability::Optional);
        assert_eq!(build.stickiness(), ProviderStickinessCapability::Optional);
        assert_eq!(generic.max_pre_submit_recoveries(), 1);
        assert_eq!(build.max_pre_submit_recoveries(), 1);
        assert!(console.supports_provider_session());
        assert!(!console.supports_clearance());
        assert_eq!(console.max_auxiliary_requests(), 2);
        assert_eq!(console.max_pre_submit_recoveries(), 1);
        assert_eq!(web.stickiness(), ProviderStickinessCapability::Required);
        assert!(web.supports_provider_session());
        assert!(web.supports_clearance());
        assert_eq!(web.max_auxiliary_requests(), 4);
        assert!(!web.supports_one_shot_diagnostic());
        for ordinary in [&official, &codex, &kiro, &claude, &other] {
            assert_eq!(ordinary.stickiness(), ProviderStickinessCapability::None);
            assert!(!ordinary.supports_provider_session());
            assert!(!ordinary.supports_clearance());
            assert_eq!(ordinary.max_auxiliary_requests(), 0);
            assert!(ordinary.supports_one_shot_diagnostic());
        }
        Ok(())
    }

    #[test]
    fn capability_registry_rejects_duplicates_and_unbounded_identities()
    -> Result<(), Box<dyn std::error::Error>> {
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        assert!(matches!(
            ProviderChannelCapabilityRegistry::try_new(Vec::new()),
            Err(ProviderEgressRuntimeError::EmptyCapabilities)
        ));
        assert!(matches!(
            ProviderChannelCapabilityRegistry::try_new(vec![web.clone(), web]),
            Err(ProviderEgressRuntimeError::DuplicateCapability)
        ));
        assert_eq!(
            ProviderChannelIdentity::try_new(
                ProviderId::try_new(" grok")?,
                UpstreamId::try_new("grok-a")?,
                EndpointId::try_new("web")?,
            ),
            Err(ProviderEgressRuntimeError::InvalidIdentity)
        );
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn exact_namespaces_and_state_domains_cannot_cross_mutate()
    -> Result<(), Box<dyn std::error::Error>> {
        let build = capability("grok", "grok-a", "build", ProviderEgressChannel::GrokBuild)?;
        let console = capability(
            "grok",
            "grok-a",
            "console",
            ProviderEgressChannel::GrokConsole,
        )?;
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let relay = capability(
            "generic",
            "relay-a",
            "responses",
            ProviderEgressChannel::GenericCompatible,
        )?;
        let runtime =
            ProviderEgressRuntime::new(ProviderChannelCapabilityRegistry::try_new(vec![
                build.clone(),
                console.clone(),
                web.clone(),
                relay.clone(),
            ])?);
        let shared_node = ProviderEgressTargetIdentity::named("node-01")?;
        let web_egress = ProviderEgressStateKey::new(web.identity().clone(), shared_node.clone());
        let console_egress =
            ProviderEgressStateKey::new(console.identity().clone(), shared_node.clone());
        let build_egress =
            ProviderEgressStateKey::new(build.identity().clone(), shared_node.clone());
        let relay_egress = ProviderEgressStateKey::new(relay.identity().clone(), shared_node);
        runtime.set_egress_state(
            web_egress.clone(),
            ProviderEgressRuntimeState::CoolingDown { until_ms: 200 },
            100,
        )?;
        runtime.set_egress_state(
            console_egress.clone(),
            ProviderEgressRuntimeState::Available,
            100,
        )?;
        runtime.set_egress_state(
            build_egress.clone(),
            ProviderEgressRuntimeState::Disabled,
            100,
        )?;
        runtime.set_egress_state(
            relay_egress.clone(),
            ProviderEgressRuntimeState::ProbeDue,
            100,
        )?;

        let credential_id = CredentialId::try_new("shared-account")?;
        let console_session = ProviderSessionStateKey::try_new(
            console.identity().clone(),
            credential_id.clone(),
            1,
            1,
        )?;
        let web_session =
            ProviderSessionStateKey::try_new(web.identity().clone(), credential_id, 1, 1)?;
        runtime.set_session_state(
            console_session.clone(),
            ProviderSessionRuntimeState::Active { expires_at_ms: 300 },
            100,
        )?;
        runtime.set_session_state(
            web_session.clone(),
            ProviderSessionRuntimeState::ChallengeRequired,
            100,
        )?;
        let web_clearance = ProviderClearanceStateKey::try_new(
            web_session,
            ProviderEgressTargetIdentity::named("node-01")?,
            1,
        )?;
        runtime.set_clearance_state(
            web_clearance.clone(),
            ProviderClearanceRuntimeState::RefreshRequired,
            100,
        )?;

        assert_eq!(
            runtime.egress_state_at(&web_egress, 150)?,
            ProviderEgressRuntimeState::CoolingDown { until_ms: 200 }
        );
        assert_eq!(
            runtime.egress_state_at(&console_egress, 150)?,
            ProviderEgressRuntimeState::Available
        );
        assert_eq!(
            runtime.egress_state_at(&build_egress, 150)?,
            ProviderEgressRuntimeState::Disabled
        );
        assert_eq!(
            runtime.egress_state_at(&relay_egress, 150)?,
            ProviderEgressRuntimeState::ProbeDue
        );
        assert_eq!(
            runtime.session_state_at(&console_session, 150)?,
            ProviderSessionRuntimeState::Active { expires_at_ms: 300 }
        );
        assert_eq!(
            runtime.clearance_state_at(&web_clearance, 150)?,
            ProviderClearanceRuntimeState::RefreshRequired
        );
        assert_eq!(
            runtime.set_session_state(
                ProviderSessionStateKey::try_new(
                    relay.identity().clone(),
                    CredentialId::try_new("relay-key")?,
                    1,
                    1,
                )?,
                ProviderSessionRuntimeState::Absent,
                100,
            ),
            Err(ProviderEgressRuntimeError::ProviderSessionUnsupported)
        );
        Ok(())
    }

    #[test]
    fn deterministic_deadlines_and_sticky_loss_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let runtime =
            ProviderEgressRuntime::new(ProviderChannelCapabilityRegistry::try_new(vec![
                web.clone(),
            ])?);
        let egress = ProviderEgressStateKey::new(
            web.identity().clone(),
            ProviderEgressTargetIdentity::named("sticky-node")?,
        );
        let direct = ProviderEgressStateKey::new(
            web.identity().clone(),
            ProviderEgressTargetIdentity::Direct,
        );
        assert_eq!(
            runtime.set_egress_state(direct, ProviderEgressRuntimeState::Available, 100),
            Err(ProviderEgressRuntimeError::StickyEgressRequired)
        );
        runtime.set_egress_state(
            egress.clone(),
            ProviderEgressRuntimeState::CircuitOpen {
                probe_due_at_ms: 200,
            },
            100,
        )?;
        assert_eq!(
            runtime.egress_state_at(&egress, 199)?,
            ProviderEgressRuntimeState::CircuitOpen {
                probe_due_at_ms: 200
            }
        );
        assert_eq!(
            runtime.egress_state_at(&egress, 200)?,
            ProviderEgressRuntimeState::ProbeDue
        );
        assert_eq!(
            runtime.require_exact_egress_available(&egress, 200),
            Err(ProviderEgressRuntimeError::EgressUnavailable)
        );

        runtime.set_egress_state(
            egress.clone(),
            ProviderEgressRuntimeState::CoolingDown { until_ms: 300 },
            200,
        )?;
        assert_eq!(
            runtime.egress_state_at(&egress, 300)?,
            ProviderEgressRuntimeState::Available
        );

        let session = ProviderSessionStateKey::try_new(
            web.identity().clone(),
            CredentialId::try_new("web-account")?,
            2,
            3,
        )?;
        runtime.set_session_state(
            session.clone(),
            ProviderSessionRuntimeState::Active { expires_at_ms: 400 },
            300,
        )?;
        assert_eq!(
            runtime.session_state_at(&session, 400)?,
            ProviderSessionRuntimeState::Expired
        );
        let clearance = ProviderClearanceStateKey::try_new(
            session,
            ProviderEgressTargetIdentity::named("sticky-node")?,
            5,
        )?;
        runtime.set_clearance_state(
            clearance.clone(),
            ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms: 500 },
            400,
        )?;
        assert_eq!(
            runtime.clearance_state_at(&clearance, 500)?,
            ProviderClearanceRuntimeState::RefreshRequired
        );
        Ok(())
    }

    #[test]
    fn failure_ownership_preserves_ambiguous_and_confirmed_403_boundaries()
    -> Result<(), Box<dyn std::error::Error>> {
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let generic = capability(
            "generic",
            "relay-a",
            "responses",
            ProviderEgressChannel::GenericCompatible,
        )?;
        let ambiguous = web.classify_failure(ProviderEgressFailureEvidence::HttpForbidden {
            account_evidence: ProviderAccountEvidence::None,
        })?;
        assert_eq!(
            ambiguous.owner(),
            ProviderEgressFailureOwner::AmbiguousProvider
        );
        assert_eq!(ambiguous.action(), ProviderEgressRecoveryAction::None);

        let confirmed = classify_provider_egress_failure(
            &web,
            ProviderEgressFailureEvidence::HttpForbidden {
                account_evidence: ProviderAccountEvidence::ConfirmedForbidden,
            },
        )?;
        assert_eq!(confirmed.owner(), ProviderEgressFailureOwner::Credential);
        assert_eq!(
            confirmed.action(),
            ProviderEgressRecoveryAction::RequireCredentialReplacement
        );
        assert_eq!(
            classify_provider_egress_failure(
                &generic,
                ProviderEgressFailureEvidence::ClearanceChallenge,
            ),
            Err(ProviderEgressRuntimeError::ClearanceUnsupported)
        );
        assert_eq!(
            classify_provider_egress_failure(
                &generic,
                ProviderEgressFailureEvidence::SessionInvalid,
            ),
            Err(ProviderEgressRuntimeError::ProviderSessionUnsupported)
        );
        assert_eq!(
            classify_provider_egress_failure(&web, ProviderEgressFailureEvidence::PostSemantic,)?
                .owner(),
            ProviderEgressFailureOwner::RequestOutcome
        );
        Ok(())
    }

    #[test]
    fn hidden_auxiliary_and_recovery_calls_are_bounded_before_semantic_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let generic = capability(
            "generic",
            "relay-a",
            "responses",
            ProviderEgressChannel::GenericCompatible,
        )?;
        let console = capability(
            "grok",
            "grok-a",
            "console",
            ProviderEgressChannel::GrokConsole,
        )?;
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;

        let mut generic_budget = ProviderTransportAttemptBudget::for_capability(&generic);
        assert_eq!(
            generic_budget.record_auxiliary_request(),
            Err(ProviderTransportAttemptBudgetError::AuxiliaryRequestLimit)
        );
        assert_eq!(generic_budget.record_pre_submit_recovery(), Ok(()));
        assert_eq!(
            generic_budget.record_pre_submit_recovery(),
            Err(ProviderTransportAttemptBudgetError::RecoveryLimit)
        );

        let mut console_budget = ProviderTransportAttemptBudget::for_capability(&console);
        console_budget.record_auxiliary_request()?;
        console_budget.record_auxiliary_request()?;
        assert_eq!(console_budget.auxiliary_requests(), 2);
        assert_eq!(
            console_budget.record_auxiliary_request(),
            Err(ProviderTransportAttemptBudgetError::AuxiliaryRequestLimit)
        );
        console_budget.record_pre_submit_recovery()?;
        assert_eq!(
            console_budget.record_pre_submit_recovery(),
            Err(ProviderTransportAttemptBudgetError::RecoveryLimit)
        );
        console_budget.record_inference_submission()?;
        assert_eq!(
            console_budget.record_auxiliary_request(),
            Err(ProviderTransportAttemptBudgetError::InferenceAlreadySubmitted)
        );
        console_budget.observe_semantic_event();
        assert_eq!(
            console_budget.record_pre_submit_recovery(),
            Err(ProviderTransportAttemptBudgetError::SemanticEventClosed)
        );

        let mut web_budget = ProviderTransportAttemptBudget::for_capability(&web);
        for _ in 0..4 {
            web_budget.record_auxiliary_request()?;
        }
        assert_eq!(web_budget.auxiliary_requests(), 4);
        assert_eq!(
            web_budget.record_auxiliary_request(),
            Err(ProviderTransportAttemptBudgetError::AuxiliaryRequestLimit)
        );
        Ok(())
    }

    #[test]
    fn synthetic_fixture_performs_zero_provider_dns_store_or_proxy_calls()
    -> Result<(), Box<dyn std::error::Error>> {
        #[allow(clippy::struct_field_names)]
        struct RejectingFakeTransport {
            provider: AtomicUsize,
            dns: AtomicUsize,
            store: AtomicUsize,
            proxy: AtomicUsize,
        }

        let fake = RejectingFakeTransport {
            provider: AtomicUsize::new(0),
            dns: AtomicUsize::new(0),
            store: AtomicUsize::new(0),
            proxy: AtomicUsize::new(0),
        };
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let runtime =
            ProviderEgressRuntime::new(ProviderChannelCapabilityRegistry::try_new(vec![
                web.clone(),
            ])?);
        let key = ProviderEgressStateKey::new(
            web.identity().clone(),
            ProviderEgressTargetIdentity::named("fake-node")?,
        );
        runtime.set_egress_state(key.clone(), ProviderEgressRuntimeState::Available, 1)?;
        runtime.require_exact_egress_available(&key, 1)?;
        assert_eq!(fake.provider.load(Ordering::Relaxed), 0);
        assert_eq!(fake.dns.load(Ordering::Relaxed), 0);
        assert_eq!(fake.store.load(Ordering::Relaxed), 0);
        assert_eq!(fake.proxy.load(Ordering::Relaxed), 0);
        assert!(!format!("{runtime:?}").contains("fake-node"));
        Ok(())
    }
}
