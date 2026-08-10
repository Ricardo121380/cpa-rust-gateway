//! Secret-free, version-scoped operational projections for the management surface.
//!
//! This module compiles configured Endpoint/Credential bindings into a stable Provider/Channel/
//! Account inventory. It reads one [`ControlPlaneConfiguration`] only; it never decrypts a
//! Credential, observes runtime health, or calls a Provider.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use gateway_core::{
    AccessGroupId, AttemptEvent, AttemptOutcome, ClientKeyId, CredentialId, EgressPolicyId,
    EndpointId, GatewayEvent, GatewayProtocol, RequestEvent, RouteId, UpstreamId, UsageEvent,
};
use gateway_store::control_plane::{
    ControlPlaneConfiguration, CredentialConfiguration, CredentialStatus, EndpointConfiguration,
    EndpointCredentialBindingConfiguration, EndpointTransport, UpstreamConfiguration,
};
use gateway_store::event_store::StoredGatewayEvent;

/// Default number of configured account bindings returned by one inventory page.
pub const DEFAULT_ACCOUNT_POOL_LIMIT: usize = 50;
/// Maximum number of configured account bindings returned by one inventory page.
pub const MAX_ACCOUNT_POOL_LIMIT: usize = 100;
/// Default number of aggregated usage groups returned by one operations page.
pub const DEFAULT_USAGE_LIMIT: usize = 50;
/// Maximum number of aggregated usage groups returned by one operations page.
pub const MAX_USAGE_LIMIT: usize = 100;
/// Maximum number of durable events admitted to one in-process usage aggregation.
pub const MAX_USAGE_EVENTS: usize = 100_000;
/// Maximum public model label length admitted to the operations projection.
pub const MAX_USAGE_MODEL_CHARS: usize = 256;

/// Typed filters and page position for the configured account-pool inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalAccountPoolQuery {
    /// Restricts results to one exact Provider (stored as an Upstream).
    pub provider_id: Option<UpstreamId>,
    /// Restricts results to one exact Channel (stored as an Endpoint).
    pub channel_id: Option<EndpointId>,
    /// Restricts results to one persisted Credential lifecycle state.
    pub account_status: Option<CredentialStatus>,
    /// Restricts results by the static Provider/Channel/Binding enabled conjunction.
    pub enabled: Option<bool>,
    /// Bounded requested page size.
    pub limit: usize,
    /// Optional revision-bound stable keyset cursor.
    pub cursor: Option<OperationalAccountPoolCursor>,
}

impl Default for OperationalAccountPoolQuery {
    fn default() -> Self {
        Self {
            provider_id: None,
            channel_id: None,
            account_status: None,
            enabled: None,
            limit: DEFAULT_ACCOUNT_POOL_LIMIT,
            cursor: None,
        }
    }
}

impl OperationalAccountPoolQuery {
    /// Builds a query only when its requested page size is within the public bound.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementOperationsError::InvalidQuery`] for a zero or oversized limit.
    pub fn try_new(
        provider_id: Option<UpstreamId>,
        channel_id: Option<EndpointId>,
        account_status: Option<CredentialStatus>,
        enabled: Option<bool>,
        limit: usize,
        cursor: Option<OperationalAccountPoolCursor>,
    ) -> Result<Self, ManagementOperationsError> {
        if !(1..=MAX_ACCOUNT_POOL_LIMIT).contains(&limit) {
            return Err(ManagementOperationsError::InvalidQuery);
        }
        Ok(Self {
            provider_id,
            channel_id,
            account_status,
            enabled,
            limit,
            cursor,
        })
    }
}

/// Opaque-transport payload for one revision-bound account inventory page position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalAccountPoolCursor {
    config_version_id: String,
    revision: i64,
    provider_id: UpstreamId,
    channel_id: EndpointId,
    account_id: CredentialId,
}

impl OperationalAccountPoolCursor {
    /// Reconstructs a decoded cursor payload before it is checked against the selected graph.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementOperationsError::InvalidQuery`] for an empty Version id or negative
    /// revision. Identifier constructors validate the remaining opaque fields at the HTTP edge.
    pub fn try_new(
        config_version_id: impl Into<String>,
        revision: i64,
        provider_id: UpstreamId,
        channel_id: EndpointId,
        account_id: CredentialId,
    ) -> Result<Self, ManagementOperationsError> {
        let config_version_id = config_version_id.into();
        if config_version_id.is_empty() || revision < 0 {
            return Err(ManagementOperationsError::InvalidQuery);
        }
        Ok(Self {
            config_version_id,
            revision,
            provider_id,
            channel_id,
            account_id,
        })
    }

    /// Returns the exact Config Version bound into the cursor.
    #[must_use]
    pub fn config_version_id(&self) -> &str {
        &self.config_version_id
    }

    /// Returns the exact Config Version revision bound into the cursor.
    #[must_use]
    pub const fn revision(&self) -> i64 {
        self.revision
    }

    /// Returns the last Provider key emitted by the prior page.
    #[must_use]
    pub const fn provider_id(&self) -> &UpstreamId {
        &self.provider_id
    }

    /// Returns the last Channel key emitted by the prior page.
    #[must_use]
    pub const fn channel_id(&self) -> &EndpointId {
        &self.channel_id
    }

    /// Returns the last Account key emitted by the prior page.
    #[must_use]
    pub const fn account_id(&self) -> &CredentialId {
        &self.account_id
    }

    fn key(&self) -> (&str, &str, &str) {
        (
            self.provider_id.as_str(),
            self.channel_id.as_str(),
            self.account_id.as_str(),
        )
    }
}

/// One configured Provider/Channel/Account binding safe for an operator inventory response.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct OperationalAccountPoolItem {
    /// Stable Provider identity (the owning Upstream).
    pub provider_id: UpstreamId,
    /// Non-secret configured Provider name.
    pub provider_name: String,
    /// Non-secret Provider family.
    pub provider_kind: String,
    /// Provider administrative eligibility.
    pub provider_enabled: bool,
    /// Optional same-version egress policy identity; never an endpoint URL or proxy secret.
    pub egress_policy_id: Option<EgressPolicyId>,
    /// Stable Channel identity (the bound Endpoint).
    pub channel_id: EndpointId,
    /// Registered Provider adapter identity.
    pub adapter_id: String,
    /// Configured protocol format.
    pub api_format: String,
    /// Configured transport, not a live connectivity observation.
    pub transport: EndpointTransport,
    /// Channel administrative eligibility.
    pub channel_enabled: bool,
    /// Stable Account identity (the encrypted Credential record).
    pub account_id: CredentialId,
    /// Non-secret Credential family.
    pub account_kind: String,
    /// Persisted Credential lifecycle state.
    pub account_status: CredentialStatus,
    /// Per-Credential record revision.
    pub account_revision: i64,
    /// Binding administrative eligibility.
    pub binding_enabled: bool,
    /// Static conjunction of Provider, Channel, and Binding eligibility.
    pub configured_enabled: bool,
    /// Lower configured scheduling value has higher priority.
    pub priority: i64,
    /// Configured scheduling weight.
    pub weight: i64,
    /// Configured concurrency ceiling.
    pub concurrency: i64,
    /// Deterministically ordered Routes that structurally reference this Channel.
    pub route_ids: Vec<RouteId>,
}

impl OperationalAccountPoolItem {
    fn key(&self) -> (&str, &str, &str) {
        (
            self.provider_id.as_str(),
            self.channel_id.as_str(),
            self.account_id.as_str(),
        )
    }
}

/// One stable page of the selected Config Version's configured account bindings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalAccountPoolPage {
    /// Selected Config Version identity.
    pub config_version_id: String,
    /// Exact revision used to compile the page.
    pub revision: i64,
    /// Deterministically ordered secret-free inventory items.
    pub items: Vec<OperationalAccountPoolItem>,
    /// Cursor for the next page, absent when this page exhausted the filtered result.
    pub next_cursor: Option<OperationalAccountPoolCursor>,
}

/// Exact/partial/unknown confidence for one aggregated token counter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalTokenConfidence {
    /// Every usage observation in the group supplied this counter.
    Exact,
    /// At least one observation supplied the counter and at least one omitted it.
    Partial,
    /// No observation in the group supplied the counter.
    Unknown,
}

/// One bounded aggregated token counter with an explicit confidence label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationalTokenMetric {
    /// Checked sum when at least one observation supplied a value.
    pub total: Option<u64>,
    /// Whether the total is exact, partial, or unavailable.
    pub confidence: OperationalTokenConfidence,
}

/// Cost confidence for the P13-04 read model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalCostConfidence {
    /// No price catalog is available, so the API must not fabricate a cost.
    Unpriced,
}

/// Opaque keyset position for an aggregated usage group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalUsageCursor {
    provider_id: UpstreamId,
    channel_id: EndpointId,
    account_id: CredentialId,
    public_model: String,
    protocol: GatewayProtocol,
    client_key_id: ClientKeyId,
    access_group_id: Option<AccessGroupId>,
}

impl OperationalUsageCursor {
    /// Reconstructs one cursor after the HTTP boundary validated its bounded fields.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementOperationsError::InvalidQuery`] for an empty or overlong model.
    pub fn try_new(
        provider_id: UpstreamId,
        channel_id: EndpointId,
        account_id: CredentialId,
        public_model: String,
        protocol: GatewayProtocol,
        client_key_id: ClientKeyId,
        access_group_id: Option<AccessGroupId>,
    ) -> Result<Self, ManagementOperationsError> {
        if public_model.is_empty() || public_model.chars().count() > MAX_USAGE_MODEL_CHARS {
            return Err(ManagementOperationsError::InvalidQuery);
        }
        Ok(Self {
            provider_id,
            channel_id,
            account_id,
            public_model,
            protocol,
            client_key_id,
            access_group_id,
        })
    }

    /// Returns the Provider in this keyset position.
    #[must_use]
    pub const fn provider_id(&self) -> &UpstreamId {
        &self.provider_id
    }

    /// Returns the Channel in this keyset position.
    #[must_use]
    pub const fn channel_id(&self) -> &EndpointId {
        &self.channel_id
    }

    /// Returns the Account in this keyset position.
    #[must_use]
    pub const fn account_id(&self) -> &CredentialId {
        &self.account_id
    }

    /// Returns the public model in this keyset position.
    #[must_use]
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// Returns the protocol in this keyset position.
    #[must_use]
    pub const fn protocol(&self) -> GatewayProtocol {
        self.protocol
    }

    /// Returns the non-secret Client Key identity in this keyset position.
    #[must_use]
    pub const fn client_key_id(&self) -> &ClientKeyId {
        &self.client_key_id
    }

    /// Returns the optional Access Group identity in this keyset position.
    #[must_use]
    pub fn access_group_id(&self) -> Option<&AccessGroupId> {
        self.access_group_id.as_ref()
    }

    fn key(&self) -> OperationalUsageSortKey {
        OperationalUsageSortKey {
            provider_id: self.provider_id.as_str().to_owned(),
            channel_id: self.channel_id.as_str().to_owned(),
            account_id: self.account_id.as_str().to_owned(),
            public_model: self.public_model.clone(),
            protocol: protocol_key(self.protocol).to_owned(),
            client_key_id: self.client_key_id.as_str().to_owned(),
            access_group_id: self
                .access_group_id
                .as_ref()
                .map_or_else(String::new, |id| id.as_str().to_owned()),
        }
    }
}

/// Typed query for the durable usage/cost operations projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalUsageQuery {
    /// Inclusive lower bound applied to the selected successful Attempt's end time.
    pub from_ms: Option<i64>,
    /// Inclusive upper bound applied to the selected successful Attempt's end time.
    pub to_ms: Option<i64>,
    /// Exact Provider filter.
    pub provider_id: Option<UpstreamId>,
    /// Exact Channel filter.
    pub channel_id: Option<EndpointId>,
    /// Exact Account filter.
    pub account_id: Option<CredentialId>,
    /// Exact public model filter.
    pub public_model: Option<String>,
    /// Exact Client Key filter.
    pub client_key_id: Option<ClientKeyId>,
    /// Exact Access Group filter.
    pub access_group_id: Option<AccessGroupId>,
    /// Exact inbound protocol filter.
    pub protocol: Option<GatewayProtocol>,
    /// Bounded requested page size.
    pub limit: usize,
    /// Optional stable keyset cursor.
    pub cursor: Option<OperationalUsageCursor>,
}

impl Default for OperationalUsageQuery {
    fn default() -> Self {
        Self {
            from_ms: None,
            to_ms: None,
            provider_id: None,
            channel_id: None,
            account_id: None,
            public_model: None,
            client_key_id: None,
            access_group_id: None,
            protocol: None,
            limit: DEFAULT_USAGE_LIMIT,
            cursor: None,
        }
    }
}

impl OperationalUsageQuery {
    /// Builds a bounded query for the durable usage projection.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementOperationsError::InvalidQuery`] for reversed/negative time bounds,
    /// an empty/overlong model, or an out-of-range page size.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        from_ms: Option<i64>,
        to_ms: Option<i64>,
        provider_id: Option<UpstreamId>,
        channel_id: Option<EndpointId>,
        account_id: Option<CredentialId>,
        public_model: Option<String>,
        client_key_id: Option<ClientKeyId>,
        access_group_id: Option<AccessGroupId>,
        protocol: Option<GatewayProtocol>,
        limit: usize,
        cursor: Option<OperationalUsageCursor>,
    ) -> Result<Self, ManagementOperationsError> {
        if from_ms.is_some_and(|value| value < 0)
            || to_ms.is_some_and(|value| value < 0)
            || matches!((from_ms, to_ms), (Some(from), Some(to)) if from > to)
            || !(1..=MAX_USAGE_LIMIT).contains(&limit)
            || public_model.as_ref().is_some_and(|model| {
                model.is_empty() || model.chars().count() > MAX_USAGE_MODEL_CHARS
            })
        {
            return Err(ManagementOperationsError::InvalidQuery);
        }
        Ok(Self {
            from_ms,
            to_ms,
            provider_id,
            channel_id,
            account_id,
            public_model,
            client_key_id,
            access_group_id,
            protocol,
            limit,
            cursor,
        })
    }
}

/// One aggregated, secret-free usage group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalUsageItem {
    /// Provider selected by the successful Attempt.
    pub provider_id: UpstreamId,
    /// Channel selected by the successful Attempt.
    pub channel_id: EndpointId,
    /// Account selected by the successful Attempt.
    pub account_id: CredentialId,
    /// Public model identity from the accepted inbound Request.
    pub public_model: String,
    /// Accepted inbound protocol.
    pub protocol: GatewayProtocol,
    /// Non-secret Client Key identity.
    pub client_key_id: ClientKeyId,
    /// Optional non-secret Access Group identity.
    pub access_group_id: Option<AccessGroupId>,
    /// Number of accepted requests represented by this group.
    pub request_count: u64,
    /// Number of final Usage observations represented by this group.
    pub usage_observations: u64,
    /// Aggregated input token count with explicit confidence.
    pub input_tokens: OperationalTokenMetric,
    /// Aggregated output token count with explicit confidence.
    pub output_tokens: OperationalTokenMetric,
    /// Aggregated reasoning token count with explicit confidence.
    pub reasoning_tokens: OperationalTokenMetric,
    /// Aggregated cache-read token count with explicit confidence.
    pub cache_read_tokens: OperationalTokenMetric,
    /// Aggregated cache-creation token count with explicit confidence.
    pub cache_creation_tokens: OperationalTokenMetric,
    /// Aggregated cached token count with explicit confidence.
    pub cached_tokens: OperationalTokenMetric,
    /// Latest selected successful Attempt end time in the group.
    pub observed_at_ms: i64,
    /// Cost is null until a versioned price catalog is introduced.
    pub cost_microunits: Option<u64>,
    /// Explicit cost confidence; never inferred from token totals.
    pub cost_confidence: OperationalCostConfidence,
}

impl OperationalUsageItem {
    fn key(&self) -> OperationalUsageSortKey {
        OperationalUsageSortKey {
            provider_id: self.provider_id.as_str().to_owned(),
            channel_id: self.channel_id.as_str().to_owned(),
            account_id: self.account_id.as_str().to_owned(),
            public_model: self.public_model.clone(),
            protocol: protocol_key(self.protocol).to_owned(),
            client_key_id: self.client_key_id.as_str().to_owned(),
            access_group_id: self
                .access_group_id
                .as_ref()
                .map_or_else(String::new, |id| id.as_str().to_owned()),
        }
    }
}

/// One page from the durable usage/cost read model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalUsagePage {
    /// Latest selected successful Attempt end time among all filtered groups.
    pub observed_through_ms: Option<i64>,
    /// Stable sorted usage groups.
    pub items: Vec<OperationalUsageItem>,
    /// Cursor for the next page, absent when the filtered result is exhausted.
    pub next_cursor: Option<OperationalUsageCursor>,
}

/// Compiles one secret-free, deterministic configured account-pool inventory page.
///
/// # Errors
///
/// Returns a safe error when the query is invalid, a cursor belongs to another graph revision,
/// or a binding violates Provider ownership invariants.
pub fn compile_operational_account_pool_page(
    configuration: &ControlPlaneConfiguration,
    query: &OperationalAccountPoolQuery,
) -> Result<OperationalAccountPoolPage, ManagementOperationsError> {
    if !(1..=MAX_ACCOUNT_POOL_LIMIT).contains(&query.limit) || configuration.version.revision < 0 {
        return Err(ManagementOperationsError::InvalidQuery);
    }
    if query.cursor.as_ref().is_some_and(|cursor| {
        cursor.config_version_id() != configuration.version.id.as_str()
            || cursor.revision() != configuration.version.revision
    }) {
        return Err(ManagementOperationsError::CursorVersionConflict);
    }

    let providers = configuration
        .upstreams
        .iter()
        .map(|provider| (provider.id.as_str(), provider))
        .collect::<BTreeMap<_, _>>();
    let channels = configuration
        .endpoints
        .iter()
        .map(|channel| (channel.id.as_str(), channel))
        .collect::<BTreeMap<_, _>>();
    let accounts = configuration
        .credentials
        .iter()
        .map(|account| (account.id.as_str(), account))
        .collect::<BTreeMap<_, _>>();
    let mut routes_by_channel = BTreeMap::<&str, BTreeSet<RouteId>>::new();
    for candidate in &configuration.route_candidates {
        routes_by_channel
            .entry(candidate.endpoint_id.as_str())
            .or_default()
            .insert(candidate.route_id.clone());
    }

    let mut items = Vec::with_capacity(configuration.endpoint_credential_bindings.len());
    for binding in &configuration.endpoint_credential_bindings {
        let item = compile_operational_account_pool_item(
            binding,
            &providers,
            &channels,
            &accounts,
            &routes_by_channel,
        )?;
        if matches_operational_account_pool_query(&item, query) {
            items.push(item);
        }
    }

    items.sort_by(|left, right| left.key().cmp(&right.key()));
    if let Some(cursor) = &query.cursor {
        items.retain(|item| item.key() > cursor.key());
    }

    let has_more = items.len() > query.limit;
    items.truncate(query.limit);
    let next_cursor =
        has_more
            .then(|| items.last())
            .flatten()
            .map(|item| OperationalAccountPoolCursor {
                config_version_id: configuration.version.id.as_str().to_owned(),
                revision: configuration.version.revision,
                provider_id: item.provider_id.clone(),
                channel_id: item.channel_id.clone(),
                account_id: item.account_id.clone(),
            });

    Ok(OperationalAccountPoolPage {
        config_version_id: configuration.version.id.as_str().to_owned(),
        revision: configuration.version.revision,
        items,
        next_cursor,
    })
}

fn compile_operational_account_pool_item<'a>(
    binding: &EndpointCredentialBindingConfiguration,
    providers: &BTreeMap<&'a str, &'a UpstreamConfiguration>,
    channels: &BTreeMap<&'a str, &'a EndpointConfiguration>,
    accounts: &BTreeMap<&'a str, &'a CredentialConfiguration>,
    routes_by_channel: &BTreeMap<&'a str, BTreeSet<RouteId>>,
) -> Result<OperationalAccountPoolItem, ManagementOperationsError> {
    let provider = providers
        .get(binding.upstream_id.as_str())
        .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
    let channel = channels
        .get(binding.endpoint_id.as_str())
        .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
    let account = accounts
        .get(binding.credential_id.as_str())
        .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
    if channel.upstream_id != binding.upstream_id || account.upstream_id != binding.upstream_id {
        return Err(ManagementOperationsError::InconsistentConfiguration);
    }

    Ok(OperationalAccountPoolItem {
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        provider_kind: provider.kind.clone(),
        provider_enabled: provider.enabled,
        egress_policy_id: provider.egress_policy_id.clone(),
        channel_id: channel.id.clone(),
        adapter_id: channel.adapter_id.clone(),
        api_format: channel.api_format.clone(),
        transport: channel.transport,
        channel_enabled: channel.enabled,
        account_id: account.id.clone(),
        account_kind: account.kind.clone(),
        account_status: account.status,
        account_revision: account.revision,
        binding_enabled: binding.enabled,
        configured_enabled: provider.enabled && channel.enabled && binding.enabled,
        priority: binding.priority,
        weight: binding.weight,
        concurrency: binding.concurrency,
        route_ids: routes_by_channel
            .get(channel.id.as_str())
            .map_or_else(Vec::new, |routes| routes.iter().cloned().collect()),
    })
}

fn matches_operational_account_pool_query(
    item: &OperationalAccountPoolItem,
    query: &OperationalAccountPoolQuery,
) -> bool {
    query
        .provider_id
        .as_ref()
        .is_none_or(|id| id == &item.provider_id)
        && query
            .channel_id
            .as_ref()
            .is_none_or(|id| id == &item.channel_id)
        && query
            .account_status
            .is_none_or(|status| status == item.account_status)
        && query
            .enabled
            .is_none_or(|enabled| enabled == item.configured_enabled)
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct OperationalUsageSortKey {
    provider_id: String,
    channel_id: String,
    account_id: String,
    public_model: String,
    protocol: String,
    client_key_id: String,
    access_group_id: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct UsageTokenAccumulator {
    total: u64,
    observed: u64,
    missing: u64,
}

impl UsageTokenAccumulator {
    fn add(&mut self, value: Option<u64>) -> Result<(), ManagementOperationsError> {
        match value {
            Some(value) => {
                self.total = self
                    .total
                    .checked_add(value)
                    .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
                self.observed = self
                    .observed
                    .checked_add(1)
                    .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
            }
            None => {
                self.missing = self
                    .missing
                    .checked_add(1)
                    .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
            }
        }
        Ok(())
    }

    fn merge(&mut self, other: &Self) -> Result<(), ManagementOperationsError> {
        self.total = self
            .total
            .checked_add(other.total)
            .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
        self.observed = self
            .observed
            .checked_add(other.observed)
            .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
        self.missing = self
            .missing
            .checked_add(other.missing)
            .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
        Ok(())
    }

    fn metric(&self) -> OperationalTokenMetric {
        let (total, confidence) = if self.observed == 0 {
            (None, OperationalTokenConfidence::Unknown)
        } else if self.missing == 0 {
            (Some(self.total), OperationalTokenConfidence::Exact)
        } else {
            (Some(self.total), OperationalTokenConfidence::Partial)
        };
        OperationalTokenMetric { total, confidence }
    }
}

#[derive(Clone)]
struct UsageAccumulator {
    provider_id: UpstreamId,
    channel_id: EndpointId,
    account_id: CredentialId,
    public_model: String,
    protocol: GatewayProtocol,
    client_key_id: ClientKeyId,
    access_group_id: Option<AccessGroupId>,
    request_count: u64,
    usage_observations: u64,
    input_tokens: UsageTokenAccumulator,
    output_tokens: UsageTokenAccumulator,
    reasoning_tokens: UsageTokenAccumulator,
    cache_read_tokens: UsageTokenAccumulator,
    cache_creation_tokens: UsageTokenAccumulator,
    cached_tokens: UsageTokenAccumulator,
    observed_at_ms: i64,
}

impl UsageAccumulator {
    fn new(
        request: &RequestEvent,
        attempt: &AttemptEvent,
    ) -> Result<Self, ManagementOperationsError> {
        if attempt.ended_at_ms() < 0 || !bounded_usage_text(request.public_model()) {
            return Err(ManagementOperationsError::InconsistentConfiguration);
        }
        Ok(Self {
            provider_id: attempt.upstream_id().clone(),
            channel_id: attempt.endpoint_id().clone(),
            account_id: attempt.credential_id().clone(),
            public_model: request.public_model().to_owned(),
            protocol: request.protocol(),
            client_key_id: request.client_key_id().clone(),
            access_group_id: request.access_group_id().cloned(),
            request_count: 0,
            usage_observations: 0,
            input_tokens: UsageTokenAccumulator::default(),
            output_tokens: UsageTokenAccumulator::default(),
            reasoning_tokens: UsageTokenAccumulator::default(),
            cache_read_tokens: UsageTokenAccumulator::default(),
            cache_creation_tokens: UsageTokenAccumulator::default(),
            cached_tokens: UsageTokenAccumulator::default(),
            observed_at_ms: attempt.ended_at_ms(),
        })
    }

    fn add_usage(&mut self, usage: &UsageEvent) -> Result<(), ManagementOperationsError> {
        self.request_count = self
            .request_count
            .checked_add(1)
            .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
        self.usage_observations = self
            .usage_observations
            .checked_add(1)
            .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
        let usage = usage.usage();
        self.input_tokens.add(usage.input_tokens)?;
        self.output_tokens.add(usage.output_tokens)?;
        self.reasoning_tokens.add(usage.reasoning_tokens)?;
        self.cache_read_tokens.add(usage.cache_read_tokens)?;
        self.cache_creation_tokens
            .add(usage.cache_creation_tokens)?;
        self.cached_tokens.add(usage.cached_tokens)?;
        Ok(())
    }

    fn merge(&mut self, other: &Self) -> Result<(), ManagementOperationsError> {
        self.request_count = self
            .request_count
            .checked_add(other.request_count)
            .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
        self.usage_observations = self
            .usage_observations
            .checked_add(other.usage_observations)
            .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
        self.input_tokens.merge(&other.input_tokens)?;
        self.output_tokens.merge(&other.output_tokens)?;
        self.reasoning_tokens.merge(&other.reasoning_tokens)?;
        self.cache_read_tokens.merge(&other.cache_read_tokens)?;
        self.cache_creation_tokens
            .merge(&other.cache_creation_tokens)?;
        self.cached_tokens.merge(&other.cached_tokens)?;
        self.observed_at_ms = self.observed_at_ms.max(other.observed_at_ms);
        Ok(())
    }

    fn into_item(self) -> OperationalUsageItem {
        OperationalUsageItem {
            provider_id: self.provider_id,
            channel_id: self.channel_id,
            account_id: self.account_id,
            public_model: self.public_model,
            protocol: self.protocol,
            client_key_id: self.client_key_id,
            access_group_id: self.access_group_id,
            request_count: self.request_count,
            usage_observations: self.usage_observations,
            input_tokens: self.input_tokens.metric(),
            output_tokens: self.output_tokens.metric(),
            reasoning_tokens: self.reasoning_tokens.metric(),
            cache_read_tokens: self.cache_read_tokens.metric(),
            cache_creation_tokens: self.cache_creation_tokens.metric(),
            cached_tokens: self.cached_tokens.metric(),
            observed_at_ms: self.observed_at_ms,
            cost_microunits: None,
            cost_confidence: OperationalCostConfidence::Unpriced,
        }
    }
}

fn bounded_usage_text(value: &str) -> bool {
    !value.is_empty() && value.chars().count() <= MAX_USAGE_MODEL_CHARS
}

fn protocol_key(value: GatewayProtocol) -> &'static str {
    match value {
        GatewayProtocol::OpenAiChatCompletions => "openai_chat_completions",
        GatewayProtocol::OpenAiResponses => "openai_responses",
        GatewayProtocol::AnthropicMessages => "anthropic_messages",
    }
}

fn insert_attempt_event(
    attempts: &mut BTreeMap<String, AttemptEvent>,
    next: &AttemptEvent,
) -> Result<(), ManagementOperationsError> {
    let key = next.request_id().as_str().to_owned();
    if next.attempt_number() == 0 || next.ended_at_ms() < 0 {
        return Err(ManagementOperationsError::InconsistentConfiguration);
    }
    match attempts.get(&key) {
        Some(existing) if existing.attempt_number() == next.attempt_number() => {
            if existing == next {
                Ok(())
            } else {
                Err(ManagementOperationsError::InconsistentConfiguration)
            }
        }
        Some(existing) if existing.attempt_number() > next.attempt_number() => Ok(()),
        _ => {
            attempts.insert(key, next.clone());
            Ok(())
        }
    }
}

fn usage_item_matches_query(item: &OperationalUsageItem, query: &OperationalUsageQuery) -> bool {
    query.from_ms.is_none_or(|from| item.observed_at_ms >= from)
        && query.to_ms.is_none_or(|to| item.observed_at_ms <= to)
        && query
            .provider_id
            .as_ref()
            .is_none_or(|id| id == &item.provider_id)
        && query
            .channel_id
            .as_ref()
            .is_none_or(|id| id == &item.channel_id)
        && query
            .account_id
            .as_ref()
            .is_none_or(|id| id == &item.account_id)
        && query
            .public_model
            .as_deref()
            .is_none_or(|model| model == item.public_model)
        && query
            .client_key_id
            .as_ref()
            .is_none_or(|id| id == &item.client_key_id)
        && query
            .access_group_id
            .as_ref()
            .is_none_or(|id| item.access_group_id.as_ref() == Some(id))
        && query
            .protocol
            .is_none_or(|protocol| protocol == item.protocol)
}

/// Aggregates final durable Usage events into a bounded Provider/Channel/Account/Model view.
///
/// The aggregation uses the highest numbered Attempt for each Request and admits only a
/// successful Attempt. This keeps retry attempts from being misattributed to the final usage
/// group. Missing token fields retain explicit confidence instead of being treated as zero.
/// Cost remains unpriced until a versioned price catalog exists.
///
/// # Errors
///
/// Returns a safe error for oversized input, conflicting event identities, missing request or
/// Attempt lineage, malformed timestamps, invalid query bounds, or checked-counter overflow.
#[allow(clippy::too_many_lines)]
pub fn compile_operational_usage_page(
    events: &[StoredGatewayEvent],
    query: &OperationalUsageQuery,
) -> Result<OperationalUsagePage, ManagementOperationsError> {
    if events.len() > MAX_USAGE_EVENTS {
        return Err(ManagementOperationsError::SourceUnavailable);
    }
    OperationalUsageQuery::try_new(
        query.from_ms,
        query.to_ms,
        query.provider_id.clone(),
        query.channel_id.clone(),
        query.account_id.clone(),
        query.public_model.clone(),
        query.client_key_id.clone(),
        query.access_group_id.clone(),
        query.protocol,
        query.limit,
        query.cursor.clone(),
    )?;

    let mut requests = BTreeMap::<String, RequestEvent>::new();
    let mut attempts = BTreeMap::<String, AttemptEvent>::new();
    let mut usages = BTreeMap::<String, UsageEvent>::new();
    for stored in events {
        match stored.event() {
            GatewayEvent::Request(event) => {
                let key = event.request_id().as_str().to_owned();
                if let Some(existing) = requests.get(&key) {
                    if existing != event {
                        return Err(ManagementOperationsError::InconsistentConfiguration);
                    }
                } else {
                    requests.insert(key, event.clone());
                }
            }
            GatewayEvent::Attempt(event) => insert_attempt_event(&mut attempts, event)?,
            GatewayEvent::Usage(event) => {
                let key = event.request_id().as_str().to_owned();
                if let Some(existing) = usages.get(&key) {
                    if existing != event {
                        return Err(ManagementOperationsError::InconsistentConfiguration);
                    }
                } else {
                    usages.insert(key, event.clone());
                }
            }
            GatewayEvent::Health(_) | GatewayEvent::Diagnostic(_) => {}
        }
    }

    let mut groups = BTreeMap::<OperationalUsageSortKey, UsageAccumulator>::new();
    let mut observed_through_ms: Option<i64> = None;
    for (request_id, usage) in usages {
        let request = requests
            .get(&request_id)
            .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
        let attempt = attempts
            .get(&request_id)
            .ok_or(ManagementOperationsError::InconsistentConfiguration)?;
        if !matches!(attempt.outcome(), AttemptOutcome::Succeeded) {
            return Err(ManagementOperationsError::InconsistentConfiguration);
        }
        let mut accumulator = UsageAccumulator::new(request, attempt)?;
        let candidate = accumulator.clone().into_item();
        if !usage_item_matches_query(&candidate, query) {
            continue;
        }
        observed_through_ms = Some(
            observed_through_ms.map_or(candidate.observed_at_ms, |current| {
                current.max(candidate.observed_at_ms)
            }),
        );
        accumulator.add_usage(&usage)?;
        let key = candidate.key();
        if let Some(existing) = groups.get_mut(&key) {
            existing.merge(&accumulator)?;
        } else {
            groups.insert(key, accumulator);
        }
    }

    let mut items = groups
        .into_values()
        .map(UsageAccumulator::into_item)
        .collect::<Vec<_>>();
    items.sort_by_key(OperationalUsageItem::key);
    if let Some(cursor) = &query.cursor {
        items.retain(|item| item.key() > cursor.key());
    }
    let has_more = items.len() > query.limit;
    items.truncate(query.limit);
    let next_cursor = has_more
        .then(|| items.last())
        .flatten()
        .map(|item| OperationalUsageCursor {
            provider_id: item.provider_id.clone(),
            channel_id: item.channel_id.clone(),
            account_id: item.account_id.clone(),
            public_model: item.public_model.clone(),
            protocol: item.protocol,
            client_key_id: item.client_key_id.clone(),
            access_group_id: item.access_group_id.clone(),
        });

    Ok(OperationalUsagePage {
        observed_through_ms,
        items,
        next_cursor,
    })
}

/// Safe failures produced while compiling the configured operational inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementOperationsError {
    /// A filter, page size, decoded cursor field, or persisted revision was invalid.
    InvalidQuery,
    /// The cursor was valid but belongs to another Config Version or revision.
    CursorVersionConflict,
    /// A persisted binding violated same-Provider graph ownership.
    InconsistentConfiguration,
    /// The durable runtime observation source is unavailable or exceeded its bound.
    SourceUnavailable,
}

impl fmt::Display for ManagementOperationsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery => formatter.write_str("management operations query is invalid"),
            Self::CursorVersionConflict => {
                formatter.write_str("management operations cursor revision changed")
            }
            Self::InconsistentConfiguration => {
                formatter.write_str("management operations configuration is inconsistent")
            }
            Self::SourceUnavailable => {
                formatter.write_str("management operations observation source is unavailable")
            }
        }
    }
}

impl Error for ManagementOperationsError {}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{
        AccessGroupId, AttemptEvent, AttemptOutcome, AttemptRetryDecision, ClientKeyId,
        CredentialId, EndpointId, GatewayEvent, GatewayProtocol, RequestEvent, RouteCandidateId,
        RouteId, UpstreamId, Usage, UsageEvent,
    };
    use gateway_store::{
        control_plane::{
            ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialConfiguration, CredentialScope, CredentialStatus, EndpointConfiguration,
            EndpointCredentialBindingConfiguration, EndpointTransport, RouteCandidateConfiguration,
            TransformMode, UpstreamConfiguration,
        },
        event_store::SqliteEventStore,
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };

    use super::{
        ManagementOperationsError, OperationalAccountPoolCursor, OperationalAccountPoolQuery,
        OperationalTokenConfidence, OperationalUsageQuery, compile_operational_account_pool_page,
        compile_operational_usage_page,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn inventory_is_secret_free_sorted_filtered_and_route_deduplicated() -> TestResult {
        let configuration = fixture()?;
        let page = compile_operational_account_pool_page(
            &configuration,
            &OperationalAccountPoolQuery {
                account_status: Some(CredentialStatus::Active),
                enabled: Some(true),
                ..OperationalAccountPoolQuery::default()
            },
        )?;

        assert_eq!(page.config_version_id, "version-a");
        assert_eq!(page.revision, 7);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].provider_id.as_str(), "provider-a");
        assert_eq!(page.items[0].channel_id.as_str(), "channel-a");
        assert_eq!(page.items[0].account_id.as_str(), "account-a");
        assert_eq!(page.items[1].account_id.as_str(), "account-b");
        assert_eq!(page.items[0].transport, EndpointTransport::Sse);
        assert_eq!(
            page.items[0]
                .route_ids
                .iter()
                .map(RouteId::as_str)
                .collect::<Vec<_>>(),
            ["route-a"]
        );
        let debug = format!("{page:?}");
        for forbidden in ["secret-a", "ciphertext", "base_url", "https://"] {
            assert!(!debug.contains(forbidden));
        }
        Ok(())
    }

    #[test]
    fn pagination_uses_a_revision_bound_stable_keyset() -> TestResult {
        let configuration = fixture()?;
        let first = compile_operational_account_pool_page(
            &configuration,
            &OperationalAccountPoolQuery::try_new(None, None, None, None, 1, None)?,
        )?;
        assert_eq!(first.items.len(), 1);
        let cursor = first.next_cursor.ok_or("missing next cursor")?;
        let second = compile_operational_account_pool_page(
            &configuration,
            &OperationalAccountPoolQuery::try_new(None, None, None, None, 1, Some(cursor))?,
        )?;
        assert_eq!(second.items.len(), 1);
        assert_ne!(first.items[0].account_id, second.items[0].account_id);

        let stale = OperationalAccountPoolCursor::try_new(
            "version-a",
            6,
            UpstreamId::try_new("provider-a")?,
            EndpointId::try_new("channel-a")?,
            CredentialId::try_new("account-a")?,
        )?;
        assert_eq!(
            compile_operational_account_pool_page(
                &configuration,
                &OperationalAccountPoolQuery::try_new(None, None, None, None, 1, Some(stale))?
            ),
            Err(ManagementOperationsError::CursorVersionConflict)
        );
        Ok(())
    }

    #[test]
    fn cross_provider_binding_fails_closed() -> TestResult {
        let mut configuration = fixture()?;
        configuration.endpoint_credential_bindings[0].upstream_id =
            UpstreamId::try_new("provider-b")?;
        assert_eq!(
            compile_operational_account_pool_page(
                &configuration,
                &OperationalAccountPoolQuery::default()
            ),
            Err(ManagementOperationsError::InconsistentConfiguration)
        );
        Ok(())
    }

    #[test]
    fn usage_aggregation_is_grouped_paginated_and_explicitly_unpriced() -> TestResult {
        let mut store = SqliteEventStore::open_in_memory()?;
        let request_one = RequestEvent::new(
            gateway_core::RequestId::try_new("usage-request-a")?,
            ClientKeyId::try_new("client-a")?,
            Some(AccessGroupId::try_new("group-a")?),
            GatewayProtocol::OpenAiResponses,
            "public-model".to_owned(),
            "public-model".to_owned(),
            None,
            false,
        );
        let request_two = RequestEvent::new(
            gateway_core::RequestId::try_new("usage-request-b")?,
            ClientKeyId::try_new("client-a")?,
            Some(AccessGroupId::try_new("group-a")?),
            GatewayProtocol::OpenAiResponses,
            "public-model".to_owned(),
            "public-model".to_owned(),
            None,
            false,
        );
        let attempt = |request_id: &gateway_core::RequestId,
                       number: u64,
                       ended_at_ms: i64|
         -> Result<AttemptEvent, Box<dyn Error>> {
            Ok(AttemptEvent::new(
                request_id.clone(),
                number,
                RouteId::try_new("usage-route")?,
                RouteCandidateId::try_new("usage-candidate")?,
                CredentialId::try_new("usage-account")?,
                EndpointId::try_new("usage-channel")?,
                UpstreamId::try_new("usage-provider")?,
                "private-upstream-model".to_owned(),
                ended_at_ms - 1,
                ended_at_ms,
                AttemptOutcome::Succeeded,
                AttemptRetryDecision::Completed,
            ))
        };
        let usage_one = UsageEvent::from_usage(
            request_one.request_id().clone(),
            gateway_core::ResponseId::try_new("usage-response-a")?,
            &Usage {
                input_tokens: Some(3),
                output_tokens: Some(5),
                ..Usage::default()
            },
        );
        let usage_two = UsageEvent::from_usage(
            request_two.request_id().clone(),
            gateway_core::ResponseId::try_new("usage-response-b")?,
            &Usage {
                input_tokens: Some(7),
                ..Usage::default()
            },
        );
        store.append_batch(&[
            GatewayEvent::Request(request_one.clone()),
            GatewayEvent::Attempt(attempt(request_one.request_id(), 1, 100)?),
            GatewayEvent::Usage(usage_one),
            GatewayEvent::Request(request_two.clone()),
            GatewayEvent::Attempt(attempt(request_two.request_id(), 1, 200)?),
            GatewayEvent::Usage(usage_two),
        ])?;
        let events = store.list_events()?;
        let first = compile_operational_usage_page(
            &events,
            &OperationalUsageQuery::try_new(
                Some(100),
                Some(200),
                None,
                None,
                None,
                Some("public-model".to_owned()),
                None,
                None,
                Some(GatewayProtocol::OpenAiResponses),
                1,
                None,
            )?,
        )?;
        assert_eq!(first.items.len(), 1);
        assert_eq!(first.items[0].request_count, 2);
        assert_eq!(first.items[0].input_tokens.total, Some(10));
        assert_eq!(
            first.items[0].output_tokens.confidence,
            OperationalTokenConfidence::Partial
        );
        assert_eq!(first.items[0].cost_microunits, None);
        assert_eq!(first.items[0].observed_at_ms, 200);
        assert!(first.next_cursor.is_none());
        assert_eq!(first.observed_through_ms, Some(200));
        Ok(())
    }

    fn fixture() -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
        let version_id = ConfigVersionId::try_new("version-a")?;
        let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
            id: version_id.clone(),
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 7,
            created_at_ms: 1,
            description: "P13-04A fixture".to_owned(),
        });
        configuration.upstreams.push(UpstreamConfiguration {
            id: UpstreamId::try_new("provider-a")?,
            name: "Provider A".to_owned(),
            kind: "openai-compatible".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: None,
        });
        configuration.endpoints.push(EndpointConfiguration {
            id: EndpointId::try_new("channel-a")?,
            upstream_id: UpstreamId::try_new("provider-a")?,
            adapter_id: "provider.responses".to_owned(),
            api_format: "openai/responses".to_owned(),
            base_url: "https://must-not-escape.example".to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: None,
            transport: EndpointTransport::Sse,
            enabled: true,
        });
        let key_version = KeyVersion::try_new(1)?;
        let key_ring = MasterKeyRing::try_new(
            key_version,
            [(key_version, MasterKey::try_from_bytes([0x51_u8; 32])?)],
        )?;
        let store = SecretStore::new(key_ring);
        for (id, status) in [
            ("account-b", CredentialStatus::Active),
            ("account-a", CredentialStatus::Active),
            ("account-c", CredentialStatus::Unauthorized),
        ] {
            configuration.credentials.push(CredentialConfiguration {
                id: CredentialId::try_new(id)?,
                upstream_id: UpstreamId::try_new("provider-a")?,
                kind: "oauth_json".to_owned(),
                encrypted_secret: store.seal(b"secret-a", b"p13-04a-fixture")?,
                status,
                revision: 2,
            });
            configuration.endpoint_credential_bindings.push(
                EndpointCredentialBindingConfiguration {
                    endpoint_id: EndpointId::try_new("channel-a")?,
                    credential_id: CredentialId::try_new(id)?,
                    upstream_id: UpstreamId::try_new("provider-a")?,
                    enabled: id != "account-c",
                    priority: 0,
                    weight: 1,
                    concurrency: 2,
                },
            );
        }
        for candidate_id in ["candidate-b", "candidate-a"] {
            configuration
                .route_candidates
                .push(RouteCandidateConfiguration {
                    id: RouteCandidateId::try_new(candidate_id)?,
                    route_id: RouteId::try_new("route-a")?,
                    endpoint_id: EndpointId::try_new("channel-a")?,
                    upstream_model: "model".to_owned(),
                    credential_scope: CredentialScope::EndpointBindings,
                    transform_mode: TransformMode::Canonical,
                    enabled: true,
                    priority: 0,
                    weight: 1,
                    capability_override_json: "{}".to_owned(),
                });
        }
        Ok(configuration)
    }
}
