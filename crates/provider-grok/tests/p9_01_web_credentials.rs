//! P9-01 synthetic Grok Web SSO credential and lineage evidence.

use std::error::Error;

use gateway_store::secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore};
use provider_grok::{
    GROK_WEB_PROVIDER_ID, GrokWebCredential, GrokWebCredentialCasOutcome, GrokWebCredentialError,
    GrokWebCredentialSlot, GrokWebCredentialSource,
};

const OBSERVED_AT_MS: i64 = 1_000_000;

#[test]
fn strict_sso_import_retains_only_validated_scopes_and_redacts_cookie_values()
-> Result<(), Box<dyn Error>> {
    let credential = import(&export(
        "web_account_01",
        "sso_import_01",
        7,
        "session_value_alpha",
    ))?;

    assert_eq!(GrokWebCredential::provider_id(), GROK_WEB_PROVIDER_ID);
    assert_eq!(credential.account_reference(), "web_account_01");
    assert_eq!(
        credential.lineage().source(),
        GrokWebCredentialSource::ImportedSso
    );
    assert_eq!(credential.lineage().reference(), "sso_import_01");
    assert_eq!(credential.revision(), 7);
    assert_eq!(credential.expires_at_ms(), 2_000_000);
    assert!(!credential.is_expired_at(1_999_999));
    assert!(credential.is_expired_at(2_000_000));
    assert_eq!(credential.cookies().len(), 2);
    assert_eq!(credential.cookies()[0].name(), "sso_session");
    assert_eq!(credential.cookies()[0].value(), "session_value_alpha");
    assert_eq!(credential.cookies()[0].domain(), "grok.example.test");
    assert_eq!(credential.cookies()[0].path(), "/");
    assert!(credential.cookies()[0].secure());
    assert!(credential.cookies()[0].http_only());

    let debug = format!("{credential:?}");
    let cookie_debug = format!("{:?}", credential.cookies()[0]);
    assert!(!debug.contains("session_value_alpha"));
    assert!(!cookie_debug.contains("session_value_alpha"));
    assert!(cookie_debug.contains("<redacted>"));
    Ok(())
}

#[test]
fn sealed_sso_envelope_is_aead_protected_and_expiry_checked() -> Result<(), Box<dyn Error>> {
    let credential = import(&export(
        "web_account_01",
        "sso_import_01",
        7,
        "sealed_session_value_alpha",
    ))?;
    let store = secret_store(0x51)?;
    let envelope = credential.seal(&store)?;
    let encrypted_debug = format!("{envelope:?}");
    assert!(!encrypted_debug.contains("sealed_session_value_alpha"));
    assert!(encrypted_debug.contains("<redacted>"));
    assert!(
        !envelope
            .ciphertext()
            .windows("sealed_session_value_alpha".len())
            .any(|window| window == b"sealed_session_value_alpha"),
        "AEAD ciphertext retained synthetic Cookie plaintext"
    );

    let recovered = envelope.open(&store, OBSERVED_AT_MS)?;
    assert_eq!(recovered, credential);
    assert_eq!(
        envelope.open(&secret_store(0x52)?, OBSERVED_AT_MS),
        Err(GrokWebCredentialError::SecretStoreFailure)
    );
    assert_eq!(
        envelope.open(&store, 2_000_000),
        Err(GrokWebCredentialError::InvalidPersistedCredential)
    );
    Ok(())
}

#[test]
fn malformed_expired_or_ambiguous_sso_exports_fail_closed() {
    let duplicate_root = br#"{
        "kind":"grok_web_sso",
        "account_ref":"web_account_01",
        "account_ref":"web_account_02",
        "lineage_ref":"sso_import_01",
        "revision":0,
        "expires_at_ms":2000000,
        "cookies":[]
    }"#;
    assert_eq!(
        GrokWebCredential::import_sso_json(duplicate_root, OBSERVED_AT_MS),
        Err(GrokWebCredentialError::InvalidJson)
    );

    let duplicate_cookie_scope = br#"{
        "kind":"grok_web_sso",
        "account_ref":"web_account_01",
        "lineage_ref":"sso_import_01",
        "revision":0,
        "expires_at_ms":2000000,
        "cookies":[
            {"name":"sso_session","value":"session_one","domain":"grok.example.test","path":"/","secure":true,"http_only":true},
            {"name":"sso_session","value":"session_two","domain":".grok.example.test","path":"/","secure":true,"http_only":true}
        ]
    }"#;
    assert_eq!(
        GrokWebCredential::import_sso_json(duplicate_cookie_scope, OBSERVED_AT_MS),
        Err(GrokWebCredentialError::DuplicateCookieScope)
    );

    let unsafe_cookie = export("web_account_01", "sso_import_01", 0, "session;unsafe");
    assert_eq!(
        import(&unsafe_cookie),
        Err(GrokWebCredentialError::InvalidField)
    );

    let expired = export_with_expiry(
        "web_account_01",
        "sso_import_01",
        0,
        "session_value",
        OBSERVED_AT_MS,
    );
    assert_eq!(
        import(&expired),
        Err(GrokWebCredentialError::InvalidTimestamp)
    );
}

#[test]
fn revision_cas_keeps_web_account_and_lineage_isolated() -> Result<(), Box<dyn Error>> {
    let initial = import(&export(
        "web_account_01",
        "sso_import_01",
        0,
        "session_initial",
    ))?;
    let slot = GrokWebCredentialSlot::new(initial);
    let replacement = import(&export(
        "web_account_01",
        "sso_import_01",
        1,
        "session_replaced",
    ))?;
    assert_eq!(
        slot.compare_and_replace(0, replacement)?,
        GrokWebCredentialCasOutcome::Replaced
    );
    assert_eq!(slot.load()?.revision(), 1);
    assert_eq!(
        slot.compare_and_replace(
            0,
            import(&export(
                "web_account_01",
                "sso_import_01",
                1,
                "session_stale"
            ))?
        )?,
        GrokWebCredentialCasOutcome::Conflict
    );

    let foreign_lineage = import(&export(
        "web_account_01",
        "sso_import_02",
        2,
        "session_foreign",
    ))?;
    assert_eq!(
        slot.compare_and_replace(1, foreign_lineage),
        Err(GrokWebCredentialError::LineageMismatch)
    );
    let retained = slot.load()?;
    assert_eq!(retained.revision(), 1);
    assert_eq!(retained.lineage().reference(), "sso_import_01");
    assert_eq!(retained.cookies()[0].value(), "session_replaced");
    Ok(())
}

fn import(input: &str) -> Result<GrokWebCredential, GrokWebCredentialError> {
    GrokWebCredential::import_sso_json(input.as_bytes(), OBSERVED_AT_MS)
}

fn export(account_ref: &str, lineage_ref: &str, revision: u64, session_value: &str) -> String {
    export_with_expiry(account_ref, lineage_ref, revision, session_value, 2_000_000)
}

fn export_with_expiry(
    account_ref: &str,
    lineage_ref: &str,
    revision: u64,
    session_value: &str,
    expires_at_ms: i64,
) -> String {
    format!(
        r#"{{
            "kind":"grok_web_sso",
            "account_ref":"{account_ref}",
            "lineage_ref":"{lineage_ref}",
            "revision":{revision},
            "expires_at_ms":{expires_at_ms},
            "cookies":[
                {{"name":"sso_session","value":"{session_value}","domain":".grok.example.test","path":"/","secure":true,"http_only":true}},
                {{"name":"csrf","value":"csrf_value","domain":"grok.example.test","path":"/chat","secure":true,"http_only":false}}
            ]
        }}"#
    )
}

fn secret_store(key_byte: u8) -> Result<SecretStore, Box<dyn Error>> {
    let key_version = KeyVersion::try_new(1)?;
    Ok(SecretStore::new(MasterKeyRing::try_new(
        key_version,
        [(key_version, MasterKey::try_from_bytes([key_byte; 32])?)],
    )?))
}
