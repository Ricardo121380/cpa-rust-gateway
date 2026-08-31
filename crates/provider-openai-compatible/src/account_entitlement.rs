//! Strict ChatGPT/Codex account-plan normalization.

use gateway_core::{
    ProviderAccountEntitlement, ProviderAccountEntitlementConfidence,
    ProviderAccountEntitlementSource, ProviderAccountEntitlementTier,
};

/// Normalizes one explicit imported `ChatGPT` plan label without guessing between paid products.
pub(crate) fn chatgpt_entitlement_from_imported_plan(
    plan: &str,
    observed_at_ms: i64,
) -> Option<ProviderAccountEntitlement> {
    entitlement(
        plan,
        ProviderAccountEntitlementSource::ImportedMetadata,
        ProviderAccountEntitlementConfidence::Declared,
        observed_at_ms,
    )
}

/// Normalizes one `ChatGPT` plan carried by an OAuth signed-token claim.
pub(crate) fn chatgpt_entitlement_from_signed_plan(
    plan: &str,
    observed_at_ms: i64,
) -> Option<ProviderAccountEntitlement> {
    entitlement(
        plan,
        ProviderAccountEntitlementSource::SignedToken,
        ProviderAccountEntitlementConfidence::Derived,
        observed_at_ms,
    )
}

fn entitlement(
    plan: &str,
    source: ProviderAccountEntitlementSource,
    confidence: ProviderAccountEntitlementConfidence,
    observed_at_ms: i64,
) -> Option<ProviderAccountEntitlement> {
    let tier = match plan.trim().to_ascii_lowercase().as_str() {
        "free" => ProviderAccountEntitlementTier::ChatGptFree,
        "go" | "chatgpt go" | "chatgpt_go" | "chatgpt-go" => {
            ProviderAccountEntitlementTier::ChatGptGo
        }
        "plus" | "chatgpt plus" | "chatgpt_plus" | "chatgpt-plus" => {
            ProviderAccountEntitlementTier::ChatGptPlus
        }
        "pro5x" | "pro_5x" | "pro-5x" | "pro 5x" => ProviderAccountEntitlementTier::ChatGptPro5x,
        "pro20x" | "pro_20x" | "pro-20x" | "pro 20x" => {
            ProviderAccountEntitlementTier::ChatGptPro20x
        }
        _ => ProviderAccountEntitlementTier::ChatGptUnknown,
    };
    ProviderAccountEntitlement::try_new(tier, source, confidence, observed_at_ms).ok()
}

#[cfg(test)]
mod tests {
    use gateway_core::{
        ProviderAccountEntitlementConfidence, ProviderAccountEntitlementDomain,
        ProviderAccountEntitlementSource, ProviderAccountEntitlementTier,
    };

    use super::*;

    #[test]
    fn normalizes_only_the_supported_chatgpt_plan_vocabulary()
    -> Result<(), Box<dyn std::error::Error>> {
        for (label, expected) in [
            ("free", ProviderAccountEntitlementTier::ChatGptFree),
            ("Go", ProviderAccountEntitlementTier::ChatGptGo),
            ("chatgpt_plus", ProviderAccountEntitlementTier::ChatGptPlus),
            ("pro-5x", ProviderAccountEntitlementTier::ChatGptPro5x),
            ("pro 20x", ProviderAccountEntitlementTier::ChatGptPro20x),
        ] {
            let entitlement = chatgpt_entitlement_from_imported_plan(label, 42)
                .ok_or("ChatGPT entitlement missing")?;
            assert_eq!(
                entitlement.domain(),
                ProviderAccountEntitlementDomain::ChatGpt
            );
            assert_eq!(entitlement.tier(), expected);
            assert_eq!(
                entitlement.source(),
                ProviderAccountEntitlementSource::ImportedMetadata
            );
            assert_eq!(
                entitlement.confidence(),
                ProviderAccountEntitlementConfidence::Declared
            );
        }
        Ok(())
    }

    #[test]
    fn ambiguous_and_fuzzy_labels_are_unknown_instead_of_guessed()
    -> Result<(), Box<dyn std::error::Error>> {
        for label in ["pro", "premium", "max", "pro10x", "plus-team"] {
            let entitlement = chatgpt_entitlement_from_signed_plan(label, 42)
                .ok_or("ChatGPT unknown entitlement missing")?;
            assert_eq!(
                entitlement.tier(),
                ProviderAccountEntitlementTier::ChatGptUnknown
            );
        }
        Ok(())
    }

    #[test]
    fn invalid_observation_time_is_rejected() {
        assert!(chatgpt_entitlement_from_imported_plan("plus", -1).is_none());
    }
}
