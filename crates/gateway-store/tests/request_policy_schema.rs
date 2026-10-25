//! #71 upgrades the unchanged schema40 prefix and preserves existing encrypted configuration.
#![deny(unsafe_code)]
use gateway_core::{CredentialId, EndpointId, UpstreamId};
use gateway_store::{
    control_plane::*, migrate, open, rollback_to_version, schema_version, secret_store::*,
};
use std::error::Error;

#[test]
fn request_policy_schema40_upgrade_preserves_configuration_and_ciphertext()
-> Result<(), Box<dyn Error>> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../output/issue-71/state")
        .join(format!(
            "migration-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
    std::fs::create_dir_all(&root)?;
    let database = root.join("control.sqlite3");
    let key = KeyVersion::try_new(1)?;
    let secrets = SecretStore::new(MasterKeyRing::try_new(
        key,
        [(key, MasterKey::try_from_bytes([0x71; 32])?)],
    )?);
    let version = ConfigVersionId::try_new("retained")?;
    let mut config = ControlPlaneConfiguration::new(ConfigVersion {
        id: version.clone(),
        parent_id: None,
        status: ConfigVersionStatus::Draft,
        revision: 0,
        created_at_ms: 1,
        description: "schema40 policy upgrade".into(),
    });
    let owner = UpstreamId::try_new("upstream")?;
    config.upstreams.push(UpstreamConfiguration {
        id: owner.clone(),
        name: "retained".into(),
        kind: "openai-compatible".into(),
        enabled: true,
        tags_json: "[]".into(),
        egress_policy_id: None,
    });
    config.endpoints.push(EndpointConfiguration {
        request_policy: gateway_core::request_policy::RequestPolicy::default(),
        id: EndpointId::try_new("endpoint")?,
        upstream_id: owner.clone(),
        adapter_id: "openai-compatible.responses".into(),
        api_format: "openai/responses".into(),
        base_url: "https://synthetic.invalid".into(),
        inference_path: "/responses".into(),
        models_path: None,
        transport: EndpointTransport::Http,
        enabled: true,
    });
    config.credentials.push(CredentialConfiguration {
        id: CredentialId::try_new("account")?,
        upstream_id: owner,
        kind: "bearer".into(),
        encrypted_secret: secrets.seal(b"synthetic-retained-account", b"retained-aad")?,
        status: CredentialStatus::Active,
        revision: 7,
    });
    SqliteControlPlaneRepository::open(&database)?.write_configuration(&config)?;
    let mut connection = open(&database)?;
    rollback_to_version(&mut connection, 40)?;
    assert_eq!(schema_version(&connection)?, Some(40));
    let ciphertext: Vec<u8> =
        connection.query_row("SELECT ciphertext FROM upstream_credentials", [], |r| {
            r.get(0)
        })?;
    migrate(&mut connection)?;
    assert_eq!(
        schema_version(&connection)?,
        Some(gateway_store::CURRENT_SCHEMA_VERSION)
    );
    let after: Vec<u8> =
        connection.query_row("SELECT ciphertext FROM upstream_credentials", [], |r| {
            r.get(0)
        })?;
    assert_eq!(after, ciphertext);
    let loaded = SqliteControlPlaneRepository::open(&database)?
        .load_configuration(&version)?
        .ok_or("retained")?;
    assert!(loaded.endpoints[0].request_policy.is_empty());
    assert_eq!(loaded.credentials[0].revision, 7);
    assert_eq!(
        secrets
            .open(&loaded.credentials[0].encrypted_secret, b"retained-aad")?
            .as_bytes(),
        b"synthetic-retained-account"
    );
    let versions: Vec<i64> = connection
        .prepare("SELECT version FROM schema_migrations ORDER BY version")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    assert_eq!(
        versions,
        (1..=gateway_store::CURRENT_SCHEMA_VERSION).collect::<Vec<_>>()
    );
    Ok(())
}
