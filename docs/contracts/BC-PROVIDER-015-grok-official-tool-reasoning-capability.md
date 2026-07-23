# BC-PROVIDER-015 Grok Official Tool, Reasoning, and Search capability boundary

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-015` |
| Task | `P8-04` |
| ADR | [ADR-0057](../adr/ADR-0057-grok-official-tool-reasoning-capability-boundary.md) |
| Matrix | `B11`、`B12`、`B16`、`C01`、`C03`、`C18`、`C31`、`F02` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-DEFER-002`; no Official E2E has run |
| Domain | Explicit Official Function Tool/Reasoning conversion and truthful Search non-capability |

## Preconditions and bounds

1. The native Official request builder receives an explicitly selected API key, upstream model,
   Canonical Request, and JSON/SSE mode. It reads no ambient credential, server, account, route,
   proxy, browser/OAuth cache, database, or Search configuration.
2. Only extension-free `low`, `medium`, and `high` Reasoning effort is representable. Function
   schema and arguments must be bounded JSON objects; Tool Result is an exact non-error JSON string.
3. The native Search state is exactly `UnavailablePendingCanonicalContract`: `B21` has no admitted
   Canonical/ingress mapping, so no Endpoint/Candidate may advertise Search through this contract.
4. This local task does not authorize a real Official E2E. `P8-07` / `BC-E2E-004` own the separate
   authorization; `CR-P7-DEFER-002` makes P8 closeout and Delivery Gate independent of P7/G7.

## Required behavior

| Concern | Required behavior |
|---|---|
| Capability declaration | Return only Tools, ParallelTools, JSON Schema, Reasoning, and Streaming. Parallel Tools remains paired with Tools. Vision and Search are not declared. |
| Request conversion | Encode Function Tools, historical assistant Function Calls, string Tool Results, and supported Reasoning as fixed Official Responses JSON. Cache, opaque content, error Tool Result, unknown extension, unsupported role/effort, non-object schema, and non-object arguments return `ClientRequestError/Request` before transport. |
| Tool response conversion | `function_call` item, `call_id`, and name are stable from add through done. Argument deltas are bounded; final arguments are JSON objects and must agree with any incremental concatenation. Empty object/whitespace normalizes only to `{}`. |
| Reasoning response conversion | `reasoning` item output and bounded reasoning SSE deltas map only to Canonical ReasoningDelta. A terminal value must confirm the accumulated value; non-exported completed reasoning may produce zero deltas. |
| Completion | Completed output identities must equal exactly the completed added identity set. Canonical Tool/Reasoning/text/Usage lifecycle stays valid; malformed SSE record cannot advance externally held state. |
| Search | Native Search request/input/output is fail-closed. It cannot be re-labelled as Function Tool or opaque Canonical content and must not be advertised as supported. |
| Isolation and diagnostics | No Build/Web request profile, credential, quota/account/health/retry/continuity state, raw Tool arguments, Reasoning text, endpoint, API key, or Search value is exposed or mutated. |

## Corresponding tests

- `capability_declaration_admits_only_lossless_semantics`
- `tools_reasoning_and_history_encode_without_loss`
- `unsupported_search_and_unsafe_tool_or_reasoning_forms_fail_closed`
- `non_streaming_and_every_sse_chunk_size_preserve_tool_reasoning_semantics`
- `mismatched_or_non_object_tool_arguments_remain_protocol_errors`
