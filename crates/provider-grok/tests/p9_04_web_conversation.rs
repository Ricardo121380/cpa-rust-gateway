//! P9-04 local Grok Web Conversation/account/egress binding evidence.

#![deny(unsafe_code)]

use std::error::Error;

use gateway_upstream::UpstreamProxy;
use provider_grok::{
    GrokWebBrowserEgressSession, GrokWebBrowserUserAgent, GrokWebConversationAvailability,
    GrokWebConversationError, GrokWebConversationId, GrokWebConversationState, GrokWebCredential,
    GrokWebEgressSessionId, GrokWebParentMessageId, GrokWebTlsProfile,
};

type TestResult = Result<(), Box<dyn Error>>;

const OBSERVED_AT_MS: i64 = 1_000_000;
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/537.36";

#[test]
fn conversation_turns_keep_one_exact_account_lineage_revision_and_egress_binding() -> TestResult {
    let current = session(
        "web_account_01",
        "sso_import_01",
        7,
        "web_egress_01",
        1_500_000,
    )?;
    let mut conversation = GrokWebConversationState::try_new(
        GrokWebConversationId::try_new("conversation_01")?,
        &current,
        OBSERVED_AT_MS,
    )?;

    let initial = conversation.prepare_turn(&current, OBSERVED_AT_MS)?;
    assert_eq!(initial.conversation_id().as_str(), "conversation_01");
    assert!(initial.parent_message_id().is_none());
    conversation.record_parent_message(
        &current,
        GrokWebParentMessageId::try_new("message_01")?,
        OBSERVED_AT_MS,
    )?;
    let continuation = conversation.prepare_turn(&current, OBSERVED_AT_MS)?;
    assert_eq!(
        continuation
            .parent_message_id()
            .map(GrokWebParentMessageId::as_str),
        Some("message_01")
    );

    for (other, expected) in [
        (
            session(
                "web_account_02",
                "sso_import_01",
                7,
                "web_egress_01",
                1_500_000,
            )?,
            GrokWebConversationError::AccountMismatch,
        ),
        (
            session(
                "web_account_01",
                "sso_import_02",
                7,
                "web_egress_01",
                1_500_000,
            )?,
            GrokWebConversationError::LineageMismatch,
        ),
        (
            session(
                "web_account_01",
                "sso_import_01",
                8,
                "web_egress_01",
                1_500_000,
            )?,
            GrokWebConversationError::CredentialRevisionMismatch,
        ),
        (
            session(
                "web_account_01",
                "sso_import_01",
                7,
                "web_egress_02",
                1_500_000,
            )?,
            GrokWebConversationError::EgressSessionMismatch,
        ),
        (
            session(
                "web_account_01",
                "sso_import_01",
                7,
                "web_egress_01",
                1_400_000,
            )?,
            GrokWebConversationError::CredentialExpiryMismatch,
        ),
    ] {
        assert_eq!(
            conversation.prepare_turn(&other, OBSERVED_AT_MS),
            Err(expected)
        );
    }
    Ok(())
}

#[test]
fn parent_update_expiry_and_account_unavailability_fail_closed_without_cross_session_mutation()
-> TestResult {
    let current = session(
        "web_account_01",
        "sso_import_01",
        7,
        "web_egress_01",
        1_500_000,
    )?;
    let mut conversation = GrokWebConversationState::try_new(
        GrokWebConversationId::try_new("conversation_01")?,
        &current,
        OBSERVED_AT_MS,
    )?;
    conversation.record_parent_message(
        &current,
        GrokWebParentMessageId::try_new("message_01")?,
        OBSERVED_AT_MS,
    )?;
    assert_eq!(
        conversation.record_parent_message(
            &current,
            GrokWebParentMessageId::try_new("message_01")?,
            OBSERVED_AT_MS,
        ),
        Err(GrokWebConversationError::DuplicateParentMessageId)
    );
    let different = session(
        "web_account_02",
        "sso_import_01",
        7,
        "web_egress_01",
        1_500_000,
    )?;
    assert_eq!(
        conversation.mark_account_unavailable(&different, OBSERVED_AT_MS),
        Err(GrokWebConversationError::AccountMismatch)
    );
    assert_eq!(
        conversation.availability(),
        GrokWebConversationAvailability::Available
    );
    conversation.mark_account_unavailable(&current, OBSERVED_AT_MS)?;
    assert_eq!(
        conversation.availability(),
        GrokWebConversationAvailability::AccountUnavailable
    );
    assert_eq!(
        conversation.prepare_turn(&current, OBSERVED_AT_MS),
        Err(GrokWebConversationError::AccountUnavailable)
    );

    let expired = session(
        "web_account_01",
        "sso_import_01",
        7,
        "web_egress_01",
        1_000_001,
    )?;
    assert_eq!(
        GrokWebConversationState::try_new(
            GrokWebConversationId::try_new("conversation_expired")?,
            &expired,
            1_000_001,
        ),
        Err(GrokWebConversationError::ExpiredEgressSession)
    );
    Ok(())
}

#[test]
fn conversation_identifiers_and_debug_forms_are_bounded_and_redacted() -> TestResult {
    assert_eq!(
        GrokWebConversationId::try_new(""),
        Err(GrokWebConversationError::InvalidConversationId)
    );
    assert_eq!(
        GrokWebParentMessageId::try_new("bad\r\nnext"),
        Err(GrokWebConversationError::InvalidParentMessageId)
    );
    let current = session(
        "web_account_secret",
        "sso_lineage_secret",
        7,
        "web_egress_secret",
        1_500_000,
    )?;
    let conversation = GrokWebConversationState::try_new(
        GrokWebConversationId::try_new("conversation_secret")?,
        &current,
        OBSERVED_AT_MS,
    )?;
    let diagnostic = format!("{conversation:?}");
    for private_value in [
        "conversation_secret",
        "web_account_secret",
        "sso_lineage_secret",
        "web_egress_secret",
    ] {
        assert!(!diagnostic.contains(private_value));
    }
    assert!(diagnostic.contains("<redacted>"));
    Ok(())
}

fn session(
    account: &str,
    lineage: &str,
    revision: u64,
    egress_session_id: &str,
    expires_at_ms: i64,
) -> Result<GrokWebBrowserEgressSession, Box<dyn Error>> {
    let credential = GrokWebCredential::import_sso_json(
        format!(
            r#"{{
                "kind":"grok_web_sso",
                "account_ref":"{account}",
                "lineage_ref":"{lineage}",
                "revision":{revision},
                "expires_at_ms":{expires_at_ms},
                "cookies":[{{"name":"sso_session","value":"session_value","domain":".grok.example.test","path":"/","secure":true,"http_only":true}}]
            }}"#
        )
        .as_bytes(),
        OBSERVED_AT_MS,
    )?;
    Ok(GrokWebBrowserEgressSession::try_new(
        GrokWebEgressSessionId::try_new(egress_session_id)?,
        credential,
        GrokWebBrowserUserAgent::try_new(USER_AGENT)?,
        GrokWebTlsProfile::try_new("chrome_136_macos")?,
        UpstreamProxy::Direct,
        OBSERVED_AT_MS,
    )?)
}
