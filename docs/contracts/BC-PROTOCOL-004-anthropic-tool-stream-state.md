# BC-PROTOCOL-004 Anthropic Tool stream state

| Field | Value |
|---|---|
| Contract | `BC-PROTOCOL-004` |
| Task | `P5-03` |
| ADR | [ADR-0036](../adr/ADR-0036-anthropic-tool-stream-state.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | Canonical Tool events to Anthropic Message and SSE output |

## Boundary

```text
Canonical ToolCallStart(call_id, name)
  -> content_block_start(index, tool_use { id: call_id, name, input: {} })
Canonical ToolCallArgumentsDelta(call_id, fragment)
  -> per-call partial_json + input_json_delta(index, fragment)
Canonical ToolCallEnd(call_id, final JSON object)
  -> reconciled non-streaming tool_use input + content_block_stop(index)
```

The boundary is response-only and pure. It has no HTTP writer, route lookup, Provider request,
credential, network I/O, cancellation implementation, request Tool schema, or Tool execution.

## Invariants

1. Each non-empty Canonical `call_id` gets one stable public `tool_use.id` and one immutable,
   append-only content-block index. Multiple calls can receive deltas in an interleaved order
   without sharing an accumulator or changing IDs/indexes.
2. A Tool start closes the active text block before allocating the Tool block. A Tool block starts
   with `input: {}` and has exactly one `content_block_stop` after its completed input is accepted.
3. Each non-empty `input_json_delta` uses the declared Tool's own index. Empty decoded fragments
   are ignored because they carry no JSON input. Concatenating a Tool's emitted argument deltas
   preserves that Tool's received decoded fragment sequence; a different valid
   chunk schedule may change the frame count but not the final Tool semantic projection.
4. If any argument delta was received, its accumulated normalized JSON text must equal the final
   normalized `ToolCallEnd.arguments`; disagreement is `UpstreamProtocolError/Stream`. A Tool end
   without deltas emits one complete non-empty delta only when needed to make the final input
   visible.
5. Empty/whitespace-only input and a whitespace-wrapped empty object normalize to `{}`. A Tool
   whose completed input is `{}` and received no non-empty argument delta emits no synthetic
   `input_json_delta`; `EnterPlanMode`, `ExitPlanMode`, and ordinary no-argument Tools therefore
   have an explicit empty object from their start frame.
6. The completed input must parse as a JSON object. Array, scalar, malformed, duplicate,
   post-completion, unknown-call, mismatched, or unfinished Tool state fails closed with
   `UpstreamProtocolError/Stream`. No Message/response terminal event is accepted while a Tool is
   unfinished.
7. A completed response containing Tool output uses `stop_reason: "tool_use"` in both Anthropic
   response forms. P5-06 owns all broader Stop Reason, Thinking, cache-usage, and model-rewrite
   mapping.
8. This contract does not prove required-property validation because the response codec has no
   request Tool schema and performs no execution. A later request-and-execution composition must
   reject a missing required property before Tool execution; this codec must not invent one.

## Corresponding tests

- `tool-canonical-events.json` produces frozen non-streaming and SSE snapshots with a text block,
  a parameterized Tool, a no-argument `EnterPlanMode`, and stable block indexes.
- A one-byte ASCII regression interleaves two parameterized Tools with escaped JSON and verifies
  explicit `{}` for `EnterPlanMode`, `ExitPlanMode`, and an ordinary no-argument Tool.
- A fixed-seed 128-case property suite splits Unicode-safe decoded strings at arbitrary scalar
  boundaries, interleaves two Tool schedules, and compares per-call ID/name/input and SSE delta
  reassembly to the non-streaming projection.
- Negative regressions require safe stream errors for mismatched final input, an unfinished Tool,
  and an array Tool input; separate regressions verify whitespace-wrapped `{}` emits no input
  delta and an empty delta does not create a false final mismatch.
