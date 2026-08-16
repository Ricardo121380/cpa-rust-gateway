# P13-10 public Responses WebSocket phase preflight receipt — 2026-08-16

Status: `READY_FOR_FORMAL_DELIVERY_GATE`

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
- Exact closeout commit: the immutable commit containing this receipt and the phase review; it must
  be recorded after the commit is created and before the tag is pushed.

The four pre-existing untracked helper files remain outside the candidate. The formal tag must be
created once and must not be moved or recreated.

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
| P13-10A | `p13-10a-public-responses-websocket.md`, ADR-0092, BC-RESP-004 | `LOCAL_PASS_PENDING_PHASE_GATE` |

## Formal Delivery Gate target

The only remaining P13-10 closeout action is one annotated-tag Delivery Gate for
`phase-p13-websocket-complete`. The tag must resolve to the exact pushed closeout commit containing
this receipt and the phase review. Authorize, Fast, Full supply-chain and Required must all pass
before P13-10A or the aggregate is changed to `DONE_WITH_BOUNDARY`.

## Accepted preflight boundary

The local Full preflight proves deterministic code, protocol, resource-bound, dependency,
security and supply-chain checks on the reviewed local candidate. It does not prove a real
Provider's current account or upstream-native WebSocket behavior, and it does not deploy or mutate
CPAR on a server. P13-11 remains blocked until the single formal tagged Gate succeeds and the exact
run is reconciled into the phase evidence.
