# P4-07 Append-only SQLite Request/Attempt/Usage/Health event writer

| Field | Value |
|---|---|
| Plan version | `v1.4` |
| Task | `P4-07` |
| Status | `LOCAL_PASS_PENDING_CI`; local implementation, review, and complete Gate passed; GitHub Code Gate pending |
| Scope level / execution budget | `L`; detailed subplan required before implementation |
| Task Card | `gateway-core` event contract plus `gateway-store` migration/writer only; no Router/HTTP hot-path write, reverse observability dependency, quota, Route Explain, exporter, body logging, real Provider request, or P5 work |
| References | `G19`, `G20`, `G21`, `BL-09`, `BL-10`; [ADR-0027](../adr/ADR-0027-append-only-bounded-sqlite-event-writer.md); [BC-OBS-002](../contracts/BC-OBS-002-append-only-sqlite-event-writer.md) |

## Detailed subplan and invariants

1. Add only a secret-safe `HealthEvent`/`HealthEventId` to the Core event port and make it
   Required-priority.
2. Add one append-only `gateway_event_log` migration and a typed Store API with unique
   `(event_type, event_id)` idempotence, batch atomicity, Request/Health query, reopen, and
   `quick_check` validation.
3. Add one-way Store consumption of `EventQueueReceiver`; use a finite batch and blocking worker,
   retaining a failed Required batch while publishing explicit counters.
4. Prove that Diagnostics are counted but not claimed durable, and that producer saturation remains
   non-blocking.

The writer must never run in the HTTP/Router data path, receive a reverse Store dependency from
observability, persist body/headers/URLs/credentials, send Provider traffic, or claim that an
uncommitted in-memory batch survives a process crash.

## Implemented scope

- Added `HealthEventId`, `HealthEvent`, `HealthEventKind`, Required priority, and a redacted
  `Debug` regression in `gateway-core`; existing P3 aggregation test matches explicitly reject or
  ignore future Health observations without weakening their Request/Attempt/Usage expectations.
- Added migration `0005_gateway_event_log`, append-only triggers, strict typed decode, atomic
  insertion, idempotent replay, conflict refusal, request/health lookup, file reopen, and
  `PRAGMA quick_check` in `gateway-store`.
- Added `AsyncSqliteEventWriter` with a bounded batch, Tokio blocking write boundary, retained
  pending Required batch, positive retry delay, and separate committed/inserted/failure/pending/
  diagnostic-not-persisted counters.
- Updated the crate-boundary policy to permit only `gateway-store -> gateway-observability` for
  receiver consumption; `gateway-observability -> gateway-store` remains forbidden.

## Local targeted verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS after one formatter normalization; no semantic rework. |
| `cargo test --locked -p gateway-core -p gateway-observability -p gateway-store` | PASS; 74 tests (73 unit + 1 integration), including Core privacy, bounded queue behavior, Store atomicity/idempotence/reopen/quick-check, transient writer recovery, and explicit Diagnostic accounting. |
| `cargo clippy --locked -p gateway-core -p gateway-observability -p gateway-store --all-targets --all-features -- -D warnings` | PASS after one direct lint correction batch. |
| `ruby scripts/check-crate-boundaries.rb` and `ruby scripts/check-source-policy.rb` | PASS; 21 crate boundaries and 63 Rust files. |

No ignored real-test harness ran and no Provider request was sent.

## Review and execution measurement

Focused review confirmed that `gateway_event_log` uses a type-plus-id idempotence key rather than
a global id, a conflicting replay rolls back the whole batch, query decoding validates redundant
metadata, SQLite failures leave the finite pending batch intact, Diagnostics have a separate
counter, and queue pressure remains explicit at the producer. The writer moves only Store work to
Tokio's blocking pool; it does not let Router/HTTP await it. Health values retain only stable IDs,
a sanitized kind, explicit time, and an optional access-controlled model label whose debug form is
redacted.

| Measurement | Evidence / value |
|---|---|
| Scope / budget | `L`; Core contract + Store schema + asynchronous persistence boundary. |
| Task Card | Visible before focused dependency reads; detailed four-step scope above. |
| Local complete Gate | `CHECK_REPORT_PATH=tmp/p4-07-full-check.md ./scripts/check.sh full` PASS; started `2026-07-21T16:48:11+08:00`, 45 seconds total. It covered shell/CI/plan guards, format, workspace Clippy/tests, source and crate policy, links, Secret scan, whitespace, pinned tools, `cargo deny`, and RustSec audit. |
| Repeated complete Gates | `1` necessary retry: the first run stopped at Clippy in 8 seconds because a new P3-09 test used prohibited `panic!`; it executed no later Full checks. The direct test-error correction then passed the one completed Full Gate. |
| Rework | `3` direct correction batches: lockfile update for new internal dependencies; focused Clippy/format corrections; and the prohibited-test-panic correction identified by the first Full Gate. |
| Code commit / Code Gate / docs closeout / docs Gate | Pending immutable evidence. |

## Closeout boundary

After the one local complete Gate and review, the code delivery will use the normal cache-visible
Code Gate. A successful Code Gate will be followed by exactly one docs-only closeout that records
immutable results and marks P4-07 `DONE`; P4-08 remains `PENDING` until then.
