# ADR-0036: Anthropic Tool stream state and normalized object input

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P5-03` |
| Contract | [BC-PROTOCOL-004](../contracts/BC-PROTOCOL-004-anthropic-tool-stream-state.md) |

## Context

The Canonical lifecycle permits several Tool Calls to be declared and receive argument fragments
in an interleaved order. Anthropic Messages represents every Tool Call as a numbered `tool_use`
content block, while its SSE stream sends JSON fragments by content-block index. A single shared
argument buffer would therefore mix concurrent calls; forwarding an incomplete JSON value would
let a client observe a Tool that cannot safely be executed.

P5-01 intentionally rejected all outbound Tool events. The pure response codec receives a
`CanonicalResponse` but not the originating request's Tool schema, so it can preserve and validate
the response's JSON-object boundary but cannot decide whether a named required property is missing.

## Decision

1. `protocol-anthropic` keeps append-only output `ContentBlock`s plus a separate Tool state keyed
   by Canonical `call_id`. Each state owns its stable Anthropic block index, accumulated decoded
   `partial_json`, and completion flag. Empty decoded delta fragments are no semantic input and
   are ignored. The `call_id` becomes the public `tool_use.id` unchanged.
2. A Tool start closes an active text block in the logical content projection and reserves its
   index. Multiple Tool states may subsequently receive deltas in arbitrary interleaving while
   retaining their own index and accumulator. The Anthropic wire layer emits one active
   `content_block_start` at a time; later logical blocks buffer until every earlier block has
   emitted its matching stop, then replay their preserved per-Tool decoded fragment sequence.
3. A Tool end normalizes an empty or whitespace-wrapped empty JSON object to `{}`, requires the
   non-empty accumulated fragments (when present) to match that normalized complete input, parses it, and
   accepts only a JSON object. Non-empty complete input that had no delta is emitted as one final
   `input_json_delta`; normalized empty input needs no synthetic delta because the start frame
   already contains `{}`.
4. Unknown, duplicate, completed, non-object, mismatched, or unclosed Tool state fails with the
   safe `UpstreamProtocolError/Stream` category. A Message cannot close while any declared Tool
   remains incomplete; no non-streaming Message can contain an unfinished Tool.
5. The narrow Tool slice emits Anthropic `tool_use` as the successful terminal reason whenever
   the completed Message contains a Tool. Full Stop Reason mapping, Thinking, cache usage, and
   model rewrite remain P5-06 work.
6. Required-property validation is deliberately not guessed from Tool names or output JSON. This
   codec never executes a Tool and has no request-side schema carrier. P5-07's client/Plan-mode
   composition must validate a request-declared schema before an emitted Tool is executed; the
   current codec fails closed for values that cannot be an Anthropic Tool input object.

## Consequences

- Parallel Tool argument fragments remain isolated and their public IDs remain stable across both
  non-streaming and SSE responses.
- Canonical interleaving never produces overlapping Anthropic wire block lifecycles; buffering
  changes only frame timing, not per-Tool final inputs, IDs, names, order, or stop reason.
- Explicit no-argument Tool Calls such as `EnterPlanMode` and `ExitPlanMode` result in exactly
  `{}` without a fabricated argument-delta frame.
- The protocol boundary detects an upstream reconstruction disagreement before it emits a terminal
  Tool block, although already-streamed fragments remain client-visible by design.
- The response-only codec cannot claim request-schema validation or Tool execution safety. That
  proof is deferred to the composition that owns both a request schema and execution.

## Alternatives considered

- One global argument string: rejected because interleaved parallel calls would mix JSON.
- Renumber IDs or derive IDs from Tool names: rejected because retries, Tool results, and clients
  require the Canonical correlation identifier to remain stable.
- Autocomplete braces, arrays, scalars, or missing required fields: rejected because it would
  invent executable semantics. Only a normalized empty object is a defined no-argument value.
- Add required-field checks to the response codec: rejected because the response API has no
  request Tool schema and guessing one from names would be a hidden protocol policy.

## Validation and rollback

Fixtures cover text plus Tool ordering, SSE block indices, non-streaming `tool_use` output, and a
no-argument Plan Mode Tool. Fixed-seed property tests cover scalar-boundary slicing, one-byte ASCII
JSON, Unicode/escape content, and interleaved two-Tool schedules. Negative tests cover mismatched
final input, an unfinished Tool, non-object input, and whitespace-wrapped `{}` normalization.
Rollback removes only protocol encoding and test/documentation assets; no database migration,
credential, network request, or Provider state needs reversal.
