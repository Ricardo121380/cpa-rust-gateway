# P4-05 Exact-target Runtime Quota and controlled Reset recovery

| Field | Value |
|---|---|
| Plan version | `v1.4` |
| Task | `P4-05` |
| Status | `LOCAL_PASS_PENDING_CI`; local implementation, review, and complete Gate passed; GitHub Code Gate pending |
| Scope level / execution budget | `M`; `<=25min` from Task Card to code commit |
| Task Card | `gateway-router` runtime quota state, pre-lease scheduling filter, 429 ownership, and controlled Reset recovery only; no real Provider request, SQLite, Route Explain, exporter, body logging, or P4-06+ behavior |
| References | `E19`, `G20`, `G26`, `BL-17`; [ADR-0028](../adr/ADR-0028-exact-target-runtime-quota-and-controlled-reset-recovery.md); [BC-CRED-002](../contracts/BC-CRED-002-exact-target-runtime-quota-and-controlled-reset-recovery.md) |

## Scope

P4-05 adds bounded, sharded, transport-neutral runtime quota snapshots at exact
Endpoint/Credential and Endpoint/Credential/model scope. It records source, confidence, explicit
observation time, bounded windows, and the latest blocking Reset. A 429 now becomes a binding-wide
quota observation while connection, 5xx, and pre-semantic truncation remain Endpoint health
cooldowns.

Passing Reset does not automatically admit ordinary traffic. One non-cloneable controlled recovery
ticket must complete with a current exact sanitized snapshot before the binding becomes schedulable.
No raw provider material or real request enters this Task.

## Local verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS. |
| `cargo test --locked -p gateway-router` | PASS; 50 tests, including 429 source/fallback distinction, zero-fallback rejection, exact target isolation, sibling fallback, Reset-not-auto-open, one recovery ticket, stale-ticket rejection, and safe capacity reclamation. |
| `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings` | PASS after one direct lint-correction batch. |
| `ruby scripts/check-crate-boundaries.rb` and `ruby scripts/check-source-policy.rb` | PASS; 21 crate boundaries and 64 Rust files. |
| `CHECK_REPORT_PATH=tmp/p4-05-full-check.md ./scripts/check.sh full` | PASS in 26 seconds (started `2026-07-21T17:54:03+08:00`, completed `2026-07-21T17:54:29+08:00`); it covered shell/CI/plan guards, format, workspace Clippy/tests, source and crate policy, links, Secret scan, whitespace, pinned tools, `cargo deny`, and RustSec audit. |

No ignored real-test harness ran and no Provider request was sent.

## Review and execution measurement

Review confirmed that a quota target is never global/model-only: it is exactly binding-wide or
binding-plus-upstream-model. Scheduler quota checks occur before pool lease acquisition. A 429 does
not mutate endpoint health, while 5xx/connection/truncation do not fabricate quota evidence.
Estimated source/confidence cannot impersonate a direct observation. Reset due time remains
closed, a single ticket remains closed, and a stale ticket cannot overwrite fresher quota state.

The review also added capacity reclamation that removes only already-available snapshots. Deleting
a due-but-unrecovered quota would make absence look available, so exhausted, recovery-required, and
in-flight targets are never reclaimed under pressure. No RouteSnapshot/public-model visibility,
HTTP/Provider access, SQLite, management API, exporter, body, Header, URL, or Secret enters scope.

| Measurement | Evidence / value |
|---|---|
| Scope / budget | `M`; one `gateway-router` runtime-state boundary and its pre-lease integration. |
| Task Card | Durable plan state was already `IN_PROGRESS` before focused code review; no artificial start-to-commit duration is claimed. |
| Local complete Gate | `2026-07-21T17:54:03+08:00` to `2026-07-21T17:54:29+08:00` (26s); all 18 required steps passed. |
| Repeated complete Gates | `1` necessary repeat: the first completed Gate passed in 32s, then the pre-commit API review added explicit zero-fallback rejection. No unchanged code was rechecked. |
| Rework | Two focused review-correction batches: capacity-safe snapshot reclamation and strict source/confidence pairing; then explicit zero-fallback rejection plus Clippy-directed control-flow simplification. |
| Code commit / Code Gate / docs closeout / docs Gate | Pending immutable evidence after this code delivery and its normal GitHub workflow. |

## Remote Code Gate

The normal cache-visible Code Gate will be started from this code delivery. No manual rerun will be
issued.

## Closeout boundary

After Code Gate success, one docs-only closeout will record immutable evidence and mark P4-05
`DONE`. P4-06 remains `PENDING` until that closeout completes.
