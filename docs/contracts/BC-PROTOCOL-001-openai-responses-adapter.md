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

## 2026-10-08 upstream SSE confirmation supplement (B-02)

This fixes the existing strict native decoding boundary; it does not add a protocol, capability,
Canonical event, retry policy, or persistence entity. [ADR-0100](../adr/ADR-0100-exact-protocol-semantics-and-usage-evidence.md),
[BC-CORE-003](BC-CORE-003-canonical-event-state-machine.md),
[BC-RESP-002](BC-RESP-002-stored-response-public-lifecycle.md) and
[BC-RESP-004](BC-RESP-004-public-responses-websocket.md) retain authority for fidelity, terminality,
durability, and successful replay roots.

The field definitions were checked against the official
[Responses streaming events reference](https://developers.openai.com/api/reference/resources/responses/streaming-events)
on 2026-10-08 (Asia/Shanghai). It is an unversioned live reference; the retrieval timestamp and
content hash are recorded in [the source receipt](../reports/assets/cpar-b02-20261008/official-source.json).
The supported subset is narrower than the full API and remains explicit.

Every received confirmation has a closed root field set. `type`, nonempty bounded `item_id`,
nonnegative integer `output_index`, `sequence_number`, and the applicable part index are required.
Part indices and output indices are below 64. The output index must identify the started item;
the item must be open in the current response. These events have no `response_id` field: response
association comes from the active decoder lifecycle. A foreign response field is an extension
error, rather than an ignored alternate identity.

| Event suffix | Additional fields | Confirmation rule |
| --- | --- | --- |
| `output_text.done` | `content_index`, string `text`; optional reviewed `logprobs` | Open message part; text equals accumulated text exactly |
| `content_part.done` | `content_index`, object `part` | Open message output_text or reasoning content reasoning_text; closed part matches accumulated text and observed annotations |
| `reasoning_summary_part.done` | `summary_index`, object `part`; upstream optional `status` | Open reasoning summary_text; exact accumulated text; interrupted per-part status has no native Canonical representation and fails explicitly |
| `reasoning_summary_text.done` | `summary_index`, string `text` | Open reasoning summary part; exact accumulated text |
| `reasoning_text.done` | `content_index`, string `text` | Open reasoning content part; exact accumulated text |

Text parts admit only `type`, string `text`, reviewed `annotations`, and null/empty `logprobs`;
reasoning parts admit only `type` and string `text`. Nonempty token metadata, unsupported part
kinds, unknown fields, and malformed annotations are rejected. The local compatibility omission
of done-event logprobs follows the existing native encoder, which emits that confirmation without
logprobs while preserving the part's final null/empty representation; it is not an assertion that
the current official schema marks it optional. If present on `output_text.done`, logprobs must be
an empty array: event-level null is rejected, independently of the final part's nullable metadata.
Final part snapshots must match item-done metadata
exactly, including annotations and null/empty representation.

The two supported part-added events validate the same identity/index fields and a closed initial
part. Initial text/annotations enter existing bounded accumulation; they are not discarded or
replayed. Observed added parts must appear in item-done. Deltas cannot grow a part after text-done
or part-done; annotations cannot change after part-done. An item-done snapshot cannot append text
after confirmation or replace a confirmed part. Before item completion, an identical repeated
confirmation remains a no-op; differing repetition and confirmation after item completion fail.
No new requirement forces fine-grained confirmation events to exist: legacy delta/item-done and
snapshot completion paths still pass, including missing part-added events. Existing legacy delta
index defaults remain unchanged; received confirmation events use the required fields above.

Decoder rejection is permanent. A conflicting frame stops processing the current read, including
any following completed frame. Semantic residue already present after a terminal frame also fails
before buffered success is returned. No successful ResponseEnd, durable completed response, or
WebSocket root can be formed from the failed sequence. Validated buffered Usage remains accessible;
this does not repair the independent B-04 bootstrap ownership gap. Retained initial/final
confirmation snapshots share a fixed 8-MiB budget; identical repeats do not accumulate copies.

The existing Attempt observation reports the driver/bootstrap result; RequestFinished reports
the complete request outcome. An established source may therefore retain a Succeeded Attempt
beside a Failed Request. Bootstrap rejection instead has a Failed Attempt. These existing meanings
must not be combined into a successful-response assertion.

## Corresponding tests

- `contradictory_text_done_fails_through_public_gateway_without_replay`
- `contradictory_native_done_confirmations_fail_public_storage_and_continuation`
- `done_field_and_correlation_errors_fail_through_public_gateway`
- `native_done_lifecycle_and_metadata_conflicts_fail_publicly`
- `valid_native_done_preserves_parts_and_allows_owned_stored_continuation`
- `done_conflict_never_registers_a_websocket_root_or_stored_response`
- `done_frames_are_read_boundary_invariant_and_rejection_is_permanent`
- `bootstrap_done_conflict_records_failed_attempt_and_no_owned_result`

Tests use the actual public handler, compiled route, selected lease, decoder and real loopback HTTP
upstream. Actual content-free Request/Attempt observations are persisted and reopened in temporary
SQLite for outcome/identity readback; this test does not claim production asynchronous-writer
deployment. HTTP GET/continuation and real WebSocket session reuse supply successful and failed
ownership controls. No Provider URL/file citations are fetched.

Rollback reverts this implementation, tests and supplement together. There is no data migration;
the previous source again fails the contradictory-confirmation regression. Local validation,
independent review and exact source identity are recorded in the B-02 delivery report.
