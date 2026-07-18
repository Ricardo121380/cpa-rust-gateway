# BC-CORE-003 Canonical event state machine

| Field | Value |
|---|---|
| Contract | `BC-CORE-003` |
| Task | `P1-03` |
| Status | `DONE` |
| Domain | Framework-independent core |

## Entry

A Provider or deterministic mock emits `CanonicalEvent` values after it accepts a
`CanonicalRequest` and before a bounded stream transport or outbound protocol encoder consumes
them.

## Preconditions

- `gateway-core` receives protocol-neutral event semantics only: no HTTP, Actix, SSE framing,
  async stream trait, provider type, route, credential, endpoint, or cancellation primitive.
- Text, Reasoning, and Tool-argument deltas are retained as supplied fragments. This task does not
  parse incremental Tool JSON, normalize empty arguments to `{}`, or encode an external protocol.
- Every non-error event-specific unknown field is retained only under explicit raw `extensions`;
  it cannot collide with a canonical field or be emitted through diagnostics.

## Event sequence

```text
ResponseStart
  -> zero or more sequential MessageStart ... MessageEnd regions
       -> TextDelta | ReasoningDelta
       -> zero or more interleaved ToolCallStart ... ToolCallEnd regions
  -> zero or more UsageDelta updates at any valid point before ResponseEnd
  -> ResponseEnd

ResponseStart -> any valid partial sequence -> StreamError
```

## Invariants

- The canonical event vocabulary is exactly `ResponseStart`, `MessageStart`, `TextDelta`,
  `ReasoningDelta`, `ToolCallStart`, `ToolCallArgumentsDelta`, `ToolCallEnd`, `UsageDelta`,
  `MessageEnd`, `ResponseEnd`, and `StreamError`.
- A response begins once and terminates once. `ResponseEnd` and `StreamError` are terminal; no
  later semantic event is accepted.
- At most one Message is active at a time. Text and Reasoning deltas must be non-empty and belong
  to its active Message; a later sequential Message may begin only after the current one ends.
- A Tool Call begins once inside its active Message, has a stable non-empty correlation ID and
  non-empty name, accepts zero or more argument deltas only before its end, and cannot be reused.
  Its end carries complete valid JSON arguments but this task does not derive or normalize them.
  `ToolCallEnd` is the atomic `ArgumentsComplete -> Emitted` transition: `RawJson` proves that the
  already-assembled arguments are complete, while incremental assembly and normalization remain
  outside this core task. Multiple open Tool Calls may have argument deltas interleaved.
- Normal `ResponseEnd` requires all Messages and Tool Calls to have ended. A terminal StreamError
  may end an otherwise partial sequence without pretending that it completed normally.
- Usage updates may occur before a terminal event. An explicitly final Usage update is accepted at
  most once. A response may end normally whether an upstream reported no Usage updates, interim
  Usage updates, or a final Usage update; an interim report never requires a final report.
- Raw extensions remain explicit and opaque on non-error events. `StreamError` contains only a
  safe `GatewayError`, never raw upstream diagnostics. Event `Debug` output must redact client- or
  provider-supplied text, Tool names/arguments, IDs, and raw JSON.

## Error semantics

- An event that violates lifecycle order, correlation, or terminality is rejected as
  `UpstreamProtocolError` with `Stream` scope.
- Source completion or a normal `ResponseEnd` with an unclosed Response, Message, or Tool Call is
  rejected as `StreamTruncated` with `Stream` scope.
- P1-04 owns bounded delivery, backpressure, cancellation propagation, and first-semantic-event
  tracking. P1-05 and later own HTTP/SSE and target-protocol error encoding.

## Corresponding tests

- A desensitized successful fixture covers all non-error canonical events and raw extensions
  through JSON and memory. Unit tests cover `StreamError` separately.
- Unit tests cover a valid full sequence, interleaved Tool Call argument deltas, duplicate and
  out-of-order events, empty text/reasoning and Tool identifiers, sequential Messages, incomplete
  normal termination, interim/final Usage ordering, duplicate final Usage, terminality, and
  diagnostic redaction.
