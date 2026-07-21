# P4-07 Append-only SQLite Request/Attempt/Usage/Health event writer

| Field | Value |
|---|---|
| Plan version | `v1.4` |
| Task | `P4-07` |
| Status | `DONE` after this docs-only closeout Gate; Code Gate passed |
| Scope level / execution budget | `L`; 45-minute scoped code-delivery target excluding external Gates |
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
| Scope / budget | `L`; Core contract + Store schema + asynchronous persistence boundary; 45-minute scoped delivery target excluding external Gates. |
| Task Card | Visible before current-turn focused dependency reads; the inherited initial Core type patch predates this durable anchor, so no false start-to-commit duration is claimed. The detailed four-step scope above bounded all remaining work. |
| Local complete Gate | `CHECK_REPORT_PATH=tmp/p4-07-full-check.md ./scripts/check.sh full` PASS; started `2026-07-21T16:48:11+08:00`, 45 seconds total. It covered shell/CI/plan guards, format, workspace Clippy/tests, source and crate policy, links, Secret scan, whitespace, pinned tools, `cargo deny`, and RustSec audit. |
| Repeated complete Gates | `1` necessary retry: the first run stopped at Clippy in 8 seconds because a new P3-09 test used prohibited `panic!`; it executed no later Full checks. The direct test-error correction then passed the one completed Full Gate. |
| Rework | `3` direct correction batches: lockfile update for new internal dependencies; focused Clippy/format corrections; and the prohibited-test-panic correction identified by the first Full Gate. |
| Code commit | `25b6df6`, `2026-07-21T16:50:58+08:00`. |
| Code Gate passed | `2026-07-21T16:59:09+08:00`; [run 29816136225](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29816136225). |
| Docs closeout / docs Gate | This one docs-only closeout records immutable Code Gate evidence. Its required docs-only Gate is external evidence and will not cause a second status commit. |

## Accepted GitHub Code Gate and delivery-flow measurement

GitHub Actions [run 29816136225](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29816136225)
passed for implementation commit `25b6df6` on the cache-visible
`codex/p4-01-catalog-singleflight` delivery ref. It was the normal push Gate, not a manual rerun.

| Job / step | Result and duration |
|---|---|
| Classify delivery gate | PASS; selected `code`; Docs-only Gate correctly skipped. |
| Fast gate | PASS; job about 184 seconds, complete `Run fast gate` about 155 seconds. |
| Full supply-chain gate | PASS; job about 39 seconds after Fast. |
| Cache | PASS; primary key hit; restore took about 4 seconds. |
| Install pinned quality tools | PASS; version-verified `cargo-deny` and `cargo-audit` completed in about 2 seconds, within the `<=10s` operational target and `<=90s` hard ceiling. |
| Supplemental supply-chain | PASS; version verification, `cargo deny check`, and RustSec audit completed in about 7 seconds without replaying Workspace Fast checks. |
| Required delivery gate | PASS; fail-closed verification of the code path's Fast + Full results. |

The workflow was created at `2026-07-21T16:55:00+08:00`, first started about 3 seconds later, and
completed at `2026-07-21T16:59:09+08:00` (about 4 minutes 9 seconds). The warm `<=4min` target
missed by about 9 seconds, but the cache hit and all required Gates passed. This is a
delivery-performance observation rather than a correctness or supply-chain failure; no manual
rerun is issued.

## Closeout boundary

This is the single docs-only closeout that records the immutable Code Gate evidence and marks P4-07
`DONE`. Its own GitHub status is external evidence and will not cause another status-only commit.
