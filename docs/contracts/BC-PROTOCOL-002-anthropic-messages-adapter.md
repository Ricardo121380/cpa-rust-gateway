# BC-PROTOCOL-002 Anthropic Messages adapter

| Field | Value |
|---|---|
| Contract | `BC-PROTOCOL-002` |
| Task | `P5-01` |
| ADR | [ADR-0034](../adr/ADR-0034-anthropic-messages-pure-codec.md) |
| Status | `DONE` |
| Domain | Anthropic Messages protocol codec |

## Boundary

```text
Anthropic Messages JSON -> CanonicalRequest + output mode
CanonicalResponse -> Anthropic Message JSON
CanonicalEvent -> typed Anthropic SSE frame
GatewayError -> safe Anthropic error envelope
```

The boundary is pure: no HTTP header parsing, authentication, route selection, Provider request,
network I/O, bounded delivery, cancellation primitive, or `FirstSemanticEvent` commit belongs to
this contract.

## Inbound behavior

- `model`, positive `max_tokens`, and a non-empty `messages` array are required. `stream` defaults
  to non-streaming and otherwise must be Boolean.
- `system` accepts string or ordered blocks and becomes a canonical `system` message. `user` and
  `assistant` accept string or ordered blocks. Text maps to canonical text.
- Assistant `tool_use` maps to a canonical Tool Call with a non-empty ID/name and JSON `input`.
  User `tool_result` maps to a canonical Tool Result, retaining `content` as JSON and `is_error`.
  A known Tool block in an incompatible role is a client request error; it is not reclassified as
  opaque content.
- A user message containing Tool Results is split at canonical role boundaries. Its raw message
  extensions occur exactly once on the first split result; content order is retained.
- Tool definitions map name, optional description, and object `input_schema`; missing schema
  becomes explicit `{}`. Unknown content blocks become opaque canonical content.
- Every unknown root/nested field is retained under an explicit `anthropic.messages.*` or
  `anthropic.*` extension namespace. It is not an execution guarantee. Duplicate JSON member names
  at any depth are rejected before semantic decoding.

## P5-01 outbound text/Usage slice

- Metadata supplies the selected public model; the canonical response retains only its opaque ID.
- A normal response emits one assistant text content block, exact reported input/output Usage, and
  `end_turn`. It never estimates usage, expands a Model alias, exposes Provider diagnostics, or
  emits credentials.
- SSE uses standard `event:`/JSON `data:` frames: `message_start`, text content-block start/delta/
  stop, `message_delta`, then `message_stop`. `message_start` requires exact canonical input Usage;
  a later final Usage snapshot supplies exact output Usage.
- `StreamError` emits an Anthropic `error` event and no normal terminal event. Before headers, the
  caller can encode the same core error as a safe `{type:error,error:{type,message}}` payload.
- All P5-01 frames are semantic protocol data only. The HTTP writer, introduced later, owns the
  successful-write delivery commit.

## Explicit exclusions until dependent P5 Tasks

- P5-02 owns `count_tokens` execution and accurate Provider capability.
- P5-03 adds output Tool start/delta/end, parallel Tool indexing, `{}` normalization, and
  arbitrary decoded chunk state under
  [BC-PROTOCOL-004](BC-PROTOCOL-004-anthropic-tool-stream-state.md).
- P5-06 owns Thinking, broader stop-reason mapping, cache Usage fields, cache controls, and
  response-model rewrite semantics.
- P5-04 owns lossless bridge admission. Until those additions, unrepresentable canonical event
  extensions, Thinking, cache-detail, and other non-text/non-Tool output fail closed as
  stream-protocol errors.

## Error and privacy behavior

- Malformed, duplicate, missing, invalid-role, or structurally invalid input produces
  `ClientRequestError/Request`.
- Invalid lifecycle order or output that the active P5-01 slice cannot represent produces
  `UpstreamProtocolError/Stream`.
- `Debug` redacts models, messages, Tool IDs/names/arguments, raw JSON, and frame payloads. Error
  envelopes contain only the stable core error type and safe fixed message.

## Corresponding tests

- Desensitized Messages request and canonical snapshot fixtures.
- Text/Usage canonical event fixture with stable non-streaming and SSE snapshots.
- Duplicate-name, invalid role, zero max-token, empty message, missing initial Usage, unsupported
  output, split-extension retention, and redaction regressions.
