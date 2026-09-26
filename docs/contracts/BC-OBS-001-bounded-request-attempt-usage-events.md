# BC-OBS-001 Bounded Request, Attempt, and Usage events

| Field | Value |
|---|---|
| Contract | `BC-OBS-001` |
| Task | `P3-08` |
| Status | Implemented; M1 reliability amendment 2026-09-26 |
| Domain | Secret-safe structured lifecycle observations outside the response hot path |

## Entry and boundary

`gateway-core::GatewayEventSink` offers nonblocking `try_emit` and bounded asynchronous `emit_confirmed`. Production required paths use confirmation; queue-only test embeddings and explicit Noop embeddings are not durability evidence. `gateway-observability::BoundedEventQueue`
implements it with finite Required and Diagnostic Tokio channels. `gateway-http-actix` emits
Request and final Usage observations; `gateway-router::AttemptOrchestrator` can emit one terminal
Attempt observation for every driver invocation through `start_with_event_sink`.

The event contract carries structural metadata only. It is not a Store/SQLite API, exporter,
tracing span, HTTP response, Provider decoder, or source of public model data. P4-07 owns durable
SQLite batch writing; P4-08/P4-09 own exporters, logging, and body-sampling policy.

## Required behavior

| Concern | Required behavior |
|---|---|
| Non-blocking runtime | No synchronous SQL on Actix/Router. Request/Attempt/final Usage and an awaited terminal asynchronously confirm background commit, at most two seconds per confirmation. JSON terminal handling follows the actual body handoff described below. No per-chunk writes. |
| Request timing | Emit after successful authentication, decode, public-model resolution, and Request ID allocation, before executor start. Authenticated decode rejections also retain safe metadata; unresolvable model fields remain empty. Unauthenticated traffic does not fabricate a Client Key identity. |
| Request metadata | Retain Request ID, Client Key ID, optional Access Group, protocol, requested/public model, optional Alias, stream mode, and optional observed start time (absent in legacy rows). Exclude Body, messages, Tool data, headers, presented Key, and raw extensions. |
| Attempt cardinality | Emit exactly one terminal record for each actual Attempt driver call. No selection-only, no-binding, or pre-driver budget failure creates a fabricated Attempt. |
| Attempt metadata | Retain deterministic Attempt ID/sequence, safe Route/Candidate/Credential/Endpoint/Upstream identities, internal upstream model, injected-clock timestamps, safe outcome, and retry decision. No URL, Authorization, Credential bytes, raw status body, retry-after value, or free-form diagnostic is retained. |
| Usage timing | Validate lifecycle, then confirm the final canonical Usage snapshot before sending it downstream; new routed events include the exact confirmed request-local Attempt ID and Response ID; retain standardized token totals and drop raw Usage extensions. JSON and SSE use the same observation point. |
| Queue isolation | Required Request/Attempt/Usage records use a distinct bounded queue from low-priority diagnostics. Diagnostics cannot consume Required capacity. |
| Queue pressure | A full Diagnostic queue may return `DiagnosticDropped`. A full Required queue returns `RequiredQueueFull` and increments an inspectable counter; it must not block, silently wait, create an unbounded fallback, or invoke/retry an upstream when confirmation failed. Return safe `RecordingUnavailable` (HTTP503 before headers); after headers end as a failure, never manufacture a successful terminal. |
| Privacy | `Debug` redacts model values that are not public transport output; events contain no raw client content or Secret. Serialization is for access-controlled future persistence, not a public endpoint. |

## Invariants

- One Request can have zero or more Attempt events and at most one final Usage event from this P3
  Responses path.
- An Alias Request event records the original Alias and stable public model; it never changes the
  client-visible P3-07 response mapping.
- `AttemptId` is deterministically scoped by its Request ID and one-based Attempt sequence.
- Required queue overflow is explicitly observable through the return value/counter. A Diagnostic
  burst cannot cause it.
- The event queue owns no global scheduler lock, persistence handle, source task, Credential lease,
  RouteSnapshot publication, or runtime health mutation.

## Error semantics

| Condition | Result |
|---|---|
| Explicit non-production Noop embedding | `Disabled`; no durability claim. Production serve always attaches one durable writer. |
| Required queue full | `RequiredQueueFull`; counter increments; required confirmation fails closed. |
| Diagnostic queue full | `DiagnosticDropped`; counter increments; execution continues. |
| Receiver dropped | `SinkClosed`/`PersistenceUnavailable`; required confirmation fails closed. |
| Attempt driver failure | Attempt record contains only the stable safe `GatewayError` and retry decision; a recording failure prevents output or another retry; the admitted Request remains queryable even if no reliable Attempt terminal survives. |

## Corresponding tests

- `gateway-core::gateway_event::tests` verifies deterministic Attempt IDs, redacted Debug forms,
  standard Usage copying, and Required/Diagnostic priority classification.
- `gateway-observability::tests` verifies finite capacity rejection, diagnostic isolation, explicit
  Required saturation, and priority-preferring consumption.
- `gateway-router::attempt_orchestrator::tests` verifies one terminal Attempt record per actual
  driver invocation across a fallback, including correlation, safe outcome, and retry decision.
- `gateway-http-actix::tests` verifies JSON Request/Usage correlation, Snapshot Access Group and
  Alias/public-model mapping, and that a saturated required queue rejects admission or prevents a successful streaming terminal.

## M1 confirmation and crash boundary

A queued record is not a committed record. Production fanout forwards confirmations to the attached
writer. Request commit precedes execution; every actual Attempt terminal is confirmed before retry/output.
Final Usage is optional when an upstream provides none: missing usage remains unknown, never zero;
a successfully delivered response does not claim priced or complete usage. Drop/cancellation cannot
await, so its bounded terminal admission is checked; rejection is visible and the committed Request
remains unknown. Killing a process can lose observations not yet committed, never the acknowledged
Request identity. See [CR](../change-requests/CR-20260926-required-event-durability.md).

- `required_admission_full_rejects_every_http_protocol_before_executor`
- `unavailable_sqlite_writer_rejects_http_before_executor`
- `saturated_usage_queue_cannot_report_success`

For JSON, success is observed only after its body bytes are handed to Actix. A final body poll
awaits confirmation; if Actix drops a sized body immediately after its only chunk, Drop checks
bounded admission instead. A body dropped before handoff records cancellation. No terminal
acknowledgement before handoff is used to falsely label a cancelled request successful.
This is a server handoff observation, not proof the remote client received every byte.

- `json_drop_records_success_only_after_body_handoff`
- `failed_attempt_recording_stops_retry_and_releases_lease`
- `explicit_usage_attempt_does_not_follow_a_later_attempt`

- `sse_end_can_be_polled_again_while_terminal_commit_is_pending` prevents repeated exhausted-source polling while the durable terminal acknowledgement yields.
