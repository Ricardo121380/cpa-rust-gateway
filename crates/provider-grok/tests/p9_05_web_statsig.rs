//! P9-05 zero-network Statsig cache and signer SSRF-admission evidence.

#![deny(unsafe_code)]

use std::{
    collections::BTreeSet,
    error::Error,
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

use gateway_core::EgressPolicyId;
use gateway_upstream::{
    EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme,
    RedirectPolicy,
};
use provider_grok::{
    GrokWebStatsigError, GrokWebStatsigSignature, GrokWebStatsigSignatureCache,
    GrokWebStatsigSignatureKey, GrokWebStatsigSignerBoundary,
};

type TestResult = Result<(), Box<dyn Error>>;

const NOW_MS: i64 = 1_000_000;

#[test]
fn signature_cache_is_exact_key_isolated_and_a_403_invalidates_only_one_entry() -> TestResult {
    let cache = GrokWebStatsigSignatureCache::try_new(2)?;
    let first = key("GET", "/initialize", "env-01")?;
    let second = key("POST", "/initialize", "env-01")?;
    cache.insert(
        first.clone(),
        GrokWebStatsigSignature::try_new("signature_one")?,
        NOW_MS + 100,
        NOW_MS,
    )?;
    cache.insert(
        second.clone(),
        GrokWebStatsigSignature::try_new("signature_two")?,
        NOW_MS + 100,
        NOW_MS,
    )?;
    assert_eq!(
        cache
            .get(&first, NOW_MS)?
            .as_ref()
            .map(GrokWebStatsigSignature::as_str),
        Some("signature_one")
    );
    assert!(cache.invalidate_after_403(&first)?);
    assert!(!cache.invalidate_after_403(&first)?);
    assert!(cache.get(&first, NOW_MS)?.is_none());
    assert_eq!(
        cache
            .get(&second, NOW_MS)?
            .as_ref()
            .map(GrokWebStatsigSignature::as_str),
        Some("signature_two")
    );
    Ok(())
}

#[test]
fn cache_reclaims_only_expired_entries_and_rejects_unsafe_keys_values_and_capacity() -> TestResult {
    let cache = GrokWebStatsigSignatureCache::try_new(1)?;
    let expired = key("GET", "/expired", "env-01")?;
    cache.insert(
        expired.clone(),
        GrokWebStatsigSignature::try_new("signature_expired")?,
        NOW_MS + 1,
        NOW_MS,
    )?;
    let replacement = key("GET", "/replacement", "env-01")?;
    cache.insert(
        replacement.clone(),
        GrokWebStatsigSignature::try_new("signature_replacement")?,
        NOW_MS + 200,
        NOW_MS + 1,
    )?;
    assert!(cache.get(&expired, NOW_MS + 1)?.is_none());
    assert_eq!(
        cache
            .get(&replacement, NOW_MS + 1)?
            .as_ref()
            .map(GrokWebStatsigSignature::as_str),
        Some("signature_replacement")
    );
    let live = key("GET", "/live", "env-01")?;
    assert_eq!(
        cache.insert(
            live,
            GrokWebStatsigSignature::try_new("signature_live")?,
            NOW_MS + 200,
            NOW_MS + 1,
        ),
        Err(GrokWebStatsigError::CacheCapacityExhausted)
    );
    assert_eq!(
        GrokWebStatsigSignatureKey::try_new("Get", "/valid", "env-01"),
        Err(GrokWebStatsigError::InvalidMethod)
    );
    assert_eq!(
        GrokWebStatsigSignatureKey::try_new("GET", "//host-like", "env-01"),
        Err(GrokWebStatsigError::InvalidPath)
    );
    assert_eq!(
        GrokWebStatsigSignature::try_new("bad\r\nheader"),
        Err(GrokWebStatsigError::InvalidSignature)
    );
    assert!(matches!(
        GrokWebStatsigSignatureCache::try_new(0),
        Err(GrokWebStatsigError::InvalidCacheCapacity)
    ));
    Ok(())
}

#[test]
fn signer_requires_https_exact_allowlist_and_full_redirect_readmission_without_sending()
-> TestResult {
    let resolver = Arc::new(StaticResolver::new([IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))]));
    let boundary = GrokWebStatsigSignerBoundary::new(
        policy(
            [EgressScheme::Https],
            ["signer.example.test", "other.example.test"],
            RedirectPolicy::SameOrigin { max_redirects: 1 },
        )?,
        resolver,
    );
    let signer = boundary.admit_initial("https://signer.example.test/signer")?;
    assert_eq!(signer.scheme(), EgressScheme::Https);
    assert_eq!(signer.host(), "signer.example.test");
    assert_eq!(signer.port(), 443);
    let same_origin = boundary.admit_redirect(&signer, "/next", 0)?;
    assert_eq!(same_origin.host(), "signer.example.test");
    assert_eq!(
        boundary.admit_redirect(&signer, "https://other.example.test/next", 0),
        Err(GrokWebStatsigError::SignerAdmissionRejected)
    );
    assert_eq!(
        boundary.admit_initial("https://untrusted.example.test/signer"),
        Err(GrokWebStatsigError::SignerAdmissionRejected)
    );

    let http_boundary = GrokWebStatsigSignerBoundary::new(
        policy(
            [EgressScheme::Http],
            ["signer.example.test"],
            RedirectPolicy::Deny,
        )?,
        Arc::new(StaticResolver::new([IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])),
    );
    assert_eq!(
        http_boundary.admit_initial("http://signer.example.test/signer"),
        Err(GrokWebStatsigError::SignerMustUseHttps)
    );
    let diagnostic = format!("{boundary:?} {signer:?}");
    assert!(!diagnostic.contains("signer.example.test"));
    assert!(diagnostic.contains("<redacted"));
    Ok(())
}

fn key(
    method: &str,
    path: &str,
    environment_version: &str,
) -> Result<GrokWebStatsigSignatureKey, GrokWebStatsigError> {
    GrokWebStatsigSignatureKey::try_new(method, path, environment_version)
}

fn policy<const N: usize, const M: usize>(
    schemes: [EgressScheme; N],
    hosts: [&str; M],
    redirect_policy: RedirectPolicy,
) -> Result<EgressPolicy, Box<dyn Error>> {
    let allowed_ports = if schemes.contains(&EgressScheme::Http) {
        BTreeSet::from([80])
    } else {
        BTreeSet::from([443])
    };
    Ok(EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("p9-05-statsig-policy")?,
        name: "P9-05 Statsig test policy".to_owned(),
        allowed_schemes: BTreeSet::from(schemes),
        allowed_hosts: hosts
            .into_iter()
            .map(EgressHost::try_new)
            .collect::<Result<BTreeSet<_>, _>>()?,
        allowed_ports,
        allowed_cidrs: BTreeSet::new(),
        redirect_policy,
    })?)
}

struct StaticResolver {
    addresses: Vec<IpAddr>,
}

impl StaticResolver {
    fn new(addresses: impl IntoIterator<Item = IpAddr>) -> Self {
        Self {
            addresses: addresses.into_iter().collect(),
        }
    }
}

impl EgressDnsResolver for StaticResolver {
    fn resolve(&self, _: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(self.addresses.clone())
    }
}
