//! Management-time compilation of encrypted Credential bindings into runtime pools.
//!
//! This module is the only P3-04 bridge from a persisted control-plane graph to a runtime
//! [`gateway_upstream::EndpointCredentialPools`] value. It decrypts each active binding before
//! construction; request-time selection receives only the resulting in-memory pool and never a
//! `SQLite` repository or `SecretStore`.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use gateway_core::{CredentialId, EndpointId, UpstreamId};
use gateway_store::{
    control_plane::{
        ControlPlaneConfiguration, CredentialConfiguration, CredentialStatus,
        EndpointConfiguration, EndpointCredentialBindingConfiguration, UpstreamConfiguration,
    },
    secret_store::{SecretStore, SecretStoreError},
};
use gateway_upstream::{
    CredentialPoolBuildError, CredentialSecret, EndpointCredentialInput, EndpointCredentialPool,
    EndpointCredentialPools,
};

use crate::control_plane_service::{ControlPlaneServiceError, credential_associated_data};

/// Compiles control-plane Credentials into Endpoint-local runtime pools.
///
/// The compiler borrows the externally provisioned AEAD key material only for this control-path
/// operation. It retains no decrypted material itself; successful material moves into redacted,
/// zeroizing pool entries.
pub struct CredentialPoolCompiler<'store> {
    secret_store: &'store SecretStore,
}

impl<'store> CredentialPoolCompiler<'store> {
    /// Creates a management-time compiler using the active external Secret Store.
    #[must_use]
    pub const fn new(secret_store: &'store SecretStore) -> Self {
        Self { secret_store }
    }

    /// Decrypts every active, enabled Endpoint/Credential binding into an immutable pool set.
    ///
    /// Disabled upstreams/endpoints/bindings and non-active Credential states do not enter the
    /// runtime set. Their structural references are still checked before they are ignored, so a
    /// malformed graph cannot be hidden by disabling one row.
    ///
    /// # Errors
    ///
    /// Returns a safe [`CredentialPoolCompileError`] for malformed references, AAD construction,
    /// AEAD authentication, or a bounded pool validation failure. No partial pool set is returned.
    pub fn compile(
        &self,
        configuration: &ControlPlaneConfiguration,
    ) -> Result<EndpointCredentialPools, CredentialPoolCompileError> {
        self.compile_excluding_endpoints(configuration, &BTreeSet::new())
    }

    /// Compiles ordinary control-plane Credentials while leaving explicitly native Endpoint
    /// identities to their provider-owned account pool compiler.
    ///
    /// # Errors
    ///
    /// Returns the same bounded compile failures as [`Self::compile`], including malformed
    /// references, authenticated-secret failures, and invalid Endpoint pool metadata.
    pub fn compile_excluding_endpoints(
        &self,
        configuration: &ControlPlaneConfiguration,
        excluded_endpoints: &BTreeSet<EndpointId>,
    ) -> Result<EndpointCredentialPools, CredentialPoolCompileError> {
        let upstreams = index_upstreams(&configuration.upstreams)?;
        let endpoints = index_endpoints(&configuration.endpoints, &upstreams)?;
        let credentials = index_credentials(&configuration.credentials, &upstreams)?;
        let mut inputs_by_endpoint: BTreeMap<EndpointId, Vec<EndpointCredentialInput>> =
            BTreeMap::new();
        let mut binding_pairs = BTreeSet::new();

        for binding in &configuration.endpoint_credential_bindings {
            let endpoint = endpoints
                .get(&binding.endpoint_id)
                .ok_or(CredentialPoolCompileError::MissingBindingEndpoint)?;
            let credential = credentials
                .get(&binding.credential_id)
                .ok_or(CredentialPoolCompileError::MissingBindingCredential)?;
            let endpoint_upstream = upstreams
                .get(&endpoint.upstream_id)
                .ok_or(CredentialPoolCompileError::MissingEndpointUpstream)?;
            let credential_upstream = upstreams
                .get(&credential.upstream_id)
                .ok_or(CredentialPoolCompileError::MissingCredentialUpstream)?;

            if !binding_pairs.insert((binding.endpoint_id.clone(), binding.credential_id.clone())) {
                return Err(CredentialPoolCompileError::DuplicateBinding);
            }
            validate_binding_ownership(binding, endpoint, credential)?;
            if excluded_endpoints.contains(&binding.endpoint_id) {
                continue;
            }
            if !endpoint_upstream.enabled
                || !credential_upstream.enabled
                || !endpoint.enabled
                || !binding.enabled
                || credential.status != CredentialStatus::Active
            {
                continue;
            }

            let associated_data = credential_associated_data(
                &configuration.version.id,
                &credential.id,
                &credential.upstream_id,
            )?;
            let plaintext = self
                .secret_store
                .open(&credential.encrypted_secret, &associated_data)?;
            let secret = CredentialSecret::try_new(plaintext.as_bytes().to_vec())?;
            let input = EndpointCredentialInput {
                credential_id: credential.id.clone(),
                credential_kind: credential.kind.clone(),
                credential_revision: credential.revision,
                priority: binding.priority,
                weight: binding.weight,
                concurrency: binding.concurrency,
                secret,
            };
            inputs_by_endpoint
                .entry(endpoint.id.clone())
                .or_default()
                .push(input);
        }

        let pools = inputs_by_endpoint
            .into_iter()
            .map(|(endpoint_id, inputs)| EndpointCredentialPool::try_new(endpoint_id, inputs))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EndpointCredentialPools::try_new(pools)?)
    }
}

impl fmt::Debug for CredentialPoolCompiler<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialPoolCompiler")
            .field("secret_store", &"<redacted>")
            .finish()
    }
}

fn index_upstreams(
    upstreams: &[UpstreamConfiguration],
) -> Result<BTreeMap<UpstreamId, &UpstreamConfiguration>, CredentialPoolCompileError> {
    let mut indexed = BTreeMap::new();
    for upstream in upstreams {
        if indexed.insert(upstream.id.clone(), upstream).is_some() {
            return Err(CredentialPoolCompileError::DuplicateUpstream);
        }
    }
    Ok(indexed)
}

fn index_endpoints<'a>(
    endpoints: &'a [EndpointConfiguration],
    upstreams: &BTreeMap<UpstreamId, &UpstreamConfiguration>,
) -> Result<BTreeMap<EndpointId, &'a EndpointConfiguration>, CredentialPoolCompileError> {
    let mut indexed = BTreeMap::new();
    for endpoint in endpoints {
        if !upstreams.contains_key(&endpoint.upstream_id) {
            return Err(CredentialPoolCompileError::MissingEndpointUpstream);
        }
        if indexed.insert(endpoint.id.clone(), endpoint).is_some() {
            return Err(CredentialPoolCompileError::DuplicateEndpoint);
        }
    }
    Ok(indexed)
}

fn index_credentials<'a>(
    credentials: &'a [CredentialConfiguration],
    upstreams: &BTreeMap<UpstreamId, &UpstreamConfiguration>,
) -> Result<BTreeMap<CredentialId, &'a CredentialConfiguration>, CredentialPoolCompileError> {
    let mut indexed = BTreeMap::new();
    for credential in credentials {
        if !upstreams.contains_key(&credential.upstream_id) {
            return Err(CredentialPoolCompileError::MissingCredentialUpstream);
        }
        if indexed.insert(credential.id.clone(), credential).is_some() {
            return Err(CredentialPoolCompileError::DuplicateCredential);
        }
    }
    Ok(indexed)
}

fn validate_binding_ownership(
    binding: &EndpointCredentialBindingConfiguration,
    endpoint: &EndpointConfiguration,
    credential: &CredentialConfiguration,
) -> Result<(), CredentialPoolCompileError> {
    if binding.upstream_id != endpoint.upstream_id || binding.upstream_id != credential.upstream_id
    {
        return Err(CredentialPoolCompileError::BindingUpstreamMismatch);
    }
    Ok(())
}

/// Safe failures during control-path Credential pool construction.
#[derive(Debug)]
pub enum CredentialPoolCompileError {
    /// More than one configured Upstream used the same stable identity.
    DuplicateUpstream,
    /// More than one configured Endpoint used the same stable identity.
    DuplicateEndpoint,
    /// More than one configured Credential used the same stable identity.
    DuplicateCredential,
    /// More than one binding targeted the same Endpoint/Credential pair.
    DuplicateBinding,
    /// A binding referred to no configured Endpoint.
    MissingBindingEndpoint,
    /// A binding referred to no configured Credential.
    MissingBindingCredential,
    /// An Endpoint referred to no configured owning Upstream.
    MissingEndpointUpstream,
    /// A Credential referred to no configured owning Upstream.
    MissingCredentialUpstream,
    /// The binding, Endpoint, and Credential did not share one Upstream.
    BindingUpstreamMismatch,
    /// The stable Credential AAD could not be represented safely.
    AssociatedData(ControlPlaneServiceError),
    /// AEAD authentication or key lookup failed while decrypting one Credential.
    SecretStore(SecretStoreError),
    /// A decrypted runtime input could not form a bounded Credential pool.
    Pool(CredentialPoolBuildError),
}

impl fmt::Display for CredentialPoolCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::DuplicateUpstream => "Credential pool configuration has a duplicate Upstream",
            Self::DuplicateEndpoint => "Credential pool configuration has a duplicate Endpoint",
            Self::DuplicateCredential => "Credential pool configuration has a duplicate Credential",
            Self::DuplicateBinding => {
                "Credential pool configuration has a duplicate Endpoint/Credential binding"
            }
            Self::MissingBindingEndpoint => "Credential binding refers to an unknown Endpoint",
            Self::MissingBindingCredential => "Credential binding refers to an unknown Credential",
            Self::MissingEndpointUpstream => "Credential Endpoint refers to an unknown Upstream",
            Self::MissingCredentialUpstream => "Credential refers to an unknown Upstream",
            Self::BindingUpstreamMismatch => {
                "Credential binding, Endpoint, and Credential do not share an Upstream"
            }
            Self::AssociatedData(_) => "Credential associated data is invalid",
            Self::SecretStore(_) => "Credential decryption failed",
            Self::Pool(_) => "Credential runtime pool is invalid",
        };
        formatter.write_str(message)
    }
}

impl Error for CredentialPoolCompileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AssociatedData(error) => Some(error),
            Self::SecretStore(error) => Some(error),
            Self::Pool(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ControlPlaneServiceError> for CredentialPoolCompileError {
    fn from(error: ControlPlaneServiceError) -> Self {
        Self::AssociatedData(error)
    }
}

impl From<SecretStoreError> for CredentialPoolCompileError {
    fn from(error: SecretStoreError) -> Self {
        Self::SecretStore(error)
    }
}

impl From<CredentialPoolBuildError> for CredentialPoolCompileError {
    fn from(error: CredentialPoolBuildError) -> Self {
        Self::Pool(error)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_core::{CredentialId, EndpointId, UpstreamId};
    use gateway_store::{
        control_plane::{
            ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
            CredentialConfiguration, CredentialStatus, EndpointConfiguration,
            EndpointCredentialBindingConfiguration, EndpointTransport, UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };

    use super::{CredentialPoolCompileError, CredentialPoolCompiler};
    use crate::control_plane_service::credential_associated_data;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn compiles_active_decrypted_bindings_without_exposing_secrets() -> TestResult {
        let secret_store = secret_store()?;
        let configuration = configuration(&secret_store)?;
        let pools = CredentialPoolCompiler::new(&secret_store).compile(&configuration)?;
        let endpoint_id = EndpointId::try_new("endpoint-a")?;
        let lease = pools
            .try_lease(&endpoint_id)
            .ok_or("expected an active Credential lease")?;

        assert_eq!(lease.credential_id().as_str(), "credential-a");
        assert_eq!(lease.credential_kind(), "api_key");
        assert_eq!(lease.credential_revision(), 7);
        assert_eq!(lease.secret_bytes(), b"synthetic-credential-secret");
        let debug = format!("{pools:?} {lease:?}");
        assert!(!debug.contains("synthetic-credential-secret"));
        Ok(())
    }

    #[test]
    fn rejects_aad_mismatch_before_any_runtime_pool_is_returned() -> TestResult {
        let secret_store = secret_store()?;
        let mut configuration = configuration(&secret_store)?;
        configuration.version.id = ConfigVersionId::try_new("version-b")?;

        let result = CredentialPoolCompiler::new(&secret_store).compile(&configuration);
        assert!(matches!(
            result,
            Err(CredentialPoolCompileError::SecretStore(_))
        ));
        Ok(())
    }

    #[test]
    fn inactive_credentials_are_excluded_without_a_runtime_store_lookup() -> TestResult {
        let secret_store = secret_store()?;
        let mut configuration = configuration(&secret_store)?;
        configuration.credentials[0].status = CredentialStatus::Cooling;

        let pools = CredentialPoolCompiler::new(&secret_store).compile(&configuration)?;
        assert_eq!(pools.endpoint_count(), 0);
        assert!(
            pools
                .try_lease(&EndpointId::try_new("endpoint-a")?)
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn rejects_orphaned_inactive_records_before_returning_any_pool() -> TestResult {
        let secret_store = secret_store()?;
        let mut endpoint_configuration = configuration(&secret_store)?;
        let mut orphaned_endpoint = endpoint_configuration.endpoints[0].clone();
        orphaned_endpoint.id = EndpointId::try_new("endpoint-orphaned")?;
        orphaned_endpoint.upstream_id = UpstreamId::try_new("upstream-missing")?;
        orphaned_endpoint.enabled = false;
        endpoint_configuration.endpoints.push(orphaned_endpoint);
        assert!(matches!(
            CredentialPoolCompiler::new(&secret_store).compile(&endpoint_configuration),
            Err(CredentialPoolCompileError::MissingEndpointUpstream)
        ));

        let mut credential_configuration = configuration(&secret_store)?;
        let orphaned_credential_id = CredentialId::try_new("credential-orphaned")?;
        let orphaned_upstream_id = UpstreamId::try_new("upstream-missing")?;
        let orphaned_associated_data = credential_associated_data(
            &credential_configuration.version.id,
            &orphaned_credential_id,
            &orphaned_upstream_id,
        )?;
        credential_configuration
            .credentials
            .push(CredentialConfiguration {
                id: orphaned_credential_id,
                upstream_id: orphaned_upstream_id,
                kind: "api_key".to_owned(),
                encrypted_secret: secret_store
                    .seal(b"synthetic-orphaned-credential", &orphaned_associated_data)?,
                status: CredentialStatus::Cooling,
                revision: 0,
            });
        assert!(matches!(
            CredentialPoolCompiler::new(&secret_store).compile(&credential_configuration),
            Err(CredentialPoolCompileError::MissingCredentialUpstream)
        ));
        Ok(())
    }

    #[test]
    fn rejects_duplicate_binding_before_decrypting_or_returning_a_pool() -> TestResult {
        let secret_store = secret_store()?;
        let mut configuration = configuration(&secret_store)?;
        configuration
            .endpoint_credential_bindings
            .push(configuration.endpoint_credential_bindings[0].clone());

        assert!(matches!(
            CredentialPoolCompiler::new(&secret_store).compile(&configuration),
            Err(CredentialPoolCompileError::DuplicateBinding)
        ));
        Ok(())
    }

    fn configuration(
        secret_store: &SecretStore,
    ) -> Result<ControlPlaneConfiguration, Box<dyn Error>> {
        let version = ConfigVersion {
            id: ConfigVersionId::try_new("version-a")?,
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 0,
            description: "P3-04 Credential pool fixture".to_owned(),
        };
        let mut configuration = ControlPlaneConfiguration::new(version);
        let upstream_id = UpstreamId::try_new("upstream-a")?;
        let endpoint_id = EndpointId::try_new("endpoint-a")?;
        let credential_id = CredentialId::try_new("credential-a")?;
        configuration.upstreams.push(UpstreamConfiguration {
            id: upstream_id.clone(),
            name: "Synthetic Upstream".to_owned(),
            kind: "openai-compatible".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: None,
        });
        configuration.endpoints.push(EndpointConfiguration {
            id: endpoint_id.clone(),
            upstream_id: upstream_id.clone(),
            adapter_id: "openai-compatible".to_owned(),
            api_format: "openai_responses".to_owned(),
            base_url: "https://relay.example".to_owned(),
            inference_path: "/responses".to_owned(),
            models_path: None,
            transport: EndpointTransport::Http,
            enabled: true,
        });
        let associated_data =
            credential_associated_data(&configuration.version.id, &credential_id, &upstream_id)?;
        configuration.credentials.push(CredentialConfiguration {
            id: credential_id.clone(),
            upstream_id: upstream_id.clone(),
            kind: "api_key".to_owned(),
            encrypted_secret: secret_store
                .seal(b"synthetic-credential-secret", &associated_data)?,
            status: CredentialStatus::Active,
            revision: 7,
        });
        configuration
            .endpoint_credential_bindings
            .push(EndpointCredentialBindingConfiguration {
                endpoint_id,
                credential_id,
                upstream_id,
                enabled: true,
                priority: 0,
                weight: 1,
                concurrency: 2,
            });
        Ok(configuration)
    }

    fn secret_store() -> Result<SecretStore, Box<dyn Error>> {
        let version = KeyVersion::try_new(1)?;
        Ok(SecretStore::new(MasterKeyRing::try_new(
            version,
            [(version, MasterKey::try_from_bytes([0x44_u8; 32])?)],
        )?))
    }
}
