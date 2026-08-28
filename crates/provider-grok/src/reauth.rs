//! Native Grok reauthentication orchestration.
//!
//! This is the CPAR port of the useful part of the server-side `grok-register` reauth runner:
//! refresh-first fallback, one-account-at-a-time processing, bounded batches, and immediate
//! encrypted replacement. Provider interaction remains an injected operation so this module does
//! not open a browser, read a password vault, or silently contact an OAuth endpoint.

use std::{error::Error, fmt, sync::Arc};

use gateway_store::secret_store::{EncryptedSecret, PlaintextSecret};
use getrandom::fill as random_fill;
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::{
    GrokAccountCredential, GrokAccountPoolError, GrokAccountPoolStore, GrokAccountProvider,
    GrokBuildCredential, GrokConsoleSsoToken, GrokWebCredential, account_pool::credential_aad,
};

const MAX_REAUTH_BATCH: usize = 200;
const MIN_CLAIM_LEASE_MS: i64 = 1_000;
const MAX_CLAIM_LEASE_MS: i64 = 10 * 60 * 1_000;
const BASE_FAILURE_BACKOFF_MS: i64 = 60 * 1_000;
const MAX_FAILURE_BACKOFF_MS: i64 = 60 * 60 * 1_000;
const FAILURE_JITTER_RANGE_MS: u64 = 30 * 1_000 + 1;

/// Maximum number of accounts one reauthentication pass may claim.
pub const MAX_GROK_REAUTH_BATCH: usize = MAX_REAUTH_BATCH;

/// One ordered provider action in the refresh-first reauthentication plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokReauthAttempt {
    /// Reuse the current provider refresh token, when the credential supports it.
    Refresh,
    /// Start or continue an explicit Device Authorization flow.
    DeviceCode,
    /// Use an explicitly injected browser/SSO bridge supplied by the operator.
    BrowserSso,
}

/// Ordered fallback policy copied from `grok-register`, without its old CPA sink dependency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokReauthStrategy {
    /// Try refresh, then Device Code, then the external browser/SSO bridge once each.
    Auto,
    /// Only try the current refresh token once.
    RefreshOnly,
    /// Only run the explicitly supplied Device Code and browser/SSO bridge actions.
    InteractiveOnly,
}

impl GrokReauthStrategy {
    const fn attempts(self) -> &'static [GrokReauthAttempt] {
        match self {
            Self::Auto => &[
                GrokReauthAttempt::Refresh,
                GrokReauthAttempt::DeviceCode,
                GrokReauthAttempt::BrowserSso,
            ],
            Self::RefreshOnly => &[GrokReauthAttempt::Refresh],
            Self::InteractiveOnly => {
                &[GrokReauthAttempt::DeviceCode, GrokReauthAttempt::BrowserSso]
            }
        }
    }
}

/// One claimed reauthentication account. The current secret is borrowed only by the injected
/// provider operation and is never rendered through `Debug`.
pub struct GrokReauthJob {
    account_id: String,
    provider: GrokAccountProvider,
    claim_id: String,
    claim_expires_at_ms: i64,
    expected_revision: u64,
    identity_digest: [u8; 32],
    credential: PlaintextSecret,
}

impl GrokReauthJob {
    /// Returns the opaque CPAR account identifier.
    #[must_use]
    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    /// Returns the isolated native provider namespace.
    #[must_use]
    pub const fn provider(&self) -> GrokAccountProvider {
        self.provider
    }

    /// Returns the current credential only to the immediate injected operation.
    #[must_use]
    pub fn credential_bytes(&self) -> &[u8] {
        self.credential.as_bytes()
    }
}

impl fmt::Debug for GrokReauthJob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokReauthJob")
            .field("account_id", &self.account_id)
            .field("provider", &self.provider)
            .field("claim_id", &"<redacted>")
            .field("claim_expires_at_ms", &self.claim_expires_at_ms)
            .field("expected_revision", &self.expected_revision)
            .field("identity_digest", &"<redacted>")
            .field("credential", &"<redacted>")
            .finish()
    }
}

/// Provider operation result. A successful credential is validated again by CPAR immediately
/// before encryption, so an injected bridge cannot store an arbitrary payload.
pub enum GrokReauthResult {
    /// A complete replacement credential produced by the named one-shot action.
    Reauthenticated {
        /// Provider-specific replacement payload, validated before persistence.
        credential: GrokAccountCredential,
        /// Action that produced the replacement payload.
        acquisition: GrokReauthAttempt,
    },
    /// A bounded transport/provider failure; the account receives durable backoff.
    TransientFailure,
    /// The action is unavailable without an interactive operator/provider step.
    NeedsInteractive,
    /// The provider or operator denied the authorization; no automatic retry is scheduled.
    Denied,
}

impl fmt::Debug for GrokReauthResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reauthenticated { acquisition, .. } => formatter
                .debug_struct("Reauthenticated")
                .field("acquisition", acquisition)
                .field("credential", &"<redacted>")
                .finish(),
            Self::TransientFailure => formatter.write_str("TransientFailure"),
            Self::NeedsInteractive => formatter.write_str("NeedsInteractive"),
            Self::Denied => formatter.write_str("Denied"),
        }
    }
}

/// Provider-specific reauthentication seam. Implementations may reuse the existing native OAuth
/// transport, a Device Code poller, or the old project's browser/SSO helper, but must not mutate
/// CPAR persistence directly.
pub trait GrokReauthExecutor: Sync {
    /// Executes exactly one named action for one claimed account.
    fn execute(&self, job: &GrokReauthJob, attempt: GrokReauthAttempt) -> GrokReauthResult;
}

/// Value-free result of one bounded serial reauthentication pass.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GrokReauthRunSummary {
    /// Accounts claimed by this pass.
    pub claimed: usize,
    /// Accounts whose replacement was durably committed.
    pub succeeded: usize,
    /// Successful refresh actions.
    pub refreshed: usize,
    /// Successful Device Code actions.
    pub device_code: usize,
    /// Successful browser/SSO bridge actions.
    pub browser_sso: usize,
    /// Accounts that still need an operator/provider interaction.
    pub interactive_required: usize,
    /// Accounts placed into bounded transient backoff.
    pub backed_off: usize,
    /// Accounts explicitly denied by the provider/operator.
    pub denied: usize,
}

/// Safe failure classes for native reauthentication state and coordination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokReauthError {
    /// A bound, timestamp, strategy, or result shape was invalid.
    InvalidRequest,
    /// `SQLite`, locking, or migration state was unavailable.
    StoreUnavailable,
    /// Persisted account or reauth state was malformed.
    InvalidPersistedState,
    /// CPAR could not authenticate/decrypt/seal the account credential.
    SecretStoreFailure,
    /// A claim or revision was stale and no mutation was committed.
    StaleClaim,
    /// The requested account does not exist in the native account pool.
    NotFound,
    /// The provider replacement did not match the account's strict credential shape.
    InvalidCredential,
}

impl fmt::Display for GrokReauthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "native Grok reauth request is invalid",
            Self::StoreUnavailable => "native Grok reauth state is unavailable",
            Self::InvalidPersistedState => "native Grok reauth state is invalid",
            Self::SecretStoreFailure => "native Grok reauth credential encryption failed",
            Self::StaleClaim => "native Grok reauth claim is stale",
            Self::NotFound => "native Grok reauth account was not found",
            Self::InvalidCredential => "native Grok reauth credential is invalid",
        })
    }
}

impl Error for GrokReauthError {}

impl From<GrokAccountPoolError> for GrokReauthError {
    fn from(error: GrokAccountPoolError) -> Self {
        match error {
            GrokAccountPoolError::StoreUnavailable => Self::StoreUnavailable,
            GrokAccountPoolError::SecretStoreFailure => Self::SecretStoreFailure,
            GrokAccountPoolError::InvalidPersistedState => Self::InvalidPersistedState,
            GrokAccountPoolError::InvalidCredential => Self::InvalidCredential,
            GrokAccountPoolError::NotFound => Self::NotFound,
            _ => Self::InvalidRequest,
        }
    }
}

/// Bounded serial coordinator. It intentionally never opens multiple OAuth/browser flows at once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrokReauthCoordinator {
    claim_lease_ms: i64,
}

impl GrokReauthCoordinator {
    /// Creates a coordinator with a finite claim lease.
    ///
    /// # Errors
    ///
    /// Returns [`GrokReauthError::InvalidRequest`] when the lease is outside the bounded range.
    pub fn try_new(claim_lease_ms: i64) -> Result<Self, GrokReauthError> {
        if !(MIN_CLAIM_LEASE_MS..=MAX_CLAIM_LEASE_MS).contains(&claim_lease_ms) {
            return Err(GrokReauthError::InvalidRequest);
        }
        Ok(Self { claim_lease_ms })
    }

    /// Runs at most `limit` accounts in serial order, applying the source project's one-account
    /// fallback semantics. Every action is invoked at most once per account in this pass.
    ///
    /// # Errors
    ///
    /// Returns a safe reauthentication or persistence error; a failed mutation is rolled back.
    pub fn run_once<E: GrokReauthExecutor>(
        self,
        store: &Arc<GrokAccountPoolStore>,
        strategy: GrokReauthStrategy,
        limit: usize,
        observed_at_ms: i64,
        executor: &E,
    ) -> Result<GrokReauthRunSummary, GrokReauthError> {
        if observed_at_ms < 0 || limit == 0 || limit > MAX_REAUTH_BATCH {
            return Err(GrokReauthError::InvalidRequest);
        }
        let mut summary = GrokReauthRunSummary::default();
        for _ in 0..limit {
            let Some(job) = store.claim_due_reauth_job(observed_at_ms, self.claim_lease_ms)? else {
                break;
            };
            summary.claimed += 1;
            let mut completed = false;
            for attempt in strategy.attempts() {
                match executor.execute(&job, *attempt) {
                    GrokReauthResult::Reauthenticated {
                        credential,
                        acquisition,
                    } => {
                        store.complete_reauth_success(&job, &credential, observed_at_ms)?;
                        summary.succeeded += 1;
                        match acquisition {
                            GrokReauthAttempt::Refresh => summary.refreshed += 1,
                            GrokReauthAttempt::DeviceCode => summary.device_code += 1,
                            GrokReauthAttempt::BrowserSso => summary.browser_sso += 1,
                        }
                        completed = true;
                        break;
                    }
                    GrokReauthResult::TransientFailure => {
                        store.complete_reauth_failure(&job, true, observed_at_ms)?;
                        summary.backed_off += 1;
                        completed = true;
                        break;
                    }
                    GrokReauthResult::NeedsInteractive => {}
                    GrokReauthResult::Denied => {
                        store.complete_reauth_failure(&job, false, observed_at_ms)?;
                        summary.denied += 1;
                        completed = true;
                        break;
                    }
                }
            }
            if !completed {
                store.complete_reauth_failure(&job, false, observed_at_ms)?;
                summary.interactive_required += 1;
            }
        }
        Ok(summary)
    }
}

impl GrokAccountPoolStore {
    /// Releases one manually blocked account back to the bounded reauthentication queue.
    ///
    /// This is the explicit hand-off point for an operator or an external browser/SSO bridge;
    /// no password, browser cookie, or OAuth token is accepted by this method.
    ///
    /// # Errors
    ///
    /// Returns [`GrokReauthError::NotFound`] when the account is not a reauthentication-required
    /// account, or a safe storage/request error otherwise.
    pub fn requeue_reauth(
        &self,
        account_id: &str,
        observed_at_ms: i64,
    ) -> Result<(), GrokReauthError> {
        if account_id.is_empty() || account_id.len() > 128 || observed_at_ms < 0 {
            return Err(GrokReauthError::InvalidRequest);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        let changed = connection
            .execute(
                "UPDATE grok_account_reauth_state SET next_attempt_at_ms = ?2, \
                        operator_required = 0, claim_id = NULL, claim_expires_at_ms = NULL \
                 WHERE account_id = ?1 AND EXISTS (\
                     SELECT 1 FROM grok_accounts WHERE id = ?1 AND auth_status = 'reauth_required'\
                 ) AND operator_required = 1 AND (\
                     claim_id IS NULL OR claim_expires_at_ms <= ?2\
                 )",
                params![account_id, observed_at_ms],
            )
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        if changed != 1 {
            return Err(GrokReauthError::NotFound);
        }
        Ok(())
    }

    /// Claims one due reauthentication account. The claim is independent of the normal active
    /// refresh/quota worker claim so an unauthorized account cannot be silently scheduled.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, persistence, decryption, or claim error. No partial claim is
    /// committed on failure.
    pub fn claim_due_reauth_job(
        &self,
        observed_at_ms: i64,
        claim_lease_ms: i64,
    ) -> Result<Option<GrokReauthJob>, GrokReauthError> {
        if observed_at_ms < 0
            || !(MIN_CLAIM_LEASE_MS..=MAX_CLAIM_LEASE_MS).contains(&claim_lease_ms)
        {
            return Err(GrokReauthError::InvalidRequest);
        }
        let claim_expires_at_ms = observed_at_ms
            .checked_add(claim_lease_ms)
            .ok_or(GrokReauthError::InvalidRequest)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO grok_account_reauth_state (account_id) \
                 SELECT id FROM grok_accounts WHERE auth_status = 'reauth_required'",
                [],
            )
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        let row = transaction
            .query_row(
                "SELECT a.id, a.provider, a.identity_digest, a.credential_ciphertext, \
                        a.credential_key_version, a.revision \
                 FROM grok_accounts a \
                 JOIN grok_account_reauth_state r ON r.account_id = a.id \
                 WHERE a.enabled = 1 AND a.auth_status = 'reauth_required' \
                   AND r.operator_required = 0 \
                   AND (r.next_attempt_at_ms IS NULL OR r.next_attempt_at_ms <= ?1) \
                   AND (r.claim_id IS NULL OR r.claim_expires_at_ms <= ?1) \
                 ORDER BY COALESCE(r.next_attempt_at_ms, 0), a.id LIMIT 1",
                [observed_at_ms],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        let Some((account_id, provider, identity_digest, ciphertext, key_version, revision)) = row
        else {
            transaction
                .commit()
                .map_err(|_| GrokReauthError::StoreUnavailable)?;
            return Ok(None);
        };
        let provider = parse_provider(&provider)?;
        let identity_digest: [u8; 32] = identity_digest
            .try_into()
            .map_err(|_| GrokReauthError::InvalidPersistedState)?;
        let key_version = gateway_store::secret_store::KeyVersion::try_from_sqlite_i64(key_version)
            .map_err(|_| GrokReauthError::InvalidPersistedState)?;
        let encrypted = EncryptedSecret::try_from_persisted(key_version, ciphertext)
            .map_err(|_| GrokReauthError::InvalidPersistedState)?;
        let credential = self
            .secret_store
            .open(&encrypted, &credential_aad(provider, &identity_digest))
            .map_err(|_| GrokReauthError::SecretStoreFailure)?;
        let expected_revision =
            u64::try_from(revision).map_err(|_| GrokReauthError::InvalidPersistedState)?;
        let claim_id = random_claim_id()?;
        let changed = transaction
            .execute(
                "UPDATE grok_account_reauth_state SET claim_id = ?2, claim_expires_at_ms = ?3 \
                 WHERE account_id = ?1 AND (claim_id IS NULL OR claim_expires_at_ms <= ?4)",
                params![account_id, claim_id, claim_expires_at_ms, observed_at_ms],
            )
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        if changed != 1 {
            return Err(GrokReauthError::StaleClaim);
        }
        transaction
            .commit()
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        Ok(Some(GrokReauthJob {
            account_id,
            provider,
            claim_id,
            claim_expires_at_ms,
            expected_revision,
            identity_digest,
            credential,
        }))
    }

    /// Validates, seals, and atomically commits one replacement credential at the claimed revision.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, credential, encryption, or stale-claim error. The account
    /// remains unchanged when the commit cannot prove the exact claim and revision.
    pub fn complete_reauth_success(
        &self,
        job: &GrokReauthJob,
        credential: &GrokAccountCredential,
        observed_at_ms: i64,
    ) -> Result<(), GrokReauthError> {
        if observed_at_ms < 0 || credential.as_bytes().is_empty() {
            return Err(GrokReauthError::InvalidRequest);
        }
        let refresh_due_at_ms =
            validated_expiry(job.provider, credential.as_bytes(), observed_at_ms)?;
        let encrypted = self
            .secret_store
            .seal(
                credential.as_bytes(),
                &credential_aad(job.provider, &job.identity_digest),
            )
            .map_err(|_| GrokReauthError::SecretStoreFailure)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        let next_revision = job
            .expected_revision
            .checked_add(1)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(GrokReauthError::InvalidPersistedState)?;
        let changed = transaction
            .execute(
                "UPDATE grok_accounts SET credential_ciphertext = ?1, credential_key_version = ?2, \
                        revision = ?3, auth_status = 'active', refresh_due_at_ms = ?4, \
                        last_refresh_at_ms = ?5, refresh_failure_count = 0, updated_at_ms = ?5 \
                 WHERE id = ?6 AND revision = ?7 AND auth_status = 'reauth_required'",
                params![
                    encrypted.ciphertext(),
                    encrypted.key_version().as_sqlite_i64(),
                    next_revision,
                    refresh_due_at_ms,
                    observed_at_ms,
                    job.account_id,
                    i64::try_from(job.expected_revision)
                        .map_err(|_| GrokReauthError::InvalidPersistedState)?,
                ],
            )
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        if changed != 1 {
            return Err(GrokReauthError::StaleClaim);
        }
        let claim_changed = transaction
            .execute(
                "DELETE FROM grok_account_reauth_state WHERE account_id = ?1 AND claim_id = ?2 \
                 AND claim_expires_at_ms > ?3",
                params![job.account_id, job.claim_id, observed_at_ms],
            )
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        if claim_changed != 1 {
            return Err(GrokReauthError::StaleClaim);
        }
        transaction
            .commit()
            .map_err(|_| GrokReauthError::StoreUnavailable)
    }

    /// Completes a failed action. Transient failures receive deterministic backoff; interactive
    /// or denied outcomes remain blocked until a later explicit operator action.
    ///
    /// # Errors
    ///
    /// Returns a safe validation, persistence, or stale-claim error. The claim is cleared only
    /// after the failure state is durably updated.
    pub fn complete_reauth_failure(
        &self,
        job: &GrokReauthJob,
        transient: bool,
        observed_at_ms: i64,
    ) -> Result<(), GrokReauthError> {
        if observed_at_ms < 0 {
            return Err(GrokReauthError::InvalidRequest);
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        let failure_count: i64 = transaction
            .query_row(
                "SELECT failure_count FROM grok_account_reauth_state \
                 WHERE account_id = ?1 AND claim_id = ?2 AND claim_expires_at_ms > ?3",
                params![job.account_id, job.claim_id, observed_at_ms],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| GrokReauthError::StoreUnavailable)?
            .ok_or(GrokReauthError::StaleClaim)?;
        let next_failure_count = failure_count
            .checked_add(1)
            .ok_or(GrokReauthError::InvalidPersistedState)?;
        let next_attempt_at_ms = if transient {
            Some(backoff_due_at(
                &job.account_id,
                u64::try_from(next_failure_count)
                    .map_err(|_| GrokReauthError::InvalidPersistedState)?,
                observed_at_ms,
            )?)
        } else {
            None
        };
        let changed = transaction
            .execute(
                "UPDATE grok_account_reauth_state SET next_attempt_at_ms = ?1, failure_count = ?2, \
                        operator_required = ?3, claim_id = NULL, claim_expires_at_ms = NULL \
                 WHERE account_id = ?4 AND claim_id = ?5 AND claim_expires_at_ms > ?6",
                params![
                    next_attempt_at_ms,
                    next_failure_count,
                    i64::from(!transient),
                    job.account_id,
                    job.claim_id,
                    observed_at_ms,
                ],
            )
            .map_err(|_| GrokReauthError::StoreUnavailable)?;
        if changed != 1 {
            return Err(GrokReauthError::StaleClaim);
        }
        transaction
            .commit()
            .map_err(|_| GrokReauthError::StoreUnavailable)
    }
}

fn parse_provider(value: &str) -> Result<GrokAccountProvider, GrokReauthError> {
    match value {
        "build" => Ok(GrokAccountProvider::Build),
        "web" => Ok(GrokAccountProvider::Web),
        "console" => Ok(GrokAccountProvider::Console),
        _ => Err(GrokReauthError::InvalidPersistedState),
    }
}

fn validated_expiry(
    provider: GrokAccountProvider,
    credential: &[u8],
    observed_at_ms: i64,
) -> Result<Option<i64>, GrokReauthError> {
    match provider {
        GrokAccountProvider::Build => {
            GrokBuildCredential::import_runtime_json(credential, observed_at_ms)
                .map(|value| Some(value.expires_at_ms()))
                .map_err(|_| GrokReauthError::InvalidCredential)
        }
        GrokAccountProvider::Web => GrokWebCredential::import_sso_json(credential, observed_at_ms)
            .map(|value| Some(value.expires_at_ms()))
            .map_err(|_| GrokReauthError::InvalidCredential),
        GrokAccountProvider::Console => GrokConsoleSsoToken::try_from_bytes(credential)
            .map(|_| None)
            .map_err(|_| GrokReauthError::InvalidCredential),
    }
}

fn random_claim_id() -> Result<String, GrokReauthError> {
    let mut random = [0_u8; 16];
    random_fill(&mut random).map_err(|_| GrokReauthError::StoreUnavailable)?;
    let mut id = String::from("grok-reauth-");
    for byte in random {
        use std::fmt::Write as _;
        write!(&mut id, "{byte:02x}").map_err(|_| GrokReauthError::StoreUnavailable)?;
    }
    Ok(id)
}

fn backoff_due_at(
    account_id: &str,
    failure_count: u64,
    observed_at_ms: i64,
) -> Result<i64, GrokReauthError> {
    if account_id.is_empty() || failure_count == 0 || observed_at_ms < 0 {
        return Err(GrokReauthError::InvalidRequest);
    }
    let shift = u32::try_from(failure_count.saturating_sub(1).min(16))
        .map_err(|_| GrokReauthError::InvalidRequest)?;
    let backoff = BASE_FAILURE_BACKOFF_MS
        .checked_mul(1_i64 << shift)
        .unwrap_or(MAX_FAILURE_BACKOFF_MS)
        .min(MAX_FAILURE_BACKOFF_MS);
    let mut material = account_id.as_bytes().to_vec();
    material.extend_from_slice(&failure_count.to_be_bytes());
    let digest = Sha256::digest(material);
    let jitter = u64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]) % FAILURE_JITTER_RANGE_MS;
    observed_at_ms
        .checked_add(backoff)
        .and_then(|value| value.checked_add(i64::try_from(jitter).ok()?))
        .ok_or(GrokReauthError::InvalidRequest)
}
