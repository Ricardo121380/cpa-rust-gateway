//! Revision-guarded administrative changes; durable history is independent of the account row.
use super::{
    GrokAccountCredential, GrokAccountPoolError, GrokAccountPoolStore, GrokAccountProvider,
};
use gateway_store::account_identity::AccountIdentity;
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// One explicit account action. Material and identity are never included in Debug/log output.
#[derive(Clone, Copy)]
pub enum GrokManagedAccountChange<'a> {
    /// Administrative scheduling switch; does not assert provider authentication.
    SetEnabled(bool),
    /// Replace normalized SSO material, optionally carrying freshly observed provider identity.
    ReplaceSso {
        /// Normalized material for the selected channel.
        credential: &'a GrokAccountCredential,
        /// Fresh provider identity for that material, when available.
        identity: Option<&'a AccountIdentity>,
    },
    /// Remove this authorization while retaining request/ledger and audit records.
    Remove,
}

/// Value-only audit projection, bounded at the storage query.
#[derive(serde::Serialize)]
pub struct GrokManagedAccountEvent {
    /// Append sequence.
    pub id: i64,
    /// Closed native provider name.
    pub provider: String,
    /// Closed maintenance action.
    pub action: String,
    /// Revision produced by the action, including the removal tombstone revision.
    pub revision: i64,
    /// Authenticated management actor.
    pub actor: String,
    /// UTC milliseconds.
    pub occurred_at_ms: i64,
}

impl GrokAccountPoolStore {
    /// Applies a single explicit change and its audit in one immediate transaction.
    /// # Errors
    /// Rejects missing accounts, stale revisions, wrong-channel material and store failures.
    #[allow(clippy::too_many_lines)]
    pub fn manage_account(
        &self,
        id: &str,
        expected: u64,
        change: GrokManagedAccountChange<'_>,
        actor: &str,
        now: i64,
    ) -> Result<u64, GrokAccountPoolError> {
        if id.is_empty()
            || id.len() > 128
            || actor.is_empty()
            || actor.len() > 128
            || actor.chars().any(char::is_control)
            || now < 0
        {
            return Err(GrokAccountPoolError::InvalidRequest);
        }
        let expected = i64::try_from(expected).map_err(|_| GrokAccountPoolError::InvalidRequest)?;
        let next = expected
            .checked_add(1)
            .ok_or(GrokAccountPoolError::InvalidRequest)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let row = transaction
            .query_row(
                "SELECT provider, revision, identity_digest FROM grok_accounts WHERE id=?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
            .ok_or(GrokAccountPoolError::NotFound)?;
        if row.1 != expected {
            return Err(GrokAccountPoolError::ExistingAccountConflict);
        }
        let provider = GrokAccountProvider::parse(&row.0)?;
        let action = match change {
            GrokManagedAccountChange::SetEnabled(enabled) => {
                transaction.execute("UPDATE grok_accounts SET enabled=?1, auth_status=CASE WHEN ?1=1 AND auth_status='disabled' THEN 'reauth_required' ELSE auth_status END, revision=?2, updated_at_ms=?3, worker_claim_kind=NULL,worker_claim_id=NULL,worker_claim_expires_at_ms=NULL WHERE id=?4", params![enabled,next,now,id])
                    .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
                if enabled { "enabled" } else { "disabled" }
            }
            GrokManagedAccountChange::Remove => {
                transaction
                    .execute("DELETE FROM grok_accounts WHERE id=?1", [id])
                    .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
                "removed"
            }
            GrokManagedAccountChange::ReplaceSso {
                credential,
                identity,
            } => {
                let credential =
                    GrokAccountCredential::try_from_sso(provider, credential.as_bytes(), now)?;
                let enrolled = credential.enrollment_identity(provider, now, identity)?;
                let digest = super::identity_digest(provider, &enrolled.0);
                let duplicate: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM grok_accounts WHERE provider=?1 AND identity_digest=?2 AND id<>?3)",params![provider.as_str(),digest.as_slice(),id],|row|row.get(0))
                    .map_err(|_|GrokAccountPoolError::StoreUnavailable)?;
                if duplicate {
                    return Err(GrokAccountPoolError::ExistingAccountConflict);
                }
                let sealed = self
                    .secret_store
                    .seal(
                        credential.as_bytes(),
                        &super::credential_aad(provider, &digest),
                    )
                    .map_err(|_| GrokAccountPoolError::SecretStoreFailure)?;
                transaction.execute("UPDATE grok_accounts SET credential_ciphertext=?1, credential_key_version=?2, revision=?3, auth_status='active', refresh_failure_count=0, cooldown_until_ms=NULL, updated_at_ms=?4, worker_claim_kind=NULL,worker_claim_id=NULL,worker_claim_expires_at_ms=NULL, identity_digest=?6 WHERE id=?5",params![sealed.ciphertext(),sealed.key_version().as_sqlite_i64(),next,now,id,digest.as_slice()])
                    .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
                transaction
                    .execute(
                        "DELETE FROM native_account_identity_observations WHERE account_id=?1",
                        [id],
                    )
                    .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
                transaction
                    .execute(
                        "DELETE FROM grok_account_reauth_state WHERE account_id=?1",
                        [id],
                    )
                    .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
                if let Some(identity) = identity.filter(|identity| !identity.is_empty()) {
                    let fingerprint: [u8; 32] = Sha256::digest(sealed.ciphertext()).into();
                    let bytes = Zeroizing::new(
                        serde_json::to_vec(identity)
                            .map_err(|_| GrokAccountPoolError::InvalidRequest)?,
                    );
                    let observation = self
                        .secret_store
                        .seal(&bytes, &super::identity::aad(id, &fingerprint))
                        .map_err(|_| GrokAccountPoolError::SecretStoreFailure)?;
                    transaction.execute("INSERT INTO native_account_identity_observations(account_id,credential_fingerprint,ciphertext,key_version,observed_at_ms) VALUES(?1,?2,?3,?4,?5)",params![id,fingerprint.as_slice(),observation.ciphertext(),observation.key_version().as_sqlite_i64(),now])
                        .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
                }
                "credential_updated"
            }
        };
        transaction.execute("INSERT INTO native_account_management_events(account_id,provider,action,revision,actor,occurred_at_ms) VALUES(?1,?2,?3,?4,?5,?6)",params![id,provider.as_str(),action,next,actor,now])
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        transaction
            .commit()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        u64::try_from(next).map_err(|_| GrokAccountPoolError::InvalidPersistedState)
    }

    /// Returns the last 100 maintenance actions even after the account was removed.
    /// # Errors
    /// Rejects malformed IDs or unavailable storage.
    pub fn account_management_events(
        &self,
        id: &str,
    ) -> Result<Vec<GrokManagedAccountEvent>, GrokAccountPoolError> {
        if id.is_empty() || id.len() > 128 {
            return Err(GrokAccountPoolError::InvalidRequest);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let mut query = connection.prepare("SELECT id,provider,action,revision,actor,occurred_at_ms FROM native_account_management_events WHERE account_id=?1 ORDER BY id DESC LIMIT 100")
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        query
            .query_map([id], |r| {
                Ok(GrokManagedAccountEvent {
                    id: r.get(0)?,
                    provider: r.get(1)?,
                    action: r.get(2)?,
                    revision: r.get(3)?,
                    actor: r.get(4)?,
                    occurred_at_ms: r.get(5)?,
                })
            })
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GrokAccountAuthStatus, GrokAccountIdentity, GrokAccountImport};
    use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};

    #[test]
    #[allow(clippy::too_many_lines)] // One CAS/identity/delete transaction lifecycle regression.
    fn account_maintenance_cas_keeps_encrypted_identity_and_removed_history()
    -> Result<(), Box<dyn std::error::Error>> {
        let version = KeyVersion::try_new(1)?;
        let secrets = SecretStore::new(MasterKeyRing::try_new(
            version,
            [(version, MasterKey::try_from_bytes([0x35; 32])?)],
        )?);
        let store =
            GrokAccountPoolStore::try_new(rusqlite::Connection::open_in_memory()?, secrets)?;
        for (provider, batch) in [
            (GrokAccountProvider::Web, "web"),
            (GrokAccountProvider::Console, "console"),
            (GrokAccountProvider::Build, "build"),
        ] {
            let account = GrokAccountImport {
                provider,
                identity: GrokAccountIdentity::try_from_bytes(batch.as_bytes())?,
                credential: GrokAccountCredential::try_from_bytes(b"synthetic-native-material")?,
                auth_status: GrokAccountAuthStatus::Active,
                enabled: true,
                priority: 0,
                weight: 1,
                max_concurrency: 1,
                refresh_due_at_ms: None,
                quota_sync_due_at_ms: None,
                cooldown_until_ms: None,
            };
            store.import_batch(batch, &[account], 1)?;
            let id = store.single_import_account(batch)?;
            assert_eq!(
                store.manage_account(
                    &id,
                    0,
                    GrokManagedAccountChange::SetEnabled(false),
                    "admin",
                    2
                )?,
                1
            );
            assert_eq!(
                store.manage_account(&id, 0, GrokManagedAccountChange::Remove, "admin", 3),
                Err(GrokAccountPoolError::ExistingAccountConflict)
            );
            assert_eq!(
                store.manage_account(
                    &id,
                    1,
                    GrokManagedAccountChange::SetEnabled(true),
                    "admin",
                    3
                )?,
                2
            );
            let revision = if provider == GrokAccountProvider::Console {
                let material =
                    GrokAccountCredential::try_from_sso(provider, b"new-synthetic-sso", 4)?;
                let identity = AccountIdentity {
                    email: Some("member@example.test".into()),
                    phone: None,
                    username: None,
                };
                let revision = store.manage_account(
                    &id,
                    2,
                    GrokManagedAccountChange::ReplaceSso {
                        credential: &material,
                        identity: Some(&identity),
                    },
                    "admin",
                    4,
                )?;
                assert_eq!(store.observed_identity(&id, 4)?, identity);
                let snapshot = store.identity_snapshot(&id)?;
                assert_eq!(snapshot.credential.as_bytes(), b"new-synthetic-sso");
                revision
            } else {
                2
            };
            store.manage_account(&id, revision, GrokManagedAccountChange::Remove, "admin", 5)?;
            assert_eq!(
                store.identity_snapshot(&id).err(),
                Some(GrokAccountPoolError::NotFound)
            );
            let events = store.account_management_events(&id)?;
            assert_eq!(events[0].action, "removed");
            assert_eq!(
                events.len(),
                if provider == GrokAccountProvider::Console {
                    4
                } else {
                    3
                }
            );
            let connection = store.connection.lock().map_err(|_| "lock")?;
            assert!(
                connection
                    .execute(
                        "DELETE FROM native_account_management_events WHERE account_id=?1",
                        [&id]
                    )
                    .is_err()
            );
        }
        Ok(())
    }

    #[test]
    fn enrollment_identity_does_not_depend_on_import_marker()
    -> Result<(), Box<dyn std::error::Error>> {
        let version = KeyVersion::try_new(1)?;
        let secrets = SecretStore::new(MasterKeyRing::try_new(
            version,
            [(version, MasterKey::try_from_bytes([0x36; 32])?)],
        )?);
        let store =
            GrokAccountPoolStore::try_new(rusqlite::Connection::open_in_memory()?, secrets)?;
        let provider = GrokAccountProvider::Console;
        for (index, batch) in ["first-import", "same-session-again"]
            .into_iter()
            .enumerate()
        {
            let credential =
                GrokAccountCredential::try_from_sso(provider, b"stable-synthetic-sso", 1)?;
            let account = GrokAccountImport {
                provider,
                identity: credential.enrollment_identity(provider, 1, None)?,
                credential,
                auth_status: GrokAccountAuthStatus::Active,
                enabled: true,
                priority: 0,
                weight: 1,
                max_concurrency: 1,
                refresh_due_at_ms: None,
                quota_sync_due_at_ms: None,
                cooldown_until_ms: None,
            };
            let result = store.import_managed_account(batch, &account, 1)?;
            assert_eq!(result.created, usize::from(index == 0));
            assert_eq!(result.unchanged, usize::from(index == 1));
            if index == 0 {
                let id = store.single_import_account(batch)?;
                store.manage_account(
                    &id,
                    0,
                    GrokManagedAccountChange::SetEnabled(false),
                    "admin",
                    2,
                )?;
            }
        }
        assert_eq!(
            store.managed_account_page(100, "", "", None)?.items.len(),
            1
        );
        assert!(!store.managed_account_page(100, "", "", None)?.items[0].enabled);
        Ok(())
    }
}
