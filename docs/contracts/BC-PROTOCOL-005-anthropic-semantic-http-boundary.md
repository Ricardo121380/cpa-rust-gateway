# BC-PROTOCOL-005 Anthropic semantic and HTTP boundary

| Field | Value |
|---|---|
| Contract | `BC-PROTOCOL-005` |
| Task | `P5-06` |
| ADR | [ADR-0039](../adr/ADR-0039-anthropic-semantic-http-boundary.md) |
| Status | `DONE` |
| Domain | Anthropic Messages semantic codec and HTTP boundary |

## Boundary

```text
Anthropic Messages JSON -> CanonicalRequest + response mode
Canonical Event stream -> Anthropic Message JSON or typed SSE
Snapshot public model -> Anthropic response model
Canonical Request event -> AnthropicMessages protocol observation
```

The boundary does not implement a real Anthropic Provider, reconstruct an exact native body for a
bridge, read credentials, or make network traffic.

## Inbound semantics

- `thinking.type` is a required non-empty Canonical Thinking effort when `thinking` is supplied.
  `budget_tokens`, when supplied, must be a positive unsigned integer and is retained under
  `anthropic.thinking.budget_tokens`.
- A known `cache_control` must be an `ephemeral` object. Its non-empty `ttl`, or the explicit
  `ephemeral` label when omitted, becomes `prompt_cache_retention`. Multiple known controls must
  agree on that retention; disagreement is `ClientRequestError/Request`.
- Each block-local `cache_control` remains in its explicit raw extension. This preserves placement
  data while ensuring later canonical/bridge conversion rejects an unproven reconstruction.
- No Anthropic field is reinterpreted as `prompt_cache_key`.

## Outbound semantics

| Canonical semantic | Anthropic representation |
|---|---|
| `ReasoningDelta` | ordered `thinking` content block and `thinking_delta` SSE frame |
| `Usage.cache_read_tokens` | `cache_read_input_tokens` in start/completed Usage |
| `Usage.cache_creation_tokens` | `cache_creation_input_tokens` in start/completed Usage |
| `ResponseEnd.stop_reason` | exact `message_delta.delta.stop_reason` and completed `stop_reason` |
| `ResponseEnd.stop_sequence` | exact `message_delta.delta.stop_sequence` and completed `stop_sequence` |
| resolved Snapshot public model | every Messages JSON/SSE model field |

- The encoder requires exact input Usage before `message_start` and output Usage before a normal
  completion. Later partial snapshots retain earlier reported input/cache values when they only add
  output values.
- `ResponseEnd` requires a non-empty explicit stop reason at the Anthropic boundary. It never
  infers a value from Tool count or content shape.
- `reasoning_tokens`, `cached_tokens`, or raw Usage extensions have no proven Anthropic wire field
  in this contract and produce `UpstreamProtocolError/Stream` rather than a lossy response.
- A normal SSE sequence is `message_start`, ordered content block start/delta/stop frames,
  `message_delta`, then `message_stop`. A StreamError emits only the safe Anthropic error frame.

## HTTP invariants

- `POST /v1/messages` applies the existing Bearer authentication, duplicate-name-aware decoder,
  Snapshot public-model resolution, bounded stream, cancellation, first-semantic-event commit,
  and final Usage observer.
- The request observation records `GatewayProtocol::AnthropicMessages`. Alias input remains in the
  request observation, while the public response model is the forced Snapshot model.
- Pre-header failure uses the safe Anthropic error envelope. After headers, encoder failure emits
  an Anthropic SSE error and never a normal terminal frame.

## Corresponding tests

- `protocol-anthropic` request fixture and rejection tests cover Thinking, cache retention,
  conflicting controls, duplicate fields, and redaction.
- `p5-06-canonical-events.json` plus JSON/SSE snapshots cover Thinking blocks, cache Usage,
  partial Usage merging, stop reason/sequence, and public model metadata.
- Actix tests cover `/v1/messages` JSON, SSE, Snapshot alias rewrite, and protocol observation
  through the existing deterministic Router executor without external traffic.
