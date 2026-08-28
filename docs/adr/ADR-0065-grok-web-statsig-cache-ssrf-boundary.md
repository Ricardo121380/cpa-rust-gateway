# ADR-0065: Grok Web Statsig signature cache and signer SSRF boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P9-05` |
| Matrix / Contract | `C29`、`D28`、`E27-E29`; [BC-SEC-003](../contracts/BC-SEC-003-grok-web-statsig-cache-ssrf-boundary.md) |

## Context

Web/Console signer material is sensitive to method, path, and environment version. A broad 403 response must not erase unrelated valid cache entries, and any signer URL or redirect can become an SSRF path if it bypasses P2 exact URL/DNS/CIDR admission. P9 has no authorization to make a real signer request.

## Decision

1. Cache one zeroizing signature only by exact printable upper-case method, absolute path without query/fragment, and opaque environment version. The cache is finite, caller-clocked, does not evict an unexpired different key, and removes only expired entries before a write.
2. Expose only `invalidate_after_403(exact_key)`. It cannot clear a second method, path, environment, account, or the full cache. P9-07 still owns deciding whether observed 403 evidence is WAF/egress/account-related.
3. Use an injected P2 `EgressPolicy` and `EgressDnsResolver` to admit the initial signer target and every redirect. The boundary rejects a non-HTTPS initial URL before resolution, retains no raw URL/DNS diagnostic, and wraps the admitted target without public raw-URL access.
4. Redirects are not followed here. The injected P2 policy determines deny/same-origin/revalidate and hop behavior; a later explicitly authorized transport must consume the re-admitted target.

## Consequences

- Cache reuse cannot cross method/path/environment variants, and a targeted 403 remediation preserves unaffected signatures.
- Trusted-domain, private-address, and redirect rules reuse P2's tested exact-host/DNS-pinning boundary rather than a second permissive URL parser.
- P9-09 may add a live signer client only after separate Canary authorization; no P9-05 API can send an HTTP request.

## Validation and rollback

Synthetic static-DNS tests cover exact cache isolation, 403 single-key invalidation, expiry/capacity, malformed keys/signatures, HTTPS/allowlist admission, same-origin redirect re-admission, cross-domain redirect rejection, and redaction. Rollback removes this module/test and documentation only; it sends no Statsig/Web request and changes no browser, proxy/TUN, server, or production configuration.

## P12-10I-14 production amendment (`2026-08-06`)

The separately authorized production composition now consumes this boundary without widening its
targets. One Endpoint-scoped runtime obtains the current environment only from the fixed Web
`/index` page using the selected credential-bound browser session, then calls the fixed HTTPS
signer for only `POST /rest/app-chat/conversations/new`. The signer envelope is strict and its
Base64 value must decode to exactly 70 bytes.

The runtime uses a one-hour singleflight cache. A signer failure causes one fresh environment fetch
and one final signer attempt. A conversation 403 observed before any Canonical Event conditionally
invalidates only the signature that request actually sent and permits that request one retry. This
conditional comparison prevents a delayed concurrent 403 from deleting a replacement signature.
There is no retry after downstream semantic commitment, no bulk cache clearing, and no cross-account
credential mutation.

P12-10I-14 also binds the fixed Web conversation adapter into the existing native Web account pool
and runtime registry. Its capability declaration is deliberately text-only plus Streaming; Tool,
Reasoning, Vision, and native cross-provider fallback remain unavailable. Acceptance still requires
an exact-revision signed artifact, isolated Oracle CPAR HTTP E2E through a client key, and complete
rollback evidence.
