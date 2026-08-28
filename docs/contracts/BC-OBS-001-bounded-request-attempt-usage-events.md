# BC-OBS-001 Bounded Request, Attempt, and Usage events

| Field | Value |
|---|---|
| Contract | `BC-OBS-001` |
| Task | `P3-08` |
| Status | IN_PROGRESS |
| Domain | Secret-safe structured lifecycle observations outside the response hot path |

## Entry and boundary

`gateway-core::GatewayEventSink` is a synchronous port. `gateway-observability::BoundedEventQueue`
implements it with finite Required and Diagnostic Tokio channels. `gateway-http-actix` emits
Request and final Usage observations; `gateway-router::AttemptOrchestrator` can emit one terminal
Attempt observation for every driver invocation through `start_with_event_sink`.

The event contract carries structural metadata only. It is not a Store/SQLite API, exporter,
tracing span, HTTP response, Provider decoder, or source of public model data. P4-07 owns durable
SQLite batch writing; P4-08/P4-09 own exporters, logging, and body-sampling policy.

## Required behavior

| Concern | Required behavior |
|---|---|
| Non-blocking emission | Callers invoke only `try_emit`; no event path awaits a channel, SQLite, network, or consumer. Admission outcomes never alter routing or HTTP outcome. |
| Request timing | Emit after successful authentication, decode, public-model resolution, and Request ID allocation, before executor start. Rejects before those points emit no Request event. |
| Request metadata | Retain Request ID, Client Key ID, optional Access Group, protocol, requested/public model, optional Alias, and stream mode. Exclude Body, messages, Tool data, headers, presented Key, and raw extensions. |
| Attempt cardinality | Emit exactly one terminal record for each actual Attempt driver call. No selection-only, no-binding, or pre-driver budget failure creates a fabricated Attempt. |
| Attempt metadata | Retain deterministic Attempt ID/sequence, safe Route/Candidate/Credential/Endpoint/Upstream identities, internal upstream model, injected-clock timestamps, safe outcome, and retry decision. No URL, Authorization, Credential bytes, raw status body, retry-after value, or free-form diagnostic is retained. |
| Usage timing | Emit only the final canonical Usage snapshot after the bounded stream accepts it; retain standardized token totals and drop raw Usage extensions. JSON and SSE use the same observation point. |
| Queue isolation | Required Request/Attempt/Usage records use a distinct bounded queue from low-priority diagnostics. Diagnostics cannot consume Required capacity. |
| Queue pressure | A full Diagnostic queue may return `DiagnosticDropped`. A full Required queue returns `RequiredQueueFull` and increments an inspectable counter; it must not block, silently wait, create an unbounded fallback, or change the response. |
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
| No configured sink | `Disabled`; execution continues with the no-op sink. |
| Required queue full | `RequiredQueueFull`; counter increments; execution continues. |
| Diagnostic queue full | `DiagnosticDropped`; counter increments; execution continues. |
| Receiver dropped | `SinkClosed`; counter increments; execution continues. |
| Attempt driver failure | Attempt record contains only the stable safe `GatewayError` and retry decision; the existing routing error remains authoritative. |

## Corresponding tests

- `gateway-core::gateway_event::tests` verifies deterministic Attempt IDs, redacted Debug forms,
  standard Usage copying, and Required/Diagnostic priority classification.
- `gateway-observability::tests` verifies finite capacity rejection, diagnostic isolation, explicit
  Required saturation, and priority-preferring consumption.
- `gateway-router::attempt_orchestrator::tests` verifies one terminal Attempt record per actual
  driver invocation across a fallback, including correlation, safe outcome, and retry decision.
- `gateway-http-actix::tests` verifies JSON Request/Usage correlation, Snapshot Access Group and
  Alias/public-model mapping, and that a saturated queue cannot block SSE delivery.
