//! P8-03 synthetic Official rate-limit/reset/billing metadata evidence.

#![deny(unsafe_code)]

use std::{error::Error, time::Duration};

use gateway_core::{ErrorScope, GatewayErrorCode, Usage};
use provider_grok::{
    GrokOfficialBillingMetadata, GrokOfficialRateLimitKind, GrokOfficialRateLimitMetadata,
    MAX_GROK_OFFICIAL_RATE_LIMIT_RESET,
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn fixed_complete_rate_limit_windows_are_case_insensitive_and_redacted() -> TestResult {
    let metadata = GrokOfficialRateLimitMetadata::parse([
        ("X-RateLimit-Limit-Requests", "100"),
        ("x-ratelimit-remaining-requests", "99"),
        ("x-ratelimit-reset-requests", "2s"),
        ("x-ratelimit-limit-tokens", "10000"),
        ("x-ratelimit-remaining-tokens", "9999"),
        ("x-ratelimit-reset-tokens", "1500ms"),
        ("Retry-After", "3"),
        ("x-unrelated-header", "must-not-be-retained"),
    ])?;

    assert_eq!(metadata.windows().len(), 2);
    assert_eq!(
        metadata.windows()[0].kind(),
        GrokOfficialRateLimitKind::Requests
    );
    assert_eq!(metadata.windows()[0].limit(), 100);
    assert_eq!(metadata.windows()[0].remaining(), 99);
    assert_eq!(metadata.windows()[0].reset_after(), Duration::from_secs(2));
    assert_eq!(
        metadata.windows()[1].kind(),
        GrokOfficialRateLimitKind::Tokens
    );
    assert_eq!(
        metadata.windows()[1].reset_after(),
        Duration::from_millis(1500)
    );
    assert_eq!(metadata.retry_after(), Some(Duration::from_secs(3)));

    let diagnostic = format!("{metadata:?} {:?}", metadata.windows()[0]);
    for value in ["100", "9999", "1500", "must-not-be-retained"] {
        assert!(!diagnostic.contains(value));
    }
    Ok(())
}

#[test]
fn ambiguous_partial_unsafe_or_impossible_headers_fail_closed() -> TestResult {
    let invalid_headers = [
        vec![
            ("x-ratelimit-limit-requests", "100"),
            ("x-ratelimit-remaining-requests", "1"),
        ],
        vec![
            ("x-ratelimit-limit-tokens", "100"),
            ("x-ratelimit-limit-tokens", "99"),
            ("x-ratelimit-remaining-tokens", "1"),
            ("x-ratelimit-reset-tokens", "1s"),
        ],
        vec![
            ("x-ratelimit-limit-requests", "1"),
            ("x-ratelimit-remaining-requests", "2"),
            ("x-ratelimit-reset-requests", "1s"),
        ],
        vec![
            ("x-ratelimit-limit-requests", "1"),
            ("x-ratelimit-remaining-requests", "0"),
            ("x-ratelimit-reset-requests", "0s"),
        ],
        vec![
            ("x-ratelimit-limit-requests", "1"),
            ("x-ratelimit-remaining-requests", "0"),
            ("x-ratelimit-reset-requests", "25h"),
        ],
        vec![("retry-after", "Wed, 21 Oct 2015 07:28:00 GMT")],
        vec![("retry-after", "999999999999999999999999999999999")],
    ];

    for headers in invalid_headers {
        let error = GrokOfficialRateLimitMetadata::parse(headers)
            .err()
            .ok_or("ambiguous or unsafe Official header evidence was accepted")?;
        assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
        assert_eq!(error.scope(), ErrorScope::Provider);
    }
    assert!(MAX_GROK_OFFICIAL_RATE_LIMIT_RESET < Duration::from_hours(25));
    Ok(())
}

#[test]
fn token_usage_is_billing_metadata_without_inventing_a_plan_or_price() {
    let usage = Usage {
        input_tokens: Some(10),
        output_tokens: Some(3),
        reasoning_tokens: Some(2),
        cached_tokens: Some(4),
        ..Usage::default()
    };
    let billing = GrokOfficialBillingMetadata::from_usage(&usage);

    assert_eq!(billing.input_tokens(), Some(10));
    assert_eq!(billing.output_tokens(), Some(3));
    assert_eq!(billing.reasoning_tokens(), Some(2));
    assert_eq!(billing.cached_tokens(), Some(4));
    assert!(!billing.is_empty());
    let diagnostic = format!("{billing:?}");
    for value in ["10", "3", "2", "4", "price", "plan"] {
        assert!(!diagnostic.contains(value));
    }
}
