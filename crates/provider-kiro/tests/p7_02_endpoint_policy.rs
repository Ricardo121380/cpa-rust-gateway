//! P7-02 Kiro IDE/CLI endpoint-policy snapshots.

use std::{collections::BTreeMap, error::Error};

use provider_kiro::{
    credential::KiroCredentialKind,
    endpoint_policy::{
        KiroApiRegion, KiroEndpointKind, KiroEndpointPolicy, KiroEndpointPolicyError,
        KiroRequestOrigin, KiroThinkingPlacement,
    },
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn ide_and_cli_snapshots_have_exact_urls_headers_origins_and_thinking_placement() -> TestResult {
    let region = KiroApiRegion::try_new("us-east-1")?;
    let ide = KiroEndpointPolicy::try_new(KiroEndpointKind::Ide, region.clone())?;
    let cli = KiroEndpointPolicy::try_new(KiroEndpointKind::Cli, region)?;

    assert_eq!(
        ide.url().as_str(),
        "https://q.us-east-1.amazonaws.com/generateAssistantResponse"
    );
    assert_eq!(ide.origin(), KiroRequestOrigin::AiEditor);
    assert_eq!(
        ide.thinking_placement(),
        KiroThinkingPlacement::IdeThinkingWrapper
    );
    assert_eq!(
        ide.request_headers(KiroCredentialKind::Social),
        BTreeMap::from([
            ("content-type".to_owned(), "application/json".to_owned()),
            ("origin".to_owned(), "AI_EDITOR".to_owned()),
        ])
    );

    assert_eq!(cli.url().as_str(), "https://runtime.us-east-1.kiro.dev/");
    assert_eq!(cli.origin(), KiroRequestOrigin::KiroCli);
    assert_eq!(
        cli.thinking_placement(),
        KiroThinkingPlacement::CliOutputConfigEffort
    );
    assert_eq!(
        cli.request_headers(KiroCredentialKind::Enterprise),
        BTreeMap::from([
            (
                "content-type".to_owned(),
                "application/x-amz-json-1.0".to_owned(),
            ),
            ("origin".to_owned(), "KIRO_CLI".to_owned()),
            (
                "x-amz-target".to_owned(),
                "AmazonCodeWhispererStreamingService.GenerateAssistantResponse".to_owned(),
            ),
        ])
    );
    Ok(())
}

#[test]
fn api_key_header_is_explicit_and_oauth_never_inherits_it() -> TestResult {
    let policy = KiroEndpointPolicy::try_new(
        KiroEndpointKind::Cli,
        KiroApiRegion::try_new("ap-southeast-2")?,
    )?;
    assert_eq!(
        policy
            .request_headers(KiroCredentialKind::ApiKey)
            .get("tokentype"),
        Some(&"API_KEY".to_owned())
    );
    assert!(
        !policy
            .request_headers(KiroCredentialKind::Social)
            .contains_key("tokentype")
    );
    assert!(
        !policy
            .request_headers(KiroCredentialKind::Enterprise)
            .contains_key("tokentype")
    );
    Ok(())
}

#[test]
fn api_region_is_bounded_and_cannot_change_the_derived_host_shape() -> TestResult {
    for value in [
        "US_EAST_1",
        "us-east-1.amazonaws.com",
        "us-east-1/../../metadata",
        "https://example.invalid",
        "us-east",
        "us-east-1-extra",
    ] {
        assert!(matches!(
            KiroApiRegion::try_new(value),
            Err(KiroEndpointPolicyError::InvalidRegion)
        ));
    }
    let oversized = "a".repeat(129);
    assert!(matches!(
        KiroApiRegion::try_new(oversized),
        Err(KiroEndpointPolicyError::InvalidRegion)
    ));
    let region = KiroApiRegion::try_new("eu-central-1")?;
    let policy = KiroEndpointPolicy::try_new(KiroEndpointKind::Ide, region)?;
    assert_eq!(policy.url().scheme(), "https");
    assert_eq!(
        policy.url().host_str(),
        Some("q.eu-central-1.amazonaws.com")
    );
    assert_eq!(policy.url().path(), "/generateAssistantResponse");
    assert_eq!(
        KiroApiRegion::try_new("us-gov-west-1")?.as_str(),
        "us-gov-west-1"
    );
    Ok(())
}
