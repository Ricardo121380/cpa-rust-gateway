# P12-10I-14 Web Native Runtime Local Receipt

Status: `LOCAL_PASS_PENDING_SIGNED_ORACLE_E2E`

## Scope

This receipt covers only the local implementation and review of CPAR's native Grok Web execution
path. It sent no Web, Statsig signer, Console, Build, or other upstream request; changed no server,
active Config Version, public route, Caddy/DNS, CC Switch, old CPA, grok2api, or production account.

## Frozen reference behavior implemented

- Fixed Cookie-bound Chrome-profile `GET /index` environment fetch.
- Fixed HTTPS signer and exact `POST /rest/app-chat/conversations/new` signature tuple.
- Strict one-field signer JSON with a Base64 value decoding to exactly 70 bytes.
- One-hour Endpoint-scoped singleflight cache.
- One environment refresh and final signer attempt after an initial signer failure.
- One pre-Canonical-event conversation retry after a 403.
- Conditional invalidation: a delayed 403 for an old signature cannot remove a concurrently
  installed replacement signature.
- Fixed conversation target, browser Cookie/User-Agent session, DNS-pinned transport and strict
  concatenated JSON-object decoder.

## Gateway composition

- `grok.web.responses` is bound to `GrokAccountProvider::Web` and therefore uses the existing
  encrypted native account pool, scheduler, concurrency lease, Health and Quota isolation.
- Stored base URL and path must equal the fixed Web conversation target.
- The adapter declares only Streaming; Tool, Reasoning, JSON Schema, Parallel Tool and Vision
  capabilities remain absent.
- Public Chat Completions, Responses and Messages continue through the existing Canonical request
  and response projection. This is not a separate protocol implementation.

## Local validation

- provider-grok full suite: PASS, including all active tests; authorization-gated real probes stayed
  ignored.
- gateway full suite: PASS, including all unit and component tests.
- focused dynamic Statsig singleflight/403 test: PASS.
- focused native Web 403-refresh/live-stream adapter test: PASS.
- fixed meta normalization and 70-byte signer response test: PASS.
- provider-grok + gateway all-target strict Clippy: PASS.
- `cargo fmt --all -- --check` and `git diff --check`: PASS.

## Pending acceptance

This is not a live Web reverse-proxy success. P12-10I-14 remains in progress until this exact commit
has a verified signed ARM64 artifact, an isolated Oracle route accepts a real request through the
CPAR base URL and client key, the result is classified without retaining values, and every temporary
graph/database/credential/artifact is rolled back or cleaned.
