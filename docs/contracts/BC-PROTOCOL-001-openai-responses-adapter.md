# BC-PROTOCOL-001 OpenAI Responses adapter

| Field | Value |
|---|---|
| Contract | `BC-PROTOCOL-001` |
| Task | `P1-05` |
| Status | `DONE` |
| Domain | OpenAI Responses protocol codec |

## Entry and boundary

This task implements only the protocol-pure adapter in `protocol-openai-responses`:

```text
Responses JSON request -> CanonicalRequest + requested output mode
CanonicalResponse -> Responses JSON response
CanonicalEvent -> typed SSE frame
GatewayError -> safe OpenAI-shaped error object/frame
```

It does not create an Actix handler, inspect HTTP headers, select or execute a Provider, own a
bounded transport, call `FirstSemanticEvent::mark_delivered`, choose retries, or write bytes to a
client. P1-07 owns the Actix endpoint and the successful-write delivery boundary.

## Inbound mapping

- `model` is required and maps to `CanonicalRequest::requested_model`.
- `stream` defaults to non-streaming and selects the adapter output mode; it is not retained as an
  upstream extension.
- `input` supports a string and ordered message, historical Function Call, and Function Call Output
  items. `instructions` becomes a prepended canonical `developer` message. `user`, `developer`,
  `system`, and `assistant` message roles plus string/`input_text` content map to canonical text.
  Unrecognized content blocks are retained as `OpaqueContent`; unsupported top-level input item or
  Tool kinds are rejected rather than silently reclassified.
- Function Tools map their name, optional description, and JSON Schema to `ToolDefinition`.
  Missing function parameters become the explicit empty object `{}`. Tool-specific fields not
  represented by the canonical type are retained as extensions.
- `reasoning.effort`, `prompt_cache_key`, and `prompt_cache_retention` map to their corresponding
  canonical fields. Request options outside the explicit reject list and without a current
  canonical field are retained under the `openai.responses.` raw-extension namespace. This is
  lossless protocol admission, not a P1 execution claim: the deterministic Mock does not infer
  their semantics, and a later Provider adapter must map them losslessly or reject them.
- Missing Function `parameters` becomes the explicit empty object `{}`. Explicit `null` parameters,
  invalid Function Call JSON-string arguments, and a Function Call Output `status` are rejected;
  Function Call Output is retained as raw JSON and has `is_error = false` by this frozen policy.
- Built-in/MCP Tools, non-`auto` tool choice, `parallel_tool_calls: false`, structured output,
  top-logprobs, stream options, background/persistence/conversation controls, malformed fields, and
  ambiguous JSON are rejected as `ClientRequestError/Request`.
- Duplicate JSON names are rejected at every nesting level *before* semantic decoding into
  `serde_json::Value`. No unsupported field is silently discarded.

## Non-streaming output

- An adapter metadata value supplies the public model and creation timestamp, which the canonical
  response intentionally does not own.
- A successful canonical response encodes to a Responses response object with its opaque response
  ID, status, ordered output items, output text/reasoning/tool content, and final Usage when
  supplied. It never invents model tokens, provider diagnostics, or credentials.
- Usage maps only fields with a lossless Responses representation: input/output/reasoning token
  counts and aggregate cached tokens. Generic Usage extensions, separate cache-read/cache-creation
  counts, and overflowing input-plus-output totals are rejected as `UpstreamProtocolError/Stream`
  instead of being dropped, collapsed, or saturated into a fabricated total.
- The response object includes adapter-owned `model`, `created_at`, generated output item IDs,
  `error: null`, `incomplete_details: null`, and `usage: null` when no Usage was reported.
- A `GatewayError` before HTTP/SSE headers maps to exactly
  `{ "error": { "type", "code", "message", "param": null } }`, using only the core stable code
  and safe message. P1-07 selects the HTTP status and headers.
- Outbound canonical payloads with nonempty generic raw extensions are rejected as
  `UpstreamProtocolError/Stream`, because this public Responses surface has no lossless generic
  extension field.

## Streaming output

- Frames use the SSE `event:` and `data:` form. The payload `type` matches the event name, every
  typed event receives a monotonic `sequence_number`, and data is JSON encoded rather than
  assembled by string interpolation. Normal completion is `response.completed`; this is not a
  Chat-Completions `[DONE]` stream.
- `ResponseStart` emits `response.created` then `response.in_progress`. The first Text Delta lazily
  emits `response.output_item.added`, `response.content_part.added`, and
  `response.output_text.delta`; later text uses the same deterministic output/content indices.
  A Tool-only message never invents an empty assistant output item.
- Tool Start creates a separate Function Call output item. Every Tool argument delta is emitted as
  `response.function_call_arguments.delta`; if any delta occurred, their exact concatenation must
  equal `ToolCallEnd.arguments`. Tool End emits the one arguments-done event and output-item-done
  event. The complete arguments field remains a JSON *string*.
- Reasoning uses a separate `reasoning` output item and `response.reasoning_text.delta/done` frames.
  Usage updates only the encoder's latest snapshot because the Responses stream has no Usage-delta
  event; it appears in the terminal response when supplied.
- Message End emits text/content/item completion frames and closes any reasoning item. Response End
  emits exactly one `response.completed`. `StreamError` emits exactly one terminal
  `response.failed` with only the safe core error and never emits completion.
- An encoder validates canonical lifecycle order and never emits after terminality. Every emitted
  P1-05 frame is classified `semantic`; this codec has no keepalive scheduler, because the
  BC-HTTP-001 SSE body owns the transport keepalive comment and that comment is not a codec frame.
  The pure codec never commits
  FirstSemanticEvent. P1-07's HTTP writer must call the tracker only after the first semantic frame
  is successfully written to the client.

## Corresponding tests

- Desensitized official-form request fixtures cover text messages, Function Tools, historical Tool
  calls/results, reasoning, cache fields, unknown raw extension retention, malformed data, and
  duplicate JSON names.
- A canonical response fixture produces a stable non-streaming JSON snapshot.
- A canonical event fixture produces stable SSE frames for text, reasoning, interleaved Tool
  arguments, Usage, normal completion, and safe stream failure.
- Tests reject duplicate keys at root and nested input/Tool/opaque/raw-extension paths, reject
  unsupported execution controls, reject output after terminality, reject mismatched Tool argument
  fragments and unrepresentable Usage, and prove that no adapter path exposes client content or
  unsafe diagnostics through `Debug` or error envelopes.
