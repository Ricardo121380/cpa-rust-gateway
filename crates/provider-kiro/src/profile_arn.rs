//! Kiro `profileArn` lifecycle without ambient credential or network access.

use std::{error::Error, fmt};

use serde_json::Value;

use crate::{credential::KiroCredentialKind, endpoint_policy::KiroApiRegion};

const BUILDER_PROFILE_ARN: &str =
    "arn:aws:codewhisperer:us-east-1:638616132270:profile/AAAACCCCXXXX";
const FALLBACK_ACCOUNT_ID: &str = "610548660232";
const FALLBACK_PROFILE_ID: &str = "VNECVYCYYAWN";

/// Non-secret reason a request received (or omitted) a profile ARN.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroProfileArnSource {
    /// The frozen Builder default for the P7-01 Social/Builder credential family.
    BuilderDefault,
    /// A validated Enterprise lookup result.
    EnterpriseLookup,
    /// The deterministic Enterprise region-family fallback.
    EnterpriseFallback,
    /// API keys never carry a profile ARN.
    ApiKeyOmitted,
}

/// A validated Kiro `CodeWhisperer` profile ARN.
#[derive(Clone, Eq, PartialEq)]
pub struct KiroProfileArn(String);

impl KiroProfileArn {
    /// Validates the exact service, region, account, and profile shape accepted by this boundary.
    ///
    /// # Errors
    ///
    /// Returns a safe classification for a malformed or unsupported ARN.
    pub fn try_new(value: impl Into<String>) -> Result<Self, KiroProfileArnError> {
        let value = value.into();
        let parts: Vec<_> = value.split(':').collect();
        let valid = parts.len() == 6
            && parts[0] == "arn"
            && parts[1] == "aws"
            && parts[2] == "codewhisperer"
            && KiroApiRegion::try_new(parts[3].to_owned()).is_ok()
            && parts[4].len() == 12
            && parts[4].bytes().all(|value| value.is_ascii_digit())
            && parts[5].strip_prefix("profile/").is_some_and(|id| {
                !id.is_empty()
                    && id.len() <= 128
                    && id
                        .bytes()
                        .all(|value| value.is_ascii_uppercase() || value.is_ascii_digit())
            });
        if !valid {
            return Err(KiroProfileArnError::InvalidProfileArn);
        }
        Ok(Self(value))
    }
    /// Returns the validated ARN only for immediate request construction.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for KiroProfileArn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KiroProfileArn(<redacted>)")
    }
}

/// Caller-provided Enterprise profile query port; implementations own authenticated I/O later.
pub trait KiroEnterpriseProfileLookup {
    /// Returns one candidate ARN only; error/text is deliberately not retained by this boundary.
    ///
    /// # Errors
    ///
    /// Returns a classification-only lookup failure.
    fn lookup(&self, api_region: &KiroApiRegion) -> Result<String, KiroProfileArnError>;
}

/// A resolved profile value and safe provenance for one later Kiro request.
#[derive(Clone)]
pub struct KiroProfileArnResolution {
    arn: Option<KiroProfileArn>,
    source: KiroProfileArnSource,
}
impl KiroProfileArnResolution {
    /// Returns the resolved ARN, if this credential family requires one.
    #[must_use]
    pub fn arn(&self) -> Option<&KiroProfileArn> {
        self.arn.as_ref()
    }
    /// Returns the non-secret resolution provenance for an audit event.
    #[must_use]
    pub const fn source(&self) -> KiroProfileArnSource {
        self.source
    }

    /// Produces the secret-free provenance projection required for an audit sink.
    #[must_use]
    pub fn audit(&self, api_region: &KiroApiRegion) -> KiroProfileArnAudit {
        KiroProfileArnAudit {
            source: self.source,
            profile_present: self.arn.is_some(),
            api_region: api_region.as_str().to_owned(),
        }
    }
}
impl fmt::Debug for KiroProfileArnResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KiroProfileArnResolution")
            .field("present", &self.arn.is_some())
            .field("source", &self.source)
            .finish()
    }
}

/// Resolves profile behavior without leaking a query error or emitting an I/O side effect itself.
pub fn resolve_profile_arn<L: KiroEnterpriseProfileLookup>(
    kind: KiroCredentialKind,
    api_region: &KiroApiRegion,
    lookup: &L,
) -> KiroProfileArnResolution {
    match kind {
        KiroCredentialKind::ApiKey => KiroProfileArnResolution {
            arn: None,
            source: KiroProfileArnSource::ApiKeyOmitted,
        },
        KiroCredentialKind::Social => KiroProfileArnResolution {
            arn: Some(frozen_arn(BUILDER_PROFILE_ARN)),
            source: KiroProfileArnSource::BuilderDefault,
        },
        KiroCredentialKind::Enterprise => {
            match lookup.lookup(api_region).and_then(KiroProfileArn::try_new) {
                Ok(arn) => KiroProfileArnResolution {
                    arn: Some(arn),
                    source: KiroProfileArnSource::EnterpriseLookup,
                },
                Err(_) => KiroProfileArnResolution {
                    arn: Some(enterprise_fallback(api_region)),
                    source: KiroProfileArnSource::EnterpriseFallback,
                },
            }
        }
    }
}

/// A bounded, no-secret profile lifecycle audit projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KiroProfileArnAudit {
    source: KiroProfileArnSource,
    profile_present: bool,
    api_region: String,
}

impl KiroProfileArnAudit {
    /// Returns the resolution provenance without rendering the ARN.
    #[must_use]
    pub const fn source(&self) -> KiroProfileArnSource {
        self.source
    }
    /// Returns whether a profile was injected without rendering its value.
    #[must_use]
    pub const fn profile_present(&self) -> bool {
        self.profile_present
    }
    /// Returns the non-secret Region associated with the decision.
    #[must_use]
    pub fn api_region(&self) -> &str {
        &self.api_region
    }
}

/// Injects only a resolved value at the root of a validated JSON request object.
///
/// # Errors
///
/// Returns a safe classification when the request root is not an object.
pub fn inject_profile_arn(
    body: &mut Value,
    resolution: &KiroProfileArnResolution,
) -> Result<(), KiroProfileArnError> {
    let object = body
        .as_object_mut()
        .ok_or(KiroProfileArnError::RequestMustBeObject)?;
    if let Some(arn) = resolution.arn() {
        object.insert(
            "profileArn".to_owned(),
            Value::String(arn.as_str().to_owned()),
        );
    }
    Ok(())
}

fn enterprise_fallback(region: &KiroApiRegion) -> KiroProfileArn {
    let region = if region.as_str().starts_with("eu-") {
        "eu-central-1"
    } else {
        "us-east-1"
    };
    frozen_arn(&format!(
        "arn:aws:codewhisperer:{region}:{FALLBACK_ACCOUNT_ID}:profile/{FALLBACK_PROFILE_ID}"
    ))
}

fn frozen_arn(value: &str) -> KiroProfileArn {
    KiroProfileArn(value.to_owned())
}

/// Classification-only profile lifecycle error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroProfileArnError {
    /// The supplied ARN violates the fixed Kiro profile structure.
    InvalidProfileArn,
    /// Injection was requested for a non-object JSON root.
    RequestMustBeObject,
}
impl fmt::Display for KiroProfileArnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidProfileArn => "Kiro profile ARN is invalid",
            Self::RequestMustBeObject => "Kiro request body must be an object",
        })
    }
}
impl Error for KiroProfileArnError {}
