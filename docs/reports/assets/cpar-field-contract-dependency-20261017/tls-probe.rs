//! Synthetic loopback TLS probe of the production DNS-pinned client pool.
#![deny(unsafe_code)]
use gateway_core::EgressPolicyId;
use gateway_upstream::{
    EgressCidr, EgressDnsError, EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput,
    EgressScheme, RedirectPolicy, UpstreamClientPool, UpstreamHttpMethod, UpstreamHttpRequest,
    UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};
use std::{
    collections::BTreeSet,
    error::Error,
    net::{IpAddr, Ipv4Addr},
    num::NonZeroUsize,
    time::Duration,
};

struct LocalResolver;
impl EgressDnsResolver for LocalResolver {
    fn resolve(&self, _: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        Ok(vec![IpAddr::V4(Ipv4Addr::LOCALHOST)])
    }
}

async fn probe(port: u16, host: String, expected_success: bool) -> Result<(), Box<dyn Error>> {
    let policy = EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("synthetic-local-tls")?,
        name: "synthetic local TLS".into(),
        allowed_schemes: BTreeSet::from([EgressScheme::Https]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new(&host)?]),
        allowed_ports: BTreeSet::from([port]),
        allowed_cidrs: BTreeSet::from([EgressCidr::try_new(IpAddr::V4(Ipv4Addr::LOCALHOST), 32)?]),
        redirect_policy: RedirectPolicy::Deny,
    })?;
    let target = policy.admit_url(&format!("https://{host}:{port}/synthetic"), &LocalResolver)?;
    let request = UpstreamHttpRequest::try_new(
        target,
        UpstreamHttpMethod::Post,
        [("content-type".into(), "application/json".into())],
        br"{}".to_vec(),
    )?;
    let timeouts = UpstreamTimeouts::try_new(
        Duration::from_secs(2),
        Duration::from_secs(2),
        Duration::from_secs(2),
        Duration::from_secs(3),
    )?;
    let one = NonZeroUsize::new(1).ok_or("invalid probe bound")?;
    let pool = UpstreamClientPool::new(one);
    let profile = UpstreamTransportProfile::new(timeouts, UpstreamProxy::Direct, one);
    match pool.send(request, &profile).await {
        Ok(mut response) => {
            assert!(expected_success);
            let mut body = Vec::new();
            while let Some(chunk) = response.next_chunk().await? {
                body.extend_from_slice(&chunk);
            }
            assert_eq!(body, br#"{"synthetic":"ok"}"#);
            println!("VERIFIED_TLS_HTTP_SUCCESS");
        }
        Err(error) => {
            assert!(!expected_success);
            assert_eq!(
                error.code(),
                gateway_core::GatewayErrorCode::EgressUnavailable
            );
            println!("VERIFIED_TLS_REJECTION");
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args.get(1).ok_or("port required")?.parse()?;
    let host = args.get(2).ok_or("synthetic host required")?.clone();
    let success = args.get(3).ok_or("expected outcome required")? == "success";
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(probe(port, host, success))
}
