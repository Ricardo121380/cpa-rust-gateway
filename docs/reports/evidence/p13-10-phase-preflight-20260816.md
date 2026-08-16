# P13-10 public Responses WebSocket phase preflight receipt — 2026-08-16

Status: `DONE_WITH_BOUNDARY`

## Scope

This receipt covers the P13-10A public Responses WebSocket slice as one closeout candidate:

- authenticated no-Origin `GET /v1/responses` RFC 6455 upgrade;
- strict text-only flat `response.create` input and Responses lifecycle JSON output;
- reuse of the existing Canonical executor, scheduler, Credential lease, Health/Quota, usage,
  stored-response and exact-continuity owners;
- explicit `responses_websocket` capability and bounded message, fragment, queue, event, byte,
  write, liveness, turn and session policies.

It does not authorize Provider-native upstream WebSocket, Realtime, Chat/Messages WebSocket,
browser Origin support, a real Provider request, staging or production deployment, server/DNS/
Caddy mutation, credential refresh/reauth, P13-08 formal acceptance, or P13-11 implementation.

## Candidate lineage

- Branch: `codex/p13-10-websocket`
- Reviewed implementation commit: `fd1e21fc398c732763108bbe12036fa3999818f0`
- Formal tag target: `phase-p13-websocket-complete`
- Exact closeout commit: `dc48ec40e4fb38961925f203bf3cd0f7434a34a0`
- Formal Delivery Gate: GitHub Actions run
  [31926927914](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31926927914)

The four pre-existing untracked helper files remained outside the candidate. The annotated tag
resolves to the exact closeout commit above; it was created once and was not moved or recreated.

## Authoritative local preflight

```text
CHECK_REPORT_PATH=/tmp/cpar-p13-10-phase-preflight-20260816.md ./scripts/check.sh full
```

- Host: `Darwin 25.2.0 arm64`
- Started: `2026-08-16T04:26:59Z`
- Completed: `2026-08-16T04:28:58Z`
- Result: `PASS`

All 43 steps passed. The run included:

- shell/workflow/classifier/plan/Canary/Caddy guards;
- Prism dependency installation, 83-operation generated-client/contract checks and reproducible
  double build, without a P13-10 management-contract change;
- Rust format, strict workspace Clippy and the complete all-feature workspace test matrix;
- the P12 serve envelope and offline differential/observer/provider regression harnesses;
- source policy, the updated WebSocket dependency crate boundary, document links, contract
  references, tracked Secret scan and whitespace;
- pinned quality-tool versions, `cargo deny check` and RustSec audit.

Expected duplicate-version notices from `cargo-deny` remained policy-visible and non-fatal. The
advisory, ban, license and source policies passed, and RustSec scanned 360 dependencies against a
1216-advisory database without a failing vulnerability. Only tests whose own contracts require an
explicit live authorization, long soak or random seed remained ignored; they are not counted as
P13-10 success evidence.

## Phase evidence

| Slice | Evidence | Local state |
|---|---|---|
| P13-10A | `p13-10a-public-responses-websocket.md`, ADR-0092, BC-RESP-004 | `DONE_WITH_BOUNDARY` |

## Formal Delivery Gate result

The single annotated-tag event for `phase-p13-websocket-complete` triggered formal run
[31926927914](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31926927914)
against exact commit `dc48ec40e4fb38961925f203bf3cd0f7434a34a0`.

| Job | Result | Duration |
|---|---|---:|
| Authorize | `success` | 4s |
| Fast | `success` | 5m51s |
| Full supply-chain | `success` | 1m33s |
| Required | `success` | 3s |

The only workflow annotations were non-blocking GitHub Node.js 20 deprecation notices for the
pinned `actions/checkout` and `actions/cache` actions, which GitHub forced to Node.js 24. They did
not change the successful result and did not authorize an action-version change during closeout.

## Accepted preflight boundary

The local Full preflight passed all 43 steps and the exact immutable closeout commit passed all four
formal Delivery Gate jobs. P13-10 therefore closes as `DONE_WITH_BOUNDARY`. It does not prove a
real Provider's current account or upstream-native WebSocket behavior, nor did it deploy or mutate
CPAR on a server. P13-11 remains a separate deferred task and was not started by this Gate.
