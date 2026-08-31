//! Provider/channel-scoped account entitlement observations.

use std::{error::Error, fmt};

/// Closed namespace for one account entitlement vocabulary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderAccountEntitlementDomain {
    /// Grok Build OAuth subscription.
    GrokBuild,
    /// Grok Web browser subscription.
    GrokWeb,
    /// ChatGPT/Codex account plan.
    ChatGpt,
    /// Claude account plan.
    Claude,
}

impl ProviderAccountEntitlementDomain {
    /// Stable management/storage value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GrokBuild => "grok_build",
            Self::GrokWeb => "grok_web",
            Self::ChatGpt => "chatgpt",
            Self::Claude => "claude",
        }
    }
}

/// Closed normalized tier. The enum variant retains the namespace even when wire labels overlap.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderAccountEntitlementTier {
    /// Unrecognized Grok Build subscription.
    GrokBuildUnknown,
    /// Grok Build free plan.
    GrokBuildFree,
    /// Grok Build `SuperGrok` plan.
    GrokBuildSupergrok,
    /// Grok Build Heavy plan.
    GrokBuildHeavy,
    /// Unrecognized Grok Web subscription.
    GrokWebUnknown,
    /// Grok Web basic plan.
    GrokWebBasic,
    /// Grok Web Super plan.
    GrokWebSuper,
    /// Grok Web Heavy plan.
    GrokWebHeavy,
    /// Unrecognized ChatGPT/Codex plan.
    ChatGptUnknown,
    /// `ChatGPT` free plan.
    ChatGptFree,
    /// `ChatGPT` Go plan.
    ChatGptGo,
    /// `ChatGPT` Plus plan.
    ChatGptPlus,
    /// `ChatGPT` Pro 5x plan.
    ChatGptPro5x,
    /// `ChatGPT` Pro 20x plan.
    ChatGptPro20x,
    /// Unrecognized Claude plan.
    ClaudeUnknown,
    /// Claude free plan.
    ClaudeFree,
    /// Claude Pro plan.
    ClaudePro,
    /// Claude Max 5x plan.
    ClaudeMax5x,
    /// Claude Max 20x plan.
    ClaudeMax20x,
}

impl ProviderAccountEntitlementTier {
    /// Namespace to which this tier belongs.
    #[must_use]
    pub const fn domain(self) -> ProviderAccountEntitlementDomain {
        match self {
            Self::GrokBuildUnknown
            | Self::GrokBuildFree
            | Self::GrokBuildSupergrok
            | Self::GrokBuildHeavy => ProviderAccountEntitlementDomain::GrokBuild,
            Self::GrokWebUnknown | Self::GrokWebBasic | Self::GrokWebSuper | Self::GrokWebHeavy => {
                ProviderAccountEntitlementDomain::GrokWeb
            }
            Self::ChatGptUnknown
            | Self::ChatGptFree
            | Self::ChatGptGo
            | Self::ChatGptPlus
            | Self::ChatGptPro5x
            | Self::ChatGptPro20x => ProviderAccountEntitlementDomain::ChatGpt,
            Self::ClaudeUnknown
            | Self::ClaudeFree
            | Self::ClaudePro
            | Self::ClaudeMax5x
            | Self::ClaudeMax20x => ProviderAccountEntitlementDomain::Claude,
        }
    }

    /// Stable tier label interpreted only together with [`Self::domain`].
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GrokBuildUnknown
            | Self::GrokWebUnknown
            | Self::ChatGptUnknown
            | Self::ClaudeUnknown => "unknown",
            Self::GrokBuildFree | Self::ChatGptFree | Self::ClaudeFree => "free",
            Self::GrokBuildSupergrok => "supergrok",
            Self::GrokBuildHeavy | Self::GrokWebHeavy => "heavy",
            Self::GrokWebBasic => "basic",
            Self::GrokWebSuper => "super",
            Self::ChatGptGo => "go",
            Self::ChatGptPlus => "plus",
            Self::ChatGptPro5x => "pro5x",
            Self::ChatGptPro20x => "pro20x",
            Self::ClaudePro => "pro",
            Self::ClaudeMax5x => "max5x",
            Self::ClaudeMax20x => "max20x",
        }
    }

    /// Reconstructs a tier from a trusted persisted domain/value pair.
    ///
    /// # Errors
    ///
    /// Returns a value-free error when the tier is not in the selected domain's closed set.
    pub fn parse(
        domain: ProviderAccountEntitlementDomain,
        tier: &str,
    ) -> Result<Self, ProviderAccountEntitlementError> {
        match (domain, tier) {
            (ProviderAccountEntitlementDomain::GrokBuild, "unknown") => Ok(Self::GrokBuildUnknown),
            (ProviderAccountEntitlementDomain::GrokBuild, "free") => Ok(Self::GrokBuildFree),
            (ProviderAccountEntitlementDomain::GrokBuild, "supergrok") => {
                Ok(Self::GrokBuildSupergrok)
            }
            (ProviderAccountEntitlementDomain::GrokBuild, "heavy") => Ok(Self::GrokBuildHeavy),
            (ProviderAccountEntitlementDomain::GrokWeb, "unknown") => Ok(Self::GrokWebUnknown),
            (ProviderAccountEntitlementDomain::GrokWeb, "basic") => Ok(Self::GrokWebBasic),
            (ProviderAccountEntitlementDomain::GrokWeb, "super") => Ok(Self::GrokWebSuper),
            (ProviderAccountEntitlementDomain::GrokWeb, "heavy") => Ok(Self::GrokWebHeavy),
            (ProviderAccountEntitlementDomain::ChatGpt, "unknown") => Ok(Self::ChatGptUnknown),
            (ProviderAccountEntitlementDomain::ChatGpt, "free") => Ok(Self::ChatGptFree),
            (ProviderAccountEntitlementDomain::ChatGpt, "go") => Ok(Self::ChatGptGo),
            (ProviderAccountEntitlementDomain::ChatGpt, "plus") => Ok(Self::ChatGptPlus),
            (ProviderAccountEntitlementDomain::ChatGpt, "pro5x") => Ok(Self::ChatGptPro5x),
            (ProviderAccountEntitlementDomain::ChatGpt, "pro20x") => Ok(Self::ChatGptPro20x),
            (ProviderAccountEntitlementDomain::Claude, "unknown") => Ok(Self::ClaudeUnknown),
            (ProviderAccountEntitlementDomain::Claude, "free") => Ok(Self::ClaudeFree),
            (ProviderAccountEntitlementDomain::Claude, "pro") => Ok(Self::ClaudePro),
            (ProviderAccountEntitlementDomain::Claude, "max5x") => Ok(Self::ClaudeMax5x),
            (ProviderAccountEntitlementDomain::Claude, "max20x") => Ok(Self::ClaudeMax20x),
            _ => Err(ProviderAccountEntitlementError),
        }
    }
}

/// Where the entitlement observation came from.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderAccountEntitlementSource {
    /// Exact Provider subscription endpoint.
    ProviderSubscription,
    /// Locally decoded JWT-shaped token claim; this type does not assert signature verification.
    SignedToken,
    /// Explicit metadata in an imported credential envelope.
    ImportedMetadata,
}

impl ProviderAccountEntitlementSource {
    /// Stable storage and management value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProviderSubscription => "provider_subscription",
            Self::SignedToken => "signed_token",
            Self::ImportedMetadata => "imported_metadata",
        }
    }

    /// Parses one trusted persisted value.
    ///
    /// # Errors
    ///
    /// Returns a value-free error for an unknown source label.
    pub fn parse(value: &str) -> Result<Self, ProviderAccountEntitlementError> {
        match value {
            "provider_subscription" => Ok(Self::ProviderSubscription),
            "signed_token" => Ok(Self::SignedToken),
            "imported_metadata" => Ok(Self::ImportedMetadata),
            _ => Err(ProviderAccountEntitlementError),
        }
    }
}

/// Strength of the bounded evidence behind one observation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderAccountEntitlementConfidence {
    /// Exact Provider subscription response.
    Authoritative,
    /// Locally derived from a carried signed-token claim.
    Derived,
    /// Explicitly declared by an imported envelope.
    Declared,
}

impl ProviderAccountEntitlementConfidence {
    /// Stable storage and management value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authoritative => "authoritative",
            Self::Derived => "derived",
            Self::Declared => "declared",
        }
    }

    /// Parses one trusted persisted value.
    ///
    /// # Errors
    ///
    /// Returns a value-free error for an unknown confidence label.
    pub fn parse(value: &str) -> Result<Self, ProviderAccountEntitlementError> {
        match value {
            "authoritative" => Ok(Self::Authoritative),
            "derived" => Ok(Self::Derived),
            "declared" => Ok(Self::Declared),
            _ => Err(ProviderAccountEntitlementError),
        }
    }
}

/// One safe observed entitlement. Missing evidence is represented by `Option::None` at its owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderAccountEntitlement {
    tier: ProviderAccountEntitlementTier,
    source: ProviderAccountEntitlementSource,
    confidence: ProviderAccountEntitlementConfidence,
    observed_at_ms: i64,
}

impl ProviderAccountEntitlement {
    /// Creates one observation and rejects a fabricated source/confidence pairing.
    ///
    /// # Errors
    ///
    /// Returns a value-free error for a negative timestamp or invalid evidence-strength pair.
    pub fn try_new(
        tier: ProviderAccountEntitlementTier,
        source: ProviderAccountEntitlementSource,
        confidence: ProviderAccountEntitlementConfidence,
        observed_at_ms: i64,
    ) -> Result<Self, ProviderAccountEntitlementError> {
        let evidence_matches = matches!(
            (source, confidence),
            (
                ProviderAccountEntitlementSource::ProviderSubscription,
                ProviderAccountEntitlementConfidence::Authoritative
            ) | (
                ProviderAccountEntitlementSource::SignedToken,
                ProviderAccountEntitlementConfidence::Derived
            ) | (
                ProviderAccountEntitlementSource::ImportedMetadata,
                ProviderAccountEntitlementConfidence::Declared
            )
        );
        if observed_at_ms < 0 || !evidence_matches {
            return Err(ProviderAccountEntitlementError);
        }
        Ok(Self {
            tier,
            source,
            confidence,
            observed_at_ms,
        })
    }

    /// Returns the tier namespace.
    #[must_use]
    pub const fn domain(self) -> ProviderAccountEntitlementDomain {
        self.tier.domain()
    }

    /// Returns the domain-retaining tier.
    #[must_use]
    pub const fn tier(self) -> ProviderAccountEntitlementTier {
        self.tier
    }

    /// Returns the observation source.
    #[must_use]
    pub const fn source(self) -> ProviderAccountEntitlementSource {
        self.source
    }

    /// Returns the evidence confidence.
    #[must_use]
    pub const fn confidence(self) -> ProviderAccountEntitlementConfidence {
        self.confidence
    }

    /// Returns the fixed Unix-millisecond observation time.
    #[must_use]
    pub const fn observed_at_ms(self) -> i64 {
        self.observed_at_ms
    }
}

/// Value-free invalid entitlement classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderAccountEntitlementError;

impl fmt::Display for ProviderAccountEntitlementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("provider account entitlement is invalid")
    }
}

impl Error for ProviderAccountEntitlementError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_labels_retain_their_domain() {
        assert_ne!(
            ProviderAccountEntitlementTier::GrokBuildFree,
            ProviderAccountEntitlementTier::ChatGptFree
        );
        assert_eq!(
            ProviderAccountEntitlementTier::GrokBuildFree.as_str(),
            ProviderAccountEntitlementTier::ChatGptFree.as_str()
        );
        assert_eq!(
            ProviderAccountEntitlementTier::GrokBuildFree.domain(),
            ProviderAccountEntitlementDomain::GrokBuild
        );
        assert_eq!(
            ProviderAccountEntitlementTier::ChatGptFree.domain(),
            ProviderAccountEntitlementDomain::ChatGpt
        );
    }

    #[test]
    fn evidence_strength_cannot_be_overstated() {
        assert!(
            ProviderAccountEntitlement::try_new(
                ProviderAccountEntitlementTier::GrokBuildSupergrok,
                ProviderAccountEntitlementSource::SignedToken,
                ProviderAccountEntitlementConfidence::Authoritative,
                1,
            )
            .is_err()
        );
    }
}
