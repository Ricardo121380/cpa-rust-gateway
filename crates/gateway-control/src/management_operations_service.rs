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

use gateway_core::{CredentialId, EgressPolicyId, EndpointId, RouteId, UpstreamId};
use gateway_store::control_plane::{
    ControlPlaneConfiguration, CredentialConfiguration, CredentialStatus, EndpointConfiguration,
    EndpointCredentialBindingConfiguration, EndpointTransport, UpstreamConfiguration,
};

/// Default number of configured account bindings returned by one inventory page.
pub const DEFAULT_ACCOUNT_POOL_LIMIT: usize = 50;
/// Maximum number of configured account bindings returned by one inventory page.
pub const MAX_ACCOUNT_POOL_LIMIT: usize = 100;

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

/// Safe failures produced while compiling the configured operational inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementOperationsError {
    /// A filter, page size, decoded cursor field, or persisted revision was invalid.
    InvalidQuery,
    /// The cursor was valid but belongs to another Config Version or revision.
    CursorVersionConflict,
    /// A persisted binding violated same-Provider graph ownership.
    InconsistentConfiguration,
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
        }
    }
}

impl Error for ManagementOperationsError {}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{CredentialId, EndpointId, RouteCandidateId, RouteId, UpstreamId};
    use gateway_store::{
        control_plane::{
            ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialConfiguration, CredentialScope, CredentialStatus, EndpointConfiguration,
            EndpointCredentialBindingConfiguration, EndpointTransport, RouteCandidateConfiguration,
            TransformMode, UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };

    use super::{
        ManagementOperationsError, OperationalAccountPoolCursor, OperationalAccountPoolQuery,
        compile_operational_account_pool_page,
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
