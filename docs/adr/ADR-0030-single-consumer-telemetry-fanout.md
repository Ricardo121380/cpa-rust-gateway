# ADR-0030: Single-consumer telemetry fan-out through the durable event writer

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Task / Matrix / Contract references | `P4-08`; `G19`, `G20`, `G21`; [BC-OBS-003](../contracts/BC-OBS-003-single-consumer-telemetry-fanout.md) |

## Context

P3-08 provides a bounded, non-blocking `GatewayEventSink`; P4-07 makes
`AsyncSqliteEventWriter` its single `EventQueueReceiver` consumer. P4-08 must add structured
JSON, Prometheus, and OpenTelemetry-compatible observations without stealing events from durable
storage, waiting in the request path, or introducing bodies, headers, URLs, Client Keys,
credentials, Endpoint identities, model labels, or arbitrary Provider diagnostics into telemetry.

Creating an independent telemetry worker with the same receiver would make two consumers race for
one event. One sink could then observe an event that the other never receives, which would split
the durable and exported timelines and invalidate P4-07's single-consumer ownership.

## Decision

- `gateway-observability::TelemetryPipeline` converts only admitted `GatewayEvent` values and
  accepts injected `try_export` JSON and OpenTelemetry destinations. Both ports are explicitly
  non-blocking; rejection and disabled states are counted rather than retried on the request path.
- `AsyncSqliteEventWriter` remains the only receiver consumer. Its optional
  `with_telemetry_pipeline` builder calls `observe_event` exactly once at `accept_event`, before
  sorting the event into a finite Required batch or the non-persisted Diagnostic path. A failed
  `SQLite` retry does not re-export the retained pending batch.
- Structured JSON has a fixed schema and only safe fields: Request correlation ID, protocol,
  streaming flag, stable Attempt outcome/retry/error categories, bounded usage counters, and
  Health kind. A tracing JSON adapter may log only that serialized record.
- Prometheus text exposition has only frozen low-cardinality labels: event kind, Attempt outcome,
  usage field, exporter sink/outcome, and queue-admission outcome. Request IDs and all target,
  client, credential, and model identities are never labels.
- OpenTelemetry-compatible records carry deterministic, nonzero W3C IDs derived from safe stable
  IDs. Request is root `gateway.request`; Attempt and Usage are its children; Health is an
  independent `gateway.health`; Diagnostic creates no span. P4-08 deliberately does not trust or
  propagate an inbound `traceparent` header.
- The existing one-way dependency remains: `gateway-store -> gateway-observability`; no Store,
  `SQLite`, network exporter, or retry capability is exposed to HTTP, Router, or the event
  producer. An embedding that needs OTLP transport owns a bounded non-blocking exporter adapter.

## Consequences

Every admitted event can be made durable and observable through one queue consumption. Required
events remain durable only after the transaction commits; telemetry is a best-effort, explicitly
counted export and never falsely implies persistence. Diagnostics reach JSON/metrics while keeping
their P4-07 non-persisted status.

The event writer can spend bounded background work invoking exporters, but response and routing
callers still invoke only synchronous `try_emit`. An exporter must honor its non-blocking contract;
it may reject rather than create a hidden unbounded queue or perform I/O inline.

## Alternatives considered

- A second telemetry receiver worker: rejected because it competes with the P4-07 sole consumer
  and splits event delivery.
- Direct JSON/Prometheus/OTLP export from HTTP or Router: rejected because it places serialization,
  logging, queueing, or network behavior in response latency.
- Per-request or target labels in Prometheus: rejected because uncontrolled cardinality can exhaust
  the metrics registry and exposes access-controlled identifiers.
- Re-exporting after each `SQLite` retry: rejected because it inflates telemetry while durable
  idempotence treats the retained event as one admission.
- Inbound trace-header propagation now: rejected because its trust boundary and client identity
  policy have not been designed; deterministic internal correlation is sufficient for P4.

## Validation and rollback

Targeted tests prove Request/Attempt/Usage parent correlation, secret-safe structured JSON,
bounded Prometheus labels, writer fan-out to both telemetry and the SQLite log, and single export
across a failed-then-recovered durable batch. Targeted tests and Clippy pass with no Provider
request. The complete local Gate and GitHub Code Gate provide the delivery evidence.

Rollback removes the pipeline, `AsyncSqliteEventWriter` builder hook, telemetry dependencies, and
P4-08 documentation. It restores P4-07's durable-only receiver consumer without changing the
bounded event admission port, event schema, `SQLite` migration, Router/HTTP behavior, Provider
traffic, or Secret storage.

## Amendment (2026-07-26, P12 serve composition)

The P12 serve composition attaches the pipeline to its production writer unchanged, with the
`TracingJsonExporter` rendering structured JSON lines through the process-global `tracing`
subscriber installed at serve start (stdout/journald) and the no-op OpenTelemetry exporter. The
shared `PrometheusMetrics` registry becomes the source of the protected management exposition
`GET /admin/observability/metrics`, which mirrors the queue admission counters at scrape time and
renders only the frozen bounded label set.
