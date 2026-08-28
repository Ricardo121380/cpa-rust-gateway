# BC-UPSTREAM-001 DNS-pinned upstream client pool

| Field | Value |
|---|---|
| Contract | `BC-UPSTREAM-001` |
| Task | `P3-02` |
| Status | DONE |
| Domain | Shared outbound HTTP transport, timeouts, and proxy isolation |

## Entry and boundary

`UpstreamClientPool::send` accepts an `UpstreamHttpRequest` that owns one P2-09
`AdmittedEgressTarget` and one injected `UpstreamTransportProfile`. The request cannot carry a raw
target URL. The profile contains only validated connect/TTFB/idle/total deadlines, a finite per-host
idle connection limit, and either Direct or a local-DNS SOCKS5 proxy mode.

P3-01's `OpenAiResponsesOutboundRequest::into_transport_request` is the first Provider handoff. It
requires that the policy-admitted URL exactly equal the P3-01 endpoint URL before copying its fixed
headers and body into the generic transport request.

This contract does not select a Candidate, decrypt/lease a Credential, classify HTTP status, parse a
response body/SSE, follow redirects, retry, fail over, publish `/v1/models`, or emit observability
events. Those behaviors remain P3-03 through P3-10.

## Preconditions

- The caller has applied P2-09 `EgressPolicy::admit_url` immediately before constructing the
  transport request, using the current DNS resolver answer.
- The admitted target has at least one checked address and is retained unchanged through dispatch.
- Connect, TTFB, idle, and total timeouts are positive and individually no greater than total.
- Cached-client capacity and retained idle connections per Host are positive and finite.
- A proxy is either `Direct` or a credential-free `socks5://Host:port/` URL. HTTP, HTTPS,
  `socks5h`, user-info, query/fragment, and non-root paths are invalid here.
- Request headers are valid and unique. Host, connection, framing, upgrade, trailer/TE, and
  proxy-authentication headers are transport-controlled and cannot be supplied by a Provider.

## Required transport behavior

| Stage | Required behavior |
|---|---|
| Client identity | Cache by immutable transport profile plus target scheme/Host/port and full admitted address set; cache capacity is bounded. |
| Direct DNS | Override the HTTP client's resolution for a domain with exactly the admitted addresses. No process/system proxy discovery is used. |
| SOCKS5 DNS | Resolve locally through the admitted mapping and send an admitted IP/port to the SOCKS5 proxy. Do not permit a proxy-resolved Host mode. |
| Connect | Bound TCP/TLS connect by `connect`. |
| Response head | Bound dispatch through response headers by `TTFB`; a response is returned without status classification. |
| Body read | Each `next_chunk` waits no longer than `idle` and never past the original `total` deadline. |
| Redirect/retry | Disable automatic redirect and client retry. P3-06 owns re-admission and semantic retry policy. |
| Failure | Connect, TTFB, idle, total, and lower transport failures return `EgressUnavailable/Egress`, with no raw endpoint/proxy/header/body diagnostics. |

## Invariants

- A request never causes a second resolution of an admitted upstream domain. Direct and SOCKS5
  dispatch use only P2-09's address set; an IP literal is dialed as that literal.
- Direct and proxy profiles cannot share a pooled client, even for the same endpoint and address.
- A new admitted DNS answer cannot reuse a client associated with an old answer.
- System proxy settings cannot capture a Direct request. The client sets `no_proxy` and does not
  enable the `system-proxy` dependency feature.
- No target URL, proxy endpoint, Authorization value, request body, or raw response-header value is
  exposed through any new `Debug` implementation or error.
- A timeout/read failure terminally invalidates that raw response object; later body pulls do not
  resume it.
- No global application `Mutex`, SQLite query, Credential lookup, scheduler mutation, or unbounded
  request queue occurs in the transport hot path.

## Error semantics

| Condition | Result |
|---|---|
| Unsafe timeout or proxy configuration | Safe construction error code; no raw configuration value |
| Invalid/duplicate/transport-controlled outbound header | Safe request construction error code; no raw header name/value |
| P3-01 target differs from P2-admitted URL | `EgressRejected/Egress` |
| Connector, proxy, DNS override, TTFB, idle, or total failure | `EgressUnavailable/Egress` |
| HTTP response status/body contents | Preserved as raw transport data; P3-02 makes no Provider/Credential/Route decision |

## Corresponding tests

- `gateway-upstream::upstream_client::tests` uses controlled loopback HTTP/SOCKS5 peers to prove
  admitted-domain-to-local-IP resolution, same-client connection reuse, Direct/SOCKS5 cache
  separation, SOCKS5 IP/port pinning, connect/TTFB/idle/total boundaries, and proxy/header
  redaction/rejection, as well as disabled automatic redirect/retry behavior.
- `provider-openai-compatible::openai_responses::tests::hands_only_the_exact_egress_admitted_request_to_the_shared_transport`
  proves P3-01 output can enter P3-02 only after exact target admission and retains redaction.
