# P4-10 Read-only runtime management status and controlled Credential-account recovery

| Field | Value |
|---|---|
| Plan version | `v1.8` |
| Task | `P4-10` |
| Status | `LOCAL_PASS_PENDING_CI`; implementation, review, Secret scan, and final local Full Gate passed; GitHub Code Gate remains pending. |
| Scope level / execution budget | `M`; `<=25min` from Task Card to code commit, excluding external Gates |
| Task Card | `gateway-router` in-process read-only runtime status projection plus exact 403 account state/controlled recovery only; no HTTP management route, authentication, UI, SQLite, Event/exporter change, Provider request, raw provider parsing, persistent read model, or P5 work |
| References | `CR-P4-G4-001`; `G20`, `G21`, `G26`, `H19`, `H20`; [ADR-0032](../adr/ADR-0032-read-only-runtime-management-status.md); [BC-MGMT-001](../contracts/BC-MGMT-001-read-only-runtime-management-status.md) |

## Detailed subplan and invariant

1. Expose one exact Endpoint/Credential status projection at a caller-supplied time, optionally
   including model-scoped Health and Quota, without rendering a model label or retaining sensitive
   provider material.
2. Convert only the safe existing `CredentialForbidden` classification into exact binding state;
   preserve its non-retryable request behavior and sibling isolation.
3. Require a non-cloneable, exact binding recovery ticket before a 403 account can become
   schedulable again; a read-only query cannot operate that ticket.
4. Keep the query outside HTTP, SQLite, transport, Event/export, and scheduler/lease mutation.
   Same explicit time does not claim a cross-shard atomic snapshot or future scheduling outcome.

## Implemented scope

- Added `RuntimeManagementStatusQuery` and safe Target/Snapshot/Quota projection types to
  `gateway-router`.
- Added bounded exact Credential-account forbidden/recovery state to `RuntimeHealthRegistry` and
  records it before the existing non-retryable Attempt return path.
- Preserved generic Health/Circuit protection against accidental account reopening and surfaced the
  account block as a P4-06 Route Explain credential reason.
- Added direct 403, real 429-recording, Header/Estimated quota evidence, Circuit, recovery,
  explicit-time, read-only, and Debug-redaction coverage.

## Targeted verification and review

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS after formatter normalization. |
| `cargo test --locked -p gateway-router` | PASS; 59 tests, including P4-10 403 binding isolation/recovery, rejected/expired ticket fail-closed behavior, read-only status projection, actual 429 record projection, safe model redaction, Route Explain visibility, and prior P4 regressions. |
| `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings` | PASS after direct map-entry, match-compression, borrow, and focused test-size corrections. |
| `scripts/secret-scan.sh --all` | PASS; the staged P4-10 scope contains no detected Secret. |
| `CHECK_REPORT_PATH=tmp/p4-10-full-check.md ./scripts/check.sh full` | PASS in 40 seconds (started `2026-07-21T13:58:17Z`, completed `2026-07-21T13:58:57Z`); shell/CI/plan guards, format, workspace Clippy/tests, source/crate policy, Secret scans, document links, whitespace, pinned quality tools, `cargo deny`, and RustSec audit passed. |

Review confirms that the query has no mutable registry call; it does not make multiple registry
states falsely atomic; 403 recovery cannot be completed by generic Health success; a late recovery
ticket fails closed; and the new API does not expose a Provider payload, model label, Endpoint URL,
Credential bytes, or management HTTP surface. No ignored real-test harness ran and no Provider
request was sent.

## Delivery state

The implementation commit now records `LOCAL_PASS_PENDING_CI` and is awaiting its normal GitHub
Code Gate. After that Gate passes, one docs-only closeout may mark P4-10 `DONE`; its docs-only Gate
then becomes the final Task evidence. G4 remains blocked until both P4-10 delivery Gates pass.
