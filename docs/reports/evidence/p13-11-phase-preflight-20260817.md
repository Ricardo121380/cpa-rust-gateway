# P13-11 generic egress phase preflight receipt — 2026-08-17

Status: `READY_FOR_FORMAL_DELIVERY_GATE`

## Scope

This receipt aggregates the local evidence for P13-11A, P13-11B and P13-11C:

- generic compatible endpoints are represented by exact Upstream/Endpoint/Credential bindings;
- the active Config Version is composed with the existing Credential pool and shared Health/Quota
  registries;
- request-time serving reuses the existing exact Credential lease and acquires only an exact
  egress lease, preserving the JSON/SSE transport profile and source lifetime.

The aggregate does not authorize P13-11D, Provider-specific browser/clearance behavior, automatic
probe or recovery, Autoreg operations, or a production rollout.

## Candidate lineage

- Branch: `codex/p13-11-egress`
- Exact local implementation commit: `fafb34a38af33b798915d922ca035a5e32f7c9e8`
- Candidate phase tag (not created): `phase-p13-egress-complete`; its target must be the exact
  pushed HEAD that contains this receipt and the accompanying phase review, not the implementation
  commit alone.
- Host: `Darwin 25.2.0 arm64`
- Pre-existing untracked helper files: preserved outside the candidate and not staged.

## Authoritative local Full preflight

```text
CHECK_REPORT_PATH=/tmp/cpar-p13-11-phase-preflight-20260817.md ./scripts/check.sh full
```

- Started: `2026-08-16T16:12:07Z`
- Completed: `2026-08-16T16:14:15Z`
- Result: `PASS`
- Completed steps: `43/43`

The run passed the workflow and plan guards, management SPA/contract checks, Rust format,
workspace Clippy and tests, the P12 serve envelope and offline regression harnesses, source and
crate-boundary policy, document links, contract references, tracked Secret scan, whitespace,
quality-tool/dependency policy and RustSec audit. The expected duplicate dependency-version notices
from `cargo-deny` were non-fatal policy diagnostics.

## Slice evidence

| Slice | Evidence | Local state |
|---|---|---|
| P13-11A | `p13-11a-generic-compatible-endpoint-egress.md`, ADR-0093, BC-SEC-005 | `LOCAL_PASS_PENDING_PHASE_GATE` |
| P13-11B | `p13-11b-compatible-endpoint-runtime-composition.md`, ADR-0094, BC-SEC-006 | `LOCAL_PASS_PENDING_PHASE_GATE` |
| P13-11C | `p13-11c-compatible-serving-transport-handoff.md`, ADR-0095, BC-SEC-007 | `LOCAL_PASS_PENDING_PHASE_GATE` |

The affected local suites include `gateway-upstream` `37`, `gateway-router` `151`,
`gateway-control` `72`, and `gateway` `106`, together with the complete workspace matrix included
by Full. The slice reports remain the authoritative detailed test and boundary descriptions.

## Explicit non-evidence

No Provider request, DNS lookup, proxy probe, server/staging/production mutation, credential
refresh, Autoreg operation, GitHub tag, push or formal Delivery Gate was performed. The default
deployment remains Direct-only. P13-11D protected proxy-pool persistence/management and
Provider-specific probe/recovery require a later, separately authorized task.
