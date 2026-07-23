//! P11's clean-room, source-labelled differential fixture gate.

#![deny(unsafe_code)]

#[path = "../../../tests/differential/harness.rs"]
mod harness;

use harness::{Classification, FixtureError, validate_fixture};

const CPA_CANONICAL_LIFECYCLE: &str =
    include_str!("../../../tests/differential/fixtures/cpa-canonical-lifecycle.json");
const CPA_CONFIGURATION_AUTHORITY: &str =
    include_str!("../../../tests/differential/fixtures/cpa-configuration-authority.json");
const GROK2API_PROVIDER_POOL_ISOLATION: &str =
    include_str!("../../../tests/differential/fixtures/grok2api-provider-pool-isolation.json");
const GROK2API_WEB_TOOL_DEFAULT: &str =
    include_str!("../../../tests/differential/fixtures/grok2api-web-tool-default.json");
const KIRO_RS_ENDPOINT_POLICY: &str =
    include_str!("../../../tests/differential/fixtures/kiro-rs-endpoint-policy.json");
const KIRO_RS_EVENT_STREAM_INTEGRITY: &str =
    include_str!("../../../tests/differential/fixtures/kiro-rs-event-stream-integrity.json");

#[test]
fn committed_corpus_has_only_approved_value_free_classifications() -> Result<(), FixtureError> {
    let fixtures = [
        (CPA_CANONICAL_LIFECYCLE, Classification::Compatible),
        (CPA_CONFIGURATION_AUTHORITY, Classification::Intentional),
        (GROK2API_PROVIDER_POOL_ISOLATION, Classification::Compatible),
        (GROK2API_WEB_TOOL_DEFAULT, Classification::Intentional),
        (KIRO_RS_ENDPOINT_POLICY, Classification::Compatible),
        (KIRO_RS_EVENT_STREAM_INTEGRITY, Classification::Compatible),
    ];

    for (fixture, expected) in fixtures {
        let outcome = validate_fixture(fixture)?;
        assert_eq!(outcome.classification, expected);
    }
    Ok(())
}

#[test]
fn missing_or_unapproved_classification_fails_closed_without_echoing_values() {
    let missing = r#"{
        "fixture_version": 1,
        "id": "cpa-canonical-lifecycle",
        "reference": "cpa-v7.2.80",
        "subject": "canonical-lifecycle",
        "reference_projection": ["response-start"],
        "gateway_projection": ["response-start"]
    }"#;
    assert_eq!(
        validate_fixture(missing),
        Err(FixtureError::MalformedFixture)
    );

    let regression = r#"{
        "fixture_version": 1,
        "id": "cpa-canonical-lifecycle",
        "reference": "cpa-v7.2.80",
        "subject": "canonical-lifecycle",
        "reference_projection": ["response-start"],
        "gateway_projection": ["response-end"],
        "classification": "regression"
    }"#;
    assert_eq!(validate_fixture(regression), Err(FixtureError::Regression));
}

#[test]
fn body_like_fields_unknown_markers_and_reference_subject_mismatches_are_rejected() {
    let body_like = r#"{
        "fixture_version": 1,
        "id": "cpa-canonical-lifecycle",
        "reference": "cpa-v7.2.80",
        "subject": "canonical-lifecycle",
        "reference_projection": ["response-start"],
        "gateway_projection": ["response-start"],
        "classification": "compatible",
        "body": "not permitted"
    }"#;
    assert_eq!(
        validate_fixture(body_like),
        Err(FixtureError::ForbiddenFixtureShape)
    );

    let unknown_marker = r#"{
        "fixture_version": 1,
        "id": "cpa-canonical-lifecycle",
        "reference": "cpa-v7.2.80",
        "subject": "canonical-lifecycle",
        "reference_projection": ["response-start"],
        "gateway_projection": ["unapproved-marker"],
        "classification": "compatible"
    }"#;
    assert_eq!(
        validate_fixture(unknown_marker),
        Err(FixtureError::MalformedFixture)
    );

    let source_mismatch = r#"{
        "fixture_version": 1,
        "id": "cpa-canonical-lifecycle",
        "reference": "kiro-rs-c49c75e",
        "subject": "canonical-lifecycle",
        "reference_projection": ["response-start"],
        "gateway_projection": ["response-start"],
        "classification": "compatible"
    }"#;
    assert_eq!(
        validate_fixture(source_mismatch),
        Err(FixtureError::InvalidFixture)
    );

    let subject_marker_mismatch = r#"{
        "fixture_version": 1,
        "id": "cpa-canonical-lifecycle",
        "reference": "cpa-v7.2.80",
        "subject": "canonical-lifecycle",
        "reference_projection": ["build-web-pool-separation"],
        "gateway_projection": ["build-web-pool-separation"],
        "classification": "compatible"
    }"#;
    assert_eq!(
        validate_fixture(subject_marker_mismatch),
        Err(FixtureError::InvalidFixture)
    );
}
