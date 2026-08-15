# P13-07 routing phase preflight receipt — 2026-08-15

Status: `READY_FOR_FORMAL_DELIVERY_GATE`

## Scope

This receipt covers P13-07A/B/C/D as one routing closeout candidate:

- Provider-scoped deterministic eligibility and ranking;
- read-only Route Explain composition over the serving scheduler state;
- request-time same-scheduler lease revalidation, including quota recovery;
- Config-Version-bound six-dimensional routing price evidence.

It does not authorize Provider traffic, staging or production deployment, credential refresh or
reauth, proxy-pool work, a public inference-protocol change, formal Prism page implementation, or
P13-08/P13-11/P13-12 development.

## Candidate lineage

- Branch: `codex/p13-06c-operator-feedback`
- Reviewed implementation base: `8d17123c6d479d72d06e776598ef66eaf2c667b3`
- Planned immutable tag: `phase-p13-routing-complete`
- Formal closeout SHA and GitHub Actions run: recorded by the tag and the post-Gate reconciliation
  after Authorize, Fast, Full supply-chain and Required all reach `success`.

The annotated tag is created only after this receipt, the independent phase review and the status
index corrections are committed. The four pre-existing untracked helper files remain outside the
candidate.

## Authoritative local preflight

```text
CHECK_REPORT_PATH=/tmp/cpar-p13-07-phase-preflight-20260815.md ./scripts/check.sh full
```

- Host: `Darwin 25.2.0 arm64`
- Started: `2026-08-15T08:52:23Z`
- Completed: `2026-08-15T08:54:28Z`
- Result: `PASS`

All 43 steps passed. The run included:

- shell/workflow/classifier/plan/Canary/Caddy guards;
- Prism dependency install, authoritative contract/generated-client checks and reproducible double
  build with 82 management operations;
- Rust format, strict workspace Clippy and complete all-feature workspace tests;
- P12 serve/offline regression harnesses, source policy and crate boundaries;
- document links, contract references, tracked Secret scan and whitespace;
- pinned quality-tool versions, `cargo deny check` and RustSec audit.

Expected duplicate-version notices from `cargo-deny` remained policy-visible and non-fatal; no
security advisory failed the run.

## Phase evidence

| Slice | Evidence | Local state |
|---|---|---|
| P13-07A | `p13-07a-provider-scoped-selector.md`, ADR-0084, BC-ROUTE-006 | `LOCAL_PASS_PENDING_PHASE_GATE` |
| P13-07B | `p13-07b-provider-scoped-route-explain.md`, ADR-0085, BC-ROUTE-007 | `LOCAL_PASS_PENDING_PHASE_GATE` |
| P13-07C | `p13-07c-serving-lease-revalidation.md`, ADR-0086, BC-ROUTE-008 | `LOCAL_PASS_PENDING_PHASE_GATE` |
| P13-07D | `p13-07d-config-bound-routing-price-evidence.md`, ADR-0087, BC-ROUTE-009 | `LOCAL_PASS_PENDING_PHASE_GATE` |

## Formal Gate boundary

The closeout uses one annotated tag event. Ordinary branch pushes do not count as formal evidence.
The Gate is accepted only when the exact tag SHA has successful Authorize, Fast, Full supply-chain
and Required jobs. A failure does not permit P13-08 to start and the immutable failed tag must not
be moved.
