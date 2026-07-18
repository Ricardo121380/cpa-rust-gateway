# BC-PROVIDER-001 Deterministic Mock Provider

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-001` |
| Task | `P1-06` |
| Status | `DONE` |
| Domain | Core-only Provider execution boundary |

## Entry and boundary

P1-06 defines only the smallest inference capability in `gateway-provider`:

```text
RequestContext + CanonicalRequest
  -> InferenceAdapter::execute
  -> CanonicalEventSource::next_event
  -> CanonicalEvent | EOF | GatewayError
```

`ProviderAdapter` supplies only a stable `ProviderId`. `InferenceAdapter` supplies one execution
method. `CanonicalEventSource` is pull-only and returns one ordered canonical event at a time.
Public traits use a boxed standard-library Future so no async-trait macro, Actix type, stream
sender, Provider SDK, HTTP body, route, endpoint, credential, persistence, or retry type leaks
into the boundary.

P1-07 owns composition with the P1-04 bounded stream, HTTP cancellation, response headers,
OpenAI SSE/JSON encoding, and FirstSemanticEvent after a successful client write.

## Error and lifecycle ownership

- A failure before `ResponseStart` is returned from `execute` as its existing safe
  `GatewayError`; no source or protocol frame is fabricated.
- Once a source has begun, a Provider-originated failure must be a terminal
  `CanonicalEvent::StreamError`, not an out-of-band `next_event` error. The core lifecycle is the
  one authority for ordering and terminality.
- A fixture's event script is fully checked by `CanonicalEventState::apply` plus `finish` at
  construction. An invalid, duplicate-terminal, or truncated script is rejected before execution.
- A completed script reaches EOF by returning `Ok(None)`. The mock never invents a response,
  Usage, Tool result, diagnostic, or retry.

## Deterministic Mock semantics

- `MockFixture::try_events` stores an immutable, validated script. Every `execute` creates a fresh
  source from the same script, independent of prior executions, requested model, or global state.
- `MockEmission` delays are relative to the corresponding pull. The source waits with Tokio only
  when a nonzero delay is configured; it does not spawn a background task or enqueue into an
  unbounded buffer.
- Dropping a pending pull does not consume its event. Dropping the source leaves no mock-owned
  producer task behind.
- `MockFixture::pre_start_error` models a deterministic pre-response error. A scripted
  `StreamError` models an error after response start; the two cases are intentionally distinct.
- Fixture data is desensitized and test-only. Text, Tool, error, and delay fixtures cover the
  public P1 capability without pretending to be a real upstream Provider.

## Corresponding tests

- Repeated text execution emits equal ordered canonical events and a normal EOF.
- Tool fixtures preserve Tool start, argument delta, and complete raw JSON arguments.
- Stream Error remains a canonical terminal event, while pre-start Error is returned by `execute`.
- Tokio paused-time tests verify per-emission delay and pending-pull drop without wall-clock races.
- Invalid and truncated scripts fail construction with the core stream error classification.
