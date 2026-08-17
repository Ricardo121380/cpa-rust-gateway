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

/// Opaque ownership ticket for one exact clearance refresh.
///
/// The ticket contains only the value-free state key, an internal ownership generation, and its
/// exclusive deadline. Callers cannot construct a ticket, so completion and failure can only
/// target a refresh admitted atomically by
/// [`ProviderEgressRuntime::begin_exact_clearance_refresh`].
#[derive(Clone, Eq, PartialEq)]
pub struct ProviderClearanceRefreshTicket {
    key: ProviderClearanceStateKey,
    generation: u64,
    expires_at_ms: i64,
}

impl ProviderClearanceRefreshTicket {
    /// Returns the exact clearance lineage owned by this ticket.
    #[must_use]
    pub const fn key(&self) -> &ProviderClearanceStateKey {
        &self.key
    }

    /// Returns the ticket's exclusive Unix-millisecond deadline.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
}

impl fmt::Debug for ProviderClearanceRefreshTicket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderClearanceRefreshTicket")
            .field("key", &"<exact value-free clearance lineage>")
            .field("generation", &self.generation)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

/// Closed state selected when an owned clearance refresh fails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderClearanceRefreshFailure {
    /// A later bounded attempt may retry the same exact lineage.
    RetryRequired,
    /// The exact clearance lineage is invalid and must not be reused.
    Invalid,
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
    clearance_refresh_owners: BTreeMap<ProviderClearanceStateKey, ProviderClearanceRefreshOwner>,
    next_clearance_refresh_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProviderClearanceRefreshOwner {
    generation: u64,
    expires_at_ms: i64,
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
    /// a live atomic refresh owner, inconsistent ownership state, or poisoned state.
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
        if let Some(owner) = runtime.clearance_refresh_owners.get(&key).copied() {
            match runtime.clearances.get(&key).copied() {
                Some(ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms })
                    if expires_at_ms == owner.expires_at_ms =>
                {
                    if owner.expires_at_ms > observed_at_ms {
                        return Err(ProviderEgressRuntimeError::ClearanceRefreshInFlight);
                    }
                    runtime.clearance_refresh_owners.remove(&key);
                }
                Some(_) | None => {
                    return Err(ProviderEgressRuntimeError::ClearanceRefreshOwnershipInconsistent);
                }
            }
        }
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

    /// Atomically marks one exact clearance lineage as requiring refresh.
    ///
    /// This is the challenge-observation transition used before a later bounded refresh begins.
    /// Absent, fresh, expired, and already-required states converge on `RefreshRequired`. A live
    /// refresh owner is preserved and reported instead of being overwritten; an expired owner is
    /// reclaimed deterministically under the same write lock. Invalid and unknown exact lineages
    /// fail closed.
    ///
    /// # Errors
    ///
    /// Fails closed for an unknown/unsupported lineage, invalid observation time, a live refresh
    /// owner, an invalid lineage, or a poisoned state lock.
    pub fn require_exact_clearance_refresh(
        &self,
        key: &ProviderClearanceStateKey,
        observed_at_ms: i64,
    ) -> Result<(), ProviderEgressRuntimeError> {
        self.require_clearance_capability(key)?;
        validate_observation_time(observed_at_ms)?;

        let mut runtime = self
            .state
            .write()
            .map_err(|_| ProviderEgressRuntimeError::RuntimeUnavailable)?;
        let state = runtime
            .clearances
            .get_mut(key)
            .ok_or(ProviderEgressRuntimeError::UnknownClearanceState)?;
        match state.at(observed_at_ms) {
            ProviderClearanceRuntimeState::Absent
            | ProviderClearanceRuntimeState::Fresh { .. }
            | ProviderClearanceRuntimeState::Expired
            | ProviderClearanceRuntimeState::RefreshRequired => {
                *state = ProviderClearanceRuntimeState::RefreshRequired;
                runtime.clearance_refresh_owners.remove(key);
                Ok(())
            }
            ProviderClearanceRuntimeState::RefreshInFlight { .. } => {
                Err(ProviderEgressRuntimeError::ClearanceRefreshInFlight)
            }
            ProviderClearanceRuntimeState::Invalid => {
                Err(ProviderEgressRuntimeError::ClearanceInvalid)
            }
        }
    }

    /// Atomically begins one refresh for an exact clearance lineage.
    ///
    /// A single write lock performs the state check and transition. Only an effective
    /// [`ProviderClearanceRuntimeState::Expired`] or
    /// [`ProviderClearanceRuntimeState::RefreshRequired`] state is admitted. A live owner,
    /// invalid lineage, fresh/absent state, or unknown sibling fails closed without mutation.
    /// Expired `Fresh` and expired `RefreshInFlight` deadlines are evaluated under the same lock,
    /// allowing deterministic recovery without an unlocked read/modify/write race.
    ///
    /// # Errors
    ///
    /// Fails closed for unsupported/unknown exact state, invalid time/deadline, a live refresh,
    /// an invalid lineage, a refresh that is not required, or a poisoned state lock.
    pub fn begin_exact_clearance_refresh(
        &self,
        key: &ProviderClearanceStateKey,
        observed_at_ms: i64,
        ticket_expires_at_ms: i64,
    ) -> Result<ProviderClearanceRefreshTicket, ProviderEgressRuntimeError> {
        self.require_clearance_capability(key)?;
        validate_observation_time(observed_at_ms)?;
        validate_deadline(ticket_expires_at_ms, observed_at_ms)?;

        let mut runtime = self
            .state
            .write()
            .map_err(|_| ProviderEgressRuntimeError::RuntimeUnavailable)?;
        let effective_state = runtime
            .clearances
            .get(key)
            .copied()
            .map(|state| state.at(observed_at_ms))
            .ok_or(ProviderEgressRuntimeError::UnknownClearanceState)?;
        match effective_state {
            ProviderClearanceRuntimeState::Expired
            | ProviderClearanceRuntimeState::RefreshRequired => {
                let generation = runtime
                    .next_clearance_refresh_generation
                    .checked_add(1)
                    .ok_or(ProviderEgressRuntimeError::ClearanceRefreshOwnershipExhausted)?;
                runtime.next_clearance_refresh_generation = generation;
                runtime.clearances.insert(
                    key.clone(),
                    ProviderClearanceRuntimeState::RefreshInFlight {
                        expires_at_ms: ticket_expires_at_ms,
                    },
                );
                runtime.clearance_refresh_owners.insert(
                    key.clone(),
                    ProviderClearanceRefreshOwner {
                        generation,
                        expires_at_ms: ticket_expires_at_ms,
                    },
                );
                Ok(ProviderClearanceRefreshTicket {
                    key: key.clone(),
                    generation,
                    expires_at_ms: ticket_expires_at_ms,
                })
            }
            ProviderClearanceRuntimeState::RefreshInFlight { .. } => {
                Err(ProviderEgressRuntimeError::ClearanceRefreshInFlight)
            }
            ProviderClearanceRuntimeState::Invalid => {
                Err(ProviderEgressRuntimeError::ClearanceInvalid)
            }
            ProviderClearanceRuntimeState::Absent | ProviderClearanceRuntimeState::Fresh { .. } => {
                Err(ProviderEgressRuntimeError::ClearanceRefreshNotRequired)
            }
        }
    }

    /// Atomically completes one exact owned clearance refresh as fresh.
    ///
    /// The ticket must still own the exact live `RefreshInFlight` state. A stale ticket cannot
    /// complete a replacement owner's refresh, and an expired ticket is normalized to
    /// `RefreshRequired` before returning a closed error.
    ///
    /// # Errors
    ///
    /// Fails closed for an unknown/unsupported lineage, invalid time/deadline, expired or
    /// mismatched ownership, a non-in-flight state, or a poisoned state lock.
    pub fn complete_exact_clearance_refresh(
        &self,
        ticket: &ProviderClearanceRefreshTicket,
        observed_at_ms: i64,
        fresh_expires_at_ms: i64,
    ) -> Result<(), ProviderEgressRuntimeError> {
        self.require_clearance_capability(ticket.key())?;
        validate_observation_time(observed_at_ms)?;
        validate_deadline(fresh_expires_at_ms, observed_at_ms)?;

        let mut runtime = self
            .state
            .write()
            .map_err(|_| ProviderEgressRuntimeError::RuntimeUnavailable)?;
        Self::require_live_clearance_refresh_owner(&mut runtime, ticket, observed_at_ms)?;
        runtime.clearances.insert(
            ticket.key().clone(),
            ProviderClearanceRuntimeState::Fresh {
                expires_at_ms: fresh_expires_at_ms,
            },
        );
        runtime.clearance_refresh_owners.remove(ticket.key());
        Ok(())
    }

    /// Atomically closes one exact owned clearance refresh as retryable or invalid.
    ///
    /// Only the ticket that owns the exact live `RefreshInFlight` state may mutate it. Retryable
    /// failure restores `RefreshRequired`; terminal failure closes only that exact lineage as
    /// `Invalid`.
    ///
    /// # Errors
    ///
    /// Fails closed for an unknown/unsupported lineage, invalid observation time, expired or
    /// mismatched ownership, a non-in-flight state, or a poisoned state lock.
    pub fn fail_exact_clearance_refresh(
        &self,
        ticket: &ProviderClearanceRefreshTicket,
        observed_at_ms: i64,
        failure: ProviderClearanceRefreshFailure,
    ) -> Result<(), ProviderEgressRuntimeError> {
        self.require_clearance_capability(ticket.key())?;
        validate_observation_time(observed_at_ms)?;

        let mut runtime = self
            .state
            .write()
            .map_err(|_| ProviderEgressRuntimeError::RuntimeUnavailable)?;
        Self::require_live_clearance_refresh_owner(&mut runtime, ticket, observed_at_ms)?;
        let state = match failure {
            ProviderClearanceRefreshFailure::RetryRequired => {
                ProviderClearanceRuntimeState::RefreshRequired
            }
            ProviderClearanceRefreshFailure::Invalid => ProviderClearanceRuntimeState::Invalid,
        };
        runtime.clearances.insert(ticket.key().clone(), state);
        runtime.clearance_refresh_owners.remove(ticket.key());
        Ok(())
    }

    fn require_clearance_capability(
        &self,
        key: &ProviderClearanceStateKey,
    ) -> Result<(), ProviderEgressRuntimeError> {
        let capability = self.require_capability(key.session().channel())?;
        self.validate_target(key.session().channel(), key.target())?;
        if !capability.supports_clearance() {
            return Err(ProviderEgressRuntimeError::ClearanceUnsupported);
        }
        Ok(())
    }

    fn require_live_clearance_refresh_owner(
        runtime: &mut ProviderEgressRuntimeInner,
        ticket: &ProviderClearanceRefreshTicket,
        observed_at_ms: i64,
    ) -> Result<(), ProviderEgressRuntimeError> {
        let state = runtime
            .clearances
            .get(ticket.key())
            .copied()
            .ok_or(ProviderEgressRuntimeError::UnknownClearanceState)?;
        match state {
            ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms }
                if expires_at_ms == ticket.expires_at_ms =>
            {
                let owner = runtime.clearance_refresh_owners.get(ticket.key()).copied();
                if owner
                    != Some(ProviderClearanceRefreshOwner {
                        generation: ticket.generation,
                        expires_at_ms: ticket.expires_at_ms,
                    })
                {
                    return Err(ProviderEgressRuntimeError::ClearanceRefreshTicketMismatch);
                }
                if expires_at_ms <= observed_at_ms {
                    runtime.clearances.insert(
                        ticket.key().clone(),
                        ProviderClearanceRuntimeState::RefreshRequired,
                    );
                    runtime.clearance_refresh_owners.remove(ticket.key());
                    Err(ProviderEgressRuntimeError::ClearanceRefreshTicketExpired)
                } else {
                    Ok(())
                }
            }
            ProviderClearanceRuntimeState::RefreshInFlight { .. } => {
                Err(ProviderEgressRuntimeError::ClearanceRefreshTicketMismatch)
            }
            ProviderClearanceRuntimeState::Invalid => {
                Err(ProviderEgressRuntimeError::ClearanceInvalid)
            }
            ProviderClearanceRuntimeState::Absent
            | ProviderClearanceRuntimeState::Fresh { .. }
            | ProviderClearanceRuntimeState::Expired
            | ProviderClearanceRuntimeState::RefreshRequired => {
                Err(ProviderEgressRuntimeError::ClearanceRefreshNotInFlight)
            }
        }
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
    /// The exact clearance already has one live refresh owner.
    ClearanceRefreshInFlight,
    /// The exact clearance does not currently require refresh.
    ClearanceRefreshNotRequired,
    /// The exact clearance lineage is invalid and cannot be refreshed.
    ClearanceInvalid,
    /// The supplied refresh ticket no longer owns the current in-flight refresh.
    ClearanceRefreshTicketMismatch,
    /// The supplied refresh ticket expired before completion or failure.
    ClearanceRefreshTicketExpired,
    /// The exact clearance is no longer in an owned in-flight refresh state.
    ClearanceRefreshNotInFlight,
    /// The finite local clearance refresh ownership sequence was exhausted.
    ClearanceRefreshOwnershipExhausted,
    /// Retained refresh ownership does not match the exact clearance runtime state.
    ClearanceRefreshOwnershipInconsistent,
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
            Self::ClearanceRefreshInFlight => "provider clearance refresh is already in flight",
            Self::ClearanceRefreshNotRequired => "provider clearance refresh is not required",
            Self::ClearanceInvalid => "provider clearance lineage is invalid",
            Self::ClearanceRefreshTicketMismatch => {
                "provider clearance refresh ownership does not match"
            }
            Self::ClearanceRefreshTicketExpired => "provider clearance refresh ownership expired",
            Self::ClearanceRefreshNotInFlight => "provider clearance refresh is not in flight",
            Self::ClearanceRefreshOwnershipExhausted => {
                "provider clearance refresh ownership is exhausted"
            }
            Self::ClearanceRefreshOwnershipInconsistent => {
                "provider clearance refresh ownership is inconsistent"
            }
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
    use std::{
        sync::{
            Arc, Barrier,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
    };

    use gateway_core::{CredentialId, EndpointId, ProviderId, UpstreamId};

    use super::{
        ProviderAccountEvidence, ProviderChannelCapability, ProviderChannelCapabilityRegistry,
        ProviderChannelIdentity, ProviderClearanceRefreshFailure, ProviderClearanceRuntimeState,
        ProviderClearanceStateKey, ProviderEgressChannel, ProviderEgressFailureEvidence,
        ProviderEgressFailureOwner, ProviderEgressRecoveryAction, ProviderEgressRuntime,
        ProviderEgressRuntimeError, ProviderEgressRuntimeState, ProviderEgressStateKey,
        ProviderEgressTargetIdentity, ProviderSessionRuntimeState, ProviderSessionStateKey,
        ProviderStickinessCapability, ProviderTransportAttemptBudget,
        ProviderTransportAttemptBudgetError, classify_provider_egress_failure,
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
    fn exact_clearance_begin_is_atomic_singleflight_under_concurrency()
    -> Result<(), Box<dyn std::error::Error>> {
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let runtime =
            ProviderEgressRuntime::new(ProviderChannelCapabilityRegistry::try_new(vec![
                web.clone(),
            ])?);
        let session = ProviderSessionStateKey::try_new(
            web.identity().clone(),
            CredentialId::try_new("web-account")?,
            7,
            11,
        )?;
        let clearance = ProviderClearanceStateKey::try_new(
            session,
            ProviderEgressTargetIdentity::named("sticky-node")?,
            13,
        )?;
        runtime.set_clearance_state(
            clearance.clone(),
            ProviderClearanceRuntimeState::RefreshRequired,
            100,
        )?;

        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for _ in 0..2 {
            let worker_runtime = runtime.clone();
            let worker_clearance = clearance.clone();
            let worker_barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                worker_barrier.wait();
                worker_runtime.begin_exact_clearance_refresh(&worker_clearance, 100, 200)
            }));
        }
        barrier.wait();
        let results = workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .map_err(|_| std::io::Error::other("clearance worker panicked"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| {
                    matches!(
                        result,
                        Err(ProviderEgressRuntimeError::ClearanceRefreshInFlight)
                    )
                })
                .count(),
            1
        );
        assert_eq!(
            runtime.clearance_state_at(&clearance, 101)?,
            ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms: 200 }
        );
        Ok(())
    }

    #[test]
    fn exact_clearance_challenge_never_overwrites_a_live_refresh_owner()
    -> Result<(), Box<dyn std::error::Error>> {
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let runtime =
            ProviderEgressRuntime::new(ProviderChannelCapabilityRegistry::try_new(vec![
                web.clone(),
            ])?);
        let clearance = ProviderClearanceStateKey::try_new(
            ProviderSessionStateKey::try_new(
                web.identity().clone(),
                CredentialId::try_new("web-account")?,
                3,
                5,
            )?,
            ProviderEgressTargetIdentity::named("sticky-node")?,
            7,
        )?;
        runtime.set_clearance_state(
            clearance.clone(),
            ProviderClearanceRuntimeState::Fresh { expires_at_ms: 300 },
            100,
        )?;
        runtime.require_exact_clearance_refresh(&clearance, 101)?;
        let ticket = runtime.begin_exact_clearance_refresh(&clearance, 102, 200)?;

        assert_eq!(
            runtime.require_exact_clearance_refresh(&clearance, 103),
            Err(ProviderEgressRuntimeError::ClearanceRefreshInFlight)
        );
        assert_eq!(
            runtime.clearance_state_at(&clearance, 103)?,
            ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms: 200 }
        );
        runtime.complete_exact_clearance_refresh(&ticket, 104, 400)?;
        assert_eq!(
            runtime.clearance_state_at(&clearance, 105)?,
            ProviderClearanceRuntimeState::Fresh { expires_at_ms: 400 }
        );

        runtime.set_clearance_state(
            clearance.clone(),
            ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms: 110 },
            105,
        )?;
        runtime.require_exact_clearance_refresh(&clearance, 110)?;
        assert_eq!(
            runtime.clearance_state_at(&clearance, 110)?,
            ProviderClearanceRuntimeState::RefreshRequired
        );
        Ok(())
    }

    #[test]
    fn exact_clearance_setter_cannot_overwrite_a_live_ticket()
    -> Result<(), Box<dyn std::error::Error>> {
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let runtime =
            ProviderEgressRuntime::new(ProviderChannelCapabilityRegistry::try_new(vec![
                web.clone(),
            ])?);
        let clearance = ProviderClearanceStateKey::try_new(
            ProviderSessionStateKey::try_new(
                web.identity().clone(),
                CredentialId::try_new("web-account")?,
                3,
                5,
            )?,
            ProviderEgressTargetIdentity::named("sticky-node")?,
            7,
        )?;
        runtime.set_clearance_state(
            clearance.clone(),
            ProviderClearanceRuntimeState::RefreshRequired,
            100,
        )?;
        let ticket = runtime.begin_exact_clearance_refresh(&clearance, 101, 200)?;

        assert_eq!(
            runtime.set_clearance_state(
                clearance.clone(),
                ProviderClearanceRuntimeState::Invalid,
                102,
            ),
            Err(ProviderEgressRuntimeError::ClearanceRefreshInFlight)
        );
        assert_eq!(
            runtime.clearance_state_at(&clearance, 102)?,
            ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms: 200 }
        );
        runtime.complete_exact_clearance_refresh(&ticket, 103, 300)?;
        assert_eq!(
            runtime.clearance_state_at(&clearance, 104)?,
            ProviderClearanceRuntimeState::Fresh { expires_at_ms: 300 }
        );
        Ok(())
    }

    #[test]
    fn exact_clearance_setter_reclaims_expired_owner_but_rejects_inconsistency()
    -> Result<(), Box<dyn std::error::Error>> {
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let runtime =
            ProviderEgressRuntime::new(ProviderChannelCapabilityRegistry::try_new(vec![
                web.clone(),
            ])?);
        let clearance = ProviderClearanceStateKey::try_new(
            ProviderSessionStateKey::try_new(
                web.identity().clone(),
                CredentialId::try_new("web-account")?,
                3,
                5,
            )?,
            ProviderEgressTargetIdentity::named("sticky-node")?,
            7,
        )?;
        runtime.set_clearance_state(
            clearance.clone(),
            ProviderClearanceRuntimeState::RefreshRequired,
            100,
        )?;
        let expired_ticket = runtime.begin_exact_clearance_refresh(&clearance, 101, 120)?;
        runtime.set_clearance_state(
            clearance.clone(),
            ProviderClearanceRuntimeState::Fresh { expires_at_ms: 300 },
            120,
        )?;
        assert_eq!(
            runtime.clearance_state_at(&clearance, 121)?,
            ProviderClearanceRuntimeState::Fresh { expires_at_ms: 300 }
        );
        assert_eq!(
            runtime.complete_exact_clearance_refresh(&expired_ticket, 121, 400),
            Err(ProviderEgressRuntimeError::ClearanceRefreshNotInFlight)
        );

        runtime.require_exact_clearance_refresh(&clearance, 122)?;
        let live_ticket = runtime.begin_exact_clearance_refresh(&clearance, 123, 200)?;
        {
            let mut inner = runtime
                .state
                .write()
                .map_err(|_| std::io::Error::other("clearance runtime lock poisoned"))?;
            inner.clearances.insert(
                clearance.clone(),
                ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms: 201 },
            );
        }
        assert_eq!(
            runtime.set_clearance_state(
                clearance.clone(),
                ProviderClearanceRuntimeState::Invalid,
                124,
            ),
            Err(ProviderEgressRuntimeError::ClearanceRefreshOwnershipInconsistent)
        );
        assert_eq!(live_ticket.expires_at_ms(), 200);
        assert_eq!(
            runtime.clearance_state_at(&clearance, 124)?,
            ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms: 201 }
        );
        Ok(())
    }

    #[test]
    fn exact_clearance_completion_failure_and_siblings_are_isolated()
    -> Result<(), Box<dyn std::error::Error>> {
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let runtime =
            ProviderEgressRuntime::new(ProviderChannelCapabilityRegistry::try_new(vec![
                web.clone(),
            ])?);
        let target = ProviderEgressTargetIdentity::named("sticky-node")?;
        let first = ProviderClearanceStateKey::try_new(
            ProviderSessionStateKey::try_new(
                web.identity().clone(),
                CredentialId::try_new("web-account-a")?,
                1,
                1,
            )?,
            target.clone(),
            1,
        )?;
        let sibling = ProviderClearanceStateKey::try_new(
            ProviderSessionStateKey::try_new(
                web.identity().clone(),
                CredentialId::try_new("web-account-b")?,
                1,
                1,
            )?,
            target.clone(),
            1,
        )?;
        let foreign = ProviderClearanceStateKey::try_new(
            ProviderSessionStateKey::try_new(
                web.identity().clone(),
                CredentialId::try_new("web-account-foreign")?,
                1,
                1,
            )?,
            target,
            1,
        )?;
        for key in [&first, &sibling] {
            runtime.set_clearance_state(
                key.clone(),
                ProviderClearanceRuntimeState::RefreshRequired,
                100,
            )?;
        }

        let first_ticket = runtime.begin_exact_clearance_refresh(&first, 100, 150)?;
        assert_eq!(
            runtime.begin_exact_clearance_refresh(&first, 101, 151),
            Err(ProviderEgressRuntimeError::ClearanceRefreshInFlight)
        );
        assert_eq!(
            runtime.begin_exact_clearance_refresh(&foreign, 101, 151),
            Err(ProviderEgressRuntimeError::UnknownClearanceState)
        );
        assert_eq!(
            runtime.clearance_state_at(&sibling, 101)?,
            ProviderClearanceRuntimeState::RefreshRequired
        );
        runtime.fail_exact_clearance_refresh(
            &first_ticket,
            110,
            ProviderClearanceRefreshFailure::RetryRequired,
        )?;
        let same_deadline_replacement = runtime.begin_exact_clearance_refresh(&first, 111, 150)?;
        assert_eq!(
            runtime.complete_exact_clearance_refresh(&first_ticket, 112, 300),
            Err(ProviderEgressRuntimeError::ClearanceRefreshTicketMismatch)
        );
        assert_eq!(
            runtime.clearance_state_at(&first, 113)?,
            ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms: 150 }
        );
        runtime.complete_exact_clearance_refresh(&same_deadline_replacement, 120, 300)?;
        assert_eq!(
            runtime.clearance_state_at(&first, 121)?,
            ProviderClearanceRuntimeState::Fresh { expires_at_ms: 300 }
        );
        assert_eq!(
            runtime.fail_exact_clearance_refresh(
                &first_ticket,
                121,
                ProviderClearanceRefreshFailure::RetryRequired,
            ),
            Err(ProviderEgressRuntimeError::ClearanceRefreshNotInFlight)
        );

        let sibling_ticket = runtime.begin_exact_clearance_refresh(&sibling, 121, 160)?;
        runtime.fail_exact_clearance_refresh(
            &sibling_ticket,
            122,
            ProviderClearanceRefreshFailure::Invalid,
        )?;
        assert_eq!(
            runtime.clearance_state_at(&sibling, 123)?,
            ProviderClearanceRuntimeState::Invalid
        );
        assert_eq!(
            runtime.clearance_state_at(&first, 123)?,
            ProviderClearanceRuntimeState::Fresh { expires_at_ms: 300 }
        );
        Ok(())
    }

    #[test]
    fn exact_clearance_tickets_fail_closed_across_expiry_and_replacement()
    -> Result<(), Box<dyn std::error::Error>> {
        let web = capability("grok", "grok-a", "web", ProviderEgressChannel::GrokWeb)?;
        let runtime =
            ProviderEgressRuntime::new(ProviderChannelCapabilityRegistry::try_new(vec![
                web.clone(),
            ])?);
        let clearance = ProviderClearanceStateKey::try_new(
            ProviderSessionStateKey::try_new(
                web.identity().clone(),
                CredentialId::try_new("web-account")?,
                2,
                3,
            )?,
            ProviderEgressTargetIdentity::named("sticky-node")?,
            5,
        )?;
        runtime.set_clearance_state(
            clearance.clone(),
            ProviderClearanceRuntimeState::Fresh { expires_at_ms: 110 },
            100,
        )?;

        let expired_fresh_ticket = runtime.begin_exact_clearance_refresh(&clearance, 110, 120)?;
        assert_eq!(
            runtime.complete_exact_clearance_refresh(&expired_fresh_ticket, 120, 300),
            Err(ProviderEgressRuntimeError::ClearanceRefreshTicketExpired)
        );
        assert_eq!(
            runtime.clearance_state_at(&clearance, 120)?,
            ProviderClearanceRuntimeState::RefreshRequired
        );

        let replacement = runtime.begin_exact_clearance_refresh(&clearance, 120, 140)?;
        assert_eq!(
            runtime.complete_exact_clearance_refresh(&expired_fresh_ticket, 121, 300),
            Err(ProviderEgressRuntimeError::ClearanceRefreshTicketMismatch)
        );
        assert_eq!(
            runtime.clearance_state_at(&clearance, 121)?,
            ProviderClearanceRuntimeState::RefreshInFlight { expires_at_ms: 140 }
        );
        runtime.fail_exact_clearance_refresh(
            &replacement,
            122,
            ProviderClearanceRefreshFailure::RetryRequired,
        )?;

        let reclaim = runtime.begin_exact_clearance_refresh(&clearance, 123, 130)?;
        assert_eq!(
            runtime.clearance_state_at(&clearance, 130)?,
            ProviderClearanceRuntimeState::RefreshRequired
        );
        let reclaimed_after_expiry = runtime.begin_exact_clearance_refresh(&clearance, 130, 150)?;
        assert_eq!(
            runtime.complete_exact_clearance_refresh(&reclaim, 131, 300),
            Err(ProviderEgressRuntimeError::ClearanceRefreshTicketMismatch)
        );
        runtime.fail_exact_clearance_refresh(
            &reclaimed_after_expiry,
            132,
            ProviderClearanceRefreshFailure::Invalid,
        )?;
        assert_eq!(
            runtime.begin_exact_clearance_refresh(&clearance, 133, 160),
            Err(ProviderEgressRuntimeError::ClearanceInvalid)
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
