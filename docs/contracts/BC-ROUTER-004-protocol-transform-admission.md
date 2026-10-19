# BC-ROUTER-004 Protocol transform admission

| Field | Value |
|---|---|
| Contract | `BC-ROUTER-004` |
| Task | `P5-04` |
| ADR | [ADR-0037](../adr/ADR-0037-protocol-transform-admission.md) |
| Status | `DONE` |
| Domain | Router-local admission of OpenAI Responses / Anthropic Messages transform Candidates |

## Boundary

```text
ProtocolTransformInput
  (source, target, mode, native-body availability, CanonicalRequest,
   response/capability requirements, Endpoint CapabilitySet)
                    |
                    v
      analyze_protocol_transform(...)
                    |
                    +-- Approved
                    +-- Rejected(stable secret-free reason)
```

The boundary is pure and read-only. It neither encodes a target request nor selects an Endpoint,
leases a Credential, touches health/quota/circuit state, writes an Event, changes a Snapshot, or
makes an HTTP/Provider/network call.

## Transform-mode matrix

| Route mode | Source / target | Native body | Result precondition |
|---|---|---|---|
| `Passthrough` | Same protocol | `Exact` | Native payload may be forwarded unchanged; unknown native fields are not reconstructed. |
| `Canonical` | Same protocol | Any | Every retained Canonical field must have the current same-protocol re-encoding proof. |
| `LosslessBridge` | Different protocol | Any | Every retained Canonical field must have the current cross-protocol bridge proof. |
| `CanonicalBridge` | Same or different registered protocol | Any | Same-protocol Canonical proof, or the current cross-protocol bridge proof. |

`Passthrough` with different protocols or without an exact native payload is rejected.
`Canonical` across protocols and `LosslessBridge` within one protocol are rejected. These are
configuration/shape failures, not fallbacks. `CanonicalBridge` is an explicit P12 native-provider
extension; it does not alter the rejection rules of the original three modes.

## Invariants

1. Canonical and Bridge conversions reject non-empty request, message, text-content, and Tool
   declaration extension namespaces. An unknown field is never dropped or guessed.
2. Canonical and Bridge conversions reject opaque content and historical Tool calls/results. The
   current contract proves Tool declaration capability only; it does not prove a historical Tool
   conversation conversion.
3. Canonical and Bridge conversions reject any Thinking setting and prompt-cache key/retention.
   P5-06 must add an explicit semantic mapping before those inputs can participate.
4. Current target roles are `system`, `user`, and `assistant` for Anthropic Messages, and
   `system`, `developer`, `user`, and `assistant` for OpenAI Responses. Other roles are rejected.
   Historical Tool inputs are rejected with their more-specific Tool reason before role admission.
5. `streaming` requires `Streaming`; declared Tools require both `Tools` and `JsonSchema`; an
   explicit JSON-Schema requirement requires `JsonSchema`; parallel Tools additionally require
   `ParallelTools`. These checks consume only the compiler-approved Endpoint capability set.
6. `ProtocolTransformRejection` values are stable and value-free. They must not embed client
   fields, model/URL identity, raw JSON, Tool names/IDs, or credentials. Debug output for the
   input redacts the Canonical request in full.
7. An `Approved` result is admission evidence only. It does not promise that a later HTTP,
   Provider, response, health, circuit, or retry operation succeeds.

## Output-limit projection (CPAR-38, 2026-10-11)

- `openai.chat.max_completion_tokens` and `openai.responses.max_output_tokens` both
  bound total generated output, including reasoning. Map the value exactly in either
  direction; Responses to Chat emits `max_completion_tokens`.
- Native Chat retains either modern `max_completion_tokens` or legacy `max_tokens`
  without renaming. The existing legacy positive-integer bridge to Responses/Messages
  remains supported; this does not assert that every upstream model accepts the legacy
  Chat spelling. There is no model-name exception or capability change.
- A second Chat spelling, or any foreign output-limit namespace, is a collision even
  when values are equal. Public foreign/dotted names remain unknown extensions and are
  also rejected. No overwrite, minimum/maximum selection, field deletion or null defaulting.
- Every retained limit uses the existing positive-u64 execution domain documented in
  [BC-PROTOCOL-008](BC-PROTOCOL-008-openai-chat-completions-codec.md). An absent limit
  remains absent for Chat/Responses; Messages still requires a limit. Same-protocol
  canonical and exact passthrough paths enforce the same value/collision checks.
- A generic Channel Pin constructs its bounded Canonical control in the requested source
  protocol namespace, then uses this same router projection. The native-provider probe
  policy and existing Codex probe exception are unchanged. Pin is not evidence that the
  separate public request's exact controls and history have executed.

Semantic evidence checked on 2026-10-11: [OpenAI Chat create reference](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
defines the modern field and deprecates the legacy field; [OpenAI token-counting guide](https://developers.openai.com/api/docs/guides/token-counting)
explicitly describes the total-output semantics of both modern OpenAI fields. These
sources establish the modern equivalence, not provider execution or precedence for two
simultaneous controls. Collision rejection follows ADR-0100's strict fidelity policy.

## CPAR-38 response rejection observation

Projection rejection retains its existing closed reason in a core error with fixed
stage-specific public text. The six reasons and strict equivalence rules are unchanged;
no field or native item is deleted to obtain success. See
[ADR-0105](../adr/ADR-0105-projection-failure-observation.md).

## Corresponding tests

- `grok_build_projection_rejection_has_safe_reason_and_request_id`
- `grok_build_projection_failure_preserves_usage_and_ledger`

- `chat_output_limits_preserve_native_spelling_and_bridge_total_budget`
- `chat_output_limits_reject_invalid_values_and_every_collision_in_all_modes`
- `build_chat_completion_limit_reaches_native_transport_and_collisions_do_not`

The CPAR B-01 repair restores the native Responses history fidelity already required by
[ADR-0100](../adr/ADR-0100-exact-protocol-semantics-and-usage-evidence.md) and
[ADR-0101](../adr/ADR-0101-grok-native-item-metadata-fidelity.md), which supersede the early
text/Tool restrictions above. A Responses assistant message may contain ordered text parts;
closed role/field validation, per-part metadata checks and Tool correlation still apply.
The public HTTP regressions are:

- `responses_json_multipart_output_replays_exactly_through_public_http`
- `responses_sse_multipart_output_replays_exactly_through_public_http`
- `responses_multipart_history_rejects_invalid_semantics_before_upstream_http`

- Exact native pass-through accepts a retained extension only when source/target formats match;
  a missing body and cross-protocol pass-through fail closed.
- Canonical/Bridge topology, unknown extension classes, opaque content, historical Tool calls and
  results, Thinking, cache controls, and role incompatibility have explicit stable rejections.
- Streaming, Tools, Tool-schema, explicit JSON-Schema, and parallel-Tool checks each reject an
  insufficient Endpoint capability set; a complete set is admitted.
- A clean OpenAI Responses to Anthropic Messages Bridge is admitted, and `Debug` diagnostics do
  not include model or prompt values.

## 2026-10-19 summary-only Responses control

[BC-PROTOCOL-009](BC-PROTOCOL-009-codex-responses-controls.md) requires Reasoning capability for
`openai.responses.reasoning` even though typed explicit-effort Thinking is absent. Only a sole
supported summary member is admitted for a Responses target. Typed Thinking collisions and
Chat/Messages bridges reject. Passthrough performs the same control and capability checks.

- `summary_only_reasoning_keeps_effort_absent_and_requires_native_reasoning_capability`
