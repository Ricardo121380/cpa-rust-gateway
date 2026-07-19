# P3-02 DNS-pinned upstream client pool report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-02` |
| Matrix / behavior | `C16`, `K03-K06`, `L06`; Behavior 20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-02-upstream-client-pool` |
| Rust | `1.97.1` |
| Result | PASS locally and for the implementation commit in GitHub Fast/Full; verification-record acceptance pending |

## Delivered scope

- Added `gateway-upstream::UpstreamClientPool`, a process-shared, bounded concurrent cache of
  immutable `reqwest` clients. Its key includes the injected transport profile, target scheme/Host/
  port, and the complete P2-09 admitted DNS answer, so Direct/SOCKS5 modes or different DNS answers
  never reuse the same client identity.
- Added typed positive connect/TTFB/idle/total timeouts. Connect is set on the connector; dispatch
  is bounded through response headers; every raw body pull has both idle and original-total
  deadlines. Transport/deadline failures are safe `EgressUnavailable/Egress` values; P3-02 leaves
  status and body semantics unclassified.
- Added `UpstreamHttpRequest` and `UpstreamHttpResponse` as redacted raw transport envelopes. The
  former can only own P2-09's `AdmittedEgressTarget` and rejects Host, connection, framing,
  upgrade, trailer/TE, and proxy-authentication headers. The latter exposes status/header/body data
  only to a later decoder and never renders values in `Debug`.
- Disabled ambient system/environment proxy use, automatic redirect follow, and the HTTP client's
  default protocol-NACK retry. Redirect re-admission and semantic retry remain P3-06 work.
- Implemented isolated Direct and local-DNS `socks5://` profiles. SOCKS5 receives P2's admitted IP
  and port; HTTP/HTTPS proxies, `socks5h`, proxy user-info, queries/fragments, and non-root paths
  fail closed because they could re-resolve the upstream Host or need a future proxy-credential
  lifecycle.
- Added the exact-target bridge from P3-01 `OpenAiResponsesOutboundRequest` into P3-02. A merely
  allowlisted but different admitted URL is rejected before its Authorization header/body reaches
  the client transport.
- Added [ADR-0012](../adr/ADR-0012-dns-pinned-upstream-client-pool.md) and
  [BC-UPSTREAM-001](../contracts/BC-UPSTREAM-001-dns-pinned-upstream-client-pool.md), explicit
  dependency-boundary entries, and traceability/status records.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-upstream -p provider-openai-compatible` | PASS; 21 upstream tests and 6 Provider tests, including pinned DNS, direct reuse, SOCKS5 IP/port isolation, connect/TTFB/idle/total boundaries, redirect/retry suppression, header/proxy redaction, and exact P3-01 handoff |
| `cargo clippy --locked -p gateway-upstream -p provider-openai-compatible --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `ruby scripts/check-crate-boundaries.rb`, `ruby scripts/check-source-policy.rb`, and `ruby scripts/check-doc-links.rb` | PASS |
| `scripts/secret-scan.sh --all` and `git diff --check` | PASS |
| `cargo deny check` and `cargo audit` | PASS; duplicate-version notices are non-blocking and tracked by the existing dependency-policy rule |
| `./scripts/check.sh fast` | PASS |
| `./scripts/check.sh full` | PASS |

## Review

Review passed. The hot path accepts only a typed P2-09 `AdmittedEgressTarget`; no raw URL can enter
the pool, and P3-01's exact-target bridge prevents a different allowed URL from inheriting a
request's Authorization/body. Client identities include profile and address-set information, while
Direct disables ambient proxy discovery and SOCKS5 proves local resolution by receiving the admitted
IP/port in a controlled mock handshake. HTTP/HTTPS and remote-DNS proxy modes fail closed rather
than weakening DNS pinning.

The review also verified that automatic redirect and `reqwest` protocol-NACK retry are disabled,
because P3-06 must own re-admission and the FirstSemanticEvent retry boundary. It verified finite
cache/idle limits, redacted `Debug` forms, transport-controlled Header rejection, bounded raw body
pulls, and the absence of Store, SQLite, RouteSnapshot, Credential lease, scheduler, health,
response/SSE decoder, observability, and real traffic changes.

The initial Rustls TLS feature selection was rejected by `cargo deny` because it introduced ISC
licensed transitive packages outside the frozen allowlist. The final implementation uses the platform
native TLS feature instead; `cargo deny check` and `cargo audit` pass without broadening the
allowlist or adding a license exception.

## Scope and deferred work

P3-02 does not persist timeout/proxy configuration, support proxy credentials, implement an HTTP
CONNECT proxy, select Candidates, lease/decrypt Credentials, maintain health/cooldown/circuit state,
classify status codes, parse non-streaming JSON or SSE, follow an admitted redirect, retry/fail over,
publish `/v1/models`, emit events, or contact a real endpoint. P3-03 through P3-10 own those
behaviors. All tests use loopback synthetic peers and synthetic P2 policy data; no deployed URL,
Client Key, upstream Credential, Authorization value, production body, or production traffic was
read, logged, or committed.

## GitHub CI

GitHub Actions run [29697046789](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29697046789)
passed for implementation commit `0b8d93d`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T17:35:33Z` |
| Full supply-chain gate | PASS; completed `2026-07-19T17:47:16Z` |

This completes P3-02 implementation acceptance. The separate verification-record commit must also
pass the same two jobs before P3-03 can begin.
