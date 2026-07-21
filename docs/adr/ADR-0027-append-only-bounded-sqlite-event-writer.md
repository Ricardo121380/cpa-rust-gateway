# ADR-0027: Append-only bounded SQLite event writer

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Task / Matrix / Contract references | `P4-07`; `G19`, `G20`, `G21`; `BL-09`, `BL-10`; [BC-OBS-002](../contracts/BC-OBS-002-append-only-sqlite-event-writer.md) |

## Context

P3-08 deliberately stops at a synchronous `GatewayEventSink` and two bounded Tokio queues. That
keeps Request, Attempt, and Usage delivery out of HTTP and routing latency, but it cannot retain a
timeline over a process restart. P4 additionally needs sanitized Health transitions to be
correlated with those request observations, without allowing a Store dependency to leak back into
the event producer or retaining URL, Header, body, presented key, credential value, or Provider
diagnostic text.

The event writer must distinguish durable success from a temporary SQLite failure. Claiming a
Diagnostic was persisted, blocking a response while a transaction is slow, or adding an unbounded
in-memory fallback would all violate the existing P3 data-path boundary.

## Decision

- `gateway-core` owns a serializable, Required-priority `HealthEvent` with a stable
  `HealthEventId`, exact non-secret Endpoint/Credential/model scope, explicit timestamp, and a
  small transition enum. Its `Debug` form redacts the access-controlled upstream model label.
- `gateway-store` migration `0005` creates append-only `gateway_event_log`. It stores a validated
  event type, stable event id, optional Request correlation, optional event timestamp, and typed
  JSON payload. `(event_type, event_id)` is unique. Identical replays are no-ops; a replay with
  different payload fails the entire batch rather than overwriting history.
- `SqliteEventStore` writes each supplied Required batch in one immediate transaction, reloads it
  only after re-decoding the payload and cross-checking indexed metadata, supports request and
  health timelines, and accepts only the exact `PRAGMA quick_check = ok` integrity result.
- `AsyncSqliteEventWriter` is the sole P4-07 consumer of
  `gateway_observability::EventQueueReceiver`. It moves batch writes onto Tokio's blocking pool,
  retains one finite pending Required batch after an open/migration/transaction/worker failure,
  retries after a positive delay, and exposes pending/failure/commit counters. It does not add an
  overflow queue.
- Diagnostics remain deliberately non-persisted by this Required-event log. The writer records a
  separate `diagnostics_not_persisted` counter, so metrics and later exporters cannot present them
  as durable records.
- The dependency is one-way: Store may consume the observability receiver, while observability
  remains independent of Store. Router and HTTP continue to invoke only `try_emit` and cannot wait
  for the writer, SQLite, a retry, or a recovery check.

## Consequences

Committed Request/Attempt/Usage records survive a file-database reopen and retain their Request
correlation; Health records use their own durable identity and timeline. The writer makes a
temporary database failure visible while holding at most its configured batch in addition to the
already finite producer queues. A full Required queue remains an explicit non-blocking producer
outcome, not a response stall.

Only successfully committed events survive a process crash. A live pending batch is intentionally
not claimed as crash-durable; it remains visible in writer metrics until a transaction commits.
P4-08 can export the writer counters, while P4-09 owns log redaction and body sampling. This task
does not produce Health probes, mutate runtime health, classify quota, expose a management API, or
send a Provider request.

## Alternatives considered

- Direct SQLite writes from Actix or Router: rejected because durable I/O would enter the request
  hot path.
- An unbounded overflow vector after SQLite failure: rejected because it hides backpressure and
  can exhaust memory.
- Persisting Diagnostics in the Required event table: rejected because their intentional
  drop-eligibility would make a durable-history claim misleading.
- Blind `INSERT OR IGNORE` for conflicting replays: rejected because it could silently retain a
  stale record under a reused id.
- A reverse `gateway-observability -> gateway-store` dependency: rejected because event producers
  must stay transport- and persistence-independent.

## Validation and rollback

Synthetic tests cover atomic rollback, identical replay idempotence, conflict refusal,
file-database reopen, Request/Attempt/Usage/Health correlation, `quick_check`, queue saturation
without blocking, Diagnostic accounting, and a missing-directory SQLite failure that retains then
commits its pending batch after recovery. No test performs a network request.

Rollback removes migration `0005` through the normal migration rollback path and removes the P4-07
writer/Health event contract. It does not change RouteSnapshot, HTTP behavior, Provider traffic,
Secret storage, quota, runtime Circuit behavior, or the P3 bounded queue admission semantics.
