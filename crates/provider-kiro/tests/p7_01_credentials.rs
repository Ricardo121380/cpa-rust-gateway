//! P7-01 strict Kiro credential boundary regressions.

use std::{error::Error, sync::Mutex};

use gateway_core::CredentialId;
use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
use provider_kiro::credential::{
    KiroCredential, KiroCredentialError, KiroCredentialKind, KiroRefreshKind, KiroRefreshRequest,
    KiroRefreshResponse, KiroRefreshTransport,
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

fn secret_store() -> Result<SecretStore, Box<dyn Error>> {
    let version = KeyVersion::try_new(1)?;
    Ok(SecretStore::new(MasterKeyRing::try_new(
        version,
        [(version, MasterKey::try_from_bytes([0x57_u8; 32])?)],
    )?))
}
