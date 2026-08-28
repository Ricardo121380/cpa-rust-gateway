//! Management-only transactions that connect Secret sealing, Client Key issuance, and storage.

use std::{error::Error, fmt};

use gateway_auth::client_key::{
    ClientKeyError, ClientKeyRecord, ClientKeyService, ClientKeyStatus, PresentedClientKey,
};
use gateway_core::{AccessGroupId, ClientKeyId, CredentialId, UpstreamId};
use gateway_store::{
    StoreError,
    control_plane::{
        ConfigVersionId, CredentialConfiguration, CredentialStatus, SqliteControlPlaneRepository,
        StoredClientKey, StoredClientKeyStatus,
    },
    secret_store::{SecretStore, SecretStoreError},
};

const CREDENTIAL_AAD_DOMAIN: &[u8] = b"cpa-rust-gateway/control-plane/credential-aad/v1";

/// Non-secret metadata for one atomic Credential and Client Key provisioning operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialAndClientKeyProvisionRequest {
    /// The draft Config Version that owns both new records.
    pub config_version_id: ConfigVersionId,
    /// Identity for the new encrypted upstream credential.
    pub credential_id: CredentialId,
    /// Identity for the Credential's owning Upstream.
    pub upstream_id: UpstreamId,
    /// Non-secret credential kind, such as `api_key` or `oauth_json`.
    pub credential_kind: String,
    /// Identity for the new Client Key.
    pub client_key_id: ClientKeyId,
    /// Access Group that owns the new Client Key.
    pub access_group_id: AccessGroupId,
    /// Optional Client Key expiry in Unix milliseconds.
    pub client_key_expires_at_ms: Option<i64>,
}

/// The one-time Client Key presentation returned after an atomic provisioning commit.
pub struct ProvisionedCredentialAndClientKey {
    credential_id: CredentialId,
    client_key_id: ClientKeyId,
    presented_client_key: PresentedClientKey,
}

impl ProvisionedCredentialAndClientKey {
    /// Returns the persisted non-secret Credential identifier.
    #[must_use]
    pub fn credential_id(&self) -> &CredentialId {
        &self.credential_id
    }

    /// Returns the persisted non-secret Client Key identifier.
    #[must_use]
    pub fn client_key_id(&self) -> &ClientKeyId {
        &self.client_key_id
    }

    /// Returns the complete Client Key only to the immediate provisioning-result consumer.
    ///
    /// Callers must not log or retain it beyond the one-time display boundary.
    #[must_use]
    pub fn presented_client_key(&self) -> &PresentedClientKey {
        &self.presented_client_key
    }

    /// Consumes the result into its non-secret identifiers and one-time Client Key presentation.
    #[must_use]
    pub fn into_parts(self) -> (CredentialId, ClientKeyId, PresentedClientKey) {
        (
            self.credential_id,
            self.client_key_id,
            self.presented_client_key,
        )
    }
}

impl fmt::Debug for ProvisionedCredentialAndClientKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProvisionedCredentialAndClientKey")
            .field("credential_id", &self.credential_id)
            .field("client_key_id", &self.client_key_id)
            .field("presented_client_key", &"<redacted>")
            .finish()
    }
}

/// Management-only service that seals Secrets and persists them atomically with issued Client Keys.
pub struct ControlPlaneService {
    repository: SqliteControlPlaneRepository,
    secret_store: SecretStore,
    client_key_service: ClientKeyService,
}

impl ControlPlaneService {
    /// Creates a management-only service from an owned Repository and external key material.
    #[must_use]
    pub const fn new(
        repository: SqliteControlPlaneRepository,
        secret_store: SecretStore,
        client_key_service: ClientKeyService,
    ) -> Self {
        Self {
            repository,
            secret_store,
            client_key_service,
        }
    }

    /// Provisions an opaque upstream Credential and a one-time Client Key in one `SQLite` transaction.
    ///
    /// The plaintext Credential is sealed before the Repository sees it. If Client Key insertion
    /// fails, the transaction rolls back the preceding encrypted Credential and no complete Key
    /// is returned.
    ///
    /// # Errors
    ///
    /// Returns a safe error from Client Key issuance, Secret sealing, AAD construction, or the
    /// Repository. No plaintext Credential, full Client Key, Master Key, Pepper, ciphertext, or
    /// digest is included in an error representation.
    pub fn provision_credential_and_client_key(
        &mut self,
        request: CredentialAndClientKeyProvisionRequest,
        plaintext_credential: &[u8],
    ) -> Result<ProvisionedCredentialAndClientKey, ControlPlaneServiceError> {
        let issued_client_key = self.client_key_service.issue(
            request.client_key_id.clone(),
            request.access_group_id.clone(),
            request.client_key_expires_at_ms,
        )?;
        let (client_key_record, presented_client_key) = issued_client_key.into_parts();
        self.provision_with_client_key_record(&request, plaintext_credential, &client_key_record)?;

        Ok(ProvisionedCredentialAndClientKey {
            credential_id: request.credential_id,
            client_key_id: request.client_key_id,
            presented_client_key,
        })
    }

    /// Returns the owned administrative Repository for a later management operation.
    ///
    /// This is intentionally mutable and management-only; it is not an inference-path handle.
    #[must_use]
    pub fn repository_mut(&mut self) -> &mut SqliteControlPlaneRepository {
        &mut self.repository
    }

    fn provision_with_client_key_record(
        &mut self,
        request: &CredentialAndClientKeyProvisionRequest,
        plaintext_credential: &[u8],
        client_key_record: &ClientKeyRecord,
    ) -> Result<(), ControlPlaneServiceError> {
        if client_key_record.client_key_id() != &request.client_key_id
            || client_key_record.access_group_id() != &request.access_group_id
        {
            return Err(ControlPlaneServiceError::ClientKeyRecordIdentityMismatch);
        }

        let associated_data = credential_associated_data(
            &request.config_version_id,
            &request.credential_id,
            &request.upstream_id,
        )?;
        let encrypted_secret = self
            .secret_store
            .seal(plaintext_credential, &associated_data)?;
        let credential = CredentialConfiguration {
            id: request.credential_id.clone(),
            upstream_id: request.upstream_id.clone(),
            kind: request.credential_kind.clone(),
            encrypted_secret,
            status: CredentialStatus::Active,
            revision: 0,
        };
        let stored_client_key = stored_client_key_from_record(client_key_record)?;

        let mut transaction = self.repository.begin_transaction()?;
        transaction.insert_credential(&request.config_version_id, &credential)?;
        transaction.insert_client_key(&request.config_version_id, &stored_client_key)?;
        transaction.commit()?;
        Ok(())
    }
}

impl fmt::Debug for ControlPlaneService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlPlaneService")
            .field("repository", &self.repository)
            .field("secret_store", &"<redacted>")
            .field("client_key_service", &"<redacted>")
            .finish()
    }
}

/// Produces the stable length-delimited AEAD AAD for a persisted upstream Credential.
///
/// Each logical field has an explicit 32-bit big-endian byte length, so tuples such as
/// `(ab, c, d)` and `(a, bc, d)` cannot collide. The fixed domain string prevents reuse by an
/// unrelated encrypted-record type.
///
/// # Errors
///
/// Returns [`ControlPlaneServiceError::AssociatedDataSegmentTooLong`] if an opaque identifier
/// cannot be represented in this internal format.
pub fn credential_associated_data(
    config_version_id: &ConfigVersionId,
    credential_id: &CredentialId,
    upstream_id: &UpstreamId,
) -> Result<Vec<u8>, ControlPlaneServiceError> {
    let segments = [
        config_version_id.as_str().as_bytes(),
        credential_id.as_str().as_bytes(),
        upstream_id.as_str().as_bytes(),
    ];
    let mut associated_data = Vec::with_capacity(
        CREDENTIAL_AAD_DOMAIN.len()
            + segments
                .iter()
                .map(|segment| 4 + segment.len())
                .sum::<usize>(),
    );
    associated_data.extend_from_slice(CREDENTIAL_AAD_DOMAIN);
    for segment in segments {
        let length = u32::try_from(segment.len())
            .map_err(|_| ControlPlaneServiceError::AssociatedDataSegmentTooLong)?;
        associated_data.extend_from_slice(&length.to_be_bytes());
        associated_data.extend_from_slice(segment);
    }
    Ok(associated_data)
}

fn stored_client_key_from_record(
    client_key_record: &ClientKeyRecord,
) -> Result<StoredClientKey, ControlPlaneServiceError> {
    StoredClientKey::try_new(
        client_key_record.client_key_id().clone(),
        client_key_record.access_group_id().clone(),
        client_key_record.prefix().as_str(),
        client_key_record.secret_digest().as_bytes(),
        match client_key_record.status() {
            ClientKeyStatus::Active => StoredClientKeyStatus::Active,
            ClientKeyStatus::Disabled => StoredClientKeyStatus::Disabled,
            ClientKeyStatus::Revoked => StoredClientKeyStatus::Revoked,
        },
        client_key_record.expires_at_ms(),
    )
    .map_err(ControlPlaneServiceError::from)
}

/// Safe failure classes for the control-plane provisioning service.
#[derive(Debug)]
pub enum ControlPlaneServiceError {
    /// The Store rejected a transactional operation.
    Store(StoreError),
    /// Secret sealing failed before persistence.
    SecretStore(SecretStoreError),
    /// Client Key issuance failed before persistence.
    ClientKey(ClientKeyError),
    /// A test/internal caller supplied a Client Key record for a different logical request.
    ClientKeyRecordIdentityMismatch,
    /// One identifier was too long for the stable length-delimited AAD representation.
    AssociatedDataSegmentTooLong,
}

impl fmt::Display for ControlPlaneServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(
                formatter,
                "control-plane repository operation failed: {error}"
            ),
            Self::SecretStore(error) => {
                write!(formatter, "control-plane secret operation failed: {error}")
            }
            Self::ClientKey(error) => write!(
                formatter,
                "control-plane Client Key operation failed: {error}"
            ),
            Self::ClientKeyRecordIdentityMismatch => {
                formatter.write_str("Client Key record does not match the requested identities")
            }
            Self::AssociatedDataSegmentTooLong => {
                formatter.write_str("control-plane associated-data segment is too long")
            }
        }
    }
}

impl Error for ControlPlaneServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::SecretStore(error) => Some(error),
            Self::ClientKey(error) => Some(error),
            Self::ClientKeyRecordIdentityMismatch | Self::AssociatedDataSegmentTooLong => None,
        }
    }
}

impl From<StoreError> for ControlPlaneServiceError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<SecretStoreError> for ControlPlaneServiceError {
    fn from(error: SecretStoreError) -> Self {
        Self::SecretStore(error)
    }
}

impl From<ClientKeyError> for ControlPlaneServiceError {
    fn from(error: ClientKeyError) -> Self {
        Self::ClientKey(error)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gateway_auth::client_key::{ClientKeyPepper, ClientKeyService};
    use gateway_core::{AccessGroupId, ClientKeyId, CredentialId, UpstreamId};
    use gateway_store::{
        control_plane::{
            AccessGroupConfiguration, AdministrativeStatus, ConfigVersion, ConfigVersionId,
            ConfigVersionStatus, ControlPlaneConfiguration, SqliteControlPlaneRepository,
            UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
    };

    use super::{
        ControlPlaneService, ControlPlaneServiceError, CredentialAndClientKeyProvisionRequest,
        credential_associated_data, stored_client_key_from_record,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn service_provisions_opaque_credential_and_client_key_with_stable_aad() -> TestResult {
        let version_id = ConfigVersionId::try_new("version-a")?;
        let key_version = KeyVersion::try_new(1)?;
        let mut service = ControlPlaneService::new(
            repository_with_upstream_and_access_group(&version_id)?,
            secret_store(key_version, 0x11)?,
            ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x22_u8; 32])?),
        );
        let request = request(&version_id, "credential-a", "client-key-a")?;

        let provisioned =
            service.provision_credential_and_client_key(request.clone(), b"credential-value")?;
        assert_eq!(
            provisioned.credential_id(),
            &CredentialId::try_new("credential-a")?
        );
        assert_eq!(
            provisioned.client_key_id(),
            &ClientKeyId::try_new("client-key-a")?
        );
        assert!(
            provisioned
                .presented_client_key()
                .as_str()
                .starts_with("rgw_")
        );
        assert!(!format!("{provisioned:?}").contains(provisioned.presented_client_key().as_str()));

        let loaded = service
            .repository_mut()
            .load_configuration(&version_id)?
            .ok_or("missing configuration")?;
        assert_eq!(loaded.credentials.len(), 1);
        assert_eq!(loaded.client_keys.len(), 1);

        let decryption_store = secret_store(key_version, 0x11)?;
        let associated_data = credential_associated_data(
            &version_id,
            &CredentialId::try_new("credential-a")?,
            &UpstreamId::try_new("upstream-a")?,
        )?;
        let plaintext =
            decryption_store.open(&loaded.credentials[0].encrypted_secret, &associated_data)?;
        assert_eq!(plaintext.as_bytes(), b"credential-value");
        assert!(
            decryption_store
                .open(&loaded.credentials[0].encrypted_secret, b"different-aad")
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn duplicate_client_key_rolls_back_the_preceding_credential_write() -> TestResult {
        let version_id = ConfigVersionId::try_new("version-a")?;
        let mut repository = repository_with_upstream_and_access_group(&version_id)?;
        let client_key_service =
            ClientKeyService::new(ClientKeyPepper::try_from_bytes([0x22_u8; 32])?);
        let existing_client_key = client_key_service.issue(
            ClientKeyId::try_new("client-key-existing")?,
            AccessGroupId::try_new("access-group-a")?,
            None,
        )?;
        let existing_record = existing_client_key.record().clone();
        let stored_client_key = stored_client_key_from_record(&existing_record)?;
        let mut transaction = repository.begin_transaction()?;
        transaction.insert_client_key(&version_id, &stored_client_key)?;
        transaction.commit()?;

        let mut service = ControlPlaneService::new(
            repository,
            secret_store(KeyVersion::try_new(1)?, 0x11)?,
            client_key_service,
        );
        let duplicate_request = request(
            &version_id,
            "credential-must-rollback",
            "client-key-existing",
        )?;
        let result = service.provision_with_client_key_record(
            &duplicate_request,
            b"credential-that-must-not-persist",
            &existing_record,
        );
        assert!(matches!(result, Err(ControlPlaneServiceError::Store(_))));

        let loaded = service
            .repository_mut()
            .load_configuration(&version_id)?
            .ok_or("missing configuration")?;
        assert!(loaded.credentials.is_empty());
        assert_eq!(loaded.client_keys.len(), 1);
        Ok(())
    }

    #[test]
    fn length_delimited_aad_keeps_field_boundaries_distinct() -> TestResult {
        let first = credential_associated_data(
            &ConfigVersionId::try_new("ab")?,
            &CredentialId::try_new("c")?,
            &UpstreamId::try_new("d")?,
        )?;
        let second = credential_associated_data(
            &ConfigVersionId::try_new("a")?,
            &CredentialId::try_new("bc")?,
            &UpstreamId::try_new("d")?,
        )?;
        assert_ne!(first, second);
        Ok(())
    }

    fn repository_with_upstream_and_access_group(
        version_id: &ConfigVersionId,
    ) -> Result<SqliteControlPlaneRepository, Box<dyn Error>> {
        let mut repository = SqliteControlPlaneRepository::open_in_memory()?;
        let mut configuration = ControlPlaneConfiguration::new(ConfigVersion {
            id: version_id.clone(),
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 1,
            description: "service fixture".to_owned(),
        });
        configuration.upstreams.push(UpstreamConfiguration {
            id: UpstreamId::try_new("upstream-a")?,
            name: "station-a".to_owned(),
            kind: "relay".to_owned(),
            enabled: true,
            tags_json: "[]".to_owned(),
            egress_policy_id: None,
        });
        configuration.access_groups.push(AccessGroupConfiguration {
            id: AccessGroupId::try_new("access-group-a")?,
            name: "default".to_owned(),
            status: AdministrativeStatus::Active,
            limits_json: "{}".to_owned(),
        });
        repository.write_configuration(&configuration)?;
        Ok(repository)
    }

    fn request(
        version_id: &ConfigVersionId,
        credential_id: &str,
        client_key_id: &str,
    ) -> Result<CredentialAndClientKeyProvisionRequest, Box<dyn Error>> {
        Ok(CredentialAndClientKeyProvisionRequest {
            config_version_id: version_id.clone(),
            credential_id: CredentialId::try_new(credential_id)?,
            upstream_id: UpstreamId::try_new("upstream-a")?,
            credential_kind: "api_key".to_owned(),
            client_key_id: ClientKeyId::try_new(client_key_id)?,
            access_group_id: AccessGroupId::try_new("access-group-a")?,
            client_key_expires_at_ms: None,
        })
    }

    fn secret_store(key_version: KeyVersion, key_byte: u8) -> Result<SecretStore, Box<dyn Error>> {
        Ok(SecretStore::new(MasterKeyRing::try_new(
            key_version,
            [(key_version, MasterKey::try_from_bytes([key_byte; 32])?)],
        )?))
    }
}
