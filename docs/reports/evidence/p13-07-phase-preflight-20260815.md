# P13-07 routing phase preflight receipt — 2026-08-15

Status: `DONE_WITH_BOUNDARY`

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
- Immutable tag: `phase-p13-routing-complete`
- Exact closeout commit: `0c338ee8eef76e470c55515a24728324684365c5`
- Formal Delivery Gate: GitHub Actions run
  [31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495)

The annotated tag resolves to the exact closeout commit above. It was created once after this
receipt, the independent phase review and the status-index corrections were committed; it was not
moved or recreated. The four pre-existing untracked helper files remained outside the candidate.

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
| P13-07A | `p13-07a-provider-scoped-selector.md`, ADR-0084, BC-ROUTE-006 | `DONE_WITH_BOUNDARY` |
| P13-07B | `p13-07b-provider-scoped-route-explain.md`, ADR-0085, BC-ROUTE-007 | `DONE_WITH_BOUNDARY` |
| P13-07C | `p13-07c-serving-lease-revalidation.md`, ADR-0086, BC-ROUTE-008 | `DONE_WITH_BOUNDARY` |
| P13-07D | `p13-07d-config-bound-routing-price-evidence.md`, ADR-0087, BC-ROUTE-009 | `DONE_WITH_BOUNDARY` |

## Formal Delivery Gate result

The single annotated-tag event for `phase-p13-routing-complete` triggered formal run
[31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495)
against exact commit `0c338ee8eef76e470c55515a24728324684365c5`.

| Job | Result | Duration |
|---|---|---:|
| Authorize | `success` | 3s |
| Fast | `success` | 5m57s |
| Full supply-chain | `success` | 1m16s |
| Required | `success` | 2s |

The only workflow annotations were non-blocking GitHub Node.js 20 deprecation notices for the
pinned `actions/checkout` and `actions/cache` actions. They did not change the successful Gate
result and did not authorize a workflow or dependency change during closeout.

## Accepted boundary

The local Full preflight passed all 43 steps and the exact immutable closeout commit passed all four
formal Delivery Gate jobs. P13-07 therefore closes as `DONE_WITH_BOUNDARY`. The P13 umbrella remains
`IN_PROGRESS` because later independent backlog still exists; P13-08 was not started. No Provider
request, staging deployment, production deployment or server mutation occurred as part of this
closeout or reconciliation.
