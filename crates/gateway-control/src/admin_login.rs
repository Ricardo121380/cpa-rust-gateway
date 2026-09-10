//! Single-administrator sessions. No configuration, provider or client-key authorization lives here.
use gateway_auth::admin_password::{
    hash_password, random_secret, valid_hash, valid_password, verify_password,
};
use gateway_store::admin_login::{AdminAccount, AdminAccountStore};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

const LOGIN_WINDOW: Duration = Duration::from_mins(1);
const MAX_VERIFICATIONS: usize = 10;
const MAX_SESSIONS: usize = 32;
const SESSION_TTL: Duration = Duration::from_hours(8);
const INITIAL_SESSION_TTL: Duration = Duration::from_mins(10);

/// Public-safe authentication failure, without account enumeration or secret values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminLoginError {
    /// Username or password does not match.
    InvalidCredentials,
    /// Session was revoked/expired, or its CSRF does not match.
    InvalidSession,
    /// The bounded password-verification budget is exhausted.
    RateLimited,
    /// Password does not meet policy or is unchanged.
    InvalidPassword,
    /// Local credential store or runtime is unavailable.
    Unavailable,
}
impl fmt::Display for AdminLoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("administrator authentication failed")
    }
}
impl Error for AdminLoginError {}

/// The transport sends these values once and must never log them.
pub struct AdminSessionGrant {
    /// Browser transport session, separate from the CLI key namespace.
    pub session_token: Zeroizing<String>,
    /// Independent CSRF proof for unsafe requests.
    pub csrf_token: Zeroizing<String>,
    /// Non-secret account identity.
    pub username: String,
    /// Absolute expiry in milliseconds since Unix epoch.
    pub expires_at_ms: u64,
    /// Restricted until the initial password is changed.
    pub password_change_required: bool,
}

/// Identity safe for management audit, with initial-session admission restriction.
#[derive(Clone, Debug)]
pub struct AdminSessionIdentity {
    /// Non-secret username.
    pub username: String,
    /// Only password change/logout are allowed until this is false.
    pub password_change_required: bool,
}
struct Session {
    expires: Instant,
    csrf_digest: [u8; 32],
}
struct State {
    account: AdminAccount,
    changing: bool,
    window: Instant,
    verifications: usize,
    sessions: BTreeMap<[u8; 32], Session>,
}
/// Shared across HTTP workers. Hash/SQLite operations must run on a blocking thread.
pub struct AdminLoginService {
    store: AdminAccountStore,
    state: Mutex<State>,
}
impl AdminLoginService {
    /// Load the explicit singleton administrator, without bootstrapping on startup.
    /// # Errors
    /// An absent/corrupt/unrecognized hash fails closed.
    pub fn new(store: AdminAccountStore) -> Result<Self, AdminLoginError> {
        let account = store.load().map_err(|_| AdminLoginError::Unavailable)?;
        if !valid_hash(&account.password_hash) {
            return Err(AdminLoginError::Unavailable);
        }
        Ok(Self {
            store,
            state: Mutex::new(State {
                account,
                changing: false,
                window: Instant::now(),
                verifications: 0,
                sessions: BTreeMap::new(),
            }),
        })
    }

    fn account_for_verification(&self) -> Result<AdminAccount, AdminLoginError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AdminLoginError::Unavailable)?;
        if state.changing {
            return Err(AdminLoginError::Unavailable);
        }
        if state.window.elapsed() >= LOGIN_WINDOW {
            state.window = Instant::now();
            state.verifications = 0;
        }
        if state.verifications >= MAX_VERIFICATIONS {
            return Err(AdminLoginError::RateLimited);
        }
        state.verifications += 1;
        Ok(state.account.clone())
    }

    /// Verify and issue a bounded opaque session. Unknown usernames still run the same KDF.
    /// # Errors
    /// Uniform bad credentials, bounded rate rejection, or runtime unavailability.
    pub fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<AdminSessionGrant, AdminLoginError> {
        let account = self.account_for_verification()?;
        let valid = verify_password(password, &account.password_hash);
        if !valid || username != account.username {
            return Err(AdminLoginError::InvalidCredentials);
        }
        let token = random_secret("session_").map_err(|_| AdminLoginError::Unavailable)?;
        let csrf = random_secret("csrf_").map_err(|_| AdminLoginError::Unavailable)?;
        let ttl = if account.password_change_required {
            INITIAL_SESSION_TTL
        } else {
            SESSION_TTL
        };
        let expires_at_ms = SystemTime::now()
            .checked_add(ttl)
            .ok_or(AdminLoginError::Unavailable)?
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AdminLoginError::Unavailable)?
            .as_millis();
        let mut state = self
            .state
            .lock()
            .map_err(|_| AdminLoginError::Unavailable)?;
        if state.changing || state.account.revision != account.revision {
            return Err(AdminLoginError::InvalidCredentials);
        }
        let now = Instant::now();
        state.sessions.retain(|_, session| session.expires > now);
        if state.sessions.len() >= MAX_SESSIONS {
            // Refresh drops browser memory, so abandoned sessions must not lock out a valid
            // administrator for eight hours. Replace the oldest after verifying the password.
            let oldest = state
                .sessions
                .iter()
                .min_by_key(|(_, session)| session.expires)
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                state.sessions.remove(&oldest);
            }
        }
        state.sessions.insert(
            digest(&token),
            Session {
                expires: now + ttl,
                csrf_digest: digest(&csrf),
            },
        );
        Ok(AdminSessionGrant {
            session_token: token,
            csrf_token: csrf,
            username: account.username,
            expires_at_ms: u64::try_from(expires_at_ms)
                .map_err(|_| AdminLoginError::Unavailable)?,
            password_change_required: account.password_change_required,
        })
    }

    /// Authenticate without I/O/KDF; optionally require the independent CSRF proof.
    /// # Errors
    /// Expired, revoked, restricted-in-progress or forged material is rejected.
    pub fn authenticate(
        &self,
        token: &str,
        csrf: Option<&str>,
        unsafe_request: bool,
    ) -> Result<AdminSessionIdentity, AdminLoginError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AdminLoginError::Unavailable)?;
        let now = Instant::now();
        state.sessions.retain(|_, session| session.expires > now);
        if state.changing {
            return Err(AdminLoginError::InvalidSession);
        }
        let session = state
            .sessions
            .get(&digest(token))
            .ok_or(AdminLoginError::InvalidSession)?;
        if unsafe_request
            && !csrf.is_some_and(|value| {
                gateway_auth::admin_password::constant_time_match(
                    &session.csrf_digest,
                    &digest(value),
                )
            })
        {
            return Err(AdminLoginError::InvalidSession);
        }
        Ok(AdminSessionIdentity {
            username: state.account.username.clone(),
            password_change_required: state.account.password_change_required,
        })
    }

    /// Revoke exactly one session; callers already enforce its CSRF proof.
    /// # Errors
    /// A poisoned runtime is unavailable.
    pub fn logout(&self, token: &str) -> Result<(), AdminLoginError> {
        self.state
            .lock()
            .map_err(|_| AdminLoginError::Unavailable)?
            .sessions
            .remove(&digest(token));
        Ok(())
    }

    /// Durable password replacement revokes every session, including the caller.
    /// # Errors
    /// Invalid current password, reused/short new password, races or storage errors fail closed.
    pub fn change_password(
        &self,
        token: &str,
        csrf: &str,
        current: &str,
        new: &str,
    ) -> Result<(), AdminLoginError> {
        self.authenticate(token, Some(csrf), true)?;
        if !valid_password(new) || new == current {
            return Err(AdminLoginError::InvalidPassword);
        }
        let account = self.account_for_verification()?;
        if !verify_password(current, &account.password_hash) {
            return Err(AdminLoginError::InvalidCredentials);
        }
        let hash = Zeroizing::new(hash_password(new).map_err(|_| AdminLoginError::Unavailable)?);
        // Re-check after the KDF: logout/expiry or another password change must win.
        self.authenticate(token, Some(csrf), true)?;
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| AdminLoginError::Unavailable)?;
            if state.changing
                || state.account.revision != account.revision
                || !state.sessions.contains_key(&digest(token))
            {
                return Err(AdminLoginError::InvalidSession);
            }
            state.changing = true;
            state.sessions.clear();
        }
        // No state lock is held during durable I/O: HTTP admission stays nonblocking.
        let result = self.store.change_password(account.revision, &hash);
        let mut state = self
            .state
            .lock()
            .map_err(|_| AdminLoginError::Unavailable)?;
        if result.is_err() {
            // Stay unavailable if commit outcome is uncertain; restart reloads durable truth.
            return Err(AdminLoginError::Unavailable);
        }
        state.account.password_hash = hash;
        state.account.revision += 1;
        state.account.password_change_required = false;
        state.changing = false;
        Ok(())
    }
}
fn digest(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};
    type TestResult = Result<(), Box<dyn Error>>;
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn setup() -> Result<(Temp, AdminLoginService), Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!(
            "admin-sessions-{}-{}-{}",
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        fs::create_dir(&path)?;
        let temp = Temp(path);
        let store = AdminAccountStore::initialize(
            temp.0.join("admin-auth.sqlite3"),
            "admin",
            &hash_password("synthetic-passphrase")?,
        )?;
        Ok((temp, AdminLoginService::new(store)?))
    }
    #[test]
    fn expired_sessions_are_removed_and_session_capacity_is_bounded() -> TestResult {
        let (_temp, service) = setup()?;
        let grant = service.login("admin", "synthetic-passphrase")?;
        {
            let mut state = service.state.lock().map_err(|_| "state")?;
            state
                .sessions
                .get_mut(&digest(&grant.session_token))
                .ok_or("session")?
                .expires = Instant::now()
                .checked_sub(Duration::from_secs(1))
                .ok_or("clock")?;
        }
        assert!(matches!(
            service.authenticate(&grant.session_token, None, false),
            Err(AdminLoginError::InvalidSession)
        ));
        assert!(
            service
                .state
                .lock()
                .map_err(|_| "state")?
                .sessions
                .is_empty()
        );
        {
            let mut state = service.state.lock().map_err(|_| "state")?;
            for i in 0..MAX_SESSIONS {
                state.sessions.insert(
                    digest(&i.to_string()),
                    Session {
                        expires: Instant::now() + SESSION_TTL,
                        csrf_digest: [0; 32],
                    },
                );
            }
        }
        let fresh = service.login("admin", "synthetic-passphrase")?;
        assert!(
            service
                .authenticate(&fresh.session_token, None, false)
                .is_ok()
        );
        assert_eq!(
            service.state.lock().map_err(|_| "state")?.sessions.len(),
            MAX_SESSIONS
        );
        Ok(())
    }
    #[test]
    fn a_login_budget_recovers_after_its_window_without_creating_unbounded_identity_buckets()
    -> TestResult {
        let (_temp, service) = setup()?;
        {
            let mut state = service.state.lock().map_err(|_| "state")?;
            state.verifications = MAX_VERIFICATIONS;
        }
        assert!(matches!(
            service.login("unknown", "anything"),
            Err(AdminLoginError::RateLimited)
        ));
        service.state.lock().map_err(|_| "state")?.window =
            Instant::now().checked_sub(LOGIN_WINDOW).ok_or("clock")?;
        assert!(service.login("admin", "synthetic-passphrase").is_ok());
        Ok(())
    }
}
