//! P12-08D4 offline legacy CPA to CPAR three-protocol differential.

#![deny(unsafe_code)]

use differential_gate::{
    LegacyProtocolClassification, LegacyProtocolError, validate_legacy_protocol_corpus,
};

const CORPUS: &str = include_str!("../fixtures/cpa-three-protocol-golden-corpus.json");

#[test]
fn all_legacy_protocol_differences_are_closed_and_classified() -> Result<(), LegacyProtocolError> {
    let outcome = validate_legacy_protocol_corpus(CORPUS)?;
    assert_eq!(outcome.parity, 6);
    assert_eq!(outcome.intentional_hardening, 2);
    assert_eq!(outcome.unsupported_fail_closed, 2);
    assert_eq!(outcome.total(), 10);
    Ok(())
}

#[test]
fn stale_missing_or_relabelled_cases_fail_closed() {
    let stale = CORPUS.replacen(
        "\"expected_gateway_projection\": [\"response-start\", \"message-start\", \"text-delta\"",
        "\"expected_gateway_projection\": [\"response-start\", \"message-start\", \"response-end\"",
        1,
    );
    assert_eq!(
        validate_legacy_protocol_corpus(&stale),
        Err(LegacyProtocolError::GatewayProjectionMismatch)
    );

    let missing = CORPUS.replacen(
        "\"subject\": \"multiple-chat-choices\"",
        "\"subject\": \"reasoning-to-chat\"",
        1,
    );
    assert_eq!(
        validate_legacy_protocol_corpus(&missing),
        Err(LegacyProtocolError::IncompleteCorpus)
    );

    let relabelled = CORPUS.replacen(
        "\"classification\": \"INTENTIONAL_HARDENING\"",
        "\"classification\": \"PARITY\"",
        1,
    );
    assert_eq!(
        validate_legacy_protocol_corpus(&relabelled),
        Err(LegacyProtocolError::MisclassifiedDifference)
    );

    let hollowed = CORPUS.replacen(
        "\"reasoning-delta\", \"text-delta\", \"tool-call-start\"",
        "\"text-delta\", \"tool-call-start\"",
        2,
    );
    assert_eq!(
        validate_legacy_protocol_corpus(&hollowed),
        Err(LegacyProtocolError::MisclassifiedDifference)
    );
}

#[test]
fn unsafe_metadata_and_unknown_classifications_are_rejected_without_echoing_values() {
    let unsafe_shape = CORPUS.replacen(
        "\"cases\": [",
        "\"secret\": \"not-permitted\", \"cases\": [",
        1,
    );
    assert_eq!(
        validate_legacy_protocol_corpus(&unsafe_shape),
        Err(LegacyProtocolError::ForbiddenCorpusShape)
    );

    let unknown = CORPUS.replacen(
        "\"classification\": \"PARITY\"",
        "\"classification\": \"REGRESSION\"",
        1,
    );
    assert_eq!(
        validate_legacy_protocol_corpus(&unknown),
        Err(LegacyProtocolError::MalformedCorpus)
    );

    let closed_taxonomy = [
        LegacyProtocolClassification::Parity,
        LegacyProtocolClassification::IntentionalHardening,
        LegacyProtocolClassification::UnsupportedFailClosed,
    ];
    assert_eq!(closed_taxonomy.len(), 3);
}
