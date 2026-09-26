# BC-OBS-002: Append-only SQLite Request/Attempt/Usage/Health event writer

| Field | Value |
|---|---|
| Contract | `BC-OBS-002` |
| Task | `P4-07` |
| ADR | [ADR-0027](../adr/ADR-0027-append-only-bounded-sqlite-event-writer.md) |
| Extends | [BC-OBS-001](BC-OBS-001-bounded-request-attempt-usage-events.md) P3 bounded event admission |
| Domain | Durable, append-only, non-hot-path lifecycle observation |

## Entry and boundary

`gateway-observability::BoundedEventQueue` remains the only response-path event admission point.
`gateway-store::AsyncSqliteEventWriter` later consumes its single `EventQueueReceiver`; it is not a
`GatewayEventSink`, an HTTP handler, a Router dependency, a Provider, an exporter, or a control
plane mutation path. It writes only already-admitted Required `Request`, `RequestFinished`, `Attempt`, `Usage`, and `Health` events.

`HealthEvent` contains a stable `HealthEventId`, Endpoint, optional Credential, optional internal
model label, explicit timestamp, and one sanitized kind. It excludes URL, headers, body, status
text, credential bytes, and arbitrary diagnostics. Its upstream model label is redacted from
`Debug` but can be serialized only for access-controlled durable consumers.

The P12 serve composition (`gateway::runtime::build_data_plane_composition`) instantiates this
contract with the M1 confirmation amendment: one `BoundedEventQueue` fans out from the data-plane sink, one
`AsyncSqliteEventWriter` consumes its receiver against the control database, the deployment
envelope spawns the writer only after both listeners bind, and stop joins it with a bounded flush
wait whose expiry is an explicit deployment failure rather than a fabricated flush.

## Durable record rules

| Event | Event type / stable id | Request correlation | Explicit event time |
|---|---|---|---|
| Request | `request` / `RequestId` | Same `RequestId` | New observed start time; legacy None |
| RequestFinished | `request_finished` / `RequestId` | Same `RequestId` | Observed finish time |
| Attempt | `attempt` / deterministic `AttemptId` | Source `RequestId` | Terminal `ended_at_ms` |
| Usage | `usage` / request-scoped SHA256 ID; legacy ResponseId preserved on replay | Source RequestId, optional explicit AttemptId, ResponseId | None |
| Health | `health` / `HealthEventId` | None | `occurred_at_ms` |

`gateway_event_log` is append-only. Its `(event_type, event_id)` unique key makes an identical
replay a no-op. A matching key with different payload fails the full transaction; it never updates
or replaces the prior durable event. On read, the payload is decoded into `GatewayEvent` and its
derived type/id/correlation/time must match the indexed columns or the row fails closed.

## Writer timeline and backpressure

```text
HTTP / Router: GatewayEventSink::emit_confirmed (bounded async wait, no synchronous SQL)
        |
        v
bounded Required queue -- full --> RequiredQueueFull (reject request / stop retry or successful output)
        |
        v
AsyncSqliteEventWriter finite pending batch
        |
        +-- SQLite transaction succeeds --> Persisted receipt, counters, batch cleared
        |
        +-- transient SQLite failure --> pending batch retained, failure counter, positive-delay retry
        |
        +-- deterministic record failure --> one-event transactions: healthy events commit, poisoned events durably quarantined and failure receipt returned
```

The pending batch is capped by `EventWriterConfig::batch_size` (maximum `1024`). The writer opens
or migrates a file connection and executes SQLite work only on Tokio's blocking pool. A failure
does not block the producer, fabricate a durable success, create an unbounded fallback, or pull
additional Required events into the pending batch. It does leave the finite producer queue to exert
its existing explicit backpressure.

A deterministic record-level failure (`ConflictingGatewayEventReplay`,
`InvalidPersistedGatewayEvent`, or `DiagnosticEventNotPersistable`) cannot be repaired by retrying
the same batch. The writer then replays that batch one event per transaction in original order:
every healthy event commits durably, each poisoned event is retained in `gateway_event_quarantine` (fingerprint, safe classification, bounded typed payload), counted in `required_events_quarantined`, and explicitly fails confirmation, and a transient failure during the replay stops the pass with the
interrupted event and unprocessed suffix retained as the pending batch. A quarantine write failure retains the event in the finite pending batch. Replays of the same poison retain one fingerprint, not an infinite set of suffix identities.

Diagnostics are drained from their lower-priority queue but not written to the Required event log.
Each such event increments `diagnostics_not_persisted`; it is not included in
`required_events_committed` or `rows_inserted`.

## Recovery and error semantics

| Condition | Required result |
|---|---|
| Empty or over-limit batch configuration | `EventWriterConfigError`; writer is not created. |
| Transient database open/migration/transaction/blocking worker failure | Current finite Required batch remains pending; `sqlite_write_failures` increments and retry uses a positive delay. |
| Deterministic record failure inside a pending writer batch | Per-event replay: healthy events commit, each poisoned event commits quarantine evidence and increments `required_events_quarantined`; a transient interruption keeps unprocessed events pending. |
| Identical replay after retry/restart | No duplicate row; batch transaction succeeds. |
| Same durable key, different payload | `StoreError::ConflictingGatewayEventReplay`; no partial batch write. |
| Diagnostic passed to synchronous Store API | `StoreError::DiagnosticEventNotPersistable`; no row. |
| Malformed indexed row or payload on read | `StoreError::InvalidPersistedGatewayEvent`; no partially decoded timeline. |
| `PRAGMA quick_check` result other than `ok` | `StoreError::GatewayEventLogIntegrityCheckFailed`. |
| Process crashes before a pending batch commits | Only prior committed rows are recoverable; the design does not claim uncommitted memory is durable. |

## Invariants

- Requests await a bounded background confirmation; no Actix/Router synchronous SQL, unbounded queue-slot wait, or per-chunk persistence.
- `gateway-observability` has no Store dependency; Store's receiver dependency is one-way.
- A transaction either appends all new valid rows or none; idempotent replay does not duplicate.
- Request query order is durable append order, not application wall-clock inference.
- Diagnostics never masquerade as durable Required events.
- One unappendable record never loops forever as a healthy batch: only that record is durably isolated, then healthy records continue. Storage outage can still fail closed for all new requests.
- Bodies, message content, Tool arguments, headers, cookies, presented keys, credential bytes,
  URLs, status text, raw extensions, and Provider diagnostic text do not enter this event schema.

## Corresponding tests

- `gateway-store::event_store::tests::batches_are_atomic_and_identical_replays_are_idempotent`
- `gateway-store::event_store::tests::file_reopen_restores_request_attempt_usage_and_health_and_quick_check`
- `gateway-store::event_store::tests::diagnostics_cannot_be_mistaken_for_durable_required_events`
- `gateway-store::event_store::tests::full_required_queue_stays_non_blocking_before_writer_consumption`
- `gateway-store::event_store::tests::writer_retains_failed_pending_batch_then_recovers_when_database_becomes_available`
- `gateway-store::event_store::tests::writer_counts_diagnostics_without_persisting_them`
- `gateway-store::event_store::tests::writer_quarantines_conflicting_replay_and_commits_healthy_batch_events`
- `gateway-store::event_store::tests::writer_quarantines_poisoned_singleton_batch_and_keeps_consuming`
- `gateway-store::event_store::tests::transient_failure_during_quarantine_fallback_preserves_healthy_events`
- `cargo test --locked -p gateway-core -p gateway-observability -p gateway-store`
- `cargo clippy --locked -p gateway-core -p gateway-observability -p gateway-store --all-targets --all-features -- -D warnings`
- `./scripts/check.sh full`

## M1 recovery and health (2026-09-26)

Schema29 adds `gateway_request_recording` and `gateway_event_quarantine`, without updating original
events or ledger. A timestamped new Request creates an active recording in the same transaction.
Its actual RequestFinished changes that state; reverse arrival order is supported. Writer startup
marks prior active recordings interrupted. Their request history result remains unknown, not a
fabricated successful or cancelled terminal. Legacy unknown events are not given invented times.

Protected `/admin/observability/metrics` exposes state (starting/ready/storage unavailable/closed),
accepting confirmed admission, unconfirmed required count, last successful commit, confirmation
failures, and recovered unknown count. On forced writer abort, unconfirmed counts intentionally
remain owed; zero is not fabricated by dropping memory. Billing processing health remains separate.

Rollback uses an isolated tested copy. A production rollback needs the pre-upgrade full snapshot:
schema28 readers deny unknown new event fields, so changing only the binary is unsafe. Do not
delete current history as a shortcut. Historical reports retain their original test claims and do
not override this amendment's fail-closed semantics.

## M1 regression tests

- `acknowledgements_follow_commit_and_poison_survives_restart`
- `locked_database_fails_closed_then_recovers_without_replaying_upstream`
- `full_queue_and_closed_writer_never_acknowledge_durability`
- `out_of_order_explicit_lineage_and_state_are_idempotent`
- `process_kill_at_each_confirmed_boundary_recovers_only_known_facts`
- `historical_production_projection_survives_migration_and_partial_read`
