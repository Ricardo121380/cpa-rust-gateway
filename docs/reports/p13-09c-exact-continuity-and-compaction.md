# P13-09C exact continuity and compaction report

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

Date: 2026-08-16

## Intended outcome

Implement Client-Key-owned `previous_response_id` and `/v1/responses/compact` over the P13-09A/B
encrypted foundation. Both operations must execute only through the stored exact Config/Provider/
Channel/Route/Candidate/Credential revision, with explicit Provider capability and no retry,
sibling, egress, credential-format, or cross-Provider fallback.

## Frozen boundary

- CPAR rebuilds Canonical history locally; it does not forward `previous_response_id`.
- Compact tokens are CPAR-owned AEAD locators under a separate domain.
- Generic adapters need explicit capability evidence; the built-in Grok matrix is Build both, Web
  continuity only, Console neither.
- Management OpenAPI/Prism, production/staging/server state, and real Provider traffic are out of
  scope.

## Implementation evidence

- `protocol-openai-responses` strictly decodes `previous_response_id`, the first CPAR-owned
  compaction item, and non-streaming `/v1/responses/compact`; gateway controls are removed from
  native payload ownership and the public compaction response exposes only an opaque locator.
- `gateway-catalog` and `gateway-control` add closed `stored_responses` / `response_compaction`
  capabilities. Compaction implies stored continuity; Build declares both, Web only continuity,
  Console neither, and a generic Responses candidate requires an explicit closed override.
- `gateway-upstream` and `gateway-router` acquire one exact Candidate and Credential revision
  without advancing ordinary cursors. Config/Provider/Upstream/Channel/Route/Candidate/Credential,
  hard eligibility, capability, Health, Quota, expiry and capacity are revalidated; the one-shot
  Attempt has no sibling, recovery, retry, egress or cross-Provider fallback.
- migration `0018` and `SqliteStoredResponseStore` add a separate AEAD compaction table/domain,
  owner-scoped random locator, 30-day TTL, read-time expiry, restart/key-rotation support,
  corruption handling and bounded GC.
- the public Responses handler locally replays complete stored Canonical history, preserves visible
  text and complete Tool-call order, and never forwards stored IDs or compact locators. Compact uses
  a fixed prompt, 2048 output-token ceiling, 4096-event/8-MiB bounds and two-minute total timeout;
  incomplete, empty, corrupt, foreign or mismatched state creates no successful public result.

## Local verification

- affected package suites: gateway-catalog `15`, gateway-control `66`, gateway-upstream `32`,
  gateway-router `138`, gateway-store `58`, protocol-openai-responses `29`,
  gateway-http-actix `61`, and gateway `106` unit tests all PASS; applicable integration suites and
  the gateway component smoke test also PASS;
- strict Clippy with `-D warnings` across all eight touched packages: PASS;
- `cargo fmt --all -- --check`, docs (`539` Markdown / `107` referenced contract tests / `129`
  plan tasks with one `IN_PROGRESS`), source policy (`224` Rust files / `21` crate roots), crate
  boundaries (`21` packages), tracked Secret scan and `git diff --check`: PASS;
- the aggregate P13-09 Full preflight passed `43/43` steps on `Darwin 25.2.0 arm64` from
  `2026-08-16T02:45:33Z` through `2026-08-16T02:48:18Z`; the one formal tagged Delivery Gate is
  the only remaining closeout step, and no per-slice GitHub Gate was started.

## Review

The final local diff review found and closed the following gaps before marking the slice locally
passed:

1. corrupt compact ciphertext originally surfaced as an internal error; it now follows the same
   owner-safe not-found projection as corrupt stored Responses, while SQLite/lock failures remain
   internal;
2. compact execution originally had event/byte bounds but no whole-source wall-clock ceiling; it
   now has a two-minute total bound and starts TTL only after the complete response and public
   metadata are ready, avoiding an invisible orphan on metadata failure;
3. the existing production capability-ledger test did not assert the new Build/Web/Console
   distinctions; its closed matrix now covers both new capability values;
4. regressions now cover exact model mismatch with zero executor calls, foreign Client Key owner,
   local compact reuse, Tool-call replay order, exact revision/no-fallback lease behavior, compact
   ciphertext corruption, restart/key rotation, expiry and bounded GC.

No remaining P1/P2 correctness, ownership, resource-bound or secret-exposure issue was found in
the accepted local scope. Deterministic runtime/router/HTTP tests do not claim a real Provider,
staging or production compact execution.

## Boundary

P13-09A/B/C are locally implemented, reviewed and covered by one aggregate Full preflight. P13-10
must not start until the single P13-09 formal Delivery Gate closes the phase boundary.
