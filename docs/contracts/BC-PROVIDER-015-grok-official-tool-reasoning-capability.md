# BC-PROVIDER-015 Grok Official Tool, Reasoning, and Search capability boundary

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-015` |
| Task | `P8-04`; `CPAR-C / #44, #47, #48` native metadata supplement |
| ADR | [ADR-0057](../adr/ADR-0057-grok-official-tool-reasoning-capability-boundary.md), [ADR-0101](../adr/ADR-0101-grok-native-item-metadata-fidelity.md) |
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
| Request conversion | Encode Function Tools, historical assistant Function Calls, string Tool Results, and supported Reasoning as fixed Official Responses JSON. Reviewed assistant native history preserves item identity/status, message phase, indexed reasoning parts, and text-part annotations/null or empty logprobs. Cache, opaque content, error Tool Result, unknown extension, unsupported role/effort, non-object schema, non-object arguments, and unowned encrypted reasoning return `ClientRequestError/Request` before transport. |
| Tool response conversion | `function_call` item, `call_id`, and name are stable from add through done. Argument deltas are bounded; final arguments are JSON objects and must agree with any incremental concatenation. Empty object/whitespace normalizes only to `{}`. |
| Native response conversion | Preserve reviewed item identity/status/order, separate reasoning summary/content parts, message phase, citations, and null/empty final logprobs through explicit Canonical item/part metadata. Preserve validated reasoning even when not requested. Official/Console reject all non-null ciphertext; Build retains its separate owner path. Rich metadata is Responses-only; one plain reasoning body may bridge exactly to Chat/Messages. |
| Event correlation and completion | Supplied item/part/output indices and optional call/name fields must confirm the started item. Unknown semantic event fields and nonempty event logprobs fail closed. Completed part snapshots and observed annotations must agree with item completion; the final output order and identity set must equal the observed order and set. Canonical Tool/Reasoning/text/Usage lifecycle stays valid; malformed SSE record cannot advance externally held state. |
| Search | Native Search request/input/output is fail-closed. It cannot be re-labelled as Function Tool or opaque Canonical content and must not be advertised as supported. |
| Isolation and diagnostics | No Build/Web request profile, credential, quota/account/health/retry/continuity state, raw Tool arguments, Reasoning text, endpoint, API key, or Search value is exposed or mutated. |

## Corresponding tests

- `capability_declaration_admits_only_lossless_semantics`
- `tools_reasoning_and_history_encode_without_loss`
- `unsupported_search_and_unsafe_tool_or_reasoning_forms_fail_closed`
- `non_streaming_and_every_sse_chunk_size_preserve_tool_reasoning_semantics`
- `mismatched_or_non_object_tool_arguments_remain_protocol_errors`
- `official_console_preserve_native_items_parts_phase_and_citations`
- `incremental_annotations_and_indexed_parts_survive_all_native_channels`
- `intermediate_snapshots_annotations_and_terminal_order_must_agree`
- `native_event_only_metadata_and_correlation_cannot_disappear`
- `optional_function_event_ids_confirm_the_started_call`
- `official_console_plain_reasoning_keeps_its_existing_bridges`
- `runtime::batch_c::grok_native_metadata_survives_public_responses_and_rejects_lossy_bridges`
