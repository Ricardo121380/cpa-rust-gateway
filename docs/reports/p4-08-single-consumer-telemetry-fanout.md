# P4-08 Structured JSON, Prometheus, and OpenTelemetry telemetry fan-out

| Field | Value |
|---|---|
| Plan version | `v1.7` |
| Task | `P4-08` |
| Status | `LOCAL_PASS_PENDING_CI`; implementation, review, targeted tests, and local complete Gate passed; GitHub Code Gate is pending for the code delivery commit. |
| Scope level / execution budget | `L`; cross-crate observability boundary, 45-minute scoped code-delivery target excluding external Gates |
| Task Card | `gateway-observability` secret-safe JSON/Prometheus/OpenTelemetry projections plus `gateway-store` single-consumer writer fan-out only; no HTTP/Router export, second queue receiver, OTLP network client, management endpoint, body sampling, real Provider request, or P5 work |
| References | `G19`, `G20`, `G21`; [ADR-0030](../adr/ADR-0030-single-consumer-telemetry-fanout.md); [BC-OBS-003](../contracts/BC-OBS-003-single-consumer-telemetry-fanout.md) |

## Detailed subplan and invariant

1. Turn only already-admitted `GatewayEvent` values into fixed-schema structured JSON, bounded
   Prometheus exposition, and OpenTelemetry-compatible span records.
2. Preserve P4-07 ownership: attach the pipeline to `AsyncSqliteEventWriter` rather than adding a
   second `EventQueueReceiver` consumer.
3. Prove Request/Attempt/Usage trace parenting, absence of sensitive fields/labels, and one
   event's single-consumer fan-out to both telemetry and the durable event log.
4. Keep exporters non-blocking, record sink outcomes, and do not repeat telemetry on SQLite retry.

No response or Router path may serialize, log, enqueue to an exporter, write SQLite, wait for a
retry, read a body/header/URL, or send a Provider request. P4-09 exclusively owns body-sampling
policy and further log-redaction controls.

## Implemented scope

- Added `gateway-observability::TelemetryPipeline`, fixed safe JSON records, bounded Prometheus
  text exposition, `tracing` JSON adapter, and injected OpenTelemetry-compatible exporter port.
- Added deterministic W3C correlation: Request root, Attempt/Usage children, independent Health,
  and Diagnostic log/metric-only behavior.
- Attached the optional pipeline to the existing `AsyncSqliteEventWriter` admission point. It
  observes events once before durable batching; diagnostics are observable but remain
  non-persisted, and retrying a retained SQLite batch does not duplicate telemetry.
- Added cross-crate tests for Required and Diagnostic single-receiver fan-out; both prove
  telemetry observation while only Required events persist. A failed-then-recovered SQLite batch
  also proves one telemetry observation across retry.
- Updated explicit crate-boundary allowlists for only the new observability dependencies. Store's
  established one-way dependency on observability remains unchanged.

## Local targeted verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS after formatter normalization; no semantic correction. |
| `cargo test --locked -p gateway-observability -p gateway-store` | PASS; 36 tests, including fixed-label rendering, trace parenting, secret-safe JSON, single-consumer durable/export fan-out, and retry non-duplication. |
| `cargo clippy --locked -p gateway-observability -p gateway-store --all-targets --all-features -- -D warnings` | PASS after one documentation-markdown lint correction. |
| `ruby scripts/check-crate-boundaries.rb` and `ruby scripts/check-source-policy.rb` | PASS within the complete Gate; 21 crate boundaries and 66 Rust files. |
| `CHECK_REPORT_PATH=tmp/p4-08-full-check.md ./scripts/check.sh full` | Final PASS in 29 seconds (started `2026-07-21T20:14:51+08:00`, completed `2026-07-21T20:15:20+08:00`); shell/CI/plan guards, format, workspace Clippy/tests, source and crate policy, document links, Secret scan, whitespace, pinned quality tools, `cargo deny`, and RustSec audit all passed. |

No ignored real-test harness ran and no Provider request was sent.

## Review and execution measurement

Focused architecture review rejected the initial two-worker design because the P4-07 SQLite writer
already owns the only receiver. The delivery instead fans out inside that established consumer,
which preserves one queue delivery while ensuring telemetry has no request-path callback. Review
also checked that no metric label or serialized projection contains model, Client Key, Credential,
Endpoint, URL, header, or body material; the code-level regression tests cover representative
forbidden values.

The first focused Clippy run found one `doc_markdown` issue (`SQLite` needed backticks); it was
corrected directly before the passing rerun. Three earlier complete Gates also passed during the
architecture/document review; the final 29-second Gate reran after adding direct Diagnostic and
Attempt-field privacy coverage. No product-behavior rework followed the first complete Gate. GitHub
evidence is added only after its immutable Code Gate finishes.

## Closeout boundary

This code-delivery commit creates the P4-08 ADR, contract, report, and index entries. After the
GitHub Code Gate passes, exactly one docs-only closeout records that immutable evidence and changes
P4-08 to `DONE`. P4-09 remains `PENDING` until that docs-only Gate passes.
