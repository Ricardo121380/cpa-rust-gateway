//! Provider-scoped fixed-point pricing for the P13-05 billing ledger.
//!
//! This module is intentionally transport-neutral.  It selects exactly one catalog entry by
//! `(Provider, Channel, Model)` and computes integer micro-units without guessing missing token
//! fields.  Persistence and replay are owned by [`gateway_store::billing_ledger`].

use gateway_core::UsageSummary;
use gateway_store::billing_ledger::{
    BillingCostConfidence, BillingPriceCatalog, BillingPriceEntry,
};

/// Number of tokens represented by a catalog rate denominator.
pub const TOKENS_PER_MILLION: u128 = 1_000_000;

/// A deterministic pricing result ready to be recorded in the durable ledger.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingQuote {
    /// Catalog version used for this quote.
    pub catalog_version_id: String,
    /// Calculated integer micro-units, or a lower-bound partial amount.
    pub cost_microunits: Option<u64>,
    /// Whether the amount is exact, partial, unknown, or unpriced.
    pub confidence: BillingCostConfidence,
}

/// Failure returned when a known rate cannot be represented in the fixed-point ledger unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BillingPricingError {
    /// A checked multiplication or conversion overflowed.
    ArithmeticOverflow,
}

/// Prices one final Usage summary against one Provider/Channel/Model catalog entry.
///
/// A missing catalog entry is a normal `unpriced` outcome.  Missing token dimensions do not become
/// zero: if some known dimensions can be priced, the result is an explicitly partial lower bound;
/// if none can be priced, the result is `unknown`.
///
/// # Errors
///
/// Returns [`BillingPricingError::ArithmeticOverflow`] if checked fixed-point multiplication,
/// addition or conversion cannot be represented.
pub fn quote_usage(
    catalog: &BillingPriceCatalog,
    provider_id: &str,
    channel_id: &str,
    model: &str,
    usage: &UsageSummary,
) -> Result<BillingQuote, BillingPricingError> {
    let Some(entry) = catalog.entries.iter().find(|entry| {
        entry.provider_id == provider_id && entry.channel_id == channel_id && entry.model == model
    }) else {
        return Ok(BillingQuote {
            catalog_version_id: catalog.catalog_version_id.clone(),
            cost_microunits: None,
            confidence: BillingCostConfidence::Unpriced,
        });
    };

    let mut cost = 0_u128;
    let mut priced_dimensions = 0_u8;
    let mut missing_dimensions = 0_u8;
    for (tokens, rate) in [
        (usage.input_tokens, entry.input_microunits_per_million),
        (usage.output_tokens, entry.output_microunits_per_million),
        (
            usage.reasoning_tokens,
            entry.reasoning_microunits_per_million,
        ),
        (
            usage.cache_read_tokens,
            entry.cache_read_microunits_per_million,
        ),
        (
            usage.cache_creation_tokens,
            entry.cache_creation_microunits_per_million,
        ),
        (usage.cached_tokens, entry.cached_microunits_per_million),
    ] {
        let Some(tokens) = tokens else {
            missing_dimensions = missing_dimensions.saturating_add(1);
            continue;
        };
        let dimension = u128::from(tokens)
            .checked_mul(u128::from(rate))
            .ok_or(BillingPricingError::ArithmeticOverflow)?
            / TOKENS_PER_MILLION;
        cost = cost
            .checked_add(dimension)
            .ok_or(BillingPricingError::ArithmeticOverflow)?;
        priced_dimensions = priced_dimensions.saturating_add(1);
    }

    let confidence = if priced_dimensions == 0 {
        BillingCostConfidence::Unknown
    } else if missing_dimensions == 0 {
        BillingCostConfidence::Exact
    } else {
        BillingCostConfidence::Partial
    };
    let cost_microunits = if priced_dimensions == 0 {
        None
    } else {
        Some(u64::try_from(cost).map_err(|_| BillingPricingError::ArithmeticOverflow)?)
    };
    Ok(BillingQuote {
        catalog_version_id: catalog.catalog_version_id.clone(),
        cost_microunits,
        confidence,
    })
}

/// Returns the catalog row selected for a Provider/Channel/Model tuple.
#[must_use]
pub fn find_price_entry<'catalog>(
    catalog: &'catalog BillingPriceCatalog,
    provider_id: &str,
    channel_id: &str,
    model: &str,
) -> Option<&'catalog BillingPriceEntry> {
    catalog.entries.iter().find(|entry| {
        entry.provider_id == provider_id && entry.channel_id == channel_id && entry.model == model
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_store::billing_ledger::{BillingCatalogSource, BillingPriceEntry};

    fn catalog() -> BillingPriceCatalog {
        BillingPriceCatalog {
            catalog_version_id: "catalog-1".to_owned(),
            effective_at_ms: 100,
            source: BillingCatalogSource::Test,
            created_at_ms: 100,
            entries: vec![BillingPriceEntry {
                provider_id: "provider-a".to_owned(),
                channel_id: "channel-a".to_owned(),
                model: "model-a".to_owned(),
                input_microunits_per_million: 2_000_000,
                output_microunits_per_million: 4_000_000,
                reasoning_microunits_per_million: 1_000_000,
                cache_read_microunits_per_million: 500_000,
                cache_creation_microunits_per_million: 500_000,
                cached_microunits_per_million: 250_000,
            }],
        }
    }

    #[test]
    fn exact_quote_uses_integer_rates() -> Result<(), BillingPricingError> {
        let usage = UsageSummary {
            input_tokens: Some(1_000_000),
            output_tokens: Some(500_000),
            reasoning_tokens: Some(0),
            cache_read_tokens: Some(0),
            cache_creation_tokens: Some(0),
            cached_tokens: Some(0),
        };
        let quote = quote_usage(&catalog(), "provider-a", "channel-a", "model-a", &usage)?;
        assert_eq!(quote.cost_microunits, Some(4_000_000));
        assert_eq!(quote.confidence, BillingCostConfidence::Exact);
        Ok(())
    }

    #[test]
    fn missing_dimensions_are_partial_or_unknown_not_zero() -> Result<(), BillingPricingError> {
        let partial = UsageSummary {
            input_tokens: Some(1_000_000),
            ..UsageSummary::default()
        };
        let quote = quote_usage(&catalog(), "provider-a", "channel-a", "model-a", &partial)?;
        assert_eq!(quote.cost_microunits, Some(2_000_000));
        assert_eq!(quote.confidence, BillingCostConfidence::Partial);

        let unknown = UsageSummary::default();
        let quote = quote_usage(&catalog(), "provider-a", "channel-a", "model-a", &unknown)?;
        assert_eq!(quote.cost_microunits, None);
        assert_eq!(quote.confidence, BillingCostConfidence::Unknown);
        Ok(())
    }

    #[test]
    fn absent_catalog_entry_is_unpriced() -> Result<(), BillingPricingError> {
        let quote = quote_usage(
            &catalog(),
            "provider-a",
            "channel-a",
            "other-model",
            &UsageSummary::default(),
        )?;
        assert_eq!(quote.cost_microunits, None);
        assert_eq!(quote.confidence, BillingCostConfidence::Unpriced);
        Ok(())
    }
}
