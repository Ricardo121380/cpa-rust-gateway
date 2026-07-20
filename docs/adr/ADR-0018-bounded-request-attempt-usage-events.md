# ADR-0018: Bounded non-blocking Request, Attempt, and Usage event port

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-20` |
| Task / Matrix / Contract references | `P3-08`; `G19`, `G21`; `BL-09`, `BL-10`; [BC-OBS-001](../contracts/BC-OBS-001-bounded-request-attempt-usage-events.md) |

## Context

P3-06 can select, start, retry, and retain a Credential lease for one request-scoped Attempt, but
it intentionally produces no durable observation. P3-07 authenticates and force-maps a public
model at the HTTP boundary, but likewise records no Request or final Usage metadata. Release 1
requires Request/Attempt/Usage detail while preserving the data-path constraints: no SQLite read or
write, no exporter wait, no unbounded channel, no request/response body by default, and no Secret
or presented Client Key in diagnostics.

The existing crate graph deliberately prevents `gateway-router` from depending on
`gateway-observability`. The event contract must therefore be transport-neutral and live below both
the router and the queue implementation.

## Decision

- `gateway-core` owns serializable, secret-safe `GatewayEvent` domain types plus a synchronous
  `GatewayEventSink::try_emit` port. The port returns an explicit admission outcome and never
  returns a future, so an event queue, SQLite writer, network exporter, or consumer cannot make a
  request wait.
- `RequestEvent` retains only correlation and routing metadata: Request ID, non-secret Client Key
  ID, optional Access Group, protocol, requested/public model, optional input Alias, and stream
  mode. It contains no messages, Tool arguments, raw extensions, HTTP headers, or presented Key.
- `AttemptEvent` is emitted once after every actual P3-06 driver invocation reaches a terminal
  local outcome. It correlates a deterministic Attempt ID/sequence with non-secret Route,
  Candidate, Credential, Endpoint, Upstream, and internal upstream-model identities; it carries
  injected-clock start/end timestamps, a safe `GatewayError` when failed, and the bounded retry
  decision. Selecting no binding or exhausting budget before a driver starts does not fabricate an
  Attempt record.
- `UsageEvent` is emitted only for the one final canonical `UsageDelta`, after the bounded
  canonical stream has admitted it. It retains standard token totals and removes raw Usage
  extensions. Partial Usage snapshots, response text, Tool data, and arbitrary provider fields do
  not enter this event path.
- `gateway-observability::BoundedEventQueue` owns two independently bounded Tokio channels:
  Required Request/Attempt/Usage records and low-priority safe diagnostics. Diagnostics cannot
  consume required capacity. A diagnostic may be dropped when its queue is full; required-queue
  saturation returns `RequiredQueueFull` and increments a visible counter rather than blocking or
  silently waiting. P4 must surface these counters through the durable writer/metrics path.
- `gateway-http-actix` takes an optional explicit event sink. It emits Request after authentication,
  decode, Snapshot public-model resolution, and Request ID allocation but before executor start;
  it observes final Usage in both JSON and SSE flows. Existing constructors use a no-op sink until
  an embedding attaches a queue. `AttemptOrchestrator::start_with_event_sink` supplies the router
  hook without adding a reverse crate dependency.

## Consequences

The currently separable P3 slices gain correlated records without claiming that P3 already has a
real HTTP aggregation executor. P3-09 can attach the same sink to its integrated executor; P4-07
can consume `EventQueueReceiver` with a bounded SQLite batch writer. The request path remains
local and bounded even when no consumer exists or queues are saturated.

Required records can still be rejected only when their dedicated finite queue itself is exhausted;
the outcome is counted and observable rather than silently discarded. A future durable writer or
operational admission policy may react to that signal, but P3 must not replace it with a blocking
write, an unbounded overflow buffer, or an HTTP response failure.

## Alternatives considered

- Making `gateway-router` depend directly on `gateway-observability`: rejected because it reverses
  the frozen architecture direction and couples routing policy to a queue implementation.
- Writing SQLite directly from HTTP or Attempt orchestration: rejected because it violates `BL-09`
  and makes database latency part of the inference hot path.
- Awaiting `mpsc::send` when a queue is full: rejected because a stalled writer would stall JSON or
  SSE responses.
- Recording full request/response bodies or raw extensions for debugging: rejected because body
  capture is default-off and these values can contain client data, Tokens, or Secrets.
