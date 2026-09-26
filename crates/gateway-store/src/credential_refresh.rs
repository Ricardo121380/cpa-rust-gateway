//! One durable refresh claim per exact ordinary Credential. No network or plaintext secrets.

use crate::{
    StoreError, StoreResult, control_plane::ConfigVersionId, secret_store::EncryptedSecret,
};
use gateway_core::CredentialId;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::path::Path;

/// Safe failure class; neither status bodies nor private identity are persisted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshFailure {
    /// Transport, timeout or upstream temporary rejection.
    Network,
    /// Successful HTTP response could not be safely decoded.
    InvalidResponse,
    /// Explicit invalid/revoked grant; requires operator authorization.
    ReauthRequired,
}
impl RefreshFailure {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::InvalidResponse => "invalid_response",
            Self::ReauthRequired => "reauth_required",
        }
    }
}

/// An opaque fenced claim. A dropped/crashed claimant becomes eligible after its bounded lease.
pub struct RefreshClaim {
    version: ConfigVersionId,
    credential: CredentialId,
    revision: i64,
    nonce: String,
}

/// Control-path repository, never queried on the inference hot path.
pub struct CredentialRefreshStore {
    connection: Connection,
}
impl CredentialRefreshStore {
    /// Opens an already migrated database.
    /// # Errors
    /// Returns a safe storage error if the database is unavailable.
    pub fn open(path: &Path) -> StoreResult<Self> {
        Ok(Self {
            connection: crate::open(path)?,
        })
    }

    /// Claims one active revision if neither a peer nor persistent backoff owns it.
    /// # Errors
    /// Invalid clocks/nonce or unavailable storage fail closed before any exchange.
    pub fn claim(
        &mut self,
        version: &ConfigVersionId,
        credential: &CredentialId,
        revision: i64,
        nonce: &str,
        now: i64,
        lease_ms: i64,
    ) -> StoreResult<Option<RefreshClaim>> {
        if !(0..i64::MAX).contains(&revision)
            || now < 0
            || !(1..=120_000).contains(&lease_ms)
            || nonce.len() != 32
            || !nonce.bytes().all(|v| v.is_ascii_hexdigit())
        {
            return Err(StoreError::ConfigVersionRevisionConflict);
        }
        let until = now
            .checked_add(lease_ms)
            .ok_or(StoreError::ConfigVersionRevisionConflict)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM upstream_credentials c JOIN config_versions v ON v.id=c.config_version_id WHERE c.config_version_id=?1 AND c.id=?2 AND c.revision=?3 AND c.status='active' AND v.status='active')",
            params![version.as_str(), credential.as_str(), revision], |row| row.get(0))?;
        if !active {
            return Ok(None);
        }
        let changed = tx.execute("INSERT INTO credential_refresh_state (config_version_id, credential_id, credential_revision, claim_id, lease_until_ms, observed_at_ms) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(config_version_id, credential_id) DO UPDATE SET credential_revision=excluded.credential_revision, claim_id=excluded.claim_id, lease_until_ms=excluded.lease_until_ms, observed_at_ms=excluded.observed_at_ms, failure_count=CASE WHEN credential_refresh_state.credential_revision=excluded.credential_revision THEN credential_refresh_state.failure_count ELSE 0 END, last_error=CASE WHEN credential_refresh_state.credential_revision=excluded.credential_revision THEN credential_refresh_state.last_error ELSE NULL END WHERE credential_refresh_state.credential_revision!=excluded.credential_revision OR (credential_refresh_state.lease_until_ms<=?6 AND credential_refresh_state.retry_after_ms<=?6 AND coalesce(credential_refresh_state.last_error,'')!='reauth_required')",
            params![version.as_str(), credential.as_str(), revision, nonce, until, now])?;
        tx.commit()?;
        Ok((changed == 1).then(|| RefreshClaim {
            version: version.clone(),
            credential: credential.clone(),
            revision,
            nonce: nonce.to_owned(),
        }))
    }

    /// Atomically rotates only the claimed active revision, its state, and the safe audit event.
    /// Kind, graph revision, bindings, and permissions remain unchanged.
    /// # Errors
    /// Storage failure aborts the transaction. A stale claim returns false without mutation.
    pub fn succeed(
        &mut self,
        claim: &RefreshClaim,
        secret: &EncryptedSecret,
        now: i64,
    ) -> StoreResult<bool> {
        let next = claim
            .revision
            .checked_add(1)
            .ok_or(StoreError::ConfigVersionRevisionConflict)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !claim_current(&tx, claim, now)? {
            return Ok(false);
        }
        let changed = tx.execute("UPDATE upstream_credentials SET ciphertext=?4, key_version=?5, revision=?6 WHERE config_version_id=?1 AND id=?2 AND revision=?3 AND status='active'",
            params![claim.version.as_str(), claim.credential.as_str(), claim.revision, secret.ciphertext(), secret.key_version().as_sqlite_i64(), next])?;
        if changed != 1 {
            return Ok(false);
        }
        tx.execute("UPDATE credential_refresh_state SET credential_revision=?3, claim_id=NULL, lease_until_ms=0, retry_after_ms=0, failure_count=0, last_error=NULL, observed_at_ms=?4 WHERE config_version_id=?1 AND credential_id=?2",
            params![claim.version.as_str(), claim.credential.as_str(), next, now])?;
        audit(&tx, claim, "credential_oauth_rotated", now)?;
        tx.commit()?;
        Ok(true)
    }

    /// Persists bounded exponential retry or explicit reauthorization, fenced by claim/revision.
    /// # Errors
    /// Storage failure aborts the transaction. Stale outcomes cannot change newer credentials.
    pub fn fail(
        &mut self,
        claim: &RefreshClaim,
        failure: RefreshFailure,
        now: i64,
    ) -> StoreResult<bool> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !claim_current(&tx, claim, now)? {
            return Ok(false);
        }
        let count: u32 = tx.query_row("SELECT failure_count FROM credential_refresh_state WHERE config_version_id=?1 AND credential_id=?2",
            params![claim.version.as_str(), claim.credential.as_str()], |r| r.get(0))?;
        let delay = 60_000_i64
            .saturating_mul(1_i64 << count.min(6))
            .min(3_600_000);
        tx.execute("UPDATE credential_refresh_state SET claim_id=NULL, lease_until_ms=0, failure_count=?3, retry_after_ms=?4, last_error=?5, observed_at_ms=?6 WHERE config_version_id=?1 AND credential_id=?2",
            params![claim.version.as_str(), claim.credential.as_str(), count.saturating_add(1), now.saturating_add(delay), failure.as_sql(), now])?;
        if failure == RefreshFailure::ReauthRequired {
            tx.execute("UPDATE upstream_credentials SET status='unauthorized', revision=revision+1 WHERE config_version_id=?1 AND id=?2 AND revision=?3",
                params![claim.version.as_str(), claim.credential.as_str(), claim.revision])?;
            audit(&tx, claim, "credential_reauthorization_required", now)?;
        }
        tx.commit()?;
        Ok(true)
    }
}

fn claim_current(
    tx: &rusqlite::Transaction<'_>,
    claim: &RefreshClaim,
    now: i64,
) -> StoreResult<bool> {
    let current: Option<i64> = tx.query_row("SELECT 1 FROM credential_refresh_state s JOIN upstream_credentials c ON c.config_version_id=s.config_version_id AND c.id=s.credential_id JOIN config_versions v ON v.id=c.config_version_id WHERE s.config_version_id=?1 AND s.credential_id=?2 AND s.credential_revision=?3 AND s.claim_id=?4 AND s.lease_until_ms>?5 AND s.observed_at_ms<=?5 AND c.revision=?3 AND c.status='active' AND v.status='active'",
        params![claim.version.as_str(), claim.credential.as_str(), claim.revision, claim.nonce, now], |r| r.get(0)).optional()?;
    Ok(current.is_some())
}
fn audit(
    tx: &rusqlite::Transaction<'_>,
    claim: &RefreshClaim,
    action: &str,
    now: i64,
) -> StoreResult<()> {
    tx.execute("INSERT INTO management_resource_audit_events(action,actor,occurred_at_ms,config_version_id,resource_kind,resource_id) VALUES (?1,'runtime-credential-refresh',?2,?3,'credential',?4)",
        params![action, now, claim.version.as_str(), claim.credential.as_str()])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    fn fixture() -> Result<
        (
            CredentialRefreshStore,
            ConfigVersionId,
            CredentialId,
            EncryptedSecret,
        ),
        Box<dyn std::error::Error>,
    > {
        let mut c = crate::open_in_memory()?;
        crate::migrate(&mut c)?;
        c.execute_batch("INSERT INTO config_versions(id,status,created_at_ms) VALUES ('cfg','active',0); INSERT INTO upstreams(config_version_id,id,name,kind,enabled) VALUES ('cfg','u','Claude','claude',1); INSERT INTO upstream_credentials(config_version_id,id,upstream_id,kind,ciphertext,key_version,status,revision) VALUES ('cfg','c','u','bearer',X'01',1,'active',1);")?;
        let key = KeyVersion::try_new(1)?;
        let secrets = SecretStore::new(MasterKeyRing::try_new(
            key,
            [(key, MasterKey::try_from_bytes([31; 32])?)],
        )?);
        let sealed = secrets.seal(b"synthetic-refresh", b"synthetic-aad")?;
        Ok((
            CredentialRefreshStore { connection: c },
            ConfigVersionId::try_new("cfg")?,
            CredentialId::try_new("c")?,
            sealed,
        ))
    }
    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn claim_lease_fences_late_success_and_late_failure() -> TestResult {
        let (mut s, v, c, secret) = fixture()?;
        let first = s.claim(&v, &c, 1, A, 1_000, 60_000)?.ok_or("claim")?;
        assert!(s.claim(&v, &c, 1, B, 1_001, 60_000)?.is_none());
        let recovered = s.claim(&v, &c, 1, B, 61_000, 60_000)?.ok_or("recovery")?;
        assert!(!s.succeed(&first, &secret, 61_001)?);
        assert!(!s.fail(&first, RefreshFailure::ReauthRequired, 61_001)?);
        assert!(s.succeed(&recovered, &secret, 61_002)?);
        let row: (String,i64,String,i64) = s.connection.query_row("SELECT kind,revision,status,(SELECT count(*) FROM management_resource_audit_events) FROM upstream_credentials",[],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        assert_eq!(row, ("bearer".into(), 2, "active".into(), 1));
        assert!(s.claim(&v, &c, 1, A, 62_000, 60_000)?.is_none());
        Ok(())
    }

    #[test]
    fn persistent_backoff_revocation_and_manual_replacement_are_revision_scoped() -> TestResult {
        let (mut s, v, c, secret) = fixture()?;
        let first = s.claim(&v, &c, 1, A, 1_000, 60_000)?.ok_or("claim")?;
        assert!(s.fail(&first, RefreshFailure::Network, 1_001)?);
        assert!(s.claim(&v, &c, 1, B, 61_000, 60_000)?.is_none());
        let second = s.claim(&v, &c, 1, B, 61_001, 60_000)?.ok_or("claim")?;
        assert!(s.fail(&second, RefreshFailure::ReauthRequired, 61_002)?);
        assert!(s.claim(&v, &c, 1, A, 9_000_000, 60_000)?.is_none());
        s.connection.execute(
            "UPDATE upstream_credentials SET status='active',revision=3",
            [],
        )?;
        let manual = s.claim(&v, &c, 3, A, 61_003, 60_000)?.ok_or("new grant")?;
        assert!(!s.succeed(&second, &secret, 61_004)?);
        assert!(s.succeed(&manual, &secret, 61_004)?);
        assert_eq!(
            s.connection.query_row(
                "SELECT failure_count FROM credential_refresh_state",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            0
        );
        Ok(())
    }

    #[test]
    fn changed_credential_or_archived_graph_blocks_all_old_outcomes() -> TestResult {
        let (mut s, v, c, secret) = fixture()?;
        let claim = s.claim(&v, &c, 1, A, 1_000, 60_000)?.ok_or("claim")?;
        s.connection
            .execute("UPDATE upstream_credentials SET revision=2", [])?;
        assert!(!s.fail(&claim, RefreshFailure::ReauthRequired, 1_002)?);
        assert!(!s.succeed(&claim, &secret, 1_002)?);
        let newer = s.claim(&v, &c, 2, B, 1_003, 60_000)?.ok_or("claim")?;
        s.connection
            .execute("UPDATE config_versions SET status='archived'", [])?;
        assert!(!s.succeed(&newer, &secret, 1_004)?);
        assert!(s.claim(&v, &c, 2, A, 100_000, 60_000)?.is_none());
        Ok(())
    }

    #[test]
    fn schema_thirty_rolls_back_without_rolling_back_rotated_secrets_or_audits() -> TestResult {
        let (mut store, version, credential, secret) = fixture()?;
        let claim = store
            .claim(&version, &credential, 1, A, 1000, 60000)?
            .ok_or("claim")?;
        assert!(store.succeed(&claim, &secret, 1001)?);
        let saved = store.connection.query_row(
            "SELECT ciphertext,revision FROM upstream_credentials",
            [],
            |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, i64>(1)?)),
        )?;
        crate::rollback_to_version(&mut store.connection, 29)?;
        assert_eq!(crate::schema_version(&store.connection)?, Some(29));
        assert!(!crate::table_exists(
            &store.connection,
            "credential_refresh_state"
        )?);
        assert_eq!(
            store.connection.query_row(
                "SELECT ciphertext,revision FROM upstream_credentials",
                [],
                |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, i64>(1)?))
            )?,
            saved
        );
        assert_eq!(
            store.connection.query_row(
                "SELECT count(*) FROM management_resource_audit_events",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            1
        );
        crate::migrate(&mut store.connection)?;
        assert_eq!(
            crate::schema_version(&store.connection)?,
            Some(crate::CURRENT_SCHEMA_VERSION)
        );
        assert!(
            store
                .claim(&version, &credential, 2, B, 1002, 60000)?
                .is_some()
        );
        Ok(())
    }

    #[test]
    fn claims_and_backoff_survive_reopen_and_peer_connections() -> TestResult {
        let (fixture, v, c, _) = fixture()?;
        let path = std::env::temp_dir().join(format!(
            "cpar-refresh-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        // VACUUM produces a closed test copy, never touches production state.
        fixture
            .connection
            .execute("VACUUM INTO ?1", [path.to_str().ok_or("path")?])?;
        let mut first = CredentialRefreshStore::open(&path)?;
        let mut second = CredentialRefreshStore::open(&path)?;
        let claim = first.claim(&v, &c, 1, A, 1_000, 60_000)?.ok_or("claim")?;
        assert!(second.claim(&v, &c, 1, B, 1_001, 60_000)?.is_none());
        assert!(first.fail(&claim, RefreshFailure::InvalidResponse, 1_002)?);
        drop(first);
        drop(second);
        let mut restarted = CredentialRefreshStore::open(&path)?;
        assert!(restarted.claim(&v, &c, 1, B, 1_003, 60_000)?.is_none());
        assert!(restarted.claim(&v, &c, 1, B, 61_002, 60_000)?.is_some());
        drop(restarted);
        std::fs::remove_file(path)?;
        Ok(())
    }
}
