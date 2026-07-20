# P3-08 bounded Request, Attempt, and Usage events report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-08` |
| Matrix / behavior | `G19`, `G21`; Behavior 1/5/9; `BL-09`, `BL-10` |
| Date | `2026-07-20` |
| Branch | `codex/p3-08-structured-events` |
| Rust | `1.97.1` |
| Result | PASS locally and for the implementation commit in GitHub Fast/Full; verification-record acceptance pending |

## Delivered scope

- Added secret-safe, serializable Request/Attempt/Usage event contracts and a synchronous
  non-blocking event-sink port in `gateway-core`.
- Added a two-class finite queue in `gateway-observability`: Required Request/Attempt/Usage records
  are isolated from drop-eligible safe Diagnostics, and both overflow paths are explicit counters
  rather than blocking fallback buffers.
- Added P3-06 `AttemptOrchestrator::start_with_event_sink`, which records one terminal event for
  every actual driver invocation with safe outcome and retry-decision metadata while preserving the
  existing no-op `start` API for legacy embeddings.
- Added P3 HTTP event injection. `POST /v1/responses` emits a Request after Snapshot model mapping
  and emits a final Usage event only after the bounded canonical stream admits it, for both JSON
  and SSE. Snapshot Alias events retain the input Alias plus the stable public model.
- Added [ADR-0018](../adr/ADR-0018-bounded-request-attempt-usage-events.md) and
  [BC-OBS-001](../contracts/BC-OBS-001-bounded-request-attempt-usage-events.md), then marked
  P3-08 as the plan's sole in-progress task.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-core -p gateway-observability -p gateway-router -p gateway-http-actix` | PASS; covers event privacy, finite queues, Attempt fallback observations, JSON/SSE Request/Usage correlation, Snapshot Alias mapping, and saturation without response blocking |
| `cargo clippy --locked -p gateway-core -p gateway-observability -p gateway-router -p gateway-http-actix --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `ruby scripts/check-crate-boundaries.rb`, `ruby scripts/check-source-policy.rb`, and `ruby scripts/check-doc-links.rb` | PASS |
| `scripts/secret-scan.sh --staged` and `git diff --cached --check` | PASS; 19 P3-08 implementation, contract, traceability, boundary, and lockfile changes staged and scanned |
| `./scripts/check.sh fast` | PASS; complete workspace fast gate |
| `./scripts/check.sh full` | PASS; complete workspace full gate, including dependency policy and RustSec audit; existing duplicate-version notices are policy-allowed warnings |

## Review

Review passed. The final data-path review found no SQLite/Repository, network exporter, global
queue, or reverse Router-to-observability dependency. All data-path emission uses only
`GatewayEventSink::try_emit`; Required and Diagnostic queues remain independent, and the Required
overflow counter/result is explicit while JSON/SSE delivery remains successful.

The review added a regression that serializes a Usage event sourced from a real raw extension and
proves that extension text is absent. It also added asynchronous receiver-priority coverage, a
Snapshot Alias/Access Group Request-event check, and an Attempt fallback check that verifies one
terminal record per actual driver invocation. `Debug` continues to redact non-public model values,
and no event contains Body, headers, presented Client Key, Credential bytes, URL, or upstream
diagnostic text.

## Scope and deferred work

P3-08 does not write SQLite, batch/flush events, persist health, export tracing/Prometheus/OpenTelemetry,
enable Body capture, create a real aggregated HTTP executor, call a deployed Endpoint, or expose an
event query API. P3-09 owns integrated Mock HTTP E2E; P3-10 owns real-endpoint validation;
P4-07 through P4-09 own durable writer/exporter/logging behavior.

## GitHub CI

GitHub Actions run [29710065770](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29710065770)
passed for implementation commit `0c0dfe5`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-20T00:53:34Z` |
| Full supply-chain gate | PASS; completed `2026-07-20T01:04:25Z` |

This completes P3-08 implementation acceptance. The separate verification-record commit must pass
the same two jobs before the final status record can be created; that final record must also pass
before P3-09 can become the plan's sole `IN_PROGRESS` task.
