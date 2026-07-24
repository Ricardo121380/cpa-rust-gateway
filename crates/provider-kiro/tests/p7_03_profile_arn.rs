//! P7-03 Kiro profile ARN lifecycle regressions.

use provider_kiro::{
    credential::KiroCredentialKind,
    endpoint_policy::KiroApiRegion,
    profile_arn::{
        KiroEnterpriseProfileLookup, KiroProfileArnError, KiroProfileArnSource, inject_profile_arn,
        resolve_profile_arn,
    },
};
use serde_json::json;
use std::error::Error;
type TestResult = Result<(), Box<dyn Error>>;
struct Lookup(Result<String, KiroProfileArnError>);
impl KiroEnterpriseProfileLookup for Lookup {
    fn lookup(&self, _: &KiroApiRegion) -> Result<String, KiroProfileArnError> {
        self.0.clone()
    }
}
#[test]
fn profile_resolution_is_kind_specific_and_fallback_is_region_aware() -> TestResult {
    let us = KiroApiRegion::try_new("us-east-1")?;
    let eu = KiroApiRegion::try_new("eu-west-1")?;
    let social = resolve_profile_arn(
        KiroCredentialKind::Social,
        &us,
        &Lookup(Err(KiroProfileArnError::InvalidProfileArn)),
    );
    assert_eq!(social.source(), KiroProfileArnSource::BuilderDefault);
    assert!(
        social
            .arn()
            .is_some_and(|arn| arn.as_str().contains("AAAACCCCXXXX"))
    );
    let api = resolve_profile_arn(KiroCredentialKind::ApiKey, &us, &Lookup(Ok("bad".into())));
    assert_eq!(api.source(), KiroProfileArnSource::ApiKeyOmitted);
    assert!(api.arn().is_none());
    assert!(!api.audit(&us).profile_present());
    let fallback = resolve_profile_arn(
        KiroCredentialKind::Enterprise,
        &eu,
        &Lookup(Err(KiroProfileArnError::InvalidProfileArn)),
    );
    assert_eq!(fallback.source(), KiroProfileArnSource::EnterpriseFallback);
    assert!(
        fallback
            .arn()
            .is_some_and(|arn| arn.as_str().contains(":eu-central-1:"))
    );
    Ok(())
}
#[test]
fn enterprise_lookup_is_validated_injected_and_auditable() -> TestResult {
    let region = KiroApiRegion::try_new("us-west-2")?;
    let resolution = resolve_profile_arn(
        KiroCredentialKind::Enterprise,
        &region,
        &Lookup(Ok(
            "arn:aws:codewhisperer:us-west-2:123456789012:profile/REAL123".into(),
        )),
    );
    assert_eq!(resolution.source(), KiroProfileArnSource::EnterpriseLookup);
    let mut body = json!({"conversationState":{}});
    inject_profile_arn(&mut body, &resolution)?;
    assert_eq!(
        body["profileArn"],
        "arn:aws:codewhisperer:us-west-2:123456789012:profile/REAL123"
    );
    assert!(!format!("{resolution:?}").contains("REAL123"));
    assert!(!format!("{:?}", resolution.audit(&region)).contains("REAL123"));
    Ok(())
}
