//! P7-01 strict Kiro credential boundary regressions.

use std::{
    error::Error,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use gateway_core::CredentialId;
use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
use provider_kiro::credential::{
    KiroCredential, KiroCredentialCasOutcome, KiroCredentialError, KiroCredentialKind,
    KiroCredentialRefreshCoordinator, KiroRefreshKind, KiroRefreshRequest, KiroRefreshResponse,
    KiroRefreshTransport,
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn three_credential_families_are_strictly_distinct_and_redacted() -> TestResult {
    let social = KiroCredential::import_json(
        br#"{"kind":"social","access_token":"fixture-social-access","refresh_token":"fixture-social-refresh","expires_at_ms":3601000}"#,
        1_000,
    )?;
    let enterprise = KiroCredential::import_json(
        br#"{"kind":"enterprise","access_token":"fixture-enterprise-access","refresh_token":"fixture-enterprise-refresh","expires_at_ms":3601000,"client_id":"fixture-client","client_secret":"fixture-client-secret","auth_region":"us-east-1"}"#,
        1_000,
    )?;
    let api_key = KiroCredential::import_json(
        br#"{"kind":"api_key","api_key":"ksk_fixture_nonlive"}"#,
        1_000,
    )?;

    assert_eq!(social.kind(), KiroCredentialKind::Social);
    assert_eq!(enterprise.kind(), KiroCredentialKind::Enterprise);
    assert_eq!(enterprise.auth_region(), Some("us-east-1"));
    assert_eq!(api_key.kind(), KiroCredentialKind::ApiKey);
    assert!(matches!(
        api_key.access_token(),
        Err(KiroCredentialError::WrongCredentialKind)
    ));
    assert!(matches!(
        social.api_key(),
        Err(KiroCredentialError::WrongCredentialKind)
    ));

    let debug = format!("{social:?} {enterprise:?} {api_key:?}");
    for value in [
        "fixture-social-access",
        "fixture-enterprise-access",
        "fixture-client-secret",
        "ksk_fixture_nonlive",
    ] {
        assert!(!debug.contains(value));
    }
    Ok(())
}

#[test]
fn import_rejects_duplicate_mixed_expired_and_malformed_credential_shapes() {
    let duplicate = KiroCredential::import_json(
        br#"{"kind":"api_key","kind":"api_key","api_key":"ksk_fixture_nonlive"}"#,
        1_000,
    );
    assert!(matches!(duplicate, Err(KiroCredentialError::InvalidJson)));

    let mixed = KiroCredential::import_json(
        br#"{"kind":"social","access_token":"fixture-access","refresh_token":"fixture-refresh","expires_at_ms":3601000,"api_key":"ksk_fixture_nonlive"}"#,
        1_000,
    );
    assert!(matches!(mixed, Err(KiroCredentialError::UnexpectedField)));

    let expired = KiroCredential::import_json(
        br#"{"kind":"social","access_token":"fixture-access","refresh_token":"fixture-refresh","expires_at_ms":1000}"#,
        1_000,
    );
    assert!(matches!(
        expired,
        Err(KiroCredentialError::InvalidTimestamp)
    ));

    let invalid_region = KiroCredential::import_json(
        br#"{"kind":"enterprise","access_token":"fixture-access","refresh_token":"fixture-refresh","expires_at_ms":3601000,"client_id":"fixture-client","client_secret":"fixture-secret","auth_region":"US_EAST_1"}"#,
        1_000,
    );
    assert!(matches!(
        invalid_region,
        Err(KiroCredentialError::InvalidRegion)
    ));
}

#[test]
fn refresh_is_kind_specific_redacted_and_never_refreshes_an_api_key() -> TestResult {
    let social = KiroCredential::import_json(
        br#"{"kind":"social","access_token":"fixture-old-access","refresh_token":"fixture-old-refresh","expires_at_ms":2000}"#,
        1_000,
    )?;
    let enterprise = KiroCredential::import_json(
        br#"{"kind":"enterprise","access_token":"fixture-old-access","refresh_token":"fixture-old-refresh","expires_at_ms":2000,"client_id":"fixture-client","client_secret":"fixture-secret","auth_region":"us-west-2"}"#,
        1_000,
    )?;
    let transport = FixtureRefresh::default();

    let refreshed_social = social.refresh(&transport, 1_500)?;
    let refreshed_enterprise = enterprise.refresh(&transport, 1_500)?;
    assert_eq!(refreshed_social.access_token()?, "fixture-new-access");
    assert_eq!(refreshed_enterprise.auth_region(), Some("us-west-2"));
    assert_eq!(
        transport.calls.lock().map_err(|_| "poisoned")?.as_slice(),
        &[KiroRefreshKind::Social, KiroRefreshKind::Enterprise]
    );
    assert!(format!("{transport:?}").contains("<redacted>"));

    let api_key = KiroCredential::import_json(
        br#"{"kind":"api_key","api_key":"ksk_fixture_nonlive"}"#,
        1_000,
    )?;
    assert!(matches!(
        api_key.refresh(&transport, 1_500),
        Err(KiroCredentialError::NotRefreshable)
    ));
    Ok(())
}

#[test]
fn sealed_credential_is_aead_bound_to_its_exact_credential_id() -> TestResult {
    let credential = KiroCredential::import_json(
        br#"{"kind":"enterprise","access_token":"fixture-access","refresh_token":"fixture-refresh","expires_at_ms":3601000,"client_id":"fixture-client","client_secret":"fixture-secret","auth_region":"eu-west-1"}"#,
        1_000,
    )?;
    let store = secret_store()?;
    let owner = CredentialId::try_new("kiro-owner")?;
    let other = CredentialId::try_new("kiro-other")?;
    let sealed = credential.seal(&store, &owner)?;
    assert!(
        !sealed
            .encrypted_secret()
            .ciphertext()
            .windows(b"fixture-access".len())
            .any(|window| window == b"fixture-access")
    );
    assert_eq!(
        sealed.open(&store, &owner)?.access_token()?,
        "fixture-access"
    );
    assert!(matches!(
        sealed.open(&store, &other),
        Err(KiroCredentialError::EncryptionFailed)
    ));
    let social = KiroCredential::import_json(
        br#"{"kind":"social","access_token":"fixture-social-access","refresh_token":"fixture-social-refresh","expires_at_ms":3601000}"#,
        1_000,
    )?;
    let social_id = CredentialId::try_new("kiro-social-owner")?;
    assert_eq!(
        social
            .seal(&store, &social_id)?
            .open(&store, &social_id)?
            .access_token()?,
        "fixture-social-access"
    );
    Ok(())
}

#[test]
fn same_credential_expiry_singleflights_and_followers_observe_the_new_revision() -> TestResult {
    let credential_id = CredentialId::try_new("kiro-refresh-owner")?;
    let coordinator = Arc::new(KiroCredentialRefreshCoordinator::try_new([(
        credential_id.clone(),
        expired_social()?,
    )])?);
    let transport = Arc::new(BlockingRefresh::new());

    let leader = {
        let coordinator = Arc::clone(&coordinator);
        let transport = Arc::clone(&transport);
        let credential_id = credential_id.clone();
        thread::spawn(move || {
            coordinator.refresh_if_expired(&credential_id, transport.as_ref(), 2_000)
        })
    };
    transport.wait_until_started()?;
    let follower = {
        let coordinator = Arc::clone(&coordinator);
        let transport = Arc::clone(&transport);
        let credential_id = credential_id.clone();
        thread::spawn(move || {
            coordinator.refresh_if_expired(&credential_id, transport.as_ref(), 2_000)
        })
    };
    transport.release()?;
    assert_eq!(leader.join().map_err(|_| "leader panic")??.revision(), 1);
    assert_eq!(
        follower.join().map_err(|_| "follower panic")??.revision(),
        1
    );
    assert_eq!(transport.call_count()?, 1);
    assert_eq!(
        coordinator
            .load(&credential_id)?
            .credential()
            .access_token()?,
        "fixture-new-access"
    );
    Ok(())
}

#[test]
fn different_credentials_refresh_without_waiting_for_each_other() -> TestResult {
    let first_id = CredentialId::try_new("kiro-first-owner")?;
    let second_id = CredentialId::try_new("kiro-second-owner")?;
    let coordinator = Arc::new(KiroCredentialRefreshCoordinator::try_new([
        (first_id.clone(), expired_social()?),
        (second_id.clone(), expired_social()?),
    ])?);
    let transport = Arc::new(BlockingRefresh::new());
    let first = {
        let coordinator = Arc::clone(&coordinator);
        let transport = Arc::clone(&transport);
        thread::spawn(move || coordinator.refresh_if_expired(&first_id, transport.as_ref(), 2_000))
    };
    transport.wait_for_calls(1)?;
    let second = {
        let coordinator = Arc::clone(&coordinator);
        let transport = Arc::clone(&transport);
        thread::spawn(move || coordinator.refresh_if_expired(&second_id, transport.as_ref(), 2_000))
    };
    transport.wait_for_calls(2)?;
    transport.release()?;
    assert_eq!(first.join().map_err(|_| "first panic")??.revision(), 1);
    assert_eq!(second.join().map_err(|_| "second panic")??.revision(), 1);
    Ok(())
}

#[test]
fn old_refresh_winner_cannot_overwrite_a_newer_cas_credential() -> TestResult {
    let credential_id = CredentialId::try_new("kiro-stale-owner")?;
    let coordinator = Arc::new(KiroCredentialRefreshCoordinator::try_new([(
        credential_id.clone(),
        expired_social()?,
    )])?);
    let transport = Arc::new(BlockingRefresh::new());
    let leader = {
        let coordinator = Arc::clone(&coordinator);
        let transport = Arc::clone(&transport);
        let credential_id = credential_id.clone();
        thread::spawn(move || {
            coordinator.refresh_if_expired(&credential_id, transport.as_ref(), 2_000)
        })
    };
    transport.wait_until_started()?;
    let replacement = KiroCredential::import_json(
        br#"{"kind":"social","access_token":"fixture-external-access","refresh_token":"fixture-external-refresh","expires_at_ms":5000}"#,
        2_000,
    )?;
    assert!(matches!(
        coordinator.compare_and_swap(&credential_id, 0, replacement)?,
        KiroCredentialCasOutcome::Committed(_)
    ));
    transport.release()?;
    assert!(matches!(
        leader.join().map_err(|_| "leader panic")?,
        Err(KiroCredentialError::ConcurrentCredentialStateChanged)
    ));
    let current = coordinator.load(&credential_id)?;
    assert_eq!(current.revision(), 1);
    assert_eq!(
        current.credential().access_token()?,
        "fixture-external-access"
    );
    Ok(())
}

#[derive(Default)]
struct FixtureRefresh {
    calls: Mutex<Vec<KiroRefreshKind>>,
}

impl std::fmt::Debug for FixtureRefresh {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FixtureRefresh")
            .field("requests", &"<redacted>")
            .finish()
    }
}

impl KiroRefreshTransport for FixtureRefresh {
    fn refresh(
        &self,
        request: KiroRefreshRequest,
    ) -> Result<KiroRefreshResponse, KiroCredentialError> {
        if request.kind() == KiroRefreshKind::Enterprise {
            assert_eq!(request.auth_region(), Some("us-west-2"));
        }
        self.calls
            .lock()
            .map_err(|_| KiroCredentialError::TransportUnavailable)?
            .push(request.kind());
        Ok(KiroRefreshResponse::new(br#"{"access_token":"fixture-new-access","refresh_token":"fixture-new-refresh","expires_in":3600,"token_type":"Bearer"}"#.to_vec()))
    }
}

struct BlockingRefresh {
    state: Mutex<(bool, bool, usize)>,
    changed: Condvar,
}

impl BlockingRefresh {
    fn new() -> Self {
        Self {
            state: Mutex::new((false, false, 0)),
            changed: Condvar::new(),
        }
    }
    fn wait_until_started(&self) -> TestResult {
        self.wait_for_calls(1)
    }
    fn wait_for_calls(&self, expected: usize) -> TestResult {
        let state = self.state.lock().map_err(|_| "poisoned")?;
        let (state, wait) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(2), |state| state.2 < expected)
            .map_err(|_| "poisoned")?;
        if wait.timed_out() || state.2 < expected {
            return Err("refresh did not start the expected calls".into());
        }
        Ok(())
    }
    fn release(&self) -> TestResult {
        let mut state = self.state.lock().map_err(|_| "poisoned")?;
        state.1 = true;
        self.changed.notify_all();
        Ok(())
    }
    fn call_count(&self) -> Result<usize, Box<dyn Error>> {
        Ok(self.state.lock().map_err(|_| "poisoned")?.2)
    }
}

impl KiroRefreshTransport for BlockingRefresh {
    fn refresh(
        &self,
        _request: KiroRefreshRequest,
    ) -> Result<KiroRefreshResponse, KiroCredentialError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| KiroCredentialError::TransportUnavailable)?;
        state.0 = true;
        state.2 += 1;
        self.changed.notify_all();
        let _state = self
            .changed
            .wait_while(state, |state| !state.1)
            .map_err(|_| KiroCredentialError::TransportUnavailable)?;
        Ok(KiroRefreshResponse::new(br#"{"access_token":"fixture-new-access","refresh_token":"fixture-new-refresh","expires_in":3600,"token_type":"Bearer"}"#.to_vec()))
    }
}

fn expired_social() -> Result<KiroCredential, KiroCredentialError> {
    KiroCredential::import_json(
        br#"{"kind":"social","access_token":"fixture-old-access","refresh_token":"fixture-old-refresh","expires_at_ms":1500}"#,
        1_000,
    )
}

fn secret_store() -> Result<SecretStore, Box<dyn Error>> {
    let version = KeyVersion::try_new(1)?;
    Ok(SecretStore::new(MasterKeyRing::try_new(
        version,
        [(version, MasterKey::try_from_bytes([0x57_u8; 32])?)],
    )?))
}
