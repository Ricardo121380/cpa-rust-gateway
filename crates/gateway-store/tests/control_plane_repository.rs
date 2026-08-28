//! Black-box integration coverage for the versioned control-plane Repository.

use std::error::Error;

use gateway_core::{
    AccessGroupId, ClientKeyId, CredentialId, EgressPolicyId, EndpointId, PublicModelId,
    RouteCandidateId, RouteId, UpstreamId,
};
use gateway_store::{
    control_plane::{
        AccessGroupConfiguration, AccessGroupRouteConfiguration, AdministrativeStatus,
        ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
        CredentialConfiguration, CredentialScope, CredentialStatus, EgressPolicyConfiguration,
        EndpointConfiguration, EndpointCredentialBindingConfiguration, EndpointTransport,
        ModelAliasConfiguration, ModelRouteConfiguration, PublicModelConfiguration,
        RouteCandidateConfiguration, RoutePolicy, SqliteControlPlaneRepository, StoredClientKey,
        StoredClientKeyStatus, StoredEgressRedirectMode, TransformMode, UpstreamConfiguration,
    },
    secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn repository_round_trips_every_version_scoped_p2_table_without_secret_leakage() -> TestResult {
    let key_version = KeyVersion::try_new(1)?;
    let secret_store = SecretStore::new(MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([0x31_u8; 32])?)],
    )?);
    let version_a = ConfigVersionId::try_new("version-a")?;
    let version_b = ConfigVersionId::try_new("version-b")?;
    let mut repository = SqliteControlPlaneRepository::open_in_memory()?;

    repository.write_configuration(&full_configuration(
        &secret_store,
        &version_a,
        "public-a",
        b"credential-for-version-a",
        0xA1,
    )?)?;
    repository.write_configuration(&full_configuration(
        &secret_store,
        &version_b,
        "public-b",
        b"credential-for-version-b",
        0xB2,
    )?)?;

    let loaded_a = repository
        .load_configuration(&version_a)?
        .ok_or("version-a configuration was not found")?;
    assert_eq!(loaded_a.version.id, version_a);
    assert_eq!(loaded_a.egress_policies.len(), 1);
    assert_eq!(loaded_a.upstreams.len(), 1);
    assert_eq!(loaded_a.endpoints.len(), 1);
    assert_eq!(loaded_a.credentials.len(), 1);
    assert_eq!(loaded_a.endpoint_credential_bindings.len(), 1);
    assert_eq!(loaded_a.public_models.len(), 1);
    assert_eq!(loaded_a.model_aliases.len(), 1);
    assert_eq!(loaded_a.model_routes.len(), 1);
    assert_eq!(loaded_a.route_candidates.len(), 1);
    assert_eq!(loaded_a.access_groups.len(), 1);
    assert_eq!(loaded_a.access_group_routes.len(), 1);
    assert_eq!(loaded_a.client_keys.len(), 1);
    assert_eq!(loaded_a.public_models[0].model_name, "public-a");
    assert_eq!(loaded_a.egress_policies[0].id.as_str(), "egress-policy-a");
    assert_eq!(
        loaded_a.upstreams[0]
            .egress_policy_id
            .as_ref()
            .map(EgressPolicyId::as_str),
        Some("egress-policy-a")
    );
    assert_eq!(
        loaded_a.credentials[0].encrypted_secret.key_version(),
        key_version
    );
    assert_eq!(loaded_a.client_keys[0].secret_digest(), &[0xA1_u8; 32]);

    let loaded_b = repository
        .load_configuration(&version_b)?
        .ok_or("version-b configuration was not found")?;
    assert_eq!(loaded_b.public_models[0].model_name, "public-b");
    assert_eq!(loaded_b.client_keys[0].secret_digest(), &[0xB2_u8; 32]);

    let debug = format!("{loaded_a:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("credential-for-version-a"));
    assert!(!debug.contains("161"));
    Ok(())
}

fn full_configuration(
    secret_store: &SecretStore,
    version_id: &ConfigVersionId,
    public_model_name: &str,
    credential_plaintext: &[u8],
    digest_byte: u8,
) -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
    let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
        id: version_id.clone(),
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision: 0,
        created_at_ms: 1,
        description: format!("repository fixture for {public_model_name}"),
    });
    configuration.egress_policies.push(default_egress_policy()?);
    configuration.upstreams.push(UpstreamConfiguration {
        id: UpstreamId::try_new("upstream-a")?,
        name: "station-a".to_owned(),
        kind: "openai-compatible".to_owned(),
        enabled: true,
        tags_json: "[\"integration\"]".to_owned(),
        egress_policy_id: Some(EgressPolicyId::try_new("egress-policy-a")?),
    });
    configuration.endpoints.push(EndpointConfiguration {
        id: EndpointId::try_new("endpoint-a")?,
        upstream_id: UpstreamId::try_new("upstream-a")?,
        adapter_id: "openai-compatible.responses".to_owned(),
        api_format: "openai/responses".to_owned(),
        base_url: "https://station.example/v1".to_owned(),
        inference_path: "/responses".to_owned(),
        models_path: Some("/models".to_owned()),
        transport: EndpointTransport::Http,
        enabled: true,
    });
    configuration.credentials.push(CredentialConfiguration {
        id: CredentialId::try_new("credential-a")?,
        upstream_id: UpstreamId::try_new("upstream-a")?,
        kind: "api_key".to_owned(),
        encrypted_secret: secret_store.seal(credential_plaintext, b"repository-fixture-aad")?,
        status: CredentialStatus::Active,
        revision: 0,
    });
    configuration
        .endpoint_credential_bindings
        .push(EndpointCredentialBindingConfiguration {
            endpoint_id: EndpointId::try_new("endpoint-a")?,
            credential_id: CredentialId::try_new("credential-a")?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            enabled: true,
            priority: 0,
            weight: 1,
            concurrency: 4,
        });
    configuration.public_models.push(PublicModelConfiguration {
        id: PublicModelId::try_new("public-model-a")?,
        model_name: public_model_name.to_owned(),
        status: AdministrativeStatus::Active,
        display_name: "Integration Model".to_owned(),
        capabilities_json: "{\"tools\":true}".to_owned(),
    });
    configuration.model_aliases.push(ModelAliasConfiguration {
        alias: "model-a".to_owned(),
        public_model_id: PublicModelId::try_new("public-model-a")?,
    });
    configuration.model_routes.push(ModelRouteConfiguration {
        id: RouteId::try_new("route-a")?,
        public_model_id: PublicModelId::try_new("public-model-a")?,
        policy: RoutePolicy::SmoothWeightedRoundRobin,
        max_attempts: 3,
        bootstrap_timeout_ms: 20_000,
    });
    configuration
        .route_candidates
        .push(RouteCandidateConfiguration {
            id: RouteCandidateId::try_new("candidate-a")?,
            route_id: RouteId::try_new("route-a")?,
            endpoint_id: EndpointId::try_new("endpoint-a")?,
            upstream_model: "upstream-model-a".to_owned(),
            credential_scope: CredentialScope::EndpointBindings,
            transform_mode: TransformMode::Canonical,
            enabled: true,
            priority: 0,
            weight: 100,
            capability_override_json: "{}".to_owned(),
        });
    add_access_graph(&mut configuration, digest_byte)?;
    Ok(configuration)
}

fn add_access_graph(
    configuration: &mut ControlPlaneConfiguration,
    digest_byte: u8,
) -> Result<(), Box<dyn Error>> {
    configuration.access_groups.push(AccessGroupConfiguration {
        id: AccessGroupId::try_new("access-group-a")?,
        name: "default".to_owned(),
        status: AdministrativeStatus::Active,
        limits_json: "{\"max_concurrency\":4}".to_owned(),
    });
    configuration
        .access_group_routes
        .push(AccessGroupRouteConfiguration {
            access_group_id: AccessGroupId::try_new("access-group-a")?,
            route_id: RouteId::try_new("route-a")?,
            enabled: true,
        });
    configuration.client_keys.push(StoredClientKey::try_new(
        ClientKeyId::try_new("client-key-a")?,
        AccessGroupId::try_new("access-group-a")?,
        "rgw_0123456789abcdef",
        [digest_byte; 32],
        StoredClientKeyStatus::Active,
        None,
    )?);
    Ok(())
}

fn default_egress_policy() -> Result<EgressPolicyConfiguration, Box<dyn Error>> {
    Ok(EgressPolicyConfiguration {
        id: EgressPolicyId::try_new("egress-policy-a")?,
        name: "default-egress".to_owned(),
        allowed_schemes_json: r#"["https"]"#.to_owned(),
        allowed_hosts_json: r#"["station.example"]"#.to_owned(),
        allowed_ports_json: "[443]".to_owned(),
        allowed_cidrs_json: "[]".to_owned(),
        redirect_mode: StoredEgressRedirectMode::Deny,
        max_redirects: 0,
    })
}
