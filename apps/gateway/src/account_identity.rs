//! Fixed-target SSO profile transport using the gateway's Web proxy and Chromium client.
use gateway_core::EgressPolicyId;
use std::{collections::BTreeSet, num::NonZeroUsize, sync::Arc, time::Duration};
type ProviderFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
use gateway_store::account_identity::AccountIdentity;
use gateway_upstream::{
    EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme, RedirectPolicy,
    SystemEgressDnsResolver, UpstreamClientPool, UpstreamHttpMethod, UpstreamHttpRequest,
    UpstreamProxy, UpstreamTimeouts, UpstreamTransportProfile,
};
use provider_grok::{
    GROK_SESSION_IDENTITY_URL, GROK_WEB_PRODUCTION_USER_AGENT, GrokSessionIdentityError,
    GrokSessionIdentityRequest, GrokSessionIdentityTransport, MAX_GROK_SESSION_IDENTITY_BYTES,
    parse_grok_session_identity,
};
use zeroize::Zeroizing;

pub(crate) struct SessionIdentityTransport {
    policy: EgressPolicy,
    pool: UpstreamClientPool,
    profile: UpstreamTransportProfile,
    capacity: Arc<tokio::sync::Semaphore>,
}
impl SessionIdentityTransport {
    pub(crate) fn new(proxy: Option<UpstreamProxy>) -> Result<Self, GrokSessionIdentityError> {
        let err = || GrokSessionIdentityError::Unavailable;
        let policy = EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("grok-session-identity").map_err(|_| err())?,
            name: "Grok session identity".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Https]),
            allowed_hosts: BTreeSet::from([EgressHost::try_new("grok.com").map_err(|_| err())?]),
            allowed_ports: BTreeSet::from([443]),
            allowed_cidrs: BTreeSet::new(),
            redirect_policy: RedirectPolicy::Deny,
        })
        .map_err(|_| err())?;
        let one = NonZeroUsize::new(1).ok_or_else(err)?;
        let timeouts = UpstreamTimeouts::try_new(
            Duration::from_secs(5),
            Duration::from_secs(15),
            Duration::from_secs(15),
            Duration::from_secs(15),
        )
        .map_err(|_| err())?;
        Ok(Self {
            policy,
            pool: UpstreamClientPool::new(NonZeroUsize::new(4).ok_or_else(err)?),
            profile: UpstreamTransportProfile::new(
                timeouts,
                proxy.unwrap_or(UpstreamProxy::Direct),
                one,
            )
            .with_chrome_146_emulation(),
            capacity: Arc::new(tokio::sync::Semaphore::new(4)),
        })
    }
}
impl GrokSessionIdentityTransport for SessionIdentityTransport {
    fn fetch(
        &self,
        request: GrokSessionIdentityRequest,
    ) -> ProviderFuture<'_, Result<AccountIdentity, GrokSessionIdentityError>> {
        let policy = self.policy.clone();
        let pool = self.pool.clone();
        let profile = self.profile.clone();
        let capacity = Arc::clone(&self.capacity);
        Box::pin(async move {
            tokio::time::timeout(Duration::from_secs(15), async move {
                let permit = capacity
                    .try_acquire_owned()
                    .map_err(|_| GrokSessionIdentityError::Unavailable)?;
                let (_permit, target) = tokio::task::spawn_blocking(move || {
                    let target = policy
                        .admit_url(GROK_SESSION_IDENTITY_URL, &SystemEgressDnsResolver)
                        .map_err(|_| GrokSessionIdentityError::Unavailable)?;
                    Ok::<_, GrokSessionIdentityError>((permit, target))
                })
                .await
                .map_err(|_| GrokSessionIdentityError::Unavailable)??;
                let outbound = UpstreamHttpRequest::try_new(
                    target,
                    UpstreamHttpMethod::Get,
                    [
                        ("accept".to_owned(), "application/json".to_owned()),
                        ("cookie".to_owned(), request.cookie().to_owned()),
                        (
                            "user-agent".to_owned(),
                            GROK_WEB_PRODUCTION_USER_AGENT.to_owned(),
                        ),
                        ("referer".to_owned(), "https://grok.com/".to_owned()),
                        ("sec-fetch-site".to_owned(), "same-origin".to_owned()),
                        ("sec-fetch-mode".to_owned(), "cors".to_owned()),
                    ],
                    Vec::new(),
                )
                .map_err(|_| GrokSessionIdentityError::Unavailable)?;
                let mut response = pool
                    .send(outbound, &profile)
                    .await
                    .map_err(|_| GrokSessionIdentityError::Unavailable)?;
                if response.status() == 401 {
                    return Err(GrokSessionIdentityError::Unauthorized);
                }
                if response.status() == 403 {
                    return Err(GrokSessionIdentityError::Forbidden);
                }
                if response.status() != 200 {
                    return Err(GrokSessionIdentityError::Unavailable);
                }
                let mut bytes = Zeroizing::new(Vec::new());
                while let Some(chunk) = response
                    .next_chunk()
                    .await
                    .map_err(|_| GrokSessionIdentityError::Unavailable)?
                {
                    if bytes.len().saturating_add(chunk.len()) > MAX_GROK_SESSION_IDENTITY_BYTES {
                        return Err(GrokSessionIdentityError::InvalidResponse);
                    }
                    bytes.extend_from_slice(&chunk);
                }
                parse_grok_session_identity(&bytes)
            })
            .await
            .unwrap_or(Err(GrokSessionIdentityError::Unavailable))
        })
    }
}
