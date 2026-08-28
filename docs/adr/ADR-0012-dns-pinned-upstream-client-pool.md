# ADR-0012: DNS-pinned bounded upstream client pool

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-20` |
| Task / Matrix / Contract references | `P3-02`; `C16`, `K03-K06`, `L06`; [BC-UPSTREAM-001](../contracts/BC-UPSTREAM-001-dns-pinned-upstream-client-pool.md) |

## Context

P3-01 constructs an exact OpenAI-compatible Responses target, headers, and body but deliberately
does not open a socket. P2-09 provides a per-attempt `AdmittedEgressTarget` whose DNS answer is
fully checked and must be used for the later dial without a second upstream-name resolution. P3
needs a process-shared client pool that preserves that property while separating connect, response
head, body-idle, and total deadlines.

The P1/P2 runtime has no proxy or timeout persistence fields, and P3-04 has not implemented a
Credential lease. This task therefore needs a transport-neutral, injected runtime profile rather
than a new database entity or a Client/Credential configuration shortcut. It also must not allow an
HTTP client library to perform invisible request retries or redirects before P3-06 can apply the
FirstSemanticEvent rules.

## Decision

- `gateway-upstream` owns `UpstreamClientPool`, `UpstreamTransportProfile`, `UpstreamTimeouts`,
  `UpstreamProxy`, `UpstreamHttpRequest`, and `UpstreamHttpResponse`. The pool only accepts an
  already admitted target, never a raw URL. Its bounded concurrent cache key contains the complete
  profile, scheme/Host/port, and sorted admitted address set; a direct request, a SOCKS request,
  or a new DNS answer cannot reuse another transport identity's client.
- Each cached `reqwest` client has environment/system proxy discovery disabled, automatic redirects
  disabled, and its built-in retry policy set to `never`. It receives only the P2-admitted DNS
  addresses through a static resolver override. A `reqwest` connection pool is consequently shared
  for equal transport identities, but cache capacity and each origin's retained-idle connection
  count are both finite.
- Connect timeout is configured on the connector; response-head/TTFB is bounded around dispatch;
  read/response-idle is bounded around every body chunk; total begins before dispatch and remains
  enforced while the body is consumed. All network/deadline failures map to
  `EgressUnavailable/Egress`; P3-02 deliberately leaves HTTP status and body semantics raw.
- Direct and `socks5://` profiles are supported. SOCKS5 uses local resolution, so the proxy receives
  an admitted IP address rather than the upstream Host. HTTP, HTTPS, `socks5h`, credentials in a
  proxy URL, and non-root proxy paths/query/fragments fail closed: those forms either let the proxy
  resolve the upstream name itself or need a dedicated proxy-credential lifecycle that does not yet
  exist. A later task may add an explicit CONNECT-capable transport only after it proves the same
  address-pinning invariant.
- `OpenAiResponsesOutboundRequest::into_transport_request` consumes P3-01 output only when the
  admitted target exactly equals the encoded endpoint. It rejects a merely allowlisted but different
  URL before the Authorization value/body reaches the client.

## Consequences

P3-06 can receive an unclassified response head and pull bounded raw chunks without reconstructing
the target or reintroducing a system proxy, redirect, or client retry. P3-04 will later supply the
request-scoped credential that P3-01 already requires. The profile is runtime-injected only: P2
SQLite, RouteSnapshot, secret storage, candidate scheduling, Credential leases, health, status-code
classification, response/SSE decoding, redirects, and retry/failover behavior remain unchanged.

Clash users can use its local SOCKS5 listener with a `socks5://` profile; a mixed HTTP proxy is not
silently treated as equivalent because it would break P2's DNS-pinning guarantee.

## Alternatives considered

- One fresh `reqwest::Client` per request was rejected because it discards TCP/TLS reuse and creates
  unbounded setup overhead.
- A single client with a mutable Host-to-address map was rejected because concurrent attempts for
  the same Host could overwrite each other's admitted DNS answers.
- Letting `reqwest` use `HTTP_PROXY`, `HTTPS_PROXY`, PAC, or the operating-system proxy was rejected
  because a Direct profile must be deterministic and isolated from ambient process state.
- Accepting HTTP/HTTPS proxies or `socks5h` was rejected for this task because those proxy paths can
  resolve the upstream Host after P2 admission. Calling that behavior DNS-pinned would be false.
- Allowing the client library's default protocol-NACK retry or redirect follow was rejected because
  P3-06 must decide whether another attempt is legal before and after the first semantic event.

## Validation and rollback

Loopback mock tests prove DNS override use and direct connection reuse, isolated Direct/SOCKS5
profiles, SOCKS5 receipt of the admitted IP/port, connect handshake, TTFB, body-idle, and total
deadline failures, and disabled automatic redirect/retry behavior.
They also cover proxy/header redaction, invalid timeout/proxy configuration, and exact P3-01 target
handoff. `cargo clippy`, crate-boundary, source-policy, documentation, Secret, Fast, and Full gates
cover the expanded dependency graph. Rolling back removes only the new runtime transport types and
their Provider handoff; no database, Snapshot, deployed Endpoint, Client Key, or Credential changes.
