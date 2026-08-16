# P13-08 Channel Pin phase preflight receipt — 2026-08-16

Status: `READY_FOR_FORMAL_DELIVERY_GATE`

## Scope

This receipt covers the P13-08 protected Channel Pin diagnostic as one independent closeout
candidate:

- authenticated and revision-bound `POST /admin/operations/channel-pin` management admission;
- exact Provider, channel, route, credential, public model, protocol and JSON/SSE pinning;
- reuse of the serving RouteSnapshot, scheduler, Credential pool/lease, Health/Quota, capability
  and egress owners;
- at most one admitted generic inference send, first-failure termination, cursor-free exact lease,
  bounded Canonical drain and value-free receipt;
- pre-send value-free audit and authoritative OpenAPI/Prism contract synchronization.

It does not authorize native Grok/Kiro/Official adapters, hidden token/Statsig/bootstrap/refresh
HTTP, a real Provider request, a public `/v1/*` diagnostic route, production or staging deployment,
server/DNS/Caddy mutation, credential refresh/reauth, proxy-pool fallback, P13-09, P13-10 or P13-11.

## Candidate lineage

- Branch: `codex/p13-08-channel-pin-gate`
- Channel Pin implementation commit: `b6a7085ed69144f395d3b5579541844fabf424e9`
- Full-gate corrective commit: `04a8d315750ae2eb08a3f7d6f4fd72efbb101144`
- Formal tag target: `phase-p13-channel-pin-complete`
- Exact closeout commit: pending the receipt commit
- Formal Delivery Gate: pending the single annotated-tag event

The four pre-existing untracked helper files remain outside the candidate.

## Corrective preflight record

The first Full attempt was not accepted as success evidence. It reached the complete Rust matrix
and exposed one pre-existing management regression in
`p13_05c_billing_catalog::catalog_import_is_csrf_guarded_atomic_revisioned_and_rollback_only_forks`:
the read-only catalog list handler incorrectly required the write context and returned `400`
instead of `200`.

Review confirmed that the already-known repair is one line: `GET /admin/billing/catalogs` must use
the existing `read_context`, not `write_context`. The correction was replayed directly on top of
the pure P13-08 implementation as commit `04a8d315750ae2eb08a3f7d6f4fd72efbb101144`, without
including any P13-09 stored-response changes. The focused P13-05C test then passed 2/2 before the
authoritative Full rerun.

## Authoritative local preflight

```text
CHECK_REPORT_PATH=/tmp/cpar-p13-08-phase-preflight-20260816.md ./scripts/check.sh full
```

- Exact tested commit: `04a8d315750ae2eb08a3f7d6f4fd72efbb101144`
- Host: `Darwin 25.2.0 arm64`
- Started: `2026-08-16T04:58:01Z`
- Completed: `2026-08-16T05:00:35Z`
- Result: `PASS`

All 43 steps passed. The run included:

- shell/workflow/classifier/plan/Canary/Caddy guards;
- Prism dependency installation, 83-operation generated-client/contract checks and reproducible
  double build;
- Rust format, strict workspace Clippy and the complete all-feature workspace test matrix;
- the P12 serve envelope and offline differential/observer/provider regression harnesses;
- source policy, crate boundaries, document links, contract references, tracked Secret scan and
  whitespace;
- pinned quality-tool versions, `cargo deny check` and RustSec audit.

Expected duplicate-version notices from `cargo-deny` remained policy-visible and non-fatal. The
advisory, ban, license and source policies passed, and RustSec scanned 348 dependencies against a
1216-advisory database without a failing vulnerability. Tests requiring explicit live Provider
authorization, long soak or an external client remained ignored by their own contracts; they are
not counted as P13-08 success evidence.

## Phase evidence

| Slice | Evidence | Local state |
|---|---|---|
| P13-08 | `p13-08-channel-pin-diagnostic.md`, ADR-0088, BC-MGMT-017 | `LOCAL_PASS_PENDING_PHASE_GATE` |

The focused P13-08 matrix remains upstream 32, router 134, gateway 105,
management HTTP/OpenAPI/security 19 and Grok one-shot regression 4, followed by the aggregate
43-step Full pass above.

## Formal Delivery Gate boundary

Only the immutable annotated tag `phase-p13-channel-pin-complete` may trigger the P13-08 formal
Delivery Gate. The branch and tag must first resolve to the exact receipt commit, and the single
Authorize, Fast, Full supply-chain and Required job chain must all succeed before P13-08 is changed
to `DONE_WITH_BOUNDARY`.

No Provider request, credential operation, server mutation, production/staging traffic or later
P13 feature is authorized by this receipt.
