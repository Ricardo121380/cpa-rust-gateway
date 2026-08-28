//! Fixture parsing, validation, and differential classification.

use std::fmt;

use serde::Deserialize;

use crate::{
    probe,
    vocabulary::{GatewayObservability, ProjectionMarker, Reference, Subject},
};

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

/// One completed differential fixture evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureOutcome {
    /// The only allowed non-regression classification for this fixture.
    pub classification: Classification,
    /// The projection actually computed by driving this repository's code.
    pub observed_gateway_projection: Vec<ProjectionMarker>,
}

/// Validates one committed fixture against a freshly computed gateway projection.
///
/// # Errors
///
/// Returns a value-free [`FixtureError`] for an unsafe, malformed, misclassified, or drifted
/// fixture, and for a gateway side that cannot be computed at all.
pub fn validate_fixture(input: &str) -> Result<FixtureOutcome, FixtureError> {
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
    expected_gateway_projection: Vec<ProjectionMarker>,
    classification: Classification,
    decision: Option<Decision>,
}

impl DifferentialFixture {
    fn validate(&self) -> Result<(), FixtureError> {
        if self.fixture_version != FIXTURE_VERSION
            || !valid_fixture_id(&self.id)
            || !self.reference.allows(self.subject)
            || !valid_projection(self.subject, &self.reference_projection)
            || !valid_projection(self.subject, &self.expected_gateway_projection)
        {
            return Err(FixtureError::InvalidFixture);
        }
        if self
            .expected_gateway_projection
            .iter()
            .any(|marker| marker.gateway_observability() == GatewayObservability::ReferenceOnly)
        {
            return Err(FixtureError::UnobservableGatewayMarker);
        }
        Ok(())
    }

    fn classify(&self) -> Result<FixtureOutcome, FixtureError> {
        if self.classification == Classification::Regression {
            return Err(FixtureError::Regression);
        }
        let observed =
            probe::observe(self.subject).map_err(|_| FixtureError::GatewayProbeUnavailable)?;
        if !valid_projection(self.subject, &observed) {
            return Err(FixtureError::GatewayProbeUnavailable);
        }
        if observed != self.expected_gateway_projection {
            return Err(FixtureError::GatewayProjectionMismatch);
        }

        let equivalent = self.reference_projection == observed;
        let classification = match self.classification {
            Classification::Compatible if equivalent && self.decision.is_none() => {
                Classification::Compatible
            }
            Classification::Intentional if !equivalent && self.decision.is_some() => {
                Classification::Intentional
            }
            Classification::Compatible
            | Classification::Intentional
            | Classification::Regression => {
                return Err(FixtureError::MisclassifiedFixture);
            }
        };
        Ok(FixtureOutcome {
            classification,
            observed_gateway_projection: observed,
        })
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
        && projection.windows(2).all(|pair| pair[0] < pair[1])
        && projection.iter().all(|marker| subject.allows(*marker))
}

/// The complete taxonomy accepted by P11's fixture gate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Classification {
    /// The recorded reference projection equals the computed gateway projection.
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
pub enum FixtureError {
    /// The committed fixture exceeded the fixed corpus size bound.
    FixtureTooLarge,
    /// The fixture used a body-bearing field name.
    ForbiddenFixtureShape,
    /// The fixture is not parseable under the closed vocabulary.
    MalformedFixture,
    /// The fixture parsed but violates a source/subject/marker or ordering rule.
    InvalidFixture,
    /// The fixture expects the gateway to emit a marker no execution can produce.
    UnobservableGatewayMarker,
    /// The gateway projection could not be computed from this repository's code.
    GatewayProbeUnavailable,
    /// The computed gateway projection differs from the projection the fixture expects.
    GatewayProjectionMismatch,
    /// Equality was declared intentional, or a difference was declared compatible.
    MisclassifiedFixture,
    /// The fixture is an accepted regression; the corpus gate always rejects it.
    Regression,
}

impl fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::FixtureTooLarge => "fixture_too_large",
            Self::ForbiddenFixtureShape => "forbidden_fixture_shape",
            Self::MalformedFixture => "malformed_fixture",
            Self::InvalidFixture => "invalid_fixture",
            Self::UnobservableGatewayMarker => "unobservable_gateway_marker",
            Self::GatewayProbeUnavailable => "gateway_probe_unavailable",
            Self::GatewayProjectionMismatch => "gateway_projection_mismatch",
            Self::MisclassifiedFixture => "misclassified_fixture",
            Self::Regression => "regression",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for FixtureError {}
