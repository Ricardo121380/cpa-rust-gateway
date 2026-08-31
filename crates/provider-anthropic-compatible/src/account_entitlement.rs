//! Strict Claude account-plan normalization.

use gateway_core::{
    ProviderAccountEntitlement, ProviderAccountEntitlementConfidence,
    ProviderAccountEntitlementSource, ProviderAccountEntitlementTier,
};

/// Normalizes one explicit imported Claude plan label without guessing between Max multipliers.
pub(crate) fn claude_entitlement_from_imported_plan(
    plan: &str,
    observed_at_ms: i64,
) -> Option<ProviderAccountEntitlement> {
    let tier = match plan.trim().to_ascii_lowercase().as_str() {
        "free" => ProviderAccountEntitlementTier::ClaudeFree,
        "pro" | "claude pro" | "claude_pro" | "claude-pro" => {
            ProviderAccountEntitlementTier::ClaudePro
        }
        "max5x" | "max_5x" | "max-5x" | "max 5x" => ProviderAccountEntitlementTier::ClaudeMax5x,
        "max20x" | "max_20x" | "max-20x" | "max 20x" => {
            ProviderAccountEntitlementTier::ClaudeMax20x
        }
        _ => ProviderAccountEntitlementTier::ClaudeUnknown,
    };
    ProviderAccountEntitlement::try_new(
        tier,
        ProviderAccountEntitlementSource::ImportedMetadata,
        ProviderAccountEntitlementConfidence::Declared,
        observed_at_ms,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use gateway_core::{ProviderAccountEntitlementDomain, ProviderAccountEntitlementTier};

    use super::*;

    #[test]
    fn normalizes_only_the_supported_claude_plan_vocabulary()
    -> Result<(), Box<dyn std::error::Error>> {
        for (label, expected) in [
            ("free", ProviderAccountEntitlementTier::ClaudeFree),
            ("Claude Pro", ProviderAccountEntitlementTier::ClaudePro),
            ("max-5x", ProviderAccountEntitlementTier::ClaudeMax5x),
            ("max 20x", ProviderAccountEntitlementTier::ClaudeMax20x),
        ] {
            let entitlement = claude_entitlement_from_imported_plan(label, 42)
                .ok_or("Claude entitlement missing")?;
            assert_eq!(
                entitlement.domain(),
                ProviderAccountEntitlementDomain::Claude
            );
            assert_eq!(entitlement.tier(), expected);
        }
        Ok(())
    }

    #[test]
    fn ambiguous_and_fuzzy_labels_are_unknown_instead_of_guessed()
    -> Result<(), Box<dyn std::error::Error>> {
        for label in ["max", "premium", "max10x", "pro-team"] {
            let entitlement = claude_entitlement_from_imported_plan(label, 42)
                .ok_or("Claude unknown entitlement missing")?;
            assert_eq!(
                entitlement.tier(),
                ProviderAccountEntitlementTier::ClaudeUnknown
            );
        }
        Ok(())
    }
}
