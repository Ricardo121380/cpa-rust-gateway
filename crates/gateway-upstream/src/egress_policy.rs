//! Immutable outbound URL admission with DNS-rebinding-resistant address pinning.
//!
//! This module validates an operator-configured target before a later HTTP client is allowed to
//! create a connection. It deliberately does not own a socket, proxy, TLS client, or redirect
//! loop. A caller receives the approved DNS addresses and must dial one of them without resolving
//! the hostname again for the same attempt.

use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs},
};

use gateway_core::{EgressPolicyId, ErrorScope, GatewayError, GatewayErrorCode};
use url::{Host, Url};

const MAX_REDIRECTS: u8 = 10;

/// A supported outbound URL scheme.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EgressScheme {
    /// Unencrypted HTTP, allowed only when an operator explicitly chooses it.
    Http,
    /// TLS-protected HTTPS, the normal production default.
    Https,
}

impl EgressScheme {
    /// Parses the canonical lower-case scheme label admitted by this policy.
    ///
    /// # Errors
    ///
    /// Returns [`EgressAdmissionErrorCode::UnsupportedScheme`] for every scheme other than
    /// `http` or `https`.
    pub fn try_from_url_scheme(value: &str) -> Result<Self, EgressAdmissionError> {
        match value {
            "http" => Ok(Self::Http),
            "https" => Ok(Self::Https),
            _ => Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::UnsupportedScheme,
            )),
        }
    }

    /// Returns the exact URL scheme representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

impl fmt::Display for EgressScheme {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One exact canonical Host rule, represented as a DNS name or IP literal.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EgressHost {
    /// A canonical exact DNS name. Wildcards and suffix matching are not supported.
    Domain(String),
    /// A literal IPv4 or IPv6 destination that must be explicitly allowlisted.
    Ip(IpAddr),
}

impl EgressHost {
    /// Parses one canonical exact Host rule.
    ///
    /// # Errors
    ///
    /// Returns [`EgressCidrError::Host`] when the text is not a DNS name or IP literal
    /// understood by the URL parser.
    pub fn try_new(value: &str) -> Result<Self, EgressCidrError> {
        let ip_literal = value
            .strip_prefix('[')
            .and_then(|without_opening_bracket| without_opening_bracket.strip_suffix(']'))
            .unwrap_or(value);
        if let Ok(address) = ip_literal.parse::<IpAddr>() {
            return Ok(Self::Ip(address));
        }
        match Host::parse(value).map_err(|_| EgressCidrError::Host)? {
            Host::Domain(domain) => Ok(Self::Domain(domain)),
            Host::Ipv4(address) => Ok(Self::Ip(IpAddr::V4(address))),
            Host::Ipv6(address) => Ok(Self::Ip(IpAddr::V6(address))),
        }
    }

    /// Returns the canonical Host text without a port, user-info, path, or query component.
    #[must_use]
    pub fn as_str(&self) -> String {
        match self {
            Self::Domain(domain) => domain.clone(),
            Self::Ip(address) => address.to_string(),
        }
    }

    fn literal_address(&self) -> Option<IpAddr> {
        match self {
            Self::Domain(_) => None,
            Self::Ip(address) => Some(*address),
        }
    }

    fn domain(&self) -> Option<&str> {
        match self {
            Self::Domain(domain) => Some(domain),
            Self::Ip(_) => None,
        }
    }
}

impl fmt::Display for EgressHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(domain) => formatter.write_str(domain),
            Self::Ip(address) => write!(formatter, "{address}"),
        }
    }
}

/// One normalized IPv4 or IPv6 CIDR range used by an `EgressPolicy`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EgressCidr {
    network: IpAddr,
    prefix_length: u8,
}

impl EgressCidr {
    /// Validates and normalizes one CIDR from an address and prefix length.
    ///
    /// # Errors
    ///
    /// Returns [`EgressCidrError::PrefixLength`] when `prefix_length` is outside the
    /// address family's bit width.
    pub fn try_new(address: IpAddr, prefix_length: u8) -> Result<Self, EgressCidrError> {
        let network = match address {
            IpAddr::V4(address) => {
                if prefix_length > 32 {
                    return Err(EgressCidrError::PrefixLength);
                }
                IpAddr::V4(Ipv4Addr::from(mask_v4(address, prefix_length)))
            }
            IpAddr::V6(address) => {
                if prefix_length > 128 {
                    return Err(EgressCidrError::PrefixLength);
                }
                IpAddr::V6(Ipv6Addr::from(mask_v6(address, prefix_length)))
            }
        };
        Ok(Self {
            network,
            prefix_length,
        })
    }

    /// Parses `address/prefix` CIDR text without retaining the original input on failure.
    ///
    /// # Errors
    ///
    /// Returns [`EgressCidrError::Format`] for invalid address or delimiter syntax, and
    /// [`EgressCidrError::PrefixLength`] for an out-of-range prefix length.
    pub fn try_parse(value: &str) -> Result<Self, EgressCidrError> {
        let Some((address, prefix_length)) = value.split_once('/') else {
            return Err(EgressCidrError::Format);
        };
        if prefix_length.contains('/') {
            return Err(EgressCidrError::Format);
        }
        let address = address
            .parse::<IpAddr>()
            .map_err(|_| EgressCidrError::Format)?;
        let prefix_length = prefix_length
            .parse::<u8>()
            .map_err(|_| EgressCidrError::Format)?;
        Self::try_new(address, prefix_length)
    }

    /// Returns the normalized network address.
    #[must_use]
    pub const fn network(&self) -> IpAddr {
        self.network
    }

    /// Returns the validated prefix length.
    #[must_use]
    pub const fn prefix_length(&self) -> u8 {
        self.prefix_length
    }

    /// Returns whether this CIDR contains `address`.
    #[must_use]
    pub fn contains(&self, address: IpAddr) -> bool {
        match (self.network, address) {
            (IpAddr::V4(network), IpAddr::V4(address)) => {
                mask_v4(address, self.prefix_length) == u32::from_be_bytes(network.octets())
            }
            (IpAddr::V6(network), IpAddr::V6(address)) => {
                mask_v6(address, self.prefix_length) == u128::from_be_bytes(network.octets())
            }
            _ => false,
        }
    }
}

impl fmt::Display for EgressCidr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.network, self.prefix_length)
    }
}

/// Safe parse failures for Host and CIDR configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EgressCidrError {
    /// The supplied CIDR text did not contain one valid address and prefix.
    Format,
    /// The supplied prefix length exceeded the address family's bit width.
    PrefixLength,
    /// The supplied Host was not a URL-parser-recognized DNS name or IP literal.
    Host,
}

impl fmt::Display for EgressCidrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format => formatter.write_str("egress CIDR format is invalid"),
            Self::PrefixLength => formatter.write_str("egress CIDR prefix length is invalid"),
            Self::Host => formatter.write_str("egress Host format is invalid"),
        }
    }
}

impl Error for EgressCidrError {}

/// Redirect behavior allowed after an upstream response supplies a Location header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectPolicy {
    /// Never follow a redirect response.
    Deny,
    /// Follow only a fully re-admitted redirect whose scheme, Host, and port match the source.
    SameOrigin {
        /// Maximum number of redirects that may be followed.
        max_redirects: u8,
    },
    /// Follow a fully re-admitted redirect to any other configured exact Host.
    Revalidate {
        /// Maximum number of redirects that may be followed.
        max_redirects: u8,
    },
}

impl RedirectPolicy {
    fn maximum_redirects(self) -> Option<u8> {
        match self {
            Self::Deny => None,
            Self::SameOrigin { max_redirects } | Self::Revalidate { max_redirects } => {
                Some(max_redirects)
            }
        }
    }
}

/// Complete immutable input for one outbound `EgressPolicy`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EgressPolicyInput {
    /// Stable version-scoped policy identity.
    pub id: EgressPolicyId,
    /// Non-secret operator-facing policy label.
    pub name: String,
    /// Explicitly allowed URL schemes.
    pub allowed_schemes: BTreeSet<EgressScheme>,
    /// Explicitly allowed exact DNS names or IP literals.
    pub allowed_hosts: BTreeSet<EgressHost>,
    /// Explicitly allowed effective destination ports.
    pub allowed_ports: BTreeSet<u16>,
    /// Optional CIDR restriction and narrow private-network exception list.
    pub allowed_cidrs: BTreeSet<EgressCidr>,
    /// Redirect handling and maximum hop limit.
    pub redirect_policy: RedirectPolicy,
}

/// Safe construction failures for an `EgressPolicy`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EgressPolicyErrorCode {
    /// The operator-facing policy name was empty or whitespace only.
    EmptyPolicyName,
    /// The policy allowed no URL scheme.
    EmptyAllowedSchemes,
    /// The policy allowed no exact Host.
    EmptyAllowedHosts,
    /// The policy allowed no effective destination port.
    EmptyAllowedPorts,
    /// One configured port was zero.
    InvalidAllowedPort,
    /// An enabled redirect policy used zero or too many hops.
    InvalidRedirectLimit,
}

/// A safe `EgressPolicy` construction failure without original configuration values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EgressPolicyError {
    code: EgressPolicyErrorCode,
}

impl EgressPolicyError {
    const fn new(code: EgressPolicyErrorCode) -> Self {
        Self { code }
    }

    /// Returns the stable construction failure category.
    #[must_use]
    pub const fn code(self) -> EgressPolicyErrorCode {
        self.code
    }
}

impl fmt::Display for EgressPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = match self.code {
            EgressPolicyErrorCode::EmptyPolicyName => "egress policy name is empty",
            EgressPolicyErrorCode::EmptyAllowedSchemes => "egress policy has no allowed schemes",
            EgressPolicyErrorCode::EmptyAllowedHosts => "egress policy has no allowed Hosts",
            EgressPolicyErrorCode::EmptyAllowedPorts => "egress policy has no allowed ports",
            EgressPolicyErrorCode::InvalidAllowedPort => "egress policy contains an invalid port",
            EgressPolicyErrorCode::InvalidRedirectLimit => {
                "egress policy redirect limit is invalid"
            }
        };
        formatter.write_str(description)
    }
}

impl Error for EgressPolicyError {}

/// Safe target-admission failure categories that never retain URL, Host, or DNS diagnostic text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EgressAdmissionErrorCode {
    /// The URL parser rejected the target or redirect Location syntax.
    InvalidUrl,
    /// The URL had a scheme other than HTTP or HTTPS.
    UnsupportedScheme,
    /// The URL carried user-info, which can hide a credential in configuration text.
    UrlContainsUserInfo,
    /// The URL did not contain a hierarchical Host.
    MissingHost,
    /// The canonical Host did not match the policy's exact allowlist.
    HostNotAllowed,
    /// The effective port did not match the policy's allowlist.
    PortNotAllowed,
    /// DNS lookup was unavailable or otherwise failed.
    DnsUnavailable,
    /// DNS lookup returned no address.
    DnsReturnedNoAddresses,
    /// A resolved address was outside the configured CIDR restriction.
    AddressOutsideAllowedCidr,
    /// A resolved address was denied by the private/special-address policy.
    AddressDenied,
    /// Redirect following was disabled for this policy.
    RedirectDisabled,
    /// The caller attempted to exceed the configured redirect-hop limit.
    RedirectLimitExceeded,
    /// A same-origin redirect changed scheme, Host, or effective port.
    RedirectOriginMismatch,
}

/// A safe target-admission error suitable for classification at a transport boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EgressAdmissionError {
    code: EgressAdmissionErrorCode,
}

impl EgressAdmissionError {
    const fn new(code: EgressAdmissionErrorCode) -> Self {
        Self { code }
    }

    /// Returns the stable target-admission failure category.
    #[must_use]
    pub const fn code(self) -> EgressAdmissionErrorCode {
        self.code
    }

    /// Maps this policy result to the existing safe gateway error taxonomy.
    #[must_use]
    pub const fn gateway_error(self) -> GatewayError {
        match self.code {
            EgressAdmissionErrorCode::DnsUnavailable
            | EgressAdmissionErrorCode::DnsReturnedNoAddresses => {
                GatewayError::new(GatewayErrorCode::EgressUnavailable, ErrorScope::Egress)
            }
            EgressAdmissionErrorCode::InvalidUrl
            | EgressAdmissionErrorCode::UnsupportedScheme
            | EgressAdmissionErrorCode::UrlContainsUserInfo
            | EgressAdmissionErrorCode::MissingHost
            | EgressAdmissionErrorCode::HostNotAllowed
            | EgressAdmissionErrorCode::PortNotAllowed
            | EgressAdmissionErrorCode::AddressOutsideAllowedCidr
            | EgressAdmissionErrorCode::AddressDenied
            | EgressAdmissionErrorCode::RedirectDisabled
            | EgressAdmissionErrorCode::RedirectLimitExceeded
            | EgressAdmissionErrorCode::RedirectOriginMismatch => {
                GatewayError::new(GatewayErrorCode::EgressRejected, ErrorScope::Egress)
            }
        }
    }
}

impl fmt::Display for EgressAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = match self.code {
            EgressAdmissionErrorCode::InvalidUrl => "egress URL is invalid",
            EgressAdmissionErrorCode::UnsupportedScheme => "egress URL scheme is not supported",
            EgressAdmissionErrorCode::UrlContainsUserInfo => {
                "egress URL must not contain user-info"
            }
            EgressAdmissionErrorCode::MissingHost => "egress URL does not contain a Host",
            EgressAdmissionErrorCode::HostNotAllowed => "egress Host is not allowed",
            EgressAdmissionErrorCode::PortNotAllowed => "egress port is not allowed",
            EgressAdmissionErrorCode::DnsUnavailable => "egress DNS is unavailable",
            EgressAdmissionErrorCode::DnsReturnedNoAddresses => "egress DNS returned no addresses",
            EgressAdmissionErrorCode::AddressOutsideAllowedCidr => {
                "egress address is outside the allowed CIDR range"
            }
            EgressAdmissionErrorCode::AddressDenied => "egress address is denied",
            EgressAdmissionErrorCode::RedirectDisabled => "egress redirects are disabled",
            EgressAdmissionErrorCode::RedirectLimitExceeded => "egress redirect limit was exceeded",
            EgressAdmissionErrorCode::RedirectOriginMismatch => {
                "egress redirect changed its required origin"
            }
        };
        formatter.write_str(description)
    }
}

impl Error for EgressAdmissionError {}

/// An opaque DNS-resolution failure without resolver diagnostics or target text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EgressDnsError;

impl fmt::Display for EgressDnsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("egress DNS resolution failed")
    }
}

impl Error for EgressDnsError {}

/// Resolves one exact DNS Host for a single policy-admission attempt.
pub trait EgressDnsResolver: Send + Sync {
    /// Resolves the canonical Host to every candidate IP address for this one attempt.
    ///
    /// # Errors
    ///
    /// Returns [`EgressDnsError`] without retaining DNS server diagnostics or target text.
    fn resolve(&self, host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError>;
}

/// The normal synchronous resolver used by a later connection-owning component.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemEgressDnsResolver;

impl EgressDnsResolver for SystemEgressDnsResolver {
    fn resolve(&self, host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
        if let Some(address) = host.literal_address() {
            return Ok(vec![address]);
        }
        let Some(domain) = host.domain() else {
            return Err(EgressDnsError);
        };
        let addresses = (domain, 0)
            .to_socket_addrs()
            .map_err(|_| EgressDnsError)?
            .map(|address| address.ip())
            .collect::<BTreeSet<_>>();
        Ok(addresses.into_iter().collect())
    }
}

/// A complete immutable outbound policy with no database, socket, or HTTP-client state.
#[derive(Clone, Eq, PartialEq)]
pub struct EgressPolicy {
    id: EgressPolicyId,
    name: String,
    allowed_schemes: BTreeSet<EgressScheme>,
    allowed_hosts: BTreeSet<EgressHost>,
    allowed_ports: BTreeSet<u16>,
    allowed_cidrs: BTreeSet<EgressCidr>,
    redirect_policy: RedirectPolicy,
}

impl EgressPolicy {
    /// Validates one complete immutable `EgressPolicy`.
    ///
    /// # Errors
    ///
    /// Returns [`EgressPolicyError`] when a mandatory allowlist is empty, a port is zero, or an
    /// enabled redirect policy lacks a finite positive hop limit.
    pub fn try_new(input: EgressPolicyInput) -> Result<Self, EgressPolicyError> {
        if input.name.trim().is_empty() {
            return Err(EgressPolicyError::new(
                EgressPolicyErrorCode::EmptyPolicyName,
            ));
        }
        if input.allowed_schemes.is_empty() {
            return Err(EgressPolicyError::new(
                EgressPolicyErrorCode::EmptyAllowedSchemes,
            ));
        }
        if input.allowed_hosts.is_empty() {
            return Err(EgressPolicyError::new(
                EgressPolicyErrorCode::EmptyAllowedHosts,
            ));
        }
        if input.allowed_ports.is_empty() {
            return Err(EgressPolicyError::new(
                EgressPolicyErrorCode::EmptyAllowedPorts,
            ));
        }
        if input.allowed_ports.contains(&0) {
            return Err(EgressPolicyError::new(
                EgressPolicyErrorCode::InvalidAllowedPort,
            ));
        }
        if let Some(max_redirects) = input.redirect_policy.maximum_redirects()
            && (max_redirects == 0 || max_redirects > MAX_REDIRECTS)
        {
            return Err(EgressPolicyError::new(
                EgressPolicyErrorCode::InvalidRedirectLimit,
            ));
        }

        Ok(Self {
            id: input.id,
            name: input.name,
            allowed_schemes: input.allowed_schemes,
            allowed_hosts: input.allowed_hosts,
            allowed_ports: input.allowed_ports,
            allowed_cidrs: input.allowed_cidrs,
            redirect_policy: input.redirect_policy,
        })
    }

    /// Returns the stable version-scoped policy identity.
    #[must_use]
    pub fn id(&self) -> &EgressPolicyId {
        &self.id
    }

    /// Returns the non-secret policy label.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Checks URL syntax, scheme, exact Host, and port without performing DNS.
    ///
    /// This is appropriate for management-time static validation. A connection-owning caller must
    /// still call [`Self::admit_url`] immediately before it dials.
    ///
    /// # Errors
    ///
    /// Returns a safe [`EgressAdmissionError`] without retaining raw URL text.
    pub fn validate_url_shape(&self, raw_url: &str) -> Result<(), EgressAdmissionError> {
        self.parse_url(raw_url).map(|_| ())
    }

    /// Fully admits one URL and pins the complete approved DNS answer for this attempt.
    ///
    /// # Errors
    ///
    /// Returns a safe [`EgressAdmissionError`] when URL policy, DNS, or any resolved address is
    /// not admitted. Callers must use [`AdmittedEgressTarget::resolved_addresses`] directly for
    /// their later dial rather than resolving its Host again.
    pub fn admit_url(
        &self,
        raw_url: &str,
        resolver: &dyn EgressDnsResolver,
    ) -> Result<AdmittedEgressTarget, EgressAdmissionError> {
        let parsed = self.parse_url(raw_url)?;
        let addresses = match parsed.host.literal_address() {
            Some(address) => BTreeSet::from([address]),
            None => resolver
                .resolve(&parsed.host)
                .map_err(|_| EgressAdmissionError::new(EgressAdmissionErrorCode::DnsUnavailable))?
                .into_iter()
                .collect(),
        };
        if addresses.is_empty() {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::DnsReturnedNoAddresses,
            ));
        }
        for address in &addresses {
            self.validate_address(*address)?;
        }

        Ok(AdmittedEgressTarget {
            url: parsed.url,
            scheme: parsed.scheme,
            host: parsed.host,
            port: parsed.port,
            resolved_addresses: addresses.into_iter().collect(),
        })
    }

    /// Resolves and fully validates one redirect Location relative to the current admitted URL.
    ///
    /// `followed_redirects` is the number already followed. A caller supplies zero before the
    /// first follow and increments it after each successful result.
    ///
    /// # Errors
    ///
    /// Returns [`EgressAdmissionErrorCode::RedirectDisabled`] for the default policy,
    /// [`EgressAdmissionErrorCode::RedirectLimitExceeded`] when the bounded hop count is spent,
    /// or another safe admission error when the Location cannot pass full validation.
    pub fn admit_redirect(
        &self,
        current: &AdmittedEgressTarget,
        location: &str,
        followed_redirects: u8,
        resolver: &dyn EgressDnsResolver,
    ) -> Result<AdmittedEgressTarget, EgressAdmissionError> {
        let Some(max_redirects) = self.redirect_policy.maximum_redirects() else {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::RedirectDisabled,
            ));
        };
        if followed_redirects >= max_redirects {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::RedirectLimitExceeded,
            ));
        }
        let redirected_url = current
            .url
            .join(location)
            .map_err(|_| EgressAdmissionError::new(EgressAdmissionErrorCode::InvalidUrl))?;
        let redirected = self.admit_url(redirected_url.as_str(), resolver)?;
        if matches!(self.redirect_policy, RedirectPolicy::SameOrigin { .. })
            && !current.has_same_origin(&redirected)
        {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::RedirectOriginMismatch,
            ));
        }
        Ok(redirected)
    }

    fn parse_url(&self, raw_url: &str) -> Result<ParsedEgressUrl, EgressAdmissionError> {
        let url = Url::parse(raw_url)
            .map_err(|_| EgressAdmissionError::new(EgressAdmissionErrorCode::InvalidUrl))?;
        if url.cannot_be_a_base() {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::InvalidUrl,
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::UrlContainsUserInfo,
            ));
        }
        let scheme = EgressScheme::try_from_url_scheme(url.scheme())?;
        if !self.allowed_schemes.contains(&scheme) {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::UnsupportedScheme,
            ));
        }
        let host_text = url
            .host_str()
            .ok_or_else(|| EgressAdmissionError::new(EgressAdmissionErrorCode::MissingHost))?;
        let host = EgressHost::try_new(host_text)
            .map_err(|_| EgressAdmissionError::new(EgressAdmissionErrorCode::InvalidUrl))?;
        if !self.allowed_hosts.contains(&host) {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::HostNotAllowed,
            ));
        }
        let port = url
            .port_or_known_default()
            .ok_or_else(|| EgressAdmissionError::new(EgressAdmissionErrorCode::PortNotAllowed))?;
        if !self.allowed_ports.contains(&port) {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::PortNotAllowed,
            ));
        }

        Ok(ParsedEgressUrl {
            url,
            scheme,
            host,
            port,
        })
    }

    fn validate_address(&self, address: IpAddr) -> Result<(), EgressAdmissionError> {
        if is_metadata_address(address) {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::AddressDenied,
            ));
        }
        let is_in_allowed_cidr = self.allowed_cidrs.is_empty()
            || self
                .allowed_cidrs
                .iter()
                .any(|allowed_cidr| allowed_cidr.contains(address));
        if !is_in_allowed_cidr {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::AddressOutsideAllowedCidr,
            ));
        }
        if is_default_denied_address(address)
            && !self.has_narrow_private_or_local_exception_for(address)
        {
            return Err(EgressAdmissionError::new(
                EgressAdmissionErrorCode::AddressDenied,
            ));
        }
        Ok(())
    }

    fn has_narrow_private_or_local_exception_for(&self, address: IpAddr) -> bool {
        self.allowed_cidrs.iter().any(|allowed_cidr| {
            allowed_cidr.contains(address) && is_narrow_private_or_local_exception(*allowed_cidr)
        })
    }
}

impl fmt::Debug for EgressPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EgressPolicy")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("allowed_schemes", &self.allowed_schemes)
            .field("allowed_hosts", &self.allowed_hosts)
            .field("allowed_ports", &self.allowed_ports)
            .field("allowed_cidrs", &self.allowed_cidrs)
            .field("redirect_policy", &self.redirect_policy)
            .finish()
    }
}

struct ParsedEgressUrl {
    url: Url,
    scheme: EgressScheme,
    host: EgressHost,
    port: u16,
}

/// One fully validated outbound target whose DNS result is pinned for a single dial attempt.
#[derive(Clone, Eq, PartialEq)]
pub struct AdmittedEgressTarget {
    url: Url,
    scheme: EgressScheme,
    host: EgressHost,
    port: u16,
    resolved_addresses: Vec<IpAddr>,
}

impl AdmittedEgressTarget {
    /// Returns the fully parsed request URL for a later transport implementation.
    ///
    /// Callers must not log this URL because a future redirect may carry a sensitive query string.
    #[must_use]
    pub fn request_url(&self) -> &Url {
        &self.url
    }

    /// Returns the admitted URL scheme.
    #[must_use]
    pub const fn scheme(&self) -> EgressScheme {
        self.scheme
    }

    /// Returns the admitted canonical Host.
    #[must_use]
    pub fn host(&self) -> &EgressHost {
        &self.host
    }

    /// Returns the admitted effective destination port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Returns the sorted, fully validated DNS answer pinned for this attempt.
    #[must_use]
    pub fn resolved_addresses(&self) -> &[IpAddr] {
        &self.resolved_addresses
    }

    fn has_same_origin(&self, other: &Self) -> bool {
        self.scheme == other.scheme && self.host == other.host && self.port == other.port
    }
}

impl fmt::Debug for AdmittedEgressTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedEgressTarget")
            .field("scheme", &self.scheme)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("resolved_addresses", &self.resolved_addresses)
            .finish_non_exhaustive()
    }
}

fn mask_v4(address: Ipv4Addr, prefix_length: u8) -> u32 {
    let value = u32::from_be_bytes(address.octets());
    if prefix_length == 0 {
        0
    } else {
        value & (u32::MAX << u32::from(32 - prefix_length))
    }
}

fn mask_v6(address: Ipv6Addr, prefix_length: u8) -> u128 {
    let value = u128::from_be_bytes(address.octets());
    if prefix_length == 0 {
        0
    } else {
        value & (u128::MAX << u32::from(128 - prefix_length))
    }
}

fn is_default_denied_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_default_denied_v4(address),
        IpAddr::V6(address) => is_default_denied_v6(address),
    }
}

fn is_metadata_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let value = u32::from_be_bytes(address.octets());
            METADATA_V4.contains(&value)
        }
        IpAddr::V6(address) => {
            let value = u128::from_be_bytes(address.octets());
            METADATA_V6.contains(&value)
        }
    }
}

fn is_default_denied_v4(address: Ipv4Addr) -> bool {
    let value = u32::from_be_bytes(address.octets());
    DEFAULT_DENIED_V4
        .iter()
        .any(|(network, prefix_length)| matches_v4_prefix(value, *network, *prefix_length))
}

fn is_default_denied_v6(address: Ipv6Addr) -> bool {
    let value = u128::from_be_bytes(address.octets());
    let is_ipv4_mapped = value >> 32 == u128::from(u16::MAX);
    is_ipv4_mapped
        || DEFAULT_DENIED_V6
            .iter()
            .any(|(network, prefix_length)| matches_v6_prefix(value, *network, *prefix_length))
}

fn is_narrow_private_or_local_exception(cidr: EgressCidr) -> bool {
    match cidr.network() {
        IpAddr::V4(network) => {
            let network = u32::from_be_bytes(network.octets());
            EXCEPTIONABLE_PRIVATE_OR_LOCAL_V4.iter().any(
                |(private_or_local_network, private_or_local_prefix)| {
                    cidr.prefix_length() >= *private_or_local_prefix
                        && matches_v4_prefix(
                            network,
                            *private_or_local_network,
                            *private_or_local_prefix,
                        )
                },
            )
        }
        IpAddr::V6(network) => {
            let network = u128::from_be_bytes(network.octets());
            EXCEPTIONABLE_PRIVATE_OR_LOCAL_V6.iter().any(
                |(private_or_local_network, private_or_local_prefix)| {
                    cidr.prefix_length() >= *private_or_local_prefix
                        && matches_v6_prefix(
                            network,
                            *private_or_local_network,
                            *private_or_local_prefix,
                        )
                },
            )
        }
    }
}

fn matches_v4_prefix(value: u32, network: u32, prefix_length: u8) -> bool {
    if prefix_length == 0 {
        true
    } else {
        let mask = u32::MAX << u32::from(32 - prefix_length);
        value & mask == network & mask
    }
}

fn matches_v6_prefix(value: u128, network: u128, prefix_length: u8) -> bool {
    if prefix_length == 0 {
        true
    } else {
        let mask = u128::MAX << u32::from(128 - prefix_length);
        value & mask == network & mask
    }
}

const DEFAULT_DENIED_V4: &[(u32, u8)] = &[
    (0x0000_0000, 8),
    (0x0A00_0000, 8),
    (0x6440_0000, 10),
    (0x7F00_0000, 8),
    (0xA9FE_0000, 16),
    (0xAC10_0000, 12),
    (0xC000_0000, 24),
    (0xC000_0200, 24),
    (0xC0A8_0000, 16),
    (0xC058_6300, 24),
    (0xC612_0000, 15),
    (0xC633_6400, 24),
    (0xCB00_7100, 24),
    (0xE000_0000, 4),
    (0xF000_0000, 4),
];

const DEFAULT_DENIED_V6: &[(u128, u8)] = &[
    (0x0000_0000_0000_0000_0000_0000_0000_0000, 96),
    (0x0100_0000_0000_0000_0000_0000_0000_0000, 64),
    (0xFC00_0000_0000_0000_0000_0000_0000_0000, 7),
    (0xFE80_0000_0000_0000_0000_0000_0000_0000, 10),
    (0xFF00_0000_0000_0000_0000_0000_0000_0000, 8),
    (0x2001_0000_0000_0000_0000_0000_0000_0000, 32),
    (0x2001_0010_0000_0000_0000_0000_0000_0000, 28),
    (0x2001_0020_0000_0000_0000_0000_0000_0000, 28),
    (0x2001_0DB8_0000_0000_0000_0000_0000_0000, 32),
    (0x0064_FF9B_0000_0000_0000_0000_0000_0000, 96),
    (0x0064_FF9B_0001_0000_0000_0000_0000_0000, 48),
    (0x2002_0000_0000_0000_0000_0000_0000_0000, 16),
];

const EXCEPTIONABLE_PRIVATE_OR_LOCAL_V4: &[(u32, u8)] = &[
    (0x0A00_0000, 8),
    (0x7F00_0000, 8),
    (0xAC10_0000, 12),
    (0xC0A8_0000, 16),
];

const EXCEPTIONABLE_PRIVATE_OR_LOCAL_V6: &[(u128, u8)] = &[
    (0x0000_0000_0000_0000_0000_0000_0000_0001, 128),
    (0xFC00_0000_0000_0000_0000_0000_0000_0000, 7),
];

const METADATA_V4: &[u32] = &[0x6464_64C8, 0xA9FE_A9FE];
const METADATA_V6: &[u128] = &[0xFD00_0EC2_0000_0000_0000_0000_0000_0254];

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        error::Error,
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use gateway_core::{EgressPolicyId, ErrorScope, GatewayErrorCode};

    use super::{
        EgressAdmissionError, EgressAdmissionErrorCode, EgressCidr, EgressDnsError,
        EgressDnsResolver, EgressHost, EgressPolicy, EgressPolicyInput, EgressScheme,
        RedirectPolicy,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn exact_scheme_host_and_port_rules_admit_only_the_configured_target() -> TestResult {
        let policy = policy(&["api.example.test"], &[], RedirectPolicy::Deny)?;
        let resolver = StaticResolver::new([IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))]);

        let admitted = policy.admit_url("https://api.example.test/v1", &resolver)?;
        assert_eq!(admitted.scheme(), EgressScheme::Https);
        assert_eq!(admitted.host().as_str(), "api.example.test");
        assert_eq!(admitted.port(), 443);
        assert_eq!(
            admitted.resolved_addresses(),
            &[IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))]
        );
        let query_target = policy.admit_url(
            "https://api.example.test/v1?query=sensitive-query-value",
            &resolver,
        )?;
        assert!(!format!("{query_target:?}").contains("sensitive-query-value"));

        assert_admission_error(
            policy.admit_url("http://api.example.test/v1", &resolver),
            EgressAdmissionErrorCode::UnsupportedScheme,
        )?;
        assert_admission_error(
            policy.admit_url("https://other.example.test/v1", &resolver),
            EgressAdmissionErrorCode::HostNotAllowed,
        )?;
        assert_admission_error(
            policy.admit_url("https://api.example.test:8443/v1", &resolver),
            EgressAdmissionErrorCode::PortNotAllowed,
        )?;
        assert_admission_error(
            policy.admit_url("https://user@api.example.test/v1", &resolver),
            EgressAdmissionErrorCode::UrlContainsUserInfo,
        )?;
        Ok(())
    }

    #[test]
    fn default_policy_rejects_ssrf_ranges_and_mixed_dns_answers() -> TestResult {
        let policy = policy(&["api.example.test"], &[], RedirectPolicy::Deny)?;
        for denied_address in [
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::new(10, 20, 30, 40),
            Ipv4Addr::new(100, 100, 100, 200),
            Ipv4Addr::new(169, 254, 169, 254),
            Ipv4Addr::new(224, 0, 0, 1),
        ] {
            let resolver = StaticResolver::new([IpAddr::V4(denied_address)]);
            assert_admission_error(
                policy.admit_url("https://api.example.test/v1", &resolver),
                EgressAdmissionErrorCode::AddressDenied,
            )?;
        }

        let mixed_resolver = StaticResolver::new([
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        ]);
        assert_admission_error(
            policy.admit_url("https://api.example.test/v1", &mixed_resolver),
            EgressAdmissionErrorCode::AddressDenied,
        )?;
        Ok(())
    }

    #[test]
    fn explicit_narrow_private_cidr_allows_local_testing_but_broad_bypass_stays_denied()
    -> TestResult {
        let local_policy = policy(&["127.0.0.1"], &["127.0.0.1/32"], RedirectPolicy::Deny)?;
        let admitted = local_policy.admit_url("https://127.0.0.1/v1", &NeverResolver)?;
        assert_eq!(
            admitted.resolved_addresses(),
            &[IpAddr::V4(Ipv4Addr::LOCALHOST)]
        );

        let broad_policy = policy(&["api.example.test"], &["0.0.0.0/0"], RedirectPolicy::Deny)?;
        let loopback_resolver = StaticResolver::new([IpAddr::V4(Ipv4Addr::LOCALHOST)]);
        assert_admission_error(
            broad_policy.admit_url("https://api.example.test/v1", &loopback_resolver),
            EgressAdmissionErrorCode::AddressDenied,
        )?;
        Ok(())
    }

    #[test]
    fn only_private_or_local_cidrs_can_override_the_default_special_address_denial() -> TestResult {
        let multicast_policy = policy(
            &["api.example.test"],
            &["224.0.0.0/4"],
            RedirectPolicy::Deny,
        )?;
        let multicast_resolver = StaticResolver::new([IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1))]);
        assert_admission_error(
            multicast_policy.admit_url("https://api.example.test/v1", &multicast_resolver),
            EgressAdmissionErrorCode::AddressDenied,
        )?;

        let private_v6_policy = policy(&["fd00::1"], &["fd00::/8"], RedirectPolicy::Deny)?;
        let private_v6 = private_v6_policy.admit_url("https://[fd00::1]/v1", &NeverResolver)?;
        assert_eq!(
            private_v6.resolved_addresses(),
            &[IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1))]
        );

        let metadata_policy = policy(&["fd00:ec2::254"], &["fc00::/7"], RedirectPolicy::Deny)?;
        assert_admission_error(
            metadata_policy.admit_url("https://[fd00:ec2::254]/v1", &NeverResolver),
            EgressAdmissionErrorCode::AddressDenied,
        )?;
        Ok(())
    }

    #[test]
    fn ipv6_special_ranges_are_rejected_and_public_ip_literals_skip_dns() -> TestResult {
        let default_policy = policy(&["api.example.test"], &[], RedirectPolicy::Deny)?;
        for denied_address in [
            Ipv6Addr::LOCALHOST,
            Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1),
            Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1),
            Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 1),
        ] {
            let resolver = StaticResolver::new([IpAddr::V6(denied_address)]);
            assert_admission_error(
                default_policy.admit_url("https://api.example.test/v1", &resolver),
                EgressAdmissionErrorCode::AddressDenied,
            )?;
        }

        let public_literal_policy = policy(&["2001:4860:4860::8888"], &[], RedirectPolicy::Deny)?;
        let public_literal =
            public_literal_policy.admit_url("https://[2001:4860:4860::8888]/v1", &NeverResolver)?;
        assert_eq!(
            public_literal.resolved_addresses(),
            &[IpAddr::V6(Ipv6Addr::new(
                0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888
            ))]
        );
        Ok(())
    }

    #[test]
    fn every_admission_re_resolves_dns_and_pins_only_the_checked_answer() -> TestResult {
        let policy = policy(&["api.example.test"], &[], RedirectPolicy::Deny)?;
        let resolver = RotatingResolver::new(
            [IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))],
            [IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7))],
        );

        let first = policy.admit_url("https://api.example.test/v1", &resolver)?;
        assert_eq!(
            first.resolved_addresses(),
            &[IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))]
        );
        assert_admission_error(
            policy.admit_url("https://api.example.test/v1", &resolver),
            EgressAdmissionErrorCode::AddressDenied,
        )?;
        assert_eq!(resolver.calls.load(Ordering::Acquire), 2);
        Ok(())
    }

    #[test]
    fn dns_failures_and_empty_answers_fail_closed_without_target_diagnostics() -> TestResult {
        let policy = policy(&["api.example.test"], &[], RedirectPolicy::Deny)?;
        assert_admission_error(
            policy.admit_url("https://api.example.test/v1", &NeverResolver),
            EgressAdmissionErrorCode::DnsUnavailable,
        )?;
        let empty_resolver = StaticResolver::new([]);
        assert_admission_error(
            policy.admit_url("https://api.example.test/v1", &empty_resolver),
            EgressAdmissionErrorCode::DnsReturnedNoAddresses,
        )?;
        Ok(())
    }

    #[test]
    fn redirects_are_disabled_by_default_and_revalidated_when_enabled() -> TestResult {
        let resolver = RotatingResolver::new(
            [IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))],
            [IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7))],
        );
        let deny = policy(&["api.example.test"], &[], RedirectPolicy::Deny)?;
        let current = deny.admit_url("https://api.example.test/v1", &resolver)?;
        assert_admission_error(
            deny.admit_redirect(&current, "/next", 0, &resolver),
            EgressAdmissionErrorCode::RedirectDisabled,
        )?;

        let same_origin = policy(
            &["api.example.test", "other.example.test"],
            &[],
            RedirectPolicy::SameOrigin { max_redirects: 1 },
        )?;
        let public_resolver = StaticResolver::new([IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))]);
        let same_origin_current =
            same_origin.admit_url("https://api.example.test/v1", &public_resolver)?;
        let relative =
            same_origin.admit_redirect(&same_origin_current, "/next", 0, &public_resolver)?;
        assert_eq!(relative.request_url().path(), "/next");
        assert_admission_error(
            same_origin.admit_redirect(
                &same_origin_current,
                "https://other.example.test/next",
                0,
                &public_resolver,
            ),
            EgressAdmissionErrorCode::RedirectOriginMismatch,
        )?;
        assert_admission_error(
            same_origin.admit_redirect(&same_origin_current, "/next", 1, &public_resolver),
            EgressAdmissionErrorCode::RedirectLimitExceeded,
        )?;

        let revalidate = policy(
            &["api.example.test", "other.example.test"],
            &[],
            RedirectPolicy::Revalidate { max_redirects: 1 },
        )?;
        let revalidated_current =
            revalidate.admit_url("https://api.example.test/v1", &public_resolver)?;
        let revalidated = revalidate.admit_redirect(
            &revalidated_current,
            "https://other.example.test/next",
            0,
            &public_resolver,
        )?;
        assert_eq!(revalidated.host().as_str(), "other.example.test");
        Ok(())
    }

    #[test]
    fn admission_errors_preserve_the_egress_error_ownership() {
        let rejected =
            EgressAdmissionError::new(EgressAdmissionErrorCode::AddressDenied).gateway_error();
        let unavailable =
            EgressAdmissionError::new(EgressAdmissionErrorCode::DnsUnavailable).gateway_error();

        assert_eq!(rejected.code(), GatewayErrorCode::EgressRejected);
        assert_eq!(rejected.scope(), ErrorScope::Egress);
        assert_eq!(unavailable.code(), GatewayErrorCode::EgressUnavailable);
        assert_eq!(unavailable.scope(), ErrorScope::Egress);
    }

    fn policy(
        hosts: &[&str],
        cidrs: &[&str],
        redirect_policy: RedirectPolicy,
    ) -> Result<EgressPolicy, Box<dyn Error>> {
        let allowed_hosts = hosts
            .iter()
            .map(|host| EgressHost::try_new(host))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let allowed_cidrs = cidrs
            .iter()
            .map(|cidr| EgressCidr::try_parse(cidr))
            .collect::<Result<BTreeSet<_>, _>>()?;
        Ok(EgressPolicy::try_new(EgressPolicyInput {
            id: EgressPolicyId::try_new("egress-policy-test")?,
            name: "Test policy".to_owned(),
            allowed_schemes: BTreeSet::from([EgressScheme::Https]),
            allowed_hosts,
            allowed_ports: BTreeSet::from([443]),
            allowed_cidrs,
            redirect_policy,
        })?)
    }

    fn assert_admission_error(
        result: Result<super::AdmittedEgressTarget, EgressAdmissionError>,
        expected: EgressAdmissionErrorCode,
    ) -> TestResult {
        match result {
            Ok(_admitted_target) => Err("egress target unexpectedly admitted".into()),
            Err(error) => {
                assert_eq!(error.code(), expected);
                Ok(())
            }
        }
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
        fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
            Ok(self.addresses.clone())
        }
    }

    struct RotatingResolver {
        calls: AtomicUsize,
        first: Vec<IpAddr>,
        later: Vec<IpAddr>,
    }

    impl RotatingResolver {
        fn new(
            first: impl IntoIterator<Item = IpAddr>,
            later: impl IntoIterator<Item = IpAddr>,
        ) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                first: first.into_iter().collect(),
                later: later.into_iter().collect(),
            }
        }
    }

    impl EgressDnsResolver for RotatingResolver {
        fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
            let call = self.calls.fetch_add(1, Ordering::AcqRel);
            if call == 0 {
                Ok(self.first.clone())
            } else {
                Ok(self.later.clone())
            }
        }
    }

    struct NeverResolver;

    impl EgressDnsResolver for NeverResolver {
        fn resolve(&self, _host: &EgressHost) -> Result<Vec<IpAddr>, EgressDnsError> {
            Err(EgressDnsError)
        }
    }
}
