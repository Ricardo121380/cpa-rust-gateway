# BC-PROVIDER-017 Grok Official local differential, load, and error matrix

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-017` |
| Task | `P8-06` |
| ADR | [ADR-0059](../adr/ADR-0059-grok-official-local-differential-and-error-matrix.md) |
| Matrix | `C01`、`C03`、`C04`、`C31`、`C33`、`F07`、`G24-G27` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-DEFER-002`; no Official E2E or external load has run |
| Domain | Deterministic local mode differential, bounded concurrent parser safety, and exact failure state matrix |

## Preconditions and bounds

1. All inputs are in-memory synthetic fixtures passed through the existing Official adapter,
   transport seam, decoder, and runtime-state API. No DNS, socket, xAI endpoint, credential source,
   OAuth cache, server, browser, proxy, route, account, or database is read or changed.
2. The valid fixture carries only P8-04's admitted Tool/Reasoning semantics. Native Search and
   opaque provider output remain fail-closed; their absence is not an emulation claim.
3. This local task cannot perform an Official E2E. `P8-07` / `BC-E2E-004` own its separate
   authorization; `CR-P7-DEFER-002` makes P8 phase closeout and Delivery Gate independent of P7/G7.

## Required behavior

| Concern | Required behavior |
|---|---|
| Mode differential | Completed JSON and SSE must have the same final Canonical Tool-call name/object arguments, Reasoning text, assistant text, and final reasoning-token usage. Tested legal byte splits cannot change this semantic projection. |
| Concurrent local load | Each decoder has exclusive buffered bytes/lifecycle/item state. Ninety-six concurrent synthetic decoders on bounded legal records must all complete with the same projection. The test uses twelve OS threads behind a start barrier, with eight independent decoders each; this is a correctness check, not a throughput or upstream capacity measurement. |
| 401 / 403 / transient / permanent | 401 requests only Official credential replacement; unknown 403 is egress-local; 408/5xx request only Official endpoint cooling; other failures are permanent/non-mutating. None creates an Official quota snapshot or changes another binding. |
| 429 without Header | Record only the exact Official binding as `Estimated/Estimated` using the bounded 30-second generic fallback. No Build/Web target is written. |
| 429 with `Retry-After` | Replace the older exact observation with `Header/Observed` and the Header-derived reset. No other target is read or written. |
| Limits of evidence | No result asserts live xAI protocol compatibility, availability, capacity, latency, price, quota balance, or G8 completion. |

## Corresponding tests

- `completed_and_sse_adapters_have_one_tool_reasoning_semantic_projection`
- `ninety_six_concurrent_sse_decoders_remain_chunk_and_state_isolated`
- `official_failure_matrix_is_exact_target_only_and_preserves_other_bindings`
