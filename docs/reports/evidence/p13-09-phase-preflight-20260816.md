# P13-09 stored Responses phase preflight receipt — 2026-08-16

Status: `DONE_WITH_BOUNDARY`

## Scope

This receipt covers P13-09A/B/C as one stored Responses closeout candidate:

- Client-Key-owned, domain-separated AEAD storage with fixed TTL, read-time expiry, bounded GC,
  restart recovery and Master Key rotation;
- opt-in `store:true` plus exact-owner `GET/DELETE /v1/responses/{id}` for complete JSON and SSE
  Canonical results;
- locally reconstructed `previous_response_id` continuity and gateway-owned
  `POST /v1/responses/compact` with exact Config/Provider/Channel/Route/Candidate/Credential
  revision pinning.

It does not authorize Provider traffic, staging or production deployment, credential refresh or
reauth, public WebSocket work, a management OpenAPI/Prism change, or P13-10 implementation.

## Candidate lineage

- Branch: `codex/p13-09-stored-responses`
- Reviewed implementation commit: `020aa61055f904f6210b7521252b23d4a503f3a3`
- Formal tag target: `phase-p13-responses-complete`
- Exact closeout commit: `d419c4678bd2ff563046849cef800c1985d48688`
- Formal Delivery Gate: GitHub Actions run
  [31922870604](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31922870604)

The four pre-existing untracked helper files remained outside the candidate. The annotated tag
resolves to the exact closeout commit above; it was created once and was not moved or recreated.

## Authoritative local preflight

```text
CHECK_REPORT_PATH=/tmp/cpar-p13-09-phase-preflight-20260816.md ./scripts/check.sh full
```

- Host: `Darwin 25.2.0 arm64`
- Started: `2026-08-16T02:45:33Z`
- Completed: `2026-08-16T02:48:18Z`
- Result: `PASS`

All 43 steps passed. The run included:

- shell/workflow/classifier/plan/Canary/Caddy guards;
- Prism dependency installation, 83-operation generated-client/contract checks and reproducible
  double build, without a P13-09 management-contract change;
- Rust format, strict workspace Clippy and the complete all-feature workspace test matrix;
- the P12 serve envelope and offline differential/observer/provider regression harnesses;
- source policy, crate boundaries, document links, contract references, tracked Secret scan and
  whitespace;
- pinned quality-tool versions, `cargo deny check` and RustSec audit.

Expected duplicate-version notices from `cargo-deny` remained policy-visible and non-fatal. The
advisory, ban, license and source policies passed, and RustSec reported no failing vulnerability.
Only tests whose own contracts require an explicit live authorization, long soak or random seed
remained ignored; they are not counted as P13-09 success evidence.

## Phase evidence

| Slice | Evidence | Local state |
|---|---|---|
| P13-09A | `p13-09a-stored-response-foundation.md`, ADR-0089, BC-RESP-001 | `DONE_WITH_BOUNDARY` |
| P13-09B | `p13-09b-stored-response-public-lifecycle.md`, ADR-0090, BC-RESP-002 | `DONE_WITH_BOUNDARY` |
| P13-09C | `p13-09c-exact-continuity-and-compaction.md`, ADR-0091, BC-RESP-003 | `DONE_WITH_BOUNDARY` |

## Formal Delivery Gate result

The single annotated-tag event for `phase-p13-responses-complete` triggered formal run
[31922870604](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31922870604)
against exact commit `d419c4678bd2ff563046849cef800c1985d48688`.

| Job | Result | Duration |
|---|---|---:|
| Authorize | `success` | 2s |
| Fast | `success` | 6m10s |
| Full supply-chain | `success` | 59s |
| Required | `success` | 3s |

The only workflow annotations were non-blocking GitHub Node.js 20 deprecation notices for the
pinned `actions/checkout` and `actions/cache` actions, which GitHub forced to Node.js 24. They did
not change the successful result and did not authorize an action-version change during closeout.

## Accepted preflight boundary

The local Full preflight passed all 43 steps and the exact immutable closeout commit passed all four
formal Delivery Gate jobs. P13-09 therefore closes as `DONE_WITH_BOUNDARY`. It does not prove a
real Provider currently supports a declared stored-response capability, nor did it deploy or
mutate CPAR on a server. P13-10 remains a separate deferred task and was not started by this Gate.
