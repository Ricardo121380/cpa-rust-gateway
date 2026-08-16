//! Bounded runtime transport-node composition for generic compatible endpoints.
//!
//! This module owns only local transport-profile selection and node capacity. It deliberately
//! does not resolve DNS, open a socket, classify Provider responses, refresh credentials, or
//! mutate the shared Credential Health/Quota registries. The serving path may later consume the
//! returned [`CompatibleEgressTransportLease`] and still has to call
//! `EgressPolicy::admit_url` immediately before dialing.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::{
        Arc, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use gateway_core::UpstreamId;
use gateway_upstream::{
    CompatibleEgressTarget, MAX_COMPATIBLE_EGRESS_LABEL_LENGTH, UpstreamProxy,
    UpstreamTransportProfile,
};

/// Maximum number of named proxy pools retained by one runtime composition registry.
pub const MAX_COMPATIBLE_EGRESS_PROXY_POOLS: usize = 128;
/// Maximum number of nodes in one named proxy pool.
pub const MAX_COMPATIBLE_EGRESS_NODES_PER_POOL: usize = 128;
/// Maximum number of all fixed/pool nodes in one registry.
pub const MAX_COMPATIBLE_EGRESS_TOTAL_NODES: usize = 512;
/// Maximum sum of node weights in one pool schedule.
pub const MAX_COMPATIBLE_EGRESS_SCHEDULE_SLOTS: usize = 1024;

/// Effective local availability of one fixed proxy or pool node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleEgressNodeAvailability {
    /// The node may accept a lease if capacity remains.
    Available,
    /// The node is excluded until this Unix-millisecond instant.
    CoolingDown {
        /// Exclusive cooldown deadline.
        until_ms: i64,
    },
    /// The node is administratively excluded.
    Disabled,
}

impl CompatibleEgressNodeAvailability {
    /// Returns the effective state at one explicit timestamp.
    #[must_use]
    pub const fn at(self, now_ms: i64) -> Self {
        match self {
            Self::CoolingDown { until_ms } if until_ms <= now_ms => Self::Available,
            other => other,
        }
    }

    /// Returns whether the node is eligible before checking capacity.
    #[must_use]
    pub const fn is_available_at(self, now_ms: i64) -> bool {
        matches!(self.at(now_ms), Self::Available)
    }
}

/// Aggregate target availability exposed by a value-free observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleEgressTargetAvailability {
    /// At least one node (or direct transport) is available and has capacity.
    Available,
    /// Every currently blocked node is cooling until the earliest retained deadline.
    CoolingDown {
        /// Earliest future cooldown deadline.
        until_ms: i64,
    },
    /// Nodes are healthy but all reached their local concurrency limit.
    Saturated,
    /// Every configured node is administratively disabled.
    Disabled,
}

impl CompatibleEgressTargetAvailability {
    /// Returns whether a target can accept a new local lease.
    #[must_use]
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// One bounded, non-secret node configuration for a proxy pool.
#[derive(Clone, Debug)]
pub struct CompatibleEgressNodeInput {
    /// Opaque node identity within its named pool.
    pub node_id: String,
    /// Existing validated transport profile. This slice requires local-DNS SOCKS5 for nodes.
    pub transport_profile: UpstreamTransportProfile,
    /// Smooth weighted selection weight.
    pub weight: usize,
    /// Maximum concurrent requests assigned to this node.
    pub maximum_concurrency: usize,
}

/// One bounded fixed-proxy configuration.
#[derive(Clone, Debug)]
pub struct CompatibleFixedProxyInput {
    /// Opaque fixed proxy profile identity, also used as its node identity.
    pub profile_id: String,
    /// Existing validated local-DNS SOCKS5 transport profile.
    pub transport_profile: UpstreamTransportProfile,
    /// Maximum concurrent requests assigned to this proxy.
    pub maximum_concurrency: usize,
}

/// One bounded named proxy-pool configuration.
#[derive(Clone, Debug)]
pub struct CompatibleProxyPoolInput {
    /// Opaque pool identity.
    pub pool_id: String,
    /// Bounded pool members.
    pub nodes: Vec<CompatibleEgressNodeInput>,
}

/// Complete input for one transport-node registry.
#[derive(Clone, Debug)]
pub struct CompatibleEgressTransportRegistryInput {
    /// Exact Upstream/Provider instance that owns every target and node state in this registry.
    pub owner_upstream_id: UpstreamId,
    /// Direct profile used by `CompatibleEgressTarget::Direct`.
    pub direct_profile: UpstreamTransportProfile,
    /// Named fixed proxies.
    pub fixed_proxies: Vec<CompatibleFixedProxyInput>,
    /// Named proxy pools.
    pub proxy_pools: Vec<CompatibleProxyPoolInput>,
}

/// A safe construction failure for the bounded transport registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleEgressTransportBuildError {
    /// An opaque profile, pool, or node label was empty.
    EmptyLabel,
    /// An opaque profile, pool, or node label exceeded the fixed bound.
    LabelTooLong,
    /// An opaque profile, pool, or node label contains controls or surrounding whitespace.
    InvalidLabelShape,
    /// A pool had no nodes.
    EmptyProxyPool,
    /// The registry exceeded its finite pool count.
    TooManyProxyPools,
    /// A pool exceeded its finite node count.
    TooManyProxyNodes,
    /// The registry exceeded its finite total node count.
    TooManyTotalNodes,
    /// A fixed profile or pool node identity was repeated.
    DuplicateTransportIdentity,
    /// A node weight was zero or made the bounded schedule too large.
    InvalidNodeWeight,
    /// A node concurrency limit was zero.
    InvalidNodeConcurrency,
    /// A profile intended for direct transport was not direct.
    DirectProfileMustBeDirect,
    /// A fixed/pool profile used a remote-DNS or direct proxy instead of local-DNS SOCKS5.
    ProxyProfileMustBeLocalDnsSocks5,
}

impl fmt::Display for CompatibleEgressTransportBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyLabel => "compatible egress transport label is empty",
            Self::LabelTooLong => "compatible egress transport label exceeds its bound",
            Self::InvalidLabelShape => "compatible egress transport label has an invalid shape",
            Self::EmptyProxyPool => "compatible egress proxy pool has no nodes",
            Self::TooManyProxyPools => "compatible egress proxy pool count exceeds its bound",
            Self::TooManyProxyNodes => "compatible egress proxy node count exceeds its bound",
            Self::TooManyTotalNodes => "compatible egress total node count exceeds its bound",
            Self::DuplicateTransportIdentity => {
                "compatible egress transport identity is duplicated"
            }
            Self::InvalidNodeWeight => "compatible egress node weight is invalid",
            Self::InvalidNodeConcurrency => "compatible egress node concurrency is invalid",
            Self::DirectProfileMustBeDirect => {
                "compatible direct target requires a direct transport profile"
            }
            Self::ProxyProfileMustBeLocalDnsSocks5 => {
                "compatible proxy target requires a local-DNS socks5 profile"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for CompatibleEgressTransportBuildError {}

/// A safe runtime lookup, capacity, or state-update failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibleEgressTransportError {
    /// The requested fixed proxy identity was not registered.
    UnknownFixedProxy,
    /// The requested proxy-pool identity was not registered.
    UnknownProxyPool,
    /// The requested node identity was not found in its target.
    UnknownProxyNode,
    /// Every node was cooling, disabled, or saturated.
    NoAvailableNode,
    /// The local node state lock was poisoned.
    RegistryUnavailable,
    /// A cooldown deadline was not strictly after the observation time.
    InvalidCooldownDeadline,
    /// Direct transport has no mutable node state.
    DirectTargetHasNoNode,
}

impl fmt::Display for CompatibleEgressTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnknownFixedProxy => "compatible fixed proxy is unknown",
            Self::UnknownProxyPool => "compatible proxy pool is unknown",
            Self::UnknownProxyNode => "compatible proxy node is unknown",
            Self::NoAvailableNode => "no compatible egress node is available",
            Self::RegistryUnavailable => "compatible egress node registry is unavailable",
            Self::InvalidCooldownDeadline => "compatible egress cooldown deadline is invalid",
            Self::DirectTargetHasNoNode => "direct compatible egress has no mutable node",
        };
        formatter.write_str(message)
    }
}

impl Error for CompatibleEgressTransportError {}

/// A value-free observation of one target's local capacity and node state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibleEgressTargetObservation {
    target: CompatibleEgressTarget,
    availability: CompatibleEgressTargetAvailability,
    available_nodes: usize,
    total_nodes: usize,
    active_leases: usize,
    maximum_concurrency: usize,
}

impl CompatibleEgressTargetObservation {
    /// Returns the target identity without any proxy secret.
    #[must_use]
    pub fn target(&self) -> &CompatibleEgressTarget {
        &self.target
    }

    /// Returns aggregate target availability.
    #[must_use]
    pub const fn availability(&self) -> CompatibleEgressTargetAvailability {
        self.availability
    }

    /// Returns the number of currently available nodes with capacity.
    #[must_use]
    pub const fn available_nodes(&self) -> usize {
        self.available_nodes
    }

    /// Returns the total configured node count. Direct targets report zero.
    #[must_use]
    pub const fn total_nodes(&self) -> usize {
        self.total_nodes
    }

    /// Returns the current aggregate active lease count.
    #[must_use]
    pub const fn active_leases(&self) -> usize {
        self.active_leases
    }

    /// Returns the aggregate node concurrency capacity.
    #[must_use]
    pub const fn maximum_concurrency(&self) -> usize {
        self.maximum_concurrency
    }
}

/// Immutable-plus-shared local transport-node registry.
#[derive(Clone)]
pub struct CompatibleEgressTransportRegistry {
    inner: Arc<CompatibleEgressTransportRegistryInner>,
}

impl CompatibleEgressTransportRegistry {
    /// Builds a bounded registry without opening a socket or resolving a proxy host.
    ///
    /// # Errors
    ///
    /// Returns a closed [`CompatibleEgressTransportBuildError`] for duplicate/unbounded input or
    /// for a proxy profile that cannot preserve the existing local-DNS address-pinning contract.
    pub fn try_new(
        input: CompatibleEgressTransportRegistryInput,
    ) -> Result<Self, CompatibleEgressTransportBuildError> {
        if !matches!(input.direct_profile.proxy(), UpstreamProxy::Direct) {
            return Err(CompatibleEgressTransportBuildError::DirectProfileMustBeDirect);
        }
        if input.proxy_pools.len() > MAX_COMPATIBLE_EGRESS_PROXY_POOLS {
            return Err(CompatibleEgressTransportBuildError::TooManyProxyPools);
        }

        let mut fixed = BTreeMap::new();
        let mut pools = BTreeMap::new();
        let mut total_nodes = 0_usize;
        for proxy in input.fixed_proxies {
            validate_label(&proxy.profile_id)?;
            validate_proxy_profile(&proxy.transport_profile)?;
            if proxy.maximum_concurrency == 0 {
                return Err(CompatibleEgressTransportBuildError::InvalidNodeConcurrency);
            }
            if fixed.contains_key(&proxy.profile_id) {
                return Err(CompatibleEgressTransportBuildError::DuplicateTransportIdentity);
            }
            total_nodes = total_nodes
                .checked_add(1)
                .ok_or(CompatibleEgressTransportBuildError::TooManyTotalNodes)?;
            if total_nodes > MAX_COMPATIBLE_EGRESS_TOTAL_NODES {
                return Err(CompatibleEgressTransportBuildError::TooManyTotalNodes);
            }
            fixed.insert(
                proxy.profile_id.clone(),
                Arc::new(EgressNodeSlot::new(
                    proxy.profile_id,
                    proxy.transport_profile,
                    proxy.maximum_concurrency,
                )),
            );
        }

        for pool in input.proxy_pools {
            validate_label(&pool.pool_id)?;
            if pool.nodes.is_empty() {
                return Err(CompatibleEgressTransportBuildError::EmptyProxyPool);
            }
            if pool.nodes.len() > MAX_COMPATIBLE_EGRESS_NODES_PER_POOL {
                return Err(CompatibleEgressTransportBuildError::TooManyProxyNodes);
            }
            if fixed.contains_key(&pool.pool_id) || pools.contains_key(&pool.pool_id) {
                return Err(CompatibleEgressTransportBuildError::DuplicateTransportIdentity);
            }

            let mut nodes = Vec::with_capacity(pool.nodes.len());
            let mut node_ids = BTreeSet::new();
            let mut weights = Vec::with_capacity(pool.nodes.len());
            for node in pool.nodes {
                validate_label(&node.node_id)?;
                validate_proxy_profile(&node.transport_profile)?;
                if node.weight == 0 {
                    return Err(CompatibleEgressTransportBuildError::InvalidNodeWeight);
                }
                if node.maximum_concurrency == 0 {
                    return Err(CompatibleEgressTransportBuildError::InvalidNodeConcurrency);
                }
                if !node_ids.insert(node.node_id.clone()) {
                    return Err(CompatibleEgressTransportBuildError::DuplicateTransportIdentity);
                }
                total_nodes = total_nodes
                    .checked_add(1)
                    .ok_or(CompatibleEgressTransportBuildError::TooManyTotalNodes)?;
                if total_nodes > MAX_COMPATIBLE_EGRESS_TOTAL_NODES {
                    return Err(CompatibleEgressTransportBuildError::TooManyTotalNodes);
                }
                weights.push(node.weight);
                nodes.push(Arc::new(EgressNodeSlot::new(
                    node.node_id,
                    node.transport_profile,
                    node.maximum_concurrency,
                )));
            }
            let schedule = weighted_schedule(&weights)?;
            pools.insert(
                pool.pool_id,
                Arc::new(EgressNodePool {
                    nodes,
                    schedule,
                    cursor: AtomicUsize::new(0),
                }),
            );
        }

        Ok(Self {
            inner: Arc::new(CompatibleEgressTransportRegistryInner {
                owner_upstream_id: input.owner_upstream_id,
                direct_profile: input.direct_profile,
                fixed,
                pools,
            }),
        })
    }

    /// Returns the exact Upstream/Provider instance that owns this registry.
    #[must_use]
    pub fn owner_upstream_id(&self) -> &UpstreamId {
        &self.inner.owner_upstream_id
    }

    /// Returns whether a target identity exists in this registry.
    #[must_use]
    pub fn contains_target(&self, target: &CompatibleEgressTarget) -> bool {
        match target {
            CompatibleEgressTarget::Direct => true,
            CompatibleEgressTarget::FixedProxy { profile_id } => {
                self.inner.fixed.contains_key(profile_id)
            }
            CompatibleEgressTarget::ProxyPool { pool_id } => self.inner.pools.contains_key(pool_id),
        }
    }

    /// Returns a value-free target/capacity observation at one explicit timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibleEgressTransportError`] when the target is unknown or its node state
    /// cannot be read.
    pub fn observe(
        &self,
        target: &CompatibleEgressTarget,
        now_ms: i64,
    ) -> Result<CompatibleEgressTargetObservation, CompatibleEgressTransportError> {
        match target {
            CompatibleEgressTarget::Direct => Ok(CompatibleEgressTargetObservation {
                target: target.clone(),
                availability: CompatibleEgressTargetAvailability::Available,
                available_nodes: 0,
                total_nodes: 0,
                active_leases: 0,
                maximum_concurrency: 0,
            }),
            CompatibleEgressTarget::FixedProxy { profile_id } => {
                let node = self
                    .inner
                    .fixed
                    .get(profile_id)
                    .ok_or(CompatibleEgressTransportError::UnknownFixedProxy)?;
                let observation = node.observe(now_ms)?;
                let availability = if observation.has_capacity {
                    CompatibleEgressTargetAvailability::Available
                } else {
                    match observation.availability {
                        CompatibleEgressNodeAvailability::CoolingDown { until_ms } => {
                            CompatibleEgressTargetAvailability::CoolingDown { until_ms }
                        }
                        CompatibleEgressNodeAvailability::Disabled => {
                            CompatibleEgressTargetAvailability::Disabled
                        }
                        CompatibleEgressNodeAvailability::Available => {
                            CompatibleEgressTargetAvailability::Saturated
                        }
                    }
                };
                Ok(CompatibleEgressTargetObservation {
                    target: target.clone(),
                    availability,
                    available_nodes: usize::from(observation.has_capacity),
                    total_nodes: 1,
                    active_leases: observation.active_leases,
                    maximum_concurrency: observation.maximum_concurrency,
                })
            }
            CompatibleEgressTarget::ProxyPool { pool_id } => {
                let pool = self
                    .inner
                    .pools
                    .get(pool_id)
                    .ok_or(CompatibleEgressTransportError::UnknownProxyPool)?;
                pool.observe(target.clone(), now_ms)
            }
        }
    }

    /// Acquires one target lease without network I/O.
    ///
    /// The returned lease owns one node-capacity reservation for fixed/pool targets. Direct
    /// targets carry no mutable node reservation.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibleEgressTransportError::NoAvailableNode`] when every candidate is
    /// cooling, disabled, or saturated.
    pub fn try_acquire(
        &self,
        target: &CompatibleEgressTarget,
        now_ms: i64,
    ) -> Result<CompatibleEgressTransportLease, CompatibleEgressTransportError> {
        match target {
            CompatibleEgressTarget::Direct => Ok(CompatibleEgressTransportLease::direct(
                target.clone(),
                self.inner.direct_profile.clone(),
            )),
            CompatibleEgressTarget::FixedProxy { profile_id } => {
                let node = self
                    .inner
                    .fixed
                    .get(profile_id)
                    .ok_or(CompatibleEgressTransportError::UnknownFixedProxy)?;
                acquire_node(target.clone(), Arc::clone(node), now_ms)
            }
            CompatibleEgressTarget::ProxyPool { pool_id } => {
                let pool = self
                    .inner
                    .pools
                    .get(pool_id)
                    .ok_or(CompatibleEgressTransportError::UnknownProxyPool)?;
                pool.try_acquire(target.clone(), now_ms)
            }
        }
    }

    /// Acquires one exact fixed/pool node without advancing a pool cursor.
    ///
    /// Serving uses this only for a process-local `CredentialAndEgress` sticky assignment. It
    /// never searches a sibling node and therefore cannot silently rotate an exact sticky
    /// binding across egress identities.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibleEgressTransportError::NoAvailableNode`] when the requested node is
    /// cooling, disabled, or saturated.
    pub fn try_acquire_exact(
        &self,
        target: &CompatibleEgressTarget,
        node_id: &str,
        now_ms: i64,
    ) -> Result<CompatibleEgressTransportLease, CompatibleEgressTransportError> {
        match target {
            CompatibleEgressTarget::Direct => {
                Err(CompatibleEgressTransportError::DirectTargetHasNoNode)
            }
            CompatibleEgressTarget::FixedProxy { profile_id } => {
                if profile_id != node_id {
                    return Err(CompatibleEgressTransportError::UnknownProxyNode);
                }
                let node = self
                    .inner
                    .fixed
                    .get(profile_id)
                    .ok_or(CompatibleEgressTransportError::UnknownFixedProxy)?;
                acquire_node(target.clone(), Arc::clone(node), now_ms)
            }
            CompatibleEgressTarget::ProxyPool { pool_id } => {
                let pool = self
                    .inner
                    .pools
                    .get(pool_id)
                    .ok_or(CompatibleEgressTransportError::UnknownProxyPool)?;
                pool.try_acquire_exact(target.clone(), node_id, now_ms)
            }
        }
    }

    /// Marks one fixed/pool node cooling until a future timestamp.
    ///
    /// This is a local failure-feedback primitive. It does not touch Credential Health/Quota and
    /// does not start a probe.
    ///
    /// # Errors
    ///
    /// Returns a closed error when the deadline is not future, the target/node is unknown, or a
    /// local state lock cannot be read.
    pub fn cool_down_until(
        &self,
        target: &CompatibleEgressTarget,
        node_id: &str,
        until_ms: i64,
        observed_at_ms: i64,
    ) -> Result<(), CompatibleEgressTransportError> {
        if until_ms <= observed_at_ms {
            return Err(CompatibleEgressTransportError::InvalidCooldownDeadline);
        }
        let node = self.node_for_target(target, node_id)?;
        let mut state = node
            .availability
            .write()
            .map_err(|_| CompatibleEgressTransportError::RegistryUnavailable)?;
        *state = CompatibleEgressNodeAvailability::CoolingDown { until_ms };
        Ok(())
    }

    /// Disables one fixed/pool node without changing Credential Health/Quota.
    ///
    /// # Errors
    ///
    /// Returns a closed error when the target/node is unknown or a local state lock cannot be
    /// acquired.
    pub fn disable(
        &self,
        target: &CompatibleEgressTarget,
        node_id: &str,
    ) -> Result<(), CompatibleEgressTransportError> {
        let node = self.node_for_target(target, node_id)?;
        let mut state = node
            .availability
            .write()
            .map_err(|_| CompatibleEgressTransportError::RegistryUnavailable)?;
        *state = CompatibleEgressNodeAvailability::Disabled;
        Ok(())
    }

    /// Restores one fixed/pool node to the available state.
    ///
    /// # Errors
    ///
    /// Returns a closed error when the target/node is unknown or a local state lock cannot be
    /// acquired.
    pub fn restore(
        &self,
        target: &CompatibleEgressTarget,
        node_id: &str,
    ) -> Result<(), CompatibleEgressTransportError> {
        let node = self.node_for_target(target, node_id)?;
        let mut state = node
            .availability
            .write()
            .map_err(|_| CompatibleEgressTransportError::RegistryUnavailable)?;
        *state = CompatibleEgressNodeAvailability::Available;
        Ok(())
    }

    fn node_for_target(
        &self,
        target: &CompatibleEgressTarget,
        node_id: &str,
    ) -> Result<Arc<EgressNodeSlot>, CompatibleEgressTransportError> {
        match target {
            CompatibleEgressTarget::Direct => {
                Err(CompatibleEgressTransportError::DirectTargetHasNoNode)
            }
            CompatibleEgressTarget::FixedProxy { profile_id } => {
                if node_id != profile_id {
                    return Err(CompatibleEgressTransportError::UnknownProxyNode);
                }
                self.inner
                    .fixed
                    .get(profile_id)
                    .cloned()
                    .ok_or(CompatibleEgressTransportError::UnknownFixedProxy)
            }
            CompatibleEgressTarget::ProxyPool { pool_id } => self
                .inner
                .pools
                .get(pool_id)
                .ok_or(CompatibleEgressTransportError::UnknownProxyPool)?
                .nodes
                .iter()
                .find(|node| node.node_id == node_id)
                .cloned()
                .ok_or(CompatibleEgressTransportError::UnknownProxyNode),
        }
    }
}

impl fmt::Debug for CompatibleEgressTransportRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompatibleEgressTransportRegistry")
            .field("owner_upstream_id", &self.inner.owner_upstream_id)
            .field("fixed_proxy_count", &self.inner.fixed.len())
            .field("proxy_pool_count", &self.inner.pools.len())
            .field(
                "proxy_node_count",
                &self
                    .inner
                    .fixed
                    .len()
                    .saturating_add(self.inner.pools.values().map(|pool| pool.nodes.len()).sum()),
            )
            .finish_non_exhaustive()
    }
}

/// A request-scoped transport-node lease.
pub struct CompatibleEgressTransportLease {
    target: CompatibleEgressTarget,
    selected_node_id: Option<String>,
    transport_profile: UpstreamTransportProfile,
    node: Option<Arc<EgressNodeSlot>>,
}

impl CompatibleEgressTransportLease {
    fn direct(target: CompatibleEgressTarget, transport_profile: UpstreamTransportProfile) -> Self {
        Self {
            target,
            selected_node_id: None,
            transport_profile,
            node: None,
        }
    }

    fn proxied(target: CompatibleEgressTarget, node: Arc<EgressNodeSlot>) -> Self {
        Self {
            target,
            selected_node_id: Some(node.node_id.clone()),
            transport_profile: node.transport_profile.clone(),
            node: Some(node),
        }
    }

    /// Returns the configured target identity.
    #[must_use]
    pub fn target(&self) -> &CompatibleEgressTarget {
        &self.target
    }

    /// Returns the selected node identity for fixed/pool targets.
    #[must_use]
    pub fn selected_node_id(&self) -> Option<&str> {
        self.selected_node_id.as_deref()
    }

    /// Returns the already validated transport profile for the later HTTP client.
    #[must_use]
    pub const fn transport_profile(&self) -> &UpstreamTransportProfile {
        &self.transport_profile
    }

    /// Explicitly releases the node capacity reservation.
    pub fn release(self) {
        drop(self);
    }
}

impl Drop for CompatibleEgressTransportLease {
    fn drop(&mut self) {
        if let Some(node) = &self.node {
            node.release();
        }
    }
}

impl fmt::Debug for CompatibleEgressTransportLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompatibleEgressTransportLease")
            .field("target", &self.target)
            .field("selected_node_id", &self.selected_node_id)
            .field("transport_profile", &self.transport_profile)
            .field("node_capacity_reserved", &self.node.is_some())
            .finish()
    }
}

struct CompatibleEgressTransportRegistryInner {
    owner_upstream_id: UpstreamId,
    direct_profile: UpstreamTransportProfile,
    fixed: BTreeMap<String, Arc<EgressNodeSlot>>,
    pools: BTreeMap<String, Arc<EgressNodePool>>,
}

struct EgressNodePool {
    nodes: Vec<Arc<EgressNodeSlot>>,
    schedule: Vec<usize>,
    cursor: AtomicUsize,
}

struct EgressNodeSlot {
    node_id: String,
    transport_profile: UpstreamTransportProfile,
    maximum_concurrency: usize,
    active_leases: AtomicUsize,
    availability: RwLock<CompatibleEgressNodeAvailability>,
}

impl EgressNodeSlot {
    fn new(
        node_id: String,
        transport_profile: UpstreamTransportProfile,
        maximum_concurrency: usize,
    ) -> Self {
        Self {
            node_id,
            transport_profile,
            maximum_concurrency,
            active_leases: AtomicUsize::new(0),
            availability: RwLock::new(CompatibleEgressNodeAvailability::Available),
        }
    }

    fn observe(&self, now_ms: i64) -> Result<NodeObservation, CompatibleEgressTransportError> {
        let state = self
            .availability
            .read()
            .map_err(|_| CompatibleEgressTransportError::RegistryUnavailable)?;
        let availability = state.at(now_ms);
        let active_leases = self.active_leases.load(Ordering::Acquire);
        Ok(NodeObservation {
            availability,
            has_capacity: availability.is_available_at(now_ms)
                && active_leases < self.maximum_concurrency,
            active_leases,
            maximum_concurrency: self.maximum_concurrency,
        })
    }

    fn try_acquire(&self, now_ms: i64) -> Result<bool, CompatibleEgressTransportError> {
        // Keep the read guard through the capacity CAS. A concurrent cooldown/disable therefore
        // linearizes either before this acquisition (and rejects it) or after the lease exists.
        let availability = self
            .availability
            .read()
            .map_err(|_| CompatibleEgressTransportError::RegistryUnavailable)?;
        if !availability.is_available_at(now_ms) {
            return Ok(false);
        }
        let mut active = self.active_leases.load(Ordering::Acquire);
        loop {
            if active >= self.maximum_concurrency {
                return Ok(false);
            }
            match self.active_leases.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(true),
                Err(observed) => active = observed,
            }
        }
    }

    fn release(&self) {
        let previous = self.active_leases.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(
            previous > 0,
            "egress node lease must release one acquisition"
        );
    }
}

struct NodeObservation {
    availability: CompatibleEgressNodeAvailability,
    has_capacity: bool,
    active_leases: usize,
    maximum_concurrency: usize,
}

impl EgressNodePool {
    fn observe(
        &self,
        target: CompatibleEgressTarget,
        now_ms: i64,
    ) -> Result<CompatibleEgressTargetObservation, CompatibleEgressTransportError> {
        let mut available_nodes = 0_usize;
        let mut active_leases = 0_usize;
        let mut maximum_concurrency = 0_usize;
        let mut earliest_cooldown: Option<i64> = None;
        let mut disabled_nodes = 0_usize;
        for node in &self.nodes {
            let observation = node.observe(now_ms)?;
            active_leases = active_leases.saturating_add(observation.active_leases);
            maximum_concurrency =
                maximum_concurrency.saturating_add(observation.maximum_concurrency);
            if observation.has_capacity {
                available_nodes = available_nodes.saturating_add(1);
            }
            match observation.availability {
                CompatibleEgressNodeAvailability::CoolingDown { until_ms } => {
                    earliest_cooldown =
                        Some(earliest_cooldown.map_or(until_ms, |old| old.min(until_ms)));
                }
                CompatibleEgressNodeAvailability::Disabled => {
                    disabled_nodes = disabled_nodes.saturating_add(1);
                }
                CompatibleEgressNodeAvailability::Available => {}
            }
        }
        let availability = if available_nodes > 0 {
            CompatibleEgressTargetAvailability::Available
        } else if let Some(until_ms) = earliest_cooldown {
            CompatibleEgressTargetAvailability::CoolingDown { until_ms }
        } else if disabled_nodes == self.nodes.len() {
            CompatibleEgressTargetAvailability::Disabled
        } else {
            CompatibleEgressTargetAvailability::Saturated
        };
        Ok(CompatibleEgressTargetObservation {
            target,
            availability,
            available_nodes,
            total_nodes: self.nodes.len(),
            active_leases,
            maximum_concurrency,
        })
    }

    fn try_acquire(
        &self,
        target: CompatibleEgressTarget,
        now_ms: i64,
    ) -> Result<CompatibleEgressTransportLease, CompatibleEgressTransportError> {
        let start = self.cursor.fetch_add(1, Ordering::Relaxed);
        for offset in 0..self.schedule.len() {
            let position = start.wrapping_add(offset) % self.schedule.len();
            let node = self
                .nodes
                .get(self.schedule[position])
                .ok_or(CompatibleEgressTransportError::RegistryUnavailable)?;
            if node.try_acquire(now_ms)? {
                return Ok(CompatibleEgressTransportLease::proxied(
                    target,
                    Arc::clone(node),
                ));
            }
        }
        Err(CompatibleEgressTransportError::NoAvailableNode)
    }

    fn try_acquire_exact(
        &self,
        target: CompatibleEgressTarget,
        node_id: &str,
        now_ms: i64,
    ) -> Result<CompatibleEgressTransportLease, CompatibleEgressTransportError> {
        let node = self
            .nodes
            .iter()
            .find(|node| node.node_id == node_id)
            .ok_or(CompatibleEgressTransportError::UnknownProxyNode)?;
        acquire_node(target, Arc::clone(node), now_ms)
    }
}

fn acquire_node(
    target: CompatibleEgressTarget,
    node: Arc<EgressNodeSlot>,
    now_ms: i64,
) -> Result<CompatibleEgressTransportLease, CompatibleEgressTransportError> {
    if node.try_acquire(now_ms)? {
        Ok(CompatibleEgressTransportLease::proxied(target, node))
    } else {
        Err(CompatibleEgressTransportError::NoAvailableNode)
    }
}

fn validate_proxy_profile(
    profile: &UpstreamTransportProfile,
) -> Result<(), CompatibleEgressTransportBuildError> {
    if matches!(profile.proxy(), UpstreamProxy::Socks5(_)) {
        Ok(())
    } else {
        Err(CompatibleEgressTransportBuildError::ProxyProfileMustBeLocalDnsSocks5)
    }
}

fn validate_label(value: &str) -> Result<(), CompatibleEgressTransportBuildError> {
    if value.trim().is_empty() {
        return Err(CompatibleEgressTransportBuildError::EmptyLabel);
    }
    if value != value.trim() || value.chars().any(char::is_control) {
        return Err(CompatibleEgressTransportBuildError::InvalidLabelShape);
    }
    if value.len() > MAX_COMPATIBLE_EGRESS_LABEL_LENGTH {
        return Err(CompatibleEgressTransportBuildError::LabelTooLong);
    }
    Ok(())
}

fn weighted_schedule(weights: &[usize]) -> Result<Vec<usize>, CompatibleEgressTransportBuildError> {
    let mut total = 0_usize;
    let mut signed_weights = Vec::with_capacity(weights.len());
    for weight in weights {
        total = total
            .checked_add(*weight)
            .ok_or(CompatibleEgressTransportBuildError::InvalidNodeWeight)?;
        if total > MAX_COMPATIBLE_EGRESS_SCHEDULE_SLOTS {
            return Err(CompatibleEgressTransportBuildError::InvalidNodeWeight);
        }
        signed_weights.push(
            i64::try_from(*weight)
                .map_err(|_| CompatibleEgressTransportBuildError::InvalidNodeWeight)?,
        );
    }
    let total_i64 =
        i64::try_from(total).map_err(|_| CompatibleEgressTransportBuildError::InvalidNodeWeight)?;
    let mut current = vec![0_i64; weights.len()];
    let mut schedule = Vec::with_capacity(total);
    for _ in 0..total {
        for (current_weight, weight) in current.iter_mut().zip(&signed_weights) {
            *current_weight = current_weight
                .checked_add(*weight)
                .ok_or(CompatibleEgressTransportBuildError::InvalidNodeWeight)?;
        }
        let mut selected = 0_usize;
        for position in 1..current.len() {
            if current[position] > current[selected] {
                selected = position;
            }
        }
        current[selected] = current[selected]
            .checked_sub(total_i64)
            .ok_or(CompatibleEgressTransportBuildError::InvalidNodeWeight)?;
        schedule.push(selected);
    }
    Ok(schedule)
}

#[cfg(test)]
mod tests {
    use std::{error::Error, num::NonZeroUsize, time::Duration};

    use gateway_core::UpstreamId;
    use gateway_upstream::{UpstreamProxy, UpstreamTimeouts};

    use super::{
        CompatibleEgressNodeInput, CompatibleEgressTargetAvailability,
        CompatibleEgressTransportError, CompatibleEgressTransportRegistry,
        CompatibleEgressTransportRegistryInput, CompatibleFixedProxyInput,
        CompatibleProxyPoolInput,
    };

    fn timeouts() -> Result<UpstreamTimeouts, Box<dyn Error>> {
        Ok(UpstreamTimeouts::try_new(
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(3),
            Duration::from_secs(5),
        )?)
    }

    fn direct_profile() -> Result<gateway_upstream::UpstreamTransportProfile, Box<dyn Error>> {
        Ok(gateway_upstream::UpstreamTransportProfile::new(
            timeouts()?,
            UpstreamProxy::Direct,
            NonZeroUsize::new(2).ok_or("nonzero client cache")?,
        ))
    }

    fn socks_profile(
        port: u16,
    ) -> Result<gateway_upstream::UpstreamTransportProfile, Box<dyn Error>> {
        Ok(gateway_upstream::UpstreamTransportProfile::new(
            timeouts()?,
            UpstreamProxy::try_socks5(&format!("socks5://127.0.0.1:{port}"))?,
            NonZeroUsize::new(2).ok_or("nonzero client cache")?,
        ))
    }

    fn registry() -> Result<CompatibleEgressTransportRegistry, Box<dyn Error>> {
        Ok(CompatibleEgressTransportRegistry::try_new(
            CompatibleEgressTransportRegistryInput {
                owner_upstream_id: UpstreamId::try_new("upstream-a")?,
                direct_profile: direct_profile()?,
                fixed_proxies: vec![CompatibleFixedProxyInput {
                    profile_id: "fixed-a".to_owned(),
                    transport_profile: socks_profile(19081)?,
                    maximum_concurrency: 1,
                }],
                proxy_pools: vec![CompatibleProxyPoolInput {
                    pool_id: "pool-a".to_owned(),
                    nodes: vec![
                        CompatibleEgressNodeInput {
                            node_id: "node-a".to_owned(),
                            transport_profile: socks_profile(19082)?,
                            weight: 2,
                            maximum_concurrency: 1,
                        },
                        CompatibleEgressNodeInput {
                            node_id: "node-b".to_owned(),
                            transport_profile: socks_profile(19083)?,
                            weight: 1,
                            maximum_concurrency: 1,
                        },
                    ],
                }],
            },
        )?)
    }

    #[test]
    fn direct_fixed_and_pool_targets_are_resolved_without_network() -> Result<(), Box<dyn Error>> {
        let registry = registry()?;
        let direct = gateway_upstream::CompatibleEgressTarget::Direct;
        let fixed = gateway_upstream::CompatibleEgressTarget::FixedProxy {
            profile_id: "fixed-a".to_owned(),
        };
        let pool = gateway_upstream::CompatibleEgressTarget::ProxyPool {
            pool_id: "pool-a".to_owned(),
        };
        assert!(registry.contains_target(&direct));
        assert!(registry.contains_target(&fixed));
        assert!(registry.contains_target(&pool));
        assert_eq!(
            registry.observe(&direct, 100)?.availability(),
            CompatibleEgressTargetAvailability::Available
        );
        assert_eq!(registry.observe(&pool, 100)?.available_nodes(), 2);
        let fixed_lease = registry.try_acquire(&fixed, 100)?;
        assert_eq!(fixed_lease.selected_node_id(), Some("fixed-a"));
        assert!(matches!(
            fixed_lease.transport_profile().proxy(),
            UpstreamProxy::Socks5(_)
        ));
        fixed_lease.release();
        Ok(())
    }

    #[test]
    fn pool_weight_and_capacity_are_bounded_and_drop_releases_node() -> Result<(), Box<dyn Error>> {
        let registry = registry()?;
        let pool = gateway_upstream::CompatibleEgressTarget::ProxyPool {
            pool_id: "pool-a".to_owned(),
        };
        let first = registry.try_acquire(&pool, 100)?;
        let first_node = first.selected_node_id().map(str::to_owned).ok_or("node")?;
        assert_eq!(registry.observe(&pool, 100)?.active_leases(), 1);
        let second = registry.try_acquire(&pool, 100)?;
        assert_ne!(second.selected_node_id(), Some(first_node.as_str()));
        assert_eq!(registry.observe(&pool, 100)?.active_leases(), 2);
        assert!(matches!(
            registry.try_acquire(&pool, 100),
            Err(CompatibleEgressTransportError::NoAvailableNode)
        ));
        drop(first);
        assert_eq!(registry.observe(&pool, 100)?.active_leases(), 1);
        drop(second);
        assert_eq!(registry.observe(&pool, 100)?.active_leases(), 0);
        Ok(())
    }

    #[test]
    fn node_cooldown_and_disabled_state_do_not_change_direct_or_other_pool_nodes()
    -> Result<(), Box<dyn Error>> {
        let registry = registry()?;
        let pool = gateway_upstream::CompatibleEgressTarget::ProxyPool {
            pool_id: "pool-a".to_owned(),
        };
        let fixed = gateway_upstream::CompatibleEgressTarget::FixedProxy {
            profile_id: "fixed-a".to_owned(),
        };
        registry.cool_down_until(&pool, "node-a", 200, 100)?;
        let observation = registry.observe(&pool, 100)?;
        assert_eq!(observation.available_nodes(), 1);
        assert_eq!(
            observation.availability(),
            CompatibleEgressTargetAvailability::Available
        );
        registry.disable(&pool, "node-b")?;
        assert_eq!(
            registry.observe(&pool, 100)?.availability(),
            CompatibleEgressTargetAvailability::CoolingDown { until_ms: 200 }
        );
        assert_eq!(
            registry.observe(&pool, 250)?.availability(),
            CompatibleEgressTargetAvailability::Available
        );
        assert_eq!(registry.observe(&pool, 250)?.available_nodes(), 1);
        assert_eq!(
            registry.observe(&fixed, 100)?.availability(),
            CompatibleEgressTargetAvailability::Available
        );
        assert_eq!(
            registry
                .observe(&gateway_upstream::CompatibleEgressTarget::Direct, 100)?
                .availability(),
            CompatibleEgressTargetAvailability::Available
        );
        registry.restore(&pool, "node-a")?;
        registry.restore(&pool, "node-b")?;
        assert_eq!(registry.observe(&pool, 250)?.available_nodes(), 2);
        Ok(())
    }

    #[test]
    fn proxy_registry_rejects_direct_or_remote_proxy_profiles_and_invalid_shapes()
    -> Result<(), Box<dyn Error>> {
        let direct = direct_profile()?;
        let invalid_direct =
            CompatibleEgressTransportRegistry::try_new(CompatibleEgressTransportRegistryInput {
                owner_upstream_id: UpstreamId::try_new("upstream-a")?,
                direct_profile: socks_profile(19090)?,
                fixed_proxies: Vec::new(),
                proxy_pools: Vec::new(),
            });
        assert!(matches!(
            invalid_direct,
            Err(super::CompatibleEgressTransportBuildError::DirectProfileMustBeDirect)
        ));
        let invalid_node =
            CompatibleEgressTransportRegistry::try_new(CompatibleEgressTransportRegistryInput {
                owner_upstream_id: UpstreamId::try_new("upstream-a")?,
                direct_profile: direct,
                fixed_proxies: vec![CompatibleFixedProxyInput {
                    profile_id: "fixed".to_owned(),
                    transport_profile: gateway_upstream::UpstreamTransportProfile::new(
                        timeouts()?,
                        UpstreamProxy::Direct,
                        NonZeroUsize::new(1).ok_or("nonzero")?,
                    ),
                    maximum_concurrency: 1,
                }],
                proxy_pools: Vec::new(),
            });
        assert!(matches!(
            invalid_node,
            Err(super::CompatibleEgressTransportBuildError::ProxyProfileMustBeLocalDnsSocks5)
        ));
        Ok(())
    }

    #[test]
    fn state_errors_are_closed_and_debug_has_no_proxy_value() -> Result<(), Box<dyn Error>> {
        let registry = registry()?;
        let direct = gateway_upstream::CompatibleEgressTarget::Direct;
        assert_eq!(
            registry.cool_down_until(&direct, "node", 200, 100),
            Err(CompatibleEgressTransportError::DirectTargetHasNoNode)
        );
        let debug = format!("{registry:?}");
        assert!(!debug.contains("127.0.0.1"));
        assert!(!debug.contains("socks5://"));
        Ok(())
    }
}
