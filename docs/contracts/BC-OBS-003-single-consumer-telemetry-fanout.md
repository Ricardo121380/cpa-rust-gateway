# BC-OBS-003: Single-consumer structured telemetry fan-out

| Field | Value |
|---|---|
| Contract | `BC-OBS-003` |
| Task | `P4-08` |
| ADR | [ADR-0030](../adr/ADR-0030-single-consumer-telemetry-fanout.md) |
| Extends | [BC-OBS-001](BC-OBS-001-bounded-request-attempt-usage-events.md) and [BC-OBS-002](BC-OBS-002-append-only-sqlite-event-writer.md) |
| Domain | Secret-safe background telemetry after bounded event admission |

## Entry and ownership

`gateway-observability::BoundedEventQueue` remains the sole response-path admission point and
`gateway-store::AsyncSqliteEventWriter` remains its sole `EventQueueReceiver` consumer. An
optional `TelemetryPipeline` is attached to that writer; it is not a `GatewayEventSink`, HTTP
handler, Router dependency, second receiver, synchronous `SQLite` writer, or network executor.

For every event delivered to the writer, the pipeline runs once at admission before durable batch
handling. The same event is not re-exported because its `SQLite` transaction retries. Diagnostics
are observed but retain their BC-OBS-002 status as non-persisted records.

The P12 serve composition attaches this pipeline unchanged: the shared `PrometheusMetrics`
registry it aggregates is also the source of the protected management exposition
`GET /admin/observability/metrics`, which renders only the frozen bounded counters plus the
scrape-time-mirrored queue admission counters and never reads the durable log or blocks a scrape
on `SQLite`. Structured JSON records render through the process-global `tracing` subscriber.

```text
HTTP / Router: GatewayEventSink::try_emit (never awaits)
        |
        v
bounded Required / Diagnostic queues
        |
        v
AsyncSqliteEventWriter (single consumer)
        +-- TelemetryPipeline::observe_event once --> JSON / Prometheus / OpenTelemetry sink
        |
        +-- Required --> finite pending batch --> SQLite transaction / bounded retry
        +-- Diagnostic --> non-persisted counter
```

## Event projections

| Source event | Structured JSON / Prometheus | OpenTelemetry-compatible span |
|---|---|---|
| Request | request correlation, protocol, streaming; event counter | root `gateway.request` Server span |
| Attempt | request correlation, terminal outcome, retry decision, stable error category; outcome counter | child `gateway.upstream.attempt` Client span |
| Usage | request correlation and numeric usage fields; token counters | child `gateway.usage` Internal span |
| Health | sanitized Health kind; event counter | independent `gateway.health` Internal span |
| Diagnostic | stable error code/scope; event counter | no span |

The JSON schema and span attributes must exclude request/response bodies, message content, Tool
arguments, headers, cookies, URLs, Endpoint identity, Client Key material, Credential material,
raw model labels, raw status text, extensions, and arbitrary diagnostic text.

## Metrics and tracing invariants

- Prometheus labels are fixed to event kind, Attempt outcome, usage field, exporter sink/outcome,
  and producer queue-admission outcome. No request-scoped or target-scoped label is permitted.
- Each emitted Request/Attempt/Usage correlation uses a valid nonzero W3C trace ID. Attempt and
  Usage share their Request trace ID and reference the Request root span ID. Health is independent;
  Diagnostic has no OpenTelemetry span.
- Trace identifiers derive from stable safe event IDs and must not contain a raw Client Key,
  Credential, Endpoint, model label, URL, header, body, or inbound tracing header.
- JSON and OpenTelemetry sinks expose only a synchronous `try_export` operation. `Emitted`,
  `Disabled`, and `Rejected` outcomes are explicitly counted; a rejection does not block the
  producer, retry a durable batch, or fabricate export success.
- `try_init_json_tracing` may install a process-global JSON subscriber only once. Embeddings that
  own tracing globally install their own subscriber and may still use `TracingJsonExporter`.

## Failure and recovery semantics

| Condition | Required result |
|---|---|
| No telemetry pipeline configured | Writer preserves P4-07 durable behavior without export calls. |
| JSON sink disabled or rejects | Record remains eligible for P4-07 persistence; corresponding fixed counter changes. |
| OpenTelemetry sink disabled or rejects | JSON/metrics behavior continues; only trace export counter changes. |
| SQLite batch fails then later recovers | Event remains one telemetry observation and one eventual durable row, not multiple exports. |
| Diagnostic reaches writer | Pipeline may count/export it; it is never inserted into `gateway_event_log`. |
| Queue is saturated or closed | Producer returns its existing non-blocking admission result; no telemetry callback occurs for the rejected event. |

## Corresponding tests

- `gateway-observability::telemetry::tests::request_attempt_and_usage_share_one_w3c_trace`
- `gateway-observability::telemetry::tests::structured_json_and_otel_exclude_target_and_identity_material`
- `gateway-observability::telemetry::tests::prometheus_rendering_uses_only_bounded_labels`
- `gateway-store::event_store::tests::writer_fans_out_one_admitted_event_to_store_and_telemetry_without_a_second_receiver`
- `gateway-store::event_store::tests::writer_retains_failed_pending_batch_then_recovers_when_database_becomes_available`
- `gateway-store::event_store::tests::writer_counts_diagnostics_without_persisting_them`
- `cargo test --locked -p gateway-observability -p gateway-store`
- `cargo clippy --locked -p gateway-observability -p gateway-store --all-targets --all-features -- -D warnings`
- `./scripts/check.sh full`
