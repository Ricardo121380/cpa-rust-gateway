//! Offline, clean-room differential fixture validation for P11.
//!
//! The harness deliberately handles only a small, closed semantic vocabulary. It is test-only:
//! it has no transport, filesystem traversal, environment lookup, reference-source reader, or
//! credential type. Every error is value-free so a malformed fixture cannot surface captured
//! request or response material through a test failure.

use std::{collections::BTreeSet, fmt};

use serde::Deserialize;

const FIXTURE_VERSION: u8 = 1;
const MAX_FIXTURE_BYTES: usize = 8 * 1024;
const MAX_PROJECTION_MARKERS: usize = 12;
const FORBIDDEN_FIELD_NAMES: &[&str] = &[
    "authorization",
    "body",
    "cookie",
    "database_row",
    "endpoint",
    "header",
    "oauth",
    "request",
    "response",
    "secret",
    "source",
    "token",
    "url",
];

/// One completed clean-room fixture validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FixtureOutcome {
    /// The only allowed non-regression classification for this fixture.
    pub(crate) classification: Classification,
}

/// Validates a single committed fixture without retaining or exposing its input values.
pub(crate) fn validate_fixture(input: &str) -> Result<FixtureOutcome, FixtureError> {
    reject_unsafe_shape(input)?;
    let fixture: DifferentialFixture =
        serde_json::from_str(input).map_err(|_| FixtureError::MalformedFixture)?;
    fixture.validate()?;
    fixture.classify()
}

fn reject_unsafe_shape(input: &str) -> Result<(), FixtureError> {
    if input.len() > MAX_FIXTURE_BYTES {
        return Err(FixtureError::FixtureTooLarge);
    }

    let lower = input.to_ascii_lowercase();
    if FORBIDDEN_FIELD_NAMES
        .iter()
        .any(|field| lower.contains(&format!(r#""{field}""#)))
    {
        return Err(FixtureError::ForbiddenFixtureShape);
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DifferentialFixture {
    fixture_version: u8,
    id: String,
    reference: Reference,
    subject: Subject,
    reference_projection: Vec<ProjectionMarker>,
    gateway_projection: Vec<ProjectionMarker>,
    classification: Classification,
    decision: Option<Decision>,
}

impl DifferentialFixture {
    fn validate(&self) -> Result<(), FixtureError> {
        if self.fixture_version != FIXTURE_VERSION
            || !valid_fixture_id(&self.id)
            || !self.reference.allows(self.subject)
            || !valid_projection(self.subject, &self.reference_projection)
            || !valid_projection(self.subject, &self.gateway_projection)
        {
            return Err(FixtureError::InvalidFixture);
        }
        Ok(())
    }

    fn classify(&self) -> Result<FixtureOutcome, FixtureError> {
        let equivalent = self.reference_projection == self.gateway_projection;
        match self.classification {
            Classification::Compatible if equivalent && self.decision.is_none() => {
                Ok(FixtureOutcome {
                    classification: Classification::Compatible,
                })
            }
            Classification::Intentional if !equivalent && self.decision.is_some() => {
                Ok(FixtureOutcome {
                    classification: Classification::Intentional,
                })
            }
            Classification::Regression => Err(FixtureError::Regression),
            Classification::Compatible | Classification::Intentional => {
                Err(FixtureError::MisclassifiedFixture)
            }
        }
    }
}

fn valid_fixture_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_projection(subject: Subject, projection: &[ProjectionMarker]) -> bool {
    !projection.is_empty()
        && projection.len() <= MAX_PROJECTION_MARKERS
        && projection.iter().collect::<BTreeSet<_>>().len() == projection.len()
        && projection.iter().all(|marker| subject.allows(*marker))
}

/// Frozen references allowed in the P11 corpus.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Reference {
    #[serde(rename = "cpa-v7.2.80")]
    CpaV7_2_80,
    #[serde(rename = "grok2api-v3.0.0-ec6cddca7")]
    Grok2ApiV3,
    #[serde(rename = "kiro-rs-c49c75e")]
    KiroRs,
}

impl Reference {
    fn allows(self, subject: Subject) -> bool {
        matches!(
            (self, subject),
            (
                Self::CpaV7_2_80,
                Subject::CanonicalLifecycle | Subject::ConfigurationAuthority
            ) | (
                Self::Grok2ApiV3,
                Subject::ProviderPoolIsolation | Subject::WebToolDefault
            ) | (
                Self::KiroRs,
                Subject::EndpointPolicy | Subject::EventStreamIntegrity
            )
        )
    }
}

/// Semantic property families with no body-bearing representation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Subject {
    CanonicalLifecycle,
    ConfigurationAuthority,
    ProviderPoolIsolation,
    WebToolDefault,
    EndpointPolicy,
    EventStreamIntegrity,
}

impl Subject {
    fn allows(self, marker: ProjectionMarker) -> bool {
        matches!(
            (self, marker),
            (
                Self::CanonicalLifecycle,
                ProjectionMarker::ResponseStart
                    | ProjectionMarker::TextDelta
                    | ProjectionMarker::ResponseEnd
            ) | (
                Self::ConfigurationAuthority,
                ProjectionMarker::FileWatcherAuthority | ProjectionMarker::VersionedSqliteSnapshot
            ) | (
                Self::ProviderPoolIsolation,
                ProjectionMarker::BuildWebPoolSeparation
                    | ProjectionMarker::BrowserEgressBoundConversation
            ) | (
                Self::WebToolDefault,
                ProjectionMarker::ToolEmulationDefaultEnabled
                    | ProjectionMarker::ToolEmulationDefaultDisabled
            ) | (Self::EndpointPolicy, ProjectionMarker::CliIdeEndpointPolicy)
                | (
                    Self::EventStreamIntegrity,
                    ProjectionMarker::EventStreamCrcValidation
                        | ProjectionMarker::ChunkInvariantCanonicalEvents
                )
        )
    }
}

/// Closed, value-free markers that may appear in a semantic projection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum ProjectionMarker {
    ResponseStart,
    TextDelta,
    ResponseEnd,
    FileWatcherAuthority,
    VersionedSqliteSnapshot,
    BuildWebPoolSeparation,
    BrowserEgressBoundConversation,
    ToolEmulationDefaultEnabled,
    ToolEmulationDefaultDisabled,
    CliIdeEndpointPolicy,
    EventStreamCrcValidation,
    ChunkInvariantCanonicalEvents,
}

/// The complete taxonomy accepted by P11's fixture gate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Classification {
    /// The two source-labelled semantic projections are equal.
    Compatible,
    /// A reviewed and documented behavior change is permitted.
    Intentional,
    /// A non-approved mismatch; this is always rejected by the corpus gate.
    Regression,
}

/// Frozen decisions that can justify an intentional difference.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Decision {
    Bl09VersionedControlPlane,
    Bl20WebToolDefaultOff,
}

/// A value-free reason that a fixture cannot be accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FixtureError {
    FixtureTooLarge,
    ForbiddenFixtureShape,
    MalformedFixture,
    InvalidFixture,
    MisclassifiedFixture,
    Regression,
}

impl fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::FixtureTooLarge => "fixture_too_large",
            Self::ForbiddenFixtureShape => "forbidden_fixture_shape",
            Self::MalformedFixture => "malformed_fixture",
            Self::InvalidFixture => "invalid_fixture",
            Self::MisclassifiedFixture => "misclassified_fixture",
            Self::Regression => "regression",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for FixtureError {}
