//! Bounded native account inventory, guarded by connection change clocks.
use super::{
    GrokAccountMetadata, GrokAccountPoolError, GrokAccountPoolStore, decode_metadata,
    validate_metadata,
};
use rusqlite::params;

/// Safe page of native identities, including unbound and disabled accounts.
pub struct GrokManagedAccountPage {
    /// Metadata rows. Entitlements are not observed by this inventory read.
    pub items: Vec<GrokAccountMetadata>,
    /// Durable native-account metadata generation.
    pub stamp: i64,
    /// More rows follow the final returned ID.
    pub has_more: bool,
}

impl GrokAccountPoolStore {
    /// Enumerates native account metadata without selecting any credential ciphertext.
    /// # Errors
    /// Rejects invalid bounds, stale continuation stamps and unavailable stores.
    pub fn managed_account_page(
        &self,
        limit: usize,
        after: &str,
        search: &str,
        expected: Option<i64>,
    ) -> Result<GrokManagedAccountPage, GrokAccountPoolError> {
        if !(1..=100).contains(&limit) || after.len() > 128 || search.len() > 256 {
            return Err(GrokAccountPoolError::InvalidRequest);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let stamp = connection
            .query_row(
                "SELECT generation FROM native_account_inventory_generation WHERE singleton=1",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        if expected.is_some_and(|value| value != stamp) {
            return Err(GrokAccountPoolError::ExistingAccountConflict);
        }
        let mut statement = connection.prepare("SELECT id, provider, auth_status, enabled, priority, weight, max_concurrency, refresh_due_at_ms, quota_sync_due_at_ms, cooldown_until_ms, revision, import_batch_id FROM grok_accounts WHERE id > ?1 AND (?2 = '' OR instr(lower(id || ' ' || provider || ' ' || import_batch_id), lower(?2)) > 0) ORDER BY id LIMIT ?3").map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let count = i64::try_from(limit + 1).map_err(|_| GrokAccountPoolError::InvalidRequest)?;
        let mut items = statement
            .query_map(params![after, search, count], decode_metadata)
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?
            .map(|row| {
                row.map_err(|_| GrokAccountPoolError::InvalidPersistedState)
                    .and_then(validate_metadata)
            })
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let current = connection
            .query_row(
                "SELECT generation FROM native_account_inventory_generation WHERE singleton=1",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        if current != stamp {
            return Err(GrokAccountPoolError::ExistingAccountConflict);
        }
        let has_more = items.len() > limit;
        items.truncate(limit);
        Ok(GrokManagedAccountPage {
            items,
            stamp,
            has_more,
        })
    }
}

impl GrokAccountPoolStore {
    /// Replaces Build authorization only for the same provider subject and exact revision.
    /// # Errors
    /// Rejects missing identity evidence, another account, stale revisions and storage failures.
    pub fn replace_build_authorization(
        &self,
        id: &str,
        expected: u64,
        credential: &crate::GrokBuildCredential,
        now: i64,
    ) -> Result<(), GrokAccountPoolError> {
        use gateway_store::secret_store::{EncryptedSecret, KeyVersion};
        use rusqlite::OptionalExtension;
        if id.is_empty() || id.len() > 128 || now < 0 || credential.is_expired_at(now) {
            return Err(GrokAccountPoolError::InvalidRequest);
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        let row=transaction.query_row("SELECT identity_digest, credential_ciphertext, credential_key_version, revision FROM grok_accounts WHERE id=?1 AND provider='build'",[id],|r|Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,Vec<u8>>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?))).optional().map_err(|_|GrokAccountPoolError::StoreUnavailable)?.ok_or(GrokAccountPoolError::NotFound)?;
        let revision = i64::try_from(expected).map_err(|_| GrokAccountPoolError::InvalidRequest)?;
        if row.3 != revision {
            return Err(GrokAccountPoolError::ExistingAccountConflict);
        }
        let digest: [u8; 32] = row
            .0
            .try_into()
            .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?;
        let aad = super::credential_aad(super::GrokAccountProvider::Build, &digest);
        let encrypted = EncryptedSecret::try_from_persisted(
            KeyVersion::try_from_sqlite_i64(row.2)
                .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?,
            row.1,
        )
        .map_err(|_| GrokAccountPoolError::InvalidPersistedState)?;
        let previous = self
            .secret_store
            .open(&encrypted, &aad)
            .map_err(|_| GrokAccountPoolError::SecretStoreFailure)?;
        let previous =
            crate::GrokBuildCredential::import_refreshable_runtime(previous.as_bytes(), now)
                .map_err(|_| GrokAccountPoolError::InvalidCredential)?;
        if build_subject(previous.access_token())? != build_subject(credential.access_token())? {
            return Err(GrokAccountPoolError::ExistingAccountConflict);
        }
        let bytes = credential
            .persisted_bytes()
            .map_err(|_| GrokAccountPoolError::InvalidCredential)?;
        let sealed = self
            .secret_store
            .seal(&bytes, &aad)
            .map_err(|_| GrokAccountPoolError::SecretStoreFailure)?;
        let next = revision
            .checked_add(1)
            .ok_or(GrokAccountPoolError::InvalidPersistedState)?;
        transaction.execute("UPDATE grok_accounts SET credential_ciphertext=?1,credential_key_version=?2,revision=?3,auth_status='active',refresh_due_at_ms=?4,last_refresh_at_ms=?5,refresh_failure_count=0,updated_at_ms=?5,worker_claim_kind=NULL,worker_claim_id=NULL,worker_claim_expires_at_ms=NULL WHERE id=?6 AND revision=?7",params![sealed.ciphertext(),sealed.key_version().as_sqlite_i64(),next,credential.expires_at_ms().saturating_sub(60000).max(now),now,id,revision]).map_err(|_|GrokAccountPoolError::StoreUnavailable)?;
        transaction
            .execute(
                "DELETE FROM grok_account_reauth_state WHERE account_id=?1",
                [id],
            )
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)?;
        transaction.execute("INSERT INTO native_account_authorization_events(account_id,revision,action,occurred_at_ms) VALUES(?1,?2,'device_reauthorized',?3)",params![id,next,now]).map_err(|_|GrokAccountPoolError::StoreUnavailable)?;
        transaction
            .commit()
            .map_err(|_| GrokAccountPoolError::StoreUnavailable)
    }
}

fn build_subject(token: &str) -> Result<String, GrokAccountPoolError> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let payload = token
        .split('.')
        .nth(1)
        .ok_or(GrokAccountPoolError::InvalidCredential)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| GrokAccountPoolError::InvalidCredential)?;
    let value: serde_json::Value =
        serde_json::from_slice(&decoded).map_err(|_| GrokAccountPoolError::InvalidCredential)?;
    value
        .get("sub")
        .and_then(serde_json::Value::as_str)
        .filter(|v| !v.is_empty() && v.len() <= 1024)
        .map(str::to_owned)
        .ok_or(GrokAccountPoolError::InvalidCredential)
}
