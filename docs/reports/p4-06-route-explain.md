# P4-06 Fixed-input Route Explain and Candidate exclusion reasons

| Field | Value |
|---|---|
| Plan version | `v1.4` |
| Task | `P4-06` |
| Status | `LOCAL_PASS_PENDING_CI`; local implementation, review, and complete Gate passed; GitHub Code Gate pending |
| Scope level / execution budget | `M`; `<=25min` from Task Card to code commit |
| Task Card | `gateway-router` fixed-input Route Explain plus secret-free `gateway-upstream` pool diagnostics only; no management HTTP, SQLite, event write, exporter, body logging, real Provider request, affinity/continuity behavior, P4-08/P4-09, or P5 work |
| References | `E15`, `E16`, `E23`, `G20`, `G21`, `L20-L26`; [ADR-0029](../adr/ADR-0029-fixed-input-route-explain.md); [BC-ROUTE-003](../contracts/BC-ROUTE-003-fixed-input-route-explain.md) |

## Scope

P4-06 adds a pure Route Explain projection over the immutable route/pool assembly, explicit
observation time, explicit schedule starts, request-local exclusions, runtime Health, and runtime
Quota. It returns every Candidate and binding with exact typed reasons, plus an optional projected
policy selection. It never reserves capacity or changes real weighted scheduling state.

The only `gateway-upstream` addition is a bounded secret-free pool-entry snapshot and a
non-mutating peek. No Credential Secret, HTTP/Provider data, persistence, management endpoint, or
real probe enters this Task.

## Local verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS. |
| `cargo test --locked -p gateway-upstream -p gateway-router` | PASS; 27 upstream + 54 router tests, including exact endpoint/model reasons, saturation, request-local exclusion, fixed starts, no cursor/lease side effect, and unknown Route refusal. |
| `cargo clippy --locked -p gateway-upstream -p gateway-router --all-targets --all-features -- -D warnings` | PASS after one direct lint-correction batch. |
| `ruby scripts/check-crate-boundaries.rb` and `ruby scripts/check-source-policy.rb` | PASS; 21 crate boundaries and 65 Rust files. |
| `CHECK_REPORT_PATH=tmp/p4-06-full-check.md ./scripts/check.sh full` | PASS in 26 seconds (started `2026-07-21T18:27:05+08:00`, completed `2026-07-21T18:27:31+08:00`); it covered shell/CI/plan guards, format, workspace Clippy/tests, source and crate policy, links, Secret scan, whitespace, pinned tools, `cargo deny`, and RustSec audit. |

No ignored real-test harness ran and no Provider request was sent.

## Review and execution measurement

Review confirmed that Explain reads exactly the same immutable route/pool structure as real
scheduling but has no `try_lease`, cursor increment, state mutation, Store, HTTP, or Provider
path. Explicit time and schedule starts make the projection fixture-stable. Each Health/Quota
lookup is target-local and failures are reported as fail-closed reasons rather than rich internal
errors. Credential diagnostic values exclude kind, revision, and Secret material.

One concurrent-capacity review correction changed the projection from an early `None` to continued
Candidate scanning if a binding becomes saturated between its recorded observation and the
non-mutating peek. This preserves the projection's safety claim without fabricating a lease or
hiding a healthy fallback.

| Measurement | Evidence / value |
|---|---|
| Scope / budget | `M`; explain snapshot and a narrow secret-free pool diagnostic seam. |
| Task Card | P4-05 docs-only Gate completed before focused P4-06 work; no artificial start-to-commit duration is claimed. |
| Local complete Gate | `2026-07-21T18:27:05+08:00` to `2026-07-21T18:27:31+08:00` (26s); all 18 required steps passed. |
| Repeated complete Gates | `1` necessary repeat: the first completed Gate passed in 39s, then review strengthened the test to prove both Candidate and Credential cursors remain untouched. |
| Rework | Two direct review-correction batches: concurrent non-mutating peek fallback and Clippy's single-pattern control-flow simplification; then explicit live Candidate-cursor proof. |
| Code commit / Code Gate / docs closeout / docs Gate | Pending immutable evidence after this code delivery and its normal GitHub workflow. |

## Remote Code Gate

The normal cache-visible Code Gate will be started from this code delivery. No manual rerun will be
issued.

## Closeout boundary

After Code Gate success, one docs-only closeout will record immutable evidence and mark P4-06
`DONE`. P4-08 remains `PENDING` until that closeout completes.
