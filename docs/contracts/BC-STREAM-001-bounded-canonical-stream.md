# BC-STREAM-001 Bounded canonical stream

| Field | Value |
|---|---|
| Contract | `BC-STREAM-001` |
| Task | `P1-04` |
| Status | `DONE` |
| Domain | Bounded canonical-event delivery |

## Entry

One single-source `CanonicalEvent` producer delivers a validated response sequence to one
downstream consumer through a bounded in-memory channel. The producer and consumer share only a
cancellation capability. The downstream consumer owns the separate FirstSemanticEvent tracker for
later HTTP/protocol adapters.

## Preconditions

- Capacity is explicit, strictly greater than zero, and no larger than Tokio's supported semaphore
  bound, so construction cannot panic. P1-04 has no global default capacity, slow-consumer timeout,
  byte-size limit, HTTP Header state, SSE keepalive, wire encoding, Provider task, route, retry
  policy, or persistence work.
- The stream validates events with `CanonicalEventState`. It carries only `CanonicalEvent`, not
  bytes or target-protocol frames.
- Only the downstream owner of `StreamControl` may mark FirstSemanticEvent, and only after a
  canonical event has successfully crossed the client-visible output boundary. The producer has no
  tracker capability. Enqueue and dequeue alone are not delivery.

## Event sequence

```text
CanonicalEvent source
  -> validate one event
  -> await bounded capacity
  -> downstream receives the same ordered event
  -> downstream explicitly marks the delivered semantic event

ResponseEnd | StreamError
  -> close the source after the terminal event
  -> downstream finishes normally

source closes before a terminal event
  -> downstream receives StreamTruncated/Stream once

client cancellation or downstream drop before terminal
  -> shared cancellation signal
  -> blocked or later producer operation receives Cancelled/Request
```

## Invariants

- One stream permits one producer and one consumer. `send` preserves input order and waits for
  capacity; it never silently drops a canonical event. The bound limits event count, not the byte
  size of individual event payloads.
- Invalid lifecycle/order events are rejected before they occupy channel capacity as
  `UpstreamProtocolError` with `Stream` scope. A normal source close without `ResponseEnd` or
  `StreamError` is `StreamTruncated` with `Stream` scope.
- A valid `StreamError` is an ordinary terminal `CanonicalEvent` delivered as `Ok(event)`; it is
  not replaced with a transport error.
- Cancellation is idempotent. It does not synthesize `StreamError`, does not enqueue further
  events, and returns `Cancelled` with `Request` scope to in-flight or later transport operations.
  Dropping the consumer before it observes a terminal event triggers this same cancellation.
- FirstSemanticEvent begins uncommitted and changes monotonically once. All `CanonicalEvent`
  variants are semantic candidates; no P1-04 keepalive type exists, so later SSE comments must not
  call the tracker. The tracker itself reports only committed/uncommitted state. A transparent
  retry is permitted only while it is uncommitted *and* cancellation has not been requested; after
  downstream delivery or client cancellation, it is not permitted.

## Error semantics

- Capacity exhaustion is backpressure, not an error or event drop.
- Source/lifecycle sequence errors retain the P1-03 classifications.
- Client cancellation remains a request-owned cancellation; it does not imply a Provider,
  Credential, or Stream-invalidity classification.

## Corresponding tests

- Unit tests reject zero and overlarge capacity, preserve a valid terminal sequence, and report a
  truncated source exactly once.
- Slow-consumer and capacity tests prove that a full channel blocks the next send until one event
  is received, without reordering or loss.
- Cancellation tests cover explicit cancellation, consumer drop, and a producer blocked on full
  capacity.
- FirstSemanticEvent tests prove that enqueue and dequeue retain the uncommitted state until
  explicit downstream delivery, after which it is irreversible; cancellation leaves FSE
  uncommitted but forbids a transparent retry.
