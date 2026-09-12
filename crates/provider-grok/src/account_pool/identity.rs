//! Encrypted display observations bound to the exact native credential that supplied them.
use super::{
    GrokAccountPoolError, GrokAccountPoolStore, GrokAccountProvider, load_persisted_credential,
    open_persisted_credential,
};
use gateway_store::{
    account_identity::AccountIdentity,
    secret_store::{EncryptedSecret, KeyVersion, PlaintextSecret},
};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// One atomic, redacted credential observation for a provider identity lookup.
pub struct GrokAccountIdentitySnapshot {
    /// Exact account ID; never used as a human display name.
    pub account_id: String,
    /// Native provider that owns this credential.
    pub provider: GrokAccountProvider,
    /// Revision that must still be current when persisting an observation.
    pub revision: u64,
    /// Credential material, available only to the server-side provider lookup.
    pub credential: PlaintextSecret,
    fingerprint: [u8; 32],
    observation_fingerprint: Option<[u8; 32]>,
}
impl std::fmt::Debug for GrokAccountIdentitySnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrokAccountIdentitySnapshot")
            .field("provider", &self.provider)
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

pub(super) fn aad(account: &str, fingerprint: &[u8; 32]) -> Vec<u8> {
    let mut value = b"cpar/native-account/identity/v1\0".to_vec();
    value.extend_from_slice(account.as_bytes());
    value.push(0);
    value.extend_from_slice(fingerprint);
    value
}

fn observation_fingerprint(
    connection: &rusqlite::Connection,
    id: &str,
) -> Result<Option<[u8; 32]>, GrokAccountPoolError> {
    connection
        .query_row(
            "SELECT ciphertext FROM native_account_identity_observations WHERE account_id=?1",
            [id],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map(|value| value.map(|bytes| Sha256::digest(bytes).into()))
        .map_err(|_| GrokAccountPoolError::StoreUnavailable)
}

impl GrokAccountPoolStore {
    /// Opens a revision-bound credential solely for a requested identity lookup.
    /// # Errors
    /// Rejects missing records, unavailable storage and failed credential authentication.
    pub fn identity_snapshot(
        &self,
        id: &str,
    ) -> Result<GrokAccountIdentitySnapshot, GrokAccountPoolError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let persisted =
            load_persisted_credential(&connection, id)?.ok_or(GrokAccountPoolError::NotFound)?;
        let revision: i64 = connection
            .query_row(
                "SELECT revision FROM grok_accounts WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        Ok(GrokAccountIdentitySnapshot {
            account_id: id.to_owned(),
            provider: persisted.provider,
            revision: u64::try_from(revision)
                .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
            fingerprint: Sha256::digest(&persisted.ciphertext).into(),
            observation_fingerprint: observation_fingerprint(&connection, id)?,
            credential: open_persisted_credential(&self.secret_store, &persisted)?,
        })
    }

    /// Returns current human identity without contacting a provider or acquiring a runtime lease.
    /// # Errors
    /// Rejects storage failures and invalid encrypted observations.
    pub fn observed_identity(
        &self,
        id: &str,
        now_ms: i64,
    ) -> Result<AccountIdentity, GrokAccountPoolError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let persisted =
            load_persisted_credential(&connection, id)?.ok_or(GrokAccountPoolError::NotFound)?;
        let plain = open_persisted_credential(&self.secret_store, &persisted)?;
        let mut identity = if persisted.provider == GrokAccountProvider::Build {
            crate::GrokBuildCredential::import_refreshable_runtime(plain.as_bytes(), now_ms)
                .ok()
                .map(|c| c.identity().clone())
                .unwrap_or_default()
        } else {
            AccountIdentity::from_credential(plain.as_bytes())
        };
        let fingerprint: [u8; 32] = Sha256::digest(&persisted.ciphertext).into();
        let stored=connection.query_row("SELECT ciphertext,key_version FROM native_account_identity_observations WHERE account_id=?1 AND credential_fingerprint=?2",params![id,fingerprint.as_slice()],|r|Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,i64>(1)?))).optional().map_err(|_|GrokAccountPoolError::StoreUnavailable)?;
        if let Some((bytes, version)) = stored {
            let version = KeyVersion::try_from_sqlite_i64(version)
                .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?;
            let encrypted = EncryptedSecret::try_from_persisted(version, bytes)
                .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?;
            let value = self
                .secret_store
                .open(&encrypted, &aad(id, &fingerprint))
                .map_err(|_| GrokAccountPoolError::SecretStoreFailure)?;
            identity.retain_missing(&AccountIdentity::from_credential(value.as_bytes()));
        }
        Ok(identity)
    }

    /// Saves provider-observed identity only while the exact source credential is unchanged.
    /// # Errors
    /// Rejects empty observations, stale revisions, changed ciphertext and storage failures.
    pub fn save_observed_identity(
        &self,
        snapshot: &GrokAccountIdentitySnapshot,
        identity: &AccountIdentity,
        now_ms: i64,
    ) -> Result<(), GrokAccountPoolError> {
        if identity.is_empty() || now_ms < 0 {
            return Err(GrokAccountPoolError::InvalidRequest);
        }
        let bytes = Zeroizing::new(
            serde_json::to_vec(identity).map_err(|_| GrokAccountPoolError::InvalidRequest)?,
        );
        let encrypted = self
            .secret_store
            .seal(&bytes, &aad(&snapshot.account_id, &snapshot.fingerprint))
            .map_err(|_| GrokAccountPoolError::SecretStoreFailure)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let current = transaction
            .query_row(
                "SELECT revision,credential_ciphertext FROM grok_accounts WHERE id=?1",
                [&snapshot.account_id],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)),
            )
            .optional()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
            .ok_or(GrokAccountPoolError::NotFound)?;
        if u64::try_from(current.0).ok() != Some(snapshot.revision)
            || <[u8; 32]>::from(Sha256::digest(current.1)) != snapshot.fingerprint
            || observation_fingerprint(&transaction, &snapshot.account_id)?
                != snapshot.observation_fingerprint
        {
            return Err(GrokAccountPoolError::ExistingAccountConflict);
        }
        transaction.execute("INSERT INTO native_account_identity_observations(account_id,credential_fingerprint,ciphertext,key_version,observed_at_ms) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(account_id) DO UPDATE SET credential_fingerprint=excluded.credential_fingerprint,ciphertext=excluded.ciphertext,key_version=excluded.key_version,observed_at_ms=excluded.observed_at_ms",params![snapshot.account_id,snapshot.fingerprint.as_slice(),encrypted.ciphertext(),encrypted.key_version().as_sqlite_i64(),now_ms]).map_err(|_|GrokAccountPoolError::StoreUnavailable)?;
        transaction
            .commit()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)
    }

    /// Resolves the single native account produced by the management import entry point.
    /// # Errors
    /// Rejects ambiguous batches or unavailable storage; does not select an arbitrary account.
    pub fn single_import_account(&self, batch: &str) -> Result<String, GrokAccountPoolError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let mut query = connection
            .prepare("SELECT id FROM grok_accounts WHERE import_batch_id=?1 LIMIT 2")
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let ids = query
            .query_map([batch], |r| r.get::<_, String>(0))
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        if ids.len() != 1 {
            return Err(GrokAccountPoolError::InvalidRequest);
        }
        Ok(ids[0].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GrokAccountAuthStatus, GrokAccountCredential, GrokAccountIdentity, GrokAccountImport,
    };
    use gateway_store::secret_store::{MasterKey, MasterKeyRing, SecretStore};
    #[test]
    fn observation_is_encrypted_cas_bound_and_invalidated_by_credential_changes()
    -> Result<(), Box<dyn std::error::Error>> {
        let version = KeyVersion::try_new(1)?;
        let secrets = SecretStore::new(MasterKeyRing::try_new(
            version,
            [(version, MasterKey::try_from_bytes([0x52; 32])?)],
        )?);
        let store = GrokAccountPoolStore::try_new(
            rusqlite::Connection::open_in_memory()?,
            secrets.clone(),
        )?;
        let account = GrokAccountImport {
            provider: GrokAccountProvider::Console,
            identity: GrokAccountIdentity::try_from_bytes(b"identity-test")?,
            credential: GrokAccountCredential::try_from_bytes(b"synthetic-sso")?,
            auth_status: GrokAccountAuthStatus::Active,
            enabled: true,
            priority: 0,
            weight: 1,
            max_concurrency: 1,
            refresh_due_at_ms: None,
            quota_sync_due_at_ms: None,
            cooldown_until_ms: None,
        };
        store.import_batch("identity-import", &[account], 1)?;
        let id = store.single_import_account("identity-import")?;
        let old_page = store.managed_account_page(1, "", "", None)?;
        let snapshot = store.identity_snapshot(&id)?;
        let identity = AccountIdentity {
            email: Some("member@example.test".to_owned()),
            phone: None,
            username: None,
        };
        store.save_observed_identity(&snapshot, &identity, 2)?;
        // A concurrent later observation cannot be overwritten even in the same millisecond.
        assert_eq!(
            store.save_observed_identity(&snapshot, &identity, 2),
            Err(GrokAccountPoolError::ExistingAccountConflict)
        );
        assert_eq!(store.observed_identity(&id, 2)?, identity);
        assert!(
            store
                .managed_account_page(1, "", "", Some(old_page.stamp))
                .is_err()
        );
        let connection = store.connection.lock().map_err(|_| "lock")?;
        let cipher: Vec<u8> = connection.query_row(
            "SELECT ciphertext FROM native_account_identity_observations",
            [],
            |r| r.get(0),
        )?;
        assert!(
            !cipher
                .windows(b"member@example.test".len())
                .any(|part| part == b"member@example.test")
        );
        connection.execute(
            "UPDATE grok_accounts SET revision=revision+1 WHERE id=?1",
            [&id],
        )?;
        drop(connection);
        assert_eq!(
            store.save_observed_identity(&snapshot, &identity, 3),
            Err(GrokAccountPoolError::ExistingAccountConflict)
        );
        // Metadata-only changes preserve the profile, but a different credential cannot inherit it.
        assert_eq!(store.observed_identity(&id, 3)?, identity);
        let connection = store.connection.lock().map_err(|_| "lock")?;
        let persisted = load_persisted_credential(&connection, &id)?.ok_or("missing")?;
        let changed = secrets.seal(
            b"replacement-sso",
            &super::super::credential_aad(persisted.provider, &persisted.identity_digest),
        )?;
        connection.execute(
            "UPDATE grok_accounts SET credential_ciphertext=?1,revision=revision+1 WHERE id=?2",
            params![changed.ciphertext(), id],
        )?;
        drop(connection);
        assert!(store.observed_identity(&id, 4)?.is_empty());
        Ok(())
    }
}
