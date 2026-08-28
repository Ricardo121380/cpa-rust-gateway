//! P11's clean-room differential gate: recorded reference vs. executed gateway projections.

#![deny(unsafe_code)]

use differential_gate::{
    Classification, FixtureError, ProbeError, ProjectionMarker, Subject, validate_fixture,
};

const CPA_CANONICAL_LIFECYCLE: &str = include_str!("../fixtures/cpa-canonical-lifecycle.json");
const CPA_CONFIGURATION_AUTHORITY: &str =
    include_str!("../fixtures/cpa-configuration-authority.json");
const GROK2API_PROVIDER_POOL_ISOLATION: &str =
    include_str!("../fixtures/grok2api-provider-pool-isolation.json");
const GROK2API_WEB_TOOL_DEFAULT: &str = include_str!("../fixtures/grok2api-web-tool-default.json");
const KIRO_RS_ENDPOINT_POLICY: &str = include_str!("../fixtures/kiro-rs-endpoint-policy.json");
const KIRO_RS_EVENT_STREAM_INTEGRITY: &str =
    include_str!("../fixtures/kiro-rs-event-stream-integrity.json");

#[test]
fn committed_corpus_matches_the_executed_gateway_projection() -> Result<(), FixtureError> {
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
        assert!(!outcome.observed_gateway_projection.is_empty());
    }
    Ok(())
}

#[test]
fn every_subject_computes_a_gateway_projection_from_real_code() -> Result<(), ProbeError> {
    let subjects = [
        (
            Subject::CanonicalLifecycle,
            vec![
                ProjectionMarker::ResponseStart,
                ProjectionMarker::TextDelta,
                ProjectionMarker::ResponseEnd,
            ],
        ),
        (
            Subject::ConfigurationAuthority,
            vec![ProjectionMarker::VersionedSqliteSnapshot],
        ),
        (
            Subject::ProviderPoolIsolation,
            vec![
                ProjectionMarker::BuildWebPoolSeparation,
                ProjectionMarker::BrowserEgressBoundConversation,
            ],
        ),
        (
            Subject::WebToolDefault,
            vec![ProjectionMarker::ToolEmulationDefaultDisabled],
        ),
        (
            Subject::EndpointPolicy,
            vec![ProjectionMarker::CliIdeEndpointPolicy],
        ),
        (
            Subject::EventStreamIntegrity,
            vec![
                ProjectionMarker::EventStreamCrcValidation,
                ProjectionMarker::ChunkInvariantCanonicalEvents,
            ],
        ),
    ];

    for (subject, expected) in subjects {
        assert_eq!(differential_gate::observe(subject)?, expected);
    }
    Ok(())
}

#[test]
fn a_gateway_side_that_cannot_be_computed_is_never_compatible() {
    let reference_only = r#"{
        "fixture_version": 1,
        "id": "cpa-configuration-authority",
        "reference": "cpa-v7.2.80",
        "subject": "configuration-authority",
        "reference_projection": ["file-watcher-authority"],
        "expected_gateway_projection": ["file-watcher-authority"],
        "classification": "compatible"
    }"#;
    assert_eq!(
        validate_fixture(reference_only),
        Err(FixtureError::UnobservableGatewayMarker)
    );
}

#[test]
fn a_stale_expected_gateway_projection_fails_instead_of_passing() {
    let drifted = r#"{
        "fixture_version": 1,
        "id": "grok2api-web-tool-default",
        "reference": "grok2api-v3.0.0-ec6cddca7",
        "subject": "web-tool-default",
        "reference_projection": ["tool-emulation-default-enabled"],
        "expected_gateway_projection": ["tool-emulation-default-enabled"],
        "classification": "compatible"
    }"#;
    assert_eq!(
        validate_fixture(drifted),
        Err(FixtureError::GatewayProjectionMismatch)
    );
}

#[test]
fn missing_or_unapproved_classification_fails_closed_without_echoing_values() {
    let missing = r#"{
        "fixture_version": 1,
        "id": "cpa-canonical-lifecycle",
        "reference": "cpa-v7.2.80",
        "subject": "canonical-lifecycle",
        "reference_projection": ["response-start"],
        "expected_gateway_projection": ["response-start"]
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
        "expected_gateway_projection": ["response-end"],
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
        "expected_gateway_projection": ["response-start"],
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
        "expected_gateway_projection": ["unapproved-marker"],
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
        "expected_gateway_projection": ["response-start"],
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
        "expected_gateway_projection": ["build-web-pool-separation"],
        "classification": "compatible"
    }"#;
    assert_eq!(
        validate_fixture(subject_marker_mismatch),
        Err(FixtureError::InvalidFixture)
    );

    let unordered = r#"{
        "fixture_version": 1,
        "id": "cpa-canonical-lifecycle",
        "reference": "cpa-v7.2.80",
        "subject": "canonical-lifecycle",
        "reference_projection": ["text-delta", "response-start"],
        "expected_gateway_projection": ["response-start", "text-delta"],
        "classification": "compatible"
    }"#;
    assert_eq!(
        validate_fixture(unordered),
        Err(FixtureError::InvalidFixture)
    );
}
