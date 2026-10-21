# BC-PROTOCOL-012: Bounded native non-assistant input message IDs

| Field | Value |
| --- | --- |
| Task | #62 C6; #58 T02/T03/T08/T10 |
| ADR | [ADR-0118](../adr/ADR-0118-native-input-message-id.md) |
| Scope | Authenticated standard HTTP Responses, actual GrokBuildResponses, Responses target, Canonical/CanonicalBridge |
| Activation | Explicit owned local controlled assembly only; production BUILD_ACTIVATION_BLOCKED |

## Authority and source boundary

This is a CPAR local compatibility contract, not a generally admitted Responses POST field or
an assertion about the fixed Build service. Freeze full source URL, revision, source date when
published, retrieval date, byte count and SHA-256 in the C6 delivery source manifest.

At Codex rust-v0.161.0 commit `979011409de0a60b52f179721948e65531d26144`:

- [models.rs](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/protocol/src/models.rs#L1021) retains Message.id as Option<ResponseItemId> and serializes it when present.
- [common.rs](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/codex-api/src/common.rs#L288) uses Vec<ResponseItem> as ResponsesApiRequest.input.
- [responses.rs](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/codex-api/src/endpoint/responses.rs#L83) encodes that request directly and sends POST /responses.

The public [create schema](https://developers.openai.com/api/reference/resources/responses/methods/create)
does not list id on its non-assistant message branches. [GET input_items](https://developers.openai.com/api/reference/resources/responses/subresources/input_items/methods/list)
returning id does not authorize arbitrary caller-selected POST identities. The frozen
[Build client history encoder](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation/responses.rs#L194)
does not send these non-assistant IDs and does not establish server behavior. Source/release/service
mapping and fixed proxy/profile 1.0.50 input-ID semantics remain unqualified.

## Closed input and identity rules

Only user, developer and system message id is newly covered. Absent id remains absent. Present
id must be a JSON string of 1 through 512 ASCII graphic bytes (0x21 through 0x7e), with no trimming,
normalization, case folding, prefix restriction, hashing, replacement or truncation. Null, empty,
wrong type, whitespace/control, non-ASCII and over-bound values reject before send. The ordinary
request body/depth bounds and nonempty message-content requirement remain. Empty text is valid;
an empty content array is not. Escaped JSON spelling may change on encoding; the decoded string
value must be identical. No new ID is generated for absence.

The message has only the already supported type/role/content plus this id. Duplicate JSON keys
(even equal values or escaped-equivalent names), status, phase and any unknown sibling or nested
extension reject. C6 grants no new role, content, tool control or arbitrary extension permission.

In one effective input history, a native item ID may occur only once, even if both occurrences
have equal role/content. Reuse on a different role, changed content, assistant message, function,
reasoning or hosted item is the same conflict. Function call_id remains an independent correlation
namespace under existing call/result rules. Replaying an ID in a separate self-contained request
is legal; there is no cross-request global registry, deduplication or idempotency promise.

## Fidelity, history and authorization

decode -> Canonical message extensions -> Router -> actual Build body retains id value, role,
message order and ordered content association. The receiving upstream consumes the native field;
CPAR does not use the C4 local diagnostics policy to remove or consume it.

JSON and SSE completion preserve existing assistant/function/reasoning identities and tool/result
association. Explicit history resends each old input ID once. Existing owned previous_response_id
continuation clones prior inputs and appends validated prior output and current input in order;
the resulting complete history is revalidated before lease/send. Repeating a prior ID in the
current continuation input rejects rather than merging, overwriting or deduplicating messages.
Owner, public model, exact route/candidate/credential lineage, TTL and encrypted storage rules
remain. An id creates no owner, route, authorization, idempotency or item_reference lookup right.
Compaction and WebSocket activation are outside this slice.

## Qualification, persistence and evidence

Normal production composition rejects this family before lease/send. Admission requires the
actual adapter, source/target, transform, HTTP transport and independent C6 local qualification.
An owned loopback transport alone, URL, request, User-Agent, header, ordinary configuration or
other field qualification cannot grant it. C6 does not grant namespace, hosted search,
ParallelTools or non-assistant status/phase. Build's default constructor independently rejects
unqualified IDs. Direct representation validation cannot activate a production candidate.

Reuse existing RawExtensions, shared native ID format and cross-item history uniqueness checks;
no identity service, database migration, Canonical/payload-v1/AAD/key/TTL change or backfill.
Existing encrypted stored requests retain exact values. Old execution code still rejects them;
readability is not execution qualification. Preserve old records and keys during any recovery.

Success and rejection use independent receiver observations plus Request/actual Attempt and
applicable Usage/ledger. Unobserved counters/effects/costs are UNKNOWN. Known token accounting
does not qualify #61 cost_in_usd_ticks. Public HTTP RED, controlled JSON/fragmented SSE,
two related tool cycles, explicit/stored history, owner/lineage isolation, cross-kind conflicts,
qualification negatives and existing C1-C5/Pi regression belong to the delivery report.

## Corresponding tests

- `public_build_c6_native_input_message_ids_reach_receiver`

- `public_build_c6_absence_empty_text_and_exact_id_boundaries`
- `public_build_c6_explicit_and_stored_history_keep_two_related_tool_cycles`
- `public_build_c6_stored_continuation_rejects_duplicates_foreign_owner_and_lineage`
- `public_build_c6_invalid_and_unqualified_requests_send_zero`
- `public_build_c6_cross_kind_ids_and_field_qualifications_are_independent`
- `public_build_c6_cannot_qualify_other_adapters_or_passthrough`
- `build_input_message_ids_require_independent_qualification_and_exact_shape`

Delivery evidence: [C6 bounded report](../reports/cpar-62-c6-input-message-id-20261021.md).
